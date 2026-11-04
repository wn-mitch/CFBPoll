//! Forecast scoring.

use serde::{Deserialize, Serialize};

/// One scored forecast.
#[derive(Debug, Clone, Copy)]
pub struct Forecast {
    pub home_win: f64,
    pub margin: Option<f64>,
    pub total: Option<f64>,
    /// Actual home margin and total.
    pub actual_margin: f64,
    pub actual_total: f64,
    pub market_margin: Option<f64>,
    pub market_total: Option<f64>,
}

/// Running accumulator of forecast quality.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scorecard {
    pub games: usize,
    pub log_loss_sum: f64,
    pub brier_sum: f64,
    pub correct: usize,
    pub margin_abs_sum: f64,
    pub margin_n: usize,
    pub total_abs_sum: f64,
    pub total_n: usize,
    /// Games with a market line: model and market absolute margin errors.
    pub vs_market_n: usize,
    pub vs_market_model_abs: f64,
    pub vs_market_market_abs: f64,
    pub total_market_n: usize,
    pub total_market_model_abs: f64,
    pub total_market_market_abs: f64,
}

/// Probabilities are clamped away from 0 and 1 so one overconfident miss
/// cannot make log loss infinite.
const P_FLOOR: f64 = 1e-6;

impl Scorecard {
    pub fn add(&mut self, f: &Forecast) {
        if f.actual_margin == 0.0 {
            // Ties do not occur under overtime rules; skip any data error.
            return;
        }
        let won = f.actual_margin > 0.0;
        let p = f.home_win.clamp(P_FLOOR, 1.0 - P_FLOOR);
        let p_outcome = if won { p } else { 1.0 - p };
        self.games += 1;
        self.log_loss_sum -= p_outcome.ln();
        self.brier_sum += (if won { 1.0 } else { 0.0 } - p).powi(2);
        if (p > 0.5) == won && p != 0.5 {
            self.correct += 1;
        }
        if let Some(m) = f.margin {
            self.margin_abs_sum += (m - f.actual_margin).abs();
            self.margin_n += 1;
            if let Some(mm) = f.market_margin {
                self.vs_market_n += 1;
                self.vs_market_model_abs += (m - f.actual_margin).abs();
                self.vs_market_market_abs += (mm - f.actual_margin).abs();
            }
        }
        if let Some(t) = f.total {
            self.total_abs_sum += (t - f.actual_total).abs();
            self.total_n += 1;
            if let Some(mt) = f.market_total {
                self.total_market_n += 1;
                self.total_market_model_abs += (t - f.actual_total).abs();
                self.total_market_market_abs += (mt - f.actual_total).abs();
            }
        }
    }

    pub fn merge(&mut self, o: &Scorecard) {
        self.games += o.games;
        self.log_loss_sum += o.log_loss_sum;
        self.brier_sum += o.brier_sum;
        self.correct += o.correct;
        self.margin_abs_sum += o.margin_abs_sum;
        self.margin_n += o.margin_n;
        self.total_abs_sum += o.total_abs_sum;
        self.total_n += o.total_n;
        self.vs_market_n += o.vs_market_n;
        self.vs_market_model_abs += o.vs_market_model_abs;
        self.vs_market_market_abs += o.vs_market_market_abs;
        self.total_market_n += o.total_market_n;
        self.total_market_model_abs += o.total_market_model_abs;
        self.total_market_market_abs += o.total_market_market_abs;
    }

    pub fn summary(&self) -> Summary {
        let div = |a: f64, n: usize| if n == 0 { f64::NAN } else { a / n as f64 };
        Summary {
            games: self.games,
            log_loss: div(self.log_loss_sum, self.games),
            brier: div(self.brier_sum, self.games),
            accuracy: div(self.correct as f64, self.games),
            margin_mae: div(self.margin_abs_sum, self.margin_n),
            total_mae: div(self.total_abs_sum, self.total_n),
            market_games: self.vs_market_n,
            model_mae_on_market_games: div(self.vs_market_model_abs, self.vs_market_n),
            market_mae: div(self.vs_market_market_abs, self.vs_market_n),
            model_total_mae_on_market_games: div(self.total_market_model_abs, self.total_market_n),
            market_total_mae: div(self.total_market_market_abs, self.total_market_n),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Summary {
    pub games: usize,
    pub log_loss: f64,
    pub brier: f64,
    pub accuracy: f64,
    pub margin_mae: f64,
    pub total_mae: f64,
    pub market_games: usize,
    pub model_mae_on_market_games: f64,
    pub market_mae: f64,
    pub model_total_mae_on_market_games: f64,
    pub market_total_mae: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(p: f64, margin: f64) -> Forecast {
        Forecast {
            home_win: p,
            margin: None,
            total: None,
            actual_margin: margin,
            actual_total: 50.0,
            market_margin: None,
            market_total: None,
        }
    }

    #[test]
    fn coin_flip_log_loss_is_ln2() {
        let mut s = Scorecard::default();
        s.add(&f(0.5, 7.0));
        s.add(&f(0.5, -3.0));
        let m = s.summary();
        assert!((m.log_loss - std::f64::consts::LN_2).abs() < 1e-12);
        assert!((m.brier - 0.25).abs() < 1e-12);
        assert_eq!(m.accuracy, 0.0, "a 50% forecast picks no one");
    }

    #[test]
    fn certain_miss_is_finite() {
        let mut s = Scorecard::default();
        s.add(&f(1.0, -1.0));
        assert!(s.summary().log_loss.is_finite());
    }

    #[test]
    fn ties_are_skipped() {
        let mut s = Scorecard::default();
        s.add(&f(0.9, 0.0));
        assert_eq!(s.games, 0);
    }
}

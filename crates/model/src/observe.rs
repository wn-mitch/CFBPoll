//! Turns a game into an observation, and a state into a game prediction.

use crate::data::Game;
use crate::kalman::{Gaussian, Observation, Row};
use crate::layout::Layout;
use crate::params::Params;
use serde::{Deserialize, Serialize};

/// Soft cap on a margin: linear near zero, saturating at `±cap`. A
/// non-finite or non-positive cap disables it.
pub fn cap_margin(margin: f64, cap: f64) -> f64 {
    if !(cap.is_finite() && cap > 0.0) {
        return margin;
    }
    cap * (margin / cap).tanh()
}

/// Home-field shift applied to each team's expected score: `+hfa/2` to the
/// home team and `−hfa/2` to the away team, or zero at a neutral site.
pub fn home_shift(game: &Game, params: &Params) -> f64 {
    if game.neutral {
        0.0
    } else {
        params.hfa / 2.0
    }
}

/// Score observation for a completed game. The margin is soft-capped while
/// the total is kept, so a blowout counts like a large-but-finite win.
pub fn score_observation(
    layout: &Layout,
    params: &Params,
    game: &Game,
    score: (f64, f64),
) -> Observation {
    let (h, a) = score;
    let total = h + a;
    let margin = cap_margin(h - a, params.margin_cap);
    let shift = home_shift(game, params);
    let s2 = params.score_sd.powi(2);
    let c = params.score_corr * s2;
    Observation {
        rows: vec![
            layout.points(game.home, game.away),
            layout.points(game.away, game.home),
        ],
        offset: vec![shift, -shift],
        y: vec![(total + margin) / 2.0, (total - margin) / 2.0],
        noise: vec![s2, c, c, s2],
    }
}

/// Pregame prediction for one game.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    pub home_points: f64,
    pub away_points: f64,
    /// Expected home margin.
    pub margin: f64,
    /// Standard deviation of the actual margin (rating uncertainty plus game
    /// noise).
    pub margin_sd: f64,
    pub total: f64,
    pub total_sd: f64,
    pub home_win: f64,
}

pub fn predict(state: &Gaussian, layout: &Layout, params: &Params, game: &Game) -> Prediction {
    let home: Row = layout.points(game.home, game.away);
    let away: Row = layout.points(game.away, game.home);
    let shift = home_shift(game, params);
    let hp = state.combo_mean(&home) + shift;
    let ap = state.combo_mean(&away) - shift;

    let mut margin_row = home.clone();
    margin_row.extend(away.iter().map(|&(i, c)| (i, -c)));
    let mut total_row = home;
    total_row.extend(away);

    let margin_sd = (state.combo_var(&margin_row) + params.margin_var()).sqrt();
    let total_sd = (state.combo_var(&total_row) + params.total_var()).sqrt();
    let margin = hp - ap;
    Prediction {
        home_points: hp,
        away_points: ap,
        margin,
        margin_sd,
        total: hp + ap,
        total_sd,
        home_win: normal_cdf(margin / margin_sd),
    }
}

pub fn normal_cdf(z: f64) -> f64 {
    0.5 * libm::erfc(-z / std::f64::consts::SQRT_2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_is_monotone_and_bounded() {
        let cap = 30.0;
        let mut prev = f64::NEG_INFINITY;
        for m in -100..=100 {
            let c = cap_margin(m as f64, cap);
            assert!(c > prev, "cap must be strictly increasing");
            assert!(c.abs() < cap);
            prev = c;
        }
        assert!(
            (cap_margin(3.0, cap) - 3.0).abs() < 0.05,
            "near-linear for close games"
        );
        assert_eq!(cap_margin(70.0, f64::INFINITY), 70.0);
    }

    #[test]
    fn normal_cdf_values() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-15);
        assert!((normal_cdf(1.959_963_984_540_054) - 0.975).abs() < 1e-12);
        assert!((normal_cdf(-3.0) - 0.001_349_898_031_630_094_5).abs() < 1e-15);
    }
}

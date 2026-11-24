//! Turns a game into an observation, and a state into a game prediction.

use crate::data::Game;
use crate::kalman::{invert_spd, Gaussian, Observation, Row};
use crate::layout::{Layout, MU};
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

/// Observation for a completed game: both scores, plus each offense's
/// readings when present and enabled. The margin is soft-capped while the
/// total is kept, so a blowout counts like a large-but-finite win. Readings
/// are not capped.
///
/// Every reading of one offense measures the same quantity, that offense
/// minus the opposing defense, so the readings are first combined, weighted
/// by their noise covariance, into one equivalent measurement per offense.
/// The update is exactly the one the full set of readings gives, at the cost
/// of a three-row observation however many readings there are.
pub fn game_observation(
    layout: &Layout,
    params: &Params,
    game: &Game,
    score: (f64, f64),
) -> Observation {
    let (h, a) = score;
    let total = h + a;
    let margin = cap_margin(h - a, params.margin_cap);
    let shift = home_shift(game, params);
    let score_rows = vec![
        layout.points(game.home, game.away),
        layout.points(game.away, game.home),
    ];
    let score_y = vec![
        (total + margin) / 2.0 - shift,
        (total - margin) / 2.0 + shift,
    ];

    let mut present: [Vec<(usize, f64)>; 2] = [Vec::new(), Vec::new()];
    if params.use_efficiency {
        for (j, r) in params.readings.iter().enumerate() {
            if let Some((hv, av)) = game.reading(&r.name) {
                present[0].push((j, hv));
                present[1].push((j, av));
            }
        }
    }
    if present[0].is_empty() {
        return Observation {
            rows: score_rows,
            offset: vec![0.0, 0.0],
            y: score_y,
            noise: params.game_noise(&[], &[]),
        };
    }

    // Base combinations: μ, home offense − away defense, and the reverse.
    let z = |off: usize, def: usize| -> Row {
        let mut r = layout.offense(off);
        r.extend(layout.defense(def).into_iter().map(|(i, c)| (i, -c)));
        r
    };
    let base = vec![
        vec![(MU, 1.0)],
        z(game.home, game.away),
        z(game.away, game.home),
    ];
    // Each observation as coefficients on `base`, with its value net of
    // known offsets.
    let mut b: Vec<[f64; 3]> = vec![[1.0, 1.0, 0.0], [1.0, 0.0, 1.0]];
    let mut y = score_y;
    for (side, s) in [(1, shift), (2, -shift)] {
        for &(j, v) in &present[side - 1] {
            let r = &params.readings[j];
            let mut row = [0.0; 3];
            row[side] = r.scale;
            b.push(row);
            y.push(v - r.mean - r.scale * s);
        }
    }
    let idx = |side: usize| present[side].iter().map(|&(j, _)| j).collect::<Vec<_>>();
    let noise = params.game_noise(&idx(0), &idx(1));
    let k = y.len();
    let r_inv = invert_spd(&noise, k).expect("reading noise is positive definite");
    // W = Bᵀ R⁻¹ B and Bᵀ R⁻¹ y; the reduced observation is W⁻¹ Bᵀ R⁻¹ y
    // with noise W⁻¹.
    let mut w = vec![0.0; 9];
    let mut bty = [0.0; 3];
    for i in 0..k {
        for j in 0..k {
            let rij = r_inv[i * k + j];
            for p in 0..3 {
                bty[p] += b[i][p] * rij * y[j];
                for q in 0..3 {
                    w[p * 3 + q] += b[i][p] * rij * b[j][q];
                }
            }
        }
    }
    let w_inv = invert_spd(&w, 3).expect("scores identify all three base combinations");
    let y_hat: Vec<f64> = (0..3)
        .map(|p| (0..3).map(|q| w_inv[p * 3 + q] * bty[q]).sum())
        .collect();
    Observation {
        rows: base,
        offset: vec![0.0; 3],
        y: y_hat,
        noise: w_inv,
    }
}

/// Pregame prediction for one game.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    /// Mean of the model's (unbounded) home points. Can be negative in a
    /// lopsided game; use `home_score` for an expected final score.
    pub home_points: f64,
    pub away_points: f64,
    /// Expected home score with the floor at zero applied: the mean of
    /// `max(0, points)` under the predictive distribution.
    #[serde(default)]
    pub home_score: f64,
    #[serde(default)]
    pub away_score: f64,
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
    let points_sd = |row: &Row| (state.combo_var(row) + params.score_sd.powi(2)).sqrt();
    let home_score = floored_mean(hp, points_sd(&home));
    let away_score = floored_mean(ap, points_sd(&away));

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
        home_score,
        away_score,
        margin,
        margin_sd,
        total: hp + ap,
        total_sd,
        home_win: normal_cdf(margin / margin_sd),
    }
}

/// Mean of `max(0, X)` for `X ~ N(mean, sd²)`: a score's expectation when
/// the model's unbounded points are floored at zero.
pub fn floored_mean(mean: f64, sd: f64) -> f64 {
    if sd <= 0.0 {
        return mean.max(0.0);
    }
    let z = mean / sd;
    let pdf = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
    mean * normal_cdf(z) + sd * pdf
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
    fn floored_mean_never_negative_and_matches_tails() {
        // A heavy underdog's unbounded mean is below zero; the floored
        // expectation is a small positive score.
        let m = floored_mean(-2.0, 13.0);
        assert!(m > 3.0 && m < 6.0, "{m}");
        // Far from the floor the correction vanishes.
        assert!((floored_mean(40.0, 13.0) - 40.0).abs() < 0.05);
        // Never below zero or below the unbounded mean.
        for mean in [-60.0, -20.0, -1.0, 0.0, 1.0, 20.0] {
            let f = floored_mean(mean, 13.0);
            assert!(f >= 0.0 && f >= mean, "mean {mean}: {f}");
        }
        // At mean 0 it is sd / sqrt(2π).
        assert!(
            (floored_mean(0.0, 10.0) - 10.0 / (2.0 * std::f64::consts::PI).sqrt()).abs() < 1e-12
        );
        assert_eq!(floored_mean(-3.0, 0.0), 0.0);
    }

    #[test]
    fn normal_cdf_values() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-15);
        assert!((normal_cdf(1.959_963_984_540_054) - 0.975).abs() < 1e-12);
        assert!((normal_cdf(-3.0) - 0.001_349_898_031_630_094_5).abs() < 1e-15);
    }
}

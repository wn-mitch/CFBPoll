//! Model hyperparameters. Defaults are starting points for `cfbpoll fit`;
//! the fitted values live in the committed `params.json`.

use serde::{Deserialize, Serialize};

/// Prior for one class of team: mean and standard deviation of offense and
/// defense, in points per game relative to the league scoring level.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClassPrior {
    pub mean: f64,
    pub sd: f64,
}

/// One per-game offensive reading (`"ppa"` or a name from
/// `data::EFFICIENCY`), modeled as
/// `mean + scale · (offense − opposing defense + home shift) + error`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub name: String,
    pub mean: f64,
    pub scale: f64,
    /// Standard deviation of the error.
    pub sd: f64,
    /// Correlation of the error with the same offense's score error.
    pub score_corr: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    /// Prior mean of the league scoring level (points per team per game).
    pub mu_mean: f64,
    pub mu_sd: f64,
    /// Home-field advantage in points of margin, split evenly between the
    /// two teams' scores. Zero at neutral sites.
    pub hfa: f64,
    /// Standard deviation of one team's score around its expectation.
    pub score_sd: f64,
    /// Correlation between the two teams' score errors in one game (pace,
    /// weather).
    pub score_corr: f64,
    /// Soft cap on the observed margin: `cap * tanh(margin / cap)`.
    pub margin_cap: f64,
    /// Fresh-start priors for each rated class. Applied to both offense and
    /// defense deviations.
    pub fbs: ClassPrior,
    pub fcs: ClassPrior,
    /// Lower-division pseudo-team priors, redrawn before every game.
    pub division_ii: ClassPrior,
    pub division_iii: ClassPrior,
    pub other: ClassPrior,
    /// Prior sd of each conference's offense and defense level.
    pub conf_sd: f64,
    /// Weekly process-noise sd for team deviations and conference levels.
    pub team_drift_sd: f64,
    pub conf_drift_sd: f64,
    /// Weekly process-noise sd for the scoring level.
    pub mu_drift_sd: f64,
    /// Multiplier on per-team prior sds supplied by the roster-churn model.
    pub prior_sd_scale: f64,
    /// Whether games also update on the offensive readings.
    pub use_efficiency: bool,
    /// Offensive readings each game updates on, when present.
    pub readings: Vec<Reading>,
    /// Row-major correlation matrix of the readings' errors within one
    /// offense. Errors of the two offenses in a game are independent apart
    /// from `score_corr`.
    pub reading_corr: Vec<f64>,
    /// Multiplier on every reading's error sd, fit by `cfbpoll fit`: the
    /// readings' sds are estimated against season-end ratings, which
    /// overstates them by the ratings' own error.
    pub reading_noise_scale: f64,
    /// Prior sd of a quarterback's offense relative to his team's opening
    /// starter. Zero ignores starters.
    pub qb_sd: f64,
    /// Prior mean of a non-opening starter relative to the opener.
    pub qb_new_mean: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            mu_mean: 28.0,
            mu_sd: 3.0,
            hfa: 2.5,
            score_sd: 10.5,
            score_corr: 0.0,
            margin_cap: 40.0,
            fbs: ClassPrior { mean: 0.0, sd: 6.0 },
            fcs: ClassPrior {
                mean: -8.0,
                sd: 6.0,
            },
            division_ii: ClassPrior {
                mean: -14.0,
                sd: 6.0,
            },
            division_iii: ClassPrior {
                mean: -20.0,
                sd: 6.0,
            },
            other: ClassPrior {
                mean: -20.0,
                sd: 8.0,
            },
            conf_sd: 3.0,
            team_drift_sd: 0.6,
            conf_drift_sd: 0.2,
            mu_drift_sd: 0.3,
            prior_sd_scale: 1.0,
            use_efficiency: true,
            readings: vec![Reading {
                name: "ppa".into(),
                mean: 0.1,
                scale: 0.015,
                sd: 0.15,
                score_corr: 0.5,
            }],
            reading_corr: vec![1.0],
            reading_noise_scale: 1.0,
            qb_sd: 0.0,
            qb_new_mean: 0.0,
        }
    }
}

impl Params {
    /// Variance of the observed margin's error.
    pub fn margin_var(&self) -> f64 {
        2.0 * self.score_sd.powi(2) * (1.0 - self.score_corr)
    }

    /// Noise covariance of one game's observations, row-major, in the order
    /// home score, away score, the readings in `home` (indices into
    /// `readings`) for the home offense, then those in `away`.
    pub fn game_noise(&self, home: &[usize], away: &[usize]) -> Vec<f64> {
        let k = 2 + home.len() + away.len();
        let mut r = vec![0.0; k * k];
        let s = self.score_sd;
        let sd = |j: usize| self.readings[j].sd * self.reading_noise_scale;
        let m = self.readings.len();
        r[0] = s * s;
        r[k + 1] = s * s;
        r[1] = self.score_corr * s * s;
        r[k] = r[1];
        let mut at = 2;
        for (side, idx) in [(0, home), (1, away)] {
            let first = at;
            for (a, &j) in idx.iter().enumerate() {
                let x = self.readings[j].score_corr * s * sd(j);
                r[side * k + first + a] = x;
                r[(first + a) * k + side] = x;
                for (b, &l) in idx.iter().enumerate() {
                    let c = if j == l {
                        1.0
                    } else {
                        self.reading_corr[j * m + l]
                    };
                    r[(first + a) * k + first + b] = c * sd(j) * sd(l);
                }
            }
            at += idx.len();
        }
        r
    }

    /// Whether every covariance the model builds is positive definite and
    /// every scale is positive. The fitter treats invalid points as infinitely
    /// bad instead of letting the filter fail.
    pub fn is_valid(&self) -> bool {
        let finite_pos = |v: f64| v.is_finite() && v > 0.0;
        let m = self.readings.len();
        let all: Vec<usize> = (0..m).collect();
        [
            self.score_sd,
            self.reading_noise_scale,
            self.mu_sd,
            self.fbs.sd,
            self.fcs.sd,
        ]
        .into_iter()
        .chain(self.readings.iter().flat_map(|r| [r.sd, r.scale.abs()]))
        .all(finite_pos)
            && self.qb_sd >= 0.0
            && self.reading_corr.len() == m * m
            && crate::kalman::cholesky(&self.game_noise(&all, &all), 2 + 2 * m).is_some()
            && crate::kalman::cholesky(&self.game_noise(&[], &[]), 2).is_some()
    }

    /// Variance of the observed total's error.
    pub fn total_var(&self) -> f64 {
        2.0 * self.score_sd.powi(2) * (1.0 + self.score_corr)
    }
}

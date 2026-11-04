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
    /// Whether games also update on offensive efficiency (PPA per play).
    pub use_efficiency: bool,
    /// League-average garbage-time-excluded offensive PPA per play.
    pub ppa_mean: f64,
    /// PPA per play per point of `offense − opposing defense`.
    pub ppa_scale: f64,
    /// Standard deviation of one offense's per-game PPA around expectation.
    pub ppa_sd: f64,
    /// Correlation between an offense's score error and its PPA error in the
    /// same game.
    pub ppa_score_corr: f64,
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
            ppa_mean: 0.1,
            ppa_scale: 0.015,
            ppa_sd: 0.15,
            ppa_score_corr: 0.5,
        }
    }
}

impl Params {
    /// Variance of the observed margin's error.
    pub fn margin_var(&self) -> f64 {
        2.0 * self.score_sd.powi(2) * (1.0 - self.score_corr)
    }

    /// Noise covariance of one game's readings, row-major, in the order
    /// `[home score, away score, home PPA, away PPA]` (the first two only
    /// when `with_ppa` is false).
    pub fn game_noise(&self, with_ppa: bool) -> Vec<f64> {
        let s2 = self.score_sd.powi(2);
        let c = self.score_corr * s2;
        if !with_ppa {
            return vec![s2, c, c, s2];
        }
        let t2 = self.ppa_sd.powi(2);
        let x = self.ppa_score_corr * self.score_sd * self.ppa_sd;
        vec![
            s2, c, x, 0.0, //
            c, s2, 0.0, x, //
            x, 0.0, t2, 0.0, //
            0.0, x, 0.0, t2,
        ]
    }

    /// Whether every covariance the model builds is positive definite and
    /// every scale is positive. The fitter treats invalid points as infinitely
    /// bad instead of letting the filter fail.
    pub fn is_valid(&self) -> bool {
        let finite_pos = |v: f64| v.is_finite() && v > 0.0;
        [
            self.score_sd,
            self.ppa_sd,
            self.ppa_scale,
            self.mu_sd,
            self.fbs.sd,
            self.fcs.sd,
        ]
        .into_iter()
        .all(finite_pos)
            && crate::kalman::cholesky(&self.game_noise(true), 4).is_some()
            && crate::kalman::cholesky(&self.game_noise(false), 2).is_some()
    }

    /// Variance of the observed total's error.
    pub fn total_var(&self) -> f64 {
        2.0 * self.score_sd.powi(2) * (1.0 + self.score_corr)
    }
}

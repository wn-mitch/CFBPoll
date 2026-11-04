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
        }
    }
}

impl Params {
    /// Variance of the observed margin's error.
    pub fn margin_var(&self) -> f64 {
        2.0 * self.score_sd.powi(2) * (1.0 - self.score_corr)
    }

    /// Variance of the observed total's error.
    pub fn total_var(&self) -> f64 {
        2.0 * self.score_sd.powi(2) * (1.0 + self.score_corr)
    }
}

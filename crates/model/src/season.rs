//! Runs one season game by game: predict, then update.

use crate::data::{Season, Week};
use crate::kalman::Gaussian;
use crate::layout::{class_prior, drift_variances, initial_state, Layout, TeamPrior};
use crate::observe::{predict, score_observation, Prediction};
use crate::params::Params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TeamRating {
    pub offense: f64,
    pub offense_sd: f64,
    pub defense: f64,
    pub defense_sd: f64,
    pub overall: f64,
    pub overall_sd: f64,
}

/// Ratings after every game of `week` (or before any game, for the
/// preseason snapshot, `week == None`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub week: Option<Week>,
    pub ratings: Vec<TeamRating>,
}

#[derive(Debug, Clone)]
pub struct SeasonRun {
    pub layout: Layout,
    pub state: Gaussian,
    /// `(game index, pregame prediction)` for every completed game processed.
    pub predictions: Vec<(usize, Prediction)>,
    pub snapshots: Vec<Snapshot>,
    /// Day of the last processed game; process noise accrues from here.
    pub last_day: Option<i64>,
}

impl SeasonRun {
    pub fn rating(&self, t: usize) -> TeamRating {
        rating(&self.state, &self.layout, t)
    }

    /// Ratings after the last processed game.
    pub fn final_ratings(&self) -> &[TeamRating] {
        &self
            .snapshots
            .last()
            .expect("a run has a preseason snapshot")
            .ratings
    }
}

pub fn rating(state: &Gaussian, layout: &Layout, t: usize) -> TeamRating {
    let o = layout.offense(t);
    let d = layout.defense(t);
    let all = layout.overall(t);
    TeamRating {
        offense: state.combo_mean(&o),
        offense_sd: state.combo_var(&o).sqrt(),
        defense: state.combo_mean(&d),
        defense_sd: state.combo_var(&d).sqrt(),
        overall: state.combo_mean(&all),
        overall_sd: state.combo_var(&all).sqrt(),
    }
}

/// Options for one season run.
#[derive(Debug, Clone, Default)]
pub struct RunOptions<'a> {
    /// Per-team prior overrides (index by team); `None` uses the class prior.
    pub priors: &'a [Option<TeamPrior>],
    /// Process only games strictly before this week.
    pub before: Option<Week>,
}

pub fn run_season(season: &Season, params: &Params, opts: &RunOptions) -> SeasonRun {
    let layout = Layout::new(season);
    let mut state = initial_state(season, &layout, params, opts.priors);
    let drift = drift_variances(season, &layout, params);
    let snapshot = |state: &Gaussian, week: Option<Week>| Snapshot {
        week,
        ratings: (0..season.teams.len())
            .map(|t| rating(state, &layout, t))
            .collect(),
    };

    let mut snapshots = vec![snapshot(&state, None)];
    let mut predictions = Vec::new();
    let mut last_day: Option<i64> = None;
    let mut current_week: Option<Week> = None;

    for (gi, game) in season.games.iter().enumerate() {
        if opts.before.is_some_and(|b| game.week >= b) {
            break;
        }
        let Some(score) = game.score else { continue };

        if current_week.is_some_and(|w| w != game.week) {
            snapshots.push(snapshot(&state, current_week));
        }
        current_week = Some(game.week);

        if let Some(prev) = last_day {
            let weeks = (game.day - prev).max(0) as f64 / 7.0;
            if weeks > 0.0 {
                let scaled: Vec<f64> = drift.iter().map(|v| v * weeks).collect();
                state.add_diagonal(&scaled);
            }
        }
        last_day = Some(game.day.max(last_day.unwrap_or(game.day)));

        // Lower-division opponents are a different school every game.
        for t in [game.home, game.away] {
            let class = season.teams[t].class;
            if !class.is_rated() {
                let p = class_prior(params, class);
                state.reset(layout.team_o(t), p.mean, p.sd.powi(2));
                state.reset(layout.team_d(t), p.mean, p.sd.powi(2));
            }
        }

        predictions.push((gi, predict(&state, &layout, params, game)));
        state.update(&score_observation(&layout, params, game, score));
    }
    if current_week.is_some() {
        snapshots.push(snapshot(&state, current_week));
    }

    SeasonRun {
        layout,
        state,
        predictions,
        snapshots,
        last_day,
    }
}

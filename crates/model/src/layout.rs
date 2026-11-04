//! Maps teams and conferences to state-vector indices, and builds the
//! initial (fresh-start or supplied) prior.

use crate::data::{Class, Season};
use crate::kalman::{Gaussian, Row};
use crate::params::{ClassPrior, Params};

/// State layout: `[μ, (o, d) per team, (co, cd) per conference]`.
///
/// `o` is points scored above the league level; `d` is points prevented
/// (higher is better). A team's effective offense is its conference's `co`
/// plus its own `o`; independents and pseudo-teams have no conference term.
#[derive(Debug, Clone)]
pub struct Layout {
    team_conf: Vec<Option<usize>>,
    pub conferences: Vec<String>,
    n_teams: usize,
}

pub const MU: usize = 0;

impl Layout {
    pub fn new(season: &Season) -> Self {
        let conferences = season.conferences();
        let team_conf = season
            .teams
            .iter()
            .map(|t| {
                if !t.class.is_rated() {
                    return None;
                }
                let c = t.conference.as_ref()?;
                conferences.iter().position(|x| x == c)
            })
            .collect();
        Self {
            team_conf,
            conferences,
            n_teams: season.teams.len(),
        }
    }

    pub fn dim(&self) -> usize {
        1 + 2 * self.n_teams + 2 * self.conferences.len()
    }

    pub fn team_o(&self, t: usize) -> usize {
        1 + 2 * t
    }

    pub fn team_d(&self, t: usize) -> usize {
        2 + 2 * t
    }

    pub fn conf_o(&self, c: usize) -> usize {
        1 + 2 * self.n_teams + 2 * c
    }

    pub fn conf_d(&self, c: usize) -> usize {
        2 + 2 * self.n_teams + 2 * c
    }

    pub fn conference_of(&self, t: usize) -> Option<usize> {
        self.team_conf[t]
    }

    /// Effective offense of team `t` (team deviation plus conference level).
    pub fn offense(&self, t: usize) -> Row {
        let mut r = vec![(self.team_o(t), 1.0)];
        if let Some(c) = self.team_conf[t] {
            r.push((self.conf_o(c), 1.0));
        }
        r
    }

    /// Effective defense of team `t`.
    pub fn defense(&self, t: usize) -> Row {
        let mut r = vec![(self.team_d(t), 1.0)];
        if let Some(c) = self.team_conf[t] {
            r.push((self.conf_d(c), 1.0));
        }
        r
    }

    /// Overall strength `offense + defense`: expected margin against a team
    /// whose offense and defense are both zero.
    pub fn overall(&self, t: usize) -> Row {
        let mut r = self.offense(t);
        r.extend(self.defense(t));
        r
    }

    /// Expected points for `off` against `def`, excluding home field:
    /// `μ + O_off − D_def`.
    pub fn points(&self, off: usize, def: usize) -> Row {
        let mut r = vec![(MU, 1.0)];
        r.extend(self.offense(off));
        r.extend(self.defense(def).into_iter().map(|(i, c)| (i, -c)));
        r
    }
}

pub fn class_prior(params: &Params, class: Class) -> ClassPrior {
    match class {
        Class::Fbs => params.fbs,
        Class::Fcs => params.fcs,
        Class::DivisionII => params.division_ii,
        Class::DivisionIII => params.division_iii,
        Class::Other => params.other,
    }
}

/// Per-team prior on `(o, d)` that overrides the class prior, e.g. from the
/// roster-churn regression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamPrior {
    pub o_mean: f64,
    pub o_sd: f64,
    pub d_mean: f64,
    pub d_sd: f64,
}

/// Initial state: independent priors on every entry. Conference levels start
/// at zero; `overrides[t]` replaces team `t`'s class prior when present.
pub fn initial_state(
    season: &Season,
    layout: &Layout,
    params: &Params,
    overrides: &[Option<TeamPrior>],
) -> Gaussian {
    let n = layout.dim();
    let mut mean = vec![0.0; n];
    let mut var = vec![0.0; n];
    mean[MU] = params.mu_mean;
    var[MU] = params.mu_sd.powi(2);
    for (t, team) in season.teams.iter().enumerate() {
        let p = overrides.get(t).copied().flatten().unwrap_or_else(|| {
            let c = class_prior(params, team.class);
            TeamPrior {
                o_mean: c.mean,
                o_sd: c.sd,
                d_mean: c.mean,
                d_sd: c.sd,
            }
        });
        mean[layout.team_o(t)] = p.o_mean;
        var[layout.team_o(t)] = p.o_sd.powi(2);
        mean[layout.team_d(t)] = p.d_mean;
        var[layout.team_d(t)] = p.d_sd.powi(2);
    }
    for c in 0..layout.conferences.len() {
        var[layout.conf_o(c)] = params.conf_sd.powi(2);
        var[layout.conf_d(c)] = params.conf_sd.powi(2);
    }
    Gaussian::diagonal(mean, &var)
}

/// Weekly process-noise variances for each state entry. Pseudo-teams get
/// none because they are redrawn before every game.
pub fn drift_variances(season: &Season, layout: &Layout, params: &Params) -> Vec<f64> {
    let mut v = vec![0.0; layout.dim()];
    v[MU] = params.mu_drift_sd.powi(2);
    for (t, team) in season.teams.iter().enumerate() {
        if team.class.is_rated() {
            v[layout.team_o(t)] = params.team_drift_sd.powi(2);
            v[layout.team_d(t)] = params.team_drift_sd.powi(2);
        }
    }
    for c in 0..layout.conferences.len() {
        v[layout.conf_o(c)] = params.conf_drift_sd.powi(2);
        v[layout.conf_d(c)] = params.conf_drift_sd.powi(2);
    }
    v
}

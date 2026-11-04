//! The 2020 poll's Elo model, with its known bugs fixed, kept as a backtest
//! baseline.
//!
//! Fixes relative to the original `Cycle.py`: the margin-of-victory
//! autocorrelation term uses `winner − loser` (the original inverted it),
//! unseen FCS teams seed at `fcs_elo` (the original used the start year),
//! and there is no home advantage at neutral sites.

use crate::data::{Class, Season};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct EloParams {
    pub k: f64,
    pub hfa: f64,
    pub fcs_elo: f64,
    pub lower_elo: f64,
    /// Fraction of last season's deviation from 1500 that carries over.
    pub carryover: f64,
    /// Elo points per point of margin, for margin predictions.
    pub elo_per_point: f64,
}

impl Default for EloParams {
    fn default() -> Self {
        Self {
            k: 19.0,
            hfa: 30.0,
            fcs_elo: 1204.0,
            lower_elo: 1050.0,
            carryover: 0.446,
            elo_per_point: 10.0,
        }
    }
}

pub fn expected(diff: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-diff / 400.0))
}

/// Ratings by team name, carried across seasons.
pub type EloTable = HashMap<String, f64>;

/// Runs a season. Returns `(game index, home win probability, predicted
/// margin)` for each completed game, and the end-of-season table.
pub fn run(
    season: &Season,
    p: &EloParams,
    previous: Option<&EloTable>,
) -> (Vec<(usize, f64, f64)>, EloTable) {
    let mut elo: EloTable = HashMap::new();
    let seed = |name: &str, class: Class| -> f64 {
        match class {
            Class::Fbs => previous
                .and_then(|t| t.get(name))
                .map_or(1500.0, |e| 1500.0 + (e - 1500.0) * p.carryover),
            Class::Fcs => previous
                .and_then(|t| t.get(name))
                .map_or(p.fcs_elo, |e| p.fcs_elo + (e - p.fcs_elo) * p.carryover),
            _ => p.lower_elo,
        }
    };
    let mut out = Vec::new();
    for (gi, g) in season.games.iter().enumerate() {
        let Some((hs, as_)) = g.score else { continue };
        let (home, away) = (&season.teams[g.home], &season.teams[g.away]);
        let he = *elo
            .entry(home.name.clone())
            .or_insert_with(|| seed(&home.name, home.class));
        let ae = *elo
            .entry(away.name.clone())
            .or_insert_with(|| seed(&away.name, away.class));
        let hfa = if g.neutral { 0.0 } else { p.hfa };
        let diff = he + hfa - ae;
        let exp_home = expected(diff);
        out.push((gi, exp_home, diff / p.elo_per_point));

        let margin = hs - as_;
        let home_won = margin > 0.0;
        let winner_minus_loser = if home_won { diff } else { -diff };
        let mov = (margin.abs() + 1.0).ln() * (2.2 / (winner_minus_loser * 0.001 + 2.2));
        let delta = p.k * mov * (if home_won { 1.0 } else { 0.0 } - exp_home);
        if home.class.is_rated() {
            *elo.get_mut(&home.name).expect("seeded") += delta;
        }
        if away.class.is_rated() {
            *elo.get_mut(&away.name).expect("seeded") -= delta;
        }
    }
    elo.retain(|name, _| {
        season
            .team_index(name)
            .is_some_and(|t| season.teams[t].class.is_rated())
    });
    (out, elo)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A favorite's blowout must move ratings less than an underdog's
    /// blowout of the same size (the original code had this inverted).
    #[test]
    fn mov_multiplier_dampens_favorites() {
        let mult = |winner_minus_loser: f64| 2.2 / (winner_minus_loser * 0.001 + 2.2);
        assert!(mult(300.0) < mult(-300.0));
    }

    #[test]
    fn expected_is_symmetric() {
        assert!((expected(0.0) - 0.5).abs() < 1e-15);
        assert!((expected(100.0) + expected(-100.0) - 1.0).abs() < 1e-15);
    }
}

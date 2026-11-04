//! Builds roster-churn prior features and training examples from cached
//! CFBD data and fresh-start season runs.

use anyhow::Result;
use cfbd::{Cache, Dataset, PlayerTransfer, ReturningProduction, RosterPlayer, TeamTalent};
use model::data::{Class, Season};
use model::params::Params;
use model::prior::{position_group, Example, Features};
use model::season::{run_season, RunOptions};
use std::collections::{HashMap, HashSet};

/// End-of-season fresh-start `(offense, defense)` per rated team, relative
/// to that season's FBS average, plus each team's class.
#[derive(Debug, Clone, Default)]
pub struct Finals {
    pub ratings: HashMap<String, (f64, f64)>,
    pub class: HashMap<String, Class>,
}

pub fn fresh_finals(season: &Season, params: &Params) -> Finals {
    let run = run_season(season, params, &RunOptions::default());
    let r = run.final_ratings();
    let fbs: Vec<usize> = (0..season.teams.len())
        .filter(|&t| season.teams[t].class == Class::Fbs)
        .collect();
    let n = fbs.len().max(1) as f64;
    let mo = fbs.iter().map(|&t| r[t].offense).sum::<f64>() / n;
    let md = fbs.iter().map(|&t| r[t].defense).sum::<f64>() / n;
    let mut out = Finals::default();
    for (t, team) in season.teams.iter().enumerate() {
        if team.class.is_rated() {
            out.ratings
                .insert(team.name.clone(), (r[t].offense - mo, r[t].defense - md));
            out.class.insert(team.name.clone(), team.class);
        }
    }
    out
}

/// Fresh-start finals for many seasons, in parallel.
pub fn fresh_finals_all(seasons: &[Season], params: &Params) -> HashMap<i32, Finals> {
    std::thread::scope(|s| {
        let handles: Vec<_> = seasons
            .iter()
            .map(|season| s.spawn(move || (season.year, fresh_finals(season, params))))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("season run panicked"))
            .collect()
    })
}

/// Portal rating used when a transfer has stars but no rating.
fn stars_rating(stars: Option<i32>) -> f64 {
    match stars {
        Some(5) => 0.98,
        Some(4) => 0.92,
        Some(3) => 0.85,
        Some(2) => 0.78,
        _ => 0.80,
    }
}

/// Offseason data for the season `year`: everything needed to build each
/// team's features except last season's ratings.
pub struct Offseason {
    returning: HashMap<String, ReturningProduction>,
    continuity: HashMap<String, [f64; 6]>,
    portal: Option<HashMap<String, [f64; 6]>>,
    talent: HashMap<String, f64>,
}

impl Offseason {
    pub fn load(cache: &Cache, year: i32) -> Result<Offseason> {
        let returning = cache
            .load_optional::<ReturningProduction>(year, Dataset::Returning)?
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.team.clone(), r))
            .collect();
        let talent = cache
            .load_optional::<TeamTalent>(year, Dataset::Talent)?
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.team, t.talent))
            .collect();

        let roster_now = cache.load_optional::<RosterPlayer>(year, Dataset::Roster)?;
        let roster_prev = cache.load_optional::<RosterPlayer>(year - 1, Dataset::Roster)?;
        let continuity = match (roster_prev, roster_now) {
            (Some(prev), Some(now)) => continuity(&prev, &now),
            _ => HashMap::new(),
        };

        let portal = cache
            .load_optional::<PlayerTransfer>(year, Dataset::Portal)?
            .filter(|rows| !rows.is_empty())
            .map(|rows| portal_flow(&rows));
        Ok(Offseason {
            returning,
            continuity,
            portal,
            talent,
        })
    }

    pub fn features(&self, team: &str, class: Class, prev: Option<(f64, f64)>) -> Features {
        let r = self.returning.get(team);
        Features {
            fcs: class == Class::Fcs,
            prev,
            returning_ppa: r.map(|r| {
                [
                    r.percent_ppa,
                    r.percent_passing_ppa,
                    r.percent_rushing_ppa,
                    r.percent_receiving_ppa,
                ]
            }),
            returning_usage: r
                .map(|r| [r.usage, r.passing_usage, r.rushing_usage, r.receiving_usage]),
            continuity: self.continuity.get(team).copied(),
            portal: self
                .portal
                .as_ref()
                .map(|p| p.get(team).copied().unwrap_or([0.0; 6])),
            talent: self.talent.get(team).copied(),
        }
    }
}

/// Share of each team's previous roster, per position group, that appears
/// on the team's current roster (matched by player id).
fn continuity(prev: &[RosterPlayer], now: &[RosterPlayer]) -> HashMap<String, [f64; 6]> {
    let current: HashSet<(&str, &str)> = now
        .iter()
        .map(|p| (p.team.as_str(), p.id.as_str()))
        .collect();
    let mut counts: HashMap<&str, ([f64; 6], [f64; 6])> = HashMap::new();
    for p in prev {
        let Some(g) = p.position.as_deref().and_then(position_group) else {
            continue;
        };
        let e = counts.entry(p.team.as_str()).or_default();
        e.0[g] += 1.0;
        if current.contains(&(p.team.as_str(), p.id.as_str())) {
            e.1[g] += 1.0;
        }
    }
    // Teams absent from the current roster file have no data, not zero
    // continuity.
    let teams_now: HashSet<&str> = now.iter().map(|p| p.team.as_str()).collect();
    counts
        .into_iter()
        .filter(|(t, _)| teams_now.contains(t))
        .map(|(t, (total, kept))| {
            let mut share = [0.5; 6];
            for g in 0..6 {
                if total[g] > 0.0 {
                    share[g] = kept[g] / total[g];
                }
            }
            (t.to_string(), share)
        })
        .collect()
}

/// Net incoming transfer quality per team and position group.
fn portal_flow(rows: &[PlayerTransfer]) -> HashMap<String, [f64; 6]> {
    let mut out: HashMap<String, [f64; 6]> = HashMap::new();
    for t in rows {
        let Some(g) = position_group(&t.position) else {
            continue;
        };
        let q = t.rating.unwrap_or_else(|| stars_rating(t.stars));
        out.entry(t.origin.clone()).or_default()[g] -= q;
        if let Some(dest) = &t.destination {
            out.entry(dest.clone()).or_default()[g] += q;
        }
    }
    out
}

/// Features for every rated team entering `season`.
pub fn season_features(
    season: &Season,
    off: &Offseason,
    prev: Option<&Finals>,
) -> Vec<Option<Features>> {
    season
        .teams
        .iter()
        .map(|t| {
            t.class.is_rated().then(|| {
                let p = prev.and_then(|f| f.ratings.get(&t.name).copied());
                off.features(&t.name, t.class, p)
            })
        })
        .collect()
}

/// Training examples for one season: features entering it and its
/// fresh-start finals as targets.
pub fn examples(
    season: &Season,
    off: &Offseason,
    prev: Option<&Finals>,
    finals: &Finals,
) -> Vec<Example> {
    season
        .teams
        .iter()
        .zip(season_features(season, off, prev))
        .filter_map(|(t, f)| {
            Some(Example {
                year: season.year,
                team: t.name.clone(),
                features: f?,
                target: *finals.ratings.get(&t.name)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(team: &str, id: &str, pos: &str) -> RosterPlayer {
        serde_json::from_value(serde_json::json!({
            "id": id, "team": team, "year": 1, "position": pos
        }))
        .unwrap()
    }

    #[test]
    fn continuity_by_group() {
        let prev = vec![
            player("A", "1", "QB"),
            player("A", "2", "QB"),
            player("A", "3", "CB"),
            player("B", "4", "QB"),
        ];
        let now = vec![
            player("A", "1", "QB"),
            player("A", "3", "S"),
            player("A", "9", "QB"),
        ];
        let c = continuity(&prev, &now);
        assert_eq!(c["A"][0], 0.5, "one of two QBs returned");
        assert_eq!(c["A"][5], 1.0, "a CB who moved to S still counts for DB");
        assert!(
            !c.contains_key("B"),
            "team missing from the new roster has no data"
        );
    }

    #[test]
    fn portal_nets_in_and_out() {
        let t = |pos: &str, from: &str, to: Option<&str>, rating: Option<f64>| -> PlayerTransfer {
            serde_json::from_value(serde_json::json!({
                "season": 2024, "position": pos, "origin": from,
                "destination": to, "rating": rating, "stars": 3
            }))
            .unwrap()
        };
        let flow = portal_flow(&[
            t("QB", "A", Some("B"), Some(0.9)),
            t("QB", "C", Some("A"), None),
            t("WR", "A", None, Some(0.8)),
        ]);
        assert!((flow["A"][0] - (0.85 - 0.9)).abs() < 1e-12);
        assert!(
            (flow["A"][1] + 0.8).abs() < 1e-12,
            "unplaced departures still leave"
        );
        assert!((flow["B"][0] - 0.9).abs() < 1e-12);
    }
}

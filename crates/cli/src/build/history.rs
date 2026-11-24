//! Week-by-week rank, rating and results for each FBS team, read by the
//! expandable rankings rows and the team page.

use super::schema::{GameRef, WeekPoint};
use super::teams::{slug, week_label};
use super::Built;
use model::bias::ranks_with_range;
use model::data::Class;
use std::collections::HashMap;

/// Draws for the per-week rank ranges. The ranges come from independent
/// normal draws of each team's rating at that week (see
/// `ranks_with_range`); the latest week instead uses the simulation's
/// correlated range so it matches the ranking row.
const DRAWS: usize = 400;

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// One entry per snapshot (preseason first) for every FBS team in
/// `b.order`.
pub fn by_team(b: &Built) -> HashMap<usize, Vec<WeekPoint>> {
    let fbs = &b.order;
    let snaps = &b.run.snapshots;
    let mut out: HashMap<usize, Vec<WeekPoint>> = fbs
        .iter()
        .map(|&t| (t, Vec::with_capacity(snaps.len())))
        .collect();
    let mut prev_rel: HashMap<usize, f64> = HashMap::new();
    for (si, s) in snaps.iter().enumerate() {
        let avg = fbs.iter().map(|&t| s.ratings[t].overall).sum::<f64>() / fbs.len().max(1) as f64;
        let ranks = ranks_with_range(s, fbs, DRAWS, 0x5eed_0000 + si as u64);
        let last = si + 1 == snaps.len();
        for (&t, (rank, mut low, mut high)) in fbs.iter().zip(ranks) {
            if last {
                if let Some(o) = b.sim.teams[t].as_ref() {
                    (low, high) = (o.rank_p05.min(rank), o.rank_p95.max(rank));
                }
            }
            let rel = s.ratings[t].overall - avg;
            let games = s
                .week
                .map(|w| {
                    b.season
                        .games
                        .iter()
                        .filter(|g| g.week == w && (g.home == t || g.away == t))
                        .filter_map(|g| {
                            let (h, a) = g.score?;
                            let home = g.home == t;
                            let opp = if home { g.away } else { g.home };
                            let team = &b.season.teams[opp];
                            Some(GameRef {
                                opponent: team.name.clone(),
                                opponent_slug: (team.class == Class::Fbs).then(|| slug(&team.name)),
                                opponent_logo_id: team.id,
                                site: if g.neutral {
                                    "neutral"
                                } else if home {
                                    "home"
                                } else {
                                    "away"
                                },
                                points_for: if home { h } else { a },
                                points_against: if home { a } else { h },
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.get_mut(&t).expect("FBS team").push(WeekPoint {
                label: week_label(s.week),
                rank,
                rank_low: low,
                rank_high: high,
                overall: round1(rel),
                change: prev_rel.get(&t).map(|p| round1(rel - p)),
                games,
            });
            prev_rel.insert(t, rel);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::data::{Game, Season, Team, Week};
    use model::params::Params;
    use model::playoff::PlayoffRules;
    use model::season::{run_season, RunOptions};
    use model::sim::{simulate, SimOptions};

    fn team(name: &str, class: Class) -> Team {
        Team {
            name: name.into(),
            class,
            conference: (class == Class::Fbs).then(|| "SEC".into()),
            id: None,
        }
    }

    fn game(id: i64, week: i32, home: usize, away: usize, score: Option<(f64, f64)>) -> Game {
        Game {
            id,
            week: Week {
                postseason: false,
                number: week,
            },
            day: 20_000 + 7 * week as i64,
            neutral: false,
            conference_game: false,
            championship: false,
            home,
            away,
            score,
            ppa: None,
            efficiency: Vec::new(),
            starters: (None, None),
            market_margin: None,
            market_total: None,
            cfbd_elo: None,
        }
    }

    /// Four FBS teams and one FCS team over three played weeks plus one
    /// unplayed week. Team 3 has a bye in week 2.
    fn season() -> Season {
        Season {
            year: 2030,
            teams: vec![
                team("A", Class::Fbs),
                team("B", Class::Fbs),
                team("C", Class::Fbs),
                team("D", Class::Fbs),
                team("Lower", Class::Fcs),
            ],
            games: vec![
                game(1, 1, 0, 1, Some((35.0, 10.0))),
                game(2, 1, 2, 3, Some((14.0, 21.0))),
                game(3, 2, 1, 2, Some((24.0, 27.0))),
                game(4, 2, 0, 4, Some((56.0, 3.0))),
                game(5, 3, 3, 0, Some((17.0, 20.0))),
                game(6, 3, 1, 2, Some((30.0, 13.0))),
                game(7, 4, 0, 2, None),
            ],
        }
    }

    #[test]
    fn history_tracks_every_week_and_game() {
        let season = season();
        let params = Params::default();
        let run = run_season(
            &season,
            &params,
            &RunOptions {
                priors: &[],
                before: None,
            },
        );
        let sim = simulate(
            &season,
            &run,
            &params,
            &PlayoffRules::cfp_2026(),
            SimOptions {
                draws: 300,
                ..SimOptions::default()
            },
        );
        let ratings = run.final_ratings();
        let mut order: Vec<usize> = (0..4).collect();
        order.sort_by(|&a, &b| ratings[b].overall.total_cmp(&ratings[a].overall));
        let snapshots = run.snapshots.len();
        let mut b = Built {
            season: &season,
            params: &params,
            run,
            sim,
            order,
            avg: (0.0, 0.0, 0.0),
            history: HashMap::new(),
        };
        b.history = by_team(&b);

        assert_eq!(b.history.len(), 4, "FBS teams only");
        for (&t, h) in &b.history {
            assert_eq!(h.len(), snapshots);
            assert_eq!(h[0].label, "Preseason");
            assert!(h[0].change.is_none() && h[0].games.is_empty());
            for p in h {
                assert!(p.rank_low <= p.rank && p.rank <= p.rank_high, "{t}: {p:?}");
            }
            let last = h.last().unwrap();
            assert_eq!(
                Some(last.rank),
                b.rank_of(t),
                "latest week matches the row rank"
            );
            // Every played game shows up once, in its own week, from this
            // team's side; unplayed games never do.
            let listed: usize = h.iter().map(|p| p.games.len()).sum();
            let played = season
                .games
                .iter()
                .filter(|g| g.score.is_some() && (g.home == t || g.away == t))
                .count();
            assert_eq!(listed, played, "team {t}");
        }
        // Weekly ranks are a permutation of 1..=4.
        for i in 0..snapshots {
            let mut ranks: Vec<usize> = b.history.values().map(|h| h[i].rank).collect();
            ranks.sort_unstable();
            assert_eq!(ranks, vec![1, 2, 3, 4], "snapshot {i}");
        }

        let d = &b.history[&3];
        let wk = |label: &str| d.iter().find(|p| p.label == label).unwrap();
        assert!(wk("Week 2").games.is_empty(), "bye week");
        let w3 = &wk("Week 3").games[0];
        assert_eq!((w3.opponent.as_str(), w3.site), ("A", "home"));
        assert_eq!((w3.points_for, w3.points_against), (17.0, 20.0));
        let w1 = &wk("Week 1").games[0];
        assert_eq!(
            (w1.site, w1.points_for, w1.points_against),
            ("away", 21.0, 14.0)
        );

        let a = &b.history[&0];
        let fcs = &a.iter().find(|p| p.label == "Week 2").unwrap().games[0];
        assert_eq!(fcs.opponent, "Lower");
        assert!(
            fcs.opponent_slug.is_none(),
            "lower-division teams have no page"
        );
        let c = a
            .iter()
            .find(|p| p.label == "Week 1")
            .unwrap()
            .change
            .unwrap();
        assert!(c > 0.0, "a 25-point win raises the rating: {c}");
    }
}

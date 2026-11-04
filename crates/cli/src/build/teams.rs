//! Builds the per-page JSON documents from a season run.

use super::schema::*;
use super::Built;
use crate::season::{civil_from_days, today};
use model::data::{Class, Game, Week};
use model::observe::normal_cdf;
use std::collections::HashMap;

pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    s.trim_end_matches('-').to_string()
}

pub fn week_label(w: Option<Week>) -> String {
    match w {
        None => "Preseason".into(),
        Some(w) if w.postseason => "Postseason".into(),
        Some(w) => format!("Week {}", w.number),
    }
}

fn date(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn site(g: &Game, home_side: bool) -> &'static str {
    if g.neutral {
        "neutral"
    } else if home_side {
        "home"
    } else {
        "away"
    }
}

impl Built<'_> {
    fn is_fbs(&self, t: usize) -> bool {
        self.season.teams[t].class == Class::Fbs
    }

    fn team_slug(&self, t: usize) -> Option<String> {
        self.is_fbs(t).then(|| slug(&self.season.teams[t].name))
    }

    fn games_of(&self, t: usize) -> impl Iterator<Item = (usize, &Game)> {
        self.season
            .games
            .iter()
            .enumerate()
            .filter(move |(_, g)| g.home == t || g.away == t)
    }

    fn record(&self, t: usize) -> Record {
        let mut r = Record {
            wins: 0,
            losses: 0,
            conf_wins: 0,
            conf_losses: 0,
        };
        for (_, g) in self.games_of(t) {
            let Some((h, a)) = g.score else { continue };
            let won = (g.home == t) == (h > a);
            if won {
                r.wins += 1;
            } else {
                r.losses += 1;
            }
            if g.conference_game && !g.championship {
                if won {
                    r.conf_wins += 1;
                } else {
                    r.conf_losses += 1;
                }
            }
        }
        r
    }

    fn rel_overall(&self, t: usize) -> f64 {
        self.run.final_ratings()[t].overall - self.avg.0
    }

    fn row(&self, t: usize, ranks_prev: &HashMap<usize, usize>) -> RankRow {
        let team = &self.season.teams[t];
        let r = self.run.final_ratings()[t];
        let prev = self
            .run
            .snapshots
            .iter()
            .rev()
            .nth(1)
            .map(|s| s.ratings[t].overall);
        let prev_avg = self.run.snapshots.iter().rev().nth(1).map(|s| {
            let fbs: Vec<usize> = self.order.clone();
            fbs.iter().map(|&x| s.ratings[x].overall).sum::<f64>() / fbs.len().max(1) as f64
        });
        let last_game = self
            .games_of(t)
            .filter(|(_, g)| g.score.is_some())
            .last()
            .map(|(_, g)| {
                let (h, a) = g.score.unwrap();
                let home = g.home == t;
                let opp = if home { g.away } else { g.home };
                GameRef {
                    opponent: self.season.teams[opp].name.clone(),
                    opponent_slug: self.team_slug(opp),
                    site: site(g, home),
                    points_for: if home { h } else { a },
                    points_against: if home { a } else { h },
                }
            });
        let odds = self.sim.teams[t].as_ref();
        let (sos, top25) = self.schedule_strength(t);
        RankRow {
            rank: self.rank_of(t).unwrap_or(0),
            rank_low: odds.map_or(0, |o| o.rank_p05),
            rank_high: odds.map_or(0, |o| o.rank_p95),
            previous_rank: ranks_prev.get(&t).copied(),
            team: team.name.clone(),
            slug: slug(&team.name),
            conference: team.conference.clone(),
            record: self.record(t),
            overall: Rated {
                value: round1(r.overall - self.avg.0),
                sd: round1(r.overall_sd),
            },
            offense: Rated {
                value: round1(r.offense - self.avg.1),
                sd: round1(r.offense_sd),
            },
            defense: Rated {
                value: round1(r.defense - self.avg.2),
                sd: round1(r.defense_sd),
            },
            change: round1(match (prev, prev_avg) {
                (Some(p), Some(pa)) => (r.overall - self.avg.0) - (p - pa),
                _ => 0.0,
            }),
            last_game,
            schedule_strength: round1(sos),
            top25_expected_losses: round1(top25),
        }
    }

    /// Average opponent rating over the full schedule, and the expected
    /// losses of the 25th-ranked team against it.
    fn schedule_strength(&self, t: usize) -> (f64, f64) {
        let bench = self.order.get(24).map_or(0.0, |&b| self.rel_overall(b));
        let noise = self.params.margin_var();
        let ratings = self.run.final_ratings();
        let mut sum = 0.0;
        let mut n = 0.0;
        let mut losses = 0.0;
        for (_, g) in self.games_of(t) {
            if g.week.postseason {
                continue;
            }
            let home = g.home == t;
            let opp = if home { g.away } else { g.home };
            let r = &ratings[opp];
            let opp_rel = r.overall - self.avg.0;
            sum += opp_rel;
            n += 1.0;
            let hfa = if g.neutral {
                0.0
            } else if home {
                self.params.hfa
            } else {
                -self.params.hfa
            };
            let sd = (r.overall_sd.powi(2) + noise).sqrt();
            losses += 1.0 - normal_cdf((bench - opp_rel + hfa) / sd);
        }
        (if n > 0.0 { sum / n } else { 0.0 }, losses)
    }

    fn previous_ranks(&self) -> HashMap<usize, usize> {
        let Some(prev) = self.run.snapshots.iter().rev().nth(1) else {
            return HashMap::new();
        };
        if prev.week.is_none() && self.run.snapshots.len() <= 2 {
            return HashMap::new();
        }
        let mut order = self.order.clone();
        order.sort_by(|&a, &b| prev.ratings[b].overall.total_cmp(&prev.ratings[a].overall));
        order
            .into_iter()
            .enumerate()
            .map(|(i, t)| (t, i + 1))
            .collect()
    }

    fn odds_out(&self, t: usize) -> Option<TeamOddsOut> {
        self.sim.teams[t].as_ref().map(|o| TeamOddsOut {
            expected_wins: round1(o.expected_wins),
            expected_losses: round1(o.expected_losses),
            conf_title: round3(o.conf_title),
            conf_title_game: round3(o.conf_title_game),
            playoff: round3(o.playoff),
            bye: round3(o.bye),
            semifinal: round3(o.semifinal),
            title: round3(o.title),
        })
    }

    fn game_prediction(&self, gi: usize) -> GamePrediction {
        let g = &self.season.games[gi];
        let p = match g.score {
            Some(_) => self
                .run
                .predictions
                .iter()
                .find(|(i, _)| *i == gi)
                .map(|(_, p)| *p)
                .unwrap_or_else(|| self.predict_future(g)),
            None => self.predict_future(g),
        };
        GamePrediction {
            id: g.id,
            date: date(g.day),
            week_label: week_label(Some(g.week)),
            home: self.season.teams[g.home].name.clone(),
            home_slug: self.team_slug(g.home),
            home_rank: self.rank_of(g.home),
            away: self.season.teams[g.away].name.clone(),
            away_slug: self.team_slug(g.away),
            away_rank: self.rank_of(g.away),
            neutral: g.neutral,
            home_win: round3(p.home_win),
            home_points: round1(p.home_points),
            away_points: round1(p.away_points),
            margin_sd: round1(p.margin_sd),
            market_margin: g.market_margin,
            market_total: g.market_total,
            result: g.score,
        }
    }
}

pub fn meta(b: &Built, uses_prior: bool) -> Meta {
    let latest = b.run.snapshots.last().and_then(|s| s.week);
    let (y, m, d) = today();
    Meta {
        year: b.season.year,
        as_of: week_label(latest),
        generated: format!("{y:04}-{m:02}-{d:02}"),
        games_played: b.season.games.iter().filter(|g| g.score.is_some()).count(),
        games_remaining: b.season.games.iter().filter(|g| g.score.is_none()).count(),
        uses_prior,
        draws: b.sim.draws,
        hfa: round1(b.params.hfa),
    }
}

pub fn rankings(b: &Built) -> Vec<RankRow> {
    let prev = b.previous_ranks();
    b.order.iter().map(|&t| b.row(t, &prev)).collect()
}

pub fn team_pages(b: &Built) -> Vec<(String, TeamPage)> {
    let prev = b.previous_ranks();
    b.order
        .iter()
        .map(|&t| {
            let fbs = &b.order;
            let trajectory = b
                .run
                .snapshots
                .iter()
                .map(|s| {
                    let n = fbs.len().max(1) as f64;
                    let avg = |f: fn(&model::season::TeamRating) -> f64| {
                        fbs.iter().map(|&x| f(&s.ratings[x])).sum::<f64>() / n
                    };
                    let r = s.ratings[t];
                    TrajectoryPoint {
                        label: week_label(s.week),
                        overall: round1(r.overall - avg(|r| r.overall)),
                        overall_sd: round1(r.overall_sd),
                        offense: round1(r.offense - avg(|r| r.offense)),
                        defense: round1(r.defense - avg(|r| r.defense)),
                    }
                })
                .collect();
            let schedule = b
                .games_of(t)
                .map(|(gi, g)| {
                    let p = b.game_prediction(gi);
                    let home = g.home == t;
                    let opp = if home { g.away } else { g.home };
                    ScheduleGame {
                        id: g.id,
                        date: p.date.clone(),
                        week_label: p.week_label.clone(),
                        opponent: b.season.teams[opp].name.clone(),
                        opponent_slug: b.team_slug(opp),
                        opponent_rank: b.rank_of(opp),
                        site: site(g, home),
                        conference_game: g.conference_game,
                        result: g.score.map(|(h, a)| if home { (h, a) } else { (a, h) }),
                        win_prob: if home {
                            p.home_win
                        } else {
                            round3(1.0 - p.home_win)
                        },
                        points_for: if home { p.home_points } else { p.away_points },
                        points_against: if home { p.away_points } else { p.home_points },
                        market_margin: g.market_margin.map(|m| if home { m } else { -m }),
                    }
                })
                .collect();
            let page = TeamPage {
                row: b.row(t, &prev),
                trajectory,
                schedule,
                odds: b.odds_out(t),
            };
            (slug(&b.season.teams[t].name), page)
        })
        .collect()
}

/// The current (first unfinished) week's games involving an FBS team, and
/// the last finished week's results.
pub fn games(b: &Built) -> Games {
    let involves_fbs = |g: &Game| b.is_fbs(g.home) || b.is_fbs(g.away);
    let next = b
        .season
        .games
        .iter()
        .filter(|g| g.score.is_none() && involves_fbs(g))
        .map(|g| g.week)
        .min();
    // The latest week with results that is not still in progress.
    let last = b
        .season
        .games
        .iter()
        .filter(|g| g.score.is_some() && involves_fbs(g))
        .map(|g| g.week)
        .filter(|w| next.is_none_or(|n| *w < n))
        .max();
    let pick = |w: Option<Week>| -> Vec<GamePrediction> {
        let Some(w) = w else { return Vec::new() };
        let mut v: Vec<GamePrediction> = (0..b.season.games.len())
            .filter(|&i| {
                let g = &b.season.games[i];
                g.week == w && involves_fbs(g)
            })
            .map(|i| b.game_prediction(i))
            .collect();
        // Best matchups first: lowest sum of ranks among ranked teams.
        v.sort_by_key(|p| p.home_rank.unwrap_or(200) + p.away_rank.unwrap_or(200));
        v
    };
    Games {
        upcoming_label: next.map(|w| week_label(Some(w))),
        upcoming: pick(next),
        recent_label: last.map(|w| week_label(Some(w))),
        recent: pick(last),
    }
}

pub fn conferences(b: &Built) -> Vec<ConferenceRow> {
    let layout = &b.run.layout;
    let state = &b.run.state;
    let fbs_confs: Vec<(usize, &String)> = layout
        .conferences
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            b.order
                .iter()
                .any(|&t| b.season.teams[t].conference.as_ref() == Some(*name))
        })
        .collect();
    let level_row = |c: usize| vec![(layout.conf_o(c), 1.0), (layout.conf_d(c), 1.0)];
    let mean_level = fbs_confs
        .iter()
        .map(|&(c, _)| state.combo_mean(&level_row(c)))
        .sum::<f64>()
        / fbs_confs.len().max(1) as f64;
    let mut rows: Vec<ConferenceRow> = fbs_confs
        .iter()
        .map(|&(c, name)| {
            let members: Vec<usize> = b
                .order
                .iter()
                .copied()
                .filter(|&t| b.season.teams[t].conference.as_ref() == Some(name))
                .collect();
            let ratings: Vec<f64> = members.iter().map(|&t| round1(b.rel_overall(t))).collect();
            let nonconf = b
                .season
                .games
                .iter()
                .filter(|g| g.score.is_some() && !g.conference_game)
                .filter(|g| members.contains(&g.home) || members.contains(&g.away))
                .count();
            ConferenceRow {
                name: name.clone(),
                level: Rated {
                    value: round1(state.combo_mean(&level_row(c)) - mean_level),
                    sd: round1(state.combo_var(&level_row(c)).sqrt()),
                },
                member_average: round1(ratings.iter().sum::<f64>() / ratings.len().max(1) as f64),
                members: members
                    .iter()
                    .map(|&t| b.season.teams[t].name.clone())
                    .collect(),
                member_slugs: members
                    .iter()
                    .map(|&t| slug(&b.season.teams[t].name))
                    .collect(),
                member_ratings: ratings,
                nonconference_games: nonconf,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.member_average.total_cmp(&a.member_average));
    rows
}

pub fn odds(b: &Built) -> Vec<OddsRow> {
    let mut rows: Vec<OddsRow> = b
        .order
        .iter()
        .filter_map(|&t| {
            Some(OddsRow {
                team: b.season.teams[t].name.clone(),
                slug: slug(&b.season.teams[t].name),
                conference: b.season.teams[t].conference.clone(),
                rank: b.rank_of(t)?,
                odds: b.odds_out(t)?,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b.odds
            .playoff
            .total_cmp(&a.odds.playoff)
            .then(a.rank.cmp(&b.rank))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Texas A&M"), "texas-a-m");
        assert_eq!(slug("Hawai'i"), "hawai-i");
        assert_eq!(slug("Miami (OH)"), "miami-oh");
        assert_eq!(slug("San José State"), "san-jose-state");
    }
}

//! Converts cached CFBD data into the model's `Season`.

use anyhow::Result;
use cfbd::{
    BettingGame, Cache, Dataset, GamePlayers, TeamGameAdvanced, TeamGameHavoc, TeamGamePpa,
    WeeklyDataset,
};
use model::data::{iso_day, Class, Game, Season, Team, Week, EFFICIENCY};
use std::collections::{BTreeMap, HashMap};

/// Conference labels that mean "no conference".
const INDEPENDENT: [&str; 3] = ["FBS Independents", "FCS Independents", "Independent DII"];

/// Pseudo-team name for a lower-division class.
fn pseudo_name(class: Class) -> &'static str {
    match class {
        Class::DivisionII => "Division II",
        Class::DivisionIII => "Division III",
        _ => "Other",
    }
}

#[derive(Default)]
struct TeamVotes {
    id: Option<i64>,
    class: BTreeMap<String, usize>,
    conference: BTreeMap<String, usize>,
}

fn majority(votes: &BTreeMap<String, usize>) -> Option<String> {
    // BTreeMap iteration makes ties resolve alphabetically, deterministically.
    votes
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, _)| k.clone())
}

pub fn load_season(cache: &Cache, year: i32) -> Result<Season> {
    let mut raw: BTreeMap<i64, cfbd::Game> = BTreeMap::new();
    for ds in [Dataset::GamesFbs, Dataset::GamesFcs] {
        for g in cache.load::<cfbd::Game>(year, ds)? {
            raw.insert(g.id, g);
        }
    }
    let raw: Vec<cfbd::Game> = raw
        .into_values()
        .filter(|g| g.season_type == "regular" || g.season_type == "postseason")
        .collect();

    // A team's class and conference are its most common labels this season.
    let mut votes: HashMap<String, TeamVotes> = HashMap::new();
    for g in &raw {
        for (name, id, class, conf) in [
            (
                &g.home_team,
                g.home_id,
                &g.home_classification,
                &g.home_conference,
            ),
            (
                &g.away_team,
                g.away_id,
                &g.away_classification,
                &g.away_conference,
            ),
        ] {
            let v = votes.entry(name.clone()).or_default();
            v.id = Some(id);
            *v.class
                .entry(class.clone().unwrap_or_default())
                .or_default() += 1;
            if let Some(c) = conf {
                *v.conference.entry(c.clone()).or_default() += 1;
            }
        }
    }

    let mut teams: Vec<Team> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut names: Vec<&String> = votes.keys().collect();
    names.sort();
    for name in names {
        let v = &votes[name];
        let class_label = majority(&v.class).filter(|c| !c.is_empty());
        let class = Class::from_cfbd(class_label.as_deref());
        let key = if class.is_rated() {
            name.clone()
        } else {
            pseudo_name(class).to_string()
        };
        if let Some(&i) = index.get(&key) {
            index.insert(name.clone(), i);
            continue;
        }
        let conference = if class.is_rated() {
            majority(&v.conference).filter(|c| !INDEPENDENT.contains(&c.as_str()))
        } else {
            None
        };
        teams.push(Team {
            name: key.clone(),
            class,
            conference,
            id: if class.is_rated() { v.id } else { None },
        });
        index.insert(key, teams.len() - 1);
        index.insert(name.clone(), teams.len() - 1);
    }

    let ppa = ppa_by_game(cache, year)?;
    let lines = market_by_game(&cache.load::<BettingGame>(year, Dataset::Lines)?);
    let efficiency = efficiency_by_game(cache, year)?;
    let starters = starters_by_game(cache, year)?;

    let mut games: Vec<Game> = Vec::new();
    for g in &raw {
        let (home, away) = (index[&g.home_team], index[&g.away_team]);
        if !teams[home].class.is_rated() && !teams[away].class.is_rated() {
            continue;
        }
        let Some(day) = iso_day(&g.start_date) else {
            anyhow::bail!("game {}: unparseable start date {:?}", g.id, g.start_date);
        };
        let score = g.score().map(|(h, a)| (h as f64, a as f64));
        let ppa = ppa
            .get(&g.id)
            .and_then(|m| Some((*m.get(&g.home_team)?, *m.get(&g.away_team)?)));
        let (market_margin, market_total) = lines.get(&g.id).copied().unwrap_or((None, None));
        games.push(Game {
            id: g.id,
            week: Week {
                postseason: g.season_type == "postseason",
                number: g.week,
            },
            day,
            neutral: g.neutral_site,
            conference_game: g.conference_game,
            championship: g.conference_game
                && g.notes
                    .as_deref()
                    .is_some_and(|n| n.contains("Championship")),
            home,
            away,
            score,
            ppa,
            efficiency: efficiency
                .get(&g.id)
                .map(|m| {
                    let (h, a) = (m.get(&g.home_team), m.get(&g.away_team));
                    (0..EFFICIENCY.len())
                        .map(|i| Some((h?[i]?, a?[i]?)))
                        .collect()
                })
                .unwrap_or_default(),
            starters: starters.get(&g.id).map_or((None, None), |m| {
                (m.get(&g.home_team).cloned(), m.get(&g.away_team).cloned())
            }),
            market_margin,
            market_total,
            cfbd_elo: g
                .home_pregame_elo
                .zip(g.away_pregame_elo)
                .map(|(h, a)| (h as f64, a as f64)),
        });
    }
    games.sort_by_key(|g| (g.week, g.day, g.id));
    Ok(Season { year, teams, games })
}

type Readings = [Option<f64>; EFFICIENCY.len()];

/// `EFFICIENCY` readings by game and offense. Rates over fewer than 10
/// plays of their kind are dropped as too noisy.
fn efficiency_by_game(cache: &Cache, year: i32) -> Result<HashMap<i64, HashMap<String, Readings>>> {
    let mut out: HashMap<i64, HashMap<String, Readings>> = HashMap::new();
    for r in cache
        .load_optional::<TeamGameAdvanced>(year, Dataset::Advanced)?
        .unwrap_or_default()
    {
        let o = &r.offense;
        if o.plays.is_none_or(|n| n < 10.0) {
            continue;
        }
        let mut v: Readings = [None; EFFICIENCY.len()];
        v[0] = o.success_rate;
        v[1] = o.explosiveness;
        v[2] = o.rushing_plays.ppa;
        v[3] = o.passing_plays.ppa;
        v[4] = o.passing_downs.success_rate;
        v[5] = o.line_yards;
        v[6] = o.stuff_rate;
        out.entry(r.game_id).or_default().insert(r.team, v);
    }
    for r in cache
        .load_optional::<TeamGameHavoc>(year, Dataset::Havoc)?
        .unwrap_or_default()
    {
        if r.offense.total_plays.is_none_or(|n| n < 10.0) {
            continue;
        }
        out.entry(r.game_id)
            .or_default()
            .entry(r.team)
            .or_insert([None; EFFICIENCY.len()])[7] = r.offense.havoc_rate;
    }
    Ok(out)
}

/// Starting quarterback (most pass attempts) by game and team.
fn starters_by_game(cache: &Cache, year: i32) -> Result<HashMap<i64, HashMap<String, String>>> {
    let mut out: HashMap<i64, HashMap<String, String>> = HashMap::new();
    for ds in WeeklyDataset::ALL {
        for g in cache
            .load_weekly::<GamePlayers>(year, ds)?
            .unwrap_or_default()
        {
            let m = out.entry(g.id).or_default();
            for (team, id) in g.starting_passers() {
                m.entry(team).or_insert(id);
            }
        }
    }
    Ok(out)
}

/// Offensive PPA per play by game and team.
fn ppa_by_game(cache: &Cache, year: i32) -> Result<HashMap<i64, HashMap<String, f64>>> {
    let mut out: HashMap<i64, HashMap<String, f64>> = HashMap::new();
    for ds in [Dataset::PpaFbs, Dataset::PpaFcs] {
        for r in cache
            .load_optional::<TeamGamePpa>(year, ds)?
            .unwrap_or_default()
        {
            out.entry(r.game_id)
                .or_default()
                .insert(r.team, r.offense.overall);
        }
    }
    Ok(out)
}

/// Closing spread and total per game: the `consensus` line when present,
/// otherwise the mean over providers. Spreads are converted to predicted
/// home margin (CFBD spreads are negative when the home team is favored).
fn market_by_game(rows: &[BettingGame]) -> HashMap<i64, (Option<f64>, Option<f64>)> {
    rows.iter()
        .map(|b| {
            let pick = |f: fn(&cfbd::GameLine) -> Option<f64>| -> Option<f64> {
                if let Some(v) = b
                    .lines
                    .iter()
                    .find(|l| l.provider == "consensus")
                    .and_then(f)
                {
                    return Some(v);
                }
                let vals: Vec<f64> = b.lines.iter().filter_map(f).collect();
                (!vals.is_empty()).then(|| vals.iter().sum::<f64>() / vals.len() as f64)
            };
            (
                b.id,
                (pick(|l| l.spread).map(|s| -s), pick(|l| l.over_under)),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn majority_breaks_ties_alphabetically() {
        let mut v = BTreeMap::new();
        v.insert("SEC".to_string(), 3);
        v.insert("ACC".to_string(), 3);
        v.insert("Big Ten".to_string(), 1);
        assert_eq!(majority(&v).as_deref(), Some("ACC"));
    }

    fn line(provider: &str, spread: Option<f64>) -> cfbd::GameLine {
        serde_json::from_value(serde_json::json!({
            "provider": provider, "spread": spread, "overUnder": 50.0
        }))
        .unwrap()
    }

    fn betting(lines: Vec<cfbd::GameLine>) -> BettingGame {
        BettingGame {
            id: 1,
            season: 2024,
            week: 1,
            home_team: "A".into(),
            away_team: "B".into(),
            lines,
        }
    }

    #[test]
    fn market_prefers_consensus_and_flips_sign() {
        let m = market_by_game(&[betting(vec![
            line("x", Some(-10.0)),
            line("consensus", Some(-7.0)),
        ])]);
        assert_eq!(m[&1].0, Some(7.0), "home -7 means home favored by 7");
    }

    #[test]
    fn market_averages_without_consensus() {
        let m = market_by_game(&[betting(vec![
            line("x", Some(-6.0)),
            line("y", Some(-8.0)),
            line("z", None),
        ])]);
        assert_eq!(m[&1].0, Some(7.0));
    }
}

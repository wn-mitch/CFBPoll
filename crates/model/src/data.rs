//! Model-side season data, independent of the API's shapes.

use serde::{Deserialize, Serialize};

/// Competitive level. FBS and FCS teams are rated individually; lower
/// divisions are represented by one pseudo-team per class whose rating is
/// redrawn from its prior before every game (each opponent is a different
/// school).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Class {
    Fbs,
    Fcs,
    DivisionII,
    DivisionIII,
    /// NAIA and other unclassified opponents.
    Other,
}

impl Class {
    pub fn is_rated(self) -> bool {
        matches!(self, Class::Fbs | Class::Fcs)
    }

    /// Maps a CFBD classification string. `None` (unclassified) maps to `Other`.
    pub fn from_cfbd(s: Option<&str>) -> Class {
        match s {
            Some("fbs") => Class::Fbs,
            Some("fcs") => Class::Fcs,
            Some("ii") => Class::DivisionII,
            Some("iii") | Some("ii/iii") => Class::DivisionIII,
            _ => Class::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub class: Class,
    /// Conference this season. `None` for independents and pseudo-teams,
    /// which have no conference level in the model.
    pub conference: Option<String>,
    /// CFBD team id (used for logos); `None` for pseudo-teams.
    #[serde(default)]
    pub id: Option<i64>,
}

/// Ordinal position of a game in the season, used for weekly snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Week {
    pub postseason: bool,
    pub number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: i64,
    pub week: Week,
    /// Days since 1970-01-01 (UTC) of kickoff.
    pub day: i64,
    pub neutral: bool,
    pub conference_game: bool,
    /// A conference championship game.
    #[serde(default)]
    pub championship: bool,
    /// Index into `Season::teams`.
    pub home: usize,
    pub away: usize,
    /// `(home, away)` points when the game is complete.
    pub score: Option<(f64, f64)>,
    /// Garbage-time-excluded offensive PPA per play, `(home, away)`, when
    /// both sides are available.
    pub ppa: Option<(f64, f64)>,
    /// Other per-game offensive readings, `(home, away)`, indexed like
    /// `EFFICIENCY`; `None` where either side is missing. Empty when the
    /// season has no advanced stats.
    #[serde(default)]
    pub efficiency: Vec<Option<(f64, f64)>>,
    /// Athlete id of each side's starting quarterback (most pass attempts),
    /// `(home, away)`, when the box score has one.
    #[serde(default)]
    pub starters: (Option<String>, Option<String>),
    /// Closing spread as predicted home margin (positive favors home).
    pub market_margin: Option<f64>,
    pub market_total: Option<f64>,
    /// CFBD's pregame Elo, `(home, away)`, used as a baseline.
    pub cfbd_elo: Option<(f64, f64)>,
}

/// Names of the readings in `Game::efficiency`, all from the offense's side:
/// garbage-time-excluded success rate, explosiveness (PPA on successful
/// plays), rushing and passing PPA, passing-down success rate, line yards per
/// carry, stuff rate, and the havoc rate the offense allowed.
pub const EFFICIENCY: [&str; 8] = [
    "success_rate",
    "explosiveness",
    "rush_ppa",
    "pass_ppa",
    "passing_downs_success",
    "line_yards",
    "stuff_rate",
    "havoc",
];

impl Game {
    /// `(home, away)` value of the named offensive reading: `"ppa"` or one of
    /// `EFFICIENCY`.
    pub fn reading(&self, name: &str) -> Option<(f64, f64)> {
        if name == "ppa" {
            return self.ppa;
        }
        let i = EFFICIENCY.iter().position(|&n| n == name)?;
        self.efficiency.get(i).copied().flatten()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Season {
    pub year: i32,
    pub teams: Vec<Team>,
    /// Sorted by kickoff.
    pub games: Vec<Game>,
}

impl Season {
    pub fn team_index(&self, name: &str) -> Option<usize> {
        self.teams.iter().position(|t| t.name == name)
    }

    /// Distinct conference names among rated teams, sorted.
    pub fn conferences(&self) -> Vec<String> {
        let mut c: Vec<String> = self
            .teams
            .iter()
            .filter(|t| t.class.is_rated())
            .filter_map(|t| t.conference.clone())
            .collect();
        c.sort();
        c.dedup();
        c
    }
}

/// Parses the date part of an ISO-8601 timestamp (`2024-08-24T23:00:00.000Z`)
/// into days since 1970-01-01.
pub fn iso_day(s: &str) -> Option<i64> {
    let date = s.get(..10)?;
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_days() {
        assert_eq!(iso_day("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(iso_day("2000-02-29"), Some(11_016));
        assert_eq!(iso_day("2000-03-01"), Some(11_017));
        assert_eq!(iso_day("2024-13-01"), None);
        assert_eq!(iso_day("2024"), None);
    }

    #[test]
    fn postseason_sorts_after_regular_season() {
        let reg = Week {
            postseason: false,
            number: 15,
        };
        let post = Week {
            postseason: true,
            number: 1,
        };
        assert!(reg < post);
    }

    #[test]
    fn classification_mapping() {
        assert_eq!(Class::from_cfbd(Some("fbs")), Class::Fbs);
        assert_eq!(Class::from_cfbd(Some("ii/iii")), Class::DivisionIII);
        assert_eq!(Class::from_cfbd(None), Class::Other);
        assert!(!Class::DivisionII.is_rated());
    }
}

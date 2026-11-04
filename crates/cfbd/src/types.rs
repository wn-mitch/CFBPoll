//! Typed subsets of CollegeFootballData API v2 responses.
//!
//! Only fields the model uses are declared; serde ignores the rest. Field
//! names follow the API's camelCase JSON.

use serde::Deserialize;

/// `/games` row.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub season: i32,
    pub week: i32,
    pub season_type: String,
    pub start_date: String,
    pub completed: bool,
    pub neutral_site: bool,
    pub conference_game: bool,
    pub home_id: i64,
    pub home_team: String,
    pub home_conference: Option<String>,
    pub home_classification: Option<String>,
    pub home_points: Option<i32>,
    pub home_pregame_elo: Option<i32>,
    pub away_id: i64,
    pub away_team: String,
    pub away_conference: Option<String>,
    pub away_classification: Option<String>,
    pub away_points: Option<i32>,
    pub away_pregame_elo: Option<i32>,
    pub notes: Option<String>,
}

impl Game {
    /// Final score as `(home, away)`, present only for completed games.
    pub fn score(&self) -> Option<(i32, i32)> {
        if !self.completed {
            return None;
        }
        Some((self.home_points?, self.away_points?))
    }
}

/// `/ppa/games` row: one team's per-play predicted points added in one game.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamGamePpa {
    pub game_id: i64,
    pub season: i32,
    pub week: i32,
    pub team: String,
    pub opponent: String,
    pub offense: PpaSplit,
    pub defense: PpaSplit,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PpaSplit {
    pub overall: f64,
}

/// `/lines` row.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BettingGame {
    pub id: i64,
    pub season: i32,
    pub week: i32,
    pub home_team: String,
    pub away_team: String,
    pub lines: Vec<GameLine>,
}

/// A sportsbook line. `spread` is from the home team's perspective
/// (negative means the home team is favored).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLine {
    pub provider: String,
    pub spread: Option<f64>,
    pub over_under: Option<f64>,
}

/// `/rankings` row: every poll released for one week.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollWeek {
    pub season: i32,
    pub season_type: String,
    pub week: i32,
    pub polls: Vec<Poll>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Poll {
    pub poll: String,
    pub ranks: Vec<PollRank>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PollRank {
    pub rank: i32,
    pub school: String,
    pub conference: Option<String>,
}

/// `/player/returning` row. Shares are fractions in `[0, 1]` of the previous
/// season's production that returns.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturningProduction {
    pub season: i32,
    pub team: String,
    #[serde(rename = "percentPPA")]
    pub percent_ppa: f64,
    #[serde(rename = "percentPassingPPA")]
    pub percent_passing_ppa: f64,
    #[serde(rename = "percentReceivingPPA")]
    pub percent_receiving_ppa: f64,
    #[serde(rename = "percentRushingPPA")]
    pub percent_rushing_ppa: f64,
    pub usage: f64,
    pub passing_usage: f64,
    pub receiving_usage: f64,
    pub rushing_usage: f64,
}

/// `/player/portal` row.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTransfer {
    pub season: i32,
    pub position: String,
    pub origin: String,
    pub destination: Option<String>,
    pub rating: Option<f64>,
    pub stars: Option<i32>,
}

/// `/roster` row.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterPlayer {
    pub id: String,
    pub team: String,
    pub year: i32,
    pub position: Option<String>,
}

/// `/talent` row (247Sports team talent composite).
#[derive(Debug, Clone, Deserialize)]
pub struct TeamTalent {
    pub year: i32,
    pub team: String,
    pub talent: f64,
}

/// `/calendar` row.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarWeek {
    pub season: i32,
    pub week: i32,
    pub season_type: String,
    pub start_date: String,
    pub end_date: String,
}

/// `/info` response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub tier_name: String,
    pub monthly_limit: f64,
    pub remaining_calls: f64,
    pub used_calls: f64,
    pub reset_at: String,
}

//! JSON shapes the site reads. `site/src/lib/types.ts` mirrors these.
//!
//! Ratings are points relative to the average FBS team: `overall` is the
//! expected margin against that team on a neutral field.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Meta {
    pub year: i32,
    /// Label of the latest week with results, e.g. "Week 10".
    pub as_of: String,
    pub generated: String,
    pub games_played: usize,
    pub games_remaining: usize,
    pub uses_prior: bool,
    pub draws: usize,
    pub hfa: f64,
}

#[derive(Debug, Serialize)]
pub struct Record {
    pub wins: u32,
    pub losses: u32,
    pub conf_wins: u32,
    pub conf_losses: u32,
}

#[derive(Debug, Serialize)]
pub struct Rated {
    pub value: f64,
    pub sd: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameRef {
    pub opponent: String,
    pub opponent_slug: Option<String>,
    pub opponent_logo_id: Option<i64>,
    /// "home", "away" or "neutral".
    pub site: &'static str,
    pub points_for: f64,
    pub points_against: f64,
}

#[derive(Debug, Serialize)]
pub struct RankRow {
    pub rank: usize,
    pub rank_low: usize,
    pub rank_high: usize,
    pub previous_rank: Option<usize>,
    pub team: String,
    pub slug: String,
    /// CFBD team id; the site builds logo URLs from it.
    pub logo_id: Option<i64>,
    pub conference: Option<String>,
    pub record: Record,
    pub overall: Rated,
    pub offense: Rated,
    pub defense: Rated,
    /// Rank of the offense and defense ratings among FBS teams.
    pub offense_rank: usize,
    pub defense_rank: usize,
    /// Overall change since the previous week's snapshot.
    pub change: f64,
    pub last_game: Option<GameRef>,
    /// Average current overall rating of all opponents on the schedule.
    pub schedule_strength: f64,
    /// Expected losses for a 25th-ranked team playing this schedule.
    pub top25_expected_losses: f64,
    /// Rank and rating after each week, preseason first.
    pub history: Vec<WeekPoint>,
}

/// A team's standing after one week's games.
#[derive(Debug, Clone, Serialize)]
pub struct WeekPoint {
    pub label: String,
    pub rank: usize,
    /// 90% rank range at that week.
    pub rank_low: usize,
    pub rank_high: usize,
    pub overall: f64,
    /// Overall change since the previous week; `None` for preseason.
    pub change: Option<f64>,
    /// Games played that week, from this team's side.
    pub games: Vec<GameRef>,
}

#[derive(Debug, Serialize)]
pub struct TrajectoryPoint {
    pub label: String,
    pub overall: f64,
    pub overall_sd: f64,
    pub offense: f64,
    pub defense: f64,
}

#[derive(Debug, Serialize)]
pub struct ScheduleGame {
    pub id: i64,
    pub date: String,
    pub week_label: String,
    pub opponent: String,
    pub opponent_slug: Option<String>,
    pub opponent_rank: Option<usize>,
    pub opponent_logo_id: Option<i64>,
    pub site: &'static str,
    pub conference_game: bool,
    /// Final score from this team's side, when played.
    pub result: Option<(f64, f64)>,
    /// Pregame forecast (made before the game for played games).
    pub win_prob: f64,
    pub points_for: f64,
    pub points_against: f64,
    /// Closing market margin from this team's side.
    pub market_margin: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct TeamOddsOut {
    pub expected_wins: f64,
    pub expected_losses: f64,
    pub conf_title: f64,
    pub conf_title_game: f64,
    pub playoff: f64,
    pub bye: f64,
    pub semifinal: f64,
    pub title: f64,
}

#[derive(Debug, Serialize)]
pub struct TeamPage {
    pub row: RankRow,
    pub trajectory: Vec<TrajectoryPoint>,
    pub schedule: Vec<ScheduleGame>,
    pub odds: Option<TeamOddsOut>,
}

#[derive(Debug, Serialize)]
pub struct GamePrediction {
    pub id: i64,
    pub date: String,
    pub week_label: String,
    pub home: String,
    pub home_slug: Option<String>,
    pub home_rank: Option<usize>,
    pub home_logo_id: Option<i64>,
    pub away: String,
    pub away_slug: Option<String>,
    pub away_rank: Option<usize>,
    pub away_logo_id: Option<i64>,
    pub neutral: bool,
    pub home_win: f64,
    /// Expected final scores (floored at zero, so they can understate the
    /// margin in a mismatch).
    pub home_points: f64,
    pub away_points: f64,
    /// Expected home margin.
    pub margin: f64,
    pub margin_sd: f64,
    pub market_margin: Option<f64>,
    pub market_total: Option<f64>,
    pub result: Option<(f64, f64)>,
}

#[derive(Debug, Serialize)]
pub struct Games {
    pub upcoming_label: Option<String>,
    pub upcoming: Vec<GamePrediction>,
    pub recent_label: Option<String>,
    pub recent: Vec<GamePrediction>,
}

#[derive(Debug, Serialize)]
pub struct ConferenceRow {
    pub name: String,
    /// In-season conference level (offense + defense), relative to the FBS
    /// conference average.
    pub level: Rated,
    pub member_average: f64,
    pub members: Vec<String>,
    pub member_slugs: Vec<String>,
    pub member_ratings: Vec<f64>,
    pub member_logo_ids: Vec<Option<i64>>,
    pub nonconference_games: usize,
}

#[derive(Debug, Serialize)]
pub struct OddsRow {
    pub team: String,
    pub slug: String,
    pub logo_id: Option<i64>,
    pub conference: Option<String>,
    pub rank: usize,
    #[serde(flatten)]
    pub odds: TeamOddsOut,
}

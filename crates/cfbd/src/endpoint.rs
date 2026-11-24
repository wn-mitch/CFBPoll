//! The per-season datasets the poll needs, with their API paths, query
//! parameters, and cache file names.

/// One per-season dataset. Each maps to exactly one API call and one cache
/// file under `data/raw/<year>/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dataset {
    /// Games with at least one FBS team.
    GamesFbs,
    /// Games with at least one FCS team. Overlaps `GamesFbs` on FBS-FCS games.
    GamesFcs,
    /// Garbage-time-excluded per-game PPA for FBS teams.
    PpaFbs,
    /// Garbage-time-excluded per-game PPA for FCS teams.
    PpaFcs,
    Lines,
    Rankings,
    Returning,
    Portal,
    Roster,
    Talent,
    Calendar,
    /// Garbage-time-excluded per-game advanced team stats: success rate,
    /// explosiveness, rush and pass splits.
    Advanced,
    /// Per-game havoc rates (tackles for loss, passes defended, forced
    /// fumbles per play).
    Havoc,
    /// Head coaches with their seasons by school.
    Coaches,
}

impl Dataset {
    pub const ALL: [Dataset; 14] = [
        Dataset::GamesFbs,
        Dataset::GamesFcs,
        Dataset::PpaFbs,
        Dataset::PpaFcs,
        Dataset::Lines,
        Dataset::Rankings,
        Dataset::Returning,
        Dataset::Portal,
        Dataset::Roster,
        Dataset::Talent,
        Dataset::Calendar,
        Dataset::Advanced,
        Dataset::Havoc,
        Dataset::Coaches,
    ];

    /// Datasets that change while a season is in progress. The rest are
    /// fixed once the season starts (or are offseason data).
    pub fn changes_in_season(self) -> bool {
        matches!(
            self,
            Dataset::GamesFbs
                | Dataset::GamesFcs
                | Dataset::PpaFbs
                | Dataset::PpaFcs
                | Dataset::Lines
                | Dataset::Rankings
                | Dataset::Portal
                | Dataset::Roster
                | Dataset::Advanced
                | Dataset::Havoc
                | Dataset::Coaches
        )
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Dataset::GamesFbs => "games_fbs.json",
            Dataset::GamesFcs => "games_fcs.json",
            Dataset::PpaFbs => "ppa_fbs.json",
            Dataset::PpaFcs => "ppa_fcs.json",
            Dataset::Lines => "lines.json",
            Dataset::Rankings => "rankings.json",
            Dataset::Returning => "returning.json",
            Dataset::Portal => "portal.json",
            Dataset::Roster => "roster.json",
            Dataset::Talent => "talent.json",
            Dataset::Calendar => "calendar.json",
            Dataset::Advanced => "advanced.json",
            Dataset::Havoc => "havoc.json",
            Dataset::Coaches => "coaches.json",
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Dataset::GamesFbs | Dataset::GamesFcs => "/games",
            Dataset::PpaFbs | Dataset::PpaFcs => "/ppa/games",
            Dataset::Lines => "/lines",
            Dataset::Rankings => "/rankings",
            Dataset::Returning => "/player/returning",
            Dataset::Portal => "/player/portal",
            Dataset::Roster => "/roster",
            Dataset::Talent => "/talent",
            Dataset::Calendar => "/calendar",
            Dataset::Advanced => "/stats/game/advanced",
            Dataset::Havoc => "/stats/game/havoc",
            Dataset::Coaches => "/coaches",
        }
    }

    /// Query parameters for `year`, in a stable order.
    pub fn query(self, year: i32) -> Vec<(&'static str, String)> {
        let mut q = vec![("year", year.to_string())];
        let both = ("seasonType", "both".to_string());
        match self {
            Dataset::GamesFbs => q.extend([both, ("classification", "fbs".into())]),
            Dataset::GamesFcs => q.extend([both, ("classification", "fcs".into())]),
            Dataset::PpaFbs => q.extend([
                both,
                ("excludeGarbageTime", "true".into()),
                ("classification", "fbs".into()),
            ]),
            Dataset::PpaFcs => q.extend([
                both,
                ("excludeGarbageTime", "true".into()),
                ("classification", "fcs".into()),
            ]),
            Dataset::Lines | Dataset::Rankings | Dataset::Havoc => q.push(both),
            Dataset::Advanced => q.extend([both, ("excludeGarbageTime", "true".into())]),
            Dataset::Returning
            | Dataset::Portal
            | Dataset::Roster
            | Dataset::Talent
            | Dataset::Calendar
            | Dataset::Coaches => {}
        }
        q
    }
}

/// A dataset the API serves only one week at a time. Each week is cached in
/// its own file under `data/raw/<year>/<dir>/`, so a refresh refetches only
/// weeks that were incomplete when last fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeeklyDataset {
    /// Passing box scores for games with an FBS team; used to find each
    /// team's starting quarterback.
    PassingFbs,
    /// Passing box scores for games with an FCS team.
    PassingFcs,
}

impl WeeklyDataset {
    pub const ALL: [WeeklyDataset; 2] = [WeeklyDataset::PassingFbs, WeeklyDataset::PassingFcs];

    pub fn dir(self) -> &'static str {
        match self {
            WeeklyDataset::PassingFbs => "passing_fbs",
            WeeklyDataset::PassingFcs => "passing_fcs",
        }
    }

    pub fn path(self) -> &'static str {
        "/games/players"
    }

    pub fn query(self, year: i32, season_type: &str, week: i32) -> Vec<(&'static str, String)> {
        let class = match self {
            WeeklyDataset::PassingFbs => "fbs",
            WeeklyDataset::PassingFcs => "fcs",
        };
        vec![
            ("year", year.to_string()),
            ("week", week.to_string()),
            ("seasonType", season_type.to_string()),
            ("category", "passing".into()),
            ("classification", class.into()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cache_file_names_are_unique() {
        let names: HashSet<_> = Dataset::ALL.iter().map(|d| d.file_name()).collect();
        assert_eq!(names.len(), Dataset::ALL.len());
    }

    #[test]
    fn ppa_excludes_garbage_time() {
        for d in [Dataset::PpaFbs, Dataset::PpaFcs] {
            assert!(d
                .query(2024)
                .contains(&("excludeGarbageTime", "true".into())));
        }
    }

    #[test]
    fn advanced_stats_exclude_garbage_time() {
        assert!(Dataset::Advanced
            .query(2024)
            .contains(&("excludeGarbageTime", "true".into())));
    }

    #[test]
    fn game_queries_include_postseason() {
        for d in [
            Dataset::GamesFbs,
            Dataset::GamesFcs,
            Dataset::Lines,
            Dataset::Advanced,
            Dataset::Havoc,
        ] {
            assert!(d.query(2024).contains(&("seasonType", "both".into())));
        }
    }
}

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
}

impl Dataset {
    pub const ALL: [Dataset; 11] = [
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
            Dataset::Lines | Dataset::Rankings => q.push(both),
            Dataset::Returning
            | Dataset::Portal
            | Dataset::Roster
            | Dataset::Talent
            | Dataset::Calendar => {}
        }
        q
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
    fn game_queries_include_postseason() {
        for d in [Dataset::GamesFbs, Dataset::GamesFcs, Dataset::Lines] {
            assert!(d.query(2024).contains(&("seasonType", "both".into())));
        }
    }
}

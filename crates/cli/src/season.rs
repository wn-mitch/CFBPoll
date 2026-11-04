//! Calendar helpers for picking the current season without a date crate.

use std::time::{SystemTime, UNIX_EPOCH};

/// Civil (year, month, day) in UTC for a Unix day number.
/// Howard Hinnant's `civil_from_days`.
pub fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// Today's UTC date.
pub fn today() -> (i32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86_400))
}

/// The season a date belongs to. Seasons start in late August and end with
/// the title game in January, so January through July belong to the
/// previous year's season.
pub fn season_for(year: i32, month: u32) -> i32 {
    if month >= 8 {
        year
    } else {
        year - 1
    }
}

pub fn current_season() -> i32 {
    let (y, m, _) = today();
    season_for(y, m)
}

/// A list of seasons parsed from one CLI argument.
#[derive(Debug, Clone)]
pub struct Years(pub Vec<i32>);

/// Parses `2024` or `2010-2026` (inclusive).
pub fn parse_years(s: &str) -> Result<Years, String> {
    parse_year_list(s).map(Years)
}

fn parse_year_list(s: &str) -> Result<Vec<i32>, String> {
    let parse = |p: &str| {
        p.trim()
            .parse::<i32>()
            .map_err(|e| format!("bad year {p:?}: {e}"))
    };
    match s.split_once('-') {
        Some((a, b)) => {
            let (a, b) = (parse(a)?, parse(b)?);
            if a > b {
                return Err(format!("empty range {s}"));
            }
            Ok((a..=b).collect())
        }
        None => Ok(vec![parse(s)?]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2000-02-29 (leap day) is day 11_016.
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn january_belongs_to_previous_season() {
        assert_eq!(season_for(2026, 1), 2025);
        assert_eq!(season_for(2026, 7), 2025);
        assert_eq!(season_for(2026, 8), 2026);
    }

    #[test]
    fn year_ranges() {
        assert_eq!(parse_year_list("2024").unwrap(), vec![2024]);
        assert_eq!(
            parse_year_list("2010-2012").unwrap(),
            vec![2010, 2011, 2012]
        );
        assert!(parse_years("2012-2010").is_err());
        assert!(parse_years("abc").is_err());
    }
}

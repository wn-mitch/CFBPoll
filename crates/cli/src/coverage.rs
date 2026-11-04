//! `cfbpoll coverage`: row counts per cached dataset and season, to see which
//! model inputs exist for which years.

use anyhow::Result;
use cfbd::{Cache, Dataset};
use serde::de::DeserializeOwned;

pub fn run(cache: &Cache, years: &[i32]) -> Result<()> {
    let names: Vec<_> = Dataset::ALL
        .iter()
        .map(|d| d.file_name().trim_end_matches(".json"))
        .collect();
    println!(
        "year  {}",
        names.iter().map(|n| format!("{n:>11}")).collect::<String>()
    );
    for &year in years {
        let mut row = format!("{year}  ");
        for &ds in &Dataset::ALL {
            let cell = match typed_rows(cache, year, ds)? {
                None => "-".to_string(),
                Some(n) => n.to_string(),
            };
            row.push_str(&format!("{cell:>11}"));
        }
        println!("{row}");
    }
    Ok(())
}

/// Row count after parsing into the typed struct the model uses, so a schema
/// mismatch fails here rather than mid-backtest.
fn typed_rows(cache: &Cache, year: i32, ds: Dataset) -> Result<Option<usize>> {
    fn n<T: DeserializeOwned>(c: &Cache, y: i32, d: Dataset) -> Result<Option<usize>> {
        Ok(c.load_optional::<T>(y, d)?.map(|v| v.len()))
    }
    match ds {
        Dataset::GamesFbs | Dataset::GamesFcs => n::<cfbd::Game>(cache, year, ds),
        Dataset::PpaFbs | Dataset::PpaFcs => n::<cfbd::TeamGamePpa>(cache, year, ds),
        Dataset::Lines => n::<cfbd::BettingGame>(cache, year, ds),
        Dataset::Rankings => n::<cfbd::PollWeek>(cache, year, ds),
        Dataset::Returning => n::<cfbd::ReturningProduction>(cache, year, ds),
        Dataset::Portal => n::<cfbd::PlayerTransfer>(cache, year, ds),
        Dataset::Roster => n::<cfbd::RosterPlayer>(cache, year, ds),
        Dataset::Talent => n::<cfbd::TeamTalent>(cache, year, ds),
        Dataset::Calendar => n::<cfbd::CalendarWeek>(cache, year, ds),
    }
}

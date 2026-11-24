//! `cfbpoll fetch`: fill the raw-data cache from the CFBD API.

use crate::season::current_season;
use anyhow::Result;
use cfbd::{Cache, Client, Dataset, Refresh, WeeklyDataset};
use std::time::Duration;

/// Completed seasons are fetched once; the current season's in-season
/// datasets are refetched when older than `stale_hours`. `force` refetches
/// everything requested.
pub fn run(cache: &Cache, years: &[i32], stale_hours: u64, force: bool) -> Result<()> {
    let client = Client::from_env(std::path::Path::new(".env"))?;
    let current = current_season();
    let mut calls = 0;
    for &year in years {
        let refresh = if force {
            Refresh::All
        } else if year >= current {
            Refresh::OlderThan(Duration::from_secs(stale_hours * 3600))
        } else {
            Refresh::Missing
        };
        let fetched = client.sync(cache, year, &Dataset::ALL, refresh)?;
        calls += fetched.len();
        let mut names: Vec<String> = fetched.iter().map(|d| d.file_name().to_string()).collect();
        for ds in WeeklyDataset::ALL {
            let n = client.sync_weekly(cache, year, ds, refresh)?;
            calls += n;
            if n > 0 {
                names.push(format!("{} ({n} weeks)", ds.dir()));
            }
        }
        if !names.is_empty() {
            eprintln!("{year}: fetched {}", names.join(", "));
        }
    }
    eprintln!("{calls} API calls");
    match client.info() {
        Ok(info) => eprintln!(
            "quota ({}): {} of {} calls left, resets {}",
            info.tier_name, info.remaining_calls, info.monthly_limit, info.reset_at
        ),
        Err(e) => eprintln!("could not read quota: {e:#}"),
    }
    Ok(())
}

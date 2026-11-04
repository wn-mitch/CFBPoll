mod coverage;
mod fetch;
mod season;

use anyhow::Result;
use cfbd::Cache;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cfbpoll", about = "College football ratings and poll")]
struct Cli {
    /// Raw-data cache directory.
    #[arg(long, global = true, default_value = "data/raw")]
    cache: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fetch raw data from the CFBD API into the cache.
    Fetch {
        /// A season (2024) or inclusive range (2010-2026). Defaults to the current season.
        #[arg(long, value_parser = season::parse_years)]
        years: Option<season::Years>,
        /// Refetch current-season data older than this many hours.
        #[arg(long, default_value_t = 6)]
        stale_hours: u64,
        /// Refetch everything requested, even completed seasons.
        #[arg(long)]
        force: bool,
    },
    /// Print cached row counts per dataset and season.
    Coverage {
        #[arg(long, value_parser = season::parse_years)]
        years: season::Years,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cache = Cache::new(cli.cache);
    match cli.cmd {
        Cmd::Fetch {
            years,
            stale_hours,
            force,
        } => {
            let years = years
                .map(|y| y.0)
                .unwrap_or_else(|| vec![season::current_season()]);
            fetch::run(&cache, &years, stale_hours, force)
        }
        Cmd::Coverage { years } => coverage::run(&cache, &years.0),
    }
}

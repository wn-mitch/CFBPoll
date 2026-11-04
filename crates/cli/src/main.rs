mod backtest;
mod coverage;
mod fetch;
mod load;
mod rank;
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
    /// Score out-of-sample forecasts against baselines.
    Backtest {
        #[arg(long, value_parser = season::parse_years)]
        years: season::Years,
        /// Model parameters (JSON). Defaults to built-in starting values.
        #[arg(long)]
        params: Option<PathBuf>,
        /// Print combined summaries as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print the FBS ranking after the latest completed game.
    Rank {
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        params: Option<PathBuf>,
        #[arg(long, default_value_t = 25)]
        top: usize,
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
        Cmd::Backtest {
            years,
            params,
            json,
        } => {
            let params = load_params(params.as_deref())?;
            backtest::run(&cache, &years.0, &params, json)
        }
        Cmd::Rank { year, params, top } => {
            let params = load_params(params.as_deref())?;
            rank::run(
                &cache,
                year.unwrap_or_else(season::current_season),
                &params,
                top,
            )
        }
        Cmd::Coverage { years } => coverage::run(&cache, &years.0),
    }
}

fn load_params(path: Option<&std::path::Path>) -> Result<model::params::Params> {
    use anyhow::Context;
    match path {
        None => Ok(model::params::Params::default()),
        Some(p) => {
            let text =
                std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
            serde_json::from_str(&text).with_context(|| format!("parsing {}", p.display()))
        }
    }
}

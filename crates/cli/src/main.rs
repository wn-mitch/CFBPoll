mod backtest;
mod coverage;
mod features;
mod fetch;
mod fit;
mod load;
mod odds;
mod prior;
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
        /// Roster-churn prior model (JSON) to also score with priors.
        #[arg(long)]
        prior: Option<PathBuf>,
        /// Print combined summaries as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Fit hyperparameters on training seasons.
    Fit {
        #[arg(long, value_parser = season::parse_years)]
        years: season::Years,
        /// Starting parameters (JSON); defaults to built-in values.
        #[arg(long)]
        params: Option<PathBuf>,
        /// Roster-churn prior model (JSON); fits with priors when given.
        #[arg(long)]
        prior: Option<PathBuf>,
        /// Fit without efficiency (PPA) observations.
        #[arg(long)]
        score_only: bool,
        #[arg(long, default_value = "params.json")]
        out: PathBuf,
        #[arg(long, default_value_t = 1500)]
        max_evals: usize,
    },
    /// Fit the roster-churn season prior and report ablations.
    Prior {
        #[arg(long, value_parser = season::parse_years)]
        years: season::Years,
        #[arg(long)]
        params: Option<PathBuf>,
        /// Ridge penalty; chosen by cross-validation when omitted.
        #[arg(long)]
        lambda: Option<f64>,
        #[arg(long, default_value = "prior.json")]
        out: PathBuf,
    },
    /// Simulate the rest of a season and print playoff odds.
    Odds {
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        params: Option<PathBuf>,
        #[arg(long, default_value_t = 10_000)]
        draws: usize,
        #[arg(long, default_value_t = 25)]
        top: usize,
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
            prior,
            json,
        } => {
            let params = load_params(params.as_deref())?;
            backtest::run(&cache, &years.0, &params, prior.as_deref(), json)
        }
        Cmd::Fit {
            years,
            params,
            prior,
            score_only,
            out,
            max_evals,
        } => {
            let mut params = load_params(params.as_deref())?;
            if score_only {
                params.use_efficiency = false;
            }
            fit::run(&cache, &years.0, &params, prior.as_deref(), &out, max_evals)
        }
        Cmd::Prior {
            years,
            params,
            lambda,
            out,
        } => {
            let params = load_params(params.as_deref())?;
            prior::run(&cache, &years.0, &params, lambda, &out)
        }
        Cmd::Odds {
            year,
            params,
            draws,
            top,
        } => {
            let params = load_params(params.as_deref())?;
            odds::run(
                &cache,
                year.unwrap_or_else(season::current_season),
                &params,
                draws,
                top,
            )
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

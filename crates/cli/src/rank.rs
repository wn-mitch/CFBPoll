//! `cfbpoll rank`: print the current FBS ranking for a season.

use crate::load::load_season;
use anyhow::Result;
use cfbd::Cache;
use model::data::Class;
use model::params::Params;
use model::season::{run_season, RunOptions};

pub fn run(cache: &Cache, year: i32, params: &Params, top: usize) -> Result<()> {
    let season = load_season(cache, year)?;
    let run = run_season(&season, params, &RunOptions::default());
    let ratings = run.final_ratings();
    let fbs: Vec<usize> = (0..season.teams.len())
        .filter(|&t| season.teams[t].class == Class::Fbs)
        .collect();
    // Report relative to the average FBS team.
    let avg = fbs.iter().map(|&t| ratings[t].overall).sum::<f64>() / fbs.len() as f64;
    let mut order = fbs.clone();
    order.sort_by(|&a, &b| ratings[b].overall.total_cmp(&ratings[a].overall));
    println!(
        "{:>4} {:<24} {:<18} {:>7} {:>6} {:>7} {:>7}",
        "rank", "team", "conference", "overall", "±sd", "off", "def"
    );
    for (i, &t) in order.iter().take(top).enumerate() {
        let r = ratings[t];
        let team = &season.teams[t];
        println!(
            "{:>4} {:<24} {:<18} {:>7.1} {:>6.1} {:>7.1} {:>7.1}",
            i + 1,
            team.name,
            team.conference.as_deref().unwrap_or("Independent"),
            r.overall - avg,
            r.overall_sd,
            r.offense,
            r.defense
        );
    }
    Ok(())
}

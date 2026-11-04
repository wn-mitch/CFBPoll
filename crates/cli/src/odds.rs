//! `cfbpoll odds`: simulate the rest of a season and print title odds.

use crate::load::load_season;
use anyhow::Result;
use cfbd::Cache;
use model::data::Class;
use model::params::Params;
use model::playoff::PlayoffRules;
use model::season::{run_season, RunOptions};
use model::sim::{simulate, SimOptions};

pub fn run(cache: &Cache, year: i32, params: &Params, draws: usize, top: usize) -> Result<()> {
    let season = load_season(cache, year)?;
    let run = run_season(&season, params, &RunOptions::default());
    let res = simulate(
        &season,
        &run,
        params,
        &PlayoffRules::cfp_2026(),
        SimOptions {
            draws,
            ..SimOptions::default()
        },
    );
    let mut rows: Vec<(usize, &model::sim::TeamOdds)> = res
        .teams
        .iter()
        .enumerate()
        .filter(|(t, _)| season.teams[*t].class == Class::Fbs)
        .filter_map(|(t, o)| o.as_ref().map(|o| (t, o)))
        .collect();
    rows.sort_by(|a, b| b.1.playoff.total_cmp(&a.1.playoff));
    println!(
        "{:<22} {:<18} {:>5} {:>5} {:>7} {:>7} {:>6} {:>6} {:>10}",
        "team", "conference", "W", "L", "conf", "playoff", "bye", "title", "rank 5-95"
    );
    for (t, o) in rows.into_iter().take(top) {
        let team = &season.teams[t];
        println!(
            "{:<22} {:<18} {:>5.1} {:>5.1} {:>6.1}% {:>6.1}% {:>5.1}% {:>5.1}% {:>4}-{:<4}",
            team.name,
            team.conference.as_deref().unwrap_or("Independent"),
            o.expected_wins,
            o.expected_losses,
            100.0 * o.conf_title,
            100.0 * o.playoff,
            100.0 * o.bye,
            100.0 * o.title,
            o.rank_p05,
            o.rank_p95
        );
    }
    Ok(())
}

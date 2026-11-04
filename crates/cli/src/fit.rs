//! `cfbpoll fit`: tune hyperparameters on training seasons.

use crate::load::load_season;
use crate::prior::{load_model, loso_priors, union_years, PriorData};
use anyhow::{Context, Result};
use cfbd::Cache;
use model::data::Season;
use model::fit::{from_vector, knobs, nelder_mead, objective, to_vector, Knob};
use model::layout::TeamPrior;
use model::params::Params;
use std::path::Path;

/// Knobs that affect the objective under these settings: efficiency
/// parameters only when efficiency is used, the prior sd scale only when
/// priors are supplied.
fn active_knobs(params: &Params, with_priors: bool) -> Vec<Knob> {
    knobs()
        .into_iter()
        .filter(|k| params.use_efficiency || !k.name.starts_with("ppa_"))
        .filter(|k| with_priors || k.name != "prior_sd_scale")
        .collect()
}

pub fn run(
    cache: &Cache,
    years: &[i32],
    start: &Params,
    prior: Option<&Path>,
    out: &Path,
    max_evals: usize,
) -> Result<()> {
    let seasons: Vec<Season> = years
        .iter()
        .map(|&y| load_season(cache, y))
        .collect::<Result<_>>()?;
    // Priors are computed once from the starting parameters' fresh-start
    // ratings and held fixed during the search.
    let priors: Vec<Vec<Option<TeamPrior>>> = match prior {
        None => Vec::new(),
        Some(path) => {
            let template = load_model(path)?;
            let data = PriorData::load(cache, &union_years(years, &template.train_years), start)?;
            loso_priors(&data, &template, years)
        }
    };
    let knobs = active_knobs(start, prior.is_some());
    let x0 = to_vector(&knobs, start);
    let step: Vec<f64> = knobs
        .iter()
        .map(|k| match k.name {
            "ppa_mean" => 0.05,
            "score_corr" | "ppa_score_corr" => 0.2,
            _ if k.positive => 0.3,
            _ => 2.0,
        })
        .collect();
    let base = start.clone();
    let start_value = objective(&seasons, start, &priors);
    eprintln!("start objective {start_value:.5} ({} knobs)", knobs.len());
    let (x, v) = nelder_mead(
        |x| objective(&seasons, &from_vector(&knobs, &base, x), &priors),
        &x0,
        &step,
        max_evals,
        1e-6,
        |evals, best| {
            if evals % 50 < 3 {
                eprintln!("  {evals:>5} evals  best {best:.5}");
            }
        },
    );
    let fitted = from_vector(&knobs, &base, &x);
    eprintln!("fitted objective {v:.5} (start {start_value:.5})");
    for k in &knobs {
        eprintln!(
            "  {:<20} {:>9.4} -> {:>9.4}",
            k.name,
            (k.get)(start),
            (k.get)(&fitted)
        );
    }
    let json = serde_json::to_string_pretty(&fitted)?;
    std::fs::write(out, json + "\n").with_context(|| format!("writing {}", out.display()))?;
    eprintln!("wrote {}", out.display());
    Ok(())
}

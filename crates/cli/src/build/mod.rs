//! `cfbpoll build`: write the site's JSON data for one season.

mod history;
mod schema;
mod teams;

use crate::features::{fresh_finals, Offseason};
use crate::load::load_season;
use crate::prior::load_model;
use anyhow::{Context, Result};
use cfbd::Cache;
use model::data::{Class, Game, Season};
use model::kalman::Gaussian;
use model::layout::{drift_variances, TeamPrior};
use model::observe::{predict, Prediction};
use model::params::Params;
use model::playoff::PlayoffRules;
use model::season::{run_season, RunOptions, SeasonRun};
use model::sim::{simulate, SimOptions, SimResult};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Inputs to a build besides the cache.
pub struct BuildInputs {
    pub year: i32,
    pub params: Params,
    pub prior: Option<PathBuf>,
    pub artifacts: PathBuf,
    pub out: PathBuf,
    pub draws: usize,
}

/// Everything derived from one season run, shared by the output writers.
pub struct Built<'a> {
    pub season: &'a Season,
    pub params: &'a Params,
    pub run: SeasonRun,
    pub sim: SimResult,
    /// FBS teams best-first by current overall rating.
    pub order: Vec<usize>,
    /// FBS average overall, offense and defense at the latest snapshot.
    pub avg: (f64, f64, f64),
    /// Week-by-week standing of each FBS team.
    pub history: std::collections::HashMap<usize, Vec<schema::WeekPoint>>,
}

impl Built<'_> {
    pub fn rank_of(&self, t: usize) -> Option<usize> {
        self.order.iter().position(|&x| x == t).map(|i| i + 1)
    }

    /// Prediction for a game not yet played, with rating drift added for
    /// the weeks between the latest result and kickoff.
    pub fn predict_future(&self, game: &Game) -> Prediction {
        let weeks = self
            .run
            .last_day
            .map_or(0.0, |d| (game.day - d).max(0) as f64 / 7.0);
        let mut state: Gaussian = self.run.state.clone();
        if weeks > 0.0 {
            let v: Vec<f64> = drift_variances(self.season, &self.run.layout, self.params)
                .into_iter()
                .map(|x| x * weeks)
                .collect();
            state.add_diagonal(&v);
        }
        predict(&state, &self.run.layout, self.params, game)
    }
}

fn write_json<T: Serialize>(dir: &Path, name: &str, value: &T) -> Result<()> {
    let path = dir.join(name);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).with_context(|| format!("creating {}", p.display()))?;
    }
    let text = serde_json::to_string(value)?;
    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
}

/// Season priors from the roster-churn model, or none when it is absent.
fn season_priors(
    cache: &Cache,
    season: &Season,
    inputs: &BuildInputs,
) -> Result<Vec<Option<TeamPrior>>> {
    let Some(path) = &inputs.prior else {
        return Ok(Vec::new());
    };
    let model = load_model(path)?;
    let prev = match load_season(cache, season.year - 1) {
        Ok(s) => Some(fresh_finals(&s, &inputs.params)),
        Err(_) => None,
    };
    let off = Offseason::load(cache, season.year)?;
    Ok(
        crate::features::season_features(season, &off, prev.as_ref())
            .into_iter()
            .map(|f| f.map(|f| model.predict(&f)))
            .collect(),
    )
}

pub fn run(cache: &Cache, inputs: &BuildInputs) -> Result<()> {
    let season = load_season(cache, inputs.year)?;
    let priors = season_priors(cache, &season, inputs)?;
    let run = run_season(
        &season,
        &inputs.params,
        &RunOptions {
            priors: &priors,
            before: None,
        },
    );
    let sim = simulate(
        &season,
        &run,
        &inputs.params,
        &PlayoffRules::cfp_2026(),
        SimOptions {
            draws: inputs.draws,
            ..SimOptions::default()
        },
    );
    let ratings = run.final_ratings();
    let fbs: Vec<usize> = (0..season.teams.len())
        .filter(|&t| season.teams[t].class == Class::Fbs)
        .collect();
    let n = fbs.len().max(1) as f64;
    let avg = (
        fbs.iter().map(|&t| ratings[t].overall).sum::<f64>() / n,
        fbs.iter().map(|&t| ratings[t].offense).sum::<f64>() / n,
        fbs.iter().map(|&t| ratings[t].defense).sum::<f64>() / n,
    );
    let mut order = fbs.clone();
    order.sort_by(|&a, &b| ratings[b].overall.total_cmp(&ratings[a].overall));
    let mut built = Built {
        season: &season,
        params: &inputs.params,
        run,
        sim,
        order,
        avg,
        history: Default::default(),
    };
    built.history = history::by_team(&built);

    let out = &inputs.out;
    let mut slugs: Vec<String> = built
        .order
        .iter()
        .map(|&t| teams::slug(&season.teams[t].name))
        .collect();
    slugs.sort();
    if let Some(w) = slugs.windows(2).find(|w| w[0] == w[1]) {
        anyhow::bail!("two teams share the page slug {:?}", w[0]);
    }
    write_json(
        out,
        "meta.json",
        &teams::meta(&built, inputs.prior.is_some()),
    )?;
    write_json(out, "rankings.json", &teams::rankings(&built))?;
    for (slug, page) in teams::team_pages(&built) {
        write_json(out, &format!("teams/{slug}.json"), &page)?;
    }
    write_json(out, "games.json", &teams::games(&built))?;
    write_json(out, "conferences.json", &teams::conferences(&built))?;
    write_json(out, "odds.json", &teams::odds(&built))?;
    if let Some(path) = &inputs.prior {
        write_json(out, "prior.json", &prior_summary(&load_model(path)?))?;
    }
    for name in ["accuracy.json", "bias.json"] {
        let src = inputs.artifacts.join(name);
        if src.exists() {
            std::fs::copy(&src, out.join(name))
                .with_context(|| format!("copying {}", src.display()))?;
        }
    }
    eprintln!("wrote site data for {} to {}", inputs.year, out.display());
    Ok(())
}

#[derive(Serialize)]
struct GroupEffect {
    group: &'static str,
    offense: f64,
    defense: f64,
}

#[derive(Serialize)]
struct PriorSummary {
    train_years: Vec<i32>,
    blocks: model::prior::Blocks,
    talent_kept: bool,
    sd_offense: f64,
    sd_defense: f64,
    continuity_effects: Vec<GroupEffect>,
    cv: Vec<model::prior::CvRow>,
}

fn prior_summary(m: &model::prior::PriorModel) -> PriorSummary {
    PriorSummary {
        train_years: m.train_years.clone(),
        blocks: m.blocks,
        talent_kept: m.blocks.talent,
        sd_offense: m.sd_o,
        sd_defense: m.sd_d,
        continuity_effects: model::prior::GROUPS
            .iter()
            .enumerate()
            .map(|(g, &group)| {
                let (offense, defense) = m.continuity_effect(g);
                GroupEffect {
                    group,
                    offense,
                    defense,
                }
            })
            .collect(),
        cv: m.report.clone(),
    }
}

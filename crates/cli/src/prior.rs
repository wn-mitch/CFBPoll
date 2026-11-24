//! `cfbpoll prior`: fit the roster-churn prior and report its
//! cross-validated accuracy and the feature-block ablations.

use crate::features::{examples, fresh_finals_all, season_features, Finals, Offseason};
use crate::load::load_season;
use anyhow::{Context, Result};
use cfbd::Cache;
use model::data::Season;
use model::layout::TeamPrior;
use model::params::Params;
use model::prior::{cross_validate, Blocks, CvRow, Example, PriorModel, GROUPS};
use std::collections::HashMap;
use std::path::Path;

/// Everything needed to produce priors for any season.
pub struct PriorData {
    pub seasons: Vec<Season>,
    pub finals: HashMap<i32, Finals>,
    pub offseason: HashMap<i32, Offseason>,
}

impl PriorData {
    /// Loads `years` plus the season before the first (for its finals).
    pub fn load(cache: &Cache, years: &[i32], params: &Params) -> Result<PriorData> {
        let first = years[0] - 1;
        let mut all: Vec<i32> = vec![first];
        all.extend_from_slice(years);
        let seasons: Vec<Season> = all
            .iter()
            .filter(|&&y| y != first || cache.path(y, cfbd::Dataset::GamesFbs).exists())
            .map(|&y| load_season(cache, y))
            .collect::<Result<_>>()?;
        let finals = fresh_finals_all(&seasons, params);
        let offseason = years
            .iter()
            .map(|&y| Ok((y, Offseason::load(cache, y)?)))
            .collect::<Result<_>>()?;
        Ok(PriorData {
            seasons,
            finals,
            offseason,
        })
    }

    pub fn season(&self, year: i32) -> Option<&Season> {
        self.seasons.iter().find(|s| s.year == year)
    }

    /// Training examples for every loaded season that has a predecessor.
    pub fn examples(&self, years: &[i32]) -> Vec<Example> {
        years
            .iter()
            .filter_map(|&y| {
                let s = self.season(y)?;
                Some(examples(
                    s,
                    &self.offseason[&y],
                    self.finals.get(&(y - 1)),
                    &self.finals[&y],
                ))
            })
            .flatten()
            .collect()
    }

    /// Per-team priors for `year` from `model`.
    pub fn priors(&self, year: i32, model: &PriorModel) -> Vec<Option<TeamPrior>> {
        let Some(s) = self.season(year) else {
            return Vec::new();
        };
        season_features(s, &self.offseason[&year], self.finals.get(&(year - 1)))
            .into_iter()
            .map(|f| f.map(|f| model.predict(&f)))
            .collect()
    }
}

fn rmse(ex: &[Example], preds: &[(usize, (f64, f64))]) -> (f64, f64) {
    let n = preds.len().max(1) as f64;
    let so = preds
        .iter()
        .map(|(i, p)| (p.0 - ex[*i].target.0).powi(2))
        .sum::<f64>();
    let sd = preds
        .iter()
        .map(|(i, p)| (p.1 - ex[*i].target.1).powi(2))
        .sum::<f64>();
    ((so / n).sqrt(), (sd / n).sqrt())
}

const NONE: Blocks = Blocks {
    returning: false,
    continuity: false,
    portal: false,
    talent: false,
    coaching: false,
};

pub fn run(
    cache: &Cache,
    years: &[i32],
    params: &Params,
    lambda: Option<f64>,
    out: &Path,
) -> Result<()> {
    let data = PriorData::load(cache, years, params)?;
    let ex = data.examples(years);
    eprintln!("{} team-seasons", ex.len());

    // Class mean only: what a fresh start assumes.
    let class_mean: Vec<(usize, (f64, f64))> = cross_validate(
        &ex.iter()
            .map(|e| {
                let mut e = e.clone();
                e.features.prev = None;
                e
            })
            .collect::<Vec<_>>(),
        NONE,
        1e6,
    );
    let lambdas = [1.0, 3.0, 10.0, 30.0, 100.0, 300.0];
    let best_lambda = |blocks: Blocks| -> (f64, (f64, f64)) {
        let pick = |l: f64| (l, rmse(&ex, &cross_validate(&ex, blocks, l)));
        match lambda {
            Some(l) => pick(l),
            None => lambdas
                .iter()
                .map(|&l| pick(l))
                .min_by(|a, b| (a.1 .0 + a.1 .1).total_cmp(&(b.1 .0 + b.1 .1)))
                .expect("non-empty grid"),
        }
    };
    println!(
        "{:<34} {:>7} {:>8} {:>8}",
        "features", "lambda", "rmse_o", "rmse_d"
    );
    let (o, d) = rmse(&ex, &class_mean);
    println!(
        "{:<34} {:>7} {:>8.3} {:>8.3}",
        "fresh start (class mean)", "-", o, d
    );
    let mut report = vec![CvRow {
        name: "Division average (fresh start)".into(),
        rmse_o: o,
        rmse_d: d,
    }];
    let variants = [
        ("Last season's rating", NONE),
        (
            "+ returning production",
            Blocks {
                returning: true,
                ..NONE
            },
        ),
        (
            "+ returning + coaching change",
            Blocks {
                returning: true,
                coaching: true,
                ..NONE
            },
        ),
        (
            "+ returning + continuity",
            Blocks {
                returning: true,
                continuity: true,
                ..NONE
            },
        ),
        (
            "+ returning + continuity + portal",
            Blocks {
                talent: false,
                coaching: false,
                ..Blocks::ALL
            },
        ),
        (
            "+ talent",
            Blocks {
                coaching: false,
                ..Blocks::ALL
            },
        ),
        ("all, including coaching change", Blocks::ALL),
    ];
    let mut results = Vec::new();
    for (name, blocks) in variants {
        let (l, (o, d)) = best_lambda(blocks);
        println!("{name:<34} {l:>7} {o:>8.3} {d:>8.3}");
        report.push(CvRow {
            name: name.to_string(),
            rmse_o: o,
            rmse_d: d,
        });
        results.push((blocks, l, o + d));
    }

    // Keep the feature set with the lowest cross-validated error. Each set
    // adds one block, so a block (talent included, which is a recruiting
    // rating and can carry reputation) survives only if it earns its place.
    let (blocks, l, _) = *results
        .iter()
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .expect("at least one feature set");
    eprintln!("chose {blocks:?} (lambda {l})");

    let mut model = PriorModel::fit(&ex, blocks, l);
    let (cv_o, cv_d) = rmse(&ex, &cross_validate(&ex, blocks, l));
    model.sd_o = cv_o;
    model.sd_d = cv_d;
    model.report = report;
    println!("\nposition contribution (full continuity vs none, points):");
    println!("{:<6} {:>8} {:>8}", "group", "offense", "defense");
    for (g, name) in GROUPS.iter().enumerate() {
        let (o, d) = model.continuity_effect(g);
        println!("{name:<6} {o:>8.2} {d:>8.2}");
    }
    let json = serde_json::to_string_pretty(&model)?;
    std::fs::write(out, json + "\n").with_context(|| format!("writing {}", out.display()))?;
    eprintln!("wrote {}", out.display());
    Ok(())
}

/// Priors for each of `years`, each from a model refit without that
/// season's examples (and only on `template.train_years`), so no season's
/// prior has seen its own outcomes. Requires `data` to cover the union of
/// `years` and the template's training years.
pub fn loso_priors(
    data: &PriorData,
    template: &PriorModel,
    years: &[i32],
) -> Vec<Vec<Option<TeamPrior>>> {
    years
        .iter()
        .map(|&y| {
            let train: Vec<i32> = template
                .train_years
                .iter()
                .copied()
                .filter(|&t| t != y)
                .collect();
            let mut m = PriorModel::fit(&data.examples(&train), template.blocks, template.lambda);
            m.sd_o = template.sd_o;
            m.sd_d = template.sd_d;
            data.priors(y, &m)
        })
        .collect()
}

pub fn load_model(path: &Path) -> Result<PriorModel> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Sorted union of season lists.
pub fn union_years(a: &[i32], b: &[i32]) -> Vec<i32> {
    let mut v: Vec<i32> = a.iter().chain(b).copied().collect();
    v.sort();
    v.dedup();
    v
}

//! `cfbpoll readings`: estimate the offensive readings' observation model.
//!
//! Each reading is regressed on the gap it measures, the offense's
//! season-end rating minus the opposing defense's (plus home shift), from a
//! score-only run with season-start priors. The residuals give each
//! reading's error sd, its correlation with the score error, and the
//! correlations between readings. Season-end ratings carry their own error,
//! so the sds come out too large; `reading_noise_scale`, fit afterwards by
//! `cfbpoll fit`, corrects for that.

use crate::load::load_season;
use crate::prior::{load_model, loso_priors, union_years, PriorData};
use anyhow::{bail, Context, Result};
use cfbd::Cache;
use model::data::{Season, EFFICIENCY};
use model::kalman::cholesky;
use model::layout::{TeamPrior, MU};
use model::observe::home_shift;
use model::params::{Params, Reading};
use model::season::{run_season, RunOptions};
use std::path::Path;

/// One offense in one game: the gap its readings measure, its score
/// residual, and each reading's value when present.
struct Sample {
    gap: f64,
    score_resid: f64,
    values: Vec<Option<f64>>,
}

fn samples(
    season: &Season,
    params: &Params,
    priors: &[Option<TeamPrior>],
    names: &[String],
) -> Vec<Sample> {
    let run = run_season(
        season,
        params,
        &RunOptions {
            priors,
            before: None,
        },
    );
    let (layout, state) = (&run.layout, &run.state);
    let mut out = Vec::new();
    for g in &season.games {
        let Some((hs, as_)) = g.score else { continue };
        if !(season.teams[g.home].class.is_rated() && season.teams[g.away].class.is_rated()) {
            continue;
        }
        let shift = home_shift(g, params);
        for (off, def, pts, s, side) in [
            (g.home, g.away, hs, shift, 0),
            (g.away, g.home, as_, -shift, 1),
        ] {
            let mut z = layout.offense(off);
            z.extend(layout.defense(def).into_iter().map(|(i, c)| (i, -c)));
            let gap = state.combo_mean(&z) + s;
            let values: Vec<Option<f64>> = names
                .iter()
                .map(|n| g.reading(n).map(|(h, a)| if side == 0 { h } else { a }))
                .collect();
            if values.iter().all(Option::is_none) {
                continue;
            }
            out.push(Sample {
                gap,
                score_resid: pts - state.mean[MU] - gap,
                values,
            });
        }
    }
    out
}

pub fn run(
    cache: &Cache,
    years: &[i32],
    params: &Params,
    prior: Option<&Path>,
    names: &[String],
    out: &Path,
) -> Result<()> {
    let names: Vec<String> = if names.is_empty() {
        std::iter::once("ppa".to_string())
            .chain(EFFICIENCY.iter().map(|s| s.to_string()))
            .collect()
    } else {
        names.to_vec()
    };
    for n in &names {
        if n != "ppa" && !EFFICIENCY.contains(&n.as_str()) {
            bail!("unknown reading {n:?}; choose ppa or one of {EFFICIENCY:?}");
        }
    }
    let seasons: Vec<Season> = years
        .iter()
        .map(|&y| load_season(cache, y))
        .collect::<Result<_>>()?;
    let priors: Vec<Vec<Option<TeamPrior>>> = match prior {
        None => vec![Vec::new(); seasons.len()],
        Some(path) => {
            let template = load_model(path)?;
            let data = PriorData::load(cache, &union_years(years, &template.train_years), params)?;
            loso_priors(&data, &template, years)
        }
    };
    let score_only = Params {
        use_efficiency: false,
        qb_sd: 0.0,
        ..params.clone()
    };
    let all: Vec<Sample> = std::thread::scope(|s| {
        let handles: Vec<_> = seasons
            .iter()
            .zip(&priors)
            .map(|(season, p)| {
                let (score_only, names) = (&score_only, &names);
                s.spawn(move || samples(season, score_only, p, names))
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("season run panicked"))
            .collect()
    });

    let m = names.len();
    let mut readings = Vec::with_capacity(m);
    let mut resid: Vec<Vec<Option<f64>>> = vec![vec![None; all.len()]; m];
    eprintln!(
        "{:<22} {:>6} {:>9} {:>9} {:>8} {:>8}",
        "reading", "n", "mean", "scale", "sd", "r_score"
    );
    for j in 0..m {
        let xs: Vec<(usize, f64, f64)> = all
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((i, s.gap, s.values[j]?)))
            .collect();
        if xs.len() < 100 {
            bail!("reading {} has only {} samples", names[j], xs.len());
        }
        let n = xs.len() as f64;
        let mx = xs.iter().map(|x| x.1).sum::<f64>() / n;
        let my = xs.iter().map(|x| x.2).sum::<f64>() / n;
        let sxx: f64 = xs.iter().map(|x| (x.1 - mx).powi(2)).sum();
        let scale = xs.iter().map(|x| (x.1 - mx) * (x.2 - my)).sum::<f64>() / sxx;
        let mean = my - scale * mx;
        for &(i, x, y) in &xs {
            resid[j][i] = Some(y - mean - scale * x);
        }
        let e: Vec<(f64, f64)> = xs
            .iter()
            .map(|&(i, _, _)| (resid[j][i].unwrap(), all[i].score_resid))
            .collect();
        let sd = (e.iter().map(|p| p.0 * p.0).sum::<f64>() / n).sqrt();
        let score_corr = corr(&e);
        eprintln!(
            "{:<22} {:>6} {:>9.4} {:>9.5} {:>8.4} {:>8.3}",
            names[j],
            xs.len(),
            mean,
            scale,
            sd,
            score_corr
        );
        readings.push(Reading {
            name: names[j].clone(),
            mean,
            scale,
            sd,
            score_corr,
        });
    }
    let mut reading_corr = vec![0.0; m * m];
    for j in 0..m {
        for l in 0..m {
            reading_corr[j * m + l] = if j == l {
                1.0
            } else {
                let pairs: Vec<(f64, f64)> = (0..all.len())
                    .filter_map(|i| Some((resid[j][i]?, resid[l][i]?)))
                    .collect();
                corr(&pairs)
            };
        }
    }

    // Pairwise estimates need not form a valid covariance; shrink the
    // correlations toward zero until the full score-and-readings matrix is
    // positive definite.
    let mut fitted = Params {
        readings,
        reading_corr,
        reading_noise_scale: 1.0,
        ..params.clone()
    };
    let mut shrink = 1.0;
    while !fitted.is_valid() {
        shrink *= 0.98;
        if shrink < 0.5 {
            bail!("reading covariance is not positive definite even after shrinking");
        }
        for r in &mut fitted.readings {
            r.score_corr *= 0.98;
        }
        for j in 0..m {
            for l in 0..m {
                if j != l {
                    fitted.reading_corr[j * m + l] *= 0.98;
                }
            }
        }
    }
    if shrink < 1.0 {
        eprintln!(
            "shrank correlations by {:.3} to keep the covariance positive definite",
            shrink
        );
    }
    let all_idx: Vec<usize> = (0..m).collect();
    debug_assert!(cholesky(&fitted.game_noise(&all_idx, &all_idx), 2 + 2 * m).is_some());
    let text = serde_json::to_string_pretty(&fitted)?;
    std::fs::write(out, text + "\n").with_context(|| format!("writing {}", out.display()))?;
    eprintln!("wrote {}", out.display());
    Ok(())
}

fn corr(pairs: &[(f64, f64)]) -> f64 {
    let n = pairs.len() as f64;
    if n < 3.0 {
        return 0.0;
    }
    let (ma, mb) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for &(a, b) in pairs {
        sab += (a - ma) * (b - mb);
        saa += (a - ma).powi(2);
        sbb += (b - mb).powi(2);
    }
    sab / (saa * sbb).sqrt()
}

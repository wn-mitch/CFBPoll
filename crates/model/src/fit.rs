//! Hyperparameter fitting: Nelder–Mead on out-of-sample predictive density.
//!
//! Every forecast a season run makes is pregame, so the objective is the
//! mean negative log predictive density of the actual margin and total over
//! all games between rated teams. Lower is better.

use crate::data::Season;
use crate::layout::TeamPrior;
use crate::params::Params;
use crate::season::{run_season, RunOptions};
use std::f64::consts::PI;

/// A fitted parameter: how to read and write it, and the transform that
/// makes the search space unconstrained.
pub struct Knob {
    pub name: &'static str,
    pub get: fn(&Params) -> f64,
    pub set: fn(&mut Params, f64),
    pub positive: bool,
}

macro_rules! knob {
    ($name:literal, $($field:ident).+, $positive:expr) => {
        Knob {
            name: $name,
            get: |p| p.$($field).+,
            set: |p, v| p.$($field).+ = v,
            positive: $positive,
        }
    };
}

/// Parameters fit by `cfbpoll fit`. The FBS prior mean stays at zero: it is
/// confounded with the scoring level `mu_mean`.
pub fn knobs() -> Vec<Knob> {
    vec![
        knob!("mu_mean", mu_mean, true),
        knob!("mu_sd", mu_sd, true),
        knob!("hfa", hfa, false),
        knob!("score_sd", score_sd, true),
        knob!("score_corr", score_corr, false),
        knob!("margin_cap", margin_cap, true),
        knob!("fbs.sd", fbs.sd, true),
        knob!("fcs.mean", fcs.mean, false),
        knob!("fcs.sd", fcs.sd, true),
        knob!("division_ii.mean", division_ii.mean, false),
        knob!("division_iii.mean", division_iii.mean, false),
        knob!("other.mean", other.mean, false),
        knob!("conf_sd", conf_sd, true),
        knob!("team_drift_sd", team_drift_sd, true),
        knob!("conf_drift_sd", conf_drift_sd, true),
        knob!("mu_drift_sd", mu_drift_sd, true),
        knob!("reading_noise_scale", reading_noise_scale, true),
        knob!("qb_sd", qb_sd, true),
        knob!("qb_new_mean", qb_new_mean, false),
        knob!("prior_sd_scale", prior_sd_scale, true),
    ]
}

pub fn to_vector(knobs: &[Knob], p: &Params) -> Vec<f64> {
    knobs
        .iter()
        .map(|k| {
            let v = (k.get)(p);
            if k.positive {
                v.max(1e-6).ln()
            } else {
                v
            }
        })
        .collect()
}

pub fn from_vector(knobs: &[Knob], base: &Params, x: &[f64]) -> Params {
    let mut p = base.clone();
    for (k, &v) in knobs.iter().zip(x) {
        (k.set)(&mut p, if k.positive { v.exp() } else { v });
    }
    p
}

/// Log density of `N(mean, sd²)` at `x`.
fn log_normal(x: f64, mean: f64, sd: f64) -> f64 {
    let z = (x - mean) / sd;
    -0.5 * z * z - sd.ln() - 0.5 * (2.0 * PI).ln()
}

/// Sum of negative log predictive densities and game count for one season.
pub fn season_nll(season: &Season, params: &Params, priors: &[Option<TeamPrior>]) -> (f64, usize) {
    let run = run_season(
        season,
        params,
        &RunOptions {
            priors,
            before: None,
        },
    );
    let mut nll = 0.0;
    let mut n = 0;
    for (gi, p) in &run.predictions {
        let g = &season.games[*gi];
        let (h, a) = (season.teams[g.home].class, season.teams[g.away].class);
        if !(h.is_rated() && a.is_rated()) {
            continue;
        }
        let Some((hs, as_)) = g.score else { continue };
        nll -= log_normal(hs - as_, p.margin, p.margin_sd);
        nll -= log_normal(hs + as_, p.total, p.total_sd);
        n += 1;
    }
    (nll, n)
}

/// Mean NLL per game over `seasons`, evaluated in parallel.
pub fn objective(seasons: &[Season], params: &Params, priors: &[Vec<Option<TeamPrior>>]) -> f64 {
    if !params.is_valid() {
        return f64::INFINITY;
    }
    let results: Vec<(f64, usize)> = std::thread::scope(|s| {
        let handles: Vec<_> = seasons
            .iter()
            .enumerate()
            .map(|(i, season)| {
                let pri: &[Option<TeamPrior>] = priors.get(i).map_or(&[], |v| v.as_slice());
                s.spawn(move || season_nll(season, params, pri))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("season run panicked"))
            .collect()
    });
    let (nll, n) = results
        .iter()
        .fold((0.0, 0), |(a, b), (x, y)| (a + x, b + y));
    if n == 0 || !nll.is_finite() {
        f64::INFINITY
    } else {
        nll / n as f64
    }
}

/// Nelder–Mead minimization. `step` sets the initial simplex size per
/// coordinate. Stops after `max_evals` evaluations or when the simplex's
/// value spread falls below `tol`.
pub fn nelder_mead(
    mut f: impl FnMut(&[f64]) -> f64,
    x0: &[f64],
    step: &[f64],
    max_evals: usize,
    tol: f64,
    mut progress: impl FnMut(usize, f64),
) -> (Vec<f64>, f64) {
    let n = x0.len();
    let mut simplex: Vec<(Vec<f64>, f64)> = Vec::with_capacity(n + 1);
    simplex.push((x0.to_vec(), f(x0)));
    for i in 0..n {
        let mut x = x0.to_vec();
        x[i] += step[i];
        let v = f(&x);
        simplex.push((x, v));
    }
    let mut evals = n + 1;
    let sort = |s: &mut Vec<(Vec<f64>, f64)>| s.sort_by(|a, b| a.1.total_cmp(&b.1));
    sort(&mut simplex);
    while evals < max_evals {
        let spread = simplex[n].1 - simplex[0].1;
        if spread.abs() < tol {
            break;
        }
        let centroid: Vec<f64> = (0..n)
            .map(|j| simplex[..n].iter().map(|(x, _)| x[j]).sum::<f64>() / n as f64)
            .collect();
        let toward = |t: f64| -> Vec<f64> {
            (0..n)
                .map(|j| centroid[j] + t * (simplex[n].0[j] - centroid[j]))
                .collect()
        };
        let xr = toward(-1.0);
        let fr = f(&xr);
        evals += 1;
        if fr < simplex[0].1 {
            let xe = toward(-2.0);
            let fe = f(&xe);
            evals += 1;
            simplex[n] = if fe < fr { (xe, fe) } else { (xr, fr) };
        } else if fr < simplex[n - 1].1 {
            simplex[n] = (xr, fr);
        } else {
            let (xc, fc) = if fr < simplex[n].1 {
                let xc = toward(-0.5);
                let fc = f(&xc);
                (xc, fc)
            } else {
                let xc = toward(0.5);
                let fc = f(&xc);
                (xc, fc)
            };
            evals += 1;
            if fc < simplex[n].1.min(fr) {
                simplex[n] = (xc, fc);
            } else {
                // Shrink toward the best point.
                let best = simplex[0].0.clone();
                for item in simplex.iter_mut().skip(1) {
                    let x: Vec<f64> = (0..n)
                        .map(|j| best[j] + 0.5 * (item.0[j] - best[j]))
                        .collect();
                    let v = f(&x);
                    *item = (x, v);
                }
                evals += n;
            }
        }
        sort(&mut simplex);
        progress(evals, simplex[0].1);
    }
    simplex.swap_remove(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nelder_mead_finds_quadratic_minimum() {
        let f = |x: &[f64]| (x[0] - 3.0).powi(2) + 10.0 * (x[1] + 1.0).powi(2) + 0.5;
        let (x, v) = nelder_mead(f, &[0.0, 0.0], &[1.0, 1.0], 2000, 1e-14, |_, _| {});
        assert!(
            (x[0] - 3.0).abs() < 1e-4 && (x[1] + 1.0).abs() < 1e-4,
            "{x:?}"
        );
        assert!((v - 0.5).abs() < 1e-8);
    }

    #[test]
    fn vector_round_trip() {
        let k = knobs();
        // Positive knobs are searched on a log scale, so start them above zero.
        let p = Params {
            qb_sd: 2.0,
            ..Params::default()
        };
        let back = from_vector(&k, &p, &to_vector(&k, &p));
        for knob in &k {
            assert!(
                ((knob.get)(&back) - (knob.get)(&p)).abs() < 1e-9,
                "{}",
                knob.name
            );
        }
    }

    #[test]
    fn log_normal_matches_density() {
        let v = log_normal(1.0, 0.0, 2.0);
        let expect = (-(0.125f64) - (2.0f64).ln() - 0.5 * (2.0 * PI).ln()).exp();
        assert!((v.exp() - expect).abs() < 1e-15);
    }
}

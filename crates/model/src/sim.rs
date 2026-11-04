//! Monte Carlo simulation of the rest of a season: remaining games,
//! conference championships, playoff selection and bracket, plus each
//! team's rank distribution under rating uncertainty.
//!
//! Each draw samples every rated team's true overall strength from the
//! posterior (with its correlations), then plays games with margin noise.
//! Playoff selection ranks teams by an approximation of this model's own
//! end-of-season rating: a per-team (diagonal) Kalman update of the current
//! rating on the simulated results.

use crate::data::{Class, Season};
use crate::kalman::cholesky;
use crate::layout::class_prior;
use crate::params::Params;
use crate::playoff::{play_bracket, select_field, standings, PlayoffRules};
use crate::rng::Rng;
use crate::season::SeasonRun;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamOdds {
    pub expected_wins: f64,
    pub expected_losses: f64,
    pub conf_title: f64,
    pub conf_title_game: f64,
    pub playoff: f64,
    pub bye: f64,
    pub quarterfinal: f64,
    pub semifinal: f64,
    pub final_: f64,
    pub title: f64,
    /// Current-strength rank among FBS teams: 5th, 50th and 95th percentile.
    pub rank_p05: usize,
    pub rank_p50: usize,
    pub rank_p95: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimResult {
    pub draws: usize,
    pub seed: u64,
    /// Indexed by team; `None` for unrated pseudo-teams.
    pub teams: Vec<Option<TeamOdds>>,
}

#[derive(Debug, Clone, Copy)]
pub struct SimOptions {
    pub draws: usize,
    pub seed: u64,
}

impl Default for SimOptions {
    fn default() -> Self {
        Self {
            draws: 10_000,
            seed: 0x00c0_ffee,
        }
    }
}

struct Context<'a> {
    season: &'a Season,
    params: &'a Params,
    rated: Vec<usize>,
    fbs: Vec<usize>,
    /// Current posterior mean and variance of overall strength.
    mean: Vec<f64>,
    var: Vec<f64>,
    /// Lower-triangular Cholesky factor of the rated teams' covariance.
    chol: Vec<f64>,
    /// Conference name → member teams (rated only).
    conferences: Vec<(String, Vec<usize>)>,
}

impl<'a> Context<'a> {
    fn new(season: &'a Season, run: &SeasonRun, params: &'a Params) -> Self {
        let n = season.teams.len();
        let rated: Vec<usize> = (0..n)
            .filter(|&t| season.teams[t].class.is_rated())
            .collect();
        let fbs = rated
            .iter()
            .copied()
            .filter(|&t| season.teams[t].class == Class::Fbs)
            .collect();
        let rows: Vec<_> = rated.iter().map(|&t| run.layout.overall(t)).collect();
        let k = rated.len();
        let mut cov = vec![0.0; k * k];
        for i in 0..k {
            for j in 0..=i {
                let c = run.state.combo_cov(&rows[i], &rows[j]);
                cov[i * k + j] = c;
                cov[j * k + i] = c;
            }
        }
        // A tiny ridge guards against round-off making the factorization fail.
        let chol = (0..6)
            .find_map(|p| {
                let mut c = cov.clone();
                let eps = if p == 0 { 0.0 } else { 1e-9 * 10f64.powi(p) };
                for i in 0..k {
                    c[i * k + i] += eps;
                }
                cholesky(&c, k)
            })
            .expect("rating covariance is positive definite");

        let mut mean = vec![0.0; n];
        let mut var = vec![0.0; n];
        for t in 0..n {
            let class = season.teams[t].class;
            if class.is_rated() {
                let r = run.layout.overall(t);
                mean[t] = run.state.combo_mean(&r);
                var[t] = run.state.combo_var(&r);
            } else {
                let p = class_prior(params, class);
                mean[t] = 2.0 * p.mean;
                var[t] = 2.0 * p.sd.powi(2);
            }
        }
        let mut by_conf: HashMap<String, Vec<usize>> = HashMap::new();
        for &t in &rated {
            if let Some(c) = &season.teams[t].conference {
                by_conf.entry(c.clone()).or_default().push(t);
            }
        }
        let mut conferences: Vec<(String, Vec<usize>)> = by_conf.into_iter().collect();
        conferences.sort();
        Self {
            season,
            params,
            rated,
            fbs,
            mean,
            var,
            chol,
            conferences,
        }
    }

    fn draw_strengths(&self, rng: &mut Rng) -> Vec<f64> {
        let k = self.rated.len();
        let z: Vec<f64> = (0..k).map(|_| rng.normal()).collect();
        let mut theta = self.mean.clone();
        for i in 0..k {
            let s: f64 = (0..=i).map(|j| self.chol[i * k + j] * z[j]).sum();
            theta[self.rated[i]] = self.mean[self.rated[i]] + s;
        }
        theta
    }
}

/// Per-draw mutable state.
struct Draw {
    theta: Vec<f64>,
    /// Approximate model rating mean/variance after simulated games.
    rating: Vec<f64>,
    rating_var: Vec<f64>,
    wins: Vec<f64>,
    losses: Vec<f64>,
}

impl Draw {
    /// Plays one game; returns the winner and updates records and ratings.
    fn play(
        &mut self,
        ctx: &Context,
        rng: &mut Rng,
        home: usize,
        away: usize,
        neutral: bool,
    ) -> usize {
        let hfa = if neutral { 0.0 } else { ctx.params.hfa };
        let sd_noise = ctx.params.margin_var().sqrt();
        let strength = |t: usize, rng: &mut Rng| {
            let class = ctx.season.teams[t].class;
            if class.is_rated() {
                self.theta[t]
            } else {
                let p = class_prior(ctx.params, class);
                2.0 * p.mean + (2.0f64).sqrt() * p.sd * rng.normal()
            }
        };
        let margin = strength(home, rng) - strength(away, rng) + hfa + sd_noise * rng.normal();
        let (w, l) = if margin > 0.0 {
            (home, away)
        } else {
            (away, home)
        };
        self.wins[w] += 1.0;
        self.losses[l] += 1.0;

        let pred = self.rating[home] - self.rating[away] + hfa;
        let s = self.rating_var[home] + self.rating_var[away] + ctx.params.margin_var();
        let innov = margin - pred;
        for (t, sign) in [(home, 1.0), (away, -1.0)] {
            if ctx.season.teams[t].class.is_rated() {
                let v = self.rating_var[t];
                self.rating[t] += sign * v / s * innov;
                self.rating_var[t] = v - v * v / s;
            }
        }
        w
    }
}

pub fn simulate(
    season: &Season,
    run: &SeasonRun,
    params: &Params,
    rules: &PlayoffRules,
    opts: SimOptions,
) -> SimResult {
    let ctx = Context::new(season, run, params);
    let n = season.teams.len();
    let mut acc: Vec<TeamOdds> = vec![TeamOdds::default(); n];
    let mut rank_samples: Vec<Vec<usize>> = vec![Vec::with_capacity(opts.draws); n];
    let mut rng = Rng::new(opts.seed);

    // Completed games count toward every draw's records and standings.
    let mut base_wins = vec![0.0; n];
    let mut base_losses = vec![0.0; n];
    let mut conf_results: Vec<(usize, usize)> = Vec::new();
    let mut real_champions: HashMap<String, usize> = HashMap::new();
    for g in &season.games {
        let Some((h, a)) = g.score else { continue };
        let (w, l) = if h > a {
            (g.home, g.away)
        } else {
            (g.away, g.home)
        };
        if g.championship {
            if let Some(c) = &season.teams[g.home].conference {
                real_champions.insert(c.clone(), w);
            }
        }
        if !g.week.postseason {
            base_wins[w] += 1.0;
            base_losses[l] += 1.0;
        }
        if g.conference_game && !g.championship {
            conf_results.push((w, l));
        }
    }
    let remaining: Vec<usize> = (0..season.games.len())
        .filter(|&i| season.games[i].score.is_none() && !season.games[i].week.postseason)
        .collect();

    for _ in 0..opts.draws {
        let theta = ctx.draw_strengths(&mut rng);
        let mut order = ctx.fbs.clone();
        order.sort_by(|&a, &b| theta[b].total_cmp(&theta[a]));
        for (r, &t) in order.iter().enumerate() {
            rank_samples[t].push(r + 1);
        }

        let mut d = Draw {
            theta,
            rating: ctx.mean.clone(),
            rating_var: ctx.var.clone(),
            wins: base_wins.clone(),
            losses: base_losses.clone(),
        };
        let mut results = conf_results.clone();
        let mut pending_ccg: Vec<usize> = Vec::new();
        for &gi in &remaining {
            let g = &season.games[gi];
            if g.championship {
                pending_ccg.push(gi);
                continue;
            }
            let w = d.play(&ctx, &mut rng, g.home, g.away, g.neutral);
            if g.conference_game {
                results.push((w, if w == g.home { g.away } else { g.home }));
            }
        }

        let mut champions: HashMap<String, usize> = HashMap::new();
        for (conf, members) in &ctx.conferences {
            if let Some(&c) = real_champions.get(conf) {
                champions.insert(conf.clone(), c);
                acc[c].conf_title += 1.0;
                continue;
            }
            let rating = |t: usize| d.rating[t];
            let table = standings(members, &results, &rating);
            let is_fbs = season.teams[members[0]].class == Class::Fbs;
            let scheduled = pending_ccg
                .iter()
                .map(|&gi| &season.games[gi])
                .find(|g| season.teams[g.home].conference.as_ref() == Some(conf));
            let champ = match (scheduled, is_fbs && table.len() >= 2) {
                (Some(g), _) => {
                    acc[g.home].conf_title_game += 1.0;
                    acc[g.away].conf_title_game += 1.0;
                    d.play(&ctx, &mut rng, g.home, g.away, g.neutral)
                }
                (None, true) => {
                    let (a, b) = (table[0], table[1]);
                    acc[a].conf_title_game += 1.0;
                    acc[b].conf_title_game += 1.0;
                    d.play(&ctx, &mut rng, a, b, true)
                }
                (None, false) => table[0],
            };
            champions.insert(conf.clone(), champ);
            acc[champ].conf_title += 1.0;
        }

        // Records through conference championships; playoff games excluded.
        let record = (d.wins.clone(), d.losses.clone());
        let mut ranking = ctx.fbs.clone();
        ranking.sort_by(|&a, &b| d.rating[b].total_cmp(&d.rating[a]));
        let name = |t: usize| season.teams[t].name.clone();
        let field = select_field(rules, &ranking, &champions, &name);
        if field.len() == rules.field_size && rules.field_size == 12 {
            for (seed, &t) in field.iter().enumerate() {
                acc[t].playoff += 1.0;
                if seed < rules.byes {
                    acc[t].bye += 1.0;
                }
            }
            let (champ, quarter, semis, finals) =
                play_bracket(&field, rules.byes, &mut |h, a, neutral| {
                    d.play(&ctx, &mut rng, h, a, neutral)
                });
            for t in quarter {
                acc[t].quarterfinal += 1.0;
            }
            for t in semis {
                acc[t].semifinal += 1.0;
            }
            for t in finals {
                acc[t].final_ += 1.0;
            }
            acc[champ].title += 1.0;
        }
        for (t, a) in acc.iter_mut().enumerate() {
            a.expected_wins += record.0[t];
            a.expected_losses += record.1[t];
        }
    }

    let k = opts.draws as f64;
    let teams = (0..n)
        .map(|t| {
            season.teams[t].class.is_rated().then(|| {
                let mut o = acc[t].clone();
                for v in [
                    &mut o.expected_wins,
                    &mut o.expected_losses,
                    &mut o.conf_title,
                    &mut o.conf_title_game,
                    &mut o.playoff,
                    &mut o.bye,
                    &mut o.quarterfinal,
                    &mut o.semifinal,
                    &mut o.final_,
                    &mut o.title,
                ] {
                    *v /= k;
                }
                let mut r = std::mem::take(&mut rank_samples[t]);
                r.sort_unstable();
                if !r.is_empty() {
                    let q = |p: f64| r[((r.len() - 1) as f64 * p).round() as usize];
                    o.rank_p05 = q(0.05);
                    o.rank_p50 = q(0.5);
                    o.rank_p95 = q(0.95);
                }
                o
            })
        })
        .collect();
    SimResult {
        draws: opts.draws,
        seed: opts.seed,
        teams,
    }
}

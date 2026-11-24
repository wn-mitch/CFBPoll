//! Season-start prior from roster churn.
//!
//! Two ridge regressions predict a team's fresh-start end-of-season offense
//! and defense from last season's fresh-start ratings and this season's
//! roster turnover. "Fresh-start" ratings come from runs with no carried
//! prior, so the regression target never depends on the regression.

use crate::kalman::invert_spd;
use crate::layout::TeamPrior;
use serde::{Deserialize, Serialize};

/// Position groups for roster continuity and portal flow.
pub const GROUPS: [&str; 6] = ["QB", "skill", "OL", "DL", "LB", "DB"];

/// Maps a roster/portal position label to a group index.
pub fn position_group(pos: &str) -> Option<usize> {
    match pos {
        "QB" => Some(0),
        "RB" | "WR" | "TE" | "FB" | "ATH" | "APB" => Some(1),
        "OL" | "OT" | "OG" | "G" | "C" | "IOL" => Some(2),
        "DL" | "DE" | "DT" | "NT" | "EDGE" => Some(3),
        "LB" | "ILB" | "OLB" => Some(4),
        "DB" | "CB" | "S" => Some(5),
        _ => None,
    }
}

/// Inputs for one team entering one season. Ratings are relative to the
/// previous season's FBS average.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Features {
    pub fcs: bool,
    /// Last season's fresh-start end-of-season offense and defense; `None`
    /// when the team was not rated last season.
    pub prev: Option<(f64, f64)>,
    /// Mean of `prev` over this season's conference mates (the team
    /// included); `None` for independents. Conference strength persists
    /// more than a team's own deviation from it.
    #[serde(default)]
    pub conf_prev: Option<(f64, f64)>,
    /// Returning share of last season's PPA: total, passing, rushing, receiving.
    pub returning_ppa: Option<[f64; 4]>,
    /// Returning share of usage: total, passing, rushing, receiving.
    pub returning_usage: Option<[f64; 4]>,
    /// Share of last season's roster in each group that is on this season's roster.
    pub continuity: Option<[f64; 6]>,
    /// Net incoming transfer quality per group (sum of portal ratings in
    /// minus out); `None` before the portal existed.
    pub portal: Option<[f64; 6]>,
    /// 247 team talent composite.
    pub talent: Option<f64>,
    /// Whether the head coach changed since last season; `None` without
    /// coach data (FCS teams).
    #[serde(default)]
    pub new_coach: Option<bool>,
}

/// Which feature blocks the design matrix includes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocks {
    pub returning: bool,
    pub continuity: bool,
    pub portal: bool,
    pub talent: bool,
    /// A coaching change, and its interaction with last season's rating
    /// (a new coach may change how much of last season carries over).
    #[serde(default)]
    pub coaching: bool,
}

impl Blocks {
    pub const ALL: Blocks = Blocks {
        returning: true,
        continuity: true,
        portal: true,
        talent: true,
        coaching: true,
    };
}

/// Raw feature values with an availability mask. Missing values are
/// imputed with the training mean, so they contribute nothing.
fn raw(f: &Features, b: Blocks) -> Vec<Option<f64>> {
    let (po, pd) = f.prev.unzip();
    let (co, cd) = f.conf_prev.unzip();
    let mut v = vec![
        Some(if f.fcs { 1.0 } else { 0.0 }),
        Some(if f.prev.is_some() { 1.0 } else { 0.0 }),
        po,
        pd,
        co,
        cd,
    ];
    if b.returning {
        let r = f.returning_ppa;
        let u = f.returning_usage;
        for i in 0..4 {
            v.push(r.map(|x| x[i]));
        }
        for i in 1..4 {
            v.push(u.map(|x| x[i]));
        }
        // Last season's offense matters less when less of it returns.
        v.push(po.zip(r).map(|(o, r)| o * r[0]));
        v.push(po.zip(r).map(|(o, r)| o * r[1]));
    }
    if b.continuity {
        let c = f.continuity;
        for i in 0..6 {
            v.push(c.map(|x| x[i]));
        }
        v.push(po.zip(c).map(|(o, c)| o * (c[0] + c[1] + c[2]) / 3.0));
        v.push(pd.zip(c).map(|(d, c)| d * (c[3] + c[4] + c[5]) / 3.0));
    }
    if b.portal {
        for i in 0..6 {
            v.push(f.portal.map(|x| x[i]));
        }
    }
    if b.talent {
        v.push(f.talent);
    }
    if b.coaching {
        let n = f.new_coach.map(|n| if n { 1.0 } else { 0.0 });
        v.push(n);
        v.push(po.zip(n).map(|(o, n)| o * n));
        v.push(pd.zip(n).map(|(d, n)| d * n));
    }
    v
}

/// Fitted prior model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorModel {
    /// Cross-validated error of each feature set tried when this model was
    /// chosen, for publication.
    #[serde(default)]
    pub report: Vec<CvRow>,
    /// Seasons whose examples trained this model.
    #[serde(default)]
    pub train_years: Vec<i32>,
    pub blocks: Blocks,
    pub lambda: f64,
    /// Training means and sds per raw feature (for imputation and scaling).
    pub mean: Vec<f64>,
    pub scale: Vec<f64>,
    pub intercept_o: f64,
    pub intercept_d: f64,
    /// Coefficients on standardized features.
    pub beta_o: Vec<f64>,
    pub beta_d: Vec<f64>,
    /// Residual sd (cross-validated when produced by `cross_validate`).
    pub sd_o: f64,
    pub sd_d: f64,
}

/// Leave-one-season-out error of one feature set, in points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CvRow {
    pub name: String,
    pub rmse_o: f64,
    pub rmse_d: f64,
}

/// One training example: features and the season's end-of-season
/// fresh-start offense and defense.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub year: i32,
    pub team: String,
    pub features: Features,
    pub target: (f64, f64),
}

fn standardize(row: &[Option<f64>], mean: &[f64], scale: &[f64]) -> Vec<f64> {
    row.iter()
        .zip(mean.iter().zip(scale))
        .map(|(v, (m, s))| v.map_or(0.0, |v| (v - m) / s))
        .collect()
}

/// Ridge solution `(XᵀX + λI)⁻¹ Xᵀy` for already-centered `x` and `y`.
pub fn ridge(x: &[Vec<f64>], y: &[f64], lambda: f64) -> Vec<f64> {
    let k = x.first().map_or(0, Vec::len);
    let mut xtx = vec![0.0; k * k];
    let mut xty = vec![0.0; k];
    for (row, &t) in x.iter().zip(y) {
        for i in 0..k {
            xty[i] += row[i] * t;
            for j in 0..k {
                xtx[i * k + j] += row[i] * row[j];
            }
        }
    }
    for i in 0..k {
        xtx[i * k + i] += lambda;
    }
    let inv = invert_spd(&xtx, k).expect("ridge system is positive definite for lambda > 0");
    (0..k)
        .map(|i| (0..k).map(|j| inv[i * k + j] * xty[j]).sum())
        .collect()
}

impl PriorModel {
    pub fn fit(examples: &[Example], blocks: Blocks, lambda: f64) -> PriorModel {
        assert!(!examples.is_empty(), "no training examples");
        let rows: Vec<Vec<Option<f64>>> =
            examples.iter().map(|e| raw(&e.features, blocks)).collect();
        let k = rows[0].len();
        let mut mean = vec![0.0; k];
        let mut scale = vec![1.0; k];
        for j in 0..k {
            let vals: Vec<f64> = rows.iter().filter_map(|r| r[j]).collect();
            if vals.len() >= 2 {
                let m = vals.iter().sum::<f64>() / vals.len() as f64;
                let var =
                    vals.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
                mean[j] = m;
                scale[j] = if var > 1e-12 { var.sqrt() } else { 1.0 };
            }
        }
        let x: Vec<Vec<f64>> = rows.iter().map(|r| standardize(r, &mean, &scale)).collect();
        let fit_one = |target: &dyn Fn(&Example) -> f64| {
            let y: Vec<f64> = examples.iter().map(target).collect();
            let my = y.iter().sum::<f64>() / y.len() as f64;
            let yc: Vec<f64> = y.iter().map(|v| v - my).collect();
            // Features are standardized; center X so the intercept is my.
            let xm: Vec<f64> = (0..k)
                .map(|j| x.iter().map(|r| r[j]).sum::<f64>() / x.len() as f64)
                .collect();
            let xc: Vec<Vec<f64>> = x
                .iter()
                .map(|r| r.iter().zip(&xm).map(|(a, b)| a - b).collect())
                .collect();
            let beta = ridge(&xc, &yc, lambda);
            let intercept = my - beta.iter().zip(&xm).map(|(b, m)| b * m).sum::<f64>();
            let resid = examples
                .iter()
                .zip(&x)
                .map(|(e, r)| target(e) - intercept - dot(&beta, r))
                .map(|v| v * v)
                .sum::<f64>();
            let sd = (resid / examples.len().max(2) as f64).sqrt();
            (intercept, beta, sd)
        };
        let (intercept_o, beta_o, sd_o) = fit_one(&|e| e.target.0);
        let (intercept_d, beta_d, sd_d) = fit_one(&|e| e.target.1);
        let mut train_years: Vec<i32> = examples.iter().map(|e| e.year).collect();
        train_years.sort();
        train_years.dedup();
        PriorModel {
            report: Vec::new(),
            train_years,
            blocks,
            lambda,
            mean,
            scale,
            intercept_o,
            intercept_d,
            beta_o,
            beta_d,
            sd_o,
            sd_d,
        }
    }

    /// Predicted `(offense, defense)` means.
    pub fn predict_mean(&self, f: &Features) -> (f64, f64) {
        let x = standardize(&raw(f, self.blocks), &self.mean, &self.scale);
        (
            self.intercept_o + dot(&self.beta_o, &x),
            self.intercept_d + dot(&self.beta_d, &x),
        )
    }

    pub fn predict(&self, f: &Features) -> TeamPrior {
        let (o, d) = self.predict_mean(f);
        TeamPrior {
            o_mean: o,
            o_sd: self.sd_o,
            d_mean: d,
            d_sd: self.sd_d,
        }
    }

    /// Change in predicted offense and defense when one group's continuity
    /// rises from 0 to 1 for an average team: the published "position
    /// contribution". Includes the group's share of the interaction terms
    /// at the training-mean previous rating.
    pub fn continuity_effect(&self, group: usize) -> (f64, f64) {
        let mut lo = Features {
            prev: Some((self.mean[2], self.mean[3])),
            continuity: Some([0.5; 6]),
            ..Features::default()
        };
        let mut hi = lo.clone();
        if let Some(c) = lo.continuity.as_mut() {
            c[group] = 0.0;
        }
        if let Some(c) = hi.continuity.as_mut() {
            c[group] = 1.0;
        }
        let a = self.predict_mean(&lo);
        let b = self.predict_mean(&hi);
        (b.0 - a.0, b.1 - a.1)
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Leave-one-season-out predictions: each season's examples are predicted
/// by a model fit on all other seasons. Returns `(example index, prediction)`.
pub fn cross_validate(
    examples: &[Example],
    blocks: Blocks,
    lambda: f64,
) -> Vec<(usize, (f64, f64))> {
    let mut years: Vec<i32> = examples.iter().map(|e| e.year).collect();
    years.sort();
    years.dedup();
    let mut out = Vec::new();
    for y in years {
        let train: Vec<Example> = examples.iter().filter(|e| e.year != y).cloned().collect();
        if train.is_empty() {
            continue;
        }
        let m = PriorModel::fit(&train, blocks, lambda);
        for (i, e) in examples.iter().enumerate().filter(|(_, e)| e.year == y) {
            out.push((i, m.predict_mean(&e.features)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ridge_recovers_coefficients() {
        // y = 2 x0 − 3 x1, centered inputs, no noise.
        let x: Vec<Vec<f64>> = (0..50)
            .map(|i| {
                let a = (i as f64 * 0.37).sin();
                let b = (i as f64 * 0.91).cos();
                vec![a, b]
            })
            .collect();
        let mx: Vec<f64> = (0..2)
            .map(|j| x.iter().map(|r| r[j]).sum::<f64>() / 50.0)
            .collect();
        let xc: Vec<Vec<f64>> = x.iter().map(|r| vec![r[0] - mx[0], r[1] - mx[1]]).collect();
        let y: Vec<f64> = xc.iter().map(|r| 2.0 * r[0] - 3.0 * r[1]).collect();
        let b = ridge(&xc, &y, 1e-9);
        assert!(
            (b[0] - 2.0).abs() < 1e-6 && (b[1] + 3.0).abs() < 1e-6,
            "{b:?}"
        );
        let shrunk = ridge(&xc, &y, 100.0);
        assert!(
            shrunk[0].abs() < 2.0 && shrunk[1].abs() < 3.0,
            "ridge must shrink"
        );
    }

    fn example(year: i32, prev: f64, cont_qb: f64) -> Example {
        Example {
            year,
            team: format!("T{year}{prev}"),
            features: Features {
                prev: Some((prev, prev)),
                continuity: Some([cont_qb, 0.5, 0.5, 0.5, 0.5, 0.5]),
                ..Features::default()
            },
            // Truth: 60% carryover, +4 points of offense for full QB return.
            target: (0.6 * prev + 4.0 * cont_qb, 0.6 * prev),
        }
    }

    #[test]
    fn fit_recovers_carryover_and_group_effect() {
        let mut ex = Vec::new();
        for y in 2015..2020 {
            for i in 0..40 {
                let prev = (i as f64 - 20.0) * 0.8;
                let qb = ((i * 13 + y as usize) % 10) as f64 / 10.0;
                ex.push(example(y, prev, qb));
            }
        }
        let m = PriorModel::fit(&ex, Blocks::ALL, 1e-6);
        let (o, d) = m.predict_mean(&example(2021, 10.0, 1.0).features);
        assert!((o - 10.0).abs() < 0.05, "offense {o}");
        assert!((d - 6.0).abs() < 0.05, "defense {d}");
        let (qb_o, _) = m.continuity_effect(0);
        assert!((qb_o - 4.0).abs() < 0.1, "QB continuity effect {qb_o}");

        let cv = cross_validate(&ex, Blocks::ALL, 1e-6);
        assert_eq!(cv.len(), ex.len());
        for (i, (po, _)) in cv {
            assert!((po - ex[i].target.0).abs() < 0.1);
        }
    }

    #[test]
    fn missing_features_fall_back_to_training_mean() {
        let ex: Vec<Example> = (0..30)
            .map(|i| example(2020 + i % 3, i as f64 - 15.0, 0.5))
            .collect();
        let m = PriorModel::fit(&ex, Blocks::ALL, 1.0);
        let none = Features::default();
        let (o, d) = m.predict_mean(&none);
        assert!(o.is_finite() && d.is_finite());
    }

    #[test]
    fn positions_map_to_groups() {
        assert_eq!(position_group("QB"), Some(0));
        assert_eq!(position_group("EDGE"), Some(3));
        assert_eq!(position_group("CB"), Some(5));
        assert_eq!(position_group("PK"), None);
    }
}

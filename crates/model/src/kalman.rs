//! Dense Gaussian state with exact linear-Gaussian (Kalman) updates.
//!
//! Observation rows are sparse because a game touches at most a handful of
//! state entries (two teams' offense and defense, their conference levels,
//! and the scoring level).

/// A sparse linear combination of state entries: `Σ coef · x[index]`.
pub type Row = Vec<(usize, f64)>;

/// One vector observation `y = H x + offset + e`, `e ~ N(0, R)`.
#[derive(Debug, Clone)]
pub struct Observation {
    pub rows: Vec<Row>,
    pub offset: Vec<f64>,
    pub y: Vec<f64>,
    /// Row-major `k × k` noise covariance.
    pub noise: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct Gaussian {
    pub mean: Vec<f64>,
    /// Row-major `n × n`, kept symmetric.
    pub cov: Vec<f64>,
}

impl Gaussian {
    /// Independent entries with the given means and variances.
    pub fn diagonal(mean: Vec<f64>, var: &[f64]) -> Self {
        let n = mean.len();
        assert_eq!(var.len(), n);
        let mut cov = vec![0.0; n * n];
        for (i, v) in var.iter().enumerate() {
            cov[i * n + i] = *v;
        }
        Self { mean, cov }
    }

    pub fn dim(&self) -> usize {
        self.mean.len()
    }

    pub fn cov_at(&self, i: usize, j: usize) -> f64 {
        self.cov[i * self.dim() + j]
    }

    /// Mean of a linear combination.
    pub fn combo_mean(&self, row: &[(usize, f64)]) -> f64 {
        row.iter().map(|&(i, c)| c * self.mean[i]).sum()
    }

    /// Variance of a linear combination.
    pub fn combo_var(&self, row: &[(usize, f64)]) -> f64 {
        let mut v = 0.0;
        for &(i, a) in row {
            for &(j, b) in row {
                v += a * b * self.cov_at(i, j);
            }
        }
        v.max(0.0)
    }

    /// Covariance between two linear combinations.
    pub fn combo_cov(&self, a: &[(usize, f64)], b: &[(usize, f64)]) -> f64 {
        let mut v = 0.0;
        for &(i, x) in a {
            for &(j, y) in b {
                v += x * y * self.cov_at(i, j);
            }
        }
        v
    }

    /// Adds independent process noise with variance `var[i]` to entry `i`.
    pub fn add_diagonal(&mut self, var: &[f64]) {
        let n = self.dim();
        for (i, v) in var.iter().enumerate() {
            self.cov[i * n + i] += v;
        }
    }

    /// Replaces entry `i` with an independent draw from `N(mean, var)`:
    /// zeroes its covariance with every other entry.
    pub fn reset(&mut self, i: usize, mean: f64, var: f64) {
        let n = self.dim();
        for j in 0..n {
            self.cov[i * n + j] = 0.0;
            self.cov[j * n + i] = 0.0;
        }
        self.cov[i * n + i] = var;
        self.mean[i] = mean;
    }

    /// Conditions on `obs`. Returns the innovation `y − E[y]` per row.
    // Index loops mirror the matrix algebra more clearly than iterator chains.
    #[allow(clippy::needless_range_loop)]
    pub fn update(&mut self, obs: &Observation) -> Vec<f64> {
        let n = self.dim();
        let k = obs.rows.len();
        assert_eq!(obs.y.len(), k);
        assert_eq!(obs.offset.len(), k);
        assert_eq!(obs.noise.len(), k * k);

        // P Hᵀ, stored column-per-observation: pht[m][i].
        let pht: Vec<Vec<f64>> = obs
            .rows
            .iter()
            .map(|row| {
                let mut col = vec![0.0; n];
                for &(l, c) in row {
                    let base = l * n;
                    for (i, v) in col.iter_mut().enumerate() {
                        *v += c * self.cov[base + i];
                    }
                }
                col
            })
            .collect();

        // S = H P Hᵀ + R.
        let mut s = obs.noise.clone();
        for (a, row) in obs.rows.iter().enumerate() {
            for b in 0..k {
                s[a * k + b] += row.iter().map(|&(l, c)| c * pht[b][l]).sum::<f64>();
            }
        }
        let s_inv = invert_spd(&s, k).expect("innovation covariance must be positive definite");

        let innovation: Vec<f64> = (0..k)
            .map(|m| obs.y[m] - obs.offset[m] - self.combo_mean(&obs.rows[m]))
            .collect();

        // K = P Hᵀ S⁻¹, column-per-observation: gain[m][i].
        let gain: Vec<Vec<f64>> = (0..k)
            .map(|m| {
                (0..n)
                    .map(|i| (0..k).map(|b| pht[b][i] * s_inv[b * k + m]).sum())
                    .collect()
            })
            .collect();

        for i in 0..n {
            self.mean[i] += (0..k).map(|m| gain[m][i] * innovation[m]).sum::<f64>();
        }

        // P ← P − K (P Hᵀ)ᵀ, computed on the upper triangle and mirrored so
        // the matrix stays exactly symmetric.
        for i in 0..n {
            for j in i..n {
                let d: f64 = (0..k).map(|m| gain[m][i] * pht[m][j]).sum();
                let v = self.cov[i * n + j] - d;
                self.cov[i * n + j] = v;
                self.cov[j * n + i] = v;
            }
        }
        innovation
    }
}

/// Inverts a small symmetric positive-definite matrix via Cholesky.
/// Returns `None` if it is not positive definite.
pub fn invert_spd(a: &[f64], k: usize) -> Option<Vec<f64>> {
    let l = cholesky(a, k)?;
    // Solve L Lᵀ X = I column by column.
    let mut inv = vec![0.0; k * k];
    for col in 0..k {
        let mut z = vec![0.0; k];
        for i in 0..k {
            let e = if i == col { 1.0 } else { 0.0 };
            let s: f64 = (0..i).map(|j| l[i * k + j] * z[j]).sum();
            z[i] = (e - s) / l[i * k + i];
        }
        for i in (0..k).rev() {
            let s: f64 = (i + 1..k).map(|j| l[j * k + i] * inv[j * k + col]).sum();
            inv[i * k + col] = (z[i] - s) / l[i * k + i];
        }
    }
    Some(inv)
}

/// Lower-triangular Cholesky factor of a symmetric positive-definite
/// matrix, row-major. `None` if a pivot is not positive.
pub fn cholesky(a: &[f64], k: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; k * k];
    for i in 0..k {
        for j in 0..=i {
            let s: f64 = (0..j).map(|p| l[i * k + p] * l[j * k + p]).sum();
            if i == j {
                let d = a[i * k + i] - s;
                if d <= 0.0 || !d.is_finite() {
                    return None;
                }
                l[i * k + i] = d.sqrt();
            } else {
                l[i * k + j] = (a[i * k + j] - s) / l[j * k + j];
            }
        }
    }
    Some(l)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// Two independent N(0, 4) strengths, one observation of their
    /// difference with noise variance 2. Closed form: S = 4 + 4 + 2 = 10,
    /// gain for a = 4/10, for b = −4/10.
    #[test]
    fn two_entry_difference_matches_closed_form() {
        let mut g = Gaussian::diagonal(vec![0.0, 0.0], &[4.0, 4.0]);
        let obs = Observation {
            rows: vec![vec![(0, 1.0), (1, -1.0)]],
            offset: vec![0.0],
            y: vec![5.0],
            noise: vec![2.0],
        };
        g.update(&obs);
        assert!(close(g.mean[0], 2.0, 1e-12));
        assert!(close(g.mean[1], -2.0, 1e-12));
        assert!(close(g.cov_at(0, 0), 4.0 - 16.0 / 10.0, 1e-12));
        assert!(close(g.cov_at(0, 1), 16.0 / 10.0, 1e-12));
        assert!(close(g.cov_at(1, 0), g.cov_at(0, 1), 0.0));
    }

    /// A two-row observation must equal two sequential one-row updates when
    /// the rows' noise is independent.
    #[test]
    fn joint_update_equals_sequential_for_independent_noise() {
        let prior = Gaussian::diagonal(vec![1.0, -1.0, 0.5], &[3.0, 2.0, 1.0]);
        let rows = vec![
            vec![(0, 1.0), (1, -1.0), (2, 1.0)],
            vec![(1, 1.0), (0, -0.5)],
        ];
        let joint_obs = Observation {
            rows: rows.clone(),
            offset: vec![0.3, -0.2],
            y: vec![4.0, -2.0],
            noise: vec![1.5, 0.0, 0.0, 0.7],
        };
        let mut joint = prior.clone();
        joint.update(&joint_obs);

        let mut seq = prior;
        for (m, row) in rows.into_iter().enumerate() {
            seq.update(&Observation {
                rows: vec![row],
                offset: vec![joint_obs.offset[m]],
                y: vec![joint_obs.y[m]],
                noise: vec![joint_obs.noise[m * 2 + m]],
            });
        }
        for i in 0..3 {
            assert!(close(joint.mean[i], seq.mean[i], 1e-10));
            for j in 0..3 {
                assert!(close(joint.cov_at(i, j), seq.cov_at(i, j), 1e-10));
            }
        }
    }

    #[test]
    fn observation_never_increases_variance() {
        let mut g = Gaussian::diagonal(vec![0.0; 4], &[5.0, 3.0, 2.0, 1.0]);
        for step in 0..20 {
            let before: Vec<f64> = (0..4).map(|i| g.cov_at(i, i)).collect();
            let a = step % 4;
            let b = (step + 1) % 4;
            g.update(&Observation {
                rows: vec![vec![(a, 1.0), (b, -1.0)]],
                offset: vec![0.0],
                y: vec![step as f64 - 10.0],
                noise: vec![4.0],
            });
            for (i, v) in before.iter().enumerate() {
                assert!(g.cov_at(i, i) <= v + 1e-12);
            }
        }
        assert!(
            cholesky(&g.cov, 4).is_some(),
            "covariance must stay positive definite"
        );
    }

    #[test]
    fn reset_decouples_entry() {
        let mut g = Gaussian::diagonal(vec![0.0, 0.0], &[4.0, 4.0]);
        g.update(&Observation {
            rows: vec![vec![(0, 1.0), (1, -1.0)]],
            offset: vec![0.0],
            y: vec![3.0],
            noise: vec![1.0],
        });
        assert!(g.cov_at(0, 1) != 0.0);
        g.reset(1, -7.0, 9.0);
        assert_eq!(g.cov_at(0, 1), 0.0);
        assert_eq!(g.cov_at(1, 0), 0.0);
        assert_eq!(g.cov_at(1, 1), 9.0);
        assert_eq!(g.mean[1], -7.0);
    }

    #[test]
    fn spd_inverse() {
        let a = [4.0, 1.0, 1.0, 3.0];
        let inv = invert_spd(&a, 2).unwrap();
        let det = 11.0;
        let expect = [3.0 / det, -1.0 / det, -1.0 / det, 4.0 / det];
        for (x, y) in inv.iter().zip(expect) {
            assert!(close(*x, y, 1e-12));
        }
        assert!(invert_spd(&[1.0, 2.0, 2.0, 1.0], 2).is_none());
    }
}

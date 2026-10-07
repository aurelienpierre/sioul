// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A linear SVM: L2-regularized, squared hinge loss, solved in the dual by
//! coordinate descent with shrinking (Hsieh, Chang, Lin, Keerthi &
//! Sundararajan, "A dual coordinate descent method for large-scale linear
//! SVM", ICML 2008: liblinear's solver for `-s 1`).
//!
//! The primal it solves, with `x̃ = (x, bias)` and `w̃ = (w, w_b)`:
//! `min ½‖w̃‖² + Σ Cᵢ max(0, 1 − yᵢ w̃·x̃ᵢ)²`, where `Cᵢ` is `C` times the
//! weight of the message's class (ham weighs more: calling ham spam costs
//! more). The bias is a feature of its own and is regularized, as liblinear's
//! `-B`. Rows are dense `f32` (the standardized message vector and header
//! features); every sum is `f64`.

use crate::Rng;

/// Labels: spam is +1, ham −1.
pub const SPAM: i8 = 1;
pub const HAM: i8 = -1;

/// The rows to learn from.
#[derive(Debug, Clone, Copy)]
pub struct Problem<'a> {
    /// `y.len()` rows of `dim` values each, one after the other.
    pub x: &'a [f32],
    pub dim: usize,
    /// `SPAM` or `HAM`, one per row.
    pub y: &'a [i8],
}

impl Problem<'_> {
    pub fn rows(&self) -> usize {
        self.y.len()
    }

    pub fn row(&self, i: usize) -> &[f32] {
        &self.x[i * self.dim..(i + 1) * self.dim]
    }
}

/// How the SVM learns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// The cost of a margin error: lower is smoother (and steadier when labels are noisy).
    pub c: f64,
    /// `C` is multiplied by this for ham (5: calling ham spam costs five times more).
    pub ham_weight: f64,
    /// … and by this for spam.
    pub spam_weight: f64,
    /// The bias feature's value (liblinear's `-B`); 0 for no bias.
    pub bias: f64,
    /// Stops when the projected gradient's spread falls under this (liblinear: 0.1).
    pub eps: f64,
    /// Passes over the data, at most.
    pub max_passes: usize,
    /// For the order rows are visited in: the same seed, the same model.
    pub seed: u64,
}

impl Default for Params {
    fn default() -> Self {
        Params { c: 1.0, ham_weight: 5.0, spam_weight: 1.0, bias: 1.0, eps: 0.01, max_passes: 1000, seed: 1 }
    }
}

/// A learned linear SVM: `decision(x) = w·x + b`, positive for spam.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub w: Vec<f64>,
    /// The bias feature's weight times its value.
    pub b: f64,
    /// Passes made over the data.
    pub passes: usize,
    /// The stopping rule was met before `max_passes`.
    pub converged: bool,
}

impl Model {
    pub fn decision(&self, row: &[f32]) -> f64 {
        dot(&self.w, row) + self.b
    }
}

/// Learning was stopped from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

/// Learns `problem`'s SVM. `cancelled` is asked once per pass.
pub fn train(problem: &Problem, params: &Params, cancelled: &dyn Fn() -> bool) -> Result<Model, Cancelled> {
    let n = problem.rows();
    let dim = problem.dim;
    let mut w = vec![0.0f64; dim];
    let mut w_bias = 0.0f64;
    let mut alpha = vec![0.0f64; n];
    // ½ / Cᵢ: the squared hinge puts it on the dual's diagonal; no upper bound on αᵢ.
    let diag = |i: usize| {
        let weight = if problem.y[i] == SPAM { params.spam_weight } else { params.ham_weight };
        0.5 / (params.c * weight)
    };
    let qd: Vec<f64> = (0..n)
        .map(|i| {
            let row = problem.row(i);
            row.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() + params.bias * params.bias + diag(i)
        })
        .collect();
    let mut index: Vec<usize> = (0..n).collect();
    let mut active = n;
    let mut rng = Rng::new(params.seed);
    // Shrinking needs only the upper end: αᵢ has no upper bound with the squared hinge.
    let mut pg_max_old = f64::INFINITY;
    let mut passes = 0;
    let mut converged = false;
    while passes < params.max_passes {
        if cancelled() {
            return Err(Cancelled);
        }
        let (mut pg_max, mut pg_min) = (f64::NEG_INFINITY, f64::INFINITY);
        for i in 0..active {
            let j = i + rng.below(active - i);
            index.swap(i, j);
        }
        let mut s = 0;
        while s < active {
            let i = index[s];
            let yi = f64::from(problem.y[i]);
            let row = problem.row(i);
            let g = yi * (dot(&w, row) + w_bias * params.bias) - 1.0 + alpha[i] * diag(i);
            let mut pg = 0.0;
            if alpha[i] == 0.0 {
                if g > pg_max_old {
                    // Shrunk: at its bound and pushing against it; looked at again at the end.
                    active -= 1;
                    index.swap(s, active);
                    continue;
                } else if g < 0.0 {
                    pg = g;
                }
            } else {
                pg = g;
            }
            pg_max = pg_max.max(pg);
            pg_min = pg_min.min(pg);
            if pg.abs() > 1e-12 {
                let old = alpha[i];
                alpha[i] = (old - g / qd[i]).max(0.0);
                let step = (alpha[i] - old) * yi;
                for (wk, &xk) in w.iter_mut().zip(row) {
                    *wk += step * f64::from(xk);
                }
                w_bias += step * params.bias;
            }
            s += 1;
        }
        passes += 1;
        if pg_max - pg_min <= params.eps {
            if active == n {
                converged = true;
                break;
            }
            // Optimal on what is left: every row again, to be sure.
            active = n;
            pg_max_old = f64::INFINITY;
            continue;
        }
        pg_max_old = if pg_max <= 0.0 { f64::INFINITY } else { pg_max };
    }
    Ok(Model { w, b: w_bias * params.bias, passes, converged })
}

fn dot(w: &[f64], row: &[f32]) -> f64 {
    w.iter().zip(row).map(|(&a, &b)| a * f64::from(b)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact() -> Params {
        Params { eps: 1e-10, max_passes: 100_000, ..Params::default() }
    }

    /// Solved by hand: two points, no bias, `min ½w² + Σ Cᵢ(1 − w)²`.
    #[test]
    fn two_points_without_bias() {
        let x = [1.0f32, -1.0];
        let y = [SPAM, HAM];
        let problem = Problem { x: &x, dim: 1, y: &y };
        // Equal weights: ½w² + 2(1 − w)² is least at w = 4/5.
        let even = Params { bias: 0.0, ham_weight: 1.0, ..exact() };
        let model = train(&problem, &even, &|| false).unwrap();
        assert!((model.w[0] - 0.8).abs() < 1e-9, "{model:?}");
        assert_eq!(model.b, 0.0);
        // Ham weighs 5: ½w² + (1 − w)² + 5(1 − w)² is least at w = 12/13.
        let weighted = Params { bias: 0.0, ..exact() };
        let model = train(&problem, &weighted, &|| false).unwrap();
        assert!((model.w[0] - 12.0 / 13.0).abs() < 1e-9, "{model:?}");
        assert!(model.converged);
    }

    /// Solved by hand, with the regularized bias: x = 2 is spam, x = 0 ham,
    /// C = 1 for both; then x = 3, spam beyond the margin, changes nothing.
    /// ½(w² + b²) + (1 − 2w − b)² + (1 + b)²: w = 20/29, b = −16/29.
    #[test]
    fn three_points_with_bias() {
        let even = Params { ham_weight: 1.0, ..exact() };
        for (x, y) in [(vec![2.0f32, 0.0], vec![SPAM, HAM]), (vec![2.0f32, 0.0, 3.0], vec![SPAM, HAM, SPAM])] {
            let problem = Problem { x: &x, dim: 1, y: &y };
            let model = train(&problem, &even, &|| false).unwrap();
            assert!((model.w[0] - 20.0 / 29.0).abs() < 1e-9, "{model:?}");
            assert!((model.b + 16.0 / 29.0).abs() < 1e-9, "{model:?}");
            assert!((model.decision(&[2.0]) - 24.0 / 29.0).abs() < 1e-9);
            assert!(model.decision(&[3.0]) > 1.0, "beyond the margin: no weight of its own");
        }
    }

    /// Separable points in two dimensions: every one on its side.
    #[test]
    fn separates_what_can_be() {
        let mut rng = Rng::new(7);
        let (mut x, mut y) = (Vec::new(), Vec::new());
        for i in 0..400 {
            let spam = i % 2 == 0;
            let centre = if spam { 2.0 } else { -2.0 };
            x.push((centre + rng.unit() - 0.5) as f32);
            x.push((rng.unit() * 4.0 - 2.0) as f32);
            y.push(if spam { SPAM } else { HAM });
        }
        let problem = Problem { x: &x, dim: 2, y: &y };
        let model = train(&problem, &Params::default(), &|| false).unwrap();
        for i in 0..problem.rows() {
            assert_eq!(model.decision(problem.row(i)) > 0.0, y[i] == SPAM, "row {i}");
        }
        // The same seed, the same model.
        assert_eq!(train(&problem, &Params::default(), &|| false).unwrap(), model);
    }

    #[test]
    fn stops_when_asked() {
        let x = [1.0f32, -1.0];
        let y = [SPAM, HAM];
        let problem = Problem { x: &x, dim: 1, y: &y };
        assert_eq!(train(&problem, &Params::default(), &|| true), Err(Cancelled));
    }
}

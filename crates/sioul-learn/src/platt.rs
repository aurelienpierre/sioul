// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Platt scaling: an SVM's decision values turned into probabilities,
//! `P(spam | f) = 1 / (1 + exp(A·f + B))`, fitted on scores the SVM gave to
//! messages it had not learned from (Platt 1999). Fitted by Newton's method
//! with a backtracking line search, as Lin, Lin & Weng ("A note on Platt's
//! probabilistic outputs for support vector machines", Machine Learning 68,
//! 2007) wrote it: their targets, their stopping rule, a stable log-loss.

/// `A` and `B` of the sigmoid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Platt {
    pub a: f64,
    pub b: f64,
}

impl Platt {
    /// The probability that a message with decision value `f` is spam.
    pub fn probability(&self, f: f64) -> f64 {
        let t = self.a * f + self.b;
        // Never exp of a large positive number.
        if t >= 0.0 { (-t).exp() / (1.0 + (-t).exp()) } else { 1.0 / (1.0 + t.exp()) }
    }

    /// Whether a higher score always means more likely spam.
    pub fn is_increasing(&self) -> bool {
        self.a < 0.0
    }
}

/// Fits A and B on `(decision value, is spam)` pairs; None without both kinds.
pub fn fit(scores: &[(f64, bool)]) -> Option<Platt> {
    let spam = scores.iter().filter(|(_, s)| *s).count() as f64;
    let ham = scores.len() as f64 - spam;
    if spam == 0.0 || ham == 0.0 {
        return None;
    }
    // Targets pulled a little from 0 and 1 (Platt's prior), so separable data stays finite.
    let high = (spam + 1.0) / (spam + 2.0);
    let low = 1.0 / (ham + 2.0);
    let target = |is_spam: bool| if is_spam { high } else { low };
    let loss = |a: f64, b: f64| -> f64 {
        scores
            .iter()
            .map(|&(f, s)| {
                let t = f * a + b;
                let p = target(s);
                if t >= 0.0 { p * t + (1.0 + (-t).exp()).ln() } else { (p - 1.0) * t + (1.0 + t.exp()).ln() }
            })
            .sum()
    };
    let (max_iterations, min_step, sigma) = (100, 1e-10, 1e-12);
    let mut a = 0.0;
    let mut b = ((ham + 1.0) / (spam + 1.0)).ln();
    let mut value = loss(a, b);
    for _ in 0..max_iterations {
        // Gradient and Hessian (made positive definite by `sigma`).
        let (mut h11, mut h22, mut h21, mut g1, mut g2) = (sigma, sigma, 0.0, 0.0, 0.0);
        for &(f, s) in scores {
            let t = f * a + b;
            let (p, q) = if t >= 0.0 { ((-t).exp() / (1.0 + (-t).exp()), 1.0 / (1.0 + (-t).exp())) } else { (1.0 / (1.0 + t.exp()), t.exp() / (1.0 + t.exp())) };
            let d2 = p * q;
            h11 += f * f * d2;
            h22 += d2;
            h21 += f * d2;
            let d1 = target(s) - p;
            g1 += f * d1;
            g2 += d1;
        }
        if g1.abs() < 1e-5 && g2.abs() < 1e-5 {
            break;
        }
        let det = h11 * h22 - h21 * h21;
        let da = -(h22 * g1 - h21 * g2) / det;
        let db = -(-h21 * g1 + h11 * g2) / det;
        let gd = g1 * da + g2 * db;
        let mut step = 1.0;
        while step >= min_step {
            let (new_a, new_b) = (a + step * da, b + step * db);
            let new_value = loss(new_a, new_b);
            if new_value < value + 0.0001 * step * gd {
                (a, b, value) = (new_a, new_b, new_value);
                break;
            }
            step /= 2.0;
        }
        if step < min_step {
            // The line search failed: as good as it gets.
            break;
        }
    }
    Some(Platt { a, b })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rng;

    /// Scores drawn from a known sigmoid: the fit finds it again, and it rises.
    #[test]
    fn finds_a_known_sigmoid() {
        let truth = Platt { a: -2.0, b: 0.5 };
        let mut rng = Rng::new(3);
        let scores: Vec<(f64, bool)> = (0..20_000)
            .map(|_| {
                let f = rng.unit() * 6.0 - 3.0;
                (f, rng.unit() < truth.probability(f))
            })
            .collect();
        let fitted = fit(&scores).unwrap();
        assert!((fitted.a - truth.a).abs() < 0.15 && (fitted.b - truth.b).abs() < 0.1, "{fitted:?}");
        assert!(fitted.is_increasing());
        let mut last = 0.0;
        for i in -50..=50 {
            let p = fitted.probability(f64::from(i) / 10.0);
            assert!(p > last && (0.0..=1.0).contains(&p));
            last = p;
        }
    }

    /// Fully separated scores stay finite (the targets are pulled from 0 and 1).
    #[test]
    fn separated_scores_stay_finite() {
        let scores: Vec<(f64, bool)> = (0..100).map(|i| (if i % 2 == 0 { 1.0 + f64::from(i) / 100.0 } else { -1.0 - f64::from(i) / 100.0 }, i % 2 == 0)).collect();
        let fitted = fit(&scores).unwrap();
        assert!(fitted.a.is_finite() && fitted.b.is_finite() && fitted.is_increasing(), "{fitted:?}");
        assert!(fitted.probability(1.0) > 0.9 && fitted.probability(-1.0) < 0.1);
        assert_eq!(fit(&[(1.0, true)]), None, "one kind only");
    }

    #[test]
    fn probabilities_never_overflow() {
        let p = Platt { a: -5.0, b: 0.0 };
        assert_eq!(p.probability(1e6), 1.0);
        assert_eq!(p.probability(-1e6), 0.0);
    }
}

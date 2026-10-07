// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The filter's numbers, aggregate only: how much ham it would call spam and
//! how much spam it would catch at your two thresholds, each with its 95 %
//! exact binomial interval (Clopper–Pearson), the share it calls unsure, and
//! the area under the ROC curve. Nothing here names a message: these numbers
//! may be read by an AI agent, so no subject, sender or word ever enters them.

use serde::{Deserialize, Serialize};

/// `count` of `of`, with its 95 % interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Share {
    pub count: u64,
    pub of: u64,
    /// The interval's ends, as fractions (0.0123 for 1.23 %).
    pub low: f64,
    pub high: f64,
}

impl Share {
    pub fn new(count: u64, of: u64) -> Share {
        let (low, high) = clopper_pearson(count, of, 0.95);
        Share { count, of, low, high }
    }

    /// The fraction itself; none of nothing is 0.
    pub fn fraction(&self) -> f64 {
        if self.of == 0 { 0.0 } else { self.count as f64 / self.of as f64 }
    }
}

/// What one threshold does: messages at or above it are called spam.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AtThreshold {
    pub threshold: f64,
    /// Ham at or above the threshold: lost (set aside) or marked unsure.
    pub ham_called_spam: Share,
    /// Spam at or above the threshold.
    pub spam_caught: Share,
}

/// The evaluation of probabilities against labels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Numbers {
    pub ham: u64,
    pub spam: u64,
    /// At `threshold_spam`: set aside.
    pub at_spam: AtThreshold,
    /// At `threshold_unsure`: unsure or set aside.
    pub at_unsure: AtThreshold,
    /// Messages between the thresholds (unsure), of all.
    pub unsure: Share,
    /// Area under the ROC curve; none without both kinds.
    pub auc: Option<f64>,
    /// The lowest threshold that calls at most `STRICT` of the ham spam,
    /// and what it catches there; none without ham.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<AtThreshold>,
}

/// The share of ham a strict threshold may call spam: one in two hundred.
pub const STRICT: f64 = 0.005;

/// Evaluates `(probability of spam, is spam)` pairs at the two thresholds.
pub fn evaluate(scored: &[(f64, bool)], threshold_spam: f64, threshold_unsure: f64) -> Numbers {
    let spam = scored.iter().filter(|(_, s)| *s).count() as u64;
    let ham = scored.len() as u64 - spam;
    let unsure = scored.iter().filter(|&&(p, _)| p >= threshold_unsure && p < threshold_spam).count() as u64;
    let strict = strict(scored, STRICT).map(|threshold| at(scored, threshold));
    Numbers { ham, spam, at_spam: at(scored, threshold_spam), at_unsure: at(scored, threshold_unsure), unsure: Share::new(unsure, scored.len() as u64), auc: auc(scored), strict }
}

/// The lowest threshold at which at most `rate` of the ham is called spam:
/// just above the score of the ham that would be one too many. None without ham.
pub fn strict(scored: &[(f64, bool)], rate: f64) -> Option<f64> {
    let mut ham: Vec<f64> = scored.iter().filter(|(_, spam)| !spam).map(|(p, _)| *p).collect();
    if ham.is_empty() {
        return None;
    }
    ham.sort_by(|a, b| b.total_cmp(a));
    let allowed = (rate * ham.len() as f64).floor() as usize;
    // The (allowed + 1)-th surest ham must stay below: the next number above its score.
    Some(ham.get(allowed).map_or(0.0, |p| f64::from_bits(p.to_bits() + 1)))
}

/// What one threshold does to `(probability of spam, is spam)` pairs.
pub fn at(scored: &[(f64, bool)], threshold: f64) -> AtThreshold {
    let spam = scored.iter().filter(|(_, s)| *s).count() as u64;
    let ham = scored.len() as u64 - spam;
    AtThreshold {
        threshold,
        ham_called_spam: Share::new(scored.iter().filter(|&&(p, s)| !s && p >= threshold).count() as u64, ham),
        spam_caught: Share::new(scored.iter().filter(|&&(p, s)| s && p >= threshold).count() as u64, spam),
    }
}

/// The same at each of `thresholds`, so that a threshold can be chosen
/// (`sioul spam eval`'s grid): what each would set aside, and catch.
pub fn grid(scored: &[(f64, bool)], thresholds: &[f64]) -> Vec<AtThreshold> {
    thresholds.iter().map(|&t| at(scored, t)).collect()
}

/// The area under the ROC curve: the chance a random spam scores above a
/// random ham (ties count half), by ranks (Mann–Whitney).
pub fn auc(scored: &[(f64, bool)]) -> Option<f64> {
    let spam = scored.iter().filter(|(_, s)| *s).count();
    let ham = scored.len() - spam;
    if spam == 0 || ham == 0 {
        return None;
    }
    let mut sorted: Vec<(f64, bool)> = scored.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut rank_sum = 0.0;
    let mut i = 0;
    while i < sorted.len() {
        let mut j = i;
        while j < sorted.len() && sorted[j].0 == sorted[i].0 {
            j += 1;
        }
        // Ranks i+1..=j share their mean.
        let mean_rank = (i + 1 + j) as f64 / 2.0;
        rank_sum += mean_rank * sorted[i..j].iter().filter(|(_, s)| *s).count() as f64;
        i = j;
    }
    let (spam, ham) = (spam as f64, ham as f64);
    Some((rank_sum - spam * (spam + 1.0) / 2.0) / (spam * ham))
}

/// The exact (Clopper–Pearson) interval of `k` successes in `n` trials.
pub fn clopper_pearson(k: u64, n: u64, confidence: f64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let tail = (1.0 - confidence) / 2.0;
    let (k, n) = (k as f64, n as f64);
    let low = if k == 0.0 { 0.0 } else { beta_quantile(tail, k, n - k + 1.0) };
    let high = if k == n { 1.0 } else { beta_quantile(1.0 - tail, k + 1.0, n - k) };
    (low, high)
}

/// The x where the Beta(a, b) distribution reaches `p`, by bisection.
fn beta_quantile(p: f64, a: f64, b: f64) -> f64 {
    let (mut low, mut high) = (0.0f64, 1.0f64);
    for _ in 0..200 {
        let mid = (low + high) / 2.0;
        if incomplete_beta(mid, a, b) < p { low = mid } else { high = mid }
    }
    (low + high) / 2.0
}

/// The regularized incomplete beta function I_x(a, b), by its continued
/// fraction (Numerical Recipes, `betai`), with the symmetry for large x.
fn incomplete_beta(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) { front * beta_fraction(x, a, b) / a } else { 1.0 - front * beta_fraction(1.0 - x, b, a) / b }
}

/// The continued fraction of the incomplete beta function (modified Lentz).
fn beta_fraction(x: f64, a: f64, b: f64) -> f64 {
    let tiny = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..1000 {
        let m = f64::from(m);
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let step = d * c;
        h *= step;
        if (step - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

/// ln Γ(x) for x > 0 (Lanczos, g = 7, nine terms).
fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // Reflection: Γ(x)Γ(1 − x) = π / sin(πx).
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let t = x + 7.5;
    let series = G[1..].iter().enumerate().fold(G[0], |sum, (i, g)| sum + g / (x + i as f64 + 1.0));
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + series.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-4
    }

    /// Known Clopper–Pearson intervals (R's binom.test).
    #[test]
    fn exact_intervals() {
        let (low, high) = clopper_pearson(0, 10, 0.95);
        assert!(low == 0.0 && close(high, 0.308_5), "{high}");
        let (low, high) = clopper_pearson(10, 10, 0.95);
        assert!(close(low, 0.691_5) && high == 1.0, "{low}");
        let (low, high) = clopper_pearson(5, 10, 0.95);
        assert!(close(low, 0.187_1) && close(high, 0.812_9), "{low} {high}");
        let (low, high) = clopper_pearson(3, 1000, 0.95);
        assert!(close(low, 0.000_619) && close(high, 0.008_742), "{low} {high}");
        // The rule of three: no error in 3000 is under about 0.1 %.
        let (_, high) = clopper_pearson(0, 3000, 0.95);
        assert!(high > 0.000_99 && high < 0.001_23, "{high}");
        assert_eq!(clopper_pearson(0, 0, 0.95), (0.0, 1.0));
    }

    #[test]
    fn area_under_the_curve() {
        let perfect = [(0.9, true), (0.8, true), (0.2, false), (0.1, false)];
        assert_eq!(auc(&perfect), Some(1.0));
        let reversed = [(0.1, true), (0.9, false)];
        assert_eq!(auc(&reversed), Some(0.0));
        // One tie between a spam and a ham counts half: (1 + 0.5) / 2.
        let tied = [(0.5, true), (0.5, false), (0.9, true), (0.1, false)];
        assert_eq!(auc(&tied), Some(0.875));
        assert_eq!(auc(&[(0.5, true)]), None);
    }

    #[test]
    fn the_thresholds_count_what_they_should() {
        let scored = [(0.99, true), (0.96, false), (0.7, true), (0.6, false), (0.2, false), (0.1, true)];
        let numbers = evaluate(&scored, 0.95, 0.5);
        assert_eq!((numbers.ham, numbers.spam), (3, 3));
        assert_eq!((numbers.at_spam.ham_called_spam.count, numbers.at_spam.spam_caught.count), (1, 1));
        assert_eq!((numbers.at_unsure.ham_called_spam.count, numbers.at_unsure.spam_caught.count), (2, 2));
        assert_eq!((numbers.unsure.count, numbers.unsure.of), (2, 6));
        assert!(close(numbers.at_spam.spam_caught.fraction(), 1.0 / 3.0));
        // The grid: each threshold as one of the two would count.
        let grid = grid(&scored, &[0.5, 0.95, 0.99]);
        assert_eq!(grid.iter().map(|a| (a.ham_called_spam.count, a.spam_caught.count)).collect::<Vec<_>>(), vec![(2, 2), (1, 1), (0, 1)]);
        assert_eq!((grid[0], grid[1]), (numbers.at_unsure, numbers.at_spam));
        // Strict: no ham of three may be called spam (0.5 % of 3 is none): just above the surest ham, 0.96.
        let strict = numbers.strict.unwrap();
        assert!(strict.threshold > 0.96 && strict.threshold < 0.961, "{strict:?}");
        assert_eq!((strict.ham_called_spam.count, strict.spam_caught.count), (0, 1));
        // Of four hundred ham, two may be (0.5 %): the third surest sets it.
        let many: Vec<(f64, bool)> = (0..400).map(|k| (f64::from(k) / 400.0, false)).chain([(0.999, true), (0.5, true)]).collect();
        let threshold = super::strict(&many, STRICT).unwrap();
        assert_eq!(many.iter().filter(|(p, s)| !s && *p >= threshold).count(), 2, "{threshold}");
        assert!(super::strict(&[(0.9, true)], STRICT).is_none(), "no ham, no threshold");
    }
}

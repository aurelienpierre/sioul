// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An invoice's totals, computed once, in integer cents, as EN 16931 checks
//! them: each line rounded to the cent (BR-LIN), the lines summed (BR-CO-10),
//! each category and rate's tax computed on its base and rounded
//! (BR-CO-17), summed (BR-CO-14), then with tax (BR-CO-15) and due once the
//! deposits are deducted (BR-CO-16).

use super::record::{Line, TaxBreak, Totals, rounded};

/// Why a category is not taxed, as printed and as EN 16931 codes it (BT-120, BT-121).
pub type Reasons<'a> = &'a dyn Fn(&str, &[&Line]) -> (String, String);

/// The totals of `lines`, `deposits` (in cents, with tax) deducted;
/// `reasons` gives each untaxed category's reason.
pub fn compute(lines: &mut [Line], deposits: i64, reasons: Reasons) -> Totals {
    for line in lines.iter_mut() {
        line.amount = line.net();
        // A category without tax has no rate (BR-E-05, BR-AE-05, BR-O-05).
        if !taxed(&line.category) {
            line.rate = 0;
        }
    }
    let sum: i64 = lines.iter().map(|l| l.amount).sum();
    let mut keys: Vec<(String, i64)> = lines.iter().map(|l| (l.category.clone(), l.rate)).collect();
    keys.sort();
    keys.dedup();
    let breakdown: Vec<TaxBreak> = keys
        .into_iter()
        .map(|(category, rate): (String, i64)| {
            let of: Vec<&Line> = lines.iter().filter(|l| l.category == category && l.rate == rate).collect();
            let base: i64 = of.iter().map(|l| l.amount).sum();
            let tax = if taxed(&category) { i64::try_from(rounded(i128::from(base) * i128::from(rate), 10_000)).unwrap_or(0) } else { 0 };
            let (reason, reason_code) = if taxed(&category) { (String::new(), String::new()) } else { reasons(&category, &of) };
            TaxBreak { category, rate, base, tax, reason, reason_code }
        })
        .collect();
    let tax: i64 = breakdown.iter().map(|b| b.tax).sum();
    Totals { lines: sum, without_tax: sum, tax, with_tax: sum + tax, deposits, due: sum + tax - deposits, breakdown }
}

/// Whether a category carries tax: standard rated (S); the others (Z, E,
/// AE, K, G, O) none.
pub fn taxed(category: &str) -> bool {
    category == "S"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoice::record::Nature;

    fn line(amount_cents: i64, category: &str, rate: i64) -> Line {
        Line { quantity: 1000, unit: "C62".into(), price: amount_cents, base: 1, nature: Nature::Goods, ..Line::of_time("x", 0, 0, category, rate) }
    }

    /// The tax of each rate on its base, rounded once (BR-CO-17), never the sum of each line's.
    #[test]
    fn totals_add_up_as_en_16931_checks_them() {
        let none = |_: &str, _: &[&Line]| (String::new(), String::new());
        // Three lines of €0.05 at 20 %: a line's tax would round to €0.01 each, €0.03; the base's is €0.03 too.
        let mut lines = vec![line(5, "S", 2000), line(5, "S", 2000), line(5, "S", 2000), line(1000, "S", 550), line(2000, "E", 2000)];
        let totals = compute(&mut lines, 0, &none);
        assert_eq!(totals.lines, 3015);
        assert_eq!(totals.breakdown.len(), 3);
        let at = |c: &str, r: i64| totals.breakdown.iter().find(|b| b.category == c && b.rate == r).cloned().unwrap();
        assert_eq!((at("S", 2000).base, at("S", 2000).tax), (15, 3));
        assert_eq!((at("S", 550).base, at("S", 550).tax), (1000, 55));
        assert_eq!((at("E", 0).base, at("E", 0).tax), (2000, 0), "an exempt line has no rate");
        assert_eq!((totals.tax, totals.with_tax, totals.due), (58, 3073, 3073));
        // A deposit of €10 deducted.
        assert_eq!(compute(&mut lines, 1000, &none).due, 2073);
    }
}

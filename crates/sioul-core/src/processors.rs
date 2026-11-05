// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Payment processors and platforms (docs/accounting.md, "Fees"): who
//! collects money for the person and pays it out later, less a fee. Each one
//! says its names, the domains its mail comes from, the words of its payouts
//! on the bank's line ("PAYPAL", "STRIPE"), how it pays out, the export
//! columns Sioul reads, its fee rules by country (each with the day it was
//! checked and its source), the bounds a payout's fees fall within, and
//! whether a refund keeps the fee. A plain JSON file anyone can update
//! without building Sioul (`presets/processors.json`): the newest, by its
//! `checked` date, of the copy built in, one in `$XDG_DATA_DIRS/sioul/presets/`,
//! and yours in `$XDG_DATA_HOME/sioul/presets/`, as the site presets are.
//!
//! A rule is only ever an estimate, said "about": the fees of a payment come
//! from its processor's export, a fee invoice, the mail or the payout first
//! (research/money-ledger.md §3, §7 recommendation 3).

use crate::money::Money;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::LazyLock;

/// Every processor Sioul knows.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Processors {
    #[serde(default)]
    pub version: u32,
    /// When the rules were last checked: "2026-11-05". The newest file wins.
    #[serde(default)]
    pub checked: String,
    #[serde(default)]
    pub processors: Vec<Processor>,
}

/// One processor or platform.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Processor {
    /// "paypal", "stripe": a bank account's `kind` names it.
    pub id: String,
    pub name: String,
    /// "processor" (it collects card payments), "platform" (a marketplace or
    /// a freelancing platform, which keeps a commission), "account" (money
    /// is held there in its own right: Wise, Revolut).
    #[serde(default)]
    pub kind: String,
    /// The domains its mail comes from; a subdomain counts ("intl.paypal.com").
    #[serde(default)]
    pub domains: Vec<String>,
    /// The words of its payouts on the bank's line, each a whole word.
    #[serde(default)]
    pub bank_words: Vec<String>,
    /// "request", "automatic" or "batch".
    #[serde(default)]
    pub pays_out: String,
    /// The export columns Sioul reads (for the reader of this file).
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub fees: Vec<Rule>,
    #[serde(default)]
    pub bounds: Bounds,
    /// Whether a refund keeps the original fee; none when not found.
    #[serde(default)]
    pub refund_keeps_fee: Option<bool>,
    /// Its fee comes on an invoice of its own (Malt's commission, SumUp's monthly invoice).
    #[serde(default)]
    pub fee_invoice: bool,
}

/// A published fee rule: a share of the gross and a fixed part per payment.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Rule {
    /// ISO 3166-1 code, or "*" for every country.
    pub country: String,
    pub percent: f64,
    #[serde(default)]
    pub fixed: f64,
    #[serde(default)]
    pub currency: String,
    /// What the rule is for: "most commercial payments received".
    #[serde(default)]
    pub what: String,
    pub checked: String,
    pub source: String,
}

/// What a payout's fees may be, for `n` payments of `gross` in all: from
/// `low` % of it to `high` % of it plus `fixed` per payment. Sioul's own
/// bounds, wide on purpose, never a published rate.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Bounds {
    #[serde(default)]
    pub low: f64,
    #[serde(default)]
    pub high: f64,
    #[serde(default)]
    pub fixed: f64,
}

impl Bounds {
    /// The least and the most the fees of `n` payments of `gross` in all may be.
    pub fn of(&self, gross: Money, n: usize) -> (Money, Money) {
        let cents = gross.cents().abs() as f64;
        let low = (cents * self.low / 100.0).floor() as i64;
        let high = (cents * self.high / 100.0 + self.fixed * 100.0 * n as f64).ceil() as i64;
        (Money(low), Money(high))
    }
}

impl Processor {
    /// Whether a mail from `domain` is this processor's own.
    pub fn sends_from(&self, domain: &str) -> bool {
        let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
        self.domains.iter().map(|d| d.to_ascii_lowercase()).any(|d| !d.is_empty() && (domain == d || domain.ends_with(&format!(".{d}"))))
    }

    /// Whether a bank line's label names this processor, by one of its words as a whole word.
    pub fn named_in(&self, label: &str) -> bool {
        let words = crate::matching::plain_words(label);
        self.bank_words.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && words.iter().any(|x| *x == w))
    }

    /// Its rule in `country` (an ISO code), else its rule for every country.
    pub fn rule(&self, country: &str) -> Option<&Rule> {
        self.fees.iter().find(|r| r.country.eq_ignore_ascii_case(country)).or_else(|| self.fees.iter().find(|r| r.country == "*"))
    }

    /// The fee its rule gives for one payment of `gross`, about; none without a rule.
    pub fn about_fee(&self, gross: Money, country: &str) -> Option<Money> {
        let rule = self.rule(country)?;
        let cents = gross.cents().abs() as f64 * rule.percent / 100.0 + rule.fixed * 100.0;
        Some(Money(cents.round() as i64))
    }
}

impl Processors {
    /// The processor a mail from `domain` comes from.
    pub fn by_domain(&self, domain: &str) -> Option<&Processor> {
        self.processors.iter().find(|p| p.sends_from(domain))
    }

    /// The processor a bank line's label names.
    pub fn by_label(&self, label: &str) -> Option<&Processor> {
        self.processors.iter().find(|p| p.named_in(label))
    }

    /// The processor `id` names (a bank account's `kind`).
    pub fn by_id(&self, id: &str) -> Option<&Processor> {
        self.processors.iter().find(|p| p.id.eq_ignore_ascii_case(id))
    }
}

const BUILT_IN: &str = include_str!("../../../presets/processors.json");

/// The processors built into this Sioul.
pub fn built_in() -> Processors {
    serde_json::from_str(BUILT_IN).unwrap_or_default()
}

/// Where newer processors may be: yours first, then the system's.
fn places() -> Vec<PathBuf> {
    let mut places = vec![crate::config::data_dir().join("presets").join("processors.json")];
    let system = std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| if cfg!(windows) { Default::default() } else { "/usr/local/share:/usr/share".into() });
    places.extend(std::env::split_paths(&system).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join("sioul").join("presets").join("processors.json")));
    places
}

/// The newest processors: of the copy built in and those on disk, the one checked last.
pub fn load() -> Processors {
    let mut best = built_in();
    for path in places() {
        if let Some(found) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Processors>(&t).ok())
            && found.checked > best.checked
        {
            best = found;
        }
    }
    best
}

/// The processors, read once in a run of Sioul.
pub fn known() -> &'static Processors {
    static KNOWN: LazyLock<Processors> = LazyLock::new(load);
    &KNOWN
}

/// The country whose rules apply: the first of the words' countries
/// (`[words] countries`, else the contacts' region, else the locale's), else France's.
pub fn country_of(words: &crate::words::Words) -> &str {
    words.countries.first().map_or(crate::words::FIRST_COUNTRIES[0], String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_processors_read() {
        // Read strictly: a mistake in the file says where.
        let all: Processors = serde_json::from_str(BUILT_IN).unwrap();
        assert_eq!(all.version, 1);
        assert_eq!(all.checked.len(), 10);
        for p in &all.processors {
            assert!(!p.id.is_empty() && !p.name.is_empty() && !p.bank_words.is_empty(), "{}", p.id);
            assert!(["processor", "platform", "account"].contains(&p.kind.as_str()), "{}: {}", p.id, p.kind);
            assert!(["request", "automatic", "batch"].contains(&p.pays_out.as_str()), "{}: {}", p.id, p.pays_out);
            assert!(p.bounds.high >= p.bounds.low, "{}", p.id);
            for rule in &p.fees {
                assert!(rule.source.starts_with("https://") && rule.checked.len() == 10, "{}: every rule says its source and its day", p.id);
            }
        }
    }

    #[test]
    fn mail_and_bank_lines_name_their_processor() {
        let all = built_in();
        assert_eq!(all.by_domain("intl.paypal.com").map(|p| p.id.as_str()), Some("paypal"));
        assert_eq!(all.by_domain("notpaypal.com").map(|p| p.id.as_str()), None, "a lookalike domain is no subdomain");
        assert_eq!(all.by_label("VIR PAYPAL EUROPE S.A.R.L. ET CIE").map(|p| p.id.as_str()), Some("paypal"));
        assert_eq!(all.by_label("STRIPE PAYMENTS EU").map(|p| p.id.as_str()), Some("stripe"));
        assert_eq!(all.by_label("CB MALTERIE DU PORT").map(|p| p.id.as_str()), None, "a whole word only");
        // PayPal France's published rule, about: 2.90 % plus €0.35 of €52.
        assert_eq!(all.by_id("paypal").and_then(|p| p.about_fee(Money(5200), "FR")), Some(Money(186)));
        assert_eq!(all.by_id("stripe").and_then(|p| p.about_fee(Money(5200), "FR")), None, "no rule read: no estimate");
        let (low, high) = all.by_id("paypal").unwrap().bounds.of(Money(122000), 3);
        assert_eq!((low, high), (Money(0), Money(8035)));
    }
}

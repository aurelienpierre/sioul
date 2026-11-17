// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The VAT summary, for an issuer liable to VAT (design §6): per period
//! (each month, or each quarter as set) and per rate, the base and the VAT
//! collected, as each becomes due (research-fr §8.2, §9.2): on the payments
//! received for services and for deposit invoices, on the invoice's day for
//! goods, and for services too under the option for debits, as each record
//! printed it when it was issued (its issuer's copy), never as the settings
//! say today. A final invoice counts what its deposits did not. Lines whose
//! VAT is due elsewhere (the reverse charge, an intra-EU delivery, an
//! export, outside the scope) are said with their base and no tax.
//!
//! It prepares the return: Sioul files nothing, and the return's period and
//! day are not in Sioul's sources. Only invoices issued liable to VAT count.
//! A credit note takes its goods' VAT off on its day, its services' on the
//! days its refund is paid back (`books::received_of`): VAT comes off only
//! where it was due. One on services of an invoice paid beyond what it asks
//! once credited, its refund not written, is said, not counted until it is;
//! one for what was never paid needs no refund, and counts nothing.

use super::{Every, Input, Settings, period_of, shift_days, share};
use crate::invoice::{Invoice, Kind, Nature};
use jiff::civil::Date;
use serde::Serialize;
use std::collections::BTreeMap;

/// One rate's line of a period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// EN 16931's category ("S", "AE"…) and rate (basis points).
    pub category: String,
    pub rate: i64,
    pub base: i64,
    pub tax: i64,
}

/// One period's summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub every: Every,
    pub from: Date,
    pub to: Date,
    /// Taxed rates first, the highest first; then the others.
    pub entries: Vec<Entry>,
    pub base: i64,
    pub tax: i64,
    /// Credit notes on services of a paid invoice, of the period, whose refund is not written: not counted yet.
    pub credits_not_counted: Vec<String>,
    /// Invoices in another currency, not counted.
    pub foreign: Vec<String>,
}

/// A part of a record: what it sells (its nature, which says when its VAT
/// is due and where), the activity it is declared in (`books::line_activity`,
/// which says its contribution rates and its threshold), its VAT category
/// and rate.
pub type Key = (&'static str, String, String, i64);

/// A record's own parts, from its lines and its breakdown: each category
/// and rate's base and tax split by its lines' nature and activity, the
/// tax by the share of each base (the last takes what rounding leaves).
fn own_parts(invoice: &Invoice, settings: &Settings) -> BTreeMap<Key, (i64, i64)> {
    let mut out: BTreeMap<Key, (i64, i64)> = BTreeMap::new();
    for b in &invoice.totals.breakdown {
        let mut groups: BTreeMap<(&'static str, String), i64> = BTreeMap::new();
        for line in invoice.lines.iter().filter(|l| l.category == b.category && (l.rate == b.rate || !crate::invoice::totals::taxed(&b.category))) {
            let nature = if line.nature == Nature::Goods { "goods" } else { "service" };
            *groups.entry((nature, super::line_activity(settings, line))).or_default() += line.amount;
        }
        if groups.is_empty() {
            groups.insert(("service", settings.activity_unsaid().unwrap_or_default().to_string()), b.base);
        }
        let lines_base: i64 = groups.values().sum();
        let (mut base_left, mut tax_left) = (b.base, b.tax);
        let count = groups.len();
        for (i, ((nature, activity), amount)) in groups.into_iter().enumerate() {
            let (base, tax) = if i + 1 == count { (base_left, tax_left) } else { (share(amount, b.base, lines_base), share(amount, b.tax, lines_base)) };
            base_left -= base;
            tax_left -= tax;
            let at = out.entry((nature, activity, b.category.clone(), b.rate)).or_default();
            at.0 += base;
            at.1 += tax;
        }
    }
    out
}

/// `(base, tax)` spread over `like`'s keys in proportion to their bases and
/// taxes (the last takes what rounding leaves).
fn spread(like: &BTreeMap<Key, (i64, i64)>, base: i64, tax: i64) -> BTreeMap<Key, (i64, i64)> {
    let (all_base, all_tax): (i64, i64) = like.values().fold((0, 0), |(b, t), (x, y)| (b + x, t + y));
    let count = like.len();
    let (mut base_left, mut tax_left) = (base, tax);
    let mut out = BTreeMap::new();
    for (i, (key, (b, t))) in like.iter().enumerate() {
        let part_base = if i + 1 == count { base_left } else if all_base == 0 { 0 } else { share(*b, base, all_base) };
        let part_tax = if i + 1 == count { tax_left } else if all_tax == 0 { if all_base == 0 { 0 } else { share(*b, tax, all_base) } } else { share(*t, tax, all_tax) };
        base_left -= part_base;
        tax_left -= part_tax;
        out.insert(key.clone(), (part_base, part_tax));
    }
    out
}

/// The final invoice citing `deposit`, when one was issued.
fn final_of<'a>(deposit: &Invoice, all: &'a [Invoice]) -> Option<&'a Invoice> {
    all.iter().find(|f| !f.unreadable && !f.number.is_empty() && f.deposits.iter().any(|c| c.number == deposit.number))
}

/// A record's parts (`Key`), signed (a credit note's taken off), each its
/// base and tax, so that a deposit counts once (the review of phase 4, P4-2,
/// P4-7): a deposit invoice a final invoice cites pays for that invoice's
/// lines, so it takes their nature and activity, its amount spread over
/// them, whatever its own line says (before, its own line's, the choice
/// made when it was issued); a final invoice holds only what its deposits
/// did not, each deposit taken off its parts in the same proportion.
pub fn parts(invoice: &Invoice, all: &[Invoice], settings: &Settings) -> BTreeMap<Key, (i64, i64)> {
    let sign = if invoice.kind == Kind::CreditNote { -1 } else { 1 };
    let mut out = match final_of(invoice, all).filter(|_| invoice.kind == Kind::Deposit) {
        Some(last) => spread(&own_parts(last, settings), invoice.totals.without_tax, invoice.totals.tax),
        None => own_parts(invoice, settings),
    };
    let own = out.clone();
    for cited in &invoice.deposits {
        if let Some(deposit) = all.iter().find(|d| d.number == cited.number && !d.unreadable) {
            for (key, (base, tax)) in spread(&own, deposit.totals.without_tax, deposit.totals.tax) {
                let at = out.entry(key).or_default();
                at.0 -= base;
                at.1 -= tax;
            }
        }
    }
    out.retain(|_, (base, tax)| *base != 0 || *tax != 0);
    out.into_iter().map(|(k, (b, t))| (k, (sign * b, sign * t))).collect()
}

/// One amount becoming due.
struct Due {
    day: Date,
    category: String,
    rate: i64,
    base: i64,
    tax: i64,
}

/// The periods of the VAT summary from the one holding `from` to the one
/// holding `to`, each whole; none when no invoice was issued liable to VAT
/// and the scheme is not "liable" on any of these days' periods' ends.
pub fn between(input: &Input, from: Date, to: Date) -> Vec<Summary> {
    let settings = input.settings;
    let every = settings.vat_every;
    let (first, _) = period_of(every, from);
    let (_, last) = period_of(every, to);
    let liable = |i: &&Invoice| !i.number.is_empty() && !i.unreadable && i.issuer.scheme == "liable";
    let any = input.invoices.iter().filter(liable).any(|i| i.date.get(..10).and_then(|d| d.parse::<Date>().ok()).is_some_and(|d| d <= last));
    if !any && settings.scheme_on(to) != "liable" && settings.scheme_on(from) != "liable" {
        return Vec::new();
    }
    let currency = settings.currency.as_str();
    let mut due: Vec<Due> = Vec::new();
    let mut credits: Vec<(Date, String)> = Vec::new();
    let mut foreign: Vec<(Date, String)> = Vec::new();
    for invoice in input.invoices.iter().filter(liable) {
        let Some(day) = invoice.date.get(..10).and_then(|d| d.parse::<Date>().ok()) else { continue };
        if !invoice.currency.trim().is_empty() && !invoice.currency.eq_ignore_ascii_case(currency) {
            foreign.push((day, invoice.number.clone()));
            continue;
        }
        let parts = parts(invoice, input.invoices, settings);
        let deposit = invoice.kind == Kind::Deposit;
        // The option for debits as the record printed it, frozen at its issue
        // (the review of phase 4, P4-8): never today's settings.
        let debits = invoice.issuer.options.iter().any(|o| o == "debits");
        // Due on the invoice's day: goods (a deposit's on its payment), services under the option for debits.
        let on_issue = |nature: &str| !deposit && (nature == "goods" || debits);
        for ((nature, _, category, rate), (base, tax)) in &parts {
            if on_issue(nature) {
                due.push(Due { day, category: category.clone(), rate: *rate, base: *base, tax: *tax });
            }
        }
        // On each payment received, or refund paid back: its share of the record's amount.
        let received = super::received_of(invoice);
        let whole = invoice.totals.due;
        if whole > 0 {
            for p in &received {
                for ((nature, _, category, rate), (base, tax)) in &parts {
                    if !on_issue(nature) {
                        due.push(Due { day: p.day, category: category.clone(), rate: *rate, base: share(*base, p.entry, whole), tax: share(*tax, p.entry, whole) });
                    }
                }
            }
        }
        // A credit note on services whose invoice was paid beyond what it
        // still asks once credited, its refund not written yet: money owed
        // back, said. One for what was never paid needs no refund: it lowers
        // what is due, and counts nothing (the review of phase 4, P4-10).
        let owed_back = || {
            invoice.corrects.as_ref().and_then(|c| input.invoices.iter().find(|i| i.number == c.number && !i.unreadable)).is_some_and(|credited| {
                let paid: i64 = super::received_of(credited).iter().map(|p| p.amount).sum();
                paid + credited.credited(input.invoices) > credited.totals.due
            })
        };
        if invoice.kind == Kind::CreditNote && received.is_empty() && parts.keys().any(|(nature, ..)| !on_issue(nature)) && owed_back() {
            credits.push((day, invoice.number.clone()));
        }
    }
    let mut out = Vec::new();
    let mut p = first;
    while p <= last {
        let (a, b) = period_of(every, p);
        let mut by: BTreeMap<(String, i64), (i64, i64)> = BTreeMap::new();
        for d in due.iter().filter(|d| d.day >= a && d.day <= b) {
            let at = by.entry((d.category.clone(), d.rate)).or_default();
            at.0 += d.base;
            at.1 += d.tax;
        }
        let mut entries: Vec<Entry> = by.into_iter().filter(|(_, (base, tax))| *base != 0 || *tax != 0).map(|((category, rate), (base, tax))| Entry { category, rate, base, tax }).collect();
        entries.sort_by(|x, y| (x.category != "S", -x.rate, &x.category).cmp(&(y.category != "S", -y.rate, &y.category)));
        out.push(Summary {
            every,
            from: a,
            to: b,
            base: entries.iter().map(|e| e.base).sum(),
            tax: entries.iter().map(|e| e.tax).sum(),
            entries,
            credits_not_counted: credits.iter().filter(|(d, _)| *d >= a && *d <= b).map(|(_, n)| n.clone()).collect(),
            foreign: foreign.iter().filter(|(d, _)| *d >= a && *d <= b).map(|(_, n)| n.clone()).collect(),
        });
        p = shift_days(b, 1);
    }
    out
}

/// What a period's summary says: its VAT collected, rate by rate, and what
/// is not counted.
pub fn sentences(tr: &crate::i18n::Translator, s: &Summary, currency: &str) -> Vec<String> {
    let money = |m: i64| tr.money_in(crate::money::Money(m), currency);
    let mut args = crate::i18n::args();
    let name = match s.every {
        Every::Month => tr.month_year(s.from),
        Every::Quarter => {
            let mut a = crate::i18n::args();
            a.set("quarter", i64::from(super::quarter_of(s.from)));
            a.set("year", s.from.year().to_string());
            tr.text("books-total-quarter", Some(&a))
        }
    };
    args.set("period", name);
    args.set("tax", money(s.tax));
    args.set("base", money(s.base));
    let mut out = vec![tr.text(if s.entries.is_empty() { "books-vat-nothing" } else { "books-vat-head" }, Some(&args))];
    for e in &s.entries {
        let mut a = crate::i18n::args();
        a.set("name", super::category_words(tr, &e.category, e.rate));
        a.set("base", money(e.base));
        a.set("tax", money(e.tax));
        out.push(tr.text(if e.category == "S" { "books-vat-line" } else { "books-vat-line-untaxed" }, Some(&a)));
    }
    if !s.credits_not_counted.is_empty() {
        let mut a = crate::i18n::args();
        a.set("numbers", s.credits_not_counted.join(", "));
        out.push(tr.text("books-vat-credits", Some(&a)));
    }
    if !s.foreign.is_empty() {
        let mut a = crate::i18n::args();
        a.set("numbers", s.foreign.join(", "));
        out.push(tr.text("books-vat-foreign", Some(&a)));
    }
    out
}

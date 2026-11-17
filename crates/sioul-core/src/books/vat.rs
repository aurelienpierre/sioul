// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The VAT summary, for an issuer liable to VAT (design §6): per period
//! (each month, or each quarter as set) and per rate, the base and the VAT
//! collected, as each becomes due (research-fr §8.2, §9.2): on the payments
//! received for services and for deposit invoices, on the invoice's day for
//! goods, and for services too under the option for debits. A final invoice
//! counts what its deposits did not. Lines whose VAT is due elsewhere (the
//! reverse charge, an intra-EU delivery, an export, outside the scope) are
//! said with their base and no tax.
//!
//! It prepares the return: Sioul files nothing, and the return's period and
//! day are not in Sioul's sources. Only invoices issued liable to VAT count.
//! A credit note takes its goods' VAT off on its day, its services' on the
//! days its refund is paid back (`books::received_of`); one on services of
//! an invoice that was paid, whose refund is not written, is said, not
//! counted until it is.

use super::{Every, Input, period_of, shift_days, share};
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

/// An invoice's tax by nature ("goods", "service"), category and rate (base, tax), signed (a
/// credit note's taken off): its breakdown split between goods and services
/// by its lines' share of each base; a final invoice's less the deposits it
/// deducts, whose tax counted with them.
pub fn parts(invoice: &Invoice, all: &[Invoice]) -> BTreeMap<(&'static str, String, i64), (i64, i64)> {
    let mut out: BTreeMap<(&'static str, String, i64), (i64, i64)> = BTreeMap::new();
    let mut add = |of: &Invoice, sign: i64| {
        for b in &of.totals.breakdown {
            let goods: i64 = of.lines.iter().filter(|l| l.category == b.category && l.rate == b.rate && l.nature == Nature::Goods).map(|l| l.amount).sum();
            let goods_tax = if b.base == 0 { 0 } else { share(goods, b.tax, b.base) };
            for (nature, base, tax) in [("goods", goods, goods_tax), ("service", b.base - goods, b.tax - goods_tax)] {
                if base == 0 && tax == 0 {
                    continue;
                }
                let at = out.entry((nature, b.category.clone(), b.rate)).or_default();
                at.0 += sign * base;
                at.1 += sign * tax;
            }
        }
    };
    let sign = if invoice.kind == Kind::CreditNote { -1 } else { 1 };
    add(invoice, sign);
    for cited in &invoice.deposits {
        if let Some(deposit) = all.iter().find(|d| d.number == cited.number && !d.unreadable) {
            add(deposit, -sign);
        }
    }
    out.retain(|_, (base, tax)| *base != 0 || *tax != 0);
    out
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
        let parts = parts(invoice, input.invoices);
        let deposit = invoice.kind == Kind::Deposit;
        // Due on the invoice's day: goods (a deposit's on its payment), services under the option for debits.
        let on_issue = |nature: &str| !deposit && (nature == "goods" || settings.debits);
        for ((nature, category, rate), (base, tax)) in &parts {
            if on_issue(nature) {
                due.push(Due { day, category: category.clone(), rate: *rate, base: *base, tax: *tax });
            }
        }
        // On each payment received, or refund paid back: its share of the record's amount.
        let received = super::received_of(invoice);
        let whole = invoice.totals.due;
        if whole > 0 {
            for p in &received {
                for ((nature, category, rate), (base, tax)) in &parts {
                    if !on_issue(nature) {
                        due.push(Due { day: p.day, category: category.clone(), rate: *rate, base: share(*base, p.entry, whole), tax: share(*tax, p.entry, whole) });
                    }
                }
            }
        }
        // A credit note on services of an invoice that was paid, its refund not written yet: said.
        let credited_paid = || invoice.corrects.as_ref().and_then(|c| input.invoices.iter().find(|i| i.number == c.number && !i.unreadable)).is_some_and(|i| super::received_of(i).iter().any(|p| p.amount > 0));
        if invoice.kind == Kind::CreditNote && received.is_empty() && parts.keys().any(|(nature, _, _)| !on_issue(nature)) && credited_paid() {
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

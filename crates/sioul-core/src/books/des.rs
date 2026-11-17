// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The declaration of services to businesses in other member states
//! (France's DES, déclaration européenne de services: douane.gouv.fr,
//! updated 7 February 2025; art. 289 B CGI; research-fr §10.1): the
//! services whose VAT the client owes in its own country (the reverse
//! charge, category AE), month by month, at the latest the 10th working
//! day of the following month; under the franchise, the paper form cerfa
//! 13694 may be sent instead. One reminder per month, listing the invoices
//! it covers, each with its client's VAT number and its amount without VAT.
//!
//! The month of a service is its invoice's service day (the last day of its
//! period), as the invoice says it: the DES counts the month VAT became due
//! in the client's country, which Sioul's sources do not set apart for
//! services. A credit note on such services is listed too, taken off.
//! Working days are Monday to Friday: public holidays are not counted,
//! so the day said may come a day or two before the law's, never after.

use super::{Input, days_between, nth_working, shift_days, shift_months};
use crate::invoice::{Invoice, Kind};
use jiff::civil::Date;
use serde::Serialize;

/// One invoice a DES covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Covered {
    pub number: String,
    pub client: String,
    /// The client's VAT number, as the invoice printed it ("" when none).
    pub vat: String,
    /// Its services under the reverse charge, without VAT; a credit note's taken off.
    pub amount: i64,
    pub currency: String,
    pub project: String,
}

/// One month's DES.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Des {
    /// The month's first day.
    pub month: Date,
    /// The day it can be made: the next month's first.
    pub opens: Date,
    /// The day to declare by: the preset's working day of the next month.
    pub due: Date,
    pub invoices: Vec<Covered>,
    /// The amounts in the books' currency, added.
    pub total: i64,
}

impl Des {
    /// Its key, the same on every device: "2026-10".
    pub fn key(&self) -> String {
        format!("{}-{:02}", self.month.year(), self.month.month())
    }
}

/// An invoice's service day: the last day of its period, else its issue's.
fn service_day(invoice: &Invoice) -> Option<Date> {
    [invoice.period_to.as_str(), invoice.date.as_str()].into_iter().find_map(|d| d.get(..10).and_then(|d| d.parse::<Date>().ok()))
}

/// The DES of each month from `from`'s to `to`'s that has an invoice to
/// cover; none when the preset has no DES.
pub fn between(input: &Input, from: Date, to: Date) -> Vec<Des> {
    let Some(rule) = input.settings.preset.des.as_ref() else { return Vec::new() };
    let (first, last) = (from.first_of_month(), to.last_of_month());
    let mut months: std::collections::BTreeMap<Date, Vec<Covered>> = std::collections::BTreeMap::new();
    for invoice in input.invoices.iter().filter(|i| !i.number.is_empty() && !i.unreadable) {
        let Some(day) = service_day(invoice).filter(|d| *d >= first && *d <= last) else { continue };
        let nature_ok = |l: &crate::invoice::Line| rule.nature.is_empty() || rule.nature.iter().any(|n| n == if l.nature == crate::invoice::Nature::Goods { "goods" } else { "service" });
        let amount: i64 = invoice.lines.iter().filter(|l| rule.category.contains(&l.category) && nature_ok(l)).map(|l| l.amount).sum();
        if amount == 0 {
            continue;
        }
        let sign = if invoice.kind == Kind::CreditNote { -1 } else { 1 };
        months.entry(day.first_of_month()).or_default().push(Covered {
            number: invoice.number.clone(),
            client: invoice.client.first_line().to_string(),
            vat: invoice.client.id("vat").unwrap_or_default().to_string(),
            amount: sign * amount,
            currency: invoice.currency.clone(),
            project: invoice.project.clone(),
        });
    }
    let currency = input.settings.currency.as_str();
    months
        .into_iter()
        .map(|(month, invoices)| {
            let next = shift_months(month, 1);
            let total = invoices.iter().filter(|c| c.currency.trim().is_empty() || c.currency.eq_ignore_ascii_case(currency)).map(|c| c.amount).sum();
            Des { month, opens: next, due: nth_working(next, rule.working_day), invoices, total }
        })
        .collect()
}

/// The DES to make now, for the Porch: their month over by `today`, their
/// day still to come or within `after` days past it.
pub fn open_now(input: &Input, today: Date, after: i64) -> Vec<Des> {
    let from = shift_months(today.first_of_month(), -3);
    between(input, from, shift_days(today.first_of_month(), -1)).into_iter().filter(|d| d.opens <= today && days_between(d.due, today) <= after).collect()
}

/// What a DES says: the month, the day, each invoice.
pub fn sentences(tr: &crate::i18n::Translator, d: &Des, currency: &str) -> Vec<String> {
    let mut args = crate::i18n::args();
    args.set("month", tr.month_year(d.month));
    args.set("due", super::long_day(tr, d.due));
    args.set("total", tr.money_in(crate::money::Money(d.total), currency));
    args.set("n", d.invoices.len() as i64);
    let mut out = vec![tr.text("books-des", Some(&args))];
    out.extend(d.invoices.iter().map(|c| covered_words(tr, c)));
    out
}

/// One invoice it covers: "2026-031 · Example GmbH (DE123456789) · €1,200".
pub fn covered_words(tr: &crate::i18n::Translator, c: &Covered) -> String {
    let who = if c.vat.is_empty() { c.client.clone() } else { format!("{} ({})", c.client, c.vat) };
    format!("{} · {who} · {}", c.number, tr.money_in(crate::money::Money(c.amount), &c.currency))
}

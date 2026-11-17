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
//! period), as the invoice says it: the DES counts "the calendar month in
//! which VAT became due" (the customs' online service's manual, DGDDI,
//! updated September 2019), which Sioul's sources do not set apart for
//! services. A deposit and its final invoice hold the service once, the
//! final invoice what the deposits did not (`vat::parts`; the review of
//! phase 4, P4-2). A credit note is listed in the month it is issued, below
//! zero, as the customs' service enters a decrease ("minoration": "indiquer
//! la valeur de la minoration précédée du signe moins", same manual), its
//! month not settled there: said so (P4-18). Values are declared in euros,
//! rounded to the nearest euro (same manual): an invoice in another
//! currency is listed in it, not added, and said. Working days are Monday
//! to Friday: public holidays are not counted, so the day said may come a
//! day or two before the law's, never after.

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
    /// Invoices in another currency, listed, not added.
    pub foreign: usize,
    /// Credit notes listed in their own month that cite an invoice of another month.
    pub credits_elsewhere: Vec<String>,
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
    let mut elsewhere: std::collections::BTreeMap<Date, Vec<String>> = std::collections::BTreeMap::new();
    // Only a client in another member state: the reverse charge of a domestic
    // operation (France's subcontracting in construction) is no DES's. Where
    // the client is, by the one table of countries (`books::zone`).
    let other_member = |i: &Invoice| super::zone(i) == "eu";
    for invoice in input.invoices.iter().filter(|i| !i.number.is_empty() && !i.unreadable && other_member(i)) {
        // A credit note in the month it is issued; any other record in its service's.
        let credit = invoice.kind == Kind::CreditNote;
        let when = if credit { invoice.date.get(..10).and_then(|d| d.parse::<Date>().ok()) } else { service_day(invoice) };
        let Some(day) = when.filter(|d| *d >= first && *d <= last) else { continue };
        // Its services under the reverse charge, as `vat::parts` holds them: a
        // final invoice less its deposits, a deposit as what it pays for, a credit note below zero.
        let amount: i64 = super::vat::parts(invoice, input.invoices, input.settings)
            .iter()
            .filter(|((nature, _, category, _), _)| rule.category.contains(category) && (rule.nature.is_empty() || rule.nature.iter().any(|n| n == nature)))
            .map(|(_, (base, _))| base)
            .sum();
        if amount == 0 {
            continue;
        }
        if credit
            && let Some(cited) = invoice.corrects.as_ref().and_then(|c| input.invoices.iter().find(|i| i.number == c.number && !i.unreadable))
            && service_day(cited).map(|d| d.first_of_month()) != Some(day.first_of_month())
        {
            elsewhere.entry(day.first_of_month()).or_default().push(invoice.number.clone());
        }
        months.entry(day.first_of_month()).or_default().push(Covered {
            number: invoice.number.clone(),
            client: invoice.client.first_line().to_string(),
            vat: invoice.client.id("vat").unwrap_or_default().to_string(),
            amount,
            currency: invoice.currency.clone(),
            project: invoice.project.clone(),
        });
    }
    let currency = input.settings.currency.as_str();
    let ours = |c: &Covered| c.currency.trim().is_empty() || c.currency.eq_ignore_ascii_case(currency);
    months
        .into_iter()
        .map(|(month, invoices)| {
            let next = shift_months(month, 1);
            let total = invoices.iter().filter(|c| ours(c)).map(|c| c.amount).sum();
            let foreign = invoices.iter().filter(|c| !ours(c)).count();
            Des { month, opens: next, due: nth_working(next, rule.working_day), invoices, total, foreign, credits_elsewhere: elsewhere.remove(&month).unwrap_or_default() }
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
    args.set("foreign", d.foreign as i64);
    let mut out = vec![tr.text(if d.foreign == 0 { "books-des" } else { "books-des-foreign" }, Some(&args))];
    out.extend(d.invoices.iter().map(|c| covered_words(tr, c)));
    for number in &d.credits_elsewhere {
        let mut a = crate::i18n::args();
        a.set("number", number.clone());
        out.push(tr.text("books-des-credit", Some(&a)));
    }
    out.push(tr.text("books-des-rounded", None));
    out
}

/// One invoice it covers: "2026-031 · Example GmbH (DE123456789) · €1,200".
pub fn covered_words(tr: &crate::i18n::Translator, c: &Covered) -> String {
    let who = if c.vat.is_empty() { c.client.clone() } else { format!("{} ({})", c.client, c.vat) };
    format!("{} · {who} · {}", c.number, tr.money_in(crate::money::Money(c.amount), &c.currency))
}

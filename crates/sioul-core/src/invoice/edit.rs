// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A draft changed in the window's editor (docs/projects.md, "The
//! editor"), saved as it is typed: each change names only what it changes
//! (`Change`), in the words and numbers the window shows (2.5 hours, €450,
//! 20 %), made integers here, at the boundary, as a project's rate is: cents,
//! thousandths, basis points. `life::change_draft` keeps the draft made
//! again as it would be issued (`life::refresh`); an issued invoice is never
//! changed here.
//!
//! A line from time keeps its time (its minutes and its sessions): only its
//! words and its hourly price change, and taking it out leaves its time to
//! bill later. A line's tax is placed by its preset (`life::placed`) unless
//! it says its own, under "More": its category, its rate, its words.

use super::life;
use super::record::{Cited, Invoice, Kind, Line, Nature, Note};
use crate::config::InvoiceSettings;
use jiff::civil::Date;
use serde::Deserialize;

/// What the editor changed, each field when it changed.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Change {
    /// Who it bills: a client profile's id, a contact's address
    /// (`sioul:contact/<uid>`), or a name as written. The profile's
    /// language, currency, terms and order come with it.
    pub client: Option<String>,
    /// "invoice" or "deposit"; a credit note stays one.
    pub kind: Option<String>,
    /// The service's first and last days ("2026-11-02"); both empty: the
    /// day of issue, said by Sioul.
    pub period_from: Option<String>,
    pub period_to: Option<String>,
    pub terms_days: Option<u32>,
    pub end_of_month: Option<bool>,
    /// The client's purchase order (BT-13).
    pub order: Option<String>,
    pub language: Option<String>,
    pub currency: Option<String>,
    /// Your own notes, a line each; the preset's legal lines stay its own.
    pub notes: Option<Vec<String>>,
    /// Every line, in order.
    pub lines: Option<Vec<LineEdit>>,
    /// The deposit invoices a final invoice deducts, by number.
    pub deposits: Option<Vec<String>>,
}

/// One line as the editor shows it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LineEdit {
    /// Its place among the draft's lines before the change: what it keeps
    /// from there (its time); none for a new line.
    pub from: Option<usize>,
    pub description: String,
    pub details: String,
    /// In its unit: 2.5 (hours, days, units). A line of time keeps its own.
    pub quantity: f64,
    /// A Recommendation 20 code or a word for one ("h", "jours"): read as
    /// its code; anything else stays as written, and a check asks for a unit.
    pub unit: String,
    /// For one unit (a line of time: for an hour), in the invoice's currency.
    pub price: f64,
    /// An amount off the line.
    pub discount: f64,
    /// "goods" or "service".
    pub nature: String,
    /// The activity it is declared in (France: "goods-bic", "services-bic",
    /// "bnc", "cipav"); empty: the issuer's for its nature.
    pub activity: String,
    /// Its tax set here, not placed by the preset: its category ("S", "Z",
    /// "E", "AE", "K", "G", "O"), its rate in percent, the words saying why
    /// it bears no VAT here.
    pub own_tax: bool,
    pub category: String,
    pub rate: f64,
    pub exemption: String,
}

/// Cents of an amount written as a number with decimals, half away from zero.
pub fn cents(amount: f64) -> i64 {
    if amount.is_finite() { (amount * 100.0).round() as i64 } else { 0 }
}

/// Thousandths of a quantity.
pub fn thousandths(quantity: f64) -> i64 {
    if quantity.is_finite() { (quantity * 1000.0).round() as i64 } else { 0 }
}

/// A line of the editor made a record's line: a line of time keeps its
/// minutes and sessions, any other is counted in its unit.
pub fn line_of(edit: &LineEdit, before: Option<&Line>) -> Line {
    let mut line = before.cloned().unwrap_or_else(|| Line { quantity: 1000, unit: "C62".into(), base: 1, ..Line::of_time("", 0, 0, "", 0) });
    let timed = !line.sessions.is_empty() || (line.unit == "MIN" && line.base == 60);
    line.description = edit.description.trim().to_string();
    line.details = edit.details.trim().to_string();
    line.price = cents(edit.price);
    line.discount = cents(edit.discount).max(0);
    if !timed {
        line.quantity = thousandths(edit.quantity);
        line.unit = super::units::code(&edit.unit).map_or_else(|| edit.unit.trim().to_string(), str::to_string);
        line.base = 1;
    }
    line.nature = if edit.nature == "goods" { Nature::Goods } else { Nature::Service };
    line.activity = edit.activity.trim().to_string();
    line.own_tax = edit.own_tax;
    if edit.own_tax {
        line.category = edit.category.trim().to_uppercase();
        line.rate = if line.category == "S" { (edit.rate * 100.0).round() as i64 } else { 0 };
        line.exemption = edit.exemption.trim().to_string();
        line.reason_code.clear();
    }
    line.amount = line.net();
    line
}

/// What the change is made with: the settings, the invoices issued (the
/// deposits a final invoice cites), the client profiles and contacts a
/// client is found in, the window's language, and today.
pub struct Context<'a> {
    pub settings: &'a InvoiceSettings,
    pub records: &'a [Invoice],
    pub clients: &'a crate::clients::Clients,
    pub contacts: &'a [crate::contacts::Contact],
    pub fallback: &'a str,
    pub today: Date,
}

/// `change` made on `draft` (kept by `life::change_draft`, which makes it
/// again as it would be issued).
pub fn apply(draft: &mut Invoice, change: &Change, cx: &Context) {
    if let Some(reference) = &change.client
        && draft.kind != Kind::CreditNote
    {
        // Another client: its own language, currency, terms and order come with it.
        let fresh = life::for_client(cx.settings, reference, cx.clients, cx.contacts, cx.fallback, cx.today);
        draft.client_ref = fresh.client_ref;
        draft.client = fresh.client;
        draft.language = fresh.language;
        draft.currency = fresh.currency;
        draft.terms = fresh.terms;
        draft.order = fresh.order;
    }
    match change.kind.as_deref() {
        Some("deposit") if draft.kind == Kind::Invoice => {
            draft.kind = Kind::Deposit;
            draft.deposits.clear();
        }
        Some("invoice") if draft.kind == Kind::Deposit => draft.kind = Kind::Invoice,
        _ => {}
    }
    if change.period_from.is_some() || change.period_to.is_some() {
        let from = change.period_from.clone().unwrap_or_else(|| draft.period_from.clone());
        let to = change.period_to.clone().unwrap_or_else(|| draft.period_to.clone());
        let read = |t: &str| t.trim().get(..10).and_then(|d| d.parse::<Date>().ok());
        match (read(&from), read(&to)) {
            // Both said: the period, in its order.
            (Some(a), Some(b)) => {
                draft.period_from = a.min(b).to_string();
                draft.period_to = a.max(b).to_string();
                draft.period_default = false;
            }
            // One day.
            (Some(a), None) | (None, Some(a)) => {
                draft.period_from = a.to_string();
                draft.period_to = a.to_string();
                draft.period_default = false;
            }
            // None: the day of issue, said by Sioul again.
            (None, None) => {
                draft.period_from.clear();
                draft.period_to.clear();
                draft.period_default = true;
            }
        }
    }
    if let Some(days) = change.terms_days {
        draft.terms.days = days.min(3650);
    }
    if let Some(end) = change.end_of_month {
        draft.terms.end_of_month = end;
    }
    if let Some(order) = &change.order {
        draft.order = order.trim().to_string();
    }
    if let Some(language) = &change.language {
        draft.language = life::language_of(cx.settings, language, cx.fallback);
    }
    if let Some(currency) = &change.currency
        && draft.kind != Kind::CreditNote
    {
        let code = currency.trim().to_uppercase();
        if !code.is_empty() {
            draft.currency = code;
        }
    }
    if let Some(notes) = &change.notes {
        draft.notes.retain(|n| !n.legal.is_empty());
        draft.notes.extend(notes.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).map(|t| Note { text: t.to_string(), subject: String::new(), legal: String::new() }));
    }
    if let Some(lines) = &change.lines {
        let before = draft.lines.clone();
        draft.lines = lines.iter().map(|edit| line_of(edit, edit.from.and_then(|i| before.get(i)))).collect();
    }
    if let Some(numbers) = &change.deposits
        && draft.kind == Kind::Invoice
    {
        draft.deposits = numbers
            .iter()
            .filter_map(|n| cx.records.iter().find(|r| r.number == *n && r.kind == Kind::Deposit && !r.unreadable))
            .map(|d| Cited { number: d.number.clone(), date: d.date.clone(), amount: d.totals.with_tax })
            .collect();
    }
}

/// The deposit invoices a final invoice for `draft`'s client may deduct:
/// issued, for the same client (or project), in its currency, not deducted
/// by another live invoice, nor cancelled by a credit note.
pub fn deposits_for<'a>(draft: &Invoice, records: &'a [Invoice]) -> Vec<&'a Invoice> {
    let taken: Vec<&str> = records
        .iter()
        .filter(|r| r.kind == Kind::Invoice && !r.number.is_empty() && r.credited(records) < r.totals.due.max(1))
        .flat_map(|r| r.deposits.iter().map(|d| d.number.as_str()))
        .collect();
    records
        .iter()
        .filter(|r| r.kind == Kind::Deposit && !r.number.is_empty() && !r.unreadable)
        .filter(|r| r.currency.eq_ignore_ascii_case(&draft.currency))
        .filter(|r| (!draft.client_ref.is_empty() && r.client_ref == draft.client_ref) || (!draft.project.is_empty() && r.project == draft.project) || (!r.client.name.is_empty() && r.client.name == draft.client.name))
        .filter(|r| !taken.contains(&r.number.as_str()) && r.left_to_credit(records) > 0)
        .collect()
}

/// A deposit's amount, excluding tax, in cents: `amount` as written, or
/// `share` percent of `of` (a final invoice's total excluding tax).
pub fn deposit_amount(amount: f64, share: f64, of: i64) -> i64 {
    if share > 0.0 { i64::try_from(super::record::rounded(i128::from(of) * i128::from((share * 100.0).round() as i64), 10_000)).unwrap_or(0) } else { cents(amount) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IdSetting, TaxSetting};

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn settings() -> InvoiceSettings {
        InvoiceSettings {
            preset: "fr".into(),
            legal_form: "ei".into(),
            regime: "micro".into(),
            name: "Camille Exemple".into(),
            address: "2 place de l’Exemple".into(),
            postcode: "69001".into(),
            city: "Lyon".into(),
            ids: vec![IdSetting { kind: "siret".into(), value: "732 829 320 00017".into(), printed: true }],
            tax: vec![TaxSetting { from: String::new(), scheme: "liable".into(), mention: String::new() }],
            rate: 60.0,
            ..InvoiceSettings::default()
        }
    }

    /// The editor's numbers made integers at the boundary; a line of time
    /// keeps its minutes and sessions whatever the editor says; a line's own
    /// tax kept, the others placed by the preset again.
    #[test]
    fn the_editor_s_lines_become_the_record_s() {
        let s = settings();
        let today = day("2026-11-17");
        let clients = crate::clients::Clients::default();
        let cx = Context { settings: &s, records: &[], clients: &clients, contacts: &[], fallback: "en", today };
        let mut draft = life::blank(&s, crate::invoice::Client { name: "Studio Lumen".into(), ..Default::default() }, "Studio Lumen", "en", "EUR", crate::invoice::Terms { days: 30, end_of_month: false }, today);
        draft.lines.push(Line { sessions: vec!["s1".into()], ..Line::of_time("Build the site", 90, 6000, "S", 2000) });
        let edit = Change {
            lines: Some(vec![
                LineEdit { from: Some(0), description: "Build the site, first part".into(), quantity: 99.0, unit: "h".into(), price: 70.0, nature: "service".into(), ..LineEdit::default() },
                LineEdit { description: "Poster".into(), quantity: 2.5, unit: "pcs".into(), price: 12.3, discount: 1.0, nature: "goods".into(), ..LineEdit::default() },
                LineEdit { description: "Book".into(), quantity: 1.0, unit: "C62".into(), price: 20.0, nature: "goods".into(), own_tax: true, category: "S".into(), rate: 5.5, ..LineEdit::default() },
            ]),
            period_from: Some("2026-11-10".into()),
            period_to: Some("2026-11-02".into()),
            notes: Some(vec!["Thank you.".into(), " ".into()]),
            ..Change::default()
        };
        apply(&mut draft, &edit, &cx);
        life::refresh(&mut draft, &s, today);
        let time = &draft.lines[0];
        assert_eq!((time.description.as_str(), time.minutes(), time.price, time.sessions.len(), time.amount), ("Build the site, first part", Some(90), 7000, 1, 10500), "its time kept, its price changed");
        let poster = &draft.lines[1];
        assert_eq!((poster.quantity, poster.unit.as_str(), poster.price, poster.discount, poster.amount, poster.nature), (2500, "C62", 1230, 100, 2975, Nature::Goods));
        assert_eq!((poster.category.as_str(), poster.rate), ("S", 2000), "placed by the preset");
        assert_eq!((draft.lines[2].rate, draft.lines[2].own_tax), (550, true), "its own rate kept");
        assert_eq!((draft.period_from.as_str(), draft.period_to.as_str(), draft.period_default), ("2026-11-02", "2026-11-10", false), "the period in its order");
        assert_eq!(draft.notes.iter().filter(|n| n.legal.is_empty()).map(|n| n.text.as_str()).collect::<Vec<_>>(), ["Thank you."]);
        // Both days taken out: the day of issue again, said by Sioul.
        apply(&mut draft, &Change { period_from: Some(String::new()), period_to: Some(String::new()), ..Change::default() }, &cx);
        life::refresh(&mut draft, &s, today);
        assert_eq!((draft.period_from.as_str(), draft.period_default), ("2026-11-17", true));
        // A deposit invoice, then back: no period on a deposit.
        apply(&mut draft, &Change { kind: Some("deposit".into()), ..Change::default() }, &cx);
        life::refresh(&mut draft, &s, today);
        assert_eq!((draft.kind, draft.period_from.as_str()), (Kind::Deposit, ""));
    }

    /// A final invoice deducts the deposits issued for its client, each once:
    /// one another invoice deducts is not offered again.
    #[test]
    fn a_final_invoice_cites_its_deposits() {
        let s = settings();
        let today = day("2026-11-17");
        let client = crate::invoice::Client { name: "Studio Lumen".into(), ..Default::default() };
        let deposit = |number: &str| Invoice { kind: Kind::Deposit, number: number.into(), date: "2026-10-01".into(), currency: "EUR".into(), client_ref: "Studio Lumen".into(), client: client.clone(), totals: crate::invoice::Totals { with_tax: 30_000, due: 30_000, ..Default::default() }, ..Invoice::default() };
        let taken = Invoice { kind: Kind::Invoice, number: "2026-020".into(), currency: "EUR".into(), deposits: vec![Cited { number: "2026-010".into(), date: "2026-10-01".into(), amount: 30_000 }], totals: crate::invoice::Totals { due: 50_000, ..Default::default() }, ..Invoice::default() };
        let records = vec![deposit("2026-010"), deposit("2026-011"), taken];
        let mut draft = life::blank(&s, client.clone(), "Studio Lumen", "en", "EUR", crate::invoice::Terms { days: 30, end_of_month: false }, today);
        assert_eq!(deposits_for(&draft, &records).iter().map(|d| d.number.as_str()).collect::<Vec<_>>(), ["2026-011"]);
        let clients = crate::clients::Clients::default();
        let cx = Context { settings: &s, records: &records, clients: &clients, contacts: &[], fallback: "en", today };
        draft.lines.push(Line { quantity: 1000, unit: "C62".into(), price: 100_000, base: 1, ..Line::of_time("Site", 0, 0, "S", 2000) });
        apply(&mut draft, &Change { deposits: Some(vec!["2026-011".into(), "2026-099".into()]), ..Change::default() }, &cx);
        life::refresh(&mut draft, &s, today);
        assert_eq!(draft.deposits.iter().map(|d| d.number.as_str()).collect::<Vec<_>>(), ["2026-011"]);
        assert_eq!((draft.totals.with_tax, draft.totals.deposits, draft.totals.due), (120_000, 30_000, 90_000));
        assert_eq!(deposit_amount(0.0, 30.0, 100_000), 30_000, "a share of the total");
        assert_eq!(deposit_amount(250.5, 0.0, 100_000), 25_050, "an amount");
    }
}

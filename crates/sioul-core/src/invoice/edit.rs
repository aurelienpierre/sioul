// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A draft changed in the window's editor (docs/projects.md, "The
//! editor"), saved as it is typed. Each change names only what it changes
//! (`Change`): a field, or a line by its id with only the fields changed,
//! in the words and numbers the window shows (2.5 hours, €450, 20 %), made
//! integers here, at the boundary, as a project's rate is: cents,
//! thousandths, basis points. So a change is merged into the draft as it is
//! now, field by field and line by line: what changed elsewhere meanwhile
//! (another device, a later "Make the invoice") is never written over, and
//! is said (`save`: the draft's revision as the editor read it). A line of
//! time keeps its minutes and sessions under its id: only its words and its
//! hourly price change; taken out, its time is left aside
//! (`Invoice::set_aside`), never put back by "Make the invoice". A line's
//! tax is placed by its preset (`life::placed`) unless it says its own,
//! under "More": VAT at a rate takes the preset's standard rate when it is
//! chosen without one. `life::change_draft` keeps the draft made again as it
//! would be issued (`life::refresh`); an issued invoice is never changed
//! here, nor a draft a newer Sioul wrote.

use super::life;
use super::presets;
use super::record::{Cited, Invoice, Kind, Line, Nature, Note};
use crate::config::InvoiceSettings;
use jiff::civil::Date;
use serde::Deserialize;
use std::path::Path;

/// What the editor changed, each field when it changed.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Change {
    /// The draft's revision as the editor last read it (`invoice::draft_revision`):
    /// when the draft changed elsewhere since, its changes are kept, this
    /// one merged into them, and that is said.
    pub revision: Option<String>,
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
    /// Lines changed, each named by its id with only the fields changed; a
    /// line without an id is a new one, added at the end.
    pub lines: Vec<LineEdit>,
    /// Lines taken out, by id: a line of time leaves its time aside.
    pub removed: Vec<String>,
    /// The deposit invoices a final invoice deducts, by number.
    pub deposits: Option<Vec<String>>,
}

/// A line's fields the editor changed.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LineEdit {
    /// The line's id; none for a new line.
    pub id: Option<String>,
    pub description: Option<String>,
    pub details: Option<String>,
    /// In its unit: 2.5 (hours, days, units). A line of time keeps its own.
    pub quantity: Option<f64>,
    /// A Recommendation 20 code or a word for one ("h", "jours"): read as
    /// its code; anything else stays as written, and a check asks for a unit.
    pub unit: Option<String>,
    /// For one unit (a line of time: for an hour), in the invoice's currency.
    pub price: Option<f64>,
    /// An amount off the line.
    pub discount: Option<f64>,
    /// "goods" or "service".
    pub nature: Option<String>,
    /// The activity it is declared in (France: "goods-bic", "services-bic",
    /// "bnc", "cipav"); empty: the issuer's for its nature.
    pub activity: Option<String>,
    /// Its tax set here, not placed by the preset: its category ("S", "Z",
    /// "E", "AE", "K", "G", "O"), its rate in percent, the words saying why
    /// it bears no VAT here.
    pub own_tax: Option<bool>,
    pub category: Option<String>,
    pub rate: Option<f64>,
    pub exemption: Option<String>,
}

/// What a change found to say besides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// Deposits of another client or another currency than the draft's
    /// now: no longer deducted.
    DepositsLeft(Vec<String>),
    /// A line changed here was taken out elsewhere meanwhile: its change is dropped.
    LineGone,
}

/// Cents of an amount written as a number with decimals, half away from zero.
pub fn cents(amount: f64) -> i64 {
    if amount.is_finite() { (amount * 100.0).round() as i64 } else { 0 }
}

/// Thousandths of a quantity.
pub fn thousandths(quantity: f64) -> i64 {
    if quantity.is_finite() { (quantity * 1000.0).round() as i64 } else { 0 }
}

/// A line of time: its minutes come from the time noted, never typed.
pub fn timed(line: &Line) -> bool {
    !line.sessions.is_empty() || (line.unit == "MIN" && line.base == 60)
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

/// The fields of `edit` written on `line`: a line of time keeps its
/// minutes and its unit; VAT at a rate chosen with no rate takes the
/// preset's standard one, which the editor shows as it is.
fn edit_line(line: &mut Line, edit: &LineEdit, cx: &Context) {
    if let Some(text) = &edit.description {
        line.description = text.trim().to_string();
    }
    if let Some(text) = &edit.details {
        line.details = text.trim().to_string();
    }
    if let Some(price) = edit.price {
        line.price = cents(price);
    }
    if let Some(discount) = edit.discount {
        line.discount = cents(discount).max(0);
    }
    if !timed(line) {
        if let Some(quantity) = edit.quantity {
            line.quantity = thousandths(quantity);
        }
        if let Some(unit) = &edit.unit {
            line.unit = super::units::code(unit).map_or_else(|| unit.trim().to_string(), str::to_string);
            line.base = 1;
        }
    }
    if let Some(nature) = &edit.nature {
        line.nature = if nature == "goods" { Nature::Goods } else { Nature::Service };
    }
    if let Some(activity) = &edit.activity {
        line.activity = activity.trim().to_string();
    }
    if let Some(own) = edit.own_tax {
        line.own_tax = own;
    }
    if let Some(category) = &edit.category {
        line.category = category.trim().to_uppercase();
        if line.category != "S" {
            line.rate = 0;
        }
        line.reason_code.clear();
    }
    if let Some(rate) = edit.rate {
        line.rate = if rate.is_finite() { (rate * 100.0).round() as i64 } else { 0 };
    }
    if let Some(words) = &edit.exemption {
        line.exemption = words.trim().to_string();
    }
    // VAT at a rate with no rate: the preset's standard rate (the review of phase 3, F3).
    if line.own_tax && line.category == "S" && line.rate <= 0 && edit.rate.is_none() {
        let preset = presets::get(&life::issuer_on(cx.settings, cx.today).preset);
        if let Some(first) = preset.rates_on(cx.today).first() {
            line.rate = first.rate;
        }
    }
    line.amount = line.net();
}

/// The deposits of `draft` that bill another client or another currency
/// than the draft's own (`life::same_client`), by number.
fn foreign_deposits(draft: &Invoice, records: &[Invoice]) -> Vec<String> {
    draft
        .deposits
        .iter()
        .filter(|c| records.iter().find(|r| r.number == c.number && r.kind == Kind::Deposit).is_some_and(|d| !life::same_client(d, draft) || !d.currency.eq_ignore_ascii_case(&draft.currency)))
        .map(|c| c.number.clone())
        .collect()
}

/// `change` made on `draft` as it is now (kept by `life::change_draft`,
/// which makes it again as it would be issued); what it found to say.
pub fn apply(draft: &mut Invoice, change: &Change, cx: &Context) -> Vec<Said> {
    let mut said = Vec::new();
    let before = (draft.client_ref.clone(), draft.currency.clone());
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
        match (crate::config::read_day(&from), crate::config::read_day(&to)) {
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
    // Lines taken out: a line of time leaves its time aside, never put back by "Make the invoice".
    for id in &change.removed {
        if let Some(at) = draft.lines.iter().position(|l| l.id == *id) {
            let gone = draft.lines.remove(at);
            for key in gone.sessions {
                if !draft.set_aside.contains(&key) {
                    draft.set_aside.push(key);
                }
            }
        }
    }
    // Lines changed, by id, only in the fields changed; new lines at the end.
    for edit in &change.lines {
        match &edit.id {
            Some(id) => match draft.lines.iter_mut().find(|l| l.id == *id) {
                Some(line) => edit_line(line, edit, cx),
                None => said.push(Said::LineGone),
            },
            None => {
                let mut line = Line { quantity: 1000, unit: "C62".into(), base: 1, ..Line::of_time("", 0, 0, "", 0) };
                edit_line(&mut line, edit, cx);
                draft.lines.push(line);
            }
        }
    }
    life::line_ids(draft);
    // A deposit of another client or currency is no longer deducted (the review of phase 3, F4).
    if (draft.client_ref.clone(), draft.currency.clone()) != before {
        let left = foreign_deposits(draft, cx.records);
        if !left.is_empty() {
            draft.deposits.retain(|c| !left.contains(&c.number));
            said.push(Said::DepositsLeft(left));
        }
    }
    if let Some(numbers) = &change.deposits
        && draft.kind == Kind::Invoice
    {
        // Each deposit of the same client and currency, for what is left of it after its credit notes.
        let open = life::deposits_open(draft, cx.records);
        draft.deposits = numbers
            .iter()
            .filter_map(|n| open.iter().find(|d| d.number == *n))
            .map(|d| Cited { number: d.number.clone(), date: d.date.clone(), amount: life::deposit_left(d, cx.records) })
            .collect();
    }
    said
}

/// A change saved: the draft as kept, whether it had changed elsewhere
/// since the editor read it (that change kept, this one merged into it),
/// and what the change found to say.
#[derive(Debug, Clone)]
pub struct Saved {
    pub draft: Invoice,
    pub elsewhere: bool,
    pub said: Vec<Said>,
}

/// `change` merged into the draft `id` of `dir` as it is now, and kept.
pub fn save(dir: &Path, id: &str, change: &Change, cx: &Context) -> Result<Saved, String> {
    let mut said = Vec::new();
    let mut elsewhere = false;
    // The revision compared under the lock the draft is read and written with (the second review of phase 3, R4).
    let draft = life::change_draft_read(dir, id, cx.settings, cx.today, |d, now| {
        elsewhere = change.revision.as_deref().is_some_and(|read| !read.is_empty() && read != now);
        said = apply(d, change, cx);
    })?;
    Ok(Saved { draft, elsewhere, said })
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

    fn settings(scheme: &str) -> InvoiceSettings {
        InvoiceSettings {
            preset: "fr".into(),
            legal_form: "ei".into(),
            regime: "micro".into(),
            name: "Camille Exemple".into(),
            address: "2 place de l’Exemple".into(),
            postcode: "69001".into(),
            city: "Lyon".into(),
            ids: vec![IdSetting { kind: "siret".into(), value: "732 829 320 00017".into(), printed: true }],
            tax: vec![TaxSetting { from: String::new(), scheme: scheme.into(), mention: String::new() }],
            rate: 60.0,
            ..InvoiceSettings::default()
        }
    }

    fn client(name: &str) -> crate::invoice::Client {
        crate::invoice::Client { name: name.into(), address: crate::invoice::Address { lines: vec!["1 rue de l’Exemple".into()], postcode: "75002".into(), city: "Paris".into(), country: "FR".into() }, ..Default::default() }
    }

    fn draft_for(s: &InvoiceSettings, name: &str, today: Date) -> Invoice {
        life::blank(s, client(name), name, "en", "EUR", crate::invoice::Terms { days: 30, end_of_month: false }, today)
    }

    fn edit(id: &str) -> LineEdit {
        LineEdit { id: Some(id.into()), ..LineEdit::default() }
    }

    /// The editor's numbers made integers at the boundary; lines named by
    /// their ids, each field changed alone; a line of time keeps its minutes
    /// and sessions; a line taken out leaves its time aside.
    #[test]
    fn the_editor_s_lines_become_the_record_s() {
        let s = settings("liable");
        let today = day("2026-11-17");
        let clients = crate::clients::Clients::default();
        let cx = Context { settings: &s, records: &[], clients: &clients, contacts: &[], fallback: "en", today };
        let mut draft = draft_for(&s, "Studio Lumen", today);
        draft.lines.push(Line { sessions: vec!["s1".into()], ..Line::of_time("Build the site", 90, 6000, "S", 2000) });
        draft.lines.push(Line { sessions: vec!["s2".into()], ..Line::of_time("Call", 30, 6000, "S", 2000) });
        life::line_ids(&mut draft);
        let (time, call) = (draft.lines[0].id.clone(), draft.lines[1].id.clone());
        let change = Change {
            lines: vec![
                LineEdit { description: Some("Build the site, first part".into()), quantity: Some(99.0), unit: Some("h".into()), price: Some(70.0), ..edit(&time) },
                LineEdit { description: Some("Poster".into()), quantity: Some(2.5), unit: Some("pcs".into()), price: Some(12.3), discount: Some(1.0), nature: Some("goods".into()), ..LineEdit::default() },
                LineEdit { description: Some("Book".into()), quantity: Some(1.0), unit: Some("C62".into()), price: Some(20.0), nature: Some("goods".into()), own_tax: Some(true), category: Some("S".into()), rate: Some(5.5), ..LineEdit::default() },
            ],
            removed: vec![call],
            period_from: Some("10/11/2026".into()),
            period_to: Some("2026-11-02".into()),
            notes: Some(vec!["Thank you.".into(), " ".into()]),
            ..Change::default()
        };
        apply(&mut draft, &change, &cx);
        life::refresh(&mut draft, &s, today);
        let line = &draft.lines[0];
        assert_eq!((line.id.as_str(), line.description.as_str(), line.minutes(), line.price, line.sessions.len(), line.amount), (time.as_str(), "Build the site, first part", Some(90), 7000, 1, 10500), "its time kept, its price changed");
        let poster = &draft.lines[1];
        assert_eq!((poster.quantity, poster.unit.as_str(), poster.price, poster.discount, poster.amount, poster.nature), (2500, "C62", 1230, 100, 2975, Nature::Goods));
        assert!(!poster.id.is_empty(), "a new line takes an id of its own");
        assert_eq!((poster.category.as_str(), poster.rate), ("S", 2000), "placed by the preset");
        assert_eq!((draft.lines[2].rate, draft.lines[2].own_tax), (550, true), "its own rate kept");
        assert_eq!(draft.lines.len(), 3, "the call taken out");
        assert_eq!(draft.set_aside, ["s2"], "its time left aside");
        assert_eq!((draft.period_from.as_str(), draft.period_to.as_str(), draft.period_default), ("2026-11-02", "2026-11-10", false), "the period in its order, a day as people write it");
        assert_eq!(draft.notes.iter().filter(|n| n.legal.is_empty()).map(|n| n.text.as_str()).collect::<Vec<_>>(), ["Thank you."]);
        // Both days taken out: the day of issue again, said by Sioul.
        apply(&mut draft, &Change { period_from: Some(String::new()), period_to: Some(String::new()), ..Change::default() }, &cx);
        life::refresh(&mut draft, &s, today);
        assert_eq!((draft.period_from.as_str(), draft.period_default), ("2026-11-17", true));
        // A deposit invoice: no period on it.
        apply(&mut draft, &Change { kind: Some("deposit".into()), ..Change::default() }, &cx);
        life::refresh(&mut draft, &s, today);
        assert_eq!((draft.kind, draft.period_from.as_str()), (Kind::Deposit, ""));
    }

    /// An editor left open sends what it changed by the lines' ids, never
    /// their places: the lines another device changed meanwhile stay, a line
    /// of time keeps its time and its price, a line taken out elsewhere is
    /// said, and the change made elsewhere is said (the review of phase 3, F2).
    #[test]
    fn a_change_made_elsewhere_is_never_written_over() {
        let dir = std::env::temp_dir().join(format!("sioul-edit-elsewhere-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = settings("liable");
        let today = day("2026-11-17");
        let clients = crate::clients::Clients::default();
        let cx = Context { settings: &s, records: &[], clients: &clients, contacts: &[], fallback: "en", today };
        let mut draft = draft_for(&s, "Studio Lumen", today);
        draft.lines.push(Line { quantity: 1000, unit: "C62".into(), price: 50_000, base: 1, ..Line::of_time("Design", 0, 0, "S", 2000) });
        draft.lines.push(Line { sessions: vec!["s1".into()], ..Line::of_time("Build the site", 90, 6000, "S", 2000) });
        life::refresh(&mut draft, &s, today);
        let draft = crate::invoice::save_draft(&dir, &draft).unwrap();
        let (design, time) = (draft.lines[0].id.clone(), draft.lines[1].id.clone());
        let read = crate::invoice::draft_revision(&dir, &draft.draft);
        // Elsewhere meanwhile: Design taken out, Travel added.
        let travel = LineEdit { description: Some("Travel".into()), quantity: Some(1.0), unit: Some("C62".into()), price: Some(120.0), ..LineEdit::default() };
        save(&dir, &draft.draft, &Change { removed: vec![design.clone()], lines: vec![travel], ..Change::default() }, &cx).unwrap();
        // Here, the editor as it read the draft: Design's price, the time's words.
        let late = Change { revision: Some(read), lines: vec![LineEdit { price: Some(600.0), ..edit(&design) }, LineEdit { description: Some("Build the site, all of it".into()), ..edit(&time) }], ..Change::default() };
        let saved = save(&dir, &draft.draft, &late, &cx).unwrap();
        assert!(saved.elsewhere, "said: the draft changed elsewhere");
        assert_eq!(saved.said, [Said::LineGone], "Design was taken out elsewhere: its price dropped");
        let lines: Vec<(&str, Option<u32>, i64, usize)> = saved.draft.lines.iter().map(|l| (l.description.as_str(), l.minutes(), l.amount, l.sessions.len())).collect();
        assert_eq!(lines, [("Build the site, all of it", Some(90), 9000, 1), ("Travel", None, 12000, 0)], "the time kept with its line, Travel kept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// VAT at a rate chosen with no rate takes the preset's standard rate; a
    /// line with VAT under the franchise, or VAT at a rate with none, holds
    /// the issue back (the review of phase 3, F3, F19).
    #[test]
    fn a_line_s_own_tax_fits_the_scheme() {
        let today = day("2026-11-17");
        let clients = crate::clients::Clients::default();
        let liable = settings("liable");
        let cx = Context { settings: &liable, records: &[], clients: &clients, contacts: &[], fallback: "en", today };
        let mut draft = draft_for(&liable, "Studio Lumen", today);
        draft.lines.push(Line { quantity: 1000, unit: "C62".into(), price: 10_000, base: 1, ..Line::of_time("Consulting", 0, 0, "AE", 0) });
        life::refresh(&mut draft, &liable, today);
        let id = draft.lines[0].id.clone();
        apply(&mut draft, &Change { lines: vec![LineEdit { own_tax: Some(true), category: Some("S".into()), ..edit(&id) }], ..Change::default() }, &cx);
        life::refresh(&mut draft, &liable, today);
        assert_eq!((draft.lines[0].category.as_str(), draft.lines[0].rate), ("S", 2000), "the preset's standard rate, shown as it is");
        assert!(life::checks(&draft, &liable, today, &[]).iter().all(|c| !c.text.contains("Line 1")), "{:?}", life::checks(&draft, &liable, today, &[]));
        apply(&mut draft, &Change { lines: vec![LineEdit { rate: Some(0.0), ..edit(&id) }], ..Change::default() }, &cx);
        life::refresh(&mut draft, &liable, today);
        assert!(life::checks(&draft, &liable, today, &[]).iter().any(|c| c.required && c.text.contains("rate")), "VAT at a rate needs its rate");
        // The franchise from 1 December: VAT on a line held back on that day, as people write the day.
        let mut franchise = settings("liable");
        franchise.tax.push(TaxSetting { from: "01/12/2026".into(), scheme: "franchise".into(), mention: String::new() });
        apply(&mut draft, &Change { lines: vec![LineEdit { rate: Some(20.0), ..edit(&id) }], ..Change::default() }, &cx);
        assert!(life::checks(&draft, &franchise, today, &[]).iter().all(|c| !c.text.contains("Line 1")), "liable on 17 November");
        assert!(life::checks(&draft, &franchise, day("2026-12-02"), &[]).iter().any(|c| c.required && c.text.contains("VAT")), "VAT under the franchise held back");
        // A day that does not read is said, never ignored.
        franchise.tax[1].from = "1 December".into();
        assert!(life::checks(&draft, &franchise, today, &[]).iter().any(|c| c.required && c.text.contains("1 December")));
        // Elsewhere has no rates: the rate typed is kept.
        let elsewhere = InvoiceSettings { preset: "elsewhere".into(), tax: vec![TaxSetting { from: String::new(), scheme: "liable".into(), mention: String::new() }], ..settings("liable") };
        let cx = Context { settings: &elsewhere, records: &[], clients: &clients, contacts: &[], fallback: "en", today };
        apply(&mut draft, &Change { lines: vec![LineEdit { own_tax: Some(true), category: Some("S".into()), rate: Some(13.0), ..edit(&id) }], ..Change::default() }, &cx);
        assert_eq!(draft.lines[0].rate, 1300);
    }

    /// A final invoice deducts what is left of each deposit of its client
    /// and currency after its credit notes, ticked from the deposits open;
    /// another client's or currency's is untied when the draft changes
    /// client or currency, and said (the review of phase 3, F4, F10).
    #[test]
    fn a_final_invoice_deducts_what_is_left_of_its_deposits() {
        let s = settings("franchise");
        let today = day("2026-11-17");
        let issued = |number: &str, kind: Kind, client: &str, currency: &str, total: i64| Invoice { kind, number: number.into(), date: "2026-10-01".into(), currency: currency.into(), client_ref: client.into(), client: self::client(client), totals: crate::invoice::Totals { with_tax: total, due: total, ..Default::default() }, ..Invoice::default() };
        let credit = Invoice { corrects: Some(Cited { number: "2026-010".into(), date: "2026-10-01".into(), amount: 30_000 }), ..issued("2026-012", Kind::CreditNote, "Studio Lumen", "EUR", 10_000) };
        let records = vec![issued("2026-010", Kind::Deposit, "Studio Lumen", "EUR", 30_000), issued("2026-011", Kind::Deposit, "Silkworks Studio", "EUR", 50_000), issued("2026-013", Kind::Deposit, "Studio Lumen", "USD", 20_000), credit];
        let clients = crate::clients::Clients::default();
        let cx = Context { settings: &s, records: &records, clients: &clients, contacts: &[], fallback: "en", today };
        let mut draft = draft_for(&s, "Studio Lumen", today);
        draft.lines.push(Line { quantity: 1000, unit: "C62".into(), price: 100_000, base: 1, ..Line::of_time("Site", 0, 0, "E", 0) });
        assert_eq!(life::deposits_open(&draft, &records).iter().map(|d| d.number.as_str()).collect::<Vec<_>>(), ["2026-010"], "its client, its currency");
        assert!(life::checks(&draft, &s, today, &records).iter().any(|c| !c.required && c.text.contains("2026-010")), "a deposit not deducted is said");
        apply(&mut draft, &Change { deposits: Some(vec!["2026-010".into(), "2026-011".into(), "2026-013".into()]), ..Change::default() }, &cx);
        life::refresh(&mut draft, &s, today);
        assert_eq!(draft.deposits.iter().map(|d| (d.number.as_str(), d.amount)).collect::<Vec<_>>(), [("2026-010", 20_000)], "what is left after its credit note");
        assert_eq!(draft.totals.due, 80_000);
        // Another client: the deposit untied, said.
        let said = apply(&mut draft, &Change { client: Some("Silkworks Studio".into()), ..Change::default() }, &cx);
        assert_eq!(said, [Said::DepositsLeft(vec!["2026-010".into()])]);
        assert!(draft.deposits.is_empty());
        assert_eq!(deposit_amount(0.0, 30.0, 100_000), 30_000, "a share of the total");
        assert_eq!(deposit_amount(250.5, 0.0, 100_000), 25_050, "an amount");
    }
}

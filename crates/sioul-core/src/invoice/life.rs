// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The life of an invoice (docs/projects.md, "Invoices"): a draft made from
//! a project's unbilled time, for a client, or blank; changed freely; checked
//! against its country's preset, in plain words, what is required blocking
//! the issue and what is recommended not; issued all or nothing (its number,
//! its issuer and client frozen, its record and the series journal, its
//! time marked, its project's budget line), a step that fails undoing the
//! ones before; answered by a credit note that cites it; preceded by
//! deposit invoices that a final invoice deducts; paid by payments, said by
//! hand or found by the bank's matching.
//!
//! Legal form, income regime and VAT scheme are three settings of their own
//! (`issuer_on`): « EI » is printed for a French entrepreneur individuel
//! whatever the VAT; the franchise's mention under the franchise whatever
//! the form.

use super::presets::{self, Level, Preset, Who};
use super::record::{Address, Bank, Cited, Client, FORMAT, Identifier, Invoice, Issuer, Kind, Line, Nature, Note, Payment, Terms, rounded};
use super::series::{self, Journal, Pattern, Reset};
use super::totals;
use crate::config::InvoiceSettings;
use crate::i18n::Translator;
use crate::projects::Project;
use crate::timereport::Entry;
use jiff::civil::Date;
use std::collections::BTreeMap;
use std::path::Path;

/// A table from before November 2026 that means France under the
/// franchise: no preset chosen, its VAT line citing article 293 B.
fn older_french(s: &InvoiceSettings) -> bool {
    s.preset.trim().is_empty() && s.vat.split_whitespace().collect::<String>().to_uppercase().contains("293B")
}

/// The preset the settings name, or the one an older table means: France
/// for a VAT line citing 293 B, "elsewhere" otherwise.
pub fn preset_of(s: &InvoiceSettings) -> &'static Preset {
    if older_french(s) { presets::get("fr") } else { presets::get(s.preset.trim()) }
}

/// The legal form: as set; else the preset's first (in France, an EI), a
/// sole trader being who most often bills alone; none for "elsewhere".
pub fn legal_form_of(s: &InvoiceSettings) -> String {
    if !s.legal_form.trim().is_empty() {
        s.legal_form.trim().to_string()
    } else {
        preset_of(s).legal_form.first().map(|f| f.id.clone()).unwrap_or_default()
    }
}

/// The income regime for the books: as set; an older French table's, micro.
pub fn regime_of(s: &InvoiceSettings) -> String {
    if !s.regime.trim().is_empty() {
        s.regime.trim().to_string()
    } else if older_french(s) {
        "micro".into()
    } else {
        String::new()
    }
}

/// The VAT scheme on `day` and the franchise's mention: the setting in
/// force that day; without one, the preset's first scheme, an older table's
/// own VAT line kept word for word as the mention.
pub fn scheme_on(s: &InvoiceSettings, day: Date) -> (String, String) {
    let preset = preset_of(s);
    let set = s.tax.iter().filter(|t| t.from.parse::<Date>().map_or(t.from.trim().is_empty(), |from| from <= day)).max_by_key(|t| t.from.parse::<Date>().unwrap_or(Date::MIN));
    let scheme = match set {
        Some(t) => t.scheme.trim().to_string(),
        None if older_french(s) => "franchise".into(),
        None if s.preset.trim().is_empty() => "none".into(),
        None => preset.tax.schemes.first().cloned().unwrap_or_else(|| "none".into()),
    };
    if scheme != "franchise" {
        return (scheme, String::new());
    }
    let own = set.map(|t| t.mention.trim()).filter(|m| !m.is_empty()).or_else(|| Some(s.vat.trim()).filter(|m| !m.is_empty() && set.is_none()));
    let mention = own.map_or_else(|| preset.mention_on(day).to_string(), str::to_string);
    (scheme, mention)
}

/// The name as printed: followed by the legal form's mark (« EI ») unless it
/// says it already, at its end or its start, or in words.
pub fn marked_name(name: &str, mark: &str) -> String {
    let name = name.trim();
    if mark.is_empty() || name.is_empty() {
        return name.to_string();
    }
    let words: Vec<String> = name.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | ',' | '«' | '»' | '-')).filter(|w| !w.is_empty()).map(str::to_uppercase).collect();
    let said = words.last().is_some_and(|w| w == mark) || words.first().is_some_and(|w| w == mark) || name.to_lowercase().contains("entrepreneur individuel");
    if said { name.to_string() } else { format!("{name} {mark}") }
}

/// The numbering pattern of `kind`: the settings', else the older prefix's.
pub fn pattern_of(s: &InvoiceSettings, kind: Kind) -> Result<Pattern, String> {
    let reset = Reset::named(s.reset.trim());
    if kind == Kind::CreditNote && !s.credit_pattern.trim().is_empty() {
        return Pattern::parse(s.credit_pattern.trim(), reset);
    }
    if s.pattern.trim().is_empty() { Ok(Pattern::of_prefix(&s.prefix)) } else { Pattern::parse(s.pattern.trim(), reset) }
}

/// The issuer as the settings say it on `day`: its preset, legal form,
/// regime, name with its mark, address, identifiers (an older table's SIRET
/// among them), VAT scheme and mention, banks and payment words.
pub fn issuer_on(s: &InvoiceSettings, day: Date) -> Issuer {
    let preset = preset_of(s);
    let legal_form = legal_form_of(s);
    let mark = preset.form(&legal_form).map_or("", |f| f.mark.as_str());
    let (scheme, mention) = scheme_on(s, day);
    let mut ids: Vec<Identifier> = s.ids.iter().filter(|i| !i.value.trim().is_empty()).map(|i| Identifier { kind: i.kind.trim().to_string(), value: i.value.trim().to_string(), printed: i.printed }).collect();
    if ids.is_empty() && !s.siret.trim().is_empty() {
        let digits = super::ids::compact(&s.siret);
        let kind = if preset.id == "fr" && digits.len() == 9 { "siren" } else { "siret" };
        ids.push(Identifier { kind: kind.into(), value: s.siret.trim().to_string(), printed: true });
    }
    let country = if s.country.trim().is_empty() { preset.country.clone() } else { s.country.trim().to_uppercase() };
    Issuer {
        preset: preset.id.clone(),
        legal_form,
        regime: regime_of(s),
        name: marked_name(&s.name, mark),
        trade_name: s.trade_name.trim().to_string(),
        address: Address { country, postcode: s.postcode.trim().to_string(), city: s.city.trim().to_string(), ..Address::from_text(&s.address) },
        ids,
        scheme,
        mention,
        options: s.options.clone(),
        capital: s.capital.trim().to_string(),
        banks: s.banks.iter().filter(|b| !b.iban.trim().is_empty()).map(|b| Bank { holder: b.holder.trim().to_string(), iban: b.iban.trim().to_string(), bic: b.bic.trim().to_string() }).collect(),
        payment: s.payment.trim().to_string(),
        insurance: s.insurance.trim().to_string(),
        email: s.email.trim().to_string(),
    }
}

/// The currency of a new invoice: the client's, the settings', the preset's, euros.
pub fn currency_of(s: &InvoiceSettings, client_currency: &str) -> String {
    [client_currency, s.currency.as_str(), preset_of(s).currency.as_str(), "EUR"].into_iter().map(str::trim).find(|c| !c.is_empty()).unwrap_or("EUR").to_uppercase()
}

/// The late-payment rate printed on an invoice of `day`, in basis points:
/// the settings', else the preset's default; 0 when neither says one.
pub fn penalty_on(s: &InvoiceSettings, day: Date) -> i64 {
    match s.penalty_rate {
        Some(percent) if percent > 0.0 => (percent * 100.0).round() as i64,
        _ => preset_of(s).penalty_on(day).unwrap_or(0),
    }
}

/// The payment terms: the client's, the settings', the preset's, 30 days.
pub fn terms_of(s: &InvoiceSettings, client_days: Option<u32>, client_end_of_month: Option<bool>) -> Terms {
    Terms { days: client_days.or(s.terms_days).or(preset_of(s).terms_days).unwrap_or(30), end_of_month: client_end_of_month.unwrap_or(s.end_of_month) }
}

/// Where the client is, from the issuer: "domestic", "eu", "abroad".
fn zone_of(issuer: &Issuer, client: &Client) -> &'static str {
    let country = if client.address.country.trim().is_empty() { issuer.address.country.as_str() } else { client.address.country.as_str() };
    presets::zone(&issuer.address.country, country)
}

/// The category and rate a new line of `nature` takes for `client`: none
/// under no scheme ("O"); the client's reverse charge for a service to a
/// business in another member state; the franchise's ("E"); liable, the
/// preset's first rate, standard ("S").
pub fn default_tax(issuer: &Issuer, client: &Client, nature: Nature, day: Date) -> (String, i64) {
    let preset = presets::get(&issuer.preset);
    if let Some(reverse) = &preset.tax.reverse
        && client.is_business()
        && nature == Nature::Service
        && zone_of(issuer, client) == "eu"
        && issuer.scheme != "none"
    {
        return (reverse.category.clone(), 0);
    }
    match issuer.scheme.as_str() {
        "franchise" => (preset.tax.franchise.as_ref().map_or_else(|| "E".into(), |f| f.category.clone()), 0),
        "liable" => ("S".into(), preset.rates_on(day).first().map_or(0, |r| r.rate)),
        _ => ("O".into(), 0),
    }
}

/// The language of an invoice: the client's, the settings', else `fallback` (the window's).
pub fn language_of(s: &InvoiceSettings, client_language: &str, fallback: &str) -> String {
    let wanted = [client_language, s.language.as_str(), fallback].into_iter().map(str::trim).find(|l| !l.is_empty()).unwrap_or("en");
    Translator::new(wanted).language().to_string()
}

/// A new draft for `client`, nothing billed yet.
pub fn blank(s: &InvoiceSettings, client: Client, client_ref: &str, language: &str, currency: &str, terms: Terms, today: Date) -> Invoice {
    let mut draft = Invoice {
        format: FORMAT,
        kind: Kind::Invoice,
        currency: currency.to_string(),
        language: language.to_string(),
        client_ref: client_ref.to_string(),
        client,
        terms,
        ..Invoice::default()
    };
    refresh(&mut draft, s, today);
    draft
}

/// A new draft for the client `reference` names (`clients::resolve`: its
/// profile, its contact, or a name): the profile's language, currency,
/// terms and purchase order, else the settings'; `fallback` is the
/// window's language.
pub fn for_client(s: &InvoiceSettings, reference: &str, clients: &crate::clients::Clients, contacts: &[crate::contacts::Contact], fallback: &str, today: Date) -> Invoice {
    let (client, profile) = crate::clients::resolve(reference, clients, contacts);
    let language = language_of(s, profile.map_or("", |p| p.language.as_str()), fallback);
    let currency = currency_of(s, profile.map_or("", |p| p.currency.as_str()));
    let terms = terms_of(s, profile.and_then(|p| p.terms_days), profile.and_then(|p| p.end_of_month));
    let mut draft = blank(s, client, reference, &language, &currency, terms, today);
    draft.order = profile.map(|p| p.order.clone()).unwrap_or_default();
    draft
}

/// A draft of a project's billable time not billed yet, among `entries`: a
/// line per task or note, its time at the project's rate (the settings'
/// when it has none), in the order first worked on; the service's period
/// from the first and last days billed. None when nothing waits to be billed.
pub fn from_time(entries: &[Entry], project: &Project, mut draft: Invoice, s: &InvoiceSettings, today: Date) -> Option<Invoice> {
    let billed: Vec<&Entry> = entries.iter().filter(|e| e.project == project.id && e.billable && e.invoice.is_empty() && e.minutes > 0).collect();
    if billed.is_empty() {
        return None;
    }
    // The project's rate, a float in its file, made cents here at the boundary.
    let price = (project.rate.unwrap_or(s.rate) * 100.0).round() as i64;
    let issuer = issuer_on(s, today);
    let (category, rate) = default_tax(&issuer, &draft.client, Nature::Service, today);
    let mut grouped: Vec<(i64, String, u32, Vec<String>)> = Vec::new();
    for entry in &billed {
        let label = if entry.title.trim().is_empty() { project.title.clone() } else { entry.title.clone() };
        match grouped.iter_mut().find(|(_, l, _, _)| *l == label) {
            Some((first, _, minutes, keys)) => {
                *first = (*first).min(entry.start);
                *minutes += entry.minutes;
                keys.push(entry.key.clone());
            }
            None => grouped.push((entry.start, label, entry.minutes, vec![entry.key.clone()])),
        }
    }
    grouped.sort_by_key(|(first, ..)| *first);
    draft.lines.extend(grouped.into_iter().map(|(_, label, minutes, keys)| Line { sessions: keys, ..Line::of_time(&label, minutes, price, &category, rate) }));
    draft.period_from = billed.iter().map(|e| e.day).min().map(|d| d.to_string()).unwrap_or_default();
    draft.period_to = billed.iter().map(|e| e.day).max().map(|d| d.to_string()).unwrap_or_default();
    draft.project = project.id.clone();
    draft.project_title = project.title.clone();
    refresh(&mut draft, s, today);
    Some(draft)
}

/// A draft credit note for `invoice`: all its lines to cancel it, or the
/// ones at `chosen` to correct some; it cites the invoice's number and day.
pub fn credit(invoice: &Invoice, chosen: Option<&[usize]>, s: &InvoiceSettings, today: Date) -> Invoice {
    // Each line as it was billed, its tax with it: an exempt line keeps the
    // reason the invoice printed (the franchise's mention), should the
    // scheme have changed since.
    let reason_of = |category: &str| invoice.totals.breakdown.iter().find(|b| b.category == category).map(|b| b.reason.clone()).unwrap_or_default();
    let lines: Vec<Line> = invoice
        .lines
        .iter()
        .enumerate()
        .filter(|(i, _)| chosen.is_none_or(|c| c.contains(i)))
        .map(|(_, l)| {
            let mut line = l.clone();
            if line.category == "E" && line.exemption.trim().is_empty() {
                line.exemption = reason_of("E");
            }
            line
        })
        .collect();
    let mut draft = Invoice {
        format: FORMAT,
        kind: Kind::CreditNote,
        currency: invoice.currency.clone(),
        language: invoice.language.clone(),
        project: invoice.project.clone(),
        project_title: invoice.project_title.clone(),
        client_ref: invoice.client_ref.clone(),
        client: invoice.client.clone(),
        lines,
        period_from: invoice.period_from.clone(),
        period_to: invoice.period_to.clone(),
        corrects: Some(Cited { number: invoice.number.clone(), date: invoice.date.clone(), amount: invoice.totals.due }),
        order: invoice.order.clone(),
        terms: Terms { days: 0, end_of_month: false },
        ..Invoice::default()
    };
    // A format-1 record's time is its own, not its lines': all of it with all its lines.
    if chosen.is_none()
        && let Some(older) = &invoice.older
        && let Some(first) = draft.lines.first_mut()
    {
        first.sessions = older.sessions.clone();
    }
    refresh(&mut draft, s, today);
    draft
}

/// A draft deposit invoice: `amount` excluding tax, in cents, for what `description` says.
pub fn deposit(s: &InvoiceSettings, mut draft: Invoice, description: &str, amount: i64, today: Date) -> Invoice {
    draft.kind = Kind::Deposit;
    let issuer = issuer_on(s, today);
    let (category, rate) = default_tax(&issuer, &draft.client, Nature::Service, today);
    draft.lines = vec![Line { quantity: 1000, unit: "C62".into(), price: amount, base: 1, ..Line::of_time(description, 0, 0, &category, rate) }];
    refresh(&mut draft, s, today);
    draft
}

/// A final invoice deducts the deposit invoices it cites, with tax.
pub fn cite_deposits(draft: &mut Invoice, deposits: &[Invoice], s: &InvoiceSettings, today: Date) {
    draft.deposits = deposits.iter().filter(|d| d.kind == Kind::Deposit && !d.number.is_empty()).map(|d| Cited { number: d.number.clone(), date: d.date.clone(), amount: d.totals.with_tax }).collect();
    refresh(draft, s, today);
}

/// A draft made as it would be issued on `day`: the issuer as the settings
/// say then (frozen at issue), each line's tax as the scheme asks (no VAT
/// is ever shown under the franchise or without a scheme), the legal lines
/// the preset prints for this client, the late-payment rate, the due day
/// and the totals. The person's own notes stay.
pub fn refresh(draft: &mut Invoice, s: &InvoiceSettings, day: Date) {
    draft.format = FORMAT;
    draft.issuer = issuer_on(s, day);
    // A credit note keeps the tax of the lines it credits, whatever the scheme is now.
    for line in draft.lines.iter_mut().filter(|_| draft.kind != Kind::CreditNote) {
        let (category, rate) = default_tax(&draft.issuer, &draft.client, line.nature, day);
        if draft.issuer.scheme != "liable" || category == "AE" {
            // No VAT is ever shown under the franchise or without a scheme
            // (art. 293 E CGI: VAT shown is VAT owed); the client's reverse charge.
            line.category = category;
            line.rate = rate;
        } else {
            match line.category.as_str() {
                // A rate of the person's own, kept.
                "S" if line.rate > 0 => {}
                // Zero-rated, an EU delivery, an export: the person's choice.
                "Z" | "K" | "G" => {}
                // Exempt with its reason written: kept.
                "E" | "O" if !line.exemption.trim().is_empty() => {}
                // Left from a franchise, a reverse charge no longer due, no rate yet: the standard one.
                _ => {
                    line.category = category;
                    line.rate = rate;
                }
            }
        }
    }
    let preset = presets::get(&draft.issuer.preset);
    draft.penalty_rate = if draft.client.is_business() && draft.kind != Kind::CreditNote { penalty_on(s, day) } else { 0 };
    draft.notes = legal_notes(draft, s, preset).into_iter().chain(draft.notes.iter().filter(|n| n.legal.is_empty()).cloned()).collect();
    if !draft.number.is_empty() || !draft.date.is_empty() {
        let issued: Date = draft.date.parse().unwrap_or(day);
        draft.due = draft.terms.due(issued).to_string();
    } else {
        draft.due = draft.terms.due(day).to_string();
    }
    let issuer = draft.issuer.clone();
    let language = draft.language.clone();
    let reverse = preset.tax.reverse.clone();
    let franchise_code = preset.tax.franchise.as_ref().map(|f| f.reason_code.clone()).unwrap_or_default();
    let reasons = move |category: &str, lines: &[&Line]| -> (String, String) {
        match category {
            "AE" => reverse.as_ref().map_or_else(|| ("Reverse charge".into(), "VATEX-EU-AE".into()), |r| (r.mention.get(&language).to_string(), r.reason_code.clone())),
            "E" if issuer.scheme == "franchise" => (issuer.mention.clone(), franchise_code.clone()),
            "E" => (lines.iter().map(|l| l.exemption.trim()).find(|e| !e.is_empty()).unwrap_or("").to_string(), String::new()),
            _ => (String::new(), String::new()),
        }
    };
    let deposits: i64 = draft.deposits.iter().map(|d| d.amount).sum();
    draft.totals = totals::compute(&mut draft.lines, deposits, &reasons);
}

/// The preset's legal lines this invoice prints, changed or taken out by
/// the settings (`[[invoice.line]]`), the person's own lines after them; an
/// older table's late-payment sentence kept on an "elsewhere" invoice.
fn legal_notes(draft: &Invoice, s: &InvoiceSettings, preset: &Preset) -> Vec<Note> {
    let tr = Translator::new(if draft.language.is_empty() { "en" } else { &draft.language });
    let language = tr.language().to_string();
    let issuer = &draft.issuer;
    let form = preset.form(&issuer.legal_form);
    let company = form.is_some_and(|f| f.company);
    let payment_line = |subject: &str| matches!(subject, "AAB" | "PMD" | "PMT");
    let mut out = Vec::new();
    for line in &preset.line {
        let for_client = line.client.is_empty() || line.client.contains(&draft.client.kind);
        let for_option = line.option.is_empty() || issuer.options.contains(&line.option);
        let for_scheme = line.scheme.is_empty() || line.scheme.contains(&issuer.scheme);
        let for_form = line.form.is_empty() || line.form.iter().any(|f| f == &issuer.legal_form || (f == "company" && company));
        if !(for_client && for_option && for_scheme && for_form) || (draft.kind == Kind::CreditNote && payment_line(&line.subject)) {
            continue;
        }
        let own = s.lines.iter().find(|l| l.id == line.id);
        if own.is_some_and(|l| l.off) {
            continue;
        }
        let template = own.map(|l| l.text.trim()).filter(|t| !t.is_empty()).unwrap_or_else(|| line.text.get(&language));
        let rate = if draft.penalty_rate > 0 {
            let percent = tr.percent_bp(draft.penalty_rate);
            let default = s.penalty_rate.is_none_or(|p| p <= 0.0);
            let note = preset.penalty.as_ref().map(|p| p.default_note.get(&language)).unwrap_or("");
            if default && !note.is_empty() { format!("{percent} ({note})") } else { percent }
        } else {
            String::new()
        };
        if template.contains("{ $rate }") && rate.is_empty() {
            continue;
        }
        let capital = if issuer.capital.is_empty() { String::new() } else { issuer.capital.clone() };
        if template.contains("{ $capital }") && capital.is_empty() {
            continue;
        }
        let text = template.replace("{ $rate }", &rate).replace("{ $capital }", &capital).replace("{ $form }", form.map_or("", |f| f.name.get(&language)));
        out.push(Note { text, subject: own.map(|l| l.subject.trim()).filter(|t| !t.is_empty()).unwrap_or(&line.subject).to_string(), legal: line.id.clone() });
    }
    // Your own lines, in Settings, after the preset's.
    for own in s.lines.iter().filter(|l| l.id.trim().is_empty() && !l.off && !l.text.trim().is_empty()) {
        out.push(Note { text: own.text.trim().to_string(), subject: own.subject.trim().to_string(), legal: "own".into() });
    }
    // An older table's lines, as they were printed: its VAT line (when not
    // the franchise's mention) and the late-payment sentence.
    if s.preset.trim().is_empty() && !older_french(s) {
        if !s.vat.trim().is_empty() {
            out.push(Note { text: s.vat.trim().to_string(), subject: String::new(), legal: "older-vat".into() });
        }
        if !s.lines.iter().any(|l| l.id == "older-late" && l.off) && draft.kind != Kind::CreditNote {
            out.push(Note { text: tr.text("invoice-late", None), subject: String::new(), legal: "older-late".into() });
        }
    }
    out
}

/// A draft of `dir` changed by `change` (its lines, its client, its dates,
/// its notes), then made again as it would be issued on `day` (`refresh`)
/// and kept; an issued invoice is never changed: a credit note answers it.
pub fn change_draft(dir: &Path, id: &str, s: &InvoiceSettings, day: Date, change: impl FnOnce(&mut Invoice)) -> Result<Invoice, String> {
    let mut draft = super::draft_in(dir, id).ok_or_else(|| format!("{id}: no such draft here"))?;
    change(&mut draft);
    draft.number.clear();
    draft.draft = id.to_string();
    refresh(&mut draft, s, day);
    super::save_draft(dir, &draft)
}

/// What a check found: required (issue waits) or recommended (said only).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Check {
    pub required: bool,
    pub text: String,
}

/// What the preset asks of this draft and it lacks, in plain words, as it
/// would be issued on `day`: required items first.
pub fn checks(draft: &Invoice, s: &InvoiceSettings, day: Date) -> Vec<Check> {
    let tr = Translator::new(if draft.language.is_empty() { "en" } else { &draft.language });
    let language = tr.language().to_string();
    let issuer = issuer_on(s, day);
    let preset = presets::get(&issuer.preset);
    let mut out: Vec<Check> = Vec::new();
    let mut say = |required: bool, text: String| {
        if !out.iter().any(|c| c.text == text) {
            out.push(Check { required, text });
        }
    };
    let words = |id: &str| tr.text(id, None);
    if !preset.supported {
        say(true, preset.why_not.get(&language).to_string());
    }
    if let Err(e) = pattern_of(s, draft.kind) {
        say(true, e);
    }
    if draft.lines.is_empty() {
        say(true, words("invoice-check-no-lines"));
    }
    if draft.lines.iter().any(|l| l.description.trim().is_empty()) {
        say(true, words("invoice-check-line-unnamed"));
    }
    if draft.kind == Kind::CreditNote && draft.corrects.as_ref().is_none_or(|c| c.number.is_empty()) {
        say(true, words("invoice-check-credit-cites"));
    }
    if draft.currency.trim().len() != 3 {
        say(true, words("invoice-check-currency"));
    }
    let company = preset.form(&issuer.legal_form).is_some_and(|f| f.company);
    let zone = zone_of(&issuer, &draft.client);
    let natures: Vec<&str> = draft.lines.iter().map(|l| if l.nature == Nature::Goods { "goods" } else { "service" }).collect();
    for rule in &preset.require {
        let applies = (rule.client.is_empty() || rule.client.contains(&draft.client.kind))
            && (rule.zone.is_empty() || rule.zone.iter().any(|z| z == zone))
            && (rule.nature.is_empty() || rule.nature.iter().any(|n| natures.contains(&n.as_str())))
            && (rule.scheme.is_empty() || rule.scheme.contains(&issuer.scheme))
            && (rule.form.is_empty() || rule.form.iter().any(|f| f == &issuer.legal_form || (f == "company" && company)))
            && rule.from.is_none_or(|from| day >= from)
            && rule.until.is_none_or(|until| day < until);
        if !applies {
            continue;
        }
        let ids = match rule.who {
            Who::Issuer => &issuer.ids,
            Who::Client => &draft.client.ids,
        };
        let has = |kind: &str| ids.iter().any(|i| i.kind == kind && !i.value.trim().is_empty());
        let met = if !rule.any.is_empty() {
            rule.any.iter().any(|k| has(k))
        } else {
            match (rule.who, rule.field.as_str()) {
                (Who::Issuer, "name") => !issuer.name.trim().is_empty(),
                (Who::Issuer, "address") => !issuer.address.is_empty(),
                (Who::Issuer, "mention") => !issuer.mention.trim().is_empty(),
                (Who::Issuer, "capital") => !issuer.capital.trim().is_empty(),
                (Who::Issuer, "currency-when-taxed") => draft.totals.tax == 0 || preset.currency.is_empty() || draft.currency.eq_ignore_ascii_case(&preset.currency),
                (Who::Client, "name") => !draft.client.name.trim().is_empty(),
                (Who::Client, "address") => !draft.client.address.is_empty(),
                _ => true,
            }
        };
        if !met {
            say(rule.level == Level::Required, rule.why.get(&language).to_string());
        }
    }
    // Identifiers given are checked by their keys (a SIREN by Luhn, a French VAT number by its key).
    for (who, ids) in [("issuer", &issuer.ids), ("client", &draft.client.ids)] {
        for id in ids.iter().filter(|i| !i.value.trim().is_empty()) {
            let check = super::ids::Check::named(preset.id_kind(&id.kind).map_or("", |k| k.check.as_str()));
            if !super::ids::valid(check, &id.value) {
                let name = preset.id_kind(&id.kind).map_or_else(|| id.kind.to_uppercase(), |k| k.name.get(&language).to_string());
                let mut args = crate::i18n::args();
                args.set("name", name);
                args.set("value", id.value.clone());
                say(true, tr.text(if who == "issuer" { "invoice-check-own-id-wrong" } else { "invoice-check-client-id-wrong" }, Some(&args)));
            }
        }
    }
    if let Some(max) = preset.terms_max_days
        && draft.client.is_business()
        && draft.terms.days > max
    {
        let mut args = crate::i18n::args();
        args.set("days", i64::from(max));
        say(false, tr.text("invoice-check-terms-long", Some(&args)));
    }
    if let Some(floor) = preset.penalty_floor_on(day)
        && draft.penalty_rate > 0
        && draft.penalty_rate < floor
    {
        let mut args = crate::i18n::args();
        args.set("floor", tr.percent_bp(floor));
        say(false, tr.text("invoice-check-penalty-low", Some(&args)));
    }
    out.sort_by_key(|c| !c.required);
    out
}

/// What issuing needs of this device: whether it shares, whether its
/// `drafts` part (which carries invoices) is on, and every device's last
/// numbers (`sioul_sync::lease::Claim::series`).
#[derive(Debug, Clone, Default)]
pub struct Numbering {
    pub sharing: bool,
    pub drafts_shared: bool,
    pub claimed: BTreeMap<String, u64>,
}

/// Where issuing writes besides the invoices' folder: the time's folder
/// (its sessions marked), and the budget file with the budget the
/// project's invoices are expected in.
#[derive(Debug, Clone, Default)]
pub struct Steps<'a> {
    pub time: Option<&'a Path>,
    pub ledger: Option<(&'a Path, &'a str)>,
}

/// Issues the draft `id` of `dir` on `day`: checked first (what is required
/// and missing refuses it, in words); then, under the series journal's
/// lock, numbered as one more than the journal, the records and every
/// device's claim, its issuer frozen, kept under its number, the journal
/// written; its time marked (a credit note's cancelled time freed); its
/// budget line planned; the draft taken out. All or nothing: a step that
/// fails undoes the ones before it. `client`, when given, is the client
/// as its profile says now, frozen with it.
#[allow(clippy::too_many_arguments)]
pub fn issue(dir: &Path, id: &str, s: &InvoiceSettings, day: Date, numbering: &Numbering, client: Option<Client>, steps: &Steps, tr: &Translator) -> Result<Invoice, String> {
    // Sharing on, the `drafts` part off: the records do not travel, so the
    // other devices' numbers may not be known here.
    if numbering.sharing && !numbering.drafts_shared {
        return Err(tr.text("invoice-drafts-part-off", None));
    }
    let mut invoice = super::draft_in(dir, id).ok_or_else(|| tr.text("invoice-draft-gone", None))?;
    if let Some(client) = client {
        invoice.client = client;
    }
    invoice.date = day.to_string();
    refresh(&mut invoice, s, day);
    let blocking: Vec<String> = checks(&invoice, s, day).into_iter().filter(|c| c.required).map(|c| c.text).collect();
    if !blocking.is_empty() {
        return Err(blocking.join("\n"));
    }
    let pattern = pattern_of(s, invoice.kind)?;
    let time_folder = steps.time.map(Path::to_path_buf).unwrap_or_else(crate::timelog::folder);
    // Time billed meanwhile (on another device, by another draft) is never billed twice.
    let sessions = invoice.sessions();
    if invoice.kind != Kind::CreditNote && !sessions.is_empty() {
        let billed: Vec<String> = crate::timelog::sessions_in(&time_folder).into_iter().filter(|x| !x.invoice.is_empty() && sessions.contains(&x.key())).map(|x| x.invoice).collect();
        if let Some(number) = billed.first() {
            let mut args = crate::i18n::args();
            args.set("number", number.clone());
            return Err(tr.text("invoice-time-billed-meanwhile", Some(&args)));
        }
    }
    let records = super::all_in(dir);
    let freed = if invoice.kind == Kind::CreditNote { freed_sessions(&invoice, &records) } else { Vec::new() };
    // Numbered and kept under the journal's lock: one number at a time on this device.
    let journal_path = dir.join(series::JOURNAL);
    let before = crate::filelock::with_lock(&journal_path, || -> Result<Option<Journal>, String> {
        let existed = journal_path.exists();
        let journal = Journal::load(dir);
        let records = super::all_in(dir);
        let (number, n) = series::next(&pattern, day, &journal, &records, &numbering.claimed);
        invoice.number = number.clone();
        invoice.series = pattern.sequence(day);
        super::write_new(dir, &invoice)?;
        let mut after = journal.clone();
        after.series.insert(invoice.series.clone(), series::Last { last: n, number, day: day.to_string() });
        if let Err(e) = after.save(dir) {
            let _ = super::remove_issued(dir, &invoice.number);
            return Err(e);
        }
        Ok(existed.then_some(journal))
    })?;
    let undo_record = |also: &dyn Fn()| {
        also();
        crate::filelock::with_lock(&journal_path, || {
            let _ = super::remove_issued(dir, &invoice.number);
            let _ = match &before {
                Some(journal) => journal.save(dir),
                None => std::fs::remove_file(&journal_path).map_err(|e| e.to_string()),
            };
        });
    };
    // Its time marked, or a credit note's cancelled time freed.
    let number = invoice.number.clone();
    let marked = if invoice.kind == Kind::CreditNote {
        let corrected = invoice.corrects.as_ref().map(|c| c.number.clone()).unwrap_or_default();
        crate::timelog::update_in(&time_folder, &freed, |x| {
            if x.invoice == corrected {
                x.invoice.clear();
            }
        })
    } else {
        crate::timelog::update_in(&time_folder, &sessions, |x| x.invoice = number.clone())
    };
    let unmark = || {
        if invoice.kind == Kind::CreditNote {
            let corrected = invoice.corrects.as_ref().map(|c| c.number.clone()).unwrap_or_default();
            let _ = crate::timelog::update_in(&time_folder, &freed, |x| {
                if x.invoice.is_empty() {
                    x.invoice = corrected.clone();
                }
            });
        } else {
            let _ = crate::timelog::update_in(&time_folder, &sessions, |x| {
                if x.invoice == number {
                    x.invoice.clear();
                }
            });
        }
    };
    if let Err(e) = marked {
        undo_record(&unmark);
        return Err(e);
    }
    // Expected in the project's budget until paid.
    let mut line_key = None;
    if invoice.kind != Kind::CreditNote
        && let Some((ledger, budget)) = steps.ledger.filter(|(_, b)| !b.is_empty())
    {
        let line_id = crate::ids::new();
        let mut args = crate::i18n::args();
        args.set("number", number.clone());
        args.set("client", invoice.client.first_line().to_string());
        let line = crate::budget::Line {
            id: line_id.clone(),
            budget: budget.to_string(),
            date: invoice.due_day().unwrap_or(day),
            amount: crate::money::Money(invoice.totals.due),
            label: tr.text("invoice-budget-line", Some(&args)),
            planned: true,
            links: vec![crate::links::invoice_uri(&number), crate::links::project_uri(&invoice.project)].into_iter().filter(|l| !l.ends_with('/')).collect(),
            ..crate::budget::Line::default()
        };
        let mut origin = crate::i18n::args();
        origin.set("number", number.clone());
        if let Err(e) = crate::budget::record_line(ledger, &line, &tr.text("invoice-budget-origin", Some(&origin))) {
            undo_record(&unmark);
            return Err(e);
        }
        line_key = Some((ledger, format!("line:{line_id}")));
    }
    // The draft taken out: issued, it is no draft any more.
    if let Err(e) = super::remove_draft(dir, id) {
        if let Some((ledger, key)) = &line_key {
            let _ = crate::budget::remove_line_by_key(ledger, key);
        }
        undo_record(&unmark);
        return Err(e);
    }
    Ok(invoice)
}

/// The time a credit note frees to be billed again: all of the invoice's
/// when it cancels the invoice whole; else the time of each line it cancels
/// whole (as much as the invoice's line with the same time, or more).
fn freed_sessions(credit: &Invoice, records: &[Invoice]) -> Vec<String> {
    let Some(cited) = &credit.corrects else { return Vec::new() };
    let Some(invoice) = records.iter().find(|r| r.number == cited.number && !r.unreadable) else { return Vec::new() };
    if credit.totals.due >= invoice.totals.due {
        return invoice.sessions();
    }
    credit
        .lines
        .iter()
        .filter(|l| !l.sessions.is_empty() && invoice.lines.iter().any(|o| o.sessions == l.sessions && l.amount >= o.amount))
        .flat_map(|l| l.sessions.iter().cloned())
        .collect()
}

/// A payment written on the issued invoice `number` of `dir`; returns the
/// invoice as it is now. One matched with a bank line it holds already is
/// not written again.
pub fn add_payment(dir: &Path, number: &str, payment: Payment) -> Result<Invoice, String> {
    if super::find_in(dir, number).is_none() {
        return Err(format!("{number}: no such invoice here"));
    }
    let (_, invoice) = super::change_life(dir, number, |invoice| {
        let twice = !payment.movement.is_empty() && invoice.payments.iter().any(|p| p.movement == payment.movement);
        if payment.amount != 0 && !twice {
            invoice.payments.push(payment);
        }
    })?;
    Ok(invoice)
}

/// A payment taken off `number` (its Undo); Ok(false) when it is not there any more.
pub fn remove_payment(dir: &Path, number: &str, payment: &Payment) -> Result<bool, String> {
    if super::find_in(dir, number).is_none() {
        return Ok(false);
    }
    let (removed, _) = super::change_life(dir, number, |invoice| match invoice.payments.iter().position(|p| p == payment) {
        Some(at) => {
            invoice.payments.remove(at);
            true
        }
        None => false,
    })?;
    Ok(removed)
}

/// "Paid", said by hand on `day` by `means`: a payment of what is left.
/// Returns the payment written, None when nothing was left to pay.
pub fn pay_rest(dir: &Path, number: &str, day: Date, means: &str) -> Result<Option<Payment>, String> {
    let all = super::all_in(dir);
    let invoice = all.iter().find(|i| i.number == number && !i.unreadable).ok_or_else(|| format!("{number}: no such invoice here"))?;
    let left = invoice.left(&all);
    if left <= 0 {
        return Ok(None);
    }
    let payment = Payment { date: day.to_string(), amount: left, means: if means.is_empty() { "transfer".into() } else { means.to_string() }, movement: String::new() };
    add_payment(dir, number, payment.clone())?;
    Ok(Some(payment))
}

/// "Not paid" after all: every payment said by hand taken off, a format-1
/// record's own word too; those the bank's matching found stay.
pub fn unpay(dir: &Path, number: &str) -> Result<(), String> {
    if super::find_in(dir, number).is_none() {
        return Err(format!("{number}: no such invoice here"));
    }
    super::change_life(dir, number, |invoice| {
        invoice.payments.retain(|p| !p.movement.is_empty());
        if let Some(older) = invoice.older.as_mut() {
            older.paid = false;
        }
    })
    .map(|_| ())
}

/// Of a payment of `amount` on `invoice`, what is turnover excluding VAT,
/// as declarations and the micro ceilings count it: its share of the
/// total without tax.
pub fn received_without_tax(invoice: &Invoice, amount: i64) -> i64 {
    if invoice.totals.with_tax == 0 || invoice.totals.tax == 0 {
        return amount;
    }
    i64::try_from(rounded(i128::from(amount) * i128::from(invoice.totals.without_tax), i128::from(invoice.totals.with_tax))).unwrap_or(amount)
}

/// The turnover received from `from` to `to` (both days counted), excluding
/// VAT, by the nature of what was sold: payments on invoices and deposits,
/// refunds on credit notes taken off.
pub fn received(invoices: &[Invoice], from: Date, to: Date) -> BTreeMap<&'static str, i64> {
    let mut out: BTreeMap<&'static str, i64> = BTreeMap::new();
    for invoice in invoices.iter().filter(|i| !i.number.is_empty()) {
        let sign = if invoice.kind == Kind::CreditNote { -1 } else { 1 };
        let goods: i64 = invoice.lines.iter().filter(|l| l.nature == Nature::Goods).map(|l| l.amount).sum();
        let base = invoice.totals.lines;
        for p in invoice.payments.iter().filter(|p| p.date.parse::<Date>().is_ok_and(|d| d >= from && d <= to)) {
            let without = received_without_tax(invoice, p.amount) * sign;
            let goods_part = if base == 0 { 0 } else { i64::try_from(rounded(i128::from(without) * i128::from(goods), i128::from(base))).unwrap_or(0) };
            *out.entry("goods").or_default() += goods_part;
            *out.entry("service").or_default() += without - goods_part;
        }
    }
    out.retain(|_, v| *v != 0);
    out
}

#[cfg(test)]
mod tests;

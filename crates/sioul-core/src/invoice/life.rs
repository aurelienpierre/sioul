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

use super::lives;
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

/// The legal form: as set; for a country chosen in Settings, else its
/// first (in France, an EI), a sole trader being who most often bills
/// alone; for a table from before November 2026, none until said (a
/// company under the franchise must not read « EI »: a check asks for it).
pub fn legal_form_of(s: &InvoiceSettings) -> String {
    if !s.legal_form.trim().is_empty() {
        s.legal_form.trim().to_string()
    } else if s.preset.trim().is_empty() {
        String::new()
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
    // A day as people write it (2027-01-01, 01/01/2027); one that does not read counts for none, and a check says it (`checks`).
    let set = s.tax.iter().filter(|t| crate::config::read_day(&t.from).map_or(t.from.trim().is_empty(), |from| from <= day)).max_by_key(|t| crate::config::read_day(&t.from).unwrap_or(Date::MIN));
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

/// The numbering pattern of `kind`: the settings', else the older prefix's;
/// the reset said in the settings applies to either (a prefix without the
/// year cannot start again each year: refused in words), so that the reset
/// Settings shows is the one the numbering follows.
pub fn pattern_of(s: &InvoiceSettings, kind: Kind) -> Result<Pattern, String> {
    let reset = Reset::named(s.reset.trim());
    if kind == Kind::CreditNote && !s.credit_pattern.trim().is_empty() {
        return Pattern::parse(s.credit_pattern.trim(), reset);
    }
    if !s.pattern.trim().is_empty() {
        return Pattern::parse(s.pattern.trim(), reset);
    }
    let usual = Pattern::of_prefix(&s.prefix);
    if s.reset.trim().is_empty() { Ok(usual) } else { usual.with_reset(reset) }
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

/// The client's country, as a code: its address's (a profile's, or a
/// card's last line naming one: `clients::card_address`; "Belgique" is BE,
/// "UK" GB: `clients::country`), else its VAT number's prefix ("BE0…";
/// Greece's EL, Northern Ireland's XI); empty when neither says. A country
/// Sioul does not know stays as written, and a required check asks for it
/// (`checks`).
pub fn client_country(client: &Client) -> String {
    let written = client.address.country.trim();
    if !written.is_empty() {
        return country_code(written);
    }
    let prefix: String = client.id("vat").unwrap_or("").chars().filter(char::is_ascii_alphabetic).take(2).collect::<String>().to_uppercase();
    match prefix.as_str() {
        "EL" => "GR".into(),
        "XI" => "GB".into(),
        p if p.len() == 2 && crate::clients::country(p).is_some() => p.to_string(),
        _ => String::new(),
    }
}

/// A country as a code (`clients::country`: "France" is FR, "UK" GB, "EL"
/// GR); one Sioul does not know as written, in capitals.
fn country_code(written: &str) -> String {
    crate::clients::country(written).map_or_else(|| written.trim().to_uppercase(), |c| c.code.to_string())
}

/// Where the client is, from the issuer: "domestic", "eu", "abroad", by
/// their VAT territories (`presets::zone`: Monaco is France's); or
/// "unknown" when nothing tells: no address at all, or one whose last
/// line names a country Sioul does not know (`clients::country_unsaid`).
/// An address written without a country line is one written for the
/// issuer's own post.
pub fn zone_of(issuer: &Issuer, client: &Client) -> &'static str {
    let country = client_country(client);
    if !country.is_empty() {
        return presets::zone(&country_code(&issuer.address.country), &country);
    }
    if client.address.is_empty() || crate::clients::country_unsaid(&client.address) { "unknown" } else { "domestic" }
}

/// How a line is taxed: its category, rate, the words saying why it bears
/// no VAT here (empty when there are none to say, or none known) and their
/// EN 16931 code, and the rule that placed it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placed {
    pub category: String,
    pub rate: i64,
    pub mention: String,
    pub reason_code: String,
    pub rule: Option<presets::Place>,
}

/// Where a line of `nature` for `client` is taxed, on `day`, in `language`:
/// the preset's first placement rule that applies (the issuer's scheme, the
/// client's zone and kind, the line's nature: the reverse charge, an
/// intra-EU delivery, an export, outside the scope); else the scheme's own:
/// none ("O"), the franchise ("E"), liable at the preset's first rate ("S").
/// A client whose zone is unknown is taken as domestic here, and a check asks for its country.
pub fn placed(issuer: &Issuer, client: &Client, nature: Nature, day: Date, language: &str) -> Placed {
    let preset = presets::get(&issuer.preset);
    let zone = match zone_of(issuer, client) {
        "unknown" => "domestic",
        zone => zone,
    };
    let nature_id = if nature == Nature::Goods { "goods" } else { "service" };
    let rule = preset.tax.place.iter().find(|r| {
        (r.scheme.is_empty() || r.scheme.contains(&issuer.scheme))
            && (r.zone.is_empty() || r.zone.iter().any(|z| z == zone))
            && (r.client.is_empty() || r.client.contains(&client.kind))
            && (r.nature.is_empty() || r.nature.iter().any(|n| n == nature_id))
    });
    if let Some(rule) = rule {
        return Placed { category: rule.category.clone(), rate: 0, mention: rule.mention.get(language).to_string(), reason_code: rule.reason_code.clone(), rule: Some(rule.clone()) };
    }
    match issuer.scheme.as_str() {
        "franchise" => {
            let franchise = preset.tax.franchise.as_ref();
            Placed { category: franchise.map_or_else(|| "E".into(), |f| f.category.clone()), reason_code: franchise.map(|f| f.reason_code.clone()).unwrap_or_default(), ..Placed::default() }
        }
        "liable" => Placed { category: "S".into(), rate: preset.rates_on(day).first().map_or(0, |r| r.rate), ..Placed::default() },
        _ => Placed { category: "O".into(), ..Placed::default() },
    }
}

/// The category and rate a new line of `nature` takes for `client` (`placed`).
pub fn default_tax(issuer: &Issuer, client: &Client, nature: Nature, day: Date) -> (String, i64) {
    let p = placed(issuer, client, nature, day, "en");
    (p.category, p.rate)
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

/// A project's billable time not billed yet among `entries`, as its
/// invoice's lines group it: a line per task or note (its title, else the
/// project's), its minutes and its sessions, in the order first worked on.
pub fn time_lines(entries: &[Entry], project: &Project) -> Vec<(String, u32, Vec<String>)> {
    let mut grouped: Vec<(i64, String, u32, Vec<String>)> = Vec::new();
    for entry in entries.iter().filter(|e| e.project == project.id && e.billable && e.invoice.is_empty() && e.minutes > 0) {
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
    grouped.into_iter().map(|(_, label, minutes, keys)| (label, minutes, keys)).collect()
}

/// The time no draft holds and none of the project's drafts set aside:
/// what a draft of the project's time may still take, so that the same
/// hours are never in two drafts, nor back where they were taken out (the
/// review of phase 3, F9; the window, the command line and the agents'
/// tool alike: the second review, R15).
pub fn untaken(entries: Vec<Entry>, drafts: &[Invoice], project: &str) -> Vec<Entry> {
    let held: std::collections::BTreeSet<String> = drafts.iter().flat_map(Invoice::sessions).collect();
    let aside: std::collections::BTreeSet<&String> = drafts.iter().filter(|d| d.project == project).flat_map(|d| d.set_aside.iter()).collect();
    entries.into_iter().filter(|e| !held.contains(&e.key) && !aside.contains(&e.key)).collect()
}

/// The project's draft of its time: the one that holds some (the newest
/// first), else its newest, among the drafts `usable` takes.
pub fn draft_of_project<'a>(drafts: &'a [Invoice], project: &str, usable: &dyn Fn(&Invoice) -> bool) -> Option<&'a Invoice> {
    let mine: Vec<&Invoice> = drafts.iter().filter(|d| d.project == project && d.kind == Kind::Invoice && d.copy_of.is_empty() && !d.newer && usable(d)).collect();
    mine.iter().find(|d| !d.sessions().is_empty()).or_else(|| mine.first()).copied()
}

/// Lines of time added to a draft: each joins the draft's line of time of
/// the same words and hourly price (its minutes and its sessions), else it
/// is a line of its own; so that a project's "to bill", which rounds each
/// label's time once, and its draft agree to the cent (R15). Returns the
/// lines and the minutes added.
pub fn add_time(draft: &mut Invoice, more: Vec<Line>) -> (usize, u32) {
    let (count, minutes) = (more.len(), more.iter().filter_map(Line::minutes).sum());
    for line in more {
        match draft.lines.iter_mut().find(|l| super::edit::timed(l) && l.description == line.description && l.price == line.price && l.base == line.base && l.unit == line.unit && l.discount == 0) {
            Some(there) => {
                there.quantity += line.quantity;
                there.sessions.extend(line.sessions);
            }
            None => draft.lines.push(line),
        }
    }
    (count, minutes)
}

/// The minutes of the time set aside in a draft, as its sessions' keys say them.
pub fn set_aside_minutes(draft: &Invoice) -> u32 {
    draft.set_aside.iter().filter_map(|key| key.rsplit(':').next()?.parse::<u32>().ok()).sum()
}

/// What a project's time not billed yet comes to at `price` (cents an
/// hour), each of its lines rounded to the cent as its invoice rounds them
/// (`Line::net`): the project's "to bill" and the invoice made of it are
/// never a cent apart (the review of phase 3, F18).
pub fn time_amount(entries: &[Entry], project: &Project, price: i64) -> i64 {
    time_lines(entries, project).iter().map(|(label, minutes, _)| Line::of_time(label, *minutes, price, "", 0).net()).sum()
}

/// A draft of a project's billable time not billed yet, among `entries`: a
/// line per task or note, its time at the project's rate (the settings'
/// when it has none), in the order first worked on; the service's period
/// from the first and last days billed. None when nothing waits to be billed.
/// The time billed is what the issued `records` say (`with_billing`), whatever the sessions' marks.
pub fn from_time(entries: &[Entry], project: &Project, mut draft: Invoice, records: &[Invoice], s: &InvoiceSettings, today: Date) -> Option<Invoice> {
    let entries = with_billing(entries, records);
    let billed: Vec<&Entry> = entries.iter().filter(|e| e.project == project.id && e.billable && e.invoice.is_empty() && e.minutes > 0).collect();
    if billed.is_empty() {
        return None;
    }
    // The project's rate, a float in its file, made cents here at the boundary.
    let price = (project.rate.unwrap_or(s.rate) * 100.0).round() as i64;
    let issuer = issuer_on(s, today);
    let (category, rate) = default_tax(&issuer, &draft.client, Nature::Service, today);
    draft.lines.extend(time_lines(&entries, project).into_iter().map(|(label, minutes, keys)| Line { sessions: keys, ..Line::of_time(&label, minutes, price, &category, rate) }));
    draft.period_from = billed.iter().map(|e| e.day).min().map(|d| d.to_string()).unwrap_or_default();
    draft.period_to = billed.iter().map(|e| e.day).max().map(|d| d.to_string()).unwrap_or_default();
    draft.period_default = false;
    draft.project = project.id.clone();
    draft.project_title = project.title.clone();
    refresh(&mut draft, s, today);
    Some(draft)
}

/// A draft credit note for `invoice`: all its lines to cancel it, or the
/// ones at `chosen` to correct some; it cites the invoice's number and day.
/// A final invoice cancelled whole deducts its deposits as it did, which
/// it gives back to be deducted again. A format-1 invoice's lines carry no
/// time of their own: a line credited takes its sessions from `entries`
/// (the time noted), by its name among the invoice's, to free them.
pub fn credit(invoice: &Invoice, chosen: Option<&[usize]>, entries: &[Entry], s: &InvoiceSettings, today: Date) -> Invoice {
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
            if line.exemption.trim().is_empty() {
                line.exemption = reason_of(&line.category);
            }
            line.own_tax = true;
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
    // A format-1 record's time is its own, not its lines': all of it with
    // all its lines; a line alone, the sessions of its name.
    if let Some(older) = &invoice.older {
        if chosen.is_none() {
            if let Some(first) = draft.lines.first_mut() {
                first.sessions = older.sessions.clone();
            }
        } else {
            for line in &mut draft.lines {
                line.sessions = entries.iter().filter(|e| older.sessions.contains(&e.key) && (if e.title.trim().is_empty() { invoice.project_title.as_str() } else { e.title.as_str() }) == line.description).map(|e| e.key.clone()).collect();
            }
        }
    }
    if chosen.is_none() {
        draft.deposits = invoice.deposits.clone();
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

/// A final invoice deducts the deposit invoices it cites: what is left of
/// each, with tax, after its credit notes among `all` (`deposit_left`).
pub fn cite_deposits(draft: &mut Invoice, deposits: &[&Invoice], all: &[Invoice], s: &InvoiceSettings, today: Date) {
    draft.deposits = deposits.iter().filter(|d| d.kind == Kind::Deposit && !d.number.is_empty()).map(|d| Cited { number: d.number.clone(), date: d.date.clone(), amount: deposit_left(d, all) }).collect();
    refresh(draft, s, today);
}

/// Each line without an id given one: an id is given once and kept.
pub fn line_ids(draft: &mut Invoice) {
    for line in draft.lines.iter_mut().filter(|l| l.id.is_empty()) {
        line.id = crate::ids::new().chars().filter(char::is_ascii_alphanumeric).take(10).collect();
    }
}

/// What is left of a deposit invoice to deduct: its amount with tax, less
/// its credit notes among `all`.
pub fn deposit_left(deposit: &Invoice, all: &[Invoice]) -> i64 {
    (deposit.totals.with_tax - deposit.credited(all)).max(0)
}

/// Whether two invoices bill the same client: the reference it was chosen
/// by (a profile, a contact, a name), else the first line of its name.
pub fn same_client(a: &Invoice, b: &Invoice) -> bool {
    let (x, y) = (a.client_ref.trim(), b.client_ref.trim());
    if !x.is_empty() && x == y {
        return true;
    }
    let (x, y) = (a.client.first_line().trim(), b.client.first_line().trim());
    !x.is_empty() && x.eq_ignore_ascii_case(y)
}

/// The final invoices among `all` that deduct `deposit` and are not credited whole.
pub fn deducted_by<'a>(deposit: &Invoice, all: &'a [Invoice]) -> Vec<&'a Invoice> {
    all.iter().filter(|r| r.kind == Kind::Invoice && !r.unreadable && !r.number.is_empty() && r.deposits.iter().any(|d| d.number == deposit.number) && r.credited(all) < r.totals.due.max(1)).collect()
}

/// The issued deposit invoices a final invoice `draft` may deduct: of the
/// same client (`same_client`), in its currency, something left of them
/// after their credit notes, deducted by no live final invoice.
pub fn deposits_open<'a>(draft: &Invoice, all: &'a [Invoice]) -> Vec<&'a Invoice> {
    all.iter()
        .filter(|r| r.kind == Kind::Deposit && !r.number.is_empty() && !r.unreadable)
        .filter(|r| r.currency.eq_ignore_ascii_case(&draft.currency) && same_client(r, draft))
        .filter(|r| deposit_left(r, all) > 0 && deducted_by(r, all).iter().all(|f| f.number == draft.number && !draft.number.is_empty()))
        .collect()
}

/// An invoice draft's deposits made again from the records: what is left of
/// each after its credit notes; the day it was issued. A credit note keeps
/// the amounts of the invoice it cancels (the second review of phase 3, R7).
pub fn recite(draft: &mut Invoice, all: &[Invoice]) {
    if draft.kind != Kind::Invoice {
        return;
    }
    for cited in &mut draft.deposits {
        if let Some(deposit) = all.iter().find(|r| r.number == cited.number && r.kind == Kind::Deposit && !r.unreadable) {
            cited.amount = deposit_left(deposit, all);
            cited.date = deposit.date.clone();
        }
    }
}

/// A draft made as it would be issued on `day`: the issuer as the settings
/// say then (frozen at issue), each line's tax as the scheme asks (no VAT
/// is ever shown under the franchise or without a scheme), the legal lines
/// the preset prints for this client, the late-payment rate, the due day
/// and the totals. The person's own notes stay.
pub fn refresh(draft: &mut Invoice, s: &InvoiceSettings, day: Date) {
    draft.format = FORMAT;
    line_ids(draft);
    draft.issuer = issuer_on(s, day);
    // Each line placed where its VAT is due (`placed`), unless its tax is
    // its own (set by you, or kept from the invoice a credit note credits).
    // No VAT is ever shown under the franchise or without a scheme (art. 293
    // E CGI: VAT shown is VAT owed): the rules of those schemes say so.
    let language = if draft.language.is_empty() { "en".to_string() } else { Translator::new(&draft.language).language().to_string() };
    for line in draft.lines.iter_mut().filter(|l| !l.own_tax) {
        let p = placed(&draft.issuer, &draft.client, line.nature, day, &language);
        line.category = p.category;
        line.rate = p.rate;
        line.exemption = p.mention;
        line.reason_code = p.reason_code;
    }
    let preset = presets::get(&draft.issuer.preset);
    // A business's credit note says the payment lines too, as French rules ask of every invoice (BR-FR-05).
    draft.penalty_rate = if draft.client.is_business() { penalty_on(s, day) } else { 0 };
    draft.notes = legal_notes(draft, s, preset).into_iter().chain(draft.notes.iter().filter(|n| n.legal.is_empty()).cloned()).collect();
    if !draft.number.is_empty() || !draft.date.is_empty() {
        let issued: Date = draft.date.parse().unwrap_or(day);
        draft.due = draft.terms.due(issued).to_string();
    } else {
        draft.due = draft.terms.due(day).to_string();
    }
    // The service's day, when nothing says it: the day of issue, said on
    // the draft (the day it is made again), so that it can be changed;
    // issued, it is the issue's day (ann. II CGI, art. 242 nonies A I-10°).
    // A deposit invoice says none: the service is to come.
    if draft.kind == Kind::Deposit {
        draft.period_from.clear();
        draft.period_to.clear();
        draft.period_default = false;
    } else if draft.number.is_empty() && (draft.period_from.is_empty() || draft.period_default) {
        let on = if draft.date.is_empty() { day.to_string() } else { draft.date.clone() };
        draft.period_from = on.clone();
        draft.period_to = on;
        draft.period_default = true;
    }
    let issuer = draft.issuer.clone();
    let franchise_code = preset.tax.franchise.as_ref().map(|f| f.reason_code.clone()).unwrap_or_default();
    // Why a category bears no VAT: its lines' own words (the placement's, or
    // yours); the franchise's mention for its exempt lines.
    let reasons = move |category: &str, lines: &[&Line]| -> (String, String) {
        let own = lines.iter().map(|l| l.exemption.trim()).find(|e| !e.is_empty()).map(str::to_string);
        let code = lines.iter().map(|l| l.reason_code.trim()).find(|c| !c.is_empty()).map(str::to_string);
        match (category, own) {
            (_, Some(words)) => (words, code.unwrap_or_default()),
            ("E", None) if issuer.scheme == "franchise" => (issuer.mention.clone(), code.unwrap_or_else(|| franchise_code.clone())),
            (_, None) => (String::new(), code.unwrap_or_default()),
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
    let mut out = Vec::new();
    for line in &preset.line {
        let for_client = line.client.is_empty() || line.client.contains(&draft.client.kind);
        let for_option = line.option.is_empty() || issuer.options.contains(&line.option);
        let for_scheme = line.scheme.is_empty() || line.scheme.contains(&issuer.scheme);
        let for_form = line.form.is_empty() || line.form.iter().any(|f| f == &issuer.legal_form || (f == "company" && company));
        if !(for_client && for_option && for_scheme && for_form) {
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
    change_draft_read(dir, id, s, day, |draft, _| change(draft))
}

/// `change_draft`, its change given the draft's revision as it is read
/// under the lock: the draft's one file is read, changed and written in
/// one step, under the lock the sharing takes to write it, so that no
/// other writer's change is lost between the read and the write (the
/// second review of phase 3, R4). Its deposits are cited again as they are
/// now, as at issue: the editor, the preview and the issue agree (R8).
pub fn change_draft_read(dir: &Path, id: &str, s: &InvoiceSettings, day: Date, change: impl FnOnce(&mut Invoice, &str)) -> Result<Invoice, String> {
    super::with_draft_lock(dir, id, || {
        let mut draft = super::draft_in(dir, id).ok_or_else(|| format!("{id}: no such draft here"))?;
        // Written by a newer Sioul: never written over with what this one does not know.
        if draft.newer {
            return Err(format!("{id}: written by a newer Sioul; update Sioul on this device to change it."));
        }
        let revision = super::draft_revision(dir, id);
        change(&mut draft, &revision);
        draft.number.clear();
        draft.draft = id.to_string();
        if !draft.deposits.is_empty() {
            recite(&mut draft, &super::all_in(dir));
        }
        refresh(&mut draft, s, day);
        super::save_draft(dir, &draft)
    })
}

/// What a check found: required (issue waits) or recommended (said only).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Check {
    pub required: bool,
    pub text: String,
}

/// What the preset asks of this draft and it lacks, in plain words, as it
/// would be issued on `day`: required items first.
/// `records` are the invoices issued (`all_in`): a credit note is checked
/// against what is left to credit on the invoice it cites.
pub fn checks(draft: &Invoice, s: &InvoiceSettings, day: Date, records: &[Invoice]) -> Vec<Check> {
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
        say(true, if e == super::series::NEEDS_YEAR { words("invoice-check-reset-year") } else { e });
    }
    if draft.lines.is_empty() {
        say(true, words("invoice-check-no-lines"));
    }
    // The day a VAT scheme starts, as written in Settings: one that does not
    // read counts for none, so it is said and holds the issue back (F19).
    for tax in s.tax.iter().filter(|t| !t.from.trim().is_empty() && crate::config::read_day(&t.from).is_none()) {
        let mut args = crate::i18n::args();
        args.set("day", tax.from.trim().to_string());
        say(true, tr.text("invoice-check-scheme-day", Some(&args)));
    }
    // Each line's tax as the scheme in force on the day of issue allows it
    // (F3): no VAT under the franchise or without a scheme (art. 283-3 CGI:
    // VAT shown on an invoice is owed), and VAT at a rate with its rate
    // (EN 16931, BR-S-05). A credit note keeps the tax its invoice was billed with.
    if draft.kind != Kind::CreditNote {
        for (n, line) in draft.lines.iter().enumerate() {
            let mut args = crate::i18n::args();
            args.set("line", (n + 1) as i64);
            if issuer.scheme != "liable" && (line.rate > 0 || line.category == "S") {
                args.set("scheme", tr.text(&format!("invoices-scheme-{}", if issuer.scheme.is_empty() { "none" } else { issuer.scheme.as_str() }), None));
                args.set("day", crate::books::long_day(&tr, day));
                say(true, tr.text("invoice-check-line-vat-unowed", Some(&args)));
            } else if issuer.scheme == "liable" && line.category == "S" && line.rate <= 0 {
                say(true, tr.text("invoice-check-line-rate-missing", Some(&args)));
            }
        }
    }
    if draft.lines.iter().any(|l| l.description.trim().is_empty()) {
        say(true, words("invoice-check-line-unnamed"));
    }
    if draft.kind == Kind::CreditNote && draft.corrects.as_ref().is_none_or(|c| c.number.is_empty()) {
        say(true, words("invoice-check-credit-cites"));
    }
    // Never more credited than is left to credit: an invoice credited twice gives the client twice.
    if draft.kind == Kind::CreditNote
        && let Some(cited) = draft.corrects.as_ref().filter(|c| !c.number.is_empty())
        && let Some(invoice) = records.iter().find(|r| r.number == cited.number && !r.unreadable)
    {
        let left = invoice.left_to_credit(records);
        if draft.totals.due > left {
            let notes: Vec<String> = records.iter().filter(|r| r.kind == Kind::CreditNote && r.corrects.as_ref().is_some_and(|c| c.number == invoice.number)).map(|r| r.number.clone()).collect();
            let mut args = crate::i18n::args();
            args.set("number", invoice.number.clone());
            args.set("left", tr.money_in(crate::money::Money(left), &invoice.currency));
            args.set("notes", if notes.is_empty() { "—".to_string() } else { notes.join(", ") });
            say(true, tr.text("invoice-check-credit-too-much", Some(&args)));
        }
    }
    // A deposit a live final invoice deducts is credited through that
    // invoice: credited on its own, the final invoice would go on deducting
    // an amount the deposit no longer bills, and could no longer be
    // cancelled whole (the second review of phase 3, R7).
    if draft.kind == Kind::CreditNote
        && let Some(cited) = draft.corrects.as_ref().filter(|c| !c.number.is_empty())
        && let Some(deposit) = records.iter().find(|r| r.number == cited.number && r.kind == Kind::Deposit && !r.unreadable)
        && let Some(by) = deducted_by(deposit, records).first()
    {
        let mut args = crate::i18n::args();
        args.set("deposit", deposit.number.clone());
        args.set("number", by.number.clone());
        say(true, tr.text("invoice-check-credit-deposit-deducted", Some(&args)));
    }
    if draft.currency.trim().len() != 3 {
        say(true, words("invoice-check-currency"));
    }
    let company = preset.form(&issuer.legal_form).is_some_and(|f| f.company);
    let zone = zone_of(&issuer, &draft.client);
    let natures: Vec<&str> = draft.lines.iter().map(|l| if l.nature == Nature::Goods { "goods" } else { "service" }).collect();
    let categories: Vec<&str> = draft.lines.iter().map(|l| l.category.as_str()).collect();
    // A table from before November 2026 says no legal form: asked once, « EI » not printed meanwhile.
    if s.legal_form.trim().is_empty() && s.preset.trim().is_empty() && !preset.legal_form.is_empty() {
        say(true, words("invoice-check-legal-form"));
    }
    // A scheme the country does not offer (none under France, where every
    // business is under the franchise or liable) is never taken as said.
    if preset.id != presets::ELSEWHERE && !preset.tax.schemes.is_empty() && !preset.tax.schemes.contains(&issuer.scheme) {
        let mut args = crate::i18n::args();
        args.set("scheme", issuer.scheme.clone());
        args.set("schemes", preset.tax.schemes.join(", "));
        say(true, tr.text("invoice-check-scheme-offered", Some(&args)));
    }
    // The client's country, where a line's VAT would be due elsewhere had
    // it another: never taken for the issuer's own without a word (G2).
    let placing = preset.tax.place.iter().any(|r| (r.scheme.is_empty() || r.scheme.contains(&issuer.scheme)) && (r.client.is_empty() || r.client.contains(&draft.client.kind)) && (r.nature.is_empty() || r.nature.iter().any(|n| natures.contains(&n.as_str()))));
    if zone == "unknown" && placing {
        say(true, words("invoice-check-client-country"));
    }
    // An address with no country line is written for your own post: taken
    // as your country, and said (never without a word, G2).
    if zone == "domestic" && placing && client_country(&draft.client).is_empty() && !draft.client.address.is_empty() {
        let mut args = crate::i18n::args();
        let home = presets::vat_territory(&issuer.address.country);
        args.set("country", crate::clients::country(&home).map_or(home.clone(), |c| c.name(&language).to_string()));
        say(false, tr.text("invoice-check-client-country-assumed", Some(&args)));
    }
    // A deposit is deducted for its own client, in its own currency, while
    // something of it is left after its credit notes (the review of phase 3, F4).
    for cited in draft.deposits.iter().filter(|_| draft.kind == Kind::Invoice) {
        let Some(deposit) = records.iter().find(|r| r.number == cited.number && r.kind == Kind::Deposit && !r.unreadable) else { continue };
        let mut args = crate::i18n::args();
        args.set("deposit", cited.number.clone());
        if !same_client(deposit, draft) || !deposit.currency.eq_ignore_ascii_case(&draft.currency) {
            say(true, tr.text("invoice-check-deposit-other", Some(&args)));
        } else if deposit_left(deposit, records) == 0 {
            say(true, tr.text("invoice-check-deposit-credited", Some(&args)));
        }
    }
    // A deposit of this client not deducted yet is said, so that a final
    // invoice never forgets it (the review of phase 3, F10).
    if draft.kind == Kind::Invoice && draft.number.is_empty() {
        let open: Vec<String> = deposits_open(draft, records).into_iter().filter(|d| !draft.deposits.iter().any(|c| c.number == d.number)).map(|d| d.number.clone()).collect();
        if !open.is_empty() {
            let mut args = crate::i18n::args();
            args.set("numbers", open.join(", "));
            args.set("n", open.len() as i64);
            say(false, tr.text("invoice-check-deposit-undeducted", Some(&args)));
        }
    }
    // A deposit invoice is deducted by one final invoice at a time: a final
    // invoice credited whole gives its deposits back.
    for cited in draft.deposits.iter().filter(|_| draft.kind == Kind::Invoice) {
        let taken = records.iter().find(|r| {
            r.kind == Kind::Invoice && !r.unreadable && r.number != draft.number && r.deposits.iter().any(|d| d.number == cited.number) && r.credited(records) < r.totals.due.max(1)
        });
        if let Some(by) = taken {
            let mut args = crate::i18n::args();
            args.set("deposit", cited.number.clone());
            args.set("number", by.number.clone());
            say(true, tr.text("invoice-check-deposit-taken", Some(&args)));
        }
    }
    // A line not taxed here says why, in words the research gives: where
    // they lack, the line writes its own (the review of phase 1, F4, F29).
    for line in &draft.lines {
        if !line.exemption.trim().is_empty() {
            continue;
        }
        if line.own_tax {
            let says_nothing = matches!(line.category.as_str(), "K" | "G" | "AE") || (line.category == "E" && issuer.scheme != "franchise") || (line.category == "O" && issuer.scheme != "none");
            if says_nothing {
                say(true, words("invoice-check-line-exemption"));
            }
            continue;
        }
        if let Some(rule) = placed(&issuer, &draft.client, line.nature, day, &language).rule.filter(|r| r.mention.is_empty()) {
            say(true, rule.why.get(&language).to_string());
        }
    }
    // Two late-payment rates never: your payment words speak of late fees,
    // and Sioul prints its own (the review of phase 1, F27).
    let folded: String = crate::text::fold(&issuer.payment).into_iter().collect();
    let speaks = ["penalit", "penalt", "indemnit", "late fee", "late payment", "retard de paiement", "recouvrement"].iter().any(|w| folded.contains(w));
    if speaks && draft.notes.iter().any(|n| matches!(n.legal.as_str(), "penalties" | "indemnity")) {
        say(true, words("invoice-check-two-late-fees"));
    }
    for rule in &preset.require {
        let applies = (rule.client.is_empty() || rule.client.contains(&draft.client.kind))
            && (rule.category.is_empty() || rule.category.iter().any(|c| categories.contains(&c.as_str())))
            && (rule.activity.is_empty() || rule.activity.iter().any(|a| s.activity.contains(a)))
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
        // Only what the invoice prints counts (G11). A SIRET being given,
        // said so, stands for it (SP F31808), as "other" too.
        let pending = |kind: &str| matches!(kind, "siren" | "siret") && ids.iter().any(|i| i.kind == "other" && i.printed && super::ids::pending(&i.value));
        let has = |kind: &str| ids.iter().any(|i| i.kind == kind && i.printed && !i.value.trim().is_empty()) || pending(kind);
        let met = if !rule.any.is_empty() {
            rule.any.iter().any(|k| has(k))
        } else {
            match (rule.who, rule.field.as_str()) {
                (Who::Issuer, "name") => !issuer.name.trim().is_empty(),
                (Who::Issuer, "address") => !issuer.address.is_empty(),
                (Who::Issuer, "mention") => !issuer.mention.trim().is_empty(),
                (Who::Issuer, "capital") => !issuer.capital.trim().is_empty(),
                (Who::Issuer, "contact") => !issuer.email.trim().is_empty(),
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
    // The issued file: a character its font cannot print waits (a PDF/A
    // never draws a missing glyph); a wrong IBAN is said; an invoice whose
    // PDF will carry no Factur-X XML is said, never held back.
    let probe = Invoice { issuer: issuer.clone(), ..draft.clone() };
    let unprintable = super::pdf::unprintable(&probe);
    if !unprintable.is_empty() {
        let mut args = crate::i18n::args();
        args.set("characters", unprintable.into_iter().collect::<String>());
        say(true, tr.text("invoice-check-unprintable", Some(&args)));
    }
    // Right-to-left writing is printed backwards until Sioul orders it (Unicode's bidi): held back.
    let backwards = super::pdf::right_to_left(&probe);
    if !backwards.is_empty() {
        let mut args = crate::i18n::args();
        args.set("characters", backwards.into_iter().collect::<String>());
        say(true, tr.text("invoice-check-right-to-left", Some(&args)));
    }
    // What EN 16931 refuses in the file's data: a unit price below zero
    // (BR-27: a discount or a credit note says less), a unit outside
    // Recommendation 20's, a country that is none.
    for (n, line) in draft.lines.iter().enumerate() {
        let mut args = crate::i18n::args();
        args.set("line", (n + 1) as i64);
        args.set("unit", line.unit.clone());
        if line.price < 0 {
            say(true, tr.text("invoice-check-price-negative", Some(&args)));
        }
        if !super::units::known(&line.unit) {
            say(true, tr.text("invoice-check-unit-unknown", Some(&args)));
        }
    }
    let delivery = draft.client.delivery.as_ref().map(|d| d.country.clone()).unwrap_or_default();
    for (country, id) in [(&issuer.address.country, "invoice-check-own-country"), (&draft.client.address.country, "invoice-check-client-country-unknown"), (&delivery, "invoice-check-delivery-country")] {
        if !country.trim().is_empty() && crate::clients::country(country).is_none() {
            let mut args = crate::i18n::args();
            args.set("country", country.trim().to_string());
            say(true, tr.text(id, Some(&args)));
        }
    }
    for bank in issuer.banks.iter().filter(|b| !super::epc::iban_valid(&b.iban)) {
        let mut args = crate::i18n::args();
        args.set("iban", bank.iban.clone());
        say(false, tr.text("invoice-check-iban-wrong", Some(&args)));
    }
    if let Some(why) = super::cii::without(&probe).filter(|w| *w != super::cii::Without::Older) {
        say(false, why.said(&tr));
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

/// What issuing needs of this device: its name in the sharing (each
/// device writes its own journal and life files), whether it shares,
/// whether its `drafts` part (which carries invoices) is on, and every
/// device's last numbers (`sioul_sync::lease::Claim::series`).
#[derive(Debug, Clone, Default)]
pub struct Numbering {
    pub device: String,
    pub sharing: bool,
    pub drafts_shared: bool,
    pub claimed: BTreeMap<String, u64>,
}

/// Where issuing writes besides the invoices' folder: the time's folder
/// (its sessions marked), and the budget file with the budget the
/// project's invoices are expected in; and the client profiles and
/// contacts a draft's client is made again from (`refresh_client`).
#[derive(Debug, Clone, Default)]
pub struct Steps<'a> {
    pub time: Option<&'a Path>,
    pub ledger: Option<(&'a Path, &'a str)>,
    pub clients: Option<(&'a crate::clients::Clients, &'a [crate::contacts::Contact])>,
}

/// A draft's client made again from its profile, or its contact's card,
/// until it is issued (the second review, G12): a profile changed since the
/// draft was made counts; the issued record keeps what it was then. A
/// credit note keeps the client of the invoice it credits, as it was; a
/// client written as a name alone, or whose profile and card are gone,
/// stays as the draft has it.
pub fn refresh_client(draft: &mut Invoice, clients: &crate::clients::Clients, contacts: &[crate::contacts::Contact]) {
    let reference = draft.client_ref.trim();
    if draft.kind == Kind::CreditNote || reference.is_empty() {
        return;
    }
    let (client, profile) = crate::clients::resolve(reference, clients, contacts);
    let named_only = Client { kind: client.kind.clone(), name: reference.to_string(), ..Client::default() };
    if profile.is_some() || client != named_only {
        draft.client = client;
    }
}

/// The time billed, by session key, as the issued records say it: each
/// session an invoice or a deposit invoice lists is billed by it, unless a
/// credit note cancelled it since (`freed_sessions`), whatever the sessions'
/// own marks say: those are a cache, made again from this (`reconcile_marks`).
pub fn billed(records: &[Invoice]) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for invoice in records.iter().filter(|r| !r.unreadable && !r.number.is_empty() && r.kind != Kind::CreditNote) {
        for key in invoice.sessions() {
            out.insert(key, invoice.number.clone());
        }
    }
    for credit in records.iter().filter(|r| !r.unreadable && !r.number.is_empty() && r.kind == Kind::CreditNote) {
        let corrected = credit.corrects.as_ref().map(|c| c.number.as_str()).unwrap_or("");
        for key in freed_sessions(credit, records) {
            if out.get(&key).is_some_and(|n| n == corrected) {
                out.remove(&key);
            }
        }
    }
    out
}

/// Entries with their billing as the issued records say it (`billed`): a
/// session a record bills is billed by it, whatever its mark; one marked
/// by a number whose record is not here (yet) stays as marked.
pub fn with_billing(entries: &[Entry], records: &[Invoice]) -> Vec<Entry> {
    let billed = billed(records);
    let here: std::collections::BTreeSet<&str> = records.iter().filter(|r| !r.unreadable).map(|r| r.number.as_str()).collect();
    entries
        .iter()
        .map(|e| {
            let mut e = e.clone();
            match billed.get(&e.key) {
                Some(number) => e.invoice = number.clone(),
                // Marked by an invoice that is here and does not bill it (a credit note freed it).
                None if here.contains(e.invoice.as_str()) => e.invoice.clear(),
                None => {}
            }
            e
        })
        .collect()
}

/// The sessions' marks made again from the issued records (`billed`): a
/// session a record bills carries its number; one marked by a number whose
/// record is here and bills it no more (credited) is cleared; a mark whose
/// record is not here yet stays. Returns how many sessions changed. Run
/// before each issue, so that a crash between a record and its marks never
/// lets the same time be billed twice.
pub fn reconcile_marks(records: &[Invoice], time_folder: &Path) -> Result<usize, String> {
    let billed = billed(records);
    let here: std::collections::BTreeSet<String> = records.iter().filter(|r| !r.unreadable).map(|r| r.number.clone()).collect();
    let sessions = crate::timelog::sessions_in(time_folder);
    let wrong: Vec<String> = sessions
        .iter()
        .filter(|x| match billed.get(&x.key()) {
            Some(number) => x.invoice != *number,
            None => !x.invoice.is_empty() && here.contains(&x.invoice),
        })
        .map(crate::timelog::Session::key)
        .collect();
    if wrong.is_empty() {
        return Ok(0);
    }
    crate::timelog::update_in(time_folder, &wrong, |x| match billed.get(&x.key()) {
        Some(number) => x.invoice = number.clone(),
        None => x.invoice.clear(),
    })
}

/// Issues the draft `id` of `dir` on `day`: checked first (what is required
/// and missing refuses it, in words); then, under the numbering's lock,
/// numbered as one more than the journal, the records and every device's
/// claim, kept under its number (its client as the draft froze it), this
/// device's journal written; its time marked (a credit note's cancelled
/// time freed); its budget line planned (a credit note lowers the credited
/// invoice's instead); the draft taken out. All or nothing: a step that
/// fails undoes the ones before it.
/// Why an issue asked for a draft as it was read did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotIssued {
    /// The draft's file moved since it was read (another device, the
    /// command line, an agent): it is to be read again first; in words.
    Moved(String),
    /// Refused as `issue` refuses, in words.
    Refused(String),
}

/// `issue` for the draft as it was read (`revision`, its
/// `invoice::draft_revision` then): refused when its file moved since, so
/// that what is numbered is what was shown (the second review of phase 3,
/// R6). The file's lock is held from the look to the issue.
pub fn issue_as_read(dir: &Path, id: &str, revision: &str, s: &InvoiceSettings, day: Date, numbering: &Numbering, steps: &Steps, tr: &Translator) -> Result<Invoice, NotIssued> {
    super::with_draft_lock(dir, id, || {
        let now = super::draft_revision(dir, id);
        if !now.is_empty() && now != revision {
            return Err(NotIssued::Moved(tr.text("invoice-draft-moved", None)));
        }
        issue(dir, id, s, day, numbering, steps, tr).map_err(NotIssued::Refused)
    })
}

pub fn issue(dir: &Path, id: &str, s: &InvoiceSettings, day: Date, numbering: &Numbering, steps: &Steps, tr: &Translator) -> Result<Invoice, String> {
    // Sharing on, the `drafts` part off: the records do not travel, so the
    // other devices' numbers may not be known here.
    if numbering.sharing && !numbering.drafts_shared {
        return Err(tr.text("invoice-drafts-part-off", None));
    }
    let mut invoice = super::draft_in(dir, id).ok_or_else(|| tr.text("invoice-draft-gone", None))?;
    let records = super::all_in(dir);
    // Issued already (a crash after its record, before its draft went):
    // taken out, never issued twice.
    if let Some(done) = records.iter().find(|r| !r.number.is_empty() && r.draft == id) {
        let _ = super::remove_draft(dir, id);
        let mut args = crate::i18n::args();
        args.set("number", done.number.clone());
        return Err(tr.text("invoice-draft-issued", Some(&args)));
    }
    // A copy the sync made of a draft issued since: kept for you to read, never issued.
    if let Some(done) = records.iter().find(|r| !r.number.is_empty() && !invoice.copy_of.is_empty() && r.draft == invoice.copy_of) {
        let mut args = crate::i18n::args();
        args.set("number", done.number.clone());
        return Err(tr.text("invoice-draft-copy-issued", Some(&args)));
    }
    // Written by a newer Sioul: issued there, never here without what it holds.
    if invoice.newer {
        return Err(tr.text("invoice-draft-newer", None));
    }
    if let Some((clients, contacts)) = steps.clients {
        refresh_client(&mut invoice, clients, contacts);
    }
    // For a client whose file holds a health mark: kept from agents, its file named by its number alone.
    invoice.health = invoice.health || crate::clients::protect::health_at_issue(&crate::clients::door::opening(), &invoice.client_ref, &invoice.contact, steps.clients.map(|(clients, _)| clients));
    invoice.date = day.to_string();
    // Its deposits as they are now: what is left of each after its credit notes.
    recite(&mut invoice, &records);
    refresh(&mut invoice, s, day);
    let blocking: Vec<String> = checks(&invoice, s, day, &records).into_iter().filter(|c| c.required).map(|c| c.text).collect();
    if !blocking.is_empty() {
        return Err(blocking.join("\n"));
    }
    let pattern = pattern_of(s, invoice.kind)?;
    let time_folder = steps.time.map(Path::to_path_buf).unwrap_or_else(crate::timelog::folder);
    // The marks made again from the records first: a crash between a record and its marks bills nothing twice.
    reconcile_marks(&records, &time_folder)?;
    // Time billed meanwhile (on another device, by another draft) is never billed twice.
    let sessions = invoice.sessions();
    if invoice.kind != Kind::CreditNote && !sessions.is_empty() {
        let by_records = billed(&records);
        let marked = crate::timelog::sessions_in(&time_folder).into_iter().filter(|x| !x.invoice.is_empty() && sessions.contains(&x.key())).map(|x| x.invoice);
        if let Some(number) = sessions.iter().find_map(|k| by_records.get(k).cloned()).or_else(|| marked.into_iter().next()) {
            let mut args = crate::i18n::args();
            args.set("number", number);
            return Err(tr.text("invoice-time-billed-meanwhile", Some(&args)));
        }
    }
    let freed = if invoice.kind == Kind::CreditNote { freed_sessions(&invoice, &records) } else { Vec::new() };
    // Numbered and kept under the numbering's lock: one number at a time on this device.
    let lock = dir.join(series::LOCK);
    let mut set_aside = Vec::new();
    let (given, before) = crate::filelock::with_lock(&lock, || -> Result<(u64, Option<series::Last>), String> {
        // What an issue stopped halfway left, finished or set aside first: it never blocks a number.
        set_aside = super::kept::tidy(dir, tr);
        let journal = Journal::load(dir);
        let records = super::records_in(dir);
        let (number, n) = series::next(&pattern, day, &journal, &records, &numbering.claimed);
        invoice.number = number.clone();
        invoice.series = pattern.sequence(day);
        invoice.period_default = false;
        // Its file first, waiting beside it: the record names it with its
        // SHA-256, and is the commit; the file takes its name after.
        super::kept::write(dir, &mut invoice, &super::pdf::Options { qr: s.qr != Some(false) })?;
        let written = match super::write_new(dir, &invoice) {
            Ok(path) => path,
            Err(e) => {
                super::kept::remove(dir, &invoice);
                return Err(e);
            }
        };
        if let Err(e) = super::kept::commit(dir, &invoice) {
            let _ = super::remove_issued(dir, &invoice.number);
            super::kept::remove(dir, &invoice);
            return Err(e);
        }
        // The record's own SHA-256, of its bytes as written: it is never
        // written again, so bytes that differ later were changed outside
        // Sioul (`invoice::outside`). Kept in this device's journal, with its number.
        let record_sha = crate::files::read(&written).map(|bytes| super::kept::sha256(&bytes)).unwrap_or_default();
        let mut own = Journal::own(dir, &numbering.device).unwrap_or_default();
        let before = own.series.get(&invoice.series).cloned();
        own.series.insert(invoice.series.clone(), series::Last { last: n, number: number.clone(), day: day.to_string() });
        if !record_sha.is_empty() {
            own.records.insert(number, record_sha);
        }
        if let Err(e) = own.save(dir, &numbering.device) {
            let _ = super::remove_issued(dir, &invoice.number);
            super::kept::remove(dir, &invoice);
            return Err(e);
        }
        Ok((n, before))
    })?;
    // A later step failed: the record taken out and, under the numbering's
    // lock again, this number taken out of this device's journal, only
    // while the journal still says it: a number another process gave
    // meanwhile stays, and the gap is said (`series::missing`).
    let undo_record = |also: &dyn Fn()| {
        also();
        crate::filelock::with_lock(&lock, || {
            let _ = super::remove_issued(dir, &invoice.number);
            super::kept::remove(dir, &invoice);
            let _ = forget_number(dir, &numbering.device, &invoice.series, &invoice.number, given, before.as_ref());
        });
    };
    // Its time marked, or a credit note's cancelled time freed.
    let number = invoice.number.clone();
    let corrected = invoice.corrects.as_ref().map(|c| c.number.clone()).unwrap_or_default();
    let marked = if invoice.kind == Kind::CreditNote {
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
    let mut origin = crate::i18n::args();
    origin.set("number", number.clone());
    let origin = tr.text("invoice-budget-origin", Some(&origin));
    // Expected in the project's budget until paid, in euros (the ledger's
    // money: an invoice in another currency is not expected there, and no
    // bank line in euros ever joins it); a credit note lowers the credited
    // invoice's line instead, so that its money is no longer expected.
    let mut line_key = None;
    let mut lowered = None;
    if invoice.kind == Kind::CreditNote {
        if let Some((ledger, _)) = steps.ledger {
            match crate::budget::lower_planned(ledger, &crate::links::invoice_uri(&corrected), crate::money::Money(invoice.totals.due)) {
                Ok(was) => lowered = was.map(|w| (ledger, w)),
                Err(e) => {
                    undo_record(&unmark);
                    return Err(e);
                }
            }
        }
    } else if invoice.currency.eq_ignore_ascii_case("EUR")
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
        if let Err(e) = crate::budget::record_line(ledger, &line, &origin) {
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
        if let Some((ledger, (was, gone))) = &lowered {
            let _ = crate::budget::restore_lowered(ledger, was, *gone, &origin);
        }
        undo_record(&unmark);
        return Err(e);
    }
    invoice.said = set_aside;
    Ok(invoice)
}

/// A number taken out of this `device`'s journal after its issue failed
/// halfway (under the numbering's lock): its sequence put back to `before`,
/// only while the journal still says `given`; a number another process
/// gave meanwhile stays, and the gap is said (`series::missing`).
fn forget_number(dir: &Path, device: &str, sequence: &str, number: &str, given: u64, before: Option<&series::Last>) -> Result<(), String> {
    let Some(mut own) = Journal::own(dir, device) else { return Ok(()) };
    if own.series.get(sequence).is_none_or(|l| l.last != given) {
        return Ok(());
    }
    match before {
        Some(last) => own.series.insert(sequence.to_string(), last.clone()),
        None => own.series.remove(sequence),
    };
    // Its record's hash goes with it: the record is taken out too.
    own.records.remove(number);
    own.save(dir, device)
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
    // A line's time is the invoice line's with the same time, or, on a
    // format-1 invoice, whose lines hold none, the one of its name.
    let older = invoice.older.is_some();
    credit
        .lines
        .iter()
        .filter(|l| !l.sessions.is_empty() && invoice.lines.iter().any(|o| (o.sessions == l.sessions || (older && o.description == l.description)) && l.amount >= o.amount))
        .flat_map(|l| l.sessions.iter().cloned())
        .collect()
}

/// The invoice `number` of `dir` sent on `day`, in this `device`'s life
/// (`[[sent]]`): the mail "Write the mail" made for it left Sioul. Said
/// once per device; the first day any device said counts.
pub fn mark_sent(dir: &Path, device: &str, number: &str, day: Date) -> Result<(), String> {
    lives::change_own(dir, device, |book| {
        if !book.sent.iter().any(|s| s.invoice == number) {
            book.sent.push(lives::Sent { invoice: number.to_string(), date: day.to_string() });
        }
    })
}

/// Why no payment goes on an invoice, in words; None when one may: a
/// credit note takes money paid back only while it has some to pay back
/// (`Invoice::pay_back_share`).
fn unpayable(invoice: &Invoice, all: &[Invoice], tr: &Translator) -> Option<String> {
    let mut args = crate::i18n::args();
    args.set("number", invoice.corrects.as_ref().map(|c| c.number.clone()).unwrap_or_default());
    (invoice.kind == Kind::CreditNote && invoice.pay_back_share(all) == 0).then(|| tr.text("invoice-nothing-to-pay-back", Some(&args)))
}

/// What is left to pay on `invoice` (on a credit note: to pay back),
/// "Paid" aside: what a payment with an amount may still be.
fn left_by_amounts(invoice: &Invoice, all: &[Invoice]) -> i64 {
    let target = if invoice.kind == Kind::CreditNote { invoice.pay_back_share(all) } else { invoice.totals.due - invoice.credited(all) };
    let said: i64 = invoice.payments.iter().filter(|p| !p.id.starts_with("settled:") && !p.id.starts_with("older:")).map(|p| p.amount).sum();
    (target - said).max(0)
}

/// A payment said by hand on the issued invoice `number` of `dir`, with
/// its amount (on a credit note: money paid back), written in this
/// `device`'s life (`lives`): never more than is left, "Paid" aside. Its
/// id is made of what it says, so that the same payment said on another
/// device counts once. Returns the payment, with its id.
pub fn add_payment(dir: &Path, device: &str, number: &str, payment: Payment, tr: &Translator) -> Result<Payment, String> {
    let all = super::all_in(dir);
    let invoice = all.iter().find(|i| i.number == number && !i.unreadable).ok_or_else(|| tr.text("invoice-gone", None))?;
    if let Some(why) = unpayable(invoice, &all, tr) {
        return Err(why);
    }
    let left = left_by_amounts(invoice, &all);
    if payment.amount <= 0 || payment.amount > left {
        let mut args = crate::i18n::args();
        args.set("left", tr.money_in(crate::money::Money(left), &invoice.currency));
        return Err(tr.text("invoice-payment-too-much", Some(&args)));
    }
    let means = if payment.means.trim().is_empty() { "transfer".to_string() } else { payment.means.trim().to_string() };
    let lives = lives::Lives::load(dir);
    let rank = lives.next_rank(&format!("hand:{number}/{}/{}/{means}/", payment.date, payment.amount));
    let id = lives::hand_id(number, &payment.date, payment.amount, &means, rank);
    let entry = lives::Paid { id: id.clone(), invoice: number.to_string(), date: payment.date.clone(), amount: payment.amount, means: means.clone(), line: String::new(), fee: 0 };
    lives::change_own(dir, device, |book| book.payments.push(entry))?;
    Ok(Payment { id, means, line: String::new(), fee: 0, ..payment })
}

/// Entries of the life taken back by their ids (a payment's Undo, "Not
/// paid", "Not the same payment"), whichever device wrote them: a removal
/// in this `device`'s file.
pub fn take_back(dir: &Path, device: &str, ids: &[String]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    lives::change_own(dir, device, |book| {
        for id in ids {
            if !book.removed.iter().any(|r| r.id == *id) {
                book.removed.push(lives::Removed { id: id.clone() });
            }
        }
    })
}

/// "Paid", said by hand on `day` by `means` (on a credit note: "Paid
/// back"): settled, whatever the other payments come to, read each time
/// (`lives`): said on two devices, or before the bank's line comes, it
/// never counts twice. Returns the payment it makes now, None when
/// nothing was left to pay or it was said already.
pub fn pay_rest(dir: &Path, device: &str, number: &str, day: Date, means: &str, tr: &Translator) -> Result<Option<Payment>, String> {
    let all = super::all_in(dir);
    let invoice = all.iter().find(|i| i.number == number && !i.unreadable).ok_or_else(|| tr.text("invoice-gone", None))?;
    if let Some(why) = unpayable(invoice, &all, tr) {
        return Err(why);
    }
    let left = invoice.left(&all);
    let lives = lives::Lives::load(dir);
    if left <= 0 || lives.settled.get(number).is_some_and(|s| !s.is_empty()) {
        return Ok(None);
    }
    let means = if means.is_empty() { "transfer".to_string() } else { means.to_string() };
    let id = lives::settled_id(number, lives.next_rank(&format!("settled:{number}/")));
    let entry = lives::Settled { id: id.clone(), invoice: number.to_string(), date: day.to_string(), means: means.clone() };
    lives::change_own(dir, device, |book| book.settled.push(entry))?;
    Ok(Some(Payment { id, date: day.to_string(), amount: left, means, line: String::new(), fee: 0 }))
}

/// "Not paid" after all: everything said by hand taken back (its payments,
/// "Paid", a format-1 record's own word); what the bank's lines paid stays
/// ("Not the same payment" takes a line's back: `take_back_line`).
pub fn unpay(dir: &Path, device: &str, number: &str, tr: &Translator) -> Result<(), String> {
    let invoice = super::find_in(dir, number).ok_or_else(|| tr.text("invoice-gone", None))?;
    let ids = lives::Lives::load(dir).said_by_hand(number, invoice.older.as_ref().is_some_and(|o| o.paid));
    take_back(dir, device, &ids)
}

/// What the bank line `line` paid on any invoice taken back: the line was
/// not their payment ("Not the same payment"). Returns the ids taken back.
pub fn take_back_line(dir: &Path, device: &str, line: &str) -> Result<Vec<String>, String> {
    let ids = lives::Lives::load(dir).of_line(line);
    take_back(dir, device, &ids)?;
    Ok(ids)
}

/// What one bank line did to your invoices (`take_bank_line`): each
/// payment written with its invoice and amount, what the line paid beyond
/// what they asked, and what a processor kept, said as its fee.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Taken {
    pub entries: Vec<(String, String, i64)>,
    pub overpaid: i64,
    pub fee: i64,
}

impl Taken {
    pub fn ids(&self) -> Vec<String> {
        self.entries.iter().map(|(_, id, _)| id.clone()).collect()
    }
}

/// A bank line the matching joined to invoices' planned lines.
pub struct BankLine<'a> {
    /// Its key, the same on every device (`movements::line_key`).
    pub key: &'a str,
    /// The day it was paid: at its processor, for what came through one.
    pub day: Date,
    /// What the payer paid: the gross when its export says it, else what came.
    pub amount: i64,
    /// What a processor kept, when its export says it.
    pub fee: i64,
    pub currency: &'a str,
    /// For a line that came through a processor whose export said only
    /// what came: the most it usually keeps of a payment of so many cents
    /// (its published rule).
    pub usual_fee: Option<&'a dyn Fn(i64) -> i64>,
}

/// A bank line that the matching joined to the planned lines of the
/// invoices `numbers` (several, when one transfer pays several): split
/// across them, the oldest first, each up to what it is owed, what the
/// bank's lines and cash paid it aside (a payment said by hand otherwise
/// takes this line in as its evidence when read: `lives`); nothing on an
/// invoice in another currency, a credit note, or one this line paid
/// before (or was said not to pay). Through a processor whose export said
/// only what came, a line short of what is owed by at most its usual fee
/// settles them, the difference its fee. What remains is said as paid
/// beyond. Written in one step in this `device`'s life, each share keyed by
/// the line, so that the same line taken in on two devices counts once.
pub fn take_bank_line(dir: &Path, device: &str, numbers: &[String], line: &BankLine) -> Result<Taken, String> {
    let all = super::all_in(dir);
    let lives = lives::Lives::load(dir);
    let mut invoices: Vec<&Invoice> = numbers.iter().filter_map(|n| all.iter().find(|i| i.number == *n && !i.unreadable)).collect();
    invoices.sort_by(|a, b| (a.date.as_str(), a.number.as_str()).cmp(&(b.date.as_str(), b.number.as_str())));
    invoices.dedup_by(|a, b| a.number == b.number);
    let owed: Vec<(&Invoice, i64)> = invoices
        .into_iter()
        .filter(|i| i.kind != Kind::CreditNote && i.currency.eq_ignore_ascii_case(line.currency) && !lives.knows(&lives::bank_id(line.key, &i.number)))
        .map(|i| (i, (i.totals.due - i.credited(&all) - lives.bound(&i.number)).max(0)))
        .collect();
    let total: i64 = owed.iter().map(|(_, o)| o).sum();
    let (mut rest, mut fee) = (line.amount.max(0), line.fee.max(0));
    if fee == 0
        && let Some(usual) = line.usual_fee
        && rest < total
        && total - rest <= usual(total)
    {
        fee = total - rest;
        rest = total;
    }
    let mut taken = Taken { fee, ..Taken::default() };
    let mut written = Vec::new();
    for (invoice, owed) in owed {
        let pay = owed.min(rest);
        if pay <= 0 {
            continue;
        }
        let id = lives::bank_id(line.key, &invoice.number);
        let share_fee = if written.is_empty() { fee } else { 0 };
        written.push(lives::Paid { id: id.clone(), invoice: invoice.number.clone(), date: line.day.to_string(), amount: pay, means: "transfer".into(), line: line.key.to_string(), fee: share_fee });
        taken.entries.push((invoice.number.clone(), id, pay));
        rest -= pay;
    }
    taken.overpaid = rest;
    if !written.is_empty() {
        lives::change_own(dir, device, |book| book.payments.extend(written))?;
    }
    Ok(taken)
}

/// Of a payment of `amount` on `invoice`, what is turnover excluding VAT,
/// as declarations and the micro ceilings count it: its share of the total
/// without tax; money paid back on a credit note is a negative receipt.
pub fn received_without_tax(invoice: &Invoice, amount: i64) -> i64 {
    let sign = if invoice.kind == Kind::CreditNote { -1 } else { 1 };
    if invoice.totals.with_tax == 0 || invoice.totals.tax == 0 {
        return sign * amount;
    }
    sign * i64::try_from(rounded(i128::from(amount) * i128::from(invoice.totals.without_tax), i128::from(invoice.totals.with_tax))).unwrap_or(amount)
}

/// The turnover received from `from` to `to` (both days counted), excluding
/// VAT, by the nature of what was sold: payments on invoices and deposits,
/// money paid back on credit notes taken off on its day (a negative receipt).
pub fn received(invoices: &[Invoice], from: Date, to: Date) -> BTreeMap<&'static str, i64> {
    let mut out: BTreeMap<&'static str, i64> = BTreeMap::new();
    for invoice in invoices.iter().filter(|i| !i.number.is_empty()) {
        let goods: i64 = invoice.lines.iter().filter(|l| l.nature == Nature::Goods).map(|l| l.amount).sum();
        let base = invoice.totals.lines;
        for p in invoice.payments.iter().filter(|p| p.date.parse::<Date>().is_ok_and(|d| d >= from && d <= to)) {
            let without = received_without_tax(invoice, p.amount);
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

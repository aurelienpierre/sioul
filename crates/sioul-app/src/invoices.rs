// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Money ▸ Invoices (docs/projects.md, "Invoices in the window"; the
//! invoicing design §9): every invoice and draft by date, with where it
//! stands in words (a draft, not paid yet, partly paid, paid, credited, to
//! pay back: nothing "overdue"), filtered by client, year and status; the
//! editor of a draft, saved as it is typed (`invoice::edit`), with its
//! checks and its preview; issuing; "Paid…" with its day and its means,
//! partial payments adding up, each with Undo; "Credit note…", "Deposit
//! invoice…", a final invoice deducting its deposits; "Write the mail", the
//! invoice marked sent when that mail leaves Sioul (`sent_with`); Settings
//! ▸ Invoices (`settings_view`, `set_setting`); client profiles from
//! Contacts and Projects; the declarations said "Declared".
//!
//! Every answer is JSON for a page, its sentences in your language; what
//! went wrong is {"error"}.

use crate::backend::{QtThread, Shared, config_path, load_config, say, tr};
use crate::projects;
use crate::work::{self, loaded};
use jiff::civil::Date;
use serde::Deserialize;
use serde_json::{Value, json};
use sioul_core::budget;
use sioul_core::clients::{self, Clients, Profile};
use sioul_core::config::{InvoiceSettings, SettingValue};
use sioul_core::i18n::Translator;
use sioul_core::invoice::{self, Invoice, Kind, Status, edit, life, presets, units};
use sioul_core::money::Money;
use std::sync::Arc;

fn today() -> Date {
    sioul_core::zone::now().date()
}

/// What the pages read (contacts among them), as last read: an editor's
/// save never reads the stores again on the window's thread (the review of
/// phase 3, F7); read once when nothing was read yet.
fn world(shared: &Shared) -> Arc<sioul_core::links::Loaded> {
    work::loaded_known(shared).unwrap_or_else(|| loaded(shared))
}

/// A duration that never breaks inside: "2 h 30".
fn duration(minutes: u32) -> String {
    sioul_core::timereport::duration(minutes).replace(' ', "\u{a0}")
}

/// The client profiles of the notes folder, as they are now.
fn clients_now() -> Clients {
    load_config().records_root().and_then(|root| Clients::load(&root).ok()).unwrap_or_default()
}

fn refused(error: impl Into<String>) -> String {
    json!({ "error": error.into() }).to_string()
}

fn day_of(text: &str) -> Option<Date> {
    text.trim().get(..10).and_then(|d| d.parse().ok())
}

/// "12 November 2026".
fn long(text: &str) -> String {
    day_of(text).map(|d| sioul_core::books::long_day(tr(), d)).unwrap_or_default()
}

/// "Thu 12 Nov".
fn short(text: &str) -> String {
    day_of(text).map(|d| tr().day_short(d)).unwrap_or_default()
}

fn money_in(cents: i64, currency: &str) -> String {
    tr().money_in(Money(cents), currency)
}

/// Where an invoice stands, as the window says it (design §2.5): a draft,
/// not paid yet (issued, or sent), partly paid, paid, credited, a credit
/// note, to pay back. Never "late".
fn status_of(i: &Invoice, all: &[Invoice]) -> &'static str {
    match i.status(all) {
        Status::Draft => "draft",
        Status::Issued | Status::Sent => "unpaid",
        Status::PartlyPaid => "partly-paid",
        Status::Paid => "paid",
        Status::Credited => "credited",
        Status::CreditNote => "credit-note",
        Status::ToPayBack => "to-pay-back",
    }
}

/// The statuses the list filters by, in their order.
const STATUSES: [&str; 7] = ["draft", "unpaid", "partly-paid", "paid", "credited", "credit-note", "to-pay-back"];

/// "Invoice No. 2026-014", "Draft credit note".
fn title_of(i: &Invoice) -> String {
    if i.number.is_empty() { tr().text(&format!("invoice-draft-title-{}", i.kind.id()), None) } else { say(&format!("invoice-title-{}", i.kind.id()), &[("number", i.number.clone())]) }
}

/// A draft's day: when it was made (its id begins with it).
fn draft_day(i: &Invoice) -> String {
    let digits: String = i.draft.chars().take(8).collect();
    if digits.len() == 8 && digits.chars().all(|c| c.is_ascii_digit()) { format!("{}-{}-{}", &digits[..4], &digits[4..6], &digits[6..]) } else { String::new() }
}

/// One row of the list.
fn row(i: &Invoice, all: &[Invoice], outside: bool) -> Value {
    let status = status_of(i, all);
    let day = if i.number.is_empty() { draft_day(i) } else { i.date.get(..10).unwrap_or("").to_string() };
    let left = i.left(all);
    let left_words = match status {
        "partly-paid" => say("invoices-left-to-pay", &[("amount", money_in(left, &i.currency))]),
        "to-pay-back" => say("invoices-left-to-pay-back", &[("amount", money_in(left, &i.currency))]),
        _ => String::new(),
    };
    json!({
        "key": if i.number.is_empty() { format!("draft:{}", i.draft) } else { i.number.clone() },
        "draft": if i.number.is_empty() { i.draft.clone() } else { String::new() },
        "number": i.number,
        "kind": i.kind.id(),
        "title": title_of(i),
        "client": i.who().lines().next().unwrap_or("").to_string(),
        "project": i.project_title,
        "day": day,
        "date": short(&day),
        "year": day.get(..4).unwrap_or(""),
        "total": money_in(i.totals.due, &i.currency),
        "status": status,
        "status_words": tr().text(&format!("invoices-status-{status}"), None),
        "left": left_words,
        "sent": if i.sent.is_empty() || i.number.is_empty() { String::new() } else { say("invoices-sent-on", &[("day", short(&i.sent))]) },
        "outside": outside,
        "copy_of": i.copy_of,
        // A draft a newer Sioul wrote: read only, in the list too (the second review of phase 3, R13).
        "read_only": i.newer && i.number.is_empty(),
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Asked {
    client: String,
    year: String,
    status: String,
}

/// Money ▸ Invoices: every draft (the newest first), then every invoice by
/// its day, the newest first; filtered by `asked` ({client, year, status});
/// the clients, years and statuses there are; what the files say besides
/// (a number with no record here, one number given twice, a newer Sioul's
/// file).
pub(crate) fn list(asked: &str) -> String {
    let asked: Asked = serde_json::from_str(asked).unwrap_or_default();
    let folder = invoice::folder();
    let all = invoice::all_in(&folder);
    let journal = invoice::series::Journal::load(&folder);
    let mut rows: Vec<Value> = invoice::drafts_in(&folder).iter().map(|d| row(d, &all, false)).collect();
    let mut issued: Vec<&Invoice> = all.iter().filter(|i| !i.unreadable && !i.number.is_empty()).collect();
    issued.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| invoice::natural(&b.number, &a.number)));
    rows.extend(issued.iter().map(|i| row(i, &all, invoice::outside(&folder, i, &journal).any())));
    let mut clients: Vec<String> = rows.iter().filter_map(|r| r["client"].as_str()).filter(|c| !c.trim().is_empty()).map(str::to_string).collect();
    clients.sort_by_key(|c| c.to_lowercase());
    clients.dedup();
    let mut years: Vec<String> = rows.iter().filter_map(|r| r["year"].as_str()).filter(|y| !y.is_empty()).map(str::to_string).collect();
    years.sort_by(|a, b| b.cmp(a));
    years.dedup();
    let statuses: Vec<Value> = STATUSES.iter().filter(|s| rows.iter().any(|r| r["status"] == **s)).map(|s| json!({ "id": s, "label": tr().text(&format!("invoices-status-{s}"), None) })).collect();
    let shown: Vec<Value> = rows.into_iter().filter(|r| (asked.client.is_empty() || r["client"] == asked.client.as_str()) && (asked.year.is_empty() || r["year"] == asked.year.as_str()) && (asked.status.is_empty() || r["status"] == asked.status.as_str())).collect();
    let mut said = missing_words(invoice::series::missing(&journal, &all));
    said.extend(invoice::twice_in(&folder).into_iter().map(|t| say("invoice-cli-twice", &[("number", t.number), ("first", t.clients.0), ("second", t.clients.1), ("file", t.copy.display().to_string())])));
    said.extend(invoice::lives::Lives::load(&folder).newer.iter().chain(invoice::series::Journal::newer(&folder).iter()).chain(invoice::newer_in(&folder).iter()).map(|p| say("invoice-cli-newer", &[("file", p.display().to_string())])));
    json!({ "rows": shown, "clients": clients, "years": years, "statuses": statuses, "said": said, "declares": !all.is_empty() }).to_string()
}

/// The numbers with no record here, said in few lines: a run of numbers
/// that follow one another as one ("2026-001 to 2026-013 have no record on
/// this device."), a number alone as itself.
fn missing_words(missing: Vec<invoice::series::Missing>) -> Vec<String> {
    let mut out = Vec::new();
    let mut run: Vec<invoice::series::Missing> = Vec::new();
    let flush = |run: &mut Vec<invoice::series::Missing>, out: &mut Vec<String>| {
        match run.as_slice() {
            [] => {}
            [one] => out.push(match &one.number {
                Some(number) => say("invoice-cli-missing", &[("number", number.clone())]),
                None => say("invoice-cli-missing-in", &[("sequence", one.sequence.clone()), ("n", one.counter.to_string())]),
            }),
            [first, .., last] => out.push(match (&first.number, &last.number) {
                (Some(a), Some(b)) => say("invoices-missing-run", &[("first", a.clone()), ("last", b.clone())]),
                _ => say("invoices-missing-run-in", &[("sequence", first.sequence.clone()), ("first", first.counter.to_string()), ("last", last.counter.to_string())]),
            }),
        }
        run.clear();
    };
    for m in missing {
        if run.last().is_some_and(|l| l.sequence != m.sequence || l.counter + 1 != m.counter) {
            flush(&mut run, &mut out);
        }
        run.push(m);
    }
    flush(&mut run, &mut out);
    out
}

// ---------------------------------------------------------------- the editor

/// A line's tax in words: "VAT 20 %", or its category's words.
fn tax_words(line: &invoice::Line) -> String {
    match line.category.as_str() {
        "S" => say("invoices-tax-rate", &[("rate", tr().percent_bp(line.rate))]),
        other => tr().text(&format!("invoices-category-{}", other.to_lowercase()), None),
    }
}

/// A line as the editor shows it: what `edit::LineEdit` sends back, and its words.
fn line_view(i: usize, l: &invoice::Line, currency: &str) -> Value {
    let time = edit::timed(l);
    json!({
        "id": l.id,
        "from": i,
        "description": l.description,
        "details": l.details,
        "quantity": l.quantity as f64 / 1000.0,
        "unit": l.unit,
        // Its unit by its name, whatever it is: one the list does not offer is shown as itself (F17).
        "unit_words": if units::CODES.contains(&l.unit.as_str()) { tr().text(&format!("invoices-unit-{}", l.unit), None) } else { l.unit.clone() },
        "price": l.price as f64 / 100.0,
        "discount": l.discount as f64 / 100.0,
        "nature": if l.nature == invoice::Nature::Goods { "goods" } else { "service" },
        "activity": l.activity,
        "own_tax": l.own_tax,
        "category": l.category,
        "rate": l.rate as f64 / 100.0,
        "exemption": l.exemption,
        "time": time,
        "time_words": l.minutes().filter(|_| time).map(duration).unwrap_or_default(),
        "amount": money_in(l.amount, currency),
        "tax_words": tax_words(l),
    })
}

/// The totals as the editor says them, in the window's language.
fn totals_view(d: &Invoice) -> Vec<Value> {
    let t = &d.totals;
    let money = |c: i64| money_in(c, &d.currency);
    let mut out = Vec::new();
    let taxed = t.tax != 0 || t.breakdown.iter().any(|b| b.rate > 0);
    if taxed {
        out.push(json!({ "label": tr().text("invoice-total-without-tax", None), "amount": money(t.without_tax) }));
        for b in t.breakdown.iter().filter(|b| b.tax != 0 || b.rate > 0) {
            out.push(json!({ "label": say("invoice-vat-on", &[("rate", tr().percent_bp(b.rate)), ("base", money(b.base))]), "amount": money(b.tax) }));
        }
        out.push(json!({ "label": tr().text("invoice-total-with-tax", None), "amount": money(t.with_tax) }));
    } else {
        out.push(json!({ "label": tr().text("invoice-total", None), "amount": money(t.with_tax) }));
    }
    if t.deposits != 0 {
        out.push(json!({ "label": say("invoice-deposits-deducted", &[("numbers", d.deposits.iter().map(|c| c.number.clone()).collect::<Vec<_>>().join(", "))]), "amount": money(-t.deposits) }));
    }
    out.push(json!({ "label": tr().text("invoice-amount-due", None), "amount": money(t.due), "last": true }));
    out
}

/// What the editor offers: units, natures, VAT categories and rates, activities, languages.
fn choices(s: &InvoiceSettings, issuer: &invoice::Issuer) -> Value {
    let preset = presets::get(&issuer.preset);
    let language = tr().language().to_string();
    // Minutes are time's own unit, priced by the hour: a line in minutes shows them by their name (`line_view`'s `unit_words`), never offered for another.
    let units: Vec<Value> = units::CODES.iter().filter(|c| **c != "MIN").map(|c| json!({ "code": c, "label": tr().text(&format!("invoices-unit-{c}"), None) })).collect();
    let categories: Vec<Value> = ["S", "Z", "E", "AE", "K", "G", "O"].iter().map(|c| json!({ "code": c, "label": tr().text(&format!("invoices-category-{}", c.to_lowercase()), None) })).collect();
    let rates: Vec<Value> = preset.rates_on(today()).iter().map(|r| json!({ "rate": r.rate as f64 / 100.0, "label": format!("{} · {}", tr().percent_bp(r.rate), r.name.get(&language)) })).collect();
    let activities: Vec<Value> = preset.activity.iter().filter(|a| s.activity.is_empty() || s.activity.contains(&a.id)).map(|a| json!({ "id": a.id, "label": a.name.get(&language) })).collect();
    let kinds: Vec<Value> = ["business", "consumer", "public"].iter().map(|k| json!({ "id": k, "label": tr().text(&format!("invoices-client-kind-{k}"), None) })).collect();
    let languages: Vec<Value> = ["en", "fr"].iter().map(|l| json!({ "id": l, "label": tr().text(&format!("invoices-language-{l}"), None) })).collect();
    json!({
        "units": units,
        "categories": categories,
        "rates": rates,
        "activities": activities,
        "languages": languages,
        "kinds": kinds,
    })
}

/// A draft as the editor shows it: its client (its profile, its kind and
/// country), its days, its lines, its notes and terms, the deposits it may
/// deduct, its totals, its checks (required, then recommended and notes),
/// its preview in Qt's HTML; made again first from its client's profile
/// and the settings of today, as it would be issued.
fn draft_view(mut d: Invoice, shared: &Shared) -> Value {
    let config = load_config();
    let s = &config.invoice;
    let today = today();
    let folder = invoice::folder();
    let records = invoice::all_in(&folder);
    let clients = clients_now();
    let world = world(shared);
    // A draft a newer Sioul wrote is shown as it is, never made again here (F13).
    if !d.newer {
        life::refresh_client(&mut d, &clients, &world.contacts);
        // Its deposits as they are now, as Issue would deduct them (R8).
        life::recite(&mut d, &records);
        life::refresh(&mut d, s, today);
    }
    let checks = life::checks(&d, s, today, &records);
    let profile = clients::resolve(&d.client_ref, &clients, &world.contacts).1.cloned();
    let country = life::client_country(&d.client);
    let country_name = clients::country(&country).map_or_else(|| country.clone(), |c| c.name(tr().language()).to_string());
    // The client's card, named either way (`contacts::card_address`: Sioul's own address, or `urn:uuid:`).
    let card_of = |reference: &str| sioul_core::contacts::card_address(&world.contacts, reference).map(|a| sioul_core::links::id_of(&a));
    let contact = card_of(&d.client_ref).or_else(|| profile.as_ref().and_then(|p| card_of(&p.contact))).unwrap_or_default();
    let cited: Vec<&str> = d.deposits.iter().map(|c| c.number.as_str()).collect();
    // The deposits of this client and currency not deducted yet, for what is left of each after its credit notes (F4).
    let mut deposits: Vec<Value> = life::deposits_open(&d, &records)
        .into_iter()
        .map(|r| {
            let left = life::deposit_left(r, &records);
            // Less than it billed: its credit notes gave the rest back, said on its row (R8).
            let why = if left < r.totals.with_tax { say("invoices-deposit-less", &[("whole", money_in(r.totals.with_tax, &r.currency))]) } else { String::new() };
            json!({ "number": r.number, "date": short(&r.date), "amount": money_in(left, &r.currency), "why": why, "cited": cited.contains(&r.number.as_str()) })
        })
        .collect();
    // Those it cites already, whatever happened to them since (a check says what is wrong with them).
    let gone: Vec<Value> = d.deposits.iter().filter(|c| !deposits.iter().any(|v| v["number"] == c.number.as_str())).map(|c| json!({ "number": c.number, "date": short(&c.date), "amount": money_in(c.amount, &d.currency), "cited": true })).collect();
    deposits.extend(gone);
    let required: Vec<String> = checks.iter().filter(|c| c.required).map(|c| c.text.clone()).collect();
    let said: Vec<String> = checks.iter().filter(|c| !c.required).map(|c| c.text.clone()).collect();
    let language = if d.language.is_empty() { tr().language().to_string() } else { d.language.clone() };
    json!({
        "id": d.draft,
        // The draft as read, so that a change made elsewhere meanwhile is taken in and said (F2).
        "revision": invoice::draft_revision(&folder, &d.draft),
        "read_only": d.newer,
        "read_only_words": if d.newer { tr().text("invoice-draft-newer", None) } else { String::new() },
        // The time taken out of it, which Put back gives it again (R15).
        "set_aside": if d.set_aside.is_empty() { String::new() } else { say("invoices-set-aside", &[("time", duration(life::set_aside_minutes(&d)))]) },
        "kind": d.kind.id(),
        "title": title_of(&d),
        "copy_of": d.copy_of,
        "project": d.project_title,
        "client": {
            "ref": d.client_ref,
            "name": d.client.name,
            "first": d.client.first_line(),
            "kind": d.client.kind,
            "kind_words": tr().text(&format!("invoices-client-kind-{}", d.client.kind), None),
            "country": country,
            "country_name": country_name,
            "email": d.client.email,
            "address": d.client.address.text("", &language),
            "profile": profile.as_ref().map(|p| p.id.clone()).unwrap_or_default(),
            "contact": contact,
        },
        "issue_day": long(&today.to_string()),
        "period_from": d.period_from,
        "period_to": d.period_to,
        "period_default": d.period_default,
        "terms_days": d.terms.days,
        "end_of_month": d.terms.end_of_month,
        "due": long(&d.due),
        "order": d.order,
        "language": language,
        "currency": d.currency,
        "lines": d.lines.iter().enumerate().map(|(i, l)| line_view(i, l, &d.currency)).collect::<Vec<_>>(),
        "notes": d.notes.iter().filter(|n| n.legal.is_empty()).map(|n| n.text.clone()).collect::<Vec<_>>(),
        "legal": d.notes.iter().filter(|n| !n.legal.is_empty()).map(|n| n.text.clone()).collect::<Vec<_>>(),
        "corrects": d.corrects.as_ref().map(|c| say("invoice-corrects", &[("number", c.number.clone()), ("date", long(&c.date))])).unwrap_or_default(),
        "deposits": if d.kind == Kind::Invoice { deposits } else { Vec::new() },
        "totals": totals_view(&d),
        "required": required,
        "said": said,
        "preview": invoice::page(&d, tr()),
        "choices": choices(s, &d.issuer),
    })
}

/// A draft for the editor, by its id; {"error"} when it is gone.
pub(crate) fn draft(shared: &Shared, id: &str) -> String {
    // Reading never writes: a draft from before its lines had ids gets them in memory, alike on every device (`invoice::draft_in`; R3).
    match invoice::draft_in(&invoice::folder(), id) {
        Some(d) => draft_view(d, shared).to_string(),
        None => refused(tr().text("invoice-draft-gone", None)),
    }
}

/// The drafts this window changed since the last exchange with your other
/// devices, each with its revision as it was last written here: a draft
/// travels whole, the later file replacing the other, so one of these that
/// the exchange replaced lost what was changed here, and that is said
/// (`after_exchange`; the second review of phase 3, R2).
static CHANGED: std::sync::Mutex<std::collections::BTreeMap<String, String>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The draft `id` was just written here.
pub(crate) fn changed_here(id: &str) {
    if let Ok(mut changed) = CHANGED.lock() {
        changed.insert(id.to_string(), invoice::draft_revision(&invoice::folder(), id));
    }
}

/// An exchange with your other devices ended (`wrote`: it wrote invoices'
/// files here). The drafts changed here before it either went out as they
/// were, or were replaced by another device's later version: those are
/// named in the status line, their version of here kept by the sharing
/// among the earlier versions. Changes made here after an exchange and
/// replaced later are a later version's, not a loss: nothing is said.
pub(crate) fn after_exchange(qt: &QtThread, wrote: bool) {
    let tracked = match CHANGED.lock() {
        Ok(mut changed) => std::mem::take(&mut *changed),
        Err(_) => return,
    };
    if !wrote || tracked.is_empty() {
        return;
    }
    let folder = invoice::folder();
    let replaced = replaced_among(&folder, &tracked, &CHANGED.lock().map(|c| c.clone()).unwrap_or_default());
    if !replaced.is_empty() {
        crate::backend::set_status(qt, say("invoices-draft-replaced", &[("n", replaced.len().to_string()), ("drafts", replaced.join(", "))]));
    }
}

/// Of the drafts `tracked` (id, revision written here), those whose file is
/// another now, and not by a newer write of here (`since`): each by its
/// client's name, else its title.
fn replaced_among(folder: &std::path::Path, tracked: &std::collections::BTreeMap<String, String>, since: &std::collections::BTreeMap<String, String>) -> Vec<String> {
    tracked
        .iter()
        .filter(|(id, written)| {
            let now = invoice::draft_revision(folder, id);
            !now.is_empty() && now != **written && since.get(*id) != Some(&now)
        })
        .filter_map(|(id, _)| invoice::draft_in(folder, id))
        .map(|d| if d.client.first_line().trim().is_empty() { title_of(&d) } else { d.client.first_line().to_string() })
        .collect()
}

/// The drafts New and "Bill this client" made in this window, each with
/// its revision as made: one left as it was made was never written in.
static FRESH: std::sync::Mutex<std::collections::BTreeMap<String, String>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The editor left a draft (Back, another draft, Quit): one New or "Bill
/// this client" made in this window and nothing was written in since (its
/// file is as it was made) is taken out, without a word (the review of
/// phase 3, F17). A draft changed in any way, or made before, stays.
pub(crate) fn leave_draft(id: &str) {
    let Some(made) = FRESH.lock().ok().and_then(|mut fresh| fresh.remove(id)) else { return };
    let folder = invoice::folder();
    if !made.is_empty() && invoice::draft_revision(&folder, id) == made {
        let _ = invoice::remove_draft(&folder, id);
    }
}

/// What a new draft is made from.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Making {
    /// Who it bills: a profile's id, a contact's address, a name; "" for none yet.
    client: String,
    /// "invoice" or "deposit".
    kind: String,
    /// A deposit: what it is for, its amount, or a share of `of` (a draft's total).
    description: String,
    amount: f64,
    share: f64,
    of: String,
}

/// A new draft (Money ▸ Invoices ▸ New, a contact's "Bill this client",
/// a draft's "Deposit invoice…"): for its client (its profile's language,
/// currency and terms), blank; a deposit invoice of an amount, or of a
/// share of the draft `of` (its total excluding tax), for that draft's
/// client and project. Returns {"draft"} or {"error"}.
pub(crate) fn new_draft(shared: &Arc<Shared>, making: &str) -> String {
    let making: Making = serde_json::from_str(making).unwrap_or_default();
    let result = (|| -> Result<String, String> {
        let config = load_config();
        let s = &config.invoice;
        let today = today();
        let folder = invoice::folder();
        let clients = clients_now();
        let world = world(shared);
        let from = (!making.of.is_empty()).then(|| invoice::draft_in(&folder, &making.of)).flatten();
        let reference = from.as_ref().map_or(making.client.clone(), |d| d.client_ref.clone());
        let mut draft = life::for_client(s, &reference, &clients, &world.contacts, tr().language(), today);
        if let Some(of) = &from {
            draft.project = of.project.clone();
            draft.project_title = of.project_title.clone();
            draft.currency = of.currency.clone();
            draft.language = of.language.clone();
        }
        if making.kind == "deposit" {
            let total = from.as_ref().map_or(0, |d| d.totals.without_tax);
            let amount = edit::deposit_amount(making.amount, making.share, total);
            if amount <= 0 {
                return Err(tr().text("invoices-deposit-no-amount", None));
            }
            let words = if making.description.trim().is_empty() { from.as_ref().and_then(|d| d.lines.first()).map(|l| say("invoices-deposit-on", &[("what", l.description.clone())])).unwrap_or_else(|| tr().text("invoices-deposit-line", None)) } else { making.description.trim().to_string() };
            draft = life::deposit(s, draft, &words, amount, today);
            // The draft it was made from: "Final invoice…" opens it again, the deposit ticked (F10).
            draft.deposit_of = from.as_ref().map(|d| d.draft.clone()).unwrap_or_default();
        }
        let id = invoice::save_draft(&folder, &draft)?.draft;
        // A blank draft is remembered as made: left as it is, it is taken out (`leave_draft`).
        if making.kind != "deposit"
            && let Ok(mut fresh) = FRESH.lock()
        {
            fresh.insert(id.clone(), invoice::draft_revision(&folder, &id));
        }
        Ok(id)
    })();
    match result {
        Ok(id) => json!({ "draft": id }).to_string(),
        Err(e) => refused(e),
    }
}

/// A draft changed as the editor says (`edit::Change`, JSON: only what it
/// changed, lines by their ids, the revision it read), merged into the
/// draft as it is now and saved (`edit::save`), then shown as the editor
/// shows it, with "said": a change made elsewhere meanwhile taken in,
/// deposits no longer deducted, a line taken out elsewhere. Only the
/// invoices' views hear of it: nothing else is read again, the day is never
/// planned again (the review of phase 3, F2, F7). {"error"} when it is gone,
/// or a newer Sioul wrote it.
pub(crate) fn change(shared: &Arc<Shared>, id: &str, change: &str) -> String {
    let change: edit::Change = match serde_json::from_str(change) {
        Ok(c) => c,
        Err(e) => return refused(e.to_string()),
    };
    let config = load_config();
    let s = &config.invoice;
    let folder = invoice::folder();
    let records = invoice::all_in(&folder);
    let clients = clients_now();
    let world = world(shared);
    let cx = edit::Context { settings: s, records: &records, clients: &clients, contacts: &world.contacts, fallback: tr().language(), today: today() };
    match edit::save(&folder, id, &change, &cx) {
        Ok(saved) => {
            changed_here(id);
            let mut said: Vec<String> = Vec::new();
            if saved.elsewhere {
                said.push(tr().text("invoices-changed-elsewhere", None));
            }
            for word in &saved.said {
                said.push(match word {
                    edit::Said::DepositsLeft(numbers) => say("invoices-deposits-left", &[("numbers", numbers.join(", ")), ("n", numbers.len().to_string())]),
                    edit::Said::LineGone => tr().text("invoices-line-gone", None),
                });
            }
            let mut view = draft_view(saved.draft, shared);
            view["elsewhere"] = json!(saved.elsewhere);
            view["said_now"] = json!(said.join(" "));
            view.to_string()
        }
        Err(e) if invoice::draft_in(&folder, id).is_some() => refused(e),
        // Gone (discarded, or issued elsewhere): nothing to try again (R5).
        Err(_) => json!({ "error": gone_words(&folder, id), "gone": true }).to_string(),
    }
}

/// Why the draft `id` is not there: issued meanwhile, under its number, or gone.
fn gone_words(folder: &std::path::Path, id: &str) -> String {
    match invoice::all_in(folder).iter().find(|r| !r.number.is_empty() && r.draft == id) {
        Some(done) => say("invoice-draft-issued", &[("number", done.number.clone())]),
        None => tr().text("invoice-draft-gone", None),
    }
}

/// The time set aside in the draft `id` put back in it (the second review
/// of phase 3, R15): its lines made again from the time noted, joined to
/// the draft's lines of the same words; the draft as the editor shows it.
pub(crate) fn put_back(shared: &Arc<Shared>, id: &str) -> String {
    let result = (|| -> Result<(), String> {
        let config = load_config();
        let s = &config.invoice;
        let folder = invoice::folder();
        let draft = invoice::draft_in(&folder, id).ok_or_else(|| tr().text("invoice-draft-gone", None))?;
        let store = projects::store()?;
        let project = store.get(&draft.project).ok_or_else(|| tr().text("project-gone", None))?.clone();
        let aside: Vec<_> = projects::entries(shared).into_iter().filter(|e| draft.set_aside.contains(&e.key)).collect();
        let records = invoice::all_in(&folder);
        match life::from_time(&aside, &project, Invoice { lines: Vec::new(), ..draft.clone() }, &records, s, today()) {
            Some(more) => {
                projects::add_to_draft(&folder, id, &more, s, today())?;
            }
            // Billed or deleted since: nothing to put back.
            None => {}
        }
        life::change_draft(&folder, id, s, today(), |d| d.set_aside.clear()).map(|_| ())
    })();
    match result {
        Ok(()) => {
            changed_here(id);
            draft(shared, id)
        }
        Err(e) => refused(e),
    }
}

/// A draft taken out (Discard), with Undo in the status line, which keeps it again as it was.
pub(crate) fn discard(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let folder = invoice::folder();
    let Some(before) = invoice::draft_in(&folder, id) else { return tr().text("invoice-draft-gone", None) };
    // A newer Sioul's draft is never taken out here: the removal would travel, and nothing here could write it back as it was (R13).
    if before.newer {
        return tr().text("invoice-draft-newer", None);
    }
    // Its file's own bytes, which Undo writes back as they were.
    let Some(bytes) = invoice::draft_bytes(&folder, id) else { return tr().text("invoice-draft-gone", None) };
    if let Err(e) = invoice::remove_draft(&folder, id) {
        return e;
    }
    let id = id.to_string();
    crate::mail::offer_back(qt, shared, say("invoices-discarded", &[("title", title_of(&before))]), move |_, _| invoice::restore_draft(&invoice::folder(), &id, &bytes).err().unwrap_or_default());
    String::new()
}

/// Issues the draft `id` (the editor's Issue): checked, numbered, its file
/// and its record kept, its time marked, its budget line planned, all or
/// nothing (`invoice::life::issue`); its file copied where invoices go
/// (`projects::printed`). Returns {"number", "pdf", "said"}: the status
/// line's words (a file set aside, the checks' notes, a copy not written);
/// or {"error"}, with "elsewhere" when another computer numbers invoices.
pub(crate) fn issue(qt: &QtThread, shared: &Arc<Shared>, id: &str, revision: &str) -> String {
    // One computer numbers invoices (`sioul_sync::lease`): two at once could give one number twice.
    if let Some(elsewhere) = projects::invoices_held_elsewhere(shared) {
        return elsewhere;
    }
    let result = (|| -> Result<Value, String> {
        let config = load_config();
        let s = &config.invoice;
        let today = today();
        let folder = invoice::folder();
        let records = invoice::all_in(&folder);
        let draft = invoice::draft_in(&folder, id).ok_or_else(|| tr().text("invoice-draft-gone", None))?;
        let clients = clients_now();
        let world = world(shared);
        let store = projects::store().ok();
        let ledger = store.as_ref().map(|st| st.root.join(budget::LEDGER));
        // Expected in its project's budget, as the project's invoices always were.
        let budget_id = store.as_ref().and_then(|st| st.get(&draft.project)).and_then(|p| p.budget.clone()).unwrap_or_default();
        let numbering = life::Numbering { device: projects::device(), sharing: crate::share::configured(), drafts_shared: crate::share::shares("drafts"), claimed: crate::share::claimed_series(projects::INVOICES) };
        let steps = life::Steps { time: None, ledger: ledger.as_deref().map(|l| (l, budget_id.as_str())), clients: Some((&clients, &world.contacts)) };
        // What is numbered is what the editor read: a draft that moved since is read again first (R6).
        let made = match life::issue_as_read(&folder, id, revision, s, today, &numbering, &steps, tr()) {
            Ok(made) => made,
            Err(life::NotIssued::Moved(words)) => return Ok(json!({ "error": words, "moved": true })),
            Err(life::NotIssued::Refused(words)) => return Err(words),
        };
        // The claim says the new number at once, not at the next minute's renewal.
        let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
        let _ = crate::share::keeper(projects::INVOICES, sioul_sync::lease::Rule::StaysPut, active, false);
        let mut said = made.said.clone();
        said.extend(life::checks(&made, s, today, &records).into_iter().filter(|c| !c.required).map(|c| c.text));
        let mut answer = match projects::printed(&made) {
            Ok(answer) => answer,
            // Issued all the same: only its copy is missing, said apart.
            Err(invoice::kept::NotGiven::Copy(path, _)) => {
                said.insert(0, say("invoice-issued-not-copied", &[("number", made.number.clone()), ("path", path.display().to_string())]));
                json!({})
            }
            Err(invoice::kept::NotGiven::Kept(why)) => {
                said.insert(0, why);
                json!({})
            }
        };
        answer["number"] = json!(made.number);
        if !said.is_empty() {
            answer["said"] = json!(said.join(" "));
        }
        Ok(answer)
    })();
    work::refresh(qt, shared);
    crate::backend::show(qt, shared);
    match result {
        Ok(answer) => answer.to_string(),
        Err(error) => refused(error),
    }
}

// ---------------------------------------------------------------- an issued invoice

/// How a payment was made, in words.
fn means_words(means: &str) -> String {
    sioul_core::books::means_words(tr(), means)
}

/// An issued invoice's pane: its words, its lines, its payments, its
/// credit notes, where it stands, what changed outside Sioul (its record
/// or its kept file: `invoice::outside`), and what may be done with it now.
pub(crate) fn show(number: &str) -> String {
    let folder = invoice::folder();
    let all = invoice::all_in(&folder);
    let Some(i) = all.iter().find(|i| i.number == number && !i.unreadable) else { return refused(tr().text("invoice-gone", None)) };
    let journal = invoice::series::Journal::load(&folder);
    let outside = invoice::outside(&folder, i, &journal);
    let mut changed = Vec::new();
    if outside.record {
        changed.push(say("invoices-record-changed", &[("path", folder.join(invoice::file_name(&i.number)).display().to_string())]));
    }
    if outside.file {
        changed.push(say("invoices-file-changed", &[("path", invoice::kept::path(&folder, i).display().to_string())]));
    }
    let status = status_of(i, &all);
    let credit_notes: Vec<String> = all.iter().filter(|c| c.kind == Kind::CreditNote && c.corrects.as_ref().is_some_and(|x| x.number == i.number)).map(|c| c.number.clone()).collect();
    let deducted_by: Vec<String> = all.iter().filter(|f| f.deposits.iter().any(|d| d.number == i.number)).map(|f| f.number.clone()).collect();
    // A final invoice credited whole gives its deposits back (F17).
    let deducted_now = i.kind == Kind::Deposit && !life::deducted_by(i, &all).is_empty();
    let payments: Vec<Value> = i
        .payments
        .iter()
        .map(|p| json!({ "id": p.id, "words": say(if i.kind == Kind::CreditNote { "invoices-paid-back-line" } else { "invoices-paid-line" }, &[("amount", money_in(p.amount, &i.currency)), ("day", long(&p.date)), ("means", means_words(&p.means))]), "bank": !p.line.is_empty() }))
        .collect();
    let left = i.left(&all);
    let lines: Vec<Value> = i.lines.iter().map(|l| json!({ "description": l.description, "amount": money_in(l.amount, &i.currency), "time": l.minutes().filter(|_| !l.sessions.is_empty() || i.older.is_some()).map(duration).unwrap_or_default() })).collect();
    let business = i.kind != Kind::CreditNote;
    json!({
        "number": i.number,
        "kind": i.kind.id(),
        "title": title_of(i),
        "client": i.who(),
        "project": i.project_title,
        "date": long(&i.date),
        "due": if business { long(&i.due) } else { String::new() },
        "total": money_in(i.totals.due, &i.currency),
        "currency": i.currency,
        "status": status,
        "status_words": tr().text(&format!("invoices-status-{status}"), None),
        "left": left as f64 / 100.0,
        // What is left as the window's language writes an amount ("751,66"), to fill Paid… with (F17).
        "left_text": amount_text(left),
        // What is left, once a part is paid (or on a credit note, to pay back).
        "left_words": if left > 0 && matches!(status, "partly-paid" | "to-pay-back") { say(if i.kind == Kind::CreditNote { "invoices-left-to-pay-back" } else { "invoices-left-to-pay" }, &[("amount", money_in(left, &i.currency))]) } else { String::new() },
        "sent": if i.sent.is_empty() { String::new() } else { say("invoices-sent-on", &[("day", long(&i.sent))]) },
        "corrects": i.corrects.as_ref().map(|c| say("invoice-corrects", &[("number", c.number.clone()), ("date", long(&c.date))])).unwrap_or_default(),
        "credit_notes": credit_notes,
        "deducted_by": deducted_by,
        "payments": payments,
        "lines": lines,
        "outside": changed,
        "older": i.older.is_some(),
        "acts": {
            // Paid…: money still expected; Paid back…: a credit note with money to give back.
            "paid": i.kind != Kind::CreditNote && left > 0,
            "paid_back": i.kind == Kind::CreditNote && left > 0,
            "credit": i.kind != Kind::CreditNote && i.left_to_credit(&all) > 0,
            "final": i.kind == Kind::Deposit && !deducted_now && life::deposit_left(i, &all) > 0,
            "mail": !i.file.is_empty() || i.older.is_some(),
        },
    })
    .to_string()
}

/// What "Paid…" says.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Paying {
    /// "2026-11-17".
    day: String,
    /// "transfer", "card", "cash", "cheque", "direct-debit", "other".
    means: String,
    /// In the invoice's currency: 400.5.
    amount: f64,
}

/// "Paid…" on an invoice (on a credit note, "Paid back…"): a payment of its
/// amount, on its day, by its means, written in this device's life
/// (`life::add_payment`, never more than is left); partial payments add up.
/// Paid in full, the invoice's planned budget line becomes real on that
/// day. Said in the status line with Undo, which takes the payment back
/// (and the line, while it still holds what Paid wrote). Returns what went
/// wrong, else "".
pub(crate) fn pay(qt: &QtThread, shared: &Arc<Shared>, number: &str, paying: &str) -> String {
    let paying: Paying = serde_json::from_str(paying).unwrap_or_default();
    let Some(day) = day_of(&paying.day) else { return tr().text("time-bad-day", None) };
    let cents = edit::cents(paying.amount);
    if cents <= 0 {
        return tr().text("invoices-pay-no-amount", None);
    }
    let folder = invoice::folder();
    let means = if paying.means.trim().is_empty() { "transfer".to_string() } else { paying.means.trim().to_string() };
    let written = life::add_payment(&folder, &projects::device(), number, invoice::Payment { date: day.to_string(), amount: cents, means, ..invoice::Payment::default() }, tr());
    let payment = match written {
        Ok(p) => p,
        Err(e) => return e,
    };
    // Paid in full: its line in the budget made real on that day.
    let all = invoice::all_in(&folder);
    let settled = all.iter().find(|i| i.number == number).is_some_and(|i| i.kind != Kind::CreditNote && i.status(&all).settled());
    let link = format!("sioul:invoice/{number}");
    let ledger = projects::store().ok().map(|s| s.root.join(budget::LEDGER));
    let line = match (&ledger, settled) {
        (Some(ledger), true) => budget::settle_line(ledger, &link, day).ok().flatten(),
        _ => None,
    };
    work::refresh(qt, shared);
    crate::backend::show(qt, shared);
    let found = all.iter().find(|i| i.number == number);
    let words = say(if found.is_some_and(|i| i.kind == Kind::CreditNote) { "invoices-paid-back-said" } else { "invoices-paid-said" }, &[("number", number.to_string()), ("amount", money_in(cents, found.map_or("EUR", |i| i.currency.as_str())))]);
    crate::mail::offer_back(qt, shared, words, move |qt, shared| {
        if let Err(said) = put_line_back(ledger.as_deref(), &link, line, day) {
            return said;
        }
        let problem = life::take_back(&invoice::folder(), &projects::device(), std::slice::from_ref(&payment.id)).err().unwrap_or_default();
        work::refresh(qt, shared);
        crate::backend::show(qt, shared);
        problem
    });
    String::new()
}

/// Paid…'s Undo on the budget line Paid… made real (the review of phase 3,
/// F1): planned again as it was, while it holds what Paid… wrote, real on
/// the payment's `day` (whatever day that is); else what to say, and then
/// nothing is undone.
fn put_line_back(ledger: Option<&std::path::Path>, link: &str, line: Option<budget::Settled>, day: Date) -> Result<(), String> {
    let (Some(ledger), Some(line)) = (ledger, line) else { return Ok(()) };
    if !line.planned {
        return Ok(());
    }
    match budget::unsettle_paid(ledger, link, line, day) {
        Ok(true) => Ok(()),
        Ok(false) => Err(tr().text("undo-too-late", None)),
        Err(e) => Err(e),
    }
}

/// "Credit note…": a draft credit note citing invoice `number`, all its
/// lines (to cancel it) or those at `lines` (JSON list of their places, to
/// correct some), opened in the editor. Returns {"draft"} or {"error"}.
pub(crate) fn credit(shared: &Arc<Shared>, number: &str, lines: &str) -> String {
    let chosen: Option<Vec<usize>> = serde_json::from_str::<Vec<usize>>(lines).ok().filter(|l| !l.is_empty());
    let result = (|| -> Result<String, String> {
        let folder = invoice::folder();
        let found = invoice::find_in(&folder, number).ok_or_else(|| tr().text("invoice-gone", None))?;
        if found.kind == Kind::CreditNote {
            return Err(tr().text("invoices-credit-of-credit", None));
        }
        // A deposit a live final invoice deducts is credited through that invoice (the second review of phase 3, R7): refused here, with the way out, as the draft's check would say at issue.
        if found.kind == Kind::Deposit
            && let Some(by) = life::deducted_by(&found, &invoice::all_in(&folder)).first()
        {
            return Err(say("invoice-check-credit-deposit-deducted", &[("deposit", found.number.clone()), ("number", by.number.clone())]));
        }
        let config = load_config();
        let draft = life::credit(&found, chosen.as_deref(), &projects::entries(shared), &config.invoice, today());
        Ok(invoice::save_draft(&folder, &draft)?.draft)
    })();
    match result {
        Ok(id) => json!({ "draft": id }).to_string(),
        Err(e) => refused(e),
    }
}

/// "Final invoice…" on a deposit invoice (the review of phase 3, F10; the
/// second, R9): the draft that holds the work, never a second one beside
/// it: the draft the deposit was made from, while it is there; else the
/// project's draft that holds its time (the time noted since added, and
/// said, as "Make the invoice" says it); else a draft made of the
/// project's time not billed yet; else one for its client. The deposit it
/// was pressed on is ticked, the draft's other ticks left as they are; a
/// draft this press just made also ticks the project's other open deposits
/// and those made from the same draft; the client's others are named by
/// the draft's recommended check. Returns {"draft", "said"} or {"error"}.
pub(crate) fn final_of(shared: &Arc<Shared>, number: &str) -> String {
    let result = (|| -> Result<(String, String), String> {
        let folder = invoice::folder();
        let records = invoice::all_in(&folder);
        let deposit = records.iter().find(|r| r.number == number && r.kind == Kind::Deposit).ok_or_else(|| tr().text("invoice-gone", None))?;
        let config = load_config();
        let s = &config.invoice;
        let today = today();
        let usable = |d: &Invoice| d.kind == Kind::Invoice && !d.newer && d.copy_of.is_empty() && life::same_client(d, deposit) && d.currency.eq_ignore_ascii_case(&deposit.currency);
        let drafts = invoice::drafts_in(&folder);
        let mut id = drafts.iter().find(|d| !deposit.deposit_of.is_empty() && d.draft == deposit.deposit_of && usable(d)).map(|d| d.draft.clone());
        let (mut made, mut said) = (false, String::new());
        // Never a draft of another client or currency touched, nor one made for nothing.
        if id.is_none()
            && !deposit.project.is_empty()
            && let Ok(timed) = projects::time_draft_for(shared, &deposit.project, &usable)
        {
            made = timed.made;
            if let Some((lines, minutes)) = timed.added {
                said = say("invoices-time-added", &[("n", lines.to_string()), ("time", duration(minutes))]);
            }
            id = Some(timed.id);
        }
        let id = match id {
            Some(id) => id,
            None => {
                let clients = clients_now();
                let world = world(shared);
                let mut draft = life::for_client(s, &deposit.client_ref, &clients, &world.contacts, tr().language(), today);
                draft.project = deposit.project.clone();
                draft.project_title = deposit.project_title.clone();
                draft.currency = deposit.currency.clone();
                draft.language = deposit.language.clone();
                made = true;
                invoice::save_draft(&folder, &draft)?.draft
            }
        };
        life::change_draft(&folder, &id, s, today, |d| {
            let open: Vec<&Invoice> = life::deposits_open(d, &records);
            let ticked: Vec<&Invoice> = open.iter().copied().filter(|o| final_ticks(o, deposit, d, made)).collect();
            life::cite_deposits(d, &ticked, &records, s, today);
        })?;
        changed_here(&id);
        Ok((id, said))
    })();
    match result {
        Ok((id, said)) => json!({ "draft": id, "said": said }).to_string(),
        Err(e) => refused(e),
    }
}

/// Whether "Final invoice…", pressed on `pressed`, ticks the open deposit
/// `open` in `draft` (R9): the one pressed, always; those the draft ticks
/// already, kept; and, only in a draft this press just made, the project's
/// others and those made from the same draft. Never a deposit of another
/// project by itself, nor one the person unticked in a draft that was there.
fn final_ticks(open: &Invoice, pressed: &Invoice, draft: &Invoice, made: bool) -> bool {
    if open.number == pressed.number || draft.deposits.iter().any(|c| c.number == open.number) {
        return true;
    }
    let same_project = !pressed.project.is_empty() && open.project == pressed.project;
    let same_draft = !pressed.deposit_of.is_empty() && open.deposit_of == pressed.deposit_of;
    made && (same_project || same_draft)
}

/// "Write the mail": a draft to the client's invoice address (its
/// profile's, else its card's), in the invoice's language, from the
/// account whose address is the one your invoices print (else the first
/// that sends), with the file issued attached as it was kept
/// (`invoice::kept::issued`: made again when gone, refused when it cannot
/// be), and the invoice tied to it: once that mail leaves Sioul, the
/// invoice is marked sent (`sent_with`). An invoice from before November
/// 2026 kept no file: its copy where invoices go, when it is there.
/// Returns {"draft"} or {"error"}.
pub(crate) fn write_mail(shared: &Shared, number: &str) -> String {
    let folder = invoice::folder();
    let Some(found) = invoice::find_in(&folder, number) else { return refused(tr().text("invoice-gone", None)) };
    let attached = if found.file.is_empty() {
        let copy = projects::pdf_path(&found);
        if !copy.is_file() {
            return refused(say("invoices-mail-print-first", &[("path", copy.display().to_string())]));
        }
        copy
    } else {
        if let Err(why) = invoice::kept::issued(&folder, &found, tr()) {
            return refused(match why {
                invoice::kept::NotGiven::Kept(why) => why,
                invoice::kept::NotGiven::Copy(path, _) => say("invoice-not-written", &[("path", path.display().to_string())]),
            });
        }
        invoice::kept::path(&folder, &found)
    };
    let config = load_config();
    let world = loaded(shared);
    let (client, _) = clients::resolve(&found.client_ref, &clients_now(), &world.contacts);
    let to = [found.client.email.trim(), client.email.trim()].into_iter().find(|e| !e.is_empty()).unwrap_or("").to_string();
    let issuer_mail = found.issuer.email.trim().to_lowercase();
    let account = config.accounts.iter().filter(|a| a.syncs()).find(|a| !issuer_mail.is_empty() && a.address.as_deref().is_some_and(|m| m.trim().eq_ignore_ascii_case(&issuer_mail))).or_else(|| config.accounts.iter().find(|a| a.syncs())).map(|a| a.id.clone()).unwrap_or_default();
    let language = Translator::new(if found.language.is_empty() { tr().language() } else { &found.language });
    let words = |id: &str, pairs: &[(&str, String)]| {
        let mut args = sioul_core::i18n::args();
        for (k, v) in pairs {
            args.set(k.to_string(), v.clone());
        }
        language.text(id, Some(&args))
    };
    let title = words(&format!("invoice-title-{}", found.kind.id()), &[("number", found.number.clone())]);
    let mut draft = sioul_core::compose::Draft::new(&account);
    draft.to = if to.is_empty() { Vec::new() } else { vec![if found.client.first_line().is_empty() { to.clone() } else { format!("{} <{}>", found.client.first_line().replace([',', ';', '<', '>', '"'], " ").trim(), to) }] };
    draft.subject = title.clone();
    let due = day_of(&found.due).map(|d| sioul_core::books::long_day(&language, d)).unwrap_or_default();
    let body = if found.kind == Kind::CreditNote { "invoices-mail-body-credit" } else if due.is_empty() { "invoices-mail-body-no-due" } else { "invoices-mail-body" };
    draft.body = words(body, &[("title", title), ("amount", language.money_in(found.total(), &found.currency)), ("due", due), ("name", found.issuer.name.clone())]);
    // The account's signature, as every draft written for you has it (F17).
    draft.add_signature(config.account(&account).and_then(|a| a.signature.as_deref()));
    draft.attachments = vec![attached];
    draft.links = vec![sioul_core::links::invoice_uri(&found.number)];
    // The invoice's own mail: sent with its file to someone else than you, the invoice is sent (R11).
    draft.invoice = found.number.clone();
    match draft.save() {
        Ok(_) => json!({ "draft": draft.id, "to": to }).to_string(),
        Err(e) => refused(e),
    }
}

/// A mail that left Sioul (`missed`: the recipients its server refused):
/// each invoice it is tied to marked sent on this device, today
/// (`life::mark_sent`), only when it carried that invoice to its client:
/// the invoice's kept PDF among its attachments, by its SHA-256 (an
/// invoice from before November 2026, its printed copy), and the client's
/// invoice address among the recipients it reached. A mail merely tied to
/// an invoice (a reminder, an agent's draft) marks nothing (the review of
/// phase 3, F5). Called by the send path, before the draft goes.
pub(crate) fn sent_with(shared: &Shared, draft: &sioul_core::compose::Draft, missed: &[String]) {
    let numbers: Vec<String> = draft.links.iter().filter_map(|l| l.strip_prefix("sioul:invoice/")).map(sioul_core::notes::decode).collect();
    if numbers.is_empty() {
        return;
    }
    let folder = invoice::folder();
    let attached: Vec<String> = draft.attachments.iter().filter_map(|p| invoice::kept::sha256_of(p)).collect();
    let clients = clients_now();
    // The client's addresses as "Write the mail" resolves them: the record's, its profile's, its contact's card's; your own, which reach no client.
    let world = world(shared);
    let contacts = &world.contacts;
    let own: Vec<String> = load_config().accounts.iter().filter_map(|a| a.address.as_deref()).map(address_of).collect();
    for number in numbers {
        let Some(found) = invoice::find_in(&folder, &number) else { continue };
        if !delivered(&found, draft, &attached, missed, &clients, contacts, &own) {
            continue;
        }
        if let Err(e) = life::mark_sent(&folder, &projects::device(), &number, today()) {
            eprintln!("Sioul: invoice {number} sent, not marked so: {e}");
        }
    }
}

/// Whether a mail that left carried the invoice to its client (the review
/// of phase 3, F5; the second, R11): its file among `attached` (the
/// attachments' hashes), and, among the recipients its server took (not
/// `missed`), either a known address of the client (the record's, its
/// profile's, its contact's card's), or, for the mail "Write the mail"
/// drafted for this invoice (`Draft::invoice`), anyone who is not you
/// (`own`): an address typed in that mail counts, your own copy alone does not.
fn delivered(found: &Invoice, draft: &sioul_core::compose::Draft, attached: &[String], missed: &[String], clients: &Clients, contacts: &[sioul_core::contacts::Contact], own: &[String]) -> bool {
    let missed: Vec<String> = missed.iter().map(|m| address_of(m)).collect();
    let reached: Vec<String> = draft.to.iter().chain(&draft.cc).chain(&draft.bcc).map(|r| address_of(r)).filter(|a| !a.is_empty() && !missed.contains(a)).collect();
    let known = invoice_addresses(found, clients, contacts).iter().any(|a| reached.contains(a));
    let its_mail = draft.invoice == found.number && reached.iter().any(|a| !own.contains(a));
    carried(found, draft, attached) && (known || its_mail)
}

/// A recipient's address, in small letters: "Anna <anna@example.org>" is "anna@example.org".
fn address_of(recipient: &str) -> String {
    let r = recipient.trim();
    r.rsplit_once('<').and_then(|(_, a)| a.split_once('>')).map_or(r, |(a, _)| a).trim().to_lowercase()
}

/// Whether a mail's attachments carry the invoice: its kept PDF by its
/// hash; one from before November 2026 kept none, its printed copy by its path.
fn carried(found: &Invoice, draft: &sioul_core::compose::Draft, attached: &[String]) -> bool {
    if found.file.is_empty() || found.file_sha256.is_empty() {
        let copy = projects::pdf_path(found);
        return draft.attachments.iter().any(|p| *p == copy);
    }
    attached.contains(&found.file_sha256)
}

/// Where the invoice goes by mail, as "Write the mail" resolves it: the
/// record's client address, its profile's, its contact's card's.
fn invoice_addresses(found: &Invoice, clients: &Clients, contacts: &[sioul_core::contacts::Contact]) -> Vec<String> {
    let (client, profile) = clients::resolve(&found.client_ref, clients, contacts);
    let profile = profile.map(|p| p.email.clone()).unwrap_or_default();
    [found.client.email.as_str(), client.email.as_str(), profile.as_str()].iter().map(|e| address_of(e)).filter(|e| !e.is_empty()).collect()
}

// ---------------------------------------------------------------- clients

/// Who an invoice may bill, matching `query`: client profiles first, then
/// contacts with a name or an organisation; at most twelve. Each
/// {ref, name, detail}.
pub(crate) fn client_choices(shared: &Shared, query: &str) -> String {
    let query = query.trim().to_lowercase();
    let clients = clients_now();
    let world = loaded(shared);
    let hit = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
    let mut out: Vec<Value> = Vec::new();
    for p in clients.list.iter().filter(|p| hit(&p.name)) {
        let reference = if p.contact.is_empty() { p.id.clone() } else { p.contact.clone() };
        out.push(json!({ "ref": reference, "name": p.name.lines().next().unwrap_or(""), "detail": tr().text(&format!("invoices-client-kind-{}", p.kind), None) }));
    }
    for c in world.contacts.iter().filter(|c| (!c.org.is_empty() || !c.name.is_empty()) && (hit(&c.org) || hit(&c.name))) {
        if out.len() >= 12 {
            break;
        }
        let reference = sioul_core::links::contact_uri(&c.uid);
        if clients.of_contact(&c.uid).is_some() || out.iter().any(|o| o["ref"] == reference.as_str()) {
            continue;
        }
        let name = if c.org.is_empty() { c.name.clone() } else { c.org.clone() };
        out.push(json!({ "ref": reference, "name": name, "detail": if c.org.is_empty() { String::new() } else { c.name.clone() } }));
    }
    out.truncate(12);
    json!(out).to_string()
}

/// A client's profile, as Contacts and Projects show it: the profile of the
/// contact (`sioul:contact/<uid>`), or of the name, a reference names; a
/// profile to make, filled from the card, when there is none
/// (`"exists": false`).
pub(crate) fn client_profile(shared: &Shared, reference: &str) -> String {
    let clients = clients_now();
    let world = loaded(shared);
    let (client, profile) = clients::resolve(reference, &clients, &world.contacts);
    let made = profile.cloned().unwrap_or_else(|| Profile {
        // Its contact by Sioul's own address, however the reference names the card.
        contact: sioul_core::contacts::card_address(&world.contacts, reference).unwrap_or_default(),
        kind: client.kind.clone(),
        name: client.name.clone(),
        address: client.address.clone(),
        email: client.email.clone(),
        ..Profile::default()
    });
    let language = tr().language().to_string();
    let country = made.address.country.clone();
    json!({
        "exists": profile.is_some(),
        "id": made.id,
        "contact": made.contact,
        "kind": made.kind,
        "kind_words": tr().text(&format!("invoices-client-kind-{}", made.kind), None),
        "name": made.name,
        "lines": made.address.lines.join("\n"),
        "postcode": made.address.postcode,
        "city": made.address.city,
        "country": country,
        "country_name": clients::country(&country).map(|c| c.name(&language).to_string()).unwrap_or_default(),
        "delivery_lines": made.delivery.as_ref().map(|d| d.lines.join("\n")).unwrap_or_default(),
        "delivery_postcode": made.delivery.as_ref().map(|d| d.postcode.clone()).unwrap_or_default(),
        "delivery_city": made.delivery.as_ref().map(|d| d.city.clone()).unwrap_or_default(),
        "delivery_country": made.delivery.as_ref().map(|d| d.country.clone()).unwrap_or_default(),
        "ids": made.ids.iter().map(|i| json!({ "kind": i.kind, "value": i.value, "printed": i.printed })).collect::<Vec<_>>(),
        "language": made.language,
        "currency": made.currency,
        "terms_days": made.terms_days,
        "end_of_month": made.end_of_month.unwrap_or(false),
        "email": made.email,
        "order": made.order,
        "summary": profile_summary(&made, &language),
        "id_kinds": id_kinds(&made),
    })
    .to_string()
}

/// The kinds of identifier a profile's form offers, and any its profile
/// holds besides, by its own name: never shown as another (F17).
fn id_kinds(p: &Profile) -> Vec<Value> {
    let mut kinds = vec![json!({ "kind": "siren", "label": "SIREN" }), json!({ "kind": "siret", "label": "SIRET" }), json!({ "kind": "vat", "label": tr().text("invoices-id-vat", None) }), json!({ "kind": "other", "label": tr().text("invoices-id-other", None) })];
    for i in &p.ids {
        if !kinds.iter().any(|k| k["kind"] == i.kind.as_str()) {
            kinds.push(json!({ "kind": i.kind, "label": i.kind }));
        }
    }
    kinds
}

/// A profile in one line: "A business · Belgium · BE0123456789 · invoices in French".
fn profile_summary(p: &Profile, language: &str) -> String {
    let mut parts = vec![tr().text(&format!("invoices-client-kind-{}", p.kind), None)];
    if let Some(c) = clients::country(&p.address.country) {
        parts.push(c.name(language).to_string());
    }
    parts.extend(p.ids.iter().filter(|i| !i.value.trim().is_empty()).map(|i| i.value.trim().to_string()));
    if !p.language.is_empty() {
        parts.push(tr().text(&format!("invoices-in-language-{}", p.language), None));
    }
    if !p.currency.is_empty() {
        parts.push(p.currency.clone());
    }
    if !p.email.is_empty() {
        parts.push(p.email.clone());
    }
    parts.join("  ·  ")
}

/// A client profile's form: its id ("" for a new one) and only what it
/// changed (`clients::ProfileChange`).
#[derive(Deserialize, Default)]
#[serde(default)]
struct ProfileForm {
    id: String,
    #[serde(flatten)]
    change: clients::ProfileChange,
}

/// What a profile's form changed, made right to be written: a country
/// written as a name kept as its two letters (one Sioul does not know
/// refused), a kind it does not offer left as it was, an identifier
/// without a kind "other".
fn checked(mut change: clients::ProfileChange) -> Result<clients::ProfileChange, String> {
    for country in [&mut change.country, &mut change.delivery_country].into_iter().flatten() {
        let written = country.trim().to_string();
        if written.is_empty() {
            continue;
        }
        match clients::country(&written) {
            Some(found) => *country = found.code.to_string(),
            None => return Err(say("invoice-check-client-country-unknown", &[("country", written)])),
        }
    }
    if change.kind.as_deref().is_some_and(|k| !["business", "consumer", "public"].contains(&k)) {
        change.kind = None;
    }
    for id in change.ids.iter_mut().flatten() {
        if id.kind.trim().is_empty() {
            id.kind = "other".into();
        }
    }
    Ok(change)
}

/// A client profile saved from its form (`sioul-clients.toml`, the
/// projects' part of the sharing): only what the form changed, key by key,
/// the rest of the profile, its comments, the keys Sioul does not know, an
/// identifier left unprinted and the delivery address kept as written
/// (`clients::change`, the review of phase 3, F6); made when new. Returns
/// {"id"} or {"error"}.
pub(crate) fn save_client_profile(form: &str) -> String {
    let form: ProfileForm = match serde_json::from_str(form) {
        Ok(f) => f,
        Err(e) => return refused(e.to_string()),
    };
    let Some(root) = load_config().records_root() else { return refused(tr().text("error-no-store", None)) };
    let change = match checked(form.change) {
        Ok(c) => c,
        Err(e) => return refused(e),
    };
    let named = |o: &Option<String>| o.as_deref().is_some_and(|t| !t.trim().is_empty());
    if form.id.trim().is_empty() && !named(&change.name) && !named(&change.contact) {
        return refused(tr().text("invoices-client-no-name", None));
    }
    match clients::change(&clients::file_in(&root), form.id.trim(), &change) {
        Ok(id) => json!({ "id": id }).to_string(),
        Err(e) if e == clients::GONE => refused(tr().text("invoices-client-changed-elsewhere", None)),
        Err(e) => refused(e),
    }
}

/// The kind and the country of a draft's client changed from the editor:
/// written in its profile (made for its contact, or its name, when it has
/// none), since a draft's client is made again from its profile until it
/// is issued. Returns the draft as the editor shows it, or {"error"}.
pub(crate) fn set_client_traits(shared: &Arc<Shared>, id: &str, kind: &str, country: &str) -> String {
    let folder = invoice::folder();
    let Some(d) = invoice::draft_in(&folder, id) else { return refused(tr().text("invoice-draft-gone", None)) };
    let Some(root) = load_config().records_root() else { return refused(tr().text("error-no-store", None)) };
    let clients = clients_now();
    let world = world(shared);
    let (client, profile) = clients::resolve(&d.client_ref, &clients, &world.contacts);
    // Its kind and country only, the rest of its profile as written (F6);
    // without a profile, one made of what the draft knows of its client.
    let mut change = clients::ProfileChange { kind: Some(kind.to_string()), country: Some(country.trim().to_string()), ..clients::ProfileChange::default() };
    if profile.is_none() {
        change.contact = sioul_core::contacts::card_address(&world.contacts, &d.client_ref);
        change.name = Some(client.name.clone());
        change.lines = Some(client.address.lines.clone());
        change.postcode = Some(client.address.postcode.clone());
        change.city = Some(client.address.city.clone());
        change.email = Some(client.email.clone());
    }
    let change = match checked(change) {
        Ok(c) => c,
        Err(e) => return refused(e),
    };
    if let Err(e) = clients::change(&clients::file_in(&root), profile.map_or("", |p| p.id.as_str()), &change) {
        return refused(if e == clients::GONE { tr().text("invoices-client-changed-elsewhere", None) } else { e });
    }
    draft(shared, id)
}

// ---------------------------------------------------------------- Settings ▸ Invoices

/// The keys of `[invoice]` written as texts.
const TEXTS: [&str; 20] = ["preset", "legal_form", "regime", "name", "trade_name", "address", "postcode", "city", "country", "email", "payment", "language", "currency", "reset", "insurance", "capital", "declare", "vat_period", "trade", "platform"];

/// Settings ▸ Invoices (design §3, §9): the preset first, then what it asks
/// (your legal form, asked once of a table from before; the identifiers it
/// requires), the rest folded under "More": income regime, the dated VAT
/// scheme entries, identifiers with "printed", trade name, address,
/// country and email, bank accounts and the QR code, terms, the penalty
/// rate, language and currency, the numbering, the legal lines on and off,
/// insurance, capital for companies; then the books' settings
/// (activities, period, versement libératoire, start, VAT summary's period,
/// the option for debits, your own contribution rates); the e-invoicing
/// platform.
pub(crate) fn settings_view() -> String {
    let config = load_config();
    let s = &config.invoice;
    let tr = tr();
    let language = tr.language().to_string();
    let today = today();
    let preset = life::preset_of(s);
    let form = life::legal_form_of(s);
    let presets_list: Vec<Value> = presets::all().iter().map(|p| json!({ "id": p.id, "name": p.name.get(&language), "supported": p.supported, "why_not": p.why_not.get(&language) })).collect();
    let forms: Vec<Value> = preset.legal_form.iter().map(|f| json!({ "id": f.id, "name": f.name.get(&language), "company": f.company })).collect();
    let regimes: Vec<Value> = preset.regime.iter().map(|r| json!({ "id": r.id, "name": r.name.get(&language) })).collect();
    let schemes: Vec<Value> = preset.tax.schemes.iter().map(|id| json!({ "id": id, "name": tr.text(&format!("invoices-scheme-{id}"), None) })).collect();
    let id_kinds: Vec<Value> = preset.identifier.iter().map(|k| json!({ "kind": k.kind, "name": k.name.get(&language) })).collect();
    // What the preset asks of you before issuing any invoice, as you are today:
    // its rules for the issuer under your VAT scheme, legal form and activities,
    // whatever the client and the lines.
    let scheme_now = life::scheme_on(s, today).0;
    let company = preset.form(&form).is_some_and(|f| f.company);
    let applies = |r: &&presets::Require| {
        r.who == presets::Who::Issuer
            && r.level == presets::Level::Required
            && r.client.is_empty()
            && r.zone.is_empty()
            && r.nature.is_empty()
            && r.category.is_empty()
            && (r.scheme.is_empty() || r.scheme.contains(&scheme_now))
            && (r.form.is_empty() || r.form.iter().any(|f| *f == form || (f == "company" && company)))
            && (r.activity.is_empty() || s.activity.iter().any(|a| r.activity.contains(a)))
            && r.from.is_none_or(|f| f <= today)
            && r.until.is_none_or(|u| u >= today)
    };
    let mut asks: Vec<String> = preset.require.iter().filter(applies).map(|r| r.why.get(&language).to_string()).collect();
    asks.dedup();
    let pattern = life::pattern_of(s, Kind::Invoice);
    let folder = invoice::folder();
    let next = pattern.as_ref().ok().map(|p| invoice::series::next(p, today, &invoice::series::Journal::load(&folder), &invoice::records_in(&folder), &crate::share::claimed_series(projects::INVOICES)).0).unwrap_or_default();
    let penalty = preset.penalty_on(today).map(|bp| {
        let note = preset.penalty.as_ref().map(|p| p.default_note.get(&language).to_string()).unwrap_or_default();
        if note.is_empty() { tr.percent_bp(bp) } else { format!("{} ({note})", tr.percent_bp(bp)) }
    });
    let lines: Vec<Value> = preset
        .line
        .iter()
        .map(|l| {
            let own = s.lines.iter().find(|o| o.id == l.id);
            json!({ "id": l.id, "text": l.text.get(&language), "own": own.map(|o| o.text.clone()).unwrap_or_default(), "subject": own.map(|o| o.subject.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| l.subject.clone()), "off": own.is_some_and(|o| o.off), "preset": true })
        })
        .chain(s.lines.iter().filter(|o| o.id.trim().is_empty() || o.id.starts_with("older-")).map(|o| json!({ "id": o.id, "text": "", "own": o.text, "subject": o.subject, "off": o.off, "preset": false })))
        .collect();
    let declares = preset.declaration.is_some();
    let periods: Vec<Value> = preset.declaration.as_ref().map(|d| d.periods.clone()).unwrap_or_default().iter().map(|p| json!({ "id": p, "name": tr.text(&format!("invoices-every-{p}"), None) })).collect();
    let activities: Vec<Value> = preset.activity.iter().map(|a| json!({ "id": a.id, "name": a.name.get(&language) })).collect();
    let unread = sioul_core::books::unread_sentences(tr, &sioul_core::books::Settings::of(s, None));
    let mut view = json!({
        "preset": preset.id,
        "chosen": !s.preset.trim().is_empty(),
        "presets": presets_list,
        "limits": preset.limits.get(&language),
        "why_not": if preset.supported { String::new() } else { preset.why_not.get(&language).to_string() },
        "asks": asks,
        "form": form,
        "form_asked": form.is_empty() && !preset.legal_form.is_empty(),
        "forms": forms,
        "company": company,
        "regime": life::regime_of(s),
        "regimes": regimes,
        "schemes": schemes,
        // Each day as a day (2027-01-01), however it was typed; one that does not read as written, said by the books and refused at issue.
        "tax": s.tax.iter().map(|t| json!({ "from": sioul_core::config::read_day(&t.from).map_or_else(|| t.from.clone(), |d| d.to_string()), "scheme": t.scheme, "mention": t.mention })).collect::<Vec<_>>(),
        "scheme_now": life::scheme_on(s, today).0,
        "mention_default": preset.mention_on(today),
    });
    let more = json!({
        "ids": s.ids.iter().map(|i| json!({ "kind": i.kind, "value": i.value, "printed": i.printed })).collect::<Vec<_>>(),
        "id_kinds": id_kinds,
        "older_siret": if s.ids.is_empty() { s.siret.clone() } else { String::new() },
        "name": s.name,
        "trade_name": s.trade_name,
        "address": s.address,
        "postcode": s.postcode,
        "city": s.city,
        "country": s.country,
        "country_default": preset.country,
        "email": s.email,
        "banks": s.banks.iter().map(|b| json!({ "holder": b.holder, "iban": b.iban, "bic": b.bic, "valid": b.iban.trim().is_empty() || invoice::epc::iban_valid(&b.iban) })).collect::<Vec<_>>(),
        "qr": s.qr != Some(false),
        "payment": s.payment,
        "terms_days": s.terms_days,
        "terms_default": preset.terms_days.unwrap_or(30),
        "end_of_month": s.end_of_month,
        "penalty_rate": s.penalty_rate,
        "penalty_default": penalty.unwrap_or_default(),
        "language": s.language,
        "currency": s.currency,
        "currency_default": if preset.currency.is_empty() { "EUR".to_string() } else { preset.currency.clone() },
    });
    let rest = json!({
        "pattern": s.pattern,
        "pattern_default": invoice::series::PATTERN,
        "pattern_problem": pattern.err().unwrap_or_default(),
        // The reset the numbering follows (F20): the one written, else the usual pattern's own.
        "reset": life::pattern_of(s, Kind::Invoice).map_or_else(|_| invoice::series::Reset::named(s.reset.trim()).id(), |p| p.reset().id()),
        "reset_written": s.reset.trim(),
        "credit_pattern": s.credit_pattern,
        "next": next,
        "lines": lines,
        "insurance": s.insurance,
        "capital": s.capital,
        "declares": declares,
        "activities": s.activity,
        "activity_choices": activities,
        "declare": s.declare,
        "periods": periods,
        "vl": s.vl,
        "trade": s.trade,
        "started": s.started,
        "vat_period": s.vat_period,
        "debits": s.options.iter().any(|o| o == "debits"),
        "contributions": s.contributions.iter().map(|c| json!({ "kind": c.kind, "activity": c.activity, "rate": c.rate, "from": c.from, "until": c.until })).collect::<Vec<_>>(),
        "platform": s.platform,
        "unread": unread,
    });
    for part in [more, rest] {
        if let (Some(into), Value::Object(from)) = (view.as_object_mut(), part) {
            into.extend(from);
        }
    }
    view
    .to_string()
}

/// A JSON object of a list's row made a TOML table: texts, switches and
/// numbers kept; empty texts and nulls left out, so that what is not said
/// takes its default.
fn table_of(row: &Value) -> toml::Table {
    let mut table = toml::Table::new();
    for (key, value) in row.as_object().into_iter().flatten() {
        let item = match value {
            Value::String(t) if t.trim().is_empty() => continue,
            Value::String(t) => toml::Value::String(t.trim().to_string()),
            Value::Bool(b) => toml::Value::Boolean(*b),
            Value::Number(n) if key == "rate" || key == "amount" => toml::Value::Float(n.as_f64().unwrap_or(0.0)),
            Value::Number(n) => match n.as_i64() {
                Some(i) => toml::Value::Integer(i),
                None => toml::Value::Float(n.as_f64().unwrap_or(0.0)),
            },
            _ => continue,
        };
        table.insert(key.clone(), item);
    }
    table
}

/// One of Settings ▸ Invoices' values written (`key` without `invoice.`,
/// its value as JSON): a list whole (`config::set_invoice_list`), a text, a
/// number, a switch; an empty text removes the key, its default coming
/// back. A numbering pattern that does not read, or a day that is none, is
/// refused in words. Returns what went wrong, else "".
pub(crate) fn set_setting(qt: &QtThread, shared: &Arc<Shared>, key: &str, value: &str) -> String {
    let value: Value = match serde_json::from_str(value) {
        Ok(v) => v,
        Err(e) => return format!("{key}: {e}"),
    };
    let path = config_path();
    let text = value.as_str().map(str::trim).unwrap_or("").to_string();
    let config_key = format!("invoice.{key}");
    let result = match key {
        // A VAT scheme's day as people type it (01/01/2027) written as a day (2027-01-01); one that does not read refused (F19).
        "tax" => match tax_rows(&value) {
            Ok(rows) => sioul_core::config::set_invoice_list(&path, key, &rows),
            Err(e) => Err(e),
        },
        "id" | "bank" | "line" | "contribution" => {
            let rows: Vec<toml::Table> = value.as_array().into_iter().flatten().map(table_of).filter(|t| !t.is_empty()).collect();
            sioul_core::config::set_invoice_list(&path, key, &rows)
        }
        // A reset the numbering can follow: from 1 each year asks the year in the numbers (F20).
        "reset" => {
            let mut s = load_config().invoice;
            s.reset = text.clone();
            match life::pattern_of(&s, Kind::Invoice) {
                Err(e) if e.contains("start again each year") => Err(tr().text("invoice-check-reset-year", None)),
                _ => sioul_core::config::set_value(&path, &config_key, &SettingValue::Text(text.clone())),
            }
        }
        "pattern" | "credit_pattern" if !text.is_empty() => {
            let reset = invoice::series::Reset::named(load_config().invoice.reset.trim());
            invoice::series::Pattern::parse(&text, reset).map_err(|e| say("invoices-pattern-wrong", &[("pattern", text.clone()), ("why", e)])).and_then(|_| sioul_core::config::set_value(&path, &config_key, &SettingValue::Text(text.clone())))
        }
        "pattern" | "credit_pattern" => sioul_core::config::set_value(&path, &config_key, &SettingValue::Text(String::new())),
        "started" if !text.is_empty() && day_of(&text).is_none() => Err(tr().text("time-bad-day", None)),
        "started" => sioul_core::config::set_value(&path, &config_key, &SettingValue::Text(text.clone())),
        "terms_days" => match value.as_u64() {
            Some(days) => sioul_core::config::set_value(&path, &config_key, &SettingValue::Int(days.min(3650) as i64)),
            None => sioul_core::config::set_value(&path, &config_key, &SettingValue::Text(String::new())),
        },
        "penalty_rate" => match value.as_f64().filter(|p| *p > 0.0) {
            Some(rate) => sioul_core::config::set_value(&path, &config_key, &SettingValue::Float(rate)),
            None => sioul_core::config::set_value(&path, &config_key, &SettingValue::Text(String::new())),
        },
        "vl" | "end_of_month" | "qr" => sioul_core::config::set_value(&path, &config_key, &SettingValue::Bool(value.as_bool().unwrap_or(false))),
        "activity" => {
            let list: Vec<String> = value.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
            sioul_core::config::set_value(&path, &config_key, &SettingValue::Texts(list))
        }
        "debits" => {
            let mut options: Vec<String> = load_config().invoice.options.into_iter().filter(|o| o != "debits").collect();
            if value.as_bool() == Some(true) {
                options.push("debits".into());
            }
            sioul_core::config::set_value(&path, "invoice.options", &SettingValue::Texts(options))
        }
        k if TEXTS.contains(&k) => sioul_core::config::set_value(&path, &config_key, &SettingValue::Text(text.clone())),
        other => Err(format!("invoice.{other}: no such setting")),
    };
    crate::backend::show(qt, shared);
    work::refresh(qt, shared);
    result.err().unwrap_or_default()
}

/// The dated VAT schemes as Settings ▸ Invoices gives them, each day read
/// as people type it and written as 2027-01-01; a day that does not read
/// refused, never written to be ignored (the review of phase 3, F19).
fn tax_rows(value: &Value) -> Result<Vec<toml::Table>, String> {
    let mut rows = Vec::new();
    for row in value.as_array().into_iter().flatten() {
        let mut table = table_of(row);
        if let Some(from) = table.get("from").and_then(toml::Value::as_str).map(str::to_string) {
            match sioul_core::config::read_day(&from) {
                Some(day) => {
                    table.insert("from".into(), toml::Value::String(day.to_string()));
                }
                None => return Err(say("invoices-scheme-day-wrong", &[("day", from)])),
            }
        }
        if !table.is_empty() {
            rows.push(table);
        }
    }
    Ok(rows)
}

// ---------------------------------------------------------------- amounts

/// An amount in cents as the window's language writes it for typing: "751.66", "751,66".
fn amount_text(cents: i64) -> String {
    let text = format!("{}.{:02}", cents / 100, (cents % 100).abs());
    if tr().language() == "fr" { text.replace('.', ",") } else { text }
}

/// An amount typed in the window (the review of phase 3, F8), read by its
/// language: in English a comma groups thousands and a dot marks decimals
/// ("1,500.50"); in French a space, a no-break or a narrow space groups and
/// a comma marks decimals ("1 500,50"), a dot alone read as decimals.
/// None when it does not read as an amount ("1,5,0", "12abc").
pub(crate) fn read_amount_in(text: &str, language: &str) -> Option<f64> {
    let text: String = text.trim().chars().filter(|c| !matches!(c, '€' | '$' | '£')).collect();
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (group, decimal): (&[char], char) = if language == "fr" { (&[' ', '\u{a0}', '\u{202f}'], ',') } else { (&[','], '.') };
    let mut cleaned = String::new();
    for c in text.chars() {
        match c {
            c if group.contains(&c) => {}
            c if c == decimal => cleaned.push('.'),
            '.' if language == "fr" => cleaned.push('.'),
            '-' if cleaned.is_empty() => cleaned.push('-'),
            c if c.is_ascii_digit() => cleaned.push(c),
            _ => return None,
        }
    }
    if cleaned.matches('.').count() > 1 {
        return None;
    }
    // A group of thousands is three digits: "1,50" is no amount in English.
    if language != "fr" && text.contains(',') && text.split('.').next().unwrap_or("").split(',').skip(1).any(|g| g.len() != 3) {
        return None;
    }
    if language == "fr" {
        // "1.500", "12.500": a dot before exactly three digits, no comma
        // after, is thousands to many hands and decimals to Sioul: refused,
        // never read as the smaller number (the second review of phase 3,
        // R14); "12.50" and "1.5" stay decimals.
        if !text.contains(',') && text.rsplit_once('.').is_some_and(|(_, after)| after.len() == 3 && after.bytes().all(|b| b.is_ascii_digit())) {
            return None;
        }
        // A space groups three digits: "12 5" and "1 50" are no amounts.
        let whole = text.split([',', '.']).next().unwrap_or("");
        if whole.split([' ', '\u{a0}', '\u{202f}']).skip(1).any(|g| g.len() != 3) {
            return None;
        }
    }
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A number typed that is no money (a quantity, a rate), read as an amount
/// is (`read_amount_in`) and said back as the window's language writes it:
/// {"ok", "value", "words"}, "= 2.5"; an empty text asks for one (the
/// second review of phase 3, R1).
pub(crate) fn read_number(text: &str) -> String {
    let language = tr().language();
    match read_amount_in(text, language) {
        Some(value) => {
            let plain = format!("{value}");
            json!({ "ok": true, "value": value, "words": format!("= {}", if language == "fr" { plain.replace('.', ",") } else { plain }) }).to_string()
        }
        None if text.trim().is_empty() => json!({ "ok": false, "value": 0, "words": tr().text("invoices-number-missing", None) }).to_string(),
        None => json!({ "ok": false, "value": 0, "words": tr().text("invoices-amount-unread", None) }).to_string(),
    }
}

/// An amount typed, read: {"ok", "value", "words"}: "words" says what it
/// reads as ("= €1,500.00"), beside the field, as it is typed.
pub(crate) fn read_amount(text: &str, currency: &str) -> String {
    let currency = if currency.trim().is_empty() { "EUR" } else { currency.trim() };
    match read_amount_in(text, tr().language()) {
        Some(value) => json!({ "ok": true, "value": value, "words": format!("= {}", money_in(edit::cents(value), currency)) }).to_string(),
        None if text.trim().is_empty() => json!({ "ok": false, "value": 0, "words": "" }).to_string(),
        None => json!({ "ok": false, "value": 0, "words": tr().text("invoices-amount-unread", None) }).to_string(),
    }
}

// ---------------------------------------------------------------- the books

/// A declaration said made ("Declared") or not (`on`), by its key
/// ("declaration:2026-Q4", "des:2026-10"), in this device's life: the
/// Porch's item goes, and a refund of its money given back later corrects
/// it rather than lowering a later period (`books::declarations::place_refund`).
/// Said with Undo. Returns what went wrong, else "".
pub(crate) fn declared(qt: &QtThread, shared: &Arc<Shared>, key: &str, on: bool) -> String {
    let folder = invoice::folder();
    let device = projects::device();
    // What it was: its Undo says it again, as a newer word (the review of phase 3, F16).
    let before = sioul_core::books::declared_days(&invoice::lives::Lives::load(&folder)).get(key).copied();
    let now_ms = || jiff::Timestamp::now().as_millisecond();
    let result = sioul_core::books::mark_declared(&folder, &device, key, today(), on, now_ms());
    crate::backend::show(qt, shared);
    if let Err(e) = result {
        return e;
    }
    let key = key.to_string();
    let words = tr().text(if on { "invoices-declared-said" } else { "invoices-not-declared-said" }, None);
    crate::mail::offer_back(qt, shared, words, move |qt, shared| {
        let back = sioul_core::books::mark_declared(&invoice::folder(), &projects::device(), &key, before.unwrap_or_else(today), before.is_some(), now_ms()).err().unwrap_or_default();
        crate::backend::show(qt, shared);
        back
    });
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list's words: never "overdue"; a draft's day from its id; the
    /// statuses the filter offers are those there are.
    #[test]
    fn the_list_says_where_each_stands() {
        let all = vec![
            Invoice { number: "2026-001".into(), date: "2026-10-01".into(), due: "2026-10-31".into(), currency: "EUR".into(), totals: invoice::Totals { due: 10_000, ..Default::default() }, ..Invoice::default() },
            Invoice { number: "2026-002".into(), date: "2026-10-05".into(), currency: "EUR".into(), totals: invoice::Totals { due: 10_000, ..Default::default() }, payments: vec![invoice::Payment { id: "hand:2026-002/x".into(), date: "2026-10-09".into(), amount: 4_000, means: "transfer".into(), ..Default::default() }], ..Invoice::default() },
        ];
        assert_eq!(status_of(&all[0], &all), "unpaid", "past its day: not paid yet, never late");
        assert_eq!(status_of(&all[1], &all), "partly-paid");
        let r = row(&all[1], &all, false);
        assert_eq!((r["year"].as_str(), r["status"].as_str(), r["key"].as_str()), (Some("2026"), Some("partly-paid"), Some("2026-002")));
        assert!(!r["left"].as_str().unwrap_or("").is_empty(), "what is left said");
        let draft = Invoice { draft: "20261117-093012-abcdef".into(), ..Invoice::default() };
        assert_eq!(draft_day(&draft), "2026-11-17");
        assert_eq!(row(&draft, &all, false)["key"].as_str(), Some("draft:20261117-093012-abcdef"));
    }

    /// The numbers with no record here are said as runs, never a line each.
    #[test]
    fn missing_numbers_are_said_as_runs() {
        use invoice::series::Missing;
        let m = |n: u64| Missing { sequence: "2026-{N:3}".into(), counter: n, number: Some(format!("2026-{n:03}")) };
        let said = missing_words(vec![m(1), m(2), m(3), m(7)]);
        assert_eq!(said.len(), 2, "{said:?}");
        assert!(said[0].contains("2026-001") && said[0].contains("2026-003") && !said[0].contains("2026-002"), "{said:?}");
        assert!(said[1].contains("2026-007"), "{said:?}");
    }

    /// An amount is read by the window's language (the review of phase 3,
    /// F8): a comma groups thousands in English, a space in French, where
    /// a comma marks decimals; what does not read is never taken for
    /// another amount.
    #[test]
    fn an_amount_is_read_by_the_window_s_language() {
        let en = |t: &str| read_amount_in(t, "en");
        let fr = |t: &str| read_amount_in(t, "fr");
        assert_eq!((en("1,500"), en("1,500.50"), en("€1,500"), en("1500"), en("12.5")), (Some(1500.0), Some(1500.5), Some(1500.0), Some(1500.0), Some(12.5)));
        assert_eq!((en("1,50"), en("1.500.00"), en("12abc"), en("")), (None, None, None, None));
        assert_eq!((fr("1 500,50"), fr("1\u{202f}500"), fr("1\u{a0}500,5"), fr("751,66"), fr("12.5")), (Some(1500.5), Some(1500.0), Some(1500.5), Some(751.66), Some(12.5)));
        assert_eq!((fr("1,5,0"), fr("douze")), (None, None));
        // In French a dot before exactly three digits is refused, and a space groups three digits only (the second review, R14).
        assert_eq!((fr("1.500"), fr("12.500"), fr("12 5"), fr("1 50"), fr("1 500 00")), (None, None, None, None, None));
        assert_eq!((fr("12.50"), fr("1.5"), fr("1.500,50"), fr("1 500"), fr("12 500,5")), (Some(12.5), Some(1.5), None, Some(1500.0), Some(12500.5)));
    }

    /// A VAT scheme's day as people type it is written as a day; one that
    /// does not read is refused, never written to be ignored (F19).
    #[test]
    fn a_scheme_s_day_is_written_as_a_day() {
        let rows = tax_rows(&json!([{ "scheme": "franchise" }, { "from": "01/01/2027", "scheme": "liable" }])).unwrap();
        assert_eq!(rows[1].get("from").and_then(toml::Value::as_str), Some("2027-01-01"));
        assert!(!rows[0].contains_key("from"));
        assert!(tax_rows(&json!([{ "from": "1er janvier 2027", "scheme": "liable" }])).is_err());
    }

    /// An invoice is sent only by a mail that carried its kept PDF to its
    /// client (the review of phase 3, F5): a reminder tied to it marks
    /// nothing, nor a send whose server refused the client's address.
    #[test]
    fn an_invoice_is_sent_by_the_mail_that_carried_it() {
        let found = Invoice { number: "2026-005".into(), file: "2026-005.pdf".into(), file_sha256: "ab12".into(), client: invoice::Client { email: "anna@example.org".into(), ..invoice::Client::default() }, ..Invoice::default() };
        let mut mail = sioul_core::compose::Draft::new("work");
        mail.to = vec!["Anna Exemple <Anna@Example.org>".into()];
        mail.cc = vec!["me@example.org".into()];
        let none = Clients::default();
        let own = vec!["me@example.org".to_string()];
        let sent = |found: &Invoice, mail: &sioul_core::compose::Draft, attached: &[&str], missed: &[&str], contacts: &[sioul_core::contacts::Contact]| {
            delivered(found, mail, &attached.iter().map(|a| a.to_string()).collect::<Vec<_>>(), &missed.iter().map(|m| m.to_string()).collect::<Vec<_>>(), &none, contacts, &own)
        };
        assert!(!sent(&found, &mail, &[], &[], &[]), "no file: a reminder");
        assert!(sent(&found, &mail, &["ab12"], &[], &[]));
        assert!(!sent(&found, &mail, &["ab12"], &["anna@example.org"], &[]), "the client's address refused, the copy taken");
        assert!(!sent(&found, &mail, &["ff00"], &[], &[]), "another file");
        // A client with no address known (the second review of phase 3, R11): the mail "Write the mail" drafted for it, sent to an address typed, marks it; a mail merely tied to it does not; nor your own copy alone.
        let unknown = Invoice { client: invoice::Client::default(), ..found.clone() };
        mail.to = vec!["billing@example.org".into()];
        assert!(!sent(&unknown, &mail, &["ab12"], &[], &[]), "tied, not its own mail");
        mail.invoice = "2026-005".into();
        assert!(sent(&unknown, &mail, &["ab12"], &[], &[]), "its own mail, to an address typed");
        assert!(!sent(&unknown, &mail, &["ab12"], &["billing@example.org"], &[]), "only your own copy left");
        // The contact's card got its address after the invoice was issued: known, as Write the mail resolves it.
        mail.invoice.clear();
        let card = sioul_core::contacts::Contact { uid: "lumen-1".into(), name: "Studio Lumen".into(), emails: vec![sioul_core::contacts::Labeled { label: String::new(), value: "billing@example.org".into() }], ..Default::default() };
        let of_card = Invoice { client_ref: "sioul:contact/lumen-1".into(), ..unknown.clone() };
        assert!(sent(&of_card, &mail, &["ab12"], &[], std::slice::from_ref(&card)));
    }

    /// A draft saved as it is typed reads no store again and keeps what
    /// the pages read: the day is never planned again for it (F7).
    #[test]
    fn a_draft_saved_reads_no_store() {
        sioul_core::config::stand_ins_only();
        let shared = Arc::new(Shared::default());
        let _ = loaded(&shared);
        let folder = invoice::folder();
        let made = invoice::save_draft(&folder, &Invoice { kind: Kind::Invoice, currency: "EUR".into(), ..Invoice::default() }).unwrap();
        crate::work::LOADS.with(|n| n.set(0));
        let answer: Value = serde_json::from_str(&change(&shared, &made.draft, &json!({ "notes": ["Thank you."] }).to_string())).unwrap();
        assert!(answer["error"].is_null(), "{answer}");
        assert_eq!(answer["notes"][0].as_str(), Some("Thank you."));
        assert_eq!(crate::work::LOADS.with(std::cell::Cell::get), 0, "nothing read again");
        assert!(shared.loaded.lock().unwrap().is_some(), "what the pages read is kept");
        // New's draft left as it was made is taken out; written in, it stays; a draft made before is never touched.
        let new = || serde_json::from_str::<Value>(&new_draft(&shared, "{}")).unwrap()["draft"].as_str().unwrap_or("").to_string();
        let blank = new();
        assert!(invoice::draft_in(&folder, &blank).is_some());
        leave_draft(&blank);
        assert!(invoice::draft_in(&folder, &blank).is_none(), "made and never written in: taken out");
        let typed = new();
        change(&shared, &typed, &json!({ "order": "PO-1" }).to_string());
        leave_draft(&typed);
        assert!(invoice::draft_in(&folder, &typed).is_some(), "written in: kept");
        let older = invoice::save_draft(&folder, &Invoice { kind: Kind::Invoice, currency: "EUR".into(), ..Invoice::default() }).unwrap();
        leave_draft(&older.draft);
        assert!(invoice::draft_in(&folder, &older.draft).is_some(), "an empty draft made before stays");
        for id in [&made.draft, &typed, &older.draft] {
            let _ = invoice::remove_draft(&folder, id);
        }
    }

    /// Paid…'s Undo puts the budget line back as it was when Paid… was said
    /// for another day than today; once the line changed since, it is too
    /// late, said, and nothing is undone (the review of phase 3, F1).
    #[test]
    fn paid_s_undo_puts_the_line_back() {
        let dir = std::env::temp_dir().join(format!("sioul-paid-undo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ledger = dir.join("ledger.toml");
        let link = "sioul:invoice/2026-005";
        std::fs::write(&ledger, "[[line]]\nbudget = \"work\"\ndate = 2026-12-01\namount = 990\nlabel = \"Invoice 2026-005\"\nplanned = true\nlinks = [\"sioul:invoice/2026-005\"]\n").unwrap();
        let paid_on: Date = "2026-11-10".parse().unwrap();
        let line = budget::settle_line(&ledger, link, paid_on).unwrap();
        assert!(line.is_some_and(|l| l.planned));
        assert_eq!(put_line_back(Some(&ledger), link, line, paid_on), Ok(()));
        let text = std::fs::read_to_string(&ledger).unwrap();
        assert!(text.contains("planned = true") && text.contains("2026-12-01"), "{text}");
        // Changed since (its day moved by hand): too late, said.
        let line = budget::settle_line(&ledger, link, paid_on).unwrap();
        std::fs::write(&ledger, std::fs::read_to_string(&ledger).unwrap().replace("2026-11-10", "2026-11-12")).unwrap();
        assert!(put_line_back(Some(&ledger), link, line, paid_on).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A draft changed here that an exchange replaced is named, so that it
    /// is said; one that went out as it was, or that this window wrote again
    /// since, is not (the second review of phase 3, R2).
    #[test]
    fn a_draft_replaced_by_the_exchange_is_named() {
        sioul_core::config::stand_ins_only();
        let folder = invoice::folder();
        let draft = |name: &str| invoice::save_draft(&folder, &Invoice { kind: Kind::Invoice, currency: "EUR".into(), client: invoice::Client { name: name.into(), ..invoice::Client::default() }, ..Invoice::default() }).unwrap().draft;
        let (lost, sent, again) = (draft("Studio Lumen"), draft("Atelier Nord"), draft("Fernhill"));
        let tracked: std::collections::BTreeMap<String, String> = [&lost, &sent, &again].iter().map(|id| (id.to_string(), invoice::draft_revision(&folder, id))).collect();
        // The exchange brings another version of the first; the third is written again here meanwhile.
        let config = load_config();
        life::change_draft(&folder, &lost, &config.invoice, today(), |d| d.order = "from the phone".into()).unwrap();
        life::change_draft(&folder, &again, &config.invoice, today(), |d| d.order = "typed since".into()).unwrap();
        let since: std::collections::BTreeMap<String, String> = [(again.clone(), invoice::draft_revision(&folder, &again))].into();
        assert_eq!(replaced_among(&folder, &tracked, &since), ["Studio Lumen"]);
        for id in [&lost, &sent, &again] {
            let _ = invoice::remove_draft(&folder, id);
        }
    }

    /// Final invoice… ticks the deposit it was pressed on and leaves the
    /// draft's other ticks as they are; only a draft it just made also takes
    /// the project's other deposits, never another project's (R9).
    #[test]
    fn final_invoice_ticks_what_it_was_pressed_on() {
        let deposit = |number: &str, project: &str, of: &str| Invoice { number: number.into(), kind: Kind::Deposit, project: project.into(), deposit_of: of.into(), ..Invoice::default() };
        let (pressed, same_project, other_project, same_draft) = (deposit("2026-010", "site", "d1"), deposit("2026-011", "site", ""), deposit("2026-012", "shop", ""), deposit("2026-013", "", "d1"));
        let there = Invoice { deposits: vec![invoice::Cited { number: "2026-012".into(), date: String::new(), amount: 1 }], ..Invoice::default() };
        let blank = Invoice::default();
        // A draft that was there: the one pressed, and what it ticks already; nothing else.
        assert_eq!([&pressed, &same_project, &other_project, &same_draft].map(|o| final_ticks(o, &pressed, &there, false)), [true, false, true, false]);
        // A draft this press just made: the project's others and those of the same draft too, never another project's.
        assert_eq!([&pressed, &same_project, &other_project, &same_draft].map(|o| final_ticks(o, &pressed, &blank, true)), [true, true, false, true]);
    }

    /// A list's row of Settings ▸ Invoices made a table: empty texts left
    /// out, a rate always a number with decimals (a contribution's rate
    /// reads as one).
    #[test]
    fn a_settings_row_becomes_a_table() {
        let t = table_of(&json!({ "kind": "social", "activity": "bnc", "rate": 12, "from": "2026-01-01", "until": "" }));
        assert_eq!(t.get("rate"), Some(&toml::Value::Float(12.0)));
        assert!(!t.contains_key("until"));
        let id = table_of(&json!({ "kind": "siret", "value": "732 829 320 00017", "printed": false }));
        assert_eq!(id.get("printed"), Some(&toml::Value::Boolean(false)));
    }
}

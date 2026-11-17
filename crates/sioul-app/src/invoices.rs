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

/// The client profiles of the notes folder, as they are now.
fn clients_now() -> Clients {
    load_config().notes_root_path().and_then(|root| Clients::load(&root).ok()).unwrap_or_default()
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
    let mut said: Vec<String> = invoice::series::missing(&journal, &all)
        .into_iter()
        .map(|m| match m.number {
            Some(number) => say("invoice-cli-missing", &[("number", number)]),
            None => say("invoice-cli-missing-in", &[("sequence", m.sequence), ("n", m.counter.to_string())]),
        })
        .collect();
    said.extend(invoice::twice_in(&folder).into_iter().map(|t| say("invoice-cli-twice", &[("number", t.number), ("first", t.clients.0), ("second", t.clients.1), ("file", t.copy.display().to_string())])));
    said.extend(invoice::lives::Lives::load(&folder).newer.iter().chain(invoice::series::Journal::newer(&folder).iter()).map(|p| say("invoice-cli-newer", &[("file", p.display().to_string())])));
    json!({ "rows": shown, "clients": clients, "years": years, "statuses": statuses, "said": said, "declares": !all.is_empty() }).to_string()
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
    let time = !l.sessions.is_empty() || (l.unit == "MIN" && l.base == 60);
    json!({
        "from": i,
        "description": l.description,
        "details": l.details,
        "quantity": l.quantity as f64 / 1000.0,
        "unit": l.unit,
        "price": l.price as f64 / 100.0,
        "discount": l.discount as f64 / 100.0,
        "nature": if l.nature == invoice::Nature::Goods { "goods" } else { "service" },
        "activity": l.activity,
        "own_tax": l.own_tax,
        "category": l.category,
        "rate": l.rate as f64 / 100.0,
        "exemption": l.exemption,
        "time": time,
        "time_words": l.minutes().filter(|_| time).map(sioul_core::timereport::duration).unwrap_or_default(),
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
    let world = loaded(shared);
    life::refresh_client(&mut d, &clients, &world.contacts);
    life::refresh(&mut d, s, today);
    let checks = life::checks(&d, s, today, &records);
    let profile = clients::resolve(&d.client_ref, &clients, &world.contacts).1.cloned();
    let country = life::client_country(&d.client);
    let country_name = clients::country(&country).map_or_else(|| country.clone(), |c| c.name(tr().language()).to_string());
    let contact = d.client_ref.strip_prefix("sioul:contact/").map(sioul_core::notes::decode).or_else(|| profile.as_ref().and_then(|p| p.contact.strip_prefix("sioul:contact/").map(sioul_core::notes::decode))).unwrap_or_default();
    let cited: Vec<&str> = d.deposits.iter().map(|c| c.number.as_str()).collect();
    let mut deposits: Vec<Value> = edit::deposits_for(&d, &records).into_iter().map(|r| json!({ "number": r.number, "date": short(&r.date), "amount": money_in(r.totals.with_tax, &r.currency), "cited": cited.contains(&r.number.as_str()) })).collect();
    // Those it cites already, whatever happened to them since.
    let gone: Vec<Value> = d.deposits.iter().filter(|c| !deposits.iter().any(|v| v["number"] == c.number.as_str())).map(|c| json!({ "number": c.number, "date": short(&c.date), "amount": money_in(c.amount, &d.currency), "cited": true })).collect();
    deposits.extend(gone);
    let required: Vec<String> = checks.iter().filter(|c| c.required).map(|c| c.text.clone()).collect();
    let said: Vec<String> = checks.iter().filter(|c| !c.required).map(|c| c.text.clone()).collect();
    let language = if d.language.is_empty() { tr().language().to_string() } else { d.language.clone() };
    json!({
        "id": d.draft,
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
    match invoice::draft_in(&invoice::folder(), id) {
        Some(d) => draft_view(d, shared).to_string(),
        None => refused(tr().text("invoice-draft-gone", None)),
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
pub(crate) fn new_draft(qt: &QtThread, shared: &Arc<Shared>, making: &str) -> String {
    let making: Making = serde_json::from_str(making).unwrap_or_default();
    let result = (|| -> Result<String, String> {
        let config = load_config();
        let s = &config.invoice;
        let today = today();
        let folder = invoice::folder();
        let clients = clients_now();
        let world = loaded(shared);
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
        }
        Ok(invoice::save_draft(&folder, &draft)?.draft)
    })();
    work::refresh(qt, shared);
    match result {
        Ok(id) => json!({ "draft": id }).to_string(),
        Err(e) => refused(e),
    }
}

/// A draft changed as the editor says (`edit::Change`, JSON), saved, then
/// shown as the editor shows it; {"error"} when it is gone.
pub(crate) fn change(qt: &QtThread, shared: &Arc<Shared>, id: &str, change: &str) -> String {
    let change: edit::Change = match serde_json::from_str(change) {
        Ok(c) => c,
        Err(e) => return refused(e.to_string()),
    };
    let config = load_config();
    let s = &config.invoice;
    let folder = invoice::folder();
    let records = invoice::all_in(&folder);
    let clients = clients_now();
    let world = loaded(shared);
    let cx = edit::Context { settings: s, records: &records, clients: &clients, contacts: &world.contacts, fallback: tr().language(), today: today() };
    let made = life::change_draft(&folder, id, s, today(), |d| edit::apply(d, &change, &cx));
    work::refresh(qt, shared);
    match made {
        Ok(d) => draft_view(d, shared).to_string(),
        Err(_) => refused(tr().text("invoice-draft-gone", None)),
    }
}

/// A draft taken out (Discard), with Undo in the status line, which keeps it again as it was.
pub(crate) fn discard(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let folder = invoice::folder();
    let Some(before) = invoice::draft_in(&folder, id) else { return tr().text("invoice-draft-gone", None) };
    if let Err(e) = invoice::remove_draft(&folder, id) {
        return e;
    }
    work::refresh(qt, shared);
    crate::mail::offer_back(qt, shared, say("invoices-discarded", &[("title", title_of(&before))]), move |qt, shared| {
        let back = invoice::save_draft(&invoice::folder(), &before).err().unwrap_or_default();
        work::refresh(qt, shared);
        back
    });
    String::new()
}

/// Issues the draft `id` (the editor's Issue): checked, numbered, its file
/// and its record kept, its time marked, its budget line planned, all or
/// nothing (`invoice::life::issue`); its file copied where invoices go
/// (`projects::printed`). Returns {"number", "pdf", "said"}: the status
/// line's words (a file set aside, the checks' notes, a copy not written);
/// or {"error"}, with "elsewhere" when another computer numbers invoices.
pub(crate) fn issue(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
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
        let world = loaded(shared);
        let store = projects::store().ok();
        let ledger = store.as_ref().map(|st| st.root.join(budget::LEDGER));
        // Expected in its project's budget, as the project's invoices always were.
        let budget_id = store.as_ref().and_then(|st| st.get(&draft.project)).and_then(|p| p.budget.clone()).unwrap_or_default();
        let numbering = life::Numbering { device: projects::device(), sharing: crate::share::configured(), drafts_shared: crate::share::shares("drafts"), claimed: crate::share::claimed_series(projects::INVOICES) };
        let steps = life::Steps { time: None, ledger: ledger.as_deref().map(|l| (l, budget_id.as_str())), clients: Some((&clients, &world.contacts)) };
        let made = life::issue(&folder, id, s, today, &numbering, &steps, tr())?;
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
    let payments: Vec<Value> = i
        .payments
        .iter()
        .map(|p| json!({ "id": p.id, "words": say(if i.kind == Kind::CreditNote { "invoices-paid-back-line" } else { "invoices-paid-line" }, &[("amount", money_in(p.amount, &i.currency)), ("day", long(&p.date)), ("means", means_words(&p.means))]), "bank": !p.line.is_empty() }))
        .collect();
    let left = i.left(&all);
    let lines: Vec<Value> = i.lines.iter().map(|l| json!({ "description": l.description, "amount": money_in(l.amount, &i.currency), "time": l.minutes().filter(|_| !l.sessions.is_empty() || i.older.is_some()).map(sioul_core::timereport::duration).unwrap_or_default() })).collect();
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
        "left_words": if left > 0 { say(if i.kind == Kind::CreditNote { "invoices-left-to-pay-back" } else { "invoices-left-to-pay" }, &[("amount", money_in(left, &i.currency))]) } else { String::new() },
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
            "final": i.kind == Kind::Deposit && deducted_by.is_empty(),
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
        let mut problem = String::new();
        if let (Some(ledger), Some(line)) = (&ledger, line.clone())
            && line.planned
            && let Ok(false) = budget::unsettle_paid(ledger, &link, line, today())
        {
            problem = tr().text("undo-too-late", None);
        }
        if let Err(e) = life::take_back(&invoice::folder(), &projects::device(), std::slice::from_ref(&payment.id)) {
            problem = e;
        }
        work::refresh(qt, shared);
        crate::backend::show(qt, shared);
        problem
    });
    String::new()
}

/// "Credit note…": a draft credit note citing invoice `number`, all its
/// lines (to cancel it) or those at `lines` (JSON list of their places, to
/// correct some), opened in the editor. Returns {"draft"} or {"error"}.
pub(crate) fn credit(qt: &QtThread, shared: &Arc<Shared>, number: &str, lines: &str) -> String {
    let chosen: Option<Vec<usize>> = serde_json::from_str::<Vec<usize>>(lines).ok().filter(|l| !l.is_empty());
    let result = (|| -> Result<String, String> {
        let folder = invoice::folder();
        let found = invoice::find_in(&folder, number).ok_or_else(|| tr().text("invoice-gone", None))?;
        if found.kind == Kind::CreditNote {
            return Err(tr().text("invoices-credit-of-credit", None));
        }
        let config = load_config();
        let draft = life::credit(&found, chosen.as_deref(), &projects::entries(shared), &config.invoice, today());
        Ok(invoice::save_draft(&folder, &draft)?.draft)
    })();
    work::refresh(qt, shared);
    match result {
        Ok(id) => json!({ "draft": id }).to_string(),
        Err(e) => refused(e),
    }
}

/// A deposit invoice's final invoice: a draft for its client and project,
/// deducting it (and any other deposit of that client not deducted yet,
/// left for you to tick). Returns {"draft"} or {"error"}.
pub(crate) fn final_of(qt: &QtThread, shared: &Arc<Shared>, number: &str) -> String {
    let result = (|| -> Result<String, String> {
        let folder = invoice::folder();
        let records = invoice::all_in(&folder);
        let deposit = records.iter().find(|r| r.number == number && r.kind == Kind::Deposit).ok_or_else(|| tr().text("invoice-gone", None))?;
        let config = load_config();
        let s = &config.invoice;
        let clients = clients_now();
        let world = loaded(shared);
        let mut draft = life::for_client(s, &deposit.client_ref, &clients, &world.contacts, tr().language(), today());
        draft.project = deposit.project.clone();
        draft.project_title = deposit.project_title.clone();
        draft.currency = deposit.currency.clone();
        draft.language = deposit.language.clone();
        life::cite_deposits(&mut draft, std::slice::from_ref(deposit), s, today());
        Ok(invoice::save_draft(&folder, &draft)?.draft)
    })();
    work::refresh(qt, shared);
    match result {
        Ok(id) => json!({ "draft": id }).to_string(),
        Err(e) => refused(e),
    }
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
    draft.attachments = vec![attached];
    draft.links = vec![sioul_core::links::invoice_uri(&found.number)];
    match draft.save() {
        Ok(_) => json!({ "draft": draft.id, "to": to }).to_string(),
        Err(e) => refused(e),
    }
}

/// A mail that left Sioul: each invoice it was written for ("Write the
/// mail", the draft's links) marked sent on this device, today
/// (`life::mark_sent`). Called by the send path, before the draft goes.
pub(crate) fn sent_with(draft: &sioul_core::compose::Draft) {
    let numbers: Vec<String> = draft.links.iter().filter_map(|l| l.strip_prefix("sioul:invoice/")).map(sioul_core::notes::decode).collect();
    if numbers.is_empty() {
        return;
    }
    let folder = invoice::folder();
    for number in numbers {
        if let Err(e) = life::mark_sent(&folder, &projects::device(), &number, today()) {
            eprintln!("Sioul: invoice {number} sent, not marked so: {e}");
        }
    }
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
        contact: if reference.starts_with("sioul:contact/") { reference.to_string() } else { String::new() },
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
        "delivery": made.delivery.as_ref().map(|d| d.text("", &language)).unwrap_or_default(),
        "ids": made.ids.iter().map(|i| json!({ "kind": i.kind, "value": i.value })).collect::<Vec<_>>(),
        "language": made.language,
        "currency": made.currency,
        "terms_days": made.terms_days,
        "end_of_month": made.end_of_month.unwrap_or(false),
        "email": made.email,
        "order": made.order,
        "summary": profile_summary(&made, &language),
        "id_kinds": [{ "kind": "siren", "label": "SIREN" }, { "kind": "siret", "label": "SIRET" }, { "kind": "vat", "label": tr().text("invoices-id-vat", None) }, { "kind": "other", "label": tr().text("invoices-id-other", None) }],
    })
    .to_string()
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

/// A client profile as its form gives it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ProfileEdit {
    id: String,
    contact: String,
    kind: String,
    name: String,
    lines: String,
    postcode: String,
    city: String,
    country: String,
    delivery: String,
    ids: Vec<IdEdit>,
    language: String,
    currency: String,
    terms_days: Option<u32>,
    end_of_month: bool,
    email: String,
    order: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct IdEdit {
    kind: String,
    value: String,
}

/// A client profile saved from its form (`sioul-clients.toml`, the
/// projects' part of the sharing): made when new, every other profile kept.
/// A country written as a name is kept as its two letters. Returns {"id"}
/// or {"error"}.
pub(crate) fn save_client_profile(qt: &QtThread, shared: &Arc<Shared>, form: &str) -> String {
    let form: ProfileEdit = match serde_json::from_str(form) {
        Ok(f) => f,
        Err(e) => return refused(e.to_string()),
    };
    let Some(root) = load_config().notes_root_path() else { return refused(tr().text("error-no-store", None)) };
    let code = |written: &str| clients::country(written).map_or_else(|| written.trim().to_uppercase(), |c| c.code.to_string());
    let country = form.country.trim();
    if !country.is_empty() && clients::country(country).is_none() {
        return refused(say("invoice-check-client-country-unknown", &[("country", country.to_string())]));
    }
    let delivery = invoice::Address::from_text(&form.delivery);
    let profile = Profile {
        id: form.id.trim().to_string(),
        contact: form.contact.trim().to_string(),
        kind: if ["business", "consumer", "public"].contains(&form.kind.as_str()) { form.kind } else { "business".into() },
        name: form.name.trim().to_string(),
        address: invoice::Address { lines: form.lines.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect(), postcode: form.postcode.trim().to_string(), city: form.city.trim().to_string(), country: if country.is_empty() { String::new() } else { code(country) } },
        delivery: (!delivery.is_empty()).then_some(delivery),
        ids: form.ids.into_iter().filter(|i| !i.value.trim().is_empty()).map(|i| invoice::Identifier { kind: if i.kind.trim().is_empty() { "other".into() } else { i.kind.trim().to_string() }, value: i.value.trim().to_string(), printed: true }).collect(),
        language: form.language.trim().to_string(),
        currency: form.currency.trim().to_uppercase(),
        terms_days: form.terms_days,
        end_of_month: form.end_of_month.then_some(true),
        email: form.email.trim().to_string(),
        order: form.order.trim().to_string(),
    };
    if profile.name.is_empty() && profile.contact.is_empty() {
        return refused(tr().text("invoices-client-no-name", None));
    }
    let result = clients::save(&clients::file_in(&root), &profile);
    work::refresh(qt, shared);
    match result {
        Ok(id) => json!({ "id": id }).to_string(),
        Err(e) => refused(e),
    }
}

/// The kind and the country of a draft's client changed from the editor:
/// written in its profile (made for its contact, or its name, when it has
/// none), since a draft's client is made again from its profile until it
/// is issued. Returns the draft as the editor shows it, or {"error"}.
pub(crate) fn set_client_traits(qt: &QtThread, shared: &Arc<Shared>, id: &str, kind: &str, country: &str) -> String {
    let folder = invoice::folder();
    let Some(d) = invoice::draft_in(&folder, id) else { return refused(tr().text("invoice-draft-gone", None)) };
    let Some(root) = load_config().notes_root_path() else { return refused(tr().text("error-no-store", None)) };
    let country = country.trim();
    if !country.is_empty() && clients::country(country).is_none() {
        return refused(say("invoice-check-client-country-unknown", &[("country", country.to_string())]));
    }
    let clients = clients_now();
    let world = loaded(shared);
    let (client, profile) = clients::resolve(&d.client_ref, &clients, &world.contacts);
    let mut made = profile.cloned().unwrap_or_else(|| Profile {
        contact: if d.client_ref.starts_with("sioul:contact/") { d.client_ref.clone() } else { String::new() },
        name: client.name.clone(),
        address: client.address.clone(),
        email: client.email.clone(),
        ..Profile::default()
    });
    if ["business", "consumer", "public"].contains(&kind) {
        made.kind = kind.to_string();
    }
    made.address.country = clients::country(country).map(|c| c.code.to_string()).unwrap_or_default();
    if let Err(e) = clients::save(&clients::file_in(&root), &made) {
        return refused(e);
    }
    work::refresh(qt, shared);
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
    // What the preset asks of you before issuing: its required identifiers.
    let mut asks: Vec<String> = preset.require.iter().filter(|r| r.who == presets::Who::Issuer && r.level == presets::Level::Required && r.client.is_empty() && r.zone.is_empty() && r.category.is_empty() && r.until.is_none_or(|u| u >= today)).map(|r| r.why.get(&language).to_string()).collect();
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
    let company = preset.form(&form).is_some_and(|f| f.company);
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
        "tax": s.tax.iter().map(|t| json!({ "from": t.from, "scheme": t.scheme, "mention": t.mention })).collect::<Vec<_>>(),
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
        "reset": if s.reset.trim().is_empty() { "yearly".to_string() } else { s.reset.trim().to_string() },
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
        "id" | "tax" | "bank" | "line" | "contribution" => {
            let rows: Vec<toml::Table> = value.as_array().into_iter().flatten().map(table_of).filter(|t| !t.is_empty()).collect();
            sioul_core::config::set_invoice_list(&path, key, &rows)
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

// ---------------------------------------------------------------- the books

/// A declaration said made ("Declared") or not (`on`), by its key
/// ("declaration:2026-Q4", "des:2026-10"), in this device's life: the
/// Porch's item goes, and a refund of its money given back later corrects
/// it rather than lowering a later period (`books::declarations::place_refund`).
/// Said with Undo. Returns what went wrong, else "".
pub(crate) fn declared(qt: &QtThread, shared: &Arc<Shared>, key: &str, on: bool) -> String {
    let folder = invoice::folder();
    let device = projects::device();
    let result = if on { sioul_core::books::mark_declared(&folder, &device, key, today()).map(|id| vec![id]) } else { sioul_core::books::unmark_declared(&folder, &device, key).map(|_| Vec::new()) };
    crate::backend::show(qt, shared);
    match result {
        Ok(ids) if on => {
            crate::mail::offer_back(qt, shared, tr().text("invoices-declared-said", None), move |qt, shared| {
                let back = life::take_back(&invoice::folder(), &projects::device(), &ids).err().unwrap_or_default();
                crate::backend::show(qt, shared);
                back
            });
            String::new()
        }
        Ok(_) => String::new(),
        Err(e) => e,
    }
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

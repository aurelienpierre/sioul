// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Budgets page's Movements and From mail tabs, and the declaration (the
//! money ledger's design §4 to §6; docs/accounting.md, "Movements tab",
//! "From mail", "The declaration"):
//!
//! - **Movements**: the ledger found by its filters and its words, newest
//!   first, in pages of 60 (`moneylist`), the filters in use said as words;
//!   the hour's areas as the page's filter says them, a typed search reaching
//!   every area. A movement opened in full, with what it allows by what it is
//!   made from (`acts::allowed`); made, changed and removed so, each act said
//!   in one line with Undo, which puts the files back while they are still as
//!   the act left them.
//! - **From mail**: every money mail kept on this device, with what became
//!   of it (`moneyindex::history`), read off the window's thread; "Not a
//!   payment" learns the sender's shape, "Count it again" forgets it, "The
//!   same as…" joins a mail to a movement and "Not the same" parts them,
//!   each with Undo.
//! - **The declaration**: a declaring work budget's turnover by the day each
//!   payment was paid, and its receipts register, saved as CSV here and as
//!   a PDF by the window, which prints its HTML as invoices are printed.

use crate::backend::{QtThread, Shared, load_config, say, show, tr};
use serde::Deserialize;
use serde_json::{Value, json as j};
use sioul_core::agentwrites::{FileChange, Said};
use sioul_core::money::Money;
use sioul_core::movements::{self, Fee, FeeFrom, FeeKind, Store, Way, acts};
use sioul_core::{moneyindex, moneylist, moneyview, receipts};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The notes folder that holds the budget file, and the words.
fn folder() -> Result<(sioul_core::config::Config, PathBuf), String> {
    let config = load_config();
    let root = config.notes_root_path().ok_or_else(|| tr().text("papers-no-store", None))?;
    Ok((config, root))
}

fn problem(text: String) -> String {
    j!({ "problem": text }).to_string()
}

/// What the tabs ask, as their filters say it; all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Asked {
    text: String,
    from: String,
    to: String,
    account: String,
    budget: String,
    /// "in", "out", "".
    way: String,
    at_least: Option<f64>,
    at_most: Option<f64>,
    /// "bank", "mail", "plan", "hand".
    told_by: Vec<String>,
    page: usize,
    /// From mail: the address it came to, tied or not, its kind.
    address: String,
    tied: Option<bool>,
    kind: String,
}

fn day_of(text: &str) -> Option<jiff::civil::Date> {
    text.trim().parse().ok()
}

fn some(text: &str) -> Option<String> {
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

fn way_of(text: &str) -> Option<Way> {
    match text {
        "in" => Some(Way::In),
        "out" => Some(Way::Out),
        _ => None,
    }
}

/// The areas the page's filter shows now (`payments::budget_filter`), and the filter's button.
fn page_areas() -> (sioul_core::areas::Area, sioul_core::areafilter::Shown) {
    let now = sioul_core::zone::now();
    crate::payments::budget_filter(&crate::hours::mode_at(&now), now.timestamp().as_second())
}

/// Each filter in use, in words, for its × (design §4: "Work · money in · October").
fn chips(ledger: &sioul_core::budget::Ledger, asked: &Asked, today: jiff::civil::Date) -> Vec<Value> {
    let mut out = Vec::new();
    let mut chip = |id: &str, words: String| out.push(j!({ "id": id, "words": words }));
    if let Some(account) = some(&asked.account) {
        chip("account", ledger.bank_accounts.iter().find(|a| a.id == account).map_or(account.clone(), |a| a.title.clone()));
    }
    if let Some(budget) = some(&asked.budget) {
        chip("budget", ledger.budget_title(&budget).to_string());
    }
    if let Some(way) = way_of(&asked.way) {
        chip("way", tr().text(if way == Way::In { "movements-filter-in" } else { "movements-filter-out" }, None));
    }
    match (day_of(&asked.from), day_of(&asked.to)) {
        (Some(from), Some(to)) => chip("days", say("movements-filter-days", &[("from", tr().day_in(from, today)), ("to", tr().day_in(to, today))])),
        (Some(from), None) => chip("days", say("movements-filter-from", &[("from", tr().day_in(from, today))])),
        (None, Some(to)) => chip("days", say("movements-filter-to", &[("to", tr().day_in(to, today))])),
        (None, None) => {}
    }
    match (asked.at_least.filter(|a| *a > 0.0), asked.at_most.filter(|a| *a > 0.0)) {
        (Some(least), Some(most)) => chip("amount", say("movements-filter-between", &[("least", tr().money(Money::from(least))), ("most", tr().money(Money::from(most)))])),
        (Some(least), None) => chip("amount", say("movements-filter-least", &[("least", tr().money(Money::from(least)))])),
        (None, Some(most)) => chip("amount", say("movements-filter-most", &[("most", tr().money(Money::from(most)))])),
        (None, None) => {}
    }
    if !asked.told_by.is_empty() {
        let words: Vec<String> = asked.told_by.iter().map(|t| tr().text(&format!("movements-told-{t}"), None)).collect();
        chip("told", words.join(", "));
    }
    if let Some(address) = some(&asked.address) {
        chip("address", load_config().account(&address).and_then(|a| a.address.clone()).unwrap_or(address));
    }
    if let Some(tied) = asked.tied {
        chip("tied", tr().text(if tied { "money-mail-filter-tied" } else { "money-mail-filter-untied" }, None));
    }
    if let Some(kind) = some(&asked.kind) {
        chip("kind", tr().text(&format!("kind-{kind}"), None));
    }
    out
}

/// The choices the filters and the forms offer: accounts, budgets, the budgets that declare.
fn choices(ledger: &sioul_core::budget::Ledger) -> Value {
    j!({
        "accounts": ledger.bank_accounts.iter().map(|a| j!({ "id": a.id, "title": a.title })).collect::<Vec<_>>(),
        "budgets": ledger.budgets.iter().map(|b| j!({ "id": b.id, "title": b.title })).collect::<Vec<_>>(),
        "declares": ledger.budgets.iter().filter(|b| receipts::declares(b)).map(|b| j!({ "id": b.id, "title": b.title })).collect::<Vec<_>>(),
    })
}

/// One row of the ledger, flat, as the tab's list model holds it: its day
/// as the section, who, what moved with its sign, its budget, its state in
/// words; a second line only for fees or a payout; your note.
fn row(ledger: &sioul_core::budget::Ledger, m: &movements::Movement, today: jiff::civil::Date) -> Value {
    let r = moneyview::coming_row(ledger, m, &Default::default(), tr(), today);
    j!({
        "address": m.address(),
        "key": m.steady_key(),
        "section": tr().day_in(m.day, today),
        "who": r.who,
        "amount": moneylist::signed(tr(), m.sum.amount()),
        "cents": m.sum.amount().cents(),
        "way": r.way,
        "budget": if r.budget.is_empty() { r.way_words } else { r.budget },
        "standing": r.state,
        "second": moneylist::second_line(tr(), m).unwrap_or_default(),
        "note": m.note.trim(),
    })
}

/// The ledger's page `asked` (design §4), as JSON: {rows, page, more, total,
/// chips, filter, accounts, budgets, declares}. It opens on the latest
/// movements: up to today, unless the filters ask for a later day.
pub(crate) fn movements(asked: &str) -> String {
    let (config, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    let asked: Asked = serde_json::from_str(asked).unwrap_or_default();
    let today = sioul_core::zone::now().date();
    let books = movements::books(&root, &sioul_core::words::Words::of(&config), today);
    let (areas, filter) = page_areas();
    let query = moneylist::Query {
        from: day_of(&asked.from),
        // The latest movements first: up to today, unless a later day is asked;
        // what is still to come is Overview's What is coming.
        to: day_of(&asked.to).or(Some(today)),
        account: some(&asked.account),
        budget: some(&asked.budget),
        way: way_of(&asked.way),
        areas: Some(areas),
        at_least: asked.at_least.filter(|a| *a > 0.0).map(Money::from),
        at_most: asked.at_most.filter(|a| *a > 0.0).map(Money::from),
        text: asked.text.clone(),
        told_by: asked.told_by.iter().filter_map(|t| moneylist::ToldBy::parse(t)).collect(),
        page: asked.page,
    };
    let page = moneylist::query(&books, &query);
    let mut out = choices(&books.ledger);
    out["rows"] = page.movements.iter().map(|m| row(&books.ledger, m, today)).collect();
    out["page"] = j!(page.page);
    out["more"] = j!(page.more);
    // A sum only when you asked for days: today's bound alone asks no question.
    let days = day_of(&asked.from).is_some() || day_of(&asked.to).is_some();
    out["total"] = j!(page.total.filter(|_| days).map(|t| moneylist::total_words(tr(), &t)).unwrap_or_default());
    out["chips"] = j!(chips(&books.ledger, &asked, today));
    out["filter"] = serde_json::to_value(&filter).unwrap_or(Value::Null);
    out.to_string()
}

/// One movement in full (design §4, "Opening a row"), as JSON: its words,
/// its sources (each opens), the payments a payout carried and the payout
/// that carried it, what it allows (`acts::allowed`), and its data for
/// the form. One you removed says so (`gone`), with Put back.
pub(crate) fn movement(address: &str) -> String {
    let (config, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    let today = sioul_core::zone::now().date();
    let books = movements::books(&root, &sioul_core::words::Words::of(&config), today);
    let Some((m, gone)) = moneylist::get(&books, address) else { return problem(tr().text("movement-gone", None)) };
    let store = Store::load(&root).unwrap_or_default();
    let allowed = acts::allowed(&books.ledger, &store, m);
    let r = moneyview::coming_row(&books.ledger, m, &Default::default(), tr(), today);
    let says = match allowed.made {
        "bank" | "joined" => tr().text("movement-form-bank", None),
        "occurrence" => tr().text("movement-form-occurrence", None),
        "told" => tr().text("movement-form-told", None),
        "invoice" => tr().text("movement-refused-invoice", None),
        "set-aside" => tr().text("movement-refused-set-aside", None),
        _ => String::new(),
    };
    // The payments a payout carried, each opened from its own row: said there, not twice.
    let carried: Vec<(String, String)> = m
        .carried
        .iter()
        .map(|c| {
            let mut parts = vec![moneyview::short_day(tr(), c.day), c.label.clone(), moneylist::signed(tr(), c.gross)];
            if let Some(fee) = &c.fee {
                parts.push(moneylist::fee_words(tr(), fee));
            }
            (sioul_core::links::movement_uri(&c.key), parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · "))
        })
        .collect();
    let mut lines = moneylist::detail_words(tr(), &books.ledger, m, today);
    lines.retain(|l| !carried.iter().any(|(_, words)| l.trim_start() == words));
    j!({
        "address": m.address(),
        "key": m.steady_key(),
        "gone": gone,
        "who": r.who,
        "amount": moneylist::signed(tr(), m.sum.amount()),
        "cents": m.sum.amount().cents(),
        "budget_id": m.place.budgets().first().map(|b| b.to_string()).unwrap_or_default(),
        "lines": lines,
        "sources": r.sources,
        "probable": r.probable,
        "carried": carried.iter().map(|(address, words)| j!({ "address": address, "words": words })).collect::<Vec<_>>(),
        "carried_by": m.carried_by.as_deref().map(sioul_core::links::movement_uri).unwrap_or_default(),
        "allowed": allowed,
        "says": says,
        "data": moneylist::data(&books.ledger, m),
    })
    .to_string()
}

/// The files a money act may write: the budget file and the store.
fn money_files(root: &Path) -> Vec<PathBuf> {
    vec![root.join(sioul_core::budget::LEDGER), Store::path(root)]
}

/// An act done with Undo: `files` read before it, the act, then its line said
/// with Undo in the status line, which puts back each file that changed
/// while it is still as the act left it; the views made again either way.
/// Returns {said} or {problem}.
fn with_undo(qt: &QtThread, shared: &Arc<Shared>, files: Vec<PathBuf>, act: impl FnOnce() -> Result<String, String>) -> String {
    let before: Vec<(PathBuf, Option<String>)> = files.into_iter().map(|path| {
        let text = std::fs::read_to_string(&path).ok();
        (path, text)
    }).collect();
    let said = match act() {
        Ok(said) => said,
        Err(e) => return problem(e),
    };
    let changed: Vec<FileChange> = before.into_iter().map(|(path, text)| FileChange::of(&path, text)).filter(|f| f.before != f.after).collect();
    show(qt, shared);
    if !changed.is_empty() {
        let line = if said.is_empty() { tr().text("movement-done", None) } else { said.clone() };
        crate::mail::offer_back(qt, shared, line, move |qt, shared| {
            if !(Said { at: 0, line: String::new(), files: changed.clone() }).put_back() {
                return tr().text("undo-too-late", None);
            }
            sioul_core::changes::said(sioul_core::changes::Kind::Money);
            show(qt, shared);
            String::new()
        });
    }
    j!({ "said": said }).to_string()
}

/// A fee as the form gives it: {kind, amount} in units, without sign.
fn fees_of(value: &Value) -> Vec<Fee> {
    value
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(|f| {
                    let amount = Money::from(f["amount"].as_f64()?.abs());
                    (amount != Money::ZERO).then(|| Fee { kind: FeeKind::parse(f["kind"].as_str().unwrap_or("processor")), amount, from: FeeFrom::Hand, about: false })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn money_of(value: &Value) -> Option<Money> {
    value.as_f64().filter(|v| v.is_finite() && *v != 0.0).map(Money::from)
}

/// "New movement" (design §3.1): its form's JSON {label, gross, net, fees,
/// account, budget, day, paid_on, planned, note}, amounts signed (+ in, −
/// out); a line of the budget file, with Undo. Returns {said} or {problem}.
pub(crate) fn make(qt: &QtThread, shared: &Arc<Shared>, edit: &str) -> String {
    let (_, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    let edit: Value = serde_json::from_str(edit).unwrap_or_default();
    let today = sioul_core::zone::now().date();
    let new = acts::New {
        label: edit["label"].as_str().unwrap_or_default().to_string(),
        gross: money_of(&edit["gross"]),
        net: money_of(&edit["net"]),
        fees: fees_of(&edit["fees"]),
        account: edit["account"].as_str().and_then(some),
        budget: edit["budget"].as_str().unwrap_or_default().to_string(),
        day: edit["day"].as_str().and_then(day_of).unwrap_or(today),
        paid_on: edit["paid_on"].as_str().and_then(day_of),
        planned: edit["planned"].as_bool().unwrap_or(false),
        note: edit["note"].as_str().unwrap_or_default().to_string(),
        ties: edit["ties"].as_array().map(|t| t.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default(),
    };
    let origin = say("budget-origin-hand", &[("date", today.to_string())]);
    with_undo(qt, shared, money_files(&root), || acts::make(&root, tr(), &new, &origin, today).map(|act| act.said))
}

/// A movement changed (design §3.1): only the fields its JSON holds, each
/// as `acts::change` takes it; a field its kind does not allow is refused,
/// with why. With Undo. Returns {said} or {problem}.
pub(crate) fn change(qt: &QtThread, shared: &Arc<Shared>, address: &str, edit: &str) -> String {
    let (config, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    let edit: Value = serde_json::from_str(edit).unwrap_or_default();
    let has = |key: &str| edit.get(key).is_some();
    let today = sioul_core::zone::now().date();
    let change = acts::Change {
        label: edit["label"].as_str().map(str::to_string),
        gross: has("gross").then(|| money_of(&edit["gross"])),
        fees: has("fees").then(|| fees_of(&edit["fees"])),
        net: money_of(&edit["net"]),
        day: edit["day"].as_str().and_then(day_of),
        paid_on: has("paid_on").then(|| edit["paid_on"].as_str().and_then(day_of)),
        note: edit["note"].as_str().map(str::to_string),
        budget: edit["budget"].as_str().and_then(some),
        planned: edit["planned"].as_bool(),
        account: edit["account"].as_str().and_then(some),
    };
    let words = sioul_core::words::Words::of(&config);
    let origin = say("budget-origin-hand", &[("date", today.to_string())]);
    with_undo(qt, shared, money_files(&root), || acts::change(&root, &words, tr(), address, &change, &origin, today).map(|act| act.said))
}

/// A movement removed by what it is made from (Delete, Not this time,
/// Leave out, Not a payout), with Undo. Returns {said} or {problem}.
pub(crate) fn remove(qt: &QtThread, shared: &Arc<Shared>, address: &str) -> String {
    let (config, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    let today = sioul_core::zone::now().date();
    let words = sioul_core::words::Words::of(&config);
    with_undo(qt, shared, money_files(&root), || acts::remove(&root, &words, tr(), address, today).map(|act| act.said))
}

/// What you removed put back (Not this time, Leave out). Returns {said} or {problem}.
pub(crate) fn put_back(qt: &QtThread, shared: &Arc<Shared>, address: &str) -> String {
    let (_, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    with_undo(qt, shared, money_files(&root), || acts::put_back(&root, address).map(|_| tr().text("movement-put-back", None)))
}

// ---------------------------------------------------------------- from mail

/// The newest money mail question asked: an older answer is not shown.
static ASKED: AtomicU64 = AtomicU64::new(0);

/// Messages a first look opens at once; the rest are read after it.
const FIRST_FILL: usize = 300;

/// One money mail's row, flat, as the tab's list model holds it.
fn mail_row(ledger: &sioul_core::budget::Ledger, r: &moneyindex::MailRow, today: jiff::civil::Date) -> Value {
    let (became, movement, shape) = match &r.became {
        moneyindex::Became::Joined { address, .. } => ("joined", address.clone(), false),
        moneyindex::Became::Carried { address, .. } => ("carried", address.clone(), false),
        moneyindex::Became::Fees { address, .. } => ("fees", address.clone(), false),
        moneyindex::Became::Recorded { address, .. } => ("recorded", address.clone(), false),
        moneyindex::Became::Ignored { shape } => ("ignored", String::new(), *shape),
        moneyindex::Became::Untied => ("untied", String::new(), false),
    };
    j!({
        "key": r.key,
        "file": r.path.display().to_string(),
        "section": tr().day_in(r.date, today),
        "day": r.date.to_string(),
        "kind": tr().text(&format!("kind-{}", r.kind), None),
        "who": r.who,
        "amount": r.amount.map(|a| moneylist::signed(tr(), a)).unwrap_or_default(),
        "cents": r.amount.map_or(0, |a| a.cents()),
        "fee": r.fee.map(|f| say("money-mail-fee", &[("fees", tr().money(f))])).unwrap_or_default(),
        "subject": r.subject,
        "became": moneyindex::became_words(tr(), ledger, r, &r.sender),
        "outcome": became,
        "movement": movement,
        "shape": shape,
        "doubtful": r.doubtful,
        "domain": r.domain.clone().unwrap_or_default(),
        "sender": r.sender,
    })
}

/// The page of money mail `asked` (design §5), computed off the window's
/// thread and given as the `money_mail` property: {asked, rows, page, more,
/// waiting, chips, filter, accounts, budgets, addresses}. A first look
/// opens a few hundred messages at once and says how many wait; the rest
/// are read after it, and the page given again.
pub(crate) fn ask_mail(qt: &QtThread, asked: String) {
    let generation = ASKED.fetch_add(1, Ordering::Relaxed) + 1;
    let qt = qt.clone();
    std::thread::spawn(move || {
        let give = |page: String| {
            let _ = qt.queue(move |mut sioul| {
                if ASKED.load(Ordering::Relaxed) == generation {
                    sioul.as_mut().set_money_mail(cxx_qt_lib::QString::from(&page));
                }
            });
        };
        let (page, waiting) = mail_page(&asked, FIRST_FILL);
        give(page);
        if waiting > 0 && ASKED.load(Ordering::Relaxed) == generation {
            give(mail_page(&asked, usize::MAX).0);
        }
    });
}

/// The page and how many messages still wait to be looked at.
fn mail_page(asked_text: &str, fill: usize) -> (String, usize) {
    let (config, root) = match folder() {
        Ok(found) => found,
        Err(e) => return (j!({ "asked": asked_text, "problem": e }).to_string(), 0),
    };
    let asked: Asked = serde_json::from_str(asked_text).unwrap_or_default();
    let today = sioul_core::zone::now().date();
    let books = movements::books(&root, &sioul_core::words::Words::of(&config), today);
    let store = Store::load(&root).unwrap_or_default();
    let ignored = sioul_core::state::MoneyState::load(&sioul_core::state::MoneyState::default_path()).ignored;
    let (areas, filter) = page_areas();
    let query = moneyindex::MailQuery {
        from: day_of(&asked.from),
        to: day_of(&asked.to),
        address: some(&asked.address),
        account: some(&asked.account),
        budget: some(&asked.budget),
        tied: asked.tied,
        way: way_of(&asked.way),
        at_least: asked.at_least.filter(|a| *a > 0.0).map(Money::from),
        at_most: asked.at_most.filter(|a| *a > 0.0).map(Money::from),
        areas: Some(areas),
        kind: kind_of(&asked.kind),
        text: asked.text.clone(),
        page: asked.page,
    };
    let page = moneyindex::history(&config, &books, &store, &ignored, &query, fill);
    let mut out = choices(&books.ledger);
    out["asked"] = j!(asked_text);
    out["rows"] = page.rows.iter().map(|r| mail_row(&books.ledger, r, today)).collect();
    out["page"] = j!(page.page);
    out["more"] = j!(page.more);
    out["waiting"] = j!(if page.waiting > 0 { say("money-mail-waiting", &[("count", page.waiting.to_string())]) } else { String::new() });
    out["chips"] = j!(chips(&books.ledger, &asked, today));
    out["filter"] = serde_json::to_value(&filter).unwrap_or(Value::Null);
    out["addresses"] = moneyindex::addresses(&config).iter().map(|(id, _)| j!({ "id": id, "title": config.account(id).and_then(|a| a.address.clone()).unwrap_or_else(|| id.clone()) })).collect();
    (out.to_string(), page.waiting)
}

fn kind_of(kind: &str) -> Option<sioul_core::payments::PaymentKind> {
    use sioul_core::payments::PaymentKind;
    [PaymentKind::Received, PaymentKind::Paid, PaymentKind::Order, PaymentKind::Bill, PaymentKind::Refund].into_iter().find(|k| k.message_id().trim_start_matches("kind-") == kind)
}

/// The row a money mail act names: {key, domain, sender, subject, movement}.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MailNamed {
    key: String,
    domain: String,
    sender: String,
    subject: String,
    movement: String,
}

/// "Not a payment" on a money mail (design §5): this message, and mail of
/// its shape from its sender, no longer counted; said once on its row, with
/// Undo. Returns {said} or {problem}.
pub(crate) fn not_a_payment(qt: &QtThread, shared: &Arc<Shared>, named: &str) -> String {
    let named: MailNamed = serde_json::from_str(named).unwrap_or_default();
    let path = sioul_core::state::MoneyState::default_path();
    with_undo(qt, shared, vec![path.clone()], || {
        let mut state = sioul_core::state::MoneyState::load(&path);
        moneyindex::not_a_payment(&mut state.ignored, &named.key, some(&named.domain).as_deref(), &named.sender, &named.subject);
        state.save(&path)?;
        Ok(say("money-mail-not-counted", &[("sender", named.sender.clone())]))
    })
}

/// "Count it again": this message, and mail of its shape, counted again, with Undo.
pub(crate) fn count_again(qt: &QtThread, shared: &Arc<Shared>, named: &str) -> String {
    let named: MailNamed = serde_json::from_str(named).unwrap_or_default();
    let path = sioul_core::state::MoneyState::default_path();
    with_undo(qt, shared, vec![path.clone()], || {
        let mut state = sioul_core::state::MoneyState::load(&path);
        moneyindex::count_again(&mut state.ignored, some(&named.domain).as_deref(), &named.sender, &named.subject);
        state.ignored.remove(&named.key);
        state.save(&path)?;
        Ok(tr().text("money-mail-counted-again", None))
    })
}

/// The movements a money mail may be the same payment as: the same way,
/// within a month of its day, nearest first by day then amount; ten at most.
pub(crate) fn mail_candidates(day: &str, cents: i64) -> String {
    let Ok((config, root)) = folder() else { return "[]".into() };
    let today = sioul_core::zone::now().date();
    let books = movements::books(&root, &sioul_core::words::Words::of(&config), today);
    let Some(day) = day_of(day) else { return "[]".into() };
    let way = if cents < 0 { Way::Out } else { Way::In };
    let near = |d: jiff::civil::Date| (d - day).get_days().abs();
    let mut list: Vec<&movements::Movement> = books.movements.iter().filter(|m| m.way == way && near(m.day) <= 31).collect();
    list.sort_by_key(|m| (near(m.day), (m.gross_or_net().cents() - cents).abs().min((m.sum.amount().cents() - cents).abs())));
    let rows: Vec<Value> = list.iter().take(10).map(|m| j!({ "key": m.steady_key(), "label": moneyview::who(tr(), m), "date": moneyview::short_day(tr(), m.day), "amount": moneylist::signed(tr(), m.sum.amount()) })).collect();
    Value::Array(rows).to_string()
}

/// "The same as…" from a money mail: the mail joined to that movement, by
/// your hand, in the store; with Undo.
pub(crate) fn mail_same(qt: &QtThread, shared: &Arc<Shared>, key: &str, movement: &str) -> String {
    let (_, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    let (key, movement) = (key.to_string(), movement.to_string());
    with_undo(qt, shared, money_files(&root), || {
        Store::edit(&root, |edit| edit.link(&movement, &key, "hand", None))?;
        movements::said();
        Ok(tr().text("money-mail-joined-now", None))
    })
}

/// "Not the same" from a money mail: the mail kept apart from the movement
/// it was joined to, or the payout that carried it; with Undo.
pub(crate) fn mail_not_same(qt: &QtThread, shared: &Arc<Shared>, named: &str) -> String {
    let (config, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    let named: MailNamed = serde_json::from_str(named).unwrap_or_default();
    let today = sioul_core::zone::now().date();
    let books = movements::books(&root, &sioul_core::words::Words::of(&config), today);
    let Some((m, _)) = moneylist::get(&books, &named.movement) else { return problem(tr().text("movement-gone", None)) };
    let other = m.steady_key().to_string();
    with_undo(qt, shared, money_files(&root), || {
        acts::not_same(&root, &named.key, &other)?;
        Ok(tr().text("money-mail-apart", None))
    })
}

// ---------------------------------------------------------------- the declaration

/// A declaring budget's turnover over the declaration period that holds
/// `anchor` (its quarter or month; today's when not a day), and its
/// receipts register (design §6, "The declaration"), as JSON: {budget,
/// title, from, to, previous, next, sentences, rows, declares}.
pub(crate) fn declaration(budget: &str, anchor: &str) -> String {
    let (config, root) = match folder() {
        Ok(found) => found,
        Err(e) => return problem(e),
    };
    let today = sioul_core::zone::now().date();
    let books = movements::books(&root, &sioul_core::words::Words::of(&config), today);
    let declaring: Vec<&sioul_core::budget::Budget> = books.ledger.budgets.iter().filter(|b| receipts::declares(b)).collect();
    let Some(found) = declaring.iter().find(|b| b.id == budget).or_else(|| declaring.first()) else { return problem(tr().text("receipts-none", None)) };
    let anchor = day_of(anchor).unwrap_or(today);
    let r = receipts::of_period(&books, found, anchor);
    let shift = |d: jiff::civil::Date, days: i64| d.checked_add(jiff::Span::new().days(days)).unwrap_or(d);
    let unknown = tr().text("receipts-gross-unknown", None);
    let rows: Vec<Value> = r
        .receipts
        .iter()
        .map(|x| {
            j!({
                "day": moneyview::short_day(tr(), x.day),
                "client": x.client,
                "invoice": x.invoice,
                "means": x.means.words(tr()),
                "gross": x.gross.map_or_else(|| unknown.clone(), |g| tr().money(g)),
                "known": x.gross.is_some(),
                "address": x.address,
            })
        })
        .collect();
    j!({
        "budget": found.id,
        "title": found.title,
        "anchor": anchor.to_string(),
        "previous": shift(r.from, -1).to_string(),
        "next": if r.to < today { shift(r.to, 1).to_string() } else { String::new() },
        "sentences": receipts::sentences(tr(), &r),
        "rows": rows,
        "declares": declaring.iter().map(|b| j!({ "id": b.id, "title": b.title })).collect::<Vec<_>>(),
    })
    .to_string()
}

/// "Save the receipts register…": the register of `budget`'s period that
/// holds `anchor` written as CSV into `folder`, a folder you chose; its
/// page to print, and where its PDF goes beside it (the window prints it
/// as it prints invoices). Returns {csv, pdf, html, folder} or {error}.
pub(crate) fn save_receipts(budget: &str, anchor: &str, folder_chosen: &str) -> String {
    let refused = |e: String| j!({ "error": e }).to_string();
    let (config, root) = match folder() {
        Ok(found) => found,
        Err(e) => return refused(e),
    };
    let today = sioul_core::zone::now().date();
    let books = movements::books(&root, &sioul_core::words::Words::of(&config), today);
    let Some(found) = books.ledger.budgets.iter().find(|b| b.id == budget && receipts::declares(b)) else { return refused(tr().text("receipts-none", None)) };
    let r = receipts::of_period(&books, found, day_of(anchor).unwrap_or(today));
    let dir = PathBuf::from(folder_chosen);
    if !dir.is_dir() {
        return refused(say("invoice-not-written", &[("path", folder_chosen.to_string())]));
    }
    // "Receipts register, Work, 2026-10-01 to 2026-12-31": nothing a system refuses in a name.
    let title: String = found.title.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '-' }).collect();
    let name = say("receipts-file-name", &[("budget", title.trim().to_string()), ("from", r.from.to_string()), ("to", r.to.to_string())]);
    let csv_path = dir.join(format!("{name}.csv"));
    if let Err(e) = save_csv(&csv_path, &receipts::csv(tr(), &r)) {
        return refused(say("invoice-not-written", &[("path", format!("{} ({e})", csv_path.display()))]));
    }
    j!({
        "csv": csv_path.display().to_string(),
        "pdf": dir.join(format!("{name}.pdf")).display().to_string(),
        "html": receipts::html(tr(), &r, &found.title),
        "folder": dir.display().to_string(),
    })
    .to_string()
}

/// The register's spreadsheet, where you chose: no item of any calendar.
fn save_csv(path: &Path, csv: &str) -> std::io::Result<()> {
    std::fs::write(path, csv)
}

// ---------------------------------------------------------------- the phone's background step

/// The phone's background step (design phase 3): a few hundred messages
/// more read for money each step, after its mail, so that From mail opens
/// on what is already found; the rest at the next steps.
pub(crate) fn background_step(config: &sioul_core::config::Config) {
    let Some(root) = config.notes_root_path() else { return };
    let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
    let reader = moneyindex::Reader::new(&ledger.mail_rules, &sioul_core::words::Words::of(config));
    let _ = sioul_core::mailpass::every(&reader, &moneyindex::addresses(config), FIRST_FILL);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The filters in use, in words, each with its id for its ×; an amount
    /// as the core says amounts; none when nothing is asked.
    #[test]
    fn the_filters_in_use_are_said_in_words() {
        let ledger: sioul_core::budget::Ledger = toml::from_str("[[budget]]\nid = 'work'\ntitle = 'Work'\nperiod = 'month'\n[[bank_account]]\nid = 'main'\ntitle = 'My bank'\n").unwrap();
        let today: jiff::civil::Date = "2026-11-05".parse().unwrap();
        assert!(chips(&ledger, &Asked::default(), today).is_empty());
        let asked = Asked { account: "main".into(), budget: "work".into(), way: "in".into(), from: "2026-10-01".into(), to: "2026-10-31".into(), at_least: Some(50.0), told_by: vec!["bank".into()], ..Asked::default() };
        let said: Vec<(String, String)> = chips(&ledger, &asked, today).iter().map(|c| (c["id"].as_str().unwrap().to_string(), c["words"].as_str().unwrap().to_string())).collect();
        let ids: Vec<&str> = said.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["account", "budget", "way", "days", "amount", "told"]);
        assert_eq!(said[0].1, "My bank");
        assert_eq!(said[1].1, "Work");
        assert!(said[4].1.contains("50"), "{said:?}");
        assert!(said.iter().all(|(_, words)| !words.is_empty() && !words.contains("movements-")), "{said:?}");
    }

    /// A fee from the form: its kind, its amount without sign, by your hand; none for nothing.
    #[test]
    fn the_forms_fees_are_costs_of_your_hand() {
        let fees = fees_of(&j!([{ "kind": "commission", "amount": -12.5 }, { "kind": "processor", "amount": 0 }]));
        assert_eq!(fees, vec![Fee { kind: FeeKind::Commission, amount: Money(1250), from: FeeFrom::Hand, about: false }]);
        assert_eq!(money_of(&j!(0)), None);
        assert_eq!(money_of(&j!(-52.3)), Some(Money(-5230)));
    }
}

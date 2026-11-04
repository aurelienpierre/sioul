// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Money movements from the window (docs/accounting.md, "Moving a movement",
//! "A payment from any mail"): "A payment…" from a message or a letter, a
//! short form filled from what it says, any budget chosen whatever the hour;
//! a movement moved from its row, its effect said, and the payee's next
//! movements asked about once, your answer the consent.

use crate::backend::{load_config, say, tr};
use serde_json::json;
use sioul_core::movements::{self, Place, Way, acts};

/// Every budget, whatever the hour: what a page shows is no limit on what a
/// payment may be given to (the owner's report of 4 November 2026).
fn budgets(ledger: &sioul_core::budget::Ledger) -> Vec<serde_json::Value> {
    ledger.budgets.iter().map(|b| json!({ "id": b.id, "title": b.title })).collect()
}

/// What "A payment…" starts from: a message by its file (`kind` "mail", `key`
/// its file, `trust` its sender's level as the list says it: "verified" and
/// "own" need no check), or a paper letter by its id (`kind` "letter").
/// Returns {payment, title, budgets}, or {problem}.
pub(crate) fn from(kind: &str, key: &str, trust: &str) -> String {
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return json!({ "problem": tr().text("papers-no-store", None) }).to_string() };
    let words = sioul_core::words::Words::of(&config);
    let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
    let made = match kind {
        "letter" => sioul_core::letters::Letters::load(&root).ok().and_then(|letters| letters.get(key).cloned()).map(|letter| (acts::Payment::from_letter(&letter), letter.reading.sender.clone())),
        _ => sioul_core::maildir::locate(std::path::Path::new(key)).and_then(|path| {
            let mut card = sioul_core::card::Card::from_bytes(&std::fs::read(&path).ok()?)?;
            card.path = Some(path);
            let address = crate::work::uri_of("mail", key);
            let doubtful = !matches!(trust, "verified" | "own");
            Some((acts::Payment::from_card(&words, &card, &address, doubtful), card.subject.clone()))
        }),
    };
    match made {
        Some((payment, title)) if !payment.source.is_empty() => json!({ "payment": payment, "title": title, "budgets": budgets(&ledger) }).to_string(),
        _ => json!({ "problem": tr().text("payment-unread", None) }).to_string(),
    }
}

/// The form's payment written into the budget file (`acts::Payment::record`):
/// its message or letter in its links, a comment saying where it came from
/// (`title`: the message's subject, the letter's sender). Returns {said} or {problem}.
pub(crate) fn add(edit: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Edit {
        payment: acts::Payment,
        #[serde(default)]
        title: String,
    }
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return json!({ "problem": tr().text("papers-no-store", None) }).to_string() };
    let edit: Edit = match serde_json::from_str(edit) {
        Ok(edit) => edit,
        Err(e) => return json!({ "problem": e.to_string() }).to_string(),
    };
    let today = sioul_core::zone::now().date();
    let origin = if edit.payment.source.starts_with("sioul:letter/") { "payment-origin-letter" } else { "payment-origin-mail" };
    let origin = say(origin, &[("title", edit.title.clone()), ("date", today.to_string())]);
    match edit.payment.record(&root, today, &origin) {
        Ok(_) => {
            let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
            json!({ "said": say("ui-line-added", &[("budget", ledger.budget_title(&edit.payment.budget).to_string())]) }).to_string()
        }
        Err(problem) => json!({ "problem": problem }).to_string(),
    }
}

/// One of a bank account's movements placed from its row: `budget:<id>`,
/// `none`, or `""` (as the rules say again). Moved to a budget, its effect is
/// said ("Home holds €12 more for October 2026."), and the payee's next movements
/// are asked about once. Returns {said, ask: {payee, budget, title, way}|null} or {problem}.
pub(crate) fn place(account: &str, movement: &str, choice: &str) -> String {
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return json!({ "problem": tr().text("papers-no-store", None) }).to_string() };
    let path = root.join(sioul_core::budget::LEDGER);
    let Some(budget) = choice.strip_prefix("budget:") else {
        return match sioul_core::accounts::set_assignment(&path, account, movement, choice) {
            Ok(()) => json!({ "said": "", "ask": null }).to_string(),
            Err(problem) => json!({ "problem": problem }).to_string(),
        };
    };
    let words = sioul_core::words::Words::of(&config);
    let today = sioul_core::zone::now().date();
    let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
    // The movement, read around its own day.
    let day = sioul_core::bank::Bank::load(&root).ok().and_then(|bank| bank.movements.iter().find(|m| m.id == movement && sioul_core::accounts::account_of(&ledger, &m.account).is_some_and(|a| a.id == account)).map(|m| m.date)).unwrap_or(today);
    let shift = |days: i64| day.checked_add(jiff::Span::new().days(days)).unwrap_or(day);
    let all = movements::load(&root, &words, shift(-70), shift(70), today);
    let Some(found) = all.find(&movements::bank_key(account, movement)) else {
        return match sioul_core::accounts::set_assignment(&path, account, movement, choice) {
            Ok(()) => json!({ "said": "", "ask": null }).to_string(),
            Err(problem) => json!({ "problem": problem }).to_string(),
        };
    };
    match acts::move_to(&root, &words, found, &Place::Budget(budget.to_string())) {
        Ok(moved) => {
            let said = effect(&ledger, &moved);
            let way = if found.way == Way::In { "in" } else { "out" };
            let ask = moved.ask.map(|payee| json!({ "payee": payee, "budget": budget, "title": ledger.budget_title(budget), "way": way }));
            json!({ "said": said, "ask": ask }).to_string()
        }
        Err(problem) => json!({ "problem": problem }).to_string(),
    }
}

/// "Home holds €12 more for October 2026." ("for 2026" in a yearly budget); none when nothing moved.
fn effect(ledger: &sioul_core::budget::Ledger, moved: &acts::Moved) -> String {
    if moved.budget.is_empty() || moved.more == sioul_core::money::Money::ZERO {
        return String::new();
    }
    let yearly = ledger.budgets.iter().find(|b| b.id == moved.budget).is_some_and(|b| b.period == sioul_core::budget::Period::Year);
    let period = if yearly { moved.day.year().to_string() } else { tr().month_year(moved.day) };
    say("moved-more", &[("budget", ledger.budget_title(&moved.budget).to_string()), ("amount", tr().money(moved.more)), ("period", period)])
}

/// Your answer, once, on whether `payee`'s next movements go to `budget` too
/// (`acts::decide`): a yes is a rule of the budget file. Returns what went wrong, else what it did, in words.
pub(crate) fn decide(payee: &str, budget: &str, way: &str, next: bool) -> String {
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return tr().text("papers-no-store", None) };
    let way = if way == "in" { Way::In } else { Way::Out };
    match acts::decide(&root, payee, budget, way, next) {
        Ok(()) if next => {
            let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
            say("moved-next-yes", &[("payee", payee.to_string()), ("budget", ledger.budget_title(budget).to_string())])
        }
        Ok(()) => String::new(),
        Err(problem) => problem,
    }
}

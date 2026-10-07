// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Who someone is to you (docs/porch.md, "Who may reach you, and when"): a
//! message's sender ("Their mail"), and a contact's card ("Their list"),
//! where they stand and why, and their own choice set or taken back. The
//! lists and their order are `sioul_core::porch`'s, the states
//! `sioul_core::reach`'s.

use crate::backend::{load_config, say, tr};
use sioul_core::porch::{self, By, Judged, Standing};
use sioul_core::reach::Who;

/// A JSON array of addresses, or one address.
fn addresses(text: &str) -> Vec<String> {
    let text = text.trim();
    let list: Vec<String> = if text.starts_with('[') { serde_json::from_str(text).unwrap_or_default() } else { vec![text.to_string()] };
    list.into_iter().map(|a| a.trim().to_ascii_lowercase()).filter(|a| !a.is_empty()).collect()
}

/// The sentence saying who decided: "Safe, as the category Friends says.",
/// "Neutral: in your address book, on no list.", "A stranger: in none of
/// your address books, on no list."
fn said(judged: &Judged) -> String {
    let one = tr().text(&format!("sender-one-{}", judged.who.id()), None);
    match judged.by {
        By::Address | By::Number => say("sender-from-address", &[("list", one)]),
        By::Card => say("sender-from-card", &[("list", one), ("name", judged.name.clone())]),
        By::Category => say("sender-from-category", &[("list", one), ("name", judged.name.clone())]),
        By::Domain | By::Prefix => say("sender-from-domain", &[("list", one), ("name", judged.name.trim_start_matches("*@").to_string())]),
        By::LetIn => say("sender-from-let-in", &[("list", one)]),
        By::Book => say("sender-from-book", &[("list", one)]),
        By::Default => say("sender-from-default", &[("list", one)]),
    }
}

/// What decided, as the window reads it: "address", "number", "card", "category", "domain", "prefix", "let-in", "book", "default".
fn by_id(by: By) -> String {
    serde_json::to_value(by).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Where a sender stands, and why, for "Their mail" (`addresses`: theirs, a
/// JSON array, or one; or a caller's number, `tel:…`, judged as calls judge
/// it): {standing (one of the five states), from (what decided), name (the
/// card, category or pattern that decided), own (their first address or
/// number has its own entry), choice (that entry's list, "" when none), said
/// (a sentence), choices [{value, label}]: "As their categories say" and the
/// four lists with their mail's times}.
pub(crate) fn standing_json(text: &str) -> String {
    let config = load_config();
    let senders = porch::Senders::load(&config);
    let attention = sioul_core::attention::Attention::of(&config);
    let first = addresses(text).into_iter().next().unwrap_or_default();
    let (judged, own) = match first.strip_prefix(porch::TEL) {
        Some(number) => {
            let judged = senders.judge_number(number);
            let own = judged.by == By::Number;
            (judged, own)
        }
        None => {
            let judged = senders.judge(&first);
            let own = judged.by == By::Address;
            (judged, own)
        }
    };
    let choices: Vec<serde_json::Value> = std::iter::once(serde_json::json!({ "value": "", "label": tr().text("sender-categories", None) }))
        .chain(Standing::ALL.iter().map(|s| serde_json::json!({ "value": s.as_str(), "label": sioul_core::attention::list_choice(tr(), Who::of(*s), &attention, true) })))
        .collect();
    serde_json::json!({
        "standing": judged.who.id(),
        "from": by_id(judged.by),
        "name": judged.name,
        "own": own,
        "choice": if own { judged.who.id() } else { "" },
        "said": said(&judged),
        "choices": choices,
    })
    .to_string()
}

/// "Their mail" for every address given: on one list ("safe", "neutral",
/// "restricted", "blocked") and out of the others, or ("") out of every list,
/// their card, its categories, else their domain, deciding. Returns the status line.
pub(crate) fn set_standing_of(text: &str, standing: &str) -> String {
    let config = load_config();
    let list = addresses(text);
    let Some(first) = list.first().cloned() else { return String::new() };
    let wanted = Standing::read(standing);
    if wanted.is_none() && !standing.is_empty() {
        return String::new();
    }
    for address in &list {
        let done = match wanted {
            Some(standing) => porch::set_standing(&config, address, standing),
            None => porch::clear_standing(&config, address),
        };
        if let Err(e) = done {
            return e;
        }
    }
    let entry = porch::normalize(&first).unwrap_or(first).trim_start_matches("*@").to_string();
    match wanted {
        Some(standing) => say(&format!("sender-now-{}", standing.as_str()), &[("entry", entry)]),
        None => say("sender-now-categories", &[("entry", entry)]),
    }
}

/// The card with this file or UID, from every address book.
fn card(key: &str) -> Option<sioul_core::contacts::Contact> {
    let key = key.trim();
    (!key.is_empty()).then(|| sioul_core::contacts::all().into_iter().find(|c| c.key == key || c.uid.trim() == key)).flatten()
}

/// A contact's list, for its card ("Their list"; `key`: its file or UID):
/// {who (one of the five states), from (what decided), choice (the card's own
/// list, "" when none), said (a sentence), own [lines: an address or a
/// number of theirs with a list of its own, which comes before the card's],
/// choices [{value, label}]: "As their categories say" and the four lists}.
/// "null" for a card no address book holds.
pub(crate) fn person_json(key: &str) -> String {
    let config = load_config();
    let senders = porch::Senders::load(&config);
    let Some(card) = card(key) else { return String::from("null") };
    let judged = senders.judge_card(&card.key).unwrap_or_else(Judged::stranger);
    // Their own choice: the card's line, else the one list all their addresses are on ("Their mail" before).
    let choice = senders.person_standing(&card.key).map_or("", |s| s.as_str());
    // Their addresses and numbers with an entry of their own that says otherwise: it comes first.
    let own: Vec<String> = card
        .emails
        .iter()
        .map(|e| senders.judge(e.value.trim().trim_start_matches("mailto:")))
        .chain(card.phones.iter().map(|p| senders.judge_number(&p.value)))
        .filter(|j| matches!(j.by, By::Address | By::Number) && j.who != judged.who)
        .map(|j| say("sender-own-entry", &[("entry", j.name.clone()), ("list", tr().text(&format!("sender-one-{}", j.who.id()), None))]))
        .collect();
    // On their own card, their own choice is theirs, not "as you chose for" them.
    let sentence = if judged.by == By::Card { say("sender-from-address", &[("list", tr().text(&format!("sender-one-{}", judged.who.id()), None))]) } else { said(&judged) };
    let choices: Vec<serde_json::Value> = std::iter::once(serde_json::json!({ "value": "", "label": tr().text("sender-categories", None) }))
        .chain(Standing::ALL.iter().map(|s| serde_json::json!({ "value": s.as_str(), "label": tr().text(&format!("sender-one-{}", s.as_str()), None) })))
        .collect();
    serde_json::json!({
        "who": judged.who.id(),
        "from": by_id(judged.by),
        "choice": choice,
        "said": sentence,
        "own": own,
        "choices": choices,
    })
    .to_string()
}

/// A contact's list, for all their addresses and numbers (`key`: the card's
/// file or UID): their card on one list ("safe", "neutral", "restricted",
/// "blocked"), or ("") on none, their categories deciding; the own entries
/// of their addresses and numbers taken out. Returns the status line.
pub(crate) fn set_person(key: &str, standing: &str) -> String {
    let config = load_config();
    let Some(card) = card(key) else { return String::new() };
    let wanted = Standing::read(standing);
    if wanted.is_none() && !standing.is_empty() {
        return String::new();
    }
    let addresses: Vec<String> = card.emails.iter().map(|e| e.value.clone()).collect();
    let numbers: Vec<String> = card.phones.iter().map(|p| p.value.clone()).collect();
    if let Err(e) = porch::set_person(&config, &card.uid, &addresses, &numbers, wanted) {
        return e;
    }
    let name = if card.name.trim().is_empty() { addresses.first().cloned().unwrap_or_default() } else { card.name.trim().to_string() };
    match wanted {
        Some(standing) => say(&format!("person-now-{}", standing.as_str()), &[("name", name)]),
        None => say("person-now-categories", &[("name", name)]),
    }
}

/// An address, a number or a pattern unblocked: out of the blocked list,
/// written neutral when a card, a category or a domain would still block it.
/// Returns the status line.
pub(crate) fn unblock(entry: &str) -> String {
    let config = load_config();
    let entry = entry.trim();
    match porch::unblock(&config, entry) {
        Ok(()) => say("ui-unblocked", &[("entry", porch::Entry::read(entry, sioul_core::reach::region(&config)).map_or_else(|| entry.to_string(), |e| e.shown()))]),
        Err(e) => e,
    }
}

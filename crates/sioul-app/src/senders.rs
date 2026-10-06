// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! "Their mail", for a message's sender and a contact's card (docs/porch.md,
//! "Who may write to you"): where a person stands and why, and their own
//! choice set or taken back. The lists and their order are `sioul_core::porch`'s.

use crate::backend::{load_config, say, tr};
use sioul_core::porch::{self, By, Standing};
use sioul_core::quiet::{self, Reach};

/// A JSON array of addresses, or one address.
fn addresses(text: &str) -> Vec<String> {
    let text = text.trim();
    let list: Vec<String> = if text.starts_with('[') { serde_json::from_str(text).unwrap_or_default() } else { vec![text.to_string()] };
    list.into_iter().map(|a| a.trim().to_ascii_lowercase()).filter(|a| !a.is_empty()).collect()
}

/// Where a person stands, and why, for "Their mail" (`addresses`: theirs, a
/// JSON array, or one): {standing, from (address, category, domain, default),
/// name (the category or the pattern that decided), own (their first
/// address has its own entry), choice (that entry's list, "" when none), said
/// (a sentence), choices [{value, label}]: "As their categories say" and the
/// four lists with their times}.
pub(crate) fn standing_json(text: &str) -> String {
    let config = load_config();
    let senders = porch::Senders::load(&config);
    let reach = Reach::of(&config.reach);
    let first = addresses(text).into_iter().next().unwrap_or_default();
    let judged = senders.judge(&first);
    let own = judged.by == By::Address;
    let one = tr().text(&format!("sender-one-{}", judged.standing.as_str()), None);
    let (from, said) = match judged.by {
        By::Address => ("address", say("sender-from-address", &[("list", one)])),
        By::Category => ("category", say("sender-from-category", &[("list", one), ("name", judged.name.clone())])),
        By::Domain => ("domain", say("sender-from-domain", &[("list", one), ("name", judged.name.clone())])),
        By::Default => ("default", say("sender-from-default", &[("list", one)])),
    };
    let choices: Vec<serde_json::Value> = std::iter::once(serde_json::json!({ "value": "", "label": tr().text("sender-categories", None) }))
        .chain(Standing::ALL.iter().map(|s| serde_json::json!({ "value": s.as_str(), "label": quiet::list_choice(tr(), *s, &reach, true) })))
        .collect();
    serde_json::json!({
        "standing": judged.standing.as_str(),
        "from": from,
        "name": judged.name,
        "own": own,
        "choice": if own { judged.standing.as_str() } else { "" },
        "said": said,
        "choices": choices,
    })
    .to_string()
}

/// "Their mail" for every address given: on one list ("safe", "neutral",
/// "restricted", "blocked") and out of the others, or ("") out of every list,
/// their categories, else their domain, deciding. Returns the status line.
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

/// An address or a pattern unblocked: out of the blocked list, written
/// neutral when a category or a domain would still block it. Returns the status line.
pub(crate) fn unblock(entry: &str) -> String {
    let entry = entry.trim().to_ascii_lowercase();
    match porch::unblock(&load_config(), &entry) {
        Ok(()) => say("ui-unblocked", &[("entry", porch::normalize(&entry).unwrap_or(entry.clone()).trim_start_matches("*@").to_string())]),
        Err(e) => e,
    }
}

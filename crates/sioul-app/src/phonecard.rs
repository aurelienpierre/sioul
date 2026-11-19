// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! This phone's own card in Settings ▸ Devices (design of 18 November 2026:
//! the tab "This phone" merges into it): "What Android must allow", one
//! line per access, in the order the design gives (the role for calls,
//! contacts, the call log, notification access, the texts' permissions,
//! do-not-disturb access, exact alarms, no battery restriction; and, while
//! starred people both ways is on, the permission to modify contacts), each
//! with what it is for in a few words, its state (given, not given, cannot
//! tell) and one button that asks Android or opens its page; and Android's
//! restricted settings' card (`access::restricted`). On a computer, another
//! phone's card shows what that phone says of itself, read only.

use crate::backend::tr;
use serde_json::{Value, json};
use sioul_core::i18n::Translator;

/// What Android allows Sioul on this phone, as each access is read; none: cannot tell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Allowed {
    /// The role "Caller ID & spam app": none where Android has no such role (Android 9).
    pub role: Option<bool>,
    pub contacts: bool,
    pub history: bool,
    pub notifications: Option<bool>,
    /// The three texts' permissions; none on a phone with no SMS app.
    pub texts: Option<bool>,
    /// Do Not Disturb access; none on an Android too old for Sioul's modes.
    pub dnd: Option<bool>,
    pub alarms: bool,
    pub battery: bool,
    /// Starred people both ways is on: the permission to modify contacts, and whether given.
    pub stars: Option<bool>,
}

/// The checklist's lines: {what, label, for, state ("given", "not-given",
/// "unknown"), said, button}. Pure: tested below.
pub(crate) fn checklist(a: &Allowed, tr: &Translator) -> Vec<Value> {
    let state = |given: Option<bool>| match given {
        Some(true) => "given",
        Some(false) => "not-given",
        None => "unknown",
    };
    let mut lines: Vec<(&str, Option<bool>)> = vec![
        ("role", a.role),
        ("contacts", Some(a.contacts)),
        ("history", Some(a.history)),
        ("notifications", a.notifications),
        ("texts", a.texts),
        ("dnd", a.dnd),
        ("alarms", Some(a.alarms)),
        ("battery", Some(a.battery)),
    ];
    if let Some(stars) = a.stars {
        lines.push(("stars", Some(stars)));
    }
    lines
        .into_iter()
        .map(|(what, given)| {
            let state = state(given);
            json!({
                "what": what,
                "label": tr.text(&format!("phone-allow-{what}"), None),
                "for": tr.text(&format!("phone-allow-{what}-for"), None),
                "state": state,
                "said": tr.text(&format!("phone-allow-{state}"), None),
                "button": tr.text(if given == Some(true) { "phone-allow-open" } else { "phone-allow-ask" }, None),
            })
        })
        .collect()
}

/// The demo profile's pictures of a phone (`SIOUL_DEMO` with
/// `SIOUL_GRAB_PHONE`, tools/demo/run.sh): a phone's card stood in on a
/// computer, which has no Android to ask.
pub(crate) fn demo() -> bool {
    !cfg!(target_os = "android") && std::env::var_os("SIOUL_DEMO").is_some() && std::env::var_os("SIOUL_GRAB_PHONE").is_some()
}

/// What Android allows now on this phone; none elsewhere (the demo's pictures stand one in).
fn allowed_here() -> Option<Allowed> {
    if demo() {
        let stars = crate::backend::load_config().dnd.stars;
        return Some(Allowed { role: Some(true), contacts: true, history: false, notifications: Some(false), texts: Some(false), dnd: Some(true), alarms: true, battery: false, stars: stars.then_some(false) });
    }
    if !cfg!(target_os = "android") {
        return None;
    }
    let calls = crate::steps::java("calls-state", "{}");
    let texts = crate::steps::java("texts-state", "{}");
    let dnd = crate::dnd::can();
    let config = crate::backend::load_config();
    Some(Allowed {
        role: (calls["available"] == true).then_some(calls["held"] == true),
        contacts: calls["contacts"] == true,
        history: calls["history"] == true,
        notifications: crate::appnotes::access_here(),
        texts: texts["sms_app"].as_str().is_some_and(|app| !app.is_empty()).then(|| texts["read"] == true && texts["receive"] == true && texts["send"] == true),
        dnd: (!dnd.offers.is_empty() || dnd.on).then_some(dnd.on),
        alarms: crate::wake::allowed_bits() & 1 != 0,
        battery: crate::steps::java("battery", "{}") == true,
        stars: config.dnd.stars.then(crate::stars::writable_here),
    })
}

/// This phone's own card, as JSON: {phone, allow: [lines], restricted
/// (`access::restricted_card`)}; {phone: false} elsewhere. Read when the
/// window comes back too: back from Sioul's page in Android, the accesses
/// still held are asked again by themselves (`back`, `access::restricted_back`).
pub(crate) fn view() -> String {
    match allowed_here() {
        Some(allowed) => json!({ "phone": true, "allow": checklist(&allowed, tr()), "restricted": crate::access::restricted_card() }).to_string(),
        None => json!({ "phone": false }).to_string(),
    }
}

/// The card when the window comes back (Android's page left): the accesses
/// still held asked again by themselves first, once for each opening of
/// Sioul's page (`access::restricted_back`); only then (M10).
pub(crate) fn back() -> String {
    crate::access::restricted_back();
    view()
}

/// A line's button: Android's question, or its page for that access; "sims":
/// Android's page of the SIMs. The card again.
pub(crate) fn ask(what: &str) -> String {
    match what {
        "role" => {
            crate::steps::java("calls-ask-role", "{}");
        }
        "contacts" => crate::steps::ask_contacts(),
        "history" => {
            crate::steps::java("calls-ask-history", "{}");
        }
        "notifications" => {
            crate::appnotes::change("open-access", "{}");
        }
        "texts" => {
            crate::steps::java("texts-ask", "{}");
        }
        "dnd" => crate::dnd::open_access(),
        "alarms" => crate::wake::open_settings("exact"),
        "battery" => {
            crate::steps::java("ask-battery", "{}");
        }
        "stars" => {
            crate::steps::java("calls-ask-stars", "{}");
        }
        // Android's page of the SIMs, from the section Calls (Sioul cannot screen by SIM).
        "sims" => {
            crate::steps::java("calls-open-sims", "{}");
        }
        // "Android greys the switch?": the restricted settings' card, shown from now on (M13).
        "restricted" => crate::access::restricted_open(),
        _ => {}
    }
    view()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The checklist in the design's order, each line with what it is for,
    /// its state in words and one button; the stars' line only while
    /// starred people both ways is on; a role Android does not have, and a
    /// phone with no texts, "cannot tell"; both languages.
    #[test]
    fn what_android_must_allow_is_one_checklist() {
        let tr = Translator::new("en");
        let all = Allowed { role: Some(true), contacts: true, history: true, notifications: Some(true), texts: Some(true), dnd: Some(true), alarms: true, battery: true, stars: None };
        let whats = |a: &Allowed| checklist(a, &tr).iter().map(|l| (l["what"].as_str().unwrap().to_string(), l["state"].as_str().unwrap().to_string())).collect::<Vec<_>>();
        assert_eq!(whats(&all).iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>(), ["role", "contacts", "history", "notifications", "texts", "dnd", "alarms", "battery"]);
        assert!(whats(&all).iter().all(|(_, s)| s == "given"));
        let some = Allowed { role: None, notifications: Some(false), texts: None, stars: Some(false), ..all.clone() };
        let said = whats(&some);
        assert_eq!(said[0].1, "unknown");
        assert_eq!(said[3].1, "not-given");
        assert_eq!(said[4].1, "unknown");
        assert_eq!(said.last().unwrap(), &("stars".to_string(), "not-given".to_string()));
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for line in checklist(&some, &tr) {
                for key in ["label", "for", "said", "button"] {
                    let words = line[key].as_str().unwrap();
                    assert!(!words.is_empty() && !words.starts_with("phone-allow-"), "{language}: {key}: {words}");
                    if language == "fr" {
                        assert!(![" :", " ;", " ?", " !", "'"].iter().any(|bad| words.contains(bad)), "{words}");
                    }
                }
            }
        }
    }
}

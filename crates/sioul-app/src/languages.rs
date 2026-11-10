// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The languages found in your mail, for the window (docs/words.md, "The
//! languages found in your mail"; `sioul_core::languages`). On a computer,
//! at the window's start and then twice a day, in a thread of its own: the
//! newest 500 messages received are read; a new configuration gets its
//! languages written once, said in the status line; a language found later
//! is offered once on this device, in a quiet notice, during work or admin
//! hours only (or when no hours are set), with "Not now" and "Read German
//! too". Nothing is switched on by itself.

use crate::backend::{QtThread, config_path, load_config, set_status, tr};
use sioul_core::languages;

/// How often the mail is read again for its languages.
const EVERY: std::time::Duration = std::time::Duration::from_secs(12 * 3600);

/// Started once, at the window's start; a phone does not offer (its
/// configuration's languages come from your computers).
pub(crate) fn start(qt: &QtThread) {
    if cfg!(target_os = "android") || crate::backend::offline() {
        return;
    }
    let qt = qt.clone();
    std::thread::spawn(move || {
        // After the first fetches of the window's start.
        std::thread::sleep(std::time::Duration::from_secs(120));
        loop {
            look(&qt);
            std::thread::sleep(EVERY);
        }
    });
}

/// The words of a language's name ("German", "en allemand").
fn names(code: &str) -> (String, String) {
    (tr().text(&format!("words-language-{code}"), None), tr().text(&format!("words-language-{code}-in"), None))
}

fn look(qt: &QtThread) {
    let config = load_config();
    let lists = languages::lists();
    let found = languages::found_in(languages::mail_texts(&config, languages::LAST), &lists);
    match languages::decide_once(&config_path(), &config, &found) {
        Ok(Some(decided)) => {
            let said: Vec<String> = decided.iter().map(|code| names(code).1).collect();
            let and = tr().text("word-and", None);
            let listed = match said.as_slice() {
                [] => String::new(),
                [one] => one.clone(),
                [first @ .., last] => format!("{} {and} {last}", first.join(", ")),
            };
            set_status(qt, crate::backend::say("languages-decided", &[("languages", listed)]));
            return;
        }
        Ok(None) => {}
        Err(e) => eprintln!("Sioul: the languages of your mail: {e}"),
    }
    // Offered during work or admin hours alone, as Sioul's other notices of admin.
    let mode = crate::hours::mode_now();
    if !matches!(mode.time, sioul_core::areas::Time::Work | sioul_core::areas::Time::Admin | sioul_core::areas::Time::Any | sioul_core::areas::Time::Several(_)) || mode.paused() {
        return;
    }
    let path = languages::offered_path();
    let Some(code) = languages::to_offer(&config, &found, &languages::offered(&path)).into_iter().next() else { return };
    // Offered once on this device, taken or not.
    if let Err(e) = languages::mark_offered(&path, &code) {
        eprintln!("Sioul: the languages of your mail: {e}");
        return;
    }
    let (name, inside) = names(&code);
    let pairs = [("name", name.clone()), ("in", inside.clone())];
    let title = crate::backend::say("languages-offer-title", &pairs);
    let body = crate::backend::say("languages-offer-body", &pairs);
    let (qt_read, read_code) = (qt.clone(), code.clone());
    let read: Box<dyn FnOnce() + Send> = Box::new(move || {
        let said = match languages::read_too(&config_path(), &load_config(), &read_code) {
            Ok(()) => crate::backend::say("languages-read-now", &[("name", name), ("in", inside)]),
            Err(e) => e,
        };
        set_status(&qt_read, said);
    });
    // "Not now" first: a click on the notice itself presses the first, and changes nothing.
    let not_now: Box<dyn FnOnce() + Send> = Box::new(|| {});
    let choices = vec![(tr().text("languages-offer-not-now", None), not_now), (crate::backend::say("languages-offer-read", &pairs), read)];
    if let Err(e) = sioul_sync::notify::remind_offering(&title, &body, choices) {
        eprintln!("Sioul: the languages of your mail: {e}");
    }
}

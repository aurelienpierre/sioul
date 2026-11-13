// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The languages found in your mail, for the window (docs/words.md, "The
//! languages found in your mail"; `sioul_core::languages`). On a computer,
//! at the window's start and then twice a day, in a thread of its own: the
//! newest 500 messages received are read; a new configuration gets its
//! languages written once, said in the status line; a language found later
//! is offered once on this device, in a notice to know (docs/notifications.md),
//! during work or admin hours only (or when no hours are set), with "Read
//! German too"; a click on it opens Settings ▸ Words, where the languages are
//! chosen. Nothing is switched on by itself.

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
    let (name, inside) = names(&code);
    let pairs = [("name", name), ("in", inside)];
    use sioul_core::notice::{Act, Kind, Notice, address};
    let notice = Notice::new(address::settings("words.languages"), Kind::Minor, crate::backend::say("languages-offer-title", &pairs), crate::backend::say("languages-offer-body", &pairs))
        .action(crate::backend::say("languages-offer-read", &pairs), Act::Verb(format!("read-language:{code}")));
    // Offered once on this device, taken or not, marked once posted: one that
    // waits (nobody at the computer, a pause, do-not-disturb) is offered then.
    let claim: crate::tell::Claim = Box::new(move || {
        if let Err(e) = languages::mark_offered(&path, &code) {
            eprintln!("Sioul: the languages of your mail: {e}");
        }
        true
    });
    crate::tell::tell_with(notice, sioul_core::notice::told(None), None, Some(claim), true);
}

/// "Read German too", pressed on the offer's notice (`tell`): that
/// language's mail read too from now on, said in the status line.
pub(crate) fn read_also(qt: &QtThread, code: &str) {
    let (name, inside) = names(code);
    let said = match languages::read_too(&config_path(), &load_config(), code) {
        Ok(()) => crate::backend::say("languages-read-now", &[("name", name), ("in", inside)]),
        Err(e) => e,
    };
    set_status(qt, said);
}

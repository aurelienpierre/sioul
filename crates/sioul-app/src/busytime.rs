// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An agenda's "Counts as busy time" (docs/client.md, "Counts as busy time";
//! 26 November 2026): its one switch, on its row in Agenda ⚙ ▸ Your agendas,
//! the one value everything that counts your busy time reads
//! (`sioul_core::agendas::busy_time`). Written where it is kept:
//!
//! - **a calendar subscribed by its address**: its own setting, `engagements`;
//! - **any other**: your word in the shared settings (`busy_time`), carried to
//!   your devices; the extensions writing into it get the same choice
//!   (`[extensions.busy]`, which marks their events and which a Sioul 0.1.0
//!   reads), their events written again with it by the device that runs the
//!   extension, at once there (`extensions::choose_busy`) or at its next
//!   refresh when the switch was turned elsewhere (`extensions::door::plan`:
//!   a lesson held with the other mark is written again);
//! - **where its server keeps the mark** (`schedule-calendar-transp`, RFC 6638
//!   §9.1: Nextcloud, Murena): written there too, at once when the network
//!   allows, else after the account's next sync (`keep_account`). Once there,
//!   your word on an agenda of yours is let go: its server keeps it, so that
//!   Nextcloud's own "transparent" and this switch are one setting. On a
//!   child's or a shared agenda your word stays, since its server's opaque
//!   default for a calendar on your account would otherwise count it.

use crate::backend::{QtThread, Shared, load_config, say};
use sioul_core::agendas::{self, Agenda, BusySource, Whose};
use sioul_core::config::{Account, Config};
use sioul_core::vdir::{self, Collection, Kind, State};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The calendar at `place`, and its agenda as the settings say now.
fn find(config: &Config, place: &str) -> Option<(Collection, Agenda)> {
    let collection = vdir::collections(Kind::Calendars).into_iter().find(|c| agendas::place(c) == place)?;
    let agenda = agendas::agenda_of(&collection, config);
    Some((collection, agenda))
}

/// The switch turned to `counts` on the agenda at `place`. Returns what went
/// wrong, else "": its server's answer comes later, said on its row
/// (`agendas_changed`) and, refused, in the status line.
pub(crate) fn set(qt: &QtThread, shared: &Arc<Shared>, place: &str, counts: bool) -> String {
    let config = load_config();
    let Some((collection, agenda)) = find(&config, place) else { return String::new() };
    if !agenda.busy_time.changeable() {
        return String::new();
    }
    if collection.account == vdir::SUBSCRIBED {
        return crate::subscriptions::set_field(qt, shared, &collection.id, "engagements", if counts { "true" } else { "false" });
    }
    if let Err(problem) = agendas::set_field(&crate::backend::config_path(), place, "busy_time", if counts { "yes" } else { "no" }) {
        return problem;
    }
    // The extensions writing into it: the same choice, where a Sioul 0.1.0 reads it, on every
    // device; their events written again with it here when this device runs the extension.
    for key in agendas::extension_agendas(&config, place) {
        if let Some((extension, its)) = key.split_once('/') {
            if let Err(problem) = sioul_core::config::set_extension_busy(&crate::backend::config_path(), extension, its, counts) {
                return problem;
            }
            crate::extensions::choose_busy(qt, extension, its, counts);
        }
    }
    if agenda.busy_time.server_keeps {
        let (qt, place) = (qt.clone(), place.to_string());
        std::thread::spawn(move || {
            if let Err(problem) = push(&load_config(), &place) {
                eprintln!("sioul: busy time: {place}: {problem}");
                crate::backend::set_status(&qt, say("agenda-busy-not-yet", &[]));
            }
            let _ = qt.queue(|mut sioul| sioul.as_mut().agendas_changed());
        });
    }
    String::new()
}

/// The account a calendar is kept on, when its server may be told its mark.
fn account_of<'a>(config: &'a Config, account: &str) -> Option<&'a Account> {
    config.accounts.iter().find(|a| a.id == account && a.is_dav() && a.auth.as_deref() != Some("google"))
}

/// Your word on the agenda at `place` written as its server's mark, when
/// they differ; once its server has it, the word let go on an agenda of
/// yours (`let_go`). Ok(false) when nothing needed writing.
fn push(config: &Config, place: &str) -> Result<bool, String> {
    let Some((collection, agenda)) = find(config, place) else { return Ok(false) };
    if agenda.busy_time.from != BusySource::Word || !agenda.busy_time.server_keeps {
        return Ok(false);
    }
    let counts = agenda.busy_time.counts;
    if agenda.busy_time.pending {
        let account = account_of(config, &collection.account).ok_or_else(|| format!("{}: no calendars account", collection.account))?;
        let state = State::load(&collection.state_path());
        if state.url.is_empty() || state.pending {
            return Err("not on its server yet".into());
        }
        let client = sioul_sync::dav::client_for(account).map_err(|e| e.to_string())?;
        sioul_sync::calendars::set_transparent(&client, &state.url, !counts).map_err(|e| e.to_string())?;
        vdir::note_calendar_mark(&collection.account, &collection.id, !counts)?;
    }
    if let_go(config, &agenda) {
        agendas::set_field(&crate::backend::config_path(), place, "busy_time", "")?;
    }
    Ok(agenda.busy_time.pending)
}

/// Whether your word on an agenda is let go once its server keeps the same
/// mark: an agenda of yours alone, that no extension writes into (their
/// choices stay where a Sioul 0.1.0 reads them, and count as your word).
fn let_go(config: &Config, agenda: &Agenda) -> bool {
    agenda.whose == Whose::Yours && agenda.said.busy_word().is_some() && agendas::extension_agendas(config, &agenda.id).is_empty()
}

/// After a sync of `account`: each word of yours its server's mark differs
/// from written there (a switch turned while the network was away, or on a
/// device on 0.1.0's settings), each one it already keeps let go. Never more
/// than once a quarter of an hour for a refusal (`TRIED`).
pub(crate) fn keep_account(account: &str) {
    let config = load_config();
    if account_of(&config, account).is_none() {
        return;
    }
    let now = sioul_core::zone::now().timestamp().as_second();
    // The agendas with a word of yours, turned on their row: never an extension's choice alone (F7).
    let worded: Vec<&String> = config.agenda.calendars.iter().filter(|(_, said)| said.busy_word().is_some()).map(|(place, _)| place).collect();
    for place in worded {
        if place.split_once('/').map(|(a, _)| a) != Some(account) {
            continue;
        }
        if TRIED.lock().ok().and_then(|t| t.get(place).copied()).is_some_and(|at| now - at < 15 * 60) {
            continue;
        }
        match push(&config, place) {
            Ok(_) => {
                if let Ok(mut tried) = TRIED.lock() {
                    tried.remove(place);
                }
            }
            Err(problem) => {
                eprintln!("sioul: busy time: {place}: {problem}");
                if let Ok(mut tried) = TRIED.lock() {
                    tried.insert(place.clone(), now);
                }
            }
        }
    }
}

/// When writing a calendar's mark last failed here, by place.
static TRIED: Mutex<BTreeMap<String, i64>> = Mutex::new(BTreeMap::new());

/// The sentence of "Busy time" on its row (`short` in a few words): why the
/// switch cannot be turned, or that its server has not got your word yet,
/// or counts what Sioul does not; "" otherwise.
pub(crate) fn line(agenda: &Agenda) -> String {
    if agenda.mirror {
        return say("agenda-busy-mirror", &[]);
    }
    if !agenda.events {
        return say("agenda-busy-tasks", &[]);
    }
    match agenda.busy_time.from {
        BusySource::Theirs => say("agenda-busy-theirs", &[]),
        BusySource::Holidays => say("agenda-busy-holidays", &[]),
        _ if agenda.busy_time.pending => say("agenda-busy-pending", &[]),
        _ if agenda.busy_time.differs => say("agenda-busy-differs", &[]),
        _ => String::new(),
    }
}

/// When each account's calendars were last synced in this process, by account.
static SYNCED: Mutex<BTreeMap<String, i64>> = Mutex::new(BTreeMap::new());

/// An account's sync went through now (`pim::reported`).
pub(crate) fn synced_now(account: &str) {
    if let Ok(mut synced) = SYNCED.lock() {
        synced.insert(account.to_string(), sioul_core::zone::now().timestamp().as_second());
    }
}

/// An agenda's "Synced", for its row, from what this device knows alone
/// (nothing asked of a server), in one form for every agenda: when it was
/// last synced or read ("at 09:12", "yesterday at 09:12", a day: this
/// process's record of its account's sync, else when its sync state was last
/// written; a calendar read by its address, when its copy was last found
/// current), else a short word: this device, not yet, did not go through
/// (warm). Then the full sentence, for its tooltip and a phone's block.
pub(crate) fn synced(collection: &Collection, problems: &std::collections::BTreeSet<String>, subscription: &serde_json::Value) -> Synced {
    let now = sioul_core::zone::now().timestamp().as_second();
    let short = |key: &str| say(key, &[]);
    if collection.account == vdir::SUBSCRIBED {
        let fetched = sioul_core::subscribed::Fetched::load(&collection.id);
        let full = subscription["line"].as_str().unwrap_or_default().to_string();
        return match (fetched.checked, fetched.problem.is_empty()) {
            (_, false) => Synced { short: short("agenda-synced-problem"), full, warn: true },
            (0, true) => Synced { short: short("agenda-synced-not-yet"), full, warn: false },
            (at, true) => Synced { short: crate::mirrors::when(at, now), full, warn: false },
        };
    }
    if collection.account == vdir::LOCAL {
        return Synced { short: short("agenda-synced-here"), full: say("agenda-synced-here-full", &[]), warn: false };
    }
    let path = collection.state_path();
    let state = State::load(&path);
    if state.url.is_empty() || state.pending {
        return Synced { short: short("agenda-synced-not-yet"), full: say("agenda-synced-not-yet-full", &[]), warn: false };
    }
    if problems.contains(&collection.account) {
        return Synced { short: short("agenda-synced-problem"), full: say("agenda-synced-problem-full", &[]), warn: true };
    }
    let written = crate::files::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).and_then(|d| i64::try_from(d.as_secs()).ok());
    let at = SYNCED.lock().ok().and_then(|s| s.get(&collection.account).copied()).or(written);
    let short = at.map(|at| crate::mirrors::when(at, now)).unwrap_or_else(|| short("agenda-synced-not-yet"));
    Synced { full: short.clone(), short, warn: false }
}

/// An agenda's "Synced": its short words for the cell, the sentence, whether it is said warm.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Synced {
    pub short: String,
    pub full: String,
    pub warn: bool,
}

/// The few words of "Busy time" in its cell, beside or in place of its
/// switch, so that no row's cell is empty or cut (review of edef0091, F10;
/// the coordinator's notes, 2): why it cannot be turned (another person's,
/// holidays, a list of tasks, Sioul's copy of your busy times), that its
/// server has not got your word yet, or that its server counts what Sioul
/// does not; "" for a switch alone. The sentence is `line`.
pub(crate) fn short(agenda: &Agenda) -> String {
    if agenda.mirror {
        return say("agenda-busy-short-mirror", &[]);
    }
    if !agenda.events {
        return say("agenda-busy-short-tasks", &[]);
    }
    match agenda.busy_time.from {
        BusySource::Theirs => say("agenda-busy-short-theirs", &[]),
        BusySource::Holidays => say("agenda-busy-short-holidays", &[]),
        _ if agenda.busy_time.pending => say("agenda-busy-short-pending", &[]),
        _ if agenda.busy_time.differs => say("agenda-busy-short-differs", &[]),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Let go once on its server: your own agenda alone, never a child's nor
    /// one an extension writes into, never without a word.
    #[test]
    fn a_word_is_let_go_only_for_your_own_agenda() {
        let collection = Collection { kind: Kind::Calendars, account: "cloud".into(), id: "home".into(), dir: std::path::PathBuf::from("/nowhere/cloud/home"), name: "Home".into(), color: None, read_only: false, components: vec!["VEVENT".into()] };
        let of = |config: &Config, said: agendas::Said| Agenda::of(&collection, &said, config, false, &|| true);
        let plain = Config::default();
        let word = agendas::Said { busy_time: Some("no".into()), ..agendas::Said::default() };
        assert!(let_go(&plain, &of(&plain, word.clone())));
        assert!(!let_go(&plain, &of(&plain, agendas::Said::default())), "no word: nothing to let go");
        assert!(!let_go(&plain, &of(&plain, agendas::Said { whose: Some("child".into()), ..word.clone() })), "a child's: kept");
        let extension: Config = toml::from_str("[extensions.calendars]\n\"school/p-1\" = \"cloud/home\"\n").unwrap();
        assert!(!let_go(&extension, &of(&extension, word)), "an extension writes into it: kept");
    }

    /// "Busy time" in its cell is never empty nor cut: a few words where there
    /// is no switch (Sioul's copy, a list of tasks, another person's,
    /// holidays) or where its server differs; nothing beside a plain switch.
    #[test]
    fn its_cell_says_why_in_a_few_words() {
        let collection = Collection { kind: Kind::Calendars, account: "cloud".into(), id: "home".into(), dir: std::path::PathBuf::from("/nowhere/cloud/home"), name: "Home".into(), color: None, read_only: false, components: vec!["VEVENT".into()] };
        let plain = Config::default();
        let of = |said: agendas::Said| Agenda::of(&collection, &said, &plain, false, &|| true);
        assert_eq!(short(&of(agendas::Said::default())), "", "a plain switch says nothing more");
        assert_eq!(short(&Agenda { mirror: true, ..of(agendas::Said::default()) }), say("agenda-busy-short-mirror", &[]));
        assert_eq!(short(&Agenda { events: false, ..of(agendas::Said::default()) }), say("agenda-busy-short-tasks", &[]));
        let partner = of(agendas::Said { whose: Some("partner".into()), ..agendas::Said::default() });
        assert_eq!((short(&partner), line(&partner)), (say("agenda-busy-short-theirs", &[]), say("agenda-busy-theirs", &[])));
        let mut child = of(agendas::Said { whose: Some("child".into()), ..agendas::Said::default() });
        child.busy_time = agendas::busy_time(&child, &plain, agendas::Server { mark: Some(false), owned: Some(true) });
        assert_eq!(short(&child), say("agenda-busy-short-differs", &[]), "its server counts it, Sioul does not");
    }
}

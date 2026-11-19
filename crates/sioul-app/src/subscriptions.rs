// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The calendars subscribed by their web address, on this device (sioul-core's
//! `subscribed`, sioul-sync's `feeds`; docs/client.md, "Calendars by their
//! address"): each device reads them by itself, never on the window's thread,
//! one round at a time:
//!
//! - **when Sioul opens** (the window's first minute), each calendar not read
//!   in the last ten minutes;
//! - **every minute after, on a computer**, those whose time has come
//!   (`subscribed::due`: three hours, a day for holidays, the calendar's own
//!   wish between an hour and a day);
//! - **from a phone's background step** (`background_step`), at most every
//!   six hours while you are awake, beside the mail, so that the radio wakes
//!   once (`steps`);
//! - **Refresh now**, on its line (`refresh_now`).
//!
//! Before each round the folders follow the settings (`subscribed::settle`):
//! a subscription made on another device gets its folder here, one stopped
//! there loses it. A calendar of holidays changed, the series that pause for
//! it are kept up to date (`series`), and the plan made again.

use crate::backend::{QtThread, Shared, json, load_config, tell, tr};
use sioul_core::subscribed::{self, Taken};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The least time between two readings of a calendar here (F23): on a phone,
/// a few times a day (`steps::SUBSCRIBED_EVERY`, six hours; a calendar of
/// holidays once a day, `subscribed::every`), each calendar's own last
/// asking kept in its state, so that a new process does not read again;
/// on a computer, as `subscribed::every` says.
fn at_least() -> i64 {
    if cfg!(target_os = "android") { crate::steps::SUBSCRIBED_EVERY } else { 0 }
}

/// Whether this device reads calendars on the network it is on now: never
/// on one the system calls metered when this device keeps its large
/// downloads to Wi-Fi or other unmetered networks (its own choice,
/// Settings ▸ Mail ▸ the filter's download, `[spam] train_network`). Refresh
/// now, pressed by you, reads whatever the network (F23).
fn network_allows(config: &sioul_core::config::Config) -> bool {
    let metered = if cfg!(target_os = "android") { crate::spam::phone_power().metered } else { sioul_sync::power::kept_power().and_then(|p| p.metered) };
    config.spam.downloads_on(metered)
}

/// A round running: one at a time, whoever asks.
static RUNNING: AtomicBool = AtomicBool::new(false);
/// The window's first minute passed: Sioul opened.
static OPENED: AtomicBool = AtomicBool::new(false);

/// What a round read: each calendar asked, and what became of it.
type Round = Vec<(String, Taken)>;

/// A round running, set back when it ends, a panic in it included (F5): a
/// round that fails never stops the next ones.
struct Running;

impl Running {
    /// The round's mark taken; none while another round runs.
    fn take() -> Option<Running> {
        (!RUNNING.swap(true, Ordering::Relaxed)).then_some(Running)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Relaxed);
    }
}

/// The calendars whose answer could not even be noted here (a full disk:
/// their state not written, F4), each with how many times in a row and
/// when it may be asked again: an hour, then two, four… a day at most.
static HELD_BACK: std::sync::Mutex<std::collections::BTreeMap<String, (u32, i64)>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// One round: the folders made to follow the settings, then each calendar
/// due (or `only` that one, whatever its time) asked and taken in.
fn round(opening: bool, only: Option<&str>) -> Round {
    // Settings that do not read change nothing: no copy taken out, none read (F19).
    let Some(config) = crate::backend::config_read() else { return Vec::new() };
    subscribed::settle(&config);
    let mut out = Vec::new();
    for (id, subscription) in &config.agenda.subscriptions {
        let now = jiff::Timestamp::now().as_second();
        // An entry that does not read is neither read nor taken out (F20).
        if subscription.unread || only.is_some_and(|o| o != id) || (only.is_none() && !subscribed::due(id, subscription, now, opening, at_least())) {
            continue;
        }
        let held_back = HELD_BACK.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(id).copied();
        if only.is_none() && held_back.is_some_and(|(_, next)| now < next) {
            continue;
        }
        let answer = sioul_sync::feeds::fetch(&subscription.url, subscribed::validators(id, subscription));
        // Said by its id alone: its address never goes to a log.
        let mut held = HELD_BACK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match subscribed::take(id, subscription, answer, now) {
            Ok(taken) => {
                held.remove(id);
                out.push((id.clone(), taken));
            }
            Err(e) => {
                eprintln!("subscribed calendar {id}: {e}");
                let times = held_back.map_or(1, |(n, _)| n.saturating_add(1));
                let wait = (subscribed::SOONEST.saturating_mul(1i64 << times.saturating_sub(1).min(16))).min(subscribed::LATEST);
                held.insert(id.clone(), (times, now + wait));
            }
        }
    }
    if !out.is_empty() {
        // Each line says when it was read, or why not: the agendas' block reads again.
        sioul_core::changes::said(sioul_core::changes::Kind::Agendas);
    }
    out
}

/// After a round: a calendar of holidays changed, the series that pause for
/// it kept up to date, and the plan, the hours and the agenda made again
/// (their days off may have moved).
fn after(qt: &QtThread, shared: &Arc<Shared>, round: &Round) {
    let config = load_config();
    let holidays_changed = round.iter().any(|(id, taken)| *taken == Taken::Changed && config.agenda.subscriptions.get(id).is_some_and(|s| s.holidays().is_some()));
    if holidays_changed {
        crate::series::keep_paused(qt, shared, "holidays");
        crate::work::show_work(qt, shared);
        crate::homecard::now();
    }
}

/// The window's minute: the first one reads what Sioul opening asks; each
/// one after, what is due; on a thread of its own, never while another
/// round runs.
pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    // A phone's window starts cold many times a day: no reading for its opening there (F23).
    let opening = !OPENED.swap(true, Ordering::Relaxed) && !cfg!(target_os = "android");
    // Nothing subscribed, nothing left here to take out: no round.
    let left = std::fs::read_dir(sioul_core::vdir::Kind::Calendars.root().join(subscribed::ACCOUNT)).is_ok_and(|mut d| d.next().is_some());
    // Settings that do not read: no round (F19).
    let Some(config) = crate::backend::config_read() else { return };
    if config.agenda.subscriptions.is_empty() && !left {
        return;
    }
    // A network this device keeps large downloads off: no reading on it (F23).
    if !network_allows(&config) {
        return;
    }
    let Some(running) = Running::take() else { return };
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let done = round(opening, None);
        drop(running);
        after(&qt, &shared, &done);
    });
}

/// "Refresh now" on a calendar's line: read at once, whatever its time,
/// and said in the status line. Returns "" (the round runs apart).
pub(crate) fn refresh_now(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let Some(running) = Running::take() else { return tr().text("subscribe-reading", None) };
    let (qt, shared, id) = (qt.clone(), Arc::clone(shared), id.to_string());
    std::thread::spawn(move || {
        let done = round(false, Some(&id));
        drop(running);
        let said = match done.first().map(|(_, taken)| taken) {
            Some(Taken::Changed) => tr().text("subscribe-refreshed", None),
            Some(Taken::Unchanged) => tr().text("subscribe-unchanged", None),
            Some(Taken::Kept(problem)) => problem.said(tr()),
            None => String::new(),
        };
        if !said.is_empty() {
            tell(&qt, &shared, said);
        }
        after(&qt, &shared, &done);
    });
    String::new()
}

/// A phone's background step (`steps`): the calendars due read, those of
/// holidays changed then kept as on a computer (the series that pause for
/// them); no window to tell.
pub(crate) fn background_step() {
    if !crate::backend::config_read().is_some_and(|c| network_allows(&c)) {
        return;
    }
    let Some(running) = Running::take() else { return };
    let done = round(false, None);
    drop(running);
    let config = load_config();
    if done.iter().any(|(id, taken)| *taken == Taken::Changed && config.agenda.subscriptions.get(id).is_some_and(|s| s.holidays().is_some())) {
        crate::series::keep_paused_here("holidays");
    }
}

/// What a subscribed agenda's row adds in Agenda ⚙ ▸ Your agendas: its
/// address (yours to see and change, here alone), what it holds, and its
/// line on this device; none for another agenda.
pub(crate) fn row(config: &sioul_core::config::Config, place: &str) -> serde_json::Value {
    let Some(id) = subscribed::id_of(place) else { return serde_json::Value::Null };
    let now = sioul_core::zone::now();
    // Settings that do not read: its copy stays, and its line says why nothing else shows (F19).
    if crate::backend::config_read().is_none() {
        return serde_json::json!({ "id": id, "url": "", "name": "", "holidays": "", "days_off": false, "after_class": false, "engagements": false, "unread": true, "line": subscribed::line_unread(id, true, &now, tr()) });
    }
    let Some(subscription) = config.agenda.subscriptions.get(id) else { return serde_json::Value::Null };
    let line = if subscription.unread { subscribed::line_unread(id, false, &now, tr()) } else { subscribed::line(id, &now, tr()) };
    serde_json::json!({
        "id": id,
        "url": subscription.url,
        "name": subscription.name,
        "holidays": subscription.holidays().map_or("", |h| h.id()),
        "days_off": subscription.days_off(),
        "after_class": subscription.after_class(),
        "engagements": subscription.engagements(),
        "unread": subscription.unread,
        "line": line,
    })
}

/// The official French calendars offered, with their words (Agenda ⚙ ▸
/// Subscribe to a calendar…): public holidays by territory, school holidays by zone.
pub(crate) fn officials() -> String {
    let config = load_config();
    let list: Vec<serde_json::Value> = subscribed::officials()
        .into_iter()
        .map(|o| {
            let kind = o.holidays.id();
            let mut args = sioul_core::i18n::args();
            args.set("name", o.name);
            serde_json::json!({
                "id": o.id,
                "holidays": kind,
                "name": tr().text(&format!("subscribe-official-{kind}"), Some(&args)),
                "hint": o.hint,
                "url": o.url,
                // Subscribed already: offered, said so.
                "taken": config.agenda.subscriptions.values().any(|s| s.url == o.url),
            })
        })
        .collect();
    json(&serde_json::json!({ "officials": list }))
}

/// A new subscription from the window ({url, name, holidays, days_off,
/// after_class, id, whose, person, area}): written into the settings, its
/// agenda's role with it, then read at once. Its id and what went wrong, as JSON.
pub(crate) fn subscribe(qt: &QtThread, shared: &Arc<Shared>, text: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Asked {
        #[serde(flatten)]
        new: subscribed::New,
        #[serde(default)]
        whose: String,
        #[serde(default)]
        person: String,
        #[serde(default)]
        area: String,
    }
    let answer = |id: &str, problem: &str| json(&serde_json::json!({ "id": id, "problem": problem }));
    let Ok(asked) = serde_json::from_str::<Asked>(text) else { return answer("", &tr().text("subscribe-not-address", None)) };
    let config = load_config();
    let path = crate::backend::config_path();
    let id = match subscribed::subscribe(&path, &config, &asked.new, tr()) {
        Ok(id) => id,
        Err(problem) => return answer("", &problem),
    };
    let place = subscribed::place(&id);
    for (field, value) in [("whose", asked.whose.as_str()), ("name", asked.person.as_str()), ("area", asked.area.as_str())] {
        if !value.trim().is_empty() && !(field == "whose" && value == "yours") {
            let _ = sioul_core::agendas::set_field(&path, &place, field, value);
        }
    }
    // A child's timetable goes to that child: their person found, or made (docs/persons.md).
    let problem = crate::persons::tie_role(&place);
    if let Some(config) = crate::backend::config_read() {
        subscribed::settle(&config);
    }
    let busy = refresh_now(qt, shared, &id);
    answer(&id, if problem.is_empty() { &busy } else { &problem })
}

/// One field of a subscription written (`subscribed::set`); an address
/// changed, read again at once. Returns what went wrong, else "".
pub(crate) fn set_field(qt: &QtThread, shared: &Arc<Shared>, id: &str, field: &str, value: &str) -> String {
    if let Err(problem) = subscribed::set(&crate::backend::config_path(), id, field, value, tr()) {
        return problem;
    }
    if let Some(config) = crate::backend::config_read() {
        subscribed::settle(&config);
    }
    match field {
        "url" => refresh_now(qt, shared, id),
        // What counts as your days off, when holidays begin, or whether its events hold your time, changed: the series, the plan, the hours.
        "days_off" | "after_class" | "holidays" | "engagements" => {
            crate::work::show_work(qt, shared);
            crate::homecard::now();
            String::new()
        }
        _ => String::new(),
    }
}

/// A subscription stopped (`subscribed::stop`), here and, at their next
/// look, on your other devices. Returns what went wrong, else "".
pub(crate) fn stop(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    match subscribed::stop(&crate::backend::config_path(), id) {
        Ok(()) => {
            crate::work::show_work(qt, shared);
            String::new()
        }
        Err(e) => e,
    }
}

/// Settings ▸ Hours ▸ Vacations and holidays: each calendar of holidays,
/// whether it counts as your days off, and whether this device knows its
/// days yet, as JSON.
pub(crate) fn holidays() -> String {
    let config = load_config();
    let now = sioul_core::zone::now();
    let list: Vec<serde_json::Value> = config
        .agenda
        .subscriptions
        .iter()
        .filter_map(|(id, s)| s.holidays().map(|kind| (id, s, kind)))
        .map(|(id, s, kind)| {
            let known = subscribed::Fetched::load(id).as_of().is_some();
            serde_json::json!({
                "id": id,
                "name": subscribed::display_name(s, &subscribed::Fetched::load(id).name),
                "holidays": kind.id(),
                "days_off": s.days_off(),
                "after_class": s.after_class(),
                "known": known,
                "line": if known { subscribed::line(id, &now, tr()) } else { tr().text("vacations-not-read-here", None) },
            })
        })
        .collect();
    json(&serde_json::json!({ "calendars": list }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The review's F19: a settings file that does not read reads as no
    /// subscription at all; no round runs on its word: no copy taken out,
    /// none read, and the calendar's line says why.
    #[test]
    fn settings_that_do_not_read_take_no_copy_out() {
        sioul_core::config::stand_ins_only();
        let dir = sioul_core::config::data_dir().join(format!("f19-subscriptions-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "language = \"en\"\n[agenda.subscriptions.f19-test]\nurl = \"https://calendar.example.org/a.ics\"\n[agenda\nbroken").unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        let id = "f19-test";
        sioul_core::vdir::prepare_subscribed(id, "Classes", None).unwrap();
        assert!(crate::backend::config_read().is_none());
        assert!(round(false, None).is_empty(), "nothing read");
        assert!(subscribed::folder(id).is_dir(), "its copy stays");
        let row = row(&load_config(), &subscribed::place(id));
        assert!(row["line"].as_str().unwrap_or_default().starts_with(&tr().text("subscribe-line-settings-unread", None)), "{row}");
        assert_eq!(row["unread"], true);
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        sioul_core::vdir::remove_subscribed(id).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The review's F5: a round that panics sets its mark back, so that the
    /// next rounds run.
    #[test]
    fn a_round_that_fails_does_not_stop_the_next() {
        let held = std::panic::catch_unwind(|| {
            let _running = Running::take();
            panic!("a hostile answer");
        });
        assert!(held.is_err());
        let next = Running::take();
        assert!(next.is_some(), "the next round runs");
    }
}

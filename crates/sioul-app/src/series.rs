// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The series that pause, kept up to date on this device (sioul-core's
//! `series`; docs/client.md, "A series that ends, and pauses"): each event
//! of a calendar you may write that says what it pauses during, its pauses
//! settled for the eighteen months ahead, through the agenda's one door
//! (`vdir::write_item`, which refuses a calendar you only read and a meeting
//! with others). Asked when a vacation is typed, moved or removed, when a
//! calendar of holidays changes or is switched, when a series is saved, and
//! once a day as the days pass; never on the window's thread, one round at
//! a time.
//!
//! Between two devices the one whose copy of the holidays is the freshest
//! writes (`series::may_write`, the busy calendars' rule): what it wrote
//! from is noted in its sealed entry (`devices::Entry::mirrors`, under the
//! name `pauses`), and in the series itself (`X-SIOUL-PAUSED-AS-OF`).

use crate::backend::{QtThread, Shared, json, load_config, tr};
use sioul_core::series::{self, Source};
use sioul_core::vdir::{self, Kind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A round running.
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Asked again while a round ran: one more after it.
static AGAIN: AtomicBool = AtomicBool::new(false);

/// The name under which this device notes what it wrote pauses from, in its sealed entry.
pub(crate) const NOTED: &str = "pauses";

/// The pauses kept up to date, on a thread of its own (`why`: what asked,
/// for the log); the agenda and the plan made again when a series changed.
pub(crate) fn keep_paused(qt: &QtThread, shared: &Arc<Shared>, why: &str) {
    let (qt, shared, why) = (qt.clone(), Arc::clone(shared), why.to_string());
    std::thread::spawn(move || {
        if keep_paused_here(&why) > 0 {
            crate::pim::show_pim(&qt, &shared);
            crate::work::show_work(&qt, &shared);
        }
    });
}

/// What the pauses were last kept from, on this device: your vacations,
/// your calendars of holidays as the settings say them, and the day.
static KEPT_FROM: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

/// The window's minute: the pauses kept up to date when your vacations or
/// your calendars of holidays changed in the settings (here, or brought by
/// another device), and at each new day.
pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    use std::hash::{Hash, Hasher};
    let config = load_config();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sioul_core::zone::now().date().to_string().hash(&mut hasher);
    for off in sioul_core::vacations::DaysOff::typed_of(&config).periods() {
        (off.from.to_string(), off.until.to_string(), off.why.id()).hash(&mut hasher);
    }
    for (id, s) in &config.agenda.subscriptions {
        (id, s.holidays(), s.after_class()).hash(&mut hasher);
    }
    let signature = hasher.finish();
    let Ok(mut kept) = KEPT_FROM.lock() else { return };
    if *kept == signature {
        return;
    }
    *kept = signature;
    drop(kept);
    keep_paused(qt, shared, "settings");
}

/// One round, on this thread (the phone's background step's too): each
/// series that pauses settled, those this device may write. How many changed.
pub(crate) fn keep_paused_here(why: &str) -> usize {
    if RUNNING.swap(true, Ordering::Relaxed) {
        AGAIN.store(true, Ordering::Relaxed);
        return 0;
    }
    let mut changed = 0;
    loop {
        AGAIN.store(false, Ordering::Relaxed);
        changed += round(why);
        if !AGAIN.load(Ordering::Relaxed) {
            break;
        }
    }
    RUNNING.store(false, Ordering::Relaxed);
    changed
}

/// The round itself.
fn round(why: &str) -> usize {
    let config = load_config();
    let zone = sioul_core::zone::current();
    let now = sioul_core::zone::now();
    let today = now.date();
    let days = series::Days::of(&config, &zone, today);
    let from = sioul_core::timeline::midnight(today, &zone);
    let to = from + series::AHEAD_DAYS * 86_400;
    let stamp = now.timestamp().as_second();
    let others: Vec<i64> = crate::devices::entries().map(|(own, all)| sioul_sync::devices::mirror_writers(&all, &own, NOTED, stamp)).unwrap_or_default();
    let mut changed = 0;
    let mut wrote_from: Option<i64> = None;
    for collection in vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only && c.holds("VEVENT") && c.account != vdir::SUBSCRIBED) {
        for item in collection.items() {
            let Ok(text) = std::fs::read_to_string(&item) else { continue };
            if !text.contains("X-SIOUL-PAUSE") {
                continue;
            }
            let view = days.view(&series::all_sources(&text, &zone));
            if !series::may_write(view, &others, &text) {
                continue;
            }
            let as_of = sioul_core::availability::as_of(view.unwrap_or((stamp, stamp)));
            let Some(new) = series::keep(&text, &days, from, to, &zone, as_of) else { continue };
            match vdir::write_item(&item, &new) {
                Ok(()) => {
                    changed += 1;
                    wrote_from = wrote_from.max(Some(as_of));
                }
                // Said by its file's name alone, and why it was refused (a meeting with others, a calendar you only read).
                Err(e) => eprintln!("series ({why}): {}: {e}", item.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            }
        }
    }
    if let Some(at) = wrote_from {
        note_written(at);
    }
    changed
}

/// What this device wrote pauses from, in its sealed entry (`devices::Entry::mirrors`).
fn note_written(as_of: i64) {
    let Some((_, vault)) = crate::share::vault() else { return };
    let path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    let _ = sioul_sync::devices::change(&path, vault.as_ref().map(|(folder, key)| (folder.as_path(), key)), |entry| {
        entry.mirrors.insert(NOTED.to_string(), as_of);
    });
}

/// A source's name, as the form and the card say it: "my vacations",
/// "public holidays", a school calendar's own name.
pub(crate) fn source_name(config: &sioul_core::config::Config, source: &Source) -> String {
    match source {
        Source::Vacations => tr().text("series-pause-vacations", None),
        Source::Public => tr().text("series-pause-public", None),
        Source::School(id) => config.agenda.subscriptions.get(id).map_or_else(|| tr().text("subscribe-holidays-school", None), |s| sioul_core::subscribed::display_name(s, &sioul_core::subscribed::Fetched::load(id).name)),
    }
}

/// What a series may pause during, for the event form (`pauseSources`): my
/// vacations, public holidays, each school-holidays calendar by its name, as JSON.
pub(crate) fn sources() -> String {
    let config = load_config();
    let mut list = vec![Source::Vacations, Source::Public];
    list.extend(config.agenda.subscriptions.iter().filter(|(_, s)| s.holidays() == Some(sioul_core::subscribed::Holidays::School)).map(|(id, _)| Source::School(id.clone())));
    let rows: Vec<serde_json::Value> = list.iter().map(|s| serde_json::json!({ "id": s.id(), "name": source_name(&config, s) })).collect();
    json(&serde_json::json!({ "sources": rows }))
}

/// What the event card says of a series (its file's `text`): how it repeats
/// ("Every week until Friday 26 June"), and its next pause and when it
/// resumes; "" each when it says nothing.
pub(crate) fn card_lines(text: &str) -> (String, String) {
    let zone = sioul_core::zone::current();
    let now = sioul_core::zone::now();
    let Some(edit) = sioul_core::agenda::edit_of_text(text, &zone) else { return (String::new(), String::new()) };
    let config = load_config();
    let names = |source: &Source| source_name(&config, source);
    (series::rule_words(&edit, now.date(), tr()), series::pause_line(text, &now, &names, tr()))
}

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
//! Each source is judged by itself (`series::keep`): this device acts on a
//! source only when it knows it, at a version at least as new as the one the
//! series notes (`X-SIOUL-PAUSED-VERSION`), and leaves the times of every
//! other source as they are. A settings file that does not read changes
//! nothing: no round runs.

use crate::backend::{QtThread, Shared, json, load_config, tr};
use sioul_core::series::{self, Source};
use sioul_core::vdir::{self, Kind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A round running.
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Asked again while a round ran: one more after it.
static AGAIN: AtomicBool = AtomicBool::new(false);

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
    // Settings that do not read change nothing (F27): no round on their account.
    let Some(config) = crate::backend::config_read() else { return };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sioul_core::zone::now().date().to_string().hash(&mut hasher);
    for off in sioul_core::vacations::DaysOff::typed_of(&config).periods() {
        (off.from.to_string(), off.until.to_string(), off.why.id()).hash(&mut hasher);
    }
    for (id, s) in &config.agenda.subscriptions {
        (id, s.holidays(), s.after_class()).hash(&mut hasher);
    }
    (&config.vacations.owner, &config.vacations.stamp).hash(&mut hasher);
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
    /// The round's mark, set back when it ends, a panic in it included (F5).
    struct Running;
    impl Drop for Running {
        fn drop(&mut self) {
            RUNNING.store(false, Ordering::Relaxed);
        }
    }
    if RUNNING.swap(true, Ordering::Relaxed) {
        AGAIN.store(true, Ordering::Relaxed);
        return 0;
    }
    let _running = Running;
    let mut changed = 0;
    loop {
        AGAIN.store(false, Ordering::Relaxed);
        changed += round(why);
        if !AGAIN.load(Ordering::Relaxed) {
            break;
        }
    }
    changed
}

/// The round itself: each series of a calendar this device may write that
/// says what it pauses during, settled source by source (`series::keep`);
/// a meeting with others is passed over, silently: its pauses are no longer
/// kept (the card says so), the times already left out stay.
fn round(why: &str) -> usize {
    // Settings that do not read change nothing (F27).
    let Some(config) = crate::backend::config_read() else { return 0 };
    let zone = sioul_core::zone::current();
    let today = sioul_core::zone::now().date();
    let days = series::Days::of(&config, &zone);
    let from = sioul_core::timeline::midnight(today, &zone);
    let to = from + series::AHEAD_DAYS * 86_400;
    let mut changed = 0;
    for collection in vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only && c.holds("VEVENT") && c.account != vdir::SUBSCRIBED) {
        for item in collection.items() {
            let Ok(text) = crate::files::read_to_string(&item) else { continue };
            if !text.contains("X-SIOUL-PAUSE") || sioul_core::agenda::with_others(&text) {
                continue;
            }
            let Some(new) = series::keep(&text, &days, from, to, &zone) else { continue };
            match vdir::write_item(&item, &new) {
                Ok(()) => changed += 1,
                // Said by its file's name alone, and why it was refused (a calendar you only read).
                Err(e) => eprintln!("series ({why}): {}: {e}", item.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            }
        }
    }
    // The versions held now, so that a series noting one of them is known to be older later.
    if let Err(e) = days.remember() {
        eprintln!("series ({why}): {e}");
    }
    changed
}

/// A source's name, as the form and the card say it: "my vacations",
/// "public holidays", a school calendar's own name.
pub(crate) fn source_name(config: &sioul_core::config::Config, source: &Source) -> String {
    match source {
        // Another person's settings' vacations (a calendar you share with them): said so, never as yours.
        Source::Vacations(_) if Source::mine(config).as_ref() != Some(source) => tr().text("series-pause-vacations-other", None),
        Source::Vacations(_) => tr().text("series-pause-vacations", None),
        Source::Public => tr().text("series-pause-public", None),
        Source::School(id) => config.agenda.subscriptions.get(id).map_or_else(|| tr().text("subscribe-holidays-school", None), |s| sioul_core::subscribed::display_name(s, &sioul_core::subscribed::Fetched::load(id).name)),
    }
}

/// What a series may pause during, for the event form (`pauseSources`): my
/// vacations, public holidays, each school-holidays calendar by its name, as JSON.
/// Your vacations are offered as yours (`vacations/<owner>`), or, before
/// your settings have an owner id, as `series::MY_VACATIONS`, which the
/// form's save makes yours (`mine_named`).
pub(crate) fn sources() -> String {
    let config = load_config();
    let mut rows = vec![serde_json::json!({ "id": Source::mine(&config).map_or_else(|| series::MY_VACATIONS.to_string(), |s| s.id()), "name": tr().text("series-pause-vacations", None) })];
    let mut list = vec![Source::Public];
    list.extend(config.agenda.subscriptions.iter().filter(|(_, s)| s.holidays() == Some(sioul_core::subscribed::Holidays::School)).map(|(id, _)| Source::School(id.clone())));
    rows.extend(list.iter().map(|s| serde_json::json!({ "id": s.id(), "name": source_name(&config, s) })));
    json(&serde_json::json!({ "sources": rows }))
}

/// The form's pauses with "my vacations" named yours (`series::MY_VACATIONS`
/// as `vacations/<owner>`), your settings' owner id made first when there is
/// none (`config::vacations_owner`). What went wrong, when it could not be made.
pub(crate) fn mine_named(pauses: &mut [String]) -> Result<(), String> {
    if !pauses.iter().any(|p| p.trim() == series::MY_VACATIONS) {
        return Ok(());
    }
    let owner = sioul_core::config::vacations_owner(&crate::backend::config_path())?;
    let mine = Source::Vacations(owner).id();
    for pause in pauses.iter_mut().filter(|p| p.trim() == series::MY_VACATIONS) {
        *pause = mine.clone();
    }
    Ok(())
}

/// What the event card says of a series (its file's `text`): how it repeats
/// ("Every week until Friday 26 June"), and its next pause and when it
/// resumes; "" each when it says nothing.
///
/// A series given guests after Sioul left times out of it keeps those
/// times, and Sioul no longer keeps its pauses (each change would go to
/// every guest): its card says so in one line, in place of the next pause (F37).
pub(crate) fn card_lines(text: &str) -> (String, String) {
    let zone = sioul_core::zone::current();
    let now = sioul_core::zone::now();
    let Some(edit) = sioul_core::agenda::edit_of_text(text, &zone) else { return (String::new(), String::new()) };
    // After a number of times, some in pauses: how many take place, in plain words (F36).
    let counted = series::count_words(text, &now, tr());
    let words = if counted.is_empty() { series::rule_words(&edit, now.date(), tr()) } else { counted };
    if sioul_core::agenda::with_others(text) {
        let noted = text.contains(&format!("{};", series::PAUSED)) || text.contains(series::PAUSE_DURING);
        return (words, if noted { tr().text("series-paused-guests", None) } else { String::new() });
    }
    let config = load_config();
    let names = |source: &Source| source_name(&config, source);
    (words, series::pause_line(text, &now, &names, tr()))
}

#[cfg(test)]
mod tests {
    /// The review's F37: a series given guests after it paused keeps the
    /// times left out, and its card says in one line that Sioul no longer
    /// keeps its pauses; one that never paused says nothing of them.
    #[test]
    fn a_series_with_guests_says_its_pauses_are_no_longer_kept() {
        sioul_core::agenda::own_addresses_here(Some(&["me@example.org"]));
        let series = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:f37@example.org\r\nDTSTAMP:20301001T000000Z\r\nDTSTART:20301002T160000Z\r\nDTEND:20301002T170000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Choir\r\nEXDATE:20301225T160000Z\r\nX-SIOUL-PAUSED;X-SOURCE=public:20301225T160000Z\r\nX-SIOUL-PAUSED-VERSION;X-SOURCE=public:0123456789abcdef\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:sam@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let (words, pause) = super::card_lines(series);
        assert!(!words.is_empty());
        assert_eq!(pause, crate::backend::tr().text("series-paused-guests", None));
        let never = series.replace("EXDATE:20301225T160000Z\r\nX-SIOUL-PAUSED;X-SOURCE=public:20301225T160000Z\r\nX-SIOUL-PAUSED-VERSION;X-SOURCE=public:0123456789abcdef\r\n", "");
        assert_eq!(super::card_lines(&never).1, "");
        sioul_core::agenda::own_addresses_here(None);
    }
}

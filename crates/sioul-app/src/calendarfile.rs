// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A calendar file (`.ics`) opened in Sioul (docs/client.md, "A calendar
//! file opened"): its events shown in one dialog, the agenda and the
//! categories chosen, then recorded through the agenda's own door
//! (`agenda::CalendarFile::record`). Sioul is one of the system's openers of
//! calendar files: on a computer `sioul-app <file>` (the desktop file's
//! `%u`), handed to the Sioul already open as a `mailto:` link is
//! (`outside::hand_over`); on a phone Android's VIEW (ShareActivity.java).
//! Either way the file's bytes, a megabyte and one byte at most, wait in
//! `<data>/sioul/handed/calendar/` until the window takes them, one at a
//! time. Nothing in the file is followed, and no answer goes to its organizer.

use crate::backend::qobject::Sioul;
use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use cxx_qt_lib::QString;
use sioul_core::agenda::{self, CalendarFile, FILE_MOST};
use sioul_core::vdir::{self, Collection, Kind};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// Where the files handed wait for the window, one each: `<data>/sioul/handed/calendar/`.
pub(crate) fn folder() -> PathBuf {
    sioul_core::config::data_dir().join("handed").join("calendar")
}

/// The file the dialog shows now: what was read of it.
static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);

struct Shown {
    file: Result<CalendarFile, agenda::FileProblem>,
}

/// A file's bytes, a megabyte and one byte at most (what is more is too
/// large: `agenda::calendar_file` says so); none read, nothing.
fn read_at_most(path: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Ok(file) = std::fs::File::open(path)
        && file.metadata().is_ok_and(|m| m.is_file())
    {
        let _ = file.take(FILE_MOST as u64 + 1).read_to_end(&mut bytes);
    }
    bytes
}

/// The program's arguments that are files (a calendar file opened from a
/// file manager: a path, or a `file://` address), made whole from where
/// this program started; `mailto:` addresses are `outside::addresses`'.
pub(crate) fn files() -> Vec<PathBuf> {
    let here = std::env::current_dir().unwrap_or_default();
    std::env::args_os()
        .skip(1)
        .filter_map(|a| a.into_string().ok())
        .filter(|a| !sioul_core::mailto::is_mailto(a) && !a.starts_with('-'))
        .filter_map(|a| {
            let path = if a.starts_with("file://") { crate::backend::local_path(&a) } else { PathBuf::from(&a) };
            let path = if path.is_absolute() { path } else { here.join(path) };
            path.is_file().then_some(path)
        })
        .collect()
}

/// Files handed (`sioul-app <file>`, another Sioul's): each read and kept for
/// the window, which shows them as it can.
pub(crate) fn hand(paths: &[PathBuf]) {
    for path in paths {
        if let Err(e) = keep(&read_at_most(path)) {
            eprintln!("Sioul: {}: {e}", path.display());
        }
    }
    crate::outside::poke();
}

/// Bytes of a file kept for the window, under a name of their own (beside, then renamed).
fn keep(bytes: &[u8]) -> std::io::Result<()> {
    let dir = folder();
    std::fs::create_dir_all(&dir)?;
    sioul_core::filelock::replace(&dir.join(format!("{}.ics", vdir::new_name())), bytes)
}

/// On the window's thread (`outside::take`): the next file waiting shown,
/// unless one is shown already. Its file taken out once read.
pub(crate) fn take(mut sioul: Pin<&mut Sioul>) {
    if SHOWN.lock().map_or(true, |shown| shown.is_some()) {
        return;
    }
    let mut waiting: Vec<PathBuf> = std::fs::read_dir(folder()).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ics")).collect();
    waiting.sort();
    let Some(next) = waiting.into_iter().next() else { return };
    let bytes = read_at_most(&next);
    let _ = std::fs::remove_file(&next);
    let file = agenda::calendar_file(&bytes, &sioul_core::zone::current());
    let shown = view(&file, &sioul.shared());
    if let Ok(mut kept) = SHOWN.lock() {
        *kept = Some(Shown { file });
    }
    sioul.as_mut().calendar_file_opened(QString::from(&shown));
}

#[derive(serde::Serialize)]
struct View {
    /// Why it does not open, in a sentence; "" when it does.
    problem: String,
    events: Vec<EventView>,
    /// The calendars that take events and can be written, the one new events go to first (`pim::calendars`).
    calendars: serde_json::Value,
    /// The categories tasks already have, the most used first.
    categories: Vec<String>,
}

#[derive(serde::Serialize)]
struct EventView {
    uid: String,
    title: String,
    /// "Wednesday 4 November, 15:00 – 15:15", on the time zone shown.
    when: String,
    /// On its own clock, when it is another: "15:00 – 15:15 Paris · 16:00 – 16:15 here"; else "".
    there: String,
    place: String,
    organizer: String,
    notes: String,
}

/// What the dialog shows of a file read.
fn view(file: &Result<CalendarFile, agenda::FileProblem>, shared: &Shared) -> String {
    let zone = sioul_core::zone::current();
    let (problem, events) = match file {
        Ok(file) => (String::new(), file.events.iter().map(|e| event_view(e, &zone)).collect()),
        Err(problem) => (tr().text(problem.message_id(), None), Vec::new()),
    };
    let calendars = serde_json::from_str(&crate::pim::calendars()).unwrap_or_default();
    // The tasks' categories as the last read knows them: never a read of the stores on the window's thread.
    let choices: serde_json::Value = serde_json::from_str(&crate::work::task_choices(shared)).unwrap_or_default();
    let categories: Vec<String> = serde_json::from_value(choices["categories"].clone()).unwrap_or_default();
    json(&View { problem, events, calendars, categories })
}

fn event_view(event: &agenda::FileEvent, zone: &jiff::tz::TimeZone) -> EventView {
    let at = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()));
    let when = match (at(event.at), at(event.until)) {
        (Some(from), _) if event.all_day => tr().day(from.date()),
        (Some(from), Some(until)) if until.date() == from.date() => format!("{}, {} – {}", tr().day(from.date()), from.strftime("%H:%M"), until.strftime("%H:%M")),
        (Some(from), Some(until)) => format!("{} – {}", tr().date(&from, false), tr().date(&until, false)),
        (Some(from), None) => tr().date(&from, false),
        _ => String::new(),
    };
    let there = if event.clock.is_empty() || event.all_day { String::new() } else { agenda::clock_line(&event.start, &event.end, &event.clock, zone, tr()) };
    let organizer = if event.organizer.is_empty() { String::new() } else { say("calendar-file-by", &[("organizer", event.organizer.clone())]) };
    EventView { uid: event.uid.clone(), title: event.title.clone(), when, there, place: event.location.clone(), organizer, notes: event.notes.clone() }
}

/// The calendar ("account/id") among those that can take an event.
fn calendar_of(id: &str) -> Option<Collection> {
    vdir::collections(Kind::Calendars).into_iter().find(|c| !c.read_only && c.holds("VEVENT") && format!("{}/{}", c.account, c.id) == id)
}

/// The UIDs of the file shown that `calendar` holds already, as a JSON list.
pub(crate) fn known(calendar: &str) -> String {
    let Some(calendar) = calendar_of(calendar) else { return "[]".into() };
    let uids: Vec<String> = SHOWN.lock().ok().and_then(|shown| shown.as_ref().and_then(|s| s.file.as_ref().ok().map(|f| f.known_in(&calendar).into_keys().collect()))).unwrap_or_default();
    json(&uids)
}

/// The file shown written into `calendar` with `categories` (a JSON list):
/// "" when done (said on the status line, the next file shown), else why not.
pub(crate) fn add(qt: &QtThread, shared: &Arc<Shared>, calendar: &str, categories: &str) -> String {
    let Some(collection) = calendar_of(calendar) else { return tr().text("dav-no-calendar", None) };
    let categories: Vec<String> = serde_json::from_str(categories).unwrap_or_default();
    let written = {
        let Ok(shown) = SHOWN.lock() else { return tr().text("calendar-file-gone", None) };
        let Some(Ok(file)) = shown.as_ref().map(|s| &s.file) else { return tr().text("calendar-file-gone", None) };
        match file.record(&collection, &categories) {
            Ok(written) => written,
            Err(e) => return e,
        }
    };
    if let Ok(mut shown) = SHOWN.lock() {
        *shown = None;
    }
    let name = collection.label(&load_config(), tr());
    let updated = written.iter().filter(|(_, there)| *there).count();
    let line = if updated == written.len() {
        say("calendar-file-updated", &[("count", written.len().to_string()), ("calendar", name)])
    } else {
        say("calendar-file-added", &[("count", written.len().to_string()), ("calendar", name)])
    };
    crate::backend::tell(qt, shared, line);
    if let Some(account) = written.first().and_then(|(path, _)| crate::pim::account_of(path)) {
        crate::pim::nudge(shared, &account);
    }
    crate::work::show_work(qt, shared);
    crate::pim::show_pim(qt, shared);
    next(qt);
    String::new()
}

/// The file shown set aside, nothing written; the next one waiting shown.
pub(crate) fn close(qt: &QtThread) {
    if let Ok(mut shown) = SHOWN.lock() {
        *shown = None;
    }
    next(qt);
}

/// The next file waiting, shown once the window's thread is free.
fn next(qt: &QtThread) {
    let _ = qt.queue(take);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file handed is kept for the window whole, a megabyte and one byte
    /// at most: what is larger reads as too large, never cut into a calendar.
    #[test]
    fn a_file_handed_is_read_a_megabyte_at_most() {
        sioul_core::config::stand_ins_only();
        let dir = std::env::temp_dir().join(format!("sioul-calendar-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let large = dir.join("large.ics");
        std::fs::write(&large, vec![b'x'; FILE_MOST + 10]).unwrap();
        assert_eq!(read_at_most(&large).len(), FILE_MOST + 1);
        assert_eq!(agenda::calendar_file(&read_at_most(&large), &jiff::tz::TimeZone::UTC).err(), Some(agenda::FileProblem::TooLarge));
        assert!(read_at_most(&dir).is_empty(), "a folder is no file");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

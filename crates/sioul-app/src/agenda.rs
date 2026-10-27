// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The agenda's job: the days the agenda page asks for, made from the one
//! model of time (`sioul_core::timeline`), and published as its `agenda`
//! property (version 2), with the overlaps of the next two weeks.
//!
//! Within the plan's horizon the events are the plan's one read of the
//! calendars (`work::PlanShown`), when nothing was written since it was read;
//! else, and for the days before today or beyond the horizon, the job reads
//! the calendars itself, as the page's own read did. Today is never laid
//! here: the Tasks job lays it, and writes the slots of time for you, alone.
//! The job writes nothing. It computes nothing until the page asked once, nor
//! while Sioul is away on a phone (`backend::coalesced`).
//!
//! Version 2 draws what version 1 drew, from the model: the events and the
//! time blocks, each with what may change it (`timeline::Can`). The other
//! kinds of the model (steps, meals, doses…) come with the views that draw
//! them.

use crate::backend::{QtThread, Shared, json, load_config, tr};
use crate::{mail, pim, work};
use cxx_qt_lib::QString;
use jiff::Zoned;
use jiff::tz::TimeZone;
use serde::Serialize;
use sioul_core::agenda::{self, Occurrence};
use sioul_core::timeline::{self, Placed};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// The days the agenda shows, version 2 of its JSON.
#[derive(Serialize)]
struct Shown {
    version: u32,
    /// The first day shown, "2026-10-03", to move week by week or month by month.
    from: String,
    /// "Nothing planned in the next two weeks.", when so.
    sentence: String,
    /// Whether an event can be added: a calendar that takes it exists.
    can_add: bool,
    days: Vec<DayShown>,
}

/// One day, as `timeline::days` places it, each thing with its details.
#[derive(Serialize)]
struct DayShown {
    date: String,
    title: String,
    today: bool,
    past: bool,
    start: i64,
    hours_from: i32,
    hours_to: i32,
    head: Vec<Row>,
    timed: Vec<Row>,
}

/// A thing of the day: the item as placed, and an event's details for its
/// panel (until its card reads them itself).
#[derive(Serialize)]
struct Row {
    #[serde(flatten)]
    placed: Placed,
    detail: Option<Detail>,
}

/// What the event's panel says of it, as the occurrence read has it.
#[derive(Serialize)]
struct Detail {
    key: String,
    uid: String,
    /// This occurrence's start and end, Unix seconds.
    start: i64,
    end: i64,
    location: String,
    notes: String,
    calendar: String,
    color: Option<String>,
    recurring: bool,
    cancelled: bool,
    tentative: bool,
    read_only: bool,
    with_others: bool,
    organizer: String,
    attendees: Vec<agenda::Attendee>,
    all_day: bool,
    /// The task it is the time block of, by UID, known here; "" for an event of its own.
    task: String,
    list: String,
    /// Minutes it keeps before and after it, its own.
    before: u32,
    after: u32,
}

impl Detail {
    fn of(o: &Occurrence) -> Detail {
        Detail {
            key: o.key.clone(),
            uid: o.uid.clone(),
            start: o.start,
            end: o.end,
            location: o.location.clone(),
            notes: o.notes.clone(),
            calendar: o.calendar.clone(),
            color: o.color.clone(),
            recurring: o.recurring,
            cancelled: o.cancelled,
            tentative: o.tentative,
            read_only: o.read_only,
            with_others: o.with_others,
            organizer: o.organizer.clone(),
            attendees: o.attendees.clone(),
            all_day: o.all_day,
            task: o.task.clone(),
            list: o.list.clone(),
            before: o.margins.before,
            after: o.margins.after,
        }
    }
}

/// Midnight of `date` where you are, Unix seconds.
fn midnight(date: jiff::civil::Date, zone: &TimeZone) -> i64 {
    timeline::midnight(date, zone)
}

/// The calendars' occurrences from `from` to `to` (Unix seconds): the plan's
/// one read where it reaches and nothing was written since it was read
/// (`work::PlanShown::current`), read here elsewhere; each occurrence once, in
/// the order `agenda::occurrences` gives them.
fn read_between(from: i64, to: i64, zone: &TimeZone) -> Vec<Occurrence> {
    let today = Zoned::now().date();
    let Some(plan) = work::plan_shown().filter(|s| s.made_on == today && s.current()) else {
        return agenda::occurrences(from, to);
    };
    let (reach_from, reach_to) = (midnight(today, zone), midnight(today, zone) + sioul_core::plan::HORIZON_DAYS * 86_400);
    let mut all: Vec<Occurrence> = plan.read().iter().filter(|o| o.end > from && o.start < to).cloned().collect();
    if from < reach_from {
        all.extend(agenda::occurrences(from, reach_from.min(to)));
    }
    if to > reach_to {
        all.extend(agenda::occurrences(reach_to.max(from), to));
    }
    // One read's edge and the other's: an event over both, once.
    let mut seen = std::collections::BTreeSet::new();
    all.retain(|o| seen.insert((o.key.clone(), o.start)));
    all.sort_by(|a, b| (a.start, !a.all_day, &a.summary).cmp(&(b.start, !b.all_day, &b.summary)));
    all
}

/// What the window's agenda shows from `from` to `to` (Unix seconds), the
/// phone's alike: the calendars' events (`read_between`), those whose
/// deletion waits ("Undo") aside, a task's time block marked as the task it
/// is, with its list (`agenda::mark_tasks`).
pub(crate) fn occurrences_between(from: i64, to: i64, removed: &std::collections::BTreeSet<std::path::PathBuf>, skipped: &std::collections::BTreeSet<(std::path::PathBuf, i64)>, tasks: &[sioul_core::tasks::Task]) -> Vec<Occurrence> {
    let mut items = pim::without_waiting(read_between(from, to, &TimeZone::system()), removed, skipped);
    agenda::mark_tasks(&mut items, tasks);
    items
}

/// The days the page asks for (`pim::PimState`: from, how many, what comes
/// only), as JSON, and the overlaps of the next two weeks.
fn made(shared: &Shared) -> (String, String) {
    let state = shared.pim.lock().map(|s| s.clone()).unwrap_or_default();
    let zone = TimeZone::system();
    let now = Zoned::now();
    let today = now.date();
    let from = midnight(state.from, &zone);
    let to = from + state.days * 86_400 + 3_600;
    let (removed, skipped) = mail::hidden_pim(shared);
    let stamp = now.timestamp().as_second();
    // A task's time block shows as the task it is, with its list (`agenda::mark_tasks`).
    let tasks = work::plan_shown().map_or_else(|| work::loaded(shared).tasks.clone(), |s| s.loaded().tasks.clone());
    let occurrences: Vec<Occurrence> = occurrences_between(from, to, &removed, &skipped, &tasks)
        .into_iter()
        // What comes: an event over is the past, shown only when you go back to it.
        .filter(|o| !state.upcoming || o.end.max(o.start) > stamp)
        .collect();
    // The window's own items reach no agent (the MCP makes its own, through
    // `ForAgents`): errands are told apart where they are named (the agenda's
    // next views), not here.
    let mut items = agenda::items(&occurrences, &zone, &|_| false);
    // An event of no title, said so.
    for item in items.iter_mut().filter(|i| i.title.trim().is_empty()) {
        item.title = tr().text("agenda-untitled", None);
    }
    let mut days = timeline::days(&items, state.from, state.days, today, &zone, tr());
    timeline::set_hours(&mut days, &load_config().week_hours(), &zone);
    let detail = |placed: &Placed| match &placed.item.source {
        timeline::Source::Calendar { key, start, .. } => occurrences.iter().find(|o| &o.key == key && o.start == *start).map(Detail::of),
        _ => None,
    };
    let row = |placed: Placed| Row { detail: detail(&placed), placed };
    let empty = days.iter().all(|d| d.head.is_empty() && d.timed.is_empty());
    let can_add = agenda::default_calendar().is_some();
    let sentence = match (empty, can_add || !sioul_core::vdir::collections(sioul_core::vdir::Kind::Calendars).is_empty()) {
        (true, false) => tr().text("agenda-no-calendar", None),
        (true, true) => tr().text("agenda-nothing", None),
        _ => String::new(),
    };
    let shown = Shown {
        version: 2,
        from: state.from.to_string(),
        sentence,
        can_add,
        days: days
            .into_iter()
            .map(|d| DayShown { date: d.date, title: d.title, today: d.today, past: d.past, start: d.start, hours_from: d.hours_from, hours_to: d.hours_to, head: d.head.into_iter().map(row).collect(), timed: d.timed.into_iter().map(row).collect() })
            .collect(),
    };
    // Two events at once in the next two weeks, from the same read.
    let reach = midnight(today, &zone);
    let near: Vec<Occurrence> = pim::without_waiting(read_between(reach, reach + 14 * 86_400, &zone), &removed, &skipped);
    (json(&shown), pim::overlaps_of(&near))
}

/// Makes the agenda's days on a thread and shows them: once the page asked
/// for them (`asked`), whenever the plan or a calendar changes.
pub(crate) fn show_agenda(qt: &QtThread, shared: &Arc<Shared>) {
    if !shared.agenda_asked.load(Ordering::Relaxed) {
        return;
    }
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.agenda_job, move |shared| {
        let generation = shared.agenda_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let started = std::time::Instant::now();
        let (agenda, overlaps) = made(shared);
        if crate::timed() {
            eprintln!("sioul-perf: agenda job {} ms, {} bytes", started.elapsed().as_millis(), agenda.len());
        }
        let _ = qt.queue(move |mut sioul| {
            // Only the newest (see `backend::show`).
            if sioul.shared().agenda_generation.load(Ordering::Relaxed) != generation || sioul.shared().agenda_shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
                return;
            }
            sioul.as_mut().set_agenda(QString::from(&agenda));
            sioul.as_mut().set_overlaps(QString::from(&overlaps));
        });
    });
}

/// The days the agenda page shows from now on (`show_days`): it has asked.
pub(crate) fn asked(qt: &QtThread, shared: &Arc<Shared>) {
    shared.agenda_asked.store(true, Ordering::Relaxed);
    show_agenda(qt, shared);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::vdir::{self, Kind};

    /// Every file under `dir`, with its length and time: what a run wrote, compared.
    fn files(dir: &std::path::Path) -> Vec<(std::path::PathBuf, u64, Option<std::time::SystemTime>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(at) = stack.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                let path = entry.path();
                match entry.metadata() {
                    Ok(meta) if meta.is_dir() => stack.push(path),
                    Ok(meta) => out.push((path, meta.len(), meta.modified().ok())),
                    Err(_) => {}
                }
            }
        }
        out.sort();
        out
    }

    /// The agenda's job makes its days while another thread holds every
    /// health lock (it takes none), writes no file, and reads the calendars
    /// itself once something was written after the plan it would read.
    #[test]
    fn the_agenda_job_writes_nothing_and_waits_for_no_health_lock() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Agenda job", None, &["VEVENT"]).unwrap();
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let tomorrow = now.date().tomorrow().unwrap();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let event = |uid: &str, hour: i8| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{uid}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(hour), utc(hour + 1));
        std::fs::write(calendar.dir.join("dentist.ics"), event("agenda-job-dentist", 9)).unwrap();
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            state.from = now.date();
            state.days = 3;
            state.upcoming = false;
        }
        // What a layout or a reader of health could write: today's slots of time for you (the Tasks
        // job's alone), the health file's last-read and aside copies; and this test's calendar.
        // Other tests write elsewhere in the same folders meanwhile.
        let health = sioul_core::health::Health::default_path();
        let watched = [sioul_core::attention::Slots::default_path(), sioul_core::health::Health::last_read_path(&health), health.with_extension("toml.unreadable")];
        let state = |path: &std::path::Path| std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()));
        let seen = || (watched.iter().map(|p| state(p)).collect::<Vec<_>>(), files(&calendar.dir));
        let before = seen();
        // Every health lock held on another thread, for longer than a run takes.
        let (held, release) = std::sync::mpsc::channel::<()>();
        let (go, ready) = std::sync::mpsc::channel::<()>();
        let holder = std::thread::spawn(move || {
            let _locks = crate::health::every_lock_held();
            let _ = go.send(());
            let _ = release.recv_timeout(std::time::Duration::from_secs(20));
        });
        ready.recv().unwrap();
        let started = std::time::Instant::now();
        let (shown, _) = made(&shared);
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "it waited for a health lock");
        let _ = held.send(());
        holder.join().unwrap();
        assert!(shown.contains("agenda-job-dentist"), "{shown}");
        let parsed: serde_json::Value = serde_json::from_str(&shown).unwrap();
        assert_eq!(parsed["version"], 2);
        assert_eq!(before, seen(), "nothing written");
        // A write after the plan it reads: the calendars read again, the new event there at once.
        let written = sioul_core::timeline::written();
        vdir::write_item(&calendar.dir.join("visit.ics"), &event("agenda-job-visit", 14)).unwrap();
        assert!(sioul_core::timeline::written() > written, "a calendar write is counted");
        let (shown, _) = made(&shared);
        assert!(shown.contains("agenda-job-visit"), "{shown}");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
    /// A heavy week (fifteen events a day, each with a few lines of notes)
    /// stays within the phone's budget for the agenda's JSON: under 150 KB
    /// (the unified agenda, §4.9).
    #[test]
    fn a_heavy_week_stays_within_its_budget() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Heavy week", None, &["VEVENT"]).unwrap();
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let monday = now.date();
        for day in 0..7 {
            let date = monday.checked_add(jiff::Span::new().days(day)).unwrap();
            for n in 0..15 {
                let utc = |minute: i64| date.at(7, 0, 0, 0).to_zoned(zone.clone()).unwrap().checked_add(jiff::Span::new().minutes(minute)).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
                let text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:heavy-{day}-{n}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Appointment number {n} of a long day\r\nLOCATION:12 rue de l'Exemple, 75000 Paris\r\nDESCRIPTION:Bring the papers.\\nAsk about the next visit.\\nThe door code is on the letter.\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(n * 50), utc(n * 50 + 45));
                std::fs::write(calendar.dir.join(format!("heavy-{day}-{n}.ics")), text).unwrap();
            }
        }
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            state.from = monday;
            state.days = 7;
            state.upcoming = false;
        }
        let (shown, _) = made(&shared);
        let ours = shown.matches("heavy-").count();
        assert!(ours >= 105, "{ours}");
        eprintln!("sioul-perf: a heavy week's agenda JSON: {} bytes", shown.len());
        assert!(shown.len() < 150_000, "{} bytes", shown.len());
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
}


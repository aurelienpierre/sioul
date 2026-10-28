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

/// What the event's panel says of it, beyond its item (its time, its file,
/// its calendar and colour are the item's: `timeline::Source`, `Time`).
#[derive(Serialize)]
struct Detail {
    /// This occurrence's end, Unix seconds (an all-day one's included).
    end: i64,
    location: String,
    notes: String,
    cancelled: bool,
    tentative: bool,
    read_only: bool,
    with_others: bool,
    organizer: String,
    attendees: Vec<agenda::Attendee>,
    /// The task it is the time block of, by UID, known here; "" for an event of its own.
    task: String,
    list: String,
}

impl Detail {
    fn of(o: &Occurrence) -> Detail {
        Detail {
            end: o.end,
            location: o.location.clone(),
            notes: o.notes.clone(),
            cancelled: o.cancelled,
            tentative: o.tentative,
            read_only: o.read_only,
            with_others: o.with_others,
            organizer: o.organizer.clone(),
            attendees: o.attendees.clone(),
            task: o.task.clone(),
            list: o.list.clone(),
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

/// The tasks a time block is marked with: the plan's, else those last read,
/// else the tasks alone, read now: never everything (`Loaded::read`), which
/// may write the mail's index (review of 1880bce, L9).
fn tasks_known(shared: &Shared) -> Vec<sioul_core::tasks::Task> {
    if let Some(plan) = work::plan_shown() {
        return plan.loaded().tasks.clone();
    }
    if let Some(loaded) = shared.loaded.lock().ok().and_then(|l| l.clone()) {
        return loaded.tasks.clone();
    }
    sioul_core::tasks::all(&TimeZone::system())
}

/// The overlaps of the next two weeks (`pim::overlaps_of`), what waits for
/// Undo left out: the Porch's, made at every run whether the agenda page has
/// asked or not (review of 1880bce, M2).
fn overlaps_now(shared: &Shared) -> String {
    let zone = TimeZone::system();
    let reach = midnight(Zoned::now().date(), &zone);
    let (removed, skipped) = mail::hidden_pim(shared);
    pim::overlaps_of(&pim::without_waiting(read_between(reach, reach + 14 * 86_400, &zone), &removed, &skipped))
}

/// The days the page asks for (`pim::PimState`: from, how many, what comes
/// only), as JSON, and how long their read of the calendars took.
fn made(shared: &Shared) -> (String, std::time::Duration) {
    let state = shared.pim.lock().map(|s| s.clone()).unwrap_or_default();
    let zone = TimeZone::system();
    let now = Zoned::now();
    let today = now.date();
    let from = midnight(state.from, &zone);
    let to = from + state.days * 86_400 + 3_600;
    let (removed, skipped) = mail::hidden_pim(shared);
    let stamp = now.timestamp().as_second();
    // A task's time block shows as the task it is, with its list (`agenda::mark_tasks`).
    let tasks = tasks_known(shared);
    let started = std::time::Instant::now();
    let occurrences: Vec<Occurrence> = occurrences_between(from, to, &removed, &skipped, &tasks)
        .into_iter()
        // What comes: an event over is the past, shown only when you go back to it.
        .filter(|o| !state.upcoming || o.end.max(o.start) > stamp)
        .collect();
    let read = started.elapsed();
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
    (json(&shown), read)
}

/// Whether a run makes the days: once the page asked for them (`asked`), or
/// once at start, from the first plan (the Tasks job's first run), so that the
/// page's first opening never shows an empty week, which would read as
/// "nothing planned" (review of 1880bce, L18).
fn days_wanted(shared: &Shared) -> bool {
    shared.agenda_asked.load(Ordering::Relaxed) || (work::plan_shown().is_some() && !shared.agenda_prefilled.swap(true, Ordering::Relaxed))
}

/// Makes the overlaps of the next two weeks at every run, for the Porch, and
/// the agenda's days once they are wanted (`days_wanted`), on a thread, then
/// shows them: whenever the plan or a calendar changes.
pub(crate) fn show_agenda(qt: &QtThread, shared: &Arc<Shared>) {
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.agenda_job, move |shared| {
        let generation = shared.agenda_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let started = std::time::Instant::now();
        let overlaps = overlaps_now(shared);
        let days = days_wanted(shared).then(|| made(shared));
        if crate::timed() {
            let (bytes, read) = days.as_ref().map_or((0, 0), |(agenda, read)| (agenda.len(), read.as_millis()));
            eprintln!("sioul-perf: agenda job {} ms, its calendars read {read} ms, {bytes} bytes", started.elapsed().as_millis());
        }
        let _ = qt.queue(move |mut sioul| {
            // Only the newest (see `backend::show`).
            if sioul.shared().agenda_generation.load(Ordering::Relaxed) != generation || sioul.shared().agenda_shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
                return;
            }
            if let Some((agenda, _)) = &days {
                sioul.as_mut().set_agenda(QString::from(agenda));
            }
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
        // Other tests write elsewhere in the same folders meanwhile; the health tests, which
        // write and read these, wait (their own read once came in as this job's write).
        let _health_tests = crate::health::health_files_held();
        let health = sioul_core::health::Health::default_path();
        let watched = [sioul_core::attention::Slots::default_path(), sioul_core::health::Health::last_read_path(&health), health.with_extension("toml.unreadable")];
        let state = |path: &std::path::Path| std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()));
        let seen = || (watched.iter().map(|p| state(p)).collect::<Vec<_>>(), files(&calendar.dir));
        let before = seen();
        // With no plan yet, the job reads the tasks alone, never everything, which writes the
        // mail's index (review of 1880bce, L9): counted on this thread, where the job runs.
        crate::work::LOADS.with(|n| n.set(0));
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
        let _ = overlaps_now(&shared);
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "it waited for a health lock");
        let _ = held.send(());
        holder.join().unwrap();
        assert!(shown.contains("agenda-job-dentist"), "{shown}");
        let parsed: serde_json::Value = serde_json::from_str(&shown).unwrap();
        assert_eq!(parsed["version"], 2);
        assert_eq!(before, seen(), "nothing written");
        assert_eq!(crate::work::LOADS.with(std::cell::Cell::get), 0, "the tasks alone read");
        // A write after the plan it reads: the calendars read again, the new event there at once.
        let written = sioul_core::timeline::written();
        vdir::write_item(&calendar.dir.join("visit.ics"), &event("agenda-job-visit", 14)).unwrap();
        assert!(sioul_core::timeline::written() > written, "a calendar write is counted");
        let (shown, _) = made(&shared);
        assert!(shown.contains("agenda-job-visit"), "{shown}");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
    /// The Porch's overlaps are made at every run, the agenda page never
    /// opened: two of today's events at once are said from the start (review of
    /// 1880bce, M2); the days wait for the page's first ask, or the first plan.
    #[test]
    fn the_porch_s_overlaps_never_wait_for_the_agenda_page() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Overlaps", None, &["VEVENT"]).unwrap();
        let now = Zoned::now();
        let utc = |minutes: i64| now.checked_add(jiff::Span::new().minutes(minutes)).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M00Z").to_string();
        for (uid, from, to) in [("overlap-dentist", 120, 180), ("overlap-bank", 150, 210)] {
            std::fs::write(calendar.dir.join(format!("{uid}.ics")), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{uid}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(from), utc(to))).unwrap();
        }
        let shared = Arc::new(Shared::default());
        assert!(!shared.agenda_asked.load(Ordering::Relaxed));
        let overlaps = overlaps_now(&shared);
        assert!(overlaps.contains("overlap-dentist") && overlaps.contains("overlap-bank"), "{overlaps}");
        assert!(!days_wanted(&shared), "no days made: the page never asked, no plan yet");
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
        let ours = shown.matches("\"heavy-").count();
        assert!(ours >= 105, "{ours}");
        eprintln!("sioul-perf: a heavy week's agenda JSON: {} bytes", shown.len());
        assert!(shown.len() < 150_000, "{} bytes", shown.len());
        // The Month (six weeks from this week's Monday) and the 15-day list, said for the record.
        for (name, days) in [("month", 42), ("list", 15)] {
            if let Ok(mut state) = shared.pim.lock() {
                state.from = monday;
                state.days = days;
            }
            eprintln!("sioul-perf: a heavy {name}'s agenda JSON ({days} days, the heavy week in it): {} bytes", made(&shared).0.len());
        }
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// A time block dragged is one write, and the agenda shows it at its new
    /// time at once and after the plan is made again at that write, never
    /// back at the old one (unified agenda, plan step 7: a drag published
    /// once, at its generation; review of 1880bce, L22).
    #[test]
    fn a_drag_is_shown_once_at_its_generation() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Drag test", None, &["VEVENT", "VTODO"]).unwrap();
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let tomorrow = now.date().tomorrow().unwrap();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let edit = sioul_core::tasks::TaskEdit { title: "Drag test task".into(), estimate: 30, ..sioul_core::tasks::TaskEdit::default() };
        vdir::write_item(&calendar.dir.join("drag-task.ics"), &sioul_core::tasks::new_task(&edit, "drag-task", &zone, &now).unwrap()).unwrap();
        let block = calendar.dir.join("drag-block.ics");
        let text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:drag-block\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Drag test task\r\n{}:drag-task\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(9), utc(10), sioul_core::blocks::TASK);
        vdir::write_item(&block, &text).unwrap();
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            state.from = now.date();
            state.days = 3;
            state.upcoming = false;
        }
        work::publish_now(&shared);
        // The block's minute of the day as the agenda shows it.
        let minute = |shared: &Shared| -> Option<i64> {
            let shown: serde_json::Value = serde_json::from_str(&made(shared).0).unwrap();
            let rows: Vec<serde_json::Value> = shown["days"].as_array()?.iter().filter(|d| d["date"] == tomorrow.to_string()).flat_map(|d| d["timed"].as_array().cloned().unwrap_or_default()).collect();
            rows.iter().find(|r| r["item"]["source"].to_string().contains("\"drag-block\"")).and_then(|r| r["from_minute"].as_i64())
        };
        assert_eq!(minute(&shared), Some(9 * 60));
        // The drag: the block written at 11:00 through the door a drag takes (`blocks::moved`, then `write`).
        let generation = sioul_core::timeline::written();
        let at = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let moved = sioul_core::blocks::moved(&text, at(11), at(12), &zone).unwrap();
        vdir::write_item(&block, &moved).unwrap();
        // Other tests write beside this one (the count is the process's): counted at least once.
        assert!(sioul_core::timeline::written() > generation, "the drag is a write, counted");
        assert!(work::plan_shown().is_some_and(|p| !p.current()), "the plan made before it is older");
        assert_eq!(minute(&shared), Some(11 * 60), "shown at its new time at once");
        // The plan made again after it, current (made again while another test wrote meanwhile):
        // still at its new time, and the block as the drag wrote it.
        for _ in 0..5 {
            work::publish_now(&shared);
            if work::plan_shown().is_some_and(|p| p.current()) {
                break;
            }
        }
        assert!(work::plan_shown().is_some_and(|p| p.stamp.written > generation), "made at the drag's generation or after");
        assert_eq!(minute(&shared), Some(11 * 60), "never back at the old one");
        assert_eq!(std::fs::read_to_string(&block).unwrap(), moved, "shown, never written again");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
}

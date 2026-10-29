// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The agenda's job: the days the agenda page asks for, made from the one
//! model of time (`sioul_core::timeline`), and published as its `agenda`
//! property (version 3), with the overlaps of the next two weeks.
//!
//! Within the plan's horizon the events are the plan's one read of the
//! calendars (`work::PlanShown`), when nothing was written since it was read;
//! else, and for the days before today or beyond the horizon, the job reads
//! the calendars itself, as the page's own read did. Today is never laid
//! here: the Tasks job lays it, and writes the slots of time for you, alone
//! (`PlanShown::today`). The job writes nothing, and reads health only as a
//! pure read (`Needs::read`, `Health::read`): never a record, never what the
//! other devices said. It computes nothing until the page asked once, nor
//! while Sioul is away on a phone (`backend::coalesced`).
//!
//! The Day and the Week (version 3) draw every kind: the events and the time
//! blocks, the plan's steps (today's as the Tasks job laid them, a day to
//! come's laid from the plan, within its reach), the tasks done, the time
//! kept for you, meals, naps and the night, the doses by key and time (today's
//! states are the Health job's rows, `doseMarks`), the days off; each with
//! what may change it (`timeline::Can`), placed and dressed in Rust
//! (`timeline::dress`), with the hours shown and today's head line given
//! ahead. The Month and What comes draw the events alone, as before.

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

/// The days the agenda shows, version 3 of its JSON.
#[derive(Serialize)]
struct Shown {
    version: u32,
    /// The first day shown, "2026-10-03", to move week by week or month by month.
    from: String,
    /// "Nothing planned in the next two weeks.", when so.
    sentence: String,
    /// Whether an event can be added: a calendar that takes it exists.
    can_add: bool,
    /// The hours the Day and the Week show, the same every day (`timeline::dress`).
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<timeline::Frame>,
    /// Today's head line, given ahead (`timeline::head_line`), when today is shown.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    head: Vec<timeline::HeadFrame>,
    /// The medicines could not be read here: said where the doses would be.
    #[serde(skip_serializing_if = "String::is_empty")]
    doses_note: String,
    days: Vec<DayShown>,
}

/// One day, as `timeline::days` places it and `timeline::dress` dresses it,
/// each thing with its details.
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    hours: Vec<sioul_core::dayview::Stretch>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bands: Vec<Placed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    hidden: Vec<Placed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    doses: Vec<Placed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    edges: Vec<Placed>,
    planned: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    plan: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    more: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    said: Vec<String>,
    /// The Month's lines (`timeline::month_lines`): its dates, then its
    /// appointments and pinned times; the Month's days alone.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lines: Vec<Row>,
    /// A day off: its Month cell shaded.
    #[serde(skip_serializing_if = "is_false")]
    off: bool,
    /// What comes' rows, its doses by the minute and the plan's line
    /// (`timeline::comes`); What comes' days alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    comes: Option<ComesShown>,
}

/// A day of What comes: its rows with their details, its doses, the plan's line.
#[derive(Serialize)]
struct ComesShown {
    rows: Vec<Row>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    doses: Vec<timeline::DoseLine>,
    #[serde(skip_serializing_if = "String::is_empty")]
    plan: String,
}

/// What the days are shown as: the Day and the Week (one timeline of every
/// kind), the Month (dates and appointments), What comes (today in full,
/// other days what differs); any other count of days, their events alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Drawn,
    Month,
    Comes,
    Events,
}

impl Mode {
    /// As the page asks for its days (`showDays`): one or seven, What comes'
    /// fifteen days, the Month's six weeks.
    fn of(days: i64) -> Mode {
        match days {
            1 | 7 => Mode::Drawn,
            15 => Mode::Comes,
            42 => Mode::Month,
            _ => Mode::Events,
        }
    }
}

/// A thing of the day: the item as placed, and an event's details for its
/// panel (until its card reads them itself).
#[derive(Serialize)]
struct Row {
    #[serde(flatten)]
    placed: Placed,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<Detail>,
}

/// What the event's panel says of it, beyond its item (its time, its file,
/// its calendar and colour are the item's: `timeline::Source`, `Time`); its
/// notes, organizer and guests come when it opens (`event_details`), so that
/// a week of long invitations stays light on a phone. What is false or empty
/// is left out.
#[derive(Serialize)]
struct Detail {
    /// This occurrence's end, Unix seconds (an all-day one's included).
    end: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    location: String,
    #[serde(skip_serializing_if = "is_false")]
    cancelled: bool,
    #[serde(skip_serializing_if = "is_false")]
    tentative: bool,
    #[serde(skip_serializing_if = "is_false")]
    read_only: bool,
    #[serde(skip_serializing_if = "is_false")]
    with_others: bool,
    /// The task it is the time block of, by UID, known here; "" for an event of its own.
    #[serde(skip_serializing_if = "String::is_empty")]
    task: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    list: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Detail {
    fn of(o: &Occurrence) -> Detail {
        Detail { end: o.end, location: o.location.clone(), cancelled: o.cancelled, tentative: o.tentative, read_only: o.read_only, with_others: o.with_others, task: o.task.clone(), list: o.list.clone() }
    }
}

/// One time of an event as its card shows it (`EventCard.qml`), read from its
/// file alone, as it is now, whatever page opened it (the unified agenda,
/// part 3): its time and the time kept around it, its place, its calendar,
/// whether it repeats, its notes, organizer and guests, the task it is the
/// block of, what may change it and why not. `start` names the time of a
/// repeating one; 0 asks for the next time it comes, else its last. As JSON;
/// "{}" when it is gone, or while its deletion (or that time's) waits for Undo.
pub(crate) fn event_card(shared: &Shared, key: &str, start: i64) -> String {
    event_card_as(shared, key, start, &|| load_config().every_account().filter_map(|a| a.address.clone()).collect())
}

/// `event_card`, with `own` giving your addresses (read only when someone declined).
fn event_card_as(shared: &Shared, key: &str, start: i64, own: &dyn Fn() -> Vec<String>) -> String {
    let Some(path) = pim::ours(key) else { return "{}".into() };
    let Some(calendar) = pim::collection_of(&path) else { return "{}".into() };
    let (removed, skipped) = mail::hidden_pim(shared);
    if removed.contains(&path) {
        return "{}".into();
    }
    let zone = TimeZone::system();
    let now = Zoned::now().timestamp().as_second();
    let at = |start: i64| agenda::file_occurrences(&path, &calendar, start, start + 1, &zone).into_iter().filter(|o| o.start == start).collect::<Vec<_>>();
    let mut found = if start > 0 {
        let mut found = at(start);
        // That time moved "only this time" since it was asked (a drag, the
        // other device): the changed occurrence its RECURRENCE-ID names (review
        // of 590eb18, later 7).
        if found.is_empty()
            && let Some(moved) = std::fs::read_to_string(&path).ok().and_then(|text| agenda::moved_to(&text, start, &zone))
            && moved != start
        {
            found = at(moved);
        }
        found
    } else {
        // The next time it comes, else its last, within a year either way.
        let all = agenda::file_occurrences(&path, &calendar, now - 366 * 86_400, now + 366 * 86_400, &zone);
        let next = all.iter().filter(|o| o.end > now).min_by_key(|o| o.start).or_else(|| all.iter().max_by_key(|o| o.start)).cloned();
        next.into_iter().collect()
    };
    agenda::mark_tasks(&mut found, &tasks_known(shared));
    // Declined by you, as the agenda marks it: takes no leg (review of ab7daa4, later 15).
    if found.iter().any(|e| e.attendees.iter().any(|a| a.answer == "declined")) {
        agenda::mark_declined(&mut found, &own());
    }
    let Some(o) = found.into_iter().next().filter(|o| !skipped.contains(&(path.clone(), o.start))) else { return "{}".into() };
    let item = agenda::items(std::slice::from_ref(&o), &zone, &|_| false).into_iter().next();
    let date = jiff::Timestamp::from_second(o.start).map_or_else(|_| Zoned::now().date(), |t| t.to_zoned(zone.clone()).date());
    let day = midnight(date, &zone);
    let end = date.tomorrow().map_or(day + 86_400, |d| midnight(d, &zone));
    let when = format!("{} · {}", tr().day(date), timeline::when_on(o.start, o.end, o.all_day, day, end, &zone, tr()));
    json(&serde_json::json!({
        "id": item.as_ref().map(|i| i.id.clone()).unwrap_or_default(),
        "key": o.key,
        "uid": o.uid,
        "start": o.start,
        "end": o.end,
        "all_day": o.all_day,
        "summary": if o.summary.trim().is_empty() { tr().text("agenda-untitled", None) } else { o.summary.clone() },
        "when": when,
        "before": o.margins.before,
        "after": o.margins.after,
        "location": o.location,
        "calendar": o.calendar,
        "color": o.color,
        "recurring": o.recurring,
        "cancelled": o.cancelled,
        "declined": o.declined,
        "tentative": o.tentative,
        "read_only": o.read_only,
        "with_others": o.with_others,
        "task": o.task,
        "list": o.list,
        "notes": o.notes,
        "organizer": o.organizer,
        "attendees": o.attendees,
        "why": item.map(|i| i.can.why).unwrap_or_default(),
    }))
}

/// One time of an event, read from its file when its panel opens: its notes,
/// organizer and guests, as JSON (`{notes, organizer, attendees}`); "{}" when
/// it is gone.
pub(crate) fn event_details(key: &str, start: i64) -> String {
    let Some(path) = pim::ours(key) else { return "{}".into() };
    let Some(calendar) = pim::collection_of(&path) else { return "{}".into() };
    let found = agenda::file_occurrences(&path, &calendar, start, start + 1, &TimeZone::system());
    found.iter().find(|o| o.start == start).map_or_else(|| "{}".to_string(), |o| serde_json::json!({ "notes": o.notes, "organizer": o.organizer, "attendees": o.attendees }).to_string())
}

/// What the Day and the Week show on this device ("Show"): the plan's steps,
/// meals, naps and the night, doses, and each calendar, all shown unless you
/// say. A calendar hidden is kept by its place, "account/id", with its name
/// for Show's list: two calendars of one name stay apart (review of 3a2fbd4,
/// later 11). Kept in the state folder, never shared: each device shows what
/// you ask of it there. Written only when it changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct View {
    steps: bool,
    needs: bool,
    doses: bool,
    /// Each ride or walk around an appointment drawn as an event of its own,
    /// rather than a quiet line in the appointment's lead (part E; D25). Off
    /// unless you say; an older Show without it reads as off.
    legs: bool,
    calendars: Vec<HiddenCalendar>,
    /// Calendars hidden by their names alone, as Show kept them until 28
    /// October 2026: read once, each name as every calendar it names then,
    /// and never written again.
    #[serde(skip_serializing)]
    calendars_hidden: Vec<String>,
}

/// A calendar Show hides: its place (`timeline::collection_of`), and its name when it was hidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
struct HiddenCalendar {
    id: String,
    #[serde(default)]
    name: String,
}

impl Default for View {
    fn default() -> View {
        View { steps: true, needs: true, doses: true, legs: false, calendars: Vec::new(), calendars_hidden: Vec::new() }
    }
}

impl View {
    /// Whether a thing of this source is left out: an event or a block of a calendar hidden.
    fn hides(&self, source: &timeline::Source) -> bool {
        matches!(source, timeline::Source::Calendar { collection, .. } if self.calendars.iter().any(|c| c.id == *collection))
    }
}

fn view_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("agenda-view.toml")
}

/// What Show says, as JSON (`{steps, needs, doses, calendars: [{id, name}]}`);
/// the default when nothing was said or the file does not read.
pub(crate) fn view() -> String {
    json(&read_view())
}

fn read_view() -> View {
    let mut view: View = std::fs::read_to_string(view_path()).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default();
    if !view.calendars_hidden.is_empty() {
        // An older Show, by names: each name as the calendars it names now, written once in places.
        let names = std::mem::take(&mut view.calendars_hidden);
        for collection in sioul_core::vdir::collections(sioul_core::vdir::Kind::Calendars).into_iter().filter(|c| names.contains(&c.name)) {
            let id = format!("{}/{}", collection.account, collection.id);
            if !view.calendars.iter().any(|c| c.id == id) {
                view.calendars.push(HiddenCalendar { id, name: collection.name.clone() });
            }
        }
        let _ = write_view(&view);
    }
    view
}

/// Show changed (`view`'s JSON): written whole, beside then in place. Returns
/// what went wrong, else "".
pub(crate) fn set_view(asked: &str) -> String {
    let Ok(view) = serde_json::from_str::<View>(asked) else { return tr().text("agenda-show-not-saved", None) };
    if view == read_view() {
        return String::new();
    }
    write_view(&view).map_or_else(|_| tr().text("agenda-show-not-saved", None), |()| String::new())
}

fn write_view(view: &View) -> std::io::Result<()> {
    let path = view_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text = toml::to_string(view).unwrap_or_default();
    sioul_core::filelock::replace(&path, text.as_bytes())
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

/// The tasks a time block is marked with: the plan's while nothing was written
/// since it was read (a task renamed or moved since shows as it is now:
/// review of 1880bce, later 4), else those last read, else the tasks alone,
/// read now: never everything (`Loaded::read`), which may write the mail's
/// index (review of 1880bce, L9).
fn tasks_known(shared: &Shared) -> Vec<sioul_core::tasks::Task> {
    if let Some(plan) = work::plan_shown().filter(|p| p.current()) {
        return plan.loaded().tasks.clone();
    }
    if work::plan_shown().is_none()
        && let Some(loaded) = shared.loaded.lock().ok().and_then(|l| l.clone())
    {
        return loaded.tasks.clone();
    }
    tasks_read_at(timeline::Stamp::now()).as_ref().clone()
}

/// The tasks read from disk while the plan is older than a write, kept for
/// that write (`timeline::Stamp`, taken before the read): read once until the
/// next write, never at each agenda run before the re-plan (review of
/// 3a2fbd4, later 15).
fn tasks_read_at(stamp: timeline::Stamp) -> Arc<Vec<sioul_core::tasks::Task>> {
    static READ: std::sync::Mutex<TasksRead> = std::sync::Mutex::new(None);
    tasks_read_in(&READ, stamp)
}

/// The tasks last read from disk, with the stamp they were read at.
type TasksRead = Option<(timeline::Stamp, Arc<Vec<sioul_core::tasks::Task>>)>;

fn tasks_read_in(read: &std::sync::Mutex<TasksRead>, stamp: timeline::Stamp) -> Arc<Vec<sioul_core::tasks::Task>> {
    let mut kept = read.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((at, tasks)) = kept.as_ref()
        && *at == stamp
    {
        return Arc::clone(tasks);
    }
    #[cfg(test)]
    TASK_READS.with(|n| n.set(n.get() + 1));
    let tasks = Arc::new(sioul_core::tasks::all(&TimeZone::system()));
    *kept = Some((stamp, Arc::clone(&tasks)));
    tasks
}

#[cfg(test)]
thread_local! {
    /// The tasks read from disk on this thread (`tasks_read_at`).
    static TASK_READS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
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
    let mut to = from + state.days * 86_400 + 3_600;
    let (removed, skipped) = mail::hidden_pim(shared);
    let stamp = now.timestamp().as_second();
    // A task's time block shows as the task it is, with its list (`agenda::mark_tasks`).
    let tasks = tasks_known(shared);
    let started = std::time::Instant::now();
    // The Day and the Week draw every kind, the day before's night too (its morning); the
    // Month its dates and appointments; What comes today in full and what differs (§4.7).
    let mode = Mode::of(state.days);
    let drawn = mode == Mode::Drawn;
    let read_from = if drawn { from - 86_400 } else { from };
    // Today drawn: tomorrow read with it, for the head line (one read of the calendars).
    let after_tomorrow = today.tomorrow().and_then(|d| d.tomorrow()).map_or(to, |d| midnight(d, &zone));
    let today_start = midnight(today, &zone);
    if drawn && today_start >= from && today_start < to {
        to = to.max(after_tomorrow);
    }
    let occurrences: Vec<Occurrence> = occurrences_between(read_from, to, &removed, &skipped, &tasks)
        .into_iter()
        // What comes: an event over is the past, shown only when you go back to it.
        .filter(|o| !state.upcoming || o.end.max(o.start) > stamp)
        .collect();
    let read = started.elapsed();
    // An errand's block and steps are health's (`errand_in`, decision 7).
    let errand = errand_in(&tasks);
    let mut items = agenda::items(&occurrences, &zone, &errand);
    // Legs as events of their own when Show says so (D25); else quiet lines in their appointment's lead.
    if read_view().legs {
        for item in items.iter_mut().filter(|i| i.kind == timeline::Kind::Leg) {
            item.kind = timeline::Kind::Event;
            item.owner.clear();
        }
    }
    // Each appointment's line, part F's agenda half: "Leave at 13:05 · Next: …".
    appointment_lines(&mut items, &occurrences, &tasks, &zone);
    let mut doses_note = String::new();
    let plan = work::plan_shown().filter(|p| p.made_on == today);
    let mut health_read = None;
    if mode == Mode::Comes {
        if let Some(plan) = &plan {
            items.extend(laid_items(plan, &occurrences, state.from, state.days, &now).into_iter().map(|mut item| {
                if errand(&item.owner) {
                    item.privacy = timeline::Privacy::Health;
                    item.address.clear();
                }
                item
            }));
        }
        let needs = sioul_core::needs::Needs::read(&sioul_core::health::Health::default_path());
        let days = plan.as_ref().filter(|p| p.current()).map_or_else(|| sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path()), |p| p.settings().needs_days.clone());
        let shifts_today = plan.as_ref().filter(|p| p.current()).map(|p| p.settings().shifts.clone());
        items.extend(needs_items(&needs, &days, shifts_today.as_ref(), &occurrences, state.from, state.days, &now));
        let health = sioul_core::health::Health::read(&sioul_core::health::Health::default_path());
        match &health {
            Ok(health) => items.extend(dose_items(health, state.from, state.days, &now)),
            Err(_) => doses_note = tr().text("agenda-doses-unread", None),
        }
        health_read = Some((health, needs, days));
    }
    if matches!(mode, Mode::Month | Mode::Comes) {
        // The Month's dates too: a prescription's, by its kind (a pure read of the health file).
        let month_health = (mode == Mode::Month).then(|| sioul_core::health::Health::read(&sioul_core::health::Health::default_path()).ok()).flatten();
        let health = health_read.as_ref().and_then(|(h, _, _)| h.as_ref().ok()).or(month_health.as_ref());
        items.extend(timeline::days_off(&load_config().time_off));
        items.extend(crate::dated::items(&tasks, &occurrences, health, state.from, state.days, &now, &errand));
        let open = |uid: &str| tasks.iter().any(|t| t.uid == uid && t.status.is_open());
        items = timeline::merge(items, stamp, &open);
    }
    if drawn {
        if let Some(plan) = &plan {
            // An errand's steps and its task done are health too, as its block is.
            items.extend(laid_items(plan, &occurrences, state.from, state.days, &now).into_iter().map(|mut item| {
                if errand(&item.owner) {
                    item.privacy = timeline::Privacy::Health;
                    item.address.clear();
                }
                item
            }));
        }
        let needs = sioul_core::needs::Needs::read(&sioul_core::health::Health::default_path());
        let days = plan.as_ref().filter(|p| p.current()).map_or_else(|| sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path()), |p| p.settings().needs_days.clone());
        let shifts_today = plan.as_ref().filter(|p| p.current()).map(|p| p.settings().shifts.clone());
        items.extend(needs_items(&needs, &days, shifts_today.as_ref(), &occurrences, state.from, state.days, &now));
        let health = sioul_core::health::Health::read(&sioul_core::health::Health::default_path());
        match &health {
            Ok(health) => items.extend(dose_items(health, state.from, state.days, &now)),
            Err(_) => doses_note = tr().text("agenda-doses-unread", None),
        }
        items.extend(timeline::days_off(&load_config().time_off));
        // The dates asked of you on their days, today and after (the unified agenda, part 4).
        items.extend(crate::dated::items(&tasks, &occurrences, health.as_ref().ok(), state.from, state.days, &now, &errand));
        // A task once a day: a block that holds its step gives way to it (`timeline::merge`).
        let open = |uid: &str| tasks.iter().any(|t| t.uid == uid && t.status.is_open());
        items = timeline::merge(items, stamp, &open);
    }
    named(&mut items);
    let mut days = timeline::days(&items, state.from, state.days, today, &zone, tr());
    timeline::set_hours(&mut days, &load_config().week_hours(), &zone);
    let (frame, head) = if drawn { dressed(&mut days, &items, plan.as_deref(), &occurrences, (read_from, to), &tasks, &now) } else { (None, Vec::new()) };
    // What comes: each day's rows (`timeline::comes`), and today's head line above them.
    let planned: &[sioul_core::tasks::Task] = plan.as_ref().map(|p| p.loaded().tasks.as_slice()).unwrap_or_default();
    let quiet = plan.as_ref().and_then(|p| p.quiet());
    let hidden = |uid: &str| quiet.is_some_and(|q| planned.iter().find(|t| t.uid == uid).is_some_and(|t| !q.keeps(t)));
    let comes: Vec<Option<timeline::Comes>> = match (&health_read, mode) {
        (Some((health, needs, needs_days)), Mode::Comes) => {
            let no_alarm: std::collections::BTreeSet<jiff::civil::Date> = dates(state.from, state.days).filter(|date| crate::wake::of_night(needs, *date, &zone, needs_days).1).collect();
            let set_times: std::collections::BTreeSet<String> = health.as_ref().map(|h| h.medicines.iter().filter(|m| matches!(m.schedule, sioul_core::health::Schedule::Day { .. })).map(|m| m.id.clone()).collect()).unwrap_or_default();
            let every_day = |key: &str| key.rsplit_once('@').is_some_and(|(medicine, _)| set_times.contains(medicine));
            let of = timeline::ComesOf { now: stamp, today, no_alarm: &no_alarm, every_day: &every_day, hidden: &hidden, zone: &zone, tr: tr() };
            // Each day with the next, where a night winding down after midnight lies.
            days.iter().enumerate().map(|(i, d)| Some(timeline::comes(d, days.get(i + 1), &of))).collect()
        }
        _ => days.iter().map(|_| None).collect(),
    };
    let head = if mode == Mode::Comes && days.iter().any(|d| d.today) { head_frames(&items, plan.as_deref(), &occurrences, (read_from, to), &tasks, &hidden, &now) } else { head };
    let lines: Vec<(Vec<Placed>, bool)> = days.iter().map(|d| if mode == Mode::Month { (timeline::month_lines(d), timeline::day_off(d)) } else { (Vec::new(), false) }).collect();
    let detail = |placed: &Placed| match &placed.item.source {
        timeline::Source::Calendar { key, start, .. } => occurrences.iter().find(|o| &o.key == key && o.start == *start).map(Detail::of),
        _ => None,
    };
    let row = |placed: Placed| Row { detail: detail(&placed), placed };
    let empty = days.iter().all(|d| d.head.is_empty() && d.timed.is_empty()) && comes.iter().flatten().all(timeline::Comes::is_empty);
    let can_add = agenda::default_calendar().is_some();
    let sentence = match (empty, can_add || !sioul_core::vdir::collections(sioul_core::vdir::Kind::Calendars).is_empty()) {
        (true, false) => tr().text("agenda-no-calendar", None),
        (true, true) => tr().text("agenda-nothing", None),
        _ => String::new(),
    };
    let shown = Shown {
        version: 3,
        from: state.from.to_string(),
        sentence,
        can_add,
        frame,
        head,
        doses_note,
        days: days
            .into_iter()
            .zip(comes)
            .zip(lines)
            .map(|((d, comes), (lines, off))| DayShown {
                date: d.date,
                title: d.title,
                today: d.today,
                past: d.past,
                start: d.start,
                hours_from: d.hours_from,
                hours_to: d.hours_to,
                // The Month and What comes say their own (`lines`, `comes`): the day's things once.
                head: if matches!(mode, Mode::Drawn | Mode::Events) { d.head.into_iter().map(row).collect() } else { Vec::new() },
                timed: if matches!(mode, Mode::Drawn | Mode::Events) { d.timed.into_iter().map(row).collect() } else { Vec::new() },
                hours: d.hours,
                bands: d.bands,
                hidden: d.hidden,
                doses: d.doses,
                edges: d.edges,
                planned: d.planned,
                plan: d.plan,
                more: d.more,
                said: d.said,
                lines: lines.into_iter().map(row).collect(),
                off,
                comes: comes.map(|c| ComesShown { rows: c.rows.into_iter().map(row).collect(), doses: c.doses, plan: c.plan }),
            })
            .collect(),
    };
    (json(&shown), read)
}

/// Each appointment's line on the timeline (composed tasks' part F, the
/// unified agenda's part 4): when to leave, from the time kept before it
/// (your own time around a meeting with others), and the next step of what
/// prepares it (`pairs::appointment_view`); its file's links read with it.
fn appointment_lines(items: &mut [timeline::Item], occurrences: &[Occurrence], tasks: &[sioul_core::tasks::Task], zone: &TimeZone) {
    for item in items.iter_mut().filter(|i| i.kind == timeline::Kind::Event) {
        let timeline::Source::Calendar { key, start, .. } = &item.source else { continue };
        let Some(o) = occurrences.iter().find(|o| &o.key == key && o.start == *start) else { continue };
        let links = std::fs::read_to_string(&o.key).ok().and_then(|text| agenda::event_ref(&text, &o.key, zone)).map(|r| r.links).unwrap_or_default();
        // Its legs read with it (part E): Leave at from the first, their titles on its line.
        let legs = sioul_core::anchors::legs_seen_in(&o.uid, occurrences);
        let view = sioul_core::pairs::appointment_view(o, &links, tasks, &legs, None, tr(), zone);
        if !view.line.is_empty() {
            item.line = view.line;
        }
    }
}

/// An appointment as part F says it, for its card (`appointmentView(key,
/// start)`): when to leave, what prepares it, its steps (Before) and its
/// lists (Bring), as JSON; "{}" when it is gone. Reads its one file, and the
/// tasks as last read (never the stores on the window's thread).
pub(crate) fn appointment_card(shared: &Shared, key: &str, start: i64) -> String {
    let Some(path) = pim::ours(key) else { return "{}".into() };
    let Some(calendar) = pim::collection_of(&path) else { return "{}".into() };
    let zone = TimeZone::system();
    let found = agenda::file_occurrences(&path, &calendar, start, start + 1, &zone);
    let Some(o) = found.into_iter().find(|o| o.start == start) else { return "{}".into() };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let links = agenda::event_ref(&text, key, &zone).map(|r| r.links).unwrap_or_default();
    let known = work::loaded_known(shared);
    let tasks = known.as_ref().map(|l| l.tasks.clone()).unwrap_or_default();
    let events = known.as_ref().map(|l| l.events.as_slice()).unwrap_or_default();
    // Its legs, each read from its file (part E: Getting there, Coming back); a leg's own: what it is to its appointment.
    let legs = sioul_core::anchors::legs_seen_of(&o.uid, events, &zone);
    let leg_of = sioul_core::anchors::leg_of(&text, events, tr(), &zone);
    json(&sioul_core::pairs::appointment_view(&o, &links, &tasks, &legs, leg_of, tr(), &zone))
}

/// The days from `from`, `count` of them: each date.
fn dates(from: jiff::civil::Date, count: i64) -> impl Iterator<Item = jiff::civil::Date> {
    (0..count).map_while(move |offset| from.checked_add(jiff::Span::new().days(offset)).ok())
}

/// Each day's steps, tasks done and time kept for you, as the plan lays them
/// (`timeline::from_day`): today as the Tasks job laid and dressed it
/// (`PlanShown::today`), the one producer of today's layout; a day to come
/// laid from the plan (`dayview::lay_on`, every task: in quiet time those
/// hidden now are folded into bands later, `timeline::dress`), within its
/// reach; a day gone by, its tasks done where their work was.
fn laid_items(plan: &work::PlanShown, occurrences: &[Occurrence], from: jiff::civil::Date, count: i64, now: &Zoned) -> Vec<timeline::Item> {
    let zone = now.time_zone();
    let tasks = &plan.loaded().tasks;
    let mut out = Vec::new();
    for date in dates(from, count) {
        if date == now.date() {
            out.extend(timeline::from_day(&plan.today, date, tasks));
            continue;
        }
        let start = midnight(date, zone);
        let end = date.tomorrow().map_or(start + 86_400, |d| midnight(d, zone));
        let events: Vec<Occurrence> = occurrences.iter().filter(|o| o.end > start && o.start < end).cloned().collect();
        let laid = sioul_core::dayview::lay_on(date, now, &events, plan.plan(), tasks, plan.settings(), sioul_core::dayview::Drawn::ADMIN);
        out.extend(timeline::from_day(&laid, date, tasks));
    }
    out
}

/// Meals, naps and the night, each day's own as it has them (`Needs::blocks_of`:
/// moved, its times changed, quiet that day; taken out that day, not there),
/// and the night before the first day (its morning): Health's, never
/// beyond the window, never named but by their own names. Meals sit where
/// the plan keeps them: today's moved past today's events as the plan moved
/// them (`Settings::shifts`), another day's past its events by the plan's rule
/// (`work::held_from`). What each lets change it: a day past, or a block over,
/// stays as it was (its writer refuses it: `Needs::change_day`).
fn needs_items(needs: &sioul_core::needs::Needs, days: &sioul_core::needs::Days, shifts_today: Option<&std::collections::BTreeMap<String, i64>>, occurrences: &[Occurrence], from: jiff::civil::Date, count: i64, now: &Zoned) -> Vec<timeline::Item> {
    use sioul_core::timeline::{Can, Certainty, Form, Item, Kind, Move, Privacy, Source, Time, Tone};
    let zone = now.time_zone();
    let today = now.date();
    let stamp = now.timestamp().as_second();
    let mut out = Vec::new();
    let before = from.yesterday().ok();
    for date in before.into_iter().chain(dates(from, count)) {
        let held = || {
            let start = midnight(date, zone);
            let end = date.tomorrow().map_or(start + 86_400, |d| midnight(d, zone));
            let near: Vec<Occurrence> = occurrences.iter().filter(|o| o.end > start - 86_400 && o.start < end + 86_400).cloned().collect();
            work::held_by_plan(date, now).unwrap_or_else(|| work::held_from(date, now, near))
        };
        // One rule with Health's list (the needs adapter, part 5): today as the plan pushed them.
        let pushed = match (date == today, shifts_today) {
            (true, Some(shifts)) => shifts.clone(),
            _ => crate::health::pushed_on(needs, days, date, now, &held),
        };
        for kept in needs.blocks_of(date, zone, days, &|key: &str| pushed.get(key).copied().unwrap_or(0)) {
            // The day before the first: its night alone, for its morning.
            if Some(date) == before && kept.kind != "sleep" {
                continue;
            }
            let kind = match kept.kind {
                "meal" => Kind::Meal,
                "nap" => Kind::Nap,
                _ => Kind::Night,
            };
            let until = match kind {
                Kind::Night => kept.end,
                _ => needs.lengths(date, &kept.key, days).map_or(kept.end, |(minutes, _, _)| (kept.at + i64::from(minutes) * 60).min(kept.end)),
            };
            let change = days.get(date, &kept.key);
            let (past, over) = (date < today, kept.end <= stamp);
            let why = if past {
                tr().text("need-past", None)
            } else if over {
                tr().text("need-over", None)
            } else {
                String::new()
            };
            out.push(Item {
                id: timeline::need_id(date, &kept.key),
                kind,
                time: Time::Span { start: kept.start, at: kept.at, until, end: kept.end },
                certainty: if change.is_some() { Certainty::ThatDay } else { Certainty::Usual },
                source: Source::Needs { date, key: kept.key.clone() },
                can: Can { edit: if past { Form::None } else { Form::Need }, drag: if past || over { Move::None } else { Move::Need }, ends: !past && !over, delete: false, tick: false, why },
                privacy: Privacy::Health,
                title: crate::health::block_name(kept.kind, &kept.key, &kept.name),
                line: String::new(),
                said: String::new(),
                tone: if change.is_some_and(|c| c.quiet) { Tone::Quiet } else { Tone::Plain },
                colour: None,
                address: String::new(),
                owner: String::new(),
                holds_time: true,
            });
        }
    }
    out
}

/// The doses of today and of the days to come shown, by key and time, read
/// from the health file alone (`Health::read`: no record, nothing written):
/// today's are drawn in the state the Health job's rows give them (`doseMarks`;
/// one the rows do not hold is being read), a day to come's plainly; a day
/// gone by draws none, which could read as missed. Named "Dose", never by
/// the medicine (decision 7); nothing to press on the timeline.
fn dose_items(health: &sioul_core::health::Health, from: jiff::civil::Date, count: i64, now: &Zoned) -> Vec<timeline::Item> {
    use sioul_core::timeline::{Can, Certainty, DoseState, Item, Kind, Privacy, Source, Time, Tone};
    let zone = now.time_zone();
    let today = now.date();
    let first = from.max(today);
    let Some(last) = from.checked_add(jiff::Span::new().days(count)).ok().filter(|last| *last > first) else { return Vec::new() };
    let (Ok(start), Ok(end)) = (first.to_zoned(zone.clone()), last.to_zoned(zone.clone())) else { return Vec::new() };
    health
        .doses(&start, &end)
        .into_iter()
        .map(|dose| {
            let ahead = dose.at.date() > today;
            Item {
                id: timeline::dose_id(&dose.key),
                kind: Kind::Dose,
                time: Time::At { at: dose.at.timestamp().as_second() },
                certainty: Certainty::Dose(if ahead { DoseState::Ahead } else { DoseState::Reading }),
                source: Source::Dose { key: dose.key },
                can: Can::nothing(),
                privacy: Privacy::Health,
                title: tr().text("agenda-dose", None),
                line: String::new(),
                said: String::new(),
                tone: if ahead { Tone::Muted } else { Tone::Plain },
                colour: None,
                address: String::new(),
                owner: String::new(),
                holds_time: false,
            }
        })
        .collect()
}

/// The days laid for the Day and the Week, dressed (`timeline::dress`): their
/// hours, the night's edges, the doses' lane, quiet time's bands, the lists'
/// line, today's lines; the hours shown; and today's head line, given ahead.
fn dressed(days: &mut [timeline::Day], items: &[timeline::Item], plan: Option<&work::PlanShown>, occurrences: &[Occurrence], read: (i64, i64), known: &[sioul_core::tasks::Task], now: &Zoned) -> (Option<timeline::Frame>, Vec<timeline::HeadFrame>) {
    let zone = now.time_zone().clone();
    let today = now.date();
    let config = load_config();
    let settings = plan.map(work::PlanShown::settings);
    let hours = |date: jiff::civil::Date| -> Vec<sioul_core::dayview::Stretch> {
        let stretches = match settings {
            Some(settings) => settings.hours_on(date, &zone),
            None => sioul_core::plan::Settings::of_hours(&config.week_hours(), sioul_core::areas::TaskAreas::usual()).hours_on(date, &zone),
        };
        stretches.into_iter().map(|(start, end, area)| sioul_core::dayview::Stretch { start, end, kind: area.id() }).collect()
    };
    // In quiet time, what the task pages hide now (decision g): a coming day's steps of it make bands.
    let planned = plan.map(|p| p.loaded().tasks.as_slice()).unwrap_or_default();
    let quiet = plan.and_then(work::PlanShown::quiet);
    let hidden = |uid: &str| quiet.is_some_and(|q| planned.iter().find(|t| t.uid == uid).is_some_and(|t| !q.keeps(t)));
    let horizon = today.checked_add(jiff::Span::new().days(sioul_core::plan::HORIZON_DAYS - 1)).unwrap_or(today);
    let dressing = timeline::Dressing {
        today,
        now: now.timestamp().as_second(),
        zone: &zone,
        tr: tr(),
        hours: &hours,
        horizon,
        hidden: &hidden,
        said: plan.map(|p| p.today.said.clone()).unwrap_or_default(),
        day_start: i32::try_from(config.agenda.day_start.unwrap_or(7).min(23)).unwrap_or(7),
    };
    let frame = timeline::dress(days, &dressing);
    if !days.iter().any(|d| d.today) {
        return (Some(frame), Vec::new());
    }
    (Some(frame), head_frames(items, plan, occurrences, read, known, &hidden, now))
}

/// Whether a task (by UID) is a health errand among `tasks`: its block and
/// steps are health's, never named by its prescription on the timeline nor in
/// the head line (decision 7), never given to an agent (the MCP makes its own
/// items, through `ForAgents`). The record is never read here: an errand is
/// known by its category or its title.
fn errand_in(tasks: &[sioul_core::tasks::Task]) -> impl Fn(&str) -> bool + '_ {
    move |uid: &str| tasks.iter().find(|t| t.uid == uid).is_some_and(|t| sioul_core::health::is_errand(t, &Default::default()))
}

/// Things named as the timeline and the head line name them, one rule for
/// both: time for you and the time kept free by their words, an event of no
/// title said so, an errand by its kind, never by its prescription.
fn named(items: &mut [timeline::Item]) {
    for item in items.iter_mut() {
        if item.title.trim().is_empty() && matches!(item.kind, timeline::Kind::Gain | timeline::Kind::Slack) {
            item.title = tr().text(if item.kind == timeline::Kind::Gain { "capacity-gain-slot" } else { "capacity-slack" }, None);
        }
        if item.title.trim().is_empty() && matches!(item.kind, timeline::Kind::Event | timeline::Kind::Block) {
            item.title = tr().text("agenda-untitled", None);
        }
        // An errand's block, step, task done, event, and date asked: by its kind (a prescription's
        // own dates are named by theirs already: `dated::item`).
        let errand_date = item.kind == timeline::Kind::Dated && !item.owner.is_empty();
        if item.privacy == timeline::Privacy::Health && (errand_date || matches!(item.kind, timeline::Kind::Block | timeline::Kind::Step | timeline::Kind::Done | timeline::Kind::Event)) {
            item.title = match sioul_core::health::errand_kind_of_title(&item.title) {
                Some(sioul_core::health::ErrandKind::Renew) => tr().text("agenda-errand-renew", None),
                _ => tr().text("agenda-errand-refill", None),
            };
        }
    }
}

/// Today's head line from now to midnight, given ahead (`timeline::head_line`):
/// what each moment is for (`quiet::frames`, the home card's), today's things
/// and tomorrow's first appointment, and tomorrow morning's alarm. Tomorrow's
/// appointments come from the days' read when it holds tomorrow (`read`, its
/// span), else are read, and are marked with their tasks (`known`). Every
/// thing is named as the timeline names it (`named`: an errand by its kind,
/// never by its prescription), a calendar that Show hides is left out, and a
/// task's block that quiet time hides now (`hidden`) is said as work.
fn head_frames(items: &[timeline::Item], plan: Option<&work::PlanShown>, occurrences: &[Occurrence], read: (i64, i64), known: &[sioul_core::tasks::Task], hidden: &dyn Fn(&str) -> bool, now: &Zoned) -> Vec<timeline::HeadFrame> {
    let zone = now.time_zone().clone();
    let today = now.date();
    let config = load_config();
    let Some(tomorrow) = today.tomorrow().ok() else { return Vec::new() };
    let (Ok(next_midnight), Ok(after)) = (tomorrow.to_zoned(zone.clone()), tomorrow.tomorrow().and_then(|d| d.to_zoned(zone.clone()))) else { return Vec::new() };
    let needs = sioul_core::needs::Needs::read(&sioul_core::health::Health::default_path());
    let days = plan.filter(|p| p.current()).map_or_else(|| sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path()), |p| p.settings().needs_days.clone());
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let held = work::held_by_plan(today, now).unwrap_or_else(|| sioul_core::plan::event_spans(occurrences, 0));
    let blocks = sioul_core::quiet::Blocks::of(&needs, &days, &held, now);
    let moments: Vec<(i64, i64, sioul_core::quiet::Mode)> = sioul_core::quiet::frames(&config.week_hours(), &config.time_off, &overrides, &blocks, &[], now, &next_midnight, &[], 48)
        .into_iter()
        .map(|(from, until, mode)| (from.timestamp().as_second(), until.timestamp().as_second(), mode))
        .collect();
    let (start, end) = (midnight(today, &zone), next_midnight.timestamp().as_second());
    let view = read_view();
    let shown = |item: &timeline::Item| !view.hides(&item.source);
    let worded = |mut item: timeline::Item| {
        if matches!(item.kind, timeline::Kind::Block | timeline::Kind::Step) && item.privacy != timeline::Privacy::Health && !item.owner.is_empty() && hidden(&item.owner) {
            item.title = tr().text("head-work", None);
        }
        item
    };
    let today_items: Vec<timeline::Item> = items
        .iter()
        .filter(|item| {
            let (at, until) = item.time.whole(&zone);
            at < end && until > start
        })
        .filter(|item| shown(item))
        .cloned()
        .map(worded)
        .collect();
    // Tomorrow's appointments, whether tomorrow is shown or not.
    let next_start = next_midnight.timestamp().as_second();
    let after_start = after.timestamp().as_second();
    let tomorrow_events: Vec<Occurrence> = if read.0 <= next_start && read.1 >= after_start {
        occurrences.iter().filter(|o| o.end > next_start && o.start < after_start).cloned().collect()
    } else {
        let mut read = read_between(next_start, after_start, &zone);
        agenda::mark_tasks(&mut read, known);
        read
    };
    let mut tomorrow_items = agenda::items(&tomorrow_events, &zone, &errand_in(known));
    named(&mut tomorrow_items);
    let tomorrow_items: Vec<timeline::Item> = tomorrow_items.into_iter().filter(|item| shown(item)).map(worded).collect();
    let alarm = needs.alarm_of(today, &zone, &days).filter(|a| !a.skipped).map(|a| a.at);
    // The dates asked this week, today to Sunday (`dated::items`; health's never: the head
    // line names none), for its second sentence in work and admin time.
    let to_sunday = 8 - i64::from(today.weekday().to_monday_one_offset());
    let dates = crate::dated::items(known, occurrences, None, today, to_sunday, now, &errand_in(known));
    let inputs = timeline::HeadInputs { now: now.timestamp().as_second(), next_midnight: next_start, moments: &moments, today: &today_items, tomorrow: &tomorrow_items, alarm, dates: &dates, first_hour: load_config().agenda.day_start, zone: &zone, tr: tr() };
    timeline::head_line(&inputs)
}

/// Whether a run makes the days: once the page asked for them (`asked`), or
/// from the first plan (the Tasks job's first run) until the window has shown
/// them once, so that the page's first opening never shows an empty week,
/// which would read as "nothing planned" (review of 1880bce, L18; of
/// 85da7f0, later 10).
fn days_wanted(shared: &Shared) -> bool {
    shared.agenda_asked.load(Ordering::Relaxed) || (work::plan_shown().is_some() && !shared.agenda_prefilled.load(Ordering::Relaxed))
}

/// Whether the days wait for the re-plan under way: today's plan is older
/// than a write (a drag, a sync) and its re-plan is asked, not published yet.
/// Drawn now, today's steps would come from the older plan beside the newer
/// calendars, a step dragged showing at both its times (review of 3a2fbd4,
/// later 4); the days shown stay until the re-plan's publish brings them.
fn waits_for_replan(shared: &Shared) -> bool {
    shared.replan_due.load(Ordering::SeqCst) && work::plan_shown().is_some_and(|p| p.made_on == Zoned::now().date() && !p.current())
}

/// One run of the agenda's job: the overlaps of the next two weeks, always,
/// for the Porch; the days, with their own generation, once they are wanted
/// (`days_wanted`), with how long their read of the calendars took.
struct Run {
    generation: u64,
    overlaps: String,
    days: Option<(u64, String, std::time::Duration)>,
}

fn run(shared: &Shared) -> Run {
    let generation = shared.agenda_generation.fetch_add(1, Ordering::Relaxed) + 1;
    let overlaps = overlaps_now(shared);
    let days = (days_wanted(shared) && !waits_for_replan(shared)).then(|| {
        let (agenda, read) = made(shared);
        (shared.agenda_days_generation.fetch_add(1, Ordering::Relaxed) + 1, agenda, read)
    });
    Run { generation, overlaps, days }
}

/// Makes the overlaps of the next two weeks at every run, for the Porch, and
/// the agenda's days once they are wanted (`days_wanted`), on a thread, then
/// shows them: whenever the plan or a calendar changes.
pub(crate) fn show_agenda(qt: &QtThread, shared: &Arc<Shared>) {
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.agenda_job, move |shared| {
        let started = std::time::Instant::now();
        let Run { generation, overlaps, days } = run(shared);
        if crate::timed() {
            let (bytes, read) = days.as_ref().map_or((0, 0), |(_, agenda, read)| (agenda.len(), read.as_millis()));
            eprintln!("sioul-perf: agenda job {} ms, its calendars read {read} ms, {bytes} bytes", started.elapsed().as_millis());
        }
        let _ = qt.queue(move |mut sioul| {
            // Only the newest (see `backend::show`): the newest days of the runs
            // that made days, the newest overlaps of all.
            let shared = sioul.shared();
            if let Some((days_generation, agenda, _)) = &days
                && shared.agenda_days_generation.load(Ordering::Relaxed) == *days_generation
                && shared.agenda_days_shown_generation.fetch_max(*days_generation, Ordering::Relaxed) <= *days_generation
            {
                shared.agenda_prefilled.store(true, Ordering::Relaxed);
                sioul.as_mut().set_agenda(QString::from(agenda));
            }
            let shared = sioul.shared();
            if shared.agenda_generation.load(Ordering::Relaxed) == generation && shared.agenda_shown_generation.fetch_max(generation, Ordering::Relaxed) <= generation {
                sioul.as_mut().set_overlaps(QString::from(&overlaps));
            }
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

    /// Just after a write, while its re-plan is due, the days wait for it:
    /// today is never drawn from the older plan beside the newer calendars, a
    /// dragged step at both its times (review of 3a2fbd4, later 4). The writer
    /// itself makes the re-plan due, before any job starts (review of
    /// 590eb18, later 12): a drag's write, then the agenda's run.
    #[test]
    fn the_days_wait_for_the_replan_after_a_write() {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Replan wait", None, &["VEVENT"]).unwrap();
        let file = calendar.dir.join("moved.ics");
        vdir::write_item(&file, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:replan-wait\r\nDTSTART:20991001T090000Z\r\nDTEND:20991001T100000Z\r\nSUMMARY:Moved\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            state.from = Zoned::now().date();
            state.days = 7;
        }
        shared.agenda_asked.store(true, Ordering::Relaxed);
        work::publish_now(&shared);
        assert!(run(&shared).days.is_some(), "no re-plan due: the days");
        // A drag's write after the plan, by its own writer: its re-plan due, not published yet.
        let at = |h: i8| jiff::civil::date(2099, 10, 1).at(h, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp().as_second();
        let key = file.display().to_string();
        assert!(matches!(crate::pim::move_written(&shared, &key, at(9), at(11), at(12), false), Ok(Some(_))));
        assert!(work::plan_shown().is_some_and(|p| !p.current()));
        assert!(run(&shared).days.is_none(), "the re-plan due: the days shown stay");
        shared.replan_due.store(false, Ordering::SeqCst);
        assert!(run(&shared).days.is_some(), "published: drawn again");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// The tasks read from disk while the plan is older than a write: once a
    /// write, whatever the agenda's runs meanwhile (review of 3a2fbd4, later 15).
    #[test]
    fn the_tasks_are_read_once_a_write() {
        sioul_core::config::stand_ins_only();
        let kept = std::sync::Mutex::new(None);
        let stamp = timeline::Stamp::now();
        TASK_READS.with(|n| n.set(0));
        let first = tasks_read_in(&kept, stamp);
        let again = tasks_read_in(&kept, stamp);
        assert_eq!(TASK_READS.with(std::cell::Cell::get), 1, "read once for one write");
        assert!(Arc::ptr_eq(&first, &again));
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "Read once", None, &["VTODO"]).unwrap();
        let (zone, now) = (TimeZone::system(), Zoned::now());
        let edit = sioul_core::tasks::TaskEdit { title: "Read once".into(), ..sioul_core::tasks::TaskEdit::default() };
        vdir::write_item(&list.dir.join("read-once.ics"), &sioul_core::tasks::new_task(&edit, "read-once", &zone, &now).unwrap()).unwrap();
        let after = tasks_read_in(&kept, timeline::Stamp::now());
        assert_eq!(TASK_READS.with(std::cell::Cell::get), 2, "read again after a write");
        assert!(after.iter().any(|t| t.uid == "read-once"), "the new task in it");
        let _ = std::fs::remove_dir_all(&list.dir);
    }

    /// An event's card reads its own file, wherever it opens: the time asked
    /// of a repeating one, the next time it comes when none is asked; its
    /// notes, the time kept around it; none while its deletion waits for Undo
    /// (the unified agenda, part 3).
    #[test]
    fn an_event_s_card_reads_its_own_file() {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Card file", None, &["VEVENT"]).unwrap();
        let (now, zone) = (Zoned::now(), TimeZone::system());
        // A hundred days ahead: never the first thing of another test's tomorrow.
        let tomorrow = now.date().checked_add(jiff::Span::new().days(100)).unwrap();
        let at = |date: jiff::civil::Date, hour: i8| date.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap();
        let utc = |z: Zoned| z.with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let file = calendar.dir.join("card-weekly.ics");
        std::fs::write(&file, format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:card-weekly\r\nDTSTART:{}\r\nDTEND:{}\r\nRRULE:FREQ=DAILY;COUNT=5\r\nSUMMARY:Card standup\r\nLOCATION:Room 2\r\nDESCRIPTION:Bring the notes.\r\nX-SIOUL-BEFORE:10\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(at(tomorrow, 9)), utc(at(tomorrow, 10)))).unwrap();
        let shared = Arc::new(Shared::default());
        let key = file.display().to_string();
        let third = at(tomorrow.checked_add(jiff::Span::new().days(2)).unwrap(), 9).timestamp().as_second();
        let card: serde_json::Value = serde_json::from_str(&event_card(&shared, &key, third)).unwrap();
        assert_eq!(card["start"], third, "the time asked");
        assert_eq!((card["summary"].as_str(), card["location"].as_str(), card["notes"].as_str()), (Some("Card standup"), Some("Room 2"), Some("Bring the notes.")));
        assert_eq!((card["recurring"].as_bool(), card["before"].as_u64()), (Some(true), Some(10)), "it repeats; ten minutes kept before it");
        assert!(card["when"].as_str().is_some_and(|w| w.contains("09:00")), "{}", card["when"]);
        let next: serde_json::Value = serde_json::from_str(&event_card(&shared, &key, 0)).unwrap();
        assert_eq!(next["start"], at(tomorrow, 9).timestamp().as_second(), "none asked: the next time it comes");
        assert_eq!(event_card(&shared, &key, third + 60), "{}", "no time of it then");
        // That time moved "only this time" (here or on the other device): its card follows it (review of 590eb18, later 7).
        let text = std::fs::read_to_string(&file).unwrap();
        let later = third + 2 * 3_600;
        std::fs::write(&file, sioul_core::agenda::moved(&text, third, later, later + 3_600, true, &zone).unwrap()).unwrap();
        let moved: serde_json::Value = serde_json::from_str(&event_card(&shared, &key, third)).unwrap();
        assert_eq!(moved["start"], later, "the time asked, where it went: {moved}");
        crate::mail::wait_to_remove(&shared, file.clone());
        assert_eq!(event_card(&shared, &key, third), "{}", "its deletion waits for Undo: no card");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// An invitation you declined says so on its card, so that it offers no
    /// leg; one declined by another guest does not (review of ab7daa4, later 15).
    #[test]
    fn an_invitation_you_declined_says_so_on_its_card() {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Card declined", None, &["VEVENT"]).unwrap();
        let file = calendar.dir.join("card-declined.ics");
        // A hundred days ahead, its time asked: within the card's reach whatever the day.
        let day = Zoned::now().date().checked_add(jiff::Span::new().days(100)).unwrap();
        let start = day.at(9, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap();
        let utc = |z: &Zoned| z.strftime("%Y%m%dT%H%M%SZ").to_string();
        std::fs::write(&file, format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:card-declined\r\nDTSTAMP:20261020T100000Z\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Board review\r\nORGANIZER:mailto:chair@board.example\r\nATTENDEE;PARTSTAT=DECLINED:mailto:me@home.example\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:other@board.example\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(&start), utc(&start.checked_add(jiff::Span::new().hours(1)).unwrap()))).unwrap();
        let (shared, key) = (Shared::default(), file.display().to_string());
        let at = start.timestamp().as_second();
        let card = |own: Vec<String>| -> serde_json::Value { serde_json::from_str(&event_card_as(&shared, &key, at, &|| own.clone())).unwrap() };
        assert_eq!(card(vec!["Me@Home.example".into()])["declined"], true, "declined by you");
        assert_eq!(card(vec!["other@board.example".into()])["declined"], false, "declined by someone else");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// One test at a time writes Show's file.
    fn view_held() -> std::sync::MutexGuard<'static, ()> {
        static VIEW: std::sync::Mutex<()> = std::sync::Mutex::new(());
        VIEW.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Show is this device's: everything shown when nothing was said (or the
    /// file does not read), what is said read back, the same said again
    /// written nothing, and nothing but its own file written.
    #[test]
    fn show_is_kept_on_this_device_and_written_only_when_it_changes() {
        let _view = view_held();
        sioul_core::config::stand_ins_only();
        let path = view_path();
        let _ = std::fs::remove_file(&path);
        assert_eq!(read_view(), View::default());
        let all: serde_json::Value = serde_json::from_str(&view()).unwrap();
        assert_eq!(all, serde_json::json!({ "steps": true, "needs": true, "doses": true, "legs": false, "calendars": [] }));
        assert_eq!(set_view(r#"{"steps": true, "needs": false, "doses": true, "calendars": [{"id": "local/club", "name": "Club"}]}"#), "");
        assert_eq!(read_view(), View { needs: false, calendars: vec![HiddenCalendar { id: "local/club".into(), name: "Club".into() }], ..View::default() });
        let written = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(set_view(r#"{"needs": false, "calendars": [{"id": "local/club", "name": "Club"}]}"#), "", "what is not said stays shown");
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), written, "the same said again writes nothing");
        assert_eq!(set_view("not json"), tr().text("agenda-show-not-saved", None));
        std::fs::write(&path, "steps = [").unwrap();
        assert_eq!(read_view(), View::default(), "a file that does not read shows everything");
        let _ = std::fs::remove_file(&path);
    }

    /// Show hides a calendar by its place, never by its name: of two
    /// calendars named alike, the one hidden alone; an older Show, kept by
    /// names, read once as every calendar its names name, written in places
    /// (review of 3a2fbd4, later 11).
    #[test]
    fn show_hides_a_calendar_by_its_place() {
        let _view = view_held();
        sioul_core::config::stand_ins_only();
        let first = vdir::create(Kind::Calendars, vdir::LOCAL, "Twin name", None, &["VEVENT"]).unwrap();
        let second = vdir::create(Kind::Calendars, vdir::LOCAL, "Twin name", None, &["VEVENT"]).unwrap();
        let place = |c: &vdir::Collection| format!("{}/{}", c.account, c.id);
        assert_ne!(place(&first), place(&second));
        let source = |c: &vdir::Collection| timeline::Source::Calendar { key: c.dir.join("a.ics").display().to_string(), uid: "a".into(), start: 0, recurring: false, calendar: "Twin name".into(), collection: timeline::collection_of(&c.dir.join("a.ics").display().to_string()) };
        assert_eq!(timeline::collection_of(&first.dir.join("a.ics").display().to_string()), place(&first), "a file's calendar by its place");
        let one = View { calendars: vec![HiddenCalendar { id: place(&first), name: "Twin name".into() }], ..View::default() };
        assert!(one.hides(&source(&first)) && !one.hides(&source(&second)), "the one hidden alone");
        // Kept by names before: read once as both, in places, and written so.
        std::fs::create_dir_all(view_path().parent().unwrap()).unwrap();
        std::fs::write(view_path(), "steps = true\nneeds = true\ndoses = true\ncalendars_hidden = [\"Twin name\"]\n").unwrap();
        let read = read_view();
        assert!(read.hides(&source(&first)) && read.hides(&source(&second)), "as it hid them: {read:?}");
        let text = std::fs::read_to_string(view_path()).unwrap();
        assert!(!text.contains("calendars_hidden") && text.contains(&place(&first)) && text.contains(&place(&second)), "{text}");
        assert_eq!(read_view(), read, "read once: the same since");
        let _ = std::fs::remove_file(view_path());
        let _ = std::fs::remove_dir_all(&first.dir);
        let _ = std::fs::remove_dir_all(&second.dir);
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
            // The week, which reads health's needs and doses, by pure reads, while every health lock is held.
            state.from = now.date();
            state.days = 7;
            state.upcoming = false;
        }
        // What a reader of health could write: the health file's last-read and aside copies; and
        // this test's calendar. Other tests write elsewhere in the same folders meanwhile; the
        // health tests, which write and read these, wait (their own read once came in as this
        // job's write). Today's slots of time for you, which the Tasks job writes as it lays
        // today (other tests do it meanwhile): this thread, where the job runs, never lays it.
        let _health_tests = crate::health::health_files_held();
        let health = sioul_core::health::Health::default_path();
        let watched = [sioul_core::health::Health::last_read_path(&health), health.with_extension("toml.unreadable")];
        let state = |path: &std::path::Path| std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()));
        let seen = || (watched.iter().map(|p| state(p)).collect::<Vec<_>>(), files(&calendar.dir));
        let before = seen();
        // A run while another thread holds every health lock, for longer than a run takes.
        let unlocked = |run: &dyn Fn() -> String| {
            let (held, release) = std::sync::mpsc::channel::<()>();
            let (go, ready) = std::sync::mpsc::channel::<()>();
            let holder = std::thread::spawn(move || {
                let _locks = crate::health::every_lock_held();
                let _ = go.send(());
                let _ = release.recv_timeout(std::time::Duration::from_secs(20));
            });
            ready.recv().unwrap();
            let started = std::time::Instant::now();
            let shown = run();
            assert!(started.elapsed() < std::time::Duration::from_secs(10), "it waited for a health lock");
            let _ = held.send(());
            holder.join().unwrap();
            shown
        };
        // With no plan yet, the job reads the tasks alone, never everything, which writes the
        // mail's index (review of 1880bce, L9): counted on this thread, where the job runs.
        // Today's slots of time for you are written by their one writer (`Slots::save`),
        // counted on this thread too: other tests lay today and write them meanwhile.
        crate::work::LOADS.with(|n| n.set(0));
        let slots = sioul_core::attention::Slots::saved_here();
        let shown = unlocked(&|| {
            let (shown, _) = made(&shared);
            let _ = overlaps_now(&shared);
            shown
        });
        assert!(shown.contains("agenda-job-dentist"), "{shown}");
        let parsed: serde_json::Value = serde_json::from_str(&shown).unwrap();
        assert_eq!(parsed["version"], 3);
        assert_eq!(before, seen(), "nothing written");
        assert_eq!(sioul_core::attention::Slots::saved_here(), slots, "no slots written");
        assert_eq!(crate::work::LOADS.with(std::cell::Cell::get), 0, "the tasks alone read");
        // With the plan made (its branch: today as the Tasks job laid it, the coming days laid
        // by the plan, review of 3a2fbd4, later 5): the job still lays nothing, writes nothing.
        work::publish_now(&shared);
        crate::capacity::DRESSED.with(|n| n.set(0));
        let (slots, before) = (sioul_core::attention::Slots::saved_here(), seen());
        let shown = unlocked(&|| made(&shared).0);
        assert!(shown.contains("agenda-job-dentist"), "{shown}");
        assert_eq!(before, seen(), "nothing written, the plan made");
        assert_eq!(sioul_core::attention::Slots::saved_here(), slots, "no slots written, the plan made");
        assert_eq!(crate::capacity::DRESSED.with(std::cell::Cell::get), 0, "today never laid again by the agenda");
        // A write after the plan it reads: the calendars read again, the new event there at once.
        let written = sioul_core::timeline::written();
        vdir::write_item(&calendar.dir.join("visit.ics"), &event("agenda-job-visit", 14)).unwrap();
        assert!(sioul_core::timeline::written() > written, "a calendar write is counted");
        let (shown, _) = made(&shared);
        assert!(shown.contains("agenda-job-visit"), "{shown}");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
    /// A time block is marked with its task as it is now: renamed after the
    /// plan was made, its new title at once (review of 1880bce, later 4).
    #[test]
    fn a_block_is_marked_with_its_task_as_it_is_now() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "Marks test", None, &["VTODO"]).unwrap();
        let (zone, now) = (TimeZone::system(), Zoned::now());
        let write = |title: &str| {
            let edit = sioul_core::tasks::TaskEdit { title: title.into(), ..sioul_core::tasks::TaskEdit::default() };
            vdir::write_item(&list.dir.join("marked.ics"), &sioul_core::tasks::new_task(&edit, "marked-task", &zone, &now).unwrap()).unwrap();
        };
        write("Marked as made");
        let shared = Arc::new(Shared::default());
        work::publish_now(&shared);
        let title = |shared: &Shared| tasks_known(shared).into_iter().find(|t| t.uid == "marked-task").map(|t| t.title);
        assert_eq!(title(&shared).as_deref(), Some("Marked as made"));
        write("Marked as renamed");
        assert_eq!(title(&shared).as_deref(), Some("Marked as renamed"), "the plan older: the tasks read again");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&list.dir);
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
        // The job's own run (review of 85da7f0, later 13): the overlaps, and no days.
        let first = run(&shared);
        assert!(first.overlaps.contains("overlap-dentist") && first.overlaps.contains("overlap-bank"), "{}", first.overlaps);
        assert!(first.days.is_none(), "no days made: the page never asked, no plan yet");
        // From the first plan, the days, again at each run until the window has
        // shown them: a newer run never drops them (review of 85da7f0, later 10).
        work::publish_now(&shared);
        let (second, third) = (run(&shared), run(&shared));
        assert!(second.days.is_some() && third.days.is_some(), "made until shown");
        assert!(third.days.as_ref().unwrap().0 > second.days.as_ref().unwrap().0, "each with its own generation");
        shared.agenda_prefilled.store(true, Ordering::Relaxed);
        assert!(run(&shared).days.is_none(), "shown once: then on the page's ask");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// A heavy week (fifteen events a day, each with a few lines of notes)
    /// stays within the phone's budget for the agenda's JSON: under 150 KB
    /// (the unified agenda, §4.9).
    #[test]
    fn a_heavy_week_stays_within_its_budget() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        // Meals, the night and four dose times a day (the unified agenda, §4.9), the health
        // tests' files held meanwhile: a measure of its own, never another test's.
        let _health_tests = crate::health::health_files_held();
        let health = sioul_core::health::Health::default_path();
        std::fs::create_dir_all(health.parent().unwrap()).unwrap();
        std::fs::write(&health, "[needs]\nmeals_on = true\nsleep_on = true\n\n[[medicine]]\nid = \"heavy\"\nname = \"Heavy\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"12:00\", \"16:00\", \"20:00\"]\n").unwrap();
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
        let began = std::time::Instant::now();
        let (shown, read) = made(&shared);
        let first = began.elapsed();
        // Said for the record (the plan's aim: composed in under 20 ms on a computer, beside the
        // calendars' read): the first made, then again as each minute's turn makes it.
        let again = std::time::Instant::now();
        let _ = made(&shared);
        let took = again.elapsed();
        let ours = shown.matches("\"heavy-").count();
        assert!(ours >= 105, "{ours}");
        eprintln!("sioul-perf: a heavy week's agenda JSON: {} bytes, made in {} ms the first time, {} ms again, of which its calendars' read {} ms", shown.len(), first.as_millis(), took.as_millis(), read.as_millis());
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
        let _ = std::fs::remove_file(&health);
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

    /// The Day and the Week draw every kind (the unified agenda, part 2):
    /// the events, the plan's steps, meals and the night (its parts at the
    /// edges), the doses by key and time, today's plainly being read and a
    /// day to come's plainly ahead, the hours shown and today's head line;
    /// an errand's block named by its kind, never by its prescription, and no
    /// dose by its medicine.
    #[test]
    fn the_day_and_the_week_draw_every_kind() {
        let _health_tests = crate::health::health_files_held();
        sioul_core::config::stand_ins_only();
        let health = sioul_core::health::Health::default_path();
        std::fs::create_dir_all(health.parent().unwrap()).unwrap();
        std::fs::write(&health, "[needs]\nmeals_on = true\nsleep_on = true\n\n[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"20:00\"]\n").unwrap();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Every kind", None, &["VEVENT", "VTODO"]).unwrap();
        let (now, zone) = (Zoned::now(), TimeZone::system());
        let tomorrow = now.date().tomorrow().unwrap();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        std::fs::write(calendar.dir.join("kinds-event.ics"), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:kinds-event\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Kinds appointment\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(10), utc(11))).unwrap();
        let task = |uid: &str, title: &str| {
            let edit = sioul_core::tasks::TaskEdit { title: title.into(), estimate: 30, ..sioul_core::tasks::TaskEdit::default() };
            vdir::write_item(&calendar.dir.join(format!("{uid}.ics")), &sioul_core::tasks::new_task(&edit, uid, &zone, &now).unwrap()).unwrap();
        };
        task("kinds-task", "Kinds step");
        task("kinds-errand", "Pharmacy: Kindsoprazole 20 mg");
        // The errand's block is tomorrow's first thing: the head line names it in the evening.
        std::fs::write(calendar.dir.join("kinds-errand-block.ics"), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:kinds-errand-block\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Pharmacy: Kindsoprazole 20 mg\r\n{}:kinds-errand\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(8), utc(9), sioul_core::blocks::TASK)).unwrap();
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            state.from = now.date();
            state.days = 7;
            state.upcoming = false;
        }
        work::publish_now(&shared);
        let shown = made(&shared).0;
        let parsed: serde_json::Value = serde_json::from_str(&shown).unwrap();
        let days = parsed["days"].as_array().unwrap();
        // A coming day's steps hidden now (quiet time, the night) are drawn as bands, kept apart.
        let kinds: std::collections::BTreeSet<String> = days.iter().flat_map(|d| d["timed"].as_array().cloned().unwrap_or_default().into_iter().chain(d["hidden"].as_array().cloned().unwrap_or_default())).map(|r| r["item"]["kind"].as_str().unwrap_or("").to_string()).collect();
        for kind in ["event", "step", "meal"] {
            assert!(kinds.contains(kind), "{kind} in {kinds:?}");
        }
        assert!(days.iter().all(|d| d["edges"].as_array().is_some_and(|e| !e.is_empty())), "the night's parts on each day");
        let doses = |d: &serde_json::Value| d["doses"].as_array().cloned().unwrap_or_default();
        let dose = tr().text("agenda-dose", None);
        assert!(doses(&days[1]).iter().all(|r| r["item"]["certainty"]["dose"]["state"] == "ahead" && r["item"]["title"] == dose.as_str()), "{}", days[1]["doses"]);
        assert_eq!(doses(&days[1]).len(), 2);
        assert!(doses(&days[0]).iter().all(|r| r["item"]["certainty"]["dose"]["state"] == "reading"));
        assert!(parsed["frame"]["from"].as_i64().is_some_and(|from| from <= 7 * 60), "{}", parsed["frame"]);
        assert!(parsed["head"].as_array().is_none_or(|head| head.iter().all(|f| f["text"].as_str().is_some_and(|t| !t.is_empty()))), "today's head line: {}", parsed["head"]);
        assert!(!shown.contains("Kindsoprazole") && !shown.contains("Lumiel"), "never a prescription's or a medicine's name");
        assert!(shown.contains(&format!("\"{}\"", tr().text("agenda-errand-refill", None))), "the errand by its kind");
        // The head line in the evening, once today holds nothing more: tomorrow's first thing,
        // the errand's block, named by its kind as the timeline names it (review of 3a2fbd4, later 1).
        let evening = now.date().at(21, 0, 0, 0).to_zoned(zone.clone()).unwrap();
        let known = tasks_known(&shared);
        let head = head_frames(&[], work::plan_shown().as_deref(), &[], (0, 0), &known, &|_| false, &evening);
        let said = head.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(" | ");
        assert!(said.contains(&tr().text("agenda-errand-refill", None)), "tomorrow's errand, said: {said}");
        assert!(!said.contains("Kindsoprazole") && !said.contains("Lumiel"), "{said}");
        // In quiet time, a task's block that the task pages hide now is said as work (the proposal, §4.2).
        let at = evening.timestamp().as_second() + 1800;
        let quiet_block = timeline::Item {
            id: "block:quiet".into(),
            kind: timeline::Kind::Block,
            time: timeline::Time::span(at, at + 1800),
            certainty: timeline::Certainty::Held,
            source: timeline::Source::Calendar { key: "/c/quiet.ics".into(), uid: "quiet".into(), start: at, recurring: false, calendar: "c".into(), collection: "c".into() },
            can: timeline::Can::nothing(),
            privacy: timeline::Privacy::Admin,
            title: "Quarterly report".into(),
            line: String::new(),
            said: String::new(),
            tone: timeline::Tone::Plain,
            colour: None,
            address: String::new(),
            owner: "kinds-task".into(),
            holds_time: true,
        };
        let quiet = head_frames(&[quiet_block], work::plan_shown().as_deref(), &[], (0, 0), &known, &|uid| uid == "kinds-task", &evening);
        let said = quiet.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(" | ");
        assert!(said.contains(&tr().text("head-work", None)) && !said.contains("Quarterly report"), "{said}");
        // A calendar that Show hides is left out of it, its first appointment with it.
        {
            let _view = view_held();
            let calendar_name = std::fs::read_to_string(calendar.dir.join("displayname")).unwrap();
            assert_eq!(set_view(&serde_json::json!({ "calendars": [{ "id": format!("{}/{}", calendar.account, calendar.id), "name": calendar_name.trim() }] }).to_string()), "");
            let hidden = head_frames(&[], work::plan_shown().as_deref(), &[], (0, 0), &known, &|_| false, &evening);
            let _ = std::fs::remove_file(view_path());
            let said = hidden.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(" | ");
            assert!(!said.contains(&tr().text("agenda-errand-refill", None)) && !said.contains("Kinds appointment"), "{said}");
        }
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_file(&health);
    }
    /// What comes and the Month say their own (the unified agenda, part 5):
    /// What comes' days carry their rows, today's doses by the minute, unnamed
    /// ("Dose"), another day's usual doses left out, and no `head`/`timed` of
    /// their own; the Month's days carry their lines (appointments), and no
    /// meal, step or dose; neither ever names a medicine.
    #[test]
    fn what_comes_and_the_month_say_their_own() {
        let _health_tests = crate::health::health_files_held();
        sioul_core::config::stand_ins_only();
        let health = sioul_core::health::Health::default_path();
        std::fs::create_dir_all(health.parent().unwrap()).unwrap();
        std::fs::write(&health, "[needs]\nmeals_on = true\nsleep_on = true\n\n[[medicine]]\nid = \"comprimel\"\nname = \"Comprimel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"20:00\"]\n").unwrap();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Coming", None, &["VEVENT"]).unwrap();
        let (now, zone) = (Zoned::now(), TimeZone::system());
        let later = now.date().checked_add(jiff::Span::new().days(3)).unwrap();
        let utc = |hour: i8| later.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        std::fs::write(calendar.dir.join("coming.ics"), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:coming-1\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Coming appointment\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(10), utc(11))).unwrap();
        let shared = Arc::new(Shared::default());
        let shown = |days: i64| {
            if let Ok(mut state) = shared.pim.lock() {
                state.from = now.date();
                state.days = days;
                state.upcoming = days == 15;
            }
            let text = made(&shared).0;
            (serde_json::from_str::<serde_json::Value>(&text).unwrap(), text)
        };
        let (comes, text) = shown(15);
        let days = comes["days"].as_array().unwrap();
        assert!(days.iter().all(|d| d["head"].as_array().is_some_and(Vec::is_empty) && d["timed"].as_array().is_some_and(Vec::is_empty)), "the day's things once");
        let today = &days[0]["comes"];
        let dose = tr().text("agenda-dose", None);
        assert!(today["doses"].as_array().is_some_and(|d| !d.is_empty() && d.iter().all(|l| l["title"] == dose.as_str() && l["keys"].as_array().is_some_and(|k| k.len() == 1))), "today's doses, each its minute: {today}");
        assert!(days[1]["comes"]["doses"].as_array().is_none_or(Vec::is_empty), "another day's usual doses left out");
        assert!(days[3]["comes"]["rows"].as_array().is_some_and(|rows| rows.iter().any(|r| r["item"]["title"] == "Coming appointment" && r["detail"].is_object())), "{}", days[3]["comes"]);
        // Never a medicine's name, in any case, where a thing is said (its id is in `keys` only;
        // review of 21436bf, later 9).
        let named = |v: &serde_json::Value| said(v).iter().filter(|s| s.to_lowercase().contains("comprimel")).cloned().collect::<Vec<_>>();
        assert!(named(&comes).is_empty(), "never a medicine's name: {:?}", named(&comes));
        assert!(!text.contains("Comprimel"));
        let (month, text) = shown(42);
        let days = month["days"].as_array().unwrap();
        let kinds: std::collections::BTreeSet<String> = days.iter().flat_map(|d| d["lines"].as_array().cloned().unwrap_or_default()).map(|r| r["item"]["kind"].as_str().unwrap_or("").to_string()).collect();
        assert!(kinds.contains("event") && kinds.iter().all(|k| k == "event" || k == "block" || k == "dated" || k == "step"), "{kinds:?}");
        assert!(days.iter().all(|d| d["comes"].is_null() && d["timed"].as_array().is_some_and(Vec::is_empty)));
        assert!(!text.contains("Comprimel") && named(&month).is_empty(), "{:?}", named(&month));
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_file(&health);
    }
    /// What the page says of a thing, wherever it sits: titles, lines, times
    /// (`when`), details and the plan's line; never ids or keys.
    fn said(v: &serde_json::Value) -> Vec<String> {
        const SAID: [&str; 8] = ["title", "line", "when", "detail", "plan", "text", "note", "place"];
        match v {
            serde_json::Value::Object(map) => map.iter().flat_map(|(k, v)| match v {
                serde_json::Value::String(s) if SAID.contains(&k.as_str()) => vec![s.clone()],
                _ => said(v),
            }).collect(),
            serde_json::Value::Array(list) => list.iter().flat_map(said).collect(),
            _ => Vec::new(),
        }
    }
    /// Part 6: a leg is a quiet line in its appointment's lead (`leg`, its
    /// appointment's UID its owner), named on the appointment's line, Leave at
    /// from it; Show's choice draws it as an event of its own.
    #[test]
    fn a_leg_is_drawn_in_its_appointment_s_lead() {
        let _view = view_held();
        sioul_core::config::stand_ins_only();
        let _ = std::fs::remove_file(view_path());
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Legs drawn", None, &["VEVENT"]).unwrap();
        let (now, zone) = (Zoned::now(), TimeZone::system());
        let later = now.date().checked_add(jiff::Span::new().days(3)).unwrap();
        let utc = |hour: i8, minute: i8| later.at(hour, minute, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        std::fs::write(calendar.dir.join("legs-doctor.ics"), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:legs-doctor\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Doctor\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(14, 0), utc(14, 30))).unwrap();
        std::fs::write(calendar.dir.join("legs-bus.ics"), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:legs-bus\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Bus 24\r\nRELATED-TO;RELTYPE=PARENT:legs-doctor\r\nX-SIOUL-ANCHOR;GAP=PT25M;WAS={}:BEFORE\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(13, 10), utc(13, 35), utc(14, 0))).unwrap();
        let shared = Arc::new(Shared::default());
        let shown = || {
            if let Ok(mut state) = shared.pim.lock() {
                state.from = later;
                state.days = 1;
                state.upcoming = false;
            }
            serde_json::from_str::<serde_json::Value>(&made(&shared).0).unwrap()
        };
        let day = shown();
        let timed = day["days"][0]["timed"].as_array().unwrap().clone();
        let of = |title: &str| timed.iter().find(|p| p["item"]["title"] == title).cloned().unwrap_or_default();
        assert_eq!((of("Bus 24")["item"]["kind"].as_str(), of("Bus 24")["item"]["owner"].as_str()), (Some("leg"), Some("legs-doctor")), "{timed:?}");
        let clock = |h: i8, m: i8| later.at(h, m, 0, 0).to_zoned(zone.clone()).unwrap().strftime("%H:%M").to_string();
        assert!(of("Doctor")["item"]["line"].as_str().is_some_and(|l| l.contains(&clock(13, 10)) && l.contains("Bus 24")), "Leave at from the bus, the bus named: {}", of("Doctor"));
        assert_eq!(of("Bus 24")["column"], of("Doctor")["column"]);
        // Show: each leg as an event of its own.
        assert_eq!(set_view(r#"{"legs": true}"#), "");
        let day = shown();
        let bus = day["days"][0]["timed"].as_array().unwrap().iter().find(|p| p["item"]["title"] == "Bus 24").cloned().unwrap();
        assert_eq!(bus["item"]["kind"], "event");
        let _ = std::fs::remove_file(view_path());
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// Health's list and the agenda give today's lunch the same time, pushed
    /// past an event at lunch by the one rule (`health::pushed_on`, the needs
    /// adapter); the agenda's Rust says what may move it: a meal over moves
    /// no more (`Move::None`), and its writer refuses a day gone by (review of
    /// 21436bf, later 7 and later 9).
    #[test]
    fn health_s_list_and_the_agenda_give_lunch_one_time() {
        let _health_tests = crate::health::health_files_held();
        sioul_core::config::stand_ins_only();
        work::forget_plan();
        let health = sioul_core::health::Health::default_path();
        std::fs::create_dir_all(health.parent().unwrap()).unwrap();
        std::fs::write(&health, "[needs]\nmeals_on = true\n").unwrap();
        let (now, zone) = (Zoned::now(), TimeZone::system());
        let later = now.date().checked_add(jiff::Span::new().days(2)).unwrap();
        let at = |hour: i8, minute: i8| later.at(hour, minute, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        // An appointment across the usual lunch.
        let event = Occurrence { key: "/c/lunch-meeting.ics".into(), uid: "lunch-meeting".into(), summary: "Meeting".into(), start: at(12, 0), end: at(13, 30), ..Occurrence::default() };
        let needs = sioul_core::needs::Needs::read(&health);
        let days = sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path());
        let agenda_lunch = needs_items(&needs, &days, None, std::slice::from_ref(&event), later, 1, &now).into_iter().find(|i| i.id == timeline::need_id(later, "meal:1")).map(|i| i.time.whole(&zone).0);
        let card: serde_json::Value = serde_json::from_str(&crate::health::need_item_with(&later.to_string(), "meal:1", &[event.clone()])).unwrap();
        let clock = |t: i64| jiff::Timestamp::from_second(t).unwrap().to_zoned(zone.clone()).strftime("%H:%M").to_string();
        assert!(agenda_lunch.is_some_and(|t| t >= at(13, 30)), "pushed past the meeting: {agenda_lunch:?}");
        assert_eq!(card["item"]["from"].as_str(), agenda_lunch.map(clock).as_deref(), "{card}");
        // Today, both read the plan's pushes (`Settings::shifts`): a re-plan that pushes a meal
        // otherwise shows Health's list again (`work::health_follows`), as it does the agenda.
        let pushed: std::collections::BTreeMap<String, i64> = [("meal:1".to_string(), 90)].into();
        assert!(work::health_follows(Some(&std::collections::BTreeMap::new()), &pushed), "a meal pushed by the new plan");
        assert!(work::health_follows(None, &pushed), "no plan of today before");
        assert!(!work::health_follows(Some(&pushed), &pushed.clone()), "pushed as before: not again");
        // Over, its time cannot be moved here.
        let after = later.at(23, 0, 0, 0).to_zoned(zone.clone()).unwrap();
        let over = needs_items(&needs, &days, None, &[], later, 1, &after).into_iter().find(|i| i.id == timeline::need_id(later, "meal:1")).unwrap();
        assert_eq!(over.can.drag, timeline::Move::None, "a meal over moves no more");
        // A day gone by: its writer refuses the drop, and the day's file stays as it was.
        let yesterday = now.date().yesterday().unwrap();
        let days_file = sioul_core::needs::Days::default_path();
        let before = std::fs::read(&days_file).ok();
        let asked = serde_json::json!({ "date": yesterday.to_string(), "key": "meal:1", "action": "move", "from": "14:00", "to": "14:30" }).to_string();
        assert_eq!(crate::health::change_need(&asked), tr().text("need-past", None));
        assert_eq!(std::fs::read(&days_file).ok(), before, "nothing written for a day gone by");
        let _ = std::fs::remove_file(&health);
    }
}

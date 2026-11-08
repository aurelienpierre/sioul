// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! One search over tasks and events, with filters (8 November 2026): the
//! Search tab of the Tasks and the Agenda views and `sioul tasks search` ask
//! this engine, which reads nothing itself.
//!
//! - **Words** are looked for in the title, the notes and the place, each
//!   word somewhere, accents and case aside (`text::fold`).
//! - **Project, kind, category**: any (""), none (`NONE`), or one. An event
//!   has no kind: "no kind" holds every event, a kind none.
//! - **Dates**: anytime, or a range of days, either end open. A task falls in
//!   it by its date asked, its day to start or the day it was done; an event
//!   by its time, a repeating one by its occurrences in the range (the
//!   caller reads them, `agenda::occurrences`).
//! - **Events**: with guests or not (an ATTENDEE that is not one of your own
//!   addresses), status (confirmed, to be confirmed, cancelled).
//! - **Tasks**: done or not, with a date asked or not.
//!
//! Each thing found once, with the time it is listed at: what comes first,
//! soonest first, then the past, newest first, then what has no date.

use crate::agenda::{EventRef, Occurrence};
use crate::i18n::Translator;
use crate::projects::Project;
use crate::tasks::{Status, Task};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The choice "none" of a project, a kind or a category: things without one.
pub const NONE: &str = ":none";

/// What is searched.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Which {
    #[default]
    Both,
    Tasks,
    Events,
}

/// An event's status, as RFC 5545 §3.8.1.11 says it; none written is confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

impl EventStatus {
    pub fn of(event: &EventRef) -> EventStatus {
        if event.cancelled {
            EventStatus::Cancelled
        } else if event.tentative {
            EventStatus::Tentative
        } else {
            EventStatus::Confirmed
        }
    }
}

/// What is asked. Empty, everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Query {
    pub words: String,
    pub which: Which,
    /// A project's id, `NONE`, or "" for any.
    pub project: String,
    /// One of `tasks::KINDS`, `NONE`, or "".
    pub kind: String,
    /// A category, `NONE`, or "".
    pub category: String,
    /// The first day, "2026-11-01"; "" for no start.
    pub from: String,
    /// The last day, included; "" for no end.
    pub to: String,
    /// Events with guests (true), without (false), or both.
    pub guests: Option<bool>,
    /// The statuses wanted; none, all of them.
    pub status: Vec<EventStatus>,
    /// Tasks done (true), not done (false), or both.
    pub done: Option<bool>,
    /// Tasks with a date asked (true), without (false), or both.
    pub deadline: Option<bool>,
}

impl Query {
    /// The range of days asked, as Unix seconds from the first day's midnight
    /// to the midnight after the last; None for anytime.
    pub fn range(&self, zone: &TimeZone) -> Option<(i64, i64)> {
        let from = self.from.trim().parse::<Date>().ok();
        let to = self.to.trim().parse::<Date>().ok();
        if from.is_none() && to.is_none() {
            return None;
        }
        let midnight = |d: Date| d.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second());
        let start = from.map_or(i64::MIN, midnight);
        let end = to.and_then(|d| d.tomorrow().ok()).map_or(i64::MAX, midnight);
        Some((start, end))
    }

    /// Whether the search asks anything of events only (guests, status) or of tasks only (done, a date asked).
    fn wants_tasks(&self) -> bool {
        self.which != Which::Events && self.guests.is_none() && self.status.is_empty()
    }

    fn wants_events(&self) -> bool {
        self.which != Which::Tasks && self.done.is_none() && self.deadline.is_none()
    }
}

/// Where a thing found is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    Coming,
    Past,
    Undated,
}

/// A thing found, as the lists show it.
#[derive(Debug, Clone, Serialize)]
pub struct Found {
    /// "task" or "event".
    pub thing: &'static str,
    pub key: String,
    pub uid: String,
    pub title: String,
    pub place: String,
    /// The time it is listed at (Unix seconds): an event's start, its first
    /// occurrence in the range for a repeating one; a task's date asked, else
    /// its day to start, else the day it was done. None without a date.
    pub at: Option<i64>,
    pub all_day: bool,
    pub when: When,
    /// Its time in words: "Thu 12 Nov, 14:00", "Thu 12 Nov 2027".
    pub date: String,
    pub project_ids: Vec<String>,
    pub projects: Vec<String>,
    /// A task's kind; "" for none and for every event.
    pub kind: String,
    pub categories: Vec<String>,
    /// A task's status ("needs-action", "in-process", "completed", "cancelled"),
    /// an event's ("confirmed", "tentative", "cancelled").
    pub status: String,
    /// A task done or dropped.
    pub done: bool,
    /// A task's date asked as written ("2026-11-12", "2026-11-12T17:00"); "" for none.
    pub due: String,
    /// An event with guests other than you.
    pub guests: bool,
    pub recurring: bool,
    /// Its list or calendar, by place ("account/id") when known.
    pub list_id: String,
    pub list: String,
    pub read_only: bool,
}

/// What a search reads besides the things: your addresses (a guest that is
/// you is no guest), the projects' names, the zone and today.
pub struct Context<'a> {
    pub own: &'a [String],
    pub projects: &'a [Project],
    pub zone: &'a TimeZone,
    pub today: Date,
    pub tr: &'a Translator,
}

/// The things asked, each once, in the order lists show them (`When`, then
/// by time). `occurrences`, read by the caller for the range asked, places
/// repeating events; without them a repeating event is placed by its first time.
pub fn search(tasks: &[Task], events: &[EventRef], occurrences: Option<&[Occurrence]>, query: &Query, cx: &Context) -> Vec<Found> {
    let words: Vec<String> = query.words.split_whitespace().map(folded).collect();
    let has_words = |title: &str, notes: &str, place: &str| words.is_empty() || {
        let all = folded(&format!("{title}\n{notes}\n{place}"));
        words.iter().all(|w| all.contains(w.as_str()))
    };
    let range = query.range(cx.zone);
    let now = cx.today.to_zoned(cx.zone.clone()).map_or(0, |z| z.timestamp().as_second());
    let mut out: Vec<Found> = Vec::new();
    if query.wants_tasks() {
        for task in tasks {
            let kinds: Vec<String> = Some(task.kind.clone()).filter(|k| !k.is_empty()).into_iter().collect();
            if !has_words(&task.title, &task.notes, &task.location) || !picks(&query.project, &task.projects) || !picks(&query.kind, &kinds) || !picks_folded(&query.category, &task.categories) {
                continue;
            }
            let done = !task.status.is_open();
            if query.done.is_some_and(|d| d != done) || query.deadline.is_some_and(|d| d != task.due_date().is_some()) {
                continue;
            }
            let day = |text: &str| crate::tasks::day_of(text);
            let days: Vec<Date> = [day(&task.due), day(&task.start), task.completed.and_then(|c| jiff::Timestamp::from_second(c).ok()).map(|t| t.to_zoned(cx.zone.clone()).date())].into_iter().flatten().collect();
            let midnight = |d: Date| d.to_zoned(cx.zone.clone()).map_or(0, |z| z.timestamp().as_second());
            if let Some((from, to)) = range
                && !days.iter().any(|d| (from..to).contains(&midnight(*d)))
            {
                continue;
            }
            // Listed at its date asked, with its hour when it has one.
            let at = task.due_at(cx.zone).or_else(|| days.first().map(|d| midnight(*d)));
            let all_day = task.due_at(cx.zone).is_none();
            out.push(Found {
                thing: "task",
                key: task.key.clone(),
                uid: task.uid.clone(),
                title: task.title.clone(),
                place: task.location.clone(),
                at,
                all_day,
                when: when_of(at, now),
                date: at.map(|a| date_text(a, all_day, cx)).unwrap_or_default(),
                project_ids: task.projects.clone(),
                projects: project_titles(&task.projects, cx.projects),
                kind: task.kind.clone(),
                categories: task.categories.clone(),
                status: task_status(task.status).to_string(),
                done,
                due: task.due.clone(),
                guests: false,
                recurring: !task.repeat.is_empty(),
                list_id: task.list_id.clone(),
                list: task.list.clone(),
                read_only: task.read_only,
            });
        }
    }
    if query.wants_events() {
        // A repeating event's first occurrence in the range, by its file.
        let mut first: BTreeMap<&str, (i64, bool)> = BTreeMap::new();
        for o in occurrences.unwrap_or_default() {
            if range.is_none_or(|(from, to)| o.end > from && o.start < to) {
                first.entry(o.key.as_str()).and_modify(|e| if o.start < e.0 { *e = (o.start, o.all_day) }).or_insert((o.start, o.all_day));
            }
        }
        for event in events {
            // A task's time block is its task's time, never an event of its own.
            if !event.task.is_empty() {
                continue;
            }
            if !has_words(&event.summary, &event.notes, &event.location) || !picks(&query.project, &event.projects) || !picks(&query.kind, &[]) || !picks_folded(&query.category, &event.categories) {
                continue;
            }
            let guests = event.guests.iter().any(|g| !cx.own.iter().any(|o| o.eq_ignore_ascii_case(g)));
            let status = EventStatus::of(event);
            if query.guests.is_some_and(|g| g != guests) || (!query.status.is_empty() && !query.status.contains(&status)) {
                continue;
            }
            let (at, all_day) = match (range, event.recurring && occurrences.is_some()) {
                (_, true) => match first.get(event.key.as_str()) {
                    Some((start, all_day)) => (Some(*start), *all_day),
                    None if range.is_some() => continue,
                    None => (Some(event.start).filter(|s| *s != 0), event.all_day),
                },
                (Some((from, to)), false) => {
                    let end = event.end.max(event.start + 1);
                    if event.start == 0 || !(end > from && event.start < to) {
                        continue;
                    }
                    (Some(event.start), event.all_day)
                }
                (None, false) => (Some(event.start).filter(|s| *s != 0), event.all_day),
            };
            out.push(Found {
                thing: "event",
                key: event.key.clone(),
                uid: event.uid.clone(),
                title: event.summary.clone(),
                place: event.location.clone(),
                at,
                all_day,
                when: when_of(at, now),
                date: at.map(|a| date_text(a, all_day, cx)).unwrap_or_default(),
                project_ids: event.projects.clone(),
                projects: project_titles(&event.projects, cx.projects),
                kind: String::new(),
                categories: event.categories.clone(),
                status: match status {
                    EventStatus::Confirmed => "confirmed",
                    EventStatus::Tentative => "tentative",
                    EventStatus::Cancelled => "cancelled",
                }
                .to_string(),
                done: false,
                due: String::new(),
                guests,
                recurring: event.recurring,
                list_id: String::new(),
                list: String::new(),
                read_only: false,
            });
        }
    }
    out.sort_by(|a, b| {
        a.when.cmp(&b.when).then_with(|| match a.when {
            When::Coming => a.at.cmp(&b.at),
            When::Past => b.at.cmp(&a.at),
            When::Undated => std::cmp::Ordering::Equal,
        })
        .then_with(|| folded(&a.title).cmp(&folded(&b.title)))
    });
    out
}

/// The text folded for comparison: accents and case aside.
fn folded(text: &str) -> String {
    crate::text::fold(text).into_iter().collect()
}

/// Whether `values` hold what `pick` asks: anything (""), nothing (`NONE`), or that one.
fn picks(pick: &str, values: &[String]) -> bool {
    match pick {
        "" => true,
        NONE => values.is_empty(),
        one => values.iter().any(|v| v == one),
    }
}

/// The same, names compared accents and case aside (categories).
fn picks_folded(pick: &str, values: &[String]) -> bool {
    match pick {
        "" | NONE => picks(pick, values),
        one => values.iter().any(|v| folded(v) == folded(one)),
    }
}

/// From today's midnight on, it comes: today's whole-day things and today's dates too.
fn when_of(at: Option<i64>, today_midnight: i64) -> When {
    match at {
        None => When::Undated,
        Some(at) if at >= today_midnight => When::Coming,
        Some(_) => When::Past,
    }
}

fn task_status(status: Status) -> &'static str {
    match status {
        Status::NeedsAction => "needs-action",
        Status::InProcess => "in-process",
        Status::Completed => "completed",
        Status::Cancelled => "cancelled",
    }
}

fn project_titles(ids: &[String], projects: &[Project]) -> Vec<String> {
    ids.iter().map(|id| projects.iter().find(|p| &p.id == id).map_or_else(|| id.clone(), |p| p.title.clone())).collect()
}

/// "Thu 12 Nov, 14:00", its year when not this one; a whole day without its hour.
fn date_text(at: i64, all_day: bool, cx: &Context) -> String {
    let Ok(stamp) = jiff::Timestamp::from_second(at) else { return String::new() };
    let zoned = stamp.to_zoned(cx.zone.clone());
    let mut day = crate::moneyview::short_day(cx.tr, zoned.date());
    if zoned.date().year() != cx.today.year() {
        day = format!("{day} {}", zoned.date().year());
    }
    if all_day { day } else { format!("{day}, {}", zoned.strftime("%H:%M")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> i64 {
        text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone()).unwrap().timestamp().as_second()
    }

    fn task(uid: &str, title: &str) -> Task {
        Task { uid: uid.into(), key: format!("/stand-in/tasks/{uid}.ics"), title: title.into(), ..Task::default() }
    }

    fn event(uid: &str, title: &str, start: &str) -> EventRef {
        EventRef { uid: uid.into(), key: format!("/stand-in/cal/{uid}.ics"), summary: title.into(), start: at(start), end: at(start) + 3_600, ..EventRef::default() }
    }

    fn fixture() -> (Vec<Task>, Vec<EventRef>) {
        let tasks = vec![
            Task { notes: "Le formulaire de la CAF".into(), projects: vec!["home".into()], kind: "write".into(), categories: vec!["Papiers".into()], due: "2026-11-12".into(), ..task("caf", "Fill the form") },
            Task { kind: "call".into(), start: "2026-11-20".into(), ..task("call", "Call the bank") },
            Task { status: Status::Completed, completed: Some(at("2026-11-02T10:00")), projects: vec!["home".into()], ..task("done", "Water the plants") },
            task("undated", "Read the book"),
        ];
        let events = vec![
            EventRef { location: "Clinique du Parc".into(), guests: vec!["you@example.org".into()], projects: vec!["home".into()], ..event("dentist", "Dentist", "2026-11-10T14:00") },
            EventRef { guests: vec!["camille@example.org".into(), "you@example.org".into()], tentative: true, categories: vec!["Travail".into()], ..event("meeting", "Team meeting", "2026-11-16T09:00") },
            EventRef { cancelled: true, notes: "Rendez-vous annulé".into(), ..event("old", "Hairdresser", "2026-10-01T11:00") },
            EventRef { task: "caf".into(), ..event("block", "Fill the form", "2026-11-09T09:00") },
        ];
        (tasks, events)
    }

    fn found(query: &Query) -> Vec<String> {
        let (tasks, events) = fixture();
        let projects = [Project { id: "home".into(), title: "Home".into(), ..Project::default() }];
        let tr = Translator::new("en");
        let own = ["you@example.org".to_string()];
        let cx = Context { own: &own, projects: &projects, zone: &zone(), today: "2026-11-08".parse().unwrap(), tr: &tr };
        search(&tasks, &events, None, query, &cx).into_iter().map(|f| f.uid).collect()
    }

    /// Everything, in the lists' order: what comes, soonest first; the past,
    /// newest first; what has no date. A task's time block is its task's.
    #[test]
    fn everything_comes_first_then_the_past_then_the_undated() {
        assert_eq!(found(&Query::default()), ["dentist", "caf", "meeting", "call", "done", "old", "undated"]);
    }

    /// Words, each somewhere in the title, the notes or the place, accents and case aside.
    #[test]
    fn words_are_found_in_titles_notes_and_places() {
        let words = |w: &str| found(&Query { words: w.into(), ..Query::default() });
        assert_eq!(words("caf formulaire"), ["caf"], "in the notes");
        assert_eq!(words("clinique"), ["dentist"], "in the place");
        assert_eq!(words("ANNULE"), ["old"], "accents and case aside");
        assert!(words("caf nothing").is_empty(), "every word");
    }

    /// Project, kind and category: any, one, or explicitly none.
    #[test]
    fn project_kind_and_category_each_have_none() {
        let q = |project: &str, kind: &str, category: &str| found(&Query { project: project.into(), kind: kind.into(), category: category.into(), ..Query::default() });
        assert_eq!(q("home", "", ""), ["dentist", "caf", "done"]);
        assert_eq!(q(NONE, "", ""), ["meeting", "call", "old", "undated"]);
        assert_eq!(q("", "call", ""), ["call"], "an event has no kind");
        assert_eq!(q("", NONE, ""), ["dentist", "meeting", "done", "old", "undated"], "no kind: events too");
        assert_eq!(q("", "", "papiers"), ["caf"], "a category, case aside");
        assert_eq!(q("", "", NONE).len(), 5);
    }

    /// A range of days: a task by its date asked, its day to start or its day
    /// done; an event by its time. Either end may stay open.
    #[test]
    fn dates_anytime_or_a_range() {
        let range = |from: &str, to: &str| found(&Query { from: from.into(), to: to.into(), ..Query::default() });
        assert_eq!(range("2026-11-10", "2026-11-12"), ["dentist", "caf"], "the last day included");
        assert_eq!(range("", "2026-11-05"), ["done", "old"], "no start");
        assert_eq!(range("2026-11-16", ""), ["meeting", "call"], "no end");
    }

    /// Events: with guests other than you, or not; by status. Asking those
    /// leaves tasks out, as asking a task's done or date asked leaves events out.
    #[test]
    fn events_by_guests_and_status_tasks_by_done_and_date_asked() {
        assert_eq!(found(&Query { guests: Some(true), ..Query::default() }), ["meeting"], "you alone are no guest");
        assert_eq!(found(&Query { guests: Some(false), ..Query::default() }), ["dentist", "old"]);
        assert_eq!(found(&Query { status: vec![EventStatus::Tentative, EventStatus::Cancelled], ..Query::default() }), ["meeting", "old"]);
        assert_eq!(found(&Query { status: vec![EventStatus::Confirmed], ..Query::default() }), ["dentist"]);
        assert_eq!(found(&Query { done: Some(true), ..Query::default() }), ["done"]);
        assert_eq!(found(&Query { done: Some(false), deadline: Some(true), ..Query::default() }), ["caf"]);
        assert_eq!(found(&Query { deadline: Some(false), which: Which::Tasks, ..Query::default() }), ["call", "done", "undated"]);
        assert_eq!(found(&Query { which: Which::Events, ..Query::default() }), ["dentist", "meeting", "old"]);
    }

    /// A repeating event, its occurrences read for the range: found by the
    /// first in it, left out when none falls in it.
    #[test]
    fn a_repeating_event_by_its_occurrences_in_the_range() {
        let (tasks, mut events) = fixture();
        events.push(EventRef { recurring: true, ..event("yoga", "Yoga", "2026-01-05T18:00") });
        let occurrence = |start: &str| Occurrence { key: "/stand-in/cal/yoga.ics".into(), uid: "yoga".into(), start: at(start), end: at(start) + 3_600, recurring: true, ..Occurrence::default() };
        let read = [occurrence("2026-11-09T18:00"), occurrence("2026-11-16T18:00")];
        let tr = Translator::new("en");
        let cx = Context { own: &[], projects: &[], zone: &zone(), today: "2026-11-08".parse().unwrap(), tr: &tr };
        let query = Query { words: "yoga".into(), from: "2026-11-10".into(), to: "2026-11-20".into(), ..Query::default() };
        let hits = search(&tasks, &events, Some(&read), &query, &cx);
        assert_eq!((hits.len(), hits[0].at), (1, Some(at("2026-11-16T18:00"))));
        assert_eq!(hits[0].date, "Mon 16 Nov, 18:00");
        let none = Query { from: "2026-12-01".into(), to: "2026-12-02".into(), ..query };
        assert!(search(&tasks, &events, Some(&read), &none, &cx).is_empty());
    }

    /// The query as the window and the agents send it, in JSON.
    #[test]
    fn the_query_reads_from_json() {
        let query: Query = serde_json::from_str(r#"{"words":"caf","which":"tasks","project":":none","status":["tentative"],"done":false}"#).unwrap();
        assert_eq!((query.which, query.project.as_str(), query.status.as_slice(), query.done, query.guests), (Which::Tasks, NONE, &[EventStatus::Tentative][..], Some(false), None));
    }
}

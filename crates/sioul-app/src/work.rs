// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Tasks, notes, links and focus behind the window (docs/tasks.md).
//!
//! Tasks are written to their list's folder at once, then the account's sync
//! sends them, as contacts and events are. The plan is computed again after
//! every change, on a thread; the window only lays out what the core worded.
//! Everything links reach (tasks, events, contacts, notes, drafts, mail
//! headers) is read once per change and kept, so the Related panel answers at
//! once.

use crate::backend::{QtThread, Shared, load_config, say, tell, tr};
use crate::{mail, pim};
use cxx_qt_lib::QString;
use jiff::tz::TimeZone;
use jiff::civil::Date;
use jiff::{Timestamp, Zoned};
use serde::Serialize;
use sioul_core::capture;
use sioul_core::card::Card;
use sioul_core::projects::ProjectStore;
use sioul_core::compose::Draft;
use sioul_core::links::{self, Kind as LinkKind, Loaded, LocalLinks};
use sioul_core::notes;
use sioul_core::plan::{self, Plan, Settings};
use sioul_core::taskview::{self, BoardView, Context, Filter, ListView, NowView, TimelineView};
use sioul_core::tasks::{self, ContactRef, Link, Status, Task, TaskEdit};
use sioul_core::timelog::{self, Running};
use sioul_core::today::{Today, Weather};
use sioul_core::vdir::{self, Collection, Kind};
use sioul_core::{agenda, contacts, lines, maildir};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// Where new notes go in the notes folder, unless the configuration says otherwise.
const NOTES_FOLDER: &str = "notes";

/// What the task pages ask for: the list's grouping, done tasks, a search, one
/// project, one kind and category. All but the searches are kept between sessions.
#[derive(Clone, Default, Serialize, serde::Deserialize)]
pub(crate) struct WorkState {
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub done: bool,
    #[serde(skip)]
    pub query: String,
    /// One project; `case`, its name in a file kept before, reads too.
    #[serde(default, alias = "case")]
    pub project: String,
    #[serde(default)]
    pub filter: Filter,
    #[serde(skip)]
    pub notes_query: String,
    /// Notes as a tree of folders; else one list.
    #[serde(default)]
    pub notes_tree: bool,
    /// How other pages were left: a column folded, a section open ("sites-narrow"…).
    #[serde(default)]
    pub flags: std::collections::BTreeMap<String, bool>,
    /// The tasks with steps opened in List, Board and the Gantt, by UID: the
    /// others are one row each (`taskview::Fold`, composed tasks §7.2). Kept
    /// here, never in the task.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub opened: BTreeSet<String>,
}

impl WorkState {
    fn path() -> PathBuf {
        sioul_core::config::state_dir().join("tasks-view.toml")
    }

    /// The choices as you left them.
    pub(crate) fn load() -> WorkState {
        std::fs::read_to_string(WorkState::path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&self) {
        let path = WorkState::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = toml::to_string(self) {
            let _ = std::fs::write(&path, text);
        }
    }
}

/// Everything read, kept until the next change.
pub(crate) fn loaded(shared: &Shared) -> Arc<Loaded> {
    if let Some(found) = shared.loaded.lock().ok().and_then(|l| l.clone()) {
        return found;
    }
    let fresh = Arc::new(read_loaded(shared));
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = Some(Arc::clone(&fresh));
    }
    fresh
}

#[cfg(test)]
thread_local! {
    /// The reads of everything made on this thread (`read_loaded`): a job
    /// that reads the tasks alone is checked by them, never by the files
    /// other tests write meanwhile (the agenda's job, review of 1880bce, L9).
    pub(crate) static LOADS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Reads everything again, leaving out what waits ten seconds to be deleted.
fn read_loaded(shared: &Shared) -> Loaded {
    #[cfg(test)]
    LOADS.with(|n| n.set(n.get() + 1));
    let mut fresh = Loaded::read(&load_config());
    let (removed, _) = mail::hidden_pim(shared);
    fresh.tasks.retain(|t| !removed.contains(Path::new(&t.key)));
    // An event deleted, a task's time block left to the plan: gone from what things are tied to, too.
    fresh.events.retain(|e| !removed.contains(Path::new(&e.key)));
    fresh
}

/// The room for tasks (`plan::settings_from`): read as the window reads it
/// (its blocks kept in memory, the days' own meals as the Health page keeps
/// them), then made as an agent's server makes it, in one place. `read` is
/// the plan's one read of the calendars (`Desk::new`).
fn settings(weather: Weather, situation: &sioul_core::quiet::Situation, loaded: &Loaded, shared: &Shared, read: &[sioul_core::agenda::Occurrence]) -> Settings {
    let config = load_config();
    let now = Zoned::now();
    // The tasks pinned to a time: a block is its task's time, never an event of its own (docs/tasks.md, "Pinned to a time").
    let pins = crate::blocks::pins(shared, read, &loaded.tasks, &now);
    // Meals, naps and the night, as the health file says them, never a medicine; a file
    // lost or that does not read: as its last good read said them (`Needs::read`).
    let needs = sioul_core::needs::Needs::read(&sioul_core::health::Health::default_path());
    sioul_core::plan::settings_from(sioul_core::plan::Inputs {
        config: &config,
        now: &now,
        read,
        pins,
        needs: &needs,
        days: crate::health::days(),
        hour_blocks: crate::hours::blocks(&now),
        weather,
        situation,
        projects: &loaded.projects,
        tasks: &loaded.tasks,
        // The events there are: a task that prepares one gone waits for a date (`pairs`).
        appointments: Some(sioul_core::pairs::appointments_here(&loaded.events, now.time_zone())),
    })
}

/// Asleep, the tasks all the same: the page asked ("Show anyway"), until it is left.
static SHOWN_ANYWAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn show_anyway(qt: &QtThread, shared: &Arc<Shared>, on: bool) {
    if SHOWN_ANYWAY.swap(on, Ordering::Relaxed) != on {
        show_work(qt, shared);
    }
}

/// The moment, for the task pages: quiet or not, offices open or not.
pub(crate) fn situation(projects: &[sioul_core::projects::Project]) -> sioul_core::quiet::Situation {
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let now = Zoned::now();
    sioul_core::quiet::Situation::now(&load_config(), &overrides, &crate::hours::blocks(&now), &now, tr(), projects)
}

/// The plan, and what it needs, from what was read.
pub(crate) struct Desk {
    loaded: Arc<Loaded>,
    /// The calendars from today's midnight, the plan's horizon ahead
    /// (`plan::HORIZON_DAYS`), read once: the plan, today's layout and the
    /// agenda's coming weeks all read these. Kept as read: those whose
    /// deletion waits ("Undo") are left out at each use, so that an Undo
    /// brings them back at once (review of 1880bce, L1).
    read: Arc<Vec<sioul_core::agenda::Occurrence>>,
    filter: Filter,
    offices: taskview::Offices,
    plan: Plan,
    today: Today,
    sessions: Vec<timelog::Session>,
    spent: BTreeMap<String, u32>,
    stopped: BTreeMap<String, String>,
    settings: Settings,
    /// Work rests now (quiet time); the filter alone says only that hours are set.
    quiet: bool,
    /// The appointments and their legs (`pairs::outings`): what a step comes before.
    outings: BTreeMap<String, sioul_core::pairs::Outing>,
}

impl Desk {
    fn new(loaded: Arc<Loaded>, shared: &Shared) -> Desk {
        let now = Zoned::now();
        let date = now.date();
        let today = Today::load(&Today::default_path(), date);
        let sessions = timelog::sessions();
        let spent = timelog::spent(&sessions, 0, i64::MAX);
        let stopped = sessions.iter().filter(|s| !s.note.is_empty()).map(|s| (s.task.clone(), s.note.clone())).collect();
        let situation = situation(&loaded.projects);
        // The plan's one read of the calendars (`read`), kept as read; the plan leaves out what waits for Undo.
        let midnight = date.to_zoned(now.time_zone().clone()).map_or(0, |z| z.timestamp().as_second());
        let read = Arc::new(sioul_core::agenda::occurrences(midnight, midnight + sioul_core::plan::HORIZON_DAYS * 86_400));
        let (removed, skipped) = mail::hidden_pim(shared);
        let live = pim::without_waiting(read.as_ref().clone(), &removed, &skipped);
        // What your record says: ratings said after tasks, corrected lengths, what a day holds (docs/capacity.md).
        let settings = crate::capacity::planned(settings(today.weather, &situation, &loaded, shared, &live), &loaded.tasks, &sessions);
        let plan = plan::plan(&loaded.tasks, date, &settings, &spent, &today.aside);
        // Asleep no task shows, unless the page asked to see them all.
        let anyway = situation.mode.sleeps() && SHOWN_ANYWAY.load(Ordering::Relaxed);
        let filter = Filter { quiet: if anyway { None } else { situation.quiet_tasks() }, ..Filter::default() };
        let quiet = situation.mode.quiet;
        let outings = sioul_core::pairs::outings(&loaded.events);
        Desk { loaded, read, filter, offices: situation.offices, plan, today, sessions, spent, stopped, settings, quiet, outings }
    }

    /// "Until Thursday 8 October: about 2 h of steps, 6 h of room.", for a date asked.
    fn budget_line(&self, date: jiff::civil::Date) -> String {
        let today = Zoned::now().date();
        let budget = plan::cushion(&self.loaded.tasks, &self.plan, &self.settings, today, date);
        if budget.need == 0 {
            return String::new();
        }
        let span = |minutes: u32| match (minutes / 60, minutes % 60) {
            (0, m) => format!("{m} min"),
            (h, 0) => format!("{h} h"),
            (h, m) => format!("{h} h {m:02}"),
        };
        let id = if budget.need > budget.room { "time-budget-short" } else { "time-budget" };
        say(id, &[("date", tr().day(date)), ("need", span(budget.need)), ("room", span(budget.room))])
    }

    /// The next date asked within a week, among the tasks shown, and its time budget.
    fn next_budget(&self) -> String {
        let today = Zoned::now().date();
        let week = today.checked_add(jiff::Span::new().days(7)).unwrap_or(today);
        let shown = |t: &&Task| self.filter.quiet.as_ref().is_none_or(|q| q.keeps(t));
        let next = self.loaded.tasks.iter().filter(|t| t.status.is_open()).filter(shown).filter(|t| self.plan.items.get(&t.uid).is_some_and(|p| !p.optional)).filter_map(|t| t.due_date()).filter(|d| *d >= today && *d <= week).min();
        next.map(|date| self.budget_line(date)).unwrap_or_default()
    }

    fn context(&self) -> Context<'_> {
        self.context_with(&self.filter)
    }

    /// The task views' context, through `filter` (the page's choices).
    fn context_with<'a>(&'a self, filter: &'a Filter) -> Context<'a> {
        Context { filter, offices: self.offices.clone(), tasks: &self.loaded.tasks, plan: &self.plan, today: Zoned::now().date(), tr: tr(), projects: &self.loaded.projects, spent: &self.spent, stopped: &self.stopped, outings: &self.outings }
    }

    fn task(&self, uid: &str) -> Option<&Task> {
        self.loaded.tasks.iter().find(|t| t.uid == uid)
    }
}

#[derive(Serialize)]
pub(crate) struct ListChoice {
    pub(crate) id: String,
    name: String,
    color: Option<String>,
    /// Waits to be created on the server.
    pending: bool,
    /// It keeps tasks; a calendar that does not is shown, greyed, with why.
    pub(crate) tasks: bool,
}

#[derive(Serialize)]
struct ProjectChoice {
    id: String,
    title: String,
}

#[derive(Serialize)]
struct KindChoice {
    id: String,
    label: String,
}

/// The projects a task can go in: those not closed.
fn projects_offered(loaded: &Loaded) -> Vec<ProjectChoice> {
    loaded.projects.iter().filter(|c| c.status.as_deref() != Some("closed")).map(|c| ProjectChoice { id: c.id.clone(), title: c.title.clone() }).collect()
}

fn kinds_offered() -> Vec<KindChoice> {
    load_config().task_kinds(tr()).into_iter().map(|(id, label)| KindChoice { id, label }).collect()
}

/// The open tasks' categories, the most used first, each as first written.
fn categories_of(tasks: &[Task]) -> Vec<String> {
    let mut counted: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for task in tasks.iter().filter(|t| t.status.is_open()) {
        for category in &task.categories {
            counted.entry(category.to_lowercase()).or_insert((0, category.clone())).0 += 1;
        }
    }
    let mut categories: Vec<(usize, String)> = counted.into_values().collect();
    categories.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    categories.into_iter().map(|(_, c)| c).collect()
}

/// Everything as the plan last read it, else the last read kept; none before
/// the first read: what a card reads on the window's thread, never the stores.
pub(crate) fn loaded_known(shared: &Shared) -> Option<Arc<Loaded>> {
    plan_shown().map(|p| Arc::clone(&p.desk.loaded)).or_else(|| shared.loaded.lock().ok().and_then(|l| l.clone()))
}

/// What a task's card offers to choose from, wherever it opens (the card
/// host, the unified agenda, part 3): `{projects, kinds, categories}`, as the
/// Tasks page's JSON gives them, from the plan as last made or the last read;
/// never a read of the stores on the window's thread (empty lists before the
/// first read).
pub(crate) fn task_choices(shared: &Shared) -> String {
    let loaded = loaded_known(shared);
    let (projects, categories) = loaded.as_deref().map_or_else(|| (Vec::new(), Vec::new()), |l| (projects_offered(l), categories_of(&l.tasks)));
    crate::backend::json(&serde_json::json!({ "projects": projects, "kinds": kinds_offered(), "categories": categories }))
}

/// What the task page shows: its four views at once, so switching is instant.
#[derive(Serialize)]
struct TasksShown {
    /// The choices as you left them.
    view: WorkState,
    /// The kinds of task, in your language.
    kinds: Vec<KindChoice>,
    /// The categories open tasks have, the most used first.
    categories: Vec<String>,
    weather: Weather,
    now: NowView,
    list: ListView,
    board: BoardView,
    timeline: TimelineView,
    lists: Vec<ListChoice>,
    projects: Vec<ProjectChoice>,
    /// No list can take tasks yet.
    no_list: bool,
    /// Work time or quiet time.
    quiet: bool,
    /// "Monday. The plan starts with: …", once work is back, on its day.
    first_step: String,
    first_step_task: String,
    /// The next date asked within a week: the time budget until then, else "".
    budget: String,
    /// In free time, leisure offered, never a list to finish (docs/pauses.md).
    free: Vec<serde_json::Value>,
}

/// Today, laid out: the events of today and the steps the plan gives today, those the page shows.
fn today_laid_out(desk: &Desk, shared: &Shared) -> sioul_core::dayview::DayView {
    let now = Zoned::now();
    let midnight = now.date().to_zoned(now.time_zone().clone()).map_or(0, |z| z.timestamp().as_second());
    // Until the next midnight: a day of 23 or 25 hours when the clocks change.
    let next_midnight = now.date().tomorrow().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()).map_or(midnight + 86_400, |z| z.timestamp().as_second());
    // Today's events, from the plan's one read; deleted, or an occurrence left out, while "Undo" is offered: gone already.
    let (removed, skipped) = crate::mail::hidden_pim(shared);
    let events: Vec<sioul_core::agenda::Occurrence> = pim::without_waiting(desk.read.iter().filter(|o| o.end > midnight && o.start < next_midnight).cloned().collect(), &removed, &skipped);
    let shown: Vec<Task> = desk.loaded.tasks.iter().filter(|t| desk.filter.quiet.as_ref().is_none_or(|q| q.keeps(t))).cloned().collect();
    let mut day = sioul_core::dayview::day(&now, &events, &desk.plan, &shown, &desk.settings);
    // Meals and naps without a name of their own: their usual one, in your language.
    for block in day.blocks.iter_mut().filter(|b| (b.kind == "meal" || b.kind == "nap") && b.title.trim().is_empty()) {
        block.title = crate::health::block_name(block.kind, &block.key, "");
    }
    // Time for you, the free time kept, and why today holds what it holds, in words.
    crate::capacity::dress(&mut day, &desk.loaded.tasks);
    day
}

/// The plan as the Tasks job last made it, with its one read of the
/// calendars: what the agenda, a task's card and `laid` read, so that none of
/// them plans again (today is laid, and its slots of time for you written, by
/// the Tasks job alone).
pub(crate) struct PlanShown {
    /// The files' stamp taken before it was read (`timeline::Stamp`): a write
    /// after it, by this program or another, the sync included, makes it older.
    pub(crate) stamp: sioul_core::timeline::Stamp,
    /// The day it was made on: the plan's today.
    pub(crate) made_on: jiff::civil::Date,
    pub(crate) desk: Arc<Desk>,
    /// Today as laid and dressed then, or laid again since from it
    /// (`relay_today`): what the agenda draws of today, the one producer of
    /// today's layout (the unified agenda, §2.5.1).
    pub(crate) today: sioul_core::dayview::DayView,
}

impl PlanShown {
    /// Whether nothing was written since it was read: no write of this
    /// program's, no file made, replaced or taken out in a calendar's folder by
    /// any program (`vdir::calendars_stamp`).
    pub(crate) fn current(&self) -> bool {
        self.stamp == sioul_core::timeline::Stamp::now()
    }

    /// The plan's one read of the calendars (`Desk::read`), and the plan.
    pub(crate) fn read(&self) -> &[sioul_core::agenda::Occurrence] {
        &self.desk.read
    }

    pub(crate) fn plan(&self) -> &Plan {
        &self.desk.plan
    }

    pub(crate) fn loaded(&self) -> &Arc<Loaded> {
        &self.desk.loaded
    }

    /// The days' room the plan was made with: their hours, meals and pins.
    pub(crate) fn settings(&self) -> &Settings {
        &self.desk.settings
    }

    /// What quiet time hides now from the task pages (`Filter::quiet`).
    pub(crate) fn quiet(&self) -> Option<&sioul_core::quiet::QuietTasks> {
        self.desk.filter.quiet.as_ref()
    }
}

/// The plan last made (`show_work`).
#[cfg(not(test))]
static PLAN_SHOWN: std::sync::Mutex<Option<Arc<PlanShown>>> = std::sync::Mutex::new(None);

#[cfg(test)]
thread_local! {
    /// A test's plan, in its own thread: never another test's, run beside it.
    static PLAN_SHOWN: std::cell::RefCell<Option<Arc<PlanShown>>> = const { std::cell::RefCell::new(None) };
}

fn publish(shown: PlanShown) {
    #[cfg(not(test))]
    {
        // The old one let go after the lock: a whole Desk, never dropped while the window waits for it (`task()`).
        let old = std::mem::replace(&mut *PLAN_SHOWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner), Some(Arc::new(shown)));
        drop(old);
    }
    #[cfg(test)]
    PLAN_SHOWN.with(|plan| *plan.borrow_mut() = Some(Arc::new(shown)));
}

/// A plan made now and published, as the Tasks job makes it (`show_work`:
/// the stamp before the read), for the tests of what reads it; `forget_plan`
/// lets it go. In the test's own thread.
#[cfg(test)]
pub(crate) fn publish_now(shared: &Shared) {
    let stamp = sioul_core::timeline::Stamp::now();
    let desk = Desk::new(Arc::new(read_loaded(shared)), shared);
    let today = today_laid_out(&desk, shared);
    publish(PlanShown { stamp, made_on: Zoned::now().date(), desk: Arc::new(desk), today });
}

#[cfg(test)]
pub(crate) fn forget_plan() {
    PLAN_SHOWN.with(|plan| *plan.borrow_mut() = None);
}

/// Whether Health's list is shown again after a re-plan: today's meals are
/// pushed otherwise than by the plan before (`Settings::shifts`), or no plan
/// of today was made before. Health's list reads them (`health::pushed_on`).
pub(crate) fn health_follows(before: Option<&std::collections::BTreeMap<String, i64>>, after: &std::collections::BTreeMap<String, i64>) -> bool {
    before != Some(after)
}

/// The plan as the Tasks job last made it; none before its first run.
pub(crate) fn plan_shown() -> Option<Arc<PlanShown>> {
    #[cfg(not(test))]
    return PLAN_SHOWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    #[cfg(test)]
    return PLAN_SHOWN.with(|plan| plan.borrow().clone());
}

/// The time a day's meals move past, by the plan's rule (`Settings::with_events`),
/// from `near` (that day's events, with the evening before and the next
/// morning's): today, every event, time blocks included (`Settings::shifts`);
/// another day, its own events and the blocks tasks are pinned to, those of
/// the plan of today (its pins and its tasks), when one is made. One rule for
/// the plan, the agenda, Health's days and the writer of a day's meals
/// (`health::change_day`), whoever read the events (review of 1880bce, later 8).
pub(crate) fn held_from(date: jiff::civil::Date, now: &Zoned, near: Vec<sioul_core::agenda::Occurrence>) -> Vec<(i64, i64)> {
    // A plan older than a write holds its pins where they were: then the blocks as they are now,
    // read in `near` (review of 3a2fbd4, later 10).
    let shown = plan_shown().filter(|s| s.made_on == now.date() && s.current());
    match shown {
        Some(shown) if date != now.date() => {
            let mut held = sioul_core::plan::event_spans(&sioul_core::blocks::without_blocks(near, &shown.loaded().tasks), 0);
            held.extend(shown.plan().pins.values().map(sioul_core::blocks::Pin::span));
            held
        }
        _ => sioul_core::plan::event_spans(&near, 0),
    }
}

/// A day's span with the evening before and the next morning: what a meal can
/// be pushed past, Unix seconds.
fn near_span(date: jiff::civil::Date, zone: &TimeZone) -> (i64, i64) {
    let start = date.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second());
    let end = date.tomorrow().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map_or(start + 86_400, |z| z.timestamp().as_second());
    (start - 86_400, end + 86_400)
}

/// `held_from` over the plan's one read, for a day within its reach, while
/// nothing was written since it was read: what Health's days draw and what a
/// day's meal is moved from, so that the plan, the Health page and the writer
/// agree. None with no plan of today yet, a plan older than a write (review
/// of 1880bce, later 4), or a day beyond its reach: each caller then reads.
pub(crate) fn held_by_plan(date: jiff::civil::Date, now: &Zoned) -> Option<Vec<(i64, i64)>> {
    let shown = plan_shown().filter(|s| s.made_on == now.date() && s.current())?;
    let last = now.date().checked_add(jiff::Span::new().days(sioul_core::plan::HORIZON_DAYS - 1)).ok()?;
    if date < now.date() || date > last {
        return None;
    }
    let (from, to) = near_span(date, now.time_zone());
    let near: Vec<sioul_core::agenda::Occurrence> = shown.read().iter().filter(|o| o.end > from && o.start < to).cloned().collect();
    Some(held_from(date, now, near))
}

/// `held_by_plan`, else that day's events, read now, by the same rule: for
/// the writer of a day's meals alone, which reads once a change (never a view
/// made each minute).
pub(crate) fn held_on(date: jiff::civil::Date, now: &Zoned) -> Vec<(i64, i64)> {
    held_by_plan(date, now).unwrap_or_else(|| {
        #[cfg(test)]
        HELD_READS.with(|n| n.set(n.get() + 1));
        let (from, to) = near_span(date, now.time_zone());
        held_from(date, now, sioul_core::agenda::occurrences(from, to))
    })
}

#[cfg(test)]
thread_local! {
    /// How many times this thread read the calendars for a day's held times (`held_on`'s own read).
    pub(crate) static HELD_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}


/// The routines, as the page lists them: the admin window's first (made from
/// what is there now), then yours, each with its steps as lines to change.
pub(crate) fn routines(shared: &Shared) -> String {
    use sioul_core::routines::{Routine, Step, steps_text};
    #[derive(Serialize)]
    struct Shown {
        #[serde(flatten)]
        routine: Routine,
        text: String,
        builtin: bool,
    }
    let desk = Desk::new(loaded(shared), shared);
    let mut steps = vec![Step { title: tr().text("routine-admin-porch", None), minutes: 10, open: "porch".into() }];
    if let Some(next) = desk.plan.next.as_deref().and_then(|uid| desk.task(uid)) {
        steps.push(Step { title: say("routine-admin-next", &[("title", next.title.clone())]), minutes: desk.plan.items.get(&next.uid).map_or(25, |p| p.left.clamp(5, 45)), open: links::task_uri(&next.uid) });
    }
    steps.push(Step { title: tr().text("routine-admin-stop", None), minutes: 3, open: String::new() });
    let admin = Routine { id: "admin".into(), title: tr().text("routine-admin", None), auto: false, steps };
    let mut out = vec![Shown { text: steps_text(&admin.steps), routine: admin, builtin: true }];
    out.extend(load_config().routines.into_iter().map(|r| Shown { text: steps_text(&r.steps), routine: r, builtin: false }));
    crate::backend::json(&out)
}

/// A routine saved (new when `id` is empty) from its title and its steps, one a line; returns what went wrong, else "".
pub(crate) fn save_routine(id: &str, title: &str, text: &str, auto: bool) -> String {
    let title = title.trim();
    if title.is_empty() {
        return tr().text("routine-no-title", None);
    }
    let steps = sioul_core::routines::parse_steps(text);
    if steps.is_empty() {
        return tr().text("routine-no-steps", None);
    }
    // Read, changed and written under the configuration's lock, which the sharing takes too.
    sioul_core::routines::change(&crate::backend::config_path(), |routines| {
        let id = if id.is_empty() {
            let taken: Vec<String> = routines.iter().map(|r| r.id.clone()).chain(std::iter::once("admin".to_string())).collect();
            sioul_core::projects::new_id(title, &taken)
        } else {
            id.to_string()
        };
        let routine = sioul_core::routines::Routine { id: id.clone(), title: title.to_string(), auto, steps };
        match routines.iter_mut().find(|r| r.id == id) {
            Some(place) => *place = routine,
            None => routines.push(routine),
        }
    })
    .err()
    .unwrap_or_default()
}

/// A routine taken out; returns what went wrong, else "".
pub(crate) fn remove_routine(id: &str) -> String {
    sioul_core::routines::change(&crate::backend::config_path(), |routines| routines.retain(|r| r.id != id)).err().unwrap_or_default()
}

/// The lists a task can go into.
pub(crate) fn writable_lists() -> Vec<ListChoice> {
    let config = load_config();
    let choice = |c: Collection, tasks: bool| ListChoice { pending: vdir::State::load(&c.state_path()).pending, id: format!("{}/{}", c.account, c.id), name: c.label(&config, tr()), color: c.color, tasks };
    let mut lists: Vec<ListChoice> = tasks::lists().into_iter().filter(|c| !c.read_only).map(|c| choice(c, true)).collect();
    // The calendars that keep no task (Google's, event-only ones): greyed, never hidden.
    lists.extend(vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only && !c.holds("VTODO")).map(|c| choice(c, false)));
    lists
}

/// A task moved to another list, its steps with it: written there, then taken
/// out here (the accounts send both). When the list would not keep something
/// the task uses, nothing moves until `confirmed`: returns `{"losses": [words]}`,
/// else `{"uid"}` or `{"error"}`.
pub(crate) fn move_task(qt: &QtThread, shared: &Arc<Shared>, uid: &str, list: &str, confirmed: bool) -> String {
    let loaded = loaded(shared);
    let Some(target) = tasks::lists().into_iter().find(|c| !c.read_only && format!("{}/{}", c.account, c.id) == list) else { return answer(Err(tr().text("task-list-no-tasks", None))) };
    // The task and every step under it.
    let mut family: Vec<&Task> = Vec::new();
    let mut queue = vec![uid.to_string()];
    while let Some(next) = queue.pop() {
        if let Some(task) = loaded.tasks.iter().find(|t| t.uid == next) {
            if task.read_only {
                return answer(Err(tr().text("task-read-only", None)));
            }
            queue.extend(loaded.tasks.iter().filter(|t| t.parent() == Some(task.uid.as_str())).map(|t| t.uid.clone()));
            family.push(task);
        }
    }
    if family.is_empty() {
        return answer(Err(tr().text("task-gone", None)));
    }
    let config = load_config();
    let provider = sioul_core::capabilities::provider_of_collection(config.account(&target.account), &target);
    let mut losses: Vec<&str> = family.iter().flat_map(|t| sioul_core::capabilities::task_losses(t, provider)).collect();
    losses.sort_unstable();
    losses.dedup();
    if !losses.is_empty() && !confirmed {
        return serde_json::json!({ "losses": losses.iter().map(|l| tr().text(&format!("loss-{l}"), None)).collect::<Vec<_>>() }).to_string();
    }
    let mut accounts = BTreeSet::new();
    // Through the moves' journal, in one change (a step cut short is finished, never both, never
    // neither: the check of ef8c0ca, low 27); into a list that keeps no link (Google Tasks), a tie to
    // an appointment undone in that same change, so that a move that fails unties nothing (part E0.9;
    // review of ab7daa4, later 5).
    let google = provider == sioul_core::capabilities::Provider::GoogleTasks;
    let change = match sioul_core::pairs::moved_into(&family, &target.dir, &loaded.events, google, &Zoned::now()) {
        Ok(change) => change,
        Err(e) => return answer(Err(e)),
    };
    match change.write() {
        Ok(files) => accounts.extend(files.iter().filter_map(|f| account_of(&f.path))),
        Err(e) => return answer(Err(e)),
    }
    accounts.insert(target.account.clone());
    for account in accounts {
        pim::nudge(shared, &account);
    }
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    show_work(qt, shared);
    answer(Ok(uid.to_string()))
}

/// Reads everything again, computes the task and notes pages on a thread, and shows them.
pub(crate) fn show_work(qt: &QtThread, shared: &Arc<Shared>) {
    replan_asked(shared);
    tell_replan_due(qt);
    shared.work_replan.store(true, Ordering::SeqCst);
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.work_job, move |shared| work_run(&qt, shared));
}

/// Today laid again from now with what the plan as last made kept (its read,
/// its plan, its settings), reading no store: the step under way, the next,
/// what no longer fits; dressed (the slots of time for you written only when
/// they changed), published, and the agenda follows. While the agenda shows
/// today, at each five minutes' turn of the window's clock and when Sioul
/// comes back. With no plan of today, or one older than a write: the plan is
/// made again instead.
pub(crate) fn relay_today(qt: &QtThread, shared: &Arc<Shared>) {
    shared.work_relay.store(true, Ordering::SeqCst);
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.work_job, move |shared| work_run(&qt, shared));
}

/// The Tasks job's one work, by what was asked: the plan made again
/// (`show_work`), which lays today too and takes a re-lay asked meanwhile
/// with it, else today laid again (`relay_today`). One closure for both:
/// `backend::coalesced` runs the first caller's again while asked, so two
/// closures on one job would lose a request, one swallowing the other.
fn work_run(qt: &QtThread, shared: &Arc<Shared>) {
    let replan = shared.work_replan.swap(false, Ordering::SeqCst);
    let relay = shared.work_relay.swap(false, Ordering::SeqCst);
    if replan {
        replan_now(qt, shared);
    } else if relay {
        relay_now(qt, shared);
    }
}

fn relay_now(qt: &QtThread, shared: &Arc<Shared>) {
    if !relaid(shared) {
        replan_now(qt, shared);
        return;
    }
    crate::agenda::show_agenda(qt, shared);
}

/// Today laid again from the plan as last made, published with it (the same
/// read, the same plan: no store read); false with no plan of today, or one
/// older than a write, which wants the plan made again.
fn relaid(shared: &Shared) -> bool {
    let Some(shown) = plan_shown().filter(|s| s.made_on == Zoned::now().date() && s.current()) else { return false };
    let today = today_laid_out(&shown.desk, shared);
    publish(PlanShown { stamp: shown.stamp, made_on: shown.made_on, desk: Arc::clone(&shown.desk), today });
    true
}

/// A write the plan must follow (an event moved or saved, a task pinned, a
/// meal moved): its re-plan due from now, before any job is started, so that
/// the agenda's job started next waits for it rather than drawing today from
/// the older plan (`agenda::waits_for_replan`; review of 590eb18, later 12).
/// The writers call it as they write; `show_work` too.
pub(crate) fn replan_asked(shared: &Shared) {
    shared.replan_due.store(true, Ordering::SeqCst);
}

/// The window told whether a re-plan is due (`Sioul::replan_due`), as it is
/// when the window's thread reads it: what was let go on the timeline stays
/// drawn there until the re-plan brings it (review of 590eb18, later 2).
fn tell_replan_due(qt: &QtThread) {
    let _ = qt.queue(|mut sioul| {
        let due = sioul.shared().replan_due.load(Ordering::SeqCst);
        sioul.as_mut().set_replan_due(due);
    });
}

/// A re-plan due until its publish, or until its run ends however it ends
/// (`Shared::replan_due`). `ended` hears it stop being due, with whether it
/// was published: the window is told, and a run ended without a publish (a
/// failure, a panic) starts the agenda's job again, whose days waited for it
/// (review of 590eb18, later 4).
struct ReplanDue<'a> {
    shared: &'a Shared,
    ended: Box<dyn FnMut(bool) + 'a>,
    published: bool,
}

impl<'a> ReplanDue<'a> {
    fn new(shared: &'a Shared, ended: impl FnMut(bool) + 'a) -> ReplanDue<'a> {
        ReplanDue { shared, ended: Box::new(ended), published: false }
    }

    /// Published: no more due, unless another re-plan is already asked (it
    /// keeps it due until its own publish).
    fn published(&mut self) {
        self.published = true;
        self.end(true);
    }

    fn end(&mut self, published: bool) {
        if self.shared.work_replan.load(Ordering::SeqCst) {
            return;
        }
        self.shared.replan_due.store(false, Ordering::SeqCst);
        (self.ended)(published);
    }
}

impl Drop for ReplanDue<'_> {
    fn drop(&mut self) {
        if !self.published {
            self.end(false);
        }
    }
}

/// The plan made again from everything read again (`show_work`).
fn replan_now(qt: &QtThread, shared: &Arc<Shared>) {
    let qt = qt.clone();
    // However this run ends, the re-plan it was stops being due (never an agenda waiting for ever).
    let mut due = ReplanDue::new(shared, |published| {
        tell_replan_due(&qt);
        if !published {
            crate::agenda::show_agenda(&qt, shared);
        }
    });
    {
        let generation = shared.work_generation.fetch_add(1, Ordering::Relaxed) + 1;
        // The files' stamp before anything is read: what is written after, by any
        // program, makes this plan older (`PlanShown::current`).
        let stamp = sioul_core::timeline::Stamp::now();
        let made_on = Zoned::now().date();
        let fresh = Arc::new(read_loaded(shared));
        if let Ok(mut cache) = shared.loaded.lock() {
            *cache = Some(Arc::clone(&fresh));
        }
        let state = shared.work.lock().map(|s| s.clone()).unwrap_or_default();
        let desk = Desk::new(fresh, &shared);
        // What changed elsewhere in the blocks set right, a time given by a drag before them made one:
        // laid again then, at once (`blocks::upkeep`; nothing written when nothing differs).
        if crate::blocks::upkeep(&shared, &desk.loaded.tasks, &Zoned::now()) {
            show_work(&qt, &shared);
            pim::show_pim(&qt, &shared);
        }
        // What follows an appointment follows it (`moves`): a paired task's date, while it says the
        // day the pairing wrote (`pairs`), and its legs (`anchors`), by one rule.
        let now = Zoned::now();
        // Only a move this device saw (`moves::Seen`), and again at once only after a write that succeeded (F4, F12).
        let mut seen = sioul_core::moves::Seen::load();
        let before = seen.clone();
        // What closes by itself, from the first pass on only (kept, never moved): the past at the
        // install is never set aside; a clock behind this build starts nothing (`Seen::closing_since`).
        let closing_from = seen.closing_since(now.timestamp().as_second());
        // The pages' events alone: a copy left in an account switched off never moves a date (F15).
        let moved = sioul_core::moves::upkeep(&desk.loaded.tasks, &desk.loaded.events, &mut seen, &now);
        if seen != before {
            let _ = seen.save();
        }
        // An appointment over: the steps before it drop quietly, its task closes once none is left (`pairs::closing`).
        let mut moved = moved;
        let closed = closing_from.map(|from| sioul_core::pairs::closing(&desk.loaded.tasks, &desk.loaded.events, from, &now)).unwrap_or_default();
        if !closed.is_empty()
            && let Ok(files) = closed.write()
        {
            moved.extend(files.into_iter().map(|f| f.path));
        }
        if !moved.is_empty() {
            for account in moved.iter().filter_map(|p| account_of(p)).collect::<BTreeSet<_>>() {
                pim::nudge(&shared, &account);
            }
            show_work(&qt, &shared);
        }
        // The phone's home screen card: the next step as each coming time will have it (homecard.rs).
        crate::homecard::plan_seen(&desk.loaded, &desk.plan, &desk.today, &desk.spent, &desk.stopped);
        let page_filter = Filter { project: state.project.clone(), quiet: desk.filter.quiet.clone(), ..state.filter.clone() };
        let cx = desk.context_with(&page_filter);
        let project = Some(state.project.as_str()).filter(|c| !c.is_empty());
        let rest = desk.settings.rest_days();
        let lists = writable_lists();
        let categories = categories_of(&desk.loaded.tasks);
        // The day's first step, as the plan had it when the day before was closed.
        let today = Zoned::now();
        let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
        let first = match overrides.first_step_on(today.date()).filter(|_| desk.filter.quiet.is_none()) {
            Some((step, uid)) => (say("first-step-line", &[("day", capitalized(&tr().text(&format!("weekday-{}", today.date().weekday().to_monday_one_offset()), None))), ("step", step.to_string())]), uid.to_string()),
            None => (String::new(), String::new()),
        };
        let today_laid = today_laid_out(&desk, &shared);
        let shown = TasksShown {
            view: state.clone(),
            kinds: kinds_offered(),
            categories,
            weather: desk.today.weather,
            now: taskview::now(&cx, desk.today.weather, &desk.today.aside),
            list: taskview::list(&cx, if state.by.is_empty() { "project" } else { &state.by }, state.done, &state.query, taskview::Fold::Opened(&state.opened)),
            board: taskview::board(&cx, project, taskview::Fold::Opened(&state.opened)),
            timeline: taskview::timeline(&cx, project, &rest, taskview::Fold::Opened(&state.opened)),
            no_list: lists.is_empty(),
            lists,
            projects: projects_offered(&desk.loaded),
            quiet: desk.quiet,
            first_step: first.0,
            first_step_task: first.1,
            budget: desk.next_budget(),
            free: crate::pauses::offers(&desk.loaded.tasks, desk.filter.quiet.as_ref(), crate::hours::mode_now().free()),
        };
        let tasks_json = crate::backend::json(&shown);
        drop(shown);
        // The plan and today as laid, for the agenda, a task's card and `laid`: published before
        // the window hears of the change, so that what it reads then is this plan.
        let desk = Arc::new(desk);
        // Today's meals as the plan before pushed them past today's events (Health's list reads them).
        let shifts_before = plan_shown().filter(|p| p.made_on == made_on).map(|p| p.settings().shifts.clone());
        publish(PlanShown { stamp, made_on, desk: Arc::clone(&desk), today: today_laid });
        // Published: the agenda draws from it (a re-plan asked meanwhile sets it again).
        due.published();
        crate::agenda::show_agenda(&qt, &shared);
        // A meal pushed elsewhere by this plan (an event added at lunch): Health's list follows at
        // once, as the agenda does (review of 21436bf, later 7).
        if health_follows(shifts_before.as_ref(), &desk.settings.shifts) {
            crate::health::show_health(&qt, &shared);
        }
        let notes_json = notes_list(&desk.loaded, &state.notes_query, state.notes_tree);
        let focus_json = focus_json(&desk);
        // The time running, in the system's notifications as in the focus window.
        crate::timenote::follow(&qt, shared);
        let _ = qt.queue(move |mut sioul| {
            // Only the newest (see `backend::show`).
            if sioul.shared().work_generation.load(Ordering::Relaxed) != generation || sioul.shared().work_shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
                return;
            }
            sioul.as_mut().set_tasks(QString::from(&tasks_json));
            sioul.as_mut().set_notes(QString::from(&notes_json));
            sioul.as_mut().set_focus_session(QString::from(&focus_json));
        });
    }
}

/// The plan's first step when work comes back: in words, and its task.
#[derive(Serialize, Clone, Default)]
pub(crate) struct FirstStep {
    pub uid: String,
    pub text: String,
}

/// A date asked that falls before work comes back: told once, at closing.
#[derive(Serialize)]
struct Deadline {
    uid: String,
    line: String,
}

/// The screen after closing the day, read in ten seconds: where work went and
/// when it comes back, what got done if anything, the first step, what still
/// gets through. Nothing counts what was not done (docs/tasks.md).
#[derive(Serialize)]
pub(crate) struct Closing {
    /// "Work is put away until Monday 5 October at 09:00."
    put_away: String,
    /// "Done today: …", "Worked on: …": only when something was.
    did: Vec<String>,
    /// "Everything else has its place, from Monday, in the usual order."
    rest: String,
    /// "Monday starts with:"
    starts_with: String,
    first: FirstStep,
    deadline: Option<Deadline>,
    /// "Leave it for Monday".
    leave: String,
}

/// The day closed: the screen to show, when work comes back, and its first step.
pub(crate) struct Closed {
    pub closing: Closing,
    pub back: Zoned,
    pub first: FirstStep,
}

/// "tomorrow", "Monday", "Monday 19 October": the nearer, the shorter.
fn day_name(day: Date, today: Date) -> String {
    if today.tomorrow().is_ok_and(|t| t == day) {
        tr().text("day-tomorrow", None)
    } else if day > today && day.since(today).is_ok_and(|s| s.get_days() < 7) {
        tr().text(&format!("weekday-{}", day.weekday().to_monday_one_offset()), None)
    } else {
        tr().day(day)
    }
}

/// The first letter capitalized, for a word starting a sentence ("lundi" → "Lundi").
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Closes the day in the plan: today's room is taken away and everything flows
/// on to the next days with room, as each morning; nothing is written into the
/// tasks. Computed before the overrides change.
pub(crate) fn closing(shared: &Shared) -> Closed {
    let loaded = loaded(shared);
    let now = Zoned::now();
    let date = now.date();
    let today = Today::load(&Today::default_path(), date);
    let sessions = timelog::sessions();
    let spent = timelog::spent(&sessions, 0, i64::MAX);
    let mut situation = situation(&loaded.projects);
    // The calendars read once for both plans, what waits for Undo left out (`Desk::new`).
    let midnight = date.to_zoned(now.time_zone().clone()).map_or(0, |z| z.timestamp().as_second());
    let (removed, skipped) = mail::hidden_pim(shared);
    let read = pim::without_waiting(sioul_core::agenda::occurrences(midnight, midnight + sioul_core::plan::HORIZON_DAYS * 86_400), &removed, &skipped);
    let before = plan::plan(&loaded.tasks, date, &settings(today.weather, &situation, &loaded, shared, &read), &spent, &today.aside);
    situation.closed.insert(date);
    let after = plan::plan(&loaded.tasks, date, &settings(today.weather, &situation, &loaded, shared, &read), &spent, &today.aside);
    let config = load_config();
    let back = sioul_core::quiet::after_today(&config.working_hours(), &config.time_off, &now, i8::try_from(config.agenda.day_start.unwrap_or(7)).unwrap_or(7));
    let back_day = back.date();
    let task = |uid: &str| loaded.tasks.iter().find(|t| t.uid == uid);
    // What got done, and what was worked on: three lines at most, nothing when nothing.
    let morning = date.to_zoned(now.time_zone().clone()).map_or(0, |z| z.timestamp().as_second());
    let done: Vec<&Task> = loaded.tasks.iter().filter(|t| t.completed.is_some_and(|c| c >= morning)).take(3).collect();
    let mut worked: BTreeMap<String, u32> = BTreeMap::new();
    for session in sessions.iter().filter(|s| s.start >= morning && !s.task.is_empty() && !s.done) {
        if task(&session.task).is_some_and(|t| t.status.is_open()) {
            *worked.entry(session.task.clone()).or_default() += session.minutes;
        }
    }
    let mut did = Vec::new();
    if !done.is_empty() {
        did.push(say("closing-done", &[("list", done.iter().map(|t| t.title.clone()).collect::<Vec<_>>().join(", "))]));
    }
    let room = 3usize.saturating_sub(done.len());
    if room > 0 && !worked.is_empty() {
        let list: Vec<String> = worked.iter().filter_map(|(uid, minutes)| task(uid).map(|t| format!("{} ({})", t.title, sioul_core::timereport::duration(*minutes)))).take(room).collect();
        did.push(say("closing-worked", &[("list", list.join(", "))]));
    }
    // Where the rest went, without a number.
    let moved = before.order.iter().any(|uid| {
        let (Some(was), Some(will)) = (before.items.get(uid), after.items.get(uid)) else { return false };
        !was.optional && was.start.is_some_and(|a| a <= date) && will.start.is_none_or(|b| b > date)
    });
    let rest = if moved { say("closing-rest", &[("day", day_name(back_day, date))]) } else { String::new() };
    // The first step: the note left when a session stopped today, else the plan's first.
    let noted = sessions.iter().filter(|s| s.start >= morning && !s.note.is_empty() && task(&s.task).is_some_and(|t| t.status.is_open())).max_by_key(|s| s.start);
    let first = match noted {
        Some(session) => FirstStep { uid: session.task.clone(), text: session.note.clone() },
        None => after
            .order
            .iter()
            .filter_map(|uid| Some((uid, after.items.get(uid)?)))
            .filter(|(_, p)| !p.optional && p.open_steps == 0 && p.start.is_some_and(|s| s >= back_day))
            .min_by_key(|(_, p)| p.start)
            .and_then(|(uid, p)| {
                let t = task(uid)?;
                let text = if p.left > 0 { format!("{} ({})", t.title, sioul_core::timereport::duration(p.left)) } else { t.title.clone() };
                Some(FirstStep { uid: uid.clone(), text })
            })
            .unwrap_or_default(),
    };
    let starts_with = say("closing-starts-with", &[("day", capitalized(&day_name(back_day, date)))]);
    // A date asked before work comes back: the nearest, told once.
    let deadline = loaded
        .tasks
        .iter()
        .filter(|t| t.status.is_open())
        .filter_map(|t| Some((t, t.due_date()?)))
        .filter(|(_, due)| *due >= date && *due < back_day)
        .min_by_key(|(_, due)| *due)
        .map(|(t, due)| {
            let day = if due == date { tr().text("day-today", None) } else { day_name(due, date) };
            Deadline { uid: t.uid.clone(), line: say("closing-deadline", &[("title", t.title.clone()), ("day", day)]) }
        });
    let closing = Closing {
        put_away: say("closing-put-away", &[("back", sioul_core::quiet::until_text(tr(), &back, &now))]),
        did,
        rest,
        starts_with,
        first: first.clone(),
        deadline,
        leave: say("closing-leave", &[("day", day_name(back_day, date))]),
    };
    Closed { closing, back, first }
}

/// The task page's choices: how the list is grouped, done tasks, a search, one project.
pub(crate) fn set_view(qt: &QtThread, shared: &Arc<Shared>, by: &str, done: bool, query: &str, project: &str) {
    if let Ok(mut state) = shared.work.lock() {
        state.by = by.to_string();
        state.done = done;
        state.query = query.to_string();
        state.project = project.to_string();
        state.save();
    }
    show_work(qt, shared);
}

/// A task with steps opened (its steps shown under it) or folded again, in
/// List, Board and the Gantt; kept for next time, only for tasks still here.
pub(crate) fn fold_task(qt: &QtThread, shared: &Arc<Shared>, uid: &str, open: bool) {
    if let Ok(mut state) = shared.work.lock() {
        if open {
            state.opened.insert(uid.to_string());
        } else {
            state.opened.remove(uid);
        }
        if let Some(loaded) = loaded_known(shared) {
            state.opened.retain(|u| loaded.tasks.iter().any(|t| t.uid == *u));
        }
        state.save();
    }
    show_work(qt, shared);
}

/// Tasks of one kind and one category only ("" for all), kept for next time.
pub(crate) fn set_filter(qt: &QtThread, shared: &Arc<Shared>, kind: &str, category: &str) {
    if let Ok(mut state) = shared.work.lock() {
        state.filter = Filter { kind: kind.to_string(), category: category.to_string(), ..Filter::default() };
        state.save();
    }
    show_work(qt, shared);
}

/// The account a task file syncs through.
fn account_of(path: &Path) -> Option<String> {
    path.strip_prefix(Kind::Calendars.root()).ok().and_then(|rest| rest.components().next()).map(|c| c.as_os_str().to_string_lossy().to_string())
}

/// Writes a task file, has its account send it, and shows everything again.
fn write(qt: &QtThread, shared: &Arc<Shared>, path: &Path, text: &str) -> Result<(), String> {
    vdir::write_item(path, text)?;
    if let Some(account) = account_of(path) {
        pim::nudge(shared, &account);
    }
    show_work(qt, shared);
    Ok(())
}

/// The list a new task goes into: the one chosen ("account/id"), else the
/// usual one (Tasks ⚙, "New tasks go into"), else the first made for tasks.
pub(crate) fn target_list(id: &str) -> Result<Collection, String> {
    let lists: Vec<Collection> = tasks::lists().into_iter().filter(|c| !c.read_only).collect();
    let usual = load_config().tasks.list.clone().unwrap_or_default();
    let named = |wanted: &str| lists.iter().find(|c| !wanted.is_empty() && format!("{}/{}", c.account, c.id) == wanted).cloned();
    named(id).or_else(|| named(&usual)).or_else(tasks::default_list).ok_or_else(|| tr().text("task-no-list-yet", None))
}

/// The form of a task not made yet, as `task` gives one made (no card: it is
/// not read yet): blank, or what Add ▾ makes from `from` (`key`, `start`: which
/// message, which occurrence). In `list`, else the usual list; what that list
/// keeps. Nothing is written: `save` with an empty UID makes it, once it has
/// a title. {"error"} when what it comes from is gone.
pub(crate) fn new_form(shared: &Shared, from: &str, key: &str, start: f64, list: &str) -> String {
    let made = if from.is_empty() { Ok(TaskEdit::default()) } else { linked_edit(shared, from, key, start) };
    let edit = match made {
        Ok(edit) => edit,
        Err(error) => return answer(Err(error)),
    };
    let loaded = loaded(shared);
    let config = load_config();
    let target = target_list(list).ok();
    let list_id = target.as_ref().map(|c| format!("{}/{}", c.account, c.id)).unwrap_or_default();
    let limited = target
        .as_ref()
        .map(|c| sioul_core::capabilities::task_fields_lost(sioul_core::capabilities::provider_of_collection(config.account(&c.account), c), false))
        .unwrap_or_default();
    // What its tags would say it is for, as the panel shows it for a task made.
    let shaped = Task { categories: edit.categories.clone(), projects: edit.projects.clone(), list_id: list_id.clone(), ..Task::default() };
    let area_tags = sioul_core::areas::TaskAreas::of_config(&config, &loaded.projects).by_tags(&shaped).id();
    let source = if from.is_empty() { String::new() } else { loaded.world().describe(&loaded.world().canonical(from)).title };
    // What was felt after tasks of its title or its kind, faint in its form.
    let proposed = sioul_core::capacity::FeltIndex::of(&loaded.tasks).proposal(&Task { title: edit.title.clone(), kind: edit.kind.clone(), ..Task::default() });
    serde_json::json!({
        "card": null,
        "edit": edit,
        "proposed": proposed,
        "steps": [],
        "steps_total": "",
        "waits_for": [],
        "frees": [],
        "sessions": [],
        "related": [],
        "area_tags": area_tags,
        "lists": writable_lists(),
        "list": list_id,
        "limited": limited,
        "budget": "",
        "source": source,
        // As `task` gives them for a task made: nothing computed yet.
        "level": "",
        "estimate_first": null,
        "ratio_line": "",
        "felt_on": "",
    })
    .to_string()
}

/// Makes a task in a list; returns its UID.
fn create(qt: &QtThread, shared: &Arc<Shared>, edit: &TaskEdit, list: &str) -> Result<String, String> {
    if edit.title.trim().is_empty() {
        return Err(tr().text("task-no-title", None));
    }
    let target = target_list(list)?;
    let uid = vdir::new_name();
    let (zone, now) = (TimeZone::system(), Zoned::now());
    // A step goes after its bigger task's others, as they are on disk now: the
    // page may not have read the one added a moment ago (`tasks::next_in_sequence`).
    let mut edit = edit.clone();
    if !edit.parent.is_empty() && edit.sort_order.is_none() {
        let all = tasks::all(&zone);
        edit.sort_order = Some(tasks::next_in_sequence(tasks::steps_of(&all, &edit.parent), &now));
    }
    let text = tasks::new_task(&edit, &uid, &zone, &now)?;
    write(qt, shared, &target.dir.join(format!("{uid}.ics")), &text)?;
    write_gaps(qt, shared, &uid, &edit);
    Ok(uid)
}

/// A step moved in its bigger task's sequence, before the step `before`
/// ("": after the others), as the files are now (`tasks::reorder`): one file
/// written when there is room, else those renumbered. Returns what went
/// wrong, else "".
pub(crate) fn move_step(qt: &QtThread, shared: &Arc<Shared>, uid: &str, before: &str) -> String {
    let moved = || -> Result<(), String> {
        let zone = TimeZone::system();
        let all = tasks::all(&zone);
        let gone = || tr().text("task-gone", None);
        let parent = all.iter().find(|t| t.uid == uid).and_then(|t| t.parent()).ok_or_else(gone)?;
        let steps = tasks::steps_of(&all, parent);
        let writes = tasks::reorder(&steps, uid, Some(before).filter(|b| !b.is_empty())).map_err(|_| gone())?;
        let file_of = |u: &str| steps.iter().find(|t| t.uid == u).copied();
        if writes.iter().any(|(u, _)| file_of(u).is_none_or(|t| t.read_only)) {
            return Err(tr().text("task-read-only", None));
        }
        let now = Zoned::now();
        for (u, place) in &writes {
            let Some(step) = file_of(u) else { continue };
            let current = std::fs::read_to_string(&step.key).map_err(|e| e.to_string())?;
            let text = tasks::set_sequence(&current, *place, &now)?;
            if text != current {
                write(qt, shared, Path::new(&step.key), &text)?;
            }
        }
        Ok(())
    };
    moved().err().unwrap_or_default()
}

/// The waits the form gave a gap (`TaskEdit::wait_gaps`), written in the
/// tasks waited for, as `sioul tasks import` writes them (`tasks::set_gap`):
/// once the task that waits is written. What fails is said in the status line;
/// the task itself is kept.
fn write_gaps(qt: &QtThread, shared: &Arc<Shared>, uid: &str, edit: &TaskEdit) {
    let now = Zoned::now();
    for (other, gap) in edit.gapped_waits() {
        if let Err(e) = change(qt, shared, &other, |text| tasks::set_gap(text, uid, gap, &now)) {
            tell(qt, shared, e);
        }
    }
}

/// A task in a list kept on this computer only, made the first time with
/// `list_name`: what must never reach a server (health errands). Returns its UID.
/// A task made in a list ("account/id"); returns its UID.
pub(crate) fn create_task(qt: &QtThread, shared: &Arc<Shared>, edit: &TaskEdit, list: &str) -> Result<String, String> {
    create(qt, shared, edit, list)
}

pub(crate) fn local_task(qt: &QtThread, shared: &Arc<Shared>, list_name: &str, edit: &TaskEdit) -> Result<String, String> {
    let local = tasks::lists().into_iter().find(|c| c.account == vdir::LOCAL && !c.read_only);
    let list = match local {
        Some(list) => list,
        None => vdir::create(Kind::Calendars, vdir::LOCAL, list_name, None, &["VTODO"])?,
    };
    create(qt, shared, edit, &format!("{}/{}", list.account, list.id))
}

fn find(shared: &Shared, uid: &str) -> Result<Task, String> {
    loaded(shared).tasks.iter().find(|t| t.uid == uid).cloned().ok_or_else(|| tr().text("task-gone", None))
}

/// Changes one task's file with `change`, unless it can only be read.
fn change(qt: &QtThread, shared: &Arc<Shared>, uid: &str, change: impl FnOnce(&str) -> Result<String, String>) -> Result<(), String> {
    let task = find(shared, uid)?;
    if task.read_only {
        return Err(tr().text("task-read-only", None));
    }
    let current = std::fs::read_to_string(&task.key).map_err(|e| e.to_string())?;
    let text = change(&current)?;
    if text != current {
        write(qt, shared, Path::new(&task.key), &text)?;
    }
    Ok(())
}

/// Every category tasks have, those of open tasks first, the most used first.
pub(crate) fn categories_in_use(shared: &Shared) -> Vec<String> {
    let mut counted: BTreeMap<String, (usize, usize, String)> = BTreeMap::new();
    for task in &loaded(shared).tasks {
        for category in &task.categories {
            let entry = counted.entry(category.to_lowercase()).or_insert((0, 0, category.clone()));
            if task.status.is_open() {
                entry.0 += 1;
            }
            entry.1 += 1;
        }
    }
    let mut all: Vec<(usize, usize, String)> = counted.into_values().collect();
    all.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.to_lowercase().cmp(&b.2.to_lowercase())));
    all.into_iter().map(|(_, _, c)| c).collect()
}

/// A task's file as it is now, with `change` made to its own fields (its
/// form as the file has them), nothing else of it written: never a copy read
/// before, which would take back a step ticked or a field changed since
/// (review of 85da7f0, later 2). A file that holds no task stays as it is.
fn changed_in_file(text: &str, change: impl FnOnce(&mut TaskEdit)) -> Result<String, String> {
    let zone = TimeZone::system();
    let Some(now) = tasks::task_of_text(text, &zone) else { return Ok(text.to_string()) };
    let mut edit = TaskEdit::of(&now);
    change(&mut edit);
    tasks::apply(text, &edit, &zone, &Zoned::now())
}

/// A category renamed on every task that has it, or taken off them all (`to`
/// empty); a name already there merges with it. Written at once, then the
/// accounts send the changes.
pub(crate) fn rename_category(qt: &QtThread, shared: &Arc<Shared>, from: &str, to: &str) -> Result<(), String> {
    let fold = |s: &str| s.trim().to_lowercase();
    let (from, to) = (fold(from), to.trim().to_string());
    let mut accounts = BTreeSet::new();
    let mut problem = None;
    let renamed = |all: &[String]| {
        let mut categories: Vec<String> = Vec::new();
        for category in all {
            let next = if fold(category) == from { to.clone() } else { category.clone() };
            if !next.is_empty() && !categories.iter().any(|c| fold(c) == fold(&next)) {
                categories.push(next);
            }
        }
        categories
    };
    for task in loaded(shared).tasks.iter().filter(|t| !t.read_only && t.categories.iter().any(|c| fold(c) == from)) {
        let path = Path::new(&task.key);
        // The categories renamed over the file as it is now (`changed_in_file`).
        let written = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|current| changed_in_file(&current, |edit| edit.categories = renamed(&edit.categories)).map(|text| (current, text)))
            .and_then(|(current, text)| if text == current { Ok(()) } else { vdir::write_item(path, &text) });
        match written {
            Ok(()) => {
                if let Some(account) = account_of(path) {
                    accounts.insert(account);
                }
            }
            Err(e) => {
                problem.get_or_insert(e);
            }
        }
    }
    for account in accounts {
        pim::nudge(shared, &account);
    }
    show_work(qt, shared);
    problem.map_or(Ok(()), Err)
}

/// A note moved to the vault's trash, "Undo" offered; returns what went wrong, else "".
pub(crate) fn trash_note(qt: &QtThread, shared: &Arc<Shared>, path: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return tr().text("error-no-store", None) };
    match notes::trash(&root, path) {
        Ok(trashed) => {
            mail::offer_untrash(qt, shared, trashed, path.to_string(), say("note-trashed", &[("title", path.rsplit('/').next().unwrap_or(path).to_string())]));
            show_work(qt, shared);
            String::new()
        }
        Err(e) => e,
    }
}

/// A note renamed, the notes, tasks and ties that name it following;
/// returns {"path"} or {"error"}.
pub(crate) fn rename_note(qt: &QtThread, shared: &Arc<Shared>, path: &str, name: &str) -> String {
    let loaded = loaded(shared);
    let Some(vault) = loaded.vault.as_ref() else { return answer(Err(tr().text("error-no-store", None))) };
    // The notes naming it are read again and changed with the notes' lock held (an agent writing one meanwhile).
    let (new_path, _) = match notes::with_lock(|| notes::rename(vault, path, name)) {
        Ok(done) => done,
        Err(e) => return answer(Err(e)),
    };
    retarget_note(qt, shared, &loaded, path, &new_path);
    show_work(qt, shared);
    serde_json::json!({ "path": new_path }).to_string()
}

/// What names a note by its address follows it to its new path: the tasks'
/// links (changed, then sent), and the ties Sioul keeps itself.
fn retarget_note(qt: &QtThread, shared: &Arc<Shared>, loaded: &Loaded, path: &str, new_path: &str) {
    let (old_uri, new_uri) = (notes::uri_of(path), notes::uri_of(new_path));
    // Tasks hold their links: each one naming the note is changed, then sent.
    for task in loaded.tasks.iter().filter(|t| !t.read_only && t.links.iter().any(|l| l.uri == old_uri)) {
        // Its links followed over the file as it is now (`changed_in_file`).
        let _ = change(qt, shared, &task.uid, |text| {
            changed_in_file(text, |edit| {
                for link in edit.links.iter_mut().filter(|l| l.uri == old_uri) {
                    link.uri = new_uri.clone();
                }
            })
        });
    }
    // Ties Sioul keeps itself, read again under their lock.
    let _ = LocalLinks::change(&LocalLinks::default_path(), |ties| {
        let mut moved = false;
        for edge in ties.links.iter_mut() {
            for end in [&mut edge.from, &mut edge.to] {
                if *end == old_uri {
                    *end = new_uri.clone();
                    moved = true;
                }
            }
        }
        moved
    });
}

/// One task in full, with its lists and what it is tied to, as JSON.
pub(crate) fn task(shared: &Shared, uid: &str) -> String {
    // The plan as last made (`PlanShown`), never made again here, on the
    // window's thread (§2.5.6 of the unified agenda's proposal); a task not
    // in it yet (made a moment ago), or no plan yet: made now, once. A write
    // since is followed by the Tasks job, whose new plan reads it again.
    let shown = plan_shown().filter(|s| s.desk.task(uid).is_some());
    let desk = match &shown {
        Some(shown) => Arc::clone(&shown.desk),
        None => Arc::new(Desk::new(loaded(shared), shared)),
    };
    let loaded = Arc::clone(&desk.loaded);
    let Some(known) = desk.task(uid) else { return String::new() };
    // The task's own fields from its file, one read, never an older copy (a
    // tick elsewhere, a save, a drag, a sync since the plan was made: review of
    // 1880bce, M1); the plan's numbers (its days, its budget) from the plan as made.
    let Some(fresh) = from_its_file(known) else { return String::new() };
    let task = &fresh;
    let related = loaded.world().related(&links::task_uri(uid));
    let mut detail = taskview::detail(&desk.context(), task, &desk.sessions, &related);
    detail.ratio_line = crate::capacity::ratio_line(task);
    #[derive(Serialize)]
    struct Shown<'a> {
        #[serde(flatten)]
        detail: taskview::DetailView,
        lists: Vec<ListChoice>,
        list: &'a str,
        /// What its list does not keep: those fields greyed (Google Tasks).
        limited: Vec<&'static str>,
        /// Its date asked within three weeks: the time budget until then.
        budget: String,
        /// What was felt after it, or tasks of its kind, before: faint in its form.
        proposed: sioul_core::capacity::Proposal,
        /// The title of the task it is a step of; "" for none.
        parent_title: String,
        /// The appointment it prepares, typed as part F says it (`pairs::AppointmentView`:
        /// uid, key, title, when, here, when to leave, Before, Bring, its line); null for none.
        appointment: serde_json::Value,
        /// Its list keeps ties to appointments (not Google Tasks, `pairs::links_kept`): "Its appointment…" offered.
        keeps_ties: bool,
    }
    let config = load_config();
    let limited = tasks::lists()
        .into_iter()
        .find(|c| format!("{}/{}", c.account, c.id) == task.list_id)
        .map(|c| sioul_core::capabilities::task_fields_lost(sioul_core::capabilities::provider_of_collection(config.account(&c.account), &c), task.parent().is_some()))
        .unwrap_or_default();
    let today = Zoned::now().date();
    let soon = today.checked_add(jiff::Span::new().days(21)).unwrap_or(today);
    let budget = match task.due_date() {
        Some(date) if task.status.is_open() && date >= today && date <= soon && desk.plan.items.get(uid).is_some_and(|p| !p.optional) => desk.budget_line(date),
        _ => String::new(),
    };
    let proposed = sioul_core::capacity::FeltIndex::of(&loaded.tasks).proposal(task);
    let parent_title = task.parent().and_then(|p| loaded.tasks.iter().find(|t| t.uid == p)).map(|t| t.title.clone()).unwrap_or_default();
    // The appointment it prepares, when it is here (an account switched off's too, as the plan
    // finds it): its title and when, said as the agenda says them; no "when" for a start that
    // cannot be read.
    let zone = TimeZone::system();
    let appointment = match sioul_core::pairs::appointment_of(task) {
        Some(uid) => match loaded.events.iter().find(|e| e.uid == uid).cloned().or_else(|| sioul_core::pairs::switched_off_events(&zone).into_iter().find(|e| e.uid == uid)) {
            Some(event) => {
                // Its next time, as the plan read it (its margins and your own time around it), else its first.
                let now = Zoned::now().timestamp().as_second();
                let time = desk.read.iter().filter(|o| o.uid == uid && o.end > now).min_by_key(|o| o.start).cloned().unwrap_or_else(|| sioul_core::agenda::Occurrence { key: event.key.clone(), uid: uid.clone(), summary: event.summary.clone(), start: event.start, end: event.start, cancelled: event.cancelled, ..Default::default() });
                let legs = sioul_core::anchors::legs_seen_in(&uid, &desk.read);
                let mut view = serde_json::to_value(sioul_core::pairs::appointment_view(&time, &event.links, &loaded.tasks, &legs, None, tr(), &zone)).unwrap_or_default();
                if event.start <= 0 {
                    view["when"] = serde_json::Value::String(String::new());
                }
                view
            }
            None => serde_json::json!({ "uid": uid, "key": "", "title": "", "when": "", "here": false }),
        },
        None => serde_json::Value::Null,
    };
    let keeps_ties = sioul_core::pairs::links_kept(&config)(&task.list_id);
    crate::backend::json(&Shown { detail, lists: writable_lists(), list: &task.list_id, limited, budget, proposed, parent_title, appointment, keeps_ties })
}

/// A task read again from its own file, its list's marks kept from `known`
/// (its list, its colour, whether it can be written); None when the file is
/// gone or holds no task any more.
fn from_its_file(known: &Task) -> Option<Task> {
    let text = std::fs::read_to_string(&known.key).ok()?;
    if !text.to_ascii_uppercase().contains("BEGIN:VTODO") {
        return None;
    }
    let mut task = tasks::task_of_text(&text, &TimeZone::system())?;
    task.key = known.key.clone();
    task.list = known.list.clone();
    task.list_id = known.list_id.clone();
    task.color = known.color.clone();
    task.read_only = known.read_only;
    Some(task)
}

/// A change of a composed task (`sioul_core::composed`) written and said,
/// with Undo: {"uid"}, {"error"}, or {"losses", "said"} to ask first.
fn composed_done(qt: &QtThread, shared: &Arc<Shared>, planned: Result<sioul_core::composed::Change, sioul_core::composed::Refusal>, uid: &str, line: impl FnOnce() -> String) -> String {
    use sioul_core::composed::Refusal;
    let change = match planned {
        Ok(change) => change,
        Err(Refusal::Losses(what)) => {
            let said = Refusal::Losses(what.clone()).said(tr());
            return serde_json::json!({ "losses": what.iter().map(|w| tr().text(&format!("loss-{w}"), None)).collect::<Vec<_>>(), "said": said }).to_string();
        }
        Err(refusal) => return answer(Err(refusal.said(tr()))),
    };
    if change.is_empty() {
        return answer(Ok(uid.to_string()));
    }
    match change.write() {
        Ok(files) => {
            crate::agents::say(qt, shared, sioul_core::agentwrites::Said { at: 0, line: line(), files });
            answer(Ok(uid.to_string()))
        }
        Err(e) => answer(Err(e)),
    }
}

/// Every list's keeper, for what a move would lose.
fn keeper(config: &sioul_core::config::Config) -> impl Fn(&Collection) -> sioul_core::capabilities::Provider + '_ {
    move |c: &Collection| sioul_core::capabilities::provider_of_collection(config.account(&c.account), c)
}

/// A task made a step of `bigger` ("": a task of its own again): {"uid"},
/// {"error"}, or {"losses", "said"} when its family would move into a list
/// that keeps less, asked first (`confirmed` goes on).
pub(crate) fn make_step_of(qt: &QtThread, shared: &Arc<Shared>, uid: &str, bigger: &str, confirmed: bool) -> String {
    let (zone, now, config) = (TimeZone::system(), Zoned::now(), load_config());
    let all = tasks::all(&zone);
    let title = |u: &str| all.iter().find(|t| t.uid == u).map(|t| t.title.clone()).unwrap_or_default();
    let planned = if bigger.is_empty() { sioul_core::composed::promote(&all, uid, &now) } else { sioul_core::composed::demote(&all, uid, bigger, &tasks::lists(), &keeper(&config), confirmed, &now) };
    composed_done(qt, shared, planned, uid, || if bigger.is_empty() { say("composed-made-own", &[("title", title(uid))]) } else { say("composed-made-step", &[("title", title(uid)), ("whole", title(bigger))]) })
}

/// A step or a task an appointment over set aside by itself brought back
/// (`pairs::brought_back`): open again, never set aside again by it; a
/// step's task set aside with it comes back too. Said with Undo. {"uid"} or {"error"}.
pub(crate) fn bring_back(qt: &QtThread, shared: &Arc<Shared>, uid: &str) -> String {
    let (zone, now) = (TimeZone::system(), Zoned::now());
    let all = tasks::all(&zone);
    let back = |task: &Task| std::fs::read_to_string(&task.key).ok().and_then(|text| sioul_core::pairs::brought_back(&text, &now)).map(|t| (std::path::PathBuf::from(&task.key), t));
    let planned = match all.iter().find(|t| t.uid == uid) {
        None => Err(sioul_core::composed::Refusal::Gone),
        Some(task) if task.read_only => Err(sioul_core::composed::Refusal::ReadOnly),
        Some(task) => {
            let mut written: Vec<(std::path::PathBuf, String)> = back(task).into_iter().collect();
            if let Some(whole) = task.parent().and_then(|p| all.iter().find(|t| t.uid == p)).filter(|w| !w.read_only && w.set_aside == task.set_aside && !w.brought_back) {
                written.extend(back(whole));
            }
            Ok(sioul_core::composed::Change { written, ..sioul_core::composed::Change::default() })
        }
    };
    let title = all.iter().find(|t| t.uid == uid).map(|t| t.title.clone()).unwrap_or_default();
    composed_done(qt, shared, planned, uid, || say("task-brought-back", &[("title", title)]))
}

/// A step said to come after its appointment, or before it (part E6): laid
/// then forward from the appointment's end, said under Coming back, never
/// dropped once the appointment is over. {"uid"} or {"error"}.
pub(crate) fn set_step_after(qt: &QtThread, shared: &Arc<Shared>, uid: &str, after: bool) -> String {
    let (zone, now) = (TimeZone::system(), Zoned::now());
    let all = tasks::all(&zone);
    let planned = match all.iter().find(|t| t.uid == uid) {
        None => Err(sioul_core::composed::Refusal::Gone),
        Some(step) if step.read_only => Err(sioul_core::composed::Refusal::ReadOnly),
        Some(step) => std::fs::read_to_string(&step.key)
            .ok()
            .and_then(|text| sioul_core::anchors::step_side(&text, after, &now).ok().filter(|t| *t != text))
            .map(|text| sioul_core::composed::Change { written: vec![(std::path::PathBuf::from(&step.key), text)], ..sioul_core::composed::Change::default() })
            .ok_or(sioul_core::composed::Refusal::Gone)
            .or_else(|_| Ok(sioul_core::composed::Change::default())),
    };
    let title = all.iter().find(|t| t.uid == uid).map(|t| t.title.clone()).unwrap_or_default();
    composed_done(qt, shared, planned, uid, || say(if after { "step-after-said" } else { "step-before-said" }, &[("title", title)]))
}

/// A step folded into its bigger task's list (an item), its blocks to come
/// taken out: {"uid"} (its bigger task's), {"error"}, or {"losses", "said"}.
pub(crate) fn fold_step(qt: &QtThread, shared: &Arc<Shared>, uid: &str, confirmed: bool) -> String {
    let (zone, now, config) = (TimeZone::system(), Zoned::now(), load_config());
    let all = tasks::all(&zone);
    let step = all.iter().find(|t| t.uid == uid);
    let whole = step.and_then(|t| t.parent()).unwrap_or_default().to_string();
    let titles = (step.map(|t| t.title.clone()).unwrap_or_default(), all.iter().find(|t| t.uid == whole).map(|t| t.title.clone()).unwrap_or_default());
    let chosen = config.tasks.blocks.clone().unwrap_or_default();
    let blocks = sioul_core::blocks::to_come_of(uid, &chosen, now.timestamp().as_second(), &zone);
    let planned = sioul_core::composed::step_to_item(&all, uid, &blocks, confirmed, &now);
    composed_done(qt, shared, planned, &whole, || say("composed-folded", &[("title", titles.0), ("whole", titles.1)]))
}

/// Whether the task `uid`'s list keeps one level of steps (Google Tasks, `capabilities::one_level`).
fn one_level(all: &[Task], uid: &str) -> bool {
    all.iter().find(|t| t.uid == uid).is_some_and(|t| sioul_core::capabilities::one_level(&load_config(), &t.list_id))
}

/// An item of a checklist made a step of its task (`to` "step") or a task of
/// its own ("task"): of a task (`kind` "task", `key` its UID) or an event
/// ("event", its file). Returns what went wrong, else "".
pub(crate) fn convert_item(qt: &QtThread, shared: &Arc<Shared>, kind: &str, key: &str, line: usize, to: &str) -> String {
    use sioul_core::composed::{self, Holder};
    let (zone, now) = (TimeZone::system(), Zoned::now());
    let all = tasks::all(&zone);
    let notes = if kind == "event" { crate::pim::ours(key).and_then(|p| sioul_core::agenda::edit_of(&p)).map(|e| e.notes) } else { all.iter().find(|t| t.uid == key).map(|t| t.notes.clone()) };
    let words = notes.and_then(|n| sioul_core::checklist::items(&n).into_iter().find(|i| i.line == line)).map(|i| i.text).unwrap_or_default();
    let planned = match (kind, to) {
        ("task", "step") => composed::item_to_step(&all, key, line, one_level(&all, key), &now).map(|(change, _)| change),
        ("task", _) => match (all.iter().find(|t| t.uid == key), target_list("")) {
            (Some(task), Ok(list)) => composed::item_to_task(&Holder::Task(task), line, &list, &now).map(|(change, _)| change),
            (None, _) => Err(composed::Refusal::Gone),
            (_, Err(e)) => return e,
        },
        _ => {
            let Some(path) = crate::pim::ours(key) else { return tr().text("agenda-gone", None) };
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let writable = vdir::collections(Kind::Calendars).into_iter().find(|c| path.parent() == Some(c.dir.as_path())).is_some_and(|c| !c.read_only);
            if !writable {
                return tr().text("agenda-read-only", None);
            }
            if sioul_core::agenda::with_others(&text) {
                return tr().text("checklist-with-others", None);
            }
            let uid = sioul_core::appointments::uid_in(&path);
            match target_list("") {
                Ok(list) => composed::item_to_task(&Holder::Event { path: &path, text: &text, uid: &uid }, line, &list, &now).map(|(change, _)| change),
                Err(e) => return e,
            }
        }
    };
    let said = composed_done(qt, shared, planned, key, || say(if to == "step" { "composed-item-step" } else { "composed-item-task" }, &[("title", words)]));
    serde_json::from_str::<serde_json::Value>(&said).ok().and_then(|v| v["error"].as_str().map(str::to_string)).unwrap_or_default()
}

/// The notes with the checklist's item on `line` ticked: "toggle" (to do or
/// not needed becomes done, done to do again), "done", "open", "skipped".
/// The list changed since it was shown: said, nothing written.
pub(crate) fn ticked_notes(notes: &str, line: usize, state: &str) -> Result<String, String> {
    let stale = || tr().text("checklist-stale", None);
    let item = sioul_core::checklist::items(notes).into_iter().find(|i| i.line == line).ok_or_else(stale)?;
    let mark = if state == "toggle" { sioul_core::checklist::toggled(item.mark) } else { sioul_core::checklist::Mark::parse(state).ok_or_else(stale)? };
    sioul_core::checklist::ticked(notes, line, mark).map_err(|_| stale())
}

/// A task's checklist ticked (`ticked_notes`): its DESCRIPTION alone written.
/// Returns what went wrong, else "".
pub(crate) fn check_item(qt: &QtThread, shared: &Arc<Shared>, uid: &str, line: usize, state: &str) -> String {
    let zone = TimeZone::system();
    let result = change(qt, shared, uid, |text| {
        let task = tasks::task_of_text(text, &zone).ok_or_else(|| tr().text("task-gone", None))?;
        let mut edit = TaskEdit::of(&task);
        edit.notes = ticked_notes(&task.notes, line, state)?;
        tasks::apply(text, &edit, &zone, &Zoned::now())
    });
    result.err().unwrap_or_default()
}

/// Notes that the task's list would cut (Google Tasks keeps 8,192 characters): said before anything is written.
fn notes_fit(task: &Task, notes: &str) -> Result<(), String> {
    let config = load_config();
    let provider = tasks::lists().into_iter().find(|c| format!("{}/{}", c.account, c.id) == task.list_id).map(|c| sioul_core::capabilities::provider_of_collection(config.account(&c.account), &c));
    if provider.is_some_and(|p| sioul_core::capabilities::notes_too_long(p, notes)) { Err(tr().text("notes-google-too-long", None)) } else { Ok(()) }
}

/// Saves the task form; an empty UID makes a new task in `list`. Returns {"uid"} or {"error"}.
pub(crate) fn save(qt: &QtThread, shared: &Arc<Shared>, uid: &str, edit: &str, list: &str) -> String {
    // The form as it read the task (`read`), when it says it: only what it changed since is written.
    let read: Option<TaskEdit> = serde_json::from_str::<serde_json::Value>(edit).ok().and_then(|v| v.get("read").cloned()).and_then(|r| serde_json::from_value(r).ok());
    let result = serde_json::from_str::<TaskEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        if uid.is_empty() {
            return create(qt, shared, &edit, list);
        }
        // Notes its list would cut: said, nothing written (the field keeps what was typed).
        if let Ok(task) = find(shared, uid)
            && task.notes != edit.notes
        {
            notes_fit(&task, &edit.notes)?;
        }
        change(qt, shared, uid, |text| {
            let zone = TimeZone::system();
            // Over the file as it is now: the fields the form changed since it read the task, the rest as the file has them (§5.2).
            let edit = match (&read, tasks::task_of_text(text, &zone)) {
                (Some(read), Some(now)) => TaskEdit::of(&now).changed_over(read, &edit),
                _ => edit.clone(),
            };
            tasks::apply(text, &edit, &zone, &Zoned::now())
        })?;
        write_gaps(qt, shared, uid, &edit);
        // Its blocks to come follow its title and, when it comes back, its turn (`blocks`).
        crate::blocks::follow(qt, shared, uid);
        Ok(uid.to_string())
    });
    answer(result)
}

fn answer(result: Result<String, String>) -> String {
    match result {
        Ok(uid) => serde_json::json!({ "uid": uid }).to_string(),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

/// What a typed line says, before it becomes a task: its chips, as JSON.
pub(crate) fn captured(shared: &Shared, line: &str) -> String {
    let projects: Vec<String> = loaded(shared).projects.iter().map(|c| c.id.clone()).collect();
    crate::backend::json(&capture::capture(line, Zoned::now().date(), &projects, &load_config().task_kinds(tr())))
}

/// A task from a typed line, a step of `parent` when given.
pub(crate) fn add(qt: &QtThread, shared: &Arc<Shared>, line: &str, parent: &str, list: &str) -> String {
    let projects: Vec<String> = loaded(shared).projects.iter().map(|c| c.id.clone()).collect();
    let config = load_config();
    let mut edit = capture::capture(line, Zoned::now().date(), &projects, &config.task_kinds(tr())).edit;
    edit.parent = parent.to_string();
    // In quiet time, a thought noted waits for work to come back, out of sight.
    let now = Zoned::now();
    let mode = crate::hours::mode_at(&now);
    let mut noted = String::new();
    if edit.start.is_empty()
        && parent.is_empty()
        && mode.quiet
        && let Some(back) = mode.back
    {
        edit.start = back.date().to_string();
        noted = say("task-noted-for", &[("day", day_name(back.date(), now.date()))]);
    }
    // A step goes into its bigger task's list, and its project.
    let list = match find(shared, parent) {
        Ok(bigger) => {
            for project in bigger.projects {
                if !edit.projects.contains(&project) {
                    edit.projects.push(project);
                }
            }
            bigger.list_id
        }
        Err(_) => list.to_string(),
    };
    let made = create(qt, shared, &edit, &list);
    if made.is_ok() && !noted.is_empty() {
        tell(qt, shared, noted);
    }
    answer(made)
}

/// Done, started, open again or dropped; returns what it changed, in a sentence.
pub(crate) fn set_status(qt: &QtThread, shared: &Arc<Shared>, uid: &str, status: &str) -> String {
    let status = match status {
        "completed" => Status::Completed,
        "in-process" => Status::InProcess,
        "cancelled" => Status::Cancelled,
        _ => Status::NeedsAction,
    };
    let before = Desk::new(loaded(shared), shared);
    // The step as it was, for Undo when its bigger task closes with it.
    let step_was = before.loaded.tasks.iter().find(|t| t.uid == uid).map(|t| (PathBuf::from(&t.key), std::fs::read_to_string(&t.key).ok()));
    let result = change(qt, shared, uid, |text| tasks::set_status(text, status, &TimeZone::system(), &Zoned::now()));
    if let Err(e) = result {
        return e;
    }
    // Its bigger task, without an estimate of its own, done with its last step (docs/tasks.md, "Composed tasks"): said, with Undo.
    // Only a step done for good: a repeating one's turn comes back open, and closes nothing.
    let for_good = step_was.as_ref().and_then(|(path, _)| std::fs::read_to_string(path).ok()).is_some_and(|text| sioul_core::composed::done_for_good(&text));
    if status == Status::Completed
        && for_good
        && let Ok(closing) = sioul_core::composed::close_with(&before.loaded.tasks, uid, &Zoned::now())
        && !closing.is_empty()
        && let Ok(mut files) = closing.write()
    {
        let wholes = sioul_core::composed::closing_with(&before.loaded.tasks, uid);
        let whole = wholes.first().map(|t| t.title.clone()).unwrap_or_default();
        if let Some((path, was)) = step_was {
            files.insert(0, sioul_core::agentwrites::FileChange::of(&path, was));
        }
        // Done, their blocks not begun go, as any task's (`blocks`).
        for done in std::iter::once(uid).chain(wholes.iter().map(|t| t.uid.as_str())) {
            crate::blocks::closed(qt, shared, done, Timestamp::now().as_second());
        }
        let line = say("composed-done-too", &[("whole", whole)]);
        crate::agents::say(qt, shared, sioul_core::agentwrites::Said { at: 0, line: line.clone(), files });
        let done = uid.to_string();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().task_done(QString::from(&done)));
        return line;
    }
    // Done or dropped (a repeating one's turn done): its blocks not begun go, one under way ends now (`blocks`).
    if matches!(status, Status::Completed | Status::Cancelled) {
        crate::blocks::closed(qt, shared, uid, Timestamp::now().as_second());
    }
    if status != Status::Completed {
        return String::new();
    }
    // What finishing it freed, said once (docs/tasks.md: close the loop fast).
    let after = Desk::new(Arc::new(read_loaded(shared)), shared);
    let effect = taskview::done_effect(&after.context(), &before.plan, uid);
    let line = if effect.is_empty() { tr().text("task-done-said", None) } else { format!("{} {effect}", tr().text("task-done-said", None)) };
    tell(qt, shared, line.clone());
    // "How was it?" offered on the status line, after the line that says it is done.
    let done = uid.to_string();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().task_done(QString::from(&done)));
    line
}

/// How a task was, felt ("How was it?"): today's rating, JSON {cognitive,
/// emotional, anxiety, body, gain} (null: unsaid, never the forecast copied),
/// written alone from the file as it is, whatever the window last read;
/// every value null takes today's back. Returns what went wrong, else "".
pub(crate) fn set_felt(qt: &QtThread, shared: &Arc<Shared>, uid: &str, felt: &str) -> String {
    let felt: sioul_core::demands::Demands = match serde_json::from_str(felt) {
        Ok(felt) => felt,
        Err(e) => return e.to_string(),
    };
    let now = Zoned::now();
    change(qt, shared, uid, |text| tasks::set_felt(text, &felt, now.date(), &now)).err().unwrap_or_default()
}

/// Waits for `other`, or no longer. Without a gap, DEPENDS-ON in the task
/// that waits. With one (`gap_minutes`: "the answer comes within two
/// weeks"), FINISHTOSTART with its GAP in `other`, the one that comes first,
/// as `sioul tasks import` writes it (`tasks::set_gap`, RFC 9253 §4), and no
/// DEPENDS-ON beside it. No longer: both taken away, and a FINISHTOSTART or
/// NEXT that another application wrote in `other` for the pair.
pub(crate) fn set_waits(qt: &QtThread, shared: &Arc<Shared>, uid: &str, other: &str, wait: bool, gap_minutes: i64) -> String {
    let now = Zoned::now();
    let mine = |l: &str| lines::name(l) == "RELATED-TO" && tasks::relation_of(l).kind == "DEPENDS-ON" && tasks::relation_of(l).uid == other;
    let theirs = |l: &str| lines::name(l) == "RELATED-TO" && matches!(tasks::relation_of(l).kind.as_str(), "FINISHTOSTART" | "NEXT") && tasks::relation_of(l).uid == uid;
    let result = if !wait {
        change(qt, shared, uid, |text| tasks::remove_lines(text, mine, &now)).and_then(|()| change(qt, shared, other, |text| tasks::remove_lines(text, theirs, &now)))
    } else if gap_minutes > 0 {
        change(qt, shared, other, |text| tasks::set_gap(text, uid, gap_minutes, &now)).and_then(|()| change(qt, shared, uid, |text| tasks::remove_lines(text, mine, &now)))
    } else {
        change(qt, shared, uid, |text| tasks::add_lines(text, &[tasks::relation_line("DEPENDS-ON", other, 0)], &now))
    };
    result.err().unwrap_or_default()
}

/// A task pinned to a time ("2026-10-06T14:30", your time zone) for
/// `minutes` (0: as long as its block is, else what the plan lays for it), or
/// left to the plan again (""): its time block, an event, made, moved or taken
/// away (`blocks`); its own dates stay as set (docs/tasks.md, "The plan
/// proposes; your dates stay yours", "Pinned to a time"); the day is laid
/// again at once, and "Undo" offered for ten seconds. Returns what went wrong, else "".
pub(crate) fn set_at(qt: &QtThread, shared: &Arc<Shared>, uid: &str, at: &str, minutes: u32) -> String {
    if at.is_empty() { crate::blocks::unpin(qt, shared, uid) } else { crate::blocks::pin(qt, shared, uid, at, minutes) }
}

/// A task by its UID, as last read.
pub(crate) fn find_task(shared: &Shared, uid: &str) -> Result<Task, String> {
    find(shared, uid)
}

/// A task renamed from its time block (its event's form): its title only, from its file as it is.
pub(crate) fn retitle(qt: &QtThread, shared: &Arc<Shared>, uid: &str, title: &str) -> Result<(), String> {
    change(qt, shared, uid, |text| {
        let zone = TimeZone::system();
        let task = tasks::task_of_text(text, &zone).ok_or_else(|| tr().text("task-gone", None))?;
        tasks::apply(text, &TaskEdit { title: title.to_string(), ..TaskEdit::of(&task) }, &zone, &Zoned::now())
    })
}

/// The minutes the plan lays for a task, its margins with it (`plan::Planned::laid`); None when it plans none.
pub(crate) fn laid(shared: &Shared, uid: &str) -> Option<u32> {
    // The plan as last made; none yet: made now, once.
    let planned = |plan: &Plan| plan.items.get(uid).map(|p| p.laid).filter(|m| *m > 0);
    match plan_shown() {
        Some(shown) => planned(shown.plan()),
        None => planned(&Desk::new(loaded(shared), shared).plan),
    }
}

/// Puts a task off until tomorrow.
pub(crate) fn not_now(qt: &QtThread, shared: &Arc<Shared>, uid: &str) {
    let mut today = Today::load(&Today::default_path(), Zoned::now().date());
    today.aside.insert(uid.to_string());
    let _ = today.save(&Today::default_path());
    tell(qt, shared, tr().text("task-set-aside", None));
    show_work(qt, shared);
}

/// How today is: "clear", "haze" or "fog".
pub(crate) fn set_weather(qt: &QtThread, shared: &Arc<Shared>, weather: &str) {
    let mut today = Today::load(&Today::default_path(), Zoned::now().date());
    today.weather = Weather::parse(weather);
    let _ = today.save(&Today::default_path());
    show_work(qt, shared);
}

/// Deletes a task after ten seconds to undo; its steps stay, as tasks of their own.
pub(crate) fn delete(qt: &QtThread, shared: &Arc<Shared>, uid: &str) {
    let Ok(task) = find(shared, uid) else { return };
    let path = PathBuf::from(&task.key);
    let account = account_of(&path).unwrap_or_default();
    // Its time blocks to come go with it, under the same "Undo" (`blocks`).
    let blocks = crate::blocks::with_task(uid);
    if blocks.is_empty() {
        mail::schedule_removal(qt, shared, &account, path, tr().text("undo-task-deleted", None));
    } else {
        mail::schedule_removals(qt, shared, std::iter::once((account, path)).chain(blocks).collect(), None, tr().text("undo-task-deleted", None));
    }
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    show_work(qt, shared);
}

/// Open tasks whose title holds `query`, for "waits for" and "a step of": [{uid, title, list}].
pub(crate) fn search(shared: &Shared, query: &str, except: &str) -> String {
    let fold = |t: &str| sioul_core::text::fold(t).into_iter().collect::<String>();
    let words: Vec<String> = query.split_whitespace().map(fold).collect();
    let found: Vec<serde_json::Value> = loaded(shared)
        .tasks
        .iter()
        .filter(|t| t.status.is_open() && t.uid != except && words.iter().all(|w| fold(&t.title).contains(w.as_str())))
        .take(12)
        .map(|t| serde_json::json!({ "uid": t.uid, "title": t.title, "list": t.list }))
        .collect();
    serde_json::Value::Array(found).to_string()
}

/// Makes a task list; the next sync creates it on the account's server ("local": kept here only).
pub(crate) fn new_list(qt: &QtThread, shared: &Arc<Shared>, account: &str, name: &str) -> String {
    match vdir::create(Kind::Calendars, account, name.trim(), None, &["VTODO"]) {
        Ok(list) => {
            pim::nudge(shared, account);
            show_work(qt, shared);
            format!("{}/{}", list.account, list.id)
        }
        Err(e) => e,
    }
}

/// The accounts a task list can be made in: contacts-and-calendars accounts
/// (a Google one makes it in Google Tasks), and "local".
pub(crate) fn list_accounts() -> String {
    let mut ids: Vec<String> = load_config().accounts.iter().filter(|a| a.is_dav()).map(|a| a.id.clone()).collect();
    ids.push("local".into());
    crate::backend::json(&ids)
}

// Focus.

#[derive(Serialize)]
struct FocusShown {
    task: String,
    title: String,
    start: i64,
    planned: u32,
    paused_at: Option<i64>,
    paused: i64,
    /// "Where you stopped" last time, to start from.
    stopped: String,
}

fn focus_json(desk: &Desk) -> String {
    let Some(running) = timelog::running() else { return String::new() };
    let title = desk.task(&running.task).map(|t| t.title.clone()).unwrap_or_default();
    // Where you stopped: the line left at a pause of this session first, else the last session's.
    let left = sioul_core::stopped::Stopped::load(&sioul_core::stopped::Stopped::default_path()).filter(|s| s.task == running.task && s.at >= running.start).map(|s| s.text);
    let stopped = left.or_else(|| desk.stopped.get(&running.task).cloned()).unwrap_or_default();
    crate::backend::json(&FocusShown { stopped, task: running.task, title, start: running.start, planned: running.planned, paused_at: running.paused_at, paused: running.paused })
}

/// Where you stopped, as JSON: {text, when, task, title}; "null" when no line is left.
pub(crate) fn stopped(shared: &Shared) -> String {
    let Some(stopped) = sioul_core::stopped::Stopped::load(&sioul_core::stopped::Stopped::default_path()) else { return "null".into() };
    let title = loaded(shared).tasks.iter().find(|t| t.uid == stopped.task).map(|t| t.title.clone()).unwrap_or_default();
    let when = Timestamp::from_second(stopped.at).map(|t| tr().when(&t.to_zoned(TimeZone::system()))).unwrap_or_default();
    serde_json::json!({ "text": stopped.text, "when": when, "task": stopped.task, "title": title }).to_string()
}

/// A line left on where you stopped, about the task the timer runs for, if
/// any; an empty line: done. Returns what went wrong, else "".
pub(crate) fn set_stopped(qt: &QtThread, shared: &Arc<Shared>, text: &str) -> String {
    let task = timelog::running().map(|r| r.task).unwrap_or_default();
    let problem = sioul_core::stopped::Stopped::keep(&sioul_core::stopped::Stopped::default_path(), text, &task, Timestamp::now().as_second()).err().unwrap_or_default();
    show_work(qt, shared);
    crate::backend::show(qt, shared);
    problem
}

/// Starts a focus session on a task; one already running ends first, kept.
pub(crate) fn focus_start(qt: &QtThread, shared: &Arc<Shared>, uid: &str, minutes: i32) {
    let now = Timestamp::now().as_second();
    let _ = timelog::finish(now, false);
    // 0: open-ended, counting up.
    let running = Running { task: uid.to_string(), start: now, planned: u32::try_from(minutes.max(0)).unwrap_or(25), ..Running::default() };
    if let Err(e) = timelog::keep_running(Some(&running)) {
        tell(qt, shared, e);
    }
    // Started, it is in the "Started" column.
    if find(shared, uid).is_ok_and(|t| t.status == Status::NeedsAction) {
        let _ = change(qt, shared, uid, |text| tasks::set_status(text, Status::InProcess, &TimeZone::system(), &Zoned::now()));
    }
    show_work(qt, shared);
}

/// Pauses the session, or goes on.
pub(crate) fn focus_pause(qt: &QtThread, shared: &Arc<Shared>) {
    // Over the session as the file holds it now, under its lock, which the sharing takes too.
    let _ = timelog::change_running(|running| running.toggle_pause(Timestamp::now().as_second()));
    show_work(qt, shared);
}

/// More minutes for the session.
pub(crate) fn focus_extend(qt: &QtThread, shared: &Arc<Shared>, minutes: i32) {
    let _ = timelog::change_running(|running| running.planned = running.planned.saturating_add(u32::try_from(minutes.max(0)).unwrap_or(0)));
    show_work(qt, shared);
}

/// Ends the session: kept with where you stopped; `done` marks the task done.
/// Returns what it changed, in a sentence.
pub(crate) fn focus_stop(qt: &QtThread, shared: &Arc<Shared>, done: bool, note: &str) -> String {
    let now = Timestamp::now().as_second();
    let Some(running) = timelog::running() else { return String::new() };
    // No word given: the line left at a pause of this session, if any.
    let left = sioul_core::stopped::Stopped::load(&sioul_core::stopped::Stopped::default_path()).filter(|s| s.task == running.task && s.at >= running.start).map(|s| s.text).unwrap_or_default();
    let note = if note.trim().is_empty() { left.as_str() } else { note };
    let session = match timelog::finish_with_note(now, done, note.trim()) {
        Ok(Some(session)) => session,
        Ok(None) => return String::new(),
        Err(e) => return e,
    };
    let mut args = tr().counted(session.minutes as usize);
    args.set("minutes", session.minutes.to_string());
    let kept = tr().text("focus-stopped", Some(&args));
    if done {
        let effect = set_status(qt, shared, &running.task, "completed");
        return format!("{kept} {effect}");
    }
    tell(qt, shared, kept.clone());
    show_work(qt, shared);
    kept
}

// Notes.

#[derive(Serialize)]
struct NoteRow {
    path: String,
    title: String,
    /// "text", "image", "pdf", "audio".
    kind: notes::NoteKind,
    /// Its folder in the vault, "" at the root.
    folder: String,
    tags: Vec<String>,
    modified: i64,
}

#[derive(Serialize)]
struct NotesShown {
    /// No notes folder: none is set.
    missing: bool,
    /// Every note found, by title: the window shows them as one list or as a tree.
    notes: Vec<NoteRow>,
    /// The ten notes changed last, for the top of the page.
    recent: Vec<NoteRow>,
    /// A tree of folders, as you left it; else one list.
    tree: bool,
    /// Every folder, empty ones too, for the tree.
    folders: Vec<String>,
}

fn notes_list(loaded: &Loaded, query: &str, tree: bool) -> String {
    let Some(vault) = loaded.vault.as_ref() else { return crate::backend::json(&NotesShown { missing: true, notes: Vec::new(), recent: Vec::new(), tree, folders: Vec::new() }) };
    let row = |n: &notes::Note| NoteRow { path: n.path.clone(), title: n.title.clone(), kind: n.kind, folder: n.folder().to_string(), tags: n.tags.clone(), modified: n.modified };
    let mut found: Vec<&notes::Note> = if query.trim().is_empty() { vault.notes.iter().collect() } else { vault.search(query) };
    let mut recent: Vec<&notes::Note> = found.clone();
    recent.sort_by_key(|n| std::cmp::Reverse(n.modified));
    found.sort_by_cached_key(|n| (n.title.to_lowercase(), n.path.to_lowercase()));
    let folders = if query.trim().is_empty() { vault.folders.clone() } else { Vec::new() };
    crate::backend::json(&NotesShown { missing: false, notes: found.into_iter().map(row).collect(), recent: recent.into_iter().take(10).map(row).collect(), tree, folders })
}

/// A new note in a folder of the notes ("" for the notes' own folder); returns its path.
pub(crate) fn create_note_in(qt: &QtThread, shared: &Arc<Shared>, folder: &str, title: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return String::new() };
    let title = if title.trim().is_empty() { tr().text("note-untitled", None) } else { title.to_string() };
    let folder = if folder.trim().is_empty() { notes_folder() } else { folder.to_string() };
    match notes::create(&root, &folder, &title, &notes::new_text(&title, &[], "")) {
        Ok(path) => {
            show_work(qt, shared);
            path
        }
        Err(e) => {
            tell(qt, shared, e);
            String::new()
        }
    }
}

/// A new folder in `parent`; returns what went wrong, else "".
pub(crate) fn make_folder(qt: &QtThread, shared: &Arc<Shared>, parent: &str, name: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return tr().text("error-no-store", None) };
    match notes::make_folder(&root, parent, name) {
        Ok(_) => {
            links_changed(qt, shared);
            String::new()
        }
        Err(e) => e,
    }
}

/// A folder renamed, its notes with it, and everything naming them following;
/// returns {"path"} or {"error"}.
pub(crate) fn rename_folder(qt: &QtThread, shared: &Arc<Shared>, path: &str, name: &str) -> String {
    let loaded = loaded(shared);
    let Some(vault) = loaded.vault.as_ref() else { return answer(Err(tr().text("error-no-store", None))) };
    let (new_path, moved) = match notes::with_lock(|| notes::rename_folder(vault, path, name)) {
        Ok(done) => done,
        Err(e) => return answer(Err(e)),
    };
    for (from, to) in &moved {
        retarget_note(qt, shared, &loaded, from, to);
    }
    links_changed(qt, shared);
    serde_json::json!({ "path": new_path }).to_string()
}

/// An empty folder taken out; returns what went wrong, else "".
pub(crate) fn remove_folder(qt: &QtThread, shared: &Arc<Shared>, path: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return tr().text("error-no-store", None) };
    match notes::remove_folder(&root, path) {
        Ok(()) => {
            links_changed(qt, shared);
            String::new()
        }
        Err(_) => tr().text("note-folder-not-empty", None),
    }
}

/// Notes as a tree of folders, or as one list; kept between sessions.
/// How a page was left: a column folded, a section open.
pub(crate) fn view_flag(shared: &Shared, name: &str) -> bool {
    shared.work.lock().ok().and_then(|state| state.flags.get(name).copied()).unwrap_or(false)
}

pub(crate) fn set_view_flag(shared: &Shared, name: &str, value: bool) {
    if let Ok(mut state) = shared.work.lock() {
        state.flags.insert(name.to_string(), value);
        state.save();
    }
}

pub(crate) fn set_notes_tree(qt: &QtThread, shared: &Arc<Shared>, tree: bool) {
    let query = match shared.work.lock() {
        Ok(mut state) => {
            state.notes_tree = tree;
            state.save();
            state.notes_query.clone()
        }
        Err(_) => String::new(),
    };
    let json = notes_list(&loaded(shared), &query, tree);
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_notes(QString::from(&json)));
}

/// Shows the notes matching `query`.
pub(crate) fn search_notes(qt: &QtThread, shared: &Arc<Shared>, query: &str) {
    if let Ok(mut state) = shared.work.lock() {
        state.notes_query = query.to_string();
    }
    let tree = shared.work.lock().map(|s| s.notes_tree).unwrap_or(false);
    let json = notes_list(&loaded(shared), query, tree);
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_notes(QString::from(&json)));
}

/// One note: its text, its HTML, its tags and checkboxes, and what is tied to it, as JSON.
pub(crate) fn note(shared: &Shared, path: &str) -> String {
    let loaded = loaded(shared);
    let Some(vault) = loaded.vault.as_ref() else { return String::new() };
    let Some(file) = vault.file(path) else { return String::new() };
    // A picture, a PDF, a sound: shown or played here, tied like any note.
    if let Some(media) = vault.note(path).filter(|n| n.kind != notes::NoteKind::Text) {
        let related = taskview::related_views(tr(), &loaded.world().related(&notes::uri_of(path)));
        return serde_json::json!({
            "path": path,
            "title": media.title,
            "kind": media.kind,
            "file": file.display().to_string(),
            "url": crate::backend::file_url(&file),
            "text": "",
            "html": "",
            "tags": [],
            "checkboxes": [],
            "related": related,
        })
        .to_string();
    }
    let Ok(text) = std::fs::read_to_string(&file) else { return String::new() };
    let note = notes::read(path, &text);
    let (_, body_start) = notes::front_matter(&text);
    // The title is shown above the text: a first heading saying the same is left out.
    let mut body_lines: Vec<&str> = text.lines().skip(body_start).skip_while(|l| l.trim().is_empty()).collect();
    if body_lines.first().is_some_and(|l| l.trim_start().strip_prefix("# ").is_some_and(|h| h.trim() == note.title)) {
        body_lines.remove(0);
    }
    let body = vault.with_links(note.folder(), &body_lines.join("\n"));
    let related = taskview::related_views(tr(), &loaded.world().related(&notes::uri_of(path)));
    serde_json::json!({
        "path": path,
        "title": note.title,
        "text": text,
        "html": sioul_core::compose::markdown_html(&body),
        "tags": note.tags,
        "checkboxes": note.checkboxes,
        "related": related,
        "file": file.display().to_string(),
        "stamp": text_stamp(&text),
        "kind": notes::NoteKind::Text,
        // Pictures written the usual Markdown way, relative to the note, are found from its folder.
        "base": file.parent().map(|p| format!("{}/", crate::backend::file_url(p))).unwrap_or_default(),
    })
    .to_string()
}

/// Saves a note's text; returns what went wrong, else "".
pub(crate) fn save_note(qt: &QtThread, shared: &Arc<Shared>, path: &str, text: &str, stamp: &str) -> String {
    let answer = |problem: String, path: &str, kept: String| serde_json::json!({ "problem": problem, "stamp": text_stamp(text), "path": path, "kept": kept }).to_string();
    let Some(root) = load_config().notes_root_path() else { return answer(tr().text("error-no-store", None), path, String::new()) };
    // Looked at and written with the notes' lock held: an agent tying
    // something into this note meanwhile is seen here, never written over.
    let written = notes::with_lock(|| {
        // Changed since it was opened (another device through the sharing, a sync
        // app, another editor, an agent): that version keeps the name, this one goes beside it.
        let changed = notes::normalize(path)
            .and_then(|relative| std::fs::read_to_string(root.join(relative)).ok())
            .is_some_and(|current| !stamp.is_empty() && text_stamp(&current) != stamp && current != text);
        let target = if changed { conflict_path(&root, path) } else { path.to_string() };
        notes::write(&root, &target, text).map(|_| (target, changed))
    });
    match written {
        Ok((beside, true)) => {
            show_work(qt, shared);
            answer(String::new(), &beside, say("note-changed-elsewhere", &[("path", beside.clone())]))
        }
        Ok(_) => {
            show_work(qt, shared);
            answer(String::new(), path, String::new())
        }
        Err(e) => answer(e, path, String::new()),
    }
}

/// A fingerprint of a note's text as it was read, given back when it is saved.
pub(crate) fn text_stamp(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// A free name beside a note for a version of it that lost the race:
/// "Plan (conflict 2026-10-05 21.50).md", named as the sharing names its
/// copies, the same whatever the language (`share::conflict_name`).
fn conflict_path(root: &Path, path: &str) -> String {
    let (folder, name) = path.rsplit_once('/').map_or(("", path), |(folder, name)| (folder, name));
    let stem = notes::text_stem(name);
    // Its own extension kept: a Nextcloud Notes ".txt" stays one.
    let extension = &name[stem.len()..];
    let named = sioul_sync::share::conflict_name(stem, Zoned::now().timestamp().as_millisecond());
    let join = |file: String| if folder.is_empty() { file } else { format!("{folder}/{file}") };
    let mut candidate = join(format!("{named}{extension}"));
    let mut n = 2;
    while root.join(&candidate).exists() {
        candidate = join(format!("{named} {n}{extension}"));
        n += 1;
    }
    candidate
}

/// The folder new notes go into, in the notes folder: yours, else "notes".
fn notes_folder() -> String {
    load_config().notes_folder.filter(|f| !f.trim().is_empty()).unwrap_or_else(|| NOTES_FOLDER.to_string())
}

/// Where a new audio memo is recorded: `<notes>/memos/2026-10-03 18.40.ogg`, as
/// a file URL, its folder made; "" without a notes folder.
pub(crate) fn memo_url() -> String {
    let Some(root) = load_config().notes_root_path() else { return String::new() };
    let folder = root.join(notes_folder()).join("memos");
    if std::fs::create_dir_all(&folder).is_err() {
        return String::new();
    }
    let stamp = Zoned::now().strftime("%Y-%m-%d %H.%M").to_string();
    let mut file = folder.join(format!("{stamp}.ogg"));
    let mut n = 2;
    while file.exists() {
        file = folder.join(format!("{stamp} {n}.ogg"));
        n += 1;
    }
    crate::backend::file_url(&file)
}

/// Your recordings to rest by: the sounds in a `sounds` folder of the notes
/// (its names in the languages in use: `words.tasks.sounds_folders`), as
/// JSON [{"title", "url"}].
pub(crate) fn calm_sounds(shared: &Shared) -> String {
    let loaded = loaded(shared);
    let Some(vault) = loaded.vault.as_ref() else { return "[]".into() };
    let names = sioul_core::words::current().tasks.sounds_folders.clone();
    let rows: Vec<serde_json::Value> = vault
        .notes
        .iter()
        .filter(|n| n.kind == notes::NoteKind::Audio && n.folder().split('/').any(|f| sioul_core::words::is_named(f, &names)))
        .map(|n| serde_json::json!({ "title": n.title, "url": crate::backend::file_url(&vault.root.join(&n.path)) }))
        .collect();
    crate::backend::json(&rows)
}

/// The vault path of a file URL in the notes folder: what opens a memo once recorded.
pub(crate) fn note_path_of(url: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return String::new() };
    let file = crate::backend::local_path(url);
    file.strip_prefix(&root).map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/")).unwrap_or_default()
}

/// A new note in the notes folder, its front matter linking `links`; returns its path, or "".
fn new_note(title: &str, front: &[(&str, Vec<String>)], body: &str) -> Result<String, String> {
    let root = load_config().notes_root_path().ok_or_else(|| tr().text("error-no-store", None))?;
    notes::create(&root, &notes_folder(), title, &notes::new_text(title, front, body))
}

pub(crate) fn create_note(qt: &QtThread, shared: &Arc<Shared>, title: &str, links_json: &str) -> String {
    let wanted: Vec<String> = serde_json::from_str(links_json).unwrap_or_default();
    let title = if title.trim().is_empty() { tr().text("note-untitled", None) } else { title.to_string() };
    match new_note(&title, &[("links", wanted)], "") {
        Ok(path) => {
            show_work(qt, shared);
            path
        }
        Err(e) => {
            tell(qt, shared, e);
            String::new()
        }
    }
}

// Links.

/// What a thing is tied to, both ways, as JSON.
pub(crate) fn related(shared: &Shared, uri: &str) -> String {
    let loaded = loaded(shared);
    crate::backend::json(&taskview::related_views(tr(), &loaded.world().related(uri)))
}

/// Something changed on disk (a project, time noted, an invoice): read again.
pub(crate) fn refresh(qt: &QtThread, shared: &Arc<Shared>) {
    links_changed(qt, shared);
}

/// What the links read is read again at the next question, and the open
/// pages are told to ask: a tie made or undone shows at once.
fn links_changed(qt: &QtThread, shared: &Arc<Shared>) {
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    show_work(qt, shared);
    let _ = qt.queue(|mut sioul| sioul.as_mut().links_changed());
}

/// Writes what makes or undoes a tie, and has the account of a task or an event send it.
fn rewrite(shared: &Arc<Shared>, change: &links::Rewrite) -> Result<(), String> {
    match change.kind {
        LinkKind::Note => {
            let root = load_config().notes_root_path().ok_or_else(|| tr().text("error-no-store", None))?;
            let path = change.path.strip_prefix(&root).map_err(|_| say("note-outside-notes", &[("path", change.path.display().to_string())]))?;
            notes::write(&root, &path.to_string_lossy().replace('\\', "/"), &change.text).map(|_| ())
        }
        _ => {
            vdir::write_item(&change.path, &change.text)?;
            if let Some(account) = account_of(&change.path) {
                pim::nudge(shared, &account);
            }
            Ok(())
        }
    }
}

/// Ties two things, in the one that can hold the link: a task (its list syncs
/// it to your other devices), an event whose calendar can be written, a note's
/// front matter; else the local file, as a message is never changed. Returns
/// what the line says.
pub(crate) fn link(qt: &QtThread, shared: &Arc<Shared>, from: &str, to: &str) -> String {
    let loaded = loaded(shared);
    let world = loaded.world();
    let (from, to) = (world.canonical(from), world.canonical(to));
    if from.is_empty() || to.is_empty() || from == to {
        return String::new();
    }
    // Worked out and written with the notes' lock held: the note (or task, or
    // event) is read again then, so a change an agent made meanwhile stays.
    let result = notes::with_lock(|| match loaded.tie(&from, &to, &Zoned::now()) {
        Some(change) => rewrite(shared, &change),
        None => LocalLinks::add_all(&LocalLinks::default_path(), &[links::Edge { from: from.clone(), to: to.clone(), how: "link".into() }]).map(|_| ()),
    });
    links_changed(qt, shared);
    match result {
        Ok(()) => say("link-made", &[("title", world.describe(&to).title)]),
        Err(e) => e,
    }
}

/// Undoes a tie wherever it is written: a task, an event, a note's front
/// matter, the local file. A step, a wait, a guest or a link in a note's text
/// are changed where they are: that is said.
pub(crate) fn unlink(qt: &QtThread, shared: &Arc<Shared>, a: &str, b: &str) -> String {
    let loaded = loaded(shared);
    let world = loaded.world();
    let (a, b) = (world.canonical(a), world.canonical(b));
    let mut undone = false;
    let mut problem = None;
    let kept = loaded.kept_in_meeting(&a, &b);
    // Read again and written with the notes' lock held, as `link` does.
    notes::with_lock(|| {
        for change in loaded.untie(&a, &b, &Zoned::now()) {
            match rewrite(shared, &change) {
                Ok(()) => undone = true,
                Err(e) => problem = Some(e),
            }
        }
    });
    match LocalLinks::change(&LocalLinks::default_path(), |local| local.untie(&world, &a, &b)) {
        Ok(true) => undone = true,
        Ok(false) => {}
        Err(e) => problem = Some(e),
    }
    links_changed(qt, shared);
    if let Some(problem) = problem {
        return problem;
    }
    // Written in a meeting with others, it stays there: its guests would hear of any change.
    if kept {
        return tr().text("link-kept-in-meeting", None);
    }
    if undone {
        return tr().text("link-undone", None);
    }
    // What is left is written elsewhere: said, not changed behind your back.
    let how = world.related(&a).into_iter().find(|r| r.uri == b).map(|r| r.how).unwrap_or_default();
    tr().text(
        match how.as_str() {
            "waits-for" | "unblocks" | "part-of" | "step" => "link-is-plan",
            "mentions" | "mentioned-by" => "link-in-text",
            "contact" | "involves" => "link-is-guest",
            _ => "link-not-found",
        },
        None,
    )
}

#[derive(Serialize)]
struct Found {
    uri: String,
    kind: LinkKind,
    title: String,
    detail: String,
    when: String,
    key: String,
    /// Already tied to the thing open.
    linked: bool,
}

/// Things to tie `from` to: matching `query`, of `kind` ("" for all), as JSON.
pub(crate) fn search_things(shared: &Shared, query: &str, kind: &str, from: &str) -> String {
    let loaded = loaded(shared);
    let world = loaded.world();
    let kind = match kind {
        "task" => Some(LinkKind::Task),
        "event" => Some(LinkKind::Event),
        "mail" => Some(LinkKind::Mail),
        "note" => Some(LinkKind::Note),
        "contact" => Some(LinkKind::Contact),
        "budget" => Some(LinkKind::Budget),
        "project" | "case" => Some(LinkKind::Project),
        "site" => Some(LinkKind::Site),
        _ => None,
    };
    let linked: BTreeSet<String> = world.related(from).into_iter().map(|r| r.uri).collect();
    let found = world.search(query, kind, from, 40);
    let rows: Vec<Found> = taskview::related_views(tr(), &found)
        .into_iter()
        .map(|r| Found { linked: linked.contains(&r.uri), uri: r.uri, kind: r.kind, title: r.title, detail: r.detail, when: r.when, key: r.key })
        .collect();
    crate::backend::json(&rows)
}

/// The address of a thing a page knows by its file or id: a message's file
/// ("mail"), a note's path, a task's, an event's or a contact's UID.
pub(crate) fn uri_of(kind: &str, id: &str) -> String {
    match kind {
        "mail" => message_card(id).and_then(|(_, card)| card.message_id).map(|m| links::mail_uri(&m)).unwrap_or_default(),
        "note" => notes::uri_of(id),
        "task" => links::task_uri(id),
        "event" => links::event_uri(id),
        "contact" => links::contact_uri(id),
        "project" | "case" => links::project_uri(id),
        "site" => links::site_uri(id),
        _ => id.to_string(),
    }
}

/// Something new tied to `from`: "task" or "note" (made here), "mail" (a
/// draft). `key` and `start` say which message or which occurrence, when it
/// is one. Returns {"uid"}, {"path"}, {"draft"} or {"error"}.
pub(crate) fn make_linked(qt: &QtThread, shared: &Arc<Shared>, kind: &str, from: &str, key: &str, start: f64) -> String {
    let loaded = loaded(shared);
    let world = loaded.world();
    let from = world.canonical(from);
    let source = world.describe(&from);
    let made = |field: &str, value: String| {
        if value.is_empty() { serde_json::json!({ "error": tr().text("link-not-made", None) }).to_string() } else { serde_json::json!({ field: value }).to_string() }
    };
    match (kind, links::kind_of(&from)) {
        ("task", _) => answer(linked_edit(shared, &from, key, start).and_then(|edit| create(qt, shared, &edit, ""))),
        ("note", LinkKind::Mail) => made("path", note_from_mail(qt, shared, key)),
        ("note", LinkKind::Event) => made("path", note_from_event(qt, shared, key, start)),
        ("note", _) => made("path", create_note(qt, shared, &source.title, &serde_json::json!([from]).to_string())),
        ("mail", LinkKind::Task) => made("draft", draft_for_task(qt, shared, &links::id_of(&from))),
        ("mail", LinkKind::Note) => made("draft", mail_note(qt, shared, &notes::path_of(&from).unwrap_or_default())),
        ("mail", _) => made("draft", draft_about(qt, shared, &loaded, &from)),
        _ => made("", String::new()),
    }
}

/// What a task made from `from` starts with: a message's subject, the message
/// linked, its sender and project; for an event, "Prepare: …", its day as the
/// date asked, the event linked, its projects; else the thing's title and a tie
/// to it (a contact involved, a project, a note describing it, anything related).
fn linked_edit(shared: &Shared, from: &str, key: &str, start: f64) -> Result<TaskEdit, String> {
    let loaded = loaded(shared);
    let world = loaded.world();
    let from = world.canonical(from);
    match links::kind_of(&from) {
        LinkKind::Mail => mail_task_edit(shared, key),
        LinkKind::Event => event_task_edit(shared, key, start),
        other => {
            let source = world.describe(&from);
            let mut edit = TaskEdit { title: source.title.clone(), ..TaskEdit::default() };
            match other {
                LinkKind::Contact => edit.contacts.push(ContactRef { name: source.title.clone(), uri: from.clone() }),
                LinkKind::Project => edit.projects.push(links::id_of(&from)),
                LinkKind::Note => edit.links.push(Link { uri: from.clone(), label: source.title.clone(), rel: "describedby".into() }),
                _ => edit.links.push(Link { uri: from.clone(), label: source.title.clone(), rel: "related".into() }),
            }
            Ok(edit)
        }
    }
}

/// A draft about anything: its title as subject, the people it involves as
/// recipients (an event's guests, a contact, the contacts tied to it), the
/// thing linked. Returns its id.
fn draft_about(qt: &QtThread, shared: &Arc<Shared>, loaded: &Loaded, from: &str) -> String {
    let world = loaded.world();
    let config = load_config();
    let own: BTreeSet<String> = config.accounts.iter().filter_map(|a| a.address.clone()).map(|a| a.to_lowercase()).collect();
    let address_of = |uid: &str| {
        let card = loaded.contacts.iter().find(|c| c.uid == uid)?;
        card.emails.first().map(|e| if card.name.is_empty() { e.value.clone() } else { format!("{} <{}>", card.name, e.value) })
    };
    let mut to: Vec<String> = Vec::new();
    match links::kind_of(from) {
        LinkKind::Contact => to.extend(address_of(&links::id_of(from))),
        LinkKind::Event => {
            if let Some(event) = loaded.events.iter().find(|e| e.uid == links::id_of(from)) {
                to.extend(event.people.iter().filter(|p| !own.contains(&p.to_lowercase())).cloned());
            }
        }
        _ => {}
    }
    for tied in world.related(from).into_iter().filter(|r| r.kind == LinkKind::Contact) {
        to.extend(address_of(&links::id_of(&tied.uri)));
    }
    let mut seen = BTreeSet::new();
    to.retain(|t| seen.insert(t.to_lowercase()));
    let account = config.accounts.iter().find(|a| a.syncs()).map(|a| a.id.clone()).unwrap_or_default();
    let mut draft = Draft::new(&account);
    draft.subject = if links::kind_of(from) == LinkKind::Contact { String::new() } else { world.describe(from).title };
    draft.to = to;
    draft.links.push(from.to_string());
    if draft.save().is_err() {
        return String::new();
    }
    links_changed(qt, shared);
    draft.id
}

// Things made from other things.

/// The message's card, and its file.
fn message_card(key: &str) -> Option<(PathBuf, Card)> {
    let path = maildir::locate(Path::new(key))?;
    let card = Card::from_bytes(&std::fs::read(&path).ok()?)?;
    Some((path, card))
}

/// The sender of a message as a task's CONTACT: their card when they have one.
fn sender_contact(loaded: &Loaded, card: &Card) -> Option<ContactRef> {
    let address = card.from_address.clone()?;
    match contacts::by_address(&loaded.contacts, &address) {
        Some(contact) => Some(ContactRef { name: contact.name.clone(), uri: links::contact_uri(&contact.uid) }),
        None => Some(ContactRef { name: card.from_name.clone().map_or(address.clone(), |n| format!("{n} <{address}>")), uri: String::new() }),
    }
}

/// A task from a message: its subject, the message linked, its sender, its project.
pub(crate) fn task_from_mail(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    answer(mail_task_edit(shared, key).and_then(|edit| create(qt, shared, &edit, "")))
}

/// A message's task, before it is made.
fn mail_task_edit(shared: &Shared, key: &str) -> Result<TaskEdit, String> {
    let Some((path, card)) = message_card(key) else { return Err(tr().text("mail-message-gone", None)) };
    let loaded = loaded(shared);
    let mut edit = TaskEdit { title: card.subject.trim().to_string(), ..TaskEdit::default() };
    if let Some(id) = &card.message_id {
        edit.links.push(Link { uri: links::mail_uri(id), label: String::new(), rel: "via".into() });
    }
    edit.contacts.extend(sender_contact(&loaded, &card));
    let config = load_config();
    if let Some(store) = config.notes_root_path().and_then(|r| ProjectStore::load(&r).ok()) {
        // Its project as the Porch would file it: none for mail it sets
        // aside, none by its sender's address when nothing authenticates it.
        let stored = Card { path: Some(path), ..card };
        let gate = sioul_core::porch::Gate::load(&config, Zoned::now().timestamp().as_second());
        if let Some(seen) = gate.admission(&stored).and_then(|a| a.for_routes(&stored)) {
            edit.projects.extend(store.route(&seen).first().map(|r| r.project.id.clone()));
        }
    }
    Ok(edit)
}

/// A note from a message: its subject as title, the message linked, its text quoted.
pub(crate) fn note_from_mail(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    let Some((_, card)) = message_card(key) else { return String::new() };
    let mail = card.message_id.as_deref().map(links::mail_uri).into_iter().collect();
    let quoted: String = card.excerpt.lines().take(40).map(|l| format!("> {l}\n")).collect();
    let body = format!("{}\n\n{quoted}", say("note-from-mail", &[("sender", card.sender().to_string())]));
    match new_note(&card.subject, &[("mail", mail)], &body) {
        Ok(path) => {
            show_work(qt, shared);
            path
        }
        Err(e) => {
            tell(qt, shared, e);
            String::new()
        }
    }
}

/// An event's file and its ties.
fn event_ref(shared: &Shared, key: &str) -> Option<agenda::EventRef> {
    loaded(shared).events.iter().find(|e| e.key == key).cloned()
}

/// A note for an event: dated, the guests listed, linked both ways: the
/// note's front matter always, the event's LINK only where a tie may be
/// written (`links::Loaded::tie_into_event`: its calendar can be written,
/// and it is no meeting with others, whose server would tell every guest).
pub(crate) fn note_from_event(qt: &QtThread, shared: &Arc<Shared>, key: &str, start: f64) -> String {
    let Some(event) = event_ref(shared, key) else { return String::new() };
    let at = Timestamp::from_second(start as i64).map(|t| t.to_zoned(TimeZone::system())).unwrap_or_else(|_| Zoned::now());
    let title = format!("{} {}", at.date(), event.summary);
    let people: String = event.people.iter().map(|p| format!("- {p}\n")).collect();
    let body = if people.is_empty() { String::new() } else { format!("{}\n\n{people}", tr().text("note-guests", None)) };
    let path = match new_note(&title, &[("event", vec![links::event_uri(&event.uid)])], &body) {
        Ok(path) => path,
        Err(e) => {
            tell(qt, shared, e);
            return String::new();
        }
    };
    if let Some(change) = loaded(shared).tie_into_event(&event.uid, &notes::uri_of(&path), &Zoned::now()) {
        let _ = rewrite(shared, &change);
    }
    show_work(qt, shared);
    path
}

/// A task to prepare an event: its date the event's day, the event linked.
pub(crate) fn task_from_event(qt: &QtThread, shared: &Arc<Shared>, key: &str, start: f64) -> String {
    answer(event_task_edit(shared, key, start).and_then(|edit| create(qt, shared, &edit, "")))
}

/// The task to prepare an event (the occurrence at `start`), before it is made.
fn event_task_edit(shared: &Shared, key: &str, start: f64) -> Result<TaskEdit, String> {
    let Some(event) = event_ref(shared, key) else { return Err(tr().text("agenda-gone", None)) };
    let day = Timestamp::from_second(start as i64).map(|t| t.to_zoned(TimeZone::system()).date().to_string()).unwrap_or_default();
    Ok(TaskEdit {
        title: say("task-prepare", &[("title", event.summary.clone())]),
        due: day,
        links: vec![Link { uri: format!("uid:{}", event.uid), label: event.summary.clone(), rel: "related".into() }],
        projects: event.projects.clone(),
        ..TaskEdit::default()
    })
}

/// A task from a checkbox line of a note: its text, the note linked.
pub(crate) fn task_from_line(qt: &QtThread, shared: &Arc<Shared>, path: &str, line: i32) -> String {
    let loaded = loaded(shared);
    let Some(note) = loaded.vault.as_ref().and_then(|v| v.note(path)) else { return answer(Err(tr().text("note-gone", None))) };
    let Some(checkbox) = note.checkboxes.iter().find(|c| c.line == line as usize) else { return answer(Err(tr().text("note-gone", None))) };
    let edit = TaskEdit { title: checkbox.text.clone(), links: vec![Link { uri: notes::uri_of(path), label: note.title.clone(), rel: "describedby".into() }], ..TaskEdit::default() };
    answer(create(qt, shared, &edit, ""))
}

/// A draft to the people a task involves, linked both ways; returns its id.
pub(crate) fn draft_for_task(qt: &QtThread, shared: &Arc<Shared>, uid: &str) -> String {
    let Ok(task) = find(shared, uid) else { return String::new() };
    let loaded = loaded(shared);
    let config = load_config();
    let account = config.accounts.iter().find(|a| a.syncs()).map(|a| a.id.clone()).unwrap_or_default();
    let mut draft = Draft::new(&account);
    draft.subject = task.title.clone();
    draft.to = task
        .contacts
        .iter()
        .filter_map(|c| {
            let card = loaded.contacts.iter().find(|x| links::contact_uri(&x.uid) == c.uri)?;
            card.emails.first().map(|e| if card.name.is_empty() { e.value.clone() } else { format!("{} <{}>", card.name, e.value) })
        })
        .collect();
    draft.links.push(links::task_uri(uid));
    if draft.save().is_err() {
        return String::new();
    }
    let link = tasks::link_line(&Link { uri: links::draft_uri(&draft.id), label: String::new(), rel: "related".into() });
    let _ = change(qt, shared, uid, |text| tasks::add_lines(text, &[link], &Zoned::now()));
    draft.id
}

/// A draft sending a note to the guests of the event it is about; returns its id.
pub(crate) fn mail_note(qt: &QtThread, shared: &Arc<Shared>, path: &str) -> String {
    let loaded = loaded(shared);
    let Some(note) = loaded.vault.as_ref().and_then(|v| v.note(path)).cloned() else { return String::new() };
    let Some(file) = loaded.vault.as_ref().and_then(|v| v.file(path)) else { return String::new() };
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let (_, body_start) = notes::front_matter(&text);
    // Without the front matter and the title: the text itself.
    let body: Vec<&str> = text.lines().skip(body_start).collect();
    let body = body.iter().skip_while(|l| l.trim().is_empty() || l.trim_start().starts_with("# ")).copied().collect::<Vec<_>>().join("\n");
    let config = load_config();
    let own: BTreeSet<String> = config.accounts.iter().filter_map(|a| a.address.clone()).map(|a| a.to_lowercase()).collect();
    let guests: Vec<String> = note
        .links
        .iter()
        .filter(|l| links::kind_of(&l.target) == LinkKind::Event)
        .filter_map(|l| loaded.events.iter().find(|e| links::event_uri(&e.uid) == l.target))
        .flat_map(|e| e.people.clone())
        .filter(|p| !own.contains(&p.to_lowercase()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let account = config.accounts.iter().find(|a| a.syncs()).map(|a| a.id.clone()).unwrap_or_default();
    let mut draft = Draft::new(&account);
    draft.subject = note.title.clone();
    draft.to = guests;
    draft.body = body;
    draft.links.push(notes::uri_of(path));
    if draft.save().is_err() {
        return String::new();
    }
    show_work(qt, shared);
    draft.id
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A category renamed, or a note's link followed, over a task's file as it
    /// is now: a step ticked there since the tasks were read stays ticked, and
    /// only the field changed is written; the copy read before, written whole,
    /// would take the tick back (review of 85da7f0, later 2).
    #[test]
    fn a_rename_writes_only_what_it_changes() {
        let zone = TimeZone::system();
        let now = Zoned::now();
        let edit = TaskEdit { title: "Sort the papers".into(), notes: "- [ ] the bank's letter".into(), categories: vec!["Home".into()], links: vec![Link { uri: "sioul:note/old.md".into(), ..Link::default() }], estimate: 20, ..TaskEdit::default() };
        let text = tasks::new_task(&edit, "rename-test", &zone, &now).unwrap();
        let read = TaskEdit::of(&tasks::task_of_text(&text, &zone).unwrap());
        // Ticked on another device after the tasks were read.
        let ticked = tasks::apply(&text, &TaskEdit { notes: "- [x] the bank's letter".into(), ..read.clone() }, &zone, &now).unwrap();
        let renamed = tasks::task_of_text(&changed_in_file(&ticked, |e| e.categories = vec!["House".into()]).unwrap(), &zone).unwrap();
        assert!(renamed.notes.contains("- [x]") && renamed.categories == vec!["House".to_string()], "{renamed:?}");
        let followed = tasks::task_of_text(&changed_in_file(&ticked, |e| e.links[0].uri = "sioul:note/new.md".into()).unwrap(), &zone).unwrap();
        assert!(followed.notes.contains("- [x]") && followed.links.iter().any(|l| l.uri == "sioul:note/new.md"), "{followed:?}");
        // As before: the copy read, written whole, takes the tick back.
        let whole = tasks::task_of_text(&tasks::apply(&ticked, &TaskEdit { categories: vec!["House".into()], ..read }, &zone, &now).unwrap(), &zone).unwrap();
        assert!(whole.notes.contains("- [ ]"));
    }

    /// Today laid again (`relay_today`) reads no store and keeps the plan as
    /// made (its read, its plan, its settings), only today laid anew; a plan
    /// older than a write wants the plan made again instead (the unified
    /// agenda, §3.9).
    #[test]
    fn today_laid_again_reads_no_store() {
        sioul_core::config::stand_ins_only();
        let shared = Arc::new(Shared::default());
        assert!(!relaid(&shared), "no plan yet: made instead");
        // Made again while another test wrote meanwhile (the count and the folders' times are the process's).
        let mut laid = false;
        for _ in 0..40 {
            publish_now(&shared);
            let made = plan_shown().unwrap();
            LOADS.with(|n| n.set(0));
            if relaid(&shared) {
                assert_eq!(LOADS.with(std::cell::Cell::get), 0, "nothing read again");
                let again = plan_shown().unwrap();
                assert!(Arc::ptr_eq(&made.desk, &again.desk) && made.stamp == again.stamp, "the same plan, the same read");
                laid = true;
                break;
            }
        }
        assert!(laid, "laid again from a current plan");
        let calendar = sioul_core::vdir::create(sioul_core::vdir::Kind::Calendars, sioul_core::vdir::LOCAL, "Relay test", None, &["VEVENT"]).unwrap();
        sioul_core::vdir::write_item(&calendar.dir.join("r.ics"), "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:relay-test\r\nDTSTART:20991231T100000Z\r\nDTEND:20991231T110000Z\r\nSUMMARY:relay-test\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        assert!(!relaid(&shared), "a write since: the plan made again instead");
        forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// Another day's meals move past a pinned block where it is now: a plan
    /// older than a write (the block moved since) never lends its pins
    /// (review of 3a2fbd4, later 10).
    #[test]
    fn another_day_is_held_where_its_blocks_are_now() {
        sioul_core::config::stand_ins_only();
        let shared = Arc::new(Shared::default());
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let calendar = sioul_core::vdir::create(sioul_core::vdir::Kind::Calendars, sioul_core::vdir::LOCAL, "Pins held", None, &["VEVENT", "VTODO"]).unwrap();
        let tomorrow = now.date().tomorrow().unwrap();
        let at = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let edit = TaskEdit { title: "Pinned elsewhere".into(), estimate: 45, ..TaskEdit::default() };
        sioul_core::vdir::write_item(&calendar.dir.join("pinned-task.ics"), &tasks::new_task(&edit, "pins-held-task", &zone, &now).unwrap()).unwrap();
        let block = |hour: i8| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:pins-held-block\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Pinned elsewhere\r\n{}:pins-held-task\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(hour), utc(hour + 1), sioul_core::blocks::TASK);
        let block_path = calendar.dir.join("pins-held-block.ics");
        sioul_core::vdir::write_item(&block_path, &block(12)).unwrap();
        publish_now(&shared);
        assert!(plan_shown().is_some_and(|s| s.plan().pins.values().any(|p| p.start == at(12))), "the plan pins it at 12:00");
        // Moved to 14:00 by its file (a drag, the server): the plan is older than the write.
        sioul_core::vdir::write_item(&block_path, &block(14)).unwrap();
        let (from, to) = near_span(tomorrow, &zone);
        let held = held_from(tomorrow, &now, sioul_core::agenda::occurrences(from, to));
        let holds = |t: i64| held.iter().any(|&(a, b)| a <= t && t < b);
        assert!(holds(at(14) + 1800), "held where the block is now: {held:?}");
        assert!(!holds(at(12) + 1800), "never where the older plan had it: {held:?}");
        forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// A re-plan stays due until its publish, or until its run ends however
    /// it ends; one asked meanwhile keeps it due (review of 3a2fbd4, later 4).
    /// Its run ended without a publish, it is said so, and the agenda is
    /// asked again (review of 590eb18, later 4); published, said once.
    #[test]
    fn a_replan_is_due_until_its_publish() {
        let shared = Shared::default();
        let ended = std::cell::RefCell::new(Vec::new());
        replan_asked(&shared);
        drop(ReplanDue::new(&shared, |published| ended.borrow_mut().push(published)));
        assert!(!shared.replan_due.load(Ordering::SeqCst), "its run ended: no more due");
        assert_eq!(*ended.borrow(), [false], "ended without a publish: the agenda asked again");
        ended.borrow_mut().clear();
        replan_asked(&shared);
        let mut due = ReplanDue::new(&shared, |published| ended.borrow_mut().push(published));
        due.published();
        drop(due);
        assert!(!shared.replan_due.load(Ordering::SeqCst));
        assert_eq!(*ended.borrow(), [true], "published: said once, the agenda not asked again");
        ended.borrow_mut().clear();
        replan_asked(&shared);
        shared.work_replan.store(true, Ordering::SeqCst);
        drop(ReplanDue::new(&shared, |published| ended.borrow_mut().push(published)));
        assert!(shared.replan_due.load(Ordering::SeqCst), "another asked meanwhile: still due");
        assert!(ended.borrow().is_empty(), "nothing said: the next re-plan says it");
    }

    #[test]
    fn a_note_saved_over_a_newer_one_goes_beside_it() {
        let root = std::env::temp_dir().join(format!("sioul-note-beside-{}", std::process::id()));
        std::fs::create_dir_all(root.join("admin")).unwrap();
        std::fs::write(root.join("admin/Plan.md"), "theirs").unwrap();
        // The stamp tells a text from another, and the same text from itself.
        assert_eq!(text_stamp("theirs"), text_stamp("theirs"));
        assert_ne!(text_stamp("theirs"), text_stamp("ours"));
        // Beside it, in its folder, never over another copy.
        let first = conflict_path(&root, "admin/Plan.md");
        assert!(first.starts_with("admin/Plan (") && first.ends_with(".md"), "{first}");
        std::fs::write(root.join(&first), "ours").unwrap();
        let second = conflict_path(&root, "admin/Plan.md");
        assert!(second != first && second.ends_with(" 2.md"), "{second}");
        assert!(!conflict_path(&root, "Top.md").contains('/'));
        assert!(conflict_path(&root, "Shopping.txt").ends_with(".txt"), "a Nextcloud Notes note keeps its extension");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The plan as last made is what the agenda, a task's card and `laid`
    /// read (`PlanShown`): nothing planned again on the window's thread.
    /// While nothing was written since it was read, the agenda takes its
    /// events from it; a write counted makes it older, and the agenda reads
    /// the calendars itself.
    #[test]
    fn the_plan_as_last_made_is_read_until_a_write() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let shared = Arc::new(Shared::default());
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let calendar = sioul_core::vdir::create(sioul_core::vdir::Kind::Calendars, sioul_core::vdir::LOCAL, "Snapshot calendar", None, &["VEVENT", "VTODO"]).unwrap();
        let tomorrow = now.date().tomorrow().unwrap();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let event = |uid: &str, hour: i8| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{uid}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(hour), utc(hour + 1));
        let edit = TaskEdit { title: "Snapshot task".into(), estimate: 20, ..TaskEdit::default() };
        let task_path = calendar.dir.join("snapshot-task.ics");
        sioul_core::vdir::write_item(&task_path, &tasks::new_task(&edit, "snapshot-task", &zone, &now).unwrap()).unwrap();
        std::fs::write(calendar.dir.join("first.ics"), event("snapshot-first", 9)).unwrap();
        // Made and published, current: made again while another test wrote meanwhile (the
        // count and the folders' times are the process's).
        let make_current = || {
            for _ in 0..40 {
                let stamp = sioul_core::timeline::Stamp::now();
                let desk = Desk::new(Arc::new(read_loaded(&shared)), &shared);
                publish(PlanShown { stamp, made_on: now.date(), desk: Arc::new(desk), today: Default::default() });
                if plan_shown().is_some_and(|s| s.current()) {
                    return true;
                }
            }
            false
        };
        assert!(make_current());
        let midnight = now.date().to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let none = (BTreeSet::new(), BTreeSet::new());
        let seen = |uid: &str| crate::agenda::occurrences_between(midnight, midnight + 3 * 86_400, &none.0, &none.1, &[]).iter().any(|o| o.uid == uid);
        assert!(seen("snapshot-first"));
        // A task's card takes the task's own fields from its file, never the plan's older copy:
        // its title rewritten through a door after the plan was made is the card's at once
        // (review of 1880bce, M1), the plan still the old one.
        let renamed = TaskEdit { title: "Snapshot task, renamed".into(), estimate: 20, ..TaskEdit::default() };
        sioul_core::vdir::write_item(&task_path, &tasks::new_task(&renamed, "snapshot-task", &zone, &now).unwrap()).unwrap();
        let shown: serde_json::Value = serde_json::from_str(&task(&shared, "snapshot-task")).unwrap();
        assert_eq!(shown["edit"]["title"], "Snapshot task, renamed");
        assert_eq!(plan_shown().and_then(|s| s.desk.task("snapshot-task").map(|t| t.title.clone())).as_deref(), Some("Snapshot task"));
        assert!(plan_shown().is_some_and(|s| !s.current()), "a write counted here: older");
        // Another day's held time: never from the older plan's read; the day read again (review of 1880bce, later 4).
        assert!(held_by_plan(tomorrow, &now).is_none(), "an older plan's read is never used");
        HELD_READS.with(|n| n.set(0));
        let _ = held_on(tomorrow, &now);
        assert_eq!(HELD_READS.with(std::cell::Cell::get), 1, "read again");
        // As another program writes (no count of this one's): older all the same, by the folder's
        // time; the agenda reads the calendars (review of 1880bce, L3).
        assert!(make_current());
        let written = sioul_core::timeline::written();
        std::fs::write(calendar.dir.join("second.ics"), event("snapshot-second", 11)).unwrap();
        assert_eq!(sioul_core::timeline::written(), written, "no door of this program");
        assert!(plan_shown().is_some_and(|s| !s.current()), "another program's write makes it older");
        assert!(seen("snapshot-second"));
        // A deletion waiting for its "Undo": left out by the agenda's read, by the plan's read
        // and by today's layout; undone, back at once, from the same plan (review of 1880bce, L1).
        let stamp = sioul_core::timeline::Stamp::now();
        let desk = Desk::new(Arc::new(read_loaded(&shared)), &shared);
        publish(PlanShown { stamp, made_on: now.date(), desk: Arc::new(desk), today: Default::default() });
        let first = calendar.dir.join("first.ics");
        // One of today's too, at 00:10: today's layout shows each event of the day.
        let this_morning = |minute: i64| now.date().at(0, 0, 0, 0).to_zoned(zone.clone()).unwrap().checked_add(jiff::Span::new().minutes(minute)).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let today_event = calendar.dir.join("today.ics");
        sioul_core::vdir::write_item(&today_event, &format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:snapshot-today\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:snapshot-today\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", this_morning(10), this_morning(40))).unwrap();
        let stamp = sioul_core::timeline::Stamp::now();
        let desk = Desk::new(Arc::new(read_loaded(&shared)), &shared);
        assert!(today_laid_out(&desk, &shared).blocks.iter().any(|b| b.key == today_event.display().to_string()), "today's event in today's layout");
        publish(PlanShown { stamp, made_on: now.date(), desk: Arc::new(desk), today: Default::default() });
        crate::mail::wait_to_remove(&shared, first.clone());
        crate::mail::wait_to_remove(&shared, today_event.clone());
        let (removed, skipped) = crate::mail::hidden_pim(&shared);
        let without = crate::agenda::occurrences_between(midnight, midnight + 3 * 86_400, &removed, &skipped, &[]);
        assert!(!without.iter().any(|o| o.uid == "snapshot-first"), "waiting for Undo: not in the agenda");
        let planned = Desk::new(Arc::new(read_loaded(&shared)), &shared);
        assert!(planned.read.iter().any(|o| o.uid == "snapshot-first"), "the plan's read is kept as read");
        assert!(!today_laid_out(&planned, &shared).blocks.iter().any(|b| b.key == today_event.display().to_string()), "nor in today's layout");
        shared.pending.lock().unwrap().clear();
        assert!(seen("snapshot-first"), "undone: back at once, from the same plan");
        PLAN_SHOWN.with(|plan| *plan.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
}


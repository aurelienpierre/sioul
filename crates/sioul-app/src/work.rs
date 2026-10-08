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
use sioul_core::cases::CaseStore;
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
/// case, one kind and category. All but the searches are kept between sessions.
#[derive(Clone, Default, Serialize, serde::Deserialize)]
pub(crate) struct WorkState {
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub done: bool,
    #[serde(skip)]
    pub query: String,
    #[serde(default)]
    pub case: String,
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

/// Reads everything again, leaving out what waits ten seconds to be deleted.
fn read_loaded(shared: &Shared) -> Loaded {
    let mut fresh = Loaded::read(&load_config());
    let (removed, _) = mail::hidden_pim(shared);
    fresh.tasks.retain(|t| !removed.contains(Path::new(&t.key)));
    // An event deleted, a task's time block left to the plan: gone from what things are tied to, too.
    fresh.events.retain(|e| !removed.contains(Path::new(&e.key)));
    fresh
}

/// How far ahead events take their time out of the plan's room: four weeks.
const EVENTS_AHEAD: i64 = 28 * 86_400;

/// The room for tasks: your hours, each kind for its own tasks, less the
/// events of the coming weeks and the blocks tasks are pinned to; today's
/// from now, scaled by the weather.
fn settings(weather: Weather, situation: &sioul_core::quiet::Situation, cases: &[sioul_core::cases::Case], tasks: &[Task], shared: &Shared) -> Settings {
    let config = load_config();
    let now = Zoned::now();
    let midnight = now.date().to_zoned(now.time_zone().clone()).map_or(0, |z| z.timestamp().as_second());
    let read = sioul_core::agenda::occurrences(midnight, midnight + EVENTS_AHEAD);
    // The tasks pinned to a time: a block is its task's time, never an event of its own (docs/tasks.md, "Pinned to a time").
    let pins = crate::blocks::pins(shared, &read, tasks, &now);
    // Meals move past the blocks as past any event, as the Health page moves them.
    let held = sioul_core::plan::event_spans(&read, 0);
    let events = sioul_core::blocks::without_blocks(read, tasks);
    // Meals, naps and the night first: the work goes around them, as each day has them.
    let needs = sioul_core::health::Health::load(&sioul_core::health::Health::default_path()).needs;
    let days = crate::health::days();
    // Today's meals pushed past the events they would fall in (another day's: `with_events`).
    let pushed = needs.past_events_on(now.date(), now.time_zone(), &days, &held);
    // Today's end of work moved by free time (docs/pauses.md): its room for light steps only.
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let day = sioul_core::pause::Day::of(&config, crate::hours::blocks(&now), &events);
    let extension = sioul_core::pause::moved_today(&overrides, &day.evening(), &now, config.free_time.moves);
    let mut settings = Settings::of_hours(&config.week_hours(), sioul_core::areas::TaskAreas::of_config(&config, cases)).with_needs(&needs, pushed).with_days(days).with_extension(extension).with_pins(pins).with_events(&now, &events);
    settings.default_estimate = config.tasks.estimate.unwrap_or(settings.default_estimate);
    // Today by its weather, and no heavier than a hazy day after a pause (docs/pauses.md).
    (settings.today_percent, settings.heavy_today) = sioul_core::pause::today_level(&overrides, now.date(), weather);
    settings.closed = situation.closed.clone();
    settings.office_days = situation.office_days;
    settings.office_hours = config.office_hours();
    settings
}

/// Asleep, the tasks all the same: the page asked ("Show anyway"), until it is left.
static SHOWN_ANYWAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn show_anyway(qt: &QtThread, shared: &Arc<Shared>, on: bool) {
    if SHOWN_ANYWAY.swap(on, Ordering::Relaxed) != on {
        show_work(qt, shared);
    }
}

/// The moment, for the task pages: quiet or not, offices open or not.
pub(crate) fn situation(cases: &[sioul_core::cases::Case]) -> sioul_core::quiet::Situation {
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let now = Zoned::now();
    sioul_core::quiet::Situation::now(&load_config(), &overrides, &crate::hours::blocks(&now), &now, tr(), cases)
}

/// The plan, and what it needs, from what was read.
struct Desk {
    loaded: Arc<Loaded>,
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
}

impl Desk {
    fn new(loaded: Arc<Loaded>, shared: &Shared) -> Desk {
        let date = Zoned::now().date();
        let today = Today::load(&Today::default_path(), date);
        let sessions = timelog::sessions();
        let spent = timelog::spent(&sessions, 0, i64::MAX);
        let stopped = sessions.iter().filter(|s| !s.note.is_empty()).map(|s| (s.task.clone(), s.note.clone())).collect();
        let situation = situation(&loaded.cases);
        // What your record says: ratings said after tasks, corrected lengths, what a day holds (docs/capacity.md).
        let settings = crate::capacity::planned(settings(today.weather, &situation, &loaded.cases, &loaded.tasks, shared), &loaded.tasks, &sessions);
        let plan = plan::plan(&loaded.tasks, date, &settings, &spent, &today.aside);
        // Asleep no task shows, unless the page asked to see them all.
        let anyway = situation.mode.sleeps() && SHOWN_ANYWAY.load(Ordering::Relaxed);
        let filter = Filter { quiet: if anyway { None } else { situation.quiet_tasks() }, ..Filter::default() };
        let quiet = situation.mode.quiet;
        Desk { loaded, filter, offices: situation.offices, plan, today, sessions, spent, stopped, settings, quiet }
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
        Context { filter: &self.filter, offices: self.offices.clone(), tasks: &self.loaded.tasks, plan: &self.plan, today: Zoned::now().date(), tr: tr(), cases: &self.loaded.cases, spent: &self.spent, stopped: &self.stopped }
    }

    fn task(&self, uid: &str) -> Option<&Task> {
        self.loaded.tasks.iter().find(|t| t.uid == uid)
    }
}

#[derive(Serialize)]
struct ListChoice {
    id: String,
    name: String,
    color: Option<String>,
    /// Waits to be created on the server.
    pending: bool,
    /// It keeps tasks; a calendar that does not is shown, greyed, with why.
    tasks: bool,
}

#[derive(Serialize)]
struct CaseChoice {
    id: String,
    title: String,
}

#[derive(Serialize)]
struct KindChoice {
    id: String,
    label: String,
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
    cases: Vec<CaseChoice>,
    /// No list can take tasks yet.
    no_list: bool,
    /// Work time or quiet time.
    quiet: bool,
    /// "Monday. The plan starts with: …", once work is back, on its day.
    first_step: String,
    first_step_task: String,
    /// The next date asked within a week: the time budget until then, else "".
    budget: String,
    /// Today laid out: events at their times, today's steps in the gaps.
    day: sioul_core::dayview::DayView,
    /// In free time, leisure offered, never a list to finish (docs/pauses.md).
    free: Vec<serde_json::Value>,
}

/// Today, laid out: the events of today and the steps the plan gives today, those the page shows.
fn today_laid_out(desk: &Desk, shared: &Shared) -> sioul_core::dayview::DayView {
    let now = Zoned::now();
    let midnight = now.date().to_zoned(now.time_zone().clone()).map_or(0, |z| z.timestamp().as_second());
    // Until the next midnight: a day of 23 or 25 hours when the clocks change.
    let next_midnight = now.date().tomorrow().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()).map_or(midnight + 86_400, |z| z.timestamp().as_second());
    // Events deleted, or an occurrence left out, while "Undo" is offered: gone already.
    let (removed, skipped) = crate::mail::hidden_pim(shared);
    let events: Vec<sioul_core::agenda::Occurrence> = sioul_core::agenda::occurrences(midnight, next_midnight)
        .into_iter()
        .filter(|o| !removed.contains(std::path::Path::new(&o.key)) && !skipped.contains(&(std::path::PathBuf::from(&o.key), o.start)))
        .collect();
    let shown: Vec<Task> = desk.loaded.tasks.iter().filter(|t| desk.filter.quiet.as_ref().is_none_or(|q| q.keeps(t))).cloned().collect();
    let mut day = sioul_core::dayview::day(&now, &events, &desk.plan, &shown, &desk.settings);
    // Meals and naps without a name of their own: their usual one, in your language.
    for block in day.blocks.iter_mut().filter(|b| (b.kind == "meal" || b.kind == "nap") && b.title.trim().is_empty()) {
        block.title = crate::health::block_name(block.kind, &block.key, "");
    }
    // Time for you, the free time kept, and why today holds what it holds, in words.
    crate::capacity::dress(&mut day, &desk.loaded.tasks);
    // The steps as laid, for the Health page's day (faded, for context).
    if let Ok(mut steps) = DAY_STEPS.lock() {
        *steps = day.blocks.iter().filter(|b| b.kind == "task").map(|b| (b.start, b.end, b.title.clone())).collect();
    }
    day
}

/// Today's steps as the day's layout last laid them: (start, end, title).
static DAY_STEPS: std::sync::Mutex<Vec<(i64, i64, String)>> = std::sync::Mutex::new(Vec::new());

/// Today's steps as last laid out, for the Health page's day: faded, for context.
pub(crate) fn day_steps() -> Vec<(i64, i64, String)> {
    DAY_STEPS.lock().map(|steps| steps.clone()).unwrap_or_default()
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
    let mut routines = load_config().routines;
    let id = if id.is_empty() {
        let taken: Vec<String> = routines.iter().map(|r| r.id.clone()).chain(std::iter::once("admin".to_string())).collect();
        sioul_core::cases::new_id(title, &taken)
    } else {
        id.to_string()
    };
    let routine = sioul_core::routines::Routine { id: id.clone(), title: title.to_string(), auto, steps };
    match routines.iter_mut().find(|r| r.id == id) {
        Some(place) => *place = routine,
        None => routines.push(routine),
    }
    sioul_core::routines::save(&crate::backend::config_path(), &routines).err().unwrap_or_default()
}

/// A routine taken out; returns what went wrong, else "".
pub(crate) fn remove_routine(id: &str) -> String {
    let mut routines = load_config().routines;
    routines.retain(|r| r.id != id);
    sioul_core::routines::save(&crate::backend::config_path(), &routines).err().unwrap_or_default()
}

/// The lists a task can go into.
fn writable_lists() -> Vec<ListChoice> {
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
    for task in family {
        let from = Path::new(&task.key);
        if from.parent() == Some(target.dir.as_path()) {
            continue;
        }
        let moved = std::fs::read_to_string(from).map_err(|e| e.to_string()).and_then(|text| vdir::write_item(&target.dir.join(format!("{}.ics", vdir::new_name())), &text)).and_then(|()| std::fs::remove_file(from).map_err(|e| e.to_string()));
        if let Err(e) = moved {
            return answer(Err(e));
        }
        accounts.extend(account_of(from));
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
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.work_job, move |shared| {
        let generation = shared.work_generation.fetch_add(1, Ordering::Relaxed) + 1;
        // Today's steps as laid before: the Health page's day shows them faded.
        let steps_before = day_steps();
        let fresh = Arc::new(read_loaded(shared));
        if let Ok(mut cache) = shared.loaded.lock() {
            *cache = Some(Arc::clone(&fresh));
        }
        let state = shared.work.lock().map(|s| s.clone()).unwrap_or_default();
        let mut desk = Desk::new(fresh, &shared);
        // What changed elsewhere in the blocks set right, a time given by a drag before them made one:
        // laid again then, at once (`blocks::upkeep`; nothing written when nothing differs).
        if crate::blocks::upkeep(&shared, &desk.loaded.tasks, &Zoned::now()) {
            show_work(&qt, &shared);
            pim::show_pim(&qt, &shared);
        }
        // The phone's home screen card: the next step as each coming time will have it (homecard.rs).
        crate::homecard::plan_seen(&desk.loaded, &desk.plan, &desk.today, &desk.spent, &desk.stopped);
        desk.filter = Filter { case: state.case.clone(), quiet: desk.filter.quiet.clone(), ..state.filter.clone() };
        let cx = desk.context();
        let case = Some(state.case.as_str()).filter(|c| !c.is_empty());
        let rest = desk.settings.rest_days();
        let lists = writable_lists();
        let mut counted: BTreeMap<String, (usize, String)> = BTreeMap::new();
        for task in desk.loaded.tasks.iter().filter(|t| t.status.is_open()) {
            for category in &task.categories {
                counted.entry(category.to_lowercase()).or_insert((0, category.clone())).0 += 1;
            }
        }
        let mut categories: Vec<(usize, String)> = counted.into_values().collect();
        categories.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
        // The day's first step, as the plan had it when the day before was closed.
        let today = Zoned::now();
        let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
        let first = match overrides.first_step_on(today.date()).filter(|_| desk.filter.quiet.is_none()) {
            Some((step, uid)) => (say("first-step-line", &[("day", capitalized(&tr().text(&format!("weekday-{}", today.date().weekday().to_monday_one_offset()), None))), ("step", step.to_string())]), uid.to_string()),
            None => (String::new(), String::new()),
        };
        let shown = TasksShown {
            view: state.clone(),
            kinds: load_config().task_kinds(tr()).into_iter().map(|(id, label)| KindChoice { id, label }).collect(),
            categories: categories.into_iter().map(|(_, c)| c).collect(),
            weather: desk.today.weather,
            now: taskview::now(&cx, desk.today.weather, &desk.today.aside),
            list: taskview::list(&cx, if state.by.is_empty() { "case" } else { &state.by }, state.done, &state.query),
            board: taskview::board(&cx, case),
            timeline: taskview::timeline(&cx, case, &rest),
            no_list: lists.is_empty(),
            lists,
            cases: desk.loaded.cases.iter().filter(|c| c.status.as_deref() != Some("closed")).map(|c| CaseChoice { id: c.id.clone(), title: c.title.clone() }).collect(),
            quiet: desk.quiet,
            first_step: first.0,
            first_step_task: first.1,
            budget: desk.next_budget(),
            day: today_laid_out(&desk, &shared),
            free: crate::pauses::offers(&desk.loaded.tasks, desk.filter.quiet.as_ref(), crate::hours::mode_now().free()),
        };
        let tasks_json = crate::backend::json(&shown);
        // The steps laid elsewhere now (a meal moved, a step given a time): the Health page's day follows at once.
        if day_steps() != steps_before {
            crate::health::show_health(&qt, shared);
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
    });
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
    let mut situation = situation(&loaded.cases);
    let before = plan::plan(&loaded.tasks, date, &settings(today.weather, &situation, &loaded.cases, &loaded.tasks, shared), &spent, &today.aside);
    situation.closed.insert(date);
    let after = plan::plan(&loaded.tasks, date, &settings(today.weather, &situation, &loaded.cases, &loaded.tasks, shared), &spent, &today.aside);
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

/// The task page's choices: how the list is grouped, done tasks, a search, one case.
pub(crate) fn set_view(qt: &QtThread, shared: &Arc<Shared>, by: &str, done: bool, query: &str, case: &str) {
    if let Ok(mut state) = shared.work.lock() {
        state.by = by.to_string();
        state.done = done;
        state.query = query.to_string();
        state.case = case.to_string();
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
fn target_list(id: &str) -> Result<Collection, String> {
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
    let shaped = Task { categories: edit.categories.clone(), cases: edit.cases.clone(), list_id: list_id.clone(), ..Task::default() };
    let area_tags = sioul_core::areas::TaskAreas::of_config(&config, &loaded.cases).by_tags(&shaped).id();
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
    let text = tasks::new_task(edit, &uid, &TimeZone::system(), &Zoned::now())?;
    write(qt, shared, &target.dir.join(format!("{uid}.ics")), &text)?;
    Ok(uid)
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

/// A category renamed on every task that has it, or taken off them all (`to`
/// empty); a name already there merges with it. Written at once, then the
/// accounts send the changes.
pub(crate) fn rename_category(qt: &QtThread, shared: &Arc<Shared>, from: &str, to: &str) -> Result<(), String> {
    let fold = |s: &str| s.trim().to_lowercase();
    let (from, to) = (fold(from), to.trim().to_string());
    let mut accounts = BTreeSet::new();
    let mut problem = None;
    for task in loaded(shared).tasks.iter().filter(|t| !t.read_only && t.categories.iter().any(|c| fold(c) == from)) {
        let mut edit = TaskEdit::of(task);
        let mut categories: Vec<String> = Vec::new();
        for category in &edit.categories {
            let next = if fold(category) == from { to.clone() } else { category.clone() };
            if !next.is_empty() && !categories.iter().any(|c| fold(c) == fold(&next)) {
                categories.push(next);
            }
        }
        edit.categories = categories;
        let path = Path::new(&task.key);
        let written = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|current| tasks::apply(&current, &edit, &TimeZone::system(), &Zoned::now()).map(|text| (current, text)))
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
    let Some(root) = load_config().case_store_path() else { return tr().text("error-no-store", None) };
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
    let (new_path, _) = match notes::rename(vault, path, name) {
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
        let mut edit = TaskEdit::of(task);
        for link in edit.links.iter_mut().filter(|l| l.uri == old_uri) {
            link.uri = new_uri.clone();
        }
        let _ = change(qt, shared, &task.uid, |text| tasks::apply(text, &edit, &TimeZone::system(), &Zoned::now()));
    }
    // Ties Sioul keeps itself.
    let ties_path = LocalLinks::default_path();
    let mut ties = LocalLinks::load(&ties_path);
    let mut moved = false;
    for edge in ties.links.iter_mut() {
        for end in [&mut edge.from, &mut edge.to] {
            if *end == old_uri {
                *end = new_uri.clone();
                moved = true;
            }
        }
    }
    if moved {
        let _ = ties.save(&ties_path);
    }
}

/// One task in full, with its lists and what it is tied to, as JSON.
pub(crate) fn task(shared: &Shared, uid: &str) -> String {
    let loaded = loaded(shared);
    let desk = Desk::new(Arc::clone(&loaded), shared);
    let Some(task) = desk.task(uid) else { return String::new() };
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
    crate::backend::json(&Shown { detail, lists: writable_lists(), list: &task.list_id, limited, budget, proposed })
}

/// Saves the task form; an empty UID makes a new task in `list`. Returns {"uid"} or {"error"}.
pub(crate) fn save(qt: &QtThread, shared: &Arc<Shared>, uid: &str, edit: &str, list: &str) -> String {
    let result = serde_json::from_str::<TaskEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        if uid.is_empty() {
            return create(qt, shared, &edit, list);
        }
        change(qt, shared, uid, |text| tasks::apply(text, &edit, &TimeZone::system(), &Zoned::now()))?;
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
    let cases: Vec<String> = loaded(shared).cases.iter().map(|c| c.id.clone()).collect();
    crate::backend::json(&capture::capture(line, Zoned::now().date(), &cases, &load_config().task_kinds(tr())))
}

/// A task from a typed line, a step of `parent` when given.
pub(crate) fn add(qt: &QtThread, shared: &Arc<Shared>, line: &str, parent: &str, list: &str) -> String {
    let cases: Vec<String> = loaded(shared).cases.iter().map(|c| c.id.clone()).collect();
    let config = load_config();
    let mut edit = capture::capture(line, Zoned::now().date(), &cases, &config.task_kinds(tr())).edit;
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
    // A step goes into its bigger task's list, and its case.
    let list = match find(shared, parent) {
        Ok(bigger) => {
            for case in bigger.cases {
                if !edit.cases.contains(&case) {
                    edit.cases.push(case);
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
    let result = change(qt, shared, uid, |text| tasks::set_status(text, status, &TimeZone::system(), &Zoned::now()));
    if let Err(e) = result {
        return e;
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

/// Waits for `other`, or no longer: DEPENDS-ON in the task that waits; a
/// FINISHTOSTART written by another application in `other` is removed there.
pub(crate) fn set_waits(qt: &QtThread, shared: &Arc<Shared>, uid: &str, other: &str, wait: bool) -> String {
    let now = Zoned::now();
    let result = if wait {
        change(qt, shared, uid, |text| tasks::add_lines(text, &[tasks::relation_line("DEPENDS-ON", other, 0)], &now))
    } else {
        let mine = |l: &str| lines::name(l) == "RELATED-TO" && tasks::relation_of(l).kind == "DEPENDS-ON" && tasks::relation_of(l).uid == other;
        let theirs = |l: &str| lines::name(l) == "RELATED-TO" && matches!(tasks::relation_of(l).kind.as_str(), "FINISHTOSTART" | "NEXT") && tasks::relation_of(l).uid == uid;
        change(qt, shared, uid, |text| tasks::remove_lines(text, mine, &now)).and_then(|()| change(qt, shared, other, |text| tasks::remove_lines(text, theirs, &now)))
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
    Desk::new(loaded(shared), shared).plan.items.get(uid).map(|p| p.laid).filter(|m| *m > 0)
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
    if let Some(mut running) = timelog::running() {
        running.toggle_pause(Timestamp::now().as_second());
        let _ = timelog::keep_running(Some(&running));
    }
    show_work(qt, shared);
}

/// More minutes for the session.
pub(crate) fn focus_extend(qt: &QtThread, shared: &Arc<Shared>, minutes: i32) {
    if let Some(mut running) = timelog::running() {
        running.planned = running.planned.saturating_add(u32::try_from(minutes.max(0)).unwrap_or(0));
        let _ = timelog::keep_running(Some(&running));
    }
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
    /// No notes folder: the case store is not set.
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
    let Some(root) = load_config().case_store_path() else { return String::new() };
    let title = if title.trim().is_empty() { tr().text("note-untitled", None) } else { title.to_string() };
    let folder = if folder.trim().is_empty() { notes_folder() } else { folder.to_string() };
    let path = notes::free_path(&root, &folder, &title);
    match notes::write(&root, &path, &notes::new_text(&title, &[], "")) {
        Ok(_) => {
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
    let Some(root) = load_config().case_store_path() else { return tr().text("error-no-store", None) };
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
    let (new_path, moved) = match notes::rename_folder(vault, path, name) {
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
    let Some(root) = load_config().case_store_path() else { return tr().text("error-no-store", None) };
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
    let Some(root) = load_config().case_store_path() else { return answer(tr().text("error-no-store", None), path, String::new()) };
    // Changed since it was opened (another device through the sharing, a sync
    // app, another editor): that version keeps the name, this one goes beside it.
    let changed = notes::normalize(path)
        .and_then(|relative| std::fs::read_to_string(root.join(relative)).ok())
        .is_some_and(|current| !stamp.is_empty() && text_stamp(&current) != stamp && current != text);
    if changed {
        let beside = conflict_path(&root, path);
        return match notes::write(&root, &beside, text) {
            Ok(_) => {
                show_work(qt, shared);
                answer(String::new(), &beside, say("note-changed-elsewhere", &[("path", beside.clone())]))
            }
            Err(e) => answer(e, path, String::new()),
        };
    }
    match notes::write(&root, path, text) {
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
    let Some(root) = load_config().case_store_path() else { return String::new() };
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
    let Some(root) = load_config().case_store_path() else { return String::new() };
    let file = crate::backend::local_path(url);
    file.strip_prefix(&root).map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/")).unwrap_or_default()
}

/// A new note in the notes folder, its front matter linking `links`; returns its path, or "".
fn new_note(title: &str, front: &[(&str, Vec<String>)], body: &str) -> Result<String, String> {
    let root = load_config().case_store_path().ok_or_else(|| tr().text("error-no-store", None))?;
    let path = notes::free_path(&root, &notes_folder(), title);
    notes::write(&root, &path, &notes::new_text(title, front, body))?;
    Ok(path)
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
            let root = load_config().case_store_path().ok_or_else(|| tr().text("error-no-store", None))?;
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
    let result = match loaded.tie(&from, &to, &Zoned::now()) {
        Some(change) => rewrite(shared, &change),
        None => {
            let path = LocalLinks::default_path();
            let mut local = LocalLinks::load(&path);
            local.add(&from, &to, "link");
            local.save(&path)
        }
    };
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
    for change in loaded.untie(&a, &b, &Zoned::now()) {
        match rewrite(shared, &change) {
            Ok(()) => undone = true,
            Err(e) => problem = Some(e),
        }
    }
    let path = LocalLinks::default_path();
    let mut local = LocalLinks::load(&path);
    if local.untie(&world, &a, &b) {
        match local.save(&path) {
            Ok(()) => undone = true,
            Err(e) => problem = Some(e),
        }
    }
    links_changed(qt, shared);
    if let Some(problem) = problem {
        return problem;
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
        "case" => Some(LinkKind::Case),
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
        "case" => links::case_uri(id),
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
/// linked, its sender and case; for an event, "Prepare: …", its day as the
/// date asked, the event linked, its cases; else the thing's title and a tie
/// to it (a contact involved, a case, a note describing it, anything related).
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
                LinkKind::Case => edit.cases.push(links::id_of(&from)),
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

/// A task from a message: its subject, the message linked, its sender, its case.
pub(crate) fn task_from_mail(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    answer(mail_task_edit(shared, key).and_then(|edit| create(qt, shared, &edit, "")))
}

/// A message's task, before it is made.
fn mail_task_edit(shared: &Shared, key: &str) -> Result<TaskEdit, String> {
    let Some((_, card)) = message_card(key) else { return Err(tr().text("mail-message-gone", None)) };
    let loaded = loaded(shared);
    let mut edit = TaskEdit { title: card.subject.trim().to_string(), ..TaskEdit::default() };
    if let Some(id) = &card.message_id {
        edit.links.push(Link { uri: links::mail_uri(id), label: String::new(), rel: "via".into() });
    }
    edit.contacts.extend(sender_contact(&loaded, &card));
    if let Some(store) = load_config().case_store_path().and_then(|r| CaseStore::load(&r).ok()) {
        edit.cases.extend(store.route(&card).first().map(|r| r.case.id.clone()));
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

/// A note for an event: dated, the guests listed, linked both ways (the
/// event's LINK when its calendar can be written, the note's front matter always).
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
    let link = tasks::link_line(&Link { uri: notes::uri_of(&path), label: String::new(), rel: "describedby".into() });
    if let Ok(text) = std::fs::read_to_string(&event.key)
        && let Some(with_link) = agenda::add_lines(&text, &[link])
    {
        let _ = vdir::write_item(Path::new(&event.key), &with_link);
        if let Some(account) = account_of(Path::new(&event.key)) {
            pim::nudge(shared, &account);
        }
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
        cases: event.cases.clone(),
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
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What the task pages show, in words: the next step and why, the board, the
//! list, the timeline, one task in full. The core decides and words it; the
//! window only lays it out (docs/architecture.md).
//!
//! The words follow docs/tasks.md: nothing is "overdue", "late" or "missed";
//! a date asked is said as the time left, and once passed only as the date it
//! was; a day to start that passed says nothing at all; nothing commands, and
//! nothing counts what you did not do.

use crate::projects::Project;
use crate::i18n::Translator;
use crate::links::{Kind, Related};
use crate::plan::{Column, Plan, Planned};
use crate::tasks::{Status, Task, TaskEdit};
use crate::today::{FOG_MINUTES, Weather};
use fluent_bundle::FluentArgs;
use jiff::civil::Date;
use jiff::{Span, Timestamp, tz::TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What every view needs.
pub struct Context<'a> {
    /// What the pages show: one kind, one category, or everything.
    pub filter: &'a Filter,
    /// Offices are open now; else, when they open, in words ("Monday at 9:00").
    pub offices: Offices,
    pub tasks: &'a [Task],
    pub plan: &'a Plan,
    pub today: Date,
    pub tr: &'a Translator,
    pub projects: &'a [Project],
    /// Minutes spent on each task, all time.
    pub spent: &'a BTreeMap<String, u32>,
    /// The last "where I stopped" line of each task.
    pub stopped: &'a BTreeMap<String, String>,
}

/// What the task pages show: tasks of one kind, of one category; empty, all of them.
/// The plan stays whole: a call that waits for a message still waits for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub category: String,
    /// One project ("" for all); kept apart, as the page's own choice.
    #[serde(skip)]
    pub project: String,
    /// Quiet time: what it keeps (`quiet::QuietTasks`).
    #[serde(skip)]
    pub quiet: Option<crate::quiet::QuietTasks>,
}

impl Filter {
    pub fn wants(&self, task: &Task) -> bool {
        (self.kind.is_empty() || task.kind == self.kind)
            && (self.category.is_empty() || task.categories.iter().any(|c| c.eq_ignore_ascii_case(&self.category)))
            && (self.project.is_empty() || task.projects.iter().any(|c| c == &self.project))
            && self.quiet.as_ref().is_none_or(|quiet| quiet.keeps(task))
    }

    pub fn is_empty(&self) -> bool {
        self.kind.is_empty() && self.category.is_empty() && self.project.is_empty() && self.quiet.is_none()
    }
}

/// Whether offices are open now, for tasks that need one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Offices {
    pub open: bool,
    /// When they open next, in words; empty when open.
    pub next: String,
    /// The moment: a task's own office (its hours, `Task::office_times`) is told by it.
    pub now: Option<jiff::Zoned>,
}

impl Offices {
    /// Always open: for places that do not care (tests, a list).
    pub fn always() -> Offices {
        Offices { open: true, next: String::new(), now: None }
    }

    /// Whether the office a task needs is open now: its own hours when it has
    /// some, else offices' usual ones. A task needing none is always free.
    pub fn open_for(&self, task: &Task) -> bool {
        if !task.office_hours {
            return true;
        }
        let own = crate::window::parse_ranges(&task.office_times);
        match &self.now {
            Some(now) if !own.is_empty() => crate::window::current(&own, now).is_some(),
            _ => self.open,
        }
    }

    /// When a task's own office opens next; None for the usual hours (`next` says them).
    pub fn opening_for(&self, task: &Task) -> Option<jiff::Zoned> {
        let own = crate::window::parse_ranges(&task.office_times);
        self.now.as_ref().filter(|_| !own.is_empty()).and_then(|now| crate::window::next_opening(&own, now))
    }
}

impl Context<'_> {
    fn task(&self, uid: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.uid == uid)
    }

    fn planned(&self, uid: &str) -> Option<&Planned> {
        self.plan.items.get(uid)
    }

    fn title(&self, uid: &str) -> String {
        self.task(uid).map(|t| t.title.clone()).unwrap_or_else(|| uid.to_string())
    }

    fn project_title(&self, id: &str) -> String {
        self.projects.iter().find(|c| c.id == id).map_or_else(|| id.to_string(), |c| c.title.clone())
    }

    fn args(&self, pairs: &[(&str, String)]) -> FluentArgs<'static> {
        let mut args = FluentArgs::new();
        for (name, value) in pairs {
            args.set(name.to_string(), value.clone());
        }
        args
    }

    fn say(&self, id: &str, pairs: &[(&str, String)]) -> String {
        self.tr.text(id, Some(&self.args(pairs)))
    }

    fn counted(&self, id: &str, n: usize, pairs: &[(&str, String)]) -> String {
        let mut args = self.tr.counted(n);
        for (name, value) in pairs {
            args.set(name.to_string(), value.clone());
        }
        self.tr.text(id, Some(&args))
    }
}

/// Days from `today` to `date`.
fn days_between(today: Date, date: Date) -> i64 {
    date.since(today).map_or(0, |s| i64::from(s.get_days()))
}

/// "About 15 minutes", "About an hour", "About 1 h 30".
pub fn estimate_text(tr: &Translator, minutes: u32) -> String {
    let mut args = FluentArgs::new();
    match minutes {
        0 => String::new(),
        60 => tr.text("task-estimate-hour", None),
        m if m < 60 => {
            args.set("minutes", m);
            tr.text("task-estimate-minutes", Some(&args))
        }
        m => {
            args.set("hours", m / 60);
            args.set("minutes", if m % 60 == 0 { String::new() } else { format!("{:02}", m % 60) });
            tr.text("task-estimate-hours", Some(&args))
        }
    }
}

/// "1 minute so far", "25 minutes": the minutes as a number, so the plural
/// is chosen by it (as a word, "1 minutes"); with a `date` before them.
fn minutes_text(tr: &Translator, id: &str, minutes: u32, date: Option<String>) -> String {
    let mut args = FluentArgs::new();
    args.set("minutes", minutes);
    if let Some(date) = date {
        args.set("date", date);
    }
    tr.text(id, Some(&args))
}

/// "four weeks left", "five days left", "tomorrow", "today".
fn time_left(cx: &Context, days: i64) -> String {
    match days {
        0 => cx.tr.text("task-left-today", None),
        1 => cx.tr.text("task-left-tomorrow", None),
        d if d < 14 => cx.counted("task-left-days", d as usize, &[]),
        d if d < 61 => cx.counted("task-left-weeks", (d / 7) as usize, &[]),
        d => cx.counted("task-left-months", (d / 30) as usize, &[]),
    }
}

/// The date asked, as time left; once passed, only the date it was.
fn due_text(cx: &Context, task: &Task) -> String {
    let Some(due) = task.due_date() else { return String::new() };
    if !task.status.is_open() {
        return String::new();
    }
    let days = days_between(cx.today, due);
    let date = cx.tr.day_month(due);
    if days < 0 {
        return cx.say("task-date-asked", &[("date", date)]);
    }
    cx.say("task-due", &[("date", date), ("left", time_left(cx, days))])
}

/// One task, as every view shows it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CardView {
    pub uid: String,
    pub key: String,
    pub title: String,
    pub list: String,
    pub list_id: String,
    pub color: Option<String>,
    pub column: Column,
    pub status: Status,
    pub read_only: bool,
    pub depth: usize,
    /// "About 15 minutes".
    pub estimate: String,
    /// Minutes left, for the focus timer.
    pub minutes: u32,
    /// "By 30 October: four weeks left", or "Date asked: 30 October".
    pub due: String,
    /// The date asked is within a week: a quiet mark, never red.
    pub due_soon: bool,
    /// "Waits for: Fill the form", "Waits until Thursday 15 October".
    pub waits: String,
    /// "Frees two other steps".
    pub unblocks: String,
    /// "Two of three steps left".
    pub steps: String,
    pub has_steps: bool,
    /// "Part of: The housing aid request".
    pub parent: String,
    pub parent_uid: String,
    /// "Where you stopped: the income field".
    pub stopped: String,
    /// "45 minutes so far".
    pub spent: String,
    pub projects: Vec<String>,
    pub project_ids: Vec<String>,
    pub tags: Vec<String>,
    /// One of `tasks::KINDS`, or "".
    pub kind: String,
    /// The plan cannot keep its date at this pace, said once and calmly.
    pub tight: String,
    pub in_loop: bool,
    /// "Done Thursday 1 October".
    pub done_on: String,
    /// How many things it links to.
    pub links: usize,
    /// How heavy, from its costs when any is rated: "light", "usual", "heavy",
    /// "rest" (it gives back); "" when none is rated (its word stands: `energy`).
    pub level: &'static str,
    /// Pinned to a time (its block, `blocks`): "Pinned: today, 14:00–14:45"; "" when not.
    pub pinned: String,
    /// The same, short, beside a pin on its row: "14:00" today, "Thu 8 Oct 10:00" another day; "" when not.
    pub pinned_time: String,
}

/// Where a task is pinned, as its panel shows it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PinView {
    /// "today, 14:00–14:45", "Thursday 8 October, 10:00–10:30".
    pub when: String,
    /// Its start on the clock, "2026-10-08T10:00": where "Do at…" opens.
    pub at: String,
    /// Its minutes, its margins aside.
    pub minutes: u32,
    /// The block's file, to open it as an event.
    pub key: String,
    /// Its calendar can only be read here.
    pub read_only: bool,
}

/// "today, 14:00–14:45", "Thursday 8 October, 10:00–10:30": a block's time in words.
fn pinned_when(cx: &Context, pin: &crate::blocks::Pin) -> String {
    let zone = cx.offices.now.as_ref().map_or_else(TimeZone::system, |z| z.time_zone().clone());
    let clock = |seconds: i64| Timestamp::from_second(seconds).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    let (from, to) = (clock(pin.start), clock(pin.end));
    if pin.date == cx.today {
        cx.say("task-pinned-today", &[("from", from), ("to", to)])
    } else {
        cx.say("task-pinned-day", &[("day", cx.tr.day_in(pin.date, cx.today)), ("from", from), ("to", to)])
    }
}

/// Where a task is pinned, for its panel; None when it is not.
pub fn pin_view(cx: &Context, uid: &str) -> Option<PinView> {
    let pin = cx.plan.pins.get(uid)?;
    let zone = cx.offices.now.as_ref().map_or_else(TimeZone::system, |z| z.time_zone().clone());
    let at = Timestamp::from_second(pin.start).ok()?.to_zoned(zone).strftime("%Y-%m-%dT%H:%M").to_string();
    Some(PinView { when: pinned_when(cx, pin), at, minutes: pin.minutes(), key: pin.key.clone(), read_only: pin.read_only })
}

/// A task in words.
pub fn card(cx: &Context, task: &Task) -> CardView {
    let planned = cx.planned(&task.uid).cloned().unwrap_or_default();
    let waits = if !planned.waits_for.is_empty() {
        let titles: Vec<String> = planned.waits_for.iter().map(|u| cx.title(u)).collect();
        cx.say("task-waits-for", &[("titles", titles.join(", "))])
    } else if let Some(day) = planned.not_before.filter(|d| *d > cx.today) {
        cx.say("task-waits-until", &[("date", cx.tr.day(day))])
    } else {
        String::new()
    };
    let steps_total = cx.tasks.iter().filter(|t| t.parent() == Some(task.uid.as_str()) && t.status != Status::Cancelled).count();
    let steps = if steps_total > 0 && task.status.is_open() {
        cx.counted("task-steps-left", planned.open_steps, &[("total", cx.tr.count(steps_total, false, false))])
    } else {
        String::new()
    };
    let parent_uid = task.parent().unwrap_or_default().to_string();
    let parent = if parent_uid.is_empty() { String::new() } else { cx.say("task-part-of", &[("title", cx.title(&parent_uid))]) };
    let spent = cx.spent.get(&task.uid).copied().unwrap_or(0);
    let tight = match (planned.tight, task.due_date(), planned.finish) {
        (true, Some(due), Some(_)) => cx.say("task-tight", &[("date", cx.tr.day_month(due))]),
        _ => String::new(),
    };
    let done_on = match (task.status, task.completed) {
        (Status::Completed, Some(at)) => Timestamp::from_second(at).map(|t| cx.say("task-done-on", &[("date", cx.tr.day(t.to_zoned(TimeZone::system()).date()))])).unwrap_or_default(),
        _ => String::new(),
    };
    CardView {
        uid: task.uid.clone(),
        key: task.key.clone(),
        title: if task.title.trim().is_empty() { cx.tr.text("task-untitled", None) } else { task.title.clone() },
        list: task.list.clone(),
        list_id: task.list_id.clone(),
        color: task.color.clone(),
        column: planned.column,
        status: task.status,
        read_only: task.read_only,
        depth: planned.depth,
        estimate: if steps_total > 0 { String::new() } else { estimate_text(cx.tr, task.estimate) },
        minutes: planned.left,
        due: due_text(cx, task),
        due_soon: task.status.is_open() && task.due_date().is_some_and(|d| (0..=7).contains(&days_between(cx.today, d))),
        waits,
        unblocks: if planned.unblocks > 0 && task.status.is_open() { cx.counted("task-unblocks", planned.unblocks, &[]) } else { String::new() },
        steps,
        has_steps: steps_total > 0,
        parent,
        parent_uid,
        stopped: cx.stopped.get(&task.uid).filter(|s| !s.is_empty() && task.status.is_open()).map(|s| cx.say("task-stopped", &[("text", s.clone())])).unwrap_or_default(),
        spent: if spent > 0 { minutes_text(cx.tr, "task-spent-time", spent, None) } else { String::new() },
        projects: task.projects.iter().map(|c| cx.project_title(c)).collect(),
        project_ids: task.projects.clone(),
        tags: task.categories.clone(),
        kind: task.kind.clone(),
        tight,
        in_loop: planned.in_loop,
        done_on,
        links: task.links.len() + task.contacts.len(),
        level: task.demands.level().map_or("", crate::demands::Level::id),
        pinned: cx.plan.pins.get(&task.uid).map(|pin| cx.say("task-pinned", &[("when", pinned_when(cx, pin))])).unwrap_or_default(),
        pinned_time: cx.plan.pins.get(&task.uid).map(|pin| pinned_short(cx, pin)).unwrap_or_default(),
    }
}

/// "14:00" today, "Thu 8 Oct 10:00" another day: where a task is pinned, beside a pin on its row.
fn pinned_short(cx: &Context, pin: &crate::blocks::Pin) -> String {
    let zone = cx.offices.now.as_ref().map_or_else(TimeZone::system, |z| z.time_zone().clone());
    let Ok(start) = Timestamp::from_second(pin.start).map(|t| t.to_zoned(zone)) else { return String::new() };
    if pin.date == cx.today { start.strftime("%H:%M").to_string() } else { cx.tr.date(&start, true) }
}

/// The Now page: the next step, why, and the one after it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NowView {
    pub weather: Weather,
    /// "Steps that need an open office come back Monday at 9:00."
    pub office_note: String,
    /// Said when only steps tied to what was put off are free: "Only steps tied to what you put off are free now."
    pub tied_note: String,
    /// What the day's weather changes, in a sentence; empty when clear.
    pub weather_note: String,
    pub now: Option<CardView>,
    /// Why this one: short sentences.
    pub why: Vec<String>,
    /// Several steps were equal: Sioul picked.
    pub picked: String,
    pub then: Option<CardView>,
    /// Up to two other steps free to start, behind a key.
    pub others: Vec<CardView>,
    /// Other steps started.
    pub started: Vec<CardView>,
    /// "Three things are started. Finish or park one?"
    pub wip: String,
    /// When nothing is free to start: what is going on, in a sentence.
    pub empty: String,
    pub loops: Vec<String>,
    /// Done this week, newest first.
    pub done_week: Vec<CardView>,
    /// Tagged `joy` and free to start: offered, never proposed.
    pub joy: Vec<CardView>,
    /// After a heavy step done within two hours: something that gives back (energy "rest"), offered.
    pub rest: Option<CardView>,
}

/// The open tasks that wait for this one, further down too, with a date
/// asked still ahead: (title, date), the soonest first.
fn dates_after(cx: &Context, uid: &str) -> Vec<(String, String, Date)> {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![uid.to_string()];
    let mut found = Vec::new();
    while let Some(at) = stack.pop() {
        for next in cx.plan.items.values().filter(|p| p.waits_for.contains(&at)) {
            if seen.insert(next.uid.clone()) {
                stack.push(next.uid.clone());
                // Its own date, or its bigger task's: the step that waits is named, not the bigger task.
                let step = cx.task(&next.uid).map(|t| t.title.clone()).unwrap_or_default();
                let mut at = cx.task(&next.uid);
                while let Some(task) = at {
                    if let Some(due) = task.due_date().filter(|d| *d >= cx.today) {
                        let whole = if task.uid == next.uid { String::new() } else { task.title.clone() };
                        found.push((step.clone(), whole, due));
                        break;
                    }
                    at = task.parent().and_then(|p| cx.task(p));
                }
            }
        }
    }
    found.sort_by_key(|(_, _, d)| *d);
    found
}

/// Whether two steps rank alike, before their titles and ages break the tie.
fn alike(a: &Planned, b: &Planned, ta: &Task, tb: &Task) -> bool {
    a.latest_start == b.latest_start && a.unblocks == b.unblocks && ta.priority == tb.priority && a.left == b.left && ta.status == tb.status
}

pub fn now(cx: &Context, weather: Weather, aside: &std::collections::BTreeSet<String>) -> NowView {
    let mut view = NowView { weather, ..NowView::default() };
    let ready: Vec<&Task> = cx.plan.ready.iter().filter_map(|u| cx.task(u)).filter(|t| cx.filter.wants(t)).collect();
    // What was put off takes its stream with it for the day: what comes next is not tied to it.
    let aside_streams: std::collections::BTreeSet<usize> = aside.iter().filter_map(|u| cx.plan.streams.get(u)).copied().collect();
    let untied = |t: &&Task| cx.plan.streams.get(&t.uid).is_none_or(|s| !aside_streams.contains(s));
    // A call to an office waits for its office to open: said once, quietly,
    // with the soonest opening among them.
    let closed: Vec<&Task> = ready.iter().copied().filter(|t| !cx.offices.open_for(t) && !aside.contains(&t.uid)).collect();
    if !closed.is_empty() {
        let own = closed.iter().filter_map(|t| cx.offices.opening_for(t)).min_by_key(jiff::Zoned::timestamp);
        let usual = closed.iter().any(|t| crate::window::parse_ranges(&t.office_times).is_empty());
        let when = match own {
            Some(own) if !usual => cx.tr.when(&own),
            _ => cx.offices.next.clone(),
        };
        view.office_note = cx.say("task-office-closed", &[("when", when)]);
    }
    let not_aside: Vec<&Task> = ready.iter().copied().filter(|t| !aside.contains(&t.uid) && cx.offices.open_for(t)).collect();
    let others: Vec<&Task> = not_aside.iter().copied().filter(untied).collect();
    if others.is_empty() && !not_aside.is_empty() {
        view.tied_note = cx.tr.text("task-only-tied", None);
    }
    let free: Vec<&Task> = if others.is_empty() { not_aside } else { others };
    // A step pinned to a time later (its block, `blocks`; or a time given today
    // by a drag before blocks) waits for it: the others come first; it is still
    // proposed when nothing else is free.
    let stamp = cx.offices.now.as_ref().map_or_else(|| Timestamp::now().as_second(), |z| z.timestamp().as_second());
    let zone = cx.offices.now.as_ref().map_or_else(TimeZone::system, |z| z.time_zone().clone());
    let (later, sooner): (Vec<&Task>, Vec<&Task>) = free.into_iter().partition(|t| cx.plan.pins.get(&t.uid).map_or_else(|| t.at_on(cx.today, &zone), |pin| Some(pin.start)).is_some_and(|at| at > stamp + 15 * 60));
    let free: Vec<&Task> = sooner.into_iter().chain(later).collect();
    // Fog: only small steps; if none is small, the smallest.
    let candidates: Vec<&Task> = if weather == Weather::Fog {
        let small: Vec<&Task> = free.iter().copied().filter(|t| cx.planned(&t.uid).is_some_and(|p| p.left <= FOG_MINUTES)).collect();
        if small.is_empty() { free.iter().copied().min_by_key(|t| cx.planned(&t.uid).map_or(u32::MAX, |p| p.left)).into_iter().collect() } else { small }
    } else {
        free.clone()
    };
    view.weather_note = match weather {
        Weather::Clear => String::new(),
        Weather::Haze => cx.tr.text("task-weather-haze-note", None),
        Weather::Fog => cx.tr.text("task-weather-fog-note", None),
    };
    if let Some(first) = candidates.first() {
        let planned = cx.planned(&first.uid).cloned().unwrap_or_default();
        let mut why = Vec::new();
        if first.status == Status::InProcess {
            why.push(cx.tr.text("task-why-started", None));
        }
        let own_due = first.due_date().filter(|d| *d >= cx.today);
        if let Some(due) = own_due {
            why.push(cx.say("task-why-due", &[("date", cx.tr.day_month(due)), ("left", time_left(cx, days_between(cx.today, due)))]));
        }
        // A date further down the chain, sooner than its own: the reason it comes now.
        if let Some((title, whole, due)) = dates_after(cx, &first.uid).into_iter().find(|(_, _, d)| own_due.is_none_or(|own| d < &own)) {
            if whole.is_empty() {
                why.push(cx.say("task-why-chain", &[("title", title), ("date", cx.tr.day_month(due))]));
            } else {
                why.push(cx.say("task-why-chain-part", &[("title", title), ("whole", whole), ("date", cx.tr.day_month(due))]));
            }
        }
        if planned.unblocks > 0 {
            why.push(cx.counted("task-why-unblocks", planned.unblocks, &[]));
        }
        // Heavy on a foggy day: only because nothing else is free.
        if crate::capacity::level_of(first) == crate::demands::Level::Heavy && weather == Weather::Fog {
            why.push(cx.tr.text("task-why-heavy-fog", None));
        }
        if why.is_empty() {
            why.push(cx.tr.text("task-why-free", None));
        }
        view.why = why;
        if let Some(second) = candidates.get(1)
            && let (Some(a), Some(b)) = (cx.planned(&first.uid), cx.planned(&second.uid))
            && alike(a, b, first, second)
        {
            view.picked = cx.tr.text("task-picked", None);
        }
        view.now = Some(card(cx, first));
        if weather != Weather::Fog {
            view.then = candidates.get(1).map(|t| card(cx, t));
            view.others = candidates.iter().skip(2).take(2).map(|t| card(cx, t)).collect();
        }
    } else {
        // Nothing free today: the step the plan puts first, and its day.
        let coming = cx.plan.order.iter().filter_map(|u| Some((cx.task(u)?, cx.planned(u)?))).filter(|(_, p)| p.open_steps == 0 && p.start.is_some()).min_by_key(|(_, p)| p.start);
        let quiet_only = cx.filter.quiet.is_some() && cx.filter.kind.is_empty() && cx.filter.category.is_empty() && cx.filter.project.is_empty();
        view.empty = if !ready.is_empty() {
            cx.tr.text("task-all-aside", None)
        } else if quiet_only {
            cx.tr.text("task-quiet-empty", None)
        } else if !cx.filter.is_empty() {
            cx.tr.text("task-nothing-filtered", None)
        } else if let Some((task, planned)) = coming {
            let day = planned.start.unwrap_or(cx.today);
            cx.say("task-nothing-until", &[("date", cx.tr.day(day)), ("title", task.title.clone())])
        } else {
            cx.tr.text("task-nothing-ready", None)
        };
    }
    let started: Vec<&Task> = cx.tasks.iter().filter(|t| t.status == Status::InProcess && cx.filter.wants(t) && cx.planned(&t.uid).is_some_and(|p| p.open_steps == 0)).collect();
    view.started = started.iter().filter(|t| view.now.as_ref().is_none_or(|n| n.uid != t.uid)).map(|t| card(cx, t)).collect();
    if started.len() > 3 {
        view.wip = cx.counted("task-wip", started.len(), &[]);
    }
    view.loops = cx.plan.loops.iter().map(|l| cx.say("task-loop", &[("titles", l.iter().map(|u| cx.title(u)).collect::<Vec<_>>().join(", "))])).collect();
    let week_start = cx.today.checked_sub(Span::new().days(i64::from(cx.today.weekday().to_monday_zero_offset()))).unwrap_or(cx.today);
    let from = week_start.to_zoned(TimeZone::system()).map_or(0, |z| z.timestamp().as_second());
    let mut done: Vec<&Task> = cx.tasks.iter().filter(|t| t.status == Status::Completed && t.completed.is_some_and(|c| c >= from) && cx.filter.wants(t)).collect();
    done.sort_by_key(|t| std::cmp::Reverse(t.completed));
    view.done_week = done.iter().map(|t| card(cx, t)).collect();
    // What you do for joy (`words.tasks.joy`).
    let joy = crate::words::current().tasks.joy.clone();
    let joyful = |t: &Task| crate::words::named_in(&t.categories, &joy);
    view.joy = cx
        .plan
        .order
        .iter()
        .filter_map(|u| cx.task(u))
        .filter(|t| joyful(t) && cx.filter.wants(t) && cx.planned(&t.uid).is_some_and(|p| p.column == Column::Ready && p.open_steps == 0))
        .map(|t| card(cx, t))
        .collect();
    // Energy accounting: a deposit after a withdrawal (Toudal & Attwood), offered, never planned in.
    let lately = jiff::Timestamp::now().as_second() - 2 * 3600;
    let level = crate::capacity::level_of;
    if cx.tasks.iter().any(|t| t.status == Status::Completed && level(t) == crate::demands::Level::Heavy && t.completed.is_some_and(|c| c >= lately)) {
        view.rest = cx.tasks.iter().find(|t| level(t) == crate::demands::Level::Rest && crate::plan::is_optional(t) && t.status.is_open() && cx.filter.wants(t) && cx.planned(&t.uid).is_some_and(|p| p.column != Column::Waiting && p.open_steps == 0)).map(|t| card(cx, t));
    }
    view
}

/// One column of the board.
#[derive(Debug, Clone, Serialize)]
pub struct ColumnView {
    pub id: Column,
    pub title: String,
    pub cards: Vec<CardView>,
}

/// The board: free to start, started, waiting, done.
#[derive(Debug, Clone, Serialize)]
pub struct BoardView {
    pub columns: Vec<ColumnView>,
    pub wip: String,
}

/// The board shows the steps you act on: a bigger task stands for itself
/// only when all its steps are done.
pub fn board(cx: &Context, project: Option<&str>) -> BoardView {
    let in_project = |t: &Task| project.is_none_or(|c| t.projects.iter().any(|x| x == c)) && cx.filter.wants(t);
    let acted_on = |t: &&Task| cx.planned(&t.uid).is_some_and(|p| p.open_steps == 0 && (!p.optional || t.status != Status::NeedsAction)) && in_project(t);
    let ordered: Vec<&Task> = cx.plan.order.iter().filter_map(|u| cx.task(u)).filter(acted_on).collect();
    let fortnight = cx.today.checked_sub(Span::new().days(14)).unwrap_or(cx.today).to_zoned(TimeZone::system()).map_or(0, |z| z.timestamp().as_second());
    let mut done: Vec<&Task> = cx.tasks.iter().filter(|t| t.status == Status::Completed && t.completed.is_some_and(|c| c >= fortnight)).filter(acted_on).collect();
    done.sort_by_key(|t| std::cmp::Reverse(t.completed));
    let column = |id: Column, key: &str, tasks: Vec<&Task>| ColumnView { id, title: cx.tr.text(key, None), cards: tasks.into_iter().map(|t| card(cx, t)).collect() };
    let of = |c: Column| ordered.iter().copied().filter(|t| cx.planned(&t.uid).is_some_and(|p| p.column == c)).collect::<Vec<_>>();
    let doing = of(Column::Doing);
    BoardView {
        wip: if doing.len() > 3 { cx.counted("task-wip", doing.len(), &[]) } else { String::new() },
        columns: vec![column(Column::Ready, "board-ready", of(Column::Ready)), column(Column::Doing, "board-doing", doing), column(Column::Waiting, "board-waiting", of(Column::Waiting)), column(Column::Done, "board-done", done)],
    }
}

/// A group of the list: a project, or a list.
#[derive(Debug, Clone, Serialize)]
pub struct GroupView {
    pub id: String,
    pub title: String,
    pub rows: Vec<CardView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListView {
    pub groups: Vec<GroupView>,
}

/// The list: every open task, a bigger task followed by its steps, in the
/// plan's order, grouped by project ("project") or by list ("list"). Done tasks of
/// the last two weeks come last when `done` is asked.
pub fn list(cx: &Context, by: &str, done: bool, query: &str) -> ListView {
    let fold = |t: &str| crate::text::fold(t).into_iter().collect::<String>();
    let words: Vec<String> = query.split_whitespace().map(fold).collect();
    let matches = |t: &Task| words.iter().all(|w| fold(&format!("{} {} {}", t.title, t.notes, t.categories.join(" "))).contains(w.as_str()));
    let fortnight = cx.today.checked_sub(Span::new().days(14)).unwrap_or(cx.today).to_zoned(TimeZone::system()).map_or(0, |z| z.timestamp().as_second());
    let shown = |t: &Task| (t.status.is_open() || (done && t.status == Status::Completed && t.completed.is_some_and(|c| c >= fortnight))) && matches(t) && cx.filter.wants(t);
    // Open tasks in the plan's order, then done ones, newest first.
    let mut ordered: Vec<&Task> = cx.plan.order.iter().filter_map(|u| cx.task(u)).collect();
    let mut finished: Vec<&Task> = cx.tasks.iter().filter(|t| t.status == Status::Completed).collect();
    finished.sort_by_key(|t| std::cmp::Reverse(t.completed));
    ordered.extend(finished);
    let position: BTreeMap<&str, usize> = ordered.iter().enumerate().map(|(i, t)| (t.uid.as_str(), i)).collect();
    // A bigger task, then its steps, each after the other, each once.
    fn outline<'a>(task: &'a Task, all: &[&'a Task], position: &BTreeMap<&str, usize>, out: &mut Vec<&'a Task>, placed: &mut std::collections::BTreeSet<&'a str>, depth: usize) {
        if !placed.insert(task.uid.as_str()) {
            return;
        }
        out.push(task);
        if depth > 8 {
            return;
        }
        let mut steps: Vec<&Task> = all.iter().copied().filter(|t| t.parent() == Some(task.uid.as_str())).collect();
        steps.sort_by_key(|t| position.get(t.uid.as_str()).copied().unwrap_or(usize::MAX));
        for step in steps {
            outline(step, all, position, out, placed, depth + 1);
        }
    }
    let known: std::collections::BTreeSet<&str> = ordered.iter().map(|t| t.uid.as_str()).collect();
    let roots: Vec<&Task> = ordered.iter().copied().filter(|t| t.parent().is_none_or(|p| !known.contains(p))).collect();
    // Each bigger task with its steps; the steps go where it goes. Then what no
    // root reaches: tasks that are steps of each other in a loop (a file from
    // elsewhere), steps nested too deep; else they would not be listed at all.
    let mut groups: Vec<GroupView> = Vec::new();
    let mut placed: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for root in roots.into_iter().chain(ordered.iter().copied()) {
        if placed.contains(root.uid.as_str()) {
            continue;
        }
        let mut rows: Vec<&Task> = Vec::new();
        outline(root, &ordered, &position, &mut rows, &mut placed, 0);
        let optional = cx.planned(&root.uid).is_some_and(|p| p.optional);
        let joyful = rows.iter().any(|t| crate::words::named_in(&t.categories, &crate::words::current().tasks.joy));
        let (id, title) = match by {
            _ if optional && joyful => (OPTIONAL_JOY.to_string(), cx.tr.text("task-group-joy", None)),
            _ if optional => (OPTIONAL_SOMEDAY.to_string(), cx.tr.text("task-group-someday", None)),
            "list" => (root.list_id.clone(), root.list.clone()),
            _ => match rows.iter().find_map(|t| t.projects.first()) {
                Some(project) => (project.clone(), cx.project_title(project)),
                None => (String::new(), cx.tr.text("task-no-project", None)),
            },
        };
        let cards: Vec<CardView> = rows.into_iter().filter(|t| shown(t)).map(|t| card(cx, t)).collect();
        if cards.is_empty() {
            continue;
        }
        match groups.iter_mut().find(|g| g.id == id) {
            Some(group) => group.rows.extend(cards),
            None => groups.push(GroupView { id, title, rows: cards }),
        }
    }
    // Tasks without a project come after the projects; what you do if you want, and what is parked, last.
    groups.sort_by_key(|g| match g.id.as_str() {
        OPTIONAL_JOY => 2,
        OPTIONAL_SOMEDAY => 3,
        "" => 1,
        _ => 0,
    });
    ListView { groups }
}

/// The groups of the list that hold optional tasks.
const OPTIONAL_JOY: &str = "\u{1}joy";
const OPTIONAL_SOMEDAY: &str = "\u{1}someday";

/// One day of the timeline's head.
#[derive(Debug, Clone, Serialize)]
pub struct DayHead {
    pub date: String,
    /// "5".
    pub day: String,
    /// "Mon".
    pub weekday: String,
    /// "October", on the first day shown and the first of each month.
    pub month: String,
    /// No room for tasks that day.
    pub rest: bool,
}

/// One bar of the timeline.
#[derive(Debug, Clone, Serialize)]
pub struct BarView {
    pub uid: String,
    pub key: String,
    pub title: String,
    pub depth: usize,
    /// Days from today.
    pub start: i64,
    pub length: i64,
    /// The date asked, in days from today, when it falls in view.
    pub due: Option<i64>,
    pub tight: bool,
    pub column: Column,
    pub has_steps: bool,
    pub color: Option<String>,
    /// "Waits for: …", "Frees two other steps", in one line.
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineView {
    pub days: Vec<DayHead>,
    pub rows: Vec<BarView>,
    /// "The plan reaches Friday 6 November." or why it is empty.
    pub note: String,
}

/// The timeline (Gantt chart): each open task on the days the plan gives it,
/// from today; the dates asked as marks. `project` keeps one project.
pub fn timeline(cx: &Context, project: Option<&str>, rest_days: &[bool; 7]) -> TimelineView {
    let in_project = |t: &Task| project.is_none_or(|c| t.projects.iter().any(|x| x == c)) && cx.filter.wants(t);
    let list = list(cx, "plan", false, "");
    let ordered: Vec<&Task> = list.groups.iter().flat_map(|g| g.rows.iter()).filter_map(|c| cx.task(&c.uid)).filter(|t| in_project(t)).collect();
    let last = ordered
        .iter()
        .filter_map(|t| {
            let p = cx.planned(&t.uid)?;
            let finish = p.finish.map(|f| days_between(cx.today, f));
            let due = t.due_date().map(|d| days_between(cx.today, d));
            finish.max(due)
        })
        .max()
        .unwrap_or(0);
    let span = (last + 3).clamp(14, 180);
    let mut days = Vec::new();
    for i in 0..span {
        let Ok(date) = cx.today.checked_add(Span::new().days(i)) else { break };
        days.push(DayHead {
            date: date.to_string(),
            day: date.day().to_string(),
            weekday: cx.tr.weekday_short(date),
            month: if i == 0 || date.day() == 1 { cx.tr.month_year(date) } else { String::new() },
            rest: rest_days[date.weekday().to_monday_zero_offset() as usize],
        });
    }
    let rows: Vec<BarView> = ordered
        .iter()
        .filter_map(|t| {
            let p = cx.planned(&t.uid)?;
            let (start, finish) = (p.start?, p.finish?);
            let start = days_between(cx.today, start).max(0);
            let length = (days_between(cx.today, finish).max(start) - start + 1).max(1);
            let c = card(cx, t);
            let detail = [c.waits.clone(), c.unblocks.clone(), c.estimate.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
            Some(BarView {
                uid: t.uid.clone(),
                key: t.key.clone(),
                title: c.title,
                depth: p.depth,
                start,
                length,
                due: t.due_date().map(|d| days_between(cx.today, d)).filter(|d| (0..span).contains(d)),
                tight: p.tight,
                column: p.column,
                has_steps: c.has_steps,
                color: t.color.clone(),
                detail,
            })
        })
        .collect();
    let note = if rows.is_empty() {
        cx.tr.text("timeline-empty", None)
    } else {
        let end = cx.today.checked_add(Span::new().days(rows.iter().map(|r| r.start + r.length - 1).max().unwrap_or(0))).unwrap_or(cx.today);
        cx.say("timeline-reaches", &[("date", cx.tr.day(end))])
    };
    TimelineView { days, rows, note }
}

/// Something tied to the task open, in words.
#[derive(Debug, Clone, Serialize)]
pub struct RelatedView {
    pub uri: String,
    pub kind: Kind,
    pub title: String,
    pub detail: String,
    /// "Its notes", "Made from", "Waits for"…
    pub how: String,
    /// The date, in words.
    pub when: String,
    pub key: String,
    pub found: bool,
    /// A program on this computer: `key` is its folder, which opens instead.
    pub program: bool,
}

/// What the Related panel shows.
pub fn related_views(tr: &Translator, related: &[Related]) -> Vec<RelatedView> {
    related
        .iter()
        .map(|r| RelatedView {
            uri: r.uri.clone(),
            kind: r.kind,
            title: if r.title.trim().is_empty() { tr.text("task-untitled", None) } else { r.title.clone() },
            // A note linked here whose file is not on this computer (yet): its name, and why.
            detail: if !r.found && r.kind == Kind::Note {
                tr.text("note-not-here", None)
            } else if r.program {
                // Said once, calmly: what opens instead, and that starting it stays yours.
                tr.text("link-program", None)
            } else {
                r.detail.clone()
            },
            how: tr.text(&format!("how-{}", r.how), None),
            when: r.when.and_then(|t| Timestamp::from_second(t).ok()).map(|t| tr.date(&t.to_zoned(TimeZone::system()), true)).unwrap_or_default(),
            key: r.key.clone(),
            found: r.found,
            program: r.program,
        })
        .collect()
}

/// One task in full, for its panel.
#[derive(Debug, Clone, Serialize)]
pub struct DetailView {
    pub card: CardView,
    pub edit: TaskEdit,
    pub steps: Vec<CardView>,
    /// The steps' minutes added up, when they say: "Its steps add up to about 45 minutes".
    pub steps_total: String,
    /// What it waits for and what waits for it: (uid, title).
    pub waits_for: Vec<(String, String)>,
    pub frees: Vec<(String, String)>,
    /// "Thursday 1 October: 25 minutes", newest first.
    pub sessions: Vec<String>,
    pub related: Vec<RelatedView>,
    /// What its tags say it is for, its own area aside: "admin", "work+leisure".
    pub area_tags: String,
    /// How heavy, from its costs: as `CardView::level`.
    pub level: &'static str,
    /// Its first estimate, in minutes; None when unknown (`Task::estimate_first`).
    pub estimate_first: Option<u32>,
    /// The day of the newest rating said after it ("2026-10-06"), "" when none: `edit.felt` is that one.
    pub felt_on: String,
    /// On request: how tasks like it usually go against the first guess, in a
    /// sentence; "" when there is nothing worth saying (`capacity::ratio_line`).
    pub ratio_line: String,
    /// Pinned to a time: its block; None when the plan places it.
    pub pin: Option<PinView>,
    /// "Do at…"'s length by default, in minutes: what the plan lays for it, its margins aside (`blocks::default_minutes`).
    pub pin_minutes: u32,
}

pub fn detail(cx: &Context, task: &Task, sessions: &[crate::timelog::Session], related: &[Related]) -> DetailView {
    let mut steps: Vec<&Task> = cx.tasks.iter().filter(|t| t.parent() == Some(task.uid.as_str()) && t.status != Status::Cancelled).collect();
    let order: BTreeMap<&str, usize> = cx.plan.order.iter().enumerate().map(|(i, u)| (u.as_str(), i)).collect();
    steps.sort_by_key(|t| (!t.status.is_open(), order.get(t.uid.as_str()).copied().unwrap_or(usize::MAX)));
    let total: u32 = steps.iter().fold(0u32, |sum, t| sum.saturating_add(t.estimate));
    let waits_for = cx.planned(&task.uid).map(|p| p.waits_for.iter().map(|u| (u.clone(), cx.title(u))).collect()).unwrap_or_default();
    let frees = cx.plan.items.values().filter(|p| p.waits_for.contains(&task.uid)).map(|p| (p.uid.clone(), cx.title(&p.uid))).collect();
    let mut mine: Vec<&crate::timelog::Session> = sessions.iter().filter(|s| s.task == task.uid).collect();
    mine.sort_by_key(|s| std::cmp::Reverse(s.start));
    let sessions = mine
        .iter()
        .take(8)
        .map(|s| {
            let day = Timestamp::from_second(s.start).map(|t| cx.tr.day(t.to_zoned(TimeZone::system()).date())).unwrap_or_default();
            minutes_text(cx.tr, "task-session-time", s.minutes, Some(day))
        })
        .collect();
    DetailView {
        card: card(cx, task),
        edit: TaskEdit::of(task),
        steps: steps.iter().map(|t| card(cx, t)).collect(),
        steps_total: if total > 0 && steps.len() > 1 { cx.say("task-steps-total", &[("estimate", estimate_text(cx.tr, total).to_lowercase())]) } else { String::new() },
        waits_for,
        frees,
        sessions,
        related: related_views(cx.tr, related),
        area_tags: cx.filter.quiet.as_ref().map_or_else(|| crate::areas::TaskAreas::usual().by_tags(task), |q| q.areas.by_tags(task)).id(),
        level: task.demands.level().map_or("", crate::demands::Level::id),
        estimate_first: task.estimate_first,
        felt_on: task.felt.last().and_then(|f| f.on).map(|d| d.to_string()).unwrap_or_default(),
        ratio_line: String::new(),
        pin: pin_view(cx, &task.uid),
        pin_minutes: crate::blocks::default_minutes(task, cx.planned(&task.uid).map(|p| p.laid).filter(|l| *l > 0), 30),
    }
}

/// What finishing a task changed, in one line: what it frees, or the bigger
/// task now complete. Empty when nothing visible changed.
pub fn done_effect(cx: &Context, before: &Plan, uid: &str) -> String {
    let freed: Vec<String> = cx
        .plan
        .items
        .values()
        .filter(|p| p.column == Column::Ready && before.items.get(&p.uid).is_some_and(|b| b.column == Column::Waiting) && p.uid != uid)
        .map(|p| cx.title(&p.uid))
        .collect();
    if !freed.is_empty() {
        return cx.say("task-done-frees", &[("titles", freed.join(", "))]);
    }
    if let Some(parent) = cx.task(uid).and_then(|t| t.parent()).and_then(|p| cx.planned(p)).filter(|p| p.open_steps == 0) {
        return cx.say("task-done-all-steps", &[("title", cx.title(&parent.uid))]);
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{Settings, plan};
    use crate::tasks::Relation;
    use std::collections::BTreeSet;

    fn task(uid: &str, title: &str) -> Task {
        Task { uid: uid.into(), title: title.into(), estimate: 15, ..Task::default() }
    }

    #[test]
    fn the_now_page_in_words() {
        let today: Date = "2026-10-05".parse().unwrap();
        let mut form = task("form", "Fill the housing form");
        form.due = "2026-10-30".into();
        let mut send = task("send", "Send the certificate");
        send.relations.push(Relation { kind: "DEPENDS-ON".into(), uid: "form".into(), gap: 0 });
        let mut gone = task("gone", "Pay the bill");
        gone.due = "2026-10-01".into();
        gone.relations.push(Relation { kind: "DEPENDS-ON".into(), uid: "form".into(), gap: 0 });
        let tasks = vec![form, send, gone];
        let plan = plan(&tasks, today, &Settings::default(), &BTreeMap::new(), &BTreeSet::new());
        let tr = Translator::new("en");
        let everything = Filter::default();
        let cx = Context { filter: &everything, offices: Offices::always(), tasks: &tasks, plan: &plan, today, tr: &tr, projects: &[], spent: &BTreeMap::new(), stopped: &BTreeMap::new() };
        let view = now(&cx, Weather::Clear, &BTreeSet::new());
        let first = view.now.clone().unwrap();
        assert_eq!(first.title, "Fill the housing form");
        assert_eq!(view.why, vec!["Its date is 30 October: three weeks left.", "It frees two other steps."]);
        assert_eq!((first.due.as_str(), first.estimate.as_str()), ("By 30 October: three weeks left", "About 15 minutes"));
        let waiting = card(&cx, &tasks[2]);
        assert_eq!((waiting.waits.as_str(), waiting.due.as_str()), ("Waits for: Fill the housing form", "Date asked: 1 October"));
        let fr = Translator::new("fr");
        let cx = Context { tr: &fr, ..cx };
        assert_eq!(now(&cx, Weather::Clear, &BTreeSet::new()).why[0], "Sa date est le 30 octobre\u{202f}: encore trois semaines.");
        // Minutes chosen by their number: "1 minute", not "1 minutes".
        assert_eq!((minutes_text(&tr, "task-spent-time", 1, None), minutes_text(&tr, "task-spent-time", 25, None)), ("1 minute so far".to_string(), "25 minutes so far".to_string()));
        assert_eq!(minutes_text(&fr, "task-session-time", 1, Some("lundi 5 octobre".into())), "lundi 5 octobre\u{202f}: 1 minute");
    }

    #[test]
    fn one_kind_at_a_time() {
        let today: Date = "2026-10-05".parse().unwrap();
        let mut call = task("call", "Call the bank");
        call.kind = "call".into();
        call.categories = vec!["Money".into()];
        // Waits for a message: the plan keeps the wait when messages are not shown.
        call.relations.push(Relation { kind: "DEPENDS-ON".into(), uid: "write".into(), gap: 0 });
        let mut write = task("write", "Write to the bank");
        write.kind = "write".into();
        let mut other = task("other", "Call the CAF");
        other.kind = "call".into();
        let tasks = vec![call, write, other];
        let plan = plan(&tasks, today, &Settings::default(), &BTreeMap::new(), &BTreeSet::new());
        let tr = Translator::new("en");
        let calls = Filter { kind: "call".into(), ..Filter::default() };
        let cx = Context { filter: &calls, offices: Offices::always(), tasks: &tasks, plan: &plan, today, tr: &tr, projects: &[], spent: &BTreeMap::new(), stopped: &BTreeMap::new() };
        let titles: Vec<String> = list(&cx, "project", false, "").groups.iter().flat_map(|g| g.rows.iter().map(|r| r.title.clone())).collect();
        // In the plan's order: the free call first, the one that waits after.
        assert_eq!(titles, vec!["Call the CAF", "Call the bank"]);
        assert_eq!(now(&cx, Weather::Clear, &BTreeSet::new()).now.unwrap().title, "Call the CAF", "the bank's call still waits for the message");
        assert_eq!(card(&cx, &tasks[0]).waits, "Waits for: Write to the bank");
        let money = Filter { category: "money".into(), ..Filter::default() };
        let cx = Context { filter: &money, ..cx };
        assert_eq!(board(&cx, None).columns.iter().map(|c| c.cards.len()).sum::<usize>(), 1);
    }

    #[test]
    fn steps_of_each_other_are_still_listed() {
        // Two tasks each a step of the other (a file from elsewhere): listed once each, not lost.
        let today: Date = "2026-10-05".parse().unwrap();
        let step_of = |uid: &str, title: &str, parent: &str| Task { relations: vec![Relation { kind: "PARENT".into(), uid: parent.into(), gap: 0 }], ..task(uid, title) };
        let tasks = vec![step_of("a", "First", "b"), step_of("b", "Second", "a"), task("c", "Third")];
        let plan = plan(&tasks, today, &Settings::default(), &BTreeMap::new(), &BTreeSet::new());
        let tr = Translator::new("en");
        let everything = Filter::default();
        let cx = Context { filter: &everything, offices: Offices::always(), tasks: &tasks, plan: &plan, today, tr: &tr, projects: &[], spent: &BTreeMap::new(), stopped: &BTreeMap::new() };
        let mut titles: Vec<String> = list(&cx, "project", false, "").groups.iter().flat_map(|g| g.rows.iter().map(|r| r.title.clone())).collect();
        titles.sort();
        assert_eq!(titles, vec!["First", "Second", "Third"]);
    }
}

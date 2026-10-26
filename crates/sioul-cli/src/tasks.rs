// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Tasks, notes, links and focus from the terminal: the next step and why,
//! the list, the board, the timeline; adding a task in one line; importing a
//! whole plan from a file (for agents and scripts); following links.

use crate::{Session, one_line, plain_lines};
use clap::Subcommand;
use jiff::{Timestamp, Zoned, tz::TimeZone};
use serde::Deserialize;
use sioul_core::capture;
use sioul_core::compose::Draft;
use sioul_core::config::Account;
use sioul_core::contacts::{self, ContactEdit, Labeled};
use sioul_core::links::{self, Loaded};
use sioul_core::notes;
use sioul_core::plan::{self, Plan, Settings};
use sioul_core::taskview::{self, CardView, Context, Filter};
use sioul_core::tasks::{self, ContactRef, Link, Status, Task, TaskEdit};
use sioul_core::timelog;
use sioul_core::today::{Today, Weather};
use sioul_core::vdir::{self, Collection, Kind};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub(crate) enum TasksCommand {
    /// The next step, why, and the one after it.
    Now,
    /// Every open task, a bigger task followed by its steps, in the plan's order.
    List {
        /// "project" (default) or "list"; "case", the word before, reads as "project".
        #[arg(long, default_value = "project")]
        by: String,
        /// Also the tasks done in the last two weeks.
        #[arg(long)]
        done: bool,
        query: Option<String>,
    },
    /// Free to start, started, waiting, done.
    Board,
    /// When each open task could happen, from today.
    Timeline {
        /// One project's tasks, by its id.
        #[arg(long, alias = "case")]
        project: Option<String>,
    },
    /// One task in full, with what it is tied to.
    Show { task: String },
    /// Adds a task from one line: "Call the CAF tomorrow ~15m #housing {30/10}".
    Add {
        line: Vec<String>,
        /// The list ("account/id"); default: the first list made for tasks.
        #[arg(long)]
        list: Option<String>,
        /// The task it is a step of.
        #[arg(long)]
        parent: Option<String>,
        /// A task it waits for.
        #[arg(long)]
        after: Vec<String>,
        /// Write it here only; the next sync sends it.
        #[arg(long)]
        no_sync: bool,
    },
    /// Moves a step in its bigger task's sequence: before another step of it,
    /// else after them all.
    Move {
        step: String,
        /// The step it goes before.
        #[arg(long)]
        before: Option<String>,
        /// Write it here only; the next sync sends it.
        #[arg(long)]
        no_sync: bool,
    },
    /// Makes a task a step of a bigger one, or with "none" a task of its own again.
    Parent {
        task: String,
        /// The bigger task (its UID, or the start of its title), or "none".
        bigger: String,
        /// Move it all the same into a list that would not keep everything it uses.
        #[arg(long)]
        confirm: bool,
        /// Write it here only; the next sync sends it.
        #[arg(long)]
        no_sync: bool,
    },
    /// Ties a task to the appointment it prepares, or with "none" unties it.
    Attach {
        task: String,
        /// The event: its UID, or words of its title; "none" to untie.
        event: String,
        /// Write it here only; the next sync sends it.
        #[arg(long)]
        no_sync: bool,
    },
    /// Marks a task done (a repeating one comes back at its next turn).
    Done { task: String },
    /// Marks a task started.
    Start { task: String },
    /// Puts a task off until tomorrow.
    NotNow { task: String },
    /// How today is: clear, haze or fog.
    Weather { weather: String },
    /// The task lists.
    Lists,
    /// Makes a task list; the next sync creates it on the account's server.
    NewList {
        account: String,
        name: String,
        #[arg(long)]
        color: Option<String>,
        /// Also takes events.
        #[arg(long)]
        events: bool,
    },
    /// Imports tasks, contacts and drafts from a file (docs/tasks.md, "Importing").
    /// Importing again updates what the file made, by its keys.
    Import {
        file: PathBuf,
        /// Write here only; the next sync sends it.
        #[arg(long)]
        no_sync: bool,
        /// Say what would change, change nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum FocusCommand {
    /// Starts a focus session on a task.
    Start {
        task: String,
        #[arg(long, default_value_t = 25)]
        minutes: u32,
    },
    /// The session running now.
    Status,
    /// Ends the session; --done marks the task done; --note keeps where you stopped.
    Stop {
        #[arg(long)]
        done: bool,
        #[arg(long)]
        note: Option<String>,
    },
}

/// Tasks, the plan and the day, read once.
struct Desk {
    loaded: Loaded,
    filter: Filter,
    offices: taskview::Offices,
    plan: Plan,
    today: Today,
    spent: BTreeMap<String, u32>,
    stopped: BTreeMap<String, String>,
    sessions: Vec<timelog::Session>,
    /// The room the plan was made with: the timeline's days of rest come from it.
    settings: Settings,
}

/// The room for tasks: your hours, each kind for its own tasks, less the
/// events of the coming four weeks and the blocks tasks are pinned to;
/// today's from now, scaled by the weather.
pub(crate) fn settings(s: &Session, weather: Weather, situation: &sioul_core::quiet::Situation, projects: &[sioul_core::projects::Project], tasks: &[Task]) -> Settings {
    let now = Zoned::now();
    let midnight = now.date().to_zoned(now.time_zone().clone()).map_or(0, |z| z.timestamp().as_second());
    let read = sioul_core::agenda::occurrences(midnight, midnight + 28 * 86_400);
    // As the window plans: the tasks pinned to a time, their blocks never events of their own (docs/tasks.md, "Pinned to a time").
    let stamp = now.timestamp().as_second();
    let mut found: Vec<sioul_core::agenda::Occurrence> = read.iter().filter(|e| !e.task.is_empty()).cloned().collect();
    let chosen = s.config.tasks.blocks.clone().unwrap_or_default();
    found.extend(sioul_core::blocks::read(&sioul_core::blocks::calendars(&chosen), stamp - 2 * 86_400, stamp + sioul_core::blocks::AHEAD_DAYS * 86_400, now.time_zone()));
    let pins = sioul_core::blocks::Blocks::of(&found, tasks, stamp, now.time_zone());
    let events = sioul_core::blocks::without_blocks(read.clone(), tasks);
    // Meals, naps and the night first: the work goes around them, as each day has them,
    // meals pushed past the events they would fall in, as the window plans.
    // Health's `[needs]` alone: this process keeps no medicine (docs/mcp.md).
    let needs = sioul_core::needs::Needs::read(&sioul_core::health::Health::default_path());
    let days = sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path());
    // Meals move past the blocks as past any event.
    let pushed = needs.past_events_on(now.date(), now.time_zone(), &days, &sioul_core::plan::event_spans(&read, 0));
    // Today's end of work moved by free time, as the window plans (docs/pauses.md).
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let day = sioul_core::pause::Day::of(&s.config, sioul_core::quiet::Blocks::read(&now, &read), &events);
    let extension = sioul_core::pause::moved_today(&overrides, &day.evening(), &now, s.config.free_time.moves);
    let mut settings = Settings::of_hours(&s.config.week_hours(), sioul_core::areas::TaskAreas::of_config(&s.config, projects)).with_needs(&needs, pushed).with_days(days).with_extension(extension).with_pins(pins).with_events(&now, &events);
    settings.default_estimate = s.config.tasks.estimate.unwrap_or(settings.default_estimate);
    // As the window plans: how today is says how many heavy tasks it takes, no
    // heavier than a hazy day after a pause.
    (settings.today_percent, settings.heavy_today) = sioul_core::pause::today_level(&overrides, now.date(), weather);
    settings.closed = situation.closed.clone();
    settings.office_days = situation.office_days;
    // Calls to offices only while they are open, as the window lays them out.
    settings.office_hours = s.config.office_hours();
    settings
}

impl Desk {
    fn read(s: &Session) -> Desk {
        let loaded = Loaded::read(&s.config);
        let date = Zoned::now().date();
        let today = Today::load(&Today::default_path(), date);
        let sessions = timelog::sessions();
        let spent = timelog::spent(&sessions, 0, i64::MAX);
        let stopped = sessions.iter().filter(|x| !x.note.is_empty()).map(|x| (x.task.clone(), x.note.clone())).collect();
        let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
        let now = Zoned::now();
        let situation = sioul_core::quiet::Situation::now(&s.config, &overrides, &sioul_core::quiet::Blocks::read_now(&now), &now, &s.tr, &loaded.projects);
        let mut settings = settings(s, today.weather, &situation, &loaded.projects, &loaded.tasks);
        // As the window plans: what your record says of a day (docs/capacity.md), a block counted as its task.
        let own = |from: i64, to: i64| sioul_core::blocks::without_blocks(sioul_core::agenda::occurrences(from, to), &loaded.tasks);
        settings.capacity = sioul_core::capacity::gather(&loaded.tasks, &sessions, &settings, &s.config.planning, &own, &now).planning;
        // As the window plans: a task that prepares an appointment gone waits for a date (`pairs`).
        settings.appointments = Some(sioul_core::pairs::appointments_here(&loaded.events, now.time_zone()));
        let plan = plan::plan(&loaded.tasks, date, &settings, &spent, &today.aside);
        let filter = Filter { quiet: situation.quiet_tasks(), ..Filter::default() };
        Desk { loaded, filter, offices: situation.offices, plan, today, spent, stopped, sessions, settings }
    }

    fn context<'a>(&'a self, s: &'a Session) -> Context<'a> {
        Context { filter: &self.filter, offices: self.offices.clone(), tasks: &self.loaded.tasks, plan: &self.plan, today: Zoned::now().date(), tr: &s.tr, projects: &self.loaded.projects, spent: &self.spent, stopped: &self.stopped }
    }

    /// A task by UID, else by the start of its title, else by a word of it; one only.
    fn find(&self, s: &Session, wanted: &str) -> Result<&Task, String> {
        let tasks = &self.loaded.tasks;
        if let Some(task) = tasks.iter().find(|t| t.uid == wanted) {
            return Ok(task);
        }
        let lower = wanted.to_lowercase();
        let starting: Vec<&Task> = tasks.iter().filter(|t| t.title.to_lowercase().starts_with(&lower)).collect();
        let found: Vec<&Task> = if starting.is_empty() { tasks.iter().filter(|t| t.title.to_lowercase().contains(&lower)).collect() } else { starting };
        match found.as_slice() {
            [one] => Ok(one),
            [] => Err(s.say("task-not-found", &[("what", wanted.to_string())])),
            many => Err(s.say("task-ambiguous", &[("what", wanted.to_string()), ("titles", many.iter().take(5).map(|t| format!("{} ({})", t.title, t.uid)).collect::<Vec<_>>().join("; "))])),
        }
    }
}

pub(crate) fn run(s: &Session, command: TasksCommand) -> Result<(), String> {
    match command {
        TasksCommand::Now => now(s),
        TasksCommand::List { by, done, query } => list(s, &by, done, query.as_deref().unwrap_or("")),
        TasksCommand::Board => board(s),
        TasksCommand::Timeline { project } => timeline(s, project.as_deref()),
        TasksCommand::Show { task } => show(s, &task),
        TasksCommand::Add { line, list, parent, after, no_sync } => add(s, &line.join(" "), list.as_deref(), parent.as_deref(), &after, no_sync),
        TasksCommand::Move { step, before, no_sync } => move_step(s, &step, before.as_deref(), no_sync),
        TasksCommand::Parent { task, bigger, confirm, no_sync } => parent(s, &task, &bigger, confirm, no_sync),
        TasksCommand::Attach { task, event, no_sync } => attach(s, &task, &event, no_sync),
        TasksCommand::Done { task } => set_status(s, &task, Status::Completed),
        TasksCommand::Start { task } => set_status(s, &task, Status::InProcess),
        TasksCommand::NotNow { task } => not_now(s, &task),
        TasksCommand::Weather { weather } => set_weather(s, &weather),
        TasksCommand::Lists => lists(s),
        TasksCommand::NewList { account, name, color, events } => new_list(s, &account, &name, color.as_deref(), events),
        TasksCommand::Import { file, no_sync, dry_run } => import(s, &file, no_sync, dry_run),
    }
}

fn print_card(card: &CardView, indent: usize) {
    let pad = "  ".repeat(indent);
    let mark = match card.status {
        Status::Completed => "✓",
        Status::InProcess => "▸",
        Status::Cancelled => "×",
        Status::NeedsAction => "○",
    };
    // Titles and UIDs come from shared lists and servers: one line, no escape sequences.
    println!("{pad}{mark} {}  [{}]", one_line(&card.title), one_line(&card.uid));
    for line in [&card.due, &card.estimate, &card.waits, &card.unblocks, &card.steps, &card.stopped, &card.tight, &card.done_on] {
        if !line.is_empty() {
            println!("{pad}    {}", one_line(line));
        }
    }
}

fn now(s: &Session) -> Result<(), String> {
    let desk = Desk::read(s);
    let cx = desk.context(s);
    let view = taskview::now(&cx, desk.today.weather, &desk.today.aside);
    if !view.weather_note.is_empty() {
        println!("{}\n", one_line(&view.weather_note));
    }
    match &view.now {
        Some(card) => {
            println!("{}", s.tr.text("task-now-title", None));
            print_card(card, 0);
            for why in &view.why {
                println!("    {}", one_line(why));
            }
            if !view.picked.is_empty() {
                println!("    {}", one_line(&view.picked));
            }
            if let Some(then) = &view.then {
                println!("\n{}", s.tr.text("task-then", None));
                print_card(then, 0);
            }
        }
        None => println!("{}", one_line(&view.empty)),
    }
    for line in view.loops.iter().chain(std::iter::once(&view.wip)).filter(|l| !l.is_empty()) {
        println!("\n{}", one_line(line));
    }
    Ok(())
}

fn list(s: &Session, by: &str, done: bool, query: &str) -> Result<(), String> {
    let desk = Desk::read(s);
    let view = taskview::list(&desk.context(s), by, done, query);
    for group in &view.groups {
        println!("\n{}", one_line(&group.title));
        for card in &group.rows {
            print_card(card, card.depth + 1);
        }
    }
    Ok(())
}

fn board(s: &Session) -> Result<(), String> {
    let desk = Desk::read(s);
    let view = taskview::board(&desk.context(s), None);
    for column in &view.columns {
        println!("\n{}", one_line(&column.title));
        for card in &column.cards {
            print_card(card, 1);
        }
    }
    if !view.wip.is_empty() {
        println!("\n{}", one_line(&view.wip));
    }
    Ok(())
}

fn timeline(s: &Session, project: Option<&str>) -> Result<(), String> {
    let desk = Desk::read(s);
    // The days of rest of the room the plan was made with, as in the window.
    let rest = desk.settings.rest_days();
    let view = taskview::timeline(&desk.context(s), project, &rest);
    let width = view.days.len();
    for row in &view.rows {
        let mut bar: Vec<char> = (0..width).map(|i| if view.days[i].rest { '·' } else { ' ' }).collect();
        // Only the days drawn: a day out of them is left out, never a panic.
        for i in row.start.max(0)..(row.start + row.length).min(width as i64) {
            bar[i as usize] = if row.has_steps { '═' } else { '█' };
        }
        if let Some(slot) = row.due.and_then(|due| usize::try_from(due).ok()).and_then(|due| bar.get_mut(due)) {
            *slot = '◆';
        }
        let title: String = format!("{}{}", "  ".repeat(row.depth), one_line(&row.title)).chars().take(40).collect();
        println!("{title:<40} {}", bar.into_iter().collect::<String>());
    }
    println!("\n{}", one_line(&view.note));
    Ok(())
}

fn show(s: &Session, wanted: &str) -> Result<(), String> {
    let desk = Desk::read(s);
    let task = desk.find(s, wanted)?;
    let related = desk.loaded.world().related(&links::task_uri(&task.uid));
    let view = taskview::detail(&desk.context(s), task, &desk.sessions, &related);
    print_card(&view.card, 0);
    if !task.notes.is_empty() {
        println!("\n{}", plain_lines(&task.notes));
    }
    for step in &view.steps {
        print_card(step, 1);
    }
    if !view.steps_total.is_empty() {
        println!("  {}", one_line(&view.steps_total));
    }
    for r in &view.related {
        let when = if r.when.is_empty() { String::new() } else { format!(" · {}", one_line(&r.when)) };
        let gone = if r.found { "" } else { " ?" };
        println!("  {}: {}{when}{gone}  <{}>", one_line(&r.how), one_line(&r.title), one_line(&r.uri));
    }
    for session in &view.sessions {
        println!("  {}", one_line(session));
    }
    Ok(())
}

/// The list a new task goes into: the one named ("account/id"), else the first made for tasks.
fn target_list(s: &Session, named: Option<&str>) -> Result<Collection, String> {
    match named {
        Some(id) => tasks::lists().into_iter().find(|c| format!("{}/{}", c.account, c.id) == id && !c.read_only).ok_or_else(|| s.say("task-no-such-list", &[("list", id.to_string())])),
        None => tasks::default_list().ok_or_else(|| s.tr.text("task-no-list", None)),
    }
}

fn sync_list(s: &Session, list: &Collection) -> Result<(), String> {
    match s.config.account(&list.account).filter(|a| a.is_dav()) {
        Some(account) => crate::dav::sync_one(s, account),
        None => Ok(()),
    }
}

fn add(s: &Session, line: &str, list: Option<&str>, parent: Option<&str>, after: &[String], no_sync: bool) -> Result<(), String> {
    let desk = Desk::read(s);
    let projects: Vec<String> = desk.loaded.projects.iter().map(|c| c.id.clone()).collect();
    let captured = capture::capture(line, Zoned::now().date(), &projects, &s.config.task_kinds(&s.tr));
    let mut edit = captured.edit;
    if edit.title.is_empty() {
        return Err(s.tr.text("task-no-title", None));
    }
    if let Some(parent) = parent {
        edit.parent = desk.find(s, parent)?.uid.clone();
        // After its bigger task's other steps.
        edit.sort_order = Some(tasks::next_in_sequence(tasks::steps_of(&desk.loaded.tasks, &edit.parent), &Zoned::now()));
    }
    for wanted in after {
        edit.waits_for.push(desk.find(s, wanted)?.uid.clone());
    }
    let target = target_list(s, list)?;
    let text = tasks::new_task(&edit, &vdir::new_name(), &TimeZone::system(), &Zoned::now())?;
    let path = tasks::new_path(&target);
    vdir::write_item(&path, &text)?;
    for chip in &captured.chips {
        // In your words, as the window shows them: "date asked: 2026-10-30".
        let value = if chip.kind == "kind" { s.tr.text(&format!("task-kind-{}", chip.value), None) } else { chip.value.clone() };
        println!("  {}: {value}", s.tr.text(&format!("chip-{}", chip.kind), None));
    }
    println!("{}", path.display());
    if no_sync { Ok(()) } else { sync_list(s, &target) }
}

fn write_task(s: &Session, task: &Task, text: &str) -> Result<(), String> {
    vdir::write_item(Path::new(&task.key), text)?;
    let list = tasks::lists().into_iter().find(|c| format!("{}/{}", c.account, c.id) == task.list_id);
    match list {
        Some(list) => sync_list(s, &list),
        None => Ok(()),
    }
}

/// A step moved in its bigger task's sequence (`tasks::reorder`): before
/// `before`, a step of the same task, else after them all. One file written
/// when there is room, else those renumbered; the steps then listed in order.
fn move_step(s: &Session, wanted: &str, before: Option<&str>, no_sync: bool) -> Result<(), String> {
    let desk = Desk::read(s);
    let step = desk.find(s, wanted)?;
    let Some(parent) = step.parent() else {
        return Err(s.say("task-not-a-step", &[("title", one_line(&step.title))]));
    };
    let steps = tasks::steps_of(&desk.loaded.tasks, parent);
    let before = match before {
        Some(wanted) => {
            let other = desk.find(s, wanted)?;
            if other.parent() != Some(parent) {
                return Err(s.say("task-not-a-step-of", &[("title", one_line(&other.title)), ("whole", one_line(&desk.loaded.tasks.iter().find(|t| t.uid == parent).map_or(parent, |t| t.title.as_str())))]));
            }
            Some(other.uid.clone())
        }
        None => None,
    };
    let writes = tasks::reorder(&steps, &step.uid, before.as_deref())?;
    let file_of = |uid: &str| steps.iter().find(|t| t.uid == uid).copied();
    if writes.iter().any(|(uid, _)| file_of(uid).is_none_or(|t| t.read_only)) {
        return Err(s.tr.text("task-read-only", None));
    }
    let now = Zoned::now();
    let mut lists: BTreeSet<String> = BTreeSet::new();
    for (uid, place) in &writes {
        let Some(task) = file_of(uid) else { continue };
        let current = std::fs::read_to_string(&task.key).map_err(|e| format!("{}: {e}", task.key))?;
        let text = tasks::set_sequence(&current, *place, &now)?;
        if text != current {
            vdir::write_item(Path::new(&task.key), &text)?;
            lists.insert(task.list_id.clone());
        }
    }
    let after = Desk::read(s);
    for (k, step) in tasks::steps_of(&after.loaded.tasks, parent).iter().enumerate() {
        println!("{}. {}  [{}]", k + 1, one_line(&step.title), one_line(&step.uid));
    }
    if no_sync {
        return Ok(());
    }
    for list in tasks::lists().into_iter().filter(|c| lists.contains(&format!("{}/{}", c.account, c.id))) {
        sync_list(s, &list)?;
    }
    Ok(())
}

/// A task made a step of `bigger`, or with "none" a task of its own again
/// (`composed::demote`, `promote`): what its list would not keep said, and
/// done with `--confirm`.
fn parent(s: &Session, wanted: &str, bigger: &str, confirm: bool, no_sync: bool) -> Result<(), String> {
    let desk = Desk::read(s);
    let task = desk.find(s, wanted)?.clone();
    let now = Zoned::now();
    let keeper = |c: &Collection| sioul_core::capabilities::provider_of_collection(s.config.account(&c.account), c);
    let (planned, line) = if bigger.trim().eq_ignore_ascii_case("none") {
        (sioul_core::composed::promote(&desk.loaded.tasks, &task.uid, &now), s.say("composed-made-own", &[("title", one_line(&task.title))]))
    } else {
        let whole = desk.find(s, bigger)?;
        (sioul_core::composed::demote(&desk.loaded.tasks, &task.uid, &whole.uid, &tasks::lists(), &keeper, confirm, &now), s.say("composed-made-step", &[("title", one_line(&task.title)), ("whole", one_line(&whole.title))]))
    };
    let change = planned.map_err(|r| match r {
        sioul_core::composed::Refusal::Losses(_) => format!("{} (--confirm)", r.said(&s.tr)),
        r => r.said(&s.tr),
    })?;
    let files = change.files();
    change.write()?;
    println!("{line}");
    if no_sync {
        return Ok(());
    }
    for list in tasks::lists().into_iter().filter(|c| files.iter().any(|f| f.parent() == Some(c.dir.as_path()))) {
        sync_list(s, &list)?;
    }
    Ok(())
}

/// A task tied to the appointment it prepares (`pairs::attach`), or untied
/// with "none" (`pairs::detach`); the event by its UID, else words of its title, one only.
fn attach(s: &Session, wanted: &str, event: &str, no_sync: bool) -> Result<(), String> {
    let desk = Desk::read(s);
    let task = desk.find(s, wanted)?.clone();
    let now = Zoned::now();
    let file_of = |key: &str| -> Result<(PathBuf, String, bool), String> {
        let path = PathBuf::from(key);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{key}: {e}"))?;
        let writable = vdir::collections(Kind::Calendars).into_iter().find(|c| path.parent() == Some(c.dir.as_path())).is_some_and(|c| !c.read_only);
        Ok((path, text, writable))
    };
    let (change, line) = if event.trim().eq_ignore_ascii_case("none") {
        let found = sioul_core::pairs::appointment_of(&task).and_then(|uid| desk.loaded.events.iter().find(|e| e.uid == uid)).map(|e| file_of(&e.key)).transpose()?;
        let file = found.as_ref().map(|(path, text, writable)| sioul_core::pairs::EventFile { path, text, writable: *writable });
        (sioul_core::pairs::detach(&task, file.as_ref(), &now).map_err(|r| r.said(&s.tr))?, s.say("pair-detached", &[("title", one_line(&task.title))]))
    } else {
        let lower = event.to_lowercase();
        let found: Vec<&sioul_core::agenda::EventRef> = match desk.loaded.events.iter().find(|e| e.uid == event) {
            Some(one) => vec![one],
            None => desk.loaded.events.iter().filter(|e| e.summary.to_lowercase().contains(&lower)).collect(),
        };
        let one = match found.as_slice() {
            [one] => *one,
            [] => return Err(s.say("pair-event-not-found", &[("what", one_line(event))])),
            many => return Err(s.say("pair-event-ambiguous", &[("what", one_line(event)), ("titles", many.iter().take(5).map(|e| format!("{} ({})", one_line(&e.summary), e.uid)).collect::<Vec<_>>().join("; "))])),
        };
        let (path, text, writable) = file_of(&one.key)?;
        let file = sioul_core::pairs::EventFile { path: &path, text: &text, writable };
        (sioul_core::pairs::attach(&desk.loaded.tasks, &task.uid, &file, &desk.loaded.events, &sioul_core::pairs::links_kept(&s.config), &now).map_err(|r| r.said(&s.tr))?, s.say("pair-attached", &[("title", one_line(&task.title)), ("event", one_line(&one.summary))]))
    };
    let files = change.files();
    change.write()?;
    println!("{line}");
    if no_sync {
        return Ok(());
    }
    for list in vdir::collections(Kind::Calendars).into_iter().filter(|c| files.iter().any(|f| f.parent() == Some(c.dir.as_path()))) {
        sync_list(s, &list)?;
    }
    Ok(())
}

fn set_status(s: &Session, wanted: &str, status: Status) -> Result<(), String> {
    let desk = Desk::read(s);
    let task = desk.find(s, wanted)?;
    if task.read_only {
        return Err(s.tr.text("task-read-only", None));
    }
    let current = std::fs::read_to_string(&task.key).map_err(|e| format!("{}: {e}", task.key))?;
    let text = tasks::set_status(&current, status, &TimeZone::system(), &Zoned::now())?;
    vdir::write_item(Path::new(&task.key), &text)?;
    // Its bigger task, without an estimate of its own, done with its last step (docs/tasks.md, "Composed tasks");
    // only a step done for good: a repeating one's turn comes back open, and closes nothing.
    if status == Status::Completed && sioul_core::composed::done_for_good(&text) {
        let closing = sioul_core::composed::close_with(&desk.loaded.tasks, &task.uid, &Zoned::now()).map_err(|r| r.said(&s.tr))?;
        if !closing.is_empty() {
            closing.write()?;
            let whole = sioul_core::composed::closing_with(&desk.loaded.tasks, &task.uid).first().map(|t| t.title.clone()).unwrap_or_default();
            println!("{}", s.say("composed-done-too", &[("whole", one_line(&whole))]));
            return write_task(s, task, &text);
        }
    }
    if status == Status::Completed {
        // What finishing it changed, said once.
        let after = Desk::read(s);
        let effect = taskview::done_effect(&after.context(s), &desk.plan, &task.uid);
        println!("{}", if effect.is_empty() { s.tr.text("task-done-said", None) } else { format!("{} {}", s.tr.text("task-done-said", None), one_line(&effect)) });
    }
    write_task(s, task, &text)
}

fn not_now(s: &Session, wanted: &str) -> Result<(), String> {
    let desk = Desk::read(s);
    let uid = desk.find(s, wanted)?.uid.clone();
    let mut today = desk.today;
    today.aside.insert(uid);
    today.save(&Today::default_path())
}

fn set_weather(s: &Session, weather: &str) -> Result<(), String> {
    let mut today = Today::load(&Today::default_path(), Zoned::now().date());
    today.weather = Weather::parse(weather);
    today.save(&Today::default_path())?;
    // The morning's weather, kept with the day's reviews (docs/reviews.md).
    let said = Zoned::now();
    let _ = sioul_core::reviews::note_weather(&sioul_core::reviews::Reviews::default_path(), said.date(), today.weather, said.timestamp().as_second(), said.time_zone());
    now(s)
}

fn lists(s: &Session) -> Result<(), String> {
    for list in tasks::lists() {
        let pending = if vdir::State::load(&list.state_path()).pending { " …" } else { "" };
        let only = if list.read_only { format!(" {}", s.tr.text("list-read-only-mark", None)) } else { String::new() };
        println!("{}/{}  {}{only}{pending}  {}", one_line(&list.account), one_line(&list.id), one_line(&list.name), list.items().len());
    }
    Ok(())
}

fn new_list(s: &Session, account: &str, name: &str, color: Option<&str>, events: bool) -> Result<(), String> {
    let known: Option<&Account> = s.config.account(account);
    if known.is_none_or(|a| !a.is_dav()) && account != "local" {
        return Err(s.say("account-unknown", &[("id", account.to_string())]));
    }
    let components: &[&str] = if events { &["VEVENT", "VTODO"] } else { &["VTODO"] };
    let list = vdir::create(Kind::Calendars, account, name, color, components)?;
    println!("{}/{}", list.account, list.id);
    Ok(())
}

/// A file to import: tasks, contacts and drafts, each with a key of its own
/// (docs/tasks.md, "Importing"). Keys make UIDs stable: importing again
/// updates what the file made instead of adding it twice.
#[derive(Debug, Default, Deserialize)]
struct Plan_ {
    /// Where new tasks go: "account/id".
    #[serde(default)]
    list: Option<String>,
    /// Where new contacts go: "account/id".
    #[serde(default)]
    book: Option<String>,
    /// UIDs are this prefix and the key.
    #[serde(default)]
    prefix: String,
    #[serde(default, rename = "task")]
    tasks: Vec<ImportTask>,
    #[serde(default, rename = "contact")]
    contacts: Vec<ImportContact>,
    #[serde(default, rename = "draft")]
    drafts: Vec<ImportDraft>,
    /// Where new appointments go: "account/id" (else the agenda's usual calendar).
    #[serde(default)]
    calendar: Option<String>,
    /// Appointments: events, said so by the file (21 October 2026). Which an
    /// item is, a task or an appointment, is the file's to say, never guessed
    /// from its times.
    #[serde(default, rename = "event")]
    events: Vec<ImportEvent>,
}

/// An appointment: an event in a calendar, never a task.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportEvent {
    key: String,
    title: String,
    /// "2026-10-23T14:00" (your time zone), or a day for a whole-day one.
    start: String,
    /// The same form; unsaid, an hour after a time, the same day for a day.
    /// Unsaid on an event imported before: what it holds stays (F4).
    #[serde(default)]
    end: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    /// "daily", "weekly", "monthly", "yearly".
    #[serde(default)]
    repeat: Option<String>,
    /// Its calendar, "account/id"; else the file's, else the agenda's usual.
    #[serde(default)]
    calendar: Option<String>,
}

/// What a task in an import never has, an appointment does: met in a
/// `[[task]]`, the import asks which it is instead of guessing.
const ONLY_EVENTS: &[&str] = &["end", "all_day", "calendar", "attendees", "organizer"];

#[derive(Debug, Default, Deserialize)]
struct ImportTask {
    key: String,
    title: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    start: String,
    #[serde(default)]
    due: String,
    #[serde(default)]
    estimate: u32,
    #[serde(default)]
    priority: u8,
    #[serde(default)]
    tags: Vec<String>,
    /// Its projects' ids; `cases`, their name in files written before, reads too.
    #[serde(default, alias = "cases")]
    projects: Vec<String>,
    /// The key of the bigger task, or `uid:<UID>`.
    #[serde(default)]
    parent: String,
    /// Keys (or `uid:<UID>`) it waits for.
    #[serde(default)]
    after: Vec<String>,
    /// Keys it waits for, with the days to wait after each is done.
    #[serde(default)]
    after_gap: BTreeMap<String, u32>,
    /// Notes in the notes folder, by path ("admin/letters.md").
    #[serde(default)]
    note: Vec<String>,
    #[serde(default)]
    links: Vec<Link>,
    /// Keys of contacts in this file.
    #[serde(default)]
    contacts: Vec<String>,
    /// Keys of drafts in this file.
    #[serde(default)]
    drafts: Vec<String>,
    /// "completed", "in-process", "cancelled"; unsaid, a new task is to do and an old one keeps its state.
    #[serde(default)]
    status: Option<Status>,
    /// The day it was done ("2026-10-01"): done, on that day.
    #[serde(default)]
    done: String,
    /// What else the entry says: never read as a task's, said (`ONLY_EVENTS`: asked).
    #[serde(flatten)]
    other: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct ImportContact {
    key: String,
    name: String,
    #[serde(default)]
    org: String,
    #[serde(default)]
    emails: Vec<String>,
    #[serde(default)]
    phones: Vec<String>,
    #[serde(default)]
    address: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    notes: String,
}

#[derive(Debug, Default, Deserialize)]
struct ImportDraft {
    key: String,
    account: String,
    #[serde(default)]
    to: Vec<String>,
    #[serde(default)]
    cc: Vec<String>,
    subject: String,
    body: String,
}

/// A key as a UID: the prefix, then the key.
fn uid_of(prefix: &str, key: &str) -> String {
    match key.strip_prefix("uid:") {
        Some(uid) => uid.to_string(),
        None => format!("{prefix}{key}"),
    }
}

/// A draft's id holds letters and digits only (`Draft::by_id`).
fn draft_id(prefix: &str, key: &str) -> String {
    format!("{prefix}{key}").chars().filter(char::is_ascii_alphanumeric).collect()
}

fn import(s: &Session, file: &Path, no_sync: bool, dry_run: bool) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let plan: Plan_ = toml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    // A new task's or contact's file is named by its UID: none may lead out of
    // its folder ("../../.bashrc"), whoever wrote the file (an agent, a script).
    for key in plan.tasks.iter().map(|t| &t.key).chain(plan.contacts.iter().map(|c| &c.key)).chain(plan.events.iter().map(|e| &e.key)) {
        let uid = uid_of(&plan.prefix, key);
        if uid.is_empty() || uid.contains(['/', '\\']) || (cfg!(windows) && uid.contains(':')) {
            return Err(s.say("import-bad-key", &[("key", key.clone())]));
        }
    }
    // A task or an appointment: the file says which, or is asked.
    for task in &plan.tasks {
        if plan.events.iter().any(|e| e.key == task.key) {
            return Err(s.say("import-both-kinds", &[("key", task.key.clone())]));
        }
        let kind_says_event = task.other.get("kind").and_then(|v| v.as_str()).is_some_and(|k| matches!(k.to_ascii_lowercase().as_str(), "event" | "appointment"));
        if let Some(field) = task.other.keys().find(|k| ONLY_EVENTS.contains(&k.as_str())).cloned().or(kind_says_event.then(|| "kind".to_string())) {
            return Err(s.say("import-task-or-event", &[("task", task.key.clone()), ("field", field)]));
        }
        for field in task.other.keys() {
            eprintln!("{}", s.say("import-unknown-field", &[("task", task.key.clone()), ("field", field.clone())]));
        }
    }
    let zone = TimeZone::system();
    let now = Zoned::now();
    let existing = tasks::all(&zone);
    let target = target_list(s, plan.list.as_deref())?;
    let keys: BTreeSet<&str> = plan.tasks.iter().map(|t| t.key.as_str()).collect();
    let missing = |key: &str| !key.starts_with("uid:") && !keys.contains(key);
    for task in &plan.tasks {
        for key in task.after.iter().chain(task.after_gap.keys()).chain((!task.parent.is_empty()).then_some(&task.parent)) {
            if missing(key) {
                return Err(s.say("import-unknown-key", &[("key", key.clone()), ("task", task.key.clone())]));
            }
        }
    }

    // Contacts first: tasks name them.
    let mut contact_refs: BTreeMap<String, ContactRef> = BTreeMap::new();
    let everyone = contacts::all();
    let book = match plan.book.as_deref() {
        Some(id) => vdir::collections(Kind::Contacts).into_iter().find(|c| format!("{}/{}", c.account, c.id) == id && !c.read_only),
        None => contacts::default_book(),
    };
    let mut changed_books: BTreeSet<String> = BTreeSet::new();
    for contact in &plan.contacts {
        let uid = uid_of(&plan.prefix, &contact.key);
        let edit = ContactEdit {
            name: contact.name.clone(),
            org: contact.org.clone(),
            emails: contact.emails.iter().map(|e| Labeled { label: String::new(), value: e.clone() }).collect(),
            phones: contact.phones.iter().map(|p| Labeled { label: String::new(), value: p.clone() }).collect(),
            addresses: if contact.address.is_empty() { Vec::new() } else { vec![Labeled { label: String::new(), value: contacts::tidy_address(&contact.address.replace(", ", "\n")) }] },
            urls: if contact.url.is_empty() { Vec::new() } else { vec![contact.url.clone()] },
            notes: contact.notes.clone(),
            ..ContactEdit::default()
        };
        let (path, new_text) = match everyone.iter().find(|c| c.uid == uid) {
            Some(found) => {
                let current = std::fs::read_to_string(&found.key).map_err(|e| e.to_string())?;
                (PathBuf::from(&found.key), contacts::apply(&current, &edit))
            }
            None => {
                let book = book.clone().ok_or_else(|| s.tr.text("dav-no-book", None))?;
                (book.dir.join(format!("{uid}.vcf")), contacts::new_card_with_uid(&edit, &uid))
            }
        };
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        if old != new_text {
            println!("{} {}", if old.is_empty() { "+" } else { "~" }, one_line(&contact.name));
            if !dry_run {
                vdir::write_item(&path, &new_text)?;
                if let Some(account) = path.strip_prefix(Kind::Contacts.root()).ok().and_then(|p| p.components().next()) {
                    changed_books.insert(account.as_os_str().to_string_lossy().to_string());
                }
            }
        }
        contact_refs.insert(contact.key.clone(), ContactRef { name: contact.name.clone(), uri: links::contact_uri(&uid) });
    }

    // Drafts, kept here only: nothing is sent.
    let mut draft_links: BTreeMap<String, String> = BTreeMap::new();
    for draft in &plan.drafts {
        let id = draft_id(&plan.prefix, &draft.key);
        let mut saved = Draft::by_id(&id).unwrap_or_else(|| Draft { id: id.clone(), ..Draft::default() });
        let before = saved.clone();
        saved.account = draft.account.clone();
        saved.to = draft.to.clone();
        saved.cc = draft.cc.clone();
        saved.subject = draft.subject.clone();
        saved.body = draft.body.clone();
        let tasks_of: Vec<String> = plan.tasks.iter().filter(|t| t.drafts.contains(&draft.key)).map(|t| links::task_uri(&uid_of(&plan.prefix, &t.key))).collect();
        for link in tasks_of {
            if !saved.links.contains(&link) {
                saved.links.push(link);
            }
        }
        if saved != before {
            println!("{} {}", if before.subject.is_empty() && before.body.is_empty() { "+" } else { "~" }, one_line(&draft.subject));
            if !dry_run {
                saved.save()?;
            }
        }
        draft_links.insert(draft.key.clone(), links::draft_uri(&id));
    }

    // Tasks: new ones made, known ones changed line by line; what you added since is kept.
    let mut lists_touched: BTreeSet<String> = BTreeSet::new();
    let mut written: BTreeMap<String, PathBuf> = BTreeMap::new();
    // The place a new step of each bigger task takes next: its steps in the file's order, after those it has.
    let mut next_place: BTreeMap<String, i64> = BTreeMap::new();
    for (position, task) in plan.tasks.iter().enumerate() {
        // New tasks are made a second apart, in the file's order: among equals, the plan keeps it.
        let made = now.checked_add(jiff::Span::new().seconds(position as i64)).unwrap_or_else(|_| now.clone());
        let uid = uid_of(&plan.prefix, &task.key);
        let found = existing.iter().find(|t| t.uid == uid);
        let mut edit = found.map(TaskEdit::of).unwrap_or_default();
        edit.title = task.title.clone();
        edit.notes = task.notes.clone();
        edit.start = task.start.clone();
        edit.due = task.due.clone();
        edit.estimate = task.estimate;
        edit.priority = task.priority;
        let merge = |into: &mut Vec<String>, more: Vec<String>| {
            for item in more {
                if !into.contains(&item) {
                    into.push(item);
                }
            }
        };
        merge(&mut edit.categories, task.tags.clone());
        merge(&mut edit.projects, task.projects.clone());
        merge(&mut edit.waits_for, task.after.iter().map(|k| uid_of(&plan.prefix, k)).collect());
        if !task.parent.is_empty() {
            edit.parent = uid_of(&plan.prefix, &task.parent);
        }
        let mut wanted_links: Vec<Link> = task.note.iter().map(|path| Link { uri: notes::uri_of(path.split('#').next().unwrap_or(path)), label: path.split_once('#').map(|(_, h)| h.to_string()).unwrap_or_default(), rel: "describedby".into() }).collect();
        wanted_links.extend(task.links.iter().cloned());
        wanted_links.extend(task.drafts.iter().filter_map(|k| draft_links.get(k)).map(|uri| Link { uri: uri.clone(), label: String::new(), rel: "related".into() }));
        for link in wanted_links {
            if !edit.links.iter().any(|l| l.uri == link.uri) {
                edit.links.push(link);
            }
        }
        for key in &task.contacts {
            let contact = contact_refs.get(key).cloned().ok_or_else(|| s.say("import-unknown-key", &[("key", key.clone()), ("task", task.key.clone())]))?;
            if !edit.contacts.contains(&contact) {
                edit.contacts.push(contact);
            }
        }
        if let Some(status) = task.status {
            edit.status = status;
        }
        let done_on: Option<Zoned> = (!task.done.is_empty()).then(|| task.done.parse::<jiff::civil::Date>().ok()).flatten().and_then(|d| d.at(12, 0, 0, 0).to_zoned(zone.clone()).ok());
        if done_on.is_some() {
            edit.status = Status::Completed;
        }
        let (path, old, new_text) = match found {
            Some(found) => {
                let current = std::fs::read_to_string(&found.key).map_err(|e| e.to_string())?;
                let text = tasks::apply(&current, &edit, &zone, &now)?;
                (PathBuf::from(&found.key), current, text)
            }
            None => {
                if !edit.parent.is_empty() {
                    let place = *next_place.entry(edit.parent.clone()).or_insert_with(|| tasks::next_in_sequence(tasks::steps_of(&existing, &edit.parent), &made));
                    edit.sort_order = Some(place);
                    next_place.insert(edit.parent.clone(), place.saturating_add(tasks::SEQUENCE_GAP));
                }
                (target.dir.join(format!("{uid}.ics")), String::new(), tasks::new_task(&edit, &uid, &zone, &made)?)
            }
        };
        let new_text = match &done_on {
            Some(at) => tasks::completed_at(&new_text, at)?,
            None => new_text,
        };
        if old != new_text {
            println!("{} {}", if old.is_empty() { "+" } else { "~" }, one_line(&task.title));
            if !dry_run {
                vdir::write_item(&path, &new_text)?;
                lists_touched.insert(found.map_or_else(|| format!("{}/{}", target.account, target.id), |f| f.list_id.clone()));
            }
        }
        written.insert(uid, path);
    }

    // Appointments: events; one imported before as a task becomes the event,
    // the task and its blocks taken out with it (`vdir::replace_items`).
    lists_touched.extend(import_events(s, &plan, &existing, dry_run, &zone)?);

    // Waiting with a gap: written in the one that comes first (RFC 9253 §4).
    for task in &plan.tasks {
        let successor = uid_of(&plan.prefix, &task.key);
        for (key, days) in &task.after_gap {
            let predecessor = uid_of(&plan.prefix, key);
            let Some(path) = written.get(&predecessor).filter(|p| p.exists()) else { continue };
            let current = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let text = tasks::set_gap(&current, &successor, i64::from(*days) * 1440, &now)?;
            if text != current && !dry_run {
                vdir::write_item(path, &text)?;
            }
        }
    }

    if dry_run || no_sync {
        return Ok(());
    }
    let accounts: BTreeSet<String> = lists_touched.iter().filter_map(|l| l.split('/').next().map(str::to_string)).chain(changed_books).collect();
    for account in accounts {
        if let Some(account) = s.config.account(&account).filter(|a| a.is_dav()) {
            crate::dav::sync_one(s, account)?;
        }
    }
    Ok(())
}

/// The file of the event with this UID, in any calendar.
fn event_file(uid: &str) -> Option<PathBuf> {
    let wanted = format!("UID:{uid}");
    vdir::collections(Kind::Calendars).into_iter().flat_map(|c| c.items()).find(|p| {
        std::fs::read_to_string(p).is_ok_and(|t| {
            let lines = sioul_core::lines::unfold(&t);
            lines.iter().any(|l| l == "BEGIN:VEVENT") && lines.iter().any(|l| l.trim_end() == wanted)
        })
    })
}

/// The time blocks of the task `uid` that can be changed (`blocks::TASK`).
fn blocks_of(uid: &str) -> Vec<PathBuf> {
    vdir::collections(Kind::Calendars)
        .into_iter()
        .filter(|c| !c.read_only)
        .flat_map(|c| c.items())
        // Never a block given a guest: a meeting with others now, it stays as it is (`agenda::may_change`).
        .filter(|p| std::fs::read_to_string(p).is_ok_and(|t| sioul_core::lines::unfold(&t).iter().any(|l| sioul_core::lines::name(l) == sioul_core::blocks::TASK && sioul_core::lines::value(l).trim() == uid) && !sioul_core::agenda::with_others(&t)))
        .collect()
}

/// The file's appointments written: new ones in their calendar, known ones
/// changed line by line, one imported before as a task made the event (the
/// task and its blocks taken out, never both, never neither). The calendars
/// touched, "account/id".
fn import_events(s: &Session, plan: &Plan_, existing: &[Task], dry_run: bool, zone: &TimeZone) -> Result<BTreeSet<String>, String> {
    let mut touched = BTreeSet::new();
    for event in &plan.events {
        let uid = uid_of(&plan.prefix, &event.key);
        let all_day = !event.start.contains('T');
        let bad_start = || s.say("import-event-start", &[("key", event.key.clone()), ("start", event.start.clone())]);
        let found = event_file(&uid);
        // An event imported before: what it holds is the start, the file's
        // lines over it, its margins, costs and reminder kept (the review of 22 October 2026, F4).
        let before = found.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| sioul_core::agenda::edit_of_text(&t, zone));
        let end = match (&event.end, &before) {
            (Some(end), _) if !end.is_empty() => end.clone(),
            // Unsaid on one known: its length kept, from its new start.
            (_, Some(b)) if !all_day && b.start.contains('T') && b.end.contains('T') => {
                let (start, was, was_end): (jiff::civil::DateTime, jiff::civil::DateTime, jiff::civil::DateTime) =
                    (event.start.parse().map_err(|_| bad_start())?, b.start.parse().map_err(|_| bad_start())?, b.end.parse().map_err(|_| bad_start())?);
                let length = was_end.since(was).map_err(|_| bad_start())?;
                start.checked_add(length).map_err(|_| bad_start())?.strftime("%Y-%m-%dT%H:%M").to_string()
            }
            _ if all_day => event.start.clone(),
            _ => {
                let start: jiff::civil::DateTime = event.start.parse().map_err(|_| bad_start())?;
                start.checked_add(jiff::Span::new().hours(1)).map_err(|_| bad_start())?.strftime("%Y-%m-%dT%H:%M").to_string()
            }
        };
        let mut edit = before.clone().unwrap_or_default();
        edit.title = event.title.clone();
        edit.start = event.start.clone();
        edit.end = end;
        edit.all_day = all_day;
        for (field, given) in [(&mut edit.location, &event.location), (&mut edit.notes, &event.notes), (&mut edit.repeat, &event.repeat)] {
            if let Some(given) = given {
                *field = given.clone();
            }
        }
        let wanted = event.calendar.clone().or_else(|| plan.calendar.clone()).unwrap_or_default();
        let calendar = || {
            vdir::collections(Kind::Calendars)
                .into_iter()
                .find(|c| !c.read_only && c.holds("VEVENT") && format!("{}/{}", c.account, c.id) == wanted)
                .or_else(sioul_core::agenda::default_calendar)
                .ok_or_else(|| s.tr.text("dav-no-calendar", None))
        };
        let as_task = existing.iter().find(|t| t.uid == uid);
        match (found, as_task) {
            (Some(path), _) => {
                let current = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let text = sioul_core::agenda::apply(&current, &edit, zone)?;
                let change = sioul_core::agenda::Change::Write(&text);
                if text != current {
                    // A calendar you only read, a meeting with others: left as it is, and said (`agenda::may_change`).
                    match sioul_core::agenda::guard(&path, change) {
                        Err(refusal) => println!("= {}: {}", one_line(&event.title), s.tr.text(refusal.word(&change), None)),
                        Ok(()) => {
                            println!("~ {}", one_line(&event.title));
                            if !dry_run {
                                vdir::write_item(&path, &text)?;
                            }
                        }
                    }
                }
                touched.extend(path.strip_prefix(Kind::Calendars.root()).ok().and_then(|p| p.components().next()).map(|a| format!("{}/", a.as_os_str().to_string_lossy())));
            }
            (None, Some(task)) => {
                if task.read_only {
                    return Err(s.tr.text("task-read-only", None));
                }
                let calendar = calendar()?;
                let text = sioul_core::agenda::new_event_with_uid(&edit, &uid, zone)?;
                println!("→ {}", one_line(&event.title));
                if !dry_run {
                    let gone: Vec<PathBuf> = std::iter::once(PathBuf::from(&task.key)).chain(blocks_of(&uid)).collect();
                    for path in &gone {
                        touched.extend(path.strip_prefix(Kind::Calendars.root()).ok().and_then(|p| p.components().next()).map(|a| format!("{}/", a.as_os_str().to_string_lossy())));
                    }
                    vdir::replace_items(&gone, &[], &calendar.dir.join(format!("{uid}.ics")), &text)?;
                }
                touched.insert(format!("{}/{}", calendar.account, calendar.id));
            }
            (None, None) => {
                let calendar = calendar()?;
                let text = sioul_core::agenda::new_event_with_uid(&edit, &uid, zone)?;
                println!("+ {}", one_line(&event.title));
                if !dry_run {
                    vdir::write_item(&calendar.dir.join(format!("{uid}.ics")), &text)?;
                }
                touched.insert(format!("{}/{}", calendar.account, calendar.id));
            }
        }
    }
    Ok(touched)
}

pub(crate) fn focus(s: &Session, command: FocusCommand) -> Result<(), String> {
    let now = Timestamp::now().as_second();
    match command {
        FocusCommand::Start { task, minutes } => {
            let desk = Desk::read(s);
            let task = desk.find(s, &task)?;
            if let Some(running) = timelog::running() {
                return Err(s.say("focus-already", &[("title", desk.find(s, &running.task).map(|t| t.title.clone()).unwrap_or(running.task))]));
            }
            timelog::keep_running(Some(&timelog::Running { task: task.uid.clone(), start: now, planned: minutes.max(1), ..timelog::Running::default() }))?;
            println!("{}", s.say("focus-started", &[("title", one_line(&task.title)), ("minutes", minutes.to_string())]));
            Ok(())
        }
        FocusCommand::Status => {
            match timelog::running() {
                Some(r) => {
                    let title = one_line(&Desk::read(s).find(s, &r.task).map(|t| t.title.clone()).unwrap_or(r.task.clone()));
                    println!("{}", s.say("focus-status", &[("task", title), ("elapsed", (r.elapsed(now) / 60).to_string()), ("left", (r.remaining(now).max(0) / 60).to_string())]));
                }
                None => println!("{}", s.tr.text("focus-none", None)),
            }
            Ok(())
        }
        FocusCommand::Stop { done, note } => {
            let Some(running) = timelog::running() else { return Err(s.tr.text("focus-none", None)) };
            let session = timelog::finish_with_note(now, done, &note.unwrap_or_default())?.ok_or_else(|| s.tr.text("focus-none", None))?;
            let mut args = s.tr.counted(session.minutes as usize);
            args.set("minutes", session.minutes.to_string());
            println!("{}", s.tr.text("focus-stopped", Some(&args)));
            if done {
                set_status(s, &running.task, Status::Completed)?;
            }
            Ok(())
        }
    }
}

/// What a thing is tied to, both ways.
pub(crate) fn links_command(s: &Session, uri: &str) -> Result<(), String> {
    let loaded = Loaded::read(&s.config);
    let world = loaded.world();
    let me = world.describe(uri);
    println!("{}  <{}>", one_line(&me.title), one_line(&me.uri));
    for r in taskview::related_views(&s.tr, &world.related(uri)) {
        let when = if r.when.is_empty() { String::new() } else { format!(" · {}", one_line(&r.when)) };
        println!("  {}: {}{when}  <{}>", one_line(&r.how), one_line(&r.title), one_line(&r.uri));
    }
    Ok(())
}

/// Notes of the notes folder: those matching, or one in full with its links back.
pub(crate) fn notes_command(s: &Session, query: &str, path: Option<&str>) -> Result<(), String> {
    let root = s.config.notes_root_path().ok_or_else(|| s.tr.text("error-no-store", None))?;
    let vault = notes::Vault::open(&root);
    if let Some(path) = path {
        let note = vault.note(path).ok_or_else(|| s.say("note-not-found", &[("path", path.to_string())]))?;
        println!("{}  <{}>", one_line(&note.title), one_line(&note.uri()));
        if !note.tags.is_empty() {
            println!("  #{}", one_line(&note.tags.join(" #")));
        }
        let loaded = Loaded { vault: Some(vault.clone()), ..Loaded::read(&s.config) };
        for r in taskview::related_views(&s.tr, &loaded.world().related(&note.uri())) {
            println!("  {}: {}  <{}>", one_line(&r.how), one_line(&r.title), one_line(&r.uri));
        }
        for checkbox in note.checkboxes.iter().filter(|c| !c.done) {
            println!("  ○ {} (l. {})", one_line(&checkbox.text), checkbox.line);
        }
        return Ok(());
    }
    let found = if query.is_empty() { vault.notes.iter().collect() } else { vault.search(query) };
    for note in found {
        println!("{}  ({})", one_line(&note.title), one_line(&note.path));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Steps imported keep the file's order, whatever their titles; `sioul
    /// tasks move` puts one before another; what is no step, or a step of
    /// another task, is said.
    #[test]
    fn steps_imported_in_order_and_moved() {
        // The tests' own home, whichever test comes first (review of 36002ea's check, A2), then the guard.
        crate::mcp::tests::home();
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let s = Session { config: sioul_core::config::Config::default(), config_path: PathBuf::new(), tr: sioul_core::i18n::Translator::new("en") };
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "Steps plan", None, &["VTODO"]).unwrap();
        let file = sioul_core::config::state_dir().join("steps-import.toml");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let body = "[[task]]\nkey = \"doctor\"\ntitle = \"Going to the doctor\"\n\n[[task]]\nkey = \"teeth\"\ntitle = \"Brush teeth\"\nparent = \"doctor\"\n\n[[task]]\nkey = \"bag\"\ntitle = \"Zip the bag\"\nparent = \"doctor\"\n\n[[task]]\nkey = \"bus\"\ntitle = \"Check the bus\"\nparent = \"doctor\"\n\n[[task]]\nkey = \"other\"\ntitle = \"Return the books\"\n\n[[task]]\nkey = \"books\"\ntitle = \"Gather the books\"\nparent = \"other\"\n";
        std::fs::write(&file, format!("list = \"local/{}\"\nprefix = \"steps-\"\n\n{body}", list.id)).unwrap();
        import(&s, &file, true, false).unwrap();
        let order = || tasks::steps_of(&tasks::all(&TimeZone::system()), "steps-doctor").iter().map(|t| t.title.clone()).collect::<Vec<_>>();
        assert_eq!(order(), ["Brush teeth", "Zip the bag", "Check the bus"]);
        move_step(&s, "steps-bus", Some("steps-teeth"), true).unwrap();
        assert_eq!(order(), ["Check the bus", "Brush teeth", "Zip the bag"]);
        move_step(&s, "steps-bus", None, true).unwrap();
        assert_eq!(order(), ["Brush teeth", "Zip the bag", "Check the bus"]);
        // Imported again: what the window moved stays where it is.
        move_step(&s, "steps-teeth", None, true).unwrap();
        import(&s, &file, true, false).unwrap();
        assert_eq!(order(), ["Zip the bag", "Check the bus", "Brush teeth"]);
        let no_step = move_step(&s, "steps-doctor", None, true).unwrap_err();
        assert!(no_step.contains("not a step"), "{no_step}");
        let elsewhere = move_step(&s, "steps-bag", Some("steps-books"), true).unwrap_err();
        assert!(elsewhere.contains("Going to the doctor"), "{elsewhere}");
        // `sioul tasks parent`: a task of its own, a step of another, never of its own step.
        let parent_of = |uid: &str| tasks::all(&TimeZone::system()).into_iter().find(|t| t.uid == uid).and_then(|t| t.parent().map(str::to_string));
        parent(&s, "steps-books", "none", false, true).unwrap();
        assert_eq!(parent_of("steps-books"), None);
        parent(&s, "steps-books", "steps-doctor", false, true).unwrap();
        assert_eq!(parent_of("steps-books").as_deref(), Some("steps-doctor"));
        assert!(parent(&s, "steps-doctor", "steps-books", false, true).unwrap_err().contains("its own steps"));
    }

    #[test]
    fn a_plan_file_names_projects_either_way() {
        // `projects`, and `cases` as files written before the one name say it.
        let plan: Plan_ = toml::from_str("[[task]]\nkey = \"a\"\ntitle = \"A\"\nprojects = [\"taxes\"]\n\n[[task]]\nkey = \"b\"\ntitle = \"B\"\ncases = [\"housing\"]\n").unwrap();
        assert_eq!(plan.tasks.iter().map(|t| t.projects.clone()).collect::<Vec<_>>(), [vec!["taxes".to_string()], vec!["housing".to_string()]]);
    }

    /// An appointment had been imported as a task.
    /// A `[[task]]` with what only an appointment has is asked about, never
    /// guessed; the same key under `[[event]]` makes the event, the task and
    /// its block taken out, once; imported again, nothing changes.
    #[test]
    fn an_appointment_in_a_plan_file_is_an_event_said_so() {
        // The tests' own home, whichever test comes first (review of 36002ea's check, A2), then the guard.
        crate::mcp::tests::home();
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let s = Session { config: sioul_core::config::Config::default(), config_path: PathBuf::new(), tr: sioul_core::i18n::Translator::new("en") };
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "Kinds plan", None, &["VTODO"]).unwrap();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Kinds personal", None, &["VEVENT"]).unwrap();
        let head = format!("list = \"local/{}\"\ncalendar = \"local/{}\"\nprefix = \"kinds-\"\n\n", list.id, calendar.id);
        let file = sioul_core::config::state_dir().join("kinds-import.toml");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let run = |body: &str| {
            std::fs::write(&file, format!("{head}{body}")).unwrap();
            import(&s, &file, true, false)
        };
        let files = |c: &Collection| c.items();

        // An end is an appointment's: asked, nothing written.
        let asked = run("[[task]]\nkey = \"dentist\"\ntitle = \"Dentist\"\nstart = \"2099-10-23T14:00\"\nend = \"2099-10-23T15:00\"\n").unwrap_err();
        assert!(asked.contains("dentist") && asked.contains("[[event]]"), "{asked}");
        let kind = run("[[task]]\nkey = \"dentist\"\ntitle = \"Dentist\"\nkind = \"appointment\"\n").unwrap_err();
        assert!(kind.contains("[[event]]"), "{kind}");
        assert!(files(&list).is_empty() && files(&calendar).is_empty());

        // As it was imported before: a task with a time, and its block.
        run("[[task]]\nkey = \"dentist\"\ntitle = \"Dentist\"\nstart = \"2099-10-23T14:00\"\n").unwrap();
        assert_eq!(files(&list).len(), 1);
        std::fs::write(calendar.dir.join("block.ics"), "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:block\r\nDTSTART:20991023T120000Z\r\nDTEND:20991023T130000Z\r\nSUMMARY:Dentist\r\nX-SIOUL-TASK:kinds-dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let both = run("[[task]]\nkey = \"dentist\"\ntitle = \"Dentist\"\n\n[[event]]\nkey = \"dentist\"\ntitle = \"Dentist\"\nstart = \"2099-10-23T14:00\"\n").unwrap_err();
        assert!(both.contains("dentist"), "{both}");

        // Said an appointment: the event, the task and its block gone.
        let appointment = "[[event]]\nkey = \"dentist\"\ntitle = \"Dentist\"\nstart = \"2099-10-23T14:00\"\nlocation = \"1 Example Street\"\n";
        run(appointment).unwrap();
        assert!(files(&list).is_empty(), "the task gone");
        let events = files(&calendar);
        assert_eq!(events.len(), 1, "one event, the block gone: {events:?}");
        let text = std::fs::read_to_string(&events[0]).unwrap();
        assert!(text.contains("UID:kinds-dentist") && text.contains("BEGIN:VEVENT") && text.contains("LOCATION:1 Example Street") && !text.contains("X-SIOUL-TASK"), "{text}");
        let edit = sioul_core::agenda::edit_of(&events[0]).unwrap();
        assert_eq!((edit.start.as_str(), edit.end.as_str()), ("2099-10-23T14:00", "2099-10-23T15:00"), "an hour without an end");

        // Imported again: the same event, unchanged.
        run(appointment).unwrap();
        assert_eq!(files(&calendar), events);
        assert_eq!(std::fs::read_to_string(&events[0]).unwrap(), text);
        // Its margins, costs and reminder, set in the window, stay through a re-import that moves it (F4).
        let kept = text.replace("END:VEVENT", "X-SIOUL-BEFORE:20\r\nX-SIOUL-AFTER:15\r\nX-SIOUL-COST:ANXIETY=7\r\nX-SIOUL-REMIND:30\r\nEND:VEVENT");
        std::fs::write(&events[0], &kept).unwrap();
        run(&appointment.replace("2099-10-23T14:00", "2099-10-23T16:00")).unwrap();
        let moved = std::fs::read_to_string(&events[0]).unwrap();
        for line in ["X-SIOUL-BEFORE:20", "X-SIOUL-AFTER:15", "ANXIETY=7", "X-SIOUL-REMIND:30", "LOCATION:1 Example Street"] {
            assert!(moved.contains(line), "{line} kept: {moved}");
        }
        let edit = sioul_core::agenda::edit_of(&events[0]).unwrap();
        assert_eq!((edit.start.as_str(), edit.end.as_str()), ("2099-10-23T16:00", "2099-10-23T17:00"), "moved, its length kept");
        let _ = std::fs::remove_dir_all(&list.dir);
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_file(&file);
    }
}

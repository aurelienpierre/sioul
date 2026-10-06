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
        /// "case" (default) or "list".
        #[arg(long, default_value = "case")]
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
        #[arg(long)]
        case: Option<String>,
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
pub(crate) fn settings(s: &Session, weather: Weather, situation: &sioul_core::quiet::Situation, cases: &[sioul_core::cases::Case], tasks: &[Task]) -> Settings {
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
    let needs = sioul_core::health::Health::load(&sioul_core::health::Health::default_path()).needs;
    let days = sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path());
    // Meals move past the blocks as past any event.
    let pushed = needs.past_events_on(now.date(), now.time_zone(), &days, &sioul_core::plan::event_spans(&read, 0));
    // Today's end of work moved by free time, as the window plans (docs/pauses.md).
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let day = sioul_core::pause::Day::of(&s.config, sioul_core::quiet::Blocks::read(&now, &read), &events);
    let extension = sioul_core::pause::moved_today(&overrides, &day.evening(), &now, s.config.free_time.moves);
    let mut settings = Settings::of_hours(&s.config.week_hours(), sioul_core::areas::TaskAreas::of_config(&s.config, cases)).with_needs(&needs, pushed).with_days(days).with_extension(extension).with_pins(pins).with_events(&now, &events);
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
        let situation = sioul_core::quiet::Situation::now(&s.config, &overrides, &sioul_core::quiet::Blocks::read_now(&now), &now, &s.tr, &loaded.cases);
        let mut settings = settings(s, today.weather, &situation, &loaded.cases, &loaded.tasks);
        // As the window plans: what your record says of a day (docs/capacity.md), a block counted as its task.
        let own = |from: i64, to: i64| sioul_core::blocks::without_blocks(sioul_core::agenda::occurrences(from, to), &loaded.tasks);
        settings.capacity = sioul_core::capacity::gather(&loaded.tasks, &sessions, &settings, &s.config.planning, &own, &now).planning;
        let plan = plan::plan(&loaded.tasks, date, &settings, &spent, &today.aside);
        let filter = Filter { quiet: situation.quiet_tasks(), ..Filter::default() };
        Desk { loaded, filter, offices: situation.offices, plan, today, spent, stopped, sessions, settings }
    }

    fn context<'a>(&'a self, s: &'a Session) -> Context<'a> {
        Context { filter: &self.filter, offices: self.offices.clone(), tasks: &self.loaded.tasks, plan: &self.plan, today: Zoned::now().date(), tr: &s.tr, cases: &self.loaded.cases, spent: &self.spent, stopped: &self.stopped }
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
        TasksCommand::Timeline { case } => timeline(s, case.as_deref()),
        TasksCommand::Show { task } => show(s, &task),
        TasksCommand::Add { line, list, parent, after, no_sync } => add(s, &line.join(" "), list.as_deref(), parent.as_deref(), &after, no_sync),
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

fn timeline(s: &Session, case: Option<&str>) -> Result<(), String> {
    let desk = Desk::read(s);
    // The days of rest of the room the plan was made with, as in the window.
    let rest = desk.settings.rest_days();
    let view = taskview::timeline(&desk.context(s), case, &rest);
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
    let cases: Vec<String> = desk.loaded.cases.iter().map(|c| c.id.clone()).collect();
    let captured = capture::capture(line, Zoned::now().date(), &cases, &s.config.task_kinds(&s.tr));
    let mut edit = captured.edit;
    if edit.title.is_empty() {
        return Err(s.tr.text("task-no-title", None));
    }
    if let Some(parent) = parent {
        edit.parent = desk.find(s, parent)?.uid.clone();
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

fn set_status(s: &Session, wanted: &str, status: Status) -> Result<(), String> {
    let desk = Desk::read(s);
    let task = desk.find(s, wanted)?;
    if task.read_only {
        return Err(s.tr.text("task-read-only", None));
    }
    let current = std::fs::read_to_string(&task.key).map_err(|e| format!("{}: {e}", task.key))?;
    let text = tasks::set_status(&current, status, &TimeZone::system(), &Zoned::now())?;
    vdir::write_item(Path::new(&task.key), &text)?;
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
}

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
    #[serde(default)]
    cases: Vec<String>,
    /// The key of the bigger task, or "uid:<UID>".
    #[serde(default)]
    parent: String,
    /// Keys (or "uid:<UID>") it waits for.
    #[serde(default)]
    after: Vec<String>,
    /// Keys it waits for, with the days to wait after each is done.
    #[serde(default)]
    after_gap: BTreeMap<String, u32>,
    /// Notes in the case store, by path ("admin/letters.md").
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
    for key in plan.tasks.iter().map(|t| &t.key).chain(plan.contacts.iter().map(|c| &c.key)) {
        let uid = uid_of(&plan.prefix, key);
        if uid.is_empty() || uid.contains(['/', '\\']) || (cfg!(windows) && uid.contains(':')) {
            return Err(s.say("import-bad-key", &[("key", key.clone())]));
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
        merge(&mut edit.cases, task.cases.clone());
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
            None => (target.dir.join(format!("{uid}.ics")), String::new(), tasks::new_task(&edit, &uid, &zone, &made)?),
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

    // Waiting with a gap: written in the one that comes first (RFC 9253 §4).
    for task in &plan.tasks {
        let successor = uid_of(&plan.prefix, &task.key);
        for (key, days) in &task.after_gap {
            let predecessor = uid_of(&plan.prefix, key);
            let Some(path) = written.get(&predecessor).filter(|p| p.exists()) else { continue };
            let current = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let line = tasks::relation_line("FINISHTOSTART", &successor, i64::from(*days) * 1440);
            let without = tasks::remove_lines(&current, |l| sioul_core::lines::name(l) == "RELATED-TO" && tasks::relation_of(l).kind == "FINISHTOSTART" && tasks::relation_of(l).uid == successor && *l != line, &now)?;
            let text = tasks::add_lines(&without, std::slice::from_ref(&line), &now)?;
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

/// Notes of the case store: those matching, or one in full with its links back.
pub(crate) fn notes_command(s: &Session, query: &str, path: Option<&str>) -> Result<(), String> {
    let root = s.config.case_store_path().ok_or_else(|| s.tr.text("error-no-store", None))?;
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

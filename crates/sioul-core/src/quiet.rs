// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The time now, and what comes then (docs/areas.md). Five times: work and
//! admin, the hours you set; meals and sleep, from Health; and leisure,
//! every other time: evenings, days without hours, time off, a day closed
//! early. Sleep first, then meals, then the hours. Two overrides, each until
//! a time, kept in `$XDG_STATE_HOME/sioul/quiet.toml`: working late keeps
//! work in view; done for the day brings leisure early, until work comes
//! back. Outside work, work rests (quiet time); during sleep nothing
//! disturbs: no notification but the doses you asked for (`may_notify`).
//! Two pauses come above them (docs/pauses.md, `pause`): the pause holds
//! everything, whatever the time; Free time is leisure whatever the hour,
//! sleep first, and may move the end of today's work later.
//!
//! Who may reach you when is a matrix per channel (`reach::Reach`): for each
//! state, the times they come; mail's rule is `mail_in_view`.
//!
//! Detachment from work in the evening is what recovery needs most
//! (Sonnentag & Fritz 2007, 2015); work cues in off-hours keep it from
//! happening. Codes from verified senders still come at once: they are asked for.

use crate::areas::{Area, TaskAreas, Time, Week, in_view};
use crate::config::TimeOff;
use crate::reach::{Matrix, Who};
use crate::window::{self, AdminWindow};
use jiff::civil::Date;
use jiff::{Span, Zoned};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Why the time is what it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// Within working hours.
    Working,
    /// No working or admin hours are set: everything comes.
    NoHours,
    /// Working past the usual hours, by choice.
    WorkingLate,
    /// Work shown now, by choice, until you say otherwise or the next working day is over.
    WorkNow,
    /// After or before the day's hours.
    Evening,
    /// A day without working hours.
    DayOff,
    /// Holidays, sick leave.
    TimeOff,
    /// Stopped early, by choice.
    DoneForTheDay,
    /// Hours set for your own admin.
    AdminTime,
    /// A meal, from getting it ready to its end.
    Meal,
    /// The night, from bedtime to waking.
    Sleep,
    /// The night's first hour, winding down before bed: sleep's already.
    WindingDown,
    /// A nap, and the minutes to come back after it.
    Nap,
    /// Free time ("Temps libre"): leisure whatever the hour, by choice (docs/pauses.md).
    FreeTime,
    /// The pause ("En pause"): everything Sioul shows held, whatever the time (docs/pauses.md).
    Paused,
    /// After the usual end, while today's end of work moved by free time (docs/pauses.md).
    Extended,
}

/// What now is for, and until when.
#[derive(Debug, Clone, PartialEq)]
pub struct Mode {
    /// Work rests: these are not work's hours (leisure, meals, sleep, admin).
    pub quiet: bool,
    /// What now is for (docs/areas.md).
    pub time: Time,
    /// Which times your week holds.
    pub week: Week,
    pub reason: Reason,
    /// When this time changes: its hours end, a meal or the night begins, you wake.
    pub until: Option<Zoned>,
    /// When work comes back, for what waits for it (a thought noted now): as
    /// the hours stand, meals and sleep aside.
    pub back: Option<Zoned>,
    /// The time off's word, when it is time off.
    pub label: String,
}

impl Mode {
    /// Asleep: the night from winding down to waking, or a nap. Nothing
    /// disturbs (`may_notify`); tasks, projects and time wait behind a sentence.
    pub fn sleeps(&self) -> bool {
        self.time == Time::Sleep
    }

    /// The pause: everything Sioul shows held (docs/pauses.md).
    pub fn paused(&self) -> bool {
        self.reason == Reason::Paused
    }

    /// Free time: leisure whatever the hour (docs/pauses.md).
    pub fn free(&self) -> bool {
        self.reason == Reason::FreeTime
    }
}

/// The overrides, each until a time (Unix seconds).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Overrides {
    /// Work stays in view until then.
    #[serde(default)]
    pub work_until: Option<i64>,
    /// Quiet until then: done for the day.
    #[serde(default)]
    pub rest_until: Option<i64>,
    /// "Work now", ticked: work shown until then, the end of the next working
    /// day. Sioul takes it back when it closes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_now: Option<i64>,
    /// When the day was closed: "back to today's plan" stays offered that day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_from: Option<i64>,
    /// The plan's first step when work comes back, in words, and its task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_step_task: Option<String>,
    /// The day it is for; it is not shown after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_step_day: Option<Date>,
    /// Free time ("Temps libre"), pressed then; on while later than
    /// `free_ended` (docs/pauses.md). Stamps, never a key taken away: the
    /// sharing merges key by key, and a pause is never half on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_since: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_ended: Option<i64>,
    /// "Nothing at all" for this free time; unsaid, as set up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_nothing: Option<bool>,
    /// The day free time took working time on, and how much, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_day: Option<Date>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_lost: Option<u32>,
    /// Where the end of work moved to on `free_day` (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended_until: Option<i64>,
    /// "Keep my usual end": the day nothing moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_end_on: Option<Date>,
    /// The pause ("En pause"), pressed then; on while later than `paused_ended`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_since: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_ended: Option<i64>,
    /// Days held lighter after a pause: the bad-day level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lighter: Vec<Date>,
    /// After a pause, the Porch rests until then, unless opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porch_rests_until: Option<i64>,
}

impl Overrides {
    /// The first step to show now: on the day it is for, in work time.
    pub fn first_step_on(&self, today: Date) -> Option<(&str, &str)> {
        let step = self.first_step.as_deref().filter(|s| !s.trim().is_empty())?;
        self.first_step_day.is_none_or(|d| d == today).then(|| (step, self.first_step_task.as_deref().unwrap_or("")))
    }

    /// Whether the day was closed today, so today's plan can be taken back.
    pub fn closed_today(&self, now: &Zoned) -> bool {
        self.rest_from.and_then(|t| jiff::Timestamp::from_second(t).ok()).is_some_and(|t| t.to_zoned(now.time_zone().clone()).date() == now.date())
    }
}

impl Overrides {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("quiet.toml")
    }

    pub fn load(path: &Path) -> Overrides {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        std::fs::write(path, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)
    }
}

/// The time off that covers `date`, if any.
pub fn time_off_on(time_off: &[TimeOff], date: Date) -> Option<&TimeOff> {
    time_off.iter().find(|t| t.from <= date && date <= t.until)
}

/// When working hours next open after `now`, past any time off; None without hours.
pub fn next_work(windows: &[AdminWindow], time_off: &[TimeOff], now: &Zoned) -> Option<Zoned> {
    let mut from = now.clone();
    for _ in 0..60 {
        let opening = window::next_opening(windows, &from)?;
        match time_off_on(time_off, opening.date()) {
            None => return Some(opening),
            // Within time off: from the day after it ends.
            Some(off) => {
                let after = off.until.checked_add(Span::new().days(1)).ok()?;
                from = after.to_zoned(now.time_zone().clone()).ok()?.checked_sub(Span::new().seconds(1)).ok()?;
            }
        }
    }
    None
}

/// When work comes back once you are done for today: the next working hours
/// after today, past any time off; without hours, tomorrow at `morning`.
pub fn after_today(windows: &[AdminWindow], time_off: &[TimeOff], now: &Zoned, morning: i8) -> Zoned {
    let Some(tomorrow) = now.date().tomorrow().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()) else {
        return now.clone();
    };
    // `morning` comes from the settings: an hour that is none is midnight.
    let at_morning = jiff::civil::Time::new(morning, 0, 0, 0).ok().and_then(|t| tomorrow.date().to_datetime(t).to_zoned(now.time_zone().clone()).ok());
    next_work(windows, time_off, &tomorrow).unwrap_or_else(|| at_morning.unwrap_or(tomorrow))
}

/// When "Work now" ends: once the next working day is over, today's while
/// its working hours are not; past any time off. Without working hours, at
/// midnight. `work` are the working hours only.
pub fn end_of_next_workday(work: &[AdminWindow], time_off: &[TimeOff], now: &Zoned) -> Zoned {
    let zone = now.time_zone();
    if time_off_on(time_off, now.date()).is_none()
        && let Some((_, closing)) = window::hours_on(work, now.date(), zone).filter(|(_, closing)| closing.timestamp() > now.timestamp())
    {
        return closing;
    }
    if let Some(opening) = next_work(work, time_off, now)
        && let Some((_, closing)) = window::hours_on(work, opening.date(), zone)
    {
        return closing;
    }
    now.date().tomorrow().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).unwrap_or_else(|| now.clone())
}

/// "17:00", "tomorrow at 9:00", "Monday 5 October at 09:00".
pub fn until_text(tr: &crate::i18n::Translator, until: &Zoned, now: &Zoned) -> String {
    let mut args = crate::i18n::args();
    args.set("time", until.strftime("%H:%M").to_string());
    if until.date() == now.date() {
        until.strftime("%H:%M").to_string()
    } else if now.date().tomorrow().is_ok_and(|d| d == until.date()) {
        tr.text("until-tomorrow", Some(&args))
    } else {
        tr.date(until, false)
    }
}

/// What Health keeps free around now (docs/health.md, "Meals, rest and
/// sleep"): the meals, naps and nights of today and tomorrow, each day's
/// changes applied, and which of them are set at all.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Blocks {
    /// In order: a meal from getting it ready to its end, a nap with its
    /// minutes after, a night from winding down to waking.
    pub kept: Vec<crate::needs::Kept>,
    /// Some times are meals.
    pub meals: bool,
    /// Some times are sleep: the night or naps.
    pub sleep: bool,
    /// The night is set: without it, nights are leisure, and nothing keeps
    /// notifications away while you sleep.
    pub night: bool,
}

impl Blocks {
    /// From Health's needs and each day's changes (`days`), the meals pushed
    /// past `held` (events with their margins, `plan::event_spans`).
    pub fn of(needs: &crate::needs::Needs, days: &crate::needs::Days, held: &[(i64, i64)], now: &Zoned) -> Blocks {
        let kept = needs.kept_around(now, days, held);
        let meals = (needs.meals_on && needs.meals.iter().any(|m| m.on)) || kept.iter().any(|k| k.kind == "meal");
        let naps = (needs.naps_on && needs.naps.iter().any(|n| n.on)) || kept.iter().any(|k| k.kind == "nap");
        Blocks { kept, meals, sleep: needs.sleep_on || naps, night: needs.sleep_on }
    }

    /// Read from Health's files, the meals pushed past `events`.
    pub fn read(now: &Zoned, events: &[crate::agenda::Occurrence]) -> Blocks {
        let needs = crate::health::Health::load(&crate::health::Health::default_path()).needs;
        let days = crate::needs::Days::load(&crate::needs::Days::default_path());
        Blocks::of(&needs, &days, &crate::plan::event_spans(events, 0), now)
    }

    /// The same, today's events read first.
    pub fn read_now(now: &Zoned) -> Blocks {
        let midnight = now.date().to_zoned(now.time_zone().clone()).map_or_else(|_| now.timestamp().as_second(), |z| z.timestamp().as_second());
        Blocks::read(now, &crate::agenda::occurrences(midnight - 86_400, midnight + 2 * 86_400))
    }

    /// The block of one of `kinds` going on at `stamp` (Unix seconds).
    pub fn at(&self, stamp: i64, kinds: &[&str]) -> Option<&crate::needs::Kept> {
        self.kept.iter().find(|k| kinds.contains(&k.kind) && k.start <= stamp && stamp < k.end)
    }

    /// When the next block begins after `stamp`.
    fn next_start(&self, stamp: i64) -> Option<i64> {
        self.kept.iter().map(|k| k.start).filter(|start| *start > stamp).min()
    }
}

/// What the hours alone say, meals and sleep aside.
struct ByHours {
    time: Time,
    reason: Reason,
    until: Option<Zoned>,
    label: String,
}

/// The time by the hours, the overrides and time off: the hours of work and
/// admin, leisure outside them. Meals and sleep come after (`mode`).
fn by_hours(windows: &[AdminWindow], time_off: &[TimeOff], overrides: &Overrides, now: &Zoned) -> ByHours {
    let of = |kind: &str| -> Vec<AdminWindow> { windows.iter().filter(|w| w.kind() == kind).cloned().collect() };
    let (work, admin) = (of("work"), of("admin"));
    let at = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(now.time_zone().clone()));
    let made = |time: Time, reason: Reason, until: Option<Zoned>| ByHours { time, reason, until, label: String::new() };
    let stamp = now.timestamp().as_second();
    if let Some(rest) = overrides.rest_until.filter(|r| *r > stamp) {
        return made(Time::Leisure, Reason::DoneForTheDay, at(rest));
    }
    if let Some(late) = overrides.work_until.filter(|w| *w > stamp) {
        return made(Time::Work, Reason::WorkingLate, at(late));
    }
    if let Some(until) = overrides.work_now.filter(|w| *w > stamp) {
        return made(Time::Work, Reason::WorkNow, at(until));
    }
    if windows.is_empty() && time_off.is_empty() {
        return made(Time::Any, Reason::NoHours, None);
    }
    if let Some(off) = time_off_on(time_off, now.date()) {
        return ByHours { time: Time::Leisure, reason: Reason::TimeOff, until: next_work(windows, time_off, now), label: off.label.clone() };
    }
    if windows.is_empty() {
        return made(Time::Any, Reason::NoHours, None);
    }
    // Working and admin hours open now; both at once bring all they bring, until the first of them closes.
    let (at_work, at_admin) = (window::current(&work, now), window::current(&admin, now));
    let closing = [&at_work, &at_admin].into_iter().flatten().map(|(_, c)| c.clone()).min_by_key(Zoned::timestamp);
    match Time::of(Area { work: at_work.is_some(), admin: at_admin.is_some(), leisure: false }) {
        Time::Admin => made(Time::Admin, Reason::AdminTime, closing),
        Time::Leisure => {
            // Today's end of work moved by free time (docs/pauses.md): work from
            // the usual end until then; Sioul ends it.
            if let Some(end) = overrides.moved_end(now.date()).filter(|end| *end > stamp)
                && window::hours_on(&work, now.date(), now.time_zone()).is_some_and(|(_, closing)| closing.timestamp().as_second() <= stamp)
            {
                return made(Time::Work, Reason::Extended, at(end));
            }
            let reason = if window::open_day(&work, now.date()) { Reason::Evening } else { Reason::DayOff };
            // Until the next hours of either kind.
            made(Time::Leisure, reason, next_work(windows, time_off, now))
        }
        // Work, or work and admin at once.
        open => made(open, Reason::Working, closing),
    }
}

/// The earlier of two times.
fn earliest(a: Option<Zoned>, b: Option<Zoned>) -> Option<Zoned> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if b.timestamp() < a.timestamp() { b } else { a }),
        (a, b) => a.or(b),
    }
}

/// What now is for, and until when (docs/areas.md): sleep first (the night
/// from winding down to waking, a nap), then meals, then the hours:
/// "Done for today" is leisure, "A little longer" and "Work now" are work,
/// time off is leisure, then working and admin hours; every other time is
/// leisure. No working or admin hours at all: everything comes (`Time::Any`),
/// meals and sleep aside. `windows` are the week's hours of work and admin
/// (`Config::week_hours`): an older Sioul's free time is left aside.
pub fn mode(windows: &[AdminWindow], time_off: &[TimeOff], overrides: &Overrides, blocks: &Blocks, now: &Zoned) -> Mode {
    let windows: Vec<AdminWindow> = windows.iter().filter(|w| w.kind() != "leisure").cloned().collect();
    let week = Week { work_hours: windows.iter().any(|w| w.kind() == "work"), admin_hours: windows.iter().any(|w| w.kind() == "admin"), meals: blocks.meals, sleep: blocks.sleep };
    let at = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(now.time_zone().clone()));
    let stamp = now.timestamp().as_second();
    let hours = by_hours(&windows, time_off, overrides, now);
    // The pause holds everything Sioul shows, whatever the time (docs/pauses.md).
    if overrides.paused_from().is_some() {
        return Mode { quiet: true, time: Time::Sleep, week, reason: Reason::Paused, until: None, back: hours.until.clone(), label: String::new() };
    }
    // Work comes back when the block ends if these are work's hours; else as the hours say.
    let back_after = |end: i64| if hours.time.works() { at(end) } else { hours.until.clone() };
    // Health's times hold whatever the hours and the overrides say: "Work now" may span a night.
    if let Some(block) = blocks.at(stamp, &["sleep", "nap"]) {
        let reason = if block.kind == "nap" {
            Reason::Nap
        } else if stamp < block.at {
            Reason::WindingDown
        } else {
            Reason::Sleep
        };
        return Mode { quiet: true, time: Time::Sleep, week, reason, until: at(block.end), back: back_after(block.end), label: String::new() };
    }
    // Free time: leisure whatever the hour, sleep first; the night's start ends it (docs/pauses.md).
    if let Some(since) = overrides.free_from().filter(|since| *since <= stamp) {
        let end = crate::pause::free_until(since, blocks, now.time_zone());
        if stamp < end {
            return Mode { quiet: true, time: Time::Leisure, week, reason: Reason::FreeTime, until: at(end), back: None, label: String::new() };
        }
    }
    if let Some(block) = blocks.at(stamp, &["meal"]) {
        return Mode { quiet: true, time: Time::Meals, week, reason: Reason::Meal, until: at(block.end), back: back_after(block.end), label: String::new() };
    }
    // The hours' own end, or the next meal or night when it comes first; an
    // override and time off say their own time.
    let cut = matches!(hours.reason, Reason::Working | Reason::AdminTime | Reason::Evening | Reason::DayOff | Reason::NoHours | Reason::Extended);
    let until = if cut { earliest(hours.until.clone(), blocks.next_start(stamp).and_then(at)) } else { hours.until.clone() };
    Mode { quiet: !(hours.time.works() || hours.time == Time::Any), time: hours.time, week, reason: hours.reason, until, back: hours.until, label: hours.label }
}

/// What the task pages need of the moment: whether work rests, whether
/// offices are open, the days without room for tasks.
pub struct Situation {
    pub mode: Mode,
    pub offices: crate::taskview::Offices,
    /// Time off, and today when you are done for the day.
    pub closed: std::collections::BTreeSet<Date>,
    /// The weekdays offices open, Monday first.
    pub office_days: [bool; 7],
    /// What quiet time keeps of the tasks.
    pub quiet_tasks: QuietTasks,
}

/// What the task pages keep now (docs/areas.md): what the time is for, by
/// each task's area (its own, else its categories and projects). A call to an
/// office fits work and admin hours, never leisure: offices keep business
/// hours. In leisure (holidays, a day closed, evenings) and during a meal:
/// only what is yours to enjoy, and what is yours either way (health).
/// During sleep: no task at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuietTasks {
    pub time: Time,
    pub week: Week,
    pub areas: TaskAreas,
    /// In free time with movement off (energy-limiting illness): movement and
    /// exercise are not offered (docs/pauses.md, GP8).
    pub no_movement: bool,
}

impl QuietTasks {
    /// Whether a task stays in view now.
    pub fn keeps(&self, task: &crate::tasks::Task) -> bool {
        if self.time == Time::Sleep || (self.no_movement && crate::pause::is_movement(task)) {
            return false;
        }
        let area = self.areas.of(task);
        if task.office_hours && area.admin && !area.work {
            return self.time.offices();
        }
        in_view(area, self.time, self.week)
    }
}

impl Situation {
    pub fn now(config: &crate::config::Config, overrides: &Overrides, blocks: &Blocks, now: &Zoned, tr: &crate::i18n::Translator, cases: &[crate::cases::Case]) -> Situation {
        let mode = mode(&config.week_hours(), &config.time_off, overrides, blocks, now);
        let office = config.office_hours();
        let open = window::current(&office, now).is_some();
        let next = if open { String::new() } else { window::next_opening(&office, now).map(|z| tr.when(&z)).unwrap_or_default() };
        let mut office_days = [false; 7];
        for w in &office {
            if let Some(day) = w.weekday() {
                office_days[day.to_monday_zero_offset() as usize] = true;
            }
        }
        // Time off within the coming year, and today once done for it.
        let today = now.date();
        let horizon = today.checked_add(Span::new().days(400)).unwrap_or(today);
        let mut closed = std::collections::BTreeSet::new();
        for off in &config.time_off {
            let mut day = off.from.max(today);
            while day <= off.until && day <= horizon {
                closed.insert(day);
                let Ok(next) = day.tomorrow() else { break };
                day = next;
            }
        }
        if mode.reason == Reason::DoneForTheDay {
            closed.insert(today);
        }
        let no_movement = mode.free() && !config.free_time.movement(&config.planning);
        let quiet_tasks = QuietTasks { time: mode.time, week: mode.week, areas: TaskAreas::of_config(config, cases), no_movement };
        Situation { mode, offices: crate::taskview::Offices { open, next, now: Some(now.clone()) }, closed, office_days, quiet_tasks }
    }

    /// The task filter, by what the time is for; none when no hours are set.
    pub fn quiet_tasks(&self) -> Option<QuietTasks> {
        (self.mode.time != Time::Any).then(|| self.quiet_tasks.clone())
    }
}

/// A row's boxes: the columns ticked (`reach::Times`).
pub use crate::reach::Times;

/// A state with its times on a channel, as its matrix ticks them: "Neutral:
/// work, admin", "Safe: any time", "Blocked: never"; `one`: as said of one
/// person (French says "Sûr" of a person, "Sûrs" of the list).
pub fn list_choice(tr: &crate::i18n::Translator, who: Who, matrix: &Matrix, one: bool) -> String {
    let times = matrix.times(who);
    let words = if times == Times::ALL {
        tr.text("reach-any", None)
    } else if times == Times::NEVER {
        tr.text("reach-never", None)
    } else {
        times.ids().iter().map(|t| tr.text(&format!("reach-word-{t}"), None)).collect::<Vec<_>>().join(", ")
    };
    let mut args = crate::i18n::args();
    args.set("list", tr.text(&format!("sender-{}-{}", if one { "one" } else { "list" }, who.id()), None));
    args.set("times", words);
    tr.text("sender-list-times", Some(&args))
}

/// Whether mail comes forward now (docs/porch.md, "Who may reach you, and
/// when"): the codes and links you just asked a site for, and what you send
/// yourself, at once; with no hours set, everything; else when mail's matrix
/// (`reach`, as `pause::reach_now` gives it for now) ticks its sender's row
/// now. Your safe senders' mail comes to any of your addresses; the others'
/// only to an address for what now is for (docs/areas.md), unless the two
/// never meet in your week: then their row alone decides, so that no mail
/// waits for good. Forged mail, set aside before (porch.rs), is weighed as a
/// stranger's, never as its sender's.
pub fn mail_in_view(triaged: &crate::porch::Triaged, senders: &crate::porch::Senders, reach: &Matrix, account: Area, time: Time, week: Week) -> bool {
    if triaged.lane == crate::porch::Lane::RightNow || triaged.reasons.contains(&crate::porch::Reason::FromYourself) || time == Time::Any {
        return true;
    }
    let who = if triaged.lane == crate::porch::Lane::SetAside { Who::Stranger } else { senders.who_of(&triaged.card) };
    let times = reach.times(who);
    if !times.at(time, week) {
        return false;
    }
    if who == Who::Safe || in_view(account, time, week) {
        return true;
    }
    // The address is for other times than those of its sender: when they never meet, the row decides.
    !Time::STATES.into_iter().any(|t| week.has(t) && times.at(t, week) && in_view(account, t, week))
}

/// Whether a notification may come now (docs/health.md, "Do not disturb"):
/// any, but during sleep; then only a dose, unless you asked doses to stay
/// silent while you sleep (they come at waking). What the window shows is
/// shown when you open it. The pauses as `may_tell` says, doses coming.
pub fn may_notify(mode: &Mode, dose: bool, doses_in_sleep: bool) -> bool {
    may_tell(mode, if dose { Notice::Dose } else { Notice::Other }, doses_in_sleep, true)
}

/// What a notification is, for whether it may come now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// A dose's reminder.
    Dose,
    /// A code or a link you just asked a site for.
    Code,
    /// The alarm an event of yours carries.
    Alarm,
    /// Anything else: reminders before dates, sites, meals, the pause to move.
    Other,
}

/// Whether a notification may come now (docs/health.md, docs/pauses.md):
/// - the pause: a dose, unless the pause's setup holds them too
///   (`doses_in_pause`), and an event's own alarm, an alarm you set (P7;
///   whether its event falls in the pause is `reminders`' to say);
/// - sleep: only a dose, unless doses stay silent then (`doses_in_sleep`);
/// - Free time: doses, codes you asked for, your events' alarms; nothing else;
/// - else: any.
pub fn may_tell(mode: &Mode, notice: Notice, doses_in_sleep: bool, doses_in_pause: bool) -> bool {
    if mode.paused() {
        return (notice == Notice::Dose && doses_in_pause) || notice == Notice::Alarm;
    }
    if mode.sleeps() {
        return notice == Notice::Dose && doses_in_sleep;
    }
    !(mode.free() && notice == Notice::Other)
}

/// Whether a task is yours, outside work: one of its categories is among
/// `personal` (case and accents aside), or one of its cases is marked as yours.
pub fn personal_task(task: &crate::tasks::Task, personal: &[String], personal_cases: &[String]) -> bool {
    let fold = |s: &str| crate::text::fold(s.trim()).into_iter().collect::<String>();
    let wanted: Vec<String> = personal.iter().map(|p| fold(p)).collect();
    task.categories.iter().any(|c| wanted.contains(&fold(c))) || task.cases.iter().any(|c| personal_cases.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::needs::{Days, Needs};

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    fn week() -> Vec<AdminWindow> {
        ["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: None }).collect()
    }

    fn none() -> Blocks {
        Blocks::default()
    }

    /// Three meals, a nap and a night, as Health first sets them: breakfast
    /// 07:50–08:20, lunch 12:10–13:00, a nap 14:00–14:35, dinner 19:00–20:00,
    /// the night from 22:00 (bed at 23:00) to 07:00.
    fn health() -> Needs {
        Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() }
    }

    #[test]
    fn work_rests_outside_its_hours() {
        let none = Overrides::default();
        // 2 October 2026 is a Friday.
        let friday_noon = at("2026-10-02T12:00[Europe/Paris]");
        let m = mode(&week(), &[], &none, &self::none(), &friday_noon);
        assert_eq!((m.quiet, m.reason.clone(), m.time), (false, Reason::Working, Time::Work));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-02T17:00:00");
        let friday_evening = at("2026-10-02T19:00[Europe/Paris]");
        let m = mode(&week(), &[], &none, &self::none(), &friday_evening);
        assert_eq!((m.quiet, m.reason.clone(), m.time), (true, Reason::Evening, Time::Leisure), "the evening is leisure");
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-05T09:00:00", "until Monday morning");
        let saturday = at("2026-10-03T11:00[Europe/Paris]");
        assert_eq!(mode(&week(), &[], &none, &self::none(), &saturday).reason, Reason::DayOff);
        // Holidays the next week: leisure until the Monday after.
        let off = vec![TimeOff { from: "2026-10-05".parse().unwrap(), until: "2026-10-09".parse().unwrap(), label: "Holidays".into() }];
        let m = mode(&week(), &off, &none, &self::none(), &saturday);
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-12T09:00:00");
        let m = mode(&week(), &off, &none, &self::none(), &at("2026-10-06T10:00[Europe/Paris]"));
        assert_eq!((m.quiet, m.reason, m.label.as_str(), m.time), (true, Reason::TimeOff, "Holidays", Time::Leisure));
        // Done for the day at noon: leisure; working late in the evening: work.
        let rest = Overrides { rest_until: Some(at("2026-10-05T09:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let m = mode(&week(), &[], &rest, &self::none(), &friday_noon);
        assert_eq!((m.reason, m.time), (Reason::DoneForTheDay, Time::Leisure));
        let late = Overrides { work_until: Some(at("2026-10-02T20:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let m = mode(&week(), &[], &late, &self::none(), &friday_evening);
        assert_eq!((m.quiet, m.reason, m.time), (false, Reason::WorkingLate, Time::Work));
        assert_eq!(mode(&[], &[], &none, &self::none(), &saturday).reason, Reason::NoHours);
        // Stopping on Friday at noon: work comes back on Monday; without hours, tomorrow morning.
        assert_eq!(after_today(&week(), &[], &friday_noon, 7).datetime().to_string(), "2026-10-05T09:00:00");
        assert_eq!(after_today(&[], &[], &friday_noon, 7).datetime().to_string(), "2026-10-03T07:00:00");
        assert_eq!(after_today(&[], &[], &friday_noon, 30).datetime().to_string(), "2026-10-03T00:00:00", "a morning hour that is none");
        let tr = crate::i18n::Translator::new("en");
        assert_eq!(until_text(&tr, &at("2026-10-02T17:00[Europe/Paris]"), &friday_noon), "17:00");
        assert_eq!(until_text(&tr, &at("2026-10-03T09:00[Europe/Paris]"), &friday_noon), "tomorrow at 09:00");
        assert_eq!(until_text(&tr, &at("2026-10-05T09:00[Europe/Paris]"), &friday_noon), "Monday 5 October at 09:00");
    }

    #[test]
    fn work_now_until_the_next_working_day_is_over() {
        // Friday 2 October 2026 at 19:00, Saturday at 11:00, Friday at noon.
        let (friday_evening, saturday, friday_noon) = (at("2026-10-02T19:00[Europe/Paris]"), at("2026-10-03T11:00[Europe/Paris]"), at("2026-10-02T12:00[Europe/Paris]"));
        assert_eq!(end_of_next_workday(&week(), &[], &saturday).datetime().to_string(), "2026-10-05T17:00:00", "Monday's end");
        assert_eq!(end_of_next_workday(&week(), &[], &friday_evening).datetime().to_string(), "2026-10-05T17:00:00");
        assert_eq!(end_of_next_workday(&week(), &[], &friday_noon).datetime().to_string(), "2026-10-02T17:00:00", "today's, not over yet");
        // Two spans a day: the day is over at the last one's end.
        let mut split = week();
        split.push(AdminWindow { day: "monday".into(), start: "19:00".into(), end: Some("20:00".into()), minutes: 0, kind: None });
        assert_eq!(end_of_next_workday(&split, &[], &saturday).datetime().to_string(), "2026-10-05T20:00:00");
        // Holidays the next week: the Monday after; no working hours: midnight.
        let off = vec![TimeOff { from: "2026-10-05".parse().unwrap(), until: "2026-10-09".parse().unwrap(), label: "Holidays".into() }];
        assert_eq!(end_of_next_workday(&week(), &off, &saturday).datetime().to_string(), "2026-10-12T17:00:00");
        assert_eq!(end_of_next_workday(&[], &[], &saturday).datetime().to_string(), "2026-10-04T00:00:00");
        // Ticked on Saturday: work shown, as work, until then.
        let now = Overrides { work_now: Some(end_of_next_workday(&week(), &[], &saturday).timestamp().as_second()), ..Overrides::default() };
        let m = mode(&week(), &[], &now, &none(), &saturday);
        assert_eq!((m.time, m.reason, m.quiet), (Time::Work, Reason::WorkNow, false));
        assert_eq!(mode(&week(), &[], &now, &none(), &at("2026-10-05T17:30[Europe/Paris]")).reason, Reason::Evening, "over with Monday");
    }

    #[test]
    fn work_and_admin_at_once() {
        let none = Overrides::default();
        let mut hours = week();
        hours.push(AdminWindow { day: "friday".into(), start: "16:00".into(), end: Some("18:00".into()), minutes: 0, kind: Some("admin".into()) });
        // Friday 2 October at 16:30: both are open; until 17:00, when work's close.
        let m = mode(&hours, &[], &none, &self::none(), &at("2026-10-02T16:30[Europe/Paris]"));
        assert_eq!((m.time, m.quiet), (Time::Several(Area::MIXED), false));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-02T17:00:00");
        assert!(in_view(Area::ADMIN, m.time, m.week) && in_view(Area::WORK, m.time, m.week) && !in_view(Area::LEISURE, m.time, m.week));
        // At 17:30, admin alone.
        let m = mode(&hours, &[], &none, &self::none(), &at("2026-10-02T17:30[Europe/Paris]"));
        assert_eq!((m.time, m.reason, m.quiet), (Time::Admin, Reason::AdminTime, true));
    }

    #[test]
    fn old_leisure_windows_read_and_left_aside() {
        // An older Sioul's free time on Saturdays: read without a word, kept in the file, never a time.
        let config: crate::config::Config = toml::from_str(
            "[[window]]\nday = \"friday\"\nstart = \"09:00\"\nend = \"17:00\"\n\n[[window]]\nday = \"saturday\"\nstart = \"10:00\"\nend = \"18:00\"\nkind = \"leisure\"\n\n[[window]]\nday = \"friday\"\nstart = \"18:00\"\nend = \"19:00\"\nkind = \"admin\"\n",
        )
        .unwrap();
        assert_eq!(config.windows.len(), 3, "kept");
        assert_eq!(config.week_hours().iter().map(|w| w.kind()).collect::<Vec<_>>(), ["work", "admin"]);
        let none = Overrides::default();
        let saturday = at("2026-10-03T11:00[Europe/Paris]");
        let m = mode(&config.week_hours(), &[], &none, &self::none(), &saturday);
        assert_eq!((m.time, m.reason), (Time::Leisure, Reason::DayOff));
        // Given all the same, they change nothing.
        let m = mode(&config.windows, &[], &none, &self::none(), &saturday);
        assert_eq!((m.time, m.reason, m.week.work_hours, m.week.admin_hours), (Time::Leisure, Reason::DayOff, true, true));
        // Friday at 20:00, after admin's hour: leisure until Monday... none on Monday here: the next Friday.
        let m = mode(&config.windows, &[], &none, &self::none(), &at("2026-10-02T20:00[Europe/Paris]"));
        assert_eq!((m.time, m.reason.clone()), (Time::Leisure, Reason::Evening));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-09T09:00:00", "never Saturday's old free time");
    }

    #[test]
    fn sleep_then_meals_then_the_hours() {
        let none = Overrides::default();
        let friday = |time: &str| at(&format!("2026-10-02T{time}[Europe/Paris]"));
        let blocks = |now: &Zoned| Blocks::of(&health(), &Days::default(), &[], now);
        let mode_at = |time: &str, overrides: &Overrides| {
            let now = if time.starts_with("+") { at(&format!("2026-10-03T{}[Europe/Paris]", &time[1..])) } else { friday(time) };
            mode(&week(), &[], overrides, &blocks(&now), &now)
        };
        // Work, until lunch is got ready.
        let m = mode_at("10:00", &none);
        assert_eq!((m.time, m.reason.clone(), m.quiet), (Time::Work, Reason::Working, false));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-02T12:10:00");
        // A meal within working hours is a meal while it lasts; work comes back after it.
        let m = mode_at("12:30", &none);
        assert_eq!((m.time, m.reason.clone(), m.quiet), (Time::Meals, Reason::Meal, true));
        assert_eq!((m.until.unwrap().datetime().to_string(), m.back.unwrap().datetime().to_string()), ("2026-10-02T13:00:00".into(), "2026-10-02T13:00:00".into()));
        assert!(m.week.meals && m.week.sleep);
        // A nap, and its minutes to come back.
        let m = mode_at("14:10", &none);
        assert_eq!((m.time, m.reason.clone()), (Time::Sleep, Reason::Nap));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-02T14:35:00");
        // The evening: leisure until the night begins, winding down, then the night until waking.
        let m = mode_at("20:30", &none);
        assert_eq!((m.time, m.reason.clone(), m.until.unwrap().datetime().to_string()), (Time::Leisure, Reason::Evening, "2026-10-02T22:00:00".into()));
        assert_eq!(m.back.unwrap().datetime().to_string(), "2026-10-05T09:00:00", "work comes back on Monday");
        let m = mode_at("22:30", &none);
        assert!(m.sleeps() && m.quiet);
        assert_eq!((m.time, m.reason.clone(), m.until.unwrap().datetime().to_string()), (Time::Sleep, Reason::WindingDown, "2026-10-03T07:00:00".into()));
        assert_eq!(mode_at("23:30", &none).reason, Reason::Sleep);
        assert_eq!((mode_at("+06:00", &none).time, mode_at("+06:00", &none).reason), (Time::Sleep, Reason::Sleep), "the night ending this morning");
        // Sleep and meals come first, whatever the overrides: "Work now" may span a night.
        let work_now = Overrides { work_now: Some(friday("23:59").timestamp().as_second() + 86_400 * 3), ..Overrides::default() };
        assert_eq!(mode_at("23:30", &work_now).time, Time::Sleep);
        assert_eq!(mode_at("12:30", &work_now).time, Time::Meals);
        assert_eq!(mode_at("10:00", &work_now).reason, Reason::WorkNow);
        let done = Overrides { rest_until: Some(friday("23:59").timestamp().as_second()), ..Overrides::default() };
        assert_eq!(mode_at("12:30", &done).time, Time::Meals);
        assert_eq!(mode_at("11:00", &done).reason, Reason::DoneForTheDay);
        // No working or admin hours at all: everything, but meals and sleep keep their time.
        let now = friday("12:30");
        assert_eq!(mode(&[], &[], &none, &blocks(&now), &now).time, Time::Meals);
        let now = friday("10:00");
        let m = mode(&[], &[], &none, &blocks(&now), &now);
        assert_eq!((m.time, m.quiet), (Time::Any, false));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-02T12:10:00");
        // Lunch off today: work goes on; lunch quiet today (no notice): still a meal.
        let mut days = Days::default();
        days.change(now.date(), "meal:1", |b| b.off = true);
        let noon = friday("12:30");
        assert_eq!(mode(&week(), &[], &none, &Blocks::of(&health(), &days, &[], &noon), &noon).time, Time::Work);
        let mut days = Days::default();
        days.change(now.date(), "meal:1", |b| b.quiet = true);
        assert_eq!(mode(&week(), &[], &none, &Blocks::of(&health(), &days, &[], &noon), &noon).time, Time::Meals);
        // Without a night, nights are leisure: nothing keeps notifications away.
        let no_night = Needs { sleep_on: false, ..health() };
        let late = friday("23:30");
        let blocks = Blocks::of(&no_night, &Days::default(), &[], &late);
        assert!(!blocks.night && blocks.sleep, "naps still");
        assert_eq!(mode(&week(), &[], &none, &blocks, &late).time, Time::Leisure);
    }

    #[test]
    fn the_first_step_on_its_day() {
        let monday: Date = "2026-10-05".parse().unwrap();
        let overrides = Overrides { first_step: Some("Open the renewal form (15 min)".into()), first_step_task: Some("renewal".into()), first_step_day: Some(monday), ..Overrides::default() };
        assert_eq!(overrides.first_step_on(monday), Some(("Open the renewal form (15 min)", "renewal")));
        assert_eq!(overrides.first_step_on("2026-10-06".parse().unwrap()), None, "never the day after");
        let closed = Overrides { rest_from: Some(at("2026-10-02T12:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        assert!(closed.closed_today(&at("2026-10-02T19:00[Europe/Paris]")));
        assert!(!closed.closed_today(&at("2026-10-03T09:00[Europe/Paris]")), "today's plan is taken back the same day only");
    }

    #[test]
    fn what_is_yours() {
        let task = |cats: &[&str], cases: &[&str]| crate::tasks::Task { categories: cats.iter().map(|s| s.to_string()).collect(), cases: cases.iter().map(|s| s.to_string()).collect(), ..crate::tasks::Task::default() };
        let personal = vec!["Famille".to_string(), "joy".into()];
        assert!(personal_task(&task(&["famille"], &[]), &personal, &[]));
        assert!(personal_task(&task(&[], &["garden"]), &personal, &["garden".into()]));
        assert!(!personal_task(&task(&["you"], &["taxes"]), &personal, &["garden".into()]));
        let areas = TaskAreas { work_categories: vec!["travail".into()], work_cases: vec!["client-x".into()], ..TaskAreas::usual() };
        let week = Week { work_hours: true, ..Week::default() };
        // Asleep: no task at all, yours neither.
        let night = QuietTasks { time: Time::Sleep, week, areas: areas.clone(), no_movement: false };
        assert!(!night.keeps(&task(&[], &["taxes"])) && !night.keeps(&task(&["joy"], &[])) && !night.keeps(&task(&["Travail"], &[])));
        // In leisure (holidays, a day closed, the evening) and during a meal: only what is yours.
        for time in [Time::Leisure, Time::Meals] {
            let free = QuietTasks { time, ..night.clone() };
            assert!(!free.keeps(&task(&[], &["taxes"])) && free.keeps(&task(&["joy"], &[])) && free.keeps(&task(&["santé"], &[])));
            assert!(!free.keeps(&crate::tasks::Task { office_hours: true, ..task(&[], &["taxes"]) }), "offices keep business hours");
        }
        // Working hours without admin hours of its own: admin comes then, as it always did.
        let working = QuietTasks { time: Time::Work, ..night.clone() };
        assert!(working.keeps(&task(&[], &["taxes"])) && working.keeps(&task(&["Travail"], &[])) && !working.keeps(&task(&["joy"], &[])));
        // Admin with hours of its own: it waits for them; a call to an office still fits the working day.
        let set = QuietTasks { time: Time::Work, week: Week { admin_hours: true, ..week }, areas, no_movement: false };
        // In free time with movement off: no exercise offered.
        let free = QuietTasks { time: Time::Leisure, no_movement: true, ..set.clone() };
        assert!(!free.keeps(&task(&["joy", "Yoga"], &[])) && free.keeps(&task(&["joy"], &[])));
        assert!(!set.keeps(&task(&[], &["taxes"])));
        assert!(set.keeps(&crate::tasks::Task { office_hours: true, ..task(&[], &["taxes"]) }));
    }

    #[test]
    fn hours_for_admin() {
        let none = Overrides::default();
        let mut hours = week();
        hours.push(AdminWindow { day: "friday".into(), start: "18:00".into(), end: Some("19:00".into()), minutes: 0, kind: Some("admin".into()) });
        let m = mode(&hours, &[], &none, &self::none(), &at("2026-10-02T18:30[Europe/Paris]"));
        assert_eq!((m.time, m.reason.clone(), m.quiet), (Time::Admin, Reason::AdminTime, true));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-02T19:00:00");
        // After every hours set: leisure, until the next hours of either kind.
        let m = mode(&hours, &[], &none, &self::none(), &at("2026-10-02T20:00[Europe/Paris]"));
        assert_eq!((m.time, m.reason.clone()), (Time::Leisure, Reason::Evening));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-05T09:00:00");
        assert_eq!(mode(&hours, &[], &none, &self::none(), &at("2026-10-02T10:00[Europe/Paris]")).time, Time::Work);
        assert!(m.week.work_hours && m.week.admin_hours && !m.week.meals && !m.week.sleep);
        assert_eq!(mode(&[], &[], &none, &self::none(), &at("2026-10-02T10:00[Europe/Paris]")).time, Time::Any);
    }

    /// A message from `from` as the Porch judges it; `headers` before the subject.
    fn message(from: &str, headers: &str, subject: &str, senders: &crate::porch::Senders) -> crate::porch::Triaged {
        let raw = format!("From: {from}\r\n{headers}Subject: {subject}\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\n{subject}.\r\n");
        let known = crate::porch::SenderList::default();
        let trusted = ["mx.example.net".to_string()];
        let own = ["me@example.net".to_string()];
        let ctx = crate::porch::Context { cases: None, known: &known, senders, trusted_ids: &trusted, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &own };
        crate::porch::triage(crate::card::Card::from_bytes(raw.as_bytes()).unwrap(), &ctx)
    }

    #[test]
    fn who_may_write_when() {
        use crate::porch::{SenderList, Senders};
        use crate::reach::Reach;
        // Mail's usual matrix: safe at any time, the pause too; neutral in work and admin; restricted in work; strangers as neutral.
        let usual = Reach::default().mail;
        assert_eq!((usual.safe.ids(), usual.neutral.ids(), usual.restricted.ids(), usual.stranger.ids()), (vec!["work", "admin", "leisure", "meals", "sleep", "pause"], vec!["work", "admin"], vec!["work"], vec!["work", "admin"]));
        assert_eq!(Reach::of(&crate::config::ReachSettings::default()).mail, usual);
        let mine = Reach::of(&crate::config::ReachSettings { neutral: Some(vec!["Leisure".into(), "repas".into()]), restricted: Some(Vec::new()), ..Default::default() }).mail;
        assert_eq!((mine.safe, mine.neutral.ids(), mine.restricted), (Times::ALL, vec!["leisure", "meals"], Times::NEVER), "a row left out keeps its usual times; an empty one is never");
        assert_eq!(mine.stranger, mine.neutral, "written before the five states: strangers as the neutral");
        let senders = Senders { safe: SenderList::parse("jane@example.org"), restricted: SenderList::parse("*@company.example"), ..Senders::default() };
        let set = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        let (work_address, personal, leisure_address) = (Area::WORK, Area::PERSONAL, Area::LEISURE);
        let comes = |t: &crate::porch::Triaged, reach: &Matrix, account: Area, time: Time| mail_in_view(t, &senders, reach, account, time, set);
        // Safe: to any address, at every time ticked; untick sleep and it waits.
        let jane = message("Jane <jane@example.org>", "", "Hello", &senders);
        assert!(Time::STATES.into_iter().all(|time| comes(&jane, &usual, work_address, time)));
        let no_sleep = Matrix { safe: Times { sleep: false, ..Times::ALL }, ..usual };
        assert!(!comes(&jane, &no_sleep, work_address, Time::Sleep) && comes(&jane, &no_sleep, work_address, Time::Meals));
        // A stranger, to the work address: work time only (the address is work's, the row says work and admin).
        let stranger = message("Someone <someone@elsewhere.example>", "", "A question", &senders);
        assert_eq!(senders.who_of(&stranger.card), Who::Stranger);
        let at_times = |t: &crate::porch::Triaged, reach: &Matrix, account: Area| Time::STATES.into_iter().filter(|time| comes(t, reach, account, *time)).map(Time::id).collect::<Vec<_>>();
        assert_eq!(at_times(&stranger, &usual, work_address), ["work"]);
        assert_eq!(at_times(&stranger, &usual, personal), ["admin"]);
        // The address and the row never meet: the row alone decides, so nothing waits for good.
        assert_eq!(at_times(&stranger, &usual, leisure_address), ["work", "admin"]);
        // Strangers on their own row: never, and they wait while the neutral come.
        let no_strangers = Matrix { stranger: Times::NEVER, ..usual };
        assert_eq!(at_times(&stranger, &no_strangers, work_address), Vec::<&str>::new());
        let boss = message("Boss <boss@company.example>", "", "Monday", &senders);
        assert_eq!(at_times(&boss, &usual, personal), ["work"], "restricted to work, writing to a personal address");
        assert_eq!(at_times(&boss, &usual, work_address), ["work"]);
        assert_eq!(at_times(&boss, &mine, work_address), Vec::<&str>::new(), "never");
        // Without admin hours, work time takes admin's ticks; without work hours, admin time takes work's.
        let admin_only = Matrix { stranger: Times { admin: true, ..Times::NEVER }, ..usual };
        let no_admin = Week { admin_hours: false, ..set };
        assert!(mail_in_view(&stranger, &senders, &admin_only, Area::MIXED, Time::Work, no_admin));
        assert!(!mail_in_view(&stranger, &senders, &admin_only, Area::MIXED, Time::Work, set));
        let no_work = Week { work_hours: false, ..set };
        assert!(mail_in_view(&boss, &senders, &usual, work_address, Time::Admin, no_work));
        // Codes asked for and what you send yourself: at once, asleep or not; no hours set: everything.
        let code = message("Bank <codes@bank.example>", "", "Your verification code: 482913", &senders);
        assert_eq!(code.lane, crate::porch::Lane::RightNow);
        assert!(comes(&code, &mine, work_address, Time::Sleep));
        let mine_own = message("Me <me@example.net>", "Authentication-Results: mx.example.net; dmarc=pass header.from=example.net\r\n", "A file", &senders);
        assert!(mine_own.reasons.contains(&crate::porch::Reason::FromYourself), "{:?}", mine_own.reasons);
        assert!(comes(&mine_own, &mine, work_address, Time::Sleep));
        assert!(comes(&stranger, &usual, leisure_address, Time::Any));
        // Forged in a safe sender's name: set aside, and weighed as a stranger's.
        let forged = message("Jane <jane@example.org>", "Authentication-Results: mx.example.net; dmarc=fail (p=reject) header.from=example.org\r\n", "Hello", &senders);
        assert_eq!(forged.lane, crate::porch::Lane::SetAside);
        assert!(!comes(&forged, &usual, personal, Time::Leisure) && comes(&forged, &usual, personal, Time::Admin));
        assert!(!comes(&forged, &no_strangers, personal, Time::Admin), "on the strangers' row");
    }

    #[test]
    fn nothing_disturbs_but_doses() {
        let sleeping = Mode { quiet: true, time: Time::Sleep, week: Week::default(), reason: Reason::Sleep, until: None, back: None, label: String::new() };
        let awake = Mode { time: Time::Leisure, reason: Reason::Evening, ..sleeping.clone() };
        assert!(!may_notify(&sleeping, false, true), "no notification during sleep");
        assert!(may_notify(&sleeping, true, true), "a dose comes: you set its time");
        assert!(!may_notify(&sleeping, true, false), "unless doses stay silent then");
        assert!(may_notify(&awake, false, false) && may_notify(&awake, true, false));
        let winding = Mode { reason: Reason::WindingDown, ..sleeping };
        assert!(winding.sleeps() && !may_notify(&winding, false, true));
    }
}

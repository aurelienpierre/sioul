// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The cards on a phone's home screen (docs/android.md, "The card on the
//! home screen"). The full card: today first, the date and the weather (now,
//! the next four hours, the next morning, afternoon, evening and night, then
//! the next seven days); then what the Porch has for you that is not mail
//! (today's doses due, a code you asked for, reminders of dates, waits,
//! payments and papers, do-not-disturb's line, the calls declined, two events
//! at once); then Now: the next step when it is the time for one, and where
//! you stopped. The mail card and the agenda card list the latest messages
//! the Porch shows (sender, date, subject, a line of the text, the account's
//! colour) and the coming appointments, day by day, each in its calendar's
//! colour. Never a count. Android draws them (android/package/src/com/
//! aurelienpierre/sioul/HomeCard.java, the lists' rows HomeCardRows.java),
//! without Qt: Sioul writes what they say in a small file of its own
//! (`home-card.json`, in its state folder) and tells Android to draw them again.
//!
//! Android freezes Sioul in the background, so the card is given ahead, in
//! frames: one per change of time until the end of tomorrow (work, admin, a
//! meal, winding down, sleep, waking, the Porch's hours, midnight), each with
//! its own words, the Porch's mail as it lets mail through then, the
//! reminders and calls it lets show then, and the next step as the plan
//! stands then. The weather is given hour by hour, from the forecast kept
//! (40 hours and more ahead, `weather::card`), each hour with its own "now",
//! next hours and parts of the day. The messages and the events are written
//! once, each word that changes at midnight with until when it holds ("14:05",
//! then "yesterday"; "Tomorrow, Thu 8 Oct", then "Today, Thu 8 Oct"). Java
//! shows the frame and the hour for now, leaves out the events over, and draws
//! again at the next change. Rust writes the card when the Porch is computed
//! (mail fetched), when the plan is made, when the agenda is read again (an
//! event changed), when a forecast or the reminders change, at the window's
//! minute when a frame ended (midnight among them) or five minutes went by,
//! and when Sioul is put away; outside the window, where Rust runs already (a
//! dose's alarm, its "Taken"), it writes the doses' lines alone. Nothing here
//! fetches anything. Elsewhere than on Android nothing is written, unless
//! SIOUL_HOME_CARD is set (to read the file on a computer).

use crate::backend::{load_config, tr};
use jiff::civil::Date;
use jiff::{Span, Timestamp, Zoned};
use serde::Serialize;
use sioul_core::agenda::Occurrence;
use sioul_core::areas::{Area, Time};
use sioul_core::projects::ProjectStore;
use sioul_core::codes::CodeKind;
use sioul_core::config::Config;
use sioul_core::i18n::{self, Translator};
use sioul_core::links::Loaded;
use sioul_core::plan::Plan;
use sioul_core::porch::{self, Lane, Senders, Triaged};
use sioul_core::quiet::{self, Blocks, Mode, Overrides, Reason};
use sioul_core::reminders::Reminder;
use sioul_core::taskview::{self, Filter};
use sioul_core::today::{Today, Weather};
use sioul_core::trust::Trust;
use sioul_core::weather::{self as forecast, DaySlot, Forecast, Slot};
use sioul_core::{pause, view, window};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The card's file, in Sioul's state folder; Java reads it there
/// (`files/state/sioul/`, android/main.cpp's XDG folders).
const FILE: &str = "home-card.json";
/// The file's version: 3 from the full card made around today (October 2026).
/// Its frames are named "times": an older Sioul's Java reads "frames", finds
/// none, and says `beyond` until Sioul opens. This Java reads both.
const VERSION: u32 = 3;
/// The reminders the card holds, at most, soonest first (Java shows a few).
const REMINDERS: usize = 12;
/// A forecast older than this is not shown: Sioul was not opened for a day.
const FORECAST_KEPT: i64 = 24 * 3600;
/// A tap on the card, kept by Java (HomeCardOpener.java) for the window.
const OPENED: &str = "home-card-opened";
/// The flag, among the task pages' (`work::WorkState`), that turns the details off.
pub(crate) const PLAIN: &str = "home-card-plain";
/// How long the card says nothing of its age: past it, "at 14:05".
const FRESH_MS: i64 = 15 * 60 * 1000;
/// While the window runs, the card is written again at least this often, its age said afresh.
const REWRITE_EVERY: i64 = 5 * 60;
/// The Porch's messages the card lists, at most, newest first.
const MAIL: usize = 20;
/// The agenda's reach: today and the days to come.
const HORIZON_DAYS: i64 = 30;
/// The appointments the card lists, at most, nearest first.
const EVENTS: usize = 60;
/// A message's line of text, at most (Java shows one line of it).
const TEXT_CHARS: usize = 160;
/// Frames at most: each change of time until the end of tomorrow.
const FRAMES: usize = 48;
/// A tap older than this is let go: Sioul did not come up meanwhile.
const TAP_KEPT_MS: i64 = 5 * 60 * 1000;

/// What the card says, as Java reads it (HomeCard.java). Times are Unix milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Snapshot {
    v: u32,
    /// When Sioul made it.
    made: i64,
    /// "Details on the home screen" (Settings): off, no message, code, dose or step's title.
    details: bool,
    /// From then on, the card says how old it is (`stale`).
    stale_after: i64,
    /// "at 14:05" until midnight, "yesterday at 14:05" the next day, then a short date (`until` 0).
    stale: Vec<Label>,
    /// Said once the last frame is over: Sioul was not opened for that long.
    beyond: String,
    /// The lists' headings: "Mail", "Agenda".
    words: Words,
    /// Today, first on the full card: the date, the weather.
    today: TodayLines,
    /// The frames, from now to the end of tomorrow (`VERSION`: "times").
    #[serde(rename = "times")]
    frames: Vec<Frame>,
    /// Every message a frame lists (`PorchLines::mail` points here), newest first.
    mail: Vec<Mail>,
    /// The coming appointments; none without a calendar.
    agenda: Option<AgendaLines>,
    /// The codes and links you asked sites for, each until it expires; newest first.
    codes: Vec<Code>,
    /// Today's doses due and not marked, from their time on.
    doses: Vec<DoseLine>,
    /// The reminders of dates, waits, payments, papers, contracts and the
    /// money watch, each from its time while it makes sense, soonest first;
    /// each frame says which it shows (`Frame::reminders`).
    reminders: Vec<ReminderLine>,
    /// The calls declined, in the Porch's words; each frame says which it lists.
    calls: Vec<String>,
    /// Two events at once, today's and tomorrow's, each said on its own day
    /// until both are over.
    overlaps: Vec<Timed>,
    /// Where you stopped, the line you left, until you say it is done: "Where
    /// you stopped: …"; details off, that it waits; "" none.
    stopped: String,
}

/// Today, as the full card begins.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct TodayLines {
    /// "Thursday 8 October" until midnight, then "Friday 9 October".
    date: Vec<Label>,
    /// Said instead of the weather: no place chosen yet (where to choose
    /// one), or no forecast yet; "" with the weather.
    line: String,
    weather: Option<WeatherLines>,
}

/// The weather at the place chosen, from the forecast kept.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct WeatherLines {
    /// Hour by hour, from the hour under way to the end of the forecast or of tomorrow.
    hours: Vec<WeatherHour>,
    /// The forecast's days, each from its midnight to the next: Java shows
    /// the seven after today.
    days: Vec<WeatherDay>,
    /// "Weather: Open-Meteo.com", small, as its licence asks (CC BY 4.0).
    credit: String,
}

/// One hour of the card's weather: what it says while the hour lasts.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct WeatherHour {
    from: i64,
    until: i64,
    /// "Now": its icon, "14°", "70 %".
    now: Slot,
    /// The next four hours, on the same row as now.
    next: Vec<Slot>,
    /// The next morning, afternoon, evening and night, in order.
    parts: Vec<Slot>,
}

/// One day of the forecast, from its midnight to the next (Unix ms).
#[derive(Debug, Clone, PartialEq, Serialize)]
struct WeatherDay {
    from: i64,
    until: i64,
    #[serde(flatten)]
    day: DaySlot,
}

/// A reminder, as the card says it, from its time while it makes sense.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ReminderLine {
    /// "Rent · €650 · Planned for Friday 10 October; the account will hold
    /// it."; details off, "A reminder waits in Sioul."
    line: String,
    from: i64,
    until: i64,
    /// What a tap opens: its task, budget, paper or contract ("sioul:task/…").
    open: String,
}

/// A line said from a time until another (Unix ms).
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Timed {
    line: String,
    from: i64,
    until: i64,
}

/// The list's headings, in the person's language.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Words {
    mail: String,
    agenda: String,
}

/// One stretch of time that says the same.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Frame {
    from: i64,
    until: i64,
    /// "work", "admin", "work+admin", "any", "leisure", "meals", "sleep", "paused", "free-time".
    kind: String,
    /// The status line's sentence: "Work until 17:00.", "Sleep: nothing disturbs until 07:00."
    status: String,
    /// Under it, the do-not-disturb on every device, switched on or for a focus
    /// session: "Do not disturb, on every device, until 15:00."; "" otherwise.
    dnd: String,
    /// What the Porch shows then; none asleep, in a pause or in free time.
    porch: Option<PorchLines>,
    /// The next step, in work or admin time.
    step: Option<Step>,
    /// Codes may show: as their row of the matrix says (as usual at once,
    /// asleep too; "On the Porch only" a choice for sleep), never on the
    /// pause's card, which shows the pause alone.
    codes: bool,
    /// Doses may show: not in a pause; asleep only when doses remind during sleep.
    doses: bool,
    /// The agenda shows: not in a pause, whose card shows the pause alone.
    events: bool,
    /// The reminders shown then, their places in `Snapshot::reminders`: as
    /// the row of dates of the matrix of what reaches you lets them come
    /// (work's in work's times only); none asleep, in a pause or in free time
    /// as usual. Java shows each from its time.
    reminders: Vec<usize>,
    /// The calls declined listed then, their places in `Snapshot::calls`: at
    /// the times their callers may reach you, as the Porch lists them.
    calls: Vec<usize>,
    /// Two events at once may show: not asleep, in a pause or in free time.
    overlaps: bool,
    /// Where you stopped shows: with the next step, in work or admin time.
    stopped: bool,
    /// Under the status line: "Sioul is not set up yet."; "" otherwise.
    note: String,
}

/// The Porch's part of a frame, as written.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct PorchLines {
    /// Said instead of messages, or above none: "The Porch opens at 14:00.",
    /// "Nothing waits on the Porch.", "Sioul is not set up yet.", details off
    /// "Mail waits on the Porch."; "" when the messages say it.
    line: String,
    /// The messages listed then, newest first: their places in `Snapshot::mail`.
    mail: Vec<usize>,
}

/// What the Porch shows at a time, before its messages are given their places.
#[derive(Debug, Clone, PartialEq)]
struct PorchMade {
    line: String,
    mail: Vec<Mail>,
}

impl PorchMade {
    fn said(line: String) -> PorchMade {
        PorchMade { line, mail: Vec::new() }
    }
}

/// A message, as a mail card lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Mail {
    /// Its file, to open it (HomeCardOpener, then the Porch's Reader).
    key: String,
    /// The sender's name, else the address.
    sender: String,
    subject: String,
    /// The start of its text, on one line.
    text: String,
    /// Not read yet: its sender and subject in bold.
    unread: bool,
    /// The account it came to, as its place among your accounts (Java's
    /// colours, `sioul_home_card_account_N`, light and dark); -1 none.
    account: i32,
    /// When it came: "14:05" today, "Yesterday" the next day, "7 Oct" after
    /// (the year too when not this year's), each until its time; 0 for good.
    when: Vec<Label>,
}

/// The coming appointments, from today to a month on.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct AgendaLines {
    /// Said when none comes: "Nothing in your calendars for the coming month."
    empty: String,
    /// Every day of the reach, in order: Java lists each event under the day
    /// it starts, or today while it runs.
    days: Vec<Day>,
    /// The events, nearest first; Java leaves out those over.
    events: Vec<Event>,
}

/// One day of the agenda, and its name as it changes at each midnight.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Day {
    /// Its midnight, and the next one (Unix ms).
    from: i64,
    until: i64,
    /// "Mon 12 Oct", then "Tomorrow, Mon 12 Oct", then "Today, Mon 12 Oct", each until its time.
    label: Vec<Label>,
}

/// One appointment, as an agenda card shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Event {
    /// Its file, to open it (HomeCardOpener, then the Agenda).
    key: String,
    /// Its title; details off, "An event".
    title: String,
    /// "11:00 – 12:00"; a whole day, ""; several, "Until Fri 9 Oct".
    time: String,
    all_day: bool,
    /// Unix ms.
    start: i64,
    end: i64,
    /// Its calendar's colour, "#4c6b5c"; "" for Sioul's own.
    color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Step {
    title: String,
    /// Why it comes now; details off, its date or its length.
    why: String,
    uid: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Code {
    /// "Code from La Banque (verified): 482913"; details off, "A code waits on the Porch."
    line: String,
    /// Its sender is not verified: use it only if you just asked for it.
    warning: String,
    until: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct DoseLine {
    /// "Magnesium · 300 mg, 12:30", while it is known not marked elsewhere.
    line: String,
    /// "Magnesium · 300 mg, 12:30: check before taking it.", once that is not known.
    check: String,
    from: i64,
    until: i64,
    /// `line` until then, `check` after; 0: `check` from the start.
    known_until: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Label {
    /// Said until then; 0, for good.
    until: i64,
    text: String,
}

/// A dose for the card, as the Health page knows it (`health::card_doses`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Dose {
    /// "Magnesium · 300 mg".
    pub name: String,
    /// "12:30".
    pub time: String,
    /// When it is due, and until when it stays on the card (Unix seconds).
    pub at: i64,
    pub until: i64,
    /// Until when it is known not marked on another device; 0: not known from its time on.
    pub known_until: i64,
}

/// The Porch's messages, as the window last gathered them, before the hours sort them.
pub(crate) struct PorchInput {
    items: Vec<Triaged>,
    senders: Senders,
    store: Option<ProjectStore>,
}

/// The plan, as the window last made it.
pub(crate) struct PlanInput {
    loaded: Arc<Loaded>,
    plan: Plan,
    today: Today,
    spent: BTreeMap<String, u32>,
    stopped: BTreeMap<String, String>,
}

/// The calendars' events from today's midnight to the reach's end, as last
/// read (`agenda_input`), and whether there is a calendar at all.
pub(crate) struct AgendaInput {
    events: Vec<Occurrence>,
    calendars: bool,
    /// The day they were read for: read again the next day.
    read_on: Date,
    /// Two events at once set aside for good (`overlaps::SetAside`): not said.
    aside: BTreeSet<String>,
}

/// The weather the card is made with: whether a place is chosen, and the
/// forecast kept (`backend::update_weather`).
#[derive(Default)]
pub(crate) struct WeatherInput {
    place: bool,
    forecast: Option<Forecast>,
}

/// The calls declined as the Porch lists them at a time (`calls::for_card`):
/// their lines, seen from the time given, as the moment given lets their callers through.
type CallsAt<'a> = &'a dyn Fn(&sioul_core::attention::Now, &Zoned) -> Vec<String>;

/// What the full card is made from besides the Porch, the plan, the agenda and the doses.
#[derive(Default)]
struct More<'a> {
    weather: WeatherInput,
    /// Every reminder, told or not (`reminders::all`); the events' left out here.
    reminders: &'a [Reminder],
    calls: Option<CallsAt<'a>>,
    /// Where you stopped: the line left.
    stopped: Option<String>,
}

/// What a card is made from, besides the Porch and the plan.
struct Moment<'a> {
    now: &'a Zoned,
    config: &'a Config,
    overrides: &'a Overrides,
    blocks: &'a Blocks,
    tr: &'a Translator,
    details: bool,
    /// The do-not-disturb on every device, when the card says it (`Dnd::of`).
    dnd: Option<&'a Dnd>,
}

/// The do-not-disturb on every device, as the card says it: switched on by
/// hand or for a focus session. Sleep, the pause and free time say their own
/// words in the status line, so they add none here.
#[derive(Debug, Clone, PartialEq)]
struct Dnd {
    /// "Do not disturb, on every device, until 15:00.", in the person's language.
    line: String,
    /// When it ends (Unix seconds); 0 when it lasts until switched off; none
    /// when not said: then said in the frame for now only.
    until: Option<i64>,
}

impl Dnd {
    /// From the shared do-not-disturb's moment (`everywhere::moment`): {on,
    /// why, line, until_at}; none when it is off, or for a reason the status
    /// line says already.
    fn of(moment: &serde_json::Value) -> Option<Dnd> {
        let why = moment["why"].as_str().unwrap_or_default();
        let line = moment["line"].as_str().unwrap_or_default().trim();
        if moment["on"].as_bool() != Some(true) || !matches!(why, "manual" | "focus") || line.is_empty() {
            return None;
        }
        // A focus session's end is no promise (it is stopped, or runs over):
        // said in the frame for now only, unless an end is given.
        let until = match (why, moment["until_at"].as_i64()) {
            ("focus", Some(0)) => None,
            (_, until) => until,
        };
        Some(Dnd { line: line.to_string(), until })
    }

    /// Said in a frame starting at `from` (Unix ms): before it ends; never
    /// asleep (the night's line says nothing disturbs) nor in a pause (its
    /// line alone); its end not said, only in the frame for now.
    fn line_in(&self, kind: &str, from: i64, first: bool) -> String {
        let before_end = match self.until {
            Some(0) => true,
            Some(until) => from < until * 1000,
            None => first,
        };
        if before_end && kind != "sleep" && kind != "paused" { self.line.clone() } else { String::new() }
    }
}

fn ms(at: &Zoned) -> i64 {
    at.timestamp().as_millisecond()
}

/// A message with its words given, in `tr`'s language.
fn said(tr: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = i18n::args();
    for (key, value) in pairs {
        args.set(key.to_string(), value.clone());
    }
    tr.text(id, Some(&args))
}

/// The midnight after `at`, in its zone.
fn midnight_after(at: &Zoned) -> Option<Zoned> {
    at.date().tomorrow().ok().and_then(|d| d.to_zoned(at.time_zone().clone()).ok())
}

/// "at 14:00", "tomorrow at 9:00", "on Monday 12 October at 09:00", as seen from `from`.
fn when(tr: &Translator, at: &Zoned, from: &Zoned) -> String {
    let time = at.strftime("%H:%M").to_string();
    if at.date() == from.date() {
        said(tr, "home-card-when-at", &[("time", time)])
    } else if from.date().tomorrow().is_ok_and(|d| d == at.date()) {
        said(tr, "until-tomorrow", &[("time", time)])
    } else {
        tr.when(at)
    }
}

/// "Jeudi 8 octobre": a heading's first letter a capital, whatever the language writes.
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Today's date, as the full card begins: "Thursday 8 October" until
/// midnight, then "Friday 9 October" until the next (the frames end then).
fn date_labels(tr: &Translator, now: &Zoned) -> Vec<Label> {
    let mut labels = Vec::new();
    let mut day = now.date();
    for _ in 0..2 {
        let Some(midnight) = day.tomorrow().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()) else { break };
        labels.push(Label { until: ms(&midnight), text: capital(&tr.day(day)) });
        day = midnight.date();
    }
    labels
}

/// Today, as the full card begins: the date, then the weather at the place
/// chosen, hour by hour from the hour under way until `end` (the last frame's)
/// or the forecast's end, and its days. No place: where to choose one. No
/// forecast, or one older than a day: that none is there yet. The weather is
/// no detail: shown with the details off too.
fn today_lines(m: &Moment, weather: &WeatherInput, end: i64) -> TodayLines {
    let date = date_labels(m.tr, m.now);
    if !weather.place {
        return TodayLines { date, line: m.tr.text("home-card-weather-choose", None), weather: None };
    }
    let now = m.now.timestamp().as_second();
    let Some(kept) = weather.forecast.as_ref().filter(|f| now - f.fetched < FORECAST_KEPT && f.hours.iter().any(|h| h.at + 3600 > now)) else {
        return TodayLines { date, line: m.tr.text("weather-none", None), weather: None };
    };
    let zone = m.now.time_zone().clone();
    let hours = kept
        .hours
        .iter()
        .filter(|h| h.at + 3600 > now && h.at * 1000 < end)
        .filter_map(|h| {
            let at = Timestamp::from_second(h.at).ok()?.to_zoned(zone.clone());
            let view = forecast::card(kept, &at, m.tr)?;
            Some(WeatherHour { from: h.at * 1000, until: (h.at + 3600) * 1000, now: view.now?, next: view.hours, parts: view.parts })
        })
        .collect();
    let days = forecast::days(kept, m.tr)
        .into_iter()
        .filter_map(|day| {
            let from = day.date.to_zoned(zone.clone()).ok()?;
            let until = day.date.tomorrow().ok()?.to_zoned(zone.clone()).ok()?;
            Some(WeatherDay { from: ms(&from), until: ms(&until), day })
        })
        .collect();
    TodayLines { date, line: String::new(), weather: Some(WeatherLines { hours, days, credit: m.tr.text("home-card-weather-credit", None) }) }
}

/// The reminders the card says: those of dates, waits, payments, papers,
/// contracts and the money watch, from their time while they make sense
/// (the events' come as Android's notifications, and on the agenda card),
/// soonest first, in one line each: what, then when or why. Details off,
/// none named. Each with whether it is work's (it waits while work rests).
fn reminder_lines(tr: &Translator, all: &[Reminder], details: bool, now: i64) -> Vec<(ReminderLine, bool)> {
    use sioul_core::reminders::Kind;
    let mut kept: Vec<&Reminder> = all.iter().filter(|r| !matches!(r.kind, Kind::Event | Kind::Before | Kind::Alarm) && r.until > now).collect();
    kept.sort_by(|a, b| (a.at, &a.key).cmp(&(b.at, &b.key)));
    kept.into_iter()
        .take(REMINDERS)
        .map(|r| {
            let parts: Vec<&str> = match r.kind {
                // "Call the CAF · Asked for Friday 30 October".
                Kind::Asked => vec![&r.body, &r.title],
                // A paper's and the money watch's titles say it; their bodies are advice, on the Porch.
                Kind::Paper | Kind::Money => vec![&r.title],
                _ => vec![&r.title, &r.body],
            };
            let line = if details { parts.into_iter().map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ") } else { tr.text("home-card-reminder-plain", None) };
            (ReminderLine { line, from: r.at * 1000, until: r.until * 1000, open: r.target.clone() }, r.work)
        })
        .collect()
}

/// Two events at once today and tomorrow, each said on its own day until
/// both are over, as the Porch says today's (`overlaps`): the time to get
/// there and back counted; those set aside for good left out.
fn overlap_lines(m: &Moment, input: &AgendaInput) -> Vec<Timed> {
    let zone = m.now.time_zone().clone();
    let now = m.now.timestamp().as_second();
    let day_of = |at: i64| Timestamp::from_second(at).ok().map(|t| t.to_zoned(zone.clone()).date());
    let tomorrow = m.now.date().tomorrow().ok();
    sioul_core::overlaps::overlaps(&input.events)
        .into_iter()
        .filter(|o| !input.aside.contains(&o.key) && o.first.end.max(o.second.end) > now)
        .filter_map(|o| {
            let day = day_of(o.second.start)?;
            if day != m.now.date() && Some(day) != tomorrow {
                return None;
            }
            let from = day.to_zoned(zone.clone()).ok()?;
            let title = |e: &Occurrence| if e.summary.trim().is_empty() { m.tr.text("agenda-untitled", None) } else { e.summary.trim().to_string() };
            let line = if m.details { said(m.tr, "home-card-overlap", &[("first", title(&o.first)), ("second", title(&o.second))]) } else { m.tr.text("home-card-overlap-plain", None) };
            Some(Timed { line, from: ms(&from), until: o.first.end.max(o.second.end) * 1000 })
        })
        .collect()
}

/// The changes of time from now to the end of tomorrow (Health's meals and
/// nights are known that far: `Needs::kept_around`), each with what it is
/// for: a frame ends when the time changes (`quiet::mode`'s `until`), when a
/// meal, a nap or the night begins, starts proper (winding down becomes
/// sleep at bedtime) or ends, when the Porch opens or closes, when it stops
/// resting after a pause, and at midnight, so that "tomorrow" in a frame's
/// words stays true.
fn timeline(m: &Moment) -> Vec<(Zoned, Zoned, Mode)> {
    let Some(horizon) = midnight_after(m.now).and_then(|z| midnight_after(&z)) else { return Vec::new() };
    let hours = m.config.week_hours();
    let zone = m.now.time_zone().clone();
    let instant = |seconds: i64| Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()));
    let rests = m.overrides.porch_rests_until.and_then(instant);
    // The do-not-disturb's end too: its line goes then.
    let quiet_end = m.dnd.and_then(|d| d.until).filter(|until| *until > 0).and_then(instant);
    let kept: Vec<Zoned> = m.blocks.kept.iter().flat_map(|k| [k.start, k.at, k.end]).filter_map(instant).chain(quiet_end).collect();
    let mut frames = Vec::new();
    let mut at = m.now.clone();
    while at.timestamp() < horizon.timestamp() && frames.len() < FRAMES {
        let mode = quiet::mode(&hours, &m.config.time_off, m.overrides, m.blocks, &at);
        let mut ends = vec![horizon.clone()];
        ends.extend(midnight_after(&at));
        ends.extend(mode.until.clone());
        ends.extend(kept.iter().cloned());
        ends.extend(window::current(&m.config.windows, &at).map(|(_, closing)| closing));
        ends.extend(window::next_opening(&m.config.windows, &at));
        ends.extend(rests.clone());
        let end = ends
            .into_iter()
            .filter(|e| e.timestamp() > at.timestamp())
            .min_by_key(Zoned::timestamp)
            .unwrap_or_else(|| at.checked_add(Span::new().minutes(1)).unwrap_or_else(|_| horizon.clone()));
        frames.push((at.clone(), end.clone(), mode));
        at = end;
    }
    frames
}

/// What a frame is, for Java's colours and tests: the time's word, or the pause's.
fn kind(mode: &Mode) -> &'static str {
    match mode.reason {
        Reason::Paused => "paused",
        Reason::FreeTime => "free-time",
        _ => mode.time.id(),
    }
}

/// The status line's sentence for `mode`, as seen from `at` (`backend::mode_json`'s,
/// and "Work until 17:00." in working hours, where the window's line says nothing).
fn status(m: &Moment, mode: &Mode, at: &Zoned) -> String {
    let until = mode.until.as_ref().map(|z| quiet::until_text(m.tr, z, at)).unwrap_or_default();
    let id = match mode.reason {
        Reason::FreeTime => return m.tr.text(if pause::nothing_now(m.overrides, &m.config.free_time) { "mode-free-time-nothing" } else { "mode-free-time" }, None),
        Reason::Paused => return m.tr.text("mode-paused", None),
        // Every other sentence says until when: none known, none said.
        _ if until.is_empty() => return String::new(),
        Reason::NoHours => return String::new(),
        Reason::Working | Reason::Extended => "home-card-work",
        Reason::TimeOff => "mode-time-off",
        Reason::WorkingLate => "mode-working-late",
        Reason::WorkNow => "mode-work-now-line",
        Reason::AdminTime => "mode-admin",
        Reason::DoneForTheDay => "mode-quiet",
        Reason::Evening | Reason::DayOff => "mode-leisure",
        Reason::Meal => "mode-meal",
        Reason::WindingDown => "mode-wind-down",
        Reason::Nap => "mode-nap",
        Reason::Sleep => "mode-sleep",
    };
    let mut args = i18n::args();
    args.set("until", until);
    args.set("label", if mode.label.trim().is_empty() { "none".to_string() } else { mode.label.clone() });
    m.tr.text(id, Some(&args))
}

/// "Thu 8 Oct", "jeu. 8 oct.": a day as an agenda card names it.
fn day_short(tr: &Translator, day: Date) -> String {
    let mut args = i18n::args();
    args.set("weekday", tr.weekday_short(day));
    args.set("day", day.day());
    args.set("month", tr.month_short(day));
    tr.text("date-day", Some(&args))
}

/// "7 Oct", "7 oct.", "1er oct.": a message's day, as a mail card dates it;
/// with its year when it is not `today`'s.
fn day_month_short(tr: &Translator, day: Date, today: Date) -> String {
    let mut args = i18n::args();
    args.set("day", day.day());
    args.set("month", tr.month_short(day));
    let said = tr.text("date-day-month", Some(&args));
    if day.year() == today.year() { said } else { format!("{said} {}", day.year()) }
}

/// When a message came, as the card dates it, each wording until it holds no
/// longer (Java picks the first whose `until` is ahead, 0 for good): its time
/// today; "Yesterday" from midnight; its day after. Seen from `now`.
fn mail_when(tr: &Translator, sent: Option<i64>, now: &Zoned) -> Vec<Label> {
    let Some(at) = sent.and_then(|s| Timestamp::from_second(s).ok()).map(|t| t.to_zoned(now.time_zone().clone())) else { return Vec::new() };
    let (day, today) = (at.date(), now.date());
    let first = midnight_after(now);
    let second = first.as_ref().and_then(midnight_after);
    let yesterday = tr.text("home-card-yesterday", None);
    let mut labels = Vec::new();
    match (first, second) {
        (Some(first), Some(second)) if day == today => {
            labels.push(Label { until: ms(&first), text: at.strftime("%H:%M").to_string() });
            labels.push(Label { until: ms(&second), text: yesterday });
        }
        (Some(first), _) if today.yesterday().is_ok_and(|y| y == day) => labels.push(Label { until: ms(&first), text: yesterday }),
        _ => {}
    }
    labels.push(Label { until: 0, text: day_month_short(tr, day, today) });
    labels
}

/// A message the Porch shows, as the card lists it: from the Porch's line
/// (`item`, which keeps hostile and rude words out) and its message (`triaged`:
/// when it came, whether it was read, the account it came to). With when it
/// came, for the order.
fn mail_line(m: &Moment, item: &view::ItemView, triaged: Option<&Triaged>) -> (i64, Mail) {
    let sent = triaged.and_then(|t| t.card.date);
    let account = item.account.as_deref().and_then(|id| m.config.every_account().position(|a| a.id == id)).and_then(|at| i32::try_from(at).ok()).unwrap_or(-1);
    // Read, by its file's flags (`S`), as the Mail page says it; a file not named so (new/): not read.
    let unread = !sioul_core::maildir::flags_of(std::path::Path::new(&item.key)).contains('S');
    let text: String = item.preview.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(TEXT_CHARS).collect();
    let subject = if item.subject.trim().is_empty() { m.tr.text("mail-no-subject", None) } else { item.subject.clone() };
    (sent.unwrap_or(0), Mail { key: item.key.clone(), sender: item.sender.clone(), subject, text, unread, account, when: mail_when(m.tr, sent, m.now) })
}

/// What the Porch shows at `at`, as `backend::compute` makes it: the mail
/// the matrix of what reaches you shows at that time (`Attention::mail`,
/// Free time's "Nothing at all" too), the Porch open in its hours and in quiet time, closed
/// otherwise with when it opens, resting after a pause until its next hours
/// (`pauses::porch_rests`). Never "Open it anyway": a moment's choice in the
/// window, not the card's. Its messages newest first, as a mail card lists
/// them; never a count.
fn porch_at(m: &Moment, input: &PorchInput, mode: &Mode, at: &Zoned) -> PorchMade {
    if let Some(until) = m.overrides.porch_rests_until.filter(|_| m.overrides.porch_rests(at.timestamp().as_second())).and_then(|u| Timestamp::from_second(u).ok()) {
        let until = until.to_zoned(at.time_zone().clone());
        return PorchMade::said(said(m.tr, "home-card-porch-rests", &[("when", when(m.tr, &until, at))]));
    }
    let attention = sioul_core::attention::Attention::of(m.config);
    let mut moment = sioul_core::attention::Now::of(mode);
    moment.nothing = mode.free() && pause::nothing_now(m.overrides, &m.config.free_time);
    let always = sioul_core::everywhere::People::load(&sioul_core::everywhere::People::default_path());
    let area_of = |t: &Triaged| t.card.account.as_deref().and_then(|id| m.config.account(id)).and_then(|a| a.area.as_deref()).and_then(Area::parse).unwrap_or(Area::WORK);
    let mail: Vec<Triaged> = input.items.iter().filter(|t| attention.mail(t, &input.senders, &always, area_of(t), &moment).shown).cloned().collect();
    let shown = view::porch(&mail, m.config, input.store.as_ref(), m.tr, at, mode.quiet);
    if !shown.open {
        return PorchMade::said(match window::next_opening(&m.config.windows, at) {
            Some(next) => said(m.tr, "home-card-porch-opens", &[("when", when(m.tr, &next, at))]),
            None => m.tr.text("home-card-porch-closed", None),
        });
    }
    // The lanes the Porch shows unfolded; never what it folds (filed, less
    // important), whose words stay behind a click, and never what is set
    // aside, waits for your word on spam, or is hostile, even unfolded.
    let triaged: BTreeMap<String, &Triaged> = mail.iter().filter_map(|t| t.card.path.as_ref().map(|p| (p.display().to_string(), t))).collect();
    let mut listed: Vec<(i64, Mail)> = shown
        .lanes
        .iter()
        .filter(|l| !l.folded && !matches!(l.key.as_str(), "set-aside" | "review" | "hostile"))
        .flat_map(|l| l.items.iter())
        .filter(|i| !i.hidden && !i.spam)
        .map(|i| mail_line(m, i, triaged.get(&i.key).copied()))
        .collect();
    if listed.is_empty() {
        return PorchMade::said(m.tr.text("home-card-porch-empty", None));
    }
    // Details off: that mail waits, no one named, no count.
    if !m.details {
        return PorchMade::said(m.tr.text("home-card-mail-waits", None));
    }
    // The newest first, as a mail card lists them.
    listed.sort_by_key(|(sent, _)| std::cmp::Reverse(*sent));
    PorchMade { line: String::new(), mail: listed.into_iter().take(MAIL).map(|(_, mail)| mail).collect() }
}

/// A calendar's colour as "#rrggbb", whatever its tool wrote ("#4C6B5CFF");
/// "" when it gave none, or one that does not read.
fn colour(color: Option<&str>) -> String {
    let Some(color) = color.map(str::trim).filter(|c| c.starts_with('#')) else { return String::new() };
    let hex: String = color.chars().skip(1).take(6).collect();
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) { format!("#{}", hex.to_ascii_lowercase()) } else { String::new() }
}

/// An event's time, as an agenda card says it: "11:00 – 12:00"; ending
/// another day, "23:00 – Fri 9 Oct 01:00"; a whole day, nothing; several
/// whole days, "Until Fri 9 Oct".
fn event_time(tr: &Translator, event: &Occurrence, zone: &jiff::tz::TimeZone) -> String {
    let at = |seconds: i64| Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()));
    let (Some(start), Some(end)) = (at(event.start), at(event.end.max(event.start))) else { return String::new() };
    // The last moment it covers: an end at midnight belongs to the day before.
    let last = at(event.end.max(event.start + 1) - 1).map_or_else(|| start.date(), |z| z.date());
    if event.all_day {
        return if last > start.date() { said(tr, "home-card-until-day", &[("day", day_short(tr, last))]) } else { String::new() };
    }
    let from = start.strftime("%H:%M").to_string();
    if event.end <= event.start {
        return from;
    }
    let to = if last == start.date() { end.strftime("%H:%M").to_string() } else { tr.date(&end, true) };
    said(tr, "home-card-time-range", &[("start", from), ("end", to)])
}

/// A day of the agenda and its names: "Mon 12 Oct" until the day before,
/// "Tomorrow, Mon 12 Oct" that day, "Today, Mon 12 Oct" on the day; seen from `today`.
fn agenda_day(tr: &Translator, day: Date, today: Date, zone: &jiff::tz::TimeZone) -> Option<Day> {
    let midnight = |d: Date| d.to_zoned(zone.clone()).ok().map(|z| ms(&z));
    let (from, until) = (midnight(day)?, midnight(day.tomorrow().ok()?)?);
    let name = day_short(tr, day);
    let mut label = Vec::new();
    if day > today.tomorrow().ok()? {
        label.push(Label { until: midnight(day.yesterday().ok()?)?, text: name.clone() });
    }
    if day > today {
        label.push(Label { until: from, text: said(tr, "home-card-tomorrow", &[("day", name.clone())]) });
    }
    label.push(Label { until, text: said(tr, "home-card-today", &[("day", name)]) });
    Some(Day { from, until, label })
}

/// The coming appointments, from today to a month on: those not over yet,
/// cancelled ones left out, nearest first; each under its day (Java's), in
/// its calendar's colour. Details off, none named: "An event", its time.
/// None without a calendar.
fn agenda_lines(m: &Moment, input: &AgendaInput) -> Option<AgendaLines> {
    if !input.calendars {
        return None;
    }
    let zone = m.now.time_zone().clone();
    let today = m.now.date();
    let now = m.now.timestamp().as_second();
    let end = today.checked_add(Span::new().days(HORIZON_DAYS + 1)).ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map_or(i64::MAX, |z| z.timestamp().as_second());
    // An instant event still takes its minute.
    let mut coming: Vec<&Occurrence> = input.events.iter().filter(|e| !e.cancelled && e.end.max(e.start + 60) > now && e.start < end).collect();
    // Nearest first, a day's whole-day events before its times (Java's days only go forward).
    coming.sort_by(|a, b| (a.start, !a.all_day, &a.summary).cmp(&(b.start, !b.all_day, &b.summary)));
    let events = coming
        .into_iter()
        .take(EVENTS)
        .map(|e| Event {
            key: e.key.clone(),
            title: match e.summary.trim() {
                _ if !m.details => m.tr.text("home-card-event-plain", None),
                "" => m.tr.text("agenda-untitled", None),
                title => title.to_string(),
            },
            time: event_time(m.tr, e, &zone),
            all_day: e.all_day,
            start: e.start * 1000,
            end: e.end.max(e.start + 60) * 1000,
            color: colour(e.color.as_deref()),
        })
        .collect();
    let days = (0..=HORIZON_DAYS).filter_map(|i| today.checked_add(Span::new().days(i)).ok()).filter_map(|day| agenda_day(m.tr, day, today, &zone)).collect();
    Some(AgendaLines { empty: m.tr.text("home-card-agenda-empty", None), days, events })
}

/// Whether a step is for this time: work or admin time (or no hours set),
/// never asleep, in a pause, in free time, at leisure or during a meal.
fn time_for_a_step(mode: &Mode) -> bool {
    matches!(mode.time, Time::Work | Time::Admin | Time::Several(_) | Time::Any) && !mode.sleeps() && !mode.paused() && !mode.free()
}

/// The next step at `at`, as Now would pick it then: only in work or admin
/// time (or with no hours set), never asleep, in a pause, in free time, at
/// leisure or during a meal; offices open or not then, "Not now" and the
/// day's weather today, a new day starting clean. The page's own filters
/// (one kind, one category) are the page's, not the card's.
fn step_at(m: &Moment, input: &PlanInput, mode: &Mode, at: &Zoned) -> Option<Step> {
    if !time_for_a_step(mode) {
        return None;
    }
    let situation = quiet::Situation::now(m.config, m.overrides, m.blocks, at, m.tr, &input.loaded.projects);
    let filter = Filter { quiet: situation.quiet_tasks(), ..Filter::default() };
    let today = input.today.date == at.date().to_string();
    let (weather, aside) = if today { (input.today.weather, input.today.aside.clone()) } else { (Weather::Clear, BTreeSet::new()) };
    let cx = taskview::Context { filter: &filter, offices: situation.offices.clone(), tasks: &input.loaded.tasks, plan: &input.plan, today: at.date(), tr: m.tr, projects: &input.loaded.projects, spent: &input.spent, stopped: &input.stopped };
    let now = taskview::now(&cx, weather, &aside);
    let card = now.now?;
    if m.details {
        return Some(Step { title: card.title, why: now.why.first().cloned().unwrap_or_default(), uid: card.uid });
    }
    // Without its title, nor a reason that names another step: its date, else its length.
    Some(Step { title: m.tr.text("home-card-step-plain", None), why: if card.due.is_empty() { card.estimate } else { card.due }, uid: card.uid })
}

/// A code's kind, as the card's words name it.
fn kind_word(kind: CodeKind) -> &'static str {
    match kind {
        CodeKind::Code => "code",
        CodeKind::Password => "password",
        CodeKind::PasswordReset => "reset",
        CodeKind::SignInLink => "link",
        CodeKind::Confirmation => "confirm",
    }
}

/// The codes and links you asked sites for, still valid, newest first: each
/// as the Porch's "Right now" says it, its site and the code, with the
/// warning when its sender is not verified, until it expires (`porch::sent`
/// and its validity, as the Porch hides it). Details off, one line without
/// site or code: "A code waits on the Porch."
fn codes(m: &Moment, input: &PorchInput) -> Vec<Code> {
    let now = m.now.timestamp().as_second();
    let mut found: Vec<(i64, Code, CodeKind)> = input
        .items
        .iter()
        .filter(|t| t.lane == Lane::RightNow)
        .filter_map(|t| {
            let code = t.code.as_ref()?;
            let lasts = i64::from(code.lasts_minutes()) * 60;
            let sent = porch::sent(&t.card).unwrap_or(now);
            let until = sent + lasts;
            let line = m.tr.right_now(code.kind, t.card.sender(), t.trust, code.code.as_deref(), None);
            let warning = if t.trust == Trust::Verified { String::new() } else { m.tr.text("right-now-unverified", None) };
            (until > now).then(|| (sent, Code { line, warning, until: until * 1000 }, code.kind))
        })
        .collect();
    found.sort_by_key(|(sent, _, _)| std::cmp::Reverse(*sent));
    if m.details {
        return found.into_iter().map(|(_, code, _)| code).collect();
    }
    let last = found.iter().map(|(_, code, _)| code.until).max();
    found.first().zip(last).map(|((_, _, kind), until)| Code { line: said(m.tr, "home-card-code-plain", &[("kind", kind_word(*kind).to_string())]), warning: String::new(), until }).into_iter().collect()
}

/// Today's doses in words: "Magnesium · 300 mg, 12:30", and once that it was
/// not marked elsewhere is no longer known, "…: check before taking it."
/// Never "missed", "late" or "not taken" (docs/health.md, "Knowing"). None
/// with the details off: a medicine's name, even "a dose", says health to
/// whoever shares the launcher.
fn dose_lines(tr: &Translator, doses: &[Dose], details: bool) -> Vec<DoseLine> {
    if !details {
        return Vec::new();
    }
    doses
        .iter()
        .map(|d| {
            let pairs = [("dose", d.name.clone()), ("time", d.time.clone())];
            DoseLine { line: said(tr, "home-card-dose", &pairs), check: said(tr, "home-card-dose-check", &pairs), from: d.at * 1000, until: d.until * 1000, known_until: d.known_until * 1000 }
        })
        .collect()
}

/// How old the card is, said once it is: "at 14:05" until midnight,
/// "yesterday at 14:05" the next day, a short date after.
fn labels(tr: &Translator, made: &Zoned) -> Vec<Label> {
    let time = made.strftime("%H:%M").to_string();
    let first = midnight_after(made);
    let second = first.as_ref().and_then(midnight_after);
    vec![
        Label { until: first.as_ref().map_or(0, ms), text: said(tr, "home-card-when-at", &[("time", time.clone())]) },
        Label { until: second.as_ref().map_or(0, ms), text: said(tr, "home-card-when-yesterday", &[("time", time)]) },
        Label { until: 0, text: tr.date(made, true) },
    ]
}

/// What makes the Porch at `at` say what it says (`porch_at`): two frames
/// alike are made once. Its mail by the time it is for; open or closed, and
/// when it opens, from which day; resting after a pause.
fn porch_key(m: &Moment, mode: &Mode, at: &Zoned) -> String {
    let stamp = at.timestamp().as_second();
    let open = mode.quiet || m.config.windows.is_empty() || window::current(&m.config.windows, at).is_some();
    let opens = if open { 0 } else { window::next_opening(&m.config.windows, at).map_or(-1, |z| z.timestamp().as_second()) };
    format!("{}|{open}|{opens}|{}|{}", mode.time.id(), m.overrides.porch_rests(stamp), at.date())
}

/// The card, from now to the end of tomorrow, with no weather, reminder,
/// call or line left on where you stopped.
#[cfg(test)]
fn snapshot(m: &Moment, porch: Option<&PorchInput>, plan: Option<&PlanInput>, agenda: Option<&AgendaInput>, doses: &[Dose]) -> Snapshot {
    snapshot_with(m, porch, plan, agenda, doses, &More::default())
}

/// The card, from now to the end of tomorrow.
fn snapshot_with(m: &Moment, porch: Option<&PorchInput>, plan: Option<&PlanInput>, agenda: Option<&AgendaInput>, doses: &[Dose], more: &More) -> Snapshot {
    let set_up = m.config.every_account().next().is_some();
    let mut made: BTreeMap<String, PorchMade> = BTreeMap::new();
    // Each message written once, its place given to every frame that lists it.
    let mut mail: Vec<Mail> = Vec::new();
    let mut placed: BTreeMap<String, usize> = BTreeMap::new();
    let mut place = |message: &Mail| -> usize {
        if let Some(at) = placed.get(&message.key).filter(|_| !message.key.is_empty()) {
            return *at;
        }
        mail.push(message.clone());
        placed.insert(message.key.clone(), mail.len() - 1);
        mail.len() - 1
    };
    // Codes and doses as the matrix of what reaches you lets them come at each time.
    use sioul_core::attention::{Event, Kind as Own, Level, Now, Row};
    let attention = sioul_core::attention::Attention::of(m.config);
    let comes = |kind: Own, mode: &Mode| attention.level(Row::Own(kind), &Now::of(mode)) == Level::Now;
    // A reminder shows when its row lets it come or be seen then: not while it
    // waits for a later time (sleep, a pause, free time, work's while work rests).
    let reminders = reminder_lines(m.tr, more.reminders, m.details, m.now.timestamp().as_second());
    let shows = |work: bool, mode: &Mode| !matches!(attention.decide(&Event::own(Own::Dates).for_area(work.then_some(Area::WORK)), &Now::of(mode)).level, Level::Later | Level::Never);
    // Each call's line written once, in the Porch's words as seen from each frame's start.
    let mut calls: Vec<String> = Vec::new();
    let mut listed_calls: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut call_at = |mode: &Mode, from: &Zoned| -> Vec<usize> {
        let Some(lines_at) = more.calls else { return Vec::new() };
        if mode.paused() {
            return Vec::new();
        }
        let lines = listed_calls.entry(format!("{}|{}|{}", kind(mode), mode.time.id(), from.date())).or_insert_with(|| lines_at(&Now::of(mode), from)).clone();
        let lines = if m.details || lines.is_empty() { lines } else { vec![m.tr.text("home-card-calls-plain", None)] };
        lines
            .into_iter()
            .map(|line| match calls.iter().position(|c| *c == line) {
                Some(at) => at,
                None => {
                    calls.push(line);
                    calls.len() - 1
                }
            })
            .collect()
    };
    let frames: Vec<Frame> = timeline(m)
        .into_iter()
        .enumerate()
        .map(|(index, (from, until, mode))| {
            // Asleep, in a pause or in free time, the Porch says nothing on the card.
            let rests = mode.sleeps() || mode.paused() || mode.free();
            let porch = if rests {
                None
            } else if !set_up {
                Some(PorchMade::said(m.tr.text("home-card-setup", None)))
            } else {
                porch.map(|p| made.entry(porch_key(m, &mode, &from)).or_insert_with(|| porch_at(m, p, &mode, &from)).clone())
            };
            let kind = kind(&mode);
            let step = plan.and_then(|p| step_at(m, p, &mode, &from));
            Frame {
                from: ms(&from),
                until: ms(&until),
                kind: kind.to_string(),
                status: status(m, &mode, &from),
                dnd: m.dnd.map(|d| d.line_in(kind, ms(&from), index == 0)).unwrap_or_default(),
                porch: porch.map(|p| PorchLines { line: p.line, mail: p.mail.iter().map(&mut place).collect() }),
                // The pause's card shows the pause alone: a code or a dose
                // then comes by its own notification, as its row says.
                codes: !mode.paused() && comes(Own::Codes, &mode),
                doses: !mode.paused() && comes(Own::Doses, &mode),
                events: !mode.paused(),
                reminders: reminders.iter().enumerate().filter(|(_, (_, work))| !mode.paused() && shows(*work, &mode)).map(|(at, _)| at).collect(),
                calls: call_at(&mode, &from),
                overlaps: !rests,
                stopped: !rests && time_for_a_step(&mode),
                note: if set_up || rests { String::new() } else { m.tr.text("home-card-setup", None) },
                step,
            }
        })
        .collect();
    let made = ms(m.now);
    let end = frames.last().map_or(made, |f| f.until);
    let stopped = more.stopped.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|text| if m.details { said(m.tr, "home-card-stopped", &[("text", text.to_string())]) } else { m.tr.text("home-card-stopped-plain", None) });
    Snapshot {
        v: VERSION,
        made,
        details: m.details,
        stale_after: made + FRESH_MS,
        stale: labels(m.tr, m.now),
        beyond: m.tr.text("home-card-beyond", None),
        words: Words { mail: m.tr.text("home-card-mail", None), agenda: m.tr.text("home-card-agenda", None) },
        today: today_lines(m, &more.weather, end),
        frames,
        mail,
        agenda: agenda.and_then(|a| agenda_lines(m, a)),
        codes: porch.map(|p| codes(m, p)).unwrap_or_default(),
        doses: dose_lines(m.tr, doses, m.details),
        reminders: reminders.into_iter().map(|(line, _)| line).collect(),
        calls,
        overlaps: agenda.map(|a| overlap_lines(m, a)).unwrap_or_default(),
        stopped: stopped.unwrap_or_default(),
    }
}

/// What a card says, its age aside: written again unchanged, Android need not draw it again.
fn content(snapshot: &Snapshot) -> String {
    let mut same = snapshot.clone();
    same.made = 0;
    same.stale_after = 0;
    same.stale.clear();
    if let Some(first) = same.frames.first_mut() {
        first.from = 0;
    }
    serde_json::to_string(&same).unwrap_or_default()
}

// ---------------------------------------------------------------- written, and drawn

/// Written on Android; on a computer only when SIOUL_HOME_CARD is set.
fn on() -> bool {
    cfg!(target_os = "android") || std::env::var_os("SIOUL_HOME_CARD").is_some()
}

fn path() -> PathBuf {
    sioul_core::config::state_dir().join(FILE)
}

/// The details are on unless "Details on the home screen" was unticked on this device.
pub(crate) fn details() -> bool {
    !crate::work::WorkState::load().flags.get(PLAIN).copied().unwrap_or(false)
}

/// One writing at a time: the window's and a dose's alarm's.
static WRITING: Mutex<()> = Mutex::new(());

/// The card's text in place of the one before, whole (written beside, then renamed).
fn write(text: &str) -> bool {
    let path = path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let beside = path.with_extension("json.new");
    std::fs::write(&beside, text).and_then(|()| std::fs::rename(&beside, &path)).is_ok()
}

/// Android asked to draw the card again: an explicit broadcast to Sioul's
/// own receiver (android/main.cpp, the call the sync apps are asked with).
fn draw() {
    #[cfg(target_os = "android")]
    // SAFETY: three zero-terminated texts, valid for the call.
    unsafe {
        crate::backend::sioul_android_broadcast(c"com.aurelienpierre.sioul".as_ptr(), c"com.aurelienpierre.sioul.HomeCard".as_ptr(), c"com.aurelienpierre.sioul.action.HOME_CARD".as_ptr())
    };
}

/// What the card is made from, as the window last had it.
static PORCH: Mutex<Option<Arc<PorchInput>>> = Mutex::new(None);
static PLAN: Mutex<Option<Arc<PlanInput>>> = Mutex::new(None);

/// The card last written: what it says, when, and when its first frame ends (Unix seconds).
struct Written {
    content: String,
    made: i64,
    first_ends: i64,
}

static WRITTEN: Mutex<Option<Written>> = Mutex::new(None);

/// The Porch's messages the window just gathered, before the hours sort
/// them (`backend::compute`): the card follows.
pub(crate) fn porch_seen(items: &[Triaged], senders: &Senders, store: Option<&ProjectStore>) {
    if !on() {
        return;
    }
    *PORCH.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(PorchInput { items: items.to_vec(), senders: senders.clone(), store: store.cloned() }));
    soon(false);
}

/// The plan the window just made (`work::show_work`): the card follows.
pub(crate) fn plan_seen(loaded: &Arc<Loaded>, plan: &Plan, today: &Today, spent: &BTreeMap<String, u32>, stopped: &BTreeMap<String, String>) {
    if !on() {
        return;
    }
    *PLAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(PlanInput { loaded: Arc::clone(loaded), plan: plan.clone(), today: today.clone(), spent: spent.clone(), stopped: stopped.clone() }));
    soon(false);
}

/// The events as last read for the card, and what the window hides of them.
static AGENDA: Mutex<Option<Arc<AgendaInput>>> = Mutex::new(None);
/// Removed or skipped in the window, waiting for "Undo" to pass: left out.
type Hidden = (BTreeSet<PathBuf>, BTreeSet<(PathBuf, i64)>);
static HIDDEN: Mutex<Option<Hidden>> = Mutex::new(None);
/// Read the calendars again before the next card: an event changed.
static AGENDA_AGAIN: AtomicBool = AtomicBool::new(true);

/// The agenda read again by the window (`pim::show_pim`: a sync, an event
/// made, moved, removed or undone), with what it hides meanwhile: the card
/// reads the calendars again, and follows.
pub(crate) fn agenda_seen(removed: &BTreeSet<PathBuf>, skipped: &BTreeSet<(PathBuf, i64)>) {
    if !on() {
        return;
    }
    *HIDDEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((removed.clone(), skipped.clone()));
    AGENDA_AGAIN.store(true, Ordering::SeqCst);
    soon(false);
}

/// The events of today and the month to come, read from the calendars once,
/// then again when the window read the agenda again or the day changed.
fn agenda_input(now: &Zoned) -> Option<Arc<AgendaInput>> {
    let mut kept = AGENDA.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let again = AGENDA_AGAIN.swap(false, Ordering::SeqCst);
    if !again && let Some(input) = kept.as_ref().filter(|a| a.read_on == now.date()) {
        return Some(Arc::clone(input));
    }
    let midnight = now.date().to_zoned(now.time_zone().clone()).ok()?.timestamp().as_second();
    let reach = midnight + (HORIZON_DAYS + 1) * 86_400 + 3_600;
    let (removed, skipped) = HIDDEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone().unwrap_or_default();
    let events = sioul_core::agenda::occurrences(midnight, reach)
        .into_iter()
        .filter(|e| !removed.contains(std::path::Path::new(&e.key)) && !skipped.contains(&(PathBuf::from(&e.key), e.start)))
        .collect();
    let calendars = !sioul_core::vdir::collections(sioul_core::vdir::Kind::Calendars).is_empty();
    let aside = sioul_core::overlaps::SetAside::load(&sioul_core::overlaps::SetAside::default_path()).keys;
    let input = Arc::new(AgendaInput { events, calendars, read_on: now.date(), aside });
    *kept = Some(Arc::clone(&input));
    Some(input)
}

/// The reminders as last gathered (`remind::tick`, each minute), the events' left out.
static REMINDED: Mutex<Option<Arc<Vec<Reminder>>>> = Mutex::new(None);

/// The reminders the window just gathered, told or not (`reminders::gather`):
/// the card follows when those it may say changed.
pub(crate) fn reminders_seen(all: &[Reminder]) {
    if !on() {
        return;
    }
    use sioul_core::reminders::Kind;
    let kept: Vec<Reminder> = all.iter().filter(|r| !matches!(r.kind, Kind::Event | Kind::Before | Kind::Alarm)).cloned().collect();
    let mut seen = REMINDED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if seen.as_deref() == Some(&kept) {
        return;
    }
    *seen = Some(Arc::new(kept));
    drop(seen);
    soon(false);
}

/// A forecast fetched, or the place changed (`backend::update_weather`): the card follows.
pub(crate) fn weather_seen() {
    if on() {
        soon(false);
    }
}

/// The weather as the card says it: the place chosen, the forecast kept.
fn weather_input() -> WeatherInput {
    let config = load_config();
    let place = config.weather.place.as_deref().is_some_and(|p| !p.trim().is_empty()) && config.weather.latitude.is_some() && config.weather.longitude.is_some();
    let forecast = place.then(|| std::fs::read_to_string(crate::backend::weather_cache()).ok().and_then(|t| serde_json::from_str(&t).ok())).flatten();
    WeatherInput { place, forecast }
}

/// The window's minute: the card written again when its first frame ended,
/// or five minutes went by (its age said afresh).
pub(crate) fn tick() {
    if !on() {
        return;
    }
    let now = Timestamp::now().as_second();
    let due = WRITTEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().is_none_or(|w| now >= w.first_ends || now - w.made >= REWRITE_EVERY);
    if due {
        soon(false);
    }
}

/// Sioul put away, or a setting of the card changed: written now, and drawn.
pub(crate) fn now() {
    if on() {
        soon(true);
    }
}

/// The shared do-not-disturb's moment, as last seen in this process.
static DND: Mutex<Option<serde_json::Value>> = Mutex::new(None);

/// The do-not-disturb on every device, as its module reads it
/// (`everywhere::moment`: {on, why, until, until_at, line…}), handed after
/// each of its changes, here or from another device, and when first read in
/// a process. The same as last time: nothing. With the window's Porch and
/// plan, the card is made again; without (a receiver alone), the card on disk
/// gets its do-not-disturb lines alone, the rest as Sioul last saw it.
pub(crate) fn dnd_seen(moment: serde_json::Value) {
    if !on() {
        return;
    }
    {
        let mut kept = DND.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if kept.as_ref() == Some(&moment) {
            return;
        }
        *kept = Some(moment.clone());
    }
    let window = PORCH.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_some() && PLAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_some();
    if window {
        soon(true);
        return;
    }
    let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mut card) = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).filter(serde_json::Value::is_object) else { return };
    if patch_dnd(&mut card, Dnd::of(&moment).as_ref(), Timestamp::now().as_millisecond()) && write(&card.to_string()) {
        draw();
    }
}

/// The do-not-disturb lines of a card written before, said again: each
/// frame from now on as `Dnd::line_in` says it, a frame cut where it ends
/// (Unix ms `now`). Whether anything changed.
fn patch_dnd(card: &mut serde_json::Value, dnd: Option<&Dnd>, now: i64) -> bool {
    let Some(frames) = frames_of(card) else { return false };
    let before = frames.clone();
    // Cut the frame its end falls in: its line goes there.
    if let Some(end) = dnd.and_then(|d| d.until).filter(|u| *u > 0).map(|u| u * 1000)
        && let Some(at) = frames.iter().position(|f| f["from"].as_i64().is_some_and(|from| from < end) && f["until"].as_i64().is_some_and(|until| end < until))
    {
        let mut after = frames[at].clone();
        after["from"] = end.into();
        frames[at]["until"] = end.into();
        frames.insert(at + 1, after);
    }
    // The frame for now: the one now falls in, else the first (the clock set back).
    let current = frames.iter().position(|f| f["from"].as_i64().unwrap_or(0) <= now && now < f["until"].as_i64().unwrap_or(0)).unwrap_or(0);
    for (index, frame) in frames.iter_mut().enumerate() {
        let (kind, from) = (frame["kind"].as_str().unwrap_or_default().to_string(), frame["from"].as_i64().unwrap_or(0));
        frame["dnd"] = dnd.map(|d| d.line_in(&kind, from, index == current)).unwrap_or_default().into();
    }
    *frames != before
}

/// A written card's frames: "times" from version 3, "frames" before.
fn frames_of(card: &mut serde_json::Value) -> Option<&mut Vec<serde_json::Value>> {
    let key = if card["v"].as_u64().is_some_and(|v| v >= 3) { "times" } else { "frames" };
    card[key].as_array_mut()
}

/// Asked again while being made: once more after. Drawn when asked so once.
static AGAIN: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);
static TELL: AtomicBool = AtomicBool::new(false);

/// The card made on a thread of its own, several asks in a row making it once.
fn soon(tell: bool) {
    if tell {
        TELL.store(true, Ordering::SeqCst);
    }
    AGAIN.store(true, Ordering::SeqCst);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        loop {
            while AGAIN.swap(false, Ordering::SeqCst) {
                let tell = TELL.swap(false, Ordering::SeqCst);
                // A panic on bad data leaves the next card free to be made.
                let _ = std::panic::catch_unwind(|| refresh(tell));
            }
            RUNNING.store(false, Ordering::SeqCst);
            if !AGAIN.load(Ordering::SeqCst) || RUNNING.swap(true, Ordering::SeqCst) {
                break;
            }
        }
    });
}

/// The card made from what the window last had, written, and drawn when it
/// changed (or `tell`). Not before the window has both the Porch and the
/// plan in this process: a card made without them would lose its mail and
/// its step until Sioul comes back (put away at once after it opened, its
/// pages wait); the card written before stays, its age said.
fn refresh(tell: bool) {
    let porch = PORCH.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    let plan = PLAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    let (Some(porch), Some(plan)) = (porch, plan) else { return };
    let now = Zoned::now();
    let config = load_config();
    let overrides = Overrides::load(&Overrides::default_path());
    let blocks = crate::hours::blocks(&now);
    let details = details();
    let doses = if details { crate::health::card_doses(&now) } else { Vec::new() };
    let dnd = DND.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().and_then(Dnd::of);
    let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: tr(), details, dnd: dnd.as_ref() };
    let agenda = agenda_input(&now);
    let reminded = REMINDED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone().unwrap_or_default();
    let calls = crate::calls::for_card();
    let stopped = sioul_core::stopped::Stopped::load(&sioul_core::stopped::Stopped::default_path()).map(|s| s.text);
    let more = More { weather: weather_input(), reminders: &reminded, calls: calls.as_deref(), stopped };
    let card = snapshot_with(&moment, Some(&porch), Some(&plan), agenda.as_deref(), &doses, &more);
    let said = content(&card);
    let Ok(text) = serde_json::to_string(&card) else { return };
    let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut written = WRITTEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let changed = written.as_ref().is_none_or(|w| w.content != said);
    if !write(&text) {
        return;
    }
    let first_ends = card.frames.first().map_or(0, |f| f.until / 1000);
    *written = Some(Written { content: said, made: now.timestamp().as_second(), first_ends });
    drop(written);
    if changed || tell {
        draw();
    }
}

/// A dose marked or reminded outside the window (a dose's alarm, its "Taken":
/// `alarms`): the card's doses written again, the rest as Sioul last saw it.
/// No card yet, nothing.
pub(crate) fn doses_changed() {
    if !on() {
        return;
    }
    let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mut card) = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).filter(serde_json::Value::is_object) else { return };
    let details = details();
    let now = Zoned::now();
    let doses = if details { crate::health::card_doses(&now) } else { Vec::new() };
    let Ok(lines) = serde_json::to_value(dose_lines(tr(), &doses, details)) else { return };
    if card["doses"] == lines {
        return;
    }
    card["doses"] = lines;
    if write(&card.to_string()) {
        draw();
    }
}

/// New mail fetched outside the window (the phone's background service,
/// `steps`, while Sioul is closed or frozen): the card's messages and codes
/// made again from the Porch as it is now, the rest (its lines, the step, the
/// doses, the agenda) as Sioul last saw it. In the window's process the
/// window's own Porch follows (`porch_seen`): nothing here. No card yet, nothing.
pub(crate) fn mail_came() {
    if !on() || PORCH.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_some() {
        return;
    }
    let config = load_config();
    let now = Zoned::now();
    // The Porch as the window gathers it (`backend::World`): the projects and their ties, the senders let in.
    let ties = sioul_core::links::LocalLinks::load(&sioul_core::links::LocalLinks::default_path());
    let store = config.notes_root_path().and_then(|root| ProjectStore::load(&root).ok()).map(|s| s.with_ties(&ties));
    let known = porch::KnownSenders::load(&config.known_senders_path());
    let senders = Senders::load(&config);
    let state = sioul_core::state::PorchState::load(&sioul_core::state::PorchState::default_path());
    let items = porch::gather(&config.mail_sources(), store.as_ref(), &known, &senders, &state, now.timestamp().as_second());
    let input = PorchInput { items, senders, store };
    let overrides = Overrides::load(&Overrides::default_path());
    let blocks = crate::hours::blocks(&now);
    let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: tr(), details: details(), dnd: None };
    let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mut card) = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).filter(serde_json::Value::is_object) else { return };
    if patch_mail(&mut card, &moment, &input) && write(&card.to_string()) {
        draw();
    }
}

/// The messages and codes of a card written before, made again from `input`:
/// each frame's Porch as `snapshot` makes it at the frame's start; the frames
/// that say nothing of the Porch (asleep, a pause, free time) left so. Whether
/// anything changed.
fn patch_mail(card: &mut serde_json::Value, m: &Moment, input: &PorchInput) -> bool {
    if card["v"] != VERSION {
        return false;
    }
    let before = card.clone();
    let hours = m.config.week_hours();
    let zone = m.now.time_zone().clone();
    let mut made: BTreeMap<String, PorchMade> = BTreeMap::new();
    let mut mail: Vec<Mail> = Vec::new();
    let mut placed: BTreeMap<String, usize> = BTreeMap::new();
    let Some(frames) = frames_of(card) else { return false };
    for frame in frames.iter_mut().filter(|f| f["porch"].is_object()) {
        let Some(from) = frame["from"].as_i64().and_then(|at| Timestamp::from_millisecond(at).ok()).map(|t| t.to_zoned(zone.clone())) else { continue };
        let mode = quiet::mode(&hours, &m.config.time_off, m.overrides, m.blocks, &from);
        let porch = made.entry(porch_key(m, &mode, &from)).or_insert_with(|| porch_at(m, input, &mode, &from)).clone();
        let at: Vec<usize> = porch
            .mail
            .iter()
            .map(|message| match placed.get(&message.key).filter(|_| !message.key.is_empty()) {
                Some(at) => *at,
                None => {
                    mail.push(message.clone());
                    placed.insert(message.key.clone(), mail.len() - 1);
                    mail.len() - 1
                }
            })
            .collect();
        let Ok(lines) = serde_json::to_value(PorchLines { line: porch.line, mail: at }) else { return false };
        frame["porch"] = lines;
    }
    let (Ok(mail), Ok(codes)) = (serde_json::to_value(mail), serde_json::to_value(codes(m, input))) else { return false };
    card["mail"] = mail;
    card["codes"] = codes;
    *card != before
}

/// A tap on the card, kept by Java (HomeCardOpener.java) and taken once, as
/// main.qml's `openThing` takes it {kind, uri, key}: the Porch; a message, on
/// the Porch in its Reader ("card-mail", its file); Now on a step ("now", the
/// task's address); the Agenda; an event in it ("event", its file); what a
/// reminder is about (its task, budget, paper or contract). One older than
/// five minutes is let go.
pub(crate) fn opened() -> Option<(&'static str, String, String)> {
    let path = sioul_core::config::state_dir().join(OPENED);
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    tap(&text, Timestamp::now().as_millisecond())
}

/// A tap as Java writes it: {"open": "porch" | "now" | "mail" | "agenda" |
/// "event" | "reminder", "uid", "key", "at" (Unix ms)}. A message or an event
/// without its file opens its page; a reminder, what it is about ("key": its
/// address, `sioul:task/…`), else the Porch. The file is the window's to
/// check, as for any link.
fn tap(text: &str, now: i64) -> Option<(&'static str, String, String)> {
    let tap: serde_json::Value = serde_json::from_str(text).ok()?;
    let at = tap["at"].as_i64()?;
    if now - at > TAP_KEPT_MS || at - now > 60_000 {
        return None;
    }
    let key = tap["key"].as_str().unwrap_or_default().to_string();
    match tap["open"].as_str()? {
        "porch" => Some(("porch", String::new(), String::new())),
        "now" => Some(("now", tap["uid"].as_str().filter(|u| !u.is_empty()).map(sioul_core::links::task_uri).unwrap_or_default(), String::new())),
        "mail" if !key.is_empty() => Some(("card-mail", String::new(), key)),
        "mail" => Some(("porch", String::new(), String::new())),
        "event" if !key.is_empty() => Some(("event", String::new(), key)),
        "event" | "agenda" => Some(("agenda", String::new(), String::new())),
        "reminder" => Some(reminder_opens(&key)),
        _ => None,
    }
}

/// What a reminder's tap opens, as a reminder's notification does on a
/// computer (`remind::tick`): its task, its budget, its paper, its contract;
/// anything else, the Porch.
fn reminder_opens(target: &str) -> (&'static str, String, String) {
    if let Some(uid) = target.strip_prefix("sioul:task/") {
        ("task", target.to_string(), uid.to_string())
    } else if target.starts_with("sioul:budget/") {
        ("budget", target.to_string(), String::new())
    } else if let Some(id) = target.strip_prefix("sioul:paper/") {
        ("paper", target.to_string(), id.to_string())
    } else if let Some(id) = target.strip_prefix("sioul:contract/") {
        ("contract", target.to_string(), id.to_string())
    } else {
        ("porch", String::new(), String::new())
    }
}

/// "Details on the home screen", for Settings (on a phone): its row, in `group` and `section`.
pub(crate) fn setting(group: &str, section: &str, on: bool) -> sioul_core::settings::Setting {
    sioul_core::settings::Setting {
        key: "home_card_details".into(),
        kind: sioul_core::settings::Kind::Bool,
        label: tr().text("set-home-card-details", None),
        help: tr().text("set-home-card-details-help", None),
        value: sioul_core::config::SettingValue::Bool(on),
        choices: Vec::new(),
        rows: Vec::new(),
        min: 0.0,
        max: 0.0,
        step: 1.0,
        unit: String::new(),
        group: group.to_string(),
        section: section.to_string(),
    }
}

/// Whether the setting shows: on a phone (or with SIOUL_HOME_CARD set).
pub(crate) fn has_setting() -> bool {
    on()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::card::Card;
    use sioul_core::codes::OneTimeCode;
    use sioul_core::config::Priority;
    use sioul_core::needs::{Days, Needs};
    use sioul_core::plan::Settings;
    use sioul_core::reminders;
    use sioul_core::tasks::Task;

    /// Monday to Friday 09:00–17:00 at work, Tuesday 17:30–18:30 for admin.
    fn config() -> Config {
        toml::from_str(
            "[[account]]\nid = \"me\"\naddress = \"me@example.org\"\n\n\
             [[window]]\nday = \"monday\"\nstart = \"09:00\"\nend = \"17:00\"\n\n\
             [[window]]\nday = \"tuesday\"\nstart = \"09:00\"\nend = \"17:00\"\n\n\
             [[window]]\nday = \"wednesday\"\nstart = \"09:00\"\nend = \"17:00\"\n\n\
             [[window]]\nday = \"thursday\"\nstart = \"09:00\"\nend = \"17:00\"\n\n\
             [[window]]\nday = \"friday\"\nstart = \"09:00\"\nend = \"17:00\"\n\n\
             [[window]]\nday = \"tuesday\"\nstart = \"17:30\"\nend = \"18:30\"\nkind = \"admin\"\n",
        )
        .unwrap()
    }

    /// Three meals and the night (bed 23:00, winding down from 22:00, waking 07:00).
    fn blocks(now: &Zoned) -> Blocks {
        let needs = Needs { meals_on: true, sleep_on: true, ..Needs::default() };
        Blocks::of(&needs, &Days::default(), &[], now)
    }

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    /// A message come to the account "me", not read yet (in `new/`), its file named after its sender and subject.
    fn message(from: &str, subject: &str, lane: Lane) -> Triaged {
        let raw = format!("From: {from}\r\nSubject: {subject}\r\nDate: Mon, 05 Oct 2026 08:00:00 +0200\r\n\r\nHello,\r\n\r\n  the text   on several\r\nlines.\r\n");
        let mut card = Card::from_bytes(raw.as_bytes()).unwrap();
        let name: String = format!("{from}-{subject}").chars().filter(char::is_ascii_alphanumeric).collect();
        card.path = Some(PathBuf::from(format!("/mail/me/INBOX/new/1759600000.{name}.sioul")));
        card.account = Some("me".into());
        Triaged { card, lane, trust: Trust::Verified, code: None, reasons: Vec::new(), priority: Priority::Average, checks: None, assessment: None }
    }

    /// The same, come at `sent` (Unix seconds), read already when `read` (its file in `cur/`, flagged S).
    fn sent_at(from: &str, subject: &str, sent: i64, read: bool) -> Triaged {
        let mut letter = message(from, subject, Lane::People);
        letter.card.date = Some(sent);
        if read {
            let name = letter.card.path.as_ref().unwrap().file_name().unwrap().to_string_lossy().to_string();
            letter.card.path = Some(PathBuf::from(format!("/mail/me/INBOX/cur/{name}:2,S")));
        }
        letter
    }

    /// Four letters from people you know (safe senders: in every time but sleep), one code.
    fn porch_input(now: &Zoned) -> PorchInput {
        let mut items: Vec<Triaged> = [("Marie Dupont <marie@example.org>", "Dinner on Saturday"), ("Paul <paul@example.org>", "The photos"), ("Léa <lea@example.org>", "Sunday"), ("Tom <tom@example.org>", "A question")]
            .into_iter()
            .map(|(from, subject)| message(from, subject, Lane::People))
            .collect();
        let mut code = message("La Banque <no-reply@banque.example>", "Votre code", Lane::RightNow);
        code.code = Some(OneTimeCode { kind: CodeKind::Code, code: Some("482913".into()), expires_minutes: Some(10) });
        // Sent two minutes ago.
        code.card.date = Some(now.timestamp().as_second() - 120);
        items.push(code);
        // People you know, marked safe: their mail comes at every time but sleep (the usual matrix).
        let senders = Senders { safe: sioul_core::porch::SenderList::parse("marie@example.org\npaul@example.org\nlea@example.org\ntom@example.org\n"), ..Senders::default() };
        PorchInput { items, senders, store: None }
    }

    fn task(uid: &str, title: &str) -> Task {
        Task { uid: uid.into(), title: title.into(), estimate: 15, ..Task::default() }
    }

    fn plan_input(now: &Zoned) -> PlanInput {
        let mut form = task("form", "Call the CAF about the housing aid");
        form.due = "2026-10-30".into();
        // For work: work's hours bring it, admin's (they are set) do not.
        form.area = "work".into();
        let tasks = vec![form];
        let plan = sioul_core::plan::plan(&tasks, now.date(), &Settings::default(), &BTreeMap::new(), &BTreeSet::new());
        PlanInput { loaded: Arc::new(Loaded { tasks, ..Loaded::default() }), plan, today: Today { date: now.date().to_string(), ..Today::default() }, spent: BTreeMap::new(), stopped: BTreeMap::new() }
    }

    struct Made {
        card: Snapshot,
    }

    impl Made {
        /// The frame showing at `when`.
        fn at(&self, when: &str) -> &Frame {
            let t = at(when).timestamp().as_millisecond();
            self.card.frames.iter().find(|f| f.from <= t && t < f.until).unwrap_or_else(|| panic!("no frame at {when}: {:#?}", self.card.frames))
        }
    }

    /// The calendars' events, as the card reads them (`agenda_input`), from
    /// `(title, start, end, all day, colour, cancelled)` in Paris; one
    /// calendar, "Personal".
    fn events_of(events: &[(&str, &str, &str, bool, Option<&str>, bool)]) -> AgendaInput {
        let events = events
            .iter()
            .enumerate()
            .map(|(n, (title, start, end, all_day, color, cancelled))| Occurrence {
                key: format!("/calendars/me/personal/event-{n}.ics"),
                summary: title.to_string(),
                start: at(start).timestamp().as_second(),
                end: at(end).timestamp().as_second(),
                all_day: *all_day,
                calendar: "Personal".into(),
                color: color.map(str::to_string),
                cancelled: *cancelled,
                ..Occurrence::default()
            })
            .collect();
        AgendaInput { events, calendars: true, read_on: Date::constant(2026, 10, 5), aside: BTreeSet::new() }
    }

    /// A dentist tomorrow, a whole day on Monday next week, a trip over three days, a call that was cancelled.
    fn some_events() -> AgendaInput {
        events_of(&[
            ("Dentist", "2026-10-06T11:00[Europe/Paris]", "2026-10-06T12:00[Europe/Paris]", false, Some("#4C6B5CFF"), false),
            ("Choir", "2026-10-05T19:00[Europe/Paris]", "2026-10-05T20:30[Europe/Paris]", false, None, false),
            ("Neighbours' party", "2026-10-12T00:00[Europe/Paris]", "2026-10-13T00:00[Europe/Paris]", true, Some("#b07d3f"), false),
            ("Trip to the sea", "2026-10-14T00:00[Europe/Paris]", "2026-10-17T00:00[Europe/Paris]", true, None, false),
            ("Night train", "2026-10-20T22:30[Europe/Paris]", "2026-10-21T06:45[Europe/Paris]", false, None, false),
            ("A call", "2026-10-07T09:00[Europe/Paris]", "2026-10-07T09:30[Europe/Paris]", false, None, true),
        ])
    }

    fn make(now: &str, overrides: &Overrides, details: bool, language: &str, doses: &[Dose]) -> Made {
        let now = at(now);
        let config = config();
        let tr = Translator::new(language);
        let blocks = blocks(&now);
        let moment = Moment { now: &now, config: &config, overrides, blocks: &blocks, tr: &tr, details, dnd: None };
        let porch = porch_input(&now);
        let plan = plan_input(&now);
        Made { card: snapshot(&moment, Some(&porch), Some(&plan), Some(&some_events()), doses) }
    }

    impl Made {
        /// The messages a frame lists: their senders.
        fn senders(&self, frame: &Frame) -> Vec<&str> {
            frame.porch.iter().flat_map(|p| p.mail.iter()).map(|at| self.card.mail[*at].sender.as_str()).collect()
        }
    }

    /// Every word the card can show in `frame`, in one text: its lines, its
    /// messages, its step, the agenda's days and events.
    fn words(card: &Snapshot, frame: &Frame) -> String {
        let mut out = vec![frame.status.clone(), frame.note.clone(), card.today.line.clone()];
        out.extend(card.today.date.iter().map(|l| l.text.clone()));
        if let Some(p) = &frame.porch {
            out.push(p.line.clone());
            for mail in p.mail.iter().map(|at| &card.mail[*at]) {
                out.extend([mail.sender.clone(), mail.subject.clone(), mail.text.clone()]);
                out.extend(mail.when.iter().map(|l| l.text.clone()));
            }
        }
        if let Some(s) = &frame.step {
            out.extend([s.title.clone(), s.why.clone()]);
        }
        if let Some(agenda) = card.agenda.as_ref().filter(|_| frame.events) {
            out.push(agenda.empty.clone());
            out.extend(agenda.days.iter().flat_map(|d| d.label.iter()).map(|l| l.text.clone()));
            out.extend(agenda.events.iter().flat_map(|e| [e.title.clone(), e.time.clone()]));
        }
        out.extend(card.words.mail.lines().chain(card.words.agenda.lines()).map(str::to_string));
        out.join("\n")
    }

    #[test]
    fn each_time_says_its_own() {
        // Monday 5 October 2026, 10:00 in Paris: work until 12:10 (lunch is got ready from then).
        let made = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), true, "en", &[]);
        let work = made.at("2026-10-05T10:00[Europe/Paris]");
        assert_eq!((work.kind.as_str(), work.status.as_str()), ("work", "Work until 12:10."));
        let porch = work.porch.as_ref().unwrap();
        // Four letters and a code came: the letters listed, no sentence above them (the
        // code has its own line); come at the same time, in the Porch's order.
        assert_eq!(porch.line, "");
        assert_eq!(made.senders(work), ["Marie Dupont", "Paul", "Léa", "Tom"]);
        let first = &made.card.mail[porch.mail[0]];
        assert_eq!((first.subject.as_str(), first.text.as_str()), ("Dinner on Saturday", "Hello, the text on several lines."));
        let step = work.step.as_ref().unwrap();
        assert_eq!((step.title.as_str(), step.why.as_str(), step.uid.as_str()), ("Call the CAF about the housing aid", "Its date is 30 October: three weeks left.", "form"));
        assert!(work.codes && work.doses);
        // The code, with its site, as "Right now" says it, until it expires (ten minutes from its sending).
        assert_eq!(made.card.codes.len(), 1);
        assert_eq!(made.card.codes[0].line, "Code from La Banque (verified): 482913");
        assert_eq!(made.card.codes[0].warning, "");
        assert_eq!(made.card.codes[0].until, at("2026-10-05T10:08[Europe/Paris]").timestamp().as_millisecond());
        // Lunch: its line, no step; the Porch as at meals (people you know are safe: they come).
        let meal = made.at("2026-10-05T12:30[Europe/Paris]");
        assert_eq!((meal.kind.as_str(), meal.status.as_str()), ("meals", "Meal until 13:00."));
        assert!(meal.step.is_none());
        // Work again, then leisure in the evening: no step.
        assert_eq!(made.at("2026-10-05T15:00[Europe/Paris]").status, "Work until 17:00.");
        let evening = made.at("2026-10-05T17:30[Europe/Paris]");
        assert_eq!((evening.kind.as_str(), evening.status.as_str()), ("leisure", "Leisure until 19:00: what you enjoy."));
        assert!(evening.step.is_none() && evening.porch.is_some());
        // Winding down from 22:00, then the night from bedtime: the sleep's line alone, as the window says it;
        // doses remind during sleep by default, and codes you asked for come at once asleep too
        // (docs/attention.md, Q6; before, they waited on the Porch).
        assert_eq!(made.at("2026-10-05T22:15[Europe/Paris]").status, "Winding down: nothing disturbs until tomorrow at 07:00.");
        let night = made.at("2026-10-05T23:30[Europe/Paris]");
        assert_eq!((night.kind.as_str(), night.status.as_str()), ("sleep", "Sleep: nothing disturbs until tomorrow at 07:00."));
        assert!(night.porch.is_none() && night.step.is_none() && night.codes && night.doses);
        // Past midnight "tomorrow" is today: a frame ends at each midnight.
        assert!(made.card.frames.iter().any(|f| f.from == at("2026-10-06T00:00[Europe/Paris]").timestamp().as_millisecond()));
        assert_eq!(made.at("2026-10-06T05:00[Europe/Paris]").status, "Sleep: nothing disturbs until 07:00.");
        // Tuesday: work, then admin hours from 17:30, with no step: the CAF is work's.
        let admin = made.at("2026-10-06T17:45[Europe/Paris]");
        assert_eq!((admin.kind.as_str(), admin.status.as_str()), ("admin", "Admin time until 18:30: offices, bills, letters."));
        assert!(admin.step.is_none());
        assert!(made.at("2026-10-06T09:30[Europe/Paris]").step.is_some());
        // The frames end with tomorrow: Wednesday is not given.
        let last = made.card.frames.last().unwrap();
        assert_eq!(last.until, at("2026-10-07T00:00[Europe/Paris]").timestamp().as_millisecond());
        // In French, with its typography.
        let fr = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), true, "fr", &[]);
        assert_eq!(fr.at("2026-10-06T05:00[Europe/Paris]").status, "Sommeil\u{202f}: rien ne dérange jusqu’à 07:00.");
        assert_eq!(fr.at("2026-10-05T23:30[Europe/Paris]").status, "Sommeil\u{202f}: rien ne dérange jusqu’à demain à 07:00.");
        assert_eq!(fr.at("2026-10-05T10:00[Europe/Paris]").status, "Travail jusqu’à 12:10.");
    }

    #[test]
    fn a_pause_says_its_line_and_nothing_else() {
        let paused = Overrides { paused_since: Some(at("2026-10-05T09:30[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let made = make("2026-10-05T10:00[Europe/Paris]", &paused, true, "en", &[dose("Magnesium · 300 mg", "09:45", "2026-10-05T09:45[Europe/Paris]", 0)]);
        let frame = made.at("2026-10-05T10:00[Europe/Paris]");
        assert_eq!((frame.kind.as_str(), frame.status.as_str()), ("paused", "Paused."));
        assert!(frame.porch.is_none() && frame.step.is_none() && !frame.codes && !frame.doses && !frame.events);
        // The pause lasts until you come back: one frame per day, all paused.
        assert!(made.card.frames.iter().all(|f| f.kind == "paused"));
        // Free time: its line; codes and doses reach you, as it says; no Porch, no step; the agenda stays.
        let free = Overrides { free_since: Some(at("2026-10-05T14:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let made = make("2026-10-05T14:30[Europe/Paris]", &free, true, "en", &[]);
        let frame = made.at("2026-10-05T14:30[Europe/Paris]");
        assert_eq!(frame.kind, "free-time");
        assert_eq!(frame.status, "Free time: only your safe senders, doses and codes reach you. Work comes back when you do.");
        assert!(frame.porch.is_none() && frame.step.is_none() && frame.codes && frame.doses && frame.events);
        // It ends at the night's start: winding down then.
        assert_eq!(made.at("2026-10-05T22:30[Europe/Paris]").kind, "sleep");
    }

    #[test]
    fn the_porch_keeps_its_hours() {
        // After a pause the Porch rests until its next hours: no mail named, when it opens.
        let rests = Overrides { porch_rests_until: Some(at("2026-10-06T09:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let made = make("2026-10-05T15:00[Europe/Paris]", &rests, true, "en", &[]);
        let frame = made.at("2026-10-05T15:00[Europe/Paris]");
        let porch = frame.porch.as_ref().unwrap();
        assert_eq!(porch.line, "After the pause, the Porch opens tomorrow at 09:00.");
        assert!(porch.mail.is_empty());
        // The next morning, open again: the letters listed.
        assert_eq!(made.at("2026-10-06T09:30[Europe/Paris]").porch.as_ref().unwrap().mail.len(), 4);
        // Work now outside the hours: work's time, but the Porch keeps its own (closed until 09:00).
        let work_now = Overrides { work_now: Some(at("2026-10-05T20:30[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let made = make("2026-10-05T17:30[Europe/Paris]", &work_now, true, "en", &[]);
        let frame = made.at("2026-10-05T17:30[Europe/Paris]");
        assert_eq!(frame.status, "Work shown until 20:30, by your choice.");
        let porch = frame.porch.as_ref().unwrap();
        assert_eq!(porch.line, "The Porch opens tomorrow at 09:00.");
        assert!(porch.mail.is_empty(), "never mail waiting for later");
        // Dinner comes first, then work again until 20:30.
        assert_eq!(made.at("2026-10-05T19:30[Europe/Paris]").kind, "meals");
        assert_eq!(made.at("2026-10-05T20:15[Europe/Paris]").status, "Work shown until 20:30, by your choice.");
        // Strangers' mail waits for work and admin time: in leisure the Porch names none of it.
        let config = config();
        let tr = Translator::new("en");
        let now = at("2026-10-05T17:30[Europe/Paris]");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let mut input = porch_input(&now);
        input.items = vec![message("A stranger <someone@elsewhere.example>", "Offer", Lane::Screener)];
        let card = snapshot(&moment, Some(&input), None, None, &[]);
        let evening = card.frames.iter().find(|f| f.kind == "leisure").unwrap();
        assert_eq!(evening.porch.as_ref().unwrap().line, "Nothing waits on the Porch.");
        assert!(evening.porch.as_ref().unwrap().mail.is_empty());
        let work = card.frames.iter().find(|f| f.kind == "work").unwrap();
        assert_eq!(card.mail[work.porch.as_ref().unwrap().mail[0]].sender, "A stranger");
    }

    #[test]
    fn mail_is_listed_as_a_mail_card_lists_it() {
        let now = at("2026-10-05T10:00[Europe/Paris]");
        let stamp = |text: &str| at(text).timestamp().as_second();
        let mut config = config();
        config.accounts.push(toml::from_str::<Config>("[[account]]\nid = \"club\"\naddress = \"me@club.example\"\n").unwrap().accounts.remove(0));
        let tr = Translator::new("en");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let mut input = porch_input(&now);
        let mut club = sent_at("Ana <ana@example.org>", "Rehearsal moved", stamp("2026-10-05T09:12[Europe/Paris]"), false);
        club.card.account = Some("club".into());
        // Not shown on the card, even when the Porch would unfold them: set aside, waiting for your word on spam, hostile.
        let mut forged = sent_at("Bank <bank@bank.example>", "Your account", stamp("2026-10-05T09:50[Europe/Paris]"), false);
        forged.lane = Lane::SetAside;
        forged.priority = Priority::Above;
        let mut doubtful = sent_at("Offers <deals@shop.example>", "Big sale", stamp("2026-10-05T09:55[Europe/Paris]"), false);
        doubtful.lane = Lane::Review;
        input.items = vec![
            sent_at("Marie Dupont <marie@example.org>", "Dinner on Saturday", stamp("2026-10-04T18:30[Europe/Paris]"), true),
            club,
            sent_at("Paul <paul@example.org>", "The photos", stamp("2026-09-28T08:00[Europe/Paris]"), true),
            sent_at("Léa <lea@example.org>", "", stamp("2025-12-20T08:00[Europe/Paris]"), false),
            forged,
            doubtful,
        ];
        input.senders = Senders { safe: sioul_core::porch::SenderList::parse("marie@example.org\npaul@example.org\nlea@example.org\nana@example.org\n"), ..Senders::default() };
        let card = snapshot(&moment, Some(&input), None, None, &[]);
        let frame = &card.frames[0];
        let listed: Vec<&Mail> = frame.porch.as_ref().unwrap().mail.iter().map(|at| &card.mail[*at]).collect();
        // The newest first; the set-aside and the review queue's left out.
        assert_eq!(listed.iter().map(|m| m.sender.as_str()).collect::<Vec<_>>(), ["Ana", "Marie Dupont", "Paul", "Léa"]);
        // Its time today, until midnight; then "Yesterday"; then its day.
        let midnight = at("2026-10-06T00:00[Europe/Paris]").timestamp().as_millisecond();
        let next = at("2026-10-07T00:00[Europe/Paris]").timestamp().as_millisecond();
        let when = |m: &Mail| m.when.iter().map(|l| (l.until, l.text.clone())).collect::<Vec<_>>();
        assert_eq!(when(listed[0]), [(midnight, "09:12".to_string()), (next, "Yesterday".to_string()), (0, "5 Oct".to_string())]);
        assert_eq!(when(listed[1]), [(midnight, "Yesterday".to_string()), (0, "4 Oct".to_string())]);
        assert_eq!(when(listed[2]), [(0, "28 Sep".to_string())]);
        // Another year's: with its year.
        assert_eq!(when(listed[3]), [(0, "20 Dec 2025".to_string())]);
        // Not read yet (new/, or no S): bold in Java; read: not.
        assert_eq!(listed.iter().map(|m| m.unread).collect::<Vec<_>>(), [true, false, false, true]);
        // The account's place among yours: its colour in Java.
        assert_eq!((listed[0].account, listed[1].account), (1, 0));
        // No subject: said so.
        assert_eq!(listed[3].subject, "(no subject)");
        // Its file, to open it from the card.
        assert!(listed[0].key.ends_with(".sioul") && listed[0].key.contains("/new/"));
        // In French: "hier", "4 oct.", "1er".
        let fr = Translator::new("fr");
        let first = sent_at("Ana <ana@example.org>", "Hello", stamp("2026-10-01T09:00[Europe/Paris]"), false);
        assert_eq!(mail_when(&fr, first.card.date, &now).last().unwrap().text, "1er oct.");
        assert_eq!(mail_when(&fr, Some(stamp("2026-10-04T09:00[Europe/Paris]")), &now)[0].text, "hier");
        // At most twenty, the newest.
        let mut many = porch_input(&now);
        many.items = (0..30).map(|i| sent_at(&format!("Someone <s{i}@example.org>"), &format!("Note {i}"), stamp("2026-10-05T08:00[Europe/Paris]") - i * 60, false)).collect();
        many.senders = Senders { safe: sioul_core::porch::SenderList::parse("*@example.org\n"), ..Senders::default() };
        let card = snapshot(&moment, Some(&many), None, None, &[]);
        let shown = &card.frames[0].porch.as_ref().unwrap().mail;
        assert_eq!(shown.len(), 20);
        assert_eq!(card.mail[shown[0]].subject, "Note 0");
        // Each message written once, whichever frames list it.
        assert_eq!(card.mail.len(), 20);
    }

    #[test]
    fn details_off_names_no_one() {
        let doses = [dose("Magnesium · 300 mg", "09:45", "2026-10-05T09:45[Europe/Paris]", i64::MAX / 2000)];
        let made = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), false, "en", &doses);
        assert!(!made.card.details);
        let frame = made.at("2026-10-05T10:00[Europe/Paris]");
        assert_eq!(frame.status, "Work until 12:10.");
        // That mail waits, no one named, no count.
        let porch = frame.porch.as_ref().unwrap();
        assert_eq!(porch.line, "Mail waits on the Porch.");
        assert!(porch.mail.is_empty() && made.card.mail.is_empty());
        // The step without its title, nor a reason naming another step: its date.
        let step = frame.step.as_ref().unwrap();
        assert_eq!((step.title.as_str(), step.why.as_str()), ("Your next step", "By 30 October: three weeks left"));
        // A code, without its site or the code itself.
        assert_eq!(made.card.codes.len(), 1);
        assert_eq!(made.card.codes[0].line, "A code waits on the Porch.");
        // No dose line: a medicine's name says health.
        assert!(made.card.doses.is_empty());
        // The appointments at their times, none named.
        let agenda = made.card.agenda.as_ref().unwrap();
        assert!(agenda.events.iter().all(|e| e.title == "An event"));
        assert_eq!(agenda.events.iter().find(|e| e.start == at("2026-10-06T11:00[Europe/Paris]").timestamp().as_millisecond()).unwrap().time, "11:00 – 12:00");
        let all = serde_json::to_string(&made.card).unwrap();
        for private in ["Marie", "Dinner", "482913", "La Banque", "CAF", "Magnesium", "Dentist", "Choir", "party", "Trip"] {
            assert!(!all.contains(private), "{private} shown with the details off: {all}");
        }
        let fr = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), false, "fr", &[]);
        assert_eq!(fr.card.codes[0].line, "Un code attend sur le Porche.");
        assert_eq!(fr.at("2026-10-05T10:00[Europe/Paris]").porch.as_ref().unwrap().line, "Du courrier attend sur le Porche.");
        assert!(fr.card.agenda.as_ref().unwrap().events.iter().all(|e| e.title == "Un événement"));
    }

    #[test]
    fn the_agenda_comes_day_by_day() {
        let made = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), true, "en", &[]);
        let ms = |text: &str| at(text).timestamp().as_millisecond();
        let agenda = made.card.agenda.as_ref().unwrap();
        // Nearest first; the cancelled call left out.
        let titles: Vec<&str> = agenda.events.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Choir", "Dentist", "Neighbours' party", "Trip to the sea", "Night train"]);
        let event = |title: &str| agenda.events.iter().find(|e| e.title == title).unwrap();
        // A time range; another day's end with its day; a whole day, its title alone; several, until when.
        assert_eq!(event("Dentist").time, "11:00 – 12:00");
        assert_eq!(event("Night train").time, "22:30 – Wed 21 Oct 06:45");
        assert_eq!((event("Neighbours' party").time.as_str(), event("Neighbours' party").all_day), ("", true));
        assert_eq!(event("Trip to the sea").time, "Until Fri 16 Oct");
        // Its calendar's colour, as "#rrggbb" whatever was written; none: Sioul's own.
        assert_eq!((event("Dentist").color.as_str(), event("Choir").color.as_str(), event("Neighbours' party").color.as_str()), ("#4c6b5c", "", "#b07d3f"));
        assert_eq!((event("Dentist").start, event("Dentist").end), (ms("2026-10-06T11:00[Europe/Paris]"), ms("2026-10-06T12:00[Europe/Paris]")));
        assert_eq!(event("Dentist").key, "/calendars/me/personal/event-0.ics");
        // Every day from today to a month on, each named as seen from the day before and on the day.
        assert_eq!(agenda.days.len(), 31);
        let names = |day: &Day| day.label.iter().map(|l| (l.until, l.text.clone())).collect::<Vec<_>>();
        assert_eq!(agenda.days[0].from, ms("2026-10-05T00:00[Europe/Paris]"));
        assert_eq!(names(&agenda.days[0]), [(ms("2026-10-06T00:00[Europe/Paris]"), "Today, Mon 5 Oct".to_string())]);
        assert_eq!(names(&agenda.days[1]), [(ms("2026-10-06T00:00[Europe/Paris]"), "Tomorrow, Tue 6 Oct".to_string()), (ms("2026-10-07T00:00[Europe/Paris]"), "Today, Tue 6 Oct".to_string())]);
        assert_eq!(
            names(&agenda.days[7]),
            [(ms("2026-10-11T00:00[Europe/Paris]"), "Mon 12 Oct".to_string()), (ms("2026-10-12T00:00[Europe/Paris]"), "Tomorrow, Mon 12 Oct".to_string()), (ms("2026-10-13T00:00[Europe/Paris]"), "Today, Mon 12 Oct".to_string())]
        );
        // The change of hour on 25 October: that day lasts 25 hours.
        let long = agenda.days.iter().find(|d| d.from == ms("2026-10-25T00:00[Europe/Paris]")).unwrap();
        assert_eq!(long.until - long.from, 25 * 3_600_000);
        assert_eq!(agenda.empty, "Nothing in your calendars for the coming month.");
        // Shown at every time but in a pause.
        assert!(made.card.frames.iter().all(|f| f.events));
        // An event over is left out; one running stays.
        let later = make("2026-10-05T19:30[Europe/Paris]", &Overrides::default(), true, "en", &[]);
        assert_eq!(later.card.agenda.as_ref().unwrap().events[0].title, "Choir");
        let after = make("2026-10-05T21:00[Europe/Paris]", &Overrides::default(), true, "en", &[]);
        assert_eq!(after.card.agenda.as_ref().unwrap().events[0].title, "Dentist");
        // Beyond a month: not listed.
        let config = config();
        let tr = Translator::new("en");
        let now = at("2026-10-05T10:00[Europe/Paris]");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let far = events_of(&[("Far away", "2026-11-20T10:00[Europe/Paris]", "2026-11-20T11:00[Europe/Paris]", false, None, false)]);
        assert!(snapshot(&moment, None, None, Some(&far), &[]).agenda.unwrap().events.is_empty());
        // No calendar at all: no agenda on the card.
        let none = AgendaInput { events: Vec::new(), calendars: false, read_on: now.date(), aside: BTreeSet::new() };
        assert!(snapshot(&moment, None, None, Some(&none), &[]).agenda.is_none());
        // In French, as the phone's agenda says it.
        let fr = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), true, "fr", &[]);
        let agenda = fr.card.agenda.as_ref().unwrap();
        assert_eq!(names(&agenda.days[0])[0].1, "Aujourd’hui, lun. 5 oct.");
        assert_eq!(names(&agenda.days[3])[1].1, "Demain, jeu. 8 oct.");
        assert_eq!(names(&agenda.days[7])[0].1, "lun. 12 oct.");
        assert_eq!(agenda.events.iter().find(|e| e.title == "Trip to the sea").unwrap().time, "Jusqu’au ven. 16 oct.");
        assert_eq!(agenda.events.iter().find(|e| e.title == "Night train").unwrap().time, "22:30 – mer. 21 oct. 06:45");
        assert_eq!(fr.card.words.mail, "Courrier");
    }

    #[test]
    fn no_count_at_all() {
        let now = at("2026-10-05T10:00[Europe/Paris]");
        let config = config();
        let overrides = Overrides::default();
        let blocks = blocks(&now);
        // A number anywhere but in a time (14:05) or a date: what a badge would be.
        let digits = regex_free_digits;
        for language in ["en", "fr"] {
            for n in [0, 1, 7, 12, 30] {
                for details in [true, false] {
                    let tr = Translator::new(language);
                    let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details, dnd: None };
                    let mut input = porch_input(&now);
                    input.items = (0..n).map(|i| message(&format!("Someone <s{i}@example.org>"), "Hello", Lane::People)).collect();
                    input.senders = Senders { safe: sioul_core::porch::SenderList::parse("*@example.org\n"), ..Senders::default() };
                    let card = snapshot(&moment, Some(&input), Some(&plan_input(&now)), Some(&some_events()), &[]);
                    for frame in &card.frames {
                        let shown = words(&card, frame);
                        assert!(!digits(&shown), "a number on the card: {shown}");
                        // Never how many came, in words either.
                        for count in ["letters came", "letter came", "lettres sont arrivées", "lettre est arrivée", "Many", "Beaucoup", "Seven", "Sept", "Twelve", "Douze"] {
                            assert!(!shown.contains(count), "a count on the card: {shown}");
                        }
                    }
                }
            }
        }
    }

    /// Whether `text` holds a digit outside a time ("14:05") or a date ("30 October", "5 Oct", "1er oct.").
    fn regex_free_digits(text: &str) -> bool {
        let mut rest = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            // A time: two digits, ':', two digits.
            if chars[i].is_ascii_digit() && chars.get(i + 1).is_some_and(char::is_ascii_digit) && chars.get(i + 2) == Some(&':') && chars.get(i + 3).is_some_and(char::is_ascii_digit) && chars.get(i + 4).is_some_and(char::is_ascii_digit) {
                i += 5;
                continue;
            }
            rest.push(chars[i]);
            i += 1;
        }
        // A day of the month before its name, long or short: "30 October", "30 octobre", "5 Oct", "1er oct.".
        let months = [
            "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December", "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre", "Jan", "Feb", "Mar",
            "Apr", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec", "janv.", "févr.", "avr.", "juil.", "sept.", "oct.", "nov.", "déc.",
        ];
        let mut cleaned = rest.clone();
        for month in months {
            for day in (1..=31).rev() {
                cleaned = cleaned.replace(&format!("{day} {month}"), month).replace(&format!("{day}\u{a0}{month}"), month).replace(&format!("{day}er {month}"), month);
            }
        }
        cleaned.chars().any(|c| c.is_ascii_digit())
    }

    fn dose(name: &str, time: &str, when: &str, known_until: i64) -> Dose {
        let due = at(when).timestamp().as_second();
        Dose { name: name.into(), time: time.into(), at: due, until: due + 12 * 3600, known_until }
    }

    #[test]
    fn doses_say_what_is_known_and_no_more() {
        let due = at("2026-10-05T09:45[Europe/Paris]").timestamp().as_second();
        // Known not marked until 10:05 (another device heard at 10:00, five minutes' freshness).
        let doses = [dose("Magnesium · 300 mg", "09:45", "2026-10-05T09:45[Europe/Paris]", due + 20 * 60), dose("Levothyroxine · 75 µg", "06:00", "2026-10-06T06:00[Europe/Paris]", 0)];
        let made = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), true, "en", &doses);
        let lines = &made.card.doses;
        assert_eq!(lines[0].line, "Magnesium · 300 mg, 09:45");
        assert_eq!(lines[0].check, "Magnesium · 300 mg, 09:45: check before taking it.");
        assert_eq!((lines[0].from, lines[0].known_until), (due * 1000, (due + 20 * 60) * 1000));
        // Not known from its time on: the check, at once (Java shows `check` when known_until is 0).
        assert_eq!(lines[1].known_until, 0);
        for line in lines {
            for word in [&line.line, &line.check] {
                let lower = word.to_lowercase();
                for never in ["missed", "late", "not taken", "overdue", "forgot"] {
                    assert!(!lower.contains(never), "{word}");
                }
            }
        }
        // Asleep: the dose shows (doses remind during sleep), not in a pause.
        assert!(made.at("2026-10-06T05:00[Europe/Paris]").doses);
        // Doses silent in sleep, as an older Sioul said it ([reminders] doses_in_sleep, read once to seed the matrix).
        let mut silent = config();
        silent.reminders.doses_in_sleep = false;
        let now = at("2026-10-05T10:00[Europe/Paris]");
        let tr = Translator::new("en");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &silent, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let card = snapshot(&moment, None, None, None, &doses);
        let night = card.frames.iter().find(|f| f.kind == "sleep").unwrap();
        assert!(!night.doses, "doses during sleep: stay silent");
        assert!(night.codes, "codes you asked for come at once while you sleep (Q6; before, they waited on the Porch)");
        // As the matrix of what reaches you says (Settings ▸ What reaches you): doses held in sleep, codes on the Porch only then.
        let mut grid = config();
        grid.attention = toml::from_str::<Config>("[attention]\ndoses = [\"sleep:later\"]\ncodes = [\"sleep:never\"]\n").unwrap().attention;
        let card = snapshot(&Moment { config: &grid, ..moment }, None, None, None, &doses);
        let night = card.frames.iter().find(|f| f.kind == "sleep").unwrap();
        assert!(!night.doses && !night.codes);
        // In French, with its typography.
        let fr = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), true, "fr", &doses);
        assert_eq!(fr.card.doses[0].check, "Magnesium · 300 mg, 09:45\u{202f}: vérifiez avant de la prendre.");
    }

    #[test]
    fn its_age_is_said_once_it_is_old() {
        let made = make("2026-10-05T14:05[Europe/Paris]", &Overrides::default(), true, "en", &[]);
        let card = &made.card;
        assert_eq!(card.stale_after, card.made + 15 * 60 * 1000);
        assert_eq!(card.stale[0].text, "at 14:05");
        assert_eq!(card.stale[0].until, at("2026-10-06T00:00[Europe/Paris]").timestamp().as_millisecond());
        assert_eq!(card.stale[1].text, "yesterday at 14:05");
        assert_eq!(card.stale[1].until, at("2026-10-07T00:00[Europe/Paris]").timestamp().as_millisecond());
        assert_eq!((card.stale[2].until, card.stale[2].text.as_str()), (0, "Mon 5 Oct 14:05"));
        assert_eq!(card.beyond, "Open Sioul to bring this card up to date.");
        let fr = make("2026-10-05T14:05[Europe/Paris]", &Overrides::default(), true, "fr", &[]);
        assert_eq!((fr.card.stale[0].text.as_str(), fr.card.stale[1].text.as_str()), ("à 14:05", "hier à 14:05"));
        // Written again a minute later, nothing new: said the same, Android need not draw it again.
        let config = config();
        let tr = Translator::new("en");
        let overrides = Overrides::default();
        let first = at("2026-10-05T14:05[Europe/Paris]");
        let (porch, plan) = (porch_input(&first), plan_input(&first));
        let card_at = |now: &Zoned| {
            let blocks = blocks(now);
            let moment = Moment { now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
            snapshot(&moment, Some(&porch), Some(&plan), Some(&some_events()), &[])
        };
        let (before, after) = (card_at(&first), card_at(&at("2026-10-05T14:06[Europe/Paris]")));
        assert_ne!(before.made, after.made);
        assert_eq!(content(&before), content(&after));
        // A new letter: said differently.
        let mut more = porch_input(&first);
        more.items.push(message("Ana <ana@example.org>", "Tea?", Lane::People));
        let blocks = blocks(&first);
        let moment = Moment { now: &first, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        assert_ne!(content(&before), content(&snapshot(&moment, Some(&more), Some(&plan), Some(&some_events()), &[])));
    }

    #[test]
    fn not_set_up_yet() {
        let now = at("2026-10-05T10:00[Europe/Paris]");
        let config = Config { accounts: Vec::new(), ..config() };
        let tr = Translator::new("en");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let card = snapshot(&moment, Some(&porch_input(&now)), None, None, &[]);
        assert_eq!(card.frames[0].porch.as_ref().unwrap().line, "Sioul is not set up yet.");
        // Asleep it says only the night's line, set up or not.
        assert!(card.frames.iter().filter(|f| f.kind == "sleep").all(|f| f.porch.is_none()));
    }

    /// The names HomeCard.java and HomeCardRows.java read, each where they
    /// read it: a name changed here and not there would leave the card blank.
    /// With SIOUL_CARD_SAMPLE set, the card is written there too (in French
    /// with SIOUL_CARD_SAMPLE_FR), to check the Java's rows on a computer:
    /// Thursday 8 October 2026 at 09:20, with the weather, reminders, a call,
    /// two events at once and a line on where you stopped.
    #[test]
    fn the_file_java_reads() {
        let doses = [dose("Magnesium · 300 mg", "09:45", "2026-10-08T09:45[Europe/Paris]", 0)];
        let made = full("2026-10-08T09:20[Europe/Paris]", true, "en", &doses);
        let json = serde_json::to_value(&made).unwrap();
        for (variable, language) in [("SIOUL_CARD_SAMPLE", "en"), ("SIOUL_CARD_SAMPLE_FR", "fr")] {
            if let Some(path) = std::env::var_os(variable) {
                let card = full("2026-10-08T09:20[Europe/Paris]", true, language, &doses);
                std::fs::write(path, serde_json::to_string_pretty(&card).unwrap()).unwrap();
            }
        }
        for key in ["made", "stale_after"] {
            assert!(json[key].is_i64(), "{key}");
        }
        assert!(json["details"].is_boolean() && json["beyond"].is_string());
        assert!(json["stale"][0]["until"].is_i64() && json["stale"][0]["text"].is_string());
        // Version 3: the frames under "times", none under "frames" (an older Java says `beyond`).
        assert!(json["frames"].is_null());
        let frame = &json["times"][0];
        for key in ["from", "until"] {
            assert!(frame[key].is_i64(), "{key}");
        }
        assert!(frame["status"].is_string() && frame["codes"].is_boolean() && frame["doses"].is_boolean() && frame["events"].is_boolean());
        assert!(frame["porch"]["line"].is_string() && frame["porch"]["mail"][0].is_u64());
        for key in ["title", "why", "uid"] {
            assert!(frame["step"][key].is_string(), "{key}");
        }
        assert!(frame["reminders"][0].is_u64() && frame["calls"][0].is_u64() && frame["overlaps"].is_boolean() && frame["stopped"].is_boolean() && frame["note"].is_string());
        assert!(json["v"] == 3 && json["words"]["mail"].is_string() && json["words"]["agenda"].is_string());
        // Today (HomeCard.java): the date's wordings, the weather hour by hour, the days, the credit.
        let today = &json["today"];
        assert!(today["date"][0]["until"].is_i64() && today["date"][0]["text"].is_string() && today["line"].is_string());
        let hour = &today["weather"]["hours"][0];
        assert!(hour["from"].is_i64() && hour["until"].is_i64());
        for slot in [&hour["now"], &hour["next"][0], &hour["parts"][0]] {
            for key in ["label", "icon", "temperature", "rain", "words"] {
                assert!(slot[key].is_string(), "{key}");
            }
        }
        let day = &today["weather"]["days"][0];
        assert!(day["from"].is_i64() && day["until"].is_i64());
        for key in ["label", "icon", "high", "low", "rain", "words"] {
            assert!(day[key].is_string(), "{key}");
        }
        assert!(today["weather"]["credit"].is_string());
        // What the Porch has (HomeCard.java).
        for key in ["line", "open"] {
            assert!(json["reminders"][0][key].is_string(), "{key}");
        }
        assert!(json["reminders"][0]["from"].is_i64() && json["reminders"][0]["until"].is_i64());
        assert!(json["calls"][0].is_string() && json["stopped"].is_string());
        assert!(json["overlaps"][0]["line"].is_string() && json["overlaps"][0]["from"].is_i64() && json["overlaps"][0]["until"].is_i64());
        // The messages (HomeCardRows.java).
        let mail = &json["mail"][0];
        for key in ["key", "sender", "subject", "text"] {
            assert!(mail[key].is_string(), "{key}");
        }
        assert!(mail["unread"].is_boolean() && mail["account"].is_i64());
        assert!(mail["when"][0]["until"].is_i64() && mail["when"][0]["text"].is_string());
        // The agenda (HomeCardRows.java).
        let agenda = &json["agenda"];
        assert!(agenda["empty"].is_string());
        assert!(agenda["days"][0]["from"].is_i64() && agenda["days"][0]["until"].is_i64() && agenda["days"][0]["label"][0]["until"].is_i64() && agenda["days"][0]["label"][0]["text"].is_string());
        let event = &agenda["events"][0];
        for key in ["key", "title", "time", "color"] {
            assert!(event[key].is_string(), "{key}");
        }
        assert!(event["all_day"].is_boolean() && event["start"].is_i64() && event["end"].is_i64());
        assert!(json["codes"][0]["line"].is_string() && json["codes"][0]["warning"].is_string() && json["codes"][0]["until"].is_i64());
        for key in ["from", "until", "known_until"] {
            assert!(json["doses"][0][key].is_i64(), "{key}");
        }
        assert!(json["doses"][0]["line"].is_string() && json["doses"][0]["check"].is_string());
        assert!(frame["dnd"].is_string());
        // Asleep: no Porch, no step, written as null (Java's optJSONObject).
        let night = serde_json::to_value(Made { card: made }.at("2026-10-08T23:30[Europe/Paris]")).unwrap();
        assert!(night["porch"].is_null() && night["step"].is_null());
    }

    #[test]
    fn the_do_not_disturb_everywhere_says_its_line() {
        let now = at("2026-10-05T14:00[Europe/Paris]");
        let config = config();
        let tr = Translator::new("en");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let until = at("2026-10-05T15:00[Europe/Paris]").timestamp().as_second();
        let line = "Do not disturb, on every device, until 15:00.";
        let moment = serde_json::json!({ "on": true, "why": "manual", "until": "15:00", "until_at": until, "line": line, "everywhere": true, "here": true });
        let dnd = Dnd::of(&moment).unwrap();
        let (porch, plan) = (porch_input(&now), plan_input(&now));
        let card_with = |dnd: Option<&Dnd>| {
            let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd };
            snapshot(&moment, Some(&porch), Some(&plan), None, &[])
        };
        let made = Made { card: card_with(Some(&dnd)) };
        // Under the status line until 15:00, where a frame ends; gone after.
        assert_eq!(made.at("2026-10-05T14:30[Europe/Paris]").dnd, line);
        assert_eq!(made.at("2026-10-05T14:30[Europe/Paris]").until, at("2026-10-05T15:00[Europe/Paris]").timestamp().as_millisecond());
        assert_eq!(made.at("2026-10-05T15:00[Europe/Paris]").dnd, "");
        // Off, or for a reason the status line says already (sleep, the pause, free time): nothing.
        for (on, why) in [(false, "manual"), (true, "sleep"), (true, "paused"), (true, "free-time")] {
            assert_eq!(Dnd::of(&serde_json::json!({ "on": on, "why": why, "line": line, "until_at": until })), None, "{on} {why}");
        }
        // A focus session's, its end no promise: the frame for now only.
        let focus = Dnd::of(&serde_json::json!({ "on": true, "why": "focus", "line": line, "until_at": 0 })).unwrap();
        assert_eq!(focus.until, None);
        // Until switched off: in every frame but the night's.
        let lasting = Dnd::of(&serde_json::json!({ "on": true, "why": "manual", "line": line, "until_at": 0 })).unwrap();
        let card = card_with(Some(&lasting));
        assert!(card.frames.iter().all(|f| (f.kind == "sleep") == f.dnd.is_empty()), "{:#?}", card.frames);
        // In a pause, its line alone.
        let paused = Overrides { paused_since: Some(now.timestamp().as_second() - 60), ..Overrides::default() };
        let moment = Moment { now: &now, config: &config, overrides: &paused, blocks: &blocks, tr: &tr, details: true, dnd: Some(&lasting) };
        assert!(snapshot(&moment, Some(&porch), Some(&plan), None, &[]).frames.iter().all(|f| f.dnd.is_empty()));
        // Its end not said: in the frame for now only.
        let unsaid = Dnd::of(&serde_json::json!({ "on": true, "why": "manual", "line": line })).unwrap();
        let card = card_with(Some(&unsaid));
        assert_eq!(card.frames[0].dnd, line);
        assert!(card.frames[1..].iter().all(|f| f.dnd.is_empty()));
        // Without the window (a receiver alone): the card on disk gets its lines, a frame cut at its end.
        let mut written = serde_json::to_value(card_with(None)).unwrap();
        let before = written["times"].as_array().unwrap().len();
        let stamp = now.timestamp().as_millisecond();
        assert!(patch_dnd(&mut written, Some(&dnd), stamp));
        let frames = written["times"].as_array().unwrap().clone();
        assert_eq!(frames.len(), before + 1, "cut at 15:00");
        let shown = |when: &str| {
            let t = at(when).timestamp().as_millisecond();
            frames.iter().find(|f| f["from"].as_i64().unwrap() <= t && t < f["until"].as_i64().unwrap()).unwrap()["dnd"].clone()
        };
        assert_eq!(shown("2026-10-05T14:59[Europe/Paris]"), line);
        assert_eq!(shown("2026-10-05T15:01[Europe/Paris]"), "");
        // Said again the same: nothing changes; switched off: the lines go.
        assert!(!patch_dnd(&mut written, Some(&dnd), stamp));
        assert!(patch_dnd(&mut written, None, stamp));
        assert!(written["times"].as_array().unwrap().iter().all(|f| f["dnd"] == ""));
        // A card an older Sioul wrote (version 2, its frames under "frames"): its lines said again all the same.
        let mut older = serde_json::json!({ "v": 2, "frames": [{ "from": 0, "until": i64::MAX, "kind": "work", "dnd": "" }] });
        assert!(patch_dnd(&mut older, Some(&lasting), stamp));
        assert_eq!(older["frames"][0]["dnd"], line);
    }

    /// Open-Meteo's answer for Geneva, asked on Thursday 8 October 2026 at 09:29 (weather.rs's tests).
    const SAVED: &str = include_str!("../../sioul-core/tests/fixtures/weather/open-meteo.json");

    /// The forecast kept, fetched a minute before `now`.
    fn forecast_at(now: &Zoned) -> Forecast {
        forecast::parse(SAVED, now.timestamp().as_second() - 60).unwrap()
    }

    /// A reminder of `kind`, due at `at` until `until` (Paris times).
    fn reminder(kind: reminders::Kind, key: &str, at_: &str, until: &str, title: &str, body: &str, target: &str, work: bool) -> Reminder {
        Reminder { key: key.into(), kind, at: at(at_).timestamp().as_second(), until: at(until).timestamp().as_second(), title: title.into(), body: body.into(), target: target.into(), work, starts: None }
    }

    /// A date asked for a work task, a payment, a paper from tomorrow, and an
    /// event's reminder before it (the agenda card's and Android's: not said here).
    fn some_reminders() -> Vec<Reminder> {
        use reminders::Kind;
        vec![
            reminder(Kind::Asked, "asked:form", "2026-10-08T09:00[Europe/Paris]", "2026-10-31T00:00[Europe/Paris]", "Asked for Friday 30 October", "Call the CAF about the housing aid", "sioul:task/form", true),
            reminder(Kind::Payment, "payment:rent", "2026-10-08T09:00[Europe/Paris]", "2026-10-10T00:00[Europe/Paris]", "Rent · €650", "Planned for Friday 9 October; the account will hold it.", "sioul:budget/home", false),
            reminder(Kind::Paper, "paper:passport", "2026-10-09T09:00[Europe/Paris]", "2026-11-02T00:00[Europe/Paris]", "Passport: valid until Sunday 1 November", "Renewing takes weeks (an appointment, then the making): a step now?", "sioul:paper/passport", false),
            reminder(Kind::Before, "before:dentist", "2026-10-08T10:45[Europe/Paris]", "2026-10-08T11:00[Europe/Paris]", "11:00 · Dentist, in 15 minutes", "", "/calendars/me/personal/event-0.ics", false),
        ]
    }

    /// The calls declined as the Porch would list them: one, from the night
    /// before, its caller let through in work and leisure time, not asleep.
    fn some_calls(moment: &sioul_core::attention::Now, at_: &Zoned) -> Vec<String> {
        use sioul_core::attention::Column;
        if moment.is(Column::Sleep) || moment.is(Column::Pause) {
            return Vec::new();
        }
        let day = if at_.date() == Date::constant(2026, 10, 8) { "While you slept" } else { "Yesterday, while you slept" };
        vec![format!("{day}, Marie Dupont called at 07:40.")]
    }

    /// The full card at `now` (a Thursday, 8 October 2026), as a phone shows it:
    /// the weather at a place chosen, the reminders, a call declined, two events
    /// at once at 11:00 and on Friday, a line left on where you stopped.
    fn full(now: &str, details: bool, language: &str, doses: &[Dose]) -> Snapshot {
        let now = at(now);
        let config = config();
        let tr = Translator::new(language);
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details, dnd: None };
        let agenda = events_of(&[
            ("Dentist", "2026-10-08T11:00[Europe/Paris]", "2026-10-08T12:00[Europe/Paris]", false, None, false),
            ("School meeting", "2026-10-08T11:30[Europe/Paris]", "2026-10-08T12:30[Europe/Paris]", false, None, false),
            ("Choir", "2026-10-09T19:00[Europe/Paris]", "2026-10-09T20:30[Europe/Paris]", false, None, false),
            ("Neighbours' party", "2026-10-09T20:00[Europe/Paris]", "2026-10-09T23:00[Europe/Paris]", false, None, false),
            ("Trip to the sea", "2026-10-14T00:00[Europe/Paris]", "2026-10-17T00:00[Europe/Paris]", true, None, false),
        ]);
        let reminded = some_reminders();
        let more = More { weather: WeatherInput { place: true, forecast: Some(forecast_at(&now)) }, reminders: &reminded, calls: Some(&some_calls), stopped: Some("Halfway through the CAF's form, page 2".into()) };
        snapshot_with(&moment, Some(&porch_input(&now)), Some(&plan_input(&now)), Some(&agenda), doses, &more)
    }

    #[test]
    fn today_comes_first_hour_by_hour() {
        let card = full("2026-10-08T09:20[Europe/Paris]", true, "en", &[]);
        let ms = |text: &str| at(text).timestamp().as_millisecond();
        // The date, then tomorrow's from midnight: the frames end with tomorrow.
        let dates: Vec<(i64, &str)> = card.today.date.iter().map(|l| (l.until, l.text.as_str())).collect();
        assert_eq!(dates, [(ms("2026-10-09T00:00[Europe/Paris]"), "Thursday 8 October"), (ms("2026-10-10T00:00[Europe/Paris]"), "Friday 9 October")]);
        assert_eq!(card.today.line, "");
        let weather = card.today.weather.as_ref().unwrap();
        assert_eq!(weather.credit, "Weather: Open-Meteo.com");
        // Hour by hour from the hour under way to the end of the last frame (Saturday's midnight).
        assert_eq!(weather.hours.first().unwrap().from, ms("2026-10-08T09:00[Europe/Paris]"));
        assert_eq!(weather.hours.last().unwrap().until, card.frames.last().unwrap().until);
        assert!(weather.hours.windows(2).all(|w| w[0].until == w[1].from), "no hole between the hours");
        // Each hour says its own now, its next four hours, its next parts of the day; every
        // hour of every frame has its four (the forecast holds three days).
        assert!(weather.hours.iter().all(|h| h.next.len() == 4 && h.parts.len() == 4), "{:#?}", weather.hours.iter().map(|h| (h.from, h.next.len(), h.parts.len())).collect::<Vec<_>>());
        let hour = |text: &str| weather.hours.iter().find(|h| h.from <= ms(text) && ms(text) < h.until).unwrap();
        let morning = hour("2026-10-08T09:20[Europe/Paris]");
        assert_eq!((morning.now.temperature.as_str(), morning.now.rain.as_str()), ("16°", "98 %"));
        assert_eq!(morning.next.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["10:00", "11:00", "12:00", "13:00"]);
        assert_eq!(morning.parts.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["Afternoon", "Evening", "Night", "Morning"]);
        let afternoon = hour("2026-10-08T14:30[Europe/Paris]");
        assert_eq!((afternoon.now.temperature.as_str(), afternoon.next[0].label.as_str()), ("14°", "15:00"));
        // The afternoon's last hours are among the four: the evening comes first.
        assert_eq!(afternoon.parts.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["Evening", "Night", "Morning", "Afternoon"]);
        let late = hour("2026-10-09T22:10[Europe/Paris]");
        assert_eq!(late.next.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["23:00", "00:00", "01:00", "02:00"]);
        // The days, each from its midnight to the next: Java shows the seven after today.
        assert_eq!(weather.days.len(), 9);
        assert_eq!((weather.days[1].from, weather.days[1].until), (ms("2026-10-09T00:00[Europe/Paris]"), ms("2026-10-10T00:00[Europe/Paris]")));
        assert_eq!((weather.days[1].day.label.as_str(), weather.days[1].day.high.as_str(), weather.days[1].day.low.as_str()), ("Fri", "17°", "13°"));
        // In French, the heading with its capital.
        let fr = full("2026-10-08T09:20[Europe/Paris]", true, "fr", &[]);
        assert_eq!(fr.today.date[0].text, "Jeudi 8 octobre");
        assert_eq!(fr.today.weather.as_ref().unwrap().credit, "Météo\u{202f}: Open-Meteo.com");
        // Details off: the weather is no detail, said all the same.
        let plain = full("2026-10-08T09:20[Europe/Paris]", false, "en", &[]);
        assert_eq!(plain.today, card.today);
    }

    #[test]
    fn no_place_no_weather() {
        let now = at("2026-10-08T09:20[Europe/Paris]");
        let config = config();
        let tr = Translator::new("en");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let with = |weather: WeatherInput| snapshot_with(&moment, None, None, None, &[], &More { weather, ..More::default() }).today;
        // No place chosen: no weather part, one quiet line saying where to choose it.
        let none = with(WeatherInput::default());
        assert_eq!((none.weather.is_none(), none.line.as_str()), (true, "Choose a place for the weather in Sioul: tap the weather in its status line."));
        assert_eq!(none.date[0].text, "Thursday 8 October");
        // A place, but no forecast yet, or one kept from two days ago: none yet.
        for forecast in [None, Some(forecast::parse(SAVED, now.timestamp().as_second() - 2 * 86_400).unwrap())] {
            let shown = with(WeatherInput { place: true, forecast });
            assert_eq!((shown.weather.is_none(), shown.line.as_str()), (true, "No forecast yet."));
        }
        // In French, with its typography.
        let fr = Translator::new("fr");
        let moment = Moment { tr: &fr, ..moment };
        let said = snapshot_with(&moment, None, None, None, &[], &More::default()).today.line;
        assert_eq!(said, "Choisissez un lieu pour la météo dans Sioul\u{202f}: touchez la météo dans sa ligne d’état.");
    }

    #[test]
    fn the_porch_lines_but_no_mail() {
        let card = full("2026-10-08T09:20[Europe/Paris]", true, "en", &[dose("Magnesium · 300 mg", "09:45", "2026-10-08T09:45[Europe/Paris]", 0)]);
        let made = Made { card };
        let card = &made.card;
        // The reminders of dates, waits, payments and papers, soonest first; never an event's (Android tells those).
        let lines: Vec<&str> = card.reminders.iter().map(|r| r.line.as_str()).collect();
        assert_eq!(lines, ["Call the CAF about the housing aid · Asked for Friday 30 October", "Rent · €650 · Planned for Friday 9 October; the account will hold it.", "Passport: valid until Sunday 1 November"]);
        assert_eq!((card.reminders[0].open.as_str(), card.reminders[2].from), ("sioul:task/form", at("2026-10-09T09:00[Europe/Paris]").timestamp().as_millisecond()));
        // At work, all three (Java shows each from its time); in the evening, work's
        // date waits for work; asleep, none; their row says so.
        assert_eq!(made.at("2026-10-08T10:00[Europe/Paris]").reminders, [0, 1, 2]);
        assert_eq!(made.at("2026-10-08T17:30[Europe/Paris]").reminders, [1, 2]);
        assert!(made.at("2026-10-08T23:30[Europe/Paris]").reminders.is_empty());
        // The call declined, as the Porch lists it, said from each day.
        let work = made.at("2026-10-08T10:00[Europe/Paris]");
        assert_eq!(card.calls[work.calls[0]], "While you slept, Marie Dupont called at 07:40.");
        assert_eq!(card.calls[made.at("2026-10-09T10:00[Europe/Paris]").calls[0]], "Yesterday, while you slept, Marie Dupont called at 07:40.");
        assert!(made.at("2026-10-08T23:30[Europe/Paris]").calls.is_empty());
        // Two events at once: today's, then Friday's on Friday; never asleep.
        let overlaps: Vec<(&str, i64)> = card.overlaps.iter().map(|o| (o.line.as_str(), o.from)).collect();
        assert_eq!(
            overlaps,
            [
                ("Today, two events at once: Dentist and School meeting.", at("2026-10-08T00:00[Europe/Paris]").timestamp().as_millisecond()),
                ("Today, two events at once: Choir and Neighbours' party.", at("2026-10-09T00:00[Europe/Paris]").timestamp().as_millisecond()),
            ]
        );
        assert_eq!(card.overlaps[0].until, at("2026-10-08T12:30[Europe/Paris]").timestamp().as_millisecond());
        assert!(work.overlaps && !made.at("2026-10-08T23:30[Europe/Paris]").overlaps);
        // Now: the step and where you stopped, in work time; not in the evening.
        assert_eq!(card.stopped, "Where you stopped: Halfway through the CAF's form, page 2");
        assert!(work.stopped && work.step.is_some());
        assert!(!made.at("2026-10-08T17:30[Europe/Paris]").stopped);
        // Nothing of the mail on the full card: its lines name no sender and no subject
        // (the mail card lists them, from the same file).
        for frame in &card.frames {
            let shown = full_card_words(card, frame);
            for mail in ["Marie Dupont <", "Dinner on Saturday", "The photos", "Paul", "Léa", "A question"] {
                assert!(!shown.contains(mail), "{mail} on the full card: {shown}");
            }
        }
        assert!(!card.mail.is_empty(), "the mail card's messages are still written");
        // Details off: none named.
        let plain = full("2026-10-08T09:20[Europe/Paris]", false, "en", &[]);
        assert!(plain.reminders.iter().all(|r| r.line == "A reminder waits in Sioul."));
        assert_eq!(plain.calls, ["A call was declined: it is on the Porch."]);
        assert_eq!(plain.overlaps[0].line, "Today, two events at once.");
        assert_eq!(plain.stopped, "Your line on where you stopped waits in Sioul.");
        let all = serde_json::to_string(&serde_json::json!([plain.reminders, plain.calls, plain.overlaps, plain.stopped])).unwrap();
        for private in ["CAF", "Rent", "Passport", "Marie", "Dentist", "Choir", "Halfway"] {
            assert!(!all.contains(private), "{private} shown with the details off: {all}");
        }
        // In French.
        let fr = full("2026-10-08T09:20[Europe/Paris]", true, "fr", &[]);
        assert_eq!(fr.overlaps[0].line, "Aujourd’hui, deux événements en même temps\u{202f}: Dentist et School meeting.");
        assert_eq!(fr.stopped, "Où vous en étiez\u{202f}: Halfway through the CAF's form, page 2");
        // In a pause: none of it.
        let now = at("2026-10-08T10:00[Europe/Paris]");
        let config = config();
        let tr = Translator::new("en");
        let blocks = blocks(&now);
        let paused = Overrides { paused_since: Some(now.timestamp().as_second() - 60), ..Overrides::default() };
        let moment = Moment { now: &now, config: &config, overrides: &paused, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let reminded = some_reminders();
        let card = snapshot_with(&moment, None, None, None, &[], &More { reminders: &reminded, calls: Some(&some_calls), stopped: Some("Halfway".into()), ..More::default() });
        assert!(card.frames.iter().all(|f| f.reminders.is_empty() && f.calls.is_empty() && !f.overlaps && !f.stopped));
    }

    /// Every word the full card can show in `frame` (HomeCard.java): today, the
    /// status line, the Porch's lines, Now.
    fn full_card_words(card: &Snapshot, frame: &Frame) -> String {
        let mut out: Vec<String> = card.today.date.iter().map(|l| l.text.clone()).collect();
        out.extend([card.today.line.clone(), frame.status.clone(), frame.dnd.clone(), frame.note.clone(), card.stopped.clone()]);
        out.extend(card.codes.iter().flat_map(|c| [c.line.clone(), c.warning.clone()]));
        out.extend(card.doses.iter().flat_map(|d| [d.line.clone(), d.check.clone()]));
        out.extend(frame.reminders.iter().map(|at| card.reminders[*at].line.clone()));
        out.extend(frame.calls.iter().map(|at| card.calls[*at].clone()));
        out.extend(card.overlaps.iter().map(|o| o.line.clone()));
        out.extend(frame.step.iter().map(|s| s.title.clone()));
        out.join("\n")
    }

    #[test]
    fn a_reminders_tap_opens_what_it_is_about() {
        let now = 1_791_300_000_000;
        let none = String::new;
        let tap_on = |key: &str| tap(&format!(r#"{{"open":"reminder","key":"{key}","at":{now}}}"#), now);
        assert_eq!(tap_on("sioul:task/form"), Some(("task", "sioul:task/form".to_string(), "form".to_string())));
        assert_eq!(tap_on("sioul:budget/home"), Some(("budget", "sioul:budget/home".to_string(), none())));
        assert_eq!(tap_on("sioul:paper/passport"), Some(("paper", "sioul:paper/passport".to_string(), "passport".to_string())));
        assert_eq!(tap_on("sioul:contract/box"), Some(("contract", "sioul:contract/box".to_string(), "box".to_string())));
        assert_eq!(tap_on(""), Some(("porch", none(), none())));
    }

    #[test]
    fn a_tap_is_taken_once_and_not_late() {
        let now = 1_791_300_000_000;
        let none = String::new;
        assert_eq!(tap(&format!(r#"{{"open":"porch","at":{now}}}"#), now + 1000), Some(("porch", none(), none())));
        assert_eq!(tap(&format!(r#"{{"open":"now","uid":"a b/c","at":{now}}}"#), now), Some(("now", "sioul:task/a%20b%2Fc".to_string(), none())));
        assert_eq!(tap(&format!(r#"{{"open":"now","uid":"","at":{now}}}"#), now), Some(("now", none(), none())));
        // A message, on the Porch in its Reader; an event, in the Agenda; by their files.
        let file = "/mail/me/INBOX/new/1759600000.a.sioul";
        assert_eq!(tap(&format!(r#"{{"open":"mail","key":"{file}","at":{now}}}"#), now), Some(("card-mail", none(), file.to_string())));
        assert_eq!(tap(&format!(r#"{{"open":"event","key":"/calendars/me/personal/a.ics","at":{now}}}"#), now), Some(("event", none(), "/calendars/me/personal/a.ics".to_string())));
        assert_eq!(tap(&format!(r#"{{"open":"agenda","at":{now}}}"#), now), Some(("agenda", none(), none())));
        // Without their files, their pages.
        assert_eq!(tap(&format!(r#"{{"open":"mail","at":{now}}}"#), now), Some(("porch", none(), none())));
        assert_eq!(tap(&format!(r#"{{"open":"event","key":"","at":{now}}}"#), now), Some(("agenda", none(), none())));
        // Older than five minutes: Sioul did not come up for it.
        assert_eq!(tap(&format!(r#"{{"open":"porch","at":{now}}}"#), now + 6 * 60 * 1000), None);
        assert_eq!(tap(r#"{"open":"elsewhere","at":0}"#, 0), None);
        assert_eq!(tap("not json", now), None);
    }
}

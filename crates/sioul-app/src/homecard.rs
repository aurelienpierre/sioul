// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The card on a phone's home screen (docs/android.md, "The card on the home
//! screen"): what now is for, the Porch as it shows now, the next step when
//! it is the time for one, a dose due. Android draws it (android/package/src/
//! com/aurelienpierre/sioul/HomeCard.java), without Qt: Sioul writes what it
//! says in a small file of its own (`home-card.json`, in its state folder)
//! and tells Android to draw it again.
//!
//! Android freezes Sioul in the background, so the card is given ahead, in
//! frames: one per change of time until the end of tomorrow (work, admin, a
//! meal, winding down, sleep, waking, the Porch's hours, midnight), each with
//! its own words, the Porch's mail as it lets mail through then, and the next
//! step as the plan stands then. Java shows the frame for now and draws again
//! at its end. Rust writes the card when the Porch is computed, when the plan
//! is made, at the window's minute when a frame ended or five minutes went
//! by, and when Sioul is put away; outside the window, where Rust runs
//! already (a dose's alarm, its "Taken"), it writes the doses' lines alone.
//! Nothing here fetches anything. Elsewhere than on Android nothing is
//! written, unless SIOUL_HOME_CARD is set (to read the file on a computer).

use crate::backend::{load_config, tr};
use jiff::{Span, Timestamp, Zoned};
use serde::Serialize;
use sioul_core::areas::{Area, Time};
use sioul_core::cases::CaseStore;
use sioul_core::codes::CodeKind;
use sioul_core::config::Config;
use sioul_core::i18n::{self, Translator};
use sioul_core::links::Loaded;
use sioul_core::plan::Plan;
use sioul_core::porch::{self, Lane, Senders, Triaged};
use sioul_core::quiet::{self, Blocks, Mode, Overrides, Reason};
use sioul_core::reach::Reach;
use sioul_core::taskview::{self, Filter};
use sioul_core::today::{Today, Weather};
use sioul_core::trust::Trust;
use sioul_core::{pause, view, window};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The card's file, in Sioul's state folder; Java reads it there
/// (`files/state/sioul/`, android/main.cpp's XDG folders).
const FILE: &str = "home-card.json";
/// A tap on the card, kept by Java (HomeCardOpener.java) for the window.
const OPENED: &str = "home-card-opened";
/// The flag, among the task pages' (`work::WorkState`), that turns the details off.
pub(crate) const PLAIN: &str = "home-card-plain";
/// How long the card says nothing of its age: past it, "at 14:05".
const FRESH_MS: i64 = 15 * 60 * 1000;
/// While the window runs, the card is written again at least this often, its age said afresh.
const REWRITE_EVERY: i64 = 5 * 60;
/// The Porch's messages the card names, at most.
const ITEMS: usize = 3;
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
    frames: Vec<Frame>,
    /// The codes and links you asked sites for, each until it expires; newest first.
    codes: Vec<Code>,
    /// Today's doses due and not marked, from their time on.
    doses: Vec<DoseLine>,
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
    /// Codes may show (not asleep, not in a pause).
    codes: bool,
    /// Doses may show: not in a pause; asleep only when doses remind during sleep.
    doses: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct PorchLines {
    /// "Four letters came.", "The Porch opens at 14:00.", "Sioul is not set up yet."
    line: String,
    /// The first messages waiting, in the Porch's order.
    items: Vec<Item>,
}

impl PorchLines {
    fn said(line: String) -> PorchLines {
        PorchLines { line, items: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Item {
    sender: String,
    subject: String,
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
    store: Option<CaseStore>,
}

/// The plan, as the window last made it.
pub(crate) struct PlanInput {
    loaded: Arc<Loaded>,
    plan: Plan,
    today: Today,
    spent: BTreeMap<String, u32>,
    stopped: BTreeMap<String, String>,
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

/// The Porch's first sentence, counted in words: "Four letters came.";
/// past twelve, "Many letters came.": never a number on the home screen.
fn summary(tr: &Translator, items: &[Triaged]) -> String {
    match porch::summarise(items).total {
        0 => tr.text("summary-nothing", None),
        n if n <= 12 => tr.text("summary-total", Some(&tr.counted(n))),
        _ => tr.text("home-card-many", None),
    }
}

/// What the Porch shows at `at`, as `backend::compute` makes it: the mail
/// your lists let through at that time (`mail_in_view`; in free time,
/// `reach_now`), the Porch open in its hours and in quiet time, closed
/// otherwise with when it opens, resting after a pause until its next hours
/// (`pauses::porch_rests`). Never "Open it anyway": a moment's choice in the
/// window, not the card's.
fn porch_at(m: &Moment, input: &PorchInput, mode: &Mode, at: &Zoned) -> PorchLines {
    if let Some(until) = m.overrides.porch_rests_until.filter(|_| m.overrides.porch_rests(at.timestamp().as_second())).and_then(|u| Timestamp::from_second(u).ok()) {
        let until = until.to_zoned(at.time_zone().clone());
        return PorchLines::said(said(m.tr, "home-card-porch-rests", &[("when", when(m.tr, &until, at))]));
    }
    let reach = pause::reach_now(Reach::of(&m.config.reach).mail, mode, pause::nothing_now(m.overrides, &m.config.free_time));
    let area_of = |t: &Triaged| t.card.account.as_deref().and_then(|id| m.config.account(id)).and_then(|a| a.area.as_deref()).and_then(Area::parse).unwrap_or(Area::WORK);
    let mail: Vec<Triaged> = input.items.iter().filter(|t| mode.time == Time::Any || quiet::mail_in_view(t, &input.senders, &reach, area_of(t), mode.time, mode.week)).cloned().collect();
    let shown = view::porch(&mail, m.config, input.store.as_ref(), m.tr, at, mode.quiet);
    if !shown.open {
        return PorchLines::said(match window::next_opening(&m.config.windows, at) {
            Some(next) => said(m.tr, "home-card-porch-opens", &[("when", when(m.tr, &next, at))]),
            None => m.tr.text("home-card-porch-closed", None),
        });
    }
    // The lanes not folded, in the Porch's order; never what it folds (filed,
    // less important, set aside, hostile), whose words stay behind a click.
    let named = if m.details {
        shown.lanes.iter().filter(|l| !l.folded).flat_map(|l| l.items.iter()).filter(|i| !i.hidden).take(ITEMS).map(|i| Item { sender: i.sender.clone(), subject: i.subject.clone() }).collect()
    } else {
        Vec::new()
    };
    // Counted as the Porch counts then: the messages let through.
    PorchLines { line: summary(m.tr, &mail), items: named }
}

/// The next step at `at`, as Now would pick it then: only in work or admin
/// time (or with no hours set), never asleep, in a pause, in free time, at
/// leisure or during a meal; offices open or not then, "Not now" and the
/// day's weather today, a new day starting clean. The page's own filters
/// (one kind, one category) are the page's, not the card's.
fn step_at(m: &Moment, input: &PlanInput, mode: &Mode, at: &Zoned) -> Option<Step> {
    let time_for_it = matches!(mode.time, Time::Work | Time::Admin | Time::Several(_) | Time::Any);
    if !time_for_it || mode.sleeps() || mode.paused() || mode.free() {
        return None;
    }
    let situation = quiet::Situation::now(m.config, m.overrides, m.blocks, at, m.tr, &input.loaded.cases);
    let filter = Filter { quiet: situation.quiet_tasks(), ..Filter::default() };
    let today = input.today.date == at.date().to_string();
    let (weather, aside) = if today { (input.today.weather, input.today.aside.clone()) } else { (Weather::Clear, BTreeSet::new()) };
    let cx = taskview::Context { filter: &filter, offices: situation.offices.clone(), tasks: &input.loaded.tasks, plan: &input.plan, today: at.date(), tr: m.tr, cases: &input.loaded.cases, spent: &input.spent, stopped: &input.stopped };
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

/// The card, from now to the end of tomorrow.
fn snapshot(m: &Moment, porch: Option<&PorchInput>, plan: Option<&PlanInput>, doses: &[Dose]) -> Snapshot {
    let set_up = m.config.every_account().next().is_some();
    let mut made: BTreeMap<String, PorchLines> = BTreeMap::new();
    // Codes and doses as the notification matrix lets them come at each time (Settings ▸ Reminders and notifications).
    let notify = sioul_core::notify::Notify::of(m.config);
    let comes = |kind: sioul_core::notify::Kind, mode: &sioul_core::quiet::Mode| notify.comes(kind, &sioul_core::notify::Now::of(mode, false, false));
    let frames = timeline(m)
        .into_iter()
        .enumerate()
        .map(|(index, (from, until, mode))| {
            // Asleep, in a pause or in free time, the Porch says nothing on the card.
            let rests = mode.sleeps() || mode.paused() || mode.free();
            let porch = if rests {
                None
            } else if !set_up {
                Some(PorchLines::said(m.tr.text("home-card-setup", None)))
            } else {
                porch.map(|p| made.entry(porch_key(m, &mode, &from)).or_insert_with(|| porch_at(m, p, &mode, &from)).clone())
            };
            let kind = kind(&mode);
            Frame {
                from: ms(&from),
                until: ms(&until),
                kind: kind.to_string(),
                status: status(m, &mode, &from),
                dnd: m.dnd.map(|d| d.line_in(kind, ms(&from), index == 0)).unwrap_or_default(),
                porch,
                step: plan.and_then(|p| step_at(m, p, &mode, &from)),
                codes: comes(sioul_core::notify::Kind::Codes, &mode),
                // The pause's card shows the pause alone.
                doses: !mode.paused() && comes(sioul_core::notify::Kind::Doses, &mode),
            }
        })
        .collect();
    let made = ms(m.now);
    Snapshot {
        v: 1,
        made,
        details: m.details,
        stale_after: made + FRESH_MS,
        stale: labels(m.tr, m.now),
        beyond: m.tr.text("home-card-beyond", None),
        frames,
        codes: porch.map(|p| codes(m, p)).unwrap_or_default(),
        doses: dose_lines(m.tr, doses, m.details),
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
pub(crate) fn porch_seen(items: &[Triaged], senders: &Senders, store: Option<&CaseStore>) {
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
    let Some(frames) = card["frames"].as_array_mut() else { return false };
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
    let card = snapshot(&moment, Some(&porch), Some(&plan), &doses);
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

/// A tap on the card, kept by Java (HomeCardOpener.java) and taken once:
/// the Porch, or Now on a step (main.qml's `openThing`: "porch", "now" and
/// the task's address). One older than five minutes is let go.
pub(crate) fn opened() -> Option<(&'static str, String)> {
    let path = sioul_core::config::state_dir().join(OPENED);
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    tap(&text, Timestamp::now().as_millisecond())
}

/// A tap as Java writes it: {"open": "porch" | "now", "uid", "at" (Unix ms)}.
fn tap(text: &str, now: i64) -> Option<(&'static str, String)> {
    let tap: serde_json::Value = serde_json::from_str(text).ok()?;
    let at = tap["at"].as_i64()?;
    if now - at > TAP_KEPT_MS || at - now > 60_000 {
        return None;
    }
    match tap["open"].as_str()? {
        "porch" => Some(("porch", String::new())),
        "now" => Some(("now", tap["uid"].as_str().filter(|u| !u.is_empty()).map(sioul_core::links::task_uri).unwrap_or_default())),
        _ => None,
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
        grid: None,
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

    fn message(from: &str, subject: &str, lane: Lane) -> Triaged {
        let raw = format!("From: {from}\r\nSubject: {subject}\r\nDate: Mon, 05 Oct 2026 08:00:00 +0200\r\n\r\nHello.\r\n");
        let card = Card::from_bytes(raw.as_bytes()).unwrap();
        Triaged { card, lane, trust: Trust::Verified, code: None, reasons: Vec::new(), priority: Priority::Average, checks: None, assessment: None }
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

    fn make(now: &str, overrides: &Overrides, details: bool, language: &str, doses: &[Dose]) -> Made {
        let now = at(now);
        let config = config();
        let tr = Translator::new(language);
        let blocks = blocks(&now);
        let moment = Moment { now: &now, config: &config, overrides, blocks: &blocks, tr: &tr, details, dnd: None };
        let porch = porch_input(&now);
        let plan = plan_input(&now);
        Made { card: snapshot(&moment, Some(&porch), Some(&plan), doses) }
    }

    /// Every word the card can show at `when`, in one text.
    fn words(frame: &Frame) -> String {
        let mut out = vec![frame.status.clone()];
        if let Some(p) = &frame.porch {
            out.push(p.line.clone());
            out.extend(p.items.iter().map(|i| format!("{} · {}", i.sender, i.subject)));
        }
        if let Some(s) = &frame.step {
            out.extend([s.title.clone(), s.why.clone()]);
        }
        out.join("\n")
    }

    #[test]
    fn each_time_says_its_own() {
        // Monday 5 October 2026, 10:00 in Paris: work until 12:10 (lunch is got ready from then).
        let made = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), true, "en", &[]);
        let work = made.at("2026-10-05T10:00[Europe/Paris]");
        assert_eq!((work.kind.as_str(), work.status.as_str()), ("work", "Work until 12:10."));
        let porch = work.porch.as_ref().unwrap();
        // Four letters and a code came; the first three, sender and subject, in the Porch's order.
        assert_eq!(porch.line, "Five letters came.");
        assert_eq!(porch.items.iter().map(|i| i.sender.as_str()).collect::<Vec<_>>(), ["Marie Dupont", "Paul", "Léa"]);
        assert_eq!(porch.items[0].subject, "Dinner on Saturday");
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
        // doses remind during sleep by default.
        assert_eq!(made.at("2026-10-05T22:15[Europe/Paris]").status, "Winding down: nothing disturbs until tomorrow at 07:00.");
        let night = made.at("2026-10-05T23:30[Europe/Paris]");
        assert_eq!((night.kind.as_str(), night.status.as_str()), ("sleep", "Sleep: nothing disturbs until tomorrow at 07:00."));
        assert!(night.porch.is_none() && night.step.is_none() && !night.codes && night.doses);
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
        assert!(frame.porch.is_none() && frame.step.is_none() && !frame.codes && !frame.doses);
        // The pause lasts until you come back: one frame per day, all paused.
        assert!(made.card.frames.iter().all(|f| f.kind == "paused"));
        // Free time: its line; codes and doses reach you, as it says; no Porch, no step.
        let free = Overrides { free_since: Some(at("2026-10-05T14:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let made = make("2026-10-05T14:30[Europe/Paris]", &free, true, "en", &[]);
        let frame = made.at("2026-10-05T14:30[Europe/Paris]");
        assert_eq!(frame.kind, "free-time");
        assert_eq!(frame.status, "Free time: only your safe senders, doses and codes reach you. Work comes back when you do.");
        assert!(frame.porch.is_none() && frame.step.is_none() && frame.codes && frame.doses);
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
        assert!(porch.items.is_empty());
        // The next morning, open again: the letters shown.
        assert_eq!(made.at("2026-10-06T09:30[Europe/Paris]").porch.as_ref().unwrap().items.len(), 3);
        // Work now outside the hours: work's time, but the Porch keeps its own (closed until 09:00).
        let work_now = Overrides { work_now: Some(at("2026-10-05T20:30[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let made = make("2026-10-05T17:30[Europe/Paris]", &work_now, true, "en", &[]);
        let frame = made.at("2026-10-05T17:30[Europe/Paris]");
        assert_eq!(frame.status, "Work shown until 20:30, by your choice.");
        let porch = frame.porch.as_ref().unwrap();
        assert_eq!(porch.line, "The Porch opens tomorrow at 09:00.");
        assert!(porch.items.is_empty(), "never mail waiting for later");
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
        let card = snapshot(&moment, Some(&input), None, &[]);
        let evening = card.frames.iter().find(|f| f.kind == "leisure").unwrap();
        assert_eq!(evening.porch.as_ref().unwrap().line, "Nothing came. Nothing needs you.");
        let work = card.frames.iter().find(|f| f.kind == "work").unwrap();
        assert_eq!(work.porch.as_ref().unwrap().items[0].sender, "A stranger");
    }

    #[test]
    fn details_off_names_no_one() {
        let doses = [dose("Magnesium · 300 mg", "09:45", "2026-10-05T09:45[Europe/Paris]", i64::MAX / 2000)];
        let made = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), false, "en", &doses);
        assert!(!made.card.details);
        let frame = made.at("2026-10-05T10:00[Europe/Paris]");
        assert_eq!(frame.status, "Work until 12:10.");
        let porch = frame.porch.as_ref().unwrap();
        assert_eq!(porch.line, "Five letters came.");
        assert!(porch.items.is_empty());
        // The step without its title, nor a reason naming another step: its date.
        let step = frame.step.as_ref().unwrap();
        assert_eq!((step.title.as_str(), step.why.as_str()), ("Your next step", "By 30 October: three weeks left"));
        // A code, without its site or the code itself.
        assert_eq!(made.card.codes.len(), 1);
        assert_eq!(made.card.codes[0].line, "A code waits on the Porch.");
        // No dose line: a medicine's name says health.
        assert!(made.card.doses.is_empty());
        let all = serde_json::to_string(&made.card).unwrap();
        for private in ["Marie", "Dinner", "482913", "La Banque", "CAF", "Magnesium"] {
            assert!(!all.contains(private), "{private} shown with the details off: {all}");
        }
        assert_eq!(make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), false, "fr", &[]).card.codes[0].line, "Un code attend sur le Porche.");
    }

    #[test]
    fn no_count_as_a_number() {
        let now = at("2026-10-05T10:00[Europe/Paris]");
        let config = config();
        let overrides = Overrides::default();
        let blocks = blocks(&now);
        // A number anywhere but in a time (14:05) or a date: what a badge would be.
        let digits = regex_free_digits;
        for (language, n, said) in [("en", 7, "Seven letters came."), ("en", 12, "Twelve letters came."), ("en", 30, "Many letters came."), ("fr", 30, "Beaucoup de lettres sont arrivées."), ("fr", 1, "Une lettre est arrivée.")] {
            let tr = Translator::new(language);
            let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
            let mut input = porch_input(&now);
            input.items = (0..n).map(|i| message(&format!("Someone {i} <s{i}@example.org>"), "Hello", Lane::People)).collect();
            let card = snapshot(&moment, Some(&input), Some(&plan_input(&now)), &[]);
            let frame = card.frames.first().unwrap();
            assert_eq!(frame.porch.as_ref().unwrap().line, said);
            for frame in &card.frames {
                let mut shown = words(frame);
                // The senders are this test's own names, numbered.
                for item in frame.porch.iter().flat_map(|p| p.items.iter()) {
                    shown = shown.replace(&item.sender, "");
                }
                assert!(!digits(&shown), "a number on the card: {shown}");
            }
        }
    }

    /// Whether `text` holds a digit outside a time ("14:05") or a date ("30 October").
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
        // A day of the month before its name: "30 October", "30 octobre".
        let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December", "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
        let mut cleaned = rest.clone();
        for month in months {
            for day in (1..=31).rev() {
                cleaned = cleaned.replace(&format!("{day} {month}"), month).replace(&format!("{day}\u{a0}{month}"), month);
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
        let mut silent = config();
        silent.reminders.doses_in_sleep = false;
        let now = at("2026-10-05T10:00[Europe/Paris]");
        let tr = Translator::new("en");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &silent, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let card = snapshot(&moment, None, None, &doses);
        let night = card.frames.iter().find(|f| f.kind == "sleep").unwrap();
        assert!(!night.doses, "doses during sleep: stay silent");
        assert!(!night.codes, "as usual, codes wait on the Porch while you sleep");
        // As the notification matrix says (Settings ▸ Reminders and notifications): doses held in sleep, codes told then.
        let mut grid = config();
        grid.notify = toml::from_str::<Config>("[notify]\ndoses = [\"sleep:later\"]\ncodes = [\"sleep\"]\n").unwrap().notify;
        let card = snapshot(&Moment { config: &grid, ..moment }, None, None, &doses);
        let night = card.frames.iter().find(|f| f.kind == "sleep").unwrap();
        assert!(!night.doses && night.codes);
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
            snapshot(&moment, Some(&porch), Some(&plan), &[])
        };
        let (before, after) = (card_at(&first), card_at(&at("2026-10-05T14:06[Europe/Paris]")));
        assert_ne!(before.made, after.made);
        assert_eq!(content(&before), content(&after));
        // A new letter: said differently.
        let mut more = porch_input(&first);
        more.items.push(message("Ana <ana@example.org>", "Tea?", Lane::People));
        let blocks = blocks(&first);
        let moment = Moment { now: &first, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        assert_ne!(content(&before), content(&snapshot(&moment, Some(&more), Some(&plan), &[])));
    }

    #[test]
    fn not_set_up_yet() {
        let now = at("2026-10-05T10:00[Europe/Paris]");
        let config = Config { accounts: Vec::new(), ..config() };
        let tr = Translator::new("en");
        let blocks = blocks(&now);
        let overrides = Overrides::default();
        let moment = Moment { now: &now, config: &config, overrides: &overrides, blocks: &blocks, tr: &tr, details: true, dnd: None };
        let card = snapshot(&moment, Some(&porch_input(&now)), None, &[]);
        assert_eq!(card.frames[0].porch.as_ref().unwrap().line, "Sioul is not set up yet.");
        // Asleep it says only the night's line, set up or not.
        assert!(card.frames.iter().filter(|f| f.kind == "sleep").all(|f| f.porch.is_none()));
    }

    /// The names HomeCard.java reads, each where it reads it: a name changed
    /// here and not there would leave the card blank.
    #[test]
    fn the_file_java_reads() {
        let doses = [dose("Magnesium · 300 mg", "09:45", "2026-10-05T09:45[Europe/Paris]", 0)];
        let made = make("2026-10-05T10:00[Europe/Paris]", &Overrides::default(), true, "en", &doses);
        let json = serde_json::to_value(&made.card).unwrap();
        for key in ["made", "stale_after"] {
            assert!(json[key].is_i64(), "{key}");
        }
        assert!(json["details"].is_boolean() && json["beyond"].is_string());
        assert!(json["stale"][0]["until"].is_i64() && json["stale"][0]["text"].is_string());
        let frame = &json["frames"][0];
        for key in ["from", "until"] {
            assert!(frame[key].is_i64(), "{key}");
        }
        assert!(frame["status"].is_string() && frame["codes"].is_boolean() && frame["doses"].is_boolean());
        assert!(frame["porch"]["line"].is_string());
        assert!(frame["porch"]["items"][0]["sender"].is_string() && frame["porch"]["items"][0]["subject"].is_string());
        for key in ["title", "why", "uid"] {
            assert!(frame["step"][key].is_string(), "{key}");
        }
        assert!(json["codes"][0]["line"].is_string() && json["codes"][0]["warning"].is_string() && json["codes"][0]["until"].is_i64());
        for key in ["from", "until", "known_until"] {
            assert!(json["doses"][0][key].is_i64(), "{key}");
        }
        assert!(json["doses"][0]["line"].is_string() && json["doses"][0]["check"].is_string());
        assert!(frame["dnd"].is_string());
        // Asleep: no Porch, no step, written as null (Java's optJSONObject).
        let night = serde_json::to_value(made.at("2026-10-05T23:30[Europe/Paris]")).unwrap();
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
            snapshot(&moment, Some(&porch), Some(&plan), &[])
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
        assert!(snapshot(&moment, Some(&porch), Some(&plan), &[]).frames.iter().all(|f| f.dnd.is_empty()));
        // Its end not said: in the frame for now only.
        let unsaid = Dnd::of(&serde_json::json!({ "on": true, "why": "manual", "line": line })).unwrap();
        let card = card_with(Some(&unsaid));
        assert_eq!(card.frames[0].dnd, line);
        assert!(card.frames[1..].iter().all(|f| f.dnd.is_empty()));
        // Without the window (a receiver alone): the card on disk gets its lines, a frame cut at its end.
        let mut written = serde_json::to_value(card_with(None)).unwrap();
        let before = written["frames"].as_array().unwrap().len();
        let stamp = now.timestamp().as_millisecond();
        assert!(patch_dnd(&mut written, Some(&dnd), stamp));
        let frames = written["frames"].as_array().unwrap().clone();
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
        assert!(written["frames"].as_array().unwrap().iter().all(|f| f["dnd"] == ""));
    }

    #[test]
    fn a_tap_is_taken_once_and_not_late() {
        let now = 1_791_300_000_000;
        assert_eq!(tap(&format!(r#"{{"open":"porch","at":{now}}}"#), now + 1000), Some(("porch", String::new())));
        assert_eq!(tap(&format!(r#"{{"open":"now","uid":"a b/c","at":{now}}}"#), now), Some(("now", "sioul:task/a%20b%2Fc".to_string())));
        assert_eq!(tap(&format!(r#"{{"open":"now","uid":"","at":{now}}}"#), now), Some(("now", String::new())));
        // Older than five minutes: Sioul did not come up for it.
        assert_eq!(tap(&format!(r#"{{"open":"porch","at":{now}}}"#), now + 6 * 60 * 1000), None);
        assert_eq!(tap(r#"{"open":"elsewhere","at":0}"#, 0), None);
        assert_eq!(tap("not json", now), None);
    }
}

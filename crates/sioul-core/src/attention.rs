// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What reaches you, and when: one model for everything Sioul shows, tells,
//! holds or silences (docs/attention.md). Sioul spends one thing four ways,
//! your attention: what it shows (the Porch, the pages), what it tells (its
//! own notifications), what it holds of the system's (other apps'
//! notifications, calls), and what it silences (the system's do-not-disturb).
//!
//! **One matrix**: a row per who on each channel (mail, calls, messages from
//! other apps: Always through, safe, neutral, restricted, strangers, hidden
//! numbers for calls, groups for messages, the blocked fixed at never), and a
//! row per kind of Sioul's own and of the automated sources (codes, doses,
//! reminders, sites, other apps' automatons…). Nine columns: the seven times
//! (work, admin, leisure, meals, sleep, the pause, Free time) and two layers
//! on top of them (time for you, do-not-disturb from its switch or a focus
//! session). A cell holds a level, from the least strict to the strictest:
//! ● at once, ◑ shown, not told, ◐ when its event falls then, ◎ at the
//! gathered times, ○ later, – not at all; and on the Always through row
//! alone, = as their own row says, ☆ through this layer at their own row's
//! times. Two times open together (work and admin) give the less strict of
//! their cells; a layer that holds gives the stricter of it and the time.
//!
//! **The pipeline** (`Attention::decide`): what an event is; the emergency
//! numbers (calls); who wrote, proven or not; the blocked, never, whatever
//! else says (blocked beats Always through); the content (a code, a lane never
//! told); the floors (a second call, Let every call through); the exceptions
//! (Always through, a conversation, a site); the matrix; what the source is
//! for (its area); the named holds (the Porch resting after a pause, a
//! meeting, the chat limit). Each caller asks it, and stays where it is.
//!
//! **Kept in the configuration**: `[attention]`, one key per row in the words
//! `[notify]` used: a bare column is ●, `column:level` another level ("now",
//! "quiet", "event", "gathered", "later", "never", "as", "through"); a row
//! not written keeps its usual cells, a fixed cell keeps its value. While no
//! `[attention]` is written, the older `[reach]`, `[notify]` and the switches
//! they replaced seed it, once: what you changed there is kept, the rest is
//! the usual; the first row written writes them all and takes the older keys
//! out (`apply`).
//!
//! **Time for you everywhere**: today's slots are written to
//! `state/slots.toml` by the window that lays out the day (`Slots`); every
//! process reads them (the window, the listener of other apps, a phone's
//! background step, `sioul remind --watch`, the calls' table).

use crate::areas::{Area, Time, Week, in_view};
use crate::config::{Config, SettingValue};
use crate::i18n::Translator;
use crate::quiet::Mode;
use crate::reach::{Channel, Who};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- the objects

/// Who, as a row of a people channel (mail, calls, messages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Person {
    /// Always through (« Passent toujours »): the people on that list, one
    /// list on every device (`everywhere::People`), and a conversation set so.
    Always,
    Safe,
    Neutral,
    Restricted,
    /// In no address book and on no list; mail nothing proves is theirs.
    Stranger,
    /// Calls only: a call that shows no number.
    Hidden,
    /// Messages only: a conversation where several people write.
    Groups,
    /// Never, on any channel: a fixed row.
    Blocked,
}

impl Person {
    pub const ALL: [Person; 8] = [Person::Always, Person::Safe, Person::Neutral, Person::Restricted, Person::Stranger, Person::Hidden, Person::Groups, Person::Blocked];

    pub fn id(self) -> &'static str {
        match self {
            Person::Always => "always",
            Person::Safe => "safe",
            Person::Neutral => "neutral",
            Person::Restricted => "restricted",
            Person::Stranger => "stranger",
            Person::Hidden => "hidden",
            Person::Groups => "groups",
            Person::Blocked => "blocked",
        }
    }

    pub fn read(text: &str) -> Option<Person> {
        let text = text.trim().to_ascii_lowercase();
        match text.as_str() {
            "strangers" => Some(Person::Stranger),
            "group" => Some(Person::Groups),
            _ => Person::ALL.into_iter().find(|p| p.id() == text),
        }
    }

    /// The row of one of the five states.
    pub fn of(who: Who) -> Person {
        match who {
            Who::Safe => Person::Safe,
            Who::Neutral => Person::Neutral,
            Who::Restricted => Person::Restricted,
            Who::Stranger => Person::Stranger,
            Who::Blocked => Person::Blocked,
        }
    }

    /// One of the states' rows: neither Always through nor the blocked.
    pub fn state(self) -> bool {
        !matches!(self, Person::Always | Person::Blocked)
    }
}

/// The rows of a channel, in the order they are shown.
pub fn persons(channel: Channel) -> &'static [Person] {
    use Person::*;
    match channel {
        Channel::Mail => &[Always, Safe, Neutral, Restricted, Stranger, Blocked],
        Channel::Calls => &[Always, Safe, Neutral, Restricted, Stranger, Hidden, Blocked],
        Channel::Messages => &[Always, Safe, Neutral, Restricted, Stranger, Groups, Blocked],
    }
}

/// Sioul's own and the automated sources: a row each, with no who.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The codes and links you just asked a site for.
    Codes,
    /// A dose's reminder, and the question on doses due while Sioul was closed.
    Doses,
    /// The alarm at waking, on a phone.
    Wake,
    /// The alarms an event carries.
    Alarms,
    /// Sioul's own reminder before an event.
    Before,
    /// An event, the working day before.
    DayBefore,
    /// Dates asked, waits over, payments, papers and contracts, the money watch.
    Dates,
    /// Health's notices of a meal, a nap or the night.
    Needs,
    /// The pause to move.
    Move,
    /// "Work hours are over", with "Close the work day".
    WorkOver,
    /// The time running: a focus session's own notification.
    Time,
    /// What your sites notified, gathered (a computer).
    Sites,
    /// A site in real time (a computer).
    SitesLive,
    /// A call in a site (a computer).
    SiteCalls,
    /// Other apps' automatons, on a phone.
    AppAutomatons,
    /// Other apps and browser sites set to come at once, on a phone.
    AppAtOnce,
}

impl Kind {
    pub const ALL: [Kind; 16] = [
        Kind::Codes,
        Kind::Doses,
        Kind::Wake,
        Kind::Alarms,
        Kind::Before,
        Kind::DayBefore,
        Kind::Dates,
        Kind::Needs,
        Kind::Move,
        Kind::WorkOver,
        Kind::Time,
        Kind::Sites,
        Kind::SitesLive,
        Kind::SiteCalls,
        Kind::AppAutomatons,
        Kind::AppAtOnce,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Kind::Codes => "codes",
            Kind::Doses => "doses",
            Kind::Wake => "wake",
            Kind::Alarms => "alarms",
            Kind::Before => "before",
            Kind::DayBefore => "day-before",
            Kind::Dates => "dates",
            Kind::Needs => "needs",
            Kind::Move => "move",
            Kind::WorkOver => "work-over",
            Kind::Time => "time",
            Kind::Sites => "sites",
            Kind::SitesLive => "sites-live",
            Kind::SiteCalls => "site-calls",
            Kind::AppAutomatons => "app-automatons",
            Kind::AppAtOnce => "app-at-once",
        }
    }

    pub fn read(text: &str) -> Option<Kind> {
        let text = text.trim().to_ascii_lowercase().replace('_', "-");
        Kind::ALL.into_iter().find(|k| k.id() == text)
    }

    /// The group it is shown in: what you set or asked for, reminders, your
    /// day, sites (a computer), other apps (a phone).
    pub fn group(self) -> &'static str {
        match self {
            Kind::Codes | Kind::Doses | Kind::Wake | Kind::Alarms => "asked",
            Kind::Before | Kind::DayBefore | Kind::Dates => "reminders",
            Kind::Needs | Kind::Move | Kind::WorkOver | Kind::Time => "day",
            Kind::Sites | Kind::SitesLive | Kind::SiteCalls => "sites",
            Kind::AppAutomatons | Kind::AppAtOnce => "apps",
        }
    }
}

/// A row of the matrix: a who on a channel, or a kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Row {
    People(Channel, Person),
    Own(Kind),
}

impl Row {
    /// Every row, in the order they are shown: mail's, calls', messages', then the kinds.
    pub const ALL: [Row; 36] = [
        Row::People(Channel::Mail, Person::Always),
        Row::People(Channel::Mail, Person::Safe),
        Row::People(Channel::Mail, Person::Neutral),
        Row::People(Channel::Mail, Person::Restricted),
        Row::People(Channel::Mail, Person::Stranger),
        Row::People(Channel::Mail, Person::Blocked),
        Row::People(Channel::Calls, Person::Always),
        Row::People(Channel::Calls, Person::Safe),
        Row::People(Channel::Calls, Person::Neutral),
        Row::People(Channel::Calls, Person::Restricted),
        Row::People(Channel::Calls, Person::Stranger),
        Row::People(Channel::Calls, Person::Hidden),
        Row::People(Channel::Calls, Person::Blocked),
        Row::People(Channel::Messages, Person::Always),
        Row::People(Channel::Messages, Person::Safe),
        Row::People(Channel::Messages, Person::Neutral),
        Row::People(Channel::Messages, Person::Restricted),
        Row::People(Channel::Messages, Person::Stranger),
        Row::People(Channel::Messages, Person::Groups),
        Row::People(Channel::Messages, Person::Blocked),
        Row::Own(Kind::Codes),
        Row::Own(Kind::Doses),
        Row::Own(Kind::Wake),
        Row::Own(Kind::Alarms),
        Row::Own(Kind::Before),
        Row::Own(Kind::DayBefore),
        Row::Own(Kind::Dates),
        Row::Own(Kind::Needs),
        Row::Own(Kind::Move),
        Row::Own(Kind::WorkOver),
        Row::Own(Kind::Time),
        Row::Own(Kind::Sites),
        Row::Own(Kind::SitesLive),
        Row::Own(Kind::SiteCalls),
        Row::Own(Kind::AppAutomatons),
        Row::Own(Kind::AppAtOnce),
    ];

    /// "mail.safe", "calls.hidden", "messages.groups", "codes", "day-before".
    pub fn id(self) -> String {
        match self {
            Row::People(channel, person) => format!("{}.{}", channel.id(), person.id()),
            Row::Own(kind) => kind.id().to_string(),
        }
    }

    /// A row's id as the configuration and Settings write it; none when it is
    /// no row (a person a channel does not have: hidden mail).
    pub fn read(text: &str) -> Option<Row> {
        let text = text.trim();
        if let Some((channel, person)) = text.split_once('.') {
            let (channel, person) = (Channel::read(channel)?, Person::read(person)?);
            return persons(channel).contains(&person).then_some(Row::People(channel, person));
        }
        Kind::read(text).map(Row::Own)
    }

    fn index(self) -> usize {
        Row::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }

    /// The group it is shown in: its channel ("mail", "calls", "messages"), or its kind's.
    pub fn group(self) -> &'static str {
        match self {
            Row::People(channel, _) => channel.id(),
            Row::Own(kind) => kind.group(),
        }
    }

    /// Always through's row: its own values (= and ☆).
    pub fn always(self) -> bool {
        matches!(self, Row::People(_, Person::Always))
    }
}

/// A column: one of the seven times, or a layer above them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Column {
    Work,
    Admin,
    Leisure,
    Meals,
    /// The night from winding down to waking, and naps.
    Sleep,
    /// The pause (« En pause »), whatever the time.
    Pause,
    /// Free time (« Temps libre »).
    Free,
    /// A slot of time for you in today's plan: a layer.
    Slot,
    /// Do-not-disturb from its switch or a focus session: a layer.
    Dnd,
}

impl Column {
    pub const ALL: [Column; 9] = [Column::Work, Column::Admin, Column::Leisure, Column::Meals, Column::Sleep, Column::Pause, Column::Free, Column::Slot, Column::Dnd];
    /// The seven times, without the layers.
    pub const TIMES: [Column; 7] = [Column::Work, Column::Admin, Column::Leisure, Column::Meals, Column::Sleep, Column::Pause, Column::Free];

    pub fn id(self) -> &'static str {
        match self {
            Column::Work => "work",
            Column::Admin => "admin",
            Column::Leisure => "leisure",
            Column::Meals => "meals",
            Column::Sleep => "sleep",
            Column::Pause => "pause",
            Column::Free => "free",
            Column::Slot => "slot",
            Column::Dnd => "dnd",
        }
    }

    /// A word of the configuration: its id, or the words older settings and
    /// French used ("Repas", "En pause", "Temps libre", "do-not-disturb").
    pub fn read(word: &str) -> Option<Column> {
        Some(match word.trim().to_lowercase().as_str() {
            "work" | "travail" => Column::Work,
            "admin" | "démarches" | "demarches" => Column::Admin,
            "leisure" | "loisirs" | "rest" | "personal" | "perso" => Column::Leisure,
            "meals" | "meal" | "repas" => Column::Meals,
            "sleep" | "sommeil" | "night" | "nuit" => Column::Sleep,
            "pause" | "paused" | "en pause" => Column::Pause,
            "free" | "free-time" | "free time" | "temps libre" => Column::Free,
            "slot" | "for-you" | "time-for-you" | "time for you" | "du temps pour vous" => Column::Slot,
            "dnd" | "do-not-disturb" | "do not disturb" | "ne pas déranger" => Column::Dnd,
            _ => return None,
        })
    }

    /// One of the seven times, not a layer.
    pub fn is_time(self) -> bool {
        !matches!(self, Column::Slot | Column::Dnd)
    }

    /// Whether its end is known while it holds, for "when its event falls
    /// then": not the pause's, which lasts until you come back, nor a layer's.
    pub fn ends(self) -> bool {
        !matches!(self, Column::Pause | Column::Slot | Column::Dnd)
    }

    fn index(self) -> usize {
        Column::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }
}

/// What a cell says, from the least strict to the strictest (the first six);
/// the last two belong to the Always through row alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// ● It comes now: shown, told, let through, rings.
    Now,
    /// ◑ It is in Sioul now, if you look (the Porch); nothing tells you. It is
    /// told when a time comes in which it is told.
    Quiet,
    /// ◐ An event's own: at once when its event begins within this time; else later.
    Event,
    /// ◎ Told, or let through, at the next gathered time that allows it.
    Gathered,
    /// ○ It waits out of sight, and comes when a time it may come in begins.
    Later,
    /// – Never told; what Sioul keeps stays in its place, a message or a call never let through.
    Never,
    /// = Always through's row: as their own row says.
    As,
    /// ☆ Always through's row, on a layer: the layer does not hold them; their own row's times do.
    Through,
}

impl Level {
    pub const ALL: [Level; 8] = [Level::Now, Level::Quiet, Level::Event, Level::Gathered, Level::Later, Level::Never, Level::As, Level::Through];

    pub fn id(self) -> &'static str {
        match self {
            Level::Now => "now",
            Level::Quiet => "quiet",
            Level::Event => "event",
            Level::Gathered => "gathered",
            Level::Later => "later",
            Level::Never => "never",
            Level::As => "as",
            Level::Through => "through",
        }
    }

    pub fn read(text: &str) -> Option<Level> {
        let text = text.trim().to_ascii_lowercase();
        match text.as_str() {
            "shown" => Some(Level::Quiet),
            _ => Level::ALL.into_iter().find(|l| l.id() == text),
        }
    }

    /// Its mark in a grid; at once on the Always through row is ★ ("whatever their list").
    pub fn mark(self, always: bool) -> &'static str {
        match self {
            Level::Now if always => "★",
            Level::Now => "●",
            Level::Quiet => "◑",
            Level::Event => "◐",
            Level::Gathered => "◎",
            Level::Later => "○",
            Level::Never => "–",
            Level::As => "=",
            Level::Through => "☆",
        }
    }
}

// ---------------------------------------------------------------- the usual matrix, choices, locks

/// The usual cell of a row: what Sioul does unless you change it. Today's
/// behaviour, with the owner's decisions of 7 October 2026 (docs/attention.md,
/// §9): Always through at once everywhere (mail shown, not told, in sleep and
/// a pause); messages from people held in sleep and a pause; codes at once in
/// every time; calls carry the layers; groups a row of their own.
pub fn usual(row: Row, column: Column) -> Level {
    use Column::*;
    use Level::*;
    let hours = matches!(column, Work | Admin | Leisure | Meals);
    let rests = matches!(column, Sleep | Pause | Free);
    match row {
        Row::People(_, Person::Blocked) => Never,
        Row::People(Channel::Mail, Person::Always) => if matches!(column, Sleep | Pause) { Quiet } else { Now },
        Row::People(_, Person::Always) => Now,
        Row::People(Channel::Mail, Person::Safe) => if hours { Now } else { Quiet },
        Row::People(Channel::Mail, Person::Neutral | Person::Stranger) => match column {
            Work | Admin => Now,
            Slot | Dnd => Quiet,
            _ => Later,
        },
        Row::People(Channel::Mail, Person::Restricted) => match column {
            Work => Now,
            Slot | Dnd => Quiet,
            _ => Later,
        },
        Row::People(Channel::Calls | Channel::Messages, Person::Safe) => if matches!(column, Sleep | Pause | Dnd) { Later } else { Now },
        Row::People(Channel::Calls | Channel::Messages, Person::Neutral | Person::Hidden | Person::Groups) => if matches!(column, Work | Admin | Slot) { Now } else { Later },
        Row::People(Channel::Messages, Person::Stranger) => if matches!(column, Work | Admin | Slot) { Now } else { Later },
        Row::People(Channel::Calls | Channel::Messages, Person::Restricted) => if matches!(column, Work | Slot) { Now } else { Later },
        Row::People(Channel::Calls, Person::Stranger) => if column == Slot { Now } else { Later },
        Row::People(_, _) => Later,
        Row::Own(kind) => match kind {
            Kind::Codes | Kind::Doses | Kind::Wake | Kind::Time => Now,
            Kind::Alarms | Kind::Before => if matches!(column, Sleep | Free) { Event } else { Now },
            Kind::DayBefore | Kind::Dates => if rests { Later } else { Now },
            Kind::Needs => if rests { Never } else { Now },
            Kind::Move => if hours { Now } else { Never },
            Kind::WorkOver => if column == Work || rests { Never } else { Now },
            Kind::Sites | Kind::AppAutomatons => if hours { Gathered } else { Later },
            Kind::SitesLive | Kind::AppAtOnce => if hours { Now } else { Later },
            Kind::SiteCalls => if hours || column == Slot { Now } else { Later },
        },
    }
}

/// The values a cell may take, the usual first among them where it can.
pub fn choices(row: Row, column: Column) -> &'static [Level] {
    use Level::*;
    let layer = !column.is_time();
    match row {
        Row::People(_, Person::Blocked) => &[Never],
        Row::People(Channel::Mail, Person::Always) if layer => &[Now, Quiet, Through, As],
        Row::People(Channel::Mail, Person::Always) => &[Now, Quiet, As],
        Row::People(_, Person::Always) if layer => &[Now, Through, As],
        Row::People(_, Person::Always) => &[Now, As],
        Row::People(Channel::Mail, _) => &[Now, Quiet, Later, Never],
        Row::People(_, _) => &[Now, Later],
        Row::Own(kind) => match kind {
            Kind::Codes => &[Now, Never],
            Kind::Doses => &[Now, Later],
            Kind::Wake => &[Now],
            Kind::Alarms | Kind::Before if column.ends() => &[Now, Event, Later],
            Kind::Alarms | Kind::Before => &[Now, Later],
            Kind::DayBefore | Kind::Dates | Kind::SitesLive | Kind::SiteCalls | Kind::AppAtOnce => &[Now, Later],
            Kind::Needs | Kind::Move | Kind::WorkOver | Kind::Time => &[Now, Never],
            Kind::Sites => &[Gathered, Later],
            Kind::AppAutomatons => &[Now, Gathered, Later],
        },
    }
}

/// Why a cell is fixed, as the id of its sentence; none when it can change.
/// A dose comes at its time but in sleep and a pause, your choice; a code you
/// just asked for at once, but in sleep, where "On the Porch only" stays a
/// choice; an event's alarms in a pause and under do-not-disturb, as both
/// promise; the alarm at waking always; the blocked never.
pub fn lock(row: Row, column: Column) -> Option<&'static str> {
    match row {
        Row::People(_, Person::Blocked) => Some("attention-lock-blocked"),
        Row::Own(Kind::Doses) if !matches!(column, Column::Sleep | Column::Pause) => Some("attention-lock-doses"),
        Row::Own(Kind::Codes) if column != Column::Sleep => Some("attention-lock-codes"),
        Row::Own(Kind::Alarms) if matches!(column, Column::Pause | Column::Dnd) => Some("attention-lock-alarms"),
        Row::Own(Kind::Wake) => Some("attention-lock-wake"),
        _ => None,
    }
}

// ---------------------------------------------------------------- a source's own rows

/// A source's own row, on a phone (docs/attention.md, §1.3): an app's
/// (`app.<package>`) or a conversation's in one (`conversation.<key>`, the key
/// `appnotes::talk_key` makes). A row of the matrix like the others, a cell
/// per column, but each cell says "as usual" (=) until set, read then as the
/// row the notification takes otherwise (a person's on Messages, a group's,
/// the automatons', the apps set to come at once), as Always through's =
/// reads their own row. A conversation's cell set wins over its app's; a
/// source's cell set for a time is your word for it, so no area holds it
/// then. Blocked senders, codes and what is never held stay as they are.
pub const APP_ROW: &str = "app.";
pub const CONVERSATION_ROW: &str = "conversation.";
/// A source row's word for its name, written for your other devices
/// ("name=Discord"); never a column, so an older Sioul leaves it aside.
pub const NAME: &str = "name=";

/// The levels a source's cell takes: at once, at the gathered times, held
/// (later), as usual.
pub const SOURCE_CHOICES: [Level; 4] = [Level::Now, Level::Gathered, Level::Later, Level::As];

/// Whether an id names a source's row: `app.<package>` or `conversation.<key>`.
pub fn is_source(id: &str) -> bool {
    [APP_ROW, CONVERSATION_ROW].iter().any(|prefix| id.strip_prefix(prefix).is_some_and(|rest| !rest.is_empty() && !rest.contains(char::is_whitespace)))
}

/// The source rows a notification reads, the most precise first: its
/// conversation's when it has one, then its app's.
pub fn source_rows(package: &str, conversation: &str) -> Vec<String> {
    let (package, conversation) = (package.trim(), conversation.trim());
    let mut rows = Vec::new();
    if !conversation.is_empty() {
        rows.push(format!("{CONVERSATION_ROW}{conversation}"));
    }
    if !package.is_empty() {
        rows.push(format!("{APP_ROW}{package}"));
    }
    rows
}

/// What a source's row is read over: a row of the matrix, or a person on a
/// channel, on the Always through list or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    Row(Row),
    Person(Channel, Person, bool),
}

// ---------------------------------------------------------------- the moment

/// What holds now, beside the time and its layers: the named holds kept
/// outside the matrix (docs/attention.md, §1.7), as the caller knows them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Holds {
    /// After a pause, the Porch rests until the next admin hours: nothing of mail is told.
    pub porch_rests: bool,
    /// An event goes on: no meal's notice, no "Work hours are over".
    pub meeting: bool,
    /// The chat limit is reached: chat sites wait.
    pub chats: bool,
}

/// The moment as the matrix reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Now {
    /// The time's columns: one; work and admin when both hold (no hours set,
    /// or both open at once).
    pub times: Vec<Column>,
    /// What now is for, and the week's hours: for what a source is for (areas)
    /// and for lending work's cells to admin and back.
    pub time: Time,
    pub week: Week,
    /// In a slot of time for you (`Slots`).
    pub slot: bool,
    /// Do-not-disturb from its switch or a focus session.
    pub dnd: bool,
    /// Free time's "Nothing at all": the states' rows wait, Always through aside.
    pub nothing: bool,
    /// The time now, from when to when (Unix seconds), for "when its event falls then".
    pub span: (i64, i64),
    /// This instant is a gathered time (`[reminders] gathered`).
    pub gathering: bool,
    /// The Porch's Real time box: every site in real time.
    pub realtime: bool,
    /// Let every call through: every call rings but the blocked's.
    pub through: bool,
    pub holds: Holds,
}

impl Now {
    /// From what now is for (`quiet::mode`): the pause, sleep, Free time, then
    /// the time itself; no layer, no hold.
    pub fn of(mode: &Mode) -> Now {
        let times = if mode.paused() {
            vec![Column::Pause]
        } else if mode.sleeps() {
            vec![Column::Sleep]
        } else if mode.free() {
            vec![Column::Free]
        } else {
            match mode.time {
                Time::Work => vec![Column::Work],
                Time::Admin => vec![Column::Admin],
                Time::Leisure => vec![Column::Leisure],
                Time::Meals => vec![Column::Meals],
                Time::Sleep => vec![Column::Sleep],
                Time::Several(open) if open.work != open.admin => vec![if open.work { Column::Work } else { Column::Admin }],
                Time::Any | Time::Several(_) => vec![Column::Work, Column::Admin],
            }
        };
        let end = if mode.paused() { i64::MAX } else { mode.until.as_ref().map_or(i64::MAX, |u| u.timestamp().as_second()) };
        Now { times, time: mode.time, week: mode.week, slot: false, dnd: false, nothing: false, span: (i64::MIN, end), gathering: false, realtime: false, through: false, holds: Holds::default() }
    }

    /// From a clock's moment (`reach::Clock`, ahead: the calls' frames, other
    /// apps' times): the same columns as `of`, Free time's "Nothing at all" said.
    pub fn of_moment(moment: &crate::reach::Moment) -> Now {
        let times = if moment.paused {
            vec![Column::Pause]
        } else if moment.time == Time::Sleep {
            vec![Column::Sleep]
        } else if moment.free {
            vec![Column::Free]
        } else {
            match moment.time {
                Time::Work => vec![Column::Work],
                Time::Admin => vec![Column::Admin],
                Time::Meals => vec![Column::Meals],
                Time::Several(open) if open.work != open.admin => vec![if open.work { Column::Work } else { Column::Admin }],
                Time::Any | Time::Several(_) => vec![Column::Work, Column::Admin],
                _ => vec![Column::Leisure],
            }
        };
        Now { times, time: moment.time, week: moment.week, slot: false, dnd: false, nothing: moment.free && moment.nothing, span: (i64::MIN, i64::MAX), gathering: false, realtime: false, through: false, holds: Holds::default() }
    }

    /// One time alone, no layer: for what has no hours (tests, a grid's card).
    pub fn time(column: Column) -> Now {
        let time = match column {
            Column::Work => Time::Work,
            Column::Admin => Time::Admin,
            Column::Meals => Time::Meals,
            Column::Sleep | Column::Pause => Time::Sleep,
            _ => Time::Leisure,
        };
        let week = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        Now { times: vec![column], time, week, slot: false, dnd: false, nothing: false, span: (i64::MIN, i64::MAX), gathering: false, realtime: false, through: false, holds: Holds::default() }
    }

    /// The same moment with its two layers: a slot of time for you, do-not-disturb.
    pub fn layers(mut self, slot: bool, dnd: bool) -> Now {
        self.slot = slot;
        self.dnd = dnd;
        self
    }

    /// Whether one of the time's columns is `column`.
    pub fn is(&self, column: Column) -> bool {
        self.times.contains(&column)
    }
}

// ---------------------------------------------------------------- the matrix

/// The matrix: a level in each cell, as set; and the sources' own rows, an
/// app's or a conversation's on a phone, by id (`app.<package>`,
/// `conversation.<key>`), each with its name when written with one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attention {
    cells: [[Level; 9]; 36],
    sources: BTreeMap<String, [Level; 9]>,
    names: BTreeMap<String, String>,
}

impl Default for Attention {
    fn default() -> Attention {
        Attention::usual()
    }
}

impl Attention {
    /// What Sioul does unless you change it (`usual`).
    pub fn usual() -> Attention {
        let mut cells = [[Level::Now; 9]; 36];
        for row in Row::ALL {
            for column in Column::ALL {
                cells[row.index()][column.index()] = usual(row, column);
            }
        }
        Attention { cells, sources: BTreeMap::new(), names: BTreeMap::new() }
    }

    /// As the configuration says: the usual cells; while no `[attention]` is
    /// written, what the older keys changed (`seeded`); then each row's words.
    /// A fixed cell, a value not offered there and a word not understood are
    /// left aside; of two words for one column, the later counts.
    pub fn of(config: &Config) -> Attention {
        let Some(settings) = config.attention.as_ref() else { return seeded(config) };
        let mut attention = Attention::usual();
        for row in Row::ALL {
            for (column, level) in settings.words(&row.id()).unwrap_or_default().iter().filter_map(|w| word(w)) {
                let _ = attention.set(row, column, level);
            }
        }
        // The sources' own rows: their cells set, their name.
        for (id, words) in settings.sources() {
            for text in &words {
                match text.strip_prefix(NAME) {
                    Some(name) => attention.name_source(&id, name),
                    None => {
                        if let Some((column, level)) = word(text) {
                            let _ = attention.set_source(&id, column, level);
                        }
                    }
                }
            }
        }
        let sources = &attention.sources;
        attention.names.retain(|id, _| sources.contains_key(id));
        attention
    }

    pub fn cell(&self, row: Row, column: Column) -> Level {
        self.cells[row.index()][column.index()]
    }

    /// A cell changed; refused when it is fixed, or when the value is not one
    /// it may take (`choices`).
    pub fn set(&mut self, row: Row, column: Column, level: Level) -> Result<(), String> {
        if self.cell(row, column) == level {
            return Ok(());
        }
        if lock(row, column).is_some() {
            return Err(format!("attention.{}: {} is fixed", row.id(), column.id()));
        }
        if !choices(row, column).contains(&level) {
            let offered: Vec<&str> = choices(row, column).iter().map(|l| l.id()).collect();
            return Err(format!("attention.{}: {}: {}", row.id(), column.id(), offered.join(", ")));
        }
        self.cells[row.index()][column.index()] = level;
        Ok(())
    }

    /// A row as the configuration writes it, whole: "work" for at once,
    /// "sleep:later" for another value.
    pub fn words(&self, row: Row) -> Vec<String> {
        Column::ALL
            .iter()
            .map(|column| match self.cell(row, *column) {
                Level::Now => column.id().to_string(),
                other => format!("{}:{}", column.id(), other.id()),
            })
            .collect()
    }

    /// How many cells differ from another matrix's (a preset's); the
    /// sources' own rows aside, which no preset sets.
    pub fn changes_from(&self, other: &Attention) -> usize {
        Row::ALL.iter().flat_map(|r| Column::ALL.iter().map(move |c| (*r, *c))).filter(|(r, c)| self.cell(*r, *c) != other.cell(*r, *c)).count()
    }

    /// A source's own row (`app.<package>`, `conversation.<key>`): its nine
    /// cells; none while every one says as usual.
    pub fn source(&self, id: &str) -> Option<[Level; 9]> {
        self.sources.get(id).copied()
    }

    /// The sources' rows set, by id.
    pub fn sources(&self) -> &BTreeMap<String, [Level; 9]> {
        &self.sources
    }

    /// The name a source's row was written with (an app's, for your other
    /// devices, which never saw it); "" when none.
    pub fn source_name(&self, id: &str) -> &str {
        self.names.get(id).map_or("", String::as_str)
    }

    /// A source's cell changed; refused when the id names no source, or the
    /// value is not one a source's cell takes (`SOURCE_CHOICES`). A row as
    /// usual everywhere is no row: taken out.
    pub fn set_source(&mut self, id: &str, column: Column, level: Level) -> Result<(), String> {
        if !is_source(id) {
            return Err(format!("attention.{id}: no such row"));
        }
        if !SOURCE_CHOICES.contains(&level) {
            let offered: Vec<&str> = SOURCE_CHOICES.iter().map(|l| l.id()).collect();
            return Err(format!("attention.{id}: {}: {}", column.id(), offered.join(", ")));
        }
        let mut cells = self.source(id).unwrap_or([Level::As; 9]);
        cells[column.index()] = level;
        if cells.iter().all(|l| *l == Level::As) {
            self.sources.remove(id);
        } else {
            self.sources.insert(id.to_string(), cells);
        }
        Ok(())
    }

    /// A source's name, kept beside its row ("" takes it out).
    pub fn name_source(&mut self, id: &str, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.names.remove(id);
        } else {
            self.names.insert(id.to_string(), name.to_string());
        }
    }

    /// A source's row as the configuration writes it: its cells set ("work"
    /// for at once, "admin:later"), as usual unsaid, then its name
    /// ("name=Discord"); nothing when as usual everywhere.
    pub fn source_words(&self, id: &str) -> Vec<String> {
        let Some(cells) = self.source(id) else { return Vec::new() };
        let mut words: Vec<String> = Column::ALL
            .iter()
            .filter(|c| cells[c.index()] != Level::As)
            .map(|c| match cells[c.index()] {
                Level::Now => c.id().to_string(),
                other => format!("{}:{}", c.id(), other.id()),
            })
            .collect();
        if let Some(name) = self.names.get(id).filter(|n| !n.is_empty()) {
            words.push(format!("{NAME}{name}"));
        }
        words
    }

    /// Whether one of these sources has a row set: a notification from them reads it.
    pub fn has_sources(&self, ids: &[String]) -> bool {
        ids.iter().any(|id| self.sources.contains_key(id))
    }

    /// The rows set among these sources, the most precise first.
    fn source_cells(&self, ids: &[String]) -> Vec<[Level; 9]> {
        ids.iter().filter_map(|id| self.source(id)).collect()
    }

    /// What a row, or a person on a channel, gives at one column, as `level`
    /// and `person` read it (= and ☆ resolved, "Nothing at all" holding the
    /// states' rows in Free time): what a source's "as usual" stands for.
    fn base_at(&self, base: Base, column: Column, now: &Now) -> Level {
        let free_nothing = column == Column::Free && now.nothing;
        match base {
            Base::Row(row) => {
                if free_nothing && matches!(row, Row::People(_, person) if person.state()) {
                    return Level::Later;
                }
                match self.lent(row, column, now.week) {
                    Level::As => Level::Later,
                    Level::Through => Level::Now,
                    other => other,
                }
            }
            Base::Person(channel, person, always) => {
                if person == Person::Blocked {
                    return Level::Never;
                }
                let own = Row::People(channel, if person == Person::Always { Person::Safe } else { person });
                if !always && person != Person::Always {
                    return self.base_at(Base::Row(own), column, now);
                }
                let always_row = Row::People(channel, Person::Always);
                let level = match self.lent(always_row, column, now.week) {
                    Level::As => self.lent(own, column, now.week),
                    Level::Through => Level::Now,
                    level => level,
                };
                if free_nothing && self.cell(always_row, Column::Free) == Level::As { level.max(Level::Later) } else { level }
            }
        }
    }

    /// A row, or a person, read with sources' rows over it (`sources`, the
    /// most precise first: a conversation's, then its app's): at each column
    /// the first cell set, else the row's own (`base_at`); then as `level`
    /// reads a moment, the least strict of the times, the stricter of that
    /// and each layer that holds. With it, whether a source's cell said any
    /// of the columns read now.
    fn over(&self, base: Base, sources: &[[Level; 9]], now: &Now) -> (Level, bool) {
        let at = |column: Column| match sources.iter().map(|cells| cells[column.index()]).find(|l| *l != Level::As) {
            Some(level) => (level, true),
            None => (self.base_at(base, column, now), false),
        };
        let times: Vec<(Level, bool)> = now.times.iter().map(|c| at(*c)).collect();
        let mut level = times.iter().map(|(l, _)| *l).min().unwrap_or(Level::Now);
        let mut chosen = times.iter().any(|(_, set)| *set);
        for (holds, column) in [(now.slot, Column::Slot), (now.dnd, Column::Dnd)] {
            if holds {
                let (layer, set) = at(column);
                level = level.max(layer);
                chosen |= set;
            }
        }
        (level, chosen)
    }

    /// A row's cell at a time column, lent as the week says for the people's
    /// rows: while admin has no hours of its own, work time takes admin's
    /// cell too; while work has none, admin time takes work's (`areas::in_view`).
    fn lent(&self, row: Row, column: Column, week: Week) -> Level {
        let own = self.cell(row, column);
        if !matches!(row, Row::People(..)) {
            return own;
        }
        match column {
            Column::Work if !week.admin_hours => own.min(self.cell(row, Column::Admin)),
            Column::Admin if !week.work_hours => own.min(self.cell(row, Column::Work)),
            _ => own,
        }
    }

    /// A row now: the least strict of the time's columns, then the stricter of
    /// that and each layer that holds. A state's row waits in a Free time of
    /// "Nothing at all". Always through's own values read here as what the row
    /// alone lets through (= waits, ☆ comes); `person` reads them against a
    /// person's own row.
    pub fn level(&self, row: Row, now: &Now) -> Level {
        if let Row::People(_, person) = row
            && person.state()
            && now.nothing
            && now.is(Column::Free)
        {
            return Level::Later;
        }
        let read = |level: Level| match level {
            Level::As => Level::Later,
            Level::Through => Level::Now,
            other => other,
        };
        let mut level = now.times.iter().map(|c| read(self.lent(row, *c, now.week))).min().unwrap_or(Level::Now);
        if now.slot {
            level = level.max(read(self.cell(row, Column::Slot)));
        }
        if now.dnd {
            level = level.max(read(self.cell(row, Column::Dnd)));
        }
        level
    }

    /// Someone on a channel now: the blocked never (blocked beats Always
    /// through); on the Always through list, that row, its = read as their own
    /// row, its ☆ as a layer that does not hold them; else their own row.
    pub fn person(&self, channel: Channel, person: Person, always: bool, now: &Now) -> Level {
        if person == Person::Blocked {
            return Level::Never;
        }
        let own = Row::People(channel, if person == Person::Always { Person::Safe } else { person });
        if !always && person != Person::Always {
            return self.level(own, now);
        }
        let always_row = Row::People(channel, Person::Always);
        let at = |column: Column| match self.lent(always_row, column, now.week) {
            Level::As => self.lent(own, column, now.week),
            Level::Through => Level::Now,
            level => level,
        };
        let mut level = now.times.iter().map(|c| at(*c)).min().unwrap_or(Level::Now);
        // "Nothing at all" holds the states' rows: what Always through reads of them too.
        if now.nothing && now.is(Column::Free) && self.cell(always_row, Column::Free) == Level::As {
            level = level.max(Level::Later);
        }
        for (holds, column) in [(now.slot, Column::Slot), (now.dnd, Column::Dnd)] {
            if holds {
                level = level.max(at(column));
            }
        }
        level
    }

    /// A call after the fact (declined, on the Porch's list; a missed call's
    /// notification): when its caller's Calls row lets them through now
    /// (docs/attention.md, Q23); a row that rings at no time at all
    /// (strangers, as usual) in work and admin time, the Porch's own hours,
    /// so that no call is never listed. The blocked never.
    pub fn listed(&self, person: Person, always: bool, now: &Now) -> bool {
        if person == Person::Blocked {
            return false;
        }
        if self.person(Channel::Calls, person, always, now) == Level::Now {
            return true;
        }
        let row = Row::People(Channel::Calls, if person == Person::Always { Person::Safe } else { person });
        let never_rings = !always && Column::TIMES.iter().all(|c| self.cell(row, *c) != Level::Now);
        never_rings && !now.dnd && now.times.iter().any(|c| matches!(c, Column::Work | Column::Admin))
    }

    /// Whether a person's row lets them through at some time of the week
    /// that an area is for: when none, the area holds nothing back for good.
    fn meets(&self, channel: Channel, person: Person, area: Area, week: Week) -> bool {
        Time::STATES.into_iter().filter(|t| week.has(*t)).any(|t| {
            let column = match t {
                Time::Work => Column::Work,
                Time::Admin => Column::Admin,
                Time::Leisure => Column::Leisure,
                Time::Meals => Column::Meals,
                _ => Column::Sleep,
            };
            matches!(self.lent(Row::People(channel, person), column, week), Level::Now | Level::Quiet) && in_view(area, t, week)
        })
    }
}

/// One word of a row: "sleep" (at once) or "sleep:later"; none when either
/// half is not understood.
pub fn word(text: &str) -> Option<(Column, Level)> {
    match text.rsplit_once(':') {
        Some((column, value)) => Some((Column::read(column)?, Level::read(value)?)),
        None => Some((Column::read(text)?, Level::Now)),
    }
}

// ---------------------------------------------------------------- the pipeline

/// What happened, as its caller knows it (docs/attention.md, §2, step 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub source: Source,
    /// What its source is for (an address, an app, a site): it waits outside
    /// that area's times, but for your safe senders and Always through.
    pub area: Option<Area>,
    /// An event's own reminder: when its event begins (Unix seconds), for ◐.
    pub starts: Option<i64>,
    /// The sources' own rows it reads, the most precise first (`source_rows`:
    /// another app's notification, its conversation's and its app's).
    pub sources: Vec<String>,
}

impl Event {
    pub fn own(kind: Kind) -> Event {
        Event { source: Source::Own(kind), area: None, starts: None, sources: Vec::new() }
    }

    pub fn of(source: Source) -> Event {
        Event { source, area: None, starts: None, sources: Vec::new() }
    }

    /// Read with these sources' own rows over its own (`source_rows`).
    pub fn from_sources(mut self, sources: Vec<String>) -> Event {
        self.sources = sources;
        self
    }

    /// For an area's times only.
    pub fn for_area(mut self, area: Option<Area>) -> Event {
        self.area = area;
        self
    }

    /// An event's own reminder, its event starting then.
    pub fn starting(mut self, at: Option<i64>) -> Event {
        self.starts = at;
        self
    }
}

/// Where an event comes from, with what decides it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Sioul's own, and the automated sources: a kind's row.
    Own(Kind),
    /// A message come to one of your addresses. `who` as the Porch weighs it:
    /// set aside or unproven, a stranger's (a blocked sender stays blocked);
    /// `always`, on the Always through list.
    Mail { who: Who, always: bool, lane: Lane },
    /// A phone call; `who` none for a hidden number. `emergency`: an
    /// emergency number or its callback, or within a day of calling one;
    /// `repeat`: a second call within 15 minutes.
    Call { who: Option<Who>, always: bool, emergency: bool, repeat: bool },
    /// Another app's message between people (a phone): its channel (a mail
    /// app's: mail; a missed call: calls), who wrote, a group.
    Message { via: Channel, who: Who, group: bool, always: bool },
    /// A site's notification (a computer): a call in it; in real time; a
    /// person a chat site names, as your cards know their name.
    Site { call: bool, live: bool, named: Option<Who> },
}

/// What the Porch's lanes decide of mail (docs/attention.md, §2, step 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Told as its row says.
    Usual,
    /// A code asked for: on top of the Porch at any time; told as the Codes row says.
    Code,
    /// What you sent yourself: among People at any hour, never told.
    Own,
    /// Never told, shown as its row says: set aside, the review queue,
    /// hostile, less important accounts, newsletters unless asked, read already.
    Untold,
}

/// Which step answered (docs/attention.md, §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// An emergency number, or within a day of calling one (calls).
    Emergency,
    /// Blocked: never, whatever else says.
    Blocked,
    /// A lane, a code.
    Content,
    /// A floor: a second call within 15 minutes, Let every call through.
    Floor,
    /// The matrix, the time and its layers.
    Matrix,
    /// A source's own row, an app's or a conversation's, as you set it.
    Chosen,
    /// What its source is for holds it: work's things in your evening.
    Area,
    /// A named hold: the Porch resting after a pause, a meeting, the chat limit.
    Hold,
}

/// What comes of an event now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Output {
    /// The level now, its event resolved (never ◐, =, ☆).
    pub level: Level,
    /// Shown in Sioul now (the Porch, a page).
    pub shown: bool,
    /// Told now: a notification, let through, rings.
    pub told: bool,
    /// Told through the system's do-not-disturb: critical urgency on a
    /// computer, a channel that passes on a phone (codes, doses, alarms,
    /// Always through).
    pub pierce: bool,
    pub step: Step,
}

impl Output {
    fn at(level: Level, step: Step) -> Output {
        Output { level, shown: level != Level::Later, told: level == Level::Now, pierce: false, step }
    }
}

impl Attention {
    /// What comes of an event now, step by step (docs/attention.md, §2):
    /// the blocked first, then the content, the floors, the exceptions, the
    /// matrix, what the source is for, the named holds. Which device tells it
    /// and how stays the caller's (§8).
    pub fn decide(&self, event: &Event, now: &Now) -> Output {
        match &event.source {
            Source::Own(kind) => self.own(*kind, event, now),
            Source::Mail { who, always, lane } => self.letter(*who, *always, *lane, event, now),
            Source::Call { who, always, emergency, repeat } => self.call(*who, *always, *emergency, *repeat, now),
            Source::Message { via, who, group, always } => self.message(*via, *who, *group, *always, event, now),
            Source::Site { call, live, named } => self.site(*call, *live, *named, event, now),
        }
    }

    /// A message as the Porch judged it, now: its sender's row as the Porch
    /// weighs it (set aside or unproven, a stranger's), Always through when
    /// the list holds its proven address, what the address it came to is for.
    /// Shown says whether the Porch shows it now, told whether new mail's
    /// notification may name it (its lane aside: `mailnote::never`).
    pub fn mail(&self, t: &crate::porch::Triaged, senders: &crate::porch::Senders, always: &crate::everywhere::People, area: Area, now: &Now) -> Output {
        use crate::porch::{Lane as PorchLane, Reason};
        let unproven = t.lane == PorchLane::SetAside || t.reasons.contains(&Reason::NotAuthenticated);
        let who = if unproven { Who::Stranger } else { senders.who_of(&t.card) };
        let always = !unproven && t.card.from_address.as_deref().is_some_and(|address| always.admits_address(address));
        let lane = if t.lane == PorchLane::RightNow {
            Lane::Code
        } else if t.reasons.contains(&Reason::FromYourself) {
            Lane::Own
        } else {
            Lane::Usual
        };
        self.decide(&Event::of(Source::Mail { who, always, lane }).for_area(Some(area)), now)
    }

    fn own(&self, kind: Kind, event: &Event, now: &Now) -> Output {
        // Another app's, read with its app's and conversation's own rows over the kind's.
        let sources = self.source_cells(&event.sources);
        let (mut level, chosen) = if sources.is_empty() { (self.level(Row::Own(kind), now), false) } else { self.over(Base::Row(Row::Own(kind)), &sources, now) };
        if level == Level::Event {
            level = if event.starts.is_some_and(|s| now.span.0 <= s && s < now.span.1) { Level::Now } else { Level::Later };
        }
        let mut step = if chosen { Step::Chosen } else { Step::Matrix };
        // What a thing is for, not the matrix: work's dates wait while work rests, in
        // the hours' times (asleep, paused or in Free time, the cells say it already).
        // A source's cell set for this time is your word for it: no area holds it.
        let hours = !now.times.iter().any(|c| matches!(c, Column::Sleep | Column::Pause | Column::Free));
        if !chosen && hours && matches!(level, Level::Now | Level::Gathered) && event.area.is_some_and(|area| !in_view(area, now.time, now.week)) {
            (level, step) = (Level::Later, Step::Area);
        }
        // In a meeting: no meal's notice, no "Work hours are over".
        if now.holds.meeting && matches!(kind, Kind::Needs | Kind::WorkOver) && level == Level::Now {
            (level, step) = (Level::Never, Step::Hold);
        }
        let told = level == Level::Now || (level == Level::Gathered && now.gathering);
        Output { level, shown: true, told, pierce: told && matches!(kind, Kind::Codes | Kind::Doses | Kind::Wake | Kind::Alarms), step }
    }

    fn letter(&self, who: Who, always: bool, lane: Lane, event: &Event, now: &Now) -> Output {
        // Step 6, content: a code has its own row, what you sent yourself is shown and never told.
        match lane {
            Lane::Code => {
                let told = self.level(Row::Own(Kind::Codes), now) == Level::Now;
                return Output { level: if told { Level::Now } else { Level::Quiet }, shown: true, told, pierce: told, step: Step::Content };
            }
            Lane::Own => return Output { level: Level::Quiet, shown: true, told: false, pierce: false, step: Step::Content },
            _ => {}
        }
        let person = Person::of(who);
        if person == Person::Blocked {
            return Output { level: Level::Never, shown: false, told: false, pierce: false, step: Step::Blocked };
        }
        let mut level = self.person(Channel::Mail, person, always, now);
        let mut step = Step::Matrix;
        // Step 10: mail to an address for another time waits, but your safe
        // senders' and Always through's; and but when its row and the address
        // never meet in your week, so that nothing waits for good.
        if let Some(area) = event.area
            && matches!(level, Level::Now | Level::Quiet)
            && person != Person::Safe
            && !always
            && !in_view(area, now.time, now.week)
            && self.meets(Channel::Mail, person, area, now.week)
        {
            (level, step) = (Level::Later, Step::Area);
        }
        let mut out = Output::at(level, step);
        out.told = level == Level::Now && lane == Lane::Usual;
        if out.told && now.holds.porch_rests {
            (out.told, out.step) = (false, Step::Hold);
        }
        out.pierce = out.told && always;
        out
    }

    fn call(&self, who: Option<Who>, always: bool, emergency: bool, repeat: bool, now: &Now) -> Output {
        let rings = |step: Step| Output { level: Level::Now, shown: true, told: true, pierce: true, step };
        if emergency {
            return rings(Step::Emergency);
        }
        let person = who.map_or(Person::Hidden, Person::of);
        if person == Person::Blocked {
            return Output { level: Level::Never, shown: false, told: false, pierce: false, step: Step::Blocked };
        }
        if now.through || (repeat && who.is_some()) {
            return rings(Step::Floor);
        }
        let level = self.person(Channel::Calls, person, always && who.is_some(), now);
        let mut out = Output::at(level, Step::Matrix);
        out.pierce = out.told && always;
        out
    }

    fn message(&self, via: Channel, who: Who, group: bool, always: bool, event: &Event, now: &Now) -> Output {
        let person = if who == Who::Blocked { Person::Blocked } else if group && via == Channel::Messages { Person::Groups } else { Person::of(who) };
        if person == Person::Blocked {
            return Output { level: Level::Never, shown: false, told: false, pierce: false, step: Step::Blocked };
        }
        // Read with its app's and conversation's own rows over the person's, when set; someone
        // Always through stays so whatever an app's row says: only a conversation's own row changes them.
        let sources = if always { self.source_cells(&event.sources.iter().filter(|id| id.starts_with(CONVERSATION_ROW)).cloned().collect::<Vec<_>>()) } else { self.source_cells(&event.sources) };
        // A missed call: as a declined call is listed, by the Calls row.
        let (mut level, chosen) = if !sources.is_empty() {
            self.over(Base::Person(via, person, always), &sources, now)
        } else if via == Channel::Calls {
            (if self.listed(person, always, now) { Level::Now } else { Level::Later }, false)
        } else {
            (self.person(via, person, always, now), false)
        };
        // A phone cannot show what it holds: shown, not told, is held.
        if level == Level::Quiet {
            level = Level::Later;
        }
        let mut step = if chosen { Step::Chosen } else { Step::Matrix };
        if !chosen && level == Level::Now && person != Person::Safe && !always && event.area.is_some_and(|area| !in_view(area, now.time, now.week)) {
            (level, step) = (Level::Later, Step::Area);
        }
        let mut out = Output::at(level, step);
        out.pierce = out.told && always;
        out
    }

    fn site(&self, call: bool, live: bool, named: Option<Who>, event: &Event, now: &Now) -> Output {
        let kind = if call {
            Kind::SiteCalls
        } else if live || now.realtime {
            Kind::SitesLive
        } else {
            Kind::Sites
        };
        let mut level = self.level(Row::Own(kind), now);
        let mut step = Step::Matrix;
        // A chat site naming someone your cards know: their messages' row, which may only hold more.
        if let Some(who) = named {
            let theirs = self.person(Channel::Messages, Person::of(who), false, now);
            if theirs > level {
                level = if theirs == Level::Never { Level::Never } else { Level::Later };
                step = if who == Who::Blocked { Step::Blocked } else { Step::Matrix };
            }
        }
        if matches!(level, Level::Now | Level::Gathered) && event.area.is_some_and(|area| !in_view(area, now.time, now.week)) {
            (level, step) = (Level::Later, Step::Area);
        }
        if matches!(level, Level::Now | Level::Gathered) && now.holds.chats {
            (level, step) = (Level::Later, Step::Hold);
        }
        let told = level == Level::Now || (level == Level::Gathered && now.gathering);
        Output { level, shown: level != Level::Never, told, pierce: false, step }
    }
}

// ---------------------------------------------------------------- what the system lets through

/// Whom one of Sioul's modes lets through the system's do-not-disturb, on
/// a phone (Android names nobody, starred contacts, contacts, or anyone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Senders {
    None,
    Starred,
    Contacts,
    Anyone,
}

/// What one of Sioul's modes lets through (Android's `ZenPolicy`), from the
/// matrix at its column: the phone's mode follows the matrix as far as
/// Android lets it (docs/attention.md, §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Silence {
    pub calls: Senders,
    pub messages: Senders,
    /// A second call from the same number within 15 minutes.
    pub repeat: bool,
    /// Conversations marked important in Android (Always through conversations).
    pub conversations: bool,
    /// Android's own alarms, always.
    pub alarms: bool,
    /// Sioul's doses, on their own channel.
    pub doses: bool,
    /// An event's alarms, on a channel of their own that passes.
    pub events: bool,
}

/// What this phone does itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Phone {
    /// It screens calls: Sioul has declined what the matrix holds already.
    pub screens: bool,
}

impl Attention {
    /// What a mode lets through at a moment (`Now`): as `level` reads it,
    /// the times together the least strict, a layer (do-not-disturb over the
    /// time now) the stricter, "Nothing at all" holding the states' rows.
    /// Calls: where this phone screens them, every contact's, Sioul having
    /// declined the others already; else the starred, Android's nearest to
    /// Always through; anyone's while "Let every call through" holds
    /// (`Now::through`), so that the mode lets ring what the screening lets
    /// through. Messages: the starred when the matrix lets anyone write
    /// then, never wider, though Sioul may hold other apps' notifications:
    /// Android sounds a notification before any listener hears of it
    /// (docs/android.md, "Sound"), so a contact's message let through the
    /// mode, then held by Sioul, would ring once; the mode alone keeps a
    /// held message silent.
    pub fn silence_at(&self, now: &Now, phone: Phone) -> Silence {
        let passes = |channel: Channel, person: Person| self.person(channel, person, false, now) == Level::Now;
        let always = |channel: Channel| self.person(channel, Person::Always, true, now) == Level::Now;
        let senders = |channel: Channel, own: bool| {
            let strangers = [Person::Stranger, Person::Hidden, Person::Groups].iter().any(|p| persons(channel).contains(p) && passes(channel, *p));
            let known = [Person::Safe, Person::Neutral, Person::Restricted].iter().any(|p| passes(channel, *p));
            match (own, strangers, known || always(channel)) {
                (true, true, _) => Senders::Anyone,
                (true, false, true) => Senders::Contacts,
                (false, _, true) | (false, true, _) => Senders::Starred,
                _ => Senders::None,
            }
        };
        let calls = if now.through { Senders::Anyone } else { senders(Channel::Calls, phone.screens) };
        Silence {
            calls,
            messages: senders(Channel::Messages, false),
            repeat: calls != Senders::None,
            conversations: always(Channel::Messages),
            alarms: true,
            doses: self.level(Row::Own(Kind::Doses), now) == Level::Now,
            events: matches!(self.level(Row::Own(Kind::Alarms), now), Level::Now | Level::Event),
        }
    }

    /// The same for columns (the pause's, Free time's, sleep's; a layer, the
    /// switch's): the times among them together, the layers above them, as
    /// `silence_at` reads them, every hours set. A layer alone says what it
    /// lets through by itself.
    pub fn silence(&self, columns: &[Column], nothing: bool, phone: Phone) -> Silence {
        let mut now = Now::time(Column::Leisure);
        now.times = columns.iter().copied().filter(|c| c.is_time()).collect();
        now.slot = columns.contains(&Column::Slot);
        now.dnd = columns.contains(&Column::Dnd);
        now.nothing = nothing;
        self.silence_at(&now, phone)
    }
}

// ---------------------------------------------------------------- presets

/// Where you start from (docs/attention.md, Q19): as Sioul does now, quieter,
/// more reachable. A preset never touches a fixed cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    Usual,
    Quieter,
    Reachable,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Usual, Preset::Quieter, Preset::Reachable];

    pub fn id(self) -> &'static str {
        match self {
            Preset::Usual => "usual",
            Preset::Quieter => "quieter",
            Preset::Reachable => "reachable",
        }
    }

    pub fn read(text: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.id() == text.trim())
    }

    /// Its matrix: the usual, changed as it says.
    pub fn matrix(self) -> Attention {
        use Column::*;
        use Level::*;
        let mut matrix = Attention::usual();
        let changes: &[(Row, &[Column], Level)] = match self {
            Preset::Usual => &[],
            Preset::Quieter => &[
                (Row::People(Channel::Mail, Person::Safe), &[Leisure, Meals], Quiet),
                (Row::People(Channel::Calls, Person::Neutral), &[Admin], Later),
                (Row::People(Channel::Calls, Person::Hidden), &[Admin], Later),
                (Row::People(Channel::Messages, Person::Safe), &[Free], Later),
                (Row::Own(Kind::Dates), &[Leisure, Meals], Later),
                (Row::Own(Kind::SitesLive), &[Leisure, Meals], Later),
            ],
            Preset::Reachable => &[
                (Row::People(Channel::Mail, Person::Neutral), &[Leisure, Meals], Now),
                (Row::People(Channel::Mail, Person::Stranger), &[Leisure], Quiet),
                (Row::People(Channel::Calls, Person::Neutral), &[Leisure, Meals], Now),
                (Row::People(Channel::Calls, Person::Hidden), &[Leisure], Now),
                (Row::People(Channel::Messages, Person::Neutral), &[Leisure, Meals], Now),
                (Row::Own(Kind::AppAutomatons), &[Work, Admin], Now),
            ],
        };
        for (row, columns, level) in changes {
            for column in *columns {
                let _ = matrix.set(*row, *column, *level);
            }
        }
        matrix
    }
}

// ---------------------------------------------------------------- the configuration

/// `[attention]` as written: each row's words, read leniently (a row that is
/// no list of words is left aside, never the whole configuration).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct Settings(pub BTreeMap<String, toml::Value>);

impl Settings {
    /// A row's words, by its id: a list of words, or one text of words
    /// separated by commas; none when the row is not written.
    pub fn words(&self, row: &str) -> Option<Vec<String>> {
        let value = self.0.iter().find(|(key, _)| Row::read(key).is_some_and(|r| r.id() == row)).map(|(_, v)| v)?;
        Some(match value {
            toml::Value::Array(list) => list.iter().filter_map(|w| w.as_str().map(str::to_string)).collect(),
            toml::Value::String(text) => text.split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect(),
            _ => Vec::new(),
        })
    }

    /// The sources' own rows written (`app.<package>`, `conversation.<key>`),
    /// each with its words, as `words` reads a row's.
    pub fn sources(&self) -> Vec<(String, Vec<String>)> {
        self.0
            .iter()
            .filter(|(key, _)| is_source(key.trim()))
            .map(|(key, value)| {
                let words = match value {
                    toml::Value::Array(list) => list.iter().filter_map(|w| w.as_str().map(str::to_string)).collect(),
                    toml::Value::String(text) => text.split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect(),
                    _ => Vec::new(),
                };
                (key.trim().to_string(), words)
            })
            .collect()
    }
}

/// `[notify]` as an older Sioul wrote it (what each kind of notification did
/// at each time, before `[attention]`): each row's words, read once to seed
/// the matrix (`seeded`), never written again.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct NotifySettings(pub BTreeMap<String, toml::Value>);

impl NotifySettings {
    /// A row's words, by its id ("day-before"; "Day_Before" too): a list of
    /// words, or one text of words separated by commas; none when not written.
    pub fn words(&self, row: &str) -> Option<Vec<String>> {
        let value = self.0.iter().find(|(key, _)| key.trim().to_ascii_lowercase().replace('_', "-") == row).map(|(_, v)| v)?;
        Some(match value {
            toml::Value::Array(list) => list.iter().filter_map(|w| w.as_str().map(str::to_string)).collect(),
            toml::Value::String(text) => text.split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect(),
            _ => Vec::new(),
        })
    }
}

/// The older keys `[attention]` replaced, taken out when it is first written.
pub const OLDER: &[&str] = &["reach", "notify", "reminders.doses_in_sleep", "pause.doses", "pause.people", "dnd.people"];

/// The matrix while no `[attention]` is written: the usual, but where the
/// older keys said something else than their own usual (`[reach]`,
/// `[notify]`, the doses' switches, the people on the list getting through):
/// what you changed is kept, the decided changes hold elsewhere.
pub fn seeded(config: &Config) -> Attention {
    let (then, then_usual) = (older(config), older(&Config::default()));
    let mut attention = Attention::usual();
    for row in Row::ALL {
        for column in Column::ALL {
            let (was, usual_then) = (then[row.index()][column.index()], then_usual[row.index()][column.index()]);
            if was != usual_then {
                let _ = attention.set(row, column, was);
            }
        }
    }
    // "The people on my list get through" off: the list changes nothing, on
    // any channel; the pause's "Starred contacts get through" off: nobody
    // through the pause's mode, the list neither.
    let always = |channel: Channel| Row::People(channel, Person::Always);
    if !config.dnd.people {
        for channel in Channel::ALL {
            for column in Column::ALL {
                let _ = attention.set(always(channel), column, Level::As);
            }
        }
    } else if !config.pause.people {
        for channel in [Channel::Calls, Channel::Messages] {
            let _ = attention.set(always(channel), Column::Pause, Level::As);
        }
    }
    attention
}

/// What the older keys made Sioul do, written as cells (docs/attention.md, §5).
fn older(config: &Config) -> [[Level; 9]; 36] {
    use Column::*;
    let mut cells = [[Level::Now; 9]; 36];
    // `[notify]`: each kind's row, as `column[:value]` words.
    let notify = |row: &str, column: Column| -> &'static str {
        let usual = older_notify(row, column);
        let mut value = usual;
        for text in config.notify.words(row).unwrap_or_default() {
            let (col, val) = match text.rsplit_once(':') {
                Some((c, v)) => (Column::read(c), v.trim().to_ascii_lowercase()),
                None => (Column::read(&text), "now".to_string()),
            };
            if col == Some(column)
                && let Some(known) = ["now", "list-any", "list", "event", "gathered", "later", "never"].into_iter().find(|k| *k == val.replace('_', "-"))
            {
                value = known;
            }
        }
        value
    };
    let doses = |column: Column| match column {
        Sleep if config.notify.words("doses").is_none_or(|w| !w.iter().any(|w| w.starts_with("sleep"))) => if config.reminders.doses_in_sleep { "now" } else { "later" },
        Pause if config.notify.words("doses").is_none_or(|w| !w.iter().any(|w| w.starts_with("pause"))) => if config.pause.doses { "now" } else { "later" },
        _ => notify("doses", column),
    };
    let level = |value: &str| match value {
        "now" | "list-any" => Level::Now,
        "event" => Level::Event,
        "gathered" => Level::Gathered,
        "never" => Level::Never,
        _ => Level::Later,
    };
    for kind in Kind::ALL {
        for column in Column::ALL {
            let value = if kind == Kind::Doses { doses(column) } else { notify(kind.id(), column) };
            cells[Row::Own(kind).index()][column.index()] = level(value);
        }
    }
    // `[reach]`: the columns ticked for each row (mail's at its top, calls' and messages' in their tables).
    let reach = &config.reach;
    let before = reach.stranger.is_none();
    let ticks = |words: &[String]| -> [bool; 6] {
        let mut on = [false; 6];
        for column in words.iter().filter_map(|w| Column::read(w)) {
            if let Some(at) = [Work, Admin, Leisure, Meals, Sleep, Pause].iter().position(|c| *c == column) {
                on[at] = true;
            }
        }
        on
    };
    let mail_usual = |person: Person| -> [bool; 6] {
        match person {
            Person::Safe => [true; 6],
            Person::Neutral | Person::Stranger => [true, true, false, false, false, false],
            _ => [true, false, false, false, false, false],
        }
    };
    let mail_row = |set: &Option<Vec<String>>, person: Person| -> [bool; 6] {
        match set {
            None => mail_usual(person),
            Some(words) => {
                let mut on = ticks(words);
                // Written before the five states: the pause was sleep's.
                if before {
                    on[5] = on[4];
                }
                on
            }
        }
    };
    let neutral = mail_row(&reach.neutral, Person::Neutral);
    let mail: BTreeMap<Person, [bool; 6]> = [
        (Person::Safe, mail_row(&reach.safe, Person::Safe)),
        (Person::Neutral, neutral),
        (Person::Restricted, mail_row(&reach.restricted, Person::Restricted)),
        (Person::Stranger, if before { neutral } else { mail_row(&reach.stranger, Person::Stranger) }),
    ]
    .into_iter()
    .collect();
    let calls_usual = |person: Person| -> [bool; 6] {
        match person {
            Person::Safe => [true, true, true, true, false, false],
            Person::Neutral | Person::Hidden => [true, true, false, false, false, false],
            Person::Restricted => [true, false, false, false, false, false],
            _ => [false; 6],
        }
    };
    let row_of = |set: &crate::config::RowSettings, person: Person| match person {
        Person::Safe => set.safe.clone(),
        Person::Neutral => set.neutral.clone(),
        Person::Restricted => set.restricted.clone(),
        Person::Stranger => set.stranger.clone(),
        Person::Hidden => set.hidden.clone(),
        _ => None,
    };
    let times6 = [Work, Admin, Leisure, Meals, Sleep, Pause];
    for person in [Person::Safe, Person::Neutral, Person::Restricted, Person::Stranger] {
        // Mail: ticked and told now, at once; ticked and told later, shown, not told; unticked, later.
        let on = mail[&person];
        let row = Row::People(Channel::Mail, person).index();
        let told = |value: &str| match value {
            "now" => Level::Now,
            "never" => Level::Never,
            _ => Level::Quiet,
        };
        for (at, column) in times6.iter().enumerate() {
            cells[row][column.index()] = if on[at] { told(notify("mail", *column)) } else { Level::Later };
        }
        cells[row][Free.index()] = if person == Person::Safe && on[2] { told(notify("mail", Free)) } else { Level::Later };
        cells[row][Slot.index()] = told(notify("mail", Slot));
        cells[row][Dnd.index()] = match notify("mail", Dnd) {
            "now" => Level::Now,
            "never" => Level::Never,
            _ => Level::Quiet,
        };
        // Messages: their own ticks, else mail's; told now at once, else later.
        let on = row_of(&reach.messages, person).map_or(on, |w| ticks(&w));
        let row = Row::People(Channel::Messages, person).index();
        let now_or_later = |value: &str| if value == "now" { Level::Now } else { Level::Later };
        for (at, column) in times6.iter().enumerate() {
            cells[row][column.index()] = if on[at] { now_or_later(notify("app-people", *column)) } else { Level::Later };
        }
        cells[row][Free.index()] = if person == Person::Safe && on[2] { now_or_later(notify("app-people", Free)) } else { Level::Later };
        cells[row][Slot.index()] = now_or_later(notify("app-people", Slot));
        cells[row][Dnd.index()] = now_or_later(notify("app-people", Dnd));
        if person == Person::Stranger {
            cells[Row::People(Channel::Messages, Person::Groups).index()] = cells[row];
        }
    }
    // Calls: ticked, it rings; unticked, voicemail; Free time the safe at leisure's tick; no layer.
    for person in [Person::Safe, Person::Neutral, Person::Restricted, Person::Stranger, Person::Hidden] {
        let on = row_of(&reach.calls, person).map_or(calls_usual(person), |w| ticks(&w));
        let row = Row::People(Channel::Calls, person).index();
        for (at, column) in times6.iter().enumerate() {
            cells[row][column.index()] = if on[at] { Level::Now } else { Level::Later };
        }
        cells[row][Free.index()] = if person == Person::Safe && on[2] { Level::Now } else { Level::Later };
        cells[row][Slot.index()] = Level::Now;
        cells[row][Dnd.index()] = Level::Now;
    }
    // Always through: the do-not-disturb list. Mail and messages as their own
    // row, through do-not-disturb as `[notify]` said; calls at any time, a floor.
    for channel in Channel::ALL {
        let row = Row::People(channel, Person::Always).index();
        cells[row] = [Level::As; 9];
        if !config.dnd.people {
            continue;
        }
        match channel {
            Channel::Calls => cells[row] = [Level::Now; 9],
            Channel::Mail | Channel::Messages => {
                cells[row][Dnd.index()] = match notify(if channel == Channel::Mail { "mail" } else { "app-people" }, Dnd) {
                    "list" => Level::Through,
                    "list-any" => Level::Now,
                    _ => Level::As,
                };
            }
        }
        // The pause's mode let nobody through: the list's calls neither.
        if !config.pause.people && channel != Channel::Mail {
            cells[row][Pause.index()] = Level::As;
        }
    }
    for channel in Channel::ALL {
        cells[Row::People(channel, Person::Blocked).index()] = [Level::Never; 9];
    }
    cells
}

/// `[notify]`'s usual values, as it had them (notify.rs at 2e9bbf1).
fn older_notify(row: &str, column: Column) -> &'static str {
    use Column::*;
    let hours = matches!(column, Work | Admin | Leisure | Meals);
    let held = matches!(column, Sleep | Pause | Free);
    match row {
        "codes" => if matches!(column, Sleep | Pause) { "never" } else { "now" },
        "doses" | "wake" | "time" => "now",
        "alarms" | "before" => if matches!(column, Sleep | Free) { "event" } else { "now" },
        "day-before" | "dates" => if held { "later" } else { "now" },
        "needs" => if held { "never" } else { "now" },
        "move" => if hours { "now" } else { "never" },
        "work-over" => if column == Work || held { "never" } else { "now" },
        "mail" => match column {
            Dnd => "list",
            _ if hours => "now",
            _ => "later",
        },
        "sites" | "app-automatons" => if hours { "gathered" } else { "later" },
        "sites-live" | "app-at-once" => if hours { "now" } else { "later" },
        "site-calls" => if hours || column == Slot { "now" } else { "later" },
        "app-people" => if column == Dnd { "list" } else { "now" },
        _ => "now",
    }
}

/// A row changed from Settings (`attention.<row>`, its words; the older
/// grids' `notify.<kind>` and the senders' ticks come here too, `reach.<row>`):
/// each word read, a fixed cell or a value not offered there refused, a word
/// not understood refused; the row then written whole, or taken out when it
/// says the usual. The first time, every row the older keys seeded is
/// written, and they are taken out.
pub fn apply(path: &Path, config: &Config, key: &str, value: &SettingValue) -> Result<(), String> {
    let words: &[String] = match value {
        SettingValue::Texts(words) => words,
        SettingValue::Ints(none) if none.is_empty() => &[],
        _ => return Err(format!("{key}: a list of columns expected")),
    };
    let mut attention = Attention::of(config);
    if let Some(rest) = key.strip_prefix("reach.") {
        // The older grid of who may reach you: ticks across work…pause.
        let (channel, person) = rest.split_once('.').unwrap_or(("mail", rest));
        let row = Row::read(&format!("{channel}.{person}")).filter(|r| matches!(r, Row::People(_, p) if p.state())).ok_or_else(|| format!("{key}: no such row"))?;
        let ticked: Vec<Column> = words.iter().filter_map(|w| Column::read(w)).collect();
        for column in [Column::Work, Column::Admin, Column::Leisure, Column::Meals, Column::Sleep, Column::Pause] {
            let comes = matches!(attention.cell(row, column), Level::Now | Level::Quiet);
            let level = match (ticked.contains(&column), comes) {
                (true, true) => continue,
                (true, false) if matches!(row, Row::People(Channel::Mail, _)) && matches!(column, Column::Sleep | Column::Pause) => Level::Quiet,
                (true, false) => Level::Now,
                (false, _) => Level::Later,
            };
            attention.set(row, column, level)?;
        }
        return write(path, config, &attention, &[row]);
    }
    // A source's own row (an app's, a conversation's): its cells, and its name ("name=…").
    if let Some(id) = key.strip_prefix("attention.").filter(|id| is_source(id)) {
        for text in words {
            if let Some(name) = text.strip_prefix(NAME) {
                attention.name_source(id, name);
                continue;
            }
            let (column, level) = word(text).ok_or_else(|| format!("{key}: {text}: not understood"))?;
            attention.set_source(id, column, level)?;
        }
        return write_with(path, config, &attention, &[], &[id.to_string()]);
    }
    let rest = key.strip_prefix("attention.").or_else(|| key.strip_prefix("notify.")).ok_or_else(|| format!("{key}: no such row"))?;
    let row = Row::read(rest).ok_or_else(|| format!("{key}: no such row"))?;
    for text in words {
        let (column, level) = word(text).ok_or_else(|| format!("{key}: {text}: not understood"))?;
        attention.set(row, column, level)?;
    }
    write(path, config, &attention, &[row])
}

/// A preset chosen: every row written as it says (the usual taken out).
pub fn apply_preset(path: &Path, config: &Config, preset: Preset) -> Result<(), String> {
    let matrix = preset.matrix();
    write(path, config, &matrix, &Row::ALL)
}

/// `rows` written as `attention` has them, each whole or taken out when
/// usual; the first time, every row that is not usual too, and the older keys out.
fn write(path: &Path, config: &Config, attention: &Attention, rows: &[Row]) -> Result<(), String> {
    write_with(path, config, attention, rows, &[])
}

/// `write`, with sources' own rows (`sources`, by id) written too, each as
/// set or taken out when as usual everywhere; the other sources' rows stay.
fn write_with(path: &Path, config: &Config, attention: &Attention, rows: &[Row], sources: &[String]) -> Result<(), String> {
    let usual = Attention::usual();
    let first = config.attention.is_none();
    let rows: Vec<Row> = if first { Row::ALL.to_vec() } else { rows.to_vec() };
    let mut said: Vec<(String, Option<Vec<String>>)> = rows
        .iter()
        .filter(|r| !matches!(r, Row::People(_, Person::Blocked)))
        .map(|row| {
            let same = Column::ALL.iter().all(|c| attention.cell(*row, *c) == usual.cell(*row, *c));
            (row.id(), (!same).then(|| attention.words(*row)))
        })
        .collect();
    for id in sources {
        let words = attention.source_words(id);
        said.push((id.clone(), (!words.is_empty()).then_some(words)));
    }
    crate::config::set_table(path, "attention", &said, if first { OLDER } else { &[] })
}

// ---------------------------------------------------------------- time for you

/// The slots file, in the state folder: this device's own.
pub const SLOTS_FILE: &str = "slots.toml";

/// Today's slots of time for you (docs/capacity.md, G18b), as the window
/// that laid out the day wrote them: every process reads them, so that Time
/// for you holds everywhere.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slots {
    /// The day they are for; none, no slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<jiff::civil::Date>,
    /// From, to, Unix seconds.
    #[serde(default)]
    pub slots: Vec<(i64, i64)>,
}

impl Slots {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join(SLOTS_FILE)
    }

    /// As kept; none when missing or unreadable.
    pub fn load(path: &Path) -> Slots {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written whole beside, then put in place, only when it changed.
    pub fn save(&self, path: &Path) -> Result<bool, String> {
        if Slots::load(path) == *self && path.exists() {
            return Ok(false);
        }
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let fresh = path.with_extension(format!("toml.{}", std::process::id()));
        std::fs::write(&fresh, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)?;
        std::fs::rename(&fresh, path).map_err(fail)?;
        Ok(true)
    }

    /// Whether `at` is in one of the slots of its own day.
    pub fn at(&self, at: &jiff::Zoned) -> bool {
        self.day == Some(at.date()) && self.holds(at.timestamp().as_second())
    }

    /// Whether `stamp` (Unix seconds) is in one of the slots.
    pub fn holds(&self, stamp: i64) -> bool {
        self.slots.iter().any(|&(from, to)| from <= stamp && stamp < to)
    }

    /// Where Time for you begins or ends (Unix seconds): where what may come can change.
    pub fn edges(&self) -> Vec<i64> {
        let mut edges: Vec<i64> = self.slots.iter().flat_map(|&(from, to)| [from, to]).collect();
        edges.sort_unstable();
        edges.dedup();
        edges
    }
}

/// Whether do-not-disturb holds now from its switch or a focus session, as
/// the shared files say (`everywhere`): its layer, for what runs without the
/// window (`sioul remind --watch`, `sioul watch`, a phone's alarms and listener).
pub fn dnd_from_files(config: &Config, now: i64) -> bool {
    use crate::everywhere::{Sources, Switch};
    let switch = Switch::load(&Switch::default_path());
    let focus = crate::timelog::running().and_then(|r| crate::everywhere::focus(&r, now));
    let sources = Sources { focus, ..Sources::default() };
    crate::everywhere::now(&config.dnd, &sources, switch.latest().as_ref(), now).gates()
}

// ---------------------------------------------------------------- words

/// An id and its words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Named {
    pub id: String,
    pub label: String,
}

/// A value, its mark, and its words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mark {
    pub id: String,
    pub mark: String,
    pub label: String,
}

/// A row as Settings draws it: its words, its group's, a sentence on it, the
/// values its cells may take in its own words, its cells, and whether it
/// differs from the preset it is compared with.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GridRow {
    pub id: String,
    pub label: String,
    pub group: String,
    /// The group's id: "mail", "calls", "messages", "asked", "reminders", "day", "sites", "apps".
    pub group_id: String,
    pub help: String,
    pub choices: Vec<Mark>,
    pub cells: Vec<GridCell>,
    /// A cell differs from the preset compared with.
    pub changed: bool,
}

/// A cell: its column, its value, those it may take; why it is fixed, when it
/// is; its value differs from the preset's; and the cell in a sentence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GridCell {
    pub column: String,
    pub value: String,
    pub choices: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub locked: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub changed: bool,
    pub said: String,
}

/// A preset, and how far the matrix is from it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PresetView {
    pub id: String,
    pub label: String,
    /// The matrix is this one.
    pub current: bool,
    /// Cells that differ from it.
    pub changes: usize,
}

/// The matrix in words, as Settings draws it: the columns (a layer flagged),
/// the levels with their marks (the legend), the rows, the presets, and the
/// moment now in a sentence when one is given.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Grid {
    pub columns: Vec<Named>,
    /// The two layers' ids, after the seven times.
    pub layers: Vec<String>,
    pub marks: Vec<Mark>,
    pub rows: Vec<GridRow>,
    pub presets: Vec<PresetView>,
    /// "Changes from As Sioul does now", said; "" when none.
    pub changed: String,
    /// The moment now, in a sentence ("Now: …"); "" when not asked.
    pub now: String,
}

/// A column's words.
pub fn column_label(tr: &Translator, column: Column) -> String {
    tr.text(&format!("attention-column-{}", column.id()), None)
}

/// A row's words: its person's or its kind's.
pub fn row_label(tr: &Translator, row: Row) -> String {
    match row {
        Row::People(_, person) => tr.text(&format!("attention-person-{}", person.id()), None),
        Row::Own(kind) => tr.text(&format!("attention-row-{}", kind.id()), None),
    }
}

/// A value in a row's own words: a code not told stays on the Porch;
/// Always through's at once is whatever their list; mail not at all stays in its lane.
pub fn level_label(tr: &Translator, row: Row, level: Level) -> String {
    let own = match (row, level) {
        (Row::Own(Kind::Codes), Level::Never) => Some("attention-level-never-codes"),
        (Row::People(_, Person::Always), Level::Now) => Some("attention-level-now-always"),
        (Row::People(Channel::Mail, _), Level::Never) => Some("attention-level-never-mail"),
        (Row::People(Channel::Calls, _), Level::Later) => Some("attention-level-later-calls"),
        _ => None,
    };
    tr.text(own.unwrap_or(&format!("attention-level-{}", level.id())), None)
}

/// The grid, in `tr`'s words, of `rows` (all of them, or a part: a
/// channel's, the kinds'), compared with `preset`; `now`, the moment said.
pub fn grid(attention: &Attention, tr: &Translator, rows: &[Row], preset: Preset, now: Option<&Now>) -> Grid {
    let compared = preset.matrix();
    let columns = Column::ALL.iter().map(|c| Named { id: c.id().to_string(), label: column_label(tr, *c) }).collect();
    let marks = Level::ALL.iter().map(|l| Mark { id: l.id().to_string(), mark: l.mark(false).to_string(), label: tr.text(&format!("attention-level-{}", l.id()), None) }).collect();
    let rows: Vec<GridRow> = rows
        .iter()
        .map(|row| {
            let mut offered: Vec<Level> = Vec::new();
            for column in Column::ALL {
                for level in choices(*row, column) {
                    if !offered.contains(level) {
                        offered.push(*level);
                    }
                }
            }
            offered.sort();
            let label = row_label(tr, *row);
            let group = tr.text(&format!("attention-group-{}", row.group()), None);
            let cells: Vec<GridCell> = Column::ALL
                .iter()
                .map(|column| {
                    let value = attention.cell(*row, *column);
                    let locked = lock(*row, *column).map(|id| tr.text(id, None)).unwrap_or_default();
                    let mut args = crate::i18n::args();
                    args.set("row", if matches!(row, Row::People(..)) { format!("{group}, {label}") } else { label.clone() });
                    args.set("column", column_label(tr, *column));
                    args.set("value", level_label(tr, *row, value));
                    let said = tr.text("attention-cell", Some(&args));
                    GridCell {
                        column: column.id().to_string(),
                        value: value.id().to_string(),
                        choices: choices(*row, *column).iter().map(|l| l.id().to_string()).collect(),
                        changed: value != compared.cell(*row, *column),
                        said: if locked.is_empty() { said } else { format!("{said}. {locked}") },
                        locked,
                    }
                })
                .collect();
            let help = match row {
                Row::People(_, person) => tr.text(&format!("attention-person-{}-help", person.id()), None),
                Row::Own(kind) => tr.text(&format!("attention-row-{}-help", kind.id()), None),
            };
            GridRow {
                id: row.id(),
                group: group.clone(),
                group_id: row.group().to_string(),
                help,
                choices: offered.iter().map(|l| Mark { id: l.id().to_string(), mark: l.mark(row.always()).to_string(), label: level_label(tr, *row, *l) }).collect(),
                changed: cells.iter().any(|c| c.changed),
                label,
                cells,
            }
        })
        .collect();
    let presets: Vec<PresetView> = Preset::ALL
        .iter()
        .map(|p| {
            let changes = attention.changes_from(&p.matrix());
            PresetView { id: p.id().to_string(), label: tr.text(&format!("attention-preset-{}", p.id()), None), current: changes == 0, changes }
        })
        .collect();
    let from_usual = attention.changes_from(&Attention::usual());
    let changed = if from_usual == 0 { String::new() } else { tr.text("attention-changes", Some(&tr.counted(from_usual))) };
    Grid {
        columns,
        layers: vec![Column::Slot.id().to_string(), Column::Dnd.id().to_string()],
        marks,
        rows,
        presets,
        changed,
        now: now.map(|n| now_sentence(attention, tr, n)).unwrap_or_default(),
    }
}

/// The rows of a channel, the blocked last.
pub fn channel_rows(channel: Channel) -> Vec<Row> {
    persons(channel).iter().map(|p| Row::People(channel, *p)).collect()
}

/// The kinds' rows: Sioul's own and the automated sources.
pub fn kind_rows() -> Vec<Row> {
    Kind::ALL.iter().map(|k| Row::Own(*k)).collect()
}

/// A source's row to draw (`source_grid`): its id (`app.<package>`,
/// `conversation.<key>`), its words (an app's name; a conversation's title
/// and its app's), and the group it shows in, in words and by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLine {
    pub id: String,
    pub label: String,
    pub group: String,
    pub group_id: String,
}

/// The sources' own rows as Settings draws them (`AttentionGrid.qml`): each
/// a row of the nine columns, its cells as set or as usual, each in a
/// sentence; a cell set carries the grid's dot. The values a source's cell
/// takes, in their own words: at once, at the gathered times, held, as usual.
pub fn source_grid(attention: &Attention, tr: &Translator, lines: &[SourceLine]) -> Grid {
    let columns = Column::ALL.iter().map(|c| Named { id: c.id().to_string(), label: column_label(tr, *c) }).collect();
    let label = |level: Level| tr.text(&format!("attention-source-{}", level.id()), None);
    let choices: Vec<Mark> = SOURCE_CHOICES.iter().map(|l| Mark { id: l.id().to_string(), mark: l.mark(false).to_string(), label: label(*l) }).collect();
    let offered: Vec<String> = SOURCE_CHOICES.iter().map(|l| l.id().to_string()).collect();
    let rows = lines
        .iter()
        .map(|line| {
            let set = attention.source(&line.id).unwrap_or([Level::As; 9]);
            let cells: Vec<GridCell> = Column::ALL
                .iter()
                .map(|column| {
                    let value = set[column.index()];
                    let mut args = crate::i18n::args();
                    args.set("row", line.label.clone());
                    args.set("column", column_label(tr, *column));
                    args.set("value", label(value));
                    GridCell {
                        column: column.id().to_string(),
                        value: value.id().to_string(),
                        choices: offered.clone(),
                        locked: String::new(),
                        changed: value != Level::As,
                        said: tr.text("attention-cell", Some(&args)),
                    }
                })
                .collect();
            GridRow {
                id: line.id.clone(),
                label: line.label.clone(),
                group: line.group.clone(),
                group_id: line.group_id.clone(),
                help: tr.text("attention-source-help", None),
                choices: choices.clone(),
                changed: cells.iter().any(|c| c.changed),
                cells,
            }
        })
        .collect();
    Grid {
        columns,
        layers: vec![Column::Slot.id().to_string(), Column::Dnd.id().to_string()],
        marks: choices,
        rows,
        presets: Vec::new(),
        changed: String::new(),
        now: String::new(),
    }
}

/// A source's row in one sentence: where it comes at once, where at the
/// gathered times, where it is held, and as usual the rest of the time; "As
/// usual at every time." when nothing is set. "At once: Work and Do not
/// disturb; held: Leisure and Sleep; as usual the rest of the time."
pub fn source_sentence(attention: &Attention, tr: &Translator, id: &str) -> String {
    let Some(cells) = attention.source(id) else { return tr.text("attention-source-usual", None) };
    let and = tr.text("word-and", None);
    let mut parts: Vec<String> = [Level::Now, Level::Gathered, Level::Later]
        .iter()
        .filter_map(|level| {
            let columns: Vec<String> = Column::ALL.iter().filter(|c| cells[c.index()] == *level).map(|c| column_label(tr, *c)).collect();
            let columns = match columns.as_slice() {
                [] => return None,
                [one] => one.clone(),
                [rest @ .., last] => format!("{} {and} {last}", rest.join(", ")),
            };
            let mut args = crate::i18n::args();
            args.set("columns", columns);
            Some(tr.text(&format!("attention-source-said-{}", level.id()), Some(&args)))
        })
        .collect();
    if cells.contains(&Level::As) {
        parts.push(tr.text("attention-source-said-as", None));
    }
    let mut args = crate::i18n::args();
    args.set("parts", parts.join(&tr.text("attention-source-said-join", None)));
    let said = tr.text("attention-source-said", Some(&args));
    let mut letters = said.chars();
    letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default()
}

/// The moment now in one sentence, from the matrix: what comes at once, what
/// is shown without a word, what waits. "Now: at once: your safe senders'
/// mail, calls and messages; doses. Waiting: everyone else's."
pub fn now_sentence(attention: &Attention, tr: &Translator, now: &Now) -> String {
    let mut at_once: Vec<String> = Vec::new();
    let mut shown: Vec<String> = Vec::new();
    let mut waiting: Vec<String> = Vec::new();
    for channel in Channel::ALL {
        for person in persons(channel).iter().filter(|p| p.state()) {
            let label = format!("{} · {}", tr.text(&format!("attention-group-{}", channel.id()), None), tr.text(&format!("attention-person-{}", person.id()), None));
            match attention.person(channel, *person, false, now) {
                Level::Now => at_once.push(label),
                Level::Quiet => shown.push(label),
                _ => waiting.push(label),
            }
        }
    }
    for kind in [Kind::Codes, Kind::Doses, Kind::Alarms, Kind::Before, Kind::Dates, Kind::Needs] {
        let label = tr.text(&format!("attention-row-{}", kind.id()), None);
        match attention.level(Row::Own(kind), now) {
            Level::Now | Level::Event => at_once.push(label),
            _ => waiting.push(label),
        }
    }
    let and = tr.text("word-and", None);
    let listed = |items: &[String]| match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {and} {last}", rest.join(", ")),
    };
    let mut parts = Vec::new();
    for (key, items) in [("attention-now-at-once", &at_once), ("attention-now-shown", &shown), ("attention-now-waiting", &waiting)] {
        if !items.is_empty() {
            let mut args = crate::i18n::args();
            args.set("what", listed(items));
            parts.push(tr.text(key, Some(&args)));
        }
    }
    parts.join(" ")
}

/// A state with the times its mail comes or shows, for a list's choice:
/// "Neutral: work, admin", "Safe: any time", "Blocked: never"; `one`: as
/// said of one person (French says "Sûr" of a person, "Sûrs" of the list).
pub fn list_choice(tr: &Translator, who: Who, attention: &Attention, one: bool) -> String {
    let times: Vec<Column> = if who == Who::Blocked {
        Vec::new()
    } else {
        Column::TIMES.iter().copied().filter(|c| *c != Column::Free).filter(|c| matches!(attention.cell(Row::People(Channel::Mail, Person::of(who)), *c), Level::Now | Level::Quiet)).collect()
    };
    let words = if times.len() == 6 {
        tr.text("reach-any", None)
    } else if times.is_empty() {
        tr.text("reach-never", None)
    } else {
        times.iter().map(|c| tr.text(&format!("reach-word-{}", c.id()), None)).collect::<Vec<_>>().join(", ")
    };
    let mut args = crate::i18n::args();
    args.set("list", tr.text(&format!("sender-{}-{}", if one { "one" } else { "list" }, who.id()), None));
    args.set("times", words);
    tr.text("sender-list-times", Some(&args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::Area;
    use crate::quiet::Reason;

    fn mode(time: Time, reason: Reason) -> Mode {
        Mode { quiet: !(time.works() || time == Time::Any), time, week: Week { work_hours: true, admin_hours: true, meals: true, sleep: true }, reason, until: None, back: None, label: String::new() }
    }

    /// Every time `quiet::mode` gives, each with its reason.
    fn modes() -> Vec<Mode> {
        vec![
            mode(Time::Work, Reason::Working),
            mode(Time::Work, Reason::WorkingLate),
            mode(Time::Admin, Reason::AdminTime),
            mode(Time::Several(Area::MIXED), Reason::Working),
            mode(Time::Leisure, Reason::Evening),
            mode(Time::Leisure, Reason::DayOff),
            mode(Time::Leisure, Reason::DoneForTheDay),
            mode(Time::Meals, Reason::Meal),
            mode(Time::Any, Reason::NoHours),
            mode(Time::Sleep, Reason::Sleep),
            mode(Time::Sleep, Reason::WindingDown),
            mode(Time::Sleep, Reason::Nap),
            mode(Time::Leisure, Reason::FreeTime),
            mode(Time::Sleep, Reason::Paused),
        ]
    }

    fn row(id: &str) -> Row {
        Row::read(id).unwrap_or_else(|| panic!("{id}"))
    }

    /// The usual matrix, written out: a test that fails here changes what
    /// Sioul does by default. Marks for brevity, in the columns' order
    /// (work admin leisure meals sleep pause free | slot dnd).
    #[test]
    fn the_usual_matrix_as_decided() {
        let expected = [
            ("mail.always", "●●●●◑◑●●●"),
            ("mail.safe", "●●●●◑◑◑◑◑"),
            ("mail.neutral", "●●○○○○○◑◑"),
            ("mail.restricted", "●○○○○○○◑◑"),
            ("mail.stranger", "●●○○○○○◑◑"),
            ("mail.blocked", "–––––––––"),
            ("calls.always", "●●●●●●●●●"),
            ("calls.safe", "●●●●○○●●○"),
            ("calls.neutral", "●●○○○○○●○"),
            ("calls.restricted", "●○○○○○○●○"),
            ("calls.stranger", "○○○○○○○●○"),
            ("calls.hidden", "●●○○○○○●○"),
            ("calls.blocked", "–––––––––"),
            ("messages.always", "●●●●●●●●●"),
            ("messages.safe", "●●●●○○●●○"),
            ("messages.neutral", "●●○○○○○●○"),
            ("messages.restricted", "●○○○○○○●○"),
            ("messages.stranger", "●●○○○○○●○"),
            ("messages.groups", "●●○○○○○●○"),
            ("messages.blocked", "–––––––––"),
            ("codes", "●●●●●●●●●"),
            ("doses", "●●●●●●●●●"),
            ("wake", "●●●●●●●●●"),
            ("alarms", "●●●●◐●◐●●"),
            ("before", "●●●●◐●◐●●"),
            ("day-before", "●●●●○○○●●"),
            ("dates", "●●●●○○○●●"),
            ("needs", "●●●●–––●●"),
            ("move", "●●●●–––––"),
            ("work-over", "–●●●–––●●"),
            ("time", "●●●●●●●●●"),
            ("sites", "◎◎◎◎○○○○○"),
            ("sites-live", "●●●●○○○○○"),
            ("site-calls", "●●●●○○○●○"),
            ("app-automatons", "◎◎◎◎○○○○○"),
            ("app-at-once", "●●●●○○○○○"),
        ];
        let usual = Attention::usual();
        assert_eq!(expected.len(), Row::ALL.len());
        for (id, marks) in expected {
            let r = row(id);
            let said: String = Column::ALL.iter().map(|c| usual.cell(r, *c).mark(false)).collect();
            assert_eq!(said, marks, "{id}");
            for column in Column::ALL {
                // Each usual value is one its cell may take, fixed cells included; a fixed cell holds its usual.
                assert!(choices(r, column).contains(&usual.cell(r, column)), "{id} {column:?}");
            }
            assert_eq!(r.id(), id, "ids both ways");
        }
        assert_eq!(Attention::of(&Config::default()), usual, "an empty configuration is the usual matrix");
        assert_eq!(Row::read("calls.groups"), None, "calls have no groups' row");
        assert_eq!(Row::read("Day_Before"), Some(Row::Own(Kind::DayBefore)));
    }

    #[test]
    fn fixed_cells_and_choices() {
        let mut a = Attention::usual();
        assert!(a.set(row("doses"), Column::Dnd, Level::Later).is_err(), "a dose comes during do-not-disturb");
        assert!(a.set(row("doses"), Column::Sleep, Level::Never).is_err(), "a dose is never dropped");
        assert!(a.set(row("doses"), Column::Pause, Level::Later).is_ok());
        assert!(a.set(row("codes"), Column::Pause, Level::Never).is_err(), "a code in a pause comes, fixed");
        assert!(a.set(row("codes"), Column::Sleep, Level::Never).is_ok(), "On the Porch only stays a choice for sleep");
        assert!(a.set(row("alarms"), Column::Pause, Level::Later).is_err() && a.set(row("alarms"), Column::Dnd, Level::Later).is_err());
        assert!(a.set(row("mail.blocked"), Column::Work, Level::Now).is_err(), "the blocked never");
        assert!(a.set(row("mail.always"), Column::Work, Level::Through).is_err(), "☆ is a layer's");
        assert!(a.set(row("mail.always"), Column::Dnd, Level::Through).is_ok());
        assert!(a.set(row("calls.safe"), Column::Sleep, Level::Quiet).is_err(), "a phone cannot show a call it holds");
        assert!(a.set(row("mail.neutral"), Column::Leisure, Level::Quiet).is_ok());
        assert!(a.set(row("before"), Column::Pause, Level::Event).is_err(), "a pause has no known end");
        for r in Row::ALL {
            for column in Column::ALL {
                assert!(!choices(r, column).is_empty(), "{r:?} {column:?}");
                if lock(r, column).is_some() {
                    assert_eq!(choices(r, column).first(), Some(&usual(r, column)), "{r:?} {column:?}");
                }
            }
        }
    }

    #[test]
    fn rows_from_the_configuration_and_their_words() {
        let config: Config = toml::from_str(
            "[attention]\n\"mail.neutral\" = [\"leisure\", \"Repas:quiet\", \"sleep:never\", \"nonsense\", \"work:gathered\"]\n\
             codes = \"sleep:never, pause:never\"\n\
             \"calls.groups\" = [\"work\"]\n\
             \"messages.always\" = [\"dnd:through\", \"sleep:as\"]\n\
             day_before = [\"free\"]\n",
        )
        .unwrap();
        let a = Attention::of(&config);
        // Read leniently: French words, a value not offered left aside (gathered mail), a fixed cell kept.
        assert_eq!(a.words(row("mail.neutral")), ["work", "admin", "leisure", "meals:quiet", "sleep:never", "pause:later", "free:later", "slot:quiet", "dnd:quiet"]);
        assert_eq!((a.cell(row("codes"), Column::Sleep), a.cell(row("codes"), Column::Pause)), (Level::Never, Level::Now));
        assert_eq!((a.cell(row("messages.always"), Column::Dnd), a.cell(row("messages.always"), Column::Sleep)), (Level::Through, Level::As));
        assert_eq!(a.cell(row("day-before"), Column::Free), Level::Now);
        // A row that is no list is left aside, never the configuration.
        let odd: Config = toml::from_str("[attention]\n\"mail.safe\" = 3\n").unwrap();
        assert_eq!(Attention::of(&odd), Attention::usual());
        assert_eq!(word("Temps libre:quiet"), Some((Column::Free, Level::Quiet)));
        assert_eq!((word("sleep:soon"), word("night-shift")), (None, None));
    }

    #[test]
    fn times_together_layers_and_lending() {
        let a = Attention::usual();
        // No hours set: work and admin together, the least strict; restricted mail comes (work).
        let any = Now::of(&mode(Time::Any, Reason::NoHours));
        assert_eq!(any.times, vec![Column::Work, Column::Admin]);
        assert_eq!(a.level(row("mail.restricted"), &any), Level::Now);
        // A layer: the strictest wins.
        let work = Now::time(Column::Work).layers(true, false);
        assert_eq!((a.level(row("site-calls"), &work), a.level(row("sites-live"), &work)), (Level::Now, Level::Later));
        assert_eq!(a.level(row("site-calls"), &work.clone().layers(true, true)), Level::Later);
        // Lending: without admin hours, work time takes admin's cell (people's rows only).
        let mut admin_only = Attention::usual();
        admin_only.set(row("mail.restricted"), Column::Admin, Level::Now).unwrap();
        admin_only.set(row("mail.restricted"), Column::Work, Level::Later).unwrap();
        let mut no_admin = Now::time(Column::Work);
        no_admin.week.admin_hours = false;
        assert_eq!((admin_only.level(row("mail.restricted"), &no_admin), admin_only.level(row("mail.restricted"), &Now::time(Column::Work))), (Level::Now, Level::Later));
        let mut kinds = Attention::usual();
        kinds.set(row("dates"), Column::Work, Level::Later).unwrap();
        assert_eq!(kinds.level(row("dates"), &no_admin), Level::Later, "kinds are never lent");
        // The pause and Free time come before the hours; the night before a meal.
        assert_eq!(Now::of(&mode(Time::Sleep, Reason::Paused)).times, vec![Column::Pause]);
        assert_eq!(Now::of(&mode(Time::Leisure, Reason::FreeTime)).times, vec![Column::Free]);
        assert_eq!(Now::of(&mode(Time::Several(Area::ADMIN), Reason::AdminTime)).times, vec![Column::Admin]);
    }

    #[test]
    fn always_through_reads_their_own_row_as_said() {
        let mut a = Attention::usual();
        let leisure = Now::time(Column::Leisure);
        // At once whatever their list: a neutral person on the list, in leisure.
        assert_eq!((a.person(Channel::Mail, Person::Neutral, true, &leisure), a.person(Channel::Mail, Person::Neutral, false, &leisure)), (Level::Now, Level::Later));
        // As their own row: then their row says.
        a.set(row("mail.always"), Column::Leisure, Level::As).unwrap();
        assert_eq!(a.person(Channel::Mail, Person::Neutral, true, &leisure), Level::Later);
        assert_eq!(a.person(Channel::Mail, Person::Safe, true, &leisure), Level::Now);
        // ☆ on a layer: the layer does not hold them; their row's time does.
        a.set(row("mail.always"), Column::Dnd, Level::Through).unwrap();
        let dnd = Now::time(Column::Work).layers(false, true);
        assert_eq!((a.person(Channel::Mail, Person::Restricted, true, &dnd), a.person(Channel::Mail, Person::Restricted, false, &dnd)), (Level::Now, Level::Quiet));
        let dnd_leisure = leisure.clone().layers(false, true);
        assert_eq!(a.person(Channel::Mail, Person::Neutral, true, &dnd_leisure), Level::Later, "☆ at their own row's times");
        // Blocked beats Always through, on every channel.
        for channel in Channel::ALL {
            assert_eq!(a.person(channel, Person::Blocked, true, &leisure), Level::Never);
        }
        // "Nothing at all" holds the states' rows, never Always through.
        let mut free = Now::of(&mode(Time::Leisure, Reason::FreeTime));
        free.nothing = true;
        let usual = Attention::usual();
        assert_eq!((usual.person(Channel::Calls, Person::Safe, false, &free), usual.person(Channel::Calls, Person::Safe, true, &free)), (Level::Later, Level::Now));
    }

    // ------------------------------------------------------------ the oracle: what the code did before

    /// `quiet::mail_in_view` as it was, the matrix as usual: whether mail shows.
    fn shown_before(who: Who, time: Time, paused: bool, free: bool) -> bool {
        if time == Time::Any && !paused && !free {
            return true;
        }
        let ticks: [bool; 6] = match who {
            Who::Safe => [true; 6],
            Who::Neutral | Who::Stranger => [true, true, false, false, false, false],
            Who::Restricted => [true, false, false, false, false, false],
            Who::Blocked => [false; 6],
        };
        if paused {
            return ticks[5];
        }
        if free {
            return who == Who::Safe && ticks[2];
        }
        match time {
            Time::Work => ticks[0],
            Time::Admin => ticks[1],
            Time::Leisure => ticks[2],
            Time::Meals => ticks[3],
            Time::Sleep => ticks[4],
            Time::Several(_) | Time::Any => ticks[0] || ticks[1],
        }
    }

    /// New mail told as it was (`mailnote::Seen::tells`, notify's mail row, the list as usual).
    fn told_before(who: Who, listed: bool, m: &Mode, slot: bool, dnd: bool) -> bool {
        let cell_now = !(m.sleeps() || m.paused() || m.free()) && !slot;
        let grid = shown_before(who, m.time, m.paused(), m.free());
        if !cell_now {
            return false;
        }
        if dnd { grid && listed } else { grid }
    }

    #[test]
    fn the_usual_matrix_decides_as_the_code_did_but_where_decided() {
        let a = Attention::usual();
        for m in modes() {
            for (slot, dnd) in [(false, false), (true, false), (false, true), (true, true)] {
                let now = Now::of(&m).layers(slot, dnd);
                let said = format!("{:?} {:?} slot {slot} dnd {dnd}", m.time, m.reason);
                for who in [Who::Safe, Who::Neutral, Who::Restricted, Who::Stranger] {
                    // Mail shown on the Porch: as before, but no hours set now read as work and admin (the same, as usual).
                    let mail = a.decide(&Event::of(Source::Mail { who, always: false, lane: Lane::Usual }), &now);
                    assert_eq!(mail.shown, shown_before(who, m.time, m.paused(), m.free()), "shown {who:?} {said}");
                    assert_eq!(mail.told, told_before(who, false, &m, slot, dnd), "told {who:?} {said}");
                    // The list's people (Q2): before, told under do-not-disturb when their row let them through,
                    // else as their row; now at once whatever their row, but shown, not told, in sleep and a pause.
                    let listed = a.decide(&Event::of(Source::Mail { who, always: true, lane: Lane::Usual }), &now);
                    assert!(!told_before(who, true, &m, slot, dnd) || listed.told, "nothing told before is held now: {who:?} {said}");
                    assert_eq!(listed.told, !(m.sleeps() || m.paused()), "listed {who:?} {said}");
                    assert_eq!(listed.pierce, listed.told, "Always through mail passes the system's do-not-disturb {said}");
                }
                // Kinds of Sioul's own, as notify's usual decided, but codes at once in sleep and a pause (Q6).
                let comes = |kind: Kind| a.decide(&Event::own(kind), &now).told;
                assert!(comes(Kind::Codes), "codes {said}");
                assert!(comes(Kind::Doses) && comes(Kind::Wake) && comes(Kind::Time), "{said}");
                let quiet_ish = m.sleeps() || m.free() || m.paused();
                assert_eq!(comes(Kind::Move), !quiet_ish && !slot && !dnd, "move {said}");
                assert_eq!(comes(Kind::Needs), !quiet_ish, "needs {said}");
                assert_eq!(comes(Kind::SitesLive), !quiet_ish && !slot && !dnd, "sites live {said}");
                assert_eq!(comes(Kind::SiteCalls), !quiet_ish && !dnd, "site calls {said}");
            }
        }
    }

    #[test]
    fn messages_calls_and_the_decided_changes() {
        let a = Attention::usual();
        let night = Now::of(&mode(Time::Sleep, Reason::Sleep));
        let paused = Now::of(&mode(Time::Sleep, Reason::Paused));
        let free = Now::of(&mode(Time::Leisure, Reason::FreeTime));
        let message = |who: Who, always: bool, now: &Now| a.decide(&Event::of(Source::Message { via: Channel::Messages, who, group: false, always }), now);
        // Q4: a safe friend's message at 03:00 waits for waking (before: let through); in Free time it comes.
        assert!(!message(Who::Safe, false, &night).told && !message(Who::Safe, false, &paused).told);
        assert!(message(Who::Safe, false, &free).told);
        // Q2: Always through: messages and calls at once, sleep and pauses included.
        assert!(message(Who::Neutral, true, &night).told && message(Who::Neutral, true, &paused).told);
        // Q3: blocked beats Always through.
        assert_eq!(message(Who::Blocked, true, &night).step, Step::Blocked);
        let call = |who: Option<Who>, always: bool, now: &Now| a.decide(&Event::of(Source::Call { who, always, emergency: false, repeat: false }), now);
        assert!(call(Some(Who::Blocked), true, &Now::time(Column::Work)).level == Level::Never);
        assert!(call(Some(Who::Stranger), true, &night).told, "on the list, at any time");
        // Q11: calls carry the layers: a safe caller under the switch goes to voicemail, the list's rings.
        let dnd = Now::time(Column::Leisure).layers(false, true);
        assert!(!call(Some(Who::Safe), false, &dnd).told && call(Some(Who::Safe), true, &dnd).told);
        // Floors after the blocked: Let every call through, a second call.
        let mut through = night.clone();
        through.through = true;
        assert!(call(Some(Who::Stranger), false, &through).told && !call(Some(Who::Blocked), false, &through).told);
        let repeat = a.decide(&Event::of(Source::Call { who: None, always: false, emergency: false, repeat: true }), &night);
        assert!(!repeat.told, "hidden numbers cannot be told apart: no second call");
        assert!(a.decide(&Event::of(Source::Call { who: Some(Who::Blocked), always: false, emergency: true, repeat: false }), &night).told, "an emergency number rings, even blocked");
        // Q5: a mail app's notification takes the Mail rows, shown-not-told held: a safe sender at 03:00 waits.
        let mail_app = a.decide(&Event::of(Source::Message { via: Channel::Mail, who: Who::Safe, group: false, always: false }), &night);
        assert_eq!((mail_app.level, mail_app.told), (Level::Later, false));
        // Q22: groups have a row of their own, as strangers until changed.
        let mut groups = Attention::usual();
        groups.set(row("messages.groups"), Column::Work, Level::Later).unwrap();
        let work = Now::time(Column::Work);
        let group = groups.decide(&Event::of(Source::Message { via: Channel::Messages, who: Who::Safe, group: true, always: false }), &work);
        let stranger = groups.decide(&Event::of(Source::Message { via: Channel::Messages, who: Who::Stranger, group: false, always: false }), &work);
        assert!(!group.told && stranger.told);
        // Q15: Free time as cells: a neutral row's Free cell let through (before: code, the safe only).
        let mut cells = Attention::usual();
        cells.set(row("mail.neutral"), Column::Free, Level::Now).unwrap();
        assert!(cells.decide(&Event::of(Source::Mail { who: Who::Neutral, always: false, lane: Lane::Usual }), &free).told);
        // Q14: no hours set: mail reads work and admin; a neutral row held in both is held (before: everything came).
        let mut held = Attention::usual();
        held.set(row("mail.neutral"), Column::Work, Level::Later).unwrap();
        held.set(row("mail.neutral"), Column::Admin, Level::Later).unwrap();
        assert!(!held.decide(&Event::of(Source::Mail { who: Who::Neutral, always: false, lane: Lane::Usual }), &Now::of(&mode(Time::Any, Reason::NoHours))).shown);
        // Q13: a chat site naming someone your cards know: their row, which may only hold more.
        let site = |named: Option<Who>, now: &Now| a.decide(&Event::of(Source::Site { call: false, live: true, named }), now);
        assert!(site(None, &Now::time(Column::Leisure)).told);
        assert!(!site(Some(Who::Neutral), &Now::time(Column::Leisure)).told, "a neutral person in your leisure waits");
        assert!(site(Some(Who::Safe), &Now::time(Column::Leisure)).told);
        assert_eq!(site(Some(Who::Blocked), &Now::time(Column::Work)).level, Level::Never);
    }

    /// A message from `from` as the Porch judges it; `headers` before the subject.
    fn message(from: &str, headers: &str, subject: &str, senders: &crate::porch::Senders) -> crate::porch::Triaged {
        let raw = format!("From: {from}\r\n{headers}Subject: {subject}\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\n{subject}.\r\n");
        let known = crate::porch::SenderList::default();
        let trusted = ["mx.example.net".to_string()];
        let own = ["me@example.net".to_string()];
        let ctx = crate::porch::Context { cases: None, known: &known, senders, trusted_ids: &trusted, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, words: None, own_addresses: &own, spam: None };
        crate::porch::triage(crate::card::Card::from_bytes(raw.as_bytes()).unwrap(), &ctx)
    }

    /// The Porch's mail by who wrote and where it came (what `quiet::mail_in_view` decided before).
    #[test]
    fn mail_shows_by_who_wrote_and_where_it_came() {
        use crate::porch::{SenderList, Senders};
        let usual = Attention::usual();
        let senders = Senders { safe: SenderList::parse("jane@example.org"), restricted: SenderList::parse("*@company.example"), ..Senders::default() };
        let set = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        let nobody = crate::everywhere::People::default();
        let at = |time: Time, week: Week| Now::of(&Mode { quiet: false, time, week, reason: if time == Time::Sleep { Reason::Sleep } else { Reason::Working }, until: None, back: None, label: String::new() });
        let comes_in = |t: &crate::porch::Triaged, matrix: &Attention, account: Area, time: Time, week: Week| matrix.mail(t, &senders, &nobody, account, &at(time, week)).shown;
        let comes = |t: &crate::porch::Triaged, matrix: &Attention, account: Area, time: Time| comes_in(t, matrix, account, time, set);
        let at_times = |t: &crate::porch::Triaged, matrix: &Attention, account: Area| Time::STATES.into_iter().filter(|time| comes(t, matrix, account, *time)).map(Time::id).collect::<Vec<_>>();
        // Safe: to any address, at every time (in sleep shown, not told); held in sleep, it waits.
        let jane = message("Jane <jane@example.org>", "", "Hello", &senders);
        assert!(Time::STATES.into_iter().all(|time| comes(&jane, &usual, Area::WORK, time)));
        let mut no_sleep = Attention::usual();
        no_sleep.set(row("mail.safe"), Column::Sleep, Level::Later).unwrap();
        assert!(!comes(&jane, &no_sleep, Area::WORK, Time::Sleep) && comes(&jane, &no_sleep, Area::WORK, Time::Meals));
        // A stranger, to the work address: work time only (the address is work's, the row says work and admin).
        let stranger = message("Someone <someone@elsewhere.example>", "", "A question", &senders);
        assert_eq!(senders.who_of(&stranger.card), Who::Stranger);
        assert_eq!(at_times(&stranger, &usual, Area::WORK), ["work"]);
        assert_eq!(at_times(&stranger, &usual, Area::PERSONAL), ["admin"]);
        // The address and the row never meet: the row alone decides, so nothing waits for good.
        assert_eq!(at_times(&stranger, &usual, Area::LEISURE), ["work", "admin"]);
        // Jane's address on mail failing SPF and DKIM: nothing proves it is hers, a stranger's times.
        let unproven = message("Jane <jane@example.org>", "Authentication-Results: mx.example.net; spf=fail smtp.mailfrom=example.org; dkim=fail header.d=example.org\r\n", "Hello", &senders);
        assert_eq!(at_times(&unproven, &usual, Area::WORK), ["work"]);
        // Strangers held at every time: they wait while the neutral come.
        let mut no_strangers = Attention::usual();
        for column in [Column::Work, Column::Admin] {
            no_strangers.set(row("mail.stranger"), column, Level::Later).unwrap();
        }
        assert_eq!(at_times(&stranger, &no_strangers, Area::WORK), Vec::<&str>::new());
        let boss = message("Boss <boss@company.example>", "", "Monday", &senders);
        assert_eq!(at_times(&boss, &usual, Area::PERSONAL), ["work"], "restricted to work, writing to a personal address");
        assert_eq!(at_times(&boss, &usual, Area::WORK), ["work"]);
        // Without admin hours, work time takes admin's cells; without work hours, admin time takes work's.
        let mut admin_only = Attention::usual();
        admin_only.set(row("mail.stranger"), Column::Work, Level::Later).unwrap();
        assert!(comes_in(&stranger, &admin_only, Area::MIXED, Time::Work, Week { admin_hours: false, ..set }));
        assert!(!comes_in(&stranger, &admin_only, Area::MIXED, Time::Work, set));
        assert!(comes_in(&boss, &usual, Area::WORK, Time::Admin, Week { work_hours: false, ..set }));
        // Codes asked for and what you send yourself: shown asleep too; no hours set: work and admin together.
        let code = message("Bank <codes@bank.example>", "", "Your verification code: 482913", &senders);
        assert_eq!(code.lane, crate::porch::Lane::RightNow);
        assert!(comes(&code, &no_strangers, Area::WORK, Time::Sleep));
        let mine = message("Me <me@example.net>", "Authentication-Results: mx.example.net; dmarc=pass header.from=example.net\r\n", "A file", &senders);
        assert!(mine.reasons.contains(&crate::porch::Reason::FromYourself), "{:?}", mine.reasons);
        assert!(comes(&mine, &no_strangers, Area::WORK, Time::Sleep));
        assert!(comes(&stranger, &usual, Area::LEISURE, Time::Any));
        // Forged in a safe sender's name: set aside, and weighed as a stranger's.
        let forged = message("Jane <jane@example.org>", "Authentication-Results: mx.example.net; dmarc=fail (p=reject) header.from=example.org\r\n", "Hello", &senders);
        assert_eq!(forged.lane, crate::porch::Lane::SetAside);
        assert!(!comes(&forged, &usual, Area::PERSONAL, Time::Leisure) && comes(&forged, &usual, Area::PERSONAL, Time::Admin));
        assert!(!comes(&forged, &no_strangers, Area::PERSONAL, Time::Admin), "on the strangers' row");
        // Always through: the stranger on the list comes in leisure, told; forged in Jane's name, never as hers.
        let mut list = crate::everywhere::People::default();
        list.add(crate::everywhere::Person { name: "Someone".into(), emails: vec!["someone@elsewhere.example".into(), "jane@example.org".into()], ..Default::default() }, None);
        let leisure = at(Time::Leisure, set);
        let listed = usual.mail(&stranger, &senders, &list, Area::WORK, &leisure);
        assert!(listed.shown && listed.told && listed.pierce);
        assert!(!usual.mail(&forged, &senders, &list, Area::PERSONAL, &leisure).shown);
    }

    /// Q23: a declined call is listed by its Calls row (before: its Calls or its Mail row).
    #[test]
    fn declined_calls_are_listed_by_their_calls_row() {
        let a = Attention::usual();
        let (work, admin, leisure) = (Now::time(Column::Work), Now::time(Column::Admin), Now::time(Column::Leisure));
        // The neutral when they may ring; before, in work and admin time too, by their Mail row (the same here).
        assert!(a.listed(Person::Neutral, false, &work) && a.listed(Person::Neutral, false, &admin) && !a.listed(Person::Neutral, false, &leisure));
        // The safe in leisure, as they ring then; Always through at any time; the blocked never.
        assert!(a.listed(Person::Safe, false, &leisure) && a.listed(Person::Stranger, true, &Now::time(Column::Sleep)));
        assert!(!a.listed(Person::Blocked, true, &work));
        // Strangers ring at no time: listed in work and admin time, never in leisure; under do-not-disturb, later.
        assert!(a.listed(Person::Stranger, false, &work) && a.listed(Person::Stranger, false, &admin) && !a.listed(Person::Stranger, false, &leisure));
        assert!(!a.listed(Person::Stranger, false, &work.clone().layers(false, true)));
        // The safe while you sleep: before, listed then by their Mail row (it ticks sleep); now at waking, as they ring.
        assert!(!a.listed(Person::Safe, false, &Now::time(Column::Sleep)));
        assert!(a.listed(Person::Restricted, false, &work) && !a.listed(Person::Restricted, false, &admin));
        // Strangers set to ring in leisure: then only, as their row says.
        let mut ringing = Attention::usual();
        ringing.set(row("calls.stranger"), Column::Leisure, Level::Now).unwrap();
        assert!(ringing.listed(Person::Stranger, false, &leisure) && !ringing.listed(Person::Stranger, false, &work));
    }

    #[test]
    fn areas_and_holds() {
        let a = Attention::usual();
        let evening = Now::of(&mode(Time::Leisure, Reason::Evening));
        let work_mail = |who: Who| a.decide(&Event::of(Source::Mail { who, always: false, lane: Lane::Usual }).for_area(Some(Area::WORK)), &Now::of(&mode(Time::Admin, Reason::AdminTime)));
        // A stranger to the work address in admin time: held for work; a safe sender comes.
        assert_eq!((work_mail(Who::Stranger).step, work_mail(Who::Safe).told), (Step::Area, true));
        // Work's dates wait while work rests.
        assert_eq!(a.decide(&Event::own(Kind::Dates).for_area(Some(Area::WORK)), &evening).step, Step::Area);
        assert!(a.decide(&Event::own(Kind::Dates), &evening).told);
        // An event's own: at once when its event falls in the time now.
        let mut night = Now::of(&mode(Time::Sleep, Reason::Sleep));
        night.span = (1_000, 2_000);
        assert!(a.decide(&Event::own(Kind::Alarms).starting(Some(1_500)), &night).told);
        assert!(!a.decide(&Event::own(Kind::Alarms).starting(Some(2_500)), &night).told);
        // The Porch resting after a pause: shown, not told.
        let mut rests = Now::time(Column::Work);
        rests.holds.porch_rests = true;
        let resting = a.decide(&Event::of(Source::Mail { who: Who::Safe, always: false, lane: Lane::Usual }), &rests);
        assert_eq!((resting.shown, resting.told, resting.step), (true, false, Step::Hold));
        // A meeting: no meal's notice; the chat limit: chat sites wait.
        let mut meeting = Now::time(Column::Work);
        meeting.holds.meeting = true;
        assert_eq!(a.decide(&Event::own(Kind::Needs), &meeting).step, Step::Hold);
        // Codes: on top of the Porch, told as their row; your own mail never told.
        let code = a.decide(&Event::of(Source::Mail { who: Who::Stranger, always: false, lane: Lane::Code }), &Now::of(&mode(Time::Sleep, Reason::Sleep)));
        assert!(code.shown && code.told && code.pierce);
        let own = a.decide(&Event::of(Source::Mail { who: Who::Safe, always: false, lane: Lane::Own }), &Now::time(Column::Work));
        assert!(own.shown && !own.told);
    }

    #[test]
    fn what_each_mode_lets_through() {
        let a = Attention::usual();
        let phone = Phone { screens: true };
        // The pause: Always through only, their calls through the mode as contacts' while this phone screens; doses and alarms pass.
        let pause = a.silence(&[Column::Pause], false, phone);
        assert_eq!((pause.calls, pause.messages, pause.repeat, pause.conversations, pause.doses, pause.events), (Senders::Contacts, Senders::Starred, true, true, true, true));
        // Messages never wider than the starred, though this phone holds other apps' notifications and anyone may
        // write (work): Android sounds a message before Sioul can hold it, so the mode alone keeps a held one silent.
        assert_eq!(a.silence(&[Column::Work], false, phone).messages, Senders::Starred);
        // Q9: Free time, while this phone screens calls: every contact's call (before: starred only).
        let free = a.silence(&[Column::Free], false, phone);
        assert_eq!(free.calls, Senders::Contacts);
        assert_eq!(a.silence(&[Column::Free], false, Phone::default()).calls, Senders::Starred, "without screening, starred only, as before");
        // "Nothing at all": the list's still (Always through), nobody else.
        let nothing = a.silence(&[Column::Free], true, Phone::default());
        assert_eq!(nothing.calls, Senders::Starred);
        // Q7: an event's alarms pass a pause, on a channel of their own.
        assert!(a.silence(&[Column::Pause], false, phone).events);
        // Always through as their own row everywhere: nobody through the pause.
        let mut none = Attention::usual();
        for channel in Channel::ALL {
            for column in Column::ALL {
                none.set(Row::People(channel, Person::Always), column, Level::As).unwrap();
            }
        }
        let shut = none.silence(&[Column::Pause], false, phone);
        assert_eq!((shut.calls, shut.repeat, shut.conversations), (Senders::None, false, false));
        // Let every call through: anyone's call through every mode, so that the mode lets ring what the screening
        // lets through (before: contacts, a stranger let through by the screening then silenced by the mode).
        for (columns, nothing) in [(&[Column::Pause][..], false), (&[Column::Free][..], true), (&[Column::Leisure, Column::Dnd][..], false)] {
            let mut now = Now::time(Column::Leisure);
            now.times = columns.iter().copied().filter(|c| c.is_time()).collect();
            now.dnd = columns.contains(&Column::Dnd);
            now.nothing = nothing;
            assert_eq!(none.silence_at(&now, phone).calls, Senders::None, "{columns:?}");
            now.through = true;
            for phone in [phone, Phone::default()] {
                let through = none.silence_at(&now, phone);
                assert_eq!((through.calls, through.repeat, through.messages), (Senders::Anyone, true, Senders::None), "{columns:?}");
            }
        }
        // Do-not-disturb over the time now, as `level` reads a layer (before: the layer's column alone): a cell
        // loosened under the switch lets no more through than the time does.
        let mut loose = Attention::usual();
        loose.set(row("calls.stranger"), Column::Dnd, Level::Now).unwrap();
        assert_eq!(loose.silence(&[Column::Dnd], false, phone).calls, Senders::Anyone, "the layer by itself");
        assert_eq!(loose.silence(&[Column::Leisure, Column::Dnd], false, phone).calls, Senders::Contacts, "over leisure, where strangers wait");
        // Two times at once (no hours set: work and admin), the least strict; a layer above them, the stricter.
        let mut hidden = Attention::usual();
        hidden.set(row("calls.hidden"), Column::Admin, Level::Later).unwrap();
        assert_eq!(hidden.silence(&[Column::Admin], false, phone).calls, Senders::Contacts);
        assert_eq!(hidden.silence(&[Column::Work, Column::Admin], false, phone).calls, Senders::Anyone);
        assert_eq!(hidden.silence(&[Column::Work, Column::Admin, Column::Dnd], false, phone).calls, Senders::Contacts);
        // The same moment as the window reads it: no admin hours, work time takes admin's cells (people's rows).
        let mut lent = Attention::usual();
        lent.set(row("calls.hidden"), Column::Work, Level::Later).unwrap();
        let mut work = Now::time(Column::Work);
        assert_eq!(lent.silence_at(&work, phone).calls, Senders::Contacts);
        work.week.admin_hours = false;
        assert_eq!(lent.silence_at(&work, phone).calls, Senders::Anyone);
    }

    #[test]
    fn older_keys_seed_what_was_changed_and_nothing_else() {
        // Nothing written: the usual, decided changes included.
        assert_eq!(seeded(&Config::default()), Attention::usual());
        // Neutral mail in leisure ticked, the list's mail through whatever their row, doses silent in sleep,
        // codes held in the pause (now fixed: at once), strangers' calls in work, the list getting through nobody.
        let config: Config = toml::from_str(
            "[reach]\nsafe = [\"work\", \"admin\", \"leisure\", \"meals\", \"sleep\", \"pause\"]\nneutral = [\"work\", \"admin\", \"leisure\"]\nrestricted = [\"work\"]\nstranger = [\"work\", \"admin\"]\n\
             [reach.calls]\nstranger = [\"work\"]\n\
             [notify]\nmail = [\"dnd:list-any\"]\ncodes = [\"pause:never\"]\nsites = [\"work:later\"]\n\
             [reminders]\ndoses_in_sleep = false\n",
        )
        .unwrap();
        let a = Attention::of(&config);
        assert_eq!(a.cell(row("mail.neutral"), Column::Leisure), Level::Now);
        assert_eq!(a.cell(row("mail.always"), Column::Dnd), Level::Now);
        assert_eq!(a.cell(row("doses"), Column::Sleep), Level::Later);
        assert_eq!(a.cell(row("codes"), Column::Pause), Level::Now, "fixed now");
        assert_eq!(a.cell(row("calls.stranger"), Column::Work), Level::Now);
        assert_eq!(a.cell(row("sites"), Column::Work), Level::Later);
        // What was not changed takes the decided values: safe messages held in sleep.
        assert_eq!(a.cell(row("messages.safe"), Column::Sleep), Level::Later);
        // Before the five states: strangers as the neutral, the pause as sleep.
        let older: Config = toml::from_str("[reach]\nneutral = [\"work\", \"admin\", \"meals\"]\n").unwrap();
        let a = Attention::of(&older);
        assert_eq!(a.cell(row("mail.stranger"), Column::Meals), Level::Now);
        // The list getting through nobody: Always through as their own row.
        let nobody: Config = toml::from_str("[dnd]\npeople = false\n").unwrap();
        let a = Attention::of(&nobody);
        assert_eq!((a.cell(row("calls.always"), Column::Sleep), a.cell(row("mail.always"), Column::Free)), (Level::As, Level::As));
        // Written, the [attention] counts alone.
        let written: Config = toml::from_str("[dnd]\npeople = false\n[attention]\n").unwrap();
        assert_eq!(Attention::of(&written), Attention::usual());
    }

    #[test]
    fn written_whole_seeded_once_and_the_older_keys_taken_out() {
        let dir = std::env::temp_dir().join(format!("sioul-attention-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# Mine.\nlanguage = \"fr\"\n[reach]\nneutral = [\"work\", \"admin\", \"leisure\"]\nstranger = [\"work\", \"admin\"]\n[notify]\nsites = [\"work:later\"]\n[dnd]\nfocus = true\npeople = true\n").unwrap();
        let config = Config::load(&path).unwrap();
        // One row changed: every seeded row written, the older keys out, the comment kept.
        apply(&path, &config, "attention.codes", &SettingValue::Texts(vec!["sleep:never".into()])).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# Mine.") && text.contains("[attention]") && !text.contains("[reach]") && !text.contains("[notify]") && !text.contains("people"), "{text}");
        assert!(text.contains("focus = true"), "the rest of [dnd] kept: {text}");
        let config = Config::load(&path).unwrap();
        let a = Attention::of(&config);
        assert_eq!((a.cell(row("codes"), Column::Sleep), a.cell(row("mail.neutral"), Column::Leisure), a.cell(row("sites"), Column::Work)), (Level::Never, Level::Now, Level::Later));
        // Refused: a fixed cell, a value not offered, a word not understood, a row that is none.
        assert!(apply(&path, &config, "attention.doses", &SettingValue::Texts(vec!["work:later".into()])).is_err());
        assert!(apply(&path, &config, "attention.sites", &SettingValue::Texts(vec!["work".into()])).is_err());
        assert!(apply(&path, &config, "attention.mail.safe", &SettingValue::Texts(vec!["teatime".into()])).is_err());
        assert!(apply(&path, &config, "attention.mail.hidden", &SettingValue::Texts(vec!["work".into()])).is_err());
        // Back to the usual: the row taken out.
        apply(&path, &config, "notify.codes", &SettingValue::Texts(vec!["sleep".into()])).unwrap();
        let config = Config::load(&path).unwrap();
        assert!(config.attention.as_ref().unwrap().words("codes").is_none());
        // The senders' older ticks: unticked waits, ticked comes (mail shown, not told, in sleep).
        apply(&path, &config, "reach.safe", &SettingValue::Texts(vec!["work".into(), "admin".into(), "sleep".into()])).unwrap();
        let a = Attention::of(&Config::load(&path).unwrap());
        assert_eq!((a.cell(row("mail.safe"), Column::Leisure), a.cell(row("mail.safe"), Column::Sleep)), (Level::Later, Level::Quiet));
        apply(&path, &config, "reach.calls.stranger", &SettingValue::Texts(vec!["work".into()])).unwrap();
        assert_eq!(Attention::of(&Config::load(&path).unwrap()).cell(row("calls.stranger"), Column::Work), Level::Now);
        // A preset: Quieter written whole.
        apply_preset(&path, &Config::load(&path).unwrap(), Preset::Quieter).unwrap();
        assert_eq!(Attention::of(&Config::load(&path).unwrap()), Preset::Quieter.matrix());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Q3: blocked beats Always through, and one list takes them off the other.
    #[test]
    fn always_through_takes_them_off_the_blocked_list() {
        let dir = std::env::temp_dir().join(format!("sioul-unblock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let blocked = dir.join("blocked.txt");
        std::fs::write(&blocked, "alice@example.org\ntel:+33199000001\ncontact:uid-bob\nmallory@example.org\n").unwrap();
        let config: Config = toml::from_str(&format!("blocked_senders = \"{}\"\n[contacts]\nregion = \"FR\"\n", blocked.display().to_string().replace('\\', "/"))).unwrap();
        assert!(crate::porch::unblock_person(&config, &["Alice <alice@example.org>".into()], &["01 99 00 00 01".into()], "uid-bob").unwrap());
        assert_eq!(std::fs::read_to_string(&blocked).unwrap().lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>(), ["mallory@example.org"]);
        assert!(!crate::porch::unblock_person(&config, &["carol@example.org".into()], &[], "").unwrap(), "nothing of theirs there");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slots_are_read_by_every_process() {
        let dir = std::env::temp_dir().join(format!("sioul-slots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(SLOTS_FILE);
        let today: jiff::Zoned = "2026-10-07T10:00[Europe/Paris]".parse().unwrap();
        let start = today.timestamp().as_second() + 3600;
        let slots = Slots { day: Some(today.date()), slots: vec![(start, start + 1800)] };
        assert!(slots.save(&path).unwrap());
        assert!(!slots.save(&path).unwrap(), "written only when it changed");
        let read = Slots::load(&path);
        let at = |offset: i64| jiff::Timestamp::from_second(start + offset).unwrap().to_zoned(today.time_zone().clone());
        assert!(read.at(&at(60)) && !read.at(&at(-60)) && !read.at(&at(1800)));
        assert_eq!(read.edges(), vec![start, start + 1800]);
        // Another day's slots hold nothing today.
        let old = Slots { day: Some(today.date().yesterday().unwrap()), slots: vec![(start, start + 1800)] };
        assert!(!old.at(&at(60)));
        assert_eq!(Slots::load(&dir.join("none.toml")), Slots::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn presets_never_touch_a_fixed_cell() {
        for preset in Preset::ALL {
            let matrix = preset.matrix();
            for r in Row::ALL {
                for column in Column::ALL {
                    if lock(r, column).is_some() {
                        assert_eq!(matrix.cell(r, column), usual(r, column), "{preset:?} {r:?} {column:?}");
                    }
                }
            }
        }
        assert!(Preset::Quieter.matrix().changes_from(&Attention::usual()) > 0 && Preset::Reachable.matrix().changes_from(&Attention::usual()) > 0);
    }

    #[test]
    fn the_grid_in_both_languages() {
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let mut a = Attention::usual();
            a.set(row("mail.neutral"), Column::Leisure, Level::Now).unwrap();
            let grid = grid(&a, &tr, &Row::ALL, Preset::Usual, Some(&Now::time(Column::Leisure)));
            assert_eq!((grid.columns.len(), grid.rows.len(), grid.marks.len(), grid.presets.len()), (9, 36, 8, 3));
            assert!(grid.rows.iter().find(|r| r.id == "mail.neutral").unwrap().changed && !grid.presets[0].current && grid.presets[0].changes == 1);
            assert!(!grid.changed.is_empty() && !grid.now.is_empty());
            let words = grid
                .columns
                .iter()
                .map(|c| c.label.clone())
                .chain(grid.marks.iter().map(|m| m.label.clone()))
                .chain(grid.presets.iter().map(|p| p.label.clone()))
                .chain([grid.changed.clone(), grid.now.clone()])
                .chain(grid.rows.iter().flat_map(|r| [r.label.clone(), r.group.clone(), r.help.clone()].into_iter().chain(r.choices.iter().map(|c| c.label.clone())).chain(r.cells.iter().flat_map(|c| [c.locked.clone(), c.said.clone()]))));
            for said in words {
                assert!(!said.starts_with("attention-") && !said.contains('{') && !said.contains("sorte"), "{language}: {said}");
                if language == "fr" {
                    for mark in [" :", " ;", " !", " ?", "« ", " »", "'"] {
                        assert!(!said.contains(mark), "{language}: typography in {said:?}");
                    }
                }
            }
            let codes = grid.rows.iter().find(|r| r.id == "codes").unwrap();
            assert!(codes.cells[4].locked.is_empty() && !codes.cells[5].locked.is_empty(), "sleep free, the pause fixed");
            let always = grid.rows.iter().find(|r| r.id == "mail.always").unwrap();
            assert_eq!(always.choices.iter().find(|c| c.id == "now").unwrap().mark, "★");
            assert!(!list_choice(&tr, Who::Neutral, &a, true).contains('{'));
        }
        assert_eq!(Translator::new("fr").text("attention-column-free", None), "Temps libre");
        assert_eq!(list_choice(&Translator::new("en"), Who::Neutral, &Attention::usual(), false), "Neutral: work, admin");
        assert_eq!(list_choice(&Translator::new("en"), Who::Safe, &Attention::usual(), false), "Safe: any time");
    }

    /// A source's own row (an app's, a conversation's: §1.3), each cell as
    /// set or, as usual, the row the notification takes otherwise; at each
    /// time, under the layers, a conversation's over its app's; nothing
    /// changes for an app until a cell of its row is set.
    #[test]
    fn a_source_s_row_is_read_over_its_usual_row() {
        let app = "app.com.example.chat".to_string();
        let talk = "conversation.0123456789abcdef".to_string();
        let both = vec![talk.clone(), app.clone()];
        let message = |who: Who, group: bool| Event::of(Source::Message { via: Channel::Messages, who, group, always: false });
        let at = |column: Column| Now::time(column);
        let usual = Attention::usual();
        // Nothing set: every person, every time, layer or none, as before.
        for who in [Who::Safe, Who::Neutral, Who::Restricted, Who::Stranger, Who::Blocked] {
            for group in [false, true] {
                for column in Column::TIMES {
                    for (slot, dnd) in [(false, false), (true, false), (false, true)] {
                        let now = at(column).layers(slot, dnd);
                        assert_eq!(usual.decide(&message(who, group).from_sources(both.clone()), &now), usual.decide(&message(who, group), &now), "{who:?} {group} {column:?}");
                    }
                }
            }
        }
        // The owner's example: through during work, held otherwise.
        let mut a = Attention::usual();
        for column in Column::ALL {
            a.set_source(&app, column, if column == Column::Work { Level::Now } else { Level::Later }).unwrap();
        }
        let stranger = message(Who::Stranger, false).from_sources(both.clone());
        assert_eq!(a.decide(&stranger, &at(Column::Work)), Output { level: Level::Now, shown: true, told: true, pierce: false, step: Step::Chosen });
        for column in [Column::Admin, Column::Leisure, Column::Meals, Column::Sleep, Column::Pause, Column::Free] {
            assert_eq!(a.decide(&stranger, &at(column)).level, Level::Later, "{column:?}");
        }
        // A safe friend too: the app's row says it, whoever writes; the blocked never.
        assert_eq!(a.decide(&message(Who::Safe, false).from_sources(both.clone()), &at(Column::Leisure)).level, Level::Later);
        assert_eq!(a.decide(&message(Who::Blocked, false).from_sources(both.clone()), &at(Column::Work)).step, Step::Blocked);
        // Under do-not-disturb: held, as its layer's cell says.
        assert_eq!(a.decide(&stranger, &at(Column::Work).layers(false, true)).level, Level::Later);
        // As usual where not set: the person's own row (a safe friend comes in leisure, a stranger waits).
        let mut b = Attention::usual();
        b.set_source(&app, Column::Work, Level::Later).unwrap();
        assert_eq!(b.decide(&message(Who::Safe, false).from_sources(both.clone()), &at(Column::Leisure)), usual.decide(&message(Who::Safe, false), &at(Column::Leisure)));
        assert_eq!(b.decide(&message(Who::Stranger, false).from_sources(both.clone()), &at(Column::Leisure)).step, Step::Matrix);
        assert_eq!(b.decide(&message(Who::Safe, false).from_sources(both.clone()), &at(Column::Work)).level, Level::Later);
        // A layer as usual holds as it does: at once at work, but do-not-disturb holds a stranger; set at once, it does not.
        let mut c = Attention::usual();
        c.set_source(&app, Column::Leisure, Level::Now).unwrap();
        assert_eq!(c.decide(&stranger, &at(Column::Leisure)).level, Level::Now);
        assert_eq!(c.decide(&stranger, &at(Column::Leisure).layers(false, true)).level, Level::Later);
        c.set_source(&app, Column::Dnd, Level::Now).unwrap();
        assert_eq!(c.decide(&stranger, &at(Column::Leisure).layers(false, true)).level, Level::Now);
        // Someone Always through stays so, whatever the app's row says.
        let listed = Event::of(Source::Message { via: Channel::Messages, who: Who::Neutral, group: false, always: true }).from_sources(both.clone());
        assert_eq!(a.decide(&listed, &at(Column::Sleep)).level, Level::Now);
        // A conversation's choice wins over its app's.
        a.set_source(&talk, Column::Leisure, Level::Now).unwrap();
        assert_eq!(a.decide(&stranger, &at(Column::Leisure)).level, Level::Now);
        assert_eq!(a.decide(&message(Who::Stranger, false).from_sources(vec![app.clone()]), &at(Column::Leisure)).level, Level::Later);
        // Gathered: an app's automatons and its messages alike.
        let mut d = Attention::usual();
        d.set_source(&app, Column::Work, Level::Gathered).unwrap();
        assert_eq!(d.decide(&Event::own(Kind::AppAutomatons).from_sources(both.clone()), &at(Column::Work)).level, Level::Gathered);
        assert_eq!(d.decide(&stranger, &at(Column::Work)).level, Level::Gathered);
        assert_eq!(d.decide(&Event::own(Kind::AppAutomatons).from_sources(both.clone()), &at(Column::Leisure)), usual.decide(&Event::own(Kind::AppAutomatons), &at(Column::Leisure)));
        // Your word for a time: no area holds it then; as usual, the area does.
        let leisure_app = Event::own(Kind::AppAtOnce).for_area(Some(Area::WORK)).from_sources(both.clone());
        let mut e = Attention::usual();
        e.set_source(&app, Column::Leisure, Level::Now).unwrap();
        assert_eq!(e.decide(&leisure_app, &at(Column::Leisure)).step, Step::Chosen);
        assert_eq!(usual.decide(&leisure_app, &at(Column::Leisure)).step, Step::Area);
        // Free time's "Nothing at all": as usual, it holds a state's row; set, the cell says.
        let mut nothing = at(Column::Free);
        nothing.nothing = true;
        assert_eq!(b.decide(&message(Who::Safe, false).from_sources(both.clone()), &nothing).level, Level::Later);
        e.set_source(&app, Column::Free, Level::Now).unwrap();
        assert_eq!(e.decide(&message(Who::Safe, false).from_sources(both.clone()), &nothing).level, Level::Now);
        // Only its own values; a row as usual everywhere is no row; ids that are no source refused.
        assert!(a.set_source(&app, Column::Work, Level::Quiet).is_err() && a.set_source("app.", Column::Work, Level::Now).is_err() && a.set_source("mail.safe", Column::Work, Level::Now).is_err());
        let mut f = Attention::usual();
        f.set_source(&app, Column::Work, Level::Now).unwrap();
        f.set_source(&app, Column::Work, Level::As).unwrap();
        assert!(f.source(&app).is_none() && f.sources().is_empty() && !f.has_sources(&both));
        assert_eq!(source_rows("com.example.chat", "0123456789abcdef"), both);
        assert_eq!(source_rows("com.example.chat", " "), vec![app.clone()]);
        // In words, both languages.
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let lines = vec![SourceLine { id: app.clone(), label: "Chat".into(), group: "Apps".into(), group_id: "app-rows".into() }];
            let grid = source_grid(&a, &tr, &lines);
            assert_eq!((grid.columns.len(), grid.rows.len(), grid.marks.len(), grid.rows[0].cells.len()), (9, 1, 4, 9));
            assert!(grid.rows[0].changed && grid.rows[0].cells.iter().all(|c| c.choices.len() == 4 && c.locked.is_empty()));
            let said = source_sentence(&a, &tr, &app);
            let usual_said = source_sentence(&Attention::usual(), &tr, &app);
            let words = grid.marks.iter().map(|m| m.label.clone()).chain(grid.rows[0].cells.iter().map(|c| c.said.clone())).chain([grid.rows[0].help.clone(), said.clone(), usual_said, source_sentence(&b, &tr, &app)]);
            for text in words {
                assert!(!text.starts_with("attention-") && !text.contains('{') && !text.contains("sorte"), "{language}: {text}");
                if language == "fr" {
                    for mark in [" :", " ;", " !", " ?", "« ", " »", "'"] {
                        assert!(!text.contains(mark), "{language}: typography in {text:?}");
                    }
                }
            }
        }
        assert_eq!(source_sentence(&a, &Translator::new("en"), &app), "At once: Work; held: Admin, Leisure, Meals, Sleep, Pause, Free time, Time for you and Do not disturb.");
        assert_eq!(source_sentence(&b, &Translator::new("en"), &app), "Held: Work; as usual the rest of the time.");
        assert_eq!(source_sentence(&b, &Translator::new("fr"), &app), "Retenues\u{202f}: Travail\u{202f}; comme d’habitude le reste du temps.");
    }

    /// A source's row written with the settings, read back, its name kept for
    /// the other devices; as usual again, taken out; the other rows untouched.
    #[test]
    fn source_rows_written_and_read() {
        let dir = std::env::temp_dir().join(format!("sioul-attention-sources-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "[notify]\nsites = [\"work:later\"]\n").unwrap();
        let config = Config::load(&path).unwrap();
        let words = |w: &[&str]| SettingValue::Texts(w.iter().map(|s| s.to_string()).collect());
        // The first row written is a source's: the older keys seed the matrix as for any row.
        apply(&path, &config, "attention.app.com.example.chat", &words(&["work", "admin:later", "leisure:as", "dnd:gathered", "name=Example Chat"])).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"app.com.example.chat\" = [\"work\", \"admin:later\", \"dnd:gathered\", \"name=Example Chat\"]") && !text.contains("[notify]"), "{text}");
        let a = Attention::of(&Config::load(&path).unwrap());
        assert_eq!(a.cell(row("sites"), Column::Work), Level::Later, "seeded from [notify]");
        let cells = a.source("app.com.example.chat").unwrap();
        assert_eq!((cells[0], cells[1], cells[2], cells[8]), (Level::Now, Level::Later, Level::As, Level::Gathered));
        assert_eq!(a.source_name("app.com.example.chat"), "Example Chat");
        // Another row changed: the source's stays.
        apply(&path, &Config::load(&path).unwrap(), "attention.codes", &words(&["sleep:never"])).unwrap();
        assert!(Attention::of(&Config::load(&path).unwrap()).source("app.com.example.chat").is_some());
        // Refused: a value a source's cell does not take, a word not understood.
        assert!(apply(&path, &Config::load(&path).unwrap(), "attention.app.com.example.chat", &words(&["work:quiet"])).is_err());
        assert!(apply(&path, &Config::load(&path).unwrap(), "attention.app.com.example.chat", &words(&["teatime"])).is_err());
        // As usual everywhere: taken out, name and all.
        apply(&path, &Config::load(&path).unwrap(), "attention.app.com.example.chat", &words(&["work:as", "admin:as", "dnd:as"])).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("example.chat") && Attention::of(&Config::load(&path).unwrap()).sources().is_empty(), "{text}");
        // An older Sioul's reading leaves it aside: no row of the matrix is a source's.
        assert!(Row::read("app.com.example.chat").is_none() && Row::read("conversation.0123").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

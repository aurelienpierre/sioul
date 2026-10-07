// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each kind of Sioul's own notifications does at each time
//! (docs/reminders.md, "What comes when"): a matrix you set in Settings ▸
//! Reminders and notifications, the kinds down, the times across, one value
//! in each cell.
//!
//! **Who may reach you** (`reach`) says *from whom* mail, calls and messages
//! come at each time; this says *what* comes when. Both decide what is
//! about people (new mail; other apps' messages on a phone): a cell says
//! whether such notifications may come now at all, the sender's row in Who
//! may reach you whether this one does (`Cell::admits`).
//!
//! **The time now** is one of seven columns: the pause, then sleep (the
//! night from winding down to waking, a nap), then Free time, then a meal,
//! then the hours (work, admin, leisure), as `quiet::mode` decides it. No
//! hours set count as work and admin together, and so do work and admin
//! hours open at once: the least strict of their two cells, as Who may reach
//! you lends them. Two layers may hold on top: a slot of time for you
//! (today's plan) and do-not-disturb from its switch or a focus session
//! (`everywhere::Now::gates`; held for sleep or a pause, theirs are the
//! columns). With a layer, the strictest cell wins.
//!
//! **What stays outside**: what a thing is for and whether now is for it
//! (`areas`: a work site in the evening, a work task's date while work
//! rests, mail to a work address), an event going on (no meal's notice in a
//! meeting), the Porch resting after a pause, the switches that say whether
//! a kind is told at all (`[reminders] mail`, `gather`…), each site's, app's
//! and conversation's own choice, and which device tells.
//!
//! **Kept in the configuration**: `[notify]`, one row a key, a list of
//! column words as `[reach]` writes its rows: a bare column says "at once",
//! `column:value` another value, and a column not named keeps its usual
//! value. Sioul writes a row whole. The usual values are what Sioul did
//! before the matrix (`Notify::usual`). Doses' sleep and pause cells read the
//! older `[reminders] doses_in_sleep` and `[pause] doses` while their row
//! says nothing of them, and both are written back beside it (`apply`), for
//! an older Sioul on another device.
//!
//! **Fixed cells**, for safety (`lock`): a dose comes at its time, and is
//! never dropped; only sleep and a pause may hold it, until you wake or come
//! back. A code you just asked for comes at once but in sleep and a pause.
//! An event's alarms come during a pause and do-not-disturb, as both
//! promise. The alarm at waking rings always.

use crate::areas::Time;
use crate::config::{Config, SettingValue};
use crate::i18n::Translator;
use crate::quiet::Mode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// A kind of notification: a row of the matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The codes and links you just asked a site for.
    Codes,
    /// A dose's reminder, and the question on doses due while Sioul was closed.
    Doses,
    /// The alarm at waking, on a phone.
    Wake,
    /// The alarms an event carries (VALARM).
    Alarms,
    /// Sioul's own reminder before an event.
    Before,
    /// An event, the working day before.
    DayBefore,
    /// Dates asked, waits over, payments planned, papers and contracts to
    /// renew, the money watch: on a computer.
    Dates,
    /// Health's notices of a meal, a nap or the night.
    Needs,
    /// The pause to move.
    Move,
    /// "Work hours are over", with "Close the work day".
    WorkOver,
    /// The time running: a focus session's own notification.
    Time,
    /// The watch's gentle offers.
    Watch,
    /// New mail.
    Mail,
    /// What your sites notified, gathered in one notification.
    Sites,
    /// A site in real time.
    SitesLive,
    /// A call in a site.
    SiteCalls,
    /// Other apps, on a phone: messages between people (texts, chats, a
    /// mail app, a missed call).
    AppPeople,
    /// Other apps, on a phone: an automaton's notification.
    AppAutomatons,
    /// Other apps, on a phone, that you set to come at once.
    AppAtOnce,
}

impl Kind {
    /// In the order the grid shows them, group by group.
    pub const ALL: [Kind; 19] = [
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
        Kind::Watch,
        Kind::Mail,
        Kind::Sites,
        Kind::SitesLive,
        Kind::SiteCalls,
        Kind::AppPeople,
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
            Kind::Watch => "watch",
            Kind::Mail => "mail",
            Kind::Sites => "sites",
            Kind::SitesLive => "sites-live",
            Kind::SiteCalls => "site-calls",
            Kind::AppPeople => "app-people",
            Kind::AppAutomatons => "app-automatons",
            Kind::AppAtOnce => "app-at-once",
        }
    }

    pub fn read(text: &str) -> Option<Kind> {
        let text = text.trim().to_ascii_lowercase().replace('_', "-");
        Kind::ALL.into_iter().find(|k| k.id() == text)
    }

    /// The group it is shown in: what you set or asked for, reminders, your
    /// day, mail and sites, other apps on a phone.
    pub fn group(self) -> &'static str {
        match self {
            Kind::Codes | Kind::Doses | Kind::Wake | Kind::Alarms => "asked",
            Kind::Before | Kind::DayBefore | Kind::Dates => "reminders",
            Kind::Needs | Kind::Move | Kind::WorkOver | Kind::Time | Kind::Watch => "day",
            Kind::Mail | Kind::Sites | Kind::SitesLive | Kind::SiteCalls => "mail",
            Kind::AppPeople | Kind::AppAutomatons | Kind::AppAtOnce => "apps",
        }
    }

    /// About people: Who may reach you says from whom (`Cell::admits`).
    pub fn people(self) -> bool {
        matches!(self, Kind::Mail | Kind::AppPeople)
    }

    fn index(self) -> usize {
        Kind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// A column of the matrix: one of the seven times, or a layer above them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Column {
    Work,
    Admin,
    Leisure,
    Meals,
    /// The night from winding down to waking, and naps.
    Sleep,
    /// The pause (« En pause »).
    Pause,
    /// Free time (« Temps libre »).
    Free,
    /// A slot of time for you, in today's plan: a layer.
    Slot,
    /// Do-not-disturb from its switch or a focus session: a layer.
    Dnd,
}

impl Column {
    pub const ALL: [Column; 9] = [Column::Work, Column::Admin, Column::Leisure, Column::Meals, Column::Sleep, Column::Pause, Column::Free, Column::Slot, Column::Dnd];

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

    /// A word of the configuration: its id, or a time's word as `[reach]`
    /// reads them ("Repas", "En pause"), or the layers' longer names.
    pub fn read(word: &str) -> Option<Column> {
        let word = word.trim().to_lowercase();
        match word.as_str() {
            "free" | "free-time" | "free time" | "temps libre" => return Some(Column::Free),
            "slot" | "for-you" | "time-for-you" | "time for you" | "du temps pour vous" => return Some(Column::Slot),
            "dnd" | "do-not-disturb" | "do not disturb" | "ne pas déranger" => return Some(Column::Dnd),
            _ => {}
        }
        Some(match crate::reach::Column::read(&word)? {
            crate::reach::Column::Work => Column::Work,
            crate::reach::Column::Admin => Column::Admin,
            crate::reach::Column::Leisure => Column::Leisure,
            crate::reach::Column::Meals => Column::Meals,
            crate::reach::Column::Sleep => Column::Sleep,
            crate::reach::Column::Pause => Column::Pause,
        })
    }

    /// One of the seven times, not a layer.
    pub fn is_time(self) -> bool {
        !matches!(self, Column::Slot | Column::Dnd)
    }

    /// Whether its end is known while it holds, for "if its event falls
    /// then": not the pause's, which lasts until you come back, nor a layer's.
    pub fn ends(self) -> bool {
        !matches!(self, Column::Pause | Column::Slot | Column::Dnd)
    }

    fn index(self) -> usize {
        Column::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }
}

/// What a kind does at a time, from the least strict to the strictest: two
/// cells holding at once give the stricter (`Notify::at`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cell {
    /// At once; about people, as Who may reach you says.
    Now,
    /// During do-not-disturb: the people on its list, whatever Who may reach
    /// you says (as calls).
    ListAny,
    /// During do-not-disturb: the people on its list, when Who may reach you
    /// lets them through too.
    List,
    /// An event's own: when the event begins within this time; else later.
    Event,
    /// At the gathered times.
    Gathered,
    /// It waits, and comes when a time it may come in begins, if it still
    /// makes sense then.
    Later,
    /// Not at all: dropped (a code stays on the Porch).
    Never,
}

impl Cell {
    pub const ALL: [Cell; 7] = [Cell::Now, Cell::ListAny, Cell::List, Cell::Event, Cell::Gathered, Cell::Later, Cell::Never];

    pub fn id(self) -> &'static str {
        match self {
            Cell::Now => "now",
            Cell::ListAny => "list-any",
            Cell::List => "list",
            Cell::Event => "event",
            Cell::Gathered => "gathered",
            Cell::Later => "later",
            Cell::Never => "never",
        }
    }

    pub fn read(text: &str) -> Option<Cell> {
        let text = text.trim().to_ascii_lowercase().replace('_', "-");
        Cell::ALL.into_iter().find(|c| c.id() == text)
    }

    /// Whether a notification about someone comes now: `grid`, Who may reach
    /// you lets them through now; `listed`, they are on do-not-disturb's list
    /// (and "The people on my list get through" is on).
    pub fn admits(self, grid: bool, listed: bool) -> bool {
        match self {
            Cell::Now => grid,
            Cell::List => grid && listed,
            Cell::ListAny => listed,
            _ => false,
        }
    }
}

/// The values a cell may take; the first ones of each kind come first.
pub fn choices(kind: Kind, column: Column) -> &'static [Cell] {
    use Cell::*;
    match kind {
        Kind::Codes => &[Now, Never],
        Kind::Doses => &[Now, Later],
        Kind::Wake => &[Now],
        Kind::Alarms | Kind::Before if column.ends() => &[Now, Event, Later],
        Kind::Alarms | Kind::Before => &[Now, Later],
        Kind::DayBefore | Kind::Dates | Kind::SitesLive | Kind::SiteCalls | Kind::AppAtOnce => &[Now, Later],
        Kind::Needs | Kind::Move | Kind::WorkOver | Kind::Time | Kind::Watch => &[Now, Never],
        Kind::Mail if column == Column::Dnd => &[Now, List, ListAny, Later, Never],
        Kind::Mail => &[Now, Later, Never],
        Kind::AppPeople if column == Column::Dnd => &[Now, List, ListAny, Later],
        Kind::AppPeople => &[Now, Later],
        Kind::Sites | Kind::AppAutomatons => &[Gathered, Later],
    }
}

/// Why a cell is fixed, as the id of its sentence; none when it can change.
pub fn lock(kind: Kind, column: Column) -> Option<&'static str> {
    let rest = !matches!(column, Column::Sleep | Column::Pause);
    match kind {
        Kind::Doses if rest => Some("notify-lock-doses"),
        Kind::Codes if rest => Some("notify-lock-codes"),
        Kind::Alarms if matches!(column, Column::Pause | Column::Dnd) => Some("notify-lock-alarms"),
        Kind::Wake => Some("notify-lock-wake"),
        _ => None,
    }
}

/// What Sioul did before the matrix, kind by kind (the doses' sleep and
/// pause as the older settings say by default: they come).
pub fn usual(kind: Kind, column: Column) -> Cell {
    use Cell::*;
    use Column::*;
    let hours = matches!(column, Work | Admin | Leisure | Meals);
    let held = matches!(column, Sleep | Pause | Free);
    match kind {
        Kind::Codes => if matches!(column, Sleep | Pause) { Never } else { Now },
        Kind::Doses | Kind::Wake | Kind::Time => Now,
        Kind::Alarms | Kind::Before => if matches!(column, Sleep | Free) { Event } else { Now },
        Kind::DayBefore | Kind::Dates => if held { Later } else { Now },
        Kind::Needs => if held { Never } else { Now },
        Kind::Move => if hours { Now } else { Never },
        Kind::WorkOver => if column == Work || held { Never } else { Now },
        Kind::Watch => if matches!(column, Work | Slot | Dnd) { Now } else { Never },
        Kind::Mail => match column {
            Dnd => List,
            _ if hours => Now,
            _ => Later,
        },
        Kind::Sites | Kind::AppAutomatons => if hours { Gathered } else { Later },
        Kind::SitesLive | Kind::AppAtOnce => if hours { Now } else { Later },
        Kind::SiteCalls => if hours || column == Slot { Now } else { Later },
        Kind::AppPeople => if column == Dnd { List } else { Now },
    }
}

/// One word of a row: "sleep" (at once) or "sleep:later"; none when either
/// half is not understood.
pub fn word(text: &str) -> Option<(Column, Cell)> {
    match text.rsplit_once(':') {
        Some((column, value)) => Some((Column::read(column)?, Cell::read(value)?)),
        None => Some((Column::read(text)?, Cell::Now)),
    }
}

/// `[notify]` as written: each row's words, read leniently (a row that is
/// no list of words is left aside, never the whole configuration).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct NotifySettings(pub BTreeMap<String, toml::Value>);

impl NotifySettings {
    /// A row's words, by its id: a list of words, or one text of words
    /// separated by commas; none when the row is not written.
    pub fn words(&self, row: &str) -> Option<Vec<String>> {
        let value = self.0.iter().find(|(key, _)| Kind::read(key).is_some_and(|k| k.id() == row)).map(|(_, v)| v)?;
        Some(match value {
            toml::Value::Array(list) => list.iter().filter_map(|w| w.as_str().map(str::to_string)).collect(),
            toml::Value::String(text) => text.split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect(),
            _ => Vec::new(),
        })
    }
}

/// The matrix: what each kind does at each time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notify {
    cells: [[Cell; 9]; 19],
}

impl Default for Notify {
    fn default() -> Notify {
        Notify::usual()
    }
}

impl Notify {
    /// What Sioul did before the matrix (`usual`).
    pub fn usual() -> Notify {
        let mut cells = [[Cell::Now; 9]; 19];
        for kind in Kind::ALL {
            for column in Column::ALL {
                cells[kind.index()][column.index()] = usual(kind, column);
            }
        }
        Notify { cells }
    }

    /// As the configuration says: the usual values, the doses' older
    /// switches, then each row's words. A fixed cell, a value not offered
    /// there and a word not understood are left aside; of two words for one
    /// column, the later counts.
    pub fn of(config: &Config) -> Notify {
        let mut notify = Notify::usual();
        notify.cells[Kind::Doses.index()][Column::Sleep.index()] = if config.reminders.doses_in_sleep { Cell::Now } else { Cell::Later };
        notify.cells[Kind::Doses.index()][Column::Pause.index()] = if config.pause.doses { Cell::Now } else { Cell::Later };
        for kind in Kind::ALL {
            for (column, cell) in config.notify.words(kind.id()).unwrap_or_default().iter().filter_map(|w| word(w)) {
                let _ = notify.set(kind, column, cell);
            }
        }
        notify
    }

    /// The cell of a kind at one column.
    pub fn cell(&self, kind: Kind, column: Column) -> Cell {
        self.cells[kind.index()][column.index()]
    }

    /// A cell changed; refused when it is fixed, or when the value is not
    /// one it may take (`choices`).
    pub fn set(&mut self, kind: Kind, column: Column, cell: Cell) -> Result<(), String> {
        if self.cell(kind, column) == cell {
            return Ok(());
        }
        if lock(kind, column).is_some() {
            return Err(format!("notify.{}: {} is fixed", kind.id(), column.id()));
        }
        if !choices(kind, column).contains(&cell) {
            let offered: Vec<&str> = choices(kind, column).iter().map(|c| c.id()).collect();
            return Err(format!("notify.{}: {}: {}", kind.id(), column.id(), offered.join(", ")));
        }
        self.cells[kind.index()][column.index()] = cell;
        Ok(())
    }

    /// A row as the configuration writes it, whole: "work" for at once,
    /// "sleep:later" for another value.
    pub fn words(&self, kind: Kind) -> Vec<String> {
        Column::ALL
            .iter()
            .map(|column| match self.cell(kind, *column) {
                Cell::Now => column.id().to_string(),
                other => format!("{}:{}", column.id(), other.id()),
            })
            .collect()
    }

    /// What a kind does now: the least strict of the time's columns (one;
    /// work and admin together), then the strictest of that and of each
    /// layer that holds.
    pub fn at(&self, kind: Kind, now: &Now) -> Cell {
        let row = &self.cells[kind.index()];
        let mut cell = now.times.iter().map(|c| row[c.index()]).min().unwrap_or(Cell::Now);
        if now.slot {
            cell = cell.max(row[Column::Slot.index()]);
        }
        if now.dnd {
            cell = cell.max(row[Column::Dnd.index()]);
        }
        cell
    }

    /// Whether a kind comes now, at once.
    pub fn comes(&self, kind: Kind, now: &Now) -> bool {
        self.at(kind, now) == Cell::Now
    }
}

/// The time now, as the matrix reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Now {
    /// The time's columns: one; work and admin when both hold (no hours set,
    /// or both open at once).
    pub times: Vec<Column>,
    /// In a slot of time for you.
    pub slot: bool,
    /// Do-not-disturb from its switch or a focus session.
    pub dnd: bool,
}

impl Now {
    /// From what now is for (`quiet::mode`): the pause, sleep, Free time,
    /// then the time itself; and the two layers.
    pub fn of(mode: &Mode, slot: bool, dnd: bool) -> Now {
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
        Now { times, slot, dnd }
    }

    /// One time alone, no layer.
    pub fn time(column: Column) -> Now {
        Now { times: vec![column], slot: false, dnd: false }
    }

    /// Whether one of the time's columns is `column`.
    pub fn is(&self, column: Column) -> bool {
        self.times.contains(&column)
    }
}

/// Whether do-not-disturb holds now from its switch or a focus session, as
/// the shared files say (`everywhere`): for what runs without the window
/// (`sioul remind --watch`, `sioul watch`, a phone's alarms).
pub fn dnd_from_files(config: &Config, now: i64) -> bool {
    use crate::everywhere::{Sources, Switch};
    let switch = Switch::load(&Switch::default_path());
    let focus = crate::timelog::running().and_then(|r| crate::everywhere::focus(&r, now));
    let sources = Sources { focus, ..Sources::default() };
    crate::everywhere::now(&config.dnd, &sources, switch.latest().as_ref(), now).gates()
}

/// A row changed from Settings (`notify.<row>`, its words): each word read,
/// a fixed cell or a value not offered there refused, a word not understood
/// refused; the row then written whole. The doses' row writes the older
/// switches beside it, for an older Sioul on another device.
pub fn apply(path: &Path, config: &Config, key: &str, value: &SettingValue) -> Result<(), String> {
    let kind = key.strip_prefix("notify.").and_then(Kind::read).ok_or_else(|| format!("{key}: no such kind of notification"))?;
    let words: &[String] = match value {
        SettingValue::Texts(words) => words,
        SettingValue::Ints(none) if none.is_empty() => &[],
        _ => return Err(format!("{key}: a list of columns expected")),
    };
    let mut notify = Notify::of(config);
    for text in words {
        let (column, cell) = word(text).ok_or_else(|| format!("{key}: {text}: not understood"))?;
        notify.set(kind, column, cell)?;
    }
    let row = format!("notify.{}", kind.id());
    crate::config::set_value(path, &row, &SettingValue::Texts(notify.words(kind)))?;
    if kind == Kind::Doses {
        crate::config::set_value(path, "reminders.doses_in_sleep", &SettingValue::Bool(notify.cell(Kind::Doses, Column::Sleep) == Cell::Now))?;
        crate::config::set_value(path, "pause.doses", &SettingValue::Bool(notify.cell(Kind::Doses, Column::Pause) == Cell::Now))?;
    }
    Ok(())
}

// ---------------------------------------------------------------- the grid, in words

/// The grid as Settings draws it: its columns, its values with their marks,
/// its rows in groups, each cell with its value, its choices and, fixed,
/// why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Grid {
    pub columns: Vec<Named>,
    /// Every value, with its mark and its usual words, for the legend.
    pub marks: Vec<Mark>,
    pub rows: Vec<GridRow>,
}

/// An id and its words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Named {
    pub id: String,
    pub label: String,
}

/// A value, its mark in the grid, and its words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mark {
    pub id: String,
    pub mark: String,
    pub label: String,
}

/// A kind: its words, its group's, a sentence on it, the values its cells
/// may take in its own words, and its cells.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GridRow {
    pub id: String,
    pub label: String,
    pub group: String,
    pub help: String,
    pub choices: Vec<Mark>,
    pub cells: Vec<GridCell>,
}

/// A cell: its column, its value, those it may take; why it is fixed, when
/// it is; and the cell in a sentence, for its tip and for screen readers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GridCell {
    pub column: String,
    pub value: String,
    pub choices: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub locked: String,
    pub said: String,
}

/// The mark a value shows in the grid, as text: what the window draws
/// (`NotifyGrid.qml`) and screen readers do not read.
pub fn mark(cell: Cell) -> &'static str {
    match cell {
        Cell::Now => "●",
        Cell::ListAny => "★",
        Cell::List => "☆",
        Cell::Event => "◐",
        Cell::Gathered => "◎",
        Cell::Later => "○",
        Cell::Never => "–",
    }
}

/// A value in a kind's own words: a code not told stays on the Porch;
/// people come as Who may reach you says.
fn value_label(tr: &Translator, kind: Kind, cell: Cell) -> String {
    let own = match (kind, cell) {
        (Kind::Codes, Cell::Never) => Some("notify-value-never-codes"),
        (Kind::Mail | Kind::AppPeople, Cell::Now) => Some("notify-value-now-people"),
        _ => None,
    };
    tr.text(own.unwrap_or(&format!("notify-value-{}", cell.id())), None)
}

/// A column's words: the times as Who may reach you names them, then the
/// three of the matrix's own.
pub fn column_label(tr: &Translator, column: Column) -> String {
    match column {
        Column::Free | Column::Slot | Column::Dnd => tr.text(&format!("notify-column-{}", column.id()), None),
        other => tr.text(&format!("reach-{}", other.id()), None),
    }
}

/// The grid, in `tr`'s words, as `notify` has it.
pub fn grid(notify: &Notify, tr: &Translator) -> Grid {
    let columns = Column::ALL.iter().map(|c| Named { id: c.id().to_string(), label: column_label(tr, *c) }).collect();
    let marks = Cell::ALL.iter().map(|c| Mark { id: c.id().to_string(), mark: mark(*c).to_string(), label: tr.text(&format!("notify-value-{}", c.id()), None) }).collect();
    let rows = Kind::ALL
        .iter()
        .map(|kind| {
            let mut offered: Vec<Cell> = Vec::new();
            for column in Column::ALL {
                for cell in choices(*kind, column) {
                    if !offered.contains(cell) {
                        offered.push(*cell);
                    }
                }
            }
            offered.sort();
            let label = tr.text(&format!("notify-row-{}", kind.id()), None);
            let cells = Column::ALL
                .iter()
                .map(|column| {
                    let value = notify.cell(*kind, *column);
                    let locked = lock(*kind, *column).map(|id| tr.text(id, None)).unwrap_or_default();
                    let mut args = crate::i18n::args();
                    args.set("row", label.clone());
                    args.set("column", column_label(tr, *column));
                    args.set("value", value_label(tr, *kind, value));
                    let said = tr.text("notify-cell", Some(&args));
                    GridCell {
                        column: column.id().to_string(),
                        value: value.id().to_string(),
                        choices: choices(*kind, *column).iter().map(|c| c.id().to_string()).collect(),
                        said: if locked.is_empty() { said } else { format!("{said}. {locked}") },
                        locked,
                    }
                })
                .collect();
            GridRow {
                id: kind.id().to_string(),
                group: tr.text(&format!("notify-group-{}", kind.group()), None),
                help: tr.text(&format!("notify-row-{}-help", kind.id()), None),
                choices: offered.iter().map(|c| Mark { id: c.id().to_string(), mark: mark(*c).to_string(), label: value_label(tr, *kind, *c) }).collect(),
                label,
                cells,
            }
        })
        .collect();
    Grid { columns, marks, rows }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{Area, Week};
    use crate::quiet::Reason;

    fn mode(time: Time, reason: Reason) -> Mode {
        Mode { quiet: !(time.works() || time == Time::Any), time, week: Week { work_hours: true, admin_hours: true, meals: true, sleep: true }, reason, until: None, back: None, label: String::new() }
    }

    /// Every time `quiet::mode` gives, each with its reason.
    fn modes() -> Vec<Mode> {
        vec![
            mode(Time::Work, Reason::Working),
            mode(Time::Work, Reason::WorkingLate),
            mode(Time::Work, Reason::WorkNow),
            mode(Time::Work, Reason::Extended),
            mode(Time::Admin, Reason::AdminTime),
            mode(Time::Several(Area::MIXED), Reason::Working),
            mode(Time::Leisure, Reason::Evening),
            mode(Time::Leisure, Reason::DayOff),
            mode(Time::Leisure, Reason::TimeOff),
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

    /// The usual matrix, as the code before it decided, written out: a test
    /// that fails here changes what Sioul does by default.
    #[test]
    fn the_usual_values_are_what_sioul_did() {
        let expected = [
            ("codes", "now now now now never never now now now"),
            ("doses", "now now now now now now now now now"),
            ("wake", "now now now now now now now now now"),
            ("alarms", "now now now now event now event now now"),
            ("before", "now now now now event now event now now"),
            ("day-before", "now now now now later later later now now"),
            ("dates", "now now now now later later later now now"),
            ("needs", "now now now now never never never now now"),
            ("move", "now now now now never never never never never"),
            ("work-over", "never now now now never never never now now"),
            ("time", "now now now now now now now now now"),
            ("watch", "now never never never never never never now now"),
            ("mail", "now now now now later later later later list"),
            ("sites", "gathered gathered gathered gathered later later later later later"),
            ("sites-live", "now now now now later later later later later"),
            ("site-calls", "now now now now later later later now later"),
            ("app-people", "now now now now now now now now list"),
            ("app-automatons", "gathered gathered gathered gathered later later later later later"),
            ("app-at-once", "now now now now later later later later later"),
        ];
        let usual = Notify::usual();
        assert_eq!(expected.len(), Kind::ALL.len());
        for (row, cells) in expected {
            let kind = Kind::read(row).unwrap();
            let said: Vec<&str> = Column::ALL.iter().map(|c| usual.cell(kind, *c).id()).collect();
            assert_eq!(said.join(" "), cells, "{row}");
            // Each usual value is one its cell may take, fixed cells included.
            for column in Column::ALL {
                assert!(choices(kind, column).contains(&usual.cell(kind, column)), "{row} {column:?}");
            }
        }
        assert_eq!(Notify::of(&Config::default()), usual, "an empty configuration is the usual matrix");
    }

    // What the code decided before the matrix, kept here as it was written,
    // to check the usual matrix against it at every time.

    #[derive(Clone, Copy, PartialEq)]
    enum Notice {
        Dose,
        Code,
        Alarm,
        Other,
    }

    /// `quiet::may_tell`, as it was.
    fn may_tell(mode: &Mode, notice: Notice, doses_in_sleep: bool, doses_in_pause: bool) -> bool {
        if mode.paused() {
            return (notice == Notice::Dose && doses_in_pause) || notice == Notice::Alarm;
        }
        if mode.sleeps() {
            return notice == Notice::Dose && doses_in_sleep;
        }
        !(mode.free() && notice == Notice::Other)
    }

    /// `mailnote::may_tell`, as it was.
    fn mail_may_tell(mode: &Mode) -> bool {
        !(mode.sleeps() || mode.paused() || mode.free())
    }

    #[test]
    fn the_usual_matrix_decides_as_the_code_did() {
        let usual = Notify::usual();
        for m in modes() {
            for (slot, dnd) in [(false, false), (true, false), (false, true), (true, true)] {
                let now = Now::of(&m, slot, dnd);
                let comes = |kind: Kind| usual.comes(kind, &now);
                let at = |kind: Kind| usual.at(kind, &now);
                let other = may_tell(&m, Notice::Other, true, true);
                let said = format!("{:?} {:?} slot {slot} dnd {dnd}", m.time, m.reason);
                // hours::may_notify_code: no layer counts.
                assert_eq!(comes(Kind::Codes), may_tell(&m, Notice::Code, true, true), "codes {said}");
                // hours::doses_silent: no layer counts.
                assert_eq!(comes(Kind::Doses), may_tell(&m, Notice::Dose, true, true), "doses {said}");
                // An event's alarm in the pause (the rest: `reminders`, below).
                if m.paused() {
                    assert_eq!(comes(Kind::Alarms), may_tell(&m, Notice::Alarm, true, true), "alarms {said}");
                }
                // hours::may_notify (with do-not-disturb) and quiet_slot: the pause to move, sites.
                let may_notify = other && !dnd;
                assert_eq!(comes(Kind::Move), may_notify && !slot, "move {said}");
                assert_eq!(at(Kind::Sites) == Cell::Gathered, may_notify && !slot, "sites {said}");
                assert_eq!(comes(Kind::SitesLive), may_notify && !slot, "sites live {said}");
                assert_eq!(comes(Kind::SiteCalls), may_notify, "site calls {said}");
                // hours::may_notify_need: neither layer counts.
                assert_eq!(comes(Kind::Needs), other, "needs {said}");
                // mailnote: may_tell, not in a slot; do-not-disturb lets its list through.
                let mail = at(Kind::Mail);
                assert_eq!(mail != Cell::Later, mail_may_tell(&m) && !slot, "mail {said}");
                assert_eq!(mail == Cell::List, mail_may_tell(&m) && !slot && dnd, "mail's list {said}");
                // appnotes::Ask::free: automatons at the gathered times, apps at once.
                let free = other && !dnd && !slot;
                assert_eq!(at(Kind::AppAutomatons) == Cell::Gathered, free, "automatons {said}");
                assert_eq!(comes(Kind::AppAtOnce), free, "apps at once {said}");
                // appnotes::person: who may reach you, do-not-disturb's list when it holds.
                assert_eq!(at(Kind::AppPeople), if dnd { Cell::List } else { Cell::Now }, "people {said}");
                // reviews::offer: never at work, asleep, paused or in Free time. Work and
                // admin hours open together never hold when the last of them has ended,
                // the one moment the notice is given.
                if !matches!(m.time, Time::Several(_)) {
                    assert_eq!(comes(Kind::WorkOver), !(m.sleeps() || m.time.works() || m.free()), "work over {said}");
                }
                // wearable::offer: never in quiet time.
                assert_eq!(comes(Kind::Watch), !m.quiet, "watch {said}");
                // Never held: the alarm at waking, the time running.
                assert!(comes(Kind::Wake) && comes(Kind::Time), "{said}");
                // Reminders (`reminders::Wait`): an event's own when it falls in sleep or Free time.
                let event_held = m.sleeps() && !m.paused() || m.free();
                for kind in [Kind::Alarms, Kind::Before] {
                    assert_eq!(at(kind), if event_held { Cell::Event } else { Cell::Now }, "{kind:?} {said}");
                }
                let held = m.sleeps() || m.free();
                for kind in [Kind::DayBefore, Kind::Dates] {
                    assert_eq!(comes(kind), !held, "{kind:?} {said}");
                }
            }
        }
        // Doses as the older switches said: silent in sleep, held in the pause.
        let config: Config = toml::from_str("[reminders]\ndoses_in_sleep = false\n[pause]\ndoses = false\n").unwrap();
        let older = Notify::of(&config);
        for m in modes() {
            assert_eq!(older.comes(Kind::Doses, &Now::of(&m, false, false)), may_tell(&m, Notice::Dose, false, false), "{:?}", m.reason);
        }
    }

    #[test]
    fn rows_from_the_configuration() {
        let config: Config = toml::from_str(
            "[notify]\nmail = [\"Temps libre\", \"dnd:list-any\", \"sleep:never\", \"nonsense\", \"work:gathered\"]\n\
             before = \"pause:later, slot:event\"\n\
             doses = [\"work:later\", \"sleep:later\"]\n\
             codes = [\"sleep\"]\n\
             unknown = [\"work\"]\n\
             app_people = [\"free:later\"]\n",
        )
        .unwrap();
        let notify = Notify::of(&config);
        // Read leniently: French words, a value not offered there left aside (gathered mail), a row id with "_".
        assert_eq!(notify.words(Kind::Mail), ["work", "admin", "leisure", "meals", "sleep:never", "pause:later", "free", "slot:later", "dnd:list-any"]);
        assert_eq!((notify.cell(Kind::Before, Column::Pause), notify.cell(Kind::Before, Column::Slot)), (Cell::Later, Cell::Now), "no event in a layer");
        // Fixed: a dose comes during work whatever is written; sleep may hold it.
        assert_eq!((notify.cell(Kind::Doses, Column::Work), notify.cell(Kind::Doses, Column::Sleep)), (Cell::Now, Cell::Later));
        assert_eq!(notify.cell(Kind::Codes, Column::Sleep), Cell::Now);
        assert_eq!(notify.cell(Kind::AppPeople, Column::Free), Cell::Later);
        // The doses' older switches: read while the row says nothing of them; the row wins.
        let older: Config = toml::from_str("[reminders]\ndoses_in_sleep = false\n").unwrap();
        assert_eq!(Notify::of(&older).cell(Kind::Doses, Column::Sleep), Cell::Later);
        let both: Config = toml::from_str("[reminders]\ndoses_in_sleep = false\n[notify]\ndoses = [\"sleep\"]\n").unwrap();
        assert_eq!(Notify::of(&both).cell(Kind::Doses, Column::Sleep), Cell::Now);
        // A row that is no list is left aside, never the configuration.
        let odd: Config = toml::from_str("[notify]\nmail = 3\n").unwrap();
        assert_eq!(Notify::of(&odd), Notify::usual());
        // Words, both ways.
        assert_eq!(word("Repas:later"), Some((Column::Meals, Cell::Later)));
        assert_eq!((word("en pause"), word("ne pas déranger:list")), (Some((Column::Pause, Cell::Now)), Some((Column::Dnd, Cell::List))));
        assert_eq!((word("sleep:soon"), word("night-shift")), (None, None));
        assert_eq!((Kind::read("Day_Before"), Kind::read("nothing")), (Some(Kind::DayBefore), None));
    }

    #[test]
    fn times_together_and_layers() {
        let mut notify = Notify::usual();
        // No hours set: work and admin together, the least strict of the two.
        notify.set(Kind::Mail, Column::Admin, Cell::Later).unwrap();
        let any = Now::of(&mode(Time::Any, Reason::NoHours), false, false);
        assert_eq!((any.times.clone(), notify.at(Kind::Mail, &any)), (vec![Column::Work, Column::Admin], Cell::Now));
        notify.set(Kind::Mail, Column::Work, Cell::Never).unwrap();
        assert_eq!(notify.at(Kind::Mail, &any), Cell::Later);
        let admin = Now::of(&mode(Time::Several(Area::ADMIN), Reason::AdminTime), false, false);
        assert_eq!(admin.times, vec![Column::Admin]);
        // A layer: the strictest wins.
        let work = Now { slot: true, ..Now::time(Column::Work) };
        assert_eq!(Notify::usual().at(Kind::SiteCalls, &work), Cell::Now);
        assert_eq!(Notify::usual().at(Kind::SitesLive, &work), Cell::Later);
        let both = Now { dnd: true, ..work.clone() };
        assert_eq!(Notify::usual().at(Kind::SiteCalls, &both), Cell::Later);
        // Who may reach you and do-not-disturb's list.
        assert!(Cell::Now.admits(true, false) && !Cell::Now.admits(false, true));
        assert!(Cell::List.admits(true, true) && !Cell::List.admits(false, true) && !Cell::List.admits(true, false));
        assert!(Cell::ListAny.admits(false, true) && !Cell::ListAny.admits(true, false));
        assert!(!Cell::Later.admits(true, true) && !Cell::Never.admits(true, true));
        // The pause and Free time come before the hours; the night before a meal.
        assert_eq!(Now::of(&mode(Time::Sleep, Reason::Paused), true, true).times, vec![Column::Pause]);
        assert_eq!(Now::of(&mode(Time::Leisure, Reason::FreeTime), false, false).times, vec![Column::Free]);
        assert_eq!(Now::of(&mode(Time::Sleep, Reason::Nap), false, false).times, vec![Column::Sleep]);
    }

    #[test]
    fn fixed_cells_and_choices() {
        let mut notify = Notify::usual();
        assert!(notify.set(Kind::Doses, Column::Dnd, Cell::Later).is_err(), "a dose comes during do-not-disturb");
        assert!(notify.set(Kind::Doses, Column::Sleep, Cell::Never).is_err(), "a dose is never dropped");
        assert!(notify.set(Kind::Doses, Column::Pause, Cell::Later).is_ok());
        assert!(notify.set(Kind::Codes, Column::Free, Cell::Never).is_err());
        assert!(notify.set(Kind::Alarms, Column::Pause, Cell::Later).is_err() && notify.set(Kind::Alarms, Column::Dnd, Cell::Later).is_err());
        assert!(notify.set(Kind::Alarms, Column::Sleep, Cell::Now).is_ok());
        assert!(notify.set(Kind::Wake, Column::Sleep, Cell::Now).is_ok(), "its own value is no change");
        assert!(notify.set(Kind::Before, Column::Pause, Cell::Event).is_err(), "a pause has no known end");
        assert!(notify.set(Kind::Mail, Column::Work, Cell::List).is_err(), "the list is do-not-disturb's");
        assert!(notify.set(Kind::Needs, Column::Sleep, Cell::Later).is_err(), "a meal's notice has no sense after it");
        for kind in Kind::ALL {
            for column in Column::ALL {
                assert!(!choices(kind, column).is_empty());
                // A fixed cell holds the first of its choices: at once.
                if lock(kind, column).is_some() {
                    assert_eq!((choices(kind, column).first(), usual(kind, column)), (Some(&Cell::Now), Cell::Now), "{kind:?} {column:?}");
                }
            }
        }
    }

    #[test]
    fn written_whole_and_the_older_switches_beside() {
        let dir = std::env::temp_dir().join(format!("sioul-notify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# Mine.\n[reminders]\nmail = true\n").unwrap();
        let config = Config::load(&path).unwrap();
        // Safe senders' mail told in Free time; the list let through whatever the grid.
        apply(&path, &config, "notify.mail", &SettingValue::Texts(vec!["free".into(), "dnd:list-any".into()])).unwrap();
        let config = Config::load(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# Mine.") && text.contains("[notify]"), "{text}");
        assert_eq!(config.notify.words("mail").unwrap(), ["work", "admin", "leisure", "meals", "sleep:later", "pause:later", "free", "slot:later", "dnd:list-any"]);
        // Refused: a fixed cell, a value not offered, a word not understood, a kind that is none.
        assert!(apply(&path, &config, "notify.doses", &SettingValue::Texts(vec!["work:later".into()])).is_err());
        assert!(apply(&path, &config, "notify.sites", &SettingValue::Texts(vec!["work".into()])).is_err());
        assert!(apply(&path, &config, "notify.mail", &SettingValue::Texts(vec!["teatime".into()])).is_err());
        assert!(apply(&path, &config, "notify.everything", &SettingValue::Texts(vec!["work".into()])).is_err());
        // The doses' row: the older switches written beside it.
        apply(&path, &config, "notify.doses", &SettingValue::Texts(vec!["sleep:later".into()])).unwrap();
        let config = Config::load(&path).unwrap();
        assert!(!config.reminders.doses_in_sleep && config.pause.doses);
        assert_eq!(Notify::of(&config).cell(Kind::Doses, Column::Sleep), Cell::Later);
        apply(&path, &config, "notify.doses", &SettingValue::Texts(vec!["sleep".into(), "pause:later".into()])).unwrap();
        let config = Config::load(&path).unwrap();
        assert!(config.reminders.doses_in_sleep && !config.pause.doses);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_grid_in_both_languages() {
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let grid = grid(&Notify::usual(), &tr);
            assert_eq!((grid.columns.len(), grid.rows.len(), grid.marks.len()), (9, 19, 7));
            let words = grid
                .columns
                .iter()
                .map(|c| c.label.clone())
                .chain(grid.marks.iter().map(|m| m.label.clone()))
                .chain(grid.rows.iter().flat_map(|r| [r.label.clone(), r.group.clone(), r.help.clone()].into_iter().chain(r.choices.iter().map(|c| c.label.clone())).chain(r.cells.iter().map(|c| c.locked.clone()))));
            for said in words {
                assert!(!said.starts_with("notify-") && !said.starts_with("reach-"), "{language}: {said}");
                if language == "fr" {
                    for mark in [" :", " ;", " !", " ?", "« ", " »", "'"] {
                        assert!(!said.contains(mark), "{language}: typography in {said:?}");
                    }
                }
            }
            let doses = grid.rows.iter().find(|r| r.id == "doses").unwrap();
            assert!(!doses.cells[0].locked.is_empty() && doses.cells[4].locked.is_empty());
            assert_eq!(doses.cells[4].choices, ["now", "later"]);
            let codes = grid.rows.iter().find(|r| r.id == "codes").unwrap();
            assert_ne!(codes.choices[1].label, grid.marks.iter().find(|m| m.id == "never").unwrap().label, "a code not told stays on the Porch");
        }
        assert_eq!(Translator::new("fr").text("notify-column-free", None), "Temps libre");
    }
}

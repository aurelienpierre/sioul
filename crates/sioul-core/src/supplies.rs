// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Supplies (docs/supplies.md): what is used up or sold (stamps, envelopes,
//! cards printed by hand, cat food), one of Sioul's own lists, and how much
//! of each is left, worked out from movements, never stored.
//!
//! - **The list**, `sioul-supplies.toml`, where `sioul-projects.toml` is (the
//!   notes folder, then `records/` of the data folder once this device moved
//!   its lists): one `[[supply]]` each, its id made once from its name. Read
//!   leniently (a supply that does not read is passed over, said); written
//!   key by key under the file's lock, every other key and comment kept, and
//!   never over a file that does not read.
//! - **The movements**, `sioul-supplies-moves/<device>.toml` beside it: each
//!   device writes its own file alone and reads every one, so that two
//!   devices never write one file. A line is a count ("there are 12 now"),
//!   something received, or something used, with when it was said (`at`),
//!   when it happened (`on`, the task's completion for a task's use) and the
//!   task it came from. Several lines of one id: the latest `at` wins.
//! - **The stock** is the latest count, plus what was received after it, less
//!   what was used after it, "after" by when each happened (`stock_of`).

use crate::areas::Area;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item};

/// The list's file, beside `sioul-projects.toml`.
pub const FILE: &str = "sioul-supplies.toml";
/// The movements' folder, beside the list: one file per device.
pub const MOVES: &str = "sioul-supplies-moves";
/// The table each supply is written in.
const TABLE: &str = "supply";
// ---------------------------------------------------------------- quantities

/// A quantity, in millionths: kept as written (six decimals at most),
/// summed without a float's drift, shown without trailing zeros.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Qty(i64);

impl Qty {
    pub const ZERO: Qty = Qty(0);
    const SCALE: i64 = 1_000_000;

    /// A whole number.
    pub fn whole(n: i64) -> Qty {
        Qty(n.saturating_mul(Qty::SCALE))
    }

    /// A number as a float gives it, to the millionth; none when it is not one.
    pub fn of_f64(value: f64) -> Option<Qty> {
        (value.is_finite() && value.abs() < 9e12).then(|| Qty((value * Qty::SCALE as f64).round() as i64))
    }

    /// A number as typed: "12", "1.5", "1,5", "−2", spaces between its
    /// digits ("1 000"); none when it is no number.
    pub fn parse(text: &str) -> Option<Qty> {
        let typed: String = text.trim().chars().filter(|c| !c.is_whitespace() && *c != '\u{202f}' && *c != '\u{a0}').map(|c| if c == '−' { '-' } else if c == ',' { '.' } else { c }).collect();
        let (negative, digits) = match typed.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, typed.strip_prefix('+').unwrap_or(&typed)),
        };
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        if (whole.is_empty() && fraction.is_empty()) || !whole.chars().all(|c| c.is_ascii_digit()) || !fraction.chars().all(|c| c.is_ascii_digit()) || whole.len() > 12 {
            return None;
        }
        let mut millionths: i64 = if whole.is_empty() { 0 } else { whole.parse::<i64>().ok()?.checked_mul(Qty::SCALE)? };
        let mut kept = fraction.to_string();
        // Past the sixth decimal, rounded to it (half up).
        let round_up = kept.len() > 6 && kept.as_bytes()[6] >= b'5';
        kept.truncate(6);
        while kept.len() < 6 {
            kept.push('0');
        }
        millionths += kept.parse::<i64>().ok()? + i64::from(round_up);
        Some(Qty(if negative { -millionths } else { millionths }))
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn as_f64(self) -> f64 {
        self.0 as f64 / Qty::SCALE as f64
    }

    /// Without trailing zeros: "12", "1.5", "-3", "0.25".
    pub fn text(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let whole = (self.0 / Qty::SCALE).unsigned_abs();
        let fraction = (self.0 % Qty::SCALE).unsigned_abs();
        if fraction == 0 {
            return format!("{sign}{whole}");
        }
        let digits = format!("{fraction:06}");
        format!("{sign}{whole}.{}", digits.trim_end_matches('0'))
    }

    /// Shown in the window's language: "1.5" or « 1,5 ».
    pub fn shown(self, tr: &crate::i18n::Translator) -> String {
        let text = self.text();
        if tr.text("qt-locale", None).starts_with("fr") { text.replace('.', ",").replace('-', "−") } else { text.replace('-', "−") }
    }

    /// As TOML writes it: an integer when whole, else a float.
    fn toml(self) -> toml_edit::Value {
        if self.0 % Qty::SCALE == 0 { toml_edit::Value::from(self.0 / Qty::SCALE) } else { toml_edit::Value::from(self.as_f64()) }
    }

    /// A TOML value read: an integer, a float, or a number as text.
    fn of_toml(value: &toml::Value) -> Option<Qty> {
        match value {
            toml::Value::Integer(n) => Some(Qty::whole(*n)),
            toml::Value::Float(f) => Qty::of_f64(*f),
            toml::Value::String(s) => Qty::parse(s),
            _ => None,
        }
    }
}

impl std::ops::Add for Qty {
    type Output = Qty;
    fn add(self, other: Qty) -> Qty {
        Qty(self.0.saturating_add(other.0))
    }
}

impl std::ops::Sub for Qty {
    type Output = Qty;
    fn sub(self, other: Qty) -> Qty {
        Qty(self.0.saturating_sub(other.0))
    }
}

impl std::iter::Sum for Qty {
    fn sum<I: Iterator<Item = Qty>>(iter: I) -> Qty {
        iter.fold(Qty::ZERO, |a, b| a + b)
    }
}

impl Serialize for Qty {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0 % Qty::SCALE == 0 { serializer.serialize_i64(self.0 / Qty::SCALE) } else { serializer.serialize_f64(self.as_f64()) }
    }
}

impl<'de> Deserialize<'de> for Qty {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Qty, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Said {
            Whole(i64),
            Float(f64),
            Text(String),
        }
        let said = Said::deserialize(deserializer)?;
        match said {
            Said::Whole(n) => Some(Qty::whole(n)),
            Said::Float(f) => Qty::of_f64(f),
            Said::Text(t) => Qty::parse(&t),
        }
        .ok_or_else(|| serde::de::Error::custom("not a quantity"))
    }
}

/// Whether a word can be a supply's id: what `id_for` makes, and what a
/// hand-written one may hold that stays one parameter of a task's line.
pub fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

// ---------------------------------------------------------------- the list

/// One supply, as the list holds it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Supply {
    pub id: String,
    pub name: String,
    /// A word of the person's: one kind of category.
    pub category: String,
    /// What it is for, as written ("work", "admin+leisure"); "" unsaid: every time.
    pub area: String,
    /// "" for a piece.
    pub unit: String,
    /// A unit's buying price, as typed, in the books' currency; may be unsaid.
    pub buy: String,
    /// A unit's reselling price, optional.
    pub sell: String,
    /// Low at or under this, of what is free; none: never low.
    pub low_at: Option<Qty>,
    /// The usual quantity of an order: what a task to resupply brings.
    pub bought_by: Option<Qty>,
    /// The project its tasks to resupply go to, by id.
    pub project: String,
    /// Kept with its history, out of the page's lists.
    pub archived: bool,
}

impl Supply {
    /// What it is for: unsaid, every time, so that a supply never set is never hidden.
    pub fn area_of(&self) -> Area {
        Area::parse(&self.area).filter(|a| !a.is_empty()).unwrap_or(Area::ALL)
    }

    /// Its name, else its id.
    pub fn name_shown(&self) -> &str {
        if self.name.trim().is_empty() { &self.id } else { self.name.trim() }
    }
}

/// The supplies of a list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct List {
    pub supplies: Vec<Supply>,
    /// Supplies passed over: no id, or an id said twice.
    pub skipped: usize,
}

impl List {
    /// The list of `root` (the notes folder, or `records/` once moved); none
    /// when the file is not there. Err when it is there and cannot be read,
    /// or is no TOML: a supply that does not read is passed over (`skipped`).
    pub fn load(root: &Path) -> Result<List, String> {
        let path = file_in(root);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(List::default()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        List::of_text(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The list a file's text holds.
    pub fn of_text(text: &str) -> Result<List, String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut list = List::default();
        for item in table.get(TABLE).and_then(toml::Value::as_array).map(Vec::as_slice).unwrap_or_default() {
            match item.as_table().and_then(supply_of) {
                Some(supply) if !list.supplies.iter().any(|s| s.id == supply.id) => list.supplies.push(supply),
                _ => list.skipped += 1,
            }
        }
        Ok(list)
    }

    pub fn get(&self, id: &str) -> Option<&Supply> {
        self.supplies.iter().find(|s| s.id == id)
    }
}

/// A supply read from its table, each field leniently: a value of another
/// type reads as unsaid. None without an id that can be one.
fn supply_of(table: &toml::Table) -> Option<Supply> {
    let text = |key: &str| match table.get(key) {
        Some(toml::Value::String(s)) => s.trim().to_string(),
        Some(toml::Value::Integer(n)) => n.to_string(),
        Some(toml::Value::Float(f)) => f.to_string(),
        _ => String::new(),
    };
    let quantity = |key: &str| table.get(key).and_then(Qty::of_toml).filter(|q| *q >= Qty::ZERO);
    let id = text("id");
    if !is_id(&id) {
        return None;
    }
    Some(Supply {
        id,
        name: text("name"),
        category: text("category"),
        area: Area::parse(&text("area")).filter(|a| !a.is_empty()).map(|a| a.id()).unwrap_or_default(),
        unit: text("unit"),
        buy: text("buy"),
        sell: text("sell"),
        low_at: quantity("low_at"),
        bought_by: quantity("bought_by").filter(|q| !q.is_zero()),
        project: text("project"),
        archived: table.get("archived").and_then(toml::Value::as_bool).unwrap_or(false),
    })
}

/// The list's file in `root`.
pub fn file_in(root: &Path) -> PathBuf {
    root.join(FILE)
}

/// An id made once from a name, unique in `taken`: its letters without
/// accents, lowercase, words joined by dashes ("Gift cards" → "gift-cards").
pub fn id_for(name: &str, taken: &[&str]) -> String {
    let folded: String = crate::text::fold(name.trim()).into_iter().collect();
    let mut base = crate::config::slug(&folded);
    base.truncate(40);
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() { "supply".to_string() } else { base };
    std::iter::once(base.clone()).chain((2..).map(|n| format!("{base}-{n}"))).find(|id| !taken.contains(&id.as_str())).unwrap_or(base)
}

/// What a supply's form changed, each field when it changed; an empty text
/// takes its key out (unsaid again).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Change {
    pub name: Option<String>,
    pub category: Option<String>,
    pub area: Option<String>,
    pub unit: Option<String>,
    pub buy: Option<String>,
    pub sell: Option<String>,
    /// As typed: "10", "" for never low.
    pub low_at: Option<String>,
    pub bought_by: Option<String>,
    pub project: Option<String>,
    pub archived: Option<bool>,
}

/// Why a change was refused, as a word of the translations, with what was typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub word: &'static str,
    pub what: String,
}

impl Refused {
    fn new(word: &'static str, what: impl Into<String>) -> Refused {
        Refused { word, what: what.into() }
    }

    /// In the window's words.
    pub fn sentence(&self, tr: &crate::i18n::Translator) -> String {
        let mut args = crate::i18n::args();
        args.set("what", self.what.clone());
        tr.text(self.word, Some(&args))
    }
}

/// The supply `id` of `root`'s list changed key by key (a new one made, its
/// id from its name, when `id` is empty), every other key, supply and
/// comment kept, under the file's lock; never written over a file that does
/// not read. Returns its id.
pub fn change(root: &Path, id: &str, change: &Change) -> Result<String, Refused> {
    let file = file_in(root);
    crate::filelock::with_lock(&file, || {
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(Refused::new("supplies-unreadable", format!("{}: {e}", file.display()))),
        };
        // A list that does not read is never written over: what it holds would be lost.
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| Refused::new("supplies-unreadable", format!("{}: {e}", file.display())))?;
        let read = List::of_text(&text).map_err(|e| Refused::new("supplies-unreadable", format!("{}: {e}", file.display())))?;
        let new = id.trim().is_empty();
        let id = if new {
            let name = change.name.as_deref().unwrap_or("").trim();
            if name.is_empty() {
                return Err(Refused::new("supplies-name-needed", ""));
            }
            let taken: Vec<&str> = read.supplies.iter().map(|s| s.id.as_str()).collect();
            id_for(name, &taken)
        } else {
            id.trim().to_string()
        };
        let list = doc.entry(TABLE).or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| Refused::new("supplies-unreadable", format!("{}: `{TABLE}`", file.display())))?;
        if new {
            let mut table = toml_edit::Table::new();
            table.insert("id", toml_edit::value(id.as_str()));
            list.push(table);
        }
        let table = list.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id.as_str())).ok_or_else(|| Refused::new("supplies-gone", id.clone()))?;
        let texts: [(&str, &Option<String>); 7] = [("name", &change.name), ("category", &change.category), ("unit", &change.unit), ("buy", &change.buy), ("sell", &change.sell), ("project", &change.project), ("area", &change.area)];
        for (key, value) in texts {
            let Some(value) = value else { continue };
            let value = if key == "area" { Area::parse(value).filter(|a| !a.is_empty()).map(|a| a.id()).unwrap_or_default() } else { value.trim().to_string() };
            if value.is_empty() {
                table.remove(key);
            } else {
                put(table, key, toml_edit::value(value));
            }
        }
        for (key, value) in [("low_at", &change.low_at), ("bought_by", &change.bought_by)] {
            let Some(value) = value else { continue };
            if value.trim().is_empty() {
                table.remove(key);
                continue;
            }
            let quantity = Qty::parse(value).filter(|q| *q >= Qty::ZERO).ok_or_else(|| Refused::new("supplies-number-unreadable", value.trim()))?;
            put(table, key, Item::Value(quantity.toml()));
        }
        if let Some(archived) = change.archived {
            if archived {
                put(table, "archived", toml_edit::value(true));
            } else {
                table.remove("archived");
            }
        }
        // What it now says must read back as the list, else nothing is written.
        let written = doc.to_string();
        let back = List::of_text(&written).map_err(|e| Refused::new("supplies-unreadable", e))?;
        if back.get(&id).is_none() {
            return Err(Refused::new("supplies-unreadable", id));
        }
        write_whole(&file, &written).map_err(|e| Refused::new("supplies-unreadable", e))?;
        crate::changes::said(crate::changes::Kind::Supplies);
        Ok(id)
    })
}

/// `item` written at `key` of `table`: a key there keeps its place and the spaces around its value.
fn put(table: &mut toml_edit::Table, key: &str, item: Item) {
    match table.get_mut(key) {
        Some(old) => {
            let mut item = item;
            if let (Some(was), Some(now)) = (old.as_value(), item.as_value_mut()) {
                *now.decor_mut() = was.decor().clone();
            }
            *old = item;
        }
        None => {
            table.insert(key, item);
        }
    }
}

/// A file written beside then renamed over its place: never half a file.
fn write_whole(file: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let temporary = file.with_extension("toml.new");
    std::fs::write(&temporary, text).map_err(|e| format!("{}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, file).map_err(|e| format!("{}: {e}", file.display()))
}

// ---------------------------------------------------------------- the movements

/// What a movement says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// There are `quantity` now.
    Count,
    /// So many more.
    Received,
    /// So many fewer.
    Used,
}

impl Kind {
    fn read(word: &str) -> Option<Kind> {
        match word.trim() {
            "count" => Some(Kind::Count),
            "received" => Some(Kind::Received),
            "used" => Some(Kind::Used),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Kind::Count => "count",
            Kind::Received => "received",
            Kind::Used => "used",
        }
    }
}

/// One movement of a supply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Move {
    pub id: String,
    /// When it was said, Unix seconds: of several lines of one id, the latest wins.
    pub at: i64,
    /// When it happened, Unix seconds, when not when it was said: a task's
    /// completion for what it used, so that a correction said after a count
    /// counts where the use was (`when`).
    pub on: Option<i64>,
    pub supply: String,
    pub kind: Kind,
    pub quantity: Qty,
    /// The task's UID, for a use and for what a task to resupply brought.
    pub task: String,
    /// The device whose file it was read from (not written).
    #[serde(skip)]
    pub device: String,
}

impl Move {
    /// When it happened.
    pub fn when(&self) -> i64 {
        self.on.unwrap_or(self.at)
    }
}

/// The movements of every device, as read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Moves {
    /// Every line, in reading order: the devices' files by name, each in its order.
    pub lines: Vec<Move>,
    /// The tasks to resupply made already, by UID, on any device: never made again.
    pub made: BTreeSet<String>,
    /// The files that did not read, by name: passed over, said.
    pub unreadable: Vec<String>,
    /// This device's own file does not read: nothing is written into it.
    pub own_unreadable: bool,
}

/// The movements' folder in `root`.
pub fn moves_dir(root: &Path) -> PathBuf {
    root.join(MOVES)
}

/// The name of a device's file: its id in the sharing, made safe as a file
/// name; a device that never shared its own word.
pub fn device_file(device: &str) -> String {
    let safe: String = device.trim().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("{}.toml", if safe.is_empty() { "this-device" } else { &safe })
}

/// Every device's movements in `root`; `device` this one's.
pub fn read_moves(root: &Path, device: &str) -> Moves {
    let mut moves = Moves::default();
    let own = device_file(device);
    let Ok(dir) = std::fs::read_dir(moves_dir(root)) else { return moves };
    let mut files: Vec<String> = dir.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".toml") && !n.starts_with('.')).collect();
    files.sort();
    for name in files {
        let path = moves_dir(root).join(&name);
        let parsed = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|text| toml::from_str::<toml::Table>(&text).map_err(|e| e.to_string()));
        let Ok(table) = parsed else {
            moves.own_unreadable |= name == own;
            moves.unreadable.push(name);
            continue;
        };
        let from = name.trim_end_matches(".toml").to_string();
        for item in table.get("move").and_then(toml::Value::as_array).map(Vec::as_slice).unwrap_or_default() {
            if let Some(line) = item.as_table().and_then(|t| move_of(t, &from)) {
                moves.lines.push(line);
            }
        }
        for item in table.get("made").and_then(toml::Value::as_array).map(Vec::as_slice).unwrap_or_default() {
            if let Some(task) = item.as_table().and_then(|t| t.get("task")).and_then(toml::Value::as_str).filter(|t| !t.is_empty()) {
                moves.made.insert(task.to_string());
            }
        }
    }
    moves
}

/// A movement read from its table; none when it lacks what it must say.
fn move_of(table: &toml::Table, device: &str) -> Option<Move> {
    let text = |key: &str| table.get(key).and_then(toml::Value::as_str).map(str::trim).unwrap_or("").to_string();
    let id = text("id");
    let supply = text("supply");
    let kind = Kind::read(&text("kind"))?;
    let at = table.get("at").and_then(toml::Value::as_integer)?;
    let quantity = table.get("quantity").and_then(Qty::of_toml).filter(|q| *q >= Qty::ZERO)?;
    if id.is_empty() || supply.is_empty() {
        return None;
    }
    let on = table.get("on").and_then(toml::Value::as_integer);
    Some(Move { id, at, on, supply, kind, quantity, task: text("task"), device: device.to_string() })
}

/// A task to resupply made, kept so that it is never made again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub task: String,
    pub supply: String,
    pub at: i64,
}

/// `lines` and `made` written into this device's own file, under its lock:
/// a line of the same id said before it (an earlier word on that id) taken
/// out, so that the file holds each id once; every other line and table
/// kept. Never over a file that does not read.
pub fn record(root: &Path, device: &str, lines: &[Move], made: &[Made]) -> Result<(), String> {
    if lines.is_empty() && made.is_empty() {
        return Ok(());
    }
    let file = moves_dir(root).join(device_file(device));
    crate::filelock::with_lock(&file, || {
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("{}: {e}", file.display())),
        };
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| format!("{}: {e}", file.display()))?;
        toml::from_str::<toml::Table>(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        {
            let list = doc.entry("move").or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| format!("{}: `move`", file.display()))?;
            for line in lines {
                // An earlier word on this id, here, goes: the file holds each id once, its latest.
                let earlier: Vec<usize> = list.iter().enumerate().filter(|(_, t)| t.get("id").and_then(Item::as_str) == Some(line.id.as_str()) && t.get("at").and_then(Item::as_integer).is_none_or(|at| at <= line.at)).map(|(i, _)| i).collect();
                for i in earlier.into_iter().rev() {
                    list.remove(i);
                }
                let mut table = toml_edit::Table::new();
                table.insert("id", toml_edit::value(line.id.as_str()));
                table.insert("at", toml_edit::value(line.at));
                if let Some(on) = line.on.filter(|on| *on != line.at) {
                    table.insert("on", toml_edit::value(on));
                }
                table.insert("supply", toml_edit::value(line.supply.as_str()));
                table.insert("kind", toml_edit::value(line.kind.word()));
                table.insert("quantity", Item::Value(line.quantity.toml()));
                if !line.task.is_empty() {
                    table.insert("task", toml_edit::value(line.task.as_str()));
                }
                list.push(table);
            }
        }
        if !made.is_empty() {
            let list = doc.entry("made").or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| format!("{}: `made`", file.display()))?;
            for one in made {
                if list.iter().any(|t| t.get("task").and_then(Item::as_str) == Some(one.task.as_str())) {
                    continue;
                }
                let mut table = toml_edit::Table::new();
                table.insert("task", toml_edit::value(one.task.as_str()));
                table.insert("supply", toml_edit::value(one.supply.as_str()));
                table.insert("at", toml_edit::value(one.at));
                list.push(table);
            }
        }
        write_whole(&file, &doc.to_string())?;
        crate::changes::said(crate::changes::Kind::Supplies);
        Ok(())
    })
}

/// The latest line of each id: the latest `at` wins; at the same `at`, the
/// one read last (a device's later line, a later device's file).
pub fn latest(lines: &[Move]) -> BTreeMap<String, Move> {
    let mut out: BTreeMap<String, Move> = BTreeMap::new();
    for line in lines {
        match out.get(&line.id) {
            Some(there) if there.at > line.at => {}
            _ => {
                out.insert(line.id.clone(), line.clone());
            }
        }
    }
    out
}

/// The lines of `supply` among the latest, in the order they happened.
pub fn lines_of<'a>(latest: &'a BTreeMap<String, Move>, supply: &str) -> Vec<&'a Move> {
    let mut lines: Vec<&Move> = latest.values().filter(|m| m.supply == supply).collect();
    lines.sort_by(|a, b| (a.when(), a.at, &a.id).cmp(&(b.when(), b.at, &b.id)));
    lines
}

/// The stock of a supply from its lines in the order they happened
/// (`lines_of`): the latest count (none: 0), plus what was received after
/// it, less what was used after it, "after" by when each happened.
pub fn stock_of(lines: &[&Move]) -> Qty {
    let count = lines.iter().filter(|m| m.kind == Kind::Count).max_by_key(|m| (m.when(), m.at));
    let (base, from) = count.map_or((Qty::ZERO, None), |c| (c.quantity, Some(c.when())));
    let after = |m: &&&Move| from.is_none_or(|from| m.when() > from);
    let received: Qty = lines.iter().filter(after).filter(|m| m.kind == Kind::Received).map(|m| m.quantity).sum();
    let used: Qty = lines.iter().filter(after).filter(|m| m.kind == Kind::Used).map(|m| m.quantity).sum();
    base + received - used
}

// ---------------------------------------------------------------- by hand

/// A movement said now by hand: a count, or something received; its id by chance.
pub fn by_hand(supply: &str, kind: Kind, quantity: Qty, now: i64) -> Move {
    Move { id: crate::ids::new(), at: now, on: None, supply: supply.to_string(), kind, quantity, task: String::new(), device: String::new() }
}

#[cfg(test)]
#[path = "supplies_tests.rs"]
mod tests;

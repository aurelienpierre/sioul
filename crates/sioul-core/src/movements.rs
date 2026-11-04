// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Money movements (docs/accounting.md, "Movements"): each sum of money in or
//! out, once, with everything that tells of it. The rent is planned by its
//! preset and its contract, announced by the landlord's notice, then done when
//! the bank's line comes: one movement with four sources, never four lines
//! (docs/research/budgets.md, §6, recommendation 4).
//!
//! Derived each time from the person's files (the budget file, the bank's
//! movements, contracts, letters) and Sioul's own store beside them,
//! `sioul-movements.toml`: the sources joined to a movement, "Not the same",
//! placements the files cannot say, card holds, the names a correction taught.
//! Nothing here is stored whole: a file changed by hand or by another device
//! is read as it is, and what the person wrote stays the record.
//!
//! Five states, said in words: planned, announced, not final yet, done, not
//! seen. Never "late", "overdue", nor a count. A planned movement whose day
//! passed while no export of its account can tell stays planned: the budgets
//! count it as done, as they always did, but Sioul does not claim it.

use crate::accounts::{self, Other, Place as Placed};
use crate::areas::Area;
use crate::bank::{self, Bank};
use crate::budget::{Ledger, Line, Preset};
use crate::contracts::Contract;
use crate::letters::Letter;
use crate::money::Money;
use jiff::ToSpan;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

/// The store's name, at the root of the notes folder, beside the budget file.
pub const STORE: &str = "sioul-movements.toml";

/// The store's format: 1, its first. A store written in a later format is
/// read for what this Sioul knows of it, and never written over.
pub const FORMAT: u32 = 1;

/// Which way the money goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Way {
    /// Money that comes in.
    In,
    /// Money that goes out.
    Out,
    /// Moved between two of your own accounts: in no budget.
    Between,
}

/// Where a movement stands, in words (research §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// From a contract or a preset, or written as planned: "expected on Friday 5 December".
    Planned,
    /// A mail, a letter, an invoice or a debit notice gave its amount and day.
    Announced,
    /// A card hold: "not final yet: the shop may take another amount".
    NotFinal,
    /// The bank's line: it has left the account, or arrived. Also a line you
    /// wrote as done with nothing else to tell of it (cash).
    Done,
    /// Its day passed, inside what the bank's exports of its account cover,
    /// with no line for it. Said once, as the watch says a payment that did not pass.
    NotSeen,
}

impl State {
    /// "planned", "announced", "not-final", "done", "not-seen".
    pub fn id(self) -> &'static str {
        match self {
            State::Planned => "planned",
            State::Announced => "announced",
            State::NotFinal => "not-final",
            State::Done => "done",
            State::NotSeen => "not-seen",
        }
    }
}

/// How much, as surely as it is known; signed: + in, − out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sum {
    Exact(Money),
    /// About that much (an energy bill, a card hold): within a tenth (`ABOUT`).
    About(Money),
    /// Somewhere between the two, signed alike; its middle is the amount said.
    Between(Money, Money),
}

/// "About" is within this part of the amount: a tenth.
const ABOUT: i64 = 10;

impl Sum {
    /// The amount said: the exact one, the one it is about, a range's middle.
    pub fn amount(self) -> Money {
        match self {
            Sum::Exact(m) | Sum::About(m) => m,
            Sum::Between(a, b) => Money((a.cents() + b.cents()) / 2),
        }
    }

    /// Whether it is said "about".
    pub fn about(self) -> bool {
        !matches!(self, Sum::Exact(_))
    }

    /// The lowest and the highest it may be, in that order.
    pub fn bounds(self) -> (Money, Money) {
        match self {
            Sum::Exact(m) => (m, m),
            Sum::About(m) => {
                let room = m.cents().abs() / ABOUT;
                (Money(m.cents() - room), Money(m.cents() + room))
            }
            Sum::Between(a, b) => (a.min(b), a.max(b)),
        }
    }
}

/// What tells of a movement, in the order a movement takes its id from
/// (`Movement::id`): the person's own line first, the mail last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A line of the budget file.
    Line,
    /// A recurring payment's occurrence (a preset of the budget file).
    Preset,
    /// The bank's line.
    Bank,
    /// A card hold the bank listed as not final yet.
    Hold,
    /// An invoice you issued.
    Invoice,
    /// A paper letter's scan.
    Letter,
    /// A message.
    Mail,
    /// The contract a recurring payment pays.
    Contract,
}

impl Kind {
    /// The kind a source's key names: "bank:…", "mid:…", "sioul:letter/…".
    pub fn of_key(key: &str) -> Option<Kind> {
        let kind = match key {
            k if k.starts_with("line:") => Kind::Line,
            k if k.starts_with("preset:") => Kind::Preset,
            k if k.starts_with("bank:") => Kind::Bank,
            k if k.starts_with("hold:") => Kind::Hold,
            k if k.starts_with("sioul:invoice/") => Kind::Invoice,
            k if k.starts_with("sioul:letter/") => Kind::Letter,
            k if k.starts_with("mid:") || k.starts_with(crate::mailfind::DIGEST_SCHEME) => Kind::Mail,
            k if k.starts_with("sioul:contract/") => Kind::Contract,
            _ => return None,
        };
        Some(kind)
    }

    /// A document: what announces a movement rather than plans or settles it.
    fn announces(self) -> bool {
        matches!(self, Kind::Invoice | Kind::Letter | Kind::Mail)
    }
}

/// One thing that tells of a movement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: Kind,
    /// Its key, by which corrections name it: "line:<id>", "preset:<id>/<day>",
    /// "bank:<account>/<the bank's id>", "hold:<account>/<id>", "mid:<Message-ID>",
    /// "sioul:letter/<id>", "sioul:invoice/<number>", "sioul:contract/<id>".
    pub key: String,
    /// Where it opens (a message, a letter, a contract, a budget line); empty when nowhere.
    pub address: String,
    /// Joined as probably the same payment: "probably the same as …", with Not the same beside it.
    pub probable: bool,
}

impl Source {
    fn new(kind: Kind, key: String, address: String) -> Source {
        Source { kind, key, address, probable: false }
    }

    /// A source named by its key alone: a document's key is its address.
    fn of_key(key: &str) -> Option<Source> {
        let kind = Kind::of_key(key)?;
        let address = if kind.announces() || kind == Kind::Contract { key.to_string() } else { String::new() };
        Some(Source::new(kind, key.to_string(), address))
    }
}

/// Where a movement counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// In one budget, by its id.
    Budget(String),
    /// In parts: each budget its share, signed as the movement.
    Parts(Vec<(String, Money)>),
    /// Moved with a savings account through a budget (a line's `reserve`):
    /// counted in that budget, as reserves cover budgets (docs/accounting.md).
    Reserve { reserve: String, budget: String },
    /// Between two of your bank accounts, by their ids: in no budget.
    Transfer { from: String, to: String },
    /// In no budget: you said so, or its account fills none.
    Nowhere,
    /// Not given a budget yet: a letter that matched nothing, proposed.
    Unplaced,
}

impl Place {
    /// The budgets it counts in, by id.
    pub fn budgets(&self) -> Vec<&str> {
        match self {
            Place::Budget(b) | Place::Reserve { budget: b, .. } => vec![b.as_str()],
            Place::Parts(parts) => parts.iter().map(|(b, _)| b.as_str()).collect(),
            Place::Transfer { .. } | Place::Nowhere | Place::Unplaced => Vec::new(),
        }
    }
}

/// One sum of money in or out, from all its sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movement {
    /// Its id: the key of its first source in `Kind`'s order (its line, else
    /// its occurrence, else the bank's line…). The same files give the same id
    /// on every device; corrections name a source's key, so they hold
    /// whatever joins it later, and an address naming any source still finds it.
    pub id: String,
    /// The bank account, by its id, when known: the bank's line's, else the
    /// account that fills its budget first.
    pub account: Option<String>,
    pub way: Way,
    pub sum: Sum,
    /// ISO 4217: the ledger's money is euros.
    pub currency: String,
    /// The day it happened (the bank's), else the day announced or planned.
    pub day: Date,
    /// The day planned or announced, when the bank's came another day.
    pub expected: Option<Date>,
    /// Who: the payee or the payer, as the person's line or preset names them, else the bank.
    pub label: String,
    pub place: Place,
    /// What it is for (docs/areas.md): its budgets'; in none, its account's.
    pub area: Area,
    /// Every source, its anchor first.
    pub sources: Vec<Source>,
    /// What else it is tied to: tasks, events, projects, notes.
    pub links: Vec<String>,
    pub state: State,
    /// It waits for a line of the bank: an occurrence, a planned line, a letter's bill.
    waits: bool,
}

impl Movement {
    /// Whether `key` names it: its id or one of its sources' keys.
    pub fn has(&self, key: &str) -> bool {
        self.id == key || self.sources.iter().any(|s| s.key == key)
    }

    /// Its address: `sioul:movement/<id>`.
    pub fn address(&self) -> String {
        crate::links::movement_uri(&self.id)
    }

    fn has_kind(&self, kind: Kind) -> bool {
        self.sources.iter().any(|s| s.kind == kind)
    }

    /// The bank's own facts, once a line of it joins: its day, its amount, its account.
    fn settle(&mut self, account: &str, day: Date, amount: Money, hold: bool) {
        if self.day != day {
            self.expected = Some(self.expected.unwrap_or(self.day));
        }
        self.day = day;
        self.sum = if hold { Sum::About(amount) } else { Sum::Exact(amount) };
        self.account = Some(account.to_string());
        self.state = if hold { State::NotFinal } else { State::Done };
    }

    /// Its id again, from its sources (`Kind`'s order, then the smallest key).
    fn anchor(&mut self) {
        if let Some(first) = self.sources.iter().min_by(|a, b| (a.kind, &a.key).cmp(&(b.kind, &b.key))) {
            self.id = first.key.clone();
        }
    }
}

/// The movements of a notes folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Movements {
    pub list: Vec<Movement>,
}

impl Movements {
    /// The movement an address or a key names: its id, or any of its sources.
    pub fn find(&self, key: &str) -> Option<&Movement> {
        let key = key.strip_prefix("sioul:movement/").map_or_else(|| key.to_string(), crate::notes::decode);
        self.list.iter().find(|m| m.has(&key))
    }
}

// ---------------------------------------------------------------- the store

/// Sioul's own record of money movements, `sioul-movements.toml`: what the
/// person's files cannot say. Each entry is named by what it is about, so
/// that your devices merge them one by one, the newest word winning
/// (docs/database.md, part "projects and money").
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Store {
    /// Its format (`FORMAT`); 0 for no store yet.
    #[serde(default)]
    pub format: u32,
    #[serde(default, rename = "link")]
    pub links: Vec<Link>,
    #[serde(default, rename = "apart")]
    pub apart: Vec<Apart>,
    #[serde(default, rename = "move")]
    pub moves: Vec<Move>,
    #[serde(default, rename = "hold")]
    pub holds: Vec<Hold>,
    #[serde(default, rename = "name")]
    pub names: Vec<Name>,
}

/// A source joined to a movement, by the matching or by your hand; one per
/// source, named by it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Link {
    /// The movement, by its id or any of its sources' keys.
    pub movement: String,
    /// The source joined, by its key ("mid:…", "sioul:letter/…", "bank:…").
    pub source: String,
    /// "sure", "probable" (said "probably the same as …") or "hand".
    #[serde(default)]
    pub how: String,
    /// What a message said, kept so that the movement holds it without reading the mail again.
    #[serde(default)]
    pub amount: Option<Money>,
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub day: Option<Date>,
    #[serde(default)]
    pub label: String,
}

/// "Not the same": two sources that are never one movement, named by both
/// keys (in their order, so that the pair is one entry).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Apart {
    pub a: String,
    pub b: String,
}

/// A movement placed by your hand where the files cannot say it (an
/// occurrence moved, a line in parts), named by the movement. A bank line's
/// own placement stays in the budget file's `[[assign]]`, which every version reads.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Move {
    pub movement: String,
    #[serde(default)]
    pub budget: Option<String>,
    /// In parts: each budget its share; what they leave goes to `budget`.
    #[serde(default)]
    pub parts: Vec<crate::budget::Part>,
    /// In no budget.
    #[serde(default)]
    pub none: bool,
}

/// A card hold, as the bank's export listed it: not final yet. Named by its
/// account and its id; the last export of the account says which still hold.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Hold {
    /// The account's name as its exports give it (`bank::Movement::account`).
    pub account: String,
    pub id: String,
    #[serde(deserialize_with = "crate::budget::dates::required")]
    pub date: Date,
    pub amount: Money,
    #[serde(default)]
    pub label: String,
}

/// A payee's name as the bank writes it, taught by a correction: the name
/// its contract or its sender goes by. One per bank name.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Name {
    pub bank: String,
    pub known: String,
}

impl Store {
    pub fn path(root: &Path) -> PathBuf {
        root.join(STORE)
    }

    /// The store of a notes folder; none yet is an empty one (format 0).
    pub fn load(root: &Path) -> Result<Store, String> {
        let path = Store::path(root);
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Changes the store under its lock, which the sharing takes too: what
    /// this Sioul does not know (comments, the fields of a later Sioul) is
    /// kept; a store of a later format is never written. A new store says its
    /// format first.
    pub fn edit<R>(root: &Path, change: impl FnOnce(&mut Edit) -> R) -> Result<R, String> {
        let path = Store::path(root);
        crate::filelock::with_lock(&path, || {
            let fail = |e: String| format!("{}: {e}", path.display());
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(fail(e.to_string())),
            };
            let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
            let format = doc.get("format").and_then(Item::as_integer).unwrap_or(0);
            if format > i64::from(FORMAT) {
                return Err(fail(format!("written by a later Sioul (format {format}): left as it is")));
            }
            if format < i64::from(FORMAT) {
                doc.insert("format", value(i64::from(FORMAT)));
                if text.trim().is_empty() {
                    doc.decor_mut().set_prefix("# Sioul's own record of your money movements (docs/accounting.md, \"Movements\").\n# Your budget file stays the record; this says what it cannot.\n");
                }
            }
            let mut edit = Edit { doc };
            let out = change(&mut edit);
            let written = edit.doc.to_string();
            if written != text {
                let temporary = path.with_extension("toml.new");
                std::fs::write(&temporary, written).map_err(|e| fail(e.to_string()))?;
                std::fs::rename(&temporary, &path).map_err(|e| fail(e.to_string()))?;
            }
            Ok(out)
        })
    }
}

/// The store open for a change (`Store::edit`).
pub struct Edit {
    doc: DocumentMut,
}

/// A day as the files write it.
fn day_item(d: Date) -> Item {
    value(toml_edit::Datetime { date: Some(toml_edit::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None })
}

fn money_item(m: Money) -> Item {
    value(m.cents() as f64 / 100.0)
}

impl Edit {
    fn list(&mut self, name: &str) -> &mut ArrayOfTables {
        let item = self.doc.entry(name).or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
        if !item.is_array_of_tables() {
            *item = Item::ArrayOfTables(ArrayOfTables::new());
        }
        item.as_array_of_tables_mut().expect("a list of tables")
    }

    fn position(list: &ArrayOfTables, keys: &[(&str, &str)]) -> Option<usize> {
        list.iter().position(|t| keys.iter().all(|(k, v)| t.get(k).and_then(Item::as_str) == Some(*v)))
    }

    /// Puts an entry named by `keys`: over the one so named, its other
    /// fields kept, else at the end. A field given `None` is taken out.
    fn put(&mut self, name: &str, keys: &[(&str, &str)], fields: Vec<(&str, Option<Item>)>) {
        let list = self.list(name);
        let at = match Edit::position(list, keys) {
            Some(at) => at,
            None => {
                let mut table = Table::new();
                for (k, v) in keys {
                    table[*k] = value(*v);
                }
                list.push(table);
                list.len() - 1
            }
        };
        let Some(table) = list.get_mut(at) else { return };
        for (field, item) in fields {
            match item {
                Some(item) => table[field] = item,
                None => {
                    table.remove(field);
                }
            }
        }
    }

    fn remove(&mut self, name: &str, keys: &[(&str, &str)]) -> bool {
        let Some(list) = self.doc.get_mut(name).and_then(Item::as_array_of_tables_mut) else { return false };
        let before = list.len();
        list.retain(|t| !keys.iter().all(|(k, v)| t.get(k).and_then(Item::as_str) == Some(*v)));
        list.len() != before
    }

    /// `source` joined to the movement `movement` names; `how`: "sure",
    /// "probable" or "hand"; what a message said, when it is one.
    pub fn link(&mut self, movement: &str, source: &str, how: &str, said: Option<(Money, Date, &str)>) {
        let (amount, day, label) = match said {
            Some((amount, day, label)) => (Some(money_item(amount)), Some(day_item(day)), Some(value(label))),
            None => (None, None, None),
        };
        self.put("link", &[("source", source)], vec![("movement", Some(value(movement))), ("how", Some(value(how))), ("amount", amount), ("day", day), ("label", label)]);
    }

    /// `source` no longer joined by a link of the store.
    pub fn unlink(&mut self, source: &str) -> bool {
        self.remove("link", &[("source", source)])
    }

    /// "Not the same": `a` and `b` never one movement; a link between them goes.
    pub fn apart(&mut self, a: &str, b: &str) {
        let (a, b) = ordered(a, b);
        self.put("apart", &[("a", a), ("b", b)], Vec::new());
        let joined = |link: &Table| {
            let (m, s) = (link.get("movement").and_then(Item::as_str).unwrap_or(""), link.get("source").and_then(Item::as_str).unwrap_or(""));
            (m == a && s == b) || (m == b && s == a)
        };
        if let Some(list) = self.doc.get_mut("link").and_then(Item::as_array_of_tables_mut) {
            list.retain(|t| !joined(t));
        }
    }

    /// The same again: "Not the same" undone.
    pub fn together(&mut self, a: &str, b: &str) -> bool {
        let (a, b) = ordered(a, b);
        self.remove("apart", &[("a", a), ("b", b)])
    }

    /// The movement `movement` names placed by your hand: in `budget`, in
    /// `parts` (the rest to `budget`), or in none.
    pub fn place(&mut self, movement: &str, budget: Option<&str>, parts: &[(String, Money)], none: bool) {
        let parts_item = (!parts.is_empty()).then(|| {
            let mut array = toml_edit::Array::new();
            for (budget, amount) in parts {
                let mut part = toml_edit::InlineTable::new();
                part.insert("budget", budget.as_str().into());
                part.insert("amount", (amount.cents() as f64 / 100.0).into());
                array.push(part);
            }
            value(array)
        });
        self.put("move", &[("movement", movement)], vec![("budget", budget.map(value)), ("parts", parts_item), ("none", none.then(|| value(true)))]);
    }

    /// Back to where the files place it.
    pub fn unplace(&mut self, movement: &str) -> bool {
        self.remove("move", &[("movement", movement)])
    }

    /// The holds an account's last export listed, in place of those before.
    pub fn holds(&mut self, account: &str, holds: &[Hold]) {
        if let Some(list) = self.doc.get_mut("hold").and_then(Item::as_array_of_tables_mut) {
            list.retain(|t| t.get("account").and_then(Item::as_str) != Some(account));
        }
        for hold in holds {
            self.put("hold", &[("account", account), ("id", hold.id.as_str())], vec![("date", Some(day_item(hold.date))), ("amount", Some(money_item(hold.amount))), ("label", Some(value(hold.label.as_str())))]);
        }
    }

    /// A payee's name as the bank writes it, and the name it goes by.
    pub fn name(&mut self, bank: &str, known: &str) {
        self.put("name", &[("bank", bank)], vec![("known", Some(value(known)))]);
    }
}

/// Two keys in their order: a pair named once whichever comes first.
fn ordered<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

// ---------------------------------------------------------------- deriving

/// What movements are derived from.
pub struct Input<'a> {
    /// The budget file as it is written (never `Ledger::with_bank`'s).
    pub ledger: &'a Ledger,
    pub bank: &'a Bank,
    pub contracts: &'a [Contract],
    pub letters: &'a [Letter],
    pub store: &'a Store,
    pub words: &'a crate::words::Words,
}

/// A hold no longer holds this long after its day: some are never posted (Plaid).
const HOLD_DAYS: i64 = 14;
/// Days around a letter's date within which it announces a planned movement.
const LETTER_DAYS: i64 = 10;
/// Days after a planned one within which the bank's line may come, and before.
const LATER_DAYS: i64 = 7;
const EARLIER_DAYS: i64 = 3;
/// Occurrences are made this far beyond the days asked, for lines at the edges to find theirs.
const EDGE_DAYS: i64 = 45;

fn shifted(day: Date, days: i64) -> Date {
    day.checked_add(days.days()).unwrap_or(day)
}

fn days_apart(a: Date, b: Date) -> i64 {
    i64::from((a - b).get_days()).abs()
}

fn way_of(amount: Money) -> Way {
    if amount.is_negative() { Way::Out } else { Way::In }
}

/// FNV-1a: a short digest of what a line holds, the same on every device.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// Each line's key: `line:<id>`, else, for one written by hand or by an
/// older Sioul, `line:~<digest of what it holds>` (the n-th line just like
/// it after a dash), the same on every device.
pub fn line_keys(ledger: &Ledger) -> Vec<String> {
    let mut seen: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
    ledger
        .lines
        .iter()
        .map(|l| {
            if !l.id.trim().is_empty() {
                return format!("line:{}", l.id.trim());
            }
            let digest = fnv(&format!("{}|{}|{}|{}", l.budget, l.date, l.amount.cents(), l.label));
            let n = seen.entry(digest).or_insert(0);
            *n += 1;
            if *n == 1 { format!("line:~{digest:016x}") } else { format!("line:~{digest:016x}-{n}") }
        })
        .collect()
}

/// A preset's name for lines and occurrences: its id, else its place (`#3`), as `accounts` names it.
fn preset_key(ledger: &Ledger, index: usize) -> String {
    ledger.presets[index].id.clone().filter(|id| !id.trim().is_empty()).unwrap_or_else(|| format!("#{index}"))
}

/// An occurrence's key: "preset:<preset>/<day>".
pub fn occurrence_key(preset: &str, day: Date) -> String {
    format!("preset:{preset}/{day}")
}

/// A bank line's key: "bank:<account>/<the bank's id>".
pub fn bank_key(account: &str, id: &str) -> String {
    format!("bank:{account}/{id}")
}

/// A hold's key: "hold:<account>/<id>".
pub fn hold_key(account: &str, id: &str) -> String {
    format!("hold:{account}/{id}")
}

/// A preset's amount as sure as it is: a range when it has one, about when said so.
fn preset_sum(preset: &Preset) -> Sum {
    let signed = |m: Money| if preset.amount.is_negative() { Money(-m.cents().abs()) } else { Money(m.cents().abs()) };
    match (preset.low, preset.high) {
        (Some(low), Some(high)) if low != high => Sum::Between(signed(low), signed(high)),
        _ if preset.about => Sum::About(preset.amount),
        _ => Sum::Exact(preset.amount),
    }
}

/// A line's links: the documents that tell of it, and the rest (tasks, events, projects).
fn documents(links: &[String]) -> (Vec<Source>, Vec<String>) {
    let mut documents = Vec::new();
    let mut rest = Vec::new();
    for link in links {
        match Source::of_key(link) {
            Some(source) if source.kind.announces() || source.kind == Kind::Contract => documents.push(source),
            _ => rest.push(link.clone()),
        }
    }
    (documents, rest)
}

/// The fixed preset a line stands for in its period, by its place.
fn stood_for(ledger: &Ledger, line: &Line) -> Option<usize> {
    let id = line.preset.as_deref()?;
    let index = ledger.presets.iter().position(|p| p.id.as_deref() == Some(id) && p.budget == line.budget && !p.estimate)?;
    let preset = &ledger.presets[index];
    let (start, end) = preset.every.bounds(line.date);
    (!preset.occurrences(start, end).is_empty()).then_some(index)
}

/// Every movement whose day falls between `from` and `to`, in day order
/// (see the module's notes): occurrences of the fixed presets and the lines
/// of the file; the bank's lines and the holds, placed as `accounts::place`
/// places them, each joining the line or the occurrence it is the same
/// payment as; transfers as one movement with two sides; lines of accounts
/// not declared only confirming occurrences, as the watch does; letters with
/// an amount and a date; then the store's links, "Not the same" and
/// placements; then what was not seen.
pub fn derive(input: &Input, from: Date, to: Date, today: Date) -> Vec<Movement> {
    let ledger = input.ledger;
    let store = input.store;
    let filler = &input.words.bank.filler;
    let apart_pairs: BTreeSet<(String, String)> = store.apart.iter().map(|a| ordered(&a.a, &a.b)).map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let apart = |a: &str, b: &str| {
        let (a, b) = ordered(a, b);
        apart_pairs.contains(&(a.to_string(), b.to_string()))
    };
    // "Not the same" between a source and any source of a movement, or between two movements.
    let apart_from = |key: &str, m: &Movement| m.sources.iter().any(|s| apart(key, &s.key));
    let apart_both = |a: &Movement, b: &Movement| a.sources.iter().any(|s| apart_from(&s.key, b));
    let lines = line_keys(ledger);
    // The account that fills a budget first carries it (`accounts::carried_by`).
    let carrier = |budget: &str| ledger.bank_accounts.iter().find(|a| a.fills.iter().any(|b| b == budget)).map(|a| a.id.clone());
    let (lo, hi) = (shifted(from, -EDGE_DAYS), shifted(to, EDGE_DAYS));
    let mut out: Vec<Movement> = Vec::new();

    // 1. Each fixed preset's occurrences, with the contracts that name it.
    for (index, preset) in ledger.presets.iter().enumerate().filter(|(_, p)| !p.estimate) {
        let key = preset_key(ledger, index);
        let contracts: Vec<Source> = input.contracts.iter().filter(|c| !c.preset.is_empty() && preset.id.as_deref() == Some(c.preset.as_str())).map(|c| Source::new(Kind::Contract, crate::links::contract_uri(&c.id), crate::links::contract_uri(&c.id))).collect();
        for day in preset.occurrences(lo, hi) {
            let id = occurrence_key(&key, day);
            let mut sources = vec![Source::new(Kind::Preset, id.clone(), String::new())];
            sources.extend(contracts.iter().cloned());
            out.push(Movement {
                id,
                account: carrier(&preset.budget),
                way: way_of(preset.amount),
                sum: preset_sum(preset),
                currency: "EUR".into(),
                day,
                expected: None,
                label: preset.label.clone(),
                place: Place::Budget(preset.budget.clone()),
                area: Area::default(),
                sources,
                links: preset.links.clone(),
                state: State::Planned,
                waits: true,
            });
        }
    }

    // 2. The lines of the file; one standing for a fixed preset joins its occurrence.
    for (place, line) in ledger.lines.iter().enumerate() {
        // A bank account's movement counted in memory (`with_bank`) is no line of the file.
        if line.links.iter().any(|k| k.starts_with("bank:")) {
            continue;
        }
        let key = lines[place].clone();
        let (docs, links) = documents(&line.links);
        let state = match (line.planned, docs.iter().any(|d| d.kind.announces())) {
            (_, true) => State::Announced,
            (true, false) => State::Planned,
            (false, false) => State::Done,
        };
        let source = Source::new(Kind::Line, key.clone(), crate::links::budget_uri(&line.budget, place));
        if let Some(index) = stood_for(ledger, line) {
            let prefix = format!("preset:{}/", preset_key(ledger, index));
            let (start, end) = ledger.presets[index].every.bounds(line.date);
            let occurrence = out.iter().position(|m| {
                m.sources.first().is_some_and(|s| s.kind == Kind::Preset && s.key.starts_with(&prefix)) && m.day >= start && m.day <= end && !m.has_kind(Kind::Line) && !apart_from(&key, m)
            });
            if let Some(at) = occurrence {
                let m = &mut out[at];
                if !line.planned && m.day != line.date {
                    m.expected = Some(m.day);
                }
                m.day = line.date;
                m.sum = Sum::Exact(line.amount);
                m.state = state;
                m.sources.push(source);
                m.sources.extend(docs);
                m.links.extend(links);
                // A receipt says it was paid: only what is still planned waits for the bank's line.
                m.waits = line.planned;
                m.anchor();
                continue;
            }
        }
        out.push(Movement {
            id: key,
            account: carrier(&line.budget),
            way: way_of(line.amount),
            sum: Sum::Exact(line.amount),
            currency: "EUR".into(),
            day: line.date,
            expected: None,
            label: line.label.clone(),
            place: match &line.reserve {
                Some(reserve) => Place::Reserve { reserve: reserve.clone(), budget: line.budget.clone() },
                None => Place::Budget(line.budget.clone()),
            },
            area: Area::default(),
            sources: std::iter::once(source).chain(docs).collect(),
            links,
            state,
            // Announced for a day to come (a bill), or planned: the bank's line is awaited.
            waits: line.planned,
        });
    }

    // 3. The bank's lines and the holds of the declared accounts, placed. Each
    // source's movement, and each preset's occurrences, found by their keys.
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut by_preset: HashMap<String, Vec<usize>> = HashMap::new();
    for (at, m) in out.iter().enumerate() {
        for s in &m.sources {
            index.insert(s.key.clone(), at);
            if let Some((preset, _)) = s.key.strip_prefix("preset:").and_then(|rest| rest.rsplit_once('/')) {
                by_preset.entry(preset.to_string()).or_default().push(at);
            }
        }
    }
    let holds = live_holds(store, input.bank, today);
    let mut bank = input.bank.clone();
    bank.movements.extend(holds.iter().map(|h| bank::Movement { account: h.account.clone(), date: h.date, amount: h.amount, label: h.label.clone(), id: h.id.clone() }));
    let is_hold = |account: &str, id: &str| holds.iter().any(|h| h.id == id && accounts::account_of(ledger, &h.account).is_some_and(|a| a.id == account));
    let key_of = |account: &str, id: &str| if is_hold(account, id) { hold_key(account, id) } else { bank_key(account, id) };
    let placed = accounts::place_with(ledger, &bank, input.words, &|account, id, other| {
        let key = key_of(account, id);
        match other {
            Other::Line(i) => lines.get(i).is_some_and(|line| apart(&key, line)),
            Other::Preset(i, day) => apart(&key, &occurrence_key(&preset_key(ledger, i), day)),
        }
    });
    let mut used: BTreeSet<(String, String)> = BTreeSet::new();
    for p in &placed {
        // Outside the days asked, it joins nothing asked of.
        if p.movement.date < lo || p.movement.date > hi {
            continue;
        }
        let hold = is_hold(&p.account, &p.movement.id);
        let key = key_of(&p.account, &p.movement.id);
        used.insert((p.movement.account.clone(), p.movement.id.clone()));
        let source = Source::new(if hold { Kind::Hold } else { Kind::Bank }, key.clone(), String::new());
        // The line, or the occurrence, it is the same payment as.
        let target = match &p.place {
            Placed::Known(i) => lines.get(*i).and_then(|line| index.get(line).copied()),
            Placed::Budgets(shares) if shares.len() == 1 => shares[0].preset.as_deref().and_then(|preset| {
                let fixed = (0..ledger.presets.len()).find(|&i| !ledger.presets[i].estimate && preset_key(ledger, i) == preset)?;
                let (start, end) = ledger.presets[fixed].every.bounds(p.movement.date);
                let free = |at: usize| !out[at].has_kind(Kind::Bank) && !apart_from(&key, &out[at]);
                match p.occurrence {
                    Some(day) => index.get(&occurrence_key(preset, day)).copied().filter(|&at| free(at)),
                    None => by_preset.get(preset)?.iter().copied().filter(|&at| free(at) && out[at].day >= start && out[at].day <= end).min_by_key(|&at| days_apart(out[at].day, p.movement.date)),
                }
            }),
            _ => None,
        };
        if let Some(at) = target {
            let m = &mut out[at];
            m.settle(&p.account, p.movement.date, p.movement.amount, hold);
            m.sources.push(source);
            m.anchor();
            index.insert(key, at);
            continue;
        }
        let (way, place) = match &p.place {
            Placed::Budgets(shares) if shares.len() == 1 => (way_of(p.movement.amount), Place::Budget(shares[0].budget.clone())),
            Placed::Budgets(shares) => (way_of(p.movement.amount), Place::Parts(shares.iter().map(|s| (s.budget.clone(), s.amount)).collect())),
            Placed::Reserve { reserve, budget } => (way_of(p.movement.amount), Place::Reserve { reserve: reserve.clone(), budget: budget.clone() }),
            Placed::Transfer(other) if p.movement.amount.is_negative() => (Way::Between, Place::Transfer { from: p.account.clone(), to: other.clone() }),
            Placed::Transfer(other) => (Way::Between, Place::Transfer { from: other.clone(), to: p.account.clone() }),
            Placed::Known(_) | Placed::Nowhere => (way_of(p.movement.amount), Place::Nowhere),
        };
        index.insert(key.clone(), out.len());
        out.push(Movement {
            id: key,
            account: Some(p.account.clone()),
            way,
            sum: if hold { Sum::About(p.movement.amount) } else { Sum::Exact(p.movement.amount) },
            currency: "EUR".into(),
            day: p.movement.date,
            expected: None,
            label: p.movement.label.clone(),
            place,
            area: Area::default(),
            sources: vec![source],
            links: Vec::new(),
            state: if hold { State::NotFinal } else { State::Done },
            waits: false,
        });
    }
    drop(index);
    pair_transfers(&mut out);

    // 4. Occurrences and planned lines still waiting: a line of an account not
    // declared (which keeps no books: the watch's own test), or a declared
    // one's that a rule or the first budget put in the same budget for
    // nothing more precise, tells they passed.
    let mut order: Vec<usize> = (0..out.len()).filter(|&i| out[i].waits && (out[i].has_kind(Kind::Preset) || out[i].has_kind(Kind::Line)) && !out[i].has_kind(Kind::Bank) && !out[i].has_kind(Kind::Hold)).collect();
    order.sort_by_key(|&i| out[i].day);
    let mut absorbed: BTreeSet<usize> = BTreeSet::new();
    let undeclared: Vec<&bank::Movement> = input.bank.movements.iter().filter(|b| accounts::account_of(ledger, &b.account).is_none()).collect();
    let why_of: HashMap<String, (accounts::Why, &bank::Movement)> = placed.iter().map(|p| (bank_key(&p.account, &p.movement.id), (p.why, &p.movement))).collect();
    for i in order {
        let (label, amount, day, budget) = (bank::words(filler, &out[i].label), out[i].sum.amount(), out[i].day, out[i].place.budgets().first().map(|b| b.to_string()));
        let undeclared = undeclared.iter().filter(|b| !used.contains(&(b.account.clone(), b.id.clone()))).filter(|b| bank::matches(filler, b, &label, amount, day)).filter(|b| !apart_from(&bank_key(&b.account, &b.id), &out[i])).min_by_key(|b| days_apart(b.date, day)).map(|b| (*b).clone());
        if let Some(b) = undeclared {
            used.insert((b.account.clone(), b.id.clone()));
            let m = &mut out[i];
            m.settle(&b.account, b.date, b.amount, false);
            m.sources.push(Source::new(Kind::Bank, bank_key(&b.account, &b.id), String::new()));
            m.anchor();
            continue;
        }
        let (near_from, near_to) = (shifted(day, -EARLIER_DAYS), shifted(day, LATER_DAYS));
        let declared = (0..out.len())
            .filter(|&j| out[j].day >= near_from && out[j].day <= near_to && j != i && !absorbed.contains(&j) && out[j].sources.len() == 1 && out[j].sources[0].kind == Kind::Bank)
            .filter(|&j| budget.as_deref().is_some_and(|b| out[j].place == Place::Budget(b.to_string())))
            .filter(|&j| why_of.get(&out[j].id).is_some_and(|(why, b)| matches!(why, accounts::Why::Rule | accounts::Why::First) && bank::matches(filler, b, &label, amount, day)))
            .filter(|&j| !apart_both(&out[j], &out[i]))
            .min_by_key(|&j| days_apart(out[j].day, day));
        if let Some(j) = declared {
            absorbed.insert(j);
            let line = out[j].clone();
            let m = &mut out[i];
            m.settle(line.account.as_deref().unwrap_or_default(), line.day, line.sum.amount(), false);
            m.sources.extend(line.sources);
            m.anchor();
        }
    }
    let mut index = 0;
    out.retain(|_| {
        let keep = !absorbed.contains(&index);
        index += 1;
        keep
    });

    // 5. Letters with an amount and a date asked: a bill announced, joined to
    // the planned movement it announces, else proposed in no budget.
    for letter in input.letters.iter().filter(|l| l.status != "done") {
        let (Some(amount), Some(day)) = (letter.reading.amount, letter.reading.deadline) else { continue };
        if !matches!(letter.reading.kind, crate::letters::Kind::Bill | crate::letters::Kind::TaxNotice | crate::letters::Kind::Reminder | crate::letters::Kind::FormalNotice) || amount == Money::ZERO {
            continue;
        }
        let key = crate::links::letter_uri(&letter.id);
        if out.iter().any(|m| m.has(&key)) {
            continue;
        }
        let amount = Money(-amount.cents().abs());
        let source = Source::new(Kind::Letter, key.clone(), key.clone());
        let near = |m: &Movement| m.waits && m.state != State::Done && m.way == Way::Out && (m.sum.amount().cents() - amount.cents()).abs() <= amount.cents().abs() / 100 + 1 && days_apart(m.day, day) <= LETTER_DAYS && !apart_from(&key, m);
        match out.iter().enumerate().filter(|(_, m)| near(m)).min_by_key(|(_, m)| days_apart(m.day, day)).map(|(at, _)| at) {
            Some(at) => {
                let m = &mut out[at];
                m.sources.push(source);
                if m.state == State::Planned {
                    m.state = State::Announced;
                }
            }
            None => out.push(Movement {
                id: key,
                account: None,
                way: Way::Out,
                sum: Sum::Exact(amount),
                currency: "EUR".into(),
                day,
                expected: None,
                label: letter.reading.sender.clone(),
                place: Place::Unplaced,
                area: Area::default(),
                sources: vec![source],
                links: Vec::new(),
                state: State::Announced,
                waits: true,
            }),
        }
    }

    // 6. The store: sources joined, then placements by your hand.
    for link in &store.links {
        let Some(target) = out.iter().position(|m| m.has(&link.movement)) else { continue };
        let probable = link.how == "probable";
        if let Some(own) = out.iter().position(|m| m.has(&link.source)) {
            if own != target && apart_both(&out[own], &out[target]) {
                continue;
            }
            if own == target {
                if let Some(s) = out[target].sources.iter_mut().find(|s| s.key == link.source) {
                    s.probable = probable;
                }
                continue;
            }
            // The source's own movement (a bank line's, a letter's) is the same one: merged.
            let other = out.remove(own);
            let target = if own < target { target - 1 } else { target };
            merge(&mut out[target], other, probable);
            continue;
        }
        if apart_from(&link.source, &out[target]) {
            continue;
        }
        let Some(mut source) = Source::of_key(&link.source) else { continue };
        source.probable = probable;
        let m = &mut out[target];
        if source.kind.announces() && m.state == State::Planned {
            m.state = State::Announced;
        }
        m.sources.push(source);
    }
    for placed in &store.moves {
        let Some(m) = out.iter_mut().find(|m| m.has(&placed.movement)) else { continue };
        m.place = if placed.none {
            Place::Nowhere
        } else if !placed.parts.is_empty() {
            let mut parts: Vec<(String, Money)> = placed.parts.iter().map(|p| (p.budget.clone(), p.amount)).collect();
            let rest = m.sum.amount() - parts.iter().map(|(_, a)| *a).sum::<Money>();
            if let Some(budget) = placed.budget.clone().or_else(|| m.place.budgets().first().map(|b| b.to_string())).filter(|_| rest != Money::ZERO) {
                parts.push((budget, rest));
            }
            Place::Parts(parts)
        } else if let Some(budget) = &placed.budget {
            Place::Budget(budget.clone())
        } else {
            continue;
        };
    }

    // 7. Not seen: its window passed inside what its account's exports cover,
    // with no line. With no account declared, every export tells, as the watch reads them.
    let mut of_account: HashMap<Option<String>, Vec<&bank::Movement>> = HashMap::new();
    for b in &input.bank.movements {
        let card = accounts::account_of(ledger, &b.account).map(|a| a.id.clone());
        if card.is_some() || ledger.bank_accounts.is_empty() {
            of_account.entry(card).or_default().push(b);
        }
    }
    for m in out.iter_mut().filter(|m| m.waits && matches!(m.state, State::Planned | State::Announced)) {
        let (start, end) = (shifted(m.day, -EARLIER_DAYS), shifted(m.day, LATER_DAYS));
        if end >= today {
            continue;
        }
        let whose = if ledger.bank_accounts.is_empty() { None } else { m.account.clone() };
        if whose.is_none() && !ledger.bank_accounts.is_empty() {
            continue;
        }
        let Some(theirs) = of_account.get(&whose) else { continue };
        let (Some(first), Some(last)) = (theirs.iter().map(|b| b.date).min(), theirs.iter().map(|b| b.date).max()) else { continue };
        let label = bank::words(filler, &m.label);
        // A line plainly its own, placed elsewhere by a rule, is no doubt: it passed.
        let passed = theirs.iter().any(|b| bank::matches(filler, b, &label, m.sum.amount(), m.day));
        if first <= start && last >= end && !passed {
            m.state = State::NotSeen;
        }
    }

    // 8. What each is for: its budgets', else its account's.
    let budget_area = |id: &str| ledger.budgets.iter().find(|b| b.id == id).map_or(Area::ADMIN, crate::budget::Budget::area_of);
    let account_area = |id: &str| ledger.bank_accounts.iter().find(|a| a.id == id).map(|a| a.fills.iter().fold(Area::default(), |area, b| area.with(budget_area(b)))).filter(|a| !a.is_empty()).unwrap_or(Area::ADMIN);
    for m in &mut out {
        m.area = match &m.place {
            Place::Budget(_) | Place::Parts(_) | Place::Reserve { .. } => m.place.budgets().iter().fold(Area::default(), |area, b| area.with(budget_area(b))),
            Place::Transfer { from, to } => account_area(from).with(account_area(to)),
            Place::Nowhere | Place::Unplaced => m.account.as_deref().map_or(Area::ADMIN, account_area),
        };
    }
    out.retain(|m| m.day >= from && m.day <= to);
    out.sort_by(|a, b| (a.day, &a.id).cmp(&(b.day, &b.id)));
    out
}

/// The holds still holding: listed by the store, not yet posted (a line of
/// the bank with the same id, or on the same account within two weeks with
/// a word of its label or nearly its amount), and younger than two weeks.
fn live_holds(store: &Store, bank: &Bank, today: Date) -> Vec<Hold> {
    store
        .holds
        .iter()
        .filter(|h| days_apart(h.date, today) <= HOLD_DAYS || h.date > today)
        .filter(|h| {
            let words = bank::words(&[], &h.label);
            !bank.movements.iter().any(|b| {
                let same_account = b.account == h.account;
                let posted_later = b.date >= shifted(h.date, -1) && b.date <= shifted(h.date, HOLD_DAYS);
                let near = (b.amount.cents() - h.amount.cents()).abs() <= h.amount.cents().abs() / 5;
                let named = bank::words(&[], &b.label).iter().any(|w| words.contains(w));
                (same_account && b.id == h.id) || (same_account && posted_later && b.amount.is_negative() == h.amount.is_negative() && (named || near))
            })
        })
        .cloned()
        .collect()
}

/// Two sides of one transfer, each read from its own account's exports,
/// made one movement: the same accounts, opposite amounts, ten days apart at most.
fn pair_transfers(out: &mut Vec<Movement>) {
    let mut i = 0;
    while i < out.len() {
        let Place::Transfer { from, to } = out[i].place.clone() else {
            i += 1;
            continue;
        };
        let partner = (0..out.len()).find(|&j| {
            j != i && out[j].place == (Place::Transfer { from: from.clone(), to: to.clone() }) && out[j].sources.len() == 1 && out[i].sources.len() == 1 && out[j].sum.amount() == -out[i].sum.amount() && days_apart(out[j].day, out[i].day) <= 10 && out[j].account != out[i].account
        });
        if let Some(j) = partner {
            let other = out.remove(j);
            let i = if j < i { i - 1 } else { i };
            let m = &mut out[i];
            // Said from the account it left.
            let leaving = if m.sum.amount().is_negative() { m.sum } else { other.sum };
            m.day = m.day.min(other.day);
            m.sum = leaving;
            m.account = Some(from.clone());
            m.sources.extend(other.sources);
            m.anchor();
            continue;
        }
        i += 1;
    }
}

/// `other` is the same payment as `into`: its sources join; the bank's facts win.
fn merge(into: &mut Movement, other: Movement, probable: bool) {
    let settled = other.has_kind(Kind::Bank) || other.has_kind(Kind::Hold);
    if settled && !into.has_kind(Kind::Bank) {
        if into.day != other.day {
            into.expected = Some(into.expected.unwrap_or(into.day));
        }
        into.day = other.day;
        into.sum = other.sum;
        into.account = other.account.clone().or(into.account.take());
        into.state = other.state;
    } else if other.state == State::Announced && into.state == State::Planned {
        into.state = State::Announced;
    }
    into.sources.extend(other.sources.into_iter().map(|mut s| {
        s.probable = probable;
        s
    }));
    into.links.extend(other.links);
    into.anchor();
}

/// The movements of a notes folder, from `from` to `to`: its budget file,
/// its bank's movements, its contracts and letters, and its store. A file
/// that does not read counts as empty here; the budgets page says it.
pub fn load(root: &Path, words: &crate::words::Words, from: Date, to: Date, today: Date) -> Movements {
    let ledger = Ledger::load(root).unwrap_or_default();
    let bank = Bank::load(root).unwrap_or_default();
    let contracts = crate::contracts::Contracts::load(root).map(|c| c.list).unwrap_or_default();
    let letters = crate::letters::Letters::load(root).map(|l| l.list).unwrap_or_default();
    let store = Store::load(root).unwrap_or_default();
    let input = Input { ledger: &ledger, bank: &bank, contracts: &contracts, letters: &letters, store: &store, words };
    Movements { list: derive(&input, from, to, today) }
}

#[cfg(test)]
#[path = "movements_tests.rs"]
mod tests;

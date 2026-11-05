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

use crate::accounts::{self, Place as Placed};
use crate::matching::Evidence;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
    /// A work budget's share of its incomes set aside for the declaration
    /// (`Budget::set_aside`): `setaside:<budget>/<the declaration's day>`.
    SetAside,
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
            k if k.starts_with("setaside:") => Kind::SetAside,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub kind: Kind,
    /// Its key, by which corrections name it: `line:<id>`, `preset:<id>/<day>`,
    /// `bank:<account>/<the bank's id>`, `hold:<account>/<id>`, `mid:<Message-ID>`,
    /// `sioul:letter/<id>`, `sioul:invoice/<number>`, `sioul:contract/<id>`.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(deserialize_with = "area_or_none")]
    pub area: Area,
    /// Every source, its anchor first.
    pub sources: Vec<Source>,
    /// What else it is tied to: tasks, events, projects, notes.
    pub links: Vec<String>,
    pub state: State,
    /// The movements it is probably together with, by their steadiest keys:
    /// an order charged in parts, instalments, two invoices paid at once, a
    /// deferred-debit card's sum and the card payments it settles.
    pub together: Vec<String>,
    /// It waits for a line of the bank: an occurrence, a planned line, a letter's bill.
    waits: bool,
    /// How `accounts::place` placed its bank line, when it is one's own.
    pub(crate) why: Option<accounts::Why>,
    /// What the matching compares: its kind, its references, the names it goes by.
    pub(crate) evidence: Option<crate::matching::Evidence>,
    refs: Vec<String>,
    names: Vec<String>,
    /// The preset its bank line stands for or is spent from, as `accounts::place` said (a rule naming it, an estimate).
    pub(crate) stands: Option<String>,
    /// Its bank line's label, as the bank wrote it: the payee's name for its next movements.
    bank_label: Option<String>,
}

/// An area as a kept copy writes it ("work+admin"), none when it says none.
fn area_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Area, D::Error> {
    let text = String::deserialize(d)?;
    Ok(Area::parse(&text).unwrap_or_default())
}

impl Movement {
    /// The preset its bank line stands for or is spent from, by its key, as its placement said.
    pub fn stands_for(&self) -> Option<&str> {
        self.stands.as_deref()
    }

    /// How its own bank line was placed (`accounts::Why`), when it is one's own.
    pub fn why_placed(&self) -> Option<accounts::Why> {
        self.why
    }

    /// Whether `key` names it: its id or one of its sources' keys.
    pub fn has(&self, key: &str) -> bool {
        self.id == key || self.sources.iter().any(|s| s.key == key)
    }

    /// The key that whatever is kept names it by (a `[[move]]`, a `[[link]]`'s
    /// movement, an address), the steadiest of its sources': its bank line's;
    /// else a line's with an id; else its occurrence; else its letter, its
    /// invoice, its mail; else a card hold's. A line written by hand without
    /// an id is named by a digest of what it holds, which changes as soon as
    /// the line is edited: only when nothing else names it. A contract names
    /// every occurrence of its preset, never one movement. Its `id` stays the
    /// first in `Kind`'s order, for one reading; what is kept outlives it.
    pub fn steady_key(&self) -> &str {
        steadiest(self.sources.iter()).unwrap_or(self.id.as_str())
    }

    /// Its address: `sioul:movement/<its steadiest key>` (`steady_key`); one
    /// naming any other of its sources finds it too (`Movements::find`).
    pub fn address(&self) -> String {
        crate::links::movement_uri(self.steady_key())
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

/// The steadiest key among `sources` (`Movement::steady_key`'s order): a bank
/// line's, a line's with an id, an occurrence's, a letter's, an invoice's, a
/// mail's, a hold's, a digest's last; a contract's never.
/// The steadiest key among `sources` (`steadiest`), for a window that
/// names a movement by what is left of it once a source goes ("Not the same").
pub fn steadiest_of<'a>(sources: impl Iterator<Item = &'a Source>) -> Option<&'a str> {
    steadiest(sources)
}

pub(crate) fn steadiest<'a>(sources: impl Iterator<Item = &'a Source>) -> Option<&'a str> {
    let rank = |s: &Source| match s.kind {
        Kind::Bank => 0,
        Kind::Line if !s.key.starts_with("line:~") => 1,
        Kind::Preset | Kind::SetAside => 2,
        Kind::Letter => 3,
        Kind::Invoice => 4,
        Kind::Mail => 5,
        Kind::Hold => 6,
        Kind::Line => 7,
        Kind::Contract => 8,
    };
    sources.filter(|s| s.kind != Kind::Contract).min_by(|a, b| (rank(a), &a.key).cmp(&(rank(b), &b.key))).map(|s| s.key.as_str())
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
    #[serde(default, rename = "decided")]
    pub decided: Vec<Decided>,
}

/// What you answered, once, after moving a movement: whether this payee's
/// next movements go to that budget too. Named by the payee, so it is never
/// asked again; a yes is also a rule of the budget file (`[[split]]`), which
/// every version reads, and which the bank's rules list shows.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Decided {
    /// The payee's name, as the matching compares it (`matching::tokens`, joined by spaces).
    pub payee: String,
    pub budget: String,
    /// Yes: its next movements go there too.
    #[serde(default)]
    pub next: bool,
}

/// A source joined to a movement, by the matching or by your hand; one per
/// source, named by it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Link {
    /// The movement, by its steadiest key (`Movement::steady_key`), or any of its sources' keys.
    pub movement: String,
    /// The source joined, by its key ("mid:…", "sioul:letter/…", "bank:…").
    pub source: String,
    /// The other movements it is probably together with, by their steadiest
    /// keys: the charges of an order in parts, a purchase's instalments.
    #[serde(default)]
    pub together: Vec<String>,
    /// A purchase paid in instalments: how many, as its message says.
    #[serde(default)]
    pub instalments: Option<u32>,
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
                crate::changes::said(crate::changes::Kind::Money);
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

    /// The other movements a joined source is probably together with, and a
    /// purchase's instalments, on its link (`link` first).
    pub fn link_together(&mut self, source: &str, others: &[String], instalments: Option<u32>) {
        let others = (!others.is_empty()).then(|| value(others.iter().map(String::as_str).collect::<toml_edit::Array>()));
        self.put("link", &[("source", source)], vec![("together", others), ("instalments", instalments.map(|n| value(i64::from(n))))]);
    }

    /// Your answer, once, on a payee's next movements.
    pub fn decide(&mut self, payee: &str, budget: &str, next: bool) {
        self.put("decided", &[("payee", payee)], vec![("budget", Some(value(budget))), ("next", Some(value(next)))]);
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
pub fn preset_key(ledger: &Ledger, index: usize) -> String {
    ledger.presets[index].id.clone().filter(|id| !id.trim().is_empty()).unwrap_or_else(|| format!("#{index}"))
}

/// An occurrence's key: `preset:<preset>/<day>`.
pub fn occurrence_key(preset: &str, day: Date) -> String {
    format!("preset:{preset}/{day}")
}

/// A bank line's key: `bank:<account>/<the bank's id>`.
pub fn bank_key(account: &str, id: &str) -> String {
    format!("bank:{account}/{id}")
}

/// A hold's key: `hold:<account>/<id>`.
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
    let (start, end) = ledger.preset_bounds(preset, line.date);
    (!ledger.occurrences_of(preset, start, end).is_empty()).then_some(index)
}

/// The day a work budget's share of an income on `day` is declared and
/// paid: the last day of the next month, or of the month after its quarter
/// (URSSAF's calendar for the self-employed, docs/accounting.md, "Work money").
pub fn declaration_day(day: Date, quarterly: bool) -> Date {
    let month = if quarterly { (i64::from(day.month()) - 1) / 3 * 3 + 3 } else { i64::from(day.month()) };
    let first = Date::new(day.year(), 1, 1).ok().and_then(|jan| jan.checked_add(jiff::Span::new().months(month)).ok());
    first.map_or(day, |f| f.last_of_month())
}

/// A reference as the matching compares it (`matching::references`): its kind, its letters and digits.
fn reference(kind: &str, value: &str) -> Option<String> {
    let compact: String = value.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_uppercase();
    (compact.len() >= 4).then(|| format!("{kind}:{compact}"))
}

/// A movement with nothing but what is given; the rest is filled by its source.
fn bare(id: String, sum: Sum, day: Date, label: String, place: Place, sources: Vec<Source>, state: State) -> Movement {
    let way = way_of(sum.amount());
    Movement { id, account: None, way, sum, currency: "EUR".into(), day, expected: None, label, place, area: Area::default(), sources, links: Vec::new(), state, together: Vec::new(), waits: false, why: None, evidence: None, refs: Vec::new(), names: Vec::new(), stands: None, bank_label: None }
}

/// Whether a movement waits for the bank's line, for the matching: what was
/// planned, written or announced, with no line of the bank yet.
fn expects(m: &Movement) -> bool {
    m.way != Way::Between && m.evidence.is_some() && !m.has_kind(Kind::Bank) && !m.has_kind(Kind::Hold) && (m.has_kind(Kind::Preset) || m.has_kind(Kind::Line) || m.has_kind(Kind::Letter))
}

/// Whether a movement is a line of the bank alone, for the matching to join.
fn is_fact(m: &Movement) -> bool {
    m.way != Way::Between && m.sources.len() == 1 && matches!(m.sources[0].kind, Kind::Bank | Kind::Hold)
}

/// Whether a label is a card payment's, by its first words (`BankWords::card`).
fn card_like(words: &crate::words::BankWords, label: &str) -> bool {
    let folded: String = crate::text::fold(label).into_iter().collect();
    words.card.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && (folded == w || folded.starts_with(&format!("{w} "))))
}

/// Every movement whose day falls between `from` and `to`, in day order
/// (see the module's notes): occurrences of the fixed presets and the lines
/// of the file; the bank's lines and the holds, placed as `accounts::place`
/// places them, each a movement of its own; transfers as one movement with
/// two sides; letters with an amount and a date; the store's links; then
/// the matching (`matching`), which joins each line of the bank to what was
/// planned or announced, lines of accounts not declared only confirming it,
/// as the watch does; then what is probably together; the store's
/// placements; what was not seen.
/// Each name's words for the matching (`matching::tokens`), made once in a
/// reading: a bank label comes back thousands of times in five years of exports.
struct NameWords<'a> {
    bank: &'a crate::words::BankWords,
    made: std::cell::RefCell<HashMap<String, Vec<String>>>,
}

impl NameWords<'_> {
    fn of(&self, text: &str) -> Vec<String> {
        if let Some(made) = self.made.borrow().get(text) {
            return made.clone();
        }
        let made = crate::matching::tokens(self.bank, text);
        self.made.borrow_mut().insert(text.to_string(), made.clone());
        made
    }
}

pub fn derive(input: &Input, from: Date, to: Date, today: Date) -> Vec<Movement> {
    let ledger = input.ledger;
    let tokens = NameWords { bank: &input.words.bank, made: std::cell::RefCell::new(HashMap::new()) };
    let store = input.store;
    let bank_words = &input.words.bank;
    let filler = &bank_words.filler;
    let leads = &input.words.payments.references;
    let apart_pairs: BTreeSet<(String, String)> = store.apart.iter().map(|a| ordered(&a.a, &a.b)).map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let apart = |a: &str, b: &str| {
        if apart_pairs.is_empty() {
            return false;
        }
        let (a, b) = ordered(a, b);
        apart_pairs.contains(&(a.to_string(), b.to_string()))
    };
    // "Not the same" between a source and any source of a movement, or between two movements.
    let apart_from = |key: &str, m: &Movement| m.sources.iter().any(|s| apart(key, &s.key));
    let apart_both = |a: &Movement, b: &Movement| a.sources.iter().any(|s| apart_from(&s.key, b));
    let lines = line_keys(ledger);
    // The account that fills a budget first carries it (`accounts::carried_by`);
    // any account that fills it may pay it.
    let carrier = |budget: &str| ledger.bank_accounts.iter().find(|a| a.fills.iter().any(|b| b == budget)).map(|a| a.id.clone());
    let payers = |budget: &str| -> Vec<String> { ledger.bank_accounts.iter().filter(|a| a.fills.iter().any(|b| b == budget)).map(|a| a.id.clone()).collect() };
    let (lo, hi) = (shifted(from, -EDGE_DAYS), shifted(to, EDGE_DAYS));
    let mut out: Vec<Movement> = Vec::new();

    // 1. Each fixed preset's occurrences, with the contracts that name it:
    // their names and their references (your number with them) too.
    for (index, preset) in ledger.presets.iter().enumerate().filter(|(_, p)| !p.estimate) {
        let key = preset_key(ledger, index);
        let named: Vec<&Contract> = input.contracts.iter().filter(|c| !c.preset.is_empty() && preset.id.as_deref() == Some(c.preset.as_str())).collect();
        let contracts: Vec<Source> = named.iter().map(|c| Source::new(Kind::Contract, crate::links::contract_uri(&c.id), crate::links::contract_uri(&c.id))).collect();
        let mut names = vec![preset.label.clone()];
        names.extend(named.iter().flat_map(|c| [c.party.clone(), c.title.clone()]).filter(|n| !n.trim().is_empty()));
        let refs: Vec<String> = named.iter().filter_map(|c| reference("ref", &c.reference)).collect();
        let evidence = if preset.amount.is_negative() { Evidence::Debit } else { Evidence::Transfer };
        for day in preset.occurrences(lo, hi) {
            let id = occurrence_key(&key, day);
            let mut sources = vec![Source::new(Kind::Preset, id.clone(), String::new())];
            sources.extend(contracts.iter().cloned());
            let mut m = bare(id, preset_sum(preset), day, preset.label.clone(), Place::Budget(preset.budget.clone()), sources, State::Planned);
            m.account = carrier(&preset.budget);
            m.links = preset.links.clone();
            m.waits = true;
            m.evidence = Some(evidence);
            m.refs = refs.clone();
            m.names = names.clone();
            out.push(m);
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
        let told = docs.iter().any(|d| d.kind.announces());
        let state = match (line.planned, told) {
            (_, true) => State::Announced,
            (true, false) => State::Planned,
            (false, false) => State::Done,
        };
        let invoice = docs.iter().any(|d| d.kind == Kind::Invoice);
        let refs: Vec<String> = docs.iter().filter(|d| d.kind == Kind::Invoice).filter_map(|d| reference("ref", &crate::links::id_of(&d.key))).collect();
        // What it is, for the matching: an invoice you issued; a payment a
        // message told (a receipt); else a payment or a transfer by hand, its amount exact.
        let evidence = if invoice {
            Evidence::Invoice
        } else if !line.planned && told {
            Evidence::Card
        } else {
            Evidence::Transfer
        };
        let source = Source::new(Kind::Line, key.clone(), crate::links::budget_uri(&line.budget, place));
        if let Some(index) = stood_for(ledger, line) {
            let prefix = format!("preset:{}/", preset_key(ledger, index));
            let (start, end) = ledger.preset_bounds(&ledger.presets[index], line.date);
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
                m.names.push(line.label.clone());
                m.refs.extend(refs);
                // A receipt says it was paid, and is matched as one; only what
                // is still planned waits for the bank's line.
                if !line.planned && told {
                    m.evidence = Some(Evidence::Card);
                }
                m.waits = line.planned;
                m.anchor();
                continue;
            }
        }
        let place = match &line.reserve {
            Some(reserve) => Place::Reserve { reserve: reserve.clone(), budget: line.budget.clone() },
            None => Place::Budget(line.budget.clone()),
        };
        let mut m = bare(key, Sum::Exact(line.amount), line.date, line.label.clone(), place, std::iter::once(source).chain(docs).collect(), state);
        m.account = carrier(&line.budget);
        m.links = links;
        // Planned, or announced for a day to come (a bill): the bank's line is awaited.
        m.waits = line.planned;
        m.evidence = Some(evidence);
        m.refs = refs;
        m.names = vec![line.label.clone()];
        out.push(m);
    }

    // 3. The bank's lines and the holds of the declared accounts, each placed
    // (by your hand, a transfer, a rule, the account's first budget: never by
    // the same payment, which the matching says below) and a movement of its own.
    let holds = live_holds(store, input.bank, bank_words, today);
    let mut bank = input.bank.clone();
    bank.movements.extend(holds.iter().map(|h| bank::Movement { account: h.account.clone(), date: h.date, amount: h.amount, label: h.label.clone(), id: h.id.clone() }));
    let is_hold = |account: &str, id: &str| holds.iter().any(|h| h.id == id && accounts::account_of(ledger, &h.account).is_some_and(|a| a.id == account));
    let key_of = |account: &str, id: &str| if is_hold(account, id) { hold_key(account, id) } else { bank_key(account, id) };
    // Everything "not the same" for `place`: the matching alone tells the same payment.
    let placed = accounts::place_with(ledger, &bank, input.words, &|_, _, _| true);
    // The name a correction taught for a payee's bank name (`[[name]]`).
    let taught = |label: &str| {
        if store.names.is_empty() {
            return None;
        }
        let payee = tokens.of(label).join(" ");
        store.names.iter().find(|n| !payee.is_empty() && n.bank == payee).map(|n| n.known.clone())
    };
    for p in &placed {
        // Outside the days asked, it joins nothing asked of.
        if p.movement.date < lo || p.movement.date > hi {
            continue;
        }
        let hold = is_hold(&p.account, &p.movement.id);
        let key = key_of(&p.account, &p.movement.id);
        let source = Source::new(if hold { Kind::Hold } else { Kind::Bank }, key.clone(), String::new());
        let (way, place) = match &p.place {
            Placed::Budgets(shares) if shares.len() == 1 => (way_of(p.movement.amount), Place::Budget(shares[0].budget.clone())),
            Placed::Budgets(shares) => (way_of(p.movement.amount), Place::Parts(shares.iter().map(|s| (s.budget.clone(), s.amount)).collect())),
            Placed::Reserve { reserve, budget } => (way_of(p.movement.amount), Place::Reserve { reserve: reserve.clone(), budget: budget.clone() }),
            Placed::Transfer(other) if p.movement.amount.is_negative() => (Way::Between, Place::Transfer { from: p.account.clone(), to: other.clone() }),
            Placed::Transfer(other) => (Way::Between, Place::Transfer { from: other.clone(), to: p.account.clone() }),
            Placed::Known(_) | Placed::Nowhere => (way_of(p.movement.amount), Place::Nowhere),
        };
        let sum = if hold { Sum::About(p.movement.amount) } else { Sum::Exact(p.movement.amount) };
        let mut m = bare(key, sum, p.movement.date, p.movement.label.clone(), place, vec![source], if hold { State::NotFinal } else { State::Done });
        m.way = way;
        m.account = Some(p.account.clone());
        m.why = Some(p.why);
        m.stands = match &p.place {
            Placed::Budgets(shares) if shares.len() == 1 => shares[0].preset.clone(),
            _ => None,
        };
        m.refs = crate::matching::label_references(leads, &p.movement.label);
        m.names = std::iter::once(p.movement.label.clone()).chain(taught(&p.movement.label)).collect();
        m.bank_label = Some(p.movement.label.clone());
        out.push(m);
    }
    pair_transfers(&mut out);
    // The lines of accounts not declared keep no books: they only confirm
    // what was planned, as the watch reads them.
    let confirming: Vec<bank::Movement> = input.bank.movements.iter().filter(|b| accounts::account_of(ledger, &b.account).is_none() && b.date >= lo && b.date <= hi).cloned().collect();

    // 4. Letters with an amount and a date asked: a bill announced, joined to
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
                m.names.push(letter.reading.sender.clone());
                m.refs.extend(reference("ref", &letter.reading.reference));
                if m.state == State::Planned {
                    m.state = State::Announced;
                }
            }
            None => {
                let mut m = bare(key, Sum::Exact(amount), day, letter.reading.sender.clone(), Place::Unplaced, vec![source], State::Announced);
                m.waits = true;
                m.evidence = Some(Evidence::Bill);
                m.names = vec![letter.reading.sender.clone()];
                m.refs = reference("ref", &letter.reading.reference).into_iter().collect();
                out.push(m);
            }
        }
    }

    // 5. The store's links: what the matching or your hand joined before
    // holds, whatever came since; an automatic link counts until you correct it.
    for link in &store.links {
        // A share set aside is made after the matching (7b): its links are read there.
        if link.movement.starts_with("setaside:") || link.source.starts_with("setaside:") {
            continue;
        }
        let Some(target) = out.iter().position(|m| m.has(&link.movement)) else { continue };
        let probable = link.how == "probable";
        if let Some(own) = out.iter().position(|m| m.has(&link.source)) {
            if own == target {
                if let Some(s) = out[target].sources.iter_mut().find(|s| s.key == link.source) {
                    s.probable = probable;
                }
                continue;
            }
            if apart_both(&out[own], &out[target]) {
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
        if !link.label.trim().is_empty() {
            m.names.push(link.label.clone());
        }
        m.sources.push(source);
    }

    // 6. The matching: what waits, against the bank's lines no link joined;
    // twice, the second time with the mandates' references the sure pairs
    // of the first taught (a debit's RUM and ICS, month after month).
    let mut used = vec![false; confirming.len()];
    for pass in 0..2 {
        if pass == 1 {
            learn_mandates(&mut out);
        }
        let waiting: Vec<usize> = (0..out.len()).filter(|&i| expects(&out[i])).collect();
        let facts: Vec<usize> = (0..out.len()).filter(|&j| is_fact(&out[j])).collect();
        let others: Vec<usize> = (0..confirming.len()).filter(|&k| !used[k]).collect();
        let side_of = |m: &Movement, expected: bool| crate::matching::Side {
            way: m.way,
            sum: m.sum,
            currency: m.currency.clone(),
            day: m.day,
            evidence: m.evidence.unwrap_or(Evidence::Transfer),
            accounts: if expected { m.place.budgets().first().map(|b| payers(b)).unwrap_or_default() } else { m.account.clone().into_iter().collect() },
            names: m.names.iter().map(|n| tokens.of(n)).collect(),
            refs: m.refs.clone(),
        };
        let expected: Vec<crate::matching::Side> = waiting.iter().map(|&i| side_of(&out[i], true)).collect();
        let mut sides: Vec<crate::matching::Side> = facts.iter().map(|&j| side_of(&out[j], false)).collect();
        sides.extend(others.iter().map(|&k| {
            let b = &confirming[k];
            crate::matching::Side {
                way: way_of(b.amount),
                sum: Sum::Exact(b.amount),
                currency: "EUR".into(),
                day: b.date,
                evidence: Evidence::Transfer,
                accounts: Vec::new(),
                names: vec![tokens.of(&b.label)],
                refs: crate::matching::label_references(leads, &b.label),
            }
        }));
        let idf = crate::matching::Idf::of(expected.iter().chain(sides.iter()).flat_map(|s| s.names.iter()));
        let allowed = |e: usize, f: usize| {
            let m = &out[waiting[e]];
            match facts.get(f) {
                Some(&j) => !apart_both(m, &out[j]),
                None => {
                    let b = &confirming[others[f - facts.len()]];
                    !apart_from(&bank_key(&b.account, &b.id), m)
                }
            }
        };
        let pairs = crate::matching::assign(&expected, &sides, &idf, &allowed);
        let mut gone: BTreeSet<usize> = BTreeSet::new();
        for pair in pairs {
            let at = waiting[pair.expected];
            let probable = pair.outcome == crate::matching::Outcome::Probable;
            match facts.get(pair.fact) {
                Some(&j) => {
                    let fact = out[j].clone();
                    join(&mut out[at], fact, probable);
                    gone.insert(j);
                }
                None => {
                    let k = others[pair.fact - facts.len()];
                    used[k] = true;
                    let b = &confirming[k];
                    let m = &mut out[at];
                    m.settle(&b.account, b.date, b.amount, false);
                    m.bank_label = Some(b.label.clone());
                    let mut source = Source::new(Kind::Bank, bank_key(&b.account, &b.id), String::new());
                    source.probable = probable;
                    m.sources.push(source);
                    m.anchor();
                }
            }
        }
        let mut index = 0;
        out.retain(|_| {
            let keep = !gone.contains(&index);
            index += 1;
            keep
        });
    }

    // 7. What is probably together: two invoices paid by one transfer, an
    // order charged in parts, a deferred-debit card's sum and the card
    // payments it settles, a purchase's instalments.
    together(&mut out, input, &apart_both, &tokens);

    // 7b. Work money set aside (docs/accounting.md, "Work money"): a share of
    // each income of a budget that sets one aside, one planned movement per
    // declaration, on its day, while that day is to come; then the payment
    // itself counts (Soman & Cheema 2011, research/budgets.md §2.1).
    for budget in &ledger.budgets {
        let Some(share) = budget.set_aside.filter(|s| s.is_finite() && *s > 0.0) else { continue };
        let quarterly = budget.declare.as_deref() != Some("month");
        let mut by_day: std::collections::BTreeMap<Date, i64> = std::collections::BTreeMap::new();
        for m in out.iter().filter(|m| m.way == Way::In && m.state != State::NotSeen) {
            let part: i64 = match &m.place {
                Place::Budget(b) if *b == budget.id => m.sum.amount().cents(),
                Place::Parts(parts) => parts.iter().filter(|(b, _)| *b == budget.id).map(|(_, a)| a.cents()).sum(),
                _ => continue,
            };
            if part > 0 {
                *by_day.entry(declaration_day(m.day, quarterly)).or_default() += (part as f64 * share.min(100.0) / 100.0).round() as i64;
            }
        }
        for (day, cents) in by_day.into_iter().filter(|(day, cents)| *day >= today && *cents > 0) {
            let key = format!("setaside:{}/{day}", budget.id);
            let label = format!("{} %", (share * 100.0).round() / 100.0);
            let mut m = bare(key.clone(), Sum::About(Money(-cents)), day, label, Place::Budget(budget.id.clone()), vec![Source::new(Kind::SetAside, key, String::new())], State::Planned);
            m.account = carrier(&budget.id);
            out.push(m);
        }
    }

    // 7c. A share set aside, paid before the declaration's day and said "the
    // same as" the bank's line that paid it ("The same as…"): one movement, done
    // on the bank's day with its amount, counted once.
    for link in store.links.iter().filter(|l| l.movement.starts_with("setaside:") || l.source.starts_with("setaside:")) {
        let (aside, other) = if link.movement.starts_with("setaside:") { (&link.movement, &link.source) } else { (&link.source, &link.movement) };
        let Some(target) = out.iter().position(|m| m.has_kind(Kind::SetAside) && m.has(aside)) else { continue };
        let Some(own) = out.iter().position(|m| !m.has_kind(Kind::SetAside) && m.has(other)) else { continue };
        if apart_both(&out[own], &out[target]) {
            continue;
        }
        let fact = out.remove(own);
        let target = if own < target { target - 1 } else { target };
        join(&mut out[target], fact, link.how == "probable");
    }

    // 8. Placements by your hand, for what the files cannot place.
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

    // 9. Not seen: its window passed inside what its account's exports cover,
    // with no line. With no account declared, every export tells, as the watch reads them.
    let mut of_account: HashMap<Option<String>, Vec<&bank::Movement>> = HashMap::new();
    for b in &input.bank.movements {
        let card = accounts::account_of(ledger, &b.account).map(|a| a.id.clone());
        if card.is_some() || ledger.bank_accounts.is_empty() {
            of_account.entry(card).or_default().push(b);
        }
    }
    // What each account's exports cover, once.
    let spans: HashMap<Option<String>, (Date, Date)> = of_account.iter().filter_map(|(k, v)| Some((k.clone(), (v.iter().map(|b| b.date).min()?, v.iter().map(|b| b.date).max()?)))).collect();
    for m in out.iter_mut().filter(|m| m.waits && matches!(m.state, State::Planned | State::Announced)) {
        let (start, end) = (shifted(m.day, -EARLIER_DAYS), shifted(m.day, LATER_DAYS));
        if end >= today {
            continue;
        }
        let whose = if ledger.bank_accounts.is_empty() { None } else { m.account.clone() };
        if whose.is_none() && !ledger.bank_accounts.is_empty() {
            continue;
        }
        let (Some(theirs), Some(&(first, last))) = (of_account.get(&whose), spans.get(&whose)) else { continue };
        if first > start || last < end {
            continue;
        }
        let label = bank::words(filler, &m.label);
        // A line plainly its own, placed elsewhere by a rule, is no doubt: it passed.
        let passed = theirs.iter().any(|b| bank::matches(filler, b, &label, m.sum.amount(), m.day));
        if first <= start && last >= end && !passed {
            m.state = State::NotSeen;
        }
    }

    // 10. What each is for: its budgets', else its account's.
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

/// The mandates' references (RUM, ICS) a debit's bank line carried once
/// joined to its preset's occurrence, given to the occurrences still waiting:
/// the next month's line is then that preset's by its key, even when its
/// amount changed.
fn learn_mandates(out: &mut [Movement]) {
    let preset_of = |m: &Movement| m.sources.iter().find(|s| s.kind == Kind::Preset).and_then(|s| s.key.strip_prefix("preset:")).and_then(|k| k.rsplit_once('/')).map(|(p, _)| p.to_string());
    let mut learnt: HashMap<String, Vec<String>> = HashMap::new();
    for m in out.iter().filter(|m| m.has_kind(Kind::Bank)) {
        if let Some(preset) = preset_of(m) {
            learnt.entry(preset).or_default().extend(m.refs.iter().filter(|r| r.starts_with("rum:") || r.starts_with("ics:")).cloned());
        }
    }
    for m in out.iter_mut().filter(|m| expects(m)) {
        if let Some(refs) = preset_of(m).and_then(|p| learnt.get(&p)) {
            for r in refs {
                if !m.refs.contains(r) {
                    m.refs.push(r.clone());
                }
            }
        }
    }
}

/// What is probably together (`Movement::together`), on what the matching
/// left: two or three invoices you issued that one client's transfer pays
/// (one movement); an order a receipt told, charged in two or three parts
/// (one movement, its lines of the bank together); a deferred-debit card's
/// sum, settling the month's card payments (in no budget, beside them); a
/// purchase's instalments a message told (each in its own month).
fn together(out: &mut Vec<Movement>, input: &Input, apart_both: &dyn Fn(&Movement, &Movement) -> bool, tokens: &NameWords) {
    let words = &input.words.bank;
    let names = |m: &Movement| m.names.iter().map(|n| tokens.of(n)).collect::<Vec<_>>();
    let all: Vec<Vec<String>> = out.iter().flat_map(names).collect();
    let idf = crate::matching::Idf::of(all.iter());
    let alike = |a: &Movement, b: &Movement| {
        let (x, y) = (names(a), names(b));
        x.iter().flat_map(|p| y.iter().map(move |q| (p, q))).map(|(p, q)| crate::matching::similarity(p, q, &idf)).fold(0.0, f64::max)
    };
    // Two or three invoices, one client's transfer.
    let mut i = 0;
    while i < out.len() {
        if !(is_fact(&out[i]) && out[i].way == Way::In) {
            i += 1;
            continue;
        }
        let day = out[i].day;
        let candidates: Vec<(usize, Money)> = (0..out.len())
            .filter(|&e| expects(&out[e]) && out[e].evidence == Some(Evidence::Invoice) && out[e].way == Way::In && !apart_both(&out[e], &out[i]))
            .filter(|&e| {
                let (from, until) = Evidence::Invoice.window(out[e].day);
                day >= from && day <= until
            })
            .map(|e| (e, out[e].sum.amount()))
            .collect();
        match crate::matching::adding_up(out[i].sum.amount(), &candidates, 0).filter(|set| set.iter().all(|&e| alike(&out[e], &out[i]) >= 0.3 || out[e].names.iter().all(|n| n.trim().is_empty()))) {
            Some(mut set) => {
                set.sort_unstable();
                let fact = out.remove(i);
                let shift = |at: usize| if at > i { at - 1 } else { at };
                let first = shift(set[0]);
                // The invoices become one movement, the transfer's.
                for &e in set.iter().skip(1).rev() {
                    let other = out.remove(shift(e));
                    merge(&mut out[first], other, true);
                }
                join(&mut out[first], fact, true);
                i = 0;
            }
            None => i += 1,
        }
    }
    // An order in parts: a receipt's amount, two or three card payments to the same payee.
    let mut e = 0;
    while e < out.len() {
        if !(expects(&out[e]) && out[e].evidence == Some(Evidence::Card)) {
            e += 1;
            continue;
        }
        let (from, until) = Evidence::Card.window(out[e].day);
        let candidates: Vec<(usize, Money)> = (0..out.len())
            .filter(|&j| is_fact(&out[j]) && out[j].way == out[e].way && out[j].day >= from && out[j].day <= until && !apart_both(&out[e], &out[j]) && alike(&out[e], &out[j]) >= 0.5)
            .map(|j| (j, out[j].sum.amount()))
            .collect();
        match crate::matching::adding_up(out[e].sum.amount(), &candidates, 100) {
            Some(mut set) => {
                set.sort_unstable();
                let total: Money = set.iter().map(|&j| out[j].sum.amount()).sum();
                let first_day = set.iter().map(|&j| out[j].day).min().unwrap_or(out[e].day);
                let mut facts = Vec::new();
                for &j in set.iter().rev() {
                    facts.push(out.remove(j));
                }
                let e_at = e - set.iter().filter(|&&j| j < e).count();
                for fact in facts.into_iter().rev() {
                    join(&mut out[e_at], fact, true);
                }
                out[e_at].sum = Sum::Exact(total);
                out[e_at].day = first_day;
                e = 0;
            }
            None => e += 1,
        }
    }
    // A deferred-debit card's sum: the card payments of the month before its
    // own, on its account, as such cards are debited at the start of the next month.
    let deferred: Vec<usize> = (0..out.len()).filter(|&d| out[d].way == Way::Out && out[d].has_kind(Kind::Bank) && out[d].names.iter().any(|n| crate::words::holds_word(&words.deferred, &crate::words::folded(n)))).collect();
    for d in deferred {
        let (account, day, total) = (out[d].account.clone(), out[d].day, out[d].sum.amount());
        let month = day.first_of_month().yesterday().unwrap_or(day);
        let (first, last) = (month.first_of_month(), month.last_of_month());
        let settled: Vec<usize> = (0..out.len())
            .filter(|&c| c != d && out[c].way == Way::Out && out[c].has_kind(Kind::Bank) && out[c].account == account && out[c].day >= first && out[c].day <= last && out[c].names.iter().any(|n| card_like(words, n)) && out[c].together.is_empty())
            .collect();
        let sum: Money = settled.iter().map(|&c| out[c].sum.amount()).sum();
        if settled.is_empty() || (sum.cents() - total.cents()).abs() > total.cents().abs() / 100 + 1 {
            continue;
        }
        let keys: Vec<String> = settled.iter().map(|&c| out[c].steady_key().to_string()).collect();
        let own = out[d].steady_key().to_string();
        for &c in &settled {
            out[c].together.push(own.clone());
        }
        let m = &mut out[d];
        m.place = Place::Nowhere;
        m.together = keys;
        for s in m.sources.iter_mut().filter(|s| s.kind == Kind::Bank) {
            s.probable = true;
        }
    }
    // A purchase's instalments: the message joined to the first (its link
    // says how many), the next ones each about its share, a month apart.
    for link in input.store.links.iter().filter(|l| l.instalments.is_some_and(|n| n > 1)) {
        let Some(first) = out.iter().position(|m| m.has(&link.source)) else { continue };
        let n = i64::from(link.instalments.unwrap_or(1));
        let share = link.amount.map_or(out[first].sum.amount(), |total| Money(-total.cents().abs() / n));
        let (day, start) = (out[first].day, out[first].steady_key().to_string());
        let mut found = Vec::new();
        for k in 1..n {
            let (from, until) = (shifted(day, 25 * k), shifted(day, 35 * k + 5));
            let next = (0..out.len()).find(|&j| j != first && !found.contains(&j) && out[j].has_kind(Kind::Bank) && out[j].day >= from && out[j].day <= until && (out[j].sum.amount().cents() - share.cents()).abs() <= share.cents().abs() / 50 + 1 && alike(&out[first], &out[j]) >= 0.5);
            if let Some(j) = next {
                found.push(j);
            }
        }
        let keys: Vec<String> = found.iter().map(|&j| out[j].steady_key().to_string()).collect();
        for &j in &found {
            if !out[j].together.contains(&start) {
                out[j].together.push(start.clone());
            }
        }
        for key in keys.into_iter().chain(link.together.iter().cloned()) {
            if !out[first].together.contains(&key) && key != start {
                out[first].together.push(key);
            }
        }
    }
}

/// The holds still holding: listed by the store, not yet posted (a line of
/// the bank with the same id, or on the same account within two weeks with
/// the same payee, by the first word of its name as the matching reads it,
/// or nearly its amount), and younger than two weeks.
fn live_holds(store: &Store, bank: &Bank, words: &crate::words::BankWords, today: Date) -> Vec<Hold> {
    let payee = |label: &str| crate::matching::tokens(words, label).into_iter().next();
    store
        .holds
        .iter()
        .filter(|h| days_apart(h.date, today) <= HOLD_DAYS || h.date > today)
        .filter(|h| {
            let held = payee(&h.label);
            !bank.movements.iter().any(|b| {
                let same_account = b.account == h.account;
                let posted_later = b.date >= shifted(h.date, -1) && b.date <= shifted(h.date, HOLD_DAYS);
                let near = (b.amount.cents() - h.amount.cents()).abs() <= h.amount.cents().abs() / 5;
                let named = held.is_some() && payee(&b.label) == held;
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
            m.names.extend(other.names);
            m.anchor();
            continue;
        }
        i += 1;
    }
}

/// The bank's line `fact` is the same payment as `into`: its day, amount,
/// account and state win; its placement wins when it is yours or a rule's
/// (`[[assign]]`, `[[split]]`), never the account's first budget alone.
fn join(into: &mut Movement, fact: Movement, probable: bool) {
    if into.day != fact.day {
        into.expected = Some(into.expected.unwrap_or(into.day));
    }
    into.day = fact.day;
    into.sum = fact.sum;
    into.account = fact.account.clone().or(into.account.take());
    into.state = fact.state;
    if matches!(fact.why, Some(accounts::Why::Hand | accounts::Why::Rule)) {
        into.place = fact.place.clone();
    }
    into.why = fact.why;
    into.bank_label = fact.bank_label.clone().or(into.bank_label.take());
    into.sources.extend(fact.sources.into_iter().map(|mut s| {
        s.probable = probable;
        s
    }));
    into.refs.extend(fact.refs);
    into.names.extend(fact.names);
    into.together.extend(fact.together);
    into.anchor();
}

/// `other` is the same payment as `into`: its sources join; the bank's facts win.
fn merge(into: &mut Movement, other: Movement, probable: bool) {
    if other.has_kind(Kind::Bank) || other.has_kind(Kind::Hold) {
        if !into.has_kind(Kind::Bank) {
            join(into, other, probable);
            return;
        }
    } else if other.state == State::Announced && into.state == State::Planned {
        into.state = State::Announced;
    }
    into.sources.extend(other.sources.into_iter().map(|mut s| {
        s.probable = probable;
        s
    }));
    into.links.extend(other.links);
    into.refs.extend(other.refs);
    into.names.extend(other.names);
    into.together.extend(other.together);
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

// ---------------------------------------------------------------- counting

/// The budget file with each money movement counted once, for the budgets
/// to count (`budget::Ledger::status`), in memory; the file is not written.
/// The file's lines and presets count as they always did; then, for each
/// movement a declared account's line tells:
/// - with a line of the file (a receipt, a planned bill, two invoices one
///   transfer paid): those lines count, done now, and what the bank's amount
///   differs from them (a tip) on a line beside them;
/// - else its own line, on its day, in its budget or its parts, standing for
///   the preset whose occurrence it is (the real amount in place of the
///   planned one), or spent from an estimate;
/// - transfers between your accounts, and what is in no budget, count nowhere.
///
/// A movement placed by your hand where the files cannot say it (`[[move]]`)
/// counts where you placed it: its occurrence's planned amount leaves its
/// preset's budget for the one you chose. Lines of accounts not declared keep
/// no books (they only confirm), as before. The lines added carry the bank
/// line's key in their links, so a budget's page lists them without "Delete";
/// the file's own lines keep their places, so their addresses hold.
pub fn counted(ledger: &Ledger, movements: &[Movement], today: Date) -> Ledger {
    let mut out = ledger.clone();
    for (i, preset) in out.presets.iter_mut().enumerate() {
        if preset.id.is_none() {
            preset.id = Some(format!("#{i}"));
        }
    }
    let keys = line_keys(ledger);
    let declared = |key: &str| key.strip_prefix("bank:").or_else(|| key.strip_prefix("hold:")).and_then(|rest| rest.rsplit_once('/')).is_some_and(|(account, _)| ledger.bank_accounts.iter().any(|a| a.id == account));
    let preset_of = |key: &str| -> Option<(usize, Date)> {
        let (preset, day) = key.strip_prefix("preset:")?.rsplit_once('/')?;
        let index = (0..ledger.presets.len()).find(|&i| preset_key(ledger, i) == preset)?;
        Some((index, day.parse().ok()?))
    };
    let line = |budget: &str, day: Date, amount: Money, label: &str, preset: Option<String>, reserve: Option<String>, link: &str, planned: bool| Line {
        id: String::new(),
        budget: budget.to_string(),
        date: day,
        amount,
        label: label.to_string(),
        planned,
        reserve,
        links: vec![link.to_string()],
        preset,
    };
    for m in movements {
        // A share set aside for the declaration: planned in its budget until its
        // day; paid early and joined to its bank line, that line counts, once.
        if let Some(aside) = m.sources.iter().find(|s| s.kind == Kind::SetAside).filter(|_| !m.has_kind(Kind::Bank)) {
            for (budget, amount) in m.place.budgets().into_iter().map(|b| (b.to_string(), m.sum.amount())).take(1) {
                out.lines.push(line(&budget, m.day, amount, &m.label, None, None, &aside.key, m.day > today));
            }
            continue;
        }
        let bank = m.sources.iter().find(|s| matches!(s.kind, Kind::Bank | Kind::Hold) && declared(&s.key)).map(|s| s.key.clone());
        let file: Vec<usize> = m.sources.iter().filter(|s| s.kind == Kind::Line).filter_map(|s| keys.iter().position(|k| *k == s.key)).collect();
        let occurrence = m.sources.iter().find(|s| s.kind == Kind::Preset).and_then(|s| preset_of(&s.key));
        // Where the files place it, and where it is.
        let natural: Vec<String> = if file.is_empty() { occurrence.map(|(i, _)| vec![ledger.presets[i].budget.clone()]).unwrap_or_default() } else { file.iter().map(|&i| ledger.lines[i].budget.clone()).collect() };
        let here: Vec<String> = m.place.budgets().iter().map(|b| b.to_string()).collect();
        let moved = matches!(m.place, Place::Parts(_)) || (!here.is_empty() && here != natural);
        // The occurrence, taken out of its preset's budget by a line of nothing standing for it.
        let absorb = |out: &mut Ledger, link: &str| {
            if let Some((i, day)) = occurrence {
                let preset = &ledger.presets[i];
                out.lines.push(line(&preset.budget, day, Money::ZERO, &preset.label, Some(preset_key(ledger, i)), None, link, false));
            }
        };
        // Where it counts instead: its budget, or each part.
        let shares = |amount: Money| -> Vec<(String, Money)> {
            match &m.place {
                Place::Budget(b) | Place::Reserve { budget: b, .. } => vec![(b.clone(), amount)],
                Place::Parts(parts) => parts.clone(),
                Place::Transfer { .. } | Place::Nowhere | Place::Unplaced => Vec::new(),
            }
        };
        let reserve = match &m.place {
            Place::Reserve { reserve, .. } => Some(reserve.clone()),
            _ => None,
        };
        let Some(link) = bank else {
            // Nothing of the bank: only a placement by your hand changes what the files count.
            if !moved || file.len() > 1 {
                continue;
            }
            let link = format!("move:{}", m.steady_key());
            if let Some(&first) = file.first() {
                let parts = shares(ledger.lines[first].amount);
                if let Some((budget, amount)) = parts.first() {
                    out.lines[first].budget = budget.clone();
                    out.lines[first].amount = *amount;
                }
                for (budget, amount) in parts.iter().skip(1) {
                    out.lines.push(line(budget, ledger.lines[first].date, *amount, &ledger.lines[first].label, None, None, &link, ledger.lines[first].planned));
                }
            } else if let Some((i, day)) = occurrence {
                absorb(&mut out, &link);
                let planned = m.state == State::Planned || m.state == State::Announced;
                for (budget, amount) in shares(ledger.presets[i].amount) {
                    out.lines.push(line(&budget, day, amount, &m.label, None, None, &link, planned && day > today));
                }
            }
            continue;
        };
        if !file.is_empty() {
            // The file's lines tell it, done now; the bank's own amount, where it differs, beside them.
            for &i in &file {
                out.lines[i].planned = false;
            }
            let told: Money = file.iter().map(|&i| ledger.lines[i].amount).sum();
            let rest = m.sum.amount() - told;
            if rest != Money::ZERO {
                let budget = here.first().cloned().unwrap_or_else(|| ledger.lines[file[0]].budget.clone());
                out.lines.push(line(&budget, m.day, rest, &m.label, None, None, &link, false));
            }
            if moved && file.len() == 1 && here.len() == 1 {
                out.lines[file[0]].budget = here[0].clone();
            }
            continue;
        }
        // The bank's line alone counts: standing for its occurrence in that preset's budget, or spent from an estimate.
        let stands = match occurrence {
            Some((i, _)) if !moved => Some(preset_key(ledger, i)),
            Some(_) => {
                absorb(&mut out, &link);
                None
            }
            None => m.stands.clone(),
        };
        for (budget, amount) in shares(m.sum.amount()) {
            out.lines.push(line(&budget, m.day, amount, &m.label, stands.clone(), reserve.clone(), &link, false));
        }
    }
    out
}

/// The budget file of a notes folder with its movements counted once
/// (`counted`), from the first day anything in it tells (its lines, its
/// budgets' starts, the bank's lines) to a year ahead.
pub fn counted_in(root: &Path, ledger: &Ledger, words: &crate::words::Words, today: Date) -> Ledger {
    let bank = Bank::load(root).unwrap_or_default();
    let first = bank.movements.iter().map(|m| m.date).chain(ledger.lines.iter().map(|l| l.date)).chain(ledger.budgets.iter().filter_map(|b| b.since)).min().unwrap_or(today).min(today);
    let (from, to) = (shifted(first.first_of_month(), -31), shifted(today, 400));
    let contracts = crate::contracts::Contracts::load(root).map(|c| c.list).unwrap_or_default();
    let letters = crate::letters::Letters::load(root).map(|l| l.list).unwrap_or_default();
    let store = Store::load(root).unwrap_or_default();
    let input = Input { ledger, bank: &bank, contracts: &contracts, letters: &letters, store: &store, words };
    counted(ledger, &derive(&input, from, to, today), today)
}

/// A notes folder's money, read once (`books`): the budget file as
/// written, the bank's file, every movement derived from the first day
/// anything in them tells to a year ahead (as `counted_in`), and the budget
/// file with each counted once.
#[derive(Debug, Clone)]
pub struct Books {
    pub ledger: Ledger,
    pub counted: Ledger,
    pub bank: Bank,
    pub movements: Vec<Movement>,
    /// The days the movements were derived for.
    pub from: Date,
    pub to: Date,
}

impl Books {
    /// The movements of the days from `from` to `to`, included.
    pub fn between(&self, from: Date, to: Date) -> impl Iterator<Item = &Movement> {
        self.movements.iter().filter(move |m| m.day >= from && m.day <= to)
    }
}

/// What tells whether the books are still those of the files: their
/// modification times and sizes, the day, the words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Stamp {
    root: PathBuf,
    files: Vec<Option<(std::time::SystemTime, u64)>>,
    today: Date,
}

fn stamp(root: &Path, today: Date) -> Stamp {
    let paths = [root.join(crate::budget::LEDGER), root.join(bank::MANIFEST), root.join(crate::contracts::MANIFEST), crate::letters::Letters::folder(root).join(crate::letters::MANIFEST), Store::path(root)];
    let files = paths.iter().map(|p| std::fs::metadata(p).ok().and_then(|m| Some((m.modified().ok()?, m.len())))).collect();
    Stamp { root: root.to_path_buf(), files, today }
}

/// The last books made, by what they were made from.
static BOOKS: std::sync::Mutex<Option<(Stamp, crate::words::Words, std::sync::Arc<Books>)>> = std::sync::Mutex::new(None);

/// The kept copy's format: a copy of another is made anew.
const KEPT: u32 = 1;

/// The movements kept on this device (`$XDG_CACHE_HOME/sioul/money-books.json`),
/// by what they were made from, for a process of its own (`sioul budgets`,
/// the window's first look) to read rather than derive: never shared, made
/// anew whenever a file, the day, the words or Sioul's version changed.
#[derive(Serialize, Deserialize)]
struct Kept {
    format: u32,
    version: String,
    stamp: Stamp,
    words: u64,
    movements: Vec<Movement>,
}

fn kept_path() -> PathBuf {
    crate::config::cache_dir().join("money-books.json")
}

/// The words as a digest: the kept movements were made with these.
fn words_digest(words: &crate::words::Words) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(words).unwrap_or_default().hash(&mut hasher);
    hasher.finish()
}

/// The kept movements, when they were made from what the files are now.
fn kept(stamp: &Stamp, words: u64) -> Option<Vec<Movement>> {
    let kept: Kept = serde_json::from_slice(&std::fs::read(kept_path()).ok()?).ok()?;
    (kept.format == KEPT && kept.version == env!("CARGO_PKG_VERSION") && kept.stamp == *stamp && kept.words == words).then_some(kept.movements)
}

/// The movements kept for the next process; nothing said when it cannot be.
fn keep(stamp: &Stamp, words: u64, movements: &[Movement]) {
    let kept = Kept { format: KEPT, version: env!("CARGO_PKG_VERSION").to_string(), stamp: stamp.clone(), words, movements: movements.to_vec() };
    let path = kept_path();
    if let (Some(dir), Ok(text)) = (path.parent(), serde_json::to_vec(&kept)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = crate::filelock::replace(&path, &text);
    }
}

/// The books forgotten in this process (a test's way to read the kept copy).
#[cfg(test)]
pub(crate) fn forget_books() {
    if let Ok(mut kept) = BOOKS.lock() {
        *kept = None;
    }
}

/// A notes folder's money (`Books`), made again only when one of its files
/// changed (by their modification times and sizes), or the day, or the
/// words: the budgets page, the reminders, the dates and the links read it
/// each time, and five years of a busy account take longer than a page
/// should wait to derive (docs/accounting.md, "Counted, not copied").
pub fn books(root: &Path, words: &crate::words::Words, today: Date) -> std::sync::Arc<Books> {
    let now = stamp(root, today);
    if let Ok(kept) = BOOKS.lock()
        && let Some((was, with, books)) = kept.as_ref()
        && *was == now
        && with == words
    {
        return books.clone();
    }
    let ledger = Ledger::load(root).unwrap_or_default();
    let bank = Bank::load(root).unwrap_or_default();
    let first = bank.movements.iter().map(|m| m.date).chain(ledger.lines.iter().map(|l| l.date)).chain(ledger.budgets.iter().filter_map(|b| b.since)).min().unwrap_or(today).min(today);
    let (from, to) = (shifted(first.first_of_month(), -31), shifted(today, 400));
    // Made by another process from the same files: read, not derived again.
    let digest = words_digest(words);
    let movements = match kept(&now, digest) {
        Some(movements) => movements,
        None => {
            let contracts = crate::contracts::Contracts::load(root).map(|c| c.list).unwrap_or_default();
            let letters = crate::letters::Letters::load(root).map(|l| l.list).unwrap_or_default();
            let store = Store::load(root).unwrap_or_default();
            let movements = derive(&Input { ledger: &ledger, bank: &bank, contracts: &contracts, letters: &letters, store: &store, words }, from, to, today);
            if stamp(root, today) == now {
                keep(&now, digest, &movements);
            }
            movements
        }
    };
    let counted = counted(&ledger, &movements, today);
    let books = std::sync::Arc::new(Books { ledger, counted, bank, movements, from, to });
    // Read again before it is kept: a file changed while it was read makes
    // books that are not quite its own, made anew next time.
    if stamp(root, today) == now
        && let Ok(mut kept) = BOOKS.lock()
    {
        *kept = Some((now, words.clone(), books.clone()));
    }
    books
}

#[path = "movements_acts.rs"]
pub mod acts;

#[cfg(test)]
#[path = "movements_tests.rs"]
mod tests;

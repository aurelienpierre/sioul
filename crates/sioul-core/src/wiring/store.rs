// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The rules' list (docs/rules.md, "The store"): one of Sioul's own lists,
//! `rules.toml` in the data folder, read, changed and written under its
//! lock (`Rules::change`), travelling sealed with the settings once every
//! device says it knows it (sioul-sync's `share::RULES`).
//!
//! ```toml
//! version = 1
//!
//! [[rule]]
//! id = "0b6c…"
//! on = true
//! since = 1795435200
//! when.type = "message"
//! when.event = "arrives"
//! when.if = [
//!     { field = "sender-domain", test = "is", value = ["impots.example"] },
//! ]
//! then.type = "task"
//! then.action = "make"
//! then.with = { model = "pay-a-bill", title = { from = "subject" } }
//!
//! [[model]]
//! id = "pay-a-bill"
//! name = "Pay a bill"
//! prefix = "Pay:"
//! business_days = 3
//! ```
//!
//! - **A rule** (`Rule`): its id, on or off, the moment it acts from
//!   (`since`: a rule acts on what comes from then on, never on what was
//!   there before it), one "when" (a type, an event, conditions on its
//!   fields, all of them or one), one "then" (a type, an action, its
//!   parameters, each a fixed value or `{ from = "<field>" }` of the object
//!   that triggered it).
//! - **A model** (`Model`): a template of task any rule names, general; a
//!   project's own models (`[[project.model]]`) are read as models too
//!   (`models`), by `"<project>/<model>"`, edited in their project.
//! - **What this Sioul does not know** (a newer one wrote it: a type, an
//!   event, a field, a test, a value of a closed list, an action, a
//!   parameter, a key): the rule is kept as written, byte for byte, and
//!   never run here (`Rule::unknown`); it may be turned on or off (its `on`
//!   alone written into its table) or removed, never edited. A rule this
//!   Sioul cannot read at all is kept the same way. A model likewise.
//! - **A newer format** (`version` above `FORMAT`): the list is shown, and
//!   nothing in it runs or is written here (`Rules::read_only`).
//!
//! Every other rule and model, and every other line of the file, stays as
//! it was when one changes: only the changed entry's table is written anew.

use crate::supplies::Use;
use crate::wiring::declare::{self, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

/// The file, in the data folder.
pub const FILE: &str = "rules.toml";

/// The format this Sioul reads and writes.
pub const FORMAT: u32 = 1;

/// The rules' list on this device: `$XDG_DATA_HOME/sioul/rules.toml`.
pub fn path() -> PathBuf {
    crate::config::data_dir().join(FILE)
}

/// Every condition, or one of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Join {
    #[default]
    All,
    Any,
}

impl Join {
    pub fn is_all(&self) -> bool {
        *self == Join::All
    }
}

/// One condition on a field of the object that triggered the rule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub test: String,
    /// Its values: any of them holds ("is impots.example or dgfip.example");
    /// for a test that turns it around ("does not contain"), none of them.
    #[serde(default, deserialize_with = "one_or_many", skip_serializing_if = "Vec::is_empty")]
    pub value: Vec<String>,
    /// The end of a range (`between`): a day or a time.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub until: String,
}

/// What happens, and to what.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct When {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub event: String,
    #[serde(rename = "match", default, skip_serializing_if = "Join::is_all")]
    pub join: Join,
    #[serde(rename = "if", default)]
    pub conditions: Vec<Condition>,
}

/// A parameter's value: taken from the object that triggered the rule, or fixed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Param {
    /// `{ from = "subject" }`: "its subject".
    From { from: String },
    Flag(bool),
    Number(i64),
    Text(String),
}

impl Param {
    /// Its fixed text; none when it is taken from the object.
    pub fn text(&self) -> Option<String> {
        match self {
            Param::Text(text) => Some(text.trim().to_string()),
            Param::Number(n) => Some(n.to_string()),
            Param::Flag(b) => Some(b.to_string()),
            Param::From { .. } => None,
        }
    }

    /// The field it is taken from.
    pub fn from(&self) -> Option<&str> {
        match self {
            Param::From { from } => Some(from),
            _ => None,
        }
    }
}

/// What is done, and to what.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Then {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub action: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub with: BTreeMap<String, Param>,
}

/// Where an entry came from in its file, and what it said then (the store's
/// own: `..Rule::default()` fills it).
#[derive(Clone, Default)]
pub struct Kept {
    /// Its place in the file's list as read.
    at: Option<usize>,
    /// What it said (`print`), to know whether it changed since.
    print: String,
    /// Its `on` and `since` as read: written into its table only when they changed.
    on: Option<bool>,
    since: Option<i64>,
}

impl std::fmt::Debug for Kept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Kept({:?})", self.at)
    }
}

/// One rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub id: String,
    #[serde(default = "yes")]
    pub on: bool,
    /// Unix seconds: it acts on what comes from then on (set when made, and
    /// when turned on again).
    #[serde(default)]
    pub since: i64,
    #[serde(default)]
    pub when: When,
    #[serde(default)]
    pub then: Then,
    /// The preset it was made from (`declare::presets`), by its id; "" none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preset: String,
    /// What this Sioul does not know in it, said for a reader ("when.event =
    /// is-forwarded"): kept as written, never run, never edited here.
    #[serde(default, skip_deserializing)]
    pub unknown: Vec<String>,
    #[serde(skip)]
    #[doc(hidden)]
    pub kept: Kept,
}

fn yes() -> bool {
    true
}

impl PartialEq for Rule {
    fn eq(&self, other: &Rule) -> bool {
        (&self.id, self.on, self.since, &self.when, &self.then, &self.preset, &self.unknown) == (&other.id, other.on, other.since, &other.when, &other.then, &other.preset, &other.unknown)
    }
}

/// A model of task: what a task a rule makes is.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// The task list, "account/id"; "" for the usual one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub list: String,
    /// Put in front of each task's title ("Pay:").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// The date asked: so many business days after the object came.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_days: Option<u32>,
    /// The first estimate, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supplies: Vec<Use>,
    /// The project its tasks go in, unless the rule says another; "" none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    /// What this Sioul does not know in it: kept as written, never edited here.
    #[serde(default, skip_deserializing, skip_serializing_if = "Vec::is_empty")]
    pub unknown: Vec<String>,
    #[serde(skip)]
    #[doc(hidden)]
    pub kept: Kept,
}

impl PartialEq for Model {
    fn eq(&self, other: &Model) -> bool {
        self.print() == other.print() && self.unknown == other.unknown
    }
}

impl Model {
    /// As the projects' models are, for what makes tasks (`routetasks`).
    pub fn as_project_model(&self) -> crate::projects::Model {
        crate::projects::Model {
            id: self.id.clone(),
            name: self.name.clone(),
            list: self.list.clone(),
            prefix: self.prefix.clone(),
            business_days: self.business_days,
            estimate: self.estimate.filter(|m| *m > 0),
            supplies: self.supplies.clone(),
        }
    }

    fn print(&self) -> String {
        serde_json::to_string(&(&self.id, &self.name, &self.list, &self.prefix, self.business_days, self.estimate, &self.supplies, &self.project)).unwrap_or_default()
    }
}

/// A model as a rule names it: a general one by its id, a project's by
/// `"<project>/<model>"` (edited in its project).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelRef {
    pub id: String,
    pub model: Model,
    /// The project it belongs to, for a project's own model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Edited here (a general model); a project's, in its project.
    pub editable: bool,
}

/// The general models, then each project's, by `"<project>/<model>"`.
pub fn models(rules: &Rules, projects: Option<&crate::projects::ProjectStore>) -> Vec<ModelRef> {
    let mut out: Vec<ModelRef> = rules.models.iter().map(|m| ModelRef { id: m.id.clone(), model: m.clone(), project: None, editable: m.unknown.is_empty() }).collect();
    for project in projects.map(|s| s.projects.as_slice()).unwrap_or_default() {
        for m in &project.models {
            let model = Model { id: m.id.clone(), name: m.name.clone(), list: m.list.clone(), prefix: m.prefix.clone(), business_days: m.business_days, estimate: m.estimate, supplies: m.supplies.clone(), project: project.id.clone(), ..Model::default() };
            out.push(ModelRef { id: format!("{}/{}", project.id, m.id), model, project: Some(project.id.clone()), editable: false });
        }
    }
    out
}

/// A model by the name a rule gives it.
pub fn model<'a>(all: &'a [ModelRef], id: &str) -> Option<&'a ModelRef> {
    all.iter().find(|m| m.id == id.trim())
}

/// The rules' list, as its file says it.
#[derive(Debug, Clone)]
pub struct Rules {
    pub path: PathBuf,
    /// The format the file says; a newer one: read-only here.
    pub version: u32,
    pub rules: Vec<Rule>,
    pub models: Vec<Model>,
    doc: DocumentMut,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules { path: path(), version: FORMAT, rules: Vec::new(), models: Vec::new(), doc: DocumentMut::new() }
    }
}

/// The words a new file starts with.
const HEADER: &str = "# Your rules: when this happens, do that (docs/rules.md).\n\n";

impl Rules {
    /// The list at `path`; none there, no rules. One that does not read is
    /// an error, and is never written over.
    pub fn load(path: &Path) -> Result<Rules, String> {
        let text = match crate::files::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        Rules::parse(path, &text)
    }

    /// The list a file's text says.
    pub fn parse(path: &Path, text: &str) -> Result<Rules, String> {
        let fail = |e: String| format!("{}: {e}", path.display());
        let doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let version = match doc.get("version") {
            None => FORMAT,
            Some(item) => item.as_integer().and_then(|v| u32::try_from(v).ok()).unwrap_or(u32::MAX),
        };
        let list = |key: &str| -> Result<Vec<Table>, String> {
            match doc.get(key) {
                None => Ok(Vec::new()),
                Some(Item::ArrayOfTables(list)) => Ok(list.iter().cloned().collect()),
                Some(_) => Err(fail(format!("{key} is not a list of tables"))),
            }
        };
        let rules = list("rule")?.iter().enumerate().map(|(at, table)| read_rule(table, at)).collect();
        let models = list("model")?.iter().enumerate().map(|(at, table)| read_model(table, at)).collect();
        Ok(Rules { path: path.to_path_buf(), version, rules, models, doc })
    }

    /// Read, changed by `f` and written back, under the file's lock, which
    /// the sharing takes too: a change is made over the file as it is then.
    /// Written only when something changed; every view of rules told
    /// (`changes::Kind::Rules`). Refused when the file is of a newer format.
    pub fn change<R>(path: &Path, f: impl FnOnce(&mut Rules) -> R) -> Result<R, String> {
        crate::filelock::with_lock(path, || {
            let mut rules = Rules::load(path)?;
            if rules.read_only() {
                return Err("rules-newer-format".to_string());
            }
            let (before, models) = (rules.rules.clone(), rules.models.clone());
            let out = f(&mut rules);
            if rules.rules != before || rules.models != models {
                rules.write()?;
                crate::changes::said(crate::changes::Kind::Rules);
            }
            Ok(out)
        })
    }

    /// A newer Sioul's format: shown, nothing runs nor is written here.
    pub fn read_only(&self) -> bool {
        self.version > FORMAT
    }

    pub fn get(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// A rule added (a new id when it has none) or put in the place of the
    /// one of its id; its id. A rule this Sioul does not know is never
    /// changed here; one that cannot run is kept off (`Rule::problem`).
    /// Made or turned on now: it acts from `now` (Unix seconds).
    pub fn put(&mut self, mut rule: Rule, now: i64) -> Result<String, String> {
        // The readers' and the presets' rules are never stored as yours: theirs run in their own modules.
        if rule.id.starts_with("read:") || rule.id.starts_with("preset:") {
            return Err("rules-problem-reserved".to_string());
        }
        rule.unknown = unknown_in(&rule);
        if !rule.unknown.is_empty() {
            return Err("rules-problem-unknown".to_string());
        }
        if rule.on && rule.problem().is_some() {
            rule.on = false;
        }
        if let Some(place) = self.rules.iter_mut().find(|r| !rule.id.is_empty() && r.id == rule.id) {
            if !place.unknown.is_empty() {
                return Err("rules-problem-unknown".to_string());
            }
            // Turned on now, or changed while on: it acts from now on.
            rule.since = if rule.on && (!place.on || place.when != rule.when || place.then != rule.then) { now } else { place.since };
            rule.kept = place.kept.clone();
            let id = rule.id.clone();
            *place = rule;
            return Ok(id);
        }
        if rule.id.is_empty() {
            rule.id = crate::ids::new();
        }
        rule.since = now;
        rule.kept = Kept::default();
        let id = rule.id.clone();
        self.rules.push(rule);
        Ok(id)
    }

    /// Taken out; whether it was there.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != id);
        before != self.rules.len()
    }

    /// Turned on or off, kept either way; turned on, it acts from `now`. A
    /// rule that cannot run stays off. Whether it changed.
    pub fn set_on(&mut self, id: &str, on: bool, now: i64) -> bool {
        let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) else { return false };
        if rule.on == on || (on && rule.problem().is_some() && rule.unknown.is_empty()) {
            return false;
        }
        rule.on = on;
        if on {
            rule.since = now;
        }
        true
    }

    /// A general model added or changed (by id; a new one gets an id from
    /// its name); its id. One this Sioul does not know is never changed.
    pub fn put_model(&mut self, mut model: Model) -> Result<String, String> {
        model.unknown = Vec::new();
        if model.id.trim().is_empty() {
            let taken: Vec<String> = self.models.iter().map(|m| m.id.clone()).collect();
            model.id = crate::projects::new_id(&model.name, &taken);
        }
        model.supplies = crate::supplies::merged(model.supplies.into_iter().filter(|u| crate::supplies::is_id(&u.id)).collect());
        let id = model.id.clone();
        match self.models.iter_mut().find(|m| m.id == id) {
            Some(place) if !place.unknown.is_empty() => return Err("rules-problem-unknown".to_string()),
            Some(place) => {
                model.kept = place.kept.clone();
                *place = model;
            }
            None => self.models.push(model),
        }
        Ok(id)
    }

    pub fn remove_model(&mut self, id: &str) -> bool {
        let before = self.models.len();
        self.models.retain(|m| m.id != id);
        before != self.models.len()
    }

    /// The rules that run here now (`Rule::runs`); none when the list is of a newer format.
    pub fn running(&self) -> impl Iterator<Item = &Rule> {
        let read_only = self.read_only();
        self.rules.iter().filter(move |r| !read_only && r.runs())
    }

    /// Written whole, next to its place then moved: each entry as it was
    /// read while it did not change (its `on` and `since` alone written into
    /// its table when they did), the others' tables made anew.
    fn write(&mut self) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", self.path.display());
        let mut doc = if self.doc.as_table().is_empty() { format!("{HEADER}version = {FORMAT}\n").parse::<DocumentMut>().unwrap_or_default() } else { self.doc.clone() };
        if doc.get("version").is_none() {
            doc["version"] = toml_edit::value(i64::from(FORMAT));
        }
        save_list(&mut doc, "rule", &self.rules, |rule, table| {
            match table {
                Some(table) if !rule.changed() => {
                    let mut table = table.clone();
                    if rule.kept.on != Some(rule.on) {
                        patch(&mut table, "on", toml_edit::value(rule.on));
                    }
                    if rule.kept.since != Some(rule.since) {
                        patch(&mut table, "since", toml_edit::value(rule.since));
                    }
                    table
                }
                Some(was) => {
                    let mut table = rule_table(rule);
                    *table.decor_mut() = was.decor().clone();
                    table
                }
                None => rule_table(rule),
            }
        }, |rule| rule.kept.at);
        save_list(&mut doc, "model", &self.models, |model, table| match table {
            Some(table) if !model.unknown.is_empty() || model.print() == model.kept.print => table.clone(),
            Some(was) => {
                let mut table = model_table(model);
                *table.decor_mut() = was.decor().clone();
                table
            }
            None => model_table(model),
        }, |model| model.kept.at);
        if let Some(parent) = self.path.parent() {
            crate::files::create_dir_all(parent).map_err(fail)?;
        }
        let text = doc.to_string();
        // What this Sioul wrote, it reads again: never a file it cannot read.
        Rules::parse(&self.path, &text)?;
        let temporary = self.path.with_extension("toml.new");
        crate::files::write(&temporary, &text).and_then(|()| crate::files::rename(&temporary, &self.path)).map_err(fail)?;
        *self = Rules::parse(&self.path, &text)?;
        Ok(())
    }
}

/// A key of a table set to `value` when it says something else; its place
/// and its decor kept.
fn patch(table: &mut Table, key: &str, value: Item) {
    let same = table.get(key).and_then(Item::as_value).map(|v| v.to_string().trim().to_string()) == value.as_value().map(|v| v.to_string().trim().to_string());
    if same {
        return;
    }
    match table.get_mut(key).and_then(Item::as_value_mut) {
        Some(place) => {
            let decor = place.decor().clone();
            if let Some(mut new) = value.into_value().ok() {
                *new.decor_mut() = decor;
                *place = new;
            }
        }
        None => {
            table.insert(key, value);
        }
    }
}

/// The list `key` of `doc` made of `entries`, each table from `table` (given
/// its table as read, when it has one, by `at`).
fn save_list<E>(doc: &mut DocumentMut, key: &str, entries: &[E], table: impl Fn(&E, Option<&Table>) -> Table, at: impl Fn(&E) -> Option<usize>) {
    let old: Vec<Table> = match doc.get(key) {
        Some(Item::ArrayOfTables(list)) => list.iter().cloned().collect(),
        _ => Vec::new(),
    };
    let mut list = ArrayOfTables::new();
    for entry in entries {
        list.push(table(entry, at(entry).and_then(|i| old.get(i))));
    }
    if list.is_empty() {
        doc.remove(key);
    } else {
        doc.insert(key, Item::ArrayOfTables(list));
    }
}

/// A rule's table, as this Sioul writes it: its "when" and "then" as dotted
/// keys, its conditions one per line.
fn rule_table(rule: &Rule) -> Table {
    use toml_edit::{Array, InlineTable, value};
    let mut table = Table::new();
    table["id"] = value(&rule.id);
    table["on"] = value(rule.on);
    table["since"] = value(rule.since);
    if !rule.preset.is_empty() {
        table["preset"] = value(&rule.preset);
    }
    let mut when = Table::new();
    when.set_dotted(true);
    when["type"] = value(&rule.when.kind);
    when["event"] = value(&rule.when.event);
    if rule.when.join == Join::Any {
        when["match"] = value("any");
    }
    let mut conditions = Array::new();
    for c in &rule.when.conditions {
        let mut inline = InlineTable::new();
        inline.insert("field", c.field.as_str().into());
        inline.insert("test", c.test.as_str().into());
        if !c.value.is_empty() {
            inline.insert("value", toml_edit::Value::Array(c.value.iter().map(String::as_str).collect()));
        }
        if !c.until.is_empty() {
            inline.insert("until", c.until.as_str().into());
        }
        inline.fmt();
        let mut item = toml_edit::Value::InlineTable(inline);
        item.decor_mut().set_prefix("\n    ");
        conditions.push_formatted(item);
    }
    if !conditions.is_empty() {
        conditions.set_trailing_comma(true);
        conditions.set_trailing("\n");
    }
    when["if"] = toml_edit::Item::Value(toml_edit::Value::Array(conditions));
    table["when"] = Item::Table(when);
    let mut then = Table::new();
    then.set_dotted(true);
    then["type"] = value(&rule.then.kind);
    then["action"] = value(&rule.then.action);
    if !rule.then.with.is_empty() {
        let mut with = InlineTable::new();
        for (key, param) in &rule.then.with {
            let v: toml_edit::Value = match param {
                Param::From { from } => {
                    let mut inline = InlineTable::new();
                    inline.insert("from", from.as_str().into());
                    toml_edit::Value::InlineTable(inline)
                }
                Param::Flag(b) => (*b).into(),
                Param::Number(n) => (*n).into(),
                Param::Text(t) => t.as_str().into(),
            };
            with.insert(key, v);
        }
        with.fmt();
        then["with"] = toml_edit::Item::Value(toml_edit::Value::InlineTable(with));
    }
    table["then"] = Item::Table(then);
    table
}

/// A model's table, its usual values left out.
fn model_table(model: &Model) -> Table {
    let text = toml::to_string(&Model { unknown: Vec::new(), ..model.clone() }).unwrap_or_default();
    let written: DocumentMut = text.parse().unwrap_or_default();
    let mut table = Table::new();
    for (key, item) in written.iter() {
        table.insert(key, item.clone());
    }
    table
}

/// A table as plain TOML values, nested tables and all.
fn plain(table: &Table) -> Option<toml::Table> {
    let mut doc = DocumentMut::new();
    let mut alone = table.clone();
    alone.set_implicit(false);
    alone.set_position(None);
    doc.insert("x", Item::Table(alone));
    let read: toml::Table = toml::from_str(&doc.to_string()).ok()?;
    read.get("x")?.as_table().cloned()
}

/// Why a typed value cannot be read as what its test asks, a message id;
/// none when it reads.
fn shape(value: Value, text: &str) -> Option<&'static str> {
    let fine = match value {
        Value::Domains => !text.contains('@') && text.contains('.') && !text.contains(char::is_whitespace),
        Value::Addresses => text.contains('@') && !text.contains(char::is_whitespace),
        Value::Time => crate::rules::time(text).is_some(),
        Value::Date => crate::rules::day(text).is_some(),
        Value::Size => crate::rules::size(text).is_some(),
        _ => true,
    };
    (!fine).then_some(match value {
        Value::Domains => "rules-problem-domain",
        Value::Addresses => "rules-problem-address",
        Value::Time => "rules-problem-time",
        Value::Date => "rules-problem-date",
        _ => "rules-problem-size",
    })
}

/// What a rule as JSON or TOML says, without its place: to know whether it changed.
fn print(rule: &Rule) -> String {
    serde_json::to_string(&(&rule.id, &rule.when, &rule.then, &rule.preset)).unwrap_or_default()
}

impl Rule {
    /// Changed since it was read (or new): its table is written anew.
    fn changed(&self) -> bool {
        self.unknown.is_empty() && (self.kept.at.is_none() || print(self) != self.kept.print)
    }

    /// Whether it runs here: on, every part of it known and available in
    /// this Sioul's phase, nothing missing.
    pub fn runs(&self) -> bool {
        self.on && self.problem().is_none()
    }

    /// What keeps it from running, as a message id ("rules-problem-…"); none when it can.
    pub fn problem(&self) -> Option<&'static str> {
        if !self.unknown.is_empty() {
            return Some("rules-problem-unknown");
        }
        let Some(kind) = declare::of(&self.when.kind) else { return Some("rules-problem-no-when") };
        let Some(event) = kind.event(&self.when.event) else { return Some("rules-problem-no-when") };
        if !declare::available(event.phase) {
            return Some("rules-problem-coming");
        }
        if self.when.conditions.is_empty() {
            return Some("rules-problem-no-condition");
        }
        for c in &self.when.conditions {
            let Some(field) = kind.field(&c.field) else { return Some("rules-problem-unknown") };
            let Some(test) = field.test(&c.test) else { return Some("rules-problem-unknown") };
            if !declare::available(field.phase) {
                return Some("rules-problem-coming");
            }
            if test.value != Value::None && c.value.iter().all(|v| v.trim().is_empty()) {
                return Some("rules-problem-value");
            }
            if c.test == "between" && c.until.trim().is_empty() {
                return Some("rules-problem-value");
            }
            // A value of the wrong shape never matches: an address under a domain, "18" for a time.
            let typed = c.value.iter().chain((c.test == "between").then_some(&c.until)).map(|v| v.trim()).filter(|v| !v.is_empty());
            if let Some(why) = typed.filter_map(|v| shape(test.value, v)).next() {
                return Some(why);
            }
            if test.value == Value::Choice && c.value.iter().any(|v| field.choice(v.trim()).is_some_and(|choice| !declare::available(choice.phase))) {
                return Some("rules-problem-coming");
            }
        }
        let Some(target) = declare::of(&self.then.kind) else { return Some("rules-problem-no-action") };
        let Some(action) = target.action(&self.then.action) else { return Some("rules-problem-no-action") };
        if declare::forbidden(action.writes).is_some() {
            return Some("rules-problem-forbidden");
        }
        if !declare::available(action.phase) {
            return Some("rules-problem-coming");
        }
        // Declining a call is each phone's screening's, before it rings: never a rule's.
        if !declare::by_engine(&self.then.kind, &self.then.action) {
            return Some("rules-problem-screening");
        }
        // A call's task after a message, a task from a model after a call: nothing to act on.
        if !declare::follows(&self.when.kind, &self.when.event, &self.then.kind, &self.then.action) {
            return Some("rules-problem-pair");
        }
        for param in action.params.iter().filter(|p| p.required) {
            let given = self.then.with.get(param.id).is_some_and(|p| p.from().is_some() || p.text().is_some_and(|t| !t.is_empty()));
            if !given {
                return Some("rules-problem-param");
            }
        }
        None
    }
}

/// What a rule names that the declarations do not know, said for a reader.
fn unknown_in(rule: &Rule) -> Vec<String> {
    let mut out = Vec::new();
    match declare::of(&rule.when.kind) {
        None if !rule.when.kind.is_empty() => out.push(format!("when.type = {}", rule.when.kind)),
        None => {}
        Some(kind) => {
            if !rule.when.event.is_empty() && kind.event(&rule.when.event).is_none() {
                out.push(format!("when.event = {}", rule.when.event));
            }
            for c in &rule.when.conditions {
                match kind.field(&c.field) {
                    None => out.push(format!("when.if.field = {}", c.field)),
                    Some(field) => match field.test(&c.test) {
                        None => out.push(format!("when.if.test = {}", c.test)),
                        Some(test) if test.value == Value::Choice => {
                            for v in c.value.iter().filter(|v| field.choice(v.trim()).is_none()) {
                                out.push(format!("when.if.value = {v}"));
                            }
                        }
                        Some(_) => {}
                    },
                }
            }
        }
    }
    match declare::of(&rule.then.kind) {
        None if !rule.then.kind.is_empty() => out.push(format!("then.type = {}", rule.then.kind)),
        None => {}
        Some(kind) => match kind.action(&rule.then.action) {
            None if !rule.then.action.is_empty() => out.push(format!("then.action = {}", rule.then.action)),
            None => {}
            Some(action) => {
                for (key, param) in &rule.then.with {
                    match action.param(key) {
                        None => out.push(format!("then.with.{key}")),
                        Some(decl) => {
                            if let Some(from) = param.from()
                                && !decl.from.contains(&from)
                            {
                                out.push(format!("then.with.{key}.from = {from}"));
                            }
                        }
                    }
                }
            }
        },
    }
    out
}

/// The keys a rule's parts take.
const RULE_KEYS: &[&str] = &["id", "on", "since", "preset", "when", "then"];
const WHEN_KEYS: &[&str] = &["type", "event", "match", "if"];
const CONDITION_KEYS: &[&str] = &["field", "test", "value", "until"];
const THEN_KEYS: &[&str] = &["type", "action", "with"];
const MODEL_KEYS: &[&str] = &["id", "name", "list", "prefix", "business_days", "estimate", "supplies", "project"];

/// A rule as its table says it, and what it holds that this Sioul does not
/// know; one that does not read at all, kept as such (its id when it says one).
fn read_rule(table: &Table, at: usize) -> Rule {
    let raw = plain(table);
    let id = raw.as_ref().and_then(|t| t.get("id")).and_then(toml::Value::as_str).unwrap_or_default().to_string();
    let parsed = raw.clone().and_then(|t| toml::Value::Table(t).try_into::<Rule>().ok());
    let on = raw.as_ref().and_then(|t| t.get("on")).and_then(toml::Value::as_bool);
    let since = raw.as_ref().and_then(|t| t.get("since")).and_then(toml::Value::as_integer);
    let Some(mut rule) = parsed else {
        return Rule { id, on: on.unwrap_or(true), since: since.unwrap_or(0), unknown: vec!["unreadable".to_string()], kept: Kept { at: Some(at), print: String::new(), on, since }, ..Rule::default() };
    };
    let mut unknown = Vec::new();
    if let Some(raw) = &raw {
        let extra = |table: Option<&toml::Value>, keys: &[&str], prefix: &str, out: &mut Vec<String>| {
            for key in table.and_then(toml::Value::as_table).map(|t| t.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
                if !keys.contains(&key.as_str()) {
                    out.push(format!("{prefix}{key}"));
                }
            }
        };
        extra(Some(&toml::Value::Table(raw.clone())), RULE_KEYS, "", &mut unknown);
        extra(raw.get("when"), WHEN_KEYS, "when.", &mut unknown);
        extra(raw.get("then"), THEN_KEYS, "then.", &mut unknown);
        for condition in raw.get("when").and_then(|w| w.get("if")).and_then(toml::Value::as_array).into_iter().flatten() {
            extra(Some(condition), CONDITION_KEYS, "when.if.", &mut unknown);
        }
    }
    unknown.extend(unknown_in(&rule));
    rule.unknown = unknown;
    rule.kept = Kept { at: Some(at), print: print(&rule), on, since };
    rule
}

/// A model as its table says it; one with what this Sioul does not know, kept as such.
fn read_model(table: &Table, at: usize) -> Model {
    let raw = plain(table);
    let parsed = raw.clone().and_then(|t| toml::Value::Table(t).try_into::<Model>().ok());
    let mut model = parsed.unwrap_or_else(|| Model { id: raw.as_ref().and_then(|t| t.get("id")).and_then(toml::Value::as_str).unwrap_or_default().to_string(), unknown: vec!["unreadable".to_string()], ..Model::default() });
    if let Some(raw) = &raw {
        model.unknown.extend(raw.keys().filter(|k| !MODEL_KEYS.contains(&k.as_str())).cloned());
    }
    model.kept = Kept { at: Some(at), print: model.print(), on: None, since: None };
    model
}

/// One value or several, read as several.
fn one_or_many<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Said {
        One(String),
        Many(Vec<String>),
    }
    Ok(match Said::deserialize(deserializer)? {
        Said::One(one) => vec![one],
        Said::Many(many) => many,
    })
}

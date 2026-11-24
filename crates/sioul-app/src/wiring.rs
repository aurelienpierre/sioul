// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Settings ▸ Rules in the window (docs/rules.md, "The view"; the rules
//! themselves are `sioul_core::wiring`).
//!
//! - `view`: the one list, grouped by what happens, the rules that share a
//!   "when" together; your rules and, read-only, what exists today in its
//!   own place (projects' routes and the tasks they make, budgets' mail
//!   rules, mail filters, topics' senders: `wiring::readers`), each with
//!   where it is kept and how to open its editor there; each of yours with
//!   what it did lately (`wiring::acts`); the presets, the rules that come
//!   with Sioul. Every sentence is `wiring::words`'s.
//! - The editor (`RuleEditor.qml`): the declarations (`catalogue`), what
//!   values choose among (`choices`), the rule in words as it is changed
//!   (`said`), what it would have done in the last 30 days, found off the
//!   window's thread (`preview`), saving through the store (`save`), "also
//!   for the last 30 days" then, once each, off the window's thread.
//! - A rule turned on or off (`set_on`), removed with Undo in the status
//!   line (`remove`); an act undone, or "not for this one" (`undo_act`,
//!   `not_for_this_one`).
//! - A rule from an example (`from_message`), from a preset (`from_preset`).
//!
//! Nothing here reads mail on the window's thread: what it did lately
//! names its messages from the index the window already holds, and the
//! preview reads the last 30 days on a thread of its own.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use cxx_qt_lib::QString;
use serde::Serialize;
use sioul_core::config::Config;
use sioul_core::projects::ProjectStore;
use sioul_core::wiring::acts::{self, Act, Acts};
use sioul_core::wiring::store::{self, Rule, Rules};
use sioul_core::wiring::{declare, engine, readers, words};
use std::collections::BTreeSet;
use std::sync::Arc;

/// What a rule did lately, shown: this many acts at most.
const LATELY: usize = 5;
/// How far back the preview and "also for the last 30 days" look.
const DAYS: u32 = 30;
/// Messages the preview names, the rest counted.
const NAMED: usize = 5;
/// The newest messages "Add a rule…" offers to start from, before what the Porch sets aside is left out.
const STARTS: usize = 60;

fn now() -> i64 {
    sioul_core::zone::now().timestamp().as_second()
}

/// This device's name in the sharing: its log of acts is its own.
fn device() -> String {
    sioul_sync::share::Here::load(&sioul_core::config::state_dir()).id
}

/// The engine as this device has it now.
fn engine_here(config: &Config) -> engine::Engine<'static> {
    engine::Engine::here(config, &device(), &sioul_core::zone::now())
}

/// The projects as this device keeps them, with their ties.
fn project_store(config: &Config) -> Option<ProjectStore> {
    config.places().and_then(|places| ProjectStore::open(&places).ok())
}

/// What the core says as a message id ("rules-problem-value") in words; any other text as it is.
fn worded(text: &str) -> String {
    if text.starts_with("rules-") { tr().text(text, None) } else { text.to_string() }
}

/// The rules' list, or why it could not be read.
fn rules() -> Result<Rules, String> {
    Rules::load(&store::path())
}

/// The names a sentence needs, with the subjects of the messages the window knows.
fn names(config: &Config, projects: Option<&ProjectStore>, rules: &Rules, shared: Option<&Shared>) -> words::Names {
    let mut names = words::Names::load(config, projects, rules);
    if let Some(loaded) = shared.and_then(crate::work::loaded_known) {
        for contact in &loaded.contacts {
            names.contacts.insert(contact.uid.clone(), contact.name.clone());
        }
    }
    names
}

// ---------------------------------------------------------------- the list

#[derive(Serialize, Default)]
struct View {
    groups: Vec<Group>,
    presets: Vec<Group>,
    #[serde(rename = "readOnly")]
    read_only: bool,
    note: String,
    count: usize,
}

#[derive(Serialize)]
struct Group {
    heading: String,
    sets: Vec<Set>,
    /// Its place among the types and their events, for the order of the groups.
    #[serde(skip)]
    order: (usize, usize),
}

/// The rules that share one "when", said once above them when they are several.
#[derive(Serialize)]
struct Set {
    when: String,
    rules: Vec<Line>,
}

#[derive(Serialize, Default)]
struct Line {
    id: String,
    /// Yours, kept in the rules' list: switched, edited, removed here.
    own: bool,
    on: bool,
    sentence: String,
    /// What it does, under a "when" said once.
    then: String,
    /// Why it cannot run, in words; "" when it can.
    problem: String,
    /// Written by a newer Sioul: kept as it is, never edited here.
    unknown: bool,
    /// The rule itself, for the editor.
    rule: serde_json::Value,
    /// Kept elsewhere: where, in words.
    kept: String,
    /// Kept elsewhere: what opens its editor there (main.qml's `openThing`).
    open: Option<Open>,
    #[serde(rename = "openText")]
    open_text: String,
    lately: Vec<Lately>,
    /// A preset: whether a setting turns it on or off, which, and whether it can be copied.
    #[serde(skip_serializing_if = "Option::is_none")]
    preset: Option<serde_json::Value>,
    /// A preset: its switch and its details, as the settings they are today.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    settings: Vec<sioul_core::settings::Setting>,
}

#[derive(Serialize)]
struct Open {
    kind: String,
    uri: String,
    key: String,
}

#[derive(Serialize)]
struct Lately {
    line: String,
    act: Act,
    object: String,
    undone: bool,
}

/// A group's place: its type's among the declarations, then its event's.
fn order_of(rule: &Rule) -> (usize, usize) {
    let types = declare::TYPES;
    let at = types.iter().position(|t| t.id == rule.when.kind).unwrap_or(types.len());
    let event = types.get(at).and_then(|t| t.events.iter().position(|e| e.id == rule.when.event)).unwrap_or(usize::MAX);
    (at, event)
}

/// Lines put in their groups, and in each group, those of the same "when" together, in the order they come.
fn grouped(lines: Vec<(Rule, Line)>, names: &words::Names) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for (rule, line) in lines {
        let heading = words::when_heading(&rule, tr());
        let when = words::when_shared(&rule, names, tr());
        let group = match groups.iter().position(|g| g.heading == heading) {
            Some(at) => &mut groups[at],
            None => {
                groups.push(Group { heading, sets: Vec::new(), order: order_of(&rule) });
                groups.last_mut().expect("just pushed")
            }
        };
        match group.sets.iter_mut().find(|s| s.when == when) {
            Some(set) => set.rules.push(line),
            None => group.sets.push(Set { when, rules: vec![line] }),
        }
    }
    groups.sort_by_key(|g| g.order);
    groups
}

/// A rule's line: its sentence and its halves, why it cannot run.
fn line_of(rule: &Rule, names: &words::Names) -> Line {
    Line {
        id: rule.id.clone(),
        on: rule.on,
        sentence: words::sentence(rule, names, tr()),
        then: words::then_part(rule, names, tr()),
        problem: rule.problem().map(|p| tr().text(p, None)).unwrap_or_default(),
        unknown: !rule.unknown.is_empty(),
        rule: serde_json::to_value(rule).unwrap_or_default(),
        ..Line::default()
    }
}

/// Where a rule kept elsewhere lives, in words, and what opens it there.
fn kept_of(source: &readers::Source, names: &words::Names) -> (String, Option<Open>, String) {
    let named = |map: &std::collections::BTreeMap<String, String>, id: &str| map.get(id).cloned().unwrap_or_else(|| id.to_string());
    match source.edited {
        "project" => (
            say("rules-kept-project", &[("project", named(&names.projects, &source.project))]),
            Some(Open { kind: "project".into(), uri: format!("sioul:project/{}", source.project), key: source.project.clone() }),
            tr().text("rules-open-project", None),
        ),
        "budgets" => (say("rules-kept-budget", &[("budget", named(&names.budgets, &source.budget))]), Some(Open { kind: "budget".into(), uri: String::new(), key: source.budget.clone() }), tr().text("rules-open-budget", None)),
        "mail-filters" => (
            tr().text(if source.stop { "rules-kept-filters-stop" } else { "rules-kept-filters" }, None),
            Some(Open { kind: "mail-settings:mail.filter".into(), uri: String::new(), key: String::new() }),
            tr().text("rules-open-filters", None),
        ),
        "topics" => (say("rules-kept-topics", &[("topic", named(&names.topics, &source.topic))]), Some(Open { kind: "mail-settings:topics.list".into(), uri: String::new(), key: String::new() }), tr().text("rules-open-topics", None)),
        _ => (String::new(), None, String::new()),
    }
}

/// What a rule did lately, newest first, each in words with when it was.
fn lately(acts: &Acts, rule: &str, names: &words::Names) -> Vec<Lately> {
    let zone = sioul_core::zone::current();
    acts.lately(rule, LATELY)
        .into_iter()
        .map(|(act, undone)| {
            let at = jiff::Timestamp::from_millisecond(act.at).map(|t| t.to_zoned(zone.clone()));
            let when = at.map(|at| tr().date(&at, true)).unwrap_or_default();
            let said = words::act_line(act, names, tr());
            let line = if undone { say("rules-act-undone", &[("when", when), ("line", said)]) } else { say("rules-act-when", &[("when", when), ("line", said)]) };
            Lately { line, act: act.clone(), object: act.object.clone(), undone }
        })
        .collect()
}

/// The list as Settings ▸ Rules shows it (`View`, as JSON).
pub(crate) fn view(shared: &Shared) -> String {
    let config = load_config();
    let projects = project_store(&config);
    let mut out = View::default();
    let list = match rules() {
        Ok(list) => list,
        Err(e) => {
            out.note = say("rules-not-read", &[("error", e)]);
            Rules::default()
        }
    };
    out.read_only = list.read_only();
    if out.read_only {
        out.note = tr().text("rules-read-only", None);
    }
    let mut names = names(&config, projects.as_ref(), &list, Some(shared));
    let acts = Acts::read(&acts::folder());
    // The messages what they did names, from the index the window already holds.
    if let Some(loaded) = crate::work::loaded_known(shared) {
        for act in &acts.all {
            if let Some(mail) = act.object.strip_prefix("mid:").and_then(|id| loaded.mail.get(id)) {
                names.subjects.insert(act.object.clone(), mail.subject.clone());
            }
        }
    }
    let mut lines: Vec<(Rule, Line)> = list
        .rules
        .iter()
        .map(|rule| {
            let mut line = line_of(rule, &names);
            line.own = true;
            line.lately = lately(&acts, &rule.id, &names);
            (rule.clone(), line)
        })
        .collect();
    out.count = lines.len();
    // What exists today in its own place, as rules of the same shape, read-only.
    let ledger = config.records_root().and_then(|root| sioul_core::budget::Ledger::load(&root).ok());
    let said = sioul_core::topics::labels::Said::load();
    for read in readers::read_all(projects.as_ref(), ledger.as_ref(), &config.mail.filters, &said) {
        let mut line = line_of(&read.rule, &names);
        let (kept, open, open_text) = kept_of(&read.source, &names);
        line.kept = if read.rule.on { kept } else { format!("{kept} {}", tr().text("rules-kept-off", None)) };
        line.open = open;
        line.open_text = open_text;
        // A rule kept elsewhere runs as it does there: no problem of the engine's to say.
        line.problem = String::new();
        lines.push((read.rule, line));
    }
    out.groups = grouped(lines, &names);
    // The rules that come with Sioul, with their settings of today.
    let all = declare::presets();
    let keys: Vec<&str> = all.iter().flat_map(|p| p.setting.into_iter().chain(p.params.iter().map(|param| param.key))).collect();
    let rows = setting_rows(&config, projects.as_ref(), &keys);
    let presets = all
        .into_iter()
        .map(|preset| {
            let mut line = line_of(&preset.rule, &names);
            line.id = preset.id.to_string();
            line.problem = String::new();
            let own: Vec<&str> = preset.setting.into_iter().chain(preset.params.iter().map(|param| param.key)).collect();
            line.settings = own.iter().filter_map(|key| rows.iter().find(|r| r.key == *key).cloned()).collect();
            // On as its setting says, else as Sioul ships it.
            line.on = preset.setting.and_then(|key| line.settings.iter().find(|r| r.key == key)).map_or(preset.rule.on, |r| r.value != sioul_core::config::SettingValue::Bool(false));
            line.preset = serde_json::to_value(&preset).ok();
            (preset.rule.clone(), line)
        })
        .collect();
    out.presets = grouped(presets, &names);
    json(&out)
}

/// The settings of these keys, as their pages show them (drawn by
/// SettingRow.qml, saved as every setting is): the presets' switches and
/// details are the settings they are today, never a copy.
fn setting_rows(config: &Config, projects: Option<&ProjectStore>, keys: &[&str]) -> Vec<sioul_core::settings::Setting> {
    let mut out: Vec<sioul_core::settings::Setting> = Vec::new();
    if keys.is_empty() {
        return out;
    }
    let lists: Vec<(String, String)> = sioul_core::tasks::lists().into_iter().filter(|c| !c.read_only).map(|c| (format!("{}/{}", c.account, c.id), c.label(config, tr()))).collect();
    for view in ["parameters", "mail", "tasks", "agenda", "money", "notes", "contacts", "porch", "sites", "texts"] {
        for row in sioul_core::settings::for_view(view, config, tr(), &lists, projects) {
            if keys.contains(&row.key.as_str()) && row.kind != sioul_core::settings::Kind::Note && !out.iter().any(|r| r.key == row.key) {
                out.push(row);
            }
        }
        if keys.iter().all(|k| out.iter().any(|r| r.key == *k)) {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------- the editor

/// The declarations, in your words.
pub(crate) fn catalogue() -> String {
    json(&declare::catalogue(tr()))
}

/// One thing to choose for a value: {id, name, available}.
#[derive(Serialize)]
struct Pick {
    id: String,
    name: String,
    available: bool,
}

/// What each kind of value chooses among, by the value's kind ("project",
/// "topic", "account", "list", "model", "level", "contact", "file-type",
/// "paper-kind", "person").
pub(crate) fn choices(shared: &Shared) -> String {
    let config = load_config();
    let projects = project_store(&config);
    let list = rules().unwrap_or_default();
    let given = declare::choices(&config, projects.as_ref(), &list, tr());
    let pick = |id: &str, name: &str, available: bool| Pick { id: id.to_string(), name: name.to_string(), available };
    let titles: std::collections::BTreeMap<String, String> = given.projects.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
    let mut out: std::collections::BTreeMap<&str, Vec<Pick>> = std::collections::BTreeMap::new();
    out.insert("project", given.projects.iter().map(|p| pick(&p.id, &p.name, true)).collect());
    out.insert("topic", given.topics.iter().map(|p| pick(&p.id, &p.name, true)).collect());
    out.insert("account", given.accounts.iter().map(|p| pick(&p.id, &p.name, true)).collect());
    out.insert("list", given.lists.iter().map(|l| pick(&l.id, &l.name, !l.read_only)).collect());
    out.insert(
        "model",
        given
            .models
            .iter()
            .map(|m| {
                let name = match &m.project {
                    Some(project) => say("rules-model-of", &[("model", m.model.name.clone()), ("project", titles.get(project).cloned().unwrap_or_else(|| project.clone()))]),
                    None => m.model.name.clone(),
                };
                pick(&m.id, &name, true)
            })
            .collect(),
    );
    out.insert("level", given.levels.iter().map(|c| pick(c.id, &c.name, c.available)).collect());
    out.insert("file-type", given.file_types.iter().map(|p| pick(&p.id, &p.name, true)).collect());
    out.insert("paper-kind", given.paper_kinds.iter().map(|p| pick(&p.id, &p.name, true)).collect());
    out.insert("person", given.persons.iter().map(|p| pick(&p.id, &p.name, true)).collect());
    let mut contacts: Vec<Pick> = crate::work::loaded_known(shared).map(|l| l.contacts.iter().filter(|c| !c.uid.is_empty()).map(|c| pick(&c.uid, &c.name, true)).collect()).unwrap_or_default();
    contacts.sort_by_key(|c| c.name.to_lowercase());
    out.insert("contact", contacts);
    json(&out)
}

/// A rule being edited (JSON), in words: {sentence, problem}.
pub(crate) fn said(given: &str) -> String {
    let rule: Rule = match serde_json::from_str(given) {
        Ok(rule) => rule,
        Err(e) => return json(&serde_json::json!({ "sentence": "", "problem": e.to_string() })),
    };
    let config = load_config();
    let projects = project_store(&config);
    let list = rules().unwrap_or_default();
    let names = names(&config, projects.as_ref(), &list, None);
    json(&serde_json::json!({ "sentence": words::sentence(&rule, &names, tr()), "problem": rule.problem().map(|p| tr().text(p, None)).unwrap_or_default() }))
}

/// What the preview found: the rule as asked, how many, in words, the first few.
#[derive(Serialize, Default)]
struct Previewed {
    asked: String,
    count: usize,
    text: String,
    lines: Vec<String>,
}

/// The mail of the last 30 days, judged by the engine as this device has it.
fn recent(config: &Config) -> (engine::Engine<'static>, Vec<sioul_core::porch::Triaged>) {
    let index = sioul_core::mailindex::MailIndex::build_ranked(&sioul_core::mailindex::MailIndex::roots_of(config));
    let files = engine::recent_files(&index, now(), DAYS);
    let engine = engine_here(config);
    let judged = engine.judge(&files);
    (engine, judged)
}

/// What a rule (JSON, as `asked`) would have done in the last 30 days,
/// found off the window's thread; `rules_previewed` brings it.
pub(crate) fn preview(qt: QtThread, asked: String) {
    std::thread::spawn(move || {
        let mut found = Previewed { asked: asked.clone(), ..Previewed::default() };
        match serde_json::from_str::<Rule>(&asked) {
            Err(e) => found.text = e.to_string(),
            Ok(rule) => {
                let config = load_config();
                let (engine, judged) = recent(&config);
                let would = engine.preview(&rule, &judged, tr());
                found.count = would.len();
                found.text = if would.is_empty() { tr().text("rules-preview-none", None) } else { tr().text("rules-preview-some", Some(&tr().counted(would.len()))) };
                let zone = sioul_core::zone::current();
                found.lines = would
                    .iter()
                    .take(NAMED)
                    .map(|w| {
                        let day = jiff::Timestamp::from_second(w.arrived).map(|t| tr().day_month(t.to_zoned(zone.clone()).date())).unwrap_or_default();
                        let does = w.refused.map_or_else(|| w.line.clone(), |why| tr().text(why, None));
                        say("rules-preview-line", &[("day", day), ("who", w.from.clone()), ("does", does)])
                    })
                    .collect();
                if would.len() > NAMED {
                    found.lines.push(tr().text("rules-preview-more", Some(&tr().counted(would.len() - NAMED))));
                }
            }
        }
        let json = json(&found);
        let _ = qt.queue(move |mut sioul| sioul.as_mut().rules_previewed(QString::from(&json)));
    });
}

/// A rule saved (JSON) through the store: {id, error}. Said in the status
/// line; "also for the last 30 days", once each, off the window's thread.
pub(crate) fn save(qt: &QtThread, shared: &Arc<Shared>, given: &str, also: bool) -> String {
    let rule: Rule = match serde_json::from_str(given) {
        Ok(rule) => rule,
        Err(e) => return json(&serde_json::json!({ "id": "", "error": e.to_string() })),
    };
    let put = Rules::change(&store::path(), |list| list.put(rule.clone(), now())).and_then(|r| r);
    let id = match put {
        Ok(id) => id,
        Err(e) => return json(&serde_json::json!({ "id": "", "error": worded(&e) })),
    };
    let config = load_config();
    let projects = project_store(&config);
    let list = rules().unwrap_or_default();
    let saved = list.get(&id).cloned().unwrap_or(rule);
    let sentence = words::sentence(&saved, &names(&config, projects.as_ref(), &list, Some(shared)), tr());
    crate::backend::tell(qt, shared, say("rules-saved", &[("sentence", sentence.clone())]));
    if also && saved.runs() {
        let (qt, shared) = (qt.clone(), Arc::clone(shared));
        std::thread::spawn(move || {
            let config = load_config();
            let (mut engine, judged) = recent(&config);
            // A file a rule keeps as a paper, once the antivirus found it clean (as at arrival).
            engine.scan = Box::new(|file: &std::path::Path| sioul_sync::antivirus::for_rules(file, crate::attachments::phone()));
            let outcome = engine.for_already_here(&saved.id, &judged, tr());
            let done = tr().text("rules-done-past", Some(&tr().counted(outcome.done.len())));
            crate::backend::tell(&qt, &shared, say("rules-saved-past", &[("sentence", sentence), ("done", done)]));
        });
    }
    json(&serde_json::json!({ "id": id, "error": "" }))
}

/// A general model of task (JSON) kept with the rules: {id, error}.
pub(crate) fn save_model(given: &str) -> String {
    let model: store::Model = match serde_json::from_str(given) {
        Ok(model) => model,
        Err(e) => return json(&serde_json::json!({ "id": "", "error": e.to_string() })),
    };
    match Rules::change(&store::path(), |list| list.put_model(model)).and_then(|r| r) {
        Ok(id) => json(&serde_json::json!({ "id": id, "error": "" })),
        Err(e) => json(&serde_json::json!({ "id": "", "error": worded(&e) })),
    }
}

/// A rule turned on or off, said in the status line.
pub(crate) fn set_on(qt: &QtThread, shared: &Shared, id: &str, on: bool) {
    let changed = Rules::change(&store::path(), |list| list.set_on(id, on, now()).then(|| list.get(id).cloned()).flatten());
    let line = match changed {
        Err(e) => worded(&e),
        Ok(None) => return,
        Ok(Some(rule)) => {
            let config = load_config();
            let projects = project_store(&config);
            let list = rules().unwrap_or_default();
            let sentence = words::sentence(&rule, &names(&config, projects.as_ref(), &list, Some(shared)), tr());
            say(if rule.on { "rules-switched-on" } else { "rules-switched-off" }, &[("sentence", sentence)])
        }
    };
    crate::backend::tell(qt, shared, line);
}

/// A rule removed at once, Undo in the status line putting it back.
pub(crate) fn remove(qt: &QtThread, shared: &Arc<Shared>, id: &str) {
    let removed = Rules::change(&store::path(), |list| {
        let rule = list.get(id).cloned();
        list.remove(id);
        rule
    });
    match removed {
        Err(e) => crate::backend::tell(qt, shared, worded(&e)),
        Ok(None) => {}
        Ok(Some(rule)) => {
            let config = load_config();
            let projects = project_store(&config);
            let list = rules().unwrap_or_default();
            let sentence = words::sentence(&rule, &names(&config, projects.as_ref(), &list, Some(shared)), tr());
            crate::mail::offer_back(qt, shared, say("rules-removed", &[("sentence", sentence)]), move |_, _| {
                Rules::change(&store::path(), |list| list.put(rule.clone(), now())).and_then(|r| r).err().map(|e| worded(&e)).unwrap_or_default()
            });
        }
    }
}

/// One act of a rule undone (its JSON, as the list gave it): put back if it is still as the rule left it.
pub(crate) fn undo_act(qt: &QtThread, shared: &Shared, given: &str) {
    let line = match serde_json::from_str::<Act>(given) {
        Err(e) => e.to_string(),
        Ok(act) => {
            let engine = engine_here(&load_config());
            match engine::undo(&act, &engine.place, engine.places.as_ref(), &*engine.find_list, now() * 1000) {
                Ok(undone) => tr().text(if undone.put_back { "rules-act-put-back" } else { "rules-act-kept" }, None),
                Err(e) => worded(&e),
            }
        }
    };
    crate::backend::tell(qt, shared, line);
}

/// "Not for this one": the rule leaves that object alone from now on, on every device.
pub(crate) fn not_for_this_one(qt: &QtThread, shared: &Shared, rule: &str, object: &str) {
    let line = match engine::not_for_this_one(rule, object, &engine::Place::here(&device()), now() * 1000) {
        Ok(()) => tr().text("rules-not-this-one-said", None),
        Err(e) => worded(&e),
    };
    crate::backend::tell(qt, shared, line);
}

// ---------------------------------------------------------------- starts

#[derive(Serialize)]
struct Start {
    key: String,
    subject: String,
    from: String,
    day: String,
}

/// What "Add a rule…" offers to start from: the newest messages of the last
/// 30 days the window knows (none of your own, none the Porch sets aside),
/// newest first; the presets the engine runs now, by their sentences.
pub(crate) fn starts(shared: &Shared) -> String {
    let config = load_config();
    let own = sioul_core::porch::own_addresses(&config.mail_sources());
    let (now, since) = (now(), now() - i64::from(DAYS) * 86_400);
    let zone = sioul_core::zone::current();
    let mut found: Vec<(i64, Start)> = crate::work::loaded_known(shared)
        .map(|loaded| {
            loaded
                .mail
                .iter()
                .filter(|(_, m)| m.date >= since && m.date <= now + 86_400 && !own.contains(&m.address))
                .map(|(_, m)| {
                    let day = jiff::Timestamp::from_second(m.date).map(|t| tr().day_month(t.to_zoned(zone.clone()).date())).unwrap_or_default();
                    (m.date, Start { key: m.path.to_string_lossy().into_owned(), subject: m.subject.clone(), from: m.from.clone(), day })
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort_by_key(|(date, _)| std::cmp::Reverse(*date));
    found.truncate(STARTS);
    // Never what the Porch sets aside (a rule never sees it): judged as the engine judges.
    let files: Vec<std::path::PathBuf> = found.iter().map(|(_, s)| std::path::PathBuf::from(&s.key)).collect();
    let admitted: BTreeSet<std::path::PathBuf> = engine_here(&config).judge(&files).iter().filter(|t| !engine::set_aside(t)).filter_map(|t| t.card.path.clone()).collect();
    found.retain(|(_, s)| admitted.contains(std::path::Path::new(&s.key)));
    let projects = project_store(&config);
    let list = rules().unwrap_or_default();
    let names = names(&config, projects.as_ref(), &list, Some(shared));
    let presets: Vec<serde_json::Value> = declare::presets().into_iter().filter(|p| p.offered).map(|p| serde_json::json!({ "id": p.id, "sentence": words::sentence(&p.rule, &names, tr()) })).collect();
    json(&serde_json::json!({ "messages": found.into_iter().map(|(_, s)| s).collect::<Vec<Start>>(), "presets": presets }))
}

/// A draft rule from a message (its file, as the reader names it): its
/// "when" filled from the message, as JSON; {error} when the message is
/// gone or set aside.
pub(crate) fn from_message(key: &str) -> String {
    let config = load_config();
    let file = std::path::Path::new(key);
    match declare::example(file, &config) {
        Some(mut rule) => {
            // A new rule, on once saved.
            rule.on = true;
            // A message with a PDF: keeping its attachments in Papers, offered first, the action
            // most of such mail calls for (docs/rules.md, "The five actions of phase 1").
            let pdf = sioul_core::maildir::read_one(file).is_some_and(|card| card.attachments.iter().any(|name| name.to_lowercase().ends_with(".pdf")));
            let papers = declare::of("message").and_then(|t| t.actions.iter().find(|a| a.writes == declare::Writes::Paper && declare::available(a.phase)));
            if rule.then.action.is_empty()
                && pdf
                && let Some(action) = papers
            {
                rule.then = store::Then { kind: "message".into(), action: action.id.into(), with: Default::default() };
            }
            json(&rule)
        }
        None => json(&serde_json::json!({ "error": tr().text("rules-message-gone", None) })),
    }
}

/// A rule of your own from a preset, to change before saving, as JSON; {error} when it cannot be copied.
pub(crate) fn from_preset(id: &str) -> String {
    match declare::from_preset(id) {
        Some(mut rule) => {
            rule.on = true;
            json(&rule)
        }
        None => json(&serde_json::json!({ "error": tr().text("rules-preset-not-copied", None) })),
    }
}

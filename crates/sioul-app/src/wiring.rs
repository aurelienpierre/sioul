// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Settings ▸ Rules in the window (docs/rules.md, "The view"; the rules
//! themselves are `sioul_core::wiring`).
//!
//! - `view`: the one list, grouped by what happens, the rules that share a
//!   "when" together; your rules, each with what it did lately (`history`:
//!   one state per result, from the record of acts); what exists today in
//!   its own place (projects' routes and the tasks they make, budgets' mail
//!   rules, mail filters, topics' senders: `wiring::readers`), read-only,
//!   folded by the place that keeps it, which a link opens; the presets, the
//!   rules that come with Sioul, each in its own written sentence, grouped by
//!   what they watch, with a link to where its setting lives. Every
//!   sentence of a rule is `wiring::words`'s.
//! - The editor (`RuleEditor.qml`): the declarations (`catalogue`), what
//!   values choose among (`choices`), the rule in words as it is changed
//!   (`said`), what it would have done in the last 30 days (`preview`: only
//!   the newest asked runs to its end), saving through the store (`save`),
//!   "also for the last 30 days" then, once each.
//! - A rule turned on or off (`set_on`), removed with Undo in the status
//!   line (`remove`); an act undone, or "not for this one" (`undo_act`,
//!   `not_for_this_one`).
//! - A rule from an example (`from_message`), from a preset (`from_preset`);
//!   the messages "Add a rule…" offers (`starts`).
//!
//! Nothing here reads mail, judges a message or builds the engine on the
//! window's thread: the history names its messages from the index the
//! window already holds; the preview, the past, the starts and an act's
//! Undo run on threads of their own and answer by a signal or the status line.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use cxx_qt_lib::QString;
use serde::Serialize;
use sioul_core::config::Config;
use sioul_core::projects::ProjectStore;
use sioul_core::wiring::acts::{self, Act, Acts};
use sioul_core::wiring::store::{self, Rule, Rules};
use sioul_core::wiring::{declare, engine, readers, words};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// What a rule did lately, shown: this many acts at most.
const LATELY: usize = 5;
/// How far back the preview and "also for the last 30 days" look.
const DAYS: u32 = 30;
/// Messages the preview names, the rest counted.
const NAMED: usize = 5;
/// The newest messages "Add a rule…" judges, before what the Porch holds is left out.
const STARTS: usize = 60;

/// The newest preview asked: an older one stops where it stands.
static PREVIEW: AtomicU64 = AtomicU64::new(0);

fn now() -> i64 {
    sioul_core::zone::now().timestamp().as_second()
}

fn now_ms() -> i64 {
    sioul_core::zone::now().timestamp().as_millisecond()
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

/// A rule as the editor sends it, or why it cannot be read, in words.
fn parsed(given: &str) -> Result<Rule, String> {
    serde_json::from_str(given).map_err(|e| say("rules-not-understood", &[("error", e.to_string())]))
}

/// The names a sentence needs, with the contacts the window knows.
fn names(config: &Config, projects: Option<&ProjectStore>, rules: &Rules, shared: Option<&Shared>) -> words::Names {
    let mut names = words::Names::load(config, projects, rules);
    if let Some(loaded) = shared.and_then(crate::work::loaded_known) {
        for contact in &loaded.contacts {
            names.contacts.insert(contact.uid.clone(), contact.name.clone());
        }
    }
    names
}

/// A thread of its own, named so that a failure says which.
fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    let _ = std::thread::Builder::new().name(name.to_string()).spawn(work);
}

// ---------------------------------------------------------------- the list

#[derive(Serialize, Default)]
struct View {
    groups: Vec<Group>,
    presets: Vec<PresetGroup>,
    #[serde(rename = "readOnly")]
    read_only: bool,
    note: String,
    count: usize,
}

/// What happens, and under it the rules that answer it: yours by their
/// "when", those kept elsewhere folded by the place that keeps them.
#[derive(Serialize)]
struct Group {
    heading: String,
    sets: Vec<Set>,
    places: Vec<Place>,
    /// Its place among the types and their events, for the order of the groups.
    #[serde(skip)]
    order: (usize, usize),
}

/// Your rules that share one "when", said once above them when they are several.
#[derive(Serialize)]
struct Set {
    when: String,
    rules: Vec<Line>,
}

/// One place that keeps rules of its own (a project, a budget, Mail's filters, the
/// topics): where, in words, the link that opens it, its rules' sentences.
#[derive(Serialize)]
struct Place {
    id: String,
    /// "The project Taxes 2025": the fold's words, with how many rules it keeps (`title`).
    #[serde(skip)]
    name: String,
    title: String,
    kept: String,
    open: Option<Open>,
    #[serde(rename = "openText")]
    open_text: String,
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
    /// Written by a newer Sioul: kept as it is, never edited here, removed only once asked.
    unknown: bool,
    /// The rule itself, for the editor.
    rule: serde_json::Value,
    /// Kept elsewhere: off there, said in words.
    off: String,
    lately: Vec<Lately>,
}

#[derive(Serialize)]
struct Open {
    kind: String,
    uri: String,
    key: String,
}

/// One result of a rule, as it stands now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Done, and as the rule left it as far as the record says.
    Done,
    /// Undo took it back.
    Back,
    /// Undo found it changed since, and left it.
    Left,
    /// Taken out since: the antivirus refused its file.
    Out,
    /// The rule matched but could not act yet: tried again within the week.
    Waiting,
}

#[derive(Serialize)]
struct Lately {
    line: String,
    act: Act,
    object: String,
    /// Undo is offered: done, and the rule wrote it.
    undo: bool,
    /// "Not for this one" is offered: it waits, and would be tried again.
    #[serde(rename = "notThis")]
    not_this: bool,
}

/// A rule's history, newest first: what waits to be tried again, then
/// what it did lately, each with the state of its own result (never the
/// state of another result of the same message).
fn history(acts: &Acts, rule: &str, now_ms: i64) -> Vec<(Act, State)> {
    let mut out: Vec<(Act, State)> = acts.waiting(rule, now_ms).into_iter().map(|a| (a.clone(), State::Waiting)).collect();
    for (act, _) in acts.lately(rule, LATELY) {
        let same = |a: &Act| a.rule == act.rule && a.object == act.object && !act.result.is_empty() && a.result == act.result;
        let state = if acts.all.iter().any(|a| !a.refused.is_empty() && !act.result.is_empty() && a.result == act.result) {
            State::Out
        } else if acts.all.iter().any(|a| a.undone && a.back && same(a)) {
            State::Back
        } else if acts.all.iter().any(|a| a.undone && !a.back && same(a)) {
            State::Left
        } else {
            State::Done
        };
        out.push((act.clone(), state));
    }
    out
}

/// A rule's history in words, each line with what can be done about it.
fn lately(acts: &Acts, rule: &str, names: &words::Names) -> Vec<Lately> {
    let zone = sioul_core::zone::current();
    history(acts, rule, now_ms())
        .into_iter()
        .map(|(act, state)| {
            let when = jiff::Timestamp::from_millisecond(act.at).map(|t| tr().date(&t.to_zoned(zone.clone()), true)).unwrap_or_default();
            let line = match state {
                State::Waiting => {
                    let subject = names.subjects.get(&act.object).filter(|s| !s.trim().is_empty()).map_or_else(|| tr().text("rule-a-message-first", None), |s| say("rule-quoted", &[("text", s.clone())]));
                    say("rules-act-waiting", &[("when", when), ("Subject", subject), ("why", worded(&act.pending))])
                }
                _ => {
                    let id = match state {
                        State::Back => "rules-act-back",
                        State::Left => "rules-act-left",
                        State::Out => "rules-act-out",
                        _ => "rules-act-when",
                    };
                    say(id, &[("when", when), ("line", words::act_line(&act, names, tr()))])
                }
            };
            // A call's time added to its person's task has no Undo of its own (the task is yours to edit).
            Lately { line, undo: state == State::Done && act.wrote && act.act != "joined", not_this: state == State::Waiting, object: act.object.clone(), act }
        })
        .collect()
}

/// A group's place: its type's among the declarations, then its event's.
fn order_of(rule: &Rule) -> (usize, usize) {
    let types = declare::TYPES;
    let at = types.iter().position(|t| t.id == rule.when.kind).unwrap_or(types.len());
    let event = types.get(at).and_then(|t| t.events.iter().position(|e| e.id == rule.when.event)).unwrap_or(usize::MAX);
    (at, event)
}

/// The group a rule goes under, made when it is the first.
fn group_of<'a>(groups: &'a mut Vec<Group>, rule: &Rule) -> &'a mut Group {
    let heading = words::when_heading(rule, tr());
    match groups.iter().position(|g| g.heading == heading) {
        Some(at) => &mut groups[at],
        None => {
            groups.push(Group { heading, sets: Vec::new(), places: Vec::new(), order: order_of(rule) });
            groups.last_mut().expect("just pushed")
        }
    }
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

/// Where a rule kept elsewhere lives: its place's id, its name, where in words, and what opens it there.
fn place_of(source: &readers::Source, names: &words::Names) -> (String, String, String, Option<Open>, String) {
    let named = |map: &BTreeMap<String, String>, id: &str| map.get(id).cloned().unwrap_or_else(|| id.to_string());
    match source.edited {
        "project" => (
            format!("project:{}", source.project),
            say("rules-place-project", &[("project", named(&names.projects, &source.project))]),
            say("rules-kept-project", &[("project", named(&names.projects, &source.project))]),
            Some(Open { kind: "project".into(), uri: format!("sioul:project/{}", source.project), key: source.project.clone() }),
            tr().text("rules-open-project", None),
        ),
        "budgets" => (
            format!("budget:{}", source.budget),
            say("rules-place-budget", &[("budget", named(&names.budgets, &source.budget))]),
            say("rules-kept-budget", &[("budget", named(&names.budgets, &source.budget))]),
            Some(Open { kind: "budget".into(), uri: String::new(), key: source.budget.clone() }),
            tr().text("rules-open-budget", None),
        ),
        "mail-filters" => ("filters".into(), tr().text("rules-place-filters", None), tr().text("rules-kept-filters", None), Some(Open { kind: "mail-settings:mail.filter".into(), uri: String::new(), key: String::new() }), tr().text("rules-open-filters", None)),
        "topics" => (
            format!("topic:{}", source.topic),
            say("rules-place-topic", &[("topic", named(&names.topics, &source.topic))]),
            say("rules-kept-topics", &[("topic", named(&names.topics, &source.topic))]),
            Some(Open { kind: "mail-settings:topics.list".into(), uri: String::new(), key: String::new() }),
            tr().text("rules-open-topics", None),
        ),
        other => (other.to_string(), other.to_string(), String::new(), None, String::new()),
    }
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
    // The messages the history names, from the index the window already holds.
    if let Some(loaded) = crate::work::loaded_known(shared) {
        for act in &acts.all {
            if let Some(mail) = act.object.strip_prefix("mid:").and_then(|id| loaded.mail.get(id)) {
                names.subjects.insert(act.object.clone(), mail.subject.clone());
            }
        }
    }
    // The calls it names, by what they were ("Missed call from Noa Linden"), while their phones keep them.
    if acts.all.iter().any(|a| a.object.starts_with("sioul:call/")) {
        names.subjects.extend(crate::calls::titles());
    }
    let mut groups: Vec<Group> = Vec::new();
    for rule in &list.rules {
        let mut line = line_of(rule, &names);
        line.own = true;
        line.lately = lately(&acts, &rule.id, &names);
        let when = words::when_shared(rule, &names, tr());
        let group = group_of(&mut groups, rule);
        match group.sets.iter_mut().find(|s| s.when == when) {
            Some(set) => set.rules.push(line),
            None => group.sets.push(Set { when, rules: vec![line] }),
        }
    }
    out.count = list.rules.len();
    // What exists today in its own place, read-only, folded by that place.
    let ledger = config.records_root().and_then(|root| sioul_core::budget::Ledger::load(&root).ok());
    let said = sioul_core::topics::labels::Said::load();
    for read in readers::read_all(projects.as_ref(), ledger.as_ref(), &config.mail.filters, &said) {
        let mut line = line_of(&read.rule, &names);
        // It runs as it does there: no problem of the engine's to say.
        line.problem = String::new();
        if !read.rule.on {
            line.off = tr().text("rules-kept-off", None);
        } else if read.source.stop {
            line.off = tr().text("rules-kept-filters-stop", None);
        }
        let (id, name, kept, open, open_text) = place_of(&read.source, &names);
        let group = group_of(&mut groups, &read.rule);
        match group.places.iter_mut().find(|p| p.id == id) {
            Some(place) => place.rules.push(line),
            None => group.places.push(Place { id, name, title: String::new(), kept, open, open_text, rules: vec![line] }),
        }
    }
    // Each fold's words: the place, and how many rules it keeps.
    for place in groups.iter_mut().flat_map(|g| g.places.iter_mut()) {
        let mut args = tr().counted(place.rules.len());
        args.set("name", place.name.clone());
        place.title = tr().text("rules-place", Some(&args));
    }
    groups.sort_by_key(|g| g.order);
    out.groups = groups;
    out.presets = presets(&config, projects.as_ref());
    json(&out)
}

// ---------------------------------------------------------------- presets

/// The presets of one type of thing ("Mail", "Tasks"…).
#[derive(Serialize)]
struct PresetGroup {
    heading: String,
    presets: Vec<PresetLine>,
    #[serde(skip)]
    order: usize,
}

/// A rule that comes with Sioul: its written sentence; off, said; where
/// its setting lives, a link there (its switch is never drawn twice);
/// "Copy to make my own" when the engine runs what it says. One the engine
/// runs has its switch and its choices here, their one place (`[presets]`).
#[derive(Serialize)]
struct PresetLine {
    id: String,
    sentence: String,
    off: bool,
    open: Option<Open>,
    #[serde(rename = "openText")]
    open_text: String,
    offered: bool,
    /// Run by the engine: its switch here.
    engine: bool,
    /// Its choices, each ticked or not: "missed-call-task"'s ways.
    choices: Vec<PresetChoice>,
}

/// One of a preset's choices.
#[derive(Serialize)]
struct PresetChoice {
    id: String,
    text: String,
    on: bool,
}

/// The choices a preset of the engine takes: "missed-call-task", the calls
/// it answers (theirs you missed, yours that did not reach them).
fn preset_choices(config: &Config, id: &str) -> Vec<PresetChoice> {
    use sioul_core::wiring::call;
    if id != sioul_core::config::MISSED_CALL_TASK {
        return Vec::new();
    }
    let ways = call::preset_settings(config, id).ways.unwrap_or_else(|| vec!["in".into(), "out".into()]);
    ["in", "out"].iter().map(|way| PresetChoice { id: (*way).to_string(), text: tr().text(&format!("rules-preset-way-{way}"), None), on: ways.iter().any(|w| w == way) }).collect()
}

/// The presets grouped by what they watch, messages, tasks and events first.
fn presets(config: &Config, projects: Option<&ProjectStore>) -> Vec<PresetGroup> {
    let all = declare::presets();
    let keys: Vec<&str> = all.iter().filter_map(|p| p.setting).collect();
    let rows = setting_rows(config, projects, &keys);
    let mut groups: Vec<PresetGroup> = Vec::new();
    for preset in all {
        let kind = preset.rule.when.kind.clone();
        let sentence = tr().own(&format!("rule-preset-{}", preset.id)).unwrap_or_else(|| words::sentence(&preset.rule, &words::Names::default(), tr()));
        let engine = preset.runs == "engine";
        let row = preset.setting.and_then(|key| rows.iter().find(|(_, r)| r.key == key));
        let off = if engine { !sioul_core::wiring::call::preset_on(config, preset.id) } else { row.is_some_and(|(_, r)| r.value == sioul_core::config::SettingValue::Bool(false)) };
        // Where its setting is changed: Mail's ⚙ for mail's, else where Settings sends a key. A call's
        // preset leads to the voicemail threshold, which the Porch's list of calls reads too.
        let open = if engine && kind == "call" {
            Some(Open { kind: "settings:porch.voicemail_seconds".into(), uri: String::new(), key: String::new() })
        } else {
            row.map(|(view, r)| Open { kind: if *view == "mail" { format!("mail-settings:{}", r.key) } else { format!("settings:{}", r.key) }, uri: String::new(), key: String::new() })
        };
        let open_text = match (&open, engine) {
            (None, _) => String::new(),
            (Some(_), true) => tr().text("rules-preset-threshold", None),
            (Some(_), false) => tr().text("rules-preset-where", None),
        };
        let line = PresetLine { id: preset.id.to_string(), sentence, off, open_text, open, offered: preset.offered, engine, choices: if engine { preset_choices(config, preset.id) } else { Vec::new() } };
        match groups.iter_mut().find(|g| g.heading == tr().text(&format!("rules-presets-{kind}"), None)) {
            Some(group) => group.presets.push(line),
            None => groups.push(PresetGroup {
                heading: tr().text(&format!("rules-presets-{kind}"), None),
                presets: vec![line],
                order: declare::TYPES.iter().position(|t| t.id == kind).unwrap_or(usize::MAX),
            }),
        }
    }
    groups.sort_by_key(|g| g.order);
    groups
}

/// A preset the engine runs turned on or off, in its one place
/// (`config::set_preset`: for "missed-call-task", the switches an older
/// device reads beside it); said in the status line.
pub(crate) fn set_preset_on(qt: &QtThread, shared: &Shared, id: &str, on: bool) {
    let config = load_config();
    let mut set = sioul_core::wiring::call::preset_settings(&config, id);
    set.on = Some(on);
    if set.ways.as_ref().is_some_and(Vec::is_empty) {
        set.ways = None;
    }
    let line = match sioul_core::config::set_preset(&crate::backend::config_path(), id, &set) {
        Err(e) => e,
        Ok(()) => {
            sioul_core::changes::said(sioul_core::changes::Kind::Rules);
            let sentence = tr().own(&format!("rule-preset-{id}")).unwrap_or_default();
            say(if on { "rules-switched-on" } else { "rules-switched-off" }, &[("sentence", sentence)])
        }
    };
    crate::backend::tell(qt, shared, line);
}

/// One of a preset's choices ticked or not ("missed-call-task": a way of
/// the calls it answers); the last one is never unticked (the switch turns
/// it off). Said in the status line.
pub(crate) fn set_preset_choice(qt: &QtThread, shared: &Shared, id: &str, choice: &str, on: bool) {
    let config = load_config();
    let mut set = sioul_core::wiring::call::preset_settings(&config, id);
    let mut ways = set.ways.clone().unwrap_or_else(|| vec!["in".into(), "out".into()]);
    ways.retain(|w| w != choice);
    if on {
        ways.push(choice.to_string());
    }
    ways.sort();
    if ways.is_empty() {
        crate::backend::tell(qt, shared, tr().text("rules-preset-one-way", None));
        return;
    }
    set.ways = Some(ways);
    let line = match sioul_core::config::set_preset(&crate::backend::config_path(), id, &set) {
        Err(e) => e,
        Ok(()) => {
            sioul_core::changes::said(sioul_core::changes::Kind::Rules);
            say(if on { "rules-preset-choice-on" } else { "rules-preset-choice-off" }, &[("choice", tr().text(&format!("rules-preset-way-{choice}"), None))])
        }
    };
    crate::backend::tell(qt, shared, line);
}

/// The settings of these keys as their pages show them, each with the view that shows it.
fn setting_rows(config: &Config, projects: Option<&ProjectStore>, keys: &[&str]) -> Vec<(&'static str, sioul_core::settings::Setting)> {
    let mut out: Vec<(&'static str, sioul_core::settings::Setting)> = Vec::new();
    if keys.is_empty() {
        return out;
    }
    for view in ["parameters", "mail", "tasks", "agenda", "money", "notes", "contacts", "porch", "sites", "texts", "supplies", "letters", "papers", "senders", "accounts"] {
        for row in sioul_core::settings::for_view(view, config, tr(), &[], projects) {
            if keys.contains(&row.key.as_str()) && row.kind != sioul_core::settings::Kind::Note && !out.iter().any(|(_, r)| r.key == row.key) {
                out.push((view, row));
            }
        }
        if keys.iter().all(|k| out.iter().any(|(_, r)| r.key == *k)) {
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
    let titles: BTreeMap<String, String> = given.projects.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
    let mut out: BTreeMap<&str, Vec<Pick>> = BTreeMap::new();
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

/// A rule being edited (JSON), in words: {sentence, problem}; a contact by its name.
pub(crate) fn said(shared: &Shared, given: &str) -> String {
    let rule = match parsed(given) {
        Ok(rule) => rule,
        Err(e) => return json(&serde_json::json!({ "sentence": "", "problem": e })),
    };
    let config = load_config();
    let projects = project_store(&config);
    let list = rules().unwrap_or_default();
    let names = names(&config, projects.as_ref(), &list, Some(shared));
    json(&serde_json::json!({ "sentence": words::sentence(&rule, &names, tr()), "problem": rule.problem().map(|p| tr().text(p, None)).unwrap_or_default() }))
}

/// What the preview found: the rule as asked, how many it would act on,
/// in words, the first few; whether "also for these messages" means anything.
#[derive(Serialize, Default)]
struct Previewed {
    asked: String,
    count: usize,
    text: String,
    lines: Vec<String>,
    /// Acting on mail already here does something: never for "tell", which tells only what comes.
    past: bool,
}

/// The mail of the last 30 days, judged by the engine as this device has it.
fn recent(config: &Config) -> (engine::Engine<'static>, Vec<sioul_core::porch::Triaged>) {
    let index = sioul_core::mailindex::MailIndex::build_ranked(&sioul_core::mailindex::MailIndex::roots_of(config));
    let files = engine::recent_files(&index, now(), DAYS);
    let engine = engine_here(config);
    let judged = engine.judge(&files);
    (engine, judged)
}

/// What the preview of a rule says: only what it would do (not what it
/// did already, nor what it would refuse), the first few by their day.
fn previewed(asked: String, rule: &Rule, would: &[engine::Would]) -> Previewed {
    let acting: Vec<&engine::Would> = would.iter().filter(|w| w.refused.is_none() && !w.done_already).collect();
    let zone = sioul_core::zone::current();
    let mut lines: Vec<String> = acting
        .iter()
        .take(NAMED)
        .map(|w| {
            let day = jiff::Timestamp::from_second(w.arrived).map(|t| tr().day_month(t.to_zoned(zone.clone()).date())).unwrap_or_default();
            say("rules-preview-line", &[("day", day), ("who", w.from.clone()), ("does", w.line.clone())])
        })
        .collect();
    if acting.len() > NAMED {
        lines.push(tr().text("rules-preview-more", Some(&tr().counted(acting.len() - NAMED))));
    }
    let text = if acting.is_empty() { tr().text("rules-preview-none", None) } else { tr().text("rules-preview-some", Some(&tr().counted(acting.len()))) };
    Previewed { asked, count: acting.len(), text, lines, past: !acting.is_empty() && rule.then.action != "tell" }
}

/// What a rule about calls would have done with the phones' calls of the
/// last 30 days (`calls::seen_calls`): the calls it acts on, the first few
/// by their day and whom with. Nothing to do "also for these": a rule about
/// calls acts on what comes, on the phone that has the call.
fn previewed_calls(asked: String, rule: &Rule) -> Previewed {
    let config = load_config();
    let calls = crate::calls::seen_calls(i64::from(DAYS));
    let zone = sioul_core::zone::current();
    let region = sioul_core::reach::region(&config);
    let acting = sioul_core::wiring::call::preview(rule, &calls, sioul_core::calls::threshold(&config), region, &zone);
    let does = words::then_part(rule, &words::Names::default(), tr());
    let mut lines: Vec<String> = acting
        .iter()
        .take(NAMED)
        .map(|seen| {
            let day = jiff::Timestamp::from_millisecond(seen.call.at).map(|t| tr().day_month(t.to_zoned(zone.clone()).date())).unwrap_or_default();
            let who = seen.call.contact.as_ref().map(|c| c.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| if seen.call.hidden { tr().text("calls-who-hidden", None) } else { sioul_core::calls::shown_number(&seen.call.number, region) });
            say("rules-preview-line", &[("day", day), ("who", who), ("does", does.clone())])
        })
        .collect();
    if acting.len() > NAMED {
        lines.push(tr().text("rules-preview-more", Some(&tr().counted(acting.len() - NAMED))));
    }
    let text = if acting.is_empty() { tr().text("rules-preview-none-calls", None) } else { tr().text("rules-preview-some-calls", Some(&tr().counted(acting.len()))) };
    Previewed { asked, count: acting.len(), text, lines, past: false }
}

/// What a rule (JSON, as `asked`) would have done in the last 30 days,
/// found off the window's thread; `rules_previewed` brings it. A newer
/// preview asked meanwhile makes this one stop where it stands, unsaid.
pub(crate) fn preview(qt: QtThread, asked: String) {
    let ticket = PREVIEW.fetch_add(1, Ordering::SeqCst) + 1;
    let current = move || PREVIEW.load(Ordering::SeqCst) == ticket;
    spawn("rules-preview", move || {
        let found = match parsed(&asked) {
            Err(e) => Previewed { asked, text: e, ..Previewed::default() },
            // A rule about calls: the phones' calls of the last 30 days.
            Ok(rule) if rule.when.kind == "call" => previewed_calls(asked, &rule),
            Ok(rule) => {
                let config = load_config();
                let index = sioul_core::mailindex::MailIndex::build_ranked(&sioul_core::mailindex::MailIndex::roots_of(&config));
                if !current() {
                    return;
                }
                let files = engine::recent_files(&index, now(), DAYS);
                let engine = engine_here(&config);
                let judged = engine.judge(&files);
                if !current() {
                    return;
                }
                let would = engine.preview(&rule, &judged, tr());
                previewed(asked, &rule, &would)
            }
        };
        if !current() {
            return;
        }
        let json = json(&found);
        let _ = qt.queue(move |mut sioul| sioul.as_mut().rules_previewed(QString::from(&json)));
    });
}

/// A rule saved (JSON) through the store: {id, error}. Said in the status
/// line; "also for the last 30 days", once each, off the window's thread.
pub(crate) fn save(qt: &QtThread, shared: &Arc<Shared>, given: &str, also: bool) -> String {
    let rule = match parsed(given) {
        Ok(rule) => rule,
        Err(e) => return json(&serde_json::json!({ "id": "", "error": e })),
    };
    let put = Rules::change(&store::path(), |list| list.put(rule.clone(), now())).and_then(|r| r);
    let id = match put {
        Ok(id) => id,
        Err(e) => return json(&serde_json::json!({ "id": "", "error": worded(&e) })),
    };
    // Your own copy of a preset the engine runs, saved on, runs in its place: the preset is turned
    // off, said with Undo, so that yours alone decides (its conditions would narrow nothing beside it).
    let preset = rule.preset.clone();
    if rule.on && declare::BY_ENGINE.contains(&preset.as_str()) && sioul_core::wiring::call::preset_on(&load_config(), &preset) {
        let mut set = sioul_core::wiring::call::preset_settings(&load_config(), &preset);
        set.on = Some(false);
        match sioul_core::config::set_preset(&crate::backend::config_path(), &preset, &set) {
            Err(e) => crate::backend::tell(qt, shared, e),
            Ok(()) => {
                let sentence = tr().own(&format!("rule-preset-{preset}")).unwrap_or_default();
                crate::mail::offer_back(qt, shared, say("rules-preset-replaced", &[("sentence", sentence)]), move |_, _| {
                    let mut back = sioul_core::wiring::call::preset_settings(&load_config(), &preset);
                    back.on = Some(true);
                    sioul_core::config::set_preset(&crate::backend::config_path(), &preset, &back).err().unwrap_or_default()
                });
                return json(&serde_json::json!({ "id": id, "error": "" }));
            }
        }
    }
    let config = load_config();
    let projects = project_store(&config);
    let list = rules().unwrap_or_default();
    let saved = list.get(&id).cloned().unwrap_or(rule);
    let sentence = words::sentence(&saved, &names(&config, projects.as_ref(), &list, Some(shared)), tr());
    crate::backend::tell(qt, shared, say("rules-saved", &[("sentence", sentence.clone())]));
    if also && saved.runs() {
        let (qt, shared) = (qt.clone(), Arc::clone(shared));
        spawn("rules-past", move || {
            let config = load_config();
            let (mut engine, judged) = recent(&config);
            // A file a rule keeps as a paper, once the antivirus found it clean, as at arrival;
            // where none can be asked (a phone), kept marked "Not checked by an antivirus".
            engine.scan = Box::new(|file: &std::path::Path| sioul_sync::antivirus::for_rules(file, crate::attachments::phone()));
            engine.can_scan = !crate::attachments::phone() && sioul_sync::antivirus::present();
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
        Err(e) => return json(&serde_json::json!({ "id": "", "error": say("rules-not-understood", &[("error", e.to_string())]) })),
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

/// A rule removed at once, Undo in the status line putting it back. One a
/// newer Sioul wrote cannot be put back here (the list never writes what it
/// does not know): the view asks before it calls this.
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
            if !rule.unknown.is_empty() {
                crate::backend::tell(qt, shared, say("rules-removed-for-good", &[("sentence", sentence)]));
                return;
            }
            crate::mail::offer_back(qt, shared, say("rules-removed", &[("sentence", sentence)]), move |_, _| {
                Rules::change(&store::path(), |list| list.put(rule.clone(), now())).and_then(|r| r).err().map(|e| worded(&e)).unwrap_or_default()
            });
        }
    }
}

/// One act of a rule undone (its JSON, as the list gave it), off the
/// window's thread: put back if it is still as the rule left it, said.
pub(crate) fn undo_act(qt: &QtThread, shared: &Arc<Shared>, given: &str) {
    let act = match serde_json::from_str::<Act>(given) {
        Ok(act) => act,
        Err(e) => return crate::backend::tell(qt, shared, say("rules-not-understood", &[("error", e.to_string())])),
    };
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    spawn("rules-undo", move || {
        let engine = engine_here(&load_config());
        let line = match engine::undo(&act, &engine.place, engine.places.as_ref(), &*engine.find_list, now_ms()) {
            Ok(undone) => tr().text(if undone.put_back { "rules-act-put-back" } else { "rules-act-kept" }, None),
            Err(e) => worded(&e),
        };
        crate::backend::tell(&qt, &shared, line);
    });
}

/// "Not for this one" on a line that waits: the rule leaves that object
/// alone from now on, on every device.
pub(crate) fn not_for_this_one(qt: &QtThread, shared: &Shared, rule: &str, object: &str) {
    let line = match engine::not_for_this_one(rule, object, &engine::Place::here(&device()), now_ms()) {
        Ok(()) => tr().text("rules-not-this-one-said", None),
        Err(e) => worded(&e),
    };
    crate::backend::tell(qt, shared, line);
}

// ---------------------------------------------------------------- starts

#[derive(Debug, Clone, Serialize)]
struct Start {
    key: String,
    subject: String,
    from: String,
    day: String,
    #[serde(skip)]
    date: i64,
}

/// The newest messages of the last 30 days the window knows, none of your own.
fn candidates(shared: &Shared, own: &[String]) -> Vec<Start> {
    let (now, since) = (now(), now() - i64::from(DAYS) * 86_400);
    let zone = sioul_core::zone::current();
    let mut found: Vec<Start> = crate::work::loaded_known(shared)
        .map(|loaded| {
            loaded
                .mail
                .iter()
                .filter(|(_, m)| m.date >= since && m.date <= now + 86_400 && !own.contains(&m.address))
                .map(|(_, m)| {
                    let day = jiff::Timestamp::from_second(m.date).map(|t| tr().day_month(t.to_zoned(zone.clone()).date())).unwrap_or_default();
                    Start { key: m.path.to_string_lossy().into_owned(), subject: m.subject.clone(), from: m.from.clone(), day, date: m.date }
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort_by_key(|s| std::cmp::Reverse(s.date));
    found.truncate(STARTS);
    found
}

/// The files of the messages a rule can see: judged, and not held by the Porch (`engine::held`).
fn seen_by_rules(judged: &[sioul_core::porch::Triaged]) -> BTreeSet<std::path::PathBuf> {
    judged.iter().filter(|t| !engine::held(t)).filter_map(|t| t.card.path.clone()).collect()
}

/// Of the messages found, the ones a rule can see, in their order.
fn admitted(found: Vec<Start>, seen: &BTreeSet<std::path::PathBuf>) -> Vec<Start> {
    found.into_iter().filter(|s| seen.contains(std::path::Path::new(&s.key))).collect()
}

/// What "Add a rule…" offers to start from, found off the window's thread
/// (`rules_starts_found`): the newest messages of the last 30 days a rule
/// can see; the presets the engine runs now, by their sentences.
pub(crate) fn starts(qt: &QtThread, shared: &Arc<Shared>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    spawn("rules-starts", move || {
        let config = load_config();
        let own = sioul_core::porch::own_addresses(&config.mail_sources());
        let found = candidates(&shared, &own);
        let files: Vec<std::path::PathBuf> = found.iter().map(|s| std::path::PathBuf::from(&s.key)).collect();
        let judged = engine_here(&config).judge(&files);
        let messages = admitted(found, &seen_by_rules(&judged));
        let presets: Vec<serde_json::Value> = declare::presets()
            .into_iter()
            .filter(|p| p.offered)
            .map(|p| serde_json::json!({ "id": p.id, "sentence": tr().own(&format!("rule-preset-{}", p.id)).unwrap_or_default() }))
            .collect();
        let json = json(&serde_json::json!({ "messages": messages, "presets": presets }));
        let _ = qt.queue(move |mut sioul| sioul.as_mut().rules_starts_found(QString::from(&json)));
    });
}

/// Whether a rule can start from this message (its file): none for what
/// the Porch holds, which no rule sees. Asked when a message's menu opens.
pub(crate) fn offered(key: &str) -> bool {
    !key.is_empty() && declare::example(std::path::Path::new(key), &load_config()).is_some()
}

/// A draft rule from a message (its file, as the reader names it): its
/// "when" filled from the message, as JSON; {error} when the message is
/// gone or held.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn act(at: i64, rule: &str, object: &str, result: &str) -> Act {
        Act { at, rule: rule.into(), object: object.into(), act: "paper".into(), result: result.into(), wrote: true, depth: 1, ..Act::default() }
    }

    /// One state per result: Undo of one of two papers takes back that one
    /// alone; "not for this one" changes nothing of what was made; a paper
    /// the antivirus took out reads so; a refusal waits, the one line with
    /// "Not for this one"; Undo only where the rule wrote and nothing since.
    #[test]
    fn the_history_says_each_result_as_it_stands() {
        let first = act(1, "r", "mid:a@x", "sioul:paper/p1");
        let second = act(2, "r", "mid:a@x", "sioul:paper/p2");
        let found = Act { wrote: false, ..act(3, "r", "mid:b@x", "sioul:paper/p3") };
        let other = act(4, "r", "mid:c@x", "sioul:paper/p4");
        let lines = vec![
            first.clone(),
            second.clone(),
            found.clone(),
            other.clone(),
            // Undo of the first paper alone.
            Act { at: 5, rule: "r".into(), object: "mid:a@x".into(), result: "sioul:paper/p1".into(), undone: true, back: true, ..Act::default() },
            // "Not for this one" on the third message: what was found stays as it is.
            Act { at: 6, rule: "r".into(), object: "mid:b@x".into(), undone: true, ..Act::default() },
            // The antivirus took the fourth paper out on another device.
            Act { at: 7, object: "sioul:paper/p4".into(), act: "paper".into(), result: "sioul:paper/p4".into(), refused: "rules-refused-infected".into(), ..Act::default() },
            // A refusal that may pass.
            Act { at: 8, rule: "r".into(), object: "mid:d@x".into(), act: "tell".into(), pending: "rules-refused-moment".into(), ..Act::default() },
        ];
        let acts = Acts::of(lines);
        let states: Vec<(String, State)> = history(&acts, "r", 9).into_iter().map(|(a, s)| (if a.result.is_empty() { a.object } else { a.result }, s)).collect();
        assert_eq!(
            states,
            vec![
                ("mid:d@x".to_string(), State::Waiting),
                ("sioul:paper/p4".to_string(), State::Out),
                ("sioul:paper/p3".to_string(), State::Done),
                ("sioul:paper/p2".to_string(), State::Done),
                ("sioul:paper/p1".to_string(), State::Back),
            ]
        );
        let offered: Vec<(bool, bool)> = history(&acts, "r", 9).into_iter().map(|(a, s)| (s == State::Done && a.wrote, s == State::Waiting)).collect();
        assert_eq!(offered, vec![(false, true), (false, false), (false, false), (true, false), (false, false)], "Undo only where the rule wrote; Not for this one only where it waits");
    }

    /// What the editor sends, as RuleEditor.qml writes it, reads as a rule
    /// of the store for each action of phase 1, its details left unset:
    /// no problem once its required details are there, and a sentence.
    #[test]
    fn the_editor_s_json_is_a_rule_for_each_action() {
        let tr = sioul_core::i18n::Translator::new("en");
        for (kind, action, with) in [
            ("message", "tie-to-project", serde_json::json!({ "project": "taxes" })),
            ("message", "give-topic", serde_json::json!({ "topic": "bills" })),
            ("message", "keep-attachments", serde_json::json!({})),
            ("task", "make", serde_json::json!({ "model": "pay", "title": { "from": "subject" }, "days": 3 })),
            ("none", "tell", serde_json::json!({ "level": "quiet" })),
        ] {
            let sent = serde_json::json!({ "id": "", "on": true, "when": { "type": "message", "event": "arrives", "match": "all", "if": [{ "field": "sender-domain", "test": "is", "value": ["example.org"] }] }, "then": { "type": kind, "action": action, "with": with } });
            let rule = parsed(&sent.to_string()).unwrap_or_else(|e| panic!("{action}: {e}"));
            assert_eq!(rule.problem(), None, "{action}");
            let sentence = words::sentence(&rule, &words::Names::default(), &tr);
            assert!(sentence.starts_with("When a message arrives from anyone at example.org: ") && !sentence.contains("rule-"), "{action}: {sentence}");
        }
        // A null the editor must never send is refused in words, not in serde's English alone.
        let null = serde_json::json!({ "when": { "type": "message", "event": "arrives", "if": [] }, "then": { "type": "task", "action": "make", "with": { "days": null } } });
        assert!(parsed(&null.to_string()).is_err());
        let typed = parsed(&serde_json::json!({ "when": { "type": "message", "event": "arrives", "if": [{ "field": "sender-domain", "test": "is", "value": ["bob@example.org"] }] }, "then": { "type": "message", "action": "tie-to-project", "with": { "project": "taxes" } } }).to_string()).unwrap();
        assert_eq!(typed.problem(), Some("rules-problem-domain"), "an address under a domain is refused");
    }

    /// The starts leave out what the Porch holds: only messages judged and
    /// not held stay, in their order.
    #[test]
    fn held_mail_is_never_a_start() {
        let start = |key: &str, date: i64| Start { key: key.into(), subject: String::new(), from: String::new(), day: String::new(), date };
        let found = vec![start("/m/cur/1", 3), start("/m/cur/2", 2), start("/m/cur/3", 1)];
        assert!(admitted(found.clone(), &BTreeSet::new()).is_empty(), "nothing judged: nothing offered");
        let seen: BTreeSet<std::path::PathBuf> = ["/m/cur/1", "/m/cur/3"].into_iter().map(std::path::PathBuf::from).collect();
        let kept: Vec<String> = admitted(found, &seen).into_iter().map(|s| s.key).collect();
        assert_eq!(kept, ["/m/cur/1", "/m/cur/3"], "the held one left out, the order kept");
    }
}

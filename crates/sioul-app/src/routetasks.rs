// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Routes that make tasks, in the window (docs/notes-folder.md, "Routes that
//! make tasks"): the tasks made of mail as it arrives (`arrivals`, which the
//! core's one function makes on every device), each said once in the status line (`said`); a project's models as its routes' lines say
//! them (`models`); the form of a route that makes a task of each message,
//! opened from a project's route or from a message ("Make a task of mail
//! like this…": `form`, `save`); and "Also for the 12 messages already here"
//! (`also`).

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use serde::{Deserialize, Serialize};
use sioul_core::links::LocalLinks;
use sioul_core::porch::Gate;
use sioul_core::projects::{self, Model, ModelValue, ProjectStore, RouteValue};
use sioul_core::routetasks::{self, Making, Outcome, Workdays};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

/// The projects as this device keeps them, their mail ties aside.
fn store() -> Option<ProjectStore> {
    load_config().places().and_then(|places| ProjectStore::open(&places).ok())
}

/// `f` run with what this device needs to make tasks (`routetasks::Making`):
/// the tasks the window knows (none read here: the ties and the files say
/// the rest), the ties as `links.toml` holds them now.
fn with_making<R>(shared: &Shared, f: impl FnOnce(&Making) -> R) -> R {
    let config = load_config();
    let own = sioul_core::porch::own_addresses(&config.mail_sources());
    let lists = |model: &Model| routetasks::list_for(model, &config);
    let workdays = Workdays::of(&config);
    let uids: BTreeSet<String> = crate::work::loaded_known(shared).map(|l| l.tasks.iter().map(|t| t.uid.clone()).collect()).unwrap_or_default();
    let known = |uid: &str| uids.contains(uid);
    let ties = LocalLinks::load(&LocalLinks::default_path());
    let zone = sioul_core::zone::current();
    let now = sioul_core::zone::now();
    f(&Making { config: &config, own: &own, lists: &lists, workdays: &workdays, known: &known, ties: &ties, zone: &zone, now: &now })
}

/// What was made, said once: one task by its title, its list and its day;
/// several by how many. Their accounts told, the plan made again.
pub(crate) fn said(qt: &QtThread, shared: &Arc<Shared>, out: &Outcome) {
    for problem in &out.problems {
        eprintln!("sioul: routetasks: {problem}");
    }
    let Some(first) = out.made.first() else { return };
    let accounts: BTreeSet<&str> = out.made.iter().map(|m| m.account.as_str()).collect();
    for account in accounts {
        crate::pim::nudge(shared, account);
    }
    let today = sioul_core::zone::now().date();
    let line = if out.made.len() == 1 {
        say("routetasks-made", &[("title", first.title.clone()), ("list", first.list.clone()), ("day", tr().day_in(first.due, today))])
    } else {
        let mut args = tr().counted(out.made.len());
        args.set("list", first.list.clone());
        tr().text("routetasks-made-many", Some(&args))
    };
    tell(qt, shared, line);
    crate::work::show_work(qt, shared);
}

/// A model as a route's line says it.
#[derive(Debug, Clone, Default, Serialize)]
struct ModelLine {
    id: String,
    name: String,
    /// "Asked 3 business days after the mail · About 15 minutes · Uses 1 Stamps · in Work".
    summary: String,
    /// The number of business days is not said (a file written by hand).
    missing: bool,
}

/// What a model makes, in words.
fn summary(model: &Model) -> String {
    let mut parts = Vec::new();
    parts.push(match model.business_days {
        Some(n) => {
            let mut args = tr().counted(usize::try_from(n).unwrap_or(0));
            args.set("n", n);
            tr().text("routetasks-days", Some(&args))
        }
        None => tr().text("routetasks-days-missing", None),
    });
    if let Some(minutes) = model.estimate {
        parts.push(sioul_core::taskview::estimate_text(tr(), minutes));
    }
    if !model.supplies.is_empty() {
        let list = load_config().records_root().and_then(|root| sioul_core::supplies::List::load(&root).ok()).unwrap_or_default();
        let what: Vec<String> = model.supplies.iter().map(|u| sioul_core::supplies::named(u, &list, tr())).collect();
        parts.push(say("supplies-task-uses", &[("what", what.join(", "))]));
    }
    let config = load_config();
    let list = routetasks::list_for(model, &config).map(|c| c.name).unwrap_or_default();
    if !list.is_empty() {
        parts.push(say("routetasks-in-list", &[("list", list)]));
    }
    parts.join(" · ")
}

/// A project's models, for its routes' lines (`SettingRow.qml`), as JSON.
pub(crate) fn models(project: &str) -> String {
    let Some(store) = store() else { return "[]".into() };
    let Some(project) = store.get(project) else { return "[]".into() };
    let lines: Vec<ModelLine> = project.models.iter().map(|m| ModelLine { id: m.id.clone(), name: m.name.clone(), summary: summary(m), missing: m.business_days.is_none() }).collect();
    json(&lines)
}

#[derive(Debug, Clone, Default, Serialize)]
struct Choice {
    id: String,
    title: String,
}

/// A supply to choose, as `SupplyLines.qml` takes them.
#[derive(Debug, Clone, Default, Serialize)]
struct SupplyChoice {
    id: String,
    name: String,
    unit: String,
}

/// The form of a route that makes a task of each message (`RouteTask.qml`).
#[derive(Debug, Default, Serialize)]
struct Form {
    /// Every project, to choose the one a message's route goes in.
    projects: Vec<Choice>,
    /// The project chosen: the route's, else the message's own; "" for none yet.
    project: String,
    /// The route: its id when it is one of the project's, else its words from the message.
    route: RouteValue,
    /// An existing route's words in one line ("From shop@example.org · Words in the subject: New sale").
    route_line: String,
    /// The model: the route's, else a new one (its business days filled with 3).
    model: ModelValue,
    /// The project's models, to make this route's tasks the same way as another's.
    models: Vec<ModelValue>,
    /// The task lists that can be written, "account/id"; the first, "", the usual one.
    lists: Vec<Choice>,
    /// The supplies kept (not archived): whether there are any, and each.
    keeps: bool,
    supplies: Vec<SupplyChoice>,
    /// The message's subject, for the title the route would give its task.
    subject: String,
    title: String,
    /// What went wrong: the message no longer here.
    error: String,
}

/// A route's words in one line, as its form shows an existing one.
fn route_line(route: &RouteValue) -> String {
    let mut parts = Vec::new();
    for (key, list) in [("set-route-domains", &route.from_domains), ("set-route-addresses", &route.from_addresses), ("set-route-subject", &route.subject_contains), ("set-route-text", &route.text_contains), ("set-route-attachments", &route.attachment_contains)] {
        if !list.is_empty() {
            parts.push(format!("{}: {}", tr().text(key, None), list.join(", ")));
        }
    }
    parts.join(" · ")
}

/// The form, as JSON: for a project's route (`project`, `route`: the route
/// as its row has it, JSON, found by its id, else by its words), or for a
/// message (`key`: its file), filled with its sender and the words between
/// brackets in its subject, in its project when it has one.
pub(crate) fn form(project: &str, route: &str, key: &str) -> String {
    let route: RouteValue = serde_json::from_str(route).unwrap_or_default();
    let kept = store();
    let config = load_config();
    let mut form = Form::default();
    if let Some(kept) = &kept {
        form.projects = kept.projects.iter().filter(|p| p.status.as_deref() != Some("closed")).map(|p| Choice { id: p.id.clone(), title: p.title.clone() }).collect();
    }
    if !key.is_empty() {
        let Some(card) = sioul_core::maildir::read_one(Path::new(key)) else {
            form.error = tr().text("mail-find-gone", None);
            return json(&form);
        };
        form.subject.clone_from(&card.subject);
        form.route = RouteValue { from_addresses: card.from_address.iter().map(|a| a.trim().to_lowercase()).collect(), subject_contains: routetasks::bracketed(&card.subject), ..RouteValue::default() };
        // Its project: a route of one takes it, else it is tied to one.
        let ties = LocalLinks::load(&LocalLinks::default_path());
        form.project = kept.clone().map(|s| s.with_ties(&ties)).and_then(|s| s.route(&card).first().map(|r| r.project.id.clone())).unwrap_or_default();
        // A route of the same words there already: that one, with what it makes.
        if let Some(same) = kept.as_ref().and_then(|s| s.get(&form.project)).and_then(|p| p.routes.iter().map(RouteValue::from).find(|r| r.same_words(&form.route))) {
            form.route = same;
        }
    } else {
        form.project = project.to_string();
        let found = kept.as_ref().and_then(|s| s.get(project)).and_then(|p| p.routes.iter().map(RouteValue::from).find(|r| (!route.id.is_empty() && r.id == route.id) || (route.id.is_empty() && r.same_words(&route))));
        form.route = found.unwrap_or(route);
        form.route_line = route_line(&form.route);
    }
    let chosen = kept.as_ref().and_then(|s| s.get(&form.project).cloned());
    if let Some(project) = &chosen {
        form.models = project.models.iter().map(ModelValue::from).collect();
        form.model = project.model(&form.route.makes).map(ModelValue::from).unwrap_or_default();
    }
    if form.model.id.is_empty() {
        form.model.business_days = routetasks::BUSINESS_DAYS;
    }
    form.lists = std::iter::once(Choice { id: String::new(), title: tr().text("routetasks-list-usual", None) })
        .chain(sioul_core::tasks::lists().into_iter().filter(|c| !c.read_only).map(|c| Choice { id: format!("{}/{}", c.account, c.id), title: c.name }))
        .collect();
    let supplies = config.records_root().and_then(|root| sioul_core::supplies::List::load(&root).ok()).unwrap_or_default();
    form.keeps = !supplies.supplies.is_empty();
    form.supplies = supplies.supplies.iter().filter(|s| !s.archived).map(|s| SupplyChoice { id: s.id.clone(), name: s.name_shown().to_string(), unit: s.unit.clone() }).collect();
    form.title = routetasks::title(&form.subject, &form.route.subject_contains, &form.model.prefix, &form.model.name);
    json(&form)
}

/// The title a route of these words of the subject (JSON list) would give a
/// message's task, the prefix in front.
pub(crate) fn title(subject: &str, words: &str, prefix: &str) -> String {
    let words: Vec<String> = serde_json::from_str(words).unwrap_or_default();
    routetasks::title(subject, &words, prefix, "")
}

/// The form as the window sends it back.
#[derive(Debug, Default, Deserialize)]
struct Sent {
    #[serde(default)]
    project: String,
    /// A new project's title, when the form makes one.
    #[serde(default)]
    new_project: String,
    #[serde(default)]
    route: RouteValue,
    #[serde(default)]
    model: ModelValue,
}

/// What a save says back.
#[derive(Debug, Default, Serialize)]
struct Saved {
    project: String,
    route: String,
    model: String,
    /// The messages already here that this route takes and that made no task yet.
    already: usize,
    error: String,
}

/// The form saved: a new project first when it names one; then the model
/// and the route, in one write (`projects::save_route_task`). Says how many
/// messages already here the route takes, for "Also for…".
pub(crate) fn save(shared: &Arc<Shared>, sent: &str) -> String {
    let fail = |error: String| json(&Saved { error, ..Saved::default() });
    let sent: Sent = match serde_json::from_str(sent) {
        Ok(sent) => sent,
        Err(e) => return fail(e.to_string()),
    };
    let Some(root) = load_config().records_root() else { return fail(tr().text("error-no-store", None)) };
    let file = projects::file_in(&root);
    let mut project = sent.project.trim().to_string();
    if project.is_empty() {
        if sent.new_project.trim().is_empty() {
            return fail(tr().text("routetasks-project-needed", None));
        }
        project = match projects::save_project(&file, "", &projects::ProjectEdit { title: sent.new_project.trim().to_string(), ..projects::ProjectEdit::default() }) {
            Ok(id) => id,
            Err(e) => return fail(e),
        };
    }
    let mut model = sent.model;
    if model.name.trim().is_empty() {
        model.name = store().and_then(|s| s.get(&project).map(|p| p.title.clone())).unwrap_or_default();
    }
    let saved = match projects::save_route_task(&file, &project, &sent.route, &model) {
        Ok(saved) => saved,
        Err(e) => return fail(e),
    };
    let already = already(shared, &saved.project, &saved.route).len();
    json(&Saved { project: saved.project, route: saved.route, model: saved.model, already, error: String::new() })
}

/// The messages already here that a project's route takes, as the Porch
/// admits them, that made no task yet (`routetasks::already_here`).
fn already(shared: &Shared, project: &str, route: &str) -> Vec<sioul_core::card::Card> {
    let Some(store) = store() else { return Vec::new() };
    let gate = Gate::load(&load_config(), sioul_core::zone::now().timestamp().as_second());
    let loaded = crate::work::loaded(shared);
    with_making(shared, |making| routetasks::already_here(&gate, &store, project, route, &loaded.mail, making))
}

/// "Also for the 12 messages already here": their tasks made, once, on a
/// thread; said in the status line. Returns what went wrong, else "".
pub(crate) fn also(qt: &QtThread, shared: &Arc<Shared>, project: &str, route: &str) -> String {
    let (qt, shared, project, route) = (qt.clone(), Arc::clone(shared), project.to_string(), route.to_string());
    std::thread::spawn(move || {
        let cards = already(&shared, &project, &route);
        let Some(store) = store() else { return };
        let out = with_making(&shared, |making| routetasks::for_already_here(&cards, &store, &project, &route, making, &LocalLinks::default_path()));
        said(&qt, &shared, &out);
    });
    String::new()
}

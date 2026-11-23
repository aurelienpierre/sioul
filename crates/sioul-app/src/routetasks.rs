// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Routes that make tasks, in the window (docs/notes-folder.md, "Routes that
//! make tasks"): the tasks made of mail as it arrives (`arrived`), each said
//! once in the status line.

use crate::backend::{QtThread, Shared, load_config, say, tell, tr};
use sioul_core::links::LocalLinks;
use sioul_core::porch::Gate;
use sioul_core::projects::{Model, ProjectStore};
use sioul_core::routetasks::{self, Making, Outcome, Workdays};
use std::collections::BTreeSet;
use std::path::PathBuf;
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

/// Mail just fetched (its files), after its ties to projects: the tasks its
/// routes make, each once, said in the status line; their accounts told and
/// the plan made again. Nothing at an address's first fetch (`first`): that
/// is mail already here, which makes nothing unless asked.
pub(crate) fn arrived(qt: &QtThread, shared: &Arc<Shared>, files: &[PathBuf], first: bool) {
    if first || files.is_empty() {
        return;
    }
    let Some(store) = store() else { return };
    if !routetasks::any(&store) {
        return;
    }
    let gate = Gate::load(&load_config(), sioul_core::zone::now().timestamp().as_second());
    let out = with_making(shared, |making| routetasks::on_arrival(&gate, &store, files, making, &LocalLinks::default_path()));
    said(qt, shared, &out);
}

/// What was made, said once: one task by its title, its list and its day;
/// several by how many. Their accounts told, the plan made again.
fn said(qt: &QtThread, shared: &Arc<Shared>, out: &Outcome) {
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

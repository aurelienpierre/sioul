// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail arrives (docs/rules.md, "Where rules run"): the one function the
//! window's arrival chain, the phone's background step and `sioul watch`
//! call with what a fetch brought, so that a phone fetching alone does what a
//! computer does (the inventory's A.0: until phase 1, the phone's step tied
//! nothing to projects and made no task):
//!
//! 1. **projects' ties**: each message the Porch places in a project's lane
//!    (its routes, its conversation) tied to it in `links.toml`
//!    (`how = "project"`), only the ties this adds, under the file's lock;
//! 2. **route tasks** (`routetasks::on_arrival`), never at a first fetch;
//! 3. **the rules** (`engine::Engine::on_arrival`), never at a first fetch.
//!
//! Each writes only in Sioul's own folders, under each file's lock, which
//! every process of the device shares (the window, the phone's background
//! process, the command line): `links.toml`, the task lists' files (pushed to
//! their server at the window's next sync of that account), the topics' log,
//! the papers' list and folder, the record of acts. What the caller says of
//! it (the status line, the account's nudge, a notice) is the caller's.

use crate::config::Config;
use crate::i18n::Translator;
use crate::links::{Edge, LocalLinks};
use crate::porch::{Lane, SenderList, Senders};
use crate::projects::ProjectStore;
use crate::routetasks::{self, Workdays};
use crate::wiring::engine::{Engine, Outcome, Scanned};
use crate::wiring::store::{self, Rules};
use std::path::{Path, PathBuf};

/// What the caller says of where it runs.
pub struct Here<'a> {
    pub config: &'a Config,
    /// This device's name in the sharing.
    pub device: &'a str,
    /// This device keeps the notices: it tells.
    pub tells_here: bool,
    /// The tasks this device knows by UID (the window's); else none.
    pub known: &'a dyn Fn(&str) -> bool,
    /// The antivirus, for the files a rule keeps (sioul-sync's `antivirus::for_rules`).
    pub scan: &'a dyn Fn(&Path) -> Scanned,
    pub tr: &'a Translator,
}

/// What arrived mail made.
#[derive(Debug, Default)]
pub struct Arrived {
    /// The ties to projects added.
    pub ties: Vec<Edge>,
    pub route_tasks: routetasks::Outcome,
    pub rules: Outcome,
}

/// The ties, the route tasks and the rules of mail just fetched (its
/// files; `first`: an address's first fetch).
pub fn arrived(here: &Here, files: &[PathBuf], first: bool) -> Arrived {
    let mut out = Arrived::default();
    if files.is_empty() {
        return out;
    }
    let config = here.config;
    let now = crate::zone::now();
    let stamp = now.timestamp().as_second();
    let ties_path = LocalLinks::default_path();
    let ties = LocalLinks::load(&ties_path);
    let store = config.places().and_then(|places| ProjectStore::open(&places).ok()).map(|s| s.with_ties(&ties));
    let senders = Senders::load(config);
    let rules = Rules::load(&store::path());
    let runs = rules.as_ref().is_ok_and(|r| r.running().next().is_some());
    // Judged once, with your spam filter's verdict, for the ties and the rules.
    let sources = config.mail_sources();
    let judged = crate::porch::judge(files, &sources, store.as_ref(), &SenderList::default(), &senders, stamp);
    out.ties = project_ties(&judged, &ties);
    if !out.ties.is_empty()
        && let Err(e) = LocalLinks::add_all(&ties_path, &out.ties)
    {
        out.rules.problems.push(e);
        out.ties.clear();
    }
    if first {
        return out;
    }
    if let Some(store) = store.as_ref().filter(|s| routetasks::any(s)) {
        let gate = crate::porch::Gate::load(config, stamp);
        let own = crate::porch::own_addresses(&sources);
        let lists = |model: &crate::projects::Model| routetasks::list_for(model, config);
        let workdays = Workdays::of(config);
        let ties = LocalLinks::load(&ties_path);
        let zone = now.time_zone().clone();
        let making = routetasks::Making { config, own: &own, lists: &lists, workdays: &workdays, known: here.known, ties: &ties, zone: &zone, now: &now };
        out.route_tasks = routetasks::on_arrival(&gate, store, files, &making, &ties_path);
    }
    match rules {
        Ok(rules) if runs => {
            let mut engine = Engine::of(config, here.device, &now, rules, store, senders, Vec::new());
            engine.tells_here = here.tells_here;
            engine.known = Box::new(|uid: &str| (here.known)(uid));
            engine.scan = Box::new(|file: &Path| (here.scan)(file));
            out.rules = engine.on_arrival(&judged, first, here.tr);
        }
        Ok(_) => {}
        Err(e) => out.rules.problems.push(e),
    }
    out
}

/// The ties to projects mail judged by the Porch takes: one per message in
/// a project's lane, none that `ties` holds already.
pub fn project_ties(judged: &[crate::porch::Triaged], ties: &LocalLinks) -> Vec<Edge> {
    let mut out: Vec<Edge> = Vec::new();
    for t in judged {
        let Lane::Project(id) = &t.lane else { continue };
        let Some((_, address)) = routetasks::message_of(&t.card) else { continue };
        let edge = Edge { from: address, to: crate::links::project_uri(id), how: "project".into() };
        if !ties.links.contains(&edge) && !out.contains(&edge) {
            out.push(edge);
        }
    }
    out
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What arrived mail makes, in the window and in the phone's background step
//! alike (docs/rules.md, "Where rules run"): the one core function
//! (`sioul_core::wiring::arrival::arrived`) ties mail to its projects, makes
//! the tasks routes make, and runs your rules; here, what each place says of
//! it. The window says what was made in the status line with Undo, tells the
//! accounts whose lists got a task, and posts a rule's notice where this
//! device keeps the notices; the phone's background step, a process without
//! the window, posts a rule's notice through Notices.java the same way and
//! says nothing else: what it did is in the rules' history, with Undo.

use crate::backend::{QtThread, Shared, load_config, tr};
use sioul_core::wiring::arrival::{self, Arrived, Here};
use sioul_core::wiring::engine::{self, Place};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// This device's name in the sharing.
fn device() -> String {
    sioul_sync::share::Here::load(&sioul_core::config::state_dir()).id
}

/// Whether this device keeps the notices (the "notices" lease): it tells.
fn keeps_notices() -> bool {
    crate::share::looked("notices", sioul_sync::lease::Rule::FollowsYou).mine
}

/// The antivirus, for the files a rule keeps as papers.
fn scan(file: &Path) -> engine::Scanned {
    sioul_sync::antivirus::for_rules(file, crate::attachments::phone())
}

/// Mail the window fetched (`backend::reported`): its ties, route tasks
/// and rules, each said once; nothing but the ties at an address's first fetch.
pub(crate) fn in_window(qt: &QtThread, shared: &Arc<Shared>, files: &[PathBuf], first: bool) {
    if files.is_empty() {
        return;
    }
    let config = load_config();
    let uids: BTreeSet<String> = crate::work::loaded_known(shared).map(|l| l.tasks.iter().map(|t| t.uid.clone()).collect()).unwrap_or_default();
    let known = |uid: &str| uids.contains(uid);
    let device = device();
    let here = Here { config: &config, device: &device, tells_here: keeps_notices(), known: &known, scan: &scan, tr: tr() };
    let out = arrival::arrived(&here, files, first);
    crate::routetasks::said(qt, shared, &out.route_tasks);
    said(qt, shared, &out, &device);
}

/// Mail the phone's background step fetched (`steps::fetch_mail`): the same,
/// a rule's notice posted as a notice to know when this phone keeps the
/// notices; nothing said otherwise (the window, when it opens, reads what
/// was made, and the rules' history holds each act with its Undo).
pub(crate) fn in_background(files: &[PathBuf], first: bool) {
    if files.is_empty() {
        return;
    }
    let config = load_config();
    let device = device();
    let here = Here { config: &config, device: &device, tells_here: keeps_notices(), known: &|_| false, scan: &scan, tr: tr() };
    let out = arrival::arrived(&here, files, first);
    for problem in out.route_tasks.problems.iter().chain(&out.rules.problems) {
        eprintln!("sioul: arrivals: {problem}");
    }
    for notice in out.rules.notices {
        crate::eventalarms::day_note(&notice.key(), &notice);
    }
}

/// What the rules did, said once: the line of each act (`words::act_line`),
/// one Undo for them all, which puts each back while it is as the rule left
/// it and says "not for this one" either way; the accounts whose lists got a
/// task told, the plan made again; a rule's notice told as the moment allows.
fn said(qt: &QtThread, shared: &Arc<Shared>, out: &Arrived, device: &str) {
    let rules = &out.rules;
    for problem in &rules.problems {
        eprintln!("sioul: rules: {problem}");
    }
    for notice in &rules.notices {
        if cfg!(target_os = "android") {
            crate::eventalarms::day_note(&notice.key(), notice);
        } else {
            crate::tell::tell(notice.clone());
        }
    }
    let done: Vec<engine::Done> = rules.done.iter().filter(|d| d.act.wrote).cloned().collect();
    if done.is_empty() {
        return;
    }
    let accounts: BTreeSet<String> = done.iter().flat_map(|d| d.said.files.iter().filter_map(|f| crate::pim::account_of(&f.path))).collect();
    for account in &accounts {
        crate::pim::nudge(shared, account);
    }
    let line = done.iter().map(|d| d.line.clone()).collect::<Vec<_>>().join(" ");
    let place = Place::here(device);
    crate::mail::offer_back(qt, shared, line, move |qt, shared| {
        let places = load_config().places();
        let find = |list: &str| sioul_core::tasks::lists().into_iter().find(|c| format!("{}/{}", c.account, c.id) == list);
        let now = jiff::Timestamp::now().as_millisecond();
        let mut changed = false;
        for d in &done {
            match engine::undo(&d.act, &place, places.as_ref(), &find, now) {
                Ok(undone) => changed |= !undone.put_back,
                Err(e) => {
                    eprintln!("sioul: rules: {e}");
                    changed = true;
                }
            }
        }
        for account in &accounts {
            crate::pim::nudge(shared, account);
        }
        crate::work::show_work(qt, shared);
        if changed { tr().text("undo-too-late", None) } else { String::new() }
    });
    crate::work::show_work(qt, shared);
}

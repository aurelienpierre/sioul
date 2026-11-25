// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What arrived mail makes, in the window and in the phone's background step
//! alike (docs/rules.md, "Where rules run"): the one core function
//! (`sioul_core::wiring::arrival::arrived`) ties mail to its projects, makes
//! the tasks routes make, and runs your rules; here, what each place says of
//! it. The window says what was made in the status line with Undo, what a
//! rule could not do once per rule, and what an antivirus took out of the
//! papers; it tells the accounts whose lists got a task, and posts a rule's
//! notice where this device keeps the notices. The phone's background step,
//! a process without the window, runs them after its exchange, posts a
//! rule's notice through Notices.java as the moment allows, and says nothing
//! else: what it did is in the rules' history, with Undo.
//!
//! Undo is a state every device comes to (`Engine::settle`): when the
//! rules' record or the papers come from another device, and at each phone
//! step, what this device wrote that an Undo elsewhere took back is taken
//! back here once, even when no rule is left; the pending acts are tried
//! again (a phone's notice that waited for its moment among them), and the
//! papers kept unchecked are read where an antivirus can be asked. When you
//! let a sender in, the rules run on what they sent this week, which the
//! screener held.

use crate::backend::{QtThread, Shared, load_config, tr};
use sioul_core::wiring::arrival::{self, Here};
use sioul_core::notice::Notice;
use sioul_core::wiring::engine::{self, Engine, Outcome, Place};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// Whether an antivirus can be asked here at all: never on a phone.
fn can_scan() -> bool {
    !crate::attachments::phone() && sioul_sync::antivirus::present()
}

/// What the phone's notices say of a rule's notice now.
fn decision(config: &sioul_core::config::Config, notice: &Notice) -> sioul_core::notice::Decision {
    let now = sioul_core::zone::now();
    let moment = sioul_core::notice::moment_from_files(config, &now, true);
    sioul_core::notice::decide(notice, &sioul_core::notice::told(None), moment, config.notices.minutes(), now.timestamp().as_second())
}

/// Whether a rule's notice may come now: on a phone, unless the moment
/// makes it wait (a pause, a do-not-disturb), when it stays pending and is
/// told at a later step; a computer's window holds a notice itself.
fn moment(config: &sioul_core::config::Config, notice: &Notice) -> bool {
    !cfg!(target_os = "android") || !matches!(decision(config, notice), sioul_core::notice::Decision::Wait(_))
}

/// The refusals said already in this run, by rule and reason: each once.
static SAID: Mutex<BTreeSet<(String, String)>> = Mutex::new(BTreeSet::new());

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
    let allows = |notice: &Notice| moment(&config, notice);
    let here = Here { config: &config, device: &device, tells_here: keeps_notices(), known: &known, scan: &scan, can_scan: can_scan(), moment: &allows, tr: tr() };
    let out = arrival::arrived(&here, files, first);
    crate::routetasks::said(qt, shared, &out.route_tasks);
    said(qt, shared, &out.rules, &device);
}

/// Mail the phone's background step fetched (`steps::fetch_mail`), after
/// the step's exchange: the same; a rule's notice posted as a notice to know
/// when this phone keeps the notices and the moment lets it come; nothing
/// said otherwise (the window, when it opens, reads what was made, and the
/// rules' history holds each act with its Undo).
pub(crate) fn in_background(files: &[PathBuf], first: bool) {
    if files.is_empty() {
        return;
    }
    let config = load_config();
    let device = device();
    let allows = |notice: &Notice| moment(&config, notice);
    let here = Here { config: &config, device: &device, tells_here: keeps_notices(), known: &|_| false, scan: &scan, can_scan: can_scan(), moment: &allows, tr: tr() };
    let out = arrival::arrived(&here, files, first);
    for problem in out.route_tasks.problems.iter().chain(&out.rules.problems) {
        eprintln!("sioul: arrivals: {problem}");
    }
    post_on_phone(&config, &out.rules);
}

/// A rule's notices on a phone: posted (Notices.java) as the moment lets a
/// notice to know come; one the moment made wait was not made at all (the
/// engine's `moment`: a pending line, told at a later step).
fn post_on_phone(config: &sioul_core::config::Config, out: &Outcome) {
    for notice in &out.notices {
        if let sioul_core::notice::Decision::Post(_) = decision(config, notice) {
            crate::eventalarms::day_note(&notice.key(), notice);
        }
    }
}

/// What a phone step settles of the rules when no window runs: what an
/// Undo elsewhere took back taken back here, the pending acts tried again
/// (a notice whose moment came), the papers kept unchecked read where an
/// antivirus can be asked (never on a phone). Nothing when there is no rule,
/// nothing to take back, and no paper to read.
pub(crate) fn settle_in_background() {
    let config = load_config();
    let Some(engine) = engine_here(&config) else { return };
    let out = engine.settle(tr());
    for problem in &out.problems {
        eprintln!("sioul: rules: {problem}");
    }
    post_on_phone(&config, &out);
}

/// The window's settling, when the rules' record or the papers came from
/// another device: as `settle_in_background`, said in the status line.
pub(crate) fn settle(qt: &QtThread, shared: &Arc<Shared>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let config = load_config();
        let Some(engine) = engine_here(&config) else { return };
        let out = engine.settle(tr());
        said(&qt, &shared, &out, &engine.place.device);
    });
}

/// The engine on this device, when it has something to settle: a rule;
/// an act an Undo elsewhere took back, still to take back here, even once
/// no rule is left; a paper kept unchecked, where an antivirus can be asked.
fn engine_here(config: &sioul_core::config::Config) -> Option<Engine<'static>> {
    let device = device();
    let rules = sioul_core::wiring::store::Rules::load(&sioul_core::wiring::store::path()).unwrap_or_default();
    let record = sioul_core::wiring::acts::Acts::read(&sioul_core::wiring::acts::folder());
    let can_scan = can_scan();
    let unchecked = || can_scan && config.places().and_then(|p| sioul_core::papers::Wallet::load_at(&p).ok()).is_some_and(|w| w.papers.iter().any(|p| p.unchecked));
    if !engine::has_work(&rules, &record, &device, unchecked) {
        return None;
    }
    let mut engine = Engine::here(config, &device, &sioul_core::zone::now());
    engine.tells_here = keeps_notices();
    engine.scan = Box::new(scan);
    engine.can_scan = can_scan;
    let config = config.clone();
    engine.moment = Box::new(move |notice: &Notice| moment(&config, notice));
    Some(engine)
}

/// A sender let in (the screener's "Let in"): the rules run on what they
/// sent this week, which the screener held, as if it arrived now; what came
/// before a rule stays as it is.
pub(crate) fn let_in(qt: &QtThread, shared: &Arc<Shared>, address: &str) {
    let (qt, shared, address) = (qt.clone(), Arc::clone(shared), address.trim().to_lowercase());
    std::thread::spawn(move || {
        let config = load_config();
        let index = sioul_core::mailindex::MailIndex::build_ranked(&sioul_core::mailindex::MailIndex::roots_of(&config));
        let since = sioul_core::zone::now().timestamp().as_second() - engine::NEWEST;
        let files: Vec<PathBuf> = index.iter().map(|(_, mail)| mail).filter(|mail| mail.address == address && mail.date >= since).map(|mail| mail.path.clone()).collect();
        in_window(&qt, &shared, &files, false);
    });
}

/// What the rules did, said once: the line of each act (`words::act_line`),
/// one Undo for them all, which puts each back while it is as the rule left
/// it and says "not for this one" either way; what a rule could not do,
/// once per rule and reason in this run; what an antivirus took out of the
/// papers; the accounts whose lists got a task told, the plan made again; a
/// rule's notice told as the moment allows.
fn said(qt: &QtThread, shared: &Arc<Shared>, rules: &Outcome, device: &str) {
    for problem in &rules.problems {
        eprintln!("sioul: rules: {problem}");
    }
    if cfg!(target_os = "android") {
        post_on_phone(&load_config(), rules);
    } else {
        for notice in &rules.notices {
            crate::tell::tell(notice.clone());
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for title in &rules.taken_out {
        lines.push(crate::backend::say("rules-paper-taken-out", &[("title", title.clone())]));
    }
    if let Ok(mut told) = SAID.lock() {
        for refused in &rules.refused {
            if told.insert((refused.rule.clone(), refused.why.to_string())) {
                lines.push(tr().text(refused.why, None));
            }
        }
    }
    // What was written, and a quiet tell, which writes nothing but is said.
    let done: Vec<engine::Done> = rules.done.iter().filter(|d| d.act.wrote || d.act.act == "tell").cloned().collect();
    if done.is_empty() {
        if !lines.is_empty() {
            crate::backend::tell(qt, shared, lines.join(" "));
        }
        return;
    }
    let accounts: BTreeSet<String> = done.iter().flat_map(|d| d.said.files.iter().filter_map(|f| crate::pim::account_of(&f.path))).collect();
    for account in &accounts {
        crate::pim::nudge(shared, account);
    }
    lines.splice(0..0, done.iter().map(|d| d.line.clone()));
    let place = Place::here(device);
    crate::mail::offer_back(qt, shared, lines.join(" "), move |qt, shared| {
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

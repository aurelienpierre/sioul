// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Extensions in the window (docs/extensions.md): Settings ▸ Extensions,
//! each extension's runs, and the sign-in page it asks for.
//!
//! - **The page** (`page`): what the signed manifest offers, each with what
//!   it brings, the sites it talks to and whether it keeps a sign-in, before
//!   anything is enabled; then Enable, Sign in…, its state and last problem
//!   in one sentence, Disable, Remove (keeping its agendas or not). Opening
//!   it reads the manifest; so does a day passed while one is enabled
//!   (`tick`); nothing else does.
//! - **An update** that asks for no more than the version in use (no other
//!   host, no new sign-in) is taken when the manifest is read; one that asks
//!   for more waits for the person's yes (`accept`).
//! - **The runs**: an extension enabled here and signed in refreshes when
//!   Sioul starts, then every three hours while the window runs (`tick`),
//!   and in the phone's background step (`background_step`); never two of
//!   the same at once; never on the window's thread.
//! - **The sign-in page** (`show_page`): on a computer the window's
//!   SignInWindow.qml, on Android SignIn.java; the address returned comes
//!   back through `returned`.
//! - **The agendas** an extension brings are taken in whole, through the
//!   door of read-only agendas (`AgendaDoor`), by the device whose fetch is
//!   the freshest (`availability::may_write`, `devices::mirror_writers`):
//!   they travel to the other devices as calendars kept on a device do.

use crate::backend::{QtThread, load_config, say, tr};
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use sioul_core::extensions::manifest::{Manifest, Offer};
use sioul_core::extensions::reply::{self, ProblemKind, RefreshReply, SignInReply};
use sioul_core::extensions::state::{self, Brought, Installed, State};
use sioul_sync::extensions::{self as store, Call, Real, SignInPage, engine::Outcome};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::SystemTime;

/// Every three hours.
const EVERY: i64 = 3 * 3600;
/// The manifest read once a day while an extension is enabled.
const DAILY: i64 = 24 * 3600;

/// The manifest last read here, and where from; or why it was not.
struct Listed {
    address: String,
    manifest: Option<Manifest>,
    problem: Option<String>,
}

static LISTED: Mutex<Option<Listed>> = Mutex::new(None);
/// The extensions running now, by id: never two calls of one at once.
static RUNNING: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
/// The extensions refreshed since this process started ("when Sioul starts").
static STARTED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
/// The manifest being read.
static READING: Mutex<bool> = Mutex::new(false);
/// When the window's minute last tried to read the manifest.
static TRIED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

fn french() -> bool {
    tr().language().starts_with("fr")
}

// ---------------------------------------------------------------- the agendas' door

/// The door of read-only agendas an extension writes through: one guarded
/// write that replaces an agenda whole. Until the subscribed calendars' door
/// is merged, `Waiting` stands for it and writes nothing.
pub(crate) trait AgendaDoor {
    /// The files the agenda in `folder` holds now, by name; none when it does not exist.
    fn files(&self, folder: &str) -> BTreeMap<String, String>;
    /// The read-only agenda in `folder`, named `name`, replaced whole by `files`, made if new; whether anything changed.
    fn replace(&self, folder: &str, name: &str, files: &BTreeMap<String, String>) -> Result<bool, String>;
    /// The agenda in `folder` taken away.
    fn remove(&self, folder: &str) -> Result<(), String>;
}

/// The door until the subscribed calendars' is merged: it writes nothing,
/// and says why (the extension's page shows it).
pub(crate) struct Waiting;

impl AgendaDoor for Waiting {
    fn files(&self, _: &str) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn replace(&self, _: &str, _: &str, _: &BTreeMap<String, String>) -> Result<bool, String> {
        Err("the read-only agendas' door is not in this build yet".into())
    }

    fn remove(&self, _: &str) -> Result<(), String> {
        Err("the read-only agendas' door is not in this build yet".into())
    }
}

fn door() -> Waiting {
    Waiting
}

/// What the freshest-view rule knows an extension's agendas by, in each device's sealed entry.
fn writer_key(id: &str) -> String {
    format!("extension:{id}")
}

/// Whether this device may write the agendas of `id` from a fetch made at
/// `fetched`: when no other device last wrote them from a later one.
fn may_write(id: &str, fetched: i64) -> bool {
    let others = crate::devices::entries().map(|(own, all)| sioul_sync::devices::mirror_writers(&all, &own, &writer_key(id), now())).unwrap_or_default();
    sioul_core::availability::may_write(Some((fetched, fetched)), others)
}

/// What this device wrote the agendas of `id` from, in its sealed entry.
fn note_written(id: &str, fetched: i64) {
    let Some((_, vault)) = crate::share::vault() else { return };
    let path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    let _ = sioul_sync::devices::change(&path, vault.as_ref().map(|(folder, key)| (folder.as_path(), key)), |entry| {
        entry.mirrors.insert(writer_key(id), fetched);
    });
}

/// A refresh's agendas taken in through `door` when `write`: each one's
/// files, what it could not read kept from the last copy. What each holds now.
pub(crate) fn take_in(door: &dyn AgendaDoor, extension: &str, reply: &RefreshReply, write: bool) -> Result<Vec<Brought>, String> {
    let mut brought = Vec::new();
    for agenda in &reply.agendas {
        let folder = sioul_core::extensions::agenda_folder(extension, &agenda.id);
        let files = reply::files_after(&agenda.events, &door.files(&folder), &agenda.unread);
        if write {
            door.replace(&folder, &agenda.name, &files)?;
        }
        brought.push(Brought { id: agenda.id.clone(), name: agenda.name.clone(), events: files.len() });
    }
    Ok(brought)
}

// ---------------------------------------------------------------- the page

/// The words of a problem kept (`Installed::problem`), in one sentence.
fn problem_line(word: &str, name: &str) -> String {
    let id = match word {
        "sign-in" => "extension-problem-sign-in",
        "unreachable" => "extension-problem-unreachable",
        "refused" => "extension-problem-refused",
        "changed" => "extension-problem-changed",
        "stopped-time" => "extension-stopped-time",
        "stopped-memory" => "extension-stopped-memory",
        "stopped-error" => "extension-stopped-error",
        "stopped-nonsense" => "extension-stopped-nonsense",
        "stopped-asks-more" => "extension-stopped-asks-more",
        "stopped-interface" => "extension-stopped-interface",
        "stopped-unread" => "extension-stopped-unread",
        "door" => "extension-problem-door",
        "keyring" => "extension-problem-keyring",
        _ => "extension-problem-other",
    };
    say(id, &[("name", name.to_string())])
}

fn when(at: i64) -> String {
    jiff::Timestamp::from_second(at).map(|t| tr().date(&t.to_zoned(sioul_core::zone::current()), true)).unwrap_or_default()
}

/// An offer, and this device's record of it, for ExtensionsTab.qml.
fn offer_json(offer: &Offer, installed: Option<&Installed>, running: bool) -> Value {
    let fr = french();
    let name = offer.name.said(fr).to_string();
    let hosts: Vec<&str> = offer.hosts.iter().map(String::as_str).collect();
    let mut line = String::new();
    let mut agendas = Vec::new();
    if let Some(installed) = installed {
        line = if installed.last_run == 0 {
            say("extension-never-ran", &[])
        } else if installed.agendas.is_empty() {
            say("extension-ran", &[("when", when(installed.last_run))])
        } else {
            let events: usize = installed.agendas.iter().map(|a| a.events).sum();
            say("extension-brought", &[("when", when(installed.last_brought.max(installed.last_run))), ("agendas", installed.agendas.len().to_string()), ("events", events.to_string())])
        };
        agendas = installed.agendas.iter().map(|a| json!({ "id": a.id, "name": a.name })).collect();
    }
    let waiting = installed.and_then(|i| i.waiting.as_ref()).map(|w| {
        let added: Vec<&str> = w.hosts.iter().filter(|h| !installed.is_some_and(|i| i.offer.hosts.contains(h))).map(String::as_str).collect();
        let sign_in = w.sign_in.is_some() && installed.is_some_and(|i| i.offer.sign_in != w.sign_in);
        say("extension-update-asks", &[("version", w.version.clone()), ("sites", if added.is_empty() { "none".to_string() } else { added.join(", ") }), ("signin", if sign_in { "yes" } else { "no" }.to_string())])
    });
    json!({
        "id": offer.id,
        "name": name,
        "about": offer.about.said(fr),
        "version": installed.map_or(offer.version.clone(), |i| i.offer.version.clone()),
        "brings": say("extension-brings-agenda", &[]),
        "sites": say("extension-sites", &[("count", hosts.len().to_string()), ("sites", hosts.join(", "))]),
        "signIn": if offer.sign_in.is_some() { say("extension-keeps-sign-in", &[]) } else { say("extension-keeps-no-sign-in", &[]) },
        "needsSignIn": offer.sign_in.is_some(),
        "usable": offer.usable(),
        "unusable": if offer.usable() { String::new() } else { say("extension-needs-newer-sioul", &[]) },
        "installed": installed.is_some(),
        "enabled": installed.is_some_and(|i| i.enabled),
        "signedIn": installed.is_some_and(|i| i.signed_in),
        "running": running,
        "line": line,
        "problem": installed.and_then(|i| i.problem.as_deref()).map(|p| problem_line(p, &name)).unwrap_or_default(),
        "waiting": waiting.unwrap_or_default(),
        "agendas": agendas,
    })
}

/// The page's view: the offers of the manifest last read, then those this
/// device has that it no longer lists; where the list is read and how it went.
fn page_json() -> String {
    let state = state::load(&state::path());
    let running = RUNNING.lock().map(|r| r.clone()).unwrap_or_default();
    let listed = LISTED.lock().ok();
    let listed = listed.as_ref().and_then(|l| l.as_ref());
    let mut offers: Vec<Value> = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(manifest) = listed.and_then(|l| l.manifest.as_ref()) {
        for offer in &manifest.offers {
            seen.insert(offer.id.clone());
            offers.push(offer_json(offer, state.get(&offer.id), running.contains(&offer.id)));
        }
    }
    for installed in state.extensions.iter().filter(|i| !seen.contains(&i.offer.id)) {
        offers.push(offer_json(&installed.offer, Some(installed), running.contains(&installed.offer.id)));
    }
    let address = listed.map(|l| l.address.clone()).unwrap_or_else(|| load_config().extensions.manifest_address().to_string());
    json!({
        "reading": READING.lock().map(|r| *r).unwrap_or(false),
        "where": say("extensions-where", &[("address", address)]),
        "problem": listed.and_then(|l| l.problem.clone()).map(|p| say("extensions-list-problem", &[("detail", p)])).unwrap_or_default(),
        "offers": offers,
    })
    .to_string()
}

/// The page's view set on the window's thread.
fn show(qt: &QtThread) {
    let view = page_json();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_extensions(QString::from(&view)));
}

/// Settings ▸ Extensions shown: its view at once, then the manifest read.
pub(crate) fn page(qt: &QtThread) {
    show(qt);
    read_list(Some(qt.clone()));
}

// ---------------------------------------------------------------- the manifest

/// The manifest read, off the window's thread; an update taken or set
/// waiting; the page told when there is one.
fn read_list(qt: Option<QtThread>) {
    {
        let Ok(mut reading) = READING.lock() else { return };
        if *reading {
            return;
        }
        *reading = true;
    }
    if let Some(qt) = &qt {
        show(qt);
    }
    std::thread::spawn(move || {
        let address = load_config().extensions.manifest_address().to_string();
        let read = store::read_manifest(&address, SystemTime::now()).and_then(|manifest| apply(&manifest, &address).map(|()| manifest));
        let (manifest, problem) = match read {
            Ok(manifest) => (Some(manifest), None),
            Err(problem) => {
                eprintln!("sioul: extensions: the list: {problem}");
                (None, Some(problem))
            }
        };
        if let Ok(mut listed) = LISTED.lock() {
            let kept = listed.take().and_then(|l| l.manifest);
            // A list that did not read leaves the last one shown, with why.
            *listed = Some(Listed { address, manifest: manifest.or(kept), problem });
        }
        if let Ok(mut reading) = READING.lock() {
            *reading = false;
        }
        if let Some(qt) = &qt {
            show(qt);
        }
    });
}

/// A manifest whose signature held taken in this device's state: its serial
/// first (never lower than the last accepted here, or the whole manifest is
/// refused), then for each extension installed here a newer version taken at
/// once when it asks for no more, else set waiting for the person's yes.
/// Files are downloaded outside the state's lock.
fn apply(manifest: &Manifest, address: &str) -> Result<(), String> {
    let path = state::path();
    state::change(&path, |state| {
        state.accept_serial(manifest.serial)?;
        state.checked = now();
        Ok::<(), String>(())
    })??;
    let state = state::load(&path);
    let (mut taken, mut waiting) = (Vec::new(), Vec::new());
    for installed in &state.extensions {
        let Some(offer) = manifest.offers.iter().find(|o| o.id == installed.offer.id) else { continue };
        if offer.sha256 == installed.offer.sha256 || !offer.usable() {
            continue;
        }
        if offer.grants().asks_more_than(&installed.offer.grants()) {
            waiting.push(offer.clone());
            continue;
        }
        match store::install(address, offer) {
            Ok(_) => taken.push((installed.offer.clone(), offer.clone())),
            Err(problem) => eprintln!("sioul: extensions: {}: an update: {problem}", offer.id),
        }
    }
    state::change(&path, |state| {
        for (old, offer) in &taken {
            if let Some(installed) = state.get_mut(&offer.id) {
                store::discard_file(old);
                installed.offer = offer.clone();
                installed.waiting = None;
            }
        }
        for offer in &waiting {
            if let Some(installed) = state.get_mut(&offer.id) {
                installed.waiting = Some(offer.clone());
            }
        }
    })
}

/// The offer of `id` in the manifest last read.
fn offered(id: &str) -> Option<(Offer, String)> {
    let listed = LISTED.lock().ok()?;
    let listed = listed.as_ref()?;
    Some((listed.manifest.as_ref()?.offers.iter().find(|o| o.id == id)?.clone(), listed.address.clone()))
}

// ---------------------------------------------------------------- the person's acts

/// Enable: the file downloaded, checked against the signed manifest and
/// kept; then a sign-in asked for, or a first refresh.
pub(crate) fn enable(qt: &QtThread, id: &str) {
    let (qt, id) = (qt.clone(), id.to_string());
    std::thread::spawn(move || {
        let enabled = state::load(&state::path()).get(&id).map(|i| i.offer.clone());
        let outcome = match enabled {
            // Installed before, disabled since: enabled again as it was accepted.
            Some(offer) if store::checked_file(&offer).is_ok() => state::change(&state::path(), |s| {
                if let Some(i) = s.get_mut(&id) {
                    i.enabled = true;
                }
            }),
            _ => match offered(&id) {
                None => Err(say("extension-not-listed", &[])),
                Some((offer, _)) if !offer.usable() => Err(say("extension-needs-newer-sioul", &[])),
                Some((offer, address)) => store::install(&address, &offer).and_then(|_| {
                    state::change(&state::path(), |s| {
                        s.extensions.retain(|i| i.offer.id != offer.id);
                        s.extensions.push(Installed::new(offer.clone()));
                    })
                }),
            },
        };
        let name = name_of(&id);
        match outcome {
            Ok(()) => {
                crate::backend::set_status(&qt, say("extension-enabled", &[("name", name)]));
                if state::load(&state::path()).get(&id).is_some_and(|i| i.offer.sign_in.is_none()) {
                    refresh(Some(&qt), &id);
                }
            }
            Err(problem) => {
                eprintln!("sioul: extensions: {id}: enabling: {problem}");
                crate::backend::set_status(&qt, say("extension-not-enabled", &[("name", name)]));
            }
        }
        show(&qt);
    });
}

/// Disable: it no longer runs here; its sign-in and agendas stay.
pub(crate) fn disable(qt: &QtThread, id: &str) {
    let _ = state::change(&state::path(), |s| {
        if let Some(i) = s.get_mut(id) {
            i.enabled = false;
        }
    });
    crate::backend::set_status(qt, say("extension-disabled", &[("name", name_of(id))]));
    show(qt);
}

/// Remove: its sign-in forgotten, its file taken out, its record gone; its
/// agendas kept, or taken away when `keep` is false.
pub(crate) fn remove(qt: &QtThread, id: &str, keep: bool) {
    let (qt, id) = (qt.clone(), id.to_string());
    std::thread::spawn(move || {
        let name = name_of(&id);
        let installed = state::load(&state::path()).get(&id).cloned();
        let forgotten = store::forget_secret(&id).is_ok();
        let mut taken = true;
        if let Some(installed) = &installed {
            store::discard_file(&installed.offer);
            if let Some(waiting) = &installed.waiting {
                store::discard_file(waiting);
            }
            if !keep {
                for agenda in &installed.agendas {
                    taken &= door().remove(&sioul_core::extensions::agenda_folder(&id, &agenda.id)).is_ok();
                }
            }
        }
        let _ = state::change(&state::path(), |s| s.extensions.retain(|i| i.offer.id != id));
        let line = if !forgotten {
            say("extension-removed-keyring", &[("name", name)])
        } else if !taken {
            say("extension-removed-agendas-stay", &[("name", name)])
        } else if keep {
            say("extension-removed-kept", &[("name", name)])
        } else {
            say("extension-removed", &[("name", name)])
        };
        crate::backend::set_status(&qt, line);
        show(&qt);
    });
}

/// Yes to an update that asks for more: installed, what it is given now its own.
pub(crate) fn accept(qt: &QtThread, id: &str) {
    let (qt, id) = (qt.clone(), id.to_string());
    std::thread::spawn(move || {
        let address = load_config().extensions.manifest_address().to_string();
        let waiting = state::load(&state::path()).get(&id).and_then(|i| i.waiting.clone());
        if let Some(offer) = waiting {
            match store::install(&address, &offer) {
                Ok(_) => {
                    let _ = state::change(&state::path(), |s| {
                        if let Some(i) = s.get_mut(&id) {
                            store::discard_file(&i.offer);
                            // A new sign-in asked for: the old one no longer counts.
                            if i.offer.sign_in != offer.sign_in {
                                i.signed_in = false;
                            }
                            i.offer = offer.clone();
                            i.waiting = None;
                        }
                    });
                    crate::backend::set_status(&qt, say("extension-updated", &[("name", name_of(&id)), ("version", offer.version.clone())]));
                }
                Err(problem) => {
                    eprintln!("sioul: extensions: {id}: an update: {problem}");
                    crate::backend::set_status(&qt, say("extension-not-enabled", &[("name", name_of(&id))]));
                }
            }
        }
        show(&qt);
    });
}

fn name_of(id: &str) -> String {
    let fr = french();
    state::load(&state::path()).get(id).map(|i| i.offer.name.said(fr).to_string()).or_else(|| offered(id).map(|(o, _)| o.name.said(fr).to_string())).unwrap_or_else(|| id.to_string())
}

// ---------------------------------------------------------------- the runs

/// The log's lines of one call written, under a line saying which.
fn keep_log(id: &str, what: &str, outcome: &Outcome) {
    let mut lines = vec![format!("{} {what}", jiff::Timestamp::now())];
    lines.extend(outcome.log.iter().cloned());
    if let Err(problem) = state::append_log(&state::log_path(id), &lines) {
        eprintln!("sioul: extensions: {id}: its log: {problem}");
    }
}

/// Marks `id` running; false when it runs already.
fn start(id: &str) -> bool {
    RUNNING.lock().map(|mut r| r.insert(id.to_string())).unwrap_or(false)
}

fn end(id: &str) {
    if let Ok(mut running) = RUNNING.lock() {
        running.remove(id);
    }
}

/// One refresh of `id`, here and now (off the window's thread): its agendas
/// taken in when this device's fetch is the freshest, its state noted.
pub(crate) fn refresh(qt: Option<&QtThread>, id: &str) {
    let Some(installed) = state::load(&state::path()).get(id).cloned().filter(|i| i.enabled) else { return };
    if !start(id) {
        return;
    }
    if let Some(qt) = qt {
        show(qt);
    }
    if let Ok(mut started) = STARTED.lock() {
        started.insert(id.to_string());
    }
    let fetched = now();
    let (outcome, _) = store::run(&installed.offer, Call::Refresh, french(), Real::new(id, None));
    keep_log(id, "refresh", &outcome);
    let mut brought: Option<Vec<Brought>> = None;
    let problem: Option<String> = match &outcome.reply {
        Err(stop) => Some(stop.word().to_string()),
        Ok(text) => match reply::refresh_reply(text) {
            Err(nonsense) => {
                let _ = state::append_log(&state::log_path(id), &[format!("{} sioul: stopped: it replied nonsense: {}", jiff::Timestamp::now(), nonsense.0)]);
                Some("stopped-nonsense".into())
            }
            Ok(reply) => {
                let write = !reply.agendas.is_empty() && may_write(id, fetched);
                match take_in(&door(), id, &reply, write) {
                    Ok(taken) => {
                        if write {
                            note_written(id, fetched);
                        }
                        brought = Some(taken);
                        reply.problem.as_ref().map(|p| p.kind.word().to_string())
                    }
                    Err(problem) => {
                        let _ = state::append_log(&state::log_path(id), &[format!("{} sioul: the agendas were not taken in: {problem}", jiff::Timestamp::now())]);
                        Some("door".into())
                    }
                }
            }
        },
    };
    let signed_out = problem.as_deref() == Some(ProblemKind::SignIn.word());
    let _ = state::change(&state::path(), |s| {
        if let Some(i) = s.get_mut(id) {
            i.last_run = fetched;
            if let Some(brought) = &brought
                && !brought.is_empty()
            {
                i.agendas = brought.clone();
                i.last_brought = fetched;
            }
            if signed_out {
                i.signed_in = false;
            }
            i.problem = problem.clone();
        }
    });
    end(id);
    if let Some(qt) = qt {
        if let Some(problem) = &problem {
            crate::backend::set_status(qt, problem_line(problem, &name_of(id)));
        }
        show(qt);
    }
}

/// Sign in…: the extension's sign-in call, its page shown by the window; a
/// choice asked of the person first when the extension asks one
/// (`extension_choices`), then a first refresh once signed in.
pub(crate) fn sign_in(qt: &QtThread, id: &str, choice: Option<String>) {
    let (qt, id) = (qt.clone(), id.to_string());
    std::thread::spawn(move || {
        let Some(installed) = state::load(&state::path()).get(&id).cloned().filter(|i| i.enabled) else { return };
        if !start(&id) {
            return;
        }
        show(&qt);
        let shower_qt = qt.clone();
        let shower: store::SignInShower = Box::new(move |page| show_page(&shower_qt, page));
        let (outcome, _) = store::run(&installed.offer, Call::SignIn(choice.as_deref()), french(), Real::new(&id, Some(shower)));
        keep_log(&id, "sign-in", &outcome);
        end(&id);
        let name = name_of(&id);
        let mut signed = false;
        let problem: Option<String> = match &outcome.reply {
            Err(stop) => Some(stop.word().to_string()),
            Ok(text) => match reply::sign_in_reply(text) {
                Err(_) => Some("stopped-nonsense".into()),
                Ok(SignInReply::SignedIn) => {
                    signed = true;
                    None
                }
                Ok(SignInReply::Choose(choices)) => {
                    let list = json!(choices.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>()).to_string();
                    let id_q = id.clone();
                    let _ = qt.queue(move |mut sioul| sioul.as_mut().extension_choices(QString::from(&id_q), QString::from(&list)));
                    None
                }
                Ok(SignInReply::Cancelled) => {
                    crate::backend::set_status(&qt, say("extension-sign-in-closed", &[("name", name.clone())]));
                    None
                }
                Ok(SignInReply::Problem(p)) => Some(p.kind.word().to_string()),
            },
        };
        let _ = state::change(&state::path(), |s| {
            if let Some(i) = s.get_mut(&id) {
                if signed {
                    i.signed_in = true;
                    i.problem = None;
                } else if problem.is_some() {
                    i.problem = problem.clone();
                }
            }
        });
        if signed {
            crate::backend::set_status(&qt, say("extension-signed-in", &[("name", name)]));
            refresh(Some(&qt), &id);
        } else if let Some(problem) = &problem {
            crate::backend::set_status(&qt, problem_line(problem, &name));
        }
        show(&qt);
    });
}

/// Each minute of the window: the extensions enabled and signed in here
/// refreshed when Sioul started or three hours passed, and the manifest read
/// once a day while one is enabled.
pub(crate) fn tick(qt: &QtThread) {
    let state = state::load(&state::path());
    if state.extensions.iter().all(|i| !i.enabled) {
        return;
    }
    let now = now();
    // Once a day; after a failed read, again an hour later at the soonest.
    if now - state.checked >= DAILY && now - TRIED.load(std::sync::atomic::Ordering::Relaxed) >= 3600 {
        TRIED.store(now, std::sync::atomic::Ordering::Relaxed);
        read_list(Some(qt.clone()));
    }
    let started = STARTED.lock().map(|s| s.clone()).unwrap_or_default();
    for installed in due(&state, now, &started) {
        let (qt, id) = (qt.clone(), installed);
        std::thread::spawn(move || refresh(Some(&qt), &id));
    }
}

/// The extensions due now: enabled here, signed in when they sign in, not
/// running, and not refreshed since Sioul started or for three hours.
fn due(state: &State, now: i64, started: &BTreeSet<String>) -> Vec<String> {
    let running = RUNNING.lock().map(|r| r.clone()).unwrap_or_default();
    state
        .extensions
        .iter()
        .filter(|i| i.enabled && (i.signed_in || i.offer.sign_in.is_none()) && !running.contains(&i.offer.id))
        .filter(|i| !started.contains(&i.offer.id) || now - i.last_run >= EVERY)
        .map(|i| i.offer.id.clone())
        .collect()
}

/// The phone's background step: the extensions due refreshed here, in the
/// service's process (no sign-in page there); never waited for long.
pub(crate) fn background_step() {
    let state = state::load(&state::path());
    let now = now();
    // In the service's process "started" means nothing: three hours, or never run.
    let ids: Vec<String> = due(&state, now, &BTreeSet::new()).into_iter().filter(|id| state.get(id).is_some_and(|i| now - i.last_run >= EVERY)).collect();
    for id in ids {
        refresh(None, &id);
    }
}

// ---------------------------------------------------------------- the sign-in page

/// The sign-in page waited for: where its answer goes, and what it watches for.
static PAGE: Mutex<Option<(std::sync::mpsc::Sender<Option<String>>, String)>> = Mutex::new(None);

/// A sign-in page shown by the window (a computer's SignInWindow.qml, a
/// phone's SignIn.java), its answer waited for here, `page.wait` at most.
fn show_page(qt: &QtThread, page: &SignInPage) -> Result<Option<String>, &'static str> {
    let (said, answer) = std::sync::mpsc::channel();
    if let Ok(mut waiting) = PAGE.lock() {
        *waiting = Some((said, page.watch.clone()));
    }
    let asked = json!({ "open": page.open, "watch": page.watch, "host": page.host }).to_string();
    #[cfg(target_os = "android")]
    {
        let _ = qt;
        if crate::steps::java("sign-in-open", &asked)["asked"] != true {
            if let Ok(mut waiting) = PAGE.lock() {
                *waiting = None;
            }
            return Err("not-now");
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = qt.queue(move |mut sioul| sioul.as_mut().set_sign_in_page(QString::from(&asked)));
    }
    let returned = answer.recv_timeout(page.wait).unwrap_or(None);
    if let Ok(mut waiting) = PAGE.lock() {
        *waiting = None;
    }
    #[cfg(target_os = "android")]
    crate::steps::java("sign-in-close", "{}");
    #[cfg(not(target_os = "android"))]
    {
        let _ = qt.queue(|mut sioul| sioul.as_mut().set_sign_in_page(QString::default()));
    }
    Ok(returned)
}

/// The page's word: the address it returned to, or "" when the person
/// closed it. Only an address that begins with what was watched for is given back.
pub(crate) fn returned(address: &str) {
    let waiting = PAGE.lock().ok().and_then(|mut w| w.take());
    if let Some((said, watch)) = waiting {
        let _ = said.send((!address.is_empty() && address.starts_with(&watch)).then(|| address.to_string()));
    }
}

/// SignIn.java's word (android/main.cpp): {returned} or {cancelled}.
///
/// # Safety
/// `json` is null, or a zero-terminated UTF-8 text valid for the call.
#[cfg(target_os = "android")]
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_sign_in_returned(json: *const std::ffi::c_char) {
    // SAFETY: as the caller promises.
    let text = if json.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy().into_owned() };
    let _ = std::panic::catch_unwind(|| {
        let answer: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        returned(answer["returned"].as_str().unwrap_or(""));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::extensions::reply::refresh_reply;
    use std::cell::RefCell;

    /// A door in memory, as the read-only agendas' will be.
    #[derive(Default)]
    struct Memory(RefCell<BTreeMap<String, (String, BTreeMap<String, String>)>>);

    impl AgendaDoor for Memory {
        fn files(&self, folder: &str) -> BTreeMap<String, String> {
            self.0.borrow().get(folder).map(|(_, f)| f.clone()).unwrap_or_default()
        }
        fn replace(&self, folder: &str, name: &str, files: &BTreeMap<String, String>) -> Result<bool, String> {
            let changed = self.0.borrow().get(folder) != Some(&(name.to_string(), files.clone()));
            self.0.borrow_mut().insert(folder.to_string(), (name.to_string(), files.clone()));
            Ok(changed)
        }
        fn remove(&self, folder: &str) -> Result<(), String> {
            self.0.borrow_mut().remove(folder);
            Ok(())
        }
    }

    fn calendar(events: &[(&str, &str)]) -> String {
        let body: String = events.iter().map(|(uid, start)| format!("BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:{start}\r\nDTSTART:{start}\r\nDTEND:{start}\r\nSUMMARY:Lesson\r\nEND:VEVENT\r\n")).collect();
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\n{body}END:VCALENDAR\r\n")
    }

    fn refreshed(events: &[(&str, &str)], unread: &[(&str, &str)]) -> RefreshReply {
        let unread: Vec<Value> = unread.iter().map(|(from, until)| json!({ "from": from, "until": until })).collect();
        refresh_reply(&json!({ "agendas": [{ "id": "pupil-1", "name": "Timetable: Alex", "calendar": calendar(events), "unread": unread }] }).to_string()).unwrap()
    }

    #[test]
    fn an_agenda_is_replaced_whole_but_for_what_was_not_read() {
        let door = Memory::default();
        let first = refreshed(&[("A", "20261119T080000Z"), ("B", "20261120T080000Z"), ("C", "20261123T080000Z")], &[]);
        let brought = take_in(&door, "timetable", &first, true).unwrap();
        assert_eq!(brought, vec![Brought { id: "pupil-1".into(), name: "Timetable: Alex".into(), events: 3 }]);
        assert_eq!(door.files("ext-timetable-pupil-1").len(), 3);
        // The 20th answered 500: B stays; C was read and is gone.
        let second = refreshed(&[("A", "20261119T090000Z")], &[("2026-11-19T23:00:00Z", "2026-11-20T23:00:00Z")]);
        take_in(&door, "timetable", &second, true).unwrap();
        let files = door.files("ext-timetable-pupil-1");
        assert_eq!(files.len(), 2);
        assert!(files.values().any(|t| t.contains("UID:B")) && !files.values().any(|t| t.contains("UID:C")));
        assert!(files.values().any(|t| t.contains("DTSTART:20261119T090000Z")));
        // A device whose fetch is not the freshest writes nothing.
        let third = refreshed(&[], &[]);
        take_in(&door, "timetable", &third, false).unwrap();
        assert_eq!(door.files("ext-timetable-pupil-1").len(), 2);
        // Until the door is merged, nothing is written and the reason is said.
        assert!(take_in(&Waiting, "timetable", &first, true).is_err());
    }

    #[test]
    fn a_refresh_is_due_at_start_then_every_three_hours() {
        let offer = sioul_core::extensions::manifest::parse(&format!(
            "format = 1\nserial = 1\n[[extension]]\nid = \"t\"\nversion = \"1\"\ninterface = 1\nname = {{ en = \"T\", fr = \"T\" }}\nabout = {{ en = \"T\", fr = \"T\" }}\nbrings = [\"agenda\"]\nhosts = [\"api.example.org\"]\nsign_in = \"app://cb\"\nfile = \"t.wasm\"\nsha256 = \"{}\"\n",
            "0".repeat(64)
        ))
        .unwrap()
        .offers
        .remove(0);
        let mut installed = Installed::new(offer);
        installed.last_run = 1000;
        let mut state = State { extensions: vec![installed], ..State::default() };
        let none = BTreeSet::new();
        assert!(due(&state, 2000, &none).is_empty(), "not signed in: nothing to refresh");
        state.extensions[0].signed_in = true;
        assert_eq!(due(&state, 2000, &none), vec!["t".to_string()], "Sioul started");
        let started: BTreeSet<String> = ["t".to_string()].into();
        assert!(due(&state, 2000, &started).is_empty());
        assert_eq!(due(&state, 1000 + EVERY, &started), vec!["t".to_string()]);
        state.extensions[0].enabled = false;
        assert!(due(&state, 1000 + EVERY, &started).is_empty());
    }

    #[test]
    fn a_sign_in_page_gives_back_only_what_it_watched_for() {
        let (said, answer) = std::sync::mpsc::channel();
        *PAGE.lock().unwrap() = Some((said, "app://cb".into()));
        returned("https://elsewhere.example.org/?code=x");
        assert_eq!(answer.recv().unwrap(), None);
        let (said, answer) = std::sync::mpsc::channel();
        *PAGE.lock().unwrap() = Some((said, "app://cb".into()));
        returned("app://cb?code=x");
        assert_eq!(answer.recv().unwrap().as_deref(), Some("app://cb?code=x"));
        returned("app://cb?code=again");
        assert!(PAGE.lock().unwrap().is_none(), "once");
    }
}

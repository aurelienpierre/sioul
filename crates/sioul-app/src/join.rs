// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A device joining your other devices through your Nextcloud's own sign-in
//! (docs/database.md, "Joining through your Nextcloud's sign-in"): nothing to
//! type but the passphrase, no sync app needed. Offered by the Porch's card
//! on a device that does not share yet ("Join your other devices") and in
//! Settings ▸ Your folder and sharing (`qml/JoinPanel.qml`), on a phone and
//! on a computer alike.
//!
//! 1. **Signed in** (`sign_in`): the server's sign-in page opens in the
//!    browser (`sioul_sync::loginflow`; Murena's, or another Nextcloud by its
//!    address); the app password it gives goes to the keyring at once
//!    (`loginflow::keep`), never to a file nor a log.
//! 2. **The folder found** there (`remote::seek`): Documents/Sioul, Sioul, or
//!    a folder in Documents; proposed, or a place you type; none found, every
//!    place looked at: you may start sharing there, as before; the look
//!    incomplete (a slow or failing server): look again, or type its place,
//!    never a new sharing offered.
//! 3. **Joined** (`join`): the passphrase typed once, checked against the
//!    seal (twice for a new sharing); Sioul keeps its own copy of the folder
//!    in step with that login (`remote::begin_signed_in`); a first exchange
//!    runs at once and brings the shared settings, accounts and hours. A
//!    sharing started there instead is said as such, never as joined.
//! 4. **The contacts and calendars** (`bind_agenda`), once the first
//!    exchange brought everything (`complete_since`), else at a later one
//!    (`agenda_later`): an account of yours on that server (its address's
//!    origin) with that login, come with the settings or here already, is
//!    used, given the homes found there and the app password when it has none
//!    and it opens it there; else one is made, its homes found on that server alone.
//! 5. **Said** (`Stand`): who this device shares with now, what came, and
//!    what still needs a sign-in here; on a phone, whether it replaces
//!    another phone (`replacing`).
//!
//! Cancel stops it at any step before the join: an app password already given
//! is given back to the server (`loginflow::revoke`) and forgotten here.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use cxx_qt_lib::QString;
use serde::{Deserialize, Serialize};
use sioul_core::config::{self, Account, AccountKind, Config, state_dir};
use sioul_sync::loginflow::{self, Grant, Refusal};
use sioul_sync::{remote, secret};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Where a sign-in and its joining stand, for the window (`status`).
#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
struct Stand {
    /// "" nothing under way; "signing": the browser open, waited for;
    /// "looking": signed in, the folder looked for; "found": places found;
    /// "none": none found, every place looked at (a new sharing may start);
    /// "unchecked": the look could not go through (look again, or a place
    /// typed, never a new sharing); "joining": the first exchange running;
    /// "joined": done, said below; "failed": the sign-in did not go (`problem`).
    step: String,
    /// The server, by its name: "murena.io".
    host: String,
    /// "Signed in to murena.io as jane."
    signed: String,
    /// Where the folder was found there: "Documents/Sioul".
    places: Vec<String>,
    /// Why not, in words.
    problem: String,
    /// What the join did and brought, in sentences.
    lines: Vec<String>,
    /// What still needs a sign-in on this device, one line an account.
    needs: Vec<String>,
}

static STAND: Mutex<Stand> = Mutex::new(Stand { step: String::new(), host: String::new(), signed: String::new(), places: Vec::new(), problem: String::new(), lines: Vec::new(), needs: Vec::new() });

/// The sign-in's grant, between the sign-in and the join: in memory only
/// (zeroed when dropped), its app password in the keyring besides.
static GRANT: Mutex<Option<Grant>> = Mutex::new(None);

/// The page of the sign-in under way, to open again: its address holds the
/// sign-in's own key, never logged.
static PAGE: Mutex<Option<loginflow::Secret>> = Mutex::new(None);

/// Cancel pressed.
static STOP: AtomicBool = AtomicBool::new(false);

/// A sign-in, or a join, running: one at a time.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// `RUNNING` let go when its thread ends, however it ends (a panic too):
/// the next sign-in is never refused for one that is over.
struct Running;

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

fn stand() -> std::sync::MutexGuard<'static, Stand> {
    STAND.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn set(change: impl FnOnce(&mut Stand)) {
    change(&mut stand());
}

fn grant() -> std::sync::MutexGuard<'static, Option<Grant>> {
    GRANT.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The window told: it reads `status` again.
fn told(qt: &QtThread) {
    let _ = qt.queue(|mut sioul| sioul.as_mut().join_changed());
}

/// Where things stand, as JSON (`Stand`).
pub(crate) fn status() -> String {
    json(&*stand())
}

/// What Sioul says it is on the server's sign-in page and in its list of
/// devices and sessions: "Sioul (Android)".
fn client() -> String {
    let system = if cfg!(target_os = "android") {
        "Android"
    } else if cfg!(windows) {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    };
    format!("Sioul ({system})")
}

/// Why a sign-in did not go, in words.
fn refusal_words(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Address => tr().text("join-why-address", None),
        Refusal::NotHttps(host) => say("join-why-not-https", &[("host", host.clone())]),
        Refusal::Unreachable(host) => say("join-why-unreachable", &[("host", host.clone())]),
        Refusal::Tls(host) => say("join-why-tls", &[("host", host.clone())]),
        Refusal::NotNextcloud(host) => say("join-why-not-nextcloud", &[("host", host.clone())]),
        Refusal::Elsewhere(host, other) => say("join-why-elsewhere", &[("host", host.clone()), ("other", other.clone())]),
        Refusal::Server(host) => say("join-why-server", &[("host", host.clone())]),
        Refusal::TimedOut => tr().text("join-why-timed-out", None),
        Refusal::Cancelled => String::new(),
    }
}

/// An app password not used after all: given back to its server, forgotten here.
fn give_back(grant: &Grant) {
    let _ = loginflow::revoke(grant);
    let _ = loginflow::forget(&grant.login, &grant.host());
}

/// Signs in to `typed` ("murena", or another Nextcloud's address), off the
/// window's thread: its page opened in the browser (`open_url`), the poll
/// asked until you signed in there, Cancel or twenty minutes; then the
/// sharing folder looked for there. `join_changed` says each step.
pub(crate) fn sign_in(qt: &QtThread, typed: String) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    STOP.store(false, Ordering::SeqCst);
    // An earlier sign-in not joined: given back first.
    if let Some(earlier) = grant().take() {
        std::thread::spawn(move || give_back(&earlier));
    }
    let typed = if typed.trim().is_empty() || typed.trim().eq_ignore_ascii_case("murena") { loginflow::MURENA.to_string() } else { typed.trim().to_string() };
    set(|s| *s = Stand { step: "signing".into(), host: remote::host_of(&typed), ..Stand::default() });
    told(qt);
    let qt = qt.clone();
    std::thread::spawn(move || {
        let running = Running;
        let outcome = (|| -> Result<(), String> {
            let base = loginflow::address_of(&typed).map_err(|r| refusal_words(&r))?;
            let flow = loginflow::begin(&base, &client()).map_err(|r| refusal_words(&r))?;
            set(|s| s.host = flow.host());
            if let Ok(mut page) = PAGE.lock() {
                *page = Some(loginflow::secret(flow.page()));
            }
            let page = flow.page().to_string();
            let _ = qt.queue(move |mut sioul| sioul.as_mut().open_url(QString::from(&page)));
            told(&qt);
            let signed = loginflow::wait(&flow, &STOP, loginflow::LIMIT, loginflow::EVERY);
            if let Ok(mut page) = PAGE.lock() {
                *page = None;
            }
            let signed = signed.map_err(|r| refusal_words(&r))?;
            if STOP.load(Ordering::SeqCst) {
                give_back(&signed);
                return Err(String::new());
            }
            // In the keyring at once, under that login on that server: the sharing's own.
            loginflow::keep(&signed).map_err(|e| e.sentence(tr(), &signed.host()))?;
            set(|s| {
                s.step = "looking".into();
                s.signed = say("join-signed", &[("host", signed.host()), ("login", signed.login.clone())]);
            });
            told(&qt);
            let sought = remote::seek(&signed.login_for(""));
            if STOP.load(Ordering::SeqCst) {
                give_back(&signed);
                return Err(String::new());
            }
            set(|s| looked(s, &signed.host(), sought));
            *grant() = Some(signed);
            Ok(())
        })();
        if let Err(problem) = outcome {
            set(|s| {
                s.step = if problem.is_empty() { String::new() } else { "failed".into() };
                s.problem = problem;
            });
        }
        drop(running);
        // Cancel pressed as it ended: what came is given back.
        if STOP.load(Ordering::SeqCst)
            && let Some(late) = grant().take()
        {
            give_back(&late);
            set(|s| *s = Stand::default());
        }
        told(&qt);
    });
}

/// What the look found, said (`Stand`): places found, proposed; none found,
/// every place looked at, a new sharing may start there; the look
/// incomplete, or not possible at all (a slow server, a failing listing):
/// "could not look", look again or type a place, never a new sharing
/// offered where the folder may be (the review of the join, M3).
fn looked(stand: &mut Stand, host: &str, sought: Result<remote::Sought, String>) {
    stand.places.clear();
    stand.problem.clear();
    match sought {
        Ok(sought) if !sought.found.is_empty() => {
            stand.step = "found".into();
            stand.places = sought.found;
        }
        Ok(sought) if sought.complete => stand.step = "none".into(),
        Ok(_) => {
            stand.step = "unchecked".into();
            stand.problem = say("join-unchecked", &[("host", host.to_string())]);
        }
        Err(code) => {
            stand.step = "unchecked".into();
            stand.problem = [say("join-unchecked", &[("host", host.to_string())]), crate::share::server_problem(&code)].join(" ");
        }
    }
}

/// "Look again", the look having found nothing it could trust (`unchecked`)
/// or not the place you expected: the server looked through again with the
/// sign-in held, off the window's thread.
pub(crate) fn look_again(qt: &QtThread) {
    let Some(held) = grant().as_ref().map(|g| Grant::of(&g.server, &g.login, g.password())) else { return };
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    set(|s| {
        s.step = "looking".into();
        s.problem.clear();
    });
    told(qt);
    let qt = qt.clone();
    std::thread::spawn(move || {
        let running = Running;
        let sought = remote::seek(&held.login_for(""));
        set(|s| looked(s, &held.host(), sought));
        drop(running);
        told(&qt);
    });
}

/// The sign-in's page opened again in the browser (closed by mistake).
pub(crate) fn open_again(qt: &QtThread) {
    let page = PAGE.lock().ok().and_then(|p| p.as_ref().map(|p| p.to_string()));
    if let Some(page) = page {
        let _ = qt.queue(move |mut sioul| sioul.as_mut().open_url(QString::from(&page)));
    }
}

/// Cancel: the sign-in stops where it is; an app password given already is
/// given back to its server and forgotten here. Nothing joined is undone.
pub(crate) fn cancel(qt: &QtThread) {
    STOP.store(true, Ordering::SeqCst);
    if !RUNNING.load(Ordering::SeqCst) {
        if let Some(held) = grant().take() {
            std::thread::spawn(move || give_back(&held));
        }
        set(|s| *s = Stand::default());
    }
    told(qt);
}

/// The result read, or a failed sign-in left: nothing said any more.
pub(crate) fn done(qt: &QtThread) {
    if RUNNING.load(Ordering::SeqCst) {
        return;
    }
    let step = stand().step.clone();
    if matches!(step.as_str(), "joined" | "failed") {
        set(|s| *s = Stand::default());
    }
    told(qt);
}

/// Joins the sharing through `place` on the server signed in to, with the
/// passphrase (typed once, checked against the folder's seal; twice, `again`,
/// for a new sharing there), off the window's thread; then a first exchange
/// at once, the contacts and calendars of that server, and what came, said.
/// The grant is kept while the passphrase is wrong, for another try.
pub(crate) fn join(qt: &QtThread, shared: &Arc<Shared>, place: String, passphrase: String, again: String) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    let back = stand().step.clone();
    // A new sharing only where every place was looked at and none found.
    let may_start = back == "none";
    set(|s| {
        s.step = "joining".into();
        s.problem.clear();
    });
    told(&qt);
    std::thread::spawn(move || {
        let running = Running;
        let held = grant().as_ref().map(|g| Grant::of(&g.server, &g.login, g.password()));
        let outcome = match held {
            None => Err(tr().text("join-why-signed-out", None)),
            Some(signed) => joined(&qt, &shared, &signed, &place, &passphrase, &again, may_start),
        };
        match outcome {
            Ok((lines, needs)) => {
                // Joined: the grant leaves memory (its app password stays in the keyring).
                *grant() = None;
                set(|s| {
                    s.step = "joined".into();
                    s.lines = lines;
                    s.needs = needs;
                    s.places.clear();
                });
            }
            Err(problem) => set(|s| {
                s.step = back.clone();
                s.problem = problem;
            }),
        }
        drop(running);
        told(&qt);
        crate::backend::show(&qt, &shared);
    });
}

/// The join itself, on its thread: what it said, and what still needs a sign-in here.
#[allow(clippy::too_many_arguments)]
fn joined(qt: &QtThread, shared: &Arc<Shared>, signed: &Grant, place: &str, passphrase: &str, again: &str, may_start: bool) -> Result<(Vec<String>, Vec<String>), String> {
    let before = load_config();
    let before_text = std::fs::read_to_string(crate::backend::config_path()).unwrap_or_default();
    let login = signed.login_for(&agenda_id(&before, signed));
    let since = jiff::Timestamp::now().as_second();
    let begun = crate::share::begin_on_server(&login, place, passphrase, again, true, may_start)?;
    // The first exchange: the server looked through, the others' files read, this device's sent.
    crate::share::exchange_at_once(qt, shared);
    let host = signed.host();
    let mut lines = match begun {
        // A new sharing: nothing to wait for from other devices, never "joined".
        crate::share::Begun::Started => vec![say("join-started", &[("host", host.clone()), ("place", place.trim().trim_matches('/').to_string())])],
        crate::share::Begun::Joined => {
            let after_text = std::fs::read_to_string(crate::backend::config_path()).unwrap_or_default();
            said_joined(&load_config(), before_text != after_text)
        }
    };
    // The contacts and calendars once everything the others shared came, never on half of it (M2).
    let agenda = if begun == crate::share::Begun::Started || complete_since(since) {
        bind_agenda(qt, shared, signed).and_then(|bound| line_of(bound, &host))
    } else {
        defer_agenda(signed, since);
        Some(say("join-agenda-later", &[("host", host.clone())]))
    };
    lines.extend(agenda);
    let needs = still_to_sign_in(&load_config());
    Ok((lines, needs))
}

/// Whether a whole look through the server went through since `since`
/// (Unix seconds), every other device's records, entries and claims brought
/// into this device's copy, nothing left there to bring, and an exchange read
/// them since: then the settings and the accounts your other devices share
/// are all here, and the contacts and calendars may be decided (M2).
fn complete_since(since: i64) -> bool {
    let state = remote::State::load(&crate::share::memory_path());
    state_complete(&state, since) && crate::share::last_exchange().is_some_and(|(at, _)| at >= since)
}

/// `complete_since`'s reading of the state: Sioul keeping the folder itself,
/// a whole step gone through at or after `since`, no error said since, nothing
/// too large left on the server.
fn state_complete(state: &remote::State, since: i64) -> bool {
    state.mode == remote::MIRROR && state.last >= since && state.said.is_empty() && state.left.is_empty()
}

/// The contacts and calendars left for a later exchange, the first one not
/// having brought everything: what to decide them with, this device's own
/// (`share/agenda-pending.toml`), never shared, never a secret (the app
/// password stays in the keyring, `loginflow::kept`).
#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
struct Pending {
    server: String,
    login: String,
    since: i64,
}

fn pending_path() -> std::path::PathBuf {
    state_dir().join("share").join("agenda-pending.toml")
}

fn defer_agenda(signed: &Grant, since: i64) {
    let pending = Pending { server: signed.server.clone(), login: signed.login.clone(), since };
    if let Ok(text) = toml::to_string(&pending) {
        let _ = sioul_sync::share::write_atomically(&pending_path(), format!("# Sioul: the contacts and calendars of the server signed in to, decided once your other devices' settings came (docs/database.md). Never shared.\n{text}").as_bytes());
    }
}

/// After an exchange of the window's: the contacts and calendars left for
/// later (`defer_agenda`) decided once a whole look and an exchange went
/// through, with the app password the keyring keeps; said in the status
/// line. Nothing while none is left. Off the window's thread.
pub(crate) fn agenda_later(qt: &QtThread, shared: &Arc<Shared>) {
    let path = pending_path();
    let Some(pending) = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str::<Pending>(&t).ok()) else { return };
    if !complete_since(pending.since) {
        return;
    }
    let host = remote::host_of(&pending.server);
    let Some(password) = loginflow::kept(&pending.login, &host) else {
        let _ = std::fs::remove_file(&path);
        return;
    };
    let signed = Grant::of(&pending.server, &pending.login, &password);
    let line = bind_agenda(qt, shared, &signed).and_then(|bound| line_of(bound, &host));
    let _ = std::fs::remove_file(&path);
    if let Some(line) = line {
        crate::backend::set_status(qt, line);
    }
}

// ---------------------------------------------------------------- the contacts and calendars

/// The user of a Nextcloud's address, as its paths name it
/// (`…/remote.php/dav/calendars/<user>/`, `…/addressbooks/users/<user>/`,
/// `…/principals/users/<user>/`, `…/files/<user>/`), decoded.
fn user_of(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("/remote.php/dav/")?;
    let mut parts = rest.split('/').filter(|p| !p.is_empty());
    let user = match parts.next()? {
        "calendars" | "files" => parts.next()?,
        "addressbooks" | "principals" => match parts.next()? {
            "users" => parts.next()?,
            _ => return None,
        },
        _ => return None,
    };
    Some(decoded(user)).filter(|u| !u.is_empty())
}

/// A path's part percent-decoded ("jane%40example.org" → "jane@example.org").
fn decoded(part: &str) -> String {
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let Some(byte) = part.get(at + 1..at + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            at += 3;
            continue;
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A server's name as an account says it: its `host`, else its address's.
fn account_host(account: &Account) -> String {
    account.host.clone().filter(|h| !h.is_empty()).unwrap_or_else(|| account.url.as_deref().map(remote::host_of).unwrap_or_default())
}

/// Whether an account's address is on `server`'s origin (scheme, host and
/// port), so that the sign-in's app password may go to it: an account
/// without an address, or one whose host is the server's but whose address
/// points elsewhere, never (the review of the join, L2).
fn on_server(account: &Account, server: &str) -> bool {
    let origin = sioul_sync::dav::origin(server);
    origin.is_some() && account.url.as_deref().and_then(sioul_sync::dav::origin) == origin
}

/// Your contacts-and-calendars account for that server and login, if there
/// is one (switched off ones too): its address on the server's origin
/// (`on_server`: scheme, host and port, never the host's name alone, which
/// Accounts keeps without its port), with the same login (case aside) or the
/// same user in its address (`user_of`). Google's are never.
fn matching<'a>(config: &'a Config, server: &str, login: &str, user: Option<&str>) -> Option<&'a Account> {
    config.every_account().filter(|a| a.kind == AccountKind::Dav && a.auth.is_none()).find(|a| {
        on_server(a, server) && (a.login().is_some_and(|l| l.trim().eq_ignore_ascii_case(login.trim())) || user.is_some_and(|u| a.url.as_deref().and_then(user_of).as_deref() == Some(u)))
    })
}

/// The id a new account for that server would take: "murena-agenda", as
/// Accounts names one (`<server's first name>-agenda`), free among yours.
fn new_id(config: &Config, host: &str) -> String {
    let label = host.split(['.', ':']).next().filter(|l| !l.is_empty()).unwrap_or("nextcloud");
    let base = config::slug(&format!("{label}-agenda"));
    std::iter::once(base.clone()).chain((2..).map(|n| format!("{base}-{n}"))).find(|id| config.every_account().all(|a| &a.id != id)).unwrap_or(base)
}

/// The id the sharing's state names for the account of that server and
/// login: yours when there is one, else the one it would be made under.
/// A name only: the folder is kept with the sign-in's own login (`State::user`).
fn agenda_id(config: &Config, signed: &Grant) -> String {
    matching(config, &signed.server, &signed.login, None).map_or_else(|| new_id(config, &signed.host()), |a| a.id.clone())
}

/// What becomes of the contacts and calendars of the server signed in to.
#[derive(Debug, Clone)]
enum Agenda {
    /// An account of yours, by its id: its own login is the sign-in's, the
    /// very same, so the app password opens it; else `opens` decides.
    Use { id: String, same: bool },
    /// None: one made.
    Make(Account),
}

/// The decision, from your accounts as they are after the first exchange
/// (those another device brought among them): the server as the sign-in
/// named it, the sign-in's login, its user as the homes name it, and where
/// its homes are.
fn agenda_for(config: &Config, server: &str, login: &str, homes: &sioul_sync::dav::Homes) -> Agenda {
    let host = remote::host_of(server);
    let user = homes.calendars.as_deref().or(homes.contacts.as_deref()).and_then(user_of);
    match matching(config, server, login, user.as_deref()) {
        Some(account) => Agenda::Use { id: account.id.clone(), same: account.login().is_some_and(|l| l.trim() == login.trim()) },
        None => {
            let found = homes.calendars.clone().or_else(|| homes.contacts.clone()).unwrap_or_default();
            Agenda::Make(Account::dav(&new_id(config, &host), login, &host, Some(&found), Some(login)))
        }
    }
}

/// What became of the contacts and calendars of the server signed in to (`bind_agenda`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    /// An account made from the sign-in, synced now.
    Made,
    /// Yours, given the app password, synced now.
    Kept,
    /// Yours, which keeps the password it has here.
    Own,
    /// Yours, which the app password does not open: it asks for its own (`still_to_sign_in`).
    Needs,
}

/// A binding's line, said with the rest: "use this sign-in" only where they
/// do (the review of the join, L4); none for an account that keeps its own
/// password, and none for one that needs one (its line under "Still to sign in").
fn line_of(bound: Bound, host: &str) -> Option<String> {
    match bound {
        Bound::Made => Some(say("join-agenda-made", &[("host", host.to_string())])),
        Bound::Kept => Some(say("join-agenda-kept", &[("host", host.to_string())])),
        Bound::Own | Bound::Needs => None,
    }
}

/// The contacts and calendars of the server signed in to, once the first
/// exchange brought your accounts: yours used, given the homes found there
/// when it has none here (its first sync then never looks for them
/// elsewhere with the app password: L1) and the app password when it has
/// none here and opens it there; else one made and synced. None when nothing
/// could be found there.
fn bind_agenda(qt: &QtThread, shared: &Arc<Shared>, signed: &Grant) -> Option<Bound> {
    let homes = loginflow::homes(signed).ok()?;
    let config = load_config();
    match agenda_for(&config, &signed.server, &signed.login, &homes) {
        Agenda::Use { id, same } => {
            let account = config.every_account().find(|a| a.id == id).cloned()?;
            if sioul_sync::dav::Homes::load(&account.id).is_empty() {
                let _ = homes.save(&account.id);
            }
            if secret::password(&account).is_ok() {
                return Some(Bound::Own);
            }
            let login = account.login().unwrap_or_default().to_string();
            if (same || loginflow::opens(signed, &login).unwrap_or(false)) && secret::save(&account, signed.password()).is_ok() {
                crate::pim::start_dav_watcher(qt, shared, account);
                return Some(Bound::Kept);
            }
            Some(Bound::Needs)
        }
        Agenda::Make(account) => {
            secret::save(&account, signed.password()).ok()?;
            config::add_dav_account(&crate::backend::config_path(), &account).ok()?;
            let _ = homes.save(&account.id);
            crate::pim::start_dav_watcher(qt, shared, account);
            crate::pim::show_pim(qt, shared);
            Some(Bound::Made)
        }
    }
}

// ---------------------------------------------------------------- what came

/// An account, as the join's lines name it: its address, or its contacts and calendars' server.
fn account_label(account: &Account) -> String {
    if account.kind == AccountKind::Dav {
        return say("join-agenda", &[("host", account_host(account))]);
    }
    account.address.clone().unwrap_or_else(|| account.id.clone())
}

/// A device as the join's lines name it: a computer by its name, a phone by its model.
fn device_label(entry: &sioul_sync::devices::Entry) -> String {
    match (entry.kind == sioul_sync::devices::PHONE, entry.name.is_empty() || entry.name == sioul_sync::devices::PHONE) {
        (true, true) => tr().text("device-the-phone", None),
        _ if entry.name.is_empty() => tr().text("share-other-device", None),
        _ => entry.name.clone(),
    }
}

/// Words joined as a list: "a, b and c".
fn listed(items: Vec<String>) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => say("join-and", &[("first", items[..n - 1].join(", ")), ("last", items[n - 1].clone())]),
    }
}

/// What the join brought, in sentences: who this device shares with now;
/// the settings, the accounts and the hours that came (`changed`: the
/// settings' file changed with the first exchange).
fn said_joined(config: &Config, changed: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let others: Vec<String> = crate::devices::entries().map(|(own, entries)| entries.iter().filter(|e| e.id != own && !e.left).map(device_label).collect()).unwrap_or_default();
    if others.is_empty() {
        lines.push(tr().text("join-alone", None));
        return lines;
    }
    lines.push(say("join-with", &[("names", listed(others))]));
    if changed {
        lines.push(tr().text("join-settings", None));
    }
    let accounts: Vec<String> = config.accounts.iter().filter(|a| a.syncs() || a.is_dav()).map(account_label).collect();
    if !accounts.is_empty() {
        lines.push(say("join-accounts", &[("accounts", listed(accounts))]));
    }
    let set = |kind: &str| config.windows.iter().any(|w| w.kind() == kind);
    let mut hours: Vec<String> = Vec::new();
    if set("work") {
        hours.push(tr().text("join-hours-work", None));
    }
    if set("admin") {
        hours.push(tr().text("join-hours-admin", None));
    }
    lines.push(if hours.is_empty() { tr().text("join-hours-none", None) } else { say("join-hours", &[("which", listed(hours))]) });
    lines
}

/// The accounts that came, or were here, and need a sign-in on this device:
/// their passwords never travel. One line each, saying where.
fn still_to_sign_in(config: &Config) -> Vec<String> {
    config
        .accounts
        .iter()
        .filter(|a| a.enabled && (a.syncs() || a.is_dav()))
        .filter(|a| secret::password(a).is_err())
        .map(|a| {
            let words = if a.auth.is_some() { "join-needs-sign-in" } else { "join-needs-password" };
            say(words, &[("account", account_label(a))])
        })
        .collect()
}

// ---------------------------------------------------------------- adding a device

/// A folder's place among a server's files, decoded part by part ("Documents/Mes%20dossiers" → "Documents/Mes dossiers").
pub(crate) fn decoded_path(place: &str) -> String {
    place.split('/').map(decoded).collect::<Vec<_>>().join("/")
}

/// What a new device needs, in a few lines, for Settings ▸ Your folder and
/// sharing ▸ "Add a device" on a computer that shares: where to find the
/// join on it; the server to sign in to and the folder's place there, when
/// the folder is known on a Nextcloud (`found`: its name and the place); else
/// the same folder, carried by a sync app; and the passphrase, the one you
/// chose, never shown.
pub(crate) fn add_device(found: Option<(String, String)>) -> Vec<String> {
    let mut lines = vec![tr().text("share-add-intro", None)];
    match found {
        Some((host, place)) => {
            lines.push(if host.eq_ignore_ascii_case(&remote::host_of(loginflow::MURENA)) { tr().text("share-add-murena", None) } else { say("share-add-nextcloud", &[("host", host)]) });
            // "Found by itself" only of the places the look goes through (L5); another, to type.
            lines.push(say(if remote::sought(&place) { "share-add-place" } else { "share-add-place-typed" }, &[("place", place)]));
        }
        None => lines.push(tr().text("share-add-folder", None)),
    }
    lines.push(tr().text("share-add-passphrase", None));
    lines
}

// ---------------------------------------------------------------- stopping

/// Sharing stopped where it was joined through the server's sign-in
/// (`State::user`): its app password forgotten here, and given back to the
/// server unless one of your accounts here uses the same (its contacts and
/// calendars, `bind_agenda`). Off the window's thread.
pub(crate) fn let_go(state: &remote::State) {
    if state.user.is_empty() {
        return;
    }
    let host = state.host();
    let Some(password) = loginflow::kept(&state.user, &host) else { return };
    let used = load_config().every_account().any(|a| secret::password(a).is_ok_and(|p| p == password));
    if !used {
        let server = state.url.split_once("/remote.php/").map_or(state.url.as_str(), |(server, _)| server);
        let _ = loginflow::revoke(&Grant::of(server, &state.user, &password));
    }
    let _ = loginflow::forget(&state.user, &host);
}

// ---------------------------------------------------------------- a phone in another's place

/// A phone this one may replace, asked once (`replacing`): its id, and the
/// question in words.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Replacing {
    id: String,
    ask: String,
}

/// The phones asked about already, by id, whatever the answer: never asked
/// again. This device's own (`share/replacing.toml`), never shared.
#[derive(Serialize, Deserialize, Default, Debug)]
struct Asked {
    #[serde(default)]
    answered: Vec<String>,
}

fn asked_path() -> std::path::PathBuf {
    state_dir().join("share").join("replacing.toml")
}

fn asked() -> Asked {
    std::fs::read_to_string(asked_path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

/// A moment, said shortly: "14:05" today, "Fri 2 Oct" before.
fn moment(seconds: i64) -> String {
    let Ok(at) = jiff::Timestamp::from_second(seconds) else { return String::new() };
    let at = at.to_zoned(jiff::tz::TimeZone::system());
    if at.date() == jiff::Zoned::now().date() { at.strftime("%H:%M").to_string() } else { tr().date(&at, true) }
}

/// How long another phone says nothing, as this device sees it, before this
/// one asks whether it replaces it (`to_ask`): a day, as a computer offers
/// "Forget it now" (`health::FORGET_NOW_AFTER`). A phone in use goes silent
/// for hours at times (no network, switched off, Android deferring its
/// steps), and a Yes about one would leave the doses without its word (the
/// review of the join, M1); judged by this device's sight of its entry and
/// its own last word, the later, never by its clock alone.
const ASK_AFTER: i64 = 86_400;

/// The latest time an entry says, by its device's clock.
fn latest_of(entry: &sioul_sync::devices::Entry) -> i64 {
    entry.started.max(entry.closed).max(entry.imported).max(entry.exported)
}

/// Of the other phones in the folder, the one to ask about, if any: on a
/// phone sharing, one that has not stopped sharing, nor said anything for
/// `ASK_AFTER` as this device sees it (`quiet_since`: the later of when this
/// device saw its entry change and its own last word; none known: not
/// asked), nor been asked about (`Asked`), nor been replaced or forgotten
/// here; the one heard of last.
#[allow(clippy::too_many_arguments)]
fn to_ask(own: &str, own_entry: &sioul_sync::devices::Entry, entries: &[sioul_sync::devices::Entry], answered: &[String], forgotten: &dyn Fn(&str) -> bool, quiet_since: &dyn Fn(&str) -> Option<i64>, now: i64) -> Option<sioul_sync::devices::Entry> {
    entries
        .iter()
        .filter(|e| e.id != own && e.kind == sioul_sync::devices::PHONE && !e.left && quiet_since(&e.id).is_some_and(|since| now - since >= ASK_AFTER))
        .filter(|e| !answered.contains(&e.id) && own_entry.replaces.iter().all(|r| r.id != e.id) && !forgotten(&e.id))
        .max_by_key(|e| sioul_sync::devices::written(e))
        .cloned()
}

/// "Does this phone replace GS290, silent since …?", asked once on a phone that
/// shares while another phone shares too (the owner's, 16 October 2026: the
/// old phone dead, its battery gone, never to say Stop sharing). None
/// elsewhere, and once answered.
pub(crate) fn replacing() -> Option<Replacing> {
    if crate::devices::kind() != sioul_sync::devices::PHONE {
        return None;
    }
    let (own, entries) = crate::devices::entries()?;
    let own_entry = sioul_sync::devices::own(&sioul_sync::devices::own_path(&state_dir()));
    let other = to_ask(&own, &own_entry, &entries, &asked().answered, &crate::health::forgotten_here, &crate::health::quiet_since, jiff::Timestamp::now().as_second())?;
    let when = moment(latest_of(&other));
    Some(Replacing { ask: say("join-replace-ask", &[("name", device_label(&other)), ("when", when)]), id: other.id })
}

/// The answer: yes, this phone's entry says it replaces that one (sealed,
/// for every device: `devices::Entry::replaces`), forgotten here at once,
/// and an exchange tells the others; no, a second phone, nothing changes.
/// Asked once either way. Nothing of the other phone is deleted.
pub(crate) fn replace(qt: &QtThread, shared: &Arc<Shared>, id: &str, yes: bool) -> String {
    let id = id.trim().to_string();
    if id.is_empty() {
        return String::new();
    }
    let mut kept = asked();
    if !kept.answered.contains(&id) {
        kept.answered.push(id.clone());
    }
    let written = toml::to_string(&kept).map_err(|e| e.to_string()).and_then(|text| sioul_sync::share::write_atomically(&asked_path(), format!("# Sioul: the phones this one was asked whether it replaces (docs/database.md). Never shared.\n{text}").as_bytes()));
    if let Err(e) = written {
        return e;
    }
    if !yes {
        return String::new();
    }
    let replaced = crate::health::replaced_now(&id);
    crate::devices::replace(replaced);
    let problem = crate::health::device_forget(&id);
    crate::share::exchange(qt, shared);
    problem
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_sync::dav::Homes;

    fn config(accounts: &str) -> Config {
        toml::from_str(accounts).unwrap()
    }

    #[test]
    fn a_nextcloud_s_user_is_read_from_its_paths() {
        assert_eq!(user_of("https://murena.io/remote.php/dav/calendars/jane/").as_deref(), Some("jane"));
        assert_eq!(user_of("https://cloud.example.org/nc/remote.php/dav/addressbooks/users/jane%40example.org/").as_deref(), Some("jane@example.org"));
        assert_eq!(user_of("https://murena.io/remote.php/dav/principals/users/jane/").as_deref(), Some("jane"));
        assert_eq!(user_of("https://murena.io/remote.php/dav/files/jane/Documents/Sioul/").as_deref(), Some("jane"));
        assert_eq!(user_of("https://murena.io/remote.php/dav/"), None);
        assert_eq!(user_of("https://caldav.example.org/cal/jane/"), None);
    }

    /// The account made from a sign-in (docs/database.md, "Joining through
    /// your Nextcloud's sign-in"): none of yours for that server and login,
    /// one is made at its homes, named as Accounts names one; yours for
    /// them, come with the shared settings or here already, used instead,
    /// never a second: by its login (case aside), by the user its address
    /// names; matched by its address's origin, so that a server on its own
    /// port is found again, and an account naming the server's host but
    /// pointing elsewhere never receives the app password (M2, L2); another
    /// login's, another server's, Google's, never.
    #[test]
    fn the_account_is_made_from_the_sign_in_or_yours_is_used() {
        let homes = Homes { calendars: Some("https://murena.io/remote.php/dav/calendars/jane/".into()), contacts: Some("https://murena.io/remote.php/dav/addressbooks/users/jane/".into()) };
        let server = "https://murena.io";
        // None yet: made, at the calendars' home, its login the sign-in's.
        let empty = config("");
        let Agenda::Make(made) = agenda_for(&empty, server, "jane", &homes) else { panic!("made") };
        assert_eq!((made.id.as_str(), made.kind, made.host.as_deref(), made.url.as_deref(), made.login()), ("murena-agenda", AccountKind::Dav, Some("murena.io"), Some("https://murena.io/remote.php/dav/calendars/jane/"), Some("jane")));
        assert_eq!(agenda_id(&empty, &Grant::of(server, "jane", "x")), "murena-agenda");
        // The desk's, come with the settings: used, never a second.
        let desk = config("[[account]]\nid = \"murena\"\nkind = \"dav\"\naddress = \"Jane\"\nhost = \"murena.io\"\nurl = \"https://murena.io/remote.php/dav/calendars/jane/\"\n");
        assert!(matches!(agenda_for(&desk, server, "jane", &homes), Agenda::Use { id, same: false } if id == "murena"), "its login Jane: the same account, the app password tried with it first");
        let exact = config("[[account]]\nid = \"murena\"\nkind = \"dav\"\naddress = \"jane\"\nhost = \"murena.io\"\nurl = \"https://murena.io/remote.php/dav/calendars/jane/\"\n");
        assert!(matches!(agenda_for(&exact, server, "jane", &homes), Agenda::Use { id, same: true } if id == "murena"));
        assert_eq!(agenda_id(&desk, &Grant::of(server, "jane", "x")), "murena");
        // Its login the address, the sign-in's the user: the same account, by the user its address names.
        let by_address = config("[[account]]\nid = \"cloud\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"murena.io\"\nurl = \"https://murena.io/remote.php/dav/calendars/jane/\"\n");
        assert!(matches!(agenda_for(&by_address, server, "jane", &homes), Agenda::Use { id, same: false } if id == "cloud"));
        // A Nextcloud on its own port: Accounts keeps its host without the port; its address's origin finds it again.
        let ported = config("[[account]]\nid = \"cloud\"\nkind = \"dav\"\naddress = \"jane\"\nhost = \"cloud.example.org\"\nurl = \"https://cloud.example.org:8443/nc/remote.php/dav/calendars/jane/\"\n");
        let ported_homes = Homes { calendars: Some("https://cloud.example.org:8443/nc/remote.php/dav/calendars/jane/".into()), contacts: None };
        assert!(matches!(agenda_for(&ported, "https://cloud.example.org:8443/nc", "jane", &ported_homes), Agenda::Use { id, .. } if id == "cloud"));
        // The server's host in its name, its address elsewhere (a hand's edit): never given the app password.
        let elsewhere = config("[[account]]\nid = \"odd\"\nkind = \"dav\"\naddress = \"jane\"\nhost = \"murena.io\"\nurl = \"https://dav.example.org/remote.php/dav/calendars/jane/\"\n");
        assert!(matches!(agenda_for(&elsewhere, server, "jane", &homes), Agenda::Make(_)));
        assert!(!on_server(&elsewhere.accounts[0], server) && on_server(&exact.accounts[0], server) && !on_server(&exact.accounts[0], "http://murena.io"));
        // Another login, another server, Google's: none of them; made, under a free name.
        let others = config(
            "[[account]]\nid = \"murena-agenda\"\nkind = \"dav\"\naddress = \"paul@example.org\"\nhost = \"murena.io\"\nurl = \"https://murena.io/remote.php/dav/calendars/paul/\"\n\n[[account]]\nid = \"work\"\nkind = \"dav\"\naddress = \"jane\"\nhost = \"cloud.example.org\"\nurl = \"https://cloud.example.org/remote.php/dav/calendars/jane/\"\n\n[[account]]\nid = \"google\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"murena.io\"\nauth = \"google\"\n",
        );
        let Agenda::Make(made) = agenda_for(&others, server, "jane", &homes) else { panic!("made") };
        assert_eq!(made.id, "murena-agenda-2");
    }

    /// The desk's Murena account arriving late (the review of the join, M2):
    /// a first exchange that did not bring everything (the time out, an error
    /// said, records left on the server, no exchange since) decides nothing,
    /// so no account is made beside the desk's, which the later exchange
    /// brings: used then, never doubled, on any device (an account made here
    /// would travel to the others under another id).
    #[test]
    fn the_contacts_and_calendars_wait_for_a_complete_first_exchange() {
        let since: i64 = 1_791_360_000;
        let whole = remote::State { mode: remote::MIRROR.into(), last: since + 20, ..remote::State::default() };
        assert!(state_complete(&whole, since));
        assert!(!state_complete(&remote::State { last: since - 1, ..whole.clone() }, since), "no whole look since the join");
        assert!(!state_complete(&remote::State { said: "network:murena.io".into(), ..whole.clone() }, since), "the look cut by the time out, or an error");
        assert!(!state_complete(&remote::State { left: [("desk-1.jsonl".to_string(), 70 << 20)].into(), ..whole.clone() }, since), "records left on the server");
        assert!(!state_complete(&remote::State { mode: String::new(), ..whole.clone() }, since), "not Sioul keeping the folder");
        // Decided at the first exchange, the desk's account not here yet, one would be made beside it;
        // decided once it came, the desk's is used.
        let homes = Homes { calendars: Some("https://murena.io/remote.php/dav/calendars/jane/".into()), contacts: None };
        assert!(matches!(agenda_for(&config(""), "https://murena.io", "jane", &homes), Agenda::Make(_)), "what deciding early would do");
        let arrived = config("[[account]]\nid = \"murena\"\nkind = \"dav\"\naddress = \"jane\"\nhost = \"murena.io\"\nurl = \"https://murena.io/remote.php/dav/calendars/jane/\"\n");
        assert!(matches!(agenda_for(&arrived, "https://murena.io", "jane", &homes), Agenda::Use { id, .. } if id == "murena"));
        // What each binding says: "use this sign-in" only where they do (L4).
        assert!(line_of(Bound::Kept, "murena.io").is_some_and(|l| l.contains("murena.io")) && line_of(Bound::Made, "murena.io").is_some());
        assert_eq!((line_of(Bound::Own, "murena.io"), line_of(Bound::Needs, "murena.io")), (None, None));
    }

    /// What the look found, said (M3): places found are proposed; none found
    /// with every place looked at, a new sharing may start; the look
    /// incomplete or not possible, "could not look", never "none found".
    #[test]
    fn a_look_that_could_not_go_through_never_offers_a_new_sharing() {
        let mut stand = Stand::default();
        looked(&mut stand, "murena.io", Ok(remote::Sought { found: vec!["Documents/Sioul".into()], complete: false }));
        assert_eq!((stand.step.as_str(), stand.places.as_slice()), ("found", ["Documents/Sioul".to_string()].as_slice()));
        looked(&mut stand, "murena.io", Ok(remote::Sought { found: Vec::new(), complete: true }));
        assert_eq!(stand.step, "none");
        assert!(stand.places.is_empty() && stand.problem.is_empty());
        looked(&mut stand, "murena.io", Ok(remote::Sought { found: Vec::new(), complete: false }));
        assert_eq!(stand.step, "unchecked");
        assert!(stand.problem.contains("murena.io"), "{}", stand.problem);
        looked(&mut stand, "murena.io", Err("network:murena.io".into()));
        assert_eq!(stand.step, "unchecked");
        // "Add a device" says "found by itself" only of the places the look goes through.
        let deep = add_device(Some(("murena.io".into(), "Documents/Partage/Sioul".into())));
        let own = add_device(Some(("murena.io".into(), "Documents/Sioul".into())));
        assert_ne!(deep[2], own[2]);
        assert!(deep[2].contains("Documents/Partage/Sioul") && own[2].contains("Documents/Sioul"));
    }

    /// "Does this phone replace …?", asked once (the owner's, 16 October
    /// 2026): of the other phones that have not stopped sharing, the one
    /// heard of last, silent a day as this device sees it (M1: never by the
    /// other phone's clock alone; a phone first seen now waits a day); never
    /// one asked about, replaced or forgotten here, nor a computer.
    #[test]
    fn a_phone_is_asked_once_whether_it_replaces_another() {
        use sioul_sync::devices::{COMPUTER, Entry, PHONE, Replaced};
        let now: i64 = 1_791_360_000;
        let phone = |id: &str, exported: i64| Entry { id: id.into(), name: format!("{id} model"), kind: PHONE.into(), started: exported - 60, exported, working: false, ..Entry::default() };
        let desk = Entry { id: "desk".into(), kind: COMPUTER.into(), started: now - 900, exported: now - 10, working: true, ..Entry::default() };
        let own = phone("new", now);
        let entries = vec![own.clone(), desk, phone("old", now - 3 * 86_400), phone("older", now - 9 * 86_400), Entry { left: true, ..phone("gone", now - 86_400) }, phone("second", now - 600)];
        let never = |_: &str| false;
        // This device's view: the later of when it saw each entry change and its own last word.
        let seen = |id: &str| match id {
            "old" => Some(now - 3 * 86_400),
            "older" => Some(now - 9 * 86_400),
            "second" => Some(now - 600),
            "fresh" => Some(now - 60),
            _ => None,
        };
        assert_eq!(to_ask("new", &own, &entries, &[], &never, &seen, now).map(|e| e.id), Some("old".into()), "the one heard of last, silent a day or more");
        assert_eq!(to_ask("new", &own, &entries, &["old".into()], &never, &seen, now).map(|e| e.id), Some("older".into()), "asked once");
        let replacing = Entry { replaces: vec![Replaced { id: "old".into(), started: now - 3 * 86_400 - 60, heard: (now - 3 * 86_400) * 1000 }], ..own.clone() };
        assert_eq!(to_ask("new", &replacing, &entries, &["older".into()], &never, &seen, now), None, "replaced already");
        assert_eq!(to_ask("new", &own, &entries, &["older".into()], &|id| id == "old", &seen, now), None, "forgotten here");
        assert_eq!(to_ask("new", &own, &entries[..2], &[], &never, &seen, now), None, "a computer is never asked about");
        // A phone in use, heard ten minutes ago, or silent five hours (no network): a second phone, never asked about.
        assert_eq!(to_ask("new", &own, &[own.clone(), phone("second", now - 600)], &[], &never, &seen, now), None);
        assert_eq!(to_ask("new", &own, &[own.clone(), phone("second", now - 600)], &[], &never, &seen, now + 5 * 3_600), None, "five hours of silence are no proof");
        assert_eq!(to_ask("new", &own, &[own.clone(), phone("second", now - 600)], &[], &never, &seen, now + ASK_AFTER).map(|e| e.id), Some("second".into()), "a day of silence: asked");
        // Its own clock three days back, but first seen here a minute ago: this device's view decides, a day on.
        assert_eq!(to_ask("new", &own, &[own.clone(), phone("fresh", now - 3 * 86_400)], &[], &never, &seen, now), None);
        // Nothing known of it here yet: not asked.
        assert_eq!(to_ask("new", &own, &[own.clone(), phone("unknown", now - 9 * 86_400)], &[], &never, &seen, now), None);
    }
}

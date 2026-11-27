// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The invitations Sioul sends itself (the owner's word of 26 November 2026:
//! "Use Nextcloud invites for events if available, fallback to direct
//! emails sent by Sioul"; docs/client.md, "Who tells the guests").
//!
//! - **The outbox**: each message to the guests of a meeting you organize
//!   that its calendar's server does not send (an invitation, its update, a
//!   cancellation: iMIP, RFC 6047) is put in this device's outbox
//!   (`Draft::outbox_folder`, in its state folder, which the sharing never
//!   carries) and sent from the organizer's own address once the change's
//!   Undo is over (`DELAY`), never shown in Mail's Drafts. No account sends
//!   from that address, or it cannot send now (no password, its server
//!   refusing): it waits, the event's card says why in one line with the way
//!   to Accounts (`card_line`), and it leaves once the account works.
//! - **One device sends each change**: the one that wrote it, whose outbox
//!   holds its message; another device of yours never sends for a change it
//!   did not write (it sees the event by the sync, never its outbox).
//! - **The server's word read back** (`check_served`): where the calendar's
//!   server schedules for the organizer's address (RFC 6638, Nextcloud), it
//!   sends, and Sioul reads, after the sync that follows the write and only
//!   while the server's copy is still that write (`Watched::mark`), each
//!   guest's `SCHEDULE-STATUS` on the first VEVENT, the only one the server
//!   writes it on: 1.x or 2.x sent; 3.x or 5.x not sent. A guest that write
//!   invited, when no guest of the first VEVENT carries any word ten minutes
//!   after the write and a pull since, was not sent either: the server did
//!   not schedule at all. Sioul then sends it itself, that guest left to it
//!   (`SCHEDULE-AGENT=CLIENT`) from then on, so that no later change reaches
//!   them twice; its message is saved first, the mark written only over the
//!   text read, and the message leaves only once the server's copy holds the
//!   mark, stamped when it leaves (`take_over`). The server's word is read on
//!   its own copy alone (`vdir::server_copy`: its ETag known, no send of it
//!   waiting to be fetched back). Any other silence decides nothing by
//!   itself: an invited guest whose server spoke of others but not of them
//!   is said on the event's card, with **Send it by mail** (`send_by_mail`);
//!   a guest of some times alone is the server's, which tells them and never
//!   reports on them: not watched.
//! - **Claimed before it is sent** (`send_due`): a message is renamed
//!   `<id>.sending` before it goes, so that no Undo, no status change and no
//!   second pass takes it or sends it again meanwhile; one found so after a
//!   crash is never sent again by itself: its card says Sioul does not know
//!   whether it left, with **Send it again** (`send_again`).
//! - **Meetings made before** (their guests left to you, a draft left in
//!   Mail): nothing is sent for them by itself; their card offers "Send the
//!   invitation now" (`send_now`).

use crate::backend::{QtThread, Shared, load_config, say, tr};
use serde::{Deserialize, Serialize};
use sioul_core::agenda;
use sioul_core::compose::Draft;
use sioul_core::config::Account;
use sioul_core::lines;
use sioul_core::organizer::{self, Delivery, Guest};
use sioul_core::vdir;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// Seconds a message waits in the outbox before it is sent: the Undo of the
/// change it tells (mail's ten seconds, `mail::UNDO_SECONDS`), and two more.
pub(crate) const DELAY: i64 = 12;
/// Seconds after a write that its server's word on a guest may be missing
/// before Sioul sends that guest's message itself (once a pull came since).
pub(crate) const SERVER_WAIT: i64 = 10 * 60;
/// Seconds before a message its account refused is tried again.
const RETRY: i64 = 120;

fn now() -> i64 {
    sioul_core::zone::now().timestamp().as_second()
}

/// The account of yours that sends mail from `address`, the organizer's.
pub(crate) fn account_for(address: &str) -> Option<Account> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    load_config().accounts.into_iter().find(|a| a.syncs() && a.address.as_deref().is_some_and(|x| x.trim().eq_ignore_ascii_case(address)))
}

/// A message made for the outbox: marked so, stamped now.
pub(crate) fn queued(mut draft: Draft) -> Draft {
    draft.outbox = true;
    draft.queued = now();
    draft.waiting.clear();
    draft
}

// ---------------------------------------------------------------- what is watched

/// A write whose server-told guests' delivery is read back.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Watched {
    path: PathBuf,
    /// Their addresses, lower case.
    guests: Vec<String>,
    /// Those of them the write invited, or asked the server to tell
    /// (`SCHEDULE-FORCE-SEND`): a server that schedules always writes a word
    /// on them (sabre's `deliver` sets 5.2 when nothing else did).
    #[serde(default)]
    due: Vec<String>,
    /// When it was written (Unix seconds).
    written: i64,
    /// What it wrote (`organizer::written_mark`): the server's word is read
    /// while its copy is still this write, never another device's later one.
    #[serde(default)]
    mark: String,
    /// It told them the meeting is cancelled.
    #[serde(default)]
    cancelled: bool,
    /// The guests the server gave no sign for: said on the card, sent by mail on your press.
    #[serde(default)]
    silent: Vec<String>,
}

/// This device's own memory of the invitations (`state/invitations.toml`), never shared.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Memory {
    #[serde(default)]
    watched: Vec<Watched>,
}

fn memory_path() -> PathBuf {
    sioul_core::config::state_dir().join("invitations.toml")
}

fn with_memory<R>(change: impl FnOnce(&mut Memory) -> R) -> R {
    static LOCK: Mutex<()> = Mutex::new(());
    let _held = LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let path = memory_path();
    let mut memory: Memory = crate::files::read_to_string(&path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    let before = memory.clone();
    let out = change(&mut memory);
    if memory != before
        && let Ok(text) = toml::to_string(&memory)
    {
        if let Some(folder) = path.parent() {
            let _ = crate::files::create_dir_all(folder);
        }
        let _ = sioul_core::filelock::replace(&path, text.as_bytes());
    }
    out
}

/// A write through the organizer's door (`text`, as written) whose guests
/// `guests` its server tells, `due` those it invited or asked the server to
/// tell: their delivery read back after the sync (`check_served`). One entry
/// per meeting, the latest write's.
pub(crate) fn watch(path: &Path, text: &str, guests: &[String], due: &[String], cancelled: bool) {
    let entry = Watched { path: path.to_path_buf(), guests: guests.to_vec(), due: due.to_vec(), written: now(), mark: organizer::written_mark(text), cancelled, silent: Vec::new() };
    with_memory(|m| {
        m.watched.retain(|w| w.path != entry.path);
        m.watched.push(entry);
    });
}

/// Whether an iTIP message is a REQUEST (an invitation or its update).
fn is_request(ics: &str) -> bool {
    lines::unfold(ics).iter().any(|l| lines::name(l) == "METHOD" && lines::value(l).trim().eq_ignore_ascii_case("REQUEST"))
}

/// The invitations of a meeting made before 26 November 2026 still waiting
/// in Mail's Drafts (Sioul left them there for you to send): carried to
/// your other devices by the sharing, and gone from all of them once sent
/// or taken out, so that every device says the same of it.
fn left_in_drafts(uid: &str, own: &[String]) -> Vec<Draft> {
    organizer::pending_of(uid, own).into_iter().filter_map(|p| Draft::by_id(&p.id)).filter(|d| !d.outbox && d.calendar.as_deref().is_some_and(is_request)).collect()
}

// ---------------------------------------------------------------- sending

/// The thread that sends: woken by each write that queued a message
/// (`wake`), else once a minute, or sooner when a message falls due.
static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// The outbox told that something waits.
pub(crate) fn wake() {
    let (lock, ready) = &WAKE;
    *lock.lock().unwrap_or_else(PoisonError::into_inner) = true;
    ready.notify_all();
}

/// The sending thread started, once, by the window: what it did said on the status line.
pub(crate) fn start(qt: &QtThread, shared: &Arc<Shared>) {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        loop {
            let lines = run(now(), &send_mail, &|account| crate::pim::nudge(&shared, account), &|_| true);
            for line in lines {
                crate::backend::tell(&qt, &shared, line);
            }
            let wait = next_in(now());
            let (lock, ready) = &WAKE;
            let mut woken = lock.lock().unwrap_or_else(PoisonError::into_inner);
            if !*woken {
                woken = ready.wait_timeout(woken, std::time::Duration::from_secs(wait)).unwrap_or_else(PoisonError::into_inner).0;
            }
            *woken = false;
        }
    });
}

/// Seconds until the outbox needs looking at again: its first message due, a
/// minute at most. A message waiting for its sync (`after_sync`) is left out:
/// the sync's own nudge and `wake` are enough, and a phone never wakes each second for it.
fn next_in(now: i64) -> u64 {
    let due = Draft::outbox_all().iter().filter(|d| d.waiting.is_empty() && d.after_sync.is_none()).map(|d| d.queued + DELAY - now).min().unwrap_or(60);
    u64::try_from(due.clamp(1, 60)).unwrap_or(60)
}

/// One pass: the servers' word read back, then each message due sent.
/// `send` sends one message from an account (`send_mail`; a stand-in in the
/// tests), `nudge` asks an account's sync, `scope` says which meetings, by
/// UID, this pass looks at (all of them; a test's own). What to say.
pub(crate) fn run(now: i64, send: &dyn Fn(&Account, &Draft) -> Result<(), String>, nudge: &dyn Fn(&str), scope: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut lines = check_served(now, nudge, scope);
    lines.extend(send_due(now, send, nudge, scope));
    lines
}

/// One message sent from `account` (its Sent copy kept, as Mail sends);
/// none in the demo profile, which reaches no server. What stops it, in a sentence.
fn send_mail(account: &Account, draft: &Draft) -> Result<(), String> {
    use sioul_sync::SyncError;
    if crate::backend::offline() {
        return Ok(());
    }
    let mut moved = None;
    let sent = sioul_sync::secret::password(account).and_then(|p| sioul_sync::send::send_draft(&crate::backend::config_path(), account, &p, draft, tr(), &mut moved));
    match sent {
        Ok(()) | Err(SyncError::NotFiled(_) | SyncError::PartlySent { .. }) => Ok(()),
        Err(e) => Err(e.sentence(tr(), &account.id)),
    }
}

/// The organizer's address an iTIP message names.
fn organizer_in(ics: &str) -> String {
    organizer::organizer_of(ics).map(|o| o.address).unwrap_or_default()
}

/// Whom a message goes to, in words.
fn names(draft: &Draft) -> String {
    draft.to.iter().map(|t| t.split('<').next().map(str::trim).filter(|n| !n.is_empty()).map_or_else(|| sioul_core::compose::address_of(t).unwrap_or_else(|| t.clone()), str::to_string)).collect::<Vec<_>>().join(", ")
}

/// Its recipients' addresses, lower case.
fn recipients(draft: &Draft) -> Vec<String> {
    draft.to.iter().map(|t| sioul_core::compose::address_of(t).unwrap_or_else(|| t.trim().to_string()).to_lowercase()).collect()
}

/// When this process last tried each message its account refused.
static TRIED: Mutex<BTreeMap<String, i64>> = Mutex::new(BTreeMap::new());
/// The messages this process is sending now (claimed, `<id>.sending`): any
/// other claimed message was left by a process that stopped while sending it.
static CLAIMED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// Where a message is while it is being sent.
fn sending_path(id: &str) -> PathBuf {
    Draft::outbox_folder().join(format!("{id}.sending"))
}

/// The outbox's claimed messages (`<id>.sending`) another process left: not
/// known to have left, never sent again by themselves.
fn unsure() -> Vec<Draft> {
    let Ok(entries) = crate::files::read_dir(Draft::outbox_folder()) else { return Vec::new() };
    let claimed = CLAIMED.lock().unwrap_or_else(PoisonError::into_inner).clone();
    entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "sending")).filter_map(|p| Draft::load(&p)).filter(|d| !claimed.contains(&d.id)).collect()
}

/// A message's file in the outbox given back from its claim, with why it waits.
fn unclaim(draft: &mut Draft, why: Option<String>) {
    CLAIMED.lock().unwrap_or_else(PoisonError::into_inner).remove(&draft.id);
    if crate::files::rename(sending_path(&draft.id), draft.path()).is_err() {
        return;
    }
    if let Some(why) = why.filter(|w| *w != draft.waiting) {
        draft.waiting = why;
        let _ = draft.save();
    }
}

/// A claimed message done with (sent, or no longer holding).
fn finished(id: &str) {
    let _ = crate::files::remove_file(sending_path(id));
    CLAIMED.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
}

/// The time as an iCalendar UTC stamp ("20991110T090000Z").
fn stamp(at: i64) -> String {
    jiff::Timestamp::from_second(at).map(|t| t.strftime("%Y%m%dT%H%M%SZ").to_string()).unwrap_or_default()
}

/// Each message of the outbox due (its Undo over) claimed (`<id>.sending`:
/// from then on no Undo, status or pass takes it; `Draft::by_id` no longer
/// finds it, so whoever looks counts it heard), then sent from the account
/// that sends from its organizer's address, stamped when it leaves
/// (`organizer::stamped`). A take-over's message (`after_sync`) leaves only
/// once the server's copy holds every one of its guests left to you; a copy
/// that came back without the mark discards it and that write is watched
/// again. Sent, it leaves the outbox. No such account, or one that cannot
/// send: back in the outbox with why (`Draft::waiting`), said on the event's
/// card, tried again two minutes after a refusal.
fn send_due(now: i64, send: &dyn Fn(&Account, &Draft) -> Result<(), String>, nudge: &dyn Fn(&str), scope: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut lines = Vec::new();
    let config = load_config();
    for mut draft in Draft::outbox_all() {
        if now - draft.queued < DELAY || !scope(&draft.calendar.as_deref().map(uid_of).unwrap_or_default()) {
            continue;
        }
        if TRIED.lock().unwrap_or_else(PoisonError::into_inner).get(&draft.id).is_some_and(|at| now - at < RETRY) {
            continue;
        }
        // Claimed: renamed out of every other reader's way; another pass got it first, skipped.
        CLAIMED.lock().unwrap_or_else(PoisonError::into_inner).insert(draft.id.clone());
        if crate::files::rename(draft.path(), sending_path(&draft.id)).is_err() {
            CLAIMED.lock().unwrap_or_else(PoisonError::into_inner).remove(&draft.id);
            continue;
        }
        if let Some(path) = draft.after_sync.clone() {
            let Ok(text) = crate::files::read_to_string(&path) else {
                // Its meeting gone meanwhile (deleted, its cancellation told): it no longer holds.
                finished(&draft.id);
                continue;
            };
            if !vdir::server_copy(&path) {
                unclaim(&mut draft, None);
                continue;
            }
            let clients = organizer::clients(&text);
            let to = recipients(&draft);
            if !to.iter().all(|t| clients.contains(t)) {
                // Its server kept a copy without the mark: still the server's to tell. Watched again
                // only while that copy is the write the take-over started from; another device's
                // newer write is that device's to watch.
                finished(&draft.id);
                if !draft.taken_from.is_empty() && organizer::written_mark(&text) == draft.taken_from {
                    let cancelled = draft.calendar.as_deref().is_some_and(|ics| !is_request(ics));
                    watch(&path, &text, &to, &to, cancelled);
                }
                continue;
            }
        }
        let organizer = draft.calendar.as_deref().map(organizer_in).unwrap_or_default();
        // The account it was made for while it sends from that address, else the one that does now.
        let account = config.accounts.iter().find(|a| a.id == draft.account && a.syncs() && a.address.as_deref().is_some_and(|x| x.trim().eq_ignore_ascii_case(organizer.trim()))).cloned().or_else(|| account_for(&organizer));
        let Some(account) = account else {
            unclaim(&mut draft, Some(say("invite-why-no-account", &[("address", organizer.clone())])));
            continue;
        };
        draft.account = account.id.clone();
        draft.calendar = draft.calendar.as_deref().map(|ics| organizer::stamped(ics, &stamp(now)));
        match send(&account, &draft) {
            Ok(()) => {
                finished(&draft.id);
                TRIED.lock().unwrap_or_else(PoisonError::into_inner).remove(&draft.id);
                nudge(&account.id);
                lines.push(say("invite-sent", &[("who", names(&draft))]));
            }
            Err(why) => {
                TRIED.lock().unwrap_or_else(PoisonError::into_inner).insert(draft.id.clone(), now);
                unclaim(&mut draft, Some(why));
            }
        }
    }
    lines
}

// ---------------------------------------------------------------- the server's word

/// Whether a meeting's last time is over: nothing is sent for it any more
/// (Nextcloud sends nothing for a meeting gone by either).
fn gone_by(text: &str, now: i64) -> bool {
    let zone = sioul_core::zone::current();
    let repeats = lines::unfold(text).iter().any(|l| matches!(lines::name(l).as_str(), "RRULE" | "RDATE"));
    !repeats && agenda::first_times(text, &zone).is_some_and(|(_, end)| end <= now)
}

/// Whether this device pulled a calendar file's calendar since `at` (`State::pulled`).
fn pulled_since(path: &Path, at: i64) -> bool {
    crate::pim::collection_of(path).is_some_and(|c| vdir::State::load(&c.state_path()).pulled.is_some_and(|p| p >= at))
}

/// Each write watched (`watch`) read again, once its server has it as it is
/// (`vdir::agreed_hash`) and only while that copy is still the write this
/// device made (`Watched::mark`; another device's later write is that
/// device's to watch), its server's own copy (`vdir::server_copy`): on each
/// guest's line of the first VEVENT, 1.x or 2.x is done; 3.x or 5.x is taken
/// over (`take_over`). A guest the write invited (`Watched::due`) of whom the
/// server says nothing `SERVER_WAIT` after the write and a pull since is
/// taken over too when no guest of the first VEVENT carries any word (the
/// server did not schedule, and sends no CANCEL for the mark either); when
/// it spoke of others, the card says it gave no sign (`Watched::silent`). A
/// guest of some times alone is the server's (it tells them and never
/// reports on them), and a guest merely kept, said nothing of, had nothing
/// due: neither is watched further. A guest gone from it, or left to you
/// meanwhile, is no longer watched; a meeting gone by sends nothing.
fn check_served(now: i64, nudge: &dyn Fn(&str), scope: &dyn Fn(&str) -> bool) -> Vec<String> {
    let watched = with_memory(|m| m.watched.clone());
    if watched.is_empty() {
        return Vec::new();
    }
    let own = agenda::own_addresses();
    // None of your addresses known now (settings not read): nothing decided.
    if own.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for entry in watched {
        let raw = crate::files::read_to_string(&entry.path).ok();
        if raw.as_deref().is_some_and(|t| !scope(&uid_of(t))) {
            continue;
        }
        let forget = || with_memory(|m| m.watched.retain(|w| w.path != entry.path || w.written != entry.written));
        // Gone, or no longer a meeting you organize: no longer watched.
        let Some(text) = raw.filter(|t| agenda::organized_meeting(t, &own)) else {
            forget();
            continue;
        };
        if !vdir::server_copy(&entry.path) {
            // Not its server's copy yet (not sent, or sent and not fetched back): watched a week.
            if now - entry.written > 7 * 86_400 {
                forget();
            }
            continue;
        }
        if !entry.mark.is_empty() && organizer::written_mark(&text) != entry.mark {
            forget();
            continue;
        }
        let guests = organizer::every_guest(&text);
        let clients = organizer::clients(&text);
        let spoke = organizer::first_event_has_status(&text);
        let waited = now - entry.written >= SERVER_WAIT && pulled_since(&entry.path, entry.written + 30);
        let (mut taken, mut left, mut silent) = (Vec::new(), Vec::new(), Vec::new());
        for guest in &entry.guests {
            if !guests.iter().any(|g| g.address.eq_ignore_ascii_case(guest)) || clients.contains(guest) {
                continue;
            }
            // A guest of some times alone: its server tells them and never reports on them.
            if !organizer::on_first_event(&text, guest) {
                continue;
            }
            let due = entry.due.contains(guest);
            match organizer::delivery(&text, guest) {
                Some(Delivery::Sent) => {}
                Some(Delivery::NotSent) => taken.push(guest.clone()),
                None if !waited => left.push(guest.clone()),
                None if due && !spoke => taken.push(guest.clone()),
                None if due => silent.push(guest.clone()),
                None => {}
            }
        }
        let silent: Vec<String> = entry.silent.iter().filter(|g| guests.iter().any(|x| x.address.eq_ignore_ascii_case(g)) && !clients.contains(*g)).cloned().chain(silent).collect();
        if gone_by(&text, now) {
            taken.clear();
            left.clear();
        }
        let mut mark = entry.mark.clone();
        if !taken.is_empty()
            && let Some((said, marked)) = take_over(&entry.path, &text, &taken, entry.cancelled, nudge)
        {
            lines.push(said);
            mark = organizer::written_mark(&marked);
        }
        with_memory(|m| {
            m.watched.retain(|w| w.path != entry.path || w.written != entry.written);
            if (!left.is_empty() || !silent.is_empty()) && !m.watched.iter().any(|w| w.path == entry.path) {
                m.watched.push(Watched { guests: left.clone(), silent: silent.clone(), mark: mark.clone(), ..entry.clone() });
            }
        });
    }
    lines
}

/// The guests `taken` (lower case) of the meeting at `path` (read as
/// `text`), whose server did not deliver: their message (its invitation, or
/// its cancellation) made from the meeting with them left to you from now on
/// (`organizer::left_to_you_now`), its SEQUENCE the file's, and put in the
/// outbox first (`after_sync`, `taken_from`); then that text written through
/// the door, only over the text read (`pim::guests_taken_over`), else the
/// messages taken back. sabre answers the mark with a CANCEL of its own to
/// them, through the delivery that failed, at the file's SEQUENCE and stamped
/// at that write; Nextcloud's own broker's carries the old version's
/// SEQUENCE and stamp. Sioul's message leaves once the server's copy holds
/// the mark, stamped then (`send_due`): the later one either way. The
/// SEQUENCE is never raised for it (the other guests' answers would read as
/// older). What to say, and the text written.
fn take_over(path: &Path, text: &str, taken: &[String], cancelled: bool, nudge: &dyn Fn(&str)) -> Option<(String, String)> {
    let marked = organizer::left_to_you_now(text, taken);
    let organizer = organizer_in(&marked);
    let (account, signature) = account_for(&organizer).map(|a| (a.id, a.signature)).unwrap_or_default();
    let to: Vec<Guest> = organizer::every_guest(&marked).into_iter().filter(|g| taken.iter().any(|t| t.eq_ignore_ascii_case(&g.address))).collect();
    let series = organizer::guests_of(&marked);
    let (whole, apart): (Vec<Guest>, Vec<Guest>) = to.iter().cloned().partition(|g| series.iter().any(|s| s.address.eq_ignore_ascii_case(&g.address)));
    let addresses = |guests: &[Guest]| guests.iter().map(|g| g.address.clone()).collect::<Vec<_>>();
    let messages = if cancelled {
        vec![(to.clone(), organizer::cancel(&marked, &addresses(&to), true))]
    } else {
        vec![(whole.clone(), organizer::request(&marked)), (apart.clone(), organizer::request_for(&marked, &addresses(&apart)))]
    };
    let mut saved: Vec<Draft> = Vec::new();
    for (to, ics) in messages {
        let Some(ics) = ics.filter(|_| !to.is_empty()) else { continue };
        let mut draft = queued(organizer::draft(&account, signature.as_deref(), &to, &marked, ics, cancelled, tr()));
        draft.after_sync = Some(path.to_path_buf());
        draft.taken_from = organizer::written_mark(text);
        match draft.save() {
            Ok(_) => saved.push(draft),
            Err(e) => {
                eprintln!("Sioul: an invitation could not be kept to send: {e}");
                saved.iter().for_each(Draft::discard);
                return None;
            }
        }
    }
    if let Err(e) = crate::pim::guests_taken_over(path, text, &marked) {
        eprintln!("Sioul: a meeting's guests could not be left to you: {e}");
        saved.iter().for_each(Draft::discard);
        return None;
    }
    if let Some(calendar) = crate::pim::collection_of(path) {
        nudge(&calendar.account);
    }
    wake();
    Some((say("invite-taken-over", &[("who", to.iter().map(Guest::said).collect::<Vec<_>>().join(", "))]), marked))
}

/// "Send it by mail" on a meeting whose server gave no sign for some guests
/// (`Watched::silent`), your press: they are taken over as for a server's
/// refusal (`take_over`), from the meeting as it is now. What the status line says.
pub(crate) fn send_by_mail(path: &Path) -> Result<String, String> {
    let gone = || tr().text("agenda-gone", None);
    let text = crate::files::read_to_string(path).map_err(|_| gone())?;
    let entry = with_memory(|m| m.watched.iter().find(|w| w.path == path).cloned()).ok_or_else(gone)?;
    let clients = organizer::clients(&text);
    let silent: Vec<String> = entry.silent.iter().filter(|g| !clients.contains(*g)).cloned().collect();
    if silent.is_empty() {
        return Err(gone());
    }
    let (said, marked) = take_over(path, &text, &silent, entry.cancelled, &|_| {}).ok_or_else(|| tr().text("undo-too-late", None))?;
    with_memory(|m| {
        for w in m.watched.iter_mut().filter(|w| w.path == path) {
            w.silent.clear();
            w.mark = organizer::written_mark(&marked);
        }
        m.watched.retain(|w| w.path != path || !w.guests.is_empty());
    });
    Ok(said)
}

// ---------------------------------------------------------------- the event's card

/// The UID of a meeting's file.
fn uid_of(text: &str) -> String {
    sioul_core::meeting_lines::uid_of(text)
}

/// Whether an iTIP message is of the meeting with this UID.
fn of_uid(draft: &Draft, uid: &str) -> bool {
    draft.calendar.as_deref().is_some_and(|ics| lines::unfold(ics).iter().any(|l| lines::name(l) == "UID" && lines::value(l).trim() == uid))
}

/// The messages of this device's outbox for the meeting with this UID.
fn outbox_of(uid: &str) -> Vec<Draft> {
    Draft::outbox_all().into_iter().filter(|d| of_uid(d, uid)).collect()
}

/// What a meeting you organize says of its invitation on its card, in one
/// line, and what its button does (`act`): a message that waits, why, and
/// the way to Accounts ("accounts"); one Sioul does not know left, after a
/// stop while sending, with "Send it again" ("send-again"); one about to
/// leave ("" act); guests its server gave no sign for, with "Send it by
/// mail" ("send-by-mail"); a meeting made before Sioul sent invitations
/// itself, its guests left to you, its invitation still waiting in Mail's
/// Drafts and its time still ahead: "Send the invitation now" ("send-now";
/// every device says it while that draft is there, none once it left).
/// Nothing for another event.
pub(crate) fn card_line(path: &Path) -> serde_json::Value {
    let none = serde_json::json!({ "line": "", "act": "" });
    let Ok(text) = crate::files::read_to_string(path) else { return none };
    if !agenda::organized_meeting(&text, &agenda::own_addresses()) {
        return none;
    }
    let uid = uid_of(&text);
    let waiting = outbox_of(&uid);
    if let Some(stuck) = waiting.iter().find(|d| !d.waiting.is_empty()) {
        return serde_json::json!({ "line": say("invite-waiting", &[("who", names(stuck)), ("why", stuck.waiting.clone())]), "act": "accounts" });
    }
    if let Some(lost) = unsure().into_iter().find(|d| of_uid(d, &uid)) {
        return serde_json::json!({ "line": say("invite-unsure", &[("who", names(&lost))]), "act": "send-again" });
    }
    if let Some(leaving) = waiting.first() {
        return serde_json::json!({ "line": say("invite-leaving", &[("who", names(leaving))]), "act": "" });
    }
    let clients = organizer::clients(&text);
    let silent: Vec<Guest> = with_memory(|m| m.watched.iter().find(|w| w.path == path).map(|w| w.silent.clone())).unwrap_or_default().into_iter().filter_map(|a| organizer::every_guest(&text).into_iter().find(|g| g.address.eq_ignore_ascii_case(&a) && !clients.contains(&a))).collect();
    if !silent.is_empty() && !gone_by(&text, now()) {
        return serde_json::json!({ "line": say("invite-no-sign", &[("who", silent.iter().map(Guest::said).collect::<Vec<_>>().join(", "))]), "act": "send-by-mail" });
    }
    if !clients.is_empty() && !gone_by(&text, now()) && !left_in_drafts(&uid, &agenda::own_addresses()).is_empty() {
        return serde_json::json!({ "line": tr().text("invite-send-now-line", None), "act": "send-now" });
    }
    none
}

/// "Send it again" on a message Sioul does not know left (`unsure`): back
/// in the outbox, sent at the next pass. What the status line says.
pub(crate) fn send_again(path: &Path) -> Result<String, String> {
    let gone = || tr().text("agenda-gone", None);
    let text = crate::files::read_to_string(path).map_err(|_| gone())?;
    let uid = uid_of(&text);
    let lost: Vec<Draft> = unsure().into_iter().filter(|d| of_uid(d, &uid)).collect();
    let first = lost.first().ok_or_else(gone)?;
    let who = names(first);
    for mut draft in lost {
        let _ = crate::files::remove_file(sending_path(&draft.id));
        draft.queued = now() - DELAY;
        draft.save()?;
    }
    wake();
    Ok(say("invite-send-now-done", &[("who", who)]))
}

/// "Send the invitation now" on a meeting made before (`card_line`): its
/// invitation of before still unsent in Mail's Drafts taken out, and one
/// for each guest left to you put in the outbox, sent at once. What the
/// status line says.
pub(crate) fn send_now(path: &Path) -> Result<String, String> {
    let gone = || tr().text("agenda-gone", None);
    let text = crate::files::read_to_string(path).map_err(|_| gone())?;
    let own = agenda::own_addresses();
    if !agenda::organized_meeting(&text, &own) {
        return Err(gone());
    }
    let clients = organizer::clients(&text);
    let to: Vec<Guest> = organizer::every_guest(&text).into_iter().filter(|g| clients.contains(&g.address.to_lowercase())).collect();
    if to.is_empty() {
        return Err(gone());
    }
    let uid = uid_of(&text);
    for old in left_in_drafts(&uid, &own) {
        old.discard();
    }
    let organizer = organizer_in(&text);
    let (account, signature) = account_for(&organizer).map(|a| (a.id, a.signature)).unwrap_or_default();
    let series = organizer::guests_of(&text);
    let (whole, apart): (Vec<Guest>, Vec<Guest>) = to.iter().cloned().partition(|g| series.iter().any(|s| s.address.eq_ignore_ascii_case(&g.address)));
    let addresses: Vec<String> = apart.iter().map(|g| g.address.clone()).collect();
    for (guests, ics) in [(whole, organizer::request(&text)), (apart, organizer::request_for(&text, &addresses))] {
        let Some(ics) = ics.filter(|_| !guests.is_empty()) else { continue };
        let mut draft = queued(organizer::draft(&account, signature.as_deref(), &guests, &text, ics, false, tr()));
        // Asked now: no change to undo before it leaves.
        draft.queued -= DELAY;
        draft.save()?;
    }
    wake();
    Ok(say("invite-send-now-done", &[("who", to.iter().map(Guest::said).collect::<Vec<_>>().join(", "))]))
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Fetching new mail, read-only, into the account's Maildir.
//!
//! The first sync brings the last days of each folder (`sync_days`, 14 by
//! default), so the Porch shows soon. Then what arrives, by UID, the inbox
//! first, and, a round at a time, everything older, newest first: all your
//! mail is kept here, and the history setting only chooses what the window
//! shows. Older mail never takes the disk's reserve (`disk::room`): it waits,
//! and the window says so. Where sync stopped is kept per account and folder
//! in `$XDG_STATE_HOME/sioul/sync/<id>.toml`, after every batch, so an
//! interrupted sync resumes where it stopped.
//!
//! `watch` keeps the inbox open with IDLE (RFC 2177), so a code reaches you
//! within seconds of reaching your provider.

use crate::SyncError;
use crate::imap::{self, COMMAND, Imap, Server};
use crate::mailbox;
use crate::verify::Verifier;
use sioul_core::folders::{self, Folder, Role};
use async_imap::types::Flag;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sioul_core::card::ImapOrigin;
use sioul_core::config::{Account, DEFAULT_SYNC_DAYS, state_dir};
use sioul_core::maildir::{self, Fetched};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::task::{Poll, Waker};
use std::time::{Duration, Instant, SystemTime};

/// What one sync brought.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub account: String,
    /// The files written, oldest first.
    pub new: Vec<PathBuf>,
    /// The first sync of the inbox: `new` holds the last days of mail, not only arrivals.
    pub first: bool,
    /// Messages in the inbox, on the server.
    pub inbox: u32,
    /// Files written besides the inbox's arrivals: other folders, older mail.
    /// Tied to their cases like the rest, never notified.
    pub elsewhere: Vec<PathBuf>,
    /// Older mail waits: fetching it would take the disk's reserve.
    pub held_back: bool,
}

const INBOX: &str = "INBOX";
/// Messages per FETCH: small enough to save progress often.
const BATCH: usize = 50;
/// Older messages fetched per round, so new mail never waits long behind them.
const OLDER_PER_ROUND: usize = 300;
const FETCH: Duration = Duration::from_secs(180);
/// IDLE is renewed this often: RFC 2177 asks for less than 29 minutes, and a
/// connection that died silently is noticed sooner.
const IDLE: Duration = Duration::from_secs(5 * 60);
/// Without IDLE, Sioul looks this often.
const POLL: Duration = Duration::from_secs(2 * 60);
/// The first wait before reconnecting; it doubles up to ten minutes.
const FIRST_PAUSE: Duration = Duration::from_secs(30);
const LONGEST_PAUSE: Duration = Duration::from_secs(10 * 60);

/// Real time: every folder fetched this often, for a code or a password you wait for.
const REALTIME: Duration = Duration::from_secs(60);
/// Paces longer than this keep no connection open between fetches: IDLE
/// would bring the inbox sooner than wished (RFC 2177 renews under 29 minutes).
const SLOW: Duration = Duration::from_secs(25 * 60);

/// How often a long wait looks at the wall clock: the monotonic clock, which
/// timers count, stops while a phone sleeps.
const LOOK: Duration = Duration::from_secs(60);

/// How a watcher is told to stop, or to fetch now instead of waiting, and at what pace.
#[derive(Debug, Default)]
pub struct Control {
    stop: AtomicBool,
    nudge: AtomicBool,
    /// "Sync now": a refused password is tried again too.
    retry: AtomicBool,
    /// Every folder, every minute, until switched off.
    realtime: AtomicBool,
    /// Seconds between two fetches of the other folders; 0 for the default.
    pace: std::sync::atomic::AtomicU64,
    /// The app in the background (a phone's app put away): nothing starts on
    /// a timer but the inbox's watch; a nudge still does, and leaving it is one.
    quiet: AtomicBool,
    /// Wakes the pauses at once (`ring`); `lock` keeps a ring from passing
    /// unheard between a look at the flags and the wait.
    bell: Condvar,
    lock: Mutex<()>,
    /// The async waits (the mail watcher's), woken with the pauses.
    wakers: Mutex<Vec<Waker>>,
}

impl Control {
    /// Real time on or off; on, it fetches now.
    pub fn set_realtime(&self, on: bool) {
        self.realtime.store(on, Ordering::Relaxed);
        if on {
            self.nudge();
        }
    }

    pub fn realtime(&self) -> bool {
        self.realtime.load(Ordering::Relaxed)
    }

    /// The pace between two fetches; zero for the default.
    pub fn set_pace(&self, every: Duration) {
        self.pace.store(every.as_secs(), Ordering::Relaxed);
    }

    /// How long the inbox waits in IDLE, and the other folders between fetches.
    fn round(&self) -> Duration {
        if self.realtime() {
            return REALTIME;
        }
        match self.pace.load(Ordering::Relaxed) {
            0 => IDLE,
            seconds => Duration::from_secs(seconds),
        }
    }

    /// A pace slow enough to keep no connection open between fetches.
    fn slow(&self) -> bool {
        !self.realtime() && self.round() > SLOW
    }

    /// What a round of the mail watcher brings besides the inbox's new mail:
    /// the inbox's changes and older mail, and the other folders when they
    /// are `due`. Quiet, neither: they wait for the first round after.
    fn brings(&self, due: bool) -> (bool, bool) {
        let full = !self.quiet();
        (full, full && due)
    }

    /// Quiet or not: the app put away, or back. Quiet, a watcher starts
    /// nothing on its timer but the inbox's IDLE (new mail is still noticed,
    /// the other folders wait); leaving it wakes the watcher, as a nudge.
    pub fn set_quiet(&self, quiet: bool) {
        if self.quiet.swap(quiet, Ordering::Relaxed) && !quiet {
            self.nudge();
        }
    }

    pub fn quiet(&self) -> bool {
        self.quiet.load(Ordering::Relaxed)
    }

    /// Ends the watch at the next occasion: at once when waiting.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.ring();
    }

    /// Fetches now: ends the current wait, IDLE or pause.
    pub fn nudge(&self) {
        self.nudge.store(true, Ordering::Relaxed);
        self.ring();
    }

    /// "Sync now": a nudge, after which a watcher parked on a refused
    /// password tries it again, unchanged (see `Parked`).
    pub fn retry(&self) {
        self.retry.store(true, Ordering::Relaxed);
        self.nudge();
    }

    /// Whether "Sync now" was asked since last looked.
    pub(crate) fn retried(&self) -> bool {
        self.retry.swap(false, Ordering::Relaxed)
    }

    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Wakes every wait, to look at the flags again.
    fn ring(&self) {
        let _held = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        self.bell.notify_all();
        for waker in std::mem::take(&mut *self.wakers.lock().unwrap_or_else(PoisonError::into_inner)) {
            waker.wake();
        }
    }

    /// An async wait, woken at the next ring.
    fn listen(&self, waker: &Waker) {
        let mut wakers = self.wakers.lock().unwrap_or_else(PoisonError::into_inner);
        if !wakers.iter().any(|w| w.will_wake(waker)) {
            wakers.push(waker.clone());
        }
    }

    /// Waits `length` on this thread, or less when stopped or nudged. Counted
    /// on the wall clock too: a phone asleep for an hour ends a pause of 15
    /// minutes as soon as it wakes. While quiet, only a nudge ends it.
    pub fn pause(&self, length: Duration) {
        self.pause_by(length, SystemTime::now, LOOK);
    }

    /// `pause`, the wall clock read with `wall`, at least every `look`.
    fn pause_by(&self, length: Duration, wall: impl Fn() -> SystemTime, look: Duration) {
        let end = Deadline::after(length, wall());
        let mut held = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        while !self.wakes() {
            let left = end.left(wall());
            held = if self.quiet() {
                self.bell.wait(held).unwrap_or_else(PoisonError::into_inner)
            } else if left.is_zero() {
                return;
            } else {
                self.bell.wait_timeout(held, left.min(look)).unwrap_or_else(PoisonError::into_inner).0
            };
        }
    }

    /// Waits for a nudge, however long, quiet or not: a watcher parked after
    /// a lasting error, instead of ending. False when stopped.
    pub fn park(&self) -> bool {
        let mut held = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        while !self.wakes() {
            held = self.bell.wait(held).unwrap_or_else(PoisonError::into_inner);
        }
        !self.stopped()
    }

    /// Whether the wait should end; a nudge is used up by ending one.
    fn wakes(&self) -> bool {
        self.stopped() || self.nudge.swap(false, Ordering::Relaxed)
    }
}

/// A refused password is tried again, unchanged, this long after at the
/// soonest, however often the watcher is nudged: retries can lock an account.
const REFUSED_AGAIN: Duration = Duration::from_secs(60 * 60);

/// A watcher parked after a lasting error: it waits for a nudge (a new
/// password, "Sync now", the app back) instead of ending, then tries again;
/// a refused password, unchanged, not before `REFUSED_AGAIN`, unless asked
/// to ([`Control::retry`]).
pub(crate) struct Parked {
    refused: Option<String>,
    since: SystemTime,
}

impl Parked {
    /// After `error`, met with `password` (the one refused, when it says so).
    pub(crate) fn after(error: &SyncError, password: Option<String>) -> Parked {
        Parked { refused: password.filter(|_| error.wants_password()), since: SystemTime::now() }
    }

    /// Whether going on depends on the password kept: one was refused.
    pub(crate) fn refused(&self) -> bool {
        self.refused.is_some()
    }

    /// Whether to try again, nudged, with the password kept now.
    pub(crate) fn again(&self, kept: Option<&str>) -> bool {
        self.refused.as_deref().is_none_or(|refused| kept.is_some_and(|k| k != refused) || SystemTime::now().duration_since(self.since).is_ok_and(|d| d >= REFUSED_AGAIN))
    }
}

/// The end of a wait, on both clocks: the wall clock counts a phone's sleep,
/// the monotonic clock keeps a wall clock put back from stretching it.
struct Deadline {
    wall: Option<SystemTime>,
    steady: Option<Instant>,
}

impl Deadline {
    fn after(length: Duration, now: SystemTime) -> Deadline {
        Deadline { wall: now.checked_add(length), steady: Instant::now().checked_add(length) }
    }

    /// What is left of it, by whichever clock is further along; zero when over.
    fn left(&self, now: SystemTime) -> Duration {
        let wall = self.wall.map_or(Duration::MAX, |end| end.duration_since(now).unwrap_or_default());
        let steady = self.steady.map_or(Duration::MAX, |end| end.saturating_duration_since(Instant::now()));
        wall.min(steady)
    }
}

/// Logs in and opens the inbox read-only: the number of messages in it.
pub fn test(account: &Account, password: &str) -> Result<u32, SyncError> {
    let server = Server::of(account)?;
    block_on(async {
        let mut session = imap::open(&server, password).await?;
        let inbox = imap::within(COMMAND, session.examine(INBOX)).await?.map_err(imap::server)?;
        let _ = session.logout().await;
        Ok(inbox.exists)
    })
}

/// Fetches what is new since the last sync, in every folder; the first time,
/// the last `sync_days` days. The report is the inbox's.
pub fn sync(account: &Account, password: &str) -> Result<Report, SyncError> {
    let server = Server::of(account)?;
    block_on(async {
        let mut session = imap::open(&server, password).await?;
        let mut report = fetch_new(&mut session, account).await;
        if let Ok(report) = report.as_mut() {
            let (elsewhere, held_back) = other_folders(&mut session, account).await?;
            report.elsewhere.extend(elsewhere);
            report.held_back |= held_back;
        }
        let _ = session.logout().await;
        report
    })
}

/// The inbox's arrivals alone, in one connection and without IDLE: what a
/// phone's background service fetches at its rhythm, while Sioul is closed
/// (crates/sioul-app/src/steps.rs). One fetch at a time per account, across
/// processes too, as for every fetch (`fetch_folder`).
pub fn inbox(account: &Account, password: &str) -> Result<Report, SyncError> {
    let server = Server::of(account)?;
    block_on(async {
        let mut session = imap::open(&server, password).await?;
        let report = fetch_folder(&mut session, account, &folders::folder(INBOX, None, None), false).await;
        let _ = session.logout().await;
        report
    })
}

/// Lists the folders, keeps the list, and fetches every folder but the inbox;
/// returns the files written, and whether older mail waits for room.
async fn other_folders(session: &mut Imap, account: &Account) -> Result<(Vec<PathBuf>, bool), SyncError> {
    let list = mailbox::list(session).await?;
    mailbox::save(&account.id, &list)?;
    let (mut written, mut held_back) = (Vec::new(), false);
    for folder in list.iter().filter(|f| f.role != Role::Inbox && mailbox::fetched(f, &list, account)) {
        let report = fetch_folder(session, account, folder, true).await?;
        written.extend(report.new.into_iter().chain(report.elsewhere));
        held_back |= report.held_back;
    }
    Ok((written, held_back))
}

/// Keeps the inbox open and fetches what arrives, until stopped (see
/// [`Control`]). Every sync is reported, even an empty one, and every error,
/// before trying again; after a lasting one (a refused login, see
/// [`SyncError::is_lasting`]) it waits for a nudge, then tries again with the
/// password kept then (see `Parked`).
pub fn watch(account: &Account, password: &str, control: &Control, mut report: impl FnMut(Result<Report, SyncError>)) {
    let server = match Server::of(account) {
        Ok(server) => server,
        Err(e) => return report(Err(e)),
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(e) => return report(Err(e)),
    };
    runtime.block_on(async {
        let mut password = password.to_string();
        let mut pause = FIRST_PAUSE;
        while !control.stopped() {
            let Err(e) = keep_open(&server, &password, account, control, &mut report, &mut pause).await else { continue };
            let parked = e.is_lasting().then(|| Parked::after(&e, Some(password.clone())));
            report(Err(e));
            if let Some(parked) = parked {
                control.retried();
                loop {
                    woken(control).await;
                    if control.stopped() {
                        return;
                    }
                    if !parked.refused() {
                        break;
                    }
                    let kept = crate::secret::password(account).ok();
                    if parked.again(kept.as_deref()) || control.retried() {
                        password = kept.unwrap_or(password);
                        break;
                    }
                }
                pause = FIRST_PAUSE;
                continue;
            }
            // Nudged (the app back, "Sync now"): tried again at once, and soon again if it fails.
            pause = if wait(pause, control).await { FIRST_PAUSE } else { (pause * 2).min(LONGEST_PAUSE) };
        }
    });
}

/// One connection, kept as long as it lives: fetch, wait for news, fetch again.
async fn keep_open(
    server: &Server,
    password: &str,
    account: &Account,
    control: &Control,
    report: &mut impl FnMut(Result<Report, SyncError>),
    pause: &mut Duration,
) -> Result<(), SyncError> {
    let mut session = imap::open(server, password).await?;
    let has_idle = imap::within(COMMAND, session.capabilities()).await?.map_err(imap::server)?.has_str("IDLE");
    // The other folders: at the start, then at each quiet round (five minutes), not at each arrival.
    let mut others_due = true;
    while !control.stopped() {
        let (full, others) = control.brings(others_due);
        let mut new = fetch_folder(&mut session, account, &folders::folder(INBOX, None, None), full).await?;
        if others {
            let (elsewhere, held_back) = other_folders(&mut session, account).await?;
            new.elsewhere.extend(elsewhere);
            new.held_back |= held_back;
        }
        *pause = FIRST_PAUSE;
        report(Ok(new));
        if control.stopped() {
            break;
        }
        // A slow pace (a public address read twice a day): no connection kept meanwhile.
        if control.slow() {
            let _ = session.logout().await;
            wait(control.round(), control).await;
            return Ok(());
        }
        if !has_idle {
            wait(control.round().min(POLL), control).await;
            others_due = true;
            continue;
        }
        let mut idle = session.idle();
        imap::within(COMMAND, idle.init()).await?.map_err(imap::server)?;
        {
            let (news, _interrupt) = idle.wait_with_timeout(control.round().min(IDLE));
            tokio::select! {
                answer = news => {
                    others_due = answer.map_err(imap::server)? == async_imap::extensions::idle::IdleResponse::Timeout;
                }
                () = woken(control) => { others_due = true; }
            }
        }
        session = imap::within(COMMAND, idle.done()).await?.map_err(imap::server)?;
    }
    let _ = session.logout().await;
    Ok(())
}

/// Fetches the inbox's new messages into the Maildir.
async fn fetch_new(session: &mut Imap, account: &Account) -> Result<Report, SyncError> {
    fetch_folder(session, account, &folders::folder(INBOX, None, None), true).await
}

/// Fetches a folder's new messages into its Maildir, then (`full`) brings
/// back what changed there on the server and older mail. One fetch at a time
/// per account, across processes too (the window and `sioul sync`):
/// otherwise both would see the same new UIDs and write each message twice.
async fn fetch_folder(session: &mut Imap, account: &Account, folder: &Folder, full: bool) -> Result<Report, SyncError> {
    let state_path = state_dir().join("sync").join(format!("{}.toml", account.id));
    let _lock = lock(&state_path.with_extension("lock"))?;
    let inbox = imap::within(COMMAND, session.examine(&folder.name)).await?.map_err(imap::server)?;
    let validity = inbox.uid_validity.unwrap_or(0);
    let mut state = SyncState::load(&state_path);
    let key = folder.name.as_str();
    // A new UIDVALIDITY means the server renumbered the folder: start over.
    let known = state.folders.get(key).copied().filter(|f| f.uidvalidity == validity);
    let last_uid = known.map_or(0, |f| f.last_uid);
    let query = match known {
        // The last UID a server may give (2³²−1) has no next one.
        Some(_) => format!("UID {}:*", last_uid.saturating_add(1)),
        None => format!("SINCE {}", imap_date(first_window(account))),
    };
    let found = imap::within(COMMAND, session.uid_search(&query)).await?.map_err(imap::server)?;
    let root = account.maildir_path().join(&folder.local);
    let (uids, mut newest) = to_fetch(found, last_uid, &root, validity);
    let mut report = Report { account: account.id.clone(), new: Vec::new(), first: known.is_none(), inbox: inbox.exists, elsewhere: Vec::new(), held_back: false };
    // Without DNS (offline), mail is stored unchecked; a later pass checks it.
    let verifier = if uids.is_empty() { None } else { Verifier::new() };
    for batch in uids.chunks(BATCH) {
        let set = batch.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let arrived = imap::within(FETCH, fetch_batch(session, &set)).await??;
        let written = store_checked(arrived, validity, &root, verifier.as_ref()).await?;
        newest = written.iter().map(|(uid, _)| *uid).fold(newest, u32::max);
        report.new.extend(written.into_iter().map(|(_, path)| path));
        let reached = known.map_or_else(|| Reach::first(account), |k| k.reach());
        state.folders.insert(key.into(), FolderState { uidvalidity: validity, last_uid: newest, since: reached.since, everything: reached.everything });
        state.save(&state_path)?;
    }
    if uids.is_empty() && newest > last_uid {
        // Here already, from a sync stopped before it could say so.
        let reached = known.map_or_else(|| Reach::first(account), |k| k.reach());
        state.folders.insert(key.into(), FolderState { uidvalidity: validity, last_uid: newest, since: reached.since, everything: reached.everything });
        state.save(&state_path)?;
    }
    if known.is_none() && uids.is_empty() {
        // Nothing in the first window: remember the folder anyway, so the next
        // sync looks only at arrivals. Without UIDNEXT, the window is used again.
        if let Some(next) = inbox.uid_next {
            let reached = Reach::first(account);
            state.folders.insert(key.into(), FolderState { uidvalidity: validity, last_uid: next.saturating_sub(1), since: reached.since, everything: reached.everything });
            state.save(&state_path)?;
        }
    }
    if let Some(known) = known
        && full
    {
        mailbox::reconcile(session, &root, validity).await?;
        backfill(session, &mut state, &state_path, key, known, &root, validity, &mut report).await?;
    }
    Ok(report)
}

/// The UIDs to fetch of those found, oldest first, and the newest of those
/// here already (else `last_uid`). Here already: a sync stopped between
/// storing mail and saving where it stopped, or whose state was lost, finds
/// them again, and they are not stored twice.
fn to_fetch(found: impl IntoIterator<Item = u32>, last_uid: u32, root: &Path, validity: u32) -> (Vec<u32>, u32) {
    // "UID n:*" also returns the newest message when nothing is newer (RFC 9051 §6.4.8).
    let mut uids: Vec<u32> = found.into_iter().filter(|uid| *uid > last_uid).collect();
    if uids.is_empty() {
        return (uids, last_uid);
    }
    let here = local_uids(root, validity);
    let newest = uids.iter().copied().filter(|uid| here.contains(uid)).fold(last_uid, u32::max);
    uids.retain(|uid| !here.contains(uid));
    uids.sort_unstable();
    (uids, newest)
}

/// How far back a folder was fetched: since a day (Unix seconds), or everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reach {
    since: Option<i64>,
    everything: bool,
}

impl Reach {
    /// What a first sync brings: the first window.
    fn first(account: &Account) -> Reach {
        Reach { since: first_window(account).to_zoned(jiff::tz::TimeZone::UTC).ok().map(|z| z.timestamp().as_second()), everything: false }
    }
}

impl FolderState {
    fn reach(self) -> Reach {
        Reach { since: self.since, everything: self.everything }
    }
}

/// The older messages of a folder, those not here yet, a round at a time,
/// newest first, each round sized first so the disk keeps its reserve; the
/// folder remembers once it holds everything.
#[allow(clippy::too_many_arguments)]
async fn backfill(session: &mut Imap, state: &mut SyncState, state_path: &Path, key: &str, known: FolderState, root: &Path, validity: u32, report: &mut Report) -> Result<(), SyncError> {
    if known.everything {
        return Ok(());
    }
    let found = imap::within(COMMAND, session.uid_search("ALL")).await?.map_err(imap::server)?;
    let here: BTreeSet<u32> = local_uids(root, validity);
    let mut older: Vec<u32> = found.into_iter().filter(|uid| *uid <= known.last_uid && !here.contains(uid)).collect();
    older.sort_unstable_by(|a, b| b.cmp(a));
    let round: Vec<u32> = older.iter().take(OLDER_PER_ROUND).copied().collect();
    // What the disk can take of this round, the newest first.
    let sizes = imap::within(FETCH, sizes_of(session, &round)).await??;
    let mut room = crate::disk::room(root);
    let mut take = Vec::new();
    for uid in &round {
        let size = u64::from(sizes.get(uid).copied().unwrap_or(0));
        if size > room {
            report.held_back = true;
            break;
        }
        room -= size;
        take.push(*uid);
    }
    take.sort_unstable();
    let verifier = if take.is_empty() { None } else { Verifier::new() };
    for batch in take.chunks(BATCH) {
        let set = batch.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let arrived = imap::within(FETCH, fetch_batch(session, &set)).await??;
        report.elsewhere.extend(store_checked(arrived, validity, root, verifier.as_ref()).await?.into_iter().map(|(_, path)| path));
    }
    if !report.held_back && take.len() == older.len() {
        let mut folder = state.folders.get(key).copied().unwrap_or(known);
        folder.since = None;
        folder.everything = true;
        state.folders.insert(key.into(), folder);
        state.save(state_path)?;
    }
    Ok(())
}

/// The sizes of messages on the server (RFC822.SIZE), by UID, before fetching them.
async fn sizes_of(session: &mut Imap, uids: &[u32]) -> Result<std::collections::HashMap<u32, u32>, SyncError> {
    let mut sizes = std::collections::HashMap::new();
    if uids.is_empty() {
        return Ok(sizes);
    }
    let set = uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let mut stream = session.uid_fetch(&set, "(UID RFC822.SIZE)").await.map_err(imap::server)?;
    while let Some(fetch) = stream.next().await {
        let fetch = fetch.map_err(imap::server)?;
        if let (Some(uid), Some(size)) = (fetch.uid, fetch.size) {
            sizes.insert(uid, size);
        }
    }
    Ok(sizes)
}

/// The UIDs of a folder's messages kept here, for this UIDVALIDITY.
fn local_uids(root: &Path, validity: u32) -> BTreeSet<u32> {
    ["new", "cur"]
        .iter()
        .filter_map(|sub| std::fs::read_dir(root.join(sub)).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| maildir::origin_of(&e.path()))
        .filter(|o| o.validity == validity)
        .map(|o| o.uid)
        .collect()
}

/// A message as the server gave it, before Sioul checks and stores it.
struct Arrived {
    uid: u32,
    flags: String,
    received: Option<i64>,
    raw: Vec<u8>,
}

/// One FETCH: every message of `set`. BODY.PEEK leaves the \Seen flag alone.
async fn fetch_batch(session: &mut Imap, set: &str) -> Result<Vec<Arrived>, SyncError> {
    let mut stream = session.uid_fetch(set, "(UID FLAGS INTERNALDATE BODY.PEEK[])").await.map_err(imap::server)?;
    let mut arrived = Vec::new();
    while let Some(fetch) = stream.next().await {
        let fetch = fetch.map_err(imap::server)?;
        let (Some(uid), Some(raw)) = (fetch.uid, fetch.body()) else { continue };
        arrived.push(Arrived { uid, flags: maildir_flags(fetch.flags()), received: fetch.internal_date().map(|d| d.timestamp()), raw: raw.to_vec() });
    }
    Ok(arrived)
}

/// Checks each message (SPF, DKIM, DMARC…, see `verify`) and writes it into the Maildir.
async fn store_checked(arrived: Vec<Arrived>, validity: u32, root: &Path, verifier: Option<&Verifier>) -> Result<Vec<(u32, PathBuf)>, SyncError> {
    let mut written = Vec::new();
    for message in arrived {
        let raw = match verifier {
            Some(v) => v.stamp(&message.raw).await,
            None => message.raw,
        };
        let fetched = Fetched { origin: ImapOrigin { validity, uid: message.uid }, flags: message.flags, received: message.received, raw: &raw };
        let path = maildir::store(root, &fetched).map_err(|e| SyncError::Disk(format!("{}: {e}", root.display())))?;
        written.push((message.uid, path));
    }
    Ok(written)
}

/// IMAP's system flags as Maildir's letters (<https://cr.yp.to/proto/maildir.html>),
/// and the keywords kept, `$Junk` and `$NotJunk`, as Dovecot's lowercase
/// letters (`maildir::KEYWORDS`): what you or a mail client said of a message,
/// which the spam filter learns from and the Porch heeds. `reconcile` keeps them in step.
pub(crate) fn maildir_flags<'a>(flags: impl Iterator<Item = Flag<'a>>) -> String {
    flags
        .filter_map(|flag| match flag {
            Flag::Seen => Some('S'),
            Flag::Answered => Some('R'),
            Flag::Flagged => Some('F'),
            Flag::Deleted => Some('T'),
            Flag::Draft => Some('D'),
            Flag::Custom(keyword) => maildir::keyword_letter(&keyword),
            _ => None,
        })
        .collect()
}

/// The first day a first sync brings: the first window (`sync_days`); older
/// mail follows, a round at a time.
fn first_window(account: &Account) -> jiff::civil::Date {
    let days = i64::from(account.sync_days.unwrap_or(DEFAULT_SYNC_DAYS));
    let today = jiff::Zoned::now().date();
    today.checked_sub(jiff::Span::new().days(days)).unwrap_or(today)
}

/// A day as IMAP writes dates: "18-Sep-2026", with English month names
/// whatever your language (RFC 9051 §9, date-month).
fn imap_date(day: jiff::civil::Date) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{}-{}-{}", day.day(), MONTHS[usize::from(day.month().unsigned_abs()) - 1], day.year())
}

/// Where sync stopped, per folder.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SyncState {
    #[serde(default)]
    folders: BTreeMap<String, FolderState>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
struct FolderState {
    uidvalidity: u32,
    last_uid: u32,
    /// The first day fetched, Unix seconds; unknown for folders synced before Sioul kept it.
    #[serde(default)]
    since: Option<i64>,
    /// Everything was fetched.
    #[serde(default)]
    everything: bool,
}

/// Where a folder's sync stopped, forgotten: the next sync of it starts over.
pub(crate) fn forget_folder_state(account: &Account, name: &str) -> Result<(), SyncError> {
    let state_path = state_dir().join("sync").join(format!("{}.toml", account.id));
    let _lock = lock(&state_path.with_extension("lock"))?;
    let mut state = SyncState::load(&state_path);
    if state.folders.remove(name).is_some() {
        state.save(&state_path)?;
    }
    Ok(())
}

impl SyncState {
    fn load(path: &Path) -> SyncState {
        std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    /// Written next to its place, then moved: an interruption never leaves
    /// half a state, which would be read as none (every folder's first window
    /// fetched again).
    fn save(&self, path: &Path) -> Result<(), SyncError> {
        let fail = |e: std::io::Error| SyncError::Disk(format!("{}: {e}", path.display()));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let text = toml::to_string(self).map_err(|e| SyncError::Disk(e.to_string()))?;
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, text).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
    }
}

/// An exclusive lock on a file, released when the returned file is dropped.
pub(crate) fn lock(path: &Path) -> Result<std::fs::File, SyncError> {
    let fail = |e: std::io::Error| SyncError::Disk(format!("{}: {e}", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path).map_err(fail)?;
    file.lock().map_err(fail)?;
    Ok(file)
}

fn runtime() -> Result<tokio::runtime::Runtime, SyncError> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| SyncError::Network(e.to_string()))
}

pub(crate) fn block_on<T>(future: impl Future<Output = Result<T, SyncError>>) -> Result<T, SyncError> {
    runtime()?.block_on(future)
}

/// Returns when the watcher is stopped or nudged.
async fn woken(control: &Control) {
    std::future::poll_fn(|context| {
        if control.wakes() {
            return Poll::Ready(());
        }
        control.listen(context.waker());
        // Rung between the look and the listening: not missed.
        if control.wakes() { Poll::Ready(()) } else { Poll::Pending }
    })
    .await;
}

/// Waits `length`, or less when stopped or nudged; on the wall clock too (see
/// `Deadline`). Whether it was nudged.
async fn wait(length: Duration, control: &Control) -> bool {
    let end = Deadline::after(length, SystemTime::now());
    loop {
        let left = end.left(SystemTime::now());
        if left.is_zero() {
            return false;
        }
        tokio::select! {
            () = tokio::time::sleep(left.min(LOOK)) => {}
            () = woken(control) => return !control.stopped(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imap_dates_are_english() {
        let day = jiff::civil::date(2026, 9, 18);
        assert_eq!(imap_date(day), "18-Sep-2026");
    }

    #[test]
    fn the_first_sync_is_short_whatever_the_history() {
        let mut account = Account::imap("a", "a@example.org", "imap.example.org", 993, Default::default(), None);
        let today = jiff::Zoned::now().date();
        assert_eq!(first_window(&account), today.checked_sub(jiff::Span::new().days(i64::from(DEFAULT_SYNC_DAYS))).unwrap());
        // The history only chooses what the window shows: everything is fetched after the first window.
        account.history_weeks = Some(0);
        account.sync_days = Some(3);
        assert_eq!(first_window(&account), today.checked_sub(jiff::Span::new().days(3)).unwrap());
        assert!(!Reach::first(&account).everything);
    }

    /// F3b: a nudge ends a pause at once, not at the next look half a second later.
    #[test]
    fn a_nudge_ends_a_pause_at_once() {
        let control = Control::default();
        let started = Instant::now();
        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(50));
                control.nudge();
            });
            control.pause(Duration::from_secs(10));
        });
        assert!(started.elapsed() < Duration::from_millis(400), "{:?}", started.elapsed());
    }

    /// F3b: the same for the mail watcher's waits, which say they were
    /// nudged (a failed connection is then tried again at once, and its
    /// pause starts over).
    #[test]
    fn a_nudge_ends_a_wait_at_once() {
        let control = Control::default();
        let started = Instant::now();
        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(50));
                control.nudge();
            });
            assert!(runtime().unwrap().block_on(wait(Duration::from_secs(10), &control)), "nudged");
        });
        assert!(started.elapsed() < Duration::from_millis(400), "{:?}", started.elapsed());
        assert!(!runtime().unwrap().block_on(wait(Duration::from_millis(20), &control)), "to its end");
    }

    /// A1: a pause counts the wall clock: a phone asleep past its end ends it
    /// as it wakes, not fifteen awake minutes later.
    #[test]
    fn a_pause_counts_the_time_asleep() {
        let control = Control::default();
        let start = SystemTime::now();
        let looks = std::cell::Cell::new(0);
        // The clock read at the start, at the first look, then after an hour asleep.
        let wall = || {
            looks.set(looks.get() + 1);
            if looks.get() <= 2 { start } else { start + Duration::from_secs(3600) }
        };
        let started = Instant::now();
        control.pause_by(Duration::from_secs(15 * 60), wall, Duration::from_millis(20));
        assert!(started.elapsed() < Duration::from_secs(1), "{:?}", started.elapsed());
        assert_eq!(looks.get(), 3);
    }

    /// Quiet (the app put away): a pause ends only when nudged, and leaving
    /// quiet is a nudge; the mail watcher brings the inbox's new mail only.
    #[test]
    fn quiet_waits_for_a_nudge() {
        let control = Control::default();
        assert_eq!(control.brings(true), (true, true));
        control.set_quiet(true);
        assert_eq!(control.brings(true), (false, false));
        let (done, ended) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| {
                control.pause(Duration::from_millis(30));
                done.send(()).unwrap();
            });
            assert!(ended.recv_timeout(Duration::from_millis(300)).is_err(), "not on its timer");
            control.nudge();
            ended.recv_timeout(Duration::from_secs(5)).expect("a nudge ends it");
            s.spawn(|| {
                control.pause(Duration::from_millis(30));
                done.send(()).unwrap();
            });
            assert!(ended.recv_timeout(Duration::from_millis(300)).is_err(), "still quiet");
            control.set_quiet(false);
            ended.recv_timeout(Duration::from_secs(5)).expect("leaving quiet ends it");
        });
        assert_eq!(control.brings(false), (true, false));
    }

    /// A2: a watcher parked after a lasting error waits for a nudge, quiet
    /// or not; stopped, it ends.
    #[test]
    fn a_parked_watcher_waits_for_a_nudge() {
        let control = Control::default();
        control.set_quiet(true);
        let (done, ended) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| done.send(control.park()).unwrap());
            assert!(ended.recv_timeout(Duration::from_millis(200)).is_err(), "parked");
            control.nudge();
            assert_eq!(ended.recv_timeout(Duration::from_secs(5)), Ok(true), "nudged: tries again");
            s.spawn(|| done.send(control.park()).unwrap());
            control.stop();
            assert_eq!(ended.recv_timeout(Duration::from_secs(5)), Ok(false), "stopped: ends");
        });
    }

    /// A2: a refused password is tried again once another is kept, or an hour
    /// later, however often nudged; anything else lasting, at the next nudge.
    #[test]
    fn a_refused_password_waits_for_another() {
        let parked = Parked::after(&SyncError::Login("refused".into()), Some("old".into()));
        assert!(parked.refused());
        assert!(!parked.again(Some("old")) && !parked.again(None));
        assert!(parked.again(Some("new")));
        let hour_later = Parked { since: SystemTime::now() - REFUSED_AGAIN, ..parked };
        assert!(hour_later.again(Some("old")));
        let portal = Parked::after(&SyncError::Tls("certificate".into()), Some("old".into()));
        assert!(!portal.refused() && portal.again(Some("old")));
    }

    /// A folder of this test's own, empty.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-fetch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// F2: a sync stopped between storing mail and saving where it stopped,
    /// or whose state was lost, does not store the same messages twice.
    #[test]
    fn mail_already_here_is_not_fetched_again() {
        let root = scratch("already-here");
        let message = |validity, uid| Fetched { origin: ImapOrigin { validity, uid }, flags: String::new(), received: None, raw: b"Subject: x\r\n\r\nx\r\n" };
        for (validity, uid) in [(7, 3), (7, 4), (6, 5)] {
            maildir::store(&root, &message(validity, uid)).unwrap();
        }
        assert_eq!(to_fetch([5, 3, 4], 2, &root, 7), (vec![5], 4), "5 of another UIDVALIDITY is another message");
        // Where sync stopped forgotten: the first window again, nothing twice.
        assert_eq!(to_fetch([3, 4], 0, &root, 7), (vec![], 4));
        assert_eq!(to_fetch([4], 4, &root, 7), (vec![], 4), "the newest, given again by UID 5:*");
        let _ = std::fs::remove_dir_all(root);
    }

    /// F2: where sync stopped is written next to its place, then moved: a
    /// write cut short never leaves half a state, which would be read as
    /// none (the last 14 days of every folder fetched again).
    #[test]
    fn where_sync_stopped_is_replaced_whole() {
        let dir = scratch("state");
        let path = dir.join("account.toml");
        let mut state = SyncState::default();
        state.folders.insert("INBOX".into(), FolderState { uidvalidity: 7, last_uid: 41, since: None, everything: true });
        state.save(&path).unwrap();
        // Another name for the same file: written over in place, it would change too.
        let before = dir.join("before.toml");
        std::fs::hard_link(&path, &before).unwrap();
        state.folders.get_mut("INBOX").unwrap().last_uid = 42;
        state.save(&path).unwrap();
        assert_eq!(SyncState::load(&before).folders["INBOX"].last_uid, 41, "replaced, not written over");
        assert_eq!(SyncState::load(&path).folders["INBOX"].last_uid, 42);
        assert!(!path.with_extension("toml.new").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn flags_become_maildir_letters() {
        let flags = [Flag::Seen, Flag::Flagged, Flag::Recent];
        assert_eq!(maildir_flags(flags.into_iter()), "SF");
        // `$Junk` and `$NotJunk` are kept, whatever their case; other keywords are not.
        let flags = [Flag::Seen, Flag::Custom("$NotJunk".into()), Flag::Custom("$Forwarded".into()), Flag::Custom("$junk".into()), Flag::Custom("NonJunk".into())];
        assert_eq!(maildir_flags(flags.into_iter()), format!("S{}{}", maildir::NOT_JUNK, maildir::JUNK));
    }
}

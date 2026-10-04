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
use std::time::Duration;

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

/// How a watcher is told to stop, or to fetch now instead of waiting, and at what pace.
#[derive(Debug, Default)]
pub struct Control {
    stop: AtomicBool,
    nudge: AtomicBool,
    /// Every folder, every minute, until switched off.
    realtime: AtomicBool,
    /// Seconds between two fetches of the other folders; 0 for the default.
    pace: std::sync::atomic::AtomicU64,
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

    /// Ends the watch at the next occasion (within half a second when waiting).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Fetches now: ends the current wait, IDLE or pause.
    pub fn nudge(&self) {
        self.nudge.store(true, Ordering::Relaxed);
    }

    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Waits `length` on this thread, or less when stopped or nudged.
    pub fn pause(&self, length: Duration) {
        let end = std::time::Instant::now() + length;
        while std::time::Instant::now() < end && !self.wakes() {
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Whether the wait should end; a nudge is used up by ending one.
    fn wakes(&self) -> bool {
        self.stopped() || self.nudge.swap(false, Ordering::Relaxed)
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

/// Lists the folders, keeps the list, and fetches every folder but the inbox;
/// returns the files written, and whether older mail waits for room.
async fn other_folders(session: &mut Imap, account: &Account) -> Result<(Vec<PathBuf>, bool), SyncError> {
    let list = mailbox::list(session).await?;
    mailbox::save(&account.id, &list)?;
    let (mut written, mut held_back) = (Vec::new(), false);
    for folder in list.iter().filter(|f| f.role != Role::Inbox && mailbox::fetched(f, &list, account)) {
        let report = fetch_folder(session, account, folder).await?;
        written.extend(report.new.into_iter().chain(report.elsewhere));
        held_back |= report.held_back;
    }
    Ok((written, held_back))
}

/// Keeps the inbox open and fetches what arrives, until stopped (see
/// [`Control`]). Every sync is reported, even an empty one, and every error,
/// before trying again; a refused login ends it (see [`SyncError::is_lasting`]).
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
        let mut pause = FIRST_PAUSE;
        while !control.stopped() {
            let Err(e) = keep_open(&server, password, account, control, &mut report, &mut pause).await else { continue };
            let lasting = e.is_lasting();
            report(Err(e));
            if lasting {
                return;
            }
            wait(pause, control).await;
            pause = (pause * 2).min(LONGEST_PAUSE);
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
        let mut new = fetch_new(&mut session, account).await?;
        if others_due {
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
    fetch_folder(session, account, &folders::folder(INBOX, None, None)).await
}

/// Fetches a folder's new messages into its Maildir, then brings back what
/// changed there on the server. One fetch at a time per account, across
/// processes too (the window and `sioul sync`): otherwise both would see the
/// same new UIDs and write each message twice.
async fn fetch_folder(session: &mut Imap, account: &Account, folder: &Folder) -> Result<Report, SyncError> {
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
    // "UID n:*" also returns the newest message when nothing is newer (RFC 9051 §6.4.8).
    let mut uids: Vec<u32> = found.into_iter().filter(|uid| *uid > last_uid).collect();
    uids.sort_unstable();
    let root = account.maildir_path().join(&folder.local);
    let mut report = Report { account: account.id.clone(), new: Vec::new(), first: known.is_none(), inbox: inbox.exists, elsewhere: Vec::new(), held_back: false };
    let mut newest = last_uid;
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
    if known.is_none() && uids.is_empty() {
        // Nothing in the first window: remember the folder anyway, so the next
        // sync looks only at arrivals. Without UIDNEXT, the window is used again.
        if let Some(next) = inbox.uid_next {
            let reached = Reach::first(account);
            state.folders.insert(key.into(), FolderState { uidvalidity: validity, last_uid: next.saturating_sub(1), since: reached.since, everything: reached.everything });
            state.save(&state_path)?;
        }
    }
    if let Some(known) = known {
        mailbox::reconcile(session, &root, validity).await?;
        backfill(session, &mut state, &state_path, key, known, &root, validity, &mut report).await?;
    }
    Ok(report)
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

/// IMAP's system flags as Maildir's letters (https://cr.yp.to/proto/maildir.html).
pub(crate) fn maildir_flags<'a>(flags: impl Iterator<Item = Flag<'a>>) -> String {
    flags
        .filter_map(|flag| match flag {
            Flag::Seen => Some('S'),
            Flag::Answered => Some('R'),
            Flag::Flagged => Some('F'),
            Flag::Deleted => Some('T'),
            Flag::Draft => Some('D'),
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

    fn save(&self, path: &Path) -> Result<(), SyncError> {
        let fail = |e: std::io::Error| SyncError::Disk(format!("{}: {e}", path.display()));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let text = toml::to_string(self).map_err(|e| SyncError::Disk(e.to_string()))?;
        std::fs::write(path, text).map_err(fail)
    }
}

/// An exclusive lock on a file, released when the returned file is dropped.
fn lock(path: &Path) -> Result<std::fs::File, SyncError> {
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
    while !control.wakes() {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Waits `length`, or less when stopped or nudged.
async fn wait(length: Duration, control: &Control) {
    tokio::select! {
        () = tokio::time::sleep(length) => {}
        () = woken(control) => {}
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

    #[test]
    fn flags_become_maildir_letters() {
        let flags = [Flag::Seen, Flag::Flagged, Flag::Recent];
        assert_eq!(maildir_flags(flags.into_iter()), "SF");
    }
}

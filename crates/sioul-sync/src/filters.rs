// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail filters, on the server (docs/client.md, "Filters"; what they say and
//! how a message is matched: `sioul_core::rules`).
//!
//! After each fetch, on every device that fetches (the window's watchers, a
//! phone's background step, `sioul watch`), the inbox's arrivals still unread
//! are judged as the Porch judges them, what it protects left out (mail set
//! aside, hostile mail, the spam filter's review queue, a code you asked
//! for), and what the filters that match say is done on the server: flags
//! and keywords first, then the move, one command per folder. Never on an
//! account's first fetch (its last two weeks), never on what was there
//! before: running them on the whole inbox is yours to ask (`run`, unmarked).
//!
//! No race between devices: before acting, a device marks the message
//! `$SioulFiltered` (`rules::FILTERED`) and acts only on what it marked.
//! With CONDSTORE (RFC 7162), the mark is set only if nothing changed the
//! message since its flags were read (`UNCHANGEDSINCE`): of two devices
//! fetching it in the same instant, one marks it, the other's store fails and
//! it leaves the message alone. Without CONDSTORE, the flags are read, then
//! the mark set: two devices would have to read in the same round trip. A
//! message read meanwhile, or gone from the inbox, is left alone. An act
//! that fails takes the mark off again, and the message waits for the next
//! fetch here (`Waiting`: three tries, two days) or another device's. A
//! server that keeps no keywords of its own (no `\*` in PERMANENTFLAGS:
//! Exchange, Outlook.com) cannot carry the mark: each device acts on what it
//! fetched; a move, a flag or a read mark done twice leaves the message as once.
//!
//! What is told (`Done::to_tell`, for the new-mail notifications): a message
//! a filter could not act on here is told as any new mail is, by the device
//! that fetched it; one it acted on, read meanwhile or gone, is not. One
//! another device marked is that device's to do: not told here yet, it waits
//! here too and is looked at again at the next fetch (the other device's act
//! wakes the inbox's watcher). Unmarked and still there, unread, it is this
//! device's to do; still marked ten minutes on (`STUCK`: that device stopped
//! half-way), it is told here.

use crate::SyncError;
use crate::imap::{self, COMMAND, Imap, Server};
use crate::mailbox;
use async_imap::types::Flag;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sioul_core::card::ImapOrigin;
use sioul_core::config::{Account, Config, state_dir};
use sioul_core::folders::{self, Folder, Role};
use sioul_core::maildir;
use sioul_core::rules::{self, Destination, Plan};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// One message of an inbox, and what the filters do to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub file: PathBuf,
    pub plan: Plan,
    /// Tried before and failed: it may be read now (the flags went, the move did not).
    pub again: bool,
    /// Found marked by another device before, since then (Unix seconds): looked at again.
    pub claimed_since: Option<i64>,
}

/// Why a message could not be filtered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// No folder of that name in the account (as the filter names it).
    NoFolder(String),
    /// The server keeps no keywords: a filter whose only act is one has nothing to do.
    NoKeywords,
    /// The server refused, or did not answer.
    Server(String),
    /// The server renumbered the inbox since it was fetched here: no longer that message.
    Renumbered,
    /// Another device marked it and has not acted on it for ten minutes (`STUCK`).
    Claimed,
}

/// What became of a run.
#[derive(Debug, Clone, Default)]
pub struct Done {
    /// Filtered here.
    pub acted: Vec<Job>,
    /// Left alone, someone having dealt with it: read meanwhile, or gone from the inbox.
    pub left: Vec<PathBuf>,
    /// Marked by another device, which acts on it: not told here, and looked at again at the next fetch.
    pub elsewhere: Vec<Job>,
    /// Not done, and why; marked again for nobody: tried again at the next fetch. Told as new mail.
    pub failed: Vec<(Job, Failure)>,
    /// The server keeps no keywords: nothing was marked.
    pub unmarked: bool,
}

impl Done {
    /// The messages a filter could not act on here: told as any new mail is
    /// (the window's `mailnote`), which held them back for the filters. A
    /// retry that works later does not tell them again.
    pub fn to_tell(&self) -> Vec<PathBuf> {
        self.failed.iter().map(|(job, _)| job.file.clone()).collect()
    }

    /// What could not be done, in a sentence, for `account` (its address as
    /// you read it): the first failure, with how many failed so; none when
    /// everything was done.
    pub fn said(&self, tr: &sioul_core::i18n::Translator, account: &str) -> Option<String> {
        let (_, first) = self.failed.first()?;
        let alike = self.failed.iter().filter(|(_, why)| std::mem::discriminant(why) == std::mem::discriminant(first)).count();
        let mut args = tr.counted(alike);
        args.set("account", account.to_string());
        let id = match first {
            Failure::NoFolder(folder) => {
                args.set("folder", folder.clone());
                "filter-failed-folder"
            }
            Failure::NoKeywords => "filter-failed-keywords",
            Failure::Server(_) => "filter-failed-server",
            Failure::Renumbered => "filter-failed-renumbered",
            Failure::Claimed => "filter-failed-claimed",
        };
        Some(tr.text(id, Some(&args)))
    }
}

/// What the filters do to these messages of `account`'s inbox, judged as
/// the Porch judges them now: what it protects, and what no filter takes, left out.
pub fn jobs(config: &Config, account: &Account, files: &[PathBuf]) -> Vec<Job> {
    let filters = &config.mail.filters;
    if files.is_empty() || !filters.iter().any(|f| f.runs_on(&account.id)) {
        return Vec::new();
    }
    let ties = sioul_core::links::LocalLinks::load(&sioul_core::links::LocalLinks::default_path());
    let store = config.case_store_path().and_then(|root| sioul_core::cases::CaseStore::load(&root).ok()).map(|s| s.with_ties(&ties));
    let known = sioul_core::porch::KnownSenders::load(&config.known_senders_path());
    let senders = sioul_core::porch::Senders::load(config);
    let now = jiff::Timestamp::now().as_second();
    let judged = sioul_core::porch::judge(files, &config.mail_sources(), store.as_ref(), &known, &senders, now);
    rules::plans(filters, &judged, &senders).into_iter().filter(|(file, _)| files.contains(file)).map(|(file, plan)| Job { file, plan, again: false, claimed_since: None }).collect()
}

/// The messages of `account`'s inbox kept here, read or not.
pub fn inbox_files(account: &Account) -> Vec<PathBuf> {
    let root = account.maildir_path();
    ["new", "cur"]
        .iter()
        .filter_map(|sub| std::fs::read_dir(root.join(sub)).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| maildir::origin_of(p).is_some())
        .collect()
}

/// After a fetch of `account`'s inbox (`new`: its arrivals, oldest first,
/// none at all when only something waits): those still unread, and those an
/// earlier look left waiting (a failure, another device's mark), filtered on
/// the server, each marked first. Nothing on its first fetch.
pub fn after_fetch(config: &Config, account: &Account, password: &str, new: &[PathBuf], first: bool) -> Done {
    if first || !config.mail.filters.iter().any(|f| f.runs_on(&account.id)) {
        return Done::default();
    }
    let (waiting, mut settled) = Waiting::files_of(account, jiff::Timestamp::now().as_second());
    let mut files: Vec<PathBuf> = new.iter().filter(|f| !maildir::flags_of(f).contains('S')).cloned().collect();
    for (file, _) in &waiting {
        if !files.contains(file) {
            files.push(file.clone());
        }
    }
    if files.is_empty() {
        Waiting::note(account, &Done::default(), &settled);
        return Done::default();
    }
    let mut jobs = jobs(config, account, &files);
    for job in &mut jobs {
        if let Some((_, wait)) = waiting.iter().find(|(file, _)| *file == job.file) {
            if wait.elsewhere {
                job.claimed_since = Some(wait.since);
            } else {
                job.again = true;
            }
        }
    }
    let done = if jobs.is_empty() {
        Done::default()
    } else {
        run(account, password, &jobs, true).unwrap_or_else(|e| Done { failed: jobs.iter().map(|j| (j.clone(), Failure::Server(format!("{e:?}")))).collect(), ..Done::default() })
    };
    // What waited and no filter takes any more (changed meanwhile, read, set aside) waits no more.
    settled.extend(waiting.iter().filter(|(file, _)| !jobs.iter().any(|j| &j.file == file)).map(|(_, wait)| (wait.validity, wait.uid)));
    Waiting::note(account, &done, &settled);
    done
}

/// Whether messages of `account` are due for another look here (another
/// device's mark to see through; a failure to try again, ten minutes after
/// the last try at least): its next fetch runs the filters for them, new mail or not.
pub fn due(account: &Account) -> bool {
    let now = jiff::Timestamp::now().as_second();
    Waiting::load(&Waiting::path()).messages.iter().any(|w| w.account == account.id && w.due(now))
}

/// The plans done on `account`'s inbox, in one session: with `claim`, each
/// message marked first, and left alone when another device marked it, it
/// was read meanwhile or it left the inbox (after a fetch); without, every
/// job, as you asked (the whole inbox). The copies here follow.
pub fn run(account: &Account, password: &str, jobs: &[Job], claim: bool) -> Result<Done, SyncError> {
    let server = Server::of(account)?;
    let placed: Vec<(ImapOrigin, Job)> = jobs.iter().filter_map(|j| Some((maildir::origin_of(&j.file)?, j.clone()))).collect();
    if placed.is_empty() {
        return Ok(Done::default());
    }
    let done = crate::fetch::block_on(async {
        let mut session = imap::open(&server, password).await?;
        let done = run_in(&mut session, account, placed, claim).await;
        let _ = session.logout().await;
        done
    })?;
    for job in &done.acted {
        follow_here(job);
    }
    Ok(done)
}

async fn run_in(session: &mut Imap, account: &Account, placed: Vec<(ImapOrigin, Job)>, claim: bool) -> Result<Done, SyncError> {
    let capabilities = imap::within(COMMAND, session.capabilities()).await?.map_err(imap::server)?;
    let (can_move, uidplus, condstore) = (capabilities.has_str("MOVE"), capabilities.has_str("UIDPLUS"), capabilities.has_str("CONDSTORE"));
    let inbox = if condstore { imap::within(COMMAND, session.select_condstore("INBOX")).await? } else { imap::within(COMMAND, session.select("INBOX")).await? }.map_err(imap::server)?;
    let validity = inbox.uid_validity.unwrap_or(0);
    // Keywords of its own: `\*` among the flags it keeps, or the mark itself.
    let marks = inbox.permanent_flags.iter().any(|f| matches!(f, Flag::MayCreate) || matches!(f, Flag::Custom(k) if k.eq_ignore_ascii_case(rules::FILTERED)));
    let mut done = Done { unmarked: !marks, ..Done::default() };
    let mut here: Vec<(u32, Job)> = Vec::new();
    for (origin, job) in placed {
        // Renumbered since: no longer this message, not filtered; told as new mail.
        if origin.validity == validity { here.push((origin.uid, job)) } else { done.failed.push((job, Failure::Renumbered)) }
    }
    let mine = if claim { claimed(session, here, marks, condstore, &mut done).await? } else { here };
    // Flags and keywords first, each message its own; then the moves, one per folder.
    let mut moves: BTreeMap<String, Vec<(u32, Job)>> = BTreeMap::new();
    let mut failed: Vec<(u32, Job, Failure)> = Vec::new();
    for (uid, job) in mine {
        let junk = job.plan.to == Some(Destination::Junk);
        let mut added: Vec<String> = Vec::new();
        if job.plan.read {
            added.push("\\Seen".into());
        }
        if job.plan.flag {
            added.push("\\Flagged".into());
        }
        if marks {
            added.extend(job.plan.keywords.iter().cloned());
            if junk {
                added.push("$Junk".into());
            }
        }
        // Nothing this server can keep, and nowhere to go: a keyword alone, without keywords.
        if added.is_empty() && job.plan.to.is_none() {
            failed.push((uid, job, Failure::NoKeywords));
            continue;
        }
        if !added.is_empty() {
            if let Err(e) = mailbox::store(session, &uid.to_string(), &format!("+FLAGS.SILENT ({})", added.join(" "))).await {
                failed.push((uid, job, Failure::Server(format!("{e:?}"))));
                continue;
            }
            if junk && marks {
                let _ = mailbox::store(session, &uid.to_string(), "-FLAGS.SILENT ($NotJunk)").await;
            }
        }
        match &job.plan.to {
            None => done.acted.push(job),
            Some(to) => match target(session, account, to).await {
                Ok(Some(folder)) => moves.entry(folder).or_default().push((uid, job)),
                // Into the inbox: where it is already.
                Ok(None) => done.acted.push(job),
                Err(why) => failed.push((uid, job, why)),
            },
        }
    }
    for (folder, list) in moves {
        let uids: BTreeSet<u32> = list.iter().map(|(uid, _)| *uid).collect();
        match mailbox::move_to(session, &mailbox::ranges(&uids), &folder, can_move, uidplus).await {
            Ok(()) => done.acted.extend(list.into_iter().map(|(_, job)| job)),
            Err(e) => failed.extend(list.into_iter().map(|(uid, job)| (uid, job, Failure::Server(format!("{e:?}"))))),
        }
    }
    // Not done: unmarked again, for the next fetch or another device to try.
    if claim && marks && !failed.is_empty() {
        let uids: BTreeSet<u32> = failed.iter().map(|(uid, _, _)| *uid).collect();
        let _ = mailbox::store(session, &mailbox::ranges(&uids), &format!("-FLAGS.SILENT ({})", rules::FILTERED)).await;
    }
    done.failed.extend(failed.into_iter().map(|(_, job, why)| (job, why)));
    Ok(done)
}

/// The jobs this device may do: those still unread (or tried before), not
/// marked by another device, still in the inbox; then marked, each only if
/// nothing changed it since its flags were read (CONDSTORE), else all at once.
async fn claimed(session: &mut Imap, jobs: Vec<(u32, Job)>, marks: bool, condstore: bool, done: &mut Done) -> Result<Vec<(u32, Job)>, SyncError> {
    let uids: BTreeSet<u32> = jobs.iter().map(|(uid, _)| *uid).collect();
    // Each message's flags now: read, marked, and its mod-sequence where the server counts changes.
    let mut now: BTreeMap<u32, (bool, bool, Option<u64>)> = BTreeMap::new();
    {
        let query = if condstore { "(UID FLAGS MODSEQ)" } else { "(UID FLAGS)" };
        let mut fetches = imap::within(COMMAND, session.uid_fetch(mailbox::ranges(&uids), query)).await?.map_err(imap::server)?;
        while let Some(fetch) = fetches.next().await {
            let fetch = fetch.map_err(imap::server)?;
            let Some(uid) = fetch.uid else { continue };
            let seen = fetch.flags().any(|f| matches!(f, Flag::Seen));
            let marked = fetch.flags().any(|f| matches!(&f, Flag::Custom(k) if k.eq_ignore_ascii_case(rules::FILTERED)));
            now.insert(uid, (seen, marked, fetch.modseq));
        }
    }
    let at = jiff::Timestamp::now().as_second();
    let mut free: Vec<(u32, Job, Option<u64>)> = Vec::new();
    for (uid, job) in jobs {
        let flags = now.get(&uid);
        match found(flags.map(|(seen, marked, _)| (*seen, *marked)), &job, at) {
            Found::Free => free.push((uid, job, flags.and_then(|(_, _, modseq)| *modseq))),
            Found::Settled => done.left.push(job.file),
            Found::Elsewhere => done.elsewhere.push(job),
            Found::Stuck => done.failed.push((job, Failure::Claimed)),
        }
    }
    if !marks || free.is_empty() {
        return Ok(free.into_iter().map(|(uid, job, _)| (uid, job)).collect());
    }
    let mut mine = Vec::new();
    let mut plain = Vec::new();
    for (uid, job, modseq) in free {
        match modseq.filter(|_| condstore) {
            Some(modseq) => {
                // Only the messages it changed come back, with their new mod-sequence (RFC 7162 §3.1.3).
                let echoed = stored(session, &uid.to_string(), &format!("(UNCHANGEDSINCE {modseq}) +FLAGS.SILENT ({})", rules::FILTERED)).await?;
                // Changed meanwhile: another device's mark, most likely; looked at again.
                if echoed.contains(&uid) { mine.push((uid, job)) } else { done.elsewhere.push(job) }
            }
            None => plain.push((uid, job)),
        }
    }
    if !plain.is_empty() {
        let uids: BTreeSet<u32> = plain.iter().map(|(uid, _)| *uid).collect();
        mailbox::store(session, &mailbox::ranges(&uids), &format!("+FLAGS.SILENT ({})", rules::FILTERED)).await?;
        mine.extend(plain);
    }
    Ok(mine)
}

/// A STORE, and the UIDs of the messages the server says it changed.
async fn stored(session: &mut Imap, uid: &str, query: &str) -> Result<BTreeSet<u32>, SyncError> {
    let mut answers = imap::within(COMMAND, session.uid_store(uid, query)).await?.map_err(imap::server)?;
    let mut echoed = BTreeSet::new();
    while let Some(answer) = answers.next().await {
        if let Some(uid) = answer.map_err(imap::server)?.uid {
            echoed.insert(uid);
        }
    }
    Ok(echoed)
}

/// What a device may do with a message it is to filter, by its flags on the server now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Found {
    /// Unread, unmarked: this device marks it and acts.
    Free,
    /// Read meanwhile, or gone from the inbox: someone dealt with it.
    Settled,
    /// Marked by another device, which acts on it: looked at again later.
    Elsewhere,
    /// Marked by another device that has not acted on it for `STUCK`: told here.
    Stuck,
}

/// Another device's mark that stays this long, the message neither read nor
/// gone: that device stopped half-way (closed, out of battery, out of reach).
const STUCK: i64 = 10 * 60;

/// What a device may do with `job`'s message, from its flags on the server
/// (`flags`: read, marked; none: gone from the inbox), `now` in Unix seconds.
fn found(flags: Option<(bool, bool)>, job: &Job, now: i64) -> Found {
    match flags {
        None => Found::Settled,
        // Read meanwhile; a failure tried again goes on (its flags may have gone, not its move).
        Some((true, _)) if !job.again => Found::Settled,
        Some((_, true)) => match job.claimed_since {
            Some(since) if now - since >= STUCK => Found::Stuck,
            _ => Found::Elsewhere,
        },
        Some(_) => Found::Free,
    }
}

/// The server's name of the folder a plan sends a message to; none for the inbox itself.
async fn target(session: &mut Imap, account: &Account, to: &Destination) -> Result<Option<String>, Failure> {
    let (role, name) = match to {
        Destination::Junk => (Role::Junk, "Junk"),
        Destination::Trash => (Role::Trash, "Trash"),
        // As the Archive button: its archive, else Gmail's All Mail, else one made.
        Destination::Archive => {
            let known = mailbox::folders(&account.id);
            if let Some(folder) = folders::preferred(&known, Role::Archive).or(folders::preferred(&known, Role::All)) {
                return Ok(Some(folder.name.clone()));
            }
            (Role::Archive, "Archive")
        }
        Destination::Folder(name) => {
            return match folder_named(&account.id, name) {
                Some(folder) if folder.role == Role::Inbox => Ok(None),
                Some(folder) => Ok(Some(folder.name)),
                None => Err(Failure::NoFolder(name.clone())),
            };
        }
    };
    mailbox::role_folder(session, account, role, name).await.map(Some).map_err(|e| Failure::Server(format!("{e:?}")))
}

/// The folder of `account` a filter names: as you read it ("Banque",
/// "Factures/2026"), else as its server writes it ("INBOX.Banque"), case
/// aside, and the levels' separator, "/" or ".", either way.
pub fn folder_named(account: &str, name: &str) -> Option<Folder> {
    let wanted = name.trim().to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let all = mailbox::folders(account);
    let levels = |text: &str| text.to_lowercase().replace(['/', '.'], "\u{1f}");
    all.iter()
        .find(|f| f.display.to_lowercase() == wanted || f.name.to_lowercase() == wanted)
        .or_else(|| all.iter().find(|f| levels(&f.display) == levels(&wanted)))
        .cloned()
}

/// The copy here, once the server took a job: gone when it left the inbox
/// (the next fetch brings it into its folder), else its new flags.
fn follow_here(job: &Job) {
    let Some(file) = maildir::locate(&job.file) else { return };
    if job.plan.leaves() {
        let _ = std::fs::remove_file(&file);
        return;
    }
    let mut flags = maildir::flags_of(&file);
    for (wanted, letter) in [(job.plan.read, 'S'), (job.plan.flag, 'F')] {
        if wanted && !flags.contains(letter) {
            flags.push(letter);
        }
    }
    let _ = maildir::set_flags(&file, &flags);
}

/// Messages whose filtering failed here, tried again at the next fetches
/// (three tries in two days at most), and those another device marked,
/// looked at again until it acts or `STUCK` (`$XDG_STATE_HOME/sioul/filters/waiting.toml`,
/// by account and place on the server; never a word of the message).
#[derive(Debug, Default, Serialize, Deserialize)]
struct Waiting {
    #[serde(default, rename = "message")]
    messages: Vec<Wait>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Wait {
    account: String,
    validity: u32,
    uid: u32,
    tries: u32,
    /// The first failure, or when another device's mark was first found, Unix seconds.
    since: i64,
    /// Marked by another device: looked at again, never tried here while so.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    elsewhere: bool,
}

const TRIES: u32 = 3;
const WAITS: i64 = 2 * 86_400;
/// A failure is tried again this long after the last try at least (counted from the first).
const RETRY: i64 = 10 * 60;

impl Wait {
    /// Due for another look: another device's mark, at each fetch; a failure,
    /// ten minutes after its last try at least.
    fn due(&self, now: i64) -> bool {
        self.elsewhere || now - self.since >= i64::from(self.tries) * RETRY
    }
}

impl Waiting {
    fn path() -> PathBuf {
        state_dir().join("filters").join("waiting.toml")
    }

    fn load(path: &Path) -> Waiting {
        std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    /// The files of `account`'s inbox due for another look at `now`, with
    /// why; and where those no longer here were (gone from the inbox: waiting no more).
    fn files_of(account: &Account, now: i64) -> (Vec<(PathBuf, Wait)>, Vec<(u32, u32)>) {
        let waiting = Waiting::load(&Waiting::path());
        let wanted: Vec<&Wait> = waiting.messages.iter().filter(|w| w.account == account.id).collect();
        if wanted.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let here: Vec<(PathBuf, Wait)> = inbox_files(account)
            .into_iter()
            .filter_map(|file| {
                let origin = maildir::origin_of(&file)?;
                let wait = wanted.iter().find(|w| w.validity == origin.validity && w.uid == origin.uid)?;
                Some((file, (*wait).clone()))
            })
            .collect();
        let gone = wanted.iter().filter(|w| !here.iter().any(|(_, h)| h == **w)).map(|w| (w.validity, w.uid)).collect();
        (here.into_iter().filter(|(_, wait)| wait.due(now)).collect(), gone)
    }

    /// What a run did: its failures wait (one more try each), what another
    /// device marked waits to be looked at again, the rest and `settled` wait
    /// no more; three tries, or two days, and a message waits no more.
    fn note(account: &Account, done: &Done, settled: &[(u32, u32)]) {
        let path = Waiting::path();
        let at = |file: &PathBuf| maildir::origin_of(file).map(|o| (o.validity, o.uid));
        let now = jiff::Timestamp::now().as_second();
        // Never tried again: a keyword its server cannot keep, a renumbered inbox, a mark another device left.
        let (never, failed): (Vec<_>, Vec<_>) = done.failed.iter().partition(|(_, why)| matches!(why, Failure::NoKeywords | Failure::Renumbered | Failure::Claimed));
        let failed: BTreeSet<(u32, u32)> = failed.iter().filter_map(|(job, _)| at(&job.file)).collect();
        // Another device's to do: since when it was first found so.
        let elsewhere: BTreeMap<(u32, u32), i64> = done.elsewhere.iter().filter_map(|job| Some((at(&job.file)?, job.claimed_since.unwrap_or(now)))).collect();
        let mut over: BTreeSet<(u32, u32)> = done.acted.iter().map(|j| &j.file).chain(&done.left).chain(never.iter().map(|(job, _)| &job.file)).filter_map(at).collect();
        over.extend(settled.iter().copied());
        if failed.is_empty() && elsewhere.is_empty() && over.is_empty() && !path.exists() {
            return;
        }
        sioul_core::filelock::with_lock(&path, || {
            let mut waiting = Waiting::load(&path);
            let mine = |w: &Wait, (validity, uid): (u32, u32)| w.account == account.id && w.validity == validity && w.uid == uid;
            for &origin in &failed {
                match waiting.messages.iter_mut().find(|w| mine(w, origin)) {
                    // Another device's until now: this device's own tries from here on.
                    Some(wait) if wait.elsewhere => {
                        wait.elsewhere = false;
                        wait.tries = 1;
                        wait.since = now;
                    }
                    Some(wait) => wait.tries += 1,
                    None => waiting.messages.push(Wait { account: account.id.clone(), validity: origin.0, uid: origin.1, tries: 1, since: now, elsewhere: false }),
                }
            }
            for (&origin, &since) in &elsewhere {
                match waiting.messages.iter_mut().find(|w| mine(w, origin)) {
                    Some(wait) if wait.elsewhere => {}
                    // A failure here, another device's to do now: looked at again, not tried.
                    Some(wait) => {
                        wait.elsewhere = true;
                        wait.since = since;
                    }
                    None => waiting.messages.push(Wait { account: account.id.clone(), validity: origin.0, uid: origin.1, tries: 0, since, elsewhere: true }),
                }
            }
            waiting.messages.retain(|w| !(w.account == account.id && over.contains(&(w.validity, w.uid))) && (w.elsewhere || w.tries < TRIES) && now - w.since < WAITS);
            let text = toml::to_string(&waiting).unwrap_or_default();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, text);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::rules::{Act, Condition, Field, Filter, Test};

    #[test]
    fn a_filter_names_a_folder_as_you_read_it() {
        crate::dav::stand_in::home();
        let account = format!("filters-folders-{}", std::process::id());
        let path = sioul_core::folders::saved_path(&account);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let listed = [sioul_core::folders::folder("INBOX", Some("."), None), sioul_core::folders::folder("INBOX.Banque", Some("."), None), sioul_core::folders::folder("INBOX.Factures.2026", Some("."), None)];
        mailbox::save(&account, &listed).unwrap();
        assert_eq!(folder_named(&account, "banque").map(|f| f.name), Some("INBOX.Banque".into()));
        assert_eq!(folder_named(&account, "INBOX.Banque").map(|f| f.name), Some("INBOX.Banque".into()));
        assert_eq!(folder_named(&account, "Factures/2026").map(|f| f.name), Some("INBOX.Factures.2026".into()));
        assert_eq!(folder_named(&account, "inbox").map(|f| f.role), Some(Role::Inbox));
        assert_eq!(folder_named(&account, "Impôts"), None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn what_a_job_leaves_here() {
        let root = std::env::temp_dir().join(format!("sioul-filters-here-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let raw = b"From: a@bank.example\r\nSubject: x\r\n\r\nx\r\n".to_vec();
        let stored = |uid| maildir::store(&root, &maildir::Fetched { origin: ImapOrigin { validity: 7, uid }, flags: String::new(), received: None, raw: &raw }).unwrap();
        let (kept, moved) = (stored(1), stored(2));
        follow_here(&Job { file: kept.clone(), plan: Plan { read: true, flag: true, by: vec![0], ..Plan::default() }, again: false, claimed_since: None });
        let kept = maildir::locate(&kept).unwrap();
        assert!(maildir::flags_of(&kept).contains('S') && maildir::flags_of(&kept).contains('F'));
        follow_here(&Job { file: moved.clone(), plan: Plan { to: Some(Destination::Folder("Banque".into())), by: vec![0], ..Plan::default() }, again: false, claimed_since: None });
        assert!(maildir::locate(&moved).is_none(), "moved: the next fetch brings it into its folder");
        let _ = std::fs::remove_dir_all(&root);
        // A filter that matches nothing makes no job; the jobs say what each would do.
        let filter = Filter { conditions: vec![Condition::new(Field::From, Test::Contains, "@bank.example")], actions: vec![Act::Read], ..Filter::default() };
        assert!(filter.runs_on("home"));
    }

    #[test]
    fn what_is_told_and_what_waits() {
        let job = |name: &str, again: bool, claimed_since: Option<i64>| Job { file: PathBuf::from(name), plan: Plan { read: true, by: vec![0], ..Plan::default() }, again, claimed_since };
        let now = 1_800_000_000;
        // Gone or read: someone dealt with it. Another device's mark: its to do, ten minutes long. Unmarked and unread: ours.
        assert_eq!(found(None, &job("a", false, None), now), Found::Settled);
        assert_eq!(found(Some((true, false)), &job("a", false, None), now), Found::Settled);
        assert_eq!(found(Some((true, false)), &job("a", true, None), now), Found::Free, "a failure tried again goes on: its move may be missing still");
        assert_eq!(found(Some((false, true)), &job("a", false, None), now), Found::Elsewhere);
        assert_eq!(found(Some((false, true)), &job("a", false, Some(now - 60)), now), Found::Elsewhere);
        assert_eq!(found(Some((false, true)), &job("a", false, Some(now - STUCK)), now), Found::Stuck);
        assert_eq!(found(Some((true, true)), &job("a", false, Some(now - STUCK)), now), Found::Settled, "read meanwhile: nothing to tell");
        assert_eq!(found(Some((false, false)), &job("a", false, Some(now - 60)), now), Found::Free, "the other device's mark gone, the message still there and unread: ours to do");
        // Looked at again: another device's mark at each fetch; a failure ten minutes after its last try.
        let wait = |tries: u32, since: i64, elsewhere: bool| Wait { account: "home".into(), validity: 7, uid: 1, tries, since, elsewhere };
        assert!(wait(0, now, true).due(now));
        assert!(!wait(1, now - 60, false).due(now) && wait(1, now - RETRY, false).due(now));
        assert!(!wait(2, now - RETRY, false).due(now) && wait(2, now - 2 * RETRY, false).due(now));
        // Told as new mail: whatever a filter could not act on here; not what it did, what someone dealt with, nor what another device holds.
        let done = Done {
            acted: vec![job("moved", false, None)],
            left: vec![PathBuf::from("read")],
            elsewhere: vec![job("held", false, Some(now))],
            failed: vec![(job("no-folder", false, None), Failure::NoFolder("Banque".into())), (job("stuck", false, Some(0)), Failure::Claimed), (job("renumbered", false, None), Failure::Renumbered)],
            unmarked: false,
        };
        assert_eq!(done.to_tell(), ["no-folder", "stuck", "renumbered"].map(PathBuf::from).to_vec());
        assert!(Done::default().to_tell().is_empty());
        // Said: the first failure, with how many failed so.
        let tr = sioul_core::i18n::Translator::new("en");
        let said = done.said(&tr, "noa@example.org").unwrap();
        assert!(said.starts_with("One message of noa@example.org could not be moved") && said.contains("“Banque”"), "{said}");
        let stuck = Done { failed: done.failed[1..].to_vec(), ..Done::default() };
        assert!(stuck.said(&tr, "noa@example.org").unwrap().contains("another of your devices"));
        assert!(Done::default().said(&tr, "x").is_none());
    }

    /// Against GreenMail (docs/building.md), with invented mail only:
    /// `SIOUL_TEST_INSECURE_TLS=1 cargo test -p sioul-sync --features
    /// insecure-test-tls -- --ignored greenmail_filters`. Two devices filter
    /// the same arrivals: one acts, the other leaves them; a code is never
    /// touched; a folder missing fails, unmarks, and the next fetch tries
    /// again; the run you ask for takes read mail too.
    #[cfg(feature = "insecure-test-tls")]
    #[test]
    #[ignore]
    fn greenmail_filters_act_once_and_try_again() {
        use sioul_core::config::Security;
        let home = crate::dav::stand_in::home();
        assert!(std::env::var_os("SIOUL_TEST_INSECURE_TLS").is_some(), "GreenMail tests need SIOUL_TEST_INSECURE_TLS=1 and the container sioul-greenmail (docs/building.md)");
        let login = format!("filters-{}-{}@example.org", std::process::id(), jiff::Timestamp::now().as_second());
        let mut account = Account::imap("filters-tester", &login, "localhost", 3993, Security::Tls, None);
        account.maildir = Some(home.join(format!("mail-filters-{}", std::process::id())).display().to_string());
        let server = Server::of(&account).unwrap();
        // Dated now: a code is a code while it is valid.
        let date = jiff::Zoned::now().strftime("%a, %d %b %Y %H:%M:%S %z").to_string();
        let message = |from: &str, subject: &str, extra: &str, body: &str| format!("From: {from}\r\nTo: {login}\r\nSubject: {subject}\r\nDate: {date}\r\nMessage-ID: <{}.{}@example.org>\r\n{extra}MIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{body}\r\n", subject.replace(' ', "."), std::process::id()).into_bytes();
        let on_server = |what: &'static str, folder: &'static str| {
            crate::fetch::block_on(async {
                let mut session = imap::open(&server, "x").await?;
                imap::within(COMMAND, session.select(folder)).await?.map_err(imap::server)?;
                let mut found: Vec<(String, Vec<String>)> = Vec::new();
                {
                    let mut fetches = imap::within(COMMAND, session.uid_fetch("1:*", "(UID FLAGS BODY.PEEK[HEADER])")).await?.map_err(imap::server)?;
                    while let Some(fetch) = fetches.next().await {
                        let fetch = fetch.map_err(imap::server)?;
                        let header = String::from_utf8_lossy(fetch.header().unwrap_or_default()).to_string();
                        let subject = header.lines().find_map(|l| l.strip_prefix("Subject:")).unwrap_or("").trim().to_string();
                        found.push((subject, fetch.flags().map(|f| format!("{f:?}")).collect()));
                    }
                }
                let _ = session.logout().await;
                let _ = what;
                Ok(found)
            })
            .unwrap()
        };
        // A first fetch of an empty inbox: the next ones bring arrivals.
        crate::fetch::block_on(async {
            let mut session = imap::open(&server, "x").await?;
            let _ = imap::within(COMMAND, session.create("Banque")).await?;
            let listed = mailbox::list(&mut session).await?;
            mailbox::save(&account.id, &listed)?;
            let _ = session.logout().await;
            Ok(())
        })
        .unwrap();
        assert!(crate::fetch::inbox(&account, "x").unwrap().first);
        let arrivals = [
            message("Banque Exemple <alertes@bank.example>", "Votre relevé", "", "Votre relevé d’octobre est prêt."),
            message("Type & Pixels <news@shop.example>", "La lettre", "List-Id: <lettre.shop.example>\r\n", "Les nouvelles."),
            message("Paul <paul@example.org>", "Déjeuner", "", "Midi ?"),
            message("Banque Exemple <no-reply@bank.example>", "Your sign-in code", "", "Your Bank Example code: 482913. It expires in 10 minutes."),
            message("Boutique Exemple <commandes@shop.example>", "Votre facture", "", "Votre facture est jointe."),
        ];
        crate::fetch::block_on(async {
            let mut session = imap::open(&server, "x").await?;
            for raw in &arrivals {
                imap::within(COMMAND, session.append("INBOX", None, None, raw)).await?.map_err(imap::server)?;
            }
            // Another device marked the newsletter first.
            imap::within(COMMAND, session.select("INBOX")).await?.map_err(imap::server)?;
            mailbox::store(&mut session, "2", &format!("+FLAGS.SILENT ({})", rules::FILTERED)).await?;
            let _ = session.logout().await;
            Ok(())
        })
        .unwrap();
        let report = crate::fetch::inbox(&account, "x").unwrap();
        assert!(!report.first && report.new.len() == 5, "{report:?}");
        let filter = |field, value: &str, actions: Vec<Act>| Filter { conditions: vec![Condition::new(field, Test::Contains, value)], actions, ..Filter::default() };
        let mut config = Config::default();
        config.accounts = vec![account.clone()];
        config.mail.filters = vec![
            filter(Field::From, "@bank.example", vec![Act::Move { folder: "banque".into() }, Act::Read]),
            Filter { conditions: vec![Condition::new(Field::List, Test::Exists, "")], actions: vec![Act::Flag], ..Filter::default() },
            filter(Field::Subject, "déjeuner", vec![Act::Move { folder: "Nowhere".into() }]),
            // No archive on this server: one is made, as the Archive button makes it.
            filter(Field::Subject, "facture", vec![Act::Archive]),
        ];
        let done = after_fetch(&config, &account, "x", &report.new, false);
        let subjects = |files: &[PathBuf]| files.iter().filter_map(|f| maildir::read_one(f)).map(|c| c.subject).collect::<Vec<_>>();
        let mut by: Vec<Vec<usize>> = done.acted.iter().map(|j| j.plan.by.clone()).collect();
        by.sort();
        assert_eq!(by, vec![vec![0], vec![3]], "{done:?}");
        let held: Vec<PathBuf> = done.elsewhere.iter().map(|j| j.file.clone()).collect();
        assert_eq!(subjects(&held), vec!["La lettre".to_string()], "marked by another device: its to do, not told here");
        assert!(done.left.is_empty(), "{done:?}");
        assert_eq!(done.failed.len(), 1, "{done:?}");
        assert_eq!(done.failed[0].1, Failure::NoFolder("Nowhere".into()));
        // Told as new mail: what a filter could not act on, Paul's; not what was done, nor what another device holds.
        assert_eq!(subjects(&done.to_tell()), vec!["Déjeuner".to_string()]);
        assert!(!done.unmarked, "GreenMail keeps keywords");
        // On the server: the statement in Banque, read and marked; the code, the newsletter, Paul's in the inbox, Paul's unmarked again.
        let inbox = on_server("inbox", "INBOX");
        let banque = on_server("banque", "Banque");
        assert_eq!(banque.len(), 1, "{banque:?}");
        assert_eq!(on_server("archive", "Archive").len(), 1, "the invoice archived");
        assert!(banque[0].0.contains("relev") && banque[0].1.iter().any(|f| f.contains("Seen")) && banque[0].1.iter().any(|f| f.contains(rules::FILTERED)), "{banque:?}");
        assert_eq!(inbox.len(), 3, "{inbox:?}");
        let paul = inbox.iter().find(|(s, _)| s.contains("jeuner")).unwrap();
        assert!(!paul.1.iter().any(|f| f.contains(rules::FILTERED)), "a failed act unmarks: {paul:?}");
        let code = inbox.iter().find(|(s, _)| s.contains("code")).unwrap();
        assert!(code.1.is_empty() || !code.1.iter().any(|f| f.contains("Seen")), "a code is never filtered: {code:?}");
        // The copy here followed: gone from the inbox.
        assert_eq!(inbox_files(&account).len(), 3);
        // A second device with the same arrivals: the statement and the invoice are gone; nothing to do.
        let again = run(&account, "x", &done.acted, true).unwrap();
        assert!(again.acted.is_empty() && again.left.len() == 2, "{again:?}");
        // The folder made: the next fetch tries Paul's message again, from the waiting list.
        crate::fetch::block_on(async {
            let mut session = imap::open(&server, "x").await?;
            let _ = imap::within(COMMAND, session.create("Nowhere")).await?;
            let listed = mailbox::list(&mut session).await?;
            mailbox::save(&account.id, &listed)?;
            let _ = session.logout().await;
            Ok(())
        })
        .unwrap();
        // Paul's is tried again ten minutes on; the newsletter is looked at again at each fetch.
        assert!(due(&account), "the newsletter to look at again: the next fetch runs the filters, new mail or not");
        let path = Waiting::path();
        let mut aged = Waiting::load(&path);
        for wait in aged.messages.iter_mut().filter(|w| !w.elsewhere) {
            wait.since -= RETRY;
        }
        std::fs::write(&path, toml::to_string(&aged).unwrap()).unwrap();
        let retried = after_fetch(&config, &account, "x", &[], false);
        assert_eq!(retried.acted.len(), 1, "{retried:?}");
        assert_eq!(on_server("nowhere", "Nowhere").len(), 1);
        assert!(retried.to_tell().is_empty() && retried.elsewhere.len() == 1, "the newsletter is still the other device's: {retried:?}");
        let (waits, gone) = Waiting::files_of(&account, jiff::Timestamp::now().as_second());
        assert_eq!((waits.iter().map(|(_, w)| w.elsewhere).collect::<Vec<_>>(), gone.len()), (vec![true], 0), "Paul's done; the newsletter looked at again");
        // That device stopped half-way: its mark stays ten minutes, the message
        // neither read nor gone. Told here, and waiting no more.
        let mut aged = Waiting::load(&path);
        for wait in &mut aged.messages {
            wait.since -= STUCK;
        }
        std::fs::write(&path, toml::to_string(&aged).unwrap()).unwrap();
        let stuck = after_fetch(&config, &account, "x", &[], false);
        assert_eq!(stuck.failed.iter().map(|(_, why)| why.clone()).collect::<Vec<_>>(), vec![Failure::Claimed], "{stuck:?}");
        assert_eq!(subjects(&stuck.to_tell()), vec!["La lettre".to_string()]);
        assert!(Waiting::files_of(&account, i64::MAX / 2).0.is_empty() && !due(&account));
        // The run you ask for: read mail too, unmarked; the newsletter, read on the server, now flagged.
        crate::fetch::block_on(async {
            let mut session = imap::open(&server, "x").await?;
            imap::within(COMMAND, session.select("INBOX")).await?.map_err(imap::server)?;
            mailbox::store(&mut session, "1:*", "+FLAGS.SILENT (\\Seen)").await?;
            let _ = session.logout().await;
            Ok(())
        })
        .unwrap();
        let _ = crate::fetch::inbox(&account, "x").unwrap();
        let all = jobs(&config, &account, &inbox_files(&account));
        assert_eq!(all.len(), 1, "the newsletter alone: the code stays protected while valid: {all:?}");
        let asked = run(&account, "x", &all, false).unwrap();
        assert_eq!(asked.acted.len(), 1, "{asked:?}");
        let inbox = on_server("inbox", "INBOX");
        assert!(inbox.iter().any(|(s, f)| s.contains("lettre") && f.iter().any(|f| f.contains("Flagged"))), "{inbox:?}");
    }
}

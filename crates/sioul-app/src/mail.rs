// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The mail client behind the window: folders and their messages, what you do
//! to them, drafts and sending.
//!
//! Moving, archiving, deleting, junking, sending and discarding a draft wait
//! ten seconds with "Undo" before anything reaches the server or the disk;
//! meanwhile the message or the draft is hidden from the lists
//! (docs/client.md, "Calm first", rules 3 and 4). Reading and flagging happen
//! at once and show at once: undoing them is doing them again.

use crate::backend::{QtThread, Shared, config_path, json, load_config, say, set_status, show, tell, tr};
use cxx_qt_lib::QString;
use jiff::Zoned;
use sioul_core::card::Card;
use sioul_core::compose::{self, Draft, DraftKind};
use sioul_core::config::{Account, Config};
use sioul_core::folders::{Folder, Role};
use sioul_core::{maildir, view};
use sioul_sync::mailbox::{self, Action};
use sioul_sync::{SyncError, secret};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, UNIX_EPOCH};

/// Seconds to change your mind (docs/client.md, rules 3 and 4).
const UNDO_SECONDS: u64 = 10;

/// The folder the mail page shows.
#[derive(Clone, Default)]
pub(crate) struct OpenFolder {
    pub account: String,
    pub folder: String,
    /// Past the last two weeks.
    pub all: bool,
    pub query: String,
}

/// What waits for its ten seconds.
enum Work {
    Act { account: Account, file: PathBuf, action: Action },
    /// Into a folder of another account: copied there, then taken off here.
    Across { from: Account, file: PathBuf, to: Account, folder: String },
    /// Several messages at once, under one "Undo".
    Many(Vec<Work>),
    Send { draft: String },
    Discard { draft: String },
    /// A contact or an event deleted here; the account's sync deletes it there.
    Remove { account: String, file: PathBuf },
    /// One occurrence of a repeating event left out (EXDATE).
    Skip { account: String, file: PathBuf, start: i64 },
    /// The day closed, already in effect: "Undo" puts the time back as it was.
    Rest { previous: sioul_core::quiet::Overrides },
    /// A note in the vault's trash already: "Undo" puts it back.
    Untrash { trashed: String, path: String },
    /// A change in effect already (a meal, an event or a step moved by a
    /// drag): "Undo" runs `back`, which puts back what was there and says
    /// what went wrong, else "".
    Back { back: Box<dyn Fn(&QtThread, &Arc<Shared>) -> String + Send + Sync> },
}

pub(crate) struct Pending {
    work: Work,
    /// "Moved to the trash.", shown with "Undo".
    line: String,
    /// Taken once, by whoever comes first: its timer, "Undo", or the window closing.
    taken: AtomicBool,
}

/// What the mail page shows, computed off Qt's thread.
pub(crate) struct MailViews {
    pub accounts: String,
    pub folder: String,
    pub drafts: String,
}

/// Your addresses: never answered to, never counted among the others.
pub(crate) fn own_addresses(config: &Config) -> Vec<String> {
    config.accounts.iter().filter_map(|a| a.address.clone()).collect()
}

fn moves(action: &Action) -> bool {
    !matches!(action, Action::Read(_) | Action::Flag(_))
}

/// Messages moved and drafts sent or discarded, while "Undo" is offered.
fn hidden(shared: &Shared) -> (BTreeSet<PathBuf>, BTreeSet<String>) {
    let (mut files, mut drafts) = (BTreeSet::new(), BTreeSet::new());
    fn add(work: &Work, files: &mut BTreeSet<PathBuf>, drafts: &mut BTreeSet<String>) {
        match work {
            Work::Act { file, action, .. } if moves(action) => {
                files.insert(file.clone());
            }
            Work::Across { file, .. } | Work::Remove { file, .. } => {
                files.insert(file.clone());
            }
            Work::Send { draft } | Work::Discard { draft } => {
                drafts.insert(draft.clone());
            }
            Work::Many(works) => works.iter().for_each(|w| add(w, files, drafts)),
            Work::Act { .. } | Work::Skip { .. } | Work::Rest { .. } | Work::Untrash { .. } | Work::Back { .. } => {}
        }
    }
    for pending in shared.pending.lock().map(|p| p.clone()).unwrap_or_default() {
        add(&pending.work, &mut files, &mut drafts);
    }
    (files, drafts)
}

/// Contacts and events deleted, and occurrences left out, while "Undo" is offered.
pub(crate) fn hidden_pim(shared: &Shared) -> (BTreeSet<PathBuf>, BTreeSet<(PathBuf, i64)>) {
    let (mut files, mut skipped) = (BTreeSet::new(), BTreeSet::new());
    for pending in shared.pending.lock().map(|p| p.clone()).unwrap_or_default() {
        match &pending.work {
            Work::Remove { file, .. } => {
                files.insert(file.clone());
            }
            Work::Skip { file, start, .. } => {
                skipped.insert((file.clone(), *start));
            }
            _ => {}
        }
    }
    (files, skipped)
}

/// Deletes a contact or an event after ten seconds to undo.
pub(crate) fn schedule_removal(qt: &QtThread, shared: &Arc<Shared>, account: &str, file: PathBuf, line: String) {
    schedule(qt, shared, Work::Remove { account: account.to_string(), file }, line);
}

/// Leaves one occurrence of a repeating event out, after ten seconds to undo.
pub(crate) fn schedule_skip(qt: &QtThread, shared: &Arc<Shared>, account: &str, file: PathBuf, start: i64, line: String) {
    schedule(qt, shared, Work::Skip { account: account.to_string(), file, start }, line);
}

/// The messages the Porch should not show either.
pub(crate) fn hidden_files(shared: &Shared) -> BTreeSet<PathBuf> {
    hidden(shared).0
}

#[derive(serde::Serialize)]
struct DraftRow {
    id: String,
    subject: String,
    to: String,
    date: String,
}

fn draft_row(draft: &Draft) -> DraftRow {
    let saved = std::fs::metadata(draft.path())
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_secs()).ok());
    DraftRow {
        id: draft.id.clone(),
        subject: if draft.subject.trim().is_empty() { tr().text("mail-no-subject", None) } else { draft.subject.clone() },
        to: say("mail-to-whom", &[("names", names(&draft.to))]),
        date: view::date(tr(), saved),
    }
}

/// "Jane, paul@example.org", else "nobody yet".
fn names(entries: &[String]) -> String {
    let names: Vec<String> = entries
        .iter()
        .map(|e| match e.rfind('<') {
            Some(open) if open > 0 => e[..open].trim().trim_matches('"').to_string(),
            _ => e.trim().to_string(),
        })
        .collect();
    if names.is_empty() { tr().text("mail-nobody", None) } else { names.join(", ") }
}

pub(crate) fn views(shared: &Shared) -> MailViews {
    let config = load_config();
    let (files, drafts) = hidden(shared);
    let accounts: Vec<(Account, Vec<Folder>)> = config.accounts.iter().filter(|a| a.syncs()).map(|a| (a.clone(), mailbox::folders(&a.id))).collect();
    let open = shared.open_folder.lock().ok().and_then(|o| o.clone());
    let folder = open.and_then(|open| {
        let (account, folders) = accounts.iter().find(|(a, _)| a.id == open.account)?;
        let folder = folders.iter().find(|f| f.name == open.folder)?;
        // Gmail's "All Mail", shown as the archive: what the inbox and Sent show is left out.
        let shown_elsewhere = if folder.role == Role::All {
            let mut ids = maildir::message_ids(&account.maildir_path());
            for sent in folders.iter().filter(|f| f.role == Role::Sent) {
                ids.extend(maildir::message_ids(&account.maildir_path().join(&sent.local)));
            }
            ids
        } else {
            Default::default()
        };
        let cards: Vec<Card> = maildir::read_messages(&account.maildir_path().join(&folder.local))
            .into_iter()
            .filter(|c| c.path.as_ref().is_none_or(|p| !files.contains(p)))
            .filter(|c| c.message_id.as_ref().is_none_or(|id| !shown_elsewhere.contains(id)))
            .collect();
        let trusted = config.mail_sources().into_iter().find(|s| s.account.as_deref() == Some(account.id.as_str())).map(|s| s.trusted_ids).unwrap_or_default();
        let now = Zoned::now().timestamp().as_second();
        // By conversation, when chosen: your answers come from Sent.
        let sent = (config.mail.threads && !matches!(folder.role, Role::Sent | Role::Drafts)).then(|| {
            folders.iter().filter(|f| f.role == Role::Sent).flat_map(|f| maildir::read_messages(&account.maildir_path().join(&f.local))).collect::<Vec<Card>>()
        });
        Some(json(&view::folder(account, folder, cards, &trusted, open.all, &open.query, tr(), now, sent)))
    });
    let rows: Vec<DraftRow> = Draft::all().iter().filter(|d| !drafts.contains(&d.id)).map(draft_row).collect();
    let mode = crate::backend::mode_now();
    let hours = (mode.time != sioul_core::areas::Time::Any).then_some((mode.time, mode.week));
    MailViews { accounts: json(&view::mail_accounts(&accounts, tr(), hours)), folder: folder.unwrap_or_default(), drafts: json(&rows) }
}

/// Computes the mail page on a thread and shows it, unless a newer one came first.
pub(crate) fn show_mail(qt: &QtThread, shared: &Arc<Shared>) {
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.mail_job, move |shared| {
        let generation = shared.mail_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let views = views(shared);
        let _ = qt.queue(move |mut sioul| {
            // Only the newest (see `backend::show`).
            if sioul.shared().mail_generation.load(Ordering::Relaxed) != generation || sioul.shared().mail_shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
                return;
            }
            sioul.as_mut().set_mail_accounts(QString::from(&views.accounts));
            sioul.as_mut().set_mail_folder(QString::from(&views.folder));
            sioul.as_mut().set_drafts(QString::from(&views.drafts));
        });
    });
}

/// The account a message belongs to, and where its file is now.
pub(crate) fn locate(key: &str) -> Option<(Account, PathBuf)> {
    let file = maildir::locate(Path::new(key))?;
    let account = load_config().account_of(&file)?.clone();
    Some((account, file))
}

/// "read", "unread", "flag", "unflag", "archive", "trash", "junk", "not-junk", "move".
pub(crate) fn act(qt: &QtThread, shared: &Arc<Shared>, key: &str, action: &str, target: &str) {
    let Some((account, file)) = locate(key) else {
        set_status(qt, tr().text("mail-message-gone", None));
        return;
    };
    let folder = mailbox::folder_of(&account, &file);
    let Some(action) = action_of(action, target) else { return };
    let line = match &action {
        Action::Read(_) | Action::Flag(_) => return act_now(qt, shared, account, file, action),
        Action::Archive => tr().text("undo-archived", None),
        Action::Trash if folder.as_ref().is_some_and(|f| f.role == Role::Trash) => tr().text("undo-deleted", None),
        Action::Trash => tr().text("undo-trashed", None),
        Action::Junk => tr().text("undo-junked", None),
        Action::NotJunk => tr().text("undo-not-junk", None),
        Action::Move(name) => {
            let title = mailbox::folders(&account.id).into_iter().find(|f| f.name == *name).map_or_else(|| name.clone(), |f| folder_title(&f));
            say("undo-moved", &[("folder", title)])
        }
    };
    schedule(qt, shared, Work::Act { account, file, action }, line);
}

fn folder_title(folder: &Folder) -> String {
    if folder.role == Role::Other { folder.display.clone() } else { tr().text(folder.role.message_id(), None) }
}

/// What a key names, as an act on its message; None for what is not a message here.
fn action_of(action: &str, target: &str) -> Option<Action> {
    Some(match action {
        "read" => Action::Read(true),
        "unread" => Action::Read(false),
        "flag" => Action::Flag(true),
        "unflag" => Action::Flag(false),
        "archive" => Action::Archive,
        "trash" => Action::Trash,
        "junk" => Action::Junk,
        "not-junk" => Action::NotJunk,
        "move" if !target.is_empty() => Action::Move(target.to_string()),
        _ => return None,
    })
}

/// The same act on several messages (a selection): reading and flagging at
/// once, the rest under one "Undo".
pub(crate) fn act_many(qt: &QtThread, shared: &Arc<Shared>, keys: &[String], action: &str) {
    let Some(first) = action_of(action, "") else { return };
    let found: Vec<(Account, PathBuf)> = keys.iter().filter_map(|k| locate(k)).collect();
    if found.is_empty() {
        set_status(qt, tr().text("mail-message-gone", None));
        return;
    }
    if !moves(&first) {
        for (account, file) in found {
            act_now(qt, shared, account, file, first.clone());
        }
        return;
    }
    let in_trash = found.iter().all(|(account, file)| mailbox::folder_of(account, file).is_some_and(|f| f.role == Role::Trash));
    let id = match &first {
        Action::Archive => "undo-many-archived",
        Action::Junk => "undo-many-junked",
        Action::NotJunk => "undo-many-not-junk",
        _ if in_trash => "undo-many-deleted",
        _ => "undo-many-trashed",
    };
    let line = say(id, &[("n", found.len().to_string())]);
    let works = found.into_iter().map(|(account, file)| Work::Act { account, file, action: first.clone() }).collect();
    schedule(qt, shared, Work::Many(works), line);
}

/// Messages dropped on a folder, of their account or of another one.
pub(crate) fn move_many(qt: &QtThread, shared: &Arc<Shared>, keys: &[String], account: &str, folder: &str) {
    let config = load_config();
    let Some(to) = config.account(account).filter(|a| a.syncs()).cloned() else {
        set_status(qt, say("account-unknown", &[("id", account.to_string())]));
        return;
    };
    let works: Vec<Work> = keys
        .iter()
        .filter_map(|k| locate(k))
        // Already there: nothing to do.
        .filter(|(from, file)| from.id != to.id || mailbox::folder_of(from, file).is_none_or(|f| f.name != folder))
        .map(|(from, file)| {
            if from.id == to.id {
                Work::Act { account: from, file, action: Action::Move(folder.to_string()) }
            } else {
                Work::Across { from, file, to: to.clone(), folder: folder.to_string() }
            }
        })
        .collect();
    if works.is_empty() {
        return;
    }
    let title = mailbox::folders(&to.id).into_iter().find(|f| f.name == folder).map_or_else(|| folder.to_string(), |f| folder_title(&f));
    let n = works.len().to_string();
    let line = if works.iter().any(|w| matches!(w, Work::Across { .. })) {
        say("undo-moved-across", &[("n", n), ("folder", title), ("account", to.address.clone().unwrap_or_else(|| to.id.clone()))])
    } else if works.len() > 1 {
        say("undo-many-moved", &[("n", n), ("folder", title)])
    } else {
        say("undo-moved", &[("folder", title)])
    };
    schedule(qt, shared, Work::Many(works), line);
}

/// Reading and flagging: shown at once, then told to the server; put back if it refuses.
fn act_now(qt: &QtThread, shared: &Arc<Shared>, account: Account, file: PathBuf, action: Action) {
    let before = maildir::flags_of(&file);
    let after = match &action {
        Action::Read(true) => format!("{before}S"),
        Action::Read(false) => before.replace('S', ""),
        Action::Flag(true) => format!("{before}F"),
        Action::Flag(false) => before.replace('F', ""),
        _ => return,
    };
    let file = maildir::set_flags(&file, &after).unwrap_or(file);
    show_mail(qt, shared);
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        if let Err(e) = secret::password(&account).and_then(|p| mailbox::act(&account, &p, &file, &action)) {
            if let Some(now) = maildir::locate(&file) {
                let _ = maildir::set_flags(&now, &before);
            }
            tell(&qt, &shared, e.sentence(tr(), &account.id));
            show_mail(&qt, &shared);
        }
    });
}

/// Opening a message marks it read, except while the window takes its own pictures.
pub(crate) fn opened(qt: &QtThread, shared: &Arc<Shared>, key: &str) {
    if std::env::var_os("SIOUL_GRAB").is_some() {
        return;
    }
    if let Some((account, file)) = locate(key)
        && !maildir::flags_of(&file).contains('S')
    {
        act_now(qt, shared, account, file, Action::Read(true));
    }
}

fn schedule(qt: &QtThread, shared: &Arc<Shared>, work: Work, line: String) {
    let pending = Arc::new(Pending { work, line: line.clone(), taken: AtomicBool::new(false) });
    if let Ok(mut list) = shared.pending.lock() {
        list.push(Arc::clone(&pending));
    }
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_undo_line(QString::from(&line)));
    show(qt, shared);
    // An event deleted or left out: gone from the day and the agenda at once.
    let events = matches!(pending.work, Work::Remove { .. } | Work::Skip { .. });
    if events {
        crate::work::show_work(qt, shared);
        crate::pim::show_pim(qt, shared);
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(UNDO_SECONDS));
        finish(&qt, &shared, &pending);
    });
}

/// What waited, done now if nobody did it yet: its timer ran out, or the app is put away.
fn finish(qt: &QtThread, shared: &Arc<Shared>, pending: &Arc<Pending>) {
    if pending.taken.swap(true, Ordering::Relaxed) {
        return;
    }
    // Done: the line says it, without "Undo" now.
    let line = perform(&pending.work, shared).or_else(|| Some(pending.line.clone()));
    forget(qt, shared, pending);
    if let Some(line) = line {
        tell(qt, shared, line);
    }
    show(qt, shared);
    if matches!(pending.work, Work::Remove { .. } | Work::Skip { .. }) {
        crate::work::show_work(qt, shared);
    }
}

/// Put away on a phone, Sioul may be stopped any moment: what waits for its
/// ten seconds is done now rather than lost (an event deleted would come back).
pub(crate) fn finish_all(qt: &QtThread, shared: &Arc<Shared>) {
    for pending in shared.pending.lock().map(|p| p.clone()).unwrap_or_default() {
        finish(qt, shared, &pending);
    }
}

/// Every watcher: mail, calendars and contacts, GitHub.
fn every_watcher(shared: &Shared) -> Vec<Arc<sioul_sync::Control>> {
    let mut all: Vec<Arc<sioul_sync::Control>> = shared.watchers.lock().map(|w| w.values().cloned().collect()).unwrap_or_default();
    all.extend(shared.dav_watchers.lock().map(|w| w.values().cloned().collect::<Vec<_>>()).unwrap_or_default());
    all.extend(shared.github.lock().ok().and_then(|g| g.clone()));
    all
}

/// Put away on a phone: the watchers keep only what notices new mail (the
/// inbox's IDLE); the other folders, calendars and GitHub wait for its return.
pub(crate) fn quiet_watchers(shared: &Shared) {
    for control in every_watcher(shared) {
        control.set_quiet(true);
    }
}

/// Back on a phone's screen, where pauses may have slept through hours: every
/// watcher fetches now (leaving quiet wakes it).
pub(crate) fn wake_watchers(shared: &Shared) {
    for control in every_watcher(shared) {
        control.set_quiet(false);
        control.nudge();
    }
}

/// A note in the trash: "Undo" stays offered for a moment, as for mail.
pub(crate) fn offer_untrash(qt: &QtThread, shared: &Arc<Shared>, trashed: String, path: String, line: String) {
    schedule(qt, shared, Work::Untrash { trashed, path }, line);
}

/// The day closed: "Undo" stays offered for a moment, as for mail.
pub(crate) fn rest(qt: &QtThread, shared: &Arc<Shared>, previous: sioul_core::quiet::Overrides, line: String) {
    schedule(qt, shared, Work::Rest { previous }, line);
}

/// A change made already (a drag): "Undo" stays offered for a moment, as for
/// mail, and `back` puts back what was there.
pub(crate) fn offer_back(qt: &QtThread, shared: &Arc<Shared>, line: String, back: impl Fn(&QtThread, &Arc<Shared>) -> String + Send + Sync + 'static) {
    schedule(qt, shared, Work::Back { back: Box::new(back) }, line);
}

/// Takes a pending act off the list; the one before it, if any, can still be undone.
fn forget(qt: &QtThread, shared: &Shared, pending: &Arc<Pending>) {
    let next = shared
        .pending
        .lock()
        .map(|mut list| {
            list.retain(|p| !Arc::ptr_eq(p, pending));
            list.iter().rev().find(|p| !p.taken.load(Ordering::Relaxed)).map(|p| p.line.clone()).unwrap_or_default()
        })
        .unwrap_or_default();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_undo_line(QString::from(&next)));
}

/// Does what waited; returns what the status line says about it, if anything.
fn perform(work: &Work, shared: &Shared) -> Option<String> {
    match work {
        Work::Act { account, file, action } => match secret::password(account).and_then(|p| mailbox::act(account, &p, file, action)) {
            Ok(()) => {
                nudge(shared, &account.id);
                None
            }
            Err(e) => Some(e.sentence(tr(), &account.id)),
        },
        Work::Across { from, file, to, folder } => {
            let moved = secret::password(from).and_then(|p| secret::password(to).and_then(|q| mailbox::move_across(from, &p, file, to, &q, folder)));
            match moved {
                Ok(()) => {
                    nudge(shared, &from.id);
                    nudge(shared, &to.id);
                    None
                }
                Err(e) => Some(e.sentence(tr(), &from.id)),
            }
        }
        // Each in turn; the first problem is said, the others are still done.
        Work::Many(works) => works.iter().filter_map(|w| perform(w, shared)).next(),
        Work::Send { draft } => Some(send_now(draft, shared)),
        Work::Discard { draft } => {
            if let Some(draft) = Draft::by_id(draft) {
                draft.discard();
            }
            None
        }
        Work::Remove { account, file } => match std::fs::remove_file(file) {
            Ok(()) => {
                crate::pim::nudge(shared, account);
                None
            }
            Err(e) => Some(format!("{}: {e}", file.display())),
        },
        Work::Rest { .. } | Work::Untrash { .. } | Work::Back { .. } => None,
        Work::Skip { account, file, start } => {
            let skipped = std::fs::read_to_string(file).ok().and_then(|text| sioul_core::agenda::skip_occurrence(&text, *start));
            match skipped.map(|text| sioul_core::vdir::write_item(file, &text)) {
                Some(Ok(())) => {
                    crate::pim::nudge(shared, account);
                    None
                }
                Some(Err(e)) => Some(e),
                None => Some(tr().text("agenda-gone", None)),
            }
        }
    }
}

/// The account's watcher fetches now: a moved message shows in its new folder,
/// a sent one in Sent.
fn nudge(shared: &Shared, account: &str) {
    if let Some(control) = shared.watchers.lock().ok().and_then(|w| w.get(account).cloned()) {
        control.nudge();
    }
}

fn send_now(id: &str, shared: &Shared) -> String {
    let Some(draft) = Draft::by_id(id) else { return tr().text("mail-draft-gone", None) };
    let config = load_config();
    let Some(account) = config.account(&draft.account).filter(|a| a.syncs()).cloned() else {
        return say("account-unknown", &[("id", draft.account.clone())]);
    };
    let sent = secret::password(&account).and_then(|p| sioul_sync::send::send_draft(&config_path(), &account, &p, &draft, tr()));
    match sent {
        Ok(()) => {
            draft.discard();
            nudge(shared, &account.id);
            tr().text("mail-sent", None)
        }
        Err(e @ SyncError::NotFiled(_)) => {
            draft.discard();
            e.sentence(tr(), &account.id)
        }
        Err(e) => say("mail-not-sent", &[("detail", e.sentence(tr(), &account.id))]),
    }
}

/// Cancels the newest act still waiting; a send or a discard gives back its
/// draft, whose window opens again.
pub(crate) fn undo(qt: &QtThread, shared: &Arc<Shared>) -> Option<String> {
    let pending = shared.pending.lock().ok()?.iter().rev().find(|p| !p.taken.load(Ordering::Relaxed)).cloned()?;
    if pending.taken.swap(true, Ordering::Relaxed) {
        set_status(qt, tr().text("undo-too-late", None));
        return None;
    }
    forget(qt, shared, &pending);
    show(qt, shared);
    let (line, draft) = match &pending.work {
        Work::Untrash { trashed, path } => {
            let root = load_config().case_store_path();
            let back = root.ok_or_else(|| tr().text("error-no-store", None)).and_then(|root| sioul_core::notes::untrash(&root, trashed, path));
            crate::work::show_work(qt, shared);
            (back.err().unwrap_or_else(|| tr().text("undo-done", None)), None)
        }
        Work::Rest { previous } => {
            let _ = previous.save(&sioul_core::quiet::Overrides::default_path());
            let _ = qt.queue(|mut sioul| sioul.as_mut().set_mode(QString::from(&crate::backend::mode_json())));
            show(qt, shared);
            crate::work::show_work(qt, shared);
            (tr().text("undo-done", None), None)
        }
        Work::Back { back } => {
            let problem = back(qt, shared);
            (if problem.is_empty() { tr().text("undo-done", None) } else { problem }, None)
        }
        Work::Send { draft } => (tr().text("undo-send-undone", None), Some(draft.clone())),
        Work::Discard { draft } => (tr().text("undo-done", None), Some(draft.clone())),
        // A task or an event deleted, an occurrence left out: back in the plan and the agenda at once.
        Work::Remove { .. } | Work::Skip { .. } => {
            if let Ok(mut cache) = shared.loaded.lock() {
                *cache = None;
            }
            crate::work::show_work(qt, shared);
            crate::pim::show_pim(qt, shared);
            (tr().text("undo-done", None), None)
        }
        Work::Act { .. } | Work::Across { .. } | Work::Many(_) => (tr().text("undo-done", None), None),
    };
    tell(qt, shared, line);
    draft
}

/// The window closes: what waits is done now rather than lost.
pub(crate) fn flush(shared: &Arc<Shared>) {
    let waiting: Vec<Arc<Pending>> = shared.pending.lock().map(|p| p.clone()).unwrap_or_default();
    for pending in waiting {
        if !pending.taken.swap(true, Ordering::Relaxed) {
            let _ = perform(&pending.work, shared);
        }
    }
}

/// The account a new message goes out from: the one asked, else the first by priority.
fn sender<'a>(config: &'a Config, asked: &str) -> Option<&'a Account> {
    config.account(asked).filter(|a| a.syncs()).or_else(|| config.accounts.iter().filter(|a| a.syncs()).min_by_key(|a| a.priority))
}

/// A new draft: "new", "reply", "reply-all", "forward" (the last three from the message `key`).
pub(crate) fn compose(kind: &str, key: &str, account: &str) -> Result<String, String> {
    let config = load_config();
    let mut draft = match kind {
        "new" => {
            let account = sender(&config, account).ok_or_else(|| tr().text("mail-no-account", None))?;
            Draft::new(&account.id)
        }
        _ => {
            let kind = match kind {
                "reply" => DraftKind::Reply,
                "reply-all" => DraftKind::ReplyAll,
                "forward" => DraftKind::Forward,
                _ => return Err(String::new()),
            };
            let file = maildir::locate(Path::new(key)).ok_or_else(|| tr().text("mail-message-gone", None))?;
            let account = config.account_of(&file).or_else(|| sender(&config, account)).ok_or_else(|| tr().text("mail-no-account", None))?;
            let mut draft = compose::answer(&file, kind, &account.id, &own_addresses(&config)).ok_or_else(|| tr().text("mail-message-gone", None))?;
            draft.keep_original()?;
            crate::crypto::protect_answer(&mut draft);
            draft
        }
    };
    draft.add_signature(config.account(&draft.account).and_then(|a| a.signature.as_deref()));
    draft.save()?;
    Ok(draft.id)
}

#[derive(serde::Serialize)]
struct SenderView {
    id: String,
    label: String,
}

#[derive(serde::Serialize)]
struct AttachedView {
    index: usize,
    name: String,
    size: String,
}

#[derive(serde::Serialize)]
struct DraftView {
    id: String,
    account: String,
    accounts: Vec<SenderView>,
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    attachments: Vec<AttachedView>,
    /// A forward's attachments, by their index in the original.
    forwarded: Vec<AttachedView>,
    /// "Reply to Jane", "New message"…
    title: String,
    /// What goes below your text when it is sent: "Jane's message of …, quoted."
    below: String,
    sign: bool,
    encrypt: bool,
    protection: crate::crypto::DraftProtection,
}

/// A draft as the writing window shows it.
pub(crate) fn draft(id: &str) -> Option<String> {
    let draft = Draft::by_id(id)?;
    let config = load_config();
    let size = |bytes: u64| view::size(tr(), usize::try_from(bytes).unwrap_or(usize::MAX));
    let title = match draft.kind {
        DraftKind::New => tr().text("compose-title-new", None),
        DraftKind::Reply | DraftKind::ReplyAll => say("compose-title-reply", &[("names", names(&draft.to))]),
        DraftKind::Forward => tr().text("compose-title-forward", None),
    };
    let original = draft.original.as_deref().and_then(maildir::read_one);
    let below = match (draft.kind, original) {
        (DraftKind::New, _) | (_, None) => String::new(),
        (DraftKind::Forward, Some(card)) => say("compose-below-forward", &[("name", card.sender().to_string()), ("date", view::date(tr(), card.date))]),
        (_, Some(card)) => say("compose-below-quote", &[("name", card.sender().to_string()), ("date", view::date(tr(), card.date))]),
    };
    Some(json(&DraftView {
        id: draft.id.clone(),
        account: draft.account.clone(),
        accounts: config
            .accounts
            .iter()
            .filter(|a| a.syncs())
            .map(|a| SenderView { id: a.id.clone(), label: a.address.clone().unwrap_or_else(|| a.id.clone()) })
            .collect(),
        to: draft.to.join(", "),
        cc: draft.cc.join(", "),
        bcc: draft.bcc.join(", "),
        subject: draft.subject.clone(),
        body: draft.body.clone(),
        attachments: draft
            .attachments
            .iter()
            .enumerate()
            .map(|(index, path)| AttachedView {
                index,
                name: path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().to_string()),
                size: std::fs::metadata(path).map(|m| size(m.len())).unwrap_or_default(),
            })
            .collect(),
        forwarded: draft.forwarded().into_iter().map(|(index, name)| AttachedView { index: index as usize, name, size: String::new() }).collect(),
        title,
        below,
        sign: draft.sign,
        encrypt: draft.encrypt,
        protection: crate::crypto::draft_protection(&draft),
    }))
}

#[derive(serde::Deserialize)]
struct DraftEdit {
    id: String,
    account: String,
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    #[serde(default)]
    sign: bool,
    #[serde(default)]
    encrypt: bool,
}

/// Saves what the writing window holds; returns "Saved at 14:02".
pub(crate) fn save_draft(text: &str) -> Result<String, String> {
    let edit: DraftEdit = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut draft = Draft::by_id(&edit.id).ok_or_else(|| tr().text("mail-draft-gone", None))?;
    if load_config().account(&edit.account).is_some_and(Account::syncs) {
        draft.account = edit.account;
    }
    draft.to = compose::split_addresses(&edit.to);
    draft.cc = compose::split_addresses(&edit.cc);
    draft.bcc = compose::split_addresses(&edit.bcc);
    draft.subject = edit.subject;
    draft.body = edit.body;
    draft.sign = edit.sign;
    draft.encrypt = edit.encrypt;
    draft.save()?;
    Ok(say("compose-saved", &[("time", Zoned::now().strftime("%H:%M").to_string())]))
}

/// Adds a file, from the file dialog or dropped on the window (`file://` URLs).
pub(crate) fn attach(id: &str, url: &str) -> Result<(), String> {
    let mut draft = Draft::by_id(id).ok_or_else(|| tr().text("mail-draft-gone", None))?;
    let path = crate::backend::local_path(url);
    if !path.is_file() {
        return Err(say("compose-not-a-file", &[("path", path.display().to_string())]));
    }
    if !draft.attachments.contains(&path) {
        draft.attachments.push(path);
    }
    draft.save().map(|_| ())
}

/// Takes an attachment off: one you added, by position, or one a forward carries, by index.
pub(crate) fn detach(id: &str, index: usize, forwarded: bool) -> Result<(), String> {
    let mut draft = Draft::by_id(id).ok_or_else(|| tr().text("mail-draft-gone", None))?;
    if forwarded {
        draft.dropped.push(u32::try_from(index).unwrap_or(u32::MAX));
    } else if index < draft.attachments.len() {
        draft.attachments.remove(index);
    }
    draft.save().map(|_| ())
}

/// Sends after ten seconds; the window closes, "Undo" opens it again.
/// Returns what stops it now (no recipient), else nothing.
pub(crate) fn send(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> Option<String> {
    let Some(draft) = Draft::by_id(id) else { return Some(tr().text("mail-draft-gone", None)) };
    if draft.recipients().is_empty() {
        return Some(tr().text("compose-no-recipient", None));
    }
    let unknown: Vec<&String> = draft.to.iter().chain(&draft.cc).chain(&draft.bcc).filter(|e| compose::address_of(e).is_none()).collect();
    if let Some(entry) = unknown.first() {
        return Some(say("compose-bad-address", &[("entry", entry.to_string())]));
    }
    schedule(qt, shared, Work::Send { draft: id.to_string() }, tr().text("undo-sending", None));
    None
}

/// Deletes a draft after ten seconds; "Undo" opens it again.
pub(crate) fn discard(qt: &QtThread, shared: &Arc<Shared>, id: &str) {
    schedule(qt, shared, Work::Discard { draft: id.to_string() }, tr().text("undo-discarded", None));
}

/// The writing window closed: an untouched draft goes, a written one stays in Drafts.
pub(crate) fn closed(qt: &QtThread, shared: &Arc<Shared>, id: &str) {
    let Some(draft) = Draft::by_id(id) else { return };
    let signature = load_config().account(&draft.account).and_then(|a| a.signature.clone());
    let pending = hidden(shared).1.contains(id);
    if draft.is_empty(signature.as_deref()) {
        draft.discard();
    } else if !pending {
        set_status(qt, tr().text("compose-kept", None));
    }
    crate::mail::show_mail(qt, shared);
}

#[derive(serde::Serialize)]
struct Place {
    account: String,
    folder: String,
}

/// Where a message is: its account and its folder's server name.
pub(crate) fn place(key: &str) -> String {
    let Some((account, file)) = locate(key) else { return "{}".into() };
    let folder = mailbox::folder_of(&account, &file).map(|f| f.name).unwrap_or_default();
    json(&Place { account: account.id, folder })
}

/// The message as it came, headers and all, for "Show the source"; cut after 512 KB.
pub(crate) fn source(key: &str) -> String {
    let Some((_, file)) = locate(key) else { return String::new() };
    let raw = std::fs::read(&file).unwrap_or_default();
    let cut = raw.len().min(512 * 1024);
    String::from_utf8_lossy(&raw[..cut]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_files_keep_their_names() {
        use crate::backend::local_path;
        assert_eq!(local_path("file:///home/me/Re%C3%A7u%20final.pdf"), PathBuf::from("/home/me/Reçu final.pdf"));
        assert_eq!(local_path("file:///tmp/100%"), PathBuf::from("/tmp/100%"));
    }
}

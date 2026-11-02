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
use sioul_core::card::Card;
use sioul_core::compose::{self, Draft, DraftKind};
use sioul_core::config::{Account, Config};
use sioul_core::folders::{Folder, Role};
use sioul_core::mailsearch::ServerRef;
use sioul_core::{maildir, unsubscribe, view};
use sioul_sync::mailbox::{self, Action};
use sioul_sync::{SyncError, secret};
use std::collections::{BTreeMap, BTreeSet};
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
    /// Several messages of one account, kept here or seen on its server only
    /// (a search's), in one connection: moves, archiving, the trash.
    Batch { account: Account, files: Vec<PathBuf>, remote: Vec<ServerRef>, action: Action },
    /// A message seen on its server only, into a folder of another account.
    AcrossRemote { from: Account, place: ServerRef, to: Account, folder: String },
    /// Several messages at once, under one "Undo".
    Many(Vec<Work>),
    Send { draft: String },
    /// A message built at Send, signed with your security key: sent as built
    /// once its ten seconds are gone; "Undo" throws the signed copy away.
    SendSigned { draft: String, account: Account, outgoing: compose::Outgoing },
    Discard { draft: String },
    /// A contact or an event deleted here; the account's sync deletes it there.
    /// An event through the one rule when its ten seconds end (`vdir::remove_item`):
    /// one become a meeting with others meanwhile is not taken out.
    Remove { account: String, file: PathBuf },
    /// An invitation's event its organizer cancelled, taken out as your answer
    /// to it (`pim::answer`, the one path that writes a meeting with others):
    /// from a calendar that can be written only.
    RemoveCancelled { account: String, file: PathBuf, cancel: String },
    /// One occurrence of a repeating event left out (EXDATE).
    Skip { account: String, file: PathBuf, start: i64 },
    /// The day closed, already in effect: "Undo" puts the time back as it was.
    /// `previous` as before the act, `after` as the act left it: only what it changed is put back.
    Rest { previous: sioul_core::quiet::Overrides, after: sioul_core::quiet::Overrides },
    /// A note in the vault's trash already: "Undo" puts it back.
    Untrash { trashed: String, path: String },
    /// A change in effect already (a meal, an event or a step moved by a
    /// drag): "Undo" runs `back`, which puts back what was there and says
    /// what went wrong, else "".
    Back { back: Box<dyn Fn(&QtThread, &Arc<Shared>) -> String + Send + Sync> },
    /// Leaving a list, in one click or by a message from `account`; the message stays where it is.
    Unsubscribe { list: unsubscribe::List, sender: String, account: String, way: unsubscribe::Way },
    /// Your mail filters run on the inboxes, as you asked after their preview:
    /// each account's messages in one connection (`filters::perform`).
    Filters { planned: Vec<(Account, Vec<sioul_sync::filters::Job>)> },
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
    /// The mailbox, when Mail shows it; none otherwise (left as it was).
    pub mailbox: Option<String>,
    pub drafts: String,
}

/// The mailbox's chip and address as chosen on this device (the agenda's
/// view is kept so): `$XDG_STATE_HOME/sioul/mailbox-view.toml`.
#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq)]
struct MailboxView {
    #[serde(default)]
    chip: sioul_core::mailbox::Chip,
    #[serde(default)]
    account: String,
}

fn mailbox_view_path() -> PathBuf {
    sioul_core::config::state_dir().join("mailbox-view.toml")
}

/// The chip and address last chosen, as JSON {chip, account}.
pub(crate) fn mailbox_view() -> String {
    mailbox_view_in(&mailbox_view_path())
}

fn mailbox_view_in(path: &Path) -> String {
    let kept: MailboxView = std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    json(&kept)
}

/// The chip and address asked, kept for the next time (written when they change).
pub(crate) fn keep_mailbox_view(ask: &sioul_core::mailbox::Ask) {
    keep_mailbox_view_in(&mailbox_view_path(), ask);
}

fn keep_mailbox_view_in(path: &Path, ask: &sioul_core::mailbox::Ask) {
    let view = MailboxView { chip: ask.chip, account: ask.account.clone() };
    let kept: MailboxView = std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    if kept != view
        && let Ok(text) = toml::to_string(&view)
    {
        // Its folder made first: on a new device nothing else may have made it yet.
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = sioul_core::filelock::replace(path, text.as_bytes());
    }
}

/// The mailbox as `ask` asks it (`mailbox::page`): the addresses that sync,
/// ranked as Mail's column ranks them, their colours, who rests now and
/// until when, the world read once.
fn mailbox_page(config: &Config, ask: &sioul_core::mailbox::Ask, hidden: &BTreeSet<PathBuf>) -> String {
    use sioul_core::areas::{Area, in_view};
    let now = sioul_core::zone::now();
    let mode = crate::hours::mode_at(&now);
    let hours = (mode.time != sioul_core::areas::Time::Any).then_some((mode.time, mode.week));
    let mut ranked: Vec<&Account> = config.accounts.iter().filter(|a| a.syncs()).collect();
    ranked.sort_by_key(|a| a.priority);
    let tints = sioul_core::mailbox::tints(&ranked);
    let sources = config.mail_sources();
    let until = |at: Option<&jiff::Zoned>| at.map(|z| sioul_core::quiet::until_text(tr(), z, &now)).unwrap_or_default();
    let addresses: Vec<sioul_core::mailbox::Address> = ranked
        .iter()
        .zip(tints)
        .map(|(account, tint)| {
            let area = account.area.as_deref().and_then(Area::parse).unwrap_or(Area::WORK);
            let rests = hours.is_some_and(|(time, week)| time == sioul_core::areas::Time::Sleep || !in_view(area, time, week));
            let resting = rests.then(|| {
                if mode.sleeps() {
                    say("mailbox-rests-sleep", &[("until", until(mode.until.as_ref()))])
                } else {
                    let back = if area == Area::WORK { mode.back.as_ref().or(mode.until.as_ref()) } else { mode.until.as_ref() };
                    say("mailbox-rests", &[("address", account.address.clone().unwrap_or_else(|| account.id.clone())), ("until", until(back))])
                }
            });
            let trusted = sources.iter().find(|s| s.account.as_deref() == Some(account.id.as_str())).map(|s| s.trusted_ids.clone()).unwrap_or_default();
            sioul_core::mailbox::Address { account: (*account).clone(), folders: mailbox::folders(&account.id), trusted, tint, resting }
        })
        .collect();
    let world = sioul_core::mailbox::World::load(config, crate::hours::attention_at(&mode, &now));
    let page = sioul_core::mailbox::page(&addresses, &world, ask, hidden, tr(), now.timestamp().as_second(), &sioul_core::zone::current());
    json(&page)
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
            Work::Across { file, .. } | Work::Remove { file, .. } | Work::RemoveCancelled { file, .. } => {
                files.insert(file.clone());
            }
            Work::Batch { files: batch, .. } => files.extend(batch.iter().cloned()),
            Work::Filters { planned } => files.extend(planned.iter().flat_map(|(_, jobs)| jobs.iter().filter(|j| j.plan.leaves()).map(|j| j.file.clone()))),
            Work::Send { draft } | Work::Discard { draft } | Work::SendSigned { draft, .. } => {
                drafts.insert(draft.clone());
            }
            Work::Many(works) => works.iter().for_each(|w| add(w, files, drafts)),
            // Leaving its list, a message stays in view where it is.
            Work::Act { .. } | Work::AcrossRemote { .. } | Work::Skip { .. } | Work::Rest { .. } | Work::Untrash { .. } | Work::Back { .. } | Work::Unsubscribe { .. } => {}
        }
    }
    for pending in shared.pending.lock().map(|p| p.clone()).unwrap_or_default() {
        add(&pending.work, &mut files, &mut drafts);
    }
    (files, drafts)
}

/// Contacts and events deleted, and occurrences left out, while "Undo" is offered.
/// A deletion of `file` waiting for its "Undo", as `delete_event` leaves it
/// (tests: what every producer leaves out meanwhile).
#[cfg(test)]
pub(crate) fn wait_to_remove(shared: &Shared, file: PathBuf) {
    if let Ok(mut pending) = shared.pending.lock() {
        pending.push(Arc::new(Pending { work: Work::Remove { account: String::new(), file }, line: String::new(), taken: AtomicBool::new(false) }));
    }
}

/// The deletion of `file` waiting for its "Undo", on its own or among others.
pub(crate) fn pending_removal(shared: &Shared, file: &Path) -> Option<Arc<Pending>> {
    fn removes(work: &Work, file: &Path) -> bool {
        match work {
            Work::Remove { file: f, .. } | Work::RemoveCancelled { file: f, .. } => f == file,
            Work::Many(works) => works.iter().any(|w| removes(w, file)),
            _ => false,
        }
    }
    shared.pending.lock().ok()?.iter().find(|p| !p.taken.load(Ordering::Relaxed) && removes(&p.work, file)).cloned()
}

/// The deletion of `file` waiting for its "Undo" done now, its Undo gone (a
/// letter's task made again: Undo would give two tasks for one scan; review
/// of ab7daa4, later 16): taken at once, done on a thread of its own, as the
/// Undo timer's is, never the caller's, which may be the window's (review of
/// 8b57a35, L17: what it does may wait for a server, or for a fetch's lock).
/// Whether one waited.
pub(crate) fn remove_now(qt: &QtThread, shared: &Arc<Shared>, file: &Path) -> bool {
    let Some(pending) = pending_removal(shared, file) else { return false };
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    take_off_thread(&pending, move |pending| done(&qt, &shared, &pending));
    true
}

/// `pending` taken now (its "Undo" gone at once), then `run` with it on a
/// thread of its own. False when someone took it first (its timer, "Undo").
fn take_off_thread(pending: &Arc<Pending>, run: impl FnOnce(Arc<Pending>) + Send + 'static) -> bool {
    if pending.taken.swap(true, Ordering::Relaxed) {
        return false;
    }
    let pending = Arc::clone(pending);
    std::thread::spawn(move || run(pending));
    true
}

pub(crate) fn hidden_pim(shared: &Shared) -> (BTreeSet<PathBuf>, BTreeSet<(PathBuf, i64)>) {
    fn add(work: &Work, files: &mut BTreeSet<PathBuf>, skipped: &mut BTreeSet<(PathBuf, i64)>) {
        match work {
            Work::Remove { file, .. } | Work::RemoveCancelled { file, .. } => {
                files.insert(file.clone());
            }
            Work::Skip { file, start, .. } => {
                skipped.insert((file.clone(), *start));
            }
            // A task and its time blocks, under one "Undo".
            Work::Many(works) => works.iter().for_each(|w| add(w, files, skipped)),
            _ => {}
        }
    }
    let (mut files, mut skipped) = (BTreeSet::new(), BTreeSet::new());
    for pending in shared.pending.lock().map(|p| p.clone()).unwrap_or_default() {
        add(&pending.work, &mut files, &mut skipped);
    }
    (files, skipped)
}

/// How many delayed works on calendars' and contacts' files ended (done or
/// undone): a page that keeps what it read of the calendars a while (Health's
/// week) reads them again once it changed (the check of 36002ea, low 23).
static PIM_ENDED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `PIM_ENDED` now.
pub(crate) fn pim_ended() -> u64 {
    PIM_ENDED.load(Ordering::Relaxed)
}

/// Tests only: a delayed work on the calendars ended, as `forget` counts it.
#[cfg(test)]
pub(crate) fn pim_work_ended() {
    PIM_ENDED.fetch_add(1, Ordering::Relaxed);
}

/// Whether a work deletes a contact, an event or a task, or leaves an occurrence out: the pages follow at once.
fn touches_pim(work: &Work) -> bool {
    match work {
        Work::Remove { .. } | Work::RemoveCancelled { .. } | Work::Skip { .. } => true,
        Work::Many(works) => works.iter().any(touches_pim),
        _ => false,
    }
}

/// Deletes several files at once after ten seconds to undo, (account, file)
/// each: a task and its time blocks to come, or a task's blocks left to the
/// plan. `back` runs on "Undo", for what was changed at once (a task's link to
/// its block taken out): it says what went wrong, else "".
pub(crate) fn schedule_removals(qt: &QtThread, shared: &Arc<Shared>, files: Vec<(String, PathBuf)>, back: Option<Box<dyn Fn(&QtThread, &Arc<Shared>) -> String + Send + Sync>>, line: String) {
    let mut works: Vec<Work> = files.into_iter().map(|(account, file)| Work::Remove { account, file }).collect();
    works.extend(back.map(|back| Work::Back { back }));
    schedule(qt, shared, Work::Many(works), line);
}

/// Deletes a contact or an event after ten seconds to undo.
pub(crate) fn schedule_removal(qt: &QtThread, shared: &Arc<Shared>, account: &str, file: PathBuf, line: String) {
    schedule(qt, shared, Work::Remove { account: account.to_string(), file }, line);
}

/// Takes out an invitation's event its organizer cancelled, as your answer
/// to the cancellation (`pim::answer`, `cancel` its message's calendar),
/// after ten seconds to undo.
pub(crate) fn schedule_cancelled_removal(qt: &QtThread, shared: &Arc<Shared>, account: &str, file: PathBuf, cancel: String, line: String) {
    schedule(qt, shared, Work::RemoveCancelled { account: account.to_string(), file, cancel }, line);
}

/// Runs your mail filters on the inboxes as their preview said, after ten
/// seconds to undo; what they move leaves the lists meanwhile.
pub(crate) fn schedule_filters(qt: &QtThread, shared: &Arc<Shared>, planned: Vec<(Account, Vec<sioul_sync::filters::Job>)>, line: String) {
    schedule(qt, shared, Work::Filters { planned }, line);
}

/// Leaves one occurrence of a repeating event out, after ten seconds to undo.
pub(crate) fn schedule_skip(qt: &QtThread, shared: &Arc<Shared>, account: &str, file: PathBuf, start: i64, line: String) {
    schedule(qt, shared, Work::Skip { account: account.to_string(), file, start }, line);
}

/// The messages the Porch should not show either.
pub(crate) fn hidden_files(shared: &Shared) -> BTreeSet<PathBuf> {
    hidden(shared).0
}

/// Messages seen on their server only (a search's), moved or deleted while "Undo" is offered: their keys.
pub(crate) fn hidden_keys(shared: &Shared) -> BTreeSet<String> {
    fn add(work: &Work, keys: &mut BTreeSet<String>) {
        match work {
            Work::Batch { remote, action, .. } if moves(action) => keys.extend(remote.iter().map(ServerRef::key)),
            Work::AcrossRemote { place, .. } => {
                keys.insert(place.key());
            }
            Work::Many(works) => works.iter().for_each(|w| add(w, keys)),
            _ => {}
        }
    }
    let mut keys = BTreeSet::new();
    for pending in shared.pending.lock().map(|p| p.clone()).unwrap_or_default() {
        add(&pending.work, &mut keys);
    }
    keys
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
        let (cards, sent) = folder_cards(account, folders, folder, &files, config.mail.threads);
        let trusted = config.mail_sources().into_iter().find(|s| s.account.as_deref() == Some(account.id.as_str())).map(|s| s.trusted_ids).unwrap_or_default();
        let now = sioul_core::zone::now().timestamp().as_second();
        Some(json(&view::folder(account, folder, cards, &trusted, &sioul_core::words::Words::of(&config), open.all, &open.query, tr(), now, sent)))
    });
    let rows: Vec<DraftRow> = Draft::all().iter().filter(|d| !drafts.contains(&d.id)).map(draft_row).collect();
    let mode = crate::backend::mode_now();
    let hours = (mode.time != sioul_core::areas::Time::Any).then_some((mode.time, mode.week));
    let asked = shared.open_mailbox.lock().ok().and_then(|o| o.clone());
    let mailbox = asked.map(|ask| mailbox_page(&config, &ask, &files));
    MailViews { accounts: json(&view::mail_accounts(&accounts, tr(), hours)), folder: folder.unwrap_or_default(), mailbox, drafts: json(&rows) }
}

/// A folder's messages as its list reads them, from the folder's index
/// (`mailcache::folder`): each message's line, never the message itself,
/// but for those the index does not know yet (read once, ever). `hidden`:
/// the files waiting for their Undo. With `threads`, your answers from Sent
/// come too, for the conversations. Gmail's "All Mail", shown as the
/// archive, leaves out what the inbox and Sent show, by their lines' ids.
pub(crate) fn folder_cards(account: &Account, folders: &[Folder], folder: &Folder, hidden: &BTreeSet<PathBuf>, threads: bool) -> (Vec<Card>, Option<Vec<Card>>) {
    let root = account.maildir_path();
    // Oldest first, as `maildir::read_messages` gives them.
    let cards_of = |local: &str| {
        let mut cards: Vec<Card> = sioul_core::mailcache::folder(&account.id, &root, local).into_iter().map(|(path, line)| line.card(&path)).collect();
        cards.sort_by_key(|c| c.date.unwrap_or(0));
        cards
    };
    let sent_folders = || folders.iter().filter(|f| f.role == Role::Sent);
    let shown_elsewhere: BTreeSet<String> = if folder.role == Role::All {
        let mut ids: BTreeSet<String> = BTreeSet::new();
        for local in std::iter::once("").chain(sent_folders().map(|f| f.local.as_str())) {
            ids.extend(sioul_core::mailcache::folder(&account.id, &root, local).into_iter().filter_map(|(_, line)| line.message_id));
        }
        ids
    } else {
        BTreeSet::new()
    };
    let cards: Vec<Card> = cards_of(&folder.local)
        .into_iter()
        .filter(|c| c.path.as_ref().is_none_or(|p| !hidden.contains(p)))
        .filter(|c| c.message_id.as_ref().is_none_or(|id| !shown_elsewhere.contains(id)))
        .collect();
    // By conversation, when chosen: your answers come from Sent.
    let sent = (threads && !matches!(folder.role, Role::Sent | Role::Drafts)).then(|| sent_folders().flat_map(|f| cards_of(&f.local)).collect::<Vec<Card>>());
    (cards, sent)
}

/// Computes the mail page on a thread and shows it, unless a newer one came first.
pub(crate) fn show_mail(qt: &QtThread, shared: &Arc<Shared>) {
    crate::mailsearch::changed(qt, shared);
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.mail_job, move |shared| show_mail_here(&qt, shared));
}

/// The mail page computed on this thread, and shown unless a newer one came
/// first. `backend::show` runs it in the same look at the mail as the Porch;
/// the page reads its folder's index (`folder_cards`), never its messages.
pub(crate) fn show_mail_here(qt: &QtThread, shared: &Shared) {
    let generation = shared.mail_generation.fetch_add(1, Ordering::Relaxed) + 1;
    let views = views(shared);
    let _ = qt.queue(move |mut sioul| {
        // Only the newest (see `backend::show`).
        if sioul.shared().mail_generation.load(Ordering::Relaxed) != generation || sioul.shared().mail_shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
            return;
        }
        sioul.as_mut().set_mail_accounts(QString::from(&views.accounts));
        sioul.as_mut().set_mail_folder(QString::from(&views.folder));
        if let Some(mailbox) = &views.mailbox {
            sioul.as_mut().set_mailbox(QString::from(mailbox));
        }
        sioul.as_mut().set_drafts(QString::from(&views.drafts));
    });
}

/// The account a message belongs to, and where its file is now.
pub(crate) fn locate(key: &str) -> Option<(Account, PathBuf)> {
    let file = maildir::locate(Path::new(key))?;
    let account = load_config().account_of(&file)?.clone();
    Some((account, file))
}

/// "read", "unread", "flag", "unflag", "archive", "trash", "junk",
/// "not-junk", "spam", "not-spam", "move". "Spam" and "Not spam", from the
/// review queue or the Reader, as the message's folder asks: in a Junk
/// folder (your filter moved it there), "Spam" keeps it there marked, "Not
/// spam" brings it back to the inbox; elsewhere, "Spam" moves it into the
/// Junk folder, "Not spam" leaves it where it is, marked.
pub(crate) fn act(qt: &QtThread, shared: &Arc<Shared>, key: &str, action: &str, target: &str) {
    if let Some(place) = ServerRef::parse(key) {
        return act_on_server(qt, shared, place, action, target);
    }
    let Some((account, file)) = locate(key) else {
        set_status(qt, tr().text("mail-message-gone", None));
        return;
    };
    let folder = mailbox::folder_of(&account, &file);
    let in_junk = folder.as_ref().is_some_and(|f| f.role == Role::Junk);
    let Some(action) = action_of(action, target, in_junk) else { return };
    let line = match &action {
        Action::Read(_) | Action::Flag(_) => return act_now(qt, shared, account, file, action),
        Action::Archive => tr().text("undo-archived", None),
        Action::Trash if folder.as_ref().is_some_and(|f| f.role == Role::Trash) => tr().text("undo-deleted", None),
        Action::Trash => tr().text("undo-trashed", None),
        Action::Junk if in_junk => tr().text("undo-spam-kept", None),
        Action::Junk => tr().text("undo-junked", None),
        Action::NotJunk => tr().text("undo-not-junk", None),
        Action::NotSpam => tr().text("undo-not-spam", None),
        Action::Move(name) => {
            let title = mailbox::folders(&account.id).into_iter().find(|f| f.name == *name).map_or_else(|| name.clone(), |f| folder_title(&f));
            say("undo-moved", &[("folder", title)])
        }
        // Your filter's own move is never asked from the window.
        Action::Filtered(_) => return,
    };
    schedule(qt, shared, Work::Act { account, file, action }, line);
}

/// An act on a message seen on its server only (a search's): reading and
/// flags at once, moves, archiving and the trash with "Undo"; what teaches
/// the spam filter needs the message here first.
fn act_on_server(qt: &QtThread, shared: &Arc<Shared>, place: ServerRef, action: &str, target: &str) {
    let Some(account) = load_config().account(&place.account).filter(|a| a.syncs()).cloned() else {
        set_status(qt, say("account-unknown", &[("id", place.account.clone())]));
        return;
    };
    let Some(action) = action_of(action, target, false) else { return };
    let line = match &action {
        Action::Read(_) | Action::Flag(_) => return act_now_many(qt, shared, Vec::new(), vec![(account, place)], action),
        Action::Archive => tr().text("undo-archived", None),
        Action::Trash => tr().text("undo-trashed", None),
        Action::Move(name) => {
            let title = mailbox::folders(&account.id).into_iter().find(|f| f.name == *name).map_or_else(|| name.clone(), |f| folder_title(&f));
            say("undo-moved", &[("folder", title)])
        }
        _ => {
            set_status(qt, tr().text("search-open-first", None));
            return;
        }
    };
    schedule(qt, shared, Work::Batch { account, files: Vec::new(), remote: vec![place], action }, line);
}

/// A sender blocked from one of their messages: the message goes into the
/// spam filter's label log (`spam::labels`). Blocking does not wait on it.
pub(crate) fn label_blocked(key: &str) {
    if let Some((account, file)) = locate(key) {
        let _ = mailbox::label(&account, &file, sioul_core::spam::labels::Source::Block);
    }
}

fn folder_title(folder: &Folder) -> String {
    if folder.role == Role::Other { folder.display.clone() } else { tr().text(folder.role.message_id(), None) }
}

/// What a key names, as an act on its message (`in_junk`: it is in a Junk
/// folder); None for what is not a message here.
fn action_of(action: &str, target: &str, in_junk: bool) -> Option<Action> {
    Some(match action {
        "read" => Action::Read(true),
        "unread" => Action::Read(false),
        "flag" => Action::Flag(true),
        "unflag" => Action::Flag(false),
        "archive" => Action::Archive,
        "trash" => Action::Trash,
        "junk" | "spam" => Action::Junk,
        "not-junk" => Action::NotJunk,
        // Your filter moved it into the Junk folder: back to the inbox.
        "not-spam" if in_junk => Action::NotJunk,
        "not-spam" => Action::NotSpam,
        "move" if !target.is_empty() => Action::Move(target.to_string()),
        _ => return None,
    })
}

/// Whether a stored message is in a Junk folder of its account.
fn in_junk(account: &Account, file: &Path) -> bool {
    mailbox::folder_of(account, file).is_some_and(|f| f.role == Role::Junk)
}

/// The same act on several messages (a selection, the review queue's "for
/// all"): reading and flagging at once, the rest under one "Undo"; "spam"
/// and "not-spam" as each message's folder asks (`act`).
pub(crate) fn act_many(qt: &QtThread, shared: &Arc<Shared>, keys: &[String], action: &str) {
    let Some(first) = action_of(action, "", false) else { return };
    let config = load_config();
    let found: Vec<(Account, PathBuf)> = keys.iter().filter(|k| !ServerRef::is_key(k)).filter_map(|k| locate(k)).collect();
    // Seen on their server only (a search's).
    let remote: Vec<(Account, ServerRef)> = keys.iter().filter_map(|k| ServerRef::parse(k)).filter_map(|p| Some((config.account(&p.account).filter(|a| a.syncs())?.clone(), p))).collect();
    if found.is_empty() && remote.is_empty() {
        set_status(qt, tr().text("mail-message-gone", None));
        return;
    }
    if !moves(&first) {
        act_now_many(qt, shared, found, remote, first);
        return;
    }
    // Archiving and the trash: one connection per account, the messages of a folder as one set.
    if matches!(first, Action::Archive | Action::Trash) {
        let in_trash = !found.is_empty() && remote.is_empty() && found.iter().all(|(account, file)| mailbox::folder_of(account, file).is_some_and(|f| f.role == Role::Trash));
        let n = found.len() + remote.len();
        let id = match (&first, in_trash) {
            (Action::Archive, _) => "undo-many-archived",
            (_, true) => "undo-many-deleted",
            _ => "undo-many-trashed",
        };
        let works = batches(found, remote, &first);
        schedule(qt, shared, Work::Many(works), say(id, &[("n", n.to_string())]));
        return;
    }
    if found.is_empty() {
        set_status(qt, tr().text("search-open-first", None));
        return;
    }
    let in_trash = found.iter().all(|(account, file)| mailbox::folder_of(account, file).is_some_and(|f| f.role == Role::Trash));
    let id = match (action, &first) {
        ("spam", _) => "undo-review-all-spam",
        ("not-spam", _) => "undo-review-all-not-spam",
        (_, Action::Archive) => "undo-many-archived",
        (_, Action::Junk) => "undo-many-junked",
        (_, Action::NotJunk) => "undo-many-not-junk",
        (_, Action::NotSpam) => "undo-many-not-spam",
        _ if in_trash => "undo-many-deleted",
        _ => "undo-many-trashed",
    };
    let line = say(id, &[("n", found.len().to_string())]);
    let works = found
        .into_iter()
        .filter_map(|(account, file)| {
            let action = action_of(action, "", in_junk(&account, &file))?;
            Some(Work::Act { account, file, action })
        })
        .collect();
    schedule(qt, shared, Work::Many(works), line);
}

/// Messages of several accounts, kept here or seen on their server only, as
/// one batch per account (`Work::Batch`).
fn batches(found: Vec<(Account, PathBuf)>, remote: Vec<(Account, ServerRef)>, action: &Action) -> Vec<Work> {
    let mut by_account: BTreeMap<String, (Account, Vec<PathBuf>, Vec<ServerRef>)> = BTreeMap::new();
    for (account, file) in found {
        by_account.entry(account.id.clone()).or_insert_with(|| (account, Vec::new(), Vec::new())).1.push(file);
    }
    for (account, place) in remote {
        by_account.entry(account.id.clone()).or_insert_with(|| (account, Vec::new(), Vec::new())).2.push(place);
    }
    by_account.into_values().map(|(account, files, remote)| Work::Batch { account, files, remote, action: action.clone() }).collect()
}

/// Messages dropped on a folder, of their account or of another one: those
/// of its account in one connection, a folder's as one set (`UID MOVE`);
/// those of another one copied there first, then taken off theirs.
pub(crate) fn move_many(qt: &QtThread, shared: &Arc<Shared>, keys: &[String], account: &str, folder: &str) {
    let config = load_config();
    let Some(to) = config.account(account).filter(|a| a.syncs()).cloned() else {
        set_status(qt, say("account-unknown", &[("id", account.to_string())]));
        return;
    };
    let (mut files, mut remote, mut across) = (Vec::new(), Vec::new(), Vec::new());
    for key in keys {
        if let Some(place) = ServerRef::parse(key) {
            // Already there: nothing to do.
            if place.account == to.id {
                if place.folder != folder {
                    remote.push(place);
                }
            } else if let Some(from) = config.account(&place.account).filter(|a| a.syncs()) {
                across.push(Work::AcrossRemote { from: from.clone(), place, to: to.clone(), folder: folder.to_string() });
            }
        } else if let Some((from, file)) = locate(key) {
            if from.id != to.id {
                across.push(Work::Across { from, file, to: to.clone(), folder: folder.to_string() });
            } else if mailbox::folder_of(&from, &file).is_none_or(|f| f.name != folder) {
                files.push(file);
            }
        }
    }
    let count = files.len() + remote.len() + across.len();
    if count == 0 {
        return;
    }
    let crossing = !across.is_empty();
    let mut works = across;
    if !files.is_empty() || !remote.is_empty() {
        works.insert(0, Work::Batch { account: to.clone(), files, remote, action: Action::Move(folder.to_string()) });
    }
    let title = mailbox::folders(&to.id).into_iter().find(|f| f.name == folder).map_or_else(|| folder.to_string(), |f| folder_title(&f));
    let n = count.to_string();
    let line = if crossing {
        say("undo-moved-across", &[("n", n), ("folder", title), ("account", to.address.clone().unwrap_or_else(|| to.id.clone()))])
    } else if count > 1 {
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

/// Reading and flagging several messages: shown at once, then each
/// account's server told in one connection; put back where it refuses.
fn act_now_many(qt: &QtThread, shared: &Arc<Shared>, found: Vec<(Account, PathBuf)>, remote: Vec<(Account, ServerRef)>, action: Action) {
    let letters = |before: &str| match &action {
        Action::Read(true) => format!("{before}S"),
        Action::Read(false) => before.replace('S', ""),
        Action::Flag(true) => format!("{before}F"),
        Action::Flag(false) => before.replace('F', ""),
        _ => before.to_string(),
    };
    type Marks = (Account, Vec<(PathBuf, String)>, Vec<ServerRef>);
    let mut by_account: BTreeMap<String, Marks> = BTreeMap::new();
    for (account, file) in found {
        let before = maildir::flags_of(&file);
        let file = maildir::set_flags(&file, &letters(&before)).unwrap_or(file);
        by_account.entry(account.id.clone()).or_insert_with(|| (account, Vec::new(), Vec::new())).1.push((file, before));
    }
    for (account, place) in remote {
        by_account.entry(account.id.clone()).or_insert_with(|| (account, Vec::new(), Vec::new())).2.push(place);
    }
    let keys: Vec<String> = by_account.values().flat_map(|(_, _, remote)| remote.iter().map(ServerRef::key)).collect();
    crate::mailsearch::marked(&keys, letters);
    show_mail(qt, shared);
    for (account, files, remote) in by_account.into_values() {
        let (qt, shared, action) = (qt.clone(), Arc::clone(shared), action.clone());
        std::thread::spawn(move || {
            let paths: Vec<PathBuf> = files.iter().map(|(file, _)| file.clone()).collect();
            let pairs: Vec<(String, sioul_core::card::ImapOrigin)> = remote.iter().map(|p| (p.folder.clone(), p.origin)).collect();
            if let Err(e) = secret::password(&account).and_then(|p| mailbox::act_many(&account, &p, &paths, &pairs, &action)) {
                for (file, before) in &files {
                    if let Some(now) = maildir::locate(file) {
                        let _ = maildir::set_flags(&now, before);
                    }
                }
                tell(&qt, &shared, e.sentence(tr(), &account.id));
                show_mail(&qt, &shared);
            }
        });
    }
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
    let events = touches_pim(&pending.work);
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
    done(qt, shared, pending);
}

/// What waited, taken already: done, its line said, the pages drawn again.
fn done(qt: &QtThread, shared: &Arc<Shared>, pending: &Arc<Pending>) {
    // Done: the line says it, without "Undo" now.
    let mut moved = Vec::new();
    let line = perform_moving(&pending.work, shared, &mut moved).or_else(|| Some(pending.line.clone()));
    tell_moved(qt, &moved);
    forget(qt, shared, pending);
    if let Some(line) = line {
        tell(qt, shared, line);
    }
    show(qt, shared);
    // The agenda drawn again whatever the work did: refused by the rule, the
    // event it hid comes back at once, not at the next redraw (low 23).
    if touches_pim(&pending.work) {
        crate::pim::show_pim(qt, shared);
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
pub(crate) fn rest(qt: &QtThread, shared: &Arc<Shared>, previous: sioul_core::quiet::Overrides, after: sioul_core::quiet::Overrides, line: String) {
    schedule(qt, shared, Work::Rest { previous, after }, line);
}

/// A change made already (a drag): "Undo" stays offered for a moment, as for
/// mail, and `back` puts back what was there.
pub(crate) fn offer_back(qt: &QtThread, shared: &Arc<Shared>, line: String, back: impl Fn(&QtThread, &Arc<Shared>) -> String + Send + Sync + 'static) {
    schedule(qt, shared, Work::Back { back: Box::new(back) }, line);
}

/// Takes a pending act off the list; the one before it, if any, can still be undone.
fn forget(qt: &QtThread, shared: &Shared, pending: &Arc<Pending>) {
    if touches_pim(&pending.work) {
        PIM_ENDED.fetch_add(1, Ordering::Relaxed);
    }
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

/// The window told where moved messages' files went (`mail_moved`): a
/// message open there follows its file.
fn tell_moved(qt: &QtThread, moved: &[(PathBuf, PathBuf)]) {
    if moved.is_empty() {
        return;
    }
    let pairs: Vec<[String; 2]> = moved.iter().map(|(before, after)| [before.display().to_string(), after.display().to_string()]).collect();
    let json = serde_json::to_string(&pairs).unwrap_or_default();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().mail_moved(QString::from(&json)));
}

/// Does what waited; returns what the status line says about it, if anything.
fn perform(work: &Work, shared: &Shared) -> Option<String> {
    perform_moving(work, shared, &mut Vec::new())
}

/// `perform`, where moved messages' files went added to `moved`.
fn perform_moving(work: &Work, shared: &Shared, moved: &mut Vec<(PathBuf, PathBuf)>) -> Option<String> {
    match work {
        Work::Act { account, file, action } => match secret::password(account).and_then(|p| mailbox::act(account, &p, file, action)) {
            Ok(followed) => {
                moved.extend(followed);
                nudge(shared, &account.id);
                None
            }
            Err(e) => Some(e.sentence(tr(), &account.id)),
        },
        Work::Across { from, file, to, folder } => {
            let done = secret::password(from).and_then(|p| secret::password(to).and_then(|q| mailbox::move_across(from, &p, file, to, &q, folder)));
            match done {
                Ok(followed) => {
                    moved.extend(followed);
                    nudge(shared, &from.id);
                    nudge(shared, &to.id);
                    None
                }
                Err(e) => Some(e.sentence(tr(), &from.id)),
            }
        }
        Work::Batch { account, files, remote, action } => {
            let pairs: Vec<(String, sioul_core::card::ImapOrigin)> = remote.iter().map(|p| (p.folder.clone(), p.origin)).collect();
            match secret::password(account).and_then(|p| mailbox::act_many(account, &p, files, &pairs, action)) {
                Ok(followed) => {
                    moved.extend(followed);
                    nudge(shared, &account.id);
                    crate::mailsearch::forget(&remote.iter().map(ServerRef::key).collect::<Vec<_>>());
                    None
                }
                Err(e) => Some(e.sentence(tr(), &account.id)),
            }
        }
        Work::AcrossRemote { from, place, to, folder } => {
            let moved = secret::password(from).and_then(|p| secret::password(to).and_then(|q| mailbox::move_across_from_server(from, &p, &place.folder, place.origin, to, &q, folder)));
            match moved {
                Ok(()) => {
                    nudge(shared, &from.id);
                    nudge(shared, &to.id);
                    crate::mailsearch::forget(&[place.key()]);
                    None
                }
                Err(e) => Some(e.sentence(tr(), &from.id)),
            }
        }
        // Each in turn; the first problem is said, the others are still done.
        Work::Many(works) => works.iter().filter_map(|w| perform_moving(w, shared, moved)).collect::<Vec<_>>().into_iter().next(),
        Work::Send { draft } => Some(send_now(draft, shared)),
        Work::SendSigned { draft, account, outgoing } => Some(send_signed_now(draft, account, outgoing, shared)),
        Work::Discard { draft } => {
            if let Some(draft) = Draft::by_id(draft) {
                draft.discard();
            }
            None
        }
        // Through the one rule, on the file as it is now: one become a meeting with others meanwhile stays.
        Work::Remove { account, file } => {
            // A task taken out: the letters it stood for are free again (review of e5bd27b, later 6),
            // by the task's own UID (`letters::untie_removed`), read before the file goes.
            let before = std::fs::read_to_string(file).ok();
            match sioul_core::vdir::remove_item(file) {
                Ok(()) => {
                    crate::pim::nudge(shared, account);
                    if let (Some(text), Some(root)) = (before, load_config().notes_root_path()) {
                        crate::letters::untie_removed(&root, &text);
                    }
                    None
                }
                Err(e) => Some(e),
            }
        }
        Work::RemoveCancelled { account, file, cancel } => {
            if sioul_core::vdir::calendar_read_only(file) == Some(true) {
                return Some(tr().text("agenda-read-only", None));
            }
            // The file as it is now, read again: still the meeting its organizer
            // cancelled whole, or nothing is taken out (the check of ef8c0ca, low 27).
            let now = match std::fs::read_to_string(file) {
                Ok(now) => now,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
                Err(e) => return Some(format!("{}: {e}", file.display())),
            };
            if !matches!(sioul_core::agenda::cancellation(&now, cancel, &sioul_core::agenda::own_addresses()), sioul_core::agenda::Cancellation::Whole) {
                return Some(tr().text("invitation-cancel-changed", None));
            }
            match std::fs::remove_file(file) {
                Ok(()) => {
                    crate::pim::nudge(shared, account);
                    None
                }
                Err(e) => Some(format!("{}: {e}", file.display())),
            }
        }
        Work::Rest { .. } | Work::Untrash { .. } | Work::Back { .. } => None,
        Work::Unsubscribe { list, sender, account, way } => Some(unsubscribe_now(list, sender, account, way, shared)),
        Work::Filters { planned } => {
            let said = crate::filters::perform(planned);
            for (account, _) in planned {
                nudge(shared, &account.id);
            }
            said
        }
        Work::Skip { account, file, start } => {
            // One time left out is a deletion of it: never of a meeting with others, nor in a calendar you only read.
            let change = sioul_core::agenda::Change::Delete;
            if let Err(refusal) = sioul_core::agenda::guard(file, change) {
                return Some(tr().text(refusal.word(&change), None));
            }
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

/// Sends what was built and signed at Send, as built; then the draft goes.
fn send_signed_now(id: &str, account: &Account, outgoing: &compose::Outgoing, shared: &Shared) -> String {
    let sent = secret::password(account).and_then(|p| sioul_sync::send::send_built(&config_path(), account, &p, outgoing));
    let discard = || {
        if let Some(draft) = Draft::by_id(id) {
            draft.discard();
        }
    };
    match sent {
        Ok(()) => {
            discard();
            nudge(shared, &account.id);
            tr().text("mail-sent", None)
        }
        Err(e @ SyncError::NotFiled(_)) => {
            discard();
            e.sentence(tr(), &account.id)
        }
        Err(e) => say("mail-not-sent", &[("detail", e.sentence(tr(), &account.id))]),
    }
}

/// What a message offers to leave its list (docs/client.md, "Unsubscribing"),
/// judged as the Reader judges it, and the account a request by mail goes
/// out from: the one the message came to, when it sends.
fn unsubscribe_offer(config: &Config, triaged: &sioul_core::porch::Triaged) -> Option<(unsubscribe::Offer, Option<Account>)> {
    let account = triaged.card.account.as_deref();
    let trusted = config.mail_sources().into_iter().find(|s| s.account.as_deref() == account).map(|s| s.trusted_ids).unwrap_or_default();
    let mut offer = unsubscribe::offer(&triaged.card, &trusted, triaged.trust, &triaged.lane, &triaged.reasons)?;
    // In the junk, as spam: answering it tells its sender your address is read.
    let file = triaged.card.path.as_deref();
    if file.and_then(|f| mailbox::folder_of(config.account_of(f)?, f)).is_some_and(|f| f.role == Role::Junk) {
        offer.way = Err(unsubscribe::Refusal::Spam);
    }
    let sender = account.and_then(|id| config.account(id)).filter(|a| a.syncs() && a.address.is_some()).cloned();
    Some((offer, sender))
}

/// The Reader's button for a message: none when it names no way out of a list.
pub(crate) fn unsubscribe_view(config: &Config, triaged: &sioul_core::porch::Triaged) -> Option<unsubscribe::UnsubscribeView> {
    let (offer, sender) = unsubscribe_offer(config, triaged)?;
    let record = unsubscribe::Record::load(&unsubscribe::Record::default_path());
    let from = sender.as_ref().and_then(|a| a.address.as_deref());
    Some(unsubscribe::view(&offer, &record, from, tr(), sioul_core::zone::now().date()))
}

/// One click on "Unsubscribe": one click (RFC 8058) or a message to the list
/// waits ten seconds with "Undo", as sending does; a list that takes it only
/// on its web page has the page returned, to open in the browser, at once.
/// What cannot be done is said in the status line, the button's tip.
pub(crate) fn unsubscribe(qt: &QtThread, shared: &Arc<Shared>, config: &Config, triaged: Option<sioul_core::porch::Triaged>) -> String {
    let Some(triaged) = triaged else {
        set_status(qt, tr().text("mail-message-gone", None));
        return String::new();
    };
    let Some((offer, sender)) = unsubscribe_offer(config, &triaged) else { return String::new() };
    let record = unsubscribe::Record::default_path();
    let shown = unsubscribe::view(&offer, &unsubscribe::Record::load(&record), sender.as_ref().and_then(|a| a.address.as_deref()), tr(), sioul_core::zone::now().date());
    if !shown.offered {
        tell(qt, shared, shown.tip);
        return String::new();
    }
    // A demo profile stays off the network.
    if crate::backend::offline() {
        tell(qt, shared, tr().text("unsubscribe-demo", None));
        return String::new();
    }
    let list = offer.list.clone();
    let address = triaged.card.from_address.clone().unwrap_or_default();
    match offer.way {
        Ok(unsubscribe::Way::Page { url }) => {
            let left = unsubscribe::Left { key: list.key.clone(), name: list.name.clone(), address, way: "page".into(), at: sioul_core::zone::now().timestamp().as_second() };
            if let Err(e) = unsubscribe::Record::add(&record, left) {
                set_status(qt, e);
            }
            tell(qt, shared, say("unsubscribe-page-open", &[("list", list.name)]));
            url
        }
        Ok(way) => {
            let line = say("unsubscribe-pending", &[("list", list.name.clone())]);
            // Asked already, still waiting: its line again, not a second request.
            let waiting = shared.pending.lock().is_ok_and(|p| p.iter().any(|p| !p.taken.load(Ordering::Relaxed) && matches!(&p.work, Work::Unsubscribe { list: l, .. } if l.key == list.key)));
            if waiting {
                let _ = qt.queue(move |mut sioul| sioul.as_mut().set_undo_line(QString::from(&line)));
                return String::new();
            }
            let account = sender.map(|a| a.id).unwrap_or_default();
            schedule(qt, shared, Work::Unsubscribe { list, sender: address, account, way }, line);
            String::new()
        }
        Err(_) => String::new(),
    }
}

/// Leaves a list, its ten seconds gone: posts the one click, or sends the
/// message from `account` as any message is sent (a copy in Sent, which
/// Gmail files itself). Done, it is kept in the record. Returns what the
/// status line says.
fn unsubscribe_now(list: &unsubscribe::List, sender: &str, account: &str, way: &unsubscribe::Way, shared: &Shared) -> String {
    let failed = |why: String| say("unsubscribe-failed", &[("list", list.name.clone()), ("why", why)]);
    let done = |way: &str| {
        let left = unsubscribe::Left { key: list.key.clone(), name: list.name.clone(), address: sender.to_string(), way: way.to_string(), at: sioul_core::zone::now().timestamp().as_second() };
        unsubscribe::Record::add(&unsubscribe::Record::default_path(), left).err()
    };
    match way {
        unsubscribe::Way::OneClick { url } => match sioul_sync::unsubscribe::one_click(url) {
            Ok(sioul_sync::unsubscribe::Outcome::Done) => done("one-click").unwrap_or_else(|| say("unsubscribe-done-one-click", &[("list", list.name.clone())])),
            Ok(sioul_sync::unsubscribe::Outcome::Refused(code)) => failed(say("unsubscribe-why-status", &[("code", code.to_string())])),
            Ok(sioul_sync::unsubscribe::Outcome::Elsewhere(host)) => failed(say("unsubscribe-why-redirect", &[("host", host)])),
            Ok(sioul_sync::unsubscribe::Outcome::TimedOut) => failed(say("unsubscribe-why-timeout", &[("seconds", sioul_sync::unsubscribe::WAIT.as_secs().to_string())])),
            Err(SyncError::Tls(detail)) => failed(say("unsubscribe-why-tls", &[("detail", detail)])),
            Err(e) => failed(say("unsubscribe-why-network", &[("detail", e.detail().to_string())])),
        },
        unsubscribe::Way::Mail { to, subject, body } => {
            let config = load_config();
            let Some(account) = config.account(account).filter(|a| a.syncs()).cloned() else {
                return failed(say("account-unknown", &[("id", account.to_string())]));
            };
            let me = account.address.clone().unwrap_or_default();
            let name = account.name.clone().unwrap_or_default();
            let sent = unsubscribe::request((&name, &me), to, subject, body, sioul_core::zone::now().timestamp().as_second()).and_then(|outgoing| {
                let password = secret::password(&account).map_err(|e| e.sentence(tr(), &account.id))?;
                let account = crate::pim::with_smtp(&account)?;
                match sioul_sync::send::send(&account, &password, &outgoing) {
                    // Sent; a copy not filed in Sent changes nothing for the list.
                    Ok(()) | Err(SyncError::NotFiled(_)) => Ok(()),
                    Err(e) => Err(e.sentence(tr(), &account.id)),
                }
            });
            match sent {
                Ok(()) => {
                    nudge(shared, &account.id);
                    done("mail").unwrap_or_else(|| say("unsubscribe-done-mail", &[("list", list.name.clone()), ("to", to.clone())]))
                }
                Err(why) => failed(why),
            }
        }
        unsubscribe::Way::Page { .. } => String::new(),
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
            let root = load_config().notes_root_path();
            let back = root.ok_or_else(|| tr().text("error-no-store", None)).and_then(|root| sioul_core::notes::untrash(&root, trashed, path));
            crate::work::show_work(qt, shared);
            (back.err().unwrap_or_else(|| tr().text("undo-done", None)), None)
        }
        Work::Rest { previous, after } => {
            // What the act changed put back as it was, over the file as it is now
            // (`Overrides::put_back`: `save_merged` then writes what differs from this read).
            let path = sioul_core::quiet::Overrides::default_path();
            let _ = sioul_core::quiet::Overrides::load(&path).put_back(previous, after).save(&path);
            let _ = qt.queue(|mut sioul| sioul.as_mut().set_mode(QString::from(&crate::backend::mode_json())));
            show(qt, shared);
            crate::work::show_work(qt, shared);
            (tr().text("undo-done", None), None)
        }
        Work::Back { back } => {
            let problem = back(qt, shared);
            (if problem.is_empty() { tr().text("undo-done", None) } else { problem }, None)
        }
        Work::Send { draft } | Work::SendSigned { draft, .. } => (tr().text("undo-send-undone", None), Some(draft.clone())),
        Work::Discard { draft } => (tr().text("undo-done", None), Some(draft.clone())),
        // A task or an event deleted, an occurrence left out: back in the plan and the agenda at once.
        Work::Remove { .. } | Work::RemoveCancelled { .. } | Work::Skip { .. } => {
            if let Ok(mut cache) = shared.loaded.lock() {
                *cache = None;
            }
            crate::work::show_work(qt, shared);
            crate::pim::show_pim(qt, shared);
            (tr().text("undo-done", None), None)
        }
        // A task and its time blocks: what was changed at once put back, what waited kept.
        Work::Many(works) if works.iter().any(|w| matches!(w, Work::Back { .. })) || touches_pim(&pending.work) => {
            // Each put back, the first problem said.
            let problems: Vec<String> = works.iter().filter_map(|w| if let Work::Back { back } = w { Some(back(qt, shared)) } else { None }).collect();
            let problem = problems.into_iter().find(|p| !p.is_empty());
            if let Ok(mut cache) = shared.loaded.lock() {
                *cache = None;
            }
            crate::work::show_work(qt, shared);
            crate::pim::show_pim(qt, shared);
            (problem.unwrap_or_else(|| tr().text("undo-done", None)), None)
        }
        Work::Act { .. } | Work::Across { .. } | Work::Batch { .. } | Work::AcrossRemote { .. } | Work::Many(_) | Work::Unsubscribe { .. } | Work::Filters { .. } => (tr().text("undo-done", None), None),
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
    /// "Attaching 2 files…" while files shared from another application are copied (`outside`), else "".
    attaching: String,
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
    // Its copy here (the path saved may be another device's), read once and
    // whole (`crypto::copy_of`); who wrote it and when, from its headers,
    // which an encrypted message keeps in clear.
    let copy = crate::crypto::copy_of(&draft);
    let original = copy.as_deref().and_then(Card::from_bytes);
    let below = match (draft.kind, original) {
        (DraftKind::New, _) | (_, None) => String::new(),
        (DraftKind::Forward, Some(card)) => say("compose-below-forward", &[("name", card.sender().to_string()), ("date", view::date(tr(), card.date))]),
        (_, Some(card)) => say("compose-below-quote", &[("name", card.sender().to_string()), ("date", view::date(tr(), card.date))]),
    };
    // Its copy encrypted: it stays so, and the window says where it opens; its
    // attachments are listed only from what opens here, in memory.
    let answered = crate::crypto::answered(copy.as_deref());
    let sealed = match (&answered, draft.kind) {
        (crate::crypto::Answered::Clear, _) | (_, DraftKind::New) => None,
        (crate::crypto::Answered::Opens(_), _) => Some(tr().text("compose-below-sealed", None)),
        (crate::crypto::Answered::AtSend, _) => Some(tr().text("compose-below-sealed-card", None)),
        (crate::crypto::Answered::Elsewhere, DraftKind::Forward) => Some(tr().text("compose-forward-sealed-not-here", None)),
        (crate::crypto::Answered::Elsewhere, _) => Some(tr().text("compose-below-sealed-not-here", None)),
        // No key of yours known here opens it: never quoted nor forwarded from here (F16).
        (crate::crypto::Answered::NoKey, DraftKind::Forward) => Some(tr().text("compose-forward-sealed-no-key", None)),
        (crate::crypto::Answered::NoKey, _) => Some(tr().text("compose-below-sealed-no-key", None)),
    };
    let below = match sealed {
        Some(line) if !below.is_empty() => format!("{below} {line}"),
        _ => below,
    };
    let forwarded = match &answered {
        crate::crypto::Answered::Clear => draft.forwarded(),
        crate::crypto::Answered::Opens(opened) => draft.forwarded_from(Some(opened)),
        _ => Vec::new(),
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
        forwarded: forwarded.into_iter().map(|(index, name)| AttachedView { index: index as usize, name, size: String::new() }).collect(),
        title,
        below,
        sign: draft.sign,
        encrypt: draft.encrypt,
        protection: crate::crypto::draft_protection(&draft),
        attaching: crate::outside::attaching(&draft.id),
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
    Ok(say("compose-saved", &[("time", sioul_core::zone::now().strftime("%H:%M").to_string())]))
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
        let taken = draft.attachments.remove(index);
        draft.save()?;
        // A copy Sioul made of a file another application shared goes with it.
        compose::remove_own_copy(&taken);
        return Ok(());
    }
    draft.save().map(|_| ())
}

/// Sends after ten seconds; the window closes, "Undo" opens it again.
/// Returns what stops it now (no recipient), else nothing.
pub(crate) fn send(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> Option<String> {
    let Some(draft) = Draft::by_id(id) else { return Some(tr().text("mail-draft-gone", None)) };
    // Files shared from another application still being copied: not without them.
    if crate::outside::coming(id) {
        return Some(tr().text("compose-wait-files", None));
    }
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

/// Sends after ten seconds a message built and signed at Send with your
/// security key; "Undo" throws it away and opens the draft again.
pub(crate) fn send_signed(qt: &QtThread, shared: &Arc<Shared>, id: &str, account: Account, outgoing: compose::Outgoing) {
    schedule(qt, shared, Work::SendSigned { draft: id.to_string(), account, outgoing }, tr().text("undo-sending", None));
}

/// The writing window closed: an untouched draft goes, a written one stays in Drafts.
pub(crate) fn closed(qt: &QtThread, shared: &Arc<Shared>, id: &str) {
    let Some(draft) = Draft::by_id(id) else { return };
    let signature = load_config().account(&draft.account).and_then(|a| a.signature.clone());
    let pending = hidden(shared).1.contains(id);
    if draft.is_empty(signature.as_deref()) && !crate::outside::coming(id) {
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

    /// New mail's notification: one message opens it by its address; several,
    /// the mailbox at its top (the unified mailbox's part 4, decision 3).
    #[test]
    fn several_new_messages_open_the_mailbox() {
        assert_eq!(crate::mailnote::opens_of(Some("mid:a@example.org")), ("mail", "mid:a@example.org".to_string()));
        assert_eq!(crate::mailnote::opens_of(None), ("mailbox", String::new()));
    }

    /// The mailbox's chip and address, kept on this device and read back;
    /// what else it was asked (the search, the windows) is not kept.
    #[test]
    fn the_mailbox_s_chip_and_address_are_kept() {
        sioul_core::config::stand_ins_only();
        assert!(mailbox_view_path().starts_with(sioul_core::config::state_dir()));
        // A new device: its state folder not made yet.
        let fresh = sioul_core::config::state_dir().join(format!("mailbox-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&fresh);
        let path = fresh.join("sioul").join("mailbox-view.toml");
        assert_eq!(mailbox_view_in(&path), r#"{"chip":"all","account":""}"#);
        keep_mailbox_view_in(&path, &sioul_core::mailbox::Ask { chip: sioul_core::mailbox::Chip::Spam, account: "work".into(), windows: 3, query: "x".into(), shown_resting: vec!["home".into()] });
        assert_eq!(mailbox_view_in(&path), r#"{"chip":"spam","account":"work"}"#);
        let _ = std::fs::remove_dir_all(&fresh);
    }

    /// A removal done now (a letter's task made again, `remove_now`): its
    /// "Undo" gone at once, the work done on a thread of its own, never the
    /// caller's (review of 8b57a35, L17), and only once.
    #[test]
    fn a_removal_done_now_runs_off_the_caller_s_thread() {
        let pending = Arc::new(Pending { work: Work::Skip { account: String::new(), file: PathBuf::from("/stand-in/task.ics"), start: 0 }, line: String::new(), taken: AtomicBool::new(false) });
        let (tell, heard) = std::sync::mpsc::channel();
        assert!(take_off_thread(&pending, move |_| tell.send(std::thread::current().id()).unwrap()));
        assert!(pending.taken.load(Ordering::Relaxed), "its Undo gone at once");
        let ran_on = heard.recv_timeout(Duration::from_secs(5)).expect("done");
        assert_ne!(ran_on, std::thread::current().id(), "never on the caller's thread");
        assert!(!take_off_thread(&pending, |_| panic!("done twice")), "taken once");
    }

    #[test]
    fn a_task_and_its_blocks_go_together_or_stay_together() {
        // A task deleted with its time block to come (`work::delete`, `blocks::with_task`): one "Undo" for both.
        let dir = std::env::temp_dir().join(format!("sioul-removals-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (task, block, gone) = (dir.join("task.ics"), dir.join("block.ics"), dir.join("gone.ics"));
        std::fs::write(&task, "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:t\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
        std::fs::write(&block, "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:b\r\nX-SIOUL-TASK:t\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let shared = Shared::default();
        let work = Work::Many(vec![Work::Remove { account: String::new(), file: task.clone() }, Work::Remove { account: String::new(), file: block.clone() }]);
        let pending = Arc::new(Pending { work, line: String::new(), taken: AtomicBool::new(false) });
        shared.pending.lock().unwrap().push(Arc::clone(&pending));
        // While "Undo" is offered: both out of view, both still there.
        let (hidden, _) = hidden_pim(&shared);
        assert!(hidden.contains(&task) && hidden.contains(&block), "{hidden:?}");
        assert!(touches_pim(&pending.work) && task.exists() && block.exists());
        // Undone (taken off the list, never done): both back, untouched.
        assert!(!pending.taken.swap(true, Ordering::Relaxed));
        shared.pending.lock().unwrap().clear();
        assert!(hidden_pim(&shared).0.is_empty() && task.exists() && block.exists());
        // Left to its ten seconds: both deleted; one gone already is said, the other still deleted.
        let late = Work::Many(vec![Work::Remove { account: String::new(), file: gone.clone() }, Work::Remove { account: String::new(), file: task.clone() }, Work::Remove { account: String::new(), file: block.clone() }]);
        let said = perform(&late, &shared);
        assert!(said.is_some_and(|line| line.contains("gone.ics")));
        assert!(!task.exists() && !block.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A deletion waits its ten seconds; meanwhile its event gained a guest
    /// (a sync brought its organizer's invitation): neither it nor one time of
    /// it is taken out, and what stopped it is said; your own event with an
    /// email alarm still is (`agenda::may_change`, on the file as it is then).
    #[test]
    fn a_removal_never_takes_out_the_meeting_its_event_became() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        use sioul_core::vdir::{self, Kind};
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Removals meanwhile", None, &["VEVENT"]).unwrap();
        let shared = Shared::default();
        let event = |extra: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:removal-1\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991028T090000Z\r\nDTEND:20991028T100000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Review\r\n{extra}END:VEVENT\r\nEND:VCALENDAR\r\n");
        let file = calendar.dir.join("review.ics");
        std::fs::write(&file, event("")).unwrap();
        // Asked while it was your own; a guest came before its ten seconds ended.
        let meeting = event("ORGANIZER:mailto:boss@example.org\r\nATTENDEE:mailto:me@example.org\r\n");
        std::fs::write(&file, &meeting).unwrap();
        let said = perform(&Work::Remove { account: String::new(), file: file.clone() }, &shared);
        assert!(said.is_some() && std::fs::read_to_string(&file).unwrap() == meeting, "{said:?}");
        let first = "2099-10-28T09:00:00Z".parse::<jiff::Timestamp>().unwrap().as_second();
        let said = perform(&Work::Skip { account: String::new(), file: file.clone(), start: first }, &shared);
        assert!(said.is_some() && std::fs::read_to_string(&file).unwrap() == meeting, "{said:?}");
        // Your own, an email alarm's recipient in it: taken out.
        std::fs::write(&file, event("BEGIN:VALARM\r\nACTION:EMAIL\r\nATTENDEE:mailto:paul@example.org\r\nSUMMARY:Soon\r\nDESCRIPTION:Soon\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\n")).unwrap();
        assert_eq!(perform(&Work::Remove { account: String::new(), file: file.clone() }, &shared), None);
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// A stand-in address, its Maildir under the test's own folders: an
    /// inbox, Sent and Gmail's "All Mail", with the messages a list must show
    /// alike whether it reads lines or parses: verified, from a list with a
    /// failed DMARC and a code in its text only, an answer with folded ids,
    /// one without a Message-ID, an attachment, one unread, one flagged.
    fn lines_account(name: &str) -> (Account, Vec<Folder>, PathBuf) {
        sioul_core::config::stand_ins_only();
        use sioul_core::folders::folder;
        let id = format!("lines-{name}-{}", std::process::id());
        let root = sioul_core::config::data_dir().join(&id);
        let _ = std::fs::remove_dir_all(&root);
        let config: Config = toml::from_str(&format!("[[account]]\nid = \"{id}\"\nkind = \"imap\"\naddress = \"me@example.org\"\nhost = \"imap.example.org\"\nmaildir = \"{}\"\n", root.display())).unwrap();
        let folders = vec![folder("INBOX", Some("/"), Some(Role::Inbox)), folder("[Gmail]/Sent Mail", Some("/"), Some(Role::Sent)), folder("[Gmail]/All Mail", Some("/"), Some(Role::All))];
        for f in &folders {
            for sub in ["tmp", "new", "cur"] {
                std::fs::create_dir_all(root.join(&f.local).join(sub)).unwrap();
            }
        }
        let put = |local: &str, uid: u32, flags: Option<&str>, raw: &str| {
            let (sub, name) = match flags {
                Some(flags) => ("cur", format!("17770000{uid:02}.U7-{uid}.sioul{}2,{flags}", maildir::INFO)),
                None => ("new", format!("17770000{uid:02}.U7-{uid}.sioul")),
            };
            std::fs::write(root.join(local).join(sub).join(name), raw.replace('\n', "\r\n")).unwrap();
        };
        let pass = "Authentication-Results: mx.example.org; dkim=pass header.d=example.org; spf=pass smtp.mailfrom=example.org; dmarc=pass header.from=example.org\n";
        let lunch = format!("{pass}Message-ID: <a1@example.org>\nFrom: Ana <ana@example.org>\nTo: Me <me@example.org>\nDate: Mon, 26 Oct 2026 09:00:00 +0100\nSubject: Lunch\nMIME-Version: 1.0\nContent-Type: multipart/mixed; boundary=b\n\n--b\nContent-Type: text/plain\n\nThe menu.\n--b\nContent-Type: application/pdf; name=menu.pdf\nContent-Disposition: attachment; filename=menu.pdf\nContent-Transfer-Encoding: base64\n\nJVBERi0=\n--b--\n");
        put("", 1, Some("S"), &lunch);
        put("", 2, Some("S"), "Authentication-Results: mx.example.org; dkim=fail header.d=service.example; dmarc=fail header.from=service.example\nMessage-ID: <c2@service.example>\nFrom: Service <no-reply@service.example>\nTo: me@example.org\nDate: Tue, 27 Oct 2026 10:00:00 +0100\nSubject: Your account\nList-Unsubscribe: <mailto:unsubscribe@service.example>\nPrecedence: bulk\n\nYour verification code is 482913. It expires in 10 minutes.\n");
        put("", 3, None, "Authentication-Results: mx.example.org;\n dkim=pass header.d=example.org;\n dmarc=pass header.from=example.org\nMessage-ID: <a3@example.org>\nFrom: Ana <ana@example.org>\nTo: Me <me@example.org>\nDate: Wed, 28 Oct 2026 11:00:00 +0100\nIn-Reply-To: <s1@example.org>\nReferences: <a1@example.org>\n <s1@example.org>\nSubject: Re: Lunch\n\nSee you.\n");
        put("", 4, Some("FS"), "From: =?UTF-8?Q?L=C3=A9a?= <lea@example.net>\nTo: me@example.org\nDate: Thu, 29 Oct 2026 12:00:00 +0100\nSubject: =?UTF-8?Q?R=C3=A9union?=\n\nNo id.\n");
        let sent = "Message-ID: <s1@example.org>\nFrom: Me <me@example.org>\nTo: Ana <ana@example.org>\nDate: Mon, 26 Oct 2026 13:00:00 +0100\nIn-Reply-To: <a1@example.org>\nReferences: <a1@example.org>\nSubject: Re: Lunch\n\nYes.\n";
        put(&folders[1].local, 5, Some("S"), sent);
        // "All Mail": the inbox's and Sent's copies, and one archived.
        put(&folders[2].local, 6, Some("S"), &lunch);
        put(&folders[2].local, 7, Some("S"), sent);
        put(&folders[2].local, 8, Some("S"), "Message-ID: <x8@example.org>\nFrom: Bo <bo@example.org>\nTo: me@example.org\nDate: Fri, 23 Oct 2026 08:00:00 +0100\nSubject: Archived\n\nOld.\n");
        (config.accounts[0].clone(), folders, root)
    }

    /// The same folder's list, read as Mail read it before its index: every
    /// message parsed, Gmail's archive by the ids parsed from the inbox and Sent.
    fn parsed_cards(account: &Account, folders: &[Folder], folder: &Folder, hidden: &BTreeSet<PathBuf>, threads: bool) -> (Vec<Card>, Option<Vec<Card>>) {
        let root = account.maildir_path();
        let mut elsewhere = BTreeSet::new();
        if folder.role == Role::All {
            elsewhere.extend(maildir::message_ids(&root));
            for sent in folders.iter().filter(|f| f.role == Role::Sent) {
                elsewhere.extend(maildir::message_ids(&root.join(&sent.local)));
            }
        }
        let cards = maildir::read_messages(&root.join(&folder.local))
            .into_iter()
            .filter(|c| c.path.as_ref().is_none_or(|p| !hidden.contains(p)))
            .filter(|c| c.message_id.as_ref().is_none_or(|id| !elsewhere.contains(id)))
            .collect();
        let sent = (threads && !matches!(folder.role, Role::Sent | Role::Drafts)).then(|| folders.iter().filter(|f| f.role == Role::Sent).flat_map(|f| maildir::read_messages(&root.join(&f.local))).collect());
        (cards, sent)
    }

    /// A folder's list made from its lines equals the one made by parsing
    /// every message: the inbox alone and by conversation, Sent, Gmail's
    /// archive alone and by conversation, a message waiting for its Undo left out.
    #[test]
    fn a_folder_list_from_lines_equals_the_one_parsed() {
        let (account, folders, root) = lines_account("equal");
        let words = sioul_core::words::current();
        let trusted = vec!["mx.example.org".to_string()];
        let hidden: BTreeSet<PathBuf> = [root.join("cur").join(format!("1777000004.U7-4.sioul{}2,FS", maildir::INFO))].into();
        let now = 1_793_000_000;
        let list = |folder: &Folder, (cards, sent): (Vec<Card>, Option<Vec<Card>>)| json(&view::folder(&account, folder, cards, &trusted, &words, true, "", tr(), now, sent));
        for _ in 0..2 {
            // Made, then read from the index.
            for (folder, threads) in [(&folders[0], false), (&folders[0], true), (&folders[1], false), (&folders[1], true), (&folders[2], false), (&folders[2], true)] {
                for hidden in [&BTreeSet::new(), &hidden] {
                    let lines = list(folder, folder_cards(&account, &folders, folder, hidden, threads));
                    let parsed = list(folder, parsed_cards(&account, &folders, folder, hidden, threads));
                    assert_eq!(lines, parsed, "{} by conversation: {threads}", folder.name);
                }
            }
        }
        // The list mail with its code in its text only: forged, as the Porch judges it, from its line too.
        let inbox = list(&folders[0], folder_cards(&account, &folders, &folders[0], &BTreeSet::new(), false));
        let view: serde_json::Value = serde_json::from_str(&inbox).unwrap();
        let service = view["items"].as_array().unwrap().iter().find(|i| i["address"] == "no-reply@service.example").unwrap();
        assert_eq!(service["trust_level"], "forged", "{service}");
        // Gmail's archive: only the message the inbox and Sent do not show.
        let archive: serde_json::Value = serde_json::from_str(&list(&folders[2], folder_cards(&account, &folders, &folders[2], &BTreeSet::new(), false))).unwrap();
        assert_eq!(archive["items"].as_array().unwrap().iter().map(|i| i["subject"].as_str().unwrap()).collect::<Vec<_>>(), ["Archived"]);
        let _ = std::fs::remove_dir_all(&root);
        if let Some(dir) = sioul_core::mailcache::account_dir(&account.id) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// A refresh of Mail parses no message: the folder's list, Gmail's
    /// archive and the conversations' answers come from the lines; a new
    /// message is read once, the one opened is read whole by the Reader only.
    #[test]
    fn a_refresh_of_mail_parses_no_message() {
        let (account, folders, root) = lines_account("refresh");
        let cases = [(&folders[0], true), (&folders[1], false), (&folders[2], true)];
        for (folder, threads) in cases {
            folder_cards(&account, &folders, folder, &BTreeSet::new(), threads);
        }
        maildir::parses_reset();
        for (folder, threads) in cases {
            let (cards, _) = folder_cards(&account, &folders, folder, &BTreeSet::new(), threads);
            assert!(!cards.is_empty());
        }
        assert_eq!(maildir::parses(), 0, "a refresh parses nothing");
        std::fs::write(root.join("new").join("1777000099.U7-99.sioul"), "Message-ID: <n9@example.org>\r\nFrom: bo@example.org\r\nSubject: New\r\n\r\nHi\r\n").unwrap();
        for (folder, threads) in cases {
            folder_cards(&account, &folders, folder, &BTreeSet::new(), threads);
        }
        assert_eq!(maildir::parses(), 1, "a new message read once");
        let _ = std::fs::remove_dir_all(&root);
        if let Some(dir) = sioul_core::mailcache::account_dir(&account.id) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// A measure, run by hand (`cargo test --release -p sioul-app
    /// measure_the_folders_on_a_large_stand_in -- --ignored --nocapture`, in
    /// folders made for tests): a stand-in Maildir of 20,000 invented
    /// messages over 20 folders (`SIOUL_MEASURE_MAILDIR`, else under
    /// /var/tmp), made once; then a refresh of Mail as before the folders'
    /// indexes (every message of the folder parsed, Gmail's archive parsing
    /// the inbox and Sent for their ids), the first index of every folder,
    /// and a refresh now: times and parses, and the look at everything links
    /// make (`MailIndex::build_ranked`).
    #[test]
    #[ignore]
    fn measure_the_folders_on_a_large_stand_in() {
        sioul_core::config::stand_ins_only();
        use sioul_core::folders::folder;
        let root = std::env::var_os("SIOUL_MEASURE_MAILDIR").map_or_else(|| PathBuf::from(format!("/var/tmp/sioul-measure-maildir-{}", std::process::id())), PathBuf::from);
        let mut folders = vec![folder("INBOX", Some("/"), Some(Role::Inbox)), folder("Sent", Some("/"), Some(Role::Sent)), folder("Archive", Some("/"), Some(Role::Archive)), folder("[Gmail]/All Mail", Some("/"), Some(Role::All))];
        for n in 1..=16 {
            folders.push(folder(&format!("Projects/Project {n:02}"), Some("/"), None));
        }
        let config: Config = toml::from_str(&format!("[[account]]\nid = \"measure\"\nkind = \"imap\"\naddress = \"me@example.org\"\nhost = \"imap.example.org\"\nmaildir = \"{}\"\n", root.display())).unwrap();
        let account = config.accounts[0].clone();
        let per = 1000;
        if !root.join("made").exists() {
            let body = |n: usize| -> String {
                match n % 20 {
                    // A newsletter: HTML, list headers, sometimes a code in its text.
                    0..=4 => format!("List-Id: <news{}.example.org>\r\nList-Unsubscribe: <mailto:u@example.org>\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<html><body>{}<p>Your code is {}.</p></body></html>\r\n", n % 7, "<p>News of the week, and more news.</p>".repeat(400), 100_000 + n),
                    // With a PDF attached.
                    5 => format!("MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nThe invoice.\r\n--b\r\nContent-Type: application/pdf; name=f.pdf\r\nContent-Disposition: attachment; filename=f.pdf\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n--b--\r\n", "JVBERi0xLjQK".repeat(8000)),
                    _ => format!("Content-Type: text/plain; charset=UTF-8\r\n\r\n{}\r\n", "A line of an ordinary message, about nothing in particular.\r\n".repeat(30)),
                }
            };
            for (f, folder) in folders.iter().enumerate() {
                for sub in ["tmp", "new", "cur"] {
                    std::fs::create_dir_all(root.join(&folder.local).join(sub)).unwrap();
                }
                for i in 0..per {
                    let n = f * per + i;
                    // "All Mail" holds the inbox's and Sent's ids again, and its own.
                    let id = if folder.role == Role::All && i < 600 { if i < 300 { i } else { per + i - 300 } } else { n };
                    let reply = if n % 3 == 0 && n > 0 { format!("In-Reply-To: <m{}@example.org>\r\n", n - 1) } else { String::new() };
                    let raw = format!("Authentication-Results: mx.example.org; dkim=pass header.d=example.org; dmarc=pass header.from=example.org\r\nMessage-ID: <m{id}@example.org>\r\nFrom: Sender {} <s{}@example.org>\r\nTo: me@example.org\r\nDate: Mon, {:02} Oct 2026 {:02}:{:02}:00 +0100\r\nSubject: Message {n} about the matter {}\r\n{reply}{}", n % 300, n % 300, 1 + n % 28, n % 24, n % 60, n % 50, body(n));
                    let flags = if i % 4 == 0 { "" } else { "S" };
                    std::fs::write(root.join(&folder.local).join("cur").join(format!("17700{n:05}.U7-{}.sioul{}2,{flags}", i + 1, maildir::INFO)), raw).unwrap();
                }
            }
            std::fs::write(root.join("made"), "").unwrap();
        }
        let size: u64 = folders.iter().flat_map(|f| std::fs::read_dir(root.join(&f.local).join("cur")).unwrap().filter_map(Result::ok)).map(|e| e.metadata().map_or(0, |m| m.len())).sum();
        println!("measure: {} messages over {} folders, {} MB", per * folders.len(), folders.len(), size / 1_000_000);
        let timed = |what: &str, run: &mut dyn FnMut() -> usize| {
            maildir::parses_reset();
            let at = std::time::Instant::now();
            let n = run();
            println!("measure: {what}: {:.0} ms, {} parses, {n} cards", at.elapsed().as_secs_f64() * 1000.0, maildir::parses());
        };
        let none = BTreeSet::new();
        let (inbox, all) = (&folders[0], &folders[3]);
        // Before: every refresh parses the folder whole (as at 4d81220).
        timed("before, a refresh of the inbox by conversation", &mut || parsed_cards(&account, &folders, inbox, &none, true).0.len());
        timed("before, a refresh of Gmail's archive", &mut || parsed_cards(&account, &folders, all, &none, false).0.len());
        timed("before, a refresh of a project folder", &mut || parsed_cards(&account, &folders, &folders[10], &none, false).0.len());
        // The first index of every folder: each message parsed once.
        let at = std::time::Instant::now();
        maildir::parses_reset();
        for f in &folders {
            sioul_core::mailcache::folder(&account.id, &root, &f.local);
        }
        let first = at.elapsed().as_secs_f64();
        println!("measure: the first index of every folder: {:.0} ms, {} parses, {:.0} ms per 1000 messages", first * 1000.0, maildir::parses(), first * 1000.0 * 1000.0 / (per * folders.len()) as f64);
        timed("now, a refresh of the inbox by conversation", &mut || folder_cards(&account, &folders, inbox, &none, true).0.len());
        timed("now, a refresh of Gmail's archive", &mut || folder_cards(&account, &folders, all, &none, false).0.len());
        timed("now, a refresh of a project folder", &mut || folder_cards(&account, &folders, &folders[10], &none, false).0.len());
        let roots = [sioul_core::mailindex::Root { account: account.id.clone(), path: root.clone(), folders: folders.clone() }];
        timed("now, the look at everything (MailIndex)", &mut || sioul_core::mailindex::MailIndex::build_ranked(&roots).len());
        let index: u64 = std::fs::read_dir(sioul_core::mailcache::account_dir(&account.id).unwrap()).unwrap().filter_map(Result::ok).map(|e| e.metadata().map_or(0, |m| m.len())).sum();
        println!("measure: the indexes hold {} KB", index / 1000);
        if std::env::var_os("SIOUL_MEASURE_MAILDIR").is_none() {
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    /// The mailbox (the unified mailbox's part 4) measured on a large
    /// stand-in: 20,000 invented messages over 20 folders of two addresses
    /// (10 each: the inbox, Sent, the archive, the junk, six of projects),
    /// sent over the last eight weeks, made under `SIOUL_MEASURE_MAILBOX`
    /// (else /var/tmp) and removed after. Said: the first index of every
    /// folder; the mailbox's page (two weeks, eight weeks, the Unread chip,
    /// a search), each made again warm, and its JSON; the first inbox's list
    /// as Mail shows a folder (`folder_cards`, `view::folder`).
    #[test]
    #[ignore]
    fn measure_the_mailbox_on_a_large_stand_in() {
        sioul_core::config::stand_ins_only();
        use sioul_core::folders::folder;
        let base = std::env::var_os("SIOUL_MEASURE_MAILBOX").map_or_else(|| PathBuf::from(format!("/var/tmp/sioul-measure-mailbox-{}", std::process::id())), PathBuf::from);
        let mut folders = vec![folder("INBOX", Some("/"), Some(Role::Inbox)), folder("Sent", Some("/"), Some(Role::Sent)), folder("Archive", Some("/"), Some(Role::Archive)), folder("Junk", Some("/"), Some(Role::Junk))];
        for n in 1..=6 {
            folders.push(folder(&format!("Project {n:02}"), Some("/"), None));
        }
        let ids = [format!("measure-a-{}", std::process::id()), format!("measure-b-{}", std::process::id())];
        let config: Config = toml::from_str(&ids.iter().enumerate().map(|(i, id)| format!("[[account]]\nid = \"{id}\"\nkind = \"imap\"\naddress = \"me{i}@example.org\"\nhost = \"imap.example.org\"\nmaildir = \"{}\"\n", base.join(id).display())).collect::<String>()).unwrap();
        let now = jiff::Timestamp::now().as_second();
        let per = 1000;
        if !base.join("made").exists() {
            for (a, account) in config.accounts.iter().enumerate() {
                let root = account.maildir_path();
                for (f, folder) in folders.iter().enumerate() {
                    for sub in ["tmp", "new", "cur"] {
                        std::fs::create_dir_all(root.join(&folder.local).join(sub)).unwrap();
                    }
                    for i in 0..per {
                        let n = (a * folders.len() + f) * per + i;
                        let sent = now - ((n as i64 * 241) % (56 * 86_400));
                        let date = jiff::Timestamp::from_second(sent).unwrap().strftime("%a, %d %b %Y %H:%M:%S +0000").to_string();
                        let (from, to) = if folder.role == Role::Sent { (format!("Me <me{a}@example.org>"), format!("s{}@example.org", n % 300)) } else { (format!("Sender {} <s{}@example.org>", n % 300, n % 300), format!("me{a}@example.org")) };
                        let list = if n % 5 == 0 { "List-Id: <news.example.org>\r\n" } else { "" };
                        let raw = format!("Authentication-Results: mx.example.org; dkim=pass header.d=example.org; dmarc=pass header.from=example.org\r\nMessage-ID: <m{n}@example.org>\r\nFrom: {from}\r\nTo: {to}\r\nDate: {date}\r\nSubject: Message {n} about the matter {}\r\n{list}Content-Type: text/plain; charset=UTF-8\r\n\r\n{}\r\n", n % 50, "A line of an ordinary message, about nothing in particular.\r\n".repeat(20));
                        let flags = if i % 4 == 0 { "" } else { "S" };
                        let path = root.join(&folder.local).join("cur").join(format!("{sent}.U7-{}.sioul{}2,{flags}", i + 1, maildir::INFO));
                        std::fs::write(&path, raw).unwrap();
                        std::fs::File::options().write(true).open(&path).unwrap().set_modified(UNIX_EPOCH + Duration::from_secs(sent as u64)).unwrap();
                    }
                }
            }
            std::fs::write(base.join("made"), "").unwrap();
        }
        println!("measure: {} messages over {} folders of {} addresses", per * folders.len() * ids.len(), folders.len() * ids.len(), ids.len());
        let at = std::time::Instant::now();
        for account in &config.accounts {
            for f in &folders {
                sioul_core::mailcache::folder(&account.id, &account.maildir_path(), &f.local);
            }
        }
        println!("measure: the first index of every folder: {:.0} ms", at.elapsed().as_secs_f64() * 1000.0);
        let addresses: Vec<sioul_core::mailbox::Address> = config.accounts.iter().enumerate().map(|(i, account)| sioul_core::mailbox::Address { account: account.clone(), folders: folders.clone(), trusted: vec!["mx.example.org".into()], tint: i as u8, resting: None }).collect();
        let mode = sioul_core::quiet::Mode { quiet: false, time: sioul_core::areas::Time::Work, week: sioul_core::areas::Week::default(), reason: sioul_core::quiet::Reason::Working, until: None, back: None, label: String::new() };
        let zone = sioul_core::zone::current();
        let none = BTreeSet::new();
        let timed = |what: &str, ask: sioul_core::mailbox::Ask| {
            let at = std::time::Instant::now();
            let world = sioul_core::mailbox::World::load(&config, sioul_core::attention::Now::of(&mode));
            let page = sioul_core::mailbox::page(&addresses, &world, &ask, &none, tr(), now, &zone);
            let text = json(&page);
            println!("measure: the mailbox, {what}: {:.0} ms, {} rows, {} KB of JSON", at.elapsed().as_secs_f64() * 1000.0, page.rows.len(), text.len() / 1000);
        };
        use sioul_core::mailbox::{Ask, Chip};
        timed("two weeks, the first time after the index", Ask::default());
        timed("two weeks", Ask::default());
        timed("eight weeks", Ask { windows: 4, ..Ask::default() });
        timed("the Unread chip", Ask { chip: Chip::Unread, ..Ask::default() });
        timed("a search", Ask { query: "matter 7".into(), ..Ask::default() });
        let account = &config.accounts[0];
        let at = std::time::Instant::now();
        let (cards, sent) = folder_cards(account, &folders, &folders[0], &none, false);
        let shown = view::folder(account, &folders[0], cards, &[], &sioul_core::words::Words::of(&config), false, "", tr(), now, sent);
        let text = json(&shown);
        println!("measure: Mail's first inbox, as a folder shows it: {:.0} ms, {} rows, {} KB of JSON", at.elapsed().as_secs_f64() * 1000.0, shown.items.len(), text.len() / 1000);
        if std::env::var_os("SIOUL_MEASURE_MAILBOX").is_none() {
            let _ = std::fs::remove_dir_all(&base);
        }
        for id in &ids {
            if let Some(dir) = sioul_core::mailcache::account_dir(id) {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    /// A message is looked up by `mailfind`, and its servers asked, only by the
    /// window's one opener (openmail.rs, `Sioul::find_message`, off the
    /// window's thread: the unified mailbox's part 2), the command line and
    /// agents; never by the phone's background step, nor by what it runs (the
    /// new mail's notices, the reminders, the alarms, the home card), which
    /// never reach the opener.
    #[test]
    fn only_the_window_s_opener_asks_the_servers_for_a_message() {
        // Its own search's `ask_servers` (mailsearch.rs) is no lookup by address: only mailfind's are named.
        let needles = [["mailfind::", "find"].concat(), ["mailfind::", "ask_servers"].concat(), ["find_or", "_ask"].concat()];
        let opener = ["openmail", "::"].concat();
        // The background step and what it runs, without the window.
        let background = ["steps.rs", "mailnote.rs", "eventalarms.rs", "alarms.rs", "remind.rs", "homecard.rs", "spam.rs", "filters.rs"];
        let mut sources = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
        let (mut read, mut calls) = (0, Vec::new());
        while let Some(dir) = sources.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    sources.push(path);
                } else if path.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    let name = path.file_name().unwrap().to_string_lossy().to_string();
                    read += 1;
                    if name != "openmail.rs" {
                        for needle in &needles {
                            assert!(!text.contains(needle.as_str()), "{} calls {needle}", path.display());
                        }
                    }
                    if text.contains(&opener) {
                        calls.push(name.clone());
                    }
                    if background.contains(&name.as_str()) {
                        assert!(!text.contains(&opener), "{name}, which the background step runs, reaches the opener");
                    }
                }
            }
        }
        assert!(read > 20, "{read} files read");
        calls.sort();
        assert_eq!(calls, ["backend.rs"], "the opener is the window's alone: called from its invokable");
        let backend = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/backend.rs")).unwrap();
        let invokable = backend.find("fn find_message(self: Pin<&mut Self>").expect("the window's invokable");
        let spawned = backend[invokable..].find("std::thread::spawn").expect("a thread of its own");
        let found = backend[invokable..].find(&[opener.as_str(), "find(&asked"].concat()).expect("the opener called");
        assert!(spawned < found, "the finding runs off the window's thread");
    }

    #[test]
    fn dropped_files_keep_their_names() {
        use crate::backend::local_path;
        assert_eq!(local_path("file:///home/me/Re%C3%A7u%20final.pdf"), PathBuf::from("/home/me/Reçu final.pdf"));
        assert_eq!(local_path("file:///tmp/100%"), PathBuf::from("/tmp/100%"));
    }
}

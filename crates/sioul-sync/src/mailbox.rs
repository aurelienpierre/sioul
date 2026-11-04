// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An account's folders, and what you do to its messages.
//!
//! Sioul writes to the server only when you act: a flag with `UID STORE`, a
//! move with `UID MOVE` (RFC 6851), else a copy, a deletion flag and `UID
//! EXPUNGE` (RFC 4315). The local copy follows: a flag renames its file; a
//! moved message's file goes into the other folder at once, under the
//! UIDVALIDITY and UID the server gave it there (its COPYUID, or APPENDUID
//! into another address, RFC 4315), so that it stays open and reachable and
//! the next sync of that folder finds it kept (`follow`, the unified
//! mailbox's §3.5). A server that does not say leaves the move noted
//! (`sioul_core::mailmoved`): "on its way" until that folder's next sync
//! brings the copy. Changes made elsewhere (read on the phone, deleted in the
//! webmail) come back at each sync (`reconcile`).

use crate::{SyncError, said};
use crate::imap::{self, COMMAND, Imap, Server};
use async_imap::imap_proto::rfc4315::UidSetMember;
use async_imap::imap_proto::{RequestId, Response, ResponseCode, Status};
use async_imap::types::NameAttribute;
use futures_util::StreamExt;
use sioul_core::card::ImapOrigin;
use sioul_core::config::{Account, state_dir};
use sioul_core::folders::{self, Folder, Role};
use sioul_core::maildir;
use sioul_core::spam::labels;
use sioul_core::{mailfind, mailmoved};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The folders the server lists, with what each is for. Gmail's "All Mail",
/// "Starred" and "Important" are views of the same messages: the first is kept
/// to archive into, the other two are left out (RFC 6154, RFC 8457).
pub async fn list(session: &mut Imap) -> Result<Vec<Folder>, SyncError> {
    let mut found = Vec::new();
    let mut names = imap::within(COMMAND, session.list(Some(""), Some("*"))).await?.map_err(imap::server)?;
    while let Some(name) = names.next().await {
        let name = name.map_err(imap::server)?;
        let attributes = name.attributes();
        if attributes.iter().any(|a| matches!(a, NameAttribute::NoSelect)) {
            continue;
        }
        let special = attributes.iter().find_map(|a| match a {
            NameAttribute::Sent => Some(Role::Sent),
            NameAttribute::Drafts => Some(Role::Drafts),
            NameAttribute::Junk => Some(Role::Junk),
            NameAttribute::Trash => Some(Role::Trash),
            NameAttribute::Archive => Some(Role::Archive),
            NameAttribute::All => Some(Role::All),
            NameAttribute::Flagged => Some(Role::Other),
            _ => None,
        });
        let view = attributes.iter().any(|a| match a {
            NameAttribute::Flagged => true,
            NameAttribute::Extension(name) => name.eq_ignore_ascii_case("\\Important"),
            _ => false,
        });
        if view {
            continue;
        }
        found.push(folders::folder(name.name(), name.delimiter(), special));
    }
    drop(names);
    found.sort_by(|a, b| (a.role, a.display.to_lowercase()).cmp(&(b.role, b.display.to_lowercase())));
    Ok(found)
}

/// Whether a folder is fetched: everything but views of other folders, except
/// where "All Mail" is the only archive there is (Gmail): it is fetched then,
/// and shown as the archive, without what the inbox and Sent already show.
/// Folders you chose not to keep stay on the server; the inbox is always kept.
/// A folder whose name would make it the Maildir itself or the folder above
/// ("INBOX." or "INBOX.." with "." between levels) is not fetched.
pub(crate) fn fetched(folder: &Folder, all: &[Folder], account: &Account) -> bool {
    (folder.role != Role::All || !all.iter().any(|f| f.role == Role::Archive))
        && (folder.role == Role::Inbox || (!account.skip_folders.contains(&folder.name) && own_dir(account, folder).is_some()))
}

/// A folder's own directory in the account's Maildir: one name in it, never
/// the Maildir itself nor what is above it, whatever the server named the folder.
pub(crate) fn own_dir(account: &Account, folder: &Folder) -> Option<PathBuf> {
    let mut parts = Path::new(&folder.local).components();
    match (parts.next(), parts.next()) {
        (Some(std::path::Component::Normal(_)), None) => Some(account.maildir_path().join(&folder.local)),
        _ => None,
    }
}

/// Makes a folder on the server (RFC 9051 §6.3.4), then lists them again.
/// The name you typed goes in modified UTF-7, as servers read folder names
/// ("Reçus" is "Re&AOc-us", RFC 3501 §5.1.3), the way they are listed back.
pub fn create_folder(account: &Account, password: &str, name: &str) -> Result<(), SyncError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(SyncError::Said(said::FOLDER_NAME, String::new()));
    }
    let name = folders::encode_utf7(name);
    let server = Server::of(account)?;
    crate::fetch::block_on(async {
        let mut session = imap::open(&server, password).await?;
        let made = imap::within(COMMAND, session.create(&name)).await?.map_err(imap::server);
        let listed = list(&mut session).await;
        let _ = session.logout().await;
        made?;
        save(&account.id, &listed?)
    })
}

/// The folder still holds messages: said by the window in your language.
pub const NOT_EMPTY: &str = "not-empty";

/// Takes a folder off the server (RFC 9051 §6.3.5): only an empty one, and
/// never one with a purpose (the inbox, Sent, Drafts, the trash, junk, the
/// archive), so no message is ever lost this way.
pub fn delete_folder(account: &Account, password: &str, name: &str) -> Result<(), SyncError> {
    let folder = folders(&account.id).into_iter().find(|f| f.name == name).ok_or_else(|| SyncError::Said(said::NO_FOLDER, name.to_string()))?;
    if folder.role != Role::Other {
        return Err(SyncError::Said(said::PURPOSE, folder.display.clone()));
    }
    let server = Server::of(account)?;
    crate::fetch::block_on(async {
        let mut session = imap::open(&server, password).await?;
        let status = imap::within(COMMAND, session.status(name, "(MESSAGES)")).await?.map_err(imap::server)?;
        if status.exists > 0 {
            let _ = session.logout().await;
            return Err(SyncError::Message(NOT_EMPTY.into()));
        }
        let deleted = imap::within(COMMAND, session.delete(name)).await?.map_err(imap::server);
        let listed = list(&mut session).await;
        let _ = session.logout().await;
        deleted?;
        save(&account.id, &listed?)
    })?;
    // Its empty copy here goes too, and where its sync stopped.
    if let Some(dir) = own_dir(account, &folder) {
        let _ = std::fs::remove_dir_all(dir);
    }
    crate::fetch::forget_folder_state(account, name)
}

/// A folder no longer kept here: its copy goes (the server keeps it all), and
/// where its sync stopped is forgotten, so keeping it again brings it back whole.
pub fn forget_folder(account: &Account, name: &str) -> Result<(), SyncError> {
    // Nothing on its way there will come.
    let _ = mailmoved::forget_account(&account.id, Some(name));
    if let Some(root) = folders(&account.id).into_iter().find(|f| f.name == name && f.role != Role::Inbox).and_then(|f| own_dir(account, &f)) {
        for sub in ["cur", "new"] {
            for entry in std::fs::read_dir(root.join(sub)).into_iter().flatten().filter_map(Result::ok) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    crate::fetch::forget_folder_state(account, name)
}

fn folders_path(account: &str) -> PathBuf {
    folders::saved_path(account)
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Saved {
    #[serde(default, rename = "folder")]
    folders: Vec<Folder>,
}

/// Keeps the list for the window and for actions, which need to know where the trash is.
pub(crate) fn save(account: &str, list: &[Folder]) -> Result<(), SyncError> {
    let path = folders_path(account);
    let fail = |e: std::io::Error| SyncError::Disk(format!("{}: {e}", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let text = toml::to_string(&Saved { folders: list.to_vec() }).map_err(|e| SyncError::Disk(e.to_string()))?;
    std::fs::write(&path, text).map_err(fail)
}

/// The account's folders as last listed: the inbox alone before the first sync.
pub fn folders(account: &str) -> Vec<Folder> {
    let saved = folders::saved(account);
    if saved.is_empty() { vec![folders::folder("INBOX", None, None)] } else { saved }
}

/// The folder a stored message belongs to, from where its file is.
pub fn folder_of(account: &Account, file: &Path) -> Option<Folder> {
    let directory = file.parent()?.parent()?;
    let root = account.maildir_path();
    let all = folders(&account.id);
    if let Some(folder) = all.iter().find(|f| root.join(&f.local) == directory) {
        return Some(folder.clone());
    }
    // Each path as the system resolves it, where a folder sits behind a link (as `Config::account_of`).
    let directory = std::fs::canonicalize(directory).ok()?;
    all.into_iter().find(|f| std::fs::canonicalize(root.join(&f.local)).is_ok_and(|d| d == directory))
}

/// Brings back what changed on the server since the last sync, for the
/// messages kept here: their flags, and those deleted or moved elsewhere,
/// whose local copies go. The folder must be open (`EXAMINE` or `SELECT`).
pub(crate) async fn reconcile(session: &mut Imap, root: &Path, validity: u32) -> Result<(), SyncError> {
    let local = local_files(root);
    let uids: BTreeSet<u32> = local.iter().filter(|(o, _)| o.validity == validity).map(|(o, _)| o.uid).collect();
    if uids.is_empty() {
        return Ok(());
    }
    let mut server: BTreeMap<u32, String> = BTreeMap::new();
    {
        let mut fetches = imap::within(COMMAND, session.uid_fetch(ranges(&uids), "(UID FLAGS)")).await?.map_err(imap::server)?;
        while let Some(fetch) = fetches.next().await {
            let fetch = fetch.map_err(imap::server)?;
            if let Some(uid) = fetch.uid {
                server.insert(uid, crate::fetch::maildir_flags(fetch.flags()));
            }
        }
    }
    for (origin, path) in local.into_iter().filter(|(o, _)| o.validity == validity) {
        match server.get(&origin.uid) {
            None => {
                let _ = std::fs::remove_file(&path);
            }
            Some(flags) if sorted(flags) != sorted(&maildir::flags_of(&path)) => {
                let _ = maildir::set_flags(&path, flags);
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn sorted(flags: &str) -> String {
    let mut letters: Vec<char> = flags.chars().collect();
    letters.sort_unstable();
    letters.dedup();
    letters.into_iter().collect()
}

/// The stored messages of a Maildir folder, with where each sits on the server.
fn local_files(root: &Path) -> Vec<(ImapOrigin, PathBuf)> {
    ["new", "cur"]
        .iter()
        .filter_map(|sub| std::fs::read_dir(root.join(sub)).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter_map(|p| Some((maildir::origin_of(&p)?, p)))
        .collect()
}

/// "1:5,7,9:12": a UID set as short as it gets.
pub(crate) fn ranges(uids: &BTreeSet<u32>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut iter = uids.iter().copied().peekable();
    while let Some(start) = iter.next() {
        let mut end = start;
        while iter.peek() == Some(&(end + 1)) {
            end += 1;
            iter.next();
        }
        parts.push(if start == end { start.to_string() } else { format!("{start}:{end}") });
    }
    parts.join(",")
}

/// What you do to a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Read (true) or unread (false).
    Read(bool),
    /// Flagged to follow up, or not.
    Flag(bool),
    /// Into another folder, by its server name.
    Move(String),
    /// To the trash; from the trash, gone for good.
    Trash,
    /// To the junk folder, marked `$Junk` for the server's filter to learn.
    Junk,
    /// Out of the inbox, into the archive (Gmail: into "All Mail").
    Archive,
    /// Out of the junk folder, back into the inbox, marked `$NotJunk`.
    NotJunk,
    /// Not spam, said of mail set aside as spam or flagged by your own
    /// filter: marked `$NotJunk` (and no longer `$Junk`), it stays where it
    /// is, and the Porch puts it back in its lane.
    NotSpam,
    /// Into the account's Junk folder by your own spam filter (the matrix's
    /// "Move to spam", for a message of this class): no keyword set, no line
    /// in the label log, which is yours alone; the move goes into this
    /// device's log of what the filter moved (`spam::labels::Moved`), which
    /// the Porch's lane of its catches and the training read (in the Junk
    /// folder, learned as spam until you say otherwise).
    Filtered(sioul_core::spam::Class),
    /// Proven mail your provider put in its Junk folder, brought back into
    /// the inbox by the matrix's row "Proven mail your provider called spam"
    /// (`spam::vouched`, part 2 of 4 November 2026): no keyword set and no
    /// line in the label log (nobody said anything); the row's line goes
    /// into this device's log of moves (`labels::Back`), so that every device
    /// knows it and none brings it back twice.
    BroughtBack,
}

/// Where moved messages' files went here: each file before, and after.
pub type Moved = Vec<(PathBuf, PathBuf)>;

/// Does `action` to the message stored at `file`, on the server, then here.
/// What it says of the message, junk or not, goes into this device's label
/// log (`spam::labels`) once the server has taken it; what your own filter
/// moved, into this device's log of its moves. Moved, its file follows it
/// (`follow`); returns where it went.
pub fn act(account: &Account, password: &str, file: &Path, action: &Action) -> Result<Moved, SyncError> {
    let server = Server::of(account)?;
    let origin = maildir::origin_of(file).ok_or_else(|| SyncError::Said(said::NOT_FETCHED, String::new()))?;
    let folder = folder_of(account, file).ok_or_else(|| SyncError::Said(said::UNKNOWN_FOLDER, String::new()))?;
    if *action == Action::Archive && matches!(folder.role, Role::Archive | Role::All) {
        return Ok(Vec::new());
    }
    // Already in a Junk folder: nothing for the filter to move; brought back only from one.
    if matches!(action, Action::Filtered(_)) && folder.role == Role::Junk {
        return Ok(Vec::new());
    }
    if *action == Action::BroughtBack && folder.role != Role::Junk {
        return Ok(Vec::new());
    }
    // Read here before the message leaves this folder; written once the server took the act.
    let label = said(action).and_then(|source| labels::Entry::of_file(&account.id, &folder.name, file, source));
    let moved = match action {
        Action::Filtered(class) => labels::Moved::of_file(&account.id, &folder.name, file, *class),
        _ => None,
    };
    let back = (*action == Action::BroughtBack).then(|| labels::Back::of_file(&account.id, &folder.name, file, labels::BackKind::Brought)).flatten();
    // Its address, read before it moves: noted as on its way before the command (`Intent`).
    let address = kept_with(action, "", folder.role).is_none().then(|| mailfind::address_of_file(&account.id, file)).flatten();
    let mut intent = Intent::new(&account.id, address.iter().cloned().collect());
    let went = crate::fetch::block_on(async {
        let mut session = imap::open(&server, password).await?;
        let result = act_in(&mut session, account, &folder, origin, action, &mut |target| intent.note(target)).await;
        let _ = session.logout().await;
        result
    });
    let went = intent.after(went)?;
    // Done on the server: a log that cannot be written is said, after the rest is done here.
    let root = labels::state();
    let logged = match (&label, &moved, &back) {
        (Some(label), _, _) => labels::append_to(&labels::own_log(&root, &device()), label),
        (_, Some(moved), _) => labels::append_to(&labels::own_moved_log(&root, &device()), moved),
        (_, _, Some(back)) => labels::append_to(&labels::own_moved_log(&root, &device()), back),
        _ => Ok(()),
    }
    .map_err(SyncError::Disk);
    // A sync may have renamed the file meanwhile; gone, there is nothing left
    // to do here (moved, its note waits for the copy).
    let Some(file) = maildir::locate(file) else { return logged.map(|()| Vec::new()) };
    let flags = maildir::flags_of(&file);
    let mut followed = Vec::new();
    match (kept_with(action, &flags, folder.role), went) {
        (Some(flags), _) => {
            maildir::set_flags(&file, &flags).map_err(|e| SyncError::Disk(e.to_string()))?;
        }
        (None, Some(went)) => {
            if let Some(now) = follow(account, &file, &went.folder, went.place_of(origin.uid), &moved_flags(action, &flags), address.as_deref()) {
                followed.push((file, now));
            }
        }
        (None, None) => {
            let _ = std::fs::remove_file(&file);
        }
    }
    logged.map(|()| followed)
}

/// The letters a moved message's file keeps in its new folder, as the act
/// left the server's copy: said spam, `$Junk`'s; not junk, `$NotJunk`'s; the
/// rest as they were (the next sync brings back what a server did not keep).
fn moved_flags(action: &Action, flags: &str) -> String {
    let without = |letters: &[char]| flags.chars().filter(|c| !letters.contains(c)).collect::<String>();
    match action {
        Action::Junk => format!("{}{}", without(&[maildir::JUNK, maildir::NOT_JUNK]), maildir::JUNK),
        Action::NotJunk => format!("{}{}", without(&[maildir::JUNK, maildir::NOT_JUNK]), maildir::NOT_JUNK),
        _ => flags.to_string(),
    }
}

/// The messages of one act noted as on their way (`mailmoved`) before its
/// move's command is sent, so that a move whose answer is lost is still said;
/// the notes go again when the server refused (nothing moved), and stay when
/// the answer did not come (the move may have been done).
pub(crate) struct Intent {
    account: String,
    addresses: Vec<String>,
    noted: bool,
}

impl Intent {
    pub(crate) fn new(account: &str, addresses: Vec<String>) -> Intent {
        Intent { account: account.to_string(), addresses, noted: false }
    }

    /// Before the command that moves them into `target` (its server name).
    pub(crate) fn note(&mut self, target: &str) {
        if !self.addresses.is_empty() {
            self.noted |= mailmoved::note(&self.account, &self.addresses, target, jiff::Timestamp::now().as_second()).is_ok();
        }
    }

    /// The act's result, the notes dropped when the server refused it.
    pub(crate) fn after<T>(&self, result: Result<T, SyncError>) -> Result<T, SyncError> {
        if let Err(e) = &result
            && self.noted
            && !matches!(e, SyncError::Network(_) | SyncError::Tls(_))
        {
            let _ = mailmoved::forget(&self.addresses);
        }
        result
    }
}

/// What becomes of the local copy once the server took `action` on it in a
/// folder of `role`: its letters now, or none when it left this folder (the
/// next sync brings the copy into the other). Said not spam, it keeps its
/// keyword here as on the server; said spam in a Junk folder, it stays there.
fn kept_with(action: &Action, flags: &str, role: Role) -> Option<String> {
    match action {
        Action::Read(read) => Some(if *read { format!("{flags}S") } else { flags.replace('S', "") }),
        Action::Flag(flag) => Some(if *flag { format!("{flags}F") } else { flags.replace('F', "") }),
        Action::NotSpam => Some(format!("{}{}", flags.replace(maildir::JUNK, ""), maildir::NOT_JUNK)),
        Action::Junk if role == Role::Junk => Some(format!("{}{}", flags.replace(maildir::NOT_JUNK, "").replace(maildir::JUNK, ""), maildir::JUNK)),
        Action::Move(_) | Action::Trash | Action::Junk | Action::Archive | Action::NotJunk | Action::Filtered(_) | Action::BroughtBack => None,
    }
}

/// This device's name in the sharing (a UUID made once, `share::Here`): its
/// label log and its logs of what the filter moved or flagged are its own files.
fn device() -> String {
    crate::share::Here::load(&state_dir()).id
}

/// Writes into the spam filter's label log what you said of the message
/// stored at `file`, without acting on it there: a sender blocked from it.
pub fn label(account: &Account, file: &Path, source: labels::Source) -> Result<(), SyncError> {
    let folder = folder_of(account, file).ok_or_else(|| SyncError::Said(said::UNKNOWN_FOLDER, String::new()))?;
    let entry = labels::Entry::of_file(&account.id, &folder.name, file, source).ok_or_else(|| SyncError::Said(said::NOT_FETCHED, String::new()))?;
    labels::append_to(&labels::own_log(&labels::state(), &device()), &entry).map_err(SyncError::Disk)
}

/// Writes into this device's log of what your own filter flagged where it
/// is (`spam::labels::Flagged`) the message stored at `file`, judged
/// `class`, as it arrived: nothing changes on the server nor here. The
/// training learns from it as it is until you say otherwise: a probable spam
/// as spam, a maybe spam not at all.
pub fn flagged(account: &Account, file: &Path, class: sioul_core::spam::Class) -> Result<(), SyncError> {
    let folder = folder_of(account, file).ok_or_else(|| SyncError::Said(said::UNKNOWN_FOLDER, String::new()))?;
    let flag = labels::Flagged::of_file(&account.id, &folder.name, file, class).ok_or_else(|| SyncError::Said(said::NOT_FETCHED, String::new()))?;
    labels::append_to(&labels::own_flagged_log(&labels::state(), &device()), &flag).map_err(SyncError::Disk)
}

/// Writes into this device's log of moves that the matrix's row "Proven
/// mail your provider called spam" leaves the message stored at `file`, in
/// a Junk folder of `account`, where it is: you pressed Undo as it was
/// being brought back (`labels::BackKind::Left`). Nothing changes on the
/// server nor here; no device brings it back again.
pub fn left_in_junk(account: &Account, file: &Path) -> Result<(), SyncError> {
    let folder = folder_of(account, file).ok_or_else(|| SyncError::Said(said::UNKNOWN_FOLDER, String::new()))?;
    let back = labels::Back::of_file(&account.id, &folder.name, file, labels::BackKind::Left).ok_or_else(|| SyncError::Said(said::NOT_FETCHED, String::new()))?;
    labels::append_to(&labels::own_moved_log(&labels::state(), &device()), &back).map_err(SyncError::Disk)
}

/// What an act says of a message, for the label log: none for those that
/// say nothing of spam, nor for your own filter's move (its own log's).
fn said(action: &Action) -> Option<labels::Source> {
    match action {
        Action::Junk => Some(labels::Source::Junk),
        Action::NotJunk => Some(labels::Source::NotJunk),
        Action::NotSpam => Some(labels::Source::NotSpam),
        Action::Read(_) | Action::Flag(_) | Action::Move(_) | Action::Trash | Action::Archive | Action::Filtered(_) | Action::BroughtBack => None,
    }
}

/// Moves the message stored at `file` into `folder` of another account: a
/// copy is appended there, its flags kept, and only once that server has it
/// is the original taken off the first one. Nothing is lost on the way: if the
/// copy fails, the original stays; if removing the original fails, both remain.
/// Done, its file follows it into `to`'s Maildir, under the APPENDUID that
/// server gave (`follow`); returns where it went.
pub fn move_across(from: &Account, from_password: &str, file: &Path, to: &Account, to_password: &str, folder: &str) -> Result<Moved, SyncError> {
    let raw = std::fs::read(file).map_err(|e| SyncError::Disk(format!("{}: {e}", file.display())))?;
    let origin = maildir::origin_of(file).ok_or_else(|| SyncError::Said(said::NOT_FETCHED, String::new()))?;
    let source = folder_of(from, file).ok_or_else(|| SyncError::Said(said::UNKNOWN_FOLDER, String::new()))?;
    let flags = maildir::flags_of(file);
    // Noted under the address it goes into, as its copy will be found there.
    let mut intent = Intent::new(&to.id, vec![mailfind::address_of(&to.id, &raw)]);
    let result = append_then_remove(from, from_password, &source, origin, &raw, &flags, to, to_password, folder, &mut intent);
    let appended = intent.after(result)?;
    let Some(file) = maildir::locate(file) else { return Ok(Vec::new()) };
    let address = intent.addresses.first().map(String::as_str);
    Ok(follow(to, &file, folder, appended, &flags, address).map(|now| vec![(file, now)]).unwrap_or_default())
}

/// The same for a message seen on its server only (a search's): read whole
/// from its folder first, its flags with it, then copied and taken off; its
/// copy written into `to`'s Maildir at once, under the APPENDUID that server
/// gave (`arrive`), as its sync would write it.
pub fn move_across_from_server(from: &Account, from_password: &str, source: &str, origin: ImapOrigin, to: &Account, to_password: &str, folder: &str) -> Result<(), SyncError> {
    let source = folders(&from.id).into_iter().find(|f| f.name == source).unwrap_or_else(|| folders::folder(source, None, None));
    let server = Server::of(from)?;
    let (raw, flags) = crate::fetch::block_on(async {
        let mut session = imap::open(&server, from_password).await?;
        let result = read_whole(&mut session, &source, origin).await;
        let _ = session.logout().await;
        result
    })?;
    let mut intent = Intent::new(&to.id, vec![mailfind::address_of(&to.id, &raw)]);
    let result = append_then_remove(from, from_password, &source, origin, &raw, &flags, to, to_password, folder, &mut intent);
    let appended = intent.after(result)?;
    arrive(to, folder, appended, raw, flags, intent.addresses.first().map(String::as_str));
    Ok(())
}

/// A message of `folder`, whole, with its flags as Maildir letters.
async fn read_whole(session: &mut Imap, folder: &Folder, origin: ImapOrigin) -> Result<(Vec<u8>, String), SyncError> {
    let examined = imap::within(COMMAND, session.examine(&folder.name)).await?.map_err(imap::server)?;
    if examined.uid_validity.unwrap_or(0) != origin.validity {
        return Err(SyncError::Said(said::RENUMBERED, String::new()));
    }
    let mut fetches = imap::within(COMMAND, session.uid_fetch(origin.uid.to_string(), "(UID FLAGS BODY.PEEK[])")).await?.map_err(imap::server)?;
    let mut found = None;
    while let Some(fetch) = fetches.next().await {
        let fetch = fetch.map_err(imap::server)?;
        if fetch.uid == Some(origin.uid)
            && let Some(body) = fetch.body()
        {
            found = Some((body.to_vec(), crate::fetch::maildir_flags(fetch.flags())));
        }
    }
    found.ok_or_else(|| SyncError::Said(said::GONE, String::new()))
}

/// Copies `raw` into `folder` of `to`, its flags kept (Maildir letters), and
/// only once that server has it takes the original off `source` of `from`.
/// Returns where `to`'s server put the copy (APPENDUID), when it said; the
/// move noted as on its way (`intent`) before the copy is sent.
#[allow(clippy::too_many_arguments)]
fn append_then_remove(from: &Account, from_password: &str, source: &Folder, origin: ImapOrigin, raw: &[u8], letters: &str, to: &Account, to_password: &str, folder: &str, intent: &mut Intent) -> Result<Option<ImapOrigin>, SyncError> {
    let flags: Vec<&str> = [('S', "\\Seen"), ('F', "\\Flagged"), ('R', "\\Answered")].into_iter().filter(|(c, _)| letters.contains(*c)).map(|(_, f)| f).collect();
    let flags = format!("({})", flags.join(" "));
    let target = Server::of(to)?;
    let appended = crate::fetch::block_on(async {
        let mut session = imap::open(&target, to_password).await?;
        let uidplus = uidplus(&imap::within(COMMAND, session.capabilities()).await?.map_err(imap::server)?);
        intent.note(folder);
        let result = append(&mut session, folder, &flags, raw).await.map(|answered| match answered {
            Some(Answered::Appended(place)) if uidplus => Some(place),
            _ => None,
        });
        let _ = session.logout().await;
        result
    })?;
    let server = Server::of(from)?;
    crate::fetch::block_on(async {
        let mut session = imap::open(&server, from_password).await?;
        let result = remove_in(&mut session, source, origin).await;
        let _ = session.logout().await;
        result
    })?;
    Ok(appended)
}

/// A message `to`'s server holds now at `place` of `folder` (Sioul put it
/// there): written into that folder here at once, checked as a sync writes
/// mail, under `to`'s fetch lock, unless a copy is there already; when the
/// server did not say where, or the folder is not kept here, nothing is
/// written (the note waits for the sync, or goes).
fn arrive(to: &Account, folder: &str, place: Option<ImapOrigin>, raw: Vec<u8>, flags: String, address: Option<&str>) {
    let forget = || {
        if let Some(address) = address {
            let _ = mailmoved::forget(&[address.to_string()]);
        }
    };
    let Some(dir) = kept_dir(to, folder) else { return forget() };
    let Some(place) = place else { return };
    let Some(_lock) = crate::fetch::lock_within(&crate::fetch::lock_path(to), lock_wait()) else { return };
    if maildir::holding(&dir, place).is_some() {
        return forget();
    }
    let verifier = crate::verify::Verifier::new();
    let written = crate::fetch::block_on(crate::fetch::store_checked(vec![crate::fetch::Arrived::appended(place.uid, flags, raw)], place.validity, &dir, verifier.as_ref()));
    if written.is_ok() {
        forget();
    }
}

/// UIDs per command, at most: a long set stays well under the line servers take.
const SET: usize = 200;

/// The same act on several messages of one account, in one connection: a
/// UID set per folder (RFC 9051 §6.4.9). `files` are kept here; `remote` are
/// seen on the server only (a search's), by their folder's server name. For
/// acts that say nothing of spam (reading, flags, moves, archiving, the
/// trash); the others go one at a time through `act`, for the spam filter's
/// logs. The local copies follow as with `act` (a moved one's file into its
/// new folder, `follow`). A folder that fails is said (the first problem),
/// the others are still done. Returns where moved files went.
pub fn act_many(account: &Account, password: &str, files: &[PathBuf], remote: &[(String, ImapOrigin)], action: &Action) -> Result<Moved, SyncError> {
    if said(action).is_some() || matches!(action, Action::Filtered(_) | Action::BroughtBack) {
        let (mut first, mut followed) = (None, Vec::new());
        for file in files {
            match act(account, password, file, action) {
                Ok(moved) => followed.extend(moved),
                Err(e) => {
                    first.get_or_insert(e);
                }
            }
        }
        if !remote.is_empty() {
            first.get_or_insert(SyncError::Said(said::OPEN_FIRST, String::new()));
        }
        return first.map_or(Ok(followed), Err);
    }
    let known = folders(&account.id);
    // By folder, then UIDVALIDITY: the UIDs, and the files that follow.
    let mut groups: BTreeMap<String, (Folder, BTreeMap<u32, (BTreeSet<u32>, Vec<PathBuf>)>)> = BTreeMap::new();
    let mut add = |folder: Folder, origin: ImapOrigin, file: Option<&PathBuf>| {
        let entry = groups.entry(folder.name.clone()).or_insert_with(|| (folder, BTreeMap::new()));
        let (uids, files) = entry.1.entry(origin.validity).or_default();
        uids.insert(origin.uid);
        files.extend(file.cloned());
    };
    let mut first: Option<SyncError> = None;
    for file in files {
        match (maildir::origin_of(file), folder_of(account, file)) {
            (Some(origin), Some(folder)) => add(folder, origin, Some(file)),
            _ => {
                first.get_or_insert(SyncError::Said(said::NOT_FETCHED, String::new()));
            }
        }
    }
    for (name, origin) in remote {
        let folder = known.iter().find(|f| f.name == *name).cloned().unwrap_or_else(|| folders::folder(name, None, None));
        add(folder, *origin, None);
    }
    // Archived already: nothing to do (as `act`).
    groups.retain(|_, (folder, _)| !(*action == Action::Archive && matches!(folder.role, Role::Archive | Role::All)));
    if groups.is_empty() {
        return first.map_or(Ok(Vec::new()), Err);
    }
    // Each file's address, read before it moves: noted as on its way before each command (`Intent`).
    let moving = kept_with(action, "", Role::Other).is_none();
    let addresses: BTreeMap<PathBuf, String> = if moving { files.iter().filter_map(|f| Some((f.clone(), mailfind::address_of_file(&account.id, f)?))).collect() } else { BTreeMap::new() };
    let server = Server::of(account)?;
    type Done = (Role, Vec<(PathBuf, u32)>, Option<Went>);
    let done: Vec<Done> = crate::fetch::block_on(async {
        let mut session = imap::open(&server, password).await?;
        let mut done = Vec::new();
        for (folder, by_validity) in groups.values() {
            for (validity, (uids, files)) in by_validity {
                let numbered: Vec<(PathBuf, u32)> = files.iter().filter_map(|f| Some((f.clone(), maildir::origin_of(f)?.uid))).collect();
                let uids: Vec<u32> = uids.iter().copied().collect();
                for chunk in uids.chunks(SET) {
                    let set = ranges(&chunk.iter().copied().collect::<BTreeSet<u32>>());
                    let these: Vec<(PathBuf, u32)> = numbered.iter().filter(|(_, uid)| chunk.contains(uid)).cloned().collect();
                    let mut intent = Intent::new(&account.id, these.iter().filter_map(|(f, _)| addresses.get(f).cloned()).collect());
                    let result = act_set(&mut session, account, folder, *validity, &set, action, &mut |target| intent.note(target)).await;
                    match intent.after(result) {
                        Ok(mut went) => {
                            // A set's places, each confirmed by its Message-ID (`confirmed`).
                            if let Some(went) = went.as_mut()
                                && let Some(placed) = went.placed.take()
                            {
                                let sources: BTreeMap<u32, String> = these.iter().filter_map(|(f, uid)| Some((*uid, addresses.get(f)?.clone()))).collect();
                                went.placed = Some(confirmed(&mut session, &account.id, &went.folder, placed, &sources).await);
                            }
                            done.push((folder.role, these, went))
                        }
                        Err(e) => {
                            first.get_or_insert(e);
                            break;
                        }
                    }
                }
            }
        }
        let _ = session.logout().await;
        Ok(done)
    })?;
    // Done on the server: the copies here follow, a moved set into its folder at once (`follow_all`).
    let mut followed = Vec::new();
    for (role, files, went) in done {
        let mut moving = Vec::new();
        for (asked, uid) in files {
            let Some(file) = maildir::locate(&asked) else { continue };
            let flags = maildir::flags_of(&file);
            match (kept_with(action, &flags, role), &went) {
                (Some(flags), _) => {
                    let _ = maildir::set_flags(&file, &flags);
                }
                (None, Some(went)) => moving.push(Following { place: went.place_of(uid), flags: moved_flags(action, &flags), address: addresses.get(&asked).cloned(), file }),
                (None, None) => {
                    let _ = std::fs::remove_file(&file);
                }
            }
        }
        if let Some(went) = went.filter(|_| !moving.is_empty()) {
            followed.extend(follow_all(account, &went.folder, moving));
        }
    }
    first.map_or(Ok(followed), Err)
}

/// Takes a message off its folder for good: marked deleted, then expunged.
async fn remove_in(session: &mut Imap, folder: &Folder, origin: ImapOrigin) -> Result<(), SyncError> {
    let selected = imap::within(COMMAND, session.select(&folder.name)).await?.map_err(imap::server)?;
    if selected.uid_validity.unwrap_or(0) != origin.validity {
        return Err(SyncError::Said(said::RENUMBERED, String::new()));
    }
    let uid = origin.uid.to_string();
    let capabilities = imap::within(COMMAND, session.capabilities()).await?.map_err(imap::server)?;
    store(session, &uid, "+FLAGS.SILENT (\\Deleted)").await?;
    expunge(session, &uid, capabilities.has_str("UIDPLUS")).await
}

async fn act_in(session: &mut Imap, account: &Account, folder: &Folder, origin: ImapOrigin, action: &Action, intent: &mut dyn FnMut(&str)) -> Result<Option<Went>, SyncError> {
    act_set(session, account, folder, origin.validity, &origin.uid.to_string(), action, intent).await
}

/// Where an act moved messages: the folder (its server name), and where the
/// server put each there, when it said (COPYUID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Went {
    pub folder: String,
    pub placed: Option<Placed>,
}

impl Went {
    /// The new place of the message that was at `uid`, when the server said it.
    pub(crate) fn place_of(&self, uid: u32) -> Option<ImapOrigin> {
        let placed = self.placed.as_ref()?;
        placed.uids.get(&uid).map(|&uid| ImapOrigin { validity: placed.validity, uid })
    }
}

/// `action` on the messages of `uid`, a UID or a set of them ("4,7:9"), in
/// `folder`, whose UIDVALIDITY must still be `validity`. A move tells
/// `intent` its target (its server name) before its command is sent, and
/// returns where it went (`Went`); the other acts return none.
async fn act_set(session: &mut Imap, account: &Account, folder: &Folder, validity: u32, uid: &str, action: &Action, intent: &mut dyn FnMut(&str)) -> Result<Option<Went>, SyncError> {
    let selected = imap::within(COMMAND, session.select(&folder.name)).await?.map_err(imap::server)?;
    if selected.uid_validity.unwrap_or(0) != validity {
        return Err(SyncError::Said(said::RENUMBERED, String::new()));
    }
    let capabilities = imap::within(COMMAND, session.capabilities()).await?.map_err(imap::server)?;
    let (can_move, uidplus) = (capabilities.has_str("MOVE"), uidplus(&capabilities));
    let target = match action {
        Action::Read(read) => return store(session, uid, if *read { "+FLAGS.SILENT (\\Seen)" } else { "-FLAGS.SILENT (\\Seen)" }).await.map(|()| None),
        Action::Flag(flag) => return store(session, uid, if *flag { "+FLAGS.SILENT (\\Flagged)" } else { "-FLAGS.SILENT (\\Flagged)" }).await.map(|()| None),
        Action::Move(target) => target.clone(),
        Action::Trash if folder.role == Role::Trash => {
            store(session, uid, "+FLAGS.SILENT (\\Deleted)").await?;
            return expunge(session, uid, uidplus).await.map(|()| None);
        }
        Action::Trash => role_folder(session, account, Role::Trash, "Trash").await?,
        Action::Junk => {
            // Keywords are optional: a server without them still takes the move.
            let _ = store(session, uid, "-FLAGS.SILENT ($NotJunk)").await;
            let _ = store(session, uid, "+FLAGS.SILENT ($Junk)").await;
            // Said spam in a Junk folder already (the review queue's "Spam" on what your filter moved): it stays.
            if folder.role == Role::Junk {
                return Ok(None);
            }
            role_folder(session, account, Role::Junk, "Junk").await?
        }
        // Your own filter's move: no keyword, as nobody said anything yet.
        Action::Filtered(_) => role_folder(session, account, Role::Junk, "Junk").await?,
        Action::Archive => {
            let known = folders(&account.id);
            match folders::preferred(&known, Role::Archive).or(folders::preferred(&known, Role::All)) {
                Some(f) => f.name.clone(),
                None => role_folder(session, account, Role::Archive, "Archive").await?,
            }
        }
        Action::NotJunk => {
            let _ = store(session, uid, "-FLAGS.SILENT ($Junk)").await;
            let _ = store(session, uid, "+FLAGS.SILENT ($NotJunk)").await;
            "INBOX".to_string()
        }
        // Back into the inbox, as it came: nobody said anything of it.
        Action::BroughtBack => "INBOX".to_string(),
        // A server without keywords keeps nothing: the label log then says it, on this device.
        Action::NotSpam => {
            let _ = store(session, uid, "-FLAGS.SILENT ($Junk)").await;
            let _ = store(session, uid, "+FLAGS.SILENT ($NotJunk)").await;
            return Ok(None);
        }
    };
    intent(&target);
    let placed = move_to(session, uid, &target, can_move, uidplus).await?;
    Ok(Some(Went { folder: target, placed }))
}

pub(crate) async fn store(session: &mut Imap, uid: &str, query: &str) -> Result<(), SyncError> {
    let mut answers = imap::within(COMMAND, session.uid_store(uid, query)).await?.map_err(imap::server)?;
    while let Some(answer) = answers.next().await {
        answer.map_err(imap::server)?;
    }
    Ok(())
}

/// Moves the messages of `uid` (a UID or a set) into `target`: `UID MOVE`
/// (RFC 6851), else `UID COPY`, `\Deleted` and `UID EXPUNGE`. Returns where
/// the server put them, when a server with UIDPLUS said it (COPYUID, RFC
/// 4315 §3: untagged before a MOVE's expunges, or in a COPY's OK). Sent and
/// read by Sioul itself: async-imap's `uid_mv` and `uid_copy` keep no code.
pub(crate) async fn move_to(session: &mut Imap, uid: &str, target: &str, can_move: bool, uidplus: bool) -> Result<Option<Placed>, SyncError> {
    let verb = if can_move { "MOVE" } else { "COPY" };
    let answered = command(session, &format!("UID {verb} {uid} {}", quoted(target)?)).await?;
    if !can_move {
        store(session, uid, "+FLAGS.SILENT (\\Deleted)").await?;
        expunge(session, uid, uidplus).await?;
    }
    Ok(match answered {
        Some(Answered::Copied(placed)) if uidplus => Some(placed),
        _ => None,
    })
}

/// Where the server put what Sioul moved: the target folder's UIDVALIDITY,
/// and each message's new UID by its UID before.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Placed {
    pub validity: u32,
    pub uids: BTreeMap<u32, u32>,
}

/// A set's places as its server said them (COPYUID), kept only where each
/// message's Message-ID confirms them (review of 8b57a35, M1): a range
/// written high to low ("4:2") is read lowest first, the same set by RFC
/// 4315, so the order of the server's text cannot pair a set. After the
/// move's own commands, the destination's UIDs are read with their
/// Message-IDs (EXAMINE, `UID FETCH` of `HEADER.FIELDS (MESSAGE-ID)`), and
/// each moved message is paired with the one destination UID carrying its
/// Message-ID (`paired_by_ids`); one whose Message-ID is missing, or carried
/// twice in the set or at the destination, stays unpaired: its file goes
/// and its note waits for the copy (`follow`'s "not said"). Nothing read
/// (an error, another UIDVALIDITY): none paired. One message keeps its one
/// pair, as said. `sources`: each moved message's address (`mid:…`,
/// `sioul:mail/…`) by its UID before. Leaves `target` examined: whoever acts
/// next selects its own folder first.
pub(crate) async fn confirmed(session: &mut Imap, account: &str, target: &str, placed: Placed, sources: &BTreeMap<u32, String>) -> Placed {
    if placed.uids.len() <= 1 {
        return placed;
    }
    let unpaired = Placed { validity: placed.validity, uids: BTreeMap::new() };
    match imap::within(COMMAND, session.examine(target)).await {
        Ok(Ok(examined)) if examined.uid_validity == Some(placed.validity) => {}
        _ => return unpaired,
    }
    let set = ranges(&placed.uids.values().copied().collect());
    let mut there: BTreeMap<u32, String> = BTreeMap::new();
    let mut whole = true;
    {
        let Ok(Ok(mut fetches)) = imap::within(COMMAND, session.uid_fetch(&set, "(UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)])")).await else { return unpaired };
        // Read to its end, whatever comes: the connection goes on after it.
        while let Some(fetch) = fetches.next().await {
            match fetch {
                Ok(fetch) => {
                    if let (Some(uid), Some(header)) = (fetch.uid, fetch.header()) {
                        there.insert(uid, mailfind::address_of(account, header));
                    }
                }
                Err(_) => whole = false,
            }
        }
    }
    if !whole {
        return unpaired;
    }
    paired_by_ids(&placed, sources, &there)
}

/// Each moved message (`sources`, by its UID before, among those `placed`
/// names) paired with the destination UID (among `placed`'s, `there` giving
/// their addresses) that carries the same Message-ID, whatever order the
/// server wrote: only a Message-ID found once in the set and once at the
/// destination pairs; a message without one (a digest's address) never does.
fn paired_by_ids(placed: &Placed, sources: &BTreeMap<u32, String>, there: &BTreeMap<u32, String>) -> Placed {
    let destinations: BTreeSet<u32> = placed.uids.values().copied().collect();
    let moved: Vec<(u32, &String)> = sources.iter().filter(|(uid, _)| placed.uids.contains_key(uid)).map(|(uid, a)| (*uid, a)).collect();
    let arrived: Vec<(u32, &String)> = there.iter().filter(|(uid, _)| destinations.contains(uid)).map(|(uid, a)| (*uid, a)).collect();
    let mut uids = BTreeMap::new();
    for (uid, address) in &moved {
        if address.starts_with(mailfind::DIGEST_SCHEME) || moved.iter().filter(|(_, a)| a == address).count() != 1 {
            continue;
        }
        let mut carrying = arrived.iter().filter(|(_, a)| a == address);
        if let (Some((destination, _)), None) = (carrying.next(), carrying.next()) {
            uids.insert(*uid, *destination);
        }
    }
    Placed { validity: placed.validity, uids }
}

/// The code an OK carried about where messages went (RFC 4315 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Answered {
    Copied(Placed),
    Appended(ImapOrigin),
}

/// The most UIDs a set from a server is read into: more than any command
/// of Sioul's moves (`SET`), never a range a server wrote by mistake.
const MOST: usize = 4 * SET;

/// A UID set as its members give it, ranges lowest first (imap-proto reads
/// "4:2" as 2:4, the same set by RFC 4315 §4); none past `MOST`.
fn expanded(set: &[UidSetMember]) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for member in set {
        match member {
            UidSetMember::Uid(uid) => out.push(*uid),
            UidSetMember::UidRange(range) => {
                if (u64::from(*range.end()) - u64::from(*range.start())) as usize >= MOST {
                    return None;
                }
                out.extend(range.clone());
            }
        }
        if out.len() > MOST {
            return None;
        }
    }
    Some(out)
}

/// COPYUID's two sets, paired in the order they give (RFC 4315 §3): none
/// when they differ in length, when a UID is paired twice otherwise
/// (repeated members that agree are taken: GreenMail writes "2,2:3"), or
/// when a UIDVALIDITY or a UID is 0, which no server gives.
fn paired(validity: u32, from: &[UidSetMember], to: &[UidSetMember]) -> Option<Placed> {
    let (from, to) = (expanded(from)?, expanded(to)?);
    if validity == 0 || from.is_empty() || from.len() != to.len() || from.contains(&0) || to.contains(&0) {
        return None;
    }
    let (mut uids, mut back) = (BTreeMap::new(), BTreeMap::new());
    for (old, new) in from.into_iter().zip(to) {
        if *uids.entry(old).or_insert(new) != new || *back.entry(new).or_insert(old) != old {
            return None;
        }
    }
    Some(Placed { validity, uids })
}

fn answered(code: &ResponseCode) -> Option<Answered> {
    match code {
        ResponseCode::CopyUid(validity, from, to) => paired(*validity, from, to).map(Answered::Copied),
        ResponseCode::AppendUid(validity, set) => match expanded(set)?.as_slice() {
            [uid] if *validity != 0 && *uid != 0 => Some(Answered::Appended(ImapOrigin { validity: *validity, uid: *uid })),
            _ => None,
        },
        _ => None,
    }
}

/// A folder's name as a quoted string (RFC 9051 §4.3): never a line end.
fn quoted(name: &str) -> Result<String, SyncError> {
    if name.contains(['\r', '\n']) {
        return Err(SyncError::Said(said::NO_FOLDER, name.to_string()));
    }
    Ok(format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")))
}

/// `line` sent as a command, and its answers read to its end: the COPYUID
/// or APPENDUID an OK carried, untagged or tagged. A NO or a BAD is the
/// server's refusal, said as async-imap says it.
async fn command(session: &mut Imap, line: &str) -> Result<Option<Answered>, SyncError> {
    let id = imap::within(COMMAND, session.run_command(line)).await?.map_err(imap::server)?;
    answers(session, &id).await
}

/// The answers to the command `id`, to its tagged one.
async fn answers(session: &mut Imap, id: &RequestId) -> Result<Option<Answered>, SyncError> {
    let mut found = None;
    loop {
        let response = imap::within(COMMAND, session.read_response()).await?.map_err(|e| SyncError::Network(e.to_string()))?.ok_or_else(|| SyncError::Network("the connection was closed".into()))?;
        match response.parsed() {
            Response::Data { status: Status::Ok, outcome } => {
                if found.is_none() {
                    found = outcome.code.as_ref().and_then(answered);
                }
            }
            Response::Done { tag, status, outcome } if tag == id => {
                let words = format!("code: {:?}, info: {:?}", outcome.code, outcome.information);
                return match status {
                    Status::Ok => Ok(found.or_else(|| outcome.code.as_ref().and_then(answered))),
                    Status::No => Err(imap::server(async_imap::error::Error::No(words))),
                    _ => Err(imap::server(async_imap::error::Error::Bad(words))),
                };
            }
            _ => {}
        }
    }
}

/// `raw` appended to `folder` with `flags` (`(\Seen)`): a synchronizing
/// literal (RFC 9051 §4.3), its bytes written once the server says "+".
/// Returns where the server put it, when it said (APPENDUID).
async fn append(session: &mut Imap, folder: &str, flags: &str, raw: &[u8]) -> Result<Option<Answered>, SyncError> {
    use tokio::io::AsyncWriteExt;
    let id = imap::within(COMMAND, session.run_command(format!("APPEND {} {flags} {{{}}}", quoted(folder)?, raw.len()))).await?.map_err(imap::server)?;
    loop {
        let response = imap::within(COMMAND, session.read_response()).await?.map_err(|e| SyncError::Network(e.to_string()))?.ok_or_else(|| SyncError::Network("the connection was closed".into()))?;
        match response.parsed() {
            Response::Continue { .. } => break,
            Response::Done { tag, outcome, .. } if tag == &id => {
                return Err(imap::server(async_imap::error::Error::No(format!("code: {:?}, info: {:?}", outcome.code, outcome.information))));
            }
            _ => {}
        }
    }
    let stream = session.get_mut();
    imap::within(COMMAND, async {
        stream.write_all(raw).await?;
        stream.write_all(b"\r\n").await?;
        stream.flush().await
    })
    .await?
    .map_err(|e| SyncError::Network(e.to_string()))?;
    answers(session, &id).await
}

/// Whether a server's COPYUID and APPENDUID are read: it lists UIDPLUS
/// (RFC 4315). Tests turn it off, as a server without it.
pub(crate) fn uidplus(capabilities: &async_imap::types::Capabilities) -> bool {
    capabilities.has_str("UIDPLUS") && !without_uidplus()
}

#[cfg(not(test))]
fn without_uidplus() -> bool {
    false
}

#[cfg(test)]
fn without_uidplus() -> bool {
    tests::WITHOUT_UIDPLUS.load(std::sync::atomic::Ordering::Relaxed)
}

/// How long a moved message's file waits for its account's fetch lock
/// (`follow`): a fetch of a folder's older mail may hold it for minutes.
const LOCK_WAIT: Duration = Duration::from_secs(20);

#[cfg(not(test))]
fn lock_wait() -> Duration {
    LOCK_WAIT
}

/// `LOCK_WAIT`, which tests shorten.
#[cfg(test)]
fn lock_wait() -> Duration {
    match tests::LOCK_WAIT_SHORT.load(std::sync::atomic::Ordering::Relaxed) {
        0 => LOCK_WAIT,
        millis => Duration::from_millis(millis),
    }
}

/// The directory of `folder` (its server name) of `account` here, when that
/// folder is kept here (`fetched`: not left on the server, not Gmail's All
/// Mail beside an Archive, a directory of its own), and the account's
/// Maildir is shared with no other address's.
fn kept_dir(account: &Account, folder: &str) -> Option<PathBuf> {
    let all = folders(&account.id);
    let target = all.iter().find(|f| f.name == folder).cloned().or_else(|| folder.eq_ignore_ascii_case("INBOX").then(|| folders::folder("INBOX", None, None)))?;
    fetched(&target, &all, account).then(|| account.maildir_path().join(&target.local))
}

/// The file of a message the server moved into `folder` of `to` (its server
/// name; `to` the address it went into) follows it there at once, under the
/// UIDVALIDITY and UID the server gave it (`place`), so that the reader,
/// links and the home card keep it (the unified mailbox's §3.5) and the next
/// sync of that folder finds it kept (`fetch::to_fetch`): one rename, under
/// `to`'s fetch lock, so that no sync writes a second copy beside it. A copy
/// there already (Gmail's All Mail holds the inbox's mail; a sync fetched it
/// first): the file goes instead, nothing renamed. The file goes, as before,
/// when the folder is not kept here (no copy will come: its note goes), and,
/// its note kept until the copy comes (`mailmoved`), when `to`'s Maildir is
/// shared with another address, when the server did not say where it put it,
/// or when the lock does not come in time. `flags`: its letters there.
/// Returns where the file is now, when it is still here.
pub(crate) fn follow(to: &Account, file: &Path, folder: &str, place: Option<ImapOrigin>, flags: &str, address: Option<&str>) -> Option<PathBuf> {
    let one = Following { file: file.to_path_buf(), place, flags: flags.to_string(), address: address.map(str::to_string) };
    follow_all(to, folder, vec![one]).into_iter().next().map(|(_, now)| now)
}

/// A moved message's file, as `follow` takes it.
pub(crate) struct Following {
    pub file: PathBuf,
    pub place: Option<ImapOrigin>,
    pub flags: String,
    pub address: Option<String>,
}

/// `follow` for the files of several messages moved into one folder (a
/// selection, a filter's set): under one lock and one look at the folder.
/// Returns where each file that followed is now.
pub(crate) fn follow_all(to: &Account, folder: &str, moved: Vec<Following>) -> Moved {
    // Another address's Maildir covering `to`'s (`renumbered::shared_with_others`, which
    // counts a configuration that does not read as shared); without a configuration file, none.
    follow_all_with(to, folder, moved, || sioul_core::config::default_path().exists() && crate::renumbered::shared_with_others(to))
}

fn follow_all_with(to: &Account, folder: &str, moved: Vec<Following>, shared: impl FnOnce() -> bool) -> Moved {
    let forget = |one: &Following| {
        if let Some(address) = &one.address {
            let _ = mailmoved::forget(std::slice::from_ref(address));
        }
    };
    let leave = |file: &Path| {
        let _ = std::fs::remove_file(file);
    };
    let Some(dir) = kept_dir(to, folder) else {
        for one in &moved {
            leave(&one.file);
            forget(one);
        }
        return Vec::new();
    };
    // Where the server did not say, or another address shares the Maildir: on its way.
    let (placed, unsaid): (Vec<Following>, Vec<Following>) = moved.into_iter().partition(|one| one.place.is_some());
    for one in &unsaid {
        leave(&one.file);
    }
    if placed.is_empty() {
        return Vec::new();
    }
    if shared() {
        for one in &placed {
            leave(&one.file);
        }
        return Vec::new();
    }
    let Some(_lock) = crate::fetch::lock_within(&crate::fetch::lock_path(to), lock_wait()) else {
        for one in &placed {
            leave(&one.file);
        }
        return Vec::new();
    };
    // What the folder holds already, by place: read once.
    let mut there: BTreeMap<ImapOrigin, PathBuf> = ["cur", "new"]
        .iter()
        .filter_map(|sub| std::fs::read_dir(dir.join(sub)).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter_map(|p| Some((maildir::origin_of(&p)?, p)))
        .collect();
    let mut followed = Vec::new();
    for one in placed {
        let Some(place) = one.place else { continue };
        // Removed meanwhile by a sync of its folder (the server no longer holds it there): its copy comes.
        let Some(file) = maildir::locate(&one.file) else { continue };
        if let Some(copy) = there.get(&place) {
            leave(&file);
            forget(&one);
            followed.push((one.file.clone(), copy.clone()));
            continue;
        }
        match maildir::move_into(&file, &dir, place, &one.flags) {
            Ok(now) => {
                forget(&one);
                there.insert(place, now.clone());
                followed.push((one.file.clone(), now));
            }
            Err(_) => leave(&file),
        }
    }
    followed
}

/// Removes a message marked deleted. Without UIDPLUS, a plain EXPUNGE could also
/// remove other messages marked deleted by another client: the mark is left,
/// and most clients hide such messages.
async fn expunge(session: &mut Imap, uid: &str, uidplus: bool) -> Result<(), SyncError> {
    if !uidplus {
        return Ok(());
    }
    let gone = imap::within(COMMAND, session.uid_expunge(uid)).await?.map_err(imap::server)?;
    let mut gone = std::pin::pin!(gone);
    while let Some(answer) = gone.next().await {
        answer.map_err(imap::server)?;
    }
    Ok(())
}

/// The server name of the folder for `role`; made, with `name`, when the server has none.
pub(crate) async fn role_folder(session: &mut Imap, account: &Account, role: Role, name: &str) -> Result<String, SyncError> {
    if let Some(folder) = folders::preferred(&folders(&account.id), role) {
        return Ok(folder.name.clone());
    }
    let listed = list(session).await?;
    if let Some(folder) = folders::preferred(&listed, role) {
        let name = folder.name.clone();
        save(&account.id, &listed)?;
        return Ok(name);
    }
    // Under the inbox where the server keeps its folders there ("INBOX.Trash").
    let under_inbox = listed.iter().any(|f| f.name.to_ascii_uppercase().starts_with("INBOX."));
    let name = if under_inbox { format!("INBOX.{name}") } else { name.to_string() };
    imap::within(COMMAND, session.create(&name)).await?.map_err(imap::server)?;
    let listed = list(session).await?;
    save(&account.id, &listed)?;
    Ok(name)
}

/// `follow` with `shared` as given, for tests.
#[cfg(test)]
fn follow_with(to: &Account, file: &Path, folder: &str, place: Option<ImapOrigin>, flags: &str, address: Option<&str>, shared: impl FnOnce() -> bool) -> Option<PathBuf> {
    let one = Following { file: file.to_path_buf(), place, flags: flags.to_string(), address: address.map(str::to_string) };
    follow_all_with(to, folder, vec![one], shared).into_iter().next().map(|(_, now)| now)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server without UIDPLUS, as tests make one (`uidplus`).
    pub(super) static WITHOUT_UIDPLUS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    /// `LOCK_WAIT` shortened, in milliseconds; 0: as it is.
    pub(super) static LOCK_WAIT_SHORT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    #[test]
    fn what_an_act_leaves_here_and_says() {
        // Said not spam: it stays, its `$Junk` letter traded for `$NotJunk`'s.
        assert_eq!(kept_with(&Action::NotSpam, &format!("S{}", maildir::JUNK), Role::Inbox), Some(format!("S{}", maildir::NOT_JUNK)));
        assert_eq!(kept_with(&Action::NotSpam, "", Role::Inbox), Some(maildir::NOT_JUNK.to_string()));
        // Read, flagged: the keywords stay beside the flags.
        assert_eq!(kept_with(&Action::Read(false), &format!("S{}", maildir::NOT_JUNK), Role::Inbox), Some(maildir::NOT_JUNK.to_string()));
        // Moved: it leaves this folder; your filter's move too.
        assert_eq!(kept_with(&Action::Junk, "S", Role::Inbox), None);
        assert_eq!(kept_with(&Action::NotJunk, "S", Role::Junk), None);
        assert_eq!(kept_with(&Action::Filtered(sioul_core::spam::Class::Spam), "", Role::Inbox), None);
        // Proven mail brought back by the matrix's row: it leaves the Junk folder, its letters as they were.
        assert_eq!(kept_with(&Action::BroughtBack, "S", Role::Junk), None);
        assert_eq!(moved_flags(&Action::BroughtBack, "S"), "S");
        // Said spam where your filter moved it: it stays, `$Junk` now.
        assert_eq!(kept_with(&Action::Junk, &format!("S{}", maildir::NOT_JUNK), Role::Junk), Some(format!("S{}", maildir::JUNK)));
        // What goes into the label log, and what does not: your filter's move is never a label.
        assert_eq!([&Action::Junk, &Action::NotJunk, &Action::NotSpam].map(said), [Some(labels::Source::Junk), Some(labels::Source::NotJunk), Some(labels::Source::NotSpam)]);
        assert_eq!([&Action::Read(true), &Action::Trash, &Action::Archive, &Action::Filtered(sioul_core::spam::Class::Spam)].map(said), [None, None, None, None]);
        // Nobody said anything of proven mail brought back: no label.
        assert_eq!(said(&Action::BroughtBack), None);
    }

    /// The code of an answer, as the server writes it, read by imap-proto.
    fn code_of(line: &str) -> Option<Answered> {
        let (_, response) = Response::parse(line.as_bytes()).unwrap();
        match response {
            Response::Data { outcome, .. } | Response::Done { outcome, .. } => outcome.code.as_ref().and_then(answered),
            other => panic!("{other:?}"),
        }
    }

    /// COPYUID and APPENDUID read pair by pair (RFC 4315 §3), untagged or
    /// tagged, as GreenMail and other servers write them; nothing trusted
    /// that does not pair one to one.
    #[test]
    fn where_the_server_put_a_message_is_read_pair_by_pair() {
        let placed = |validity: u32, pairs: &[(u32, u32)]| Some(Answered::Copied(Placed { validity, uids: pairs.iter().copied().collect() }));
        // A MOVE's, untagged before its expunges; a COPY's, in its tagged OK.
        assert_eq!(code_of("* OK [COPYUID 1793587285 1 1]\r\n"), placed(1793587285, &[(1, 1)]));
        assert_eq!(code_of("a9 OK [COPYUID 38505 304,319:320 3956:3958] Done\r\n"), placed(38505, &[(304, 3956), (319, 3957), (320, 3958)]));
        // GreenMail's set with a repeated member: the pairs agree, taken.
        assert_eq!(code_of("a9 OK [COPYUID 7 2,2:3 2,2:3] COPY completed.\r\n"), placed(7, &[(2, 2), (3, 3)]));
        // Not one to one: lengths differ, a UID paired twice otherwise, 0, a range past reason.
        assert_eq!(code_of("a9 OK [COPYUID 7 2:4 10:11] x\r\n"), None);
        assert_eq!(code_of("a9 OK [COPYUID 7 2,2 10,11] x\r\n"), None);
        assert_eq!(code_of("a9 OK [COPYUID 7 2,3 10,10] x\r\n"), None);
        assert_eq!(code_of("a9 OK [COPYUID 7 0 10] x\r\n"), None);
        assert_eq!(code_of("a9 OK [COPYUID 7 1:4000000000 1:4000000000] x\r\n"), None);
        // APPENDUID: one message, one place.
        assert_eq!(code_of("a5 OK [APPENDUID 1793587285 3] APPEND completed.\r\n"), Some(Answered::Appended(ImapOrigin { validity: 1793587285, uid: 3 })));
        assert_eq!(code_of("a5 OK [APPENDUID 9 3:4] x\r\n"), None);
        // Another code, or none.
        assert_eq!(code_of("a5 OK [READ-WRITE] SELECT completed.\r\n"), None);
        assert_eq!(code_of("* OK Still here\r\n"), None);
        // A folder's name quoted, never a line end in a command.
        assert_eq!(quoted("Banque \"perso\"\\x").unwrap(), "\"Banque \\\"perso\\\"\\\\x\"");
        assert!(quoted("Banque\r\nA1 DELETE INBOX").is_err());
    }

    /// A move's notes, written before its command: gone when the server
    /// refused it (nothing moved), kept when its answer did not come (it may
    /// have moved), none written when the command was never sent.
    #[test]
    fn a_move_refused_is_not_on_its_way() {
        crate::dav::stand_in::home();
        let address = |what: &str| format!("mid:intent-{what}-{}@example.org", std::process::id());
        let intent = |what: &str| {
            let mut intent = Intent::new("intent", vec![address(what)]);
            intent.note("Archive");
            intent
        };
        assert!(intent("refused").after::<()>(Err(SyncError::Server("NO [TRYCREATE] no such folder".into()))).is_err());
        assert_eq!(mailmoved::of(&address("refused")), None, "refused: nothing moved");
        assert!(intent("lost").after::<()>(Err(SyncError::Network("the connection was closed".into()))).is_err());
        assert_eq!(mailmoved::of(&address("lost")).map(|m| m.folder), Some("Archive".into()), "its answer lost: it may have moved");
        assert!(intent("done").after(Ok(())).is_ok());
        assert!(mailmoved::of(&address("done")).is_some(), "done: its file follows, or the note waits for the copy");
        let never = Intent::new("intent", vec![address("never")]);
        assert!(never.after::<()>(Err(SyncError::Network("no answer".into()))).is_err() && mailmoved::of(&address("never")).is_none());
        mailmoved::forget(&[address("lost"), address("done")]).unwrap();
    }

    /// A stand-in account with the inbox, an archive and a folder kept on the
    /// server only, its Maildir under the test's own data folder.
    fn stand_in(name: &str) -> Account {
        crate::dav::stand_in::home();
        let id = format!("follow-{name}-{}", std::process::id());
        let mut account = Account::imap(&id, "me@example.org", "imap.example.org", 993, sioul_core::config::Security::Tls, None);
        let root = sioul_core::config::data_dir().join(&id);
        let _ = std::fs::remove_dir_all(&root);
        account.maildir = Some(root.display().to_string());
        account.skip_folders = vec!["Lunch".into()];
        let list = vec![folders::folder("INBOX", Some("/"), Some(Role::Inbox)), folders::folder("Archive", Some("/"), Some(Role::Archive)), folders::folder("Lunch", Some("/"), None)];
        save(&id, &list).unwrap();
        for f in &list {
            for sub in ["tmp", "new", "cur"] {
                std::fs::create_dir_all(root.join(&f.local).join(sub)).unwrap();
            }
        }
        account
    }

    /// A message of the stand-in's inbox at `uid`, its Message-ID `id`.
    fn inbox_file(account: &Account, uid: u32, id: &str) -> PathBuf {
        let raw = format!("Message-ID: <{id}>\r\nFrom: <ana@example.org>\r\nSubject: Lunch\r\n\r\nNoon?\r\n");
        maildir::store(&account.maildir_path(), &maildir::Fetched { origin: ImapOrigin { validity: 7, uid }, flags: "S".into(), received: Some(1_777_000_000), raw: raw.as_bytes() }).unwrap()
    }

    /// A moved message's file follows it into its folder under its new UID,
    /// once (the next sync fetches nothing); goes when a copy is there
    /// already; goes, said on its way, when the server did not say where, the
    /// Maildir is shared or a sync holds the lock too long; goes, never said
    /// on its way, into a folder kept on the server only; a file a sync
    /// removed first is said on its way.
    #[test]
    fn a_moved_message_follows_into_its_folder() {
        let account = stand_in("follows");
        let archive = account.maildir_path().join(".Archive");
        let id = |n: u32| format!("follow-{n}-{}@example.org", std::process::id());
        let address = |n: u32| format!("mid:{}", id(n));
        let noted = |n: u32| {
            mailmoved::note(&account.id, &[address(n)], "Archive", jiff::Timestamp::now().as_second()).unwrap();
        };
        let on_its_way = |n: u32| mailmoved::of(&address(n)).is_some();
        let place = ImapOrigin { validity: 9, uid: 41 };
        // Renamed into the archive under its new place; its note goes; the next sync fetches nothing.
        let file = inbox_file(&account, 1, &id(1));
        noted(1);
        let line_of = |local: &str, path: &Path| sioul_core::mailcache::folder(&account.id, &account.maildir_path(), local).into_iter().find(|(p, _)| p == path).map(|(_, line)| line);
        let before = line_of("", &file).expect("its line in the inbox's index");
        let now = follow_with(&account, &file, "Archive", Some(place), "S", Some(&address(1)), || false).unwrap();
        assert!(!file.exists() && now.starts_with(&archive) && maildir::origin_of(&now) == Some(place), "{now:?}");
        assert_eq!(mailfind::address_of_file(&account.id, &now), Some(address(1)), "its address kept");
        // A fresh line in the archive's index, under its new name, its digest and address kept.
        let after = line_of(".Archive", &now).expect("its line in the archive's index");
        assert_eq!(after.unique, maildir::unique_part(now.file_name().unwrap().to_str().unwrap()));
        assert_ne!(after.unique, before.unique);
        assert_eq!((after.digest.clone(), after.message_id.clone(), after.arrived), (before.digest.clone(), before.message_id.clone(), before.arrived));
        assert_eq!(mailfind::address_of_line(&account.id, &after), Some(address(1)));
        assert!(!on_its_way(1));
        assert_eq!(crate::fetch::to_fetch([40, 41, 42], 40, &archive, 9), (vec![42], 41), "the next sync fetches only what it lacks");
        // A copy there already (Gmail's All Mail, a sync first): the file goes, nothing renamed.
        let twin = inbox_file(&account, 2, &id(2));
        noted(2);
        assert_eq!(follow_with(&account, &twin, "Archive", Some(place), "S", Some(&address(2)), || false), Some(now.clone()));
        assert!(!twin.exists() && !on_its_way(2));
        assert_eq!(["cur", "new"].iter().flat_map(|sub| std::fs::read_dir(archive.join(sub)).unwrap()).count(), 1, "one copy, never two");
        // The server did not say where: the file goes, on its way until its copy comes.
        let unsaid = inbox_file(&account, 3, &id(3));
        noted(3);
        assert_eq!(follow_with(&account, &unsaid, "Archive", None, "S", Some(&address(3)), || false), None);
        assert!(!unsaid.exists() && on_its_way(3));
        // Another address shares the Maildir: nothing renamed into it.
        let shared = inbox_file(&account, 4, &id(4));
        noted(4);
        assert_eq!(follow_with(&account, &shared, "Archive", Some(ImapOrigin { validity: 9, uid: 44 }), "S", Some(&address(4)), || true), None);
        assert!(!shared.exists() && on_its_way(4));
        // A folder kept on the server only: no copy will come here.
        let away = inbox_file(&account, 5, &id(5));
        noted(5);
        assert_eq!(follow_with(&account, &away, "Lunch", Some(ImapOrigin { validity: 9, uid: 45 }), "S", Some(&address(5)), || false), None);
        assert!(!away.exists() && !on_its_way(5));
        // A sync holds the account's lock past the wait: the file goes, on its way.
        LOCK_WAIT_SHORT.store(200, std::sync::atomic::Ordering::Relaxed);
        let held = crate::fetch::lock(&crate::fetch::lock_path(&account)).unwrap();
        let waiting = inbox_file(&account, 6, &id(6));
        noted(6);
        let started = std::time::Instant::now();
        assert_eq!(follow_with(&account, &waiting, "Archive", Some(ImapOrigin { validity: 9, uid: 46 }), "S", Some(&address(6)), || false), None);
        assert!(started.elapsed() >= Duration::from_millis(200) && !waiting.exists() && on_its_way(6));
        drop(held);
        LOCK_WAIT_SHORT.store(0, std::sync::atomic::Ordering::Relaxed);
        // Removed by a sync of its folder first: on its way, nothing written.
        let removed = inbox_file(&account, 7, &id(7));
        noted(7);
        std::fs::remove_file(&removed).unwrap();
        assert_eq!(follow_with(&account, &removed, "Archive", Some(ImapOrigin { validity: 9, uid: 47 }), "S", Some(&address(7)), || false), None);
        assert!(on_its_way(7) && maildir::holding(&archive, ImapOrigin { validity: 9, uid: 47 }).is_none());
        mailmoved::forget(&(1..=7).map(address).collect::<Vec<_>>()).unwrap();
        // A set into one folder, under one lock and one look at it: each file
        // its own place; one whose place a copy holds already goes.
        let set: Vec<PathBuf> = (8..=10).map(|n| inbox_file(&account, n, &id(n))).collect();
        (8..=10).for_each(noted);
        let held_already = maildir::store(&archive, &maildir::Fetched { origin: ImapOrigin { validity: 9, uid: 60 }, flags: "S".into(), received: None, raw: b"Message-ID: <x@example.org>\r\n\r\nx\r\n" }).unwrap();
        let moving: Vec<Following> = set.iter().zip([58, 59, 60]).zip(8..=10).map(|((file, uid), n)| Following { file: file.clone(), place: Some(ImapOrigin { validity: 9, uid }), flags: "S".into(), address: Some(address(n)) }).collect();
        let followed = follow_all_with(&account, "Archive", moving, || false);
        assert_eq!(followed.iter().map(|(before, _)| before.clone()).collect::<Vec<_>>(), set);
        assert_eq!(followed.iter().map(|(_, now)| maildir::origin_of(now).map(|o| o.uid)).collect::<Vec<_>>(), vec![Some(58), Some(59), Some(60)]);
        assert_eq!(followed[2].1, held_already, "its place held already: the copy there");
        assert!(set.iter().all(|f| !f.exists()) && (8..=10).all(|n| !on_its_way(n)));
        let _ = std::fs::remove_dir_all(account.maildir_path());
    }

    /// Against GreenMail (docs/building.md), with invented mail only:
    /// `SIOUL_TEST_INSECURE_TLS=1 cargo test --release -p sioul-sync
    /// --features insecure-test-tls -- --ignored greenmail_a_move` (with
    /// `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`). Archive, junk, the
    /// trash, a move, a selection's move and a move into another address keep
    /// the file here at once, under the place the server gave it, and the next
    /// sync fetches nothing twice; another device sees each moved message once;
    /// a server without UIDPLUS: the move noted, "on its way", then the copy
    /// opened as it comes; the server moved but the file did not follow (a
    /// stop): one copy here, then one in its folder; a move during a sync of
    /// its folder: one copy; a sync holding the lock too long: on its way, then one copy.
    #[cfg(feature = "insecure-test-tls")]
    #[test]
    #[ignore]
    fn greenmail_a_move_keeps_the_message_here() {
        use sioul_core::config::Security;
        let home = crate::dav::stand_in::home();
        assert!(std::env::var_os("SIOUL_TEST_INSECURE_TLS").is_some(), "GreenMail tests need SIOUL_TEST_INSECURE_TLS=1 and the container sioul-greenmail (docs/building.md)");
        let (pid, stamp) = (std::process::id(), jiff::Timestamp::now().as_second());
        let login = format!("moves-{pid}-{stamp}@example.org");
        let account_at = |id: &str, login: &str| {
            let mut account = Account::imap(id, login, "localhost", 3993, Security::Tls, None);
            account.maildir = Some(home.join(format!("mail-{id}")).display().to_string());
            account
        };
        // Two devices of one address (each its own Maildir and sync state), and another address.
        let (a, b) = (account_at(&format!("moves-a-{pid}"), &login), account_at(&format!("moves-b-{pid}"), &login));
        let c = account_at(&format!("moves-c-{pid}"), &format!("moves-other-{pid}-{stamp}@example.org"));
        let id = |n: u32| format!("move-{n}-{pid}-{stamp}@example.org");
        let raw = |n: u32| format!("Message-ID: <{}>\r\nFrom: Ana <ana@example.org>\r\nTo: {login}\r\nSubject: Move {n}\r\nDate: Mon, 02 Nov 2026 09:00:00 +0100\r\n\r\nHello {n}.\r\n", id(n)).into_bytes();
        let server = Server::of(&a).unwrap();
        crate::fetch::block_on(async {
            let mut session = imap::open(&server, "x").await?;
            for n in (1..=12).chain(14..=16) {
                imap::within(COMMAND, session.append("INBOX", None, None, raw(n))).await?.map_err(imap::server)?;
            }
            // Two messages of one Message-ID (someone reused it).
            for twin in ["one", "two"] {
                let raw = String::from_utf8(raw(17)).unwrap().replace("Subject: Move 17", &format!("Subject: Twin {twin}"));
                imap::within(COMMAND, session.append("INBOX", None, None, raw)).await?.map_err(imap::server)?;
            }
            for folder in ["Archive", "Junk", "Trash", "Banque"] {
                let _ = imap::within(COMMAND, session.create(folder)).await?;
            }
            let listed = list(&mut session).await?;
            save(&a.id, &listed)?;
            save(&b.id, &listed)?;
            let _ = session.logout().await;
            Ok(())
        })
        .unwrap();
        // Where a message is on an address's server: its folder's UIDVALIDITY and its UID there.
        let on_server_of = |account: &Account, folder: &str, n: u32| -> Option<ImapOrigin> {
            let server = Server::of(account).unwrap();
            crate::fetch::block_on(async {
                let mut session = imap::open(&server, "x").await?;
                let examined = imap::within(COMMAND, session.examine(folder)).await?.map_err(imap::server)?;
                let found = imap::within(COMMAND, session.uid_search(format!("HEADER Message-ID \"{}\"", id(n)))).await?.map_err(imap::server)?;
                let _ = session.logout().await;
                Ok(found.into_iter().next().map(|uid| ImapOrigin { validity: examined.uid_validity.unwrap_or(0), uid }))
            })
            .unwrap()
        };
        let on_server = |folder: &str, n: u32| on_server_of(&a, folder, n);
        // Every file of `account` holding message `n`, with its folder's server name.
        let copies = |account: &Account, n: u32| -> Vec<(String, PathBuf)> {
            let wanted = format!("Message-ID: <{}>", id(n));
            let mut found = Vec::new();
            for folder in folders(&account.id) {
                for sub in ["cur", "new"] {
                    for entry in std::fs::read_dir(account.maildir_path().join(&folder.local).join(sub)).into_iter().flatten().filter_map(Result::ok) {
                        if std::fs::read_to_string(entry.path()).is_ok_and(|text| text.contains(&wanted)) {
                            found.push((folder.name.clone(), entry.path()));
                        }
                    }
                }
            }
            found
        };
        let one = |account: &Account, n: u32| -> (String, PathBuf) {
            let all = copies(account, n);
            assert_eq!(all.len(), 1, "message {n} of {}: one copy, never none, never two: {all:?}", account.id);
            all.into_iter().next().unwrap()
        };
        // Each moved file: in its folder, under the place its server gave it.
        let placed = |account: &Account, n: u32, folder: &str| {
            let (there, file) = one(account, n);
            assert_eq!(there, folder, "message {n}");
            assert_eq!(maildir::origin_of(&file), on_server(folder, n), "message {n}: its server's place");
            file
        };
        crate::fetch::sync(&a, "x").unwrap();
        crate::fetch::sync(&b, "x").unwrap();
        let inbox_file = |n: u32| one(&a, n).1;
        // Archive, junk, the trash, a move to a folder: the file follows at once.
        for (n, action, folder) in [(1, Action::Archive, "Archive"), (2, Action::Junk, "Junk"), (3, Action::Trash, "Trash"), (4, Action::Move("Banque".into()), "Banque")] {
            let before = inbox_file(n);
            let moved = act(&a, "x", &before, &action).unwrap();
            let now = placed(&a, n, folder);
            assert_eq!(moved, vec![(before, now)], "message {n}");
        }
        assert!(maildir::flags_of(&one(&a, 2).1).contains(maildir::JUNK), "said spam: `$Junk`'s letter kept");
        // A selection: each file paired with its own new UID.
        let (five, six) = (inbox_file(5), inbox_file(6));
        let moved = act_many(&a, "x", &[five, six], &[], &Action::Move("Banque".into())).unwrap();
        assert_eq!(moved.len(), 2);
        placed(&a, 5, "Banque");
        placed(&a, 6, "Banque");
        // A set of three, each pair confirmed by its Message-ID (review of 8b57a35, M1).
        let three: Vec<PathBuf> = [14, 15, 16].map(inbox_file).to_vec();
        let moved = act_many(&a, "x", &three, &[], &Action::Move("Banque".into())).unwrap();
        assert_eq!(moved.len(), 3, "{moved:?}");
        for n in [14, 15, 16] {
            placed(&a, n, "Banque");
        }
        // Two messages of one Message-ID in a set: no pair confirmed; their
        // files go, on their way, and their copies come with the next sync.
        let twins: Vec<PathBuf> = copies(&a, 17).into_iter().map(|(_, file)| file).collect();
        assert_eq!(twins.len(), 2);
        let moved = act_many(&a, "x", &twins, &[], &Action::Move("Banque".into())).unwrap();
        assert!(moved.is_empty() && copies(&a, 17).is_empty(), "{moved:?}");
        assert_eq!(mailmoved::of(&format!("mid:{}", id(17))).map(|m| m.folder), Some("Banque".into()));
        // Into another address: under the place its server gave it (APPENDUID).
        let seven = inbox_file(7);
        let moved = move_across(&a, "x", &seven, &c, "x", "INBOX").unwrap();
        assert_eq!(moved.len(), 1);
        assert!(copies(&a, 7).is_empty());
        assert!(moved[0].1.starts_with(c.maildir_path()) && maildir::origin_of(&moved[0].1) == on_server_of(&c, "INBOX", 7), "{moved:?}");
        // The next syncs fetch nothing twice.
        let again = crate::fetch::sync(&a, "x").unwrap();
        let twin = format!("Message-ID: <{}>", id(17));
        assert!(again.new.is_empty() && again.elsewhere.len() == 2 && again.elsewhere.iter().all(|f| std::fs::read_to_string(f).unwrap().contains(&twin)), "only the twins, which waited for it: {again:?}");
        let twins = copies(&a, 17);
        assert!(twins.len() == 2 && twins.iter().all(|(folder, _)| folder == "Banque"), "{twins:?}");
        assert!(mailmoved::of(&format!("mid:{}", id(17))).is_none(), "their copies here, the note gone");
        let other = crate::fetch::sync(&c, "x").unwrap();
        assert!(other.new.is_empty() && other.elsewhere.is_empty(), "the other address's first sync finds it kept: {other:?}");
        assert_eq!(one(&c, 7).0, "INBOX");
        for (n, folder) in [(1, "Archive"), (2, "Junk"), (3, "Trash"), (4, "Banque"), (5, "Banque"), (6, "Banque"), (14, "Banque"), (15, "Banque"), (16, "Banque")] {
            placed(&a, n, folder);
        }
        // Another device: each moved message once, in its folder.
        crate::fetch::sync(&b, "x").unwrap();
        for (n, folder) in [(1, "Archive"), (2, "Junk"), (3, "Trash"), (4, "Banque"), (5, "Banque"), (6, "Banque"), (14, "Banque"), (15, "Banque"), (16, "Banque")] {
            assert_eq!(one(&b, n).0, folder);
        }
        assert_eq!(copies(&b, 17).len(), 2);
        assert!(copies(&b, 7).is_empty());
        let roots = sioul_core::mailindex::MailIndex::roots_of(&{
            let mut config = sioul_core::config::Config::default();
            config.accounts = vec![a.clone()];
            config
        });
        let find = |n: u32| mailfind::find(&roots, &mailfind::MailRef { address: format!("mid:{}", id(n)), ..mailfind::MailRef::default() });
        // A server without UIDPLUS: the file goes, the move noted, "on its way"; its copy then comes with the sync.
        WITHOUT_UIDPLUS.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(act(&a, "x", &inbox_file(8), &Action::Archive).unwrap().is_empty());
        WITHOUT_UIDPLUS.store(false, std::sync::atomic::Ordering::Relaxed);
        assert!(copies(&a, 8).is_empty());
        assert_eq!(find(8), mailfind::Found::OnItsWay { account: a.id.clone(), folder: "Archive".into() });
        crate::fetch::sync(&a, "x").unwrap();
        placed(&a, 8, "Archive");
        assert!(matches!(find(8), mailfind::Found::Here(_)) && mailmoved::of(&format!("mid:{}", id(8))).is_none(), "the copy here, the note gone");
        // Stopped between the server's move and the file's: the note written, the server moved, nothing here changed.
        let nine = inbox_file(9);
        let mut intent = Intent::new(&a.id, vec![format!("mid:{}", id(9))]);
        crate::fetch::block_on(async {
            let mut session = imap::open(&server, "x").await?;
            let origin = maildir::origin_of(&nine).unwrap();
            let result = act_in(&mut session, &a, &folders::folder("INBOX", None, None), origin, &Action::Archive, &mut |target| intent.note(target)).await;
            let _ = session.logout().await;
            result
        })
        .unwrap();
        assert_eq!(one(&a, 9).0, "INBOX", "one copy here, where it was");
        crate::fetch::sync(&a, "x").unwrap();
        placed(&a, 9, "Archive");
        assert!(mailmoved::of(&format!("mid:{}", id(9))).is_none());
        // A move during a sync of its folder: that sync fetched it first; the file goes, one copy.
        let ten = inbox_file(10);
        let held = crate::fetch::lock(&crate::fetch::lock_path(&a)).unwrap();
        let moving = {
            let (a, ten) = (a.clone(), ten.clone());
            std::thread::spawn(move || act(&a, "x", &ten, &Action::Archive))
        };
        let place = (0..100).find_map(|_| on_server("Archive", 10).or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        let place = place.expect("moved on the server");
        let archive = a.maildir_path().join(folders(&a.id).into_iter().find(|f| f.name == "Archive").unwrap().local);
        maildir::store(&archive, &maildir::Fetched { origin: place, flags: String::new(), received: None, raw: &raw(10) }).unwrap();
        drop(held);
        let moved = moving.join().unwrap().unwrap();
        assert_eq!(moved.len(), 1);
        placed(&a, 10, "Archive");
        // A sync holding the lock past the wait: the file goes, on its way; the copy then comes.
        LOCK_WAIT_SHORT.store(300, std::sync::atomic::Ordering::Relaxed);
        let held = crate::fetch::lock(&crate::fetch::lock_path(&a)).unwrap();
        assert!(act(&a, "x", &inbox_file(11), &Action::Archive).unwrap().is_empty());
        assert_eq!(find(11), mailfind::Found::OnItsWay { account: a.id.clone(), folder: "Archive".into() });
        drop(held);
        LOCK_WAIT_SHORT.store(0, std::sync::atomic::Ordering::Relaxed);
        crate::fetch::sync(&a, "x").unwrap();
        placed(&a, 11, "Archive");
        assert!(matches!(find(11), mailfind::Found::Here(_)));
        // What stays in the inbox, once: the twelfth.
        assert_eq!(one(&a, 12).0, "INBOX");
        // A search's message, on its server only, moved into another address:
        // its copy written there at once, under the place that server gave it.
        crate::fetch::block_on(async {
            let mut session = imap::open(&server, "x").await?;
            imap::within(COMMAND, session.append("INBOX", None, None, raw(13))).await?.map_err(imap::server)?;
            let _ = session.logout().await;
            Ok(())
        })
        .unwrap();
        let away = on_server("INBOX", 13).expect("on its server");
        assert!(copies(&a, 13).is_empty(), "not here before");
        move_across_from_server(&a, "x", "INBOX", away, &c, "x", "INBOX").unwrap();
        assert!(on_server("INBOX", 13).is_none() && copies(&a, 13).is_empty());
        let (there, file) = one(&c, 13);
        assert_eq!((there.as_str(), maildir::origin_of(&file)), ("INBOX", on_server_of(&c, "INBOX", 13)));
        let other = crate::fetch::sync(&c, "x").unwrap();
        assert!(other.new.is_empty() && other.elsewhere.is_empty(), "{other:?}");
        assert_eq!(one(&c, 13).1, file);
    }

    /// A set's places paired by Message-ID, never by the order of the
    /// server's text (review of 8b57a35, M1): a range written high to low,
    /// which imap-proto reads lowest first, paired right; a Message-ID twice
    /// in the set or at the destination, none, or a destination COPYUID did
    /// not name: unpaired.
    #[test]
    fn a_set_is_paired_by_its_message_ids() {
        let Some(Answered::Copied(placed)) = code_of("a9 OK [COPYUID 7 4:2 10:12] Done\r\n") else { panic!() };
        assert_eq!(placed.uids, BTreeMap::from([(2, 10), (3, 11), (4, 12)]), "the text's order, lowest first");
        let map = |pairs: &[(u32, &str)]| pairs.iter().map(|(uid, id)| (*uid, if id.starts_with("sioul:") { id.to_string() } else { format!("mid:{id}@example.org") })).collect::<BTreeMap<u32, String>>();
        // The server meant 4→10, 3→11, 2→12: the Message-IDs say so.
        let sources = map(&[(2, "a"), (3, "b"), (4, "c")]);
        assert_eq!(paired_by_ids(&placed, &sources, &map(&[(10, "c"), (11, "b"), (12, "a")])).uids, BTreeMap::from([(2, 12), (3, 11), (4, 10)]));
        // Two messages of one Message-ID: neither; the third, yes.
        assert_eq!(paired_by_ids(&placed, &map(&[(2, "a"), (3, "a"), (4, "c")]), &map(&[(10, "c"), (11, "a"), (12, "a")])).uids, BTreeMap::from([(4, 10)]));
        // ... even where the destination shows that Message-ID once: two files never share one place.
        assert_eq!(paired_by_ids(&placed, &map(&[(2, "a"), (3, "a"), (4, "c")]), &map(&[(10, "c"), (11, "a"), (12, "x")])).uids, BTreeMap::from([(4, 10)]));
        // One Message-ID twice at the destination, or not there: unpaired.
        assert_eq!(paired_by_ids(&placed, &sources, &map(&[(10, "c"), (11, "a"), (12, "a")])).uids, BTreeMap::from([(4, 10)]));
        // A message without a Message-ID (a digest), and a destination COPYUID did not name: never.
        let digest = "sioul:mail/home/0123456789abcdef";
        assert_eq!(paired_by_ids(&placed, &map(&[(2, digest), (3, "b"), (4, "c")]), &map(&[(10, "c"), (11, "b"), (12, digest), (99, "d")])).uids, BTreeMap::from([(3, 11), (4, 10)]));
        assert_eq!(paired_by_ids(&placed, &map(&[(2, "a"), (3, "b"), (4, "d")]), &map(&[(10, "c"), (11, "b"), (12, "a"), (99, "d")])).uids, BTreeMap::from([(2, 12), (3, 11)]));
        // Nothing read at the destination: none.
        assert!(paired_by_ids(&placed, &sources, &BTreeMap::new()).uids.is_empty());
    }

    #[test]
    fn uid_sets_are_short() {
        let uids: BTreeSet<u32> = [1, 2, 3, 4, 5, 7, 9, 10, 11, 12].into_iter().collect();
        assert_eq!(ranges(&uids), "1:5,7,9:12");
        assert_eq!(ranges(&[42].into_iter().collect()), "42");
    }
}

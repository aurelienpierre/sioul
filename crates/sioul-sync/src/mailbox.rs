// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An account's folders, and what you do to its messages.
//!
//! Sioul writes to the server only when you act: a flag with `UID STORE`, a
//! move with `UID MOVE` (RFC 6851), else a copy, a deletion flag and `UID
//! EXPUNGE` (RFC 4315). The local copy follows: a flag renames its file; a
//! moved message leaves this folder, and the next sync brings it into the other.
//! Changes made elsewhere (read on the phone, deleted in the webmail) come back
//! at each sync (`reconcile`).

use crate::SyncError;
use crate::imap::{self, COMMAND, Imap, Server};
use async_imap::types::NameAttribute;
use futures_util::StreamExt;
use sioul_core::card::ImapOrigin;
use sioul_core::config::{Account, state_dir};
use sioul_core::folders::{self, Folder, Role};
use sioul_core::maildir;
use sioul_core::spam::labels;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
fn own_dir(account: &Account, folder: &Folder) -> Option<PathBuf> {
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
        return Err(SyncError::Server("a folder needs a name".into()));
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
    let folder = folders(&account.id).into_iter().find(|f| f.name == name).ok_or_else(|| SyncError::Server(format!("{name}: no such folder")))?;
    if folder.role != Role::Other {
        return Err(SyncError::Server(format!("{}: a folder with a purpose stays", folder.display)));
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
    state_dir().join("sync").join(format!("{account}.folders.toml"))
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
    let saved: Saved = std::fs::read_to_string(folders_path(account)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    if saved.folders.is_empty() { vec![folders::folder("INBOX", None, None)] } else { saved.folders }
}

/// The folder a stored message belongs to, from where its file is.
pub fn folder_of(account: &Account, file: &Path) -> Option<Folder> {
    let directory = file.parent()?.parent()?;
    let root = account.maildir_path();
    folders(&account.id).into_iter().find(|f| root.join(&f.local) == directory)
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
    /// Not spam, said of mail set aside as spam: marked `$NotJunk` (and no
    /// longer `$Junk`), it stays where it is, and the Porch puts it back in its lane.
    NotSpam,
}

/// Does `action` to the message stored at `file`, on the server, then here.
/// What it says of the message, junk or not, goes into the spam filter's
/// label log (`spam::labels`) once the server has taken it.
pub fn act(account: &Account, password: &str, file: &Path, action: &Action) -> Result<(), SyncError> {
    let server = Server::of(account)?;
    let origin = maildir::origin_of(file).ok_or_else(|| SyncError::Server("not a message fetched by Sioul".into()))?;
    let folder = folder_of(account, file).ok_or_else(|| SyncError::Server("unknown folder".into()))?;
    if *action == Action::Archive && matches!(folder.role, Role::Archive | Role::All) {
        return Ok(());
    }
    // Read here before the message leaves this folder; written once the server took the act.
    let label = said(action).and_then(|source| labels::Entry::of_file(&account.id, &folder.name, file, source));
    crate::fetch::block_on(async {
        let mut session = imap::open(&server, password).await?;
        let result = act_in(&mut session, account, &folder, origin, action).await;
        let _ = session.logout().await;
        result
    })?;
    // Done on the server: a log that cannot be written is said, after the rest is done here.
    let logged = label.as_ref().map_or(Ok(()), labels::append).map_err(SyncError::Disk);
    // A sync may have renamed the file meanwhile; gone, there is nothing left to do here.
    let Some(file) = maildir::locate(file) else { return logged };
    match kept_with(action, &maildir::flags_of(&file)) {
        Some(flags) => {
            maildir::set_flags(&file, &flags).map_err(|e| SyncError::Disk(e.to_string()))?;
        }
        None => {
            let _ = std::fs::remove_file(&file);
        }
    }
    logged
}

/// What becomes of the local copy once the server took `action`: its letters
/// now, or none when it left this folder (the next sync brings the copy into
/// the other). Said not spam, it keeps its keyword here as on the server.
fn kept_with(action: &Action, flags: &str) -> Option<String> {
    match action {
        Action::Read(read) => Some(if *read { format!("{flags}S") } else { flags.replace('S', "") }),
        Action::Flag(flag) => Some(if *flag { format!("{flags}F") } else { flags.replace('F', "") }),
        Action::NotSpam => Some(format!("{}{}", flags.replace(maildir::JUNK, ""), maildir::NOT_JUNK)),
        Action::Move(_) | Action::Trash | Action::Junk | Action::Archive | Action::NotJunk => None,
    }
}

/// Writes into the spam filter's label log what you said of the message
/// stored at `file`, without acting on it there: a sender blocked from it.
pub fn label(account: &Account, file: &Path, source: labels::Source) -> Result<(), SyncError> {
    let folder = folder_of(account, file).ok_or_else(|| SyncError::Server("unknown folder".into()))?;
    let entry = labels::Entry::of_file(&account.id, &folder.name, file, source).ok_or_else(|| SyncError::Server("not a message fetched by Sioul".into()))?;
    labels::append(&entry).map_err(SyncError::Disk)
}

/// What an act says of a message, for the label log: none for those that say nothing of spam.
fn said(action: &Action) -> Option<labels::Source> {
    match action {
        Action::Junk => Some(labels::Source::Junk),
        Action::NotJunk => Some(labels::Source::NotJunk),
        Action::NotSpam => Some(labels::Source::NotSpam),
        Action::Read(_) | Action::Flag(_) | Action::Move(_) | Action::Trash | Action::Archive => None,
    }
}

/// Moves the message stored at `file` into `folder` of another account: a
/// copy is appended there, its flags kept, and only once that server has it
/// is the original taken off the first one. Nothing is lost on the way: if the
/// copy fails, the original stays; if removing the original fails, both remain.
pub fn move_across(from: &Account, from_password: &str, file: &Path, to: &Account, to_password: &str, folder: &str) -> Result<(), SyncError> {
    let raw = std::fs::read(file).map_err(|e| SyncError::Disk(format!("{}: {e}", file.display())))?;
    let origin = maildir::origin_of(file).ok_or_else(|| SyncError::Server("not a message fetched by Sioul".into()))?;
    let source = folder_of(from, file).ok_or_else(|| SyncError::Server("unknown folder".into()))?;
    let local = maildir::flags_of(file);
    let flags: Vec<&str> = [('S', "\\Seen"), ('F', "\\Flagged"), ('R', "\\Answered")].into_iter().filter(|(c, _)| local.contains(*c)).map(|(_, f)| f).collect();
    let flags = format!("({})", flags.join(" "));
    let target = Server::of(to)?;
    crate::fetch::block_on(async {
        let mut session = imap::open(&target, to_password).await?;
        let result = imap::within(COMMAND, session.append(folder, Some(&flags), None, &raw)).await?.map_err(imap::server);
        let _ = session.logout().await;
        result
    })?;
    let server = Server::of(from)?;
    crate::fetch::block_on(async {
        let mut session = imap::open(&server, from_password).await?;
        let result = remove_in(&mut session, &source, origin).await;
        let _ = session.logout().await;
        result
    })?;
    if let Some(file) = maildir::locate(file) {
        let _ = std::fs::remove_file(&file);
    }
    Ok(())
}

/// Takes a message off its folder for good: marked deleted, then expunged.
async fn remove_in(session: &mut Imap, folder: &Folder, origin: ImapOrigin) -> Result<(), SyncError> {
    let selected = imap::within(COMMAND, session.select(&folder.name)).await?.map_err(imap::server)?;
    if selected.uid_validity.unwrap_or(0) != origin.validity {
        return Err(SyncError::Server("the server renumbered this folder; fetch the mail again".into()));
    }
    let uid = origin.uid.to_string();
    let capabilities = imap::within(COMMAND, session.capabilities()).await?.map_err(imap::server)?;
    store(session, &uid, "+FLAGS.SILENT (\\Deleted)").await?;
    expunge(session, &uid, capabilities.has_str("UIDPLUS")).await
}

async fn act_in(session: &mut Imap, account: &Account, folder: &Folder, origin: ImapOrigin, action: &Action) -> Result<(), SyncError> {
    let selected = imap::within(COMMAND, session.select(&folder.name)).await?.map_err(imap::server)?;
    if selected.uid_validity.unwrap_or(0) != origin.validity {
        return Err(SyncError::Server("the server renumbered this folder; fetch the mail again".into()));
    }
    let uid = origin.uid.to_string();
    let capabilities = imap::within(COMMAND, session.capabilities()).await?.map_err(imap::server)?;
    let (can_move, uidplus) = (capabilities.has_str("MOVE"), capabilities.has_str("UIDPLUS"));
    match action {
        Action::Read(read) => store(session, &uid, if *read { "+FLAGS.SILENT (\\Seen)" } else { "-FLAGS.SILENT (\\Seen)" }).await,
        Action::Flag(flag) => store(session, &uid, if *flag { "+FLAGS.SILENT (\\Flagged)" } else { "-FLAGS.SILENT (\\Flagged)" }).await,
        Action::Move(target) => move_to(session, &uid, target, can_move, uidplus).await,
        Action::Trash if folder.role == Role::Trash => {
            store(session, &uid, "+FLAGS.SILENT (\\Deleted)").await?;
            expunge(session, &uid, uidplus).await
        }
        Action::Trash => {
            let target = role_folder(session, account, Role::Trash, "Trash").await?;
            move_to(session, &uid, &target, can_move, uidplus).await
        }
        Action::Junk => {
            // Keywords are optional: a server without them still takes the move.
            let _ = store(session, &uid, "-FLAGS.SILENT ($NotJunk)").await;
            let _ = store(session, &uid, "+FLAGS.SILENT ($Junk)").await;
            let target = role_folder(session, account, Role::Junk, "Junk").await?;
            move_to(session, &uid, &target, can_move, uidplus).await
        }
        Action::Archive => {
            let known = folders(&account.id);
            let target = match folders::preferred(&known, Role::Archive).or(folders::preferred(&known, Role::All)) {
                Some(f) => f.name.clone(),
                None => role_folder(session, account, Role::Archive, "Archive").await?,
            };
            move_to(session, &uid, &target, can_move, uidplus).await
        }
        Action::NotJunk => {
            let _ = store(session, &uid, "-FLAGS.SILENT ($Junk)").await;
            let _ = store(session, &uid, "+FLAGS.SILENT ($NotJunk)").await;
            move_to(session, &uid, "INBOX", can_move, uidplus).await
        }
        // A server without keywords keeps nothing: the label log then says it, on this device.
        Action::NotSpam => {
            let _ = store(session, &uid, "-FLAGS.SILENT ($Junk)").await;
            let _ = store(session, &uid, "+FLAGS.SILENT ($NotJunk)").await;
            Ok(())
        }
    }
}

async fn store(session: &mut Imap, uid: &str, query: &str) -> Result<(), SyncError> {
    let mut answers = imap::within(COMMAND, session.uid_store(uid, query)).await?.map_err(imap::server)?;
    while let Some(answer) = answers.next().await {
        answer.map_err(imap::server)?;
    }
    Ok(())
}

async fn move_to(session: &mut Imap, uid: &str, target: &str, can_move: bool, uidplus: bool) -> Result<(), SyncError> {
    if can_move {
        return imap::within(COMMAND, session.uid_mv(uid, target)).await?.map_err(imap::server);
    }
    imap::within(COMMAND, session.uid_copy(uid, target)).await?.map_err(imap::server)?;
    store(session, uid, "+FLAGS.SILENT (\\Deleted)").await?;
    expunge(session, uid, uidplus).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_an_act_leaves_here_and_says() {
        // Said not spam: it stays, its `$Junk` letter traded for `$NotJunk`'s.
        assert_eq!(kept_with(&Action::NotSpam, &format!("S{}", maildir::JUNK)), Some(format!("S{}", maildir::NOT_JUNK)));
        assert_eq!(kept_with(&Action::NotSpam, ""), Some(maildir::NOT_JUNK.to_string()));
        // Read, flagged: the keywords stay beside the flags.
        assert_eq!(kept_with(&Action::Read(false), &format!("S{}", maildir::NOT_JUNK)), Some(maildir::NOT_JUNK.to_string()));
        // Moved: it leaves this folder.
        assert_eq!(kept_with(&Action::Junk, "S"), None);
        assert_eq!(kept_with(&Action::NotJunk, "S"), None);
        // What goes into the label log, and what does not.
        assert_eq!([&Action::Junk, &Action::NotJunk, &Action::NotSpam].map(said), [Some(labels::Source::Junk), Some(labels::Source::NotJunk), Some(labels::Source::NotSpam)]);
        assert_eq!([&Action::Read(true), &Action::Trash, &Action::Archive].map(said), [None, None, None]);
    }

    #[test]
    fn uid_sets_are_short() {
        let uids: BTreeSet<u32> = [1, 2, 3, 4, 5, 7, 9, 10, 11, 12].into_iter().collect();
        assert_eq!(ranges(&uids), "1:5,7,9:12");
        assert_eq!(ranges(&[42].into_iter().collect()), "42");
    }
}

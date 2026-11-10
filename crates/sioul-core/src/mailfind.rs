// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Finding a message by its address (the unified mailbox's §3.2), for Rust,
//! the command line and agents.
//!
//! A message is named beyond one device by its address: `mid:<Message-ID>`,
//! or, for one without a Message-ID, `sioul:mail/<account>/<digest>`
//! (`mailcache::digest`: the same on every device, since the server gives
//! each the same bytes). A [`MailRef`] carries the address and hints of
//! where this device last saw it (its file, its account, folder and UID),
//! tried first because they are cheap. [`find`] reads the folders' indexes
//! (`mailcache`), never a whole message, in this order:
//!
//! 1. the hinted file, if it is still there (`maildir::locate`: its flags may
//!    have renamed it) and holds that address;
//! 2. the hinted account, folder and UID, in the folder's current numbering;
//! 3. the address in the hinted account, the copy that ranks first (the
//!    inbox, your own folders, the archive, Sent, the junk, the trash, then
//!    Gmail's "All Mail": `Role::rank`), a copy of the current numbering
//!    before an old one;
//! 4. the address in any of your addresses, ranked the same;
//! 5. copies of one Message-ID from different senders: someone reused the
//!    id. The hint decides, else the copy that arrived first, and the answer
//!    says that another message carries the same id.
//!
//! No copy here, and Sioul moved it without learning where the server put
//! it: "on its way" to its folder, until that folder's next fetch brings it
//! (`mailmoved`, the unified mailbox's part 3); the servers are not asked.
//!
//! No copy here: [`find_or_ask`] asks each address's server, read only, the
//! way a search asks (`UID SEARCH HEADER Message-ID`), brings the copy that
//! ranks first here (search's own `bring`: unread stays unread), and finds
//! it again. "Gone" is said only when every server answered and none holds
//! it: this device keeps no record of what others removed. The servers are
//! never asked from the demo profile, and never for a digest (no header
//! names it). The window calls it through its one opener alone, off its
//! thread (sioul-app's openmail.rs, the unified mailbox's part 2); the
//! phone's background step never does.
//!
//! A hinted file is used only inside the configured addresses' Maildirs:
//! written as it is, `..` folded without asking the system, it must start
//! with one of them (within the hinted account's own when one is hinted, and
//! not inside another address's Maildir held within it), name a message file
//! of one of its folders (`cur/` or `new/`, plain names), and, resolved by the
//! system (links followed), still be inside that same Maildir. Anything else
//! is not looked at, and the search goes on by the address alone.

use crate::card::ImapOrigin;
use crate::config::Account;
use crate::folders::{Folder, Role};
use crate::mailcache;
use crate::mailindex::{Root, bare_id};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// What starts the address of a message without a Message-ID.
pub const DIGEST_SCHEME: &str = "sioul:mail/";

/// What names a message beyond this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    /// `mid:<Message-ID>`, the id bare.
    Id(String),
    /// `sioul:mail/<account>/<digest>`: a message without a Message-ID.
    Digest { account: String, digest: String },
}

impl Address {
    /// `mid:abc@example.org`, `<abc@example.org>`, `abc@example.org` (no
    /// separator of a path in it), `mid:abc%40example.org`, or
    /// `sioul:mail/<account>/<16 hexadecimal digits>`.
    pub fn parse(text: &str) -> Option<Address> {
        let text = text.trim();
        if let Some(rest) = text.strip_prefix(DIGEST_SCHEME) {
            let (account, digest) = rest.split_once('/')?;
            let account = crate::notes::decode(account);
            let digest = digest.to_ascii_lowercase();
            let plain = matches!(Path::new(&account).components().collect::<Vec<_>>().as_slice(), [Component::Normal(_)]);
            return (plain && digest.len() == 16 && digest.bytes().all(|b| b.is_ascii_hexdigit())).then_some(Address::Digest { account, digest });
        }
        let named = text.starts_with("mid:") || text.starts_with('<') || (text.contains('@') && !text.contains('/') && !text.contains('\\'));
        let id = bare_id(text);
        (named && !id.is_empty()).then_some(Address::Id(id))
    }

    /// As links write it.
    pub fn uri(&self) -> String {
        match self {
            Address::Id(id) => crate::links::mail_uri(id),
            Address::Digest { account, digest } => crate::links::digest_uri(account, digest),
        }
    }

    /// Whether a line of `account`'s folder is this message.
    fn names(&self, account: &str, line: &mailcache::Line) -> bool {
        match self {
            Address::Id(id) => line.message_id.as_deref().map(bare_id).is_some_and(|m| m == *id),
            Address::Digest { account: wanted, digest } => wanted == account && line.message_id.is_none() && line.digest.as_deref() == Some(digest.as_str()),
        }
    }
}

/// The address of a message of `account`, from its bytes (its headers are
/// enough): `mid:` when it has a Message-ID, else `sioul:mail/`.
pub fn address_of(account: &str, raw: &[u8]) -> String {
    let id = mail_parser::MessageParser::default().parse(raw).and_then(|m| m.message_id().map(bare_id)).filter(|id| !id.is_empty());
    match id {
        Some(id) => crate::links::mail_uri(&id),
        None => crate::links::digest_uri(account, &mailcache::digest(raw)),
    }
}

/// The address of a message file of `account`, from its headers.
pub fn address_of_file(account: &str, path: &Path) -> Option<String> {
    let mut headers = mailcache::headers_of(path)?;
    headers.extend_from_slice(b"\r\n\r\n");
    Some(address_of(account, &headers))
}

/// A message's address from its card (`mid:` by its Message-ID, else
/// `sioul:mail/` from its file's headers); "" when neither is known.
pub fn address_of_card(card: &crate::card::Card) -> String {
    match card.message_id.as_deref().map(bare_id).filter(|id| !id.is_empty()) {
        Some(id) => crate::links::mail_uri(&id),
        None => card.account.as_deref().zip(card.path.as_deref()).and_then(|(account, path)| address_of_file(account, path)).unwrap_or_default(),
    }
}

/// The address of a message of `account` from its line in its folder's
/// index: `mid:` by its Message-ID, else `sioul:mail/` by its digest.
pub fn address_of_line(account: &str, line: &mailcache::Line) -> Option<String> {
    match line.message_id.as_deref().map(bare_id).filter(|id| !id.is_empty()) {
        Some(id) => Some(crate::links::mail_uri(&id)),
        None => line.digest.as_deref().map(|digest| crate::links::digest_uri(account, digest)),
    }
}

/// A message as a page, a tie, a notification or an agent names it: its
/// address, the only part ever written anywhere, and hints of where this
/// device last saw it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailRef {
    /// `mid:…` or `sioul:mail/…`; empty when only a file is known (an older
    /// `file:` key, a path an agent gives).
    pub address: String,
    pub account: Option<String>,
    /// The folder, as its server names it (`INBOX`, `[Gmail]/All Mail`).
    pub folder: Option<String>,
    pub uid: Option<u32>,
    pub file: Option<PathBuf>,
}

impl MailRef {
    /// From what names a message in a text: an address, an older `file:`
    /// key (a money row's, a tie's), or a message file's path.
    pub fn of(text: &str) -> MailRef {
        let text = text.trim();
        if Address::parse(text).is_some() {
            return MailRef { address: text.to_string(), ..MailRef::default() };
        }
        let file = text.strip_prefix("file:").map(|rest| rest.strip_prefix("//").unwrap_or(rest)).unwrap_or(text);
        MailRef { file: (!file.is_empty()).then(|| PathBuf::from(file)), ..MailRef::default() }
    }
}

/// Where a copy found is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Here {
    pub path: PathBuf,
    /// Its address (`mid:…`, `sioul:mail/…`).
    pub address: String,
    pub account: String,
    /// Its folder, as its server names it (else its directory's name).
    pub folder: String,
    pub role: Role,
    /// Another message, from another sender, carries the same Message-ID.
    pub reused: bool,
}

/// What finding a message answers (§3.2's table, and §3.5's "on its way").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// A copy on this device.
    Here(Here),
    /// No copy here yet: Sioul moved it into `folder` of `account` without
    /// learning where the server put it (`mailmoved`); its copy comes with
    /// that folder's next fetch. The servers are not asked meanwhile.
    OnItsWay { account: String, folder: String },
    /// No copy here: a folder kept on the server only, older mail not
    /// fetched yet, or a deletion; the servers say which (`find_or_ask`).
    NotHere,
    /// The servers hold it in none of the folders.
    Gone,
    /// A server could not be reached, or the demo profile, which asks none.
    Unreachable,
    /// An agent asked for mail kept from agents (said by the MCP, as before).
    Kept,
}

/// One copy of a message here, as the folders' indexes give it.
#[derive(Debug, Clone)]
struct Copy {
    path: PathBuf,
    address: String,
    account: String,
    folder: String,
    role: Role,
    arrived: i64,
    sender: String,
    /// Its Date and Subject: with its sender, what tells another message from a copy.
    date: Option<i64>,
    subject: String,
    origin: Option<ImapOrigin>,
    current: bool,
}

/// Two copies of one Message-ID that are different messages (someone reused
/// the id): their senders differ, and so does their Date or their Subject. A
/// list that rewrites the sender ("Ana via the list") and a copy sent
/// directly carry the same Date and Subject: one message (review of 05ccd0a, L11).
fn different(a: (&str, Option<i64>, &str), b: (&str, Option<i64>, &str)) -> bool {
    !a.0.eq_ignore_ascii_case(b.0) && (a.1 != b.1 || a.2.trim() != b.2.trim())
}

impl Copy {
    fn marks(&self) -> (&str, Option<i64>, &str) {
        (&self.sender, self.date, &self.subject)
    }

    /// The order copies rank in: the folder's purpose, the current numbering, the first to arrive.
    fn rank(&self) -> (u8, bool, i64, PathBuf) {
        (self.role.rank(), !self.current, self.arrived, self.path.clone())
    }

    fn here(&self, reused: bool) -> Here {
        Here { path: self.path.clone(), address: self.address.clone(), account: self.account.clone(), folder: self.folder.clone(), role: self.role, reused }
    }
}

/// The UIDVALIDITY of each folder as the last sync of `account` recorded it
/// (`$XDG_STATE_HOME/sioul/sync/<account>.toml`), by the folder's server name.
fn validities(account: &str) -> BTreeMap<String, u32> {
    #[derive(serde::Deserialize, Default)]
    struct State {
        #[serde(default)]
        folders: BTreeMap<String, FolderState>,
    }
    #[derive(serde::Deserialize)]
    struct FolderState {
        uidvalidity: u32,
    }
    let path = crate::config::state_dir().join("sync").join(format!("{account}.toml"));
    let state: State = std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default();
    state.folders.into_iter().map(|(name, f)| (name, f.uidvalidity)).collect()
}

/// The folder of `root` whose directory is `local`, as its server names it.
fn folder_of<'a>(root: &'a Root, local: &str) -> Option<&'a Folder> {
    root.folders.iter().find(|f| f.local == local)
}

fn role_of(root: &Root, local: &str) -> Role {
    if local.is_empty() {
        return Role::Inbox;
    }
    folder_of(root, local).map_or_else(|| crate::folders::role_of(local.trim_start_matches('.'), None), |f| f.role)
}

/// Every copy of `address` in these Maildirs, from their folders' indexes.
fn copies(roots: &[Root], address: &Address) -> Vec<Copy> {
    let mut out = Vec::new();
    for (i, root) in roots.iter().enumerate() {
        if let Address::Digest { account, .. } = address
            && *account != root.account
        {
            continue;
        }
        let validities = validities(&root.account);
        for local in mailcache::folders(&root.path) {
            // A folder that is another address's Maildir (held within this one) is that address's.
            let dir = root.path.join(&local);
            if roots.iter().enumerate().any(|(j, other)| j != i && other.path != root.path && other.path.starts_with(&root.path) && dir.starts_with(&other.path)) {
                continue;
            }
            let lines = mailcache::folder(&root.account, &root.path, &local);
            let folder = folder_of(root, &local);
            // The current numbering: as the last sync recorded it, else the newest one here.
            let current = folder.and_then(|f| validities.get(&f.name).copied()).or_else(|| lines.iter().filter_map(|(path, _)| crate::maildir::origin_of(path)).map(|o| o.validity).max());
            for (path, line) in lines {
                if !address.names(&root.account, &line) {
                    continue;
                }
                let origin = crate::maildir::origin_of(&path);
                out.push(Copy {
                    address: address.uri(),
                    account: root.account.clone(),
                    folder: folder.map_or_else(|| local.trim_start_matches('.').to_string(), |f| f.name.clone()),
                    role: role_of(root, &local),
                    arrived: line.arrived,
                    sender: line.from_address.clone().unwrap_or_default().to_ascii_lowercase(),
                    date: line.date,
                    subject: line.subject.clone(),
                    current: origin.is_none_or(|o| current.is_none_or(|c| c == o.validity)),
                    origin,
                    path,
                });
            }
        }
    }
    out
}

/// The numbering of the copy that took the place of `old`, a copy of
/// `account`'s `folder` removed with the folder's old numbering, as the
/// removals' log says it (`$XDG_STATE_HOME/sioul/sync/<account>.removed.log`:
/// when, the folder, `U<validity>-<uid>` of the old copy, of the new or "-").
fn renumbered_to(account: &str, folder: &str, old: ImapOrigin) -> Option<ImapOrigin> {
    let log = std::fs::read_to_string(crate::config::state_dir().join("sync").join(format!("{account}.removed.log"))).ok()?;
    let wanted = format!("U{}-{}", old.validity, old.uid);
    log.lines().rev().find_map(|line| {
        let fields: Vec<&str> = line.split('\t').collect();
        let [_, logged, gone, new] = fields[..] else { return None };
        if logged != folder || gone != wanted {
            return None;
        }
        let (validity, uid) = new.strip_prefix('U')?.split_once('-')?;
        Some(ImapOrigin { validity: validity.parse().ok()?, uid: uid.parse().ok()? })
    })
}

/// The numberings a renumbering of `account`'s folder `folder` (its server
/// name) replaced by `new` (part 0's rule, `<account>.removed.log`): each old
/// copy's in that folder, removed once `new` came in its place. A row an
/// older Sioul keyed by the old copy's file (`file:…`) is the same message
/// (budget's `mail_lines`; review of mailbox part 0, L4). Another folder's
/// lines never count (review of wt/sync, R6: two folders renumbered alike
/// share their old numberings).
pub fn renumbered_from(account: &str, folder: &str, new: ImapOrigin) -> Vec<ImapOrigin> {
    let Ok(log) = std::fs::read_to_string(crate::config::state_dir().join("sync").join(format!("{account}.removed.log"))) else { return Vec::new() };
    // The folder as the log writes it (`renumbered::remove`).
    let folder = folder.replace(['\t', '\n', '\r'], " ");
    let wanted = format!("U{}-{}", new.validity, new.uid);
    log.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let [_, logged, gone, now] = fields[..] else { return None };
            if logged != folder || now != wanted {
                return None;
            }
            let (validity, uid) = gone.strip_prefix('U')?.split_once('-')?;
            Some(ImapOrigin { validity: validity.parse().ok()?, uid: uid.parse().ok()? })
        })
        .collect()
}

/// The server's name of the folder of `account` whose Maildir++ directory is
/// `dir` (`<Maildir>` for the inbox, `<Maildir>/.Sent`), as its last sync
/// saved its folders (`folders::saved`); none when not known.
pub fn folder_named(account: &str, dir: &Path) -> Option<String> {
    let name = dir.file_name()?.to_str()?;
    let local = if name.starts_with('.') { name } else { "" };
    crate::folders::saved(account).into_iter().find(|f| f.local == local).map(|f| f.name)
}

/// A path with its "." and ".." taken away as they are written, without
/// asking the system (which may follow a link, or reach another computer).
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(part);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// A hinted file kept inside the Maildirs: the address it is in, its
/// folder's directory, the unique part of its name, and its file now (none
/// once it is gone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inside {
    pub account: String,
    pub local: String,
    pub unique: String,
    pub now: Option<PathBuf>,
}

/// The hinted `file`, when it is a message file of one of the Maildirs of
/// `roots` (of `account`'s own, when one is hinted); none otherwise.
pub fn inside(roots: &[Root], file: &Path, account: Option<&str>) -> Option<Inside> {
    if !file.is_absolute() {
        return None;
    }
    let written = lexical(file);
    let canonical = |p: &Path| std::fs::canonicalize(p).ok();
    // The most precise Maildir it is written in: a Maildir held within another is its own.
    let starts = |root: &Root| written.starts_with(lexical(&root.path)) || canonical(&root.path).is_some_and(|c| written.starts_with(c));
    let root = roots.iter().filter(|r| starts(r)).max_by_key(|r| r.path.components().count())?;
    if account.is_some_and(|a| a != root.account) {
        return None;
    }
    let base = if written.starts_with(lexical(&root.path)) { lexical(&root.path) } else { canonical(&root.path)? };
    let parts: Vec<String> = written.strip_prefix(&base).ok()?.components().map(|c| match c {
        Component::Normal(part) => part.to_str().map(str::to_string),
        _ => None,
    }).collect::<Option<Vec<_>>>()?;
    // <folder…>/cur|new/<name>, the folders plain names, never `tmp`.
    let (name, rest) = parts.split_last()?;
    let (sub, folders) = rest.split_last()?;
    if !matches!(sub.as_str(), "cur" | "new") || name.starts_with('.') || folders.len() > 8 || folders.iter().any(|f| matches!(f.as_str(), "cur" | "new" | "tmp")) {
        return None;
    }
    let local = folders.join("/");
    // Resolved by the system, links followed: still inside that Maildir, the
    // file as written (a link to a file elsewhere is refused) and its folder.
    let real_root = canonical(&root.path)?;
    let within = |p: &Path| canonical(p).is_some_and(|real| real.starts_with(&real_root));
    if std::fs::symlink_metadata(&written).is_ok() && !within(&written) {
        return None;
    }
    if written.parent().and_then(Path::parent).is_some_and(|dir| dir.exists() && !within(dir)) {
        return None;
    }
    let now = crate::maildir::locate(&written).filter(|found| within(found));
    Some(Inside { account: root.account.clone(), local, unique: crate::maildir::unique_part(name).to_string(), now })
}

/// Finds a message here, from the folders' indexes of `roots`
/// (`MailIndex::roots_of`), never reading a whole message (the module's steps).
pub fn find(roots: &[Root], wanted: &MailRef) -> Found {
    let hint = wanted.file.as_deref().and_then(|f| inside(roots, f, wanted.account.as_deref()));
    let address = match Address::parse(&wanted.address) {
        Some(address) => address,
        // Only a file (an older `file:` key, a path): its address, from its
        // headers, else from its line kept in its folder's index (it may have
        // gone with a renumbering since).
        None => {
            let Some(hint) = &hint else { return Found::NotHere };
            let address_of_line = |line: &mailcache::Line| match line.message_id.as_deref().map(bare_id).filter(|id| !id.is_empty()) {
                Some(id) => crate::links::mail_uri(&id),
                None => crate::links::digest_uri(&hint.account, line.digest.as_deref().unwrap_or_default()),
            };
            let from_file = hint.now.as_deref().and_then(|now| address_of_file(&hint.account, now));
            let from_line = || mailcache::line_of(&hint.account, &hint.local, &hint.unique).map(|line| address_of_line(&line));
            // Removed with its folder's old numbering (part 0's rule): the copy that took its place, by its line.
            let from_renumbering = || {
                let root = roots.iter().find(|r| r.account == hint.account)?;
                let folder = folder_of(root, &hint.local).map(|f| f.name.clone())?;
                let old = hint.unique.split('.').find(|part| part.starts_with('U')).and_then(|u| u[1..].split_once('-')).and_then(|(v, u)| Some(ImapOrigin { validity: v.parse().ok()?, uid: u.parse().ok()? }))?;
                let new = renumbered_to(&hint.account, &folder, old)?;
                mailcache::folder(&hint.account, &root.path, &hint.local).into_iter().find(|(path, _)| crate::maildir::origin_of(path) == Some(new)).map(|(_, line)| address_of_line(&line))
            };
            match from_file.or_else(from_line).or_else(from_renumbering).and_then(|a| Address::parse(&a)) {
                Some(address) => address,
                None => return Found::NotHere,
            }
        }
    };
    let all = copies(roots, &address);
    if all.is_empty() {
        // Moved by Sioul, its copy still on its way (`mailmoved`).
        return crate::mailmoved::of(&address.uri()).map_or(Found::NotHere, |m| Found::OnItsWay { account: m.account, folder: m.folder });
    }
    let reused = all.iter().any(|a| all.iter().any(|b| different(a.marks(), b.marks())));
    // 1. The hinted file, where it is now, holding this address.
    if let Some(now) = hint.as_ref().and_then(|h| h.now.as_ref())
        && let Some(copy) = all.iter().find(|c| c.path == *now)
    {
        return Found::Here(copy.here(reused));
    }
    // 2. The hinted account, folder and UID, in the folder's current numbering.
    if let (Some(account), Some(folder), Some(uid)) = (&wanted.account, &wanted.folder, wanted.uid)
        && let Some(copy) = all.iter().find(|c| c.account == *account && c.folder == *folder && c.current && c.origin.is_some_and(|o| o.uid == uid))
    {
        return Found::Here(copy.here(reused));
    }
    // 3 and 4. In the hinted account, else anywhere, the copy that ranks first.
    let in_account: Vec<&Copy> = all.iter().filter(|c| wanted.account.as_ref().is_some_and(|a| *a == c.account)).collect();
    let mut candidates: Vec<&Copy> = if in_account.is_empty() { all.iter().collect() } else { in_account };
    // 5. One id carried by two different messages: the copy that arrived first decides which message it is.
    if let Some(first) = candidates.iter().min_by_key(|c| (c.arrived, c.rank())).map(|c| (c.sender.clone(), c.date, c.subject.clone())) {
        candidates.retain(|c| !different(c.marks(), (&first.0, first.1, &first.2)));
    }
    candidates.into_iter().min_by_key(|c| c.rank()).map_or(Found::NotHere, |c| Found::Here(c.here(reused)))
}

/// A copy a server holds of a message (`Servers::copies`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCopy {
    pub folder: Folder,
    pub origin: ImapOrigin,
    /// Its Message-ID, as its headers write it.
    pub message_id: String,
    /// Its sender's address.
    pub sender: String,
    /// When it reached the server (INTERNALDATE, Unix seconds).
    pub arrived: i64,
    /// Its Date header (Unix seconds) and its Subject, as its header says them.
    pub date: Option<i64>,
    pub subject: String,
}

impl ServerCopy {
    fn marks(&self) -> (&str, Option<i64>, &str) {
        (&self.sender, self.date, &self.subject)
    }
}

/// The servers of your addresses, as `find_or_ask` asks them: read only.
pub trait Servers {
    /// The copies `account`'s server holds of the message whose Message-ID
    /// is `id`, in every folder it lists (`UID SEARCH HEADER Message-ID`).
    /// An error: the server could not be reached or asked.
    fn copies(&self, account: &Account, id: &str) -> Result<Vec<ServerCopy>, String>;
    /// That copy brought here, whole, into its folder (search's `bring`): its file.
    fn bring(&self, account: &Account, copy: &ServerCopy) -> Result<PathBuf, String>;
}

/// Whether this is the demo profile, which asks no server.
fn demo() -> bool {
    std::env::var_os("SIOUL_DEMO").is_some()
}

/// `find`, then, with no copy here, the servers of `accounts` asked (the
/// hinted account's first): the copy that ranks first brought and found
/// again; "gone" only when every server answered and none holds it.
pub fn find_or_ask(roots: &[Root], accounts: &[Account], wanted: &MailRef, servers: &dyn Servers) -> Found {
    ask(roots, accounts, wanted, servers, demo())
}

/// The servers asked, for a message `find` found no copy of here (`find_or_ask`'s second half).
pub fn ask_servers(roots: &[Root], accounts: &[Account], wanted: &MailRef, servers: &dyn Servers) -> Found {
    asked(roots, accounts, wanted, servers, demo())
}

fn ask(roots: &[Root], accounts: &[Account], wanted: &MailRef, servers: &dyn Servers, demo: bool) -> Found {
    let found = find(roots, wanted);
    if found != Found::NotHere {
        return found;
    }
    asked(roots, accounts, wanted, servers, demo)
}

fn asked(roots: &[Root], accounts: &[Account], wanted: &MailRef, servers: &dyn Servers, demo: bool) -> Found {
    let Some(Address::Id(id)) = Address::parse(&wanted.address) else { return Found::NotHere };
    if demo {
        return Found::Unreachable;
    }
    let mut asked: Vec<&Account> = accounts.iter().filter(|a| a.syncs()).collect();
    asked.sort_by_key(|a| wanted.account.as_ref() != Some(&a.id));
    let mut unreachable = false;
    let mut held: Vec<(&Account, ServerCopy)> = Vec::new();
    for account in asked {
        match servers.copies(account, &id) {
            Ok(copies) => held.extend(copies.into_iter().filter(|c| bare_id(&c.message_id) == id).map(|c| (account, c))),
            Err(_) => unreachable = true,
        }
    }
    if held.is_empty() {
        return if unreachable { Found::Unreachable } else { Found::Gone };
    }
    let reused = held.iter().any(|(_, a)| held.iter().any(|(_, b)| different(a.marks(), b.marks())));
    // The hinted account's copies, else all; the message of the first to arrive; then ranked.
    let in_account: Vec<&(&Account, ServerCopy)> = held.iter().filter(|(a, _)| wanted.account.as_ref() == Some(&a.id)).collect();
    let mut candidates: Vec<&(&Account, ServerCopy)> = if in_account.is_empty() { held.iter().collect() } else { in_account };
    if let Some(first) = candidates.iter().min_by_key(|(_, c)| c.arrived).map(|(_, c)| c.clone()) {
        candidates.retain(|(_, c)| !different(c.marks(), first.marks()));
    }
    let Some((account, copy)) = candidates.into_iter().min_by_key(|(_, c)| (c.folder.role.rank(), c.arrived, c.origin)) else { return Found::Gone };
    match servers.bring(account, copy) {
        Ok(path) => match find(roots, &MailRef { address: wanted.address.clone(), account: Some(account.id.clone()), file: Some(path.clone()), ..MailRef::default() }) {
            Found::Here(mut here) => {
                here.reused |= reused;
                Found::Here(here)
            }
            // Brought outside the indexes' reach (a folder not listed here): its file, as brought.
            _ => Found::Here(Here { path, address: Address::Id(id.clone()).uri(), account: account.id.clone(), folder: copy.folder.name.clone(), role: copy.folder.role, reused }),
        },
        Err(_) => Found::Unreachable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folders::folder;

    /// Stand-in addresses, each a Maildir under the test's own data folder,
    /// with an inbox, an archive, Sent, Gmail's "All Mail" and a label; their
    /// folders saved as a sync saves them, their numbering recorded.
    struct Stand {
        roots: Vec<Root>,
        accounts: Vec<Account>,
        base: PathBuf,
    }

    const VALIDITY: u32 = 7;

    impl Stand {
        fn new(name: &str, accounts: &[&str]) -> Stand {
            crate::config::stand_ins_only();
            let base = crate::config::data_dir().join(format!("find-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let folders = vec![
                folder("INBOX", Some("/"), Some(Role::Inbox)),
                folder("Archive", Some("/"), Some(Role::Archive)),
                folder("Sent", Some("/"), Some(Role::Sent)),
                folder("[Gmail]/All Mail", Some("/"), Some(Role::All)),
                folder("Lunch", Some("/"), None),
            ];
            let mut stand = Stand { roots: Vec::new(), accounts: Vec::new(), base: base.clone() };
            for id in accounts {
                let id = format!("{id}-{name}-{}", std::process::id());
                let path = base.join(&id);
                for f in &folders {
                    for sub in ["tmp", "new", "cur"] {
                        std::fs::create_dir_all(path.join(&f.local).join(sub)).unwrap();
                    }
                }
                let sync = crate::config::state_dir().join("sync");
                std::fs::create_dir_all(&sync).unwrap();
                let states: String = folders.iter().map(|f| format!("[folders.\"{}\"]\nuidvalidity = {VALIDITY}\nlast_uid = 99\n", f.name)).collect();
                std::fs::write(sync.join(format!("{id}.toml")), states).unwrap();
                let saved: String = folders.iter().map(|f| format!("[[folder]]\nname = \"{}\"\ndisplay = \"{}\"\nrole = \"{}\"\nlocal = \"{}\"\n", f.name, f.display, serde_json::to_value(f.role).unwrap().as_str().unwrap(), f.local)).collect();
                std::fs::write(crate::folders::saved_path(&id), saved).unwrap();
                let mut account = Account::imap(&id, "me@example.org", "imap.example.org", 993, crate::config::Security::Tls, None);
                account.maildir = Some(path.display().to_string());
                stand.roots.push(Root { account: id.clone(), path, folders: crate::folders::saved(&id) });
                stand.accounts.push(account);
            }
            stand
        }

        fn local(&self, folder: &str) -> String {
            self.roots[0].folders.iter().find(|f| f.name == folder).unwrap().local.clone()
        }

        /// A message `id` (none: no Message-ID) from `sender`, in `account`'s
        /// `folder`, numbered `validity`/`uid`, arrived at `arrived`.
        #[allow(clippy::too_many_arguments)]
        fn put(&self, account: usize, folder: &str, validity: u32, uid: u32, id: Option<&str>, sender: &str, arrived: i64) -> PathBuf {
            self.put_as(account, folder, validity, uid, id, sender, arrived, "Lunch")
        }

        /// As `put`, its Subject given.
        #[allow(clippy::too_many_arguments)]
        fn put_as(&self, account: usize, folder: &str, validity: u32, uid: u32, id: Option<&str>, sender: &str, arrived: i64, subject: &str) -> PathBuf {
            let local = self.local(folder);
            let path = self.roots[account].path.join(&local).join("cur").join(format!("{arrived}.U{validity}-{uid}.sioul{}2,S", crate::maildir::INFO));
            let id = id.map(|id| format!("Message-ID: <{id}>\r\n")).unwrap_or_default();
            std::fs::write(&path, format!("{id}From: <{sender}>\r\nTo: me@example.org\r\nDate: Fri, 30 Oct 2026 09:00:00 +0100\r\nSubject: {subject}\r\n\r\nNoon?\r\n")).unwrap();
            let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(arrived as u64);
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(time).unwrap();
            path
        }

        fn find(&self, wanted: MailRef) -> Found {
            find(&self.roots, &wanted)
        }
    }

    impl Drop for Stand {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
            for root in &self.roots {
                if let Some(dir) = mailcache::account_dir(&root.account) {
                    let _ = std::fs::remove_dir_all(dir);
                }
            }
        }
    }

    fn by(address: &str) -> MailRef {
        MailRef { address: address.into(), ..MailRef::default() }
    }

    fn path_of(found: &Found) -> PathBuf {
        match found {
            Found::Here(here) => here.path.clone(),
            other => panic!("not here: {other:?}"),
        }
    }

    /// Stand-in servers: what each address's server holds, by its Message-ID;
    /// bringing one writes it into its folder here, as a sync would.
    #[derive(Default)]
    struct Servers {
        held: Vec<(String, ServerCopy, String)>,
        down: bool,
        asked: std::cell::Cell<u32>,
    }

    impl super::Servers for Servers {
        fn copies(&self, account: &Account, id: &str) -> Result<Vec<ServerCopy>, String> {
            self.asked.set(self.asked.get() + 1);
            if self.down {
                return Err("unreachable".into());
            }
            Ok(self.held.iter().filter(|(a, c, _)| *a == account.id && c.message_id.contains(id)).map(|(_, c, _)| c.clone()).collect())
        }

        fn bring(&self, account: &Account, copy: &ServerCopy) -> Result<PathBuf, String> {
            let (_, _, raw) = self.held.iter().find(|(a, c, _)| *a == account.id && c == copy).ok_or("gone")?;
            let path = account.maildir_path().join(&copy.folder.local).join("new").join(format!("1777000000.U{}-{}.sioul", copy.origin.validity, copy.origin.uid));
            std::fs::write(&path, raw).map_err(|e| e.to_string())?;
            Ok(path)
        }
    }

    /// The ids' table (§2.5), a flag changed: the hinted file found where its flags renamed it; an older `file:` key finds it too.
    #[test]
    fn a_changed_flag_keeps_the_hinted_file() {
        let stand = Stand::new("a_changed_fl", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // A flag changed: the hinted file renamed, found where it is now.
        let first = stand.put(0, "INBOX", VALIDITY, 1, Some("flag@example.org"), "ana@example.org", 1_777_000_001);
        let renamed = first.with_file_name(format!("1777000001.U{VALIDITY}-1.sioul{}2,FS", crate::maildir::INFO));
        std::fs::rename(&first, &renamed).unwrap();
        assert_eq!(path_of(&stand.find(MailRef { file: Some(first.clone()), ..by("mid:flag@example.org") })), renamed);
        // Found by the file alone (an older `file:` key), its address read from it.
        let Found::Here(here) = stand.find(MailRef::of(&format!("file:{}", first.display()))) else { panic!() };
        assert_eq!((here.path, here.address.as_str()), (renamed.clone(), "mid:flag@example.org"));
    }

    /// The ids' table (§2.5), a move here: the hinted file gone, its copy in the folder it went to.
    #[test]
    fn a_move_here_finds_the_new_folder() {
        let stand = Stand::new("a_move_here_", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // A move here: the hinted inbox file gone, its copy in the archive.
        let moved = stand.put(0, "Archive", VALIDITY, 2, Some("moved@example.org"), "ana@example.org", 1_777_000_002);
        let gone = stand.roots[0].path.join("cur").join(format!("1777000002.U{VALIDITY}-9.sioul{}2,S", crate::maildir::INFO));
        let Found::Here(here) = stand.find(MailRef { file: Some(gone), ..by("mid:moved@example.org") }) else { panic!() };
        assert_eq!((here.path, here.role, here.folder.as_str()), (moved, Role::Archive, "Archive"));
    }

    /// The ids' table (§2.5), a renumbering: the current numbering's copy first; an older `file:` key to the old copy, removed, found through its line, then through the removals' log.
    #[test]
    fn a_renumbering_prefers_the_current_numbering() {
        let stand = Stand::new("a_renumberin", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // A renumbering: the old numbering's copy and the new one; the new
        // one first, a hint of the old UID taken in the current numbering only.
        let old = stand.put(0, "Lunch", VALIDITY - 1, 3, Some("renumbered@example.org"), "ana@example.org", 1_777_000_003);
        let new = stand.put(0, "Lunch", VALIDITY, 40, Some("renumbered@example.org"), "ana@example.org", 1_777_000_040);
        assert_eq!(path_of(&stand.find(MailRef { account: Some(home.clone()), folder: Some("Lunch".into()), uid: Some(3), ..by("mid:renumbered@example.org") })), new);
        assert_eq!(path_of(&stand.find(MailRef { account: Some(home.clone()), folder: Some("Lunch".into()), uid: Some(40), ..by("mid:renumbered@example.org") })), new);
        // The old copy removed (part 0's rule): an older `file:` key to it
        // still finds the message, through the line its folder's index kept,
        // and, once that line is compacted away, through the removals' log.
        std::fs::remove_file(&old).unwrap();
        let key = MailRef::of(&format!("file:{}", old.display()));
        assert_eq!(path_of(&stand.find(key.clone())), new);
        mailcache::folder(&home, &stand.roots[0].path, &stand.local("Lunch"));
        assert_eq!(mailcache::line_of(&home, &stand.local("Lunch"), crate::maildir::unique_part(&old.file_name().unwrap().to_string_lossy())), None, "compacted");
        assert_eq!(stand.find(key.clone()), Found::NotHere, "not logged");
        let log = crate::config::state_dir().join("sync").join(format!("{home}.removed.log"));
        std::fs::write(&log, format!("2026-11-01T10:00:00Z\tLunch\tU{}-3\tU{VALIDITY}-40\n", VALIDITY - 1)).unwrap();
        assert_eq!(path_of(&stand.find(key)), new);
    }

    /// The ids' table (§2.5), Gmail's labels: the inbox's copy before "All Mail" and a label's, unless the hint names another.
    #[test]
    fn gmail_s_labels_open_the_inbox_s_copy() {
        let stand = Stand::new("gmail_s_labe", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // Gmail's labels: the inbox's copy before "All Mail" and a label's; a hint to "All Mail" decides.
        let all = stand.put(0, "[Gmail]/All Mail", VALIDITY, 5, Some("labels@example.org"), "ana@example.org", 1_777_000_005);
        stand.put(0, "Lunch", VALIDITY, 6, Some("labels@example.org"), "ana@example.org", 1_777_000_005);
        let inbox = stand.put(0, "INBOX", VALIDITY, 7, Some("labels@example.org"), "ana@example.org", 1_777_000_005);
        assert_eq!(path_of(&stand.find(by("mid:<labels@example.org>"))), inbox);
        assert_eq!(path_of(&stand.find(MailRef { file: Some(all.clone()), ..by("mid:labels@example.org") })), all);
    }

    /// The ids' table (§2.5), a message to yourself: in Sent and the inbox, the inbox's copy, no reuse said.
    #[test]
    fn a_message_to_yourself_opens_the_inbox_s_copy() {
        let stand = Stand::new("a_message_to", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // A message to yourself: in Sent and the inbox, the inbox's copy.
        stand.put(0, "Sent", VALIDITY, 8, Some("self@example.org"), "me@example.org", 1_777_000_008);
        let received = stand.put(0, "INBOX", VALIDITY, 9, Some("self@example.org"), "me@example.org", 1_777_000_009);
        let Found::Here(here) = stand.find(by("mid:self@example.org")) else { panic!() };
        assert_eq!((here.path, here.reused), (received, false));
    }

    /// The ids' table (§2.5), two of your addresses on one list: the hinted address's copy, else the first ranked, then the first to arrive.
    #[test]
    fn two_addresses_on_one_list_keep_the_hinted_one() {
        let stand = Stand::new("two_addresse", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // Two of your addresses on one list: the hinted address's copy, else the first ranked, then the first to arrive.
        let at_home = stand.put(0, "INBOX", VALIDITY, 10, Some("list@example.org"), "list@example.org", 1_777_000_010);
        let at_work = stand.put(1, "INBOX", VALIDITY, 10, Some("list@example.org"), "list@example.org", 1_777_000_011);
        assert_eq!(path_of(&stand.find(MailRef { account: Some(work.clone()), ..by("mid:list@example.org") })), at_work);
        assert_eq!(path_of(&stand.find(by("mid:list@example.org"))), at_home);
    }

    /// The ids' table (§2.5), no Message-ID: found by its digest, in its own address only.
    #[test]
    fn a_message_without_an_id_is_found_by_its_digest() {
        let stand = Stand::new("a_message_wi", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // No Message-ID: its digest, in its address.
        let bare = stand.put(1, "Archive", VALIDITY, 12, None, "ana@example.org", 1_777_000_012);
        let address = address_of_file(&work, &bare).unwrap();
        assert!(address.starts_with(&format!("sioul:mail/{work}/")), "{address}");
        let Found::Here(here) = stand.find(by(&address)) else { panic!() };
        assert_eq!((here.path, here.address.clone()), (bare, address.clone()));
        // The same digest in another address's name: another message.
        assert_eq!(stand.find(by(&address.replace(&work, &home))), Found::NotHere);
    }

    /// The ids' table (§2.5), one id from two senders: the first to arrive, unless the hint says the other, said either way; a later look-alike ranked higher never takes it.
    #[test]
    fn one_id_from_two_senders_takes_the_first_to_arrive() {
        let stand = Stand::new("one_id_from_", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // One id carried by two different messages (another sender, another
        // Subject): the first to arrive, unless the hint says the other; said either way.
        let genuine = stand.put_as(0, "INBOX", VALIDITY, 13, Some("reused@example.org"), "bank@example.org", 1_777_000_013, "Your statement");
        let lookalike = stand.put_as(0, "INBOX", VALIDITY, 14, Some("reused@example.org"), "bank@lookalike.example", 1_777_000_014, "Your statement: act now");
        let Found::Here(here) = stand.find(by("mid:reused@example.org")) else { panic!() };
        assert_eq!((here.path, here.reused), (genuine.clone(), true));
        let Found::Here(here) = stand.find(MailRef { file: Some(lookalike.clone()), ..by("mid:reused@example.org") }) else { panic!() };
        assert_eq!((here.path, here.reused), (lookalike.clone(), true));
        // A later look-alike ranked higher (the inbox) never takes a tie made to the archived first one.
        std::fs::remove_file(&genuine).unwrap();
        let archived = stand.put_as(0, "Archive", VALIDITY, 15, Some("reused@example.org"), "bank@example.org", 1_777_000_013, "Your statement");
        assert_eq!(path_of(&stand.find(by("mid:reused@example.org"))), archived);
    }

    /// One message from two senders is no reused id: a list that rewrites
    /// its sender ("Ana via the list") and the copy sent directly carry the
    /// same Date and Subject. Not said reused, and the copy that ranks first
    /// (the inbox) is opened, though the list's arrived first (review of 05ccd0a, L11).
    #[test]
    fn a_list_s_copy_is_the_same_message() {
        let stand = Stand::new("a_list_s_cop", &["home"]);
        stand.put(0, "Lunch", VALIDITY, 1, Some("list-1@example.org"), "list@lists.example.org", 1_777_000_001);
        let direct = stand.put(0, "INBOX", VALIDITY, 2, Some("list-1@example.org"), "ana@example.org", 1_777_000_002);
        let Found::Here(here) = stand.find(by("mid:list-1@example.org")) else { panic!() };
        assert_eq!((here.path, here.reused), (direct, false));
        // On the servers alike: brought from the inbox, not said reused.
        let f = |name: &str| stand.roots[0].folders.iter().find(|f| f.name == name).unwrap().clone();
        let copy = |folder: &str, uid: u32, sender: &str, arrived: i64, subject: &str| {
            let raw = format!("Message-ID: <far-list@example.org>\r\nFrom: <{sender}>\r\nDate: Fri, 30 Oct 2026 09:00:00 +0100\r\nSubject: {subject}\r\n\r\nHi.\r\n");
            (stand.roots[0].account.clone(), ServerCopy { folder: f(folder), origin: ImapOrigin { validity: VALIDITY, uid }, message_id: "<far-list@example.org>".into(), sender: sender.into(), arrived, date: Some(1_793_347_200), subject: subject.into() }, raw)
        };
        let servers = Servers { held: vec![copy("Lunch", 60, "list@lists.example.org", 1_777_000_060, "Lunch"), copy("INBOX", 61, "ana@example.org", 1_777_000_061, "Lunch")], ..Servers::default() };
        let Found::Here(here) = ask(&stand.roots, &stand.accounts, &by("mid:far-list@example.org"), &servers, false) else { panic!() };
        assert_eq!((here.folder.as_str(), here.reused), ("INBOX", false));
        // Two different messages on the servers: still the first to arrive, said reused.
        let servers = Servers { held: vec![copy("Lunch", 70, "bank@example.org", 1_777_000_070, "Statement"), copy("INBOX", 71, "bank@lookalike.example", 1_777_000_071, "Statement: act now")], ..Servers::default() };
        let raw_id = |c: &mut (String, ServerCopy, String)| {
            c.1.message_id = "<far-reused@example.org>".into();
            c.2 = c.2.replace("far-list@", "far-reused@");
        };
        let mut held = servers.held;
        held.iter_mut().for_each(raw_id);
        let servers = Servers { held, ..Servers::default() };
        let Found::Here(here) = ask(&stand.roots, &stand.accounts, &by("mid:far-reused@example.org"), &servers, false) else { panic!() };
        assert_eq!((here.folder.as_str(), here.reused), ("Lunch", true));
    }

    /// The ids' table (§2.5), a move elsewhere: the servers asked, the copy that ranks first brought; gone only when every server answered; never from the demo profile; a digest never asked.
    #[test]
    fn a_move_elsewhere_asks_the_servers() {
        let stand = Stand::new("a_move_elsew", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let _ = (&home, &work);
        // A move elsewhere: no copy here. The servers asked; the copy that ranks first brought, unread.
        let held = |account: &str, folder: &str, uid: u32, id: &str, sender: &str, arrived: i64| {
            let f = stand.roots[0].folders.iter().find(|f| f.name == folder).unwrap().clone();
            let copy = ServerCopy { folder: f, origin: ImapOrigin { validity: VALIDITY, uid }, message_id: format!("<{id}>"), sender: sender.into(), arrived, date: None, subject: "Far".into() };
            (account.to_string(), copy, format!("Message-ID: <{id}>\r\nFrom: <{sender}>\r\nSubject: Far\r\n\r\nFar.\r\n"))
        };
        let servers = Servers {
            held: vec![
                held(&home, "[Gmail]/All Mail", 50, "far@example.org", "ana@example.org", 1_777_000_050),
                held(&home, "Lunch", 51, "far@example.org", "ana@example.org", 1_777_000_050),
                // Its id inside a longer one: never taken for it.
                held(&home, "INBOX", 52, "xfar@example.org", "ana@example.org", 1_777_000_050),
            ],
            ..Servers::default()
        };
        let Found::Here(here) = ask(&stand.roots, &stand.accounts, &by("mid:far@example.org"), &servers, false) else { panic!() };
        assert_eq!((here.folder.as_str(), here.role), ("Lunch", Role::Other));
        assert!(here.path.starts_with(stand.roots[0].path.join(stand.local("Lunch")).join("new")), "{:?}", here.path);
        // Here now: found again without asking.
        let asked = servers.asked.get();
        assert!(matches!(ask(&stand.roots, &stand.accounts, &by("mid:far@example.org"), &servers, false), Found::Here(_)));
        assert_eq!(servers.asked.get(), asked);
        // Deleted: said only when every server answered, and none holds it.
        assert_eq!(ask(&stand.roots, &stand.accounts, &by("mid:deleted@example.org"), &servers, false), Found::Gone);
        let down = Servers { down: true, ..Servers::default() };
        assert_eq!(ask(&stand.roots, &stand.accounts, &by("mid:deleted@example.org"), &down, false), Found::Unreachable);
        // The demo profile asks no server; a digest is never asked (no header names it).
        let before = servers.asked.get();
        assert_eq!(ask(&stand.roots, &stand.accounts, &by("mid:deleted@example.org"), &servers, true), Found::Unreachable);
        assert_eq!(ask(&stand.roots, &stand.accounts, &by(&format!("sioul:mail/{home}/0123456789abcdef")), &servers, false), Found::NotHere);
        assert_eq!(servers.asked.get(), before);
    }

    /// A move Sioul made without learning where the server put the message
    /// (the unified mailbox's part 3): "on its way" to its folder, never the
    /// servers asked, until its copy is here; then here.
    #[test]
    fn a_move_noted_is_on_its_way_until_its_copy_is_here() {
        let stand = Stand::new("a_move_noted", &["home"]);
        let home = stand.roots[0].account.clone();
        let address = format!("mid:on-its-way-{}@example.org", std::process::id());
        crate::mailmoved::note(&home, std::slice::from_ref(&address), "Archive", jiff::Timestamp::now().as_second()).unwrap();
        let servers = Servers::default();
        let on_its_way = Found::OnItsWay { account: home.clone(), folder: "Archive".into() };
        assert_eq!(stand.find(by(&address)), on_its_way);
        // Spelled otherwise, the same message.
        assert_eq!(stand.find(by(&address.replace('@', "%40"))), on_its_way);
        assert_eq!(ask(&stand.roots, &stand.accounts, &by(&address), &servers, false), on_its_way);
        assert_eq!(servers.asked.get(), 0, "the servers are not asked while it is on its way");
        // Its copy here: found, whatever the note still says.
        let copy = stand.put(0, "Archive", VALIDITY, 3, Some(address.trim_start_matches("mid:")), "ana@example.org", 1_777_000_003);
        assert_eq!(path_of(&stand.find(by(&address))), copy);
        crate::mailmoved::forget(&[address]).unwrap();
    }

    /// A hint outside the Maildirs is never looked at, in each form: `..`
    /// climbing out, a link pointing out (a file, a folder), an absolute
    /// path elsewhere, another address's directory, a relative path, `tmp/`.
    #[test]
    fn hints_outside_the_maildirs_are_refused() {
        let stand = Stand::new("hints", &["home", "work"]);
        let (home, work) = (stand.roots[0].account.clone(), stand.roots[1].account.clone());
        let mine = stand.put(0, "INBOX", VALIDITY, 1, Some("mine@example.org"), "ana@example.org", 1_777_000_001);
        let theirs = stand.put(1, "INBOX", VALIDITY, 1, Some("theirs@example.org"), "ana@example.org", 1_777_000_001);
        // Outside every Maildir: a file that looks like a message.
        let elsewhere = stand.base.join("elsewhere");
        for sub in ["cur", "new", "tmp"] {
            std::fs::create_dir_all(elsewhere.join(sub)).unwrap();
        }
        let outside = elsewhere.join("cur").join(format!("1777000001.U{VALIDITY}-1.sioul{}2,S", crate::maildir::INFO));
        std::fs::write(&outside, "Message-ID: <outside@example.org>\r\nFrom: <eve@example.org>\r\n\r\nHi\r\n").unwrap();
        let root = &stand.roots[0].path;
        let climbing = root.join("cur").join("..").join("..").join("elsewhere").join("cur").join(outside.file_name().unwrap());
        let relative = PathBuf::from("cur").join(mine.file_name().unwrap());
        let in_tmp = root.join("tmp").join(mine.file_name().unwrap());
        std::fs::copy(&mine, &in_tmp).unwrap();
        #[cfg(unix)]
        let (linked_file, linked_folder) = {
            let file = root.join("cur").join(format!("1777000009.U{VALIDITY}-9.sioul{}2,S", crate::maildir::INFO));
            std::os::unix::fs::symlink(&outside, &file).unwrap();
            let folder = root.join(".Linked");
            std::os::unix::fs::symlink(&elsewhere, &folder).unwrap();
            (file, folder.join("cur").join(outside.file_name().unwrap()))
        };
        let mut refused = vec![outside.clone(), climbing, relative, in_tmp];
        #[cfg(unix)]
        refused.extend([linked_file, linked_folder]);
        for file in &refused {
            assert_eq!(inside(&stand.roots, file, None), None, "{file:?}");
            // Its address alone counts then: "outside@" is in no Maildir.
            assert_eq!(stand.find(MailRef { file: Some(file.clone()), ..by("mid:outside@example.org") }), Found::NotHere, "{file:?}");
            assert_eq!(stand.find(MailRef { file: Some(file.clone()), ..MailRef::default() }), Found::NotHere, "{file:?}");
        }
        // Another address's directory, for this address's hint: refused; its own: taken.
        assert_eq!(inside(&stand.roots, &theirs, Some(&home)), None);
        assert!(inside(&stand.roots, &theirs, Some(&work)).is_some_and(|i| i.now.as_ref() == Some(&theirs)));
        assert_eq!(stand.find(MailRef { file: Some(theirs.clone()), account: Some(home.clone()), ..MailRef::default() }), Found::NotHere);
        assert!(inside(&stand.roots, &mine, None).is_some_and(|i| i.account == home && i.local.is_empty()));
        // A digest address naming an address that is not one plain name: nothing.
        assert_eq!(Address::parse("sioul:mail/../0123456789abcdef"), None);
        assert_eq!(Address::parse("sioul:mail/a%2Fb/0123456789abcdef"), None);
    }

    /// One digest for the whole program: the address, the index's column and
    /// a renumbered folder's pairing (sioul-sync's `renumbered::key_of`
    /// calls `mailcache::digest`, its own test compares them).
    #[test]
    fn one_digest_for_the_address_and_the_index() {
        let stand = Stand::new("digest", &["home"]);
        let file = stand.put(0, "INBOX", VALIDITY, 1, None, "ana@example.org", 1_777_000_001);
        let raw = std::fs::read(&file).unwrap();
        let line = mailcache::folder(&stand.roots[0].account, &stand.roots[0].path, "").remove(0).1;
        assert_eq!(line.digest.as_deref(), Some(mailcache::digest(&raw).as_str()));
        assert_eq!(address_of(&stand.roots[0].account, &raw), crate::links::digest_uri(&stand.roots[0].account, &mailcache::digest(&raw)));
        let address = address_of(&stand.roots[0].account, &raw);
        assert_eq!(Address::parse(&address).unwrap().uri(), address);
        // Links read it: its kind, one spelling, the message it names, from the index.
        let index = crate::mailindex::MailIndex::build_ranked(&stand.roots);
        assert_eq!(index.get(&address).map(|m| m.path.clone()), Some(file.clone()));
        let capitals = format!("sioul:mail/{}/{}", stand.roots[0].account, mailcache::digest(&raw).to_ascii_uppercase());
        assert_eq!(index.get(&capitals).map(|m| m.path.clone()), Some(file.clone()), "the digest in capitals");
        assert_eq!(crate::links::kind_of(&address), crate::links::Kind::Mail);
        let loaded = crate::links::Loaded { mail: index, ..crate::links::Loaded::default() };
        let world = loaded.world();
        let described = world.describe(&address);
        assert!(described.found && described.key == file.display().to_string() && described.title == "Lunch", "{described:?}");
        assert!(world.search("Lunch", None, "", 10).iter().any(|r| r.uri == address), "found when tying");
    }
}

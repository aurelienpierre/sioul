// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The training corpus: every message of every folder of every account, since
//! forever, kept small enough for any disk.
//!
//! Sioul's sync already brings all your mail, but never into the disk's
//! reserve, and whole messages since forever rarely fit. Training needs much
//! less: per message, the header block (at most 32 KB), its MIME structure
//! from BODYSTRUCTURE (the parts' types and names, attachments included), and
//! the text the Porch reads of it: the first 6 000 characters of its text
//! part, and its HTML (up to 64 KB) when it has no text part or only a
//! stand-in ("view it in your browser"); which part is which, mail-parser
//! says, as for the Porch (`spamcore::bodies`). A part is read whole up to
//! 256 KB and cut here (so it decodes as the Porch decodes it), only its
//! start beyond. With where it sits and its
//! flags and keywords (`$Junk`, `$NotJunk`). Records are JSON lines, gzipped
//! a batch at a time and appended to one file per account and folder, under
//! `$XDG_DATA_HOME/sioul/spam/corpus/<account>/`.
//!
//! Downloading uses Sioul's own IMAP session (the sync's TLS, login and
//! Google sign-in) and changes nothing on the server: folders are opened with
//! EXAMINE, messages read with BODY.PEEK. Trash, Drafts and Sent are left out.
//! Gmail's All Mail is read last, for what Gmail keeps only there (archived
//! mail): your own messages (from one of your addresses, or labelled \Sent)
//! and those kept already from another folder (by Message-ID) are left out
//! of it. Another server's "all mail" folder is left out: what it holds is
//! its own (the trash, junk?). It is incremental: where each folder stopped is kept
//! after every batch (`state.toml`), so it resumes there; a folder the server
//! renumbered (a new UIDVALIDITY) is read again, its older records kept. A
//! record is never deleted when the server deletes the message: that is how
//! junk outlives the provider's purge (Gmail keeps Junk a month).
//!
//! It never fills the disk: it stops before the free space falls under 1 GB,
//! and says so (`status`), for the window and the command line. A message
//! that breaks the connection (one the IMAP parser cannot read) is taken
//! alone on a new one and, failing twice, set aside (`skipped`), so that it
//! never holds back its folder.
//!
//! The corpus is private: never shared, never shown to an AI agent.

use crate::{Cancel, Dirs, LearnError, Progress, Stage, io_error, now};
use async_imap::imap_proto::{BodyStructure, ContentEncoding, SectionPath};
use async_imap::types::{Fetch, Flag};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sioul_core::config::Account;
use sioul_core::folders::{Folder, Role};
use sioul_sync::SyncError;
use sioul_sync::imap::{self, COMMAND, Imap, Server};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{BufRead, BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A text or HTML part up to this size is read whole, and cut here; a larger
/// one, only its start (a partial FETCH, which some servers write wrongly).
const WHOLE_PART: u32 = 256 * 1024;
/// The start of a larger text part read, in bytes as the server stores it:
/// enough for the Porch's 6 000 characters, in base64 or quoted-printable too.
pub const TEXT_BYTES: u32 = 16 * 1024;
/// The HTML kept, decoded, when the Porch reads it: a mail's first kilobytes
/// are often only its styles, and the Porch reads the text after them.
pub const HTML_BYTES: u32 = 64 * 1024;
/// The header block kept, at most.
pub const HEADER_BYTES: u32 = 32 * 1024;
/// What the Porch reads of the text part (card.rs `EXCERPT_LIMIT`), and under
/// how many visible characters it reads the HTML after it (card.rs `STUB`).
const EXCERPT_CHARS: usize = 6000;
const STUB_CHARS: usize = 160;
/// The download stops before the disk's free space would fall under this.
pub const KEEP_FREE: u64 = 1 << 30;
/// Messages per FETCH and per appended batch (a few in tests, to cross batches).
const BATCH: usize = if cfg!(test) { 4 } else { 50 };
/// What one batch may take on disk, at most, before compression.
const BATCH_BYTES: u64 = BATCH as u64 * (HEADER_BYTES as u64 + 4 * EXCERPT_CHARS as u64 + HTML_BYTES as u64 + 8192);
/// Parts listed per message, at most.
const MOST_PARTS: usize = 64;
const FETCH: Duration = Duration::from_secs(180);

/// One message as training reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// The account's id in Sioul's configuration.
    pub account: String,
    /// The folder's name on the server.
    pub folder: String,
    pub role: Role,
    pub uidvalidity: u32,
    pub uid: u32,
    /// INTERNALDATE, when the server received it, in Unix seconds; 0 if unsaid.
    pub date: i64,
    /// System flags and keywords as the server says them: "\\Seen", "$Junk".
    #[serde(default)]
    pub flags: Vec<String>,
    /// RFC822.SIZE: the whole message's size on the server.
    pub size: u32,
    /// The raw header block, at most `HEADER_BYTES`; read with [`Record::header_bytes`].
    pub header: String,
    /// The header was not UTF-8: each of its bytes is kept as one character (Latin-1).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub header_latin1: bool,
    /// Its MIME structure, from BODYSTRUCTURE; none when the server gave none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<Node>,
    /// Its text part (mail-parser's first text body): the first 6 000 characters, decoded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plain: Option<Text>,
    /// Its HTML part, decoded, when the Porch reads it: no text part, or a stand-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<Text>,
    /// When this record was written, in Unix seconds.
    pub fetched: i64,
}

/// A text kept, and the part it comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    /// Its part number: [1, 2] for "1.2" (RFC 9051 §6.4.5).
    pub at: Vec<u32>,
    pub text: String,
}

/// A message's MIME structure, as BODYSTRUCTURE gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Node {
    /// `multipart/<subtype>` and its parts, in order.
    Multipart { subtype: String, parts: Vec<Node> },
    /// A part that holds something: a text, a file, an attached message.
    Leaf(Leaf),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leaf {
    /// "text/plain", "application/pdf": lowercase.
    pub mime: String,
    /// Content-Type's `name`, decoded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Content-Disposition's type ("attachment", "inline") and `filename`, decoded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    /// Its transfer encoding, lowercase ("base64", "quoted-printable", "7bit").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub encoding: String,
    /// Its size on the server, encoded.
    pub octets: u32,
}

impl Node {
    /// Every leaf with its part number, in order: a message that is not
    /// multipart has one part, "1"; a multipart's are 1, 2…, theirs 1.1…
    pub fn leaves(&self) -> Vec<(Vec<u32>, &Leaf)> {
        fn walk<'a>(node: &'a Node, path: &mut Vec<u32>, out: &mut Vec<(Vec<u32>, &'a Leaf)>) {
            match node {
                Node::Multipart { parts, .. } => {
                    for (i, part) in parts.iter().enumerate() {
                        path.push(i as u32 + 1);
                        walk(part, path, out);
                        path.pop();
                    }
                }
                Node::Leaf(leaf) => out.push((if path.is_empty() { vec![1] } else { path.clone() }, leaf)),
            }
        }
        let mut out = Vec::new();
        walk(self, &mut Vec::new(), &mut out);
        out
    }

    /// The leaf at a part number.
    pub fn leaf(&self, at: &[u32]) -> Option<&Leaf> {
        self.leaves().into_iter().find(|(path, _)| path == at).map(|(_, leaf)| leaf)
    }
}

impl Record {
    /// The header block's bytes, as the server gave them.
    pub fn header_bytes(&self) -> Vec<u8> {
        if self.header_latin1 { self.header.chars().map(|c| c as u32 as u8).collect() } else { self.header.as_bytes().to_vec() }
    }

    /// Whether it carries this keyword or flag (case aside, as IMAP compares them).
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f.eq_ignore_ascii_case(flag))
    }

    /// Where it sits for good: account, folder, UIDVALIDITY, UID.
    pub fn place(&self) -> Place {
        Place { account: self.account.clone(), folder: self.folder.clone(), uidvalidity: self.uidvalidity, uid: self.uid }
    }
}

/// A message's place on its server, which names it for good (RFC 9051 §2.3.1.1).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Place {
    pub account: String,
    pub folder: String,
    pub uidvalidity: u32,
    pub uid: u32,
}

/// Whether a folder's mail enters the corpus: not the trash, drafts or what
/// you sent; Gmail's All Mail (`gmail`: the server is Gmail's, X-GM-EXT-1),
/// for the mail Gmail keeps only there; never another server's \All folder.
pub fn kept(folder: &Folder, gmail: bool) -> bool {
    match folder.role {
        Role::Trash | Role::Drafts | Role::Sent => false,
        Role::All => gmail,
        _ => true,
    }
}

/// The folders read, in the order read: the server's, All Mail last, so that
/// what the other folders hold is kept from them and left out of it.
fn reading_order(listed: &[Folder], gmail: bool) -> Vec<&Folder> {
    let mut folders: Vec<&Folder> = listed.iter().filter(|f| kept(f, gmail)).collect();
    folders.sort_by_key(|f| f.role == Role::All);
    folders
}

/// In Gmail's All Mail, what is left out: your own messages (from one of
/// `own`, or labelled \Sent when `labels`), and copies of messages kept
/// already from the account's other folders (`seen`, by Message-ID).
pub(crate) struct AllMail<'a> {
    /// Your addresses, lowercase.
    pub own: &'a [String],
    pub seen: &'a HashSet<String>,
    /// Gmail's labels asked for too (X-GM-LABELS).
    pub labels: bool,
}

impl AllMail<'_> {
    /// Whether a message of All Mail is left out, from its header block and its labels.
    fn leaves_out(&self, header: &[u8], labels: &[String]) -> bool {
        let yours = crate::labels::from_address(header).is_some_and(|from| self.own.contains(&from)) || labels.iter().any(|l| l.eq_ignore_ascii_case("\\Sent"));
        yours || crate::labels::message_id(header).is_some_and(|id| self.seen.contains(&id))
    }
}

/// The Message-IDs of what an account's folders but `except` hold in the corpus.
fn kept_ids(dir: &Path, state: &AccountState, except: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    let files: BTreeSet<(&String, u64)> = state.folders.iter().filter(|(name, _)| name.as_str() != except).map(|(_, f)| (&f.file, f.committed)).collect();
    for (file, committed) in files {
        let _ = read_file(&dir.join(file), Some(committed), |record| {
            if let Some(id) = crate::labels::message_id(&record.header_bytes()) {
                ids.insert(id);
            }
        });
    }
    ids
}

/// Your addresses, lowercase: every account's address, and its login when that is one.
fn own_addresses(accounts: &[Account]) -> Vec<String> {
    let mut own: Vec<String> =
        accounts.iter().flat_map(|a| [a.address.as_deref(), a.login()]).flatten().map(|a| a.trim().to_lowercase()).filter(|a| a.contains('@')).collect();
    own.sort();
    own.dedup();
    own
}

/// The file of a folder's records, in its account's directory: "INBOX.jsonl.gz",
/// "_Junk.jsonl.gz" (the folder's Maildir++ name, made safe for any disk).
fn file_name(folder: &Folder) -> String {
    if folder.local.is_empty() { "INBOX.jsonl.gz".to_string() } else { format!("_{}.jsonl.gz", folder.local.trim_start_matches('.')) }
}

// --- Where each folder stopped -------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct AccountState {
    #[serde(default, rename = "folder")]
    folders: BTreeMap<String, FolderState>,
}

/// One folder's download, by its server name.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FolderState {
    role: Role,
    /// Its records' file, in the account's directory.
    file: String,
    /// Bytes of that file known whole; anything after is a batch cut short.
    committed: u64,
    /// Records in the file, every UIDVALIDITY.
    #[serde(default)]
    records: u64,
    uidvalidity: u32,
    /// UIDs recorded under this UIDVALIDITY, as ranges ("1:500,502").
    #[serde(default)]
    done: String,
    /// UIDs the server could not give, as ranges: not asked again.
    #[serde(default)]
    skipped: String,
    /// Messages in the folder on the server, at the last look.
    #[serde(default)]
    total: u32,
    /// Times the server renumbered the folder.
    #[serde(default)]
    renumbered: u32,
}

impl AccountState {
    fn load(path: &Path) -> AccountState {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written beside, then moved: never half a state.
    fn save(&self, path: &Path) -> Result<(), LearnError> {
        let text = toml::to_string(self).map_err(|e| io_error(path, e))?;
        write_whole(path, text.as_bytes())
    }
}

/// Writes a file beside its place, then moves it there.
pub(crate) fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), LearnError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    }
    let temporary = path.with_extension("new");
    std::fs::write(&temporary, bytes).map_err(|e| io_error(&temporary, e))?;
    std::fs::rename(&temporary, path).map_err(|e| io_error(path, e))
}

/// "1:5,7,9:12" for a set of UIDs; read back by `read_ranges`.
pub(crate) fn ranges(uids: &BTreeSet<u32>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut iter = uids.iter().copied().peekable();
    while let Some(start) = iter.next() {
        let mut end = start;
        while iter.peek() == Some(&(end.wrapping_add(1))) && end != u32::MAX {
            end += 1;
            iter.next();
        }
        parts.push(if start == end { start.to_string() } else { format!("{start}:{end}") });
    }
    parts.join(",")
}

pub(crate) fn read_ranges(text: &str) -> BTreeSet<u32> {
    let mut uids = BTreeSet::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once(':') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                    uids.extend(a.min(b)..=a.max(b));
                }
            }
            None => {
                if let Ok(uid) = part.parse() {
                    uids.insert(uid);
                }
            }
        }
    }
    uids
}

// --- The records' files ----------------------------------------------------------

/// Appends one batch to a folder's file as a gzip member, after cutting off
/// whatever a crash left past `committed`; the file's new committed length.
fn append(path: &Path, committed: u64, records: &[Record]) -> Result<u64, LearnError> {
    let mut lines = Vec::new();
    for record in records {
        serde_json::to_writer(&mut lines, record).map_err(|e| io_error(path, e))?;
        lines.push(b'\n');
    }
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&lines).map_err(|e| io_error(path, e))?;
    let member = encoder.finish().map_err(|e| io_error(path, e))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    }
    let mut file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path).map_err(|e| io_error(path, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    if file.metadata().map_err(|e| io_error(path, e))?.len() != committed {
        file.set_len(committed).map_err(|e| io_error(path, e))?;
    }
    file.seek(std::io::SeekFrom::Start(committed)).map_err(|e| io_error(path, e))?;
    file.write_all(&member).map_err(|e| io_error(path, e))?;
    file.sync_data().map_err(|e| io_error(path, e))?;
    Ok(committed + member.len() as u64)
}

/// Reads a folder's records, up to `committed` bytes (all when None); a
/// line that cannot be read is skipped, a member cut short ends the reading.
pub fn read_file(path: &Path, committed: Option<u64>, mut each: impl FnMut(Record)) -> Result<(), LearnError> {
    let file = std::fs::File::open(path).map_err(|e| io_error(path, e))?;
    let limit = committed.unwrap_or(u64::MAX);
    let reader = BufReader::new(flate2::read::MultiGzDecoder::new(BufReader::new(file.take(limit))));
    for line in reader.split(b'\n') {
        let Ok(line) = line else { break };
        if let Ok(record) = serde_json::from_slice::<Record>(&line) {
            each(record);
        }
    }
    Ok(())
}

/// Every record of the corpus, account after account, folder after folder.
pub fn read_all(dirs: &Dirs, mut each: impl FnMut(Record)) -> Result<(), LearnError> {
    let root = dirs.corpus();
    let Ok(accounts) = std::fs::read_dir(&root) else { return Ok(()) };
    let mut accounts: Vec<PathBuf> = accounts.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    accounts.sort();
    for dir in accounts {
        let state = AccountState::load(&dir.join("state.toml"));
        let mut files: BTreeMap<String, u64> = BTreeMap::new();
        for folder in state.folders.values() {
            files.insert(folder.file.clone(), folder.committed);
        }
        for (file, committed) in files {
            let path = dir.join(&file);
            if path.exists() {
                read_file(&path, Some(committed), &mut each)?;
            }
        }
    }
    Ok(())
}

/// What the corpus's headers say of your providers' own spam filters:
/// totals only, never a message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Verdicts {
    /// Records read (a message in two folders counts twice).
    pub records: u64,
    /// Records with a filter's verdict header anywhere (X-Spam-*, X-Rspamd-*, X-Spamd-*).
    pub with_header: u64,
    /// Of those, the ones whose verdict Sioul reads now: written by the
    /// provider when the message came in (`trust::read_spam_verdict`).
    pub read: u64,
    /// Of those read, the ones saying spam.
    pub flagged: u64,
}

/// The providers' verdicts in the corpus, per account.
pub fn verdicts(dirs: &Dirs) -> Result<BTreeMap<String, Verdicts>, LearnError> {
    let mut accounts: BTreeMap<String, Verdicts> = BTreeMap::new();
    read_all(dirs, |record| {
        let headers = sioul_core::headers::RawHeaders::parse(&record.header_bytes());
        let totals = accounts.entry(record.account.clone()).or_default();
        totals.records += 1;
        let has = headers.fields().any(|(name, _)| {
            let name = name.to_ascii_lowercase();
            name.starts_with("x-spam") || name.starts_with("x-rspamd")
        });
        if has {
            totals.with_header += 1;
            if let Some(verdict) = sioul_core::trust::read_spam_verdict(&headers) {
                totals.read += 1;
                totals.flagged += u64::from(verdict.flagged);
            }
        }
    })?;
    Ok(accounts)
}

/// Where a message is kept in the corpus, by its Message-ID (bare, as the
/// label log writes it): each copy's place and its folder's role, account
/// after account. For a label said of a message no longer stored here
/// (`sioul spam label mid:…`): its place on the server is what the label
/// log names it by.
pub fn copies_of(dirs: &Dirs, message_id: &str) -> Result<Vec<(Place, Role)>, LearnError> {
    let wanted = sioul_core::mailindex::bare_id(message_id);
    let mut found = Vec::new();
    if wanted.is_empty() {
        return Ok(found);
    }
    read_all(dirs, |record| {
        // The Message-ID's text first, before the header block is parsed.
        if record.header.contains(wanted.as_str()) && crate::labels::message_id(&record.header_bytes()).is_some_and(|id| id == wanted) {
            found.push((record.place(), record.role));
        }
    })?;
    Ok(found)
}

/// Writes records as a download would: grouped per account and folder,
/// appended, their state saved (tests: this crate's and the command line's).
#[doc(hidden)]
pub fn store(dirs: &Dirs, records: &[Record]) -> Result<(), LearnError> {
    let mut groups: BTreeMap<(String, String), Vec<Record>> = BTreeMap::new();
    for record in records {
        groups.entry((record.account.clone(), record.folder.clone())).or_default().push(record.clone());
    }
    for ((account, name), records) in groups {
        let dir = dirs.corpus().join(&account);
        let state_path = dir.join("state.toml");
        let mut state = AccountState::load(&state_path);
        let role = records[0].role;
        let folder = sioul_core::folders::folder(&name, Some("/"), (role != Role::Inbox).then_some(role));
        let entry = state.folders.entry(name.clone()).or_insert_with(|| FolderState {
            role,
            file: file_name(&folder),
            committed: 0,
            records: 0,
            uidvalidity: records[0].uidvalidity,
            done: String::new(),
            skipped: String::new(),
            total: 0,
            renumbered: 0,
        });
        entry.committed = append(&dir.join(&entry.file), entry.committed, &records)?;
        entry.records += records.len() as u64;
        let mut done = read_ranges(&entry.done);
        done.extend(records.iter().map(|r| r.uid));
        entry.done = ranges(&done);
        state.save(&state_path)?;
    }
    Ok(())
}

// --- What the corpus holds, for the settings and `sioul spam status` ---------------

/// The download's state, as the window and the command line say it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// When the last download ended (Unix seconds).
    #[serde(default)]
    pub last_run: Option<i64>,
    /// It stopped to keep the disk's free space: when, and how much was free.
    #[serde(default)]
    pub held: Option<Held>,
    /// The last error of each account, by id (a technical detail, never a message).
    #[serde(default)]
    pub errors: BTreeMap<String, String>,
    /// Filled by [`status`], never written.
    #[serde(skip)]
    pub accounts: Vec<AccountStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    pub at: i64,
    /// Bytes free when it stopped.
    pub free: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountStatus {
    pub id: String,
    pub folders: Vec<FolderStatus>,
    /// Bytes its records take.
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderStatus {
    pub name: String,
    pub role: Role,
    /// Records kept, every UIDVALIDITY (junk the server purged included).
    pub records: u64,
    /// Messages of the folder on the server at the last look, and of those, recorded.
    pub on_server: u32,
    pub recorded: u64,
}

fn status_path(dirs: &Dirs) -> PathBuf {
    dirs.corpus().join("status.toml")
}

/// What the corpus holds and how its last download went.
pub fn status(dirs: &Dirs) -> Status {
    let mut status: Status = std::fs::read_to_string(status_path(dirs)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    let mut dirs_found: Vec<PathBuf> = std::fs::read_dir(dirs.corpus()).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs_found.sort();
    for dir in dirs_found {
        let state = AccountState::load(&dir.join("state.toml"));
        let folders = state
            .folders
            .iter()
            .map(|(name, f)| FolderStatus { name: name.clone(), role: f.role, records: f.records, on_server: f.total, recorded: read_ranges(&f.done).len() as u64 })
            .collect();
        let files: BTreeSet<&String> = state.folders.values().map(|f| &f.file).collect();
        let bytes = files.iter().filter_map(|f| std::fs::metadata(dir.join(f)).ok()).map(|m| m.len()).sum();
        let id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        status.accounts.push(AccountStatus { id, folders, bytes });
    }
    status
}

fn save_status(dirs: &Dirs, status: &Status) -> Result<(), LearnError> {
    let path = status_path(dirs);
    let text = toml::to_string(status).map_err(|e| io_error(&path, e))?;
    write_whole(&path, text.as_bytes())
}

// --- Downloading ---------------------------------------------------------------------

/// What one download did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Update {
    /// Records added, every account.
    pub added: u64,
    /// Accounts that could not be read, with why (a technical detail).
    pub failed: Vec<(String, String)>,
    /// It stopped to keep the disk's free space.
    pub held: Option<Held>,
    /// Stopped when asked.
    pub cancelled: bool,
}

/// The free space on the disk holding a path; None when the system does not say.
pub fn free_space(path: &Path) -> Option<u64> {
    sioul_sync::disk::space(path).map(|(free, _)| free)
}

/// Brings the corpus up to date: every account given, every folder kept
/// (`kept`), what is new since the last time. `password` gives an account's
/// password as the sync uses it (`sioul_sync::secret::password`); `room` the
/// free space on a disk (`free_space`; tests say less).
///
/// Progress: first one `Progress { stage: Corpus, detail: "", done: 0, total }`
/// with about how many messages are to come in all (each kept folder's
/// MESSAGES, less those recorded); then, folder after folder, `detail`
/// "account · folder" with that folder's `done` of `total`.
pub fn update(
    accounts: &[Account],
    password: &dyn Fn(&Account) -> Result<String, SyncError>,
    dirs: &Dirs,
    room: &dyn Fn(&Path) -> Option<u64>,
    progress: &mut dyn FnMut(&Progress),
    cancel: &Cancel,
) -> Update {
    let mut update = Update::default();
    let mut status = status(dirs);
    status.accounts.clear();
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            update.failed.push((String::new(), e.to_string()));
            return update;
        }
    };
    let own = own_addresses(accounts);
    // About how much is to come, so the window can say "N of about M": best effort.
    let passwords: Vec<(&Account, Result<String, SyncError>)> = accounts.iter().filter(|a| a.syncs()).map(|a| (a, password(a))).collect();
    let estimate: u64 = passwords.iter().filter_map(|(a, p)| p.as_ref().ok().map(|p| runtime.block_on(to_come(a, p, dirs)).unwrap_or(0))).sum();
    progress(&Progress { stage: Stage::Corpus, done: 0, total: estimate, detail: String::new() });
    for (account, password) in passwords {
        if cancel.cancelled() {
            update.cancelled = true;
            break;
        }
        let result = password.map_err(|e| e.to_string()).and_then(|password| {
            runtime.block_on(update_account(account, &password, &own, dirs, room, progress, cancel, &mut update)).map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => {
                status.errors.remove(&account.id);
            }
            Err(e) => {
                status.errors.insert(account.id.clone(), e.clone());
                update.failed.push((account.id.clone(), e));
            }
        }
        if update.held.is_some() || update.cancelled {
            break;
        }
    }
    status.held = update.held;
    status.last_run = Some(now());
    let _ = save_status(dirs, &status);
    update
}

/// About how many messages of an account's kept folders are not recorded
/// yet: each one's MESSAGES (STATUS), less what is recorded under the same UIDVALIDITY.
async fn to_come(account: &Account, password: &str, dirs: &Dirs) -> Result<u64, SyncError> {
    let state = AccountState::load(&dirs.corpus().join(&account.id).join("state.toml"));
    let mut session = imap::open(&Server::of(account)?, password).await?;
    let gmail = is_gmail(&mut session).await;
    let mut total = 0u64;
    let listed = sioul_sync::mailbox::list(&mut session).await;
    for folder in listed.iter().flatten().filter(|f| kept(f, gmail)) {
        let Ok(Ok(status)) = imap::within(COMMAND, session.status(&folder.name, "(MESSAGES UIDVALIDITY)")).await else { continue };
        let known = state.folders.get(&folder.name).filter(|f| status.uid_validity == Some(f.uidvalidity)).map_or(0, |f| read_ranges(&f.done).len() + read_ranges(&f.skipped).len());
        total += u64::from(status.exists).saturating_sub(known as u64);
    }
    let _ = session.logout().await;
    Ok(total)
}

/// Whether the server is Gmail's: it says X-GM-EXT-1 (its labels, its All Mail).
async fn is_gmail(session: &mut Imap) -> bool {
    imap::within(COMMAND, session.capabilities()).await.ok().and_then(Result::ok).is_some_and(|c| c.has_str("X-GM-EXT-1"))
}

/// Why an account's download ended early.
#[derive(Debug)]
enum Stop {
    Sync(SyncError),
    Learn(LearnError),
    /// A batch broke the connection (a message the parser cannot read, a
    /// dropped line): its messages are taken one at a time, on a new one.
    Batch { folder: Folder, validity: u32, uids: Vec<u32>, error: SyncError },
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stop::Sync(e) | Stop::Batch { error: e, .. } => write!(f, "{e}"),
            Stop::Learn(e) => write!(f, "{e}"),
        }
    }
}

/// New connections an account's download may make after a broken batch, in one run.
const MOST_RECONNECTS: usize = 20;

impl From<SyncError> for Stop {
    fn from(e: SyncError) -> Stop {
        Stop::Sync(e)
    }
}

impl From<LearnError> for Stop {
    fn from(e: LearnError) -> Stop {
        Stop::Learn(e)
    }
}

#[allow(clippy::too_many_arguments)]
async fn update_account(
    account: &Account,
    password: &str,
    own: &[String],
    dirs: &Dirs,
    room: &dyn Fn(&Path) -> Option<u64>,
    progress: &mut dyn FnMut(&Progress),
    cancel: &Cancel,
    update: &mut Update,
) -> Result<(), Stop> {
    let dir = dirs.corpus().join(&account.id);
    std::fs::create_dir_all(&dir).map_err(|e| io_error(&dir, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dirs.corpus(), std::fs::Permissions::from_mode(0o700));
    }
    // One download per account at a time, across processes (the window and the command line).
    let lock_path = dir.join("lock");
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&lock_path).map_err(|e| io_error(&lock_path, e))?;
    if lock.try_lock().is_err() {
        return Err(LearnError::Io(format!("{}: being updated by another Sioul", account.id)).into());
    }
    let server = Server::of(account)?;
    let mut reconnects = 0;
    loop {
        let mut session = imap::open(&server, password).await?;
        let gmail = is_gmail(&mut session).await;
        let result = update_folders(&mut session, account, own, gmail, &dir, room, progress, cancel, update).await;
        let _ = session.logout().await;
        match result {
            Err(Stop::Batch { folder, validity, uids, .. }) if reconnects < MOST_RECONNECTS => {
                reconnects += 1;
                reconnects += isolate(&server, password, account, own, gmail, &dir, &folder, validity, &uids, update).await?;
            }
            Err(Stop::Batch { error, .. }) => return Err(Stop::Sync(error)),
            other => return other,
        }
    }
}

/// A broken batch's messages, one at a time, each on a working connection:
/// one that breaks it alone, twice, is set aside (`skipped`), so that it never
/// holds back the rest of its folder. When none of them comes, the fault is
/// the server's or the line's, not a message's: nothing is set aside, and the
/// error is said. The connections it made.
#[allow(clippy::too_many_arguments)]
async fn isolate(server: &Server, password: &str, account: &Account, own: &[String], gmail: bool, dir: &Path, folder: &Folder, validity: u32, uids: &[u32], update: &mut Update) -> Result<usize, Stop> {
    let state_path = dir.join("state.toml");
    let seen = if folder.role == Role::All { kept_ids(dir, &AccountState::load(&state_path), &folder.name) } else { HashSet::new() };
    let mut made = 0;
    let mut session: Option<Imap> = None;
    let mut outcomes: Vec<(u32, Option<(Vec<Record>, Vec<u32>)>)> = Vec::new();
    let mut broken = None;
    for &uid in uids {
        let mut fetched = None;
        for attempt in 0..2 {
            // In All Mail, Gmail's labels are asked the first time only: the From line still tells your own.
            let all = (folder.role == Role::All).then(|| AllMail { own, seen: &seen, labels: gmail && attempt == 0 });
            if session.is_none() {
                made += 1;
                let mut opened = imap::open(server, password).await?;
                let examined = imap::within(COMMAND, opened.examine(&folder.name)).await?.map_err(imap::server)?;
                if examined.uid_validity.unwrap_or(0) != validity {
                    // Renumbered meanwhile: the next run reads the folder again.
                    let _ = opened.logout().await;
                    return Ok(made);
                }
                session = Some(opened);
            }
            match fetch_batch(session.as_mut().expect("opened above"), account, folder, validity, &[uid], all.as_ref()).await {
                Ok(records) => {
                    fetched = Some(records);
                    break;
                }
                Err(SyncError::Server(_)) => {
                    fetched = Some((Vec::new(), Vec::new()));
                    break;
                }
                // The connection went with it: a new one for the second try.
                Err(error) => {
                    broken = Some(error);
                    session = None;
                }
            }
        }
        outcomes.push((uid, fetched));
    }
    if let Some(mut session) = session {
        let _ = session.logout().await;
    }
    if let Some(error) = broken
        && !outcomes.iter().any(|(_, fetched)| fetched.as_ref().is_some_and(|(records, left)| !records.is_empty() || !left.is_empty()))
    {
        return Err(Stop::Sync(error));
    }
    let mut state = AccountState::load(&state_path);
    for (uid, fetched) in outcomes {
        match fetched {
            Some((records, left)) if !records.is_empty() || !left.is_empty() => keep(&mut state, &state_path, dir, &folder.name, &records, &left, update)?,
            _ => keep(&mut state, &state_path, dir, &folder.name, &[], &[uid], update)?,
        }
    }
    Ok(made)
}

/// One batch kept: its records appended to the folder's file, their UIDs
/// marked done; those not kept (the server could not give them, or All Mail
/// leaves them out) marked skipped; where the folder stopped saved.
fn keep(state: &mut AccountState, state_path: &Path, dir: &Path, folder: &str, records: &[Record], unreadable: &[u32], update: &mut Update) -> Result<(), LearnError> {
    let Some(entry) = state.folders.get_mut(folder) else { return Ok(()) };
    if !records.is_empty() {
        entry.committed = append(&dir.join(&entry.file), entry.committed, records)?;
        entry.records += records.len() as u64;
    }
    let mut done = read_ranges(&entry.done);
    done.extend(records.iter().map(|r| r.uid));
    entry.done = ranges(&done);
    let mut skipped = read_ranges(&entry.skipped);
    skipped.extend(unreadable);
    entry.skipped = ranges(&skipped);
    update.added += records.len() as u64;
    state.save(state_path)
}

#[allow(clippy::too_many_arguments)]
async fn update_folders(
    session: &mut Imap,
    account: &Account,
    own: &[String],
    gmail: bool,
    dir: &Path,
    room: &dyn Fn(&Path) -> Option<u64>,
    progress: &mut dyn FnMut(&Progress),
    cancel: &Cancel,
    update: &mut Update,
) -> Result<(), Stop> {
    let state_path = dir.join("state.toml");
    let mut state = AccountState::load(&state_path);
    let listed = sioul_sync::mailbox::list(session).await?;
    for folder in reading_order(&listed, gmail) {
        if cancel.cancelled() {
            update.cancelled = true;
            return Ok(());
        }
        // All Mail: what the other folders kept, read once, before it.
        let seen = if folder.role == Role::All { kept_ids(dir, &state, &folder.name) } else { HashSet::new() };
        let all = (folder.role == Role::All).then(|| AllMail { own, seen: &seen, labels: gmail });
        let examined = imap::within(COMMAND, session.examine(&folder.name)).await?.map_err(imap::server)?;
        let validity = examined.uid_validity.unwrap_or(0);
        let entry = state.folders.entry(folder.name.clone()).or_insert_with(|| FolderState {
            role: folder.role,
            file: file_name(folder),
            committed: 0,
            records: 0,
            uidvalidity: validity,
            done: String::new(),
            skipped: String::new(),
            total: 0,
            renumbered: 0,
        });
        entry.role = folder.role;
        if entry.uidvalidity != validity {
            // Renumbered: every message is new under its new UID; the older records stay.
            entry.uidvalidity = validity;
            entry.done.clear();
            entry.skipped.clear();
            entry.renumbered += 1;
        }
        entry.total = examined.exists;
        let found = imap::within(COMMAND, session.uid_search("ALL")).await?.map_err(imap::server)?;
        let done = read_ranges(&entry.done);
        let skipped = read_ranges(&entry.skipped);
        let mut wanted: Vec<u32> = found.into_iter().filter(|uid| !done.contains(uid) && !skipped.contains(uid)).collect();
        // Newest first: if the disk stops it, what is kept is the most recent.
        wanted.sort_unstable_by(|a, b| b.cmp(a));
        let detail = format!("{} · {}", account.id, folder.display);
        let total = wanted.len() as u64;
        let mut fetched = 0u64;
        progress(&Progress { stage: Stage::Corpus, done: 0, total, detail: detail.clone() });
        state.save(&state_path)?;
        for batch in wanted.chunks(BATCH) {
            if cancel.cancelled() {
                update.cancelled = true;
                return Ok(());
            }
            if let Some(free) = room(dir)
                && free < KEEP_FREE + BATCH_BYTES
            {
                update.held = Some(Held { at: now(), free });
                return Ok(());
            }
            let (records, unreadable) = match fetch_records(session, account, folder, validity, batch, all.as_ref()).await {
                Ok(fetched) => fetched,
                // Not a refusal: the connection may be gone with it; its messages one at a time.
                Err(error) if !matches!(error, SyncError::Server(_)) => return Err(Stop::Batch { folder: folder.clone(), validity, uids: batch.to_vec(), error }),
                Err(error) => return Err(error.into()),
            };
            keep(&mut state, &state_path, dir, &folder.name, &records, &unreadable, update)?;
            fetched += batch.len() as u64;
            progress(&Progress { stage: Stage::Corpus, done: fetched, total, detail: detail.clone() });
        }
    }
    Ok(())
}

/// One batch's records, and the UIDs not kept: left out (All Mail), or
/// given nothing for alone, so that they are not asked for again and again.
/// A batch the server refuses whole is asked again one message at a time.
async fn fetch_records(session: &mut Imap, account: &Account, folder: &Folder, validity: u32, uids: &[u32], all: Option<&AllMail<'_>>) -> Result<(Vec<Record>, Vec<u32>), SyncError> {
    match fetch_batch(session, account, folder, validity, uids, all).await {
        Ok(fetched) => Ok(fetched),
        Err(SyncError::Server(_)) if uids.len() > 1 => {
            let (mut records, mut unreadable) = (Vec::new(), Vec::new());
            for &uid in uids {
                match fetch_batch(session, account, folder, validity, &[uid], all).await {
                    Ok((mut one, mut left)) if !one.is_empty() || !left.is_empty() => {
                        records.append(&mut one);
                        unreadable.append(&mut left);
                    }
                    Ok(_) | Err(SyncError::Server(_)) => unreadable.push(uid),
                    Err(e) => return Err(e),
                }
            }
            Ok((records, unreadable))
        }
        Err(SyncError::Server(_)) => Ok((Vec::new(), uids.to_vec())),
        Err(e) => Err(e),
    }
}

/// What the first FETCH says of a message, before its text is asked for.
struct Head {
    uid: u32,
    flags: Vec<String>,
    date: i64,
    size: u32,
    header: Vec<u8>,
    structure: Option<Node>,
    /// Where its text and HTML bodies are, as mail-parser sorts its parts.
    plain: Option<Vec<u32>>,
    html: Option<Vec<u32>>,
    /// Gmail's labels, when asked for.
    labels: Vec<String>,
}

/// One batch: its records, and the UIDs of All Mail's messages left out (`AllMail`).
async fn fetch_batch(session: &mut Imap, account: &Account, folder: &Folder, validity: u32, uids: &[u32], all: Option<&AllMail<'_>>) -> Result<(Vec<Record>, Vec<u32>), SyncError> {
    let set = uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let labels = if all.is_some_and(|a| a.labels) { " X-GM-LABELS" } else { "" };
    let query = format!("(UID FLAGS INTERNALDATE RFC822.SIZE BODYSTRUCTURE BODY.PEEK[HEADER]<0.{HEADER_BYTES}>{labels})");
    let mut heads = Vec::new();
    {
        let mut stream = imap::within(FETCH, session.uid_fetch(&set, &query)).await?.map_err(imap::server)?;
        while let Some(fetch) = imap::within(FETCH, stream.next()).await? {
            let fetch = fetch.map_err(imap::server)?;
            if let Some(head) = head_of(&fetch) {
                heads.push(head);
            }
        }
    }
    // In All Mail, your own messages and those other folders gave already: their texts are not asked for.
    let (heads, left): (Vec<Head>, Vec<Head>) = heads.into_iter().partition(|h| all.is_none_or(|a| !a.leaves_out(&h.header, &h.labels)));
    let left: Vec<u32> = left.iter().map(|h| h.uid).collect();
    // The text bodies first, as much as the Porch reads of them.
    let leaf_at = |head: &Head, at: &[u32]| head.structure.as_ref().and_then(|s| s.leaf(at)).cloned().unwrap_or_default();
    let wanted: Vec<(u32, Vec<u32>, Leaf)> = heads.iter().filter_map(|h| h.plain.as_ref().map(|at| (h.uid, at.clone(), leaf_at(h, at)))).collect();
    let plains: BTreeMap<u32, Text> = fetch_texts(session, &wanted, TEXT_BYTES)
        .await?
        .into_iter()
        .map(|(uid, (at, bytes, leaf))| (uid, Text { at, text: decode_text(&bytes, &leaf, false).chars().take(EXCERPT_CHARS).collect() }))
        .collect();
    // The HTML where the Porch reads it: no text body, or a stand-in one (card.rs `excerpt_of`).
    let stand_in = |text: &str| sioul_core::text::visible_len(text) < STUB_CHARS;
    let wanted: Vec<(u32, Vec<u32>, Leaf)> = heads
        .iter()
        .filter(|h| plains.get(&h.uid).is_none_or(|t| stand_in(&t.text)))
        .filter_map(|h| h.html.as_ref().map(|at| (h.uid, at.clone(), leaf_at(h, at))))
        .collect();
    let htmls: BTreeMap<u32, Text> =
        fetch_texts(session, &wanted, HTML_BYTES).await?.into_iter().map(|(uid, (at, bytes, leaf))| (uid, Text { at, text: cut(decode_text(&bytes, &leaf, true), HTML_BYTES as usize) })).collect();
    let fetched = now();
    let records = heads
        .into_iter()
        .map(|head| {
            let (header, header_latin1) = header_text(&head.header);
            Record {
                account: account.id.clone(),
                folder: folder.name.clone(),
                role: folder.role,
                uidvalidity: validity,
                uid: head.uid,
                date: head.date,
                flags: head.flags,
                size: head.size,
                header,
                header_latin1,
                structure: head.structure,
                plain: plains.get(&head.uid).cloned(),
                html: htmls.get(&head.uid).cloned(),
                fetched,
            }
        })
        .collect();
    Ok((records, left))
}

/// Each part asked for: whole up to `WHOLE_PART`, else its first `limit`
/// bytes; one FETCH per part number ("1", "1.1", "2"…) and way. The bytes as
/// stored, with the leaf that says how they are encoded.
async fn fetch_texts(session: &mut Imap, wanted: &[(u32, Vec<u32>, Leaf)], limit: u32) -> Result<BTreeMap<u32, (Vec<u32>, Vec<u8>, Leaf)>, SyncError> {
    let mut groups: BTreeMap<(&Vec<u32>, bool), BTreeMap<u32, &Leaf>> = BTreeMap::new();
    for (uid, at, leaf) in wanted {
        groups.entry((at, leaf.octets <= WHOLE_PART)).or_default().insert(*uid, leaf);
    }
    let mut texts = BTreeMap::new();
    for ((at, whole), leaves) in groups {
        let set = leaves.keys().map(u32::to_string).collect::<Vec<_>>().join(",");
        let name = at.iter().map(u32::to_string).collect::<Vec<_>>().join(".");
        let query = if whole { format!("(UID BODY.PEEK[{name}])") } else { format!("(UID BODY.PEEK[{name}]<0.{limit}>)") };
        // A part the server refuses (NO, BAD) leaves its messages without that text, headers kept:
        // a refusal does not break the connection.
        let mut stream = match imap::within(FETCH, session.uid_fetch(&set, query)).await?.map_err(imap::server) {
            Ok(stream) => stream,
            Err(SyncError::Server(_)) => continue,
            Err(e) => return Err(e),
        };
        let path = SectionPath::Part(at.clone(), None);
        while let Some(fetch) = imap::within(FETCH, stream.next()).await? {
            let fetch = match fetch.map_err(imap::server) {
                Ok(fetch) => fetch,
                Err(SyncError::Server(_)) => break,
                Err(e) => return Err(e),
            };
            if let (Some(uid), Some(bytes)) = (fetch.uid, fetch.section(&path))
                && let Some(leaf) = leaves.get(&uid)
            {
                texts.insert(uid, (at.clone(), bytes.to_vec(), (*leaf).clone()));
            }
        }
    }
    Ok(texts)
}

fn head_of(fetch: &Fetch) -> Option<Head> {
    let uid = fetch.uid?;
    let flags = fetch.flags().map(|f| flag_name(&f)).collect();
    let date = fetch.internal_date().map_or(0, |d| d.timestamp());
    let mut header = fetch.header().unwrap_or_default().to_vec();
    if header.len() >= HEADER_BYTES as usize {
        // Cut at the last whole line.
        let end = header.iter().rposition(|&b| b == b'\n').map_or(header.len(), |i| i + 1);
        header.truncate(end);
    }
    let structure = fetch.bodystructure().map(|s| node_of(s, &mut 0));
    // Which part is the text, which the HTML: mail-parser says, as it does for the Porch.
    let (plain, html) = structure.as_ref().map_or((Some(vec![1]), None), crate::spamcore::bodies);
    let labels = fetch.gmail_labels().map(|l| l.iter().map(|s| s.to_string()).collect()).unwrap_or_default();
    Some(Head { uid, flags, date, size: fetch.size.unwrap_or(0), header, structure, plain, html, labels })
}

/// "\\Seen", "$Junk": a flag as the server writes it.
fn flag_name(flag: &Flag) -> String {
    match flag {
        Flag::Seen => "\\Seen".into(),
        Flag::Answered => "\\Answered".into(),
        Flag::Flagged => "\\Flagged".into(),
        Flag::Deleted => "\\Deleted".into(),
        Flag::Draft => "\\Draft".into(),
        Flag::Recent => "\\Recent".into(),
        Flag::MayCreate => "\\*".into(),
        Flag::Custom(name) => name.to_string(),
    }
}

/// A message's structure from BODYSTRUCTURE: its multiparts and their parts,
/// each leaf's type, names, charset and encoding; an attached message is one
/// leaf. At most `MOST_PARTS` leaves (`count` counts them).
fn node_of(structure: &BodyStructure, count: &mut usize) -> Node {
    let (common, single) = match structure {
        BodyStructure::Multipart { common, bodies, .. } => {
            let mut parts = Vec::new();
            for body in bodies {
                if *count >= MOST_PARTS {
                    break;
                }
                parts.push(node_of(body, count));
            }
            return Node::Multipart { subtype: common.ty.subtype.to_ascii_lowercase(), parts };
        }
        BodyStructure::Basic { common, other, .. } | BodyStructure::Text { common, other, .. } | BodyStructure::Message { common, other, .. } => (common, other),
    };
    *count += 1;
    let param = |params: &Option<Vec<(std::borrow::Cow<str>, std::borrow::Cow<str>)>>, key: &str| -> Option<String> {
        params.as_ref()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.to_string())
    };
    let disposition = common.disposition.as_ref();
    let encoding = match &single.transfer_encoding {
        ContentEncoding::Base64 => "base64".to_string(),
        ContentEncoding::QuotedPrintable => "quoted-printable".to_string(),
        ContentEncoding::SevenBit => "7bit".to_string(),
        ContentEncoding::EightBit => "8bit".to_string(),
        ContentEncoding::Binary => "binary".to_string(),
        ContentEncoding::Other(other) => other.to_ascii_lowercase(),
    };
    Node::Leaf(Leaf {
        mime: format!("{}/{}", common.ty.ty, common.ty.subtype).to_ascii_lowercase(),
        name: name_param(&common.ty.params, "name"),
        disposition: disposition.map(|d| d.ty.to_ascii_lowercase()),
        filename: disposition.and_then(|d| name_param(&d.params, "filename")),
        charset: param(&common.ty.params, "charset"),
        encoding,
        octets: single.octets,
    })
}

/// A name parameter (`filename`, `name`), decoded: RFC 2231's `key*` (a
/// charset and percent-encoding, possibly in numbered pieces), else the plain
/// one (RFC 2047's encoded words read as mail-parser reads a subject).
fn name_param(params: &Option<Vec<(std::borrow::Cow<str>, std::borrow::Cow<str>)>>, key: &str) -> Option<String> {
    let mut pieces: Vec<(u32, bool, &str)> = Vec::new();
    for (k, v) in params.as_ref()? {
        let k = k.to_ascii_lowercase();
        let Some(rest) = k.strip_prefix(key) else { continue };
        let (rest, encoded) = match rest.strip_suffix('*') {
            Some(r) => (r, true),
            None => (rest, false),
        };
        if rest.is_empty() {
            pieces.push((0, encoded, v.as_ref()));
        } else if let Some(n) = rest.strip_prefix('*').and_then(|n| n.parse::<u32>().ok()) {
            pieces.push((n, encoded, v.as_ref()));
        }
    }
    // The encoded piece before the plain one of the same number.
    pieces.sort_by_key(|(n, encoded, _)| (*n, !*encoded));
    pieces.dedup_by_key(|(n, _, _)| *n);
    let mut bytes = Vec::new();
    let mut charset = None;
    for (n, encoded, value) in pieces {
        if encoded {
            let value = if n == 0 {
                // charset'language'text
                let mut split = value.splitn(3, '\'');
                match (split.next(), split.next(), split.next()) {
                    (Some(c), Some(_), Some(text)) => {
                        charset = Some(c.to_string());
                        text
                    }
                    _ => value,
                }
            } else {
                value
            };
            bytes.extend(percent_decode(value));
        } else {
            bytes.extend_from_slice(value.as_bytes());
        }
    }
    let text = match charset.as_deref().and_then(|c| mail_parser::decoders::charsets::map::charset_decoder(c.as_bytes())) {
        Some(decode) => decode(&bytes),
        None => String::from_utf8_lossy(&bytes).into_owned(),
    };
    let text = if text.contains("=?") { decode_words(&text) } else { text };
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// RFC 2047 encoded words, read as mail-parser reads a subject.
fn decode_words(text: &str) -> String {
    let raw = format!("Subject: {text}\r\n\r\n");
    mail_parser::MessageParser::default().parse(raw.as_bytes()).and_then(|m| m.subject().map(str::to_string)).unwrap_or_else(|| text.to_string())
}

/// The start of a text part, decoded as Sioul reads mail: its transfer
/// encoding and charset undone by mail-parser, which forgives a part cut
/// short. HTML stays HTML: what is visible is read from it at training.
fn decode_text(bytes: &[u8], part: &Leaf, html: bool) -> String {
    let charset = part.charset.as_deref().unwrap_or("us-ascii").replace(['"', '\\', '\r', '\n'], "");
    let encoding = if part.encoding.is_empty() { "7bit".to_string() } else { part.encoding.replace(['\r', '\n'], "") };
    let mut raw = format!("Content-Type: text/{}; charset=\"{charset}\"\r\nContent-Transfer-Encoding: {encoding}\r\n\r\n", if html { "html" } else { "plain" }).into_bytes();
    // A base64 group cut short is dropped, so the rest decodes.
    let body = if encoding.eq_ignore_ascii_case("base64") { whole_base64(bytes) } else { bytes };
    raw.extend_from_slice(body);
    let parsed = mail_parser::MessageParser::default().parse(&raw);
    let text = parsed.as_ref().and_then(|m| if html { m.body_html(0) } else { m.body_text(0) }).map(|t| t.into_owned());
    text.unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
}

/// A text cut to at most `bytes`, on a character's edge.
fn cut(mut text: String, bytes: usize) -> String {
    if text.len() > bytes {
        let mut end = bytes;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

/// Base64 up to its last whole group of four characters.
fn whole_base64(bytes: &[u8]) -> &[u8] {
    let mut seen = 0usize;
    let mut end = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=') {
            seen += 1;
            if seen % 4 == 0 {
                end = i + 1;
            }
        }
    }
    &bytes[..end]
}

/// The header block as text: UTF-8 as it is; anything else byte for
/// character (Latin-1), which gives the bytes back.
fn header_text(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_string(), false),
        Err(_) => (bytes.iter().map(|&b| b as char).collect(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-learn-corpus-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn record(uid: u32) -> Record {
        Record {
            account: "home".into(),
            folder: "INBOX".into(),
            role: Role::Inbox,
            uidvalidity: 7,
            uid,
            date: 1_700_000_000 + i64::from(uid),
            flags: vec!["\\Seen".into(), "$Junk".into()],
            size: 1234,
            header: "Subject: Hello\r\nFrom: a@example.org\r\n".into(),
            header_latin1: false,
            structure: Some(Node::Leaf(Leaf { mime: "text/plain".into(), charset: Some("utf-8".into()), encoding: "7bit".into(), octets: 12, ..Leaf::default() })),
            plain: Some(Text { at: vec![1], text: "Hello there".into() }),
            html: None,
            fetched: 1_700_000_100,
        }
    }

    #[test]
    fn uid_ranges_go_both_ways() {
        let uids: BTreeSet<u32> = [1, 2, 3, 4, 5, 7, 9, 10, 11, 12, u32::MAX].into_iter().collect();
        assert_eq!(ranges(&uids), format!("1:5,7,9:12,{}", u32::MAX));
        assert_eq!(read_ranges(&ranges(&uids)), uids);
        assert!(read_ranges("").is_empty());
        assert_eq!(read_ranges("5:3, x, 9"), [3, 4, 5, 9].into_iter().collect());
    }

    /// Batches appended as gzip members read back whole; a batch cut short by
    /// a crash is cut off at the next append, and never read meanwhile.
    #[test]
    fn appended_batches_survive_a_crash() {
        let dir = scratch("append");
        let path = dir.join("INBOX.jsonl.gz");
        let first = append(&path, 0, &[record(1), record(2)]).unwrap();
        // A crash in the middle of the next batch: half a member past what is committed.
        let half = append(&path, first, &[record(3)]).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(first + (half - first) / 2).unwrap();
        let mut read = Vec::new();
        read_file(&path, Some(first), |r| read.push(r.uid)).unwrap();
        assert_eq!(read, vec![1, 2], "only what was committed");
        let second = append(&path, first, &[record(4)]).unwrap();
        let mut read = Vec::new();
        read_file(&path, Some(second), |r| read.push(r)).unwrap();
        assert_eq!(read.iter().map(|r| r.uid).collect::<Vec<_>>(), vec![1, 2, 4]);
        assert_eq!(read[0], record(1), "every field kept");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn headers_keep_their_bytes() {
        let latin = b"Subject: caf\xe9\r\n";
        let (text, flagged) = header_text(latin);
        assert!(flagged);
        let record = Record { header: text, header_latin1: flagged, ..record(1) };
        assert_eq!(record.header_bytes(), latin.to_vec());
        let (text, flagged) = header_text("Subject: café\r\n".as_bytes());
        assert!(!flagged && text.contains("café"));
    }

    #[test]
    fn texts_cut_short_still_decode() {
        let part = |encoding: &str, charset: Option<&str>| Leaf { encoding: encoding.into(), charset: charset.map(str::to_string), ..Leaf::default() };
        // "Bonjour à vous" in base64, cut in the middle of a group.
        assert_eq!(decode_text(b"Qm9uam91ciDDoCB2b3Vz", &part("base64", Some("utf-8")), false).trim(), "Bonjour à vous");
        assert_eq!(decode_text(b"Qm9uam91ciDDoCB2b3", &part("base64", Some("utf-8")), false).trim(), "Bonjour à v", "16 whole characters of 18");
        // Quoted-printable in Latin-1, cut after an "=".
        assert_eq!(decode_text(b"Caf=E9 cr=E8me=", &part("quoted-printable", Some("iso-8859-1")), false).trim(), "Café crème");
        // HTML stays HTML.
        assert!(decode_text(b"<p>Hi <b>there</b></p>", &part("7bit", None), true).contains("<b>there</b>"));
    }

    #[test]
    fn file_names_in_every_form() {
        use std::borrow::Cow;
        let params = |pairs: &[(&'static str, &'static str)]| Some(pairs.iter().map(|(k, v)| (Cow::Borrowed(*k), Cow::Borrowed(*v))).collect::<Vec<_>>());
        assert_eq!(name_param(&params(&[("filename", "invoice.pdf")]), "filename"), Some("invoice.pdf".into()));
        assert_eq!(name_param(&params(&[("NAME", "=?utf-8?B?ZmFjdHVyZS5wZGY=?=")]), "name"), Some("facture.pdf".into()));
        assert_eq!(name_param(&params(&[("filename*", "utf-8''%E2%82%AC%20rates.pdf")]), "filename"), Some("€ rates.pdf".into()));
        assert_eq!(name_param(&params(&[("filename*0*", "iso-8859-1''caf%E9"), ("filename*1", "-menu.zip")]), "filename"), Some("café-menu.zip".into()));
        // A file name is not a name, nor the reverse.
        assert_eq!(name_param(&params(&[("filename", "a.pdf")]), "name"), None);
        assert_eq!(name_param(&params(&[("name", "a.pdf")]), "filename"), None);
        assert_eq!(name_param(&params(&[("charset", "utf-8")]), "name"), None);
        assert_eq!(name_param(&None, "name"), None);
    }

    /// The providers' verdicts counted: a verdict written on arrival is read,
    /// one the sender wrote below where the message came in is not.
    #[test]
    fn the_providers_verdicts_counted() {
        let dir = scratch("verdicts");
        let dirs = Dirs::under(&dir);
        let header = |h: &str| Record { header: h.to_string(), ..record(1) };
        let arrived = "X-Spam-Flag: YES\r\nX-Spam-Status: Yes, score=9.1\r\nReceived: from mx.example.net (mx.example.net [192.0.2.7]) by in.example.org\r\nSubject: a\r\n\r\n";
        let senders = "Received: from mx.example.net (mx.example.net [192.0.2.7]) by in.example.org\r\nX-Spam-Flag: NO\r\nSubject: b\r\n\r\n";
        let none = "Received: from mx.example.net (mx.example.net [192.0.2.7]) by in.example.org\r\nSubject: c\r\n\r\n";
        store(&dirs, &[Record { uid: 1, ..header(arrived) }, Record { uid: 2, ..header(senders) }, Record { uid: 3, ..header(none) }]).unwrap();
        let totals = verdicts(&dirs).unwrap();
        assert_eq!(totals["home"], Verdicts { records: 3, with_header: 2, read: 1, flagged: 1 });
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn folders_left_out() {
        use sioul_core::folders::folder;
        for gmail in [false, true] {
            assert!(kept(&folder("INBOX", None, None), gmail) && kept(&folder("Junk", Some("."), Some(Role::Junk)), gmail) && kept(&folder("Archives", Some("."), None), gmail));
            for (name, role) in [("Trash", Role::Trash), ("Drafts", Role::Drafts), ("Sent", Role::Sent)] {
                assert!(!kept(&folder(name, Some("/"), Some(role)), gmail), "{name}");
            }
        }
        // All Mail: Gmail's, where archived mail is, read; another server's \All, whose contents are its own, left out.
        let all = folder("[Gmail]/All Mail", Some("/"), Some(Role::All));
        assert!(kept(&all, true) && !kept(&all, false));
        // Read last, after the folders whose copies it leaves out.
        let listed = vec![folder("INBOX", None, None), all.clone(), folder("Clients", Some("/"), None), folder("[Gmail]/Spam", Some("/"), Some(Role::Junk))];
        let order: Vec<&str> = reading_order(&listed, true).iter().map(|f| f.name.as_str()).collect();
        assert_eq!(order, ["INBOX", "Clients", "[Gmail]/Spam", "[Gmail]/All Mail"]);
        assert_eq!(reading_order(&listed, false).len(), 3);
        assert_eq!(file_name(&folder("INBOX", None, None)), "INBOX.jsonl.gz");
        assert_eq!(file_name(&folder("[Gmail]/Spam", Some("/"), Some(Role::Junk))), "_[Gmail].Spam.jsonl.gz");
    }

    /// In All Mail: your own messages (by From, or Gmail's \Sent label) and
    /// copies of messages kept from other folders are left out; the rest is ham.
    #[test]
    fn all_mail_leaves_out_your_own_and_copies() {
        let own = vec!["owner@example.org".to_string(), "me@example.com".to_string()];
        let seen: HashSet<String> = ["inbox-copy@example.net".to_string()].into_iter().collect();
        let all = AllMail { own: &own, seen: &seen, labels: true };
        let header = |from: &str, id: &str| format!("From: Someone <{from}>\r\nMessage-ID: <{id}>\r\nSubject: x\r\n").into_bytes();
        assert!(all.leaves_out(&header("Owner@Example.org", "a@example.org"), &[]), "your own address, case aside");
        assert!(all.leaves_out(&header("alias@example.net", "b@example.org"), &["\\Inbox".into(), "\\Sent".into()]), "labelled \\Sent: sent from an alias");
        assert!(all.leaves_out(&header("friend@example.net", "inbox-copy@example.net"), &[]), "kept already from the inbox");
        assert!(!all.leaves_out(&header("friend@example.net", "archived@example.net"), &["\\Important".into()]), "archived mail is kept");
        assert_eq!(own_addresses(&[Account::imap("a", " Owner@Example.org ", "imap.example.org", 993, Default::default(), Some("login@example.org"))]), vec!["login@example.org".to_string(), "owner@example.org".to_string()]);
    }

    /// Against GreenMail (docs/building.md), with invented mail only, built
    /// with the test feature: `SIOUL_TEST_INSECURE_TLS=1 cargo test -p
    /// sioul-learn --features insecure-test-tls -- --ignored greenmail`.
    #[cfg(feature = "insecure-test-tls")]
    mod greenmail {
        use super::*;
        use sioul_core::card::Card;
        use sioul_core::config::Security;
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// A new GreenMail account (it makes one at the first login).
        fn account(name: &str) -> Account {
            let address = format!("learn-{name}-{}-{}@example.org", std::process::id(), now());
            Account::imap(name, &address, "localhost", 3993, Security::Tls, None)
        }

        fn ready() -> bool {
            let ready = std::env::var_os("SIOUL_TEST_INSECURE_TLS").is_some();
            assert!(ready, "GreenMail tests need SIOUL_TEST_INSECURE_TLS=1 and the container sioul-greenmail (docs/building.md)");
            ready
        }

        fn runtime() -> tokio::runtime::Runtime {
            tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
        }

        /// Puts messages into folders, each made first: (folder, flags, raw).
        fn put(account: &Account, messages: &[(&str, &str, Vec<u8>)]) {
            runtime().block_on(async {
                let mut session = imap::open(&Server::of(account).unwrap(), "any").await.unwrap();
                let mut made = BTreeSet::new();
                for (i, (folder, flags, raw)) in messages.iter().enumerate() {
                    if *folder != "INBOX" && made.insert(folder.to_string()) {
                        let _ = session.create(folder).await;
                    }
                    let date = format!("\"{:02}-Oct-2026 10:{:02}:00 +0000\"", 1 + i / 60, i % 60);
                    let flags = (!flags.is_empty()).then(|| format!("({flags})"));
                    session.append(folder, flags.as_deref(), Some(&date), raw).await.unwrap();
                }
                let _ = session.logout().await;
            });
        }

        /// Deletes a folder's messages on the server, as a provider purging its Junk does.
        fn purge(account: &Account, folder: &str) {
            runtime().block_on(async {
                let mut session = imap::open(&Server::of(account).unwrap(), "any").await.unwrap();
                session.select(folder).await.unwrap();
                {
                    let stored = session.store("1:*", "+FLAGS.SILENT (\\Deleted)").await.unwrap();
                    let _: Vec<_> = stored.collect().await;
                }
                {
                    let gone = session.expunge().await.unwrap();
                    let _: Vec<_> = gone.collect().await;
                }
                let _ = session.logout().await;
            });
        }

        /// Deletes a folder and makes it again: a new UIDVALIDITY. Emptied
        /// first (GreenMail drops the line on deleting a folder that holds
        /// mail); renamed away when it will not go.
        fn remake(account: &Account, folder: &str) {
            purge(account, folder);
            runtime().block_on(async {
                let server = Server::of(account).unwrap();
                let mut session = imap::open(&server, "any").await.unwrap();
                if session.delete(folder).await.is_err() {
                    session = imap::open(&server, "any").await.unwrap();
                    session.rename(folder, &format!("{folder}-before")).await.unwrap();
                }
                session.create(folder).await.unwrap();
                let _ = session.logout().await;
            });
        }

        /// A message as the server keeps it, whole: what the sync stores and the Porch reads.
        fn whole(account: &Account, folder: &str, uid: u32) -> Vec<u8> {
            runtime().block_on(async {
                let mut session = imap::open(&Server::of(account).unwrap(), "any").await.unwrap();
                session.examine(folder).await.unwrap();
                let raw = {
                    let fetched: Vec<_> = session.uid_fetch(uid.to_string(), "(UID BODY.PEEK[])").await.unwrap().collect().await;
                    fetched.into_iter().filter_map(Result::ok).find_map(|f| f.body().map(<[u8]>::to_vec)).expect("the message")
                };
                let _ = session.logout().await;
                raw
            })
        }

        fn records_of(dirs: &Dirs) -> Vec<Record> {
            let mut records = Vec::new();
            read_all(dirs, |r| records.push(r)).unwrap();
            records
        }

        fn plain(n: usize, subject: &str, body: &str) -> Vec<u8> {
            format!("From: Alice Martin <alice@example.org>\r\nTo: owner@example.org\r\nSubject: {subject}\r\nDate: Wed, 07 Oct 2026 10:00:00 +0000\r\nMessage-ID: <{n}.learn@example.org>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{body}\r\n").into_bytes()
        }

        /// Messages of every shape the Porch meets, invented, on reserved domains.
        fn shapes() -> Vec<(&'static str, Vec<u8>)> {
            let long_text = "Le budget du projet avance bien, merci pour les notes. ".repeat(400);
            let base64_text = {
                // French text, UTF-8, base64 in lines of 76.
                let text = "Réunion demain à dix heures, salle des fêtes, avec l'équipe du théâtre. ".repeat(160);
                let encoded = base64(text.as_bytes());
                encoded.as_bytes().chunks(76).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join("\r\n")
            };
            let styles = format!("<style>{}</style>", ".c{color:#333;margin:0 auto;padding:4px} ".repeat(250));
            let page = format!("<html><head>{styles}</head><body><p>{}</p><a href=\"https://www.prizes.test/claim\">Claim</a></body></html>", "You have won a prize, claim it today. ".repeat(40));
            vec![
                ("plain", plain(1, "Planning de la semaine", "Bonjour, voici le planning de la semaine. Merci.")),
                ("long", plain(2, "Notes longues", &long_text)),
                ("base64", format!("From: Bruno Petit <bruno@example.net>\r\nSubject: =?utf-8?Q?R=C3=A9union?=\r\nMessage-ID: <3.learn@example.net>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=\"utf-8\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{base64_text}\r\n").into_bytes()),
                ("latin1", b"From: Chloe Durand <chloe@example.com>\r\nSubject: Caf\xe9 cr\xe8me\r\nMessage-ID: <4.learn@example.com>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=iso-8859-1\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nUn caf=E9 cr=E8me, s'il vous pla=EEt.\r\n".to_vec()),
                ("stand-in", format!("From: Prize Department <noreply@prizes.test>\r\nSubject: Congratulations\r\nMessage-ID: <5.learn@prizes.test>\r\nList-Unsubscribe: <https://prizes.test/unsubscribe>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"alt\"\r\n\r\n--alt\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nView it in your browser: https://www.prizes.test/view\r\n--alt\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{page}\r\n--alt--\r\n").into_bytes()),
                ("html-only", format!("From: Offers <offers@offers.invalid>\r\nSubject: Exclusive offer\r\nMessage-ID: <6.learn@offers.invalid>\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{page}\r\n").into_bytes()),
                ("attached", b"From: Mairie <contact@mairie.example>\r\nSubject: Votre facture\r\nMessage-ID: <7.learn@mairie.example>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"mix\"\r\n\r\n--mix\r\nContent-Type: multipart/related; boundary=\"rel\"\r\n\r\n--rel\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Votre facture est jointe.</p><img src=\"cid:logo\">\r\n--rel\r\nContent-Type: image/png; name=\"logo.png\"\r\nContent-ID: <logo>\r\nContent-Disposition: inline; filename=\"logo.png\"\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--rel--\r\n--mix\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename*=utf-8''facture%20d%C3%A9cembre.pdf\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQK\r\n--mix\r\nContent-Type: application/zip; name=\"archive.zip\"\r\nContent-Disposition: attachment\r\nContent-Transfer-Encoding: base64\r\n\r\nUEsDBA==\r\n--mix--\r\n".to_vec()),
                ("forwarded", b"From: David Leroy <david@example.org>\r\nSubject: Fwd: notes\r\nMessage-ID: <8.learn@example.org>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"fw\"\r\n\r\n--fw\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nVoici le message.\r\n--fw\r\nContent-Type: message/rfc822\r\n\r\nFrom: Emma Moreau <emma@example.com>\r\nSubject: notes\r\n\r\nLes notes de la reunion.\r\n--fw--\r\n".to_vec()),
            ]
        }

        fn base64(bytes: &[u8]) -> String {
            const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for chunk in bytes.chunks(3) {
                let n = chunk.iter().fold(0u32, |n, b| (n << 8) | u32::from(*b)) << (8 * (3 - chunk.len()));
                for i in 0..4 {
                    out.push(if i <= chunk.len() { TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char } else { '=' });
                }
            }
            out
        }

        /// Every folder kept, none of those left out; keywords and flags as the
        /// server says them; and each message read as the Porch reads it.
        #[test]
        #[ignore]
        fn greenmail_downloads_every_kept_folder_as_the_porch_reads_it() {
            if !ready() {
                return;
            }
            let root = scratch("greenmail-folders");
            let dirs = Dirs::under(&root);
            let account = account("folders");
            let shapes = shapes();
            let mut messages: Vec<(&str, &str, Vec<u8>)> = shapes.iter().map(|(_, raw)| ("INBOX", "", raw.clone())).collect();
            messages.push(("INBOX", "$Junk", plain(20, "Winner", "You are a winner, claim your prize")));
            for n in 0..5 {
                messages.push(("Junk", "", plain(30 + n, "Lottery", "Claim your lottery prize now")));
            }
            messages.push(("Junk", "$NotJunk \\Seen", plain(40, "Concert", "La repetition est jeudi")));
            for n in 0..3 {
                messages.push(("Archive", "\\Seen", plain(50 + n, "Archive", "Compte rendu")));
            }
            messages.push(("Clients", "", plain(60, "Devis", "Le devis signe")));
            for (folder, n) in [("Trash", 70), ("Sent", 71), ("Drafts", 72)] {
                messages.push((folder, "", plain(n, "Never", "Never in the corpus")));
            }
            put(&account, &messages);
            let mut seen = Vec::new();
            let downloaded = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |p| seen.push(p.clone()), &Cancel::new());
            assert!(downloaded.failed.is_empty(), "{:?}", downloaded.failed);
            let kept_messages = (shapes.len() + 1 + 6 + 3 + 1) as u64;
            assert_eq!(downloaded.added, kept_messages);
            // First, about how many are to come in all; then each folder, by account and folder only.
            assert_eq!(seen[0], Progress { stage: Stage::Corpus, done: 0, total: kept_messages, detail: String::new() });
            assert!(seen[1..].iter().all(|p| p.stage == Stage::Corpus && p.detail.starts_with("folders · ")), "account and folder only");
            let records = records_of(&dirs);
            let count = |folder: &str| records.iter().filter(|r| r.folder == folder).count();
            assert_eq!((count("INBOX"), count("Junk"), count("Archive"), count("Clients")), (shapes.len() + 1, 6, 3, 1));
            assert!(records.iter().all(|r| !matches!(r.folder.as_str(), "Trash" | "Sent" | "Drafts")));
            let by_id = |id: &str| records.iter().find(|r| r.header.contains(id)).unwrap_or_else(|| panic!("{id}"));
            assert!(by_id("<20.learn@example.org>").has_flag("$Junk"));
            let rescued = by_id("<40.learn@example.org>");
            assert!(rescued.has_flag("$NotJunk") && rescued.has_flag("\\Seen") && rescued.role == Role::Junk);
            // Each message as the Porch reads it (the server's copy, whole, as the
            // sync stores it): the same excerpt, shape, subject, attachments.
            for (i, (name, _)) in shapes.iter().enumerate() {
                let record = by_id(&format!("<{}.learn@", i + 1));
                let whole = Card::from_bytes(&whole(&account, "INBOX", record.uid)).unwrap();
                let card = crate::spamcore::card_of(record).unwrap();
                assert_eq!(card.excerpt.trim(), whole.excerpt.trim(), "{name}: excerpt");
                assert_eq!(card.shape, whole.shape, "{name}: shape");
                assert_eq!((&card.message_id, &card.attachments), (&whole.message_id, &whole.attachments), "{name}");
                // GreenMail serves BODY[HEADER] with 8-bit header bytes made "?" (its BODY[] keeps
                // them, as Dovecot and Gmail keep both): the raw Latin-1 subject cannot match here.
                if *name != "latin1" {
                    assert_eq!(card.subject, whole.subject, "{name}: subject");
                    assert_eq!(card.headers.first("Subject"), whole.headers.first("Subject"), "{name}: raw headers");
                }
            }
            // What is kept: the Porch's span of the text, the HTML only where it reads it.
            let long = by_id("<2.learn@");
            assert_eq!(long.plain.as_ref().unwrap().text.chars().count(), 6000);
            assert!(long.html.is_none());
            assert!(by_id("<3.learn@").plain.as_ref().unwrap().text.starts_with("Réunion demain à dix heures"));
            let stand_in = by_id("<5.learn@");
            assert_eq!((stand_in.plain.as_ref().map(|t| t.at.clone()), stand_in.html.as_ref().map(|t| t.at.clone())), (Some(vec![1]), Some(vec![2])));
            let html_only = by_id("<6.learn@");
            assert!(html_only.plain.is_none() && html_only.html.as_ref().is_some_and(|h| h.text.contains("claim it today")));
            let attached = by_id("<7.learn@");
            let leaves: Vec<(String, Option<String>)> = attached.structure.as_ref().unwrap().leaves().into_iter().map(|(_, l)| (l.mime.clone(), l.filename.clone().or(l.name.clone()))).collect();
            assert!(leaves.contains(&("application/pdf".into(), Some("facture décembre.pdf".into()))), "{leaves:?}");
            // Nothing new, nothing added; the provider purges its Junk, the records stay.
            let again = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |_| {}, &Cancel::new());
            assert_eq!(again.added, 0);
            purge(&account, "Junk");
            let after = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |_| {}, &Cancel::new());
            assert_eq!(after.added, 0);
            assert_eq!(records_of(&dirs).iter().filter(|r| r.folder == "Junk").count(), 6, "junk outlives the purge");
            let status = status(&dirs);
            let junk = status.accounts[0].folders.iter().find(|f| f.name == "Junk").unwrap();
            assert_eq!((junk.on_server, junk.records), (0, 6));
            let _ = std::fs::remove_dir_all(root);
        }

        /// Stopped after a batch, it goes on where it stopped; a folder
        /// renumbered is read again, its older records kept.
        #[test]
        #[ignore]
        fn greenmail_resumes_and_survives_renumbering() {
            if !ready() {
                return;
            }
            let root = scratch("greenmail-resume");
            let dirs = Dirs::under(&root);
            let account = account("resume");
            put(&account, &(0..10).map(|n| ("INBOX", "", plain(100 + n, "Message", "Un message ordinaire"))).collect::<Vec<_>>());
            let cancel = Cancel::new();
            let stop = cancel.clone();
            let first = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |p| if p.done >= BATCH as u64 { stop.cancel() }, &cancel);
            assert!(first.cancelled);
            assert_eq!(first.added, BATCH as u64);
            let kept: BTreeSet<u32> = records_of(&dirs).iter().map(|r| r.uid).collect();
            assert_eq!(kept.len(), BATCH, "the newest first");
            assert!(kept.iter().all(|&uid| uid > 10 - BATCH as u32));
            let second = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |_| {}, &Cancel::new());
            assert_eq!(second.added, 10 - BATCH as u64);
            let uids: Vec<u32> = records_of(&dirs).iter().map(|r| r.uid).collect();
            assert_eq!(uids.len(), 10);
            assert_eq!(uids.iter().copied().collect::<BTreeSet<u32>>().len(), 10, "none twice");
            // Renumbered: the folder made again holds new messages and an old one under new UIDs.
            put(&account, &(0..3).map(|n| ("Clients", "", plain(200 + n, "Devis", "Devis signe"))).collect::<Vec<_>>());
            update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |_| {}, &Cancel::new());
            let before: Vec<u32> = records_of(&dirs).iter().filter(|r| r.folder == "Clients").map(|r| r.uidvalidity).collect();
            assert_eq!(before.len(), 3);
            remake(&account, "Clients");
            put(&account, &[("Clients", "", plain(200, "Devis", "Devis signe")), ("Clients", "", plain(210, "Nouveau", "Un nouveau devis"))]);
            let renumbered = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |_| {}, &Cancel::new());
            assert_eq!(renumbered.added, 2);
            let validities: BTreeSet<u32> = records_of(&dirs).iter().filter(|r| r.folder == "Clients").map(|r| r.uidvalidity).collect();
            assert_eq!(records_of(&dirs).iter().filter(|r| r.folder == "Clients").count(), 5, "the older records kept");
            assert_eq!(validities.len(), 2, "GreenMail gave the folder made again a new UIDVALIDITY: {validities:?}");
            let _ = std::fs::remove_dir_all(root);
        }

        /// All Mail, read from a real server (GreenMail has no \\All folder:
        /// the role is given by hand): your own messages and the copies of
        /// what other folders kept are left out, their texts never asked for;
        /// archived mail is kept, as ham.
        #[test]
        #[ignore]
        fn greenmail_all_mail_keeps_only_what_no_other_folder_has() {
            if !ready() {
                return;
            }
            let root = scratch("greenmail-all");
            let dirs = Dirs::under(&root);
            let account = account("all");
            let own = own_addresses(std::slice::from_ref(&account));
            let from = |n: usize, from: &str| format!("From: {from}\r\nTo: owner@example.org\r\nSubject: Message {n}\r\nMessage-ID: <{n}.all@example.org>\r\n\r\nUn message.\r\n").into_bytes();
            // The other folders first, as the download reads them.
            put(&account, &[("INBOX", "", from(1, "Alice <alice@example.org>")), ("Clients", "", from(2, "Bruno <bruno@example.net>"))]);
            let downloaded = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |_| {}, &Cancel::new());
            assert!(downloaded.failed.is_empty() && downloaded.added == 2, "{downloaded:?}");
            put(
                &account,
                &[
                    ("All", "", from(1, "Alice <alice@example.org>")),
                    ("All", "", from(2, "Bruno <bruno@example.net>")),
                    ("All", "\\Seen", from(3, &format!("Me <{}>", account.address.clone().unwrap()))),
                    ("All", "\\Seen", from(4, "Chloe <chloe@example.com>")),
                ],
            );
            let dir = dirs.corpus().join(&account.id);
            let mut state = AccountState::load(&dir.join("state.toml"));
            let seen = kept_ids(&dir, &state, "All");
            assert!(seen.contains("1.all@example.org") && seen.contains("2.all@example.org"));
            // GreenMail's "All" read as Gmail's All Mail.
            let folder = sioul_core::folders::folder("All", Some("/"), Some(Role::All));
            let all = AllMail { own: &own, seen: &seen, labels: false };
            let (records, left) = runtime().block_on(async {
                let mut session = imap::open(&Server::of(&account).unwrap(), "any").await.unwrap();
                let validity = session.examine("All").await.unwrap().uid_validity.unwrap();
                let fetched = fetch_batch(&mut session, &account, &folder, validity, &[1, 2, 3, 4], Some(&all)).await.unwrap();
                let _ = session.logout().await;
                fetched
            });
            assert_eq!(records.iter().map(|r| r.uid).collect::<Vec<_>>(), vec![4], "archived mail only");
            assert!(records[0].role == Role::All && records[0].plain.as_ref().is_some_and(|t| t.text.contains("Un message")));
            let mut left = left;
            left.sort_unstable();
            assert_eq!(left, vec![1, 2, 3], "copies and your own, not asked for again");
            // Kept and marked as a download does; labels then call the archived message ham.
            state.folders.insert("All".into(), FolderState { role: Role::All, file: file_name(&folder), committed: 0, records: 0, uidvalidity: records[0].uidvalidity, done: String::new(), skipped: String::new(), total: 4, renumbered: 0 });
            let mut nothing = Update::default();
            keep(&mut state, &dir.join("state.toml"), &dir, "All", &records, &left, &mut nothing).unwrap();
            let entry = &AccountState::load(&dir.join("state.toml")).folders["All"];
            assert_eq!((entry.done.as_str(), entry.skipped.as_str()), ("4", "1:3"));
            let mut copies = Vec::new();
            read_all(&dirs, |r| copies.push(crate::labels::Copy::of(&r))).unwrap();
            let (labeled, _) = crate::labels::decide(copies, &[], &[]);
            assert_eq!(labeled.len(), 3, "messages 1, 2 and 4, once each");
            assert!(labeled.iter().any(|l| l.key == "4.all@example.org" && l.label == crate::labels::Label::Ham));
            let _ = std::fs::remove_dir_all(root);
        }

        /// It stops before the disk's free space would fall under 1 GB, says
        /// so, and goes on once there is room.
        #[test]
        #[ignore]
        fn greenmail_stops_before_the_disk_fills() {
            if !ready() {
                return;
            }
            let root = scratch("greenmail-disk");
            let dirs = Dirs::under(&root);
            let account = account("disk");
            put(&account, &(0..10).map(|n| ("INBOX", "", plain(300 + n, "Message", "Un message ordinaire"))).collect::<Vec<_>>());
            // Room for one batch, then the disk is said to be full.
            let looks = AtomicUsize::new(0);
            let room = |_: &Path| Some(if looks.fetch_add(1, Ordering::Relaxed) == 0 { KEEP_FREE + BATCH_BYTES } else { KEEP_FREE + BATCH_BYTES - 1 });
            let held = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &room, &mut |_| {}, &Cancel::new());
            assert_eq!(held.added, BATCH as u64);
            assert!(held.held.is_some_and(|h| h.free == KEEP_FREE + BATCH_BYTES - 1), "{held:?}");
            assert_eq!(status(&dirs).held, held.held, "said where the window and the command line read it");
            let full = update(std::slice::from_ref(&account), &|_| Ok("any".into()), &dirs, &|_| Some(u64::MAX), &mut |_| {}, &Cancel::new());
            assert_eq!(full.added, 10 - BATCH as u64);
            assert_eq!(status(&dirs).held, None);
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

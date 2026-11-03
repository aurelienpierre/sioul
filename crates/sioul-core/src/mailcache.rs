// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each message of a folder says, read once, ever: one small index per
//! folder, kept as a cache (the unified mailbox's design, §5.1).
//!
//! - **Where:** `$XDG_CACHE_HOME/sioul/mail-index/<account>/<folder>.tsv`,
//!   `<folder>` the folder's Maildir++ directory (".Sent"), "INBOX" for the
//!   Maildir itself. Readable by you alone (0600, its folder 0700), never
//!   shared nor synced: each device keeps its own, as it keeps its own
//!   Maildir. Removed with the address (`maildir::forget_account`).
//! - **Its first line** names its format and a stamp of the one column that
//!   depends on your settings (a one-time code in a list's message: the words that
//!   find codes). Another first line: the file is made again from the
//!   messages. Nothing is ever migrated, nothing is lost: it is a cache.
//! - **One line per message**, keyed by the part of its file name that never
//!   changes (`maildir::unique_part`: when it was stored, its UIDVALIDITY and
//!   UID): what its headers say (`Line`). The folder stays the truth: its
//!   file names say what is there, read or not, answered, flagged, junk or
//!   not; the index gives only what the headers say.
//! - **Each message is parsed once:** a name the index does not know is read
//!   whole once and its line added at the end, under the file's own lock
//!   (`filelock::with_lock`), so that the window, the phone's background step
//!   and `sioul mcp` never add the same line twice.
//! - **Lines whose files are gone** stay until they make half the file; it is
//!   then written again beside and renamed over it, under the lock.
//! - **A stop half-way** leaves, during an addition, at most a cut last line,
//!   which does not read and is skipped (its message is read again and its
//!   line added again); during a compaction, the old file whole until the
//!   rename (a file left beside is written over by the next one); during a
//!   rebuild, a file of the older format, made again at the next read.
//!
//! The first time this device makes the folder `mail-index/`, it removes the
//! older single index (`mail-index.tsv`), which no part of this version
//! reads or writes. An older Sioul still running writes that file again for
//! itself, and nothing new depends on it.

use crate::card::Card;
use crate::headers::RawHeaders;
use crate::maildir;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

/// The format, as the first line names it, before its stamp.
pub const FORMAT: &str = "sioul-mail-index\t1";

/// What separates the entries of a list column (recipients, attachments, headers).
const SEP: char = '\u{1f}';

/// What a folder's line keeps of one message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    /// The part of its file name that never changes.
    pub unique: String,
    /// When it arrived: its file's time, the server's INTERNALDATE (Unix seconds).
    pub arrived: i64,
    /// Its Date header (Unix seconds).
    pub date: Option<i64>,
    /// Its Message-ID, as the card reads it.
    pub message_id: Option<String>,
    pub from_name: Option<String>,
    /// The sender's address, lowercase.
    pub from_address: Option<String>,
    /// To whom it went, as the card says it (names, else addresses), a hundred at most.
    pub to: Vec<String>,
    /// Its subject, 300 characters at most.
    pub subject: String,
    /// A newsletter or a mailing list.
    pub is_list: bool,
    /// Its attachments' names.
    pub attachments: Vec<String>,
    /// Your provider flagged it as spam (`trust::read_spam_verdict`).
    pub spam: bool,
    /// The UID of the event an invitation it carries names.
    pub invitation: Option<String>,
    /// Its Authentication-Results headers, as written (which ones are
    /// trusted is decided when read: `trust::read_auth_results`).
    pub auth: Vec<String>,
    /// Its In-Reply-To and References headers, as written.
    pub in_reply_to: Vec<String>,
    pub references: Vec<String>,
    /// A list's message carrying a one-time code, by the words that find
    /// codes (the stamp): what a folder's list weighs against a failed DMARC
    /// (`view::mail_item`, as the Porch). Only list mail is looked at: the
    /// search costs far more than the parse, and nothing reads it of other mail.
    pub code: bool,
    /// For a message without a Message-ID: its digest (`digest`).
    pub digest: Option<String>,
}

fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r', SEP], " ")
}

fn joined(list: &[String]) -> String {
    list.iter().map(|e| clean(e)).collect::<Vec<_>>().join(&SEP.to_string())
}

fn split(text: &str) -> Vec<String> {
    if text.is_empty() { Vec::new() } else { text.split(SEP).map(str::to_string).collect() }
}

impl Line {
    /// The line of a message read from its bytes: one parse. None when it does not parse at all.
    pub fn of_raw(unique: &str, arrived: i64, raw: &[u8], words: &crate::words::Words) -> Option<Line> {
        let message = mail_parser::MessageParser::default().parse(raw)?;
        maildir::parsed_once();
        let card = Card::from_parsed(raw, &message);
        let invitation = crate::reading::calendar_in(&message).and_then(|text| event_uid(&text));
        let headers = &card.headers;
        let digest = card.message_id.is_none().then(|| digest(raw));
        // As the file keeps them: a tab, a line end or the list separator in a value made a space.
        let text = |t: &Option<String>| t.as_deref().map(clean);
        let list = |l: Vec<&str>| l.into_iter().map(clean).collect::<Vec<_>>();
        Some(Line {
            unique: clean(unique),
            arrived,
            date: card.date,
            message_id: text(&card.message_id),
            from_name: text(&card.from_name),
            from_address: text(&card.from_address),
            to: list(card.to.iter().take(100).map(String::as_str).collect()),
            subject: clean(&card.subject.chars().take(300).collect::<String>()),
            is_list: card.is_list,
            attachments: list(card.attachments.iter().map(String::as_str).collect()),
            spam: crate::trust::read_spam_verdict(headers).is_some_and(|v| v.flagged),
            invitation: invitation.as_deref().map(clean),
            auth: list(headers.all("Authentication-Results").collect()),
            in_reply_to: list(headers.all("In-Reply-To").collect()),
            references: list(headers.all("References").collect()),
            code: card.is_list && crate::codes::detect_message(&words.codes, &card).is_some(),
            digest,
        })
    }

    fn to_tsv(&self) -> String {
        let flag = |b: bool| if b { "1" } else { "0" };
        let opt = |o: &Option<String>| o.as_deref().map(clean).unwrap_or_default();
        [
            clean(&self.unique),
            self.arrived.to_string(),
            self.date.map(|d| d.to_string()).unwrap_or_default(),
            opt(&self.message_id),
            opt(&self.from_name),
            opt(&self.from_address),
            joined(&self.to),
            clean(&self.subject),
            flag(self.is_list).to_string(),
            joined(&self.attachments),
            flag(self.spam).to_string(),
            opt(&self.invitation),
            joined(&self.auth),
            joined(&self.in_reply_to),
            joined(&self.references),
            flag(self.code).to_string(),
            opt(&self.digest),
        ]
        .join("\t")
    }

    fn from_tsv(line: &str) -> Option<Line> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 17 || f[0].is_empty() {
            return None;
        }
        let opt = |t: &str| (!t.is_empty()).then(|| t.to_string());
        let flag = |t: &str| match t {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        };
        Some(Line {
            unique: f[0].to_string(),
            arrived: f[1].parse().ok()?,
            date: if f[2].is_empty() { None } else { Some(f[2].parse().ok()?) },
            message_id: opt(f[3]),
            from_name: opt(f[4]),
            from_address: opt(f[5]),
            to: split(f[6]),
            subject: f[7].to_string(),
            is_list: flag(f[8])?,
            attachments: split(f[9]),
            spam: flag(f[10])?,
            invitation: opt(f[11]),
            auth: split(f[12]),
            in_reply_to: split(f[13]),
            references: split(f[14]),
            code: flag(f[15])?,
            digest: opt(f[16]),
        })
    }

    /// The card a list reads (`view::folder`): what the headers said, the
    /// file `path` it is now, its numbering from its name.
    pub fn card(&self, path: &Path) -> Card {
        let mut raw = String::new();
        for (name, values) in [("Authentication-Results", &self.auth), ("In-Reply-To", &self.in_reply_to), ("References", &self.references)] {
            for value in values {
                raw.push_str(&format!("{name}: {value}\r\n"));
            }
        }
        raw.push_str("\r\n");
        Card {
            path: Some(path.to_path_buf()),
            account: None,
            origin: maildir::origin_of(path),
            from_name: self.from_name.clone(),
            from_address: self.from_address.clone(),
            to: self.to.clone(),
            subject: self.subject.clone(),
            date: self.date,
            message_id: self.message_id.clone(),
            excerpt: String::new(),
            attachments: self.attachments.clone(),
            is_list: self.is_list,
            headers: RawHeaders::parse(raw.as_bytes()),
            shape: Default::default(),
            code: Some(self.code),
        }
    }

    /// The ids it answers or cites (In-Reply-To, References), bare, each once.
    pub fn refs(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for value in self.in_reply_to.iter().chain(&self.references) {
            for id in value.split(|c: char| c.is_whitespace() || c == ',').filter(|t| t.contains('@')).map(crate::mailindex::bare_id) {
                if !id.is_empty() && !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }
}

/// The UID of the first event of an invitation's calendar text.
fn event_uid(text: &str) -> Option<String> {
    let lines = crate::lines::unfold(text);
    let start = lines.iter().position(|l| l.trim().eq_ignore_ascii_case("BEGIN:VEVENT"))?;
    lines[start..].iter().take_while(|l| !l.trim().eq_ignore_ascii_case("END:VEVENT")).find(|l| crate::lines::name(l) == "UID").map(|l| crate::lines::value(l).trim().to_string()).filter(|u| !u.is_empty())
}

/// A message's headers: what comes before its first empty line, at most 256 KB.
pub fn headers_of(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(8192);
    std::fs::File::open(path).ok()?.take(256 * 1024).read_to_end(&mut bytes).ok()?;
    let end = bytes.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| bytes.windows(2).position(|w| w == b"\n\n"))?;
    bytes.truncate(end);
    Some(bytes)
}

/// The header fields as the bytes hold them, each with its folded lines,
/// line endings as "\n": (lowercase name, the whole field).
pub fn header_fields(raw: &[u8]) -> Vec<(String, String)> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| raw.windows(2).position(|w| w == b"\n\n")).unwrap_or(raw.len());
    let text = String::from_utf8_lossy(&raw[..end]).replace("\r\n", "\n");
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.split('\n') {
        if line.starts_with([' ', '\t']) {
            if let Some((_, field)) = out.last_mut() {
                field.push('\n');
                field.push_str(line);
            }
        } else if let Some((name, _)) = line.split_once(':') {
            out.push((name.trim().to_ascii_lowercase(), line.to_string()));
        }
    }
    out
}

/// The digest that names a message without a Message-ID (the mailbox's §3.1):
/// 16 hexadecimal digits of SHA-256 over its From, To, Date and Subject
/// fields as its bytes hold them. The server gives every device the same
/// bytes, so every device computes the same. One function for the whole
/// program: an address (`sioul:mail/<account>/<digest>`), the index, and a
/// renumbered folder's pairing (sioul-sync's `renumbered::key_of`).
pub fn digest(raw: &[u8]) -> String {
    let fields = header_fields(raw);
    let mut hasher = Sha256::new();
    for wanted in ["from", "to", "date", "subject"] {
        if let Some((_, field)) = fields.iter().find(|(name, _)| name == wanted) {
            hasher.update(field.as_bytes());
        }
        hasher.update(b"\n");
    }
    hasher.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// The stamp of the settings the `code` column depends on: the words that find codes.
pub fn stamp(words: &crate::words::Words) -> String {
    let text = serde_json::to_string(&words.codes).unwrap_or_default();
    Sha256::digest(text.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// One plain name: no separator, nothing that climbs (`..`), not empty.
fn plain(name: &str) -> bool {
    matches!(Path::new(name).components().collect::<Vec<_>>().as_slice(), [Component::Normal(_)])
}

/// The folder of the indexes, made the first time with the older single
/// index removed (`mail-index.tsv`: once per device, never again).
fn index_root() -> PathBuf {
    index_root_in(&crate::config::cache_dir())
}

fn index_root_in(cache: &Path) -> PathBuf {
    let root = cache.join("mail-index");
    let older = cache.join("mail-index.tsv");
    let made = !root.is_dir() && std::fs::create_dir_all(&root).is_ok();
    if made {
        private_dir(&root);
    }
    // Gone when the folder is first made; written again by an older Sioul
    // still running, gone once none has written it for two weeks (review of
    // 05ccd0a, L13): it holds every message's path, subject and sender.
    let stale = || older.metadata().and_then(|m| m.modified()).ok().and_then(|at| at.elapsed().ok()).is_some_and(|age| age > OLDER_KEPT);
    if made || stale() {
        let _ = std::fs::remove_file(&older);
    }
    root
}

/// How long the older single index stays unwritten before it goes.
const OLDER_KEPT: std::time::Duration = std::time::Duration::from_secs(14 * 86_400);

fn private_dir(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Where the index of `account`'s folder `local` is (`""` for the inbox,
/// ".Sent"); a folder held within another (another program's layout,
/// "Archive/2024") under `nested/`, by a hash of its path. None for an
/// account whose id is not one plain name, or a folder that climbs (`..`).
pub fn index_path(account: &str, local: &str) -> Option<PathBuf> {
    if !plain(account) {
        return None;
    }
    let dir = index_root().join(account);
    if local.is_empty() {
        return Some(dir.join("INBOX.tsv"));
    }
    if plain(local) {
        return Some(dir.join(format!("{local}.tsv")));
    }
    let levels: Vec<Component> = Path::new(local).components().collect();
    if levels.len() > 8 || !levels.iter().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let hash: String = Sha256::digest(local.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect();
    Some(dir.join("nested").join(format!("{hash}.tsv")))
}

/// The index folder of an account, for `maildir::forget_account`.
pub fn account_dir(account: &str) -> Option<PathBuf> {
    plain(account).then(|| crate::config::cache_dir().join("mail-index").join(account))
}

/// A message file of a folder: its path, the unique part of its name, when it arrived.
struct File {
    path: PathBuf,
    unique: String,
    arrived: i64,
}

/// The message files of a folder (`new/`, `cur/`), hidden ones aside.
fn files(dir: &Path) -> Vec<File> {
    let mut out = Vec::new();
    for sub in ["new", "cur"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else { continue };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let arrived = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
            out.push(File { unique: maildir::unique_part(&name).to_string(), path: entry.path(), arrived });
        }
    }
    out
}

/// The index's lines by unique part, and how many lines it holds; none when
/// its first line is not this format with this stamp (made again then).
fn read_index(path: &Path, header: &str) -> Option<(HashMap<String, Line>, usize)> {
    let bytes = std::fs::read(path).ok()?;
    // A stop may cut the last line (and a character in it): every line written ends with a line end.
    let whole = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |end| end + 1);
    let cut = whole < bytes.len();
    let text = String::from_utf8_lossy(&bytes[..whole]);
    let mut lines = text.split_terminator('\n');
    if lines.next()? != header {
        return None;
    }
    let mut known = HashMap::new();
    let mut count = usize::from(cut);
    for line in lines {
        count += 1;
        if let Some(line) = Line::from_tsv(line) {
            known.insert(line.unique.clone(), line);
        }
    }
    Some((known, count))
}

/// `path` opened with `options`, made readable by you alone (0600) when it is made.
fn private_file(path: &Path, options: &mut std::fs::OpenOptions) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// `header` and `lines` written beside `path`, then put in its place.
fn write_whole(path: &Path, header: &str, lines: &[&Line]) -> std::io::Result<()> {
    let beside = path.with_extension("tsv.new");
    let mut text = String::with_capacity(lines.len() * 200 + header.len() + 1);
    text.push_str(header);
    text.push('\n');
    for line in lines {
        text.push_str(&line.to_tsv());
        text.push('\n');
    }
    let mut file = private_file(&beside, std::fs::OpenOptions::new().write(true).create(true).truncate(true))?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&beside, path)
}

/// Lines added at the end of `path`, after a line end if a stop cut the last one.
fn append(path: &Path, lines: &[Line]) -> std::io::Result<()> {
    let mut file = private_file(path, std::fs::OpenOptions::new().read(true).append(true).create(true))?;
    let mut text = String::new();
    let length = file.metadata()?.len();
    if length > 0 {
        let mut last = [0u8; 1];
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(length - 1))?;
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            text.push('\n');
        }
    }
    for line in lines {
        text.push_str(&line.to_tsv());
        text.push('\n');
    }
    file.write_all(text.as_bytes())
}

/// The messages of one folder of `account` (its Maildir `root`, the folder's
/// directory `local`, "" for the inbox), each file with its line: lines kept
/// read from the folder's index, the others read once and added to it.
/// Without an index (an account whose id is not one plain name), every message
/// is read.
pub fn folder(account: &str, root: &Path, local: &str) -> Vec<(PathBuf, Line)> {
    look(account, root, local, usize::MAX).lines
}

/// When a message was sent, never later than it arrived (`porch::sent`): the
/// mailbox's order and window.
pub fn sent_at(line: &Line) -> i64 {
    match line.date {
        Some(date) if line.arrived > 0 => date.min(line.arrived),
        Some(date) => date,
        None => line.arrived,
    }
}

/// What a look at a folder found.
struct Looked {
    lines: Vec<(PathBuf, Line)>,
    /// The files the index does not know yet and this look left, newest first.
    waiting: Vec<File>,
    /// The index as this look left it, under its lock.
    index: Option<Stamp>,
    /// Messages read whole.
    read: usize,
}

/// `folder`, reading at most `fill` messages the index does not know yet,
/// the newest first (by when they arrived); the others wait.
fn look(account: &str, root: &Path, local: &str, fill: usize) -> Looked {
    let words = crate::words::current();
    let dir = root.join(local);
    let mut files = files(&dir);
    let Some(index) = index_path(account, local) else {
        // No index: every message read, as it must be.
        let lines: Vec<(PathBuf, Line)> = files.into_iter().filter_map(|f| read_line(&f, &words).map(|line| (f.path, line))).collect();
        let read = lines.len();
        return Looked { lines, waiting: Vec::new(), index: None, read };
    };
    // Its folders (the account's, `nested/`), yours alone as the messages are.
    let base = index_root();
    for dir in index.ancestors().skip(1).take_while(|dir| *dir != base && dir.starts_with(&base)).collect::<Vec<_>>().into_iter().rev() {
        let _ = std::fs::create_dir(dir);
        private_dir(dir);
    }
    let header = format!("{FORMAT}\t{}", stamp(&words));
    // The newest first: a bounded look reads those, which the first page needs.
    files.sort_by_key(|f| std::cmp::Reverse(f.arrived));
    crate::filelock::with_lock(&index, || {
        let (known, count, fresh_file) = match read_index(&index, &header) {
            Some((known, count)) => (known, count, false),
            None => (HashMap::new(), 0, true),
        };
        let mut out = Vec::with_capacity(files.len());
        let mut added: Vec<Line> = Vec::new();
        let mut waiting = Vec::new();
        for file in files {
            match known.get(&file.unique) {
                Some(line) => out.push((file.path, line.clone())),
                None if added.len() >= fill => waiting.push(file),
                None => {
                    if let Some(line) = read_line(&file, &words) {
                        added.push(line.clone());
                        out.push((file.path, line));
                    }
                }
            }
        }
        let live: std::collections::HashSet<&str> = out.iter().map(|(_, line)| line.unique.as_str()).collect();
        let dead = known.keys().filter(|unique| !live.contains(unique.as_str())).count() + count.saturating_sub(known.len());
        let lines_after = count + added.len();
        if fresh_file || (dead > 0 && dead * 2 >= lines_after) {
            // Made again, or more dead than alive: written whole beside, then renamed.
            let all: Vec<&Line> = out.iter().map(|(_, line)| line).collect();
            let _ = write_whole(&index, &header, &all);
        } else if !added.is_empty() {
            let _ = append(&index, &added);
        }
        // Oldest first, as before: callers sort as they need.
        out.reverse();
        Looked { lines: out, waiting, index: Stamp::of_file(&index, &header), read: added.len() }
    })
}

/// How far a kept look may be trusted: a directory changed this close to
/// its listing may change again within the same tick of its clock without
/// its time moving (file times move by the kernel's coarse tick, two
/// seconds on FAT): it is listed again next time (git's "racy" index).
const RACY: std::time::Duration = std::time::Duration::from_secs(2);

/// What says a folder and its index are as a look left them: `cur/`'s and
/// `new/`'s times (any file added, renamed by its flags or removed changes
/// its directory's), the index's length, time and inode (another process
/// adding to it, compacting it or making it again), and the stamp of the
/// settings its lines depend on.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Stamp {
    cur: Option<std::time::SystemTime>,
    new: Option<std::time::SystemTime>,
    index: Option<(u64, std::time::SystemTime, u64)>,
    header: String,
}

impl Stamp {
    fn of_dirs(dir: &Path) -> (Option<std::time::SystemTime>, Option<std::time::SystemTime>) {
        let at = |sub: &str| std::fs::metadata(dir.join(sub)).and_then(|m| m.modified()).ok();
        (at("cur"), at("new"))
    }

    fn of_file(index: &Path, header: &str) -> Option<Stamp> {
        let meta = std::fs::metadata(index).ok()?;
        #[cfg(unix)]
        let inode = std::os::unix::fs::MetadataExt::ino(&meta);
        #[cfg(not(unix))]
        let inode = 0;
        Some(Stamp { cur: None, new: None, index: Some((meta.len(), meta.modified().ok()?, inode)), header: header.to_string() })
    }

    /// The folder and its index now.
    fn now(dir: &Path, index: Option<&Path>, header: &str) -> Stamp {
        let (cur, new) = Stamp::of_dirs(dir);
        let index = index.and_then(|i| Stamp::of_file(i, header)).and_then(|s| s.index);
        Stamp { cur, new, index, header: header.to_string() }
    }
}

/// A folder's recent lines as a look left them, kept in this process.
struct Kept {
    stamp: Stamp,
    /// Whether its directories were settled when listed (`RACY`).
    settled: bool,
    /// Lines from `since` on (by `sent_at`), and whether older ones were left out.
    since: i64,
    lines: std::sync::Arc<Vec<(PathBuf, Line)>>,
    older: bool,
    waiting: usize,
}

/// The folders' recent lines, by account, Maildir and folder.
static KEPT: std::sync::Mutex<Option<HashMap<(String, PathBuf, String), Kept>>> = std::sync::Mutex::new(None);

/// A folder's lines from `since` on, as the mailbox reads them (the unified
/// mailbox's part 5, §6): kept in this process, and taken from there without
/// listing the folder or reading its index again while `Stamp` says nothing
/// changed and its directories were settled when listed. At most `fill`
/// messages the index does not know yet are read, the newest first;
/// `waiting` says how many wait. `older`: lines older than `since` exist,
/// or files that arrived before it wait. `since` at `i64::MIN` (a search):
/// every line, never kept.
pub struct Recent {
    /// Its lines, some perhaps older than `since` (a wider look kept): the caller filters.
    pub lines: std::sync::Arc<Vec<(PathBuf, Line)>>,
    pub older: bool,
    pub waiting: usize,
    /// Messages read whole by this look.
    pub read: usize,
}

/// Whether this process keeps a look at this folder already (`recent`):
/// a folder not looked at yet gets its first fill whole.
pub fn is_kept(account: &str, root: &Path, local: &str) -> bool {
    KEPT.lock().is_ok_and(|k| k.as_ref().is_some_and(|k| k.contains_key(&(account.to_string(), root.to_path_buf(), local.to_string()))))
}

pub fn recent(account: &str, root: &Path, local: &str, since: i64, fill: usize) -> Recent {
    let words = crate::words::current();
    let header = format!("{FORMAT}\t{}", stamp(&words));
    let dir = root.join(local);
    let key = (account.to_string(), root.to_path_buf(), local.to_string());
    let index = index_path(account, local);
    let before = Stamp::now(&dir, index.as_deref(), &header);
    if since != i64::MIN
        && let Ok(kept) = KEPT.lock()
        && let Some(found) = kept.as_ref().and_then(|k| k.get(&key))
        && found.settled
        && found.stamp == before
        && found.since <= since
        && (found.waiting == 0 || fill == 0)
    {
        return Recent { lines: std::sync::Arc::clone(&found.lines), older: found.older, waiting: found.waiting, read: 0 };
    }
    let listed = std::time::SystemTime::now();
    let looked = look(account, root, local, fill);
    let read = looked.read;
    let settled = [before.cur, before.new].iter().all(|t| t.is_none_or(|t| t + RACY <= listed));
    let older = looked.waiting.iter().any(|f| f.arrived < since) || looked.lines.iter().any(|(_, l)| sent_at(l) < since);
    let lines: Vec<(PathBuf, Line)> = looked.lines.into_iter().filter(|(_, l)| since == i64::MIN || sent_at(l) >= since).collect();
    let lines = std::sync::Arc::new(lines);
    let waiting = looked.waiting.len();
    if since != i64::MIN
        && let Ok(mut kept) = KEPT.lock()
    {
        // The directories' times from before the listing, the index's from after this look's own write.
        let stamp = Stamp { cur: before.cur, new: before.new, index: looked.index.and_then(|s| s.index), header };
        kept.get_or_insert_with(HashMap::new).insert(key, Kept { stamp, settled, since, lines: std::sync::Arc::clone(&lines), older, waiting });
    }
    Recent { lines, older, waiting, read }
}

/// One message file's line, read whole once; none when it does not read.
fn read_line(file: &File, words: &crate::words::Words) -> Option<Line> {
    let raw = std::fs::read(&file.path).ok()?;
    Line::of_raw(&file.unique, file.arrived, &raw, words)
}

/// The folders of a Maildir with messages, by their directory: "" for the
/// Maildir itself, then its Maildir++ subfolders (".Sent"), and folders held
/// within others, as other programs lay them out ("Archive/2024"), eight
/// levels at most. A folder that is a link is not followed.
pub fn folders(root: &Path) -> Vec<String> {
    let has_mail = |dir: &Path| dir.join("cur").is_dir() || dir.join("new").is_dir();
    let mut out = Vec::new();
    if has_mail(root) {
        out.push(String::new());
    }
    let mut found = Vec::new();
    let mut stack: Vec<(PathBuf, String, usize)> = vec![(root.to_path_buf(), String::new(), 0)];
    while let Some((dir, local, depth)) = stack.pop() {
        if depth == 8 {
            continue;
        }
        for entry in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
            // The entry itself: a link to a folder is not a folder here.
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(name.as_str(), "cur" | "new" | "tmp") || !plain(&name) {
                continue;
            }
            let path = entry.path();
            let local = if local.is_empty() { name } else { format!("{local}/{name}") };
            if has_mail(&path) {
                found.push(local.clone());
            }
            stack.push((path, local, depth + 1));
        }
    }
    found.sort();
    out.extend(found);
    out
}

/// The line `account`'s folder `local` keeps for the file whose name's unique
/// part is `unique`, whether the file is there or gone (its line stays until
/// the index is compacted): what a hint to a file gone says it was.
pub fn line_of(account: &str, local: &str, unique: &str) -> Option<Line> {
    let index = index_path(account, local)?;
    let header = format!("{FORMAT}\t{}", stamp(&crate::words::current()));
    // Read without the lock: a line being added shows cut at most, and is not taken.
    read_index(&index, &header).and_then(|(mut known, _)| known.remove(unique))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in folder of a stand-in account, under the test's own cache.
    struct Stand {
        root: PathBuf,
        account: String,
    }

    impl Stand {
        fn new(name: &str) -> Stand {
            let account = format!("stand-{name}-{}", std::process::id());
            let root = std::env::temp_dir().join(format!("sioul-mailcache-{account}"));
            let _ = std::fs::remove_dir_all(&root);
            for sub in ["tmp", "new", "cur"] {
                std::fs::create_dir_all(root.join(sub)).unwrap();
            }
            if let Some(dir) = account_dir(&account) {
                let _ = std::fs::remove_dir_all(dir);
            }
            Stand { root, account }
        }

        fn put(&self, uid: u32, id: &str, subject: &str) -> PathBuf {
            let path = self.root.join("cur").join(format!("1777000000.U1-{uid}.sioul{}2,S", maildir::INFO));
            std::fs::write(&path, format!("Message-ID: <{id}>\r\nFrom: Ana <ana@example.org>\r\nTo: Bo <bo@example.org>\r\nDate: Fri, 30 Oct 2026 09:00:00 +0100\r\nSubject: {subject}\r\n\r\nThe text.\r\n")).unwrap();
            path
        }

        fn index(&self) -> PathBuf {
            index_path(&self.account, "").unwrap()
        }

        fn read(&self) -> Vec<(PathBuf, Line)> {
            folder(&self.account, &self.root, "")
        }
    }

    impl Drop for Stand {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
            if let Some(dir) = account_dir(&self.account) {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    /// Made at the first read, each message read once; added to; a file
    /// renamed by its flags keeps its line; compacted once the dead lines
    /// make half; made again when its first line is another format; yours alone.
    #[test]
    fn the_index_is_made_added_to_compacted_and_made_again() {
        let stand = Stand::new("life");
        let first = stand.put(1, "a@example.org", "One");
        stand.put(2, "b@example.org", "Two");
        maildir::parses_reset();
        let lines = stand.read();
        assert_eq!((lines.len(), maildir::parses()), (2, 2), "each read once");
        assert_eq!(stand.read().len(), 2);
        assert_eq!(maildir::parses(), 2, "read again: nothing parsed");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(stand.index()).unwrap().permissions().mode() & 0o777, 0o600);
        }
        // Added to: one new message, one line more.
        stand.put(3, "c@example.org", "Three");
        assert_eq!(stand.read().len(), 3);
        assert_eq!(maildir::parses(), 3);
        assert_eq!(std::fs::read_to_string(stand.index()).unwrap().lines().count(), 4, "the format and three lines");
        // Its flags changed: the same line, the new name.
        let renamed = first.with_file_name(format!("1777000000.U1-1.sioul{}2,FS", maildir::INFO));
        std::fs::rename(&first, &renamed).unwrap();
        let lines = stand.read();
        assert!(lines.iter().any(|(path, line)| *path == renamed && line.subject == "One"));
        assert_eq!(maildir::parses(), 3, "a rename reads nothing");
        // Two of three gone: compacted to the one left.
        std::fs::remove_file(&renamed).unwrap();
        std::fs::remove_file(stand.root.join("cur").join(format!("1777000000.U1-2.sioul{}2,S", maildir::INFO))).unwrap();
        assert_eq!(stand.read().len(), 1);
        assert_eq!(std::fs::read_to_string(stand.index()).unwrap().lines().count(), 2, "the format and the one line left");
        // Another format: made again.
        std::fs::write(stand.index(), "sioul-mail-index\t0\tolder\nwhatever\n").unwrap();
        assert_eq!(stand.read().len(), 1);
        assert_eq!(maildir::parses(), 4, "read again once");
        assert!(std::fs::read_to_string(stand.index()).unwrap().starts_with(FORMAT));
        // Another stamp (the words that find codes changed): its lines are read again.
        let text = std::fs::read_to_string(stand.index()).unwrap();
        let (head, lines) = text.split_once('\n').unwrap();
        std::fs::write(stand.index(), format!("{}\tother-words\n{lines}", head.rsplit_once('\t').unwrap().0)).unwrap();
        assert_eq!(stand.read().len(), 1);
        assert_eq!(maildir::parses(), 5, "read again with these words");
    }

    /// Two processes adding to one folder's index at once write each line
    /// once: the second, holding the lock after the first, reads its lines.
    #[test]
    fn two_writers_at_once_write_each_line_once() {
        let stand = Stand::new("writers");
        // The index made first: what comes next is added at its end.
        stand.put(1, "m1@example.org", "Hello");
        assert_eq!(stand.read().len(), 1);
        for uid in 2..=41 {
            stand.put(uid, &format!("m{uid}@example.org"), "Hello");
        }
        let start = std::sync::Barrier::new(4);
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    start.wait();
                    assert_eq!(stand.read().len(), 41);
                });
            }
        });
        let text = std::fs::read_to_string(stand.index()).unwrap();
        let mut uniques: Vec<&str> = text.lines().skip(1).map(|l| l.split('\t').next().unwrap()).collect();
        let before = uniques.len();
        uniques.sort_unstable();
        uniques.dedup();
        assert_eq!((before, uniques.len()), (41, 41), "each line once");
    }

    /// A compaction stopped before its rename leaves the old file whole and
    /// a file beside, which is never read; a stop that cut the last line
    /// leaves a line that does not read, skipped, its message read again.
    #[test]
    fn a_stop_half_way_leaves_a_readable_index() {
        let stand = Stand::new("stop");
        stand.put(1, "a@example.org", "One");
        stand.put(2, "b@example.org", "Two");
        assert_eq!(stand.read().len(), 2);
        // A compaction stopped before the rename: a partial file beside.
        std::fs::write(stand.index().with_extension("tsv.new"), "sioul-mail-index\t1\tcut").unwrap();
        maildir::parses_reset();
        assert_eq!(stand.read().len(), 2);
        assert_eq!(maildir::parses(), 0, "the file beside is never read");
        // An addition cut in its line: that line skipped, its message read again.
        let whole = std::fs::read_to_string(stand.index()).unwrap();
        let cut = &whole[..whole.len() - 20];
        std::fs::write(stand.index(), cut).unwrap();
        let lines = stand.read();
        assert_eq!(lines.len(), 2);
        assert_eq!(maildir::parses(), 1, "the cut line's message read again");
        assert_eq!(stand.read().len(), 2);
        assert_eq!(maildir::parses(), 1);
        // Cut in its last column, a line still has all its columns: without its line end, it is not taken.
        let lone = Stand::new("stop-digest");
        std::fs::write(lone.root.join("cur").join(format!("1777000000.U1-9.sioul{}2,S", maildir::INFO)), "From: Ana <ana@example.org>\r\nSubject: No id\r\n\r\nHi\r\n").unwrap();
        let named = lone.read()[0].1.digest.clone().unwrap();
        let whole = std::fs::read_to_string(lone.index()).unwrap();
        std::fs::write(lone.index(), &whole[..whole.len() - 3]).unwrap();
        maildir::parses_reset();
        assert_eq!(lone.read()[0].1.digest.as_deref(), Some(named.as_str()), "the cut digest is not taken");
        assert_eq!(maildir::parses(), 1);
    }

    /// A message another program wrote into the folder (no numbering in its
    /// name) is read once, as Sioul's own are.
    #[test]
    fn a_file_another_program_wrote_is_read_once() {
        let stand = Stand::new("other");
        std::fs::write(stand.root.join("cur").join("1777000000.M1P2.host:2,S"), "Message-ID: <x@example.org>\r\nSubject: From mutt\r\n\r\nHi\r\n").unwrap();
        maildir::parses_reset();
        assert_eq!(stand.read()[0].1.subject, "From mutt");
        stand.read();
        assert_eq!(maildir::parses(), 1);
    }

    /// The older single index goes once per device, when the folder of the
    /// indexes is first made; an older Sioul still running writes it again
    /// for itself, and it stays while it is written (nobody fights), then
    /// goes once nothing has written it for two weeks.
    #[test]
    fn the_older_single_index_goes_once() {
        let cache = std::env::temp_dir().join(format!("sioul-mailcache-retire-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);
        std::fs::create_dir_all(&cache).unwrap();
        let older = cache.join("mail-index.tsv");
        std::fs::write(&older, "1777000000.U1-1.sioul\ta@example.org\tHello\n").unwrap();
        let root = index_root_in(&cache);
        assert!(root.is_dir() && !older.exists(), "made, the older file gone");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&root).unwrap().permissions().mode() & 0o777, 0o700);
        }
        std::fs::write(&older, "written again by an older Sioul\n").unwrap();
        index_root_in(&cache);
        assert!(older.exists(), "theirs now: left alone");
        // Unwritten for two weeks and a day: no older Sioul runs any more, it goes.
        let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(15 * 86_400);
        std::fs::File::options().write(true).open(&older).unwrap().set_modified(long_ago).unwrap();
        index_root_in(&cache);
        assert!(!older.exists(), "left two weeks: gone");
        let _ = std::fs::remove_dir_all(&cache);
    }

    /// The lines say what the headers say: who, to whom, the ids answered,
    /// the results stamped, the digest of a message without a Message-ID.
    #[test]
    fn a_line_says_what_the_headers_say() {
        let raw = b"Authentication-Results: mx.example.org; dmarc=pass header.from=example.org\r\nFrom: Ana <Ana@Example.org>\r\nTo: Bo <bo@example.org>, cy@example.org\r\nDate: Fri, 30 Oct 2026 09:00:00 +0100\r\nIn-Reply-To: <a@example.org>\r\nReferences: <z@example.org> <a@example.org>\r\nSubject: Re: tab\there\r\n\r\nBody\r\n";
        let line = Line::of_raw("1.U1-1.sioul", 7, raw, &crate::words::Words::builtin()).unwrap();
        assert_eq!(Line::from_tsv(&line.to_tsv()), Some(line.clone()), "read back as written");
        assert_eq!((line.from_address.as_deref(), line.to.len(), line.refs()), (Some("ana@example.org"), 2, vec!["a@example.org".to_string(), "z@example.org".to_string()]));
        assert_eq!(line.digest.as_deref(), Some(digest(raw).as_str()), "no Message-ID: named by its digest");
        let card = line.card(Path::new("/stand-in/cur/1.U1-1.sioul:2,S"));
        assert_eq!(card.headers.all("Authentication-Results").count(), 1);
    }

    /// Sets a directory's time, as a change at `at` would have left it.
    fn set_dir_time(dir: &Path, at: std::time::SystemTime) {
        std::fs::File::open(dir).unwrap().set_modified(at).unwrap();
    }

    fn kept_subjects(recent: &Recent) -> Vec<String> {
        let mut subjects: Vec<String> = recent.lines.iter().map(|(_, l)| l.subject.clone()).collect();
        subjects.sort();
        subjects
    }

    /// The kept look (the unified mailbox's part 5): taken again without
    /// listing the folder or reading its index while nothing changed; looked
    /// at again after a flag rename, a new file, a removal, another process
    /// adding to the index, a compaction, an index made again, a change in
    /// the same tick of the clock as the look, and a clock stepped back. A
    /// stale look never shows a removed message, nor misses a new one.
    #[test]
    fn a_kept_look_is_never_stale() {
        use std::sync::Arc;
        use std::time::{Duration, SystemTime};
        let stand = Stand::new("kept");
        let first = stand.put(1, "a@example.org", "One");
        stand.put(2, "b@example.org", "Two");
        let long_ago = SystemTime::now() - Duration::from_secs(600);
        let settle = |at: SystemTime| {
            for sub in ["cur", "new"] {
                set_dir_time(&stand.root.join(sub), at);
            }
        };
        let look = || recent(&stand.account, &stand.root, "", 0, usize::MAX);
        settle(long_ago);
        let one = look();
        assert_eq!(kept_subjects(&one), ["One", "Two"]);
        let again = look();
        assert!(Arc::ptr_eq(&one.lines, &again.lines) && again.read == 0, "nothing changed: kept, nothing listed nor read");
        // A flag rename: the new name, never the old.
        let renamed = maildir::set_flags(&first, "RS").unwrap();
        let after = look();
        assert!(after.lines.iter().any(|(p, _)| *p == renamed) && !after.lines.iter().any(|(p, _)| *p == first), "renamed");
        // A new file, then a removal.
        settle(long_ago);
        look();
        let third = stand.put(3, "c@example.org", "Three");
        assert_eq!(kept_subjects(&look()), ["One", "Three", "Two"]);
        settle(long_ago - Duration::from_secs(60));
        look();
        std::fs::remove_file(&third).unwrap();
        assert_eq!(kept_subjects(&look()), ["One", "Two"], "removed: gone");
        // Another process adding to the index: looked at again.
        settle(long_ago);
        let before = look();
        let text = std::fs::read_to_string(stand.index()).unwrap();
        let last = text.lines().last().unwrap().to_string();
        std::fs::OpenOptions::new().append(true).open(stand.index()).unwrap().write_all(format!("{last}\n").as_bytes()).unwrap();
        let added = look();
        assert!(!Arc::ptr_eq(&before.lines, &added.lines), "the index grew: looked at again");
        assert_eq!(kept_subjects(&added), ["One", "Two"]);
        // A compaction (written beside, renamed), then an index made again.
        let kept = look();
        let beside = stand.index().with_extension("tsv.new");
        std::fs::write(&beside, std::fs::read(stand.index()).unwrap()).unwrap();
        std::fs::rename(&beside, stand.index()).unwrap();
        let compacted = look();
        assert!(!Arc::ptr_eq(&kept.lines, &compacted.lines), "compacted: looked at again");
        std::fs::remove_file(stand.index()).unwrap();
        let made = look();
        assert_eq!((kept_subjects(&made), made.read), (vec!["One".to_string(), "Two".to_string()], 2), "made again: read again");
        // A change in the same tick as the look: its directory's time unmoved, never trusted.
        settle(SystemTime::now());
        let racy = look();
        let at = std::fs::metadata(stand.root.join("cur")).unwrap().modified().unwrap();
        let fourth = stand.put(4, "d@example.org", "Four");
        set_dir_time(&stand.root.join("cur"), at);
        assert!(!Arc::ptr_eq(&racy.lines, &look().lines));
        assert_eq!(kept_subjects(&look()), ["Four", "One", "Two"], "a new file in the same tick: seen");
        // A clock stepped back: a time ahead of the clock is never trusted; a time moved back is a change.
        settle(SystemTime::now() + Duration::from_secs(3_600));
        let ahead = look();
        assert!(!Arc::ptr_eq(&ahead.lines, &look().lines), "ahead of the clock: looked at again");
        settle(long_ago);
        let settled = look();
        std::fs::remove_file(&fourth).unwrap();
        set_dir_time(&stand.root.join("cur"), long_ago - Duration::from_secs(3_600));
        let back = look();
        assert!(!Arc::ptr_eq(&settled.lines, &back.lines));
        assert_eq!(kept_subjects(&back), ["One", "Two"], "removed while the clock went back: gone");
    }

    /// A wider window than the one kept (Earlier messages): looked at again,
    /// its older lines there; a narrower one is taken from the kept look.
    #[test]
    fn a_wider_window_looks_again() {
        use std::sync::Arc;
        use std::time::{Duration, SystemTime};
        let stand = Stand::new("wider");
        let now = SystemTime::now();
        for (uid, days) in [(1u32, 10u64), (2, 9), (3, 1)] {
            let path = stand.put(uid, &format!("w{uid}@example.org"), &format!("Note {uid}"));
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(now - Duration::from_secs(days * 86_400)).unwrap();
        }
        for sub in ["cur", "new"] {
            set_dir_time(&stand.root.join(sub), now - Duration::from_secs(600));
        }
        let at = |days: u64| (now - Duration::from_secs(days * 86_400)).duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let near = recent(&stand.account, &stand.root, "", at(5), usize::MAX);
        assert_eq!((near.lines.len(), near.older), (1, true));
        let nearer = recent(&stand.account, &stand.root, "", at(3), usize::MAX);
        assert!(Arc::ptr_eq(&near.lines, &nearer.lines), "narrower: the kept look");
        let wide = recent(&stand.account, &stand.root, "", at(20), usize::MAX);
        assert_eq!((wide.lines.len(), wide.older), (3, false), "wider: looked at again, the older lines there");
    }

    /// The first index in steps: the newest first, `fill` at a time, the
    /// others waiting; what arrived before the window waits as older mail.
    #[test]
    fn the_first_index_fills_the_newest_first() {
        use std::time::{Duration, SystemTime};
        let stand = Stand::new("steps");
        let now = SystemTime::now();
        for uid in 1..=5u32 {
            let path = stand.put(uid, &format!("m{uid}@example.org"), &format!("Note {uid}"));
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(now - Duration::from_secs(u64::from(10 - uid) * 86_400)).unwrap();
        }
        let since = (now - Duration::from_secs(6 * 86_400)).duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let first = recent(&stand.account, &stand.root, "", since, 2);
        assert_eq!((first.read, first.waiting, first.older), (2, 3, true));
        // The two newest: 4 and 5; their Date is older than they arrived, so they are kept by it.
        assert_eq!(first.lines.iter().map(|(_, l)| l.arrived).min().map(|a| a >= since), Some(true));
        let next = recent(&stand.account, &stand.root, "", since, 2);
        assert_eq!((next.read, next.waiting), (2, 1));
        let last = recent(&stand.account, &stand.root, "", since, 2);
        assert_eq!((last.read, last.waiting), (1, 0));
        assert_eq!(recent(&stand.account, &stand.root, "", since, 2).read, 0, "filled: kept");
    }
}

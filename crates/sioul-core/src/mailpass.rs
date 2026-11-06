// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Readers of every message kept on this device (the money ledger's design
//! §5, "Found once"): what one reader finds in each message, read once,
//! ever, and kept beside the mail cache (`mailcache`), one small file per
//! folder. Money (`moneyindex`) is one reader; the mail's topics may be
//! another: a reader says its name, the stamp of what it depends on, which
//! messages are worth opening from their cached headers alone (`wanted`),
//! and what it finds in one (`read`). Everything else is here, the same for
//! every reader:
//!
//! - **Where:** `$XDG_CACHE_HOME/sioul/<name>/<account>/<folder>.tsv`, the
//!   folder named by a digest of its directory. Yours alone (0600, its
//!   folders 0700), never shared nor synced: each device keeps its own, as
//!   it keeps its own Maildir. Removed with the address (`forget_account`,
//!   for every reader named in `NAMES`).
//! - **Its first line** names the reader, its format and its stamp. Another
//!   first line: the file is made again from the messages, in the
//!   background, never the mail cache. Nothing is migrated: it is a cache.
//! - **One line per message**, keyed as the mail cache keys it (the part of
//!   its file name that never changes): `-` for nothing found, or `+` and
//!   the reader's columns. A message the reader does not want is said
//!   nothing without being opened: the cost is the mail cache's alone.
//! - **Each message is read once:** a name the file does not know is looked
//!   at once and its line added at the end, under the file's own lock
//!   (`filelock::with_lock`), so that the window, the phone's background step
//!   and `sioul mcp` never add it twice. Lines whose messages are gone stay
//!   until they make half the file; it is then written again beside and renamed.

use crate::mailcache;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

/// The readers this version keeps files for, by their folder's name: an
/// address forgotten takes its files with it (`forget_account`).
pub const NAMES: &[&str] = &[crate::moneyindex::NAME];

/// The format, after the reader's name in the first line.
const FORMAT: &str = "1";

/// What reads every message kept on this device.
pub trait Reader {
    /// Its folder under the cache ("money-index"): a plain name, one of `NAMES`.
    fn name(&self) -> &'static str;
    /// What its findings depend on (words, rules): another stamp, its files made again.
    fn stamp(&self) -> String;
    /// Whether a message is worth opening, from what the mail cache says of it alone.
    fn wanted(&self, line: &mailcache::Line) -> bool;
    /// What it finds in a message opened (its bytes): its columns, none for nothing.
    /// A column holds no tab and no line end.
    fn read(&self, line: &mailcache::Line, path: &Path, raw: &[u8]) -> Option<Vec<String>>;
}

/// One message of a folder, with what the mail cache and the reader say of it.
#[derive(Debug, Clone)]
pub struct Found {
    pub path: PathBuf,
    pub line: mailcache::Line,
    /// The reader's columns; none for nothing.
    pub columns: Option<Vec<String>>,
}

/// A folder looked at: its messages, and how many wait to be read (`fill`).
#[derive(Debug, Clone, Default)]
pub struct Pass {
    pub found: Vec<Found>,
    pub waiting: usize,
    /// Messages this look opened.
    pub opened: usize,
}

/// A column as the file keeps it: tabs and line ends made spaces.
pub fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

fn plain(name: &str) -> bool {
    matches!(Path::new(name).components().collect::<Vec<_>>().as_slice(), [Component::Normal(_)])
}

/// Where a reader keeps what it found in the folder whose directory is `dir`
/// (the Maildir itself, or one of its folders); none for an account whose id
/// is not one plain name.
pub fn file_of(name: &str, account: &str, dir: &Path) -> Option<PathBuf> {
    if !plain(account) || !plain(name) {
        return None;
    }
    // As its components say it: "Mail/" and "Mail" are one folder.
    let dir: PathBuf = dir.components().collect();
    let hash: String = Sha256::digest(dir.to_string_lossy().as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect();
    Some(crate::config::cache_dir().join(name).join(account).join(format!("{hash}.tsv")))
}

/// The folder of a message's file: its `cur/` or `new/`'s parent.
pub fn folder_of(message: &Path) -> Option<&Path> {
    message.parent()?.parent()
}

fn private_dir(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
}

fn private_file(path: &Path, options: &mut std::fs::OpenOptions) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn header_of(reader: &dyn Reader) -> String {
    format!("sioul-pass\t{}\t{FORMAT}\t{}", reader.name(), clean(&reader.stamp()))
}

fn line_text(unique: &str, columns: &Option<Vec<String>>) -> String {
    match columns {
        None => format!("{}\t-", clean(unique)),
        Some(columns) => format!("{}\t+\t{}", clean(unique), columns.iter().map(|c| clean(c)).collect::<Vec<_>>().join("\t")),
    }
}

/// The file's findings by unique part, and how many lines it holds; none
/// when its first line is not this reader's with this stamp.
fn read_file(path: &Path, header: &str) -> Option<(HashMap<String, Option<Vec<String>>>, usize)> {
    let bytes = std::fs::read(path).ok()?;
    // A stop may cut the last line: every line written ends with a line end.
    let whole = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |end| end + 1);
    let text = String::from_utf8_lossy(&bytes[..whole]);
    let mut lines = text.split_terminator('\n');
    if lines.next()? != header {
        return None;
    }
    let mut known = HashMap::new();
    let mut count = usize::from(whole < bytes.len());
    for line in lines {
        count += 1;
        let mut parts = line.split('\t');
        let (Some(unique), Some(mark)) = (parts.next(), parts.next()) else { continue };
        match mark {
            "-" => {
                known.insert(unique.to_string(), None);
            }
            "+" => {
                known.insert(unique.to_string(), Some(parts.map(str::to_string).collect()));
            }
            _ => {}
        }
    }
    Some((known, count))
}

fn write_whole(path: &Path, header: &str, lines: &[String]) -> std::io::Result<()> {
    let beside = path.with_extension("tsv.new");
    let mut text = String::with_capacity(lines.len() * 64 + header.len() + 1);
    text.push_str(header);
    text.push('\n');
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    let mut file = private_file(&beside, std::fs::OpenOptions::new().write(true).create(true).truncate(true))?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&beside, path)
}

fn append(path: &Path, lines: &[String]) -> std::io::Result<()> {
    let mut file = private_file(path, std::fs::OpenOptions::new().read(true).append(true).create(true))?;
    let mut text = String::new();
    let length = file.metadata()?.len();
    if length > 0 {
        let mut last = [0u8; 1];
        file.seek(std::io::SeekFrom::Start(length - 1))?;
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            text.push('\n');
        }
    }
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    file.write_all(text.as_bytes())
}

/// One folder of `account` (its Maildir `root`, the folder `local`, "" for
/// the inbox), each message with what the reader found: kept findings read
/// from its file, the messages it does not know yet looked at (the newest
/// first), at most `fill` of them opened; the others wait, said nothing yet.
pub fn folder(reader: &dyn Reader, account: &str, root: &Path, local: &str, fill: usize) -> Pass {
    let dir = root.join(local);
    let mut lines = mailcache::folder(account, root, local);
    // The newest first: a bounded look reads those, which the first page needs.
    lines.sort_by_key(|(_, l)| std::cmp::Reverse(l.arrived));
    let Some(path) = file_of(reader.name(), account, &dir) else {
        let found = lines.into_iter().map(|(path, line)| {
            let columns = if reader.wanted(&line) { std::fs::read(&path).ok().and_then(|raw| reader.read(&line, &path, &raw)) } else { None };
            Found { path, line, columns }
        });
        let found: Vec<Found> = found.collect();
        let opened = found.iter().filter(|f| f.columns.is_some()).count();
        return Pass { found, waiting: 0, opened };
    };
    if let Some(parent) = path.parent() {
        private_dir(&crate::config::cache_dir().join(reader.name()));
        private_dir(parent);
    }
    let header = header_of(reader);
    crate::filelock::with_lock(&path, || {
        let (known, count, fresh) = match read_file(&path, &header) {
            Some((known, count)) => (known, count, false),
            None => (HashMap::new(), 0, true),
        };
        let mut found = Vec::with_capacity(lines.len());
        let mut added: Vec<String> = Vec::new();
        let mut opened = 0usize;
        let mut waiting = 0usize;
        for (file, line) in lines {
            if let Some(columns) = known.get(&line.unique) {
                found.push(Found { path: file, line, columns: columns.clone() });
                continue;
            }
            let columns = if reader.wanted(&line) {
                if opened >= fill {
                    waiting += 1;
                    continue;
                }
                opened += 1;
                std::fs::read(&file).ok().and_then(|raw| reader.read(&line, &file, &raw))
            } else {
                None
            };
            added.push(line_text(&line.unique, &columns));
            found.push(Found { path: file, line, columns });
        }
        let live: std::collections::HashSet<&str> = found.iter().map(|f| f.line.unique.as_str()).collect();
        let dead = known.keys().filter(|u| !live.contains(u.as_str())).count() + count.saturating_sub(known.len());
        if fresh || (dead > 0 && dead * 2 >= count + added.len()) {
            let all: Vec<String> = found.iter().map(|f| line_text(&f.line.unique, &f.columns)).collect();
            let _ = write_whole(&path, &header, &all);
        } else if !added.is_empty() {
            let _ = append(&path, &added);
        }
        Pass { found, waiting, opened }
    })
}

/// Every folder of every address: (its account id, its Maildir), each folder
/// looked at (`folder`), at most `fill` messages opened in all.
pub fn every(reader: &dyn Reader, accounts: &[(String, PathBuf)], fill: usize) -> Vec<(String, String, Pass)> {
    let mut left = fill;
    let mut out = Vec::new();
    for (account, root) in accounts {
        for local in mailcache::folders(root) {
            let pass = folder(reader, account, root, &local, left);
            left = left.saturating_sub(pass.opened);
            out.push((account.clone(), local, pass));
        }
    }
    out
}

/// The files kept in this process, by their path, length and time.
type Parsed = (u64, Option<std::time::SystemTime>, String, std::sync::Arc<HashMap<String, Option<Vec<String>>>>);
static PARSED: std::sync::Mutex<Option<HashMap<PathBuf, Parsed>>> = std::sync::Mutex::new(None);

/// What the reader found in the message `message` (its file) of `account`,
/// when its folder's file holds it with this reader's stamp: `Some(None)`
/// for nothing, `Some(Some(columns))`; none when it was not looked at yet.
/// Each file is read once while it stays as it is.
pub fn kept_for(reader: &dyn Reader, account: &str, message: &Path) -> Option<Option<Vec<String>>> {
    let dir = folder_of(message)?;
    let path = file_of(reader.name(), account, dir)?;
    let unique = crate::maildir::unique_part(&message.file_name()?.to_string_lossy()).to_string();
    let meta = std::fs::metadata(&path).ok()?;
    let (len, at) = (meta.len(), meta.modified().ok());
    let header = header_of(reader);
    let mut parsed = PARSED.lock().ok()?;
    let parsed = parsed.get_or_insert_with(HashMap::new);
    if let Some((l, t, h, known)) = parsed.get(&path)
        && *l == len
        && *t == at
        && *h == header
    {
        return known.get(&unique).cloned();
    }
    let (known, _) = read_file(&path, &header)?;
    let known = std::sync::Arc::new(known);
    parsed.insert(path, (len, at, header, std::sync::Arc::clone(&known)));
    known.get(&unique).cloned()
}

/// Every reader's files of an address, gone with it (`maildir::forget_account`).
pub fn forget_account(account: &str) -> Vec<PathBuf> {
    if !plain(account) {
        return Vec::new();
    }
    let mut gone = Vec::new();
    for name in NAMES {
        let dir = crate::config::cache_dir().join(name).join(account);
        if dir.is_dir() && std::fs::remove_dir_all(&dir).is_ok() {
            gone.push(dir);
        }
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader of stand-ins: messages whose subject says "money", their subject's length.
    struct Lengths {
        stamp: &'static str,
        read: std::cell::Cell<usize>,
    }

    impl Reader for Lengths {
        fn name(&self) -> &'static str {
            crate::moneyindex::NAME
        }
        fn stamp(&self) -> String {
            self.stamp.into()
        }
        fn wanted(&self, line: &mailcache::Line) -> bool {
            line.subject.contains("money")
        }
        fn read(&self, line: &mailcache::Line, _: &Path, raw: &[u8]) -> Option<Vec<String>> {
            self.read.set(self.read.get() + 1);
            (!raw.is_empty()).then(|| vec![line.subject.len().to_string()])
        }
    }

    /// Each message read once, ever; what is not wanted never opened; a
    /// bounded look leaves the rest waiting; another stamp makes the file again.
    #[test]
    fn each_message_is_read_once_by_each_reader() {
        let account = format!("stand-pass-{}", std::process::id());
        let root = std::env::temp_dir().join(format!("sioul-mailpass-{account}"));
        let _ = std::fs::remove_dir_all(&root);
        for sub in ["tmp", "new", "cur"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        for (n, subject) in ["About money, again", "A picnic", "More money"].iter().enumerate() {
            let raw = format!("Message-ID: <p{n}@example.org>\r\nFrom: Lou <lou@example.org>\r\nSubject: {subject}\r\nDate: Thu, 01 Oct 2026 10:0{n}:00 +0200\r\n\r\nText.\r\n");
            std::fs::write(root.join("cur").join(format!("17770000{n}0.U1-{n}.stand{}2,S", crate::maildir::INFO)), raw).unwrap();
        }
        let reader = Lengths { stamp: "one", read: std::cell::Cell::new(0) };
        let first = folder(&reader, &account, &root, "", 1);
        assert_eq!((first.waiting, reader.read.get()), (1, 1), "one opened, one waits, the picnic never opened");
        let second = folder(&reader, &account, &root, "", 10);
        assert_eq!((second.waiting, reader.read.get(), second.found.len()), (0, 2, 3));
        let again = folder(&reader, &account, &root, "", 10);
        assert_eq!(reader.read.get(), 2, "read once, ever");
        let money: Vec<&str> = again.found.iter().filter(|f| f.columns.is_some()).map(|f| f.line.subject.as_str()).collect();
        assert_eq!(money.len(), 2);
        let message = again.found.iter().find(|f| f.line.subject == "A picnic").unwrap().path.clone();
        assert_eq!(kept_for(&reader, &account, &message), Some(None));
        let other = Lengths { stamp: "two", read: std::cell::Cell::new(0) };
        assert_eq!(kept_for(&other, &account, &message), None, "another stamp: not kept");
        folder(&other, &account, &root, "", 10);
        assert_eq!(other.read.get(), 2, "made again from the messages");
        assert!(!forget_account(&account).is_empty());
        let _ = std::fs::remove_dir_all(&root);
        if let Some(dir) = mailcache::account_dir(&account) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

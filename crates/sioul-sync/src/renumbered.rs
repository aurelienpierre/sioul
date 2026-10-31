// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A renumbered folder's old copies (the unified mailbox's design, §5.4).
//!
//! When a server gives a folder a new UIDVALIDITY, the sync starts that
//! folder over and fetches its first window again under the new numbering
//! (`fetch::fetch_folder`); every later step reads the new numbering only.
//! The files of the old numbering would stay for good: the folder would show
//! its mail twice, and the old copies' flags would never follow the server's.
//!
//! This module removes them, refusing by default, since it deletes files of
//! the person's Maildir:
//! - only in the folder's own directory, never across folders or addresses;
//! - only once the sync state records the new numbering (its first window
//!   fetched), and at the end of a pass that went through (`fetch_folder`'s
//!   full pass, under the account's fetch lock, which every process takes);
//! - an old copy is removed when the new numbering holds a copy of the same
//!   message: the same Message-ID, or, for a message without one, the same
//!   digest of its From, To, Date and Subject lines (`key_of`); copies are
//!   paired one to one, and two keys must be equal (a Message-ID never pairs
//!   with a digest, two different ids never pair);
//! - an old copy no new copy pairs with is removed only once the folder is
//!   fetched whole under the new numbering: the server no longer holds it;
//! - a file whose headers do not read is never removed: nothing on a guess.
//!
//! Each removal writes one line in `$XDG_STATE_HOME/sioul/sync/<account>.removed.log`
//! (this device's own, never shared): when, the folder's server name, the old
//! copy's UIDVALIDITY and UID, the new copy's or "-". Never a subject, never an
//! address. A stop between two removals leaves the rest for the next pass.

use sha2::{Digest, Sha256};
use sioul_core::card::ImapOrigin;
use sioul_core::maildir;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// One old copy to remove, and the new copy it was paired with, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub path: PathBuf,
    pub old: ImapOrigin,
    pub new: Option<ImapOrigin>,
}

/// The files of a folder (`new/` and `cur/` under `root`) named by their
/// server's numbering, with that numbering.
fn numbered(root: &Path) -> Vec<(PathBuf, ImapOrigin)> {
    let mut out: Vec<(PathBuf, ImapOrigin)> = ["new", "cur"]
        .iter()
        .filter_map(|sub| std::fs::read_dir(root.join(sub)).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter_map(|p| maildir::origin_of(&p).map(|o| (p, o)))
        .collect();
    out.sort_by_key(|(_, origin)| *origin);
    out
}

/// A message's headers: what comes before its first empty line, at most 256 KB.
fn headers(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(8192);
    std::fs::File::open(path).ok()?.take(256 * 1024).read_to_end(&mut bytes).ok()?;
    let end = bytes.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| bytes.windows(2).position(|w| w == b"\n\n"))?;
    bytes.truncate(end);
    Some(bytes)
}

/// The header fields as the file holds them, each with its folded lines,
/// line endings as "\n": (lowercase name, the whole field).
fn fields(raw: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(raw).replace("\r\n", "\n");
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

/// What names a copy for the pairing: "id:" and its Message-ID (angle
/// brackets off), else "digest:" and 16 hexadecimal digits of SHA-256 over
/// its From, To, Date and Subject fields as the file holds them (the
/// mailbox's §3.1: the server gives every device the same bytes). None when
/// its headers do not read.
pub fn key_of(path: &Path) -> Option<String> {
    let fields = fields(&headers(path)?);
    if fields.is_empty() {
        return None;
    }
    let value = |field: &str| field.split_once(':').map_or("", |(_, v)| v).split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some((_, field)) = fields.iter().find(|(name, _)| name == "message-id") {
        let id = value(field).trim().trim_start_matches('<').trim_end_matches('>').trim().to_string();
        if !id.is_empty() {
            return Some(format!("id:{id}"));
        }
    }
    let mut hasher = Sha256::new();
    for wanted in ["from", "to", "date", "subject"] {
        if let Some((_, field)) = fields.iter().find(|(name, _)| name == wanted) {
            hasher.update(field.as_bytes());
        }
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    Some(format!("digest:{}", digest.iter().take(8).map(|b| format!("{b:02x}")).collect::<String>()))
}

/// The old copies to remove from the folder at `root`, whose current
/// UIDVALIDITY is `validity`. `recorded` is what the sync state records of the
/// folder: its UIDVALIDITY and whether it is fetched whole. Nothing while the
/// state does not record `validity` (the new numbering's first window not
/// fetched yet); nothing when the folder holds no old copy (then no file is read).
pub fn plan(root: &Path, validity: u32, recorded: Option<(u32, bool)>) -> Vec<Removal> {
    let Some((recorded_validity, whole)) = recorded else { return Vec::new() };
    if recorded_validity != validity {
        return Vec::new();
    }
    let files = numbered(root);
    let (current, old): (Vec<_>, Vec<_>) = files.into_iter().partition(|(_, origin)| origin.validity == validity);
    if old.is_empty() {
        return Vec::new();
    }
    // The new numbering's copies by key, each paired once at most.
    let mut new_by_key: HashMap<String, Vec<ImapOrigin>> = HashMap::new();
    for (path, origin) in &current {
        if let Some(key) = key_of(path) {
            new_by_key.entry(key).or_default().push(*origin);
        }
    }
    let mut out = Vec::new();
    for (path, origin) in old {
        // A copy whose headers do not read: kept, whatever else is known.
        let Some(key) = key_of(&path) else { continue };
        match new_by_key.get_mut(&key).filter(|copies| !copies.is_empty()) {
            Some(copies) => {
                let new = copies.remove(0);
                out.push(Removal { path, old: origin, new: Some(new) });
            }
            None if whole => out.push(Removal { path, old: origin, new: None }),
            None => {}
        }
    }
    out
}

/// Removes the old copies `plan` found, each followed by its line in `log`:
/// when, `folder` (its server name), the old copy's numbering, the new one's
/// or "-". How many were removed; a file gone meanwhile is skipped.
pub fn remove(removals: &[Removal], folder: &str, log: &Path) -> usize {
    let mut removed = 0;
    for removal in removals {
        if std::fs::remove_file(&removal.path).is_err() {
            continue;
        }
        removed += 1;
        let new = removal.new.map_or_else(|| "-".to_string(), |o| format!("U{}-{}", o.validity, o.uid));
        let line = format!("{}\t{}\tU{}-{}\t{new}\n", jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%SZ"), folder.replace(['\t', '\n', '\r'], " "), removal.old.validity, removal.old.uid);
        append(log, &line);
    }
    removed
}

/// One line added to the log, readable by you alone.
fn append(log: &Path, line: &str) {
    use std::io::Write;
    if let Some(parent) = log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut options = std::fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(log) {
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Folder {
        root: PathBuf,
    }

    impl Folder {
        fn new(name: &str) -> Folder {
            let root = std::env::temp_dir().join(format!("sioul-renumbered-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            for sub in ["tmp", "new", "cur"] {
                std::fs::create_dir_all(root.join(sub)).unwrap();
            }
            Folder { root }
        }

        /// A copy of a message under the numbering `validity`, as `maildir::store` names it.
        fn put(&self, validity: u32, uid: u32, headers: &str) -> PathBuf {
            let path = self.root.join("cur").join(format!("1777000000.U{validity}-{uid}.sioul{}2,S", maildir::INFO));
            std::fs::write(&path, format!("{headers}\r\n\r\nThe text.\r\n")).unwrap();
            path
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn message(id: &str, subject: &str) -> String {
        format!("Message-ID: <{id}>\r\nFrom: Ana <ana@example.org>\r\nTo: me@example.org\r\nDate: Fri, 30 Oct 2026 09:00:00 +0100\r\nSubject: {subject}")
    }

    fn origin(validity: u32, uid: u32) -> ImapOrigin {
        ImapOrigin { validity, uid }
    }

    /// An old copy goes once the new numbering holds the same Message-ID, its
    /// line logged without a subject or an address; the new copy stays.
    #[test]
    fn an_old_copy_goes_once_the_new_numbering_holds_its_id() {
        let folder = Folder::new("matched");
        let old = folder.put(1, 5, &message("a@example.org", "Hello"));
        let new = folder.put(2, 9, &message("a@example.org", "Hello"));
        let plan = plan(&folder.root, 2, Some((2, false)));
        assert_eq!(plan, [Removal { path: old.clone(), old: origin(1, 5), new: Some(origin(2, 9)) }]);
        let log = folder.root.join("removed.log");
        assert_eq!(remove(&plan, "INBOX", &log), 1);
        assert!(!old.exists() && new.exists());
        let logged = std::fs::read_to_string(&log).unwrap();
        assert!(logged.ends_with("\tINBOX\tU1-5\tU2-9\n"), "{logged}");
        assert!(!logged.contains("Hello") && !logged.contains("example.org"), "no subject, no address: {logged}");
    }

    /// An old copy the new numbering does not hold stays while the folder is
    /// not fetched whole, and goes once it is.
    #[test]
    fn an_unmatched_copy_waits_until_the_folder_is_whole() {
        let folder = Folder::new("unmatched");
        let old = folder.put(1, 6, &message("b@example.org", "Older"));
        folder.put(2, 1, &message("c@example.org", "Newer"));
        assert!(plan(&folder.root, 2, Some((2, false))).is_empty(), "not whole: kept");
        assert_eq!(plan(&folder.root, 2, Some((2, true))), [Removal { path: old, old: origin(1, 6), new: None }]);
    }

    /// A message without a Message-ID is matched by its digest.
    #[test]
    fn a_message_without_an_id_is_matched_by_its_digest() {
        let folder = Folder::new("digest");
        let lines = "From: Shop <no-reply@example.org>\r\nTo: me@example.org\r\nDate: Fri, 30 Oct 2026 08:00:00 +0100\r\nSubject: Your order";
        let old = folder.put(1, 7, lines);
        let new = folder.put(2, 10, lines);
        assert!(key_of(&old).unwrap().starts_with("digest:"));
        assert_eq!(plan(&folder.root, 2, Some((2, false))), [Removal { path: old, old: origin(1, 7), new: Some(origin(2, 10)) }]);
        assert!(new.exists());
    }

    /// Nothing is removed whose ids differ: the same sender, subject and date
    /// under another Message-ID, or a Message-ID beside a digest.
    #[test]
    fn nothing_is_removed_whose_ids_differ() {
        let folder = Folder::new("differ");
        folder.put(1, 8, &message("c@example.org", "Hello"));
        folder.put(2, 11, &message("d@example.org", "Hello"));
        folder.put(1, 12, "From: Ana <ana@example.org>\r\nTo: me@example.org\r\nDate: Fri, 30 Oct 2026 09:00:00 +0100\r\nSubject: Hello");
        assert!(plan(&folder.root, 2, Some((2, false))).is_empty());
    }

    /// The new numbering not recorded yet (its first window not fetched, or
    /// stopped before the state said it): nothing removed, matches or not.
    #[test]
    fn nothing_goes_while_the_new_numbering_is_incomplete() {
        let folder = Folder::new("incomplete");
        let old = folder.put(1, 5, &message("a@example.org", "Hello"));
        folder.put(2, 9, &message("a@example.org", "Hello"));
        folder.put(1, 6, &message("b@example.org", "Older"));
        assert!(plan(&folder.root, 2, None).is_empty(), "nothing recorded");
        assert!(plan(&folder.root, 2, Some((1, true))).is_empty(), "the old numbering still recorded");
        assert!(old.exists());
    }

    /// Two old copies of one Message-ID and one new copy: one goes, the other
    /// waits until the folder is whole; a file whose headers do not read never goes.
    #[test]
    fn copies_pair_one_to_one_and_unreadable_ones_stay() {
        let folder = Folder::new("pairs");
        let first = folder.put(1, 5, &message("a@example.org", "Hello"));
        let second = folder.put(1, 6, &message("a@example.org", "Hello"));
        folder.put(2, 9, &message("a@example.org", "Hello"));
        let unreadable = folder.root.join("cur").join(format!("1777000000.U1-7.sioul{}2,S", maildir::INFO));
        std::fs::write(&unreadable, "no headers at all").unwrap();
        assert_eq!(plan(&folder.root, 2, Some((2, false))), [Removal { path: first, old: origin(1, 5), new: Some(origin(2, 9)) }]);
        assert_eq!(plan(&folder.root, 2, Some((2, true))).iter().map(|r| r.path.clone()).collect::<Vec<_>>()[1], second);
        assert!(!plan(&folder.root, 2, Some((2, true))).iter().any(|r| r.path == unreadable), "unreadable: kept");
    }

    /// Another folder's and another address's copies are never looked at: the
    /// plan reads its own folder alone.
    #[test]
    fn another_folder_is_never_touched() {
        let (inbox, archive) = (Folder::new("inbox"), Folder::new("archive"));
        let elsewhere = archive.put(1, 5, &message("a@example.org", "Hello"));
        inbox.put(2, 9, &message("a@example.org", "Hello"));
        inbox.put(1, 3, &message("e@example.org", "Mine"));
        assert!(plan(&inbox.root, 2, Some((2, false))).is_empty());
        assert!(plan(&archive.root, 1, Some((1, true))).is_empty(), "the archive's own numbering: its copy is current");
        assert!(elsewhere.exists());
    }
}

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
//! - only in the folder's own directory, never across folders or addresses,
//!   and nothing at all in a directory another address's Maildir also
//!   covers (the same directory, or one inside the other: `shared`), where
//!   another numbering may be that address's own mail;
//! - only once the sync state records the new numbering (its first window
//!   fetched), and at the end of a pass that went through (`fetch_folder`'s
//!   full pass, under the account's fetch lock, which every process takes);
//! - an old copy is removed when the new numbering holds a copy of the same
//!   message: the same Message-ID, or, for a message without one, the same
//!   digest of its From, To, Date and Subject lines (`key_of`); copies are
//!   paired one to one, and two keys must be equal (a Message-ID never pairs
//!   with a digest, two different ids never pair);
//! - an old copy no new copy pairs with is removed only when its numbering is
//!   one this device saw the folder leave (`Left`, noted as the sync meets a
//!   new UIDVALIDITY), once the folder is fetched whole under the new
//!   numbering and the server's own list of UIDs is all here (`confirmed`):
//!   the server no longer holds it. A numbering not noted here (left before
//!   this build, or seen first by another version of Sioul) loses its paired
//!   copies alone;
//! - a file whose headers do not read is never removed: nothing on a guess.
//!
//! Each removal writes one line in `$XDG_STATE_HOME/sioul/sync/<account>.removed.log`
//! (this device's own, never shared): when, the folder's server name, the old
//! copy's UIDVALIDITY and UID, the new copy's or "-". Never a subject, never an
//! address. A stop between two removals leaves the rest for the next pass.
//! The numberings left are kept in `$XDG_STATE_HOME/sioul/sync/<account>.renumbered.toml`
//! (folder → old UIDVALIDITIES), which no older Sioul reads or writes; a
//! numbering is forgotten once the folder holds no file of it.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sioul_core::card::ImapOrigin;
use sioul_core::maildir;
use std::collections::{BTreeMap, BTreeSet, HashMap};
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

/// What may go of a folder's old copies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Old copies paired with a copy of the same message under the new numbering.
    pub paired: Vec<Removal>,
    /// Old copies of a numbering this device saw the folder leave, paired with
    /// none, the folder fetched whole by the state: they go only once the
    /// server's list of UIDs is all here (`confirmed`).
    pub unmatched: Vec<Removal>,
}

/// The old copies that may go from the folder at `root`, whose current
/// UIDVALIDITY is `validity`. `recorded` is what the sync state records of the
/// folder: its UIDVALIDITY and whether it is fetched whole; `left`, the
/// numberings this device saw it leave (`Left`); `shared`, whether another
/// address's Maildir covers its directory (`shared`). Nothing in a shared
/// directory; nothing while the state does not record `validity` (the new
/// numbering's first window not fetched yet); nothing when the folder holds no
/// old copy (then no file is read).
pub fn plan(root: &Path, validity: u32, recorded: Option<(u32, bool)>, left: &BTreeSet<u32>, shared: bool) -> Plan {
    if shared {
        return Plan::default();
    }
    let Some((recorded_validity, whole)) = recorded else { return Plan::default() };
    if recorded_validity != validity {
        return Plan::default();
    }
    let files = numbered(root);
    let (current, old): (Vec<_>, Vec<_>) = files.into_iter().partition(|(_, origin)| origin.validity == validity);
    if old.is_empty() {
        return Plan::default();
    }
    // The new numbering's copies by key, each paired once at most.
    let mut new_by_key: HashMap<String, Vec<ImapOrigin>> = HashMap::new();
    for (path, origin) in &current {
        if let Some(key) = key_of(path) {
            new_by_key.entry(key).or_default().push(*origin);
        }
    }
    let mut out = Plan::default();
    for (path, origin) in old {
        // A copy whose headers do not read: kept, whatever else is known.
        let Some(key) = key_of(&path) else { continue };
        match new_by_key.get_mut(&key).filter(|copies| !copies.is_empty()) {
            Some(copies) => {
                let new = copies.remove(0);
                out.paired.push(Removal { path, old: origin, new: Some(new) });
            }
            // Only a numbering noted here as left: one before this build, or
            // seen first by an older Sioul, keeps what nothing pairs with.
            None if whole && left.contains(&origin.validity) => out.unmatched.push(Removal { path, old: origin, new: None }),
            None => {}
        }
    }
    out
}

/// The unmatched old copies that go: all of them when every UID the server
/// lists for the folder (`UID SEARCH ALL`) is here under the new numbering
/// (`here`), else none. "Whole" in the sync state says a round listed them;
/// this says each one came.
pub fn confirmed(unmatched: Vec<Removal>, server: impl IntoIterator<Item = u32>, here: &BTreeSet<u32>) -> Vec<Removal> {
    let mut server = server.into_iter();
    if server.all(|uid| here.contains(&uid)) { unmatched } else { Vec::new() }
}

/// Whether another address's Maildir covers the directory `own` (an
/// address's Maildir): the same directory, or one inside the other,
/// compared as the system resolves them (links, `..`).
pub fn shared(own: &Path, others: &[PathBuf]) -> bool {
    let resolved = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let own = resolved(own);
    others.iter().map(|other| resolved(other)).any(|other| other == own || other.starts_with(&own) || own.starts_with(&other))
}

/// Whether another address of the configuration shares `account`'s Maildir
/// (`shared`). A configuration that does not read counts as shared: nothing
/// is removed on a guess.
pub fn shared_with_others(account: &sioul_core::config::Account) -> bool {
    let Ok(config) = sioul_core::config::Config::load(&sioul_core::config::default_path()) else { return true };
    let others: Vec<PathBuf> = config.accounts.iter().filter(|a| a.kind == sioul_core::config::AccountKind::Imap && a.id != account.id).map(|a| a.maildir_path()).collect();
    shared(&account.maildir_path(), &others)
}

/// The numbering a folder left, as `fetch_folder` meets it: the one the sync
/// state held (`previous`) when the server now gives another; none otherwise.
pub fn left_numbering(previous: Option<u32>, validity: u32) -> Option<u32> {
    previous.filter(|old| *old != validity)
}

/// The numberings this device saw each folder leave, by the folder's server
/// name: their unmatched old copies may go (`plan`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Left {
    #[serde(default)]
    folders: BTreeMap<String, BTreeSet<u32>>,
}

impl Left {
    /// The record; empty without one, or when it does not read.
    pub fn load(path: &Path) -> Left {
        std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    /// Written whole beside, then put in place.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = format!("# Sioul: the numberings (UIDVALIDITY) each folder left on this device, for removing their old copies.\n{}", toml::to_string(self).unwrap_or_default());
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, path)
    }

    /// `folder` left the numbering `validity`; whether that is new here.
    pub fn note(&mut self, folder: &str, validity: u32) -> bool {
        self.folders.entry(folder.to_string()).or_default().insert(validity)
    }

    /// The numberings `folder` left.
    pub fn of(&self, folder: &str) -> BTreeSet<u32> {
        self.folders.get(folder).cloned().unwrap_or_default()
    }

    /// Forgets the numberings of `folder` of which its directory `root`
    /// holds no file any more; whether anything was forgotten.
    pub fn forget_gone(&mut self, folder: &str, root: &Path) -> bool {
        let Some(left) = self.folders.get_mut(folder) else { return false };
        let here: BTreeSet<u32> = numbered(root).into_iter().map(|(_, origin)| origin.validity).collect();
        let before = left.len();
        left.retain(|validity| here.contains(validity));
        let changed = left.len() != before;
        if left.is_empty() {
            self.folders.remove(folder);
        }
        changed
    }
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

    /// The numberings noted as left, as `Left` keeps them.
    fn left(numberings: &[u32]) -> BTreeSet<u32> {
        numberings.iter().copied().collect()
    }

    /// An old copy goes once the new numbering holds the same Message-ID, its
    /// line logged without a subject or an address; the new copy stays.
    #[test]
    fn an_old_copy_goes_once_the_new_numbering_holds_its_id() {
        let folder = Folder::new("matched");
        let old = folder.put(1, 5, &message("a@example.org", "Hello"));
        let new = folder.put(2, 9, &message("a@example.org", "Hello"));
        let plan = plan(&folder.root, 2, Some((2, false)), &left(&[1]), false);
        assert_eq!(plan.paired, [Removal { path: old.clone(), old: origin(1, 5), new: Some(origin(2, 9)) }]);
        let log = folder.root.join("removed.log");
        assert_eq!(remove(&plan.paired, "INBOX", &log), 1);
        assert!(!old.exists() && new.exists());
        let logged = std::fs::read_to_string(&log).unwrap();
        assert!(logged.ends_with("\tINBOX\tU1-5\tU2-9\n"), "{logged}");
        assert!(!logged.contains("Hello") && !logged.contains("example.org"), "no subject, no address: {logged}");
    }

    /// An old copy of a numbering noted as left, which the new numbering does
    /// not hold, stays while the folder is not fetched whole, and may go once
    /// it is; the record forgets that numbering once its last file is gone.
    #[test]
    fn a_recorded_numbering_loses_its_unmatched_copies_once_whole() {
        let folder = Folder::new("unmatched");
        let old = folder.put(1, 6, &message("b@example.org", "Older"));
        folder.put(2, 1, &message("c@example.org", "Newer"));
        assert_eq!(plan(&folder.root, 2, Some((2, false)), &left(&[1]), false), Plan::default(), "not whole: kept");
        let whole = plan(&folder.root, 2, Some((2, true)), &left(&[1]), false);
        assert_eq!(whole.unmatched, [Removal { path: old.clone(), old: origin(1, 6), new: None }]);
        let mut record = Left::default();
        assert!(record.note("INBOX", 1));
        assert!(!record.forget_gone("INBOX", &folder.root), "a file of it still here: kept");
        assert_eq!(remove(&confirmed(whole.unmatched, [1], &left(&[1])), "INBOX", &folder.root.join("removed.log")), 1);
        assert!(!old.exists());
        assert!(record.forget_gone("INBOX", &folder.root) && record.of("INBOX").is_empty(), "its last file gone: forgotten");
        let path = folder.root.join("renumbered.toml");
        record.note("Archive", 4);
        record.save(&path).unwrap();
        assert_eq!(Left::load(&path), record);
    }

    /// An old numbering not noted here (left before this build, or seen first
    /// by an older Sioul), the folder whole: its paired copies go, the others stay.
    #[test]
    fn an_unrecorded_numbering_loses_its_paired_copies_alone() {
        let folder = Folder::new("unrecorded");
        let paired = folder.put(1, 5, &message("a@example.org", "Hello"));
        folder.put(2, 9, &message("a@example.org", "Hello"));
        folder.put(1, 6, &message("b@example.org", "Older"));
        let plan = plan(&folder.root, 2, Some((2, true)), &left(&[]), false);
        assert_eq!(plan.paired, [Removal { path: paired, old: origin(1, 5), new: Some(origin(2, 9)) }]);
        assert!(plan.unmatched.is_empty(), "never noted as left: what nothing pairs with stays");
    }

    /// Whole by the sync state, but a UID the server lists is not here under
    /// the new numbering (a message it did not serve): the unmatched copies
    /// stay; every UID here, they go.
    #[test]
    fn unmatched_copies_wait_for_every_uid_of_the_server() {
        let folder = Folder::new("server");
        let old = folder.put(1, 6, &message("b@example.org", "Older"));
        folder.put(2, 9, &message("c@example.org", "Newer"));
        let plan = plan(&folder.root, 2, Some((2, true)), &left(&[1]), false);
        let here = left(&[9]);
        assert!(confirmed(plan.unmatched.clone(), [9, 10], &here).is_empty(), "UID 10 is not here");
        assert_eq!(confirmed(plan.unmatched, [9], &here), [Removal { path: old, old: origin(1, 6), new: None }]);
    }

    /// Two addresses on one directory, or one inside another's: nothing goes
    /// for either, each folder whole, however the paths are written.
    #[test]
    fn a_directory_another_address_covers_loses_nothing() {
        let folder = Folder::new("shared");
        folder.put(1, 5, &message("a@example.org", "Hello"));
        folder.put(2, 9, &message("a@example.org", "Hello"));
        folder.put(1, 6, &message("b@example.org", "Older"));
        let same = folder.root.join("cur").join("..");
        assert!(shared(&folder.root, &[same]), "the same directory, written otherwise");
        assert!(shared(&folder.root, &[folder.root.join(".Sub")]), "another address inside this one's");
        assert!(shared(&folder.root.join(".Sub"), &[folder.root.clone()]), "this one inside another's");
        assert!(!shared(&folder.root, &[folder.root.with_file_name("sioul-renumbered-elsewhere")]), "beside it: not shared");
        for validity in [2, 1] {
            assert_eq!(plan(&folder.root, validity, Some((validity, true)), &left(&[1, 2]), true), Plan::default(), "numbering {validity}");
        }
    }

    /// The numbering a folder leaves is noted when the server gives another:
    /// the one the sync state held; none for a folder seen first, or the same.
    #[test]
    fn the_numbering_left_is_noted_when_it_changes() {
        assert_eq!(left_numbering(Some(1), 2), Some(1));
        assert_eq!(left_numbering(Some(2), 2), None);
        assert_eq!(left_numbering(None, 2), None);
    }

    /// A message without a Message-ID is matched by its digest.
    #[test]
    fn a_message_without_an_id_is_matched_by_its_digest() {
        let folder = Folder::new("digest");
        let lines = "From: Shop <no-reply@example.org>\r\nTo: me@example.org\r\nDate: Fri, 30 Oct 2026 08:00:00 +0100\r\nSubject: Your order";
        let old = folder.put(1, 7, lines);
        let new = folder.put(2, 10, lines);
        assert!(key_of(&old).unwrap().starts_with("digest:"));
        assert_eq!(plan(&folder.root, 2, Some((2, false)), &left(&[1]), false).paired, [Removal { path: old, old: origin(1, 7), new: Some(origin(2, 10)) }]);
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
        assert_eq!(plan(&folder.root, 2, Some((2, false)), &left(&[1]), false), Plan::default());
    }

    /// The new numbering not recorded yet (its first window not fetched, or
    /// stopped before the state said it): nothing removed, matches or not.
    #[test]
    fn nothing_goes_while_the_new_numbering_is_incomplete() {
        let folder = Folder::new("incomplete");
        let old = folder.put(1, 5, &message("a@example.org", "Hello"));
        folder.put(2, 9, &message("a@example.org", "Hello"));
        folder.put(1, 6, &message("b@example.org", "Older"));
        assert_eq!(plan(&folder.root, 2, None, &left(&[1]), false), Plan::default(), "nothing recorded");
        assert_eq!(plan(&folder.root, 2, Some((1, true)), &left(&[1]), false), Plan::default(), "the old numbering still recorded");
        assert!(old.exists());
    }

    /// Two old copies of one Message-ID and one new copy: one pairs, the other
    /// waits until the folder is whole; a file whose headers do not read never goes.
    #[test]
    fn copies_pair_one_to_one_and_unreadable_ones_stay() {
        let folder = Folder::new("pairs");
        let first = folder.put(1, 5, &message("a@example.org", "Hello"));
        let second = folder.put(1, 6, &message("a@example.org", "Hello"));
        folder.put(2, 9, &message("a@example.org", "Hello"));
        let unreadable = folder.root.join("cur").join(format!("1777000000.U1-7.sioul{}2,S", maildir::INFO));
        std::fs::write(&unreadable, "no headers at all").unwrap();
        assert_eq!(plan(&folder.root, 2, Some((2, false)), &left(&[1]), false), Plan { paired: vec![Removal { path: first, old: origin(1, 5), new: Some(origin(2, 9)) }], unmatched: Vec::new() });
        let whole = plan(&folder.root, 2, Some((2, true)), &left(&[1]), false);
        assert_eq!(whole.unmatched.iter().map(|r| r.path.clone()).collect::<Vec<_>>(), [second]);
        assert!(!whole.paired.iter().chain(&whole.unmatched).any(|r| r.path == unreadable), "unreadable: kept");
    }

    /// Another folder's and another address's copies are never looked at: the
    /// plan reads its own folder alone.
    #[test]
    fn another_folder_is_never_touched() {
        let (inbox, archive) = (Folder::new("inbox"), Folder::new("archive"));
        let elsewhere = archive.put(1, 5, &message("a@example.org", "Hello"));
        inbox.put(2, 9, &message("a@example.org", "Hello"));
        inbox.put(1, 3, &message("e@example.org", "Mine"));
        assert_eq!(plan(&inbox.root, 2, Some((2, false)), &left(&[1]), false), Plan::default());
        assert_eq!(plan(&archive.root, 1, Some((1, true)), &left(&[]), false), Plan::default(), "the archive's own numbering: its copy is current");
        assert!(elsewhere.exists());
    }
}

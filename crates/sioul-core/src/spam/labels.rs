// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The label log: what you said of messages, spam or not, for the spam
//! filter to learn from. One JSON line per act, added at the end of
//! `$XDG_STATE_HOME/sioul/spam/labels.jsonl` and never rewritten: "Junk" and
//! "Not junk" (the Mail page, `sioul mail`), "Not spam" on mail set aside, a
//! sender blocked from one of their messages.
//!
//! A line names its message as its server does (the account, the folder,
//! its UIDVALIDITY and UID) and by its Message-ID; never by its text: no
//! subject, no address, no word of it.
//!
//! ```text
//! {"at":1791360000,"account":"home","folder":"INBOX","uidvalidity":1700000000,"uid":4521,"message_id":"abc@example.org","label":"ham","source":"not-spam"}
//! ```
//!
//! The newest line about a message wins, and the log wins over the folders
//! and keywords it is found in. The filter's own verdicts are never written here.

use crate::card::Card;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The log's file name, in the spam filter's state folder.
pub const FILE: &str = "labels.jsonl";

/// What you said a message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Label {
    Spam,
    Ham,
}

/// What you did to say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// "Junk": into the junk folder.
    Junk,
    /// "Not junk": out of the junk folder, into the inbox.
    NotJunk,
    /// "Not spam", on mail set aside as spam: it stays in the inbox, in its lane.
    NotSpam,
    /// Its sender blocked from it.
    Block,
}

impl Source {
    /// What the act says of the message.
    pub fn label(self) -> Label {
        match self {
            Source::Junk | Source::Block => Label::Spam,
            Source::NotJunk | Source::NotSpam => Label::Ham,
        }
    }
}

/// One act, one line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// When, as Unix seconds.
    pub at: i64,
    /// The account's id in the configuration.
    pub account: String,
    /// The folder's name on the server, where the message was when you said it (`INBOX`, `[Gmail]/Spam`).
    pub folder: String,
    pub uidvalidity: u32,
    pub uid: u32,
    /// Without its angle brackets; none when the message has none.
    #[serde(default)]
    pub message_id: Option<String>,
    pub label: Label,
    pub source: Source,
}

impl Entry {
    /// `source`, now, about the message stored at `file` (a name Sioul gave
    /// it, which holds its UIDVALIDITY and UID: `maildir::origin_of`) in
    /// `folder` of `account`; none for a file Sioul did not fetch.
    pub fn of_file(account: &str, folder: &str, file: &Path, source: Source) -> Option<Entry> {
        let origin = crate::maildir::origin_of(file)?;
        Some(Entry {
            at: jiff::Timestamp::now().as_second(),
            account: account.to_string(),
            folder: folder.to_string(),
            uidvalidity: origin.validity,
            uid: origin.uid,
            message_id: message_id_of(file),
            label: source.label(),
            source,
        })
    }
}

/// A stored message's Message-ID, bare, read from its headers alone (the
/// first 256 KB of the file at most), as its card reads it.
fn message_id_of(file: &Path) -> Option<String> {
    use std::io::Read;
    let mut raw = Vec::new();
    std::fs::File::open(file).ok()?.take(256 * 1024).read_to_end(&mut raw).ok()?;
    let id = crate::mailindex::bare_id(mail_parser::MessageParser::default().parse_headers(&raw[..])?.message_id()?);
    (!id.is_empty()).then_some(id)
}

/// This device's log.
pub fn path() -> PathBuf {
    crate::config::state_dir().join("spam").join(FILE)
}

/// Adds `entry` at the end of this device's log.
pub fn append(entry: &Entry) -> Result<(), String> {
    append_to(&path(), entry)
}

/// Adds `entry` at the end of the log at `path`: one line written whole,
/// under the file's lock (`filelock`), so two writers never mix their lines.
/// The file is made yours alone, in a folder of yours alone (Unix).
pub fn append_to(path: &Path, entry: &Entry) -> Result<(), String> {
    use std::io::Write;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let line = serde_json::to_string(entry).map_err(|e| e.to_string())? + "\n";
    if let Some(folder) = path.parent() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(folder).map_err(fail)?;
    }
    crate::filelock::with_lock(path, || {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(path).and_then(|mut file| file.write_all(line.as_bytes())).map_err(fail)
    })
}

/// Every entry of this device's log, oldest first.
pub fn read() -> Vec<Entry> {
    read_from(&path())
}

/// Every entry of the log at `path`, oldest first (the order they were
/// written in); a line that cannot be read, cut by a crash or written by a
/// later Sioul, is left out.
pub fn read_from(path: &Path) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

/// The newest entry about each message, by its Message-ID: the copies of
/// one message in several folders or accounts are one.
pub fn newest_by_message_id(entries: &[Entry]) -> HashMap<&str, &Entry> {
    let mut newest = HashMap::new();
    for entry in entries {
        if let Some(id) = entry.message_id.as_deref() {
            newest.insert(id, entry);
        }
    }
    newest
}

/// The newest entry about one message, as its server names it.
pub fn newest_for<'a>(entries: &'a [Entry], account: &str, folder: &str, uidvalidity: u32, uid: u32) -> Option<&'a Entry> {
    entries.iter().rev().find(|e| e.uid == uid && e.uidvalidity == uidvalidity && e.account == account && e.folder == folder)
}

/// Whether you said the message on `card` is not spam, so that the Porch
/// takes it out of Set aside and keeps it out: the `$NotJunk` keyword
/// without `$Junk` in its file name (said here, on another device or in
/// another mail client, and kept by each sync: `maildir::keywords_of`); else,
/// where the server keeps no keywords, this device's log, whose newest entry
/// about this very message says so.
pub fn said_ham(card: &Card) -> bool {
    let keywords = card.path.as_deref().map(crate::maildir::keywords_of).unwrap_or_default();
    if keywords.not_junk && !keywords.junk {
        return true;
    }
    // A card from no account's server (a test, a file opened) has no line in the log: it is not read.
    if card.account.is_none() || card.origin.is_none() {
        return false;
    }
    kept(|entries| said_ham_in(entries, card))
}

/// Whether the newest entry of `entries` about the message on `card` says
/// it is not spam: the same account, UIDVALIDITY and UID, and the same
/// Message-ID (a card does not know its folder's server name; two folders'
/// messages that share a UIDVALIDITY and a UID do not share a Message-ID).
pub fn said_ham_in(entries: &[Entry], card: &Card) -> bool {
    let (Some(account), Some(origin)) = (card.account.as_deref(), card.origin) else { return false };
    let id = card.message_id.as_deref().map(crate::mailindex::bare_id).filter(|id| !id.is_empty());
    entries
        .iter()
        .rev()
        .find(|e| e.uid == origin.uid && e.uidvalidity == origin.validity && e.account == account && e.message_id == id)
        .is_some_and(|e| e.label == Label::Ham)
}

/// `f` on this device's log, read again only when its file changed (its
/// size or its time): the Porch asks about each message it sorts.
fn kept<T>(f: impl FnOnce(&[Entry]) -> T) -> T {
    type Seal = (PathBuf, u64, Option<std::time::SystemTime>);
    static KEPT: std::sync::Mutex<Option<(Seal, Vec<Entry>)>> = std::sync::Mutex::new(None);
    let path = path();
    let seal = std::fs::metadata(&path).map_or((path.clone(), 0, None), |m| (path.clone(), m.len(), m.modified().ok()));
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if kept.as_ref().is_none_or(|(known, _)| *known != seal) {
        *kept = Some((seal, read_from(&path)));
    }
    f(kept.as_ref().map_or(&[][..], |(_, entries)| entries.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::ImapOrigin;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-labels-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(uid: u32, id: Option<&str>, source: Source) -> Entry {
        Entry { at: 1_791_360_000 + i64::from(uid), account: "home".into(), folder: "INBOX".into(), uidvalidity: 7, uid, message_id: id.map(str::to_string), label: source.label(), source }
    }

    /// The log's lines, as the spam filter's trainer reads them: the format is a promise.
    #[test]
    fn lines_are_written_and_read_back() {
        let dir = scratch("lines");
        let path = dir.join("spam").join(FILE);
        let said = entry(4521, Some("abc@example.org"), Source::NotSpam);
        append_to(&path, &said).unwrap();
        append_to(&path, &entry(4522, None, Source::Junk)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text.lines().next().unwrap(),
            r#"{"at":1791364521,"account":"home","folder":"INBOX","uidvalidity":7,"uid":4521,"message_id":"abc@example.org","label":"ham","source":"not-spam"}"#
        );
        assert!(text.lines().nth(1).unwrap().contains(r#""message_id":null,"label":"spam","source":"junk""#));
        // A line cut by a crash, or from a later Sioul, is left out; the others stay.
        std::fs::write(&path, format!("{text}{{\"at\":1,\"acc\n")).unwrap();
        let read = read_from(&path);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], said);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600, "yours alone");
        }
        assert!(read_from(&dir.join("none.jsonl")).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A message as Sioul stores it: its server's numbers from its file name, its Message-ID from its headers.
    #[test]
    fn an_entry_names_its_message() {
        let dir = scratch("entry");
        let file = dir.join(format!("1759400000.U7-42.sioul{}2,S", crate::maildir::INFO));
        std::fs::write(&file, "From: Prize <win@lottery.test>\r\nMessage-ID: <win-1@lottery.test>\r\nSubject: You won\r\n\r\nClaim it.\r\n").unwrap();
        let said = Entry::of_file("home", "INBOX", &file, Source::Junk).unwrap();
        assert_eq!((said.uidvalidity, said.uid, said.message_id.as_deref(), said.label), (7, 42, Some("win-1@lottery.test"), Label::Spam));
        assert_eq!((said.account.as_str(), said.folder.as_str(), said.source), ("home", "INBOX", Source::Junk));
        // A file Sioul did not fetch has no place on a server.
        let loose = dir.join("letter.eml");
        std::fs::write(&loose, "Subject: x\r\n\r\nx\r\n").unwrap();
        assert!(Entry::of_file("home", "INBOX", &loose, Source::Junk).is_none());
        // No Message-ID: none written.
        let bare = dir.join(format!("1759400000.U7-43.sioul{}2,S", crate::maildir::INFO));
        std::fs::write(&bare, "Subject: x\r\n\r\nx\r\n").unwrap();
        assert_eq!(Entry::of_file("home", "INBOX", &bare, Source::Block).unwrap().message_id, None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_newest_word_wins() {
        let entries = vec![entry(1, Some("a@example.org"), Source::Junk), entry(2, Some("b@example.org"), Source::Junk), entry(1, Some("a@example.org"), Source::NotJunk)];
        let newest = newest_by_message_id(&entries);
        assert_eq!(newest["a@example.org"].label, Label::Ham);
        assert_eq!(newest["b@example.org"].label, Label::Spam);
        assert_eq!(newest_for(&entries, "home", "INBOX", 7, 1).map(|e| e.source), Some(Source::NotJunk));
        assert!(newest_for(&entries, "home", "Junk", 7, 1).is_none());
        assert!(newest_for(&entries, "work", "INBOX", 7, 1).is_none());
        assert_eq!((Source::Block.label(), Source::NotSpam.label()), (Label::Spam, Label::Ham));
    }

    #[test]
    fn a_message_said_not_spam_is_known_again() {
        let card = |path: &str, uid: u32| {
            let mut card = Card::from_bytes(b"From: Prize <win@lottery.test>\r\nSubject: You won\r\nMessage-ID: <win-1@lottery.test>\r\n\r\nClaim it.\r\n").unwrap();
            card.path = Some(PathBuf::from(path));
            card.account = Some("home".into());
            card.origin = Some(ImapOrigin { validity: 7, uid });
            card
        };
        // By its keyword: `$NotJunk` alone says it (the log is not even read then).
        let said = card(&format!("/mail/home/cur/1759400000.U7-1.sioul{}2,S{}", crate::maildir::INFO, crate::maildir::NOT_JUNK), 1);
        assert!(said_ham(&said));
        // With `$Junk` beside it, the keywords say nothing either way.
        let both = crate::maildir::keywords_of(Path::new(&format!("/mail/home/cur/1759400000.U7-1.sioul{}2,{}{}", crate::maildir::INFO, crate::maildir::JUNK, crate::maildir::NOT_JUNK)));
        assert!(both.junk && both.not_junk);
        // By the log: the newest entry about this very message, its Message-ID the same.
        let plain = card("/mail/home/new/1759400000.U7-1.sioul", 1);
        let log = vec![entry(1, Some("win-1@lottery.test"), Source::NotSpam)];
        assert!(said_ham_in(&log, &plain));
        assert!(!said_ham_in(&log, &card("/mail/home/new/1759400000.U7-2.sioul", 2)), "another UID");
        let borrowed = vec![entry(1, Some("other@lottery.test"), Source::NotSpam)];
        assert!(!said_ham_in(&borrowed, &plain), "another message under the same UID elsewhere");
        let then_junked = vec![entry(1, Some("win-1@lottery.test"), Source::NotSpam), entry(1, Some("win-1@lottery.test"), Source::Junk)];
        assert!(!said_ham_in(&then_junked, &plain));
        // A card from no account is never looked up.
        let mut loose = plain.clone();
        loose.account = None;
        assert!(!said_ham_in(&log, &loose));
    }
}

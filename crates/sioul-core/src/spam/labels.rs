// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The label log: what you said of messages, spam or not, for the spam
//! filter to learn from. One JSON line per act, added at the end of a file
//! and never rewritten: "Junk" and "Not junk" (the Mail page, `sioul mail`),
//! "Spam" and "Not spam" (the Porch's review queue, the Reader), a sender
//! blocked from one of their messages.
//!
//! Each device writes its own log, `$XDG_STATE_HOME/sioul/spam/labels/<device>.jsonl`
//! (`own_log`: the device's name in the sharing, a UUID made once), and
//! reads every device's: the logs travel sealed, with the part "Spam
//! filter" (sioul-sync's `share`), so that a message you said is not spam on
//! one device is never flagged again on another, and the training, on your
//! computer, learns from what you said on your phone. Before the logs were
//! shared, each device kept one, `labels.jsonl`: still read, and moved into
//! this device's own the first time it writes.
//!
//! A line names its message as its server does (the account, the folder,
//! its UIDVALIDITY and UID) and by its Message-ID; never by its text: no
//! subject, no address, no word of it.
//!
//! ```text
//! {"at":1791360000,"account":"home","folder":"INBOX","uidvalidity":1700000000,"uid":4521,"message_id":"abc@example.org","label":"ham","source":"not-spam"}
//! ```
//!
//! The newest line about a message, every device's together, wins, and the
//! log wins over everything else: the folders and keywords the message is
//! found in, and the filter's own verdicts. Those are never written here but
//! kept apart, in the same way, one file per device: what it moved into a
//! Junk folder (the matrix's "Move to spam") under `moved/` (`Moved`), what
//! it flagged where it is (the matrix's "Flag only") under `flagged/`
//! (`Flagged`). The Porch lists them among the filter's catches; the
//! training learns from them as they are until you say otherwise: what was
//! moved, as spam, as all mail in a Junk folder; what was flagged as
//! probably spam, as spam; what was flagged as maybe spam, not at all, a
//! doubt being no label, until you say.

use crate::card::Card;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The older single log's file name, in the spam filter's state folder.
pub const FILE: &str = "labels.jsonl";
/// The folder of the label logs, one file per device.
pub const FOLDER: &str = "labels";
/// The folder of the logs of what your own filter moved, one file per device.
pub const MOVED: &str = "moved";
/// The folder of the logs of what your own filter flagged where it is, one file per device.
pub const FLAGGED: &str = "flagged";

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

/// The spam filter's state folder on this device: `$XDG_STATE_HOME/sioul/spam`.
pub fn state() -> PathBuf {
    crate::config::state_dir().join("spam")
}

/// This device's own label log under `root` (the spam filter's state
/// folder), `labels/<device>.jsonl`; `device` is its name in the sharing (a
/// UUID made once: sioul-sync's `share::Here`). The older single log, this
/// device's own, becomes it the first time.
pub fn own_log(root: &Path, device: &str) -> PathBuf {
    let own = root.join(FOLDER).join(format!("{}.jsonl", file_safe(device)));
    let older = root.join(FILE);
    if !own.exists() && older.exists() && std::fs::create_dir_all(root.join(FOLDER)).is_ok() {
        let _ = std::fs::rename(&older, &own);
    }
    own
}

/// This device's own log of what your filter moved under `root`, `moved/<device>.jsonl`.
pub fn own_moved_log(root: &Path, device: &str) -> PathBuf {
    root.join(MOVED).join(format!("{}.jsonl", file_safe(device)))
}

/// This device's own log of what your filter flagged where it is under `root`, `flagged/<device>.jsonl`.
pub fn own_flagged_log(root: &Path, device: &str) -> PathBuf {
    root.join(FLAGGED).join(format!("{}.jsonl", file_safe(device)))
}

/// A device's name as a file name: letters, digits, `-` and `_`.
fn file_safe(device: &str) -> String {
    let safe: String = device.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if safe.is_empty() { "this-device".to_string() } else { safe }
}

/// Adds `line` at the end of the log at `path`: one line written whole,
/// under the file's lock (`filelock`), so two writers never mix their lines.
/// The file is made yours alone, in a folder of yours alone (Unix).
pub fn append_to<T: Serialize>(path: &Path, line: &T) -> Result<(), String> {
    use std::io::Write;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let line = serde_json::to_string(line).map_err(|e| e.to_string())? + "\n";
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

/// Every line of the log at `path`, in the order they were written; a line
/// that cannot be read, cut by a crash or written by a later Sioul, is left out.
pub fn read_from<T: serde::de::DeserializeOwned>(path: &Path) -> Vec<T> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

/// The logs in a folder, every device's (`*.jsonl`, by name), and `older` before them when it is there.
fn logs_in(folder: &Path, older: Option<&Path>) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(folder).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "jsonl") && p.is_file()).collect();
    files.sort();
    older.filter(|p| p.is_file()).map(Path::to_path_buf).into_iter().chain(files).collect()
}

/// Every entry of every device's label log under `root` (the spam filter's
/// state folder), the older single log's too, oldest first: by when you
/// said it, then in the order written.
pub fn read_all_in(root: &Path) -> Vec<Entry> {
    let mut entries: Vec<Entry> = logs_in(&root.join(FOLDER), Some(&root.join(FILE))).iter().flat_map(|p| read_from::<Entry>(p)).collect();
    entries.sort_by_key(|e| e.at);
    entries
}

/// Every device's label log, here: `read_all_in` of this device's state folder.
pub fn read_all() -> Vec<Entry> {
    read_all_in(&state())
}

/// The newest entry about each message, by its Message-ID: the copies of
/// one message in several folders or accounts are one. `entries` oldest first.
pub fn newest_by_message_id(entries: &[Entry]) -> HashMap<&str, &Entry> {
    let mut newest = HashMap::new();
    for entry in entries {
        if let Some(id) = entry.message_id.as_deref() {
            newest.insert(id, entry);
        }
    }
    newest
}

/// The newest entry about one message, as its server names it. `entries` oldest first.
pub fn newest_for<'a>(entries: &'a [Entry], account: &str, folder: &str, uidvalidity: u32, uid: u32) -> Option<&'a Entry> {
    entries.iter().rev().find(|e| e.uid == uid && e.uidvalidity == uidvalidity && e.account == account && e.folder == folder)
}

/// Whether you said the message on `card` is not spam, here, on another
/// device or in another mail client, so that the Porch keeps your filter's
/// word and your provider's off it for good: the `$NotJunk` keyword without
/// `$Junk` in its file name (kept by each sync: `maildir::keywords_of`);
/// else every device's label log, whose newest entry about this message says so.
pub fn said_ham(card: &Card) -> bool {
    let keywords = card.path.as_deref().map(crate::maildir::keywords_of).unwrap_or_default();
    if keywords.not_junk && !keywords.junk {
        return true;
    }
    // A card from no account's server (a test, a file opened) has no line in the logs: they are not read.
    if card.account.is_none() {
        return false;
    }
    kept(|entries| said_ham_in(entries, card))
}

/// Whether the newest entry of `entries` (oldest first) about the message on
/// `card` says it is not spam: an entry of its account with its Message-ID,
/// wherever the message was then (the logs of other devices name it by the
/// folder it was in there); for a message without one, an entry without one
/// at its very place (its UIDVALIDITY and UID).
pub fn said_ham_in(entries: &[Entry], card: &Card) -> bool {
    let Some(account) = card.account.as_deref() else { return false };
    let id = card.message_id.as_deref().map(crate::mailindex::bare_id).filter(|id| !id.is_empty());
    entries
        .iter()
        .rev()
        .find(|e| {
            e.account == account
                && match (e.message_id.as_deref(), id.as_deref()) {
                    (Some(theirs), Some(ours)) => theirs == ours,
                    (None, None) => card.origin.is_some_and(|o| o.uid == e.uid && o.validity == e.uidvalidity),
                    _ => false,
                }
        })
        .is_some_and(|e| e.label == Label::Ham)
}

/// One message your own filter moved into its account's Junk folder (the
/// matrix's "Move to spam"), from where it was; in the log of what it
/// flagged (`Flagged`), one it flagged where it is. No word of yours: the
/// Porch lists it among the filter's catches while it is there, and the
/// training learns from it as the module says, until you say otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moved {
    /// When, as Unix seconds.
    pub at: i64,
    pub account: String,
    /// The folder it was in when your filter judged it, on the server, and
    /// its place there: moved from it, or flagged in it.
    pub folder: String,
    pub uidvalidity: u32,
    pub uid: u32,
    /// Without its angle brackets; none when the message has none (then it
    /// cannot be found again in the Junk folder, where it has another UID).
    #[serde(default)]
    pub message_id: Option<String>,
    /// How the filter judged it: probably spam, maybe spam, probably not spam.
    pub class: super::Class,
}

impl Moved {
    /// Your filter moving the message stored at `file` in `folder` of
    /// `account`, now, or flagging it there (`Flagged`); none for a file
    /// Sioul did not fetch.
    pub fn of_file(account: &str, folder: &str, file: &Path, class: super::Class) -> Option<Moved> {
        let origin = crate::maildir::origin_of(file)?;
        Some(Moved { at: jiff::Timestamp::now().as_second(), account: account.to_string(), folder: folder.to_string(), uidvalidity: origin.validity, uid: origin.uid, message_id: message_id_of(file), class })
    }
}

/// One message your own filter flagged where it is (the matrix's "Flag
/// only") as it arrived: the same line as a move, in its own log
/// (`flagged/`). The training learns a probable spam flagged so as spam, and
/// leaves a maybe spam out, until you say.
pub type Flagged = Moved;

/// Every device's log of what your filter moved, under `root`, oldest first.
pub fn read_moved_in(root: &Path) -> Vec<Moved> {
    read_judged(&root.join(MOVED))
}

/// Every device's log of what your filter flagged where it is, under `root`, oldest first.
pub fn read_flagged_in(root: &Path) -> Vec<Flagged> {
    read_judged(&root.join(FLAGGED))
}

/// Every line of the logs of `folder` (`moved/`, `flagged/`), oldest first.
fn read_judged(folder: &Path) -> Vec<Moved> {
    let mut lines: Vec<Moved> = logs_in(folder, None).iter().flat_map(|p| read_from::<Moved>(p)).collect();
    lines.sort_by_key(|m| m.at);
    lines
}

/// Whether a message your filter moved or flagged is still as it judged
/// it: no entry of the label logs (`entries`, oldest first) about it since.
pub fn unreviewed(moved: &Moved, entries: &[Entry]) -> bool {
    let Some(id) = moved.message_id.as_deref() else { return false };
    !entries.iter().any(|e| e.at >= moved.at && e.account == moved.account && e.message_id.as_deref() == Some(id))
}

/// How long the logs' folder listing is trusted before it is looked at again:
/// the Porch asks about each message it sorts.
const LOOKED: std::time::Duration = std::time::Duration::from_secs(2);

/// What the logs' files were when read: their names, sizes and times.
type Seal = Vec<(PathBuf, u64, Option<std::time::SystemTime>)>;

fn seal_of(files: &[PathBuf]) -> Seal {
    files.iter().map(|p| std::fs::metadata(p).map_or((p.clone(), 0, None), |m| (p.clone(), m.len(), m.modified().ok()))).collect()
}

/// `f` on every device's label log, read again when one of the files
/// changed (its size or its time) or came: the folder is looked at again
/// after two seconds at most.
fn kept<T>(f: impl FnOnce(&[Entry]) -> T) -> T {
    type Kept = Option<(PathBuf, std::time::Instant, Seal, Vec<Entry>)>;
    static KEPT: std::sync::Mutex<Kept> = std::sync::Mutex::new(None);
    let root = state();
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    let fresh = kept.as_ref().is_some_and(|(at, looked, _, _)| *at == root && looked.elapsed() < LOOKED);
    if !fresh {
        let seal = seal_of(&logs_in(&root.join(FOLDER), Some(&root.join(FILE))));
        match kept.as_mut() {
            Some((at, looked, known, _)) if *at == root && *known == seal => *looked = std::time::Instant::now(),
            _ => *kept = Some((root.clone(), std::time::Instant::now(), seal, read_all_in(&root))),
        }
    }
    f(kept.as_ref().map_or(&[][..], |(_, _, _, entries)| entries.as_slice()))
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
        let read = read_from::<Entry>(&path);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], said);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600, "yours alone");
        }
        assert!(read_from::<Entry>(&dir.join("none.jsonl")).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Each device writes its own log; every device's is read, oldest first,
    /// the older single log too, which becomes this device's own when it first writes.
    #[test]
    fn every_devices_log_is_read() {
        let root = scratch("devices").join("spam");
        append_to(&root.join(FILE), &entry(1, Some("old@example.org"), Source::Junk)).unwrap();
        assert_eq!(read_all_in(&root).len(), 1, "the older log, before this device writes");
        let desk = own_log(&root, "desk-uuid");
        assert_eq!(desk, root.join(FOLDER).join("desk-uuid.jsonl"));
        assert!(!root.join(FILE).exists() && desk.exists(), "moved into this device's own");
        append_to(&desk, &Entry { at: 1_791_360_500, ..entry(5, Some("b@example.org"), Source::NotSpam) }).unwrap();
        // The phone's, as the sharing writes it here.
        let phone = root.join(FOLDER).join("phone-uuid.jsonl");
        append_to(&phone, &Entry { at: 1_791_360_100, ..entry(3, Some("c@example.org"), Source::Junk) }).unwrap();
        std::fs::write(root.join(FOLDER).join("notes.txt"), "not a log").unwrap();
        let all = read_all_in(&root);
        assert_eq!(all.iter().map(|e| e.at).collect::<Vec<_>>(), vec![1_791_360_001, 1_791_360_100, 1_791_360_500], "oldest first, every device's");
        assert_eq!(own_log(&root, "a/b"), root.join(FOLDER).join("a_b.jsonl"), "a name safe as a file's");
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    /// What your filter moved or flagged is no label: kept apart, each in
    /// its own log, read from every device, as it judged it until a label
    /// about it comes.
    #[test]
    fn what_the_filter_did_is_kept_apart() {
        let root = scratch("moved").join("spam");
        let file = root.join(format!("1759400000.U7-42.sioul{}2,S", crate::maildir::INFO));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&file, "From: Prize <win@lottery.test>\r\nMessage-ID: <win-1@lottery.test>\r\nSubject: You won\r\n\r\nClaim it.\r\n").unwrap();
        let moved = Moved { at: 1_791_360_000, ..Moved::of_file("home", "INBOX", &file, super::super::Class::Spam).unwrap() };
        assert_eq!((moved.uid, moved.message_id.as_deref(), moved.class), (42, Some("win-1@lottery.test"), super::super::Class::Spam));
        append_to(&own_moved_log(&root, "phone"), &moved).unwrap();
        let line = std::fs::read_to_string(own_moved_log(&root, "phone")).unwrap();
        assert!(line.contains(r#""class":"spam""#) && !line.contains("label"), "{line}");
        assert_eq!(read_moved_in(&root), vec![moved.clone()]);
        assert!(read_all_in(&root).is_empty(), "never a label");
        // Flagged where it is, on another device: its own log, apart from the moves.
        let flagged: Flagged = Moved { at: 1_791_360_050, uid: 43, class: super::super::Class::Unsure, ..moved.clone() };
        append_to(&own_flagged_log(&root, "desk"), &flagged).unwrap();
        assert_eq!(own_flagged_log(&root, "desk"), root.join(FLAGGED).join("desk.jsonl"));
        assert_eq!((read_flagged_in(&root), read_moved_in(&root)), (vec![flagged.clone()], vec![moved.clone()]));
        assert!(read_all_in(&root).is_empty(), "never a label either");
        // Unreviewed until a label about it, here or on another device, comes after the move.
        let before = Entry { at: 1_791_359_000, ..entry(42, Some("win-1@lottery.test"), Source::NotSpam) };
        assert!(unreviewed(&moved, &[before.clone()]));
        let after = Entry { at: 1_791_360_100, ..before.clone() };
        assert!(!unreviewed(&moved, &[before, after]));
        assert!(!unreviewed(&Moved { message_id: None, ..moved }, &[]), "without a Message-ID it cannot be found again");
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
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
        // The same message elsewhere (back in the inbox, or as another device's log names it): known by its Message-ID.
        assert!(said_ham_in(&log, &card("/mail/home/new/1759400000.U9-2.sioul", 2)), "the same message, another place");
        let borrowed = vec![entry(1, Some("other@lottery.test"), Source::NotSpam)];
        assert!(!said_ham_in(&borrowed, &plain), "another message under the same UID elsewhere");
        let mut elsewhere = log.clone();
        elsewhere[0].account = "work".into();
        assert!(!said_ham_in(&elsewhere, &plain), "another account's");
        // Without a Message-ID: its very place only.
        let mut nameless = plain.clone();
        nameless.message_id = None;
        assert!(said_ham_in(&[entry(1, None, Source::NotSpam)], &nameless));
        assert!(!said_ham_in(&[entry(2, None, Source::NotSpam)], &nameless));
        let then_junked = vec![entry(1, Some("win-1@lottery.test"), Source::NotSpam), entry(1, Some("win-1@lottery.test"), Source::Junk)];
        assert!(!said_ham_in(&then_junked, &plain));
        // A card from no account is never looked up.
        let mut loose = plain.clone();
        loose.account = None;
        assert!(!said_ham_in(&log, &loose));
    }
}

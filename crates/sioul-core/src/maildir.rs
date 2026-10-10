// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Messages on disk: a Maildir (`new/` and `cur/`), or a plain folder of `.eml` files.
//!
//! Sioul writes what it fetches as a Maildir (<https://cr.yp.to/proto/maildir.html>),
//! so notmuch, mutt or any other reader can open the same folders. Each file
//! name carries where the message sits on the server, `U<uidvalidity>-<uid>`,
//! so the Porch knows what you have seen without opening the files.
//!
//! After the flags, two IMAP keywords (RFC 9051 §2.3.2) are kept as Dovecot
//! keeps them: a lowercase letter each, named in the folder's
//! `dovecot-keywords` file. They say what you, or a mail client, said of a
//! message: junk (`$Junk`), or not junk (`$NotJunk`).

use crate::card::{Card, ImapOrigin};

/// `$Junk`'s letter.
pub const JUNK: char = 'a';
/// `$NotJunk`'s letter.
pub const NOT_JUNK: char = 'b';
/// The keywords kept, by letter, as the folder's `dovecot-keywords` file numbers them (a is 0, b is 1).
pub const KEYWORDS: [(char, &str); 2] = [(JUNK, "$Junk"), (NOT_JUNK, "$NotJunk")];

/// The letter of an IMAP keyword Sioul keeps; none for the others. Case
/// does not matter: `$junk` is `$Junk` to IMAP servers.
pub fn keyword_letter(keyword: &str) -> Option<char> {
    KEYWORDS.iter().find(|(_, name)| name.eq_ignore_ascii_case(keyword)).map(|(letter, _)| *letter)
}

/// What a stored message's file name says of it, junk or not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Keywords {
    /// `$Junk`.
    pub junk: bool,
    /// `$NotJunk`.
    pub not_junk: bool,
}

/// The keywords in a stored message's file name.
pub fn keywords_of(path: &Path) -> Keywords {
    let flags = flags_of(path);
    Keywords { junk: flags.contains(JUNK), not_junk: flags.contains(NOT_JUNK) }
}

/// The folder's `dovecot-keywords` file, which names the letters, written
/// once, when a letter is first kept there: Dovecot reads the same folder alike.
fn name_keywords(folder: &Path, flags: &str) {
    let file = folder.join("dovecot-keywords");
    if KEYWORDS.iter().any(|(letter, _)| flags.contains(*letter)) && !file.exists() {
        let names: String = KEYWORDS.iter().enumerate().map(|(n, (_, name))| format!("{n} {name}\n")).collect();
        let _ = std::fs::write(file, names);
    }
}

/// What separates a message's unique name from its flags: ":" by the Maildir
/// convention, "!" on Windows, where ":" cannot be in a file name (mbsync does
/// the same). Both are read on every system.
pub const INFO: char = if cfg!(windows) { '!' } else { ':' };

/// "1759400000.U17-4521.sioul:2,FS" → "1759400000.U17-4521.sioul".
pub fn unique_part(name: &str) -> &str {
    name.split([':', '!']).next().unwrap_or(name)
}

/// The flags after ":2," or "!2,".
fn flags_part(name: &str) -> Option<&str> {
    name.split_once(":2,").or_else(|| name.split_once("!2,")).map(|(_, flags)| flags)
}
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Every message of a Maildir or folder, oldest first.
pub fn read_messages(dir: &Path) -> Vec<Card> {
    read_messages_where(dir, |_| true)
}

/// The messages whose server origin passes `keep`, oldest first; the others are not even read.
pub fn read_messages_where(dir: &Path, keep: impl Fn(Option<ImapOrigin>) -> bool) -> Vec<Card> {
    let maildir = [dir.join("new"), dir.join("cur")];
    let folders: Vec<PathBuf> = if maildir.iter().any(|d| d.is_dir()) {
        maildir.into_iter().filter(|d| d.is_dir()).collect()
    } else {
        vec![dir.to_path_buf()]
    };
    let mut cards: Vec<Card> = folders.iter().flat_map(|f| read_folder(f, &keep)).collect();
    cards.sort_by_key(|c| c.date.unwrap_or(0));
    cards
}

fn read_folder(folder: &Path, keep: &impl Fn(Option<ImapOrigin>) -> bool) -> Vec<Card> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && !is_hidden(p) && keep(origin_of(p)))
        .filter_map(|p| read_one(&p))
        .collect()
}

/// One message file as a card. While a `CardsKept` lives in this thread, a
/// message already read in it is not parsed again: the file's name less its
/// flags, its size and its time say it is the same (a message's content never
/// changes; its flags move it between `new/` and `cur/`).
pub fn read_one(path: &Path) -> Option<Card> {
    let key = kept_key(path);
    if let Some(mut card) = key.as_ref().and_then(|key| KEPT.with(|kept| kept.borrow().as_ref().and_then(|kept| kept.get(key).cloned()))) {
        card.path = Some(path.to_path_buf());
        card.origin = origin_of(path);
        return Some(card);
    }
    let raw = std::fs::read(path).ok()?;
    #[cfg(test)]
    PARSED.with(|n| n.set(n.get() + 1));
    let mut card = Card::from_bytes(&raw)?;
    card.path = Some(path.to_path_buf());
    card.origin = origin_of(path);
    if let Some(key) = key {
        KEPT.with(|kept| {
            if let Some(kept) = kept.borrow_mut().as_mut() {
                kept.insert(key, card.clone());
            }
        });
    }
    Some(card)
}

type KeptCards = std::collections::HashMap<(String, u64, u64), Card>;

thread_local! {
    /// The cards read in this thread while a `CardsKept` lives.
    static KEPT: std::cell::RefCell<Option<KeptCards>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
thread_local! {
    /// Message files parsed in this test's thread.
    static PARSED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// A message file as `CardsKept` knows it: its name less its flags, its size, its time (nanoseconds).
fn kept_key(path: &Path) -> Option<(String, u64, u64)> {
    if KEPT.with(|kept| kept.borrow().is_none()) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos() as u64;
    Some((unique_part(path.file_name()?.to_str()?).to_string(), meta.len(), modified))
}

/// While it lives, each message file this thread reads is parsed once
/// (`read_one`): one look at the mail (the Porch, then the money lines it
/// holds, each going through every message kept) parses each message once,
/// not once each. Nothing is kept once it ends: a card holds a few kilobytes,
/// and every message kept would stay in memory between looks. One begun
/// inside another changes nothing.
pub struct CardsKept(bool);

impl CardsKept {
    pub fn begin() -> CardsKept {
        let outer = KEPT.with(|kept| {
            let mut kept = kept.borrow_mut();
            let outer = kept.is_some();
            if !outer {
                *kept = Some(KeptCards::new());
            }
            outer
        });
        CardsKept(outer)
    }
}

impl Drop for CardsKept {
    fn drop(&mut self) {
        if !self.0 {
            KEPT.with(|kept| *kept.borrow_mut() = None);
        }
    }
}

fn is_hidden(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.'))
}

/// `1759400000.U1700000000-4521.sioul:2,S` → validity 1700000000, UID 4521.
pub fn origin_of(path: &Path) -> Option<ImapOrigin> {
    let name = path.file_name()?.to_str()?;
    let unique = unique_part(name).split('.').find(|part| part.starts_with('U'))?;
    let (validity, uid) = unique[1..].split_once('-')?;
    Some(ImapOrigin { validity: validity.parse().ok()?, uid: uid.parse().ok()? })
}

/// A message fetched from the server, ready to be written.
pub struct Fetched<'a> {
    pub origin: ImapOrigin,
    /// The server's IMAP flags, as Maildir letters (`S` seen, `R` replied, `F` flagged…).
    pub flags: String,
    /// When the server received it (INTERNALDATE), as Unix seconds.
    pub received: Option<i64>,
    pub raw: &'a [u8],
}

/// Writes one message into a Maildir: first in `tmp/`, then moved, so a reader
/// never sees half a message. Mail without any flag goes to `new/`; mail with
/// some (read, flagged, a keyword kept) to `cur/` with its letters, as the
/// Maildir convention says, so that none is lost until the next sync.
pub fn store(root: &Path, message: &Fetched) -> std::io::Result<PathBuf> {
    for sub in ["tmp", "new", "cur"] {
        std::fs::create_dir_all(root.join(sub))?;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let unique = format!("{now}.U{}-{}.sioul", message.origin.validity, message.origin.uid);
    let temporary = root.join("tmp").join(&unique);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    // Mail is yours alone, as mail servers keep it: not readable by the other
    // accounts of a shared computer, whatever the umask.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(message.raw)?;
    file.sync_all()?;
    // The file's date is the server's arrival date, which tools sorting by date expect.
    if let Some(seconds) = message.received.and_then(|s| u64::try_from(s).ok()) {
        file.set_modified(UNIX_EPOCH + Duration::from_secs(seconds))?;
    }
    drop(file);
    let target = if message.flags.is_empty() { root.join("new").join(unique) } else { root.join("cur").join(format!("{unique}{INFO}2,{}", sorted(&message.flags))) };
    name_keywords(root, &message.flags);
    std::fs::rename(&temporary, &target)?;
    Ok(target)
}

/// The Maildir flags of a stored message, from its name, with the keywords'
/// letters: "FS", "Sb"; none in `new/`.
pub fn flags_of(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    flags_part(name).map_or_else(String::new, str::to_string)
}

/// Gives a stored message other flags: it is renamed in `cur/`, where every
/// message that has been seen or flagged lives. Returns where it is now.
pub fn set_flags(path: &Path, flags: &str) -> std::io::Result<PathBuf> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let unique = unique_part(name);
    let folder = path.parent().and_then(Path::parent).ok_or_else(|| std::io::Error::other("not in a Maildir"))?;
    let target = folder.join("cur").join(format!("{unique}{INFO}2,{}", sorted(flags)));
    if target != path {
        std::fs::create_dir_all(folder.join("cur"))?;
        name_keywords(folder, flags);
        std::fs::rename(path, &target)?;
    }
    Ok(target)
}

/// Where a stored message is now: its flags rename it, and a first reading
/// moves it from `new/` to `cur/`. The part of the name before ":2," never changes.
pub fn locate(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let name = path.file_name()?.to_str()?;
    let unique = unique_part(name);
    let folder = path.parent()?.parent()?;
    ["cur", "new"].iter().find_map(|sub| {
        std::fs::read_dir(folder.join(sub))
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| unique_part(n) == unique))
    })
}

/// Whether a Maildir holds a message not read yet: one in `new/`, or one in
/// `cur/` without the S flag. Only names are read.
pub fn has_unread(dir: &Path) -> bool {
    let names = |sub: &str| std::fs::read_dir(dir.join(sub)).into_iter().flatten().filter_map(Result::ok).filter_map(|e| e.file_name().into_string().ok());
    names("new").any(|n| !n.starts_with('.')) || names("cur").any(|n| !n.starts_with('.') && !flags_part(&n).is_some_and(|f| f.contains('S')))
}

/// The Message-IDs of a Maildir's messages, from their headers only.
pub fn message_ids(dir: &Path) -> std::collections::BTreeSet<String> {
    read_messages(dir).into_iter().filter_map(|card| card.message_id).collect()
}

/// Maildir wants the flag letters in ASCII order.
fn sorted(flags: &str) -> String {
    let mut letters: Vec<char> = flags.chars().collect();
    letters.sort_unstable();
    letters.dedup();
    letters.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_reads_back_with_its_origin() {
        let root = std::env::temp_dir().join(format!("sioul-maildir-{}", std::process::id()));
        let raw = b"From: Jane <jane@example.org>\r\nSubject: Hello\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\nHi.\r\n";
        let unread = Fetched { origin: ImapOrigin { validity: 7, uid: 41 }, flags: String::new(), received: Some(1_790_000_000), raw };
        let read = Fetched { origin: ImapOrigin { validity: 7, uid: 42 }, flags: "SF".into(), received: None, raw };
        let first = store(&root, &unread).unwrap();
        let second = store(&root, &read).unwrap();
        assert!(first.starts_with(root.join("new")));
        assert!(second.to_str().unwrap().ends_with(&format!("{INFO}2,FS")));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&first).unwrap().permissions().mode() & 0o777, 0o600, "yours alone");
        }
        let cards = read_messages(&root);
        assert_eq!(cards.len(), 2);
        assert!(cards.iter().any(|c| c.origin == Some(ImapOrigin { validity: 7, uid: 42 })));
        let newer = read_messages_where(&root, |o| o.is_some_and(|o| o.uid > 41));
        assert_eq!(newer.len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn flags_rename_the_file() {
        let root = std::env::temp_dir().join(format!("sioul-flags-{}", std::process::id()));
        let raw = b"Subject: x\r\n\r\nx\r\n";
        let unread = store(&root, &Fetched { origin: ImapOrigin { validity: 3, uid: 9 }, flags: String::new(), received: None, raw }).unwrap();
        assert_eq!(flags_of(&unread), "");
        let read = set_flags(&unread, "SF").unwrap();
        assert!(read.starts_with(root.join("cur")) && flags_of(&read) == "FS");
        assert_eq!(origin_of(&read), Some(ImapOrigin { validity: 3, uid: 9 }));
        let unflagged = set_flags(&read, "S").unwrap();
        assert_eq!(flags_of(&unflagged), "S");
        // The old name still finds the message.
        assert_eq!(locate(&unread), Some(unflagged.clone()));
        assert_eq!(locate(&read), Some(unflagged));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// `$Junk` and `$NotJunk` are kept in the file name, as Dovecot keeps
    /// keywords, from the first store: unread mail with one goes to `cur/`.
    #[test]
    fn junk_keywords_are_kept() {
        let root = std::env::temp_dir().join(format!("sioul-keywords-{}", std::process::id()));
        let raw = b"Subject: x\r\n\r\nx\r\n";
        assert_eq!((keyword_letter("$NotJunk"), keyword_letter("$junk"), keyword_letter("NonJunk")), (Some(NOT_JUNK), Some(JUNK), None));
        let plain = store(&root, &Fetched { origin: ImapOrigin { validity: 3, uid: 1 }, flags: String::new(), received: None, raw }).unwrap();
        assert!(plain.starts_with(root.join("new")) && !root.join("dovecot-keywords").exists());
        let said = store(&root, &Fetched { origin: ImapOrigin { validity: 3, uid: 2 }, flags: NOT_JUNK.to_string(), received: None, raw }).unwrap();
        assert!(said.starts_with(root.join("cur")), "unread, with its keyword: {}", said.display());
        assert_eq!(keywords_of(&said), Keywords { junk: false, not_junk: true });
        assert_eq!(std::fs::read_to_string(root.join("dovecot-keywords")).unwrap(), "0 $Junk\n1 $NotJunk\n");
        // Read, then said junk: the letters follow the flags, in ASCII order.
        let junked = set_flags(&said, &format!("S{JUNK}")).unwrap();
        assert_eq!(flags_of(&junked), format!("S{JUNK}"));
        assert_eq!(keywords_of(&junked), Keywords { junk: true, not_junk: false });
        assert_eq!(keywords_of(&plain), Keywords::default());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn origin_needs_the_sioul_pattern() {
        assert_eq!(origin_of(Path::new("cur/1759400000.U17-4521.sioul:2,S")), Some(ImapOrigin { validity: 17, uid: 4521 }));
        assert_eq!(origin_of(Path::new("01-bank.eml")), None);
        assert_eq!(origin_of(Path::new("cur/1759400000.M20P3.host:2,S")), None);
        // As Windows names them.
        assert_eq!(origin_of(Path::new("cur/1759400000.U17-4521.sioul!2,S")), Some(ImapOrigin { validity: 17, uid: 4521 }));
        assert_eq!(flags_of(Path::new("cur/1759400000.U17-4521.sioul!2,FS")), "FS");
    }

    /// Whether the phone parses each message once (the owner's question, 9
    /// October 2026): within one look at the mail (`CardsKept`), each message
    /// file is parsed once, however many passes read it, its flags moved
    /// meanwhile too; outside one, as before.
    #[test]
    fn each_message_is_parsed_once_in_a_look() {
        let dir = std::env::temp_dir().join(format!("sioul-cards-kept-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["new", "cur", "tmp"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        for n in 0..3 {
            std::fs::write(dir.join("new").join(format!("17594000{n}.U1700000000-{n}.sioul")), format!("From: a@example.org\r\nSubject: Hello {n}\r\nMessage-ID: <{n}@example.org>\r\n\r\nBody {n}\r\n")).unwrap();
        }
        let parsed = || PARSED.with(std::cell::Cell::get);
        let before = parsed();
        {
            let _kept = CardsKept::begin();
            assert_eq!(read_messages(&dir).len(), 3);
            // Read, its flags moved: the same message.
            let first = dir.join("new").join("175940000.U1700000000-0.sioul");
            std::fs::rename(&first, dir.join("cur").join("175940000.U1700000000-0.sioul:2,S")).unwrap();
            let again = read_messages(&dir);
            assert_eq!(again.len(), 3);
            assert!(again.iter().any(|c| c.path.as_ref().is_some_and(|p| p.to_string_lossy().ends_with(":2,S")) && c.subject == "Hello 0"), "its path as it is now");
            assert_eq!(parsed() - before, 3, "parsed once each");
        }
        assert_eq!(read_messages(&dir).len(), 3);
        assert_eq!(parsed() - before, 6, "nothing kept once the look ended");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

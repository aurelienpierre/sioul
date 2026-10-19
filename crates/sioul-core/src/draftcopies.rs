// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The copy of the message a draft answers or forwards, kept beside it
//! (`drafts/<id>.eml`, [`Draft::keep_original`]), stays as it came:
//! encrypted when it came so, opened only at Send, in memory
//! (`sioul_sync::send::build_draft`). Until 19 October 2026 Sioul wrote there
//! the decrypted text of an encrypted message it answered; the sharing
//! carried that text to each of your devices, and kept its earlier versions
//! on each (`sioul_sync::history`). [`seal_beside`] and [`seal_versions`]
//! encrypt those clear copies again, to your own certificates, on each
//! device that holds them, and overwrite what they held where the filesystem
//! lets it ([`crate::filelock::Placed::finish`]; docs/client.md, "What a
//! disk keeps"). Nothing else is touched: a draft's own file, its text and
//! its choices stay as they are, and a copy is replaced whole, never missing.
//!
//! Each copy's new bytes are written beside it first; then it takes its
//! place under the sharing's exchange lock (`Hold`), a short hold at a time,
//! only if the file is still as it was read; its old bytes are overwritten
//! once the lock is let go. The sharing reads these files only under that
//! lock, by their names: it never holds a file being replaced, so it never
//! reads, seals nor sends the zeros that follow. Every other reader (Send, the
//! writing window) reads through [`crate::filelock::read_whole`], which never
//! returns them.
//!
//! Which copies held decrypted text: those of a message that came
//! encrypted. Each copy names its message (its Message-ID); found in your
//! mail on this device, it says whether it came encrypted. Not found there
//! (archived elsewhere, deleted, never fetched on a phone), the draft says
//! it: one you encrypt has its copy encrypted too, at worst a message that
//! came in clear then opens with your key at Send; one you do not encrypt
//! keeps its copy, and the device that has its message decides, its
//! encrypted copy then reaching this one through the sharing. Earlier
//! versions, which only "Put back" reads, are all encrypted.

use crate::compose::Draft;
use crate::filelock::{FileStamp, Prepared};
use crate::pgp::{self, Keys};
use sequoia_openpgp::Cert;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The sharing's exchange lock, taken for a batch of replacements and let go
/// when what it returns is dropped (`sioul_sync::share::exchange_lock`); None
/// when it cannot be taken: the copies wait for the next look. A device that
/// has never shared gives a guard that holds nothing ([`unshared`]).
pub type Hold<'a> = &'a dyn Fn() -> Option<Box<dyn std::any::Any>>;

/// The guard of a device that has never shared: nothing to hold.
pub fn unshared() -> Option<Box<dyn std::any::Any>> {
    Some(Box::new(()))
}

/// The longest the replacements made under one hold of that lock take: an
/// exchange waiting for it (a dose's) never waits longer (the lesson of the
/// review of 18 October 2026, L-3). Only renames happen then: each copy's
/// new bytes are written before, its old bytes overwritten after.
const HOLD_AT_MOST: Duration = Duration::from_millis(200);
/// How long the lock is let go between two holds, for an exchange that waits.
const LET_GO: Duration = Duration::from_millis(20);
/// The copies prepared at once, their new bytes beside them on the disk.
const BATCH_FILES: usize = 32;
const BATCH_BYTES: usize = 16 << 20;

/// What a look did, in counts only: nothing of what the copies say.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sealed {
    /// Copies beside drafts encrypted again.
    pub copies: usize,
    /// Earlier versions of them, kept on this device, encrypted again.
    pub versions: usize,
    /// Copies in clear whose message is not in your mail here, of drafts you
    /// do not encrypt: left as they are (another device decides).
    pub unknown: usize,
    /// Copies that could not be rewritten (a full disk, a folder not
    /// writable): looked at again next time.
    pub failed: usize,
    /// Copies to encrypt while no certificate of yours takes encryption
    /// here: looked at again once one does.
    pub waiting: usize,
}

impl Sealed {
    /// Whether anything is worth saying.
    pub fn any(&self) -> bool {
        self.copies + self.versions + self.unknown + self.failed + self.waiting > 0
    }

    /// Both looks' counts.
    pub fn and(self, other: Sealed) -> Sealed {
        Sealed {
            copies: self.copies + other.copies,
            versions: self.versions + other.versions,
            unknown: self.unknown + other.unknown,
            failed: self.failed + other.failed,
            waiting: self.waiting + other.waiting,
        }
    }
}

/// The files looked at in this process, as they were then (name, size,
/// last change): not read again until they change.
#[derive(Debug, Default)]
pub struct Judged(HashSet<(PathBuf, u64, u128)>);

impl Judged {
    fn stamp(path: &Path) -> Option<(PathBuf, u64, u128)> {
        let meta = std::fs::metadata(path).ok()?;
        let changed = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
        Some((path.to_path_buf(), meta.len(), changed))
    }

    fn seen(&self, path: &Path) -> bool {
        Judged::stamp(path).is_some_and(|stamp| self.0.contains(&stamp))
    }

    fn mark(&mut self, path: &Path) {
        if let Some(stamp) = Judged::stamp(path) {
            self.0.insert(stamp);
        }
    }
}

/// The draft's name of a copy beside it: `<id>.eml`, its id plain letters
/// and digits as a draft's is ([`Draft::by_id`]).
fn copy_id(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let id = name.strip_suffix(".eml")?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())).then(|| id.to_string())
}

/// The Message-ID a message names, bare ("abc@example.org").
pub fn message_id(raw: &[u8]) -> Option<String> {
    let message = mail_parser::MessageParser::default().parse_headers(raw)?;
    Some(crate::mailindex::bare_id(message.message_id()?)).filter(|id| !id.is_empty())
}

/// The clear copies beside the drafts of `drafts` that held an encrypted
/// message's text, encrypted again to your own certificates. `find` gives
/// where a message of this Message-ID is in your mail here, if it is;
/// `hold`, the sharing's lock, under which each copy takes its place.
pub fn seal_beside(drafts: &Path, keys: &Keys, find: &mut dyn FnMut(&str) -> Option<PathBuf>, judged: &mut Judged, hold: Hold) -> Sealed {
    let mut out = Sealed::default();
    let to = keys.own_certs();
    let Ok(entries) = std::fs::read_dir(drafts) else { return out };
    let mut copies: Vec<(PathBuf, String)> = entries.filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_file())).filter_map(|e| Some((e.path(), copy_id(&e.path())?))).collect();
    copies.sort();
    let mut batch = Batch { waiting: VecDeque::new(), bytes: 0, hold };
    for (path, id) in copies {
        if judged.seen(&path) {
            continue;
        }
        let Ok((raw, meta)) = crate::filelock::read_whole_with(&path) else { continue };
        if pgp::is_encrypted(&raw) {
            judged.mark(&path);
            continue;
        }
        let came = message_id(&raw).and_then(|m| find(&m)).and_then(|p| crate::filelock::read_whole(&p).ok());
        let seal = match &came {
            Some(original) => pgp::is_encrypted(original),
            // Its draft encrypted (or gone): sealed; else left to a device that has its message.
            None => Draft::load(&drafts.join(format!("{id}.toml"))).is_none_or(|d| d.encrypt),
        };
        if !seal {
            if came.is_none() {
                out.unknown += 1;
            }
            judged.mark(&path);
            continue;
        }
        match prepare(&path, &raw, &to) {
            Some(Ok((prepared, size))) => batch.push(Waiting { prepared, was: FileStamp::of(&meta), path, version: false }, size, judged, &mut out),
            Some(Err(_)) => out.failed += 1,
            None => out.waiting += 1,
        }
    }
    batch.flush(judged, &mut out);
    out
}

/// Earlier versions of those copies (`paths`, each a file), those in clear
/// encrypted again to your own certificates, each under `hold` as above: only
/// "Put back" reads them, which then opens them with your key at Send.
pub fn seal_versions(paths: &[PathBuf], keys: &Keys, judged: &mut Judged, hold: Hold) -> Sealed {
    let mut out = Sealed::default();
    let to = keys.own_certs();
    let mut batch = Batch { waiting: VecDeque::new(), bytes: 0, hold };
    for path in paths {
        if judged.seen(path) {
            continue;
        }
        let Ok((raw, meta)) = crate::filelock::read_whole_with(path) else { continue };
        if pgp::is_encrypted(&raw) {
            judged.mark(path);
            continue;
        }
        match prepare(path, &raw, &to) {
            Some(Ok((prepared, size))) => batch.push(Waiting { prepared, was: FileStamp::of(&meta), path: path.clone(), version: true }, size, judged, &mut out),
            Some(Err(_)) => out.failed += 1,
            None => out.waiting += 1,
        }
    }
    batch.flush(judged, &mut out);
    out
}

/// `clear`, the content of the copy at `path`, encrypted to `to` and written
/// beside it, waiting for its place, with its size; None when no certificate
/// takes it.
fn prepare(path: &Path, clear: &[u8], to: &[&Cert]) -> Option<Result<(Prepared, usize), String>> {
    if to.is_empty() {
        return None;
    }
    Some(pgp::seal_message(clear, to).and_then(|sealed| Prepared::new(path, &sealed).map(|prepared| (prepared, sealed.len())).map_err(|e| format!("{}: {e}", path.display()))))
}

/// A copy's new bytes beside it, waiting for its place: the file as it was
/// read, and what it counts as.
struct Waiting {
    prepared: Prepared,
    was: FileStamp,
    path: PathBuf,
    version: bool,
}

/// Copies prepared, placed under the lock in short holds.
struct Batch<'a> {
    waiting: VecDeque<Waiting>,
    bytes: usize,
    hold: Hold<'a>,
}

impl Batch<'_> {
    fn push(&mut self, waiting: Waiting, size: usize, judged: &mut Judged, out: &mut Sealed) {
        self.bytes += size;
        self.waiting.push_back(waiting);
        if self.waiting.len() >= BATCH_FILES || self.bytes >= BATCH_BYTES {
            self.flush(judged, out);
        }
    }

    /// Each copy waiting put in its place under the lock, a short hold at a
    /// time, when the file is still as it was read (else it is looked at
    /// again next time: the sharing wrote another version meanwhile, or the
    /// draft went); its old bytes overwritten once the lock is let go.
    fn flush(&mut self, judged: &mut Judged, out: &mut Sealed) {
        self.bytes = 0;
        while !self.waiting.is_empty() {
            let Some(held) = (self.hold)() else {
                out.failed += self.waiting.len();
                self.waiting.clear();
                return;
            };
            let started = Instant::now();
            let mut placed = Vec::new();
            while let Some(waiting) = self.waiting.pop_front() {
                match waiting.prepared.place(Some(waiting.was)) {
                    Ok(Some(done)) => placed.push((done, waiting.path, waiting.version)),
                    Ok(None) => {}
                    Err(_) => out.failed += 1,
                }
                if started.elapsed() >= HOLD_AT_MOST {
                    break;
                }
            }
            drop(held);
            for (done, path, version) in placed {
                done.finish();
                if version {
                    out.versions += 1;
                } else {
                    out.copies += 1;
                }
                judged.mark(&path);
            }
            if !self.waiting.is_empty() {
                std::thread::sleep(LET_GO);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pgp::{SessionKeys, Unlock};

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("sioul-draftcopies-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn keys_of(key: &[u8]) -> Keys {
        let cert = Cert::from_bytes(key).unwrap();
        Keys { own: vec![cert], others: Vec::new(), cards: Vec::new() }
    }

    use sequoia_openpgp::parse::Parse;

    const CLEAR: &str = "From: Jane <jane@example.org>\r\nTo: Noa <noa@example.org>\r\nSubject: The lease\r\nMessage-ID: <lease@example.org>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nThe secret clause is on page four.\r\n";

    fn draft_with(folder: &Path, id: &str, encrypt: bool) {
        let draft = Draft { id: id.to_string(), encrypt, ..Draft::default() };
        std::fs::write(folder.join(format!("{id}.toml")), toml::to_string(&draft).unwrap()).unwrap();
    }

    fn opens(raw: &[u8], keys: &Keys) -> Vec<u8> {
        let sessions = SessionKeys::new();
        let none = |_: &pgp::Fingerprint| None;
        pgp::open(raw, keys, &Unlock::new(&none, &sessions)).and_then(|(opened, _)| opened).expect("opens with your key")
    }

    #[test]
    fn a_copy_of_an_encrypted_message_is_sealed_again_and_a_clear_one_left() {
        let base = scratch("beside");
        let (drafts, mail) = (base.join("drafts"), base.join("mail"));
        std::fs::create_dir_all(&drafts).unwrap();
        std::fs::create_dir_all(&mail).unwrap();
        let key = pgp::stand_in_key("Noa <noa@example.org>").unwrap();
        let keys = keys_of(&key);
        let cert = Cert::from_bytes(&key).unwrap();
        // In your mail: the lease came encrypted, the invoice in clear.
        std::fs::write(mail.join("lease"), pgp::seal_message(CLEAR.as_bytes(), &[&cert]).unwrap()).unwrap();
        let invoice = CLEAR.replace("lease@example.org", "invoice@example.org").replace("The secret clause is on page four.", "Your invoice.");
        std::fs::write(mail.join("invoice"), &invoice).unwrap();
        // The copies as Sioul wrote them until now: the lease's decrypted, the invoice's as it came.
        std::fs::write(drafts.join("a1.eml"), CLEAR).unwrap();
        std::fs::write(drafts.join("a2.eml"), &invoice).unwrap();
        // A copy whose message is in no mail here, of a draft you do not encrypt; and of one you do.
        let gone = CLEAR.replace("lease@example.org", "gone@example.org");
        std::fs::write(drafts.join("a3.eml"), &gone).unwrap();
        std::fs::write(drafts.join("a4.eml"), gone.replace("gone@", "gone2@")).unwrap();
        for (id, encrypt) in [("a1", false), ("a2", false), ("a3", false), ("a4", true)] {
            draft_with(&drafts, id, encrypt);
        }
        let mut find = |id: &str| match id {
            "lease@example.org" => Some(mail.join("lease")),
            "invoice@example.org" => Some(mail.join("invoice")),
            _ => None,
        };
        let mut judged = Judged::default();
        let sealed = seal_beside(&drafts, &keys, &mut find, &mut judged, &unshared);
        assert_eq!(sealed, Sealed { copies: 2, unknown: 1, ..Sealed::default() });
        // Sealed: no clear text left in the file, and it opens with your key to what it said.
        let a1 = std::fs::read(drafts.join("a1.eml")).unwrap();
        assert!(pgp::is_encrypted(&a1));
        assert!(!String::from_utf8_lossy(&a1).contains("secret clause"));
        assert!(String::from_utf8_lossy(&opens(&a1, &keys)).contains("The secret clause is on page four."));
        assert!(pgp::is_encrypted(&std::fs::read(drafts.join("a4.eml")).unwrap()));
        // Left: the clear message's copy, and the one no device here can judge.
        assert_eq!(std::fs::read_to_string(drafts.join("a2.eml")).unwrap(), invoice);
        assert_eq!(std::fs::read_to_string(drafts.join("a3.eml")).unwrap(), gone);
        // The drafts themselves untouched; nothing else left in the folder.
        for id in ["a1", "a2", "a3", "a4"] {
            assert!(Draft::load(&drafts.join(format!("{id}.toml"))).is_some());
        }
        let names: Vec<String> = std::fs::read_dir(&drafts).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        assert_eq!(names.len(), 8, "{names:?}");
        // Looked at again: nothing more to do, nothing read again.
        let again = seal_beside(&drafts, &keys, &mut |_| panic!("not asked again"), &mut judged, &unshared);
        assert_eq!(again, Sealed::default());
        // A new process: what was sealed stays sealed, once.
        let fresh = seal_beside(&drafts, &keys, &mut find, &mut Judged::default(), &unshared);
        assert_eq!(fresh, Sealed { unknown: 1, ..Sealed::default() });
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn without_a_certificate_of_yours_nothing_is_written() {
        let base = scratch("unkeyed");
        std::fs::write(base.join("a1.eml"), CLEAR).unwrap();
        draft_with(&base, "a1", true);
        let mut judged = Judged::default();
        let none = Keys::default();
        assert_eq!(seal_beside(&base, &none, &mut |_| None, &mut judged, &unshared), Sealed { waiting: 1, ..Sealed::default() });
        assert_eq!(std::fs::read_to_string(base.join("a1.eml")).unwrap(), CLEAR);
        // Looked at again once a key takes it.
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        assert_eq!(seal_beside(&base, &keys, &mut |_| None, &mut judged, &unshared).copies, 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The lock not given (the sharing's state not writable): nothing takes
    /// its place, the copy stays as it was, nothing is left beside it, and it
    /// is looked at again next time.
    #[test]
    fn nothing_takes_its_place_without_the_lock() {
        let base = scratch("unlocked");
        std::fs::write(base.join("a1.eml"), CLEAR).unwrap();
        draft_with(&base, "a1", true);
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        let mut judged = Judged::default();
        let refused = || -> Option<Box<dyn std::any::Any>> { None };
        assert_eq!(seal_beside(&base, &keys, &mut |_| None, &mut judged, &refused), Sealed { failed: 1, ..Sealed::default() });
        assert_eq!(std::fs::read_to_string(base.join("a1.eml")).unwrap(), CLEAR);
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 2, "the copy and its draft alone");
        // Each placement happens while the lock is held, and the lock is given back between holds.
        let holding = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        struct Guard(std::sync::Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let counted = {
            let holding = holding.clone();
            move || -> Option<Box<dyn std::any::Any>> {
                assert_eq!(holding.fetch_add(1, std::sync::atomic::Ordering::SeqCst), 0, "one hold at a time");
                Some(Box::new(Guard(holding.clone())))
            }
        };
        assert_eq!(seal_beside(&base, &keys, &mut |_| None, &mut judged, &counted).copies, 1);
        assert_eq!(holding.load(std::sync::atomic::Ordering::SeqCst), 0, "given back");
        assert!(pgp::is_encrypted(&std::fs::read(base.join("a1.eml")).unwrap()));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A copy the sharing wrote again between its reading and its place (or
    /// that went with its draft): left as it is now, nothing beside it.
    #[test]
    fn a_copy_changed_since_it_was_read_keeps_its_new_version() {
        let base = scratch("changed");
        let path = base.join("a1.eml");
        std::fs::write(&path, CLEAR).unwrap();
        let (_, meta) = crate::filelock::read_whole_with(&path).unwrap();
        let prepared = Prepared::new(&path, b"sealed").unwrap();
        // Another version comes meanwhile, as the sharing writes it (beside, then renamed).
        std::fs::write(base.join(".a1.eml.incoming"), "another version").unwrap();
        std::fs::rename(base.join(".a1.eml.incoming"), &path).unwrap();
        assert!(prepared.place(Some(FileStamp::of(&meta))).unwrap().is_none());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "another version");
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 1, "nothing left beside it");
        // Gone with its draft: never made again.
        let (_, meta) = crate::filelock::read_whole_with(&path).unwrap();
        let prepared = Prepared::new(&path, b"sealed").unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(prepared.place(Some(FileStamp::of(&meta))).unwrap().is_none());
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn earlier_versions_are_sealed_too() {
        let base = scratch("versions");
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        let (clear, sealed) = (base.join("2026-10-18T10-00-00.000Z"), base.join("2026-10-19T10-00-00.000Z"));
        std::fs::write(&clear, CLEAR).unwrap();
        std::fs::write(&sealed, pgp::seal_message(CLEAR.as_bytes(), &keys.own_certs()).unwrap()).unwrap();
        let before = std::fs::read(&sealed).unwrap();
        let done = seal_versions(&[clear.clone(), sealed.clone()], &keys, &mut Judged::default(), &unshared);
        assert_eq!(done, Sealed { versions: 1, ..Sealed::default() });
        assert!(!String::from_utf8_lossy(&std::fs::read(&clear).unwrap()).contains("secret clause"));
        assert_eq!(std::fs::read(&sealed).unwrap(), before, "one sealed already is left as it is");
        let _ = std::fs::remove_dir_all(&base);
    }

    // The old file's bytes overwritten once it has no name: read through a
    // descriptor opened before, as anyone holding the old blocks would.
    #[cfg(unix)]
    #[test]
    fn the_old_bytes_are_overwritten_and_the_path_never_missing() {
        use std::io::{Read, Seek};
        let base = scratch("wipe");
        let path = base.join("a1.eml");
        std::fs::write(&path, CLEAR).unwrap();
        let mut before = std::fs::File::open(&path).unwrap();
        crate::filelock::replace_wiping(&path, b"sealed").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"sealed");
        let mut old = Vec::new();
        before.seek(std::io::SeekFrom::Start(0)).unwrap();
        before.read_to_end(&mut old).unwrap();
        assert_eq!(old.len(), CLEAR.len());
        assert!(old.iter().all(|&b| b == 0), "the old file still says {:?}", String::from_utf8_lossy(&old));
        // Yours alone, and no hidden file left beside it.
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&path).unwrap().permissions()) & 0o777, 0o600);
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 1);
        // Another name of the same file (a hard link) keeps it whole.
        let other = base.join("other");
        std::fs::hard_link(&path, &other).unwrap();
        crate::filelock::replace_wiping(&path, b"sealed again").unwrap();
        assert_eq!(std::fs::read(&other).unwrap(), b"sealed");
        // Meanwhile, through 1,000 replacements, a reader reading as Sioul's
        // readers do (`read_whole`) finds the old file or the new one, never
        // the zeros written over the old one once it has no name: a plain
        // read may (main's final pass, 19 October 2026, read twelve zeros).
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reader = {
            let (path, stop) = (path.clone(), stop.clone());
            std::thread::spawn(move || {
                let (mut read, mut again) = (0usize, 0usize);
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    match crate::filelock::read_whole(&path) {
                        Ok(bytes) => {
                            assert!(bytes == b"sealed again" || bytes == b"third", "{bytes:?}");
                            read += 1;
                        }
                        // Replaced each of its eight times: said, never returned.
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => again += 1,
                        Err(e) => panic!("never missing: {e}"),
                    }
                }
                (read, again)
            })
        };
        for _ in 0..500 {
            crate::filelock::replace_wiping(&path, b"third").unwrap();
            crate::filelock::replace_wiping(&path, b"sealed again").unwrap();
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let (read, again) = reader.join().unwrap();
        assert!(read > 0 && again <= read / 10 + 1, "{read} reads, {again} given up");
        let _ = std::fs::remove_dir_all(&base);
    }
}

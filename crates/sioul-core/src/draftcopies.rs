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
//! Which copies may hold decrypted text, in doubt sealed (review of 19
//! October 2026, F6 and N7), judged by where they come from, never by their
//! shape alone: a copy kept as it came (its draft says so, from 19 October
//! 2026: `Draft::copy_as_received`) never. Any other was written before: the
//! message itself when it came in clear, its decrypted text when it came
//! encrypted. It is sealed unless it is, byte for byte, a message found here
//! under its Message-ID or the one its draft answers (`Draft::in_reply_to`),
//! every message of those IDs here having come in clear: a copy of a message
//! found nowhere, or of one another message shares its ID with (anyone who
//! knows an ID can send one), is sealed. A decrypted entity with no header
//! of its own looks like a message that came as it is (N7): its shape says
//! nothing. Its earlier versions and what was kept before sharing began are
//! judged the same way, and "Put back" opens a sealed one with your key at
//! Send.
//!
//! What a look decides is kept with the file as it was read (its stamp
//! taken from the descriptor that read it, or from the new file before it
//! took its place), never as the path names it afterwards: a version an
//! exchange puts in place meanwhile is looked at again (N6).

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
    /// Copies that could not be rewritten (a full disk, a folder not
    /// writable): looked at again next time.
    pub failed: usize,
    /// Copies to encrypt while no certificate of yours takes encryption
    /// here: looked at again once one does.
    pub waiting: usize,
    /// Encrypted copies whose draft is gone (sent, or thrown away while the
    /// look replaced the copy), taken out a day after (review of 19 October
    /// 2026, F17).
    pub orphans: usize,
}

impl Sealed {
    /// Whether anything is worth saying.
    pub fn any(&self) -> bool {
        self.copies + self.versions + self.failed + self.waiting + self.orphans > 0
    }

    /// Both looks' counts.
    pub fn and(self, other: Sealed) -> Sealed {
        Sealed {
            copies: self.copies + other.copies,
            versions: self.versions + other.versions,
            failed: self.failed + other.failed,
            waiting: self.waiting + other.waiting,
            orphans: self.orphans + other.orphans,
        }
    }
}

/// The files looked at, as they were then (their device, number, size and
/// time): not read again until they change. Kept on the disk between
/// processes ([`Judged::load`], [`Judged::save`]: `state/draftcopies.json`,
/// never shared), so that a process started for a moment (a phone's
/// background step) reads only what changed since the last look; one that
/// does not read (another format, damaged) is a look from nothing, which
/// seals nothing twice.
#[derive(Debug, Default)]
pub struct Judged {
    files: std::collections::BTreeMap<PathBuf, FileStamp>,
    /// Copies the last look left waiting for a certificate of yours (said in Accounts ▸ Encryption).
    waiting: usize,
    /// Looked at in this look: the others, gone or out of reach, are forgotten at the save.
    met: HashSet<PathBuf>,
    changed: bool,
}

/// The verdicts' file: its format, and each file's stamp.
#[derive(serde::Serialize, serde::Deserialize)]
struct JudgedFile {
    format: u32,
    files: std::collections::BTreeMap<PathBuf, FileStamp>,
    #[serde(default)]
    waiting: usize,
}

/// The verdicts' format: another one is read as none.
const JUDGED_FORMAT: u32 = 1;

impl Judged {
    /// The verdicts kept at `path`; none when it does not read.
    pub fn load(path: &Path) -> Judged {
        let (files, waiting) = std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice::<JudgedFile>(&bytes).ok()).filter(|f| f.format == JUDGED_FORMAT).map(|f| (f.files, f.waiting)).unwrap_or_default();
        Judged { files, waiting, met: HashSet::new(), changed: false }
    }

    /// The copies the last look left waiting for a certificate of yours that takes a seal.
    pub fn waiting(&self) -> usize {
        self.waiting
    }

    /// What a look found waiting, kept for the next save.
    pub fn set_waiting(&mut self, waiting: usize) {
        if self.waiting != waiting {
            self.waiting = waiting;
            self.changed = true;
        }
    }

    /// The verdicts written at `path` when they changed (beside it, then
    /// renamed: never half a file), the files not met since the last save
    /// forgotten.
    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        let before = self.files.len();
        let met = std::mem::take(&mut self.met);
        self.files.retain(|file, _| met.contains(file));
        if !self.changed && self.files.len() == before {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_vec(&JudgedFile { format: JUDGED_FORMAT, files: self.files.clone(), waiting: self.waiting }).map_err(std::io::Error::other)?;
        crate::filelock::replace(path, &text)?;
        self.changed = false;
        Ok(())
    }

    /// The file's stamp now, when it was not judged as it is (None: judged,
    /// or gone).
    fn unseen(&mut self, path: &Path) -> Option<FileStamp> {
        let now = FileStamp::now(path)?;
        self.met.insert(path.to_path_buf());
        (self.files.get(path) != Some(&now)).then_some(now)
    }

    /// What was judged: the file as it was then (`stamp`), never what the
    /// path names now, which another version may have taken meanwhile (N6).
    fn mark(&mut self, path: &Path, stamp: FileStamp) {
        self.met.insert(path.to_path_buf());
        if self.files.insert(path.to_path_buf(), stamp) != Some(stamp) {
            self.changed = true;
        }
    }
}

/// Your own certificates, to seal to: read only once a copy is to be sealed
/// (reading every key, others' too, costs a phone's step its time: 18 ms for
/// 300 others' keys, review of 19 October 2026, F9).
pub struct Certs<'a> {
    load: Box<dyn Fn() -> Keys + 'a>,
    keys: std::cell::OnceCell<Keys>,
}

impl<'a> Certs<'a> {
    /// Read by `load` when first needed.
    pub fn lazy(load: impl Fn() -> Keys + 'a) -> Certs<'a> {
        Certs { load: Box::new(load), keys: std::cell::OnceCell::new() }
    }

    /// Those of `keys`, read already.
    pub fn of(keys: Keys) -> Certs<'a> {
        let cell = std::cell::OnceCell::new();
        let _ = cell.set(keys);
        Certs { load: Box::new(Keys::default), keys: cell }
    }

    fn get(&self) -> Vec<&Cert> {
        self.keys.get_or_init(|| (self.load)()).own_certs()
    }

    /// Whether they were read: never when nothing was to be sealed.
    pub fn read(&self) -> bool {
        self.keys.get().is_some()
    }
}

#[cfg(test)]
thread_local! {
    /// The copies read whole by this thread's looks: none for a file met before, unchanged.
    static READ: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A copy read whole for a look (counted in tests).
fn read_copy(path: &Path) -> std::io::Result<(Vec<u8>, std::fs::Metadata)> {
    #[cfg(test)]
    READ.with(|n| n.set(n.get() + 1));
    crate::filelock::read_whole_with(path)
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

/// Whether a copy in clear may hold an encrypted message's text (in doubt,
/// yes), a copy not kept as it came: see the module's words. `answers`, the
/// Message-ID its draft answers, if any; `find` gives every file of a
/// Message-ID in your mail here.
fn may_hold_decrypted(raw: &[u8], answers: Option<&str>, find: &mut dyn FnMut(&str) -> Vec<PathBuf>) -> bool {
    let mut ids: Vec<String> = message_id(raw).into_iter().collect();
    if let Some(answers) = answers.map(crate::mailindex::bare_id).filter(|id| !id.is_empty() && !ids.contains(id)) {
        ids.push(answers);
    }
    if ids.is_empty() {
        return true;
    }
    let found: Vec<Vec<u8>> = ids.iter().flat_map(|id| find(id)).filter_map(|path| crate::filelock::read_whole(&path).ok()).collect();
    let every_clear = !found.is_empty() && found.iter().all(|message| !pgp::is_encrypted(message));
    !(every_clear && found.iter().any(|message| message.as_slice() == raw))
}

/// The clear copies beside the drafts of `drafts` that may hold an
/// encrypted message's text, encrypted again to your own certificates.
/// `find` gives every file of a Message-ID in your mail here; `hold`, the
/// sharing's lock, under which each copy takes its place.
pub fn seal_beside(drafts: &Path, certs: &Certs, find: &mut dyn FnMut(&str) -> Vec<PathBuf>, judged: &mut Judged, hold: Hold) -> Sealed {
    let mut out = Sealed::default();
    let Ok(entries) = std::fs::read_dir(drafts) else { return out };
    let mut copies: Vec<(PathBuf, String)> = entries.filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_file())).filter_map(|e| Some((e.path(), copy_id(&e.path())?))).collect();
    copies.sort();
    let mut batch = Batch { waiting: VecDeque::new(), bytes: 0, hold };
    let mut orphans = Vec::new();
    for (path, id) in copies {
        let draft = Draft::load(&drafts.join(format!("{id}.toml")));
        // Encrypted, its draft gone more than a day ago: an orphan, taken out below.
        if draft.is_none() && orphaned(&path) {
            orphans.push(path);
            continue;
        }
        let Some(stamp) = judged.unseen(&path) else { continue };
        // Kept as it came, wherever the draft was written: never decrypted.
        if draft.as_ref().is_some_and(|d| d.copy_as_received) {
            judged.mark(&path, stamp);
            continue;
        }
        let Ok((raw, meta)) = read_copy(&path) else { continue };
        // Judged as read: the stamp of the bytes read, from their descriptor (N6).
        let read = FileStamp::of(&meta);
        let answers = draft.as_ref().and_then(|d| d.in_reply_to.clone());
        if pgp::is_encrypted(&raw) || !may_hold_decrypted(&raw, answers.as_deref(), find) {
            judged.mark(&path, read);
            continue;
        }
        match prepare(&path, &raw, &certs.get()) {
            Some(Ok((prepared, size))) => batch.push(Waiting { prepared, was: read, path, version: false }, size, judged, &mut out),
            Some(Err(_)) => out.failed += 1,
            None => out.waiting += 1,
        }
    }
    batch.flush(judged, &mut out);
    let mut orphans: VecDeque<PathBuf> = orphans.into();
    // Taken out under the lock as the copies take their place: a short hold at a time.
    while !orphans.is_empty() {
        let Some(held) = hold() else { break };
        let started = Instant::now();
        while let Some(path) = orphans.pop_front() {
            // Still an orphan under the lock (its draft did not come meanwhile).
            if !drafts.join(format!("{}.toml", copy_id(&path).unwrap_or_default())).exists() && std::fs::remove_file(&path).is_ok() {
                out.orphans += 1;
            }
            if started.elapsed() >= HOLD_AT_MOST {
                break;
            }
        }
        drop(held);
        if !orphans.is_empty() {
            std::thread::sleep(LET_GO);
        }
    }
    out
}

/// An encrypted copy left alone more than a day: its draft, gone, will not come.
fn orphaned(path: &Path) -> bool {
    let old = std::fs::metadata(path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 24 * 3600);
    old && crate::filelock::read_whole(path).is_ok_and(|raw| pgp::is_encrypted(&raw))
}

/// Earlier versions of those copies (`paths`, each a file), judged as the
/// copies are, encrypted again to your own certificates, each under `hold`
/// as above: only "Put back" reads them, which then opens a sealed one with
/// your key at Send.
pub fn seal_versions(paths: &[PathBuf], certs: &Certs, find: &mut dyn FnMut(&str) -> Vec<PathBuf>, judged: &mut Judged, hold: Hold) -> Sealed {
    let mut out = Sealed::default();
    let mut batch = Batch { waiting: VecDeque::new(), bytes: 0, hold };
    for path in paths {
        if judged.unseen(path).is_none() {
            continue;
        }
        let Ok((raw, meta)) = read_copy(path) else { continue };
        let read = FileStamp::of(&meta);
        if pgp::is_encrypted(&raw) || !may_hold_decrypted(&raw, None, find) {
            judged.mark(path, read);
            continue;
        }
        match prepare(path, &raw, &certs.get()) {
            Some(Ok((prepared, size))) => batch.push(Waiting { prepared, was: read, path: path.clone(), version: true }, size, judged, &mut out),
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
                // The sealed copy as it took its place, never what an exchange puts there after the hold (N6).
                let stamp = done.stamp();
                done.finish();
                if version {
                    out.versions += 1;
                } else {
                    out.copies += 1;
                }
                if let Some(stamp) = stamp {
                    judged.mark(&path, stamp);
                }
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
        Keys { own: vec![cert], ..Keys::default() }
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
            "lease@example.org" => vec![mail.join("lease")],
            "invoice@example.org" => vec![mail.join("invoice")],
            _ => Vec::new(),
        };
        let mut judged = Judged::default();
        let sealed = seal_beside(&drafts, &Certs::of(keys.clone()), &mut find, &mut judged, &unshared);
        // a1 (its message came encrypted), a3 and a4 (their message found nowhere: in doubt) sealed; a2 left.
        assert_eq!(sealed, Sealed { copies: 3, ..Sealed::default() });
        // Sealed: no clear text left in the file, and it opens with your key to what it said.
        let a1 = std::fs::read(drafts.join("a1.eml")).unwrap();
        assert!(pgp::is_encrypted(&a1));
        assert!(!String::from_utf8_lossy(&a1).contains("secret clause"));
        assert!(String::from_utf8_lossy(&opens(&a1, &keys)).contains("The secret clause is on page four."));
        assert!(pgp::is_encrypted(&std::fs::read(drafts.join("a4.eml")).unwrap()));
        // Left: the clear message's copy, byte for byte that message.
        assert_eq!(std::fs::read_to_string(drafts.join("a2.eml")).unwrap(), invoice);
        assert!(pgp::is_encrypted(&std::fs::read(drafts.join("a3.eml")).unwrap()), "found nowhere: sealed, in doubt");
        // The drafts themselves untouched; nothing else left in the folder.
        for id in ["a1", "a2", "a3", "a4"] {
            assert!(Draft::load(&drafts.join(format!("{id}.toml"))).is_some());
        }
        let names: Vec<String> = std::fs::read_dir(&drafts).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        assert_eq!(names.len(), 8, "{names:?}");
        // Looked at again: nothing more to do, nothing read again.
        let again = seal_beside(&drafts, &Certs::of(keys.clone()), &mut |_| panic!("not asked again"), &mut judged, &unshared);
        assert_eq!(again, Sealed::default());
        // A new process: what was sealed stays sealed, once.
        let fresh = seal_beside(&drafts, &Certs::of(keys.clone()), &mut find, &mut Judged::default(), &unshared);
        assert_eq!(fresh, Sealed::default());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Review of 19 October 2026, P6 (F6): the message came encrypted
    /// (archived), and a clear message with the same Message-ID waits in the
    /// inbox (anyone who knows the ID can send one); the copy beside the
    /// answer, decrypted as Sioul wrote it before, its draft not encrypted:
    /// sealed, never left on the other's word. A copy without a Message-ID:
    /// sealed too.
    #[test]
    fn a_forged_duplicate_or_no_message_id_leaves_no_copy_in_clear() {
        let base = scratch("forged");
        let (drafts, mail) = (base.join("drafts"), base.join("mail"));
        std::fs::create_dir_all(&drafts).unwrap();
        std::fs::create_dir_all(&mail).unwrap();
        let key = pgp::stand_in_key("Noa <noa@example.org>").unwrap();
        let keys = keys_of(&key);
        std::fs::write(mail.join("archive"), pgp::seal_message(CLEAR.as_bytes(), &keys.own_certs()).unwrap()).unwrap();
        std::fs::write(mail.join("inbox"), CLEAR.replace("The secret clause is on page four.", "Nothing to see.")).unwrap();
        std::fs::write(drafts.join("a1.eml"), CLEAR).unwrap();
        std::fs::write(drafts.join("a2.eml"), CLEAR.replace("Message-ID: <lease@example.org>\r\n", "")).unwrap();
        for id in ["a1", "a2"] {
            draft_with(&drafts, id, false);
        }
        // The inbox first, as the index lists it.
        let mut find = |id: &str| if id == "lease@example.org" { vec![mail.join("inbox"), mail.join("archive")] } else { Vec::new() };
        assert_eq!(seal_beside(&drafts, &Certs::of(keys.clone()), &mut find, &mut Judged::default(), &unshared), Sealed { copies: 2, ..Sealed::default() });
        for id in ["a1", "a2"] {
            assert!(pgp::is_encrypted(&std::fs::read(drafts.join(format!("{id}.eml"))).unwrap()), "{id}");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Review of 19 October 2026, P7 (F7): an encrypted message whose text
    /// quotes an older armored block; its copy, as decrypted before, beside an
    /// answer: never taken for sealed. A message whose whole text is one
    /// armored block is encrypted as it came.
    #[test]
    fn a_copy_quoting_an_armored_block_is_still_sealed() {
        let base = scratch("armored");
        let (drafts, mail) = (base.join("drafts"), base.join("mail"));
        std::fs::create_dir_all(&drafts).unwrap();
        std::fs::create_dir_all(&mail).unwrap();
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        let quoting = format!("{CLEAR}> -----BEGIN PGP MESSAGE-----\r\n> hQEMA0aY\r\n> -----END PGP MESSAGE-----\r\n");
        assert!(!pgp::is_encrypted(quoting.as_bytes()));
        std::fs::write(mail.join("lease"), pgp::seal_message(quoting.as_bytes(), &keys.own_certs()).unwrap()).unwrap();
        std::fs::write(drafts.join("a1.eml"), &quoting).unwrap();
        draft_with(&drafts, "a1", false);
        let mut find = |_: &str| vec![mail.join("lease")];
        assert_eq!(seal_beside(&drafts, &Certs::of(keys.clone()), &mut find, &mut Judged::default(), &unshared).copies, 1);
        let inline = "From: Jane <jane@example.org>\r\nSubject: The lease\r\n\r\n-----BEGIN PGP MESSAGE-----\r\n\r\nhQEMA0aY\r\n-----END PGP MESSAGE-----\r\n";
        assert!(pgp::is_encrypted(inline.as_bytes()));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A copy kept as it came (its draft says so, from 19 October 2026) is
    /// never judged, even when its message is found nowhere here (a phone),
    /// whatever its shape. One written before is judged by where it comes
    /// from, never by its shape (review of 19 October 2026, N7): a clear
    /// message's copy is left where that message is found here byte for
    /// byte (by its Message-ID, or the one its draft answers), sealed where
    /// it is found nowhere, whatever mailer wrote it.
    #[test]
    fn copies_kept_as_they_came_are_left_and_the_others_judged_by_their_message() {
        let base = scratch("as-came");
        let mail = scratch("as-came-mail");
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        std::fs::write(base.join("a1.eml"), CLEAR).unwrap();
        let draft = Draft { id: "a1".into(), copy_as_received: true, ..Draft::default() };
        std::fs::write(base.join("a1.toml"), toml::to_string(&draft).unwrap()).unwrap();
        let apple = "Content-Type: text/plain; charset=us-ascii\r\nMime-Version: 1.0 (Mac OS X Mail 16.0)\r\nFrom: Jane <jane@example.org>\r\nSubject: The lease\r\nMessage-ID: <a@example.org>\r\n\r\nThe clause.\r\n";
        let outlook = "From: Jane <jane@example.org>\r\nSubject: The lease\r\nContent-Type: text/plain; charset=utf-8\r\nMIME-Version: 1.0\r\nMessage-ID: <o@example.org>\r\n\r\nThe clause.\r\n";
        let plain = "From: Jane <jane@example.org>\r\nSubject: The lease\r\n\r\nThe clause, no Message-ID.\r\n";
        for (id, text) in [("a2", apple), ("a3", outlook), ("a4", plain)] {
            std::fs::write(base.join(format!("{id}.eml")), text).unwrap();
            std::fs::write(mail.join(id), text).unwrap();
        }
        draft_with(&base, "a2", false);
        draft_with(&base, "a3", false);
        // The plain one names no Message-ID: its draft says what it answers.
        let answering = Draft { id: "a4".into(), in_reply_to: Some("<p@example.org>".into()), ..Draft::default() };
        std::fs::write(base.join("a4.toml"), toml::to_string(&answering).unwrap()).unwrap();
        let mut find = |id: &str| match id {
            "a@example.org" => vec![mail.join("a2")],
            "o@example.org" => vec![mail.join("a3")],
            "p@example.org" => vec![mail.join("a4")],
            _ => Vec::new(),
        };
        let left = seal_beside(&base, &Certs::lazy(|| -> Keys { panic!("nothing to seal") }), &mut find, &mut Judged::default(), &unshared);
        assert_eq!(left, Sealed::default(), "kept as it came, or byte for byte a clear message found here");
        // On a device where those messages are not (a phone): sealed, the kept one aside.
        let sealed = seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut Judged::default(), &unshared);
        assert_eq!(sealed, Sealed { copies: 3, ..Sealed::default() });
        assert_eq!(std::fs::read_to_string(base.join("a1.eml")).unwrap(), CLEAR, "kept as it came: untouched");
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&mail);
    }

    /// Review of 19 October 2026, N7 (probe `probe2_shapes_of_copies`): the
    /// copy of an encrypted message whose encrypted entity had an empty
    /// head, or none, as Sioul wrote it before: no `Content-` line follows
    /// the `MIME-Version: 1.0` it added, and the shape looked like a message
    /// that came as it is. Sealed, its message found encrypted here or
    /// found nowhere.
    #[test]
    fn a_decrypted_copy_without_headers_of_its_own_is_sealed() {
        let base = scratch("headless");
        let mail = scratch("headless-mail");
        let key = pgp::stand_in_key("Noa <noa@example.org>").unwrap();
        let keys = keys_of(&key);
        let cert = Cert::from_bytes(&key).unwrap();
        let none = |_: &pgp::Fingerprint| None;
        let sessions = SessionKeys::new();
        let unlock = Unlock::new(&none, &sessions);
        for (n, entity) in ["\r\nSECRET-2 the clause\r\n", "SECRET-3 the clause\r\nline two\r\n"].iter().enumerate() {
            let armored = pgp::encrypt(entity.as_bytes(), &[&cert], None).unwrap();
            let received = format!(
                "From: Jane <jane@example.org>\nTo: Noa <noa@example.org>\nSubject: ...\nMessage-ID: <s{n}@example.org>\nMIME-Version: 1.0\nContent-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=\"o\"\n\n--o\nContent-Type: application/pgp-encrypted\n\nVersion: 1\n\n--o\nContent-Type: application/octet-stream\n\n{armored}\n--o--\n"
            );
            std::fs::write(mail.join(format!("m{n}")), &received).unwrap();
            let (opened, _) = pgp::open(received.as_bytes(), &keys, &unlock).unwrap();
            let copy = opened.unwrap();
            assert!(!pgp::looks_opened(&copy), "its shape says nothing");
            std::fs::write(base.join(format!("b{n}.eml")), &copy).unwrap();
            draft_with(&base, &format!("b{n}"), false);
        }
        let mut find = |id: &str| if id == "s0@example.org" { vec![mail.join("m0")] } else { Vec::new() };
        let done = seal_beside(&base, &Certs::of(keys.clone()), &mut find, &mut Judged::default(), &unshared);
        assert_eq!(done, Sealed { copies: 2, ..Sealed::default() });
        for n in 0..2 {
            let copy = std::fs::read(base.join(format!("b{n}.eml"))).unwrap();
            assert!(pgp::is_encrypted(&copy) && !String::from_utf8_lossy(&copy).contains("SECRET-"));
        }
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&mail);
    }

    /// Review of 19 October 2026, N6 (probe `probe2_race_a_…`): a clear
    /// copy, byte for byte its clear message; while the look reads the mail,
    /// an exchange puts another device's decrypted version in its place. The
    /// verdict is kept for the file read, not for the one there now: the
    /// next look reads the new one and seals it, in this process and in a
    /// later one.
    #[test]
    fn a_copy_replaced_while_the_look_reads_the_mail_is_looked_at_again() {
        let base = scratch("race-a");
        let mail = scratch("race-a-mail");
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        let thunderbird = "Message-ID: <lease@example.org>\r\nMIME-Version: 1.0\r\nSubject: The lease\r\nFrom: Jane <jane@example.org>\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\nNothing secret here.\r\n";
        std::fs::write(mail.join("m1"), thunderbird).unwrap();
        let path = base.join("a1.eml");
        std::fs::write(&path, thunderbird).unwrap();
        draft_with(&base, "a1", false);
        let incoming = CLEAR.to_string();
        let mut find = |id: &str| {
            let beside = base.join(".incoming");
            std::fs::write(&beside, &incoming).unwrap();
            std::fs::rename(&beside, &path).unwrap();
            if id == "lease@example.org" { vec![mail.join("m1")] } else { Vec::new() }
        };
        let verdicts = base.join("state").join("draftcopies.json");
        let mut judged = Judged::load(&verdicts);
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut find, &mut judged, &unshared), Sealed::default(), "the copy read was its clear message");
        judged.save(&verdicts).unwrap();
        let mut later = Judged::load(&verdicts);
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut later, &unshared).copies, 1, "the version put in its place meanwhile, looked at");
        assert!(pgp::is_encrypted(&std::fs::read(&path).unwrap()));
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&mail);
    }

    /// Review of 19 October 2026, N6 (probe `probe2_race_b_…`): a decrypted
    /// copy sealed; as soon as the lock is let go, an exchange waiting puts
    /// another device's decrypted version in its place. The verdict is kept
    /// for the sealed file that took the place, not for the one there now:
    /// the next look seals the new one.
    #[test]
    fn a_copy_replaced_as_soon_as_the_look_lets_go_is_looked_at_again() {
        struct Swap {
            path: PathBuf,
            bytes: Vec<u8>,
            armed: std::rc::Rc<std::cell::Cell<bool>>,
        }
        impl Drop for Swap {
            fn drop(&mut self) {
                if self.armed.replace(false) {
                    let beside = self.path.with_extension("incoming");
                    std::fs::write(&beside, &self.bytes).unwrap();
                    std::fs::rename(&beside, &self.path).unwrap();
                }
            }
        }
        let base = scratch("race-b");
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        let path = base.join("a1.eml");
        std::fs::write(&path, CLEAR).unwrap();
        draft_with(&base, "a1", true);
        let second = CLEAR.replace("page four", "page five");
        let armed = std::rc::Rc::new(std::cell::Cell::new(true));
        let hold = || -> Option<Box<dyn std::any::Any>> { Some(Box::new(Swap { path: path.clone(), bytes: second.clone().into_bytes(), armed: armed.clone() })) };
        let mut judged = Judged::default();
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut judged, &hold).copies, 1);
        assert!(String::from_utf8_lossy(&std::fs::read(&path).unwrap()).contains("page five"), "the other version took its place");
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut judged, &unshared).copies, 1, "and is looked at");
        assert!(pgp::is_encrypted(&std::fs::read(&path).unwrap()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn without_a_certificate_of_yours_nothing_is_written() {
        let base = scratch("unkeyed");
        std::fs::write(base.join("a1.eml"), CLEAR).unwrap();
        draft_with(&base, "a1", true);
        let mut judged = Judged::default();
        let none = Keys::default();
        assert_eq!(seal_beside(&base, &Certs::of(none.clone()), &mut |_| Vec::new(), &mut judged, &unshared), Sealed { waiting: 1, ..Sealed::default() });
        assert_eq!(std::fs::read_to_string(base.join("a1.eml")).unwrap(), CLEAR);
        // Looked at again once a key takes it.
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut judged, &unshared).copies, 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The lock not given (the sharing's state not writable): nothing takes
    /// its place, the copy stays as it was, nothing is left beside it, and it
    /// is looked at again next time.
    /// The builder's own known gap (review of 19 October 2026, F17): a draft
    /// sent while its copy took its place left an encrypted copy without its
    /// draft, which travelled to every device and stayed there. Taken out
    /// once a day old, under the sharing's lock alone; a fresh one (its draft
    /// may still come) and one whose draft is there stay.
    #[test]
    fn an_encrypted_copy_whose_draft_went_is_taken_out_a_day_later() {
        let base = scratch("orphans");
        let key = pgp::stand_in_key("Noa <noa@example.org>").unwrap();
        let keys = keys_of(&key);
        let sealed = pgp::seal_message(CLEAR.as_bytes(), &[&Cert::from_bytes(&key).unwrap()]).unwrap();
        for id in ["gone", "fresh", "kept"] {
            std::fs::write(base.join(format!("{id}.eml")), &sealed).unwrap();
        }
        let day_old = |id: &str| std::fs::File::options().write(true).open(base.join(format!("{id}.eml"))).unwrap().set_modified(std::time::SystemTime::now() - Duration::from_secs(25 * 3600)).unwrap();
        day_old("gone");
        day_old("kept");
        draft_with(&base, "kept", true);
        let refused = || -> Option<Box<dyn std::any::Any>> { None };
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut Judged::default(), &refused), Sealed::default());
        assert!(base.join("gone.eml").exists(), "without the lock: left");
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut Judged::default(), &unshared), Sealed { orphans: 1, ..Sealed::default() });
        assert!(!base.join("gone.eml").exists(), "a day old, its draft gone: taken out");
        assert!(base.join("fresh.eml").exists() && base.join("kept.eml").exists() && base.join("kept.toml").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nothing_takes_its_place_without_the_lock() {
        let base = scratch("unlocked");
        std::fs::write(base.join("a1.eml"), CLEAR).unwrap();
        draft_with(&base, "a1", true);
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        let mut judged = Judged::default();
        let refused = || -> Option<Box<dyn std::any::Any>> { None };
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut judged, &refused), Sealed { failed: 1, ..Sealed::default() });
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
        assert_eq!(seal_beside(&base, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut judged, &counted).copies, 1);
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

    /// A later process (a phone's background step, started for a moment)
    /// reads none of the copies met before and unchanged, and reads no key:
    /// the verdicts are kept on the disk. Review of 19 October 2026, F9: each
    /// fresh process read and parsed every copy kept in the history again (21
    /// to 54 ms for 150 of 100 KB), and read every key (18 ms for 300 others').
    #[test]
    fn a_later_process_reads_nothing_met_before_and_no_key() {
        let base = scratch("verdicts");
        let keys = keys_of(&pgp::stand_in_key("Noa <noa@example.org>").unwrap());
        let verdicts = base.join("state").join("draftcopies.json");
        let large = format!("{CLEAR}{}\r\n", "The lease runs three years. ".repeat(3_700));
        let versions: Vec<PathBuf> = (0..40)
            .map(|n| {
                let path = base.join("history").join(format!("2026-10-18T10-00-{n:02}.000Z"));
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, &large).unwrap();
                path
            })
            .collect();
        let mut judged = Judged::load(&verdicts);
        assert_eq!(seal_versions(&versions, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut judged, &unshared).versions, 40);
        judged.save(&verdicts).unwrap();
        // A later process: the verdicts read from the disk, no copy read, no key read.
        READ.with(|n| n.set(0));
        let mut later = Judged::load(&verdicts);
        let certs = Certs::lazy(|| -> Keys { panic!("no key read for a look with nothing to seal") });
        assert_eq!(seal_versions(&versions, &certs, &mut |_| Vec::new(), &mut later, &unshared), Sealed::default());
        assert_eq!(READ.with(|n| n.get()), 0, "nothing read again");
        assert!(!certs.read());
        // A copy that changed is read again, that one alone, and sealed.
        std::fs::write(&versions[7], CLEAR).unwrap();
        READ.with(|n| n.set(0));
        assert_eq!(seal_versions(&versions, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut later, &unshared).versions, 1);
        assert_eq!(READ.with(|n| n.get()), 1);
        // Saved, a file no longer met is forgotten; a verdicts' file that does not read is none.
        std::fs::remove_file(&versions[3]).unwrap();
        later.save(&verdicts).unwrap();
        let mut after = Judged::load(&verdicts);
        assert_eq!(seal_versions(&versions, &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut after, &unshared), Sealed::default());
        after.save(&verdicts).unwrap();
        let kept: JudgedFile = serde_json::from_slice(&std::fs::read(&verdicts).unwrap()).unwrap();
        assert_eq!(kept.files.len(), 39);
        std::fs::write(&verdicts, "not json").unwrap();
        assert!(Judged::load(&verdicts).files.is_empty());
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
        let done = seal_versions(&[clear.clone(), sealed.clone()], &Certs::of(keys.clone()), &mut |_| Vec::new(), &mut Judged::default(), &unshared);
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

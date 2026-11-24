// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What an exchange wrote that its memory does not hold yet: the journal,
//! `share/journal.jsonl` beside the memory, on this device only, never shared
//! (docs/database.md, "What an exchange wrote, noted first").
//!
//! An exchange writes the others' changes into the files here, appends this
//! device's own changes to its records, and saves its memory last. A device
//! that ends in between (the system ending the app, a crash, a full disk that
//! fails the save) kept its files and its records as written and its memory
//! as it was before. Until 24 November 2026 the next look compared the files
//! with that older memory: what came from another device was taken for a
//! change made here and sent again with a new clock, which could be later
//! than a change made elsewhere meanwhile, and so put an older version back
//! over a newer one (a note on 21 November 2026, its newer version kept
//! beside as a conflict); and this device's own change, already appended,
//! went again with a new clock too.
//!
//! Each change is now noted here, on the disk, before it is written: what it
//! holds once written (its hash, "" taken out), its clock and its writer. The
//! next look reads the lines its memory does not hold yet (`unsaved`): a file
//! here that holds what another device's change wrote is not a change of this
//! device's (that change is read again from the records, which the memory had
//! not moved past, and settles the memory); one that holds what this device
//! appended keeps that change's own clock. Anything else found is a change
//! made here, as before. Once the memory is saved, the lines it holds go.

use super::*;

/// The journal's format: a line of another is passed over.
pub(super) const JOURNAL_FORMAT: u32 = 1;

/// A journal larger than this is not read (it cannot be one: a line is a few
/// hundred bytes, one for each change an exchange writes).
const JOURNAL_LARGEST: u64 = 64 << 20;

/// One change noted before it is written: its key, its clock and writer, and
/// what the key holds once written (an entry's value's hash, a sealed file's
/// content's hash; "" taken out), as the next look reads it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) struct Landed {
    #[serde(default)]
    pub(super) v: u32,
    pub(super) k: String,
    pub(super) c: u64,
    pub(super) w: String,
    #[serde(default)]
    pub(super) h: String,
}

impl Landed {
    pub(super) fn new(key: &str, clock: u64, writer: &str, hash: String) -> Landed {
        Landed { v: JOURNAL_FORMAT, k: key.to_string(), c: clock, w: writer.to_string(), h: hash }
    }
}

/// Where the journal is: beside the memory.
pub(super) fn journal_path(memory: &Path) -> PathBuf {
    memory.with_file_name("journal.jsonl")
}

/// Whether the memory does not hold a line yet: no entry for its key, or one
/// earlier than the line's clock (and writer, as records are ordered) holding
/// something else. A line the memory passed (a later change, made here or
/// read) or holds is spent. Saved, the memory keeps the entries of files
/// sealed apart and of the logs of lines beside the others (`Sealed::entries`,
/// `Memory::prepare_save`): looked for in both.
fn unsaved(memory: &Memory, line: &Landed) -> bool {
    memory.entries.get(&line.k).or_else(|| memory.sealed.entries.get(&line.k)).is_none_or(|k| k.h != line.h && (line.c, line.w.as_str()) > (k.c, k.w.as_str()))
}

/// The journal as an exchange uses it.
pub(super) struct Journal {
    path: PathBuf,
    /// The lines the memory does not hold yet, by key: read at the start of
    /// the exchange, then those it noted itself.
    lines: BTreeMap<String, Vec<Landed>>,
    /// The lines on the disk, in their order, and the same as a set.
    on_disk: Vec<Landed>,
    known: std::collections::HashSet<Landed>,
}

impl Journal {
    /// The journal as the last exchange left it, its lines the memory holds
    /// left aside. A line not read whole (a write cut by the end of the
    /// process) or of another format is passed over.
    pub(super) fn read(memory_path: &Path, memory: &Memory) -> Journal {
        let path = journal_path(memory_path);
        let on_disk: Vec<Landed> = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() <= JOURNAL_LARGEST => std::fs::read(&path).map(|bytes| bytes.split(|b| *b == b'\n').filter_map(|line| serde_json::from_slice::<Landed>(line).ok()).filter(|l| l.v == JOURNAL_FORMAT).collect()).unwrap_or_default(),
            _ => Vec::new(),
        };
        let mut lines: BTreeMap<String, Vec<Landed>> = BTreeMap::new();
        for line in on_disk.iter().filter(|l| unsaved(memory, l)) {
            lines.entry(line.k.clone()).or_default().push(line.clone());
        }
        let known = on_disk.iter().cloned().collect();
        Journal { path, lines, on_disk, known }
    }

    /// Nothing of before counts (a memory started anew: another sharing): its
    /// lines are about another sharing's records.
    pub(super) fn forget(&mut self) {
        self.lines.clear();
        self.on_disk.clear();
        self.known.clear();
        let _ = std::fs::remove_file(&self.path);
    }

    /// Whether `key` holding `h` ("" taken out) here is another device's
    /// change written by an exchange whose memory was never saved.
    pub(super) fn theirs(&self, key: &str, h: &str, own: &str) -> bool {
        self.lines.get(key).is_some_and(|lines| lines.iter().any(|l| l.h == h && l.w != own))
    }

    /// The clock of this device's own change of `key` to `h` ("" taken out)
    /// appended by an exchange whose memory was never saved.
    pub(super) fn own(&self, key: &str, h: &str, own: &str) -> Option<u64> {
        self.lines.get(key)?.iter().filter(|l| l.h == h && l.w == own).map(|l| l.c).max()
    }

    /// Changes noted on the disk before they are written: appended, the bytes
    /// on the disk before this returns. Not noted (a full disk), they must not
    /// be written: the caller leaves them for the next exchange.
    pub(super) fn note(&mut self, landed: Vec<Landed>) -> Result<(), String> {
        let new: Vec<Landed> = landed.into_iter().filter(|l| !self.known.contains(l)).collect();
        if new.is_empty() {
            return Ok(());
        }
        let mut text: Vec<u8> = Vec::new();
        for line in &new {
            serde_json::to_writer(&mut text, line).map_err(|e| e.to_string())?;
            text.push(b'\n');
        }
        let fail = |e: std::io::Error| format!("{}: {e}", self.path.display());
        if let Some(parent) = self.path.parent() {
            private_dirs(parent).map_err(fail)?;
        }
        // A line left half written (the process ended while it wrote): cut back
        // to the last whole line, so that the next one is not glued to it.
        if let Some(whole) = unfinished(&self.path) {
            std::fs::OpenOptions::new().write(true).open(&self.path).and_then(|file| file.set_len(whole)).map_err(fail)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&self.path).map_err(fail)?;
        file.write_all(&text).map_err(fail)?;
        file.sync_data().map_err(fail)?;
        for line in new {
            self.known.insert(line.clone());
            self.on_disk.push(line.clone());
            self.lines.entry(line.k.clone()).or_default().push(line);
        }
        Ok(())
    }

    /// Once the memory is saved: the lines it does not hold yet stay (a change
    /// noted and left waiting, its note or paper not written this time), the
    /// others go; none left, the file goes. Rewritten only when that changes
    /// what it holds. A failure here costs nothing: the lines left are spent
    /// ones, which the next exchange passes over.
    pub(super) fn settle(self, memory: &Memory) {
        let mut seen = std::collections::HashSet::new();
        let keep: Vec<Landed> = self.on_disk.iter().filter(|l| unsaved(memory, l) && seen.insert(*l)).cloned().collect();
        if keep.is_empty() {
            if !self.on_disk.is_empty() || self.path.exists() {
                let _ = std::fs::remove_file(&self.path);
            }
            return;
        }
        if keep == self.on_disk {
            return;
        }
        let mut text: Vec<u8> = Vec::new();
        for line in &keep {
            if serde_json::to_writer(&mut text, line).is_err() {
                return;
            }
            text.push(b'\n');
        }
        let _ = write_synced(&self.path, &text);
    }
}

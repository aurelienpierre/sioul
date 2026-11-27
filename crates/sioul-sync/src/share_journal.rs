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
//! holds once written (its hash, "" taken out), its clock and its writer; and
//! marked done once written (`d`, a line of its own, not waited for on the
//! disk). The next look reads the lines its memory does not hold yet
//! (`unsaved`), and for each key the last one marked done counts, and so do
//! the changes noted after it whose content the file holds exactly (written,
//! the process ended before the mark, or the mark lost to a power cut: the
//! write happened, or the person wrote exactly that, and either way the file
//! holds that change): a file here that holds what another device's change
//! wrote is not a change of this device's (that change is read again from
//! the records, which the memory had not moved past, and settles the
//! memory); one that holds what this device appended keeps that change's
//! own clock. Anything else found is a change made here, as before: a value
//! put back to what a line before the last done one held, after two ended
//! exchanges, among them. Once the memory is saved, the lines it holds go.
//!
//! Format 2 since 27 November 2026 (the marks). A line of format 1, which
//! the builds before it wrote without marks, is read as noted and never
//! marked: it counts where the file holds exactly what it wrote, which was
//! its meaning there (every line of a key counted by its content), so a note
//! received by such a build just before the update (an install ends the
//! app) never goes back as this device's own change, the very fault the
//! journal mends (the third review of 26 November 2026, A1). Read as done
//! instead, the last of two such lines hid the first, and a file still
//! holding the first went back over the newer version (the fourth review,
//! F2). A build of format 1 passes format 2 over, as a build before the
//! journal did for one exchange.
//!
//! The logs of lines (`calls_store`: calls, the phone's messages, texts, the
//! notices' words) are left out: a line there is its own name, written once
//! by its device and aged out days later, so one received and sent again is
//! the same line at worst, and noting each would make a join's journal
//! megabytes.

#[allow(unused_imports)]
use crate::files::Probe as _;
use super::*;

/// The journal's format: 2, its changes marked done once written (`Landed::d`);
/// a line of format 1 (no marks) is read as noted, never marked; a line of another is passed over.
pub(super) const JOURNAL_FORMAT: u32 = 2;

/// The journal's format before the marks: each line read as noted, never marked.
const JOURNAL_UNMARKED: u32 = 1;

/// A journal larger than this is not read (it cannot be one: a line is a few
/// hundred bytes, two for each change an exchange writes).
const JOURNAL_LARGEST: u64 = 64 << 20;

/// One change noted before it is written: its key, its clock and writer, and
/// what the key holds once written (an entry's value's hash, a sealed file's
/// content's hash; "" taken out), as the next look reads it. `d`: the same
/// change marked done, once written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) struct Landed {
    #[serde(default)]
    pub(super) v: u32,
    pub(super) k: String,
    pub(super) c: u64,
    pub(super) w: String,
    #[serde(default)]
    pub(super) h: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) d: bool,
}

impl Landed {
    pub(super) fn new(key: &str, clock: u64, writer: &str, hash: String) -> Landed {
        Landed { v: JOURNAL_FORMAT, k: key.to_string(), c: clock, w: writer.to_string(), h: hash, d: false }
    }

    /// The same change, marked done.
    fn done(&self) -> Landed {
        Landed { d: true, ..self.clone() }
    }
}

/// Whether a key's changes go through the journal: not the logs of lines (see the module's words).
pub(super) fn journaled(key: &str) -> bool {
    !calls_store(key)
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
    /// For each key, the last change marked done that the memory does not
    /// hold yet, and the changes noted after it, never marked, in their
    /// order, as read at the start of the exchange: the ones that count, the
    /// later ones first among those whose content the file holds.
    counts: BTreeMap<String, Vec<Landed>>,
    /// The clocks of the lines the memory does not hold yet, the highest.
    clock: u64,
    /// The lines on the disk, in their order (changes and their marks), and
    /// the same as a set.
    on_disk: Vec<Landed>,
    known: std::collections::HashSet<Landed>,
}

impl Journal {
    /// The journal as the last exchange left it, its lines the memory holds
    /// left aside. A line not read whole (a write cut by the end of the
    /// process) or of another format is passed over; a line of format 1 is
    /// read as noted, never marked (`JOURNAL_UNMARKED`). Of a key's changes, the last done
    /// one counts, and those noted after it never marked done count only
    /// where the file holds exactly what they wrote (`theirs`, `own`).
    pub(super) fn read(memory_path: &Path, memory: &Memory) -> Journal {
        let path = journal_path(memory_path);
        let on_disk: Vec<Landed> = match crate::files::metadata(&path) {
            Ok(meta) if meta.len() <= JOURNAL_LARGEST => crate::files::read(&path)
                .map(|bytes| {
                    bytes
                        .split(|b| *b == b'\n')
                        .filter_map(|line| serde_json::from_slice::<Landed>(line).ok())
                        .filter_map(|l| match l.v {
                            JOURNAL_FORMAT => Some(l),
                            JOURNAL_UNMARKED => Some(Landed { d: false, ..l }),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let mut counts: BTreeMap<String, Vec<Landed>> = BTreeMap::new();
        let mut clock = 0;
        for line in on_disk.iter().filter(|l| unsaved(memory, l)) {
            clock = clock.max(line.c);
            let plain = Landed { d: false, v: JOURNAL_FORMAT, ..line.clone() };
            let key = counts.entry(line.k.clone()).or_default();
            if line.d {
                // A key's last done, in the order they were written: the lines before it no longer count.
                key.clear();
                key.push(plain);
            } else if !key.contains(&plain) {
                key.push(plain);
            }
        }
        let known = on_disk.iter().cloned().collect();
        Journal { path, counts, clock, on_disk, known }
    }

    /// The highest clock of the lines the memory does not hold yet: the
    /// memory's clock is never behind it, so that no change made here after
    /// is given a clock a change already went with.
    pub(super) fn clock(&self) -> u64 {
        self.clock
    }

    /// Nothing of before counts (a memory started anew: another sharing, or
    /// one not joined yet): its lines are about other records.
    pub(super) fn forget(&mut self) {
        self.counts.clear();
        self.clock = 0;
        self.on_disk.clear();
        self.known.clear();
        let _ = crate::files::remove_file(&self.path);
    }

    /// The change of `key` that counts for a file holding `h` ("" taken
    /// out): the latest of its last done change and those noted after it
    /// that holds exactly `h`.
    fn holding(&self, key: &str, h: &str) -> Option<&Landed> {
        self.counts.get(key).and_then(|lines| lines.iter().rev().find(|l| l.h == h))
    }

    /// Whether `key` holding `h` ("" taken out) here is another device's
    /// change written by an exchange whose memory was never saved (`holding`).
    pub(super) fn theirs(&self, key: &str, h: &str, own: &str) -> bool {
        self.holding(key, h).is_some_and(|l| l.w != own)
    }

    /// The clock of this device's own change of `key` to `h` ("" taken out)
    /// appended by an exchange whose memory was never saved (`holding`).
    pub(super) fn own(&self, key: &str, h: &str, own: &str) -> Option<u64> {
        self.holding(key, h).filter(|l| l.w == own).map(|l| l.c)
    }

    /// Changes noted on the disk before they are written: appended, the bytes
    /// on the disk before this returns. Not noted (a full disk), they must not
    /// be written: the caller leaves them for the next exchange.
    pub(super) fn note(&mut self, landed: Vec<Landed>) -> Result<(), String> {
        self.append(landed.into_iter().filter(|l| journaled(&l.k)).collect(), true)
    }

    /// Changes written: marked done, appended without waiting for the disk (a
    /// process that ends leaves them there; a power cut may lose them, and the
    /// change then counts as not written, as before the journal). A failure
    /// costs the same.
    pub(super) fn done(&mut self, landed: &[Landed]) {
        let _ = self.append(landed.iter().filter(|l| journaled(&l.k)).map(Landed::done).collect(), false);
    }

    fn append(&mut self, landed: Vec<Landed>, synced: bool) -> Result<(), String> {
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
            crate::files::OpenOptions::new().write(true).open(&self.path).and_then(|file| file.set_len(whole)).map_err(fail)?;
        }
        let mut options = crate::files::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&self.path).map_err(fail)?;
        file.write_all(&text).map_err(fail)?;
        if synced {
            file.sync_data().map_err(fail)?;
        }
        for line in new {
            self.known.insert(line.clone());
            self.on_disk.push(line);
        }
        Ok(())
    }

    /// Once the memory is saved: the lines it does not hold yet stay (a change
    /// noted and left waiting, its note or paper not written this time, with
    /// its mark if it has one), the others go; none left, the file goes.
    /// Rewritten only when that changes what it holds. A failure here costs
    /// nothing: the lines left are spent ones, which the next exchange passes
    /// over.
    pub(super) fn settle(self, memory: &Memory) {
        let mut seen = std::collections::HashSet::new();
        let keep: Vec<Landed> = self.on_disk.iter().filter(|l| unsaved(memory, l) && seen.insert(*l)).cloned().collect();
        if keep.is_empty() {
            if !self.on_disk.is_empty() || self.path.present() {
                let _ = crate::files::remove_file(&self.path);
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

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul store check [--repair]` (docs/store.md, "The check"): every piece
//! of every file, of its twin and of its earlier versions opened, its sealed
//! bytes compared with what its folder's list says; per folder, what is
//! whole, how many good copies each file has, what was mended and from
//! what, what is lost here. With `--repair`, under the profile's lock held
//! exclusive (writers wait): a damaged list made again from the blobs, a
//! damaged current copy made again from its twin or its newest good version,
//! damaged copies set aside (never one that opens), a blob newer than its
//! list taken as the file's content, and blobs no list names taken out.

use super::format::{Fault, Role};
use super::lock::{self, ProfileLock};
use super::manifest::{Folder, Found, Loaded, Op, Rec};
use super::{Error, LogicalPath, Root, Store, disk, format, lines};
use std::collections::{BTreeMap, BTreeSet};

/// What the check found, folder by folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub folders: Vec<FolderReport>,
    /// Blobs no list names (an interrupted write's, a purge's leftovers).
    pub unlisted: usize,
    /// Files with a blob newer than their list names (`--repair` takes it).
    pub newer: Vec<String>,
    pub repaired: bool,
}

/// What a file was mended from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MendedFrom {
    /// Its second copy.
    SecondCopy,
    /// Its newest earlier version that opens, written then.
    Earlier { written_ms: i64 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderReport {
    pub folder: String,
    pub files: usize,
    /// Files whose every copy opened whole.
    pub whole: usize,
    /// How many files have so many good copies (current, twin and earlier versions).
    pub copies: BTreeMap<usize, usize>,
    /// Files with a copy that did not open: their name, good copies, copies kept.
    pub thin: Vec<(String, usize, usize)>,
    /// Files mended, and from what.
    pub mended: Vec<(String, MendedFrom)>,
    /// Files of which no copy opens here.
    pub lost: Vec<String>,
    /// Lines that do not open or are gone, in files that only grow.
    pub lost_lines: usize,
    /// Its list did not open (made again from the blobs with `--repair`).
    pub list_damaged: bool,
}

impl Store {
    /// The whole store checked; mended with `repair`.
    pub fn check(&self, repair: bool) -> Result<Report, Error> {
        let _profile = if repair { ProfileLock::exclusive(&self.folders)? } else { ProfileLock::shared(&self.folders)? };
        let mut report = Report { repaired: repair, ..Report::default() };
        for root in Root::ALL {
            let (found, folders, mut named) = self.survey(root)?;
            for folder in folders {
                let folder_report = self.check_folder(&folder, repair, &found, &mut named, &mut report.newer)?;
                report.folders.push(folder_report);
            }
            // Blobs that open with this key and that no list names.
            let sealed = self.folders.sealed(root);
            for f in &found {
                if named.contains(&f.name) {
                    continue;
                }
                report.unlisted += 1;
                if repair {
                    let _ = disk::remove(&format::place(&sealed, &f.name));
                }
            }
        }
        Ok(report)
    }

    fn check_folder(&self, folder: &LogicalPath, repair: bool, found: &[Found], named: &mut BTreeSet<String>, newer: &mut Vec<String>) -> Result<FolderReport, Error> {
        let mut out = FolderReport { folder: folder.to_string(), ..FolderReport::default() };
        let _held = lock::take_folder(&self.folder_lock(folder), repair, false)?;
        let mut state = match self.load(folder, repair)? {
            Loaded::Ready(state) => state,
            Loaded::NeedsRepair => {
                out.list_damaged = true;
                return Ok(out);
            }
        };
        for copy in [0u8, 1] {
            named.insert(self.name(Role::Manifest, folder.as_str(), 0, copy));
        }
        named.insert(self.name(Role::Journal, folder.as_str(), 0, 0));
        // A blob newer than the list names: an interrupted write, or a list put back older.
        for (name, generation) in Store::newer_than_list(folder, &state, found) {
            newer.push(format!("{folder}/{name}"));
            if repair {
                self.adopt(folder, &mut state, &name, generation)?;
            } else {
                // Not taken: named, so that it is not counted among the leftovers (whole, or a file that only grows, and its second copy).
                let path = format!("{folder}/{name}");
                for role in [Role::File, Role::Lines] {
                    for copy in [0u8, 1] {
                        named.insert(self.name(role, &path, generation, copy));
                    }
                }
            }
        }
        let names: Vec<String> = state.entries.keys().cloned().collect();
        for name in names {
            let Some(rec) = state.entries.get(&name).cloned() else { continue };
            let path = folder.join(&name)?;
            if rec.k == 1 {
                self.check_lines(&path, &rec, named, &mut out);
                continue;
            }
            self.check_file(folder, &mut state, &path, &name, rec, repair, named, &mut out)?;
        }
        Ok(out)
    }

    fn check_lines(&self, path: &LogicalPath, rec: &Rec, named: &mut BTreeSet<String>, out: &mut FolderReport) {
        for copy in [0u8, 1] {
            named.insert(self.name(Role::Lines, path.as_str(), rec.g, copy));
        }
        if rec.rm {
            return;
        }
        out.files += 1;
        let copies: u8 = if rec.tw { 2 } else { 1 };
        let mut good = 0usize;
        for copy in 0..copies {
            let bind = self.name(Role::Lines, path.as_str(), rec.g, copy);
            if lines::scan(&self.blob_place(path, Role::Lines, rec.g, copy), &self.keys.lines, &|_, _| bind.clone(), Some((Role::Lines, rec.g))).is_ok() {
                good += 1;
            }
        }
        match self.lines_of_pub(path, rec) {
            Ok(damaged) => {
                out.lost_lines += damaged;
                if damaged == 0 && good == usize::from(copies) {
                    out.whole += 1;
                }
                *out.copies.entry(good).or_default() += 1;
            }
            Err(_) => out.lost.push(path.file_name().to_string()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_file(&self, folder: &LogicalPath, state: &mut Folder, path: &LogicalPath, name: &str, mut rec: Rec, repair: bool, named: &mut BTreeSet<String>, out: &mut FolderReport) -> Result<(), Error> {
        // Every copy: (generation, copy, whether it opens, whether its sealed bytes are the ones the list says).
        let mut copies: Vec<(u64, u8, bool, bool)> = Vec::new();
        if !rec.rm {
            copies.push((rec.g, 0, false, false));
            if rec.tw {
                copies.push((rec.g, 1, false, false));
            }
        }
        copies.extend(rec.v.iter().map(|v| (v.g, 0, false, false)));
        for (generation, copy, opens, as_said) in &mut copies {
            named.insert(self.name(Role::File, path.as_str(), *generation, *copy));
            match self.open_blob(path, *generation, *copy, &mut std::io::sink()) {
                Ok(got) => {
                    *opens = true;
                    let said = rec.hash_of(*generation, *copy);
                    *as_said = said.is_empty() || said == got.hash;
                }
                Err(Fault::Io(e)) => return Err(Error::Io(format!("{path}: {e}"))),
                Err(_) => {}
            }
        }
        if rec.rm {
            // A removed file: its versions alone, kept for an undo; one that does not open goes aside.
            if repair && copies.iter().any(|(_, _, opens, _)| !*opens) {
                for (g, copy, _, _) in copies.iter().filter(|(_, _, opens, _)| !*opens) {
                    self.set_aside_copy(path, *g, *copy);
                }
                rec.v.retain(|v| copies.iter().any(|(g, _, opens, _)| *g == v.g && *opens));
                let op = if rec.v.is_empty() { Op::D(name.to_string()) } else { Op::P(name.to_string(), rec) };
                self.record(folder, state, op)?;
            }
            return Ok(());
        }
        out.files += 1;
        let good = copies.iter().filter(|(_, _, opens, _)| *opens).count();
        *out.copies.entry(good).or_default() += 1;
        let all_as_said = copies.iter().all(|(_, _, _, as_said)| *as_said);
        if good == copies.len() && all_as_said && !rec.d.contains(&rec.g) {
            out.whole += 1;
            return Ok(());
        }
        if good < copies.len() {
            out.thin.push((name.to_string(), good, copies.len()));
        }
        let current_good = copies.iter().any(|(g, copy, opens, _)| *g == rec.g && *copy == 0 && *opens);
        let twin_good = copies.iter().any(|(g, copy, opens, _)| *g == rec.g && *copy == 1 && *opens);
        if !repair {
            if good == 0 {
                out.lost.push(name.to_string());
            }
            if good == copies.len() {
                out.whole += 1;
            }
            return Ok(());
        }
        // Mending: copies that do not open set aside (one that opens never is), the list's hashes
        // brought to what opens, the current copy made again from its twin or its newest good version.
        for (g, copy, _, _) in copies.iter().filter(|(_, _, opens, _)| !*opens) {
            self.set_aside_copy(path, *g, *copy);
        }
        let mut mended = rec.clone();
        mended.v.retain(|v| copies.iter().any(|(g, copy, opens, _)| *g == v.g && *copy == 0 && *opens));
        mended.tw = rec.tw && twin_good;
        if !mended.tw {
            mended.th.clear();
            mended.tz = 0;
        }
        self.refresh_hashes(path, &mut mended);
        if current_good {
            mended.d.retain(|g| *g != rec.g);
            if mended != rec {
                self.record(folder, state, Op::P(name.to_string(), mended))?;
            }
            if good == copies.len() {
                out.whole += 1;
            }
        } else if twin_good {
            let mut bytes = Vec::new();
            if let Ok(got) = self.open_blob(path, rec.g, 1, &mut bytes) {
                let made = self.reseal(path, rec.g, 0, got.inner.compressed, &bytes)?;
                mended.h = made.hash;
                mended.z = made.sealed_size;
                mended.d.retain(|g| *g != rec.g);
                self.record(folder, state, Op::P(name.to_string(), mended))?;
                out.mended.push((name.to_string(), MendedFrom::SecondCopy));
            }
        } else if let Some(version) = mended.v.first().cloned() {
            let mut bytes = Vec::new();
            if self.open_blob(path, version.g, 0, &mut bytes).is_ok() {
                // The newest good version put back as a new generation; the damaged one named as such.
                mended.d = vec![rec.g];
                state.entries.insert(name.to_string(), mended);
                let written = self.restore_locked(path, state, version.g, &bytes)?;
                named.insert(self.name(Role::File, path.as_str(), written.generation, 0));
                if let Some(now) = state.entries.get(name) {
                    named.extend(now.v.iter().map(|v| self.name(Role::File, path.as_str(), v.g, 0)));
                }
                out.mended.push((name.to_string(), MendedFrom::Earlier { written_ms: version.t }));
            }
        } else {
            mended.d = vec![rec.g];
            self.record(folder, state, Op::P(name.to_string(), mended))?;
            out.lost.push(name.to_string());
        }
        Ok(())
    }

    /// A record's hashes brought to what its copies that open now are: a copy that opens is never set aside.
    fn refresh_hashes(&self, path: &LogicalPath, rec: &mut Rec) {
        let mut hashes: Vec<(u64, u8, String, u64)> = Vec::new();
        let mut copies = vec![(rec.g, 0u8)];
        if rec.tw {
            copies.push((rec.g, 1));
        }
        copies.extend(rec.v.iter().map(|v| (v.g, 0)));
        for (generation, copy) in copies {
            if let Ok(got) = self.open_blob(path, generation, copy, &mut std::io::sink()) {
                hashes.push((generation, copy, got.hash, got.sealed_size));
            }
        }
        for (generation, copy, hash, size) in hashes {
            match copy {
                1 => {
                    rec.th = hash;
                    rec.tz = size;
                }
                _ if generation == rec.g => {
                    rec.h = hash;
                    rec.z = size;
                }
                _ => {
                    if let Some(v) = rec.v.iter_mut().find(|v| v.g == generation) {
                        v.h = hash;
                        v.z = size;
                    }
                }
            }
        }
    }

    /// How many lines of a file that only grows are lost (do not open, or are gone).
    fn lines_of_pub(&self, path: &LogicalPath, rec: &Rec) -> Result<usize, Error> {
        self.lines_of(path, rec).map(|(_, damaged)| damaged.len())
    }

    /// A copy set aside (the check's `--repair`), its time set aside then.
    fn set_aside_copy(&self, path: &LogicalPath, generation: u64, copy: u8) {
        let place = self.blob_place(path, Role::File, generation, copy);
        let target = self.folders.sealed(path.root()).join(".aside").join(self.name(Role::File, path.as_str(), generation, copy));
        if place.exists()
            && let Some(dir) = target.parent()
            && disk::private_dir(dir).is_ok()
            && disk::rename_over(&place, &target).is_ok()
        {
            let _ = std::fs::File::options().append(true).open(&target).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        }
    }

    /// A copy sealed again from good bytes, at its own name.
    fn reseal(&self, path: &LogicalPath, generation: u64, copy: u8, compressed: bool, bytes: &[u8]) -> Result<format::Made, Error> {
        let name = self.name(Role::File, path.as_str(), generation, copy);
        let header = format::Header::new(Role::File, copy, generation);
        let inner = format::Inner { path: path.to_string(), written_ms: super::now_ms(), origin: self.origin.clone(), compressed };
        disk::place(&self.blob_place(path, Role::File, generation, copy), |out| format::seal(out, &self.keys.sealing, &name, &header, &inner, &mut &bytes[..])).map_err(|e| Error::Io(format!("{path}: {e}")))
    }
}

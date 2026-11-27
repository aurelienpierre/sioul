// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The encrypted manifests (docs/store.md, "Manifests"): one per logical
//! folder says which blob is which file (its generation, sizes, the hash of
//! its sealed bytes, its twin, its earlier versions, its attributes); a
//! sealed root manifest per `sealed/` folder lists the folders.
//!
//! A change appends one sealed line to its folder's journal (the moment it
//! is made), under the folder's lock; past `FOLD_LINES` lines or
//! `FOLD_BYTES` the journal is folded into a new manifest. A manifest is
//! written twice (its first copy, then its second), then the journal begun again, so
//! that a power cut at any step leaves a manifest and a journal that agree:
//! the journal's lines are bound to the manifest's generation, and a
//! journal older than its manifest was folded in already.
//!
//! **Rebuilt from the blobs** when no copy of a manifest opens, or when it is
//! older than its journal, or a line of its journal is lost: each blob opens
//! by its own name and gives its path and generation (piece 0); what the
//! manifests and journals still hold (attributes, removals) is laid over
//! it; a file that its manifest named and no blob holds stays named,
//! damaged, never absent. Said once (`Notice::Rebuilt`).

use super::format::{self, Expected, Fault, Header, Inner, Role};
use super::{Error, LogicalPath, Notice, Root, Store, disk, lines};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A journal is folded into its manifest past so many lines,
pub(crate) const FOLD_LINES: u64 = 512;
/// or so many bytes.
pub(crate) const FOLD_BYTES: u64 = 256 << 10;
/// The most a manifest may inflate to (a folder of 100 000 files with long names).
const LARGEST_MANIFEST: u64 = 256 << 20;

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

fn is_zero64(n: &u64) -> bool {
    *n == 0
}

/// A file in its folder's manifest (short field names: the manifest of a
/// folder of 3 700 messages is read at every listing).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Rec {
    /// Its current generation (a removed file's last one).
    pub g: u64,
    #[serde(default)]
    pub s: u64,
    #[serde(default)]
    pub z: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub h: String,
    #[serde(default)]
    pub t: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub o: String,
    /// 0: a whole file; 1: a file that only grows.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub k: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub tw: bool,
    /// Removed: its versions alone are kept (a removal undone by `restore`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub rm: bool,
    /// Earlier versions, the newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub v: Vec<Ver>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub a: BTreeMap<String, String>,
    /// Generations found damaged (their blob does not open).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub d: Vec<u64>,
    /// The second copy's sealed hash and size: sealed apart, its bytes are not the first's.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub th: String,
    #[serde(default, skip_serializing_if = "is_zero64")]
    pub tz: u64,
    /// Never versioned (a message: its source is its backup): a removal is final.
    #[serde(default, skip_serializing_if = "is_false")]
    pub nv: bool,
    /// Not compressed (what an outsider can drive the size of): kept so when it is sealed again.
    #[serde(default, skip_serializing_if = "is_false")]
    pub nc: bool,
    /// A file that only grows: the number of its last line, so that a cut at a line's end is seen.
    #[serde(default, skip_serializing_if = "is_zero64")]
    pub lc: u64,
    /// Fields a later Sioul added, carried through unread: a fold by this one never drops them (docs/store.md, "Formats").
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

/// An earlier version.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Ver {
    pub g: u64,
    #[serde(default)]
    pub s: u64,
    #[serde(default)]
    pub z: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub h: String,
    #[serde(default)]
    pub t: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub o: String,
    /// Fields a later Sioul added, carried through unread.
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

impl Rec {
    /// The current copy as an earlier version.
    pub fn as_version(&self) -> Ver {
        Ver { g: self.g, s: self.s, z: self.z, h: self.h.clone(), t: self.t, o: self.o.clone(), rest: BTreeMap::new() }
    }

    /// The highest generation it names.
    pub fn top(&self) -> u64 {
        self.v.iter().map(|v| v.g).chain(self.d.iter().copied()).fold(self.g, u64::max)
    }

    /// The sealed hash a copy must have: the first copy's, the second's, or a version's; empty when unknown.
    pub fn hash_of(&self, generation: u64, copy: u8) -> &str {
        match copy {
            1 if generation == self.g => &self.th,
            0 if generation == self.g => &self.h,
            _ => self.v.iter().find(|v| v.g == generation).map_or("", |v| v.h.as_str()),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Body {
    #[serde(default)]
    entries: BTreeMap<String, Rec>,
    /// The highest generation ever given in the folder: a generation never comes back,
    /// so that no old blob of a path removed or renamed opens as a new file there.
    #[serde(default)]
    top: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct RootBody {
    #[serde(default)]
    folders: BTreeSet<String>,
}

/// One journal line.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Op {
    /// A file's whole record, as it is now.
    P(String, Rec),
    /// A file gone with all its copies (a purge, a removal that kept nothing).
    D(String),
    /// A file renamed in its folder: the first name gone, the second as its record says.
    R(String, String, Rec),
}

impl Op {
    /// The change made to a folder's entries, and to its highest generation, which only grows.
    fn apply(self, entries: &mut BTreeMap<String, Rec>, top: &mut u64) {
        match self {
            Op::P(name, rec) => {
                *top = (*top).max(rec.top());
                entries.insert(name, rec);
            }
            Op::D(name) => {
                entries.remove(&name);
            }
            Op::R(from, to, rec) => {
                *top = (*top).max(rec.top());
                entries.remove(&from);
                entries.insert(to, rec);
            }
        }
    }
}

/// A journal's state on the disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JournalState {
    /// It extends the manifest: lines are appended to it.
    Current,
    /// None, or one older than the manifest: begun again at the next change.
    Again,
}

/// A folder's manifest and journal, read.
#[derive(Clone, Debug)]
pub(crate) struct Folder {
    /// The manifest's generation (0: none written yet).
    pub generation: u64,
    pub entries: BTreeMap<String, Rec>,
    pub journal: JournalState,
    /// Where the journal's valid part ends, how many lines it holds (line 0 included), how many changes.
    pub valid: u64,
    pub count: u64,
    pub changes: u64,
    /// Neither a manifest nor a journal: a folder never written.
    pub new: bool,
    /// The highest generation ever given in the folder (`Body::top`).
    pub top: u64,
}

impl Folder {
    fn empty() -> Folder {
        Folder { generation: 0, entries: BTreeMap::new(), journal: JournalState::Again, valid: 0, count: 0, changes: 0, new: true, top: 0 }
    }

    fn of(generation: u64, entries: BTreeMap<String, Rec>, top: u64) -> Folder {
        let top = entries.values().map(Rec::top).fold(top, u64::max);
        Folder { generation, entries, journal: JournalState::Again, valid: 0, count: 0, changes: 0, new: false, top }
    }

    /// The journal's lines applied, in order; none when a line does not open or read.
    fn replay(&mut self, lines: Vec<lines::Line>) -> Option<()> {
        for (_, line) in lines {
            let op = serde_json::from_slice::<Op>(&line?).ok()?;
            op.apply(&mut self.entries, &mut self.top);
            self.changes += 1;
        }
        Some(())
    }
}

/// A folder read, kept with what its files were on the disk then.
#[derive(Clone)]
pub(crate) struct Cached {
    folder: Folder,
    manifest: Option<(u64, i128, u64, i128)>,
    journal: Option<(u64, i128, u64, i128)>,
}

/// What a read found that a writer must mend: a rebuild, under the folder's lock held exclusive.
pub(crate) enum Loaded {
    Ready(Folder),
    NeedsRepair,
}

/// What the journal of `folder` is bound to: its folder, whatever its name on the disk.
fn journal_bind(folder: &LogicalPath) -> String {
    format!("journal\x1f{folder}")
}

impl Store {
    pub(crate) fn manifest_place(&self, folder: &LogicalPath, copy: u8) -> PathBuf {
        format::place(&self.folders.sealed(folder.root()), &self.name(Role::Manifest, folder.as_str(), 0, copy))
    }

    pub(crate) fn journal_place(&self, folder: &LogicalPath) -> PathBuf {
        format::place(&self.folders.sealed(folder.root()), &self.name(Role::Journal, folder.as_str(), 0, 0))
    }

    pub(crate) fn folder_lock(&self, folder: &LogicalPath) -> PathBuf {
        self.folders.sealed(folder.root()).join(".locks").join(self.name(Role::Lock, folder.as_str(), 0, 0))
    }

    /// The lock of one file reached by its path (`files::lock_place`): opaque, beside the folders' locks.
    pub(crate) fn file_lock(&self, path: &LogicalPath) -> PathBuf {
        self.folders.sealed(path.root()).join(".locks").join(self.name(Role::Lock, &format!("file\x1f{path}"), 0, 0))
    }

    /// The lock a handle takes on a sealed file itself (`files::File::lock`): its own,
    /// never `file_lock`'s, which stands for the lock file beside it (`filelock::with_lock`),
    /// as on the system a file and its lock file are two files.
    pub(crate) fn handle_lock(&self, path: &LogicalPath) -> PathBuf {
        self.folders.sealed(path.root()).join(".locks").join(self.name(Role::Lock, &format!("handle\x1f{path}"), 0, 0))
    }

    fn root_lock(&self, root_name: &str, root: Root) -> PathBuf {
        self.folders.sealed(root).join(".locks").join(self.name(Role::Lock, &format!("root\x1f{root_name}"), 0, 0))
    }

    /// A manifest's copy opened: its generation, its entries and its highest generation; `Missing` when not there.
    fn read_manifest(&self, folder: &LogicalPath, copy: u8) -> Result<(u64, BTreeMap<String, Rec>, u64), Fault> {
        let place = self.manifest_place(folder, copy);
        let name = self.name(Role::Manifest, folder.as_str(), 0, copy);
        let mut file = std::fs::File::open(&place).map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { Fault::Missing } else { Fault::Io(e) })?;
        let mut bytes = Vec::new();
        let got = format::open(&mut file, &self.keys.sealing, &name, &Expected { role: Role::Manifest, copy, generation: None, path: Some(folder.as_str()) }, &mut bytes, LARGEST_MANIFEST)?;
        let body: Body = serde_json::from_slice(&bytes).map_err(|_| Fault::Broken("a manifest that does not read"))?;
        Ok((got.header.generation, body.entries, body.top))
    }

    /// A folder's state, read from its manifest and journal (or from what this
    /// process read before, when neither changed but by lines added), under
    /// its lock. Held shared, a folder to mend is `NeedsRepair`; held
    /// exclusive, it is mended here and written again.
    pub(crate) fn load(&self, folder: &LogicalPath, exclusive: bool) -> Result<Loaded, Error> {
        let manifest_id = disk::identity(&self.manifest_place(folder, 0));
        let journal_place = self.journal_place(folder);
        let journal_id = disk::identity(&journal_place);
        let cached = self.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(folder.as_str()).cloned();
        if let Some(mut cached) = cached
            && cached.manifest == manifest_id
        {
            if cached.journal == journal_id {
                return Ok(Loaded::Ready(cached.folder));
            }
            // The same journal, grown: its new lines alone.
            let same_file = matches!((cached.journal, journal_id), (Some(a), Some(b)) if a.2 == b.2 && a.2 != 0 && b.0 >= cached.folder.valid);
            if same_file && cached.folder.journal == JournalState::Current {
                let bind = journal_bind(folder);
                if let Ok((added, valid, next)) = lines::scan_tail(&journal_place, cached.folder.valid, &self.keys.lines, &bind, cached.folder.generation) {
                    if cached.folder.replay(added).is_none() {
                        return self.load_whole(folder, exclusive);
                    }
                    cached.folder.valid = valid;
                    cached.folder.count = cached.folder.count.max(next);
                    self.keep(folder, &cached.folder);
                    return Ok(Loaded::Ready(cached.folder));
                }
            }
        }
        self.load_whole(folder, exclusive)
    }

    /// What was read kept for the next call, with the files' identities now.
    pub(crate) fn keep(&self, folder: &LogicalPath, state: &Folder) {
        let cached = Cached { folder: state.clone(), manifest: disk::identity(&self.manifest_place(folder, 0)), journal: disk::identity(&self.journal_place(folder)) };
        self.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(folder.as_str().to_string(), cached);
    }

    fn load_whole(&self, folder: &LogicalPath, exclusive: bool) -> Result<Loaded, Error> {
        let first = match self.read_manifest(folder, 0) {
            Ok(read) => Some(read),
            Err(Fault::Io(e)) => return Err(Error::Io(format!("{folder}: {e}"))),
            Err(Fault::Missing | Fault::Broken(_)) => None,
        };
        let journal_place = self.journal_place(folder);
        let bind = journal_bind(folder);
        let scanned = match lines::scan(&journal_place, &self.keys.lines, &|_, _| bind.clone(), None) {
            // A first line that does not read: its lines read all the same, bound to the generation of the manifest they follow (0 without one).
            Err(Fault::Broken(_)) => {
                let expected = match &first {
                    Some((g, _, _)) => *g,
                    None => self.read_manifest(folder, 1).map_or(0, |(g, _, _)| g),
                };
                lines::scan(&journal_place, &self.keys.lines, &|_, _| bind.clone(), Some((Role::Journal, expected)))
            }
            other => other,
        };
        let journal = match scanned {
            // Its lines are bound to its folder: a head that does not open stops nothing.
            Ok(scanned) if scanned.role == Role::Journal && scanned.head.as_ref().is_none_or(|head| head.path == folder.as_str()) => Some(scanned),
            Ok(_) | Err(Fault::Missing | Fault::Broken(_)) => None,
            Err(Fault::Io(e)) => return Err(Error::Io(format!("{folder}: {e}"))),
        };
        let journal_generation = journal.as_ref().map(|j| j.generation);
        // The manifest that goes with the journal: its first copy, else its second.
        let base = match (&first, journal_generation) {
            (Some((g, _, _)), Some(jg)) if *g >= jg => first.clone(),
            (Some(_), None) => first.clone(),
            _ => match self.read_manifest(folder, 1) {
                Ok((g, entries, top)) if journal_generation.is_none_or(|jg| g >= jg) => Some((g, entries, top)),
                Ok(_) | Err(Fault::Missing | Fault::Broken(_)) => None,
                Err(Fault::Io(e)) => return Err(Error::Io(format!("{folder}: {e}"))),
            },
        };
        let manifest_there = self.manifest_place(folder, 0).exists() || self.manifest_place(folder, 1).exists();
        let mut state = match (base, journal) {
            (Some((generation, entries, top)), Some(journal)) if journal.generation == generation => {
                let mut state = Folder { journal: JournalState::Current, valid: journal.valid, count: journal.next, ..Folder::of(generation, entries, top) };
                if state.replay(journal.lines).is_none() {
                    return self.needs_repair(folder, exclusive);
                }
                state
            }
            // A journal older than its manifest: folded in already.
            (Some((generation, entries, top)), Some(_)) => Folder::of(generation, entries, top),
            // A manifest without its journal: lost lines would show as newer blobs than it names.
            (Some((generation, entries, top)), None) => {
                let state = Folder::of(generation, entries, top);
                if journal_place.exists() || self.newer_blobs(folder, &state) {
                    return self.needs_repair(folder, exclusive);
                }
                state
            }
            // No manifest yet: the journal says everything since the folder was made.
            (None, Some(journal)) if journal.generation == 0 && !manifest_there => {
                let mut state = Folder { journal: JournalState::Current, valid: journal.valid, count: journal.next, ..Folder::of(0, BTreeMap::new(), 0) };
                if state.replay(journal.lines).is_none() {
                    return self.needs_repair(folder, exclusive);
                }
                state
            }
            (None, None) if !manifest_there && !journal_place.exists() && !self.root_lists(folder)? => Folder::empty(),
            _ => return self.needs_repair(folder, exclusive),
        };
        if state.journal == JournalState::Again {
            state.valid = 0;
            state.count = 0;
        }
        self.keep(folder, &state);
        Ok(Loaded::Ready(state))
    }

    fn needs_repair(&self, folder: &LogicalPath, exclusive: bool) -> Result<Loaded, Error> {
        if !exclusive {
            return Ok(Loaded::NeedsRepair);
        }
        let state = self.rebuild(folder)?;
        Ok(Loaded::Ready(state))
    }

    /// Whether a file of the folder has a blob of the next generations the folder would give.
    fn newer_blobs(&self, folder: &LogicalPath, state: &Folder) -> bool {
        let sealed = self.folders.sealed(folder.root());
        state.entries.iter().any(|(name, rec)| {
            let path = format!("{folder}/{name}");
            let role = if rec.k == 1 { Role::Lines } else { Role::File };
            (state.top + 1..=state.top + 2).any(|g| format::place(&sealed, &self.name(role, &path, g, 0)).exists())
        })
    }

    /// The change `op` made in the folder: one sealed line in its journal,
    /// flushed. The journal is begun again first where it must be, and
    /// folded once it is long. The caller holds the folder's lock exclusive.
    pub(crate) fn record(&self, folder: &LogicalPath, state: &mut Folder, op: Op) -> Result<(), Error> {
        let io = |e: std::io::Error| Error::Io(format!("{folder}: {e}"));
        if state.new {
            self.add_folder(folder)?;
            state.new = false;
        }
        let place = self.journal_place(folder);
        let bind = journal_bind(folder);
        if state.journal == JournalState::Again {
            let head = Inner { path: folder.to_string(), written_ms: super::now_ms(), origin: self.origin.clone(), compressed: false };
            state.valid = lines::create(&place, &self.keys.lines, Role::Journal, state.generation, &bind, &head).map_err(io)?;
            state.count = 1;
            state.changes = 0;
            state.journal = JournalState::Current;
        }
        let line = serde_json::to_vec(&op).map_err(|e| Error::Io(e.to_string()))?;
        state.valid = lines::append(&place, state.valid, &self.keys.lines, &bind, state.generation, state.count, &line).map_err(io)?;
        state.count += 1;
        state.changes += 1;
        op.apply(&mut state.entries, &mut state.top);
        if state.changes >= FOLD_LINES || state.valid >= FOLD_BYTES {
            // The change is made (its line is written): a fold that fails is tried again at the next change.
            let _ = self.fold(folder, state);
        }
        self.keep(folder, state);
        Ok(())
    }

    /// The journal folded into a new manifest, written twice, the journal begun again.
    pub(crate) fn fold(&self, folder: &LogicalPath, state: &mut Folder) -> Result<(), Error> {
        let io = |e: std::io::Error| Error::Io(format!("{folder}: {e}"));
        if state.new {
            self.add_folder(folder)?;
            state.new = false;
        }
        let generation = state.generation + 1;
        let body = serde_json::to_vec(&Body { entries: state.entries.clone(), top: state.top }).map_err(|e| Error::Io(e.to_string()))?;
        let head = Inner { path: folder.to_string(), written_ms: super::now_ms(), origin: self.origin.clone(), compressed: true };
        // Its first copy, then its second: a second copy never runs ahead of
        // the first, so lines added after a fold that stopped are never taken
        // as folded in. Once the first is in place the new generation is the
        // folder's: the journal is begun again before any other line.
        let seal_copy = |copy: u8| {
            let name = self.name(Role::Manifest, folder.as_str(), 0, copy);
            let header = Header::new(Role::Manifest, copy, generation);
            disk::place(&self.manifest_place(folder, copy), |out| format::seal(out, &self.keys.sealing, &name, &header, &head, &mut body.as_slice())).map_err(io)
        };
        seal_copy(0)?;
        state.generation = generation;
        state.journal = JournalState::Again;
        state.changes = 0;
        disk::crash(disk::Step::BetweenManifests).map_err(io)?;
        seal_copy(1)?;
        disk::crash(disk::Step::BeforeNewJournal).map_err(io)?;
        let bind = journal_bind(folder);
        state.valid = lines::create(&self.journal_place(folder), &self.keys.lines, Role::Journal, generation, &bind, &head).map_err(io)?;
        state.count = 1;
        state.changes = 0;
        state.journal = JournalState::Current;
        self.keep(folder, state);
        Ok(())
    }

    // The root manifests: which folders each `sealed/` holds.

    fn read_root(&self, root_name: &str, root: Root, copy: u8) -> Result<(u64, BTreeSet<String>), Fault> {
        let name = self.name(Role::Root, root_name, 0, copy);
        let place = format::place(&self.folders.sealed(root), &name);
        let mut file = std::fs::File::open(&place).map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { Fault::Missing } else { Fault::Io(e) })?;
        let mut bytes = Vec::new();
        let got = format::open(&mut file, &self.keys.sealing, &name, &Expected { role: Role::Root, copy, generation: None, path: Some(root_name) }, &mut bytes, LARGEST_MANIFEST)?;
        let body: RootBody = serde_json::from_slice(&bytes).map_err(|_| Fault::Broken("a list of folders that does not read"))?;
        Ok((got.header.generation, body.folders))
    }

    /// The folders a root holds: its root manifest, else its second copy, else made again from the blobs.
    pub(crate) fn root_folders(&self, root_name: &str, root: Root) -> Result<(u64, BTreeSet<String>), Error> {
        let first = self.read_root(root_name, root, 0);
        let second = self.read_root(root_name, root, 1);
        match (first, second) {
            (Ok(a), Ok(b)) => Ok(if b.0 > a.0 { b } else { a }),
            (Ok(a), Err(_)) | (Err(Fault::Missing | Fault::Broken(_)), Ok(a)) => Ok(a),
            // Neither copy, and no blob either: a root never written. Blobs there: made again below.
            (Err(Fault::Missing), Err(Fault::Missing)) if blob_places(&self.folders.sealed(root)).is_empty() => Ok((0, BTreeSet::new())),
            (Err(Fault::Io(e)), _) | (_, Err(Fault::Io(e))) => Err(Error::Io(e.to_string())),
            _ => {
                // Neither copy opens: the folders are those the blobs name.
                let folders: BTreeSet<String> = self
                    .scan_blobs(root)
                    .into_iter()
                    .filter(|found| found.inner.path.split('/').next() == Some(root_name))
                    .filter_map(|found| match found.header.role {
                        Role::File | Role::Lines => LogicalPath::checked_any(&found.inner.path).map(|p| p.folder().to_string()),
                        Role::Manifest | Role::Journal => Some(found.inner.path.clone()),
                        _ => None,
                    })
                    .collect();
                self.note(Notice::Rebuilt { folder: root_name.to_string(), files: folders.len(), attrs_lost: false });
                let _lock = super::lock::take(&self.root_lock(root_name, root), true, true)?;
                self.write_root(root_name, root, 1, &folders)?;
                Ok((1, folders))
            }
        }
    }

    pub(crate) fn write_root(&self, root_name: &str, root: Root, generation: u64, folders: &BTreeSet<String>) -> Result<(), Error> {
        let body = serde_json::to_vec(&RootBody { folders: folders.clone() }).map_err(|e| Error::Io(e.to_string()))?;
        let head = Inner { path: root_name.to_string(), written_ms: super::now_ms(), origin: self.origin.clone(), compressed: true };
        for copy in [0u8, 1] {
            let name = self.name(Role::Root, root_name, 0, copy);
            let header = Header::new(Role::Root, copy, generation);
            disk::place(&format::place(&self.folders.sealed(root), &name), |out| format::seal(out, &self.keys.sealing, &name, &header, &head, &mut body.as_slice())).map_err(|e| Error::Io(format!("{root_name}: {e}")))?;
        }
        Ok(())
    }

    fn root_lists(&self, folder: &LogicalPath) -> Result<bool, Error> {
        Ok(self.root_folders(folder.root_name(), folder.root())?.1.contains(folder.as_str()))
    }

    /// A folder listed in its root's manifest (before its first change is written).
    fn add_folder(&self, folder: &LogicalPath) -> Result<(), Error> {
        let (root_name, root) = (folder.root_name().to_string(), folder.root());
        if self.root_lists(folder)? {
            return Ok(());
        }
        let _lock = super::lock::take(&self.root_lock(&root_name, root), true, false)?;
        let (generation, mut folders) = self.root_folders(&root_name, root)?;
        if folders.insert(folder.to_string()) {
            self.write_root(&root_name, root, generation + 1, &folders)?;
        }
        Ok(())
    }

    /// A folder taken out of its root's manifest once nothing of it is left.
    pub(crate) fn drop_folder(&self, folder: &LogicalPath) -> Result<(), Error> {
        let (root_name, root) = (folder.root_name().to_string(), folder.root());
        let _lock = super::lock::take(&self.root_lock(&root_name, root), true, false)?;
        let (generation, mut folders) = self.root_folders(&root_name, root)?;
        if folders.remove(folder.as_str()) {
            self.write_root(&root_name, root, generation + 1, &folders)?;
        }
        for place in [self.manifest_place(folder, 0), self.manifest_place(folder, 1), self.journal_place(folder)] {
            disk::remove(&place).map_err(|e| Error::Io(format!("{folder}: {e}")))?;
        }
        self.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(folder.as_str());
        Ok(())
    }

    // Rebuilding from the blobs.

    /// Every blob under a root's `sealed/` folder, its head opened (role, generation, copy, path).
    pub(crate) fn scan_blobs(&self, root: Root) -> Vec<Found> {
        let sealed = self.folders.sealed(root);
        let mut found = Vec::new();
        for place in blob_places(&sealed) {
            let Some(name) = format::name_of(&place) else { continue };
            let Ok(mut file) = std::fs::File::open(&place) else { continue };
            if let Ok((header, inner)) = format::open_head(&mut file, &self.keys.sealing, &name) {
                // Only a blob at its own place: its name is the one its role, path, generation and copy make.
                let generation = if matches!(header.role, Role::File | Role::Lines) { header.generation } else { 0 };
                if self.name(header.role, &inner.path, generation, header.copy) == name {
                    found.push(Found { name, header, inner });
                }
            } else if let Ok(scanned) = lines::scan(&place, &self.keys.lines, &|_, _| name.clone(), None)
                && scanned.role == Role::Lines
                && let Some(head) = scanned.head
                && let Some(copy) = [0u8, 1].into_iter().find(|copy| self.name(Role::Lines, &head.path, scanned.generation, *copy) == name)
            {
                // A file that only grows: its lines are bound to its own name. (A journal's are bound to its folder, read by its known name.)
                let header = Header::new(Role::Lines, copy, scanned.generation);
                found.push(Found { name, header, inner: head });
            }
        }
        found
    }

    /// The folder made again from its blobs, what its manifests and journal
    /// still say laid over them, written as a new manifest; said once. Its
    /// highest generation is the highest any of them names, so that none is
    /// given again; a file the journal says gone stays gone.
    pub(crate) fn rebuild(&self, folder: &LogicalPath) -> Result<Folder, Error> {
        let found: Vec<Found> = self.scan_blobs(folder.root()).into_iter().filter(|f| matches!(f.header.role, Role::File | Role::Lines) && LogicalPath::checked_any(&f.inner.path).is_some_and(|p| p.folder() == *folder && !p.is_root())).collect();
        // What the manifests and the journal still say, the newest last.
        let mut said: BTreeMap<String, Rec> = BTreeMap::new();
        let mut gone: BTreeSet<String> = BTreeSet::new();
        let (mut generation, mut top) = (0, 0);
        for copy in [1u8, 0] {
            if let Ok((g, entries, t)) = self.read_manifest(folder, copy) {
                generation = generation.max(g);
                top = top.max(t);
                for (name, rec) in entries {
                    if said.get(&name).is_none_or(|was| was.g <= rec.g) {
                        said.insert(name, rec);
                    }
                }
            }
        }
        let bind = journal_bind(folder);
        // Its first line says its generation (a journal's name carries none); when it does not read, the manifests' is taken.
        let scanned = match lines::scan(&self.journal_place(folder), &self.keys.lines, &|_, _| bind.clone(), None) {
            Err(Fault::Broken(_)) => lines::scan(&self.journal_place(folder), &self.keys.lines, &|_, _| bind.clone(), Some((Role::Journal, generation))),
            other => other,
        };
        if let Ok(journal) = scanned {
            generation = generation.max(journal.generation);
            for (_, line) in journal.lines {
                if let Some(op) = line.and_then(|line| serde_json::from_slice::<Op>(&line).ok()) {
                    match op {
                        Op::P(name, rec) => {
                            gone.remove(&name);
                            said.insert(name, rec);
                        }
                        Op::D(name) => {
                            said.remove(&name);
                            gone.insert(name);
                        }
                        Op::R(from, to, rec) => {
                            said.remove(&from);
                            gone.insert(from);
                            gone.remove(&to);
                            said.insert(to, rec);
                        }
                    }
                }
            }
        }
        top = said.values().map(Rec::top).chain(found.iter().map(|f| f.header.generation)).fold(top, u64::max);
        // Each file's blobs, by generation: its copies and versions.
        let mut by_file: BTreeMap<String, Vec<Found>> = BTreeMap::new();
        for f in found {
            let name = f.inner.path.rsplit_once('/').map_or(f.inner.path.clone(), |(_, n)| n.to_string());
            if !gone.contains(&name) {
                by_file.entry(name).or_default().push(f);
            }
        }
        let mut entries = BTreeMap::new();
        let mut attrs_lost = false;
        for (name, mut blobs) in by_file {
            blobs.sort_by_key(|f| std::cmp::Reverse((f.header.generation, f.header.copy == 0)));
            let path = LogicalPath(format!("{folder}/{name}"));
            if blobs[0].header.role == Role::Lines {
                let newest = blobs[0].header.generation;
                let twin = blobs.iter().any(|f| f.header.generation == newest && f.header.copy == 1);
                let was = said.get(&name).filter(|was| was.g == newest);
                entries.insert(name.clone(), Rec { g: newest, k: 1, t: blobs[0].inner.written_ms, o: blobs[0].inner.origin.clone(), tw: twin, lc: was.map_or(0, |was| was.lc), ..Rec::default() });
                continue;
            }
            // Every blob opened whole: the newest that opens is the current one.
            let mut good: Vec<(u64, u8, format::Got)> = Vec::new();
            for f in &blobs {
                if let Ok(got) = self.open_blob(&path, f.header.generation, f.header.copy, &mut std::io::sink()) {
                    good.push((f.header.generation, f.header.copy, got));
                }
            }
            let Some(newest) = good.iter().map(|(g, _, _)| *g).max() else { continue };
            let current = good.iter().filter(|(g, _, _)| *g == newest).min_by_key(|(_, copy, _)| *copy).map(|(_, _, got)| got).expect("the newest is among them");
            let mut rec = Rec { g: newest, s: current.plain, z: current.sealed_size, h: current.hash.clone(), t: current.inner.written_ms, o: current.inner.origin.clone(), ..Rec::default() };
            if let Some((_, _, twin)) = good.iter().find(|(g, copy, _)| *g == newest && *copy == 1) {
                rec.tw = true;
                rec.th = twin.hash.clone();
                rec.tz = twin.sealed_size;
            }
            if !good.iter().any(|(g, copy, _)| *g == newest && *copy == 0) {
                // Its first copy is gone: its second is read, and `check --repair` seals the first again.
                rec.d.push(newest);
            }
            let mut versions: Vec<Ver> = good.iter().filter(|(g, copy, _)| *g < newest && *copy == 0).map(|(g, _, got)| Ver { g: *g, s: got.plain, z: got.sealed_size, h: got.hash.clone(), t: got.inner.written_ms, o: got.inner.origin.clone(), rest: BTreeMap::new() }).collect();
            versions.dedup_by_key(|v| v.g);
            rec.v = versions;
            match said.get(&name) {
                Some(was) if was.g == newest => {
                    rec.a = was.a.clone();
                    rec.rm = was.rm;
                    rec.nv = was.nv;
                    if was.rm {
                        // A removed file's last content is its newest version.
                        let mut v = vec![rec.as_version()];
                        v.append(&mut rec.v);
                        rec.v = v;
                    }
                }
                Some(was) => {
                    rec.a = was.a.clone();
                    rec.nv = was.nv;
                    if was.g > newest && !was.rm {
                        // Named newer than any blob that opens: that content is lost here.
                        rec.d.push(was.g);
                    }
                }
                None => attrs_lost = true,
            }
            entries.insert(name, rec);
        }
        // Named, and no blob of it opens: damaged, never absent.
        for (name, rec) in &said {
            if !entries.contains_key(name) && !rec.rm {
                entries.insert(name.clone(), Rec { d: vec![rec.g], ..rec.clone() });
            }
        }
        let files = entries.len();
        let mut state = Folder::of(generation, entries, top);
        self.add_folder(folder)?;
        self.fold(folder, &mut state)?;
        self.note(Notice::Rebuilt { folder: folder.to_string(), files, attrs_lost });
        Ok(state)
    }
}

/// A blob found on the disk: its name, its clear header, its piece 0.
pub(crate) struct Found {
    pub name: String,
    pub header: Header,
    pub inner: Inner,
}

/// Every name's place under a `sealed/` folder (its fan-out folders), temporary names left out.
pub(crate) fn blob_places(sealed: &Path) -> Vec<PathBuf> {
    let mut places = Vec::new();
    let Ok(fans) = std::fs::read_dir(sealed) else { return places };
    for fan in fans.filter_map(Result::ok) {
        let fan_name = fan.file_name();
        let fan_name = fan_name.to_string_lossy();
        if fan_name.len() != 2 || fan_name.starts_with('.') {
            continue;
        }
        let Ok(inside) = std::fs::read_dir(fan.path()) else { continue };
        for entry in inside.filter_map(Result::ok) {
            if !entry.file_name().to_string_lossy().starts_with('.') {
                places.push(entry.path());
            }
        }
    }
    places.sort();
    places
}

impl LogicalPath {
    /// A path any blob names (the store's own root included), checked as a caller's is.
    pub(crate) fn checked_any(path: &str) -> Option<LogicalPath> {
        LogicalPath::checked(path, true).ok()
    }
}

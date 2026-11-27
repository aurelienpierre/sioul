// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The door itself: a store opened with its key, and every way of reading
//! and writing through it (docs/store.md; the contract builders code
//! against is that note's "The door").

use super::format::{self, Expected, Fault, Header, Inner, Role};
use super::keys::{Keys, RootKey};
use super::lock::{self, ProfileLock};
use super::manifest::{Cached, Folder, Found, Loaded, Op, Rec, Ver};
use super::storefile::{self, StoreFile};
use super::{Collected, Entry, Error, Expect, Folders, How, Kind, Lines, LogicalPath, Notice, Opened, Options, Purged, Root, Source, Version, Written, disk, lines};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The lock held while a store is made over a profile (`.store-create.lock`
/// beside its `store.toml`: empty, in clear by rule), across processes.
fn creation_lock(folders: &Folders) -> Result<std::fs::File, Error> {
    let place = folders.config.join(".store-create.lock");
    disk::private_dir(&folders.config).map_err(|e| Error::Io(format!("{}: {e}", folders.config.display())))?;
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&place).map_err(|e| Error::Locked(format!("{}: {e}", place.display())))?;
    file.lock().map_err(|e| Error::Locked(format!("{}: {e}", place.display())))?;
    Ok(file)
}

/// The mark of a creation under way, in the configuration's `sealed/` folder, removed once `store.toml` is written.
const CREATING: &str = ".creating";

/// The most a whole file may inflate to when read (a guard against a bomb, not a limit on what is written).
const LARGEST: u64 = 4 << 30;
/// Damaged copies set aside stay this long before they go.
const ASIDE_DAYS: i64 = 30;

/// A store opened with its key.
pub struct Store {
    pub(crate) folders: Folders,
    pub(crate) id: String,
    pub(crate) keys: Keys,
    versions: AtomicUsize,
    pub(crate) origin: String,
    pub(crate) cache: Mutex<HashMap<String, Cached>>,
    /// Files that only grow, as last read here: their identity, where their valid part ends, their lines.
    grown: Mutex<HashMap<PathBuf, ((u64, i128, u64, i128), u64, u64)>>,
    notices: Mutex<(Vec<Notice>, BTreeSet<String>)>,
    /// The person's folders inside Sioul's that stay in clear (`kept_clear`), as last read here.
    clear: Mutex<Option<Vec<LogicalPath>>>,
    /// The folders marked moved, as last read here, and when (`root_moved`): a folder once moved stays so.
    pub(crate) moved_seen: Mutex<(Option<std::time::Instant>, BTreeSet<String>)>,
    /// The files the move left in clear, as last read here, and when (`noted_left`).
    pub(crate) left_seen: Mutex<Option<(std::time::Instant, BTreeSet<String>)>>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("id", &self.id).field("folders", &self.folders).finish_non_exhaustive()
    }
}

fn refuse_synced(folders: &Folders, options: &Options) -> Result<(), Error> {
    for root in Root::ALL {
        if let Some(carried) = super::synced::carried(folders.of(root), &options.synced_roots) {
            return Err(Error::Synced(carried));
        }
    }
    Ok(())
}

fn io(path: &LogicalPath) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| if e.kind() == std::io::ErrorKind::StorageFull { Error::NoRoom } else { Error::Io(format!("{path}: {e}")) }
}

impl Store {
    /// The store's id, read from `store.toml` without a key: what its key is kept under.
    pub fn id(folders: &Folders) -> Result<Option<String>, Error> {
        Ok(StoreFile::load(folders)?.map(|file| file.id))
    }

    /// Every id the copies of `store.toml` that read give, the first copy's
    /// first: a first copy whose id is damaged but still reads never hides
    /// the second's (the keyring is asked under each).
    pub fn ids(folders: &Folders) -> Result<Vec<String>, Error> {
        let mut ids: Vec<String> = Vec::new();
        for file in StoreFile::load_both(folders) {
            if !ids.contains(&file.id) {
                ids.push(file.id);
            }
        }
        if ids.is_empty() {
            StoreFile::load(folders)?;
        }
        Ok(ids)
    }

    /// A new store: `store.toml` and the four `sealed/` folders. `Exists` when there is one.
    pub fn create(folders: &Folders, key: &RootKey, options: Options) -> Result<Store, Error> {
        Store::create_with(folders, &storefile::new_id(), key, options)
    }

    /// A new store under an id made before (`new_id`): its key is kept in the
    /// keyring under that id before anything is written here.
    pub fn create_with(folders: &Folders, id: &str, key: &RootKey, options: Options) -> Result<Store, Error> {
        refuse_synced(folders, &options)?;
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Refused("a store's id is hexadecimal".to_string()));
        }
        // One creation at a time over a profile (two starts at once, two
        // processes): the second finds the first's store under the lock, `Exists`.
        let _creation = creation_lock(folders)?;
        // A store.toml lost beside sealed files is damage: a new store over them would sweep them.
        // A creation cut before its store.toml (its mark still there) left only empty lists: begun again.
        let creating = folders.sealed(Root::Config).join(CREATING);
        if StoreFile::load(folders)?.is_some() || (storefile::has_blobs(folders) && !creating.exists()) {
            return Err(Error::Exists);
        }
        if creating.exists() {
            for root in Root::ALL {
                for place in super::manifest::blob_places(&folders.sealed(root)) {
                    let _ = disk::remove(&place);
                }
            }
        }
        let id = id.to_string();
        let keys = Keys::derive(key, &id);
        for root in Root::ALL {
            disk::private_dir(&folders.sealed(root)).map_err(|e| Error::Io(format!("{}: {e}", folders.sealed(root).display())))?;
        }
        let file = StoreFile::new(id.clone(), &keys);
        let store = Store::made(folders, id, keys, options);
        // Each root's list of folders, empty, then `store.toml` last: a list missing beside it is damage.
        // The mark says, until then, that the sealed files there are only a creation's, cut short.
        disk::place(&creating, |out| out.write_all(b"")).map_err(|e| Error::Io(format!("{}: {e}", creating.display())))?;
        for root in Root::ALL {
            store.write_root(root.name(), root, 1, &BTreeSet::new())?;
        }
        store.write_root(super::INTERNAL, Root::Config, 1, &BTreeSet::new())?;
        disk::crash(disk::Step::BeforeJournal).map_err(|e| Error::Io(e.to_string()))?;
        file.save(folders)?;
        let _ = disk::remove(&creating);
        Ok(store)
    }

    /// The store opened: the key checked before anything is read (`WrongKey`, nothing touched).
    pub fn open(folders: &Folders, key: &RootKey, options: Options) -> Result<Store, Error> {
        refuse_synced(folders, &options)?;
        let both = StoreFile::load_both(folders);
        if both.is_empty() {
            // Neither copy reads: damage or a newer Sioul's (said by `load`), else none at all.
            StoreFile::load(folders)?;
            let cut_creation = folders.sealed(Root::Config).join(CREATING).exists();
            return Err(if storefile::has_blobs(folders) && !cut_creation { Error::Damaged { path: "store.toml".to_string(), what: "both copies are gone while sealed files are there: a way back is needed".to_string() } } else { Error::NotCreated });
        }
        // The copy this key opens (a first copy whose id or check was damaged gives way to the second).
        let Some((file, keys)) = both.iter().find_map(|file| {
            let keys = Keys::derive(key, &file.id);
            file.opens(&keys).then(|| (file.clone(), keys))
        }) else {
            return Err(Error::WrongKey);
        };
        if both.iter().any(|other| *other != file) {
            let _ = file.save(folders);
        }
        let store = Store::made(folders, file.id, keys, options);
        if let Some(n) = store.versions_kept_in_store() {
            store.versions.store(n, Ordering::Relaxed);
        }
        Ok(store)
    }

    fn made(folders: &Folders, id: String, keys: Keys, options: Options) -> Store {
        Store {
            folders: folders.clone(),
            id,
            keys,
            versions: AtomicUsize::new(options.versions),
            origin: options.origin,
            cache: Mutex::new(HashMap::new()),
            grown: Mutex::new(HashMap::new()),
            notices: Mutex::new((Vec::new(), BTreeSet::new())),
            clear: Mutex::new(None),
            moved_seen: Mutex::new((None, BTreeSet::new())),
            left_seen: Mutex::new(None),
        }
    }

    pub fn store_id(&self) -> &str {
        &self.id
    }

    pub fn profile(&self) -> &Folders {
        &self.folders
    }

    /// n, the earlier versions kept of each file.
    pub fn versions_kept(&self) -> usize {
        self.versions.load(Ordering::Relaxed)
    }

    /// n changed, for every process: kept in the store itself (sealed), read at each
    /// opening and at each collection; each file follows at its next change, the whole store at `collect`.
    pub fn set_versions(&self, n: usize) -> Result<(), Error> {
        self.write(&LogicalPath::internal("versions"), n.to_string().as_bytes(), How::only_here())?;
        self.versions.store(n, Ordering::Relaxed);
        Ok(())
    }

    /// n as the store keeps it, when it was ever set.
    fn versions_kept_in_store(&self) -> Option<usize> {
        let opened = self.read(&LogicalPath::internal("versions")).ok().flatten()?;
        std::str::from_utf8(&opened.bytes).ok()?.trim().parse().ok()
    }

    /// The profile's lock held shared for several changes that the move must not split.
    pub fn hold(&self) -> Result<ProfileLock, Error> {
        ProfileLock::shared(&self.folders)
    }

    /// What the store would say, each once in this process.
    pub fn take_notices(&self) -> Vec<Notice> {
        std::mem::take(&mut self.notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner).0)
    }

    pub(crate) fn note(&self, notice: Notice) {
        let mut notices = self.notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if notices.1.insert(format!("{notice:?}")) {
            notices.0.push(notice);
        }
    }

    pub(crate) fn name(&self, role: Role, path: &str, generation: u64, copy: u8) -> String {
        format::name(&self.keys.naming, role, path, generation, copy)
    }

    pub(crate) fn blob_place(&self, path: &LogicalPath, role: Role, generation: u64, copy: u8) -> PathBuf {
        format::place(&self.folders.sealed(path.root()), &self.name(role, path.as_str(), generation, copy))
    }

    fn aside_place(&self, path: &LogicalPath, generation: u64, copy: u8) -> PathBuf {
        self.folders.sealed(path.root()).join(".aside").join(self.name(Role::File, path.as_str(), generation, copy))
    }

    /// A file's blob opened whole into `out`.
    pub(crate) fn open_blob(&self, path: &LogicalPath, generation: u64, copy: u8, out: &mut dyn Write) -> Result<format::Got, Fault> {
        let place = self.blob_place(path, Role::File, generation, copy);
        let mut file = match std::fs::File::open(&place) {
            Ok(file) => std::io::BufReader::with_capacity(64 << 10, file),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Fault::Missing),
            Err(e) => return Err(Fault::Io(e)),
        };
        let name = self.name(Role::File, path.as_str(), generation, copy);
        format::open(&mut file, &self.keys.sealing, &name, &Expected { role: Role::File, copy, generation: Some(generation), path: Some(path.as_str()) }, out, LARGEST)
    }

    /// A file's blob sealed and put in place.
    fn seal_blob(&self, path: &LogicalPath, generation: u64, copy: u8, written_ms: i64, compressed: bool, content: &mut dyn Read) -> Result<format::Made, Error> {
        let name = self.name(Role::File, path.as_str(), generation, copy);
        let header = Header::new(Role::File, copy, generation);
        let inner = Inner { path: path.to_string(), written_ms, origin: self.origin.clone(), compressed };
        disk::place(&self.blob_place(path, Role::File, generation, copy), |out| format::seal(out, &self.keys.sealing, &name, &header, &inner, content)).map_err(io(path))
    }

    fn file_path(path: &LogicalPath) -> Result<(), Error> {
        if path.is_root() {
            return Err(Error::Refused("a root is a folder, not a file".to_string()));
        }
        Ok(())
    }

    /// A folder's state with its lock held shared; mended first under the lock held exclusive when it must be.
    fn read_folder(&self, folder: &LogicalPath) -> Result<(lock::Held, Folder), Error> {
        let held = lock::take_folder(&self.folder_lock(folder), false, false)?;
        match self.load(folder, false)? {
            Loaded::Ready(state) => Ok((held, state)),
            Loaded::NeedsRepair => {
                drop(held);
                {
                    let _profile = self.hold()?;
                    let _mending = lock::take_folder(&self.folder_lock(folder), true, false)?;
                    self.load(folder, true)?;
                }
                let held = lock::take_folder(&self.folder_lock(folder), false, false)?;
                match self.load(folder, false)? {
                    Loaded::Ready(state) => Ok((held, state)),
                    Loaded::NeedsRepair => Err(Error::Damaged { path: folder.to_string(), what: "its list could not be made again".to_string() }),
                }
            }
        }
    }

    /// A folder's state with its lock held exclusive (and the profile's shared), mended if it must be.
    fn change_folder(&self, folder: &LogicalPath) -> Result<(ProfileLock, lock::Held, Folder), Error> {
        let profile = self.hold()?;
        let held = lock::take_folder(&self.folder_lock(folder), true, false)?;
        match self.load(folder, true)? {
            Loaded::Ready(state) => Ok((profile, held, state)),
            Loaded::NeedsRepair => Err(Error::Damaged { path: folder.to_string(), what: "its list could not be made again".to_string() }),
        }
    }

    // Reading.

    /// The copies of a file to try, in order: its current one, its twin, its earlier versions.
    fn candidates(rec: &Rec) -> Vec<(u64, u8, Source)> {
        let mut out = Vec::new();
        if !rec.rm {
            if !rec.d.contains(&rec.g) {
                out.push((rec.g, 0, Source::Current));
            }
            if rec.tw {
                out.push((rec.g, 1, Source::Twin));
            }
        }
        for v in &rec.v {
            if !rec.d.contains(&v.g) {
                out.push((v.g, 0, Source::Earlier { generation: v.g, written_ms: v.t }));
            }
        }
        out
    }

    /// A file's content: its current copy, else the newest that opens (said once). `Err(Damaged)` when none does.
    fn read_rec(&self, path: &LogicalPath, rec: &Rec, failed: &mut Vec<(u64, u8)>) -> Result<Opened, Error> {
        let mut bytes = Vec::with_capacity(rec.s.min(64 << 20) as usize);
        let mut refused = None;
        for (generation, copy, source) in Store::candidates(rec) {
            bytes.clear();
            match self.open_checked(path, rec, generation, copy, &mut bytes) {
                Ok(_) => {
                    if source != Source::Current {
                        self.note(Notice::Mended { path: path.to_string(), from: source.clone() });
                    }
                    return Ok(Opened { bytes, generation: rec.g, from: source });
                }
                // The disk refused this copy: the next is tried, nothing is marked damaged for it.
                Err(Fault::Io(e)) => refused = Some(e),
                Err(Fault::Missing | Fault::Broken(_)) => failed.push((generation, copy)),
            }
        }
        match refused {
            Some(e) => Err(Error::Io(format!("{path}: {e}"))),
            None => Err(Error::Damaged { path: path.to_string(), what: "no copy of it opens".to_string() }),
        }
    }

    /// A copy opened, and its sealed bytes the ones its record says (when it says): another blob of the same name is refused.
    pub(crate) fn open_checked(&self, path: &LogicalPath, rec: &Rec, generation: u64, copy: u8, out: &mut dyn Write) -> Result<format::Got, Fault> {
        let got = self.open_blob(path, generation, copy, out)?;
        let said = rec.hash_of(generation, copy);
        if !said.is_empty() && said != got.hash {
            return Err(Fault::Broken("not the copy its list names"));
        }
        Ok(got)
    }

    /// The copies found damaged noted in the manifest, so that `check` names them and reads skip them.
    fn mark_damaged(&self, path: &LogicalPath, failed: &[(u64, u8)]) {
        if failed.is_empty() {
            return;
        }
        let folder = path.folder();
        let Ok((_profile, _held, mut state)) = self.change_folder(&folder) else { return };
        let Some(mut rec) = state.entries.get(path.file_name()).cloned() else { return };
        let before = rec.clone();
        for &(generation, copy) in failed {
            if copy == 1 && generation == rec.g {
                rec.tw = false;
            } else if !rec.d.contains(&generation) {
                rec.d.push(generation);
            }
        }
        if rec != before {
            let _ = self.record(&folder, &mut state, Op::P(path.file_name().to_string(), rec));
        }
    }

    pub fn read(&self, path: &LogicalPath) -> Result<Option<Opened>, Error> {
        Store::file_path(path)?;
        let mut failed = Vec::new();
        let read = {
            let (_held, state) = self.read_folder(&path.folder())?;
            match state.entries.get(path.file_name()) {
                None => return Ok(None),
                Some(rec) if rec.rm => return Ok(None),
                Some(rec) if rec.k == 1 => return Err(Error::Refused(format!("{path} only grows: read it with read_lines"))),
                Some(rec) => self.read_rec(path, rec, &mut failed),
            }
        };
        self.mark_damaged(path, &failed);
        read.map(Some)
    }

    pub fn read_text(&self, path: &LogicalPath) -> Result<Option<(String, u64)>, Error> {
        match self.read(path)? {
            None => Ok(None),
            Some(opened) => String::from_utf8(opened.bytes).map(|text| Some((text, opened.generation))).map_err(|_| Error::NotText),
        }
    }

    /// A file streamed into `out`, a piece at a time. A copy found damaged
    /// half way may have written part of itself: said `Damaged`, and read
    /// again, the next copy is read.
    pub fn read_into(&self, path: &LogicalPath, out: &mut dyn Write) -> Result<Option<u64>, Error> {
        Store::file_path(path)?;
        let mut failed = Vec::new();
        let read = {
            let (_held, state) = self.read_folder(&path.folder())?;
            let Some(rec) = state.entries.get(path.file_name()).filter(|rec| !rec.rm) else { return Ok(None) };
            let mut done = Err(Error::Damaged { path: path.to_string(), what: "no copy of it opens".to_string() });
            for (generation, copy, source) in Store::candidates(rec) {
                let mut counted = format::Hashing::new(&mut *out);
                match self.open_checked(path, rec, generation, copy, &mut counted) {
                    Ok(got) => {
                        if source != Source::Current {
                            self.note(Notice::Mended { path: path.to_string(), from: source });
                        }
                        done = Ok(Some(got.plain));
                        break;
                    }
                    Err(Fault::Io(e)) => {
                        done = Err(Error::Io(format!("{path}: {e}")));
                        break;
                    }
                    Err(_) => {
                        failed.push((generation, copy));
                        if counted.size > 0 {
                            done = Err(Error::Damaged { path: path.to_string(), what: "a copy was cut while it was read: read it again".to_string() });
                            break;
                        }
                    }
                }
            }
            done
        };
        self.mark_damaged(path, &failed);
        read
    }

    pub fn stat(&self, path: &LogicalPath) -> Result<Option<Entry>, Error> {
        Store::file_path(path)?;
        let (_held, state) = self.read_folder(&path.folder())?;
        Ok(state.entries.get(path.file_name()).filter(|rec| !rec.rm).map(|rec| Store::entry(path, rec)))
    }

    /// A folder's files, from its manifest alone; a folder never written: none.
    pub fn list(&self, folder: &LogicalPath) -> Result<Vec<Entry>, Error> {
        let (_held, state) = self.read_folder(folder)?;
        state.entries.iter().filter(|(_, rec)| !rec.rm).map(|(name, rec)| folder.join(name).map(|path| Store::entry(&path, rec))).collect()
    }

    /// Every folder below `under` (itself included when it holds files), from its root's list.
    pub fn folders(&self, under: &LogicalPath) -> Result<Vec<LogicalPath>, Error> {
        let (_, listed) = self.root_folders(under.root_name(), under.root())?;
        let mut out = BTreeSet::new();
        for folder in listed {
            let Some(path) = LogicalPath::checked_any(&folder) else { continue };
            if path.is_within(under) {
                // Its parents down from `under`, which hold folders if not files.
                let mut parent = path.clone();
                while parent != *under && !parent.is_root() {
                    out.insert(parent.clone());
                    parent = parent.folder();
                }
                out.insert(path);
            }
        }
        Ok(out.into_iter().collect())
    }

    fn entry(path: &LogicalPath, rec: &Rec) -> Entry {
        Entry {
            path: path.clone(),
            generation: rec.g,
            size: rec.s,
            sealed_size: rec.z,
            written_ms: rec.t,
            origin: rec.o.clone(),
            kind: if rec.k == 1 { Kind::Lines } else { Kind::File },
            twin: rec.tw,
            versions: rec.v.len(),
            attrs: rec.a.clone(),
            damaged: rec.d.contains(&rec.g),
        }
    }

    // Writing.

    fn expect(rec: Option<&Rec>, expect: Expect) -> Result<(), Error> {
        let now = rec.filter(|rec| !rec.rm).map(|rec| rec.g);
        match (expect, now) {
            (Expect::Any, _) | (Expect::Absent, None) => Ok(()),
            (Expect::Generation(g), Some(n)) if g == n => Ok(()),
            _ => Err(Error::Changed { now }),
        }
    }

    pub fn write(&self, path: &LogicalPath, bytes: &[u8], how: How) -> Result<Written, Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        Store::expect(state.entries.get(path.file_name()), how.expect)?;
        self.write_locked(path, &mut state, &mut &bytes[..], bytes.len() as u64, how, None)
    }

    /// The content of `generation`, still the current one, replaced without
    /// keeping it as an earlier version: a file held open and sealed again as
    /// it is written (`files::File`), which makes one new version, never
    /// pushing the earlier ones out. The new content takes a generation of its
    /// own, so that a power cut at any step leaves the old one whole, named by
    /// its line (review-finish G6); the replaced blob goes once the new line
    /// is written. `Changed` when another generation is current now.
    pub fn amend(&self, path: &LogicalPath, bytes: &[u8], generation: u64, how: How) -> Result<Written, Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let name = path.file_name().to_string();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        let changed = |state: &Folder| Error::Changed { now: state.entries.get(path.file_name()).filter(|rec| !rec.rm).map(|rec| rec.g) };
        // A blob an interrupted write left at the next generation is taken first: then this generation is no longer current.
        let Some(old) = self.caught_up(path, &mut state)?.filter(|rec| !rec.rm && rec.g == generation && rec.k == 0) else {
            return Err(changed(&state));
        };
        self.ensure_room(path, (bytes.len() as u64).saturating_mul(if old.tw { 2 } else { 1 }), &folder, &mut state)?;
        // Read again after the room was made: earlier versions may have gone for it.
        let Some(old) = state.entries.get(path.file_name()).cloned().filter(|rec| !rec.rm && rec.g == generation) else {
            return Err(changed(&state));
        };
        let new = next_generation(state.top, Some(&old));
        let written_ms = super::now_ms();
        let compressed = !how.uncompressed;
        let made = self.seal_blob(path, new, 0, written_ms, compressed, &mut &bytes[..])?;
        let twin = if old.tw { Some(self.seal_blob(path, new, 1, written_ms, compressed, &mut &bytes[..])?) } else { None };
        disk::crash(disk::Step::BeforeJournal).map_err(io(path))?;
        let mut rec = Rec { g: new, s: made.plain, z: made.sealed_size, h: made.hash.clone(), t: written_ms, o: self.origin.clone(), nc: how.uncompressed, ..old.clone() };
        rec.d.retain(|d| *d != generation);
        if let Some(twin) = &twin {
            rec.th = twin.hash.clone();
            rec.tz = twin.sealed_size;
        }
        self.record(&folder, &mut state, Op::P(name, rec))?;
        disk::crash(disk::Step::AfterJournal).map_err(io(path))?;
        let mut after = Tidy::default();
        if old.d.contains(&generation) {
            after.aside.push((generation, 0));
        } else {
            after.gone.push(self.blob_place(path, Role::File, generation, 0));
        }
        if old.tw {
            after.gone.push(self.blob_place(path, Role::File, generation, 1));
        }
        self.tidy(path, after);
        Ok(Written { generation: new, size: made.plain })
    }

    /// A large file sealed from `content` a piece at a time.
    pub fn write_from(&self, path: &LogicalPath, content: &mut dyn Read, how: How) -> Result<Written, Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        Store::expect(state.entries.get(path.file_name()), how.expect)?;
        // A stream's size is not known: room for a piece at least, then what the disk says as it goes.
        self.write_locked(path, &mut state, content, 1 << 20, how, None)
    }

    /// Read, change, write, under the folder's lock. `change` gets the
    /// current bytes (none: absent) and gives the new ones (none: leave it).
    /// A file that does not open is `Damaged`: `change` is never given nothing for it.
    pub fn update(&self, path: &LogicalPath, how: How, change: &mut dyn FnMut(Option<&[u8]>) -> Result<Option<Vec<u8>>, String>) -> Result<Option<Written>, Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let mut failed = Vec::new();
        let done = (|| {
            let (_profile, _held, mut state) = self.change_folder(&folder)?;
            let rec = state.entries.get(path.file_name()).cloned();
            Store::expect(rec.as_ref(), how.expect)?;
            let old = match &rec {
                Some(rec) if rec.k == 1 && !rec.rm => return Err(Error::Refused(format!("{path} only grows: append to it"))),
                Some(rec) if !rec.rm => {
                    let opened = self.read_rec(path, rec, &mut failed)?;
                    // Never an earlier version given as the file to change: the change would roll it back unsaid.
                    if let Source::Earlier { .. } = opened.from {
                        return Err(Error::Damaged { path: path.to_string(), what: "its current copy does not open: put an earlier version back first (restore)".to_string() });
                    }
                    Some(opened.bytes)
                }
                _ => None,
            };
            let Some(new) = change(old.as_deref()).map_err(Error::Change)? else { return Ok(None) };
            self.write_locked(path, &mut state, &mut new.as_slice(), new.len() as u64, how, None).map(Some)
        })();
        self.mark_damaged(path, &failed);
        done
    }

    /// The write itself, the folder's lock held exclusive: the blob (and its
    /// twin) put in place, the journal's line (the change made), then what
    /// it made too many taken out. `restored` keeps the generation's own time and origin.
    fn write_locked(&self, path: &LogicalPath, state: &mut Folder, content: &mut dyn Read, size: u64, how: How, restored: Option<&Ver>) -> Result<Written, Error> {
        let only_here = how.only_here;
        let compressed = !how.uncompressed;
        let folder = path.folder();
        let name = path.file_name().to_string();
        if state.entries.get(&name).is_some_and(|rec| rec.k == 1 && !rec.rm) {
            return Err(Error::Refused(format!("{path} only grows: append to it")));
        }
        self.ensure_room(path, size.saturating_mul(if only_here { 2 } else { 1 }), &folder, state)?;
        // Read after the room was made: its earlier versions may have gone for it.
        let old = self.mended_from_twin(path, self.caught_up(path, state)?);
        let generation = next_generation(state.top, old.as_ref());
        let written_ms = super::now_ms();
        let (made, twin) = if only_here {
            // Read once, sealed twice: the content held in memory for its twin.
            let mut bytes = Vec::new();
            content.read_to_end(&mut bytes).map_err(io(path))?;
            let made = self.seal_blob(path, generation, 0, written_ms, compressed, &mut bytes.as_slice())?;
            let twin = self.seal_blob(path, generation, 1, written_ms, compressed, &mut bytes.as_slice())?;
            (made, Some(twin))
        } else {
            (self.seal_blob(path, generation, 0, written_ms, compressed, content)?, None)
        };
        disk::crash(disk::Step::BeforeJournal).map_err(io(path))?;
        let (mut rec, after) = self.successor(path, old.as_ref(), &made, twin.as_ref(), generation, written_ms, how);
        if let Some(restored) = restored {
            rec.o = format!("{} (restored from {})", self.origin, restored.g);
        }
        self.record(&folder, state, Op::P(name, rec))?;
        disk::crash(disk::Step::AfterJournal).map_err(io(path))?;
        self.tidy(path, after);
        Ok(Written { generation, size: made.plain })
    }

    /// The file's record, a blob of the generation its next write would take
    /// taken first when one is there (a list put back older than the blob, an
    /// interrupted write): written over unseen, the newer content would be
    /// lost; taken, it becomes the earlier version the write replaces.
    fn caught_up(&self, path: &LogicalPath, state: &mut Folder) -> Result<Option<Rec>, Error> {
        let name = path.file_name();
        let next = next_generation(state.top, state.entries.get(name));
        if self.blob_place(path, Role::File, next, 0).exists() || self.blob_place(path, Role::Lines, next, 0).exists() {
            self.adopt(&path.folder(), state, name, next)?;
        }
        Ok(state.entries.get(name).cloned())
    }

    /// A record whose current copy is damaged while its twin opens: the
    /// twin's content sealed again as the first copy, so that the content a
    /// write or a removal replaces is kept as an earlier version (an undo),
    /// never lost with the twin it then takes out. As it was when the twin
    /// does not open either.
    fn mended_from_twin(&self, path: &LogicalPath, old: Option<Rec>) -> Option<Rec> {
        let mut rec = old?;
        if rec.rm || !rec.tw || !rec.d.contains(&rec.g) {
            return Some(rec);
        }
        let mut bytes = Vec::new();
        if self.open_checked(path, &rec, rec.g, 1, &mut bytes).is_err() {
            return Some(rec);
        }
        if let Ok(made) = self.seal_blob(path, rec.g, 0, rec.t, !rec.nc, &mut bytes.as_slice()) {
            rec.h = made.hash;
            rec.z = made.sealed_size;
            let g = rec.g;
            rec.d.retain(|d| *d != g);
        }
        Some(rec)
    }

    /// The record of a new content at `path` over `old`: the replaced one
    /// an earlier version, n kept; and what to take out once it is written.
    fn successor(&self, path: &LogicalPath, old: Option<&Rec>, made: &format::Made, twin: Option<&format::Made>, generation: u64, written_ms: i64, how: How) -> (Rec, Tidy) {
        // Never versioned (a message): its source is its backup.
        let n = if how.unversioned { 0 } else { self.versions_kept() };
        let mut rec = Rec { g: generation, s: made.plain, z: made.sealed_size, h: made.hash.clone(), t: written_ms, o: self.origin.clone(), nv: how.unversioned, nc: how.uncompressed, ..Rec::default() };
        if let Some(twin) = twin {
            rec.tw = true;
            rec.th = twin.hash.clone();
            rec.tz = twin.sealed_size;
        }
        let mut after = Tidy::default();
        if let Some(old) = old {
            rec.a = old.a.clone();
            let mut versions = Vec::new();
            if !old.rm {
                if old.d.contains(&old.g) {
                    after.aside.push((old.g, 0));
                } else {
                    versions.push(old.as_version());
                }
                if old.tw {
                    after.gone.push(self.blob_place(path, Role::File, old.g, 1));
                }
            }
            versions.extend(old.v.iter().filter(|v| !old.d.contains(&v.g)).cloned());
            after.aside.extend(old.v.iter().filter(|v| old.d.contains(&v.g)).map(|v| (v.g, 0)));
            for beyond in versions.split_off(n.min(versions.len())) {
                after.gone.push(self.blob_place(path, Role::File, beyond.g, 0));
            }
            rec.v = versions;
        }
        (rec, after)
    }

    /// What a change made too many taken out, once its line is written.
    fn tidy(&self, path: &LogicalPath, after: Tidy) {
        for place in after.gone {
            let size = std::fs::metadata(&place).map_or(0, |m| m.len());
            if disk::remove(&place).is_ok() {
                disk::freed(size);
            }
        }
        for (generation, copy) in after.aside {
            self.set_aside(path, generation, copy);
        }
    }

    /// A damaged copy moved to `sealed/.aside/`, out of the way, until `collect` or a purge takes it.
    fn set_aside(&self, path: &LogicalPath, generation: u64, copy: u8) {
        let place = self.blob_place(path, Role::File, generation, copy);
        if !place.exists() {
            return;
        }
        let target = self.aside_place(path, generation, copy);
        if let Some(dir) = target.parent()
            && disk::private_dir(dir).is_ok()
            && disk::rename_over(&place, &target).is_ok()
        {
            // Its time set aside, which `collect` counts its 30 days from.
            let _ = std::fs::File::options().append(true).open(&target).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        }
    }

    /// Room for `bytes` more: earlier versions go first, the oldest first, never a twin nor a file's last good copy.
    /// Earlier versions go first when the disk is short (the oldest first,
    /// never a twin nor a file's last good copy); the write itself is never
    /// refused for a share of the disk, as the system's is not: only a disk
    /// that is really full refuses it (`NoRoom`, the old content intact).
    fn ensure_room(&self, path: &LogicalPath, bytes: u64, folder: &LogicalPath, state: &mut Folder) -> Result<(), Error> {
        let sealed = self.folders.sealed(path.root());
        if !disk::fits(&sealed, bytes) {
            self.collect_versions(path.root(), Some((folder, state)), &|_| disk::fits(&sealed, bytes))?;
        }
        Ok(())
    }

    /// When a folder last changed (its list's last line or fold, a file
    /// added, replaced or removed there), in milliseconds: a sealed folder's
    /// time, as change checks read a folder's (review-finish F5).
    pub fn folder_changed_ms(&self, folder: &LogicalPath) -> i64 {
        [self.journal_place(folder), self.manifest_place(folder, 0)]
            .iter()
            .filter_map(|place| std::fs::metadata(place).ok()?.modified().ok())
            .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .max()
            .unwrap_or(0)
    }

    pub fn remove(&self, path: &LogicalPath, expect: Expect) -> Result<bool, Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        let name = path.file_name().to_string();
        let Some(old) = state.entries.get(&name).cloned().filter(|rec| !rec.rm) else {
            return if matches!(expect, Expect::Generation(_)) { Err(Error::Changed { now: None }) } else { Ok(false) };
        };
        Store::expect(Some(&old), expect)?;
        let old = self.mended_from_twin(path, Some(old)).expect("given");
        self.remove_locked(path, &folder, &mut state, &old)?;
        Ok(true)
    }

    /// A removal: the content kept as an earlier version (n > 0), else gone with everything.
    fn remove_locked(&self, path: &LogicalPath, folder: &LogicalPath, state: &mut Folder, old: &Rec) -> Result<(), Error> {
        let name = path.file_name().to_string();
        let n = if old.nv { 0 } else { self.versions_kept() };
        let mut gone = Vec::new();
        if old.k == 1 {
            gone.push(self.blob_place(path, Role::Lines, old.g, 0));
            gone.push(self.blob_place(path, Role::Lines, old.g, 1));
            self.record(folder, state, Op::D(name))?;
        } else {
            if old.tw {
                gone.push(self.blob_place(path, Role::File, old.g, 1));
            }
            let mut versions = Vec::new();
            if !old.d.contains(&old.g) {
                versions.push(old.as_version());
            }
            versions.extend(old.v.iter().cloned());
            for beyond in versions.split_off(n.min(versions.len())) {
                gone.push(self.blob_place(path, Role::File, beyond.g, 0));
            }
            if versions.is_empty() {
                gone.push(self.blob_place(path, Role::File, old.g, 0));
                self.record(folder, state, Op::D(name))?;
            } else {
                let rec = Rec { rm: true, tw: false, v: versions, ..old.clone() };
                self.record(folder, state, Op::P(name, rec))?;
            }
        }
        for place in gone {
            let _ = disk::remove(&place);
        }
        Ok(())
    }

    /// A file under another name: its content sealed again there (each blob
    /// is bound to its path). In one folder, one change; across two, written
    /// at `to` first, then taken from `from`: after a power cut, in both,
    /// never in neither. The old name keeps no earlier version.
    pub fn rename(&self, from: &LogicalPath, to: &LogicalPath, expect: Expect) -> Result<Written, Error> {
        Store::file_path(from)?;
        Store::file_path(to)?;
        if from == to {
            return Err(Error::Refused("a file renamed to its own name".to_string()));
        }
        let (source_folder, target_folder) = (from.folder(), to.folder());
        let _profile = self.hold()?;
        // Two folders' locks, always in one order: two renames the other way never wait on each other.
        let (first, second) = if source_folder <= target_folder { (&source_folder, &target_folder) } else { (&target_folder, &source_folder) };
        let _first = lock::take_folder(&self.folder_lock(first), true, false)?;
        let _second = if second != first { Some(lock::take_folder(&self.folder_lock(second), true, true)?) } else { None };
        let Loaded::Ready(mut source) = self.load(&source_folder, true)? else { return Err(Error::Damaged { path: source_folder.to_string(), what: "its list".to_string() }) };
        let old = source.entries.get(from.file_name()).cloned().filter(|rec| !rec.rm).ok_or(Error::Absent)?;
        Store::expect(Some(&old), expect)?;
        if old.k == 1 {
            return Err(Error::Refused(format!("{from} only grows: renaming it is not supported")));
        }
        let mut failed = Vec::new();
        let read = self.read_rec(from, &old, &mut failed)?;
        // Never an earlier version carried to the new name: the rename would roll the file back unsaid.
        if let Source::Earlier { .. } = read.from {
            return Err(Error::Damaged { path: from.to_string(), what: "its current copy does not open: put an earlier version back first (restore)".to_string() });
        }
        let bytes = read.bytes;
        let written = if source_folder == target_folder {
            // Sealed at `to`, then one line says both: atomic in one folder.
            let target_old = source.entries.get(to.file_name()).cloned();
            if target_old.as_ref().is_some_and(|rec| rec.k == 1 && !rec.rm) {
                return Err(Error::Refused(format!("{to} only grows: a file cannot be renamed over it")));
            }
            let generation = next_generation(source.top, target_old.as_ref());
            let written_ms = super::now_ms();
            let made = self.seal_blob(to, generation, 0, written_ms, !old.nc, &mut bytes.as_slice())?;
            let twin = if old.tw { Some(self.seal_blob(to, generation, 1, written_ms, !old.nc, &mut bytes.as_slice())?) } else { None };
            disk::crash(disk::Step::BeforeJournal).map_err(io(to))?;
            let how = How { only_here: old.tw, expect: Expect::Any, unversioned: old.nv, uncompressed: old.nc };
            let (mut rec, after) = self.successor(to, target_old.as_ref(), &made, twin.as_ref(), generation, written_ms, how);
            rec.a = old.a.clone();
            self.record(&source_folder, &mut source, Op::R(from.file_name().to_string(), to.file_name().to_string(), rec))?;
            self.tidy(to, after);
            Written { generation, size: made.plain }
        } else {
            let Loaded::Ready(mut target) = self.load(&target_folder, true)? else { return Err(Error::Damaged { path: target_folder.to_string(), what: "its list".to_string() }) };
            let how = How { only_here: old.tw, expect: Expect::Any, unversioned: old.nv, uncompressed: old.nc };
            let written = self.write_locked(to, &mut target, &mut bytes.as_slice(), bytes.len() as u64, how, None)?;
            disk::crash(disk::Step::AfterJournal).map_err(io(to))?;
            if !old.a.is_empty()
                && let Some(mut rec) = target.entries.get(to.file_name()).cloned()
            {
                rec.a = old.a.clone();
                self.record(&target_folder, &mut target, Op::P(to.file_name().to_string(), rec))?;
            }
            self.record(&source_folder, &mut source, Op::D(from.file_name().to_string()))?;
            written
        };
        // The old name's blobs: its content lives on at `to`.
        for place in std::iter::once(self.blob_place(from, Role::File, old.g, 0)).chain(old.tw.then(|| self.blob_place(from, Role::File, old.g, 1))).chain(old.v.iter().map(|v| self.blob_place(from, Role::File, v.g, 0))) {
            let _ = disk::remove(&place);
        }
        Ok(written)
    }

    /// Attributes changed (mail's flags, keywords, "new"): one line, the blob untouched.
    pub fn set_attrs(&self, path: &LogicalPath, changes: &[(&str, Option<&str>)]) -> Result<(), Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        let mut rec = state.entries.get(path.file_name()).cloned().filter(|rec| !rec.rm).ok_or(Error::Absent)?;
        let before = rec.a.clone();
        for (key, value) in changes {
            match value {
                Some(value) => rec.a.insert((*key).to_string(), (*value).to_string()),
                None => rec.a.remove(*key),
            };
        }
        if rec.a != before {
            self.record(&folder, &mut state, Op::P(path.file_name().to_string(), rec))?;
        }
        Ok(())
    }

    // Earlier versions, twins, purge.

    /// A file's versions, the newest first: its current one first when it is there.
    pub fn versions(&self, path: &LogicalPath) -> Result<Vec<Version>, Error> {
        Store::file_path(path)?;
        let (_held, state) = self.read_folder(&path.folder())?;
        let rec = state.entries.get(path.file_name()).ok_or(Error::Absent)?;
        let mut out = Vec::new();
        if !rec.rm {
            out.push(Version { generation: rec.g, size: rec.s, written_ms: rec.t, origin: rec.o.clone(), current: true });
        }
        out.extend(rec.v.iter().map(|v| Version { generation: v.g, size: v.s, written_ms: v.t, origin: v.o.clone(), current: false }));
        Ok(out)
    }

    pub fn read_version(&self, path: &LogicalPath, generation: u64) -> Result<Opened, Error> {
        Store::file_path(path)?;
        let (_held, state) = self.read_folder(&path.folder())?;
        let rec = state.entries.get(path.file_name()).ok_or(Error::Absent)?;
        if !(rec.v.iter().any(|v| v.g == generation) || (!rec.rm && rec.g == generation)) {
            return Err(Error::Absent);
        }
        let mut bytes = Vec::new();
        let copies: Vec<u8> = if rec.g == generation && rec.tw { vec![0, 1] } else { vec![0] };
        for copy in copies {
            bytes.clear();
            match self.open_checked(path, rec, generation, copy, &mut bytes) {
                Ok(_) => return Ok(Opened { bytes, generation, from: if rec.g == generation && !rec.rm { Source::Current } else { Source::Earlier { generation, written_ms: rec.v.iter().find(|v| v.g == generation).map_or(rec.t, |v| v.t) } } }),
                Err(Fault::Io(e)) => return Err(Error::Io(format!("{path}: {e}"))),
                Err(_) => {}
            }
        }
        Err(Error::Damaged { path: path.to_string(), what: format!("its version {generation} does not open") })
    }

    /// An earlier version put back as a new generation: what it replaces
    /// becomes an earlier version in turn, so a restore is undone by
    /// restoring that one. A removed file comes back so.
    pub fn restore(&self, path: &LogicalPath, generation: u64) -> Result<Written, Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        let rec = state.entries.get(path.file_name()).cloned().ok_or(Error::Absent)?;
        if !rec.v.iter().any(|v| v.g == generation) {
            return Err(Error::Absent);
        }
        let mut bytes = Vec::new();
        match self.open_checked(path, &rec, generation, 0, &mut bytes) {
            Ok(_) => {}
            Err(Fault::Io(e)) => return Err(Error::Io(format!("{path}: {e}"))),
            Err(_) => return Err(Error::Damaged { path: path.to_string(), what: format!("its version {generation} does not open") }),
        }
        self.restore_locked(path, &mut state, generation, &bytes)
    }

    /// A version's bytes put back as a new generation, the folder's lock held
    /// exclusive. The version restored stays among the versions: n counts from the new one.
    pub(crate) fn restore_locked(&self, path: &LogicalPath, state: &mut Folder, generation: u64, bytes: &[u8]) -> Result<Written, Error> {
        let rec = state.entries.get(path.file_name()).cloned().ok_or(Error::Absent)?;
        let version = rec.v.iter().find(|v| v.g == generation).cloned().ok_or(Error::Absent)?;
        let how = How { only_here: rec.tw, expect: Expect::Any, unversioned: rec.nv, uncompressed: rec.nc };
        self.write_locked(path, state, &mut &bytes[..], bytes.len() as u64, how, Some(&version))
    }

    /// The twin goes: the caller knows that generation has travelled.
    pub fn released(&self, path: &LogicalPath, generation: u64) -> Result<(), Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        let mut rec = state.entries.get(path.file_name()).cloned().filter(|rec| !rec.rm).ok_or(Error::Absent)?;
        if rec.g != generation || !rec.tw {
            return Ok(());
        }
        let role = if rec.k == 1 { Role::Lines } else { Role::File };
        rec.tw = false;
        rec.th.clear();
        rec.tz = 0;
        self.record(&folder, &mut state, Op::P(path.file_name().to_string(), rec))?;
        let _ = disk::remove(&self.blob_place(path, role, generation, 1));
        Ok(())
    }

    /// The file and every copy of it gone: every generation its folder ever
    /// gave probed, both copies, as a whole file and as one that only grows,
    /// and the damaged copies set aside; then its record dropped and its
    /// folder's list written again at once, so that no list names it. The
    /// blobs go first, whatever happens to the list (a full disk): a second
    /// purge finishes what a first could not.
    pub fn purge(&self, path: &LogicalPath) -> Result<Purged, Error> {
        Store::file_path(path)?;
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        let rec = state.entries.get(path.file_name()).cloned();
        let top = state.top.max(rec.as_ref().map_or(0, Rec::top));
        let mut purged = Purged::default();
        self.purge_blobs(path, top, &mut purged);
        disk::crash(disk::Step::BeforeJournal).map_err(io(path))?;
        if rec.is_none() {
            return if purged.removed + purged.left > 0 { Ok(purged) } else { Err(Error::Absent) };
        }
        self.record(&folder, &mut state, Op::D(path.file_name().to_string()))?;
        purged.folded = self.fold(&folder, &mut state).is_ok();
        Ok(purged)
    }

    /// Every blob a path ever had, below `top` and one past it: both copies,
    /// as a whole file and as one that only grows, and its copies set aside.
    fn purge_blobs(&self, path: &LogicalPath, top: u64, purged: &mut Purged) {
        let mut touched = BTreeSet::new();
        for generation in 1..=top + 1 {
            for copy in [0u8, 1] {
                let places = [self.blob_place(path, Role::File, generation, copy), self.blob_place(path, Role::Lines, generation, copy), self.aside_place(path, generation, copy)];
                for place in places {
                    match std::fs::remove_file(&place) {
                        Ok(()) => {
                            purged.removed += 1;
                            if let Some(dir) = place.parent() {
                                touched.insert(dir.to_path_buf());
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => purged.left += 1,
                    }
                }
            }
        }
        for dir in touched {
            let _ = disk::sync_dir(&dir);
        }
    }

    /// Everything ever written under a logical folder gone (a client's file
    /// erased): every file of every folder below it, the files removed earlier
    /// and their versions included, each generation the folders ever gave
    /// probed, and blobs no list names whose path lies below it; then the
    /// folders' own lists, so that nothing names them. Under the profile's
    /// lock held exclusive: nothing writes there meanwhile. A second call
    /// finishes what a first could not.
    pub fn purge_under(&self, under: &LogicalPath) -> Result<Purged, Error> {
        let _profile = ProfileLock::exclusive(&self.folders)?;
        let (found, folders, _) = self.survey(under.root())?;
        let mut purged = Purged { folded: true, ..Purged::default() };
        for folder in folders.into_iter().filter(|folder| folder.is_within(under)) {
            let _held = lock::take_folder(&self.folder_lock(&folder), true, false)?;
            let state = match self.load(&folder, true)? {
                Loaded::Ready(state) => Some(state),
                Loaded::NeedsRepair => None,
            };
            // Every name its list knows, and every name its blobs give.
            let mut names: BTreeSet<String> = state.as_ref().map(|s| s.entries.keys().cloned().collect()).unwrap_or_default();
            let mut top = state.as_ref().map_or(0, |s| s.top);
            for f in &found {
                if let Some(path) = LogicalPath::checked_any(&f.inner.path).filter(|p| !p.is_root() && p.folder() == folder) {
                    names.insert(path.file_name().to_string());
                    top = top.max(f.header.generation);
                }
            }
            for name in names {
                let path = folder.join(&name)?;
                let file_top = state.as_ref().and_then(|s| s.entries.get(&name)).map_or(0, Rec::top);
                self.purge_blobs(&path, top.max(file_top), &mut purged);
            }
            self.drop_folder(&folder)?;
        }
        Ok(purged)
    }

    // Files that only grow.

    fn grown_place(&self, path: &LogicalPath, generation: u64, copy: u8) -> PathBuf {
        self.blob_place(path, Role::Lines, generation, copy)
    }

    /// One line appended (no line end in it): its number, from 1. Written to
    /// its second copy too when it has one (`How::only_here`, on any append:
    /// the second copy is made then and kept until `released`), then its
    /// count written in the record, so that a cut at a line's end is seen.
    pub fn append_line(&self, path: &LogicalPath, line: &str, how: How) -> Result<u64, Error> {
        Store::file_path(path)?;
        if line.contains('\n') {
            return Err(Error::Refused("a line holds no line end".to_string()));
        }
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        let name = path.file_name().to_string();
        let mut rec = match state.entries.get(&name).cloned() {
            Some(rec) if rec.k == 0 && !rec.rm => return Err(Error::Refused(format!("{path} is written whole: write it whole"))),
            Some(rec) if !rec.rm => rec,
            old => {
                let generation = next_generation(state.top, old.as_ref());
                self.begin_lines(path, generation, 0, &[])?;
                Rec { g: generation, k: 1, ..Rec::default() }
            }
        };
        if how.only_here && !rec.tw {
            // Its second copy made now, with every line it holds.
            let (lines_now, _) = self.lines_of(path, &rec)?;
            self.begin_lines(path, rec.g, 1, &lines_now)?;
            rec.tw = true;
        }
        let copies: &[u8] = if rec.tw { &[0, 1] } else { &[0] };
        let mut at = Vec::new();
        let mut number = rec.lc + 1;
        for &copy in copies {
            let bind = self.name(Role::Lines, path.as_str(), rec.g, copy);
            let place = self.grown_place(path, rec.g, copy);
            let (valid, next) = match self.grown_state(path, &place, &bind, rec.g) {
                Ok(state) => state,
                // A copy gone: made again from what the other copy holds. Never
                // from nothing: when no copy reads, the file stays damaged (and
                // this append is refused) until `check` says otherwise.
                Err(Error::Damaged { .. }) => {
                    let (lines_now, _) = self.lines_of(path, &rec)?;
                    self.begin_lines(path, rec.g, copy, &lines_now)?;
                    self.grown_state(path, &place, &bind, rec.g)?
                }
                Err(e) => return Err(e),
            };
            number = number.max(next);
            at.push((copy, bind, valid));
        }
        for (copy, bind, valid) in at {
            let place = self.grown_place(path, rec.g, copy);
            let valid = lines::append(&place, valid, &self.keys.lines, &bind, rec.g, number, line.as_bytes()).map_err(io(path))?;
            if let Some(id) = disk::identity(&place) {
                self.grown.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(place, (id, valid, number + 1));
            }
        }
        rec.lc = number;
        rec.t = super::now_ms();
        rec.o = self.origin.clone();
        self.record(&folder, &mut state, Op::P(name, rec))?;
        Ok(number)
    }

    /// Where a copy of a file that only grows ends, and its next line's number: as read here before, or read now.
    fn grown_state(&self, path: &LogicalPath, place: &std::path::Path, bind: &str, generation: u64) -> Result<(u64, u64), Error> {
        let id = disk::identity(place);
        if let Some((kept, valid, next)) = self.grown.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(place).copied()
            && Some(kept) == id
        {
            return Ok((valid, next));
        }
        match lines::scan(place, &self.keys.lines, &|_, _| bind.to_string(), Some((Role::Lines, generation))) {
            Ok(scanned) => Ok((scanned.valid, scanned.next)),
            Err(Fault::Io(e)) => Err(Error::Io(format!("{path}: {e}"))),
            Err(_) => Err(Error::Damaged { path: path.to_string(), what: "its first line does not read".to_string() }),
        }
    }

    /// A copy of a file that only grows made anew, with these lines (their numbers kept).
    fn begin_lines(&self, path: &LogicalPath, generation: u64, copy: u8, lines_now: &[(u64, String)]) -> Result<(), Error> {
        let place = self.grown_place(path, generation, copy);
        let bind = self.name(Role::Lines, path.as_str(), generation, copy);
        let head = Inner { path: path.to_string(), written_ms: super::now_ms(), origin: self.origin.clone(), compressed: false };
        let mut text = format::lines_header(Role::Lines, generation);
        text.push_str(&format::seal_line(&self.keys.lines, &bind, generation, 0, &head.encode()).map_err(io(path))?);
        for (number, line) in lines_now {
            text.push_str(&format::seal_line(&self.keys.lines, &bind, generation, *number, line.as_bytes()).map_err(io(path))?);
        }
        disk::place(&place, |out| out.write_all(text.as_bytes())).map_err(io(path))
    }

    /// A file's lines from both its copies, by number, and the numbers no copy holds whole.
    pub(crate) fn lines_of(&self, path: &LogicalPath, rec: &Rec) -> Result<(Vec<(u64, String)>, Vec<u64>), Error> {
        let mut by_number: std::collections::BTreeMap<u64, String> = std::collections::BTreeMap::new();
        let mut highest = rec.lc;
        let mut read_any = false;
        let mut refused = None;
        for copy in if rec.tw { vec![0u8, 1] } else { vec![0u8] } {
            let bind = self.name(Role::Lines, path.as_str(), rec.g, copy);
            match lines::scan(&self.grown_place(path, rec.g, copy), &self.keys.lines, &|_, _| bind.clone(), Some((Role::Lines, rec.g))) {
                Ok(scanned) => {
                    read_any = true;
                    // Only lines that open count past the record's count: a flipped digit in a clear number makes no phantom line.
                    for (number, line) in scanned.lines {
                        if let (Some(number), Some(text)) = (number, line.and_then(|bytes| String::from_utf8(bytes).ok())) {
                            highest = highest.max(number);
                            by_number.entry(number).or_insert(text);
                        }
                    }
                }
                Err(Fault::Io(e)) => refused = Some(e),
                Err(_) => {}
            }
        }
        if !read_any {
            return Err(match refused {
                Some(e) => Error::Io(format!("{path}: {e}")),
                None => Error::Damaged { path: path.to_string(), what: "no copy of it reads".to_string() },
            });
        }
        let damaged = (1..=highest).filter(|n| !by_number.contains_key(n)).collect();
        Ok((by_number.into_iter().collect(), damaged))
    }

    /// A file that only grows: every line that opens, by number; the numbers
    /// of those that do not, or that are gone (a cut at a line's end, seen by
    /// the count its record keeps).
    pub fn read_lines(&self, path: &LogicalPath) -> Result<Option<Lines>, Error> {
        Store::file_path(path)?;
        let (_held, state) = self.read_folder(&path.folder())?;
        let Some(rec) = state.entries.get(path.file_name()).filter(|rec| !rec.rm) else { return Ok(None) };
        if rec.k != 1 {
            return Err(Error::Refused(format!("{path} is written whole: read it whole")));
        }
        let (lines, damaged) = self.lines_of(path, rec)?;
        Ok(Some(Lines { lines, damaged }))
    }

    /// A file that only grows written anew (trimmed, rotated): a new generation, whole or not at all, numbered from 1.
    pub fn rewrite_lines(&self, path: &LogicalPath, lines_now: &[String], how: How) -> Result<Written, Error> {
        Store::file_path(path)?;
        if lines_now.iter().any(|line| line.contains('\n')) {
            return Err(Error::Refused("a line holds no line end".to_string()));
        }
        let folder = path.folder();
        let (_profile, _held, mut state) = self.change_folder(&folder)?;
        let name = path.file_name().to_string();
        let old = self.caught_up(path, &mut state)?;
        if old.as_ref().is_some_and(|rec| rec.k == 0 && !rec.rm) {
            return Err(Error::Refused(format!("{path} is written whole: write it whole")));
        }
        let generation = next_generation(state.top, old.as_ref());
        let numbered: Vec<(u64, String)> = lines_now.iter().enumerate().map(|(n, line)| (n as u64 + 1, line.clone())).collect();
        let twin = how.only_here || old.as_ref().is_some_and(|rec| rec.tw && !rec.rm);
        self.begin_lines(path, generation, 0, &numbered)?;
        if twin {
            self.begin_lines(path, generation, 1, &numbered)?;
        }
        self.record(&folder, &mut state, Op::P(name, Rec { g: generation, k: 1, t: super::now_ms(), o: self.origin.clone(), tw: twin, lc: numbered.len() as u64, ..Rec::default() }))?;
        if let Some(old) = old.filter(|rec| rec.k == 1) {
            for copy in [0u8, 1] {
                let _ = disk::remove(&self.grown_place(path, old.g, copy));
            }
        }
        Ok(Written { generation, size: lines_now.iter().map(|l| l.len() as u64 + 1).sum() })
    }

    // The person's folders inside Sioul's that stay in clear.

    /// The person's folders (notes, papers, letters) that lie inside Sioul's
    /// own folders and stay in clear there: those the move found there, which
    /// it never seals where they are (docs/store.md, "The move"). Sealed in
    /// the store, read once per process.
    pub fn clear_folders(&self) -> Vec<LogicalPath> {
        let mut clear = self.clear.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(known) = clear.as_ref() {
            return known.clone();
        }
        let read: Vec<LogicalPath> = match self.read(&LogicalPath::internal("clear")) {
            Ok(Some(opened)) => String::from_utf8_lossy(&opened.bytes).lines().filter_map(|l| LogicalPath::new(l).ok()).collect(),
            _ => Vec::new(),
        };
        *clear = Some(read.clone());
        read
    }

    /// Whether a logical path lies in one of the person's folders kept in clear.
    pub fn kept_clear(&self, path: &LogicalPath) -> bool {
        self.clear_folders().iter().any(|folder| path.is_within(folder))
    }

    /// A folder of the person's, inside Sioul's, kept in clear from now on.
    pub fn keep_in_clear(&self, folder: &LogicalPath) -> Result<(), Error> {
        let mut all = self.clear_folders();
        if all.iter().any(|known| folder.is_within(known)) {
            return Ok(());
        }
        all.push(folder.clone());
        let text: String = all.iter().map(|f| format!("{f}\n")).collect();
        self.write(&LogicalPath::internal("clear"), text.as_bytes(), How::only_here())?;
        *self.clear.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(all);
        Ok(())
    }

    // The move's progress: sealed, never going back.

    /// The stores marked moved: what every copy of the record says, together
    /// (a copy or a version put back that says less never takes one back).
    pub fn moved(&self) -> Result<BTreeSet<String>, Error> {
        let path = LogicalPath::internal("moved");
        let (_held, state) = self.read_folder(&path.folder())?;
        let Some(rec) = state.entries.get(path.file_name()) else { return Ok(BTreeSet::new()) };
        let mut all = BTreeSet::new();
        let mut opened_any = false;
        let mut copies: Vec<(u64, u8)> = vec![(rec.g, 0), (rec.g, 1)];
        copies.extend(rec.v.iter().map(|v| (v.g, 0)));
        for (generation, copy) in copies {
            let mut bytes = Vec::new();
            if self.open_blob(&path, generation, copy, &mut bytes).is_ok() {
                opened_any = true;
                all.extend(String::from_utf8_lossy(&bytes).lines().filter(|l| !l.is_empty()).map(str::to_string));
            }
        }
        if !opened_any {
            return Err(Error::Damaged { path: "the move's progress".to_string(), what: "no copy of it opens".to_string() });
        }
        Ok(all)
    }

    /// A store marked moved, for good.
    pub fn mark_moved(&self, store: &str) -> Result<(), Error> {
        if store.is_empty() || store.contains('\n') {
            return Err(Error::Refused("a store's name is one word".to_string()));
        }
        let mut all = match self.moved() {
            Ok(all) => all,
            Err(Error::Damaged { .. }) => BTreeSet::new(),
            Err(e) => return Err(e),
        };
        all.insert(store.to_string());
        let text: String = all.iter().map(|s| format!("{s}\n")).collect();
        self.write(&LogicalPath::internal("moved"), text.as_bytes(), How::only_here())?;
        self.moved_seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).1.insert(store.to_string());
        Ok(())
    }

    // Collection.

    /// Every blob of a root that opens with this store's key (its head), the
    /// folders it holds (its list, and those its blobs name, so that a folder
    /// a list put back older forgot is found all the same), and the names of
    /// its root lists.
    pub(crate) fn survey(&self, root: Root) -> Result<(Vec<Found>, BTreeSet<LogicalPath>, BTreeSet<String>), Error> {
        let found = self.scan_blobs(root);
        let mut root_names = vec![root.name().to_string()];
        if root == Root::Config {
            root_names.push(super::INTERNAL.to_string());
        }
        let mut folders = BTreeSet::new();
        let mut named = BTreeSet::new();
        for root_name in &root_names {
            for copy in [0u8, 1] {
                named.insert(self.name(Role::Root, root_name, 0, copy));
            }
            folders.extend(self.root_folders(root_name, root)?.1.iter().filter_map(|f| LogicalPath::checked_any(f)));
        }
        for f in &found {
            let path = LogicalPath::checked_any(&f.inner.path);
            match f.header.role {
                Role::File | Role::Lines => folders.extend(path.filter(|p| !p.is_root()).map(|p| p.folder())),
                Role::Manifest => folders.extend(path),
                _ => {}
            }
        }
        Ok((found, folders, named))
    }

    /// The files of `folder` with a blob newer than their record says: an
    /// interrupted write, or a list put back older. A file no record names
    /// counts only past the folder's highest generation (below it, a file
    /// removed or purged left it). By name: the newest such generation.
    pub(crate) fn newer_than_list(folder: &LogicalPath, state: &Folder, found: &[Found]) -> BTreeMap<String, u64> {
        let mut newer: BTreeMap<String, u64> = BTreeMap::new();
        // Files that only grow too: a log exists nowhere else, and one newer than its list is never taken for a leftover.
        for f in found.iter().filter(|f| matches!(f.header.role, Role::File | Role::Lines) && f.header.copy == 0) {
            let Some(path) = LogicalPath::checked_any(&f.inner.path).filter(|p| !p.is_root() && p.folder() == *folder) else { continue };
            let past = state.entries.get(path.file_name()).map_or(state.top, Rec::top);
            if f.header.generation > past {
                let entry = newer.entry(path.file_name().to_string()).or_default();
                *entry = (*entry).max(f.header.generation);
            }
        }
        newer
    }

    /// A blob newer than its list taken as its file's content (never removed):
    /// what the record named becomes an earlier version. False when it does not open.
    pub(crate) fn adopt(&self, folder: &LogicalPath, state: &mut Folder, name: &str, generation: u64) -> Result<bool, Error> {
        let path = folder.join(name)?;
        if self.blob_place(&path, Role::Lines, generation, 0).exists() {
            return self.adopt_lines(folder, state, &path, generation);
        }
        let Ok(got) = self.open_blob(&path, generation, 0, &mut std::io::sink()) else { return Ok(false) };
        let old = state.entries.get(name).cloned();
        let mut rec = Rec { g: generation, s: got.plain, z: got.sealed_size, h: got.hash, t: got.inner.written_ms, o: got.inner.origin, nc: !got.inner.compressed, ..Rec::default() };
        if let Some(old) = old {
            rec.a = old.a.clone();
            rec.nv = old.nv;
            let keep = if old.nv { 0 } else { self.versions_kept().max(1) };
            let mut versions: Vec<Ver> = Vec::new();
            if !old.rm && !old.d.contains(&old.g) {
                versions.push(old.as_version());
            }
            versions.extend(old.v.iter().filter(|v| !old.d.contains(&v.g)).cloned());
            versions.truncate(keep);
            rec.v = versions;
        }
        self.record(folder, state, Op::P(name.to_string(), rec))?;
        Ok(true)
    }

    /// A file that only grows, newer than its list, taken as it is: its
    /// count from the highest line that opens, its second copy when there.
    /// What the list named before (a whole file, an older log) is left to the
    /// sweep of what no list names, as a rewrite would.
    fn adopt_lines(&self, folder: &LogicalPath, state: &mut Folder, path: &LogicalPath, generation: u64) -> Result<bool, Error> {
        let bind = self.name(Role::Lines, path.as_str(), generation, 0);
        let Ok(scanned) = lines::scan(&self.blob_place(path, Role::Lines, generation, 0), &self.keys.lines, &|_, _| bind.clone(), Some((Role::Lines, generation))) else { return Ok(false) };
        let count = scanned.lines.iter().filter(|(_, plain)| plain.is_some()).filter_map(|(number, _)| *number).max().unwrap_or(0);
        let written_ms = scanned.head.as_ref().map_or_else(super::now_ms, |head| head.written_ms);
        let origin = scanned.head.map_or_else(|| self.origin.clone(), |head| head.origin);
        let twin = self.blob_place(path, Role::Lines, generation, 1).exists();
        let rec = Rec { g: generation, k: 1, t: written_ms, o: origin, tw: twin, lc: count, ..Rec::default() };
        self.record(folder, state, Op::P(path.file_name().to_string(), rec))?;
        Ok(true)
    }

    /// The whole collection, under the profile's lock held exclusive (writers
    /// wait meanwhile): temporary names, damaged copies set aside 30 days ago,
    /// blobs no list names (only those that open with this store's key, once
    /// any blob newer than its list was taken as its file's content), removed
    /// files with nothing left, earlier versions beyond n and past the cap (a
    /// GB, or a twentieth of the disk if less, in each of the four folders), the
    /// oldest first; never a twin, never a file's last good copy.
    pub fn collect(&self) -> Result<Collected, Error> {
        let _profile = ProfileLock::exclusive(&self.folders)?;
        if let Some(n) = self.versions_kept_in_store() {
            self.versions.store(n, Ordering::Relaxed);
        }
        let mut collected = Collected::default();
        let now = super::now_ms();
        for root in Root::ALL {
            let sealed = self.folders.sealed(root);
            // No write is under way: every temporary name is an interrupted write's.
            collected.temporaries += sweep_old(&sealed, now, -1, true);
            collected.temporaries += sweep_old(&sealed.join(".aside"), now, ASIDE_DAYS * 86_400_000, false);
            let (found, folders, mut named) = self.survey(root)?;
            for folder in folders {
                let _held = lock::take_folder(&self.folder_lock(&folder), true, false)?;
                let Loaded::Ready(mut state) = self.load(&folder, true)? else { continue };
                for (name, generation) in Store::newer_than_list(&folder, &state, &found) {
                    if self.adopt(&folder, &mut state, &name, generation)? {
                        collected.adopted += 1;
                    }
                }
                let n = self.versions_kept();
                let names: Vec<String> = state.entries.keys().cloned().collect();
                for name in names {
                    let Some(rec) = state.entries.get(&name).cloned() else { continue };
                    let path = folder.join(&name)?;
                    if rec.rm && rec.v.is_empty() {
                        self.record(&folder, &mut state, Op::D(name))?;
                        continue;
                    }
                    let keep = if rec.nv { 0 } else { n };
                    // A file's last good copy stays: the newest version not damaged, while its current copy is.
                    let protect = (!rec.rm && rec.d.contains(&rec.g)).then(|| rec.v.iter().find(|v| !rec.d.contains(&v.g)).map(|v| v.g)).flatten();
                    let (kept, beyond): (Vec<(usize, Ver)>, Vec<(usize, Ver)>) = rec.v.iter().cloned().enumerate().partition(|(i, v)| *i < keep || Some(v.g) == protect);
                    if beyond.is_empty() {
                        continue;
                    }
                    let mut trimmed = rec.clone();
                    trimmed.v = kept.into_iter().map(|(_, v)| v).collect();
                    let op = if trimmed.rm && trimmed.v.is_empty() { Op::D(name.clone()) } else { Op::P(name.clone(), trimmed) };
                    self.record(&folder, &mut state, op)?;
                    for (_, v) in beyond {
                        let place = self.blob_place(&path, Role::File, v.g, 0);
                        let size = std::fs::metadata(&place).map_or(0, |m| m.len());
                        if disk::remove(&place).is_ok() {
                            collected.versions += 1;
                            collected.bytes += size;
                        }
                    }
                }
                for copy in [0u8, 1] {
                    named.insert(self.name(Role::Manifest, folder.as_str(), 0, copy));
                }
                named.insert(self.name(Role::Journal, folder.as_str(), 0, 0));
                for (name, rec) in &state.entries {
                    let path = format!("{folder}/{name}");
                    let role = if rec.k == 1 { Role::Lines } else { Role::File };
                    named.insert(self.name(role, &path, rec.g, 0));
                    named.insert(self.name(role, &path, rec.g, 1));
                    for v in &rec.v {
                        named.insert(self.name(Role::File, &path, v.g, 0));
                    }
                }
                if state.entries.is_empty() && !found.iter().any(|f| matches!(f.header.role, Role::File | Role::Lines) && LogicalPath::checked_any(&f.inner.path).is_some_and(|p| p.folder() == folder)) {
                    self.drop_folder(&folder)?;
                }
            }
            // Blobs that open with this key and that no list names: leftovers of a removal, a purge, a rename.
            for f in &found {
                if !named.contains(&f.name) && disk::remove(&format::place(&sealed, &f.name)).is_ok() {
                    collected.unlisted += 1;
                }
            }
            let cap = disk::versions_cap(&sealed);
            let (versions, bytes) = self.collect_versions(root, None, &|total| total <= cap)?;
            collected.versions += versions;
            collected.bytes += bytes;
        }
        Ok(collected)
    }

    /// Earlier versions taken out, the oldest first, until `enough` says so
    /// (given what all versions of the root take now): never a twin, never a
    /// file's last good copy (the newest version not damaged of a file whose
    /// current copy is). `held`: the folder whose lock the caller holds; the
    /// others are taken only when free (never waited for: no two writers wait on each other).
    pub(crate) fn collect_versions(&self, root: Root, mut held: Option<(&LogicalPath, &mut Folder)>, enough: &dyn Fn(u64) -> bool) -> Result<(usize, u64), Error> {
        let mut root_names = vec![root.name().to_string()];
        if root == Root::Config {
            root_names.push(super::INTERNAL.to_string());
        }
        // Every version that may go: (when, folder, file, generation, size).
        let mut candidates: Vec<(i64, String, String, u64, u64)> = Vec::new();
        let mut total = 0u64;
        let mut locks = Vec::new();
        let mut states: HashMap<String, Folder> = HashMap::new();
        for root_name in &root_names {
            let (_, folders) = self.root_folders(root_name, root)?;
            for folder_name in folders {
                let Some(folder) = LogicalPath::checked_any(&folder_name) else { continue };
                let state = if held.as_ref().is_some_and(|(f, _)| **f == folder) {
                    held.as_ref().map(|(_, s)| (**s).clone()).expect("held")
                } else {
                    let Some(lock) = lock::try_exclusive(&self.folder_lock(&folder))? else { continue };
                    let Loaded::Ready(state) = self.load(&folder, true)? else { continue };
                    locks.push(lock);
                    state
                };
                for (name, rec) in &state.entries {
                    let protect = (!rec.rm && rec.d.contains(&rec.g)).then(|| rec.v.iter().find(|v| !rec.d.contains(&v.g)).map(|v| v.g)).flatten();
                    for v in &rec.v {
                        total += v.z;
                        if Some(v.g) != protect {
                            candidates.push((v.t, folder_name.clone(), name.clone(), v.g, v.z));
                        }
                    }
                }
                states.insert(folder_name, state);
            }
        }
        candidates.sort();
        let (mut count, mut bytes) = (0usize, 0u64);
        for (_, folder_name, name, generation, size) in candidates {
            if enough(total) {
                break;
            }
            let Some(folder) = LogicalPath::checked_any(&folder_name) else { continue };
            let path = LogicalPath(format!("{folder_name}/{name}"));
            let is_held = held.as_ref().is_some_and(|(f, _)| **f == folder);
            let state = if is_held { held.as_mut().map(|(_, s)| &mut **s).expect("held") } else { states.get_mut(&folder_name).expect("loaded") };
            let Some(mut rec) = state.entries.get(&name).cloned() else { continue };
            rec.v.retain(|v| v.g != generation);
            let op = if rec.rm && rec.v.is_empty() { Op::D(name.clone()) } else { Op::P(name.clone(), rec) };
            self.record(&folder, state, op)?;
            let place = self.blob_place(&path, Role::File, generation, 0);
            if disk::remove(&place).is_ok() {
                disk::freed(size);
                count += 1;
                bytes += size;
                total = total.saturating_sub(size);
            }
        }
        drop(locks);
        Ok((count, bytes))
    }
}

/// What a change leaves to take out once its line is written: blobs to remove, damaged copies to set aside.
#[derive(Default)]
struct Tidy {
    gone: Vec<PathBuf>,
    aside: Vec<(u64, u8)>,
}

/// A new content's generation: past every one its folder ever gave, so that
/// a generation never comes back (an old blob of a path removed, purged or
/// renamed never opens as a new file there).
fn next_generation(top: u64, old: Option<&Rec>) -> u64 {
    top.max(old.map_or(0, Rec::top)) + 1
}

/// Temporary names (or, `temporaries` false, any file) older than `age` in a `sealed/` folder's fan-out folders (or in the folder itself), removed.
fn sweep_old(dir: &std::path::Path, now: i64, age: i64, temporaries: bool) -> usize {
    let mut removed = 0;
    let mut dirs = vec![dir.to_path_buf()];
    if temporaries {
        dirs.extend(std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && e.file_name().len() == 2).map(|e| e.path()));
    }
    for dir in dirs {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
            let name = entry.file_name();
            if temporaries && !name.to_string_lossy().starts_with(disk::TEMPORARY) {
                continue;
            }
            if !entry.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let modified = entry.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
            if (age < 0 || now - modified > age) && std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

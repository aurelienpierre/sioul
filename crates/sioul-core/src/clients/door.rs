// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The door the clients' files go through (docs/clients.md, "The store").
//!
//! Every part of a client's file is a file of the sealed store (docs/store.md)
//! under the logical folder `data/clients/`: the record, the description,
//! each note, each row, each attachment and each tie a file of its own, so
//! that a damaged file costs that one and nothing else. The clients' code
//! reaches the store through this small trait, shaped on the store's own
//! calls (a read gives a generation; a write may expect one; a listing is one
//! folder; a purge leaves nothing).
//!
//! **A failure is never an empty value**: `Ok(None)` means the file was never
//! written or was removed; a file that does not open is `Error::Damaged`, and
//! the caller keeps what it has.
//!
//! Implementations: `Sealed`, the profile's sealed store (the one every
//! program reaches, `opening`); `Memory`, for tests. **Nothing of a client
//! is ever written in clear**: on a profile that holds no store (a phone
//! until its store is turned on, a profile a sync app carries) there is no
//! door at all, and every hook does nothing (`Opening::NoStore`); while the
//! store cannot be opened (the keyring locked or late) every hook waits
//! (`Opening::Wait`), never taking that for "no client".

use super::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// What a write expects to find, as the store's `Expect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// Whatever is there.
    Any,
    /// Nothing: a new file.
    Absent,
    /// The generation read: someone else's write meanwhile is `Error::Changed`.
    Generation(u64),
}

/// A file read, with its generation.
///
/// A generation is a token for `Expect::Generation` alone (a safe overwrite
/// of what was read): the store gives them per folder and never gives one
/// back, so one is never compared across folders nor kept as a clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub bytes: Vec<u8>,
    pub generation: u64,
}

/// The store's calls the clients' files need.
pub trait Door: Send + Sync {
    /// A file; `Ok(None)` when it was never written or was removed.
    fn read(&self, path: &str) -> Result<Option<Opened>, Error>;
    /// A file written whole, atomically, keeping its earlier versions; its new generation.
    fn write(&self, path: &str, bytes: &[u8], expect: Expect) -> Result<u64, Error>;
    /// A file that never changes and has a source elsewhere (a copy of a
    /// mail's attachment): written with no earlier versions, its removal
    /// final (the store's `How::unversioned`). The stand-ins write it as any file.
    fn write_unversioned(&self, path: &str, bytes: &[u8], expect: Expect) -> Result<u64, Error> {
        self.write(path, bytes, expect)
    }
    /// A file taken out (its earlier versions kept by the store); false when it was not there.
    fn remove(&self, path: &str) -> Result<bool, Error>;
    /// The files directly in a folder, by their logical paths, sorted; a folder never written: empty.
    fn list(&self, folder: &str) -> Result<Vec<String>, Error>;
    /// Every folder below `under` that holds a file, sorted.
    fn folders(&self, under: &str) -> Result<Vec<String>, Error>;
    /// A file taken out with every earlier version of it: nothing of it is left here.
    fn purge(&self, path: &str) -> Result<bool, Error>;
    /// Everything ever written under a folder purged, files removed earlier
    /// included (their earlier versions too): what erasing a client needs.
    /// How many files went.
    fn purge_under(&self, folder: &str) -> Result<usize, Error>;

    /// Read, change, write, again while someone else wrote meanwhile (a few
    /// times at most): `change` gets the current bytes (None: absent) and
    /// gives the new ones (None: leave it as it is). Returns whether it wrote.
    fn update(&self, path: &str, change: &mut dyn FnMut(Option<&[u8]>) -> Result<Option<Vec<u8>>, Error>) -> Result<bool, Error> {
        for _ in 0..8 {
            let now = self.read(path)?;
            let expect = now.as_ref().map_or(Expect::Absent, |o| Expect::Generation(o.generation));
            let Some(bytes) = change(now.as_ref().map(|o| o.bytes.as_slice()))? else { return Ok(false) };
            match self.write(path, &bytes, expect) {
                Ok(_) => return Ok(true),
                Err(Error::Changed) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Error::Changed)
    }

    /// A file read as text; a file that is not UTF-8 is damaged.
    fn read_text(&self, path: &str) -> Result<Option<(String, u64)>, Error> {
        match self.read(path)? {
            None => Ok(None),
            Some(o) => String::from_utf8(o.bytes).map(|t| Some((t, o.generation))).map_err(|_| Error::Damaged(path.to_string())),
        }
    }
}

/// The parent of a logical path.
pub fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// The last part of a logical path.
pub fn name_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// A logical path the door takes: parts of letters, digits, `-`, `_` and
/// `.`, none empty, none `.` or `..`. The clients' code builds its paths of
/// ids alone; anything else is refused before it reaches a store.
pub fn checked(path: &str) -> Result<&str, Error> {
    let fine = !path.is_empty()
        && path.len() <= 1024
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != ".." && part.len() <= 255 && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')));
    if fine { Ok(path) } else { Err(Error::Refused("path")) }
}

// ---------------------------------------------------------------- the program's door

static INSTALLED: std::sync::RwLock<Option<std::sync::Arc<dyn Door>>> = std::sync::RwLock::new(None);

/// Whether the clients' files travel from this device to another that holds
/// them (the part on here, and another device whose files are sealed with
/// its part on: `set_travels`): a file written then exists there soon, so the
/// store writes one copy; until then it exists nowhere else and is written
/// twice (`How::only_here`, the store's twin).
static TRAVELS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A door put in place of the profile's store (a test, a program with a
/// store of its own); `uninstall` lets it go.
pub fn install(door: std::sync::Arc<dyn Door>) {
    *INSTALLED.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(door);
}

/// The door put in place let go: the profile's store again.
pub fn uninstall() {
    *INSTALLED.write().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// Whether clients' files travel from here now (said by the sharing after each exchange).
pub fn set_travels(travels: bool) {
    TRAVELS.store(travels, std::sync::atomic::Ordering::Relaxed);
}

/// Whether clients' files travel from here now.
pub fn travels() -> bool {
    TRAVELS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Where the clients' files stand in this program.
#[derive(Clone)]
pub enum Opening {
    /// The store is open: its door.
    Open(std::sync::Arc<dyn Door>),
    /// This profile is sealed and its store cannot be opened yet (the
    /// keyring locked, or late to answer): wait. Nothing is made, moved or
    /// decided absent meanwhile, and what protects fails closed.
    Wait,
    /// This profile holds no store (a phone until its store is turned on, a
    /// profile in clear): no client is kept here, nothing of one is written.
    NoStore,
}

impl Opening {
    /// Its door, or `Error::NotOpen`.
    pub fn door(&self) -> Result<std::sync::Arc<dyn Door>, Error> {
        match self {
            Opening::Open(door) => Ok(door.clone()),
            Opening::Wait | Opening::NoStore => Err(Error::NotOpen),
        }
    }
}

/// What the store's resolution found, as the clients' door: `Open`, `NoStore`, `Wait`.
pub fn opening_of(found: Result<Option<std::sync::Arc<crate::sealed::Store>>, crate::sealed::Error>) -> Opening {
    match found {
        Ok(Some(store)) => Opening::Open(std::sync::Arc::new(Sealed::new(store))),
        Ok(None) => Opening::NoStore,
        Err(_) => Opening::Wait,
    }
}

/// The clients' files in this program: a door put in place, else the
/// profile's own store as this program reaches it now (`files::profile_store`:
/// mounted by the window, opened at the first need by the command line and
/// the agents' server), asked at each call, so that a store let go is never
/// written through.
pub fn opening() -> Opening {
    if let Some(door) = INSTALLED.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone() {
        return Opening::Open(door);
    }
    opening_of(crate::sealed::files::profile_store())
}

/// The clients' door of this program: `Error::NotOpen` while there is none
/// (`opening`). Nothing of a client is written anywhere before.
pub fn opened() -> Result<std::sync::Arc<dyn Door>, Error> {
    opening().door()
}

// ---------------------------------------------------------------- in memory

/// A door in memory, for tests: generations count up; a file may be marked
/// damaged (`damage`) to see that a reader keeps what it has.
#[derive(Default)]
pub struct Memory {
    files: Mutex<BTreeMap<String, Opened>>,
    /// Files removed whose earlier versions the store still keeps.
    removed: Mutex<BTreeSet<String>>,
    damaged: Mutex<BTreeSet<String>>,
    /// Every path purged, in order: what a test checks an erase reached.
    pub purged: Mutex<Vec<String>>,
    next: Mutex<u64>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory::default()
    }

    /// The file at `path` no longer opens.
    pub fn damage(&self, path: &str) {
        self.damaged.lock().expect("lock").insert(path.to_string());
    }

    /// Every path held, sorted.
    pub fn paths(&self) -> Vec<String> {
        self.files.lock().expect("lock").keys().cloned().collect()
    }

    /// Paths removed whose earlier versions are still kept.
    pub fn removed_kept(&self) -> Vec<String> {
        self.removed.lock().expect("lock").iter().cloned().collect()
    }
}

impl Door for Memory {
    fn read(&self, path: &str) -> Result<Option<Opened>, Error> {
        let path = checked(path)?;
        if self.damaged.lock().expect("lock").contains(path) {
            return Err(Error::Damaged(path.to_string()));
        }
        Ok(self.files.lock().expect("lock").get(path).cloned())
    }

    fn write(&self, path: &str, bytes: &[u8], expect: Expect) -> Result<u64, Error> {
        let path = checked(path)?;
        let mut files = self.files.lock().expect("lock");
        let now = files.get(path).map(|o| o.generation);
        match (expect, now) {
            (Expect::Absent, Some(_)) => return Err(Error::Changed),
            (Expect::Generation(g), now) if now != Some(g) => return Err(Error::Changed),
            _ => {}
        }
        let mut next = self.next.lock().expect("lock");
        *next += 1;
        files.insert(path.to_string(), Opened { bytes: bytes.to_vec(), generation: *next });
        self.damaged.lock().expect("lock").remove(path);
        Ok(*next)
    }

    fn remove(&self, path: &str) -> Result<bool, Error> {
        let path = checked(path)?;
        let was = self.files.lock().expect("lock").remove(path).is_some();
        if was {
            self.removed.lock().expect("lock").insert(path.to_string());
        }
        Ok(was)
    }

    fn list(&self, folder: &str) -> Result<Vec<String>, Error> {
        let folder = checked(folder)?;
        Ok(self.files.lock().expect("lock").keys().filter(|p| folder_of(p) == folder).cloned().collect())
    }

    fn folders(&self, under: &str) -> Result<Vec<String>, Error> {
        let under = checked(under)?;
        let prefix = format!("{under}/");
        let found: BTreeSet<String> = self.files.lock().expect("lock").keys().filter(|p| p.starts_with(&prefix)).map(|p| folder_of(p).to_string()).filter(|f| f.len() > under.len()).collect();
        Ok(found.into_iter().collect())
    }

    fn purge(&self, path: &str) -> Result<bool, Error> {
        let path = checked(path)?;
        self.purged.lock().expect("lock").push(path.to_string());
        self.damaged.lock().expect("lock").remove(path);
        self.removed.lock().expect("lock").remove(path);
        Ok(self.files.lock().expect("lock").remove(path).is_some())
    }

    fn purge_under(&self, folder: &str) -> Result<usize, Error> {
        let prefix = format!("{}/", checked(folder)?);
        let mut files = self.files.lock().expect("lock");
        let gone: Vec<String> = files.keys().filter(|p| p.starts_with(&prefix)).cloned().collect();
        for p in &gone {
            files.remove(p);
        }
        let mut removed = self.removed.lock().expect("lock");
        let earlier: Vec<String> = removed.iter().filter(|p| p.starts_with(&prefix)).cloned().collect();
        for p in &earlier {
            removed.remove(p);
        }
        self.purged.lock().expect("lock").extend(gone.iter().chain(&earlier).cloned());
        Ok(gone.len())
    }
}

// ---------------------------------------------------------------- the sealed store

/// The profile's sealed store as the clients' door (docs/store.md): each
/// logical path the store's own, each call one of its calls; written as one
/// copy while the part travels from here, twice otherwise (`travels`). Its
/// errors keep no content: a path in them is made of ids alone.
pub struct Sealed {
    store: std::sync::Arc<crate::sealed::Store>,
}

impl Sealed {
    pub fn new(store: std::sync::Arc<crate::sealed::Store>) -> Sealed {
        Sealed { store }
    }

    fn how(expect: Expect) -> crate::sealed::How {
        let expect = match expect {
            Expect::Any => crate::sealed::Expect::Any,
            Expect::Absent => crate::sealed::Expect::Absent,
            Expect::Generation(g) => crate::sealed::Expect::Generation(g),
        };
        crate::sealed::How { only_here: !travels(), expect, ..crate::sealed::How::default() }
    }
}

fn logical(path: &str) -> Result<crate::sealed::LogicalPath, Error> {
    crate::sealed::LogicalPath::new(checked(path)?).map_err(|_| Error::Refused("path"))
}

/// The store's error as the clients': one to one, never a content.
fn of_store(path: &str, e: crate::sealed::Error) -> Error {
    use crate::sealed::Error as S;
    match e {
        S::NotOpen | S::NotCreated | S::WrongKey | S::Synced(_) => Error::NotOpen,
        S::Absent => Error::NotFound,
        S::Damaged { .. } | S::NotText => Error::Damaged(path.to_string()),
        S::Changed { .. } => Error::Changed,
        S::Locked(_) => Error::Locked,
        S::NoRoom => Error::NoRoom,
        S::Refused(_) => Error::Refused("path"),
        S::Exists | S::Change(_) | S::Io(_) => Error::Io(format!("{path}: the store refused")),
    }
}

impl Door for Sealed {
    fn read(&self, path: &str) -> Result<Option<Opened>, Error> {
        let at = logical(path)?;
        match self.store.read(&at) {
            Ok(found) => Ok(found.map(|o| Opened { bytes: o.bytes, generation: o.generation })),
            Err(e) => Err(of_store(path, e)),
        }
    }

    fn write(&self, path: &str, bytes: &[u8], expect: Expect) -> Result<u64, Error> {
        let at = logical(path)?;
        self.store.write(&at, bytes, Sealed::how(expect)).map(|w| w.generation).map_err(|e| of_store(path, e))
    }

    fn write_unversioned(&self, path: &str, bytes: &[u8], expect: Expect) -> Result<u64, Error> {
        let at = logical(path)?;
        let how = crate::sealed::How { unversioned: true, ..Sealed::how(expect) };
        self.store.write(&at, bytes, how).map(|w| w.generation).map_err(|e| of_store(path, e))
    }

    fn remove(&self, path: &str) -> Result<bool, Error> {
        let at = logical(path)?;
        self.store.remove(&at, crate::sealed::Expect::Any).map_err(|e| of_store(path, e))
    }

    /// Only names the clients' code makes (`checked`): a file another hand
    /// put there (a sharing's conflict copy, "client (conflict …).toml") is
    /// kept in the store and written out by the export, never read as a part.
    fn list(&self, folder: &str) -> Result<Vec<String>, Error> {
        let at = logical(folder)?;
        let mut out: Vec<String> = self.store.list(&at).map_err(|e| of_store(folder, e))?.into_iter().filter(|e| e.kind == crate::sealed::Kind::File).map(|e| e.path.as_str().to_string()).filter(|p| checked(p).is_ok()).collect();
        out.sort();
        Ok(out)
    }

    /// The store lists a folder that held files once, and the parents of
    /// folders below: only those holding a file now, as `Memory` answers.
    fn folders(&self, under: &str) -> Result<Vec<String>, Error> {
        let at = logical(under)?;
        let mut out = Vec::new();
        for folder in self.store.folders(&at).map_err(|e| of_store(under, e))? {
            if folder == at {
                continue;
            }
            if checked(folder.as_str()).is_ok() && self.store.list(&folder).map_err(|e| of_store(folder.as_str(), e))?.iter().any(|e| e.kind == crate::sealed::Kind::File && checked(e.path.as_str()).is_ok()) {
                out.push(folder.as_str().to_string());
            }
        }
        out.sort();
        Ok(out)
    }

    fn purge(&self, path: &str) -> Result<bool, Error> {
        let at = logical(path)?;
        match self.store.purge(&at) {
            Ok(_) => Ok(true),
            Err(crate::sealed::Error::Absent) => Ok(false),
            Err(e) => Err(of_store(path, e)),
        }
    }

    /// How many files went: counted from the lists before (the store counts blobs).
    fn purge_under(&self, folder: &str) -> Result<usize, Error> {
        let at = logical(folder)?;
        let n = self.folders(folder)?.iter().chain(std::iter::once(&folder.to_string())).map(|f| self.list(f).map(|l| l.len()).unwrap_or(0)).sum();
        self.store.purge_under(&at).map_err(|e| of_store(folder, e))?;
        Ok(n)
    }
}

/// A sealed store under a folder of its own, for a test: the clients' tests
/// run on the real door as well as on `Memory`.
#[cfg(test)]
pub(crate) fn sealed_store(name: &str) -> (std::path::PathBuf, std::sync::Arc<crate::sealed::Store>) {
    let base = std::env::temp_dir().join(format!("sioul-clients-{name}-{}", std::process::id()));
    let _ = crate::files::remove_dir_all(&base);
    crate::files::create_dir_all(&base).unwrap();
    let store = crate::sealed::Store::create(&crate::sealed::Folders::under(&base), &crate::sealed::RootKey::test(), crate::sealed::Options::default()).unwrap();
    (base, std::sync::Arc::new(store))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise(door: &dyn Door) {
        assert_eq!(door.read("data/clients/a/client.toml").unwrap(), None);
        let g = door.write("data/clients/a/client.toml", b"one", Expect::Absent).unwrap();
        assert_eq!(door.write("data/clients/a/client.toml", b"two", Expect::Absent), Err(Error::Changed), "a new file over one there");
        assert_eq!(door.write("data/clients/a/client.toml", b"two", Expect::Generation(g.wrapping_add(7))), Err(Error::Changed), "someone wrote meanwhile");
        door.write("data/clients/a/client.toml", b"two", Expect::Generation(g)).unwrap();
        door.write("data/clients/a/db/k/r1.toml", b"row", Expect::Any).unwrap();
        door.write("data/clients/b/client.toml", b"b", Expect::Any).unwrap();
        assert_eq!(door.read("data/clients/a/client.toml").unwrap().unwrap().bytes, b"two");
        assert_eq!(door.list("data/clients/a").unwrap(), vec!["data/clients/a/client.toml".to_string()]);
        assert_eq!(door.folders("data/clients").unwrap(), vec!["data/clients/a".to_string(), "data/clients/a/db/k".to_string(), "data/clients/b".to_string()]);
        let wrote = door.update("data/clients/a/client.toml", &mut |now| Ok(Some([now.unwrap_or_default(), &b"!"[..]].concat()))).unwrap();
        assert!(wrote);
        assert_eq!(door.read_text("data/clients/a/client.toml").unwrap().unwrap().0, "two!");
        assert!(door.remove("data/clients/b/client.toml").unwrap());
        assert!(!door.remove("data/clients/b/client.toml").unwrap());
        assert!(door.purge("data/clients/a/db/k/r1.toml").unwrap());
        for hostile in ["data/clients/../x", "data/clients//x", "data/clients/a b", "/data/x", "data/clients/é", ""] {
            assert_eq!(door.read(hostile), Err(Error::Refused("path")), "{hostile}");
        }
    }

    #[test]
    fn the_doors_answer_alike() {
        exercise(&Memory::new());
        let (base, store) = sealed_store("door");
        exercise(&Sealed::new(store.clone()));
        // Written twice while nothing carries it elsewhere; once while the part travels.
        let at = crate::sealed::LogicalPath::new("data/clients/c/client.toml").unwrap();
        let sealed = Sealed::new(store.clone());
        sealed.write("data/clients/c/client.toml", b"c", Expect::Any).unwrap();
        assert!(store.stat(&at).unwrap().unwrap().twin, "nowhere else yet: a twin");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The sealed door names nothing in clear: no client's id, no file's
    /// name and no content is in any name or byte on the disk.
    #[test]
    fn a_client_s_file_is_sealed_on_the_disk() {
        let (base, store) = sealed_store("disk");
        let door = Sealed::new(store);
        let id = "5b6f3c2a-0d1e-4f00-9a00-0000000000aa";
        door.write(&format!("data/clients/{id}/client.toml"), b"name = \"Studio Exemple\"\n", Expect::Absent).unwrap();
        door.write(&format!("data/clients/{id}/notes/n1.md"), b"# Allergies\nLatex.\n", Expect::Absent).unwrap();
        assert_eq!(door.list(&format!("data/clients/{id}/notes")).unwrap(), vec![format!("data/clients/{id}/notes/n1.md")]);
        // A file another hand put there (a sharing's conflict copy) is kept, never read as a part.
        let store = door.store.clone();
        store.write(&crate::sealed::LogicalPath::new(&format!("data/clients/{id}/notes/n1 (conflict 2026-11-28 10.05).md")).unwrap(), b"# Allergies\n", crate::sealed::How::default()).unwrap();
        assert_eq!(door.list(&format!("data/clients/{id}/notes")).unwrap(), vec![format!("data/clients/{id}/notes/n1.md")], "the copy passed over");
        assert_eq!(door.read_text(&format!("data/clients/{id}/notes/n1.md")).unwrap().unwrap().0, "# Allergies\nLatex.\n");
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                assert!(!name.contains(id) && !name.contains("clients") && !name.contains("n1"), "a name in clear: {name}");
                if entry.file_type().unwrap().is_dir() {
                    stack.push(entry.path());
                } else {
                    let bytes = std::fs::read(entry.path()).unwrap();
                    for clear in [&b"Studio Exemple"[..], b"Latex", b"Allergies", id.as_bytes()] {
                        assert!(!bytes.windows(clear.len()).any(|w| w == clear), "{} holds a client's words in clear", entry.path().display());
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Where the store stands, as the clients' hooks read it: open, none on
    /// this device, or not open yet (the keyring locked or late), which waits.
    #[test]
    fn a_store_not_open_yet_is_wait_never_no_store() {
        assert!(matches!(opening_of(Ok(None)), Opening::NoStore));
        assert!(matches!(opening_of(Err(crate::sealed::Error::NotOpen)), Opening::Wait));
        assert!(matches!(opening_of(Err(crate::sealed::Error::Locked("late".into()))), Opening::Wait));
        let (base, store) = sealed_store("opening");
        assert!(matches!(opening_of(Ok(Some(store))), Opening::Open(_)));
        assert_eq!(Opening::Wait.door().err(), Some(Error::NotOpen));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_damaged_file_is_never_empty() {
        let door = Memory::new();
        door.write("data/clients/a/client.toml", b"x", Expect::Any).unwrap();
        door.damage("data/clients/a/client.toml");
        assert_eq!(door.read("data/clients/a/client.toml"), Err(Error::Damaged("data/clients/a/client.toml".into())));
    }
}

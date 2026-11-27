// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sealed store (docs/store.md): the one door through which every file
//! Sioul keeps on a device is to be read and written, sealed. Each file is
//! compressed and sealed in pieces (XChaCha20-Poly1305, as the sharing seals
//! notes: `sioul_sync::blobs`), each piece bound to its blob's own name; the
//! names on the disk are opaque (HMAC-SHA-256 under a naming key); an
//! encrypted manifest per logical folder says which blob is which file.
//!
//! What it guarantees: a write is whole or not at all; a file that does not
//! open is said damaged, never taken for an empty one; every file that
//! changes keeps its n earlier versions; whatever exists nowhere else can be
//! written twice at once; a wrong key is told before anything is touched.
//! What it does not: hide anything from a program running as the person
//! that reads the keyring (as with any application), or from a person at an
//! unlocked session. Nothing here is a lock of Sioul's own.
//!
//! Every reader and writer of Sioul reaches its files through `files`, which
//! has the standard library's shape: a path under the profile's four folders
//! is sealed here once the profile's store exists (made, and the plain files
//! moved in, by the window: `Store::move_plain_files`); any other path is the
//! system's file. The notes, papers and letters folders stay outside, in
//! clear, unless the person places them inside Sioul's folders.

mod check;
mod disk;
pub mod files;
mod moving;
mod format;
mod keys;
mod lines;
mod lock;
mod manifest;
mod store;
mod storefile;
mod synced;

#[cfg(test)]
mod bench;
#[cfg(test)]
mod files_tests;
#[cfg(test)]
mod review_tests;
#[cfg(test)]
mod tests;

pub use check::{FolderReport, MendedFrom, Report};
pub use keys::RootKey;
pub use lock::ProfileLock;
pub use moving::Moved;
pub use store::Store;
pub use storefile::{StoreFile, Way, Wrapped, new_id};
pub use synced::synced_roots;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// One of Sioul's four folders, each with a `sealed/` folder of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Root {
    Config,
    Data,
    State,
    Cache,
}

impl Root {
    pub const ALL: [Root; 4] = [Root::Config, Root::Data, Root::State, Root::Cache];

    pub fn name(self) -> &'static str {
        match self {
            Root::Config => "config",
            Root::Data => "data",
            Root::State => "state",
            Root::Cache => "cache",
        }
    }
}

/// The store's own files (the move's progress): a root of their own, in the
/// configuration's `sealed/` folder, that no caller can name.
const INTERNAL: &str = "store";

/// The most a part of a logical path holds, and the whole path.
pub const PART_BYTES: usize = 255;
pub const PATH_BYTES: usize = 4096;

/// A file's or a folder's name in the store: UTF-8, `/`-separated, its first
/// part one of the four roots, the rest today's layout under that folder
/// (`config/config.toml`, `data/mail/<account>/<folder>/<unique>`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LogicalPath(String);

impl LogicalPath {
    /// Refused: an empty part, `.`, `..`, a NUL, a backslash, a part over
    /// 255 bytes, a path over 4 096 bytes, an unknown root. Any other byte is
    /// allowed: names on the disk are opaque, so no system's rule applies.
    pub fn new(path: &str) -> Result<LogicalPath, Error> {
        LogicalPath::checked(path, false)
    }

    pub(crate) fn internal(path: &str) -> LogicalPath {
        LogicalPath(format!("{INTERNAL}/{path}"))
    }

    fn checked(path: &str, internal: bool) -> Result<LogicalPath, Error> {
        if path.len() > PATH_BYTES {
            return Err(Error::Refused(format!("a path of {} bytes: {PATH_BYTES} at most", path.len())));
        }
        let mut parts = path.split('/');
        let first = parts.next().unwrap_or_default();
        let known = Root::ALL.iter().any(|root| root.name() == first) || (internal && first == INTERNAL);
        if !known {
            return Err(Error::Refused("a path starts with config, data, state or cache".to_string()));
        }
        for part in path.split('/').skip(1) {
            if part.is_empty() || part == "." || part == ".." {
                return Err(Error::Refused("an empty part, \".\" or \"..\" in a path".to_string()));
            }
            if part.len() > PART_BYTES {
                return Err(Error::Refused(format!("a part of {} bytes: {PART_BYTES} at most", part.len())));
            }
            if part.contains(['\0', '\\']) {
                return Err(Error::Refused("a NUL or a backslash in a path".to_string()));
            }
        }
        Ok(LogicalPath(path.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The root's own name: `config`, `data`, `state`, `cache` (or the store's own).
    pub fn root_name(&self) -> &str {
        self.0.split('/').next().unwrap_or_default()
    }

    /// The folder whose `sealed/` holds it (the store's own files: the configuration's).
    pub fn root(&self) -> Root {
        match self.root_name() {
            "data" => Root::Data,
            "state" => Root::State,
            "cache" => Root::Cache,
            _ => Root::Config,
        }
    }

    /// A root alone (`config`): a folder, never a file.
    pub fn is_root(&self) -> bool {
        !self.0.contains('/')
    }

    /// Its folder: the parent, or the root for a file just under it.
    pub fn folder(&self) -> LogicalPath {
        match self.0.rsplit_once('/') {
            Some((parent, _)) => LogicalPath(parent.to_string()),
            None => self.clone(),
        }
    }

    /// Its last part.
    pub fn file_name(&self) -> &str {
        self.0.rsplit_once('/').map_or(self.0.as_str(), |(_, name)| name)
    }

    /// A file or folder inside this folder.
    pub fn join(&self, name: &str) -> Result<LogicalPath, Error> {
        LogicalPath::checked(&format!("{}/{name}", self.0), self.root_name() == INTERNAL)
    }

    /// Whether `self` is `folder` or below it.
    pub fn is_within(&self, folder: &LogicalPath) -> bool {
        self.0 == folder.0 || self.0.strip_prefix(folder.0.as_str()).is_some_and(|rest| rest.starts_with('/'))
    }
}

impl std::fmt::Display for LogicalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Sioul's four folders on this device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folders {
    pub config: PathBuf,
    pub data: PathBuf,
    pub state: PathBuf,
    pub cache: PathBuf,
}

impl Folders {
    /// The profile's own: `config_dir()`, `data_dir()`, `state_dir()`, `cache_dir()`.
    pub fn of_profile() -> Folders {
        Folders { config: crate::config::config_dir(), data: crate::config::data_dir(), state: crate::config::state_dir(), cache: crate::config::cache_dir() }
    }

    /// Four folders under `base`: tests and the bench.
    pub fn under(base: &Path) -> Folders {
        Folders { config: base.join("config"), data: base.join("data"), state: base.join("state"), cache: base.join("cache") }
    }

    pub fn of(&self, root: Root) -> &Path {
        match root {
            Root::Config => &self.config,
            Root::Data => &self.data,
            Root::State => &self.state,
            Root::Cache => &self.cache,
        }
    }

    /// Where a root's blobs are: `<folder>/sealed`.
    pub fn sealed(&self, root: Root) -> PathBuf {
        self.of(root).join("sealed")
    }

    /// `store.toml`, in the configuration's folder.
    pub fn store_file(&self) -> PathBuf {
        self.config.join("store.toml")
    }
}

/// How a store is opened.
#[derive(Clone, Debug)]
pub struct Options {
    /// n: the earlier versions kept of every file that changes.
    pub versions: usize,
    /// Folders a sync app carries: a store in one of them is refused.
    pub synced_roots: Vec<PathBuf>,
    /// Who writes ("window", "watcher", "cli"…): kept in each blob, shown with its versions.
    pub origin: String,
}

/// n until it is set: two earlier versions of each file.
pub const VERSIONS: usize = 2;

impl Default for Options {
    fn default() -> Options {
        Options { versions: VERSIONS, synced_roots: Vec::new(), origin: "sioul".to_string() }
    }
}

impl Options {
    /// For the profile: the folders this system's sync apps carry, looked up now.
    pub fn for_profile(origin: &str) -> Options {
        Options { versions: VERSIONS, synced_roots: synced_roots(), origin: origin.to_string() }
    }
}

/// A file read: its bytes, its generation (to write over it safely), and
/// which copy gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opened {
    pub bytes: Vec<u8>,
    pub generation: u64,
    pub from: Source,
}

/// Which copy of a file was read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Current,
    /// Its second copy (written at once, as it exists nowhere else).
    Twin,
    /// An earlier version: the current copy did not open.
    Earlier { generation: u64, written_ms: i64 },
}

/// How a file is written. By default: one copy, n earlier versions kept, compressed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct How {
    /// It exists nowhere else (no server holds it, no other device has it
    /// yet): written twice at once, the second copy released by `released`.
    /// Only the caller knows this: the store never guesses it.
    pub only_here: bool,
    pub expect: Expect,
    /// Never versioned: a file that never changes and has a source (a
    /// message, an attachment); its removal is final, nothing kept until the cap.
    pub unversioned: bool,
    /// Not compressed: for what an outsider can drive the size of (an index
    /// of mail, a list of senders), so that a sealed size tells nothing of it.
    pub uncompressed: bool,
}

impl How {
    pub fn only_here() -> How {
        How { only_here: true, ..How::default() }
    }

    pub fn over(generation: u64) -> How {
        How { expect: Expect::Generation(generation), ..How::default() }
    }

    /// A file that never changes and has a source: no earlier versions.
    pub fn unversioned() -> How {
        How { unversioned: true, ..How::default() }
    }
}

/// What the caller expects to replace: anything, nothing, or one generation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Expect {
    #[default]
    Any,
    Absent,
    Generation(u64),
}

/// What a write made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Written {
    pub generation: u64,
    pub size: u64,
}

/// Whole, or a file that only grows, sealed line by line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Lines,
}

/// A file as its folder's manifest lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: LogicalPath,
    pub generation: u64,
    /// Its size before sealing (0 for a file that only grows: `read_lines`).
    pub size: u64,
    pub sealed_size: u64,
    pub written_ms: i64,
    pub origin: String,
    pub kind: Kind,
    pub twin: bool,
    /// How many earlier versions are kept.
    pub versions: usize,
    pub attrs: BTreeMap<String, String>,
    /// Its current copy was found damaged.
    pub damaged: bool,
}

/// An earlier version of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub generation: u64,
    pub size: u64,
    pub written_ms: i64,
    pub origin: String,
    /// The current one, or an earlier one; a removed file's versions alone.
    pub current: bool,
}

/// What a purge removed, how many blobs it could not (a second purge, or the
/// next `collect`, takes them), and whether its folder's list was written
/// again at once (false: the next change of the folder does it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Purged {
    pub removed: usize,
    pub left: usize,
    pub folded: bool,
}

/// What a collection took out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Collected {
    pub versions: usize,
    pub bytes: u64,
    pub temporaries: usize,
    pub unlisted: usize,
    /// Blobs newer than their list, taken as their files' content.
    pub adopted: usize,
}

/// A file's lines, and the numbers of those that did not open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lines {
    /// From line 1 on, in order; a line that did not open is left out and named in `damaged`.
    pub lines: Vec<(u64, String)>,
    pub damaged: Vec<u64>,
}

/// Why the store did not do what was asked. Never an empty value for a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No store opened in this process yet (its key is not had).
    NotOpen,
    /// No `store.toml` here: this profile has no store yet.
    NotCreated,
    /// `create` over a store that is there.
    Exists,
    /// The key does not open `store.toml`'s check value: nothing was read or touched.
    WrongKey,
    /// The file is not there (for calls on a file that must be).
    Absent,
    /// Named by its manifest, it opens from no copy; or a manifest that could not be rebuilt.
    Damaged { path: String, what: String },
    /// Not what was expected: someone wrote meanwhile (or it is there, for `Expect::Absent`).
    Changed { now: Option<u64> },
    /// A lock could not be taken: reported, never passed over.
    Locked(String),
    /// The disk is short, even with earlier versions gone: the old content is intact.
    NoRoom,
    /// The store would be in a folder a sync app carries.
    Synced(PathBuf),
    /// A hostile or malformed path, a file that only grows written whole, a newer format.
    Refused(String),
    /// The caller's change (`update`) said no.
    Change(String),
    /// `read_text` of bytes that are not UTF-8.
    NotText,
    /// The system refused (rights, a disk read only).
    Io(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotOpen => f.write_str("What Sioul keeps on this device is not open yet: its key is not had."),
            Error::NotCreated => f.write_str("Sioul keeps nothing sealed on this device yet."),
            Error::Exists => f.write_str("What Sioul keeps on this device is sealed already."),
            Error::WrongKey => f.write_str("This key does not open what Sioul keeps on this device: nothing was read or changed."),
            Error::Absent => f.write_str("No such file among what Sioul keeps here."),
            Error::Damaged { path, what } => write!(f, "{path} is damaged and no copy of it opens ({what})."),
            Error::Changed { .. } => f.write_str("The file changed meanwhile: read it again."),
            Error::Locked(why) => write!(f, "A lock on Sioul's sealed files could not be taken: {why}"),
            Error::NoRoom => f.write_str("The disk is too full to write this: the file was left as it was."),
            Error::Synced(folder) => write!(f, "{} is carried by a sync app: Sioul keeps nothing sealed there.", folder.display()),
            Error::Refused(why) => write!(f, "Refused: {why}."),
            Error::Change(why) => write!(f, "{why}"),
            Error::NotText => f.write_str("The file is not text."),
            Error::Io(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

/// What the store would say once: the window says it (a later branch).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A file's current copy did not open: this copy was read.
    Mended { path: String, from: Source },
    /// A folder's manifest was lost or damaged and was made again from the blobs.
    Rebuilt { folder: String, files: usize, attrs_lost: bool },
    /// A folder's manifest was older than its journal or its blobs.
    Stale { folder: String },
}

static OPENED: RwLock<Option<Arc<Store>>> = RwLock::new(None);

/// The process's store, once its key is had: every module reaches it by `opened`.
pub fn install(store: Store) -> Arc<Store> {
    let store = Arc::new(store);
    *OPENED.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(store.clone());
    store
}

/// The process's store; `Err(NotOpen)` until `install`.
pub fn opened() -> Result<Arc<Store>, Error> {
    OPENED.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone().ok_or(Error::NotOpen)
}

/// The process's store let go (its keys with it).
pub fn uninstall() {
    *OPENED.write().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// Milliseconds since 1970, now.
/// Whether `key` opens the profile's store (its `store.toml`'s check value,
/// either copy): a key a way back gave is taken only then.
pub fn key_opens(folders: &Folders, key: &RootKey) -> bool {
    StoreFile::load_both(folders).iter().any(|file| file.opens(&keys::Keys::derive(key, &file.id)))
}

/// The folder a sync app carries that holds one of the profile's four, if
/// any: a store is refused there (`Error::Synced`), and the profile stays in
/// clear, as it was.
pub fn carried_by_sync(folders: &Folders, options: &Options) -> Option<PathBuf> {
    Root::ALL.iter().find_map(|root| synced::carried(folders.of(*root), &options.synced_roots))
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

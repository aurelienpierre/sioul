// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The door for code that thinks in paths (docs/store.md, "Paths"): the
//! standard library's file calls (`read`, `write`, `rename`, `read_dir`,
//! `metadata`, `File`, `OpenOptions`…), each with the same shape, through
//! which every reader and writer of Sioul goes. A path under one of the
//! profile's four folders is read and written in its sealed store, under its
//! logical path (`<config>/config.toml` is `config/config.toml`); any other
//! path (the notes folder, papers and letters where they are kept outside
//! Sioul's folders, a file the person picks, the sharing folder) is the
//! system's file, as before.
//!
//! **What stays in clear inside Sioul's folders**, by rule (`stays_plain`):
//! the store's own files (`sealed/`, `store.toml`, the profile's lock); lock,
//! process and socket files (`*.lock`, `*.pid`, `*.sock`), which are empty or
//! a number (a lock file of a sealed file at an opaque place, `lock_target`); and what another program reads or writes by its path: the sounds
//! and voice messages Qt plays, the icons and pictures Qt shows, a text's
//! media while shown, copies of attachments opened in another application,
//! files a site downloads (Chromium writes them), files chosen in a dialog
//! while they are copied, a voice memo while Qt records it (then sealed into
//! the notes), the pages Poppler and Tesseract read a scan from,
//! Qt's own caches, the virus scanner's database, the spam filter's and the
//! topics' training input (fastText reads it by its path), a job's error
//! output, and Chromium's profile. Everything else under the four folders is
//! sealed once the profile's store exists.
//!
//! **A folder of the person's placed inside Sioul's folders** (notes, papers,
//! letters) is sealed with them: that is the trade-off the window explains
//! where the folder is chosen (gained: sealed on the disk; lost: other
//! programs no longer read it). Its readers and writers need nothing more:
//! they come through here.
//!
//! **Which store**: the stores mounted in this process (`mount`; the
//! window's, the command line's, a test's), each over its four folders. When
//! the profile's own folders hold a store (`store.toml`) that this process
//! has not opened, it is opened at the first call through the opener the
//! program set (`set_opener`: the keyring is not the core's); while it cannot
//! be (the keyring locked), every call under those folders fails, never
//! reading or writing a plain file beside the store.
//!
//! Folders inside the store are implicit: one exists while a file lives
//! below it, or since this process made it (`create_dir_all`) until it is
//! removed. A file opened for writing (`File`) is held in memory and sealed
//! at `flush`, `sync_all`, `sync_data` and when it is dropped.

use super::{Error, Expect, Folders, How, LogicalPath, Root, Store};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// The stores mounted, and the opener.

static MOUNTED: RwLock<Vec<Arc<Store>>> = RwLock::new(Vec::new());

/// How a program opens the profile's store when a call first needs it.
pub type Opener = dyn Fn(&Folders) -> Result<Store, Error> + Send + Sync;

static OPENER: RwLock<Option<Arc<Opener>>> = RwLock::new(None);
/// When the opener last failed: it is not asked again for a few seconds.
static OPENER_FAILED: Mutex<Option<Instant>> = Mutex::new(None);
/// The profile folders whose `store.toml` was seen: a profile once sealed stays so.
static SEALED_PROFILES: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());
/// Folders made in this process inside a store (which keeps no empty folder), by store (its configuration folder).
static MADE: Mutex<BTreeSet<(PathBuf, String)>> = Mutex::new(BTreeSet::new());

fn made_lock() -> std::sync::MutexGuard<'static, BTreeSet<(PathBuf, String)>> {
    MADE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The folders made in `store` (their logical paths).
fn made_in(store: &Store) -> Vec<LogicalPath> {
    let key = &store.profile().config;
    made_lock().iter().filter(|(k, _)| k == key).filter_map(|(_, m)| LogicalPath::new(m).ok()).collect()
}

fn made_insert(store: &Store, folder: &LogicalPath) {
    made_lock().insert((store.profile().config.clone(), folder.to_string()));
}

/// The folders made at or below `folder` forgotten; whether there were any.
fn made_remove_within(store: &Store, folder: &LogicalPath) -> bool {
    let key = store.profile().config.clone();
    let mut made = made_lock();
    let before = made.len();
    made.retain(|(k, m)| *k != key || !LogicalPath::new(m).is_ok_and(|m| m.is_within(folder)));
    made.len() != before
}

/// A store's files reached through paths under its four folders, until the guard is dropped.
pub struct Mounted(Arc<Store>);

impl Drop for Mounted {
    fn drop(&mut self) {
        unmount(self.0.profile());
    }
}

/// `store` reached through the paths under its four folders (a store
/// mounted over the same folders is replaced).
pub fn mount(store: Arc<Store>) -> Mounted {
    let mut mounted = MOUNTED.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    mounted.retain(|other| other.profile() != store.profile());
    mounted.push(store.clone());
    Mounted(store)
}

/// The store over these folders no longer reached through paths.
pub fn unmount(folders: &Folders) {
    MOUNTED.write().unwrap_or_else(std::sync::PoisonError::into_inner).retain(|other| other.profile() != folders);
}

/// How this program opens the profile's store when a call first needs it
/// (the window, the command line, the watcher: `sioul_sync::storekey`).
pub fn set_opener(opener: Box<Opener>) {
    *OPENER.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::from(opener));
}

// Routing.

/// Where a path's file lives.
enum Route {
    /// The system's file, as before.
    Plain(PathBuf),
    /// Under a store's folder: the folder itself (`root`) or a logical path.
    Sealed(Arc<Store>, LogicalPath),
    /// One of a store's four folders itself.
    Top(Arc<Store>, PathBuf),
    /// Under the profile's folders, sealed, while its store is not open here.
    Closed,
}

/// A path made absolute and without `.` or `..`, as the system would read it (symbolic links aside).
pub fn normal(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().map(|dir| dir.join(path)).unwrap_or_else(|_| path.to_path_buf()) };
    let mut out = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The logical path of `rest` (a path below a root's folder), or none when it is the folder itself.
fn logical(root: Root, rest: &Path) -> Option<Result<Option<LogicalPath>, ()>> {
    let mut text = root.name().to_string();
    let mut any = false;
    for part in rest.components() {
        let Component::Normal(part) = part else { continue };
        let part = part.to_str()?;
        text.push('/');
        text.push_str(part);
        any = true;
    }
    if !any {
        return Some(Ok(None));
    }
    Some(LogicalPath::new(&text).map(Some).map_err(|_| ()))
}

/// Whether a path below one of Sioul's folders stays in clear, by rule (this module's documentation).
pub fn stays_plain(root: Root, rest: &Path) -> bool {
    let parts: Vec<&str> = rest.components().filter_map(|c| if let Component::Normal(p) = c { p.to_str() } else { Some("") }).collect();
    let Some(first) = parts.first().copied() else { return false };
    let last = parts.last().copied().unwrap_or_default();
    if first == "sealed" || first.starts_with("sealed-") || last.ends_with(".lock") || last.ends_with(".pid") || last.ends_with(".sock") || parts.iter().any(|p| p.is_empty()) {
        return true;
    }
    // A job's error output, which its own process writes by the file itself (`sioul spam … --job`).
    if root == Root::State && parts.starts_with(&["spam", "jobs"]) && last.ends_with(".log") {
        return true;
    }
    match root {
        Root::Config => matches!(first, "store.toml"),
        Root::Data => matches!(first, "clamav" | "QtWebEngine" | "qtwebengine" | "memos-recording"),
        Root::State => false,
        Root::Cache => matches!(first, "sounds" | "favicons" | "downloads" | "attachments" | "photos" | "texts" | "texts-attached" | "calls" | "chosen" | "ocr" | "spam" | "qmlcache" | "QtWebEngine" | "qtwebengine") || first.starts_with("qtpipelinecache"),
    }
}

fn mounted() -> Vec<Arc<Store>> {
    MOUNTED.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

fn route(path: &Path) -> Route {
    let path = normal(path);
    for store in mounted() {
        if let Some(route) = within(&store, &path) {
            return route;
        }
    }
    // The profile's own folders: sealed once its store.toml exists, opened here at the first call.
    let profile = Folders::of_profile();
    let Some((root, rest)) = Root::ALL.iter().find_map(|root| path.strip_prefix(normal(profile.of(*root))).ok().map(|rest| (*root, rest.to_path_buf()))) else { return Route::Plain(path) };
    // The four folders themselves are the system's (made at start, before the store opens).
    if rest.as_os_str().is_empty() || stays_plain(root, &rest) || !profile_sealed(&profile) {
        return Route::Plain(path);
    }
    match open_profile(&profile) {
        Some(store) => within(&store, &path).unwrap_or(Route::Plain(path)),
        None => Route::Closed,
    }
}

/// The route of a path under `store`'s folders, if it is.
fn within(store: &Arc<Store>, path: &Path) -> Option<Route> {
    for root in Root::ALL {
        let top = normal(store.profile().of(root));
        let Ok(rest) = path.strip_prefix(&top) else { continue };
        // A lock file of a sealed file (`.<name>.lock` beside it, an account's
        // `state/dav/<id>.lock`, any `*.lock` named after what it locks) would
        // name it in clear: it lives at its opaque place (third check).
        if let Some(target) = lock_target(store, root, rest) {
            return Some(Route::Plain(store.file_lock(&target)));
        }
        if stays_plain(root, rest) {
            return Some(Route::Plain(path.to_path_buf()));
        }
        return Some(match logical(root, rest) {
            // A folder of the person's found inside Sioul's by the move, which never seals it where it is.
            Some(Ok(Some(logical))) if store.kept_clear(&logical) => Route::Plain(path.to_path_buf()),
            Some(Ok(Some(logical))) => match on_disk(&top, rest) {
                // A link and whatever lies behind it stays the system's, as before
                // (a configuration in a dotfiles folder, a calendar on another
                // disk): never moved, never read as gone (review-finish F1).
                OnDisk::Link => Route::Plain(path.to_path_buf()),
                // A plain file in a folder not moved yet is the profile's until the
                // move takes it (F2); so is one the move noted as left, or put back
                // after a write that reached it while it was moved (G1, G3).
                OnDisk::File if !store.root_moved(root) || store.noted_left(&logical) => Route::Plain(path.to_path_buf()),
                // In a folder moved before, a plain file beside the store is not the
                // profile's: an older Sioul's, another program's, a backup put back
                // by hand. It is never read as current, and the next start sets it
                // aside (third check: D24 reverted).
                _ => Route::Sealed(store.clone(), logical),
            },
            Some(Ok(None)) => Route::Top(store.clone(), top),
            // A name the store cannot hold (not UTF-8, a NUL): the system's, as before.
            _ => Route::Plain(path.to_path_buf()),
        });
    }
    None
}

/// What the system holds at `top`/`rest` that the store must not hide.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OnDisk {
    /// Nothing, or a folder alone (a folder is the store's: its files are listed from the store).
    Nothing,
    /// A plain file there.
    File,
    /// A link there, or on the way to it.
    Link,
}

fn on_disk(top: &Path, rest: &Path) -> OnDisk {
    let mut at = top.to_path_buf();
    let parts: Vec<_> = rest.components().collect();
    for (i, part) in parts.iter().enumerate() {
        at.push(part.as_os_str());
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return OnDisk::Link,
            Ok(meta) if i + 1 == parts.len() => return if meta.is_dir() { OnDisk::Nothing } else { OnDisk::File },
            Ok(_) => {}
            Err(_) => return OnDisk::Nothing,
        }
    }
    OnDisk::Nothing
}

/// The sealed file a lock file stands for (`rest`, below `root`'s folder),
/// when its lock lives at an opaque place under `sealed/.locks/`: a `*.lock`
/// whose file, its name without `.lock` (and without the leading dot of
/// `filelock`'s `.<name>.lock`), is sealed: neither the store's own, nor the
/// profile's lock, nor one beside what stays in clear by rule or in a folder
/// of the person's kept in clear.
pub(crate) fn lock_target(store: &Store, root: Root, rest: &Path) -> Option<LogicalPath> {
    let name = rest.file_name()?.to_str()?;
    let stem = name.strip_suffix(".lock")?;
    let first = rest.components().next()?.as_os_str().to_str()?;
    if first == "sealed" || first.starts_with("sealed-") {
        return None;
    }
    if root == Root::Config && rest.components().count() == 1 && matches!(name, ".profile.lock" | ".store-create.lock") {
        return None;
    }
    let stem = stem.strip_prefix('.').filter(|s| !s.is_empty()).unwrap_or(stem);
    if stem.is_empty() || stem.ends_with(".lock") {
        return None;
    }
    let target = rest.with_file_name(stem);
    if stays_plain(root, &target) {
        return None;
    }
    let Some(Ok(Some(logical))) = logical(root, &target) else { return None };
    (!store.kept_clear(&logical)).then_some(logical)
}

/// Whether the profile's folders hold a store (`store.toml`, either copy).
fn profile_sealed(profile: &Folders) -> bool {
    let config = normal(&profile.config);
    if SEALED_PROFILES.read().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&config) {
        return true;
    }
    let sealed = profile.store_file().exists() || profile.sealed(Root::Data).join("store.toml").exists();
    if sealed {
        SEALED_PROFILES.write().unwrap_or_else(std::sync::PoisonError::into_inner).push(config);
    }
    sealed
}

/// The profile's store, opened by the program's opener and mounted; none while it cannot be.
fn open_profile(profile: &Folders) -> Option<Arc<Store>> {
    if let Some(store) = mounted().into_iter().find(|s| s.profile() == profile) {
        return Some(store);
    }
    let opener = OPENER.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()?;
    let mut failed = OPENER_FAILED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if failed.is_some_and(|at| at.elapsed() < Duration::from_secs(5)) {
        return None;
    }
    // Asked under the lock: one opening at a time, the others then find it mounted.
    if let Some(store) = mounted().into_iter().find(|s| s.profile() == profile) {
        return Some(store);
    }
    match opener(profile) {
        Ok(store) => {
            *failed = None;
            let store = Arc::new(store);
            std::mem::forget(mount(store.clone()));
            Some(store)
        }
        Err(e) => {
            eprintln!("Sioul: what Sioul keeps on this device is not open: {e}");
            *failed = Some(Instant::now());
            None
        }
    }
}

/// Whether a path is read and written sealed (inside a store, or in the profile's sealed folders).
pub fn is_sealed(path: &Path) -> bool {
    matches!(route(path), Route::Sealed(..) | Route::Top(..) | Route::Closed)
}

/// Whether a folder of the person's chosen there would be sealed: inside
/// Sioul's own folders, and not one the move found there and kept in clear
/// (the window's trade-off paragraph, where the notes', papers' or letters'
/// folder is chosen).
pub fn would_seal(folder: &Path) -> bool {
    if !inside_sioul(folder) {
        return false;
    }
    let folder = normal(folder);
    for store in mounted() {
        for root in Root::ALL {
            if let Ok(rest) = folder.strip_prefix(normal(store.profile().of(root)))
                && let Some(Ok(Some(logical))) = logical(root, rest)
            {
                return !store.kept_clear(&logical);
            }
        }
    }
    true
}

/// Whether a folder lies inside one of Sioul's four folders, where it would
/// be sealed (the notes, papers or letters folder placed there: the window's
/// trade-off paragraph), whatever the state of the store.
pub fn inside_sioul(folder: &Path) -> bool {
    let folder = normal(folder);
    let profile = Folders::of_profile();
    let mut tops: Vec<PathBuf> = Root::ALL.iter().map(|root| normal(profile.of(*root))).collect();
    tops.extend(mounted().iter().flat_map(|s| Root::ALL.iter().map(|root| normal(s.profile().of(*root))).collect::<Vec<_>>()));
    tops.iter().any(|top| folder.starts_with(top))
}

/// Where the lock of a sealed path's file lives: under its store's
/// `sealed/.locks/`, its name opaque (a lock beside it would name the file in
/// clear). None for a file of the system.
pub fn lock_place(path: &Path) -> Option<PathBuf> {
    match route(path) {
        Route::Sealed(store, logical) => Some(store.file_lock(&logical)),
        _ => None,
    }
}

// Errors.

/// A store's error as the system's: absent is "not found", damaged "invalid
/// data", a full disk "storage full"; the store's own error inside, for a
/// caller that wants it (`io::Error::get_ref`).
pub fn io_error(e: Error) -> io::Error {
    let kind = match &e {
        Error::Absent => io::ErrorKind::NotFound,
        Error::Damaged { .. } | Error::NotText => io::ErrorKind::InvalidData,
        Error::Changed { .. } | Error::Locked(_) => io::ErrorKind::WouldBlock,
        Error::NoRoom => io::ErrorKind::StorageFull,
        Error::Synced(_) | Error::WrongKey => io::ErrorKind::PermissionDenied,
        Error::Refused(_) => io::ErrorKind::InvalidInput,
        Error::Exists => io::ErrorKind::AlreadyExists,
        Error::NotOpen | Error::NotCreated | Error::Change(_) | Error::Io(_) => io::ErrorKind::Other,
    };
    io::Error::new(kind, e)
}

fn closed() -> io::Error {
    io_error(Error::NotOpen)
}

fn not_found(path: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{}: no such file", path.display()))
}

/// How a file is written in a store: a cache is made again and a message has
/// its server, so neither keeps earlier versions (`moving::how_for`).
fn how_for(logical: &LogicalPath) -> How {
    super::moving::how_for(logical)
}

/// The profile whose folders hold `path` where the move could take its file
/// (not one of the four folders themselves, nor what stays in clear by rule).
fn movable_profile(path: &Path) -> Option<Folders> {
    let path = normal(path);
    let mut profiles: Vec<Folders> = mounted().iter().map(|store| store.profile().clone()).collect();
    profiles.push(Folders::of_profile());
    profiles.into_iter().find(|profile| Root::ALL.iter().any(|root| path.strip_prefix(normal(profile.of(*root))).is_ok_and(|rest| !rest.as_os_str().is_empty() && !stays_plain(*root, rest))))
}

/// A call to the system's files found, once it held the profile's lock,
/// that the move took its file meanwhile: asked again through the door.
#[derive(Debug)]
struct Rerouted;

impl std::fmt::Display for Rerouted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the file was sealed while it was written: written again through the store")
    }
}

impl std::error::Error for Rerouted {}

fn is_rerouted(e: &io::Error) -> bool {
    e.get_ref().is_some_and(|inner| inner.is::<Rerouted>())
}

/// A write to the system's files at `paths` (the last one is made or
/// written). Under the profile's folders, where the move could take them, it
/// holds the profile's lock shared, so that the move never runs under it,
/// and asks the routes again under the lock: a file the move took meanwhile
/// is `Rerouted`, and the call goes through the door again (`again`), never
/// landing in clear where the move just removed it (review-finish G3). A
/// file that stays in clear inside a sealed folder (a lock, a socket…) has
/// its folder made on the disk for it, the store keeping folders in its
/// lists alone.
fn plain_write<T>(paths: &[&Path], write: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    if let Some(target) = paths.last() {
        let profile = Folders::of_profile();
        let inside = Root::ALL.iter().any(|root| target.starts_with(normal(profile.of(*root)))) || mounted().iter().any(|store| Root::ALL.iter().any(|root| target.starts_with(normal(store.profile().of(*root)))));
        if inside
            && let Some(parent) = target.parent()
            && !parent.exists()
        {
            super::disk::private_dir(parent)?;
        }
    }
    let Some(profile) = paths.iter().find_map(|path| movable_profile(path)).filter(|profile| profile.config.exists()) else { return write() };
    let _lock = super::ProfileLock::shared(&profile).map_err(io_error)?;
    if paths.iter().any(|path| !matches!(route(path), Route::Plain(_))) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, Rerouted));
    }
    write()
}

/// A call through the door, its route asked again when the move took its
/// file meanwhile: a plain write `Rerouted`, or a plain file found gone that
/// the store holds now (renamed away by the move between the route and the
/// call). A file being moved never reads as gone (review-finish G3).
fn again<T>(path: &Path, mut call: impl FnMut(Route) -> io::Result<T>) -> io::Result<T> {
    let mut tries = 0;
    loop {
        let routed = route(path);
        let plain = matches!(routed, Route::Plain(_));
        match call(routed) {
            Err(e) if tries < 4 && (is_rerouted(&e) || (plain && e.kind() == io::ErrorKind::NotFound && !matches!(route(path), Route::Plain(_)))) => tries += 1,
            done => return done,
        }
    }
}

/// `again` for a call on two paths (a rename, a copy, a link).
fn again2<T>(from: &Path, to: &Path, mut call: impl FnMut(Route, Route) -> io::Result<T>) -> io::Result<T> {
    let mut tries = 0;
    loop {
        let (a, b) = (route(from), route(to));
        let plain = matches!(a, Route::Plain(_));
        match call(a, b) {
            Err(e) if tries < 4 && (is_rerouted(&e) || (plain && e.kind() == io::ErrorKind::NotFound && !matches!(route(from), Route::Plain(_)))) => tries += 1,
            done => return done,
        }
    }
}

/// A write to the system's file at `path`, when the move cannot take it
/// meanwhile (`plain_write`): `None` when the store holds the file by then,
/// for the caller to write it through the door (`filelock`'s own writes,
/// which keep the system's calls).
pub fn while_plain<T>(path: &Path, write: impl FnOnce(&Path) -> io::Result<T>) -> io::Result<Option<T>> {
    for _ in 0..4 {
        let Route::Plain(place) = route(path) else { return Ok(None) };
        match plain_write(&[&place], || write(&place)) {
            Err(e) if is_rerouted(&e) => return Ok(None),
            done => return done.map(Some),
        }
    }
    Ok(None)
}

/// `store`, the file `path` held open here sealed first (`settle_under`).
fn settle_then<'a>(store: &'a Arc<Store>, path: &LogicalPath) -> &'a Arc<Store> {
    settle_under(store, Some(path));
    store
}

fn settle(store: &Store) {
    settle_under(store, None);
}

// Reading and writing whole files.

/// The whole file, as `std::fs::read`.
pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let path = path.as_ref();
    again(path, |routed| match routed {
        Route::Plain(p) => std::fs::read(p),
        Route::Sealed(store, logical) => match settle_then(&store, &logical).read(&logical).map_err(io_error)? {
            Some(opened) => Ok(opened.bytes),
            None => Err(not_found(path)),
        },
        Route::Top(..) => Err(io::Error::new(io::ErrorKind::IsADirectory, format!("{}: a folder", path.display()))),
        Route::Closed => Err(closed()),
    })
}

/// The whole file and what `metadata` says of the very content read (one
/// generation in a store, never the bytes of one and the stamp of the next).
pub fn read_with_metadata<P: AsRef<Path>>(path: P) -> io::Result<(Vec<u8>, Metadata)> {
    let path = path.as_ref();
    match route(path) {
        Route::Sealed(store, logical) => {
            settle_under(&store, Some(&logical));
            for _ in 0..8 {
                let Some(opened) = store.read(&logical).map_err(io_error)? else { return Err(not_found(path)) };
                let entry = store.stat(&logical).map_err(io_error)?.ok_or_else(|| not_found(path))?;
                if entry.generation == opened.generation {
                    return Ok((opened.bytes, Metadata(Meta::Sealed { len: entry.size, written_ms: entry.written_ms, dir: false, generation: entry.generation })));
                }
            }
            Err(io::Error::new(io::ErrorKind::Interrupted, format!("{}: replaced each time it was read", path.display())))
        }
        _ => {
            let bytes = read(path)?;
            Ok((bytes, metadata(path)?))
        }
    }
}

/// The whole file as text, as `std::fs::read_to_string`.
pub fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
}

/// The file written whole, as `std::fs::write`: in a store, whole or not at all.
pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> io::Result<()> {
    let path = path.as_ref();
    again(path, |routed| match routed {
        Route::Plain(p) => plain_write(&[&p], || std::fs::write(&p, contents.as_ref())),
        Route::Sealed(store, logical) => store.write(&logical, contents.as_ref(), how_for(&logical)).map(|_| ()).map_err(io_error),
        Route::Top(..) => Err(io::Error::new(io::ErrorKind::IsADirectory, format!("{}: a folder", path.display()))),
        Route::Closed => Err(closed()),
    })
}

/// The file written whole and said to exist nowhere else (no server, no other
/// device yet): in a store, two copies at once. Elsewhere, `write`.
pub fn write_only_here<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> io::Result<()> {
    let path = path.as_ref();
    match route(path) {
        Route::Sealed(store, logical) => store.write(&logical, contents.as_ref(), How::only_here()).map(|_| ()).map_err(io_error),
        _ => write(path, contents),
    }
}

/// The file removed, as `std::fs::remove_file`: in a store, its content kept
/// as an earlier version (the store's n), so that a removal can be undone.
pub fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    again(path, |routed| match routed {
        Route::Plain(p) => plain_write(&[&p], || std::fs::remove_file(&p)),
        Route::Sealed(store, logical) => match settle_then(&store, &logical).remove(&logical, Expect::Any).map_err(io_error)? {
            true => Ok(()),
            false => Err(not_found(path)),
        },
        Route::Top(..) => Err(io::Error::new(io::ErrorKind::IsADirectory, format!("{}: a folder", path.display()))),
        Route::Closed => Err(closed()),
    })
}

/// A file or a folder under another name, as `std::fs::rename`; between a
/// store and the system's files, copied then removed.
pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    let (from, to) = (from.as_ref(), to.as_ref());
    again2(from, to, |a, b| match (a, b) {
        (Route::Plain(a), Route::Plain(b)) => plain_write(&[&a, &b], || std::fs::rename(&a, &b)),
        (Route::Closed, _) | (_, Route::Closed) => Err(closed()),
        (Route::Sealed(store, a), Route::Sealed(other, b)) if Arc::ptr_eq(&store, &other) => {
            settle_under(&store, Some(&a));
            if a == b {
                return Ok(());
            }
            match store.stat(&a).map_err(io_error)? {
                Some(_) => store.rename(&a, &b, Expect::Any).map(|_| ()).map_err(io_error),
                None if dir_exists(&store, &a) => rename_folder(&store, &a, &b),
                None => Err(not_found(from)),
            }
        }
        _ => {
            if metadata(from)?.is_dir() {
                copy_folder(from, to)?;
                remove_dir_all(from)
            } else {
                copy(from, to)?;
                remove_file(from)
            }
        }
    })
}

/// Every file under a logical folder moved under another, one by one.
fn rename_folder(store: &Store, from: &LogicalPath, to: &LogicalPath) -> io::Result<()> {
    if to.is_within(from) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "a folder moved inside itself"));
    }
    for folder in store.folders(from).map_err(io_error)? {
        let rest = &folder.as_str()[from.as_str().len()..];
        let target = LogicalPath::new(&format!("{to}{rest}")).map_err(io_error)?;
        for entry in store.list(&folder).map_err(io_error)? {
            let name = entry.path.file_name().to_string();
            store.rename(&entry.path, &target.join(&name).map_err(io_error)?, Expect::Any).map_err(io_error)?;
        }
    }
    let moved: Vec<LogicalPath> = made_in(store).into_iter().filter(|m| m.is_within(from)).collect();
    made_remove_within(store, from);
    for m in moved {
        if let Ok(target) = LogicalPath::new(&format!("{to}{}", &m.as_str()[from.as_str().len()..])) {
            made_insert(store, &target);
        }
    }
    made_insert(store, to);
    Ok(())
}

/// A folder's files copied under another (across a store and the system).
fn copy_folder(from: &Path, to: &Path) -> io::Result<()> {
    create_dir_all(to)?;
    for entry in read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_folder(&entry.path(), &target)?;
        } else {
            copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A file's bytes copied, as `std::fs::copy`; returns how many.
pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<u64> {
    let (from, to) = (from.as_ref(), to.as_ref());
    again2(from, to, |a, b| match (a, b) {
        (Route::Plain(a), Route::Plain(b)) => plain_write(&[&a, &b], || std::fs::copy(&a, &b)),
        (Route::Plain(a), Route::Sealed(store, b)) => {
            let mut source = std::fs::File::open(&a)?;
            store.write_from(&b, &mut source, how_for(&b)).map(|w| w.size).map_err(io_error)
        }
        _ => {
            let bytes = read(from)?;
            write(to, &bytes)?;
            Ok(bytes.len() as u64)
        }
    })
}

/// A file's bytes linked under another name: in a store, copied.
pub fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    let (from, to) = (from.as_ref(), to.as_ref());
    again2(from, to, |a, b| match (a, b) {
        (Route::Plain(a), Route::Plain(b)) => plain_write(&[&a, &b], || std::fs::hard_link(&a, &b)),
        _ => copy(from, to).map(|_| ()),
    })
}

// Folders.

/// Whether a logical folder exists: a file lives below it, or this process made it.
fn dir_exists(store: &Store, folder: &LogicalPath) -> bool {
    // A folder the disk holds (files not moved yet, a lock) exists, as the system says.
    if real_place(store, folder).is_some_and(|place| place.is_dir()) {
        return true;
    }
    if made_in(store).iter().any(|m| m.is_within(folder)) {
        return true;
    }
    store.folders(folder).is_ok_and(|below| below.iter().any(|f| store.list(f).is_ok_and(|files| !files.is_empty())))
}

/// The folder and its parents, as `std::fs::create_dir_all`: in a store, remembered by this process until a file lives there.
pub fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    again(path.as_ref(), |routed| match routed {
        Route::Plain(p) => plain_write(&[&p], || std::fs::create_dir_all(&p)),
        Route::Sealed(store, logical) => {
            made_insert(&store, &logical);
            Ok(())
        }
        Route::Top(_, top) => std::fs::create_dir_all(top),
        Route::Closed => Err(closed()),
    })
}

/// One folder, as `std::fs::create_dir`.
pub fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    again(path, |routed| match routed {
        Route::Plain(p) => plain_write(&[&p], || std::fs::create_dir(&p)),
        Route::Sealed(store, logical) => {
            if store.stat(&logical).map_err(io_error)?.is_some() || dir_exists(&store, &logical) {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{}: exists", path.display())));
            }
            create_dir_all(path)
        }
        Route::Top(_, top) => std::fs::create_dir(top),
        Route::Closed => Err(closed()),
    })
}

/// An empty folder removed, as `std::fs::remove_dir`.
pub fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    again(path, |routed| match routed {
        Route::Plain(p) => plain_write(&[&p], || std::fs::remove_dir(&p)),
        Route::Sealed(store, logical) => {
            let lives = store.folders(&logical).is_ok_and(|below| below.iter().any(|f| store.list(f).is_ok_and(|files| !files.is_empty())));
            if lives {
                return Err(io::Error::new(io::ErrorKind::DirectoryNotEmpty, format!("{}: not empty", path.display())));
            }
            if made_remove_within(&store, &logical) { Ok(()) } else { Err(not_found(path)) }
        }
        Route::Top(_, top) => std::fs::remove_dir(top),
        Route::Closed => Err(closed()),
    })
}

/// A folder and everything in it removed, as `std::fs::remove_dir_all`.
pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    again(path, |routed| match routed {
        Route::Plain(p) => plain_write(&[&p], || std::fs::remove_dir_all(&p)),
        Route::Sealed(store, logical) => {
            if store.stat(&logical).map_err(io_error)?.is_some() {
                return Err(io::Error::new(io::ErrorKind::NotADirectory, format!("{}: a file", path.display())));
            }
            let existed = dir_exists(&store, &logical);
            for folder in store.folders(&logical).map_err(io_error)? {
                for entry in store.list(&folder).map_err(io_error)? {
                    store.remove(&entry.path, Expect::Any).map_err(io_error)?;
                }
            }
            made_remove_within(&store, &logical);
            // What stays in clear inside it (a lock), as the system would have removed it.
            if let Some(real) = real_place(&store, &logical) {
                let _ = std::fs::remove_dir_all(real);
            }
            if existed { Ok(()) } else { Err(not_found(path)) }
        }
        Route::Top(..) => Err(io::Error::new(io::ErrorKind::PermissionDenied, "one of Sioul's own folders is never removed whole")),
        Route::Closed => Err(closed()),
    })
}

/// A logical path's place on the disk, where files that stay in clear inside it would be.
fn real_place(store: &Store, logical: &LogicalPath) -> Option<PathBuf> {
    let mut place = store.profile().of(logical.root()).to_path_buf();
    for part in logical.as_str().split('/').skip(1) {
        place.push(part);
    }
    Some(place)
}

// What a file or folder is.

impl From<std::fs::Metadata> for Metadata {
    fn from(meta: std::fs::Metadata) -> Metadata {
        Metadata(Meta::Plain(meta))
    }
}

/// A file and every copy of it gone for good, as `filelock::remove_wiping`
/// wants: in a store, purged (no earlier version kept); elsewhere, removed.
pub fn purge<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    match route(path) {
        Route::Sealed(store, logical) => match store.purge(&logical) {
            Ok(_) => Ok(()),
            Err(Error::Absent) => Err(not_found(path)),
            Err(e) => Err(io_error(e)),
        },
        Route::Closed => Err(closed()),
        _ => remove_file(path),
    }
}

/// Everything under a folder gone for good: in a store, purged
/// (`Store::purge_under`); elsewhere, removed.
pub fn purge_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    match route(path) {
        Route::Sealed(store, logical) => {
            store.purge_under(&logical).map_err(io_error)?;
            made_remove_within(&store, &logical);
            Ok(())
        }
        Route::Closed => Err(closed()),
        _ => remove_dir_all(path),
    }
}

/// What `metadata` says of a file or a folder.
#[derive(Clone, Debug)]
pub struct Metadata(Meta);

#[derive(Clone, Debug)]
enum Meta {
    Plain(std::fs::Metadata),
    Sealed { len: u64, written_ms: i64, dir: bool, generation: u64 },
}

/// Whether it is a file, a folder or a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileType {
    file: bool,
    dir: bool,
    symlink: bool,
}

impl FileType {
    pub fn is_file(&self) -> bool {
        self.file
    }
    pub fn is_dir(&self) -> bool {
        self.dir
    }
    pub fn is_symlink(&self) -> bool {
        self.symlink
    }
}

impl From<std::fs::FileType> for FileType {
    fn from(t: std::fs::FileType) -> FileType {
        FileType { file: t.is_file(), dir: t.is_dir(), symlink: t.is_symlink() }
    }
}

fn time_of(ms: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(u64::try_from(ms).unwrap_or(0))
}

impl Metadata {
    pub fn len(&self) -> u64 {
        match &self.0 {
            Meta::Plain(m) => m.len(),
            Meta::Sealed { len, .. } => *len,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn is_file(&self) -> bool {
        match &self.0 {
            Meta::Plain(m) => m.is_file(),
            Meta::Sealed { dir, .. } => !dir,
        }
    }
    pub fn is_dir(&self) -> bool {
        match &self.0 {
            Meta::Plain(m) => m.is_dir(),
            Meta::Sealed { dir, .. } => *dir,
        }
    }
    pub fn is_symlink(&self) -> bool {
        matches!(&self.0, Meta::Plain(m) if m.is_symlink())
    }
    pub fn file_type(&self) -> FileType {
        match &self.0 {
            Meta::Plain(m) => m.file_type().into(),
            Meta::Sealed { dir, .. } => FileType { file: !dir, dir: *dir, symlink: false },
        }
    }
    /// When it was last written: in a store, when its current content was sealed.
    pub fn modified(&self) -> io::Result<SystemTime> {
        match &self.0 {
            Meta::Plain(m) => m.modified(),
            Meta::Sealed { written_ms, .. } => Ok(time_of(*written_ms)),
        }
    }
    pub fn accessed(&self) -> io::Result<SystemTime> {
        match &self.0 {
            Meta::Plain(m) => m.accessed(),
            Meta::Sealed { written_ms, .. } => Ok(time_of(*written_ms)),
        }
    }
    pub fn created(&self) -> io::Result<SystemTime> {
        match &self.0 {
            Meta::Plain(m) => m.created(),
            Meta::Sealed { written_ms, .. } => Ok(time_of(*written_ms)),
        }
    }
    /// The system's own, for a file of the system (its permissions, its number on the disk).
    pub fn plain(&self) -> Option<&std::fs::Metadata> {
        match &self.0 {
            Meta::Plain(m) => Some(m),
            Meta::Sealed { .. } => None,
        }
    }
    /// What changes whenever the file is written again: the system's file
    /// number and change time, or the store's generation and time (what
    /// `filelock::identity` compares).
    pub fn identity(&self) -> Option<(u64, i128)> {
        match &self.0 {
            Meta::Plain(_) => None,
            Meta::Sealed { generation, written_ms, .. } => Some((*generation, i128::from(*written_ms) * 1_000_000)),
        }
    }
    /// The file's number on its disk (Unix's `ino`); in a store, its generation, which changes at every write.
    #[cfg(unix)]
    pub fn ino(&self) -> u64 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::ino(m),
            Meta::Sealed { generation, .. } => *generation,
        }
    }
    /// Its disk's number (Unix's `dev`); 0 in a store.
    #[cfg(unix)]
    pub fn dev(&self) -> u64 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::dev(m),
            Meta::Sealed { .. } => 0,
        }
    }
    /// When it last changed, in seconds (Unix's `ctime`); in a store, when it was sealed.
    #[cfg(unix)]
    pub fn ctime(&self) -> i64 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::ctime(m),
            Meta::Sealed { written_ms, .. } => written_ms.div_euclid(1000),
        }
    }
    /// The nanoseconds of `ctime`.
    #[cfg(unix)]
    pub fn ctime_nsec(&self) -> i64 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::ctime_nsec(m),
            Meta::Sealed { written_ms, .. } => written_ms.rem_euclid(1000) * 1_000_000,
        }
    }
    /// Its size (Unix's `size`), as `len`.
    #[cfg(unix)]
    pub fn size(&self) -> u64 {
        self.len()
    }
    /// When it was last written, in seconds (Unix's `mtime`).
    #[cfg(unix)]
    pub fn mtime(&self) -> i64 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::mtime(m),
            Meta::Sealed { written_ms, .. } => written_ms.div_euclid(1000),
        }
    }
    /// The nanoseconds of `mtime`.
    #[cfg(unix)]
    pub fn mtime_nsec(&self) -> i64 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::mtime_nsec(m),
            Meta::Sealed { written_ms, .. } => written_ms.rem_euclid(1000) * 1_000_000,
        }
    }
    /// How many names it has (Unix's `nlink`); one in a store.
    #[cfg(unix)]
    pub fn nlink(&self) -> u64 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::nlink(m),
            Meta::Sealed { .. } => 1,
        }
    }
    /// Its owner (Unix's `uid`); in a store, the person's own.
    #[cfg(unix)]
    pub fn uid(&self) -> u32 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::uid(m),
            // SAFETY: getuid has no precondition and cannot fail.
            Meta::Sealed { .. } => unsafe { libc::getuid() },
        }
    }
    /// Its mode (Unix's `mode`); in a store, a file or folder the owner's alone.
    #[cfg(unix)]
    pub fn mode(&self) -> u32 {
        match &self.0 {
            Meta::Plain(m) => std::os::unix::fs::MetadataExt::mode(m),
            Meta::Sealed { dir: true, .. } => 0o040_700,
            Meta::Sealed { .. } => 0o100_600,
        }
    }

    /// The file's permissions; in a store, the owner's alone (0600), as everything sealed is.
    pub fn permissions(&self) -> std::fs::Permissions {
        match &self.0 {
            Meta::Plain(m) => m.permissions(),
            Meta::Sealed { .. } => sealed_permissions(),
        }
    }
}

#[cfg(unix)]
fn sealed_permissions() -> std::fs::Permissions {
    std::os::unix::fs::PermissionsExt::from_mode(0o600)
}

#[cfg(not(unix))]
fn sealed_permissions() -> std::fs::Permissions {
    std::env::temp_dir().metadata().map(|m| m.permissions()).unwrap_or_else(|_| std::fs::metadata(".").map(|m| m.permissions()).expect("a folder's permissions"))
}

/// What a file or a folder is, as `std::fs::metadata`.
pub fn metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    let path = path.as_ref();
    again(path, |routed| match routed {
        Route::Plain(p) => std::fs::metadata(p).map(|m| Metadata(Meta::Plain(m))),
        Route::Sealed(store, logical) => match settle_then(&store, &logical).stat(&logical).map_err(io_error)? {
            Some(entry) => Ok(Metadata(Meta::Sealed { len: entry.size, written_ms: entry.written_ms, dir: false, generation: entry.generation })),
            None if dir_exists(&store, &logical) => Ok(Metadata(Meta::Sealed { len: 0, written_ms: store.folder_changed_ms(&logical), dir: true, generation: 0 })),
            None => Err(not_found(path)),
        },
        Route::Top(_, top) => std::fs::metadata(top).map(|m| Metadata(Meta::Plain(m))),
        Route::Closed => Err(closed()),
    })
}

/// As `std::fs::symlink_metadata`: a store holds no link.
pub fn symlink_metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    let path = path.as_ref();
    again(path, |routed| match routed {
        Route::Plain(p) => std::fs::symlink_metadata(p).map(|m| Metadata(Meta::Plain(m))),
        _ => metadata(path),
    })
}

/// Whether a file or folder is there (an error to say so is "no").
pub fn exists<P: AsRef<Path>>(path: P) -> bool {
    metadata(path).is_ok()
}

/// As `Path::try_exists`: an error that is not "not found" is said.
pub fn try_exists<P: AsRef<Path>>(path: P) -> io::Result<bool> {
    match metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
    metadata(path).is_ok_and(|m| m.is_file())
}

pub fn is_dir<P: AsRef<Path>>(path: P) -> bool {
    metadata(path).is_ok_and(|m| m.is_dir())
}

/// As `std::fs::canonicalize`: in a store, the path made absolute and normal (a store holds no link).
pub fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let path = path.as_ref();
    match route(path) {
        Route::Plain(p) => std::fs::canonicalize(p),
        Route::Top(_, top) => std::fs::canonicalize(top),
        Route::Closed => Err(closed()),
        Route::Sealed(..) => {
            metadata(path)?;
            Ok(normal(path))
        }
    }
}

/// As `std::fs::read_link`: a store holds no link.
pub fn read_link<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    match route(path.as_ref()) {
        Route::Plain(p) => std::fs::read_link(p),
        Route::Top(_, top) => std::fs::read_link(top),
        Route::Closed => Err(closed()),
        Route::Sealed(..) => Err(io::Error::new(io::ErrorKind::InvalidInput, "not a link")),
    }
}

/// As `std::fs::set_permissions`: everything sealed is the owner's alone already.
pub fn set_permissions<P: AsRef<Path>>(path: P, permissions: std::fs::Permissions) -> io::Result<()> {
    match route(path.as_ref()) {
        Route::Plain(p) => std::fs::set_permissions(p, permissions),
        Route::Top(_, top) => std::fs::set_permissions(top, permissions),
        Route::Closed => Err(closed()),
        Route::Sealed(..) => metadata(path).map(|_| ()),
    }
}

// Listing a folder.

/// A folder's entries, as `std::fs::read_dir`.
pub struct ReadDir(ReadDirInner);

enum ReadDirInner {
    Plain(std::fs::ReadDir),
    Listed(std::vec::IntoIter<io::Result<DirEntry>>),
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;
    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        match &mut self.0 {
            ReadDirInner::Plain(inner) => inner.next().map(|e| e.map(|e| DirEntry(Entry::Plain(e)))),
            ReadDirInner::Listed(inner) => inner.next(),
        }
    }
}

/// One entry of a folder.
pub struct DirEntry(Entry);

enum Entry {
    Plain(std::fs::DirEntry),
    Listed { path: PathBuf, meta: Option<Metadata> },
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        match &self.0 {
            Entry::Plain(e) => e.path(),
            Entry::Listed { path, .. } => path.clone(),
        }
    }
    pub fn file_name(&self) -> OsString {
        match &self.0 {
            Entry::Plain(e) => e.file_name(),
            Entry::Listed { path, .. } => path.file_name().map(OsString::from).unwrap_or_default(),
        }
    }
    pub fn metadata(&self) -> io::Result<Metadata> {
        match &self.0 {
            Entry::Plain(e) => e.metadata().map(|m| Metadata(Meta::Plain(m))),
            Entry::Listed { meta: Some(meta), .. } => Ok(meta.clone()),
            Entry::Listed { path, meta: None } => metadata(path),
        }
    }
    pub fn file_type(&self) -> io::Result<FileType> {
        match &self.0 {
            Entry::Plain(e) => e.file_type().map(FileType::from),
            Entry::Listed { .. } => self.metadata().map(|m| m.file_type()),
        }
    }
}

/// A folder's entries: in a store, its files (from its list), the folders
/// below it, and what stays in clear there (a lock), by name.
pub fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<ReadDir> {
    let path = path.as_ref();
    let (store, logical, real) = match route(path) {
        Route::Plain(p) => return std::fs::read_dir(p).map(|r| ReadDir(ReadDirInner::Plain(r))),
        Route::Closed => return Err(closed()),
        Route::Sealed(store, logical) => {
            if !dir_exists(&store, &logical) {
                return Err(not_found(path));
            }
            let real = real_place(&store, &logical);
            (store, Some(logical), real)
        }
        Route::Top(store, top) => (store, None, Some(top)),
    };
    match &logical {
        Some(folder) => settle_under(&store, Some(folder)),
        None => settle(&store),
    }
    let base = normal(path);
    let mut names: BTreeSet<OsString> = BTreeSet::new();
    let mut out: Vec<io::Result<DirEntry>> = Vec::new();
    let roots: Vec<LogicalPath> = match &logical {
        Some(logical) => vec![logical.clone()],
        // One of the four folders: its root's logical folder.
        None => Root::ALL.iter().filter(|root| normal(store.profile().of(**root)) == base).filter_map(|root| LogicalPath::new(root.name()).ok()).collect(),
    };
    for folder in &roots {
        for entry in store.list(folder).map_err(io_error)? {
            let name = entry.path.file_name().to_string();
            if names.insert(OsString::from(&name)) {
                let meta = Metadata(Meta::Sealed { len: entry.size, written_ms: entry.written_ms, dir: false, generation: entry.generation });
                out.push(Ok(DirEntry(Entry::Listed { path: base.join(&name), meta: Some(meta) })));
            }
        }
        let depth = folder.as_str().split('/').count();
        let mut children: BTreeSet<String> = BTreeSet::new();
        for below in store.folders(folder).map_err(io_error)? {
            let parts: Vec<&str> = below.as_str().split('/').collect();
            if parts.len() > depth && store.list(&below).is_ok_and(|files| !files.is_empty()) {
                children.insert(parts[depth].to_string());
            }
        }
        for made in made_in(&store) {
            let parts: Vec<&str> = made.as_str().split('/').collect();
            if parts.len() > depth && made.is_within(folder) {
                children.insert(parts[depth].to_string());
            }
        }
        for child in children {
            if names.insert(OsString::from(&child)) {
                let changed = LogicalPath::new(&format!("{}/{child}", folder.as_str())).map_or(0, |f| store.folder_changed_ms(&f));
                out.push(Ok(DirEntry(Entry::Listed { path: base.join(&child), meta: Some(Metadata(Meta::Sealed { len: 0, written_ms: changed, dir: true, generation: 0 })) })));
            }
        }
    }
    // What stays in clear there, but the store's own.
    if let Some(real) = real {
        for entry in std::fs::read_dir(&real).into_iter().flatten().flatten() {
            let name = entry.file_name();
            let text = name.to_string_lossy();
            if text == "sealed" || text == "store.toml" || text == ".profile.lock" {
                continue;
            }
            if text == ".store-create.lock" {
                continue;
            }
            // Whatever the disk holds there reads as the system's (a lock, a file
            // not moved yet, a link): listed as the system lists it.
            if names.insert(name.clone()) {
                out.push(Ok(DirEntry(Entry::Plain(entry))));
            }
        }
    }
    Ok(ReadDir(ReadDirInner::Listed(out.into_iter())))
}

// Files held open.

/// An open file, as `std::fs::File`. In a store, its content is held in
/// memory and sealed when it is flushed, synced or dropped.
#[derive(Debug)]
pub struct File(FileInner);

#[derive(Debug)]
enum FileInner {
    Plain(std::fs::File),
    Sealed(Arc<Mutex<Held>>),
}

/// The sealed files held open to write in this process: their content is
/// sealed before any other call reads or moves a file of their store (a file
/// written then renamed while still open, as the system allows).
static OPEN: Mutex<Vec<std::sync::Weak<Mutex<Held>>>> = Mutex::new(Vec::new());

/// The files of `store` held open here under `under` (the file a call names,
/// or a folder it lists) sealed now, so that what the call reads, lists or
/// moves is what was written; no other file is sealed early.
fn settle_under(store: &Store, under: Option<&LogicalPath>) {
    let handles: Vec<Arc<Mutex<Held>>> = {
        let mut open = OPEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        open.retain(|handle| handle.strong_count() > 0);
        open.iter().filter_map(std::sync::Weak::upgrade).collect()
    };
    for handle in handles {
        let mut held = handle.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if held.changed && std::ptr::eq(Arc::as_ptr(&held.store), store) && under.is_none_or(|under| held.path.is_within(under)) {
            if let Err(e) = held.seal() {
                eprintln!("Sioul: a file held open could not be sealed: {e}");
            }
        }
    }
}

#[derive(Debug)]
struct Held {
    store: Arc<Store>,
    path: LogicalPath,
    content: Cursor<Vec<u8>>,
    writable: bool,
    append: bool,
    changed: bool,
    generation: Option<u64>,
    only_here: bool,
    /// The lock taken on it (`File::lock`): its opaque lock file under `sealed/.locks/`.
    lock: Option<std::fs::File>,
    /// The generation this handle sealed first: later seals of the same handle
    /// amend it (`Store::amend`), so that a file written in several pieces
    /// makes one new version, never pushing the earlier ones out.
    own: Option<u64>,
}

/// Below this size a write to a sealed file is sealed at once, its error
/// given to the writer, as the system's write gives it; past it, at flush,
/// sync and drop (a large file written in many pieces).
const SEALED_AT_ONCE: usize = 1 << 20;

impl Held {
    /// The lock file of this sealed file itself, opened (not the one `filelock` takes beside it).
    fn lock_file(&mut self) -> io::Result<&std::fs::File> {
        if self.lock.is_none() {
            let place = self.store.handle_lock(&self.path);
            if let Some(dir) = place.parent() {
                super::disk::private_dir(dir)?;
            }
            self.lock = Some(std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(place)?);
        }
        Ok(self.lock.as_ref().expect("opened above"))
    }

    fn seal(&mut self) -> io::Result<()> {
        if !self.changed {
            return Ok(());
        }
        let mut how = how_for(&self.path);
        how.only_here = self.only_here;
        let written = match self.own {
            Some(generation) => match self.store.amend(&self.path, self.content.get_ref(), generation, how) {
                // Written meanwhile by another: this handle's content becomes a new generation over it, as the system's last writer wins.
                Err(Error::Changed { .. }) => self.store.write(&self.path, self.content.get_ref(), how),
                other => other,
            },
            None => self.store.write(&self.path, self.content.get_ref(), how),
        }
        .map_err(io_error)?;
        self.generation = Some(written.generation);
        self.own = Some(written.generation);
        self.changed = false;
        Ok(())
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Err(e) = self.seal() {
            eprintln!("Sioul: a file held open could not be sealed when it was let go: {e}");
        }
    }
}

/// How a file is opened, as `std::fs::OpenOptions`.
#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
    mode: Option<u32>,
    flags: Option<i32>,
    #[cfg(windows)]
    access: Option<u32>,
    #[cfg(windows)]
    share: Option<u32>,
}

impl OpenOptions {
    pub fn new() -> OpenOptions {
        OpenOptions::default()
    }
    pub fn read(&mut self, on: bool) -> &mut OpenOptions {
        self.read = on;
        self
    }
    pub fn write(&mut self, on: bool) -> &mut OpenOptions {
        self.write = on;
        self
    }
    pub fn append(&mut self, on: bool) -> &mut OpenOptions {
        self.append = on;
        self
    }
    pub fn truncate(&mut self, on: bool) -> &mut OpenOptions {
        self.truncate = on;
        self
    }
    pub fn create(&mut self, on: bool) -> &mut OpenOptions {
        self.create = on;
        self
    }
    pub fn create_new(&mut self, on: bool) -> &mut OpenOptions {
        self.create_new = on;
        self
    }
    /// The file's mode on Unix when it is made (a sealed file is the owner's alone whatever it says).
    pub fn mode(&mut self, mode: u32) -> &mut OpenOptions {
        self.mode = Some(mode);
        self
    }
    /// Flags for the system's `open` on Unix (none for a sealed file).
    pub fn custom_flags(&mut self, flags: i32) -> &mut OpenOptions {
        self.flags = Some(flags);
        self
    }
    /// The access asked of Windows (its `OpenOptionsExt::access_mode`; none for a sealed file).
    #[cfg(windows)]
    pub fn access_mode(&mut self, access: u32) -> &mut OpenOptions {
        self.access = Some(access);
        self
    }
    /// What others may do with the file meanwhile, on Windows (`OpenOptionsExt::share_mode`).
    #[cfg(windows)]
    pub fn share_mode(&mut self, share: u32) -> &mut OpenOptions {
        self.share = Some(share);
        self
    }

    fn std(&self) -> std::fs::OpenOptions {
        let mut options = std::fs::OpenOptions::new();
        options.read(self.read).write(self.write).append(self.append).truncate(self.truncate).create(self.create).create_new(self.create_new);
        #[cfg(unix)]
        if let Some(mode) = self.mode {
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, mode);
        }
        #[cfg(unix)]
        if let Some(flags) = self.flags {
            std::os::unix::fs::OpenOptionsExt::custom_flags(&mut options, flags);
        }
        #[cfg(windows)]
        {
            if let Some(flags) = self.flags {
                std::os::windows::fs::OpenOptionsExt::custom_flags(&mut options, flags as u32);
            }
            if let Some(access) = self.access {
                std::os::windows::fs::OpenOptionsExt::access_mode(&mut options, access);
            }
            if let Some(share) = self.share {
                std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, share);
            }
        }
        options
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<File> {
        let path = path.as_ref();
        let writes = self.write || self.append || self.create || self.create_new || self.truncate;
        again(path, |routed| self.open_routed(path, routed, writes))
    }

    fn open_routed(&self, path: &Path, routed: Route, writes: bool) -> io::Result<File> {
        match routed {
            Route::Plain(p) if writes => plain_write(&[&p], || self.std().open(&p)).map(|f| File(FileInner::Plain(f))),
            Route::Plain(p) => self.std().open(p).map(|f| File(FileInner::Plain(f))),
            Route::Top(..) => Err(io::Error::new(io::ErrorKind::IsADirectory, format!("{}: a folder", path.display()))),
            Route::Closed => Err(closed()),
            Route::Sealed(store, logical) => {
                settle_under(&store, Some(&logical));
                // A file made new is claimed at once, never twice (`create_new`'s meaning across processes).
                if self.create_new {
                    let mut how = how_for(&logical);
                    how.expect = Expect::Absent;
                    match store.write(&logical, b"", how) {
                        Ok(_) => {}
                        Err(Error::Changed { .. }) => return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{}: exists", path.display()))),
                        Err(e) => return Err(io_error(e)),
                    }
                }
                let now = store.read(&logical).map_err(io_error)?;

                if now.is_none() && !(self.create || self.create_new) {
                    return Err(not_found(path));
                }
                let created = now.is_none();
                let generation = now.as_ref().map(|o| o.generation);
                let bytes = if self.truncate && !self.append { Vec::new() } else { now.map(|o| o.bytes).unwrap_or_default() };
                let mut content = Cursor::new(bytes);
                if self.append {
                    content.set_position(content.get_ref().len() as u64);
                }
                let writable = self.write || self.append;
                // A file made, or emptied, is sealed so even when nothing is written in it.
                // An append handle writes through the store at each write (`write_held`); made, the file exists at once, empty.
                if self.append && created {
                    store.update(&logical, how_for(&logical), &mut |old| Ok(old.is_none().then(Vec::new))).map_err(io_error)?;
                }
                let changed = writable && !self.append && (created || (self.truncate && generation.is_some()));
                let held = Arc::new(Mutex::new(Held { store, path: logical, content, writable, append: self.append, changed, generation, only_here: false, lock: None, own: None }));
                if writable {
                    OPEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(Arc::downgrade(&held));
                }
                Ok(File(FileInner::Sealed(held)))
            }
        }
    }
}

impl File {
    /// A file opened to be read, as `std::fs::File::open`.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<File> {
        OpenOptions::new().read(true).open(path)
    }
    /// A file made or emptied to be written, as `std::fs::File::create`.
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<File> {
        OpenOptions::new().write(true).create(true).truncate(true).open(path)
    }
    /// A new file, refused when one is there, as `std::fs::File::create_new`.
    pub fn create_new<P: AsRef<Path>>(path: P) -> io::Result<File> {
        OpenOptions::new().read(true).write(true).create_new(true).open(path)
    }
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    fn held(&self) -> Option<std::sync::MutexGuard<'_, Held>> {
        match &self.0 {
            FileInner::Plain(_) => None,
            FileInner::Sealed(held) => Some(held.lock().unwrap_or_else(std::sync::PoisonError::into_inner)),
        }
    }

    /// The system's file, for what needs one (a lock, a child's output); none for a sealed file.
    pub fn as_plain(&self) -> Option<&std::fs::File> {
        match &self.0 {
            FileInner::Plain(file) => Some(file),
            FileInner::Sealed(_) => None,
        }
    }

    /// The system's file itself; a sealed file is given back.
    pub fn into_plain(self) -> Result<std::fs::File, File> {
        match self.0 {
            FileInner::Plain(file) => Ok(file),
            sealed => Err(File(sealed)),
        }
    }

    /// Said to exist nowhere else: in a store, sealed in two copies.
    pub fn only_here(&self) {
        if let Some(mut held) = self.held() {
            held.only_here = true;
        }
    }

    pub fn metadata(&self) -> io::Result<Metadata> {
        match &self.0 {
            FileInner::Plain(file) => file.metadata().map(|m| Metadata(Meta::Plain(m))),
            FileInner::Sealed(held) => {
                let held = held.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                Ok(Metadata(Meta::Sealed { len: held.content.get_ref().len() as u64, written_ms: super::now_ms(), dir: false, generation: held.generation.unwrap_or(0) }))
            }
        }
    }
    pub fn sync_all(&self) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => file.sync_all(),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).seal(),
        }
    }
    pub fn sync_data(&self) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => file.sync_data(),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).seal(),
        }
    }
    pub fn set_len(&self, size: u64) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => file.set_len(size),
            FileInner::Sealed(held) => {
                let mut held = held.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if !held.writable {
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied, "opened to be read"));
                }
                held.content.get_mut().resize(usize::try_from(size).unwrap_or(usize::MAX), 0);
                held.changed = true;
                Ok(())
            }
        }
    }
    pub fn set_modified(&self, time: SystemTime) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => file.set_modified(time),
            FileInner::Sealed(_) => Ok(()),
        }
    }
    pub fn set_permissions(&self, permissions: std::fs::Permissions) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => file.set_permissions(permissions),
            FileInner::Sealed(_) => Ok(()),
        }
    }
    /// The file's lock (`flock`), for a file of the system; a sealed file's writes are whole already.
    pub fn lock(&self) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => file.lock(),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).lock_file()?.lock(),
        }
    }
    pub fn lock_shared(&self) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => file.lock_shared(),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).lock_file()?.lock_shared(),
        }
    }
    pub fn try_lock(&self) -> Result<(), std::fs::TryLockError> {
        match &self.0 {
            FileInner::Plain(file) => file.try_lock(),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).lock_file().map_err(std::fs::TryLockError::Error)?.try_lock(),
        }
    }
    pub fn try_lock_shared(&self) -> Result<(), std::fs::TryLockError> {
        match &self.0 {
            FileInner::Plain(file) => file.try_lock_shared(),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).lock_file().map_err(std::fs::TryLockError::Error)?.try_lock_shared(),
        }
    }
    pub fn unlock(&self) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => file.unlock(),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).lock_file()?.unlock(),
        }
    }
}

fn read_held(held: &mut Held, buf: &mut [u8]) -> io::Result<usize> {
    held.content.read(buf)
}

fn write_held(held: &mut Held, buf: &[u8]) -> io::Result<usize> {
    if !held.writable {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "opened to be read"));
    }
    if held.append {
        // An append adds to what the file holds now, whoever wrote it since
        // (another handle, another process), under its folder's lock: as the
        // system's O_APPEND, no line of another writer is lost; its error is
        // the writer's.
        let mut how = how_for(&held.path);
        how.only_here = held.only_here;
        let added = buf.to_vec();
        let written = held
            .store
            .update(&held.path, how, &mut |old| {
                let mut now = old.map(<[u8]>::to_vec).unwrap_or_default();
                now.extend_from_slice(&added);
                Ok(Some(now))
            })
            .map_err(io_error)?;
        held.generation = written.map(|w| w.generation).or(held.generation);
        let content = held.content.get_mut();
        content.extend_from_slice(buf);
        let end = content.len() as u64;
        held.content.set_position(end);
        return Ok(buf.len());
    }
    held.changed = true;
    let n = held.content.write(buf)?;
    if held.content.get_ref().len() <= SEALED_AT_ONCE {
        held.seal()?;
    }
    Ok(n)
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buf)
    }
}

impl Read for &File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &self.0 {
            FileInner::Plain(file) => (&*file).read(buf),
            FileInner::Sealed(held) => read_held(&mut held.lock().unwrap_or_else(std::sync::PoisonError::into_inner), buf),
        }
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
}

impl Write for &File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &self.0 {
            FileInner::Plain(file) => (&*file).write(buf),
            FileInner::Sealed(held) => write_held(&mut held.lock().unwrap_or_else(std::sync::PoisonError::into_inner), buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match &self.0 {
            FileInner::Plain(file) => (&*file).flush(),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).seal(),
        }
    }
}

impl Seek for File {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        (&*self).seek(to)
    }
}

impl Seek for &File {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match &self.0 {
            FileInner::Plain(file) => (&*file).seek(to),
            FileInner::Sealed(held) => held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).content.seek(to),
        }
    }
}

// What the standard library has that needs no door of its own.

pub use std::fs::{Permissions, TryLockError};

/// Folders made, as `std::fs::DirBuilder`: in a store, remembered as `create_dir_all` does.
#[derive(Clone, Debug, Default)]
pub struct DirBuilder {
    recursive: bool,
    mode: Option<u32>,
}

impl DirBuilder {
    pub fn new() -> DirBuilder {
        DirBuilder::default()
    }
    pub fn recursive(&mut self, on: bool) -> &mut DirBuilder {
        self.recursive = on;
        self
    }
    /// The folder's mode on Unix (a sealed folder is the owner's alone whatever it says).
    pub fn mode(&mut self, mode: u32) -> &mut DirBuilder {
        self.mode = Some(mode);
        self
    }
    pub fn create<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let path = path.as_ref();
        match route(path) {
            Route::Plain(p) => match plain_write(&[&p], || {
                let mut builder = std::fs::DirBuilder::new();
                builder.recursive(self.recursive);
                #[cfg(unix)]
                if let Some(mode) = self.mode {
                    std::os::unix::fs::DirBuilderExt::mode(&mut builder, mode);
                }
                builder.create(&p)
            }) {
                Err(e) if is_rerouted(&e) => if self.recursive { create_dir_all(path) } else { create_dir(path) },
                done => done,
            },
            _ if self.recursive => create_dir_all(path),
            _ => create_dir(path),
        }
    }
}

/// Whether a path is there, a file or a folder, through the door: what
/// `Path::exists`, `Path::is_file` and `Path::is_dir` say of the system's
/// files alone. Also on what a folder's listing and `metadata` give, so that
/// one name reads the same on either.
pub trait Probe {
    /// Whether it is there (`Path::exists` through the door).
    fn present(&self) -> bool;
    /// Whether it is a file there (`Path::is_file` through the door).
    fn present_file(&self) -> bool;
    /// Whether it is a folder there (`Path::is_dir` through the door).
    fn present_dir(&self) -> bool;
}

impl Probe for Path {
    fn present(&self) -> bool {
        exists(self)
    }
    fn present_file(&self) -> bool {
        is_file(self)
    }
    fn present_dir(&self) -> bool {
        is_dir(self)
    }
}

impl Probe for Metadata {
    fn present(&self) -> bool {
        true
    }
    fn present_file(&self) -> bool {
        self.is_file()
    }
    fn present_dir(&self) -> bool {
        self.is_dir()
    }
}

impl Probe for FileType {
    fn present(&self) -> bool {
        true
    }
    fn present_file(&self) -> bool {
        self.is_file()
    }
    fn present_dir(&self) -> bool {
        self.is_dir()
    }
}

impl Probe for std::fs::Metadata {
    fn present(&self) -> bool {
        true
    }
    fn present_file(&self) -> bool {
        self.is_file()
    }
    fn present_dir(&self) -> bool {
        self.is_dir()
    }
}

impl Probe for std::fs::FileType {
    fn present(&self) -> bool {
        true
    }
    fn present_file(&self) -> bool {
        self.is_file()
    }
    fn present_dir(&self) -> bool {
        self.is_dir()
    }
}

// The whole store written out in clear.

/// Every file of `store` written in clear under `to`, at its logical path
/// (`config/config.toml` as `<to>/config/config.toml`): the layout Sioul
/// has always used, each file in its standard format (TOML, iCalendar,
/// vCard, Maildir, JSON). A file that only grows is written as its lines.
/// `keep` says which logical paths go (the command line leaves clients'
/// files out unless asked). Returns how many files were written, and the
/// paths that could not be read (damaged), which are said and skipped.
pub fn export(store: &Store, to: &Path, keep: &dyn Fn(&LogicalPath) -> bool) -> io::Result<(usize, Vec<String>)> {
    let mut written = 0;
    let mut damaged = Vec::new();
    for root in Root::ALL {
        let top = LogicalPath::new(root.name()).map_err(io_error)?;
        let Ok(folders) = store.folders(&top) else {
            damaged.push(top.to_string());
            continue;
        };
        for folder in folders {
            let Ok(entries) = store.list(&folder) else {
                damaged.push(folder.to_string());
                continue;
            };
            for entry in entries {
                if !keep(&entry.path) {
                    continue;
                }
                let target: PathBuf = std::iter::once(to.to_path_buf()).chain(entry.path.as_str().split('/').map(PathBuf::from)).collect();
                let bytes = match entry.kind {
                    super::Kind::File => store.read(&entry.path).map(|o| o.map(|o| o.bytes)),
                    super::Kind::Lines => store.read_lines(&entry.path).map(|l| l.map(|l| l.lines.into_iter().map(|(_, line)| line + "\n").collect::<String>().into_bytes())),
                };
                match bytes {
                    Ok(Some(bytes)) => {
                        if let Some(parent) = target.parent() {
                            super::disk::private_dir(parent)?;
                        }
                        super::disk::place(&target, |out| out.write_all(&bytes))?;
                        written += 1;
                    }
                    Ok(None) => {}
                    Err(_) => damaged.push(entry.path.to_string()),
                }
            }
        }
    }
    Ok((written, damaged))
}

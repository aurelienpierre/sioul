// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! One writer at a time on a file that two parts of Sioul change: the window
//! marks a dose in the doses' record while the sharing writes in it what your
//! other devices marked. One writing an older copy over the other would lose a
//! dose taken, and the sharing would then take it out on every device. An
//! advisory lock on a hidden file beside it (`.<name>.lock`, which the
//! sharing never carries), held while the file is read, changed and written.

use std::path::{Path, PathBuf};

/// What tells a file from another of the same name, size and time: on Unix
/// its inode and its inode's change time; on Windows its file index and its
/// change time (`GetFileInformationByHandle`, `GetFileInformationByHandleEx`),
/// never its creation time, which NTFS gives back to a name made again within
/// fifteen seconds; (0, 0) elsewhere, or when Windows could not be asked. The
/// change time is in nanoseconds since 1970: any write moves it, whatever the
/// file's own time is set back to, and a rename moves it too. On Windows the
/// file is opened for its attributes alone, shared every way.
pub fn identity(path: &Path, meta: &std::fs::Metadata) -> (u64, i128) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let _ = path;
        (meta.ino(), i128::from(meta.ctime()) * 1_000_000_000 + i128::from(meta.ctime_nsec()))
    }
    #[cfg(windows)]
    {
        let _ = meta;
        windows_identity(path).unwrap_or((0, 0))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, meta);
        (0, 0)
    }
}

#[cfg(windows)]
fn windows_identity(path: &Path) -> Option<(u64, i128)> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES, FileBasicInfo, GetFileInformationByHandle, GetFileInformationByHandleEx};
    // Its attributes alone, shared for reading, writing and deleting: never in a writer's way.
    let file = std::fs::OpenOptions::new().access_mode(FILE_READ_ATTRIBUTES).share_mode(7).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(path).ok()?;
    let handle = file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
    // SAFETY: plain C structures, zeroed, filled by the system through a handle held open here.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: as above.
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return None;
    }
    // SAFETY: as above.
    let mut basic: FILE_BASIC_INFO = unsafe { std::mem::zeroed() };
    let size = u32::try_from(std::mem::size_of::<FILE_BASIC_INFO>()).ok()?;
    // SAFETY: the buffer is a FILE_BASIC_INFO of the size said.
    if unsafe { GetFileInformationByHandleEx(handle, FileBasicInfo, (&raw mut basic).cast(), size) } == 0 {
        return None;
    }
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    // 100-nanosecond ticks since 1601, made nanoseconds since 1970.
    let changed = (i128::from(basic.ChangeTime) - 116_444_736_000_000_000) * 100;
    Some((index, changed))
}

/// Bytes written beside `path` under a hidden name of their own, then renamed
/// over it: a reader never finds the file empty or half written (the sharing
/// reads without the lock), and a full disk leaves the file as it was rather
/// than empty (review of 5 October 2026, F10).
pub fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let temporary = path.with_file_name(format!(".{name}.{}-{}.new", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let written = std::fs::write(&temporary, bytes).and_then(|()| rename_over(&temporary, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

/// `from` renamed over `to`. On Windows a file another thread or program has
/// open for a moment (a reader of the same state) refuses to be replaced:
/// tried again a few times, for a tenth of a second at most (CI's Windows job
/// of 22 October 2026: a calendar's kinds lost so).
fn rename_over(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut tries = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(e) if cfg!(windows) && e.kind() == std::io::ErrorKind::PermissionDenied && tries < 20 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            done => return done,
        }
    }
}

/// `bytes` in place of `path`'s content, what it held overwritten where the
/// filesystem lets it be (docs/client.md, "What a disk keeps"): [`Prepared`],
/// [`Prepared::place`] and [`Placed::finish`] at once, for a file no other
/// part of Sioul reads meanwhile but through [`read_whole`]. A file that
/// readers take a lock for is replaced in those three steps, the place alone
/// under that lock (`draftcopies`: the sharing's exchange lock).
pub fn replace_wiping(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(placed) = Prepared::new(path, bytes)?.place(None)? {
        placed.finish();
    }
    Ok(())
}

/// A file as it was when it was read, to know before replacing it that it
/// still is (its device and number, its size and time; size and time only
/// where a file has no number: Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileStamp {
    dev: u64,
    ino: u64,
    len: u64,
    modified: u128,
}

impl FileStamp {
    /// What `meta` says of a file (a descriptor's, after reading through it).
    pub fn of(meta: &std::fs::Metadata) -> FileStamp {
        #[cfg(unix)]
        let (dev, ino) = (std::os::unix::fs::MetadataExt::dev(meta), std::os::unix::fs::MetadataExt::ino(meta));
        #[cfg(not(unix))]
        let (dev, ino) = (0, 0);
        let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        FileStamp { dev, ino, len: meta.len(), modified }
    }

    /// The file `path` names now; none when there is none.
    pub fn now(path: &Path) -> Option<FileStamp> {
        std::fs::metadata(path).ok().map(|meta| FileStamp::of(&meta))
    }
}

/// New bytes for a file, written beside it under a hidden name (which the
/// sharing and the watcher pass over), yours alone, and on the disk, waiting
/// to take its place; taken out if they never do. The slow part of a
/// replacement, done before any lock is taken.
#[derive(Debug)]
pub struct Prepared {
    path: PathBuf,
    temporary: PathBuf,
    placed: bool,
    /// The new file as written: the same once it takes the path's place (a
    /// rename keeps its device, number, size and time).
    stamp: Option<FileStamp>,
}

impl Prepared {
    pub fn new(path: &Path, bytes: &[u8]) -> std::io::Result<Prepared> {
        use std::io::Write;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let temporary = path.with_file_name(format!(".{name}.{}-{}.new", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        // Made first, so that a failure below takes it out (`Drop`).
        let mut prepared = Prepared { path: path.to_path_buf(), temporary, placed: false, stamp: None };
        let mut file = options.open(&prepared.temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        prepared.stamp = file.metadata().ok().map(|meta| FileStamp::of(&meta));
        Ok(prepared)
    }

    /// The new bytes in the file's place, when it is still as `was` says
    /// (None: whatever it is now); else nothing is done and None returned (the
    /// file changed since it was read, or went). Quick, no byte written: meant
    /// to run under the lock its readers take. On Unix the old file is held
    /// open across the rename, to overwrite it once it has no name
    /// ([`Placed::finish`]); elsewhere it is not held, and never overwritten:
    /// Windows renames a file held open only at times, and nothing there can
    /// read zeros.
    pub fn place(mut self, was: Option<FileStamp>) -> std::io::Result<Option<Placed>> {
        if let Some(was) = was
            && FileStamp::now(&self.path) != Some(was)
        {
            return Ok(None);
        }
        // Held for its overwrite; a file yours but read-only (a message copied
        // with its mode) made writable first, else its old bytes would stay
        // (review of 19 October 2026, F17).
        #[cfg(unix)]
        let old = std::fs::OpenOptions::new().write(true).open(&self.path).ok().or_else(|| {
            let mut permissions = std::fs::metadata(&self.path).ok()?.permissions();
            std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o600);
            std::fs::set_permissions(&self.path, permissions).ok()?;
            std::fs::OpenOptions::new().write(true).open(&self.path).ok()
        });
        #[cfg(not(unix))]
        let old: Option<std::fs::File> = None;
        std::fs::rename(&self.temporary, &self.path)?;
        self.placed = true;
        Ok(Some(Placed { path: self.path.clone(), old, stamp: self.stamp }))
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if !self.placed {
            let _ = std::fs::remove_file(&self.temporary);
        }
    }
}

/// A replacement made: what is left once the lock is let go. No reader that
/// takes that lock holds the old file then (it reads within its own hold,
/// by the file's name, which names the new file), and a reader that does not
/// reads through [`read_whole`], which never returns what follows.
#[derive(Debug)]
pub struct Placed {
    path: PathBuf,
    old: Option<std::fs::File>,
    stamp: Option<FileStamp>,
}

impl Placed {
    /// The file that took the path's place, as it was written: what the path
    /// names after another version replaces it is another file.
    pub fn stamp(&self) -> Option<FileStamp> {
        self.stamp
    }

    /// The new name on the disk (after a crash, the folder names the new
    /// file); then the old file, without a name now, overwritten with zeros
    /// and on the disk before it goes. Kept whole while it has another name
    /// (a hard link): that name's own replacement overwrites it. A filesystem
    /// that writes each change elsewhere (copy on write: Btrfs, ZFS, APFS;
    /// F2FS on phones; snapshots) and a flash disk's own remapping keep the old
    /// blocks until they are reused: only the disk's encryption makes those
    /// unreadable.
    pub fn finish(mut self) {
        #[cfg(unix)]
        if let Some(folder) = self.path.parent().and_then(|p| std::fs::File::open(if p.as_os_str().is_empty() { Path::new(".") } else { p }).ok()) {
            let _ = folder.sync_all();
        }
        if let Some(old) = self.old.as_mut() {
            overwrite(old);
        }
    }
}

/// A file taken away, its bytes overwritten with zeros first, where the
/// filesystem lets them be (what [`Placed::finish`] says of copy on write
/// and flash disks holds here too): a decrypted attachment leaving the cache
/// (docs/client.md, "What a disk keeps"). A link is taken away alone, never
/// what it points to; a file with another name is left whole for that name.
pub fn remove_wiping(path: &Path) -> std::io::Result<()> {
    if !std::fs::symlink_metadata(path)?.file_type().is_file() {
        return std::fs::remove_file(path);
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut options, libc::O_NOFOLLOW);
    let Ok(mut file) = options.open(path) else { return std::fs::remove_file(path) };
    // Its name first where a file lives on without one (Unix): no reader finds the zeros.
    #[cfg(unix)]
    {
        std::fs::remove_file(path)?;
        overwrite(&mut file);
        Ok(())
    }
    #[cfg(not(unix))]
    {
        overwrite(&mut file);
        drop(file);
        std::fs::remove_file(path)
    }
}

/// A folder taken away, each file in it overwritten first ([`remove_wiping`]).
/// On Unix the files are reached through the folders' own descriptors, each
/// opened without following a link (`openat`, `O_NOFOLLOW`): a folder
/// swapped for a link meanwhile leads nowhere else; then the folder goes
/// through the standard library's `remove_dir_all`, which is safe against
/// the same swap (review of wt/mail, F9). A file with another name is left
/// whole for that name.
pub fn remove_dir_wiping(dir: &Path) -> std::io::Result<()> {
    if !std::fs::symlink_metadata(dir)?.file_type().is_dir() {
        return std::fs::remove_file(dir);
    }
    #[cfg(unix)]
    {
        if let Some(folder) = beneath::open_folder(None, dir.as_os_str()) {
            beneath::wipe(&folder, dir, 0);
        }
        std::fs::remove_dir_all(dir)
    }
    #[cfg(not(unix))]
    {
        for entry in std::fs::read_dir(dir)?.filter_map(Result::ok) {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => remove_dir_wiping(&path)?,
                _ => remove_wiping(&path)?,
            }
        }
        std::fs::remove_dir(dir)
    }
}

/// Files under a folder reached by descriptors (`remove_dir_wiping`).
#[cfg(unix)]
mod beneath {
    use std::ffi::{CString, OsStr};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    /// How deep folders are followed.
    const DEEPEST: usize = 16;

    fn at(parent: Option<&OwnedFd>, name: &OsStr, flags: libc::c_int) -> Option<OwnedFd> {
        let name = CString::new(name.as_bytes()).ok()?;
        let base = parent.map_or(libc::AT_FDCWD, AsRawFd::as_raw_fd);
        // SAFETY: a valid descriptor or AT_FDCWD, and a zero-terminated name living through the call.
        let fd = unsafe { libc::openat(base, name.as_ptr(), flags | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
        // SAFETY: a descriptor openat just gave, owned by nothing else.
        (fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) })
    }

    /// A folder, opened without following a link.
    pub(super) fn open_folder(parent: Option<&OwnedFd>, name: &OsStr) -> Option<OwnedFd> {
        at(parent, name, libc::O_RDONLY | libc::O_DIRECTORY)
    }

    /// Each file under `folder` overwritten where it has no other name; the
    /// names read at `path`, every file and folder opened from `folder` itself.
    pub(super) fn wipe(folder: &OwnedFd, path: &Path, depth: usize) {
        let Ok(entries) = std::fs::read_dir(path) else { return };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => {
                    if depth < DEEPEST
                        && let Some(inner) = open_folder(Some(folder), &name)
                    {
                        wipe(&inner, &path.join(&name), depth + 1);
                    }
                }
                Ok(kind) if kind.is_file() => {
                    if let Some(fd) = at(Some(folder), &name, libc::O_WRONLY) {
                        let mut file = std::fs::File::from(fd);
                        super::overwrite_with_links(&mut file, 1);
                    }
                }
                // A link, a socket: taken away whole by remove_dir_all, never followed.
                _ => {}
            }
        }
    }
}

/// A file without a name left, its bytes overwritten with zeros, on the disk.
fn overwrite(file: &mut std::fs::File) {
    overwrite_with_links(file, 0);
}

/// A file with `links` names (0: none left; 1: the one it is reached by),
/// its bytes overwritten with zeros, on the disk; one with more names is
/// left whole for the others.
fn overwrite_with_links(file: &mut std::fs::File, links: u64) {
    use std::io::{Seek, Write};
    let Ok(meta) = file.metadata() else { return };
    if !meta.is_file() {
        return;
    }
    // Another name still holds it: left whole for that name.
    #[cfg(unix)]
    if std::os::unix::fs::MetadataExt::nlink(&meta) > links {
        return;
    }
    #[cfg(not(unix))]
    let _ = links;
    let zeros = [0u8; 64 * 1024];
    let mut left = meta.len();
    if file.seek(std::io::SeekFrom::Start(0)).is_err() {
        return;
    }
    while left > 0 {
        let n = usize::try_from(left.min(zeros.len() as u64)).unwrap_or(zeros.len());
        if file.write_all(&zeros[..n]).is_err() {
            return;
        }
        left -= n as u64;
    }
    let _ = file.sync_all();
}

/// How many times a file replaced while it was read is read again before
/// that is said ("Interrupted").
const READ_AGAIN: usize = 8;

/// A file's bytes as one of its versions held them whole, never the zeros
/// [`Placed::finish`] writes over the file it replaced: read through its
/// descriptor, then checked that the file read still has a name and that
/// `path` names it (`still_named`); else read again, a few times. A file
/// replaced while it was read has no name, and its bytes may be zeros by
/// then; one replaced after it was read is an older version: both are read
/// again. Where a file has no number to tell (Windows), read once: there
/// nothing is overwritten.
pub fn read_whole(path: &Path) -> std::io::Result<Vec<u8>> {
    read_whole_with(path).map(|(bytes, _)| bytes)
}

/// [`read_whole`], with what the descriptor said of the file read, after the
/// read: its size and time are those of the bytes returned.
pub fn read_whole_with(path: &Path) -> std::io::Result<(Vec<u8>, std::fs::Metadata)> {
    read_through(path, &mut |path| std::fs::File::open(path))
}

/// [`read_whole_with`], each file opened by `open` (a test replaces the file
/// between its opening and its reading).
fn read_through(path: &Path, open: &mut dyn FnMut(&Path) -> std::io::Result<std::fs::File>) -> std::io::Result<(Vec<u8>, std::fs::Metadata)> {
    use std::io::Read;
    for _ in 0..READ_AGAIN {
        let mut file = open(path)?;
        let mut bytes = Vec::with_capacity(file.metadata().map_or(0, |m| usize::try_from(m.len()).unwrap_or(0)));
        file.read_to_end(&mut bytes)?;
        let held = file.metadata()?;
        if still_named(path, &held) {
            return Ok((bytes, held));
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::Interrupted, format!("{}: replaced each time it was read", path.display())))
}

/// `from` copied to `to` as one of its versions held it whole, with
/// [`read_whole`]'s check: through its descriptor, then again, a few times,
/// when the file copied lost its name meanwhile or `from` names another one.
/// `to` is made yours alone (on Unix). Returns the bytes copied.
pub fn copy_whole(from: &Path, to: &Path) -> std::io::Result<u64> {
    for _ in 0..READ_AGAIN {
        let mut source = std::fs::File::open(from)?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut target = options.open(to)?;
        let copied = std::io::copy(&mut source, &mut target)?;
        let held = source.metadata()?;
        if still_named(from, &held) {
            return Ok(copied);
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::Interrupted, format!("{}: replaced each time it was copied", from.display())))
}

/// Whether the file read through a descriptor (`held`: what it says after
/// the read) still has a name, and `path` is it.
fn still_named(path: &Path, held: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        held.nlink() > 0 && std::fs::metadata(path).is_ok_and(|named| named.dev() == held.dev() && named.ino() == held.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = (path, held);
        true
    }
}

/// Whether a writer of `path` takes its lock here (its hidden lock file is
/// there): for files the sharing writes without it otherwise (a note, which
/// most writers do not lock), so that a file one of Sioul's own writers
/// changes under its lock (the letters' list) is written under it too.
pub fn has_lock(path: &Path) -> bool {
    lock_path(path).exists()
}

fn lock_path(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    path.with_file_name(format!(".{name}.lock"))
}

thread_local! {
    /// The locks this thread holds: one taken again inside (a writer calling
    /// another of the same file) is already held, never waited for.
    static HELD: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `f`, with the lock on `path` held: other threads and programs wait for it.
/// Where no lock can be taken (a folder not writable), `f` runs all the same:
/// writing it fails there anyway. Taken again by the thread holding it, it is
/// held already.
pub fn with_lock<T>(path: &Path, f: impl FnOnce() -> T) -> T {
    let lock = lock_path(path);
    if HELD.with(|held| held.borrow().contains(&lock)) {
        return f();
    }
    struct Holding(PathBuf);
    impl Drop for Holding {
        fn drop(&mut self) {
            HELD.with(|held| held.borrow_mut().retain(|p| *p != self.0));
        }
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(false).write(true);
    // Empty, it says nothing; yours alone all the same, as what it guards is (0600 on Unix).
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = path.parent().and_then(|parent| std::fs::create_dir_all(parent).ok()).and_then(|()| options.open(lock_path(path)).ok());
    let locked = file.as_ref().is_some_and(|f| f.lock().is_ok());
    HELD.with(|held| held.borrow_mut().push(lock.clone()));
    let holding = Holding(lock);
    let out = f();
    drop(holding);
    if locked && let Some(file) = &file {
        let _ = file.unlock();
    }
    out
}

/// A TOML file as it was read, kept beside the state read from it (a field
/// that serde skips): saved, only what changed since it was read is written
/// over what the file holds then (`save_merged`), so that a change your other
/// devices brought in meanwhile (the sharing writes under the same lock)
/// stays. Equal to any other, so that it changes no comparison of the state.
#[derive(Clone, Default)]
pub struct Read(Option<std::sync::Arc<toml::Table>>);

impl Read {
    /// The file's text, as read.
    pub fn of(text: &str) -> Read {
        Read(text.parse::<toml::Table>().ok().map(std::sync::Arc::new))
    }
}

impl PartialEq for Read {
    fn eq(&self, _: &Read) -> bool {
        true
    }
}

impl Eq for Read {}

impl std::fmt::Debug for Read {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Read")
    }
}

/// `state` written into `path`, under its lock, the file read again first:
/// what changed from `read` (the file as it was read) to `state` is set over
/// what the file holds now, field by field (a list as a set: what was added
/// added, what was taken out taken out; a list or table missing on one side
/// is an empty one, as a state skips an empty field); the rest stays as the
/// file holds it. A file missing when it was read is read as empty. Nothing
/// read before that could be merged (a file that did not read): `state`
/// whole. Written beside, then moved.
pub fn save_merged<T: serde::Serialize + serde::de::DeserializeOwned>(path: &Path, read: &Read, state: &T) -> Result<(), String> {
    save_merged_by(path, read, state, |_, _| {})
}

/// The same, `settle` then given the merged table and the file as it was
/// just before (none when missing): for what a field-by-field merge cannot
/// say (a mark that must never go backwards).
pub fn save_merged_by<T: serde::Serialize + serde::de::DeserializeOwned>(path: &Path, read: &Read, state: &T, settle: impl FnOnce(&mut toml::Table, Option<&toml::Table>)) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let new = match toml::Value::try_from(state).map_err(|e| e.to_string())? {
        toml::Value::Table(table) => table,
        _ => return Err(format!("{}: not a table", path.display())),
    };
    // What of the file as read this Sioul knows: read into `T` and written
    // again. A field it does not know (a newer Sioul's) is never taken out.
    let known: Option<toml::Table> = read.0.as_ref().and_then(|base| T::deserialize(toml::Value::Table((**base).clone())).ok()).and_then(|t| toml::Value::try_from(&t).ok()).and_then(|v| match v {
        toml::Value::Table(table) => Some(table),
        _ => None,
    });
    with_lock(path, || {
        let current = std::fs::read_to_string(path).ok().and_then(|text| text.parse::<toml::Table>().ok());
        let mut table = match (&read.0, &current) {
            (Some(base), Some(current)) => {
                let mut merged = current.clone();
                merge(&mut merged, base, &new, known.as_ref());
                merged
            }
            _ => new.clone(),
        };
        settle(&mut table, current.as_ref());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let temporary = path.with_file_name(format!(".{}.sioul.tmp", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
        std::fs::write(&temporary, toml::to_string(&table).map_err(|e| e.to_string())?).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
    })
}

static EMPTY: std::sync::LazyLock<toml::Table> = std::sync::LazyLock::new(toml::Table::new);

/// What changed from `base` to `new`, set over `current`. A list or a table
/// on one side only stands against an empty one on the other: emptied, it
/// takes out what it held, never what another writer added since; filled
/// from nothing, it adds to what another writer put there.
///
/// `known`: the base as the state's type reads it (none: all of it); a key of
/// the base it does not hold, nor `new`, is one that type does not know,
/// left as `current` holds it.
fn merge(current: &mut toml::Table, base: &toml::Table, new: &toml::Table, known: Option<&toml::Table>) {
    let keys: std::collections::BTreeSet<String> = base.keys().chain(new.keys()).filter(|key| new.contains_key(*key) || known.is_none_or(|known| known.contains_key(*key)) || !base.contains_key(*key)).cloned().collect();
    let no_list = toml::Value::Array(Vec::new());
    let no_table = toml::Value::Table(toml::Table::new());
    for key in keys {
        let (before, after) = match (base.get(&key), new.get(&key)) {
            (Some(before @ toml::Value::Array(_)), None) => (Some(before), Some(&no_list)),
            (None, Some(after @ toml::Value::Array(_))) => (Some(&no_list), Some(after)),
            (Some(before @ toml::Value::Table(_)), None) => (Some(before), Some(&no_table)),
            (None, Some(after @ toml::Value::Table(_))) => (Some(&no_table), Some(after)),
            pair => pair,
        };
        match (before, after) {
            (before, after) if before == after => {}
            (Some(toml::Value::Table(before)), Some(toml::Value::Table(after))) => match current.get_mut(&key) {
                Some(toml::Value::Table(inner)) => {
                    merge(inner, before, after, known.map(|k| k.get(&key).and_then(toml::Value::as_table).unwrap_or(&*EMPTY)));
                    if inner.is_empty() && !new.contains_key(&key) {
                        current.remove(&key);
                    }
                }
                _ if after.is_empty() && !new.contains_key(&key) => {}
                _ => {
                    current.insert(key, toml::Value::Table(after.clone()));
                }
            },
            (Some(toml::Value::Array(before)), Some(toml::Value::Array(after))) => match current.get_mut(&key) {
                Some(toml::Value::Array(inner)) => {
                    inner.retain(|item| !before.contains(item) || after.contains(item));
                    for item in after.iter().filter(|item| !before.contains(item)) {
                        if !inner.contains(item) {
                            inner.push(item.clone());
                        }
                    }
                    if inner.is_empty() && !new.contains_key(&key) {
                        current.remove(&key);
                    }
                }
                _ if after.is_empty() && !new.contains_key(&key) => {}
                _ => {
                    current.insert(key, toml::Value::Array(after.clone()));
                }
            },
            (_, Some(after)) => {
                current.insert(key, after.clone());
            }
            (_, None) => {
                current.remove(&key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TODO §4 (10 November 2026): a decrypted attachment leaving the cache
    /// is overwritten first. A reader that held it open reads zeros; a link
    /// goes alone, what it named left whole; a file with another name stays
    /// whole for it; a folder goes with each file in it overwritten.
    #[cfg(unix)]
    #[test]
    fn a_file_taken_away_is_overwritten_first() {
        use std::io::Read;
        let base = std::env::temp_dir().join(format!("sioul-remove-wiping-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let cache = base.join("cache").join("0123");
        std::fs::create_dir_all(cache.join("inner")).unwrap();
        let (file, inner) = (cache.join("lease.pdf"), cache.join("inner").join("scan.png"));
        std::fs::write(&file, "The secret clause is on page four.").unwrap();
        std::fs::write(&inner, "The second secret.").unwrap();
        let mut held = std::fs::File::open(&file).unwrap();
        let mut held_inner = std::fs::File::open(&inner).unwrap();
        // A link to a file of yours elsewhere, and a file with a second name.
        let yours = base.join("yours.txt");
        std::fs::write(&yours, "Yours, untouched.").unwrap();
        std::os::unix::fs::symlink(&yours, cache.join("link.txt")).unwrap();
        let kept = base.join("kept.txt");
        std::fs::write(&kept, "Kept by its other name.").unwrap();
        std::fs::hard_link(&kept, cache.join("twice.txt")).unwrap();
        remove_dir_wiping(&base.join("cache").join("0123")).unwrap();
        assert!(!cache.exists());
        let mut read = String::new();
        held.read_to_string(&mut read).unwrap();
        assert!(!read.is_empty() && read.bytes().all(|b| b == 0), "{read:?}");
        let mut read = String::new();
        held_inner.read_to_string(&mut read).unwrap();
        assert!(read.bytes().all(|b| b == 0), "{read:?}");
        assert_eq!(std::fs::read_to_string(&yours).unwrap(), "Yours, untouched.");
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "Kept by its other name.");
        // One file alone, and one already gone: said.
        std::fs::write(base.join("one.txt"), "x").unwrap();
        remove_wiping(&base.join("one.txt")).unwrap();
        assert!(!base.join("one.txt").exists() && remove_wiping(&base.join("one.txt")).is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// The race main's final pass met (19 October 2026), made to happen: a
    /// reader opens a copy, the copy is replaced and its old bytes overwritten
    /// before the reader reads. Read through that descriptor alone, the
    /// reader gets the zeros; `read_whole` sees that the file it read has no
    /// name any more, and reads again: the new bytes, never the zeros.
    #[cfg(unix)]
    #[test]
    fn a_file_replaced_between_its_opening_and_its_reading_is_read_again() {
        use std::io::Read;
        let base = std::env::temp_dir().join(format!("sioul-read-whole-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let path = base.join("a1.eml");
        std::fs::write(&path, "The secret clause is on page four.").unwrap();
        let mut plain = std::fs::File::open(&path).unwrap();
        let mut opened = 0;
        let (bytes, held) = read_through(&path, &mut |path| {
            let file = std::fs::File::open(path)?;
            opened += 1;
            if opened == 1 {
                replace_wiping(path, b"sealed")?;
            }
            Ok(file)
        })
        .unwrap();
        assert_eq!(bytes, b"sealed");
        assert_eq!((opened, held.len()), (2, 6), "read again once, the new file's size recorded");
        let mut old = Vec::new();
        plain.read_to_end(&mut old).unwrap();
        assert!(!old.is_empty() && old.iter().all(|&b| b == 0), "a plain read gets the zeros: {old:?}");
        // Replaced each time: said, never returned.
        let again = || {
            let file = std::fs::File::open(&path)?;
            replace_wiping(&path, b"sealed")?;
            Ok(file)
        };
        assert_eq!(read_through(&path, &mut |_| again()).unwrap_err().kind(), std::io::ErrorKind::Interrupted);
        // A copy through the same check.
        let copied = base.join("copy");
        assert_eq!(copy_whole(&path, &copied).unwrap(), 6);
        assert_eq!(std::fs::read(&copied).unwrap(), b"sealed");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A copy yours but read-only (a message kept with the mode it came
    /// with) is made writable to be overwritten: once replaced, its old bytes
    /// are zeros (review of 19 October 2026, F17, probe P10a: they stayed,
    /// silently, since it could not be opened for writing).
    #[cfg(unix)]
    #[test]
    fn a_read_only_copy_is_overwritten_too() {
        use std::io::Read;
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("sioul-read-only-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let path = base.join("a1.eml");
        std::fs::write(&path, "The secret clause is on page four.").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        let mut reader = std::fs::File::open(&path).unwrap();
        let placed = Prepared::new(&path, b"sealed").unwrap().place(FileStamp::now(&path)).unwrap().expect("placed");
        placed.finish();
        let mut old = Vec::new();
        reader.read_to_end(&mut old).unwrap();
        assert!(!old.is_empty() && old.iter().all(|&b| b == 0), "the old bytes overwritten: {:?}", String::from_utf8_lossy(&old));
        assert_eq!(std::fs::read(&path).unwrap(), b"sealed");
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600, "the new copy yours alone");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A state saved after another writer changed the file since it was read
    /// keeps that writer's change: only what this one changed is written.
    #[test]
    fn a_save_keeps_what_another_writer_changed_since_it_was_read() {
        let dir = std::env::temp_dir().join(format!("sioul-merge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("quiet.toml");
        std::fs::write(&path, "work_until = 10
lighter = [2026-10-01]
").unwrap();
        let read = Read::of(&std::fs::read_to_string(&path).unwrap());
        // Another writer (the sharing) adds a day and sets a field.
        std::fs::write(&path, "work_until = 10
rest_until = 20
lighter = [2026-10-01, 2026-10-02]
").unwrap();
        // This one, from what it read, changes work_until and adds another day.
        let mine: toml::Table = "work_until = 11
lighter = [2026-10-01, 2026-10-03]
".parse().unwrap();
        save_merged(&path, &read, &mine).unwrap();
        let now: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(now["work_until"].as_integer(), Some(11));
        assert_eq!(now["rest_until"].as_integer(), Some(20), "the other writer's field stays");
        assert_eq!(now["lighter"].as_array().unwrap().len(), 3, "both days added stay: {now:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A list the window empties (a state skips an empty field) takes out
    /// what it held, never what another writer added since; a list written
    /// into a file that was missing when read adds to what another writer put
    /// there since (third review, T9).
    #[test]
    fn an_emptied_or_new_list_keeps_what_another_writer_added() {
        let dir = std::env::temp_dir().join(format!("sioul-merge-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("quiet.toml");
        std::fs::write(&path, "lighter = [2026-10-12]\n").unwrap();
        let read = Read::of(&std::fs::read_to_string(&path).unwrap());
        std::fs::write(&path, "lighter = [2026-10-12, 2026-10-14]\n").unwrap();
        save_merged(&path, &read, &toml::Table::new()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "lighter = [2026-10-14]");
        // Emptied with nothing added since: the field goes.
        let read = Read::of(&std::fs::read_to_string(&path).unwrap());
        save_merged(&path, &read, &toml::Table::new()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "");
        // Missing when read; the other writer makes it; this one adds its own.
        std::fs::remove_file(&path).unwrap();
        let read = Read::of(&std::fs::read_to_string(&path).unwrap_or_default());
        std::fs::write(&path, "ignored = [\"a\"]\n[seen]\nx = 1\n").unwrap();
        let mine: toml::Table = "ignored = [\"b\"]\n[seen]\ny = 2\n".parse().unwrap();
        save_merged(&path, &read, &mine).unwrap();
        let now: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(now["ignored"].as_array().unwrap().len(), 2, "{now:?}");
        assert_eq!((now["seen"].get("x").and_then(toml::Value::as_integer), now["seen"].get("y").and_then(toml::Value::as_integer)), (Some(1), Some(2)), "{now:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A field the state's type does not know (a newer Sioul's) stays, saved
    /// over by an older one.
    #[test]
    fn a_field_a_newer_sioul_wrote_stays() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Older {
            #[serde(default)]
            work_until: Option<i64>,
        }
        let dir = std::env::temp_dir().join(format!("sioul-merge-newer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("quiet.toml");
        std::fs::write(&path, "work_until = 10\nnewer = \"kept\"\n[table]\nnewer = 1\n").unwrap();
        let read = Read::of(&std::fs::read_to_string(&path).unwrap());
        save_merged(&path, &read, &Older { work_until: Some(11) }).unwrap();
        let now: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!((now["work_until"].as_integer(), now["newer"].as_str(), now.get("table").is_some()), (Some(11), Some("kept"), true), "{now:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writers_take_turns() {
        let dir = std::env::temp_dir().join(format!("sioul-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("count.txt");
        std::fs::write(&path, "0").unwrap();
        // Eight threads add one each, a hundred times: read, wait, write. Locked, none is lost.
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        with_lock(&path, || {
                            let n: u32 = std::fs::read_to_string(&path).unwrap().trim().parse().unwrap();
                            std::thread::yield_now();
                            std::fs::write(&path, (n + 1).to_string()).unwrap();
                        });
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "800");
        assert!(dir.join(".count.txt.lock").exists(), "hidden, so never shared");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

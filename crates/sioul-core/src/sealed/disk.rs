// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The store's ways with the disk (docs/store.md, "What each system does").
//!
//! A blob is written beside its place under a temporary name, flushed,
//! renamed into place, and its folder flushed, so that a power cut leaves
//! the old file or the new, never half of one. What each system imposes:
//! - **Linux, Android, the BSDs, macOS**: a folder is flushed by `fsync` on
//!   it, so that the rename itself survives a power cut (on macOS `fsync`
//!   reaches the drive's cache, not always its platters: the system's own limit).
//! - **Windows**: a folder cannot be flushed (NTFS journals its names
//!   itself); a rename over a file that an indexer or an antivirus holds for
//!   a moment is refused, and tried again for a tenth of a second, as
//!   `filelock::rename_over` does. Rust opens every file shared for deletion,
//!   so a reader never stops a rename.
//! - **Android**: the app's private folders, always on the device's own
//!   storage (never a card a sync app carries), mode 0700.
//!
//! The room: a write needs its size free beyond what is kept free
//! (`kept_free`, as the sharing's); earlier versions go first when it is short.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Folders yours alone (0700 on Unix), each made flushed into its parent.
pub(crate) fn private_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        private_dir(parent)?;
    }
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    match builder.create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dir.is_dir() => return Ok(()),
        Err(e) => return Err(e),
    }
    if let Some(parent) = dir.parent() {
        sync_dir(parent)?;
    }
    Ok(())
}

/// A folder's names flushed to the disk (not on Windows: above).
pub(crate) fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        File::open(dir)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// A new file yours alone (0600 on Unix), never over one.
pub(crate) fn create_private(path: &Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// What a temporary name starts with: swept by `collect` once old.
pub(crate) const TEMPORARY: &str = ".tmp-";

/// A temporary name in `dir`, random.
pub(crate) fn temporary(dir: &Path) -> PathBuf {
    use chacha20poly1305::aead::rand_core::RngCore;
    let mut bytes = [0u8; 8];
    chacha20poly1305::aead::OsRng.fill_bytes(&mut bytes);
    dir.join(format!("{TEMPORARY}{}", super::format::hex(&bytes)))
}

/// A rename over `to`, tried again on Windows while another program holds `to` a moment.
pub(crate) fn rename_over(from: &Path, to: &Path) -> std::io::Result<()> {
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

/// `write`'s bytes put at `target` whole: beside, flushed, renamed, the folder flushed.
pub(crate) fn place<T>(target: &Path, write: impl FnOnce(&mut dyn Write) -> std::io::Result<T>) -> std::io::Result<T> {
    let dir = target.parent().ok_or_else(|| std::io::Error::other("no folder"))?;
    private_dir(dir)?;
    let temporary = temporary(dir);
    let placed = (|| {
        let file = create_private(&temporary)?;
        let mut out = std::io::BufWriter::with_capacity(64 << 10, Filling { inner: file, written: 0 });
        let made = write(&mut out)?;
        let file = out.into_inner().map_err(std::io::IntoInnerError::into_error)?.inner;
        crash(Step::BeforeFlush)?;
        file.sync_all()?;
        drop(file);
        crash(Step::BeforeRename)?;
        rename_over(&temporary, target)?;
        crash(Step::AfterRename)?;
        sync_dir(dir)?;
        Ok(made)
    })();
    if let Err(e) = &placed
        && !is_crash(e)
    {
        let _ = std::fs::remove_file(&temporary);
    }
    placed
}

/// A file being written: in a test, the disk fills past `FULL_AFTER` bytes.
struct Filling {
    inner: File,
    written: u64,
}

impl Write for Filling {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        #[cfg(test)]
        if FULL_AFTER.with(std::cell::Cell::get).is_some_and(|after| self.written + bytes.len() as u64 > after) {
            return Err(std::io::Error::from(std::io::ErrorKind::StorageFull));
        }
        let n = self.inner.write(bytes)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// A file removed, its folder flushed; none there is no fault.
pub(crate) fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => path.parent().map_or(Ok(()), sync_dir),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Where a write stops in a test, as a power cut would stop it (`crash`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Its bytes written, not flushed.
    BeforeFlush,
    /// Flushed, not renamed into place.
    BeforeRename,
    /// Renamed, its folder not flushed.
    AfterRename,
    /// The blob in place, its manifest's line not written.
    BeforeJournal,
    /// The manifest's line written, the version beyond n not removed.
    AfterJournal,
    /// A fold: the manifest's second copy written, not the first.
    BetweenManifests,
    /// A fold: both copies of the manifest written, the journal not begun again.
    BeforeNewJournal,
}

#[cfg(test)]
thread_local! {
    /// The step a test stops this thread's next write at, after passing it `n` times.
    pub(crate) static CRASH: std::cell::Cell<Option<(Step, u32)>> = const { std::cell::Cell::new(None) };
    /// The room a test says there is, in its thread (none: enough).
    pub(crate) static ROOM: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
    /// In a test, the disk full past so many bytes of one file being written.
    pub(crate) static FULL_AFTER: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

const CRASHED: &str = "a power cut (a test's)";

/// In a test, the power cut at `step`; nothing elsewhere.
#[cfg_attr(not(test), inline(always))]
pub(crate) fn crash(step: Step) -> std::io::Result<()> {
    #[cfg(test)]
    {
        if let Some((at, n)) = CRASH.with(std::cell::Cell::get)
            && at == step
        {
            if n == 0 {
                CRASH.with(|c| c.set(None));
                return Err(std::io::Error::other(CRASHED));
            }
            CRASH.with(|c| c.set(Some((at, n - 1))));
        }
    }
    let _ = step;
    Ok(())
}

/// Whether a failure is a test's power cut: nothing is tidied after one.
pub(crate) fn is_crash(e: &std::io::Error) -> bool {
    cfg!(test) && e.to_string() == CRASHED
}

/// Bytes this user may still write on the disk holding `path`, and its size; none when the system does not say.
pub(crate) fn space(path: &Path) -> Option<(u64, u64)> {
    let existing = path.ancestors().find(|p| p.exists())?;
    platform_space(existing)
}

#[cfg(unix)]
fn platform_space(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: a NUL-terminated path, and a zeroed plain C structure the call fills.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: as above; the structure lives through the call.
    if unsafe { libc::statvfs(c.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block = stat.f_frsize as u64;
    Some(((stat.f_bavail as u64).saturating_mul(block), (stat.f_blocks as u64).saturating_mul(block)))
}

#[cfg(windows)]
fn platform_space(path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut free, mut total, mut all_free) = (0u64, 0u64, 0u64);
    // SAFETY: a NUL-terminated wide path, and three outputs that live through the call.
    let done = unsafe { windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut all_free) };
    (done != 0).then_some((free, total))
}

#[cfg(not(any(unix, windows)))]
fn platform_space(_path: &Path) -> Option<(u64, u64)> {
    None
}

/// What stays free whatever the store writes: a quarter of a GB, or a
/// hundredth of the disk if more (the sharing's `disk::kept_free`).
pub(crate) fn kept_free(total: u64) -> u64 {
    (256u64 << 20).max(total / 100)
}

/// Whether `bytes` more fit on the disk holding `path`, `kept_free` left free; yes when the system does not say.
pub(crate) fn fits(path: &Path, bytes: u64) -> bool {
    #[cfg(test)]
    if let Some(room) = ROOM.with(std::cell::Cell::get) {
        return room >= bytes;
    }
    space(path).is_none_or(|(free, total)| free.saturating_sub(kept_free(total)) >= bytes)
}

/// What earlier versions may take in all, in one of the four folders: a GB,
/// or a twentieth of the disk if less (the sharing's `history::cap`).
pub(crate) fn versions_cap(path: &Path) -> u64 {
    space(path).map_or(1 << 30, |(_, total)| (1u64 << 30).min(total / 20))
}

/// In a test, room given back as a version goes (`ROOM`).
pub(crate) fn freed(bytes: u64) {
    #[cfg(test)]
    ROOM.with(|room| {
        if let Some(left) = room.get() {
            room.set(Some(left.saturating_add(bytes)));
        }
    });
    let _ = bytes;
}

/// A file's identity, to see that another process changed it: its size, its time, and where the system says, its index.
pub(crate) fn identity(path: &Path) -> Option<(u64, i128, u64, i128)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as i128);
    let (index, changed) = crate::filelock::identity_plain(path, &meta);
    Some((meta.len(), modified, index, changed))
}

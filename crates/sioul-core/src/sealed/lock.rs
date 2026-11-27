// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The store's locks (docs/store.md, "Locks"): advisory locks on empty files
//! (`flock` on Unix, `LockFileEx` on Windows, through the standard library),
//! one per logical folder under `sealed/.locks/` (its name opaque as the
//! blobs'), and the profile's, `.profile.lock` in the configuration's folder.
//! A change takes its folder's lock exclusive, a read or a listing shared;
//! every change takes the profile's lock shared, and the move takes it
//! exclusive so that nothing writes meanwhile. **A lock that cannot be taken
//! is an error**, never passed over (`filelock::with_lock` runs its work
//! all the same: the review of 24 November 2026).
//!
//! Within one thread, a folder's lock taken again is refused (a caller's
//! change calling the store on its own folder would otherwise write under
//! itself), and so is a second folder's (a change calling the store on
//! another folder would wait on its mirror for ever), but where the door
//! takes two itself, in one order (a rename); the profile's is held already.
//! A lock held is never sent to another thread: what this thread holds is
//! known by this thread alone.

use super::{Error, Folders};
use std::cell::RefCell;
use std::fs::File;
use std::path::{Path, PathBuf};

thread_local! {
    /// The locks this thread holds, whether exclusive, and whether a folder's.
    static HELD: RefCell<Vec<(PathBuf, bool, bool)>> = const { RefCell::new(Vec::new()) };
}

/// A lock held until dropped, by the thread that took it (never `Send`).
pub(crate) struct Held {
    file: Option<File>,
    path: PathBuf,
    _here: std::marker::PhantomData<*const ()>,
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = file.unlock();
            HELD.with(|held| {
                let mut held = held.borrow_mut();
                if let Some(at) = held.iter().rposition(|(p, _, _)| *p == self.path) {
                    held.remove(at);
                }
            });
        }
    }
}

/// The lock on `path` (made empty, yours alone), exclusive or shared; again
/// in the same thread: held already when `again` allows it, else refused.
pub(crate) fn take(path: &Path, exclusive: bool, again: bool) -> Result<Held, Error> {
    take_as(path, exclusive, again, false)
}

/// A folder's lock: refused while this thread holds another folder's, unless
/// `second` (the door taking two in one order).
pub(crate) fn take_folder(path: &Path, exclusive: bool, second: bool) -> Result<Held, Error> {
    if !second && HELD.with(|held| held.borrow().iter().any(|(p, _, folder)| *folder && p != path)) {
        return Err(Error::Locked("a change of one folder may not call the store on another".to_string()));
    }
    take_as(path, exclusive, false, true)
}

fn take_as(path: &Path, exclusive: bool, again: bool, folder: bool) -> Result<Held, Error> {
    let already = HELD.with(|held| held.borrow().iter().find(|(p, _, _)| p == path).map(|(_, e, _)| *e));
    if let Some(was_exclusive) = already {
        if again && (was_exclusive || !exclusive) {
            return Ok(Held { file: None, path: path.to_path_buf(), _here: std::marker::PhantomData });
        }
        return Err(Error::Locked("this thread holds it already".to_string()));
    }
    if let Some(parent) = path.parent() {
        super::disk::private_dir(parent).map_err(|e| Error::Locked(format!("{}: {e}", parent.display())))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(path).map_err(|e| Error::Locked(format!("{}: {e}", path.display())))?;
    if exclusive { file.lock() } else { file.lock_shared() }.map_err(|e| Error::Locked(format!("{}: {e}", path.display())))?;
    HELD.with(|held| held.borrow_mut().push((path.to_path_buf(), exclusive, folder)));
    Ok(Held { file: Some(file), path: path.to_path_buf(), _here: std::marker::PhantomData })
}

/// The lock on `path` if no other holds it now; none when another does (never waited for).
pub(crate) fn try_exclusive(path: &Path) -> Result<Option<Held>, Error> {
    if HELD.with(|held| held.borrow().iter().any(|(p, _, _)| p == path)) {
        return Ok(None);
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(path).map_err(|e| Error::Locked(format!("{}: {e}", path.display())))?;
    match file.try_lock() {
        Ok(()) => {
            HELD.with(|held| held.borrow_mut().push((path.to_path_buf(), true, true)));
            Ok(Some(Held { file: Some(file), path: path.to_path_buf(), _here: std::marker::PhantomData }))
        }
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(Error::Locked(format!("{}: {e}", path.display()))),
    }
}

/// The profile's lock: every process that writes Sioul's files holds it
/// shared while it writes; the move holds it exclusive, and writers wait.
pub struct ProfileLock(#[allow(dead_code)] Held);

impl ProfileLock {
    pub fn path(folders: &Folders) -> PathBuf {
        folders.config.join(".profile.lock")
    }

    /// Held shared (several writers at once); again in the same thread, held already.
    pub fn shared(folders: &Folders) -> Result<ProfileLock, Error> {
        take(&ProfileLock::path(folders), false, true).map(ProfileLock)
    }

    /// Held exclusive: the move, a repair. Waits for every writer to let go.
    pub fn exclusive(folders: &Folders) -> Result<ProfileLock, Error> {
        take(&ProfileLock::path(folders), true, true).map(ProfileLock)
    }
}

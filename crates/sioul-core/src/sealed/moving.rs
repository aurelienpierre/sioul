// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The move (docs/store.md, "The move"): a profile's files, written in clear
//! by every Sioul before this one, sealed into its store, one of the four
//! folders after the other, under the profile's lock held exclusive (every
//! other writer of Sioul waits meanwhile, `files::plain_write`). Each file is
//! read and sealed while it stays where it is (readers go on reading it);
//! then its plain copy is renamed aside (`<root>/sealed-moving/`, in clear by
//! rule), found unchanged and the same as its sealed copy, and only then
//! removed: a write that reached it meanwhile, by its path or not, puts it
//! back where it was, to be taken at the next start, so that no write is
//! ever lost (review-finish G3). A move stopped half way goes on where it
//! stopped, the plain files being the truth until they are gone; a copy left
//! aside by a cut is found again first.
//!
//! **A plain file found in a folder moved before is not the profile's**: one
//! written there since by a process that does not know the store (an older
//! Sioul still running, an agents' server started before the update, another
//! program, a backup put back by hand) is never read as current (the door
//! routes it to the store, `files::within`) and is set aside at the next start,
//! sealed under `data/set-aside/<day>/<its path>`, never sealed over the
//! current file; the window says once how many and where, and `sioul store
//! bring-in` takes one in on purpose (third check: D24 reverted). A file the
//! move noted as left, or put back after a write reached it while it was
//! moved, stays the profile's until it is taken; so does a new file in one
//! of the person's folders placed inside Sioul's knowingly.
//!
//! **Lock files of the layout before** (`.<name>.lock` beside a file, an
//! account's `state/dav/<id>.lock`), empty and whose names say what they lock,
//! are taken away once nobody holds them: this version keeps those locks at
//! opaque places (`files::lock_target`).
//!
//! What stays in clear by rule (`files::stays_plain`) is left where it is.
//! The person's own folders (notes, papers, letters, where scans arrive)
//! found inside Sioul's folders by the first move are kept in clear there
//! (`Store::keep_in_clear`): the move never seals them where they lie. A file
//! the move cannot take (unreadable, a name the store cannot hold, the disk
//! full) stays where it is, in clear, read as before, said once; the store
//! notes it (`left`) so that the next start opens without showing the move
//! again for it, and tries it again later (`retry_left`).

use super::files::stays_plain;
use super::{Error, How, LogicalPath, ProfileLock, Root, Store};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// What a move did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moved {
    /// Files sealed and their plain copies removed.
    pub files: usize,
    pub bytes: u64,
    /// Plain files found in a folder moved before (an older Sioul's, another
    /// program's, a backup put back by hand), set aside sealed under
    /// `data/set-aside/<day>/`, never sealed over the current file; and copies
    /// a cut move left aside that matched no version of their file.
    pub set_aside: usize,
    /// Where they were set aside, when any were.
    pub aside_folder: Option<String>,
    /// Files left in clear where they are (a name the store cannot hold, a
    /// file that could not be read or sealed, one written to while it was
    /// moved): their paths, said to the person only.
    pub left: Vec<PathBuf>,
}

/// Above this size a file is sealed as a stream, never held whole.
const STREAMED: u64 = 32 << 20;

/// The folder below each root where a plain copy waits, renamed, while it is checked (`stays_plain`: `sealed-*`).
const ASIDE: &str = "sealed-moving";

/// How a moved file is written: a message (Maildir) and a cache have no
/// earlier versions (their source is the server, or they are made again).
pub fn how_for(path: &LogicalPath) -> How {
    if path.root() == Root::Cache || path.as_str().starts_with("data/mail/") { How::unversioned() } else { How::default() }
}

/// What tells a plain file left by the move from the same file changed since
/// (its size, its time, its number and its number's change time): a file
/// noted as left is not counted to move again until one of them moves.
type Stamp = (u64, i128, u64, i128);

fn stamp(place: &Path) -> Option<Stamp> {
    let meta = std::fs::symlink_metadata(place).ok()?;
    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as i128);
    let (number, changed) = crate::filelock::identity_plain(place, &meta);
    Some((meta.len(), modified, number, changed))
}

/// What a rename keeps of a file (its size, its own time, its number; not
/// its number's change time, which a rename moves), read where it lies now.
fn kept_by_rename(at: &Path, meta: &std::fs::Metadata) -> (u64, Option<std::time::SystemTime>, u64) {
    (meta.len(), meta.modified().ok(), crate::filelock::identity_plain(at, meta).0)
}

/// The logical path of a plain file below a root, if the store can hold its name.
fn logical_of(root: Root, rest: &Path) -> Option<LogicalPath> {
    rest.to_str().map(|r| format!("{}/{}", root.name(), separators(r))).and_then(|text| LogicalPath::new(&text).ok())
}

impl Store {
    /// The profile's plain files sealed into this store (the move). Safe to
    /// run at every start: it does nothing when nothing plain is left.
    pub fn move_plain_files(&self) -> Result<Moved, Error> {
        self.move_plain_files_beside(&[])
    }

    /// The move, the person's own folders (notes, papers, letters, where
    /// scans arrive) known: one that lies inside a folder of Sioul's that has
    /// not moved yet is kept in clear there, never sealed where it lies (the
    /// person placed it there before Sioul sealed anything); one placed
    /// inside a folder moved before was placed there knowing the trade-off,
    /// and its plain files are sealed in place.
    pub fn move_plain_files_beside(&self, person: &[PathBuf]) -> Result<Moved, Error> {
        self.move_plain_files_said(person, &|_, _| {})
    }

    /// How many plain files the move would take now (none: nothing to show
    /// before the window): neither the person's folders kept in clear, nor a
    /// name the store cannot hold, nor a file noted as left and unchanged
    /// since; a copy a cut move left aside counts (review-finish G1).
    pub fn plain_files_left(&self) -> usize {
        let left = self.left();
        let clear = self.clear_folders();
        Root::ALL
            .iter()
            .map(|root| {
                let top = self.profile().of(*root).to_path_buf();
                let mut plain = Vec::new();
                walk(&top, &top, *root, &mut plain);
                let waiting = plain
                    .iter()
                    .filter(|(place, rest)| {
                        let Some(logical) = logical_of(*root, rest) else { return false };
                        !clear.iter().any(|folder| logical.is_within(folder)) && !left.get(logical.as_str()).is_some_and(|was| stamp(place).as_ref() == Some(was))
                    })
                    .count();
                let mut aside = Vec::new();
                walk_all(&top.join(ASIDE), &top.join(ASIDE), &mut aside);
                waiting + aside.len()
            })
            .sum()
    }

    /// The move, `progress` told how many files are done of how many, after
    /// each (the screen before the window). A move stopped there goes on at
    /// the next start where it stopped.
    pub fn move_plain_files_said(&self, person: &[PathBuf], progress: &dyn Fn(usize, usize)) -> Result<Moved, Error> {
        let total = self.plain_files_left();
        let done = std::cell::Cell::new(0usize);
        progress(0, total);
        let _profile = ProfileLock::exclusive(self.profile())?;
        let moved_before = match self.moved() {
            Ok(moved) => moved,
            // Its record damaged: every folder is taken as moved, so the person's folders found now are sealed in place, as placed knowingly.
            Err(Error::Damaged { .. }) => Root::ALL.iter().map(|r| r.name().to_string()).collect(),
            Err(e) => return Err(e),
        };
        let day = jiff::Zoned::now().date().to_string();
        let mut out = Moved::default();
        // The files noted as left before (tried again in place, the profile's own), then this move's.
        let noted = self.left();
        let mut left: BTreeMap<String, Stamp> = noted.clone();
        for root in Root::ALL {
            let top = self.profile().of(root).to_path_buf();
            let done_before = moved_before.contains(root.name());
            self.recover_aside(root, &top, &day, &mut out);
            // The person's folders inside this one: kept in clear when it moves now; taken in when it moved before.
            let mut taken_in: Vec<LogicalPath> = Vec::new();
            for folder in person {
                let Ok(rest) = folder.strip_prefix(&top) else { continue };
                if rest.as_os_str().is_empty() {
                    continue;
                }
                let Some(logical) = rest.to_str().map(|r| format!("{}/{}", root.name(), separators(r))).and_then(|t| LogicalPath::new(t.trim_end_matches('/')).ok()) else { continue };
                if self.kept_clear(&logical) {
                    continue;
                }
                if done_before {
                    taken_in.push(logical);
                } else {
                    self.keep_in_clear(&logical)?;
                }
            }
            left.retain(|logical, _| !LogicalPath::new(logical).is_ok_and(|l| l.root_name() == root.name()));
            let mut plain = Vec::new();
            walk(&top, &top, root, &mut plain);
            for (place, rest) in plain {
                let Some(logical) = logical_of(root, &rest) else {
                    out.left.push(place);
                    continue;
                };
                if self.kept_clear(&logical) {
                    continue;
                }
                done.set(done.get() + 1);
                progress(done.get().min(total), total);
                // In a folder moved before, a plain file is not the profile's (an
                // older Sioul's, another program's, a backup put back by hand): set
                // aside, never sealed over the current file (third check, D24
                // reverted); but a file the move left before is the profile's, tried
                // again in place, and a new file in one of the person's folders placed
                // inside Sioul's knowingly is taken in.
                let own = !done_before || noted.contains_key(logical.as_str()) || (taken_in.iter().any(|folder| logical.is_within(folder)) && self.stat(&logical).ok().flatten().is_none());
                if !own {
                    let Ok(aside) = LogicalPath::new(&format!("data/set-aside/{day}/{logical}")) else { continue };
                    match self.seal_plain(&place, &aside, How::unversioned()) {
                        Ok(Some(_)) => {
                            out.set_aside += 1;
                            out.aside_folder = Some(format!("data/set-aside/{day}"));
                        }
                        Ok(None) => {}
                        // Not set aside this time (unreadable, written to meanwhile): still not the profile's, tried again at the next start.
                        Err(_) => out.left.push(place),
                    }
                    continue;
                }
                // A file the move cannot take (unreadable, the disk full, a
                // read-back that differs, written to meanwhile) stays where it
                // is, in clear, listed and noted: the rest goes on, and it is tried again.
                match self.seal_plain(&place, &logical, how_for(&logical)) {
                    Ok(Some((bytes, _))) => {
                        out.bytes += bytes;
                        out.files += 1;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        if let Some(was) = stamp(&place) {
                            left.insert(logical.to_string(), was);
                        }
                        out.left.push(place);
                    }
                }
            }
            remove_empty_folders(&top, &top, root, &self.clear_folders());
            remove_empty_folders_all(&top.join(ASIDE));
            self.remove_old_locks(root, &top);
            // Noted before the folder is marked moved: from then on a plain file there reads as the profile's only if noted.
            self.set_left(&left)?;
            if !done_before {
                self.mark_moved(root.name())?;
            }
        }
        Ok(out)
    }

    /// The empty lock files an older Sioul made beside the files it locked
    /// (`.<name>.lock`, an account's `state/dav/<id>.lock`…), whose names say in
    /// clear what they lock (a mail folder, an account, a project): this
    /// version keeps those locks at their opaque places (`files::lock_target`),
    /// so these are taken away, each only when it is empty and nobody holds it.
    fn remove_old_locks(&self, root: Root, top: &Path) {
        let mut locks = Vec::new();
        walk_locks(top, top, root, &mut locks);
        for (place, rest) in locks {
            if super::files::lock_target(self, root, &rest).is_none() {
                continue;
            }
            let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(&place) else { continue };
            if file.metadata().is_ok_and(|m| m.len() == 0) && file.try_lock().is_ok() {
                let _ = std::fs::remove_file(&place);
            }
        }
    }

    /// The files noted as left, tried again (the window, once open, away from
    /// the screen): each one taken now leaves the note. How many are still left.
    pub fn retry_left(&self) -> Result<usize, Error> {
        let noted = self.left();
        if noted.is_empty() {
            return Ok(0);
        }
        let _profile = ProfileLock::exclusive(self.profile())?;
        let mut still = BTreeMap::new();
        for logical in noted.keys() {
            let Ok(logical) = LogicalPath::new(logical) else { continue };
            let Some(place) = self.plain_place(&logical) else { continue };
            if std::fs::symlink_metadata(&place).is_err() {
                continue;
            }
            if self.seal_plain(&place, &logical, how_for(&logical)).is_err()
                && let Some(was) = stamp(&place)
            {
                still.insert(logical.to_string(), was);
            }
        }
        let count = still.len();
        self.set_left(&still)?;
        Ok(count)
    }

    /// Whether `root`'s folder was marked moved (read again at most each two
    /// seconds while it was not: a folder once moved stays so).
    pub(crate) fn root_moved(&self, root: Root) -> bool {
        let mut seen = self.moved_seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if seen.1.contains(root.name()) {
            return true;
        }
        if seen.0.is_none_or(|at| at.elapsed() >= std::time::Duration::from_secs(2)) {
            seen.0 = Some(std::time::Instant::now());
            match self.moved() {
                Ok(moved) => seen.1.extend(moved),
                // Its record damaged: every folder taken as moved, as the move does.
                Err(Error::Damaged { .. }) => seen.1.extend(Root::ALL.iter().map(|r| r.name().to_string())),
                Err(_) => {}
            }
        }
        seen.1.contains(root.name())
    }

    /// Whether the move noted `logical`'s plain file as left in clear (read
    /// again at most each two seconds): it reads as the profile's until taken.
    pub(crate) fn noted_left(&self, logical: &LogicalPath) -> bool {
        let mut seen = self.left_seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if seen.as_ref().is_none_or(|(at, _)| at.elapsed() >= std::time::Duration::from_secs(2)) {
            let names = self.left().into_keys().collect();
            *seen = Some((std::time::Instant::now(), names));
        }
        seen.as_ref().is_some_and(|(_, names)| names.contains(logical.as_str()))
    }

    /// Where a logical path's plain file lies in the profile's folders.
    fn plain_place(&self, logical: &LogicalPath) -> Option<PathBuf> {
        let root = Root::ALL.into_iter().find(|r| r.name() == logical.root_name())?;
        let rest = logical.as_str().strip_prefix(root.name())?.trim_start_matches('/');
        Some(self.profile().of(root).join(rest))
    }

    /// The files the move left, as noted (sealed, beside the store's own records).
    fn left(&self) -> BTreeMap<String, Stamp> {
        let Ok(Some(opened)) = self.read(&LogicalPath::internal("left")) else { return BTreeMap::new() };
        String::from_utf8_lossy(&opened.bytes)
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(5, '\t');
                let size = parts.next()?.parse().ok()?;
                let modified = parts.next()?.parse().ok()?;
                let number = parts.next()?.parse().ok()?;
                let changed = parts.next()?.parse().ok()?;
                Some((parts.next()?.to_string(), (size, modified, number, changed)))
            })
            .collect()
    }

    fn set_left(&self, left: &BTreeMap<String, Stamp>) -> Result<(), Error> {
        let path = LogicalPath::internal("left");
        if left.is_empty() {
            if self.stat(&path)?.is_some() {
                self.remove(&path, super::Expect::Any)?;
            }
            *self.left_seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((std::time::Instant::now(), Default::default()));
            return Ok(());
        }
        let text: String = left.iter().map(|(logical, (size, modified, number, changed))| format!("{size}\t{modified}\t{number}\t{changed}\t{logical}\n")).collect();
        self.write(&path, text.as_bytes(), How::unversioned())?;
        *self.left_seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((std::time::Instant::now(), left.keys().cloned().collect()));
        Ok(())
    }

    /// The plain copies a cut move left aside below `top`, found again: one
    /// that a version of its file holds goes (it was sealed); one never sealed
    /// goes back to its place, to be moved; any other (written to in the
    /// moment before the cut) is sealed apart, said, never lost.
    fn recover_aside(&self, root: Root, top: &Path, day: &str, out: &mut Moved) {
        let base = top.join(ASIDE);
        let mut found = Vec::new();
        walk_all(&base, &base, &mut found);
        for (aside, rest) in found {
            let place = top.join(&rest);
            let Some(logical) = logical_of(root, &rest) else { continue };
            let Ok(bytes) = std::fs::read(&aside) else { continue };
            let held = self.versions(&logical).unwrap_or_default().iter().any(|v| self.read_version(&logical, v.generation).is_ok_and(|o| o.bytes == bytes));
            if held {
                let _ = std::fs::remove_file(&aside);
            } else if self.stat(&logical).ok().flatten().is_none() && std::fs::symlink_metadata(&place).is_err() {
                let _ = put_back(&aside, &place, false);
            } else if let Ok(apart) = LogicalPath::new(&format!("data/set-aside/{day}/{logical}"))
                && self.write(&apart, &bytes, How::unversioned()).is_ok()
            {
                let _ = std::fs::remove_file(&aside);
                out.set_aside += 1;
                out.aside_folder = Some(format!("data/set-aside/{day}"));
            }
        }
        remove_empty_folders_all(&base);
    }

    /// One plain file sealed at `logical`, then its plain copy removed: the
    /// bytes moved and whether a new version was sealed for them (none when
    /// the file went meanwhile). It is read and sealed where it lies, then
    /// renamed aside, found unchanged and the same as its sealed copy, and
    /// only then removed; else put back, an error (left this time).
    fn seal_plain(&self, place: &Path, logical: &LogicalPath, how: How) -> Result<Option<(u64, bool)>, Error> {
        let io = |e: std::io::Error| Error::Io(format!("{}: {e}", place.display()));
        let before = match std::fs::symlink_metadata(place) {
            Ok(meta) if meta.is_file() => kept_by_rename(place, &meta),
            Ok(_) => return Err(Error::Refused(format!("{}: not a file", place.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io(e)),
        };
        let size = before.0;
        // Whether the sealed copy holds what the plain file at `at` holds (a move stopped after sealing it: not sealed twice).
        let same = |at: &Path| -> Result<bool, Error> {
            let Some(entry) = self.stat(logical)? else { return Ok(false) };
            if entry.size != size {
                return Ok(false);
            }
            let mut sealed = Vec::new();
            self.read_into(logical, &mut sealed)?;
            let mut plain = Vec::new();
            std::fs::File::open(at).and_then(|mut f| f.read_to_end(&mut plain)).map_err(io)?;
            Ok(sealed == plain)
        };
        let sealed_now = !same(place)?;
        if sealed_now {
            if size > STREAMED {
                let mut file = std::fs::File::open(place).map_err(io)?;
                self.write_from(logical, &mut file, how)?;
            } else {
                let bytes = std::fs::read(place).map_err(io)?;
                self.write(logical, &bytes, how)?;
            }
        }
        let changed = || Error::Damaged { path: logical.to_string(), what: "it was written to while it was moved, or did not read back as it was sealed: its plain copy is kept".to_string() };
        let top = self.profile().of(logical.root());
        let aside = top.join(ASIDE).join(place.strip_prefix(top).unwrap_or(place));
        let renamed = aside.parent().is_some_and(|dir| super::disk::private_dir(dir).is_ok()) && std::fs::rename(place, &aside).is_ok();
        if !renamed {
            // Nowhere to put it aside (another disk below this folder): checked where it is, then removed.
            let now = std::fs::symlink_metadata(place).map_err(io)?;
            if kept_by_rename(place, &now) != before || !same(place)? {
                return Err(changed());
            }
            std::fs::remove_file(place).map_err(io)?;
            return Ok(Some((size, sealed_now)));
        }
        let unchanged = std::fs::symlink_metadata(&aside).is_ok_and(|now| kept_by_rename(&aside, &now) == before) && same(&aside).unwrap_or(false);
        if !unchanged {
            // Written to meanwhile: what it holds now sealed as the file's newest version (nothing lost), and the file back in its place.
            let kept = std::fs::read(&aside).is_ok_and(|bytes| self.write(logical, &bytes, how).is_ok());
            put_back(&aside, place, kept).map_err(io)?;
            return Err(changed());
        }
        std::fs::remove_file(&aside).map_err(io)?;
        Ok(Some((size, sealed_now)))
    }
}

/// A copy set aside put back in its place, never over a file there now (a
/// write that came after it was set aside, which then stays the file): linked,
/// then its aside name removed; where links cannot be made, renamed if
/// nothing is there. When a file is there now, the copy goes only if it is
/// `kept` in the store, else it waits aside for the next move to find it.
fn put_back(aside: &Path, place: &Path, kept: bool) -> std::io::Result<()> {
    match std::fs::hard_link(aside, place) {
        Ok(()) => std::fs::remove_file(aside),
        Err(_) if std::fs::symlink_metadata(place).is_ok() => {
            if kept {
                std::fs::remove_file(aside)
            } else {
                Ok(())
            }
        }
        Err(_) => std::fs::rename(aside, place),
    }
}

/// The lock files (`*.lock`) below `dir`, with their paths below `top`: not
/// in the store's own folders nor in what stays in clear by rule.
fn walk_locks(top: &Path, dir: &Path, root: Root, out: &mut Vec<(PathBuf, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let place = entry.path();
        let Ok(rest) = place.strip_prefix(top) else { continue };
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if !stays_plain(root, rest) {
                walk_locks(top, &place, root, out);
            }
        } else if kind.is_file() && rest.to_str().is_some_and(|r| r.ends_with(".lock")) {
            out.push((place.clone(), rest.to_path_buf()));
        }
    }
}

/// Every file below `dir`, with its path below `top` (the copies set aside).
fn walk_all(top: &Path, dir: &Path, out: &mut Vec<(PathBuf, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let place = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            walk_all(top, &place, out);
        } else if kind.is_file()
            && let Ok(rest) = place.strip_prefix(top)
        {
            out.push((place.clone(), rest.to_path_buf()));
        }
    }
}

/// Empty folders below `dir` taken out, and `dir` itself when empty.
fn remove_empty_folders_all(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            remove_empty_folders_all(&entry.path());
        }
    }
    let _ = std::fs::remove_dir(dir);
}

/// A path below a root as the store names it: on Windows its `\\` are
/// separators; elsewhere a `\\` is a byte of a name like any other, which the
/// store refuses, so the file stays where it is, in clear (`Moved::left`).
fn separators(rest: &str) -> String {
    if cfg!(windows) { rest.replace('\\', "/") } else { rest.to_string() }
}

/// Every plain file below `dir` that the store takes (not its own, not what stays in clear), with its path below `top`.
fn walk(top: &Path, dir: &Path, root: Root, out: &mut Vec<(PathBuf, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let place = entry.path();
        let Ok(rest) = place.strip_prefix(top) else { continue };
        if stays_plain(root, rest) {
            continue;
        }
        // Links are never followed: what they point to is not Sioul's.
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            walk(top, &place, root, out);
        } else if kind.is_file() {
            out.push((place.clone(), rest.to_path_buf()));
        }
    }
}

/// Folders left empty by the move taken out (never the root's own, nor what
/// stays in clear, nor the person's folders kept in clear and what holds them).
fn remove_empty_folders(top: &Path, dir: &Path, root: Root, clear: &[LogicalPath]) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let place = entry.path();
        let Ok(rest) = place.strip_prefix(top) else { continue };
        if stays_plain(root, rest) || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let logical = rest.to_str().and_then(|r| LogicalPath::new(&format!("{}/{}", root.name(), separators(r))).ok());
        if logical.is_some_and(|l| clear.iter().any(|c| c.is_within(&l) || l.is_within(c))) {
            continue;
        }
        remove_empty_folders(top, &place, root, clear);
        let _ = std::fs::remove_dir(&place);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! One writer at a time on a file that two parts of Sioul change: the window
//! marks a dose in the doses' record while the sharing writes in it what your
//! other devices marked. One writing an older copy over the other would lose a
//! dose taken, and the sharing would then take it out on every device. An
//! advisory lock on a hidden file beside it (`.<name>.lock`, which the
//! sharing never carries), held while the file is read, changed and written.

use std::path::{Path, PathBuf};

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
/// added, what was taken out taken out); the rest stays as the file holds it.
/// Nothing read before (a new file, one that did not read): `state` whole.
/// Written beside, then moved.
pub fn save_merged<T: serde::Serialize>(path: &Path, read: &Read, state: &T) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let new = match toml::Value::try_from(state).map_err(|e| e.to_string())? {
        toml::Value::Table(table) => table,
        _ => return Err(format!("{}: not a table", path.display())),
    };
    with_lock(path, || {
        let current = std::fs::read_to_string(path).ok().and_then(|text| text.parse::<toml::Table>().ok());
        let table = match (&read.0, current) {
            (Some(base), Some(mut current)) => {
                merge(&mut current, base, &new);
                current
            }
            _ => new.clone(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let temporary = path.with_file_name(format!(".{}.sioul.tmp", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
        std::fs::write(&temporary, toml::to_string(&table).map_err(|e| e.to_string())?).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
    })
}

/// What changed from `base` to `new`, set over `current`.
fn merge(current: &mut toml::Table, base: &toml::Table, new: &toml::Table) {
    let keys: std::collections::BTreeSet<String> = base.keys().chain(new.keys()).cloned().collect();
    for key in keys {
        match (base.get(&key), new.get(&key)) {
            (before, after) if before == after => {}
            (Some(toml::Value::Table(before)), Some(toml::Value::Table(after))) => match current.get_mut(&key) {
                Some(toml::Value::Table(inner)) => merge(inner, before, after),
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
                }
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

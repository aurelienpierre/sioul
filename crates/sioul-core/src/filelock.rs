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

/// `f`, with the lock on `path` held: other threads and programs wait for it.
/// Where no lock can be taken (a folder not writable), `f` runs all the same:
/// writing it fails there anyway.
pub fn with_lock<T>(path: &Path, f: impl FnOnce() -> T) -> T {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(false).write(true);
    // Empty, it says nothing; yours alone all the same, as what it guards is (0600 on Unix).
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = path.parent().and_then(|parent| std::fs::create_dir_all(parent).ok()).and_then(|()| options.open(lock_path(path)).ok());
    let locked = file.as_ref().is_some_and(|f| f.lock().is_ok());
    let out = f();
    if locked && let Some(file) = &file {
        let _ = file.unlock();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul remind --watch --session` (TODO §4, 10 November 2026: the watcher
//! stopped on 7 November and nothing started it again). The real program,
//! started from a copy in a folder of the test's own: an install that puts a
//! new program in its place is followed, the same process running the new
//! file; a second watcher leaves at once; one started for the session leaves
//! with 0 once reminders with the window closed are turned off (their entry
//! gone), or at once when they are off. A home made in the system's
//! temporary folder, its XDG folders and HOME there: the person's own files
//! are never read.

#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

const FOLDERS: [(&str, &str); 5] = [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache"), ("HOME", "home")];

/// The program built for these tests.
const BUILT: &str = env!("CARGO_BIN_EXE_sioul");

/// Five seconds between two looks, a second for a new file to stay as it
/// is, its `--version` tried, and room for a loaded machine.
const WITHIN: Duration = Duration::from_secs(40);

fn home() -> PathBuf {
    let root = std::env::temp_dir().join(format!("sioul-remind-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (_, folder) in FOLDERS {
        std::fs::create_dir_all(root.join(folder)).unwrap();
        std::fs::write(root.join(folder).join(".sioul-stand-in"), "A folder made for Sioul's tests.").unwrap();
    }
    std::fs::create_dir_all(root.join("config/sioul")).unwrap();
    std::fs::write(root.join("config/sioul/config.toml"), "language = \"en\"\n").unwrap();
    std::fs::create_dir_all(root.join("bin")).unwrap();
    root
}

/// The entry of reminders with the window closed, as the window writes it.
fn entry(root: &Path) -> PathBuf {
    root.join("config/autostart/sioul-reminders.desktop")
}

fn watcher(root: &Path, program: &Path) -> (Child, Receiver<String>) {
    let mut command = Command::new(program);
    command.arg("--config").arg(root.join("config/sioul/config.toml")).args(["--language", "en", "remind", "--watch", "--session"]);
    for (variable, folder) in FOLDERS {
        command.env(variable, root.join(folder));
    }
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, lines) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    (child, lines)
}

/// The file the process runs: its inode.
fn running(child: &Child) -> u64 {
    std::fs::metadata(format!("/proc/{}/exe", child.id())).unwrap().ino()
}

/// Waits for `done`, `WITHIN` at most.
fn until(mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < WITHIN {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

#[test]
fn the_watcher_follows_an_install_and_leaves_when_turned_off() {
    let root = home();
    let program = root.join("bin/sioul");
    std::fs::copy(BUILT, &program).unwrap();
    // Off: a watcher for the session leaves at once, saying nothing.
    let (mut off, lines) = watcher(&root, &program);
    assert!(off.wait().unwrap().success());
    assert!(lines.recv_timeout(Duration::from_secs(1)).is_err(), "nothing said");
    // On: it watches.
    std::fs::create_dir_all(entry(&root).parent().unwrap()).unwrap();
    std::fs::write(entry(&root), "[Desktop Entry]\nType=Application\nName=Sioul reminders\nExec=sioul remind --watch --session\n").unwrap();
    let (mut child, lines) = watcher(&root, &program);
    assert_eq!(lines.recv_timeout(WITHIN).unwrap(), "Reminders watched: each one comes once, as a quiet notification. Ctrl+C stops.");
    let first = running(&child);
    assert_eq!(first, std::fs::metadata(&program).unwrap().ino());
    // A second one leaves at once: one runs.
    let (mut second, second_lines) = watcher(&root, &program);
    assert!(second.wait().unwrap().success());
    assert!(second_lines.recv_timeout(WITHIN).unwrap().contains("already"), "it says one runs");
    // An install: a new file moved over the program, as tools/install.sh does.
    let staged = root.join("bin/.sioul.new");
    std::fs::copy(BUILT, &staged).unwrap();
    std::fs::rename(&staged, &program).unwrap();
    let installed = std::fs::metadata(&program).unwrap().ino();
    assert_ne!(installed, first);
    assert!(until(|| running(&child) == installed), "the same process runs the new program");
    assert_eq!(lines.recv_timeout(WITHIN).unwrap(), "Reminders watched: each one comes once, as a quiet notification. Ctrl+C stops.", "the new program watches, the lock taken again");
    assert!(child.try_wait().unwrap().is_none());
    // Turned off: its entry gone, it leaves with 0, which nothing starts again.
    std::fs::remove_file(entry(&root)).unwrap();
    let mut status = None;
    assert!(until(|| {
        status = child.try_wait().unwrap();
        status.is_some()
    }));
    assert!(status.unwrap().success(), "{status:?}");
    let _ = std::fs::remove_dir_all(&root);
}

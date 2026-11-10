// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul remind --watch --session` (TODO §4, 10 November 2026: the watcher
//! stopped on 7 November and nothing started it again). The real program,
//! started from a copy in a folder of the test's own: an install that puts a
//! new program in its place is followed, the same process running the new
//! file; a second watcher leaves at once; one started for the session leaves
//! with 0 once reminders with the window closed are turned off (their entry
//! gone), or at once when they are off. A home made in Cargo's own folder
//! for tests' files, its XDG folders and HOME there, and a session bus that
//! is none: the person's own files and desktop are never reached. Every
//! watcher started is killed and the home removed however the test ends.

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

/// A home of the test's own, removed however the test ends, and the watchers
/// started in it, killed then.
struct Home {
    root: PathBuf,
    children: Vec<Child>,
}

impl Drop for Home {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn home() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("sioul-remind-watch-{}", std::process::id()));
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
    // No session bus: a reminder it told would reach no desktop.
    command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", root.join("no-bus").display()));
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
    let mut kept = Home { root: home(), children: Vec::new() };
    let root = kept.root.clone();
    let program = root.join("bin/sioul");
    std::fs::copy(BUILT, &program).unwrap();
    // Off: a watcher for the session leaves at once, saying nothing.
    let (off, lines) = watcher(&root, &program);
    kept.children.push(off);
    assert!(kept.children[0].wait().unwrap().success());
    assert!(lines.recv_timeout(Duration::from_secs(1)).is_err(), "nothing said");
    // Turned off by the desktop (XFCE writes Hidden=true): off too.
    std::fs::create_dir_all(entry(&root).parent().unwrap()).unwrap();
    std::fs::write(entry(&root), "[Desktop Entry]\nType=Application\nName=Sioul reminders\nExec=sioul remind --watch --session\nHidden=true\n").unwrap();
    let (hidden, lines) = watcher(&root, &program);
    kept.children.push(hidden);
    assert!(kept.children[1].wait().unwrap().success());
    assert!(lines.recv_timeout(Duration::from_secs(1)).is_err(), "nothing said");
    // On: it watches.
    std::fs::write(entry(&root), "[Desktop Entry]\nType=Application\nName=Sioul reminders\nExec=sioul remind --watch --session\n").unwrap();
    let (child, lines) = watcher(&root, &program);
    kept.children.push(child);
    let watching = 2;
    assert_eq!(lines.recv_timeout(WITHIN).unwrap(), "Reminders watched: each one comes once, as a quiet notification. Ctrl+C stops.");
    let first = running(&kept.children[watching]);
    assert_eq!(first, std::fs::metadata(&program).unwrap().ino());
    // A second one leaves at once: one runs.
    let (second, second_lines) = watcher(&root, &program);
    kept.children.push(second);
    assert!(kept.children[3].wait().unwrap().success());
    assert!(second_lines.recv_timeout(WITHIN).unwrap().contains("already"), "it says one runs");
    // An install: a new file moved over the program, as tools/install.sh does.
    let staged = root.join("bin/.sioul.new");
    std::fs::copy(BUILT, &staged).unwrap();
    std::fs::rename(&staged, &program).unwrap();
    let installed = std::fs::metadata(&program).unwrap().ino();
    assert_ne!(installed, first);
    assert!(until(|| running(&kept.children[watching]) == installed), "the same process runs the new program");
    assert_eq!(lines.recv_timeout(WITHIN).unwrap(), "Reminders watched: each one comes once, as a quiet notification. Ctrl+C stops.", "the new program watches, the lock taken again");
    assert!(kept.children[watching].try_wait().unwrap().is_none());
    // Turned off: its entry gone, it leaves with 0, which nothing starts again.
    std::fs::remove_file(entry(&root)).unwrap();
    let mut status = None;
    assert!(until(|| {
        status = kept.children[watching].try_wait().unwrap();
        status.is_some()
    }));
    assert!(status.unwrap().success(), "{status:?}");
}

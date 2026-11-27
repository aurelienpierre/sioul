// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul store` and `sioul settings` (docs/store.md, "Commands"): the real
//! program, on a home made in Cargo's own folder for tests' files, its XDG
//! folders and HOME there, no session bus, and the store's fixed test key
//! (`SIOUL_TEST_STORE_KEY`, a test build's alone): the person's own files
//! and keyring are never reached.

#![cfg(unix)]

use sioul_core::sealed::{Folders, LogicalPath, Options, RootKey, Store};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const FOLDERS: [(&str, &str); 5] = [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache"), ("HOME", "home")];

struct Home(PathBuf);

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn home(name: &str) -> Home {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("sioul-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (_, folder) in FOLDERS {
        std::fs::create_dir_all(root.join(folder)).unwrap();
        std::fs::write(root.join(folder).join(".sioul-stand-in"), "A folder made for Sioul's tests.").unwrap();
    }
    std::fs::create_dir_all(root.join("config/sioul")).unwrap();
    std::fs::write(root.join("config/sioul/config.toml"), "language = \"en\"\n").unwrap();
    Home(root)
}

/// The profile's four folders in this home, as the program finds them.
fn folders(root: &Path) -> Folders {
    Folders { config: root.join("config/sioul"), data: root.join("data/sioul"), state: root.join("state/sioul"), cache: root.join("cache/sioul") }
}

fn sioul(root: &Path, key: bool, args: &[&str], input: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sioul"));
    command.arg("--config").arg(root.join("config/sioul/config.toml")).args(["--language", "en"]).args(args);
    for (variable, folder) in FOLDERS {
        command.env(variable, root.join(folder));
    }
    command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", root.join("no-bus").display()));
    command.env_remove("XDG_RUNTIME_DIR");
    if key {
        command.env("SIOUL_TEST_STORE_KEY", "1");
    } else {
        command.env_remove("SIOUL_TEST_STORE_KEY");
    }
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    if let Some(input) = input {
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

fn said(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

#[test]
fn configuration_through_the_seal() {
    let home = home("config");
    let root = &home.0;
    // No store yet: said, nothing made.
    let none = sioul(root, true, &["store", "cat", "config/config.toml"], None);
    assert!(!none.status.success() && said(&none).contains("keeps nothing sealed"), "{}", said(&none));
    Store::create(&folders(root), &RootKey::test(), Options::default()).unwrap();

    let put = sioul(root, true, &["store", "put", "config/config.toml"], Some(b"language = \"fr\"\n"));
    assert!(put.status.success(), "{}", said(&put));
    let cat = sioul(root, true, &["store", "cat", "config/config.toml"], None);
    assert_eq!(String::from_utf8_lossy(&cat.stdout), "language = \"fr\"\n");
    // A file that does not read as its kind: refused, nothing written.
    let bad = sioul(root, true, &["store", "put", "config/config.toml"], Some(b"language = [\n"));
    assert!(!bad.status.success() && said(&bad).contains("does not read"), "{}", said(&bad));
    let put = sioul(root, true, &["store", "put", "config/config.toml"], Some(b"language = \"en\"\n"));
    assert!(put.status.success(), "{}", said(&put));
    let versions = sioul(root, true, &["store", "versions", "config/config.toml"], None);
    assert_eq!(String::from_utf8_lossy(&versions.stdout).lines().count(), 2, "now and one earlier: {}", said(&versions));
    // Put back, and undone the same way.
    let restored = sioul(root, true, &["store", "restore", "config/config.toml", "--version", "1"], None);
    assert!(restored.status.success(), "{}", said(&restored));
    assert_eq!(String::from_utf8_lossy(&sioul(root, true, &["store", "cat", "config/config.toml"], None).stdout), "language = \"fr\"\n");
    sioul(root, true, &["store", "restore", "config/config.toml", "--version", "1"], None);
    assert_eq!(String::from_utf8_lossy(&sioul(root, true, &["store", "cat", "config/config.toml"], None).stdout), "language = \"en\"\n");
    let earlier = sioul(root, true, &["store", "cat", "config/config.toml", "--version", "1"], None);
    assert_eq!(String::from_utf8_lossy(&earlier.stdout), "language = \"fr\"\n");

    // Beyond the configuration: --any; a client's file: --clients.
    let store = Store::open(&folders(root), &RootKey::test(), Options::default()).unwrap();
    store.write(&LogicalPath::new("data/records/persons.toml").unwrap(), b"[a]\n", Default::default()).unwrap();
    store.write(&LogicalPath::new("data/clients/c1/description.md").unwrap(), b"Allergies: none", Default::default()).unwrap();
    drop(store);
    let refused = sioul(root, true, &["store", "cat", "data/records/persons.toml"], None);
    assert!(!refused.status.success() && said(&refused).contains("--any"), "{}", said(&refused));
    assert_eq!(sioul(root, true, &["store", "cat", "data/records/persons.toml", "--any"], None).stdout, b"[a]\n");
    let refused = sioul(root, true, &["store", "cat", "data/clients/c1/description.md", "--any"], None);
    assert!(!refused.status.success() && said(&refused).contains("--clients"), "{}", said(&refused));
    assert_eq!(sioul(root, true, &["store", "cat", "data/clients/c1/description.md", "--clients"], None).stdout, b"Allergies: none");

    let ls = sioul(root, true, &["store", "ls", "config"], None);
    assert!(ls.status.success() && String::from_utf8_lossy(&ls.stdout).contains("config/config.toml"), "{}", said(&ls));
    let check = sioul(root, true, &["store", "check"], None);
    assert!(check.status.success(), "{}", said(&check));
    // Editing in place needs the memory-backed folder: none here, so it says what to do instead.
    let edit = sioul(root, true, &["store", "edit", "config/config.toml"], None);
    assert!(!edit.status.success() && said(&edit).contains("sioul store cat"), "{}", said(&edit));

    // Unsealed: today's layout, the clients' files only when asked.
    let out = root.join("unsealed");
    let unseal = sioul(root, true, &["store", "unseal", "--to", out.to_str().unwrap()], None);
    assert!(unseal.status.success(), "{}", said(&unseal));
    assert_eq!(std::fs::read_to_string(out.join("config/config.toml")).unwrap(), "language = \"en\"\n");
    assert!(out.join("data/records/persons.toml").exists() && !out.join("data/clients").exists());
    assert!(said(&unseal).contains("--clients"));

    // Without the test key, this test build's keyring keeps nothing: told, nothing read.
    let keyless = sioul(root, false, &["store", "cat", "config/config.toml"], None);
    assert!(!keyless.status.success() && said(&keyless).contains("keyring holds no key"), "{}", said(&keyless));
}

#[test]
fn a_setting_read_and_changed_as_the_window_does() {
    let home = home("settings");
    let root = &home.0;
    let get = sioul(root, false, &["settings", "get", "language"], None);
    assert_eq!(String::from_utf8_lossy(&get.stdout).trim(), "en", "{}", said(&get));
    let unset = sioul(root, false, &["settings", "get", "no.such.thing"], None);
    assert!(!unset.status.success(), "{}", said(&unset));
}

#[test]
fn the_export_writes_every_file_in_clear_in_its_standard_format() {
    let home = home("export");
    let root = &home.0;
    let store = Store::create(&folders(root), &RootKey::test(), Options::default()).unwrap();
    store.write(&LogicalPath::new("config/config.toml").unwrap(), b"language = \"en\"\n", Default::default()).unwrap();
    store.write(&LogicalPath::new("data/calendars/local/perso/a1.ics").unwrap(), b"BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n", Default::default()).unwrap();
    store.write(&LogicalPath::new("data/mail/home/INBOX/cur/1700000000.M1P2.host:2,S").unwrap(), b"Subject: loyer\r\n\r\nmutuelle\r\n", Default::default()).unwrap();
    store.write(&LogicalPath::new("data/records/health.toml").unwrap(), b"[[day]]\n", Default::default()).unwrap();
    store.write(&LogicalPath::new("data/clients/c1/description.md").unwrap(), b"a client's words", Default::default()).unwrap();
    store.append_line(&LogicalPath::new("state/mcp/said.jsonl").unwrap(), "{\"n\":1}", Default::default()).unwrap();
    drop(store);
    let out = root.join("export");
    let export = sioul(root, true, &["store", "export", "--to", out.to_str().unwrap()], None);
    assert!(export.status.success(), "{}", said(&export));
    assert_eq!(std::fs::read_to_string(out.join("config/config.toml")).unwrap(), "language = \"en\"\n");
    assert_eq!(std::fs::read(out.join("data/calendars/local/perso/a1.ics")).unwrap(), b"BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n");
    assert_eq!(std::fs::read(out.join("data/mail/home/INBOX/cur/1700000000.M1P2.host:2,S")).unwrap(), b"Subject: loyer\r\n\r\nmutuelle\r\n", "a Maildir as mail programs read it");
    assert_eq!(std::fs::read_to_string(out.join("data/records/health.toml")).unwrap(), "[[day]]\n", "the person's own copy: health too");
    assert_eq!(std::fs::read_to_string(out.join("state/mcp/said.jsonl")).unwrap(), "{\"n\":1}\n", "a log as its lines");
    assert!(!out.join("data/clients").exists() && said(&export).contains("--clients"), "{}", said(&export));
    // Into a folder that is not empty: refused, nothing written.
    let again = sioul(root, true, &["store", "export", "--to", out.to_str().unwrap()], None);
    assert!(!again.status.success() && said(&again).contains("not empty"), "{}", said(&again));
    // The clients' files when asked.
    let all = root.join("all");
    let export = sioul(root, true, &["store", "export", "--to", all.to_str().unwrap(), "--clients"], None);
    assert!(export.status.success(), "{}", said(&export));
    assert_eq!(std::fs::read_to_string(all.join("data/clients/c1/description.md")).unwrap(), "a client's words");
}

#[test]
fn a_setting_is_read_through_the_seal_once_the_profile_is_sealed() {
    let home = home("settings-sealed");
    let root = &home.0;
    let store = Store::create(&folders(root), &RootKey::test(), Options::default()).unwrap();
    store.write(&LogicalPath::new("config/config.toml").unwrap(), b"language = \"fr\"\n", Default::default()).unwrap();
    drop(store);
    // A file the disk still holds (not moved yet) reads as it is, as the system's (review-finish F2).
    let get = sioul(root, true, &["settings", "get", "language"], None);
    assert_eq!(String::from_utf8_lossy(&get.stdout).trim(), "en", "{}", said(&get));
    // Once it is gone (moved in), the sealed one is read.
    std::fs::remove_file(root.join("config/sioul/config.toml")).unwrap();
    let get = sioul(root, true, &["settings", "get", "language"], None);
    assert_eq!(String::from_utf8_lossy(&get.stdout).trim(), "fr", "{}", said(&get));
    // Without the key, nothing is read in its place.
    let keyless = sioul(root, false, &["settings", "get", "language"], None);
    assert_eq!(String::from_utf8_lossy(&keyless.stdout).trim(), "", "{}", said(&keyless));
}

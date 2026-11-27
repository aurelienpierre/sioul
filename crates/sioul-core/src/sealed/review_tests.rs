// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The two reviews of the finished store (27 November 2026): each test shows
//! a defect the door had, now mended; kept so that it never comes back.

use super::files::{self, OpenOptions};
use super::*;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

fn scratch(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("sioul-review-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    base
}

#[test]
fn review_two_appends_keep_both_lines() {
    let base = scratch("append");
    let store = Arc::new(Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store);
    let log = base.join("state/log.txt");
    files::write(&log, "0\n").unwrap();
    let mut a = OpenOptions::new().create(true).append(true).open(&log).unwrap();
    let mut b = OpenOptions::new().create(true).append(true).open(&log).unwrap();
    a.write_all(b"a\n").unwrap();
    b.write_all(b"b\n").unwrap();
    drop(a);
    drop(b);
    let text = files::read_to_string(&log).unwrap();
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(text, "0\na\nb\n", "an append was lost");
}

#[test]
fn review_a_lock_on_a_sealed_file_excludes_a_second_handle() {
    let base = scratch("lock");
    let store = Arc::new(Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store);
    let lock = base.join("data/spam/corpus/acct/lock");
    files::create_dir_all(lock.parent().unwrap()).unwrap();
    let one = OpenOptions::new().create(true).truncate(false).write(true).open(&lock).unwrap();
    one.try_lock().unwrap();
    let two = OpenOptions::new().create(true).truncate(false).write(true).open(&lock).unwrap();
    let second = two.try_lock().is_ok();
    let _ = std::fs::remove_dir_all(&base);
    assert!(!second, "a second handle took the same lock");
}

#[test]
fn review_a_folder_s_time_moves_when_a_file_lands_in_it() {
    let base = scratch("dirtime");
    let store = Arc::new(Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store);
    let dir = base.join("data/calendars/acct/home");
    files::create_dir_all(&dir).unwrap();
    files::write(dir.join("a.ics"), "BEGIN:VCALENDAR\n").unwrap();
    let before = files::metadata(&dir).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    files::write(dir.join("b.ics"), "BEGIN:VCALENDAR\n").unwrap();
    let after = files::metadata(&dir).unwrap().modified().unwrap();
    let _ = std::fs::remove_dir_all(&base);
    assert_ne!(before, after, "a folder's time ({before:?}) did not move");
}

#[cfg(unix)]
#[test]
fn review_a_link_inside_sioul_s_folders_reads_after_the_move() {
    let base = scratch("link");
    let outside = base.join("dotfiles");
    std::fs::create_dir_all(outside.join("cal")).unwrap();
    std::fs::write(outside.join("config.toml"), "x = 1\n").unwrap();
    std::fs::write(outside.join("cal/e.ics"), "BEGIN:VCALENDAR\n").unwrap();
    let profile = base.join("profile");
    for root in ["config", "data", "state", "cache"] {
        std::fs::create_dir_all(profile.join(root)).unwrap();
    }
    std::os::unix::fs::symlink(outside.join("config.toml"), profile.join("config/config.toml")).unwrap();
    std::fs::create_dir_all(profile.join("data/calendars/acct")).unwrap();
    std::os::unix::fs::symlink(outside.join("cal"), profile.join("data/calendars/acct/home")).unwrap();
    std::fs::write(profile.join("state/dav.toml"), "known = [\"e.ics\"]\n").unwrap();
    let store = Arc::new(Store::create(&Folders::under(&profile), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store.clone());
    let moved = store.move_plain_files().unwrap();
    let state = files::read_to_string(profile.join("state/dav.toml")).ok();
    let still_links = std::fs::symlink_metadata(profile.join("config/config.toml")).is_ok_and(|m| m.file_type().is_symlink());
    let config = files::read_to_string(profile.join("config/config.toml")).map_err(|e| e.kind());
    let item = files::metadata(profile.join("data/calendars/acct/home/e.ics")).map(|_| ()).map_err(|e| e.kind());
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(state.as_deref(), Some("known = [\"e.ics\"]\n"), "the state moved");
    assert!(still_links, "the link is still on the disk");
    assert!(config.is_ok() && item.is_ok(), "through the door: config {config:?}, item {item:?}; left {:?}", moved.left);
}

#[test]
fn review_a_file_not_moved_yet_reads_as_it_is_on_the_disk() {
    let profile = scratch("gap");
    std::fs::create_dir_all(profile.join("state")).unwrap();
    std::fs::write(profile.join("state/porch.toml"), "kept = 1\n").unwrap();
    // The store made, the move not run yet: what another process reads at its next call.
    let store = Arc::new(Store::create(&Folders::under(&profile), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store);
    let read = files::read_to_string(profile.join("state/porch.toml")).map_err(|e| e.kind());
    let _ = std::fs::remove_dir_all(&profile);
    assert_eq!(read, Ok("kept = 1\n".to_string()));
}

#[cfg(unix)]
#[test]
fn review_a_backslash_in_a_name_keeps_its_file_after_the_move() {
    let profile = scratch("backslash");
    std::fs::create_dir_all(profile.join("data/calendars/a")).unwrap();
    std::fs::write(profile.join("data/calendars/a/x\\y.ics"), "BEGIN:VCALENDAR\n").unwrap();
    let store = Arc::new(Store::create(&Folders::under(&profile), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store.clone());
    let moved = store.move_plain_files().unwrap();
    let read = files::read_to_string(profile.join("data/calendars/a/x\\y.ics")).map_err(|e| e.kind());
    let elsewhere = store.stat(&LogicalPath::new("data/calendars/a/x/y.ics").unwrap()).unwrap().is_some();
    let _ = std::fs::remove_dir_all(&profile);
    assert!(read.is_ok(), "read {read:?}; sealed under data/calendars/a/x/y.ics: {elsewhere}; moved {moved:?}");
}

#[test]
fn review_two_creations_at_once_never_both_succeed() {
    let mut both = 0;
    for round in 0..40 {
        let base = scratch(&format!("create{round}"));
        let folders = Folders::under(&base);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = [1u8, 2u8]
            .into_iter()
            .map(|k| {
                let (folders, barrier) = (folders.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    Store::create(&folders, &RootKey::from_bytes([k; 32]), Options::default()).is_ok()
                })
            })
            .collect();
        let made: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        if made.iter().all(|m| *m) {
            both += 1;
        }
        let _ = std::fs::remove_dir_all(&base);
    }
    assert_eq!(both, 0, "two stores made over one profile in {both} rounds of 40");
}

#[test]
fn review_after_two_creations_at_once_both_writers_files_read() {
    let mut lost = 0;
    for round in 0..10 {
        let base = scratch(&format!("create-after{round}"));
        let folders = Folders::under(&base);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = [1u8, 2u8]
            .into_iter()
            .map(|k| {
                let (folders, barrier) = (folders.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    let store = Store::create(&folders, &RootKey::from_bytes([k; 32]), Options::default());
                    // What each start then moves in: only the start whose store was made (the other finds it made, `Exists`).
                    store.ok().and_then(|s| s.write(&LogicalPath::new(&format!("config/moved-by-{k}.toml")).unwrap(), b"x = 1\n", How::default()).ok().map(|_| k))
                })
            })
            .collect();
        let wrote: Vec<u8> = handles.into_iter().filter_map(|h| h.join().unwrap()).collect();
        // The next start opens with whichever key store.toml names: every file a made store wrote reads.
        let opened = [1u8, 2u8].into_iter().find_map(|k| Store::open(&folders, &RootKey::from_bytes([k; 32]), Options::default()).ok());
        let readable = opened.map_or(0, |s| wrote.iter().filter(|k| s.read(&LogicalPath::new(&format!("config/moved-by-{k}.toml")).unwrap()).ok().flatten().is_some()).count());
        if wrote.is_empty() || readable < wrote.len() {
            lost += 1;
        }
        let _ = std::fs::remove_dir_all(&base);
    }
    assert_eq!(lost, 0, "files written by one of two stores made at once unreadable at the next start in {lost} rounds of 10");
}

#[test]
fn review_a_sealed_write_refused_is_said_to_its_writer() {
    let base = scratch("silent");
    let store = Arc::new(Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store);
    let log = base.join("state/senders.txt");
    super::disk::ROOM.with(|r| r.set(Some(0)));
    let said = OpenOptions::new().create(true).append(true).open(&log).and_then(|mut f| f.write_all(b"blocked@example.org\n"));
    super::disk::ROOM.with(|r| r.set(None));
    let kept = files::read_to_string(&log).ok();
    let _ = std::fs::remove_dir_all(&base);
    assert!(said.is_err() || kept.is_some(), "the writer was told Ok, and nothing was kept ({kept:?})");
}

// The second review (db4165bb): each test shows a defect the move or the
// amend had, now mended.

#[test]
fn review2_a_kept_clear_folder_leaves_nothing_to_move() {
    let profile = scratch("left-kept");
    std::fs::create_dir_all(profile.join("data/notes")).unwrap();
    std::fs::write(profile.join("data/notes/a.md"), "# a\n").unwrap();
    let store = Arc::new(Store::create(&Folders::under(&profile), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store.clone());
    let moved = store.move_plain_files_beside(&[profile.join("data/notes")]).unwrap();
    let left = store.plain_files_left();
    let still = std::fs::read_to_string(profile.join("data/notes/a.md")).ok();
    let _ = std::fs::remove_dir_all(&profile);
    assert_eq!(still.as_deref(), Some("# a\n"), "the notes kept in clear");
    assert_eq!(left, 0, "after the move ({moved:?}), {left} plain files still 'to move': `begin` says Move at every start");
}

#[cfg(unix)]
#[test]
fn review2_a_file_the_move_left_leaves_nothing_to_move() {
    let profile = scratch("left-name");
    std::fs::create_dir_all(profile.join("data/calendars/a")).unwrap();
    std::fs::write(profile.join("data/calendars/a/x\\y.ics"), "BEGIN:VCALENDAR\n").unwrap();
    let store = Arc::new(Store::create(&Folders::under(&profile), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store.clone());
    let moved = store.move_plain_files().unwrap();
    let left = store.plain_files_left();
    let _ = std::fs::remove_dir_all(&profile);
    assert_eq!(left, 0, "after the move ({moved:?}), {left} plain files still 'to move'");
}

/// Turned round by the third check (D24 reverted): a plain file that appears
/// in a folder already moved (an older process's write, another program's, a
/// backup put back by hand) is never read as current; what this version writes
/// there is sealed; the next start sets the plain file aside and still reads
/// what was written through the door. Nothing is rolled back either way.
#[test]
fn review2_a_plain_file_read_as_the_profile_s_is_not_rolled_back_at_the_next_start() {
    let profile = scratch("rollback");
    std::fs::create_dir_all(profile.join("state")).unwrap();
    let path = profile.join("state/porch.toml");
    std::fs::write(&path, "v1\n").unwrap();
    let store = Arc::new(Store::create(&Folders::under(&profile), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store.clone());
    store.move_plain_files().unwrap();
    assert_eq!(files::read_to_string(&path).unwrap(), "v1\n");
    // A process that does not know the store writes the file in clear:
    std::fs::write(&path, "v2\n").unwrap();
    let read = files::read_to_string(&path).unwrap();
    // This version writes through the door, sealed:
    files::write(&path, "v3\n").unwrap();
    let sealed_now = store.read_text(&LogicalPath::new("state/porch.toml").unwrap()).unwrap().map(|(t, _)| t);
    // The next start.
    let next = store.move_plain_files().unwrap();
    let now = files::read_to_string(&path).unwrap();
    let aside = next.aside_folder.as_ref().and_then(|folder| store.read_text(&LogicalPath::new(&format!("{folder}/state/porch.toml")).unwrap()).ok().flatten()).map(|(t, _)| t);
    let plain_left = path.exists();
    let _ = std::fs::remove_dir_all(&profile);
    assert_eq!(read, "v1\n", "the plain file is never read as current");
    assert_eq!(sealed_now.as_deref(), Some("v3\n"), "what this version wrote is sealed");
    assert_eq!(now, "v3\n", "the next start set the plain file aside ({next:?}) and still reads what was written");
    assert_eq!((next.set_aside, aside.as_deref(), plain_left), (1, Some("v2\n"), false), "set aside, sealed, nothing lost");
}

#[cfg(unix)]
#[test]
fn review2_a_file_the_move_could_not_take_never_reads_as_gone() {
    use std::os::unix::fs::PermissionsExt;
    let profile = scratch("left-aside");
    std::fs::create_dir_all(profile.join("data/calendars/a")).unwrap();
    let item = profile.join("data/calendars/a/e.ics");
    std::fs::write(&item, "BEGIN:VCALENDAR\n").unwrap();
    std::fs::set_permissions(&item, std::fs::Permissions::from_mode(0o000)).unwrap();
    let store = Arc::new(Store::create(&Folders::under(&profile), &RootKey::test(), Options::default()).unwrap());
    let _m = files::mount(store.clone());
    let first = store.move_plain_files().unwrap();
    // The reason it could not be taken goes (a disk full for a moment, a mode put right).
    std::fs::set_permissions(&item, std::fs::Permissions::from_mode(0o600)).unwrap();
    let second = store.move_plain_files().unwrap();
    let read = files::read_to_string(&item).map_err(|e| e.kind());
    let _ = std::fs::remove_dir_all(&profile);
    assert_eq!(first.left.len(), 1, "left at the first move: {first:?}");
    assert!(read.is_ok(), "the item reads {read:?} after the next start ({second:?}): a calendar's sync sends DELETE for it");
}

#[test]
fn review2_an_amend_cut_by_a_power_cut_leaves_a_whole_current_file() {
    use super::disk::{CRASH, Step};
    let base = scratch("amend-cut");
    let folders = Folders::under(&base);
    let store = Store::create(&folders, &RootKey::test(), Options::default()).unwrap();
    let path = LogicalPath::new("state/porch.toml").unwrap();
    store.write(&path, b"old", How::default()).unwrap();
    let first = store.write(&path, b"first", How::default()).unwrap();
    CRASH.with(|c| c.set(Some((Step::AfterRename, 0))));
    let cut = store.amend(&path, b"second", first.generation, How::default());
    CRASH.with(|c| c.set(None));
    drop(store);
    let store = Store::open(&folders, &RootKey::test(), Options::default()).unwrap();
    let read = store.read(&path).map(|o| o.map(|o| (o.from, String::from_utf8_lossy(&o.bytes).to_string())));
    let _ = std::fs::remove_dir_all(&base);
    assert!(cut.is_err(), "the cut happened");
    assert!(matches!(&read, Ok(Some((Source::Current, text))) if text == "first" || text == "second"), "after a cut amend: {read:?}");
}

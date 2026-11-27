// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The door through paths (`files`) and the move: invented data only, in
//! folders of their own, the fixed test key. Each test mounts a store of its
//! own over its own folders, so that tests running at once never meet.

use super::files::{self, File, OpenOptions};
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn scratch(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("sioul-files-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn mounted(base: &Path) -> (Arc<Store>, files::Mounted) {
    let store = Arc::new(Store::create(&Folders::under(base), &RootKey::test(), Options::default()).unwrap());
    let guard = files::mount(store.clone());
    (store, guard)
}

/// Every plain file under `dir`, with its path below it.
fn plain_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            if entry.file_type().unwrap().is_dir() {
                stack.push(entry.path());
            } else {
                out.push(entry.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_path_under_sioul_s_folders_is_sealed_and_any_other_is_the_system_s() {
    let base = scratch("route");
    let (store, _mounted) = mounted(&base.join("profile"));
    let config = base.join("profile/config/config.toml");
    files::write(&config, "name = \"Lise Martin\"\n").unwrap();
    assert!(std::fs::metadata(&config).is_err(), "nothing in clear at its place");
    assert_eq!(files::read_to_string(&config).unwrap(), "name = \"Lise Martin\"\n");
    assert_eq!(store.read_text(&LogicalPath::new("config/config.toml").unwrap()).unwrap().unwrap().0, "name = \"Lise Martin\"\n");
    assert!(files::exists(&config) && files::is_file(&config) && files::is_dir(base.join("profile/config")));
    assert!(files::is_sealed(&config));
    for file in plain_files(&base.join("profile")) {
        let bytes = std::fs::read(base.join("profile").join(&file)).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("Lise"), "{file} holds a name in clear");
        assert!(!file.contains("config.toml"), "{file}: a name in clear");
    }
    // Outside Sioul's folders: the system's file, as before.
    let note = base.join("notes/Lise.md");
    files::create_dir_all(note.parent().unwrap()).unwrap();
    files::write(&note, "# Lise\n").unwrap();
    assert_eq!(std::fs::read_to_string(&note).unwrap(), "# Lise\n");
    assert!(!files::is_sealed(&note));
    assert!(!files::inside_sioul(&base.join("notes")));
    // Absent is "not found"; what stays in clear stays so.
    assert_eq!(files::read(base.join("profile/config/none.toml")).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    let sound = base.join("profile/cache/sounds/bell.wav");
    files::create_dir_all(sound.parent().unwrap()).unwrap();
    files::write(&sound, b"RIFF").unwrap();
    assert_eq!(std::fs::read(&sound).unwrap(), b"RIFF", "what Qt plays by its path stays in clear");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn folders_handles_renames_and_listings_through_paths() {
    let base = scratch("handles");
    let (_store, _mounted) = mounted(&base);
    let books = base.join("data/calendars/perso");
    assert!(!files::exists(&books));
    assert_eq!(files::read_dir(&books).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound));
    files::create_dir_all(&books).unwrap();
    assert!(files::is_dir(&books), "made in this process");
    assert_eq!(files::read_dir(&books).unwrap().count(), 0);
    {
        let mut file = File::create(books.join("1.ics")).unwrap();
        file.write_all(b"BEGIN:VCALENDAR\n").unwrap();
        file.write_all(b"END:VCALENDAR\n").unwrap();
    }
    assert_eq!(files::read_to_string(books.join("1.ics")).unwrap(), "BEGIN:VCALENDAR\nEND:VCALENDAR\n", "sealed when let go");
    {
        let mut log = OpenOptions::new().create(true).append(true).open(base.join("state/log.txt")).unwrap();
        writeln!(log, "one").unwrap();
    }
    {
        let mut log = OpenOptions::new().create(true).append(true).open(base.join("state/log.txt")).unwrap();
        writeln!(log, "two").unwrap();
        log.flush().unwrap();
    }
    assert_eq!(files::read_to_string(base.join("state/log.txt")).unwrap(), "one\ntwo\n");
    let mut file = File::open(base.join("state/log.txt")).unwrap();
    file.seek(SeekFrom::Start(4)).unwrap();
    let mut rest = String::new();
    file.read_to_string(&mut rest).unwrap();
    assert_eq!(rest, "two\n");
    assert!(file.write_all(b"x").is_err(), "opened to be read");
    assert!(File::create_new(base.join("state/log.txt")).is_err());
    let meta = files::metadata(base.join("state/log.txt")).unwrap();
    assert!(meta.is_file() && meta.len() == 8 && meta.modified().is_ok());
    // A file and a folder under other names.
    files::rename(books.join("1.ics"), books.join("2.ics")).unwrap();
    assert!(!files::exists(books.join("1.ics")) && files::exists(books.join("2.ics")));
    files::rename(&books, base.join("data/calendars/work")).unwrap();
    assert!(!files::exists(&books));
    assert_eq!(files::read_to_string(base.join("data/calendars/work/2.ics")).unwrap(), "BEGIN:VCALENDAR\nEND:VCALENDAR\n");
    let names: Vec<String> = files::read_dir(base.join("data/calendars")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(names, ["work"]);
    let top: Vec<String> = files::read_dir(base.join("data")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(top, ["calendars"], "the store's own folder is never listed");
    files::copy(base.join("data/calendars/work/2.ics"), base.join("data/calendars/work/3.ics")).unwrap();
    files::remove_file(base.join("data/calendars/work/2.ics")).unwrap();
    assert_eq!(files::remove_file(base.join("data/calendars/work/2.ics")).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    files::remove_dir_all(base.join("data/calendars")).unwrap();
    assert!(!files::exists(base.join("data/calendars/work/3.ics")) && !files::is_dir(base.join("data/calendars")));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_profile_s_plain_files_are_moved_in_and_later_plain_files_set_aside() {
    let base = scratch("move");
    let plain = |rel: &str, bytes: &[u8]| {
        let place = base.join(rel);
        std::fs::create_dir_all(place.parent().unwrap()).unwrap();
        std::fs::write(place, bytes).unwrap();
    };
    plain("config/config.toml", b"[account]\nname = \"Lise Martin\"\n");
    plain("config/.config.toml.lock", b"");
    plain("data/mail/lise/INBOX/cur/1700000000.M1P2.host:2,S", b"Subject: loyer\n\nmutuelle\n");
    plain("data/records/health.toml", b"[[day]]\n");
    plain("state/porch.toml", b"seen = 3\n");
    plain("cache/sounds/bell.wav", b"RIFF");
    plain("cache/mail-index/index.json", &vec![b'x'; 40 << 20]);
    let store = Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap();
    let moved = store.move_plain_files().unwrap();
    assert_eq!(moved.files, 5, "{moved:?}");
    assert_eq!(moved.set_aside, 0);
    let left = plain_files(&base).into_iter().filter(|f| !f.contains("/sealed/") && !f.starts_with("config/store.toml")).collect::<Vec<_>>();
    assert_eq!(left, ["cache/sounds/bell.wav", "config/.profile.lock", "config/.store-create.lock"], "only what stays in clear by rule (the configuration's old lock file, named after it, taken away)");
    let store = Arc::new(store);
    let _mounted = files::mount(store.clone());
    assert_eq!(files::read_to_string(base.join("data/mail/lise/INBOX/cur/1700000000.M1P2.host:2,S")).unwrap(), "Subject: loyer\n\nmutuelle\n");
    assert_eq!(files::read(base.join("cache/mail-index/index.json")).unwrap().len(), 40 << 20, "a large file streamed in");
    assert_eq!(store.moved().unwrap().len(), 4);
    // Run again: nothing left to move.
    assert_eq!(store.move_plain_files().unwrap(), Moved::default());
    // An older Sioul still running, another program or a backup put back by hand
    // wrote plain files beside the store: never read as the profile's, set aside at
    // the next start, never sealed over the current files (third check: D24 reverted).
    plain("config/config.toml", b"# an older Sioul's defaults\n");
    plain("state/new.toml", b"older = true\n");
    assert_eq!(files::read_to_string(base.join("config/config.toml")).unwrap(), "[account]\nname = \"Lise Martin\"\n", "never read as current");
    assert!(!files::exists(base.join("state/new.toml")));
    let moved = store.move_plain_files().unwrap();
    assert_eq!((moved.files, moved.set_aside), (0, 2), "{moved:?}");
    assert_eq!(files::read_to_string(base.join("config/config.toml")).unwrap(), "[account]\nname = \"Lise Martin\"\n", "the sealed configuration stays");
    assert!(!files::exists(base.join("state/new.toml")));
    let aside = moved.aside_folder.unwrap();
    assert_eq!(store.read_text(&LogicalPath::new(&format!("{aside}/state/new.toml")).unwrap()).unwrap().unwrap().0, "older = true\n");
    assert_eq!(store.read_text(&LogicalPath::new(&format!("{aside}/config/config.toml")).unwrap()).unwrap().unwrap().0, "# an older Sioul's defaults\n");
    assert!(!base.join("state/new.toml").exists() && !base.join("state/sealed-moving").exists(), "nothing left in clear");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_move_stopped_half_way_goes_on_and_loses_nothing() {
    let base = scratch("move-again");
    for i in 0..6 {
        let place = base.join(format!("state/reminded/{i}.toml"));
        std::fs::create_dir_all(place.parent().unwrap()).unwrap();
        std::fs::write(place, format!("n = {i}\n")).unwrap();
    }
    let store = Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap();
    // As if a move had sealed one file and stopped before its plain copy went.
    store.write(&LogicalPath::new("state/reminded/3.toml").unwrap(), b"n = 3\n", How::default()).unwrap();
    let moved = store.move_plain_files().unwrap();
    assert_eq!(moved.files, 6);
    let versions = store.versions(&LogicalPath::new("state/reminded/3.toml").unwrap()).unwrap();
    assert_eq!(versions.len(), 1, "the same content is not sealed twice: {versions:?}");
    for i in 0..6 {
        assert_eq!(store.read_text(&LogicalPath::new(&format!("state/reminded/{i}.toml")).unwrap()).unwrap().unwrap().0, format!("n = {i}\n"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_export_writes_every_file_in_clear_in_its_layout() {
    let base = scratch("export");
    let (store, _mounted) = mounted(&base.join("profile"));
    files::write(base.join("profile/config/config.toml"), "a = 1\n").unwrap();
    files::write(base.join("profile/data/calendars/perso/1.ics"), "BEGIN:VCALENDAR\n").unwrap();
    files::write(base.join("profile/data/clients/c1/notes.md"), "a client's words\n").unwrap();
    store.append_line(&LogicalPath::new("state/mcp/said.jsonl").unwrap(), "{\"n\":1}", How::default()).unwrap();
    store.append_line(&LogicalPath::new("state/mcp/said.jsonl").unwrap(), "{\"n\":2}", How::default()).unwrap();
    let out = base.join("out");
    let (written, damaged) = files::export(&store, &out, &|path| !path.as_str().starts_with("data/clients/")).unwrap();
    assert_eq!((written, damaged.len()), (3, 0));
    assert_eq!(std::fs::read_to_string(out.join("config/config.toml")).unwrap(), "a = 1\n");
    assert_eq!(std::fs::read_to_string(out.join("data/calendars/perso/1.ics")).unwrap(), "BEGIN:VCALENDAR\n");
    assert_eq!(std::fs::read_to_string(out.join("state/mcp/said.jsonl")).unwrap(), "{\"n\":1}\n{\"n\":2}\n");
    assert!(!out.join("data/clients").exists(), "left out when asked");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_notes_folder_placed_inside_sioul_s_folders_is_sealed_and_outside_stays_in_clear() {
    let base = scratch("placed");
    let (store, _mounted) = mounted(&base.join("profile"));
    // Inside: the trade-off taken. Its notes are sealed, read and written through the same calls.
    let inside = base.join("profile/data/Notes");
    assert!(files::inside_sioul(&inside), "a mounted store's folders count as Sioul's");
    files::create_dir_all(inside.join("Admin")).unwrap();
    files::write(inside.join("Admin/Loyer.md"), "# Loyer\n").unwrap();
    assert!(std::fs::metadata(inside.join("Admin/Loyer.md")).is_err(), "nothing in clear");
    assert_eq!(files::read_to_string(inside.join("Admin/Loyer.md")).unwrap(), "# Loyer\n");
    assert_eq!(store.read_text(&LogicalPath::new("data/Notes/Admin/Loyer.md").unwrap()).unwrap().unwrap().0, "# Loyer\n");
    let listed: Vec<String> = files::read_dir(&inside).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(listed, ["Admin"]);
    // Outside (the default): in clear, for Obsidian, Nextcloud Notes and the system.
    let outside = base.join("Documents/Notes");
    files::create_dir_all(outside.join("Admin")).unwrap();
    files::write(outside.join("Admin/Loyer.md"), "# Loyer\n").unwrap();
    assert_eq!(std::fs::read_to_string(outside.join("Admin/Loyer.md")).unwrap(), "# Loyer\n");
    assert!(!files::inside_sioul(&outside));
    // Moved out of Sioul's folders: in clear again.
    files::rename(&inside, base.join("Documents/Moved")).unwrap();
    assert_eq!(std::fs::read_to_string(base.join("Documents/Moved/Admin/Loyer.md")).unwrap(), "# Loyer\n");
    assert!(!files::exists(inside.join("Admin/Loyer.md")));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_file_renamed_while_still_open_lands_whole_and_its_old_name_stays_gone() {
    let base = scratch("open-rename");
    let (_store, _mounted) = mounted(&base);
    let temporary = base.join("data/time/.~2026-09.toml.1-1.tmp");
    let target = base.join("data/time/2026-09.toml");
    // Written beside, then renamed while still open: as the system allows.
    let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary).unwrap();
    file.write_all(b"[[session]]\n").unwrap();
    files::rename(&temporary, &target).unwrap();
    drop(file);
    assert_eq!(files::read_to_string(&target).unwrap(), "[[session]]\n");
    assert!(!files::exists(&temporary), "the old name does not come back when the file is let go");
    // Made and listed before it is let go.
    let mut other = File::create(base.join("data/time/2026-10.toml")).unwrap();
    other.write_all(b"x").unwrap();
    let names: Vec<String> = files::read_dir(base.join("data/time")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert!(names.contains(&"2026-10.toml".to_string()), "{names:?}");
    drop(other);
    // A lock file of a sealed folder lives at its opaque place: nothing named in clear (third check).
    let lock = base.join("state/share/exchange.lock");
    OpenOptions::new().create(true).truncate(false).write(true).open(&lock).unwrap();
    assert!(files::exists(&lock) && !std::fs::exists(&lock).unwrap(), "a lock's file is the system's, at its opaque place");
    let _ = std::fs::remove_dir_all(&base);
}

/// After the move, every calendar item the sync knows (`dav.rs`'s
/// `send_known` takes a file it does not find for one deleted here, and sends
/// `DELETE`) is found where it was: a plain one moved in, one in a calendar
/// linked from another disk, one whose name holds a `\\`. So the sync sends no
/// `DELETE` after the move (review-finish F1, F7).
#[cfg(unix)]
#[test]
fn after_the_move_a_calendar_sync_finds_every_item_it_knows() {
    let base = scratch("sync-after-move");
    let profile = base.join("profile");
    let elsewhere = base.join("other-disk/cal");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("linked.ics"), "BEGIN:VCALENDAR\n").unwrap();
    let home = profile.join("data/calendars/acct/home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("plain.ics"), "BEGIN:VCALENDAR\n").unwrap();
    std::fs::write(home.join("odd\\name.ics"), "BEGIN:VCALENDAR\n").unwrap();
    std::os::unix::fs::symlink(&elsewhere, profile.join("data/calendars/acct/work")).unwrap();
    let known = [home.join("plain.ics"), home.join("odd\\name.ics"), profile.join("data/calendars/acct/work/linked.ics")];
    let store = Arc::new(Store::create(&Folders::under(&profile), &RootKey::test(), Options::default()).unwrap());
    let _mounted = files::mount(store.clone());
    let moved = store.move_plain_files().unwrap();
    for item in &known {
        assert!(files::metadata(item).is_ok(), "{} reads as gone after the move: the sync would delete it on its server ({moved:?})", item.display());
    }
    assert!(!home.join("plain.ics").exists(), "the plain item moved in");
    let names: Vec<String> = files::read_dir(&home).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert!(names.contains(&"plain.ics".to_string()) && names.contains(&"odd\\name.ics".to_string()), "{names:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// A voice memo recorded while the notes folder is sealed waits in clear in
/// Sioul's data folder (`memos-recording/`), never in the cache, which is
/// emptied: the move leaves it, and the window seals it into the notes
/// (review-finish G8).
#[test]
fn a_memo_being_recorded_stays_in_clear_outside_the_cache() {
    assert!(files::stays_plain(Root::Data, Path::new("memos-recording/2026-11-27 18.40.ogg")));
    assert!(!files::stays_plain(Root::Data, Path::new("notes/memos/2026-11-27 18.40.ogg")));
    assert!(!files::stays_plain(Root::Cache, Path::new("memos-recording/2026-11-27 18.40.ogg")), "only the data folder's");
    let base = scratch("memo-recording");
    std::fs::create_dir_all(base.join("data/memos-recording")).unwrap();
    std::fs::write(base.join("data/memos-recording/m.ogg"), b"OggS").unwrap();
    let store = Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap();
    store.move_plain_files().unwrap();
    assert_eq!(std::fs::read(base.join("data/memos-recording/m.ogg")).unwrap(), b"OggS", "left where Qt records it");
    let _ = std::fs::remove_dir_all(&base);
}

/// The lock files of the layout before the store (`.<name>.lock` beside a
/// file, an account's `state/dav/<id>.lock`) name in clear what they lock: the
/// move takes them away once empty and held by nobody, and this version keeps
/// such locks at opaque places, nothing named on the disk (third check).
#[test]
fn locks_named_after_what_they_lock_leave_the_disk() {
    let base = scratch("old-locks");
    let plain = |rel: &str, bytes: &[u8]| {
        let place = base.join(rel);
        std::fs::create_dir_all(place.parent().unwrap()).unwrap();
        std::fs::write(place, bytes).unwrap();
    };
    plain("data/mail/acct/Archives/cur/1.M1:2,S", b"Subject: x\n\n");
    plain("data/mail/acct/Archives/.uidlist.lock", b"");
    plain("state/dav/acct-home.lock", b"");
    plain("state/remind.lock", b"");
    plain("cache/sounds/.bell.wav.lock", b"");
    let held = std::fs::OpenOptions::new().write(true).open(base.join("state/remind.lock")).unwrap();
    held.try_lock().unwrap();
    let store = Arc::new(Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap());
    let _mounted = files::mount(store.clone());
    store.move_plain_files().unwrap();
    let on_disk = |rel: &str| base.join(rel).exists();
    let gone = (!on_disk("data/mail/acct/Archives/.uidlist.lock"), !on_disk("state/dav/acct-home.lock"));
    let kept = (on_disk("state/remind.lock"), on_disk("cache/sounds/.bell.wav.lock"));
    // Taken again through the door: at its opaque place.
    let again = base.join("data/mail/acct/Archives/.uidlist.lock");
    let opened = OpenOptions::new().create(true).truncate(false).write(true).open(&again).map(|_| ());
    let named = again.exists();
    drop(held);
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(gone, (true, true), "named after a mail folder and an account: taken away");
    assert_eq!(kept, (true, true), "one held (a running watcher's), and one beside what stays in clear by rule: kept");
    assert!(opened.is_ok() && !named, "a lock taken now names nothing on the disk");
}

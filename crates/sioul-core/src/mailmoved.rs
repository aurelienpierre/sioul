// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Messages on their way to another folder (the unified mailbox's §3.5,
//! part 3): what Sioul moved without learning where the server put them.
//!
//! A move Sioul makes keeps the message here: the server's COPYUID (or
//! APPENDUID) says the new folder's UIDVALIDITY and the message's new UID,
//! and the file is renamed there at once (`sioul_sync::mailbox::follow`).
//! When the server does not say (no UIDPLUS), when the answer is lost, or
//! when the file cannot follow (a sync holds the folder too long, a sync
//! removed it first), the move is noted here: the address, its account, the
//! folder, when. Until the copy comes, finding the message answers "on its
//! way" (`mailfind::Found::OnItsWay`) instead of "not here".
//!
//! - **Where:** `$XDG_STATE_HOME/sioul/sync/moved.toml`, readable by you
//!   alone (0600), written whole beside and renamed over it, under its lock
//!   (`filelock::with_lock`): the window, `sioul` and a phone's background
//!   step may move mail at once. No older Sioul reads it.
//! - **Written** before a move's command is sent (so that a move whose answer
//!   is lost is still said), one line per message whose address is known;
//!   dropped for the messages whose file followed, or when the server refused.
//! - **Dropped** when a fetch of that folder writes a file of that address
//!   (`arrived`), when the address or its folder is forgotten, and after
//!   [`KEPT_FOR`] (ignored when read, dropped at the next write): long enough
//!   for a phone whose window opens once a day (its background step fetches
//!   the inboxes only), short enough that a message moved again elsewhere is
//!   not said on its way for long. Readers write nothing.

use crate::config::state_dir;
use crate::mailfind::Address;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How long a note is believed: three days.
pub const KEPT_FOR: i64 = 3 * 24 * 3600;

/// One message on its way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moved {
    /// The address (account id) it was moved within, or into.
    pub account: String,
    /// `mid:…` or `sioul:mail/…`, as links write it.
    pub address: String,
    /// The folder it went to, as its server names it.
    pub folder: String,
    /// When (Unix seconds).
    pub at: i64,
}

impl Moved {
    fn current(&self, now: i64) -> bool {
        self.at <= now + 60 && now - self.at < KEPT_FOR
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Notes {
    #[serde(default, rename = "moved")]
    moved: Vec<Moved>,
}

/// The file, under this device's state.
pub fn path() -> PathBuf {
    state_dir().join("sync").join("moved.toml")
}

/// An address as links write it, whatever its spelling; none for what is not one.
fn canonical(address: &str) -> Option<String> {
    Address::parse(address).map(|a| a.uri())
}

fn load(path: &Path) -> Notes {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// Written beside, readable by you alone, then renamed over it; none left, the file goes.
fn save(path: &Path, notes: &Notes) -> std::io::Result<()> {
    if notes.moved.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string(notes).map_err(std::io::Error::other)?;
    let beside = path.with_extension("toml.new");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    {
        use std::io::Write;
        let mut file = options.open(&beside)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&beside, path)
}

/// The notes changed by `change` under the file's lock; those past their time dropped too.
fn change(path: &Path, now: i64, change: impl FnOnce(&mut Vec<Moved>)) -> std::io::Result<()> {
    crate::filelock::with_lock(path, || {
        let mut notes = load(path);
        let before = notes.moved.clone();
        notes.moved.retain(|m| m.current(now));
        change(&mut notes.moved);
        if notes.moved == before { Ok(()) } else { save(path, &notes) }
    })
}

/// Notes `addresses` of `account` as on their way into `folder` from `at`,
/// each in place of an earlier note of it.
pub fn note(account: &str, addresses: &[String], folder: &str, at: i64) -> std::io::Result<()> {
    note_in(&path(), account, addresses, folder, at)
}

pub fn note_in(path: &Path, account: &str, addresses: &[String], folder: &str, at: i64) -> std::io::Result<()> {
    let mut addresses: Vec<String> = addresses.iter().filter_map(|a| canonical(a)).collect();
    addresses.sort();
    addresses.dedup();
    if addresses.is_empty() {
        return Ok(());
    }
    change(path, at, |moved| {
        moved.retain(|m| !addresses.contains(&m.address));
        moved.extend(addresses.iter().map(|address| Moved { account: account.to_string(), address: address.clone(), folder: folder.to_string(), at }));
    })
}

/// Drops the notes of `addresses` (their file followed, or the server refused the move).
pub fn forget(addresses: &[String]) -> std::io::Result<()> {
    forget_in(&path(), addresses, now())
}

pub fn forget_in(path: &Path, addresses: &[String], now: i64) -> std::io::Result<()> {
    let addresses: Vec<String> = addresses.iter().filter_map(|a| canonical(a)).collect();
    if addresses.is_empty() || !path.exists() {
        return Ok(());
    }
    change(path, now, |moved| moved.retain(|m| !addresses.contains(&m.address)))
}

/// Whether notes wait for `account`'s `folder`: a fetch of it looks at the
/// addresses of what it wrote only then.
pub fn waiting_for(account: &str, folder: &str) -> bool {
    waiting_for_in(&path(), account, folder, now())
}

pub fn waiting_for_in(path: &Path, account: &str, folder: &str, now: i64) -> bool {
    load(path).moved.iter().any(|m| m.account == account && m.folder == folder && m.current(now))
}

/// The copies a fetch of `account`'s `folder` wrote (their addresses): their notes go.
pub fn arrived(account: &str, folder: &str, addresses: &[String]) -> std::io::Result<()> {
    arrived_in(&path(), account, folder, addresses, now())
}

pub fn arrived_in(path: &Path, account: &str, folder: &str, addresses: &[String], now: i64) -> std::io::Result<()> {
    let addresses: Vec<String> = addresses.iter().filter_map(|a| canonical(a)).collect();
    if addresses.is_empty() || !path.exists() {
        return Ok(());
    }
    change(path, now, |moved| moved.retain(|m| !(m.account == account && m.folder == folder && addresses.contains(&m.address))))
}

/// An address forgotten here (`maildir::forget_account`), or one of its
/// folders (`folder`: kept on the server only now): their notes go.
pub fn forget_account(account: &str, folder: Option<&str>) -> std::io::Result<()> {
    let path = path();
    if !path.exists() {
        return Ok(());
    }
    change(&path, now(), |moved| moved.retain(|m| !(m.account == account && folder.is_none_or(|f| m.folder == f))))
}

/// The newest note of `address` still believed: where it is on its way to.
pub fn of(address: &str) -> Option<Moved> {
    of_in(&path(), address, now())
}

pub fn of_in(path: &Path, address: &str, now: i64) -> Option<Moved> {
    let address = canonical(address)?;
    load(path).moved.into_iter().filter(|m| m.address == address && m.current(now)).max_by_key(|m| m.at)
}

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stand_in() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-mailmoved-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("sync").join("moved.toml")
    }

    /// A move noted, read by its address however spelled, replaced by a later
    /// one, dropped when its copy arrives in that folder, and believed three
    /// days; the file yours alone, gone with its last note.
    #[test]
    fn a_move_is_on_its_way_until_its_copy_comes() {
        let path = stand_in();
        let at = 1_793_000_000;
        note_in(&path, "work", &["mid:<plan@example.org>".into(), "not an address/at all".into()], "Archive", at).unwrap();
        let read = of_in(&path, "mid:plan%40example.org", at + 5).unwrap();
        assert_eq!((read.account.as_str(), read.address.as_str(), read.folder.as_str()), ("work", "mid:plan@example.org", "Archive"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        assert!(waiting_for_in(&path, "work", "Archive", at + 5) && !waiting_for_in(&path, "work", "Trash", at + 5) && !waiting_for_in(&path, "home", "Archive", at + 5));
        // Moved again: the later note in its place.
        note_in(&path, "work", &["mid:plan@example.org".into()], "Trash", at + 10).unwrap();
        assert_eq!(of_in(&path, "<plan@example.org>", at + 11).unwrap().folder, "Trash");
        assert_eq!(load(&path).moved.len(), 1);
        // A copy of another address, or in another folder, leaves it; its own drops it.
        arrived_in(&path, "work", "Trash", &["mid:other@example.org".into()], at + 20).unwrap();
        arrived_in(&path, "work", "Archive", &["mid:plan@example.org".into()], at + 20).unwrap();
        assert!(of_in(&path, "mid:plan@example.org", at + 20).is_some());
        arrived_in(&path, "work", "Trash", &["mid:plan@example.org".into()], at + 20).unwrap();
        assert_eq!(of_in(&path, "mid:plan@example.org", at + 20), None);
        assert!(!path.exists(), "the last note gone, the file goes");
        // Three days, then not believed, and dropped at the next write.
        note_in(&path, "work", &["sioul:mail/work/0123456789abcdef".into()], "Archive", at).unwrap();
        assert!(of_in(&path, "sioul:mail/work/0123456789ABCDEF", at + KEPT_FOR - 1).is_some());
        assert_eq!(of_in(&path, "sioul:mail/work/0123456789abcdef", at + KEPT_FOR), None);
        note_in(&path, "work", &["mid:later@example.org".into()], "Archive", at + KEPT_FOR).unwrap();
        assert_eq!(load(&path).moved.iter().map(|m| m.address.as_str()).collect::<Vec<_>>(), ["mid:later@example.org"]);
        // Its file followed: forgotten.
        forget_in(&path, &["mid:later@example.org".into()], at + KEPT_FOR).unwrap();
        assert!(!path.exists());
    }
}

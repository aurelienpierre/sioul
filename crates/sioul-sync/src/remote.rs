// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The other devices' files fetched from the server too (docs/database.md,
//! "Fetched from the server too"). The sharing folder stays the one a sync app
//! carries, whichever it is; when that folder lives on a WebDAV server Sioul
//! has an account for (Nextcloud, Murena, ownCloud), Sioul also reads it there,
//! in case the sync app is late bringing the others' files, or stops: Murena's
//! eDrive was seen bringing a computer's records an hour and a half late, and
//! its entry among the devices never.
//!
//! - **Found** on the servers of the contacts-and-calendars accounts that keep
//!   files Nextcloud's way (`…/remote.php/dav/files/<user>/`, the user as the
//!   server names it): the place given by hand, else the folder's path from a
//!   phone's storage (eDrive carries it as the cloud's root), from a folder the
//!   Nextcloud client carries (`localPath` → `targetPath`), or its last names.
//! - **Confirmed** only where the server's `seal.toml` is this folder's, byte
//!   for byte: a folder sealed otherwise is never read. Read again when it
//!   changes there, and every six hours.
//! - **Fetched** when the window or the phone's background step asks
//!   (`pull`): `PROPFIND` (`Depth: 1`) on the folder, `devices/`, `leases/` and
//!   each part's claims, a folder whose ETag did not change skipped for ten
//!   minutes; of the other devices' files, only those newer than both the
//!   synced folder's copy and what was fetched before come down, into this
//!   device's own state (`$XDG_STATE_HOME/sioul/share/remote/`, their paths as
//!   in the folder): records from where the copy here ends (its last line
//!   asked again and checked: a copy that does not join comes whole), only
//!   whole lines kept; the rest whole, a few hundred bytes each, kept only when
//!   it came whole. Ten seconds for each wait of the network, thirty for a pull.
//! - **Read** (`overlay`): the sharing reads, for each other device's file, the
//!   newer of the two copies: records the longer (they only grow), notes to
//!   each other, entries and claims by the times they hold. The folder's copy
//!   wins a tie. What was fetched goes once the folder's copy caught up.
//! - **Sent too** (`send`, docs/database.md, "Sent to the server too"): this
//!   device's own files, beside the sync app, each over what is there as it
//!   was seen; found there the same (the sync app sent it), not sent twice;
//!   records never shorter; never another device's file.
//! - **Never**: another device's file written, moved or deleted on the
//!   server; anything written into the synced folder; this device's own files
//!   fetched or read from there; plain HTTP (a test's own stand-in apart); the
//!   password kept or said (it comes from the keyring, as the calendars' sync
//!   has it, for the requests only).

use crate::SyncError;
use crate::dav::{self, DAV};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// How long the network may take: each wait (to connect, for the next
/// bytes); a listing or a small file in all; a records' file in all, as long
/// as it moves; a whole pull.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) wait: Duration,
    pub(crate) small: Duration,
    pub(crate) large: Duration,
    pub(crate) pull: Duration,
    /// A sealed note or paper, as long as it moves.
    pub(crate) blob: Duration,
    /// A file sent (`Server::put_file`): given `large`, and as long more as
    /// it takes at `floor` bytes a second; cut when nothing moved for `stall`.
    pub(crate) floor: u64,
    pub(crate) stall: Duration,
}

pub(crate) const LIMITS: Limits = Limits { wait: Duration::from_secs(10), small: Duration::from_secs(10), large: Duration::from_secs(60), pull: Duration::from_secs(30), blob: Duration::from_secs(15 * 60), floor: FLOOR, stall: Duration::from_secs(30) };

/// The slowest upload Sioul waits for (bytes a second): a phone's mobile
/// data on 3G sends faster. A 2.5 MB file is given 60 s and 156 s more.
pub const FLOOR: u64 = 16 << 10;

/// Sending this device's files (`send`): a step that sends a large file on a
/// slow line is not cut after thirty seconds, as a pull is; each file is
/// given the time its size asks (`FLOOR`).
pub(crate) const SEND_LIMITS: Limits = Limits { wait: Duration::from_secs(20), small: Duration::from_secs(30), large: Duration::from_secs(60), pull: Duration::from_secs(20 * 60), blob: Duration::from_secs(15 * 60), floor: FLOOR, stall: Duration::from_secs(30) };

/// Looked for again this long after the last look, when not found (seconds).
pub const LOOK_AGAIN: i64 = 6 * 3600;
/// The server's seal read again this long after the last time, even unchanged (seconds).
const SEAL_AGAIN: i64 = 6 * 3600;
/// A folder whose ETag did not change is looked through again after this long all the same (seconds).
const LIST_AGAIN: i64 = 10 * 60;
/// A records' file is never fetched past this.
const LARGEST: u64 = 64 << 20;
/// A device's texts to send (`texts/send/<id>.jsonl`) are never fetched past this.
const TEXTS_LARGEST: u64 = 4 << 20;
/// A listing is never read past this.
const LISTING: u64 = 4 << 20;
/// The last line asked again, to check the copy here joins what is there, when shorter than this.
const OVERLAP: u64 = 1 << 20;

const XML: &str = "application/xml; charset=utf-8";
const LIST: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns"><d:prop><d:resourcetype/><d:getetag/><d:getcontentlength/><oc:checksums/></d:prop></d:propfind>"#;
/// Nextcloud's own properties: the checksums a file was sent with (`oc:checksums`).
const OC: &str = "http://owncloud.org/ns";
const PRINCIPAL: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>"#;

// ---------------------------------------------------------------- where

/// Where the backup keeps what it fetched, beside the sharing's memory: `<state>/share/remote/`.
pub fn cache_of(memory: &Path) -> PathBuf {
    memory.with_file_name("remote")
}

/// What the backup knows, beside it: `<state>/share/remote.toml`.
fn state_path(memory: &Path) -> PathBuf {
    memory.with_file_name("remote.toml")
}

fn shown(folder: &Path) -> String {
    folder.display().to_string()
}

/// A server's name, to say where: "murena.io".
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    authority.rsplit('@').next().unwrap_or("").to_string()
}

// ---------------------------------------------------------------- the state

/// What the backup knows, kept privately beside the sharing's memory
/// (`remote.toml`), never shared. `on`, `given` and `asked` are yours,
/// written only through `State::choose`; the rest is the backup's.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Switched on or off here; unsaid, on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<bool>,
    /// The folder's place on the server, given by hand: a path in an
    /// account's files ("Documents/Sioul"), or an address; "" to look by itself.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub given: String,
    /// When you asked to look again (Unix seconds).
    #[serde(default)]
    pub asked: i64,
    /// This device's own files sent there too, beside the sync app (`send`):
    /// as switched here, else on wherever the backup is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send: Option<bool>,
    /// The sharing folder here it backs.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    /// The account whose server holds it, by its id, and the folder there (an address ending with "/").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub account: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// Since when the server's seal is found to be this folder's (Unix seconds); 0: not confirmed.
    #[serde(default)]
    pub confirmed: i64,
    /// When the server's seal was last read and found the same (Unix seconds).
    #[serde(default)]
    pub sealed: i64,
    /// When the folder was last looked for (Unix seconds).
    #[serde(default)]
    pub looked: i64,
    /// When a pull last went through, and was last tried (Unix seconds).
    #[serde(default)]
    pub last: i64,
    #[serde(default)]
    pub tried: i64,
    /// Why nothing is fetched, or the last pull failed, in a code
    /// ("not-found:murena.io", "network:murena.io"); "" when all is well.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub said: String,
    /// The other devices' records on the server past what a pull brings
    /// (`LARGEST`), longer there than here, by name, as the last look through
    /// the folder found them: only the sync app brings them, and the pull
    /// alone is not trusted meanwhile (`pulls_well`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub left: BTreeMap<String, u64>,
    /// This device's entry on the server (`devices/<id>.device`) as a pull
    /// last listed it: its size, and when (Unix seconds). Of another size than
    /// this device last sent, listed after that send, it is an older copy put
    /// back there: looked at again by the next send, and sent over it (`send`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_entry: Option<(u64, i64)>,
    /// `MIRROR` when Sioul keeps the folder in step itself, no sync app: the
    /// folder here is its own copy (`mirror_of`), the server's is the one the
    /// devices share; "" beside a sync app's folder.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,
    /// The folder's place in the account's files, as given ("Documents/Sioul"): to say where.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub place: String,
    /// When this device's own files last went up (`MIRROR`, Unix seconds).
    #[serde(default)]
    pub pushed: i64,
    /// The server's files as last fetched, or found here already, by their path in the folder.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, Fetched>,
    /// The server's folders as last looked through, by their path ("" the folder itself, "devices/").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub folders: BTreeMap<String, Looked>,
    /// This device's own files as they last went up (`MIRROR`), by their path in the folder.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sent: BTreeMap<String, Sent>,
}

/// A file of the server, as last fetched: its ETag and size there.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fetched {
    #[serde(default)]
    pub etag: String,
    #[serde(default)]
    pub size: u64,
}

/// A folder of the server, as last looked through: its ETag, and when (Unix seconds).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Looked {
    #[serde(default)]
    pub etag: String,
    #[serde(default)]
    pub at: i64,
}

/// One of this device's own files as it last went up (`MIRROR`): its size and
/// time here (nanoseconds), its ETag there ("" when the server did not say).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub modified: u64,
    #[serde(default)]
    pub etag: String,
}

impl State {
    pub fn load(memory: &Path) -> State {
        std::fs::read_to_string(state_path(memory)).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    fn write(&self, memory: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::share::write_atomically(&state_path(memory), format!("# Sioul: the sharing folder fetched from its server too (docs/database.md). Never shared.\n{text}").as_bytes())
    }

    /// Written as the backup's work left it, under its lock: what you chose
    /// meanwhile (`on`, `given`, `asked`) kept as the file has it.
    fn save(&self, memory: &Path) -> Result<(), String> {
        let path = state_path(memory);
        sioul_core::filelock::with_lock(&path, || {
            let there = State::load(memory);
            State { on: there.on, given: there.given, asked: there.asked, send: there.send, ..self.clone() }.write(memory)
        })
    }

    /// What a pull found, written over the state as it is now, under its lock:
    /// two processes pull (the window's or an alarm's, and the background
    /// step's), and neither undoes the other. The pull begun later says when
    /// a pull was tried and how it went; the latest that went through is kept;
    /// what was fetched and looked through, each file's and folder's latest;
    /// what is left to the sync app (`left`): as it is now, with what this
    /// pull found left added (new, or at another size) and what it found
    /// brought taken out, from what it knew when it began (`loaded_left`):
    /// another pull's finding meanwhile is
    /// never erased, only a look that finds it brought clears it; the rest
    /// (your choices, the place) as it is now. A place changed meanwhile
    /// (found elsewhere): what this pull found is not that place's, left out.
    /// `ended`: this pull's last moment by the clock it began by (Unix
    /// seconds); one said begun after that began after this pull ended,
    /// which no pull of the other process can: the clock was set back since,
    /// and this pull is the later.
    fn save_pulled(&self, memory: &Path, ended: i64, loaded_left: &BTreeMap<String, u64>) -> Result<(), String> {
        let path = state_path(memory);
        sioul_core::filelock::with_lock(&path, || {
            let mut merged = State::load(memory);
            if (merged.url.as_str(), merged.folder.as_str(), merged.account.as_str()) != (self.url.as_str(), self.folder.as_str(), self.account.as_str()) && !merged.url.is_empty() {
                return Ok(());
            }
            let ours_later = self.tried >= merged.tried || merged.tried > ended;
            let (later, earlier) = if ours_later { (self.clone(), merged.clone()) } else { (merged.clone(), self.clone()) };
            merged.url = later.url.clone();
            merged.folder = later.folder.clone();
            merged.account = later.account.clone();
            merged.tried = later.tried;
            // A time later than this pull's end dates from before the clock was set back: not a later pull.
            merged.last = [later.last, earlier.last].into_iter().filter(|last| *last <= ended).max().unwrap_or(self.last);
            merged.said = later.said.clone();
            merged.confirmed = later.confirmed;
            merged.sealed = later.sealed.max(earlier.sealed);
            merged.files = earlier.files.into_iter().chain(later.files).collect();
            merged.folders = earlier.folders.into_iter().chain(later.folders).collect();
            let brought: Vec<&String> = loaded_left.keys().filter(|name| !self.left.contains_key(*name)).collect();
            merged.left.retain(|name, _| !brought.contains(&name));
            // Found by this pull: new, or found again at another size (grown there since).
            merged.left.extend(self.left.iter().filter(|(name, size)| loaded_left.get(*name) != Some(*size)).map(|(name, size)| (name.clone(), *size)));
            merged.own_entry = [earlier.own_entry, later.own_entry].into_iter().flatten().max_by_key(|(_, at)| *at);
            merged.write(memory)
        })
    }

    /// What you choose (the switch, the place given by hand), changed under its lock.
    pub fn choose(memory: &Path, change: impl FnOnce(&mut State)) -> Result<State, String> {
        let path = state_path(memory);
        sioul_core::filelock::with_lock(&path, || {
            let mut state = State::load(memory);
            change(&mut state);
            state.write(memory)?;
            Ok(state)
        })
    }

    /// Its server's seal was found to be this folder's.
    pub fn confirmed_for(&self, folder: &Path) -> bool {
        self.confirmed > 0 && !self.url.is_empty() && self.folder == shown(folder)
    }

    /// Fetching: as switched here, else on.
    pub fn fetching(&self) -> bool {
        self.on != Some(false)
    }

    /// Pulling from the server works for `folder` beside a sync app: switched
    /// on, the folder confirmed there, a pull gone through and the last one
    /// too (`said` empty). A phone's background step then reads the other
    /// devices' news from its pull alone, the sync app neither asked to look
    /// nor waited for (docs/android.md, "In the background"); otherwise, as
    /// before: the sync app asked, and given twenty seconds.
    pub fn pulls_well(&self, folder: &Path) -> bool {
        self.mode != MIRROR && self.fetching() && self.confirmed_for(folder) && self.last > 0 && self.said.is_empty() && self.left.is_empty()
    }

    /// A pull begun at or after `since` and not after `now` (Unix seconds)
    /// went through, with nothing left on the server that only the sync app
    /// brings: what a dose's or a waking's alarm trusts alone (`alarm_news`).
    /// A pull said begun after now is one from before the clock was set back:
    /// not since, pulled again.
    pub fn went_through_since(&self, folder: &Path, since: i64, now: i64) -> bool {
        self.pulls_well(folder) && (since..=now).contains(&self.last)
    }

    /// Sending this device's own files there too (`send`): with the backup
    /// on, as switched here, else on.
    pub fn sending(&self) -> bool {
        self.fetching() && self.send != Some(false)
    }

    /// The server, by its name: "murena.io".
    pub fn host(&self) -> String {
        host_of(&self.url)
    }

    /// Whether to look for the folder now: asked to (a place given by hand,
    /// the switch turned on), another folder shared here, or not confirmed
    /// and never looked for, or looked for `LOOK_AGAIN` ago.
    pub fn look_due(&self, folder: &Path, now: i64) -> bool {
        self.folder != shown(folder) || self.asked > self.looked || (!self.confirmed_for(folder) && (self.looked == 0 || now - self.looked >= LOOK_AGAIN))
    }
}

// ---------------------------------------------------------------- read here

/// The cache attached to each sharing folder in this process (`attach`), and
/// what that was decided on: the backup's state, this folder's seal, the
/// cache's (their sizes and times).
type Stamp = [Option<(u64, SystemTime)>; 3];
static ATTACHED: Mutex<BTreeMap<PathBuf, (Stamp, Option<PathBuf>)>> = Mutex::new(BTreeMap::new());

fn stamp_of(path: &Path) -> Option<(u64, SystemTime)> {
    std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
}

/// The copy fetched for the sharing folder `folder`, read beside it from now
/// on in this process (`overlay`), when there is one for it: its state names
/// this folder, and what was fetched was fetched under this folder's seal
/// (the cache's `seal.toml`, written when the server's was found the same).
/// Cheap when nothing changed; nothing of it needs the network. Returns it.
pub fn attach(folder: &Path, memory: &Path) -> Option<PathBuf> {
    let cache = cache_of(memory);
    let stamp = [stamp_of(&state_path(memory)), stamp_of(&folder.join("seal.toml")), stamp_of(&cache.join("seal.toml"))];
    let mut attached = ATTACHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((was, chosen)) = attached.get(folder)
        && *was == stamp
    {
        return chosen.clone();
    }
    let chosen = usable(folder, memory).then_some(cache);
    attached.insert(folder.to_path_buf(), (stamp, chosen.clone()));
    chosen
}

/// The copy fetched for `folder`, when one is attached in this process: the
/// sharing reads, for each other device's file, the newer of the two.
pub fn overlay(folder: &Path) -> Option<PathBuf> {
    ATTACHED.lock().ok()?.get(folder).and_then(|(_, chosen)| chosen.clone())
}

/// The seal of a folder, its bytes: at most a small file's.
fn seal_of(folder: &Path) -> Option<Vec<u8>> {
    let path = folder.join("seal.toml");
    let size = std::fs::metadata(&path).ok()?.len();
    (size <= crate::share::SMALL_FILE).then(|| std::fs::read(&path).ok()).flatten()
}

/// What was fetched for this folder may be read: fetched for it, under its seal.
fn usable(folder: &Path, memory: &Path) -> bool {
    let state = State::load(memory);
    state.mode != MIRROR && state.folder == shown(folder) && seal_of(folder).is_some_and(|here| seal_of(&cache_of(memory)).is_some_and(|fetched| fetched == here))
}

/// Sharing stopped here: what was fetched and known goes, and nothing is read beside the folder.
pub fn forget(memory: &Path, folder: &Path) {
    let _ = std::fs::remove_dir_all(cache_of(memory));
    let _ = std::fs::remove_file(state_path(memory));
    let _ = std::fs::remove_file(crate::blobs::Wanted::path(memory));
    let _ = std::fs::remove_file(Lane::Main.memory(memory));
    let _ = std::fs::remove_file(Lane::Urgent.memory(memory));
    if let Ok(mut attached) = ATTACHED.lock() {
        attached.remove(folder);
    }
}

// ---------------------------------------------------------------- the server

/// An account to fetch with: its id in the configuration, where its server
/// was found (`Account::url`), its login, and its password from the keyring
/// (none kept there): used for the requests only, never kept nor said.
pub struct Login {
    pub account: String,
    pub url: String,
    pub user: String,
    pub password: Option<String>,
}

/// One server, signed in: requests with the account's login, over TLS (plain
/// HTTP only to a test's own stand-in, `dav::allowed`), never redirected. Its
/// requests go through the process's one agent (`dav::shared_agent`): a pull,
/// the sends after it and the next pull reuse one connection while it is
/// open, each request with its own budget (`limits`).
pub(crate) struct Server {
    agent: ureq::Agent,
    authorization: String,
    started: Instant,
    limits: Limits,
}

/// A folder of the server, as listed: its own ETag, and what it holds by name.
#[derive(Debug, Default)]
pub(crate) struct Listing {
    pub(crate) etag: String,
    pub(crate) items: BTreeMap<String, Item>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Item {
    pub(crate) dir: bool,
    pub(crate) etag: String,
    pub(crate) size: u64,
    /// The checksums the server keeps of it, as it says them ("SHA1:… MD5:…"); "" when none.
    pub(crate) checksum: String,
}

/// What came of a file asked from a place in it: where it starts, its bytes,
/// whether all came, and why not.
pub(crate) struct Got {
    start: u64,
    bytes: Vec<u8>,
    error: Option<SyncError>,
}

/// A file that did not go (`Server::put_file`): the error, what kind of
/// failure ("timeout", "stalled", "unreachable", "tls", "login", "network"),
/// how long the try lasted (seconds), how many of its bytes went, of how many.
#[derive(Debug)]
pub(crate) struct PutFailed {
    pub(crate) error: SyncError,
    pub(crate) kind: &'static str,
    pub(crate) words: String,
    pub(crate) seconds: u64,
    pub(crate) sent: u64,
    pub(crate) total: u64,
}

/// A body read from memory, counting what went: what a failure says it sent.
struct Counted {
    inner: std::io::Cursor<Vec<u8>>,
    moved: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl Read for Counted {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.moved.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
        Ok(n)
    }
}

fn failed(e: &ureq::Error) -> SyncError {
    match e {
        ureq::Error::Tls(_) => SyncError::Tls(e.to_string()),
        _ if format!("{e:?}").starts_with("Rustls") => SyncError::Tls(e.to_string()),
        _ => SyncError::Network(e.to_string()),
    }
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
}

impl Server {
    pub(crate) fn new(login: &Login, limits: Limits) -> Result<Server, SyncError> {
        let password = login.password.as_deref().ok_or(SyncError::NoPassword)?;
        Ok(Server {
            agent: dav::shared_agent(),
            authorization: format!("Basic {}", sioul_core::lines::base64_encode(format!("{}:{password}", login.user).as_bytes())),
            started: Instant::now(),
            limits,
        })
    }

    pub(crate) fn request(&self, large: bool, method: &str, url: &str, headers: &[(&str, &str)], body: Vec<u8>) -> Result<ureq::http::Response<ureq::Body>, SyncError> {
        self.request_with(if large { self.limits.large } else { self.limits.small }, method, url, headers, body)
    }

    /// A request run with `whole` for all of it, `limits.wait` to connect and
    /// for each read or write (`stalls::ask`, its answer's body read after
    /// included).
    fn request_with(&self, whole: Duration, method: &str, url: &str, headers: &[(&str, &str)], body: Vec<u8>) -> Result<ureq::http::Response<ureq::Body>, SyncError> {
        dav::allowed(url)?;
        if self.started.elapsed() > self.limits.pull {
            return Err(SyncError::Network("out of time".into()));
        }
        let mut request = ureq::http::Request::builder().method(method).uri(url).header("Authorization", &self.authorization).header("User-Agent", "Sioul");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let request = request.body(body).map_err(|e| SyncError::Server(e.to_string()))?;
        let request = self.agent.configure_request(request).timeout_connect(Some(self.limits.wait)).timeout_global(Some(whole)).build();
        crate::stalls::ask(self.limits.wait, false);
        let response = self.agent.run(request).map_err(|e| failed(&e))?;
        if response.status().as_u16() == 401 {
            return Err(SyncError::Login(format!("{method}: 401")));
        }
        Ok(response)
    }

    /// A folder's own ETag and what it holds (`Depth: 1`), by name.
    pub(crate) fn list(&self, url: &str) -> Result<Listing, SyncError> {
        let mut response = self.request(false, "PROPFIND", url, &[("Depth", "1"), ("Content-Type", XML)], LIST.as_bytes().to_vec())?;
        match response.status().as_u16() {
            207 => {}
            404 => return Err(SyncError::NotFound(String::new())),
            status => return Err(SyncError::Server(format!("PROPFIND: {status}"))),
        }
        let body = response.body_mut().with_config().limit(LISTING).read_to_string().map_err(|e| failed(&e))?;
        let (responses, _) = dav::multistatus(&body)?;
        let sums = checksums(&body, url);
        let me = dav::path_key(url);
        let mut listing = Listing::default();
        for response in responses {
            let path = dav::path_key(&dav::absolute(url, &response.href));
            let etag = response.text(DAV, "getetag").unwrap_or_default();
            if path == me {
                listing.etag = etag;
                continue;
            }
            let Some(name) = path.strip_prefix(&me).and_then(|rest| rest.strip_prefix('/')).filter(|n| !n.is_empty() && !n.contains('/')) else { continue };
            let size = response.text(DAV, "getcontentlength").and_then(|t| t.parse().ok()).unwrap_or(0);
            listing.items.insert(name.to_string(), Item { dir: response.is(DAV, "collection"), etag, size, checksum: sums.get(&path).cloned().unwrap_or_default() });
        }
        Ok(listing)
    }

    /// One file as the server has it now (`Depth: 0`): its ETag, size and
    /// checksums; none when it is not there.
    pub(crate) fn stat(&self, url: &str) -> Result<Option<Item>, SyncError> {
        let mut response = self.request(false, "PROPFIND", url, &[("Depth", "0"), ("Content-Type", XML)], LIST.as_bytes().to_vec())?;
        match response.status().as_u16() {
            207 => {}
            404 => return Ok(None),
            status => return Err(SyncError::Server(format!("PROPFIND: {status}"))),
        }
        let body = response.body_mut().with_config().limit(LISTING).read_to_string().map_err(|e| failed(&e))?;
        let (responses, _) = dav::multistatus(&body)?;
        let sums = checksums(&body, url);
        Ok(responses.first().map(|r| Item {
            dir: r.is(DAV, "collection"),
            etag: r.text(DAV, "getetag").unwrap_or_default(),
            size: r.text(DAV, "getcontentlength").and_then(|t| t.parse().ok()).unwrap_or(0),
            checksum: sums.get(&dav::path_key(&dav::absolute(url, &r.href))).cloned().unwrap_or_default(),
        }))
    }

    /// One of this device's files sent whole beside a sync app (`send`): over
    /// what is there as it was seen (`If-Match` its ETag, `If-None-Match: *`
    /// when none was there), dated as the file here (`X-OC-Mtime`), with its
    /// SHA-1 (`OC-Checksum`): a Nextcloud client finding the same file there
    /// as here, same size, date and checksum, takes it as its own, without a
    /// download and without a conflicted copy. Its status, and its ETag there.
    ///
    /// Given the time its size asks: `limits.large`, and as long more as it
    /// takes at `limits.floor` bytes a second (a file still moving is not cut
    /// at a flat minute); cut when nothing moved for `limits.stall`. Failed:
    /// why, how long it lasted and how many of its bytes went (`PutFailed`).
    pub(crate) fn put_file(&self, url: &str, body: Vec<u8>, seen: Option<&str>, modified_s: u64, sha1: &str) -> Result<(u16, Option<String>), PutFailed> {
        let total = body.len() as u64;
        let started = Instant::now();
        let gave_up = |error: SyncError, kind: &'static str, sent: u64| PutFailed { words: error.to_string(), error, kind, seconds: started.elapsed().as_millis().div_ceil(1000) as u64, sent, total };
        dav::allowed(url).map_err(|e| gave_up(e, "refused", 0))?;
        if self.started.elapsed() > self.limits.pull {
            return Err(gave_up(SyncError::Network("out of time".into()), "out-of-time", 0));
        }
        let precondition = match seen.filter(|etag| !etag.is_empty()) {
            Some(etag) => ("If-Match", etag),
            None => ("If-None-Match", "*"),
        };
        let whole = self.limits.large + Duration::from_secs(total / self.limits.floor.max(1));
        let moved = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let reader = Counted { inner: std::io::Cursor::new(body), moved: std::sync::Arc::clone(&moved) };
        let request = ureq::http::Request::builder()
            .method("PUT")
            .uri(url)
            .header("Authorization", &self.authorization)
            .header("User-Agent", "Sioul")
            .header(precondition.0, precondition.1)
            .header("Content-Type", "application/octet-stream")
            .header("Content-Length", total.to_string())
            .header("X-OC-Mtime", modified_s.to_string())
            .header("OC-Checksum", format!("SHA1:{sha1}"))
            .body(ureq::SendBody::from_owned_reader(reader))
            .map_err(|e| gave_up(SyncError::Server(e.to_string()), "server", 0))?;
        // Given `whole`, each write `limits.stall` at most; the answer awaited
        // within `whole` alone while the last bytes drain from the system's
        // buffers on a slow uplink.
        let request = self.agent.configure_request(request).timeout_connect(Some(self.limits.wait)).timeout_global(Some(whole)).build();
        crate::stalls::ask(self.limits.stall, true);
        match self.agent.run(request) {
            Ok(response) if response.status().as_u16() == 401 => Err(gave_up(SyncError::Login("PUT: 401".into()), "login", moved.load(std::sync::atomic::Ordering::Relaxed))),
            Ok(response) => Ok((response.status().as_u16(), header(&response, "ETag").or_else(|| header(&response, "OC-ETag")))),
            Err(e) => {
                let sent = moved.load(std::sync::atomic::Ordering::Relaxed);
                let kind = match &e {
                    ureq::Error::Timeout(ureq::Timeout::Connect | ureq::Timeout::Resolve) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => "unreachable",
                    // Nothing moved for `stall`, well before the time given.
                    ureq::Error::Timeout(_) | ureq::Error::Io(_) if started.elapsed() + Duration::from_secs(2) < whole && sent > 0 => "stalled",
                    ureq::Error::Timeout(_) => "timeout",
                    ureq::Error::Tls(_) => "tls",
                    _ if format!("{e:?}").starts_with("Rustls") => "tls",
                    _ => "network",
                };
                Err(gave_up(failed(&e), kind, sent))
            }
        }
    }

    /// A small file whole (`share::SMALL_FILE` at most) and its ETag; none
    /// when it is not there, or larger than any such file. A body that came
    /// cut, or whose end cannot be told, is an error: never taken as whole.
    pub(crate) fn small(&self, url: &str) -> Result<Option<(Vec<u8>, Option<String>)>, SyncError> {
        self.small_up_to(url, crate::share::SMALL_FILE)
    }

    /// The same, at most `cap` bytes (a texts' file may grow past a small file's).
    pub(crate) fn small_up_to(&self, url: &str, cap: u64) -> Result<Option<(Vec<u8>, Option<String>)>, SyncError> {
        let mut response = self.request(false, "GET", url, &[("Accept-Encoding", "identity")], Vec::new())?;
        match response.status().as_u16() {
            200 => {}
            404 | 410 => return Ok(None),
            status => return Err(SyncError::Server(format!("GET: {status}"))),
        }
        let etag = header(&response, "ETag");
        let length = response.body().content_length();
        let chunked = header(&response, "Transfer-Encoding").is_some_and(|t| t.to_ascii_lowercase().contains("chunked"));
        if length.is_some_and(|l| l > cap) {
            return Ok(None);
        }
        let bytes = match response.body_mut().with_config().limit(cap).read_to_vec() {
            Ok(bytes) => bytes,
            Err(ureq::Error::BodyExceedsLimit(_)) => return Ok(None),
            Err(e) => return Err(failed(&e)),
        };
        if !(length == Some(bytes.len() as u64) || (length.is_none() && chunked)) {
            return Err(SyncError::Network("GET: cut".into()));
        }
        Ok(Some((bytes, etag)))
    }

    /// A file from `from` on (all of it from 0): where what came starts (0
    /// when the server sent it all), what came as it came, and why the rest
    /// did not; none when it is not there, or shorter than `from` now.
    fn tail(&self, url: &str, from: u64) -> Result<Option<Got>, SyncError> {
        let range = format!("bytes={from}-");
        let mut headers = vec![("Accept-Encoding", "identity")];
        if from > 0 {
            headers.push(("Range", range.as_str()));
        }
        let mut response = self.request(true, "GET", url, &headers, Vec::new())?;
        let start = match response.status().as_u16() {
            200 => 0,
            206 => {
                // "bytes 100-199/200".
                let said = header(&response, "Content-Range").unwrap_or_default();
                said.trim().strip_prefix("bytes").and_then(|r| r.trim().split('-').next()).and_then(|s| s.trim().parse().ok()).ok_or_else(|| SyncError::Server(format!("GET: Content-Range {said}")))?
            }
            404 | 410 | 416 => return Ok(None),
            status => return Err(SyncError::Server(format!("GET: {status}"))),
        };
        let length = response.body().content_length();
        let mut reader = response.body_mut().with_config().limit(LARGEST).reader();
        let mut bytes = Vec::new();
        let mut piece = vec![0u8; 64 << 10];
        let mut error = loop {
            match reader.read(&mut piece) {
                Ok(0) => break None,
                Ok(n) => bytes.extend_from_slice(&piece[..n]),
                Err(e) => break Some(SyncError::Network(e.to_string())),
            }
        };
        if error.is_none() && length.is_some_and(|l| l != bytes.len() as u64) {
            error = Some(SyncError::Network("GET: cut".into()));
        }
        Ok(Some(Got { start, bytes, error }))
    }

    /// A file sent whole (`PUT`), over what is there as it was seen: `If-Match`
    /// its ETag, or `If-None-Match: *` when none was there. Its status, and its
    /// ETag there when the server says it.
    pub(crate) fn put(&self, url: &str, body: Vec<u8>, seen: Option<&str>) -> Result<(u16, Option<String>), SyncError> {
        let precondition = match seen {
            Some(etag) => ("If-Match", etag),
            None => ("If-None-Match", "*"),
        };
        let response = self.request(true, "PUT", url, &[precondition, ("Content-Type", "application/octet-stream")], body)?;
        Ok((response.status().as_u16(), header(&response, "ETag").or_else(|| header(&response, "OC-ETag"))))
    }

    /// A folder made (`MKCOL`): 201 made, 405 there already.
    pub(crate) fn mkcol(&self, url: &str) -> Result<u16, SyncError> {
        Ok(self.request(false, "MKCOL", url, &[], Vec::new())?.status().as_u16())
    }

    /// One of this device's files taken out (`DELETE`), only as it was seen (`If-Match`) when its ETag is known.
    pub(crate) fn delete(&self, url: &str, seen: Option<&str>) -> Result<u16, SyncError> {
        let headers: Vec<(&str, &str)> = seen.map(|etag| vec![("If-Match", etag)]).unwrap_or_default();
        Ok(self.request(false, "DELETE", url, &headers, Vec::new())?.status().as_u16())
    }

    /// A file's ETag there now (`Depth: 0`); none when it is not there.
    pub(crate) fn etag(&self, url: &str) -> Result<Option<String>, SyncError> {
        let mut response = self.request(false, "PROPFIND", url, &[("Depth", "0"), ("Content-Type", XML)], LIST.as_bytes().to_vec())?;
        match response.status().as_u16() {
            207 => {}
            404 => return Ok(None),
            status => return Err(SyncError::Server(format!("PROPFIND: {status}"))),
        }
        let body = response.body_mut().with_config().limit(LISTING).read_to_string().map_err(|e| failed(&e))?;
        let (responses, _) = dav::multistatus(&body)?;
        Ok(responses.first().map(|r| r.text(DAV, "getetag").unwrap_or_default()))
    }

    /// A file fetched whole into `path`, under a hidden name beside it, then
    /// renamed once all of it came (its length said, or chunked to its end),
    /// at most `limit` bytes, as long as it moves (`Limits::blob`). Its size;
    /// none when it is not there, or larger.
    pub(crate) fn download(&self, url: &str, path: &Path, limit: u64) -> Result<Option<u64>, SyncError> {
        let mut response = self.request_with(self.limits.blob, "GET", url, &[("Accept-Encoding", "identity")], Vec::new())?;
        match response.status().as_u16() {
            200 => {}
            404 | 410 => return Ok(None),
            status => return Err(SyncError::Server(format!("GET: {status}"))),
        }
        let length = response.body().content_length();
        let chunked = header(&response, "Transfer-Encoding").is_some_and(|t| t.to_ascii_lowercase().contains("chunked"));
        if length.is_some_and(|l| l > limit) {
            return Ok(None);
        }
        if length.is_none() && !chunked {
            return Err(SyncError::Network("GET: its end cannot be told".into()));
        }
        let temporary = crate::share::temporary(path);
        let disk = |e: std::io::Error| SyncError::Disk(format!("{}: {e}", path.display()));
        let written = (|| -> Result<u64, SyncError> {
            if let Some(parent) = path.parent() {
                crate::share::private_dirs(parent).map_err(disk)?;
            }
            let mut out = crate::share::new_private(&temporary).map_err(disk)?;
            let mut reader = response.body_mut().with_config().limit(limit).reader();
            let copied = std::io::copy(&mut reader, &mut out).map_err(|e| SyncError::Network(e.to_string()))?;
            if length.is_some_and(|l| l != copied) {
                return Err(SyncError::Network("GET: cut".into()));
            }
            out.sync_all().map_err(disk)?;
            std::fs::rename(&temporary, path).map_err(disk)?;
            Ok(copied)
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        written.map(Some)
    }
}

/// A path in a folder of the server as an address's: each name percent-encoded ("Mes documents/Sioul" → "Mes%20documents/Sioul").
pub(crate) fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|name| name.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>())
        .collect::<Vec<_>>()
        .join("/")
}

/// The checksums a multistatus says (`oc:checksums`, Nextcloud's), by the
/// path of each response: "SHA1:… MD5:… ADLER32:…".
fn checksums(body: &str, url: &str) -> BTreeMap<String, String> {
    let Ok(doc) = roxmltree::Document::parse(body) else { return BTreeMap::new() };
    doc.descendants()
        .filter(|n| n.has_tag_name((DAV, "response")))
        .filter_map(|response| {
            let href = response.children().find(|c| c.has_tag_name((DAV, "href")))?.text()?.trim().to_string();
            let sums = response.descendants().filter(|n| n.has_tag_name((OC, "checksum"))).filter_map(|n| n.text()).map(str::trim).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
            (!sums.is_empty()).then(|| (dav::path_key(&dav::absolute(url, &href)), sums))
        })
        .collect()
}

/// A content's SHA-1, in lower case hexadecimal, as `OC-Checksum` says it.
fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    sha1::Sha1::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-1 among checksums as a server says them ("SHA1:ab12… MD5:…"), in lower case.
fn sha1_of(checksums: &str) -> Option<String> {
    checksums.split_whitespace().find_map(|sum| sum.split_once(':').filter(|(kind, _)| kind.eq_ignore_ascii_case("sha1")).map(|(_, hex)| hex.to_ascii_lowercase()))
}

/// Why, in a code the window says in words: "login:murena.io".
fn code(error: &SyncError, host: &str) -> String {
    let kind = match error {
        SyncError::Login(_) | SyncError::AppPassword(_) => "login",
        SyncError::Tls(_) => "tls",
        SyncError::Network(_) => "network",
        SyncError::NoPassword => "no-password",
        SyncError::Disk(_) => "disk",
        SyncError::Server(d) if d == "quota" => "quota",
        _ => "server",
    };
    format!("{kind}:{host}")
}

/// Where an account's files are on its server, Nextcloud's way
/// (`…/remote.php/dav/files/<user>/`, the user as the server names it in its
/// principal), from the address its calendars were found at; none for a
/// server of another kind. On that server only: a principal named on another
/// is refused (`SyncError::Elsewhere`), since every request there would carry
/// the account's login.
pub(crate) fn files_root(server: &Server, login: &Login) -> Result<Option<String>, SyncError> {
    let Some(at) = login.url.find("/remote.php/") else { return Ok(None) };
    let base = format!("{}/remote.php/dav/", &login.url[..at]);
    let mut response = server.request(false, "PROPFIND", &base, &[("Depth", "0"), ("Content-Type", XML)], PRINCIPAL.as_bytes().to_vec())?;
    if response.status().as_u16() != 207 {
        return Ok(None);
    }
    let body = response.body_mut().with_config().limit(LISTING).read_to_string().map_err(|e| failed(&e))?;
    let (responses, _) = dav::multistatus(&body)?;
    let principal = responses.iter().find_map(|r| r.prop(DAV, "current-user-principal").and_then(|p| p.hrefs.first().cloned()));
    let root = principal.and_then(|href| {
        let at = href.find("/principals/users/")?;
        let user = href[at + "/principals/users/".len()..].trim_end_matches('/');
        (!user.is_empty() && !user.contains('/')).then(|| dav::absolute(&base, &format!("{}/files/{user}/", &href[..at])))
    });
    match root {
        Some(root) if dav::origin(&root).is_none() || dav::origin(&root) != dav::origin(&base) => Err(SyncError::Elsewhere(root)),
        root => Ok(root),
    }
}

// ---------------------------------------------------------------- finding it

/// Where a folder may be on a server, as paths in an account's files, the
/// likeliest first: on a phone, its path from the storage, which eDrive
/// carries as the cloud's root (`/storage/emulated/0/Documents/Sioul` →
/// `Documents/Sioul`), or from the Nextcloud app's own folder; under a folder
/// the Nextcloud client carries (`nextcloud`: its `nextcloud.cfg`, as text),
/// the same under its `targetPath`; then its last one, two and three names.
pub fn places(folder: &Path, nextcloud: &[String]) -> Vec<String> {
    let text = folder.to_string_lossy().replace('\\', "/");
    let text = text.trim_end_matches('/');
    let mut out: Vec<String> = Vec::new();
    for storage in ["/storage/emulated/0", "/sdcard", "/storage/self/primary", "/mnt/sdcard", "/mnt/user/0/primary"] {
        let Some(rest) = text.strip_prefix(storage).and_then(|r| r.strip_prefix('/')) else { continue };
        // The Nextcloud app's own: Android/media/com.nextcloud.client/nextcloud/<account>/<path>.
        match rest.strip_prefix("Android/media/com.nextcloud.client/nextcloud/").and_then(|r| r.split_once('/')) {
            Some((_, inside)) => out.push(inside.to_string()),
            None if !rest.starts_with("Android/") => out.push(rest.to_string()),
            None => {}
        }
    }
    for config in nextcloud {
        out.extend(nextcloud_places(text, config));
    }
    let names: Vec<&str> = text.rsplit('/').filter(|n| !n.is_empty()).take(3).collect();
    for n in 1..=names.len() {
        out.push(names[..n].iter().rev().copied().collect::<Vec<_>>().join("/"));
    }
    let mut seen = BTreeSet::new();
    out.into_iter().map(|p| p.trim_matches('/').to_string()).filter(|p| !p.is_empty() && seen.insert(p.clone())).collect()
}

/// The places a Nextcloud client's configuration gives a folder: under one of
/// its `localPath`, the same under that folder's `targetPath`
/// ("0\Folders\1\localPath=/home/me/Nextcloud/", "0\Folders\1\targetPath=/").
fn nextcloud_places(folder: &str, config: &str) -> Vec<String> {
    let mut local: BTreeMap<&str, &str> = BTreeMap::new();
    let mut target: BTreeMap<&str, &str> = BTreeMap::new();
    for line in config.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        if let Some(prefix) = key.trim().strip_suffix("localPath") {
            local.insert(prefix, value.trim());
        } else if let Some(prefix) = key.trim().strip_suffix("targetPath") {
            target.insert(prefix, value.trim());
        }
    }
    local
        .into_iter()
        .filter_map(|(prefix, root)| {
            let root = root.trim_end_matches('/');
            let rest = folder.strip_prefix(root).filter(|r| !root.is_empty() && (r.is_empty() || r.starts_with('/')))?;
            Some(format!("{}/{}", target.get(prefix)?.trim_matches('/'), rest.trim_matches('/')))
        })
        .collect()
}

/// Looks for the sharing folder on the accounts' servers: the place given by
/// hand alone when there is one (`State::given`), else `places` under each
/// account's files. Confirmed only where the server's `seal.toml` is this
/// folder's, byte for byte; a folder sealed otherwise is never used. The
/// state, saved, says what was found, or why nothing was.
pub fn find(memory: &Path, folder: &Path, logins: &[Login], places: &[String], now: i64) -> State {
    find_with(memory, folder, logins, places, now, LIMITS)
}

pub(crate) fn find_with(memory: &Path, folder: &Path, logins: &[Login], places: &[String], now: i64, limits: Limits) -> State {
    let mut state = State::load(memory);
    // Sioul keeping the folder itself: nothing to look for.
    if state.mode == MIRROR {
        return state;
    }
    if state.folder != shown(folder) {
        // Another folder: what was fetched for the last one goes.
        let _ = std::fs::remove_dir_all(cache_of(memory));
        state = State { on: state.on, given: state.given.clone(), asked: state.asked, send: state.send, folder: shown(folder), ..State::default() };
    }
    state.looked = now;
    state.confirmed = 0;
    let Some(seal) = seal_of(folder) else {
        state.said = "no-seal".into();
        let _ = state.save(memory);
        return state;
    };
    let given = state.given.trim().to_string();
    let mut said: Vec<String> = Vec::new();
    let mut looked_on: Vec<String> = Vec::new();
    for login in logins {
        let host = host_of(&login.url);
        // Given as an address: on its own server only.
        if given.contains("://") && host_of(&given) != host {
            continue;
        }
        let server = match Server::new(login, limits) {
            Ok(server) => server,
            Err(e) => {
                said.push(code(&e, &login.account));
                continue;
            }
        };
        let urls: Vec<String> = if given.contains("://") {
            vec![format!("{}/", given.trim_end_matches('/'))]
        } else {
            let root = match files_root(&server, login) {
                Ok(Some(root)) => root,
                Ok(None) => continue,
                Err(e) => {
                    said.push(code(&e, &host));
                    continue;
                }
            };
            let wanted: Vec<String> = if given.is_empty() { places.to_vec() } else { vec![given.trim_matches('/').to_string()] };
            wanted.iter().filter(|p| !p.is_empty()).map(|p| format!("{root}{}/", encode_path(p))).collect()
        };
        if !looked_on.contains(&host) {
            looked_on.push(host.clone());
        }
        for url in urls {
            match server.small(&format!("{url}seal.toml")) {
                Ok(Some((bytes, etag))) if bytes == seal => {
                    confirm(&mut state, memory, login, &url, &seal, etag, now);
                    let _ = state.save(memory);
                    return state;
                }
                Ok(Some(_)) => said.push(format!("seal-differs:{host}")),
                Ok(None) => {}
                Err(e) => {
                    said.push(code(&e, &host));
                    // A refused password is not tried again and again (it could lock the account); a network down neither.
                    if e.is_lasting() || matches!(e, SyncError::Network(_)) {
                        break;
                    }
                }
            }
        }
    }
    // The likeliest reason first: a folder sealed otherwise, then what you can mend.
    let rank = |code: &String| ["seal-differs:", "login:", "no-password:", "tls:", "network:", "server:", "disk:"].iter().position(|p| code.starts_with(p)).unwrap_or(9);
    said.sort_by_key(rank);
    state.said = match said.into_iter().next() {
        Some(why) => why,
        None if looked_on.is_empty() && given.contains("://") => format!("no-account-for:{}", host_of(&given)),
        None if looked_on.is_empty() => "no-account".into(),
        None => format!("not-found:{}", looked_on.join(", ")),
    };
    let _ = state.save(memory);
    state
}

/// The folder found at `url`: confirmed, its seal kept in the cache, which
/// holds what was fetched under that seal only.
fn confirm(state: &mut State, memory: &Path, login: &Login, url: &str, seal: &[u8], etag: Option<String>, now: i64) {
    let cache = cache_of(memory);
    if seal_of(&cache).as_deref() != Some(seal) {
        let _ = std::fs::remove_dir_all(&cache);
        state.files.clear();
        state.folders.clear();
    }
    if state.url != url {
        state.files.clear();
        state.folders.clear();
    }
    let _ = crate::share::write_atomically(&cache.join("seal.toml"), seal);
    state.account = login.account.clone();
    state.url = url.to_string();
    state.confirmed = now;
    state.sealed = now;
    state.said.clear();
    // Known by its ETag there: read again only when it changes, or six hours on.
    if let Some(etag) = etag {
        state.files.insert("seal.toml".into(), Fetched { etag, size: seal.len() as u64 });
    }
}

// ---------------------------------------------------------------- fetching

/// How a device stands, for the pace of its pulls (`due`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pace {
    /// A phone; else a computer, which pulls once a minute.
    pub phone: bool,
    /// Sioul on its screen: you are there.
    pub shown: bool,
    /// Another device says it is in use, as read here (what was fetched too).
    pub in_use: bool,
    /// A dose's alarm, a waking, the background step asking for news now.
    pub urgent: bool,
    /// The phone's background step's pace now (seconds): five minutes, fifteen while you sleep.
    pub step: i64,
}

/// Whether a pull is due now. The first after the folder is found, at once,
/// whatever the hour: until it comes, this device knows the others only from
/// the folder the sync app left stale, so whether another one is in use
/// cannot be told (6 October 2026, 23:30: found on the phone at night, never
/// pulled, the desk in use taken for closed hours before). Then a computer, a
/// phone with Sioul on its screen or another device in use, once a minute; a
/// phone in the background otherwise at its step's pace, the night's
/// included: a device opened at night is noticed within its fifteen minutes.
/// Asked for news, twenty seconds after the last.
pub fn due(state: &State, now: i64, pace: Pace) -> bool {
    let since = now - state.tried;
    // Tried "later" than now: the clock was set back since; never a reason to wait.
    if state.tried < state.confirmed || since < 0 {
        return true;
    }
    if pace.urgent {
        return since >= 20;
    }
    if !pace.phone || pace.shown || pace.in_use {
        return since >= 60;
    }
    since >= pace.step - 30
}

/// The longest of another device's records a pull brings (`LARGEST`); a test's own, in its thread.
fn round_cap() -> u64 {
    #[cfg(test)]
    if let Some(cap) = tests::ROUND_CAP.with(std::cell::Cell::get) {
        return cap;
    }
    LARGEST
}

/// How long a dose's or a waking's alarm waits for the others' news (`alarm_news`).
#[derive(Debug, Clone, Copy)]
pub struct AlarmTimes {
    /// Sioul's own pull alone, before the sync app is asked too.
    pub alone: Duration,
    /// In all, at most: the twenty seconds the sync app had, and the five the pull had after them.
    pub most: Duration,
}

/// An alarm's waits, as they are on a phone.
pub const ALARM_TIMES: AlarmTimes = AlarmTimes { alone: Duration::from_secs(5), most: Duration::from_secs(25) };

/// Where an alarm's news came from (`alarm_news`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlarmNews {
    /// A pull begun since the alarm went through: what it read is trusted alone.
    Pulled,
    /// No pull since went through; the sync app asked and waited for, as
    /// before Sioul's own pull. `heard`: the devices whose entry or claim on
    /// the health part says more since the alarm began, by its own times (in
    /// the folder, or in what was fetched beside it). Of the others no news
    /// came: what this device knows
    /// of them is from before the alarm, and one known closed then may have
    /// opened since and answered (`sioul_core::health::doubts_unread`).
    SyncApp { heard: BTreeSet<String> },
}

/// What a dose's or a waking's alarm reads before it decides, `since` its
/// start (Unix seconds), on a phone whose folder is found on the server
/// (`State::fetching`; docs/android.md, "Doses while Sioul is away"). Sioul's
/// own pull alone is trusted only once a pull begun since the alarm went
/// through (`State::went_through_since`; in this process, by its steady clock:
/// `PULLED`): this one, or another's that ran meanwhile in this process
/// (`lock`, one pull at a time: another alarm's, the window's), waited for,
/// never passed over. A pull that fails, or has not gone through within
/// `times.alone`: the sync app asked as before (`ask`, which asks it and says
/// how long its wait still runs: twenty seconds, less when another alarm
/// asked it a moment ago), and waited for, that wait ended as soon as a pull
/// since the alarm goes through; `times.most` in all at most. `clock` gives
/// the time now (Unix seconds); `key` opens the others' entries and claims.
#[allow(clippy::too_many_arguments)]
pub fn alarm_news(memory: &Path, folder: &Path, own: &str, key: [u8; 32], login: Option<Login>, lock: &'static Mutex<()>, since: i64, clock: std::sync::Arc<dyn Fn() -> i64 + Send + Sync>, ask: &mut dyn FnMut() -> Duration) -> AlarmNews {
    alarm_news_with(memory, folder, own, key, login, lock, since, ALARM_TIMES, clock, ask, LIMITS)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn alarm_news_with(memory: &Path, folder: &Path, own: &str, key: [u8; 32], login: Option<Login>, lock: &'static Mutex<()>, since: i64, times: AlarmTimes, clock: std::sync::Arc<dyn Fn() -> i64 + Send + Sync>, ask: &mut dyn FnMut() -> Duration, limits: Limits) -> AlarmNews {
    let begun = Instant::now();
    let until = begun + times.most;
    // What each other device says of itself now, for the doses: what the sync app brings may say more.
    let before = said_by_each(folder, &key, own);
    let mut pulling = {
        let (memory, folder, own, clock) = (memory.to_path_buf(), folder.to_path_buf(), own.to_string(), std::sync::Arc::clone(&clock));
        Some(std::thread::spawn(move || pull_since_with(&memory, &folder, &own, login.as_ref(), lock, since, begun, until, &*clock, limits)))
    };
    let mut went = None;
    let mut asked: Option<Instant> = None;
    loop {
        if went.is_none() && pulling.as_ref().is_some_and(std::thread::JoinHandle::is_finished) {
            went = pulling.take().map(|pull| pull.join().unwrap_or(false));
        }
        if went == Some(true) {
            return AlarmNews::Pulled;
        }
        let now = Instant::now();
        // Failed, or not through yet: the sync app asked, as before.
        if asked.is_none() && (went == Some(false) || now >= begun + times.alone) {
            asked = Some(now + ask());
        }
        if asked.is_some_and(|end| now >= end) || now >= until {
            // A pull since, through at the last moment, counts all the same.
            if through_since(memory, folder, since, begun, clock()) {
                return AlarmNews::Pulled;
            }
            return AlarmNews::SyncApp { heard: heard_since(&before, &said_by_each(folder, &key, own)) };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Whether a pull begun since the alarm (`since`, Unix seconds; `begun`, on
/// this process's steady clock) went through. The last one gone through in
/// this process, when it is the one the state says, is judged by the steady
/// clock (`PULLED`), which no setting of the clock moves; another process's
/// by its saved time, within `since` and `now`.
fn through_since(memory: &Path, folder: &Path, since: i64, begun: Instant, now: i64) -> bool {
    let state = State::load(memory);
    if !state.pulls_well(folder) {
        return false;
    }
    match PULLED.lock().ok().and_then(|all| all.get(memory).copied()) {
        Some((at, last)) if last == state.last => at >= begun,
        _ => state.went_through_since(folder, since, now),
    }
}

/// A pull begun since the alarm (`since`, Unix seconds; `begun`, this
/// process's steady clock) gone through: one that ran meanwhile, waited for
/// (`lock`: one pull at a time in this process), else this one, begun before
/// `until`. False when a pull begun since failed, or none could begin in time.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pull_since_with(memory: &Path, folder: &Path, own: &str, login: Option<&Login>, lock: &Mutex<()>, since: i64, begun: Instant, until: Instant, clock: &dyn Fn() -> i64, limits: Limits) -> bool {
    let through = || through_since(memory, folder, since, begun, clock());
    let _one = loop {
        match lock.try_lock() {
            Ok(held) => break held,
            Err(std::sync::TryLockError::Poisoned(held)) => break held.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            Err(std::sync::TryLockError::WouldBlock) => return through(),
        }
    };
    if through() {
        return true;
    }
    // One begun since that failed (another alarm's): the sync app, not another
    // try within this alarm. Tried "later" than now: the clock was set back,
    // tried again; one gone through before this alarm began: pulled again.
    let state = State::load(memory);
    if (!state.said.is_empty() && (since..=clock()).contains(&state.tried)) || Instant::now() >= until {
        return false;
    }
    let Some(login) = login else { return false };
    let _ = pull_with(memory, folder, own, login, clock(), limits);
    through()
}

/// What each other device says of itself as the doses read it (`health::know`):
/// its entry (`devices`) and its claim on the health part (`lease`), the later
/// of the folder's copy and the one fetched beside it, each by its own times
/// (`devices::written`; the claim's renewal and its end).
type Said = BTreeMap<String, (Option<(i64, i64, i64, i64, i64)>, Option<(i64, i64)>)>;

pub(crate) fn said_by_each(folder: &Path, key: &[u8; 32], own: &str) -> Said {
    let mut out: Said = BTreeMap::new();
    for entry in crate::devices::all(folder, key).0.into_iter().filter(|e| e.id != own) {
        out.entry(entry.id.clone()).or_default().0 = Some(crate::devices::written(&entry));
    }
    for claim in crate::lease::claims(folder, key, "health").into_iter().filter(|c| c.computer != own) {
        out.entry(claim.computer.clone()).or_default().1 = Some((claim.renewed, claim.until));
    }
    out
}

/// The devices whose entry or health claim says more than before: later by
/// its own times. Another part's claim (notices, invoices) is no news of a
/// session for the doses; a file touched with the same bytes, or an older
/// copy put back, is none either.
fn heard_since(before: &Said, now: &Said) -> BTreeSet<String> {
    now.iter()
        .filter(|(id, (entry, claim))| {
            let (was_entry, was_claim) = before.get(*id).copied().unwrap_or_default();
            entry.is_some_and(|e| was_entry.is_none_or(|w| e > w)) || claim.is_some_and(|c| was_claim.is_none_or(|w| c > w))
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// What a pull did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pulled {
    /// Files and folders the server's listings showed.
    pub listed: usize,
    /// Files that came down and are kept.
    pub fetched: usize,
    /// This device's own files that went up (`MIRROR`, or beside a sync app: `send`).
    pub sent: usize,
    /// This device's own files found there already as they are here: nothing sent (`send`).
    pub same: usize,
    /// This device's records found longer there than here (a sync app put an
    /// older copy back here): left as they are there (`send`).
    pub longer: usize,
    /// This device's files found otherwise there, by kind and size, for the
    /// log ("records 70708 here, 71234 there"; eight at most): never a name nor a content.
    pub differ: Vec<String>,
    /// Large files left for later on a metered or slow connection (`send`, `fetch_wanted`).
    pub paced: usize,
    /// Files that did not go (too slow, cut), held back until their backoff
    /// (`send`); sealed files fetched that did not open whole (`fetch_wanted`).
    pub held: usize,
    /// The last failure, in detail (`send`).
    pub failure: Option<Failure>,
    /// Bytes that came down.
    pub bytes: u64,
    /// The other devices' records found past what a pull brings, left to the sync app (`State::left`).
    pub left: usize,
    /// What went wrong, as `State::said` says it; none when it went through.
    pub problem: Option<String>,
}

/// Why a pull stopped: the network or the server, or the folder no longer confirmed.
enum Stop {
    Failed(SyncError),
    Unconfirmed(String),
}

impl From<SyncError> for Stop {
    fn from(e: SyncError) -> Stop {
        Stop::Failed(e)
    }
}

/// The other devices' files newer on the server than here, fetched beside the
/// synced folder (see the module's words); `own` is this device's id, whose
/// files are never fetched. Nothing when the folder is not confirmed, or its
/// seal here is no longer the one fetched under. Never on the window's thread.
pub fn pull(memory: &Path, folder: &Path, own: &str, login: &Login, now: i64) -> Pulled {
    pull_with(memory, folder, own, login, now, LIMITS)
}

pub(crate) fn pull_with(memory: &Path, folder: &Path, own: &str, login: &Login, now: i64, limits: Limits) -> Pulled {
    let mut state = State::load(memory);
    let mut pulled = Pulled::default();
    // Sioul keeping the folder itself: no backup beside it (`step`).
    if state.mode == MIRROR {
        pulled.problem = Some("mirror".into());
        return pulled;
    }
    if !state.confirmed_for(folder) || state.account != login.account {
        pulled.problem = Some("not-confirmed".into());
        return pulled;
    }
    let cache = cache_of(memory);
    // Switched off here: not a request; what the folder caught up with goes.
    if !state.fetching() {
        tidy_cache(&state, folder, memory, own);
        pulled.problem = Some("off".into());
        return pulled;
    }
    // This folder's seal changed here (sharing started again): what was
    // fetched belongs to another sharing, and the server's folder is looked at again.
    if !usable(folder, memory) {
        let _ = std::fs::remove_dir_all(&cache);
        state = State { on: state.on, given: state.given.clone(), asked: state.asked, send: state.send, folder: state.folder.clone(), said: "no-seal".into(), ..State::default() };
        let _ = state.save(memory);
        pulled.problem = Some(state.said);
        return pulled;
    }
    let loaded_left = state.left.clone();
    state.tried = now;
    let begun = Instant::now();
    let host = state.host();
    let outcome = Server::new(login, limits).map_err(Stop::Failed).and_then(|server| Puller { server: &server, folder, into: &cache, own, state: &mut state, pulled: &mut pulled, now, own_there: BTreeMap::new(), covered: BTreeSet::new(), lost: false, blobs: Vec::new(), blobs_listed: None }.run());
    match outcome {
        Ok(()) => {
            state.last = now;
            state.said.clear();
        }
        Err(Stop::Unconfirmed(why)) => {
            state.confirmed = 0;
            state.said = why;
        }
        Err(Stop::Failed(e)) => state.said = code(&e, &host),
    }
    tidy_cache(&state, folder, memory, own);
    if state.said.is_empty() {
        // Gone through, begun at this moment of this process (`went_through_since`).
        if let Ok(mut all) = PULLED.lock()
            && all.get(memory).is_none_or(|(at, _)| *at < begun)
        {
            all.insert(memory.to_path_buf(), (begun, state.last));
        }
    }
    let _ = state.save_pulled(memory, now + begun.elapsed().as_secs() as i64 + 1, &loaded_left);
    pulled.problem = (!state.said.is_empty()).then(|| state.said.clone());
    pulled
}

/// The last pull of each memory gone through in this process: when it began
/// on the process's own steady clock, and its time as saved (`State::last`).
/// A clock set back makes a pull's saved time later than the moment; an
/// alarm in this process tells by this one whether that pull began before it.
static PULLED: Mutex<BTreeMap<PathBuf, (Instant, i64)>> = Mutex::new(BTreeMap::new());

/// What is no longer needed in the cache, gone, without the network: this
/// device's own files (never fetched, never read from there), records the
/// synced folder caught up with, entries and notes the same as the folder's
/// and, the backup switched off, all of them; records longer than the
/// folder's stay until it catches up (read past its end here, a records'
/// file gone would read as cut, and the doses would doubt for hours). Sealed
/// files fetched for a note waiting (`fetch_wanted`) go once it is written,
/// once the synced folder holds them, or after a few days.
pub fn tidy(memory: &Path, folder: &Path, own: &str) {
    let state = State::load(memory);
    if state.folder == shown(folder) && state.mode != MIRROR {
        tidy_cache(&state, folder, memory, own);
    }
}

fn tidy_cache(state: &State, folder: &Path, memory: &Path, own: &str) {
    let cache = &cache_of(memory);
    // What a crash left half written, an hour later.
    for dir in [cache.to_path_buf(), cache.join("devices"), cache.join("blobs")].into_iter().chain(std::fs::read_dir(cache.join("leases")).into_iter().flatten().filter_map(Result::ok).map(|e| e.path())) {
        crate::share::clean_leftovers(&dir);
    }
    for relative in cached_files(cache) {
        let (here, fetched) = (folder.join(&relative), cache.join(&relative));
        let name = relative.rsplit('/').next().unwrap_or_default();
        let gone = match kind_of(&relative) {
            None => continue,
            Some((_, id)) if id == own => true,
            Some((Kind::Round, _)) => std::fs::metadata(&here).is_ok_and(|h| std::fs::metadata(&fetched).is_ok_and(|f| h.len() >= f.len())),
            Some(_) => !state.fetching() || std::fs::read(&here).ok().is_some_and(|h| std::fs::read(&fetched).ok().is_some_and(|f| f == h)),
        };
        if gone && !name.starts_with('.') {
            let _ = std::fs::remove_file(&fetched);
        }
    }
    // Sealed files fetched because a note or a paper waited for them
    // (`fetch_wanted`): gone once the synced folder holds the same, once
    // nothing waits for them (written), after `WANTED_KEPT_DAYS` whatever,
    // and all of them with the backup switched off.
    // One aged out is no longer waited for either, until an exchange that
    // reads notes finds it missing again: never fetched every few days for a
    // note nothing here writes (Android's leave to read them taken back).
    let wanted = crate::blobs::Wanted::load(memory);
    let mut aged: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(cache.join("blobs")).into_iter().flatten().filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(meta) = entry.metadata() else { continue };
        if name.starts_with('.') || !meta.is_file() {
            continue;
        }
        let brought = std::fs::metadata(folder.join("blobs").join(&name)).is_ok_and(|here| here.len() == meta.len());
        let old = meta.modified().ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() >= WANTED_KEPT_DAYS * 86_400);
        if !state.fetching() || brought || old || !wanted.blobs.contains_key(&name) {
            let _ = std::fs::remove_file(entry.path());
            if old {
                aged.push(name);
            }
        }
    }
    if !aged.is_empty() {
        let _ = crate::blobs::Wanted::change(memory, |wanted| wanted.blobs.retain(|name, _| !aged.contains(name)));
    }
}

/// What a file of the folder is, by its path, and whose: a device's records
/// (`<id>-<n>.jsonl`), its notes to the others (`<id>.toml`), its entry
/// (`devices/<id>.device`), a claim (`leases/<part>/<id>.lease`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Round,
    Seen,
    Device,
    Claim,
    /// Its texts to send (`texts/send/<id>.jsonl`): a small file of their own (`place_texts`).
    Texts,
}

pub(crate) fn is_id(id: &str) -> bool {
    id.len() == 36 && uuid::Uuid::parse_str(id).is_ok()
}

pub(crate) fn kind_of(relative: &str) -> Option<(Kind, &str)> {
    let parts: Vec<&str> = relative.split('/').collect();
    match parts.as_slice() {
        [name] => {
            if let Some((id, round)) = name.strip_suffix(".jsonl").and_then(|stem| stem.rsplit_once('-')) {
                return (is_id(id) && round.parse::<u32>().is_ok()).then_some((Kind::Round, id));
            }
            name.strip_suffix(".toml").filter(|id| is_id(id)).map(|id| (Kind::Seen, id))
        }
        ["devices", name] => name.strip_suffix(".device").filter(|id| is_id(id)).map(|id| (Kind::Device, id)),
        ["leases", _, name] => name.strip_suffix(".lease").filter(|id| !id.is_empty() && !id.starts_with('.')).map(|id| (Kind::Claim, id)),
        ["texts", "send", name] => name.strip_suffix(".jsonl").filter(|id| is_id(id)).map(|id| (Kind::Texts, id)),
        _ => None,
    }
}

/// The files of the cache, by their path in the folder: the folder's own, `devices/`, `leases/<part>/`.
fn cached_files(cache: &Path) -> Vec<String> {
    let names = |dir: &Path| -> Vec<String> { std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_file())).map(|e| e.file_name().to_string_lossy().to_string()).collect() };
    let mut out: Vec<String> = names(cache);
    out.extend(names(&cache.join("devices")).into_iter().map(|n| format!("devices/{n}")));
    for part in std::fs::read_dir(cache.join("leases")).into_iter().flatten().filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_dir())) {
        let part = part.file_name().to_string_lossy().to_string();
        out.extend(names(&cache.join("leases").join(&part)).into_iter().map(|n| format!("leases/{part}/{n}")));
    }
    out.extend(names(&cache.join("texts").join("send")).into_iter().map(|n| format!("texts/send/{n}")));
    out
}

// ---------------------------------------------------------------- sealed files waited for

/// A step fetches at most this many sealed files waited for (`fetch_wanted`),
const WANTED_FILES: usize = 16;
/// and this many bytes of them, the first whatever its size.
const WANTED_BYTES: u64 = 16 << 20;
/// A file waited for under this size (a note, bytes) comes at once, the
/// connection metered or slow too; a larger one (a paper) at most every ten minutes then.
pub const SMALL_WANTED: u64 = 1 << 20;
/// A sealed file fetched for a note waiting is kept this long at most (days),
/// in case it was not written meanwhile (`tidy_cache`).
const WANTED_KEPT_DAYS: u64 = 3;
/// A sealed file waited for and not on the server yet (or cut on the way) is
/// asked again within this long (seconds): it comes as soon as it is there.
const WANTED_AGAIN: i64 = 5 * 60;

/// The sealed files of the others' notes and papers this device waits for
/// (`blobs::Wanted`: their records came, the files they name did not),
/// fetched from the folder's server by their names, beside the synced folder
/// (`overlay`'s `blobs/`, read by `blobs::get`), when the backup is found
/// there under the same seal (docs/database.md, "Sealed files fetched when
/// waited for"). Only those neither in the folder nor fetched yet, the
/// smallest first, at most `WANTED_FILES` and `WANTED_BYTES` a step (the
/// first whatever its size). On a metered or slow connection (`frugal`),
/// the pace of what is sent (`send`): one of `SMALL_WANTED` or more at most
/// every ten minutes, a note's at once. Each kept only once it opens whole to
/// its content (`blobs::check`): one that does not is asked again after a
/// while (`backoff`, up to an hour); one not on the server yet, within five
/// minutes. No listing of the server's `blobs/`: the records name them. Never
/// on the window's thread; one fetch at a time on this device.
pub fn fetch_wanted(memory: &Path, folder: &Path, login: &Login, key: &[u8; 32], now: i64, frugal: bool) -> Pulled {
    fetch_wanted_with(memory, folder, login, key, now, frugal, LIMITS)
}

/// Whether a sealed file waited for is due to be fetched (`fetch_wanted`):
/// cheap, nothing of the network; none while nothing waits.
pub fn wanted_due(memory: &Path, folder: &Path, now: i64) -> bool {
    crate::blobs::Wanted::path(memory).exists() && !wanted_now(&crate::blobs::Wanted::load(memory), folder, &cache_of(memory).join("blobs"), now).is_empty()
}

/// The sealed files waited for that are neither in the folder nor fetched
/// (`into`), and not asked lately in vain.
fn wanted_now<'a>(wanted: &'a crate::blobs::Wanted, folder: &Path, into: &Path, now: i64) -> Vec<(&'a String, &'a crate::blobs::Want)> {
    let here = |dir: &Path, name: &str| std::fs::metadata(dir.join(name)).is_ok_and(|m| m.len() > 0);
    wanted.blobs.iter().filter(|(name, want)| want.again <= now && !here(&folder.join("blobs"), name) && !here(into, name)).collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fetch_wanted_with(memory: &Path, folder: &Path, login: &Login, key: &[u8; 32], now: i64, frugal: bool, limits: Limits) -> Pulled {
    let mut pulled = Pulled::default();
    let state = State::load(memory);
    if state.mode == MIRROR || !state.fetching() || !state.confirmed_for(folder) || state.account != login.account || !usable(folder, memory) {
        pulled.problem = Some("not-confirmed".into());
        return pulled;
    }
    let into = cache_of(memory).join("blobs");
    sioul_core::filelock::with_lock(&memory.with_file_name("wanted.step"), || {
        let wanted = crate::blobs::Wanted::load(memory);
        let mut due = wanted_now(&wanted, folder, &into, now);
        if due.is_empty() {
            return pulled;
        }
        due.sort_by_key(|(name, want)| (want.size, want.since, *name));
        let mut tried: BTreeMap<String, (u32, i64)> = BTreeMap::new();
        let mut large_at = wanted.large_at;
        let (mut count, mut budget) = (0, WANTED_BYTES);
        let outcome = Server::new(login, limits).and_then(|server| {
            for (name, want) in due {
                if count >= WANTED_FILES || (count > 0 && want.size > budget) {
                    break;
                }
                let large = want.size >= SMALL_WANTED;
                if frugal && large && now - large_at < PACE {
                    pulled.paced += 1;
                    continue;
                }
                if frugal && large {
                    large_at = now;
                }
                count += 1;
                budget = budget.saturating_sub(want.size);
                let path = into.join(name);
                let failed = |soon: bool| {
                    let tries = want.tries + 1;
                    (tries, now + if soon { backoff(tries).min(WANTED_AGAIN) } else { backoff(tries) })
                };
                match server.download(&format!("{}{}", state.url, encode_path(&format!("blobs/{name}"))), &path, BLOB_LIMIT) {
                    Ok(Some(size)) => match crate::blobs::check(&path, key, &want.hash) {
                        Ok(()) => {
                            pulled.fetched += 1;
                            pulled.bytes += size;
                            tried.insert(name.clone(), (0, 0));
                        }
                        // Not whole, or another content: never kept, asked again after a while.
                        Err(fault) => {
                            let _ = std::fs::remove_file(&path);
                            pulled.held += 1;
                            tried.insert(name.clone(), failed(!matches!(fault, crate::blobs::Fault::Broken)));
                        }
                    },
                    // Not on the server yet (its writer's sync app has not sent it), or too large.
                    Ok(None) => {
                        tried.insert(name.clone(), failed(true));
                    }
                    Err(e) => {
                        tried.insert(name.clone(), failed(true));
                        return Err(e);
                    }
                }
            }
            Ok(())
        });
        if let Err(e) = outcome {
            pulled.problem = Some(code(&e, &state.host()));
        }
        let _ = crate::blobs::Wanted::change(memory, |wanted| {
            for (name, (tries, again)) in tried {
                if let Some(want) = wanted.blobs.get_mut(&name) {
                    (want.tries, want.again) = (tries, again);
                }
            }
            wanted.large_at = wanted.large_at.max(large_at);
        });
        pulled
    })
}

/// One pull: the server's folder looked through, what is newer there fetched.
struct Puller<'a> {
    server: &'a Server,
    /// The sharing folder as the sync app keeps it, and where what comes goes.
    folder: &'a Path,
    into: &'a Path,
    own: &'a str,
    state: &'a mut State,
    pulled: &'a mut Pulled,
    now: i64,
    /// This device's own files seen in the folders looked through, their ETags there (`MIRROR`).
    own_there: BTreeMap<String, String>,
    /// The folders of the server looked through in this step ("" the folder itself, "devices/").
    covered: BTreeSet<String>,
    /// Sioul keeping the folder itself, its copy lost: none of its records here.
    lost: bool,
    /// The others' sealed files to bring (`MIRROR`), listed by `run`, fetched
    /// last (`fetch_blobs`), once this device's own files went up.
    blobs: Vec<(String, Item)>,
    /// `blobs/`'s ETag as listed: kept once every file of it came.
    blobs_listed: Option<String>,
}

impl Puller<'_> {
    fn url(&self, relative: &str) -> String {
        format!("{}{}", self.state.url, encode_path(relative))
    }

    /// A folder of the server not changed since it was last looked through, lately.
    fn unchanged(&self, relative: &str, etag: &str) -> bool {
        !self.lost && !etag.is_empty() && self.state.folders.get(relative).is_some_and(|l| l.etag == etag && self.now - l.at < LIST_AGAIN)
    }

    /// A folder looked through to its end: until it changes, not again for a while.
    fn looked(&mut self, relative: &str, etag: &str) {
        self.state.folders.insert(relative.to_string(), Looked { etag: etag.to_string(), at: self.now });
    }

    fn run(&mut self) -> Result<(), Stop> {
        let root = self.server.list(&self.state.url).map_err(|e| match e {
            SyncError::NotFound(_) => Stop::Unconfirmed(format!("seal-gone:{}", self.state.host())),
            e => Stop::Failed(e),
        })?;
        self.pulled.listed += root.items.len();
        // Never fetched from a folder sealed otherwise.
        self.seal(&root)?;
        // Sioul keeping the folder itself, its copy lost (none of its records
        // here): everything looked through again, what it wrote brought back first.
        self.lost = self.mirror() && own_latest(self.folder, self.own).is_none();
        if self.unchanged("", &root.etag) {
            // What was left to the sync app and has come here since: no longer left.
            let (folder, into) = (self.folder, self.into);
            let here = |name: &str| [folder.join(name), into.join(name)].iter().filter_map(|path| std::fs::metadata(path).ok()).map(|m| m.len()).max().unwrap_or(0);
            self.state.left.retain(|name, size| here(name) < *size);
            return Ok(());
        }
        // Looked through anew: what is left to the sync app, found again.
        self.state.left.clear();
        self.own_listed("", &root);
        for (name, item) in root.items.iter().filter(|(_, item)| !item.dir) {
            match kind_of(name) {
                Some((Kind::Round, id)) if id != self.own => self.round(name, item)?,
                Some((Kind::Seen, id)) if id != self.own => self.small(name, item)?,
                Some(_) if self.mirror() => self.restore(name, item)?,
                _ => {}
            }
        }
        if let Some(devices) = root.items.get("devices").filter(|item| item.dir)
            && !self.unchanged("devices/", &devices.etag)
        {
            let listing = self.server.list(&self.url("devices/"))?;
            self.pulled.listed += listing.items.len();
            self.own_listed("devices/", &listing);
            // This device's own entry there, as listed: an older copy put back there is sent over (`send`).
            if let Some(item) = listing.items.get(&format!("{}.device", self.own)).filter(|item| !item.dir) {
                self.state.own_entry = Some((item.size, self.now));
            }
            for (name, item) in listing.items.iter().filter(|(_, item)| !item.dir) {
                let relative = format!("devices/{name}");
                match kind_of(&relative) {
                    Some((_, id)) if id != self.own => self.small(&relative, item)?,
                    Some(_) if self.mirror() => self.restore(&relative, item)?,
                    _ => {}
                }
            }
            self.forget_gone("devices/", &listing);
            self.looked("devices/", &listing.etag);
        }
        if let Some(leases) = root.items.get("leases").filter(|item| item.dir)
            && !self.unchanged("leases/", &leases.etag)
        {
            let parts = self.server.list(&self.url("leases/"))?;
            self.pulled.listed += parts.items.len();
            for (part, item) in parts.items.iter().filter(|(_, item)| item.dir) {
                let dir = format!("leases/{part}/");
                if self.unchanged(&dir, &item.etag) {
                    continue;
                }
                let listing = self.server.list(&self.url(&dir))?;
                self.pulled.listed += listing.items.len();
                self.own_listed(&dir, &listing);
                for (name, item) in listing.items.iter().filter(|(_, item)| !item.dir) {
                    let relative = format!("{dir}{name}");
                    match kind_of(&relative) {
                        Some((_, id)) if id != self.own => self.small(&relative, item)?,
                        Some(_) if self.mirror() => self.restore(&relative, item)?,
                        _ => {}
                    }
                }
                self.forget_gone(&dir, &listing);
                self.looked(&dir, &listing.etag);
            }
            self.looked("leases/", &parts.etag);
        }
        // Each device's texts to send, a small file of its own: a phone has
        // them within its quarter of an hour, whatever the records' size.
        if let Some(texts) = root.items.get("texts").filter(|item| item.dir)
            && !self.unchanged("texts/", &texts.etag)
        {
            match self.server.list(&self.url("texts/send/")) {
                Ok(listing) => {
                    self.pulled.listed += listing.items.len();
                    self.own_listed("texts/send/", &listing);
                    for (name, item) in listing.items.iter().filter(|(_, item)| !item.dir) {
                        let relative = format!("texts/send/{name}");
                        if let Some((Kind::Texts, id)) = kind_of(&relative)
                            && id != self.own
                        {
                            self.small(&relative, item)?;
                        }
                    }
                    self.forget_gone("texts/send/", &listing);
                    self.looked("texts/", &texts.etag);
                }
                Err(SyncError::NotFound(_)) => self.looked("texts/", &texts.etag),
                Err(e) => return Err(Stop::Failed(e)),
            }
        }
        // Sioul keeping the folder itself: the sealed notes and papers the others put there come too.
        if self.mirror()
            && let Some(blobs) = root.items.get("blobs").filter(|item| item.dir)
            && !self.unchanged("blobs/", &blobs.etag)
        {
            let listing = self.server.list(&self.url("blobs/"))?;
            self.pulled.listed += listing.items.len();
            self.own_listed("blobs/", &listing);
            // Brought last (`fetch_blobs`): a large one on a slow line never
            // holds back this device's own records going up.
            self.blobs = listing.items.iter().filter(|(name, item)| !item.dir && !name.starts_with('.')).map(|(name, item)| (format!("blobs/{name}"), item.clone())).collect();
            self.blobs_listed = Some(listing.etag.clone());
        }
        self.forget_gone("", &root);
        self.looked("", &root.etag);
        Ok(())
    }

    /// Sioul keeps the folder itself (`MIRROR`): what comes goes into it, no sync app beside.
    fn mirror(&self) -> bool {
        self.into == self.folder
    }

    /// A folder of the server just looked through: this device's own files
    /// in it (`MIRROR`), their ETags there, for what goes up next.
    fn own_listed(&mut self, dir: &str, listing: &Listing) {
        self.covered.insert(dir.to_string());
        for (name, item) in listing.items.iter().filter(|(_, item)| !item.dir) {
            let relative = format!("{dir}{name}");
            let own = match kind_of(&relative) {
                Some((_, id)) => id == self.own,
                None => dir == "blobs/" && self.state.sent.contains_key(&relative),
            };
            if own {
                self.own_there.insert(relative, item.etag.clone());
            }
        }
    }

    /// The server's seal, read again when it changed there or every six
    /// hours: another, or none, and nothing more is fetched from there.
    fn seal(&mut self, root: &Listing) -> Result<(), Stop> {
        let host = self.state.host();
        let Some(item) = root.items.get("seal.toml").filter(|item| !item.dir) else { return Err(Stop::Unconfirmed(format!("seal-gone:{host}"))) };
        let known = Fetched { etag: item.etag.clone(), size: item.size };
        if !item.etag.is_empty() && self.state.files.get("seal.toml") == Some(&known) && self.now - self.state.sealed < SEAL_AGAIN {
            return Ok(());
        }
        let here = seal_of(self.folder).ok_or_else(|| Stop::Unconfirmed("no-seal".into()))?;
        match self.server.small(&self.url("seal.toml"))? {
            Some((bytes, _)) if bytes == here => {
                self.state.sealed = self.now;
                self.state.files.insert("seal.toml".into(), known);
                Ok(())
            }
            Some(_) => Err(Stop::Unconfirmed(format!("seal-differs:{host}"))),
            None => Err(Stop::Unconfirmed(format!("seal-gone:{host}"))),
        }
    }

    /// Files of a folder of the server no longer there: forgotten, and their
    /// copies fetched gone (a records' file only once its writer removed it:
    /// every device had read past it).
    fn forget_gone(&mut self, dir: &str, listing: &Listing) {
        let gone: Vec<String> = self.state.files.keys().filter(|path| path.strip_prefix(dir).is_some_and(|name| !name.contains('/') && !listing.items.contains_key(name))).cloned().collect();
        for path in gone {
            self.state.files.remove(&path);
            // Beside a sync app, the copy fetched goes; Sioul keeping the folder
            // itself, another device's records its writer let go (every device read past them).
            let round = matches!(kind_of(&path), Some((Kind::Round, _)));
            if path != "seal.toml" && (self.into != self.folder || round) {
                let _ = std::fs::remove_file(self.into.join(&path));
            }
        }
    }

    /// A small file (notes, an entry, a claim) fetched whole when it changed
    /// there since it was last fetched; kept only when it differs from the
    /// synced folder's copy, and only when all of it came.
    fn small(&mut self, relative: &str, item: &Item) -> Result<(), Stop> {
        let known = Fetched { etag: item.etag.clone(), size: item.size };
        // Sioul keeping the folder itself, one gone from its copy comes again.
        if !item.etag.is_empty() && self.state.files.get(relative) == Some(&known) && (!self.mirror() || self.folder.join(relative).exists()) {
            return Ok(());
        }
        let cap = if matches!(kind_of(relative), Some((Kind::Texts, _))) { TEXTS_LARGEST } else { crate::share::SMALL_FILE };
        if item.size > cap {
            return Ok(());
        }
        let Some((bytes, _)) = self.server.small_up_to(&self.url(relative), cap)? else {
            self.state.files.remove(relative);
            return Ok(());
        };
        self.pulled.bytes += bytes.len() as u64;
        let (here, into) = (self.folder.join(relative), self.into.join(relative));
        if std::fs::read(&here).ok().as_deref() == Some(&bytes[..]) {
            // The sync app brought it already: nothing kept twice.
            if into != here {
                let _ = std::fs::remove_file(&into);
            }
        } else {
            crate::share::write_atomically(&into, &bytes).map_err(|e| Stop::Failed(SyncError::Disk(e)))?;
            self.pulled.fetched += 1;
        }
        self.state.files.insert(relative.to_string(), known);
        Ok(())
    }

    /// One of this device's own files there and not here (`MIRROR`), never
    /// sent from here, or its copy lost (`lost`: none of its records here):
    /// brought back, so that it goes on after its last record rather than
    /// numbering them again over the others' reading.
    fn restore(&mut self, relative: &str, item: &Item) -> Result<(), Stop> {
        let here = self.folder.join(relative);
        if here.exists() || (self.state.sent.contains_key(relative) && !self.lost) || item.size > LARGEST {
            return Ok(());
        }
        let Some(got) = self.server.tail(&self.url(relative), 0)? else { return Ok(()) };
        if let Some(e) = got.error {
            return Err(Stop::Failed(e));
        }
        // Records keep their whole lines only.
        let keep = if matches!(kind_of(relative), Some((Kind::Round, _))) { got.bytes.iter().rposition(|b| *b == b'\n').map_or(0, |at| at + 1) } else { got.bytes.len() };
        crate::share::write_atomically(&here, &got.bytes[..keep]).map_err(|e| Stop::Failed(SyncError::Disk(e)))?;
        let modified = std::fs::metadata(&here).map(|m| crate::share::modified_ns(&m)).unwrap_or(0);
        let etag = if keep == got.bytes.len() { item.etag.clone() } else { String::new() };
        self.state.sent.insert(relative.to_string(), Sent { size: keep as u64, modified, etag });
        self.pulled.fetched += 1;
        self.pulled.bytes += keep as u64;
        Ok(())
    }

    /// The others' sealed files `run` listed, brought one after the other,
    /// last in the step. One that fails stops the rest until the next step,
    /// which looks through the folder again rather than ten minutes later.
    fn fetch_blobs(&mut self) -> Result<(), Stop> {
        for (relative, item) in std::mem::take(&mut self.blobs) {
            if let Err(stop) = self.blob(&relative, &item) {
                self.state.folders.remove("blobs/");
                self.state.folders.remove("");
                self.blobs_listed = None;
                return Err(stop);
            }
        }
        if let Some(etag) = self.blobs_listed.take() {
            self.looked("blobs/", &etag);
        }
        Ok(())
    }

    /// A sealed file another device put there and not here (`MIRROR`):
    /// fetched whole, kept only whole. One sent from here never comes back.
    fn blob(&mut self, relative: &str, item: &Item) -> Result<(), Stop> {
        let here = self.folder.join(relative);
        if self.state.sent.contains_key(relative) || item.size > BLOB_LIMIT {
            return Ok(());
        }
        let known = Fetched { etag: item.etag.clone(), size: item.size };
        if std::fs::metadata(&here).is_ok_and(|m| m.len() == item.size) {
            self.state.files.insert(relative.to_string(), known);
            return Ok(());
        }
        if let Some(size) = self.server.download(&self.url(relative), &here, BLOB_LIMIT)? {
            self.pulled.fetched += 1;
            self.pulled.bytes += size;
            self.state.files.insert(relative.to_string(), known);
        }
        Ok(())
    }

    /// This device's own files (`MIRROR`) changed here since they went up, or
    /// changed or gone there since (seen in the folders just looked through),
    /// sent, in `own_files`' order, each over what is there as it was seen
    /// (`send`). Then what it let go here goes there too: its records past
    /// their time (`remove_old_rounds`: every device read past them), never
    /// its last round, and nothing while none of its records is here; the
    /// sealed files it swept.
    fn push(&mut self) -> Result<(), Stop> {
        for relative in own_files(self.folder, self.own, self.state) {
            let Ok(meta) = std::fs::metadata(self.folder.join(&relative)) else { continue };
            let (size, modified) = (meta.len(), crate::share::modified_ns(&meta));
            let sent = self.state.sent.get(&relative).cloned();
            let covered = self.covered.contains(dir_of(&relative));
            let there = self.own_there.get(&relative).cloned();
            let same_here = sent.as_ref().is_some_and(|s| s.size == size && s.modified == modified);
            // Its ETag there, unsaid when it went up, known now.
            if same_here
                && let (Some(sent), Some(etag)) = (&sent, &there)
                && sent.etag.is_empty()
            {
                self.state.sent.insert(relative.clone(), Sent { etag: etag.clone(), ..sent.clone() });
                continue;
            }
            let same_there = !covered || there.as_deref() == sent.as_ref().map(|s| s.etag.as_str());
            if same_here && same_there {
                continue;
            }
            let seen = if covered { there } else { sent.map(|s| s.etag).filter(|e| !e.is_empty()) };
            self.send(&relative, size, modified, seen)?;
        }
        if let Some(latest) = own_latest(self.folder, self.own)
            && self.covered.contains("")
        {
            let gone: Vec<(String, String)> = self
                .own_there
                .iter()
                .filter(|(relative, _)| matches!(kind_of(relative), Some((Kind::Round, _))) && self.state.sent.contains_key(*relative) && !self.folder.join(relative).exists())
                .filter(|(relative, _)| relative.trim_end_matches(".jsonl").rsplit('-').next().and_then(|n| n.parse::<u32>().ok()).is_some_and(|round| round < latest))
                .map(|(relative, etag)| (relative.clone(), etag.clone()))
                .collect();
            for (relative, etag) in gone {
                self.take_out(&relative, Some(etag))?;
            }
        }
        let swept: Vec<(String, String)> = self.state.sent.iter().filter(|(relative, _)| relative.starts_with("blobs/") && !self.folder.join(relative).exists()).map(|(relative, sent)| (relative.clone(), sent.etag.clone())).collect();
        for (relative, etag) in swept {
            self.take_out(&relative, Some(etag).filter(|e| !e.is_empty()))?;
        }
        Ok(())
    }

    /// One of this device's files sent whole over what is there as it was
    /// seen (`seen`: its ETag; none there). Refused because it changed there
    /// meanwhile (`412`), sent again over what is there now: it is this
    /// device's own. A folder missing there (`409`), made first. A sealed file
    /// goes only where none is: one there already is the same. Refused again
    /// and again: left for the next step; nothing here is lost.
    fn send(&mut self, relative: &str, size: u64, modified: u64, seen: Option<String>) -> Result<(), Stop> {
        let blob = relative.starts_with("blobs/");
        let bytes = std::fs::read(self.folder.join(relative)).map_err(|e| Stop::Failed(SyncError::Disk(format!("{relative}: {e}"))))?;
        let url = self.url(relative);
        let mut seen = if blob { None } else { seen };
        let sha1 = sha1_hex(&bytes);
        for attempt in 0..3 {
            // Given the time its size asks on a slow line (`put_file`).
            let (status, etag) = self.server.put_file(&url, bytes.clone(), seen.as_deref(), modified / 1_000_000_000, &sha1).map_err(|failed| Stop::Failed(failed.error))?;
            match status {
                200 | 201 | 204 => {
                    self.state.sent.insert(relative.to_string(), Sent { size, modified, etag: etag.unwrap_or_default() });
                    self.pulled.sent += 1;
                    self.pulled.bytes += bytes.len() as u64;
                    return Ok(());
                }
                412 if blob => {
                    self.state.sent.insert(relative.to_string(), Sent { size, modified, etag: String::new() });
                    return Ok(());
                }
                412 => seen = self.server.etag(&url)?,
                409 if attempt == 0 => self.make_folders(relative)?,
                507 => return Err(Stop::Failed(SyncError::Server("quota".into()))),
                status => return Err(Stop::Failed(SyncError::Server(format!("PUT: {status}")))),
            }
        }
        Err(Stop::Failed(SyncError::Server("PUT: 412".into())))
    }

    /// The folders a file's path goes through, made there (`MKCOL`) when missing.
    fn make_folders(&mut self, relative: &str) -> Result<(), Stop> {
        let mut at = String::new();
        let names: Vec<&str> = relative.split('/').collect();
        for name in &names[..names.len().saturating_sub(1)] {
            at = format!("{at}{name}/");
            match self.server.mkcol(&self.url(&at))? {
                201 | 405 => {}
                status => return Err(Stop::Failed(SyncError::Server(format!("MKCOL: {status}")))),
            }
        }
        Ok(())
    }

    /// One of this device's files taken out there, only as it was seen when
    /// its ETag is known; gone already, or changed there since (another device
    /// sealed the same content again), forgotten here all the same.
    fn take_out(&mut self, relative: &str, seen: Option<String>) -> Result<(), Stop> {
        match self.server.delete(&self.url(relative), seen.as_deref())? {
            200 | 204 | 404 | 412 => {
                self.state.sent.remove(relative);
                Ok(())
            }
            status => Err(Stop::Failed(SyncError::Server(format!("DELETE: {status}")))),
        }
    }

    /// A device's records, when the server's copy is longer than both the
    /// synced folder's and what was fetched: what is new there, from the end
    /// of the longer copy here, its last whole line asked again and checked
    /// to be the same there (a copy that does not join comes whole). Only
    /// whole lines are kept: a line half arrived comes again next time.
    fn round(&mut self, name: &str, item: &Item) -> Result<(), Stop> {
        let (here, into) = (self.folder.join(name), self.into.join(name));
        let size = |path: &Path| std::fs::metadata(path).map_or(0, |m| m.len());
        let longest = size(&here).max(size(&into));
        let known = Fetched { etag: item.etag.clone(), size: item.size };
        if item.size <= longest {
            // Nothing more there than here.
            self.state.files.insert(name.to_string(), known);
            return Ok(());
        }
        if item.size > round_cap() {
            // Past what a pull brings: left to the sync app, and said (`State::left`).
            self.state.left.insert(name.to_string(), item.size);
            self.pulled.left += 1;
            return Ok(());
        }
        let base = if size(&into) >= size(&here) { into.clone() } else { here.clone() };
        let (end, last) = whole_end(&base);
        let from = if end - last > OVERLAP { end.saturating_sub(1) } else { last };
        let overlap = read_range(&base, from, end).unwrap_or_default();
        let url = self.url(name);
        let Some(mut got) = self.server.tail(&url, from)? else { return Ok(()) };
        let joins = got.start == from && from > 0;
        if joins && !got.bytes.starts_with(&overlap) {
            if got.error.is_some() && overlap.starts_with(&got.bytes) {
                // Cut within the line asked again: nothing new came.
                return Err(Stop::Failed(got.error.take().unwrap_or(SyncError::Network(String::new()))));
            }
            // Not the same there: the whole file.
            match self.server.tail(&url, 0)? {
                Some(whole) => got = whole,
                None => return Ok(()),
            }
        } else if !joins && got.start != 0 {
            return Err(Stop::Failed(SyncError::Server(format!("GET: from {}", got.start))));
        }
        let prefix = if got.start == from && from > 0 { from } else { 0 };
        self.pulled.bytes += got.bytes.len() as u64;
        // Whole lines only.
        let whole = got.bytes.iter().rposition(|b| *b == b'\n').map_or(0, |at| at + 1);
        if prefix + whole as u64 > longest {
            let temporary = crate::share::temporary(&into);
            let written = (|| -> std::io::Result<()> {
                if let Some(parent) = into.parent() {
                    crate::share::private_dirs(parent)?;
                }
                let mut out = crate::share::new_private(&temporary)?;
                if prefix > 0 {
                    std::io::copy(&mut std::fs::File::open(&base)?.take(prefix), &mut out)?;
                }
                out.write_all(&got.bytes[..whole])?;
                out.sync_all()?;
                std::fs::rename(&temporary, &into)
            })();
            if let Err(e) = written {
                let _ = std::fs::remove_file(&temporary);
                return Err(Stop::Failed(SyncError::Disk(format!("{name}: {e}"))));
            }
            self.pulled.fetched += 1;
        }
        match got.error {
            Some(e) => Err(Stop::Failed(e)),
            None => {
                self.state.files.insert(name.to_string(), known);
                Ok(())
            }
        }
    }
}

/// Where a file's last whole line ends, and where it starts: (0, 0) when it holds none.
fn whole_end(path: &Path) -> (u64, u64) {
    let Ok(mut file) = std::fs::File::open(path) else { return (0, 0) };
    let size = file.metadata().map_or(0, |m| m.len());
    let Some(end) = newline_before(&mut file, size).map(|at| at + 1) else { return (0, 0) };
    let last = newline_before(&mut file, end - 1).map_or(0, |at| at + 1);
    (end, last)
}

/// Where the last line break before `before` is, read back a piece at a time.
fn newline_before(file: &mut std::fs::File, before: u64) -> Option<u64> {
    let mut end = before;
    while end > 0 {
        let from = end.saturating_sub(64 << 10);
        let mut piece = vec![0u8; usize::try_from(end - from).ok()?];
        file.seek(SeekFrom::Start(from)).ok()?;
        file.read_exact(&mut piece).ok()?;
        if let Some(at) = piece.iter().rposition(|b| *b == b'\n') {
            return Some(from + at as u64);
        }
        end = from;
    }
    None
}

fn read_range(path: &Path, from: u64, to: u64) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut bytes = vec![0u8; usize::try_from(to.saturating_sub(from)).ok()?];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

// ---------------------------------------------------------------- kept in step by Sioul itself

/// The mode where Sioul keeps the folder in step with the server itself, no
/// sync app at all (docs/database.md, "Kept in step by Sioul itself"): the
/// folder here is its own copy (`mirror_of`), the server's the one the
/// devices share, a device beside a sync app reading the same.
pub const MIRROR: &str = "mirror";

/// Sioul's own copy of the folder in that mode: `<state>/share/mirror/`.
pub fn mirror_of(memory: &Path) -> PathBuf {
    memory.with_file_name("mirror")
}

/// A sealed file is never fetched past this: the largest note's, sealed.
const BLOB_LIMIT: u64 = crate::blobs::LARGEST + (1 << 20);

/// A folder of an account's files as Sioul keeping it itself finds it: its
/// address, where it is among the files, and its seal when it has one
/// (another device shares through it: joining), else none (a new sharing).
pub struct Opened {
    pub url: String,
    root: String,
    pub place: String,
    pub seal: Option<Vec<u8>>,
}

/// Where `place` ("Documents/Sioul") is among `login`'s files, and what it
/// holds; why not, in a code as `State::said` has them.
pub fn open(login: &Login, place: &str) -> Result<Opened, String> {
    open_with(login, place, LIMITS)
}

pub(crate) fn open_with(login: &Login, place: &str, limits: Limits) -> Result<Opened, String> {
    let host = host_of(&login.url);
    let place = place.trim().trim_matches('/').to_string();
    if place.is_empty() {
        return Err("no-place".into());
    }
    let server = Server::new(login, limits).map_err(|e| code(&e, &login.account))?;
    let root = files_root(&server, login).map_err(|e| code(&e, &host))?.ok_or_else(|| format!("no-files:{host}"))?;
    let url = format!("{root}{}/", encode_path(&place));
    let seal = server.small(&format!("{url}seal.toml")).map_err(|e| code(&e, &host))?.map(|(bytes, _)| bytes);
    Ok(Opened { url, root, place, seal })
}

/// A new sharing there: each missing folder of its place made (`MKCOL`),
/// then `folder`'s seal sent only if none is there yet (`If-None-Match: *`):
/// two devices starting at once never seal it twice ("sealed-meanwhile").
pub fn create(login: &Login, opened: &Opened, folder: &Path) -> Result<(), String> {
    create_with(login, opened, folder, LIMITS)
}

pub(crate) fn create_with(login: &Login, opened: &Opened, folder: &Path, limits: Limits) -> Result<(), String> {
    let host = host_of(&login.url);
    let failed = |e: SyncError| code(&e, &host);
    let server = Server::new(login, limits).map_err(failed)?;
    let mut at = opened.root.clone();
    for name in opened.place.split('/').filter(|n| !n.is_empty()) {
        at = format!("{at}{}/", encode_path(name));
        if !matches!(server.mkcol(&at).map_err(failed)?, 201 | 405) {
            return Err(format!("server:{host}"));
        }
    }
    let seal = seal_of(folder).ok_or_else(|| "no-seal".to_string())?;
    match server.put(&format!("{}seal.toml", opened.url), seal, None).map_err(failed)?.0 {
        200 | 201 | 204 => Ok(()),
        412 => Err("sealed-meanwhile".into()),
        _ => Err(format!("server:{host}")),
    }
}

/// From now on Sioul keeps `folder` in step with `opened` itself (`MIRROR`),
/// with `login`'s account: what the backup knew goes.
pub fn begin(memory: &Path, folder: &Path, login: &Login, opened: &Opened, now: i64) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(cache_of(memory));
    let state = State { mode: MIRROR.into(), place: opened.place.clone(), folder: shown(folder), account: login.account.clone(), url: opened.url.clone(), confirmed: now, sealed: now, ..State::default() };
    let path = state_path(memory);
    sioul_core::filelock::with_lock(&path, || state.write(memory))
}

/// One step of Sioul keeping the folder itself (`MIRROR`): with `whole`, the
/// server's folder looked through first and the other devices' newer files
/// brought into the copy here, as `pull` brings them beside a synced folder;
/// then this device's own files changed since they went up, or changed there
/// since, sent (`Puller::push`), its records first, its sealed files last;
/// then the others' sealed notes, papers and spam filter's tables brought
/// (`Puller::fetch_blobs`). A large sealed file on a slow line, going up or
/// coming down, never holds back the records: the doses' answers go first.
/// Never another device's file written; nothing here lost when the server
/// refuses. One step at a time on this device, whichever process runs it.
pub fn step(memory: &Path, folder: &Path, own: &str, login: &Login, now: i64, whole: bool) -> Pulled {
    step_with(memory, folder, own, login, now, whole, LIMITS)
}

pub(crate) fn step_with(memory: &Path, folder: &Path, own: &str, login: &Login, now: i64, whole: bool, limits: Limits) -> Pulled {
    sioul_core::filelock::with_lock(&memory.with_file_name("mirror.step"), || {
        let mut state = State::load(memory);
        let mut pulled = Pulled::default();
        if state.mode != MIRROR || !state.confirmed_for(folder) || state.account != login.account {
            pulled.problem = Some("not-confirmed".into());
            return pulled;
        }
        let host = state.host();
        if whole {
            state.tried = now;
        }
        let outcome = Server::new(login, limits).map_err(Stop::Failed).and_then(|server| {
            let mut puller = Puller { server: &server, folder, into: folder, own, state: &mut state, pulled: &mut pulled, now, own_there: BTreeMap::new(), covered: BTreeSet::new(), lost: false, blobs: Vec::new(), blobs_listed: None };
            if whole {
                puller.run()?;
            }
            // This device's own files first (its records before its sealed
            // files), the others' sealed files last: nothing large holds back
            // what the others wait for. Each said when it fails.
            let pushed = puller.push();
            let fetched = if whole { puller.fetch_blobs() } else { Ok(()) };
            pushed.and(fetched)
        });
        match outcome {
            Ok(()) => {
                if whole {
                    state.last = now;
                }
                state.pushed = now;
                state.said.clear();
            }
            Err(Stop::Unconfirmed(why)) => {
                state.confirmed = 0;
                state.said = why;
            }
            Err(Stop::Failed(e)) => state.said = code(&e, &host),
        }
        let _ = state.save(memory);
        pulled.problem = (!state.said.is_empty()).then(|| state.said.clone());
        pulled
    })
}

/// The folder a file of the folder is in, as the server's listings are kept: "" the folder itself, "devices/", "leases/health/".
fn dir_of(relative: &str) -> &str {
    relative.rfind('/').map_or("", |at| &relative[..=at])
}

/// This device's own files in the folder (`MIRROR`), by their path in it, in
/// the order they go up: its records, its notes to the others, its claims,
/// its entry (it says how far its records went), then the sealed files it
/// made (in `blobs/`, not fetched). A large sealed file on a slow line never
/// holds back the records, the doses' answers among them; a record naming a
/// sealed file not there yet is read as a sync app's late copy is: the
/// others wait for it (docs/database.md, "Coming in").
fn own_files(folder: &Path, own: &str, state: &State) -> Vec<String> {
    let mut out = own_plain(folder, own);
    out.extend(file_names(&folder.join("blobs")).into_iter().map(|n| format!("blobs/{n}")).filter(|r| !state.files.contains_key(r)));
    out
}

/// The files of a folder, by name, hidden ones left out.
fn file_names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_file())).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| !n.starts_with('.')).collect()
}

/// This device's own files in the folder but its sealed ones, in the order
/// they go up: its records, its notes to the others, its claims, its entry.
fn own_plain(folder: &Path, own: &str) -> Vec<String> {
    let names = file_names;
    let mut out: Vec<String> = Vec::new();
    let mut records: Vec<(u8, u32, String)> = names(folder)
        .into_iter()
        .filter_map(|name| match kind_of(&name) {
            Some((Kind::Round, id)) if id == own => Some((0, name.trim_end_matches(".jsonl").rsplit('-').next().and_then(|n| n.parse().ok()).unwrap_or(0), name)),
            Some((Kind::Seen, id)) if id == own => Some((1, 0, name)),
            _ => None,
        })
        .collect();
    records.sort();
    out.extend(records.into_iter().map(|(_, _, name)| name));
    let mut parts: Vec<String> = std::fs::read_dir(folder.join("leases")).into_iter().flatten().filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    parts.sort();
    out.extend(parts.into_iter().map(|part| format!("leases/{part}/{own}.lease")).filter(|claim| folder.join(claim).is_file()));
    let entry = format!("devices/{own}.device");
    if folder.join(&entry).is_file() {
        out.push(entry);
    }
    out
}

/// The highest round of this device's records in the folder here; none when it holds none.
fn own_latest(folder: &Path, own: &str) -> Option<u32> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            match kind_of(&name) {
                Some((Kind::Round, id)) if id == own => name.trim_end_matches(".jsonl").rsplit('-').next().and_then(|n| n.parse().ok()),
                _ => None,
            }
        })
        .max()
}

// ---------------------------------------------------------------- sent beside the sync app

/// A full check of what the server holds of this device's files, beside the
/// quick sends (each a few requests): every six hours, and when asked
/// ("Send everything again").
const CHECK_AGAIN: i64 = 6 * 3600;
/// A file past this size (bytes) is a large one: on a metered or slow
/// connection (`Sending::frugal`), sent at most every `PACE` seconds.
pub const LARGE_FILE: u64 = 256 << 10;
const PACE: i64 = 10 * 60;
/// Under this rate (bytes a second), measured on a file sent of 64 KiB at
/// least, the connection counts as slow: paced as a metered one.
pub const SLOW: u64 = 64 << 10;

/// How long a failed send waits before it is tried again, by how many failed
/// in a row: a minute, two, five, a quarter of an hour, half an hour, an hour.
pub fn backoff(tries: u32) -> i64 {
    [60, 120, 300, 900, 1800, 3600][tries.saturating_sub(1).min(5) as usize]
}

/// The two ways this device's files go (`send`, `send_urgent`), each with its
/// own lock and its own memory, so that a large file on a slow line, going
/// up for minutes, never holds back a text to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lane {
    /// Its records, notes, claims, entry and sealed files.
    Main,
    /// What must reach the others within minutes, in a small file of its own:
    /// its texts to send (`texts/send/<id>.jsonl`, `place_texts`).
    Urgent,
}

impl Lane {
    fn memory(self, memory: &Path) -> PathBuf {
        memory.with_file_name(match self {
            Lane::Main => "sending.toml",
            Lane::Urgent => "sending-urgent.toml",
        })
    }

    fn lock(self, memory: &Path) -> PathBuf {
        memory.with_file_name(match self {
            Lane::Main => "send.step",
            Lane::Urgent => "send.urgent",
        })
    }
}

/// What sending this device's own files beside the sync app knows (`send`):
/// `share/sending.toml` (`share/sending-urgent.toml` for the urgent lane),
/// never shared, apart from the backup's state, so that a pull and a send
/// never write over each other's.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sending {
    /// The folder there the files went to: another, and each is looked at again.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// When a send last went through whole, and was last tried (Unix seconds).
    #[serde(default)]
    pub last: i64,
    #[serde(default)]
    pub tried: i64,
    /// When the server's copies were last listed and compared with the files here (Unix seconds).
    #[serde(default)]
    pub checked: i64,
    /// Why the last send stopped, a code as `State::said` has them; "" when it went through.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub said: String,
    /// The last failure, in detail: which file, how, how long, how many of its bytes went.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
    /// The server unreachable (a connection, a certificate, a password
    /// refused): how many sends failed in a row, and none tried before
    /// `retry_at` (Unix seconds; "Send everything again" apart).
    #[serde(default)]
    pub tries: u32,
    #[serde(default)]
    pub retry_at: i64,
    /// Files that did not go (too slow, cut): how many tries in a row, and not
    /// tried again before when (`backoff`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub held: BTreeMap<String, Held>,
    /// The rate of the last file of 64 KiB or more that went, or of what went
    /// of one that failed (bytes a second); 0 unknown.
    #[serde(default)]
    pub rate: u64,
    /// When a large file was last sent or tried (Unix seconds), for the pace.
    #[serde(default)]
    pub large_at: i64,
    /// What "Send everything again" last did, in either mode.
    #[serde(default)]
    pub again: Again,
    /// This device's own files as they last went up, or were found there the same, by their path in the folder.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sent: BTreeMap<String, Sent>,
}

/// A failed send, in detail: what the panel and the log say of it.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// "timeout" (slower than the floor), "stalled" (nothing moved for half
    /// a minute), "unreachable", "tls", "login", "server", "quota", "network".
    #[serde(default)]
    pub kind: String,
    /// The file, by kind: "records", "notes", "claim", "entry", "texts", "sealed".
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub seconds: u64,
    #[serde(default)]
    pub sent: u64,
    #[serde(default)]
    pub total: u64,
    /// The error's own words, for the log.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub words: String,
    /// When (Unix seconds).
    #[serde(default)]
    pub at: i64,
}

/// A file held back after it failed: tries in a row, and when to try again (Unix seconds).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    #[serde(default)]
    pub tries: u32,
    #[serde(default)]
    pub until: i64,
}

/// What "Send everything again" did: when (Unix seconds), how many files
/// went, how many were there already as they are here, and why it stopped
/// ("" when it went through).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Again {
    #[serde(default)]
    pub at: i64,
    #[serde(default)]
    pub sent: usize,
    #[serde(default)]
    pub same: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub said: String,
}

#[cfg(test)]
fn sending_path(memory: &Path) -> PathBuf {
    Lane::Main.memory(memory)
}

impl Sending {
    pub fn load(memory: &Path) -> Sending {
        Sending::load_lane(memory, Lane::Main)
    }

    /// The urgent lane's (`send_urgent`).
    pub fn load_urgent(memory: &Path) -> Sending {
        Sending::load_lane(memory, Lane::Urgent)
    }

    fn load_lane(memory: &Path, lane: Lane) -> Sending {
        std::fs::read_to_string(lane.memory(memory)).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    fn save(&self, memory: &Path, lane: Lane) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::share::write_atomically(&lane.memory(memory), format!("# Sioul: this device's files sent to the sharing folder's server beside the sync app (docs/database.md). Never shared.\n{text}").as_bytes())
    }

    /// "Send everything again" could not start (no account for the server,
    /// the folder not found, sending switched off): why, said as a code.
    pub fn note_again(memory: &Path, now: i64, why: &str) {
        let mut sending = Sending::load(memory);
        sending.again = Again { at: now, sent: 0, same: 0, said: why.to_string() };
        let _ = sending.save(memory, Lane::Main);
    }

    /// The connection measured slow: under `SLOW` on the last large file.
    pub fn slow(&self) -> bool {
        self.rate > 0 && self.rate < SLOW
    }

    /// When the next try is due, when a send waits (Unix seconds): the server
    /// unreachable, or a file held back; 0 when nothing waits.
    pub fn next_try(&self) -> i64 {
        self.held.values().map(|h| h.until).chain([self.retry_at]).filter(|at| *at > 0).min().unwrap_or(0)
    }
}

/// This device's own files sent to the server beside the sync app that
/// carries the folder (docs/database.md, "Sent to the server too"), once the
/// backup found the folder there under the same seal. Only its own files
/// (`own_plain`, and the sealed files it made, `sealed`: their name and the
/// size it sealed, sent whole only), each over what is there as it was seen;
/// a file found there as it is here is not sent again (the sync app sent it,
/// or will find it there as its own); records only grow (never one shorter
/// than what went, or than the server's), and go in order; never another
/// device's file. Each file is given the time its size asks (`FLOOR`); one
/// that does not go is held back (`backoff`), never tried sooner, the next
/// records with it, the other files still going. The server unreachable,
/// nothing is tried before its backoff. With `frugal` (a metered connection),
/// or the connection measured slow, a large file goes at most every ten
/// minutes. With `again`, or every six hours, or the first time, the server's
/// folders are listed and every file compared: one lost there is sent again.
/// This device's records past their time and the sealed files it swept are
/// taken out there. One send at a time on this device, whichever process runs it.
#[allow(clippy::too_many_arguments)]
pub fn send(memory: &Path, folder: &Path, own: &str, login: &Login, sealed: &BTreeMap<String, u64>, now: i64, again: bool, frugal: bool) -> Pulled {
    send_lane(Lane::Main, memory, folder, own, login, sealed, now, again, frugal, SEND_LIMITS)
}

/// This device's texts to send (`texts/send/<id>.jsonl`, `place_texts`), on a
/// lane of their own: a small file, sent first and alone, never waiting
/// behind a large one going up on a slow line; Sioul keeping the folder
/// itself too (`MIRROR`).
pub fn send_urgent(memory: &Path, folder: &Path, own: &str, login: &Login, now: i64) -> Pulled {
    send_lane(Lane::Urgent, memory, folder, own, login, &BTreeMap::new(), now, false, false, SEND_LIMITS)
}

#[cfg(test)]
pub(crate) fn send_urgent_with(memory: &Path, folder: &Path, own: &str, login: &Login, now: i64, limits: Limits) -> Pulled {
    send_lane(Lane::Urgent, memory, folder, own, login, &BTreeMap::new(), now, false, false, limits)
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn send_with(memory: &Path, folder: &Path, own: &str, login: &Login, sealed: &BTreeMap<String, u64>, now: i64, again: bool, limits: Limits) -> Pulled {
    send_lane(Lane::Main, memory, folder, own, login, sealed, now, again, false, limits)
}

#[allow(clippy::too_many_arguments)]
fn send_lane(lane: Lane, memory: &Path, folder: &Path, own: &str, login: &Login, sealed: &BTreeMap<String, u64>, now: i64, again: bool, frugal: bool, limits: Limits) -> Pulled {
    sioul_core::filelock::with_lock(&lane.lock(memory), || {
        let state = State::load(memory);
        let mut pulled = Pulled::default();
        let refused = if state.mode == MIRROR && lane == Lane::Main {
            Some("mirror")
        } else if !state.confirmed_for(folder) || state.account != login.account {
            Some("not-confirmed")
        } else if state.mode != MIRROR && !state.sending() {
            Some("off")
        } else {
            None
        };
        if let Some(why) = refused {
            if again {
                Sending::note_again(memory, now, why);
            }
            pulled.problem = Some(why.into());
            return pulled;
        }
        let mut sending = Sending::load_lane(memory, lane);
        if sending.url != state.url {
            sending = Sending { url: state.url.clone(), again: sending.again.clone(), ..Sending::default() };
        }
        // This device's entry listed there by a pull after this send last went
        // through, of another size than it was sent: an older copy put back
        // there (by another device's sync app, the server's own versions).
        // Looked at again now, and sent over it, rather than at the check of
        // every six hours.
        let entry = format!("devices/{own}.device");
        if lane == Lane::Main
            && let Some((size, at)) = state.own_entry
            && at > sending.last
            && sending.sent.get(&entry).is_some_and(|sent| sent.size != size)
        {
            sending.sent.remove(&entry);
        }
        let host = state.host();
        // The server unreachable a moment ago: not tried before its backoff.
        if !again && now < sending.retry_at {
            pulled.problem = Some(format!("waiting:{host}"));
            return pulled;
        }
        sending.tried = now;
        let check = again || sending.checked == 0 || now - sending.checked >= CHECK_AGAIN;
        let frugal = frugal || sending.slow();
        let files: Vec<String> = match lane {
            Lane::Urgent => own_urgent(folder, own),
            Lane::Main => {
                let mut files = own_plain(folder, own);
                let mut names: Vec<&String> = sealed.keys().collect();
                names.sort();
                files.extend(names.into_iter().map(|name| format!("blobs/{name}")).filter(|relative| folder.join(relative).is_file()));
                files
            }
        };
        let outcome = Server::new(login, limits).map_err(Stop::Failed).and_then(|server| Sender { server: &server, folder, own, url: &state.url, sealed, sending: &mut sending, pulled: &mut pulled, now, frugal, lane }.run(&files, check, again));
        match outcome {
            Ok(()) => {
                sending.tries = 0;
                sending.retry_at = 0;
                if sending.held.is_empty() {
                    sending.last = now;
                    sending.said.clear();
                    sending.failure = None;
                } else if let Some(failure) = &sending.failure {
                    sending.said = format!("network:{host}");
                    pulled.failure = Some(failure.clone());
                }
                if check {
                    sending.checked = now;
                }
            }
            Err(Stop::Unconfirmed(why)) => sending.said = why,
            Err(Stop::Failed(e)) => {
                // Unreachable, refused, out of time: nothing more before the backoff.
                sending.tries += 1;
                sending.retry_at = now + backoff(sending.tries);
                sending.said = code(&e, &host);
                if sending.failure.as_ref().is_none_or(|f| f.at != now) {
                    sending.failure = Some(Failure { kind: kind_of_error(&e).into(), file: String::new(), words: e.to_string(), at: now, ..Failure::default() });
                }
                pulled.failure = sending.failure.clone();
            }
        }
        if again {
            sending.again = Again { at: now, sent: pulled.sent, same: pulled.same, said: sending.said.clone() };
        }
        let _ = sending.save(memory, lane);
        pulled.problem = (!sending.said.is_empty()).then(|| sending.said.clone());
        pulled
    })
}

/// A failure's kind, from its error alone (no file under way).
fn kind_of_error(e: &SyncError) -> &'static str {
    match e {
        SyncError::Login(_) | SyncError::AppPassword(_) => "login",
        SyncError::Tls(_) => "tls",
        SyncError::Server(d) if d == "quota" => "quota",
        SyncError::Server(_) => "server",
        SyncError::Disk(_) => "disk",
        SyncError::Network(d) if d == "out of time" => "out-of-time",
        _ => "unreachable",
    }
}

/// Sioul keeping the folder itself (`MIRROR`): "Send everything again". What
/// it remembers of sending this device's own files is forgotten (its sealed
/// files apart, which tell its own from the others'), and the server's
/// folders are looked through again: each own file goes over what is there
/// now, the others' newer files come. Said as `Sending::again` says it.
pub fn again_mirror(memory: &Path, folder: &Path, own: &str, login: &Login, now: i64) -> Pulled {
    again_mirror_with(memory, folder, own, login, now, LIMITS)
}

pub(crate) fn again_mirror_with(memory: &Path, folder: &Path, own: &str, login: &Login, now: i64, limits: Limits) -> Pulled {
    let forgot = sioul_core::filelock::with_lock(&memory.with_file_name("mirror.step"), || {
        let mut state = State::load(memory);
        if state.mode != MIRROR {
            return false;
        }
        state.sent.retain(|relative, _| relative.starts_with("blobs/"));
        state.folders.clear();
        state.save(memory).is_ok()
    });
    let pulled = if forgot { step_with(memory, folder, own, login, now, true, limits) } else { Pulled { problem: Some("not-confirmed".into()), ..Pulled::default() } };
    let mut sending = Sending::load(memory);
    sending.again = Again { at: now, sent: pulled.sent, same: 0, said: pulled.problem.clone().unwrap_or_default() };
    let _ = sending.save(memory, Lane::Main);
    pulled
}

/// This device's texts to send copied into the sharing folder as a small
/// file of its own, `texts/send/<id>.jsonl` (`source`: its own file of the
/// part "texts", every line sealed already): the sync app carries it, Sioul
/// sends it first (`send_urgent`), the others read it beside the records'
/// copy, so that a request reaches the phone within its quarter of an hour
/// whatever the records' size. None here, none there. Whether it changed.
pub fn place_texts(folder: &Path, own: &str, source: &Path) -> Result<bool, String> {
    let target = texts_file(folder, own);
    let Ok(bytes) = std::fs::read(source) else {
        return Ok(std::fs::remove_file(&target).is_ok());
    };
    if std::fs::read(&target).ok().as_deref() == Some(&bytes[..]) {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        crate::share::private_dirs(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    crate::share::write_atomically(&target, &bytes).map(|()| true)
}

/// Where a device's texts to send are, in the sharing folder (or a copy fetched from its server).
pub fn texts_file(folder: &Path, device: &str) -> PathBuf {
    folder.join("texts").join("send").join(format!("{device}.jsonl"))
}

/// This device's urgent files in the folder: its texts to send.
fn own_urgent(folder: &Path, own: &str) -> Vec<String> {
    let relative = format!("texts/send/{own}.jsonl");
    if folder.join(&relative).is_file() { vec![relative] } else { Vec::new() }
}

/// A file of the folder, by its kind, for what is said: never its name.
fn file_kind(relative: &str) -> &'static str {
    match kind_of(relative) {
        Some((Kind::Round, _)) => "records",
        Some((Kind::Seen, _)) => "notes",
        Some((Kind::Device, _)) => "entry",
        Some((Kind::Claim, _)) => "claim",
        Some((Kind::Texts, _)) => "texts",
        None => "sealed",
    }
}

/// What came of one file: gone or the same there, left (unchanged, paced,
/// longer there), or held back after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Went {
    Done,
    Left,
    Held,
}

/// One send beside the sync app.
struct Sender<'a> {
    server: &'a Server,
    folder: &'a Path,
    own: &'a str,
    /// The folder there, an address ending with "/".
    url: &'a str,
    sealed: &'a BTreeMap<String, u64>,
    sending: &'a mut Sending,
    pulled: &'a mut Pulled,
    now: i64,
    frugal: bool,
    lane: Lane,
}

impl Sender<'_> {
    fn url(&self, relative: &str) -> String {
        format!("{}{}", self.url, encode_path(relative))
    }

    /// `files` in their order (records first, by round, then the notes to
    /// the others, the claims, the entry, the sealed files last): nothing
    /// large holds back what the others wait for. A records file held back
    /// holds the next ones (records go in order); the other files still go.
    fn run(&mut self, files: &[String], check: bool, again: bool) -> Result<(), Stop> {
        let listed = if check { Some(self.list_own()?) } else { None };
        let mut records_held = false;
        for relative in files {
            let records = matches!(kind_of(relative), Some((Kind::Round, _)));
            if records && records_held {
                continue;
            }
            let there = listed.as_ref().map(|listed| listed.get(relative).cloned());
            let went = self.one(relative, there, again)?;
            records_held |= records && went == Went::Held;
        }
        if self.lane == Lane::Main {
            self.take_out_gone()?;
        }
        Ok(())
    }

    /// The server's copies of this device's files, by their path in the
    /// folder: the folder itself, `devices/`, each part's claims, `blobs/`
    /// (when this device sealed any), `texts/send/`. A folder not there holds none.
    fn list_own(&mut self) -> Result<BTreeMap<String, Item>, Stop> {
        let mut out = BTreeMap::new();
        let root = self.server.list(self.url).map_err(|e| match e {
            SyncError::NotFound(_) => Stop::Unconfirmed(format!("seal-gone:{}", host_of(self.url))),
            e => Stop::Failed(e),
        })?;
        self.pulled.listed += root.items.len();
        let mut dirs: Vec<String> = Vec::new();
        for (name, item) in &root.items {
            if item.dir {
                if name == "devices" || (name == "blobs" && !self.sealed.is_empty()) {
                    dirs.push(format!("{name}/"));
                }
                if name == "texts" {
                    dirs.push("texts/send/".into());
                }
                if name == "leases" {
                    match self.server.list(&self.url("leases/")) {
                        Ok(parts) => dirs.extend(parts.items.iter().filter(|(_, item)| item.dir).map(|(part, _)| format!("leases/{part}/"))),
                        Err(SyncError::NotFound(_)) => {}
                        Err(e) => return Err(Stop::Failed(e)),
                    }
                }
            } else {
                out.insert(name.clone(), item.clone());
            }
        }
        for dir in dirs {
            match self.server.list(&self.url(&dir)) {
                Ok(listing) => {
                    self.pulled.listed += listing.items.len();
                    out.extend(listing.items.into_iter().filter(|(_, item)| !item.dir).map(|(name, item)| (format!("{dir}{name}"), item)));
                }
                Err(SyncError::NotFound(_)) => {}
                Err(e) => return Err(Stop::Failed(e)),
            }
        }
        Ok(out)
    }

    /// Whether the server's copy is this file as it is here: a sealed file
    /// by its size (its name is its content's); records by their size (they
    /// only grow, one writer) and their checksum when the server keeps one;
    /// the rest by their checksum, else read there and compared.
    fn same(&self, relative: &str, there: &Item, bytes: &[u8], sha1: &str) -> Result<bool, Stop> {
        if there.dir || there.size != bytes.len() as u64 {
            return Ok(false);
        }
        if relative.starts_with("blobs/") {
            return Ok(true);
        }
        if let Some(theirs) = sha1_of(&there.checksum) {
            return Ok(theirs == sha1);
        }
        if matches!(kind_of(relative), Some((Kind::Round, _))) {
            return Ok(true);
        }
        Ok(self.server.small_up_to(&self.url(relative), TEXTS_LARGEST)?.is_some_and(|(theirs, _)| theirs == bytes))
    }

    /// One of this device's files, sent when it changed here since it went
    /// (or, `there` listed, when the server's copy is not this one), over
    /// what is there as it was seen. Refused because it changed there
    /// meanwhile (`412`): the server's copy looked at; the same as here (the
    /// sync app sent it), nothing more; else sent over it. A folder missing
    /// there (`409`), made first. Too slow or cut: held back (`backoff`).
    fn one(&mut self, relative: &str, listed: Option<Option<Item>>, again: bool) -> Result<Went, Stop> {
        let path = self.folder.join(relative);
        let Ok(meta) = std::fs::metadata(&path) else { return Ok(Went::Left) };
        let (size, modified) = (meta.len(), crate::share::modified_ns(&meta));
        let blob = relative.strip_prefix("blobs/");
        // A sealed file goes whole, as this device sealed it, or not at all.
        if blob.is_some_and(|name| self.sealed.get(name) != Some(&size)) {
            return Ok(Went::Left);
        }
        let records = matches!(kind_of(relative), Some((Kind::Round, _)));
        let sent = self.sending.sent.get(relative).cloned();
        let same_here = sent.as_ref().is_some_and(|s| s.size == size && s.modified == modified);
        match &listed {
            None if same_here => return Ok(Went::Left),
            Some(there) if same_here && !again && there.as_ref().is_some_and(|t| t.etag == sent.as_ref().map_or("", |s| s.etag.as_str())) => return Ok(Went::Left),
            _ => {}
        }
        // Held back after a failure: never tried before its backoff.
        if !again && self.sending.held.get(relative).is_some_and(|held| self.now < held.until) {
            return Ok(Went::Held);
        }
        // Records only grow: one cut here (a sync app put an older copy back)
        // is never sent over a longer one; the exchange opens a new round.
        if records && sent.as_ref().is_some_and(|s| size < s.size) {
            self.pulled.longer += 1;
            return Ok(Went::Left);
        }
        // A large file on a metered or slow connection: at most every ten minutes.
        let large = size > LARGE_FILE;
        if large && self.frugal && !again && self.now - self.sending.large_at < PACE {
            self.pulled.paced += 1;
            return Ok(if records { Went::Held } else { Went::Left });
        }
        let bytes = std::fs::read(&path).map_err(|e| Stop::Failed(SyncError::Disk(format!("{relative}: {e}"))))?;
        // Read while it grew: what was read is what goes, said as it is.
        let size = bytes.len() as u64;
        let sha1 = sha1_hex(&bytes);
        let url = self.url(relative);
        // What is there: as listed; else, never sent or its ETag unsaid, asked.
        let mut there: Option<Option<Item>> = listed;
        if there.is_none() && sent.as_ref().is_none_or(|s| s.etag.is_empty()) {
            there = Some(self.server.stat(&url)?);
        }
        let modified_s = modified / 1_000_000_000;
        for attempt in 0..3 {
            if let Some(Some(item)) = &there {
                if self.same(relative, item, &bytes, &sha1)? {
                    self.sending.sent.insert(relative.to_string(), Sent { size, modified, etag: item.etag.clone() });
                    self.sending.held.remove(relative);
                    self.pulled.same += 1;
                    return Ok(Went::Done);
                }
                // Not the same: said by kind and size, for the log (nothing of its name or content).
                if self.pulled.differ.len() < 8 {
                    self.pulled.differ.push(format!("{} {size} here, {} there", file_kind(relative), item.size));
                }
                // Longer there than here: never cut. Remembered at the server's
                // size, so that what is appended here later, while still
                // shorter, is never sent over it either.
                if records && item.size > size {
                    self.sending.sent.insert(relative.to_string(), Sent { size: item.size, modified, etag: item.etag.clone() });
                    self.pulled.longer += 1;
                    return Ok(Went::Left);
                }
            }
            let seen: Option<String> = match &there {
                Some(Some(item)) => Some(item.etag.clone()),
                Some(None) => None,
                None => sent.as_ref().map(|s| s.etag.clone()),
            };
            if large {
                self.sending.large_at = self.now;
            }
            let started = Instant::now();
            let (status, etag) = match self.server.put_file(&url, bytes.clone(), seen.as_deref(), modified_s, &sha1) {
                Ok(answer) => answer,
                Err(failed) => return self.failed(relative, failed),
            };
            match status {
                200 | 201 | 204 => {
                    self.sending.sent.insert(relative.to_string(), Sent { size, modified, etag: etag.unwrap_or_default() });
                    self.sending.held.remove(relative);
                    self.pulled.sent += 1;
                    self.pulled.bytes += size;
                    if size >= 64 << 10 {
                        self.sending.rate = size * 1000 / (started.elapsed().as_millis() as u64).max(1);
                    }
                    return Ok(Went::Done);
                }
                // Changed there meanwhile, or gone: looked at again.
                412 | 404 => there = Some(self.server.stat(&url)?),
                409 if attempt == 0 => self.make_folders(relative)?,
                507 => return Err(Stop::Failed(SyncError::Server("quota".into()))),
                status => return Err(Stop::Failed(SyncError::Server(format!("PUT: {status}")))),
            }
        }
        Err(Stop::Failed(SyncError::Server("PUT: 412".into())))
    }

    /// A file that did not go, said in detail (`Sending::failure`). Too slow
    /// or cut on its way (the connection made, its bytes moving): that file
    /// held back (`backoff`), the others still going. Else (the server
    /// unreachable, refused, out of time): the send stops.
    fn failed(&mut self, relative: &str, failed: PutFailed) -> Result<Went, Stop> {
        self.sending.failure = Some(Failure { kind: failed.kind.into(), file: file_kind(relative).into(), seconds: failed.seconds, sent: failed.sent, total: failed.total, words: failed.words.clone(), at: self.now });
        if failed.sent > 0 && failed.seconds > 0 {
            self.sending.rate = failed.sent / failed.seconds;
        }
        if matches!(failed.kind, "timeout" | "stalled" | "network") && failed.sent > 0 {
            let held = self.sending.held.entry(relative.to_string()).or_default();
            held.tries += 1;
            held.until = self.now + backoff(held.tries);
            self.pulled.held += 1;
            return Ok(Went::Held);
        }
        Err(Stop::Failed(failed.error))
    }

    /// The folders a file's path goes through, made there (`MKCOL`) when missing.
    fn make_folders(&mut self, relative: &str) -> Result<(), Stop> {
        let mut at = String::new();
        let names: Vec<&str> = relative.split('/').collect();
        for name in &names[..names.len().saturating_sub(1)] {
            at = format!("{at}{name}/");
            match self.server.mkcol(&self.url(&at))? {
                201 | 405 => {}
                status => return Err(Stop::Failed(SyncError::Server(format!("MKCOL: {status}")))),
            }
        }
        Ok(())
    }

    /// What this device sent and let go here, taken out there as it was
    /// sent (`If-Match`): its records past their time (removed here once
    /// every device read past them), never its last round, and nothing while
    /// none of its records is here; the sealed files it swept. Gone there
    /// already, or changed there since: forgotten all the same. Another file
    /// gone here (a claim of a part no longer kept) is forgotten, left there.
    fn take_out_gone(&mut self) -> Result<(), Stop> {
        let latest = own_latest(self.folder, self.own);
        let gone: Vec<(String, String)> = self.sending.sent.iter().filter(|(relative, _)| !self.folder.join(relative).exists()).map(|(relative, sent)| (relative.clone(), sent.etag.clone())).collect();
        for (relative, etag) in gone {
            let round = relative.trim_end_matches(".jsonl").rsplit('-').next().and_then(|n| n.parse::<u32>().ok());
            let past = matches!(kind_of(&relative), Some((Kind::Round, _))) && round.zip(latest).is_some_and(|(round, latest)| round < latest);
            let swept = relative.strip_prefix("blobs/").is_some_and(|name| !self.sealed.contains_key(name));
            if past || swept {
                match self.server.delete(&self.url(&relative), Some(&etag).filter(|e| !e.is_empty()).map(String::as_str))? {
                    200 | 204 | 404 | 412 => {}
                    status => return Err(Stop::Failed(SyncError::Server(format!("DELETE: {status}")))),
                }
            }
            if past || swept || !matches!(kind_of(&relative), Some((Kind::Round, _))) {
                self.sending.sent.remove(&relative);
                self.sending.held.remove(&relative);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A Nextcloud in a test: an account's files, a folder on the disk,
    //! served over WebDAV on 127.0.0.1 (plain HTTP, which only a test build
    //! reaches: `dav::allowed`), a request and a thread per connection; faults
    //! to inject, and every request seen.

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    pub(crate) const USER: &str = "jane";
    pub(crate) const PASSWORD: &str = "correct horse battery staple";
    const FILES: &str = "/remote.php/dav/files/jane";

    /// How the server misbehaves, for one request.
    #[derive(Debug, Clone, Copy)]
    pub(crate) enum Fault {
        /// Answers this status, doing nothing.
        Status(u16),
        /// Waits this long, then answers.
        Late(Duration),
        /// Says the whole answer's length, sends this many bytes of it, and closes.
        Cut(usize),
        /// Closes without a word.
        Drop,
    }

    /// A request, as read: its path decoded.
    pub(crate) struct Request {
        pub method: String,
        pub path: String,
        headers: Vec<(String, String)>,
        pub body: Vec<u8>,
    }

    impl Request {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
        }
    }

    type Faults = Vec<Box<dyn FnMut(&Request) -> Option<Fault> + Send>>;
    pub(crate) type Reply = (u16, Vec<(&'static str, String)>, Vec<u8>);

    pub(crate) struct Fake {
        /// The account's files: `<files>/<path>` is `/remote.php/dav/files/jane/<path>`.
        pub(crate) files: PathBuf,
        pub(crate) base: String,
        /// The checksums files were sent with (`OC-Checksum`), as Nextcloud
        /// keeps them: by file, with its ETag then; a file changed since by
        /// other hands (a test's own copy) has none.
        sums: Mutex<std::collections::BTreeMap<PathBuf, (String, String)>>,
        /// An uplink this slow (bytes a second) for what is sent to it; none: as fast as it comes.
        pub(crate) throttle: Mutex<Option<u64>>,
        /// The bytes of every `PUT` it took, whole or not: what a sending costs.
        pub(crate) received: std::sync::atomic::AtomicU64,
        faults: Mutex<Faults>,
        seen: Mutex<Vec<String>>,
        /// Serving one request at a time: a listing never sees a file half written by a test's own copy.
        pub(crate) turn: Mutex<()>,
    }

    impl Fake {
        pub(crate) fn start(files: &Path) -> Arc<Fake> {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
            let fake = Arc::new(Fake {
                files: files.to_path_buf(),
                base,
                sums: Mutex::new(std::collections::BTreeMap::new()),
                throttle: Mutex::new(None),
                received: std::sync::atomic::AtomicU64::new(0),
                faults: Mutex::new(Vec::new()),
                seen: Mutex::new(Vec::new()),
                turn: Mutex::new(()),
            });
            let serving = Arc::clone(&fake);
            std::thread::spawn(move || {
                for stream in listener.incoming().filter_map(Result::ok) {
                    let fake = Arc::clone(&serving);
                    std::thread::spawn(move || fake.exchange(stream));
                }
            });
            fake
        }

        /// The account, as a contacts-and-calendars account found it: at its calendars' address.
        pub(crate) fn login(&self) -> super::Login {
            super::Login { account: "cloud".into(), url: format!("{}/remote.php/dav/calendars/{USER}/", self.base), user: USER.into(), password: Some(PASSWORD.into()) }
        }

        /// A folder of the account's files, as an address ending with "/".
        pub(crate) fn folder_url(&self, path: &str) -> String {
            format!("{}{FILES}/{}/", self.base, path.trim_matches('/'))
        }

        /// A fault, asked of each request until one says one.
        pub(crate) fn fault(&self, fault: impl FnMut(&Request) -> Option<Fault> + Send + 'static) {
            self.faults.lock().unwrap().push(Box::new(fault));
        }

        pub(crate) fn clear_faults(&self) {
            self.faults.lock().unwrap().clear();
        }

        /// The requests seen, in order: "GET /remote.php/dav/files/jane/Documents/Sioul/seal.toml 200".
        pub(crate) fn seen(&self) -> Vec<String> {
            self.seen.lock().unwrap().clone()
        }

        pub(crate) fn forget_seen(&self) {
            self.seen.lock().unwrap().clear();
        }

        fn exchange(&self, mut stream: TcpStream) {
            let throttle = *self.throttle.lock().unwrap();
            let Some(request) = read(&stream, throttle, &self.received) else { return };
            let fault = self.faults.lock().unwrap().iter_mut().find_map(|f| f(&request));
            let (status, headers, body) = match fault {
                Some(Fault::Drop) => {
                    self.note(&request, "dropped");
                    return;
                }
                Some(Fault::Status(status)) => (status, Vec::new(), Vec::new()),
                Some(Fault::Late(wait)) => {
                    std::thread::sleep(wait);
                    self.answer(&request)
                }
                _ => self.answer(&request),
            };
            self.note(&request, &status.to_string());
            let mut head = format!("HTTP/1.1 {status} Fake\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
            for (name, value) in &headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str("\r\n");
            if stream.write_all(head.as_bytes()).is_err() {
                return;
            }
            let sent = match fault {
                Some(Fault::Cut(n)) => &body[..n.min(body.len())],
                _ => &body[..],
            };
            let _ = stream.write_all(sent);
            let _ = stream.flush();
        }

        /// A file's checksums as kept, while it is as it was sent.
        fn sum_of(&self, path: &Path) -> Option<String> {
            let sums = self.sums.lock().unwrap();
            sums.get(path).filter(|(etag, _)| *etag == etag_of(path)).map(|(_, sum)| sum.clone())
        }

        fn propfind(&self, request: &Request, relative: &str, path: &Path) -> Reply {
            let Ok(meta) = std::fs::metadata(path) else { return (404, Vec::new(), Vec::new()) };
            let mut out = entry(&href(relative, meta.is_dir()), meta.is_dir(), &etag_of(path), meta.len(), self.sum_of(path));
            if meta.is_dir() && request.header("Depth") != Some("0") {
                let mut children: Vec<_> = std::fs::read_dir(path).into_iter().flatten().filter_map(Result::ok).collect();
                children.sort_by_key(|e| e.file_name());
                for child in children {
                    let name = child.file_name().to_string_lossy().to_string();
                    let inside = if relative.is_empty() { name } else { format!("{relative}/{name}") };
                    let dir = child.file_type().is_ok_and(|t| t.is_dir());
                    out += &entry(&href(&inside, dir), dir, &etag_of(&child.path()), child.metadata().map_or(0, |m| m.len()), self.sum_of(&child.path()));
                }
            }
            (207, vec![("Content-Type", "application/xml; charset=utf-8".into())], multistatus(&out))
        }

        /// A file written whole (`PUT`), as Nextcloud does: its folder there,
        /// the precondition met (`If-None-Match: *` none there, `If-Match` its
        /// ETag); dated as `X-OC-Mtime` says, its `OC-Checksum` kept.
        fn put(&self, request: &Request, path: &Path) -> Reply {
            let reply = put(request, path);
            if matches!(reply.0, 201 | 204) {
                if let Some(seconds) = request.header("X-OC-Mtime").and_then(|t| t.parse::<u64>().ok())
                    && let Ok(file) = std::fs::File::options().write(true).open(path)
                {
                    let _ = file.set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
                }
                let mut sums = self.sums.lock().unwrap();
                match request.header("OC-Checksum") {
                    Some(sum) => sums.insert(path.to_path_buf(), (etag_of(path), sum.to_string())),
                    None => sums.remove(path),
                };
            }
            reply
        }

        fn note(&self, request: &Request, what: &str) {
            self.seen.lock().unwrap().push(format!("{} {} {what}", request.method, request.path));
        }

        pub(crate) fn answer(&self, request: &Request) -> Reply {
            let _turn = self.turn.lock().unwrap();
            let expected = format!("Basic {}", sioul_core::lines::base64_encode(format!("{USER}:{PASSWORD}").as_bytes()));
            if request.header("Authorization") != Some(expected.as_str()) {
                return (401, Vec::new(), Vec::new());
            }
            if request.method == "PROPFIND" && request.path == "/remote.php/dav" {
                let principal = format!("<d:response><d:href>/remote.php/dav/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>/remote.php/dav/principals/users/{USER}/</d:href></d:current-user-principal></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>");
                return (207, vec![("Content-Type", "application/xml".into())], multistatus(&principal));
            }
            let Some(relative) = request.path.strip_prefix(FILES).map(|r| r.trim_start_matches('/').to_string()) else { return (404, Vec::new(), Vec::new()) };
            let path = if relative.is_empty() { self.files.clone() } else { self.files.join(&relative) };
            match request.method.as_str() {
                "PROPFIND" => self.propfind(request, &relative, &path),
                "GET" => get(request, &path),
                "PUT" => self.put(request, &path),
                "MKCOL" => mkcol(&path),
                "DELETE" => delete(request, &path),
                _ => (405, Vec::new(), Vec::new()),
            }
        }
    }

    /// A request as it comes, its body at `throttle` bytes a second at most (a slow uplink).
    fn read(stream: &TcpStream, throttle: Option<u64>, received: &std::sync::atomic::AtomicU64) -> Option<Request> {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let mut words = line.split_whitespace();
        let (method, raw) = (words.next()?.to_string(), words.next()?.to_string());
        let mut headers: Vec<(String, String)> = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).ok()?;
            let Some((name, value)) = line.trim_end().split_once(':') else { break };
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
        let length = headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
        let mut body = vec![0; length];
        let piece = throttle.map_or(length.max(1), |rate| usize::try_from(rate / 20).unwrap_or(usize::MAX).max(1));
        let mut at = 0;
        while at < length {
            let end = (at + piece).min(length);
            let read = reader.read_exact(&mut body[at..end]);
            if method == "PUT" {
                received.fetch_add((end - at) as u64, std::sync::atomic::Ordering::Relaxed);
            }
            read.ok()?;
            at = end;
            if throttle.is_some() && at < length {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Some(Request { method, path: crate::dav::path_key(&raw), headers, body })
    }

    pub(crate) fn multistatus(responses: &str) -> Vec<u8> {
        format!(r#"<?xml version="1.0" encoding="utf-8"?><d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">{responses}</d:multistatus>"#).into_bytes()
    }

    /// A file's ETag, from its content; a folder's, from all it holds, as Nextcloud's change with anything below them.
    pub(crate) fn etag_of(path: &Path) -> String {
        let mut hasher = DefaultHasher::new();
        if path.is_dir() {
            let mut names: Vec<_> = std::fs::read_dir(path).into_iter().flatten().filter_map(Result::ok).map(|e| e.file_name()).collect();
            names.sort();
            for name in names {
                name.hash(&mut hasher);
                etag_of(&path.join(&name)).hash(&mut hasher);
            }
        } else {
            std::fs::read(path).unwrap_or_default().hash(&mut hasher);
            GENERATIONS.lock().unwrap().get(path).copied().unwrap_or(0).hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }

    /// How many times each file was written whole there: Nextcloud gives a
    /// file a new ETag at each `PUT`, the same bytes sent again included.
    static GENERATIONS: Mutex<std::collections::BTreeMap<PathBuf, u64>> = Mutex::new(std::collections::BTreeMap::new());

    /// A file written there anew (a `PUT`, or a test's sync app sending it): a new ETag, whatever it holds.
    pub(crate) fn bump(path: &Path) {
        *GENERATIONS.lock().unwrap().entry(path.to_path_buf()).or_insert(0) += 1;
    }

    fn entry(href: &str, dir: bool, etag: &str, size: u64, sum: Option<String>) -> String {
        let kind = if dir { "<d:resourcetype><d:collection/></d:resourcetype>" } else { "<d:resourcetype/>" };
        let length = if dir { String::new() } else { format!("<d:getcontentlength>{size}</d:getcontentlength>") };
        let sums = sum.map(|sum| format!("<oc:checksums><oc:checksum>{sum}</oc:checksum></oc:checksums>")).unwrap_or_default();
        format!("<d:response><d:href>{href}</d:href><d:propstat><d:prop>{kind}<d:getetag>&quot;{etag}&quot;</d:getetag>{length}{sums}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>")
    }

    pub(crate) fn href(relative: &str, dir: bool) -> String {
        let path = super::encode_path(relative);
        match (relative.is_empty(), dir) {
            (true, _) => format!("{FILES}/"),
            (false, true) => format!("{FILES}/{path}/"),
            (false, false) => format!("{FILES}/{path}"),
        }
    }

    /// A file written whole (`PUT`), as Nextcloud does: its folder there, the
    /// precondition met (`If-None-Match: *` none there, `If-Match` its ETag).
    fn put(request: &Request, path: &Path) -> Reply {
        if !path.parent().is_some_and(Path::is_dir) {
            return (409, Vec::new(), Vec::new());
        }
        let reply = put_once(request, path);
        if matches!(reply.0, 201 | 204) {
            bump(path);
            return (reply.0, vec![("ETag", format!("\"{}\"", etag_of(path)))], Vec::new());
        }
        reply
    }

    fn put_once(request: &Request, path: &Path) -> Reply {
        let current = path.is_file().then(|| format!("\"{}\"", etag_of(path)));
        let refused = match (request.header("If-None-Match"), request.header("If-Match")) {
            (Some("*"), _) => current.is_some(),
            (_, Some(wanted)) => current.as_deref() != Some(wanted),
            _ => false,
        };
        if refused {
            return (412, Vec::new(), Vec::new());
        }
        std::fs::write(path, &request.body).unwrap();
        (if current.is_some() { 204 } else { 201 }, vec![("ETag", format!("\"{}\"", etag_of(path)))], Vec::new())
    }

    fn mkcol(path: &Path) -> Reply {
        if path.exists() {
            return (405, Vec::new(), Vec::new());
        }
        if !path.parent().is_some_and(Path::is_dir) {
            return (409, Vec::new(), Vec::new());
        }
        std::fs::create_dir(path).unwrap();
        (201, Vec::new(), Vec::new())
    }

    fn delete(request: &Request, path: &Path) -> Reply {
        if !path.exists() {
            return (404, Vec::new(), Vec::new());
        }
        if request.header("If-Match").is_some_and(|wanted| format!("\"{}\"", etag_of(path)) != wanted) {
            return (412, Vec::new(), Vec::new());
        }
        if path.is_dir() {
            std::fs::remove_dir_all(path).unwrap();
        } else {
            std::fs::remove_file(path).unwrap();
        }
        (204, Vec::new(), Vec::new())
    }

    fn get(request: &Request, path: &Path) -> Reply {
        let Ok(bytes) = std::fs::read(path) else { return (404, Vec::new(), Vec::new()) };
        let etag = format!("\"{}\"", etag_of(path));
        let from = request.header("Range").and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.strip_suffix('-')).and_then(|s| s.parse::<usize>().ok());
        match from {
            Some(from) if from >= bytes.len() => (416, vec![("Content-Range", format!("bytes */{}", bytes.len()))], Vec::new()),
            Some(from) => (206, vec![("ETag", etag), ("Content-Range", format!("bytes {from}-{}/{}", bytes.len() - 1, bytes.len()))], bytes[from..].to_vec()),
            None => (200, vec![("ETag", etag)], bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{Fake, Fault, PASSWORD};
    use super::*;
    use crate::devices::{COMPUTER, Entry, PHONE};
    use crate::share::{Outcome, Roots, Sharing, Store};
    use sioul_core::config::Config;
    use sioul_core::everywhere::{Press, Switch, change_own};
    use sioul_core::health::{Doubt, Peer, doubts_now};
    use std::sync::Arc;

    const KEY: [u8; 32] = [11u8; 32];
    const SEAL: &str = "# Sioul: how your devices' shared records are sealed (docs/database.md).\nversion = 1\nsalt = \"AAAAAAAAAAAAAAAAAAAAAA==\"\nmemory_kib = 65536\npasses = 3\ncheck = \"one\"\n";
    const OTHER_SEAL: &str = "# Sioul: how your devices' shared records are sealed (docs/database.md).\nversion = 1\nsalt = \"BBBBBBBBBBBBBBBBBBBBBB==\"\nmemory_kib = 65536\npasses = 3\ncheck = \"two\"\n";
    /// The network's limits, short: a test does not wait ten seconds.
    const TEST: Limits = Limits { wait: Duration::from_millis(700), small: Duration::from_secs(3), large: Duration::from_secs(5), pull: Duration::from_secs(30), blob: Duration::from_secs(5), floor: 1 << 30, stall: Duration::from_millis(700) };
    thread_local! {
        /// The longest round a pull brings, in this test's thread (`round_cap`).
        pub(super) static ROUND_CAP: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
    }
    const DUE: i64 = 1_800_000_000;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-remote-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A server whose principal it names on another server: the files are
    /// not looked for there, and nothing is asked there with the login.
    #[test]
    fn the_files_stay_on_the_account_s_server() {
        use crate::dav::stand_in::{Reply, Request, Server as StandIn, serve};
        /// Notes what it is asked, and with what login.
        struct Noted(std::sync::Mutex<Vec<String>>);
        impl StandIn for Noted {
            fn misbehaves(&self, _: &Request) -> Option<crate::dav::stand_in::Fault> {
                None
            }
            fn answer(&self, request: &Request) -> Reply {
                self.0.lock().unwrap().push(format!("{} {} {}", request.method, request.path, request.header("Authorization").unwrap_or("")));
                Reply::new(404, "")
            }
        }
        /// Names its principal at the address it is given.
        struct Pointing(String);
        impl StandIn for Pointing {
            fn misbehaves(&self, _: &Request) -> Option<crate::dav::stand_in::Fault> {
                None
            }
            fn answer(&self, _: &Request) -> Reply {
                Reply::new(207, format!(r#"<?xml version="1.0" encoding="utf-8"?><d:multistatus xmlns:d="DAV:"><d:response><d:href>/remote.php/dav/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>{}</d:href></d:current-user-principal></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#, self.0))
            }
        }
        let noted = Arc::new(Noted(std::sync::Mutex::new(Vec::new())));
        let there = serve(noted.clone());
        let here = serve(Arc::new(Pointing(format!("{there}/remote.php/dav/principals/users/jane/"))));
        let login = Login { account: "cloud".into(), url: format!("{here}/remote.php/dav/calendars/jane/"), user: "jane".into(), password: Some("secret".into()) };
        let found = files_root(&Server::new(&login, TEST).unwrap(), &login);
        assert!(matches!(&found, Err(SyncError::Elsewhere(root)) if root.starts_with(&there)), "{found:?}");
        assert!(noted.0.lock().unwrap().is_empty());
        // Named on its own server, as Nextcloud does: the files are there.
        let own = serve(Arc::new(Pointing("/remote.php/dav/principals/users/jane/".into())));
        let login = Login { url: format!("{own}/remote.php/dav/calendars/jane/"), ..login };
        assert_eq!(files_root(&Server::new(&login, TEST).unwrap(), &login).unwrap(), Some(format!("{own}/remote.php/dav/files/jane/")));
    }

    fn put(dir: &Path, relative: &str, text: &str) {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// The files under a folder, hidden ones left out, by their path in it.
    fn walk(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(at) = stack.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().filter_map(Result::ok) {
                let path = entry.path();
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let relative = path.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                    out.insert(relative, std::fs::read(&path).unwrap());
                }
            }
        }
        out
    }

    fn copy_dir(from: &Path, to: &Path) {
        for (relative, bytes) in walk(from) {
            put_bytes(to, &relative, &bytes);
        }
    }

    fn put_bytes(dir: &Path, relative: &str, bytes: &[u8]) {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    /// A device of its own: its folders, its id, its memory.
    struct Device {
        roots: Roots,
        id: String,
        memory: PathBuf,
        kind: &'static str,
    }

    impl Device {
        fn new(base: &Path, name: &str, kind: &'static str) -> Device {
            let root = base.join(name);
            let roots = Roots { config: root.join("config"), data: root.join("data"), state: root.join("state") };
            for dir in [&roots.config, &roots.data, &roots.state] {
                std::fs::create_dir_all(dir).unwrap();
            }
            let memory = roots.state.join("share").join("memory.json");
            Device { roots, id: uuid::Uuid::new_v4().to_string(), memory, kind }
        }

        fn stores(&self) -> Vec<Store> {
            crate::share::stores(&Config::default(), &self.roots)
        }

        /// An exchange (seconds), then its entry says what it read and wrote.
        fn exchange(&self, folder: &Path, now: i64) -> Outcome {
            let outcome = crate::share::exchange(&Sharing { folder, computer: &self.id, key: &KEY, memory: &self.memory, files: false, hurry: None }, &self.stores(), now * 1000).unwrap();
            crate::devices::change(&crate::devices::own_path(&self.roots.state), Some((folder, &KEY)), |e| e.exported(now * 1000, outcome.wrote, now)).unwrap();
            outcome
        }

        fn session(&self, folder: &Path, f: impl FnOnce(&mut Entry)) {
            let (kind, id) = (self.kind, self.id.clone());
            crate::devices::change(&crate::devices::own_path(&self.roots.state), Some((folder, &KEY)), |e| {
                e.id = id;
                e.kind = kind.into();
                e.doses = true;
                f(e);
            })
            .unwrap();
        }

        fn switch(&self) -> PathBuf {
            self.roots.state.join(sioul_core::everywhere::SWITCH_FILE)
        }

        /// Do-not-disturb pressed here (seconds).
        fn press(&self, on: bool, at: i64) {
            change_own(&self.switch(), &self.id, |s| {
                s.press(&self.id, on, 0, at * 1000);
                true
            })
            .unwrap();
        }

        fn dnd(&self) -> Option<Press> {
            Switch::read(&self.switch()).unwrap().latest()
        }

        fn write(&self, relative: &str, text: &str) {
            put(&self.roots.state, relative, text);
        }

        fn read(&self, relative: &str) -> String {
            std::fs::read_to_string(self.roots.state.join(relative)).unwrap_or_default()
        }

        /// What this device knows of `other`, as the window's `know` builds it.
        fn sees(&self, folder: &Path, other: &Device, now: i64) -> Peer {
            let entry = crate::devices::all(folder, &KEY).0.into_iter().find(|e| e.id == other.id).expect("its entry");
            let heard = crate::share::heard(&self.memory, &self.id);
            Peer { id: other.id.clone(), name: entry.name.clone(), said: Some(entry.said()), complete: entry.wrote.is_none_or(|wrote| heard.complete(&other.id, wrote)), seen: now, heard: entry.exported, ..Peer::default() }
        }

        /// What it keeps of its own, as files: the shared ones, written by the exchanges (not its memory).
        fn kept(&self) -> BTreeMap<String, Vec<u8>> {
            let mut out = BTreeMap::new();
            for (root, dir) in [("config", &self.roots.config), ("data", &self.roots.data), ("state", &self.roots.state)] {
                for (relative, bytes) in walk(dir) {
                    if !relative.starts_with("share/") {
                        out.insert(format!("{root}/{relative}"), bytes);
                    }
                }
            }
            out
        }
    }

    /// A desk and a phone sharing through a Nextcloud: the desk's folder is the
    /// server's (its client keeps the two equal); the phone's sync app carried
    /// the folder down once, then only carries the phone's own files up (eDrive
    /// tonight, 6 October 2026).
    struct World {
        base: PathBuf,
        fake: Arc<Fake>,
        server: PathBuf,
        phone_folder: PathBuf,
        desk: Device,
        phone: Device,
    }

    impl World {
        fn new(name: &str) -> World {
            let base = scratch(name);
            let files = base.join("cloud");
            let server = files.join("Documents").join("Sioul");
            let phone_folder = base.join("phone").join("storage").join("Documents").join("Sioul");
            put(&server, "seal.toml", SEAL);
            put(&phone_folder, "seal.toml", SEAL);
            let fake = Fake::start(&files);
            let desk = Device::new(&base, "desk", COMPUTER);
            let phone = Device::new(&base, "phone-state", PHONE);
            let world = World { base, fake, server, phone_folder, desk, phone };
            world.desk.session(&world.server, |e| e.start(DUE - 7_200));
            world.desk.exchange(&world.server, DUE - 7_100);
            world.down();
            world.phone.session(&world.phone_folder, |e| e.start(DUE - 7_050));
            world.phone.exchange(&world.phone_folder, DUE - 7_000);
            world.up();
            world.desk.exchange(&world.server, DUE - 6_900);
            world.down();
            world
        }

        /// The phone's sync app carries its own files up.
        fn up(&self) {
            let _turn = self.fake.turn.lock().unwrap();
            for (relative, bytes) in walk(&self.phone_folder).into_iter().filter(|(r, _)| r.contains(&self.phone.id)) {
                put_bytes(&self.server, &relative, &bytes);
            }
        }

        /// The phone's sync app brings the others' files down, as it should.
        fn down(&self) {
            for (relative, bytes) in walk(&self.server).into_iter().filter(|(r, _)| !r.contains(&self.phone.id)) {
                put_bytes(&self.phone_folder, &relative, &bytes);
            }
        }

        fn find(&self, now: i64) -> State {
            find_with(&self.phone.memory, &self.phone_folder, &[self.fake.login()], &places(&self.phone_folder, &[]), now, TEST)
        }

        fn pull(&self, now: i64) -> Pulled {
            pull_with(&self.phone.memory, &self.phone_folder, &self.phone.id, &self.fake.login(), now, TEST)
        }

        fn cache(&self) -> PathBuf {
            cache_of(&self.phone.memory)
        }

        /// The desk's last records' file, by its name in the folder.
        fn desk_round(&self) -> String {
            let mut names: Vec<String> = walk(&self.server).into_keys().filter(|n| n.starts_with(&self.desk.id) && n.ends_with(".jsonl")).collect();
            names.sort_by_key(|n| n.trim_end_matches(".jsonl").rsplit('-').next().unwrap().parse::<u32>().unwrap());
            names.pop().unwrap()
        }

        /// The phone as it is now, aside, to put back (`restore`): the same device run twice.
        fn snapshot(&self) -> PathBuf {
            let aside = self.base.join("aside");
            let _ = std::fs::remove_dir_all(&aside);
            copy_dir(&self.base.join("phone-state"), &aside.join("phone-state"));
            copy_dir(&self.phone_folder, &aside.join("folder"));
            aside
        }

        fn restore(&self, aside: &Path) {
            let _ = std::fs::remove_dir_all(self.base.join("phone-state"));
            let _ = std::fs::remove_dir_all(&self.phone_folder);
            copy_dir(&aside.join("phone-state"), &self.base.join("phone-state"));
            copy_dir(&aside.join("folder"), &self.phone_folder);
        }

        /// The other devices' files in the phone's synced folder.
        fn others_here(&self) -> BTreeMap<String, Vec<u8>> {
            walk(&self.phone_folder).into_iter().filter(|(r, _)| !r.contains(&self.phone.id)).collect()
        }
    }

    impl Drop for World {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    /// Only once, then never again.
    fn once(matches: impl Fn(&super::fake::Request) -> bool + Send + 'static, fault: Fault) -> impl FnMut(&super::fake::Request) -> Option<Fault> + Send + 'static {
        let mut done = false;
        move |request| {
            if done || !matches(request) {
                return None;
            }
            done = true;
            Some(fault)
        }
    }

    #[test]
    fn a_stale_folder_is_caught_up_from_the_server() {
        let w = World::new("stale");
        // The dose falls due; the desk, in use, turns do-not-disturb on and
        // shares after it. Its client carries it to the server at once; the
        // phone's sync app carries the phone's files up, and brings nothing down.
        w.desk.press(true, DUE + 30);
        w.desk.exchange(&w.server, DUE + 150);
        w.up();
        let alone = w.phone.exchange(&w.phone_folder, DUE + 170);
        assert!(alone.problems.is_empty(), "{:?}", alone.problems);
        assert!(w.phone.dnd().is_none_or(|p| !p.on), "nothing came yet");
        let stale = w.phone.sees(&w.phone_folder, &w.desk, DUE + 180);
        assert!(matches!(doubts_now(DUE, DUE + 180, None, &[stale]).as_slice(), [Doubt::Working { .. }]), "the desk's news has not come: a doubt");
        // The backup: the folder found on the server under the same seal, the desk's news fetched.
        let state = w.find(DUE + 180);
        assert!(state.confirmed_for(&w.phone_folder), "{state:?}");
        assert_eq!(state.url, w.fake.folder_url("Documents/Sioul"));
        let before = w.others_here();
        let pulled = w.pull(DUE + 181);
        assert!(pulled.problem.is_none() && pulled.fetched >= 2, "{pulled:?}");
        assert_eq!(w.others_here(), before, "nothing written into the synced folder");
        let outcome = w.phone.exchange(&w.phone_folder, DUE + 182);
        assert!(outcome.problems.is_empty(), "{:?}", outcome.problems);
        let press = w.phone.dnd().expect("the desk's press");
        assert!(press.on && press.from == w.desk.id, "{press:?}");
        // Its entry the fresh one: the dose known not taken, as with the folder fresh.
        let fresh = w.phone.sees(&w.phone_folder, &w.desk, DUE + 183);
        assert!(doubts_now(DUE, DUE + 183, None, std::slice::from_ref(&fresh)).is_empty(), "{fresh:?}");
        let direct = crate::devices::all(&w.server, &KEY).0.into_iter().find(|e| e.id == w.desk.id).unwrap();
        assert_eq!(fresh.said, Some(direct.said()), "the same as the server's folder says");
        assert!(crate::share::others(&w.phone_folder, &w.phone.id).iter().any(|o| o.id == w.desk.id && o.heard >= DUE + 150));
        // Read only, never one of the phone's own files; the password nowhere in what it keeps.
        for line in w.fake.seen() {
            assert!(line.starts_with("PROPFIND ") || line.starts_with("GET "), "{line}");
            assert!(!(line.starts_with("GET ") && line.contains(&w.phone.id)), "{line}");
        }
        let kept = std::fs::read_to_string(state_path(&w.phone.memory)).unwrap();
        assert!(!kept.contains(PASSWORD) && !kept.contains("Basic"), "{kept}");
        assert!(walk(&w.cache()).keys().all(|name| !name.contains(&w.phone.id)), "never the phone's own");
    }

    #[test]
    fn with_the_folder_fresh_the_backup_changes_nothing() {
        let w = World::new("fresh");
        w.desk.press(true, DUE + 30);
        w.desk.exchange(&w.server, DUE + 150);
        crate::lease::renew(&w.server, &KEY, "health", &w.desk.id, DUE + 150, DUE + 150, false, crate::lease::Rule::FollowsYou, crate::share::written(&w.desk.memory, &w.desk.id)).unwrap();
        w.up();
        w.down();
        // The phone exchanges without the backup...
        let aside = w.snapshot();
        let without = w.phone.exchange(&w.phone_folder, DUE + 200);
        let kept_without = w.phone.kept();
        // ...and, put back as it was, with it: the same.
        w.restore(&aside);
        assert!(w.find(DUE + 190).confirmed_for(&w.phone_folder));
        w.fake.forget_seen();
        let pulled = w.pull(DUE + 195);
        assert!(pulled.problem.is_none(), "{pulled:?}");
        assert_eq!(pulled.fetched, 0, "nothing the folder holds is kept twice");
        assert!(w.fake.seen().iter().all(|line| !(line.starts_with("GET ") && line.contains(".jsonl"))), "no records fetched: {:?}", w.fake.seen());
        assert_eq!(walk(&w.cache()).into_keys().collect::<Vec<_>>(), ["seal.toml"]);
        let with = w.phone.exchange(&w.phone_folder, DUE + 200);
        assert_eq!((with.received, with.sent, &with.problems), (without.received, without.sent, &without.problems));
        assert_eq!(w.phone.kept(), kept_without, "the same files, byte for byte");
        // Fetched again: nothing, the server's files known by their ETag.
        w.fake.forget_seen();
        w.pull(DUE + 260);
        assert!(w.fake.seen().iter().all(|line| line.starts_with("PROPFIND ")), "{:?}", w.fake.seen());
    }

    #[test]
    fn a_half_downloaded_file_is_never_read_as_whole() {
        let w = World::new("half");
        // Two doses marked on the desk, carried to the phone as they should be.
        let first = "\"first@1800000000\" = 1800000000\n\"second@1800000000\" = 1800000001\n";
        w.desk.write("health-state.toml", &format!("[taken]\n{first}"));
        w.desk.exchange(&w.server, DUE + 50);
        w.down();
        // Forty more: forty lines of records, carried to the server only.
        let marks: String = (0..40).map(|n| format!("\"dose-{n}@1800000000\" = {}\n", DUE + n)).collect();
        w.desk.write("health-state.toml", &format!("[taken]\n{first}{marks}"));
        w.desk.exchange(&w.server, DUE + 100);
        assert!(w.find(DUE + 110).confirmed_for(&w.phone_folder));
        let round = w.desk_round();
        let whole = std::fs::read(w.server.join(&round)).unwrap();
        let (end, last) = whole_end(&w.phone_folder.join(&round));
        assert!(end > 0 && whole.len() as u64 > end, "the phone's copy is behind");
        // The records come cut in the middle of a line.
        let cut = (whole.len() - last as usize) / 2;
        let name = round.clone();
        w.fake.fault(once(move |r| r.method == "GET" && r.path.ends_with(&name), Fault::Cut(cut)));
        let pulled = w.pull(DUE + 120);
        assert!(pulled.problem.as_deref().is_some_and(|p| p.starts_with("network:")), "{pulled:?}");
        assert!(w.fake.seen().iter().any(|l| l.starts_with("GET ") && l.contains(&round) && l.ends_with(" 206")), "asked from where the phone's copy ends: {:?}", w.fake.seen());
        let cached = w.cache().join(&round);
        let arrived = std::fs::read(&cached).unwrap_or_default();
        assert!(arrived.is_empty() || (arrived.ends_with(b"\n") && whole.starts_with(&arrived) && arrived.len() < whole.len()), "whole lines only: {} of {}", arrived.len(), whole.len());
        let first = w.phone.exchange(&w.phone_folder, DUE + 130);
        assert!(first.problems.iter().all(|p| !p.starts_with("share-other")), "{:?}", first.problems);
        // Then the desk's entry comes cut: never kept, never read.
        let entry = format!("devices/{}.device", w.desk.id);
        let name = entry.clone();
        w.fake.fault(once(move |r| r.method == "GET" && r.path.ends_with(&name), Fault::Cut(40)));
        let pulled = w.pull(DUE + 140);
        assert!(pulled.problem.as_deref().is_some_and(|p| p.starts_with("network:")), "{pulled:?}");
        assert_eq!(std::fs::read(&cached).unwrap(), whole, "the records whole now");
        assert!(!w.cache().join(&entry).exists(), "an entry cut is not kept");
        let (entries, unread) = crate::devices::all(&w.phone_folder, &KEY);
        assert!(entries.iter().any(|e| e.id == w.desk.id) && !unread.contains(&w.desk.id), "the folder's copy read meanwhile");
        let second = w.phone.exchange(&w.phone_folder, DUE + 150);
        assert!(second.problems.iter().all(|p| !p.starts_with("share-other")), "{:?}", second.problems);
        assert!(w.pull(DUE + 160).problem.is_none());
        assert!(w.cache().join(&entry).exists());
        // Every dose came, each line read once, as far as the desk wrote.
        let record = w.phone.read("health-state.toml");
        for n in 0..40 {
            assert!(record.contains(&format!("dose-{n}@")), "dose-{n}:\n{record}");
        }
        let heard = crate::share::heard(&w.phone.memory, &w.phone.id);
        assert!(heard.complete(&w.desk.id, crate::share::written(&w.desk.memory, &w.desk.id).unwrap()), "{heard:?}");
        assert!(!heard.broken.contains_key(&w.desk.id), "no line lost nor read twice: {heard:?}");
    }

    #[test]
    fn a_seal_that_differs_is_never_used() {
        let w = World::new("seal");
        // A folder of the same name on the server, sealed otherwise: refused, and nothing fetched from it.
        put(&w.server, "seal.toml", OTHER_SEAL);
        let state = w.find(DUE);
        assert!(!state.confirmed_for(&w.phone_folder) && state.said.starts_with("seal-differs:"), "{state:?}");
        assert!(attach(&w.phone_folder, &w.phone.memory).is_none());
        assert_eq!(w.pull(DUE + 1).problem.as_deref(), Some("not-confirmed"));
        assert!(w.fake.seen().iter().filter(|l| l.starts_with("GET ")).all(|l| l.ends_with("seal.toml 200") || l.ends_with("seal.toml 404")), "{:?}", w.fake.seen());
        // Sealed the same: confirmed. Then sealed otherwise there: nothing more fetched.
        put(&w.server, "seal.toml", SEAL);
        State::choose(&w.phone.memory, |s| s.asked = DUE + 2).unwrap();
        assert!(w.find(DUE + 3).confirmed_for(&w.phone_folder));
        assert!(w.pull(DUE + 4).problem.is_none());
        put(&w.server, "seal.toml", OTHER_SEAL);
        w.desk.press(true, DUE + 5);
        w.desk.exchange(&w.server, DUE + 6);
        w.fake.forget_seen();
        let pulled = w.pull(DUE + 7);
        assert!(pulled.problem.as_deref().is_some_and(|p| p.starts_with("seal-differs:")), "{pulled:?}");
        assert!(w.fake.seen().iter().filter(|l| l.starts_with("GET ")).all(|l| l.contains("seal.toml")), "{:?}", w.fake.seen());
        assert!(!State::load(&w.phone.memory).confirmed_for(&w.phone_folder));
        // Sharing started again here, under another seal: what was fetched is not read.
        put(&w.server, "seal.toml", SEAL);
        State::choose(&w.phone.memory, |s| s.asked = DUE + 8).unwrap();
        assert!(w.find(DUE + 9).confirmed_for(&w.phone_folder));
        assert!(w.pull(DUE + 10).problem.is_none());
        assert!(attach(&w.phone_folder, &w.phone.memory).is_some());
        put(&w.phone_folder, "seal.toml", OTHER_SEAL);
        assert!(attach(&w.phone_folder, &w.phone.memory).is_none(), "another sharing's");
        assert_eq!(w.pull(DUE + 11).problem.as_deref(), Some("no-seal"));
        assert!(!w.cache().exists());
    }

    #[test]
    fn server_errors_and_timeouts_leave_the_exchange_as_before() {
        let w = World::new("errors");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        w.desk.press(true, DUE + 30);
        w.desk.exchange(&w.server, DUE + 60);
        w.up();
        for (n, fault) in [Fault::Status(500), Fault::Status(503), Fault::Late(Duration::from_secs(2)), Fault::Drop, Fault::Cut(20)].into_iter().enumerate() {
            w.fake.clear_faults();
            w.fake.fault(move |_| Some(fault));
            let pulled = w.pull(DUE + 100 + n as i64);
            let said = pulled.problem.clone().unwrap_or_default();
            assert!(said.starts_with("server:") || said.starts_with("network:"), "{fault:?}: {pulled:?}");
            assert_eq!(walk(&w.cache()).into_keys().collect::<Vec<_>>(), ["seal.toml"], "{fault:?}: nothing kept");
            assert_eq!(State::load(&w.phone.memory).said, said);
        }
        w.fake.clear_faults();
        // The exchange then is the one without the backup, byte for byte.
        let aside = w.snapshot();
        let with = w.phone.exchange(&w.phone_folder, DUE + 200);
        let kept_with = w.phone.kept();
        w.restore(&aside);
        forget(&w.phone.memory, &w.phone_folder);
        let without = w.phone.exchange(&w.phone_folder, DUE + 200);
        assert_eq!((with.received, &with.problems), (without.received, &without.problems));
        assert_eq!(w.phone.kept(), kept_with);
    }

    #[test]
    fn what_was_fetched_never_wins_over_a_newer_synced_copy() {
        let w = World::new("older");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        let renew = |now: i64| crate::lease::renew(&w.server, &KEY, "health", &w.desk.id, now, now, false, crate::lease::Rule::FollowsYou, crate::share::written(&w.desk.memory, &w.desk.id)).unwrap();
        w.desk.press(true, DUE + 30);
        w.desk.exchange(&w.server, DUE + 60);
        renew(DUE + 60);
        assert!(w.pull(DUE + 70).problem.is_none());
        let round = w.desk_round();
        assert!(w.cache().join(&round).exists() && w.cache().join(format!("devices/{}.device", w.desk.id)).exists());
        // The desk goes on, a quarter of an hour later; the sync app brings it all at last: the folder's copies newer.
        w.desk.press(false, DUE + 1_000);
        w.desk.exchange(&w.server, DUE + 1_100);
        renew(DUE + 1_100);
        w.up();
        w.down();
        let entry = crate::devices::all(&w.phone_folder, &KEY).0.into_iter().find(|e| e.id == w.desk.id).unwrap();
        assert_eq!(entry.exported, DUE + 1_100, "the folder's entry");
        let claim = crate::lease::claims(&w.phone_folder, &KEY, "health").into_iter().find(|c| c.computer == w.desk.id).unwrap();
        assert_eq!(claim.renewed, DUE + 1_100, "the folder's claim");
        let heard = crate::share::others(&w.phone_folder, &w.phone.id).into_iter().find(|o| o.id == w.desk.id).unwrap().heard;
        assert_eq!(heard, DUE + 1_100, "the folder's notes");
        w.phone.exchange(&w.phone_folder, DUE + 1_200);
        assert!(!w.phone.dnd().unwrap().on, "the later press, from the folder's longer records");
        // What the folder caught up with goes from the cache.
        tidy(&w.phone.memory, &w.phone_folder, &w.phone.id);
        assert!(!w.cache().join(&round).exists());
        // The backup switched off: the entries and notes fetched go too; records longer than the folder's would stay.
        State::choose(&w.phone.memory, |s| s.on = Some(false)).unwrap();
        tidy(&w.phone.memory, &w.phone_folder, &w.phone.id);
        assert_eq!(walk(&w.cache()).into_keys().collect::<Vec<_>>(), ["seal.toml"]);
        w.fake.forget_seen();
        assert_eq!(w.pull(DUE + 1_300).problem.as_deref(), Some("off"));
        assert!(w.fake.seen().is_empty(), "switched off: not a request");
    }

    /// The doses' rule (docs/health.md, "Knowing") with the phone's sync app
    /// stuck, bringing nothing down, and the backup fetching each minute:
    /// what the phone takes as known is there, and every dose comes.
    #[test]
    fn doses_are_never_known_wrongly_with_a_stuck_sync_app_and_the_backup() {
        let w = World::new("stuck");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        let mut marked: Vec<(String, i64)> = Vec::new();
        for minute in 0..90 {
            let now = DUE + minute * 60;
            if minute % 10 == 0 {
                marked.push((format!("dose@{minute}"), minute));
                let record: String = marked.iter().map(|(d, m)| format!("\"{d}\" = {m}\n")).collect();
                w.desk.write("health-state.toml", &format!("[taken]\n{record}"));
            }
            w.desk.exchange(&w.server, now);
            crate::lease::renew(&w.server, &KEY, "health", &w.desk.id, now, now, false, crate::lease::Rule::FollowsYou, crate::share::written(&w.desk.memory, &w.desk.id)).unwrap();
            w.up();
            let _ = w.pull(now + 20);
            w.phone.exchange(&w.phone_folder, now + 30);
            if let Some(claim) = crate::lease::claims(&w.phone_folder, &KEY, "health").into_iter().find(|c| c.computer == w.desk.id)
                && let Some(wrote) = claim.wrote
                && crate::share::heard(&w.phone.memory, &w.phone.id).complete(&w.desk.id, wrote)
            {
                let record = w.phone.read("health-state.toml");
                let claimed = (claim.renewed - DUE) / 60;
                for (dose, at) in marked.iter().filter(|(_, m)| *m < claimed) {
                    assert!(record.contains(dose.as_str()), "minute {minute}: {dose} ({at}) taken as known, not there:\n{record}");
                }
            }
        }
        let record = w.phone.read("health-state.toml");
        for (dose, _) in &marked {
            assert!(record.contains(dose.as_str()), "{dose} never came:\n{record}");
        }
        // The synced folder never had them: the backup brought them.
        let synced = std::fs::read(w.phone_folder.join(w.desk_round())).map(|b| b.len()).unwrap_or(0);
        assert!(synced < std::fs::read(w.server.join(w.desk_round())).unwrap().len());
    }

    /// An alarm's waits, short: a test does not wait twenty-five seconds.
    const QUICK: AlarmTimes = AlarmTimes { alone: Duration::from_secs(2), most: Duration::from_secs(5) };

    /// A dose's alarm on the phone (`health::alarm_decide`, through the app's
    /// `exchange_here`) on its own pull while it works (`alarm_news`): a pull
    /// begun since the alarm, gone through, and the sync app neither asked to
    /// look nor waited for. Minute after minute, the desk in use marking
    /// doses, the phone's sync app bringing nothing down: from the same
    /// moment, the alarm on its pull knows exactly what it knows with the sync
    /// app asked as before (it brought everything down), its doubt never
    /// weaker, and a dose marked elsewhere is never known not taken.
    #[test]
    fn a_doses_alarm_on_its_pull_knows_what_it_knew_with_the_sync_app_asked() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = World::new("alarm-alone");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        assert!(w.pull(DUE).problem.is_none());
        let mut marked: Vec<(String, i64)> = Vec::new();
        for minute in 0..16 {
            let now = DUE + minute * 60;
            if minute % 3 == 0 {
                marked.push((format!("dose@{minute}"), now));
                let record: String = marked.iter().map(|(d, at)| format!("\"{d}\" = {at}\n")).collect();
                w.desk.write("health-state.toml", &format!("[taken]\n{record}"));
            }
            w.desk.exchange(&w.server, now);
            w.up();
            // As before: the sync app asked brings the server's files down, the pull meanwhile.
            let aside = w.snapshot();
            w.down();
            let _ = w.pull(now + 20);
            w.phone.exchange(&w.phone_folder, now + 25);
            let asked = (w.phone.read("health-state.toml"), w.phone.sees(&w.phone_folder, &w.desk, now + 30));
            // The alarm on its pull, from the same moment: the phone's folder as stale as the sync app left it.
            w.restore(&aside);
            assert!(State::load(&w.phone.memory).pulls_well(&w.phone_folder), "minute {minute}");
            let mut ask = || -> Duration { panic!("minute {minute}: the pull went through, the sync app is not asked") };
            let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, now + 20, QUICK, Arc::new(move || now + 20), &mut ask, TEST);
            assert_eq!(came, AlarmNews::Pulled, "minute {minute}");
            w.phone.exchange(&w.phone_folder, now + 25);
            let alone = (w.phone.read("health-state.toml"), w.phone.sees(&w.phone_folder, &w.desk, now + 30));
            assert_eq!(alone.0, asked.0, "minute {minute}: the same answers here");
            for (dose, due) in &marked {
                let doubted = |peer: &Peer| !doubts_now(*due, now + 30, None, std::slice::from_ref(peer)).is_empty();
                assert_eq!(doubted(&alone.1), doubted(&asked.1), "minute {minute}, {dose}: the same doubt");
                assert!(alone.0.contains(dose.as_str()) || doubted(&alone.1), "minute {minute}: {dose}, marked on the desk, known not taken");
            }
        }
        // The synced folder never had them: the pull brought them.
        assert!(std::fs::read(w.phone_folder.join(w.desk_round())).map_or(0, |b| b.len()) < std::fs::read(w.server.join(w.desk_round())).unwrap().len());
    }

    /// The desk closed an hour before the dose, the phone heard it through a
    /// good pull; a minute before the dose the desk opens, the dose is marked
    /// taken there and shared. The phone's alarm: its own pull fails. It never
    /// decides on what the last good pull said (the desk closed: the dose
    /// "known" not taken); it asks the sync app, as before Sioul's own pull,
    /// and waits for it: the answer comes, or the doubt is said. The next
    /// alarm asks the sync app first; the server back, the pull is trusted
    /// again (review of battery part B, F2).
    #[test]
    fn a_doses_alarm_whose_pull_fails_asks_the_sync_app_and_never_knows_wrongly() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = World::new("alarm-fails");
        assert!(w.find(DUE - 3_000).confirmed_for(&w.phone_folder));
        w.desk.exchange(&w.server, DUE - 3_600);
        w.desk.session(&w.server, |e| e.close(DUE - 3_590));
        assert!(w.pull(DUE - 3_000).problem.is_none());
        w.phone.exchange(&w.phone_folder, DUE - 2_990);
        assert!(State::load(&w.phone.memory).pulls_well(&w.phone_folder), "the pull works");
        assert!(doubts_now(DUE, DUE + 10, None, &[w.phone.sees(&w.phone_folder, &w.desk, DUE + 10)]).is_empty(), "the desk closed before the dose: known");
        w.desk.session(&w.server, |e| e.start(DUE - 60));
        w.desk.write("health-state.toml", "[taken]\n\"dose@0\" = 1\n");
        w.desk.exchange(&w.server, DUE + 30);
        // The alarm at DUE + 60: its pull refused (503); the sync app asked brings the server's files down.
        w.fake.fault(|_| Some(Fault::Status(503)));
        let mut asked = 0;
        let mut ask = || {
            asked += 1;
            w.down();
            Duration::from_millis(200)
        };
        let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST);
        assert!(matches!(came, AlarmNews::SyncApp { ref heard } if heard.contains(&w.desk.id)) && asked == 1, "the pull failed: the sync app asked, once, the desk's entry brought: {came:?}");
        w.phone.exchange(&w.phone_folder, DUE + 65);
        let taken = w.phone.read("health-state.toml").contains("dose@0");
        let doubts = doubts_now(DUE, DUE + 70, None, &[w.phone.sees(&w.phone_folder, &w.desk, DUE + 70)]);
        assert!(taken || !doubts.is_empty(), "the dose marked on the desk is never known not taken: {doubts:?}");
        assert!(taken, "the sync app brought it");
        assert!(!State::load(&w.phone.memory).pulls_well(&w.phone_folder), "the next alarm asks the sync app first, as before");
        // The server back: the pull is trusted again.
        w.fake.clear_faults();
        assert!(w.pull(DUE + 180).problem.is_none());
        assert!(State::load(&w.phone.memory).pulls_well(&w.phone_folder));
    }

    /// Two doses due at the same minute: two alarms at once, each on its own
    /// thread (review of battery part B, F1). The second finds the first one
    /// pulling: it waits for that pull, never decides on what came before it,
    /// and knows what it brought; the sync app is asked by neither.
    #[test]
    fn two_doses_at_once_the_second_alarm_waits_for_the_first_ones_pull() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = World::new("alarm-twice");
        assert!(w.find(DUE - 3_000).confirmed_for(&w.phone_folder));
        w.desk.session(&w.server, |e| e.close(DUE - 3_590));
        assert!(w.pull(DUE - 3_000).problem.is_none());
        w.phone.exchange(&w.phone_folder, DUE - 2_990);
        // The desk opens a minute before, the doses are marked taken there.
        w.desk.session(&w.server, |e| e.start(DUE - 60));
        w.desk.write("health-state.toml", "[taken]\n\"a@0\" = 1\n\"b@0\" = 2\n");
        w.desk.exchange(&w.server, DUE + 10);
        // The first alarm's pull is slow: its first listing takes half a second (within the test's wait for an answer).
        w.fake.fault(once(|r| r.method == "PROPFIND", Fault::Late(Duration::from_millis(450))));
        let outcomes: Vec<(AlarmNews, Duration)> = std::thread::scope(|scope| {
            let alarm = |pause: u64| {
                let w = &w;
                scope.spawn(move || {
                    std::thread::sleep(Duration::from_millis(pause));
                    let begun = Instant::now();
                    let mut ask = || -> Duration { panic!("the pull went through: the sync app is not asked") };
                    (alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST), begun.elapsed())
                })
            };
            let (first, second) = (alarm(0), alarm(150));
            vec![first.join().unwrap(), second.join().unwrap()]
        });
        assert!(outcomes.iter().all(|(came, _)| *came == AlarmNews::Pulled), "{outcomes:?}");
        assert!(outcomes[1].1 >= Duration::from_millis(250), "the second waited for the first one's pull: {outcomes:?}");
        w.phone.exchange(&w.phone_folder, DUE + 70);
        let record = w.phone.read("health-state.toml");
        assert!(record.contains("a@0") && record.contains("b@0"), "{record}");
        let seen = w.phone.sees(&w.phone_folder, &w.desk, DUE + 70);
        assert!(seen.said.as_ref().is_some_and(|s| s.working), "the desk known in use, from the pull: {seen:?}");
    }

    /// An alarm while the window's pull runs (Sioul on the screen, its minute's
    /// pull begun before the alarm): the alarm waits for it, then, that pull
    /// begun before it, pulls again itself, and knows what came since (review
    /// of battery part B, F1).
    #[test]
    fn an_alarm_while_the_windows_pull_runs_pulls_again_after_it() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = World::new("alarm-window");
        assert!(w.find(DUE - 3_000).confirmed_for(&w.phone_folder));
        w.desk.session(&w.server, |e| e.close(DUE - 3_590));
        assert!(w.pull(DUE - 3_000).problem.is_none());
        w.phone.exchange(&w.phone_folder, DUE - 2_990);
        w.desk.session(&w.server, |e| e.start(DUE - 60));
        w.desk.write("health-state.toml", "[taken]\n\"dose@0\" = 1\n");
        w.desk.exchange(&w.server, DUE + 10);
        w.fake.fault(once(|r| r.method == "PROPFIND", Fault::Late(Duration::from_millis(450))));
        let came = std::thread::scope(|scope| {
            // The window's pull, begun at DUE + 50, holding the one pull at a time.
            let window = scope.spawn(|| {
                let _one = ONE_PULL.lock().unwrap();
                pull_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), DUE + 50, TEST)
            });
            std::thread::sleep(Duration::from_millis(200));
            let mut ask = || -> Duration { panic!("a pull went through: the sync app is not asked") };
            let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 61), &mut ask, TEST);
            assert!(window.join().unwrap().problem.is_none());
            came
        });
        assert_eq!(came, AlarmNews::Pulled);
        assert_eq!(State::load(&w.phone.memory).last, DUE + 61, "pulled again, after the window's");
        w.phone.exchange(&w.phone_folder, DUE + 70);
        assert!(w.phone.read("health-state.toml").contains("dose@0"));
    }

    /// An alarm whose pull is slower than its wait (a large round first, a
    /// slow network): the sync app is asked after the pull's few seconds, and
    /// waited for; the alarm decides on what it brought, never on what came
    /// before it (review of battery part B, F2 and F7).
    #[test]
    fn an_alarm_whose_pull_does_not_go_through_in_time_asks_the_sync_app() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = World::new("alarm-slow");
        assert!(w.find(DUE - 3_000).confirmed_for(&w.phone_folder));
        w.desk.session(&w.server, |e| e.close(DUE - 3_590));
        assert!(w.pull(DUE - 3_000).problem.is_none());
        w.phone.exchange(&w.phone_folder, DUE - 2_990);
        w.desk.session(&w.server, |e| e.start(DUE - 60));
        w.desk.write("health-state.toml", "[taken]\n\"dose@0\" = 1\n");
        w.desk.exchange(&w.server, DUE + 10);
        // Every answer of the server comes after six seconds: past the alarm's wait.
        w.fake.fault(|_| Some(Fault::Late(Duration::from_secs(6))));
        let mut asked = 0;
        let mut ask = || {
            asked += 1;
            w.down();
            Duration::from_millis(300)
        };
        let begun = Instant::now();
        let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST);
        let waited = begun.elapsed();
        assert!(matches!(came, AlarmNews::SyncApp { .. }) && asked == 1, "not through in time: the sync app asked, once: {came:?}");
        assert!(waited < QUICK.most, "never past the alarm's wait: {waited:?}");
        w.phone.exchange(&w.phone_folder, DUE + 65);
        assert!(w.phone.read("health-state.toml").contains("dose@0"), "what the sync app brought is read");
        w.fake.clear_faults();
    }

    /// The scene of the review's F2: the desk closed an hour before the dose,
    /// heard through a good pull (`pulled_at`, by the phone's clock then);
    /// then, if `opens`, it opens a minute before the dose, the dose is marked
    /// taken there and shared.
    fn alarm_scene(name: &str, pulled_at: i64, opens: bool) -> World {
        let w = World::new(name);
        assert!(w.find(DUE - 3_000).confirmed_for(&w.phone_folder));
        w.desk.exchange(&w.server, DUE - 3_600);
        w.desk.session(&w.server, |e| e.close(DUE - 3_590));
        assert!(w.pull(pulled_at).problem.is_none());
        w.phone.exchange(&w.phone_folder, DUE - 2_990);
        assert!(State::load(&w.phone.memory).pulls_well(&w.phone_folder));
        if opens {
            w.desk.session(&w.server, |e| e.start(DUE - 60));
            w.desk.write("health-state.toml", "[taken]\n\"dose@0\" = 1\n");
            w.desk.exchange(&w.server, DUE + 30);
        }
        w
    }

    /// The phone's clock an hour ahead when it last pulled, then set right
    /// (review of battery part B, S1). That pull, said begun after the alarm,
    /// is never taken for one since it: pulled again (refused here), the sync
    /// app asked; nothing came, so the doubt is said (S2). The clock set back
    /// never stops the background pulls either.
    #[test]
    fn an_alarm_never_takes_a_pull_from_before_the_clock_was_set_back() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = alarm_scene("alarm-clock", DUE + 600, true);
        w.fake.fault(|_| Some(Fault::Status(503)));
        let mut asked = 0;
        let mut ask = || {
            asked += 1;
            Duration::from_millis(200)
        };
        let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST);
        assert_eq!((&came, asked), (&AlarmNews::SyncApp { heard: BTreeSet::new() }, 1), "the old pull not taken: pulled again, refused; the sync app asked, nothing came");
        w.phone.exchange(&w.phone_folder, DUE + 65);
        assert!(!w.phone.read("health-state.toml").contains("dose@0"));
        let peer = w.phone.sees(&w.phone_folder, &w.desk, DUE + 70);
        assert!(doubts_now(DUE, DUE + 70, None, std::slice::from_ref(&peer)).is_empty(), "what was read before the alarm says the desk closed");
        assert!(!sioul_core::health::doubts_unread(DUE, DUE + 70, None, std::slice::from_ref(&peer), DUE + 60, &BTreeSet::new()).is_empty(), "nothing read since the alarm: the doubt said, never known");
        let state = State::load(&w.phone.memory);
        assert!(!state.pulls_well(&w.phone_folder), "the refused pull is the later one, whatever the clock said: {state:?}");
        let pace = Pace { phone: true, shown: false, in_use: false, urgent: false, step: 5 * 60 };
        assert!(due(&State { tried: DUE + 600, ..state }, DUE + 120, pace), "tried \"later\" than now: due at once");
        w.fake.clear_faults();
    }

    /// Neither route brings anything at an alarm (the server out of reach, the
    /// sync app bringing nothing): the desk, known closed before the dose by
    /// what was read before, may have opened since and answered; the doubt is
    /// said, never "not taken" (review of battery part B, S2). An alarm whose
    /// pull went through is not doubted by that rule: the desk still closed,
    /// the dose known.
    #[test]
    fn an_alarm_that_reads_no_news_says_the_doubt_and_one_that_pulls_does_not() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = alarm_scene("alarm-nothing", DUE - 3_000, true);
        w.fake.fault(|_| Some(Fault::Status(503)));
        let mut ask = || Duration::from_millis(200);
        let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST);
        assert_eq!(came, AlarmNews::SyncApp { heard: BTreeSet::new() });
        w.phone.exchange(&w.phone_folder, DUE + 65);
        let peer = w.phone.sees(&w.phone_folder, &w.desk, DUE + 70);
        assert!(!w.phone.read("health-state.toml").contains("dose@0"));
        let doubts = sioul_core::health::doubts_unread(DUE, DUE + 70, None, std::slice::from_ref(&peer), DUE + 60, &BTreeSet::new());
        assert!(matches!(doubts.as_slice(), [Doubt::Unheard { closed: true, until, .. }] if *until == DUE - 3_590), "the desk as last heard, closed: {doubts:?}");
        w.fake.clear_faults();
        // The desk stays closed; the alarm's pull goes through: known, no doubt.
        static ANOTHER: Mutex<()> = Mutex::new(());
        let quiet = alarm_scene("alarm-pulled", DUE - 3_000, false);
        let mut never = || -> Duration { panic!("the pull went through: the sync app is not asked") };
        let came = alarm_news_with(&quiet.phone.memory, &quiet.phone_folder, &quiet.phone.id, KEY, Some(quiet.fake.login()), &ANOTHER, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut never, TEST);
        assert_eq!(came, AlarmNews::Pulled);
        quiet.phone.exchange(&quiet.phone_folder, DUE + 65);
        assert!(doubts_now(DUE, DUE + 70, None, &[quiet.phone.sees(&quiet.phone_folder, &quiet.desk, DUE + 70)]).is_empty(), "pulled since the alarm, the desk closed: known");
    }

    /// What the phone's alarm decides of the desk once its news came (as
    /// `alarm_decide` does: `doubts_unread` when no pull went through since
    /// it began): whether the dose is known wrongly, read taken, the doubts.
    fn alarm_verdict(w: &World, came: &AlarmNews, since: i64, at: i64) -> (bool, bool, Vec<Doubt>) {
        w.phone.exchange(&w.phone_folder, at);
        let taken = w.phone.read("health-state.toml").contains("dose@0");
        let peer = w.phone.sees(&w.phone_folder, &w.desk, at + 5);
        let doubts = match came {
            AlarmNews::SyncApp { heard } => sioul_core::health::doubts_unread(DUE, at + 5, None, std::slice::from_ref(&peer), since, heard),
            AlarmNews::Pulled => doubts_now(DUE, at + 5, None, std::slice::from_ref(&peer)),
        };
        (!taken && doubts.is_empty(), taken, doubts)
    }

    /// The pull refused; the sync app brings one file of the desk's that says
    /// nothing of its session (its notes), not its entry: the desk, opened
    /// since and marked the dose there, is never known closed by what was read
    /// before; the doubt is said (review of battery part B, T2). Its entry
    /// brought, it is as its entry says.
    #[test]
    fn an_alarm_doubts_each_device_whose_own_entry_did_not_come() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = alarm_scene("alarm-unrelated", DUE - 3_000, true);
        w.fake.fault(|_| Some(Fault::Status(503)));
        let notes = format!("{}.toml", w.desk.id);
        let mut ask = || {
            // The sync app brings only the desk's notes (how far it read): not its entry, not its records.
            std::fs::copy(w.server.join(&notes), w.phone_folder.join(&notes)).unwrap();
            std::fs::File::options().write(true).open(w.phone_folder.join(&notes)).unwrap().set_modified(std::time::SystemTime::now()).unwrap();
            Duration::from_millis(200)
        };
        let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST);
        w.fake.clear_faults();
        assert_eq!(came, AlarmNews::SyncApp { heard: BTreeSet::new() }, "the notes are no news of the desk's session");
        let (wrong, taken, doubts) = alarm_verdict(&w, &came, DUE + 60, DUE + 65);
        assert!(!wrong && !taken && matches!(doubts.as_slice(), [Doubt::Unheard { closed: true, .. }]), "{doubts:?}");
        // The sync app brings the desk's entry too: news of it came, as it says (in use, not after the dose: a doubt of its own).
        static AGAIN: Mutex<()> = Mutex::new(());
        let other = alarm_scene("alarm-entry-came", DUE - 3_000, true);
        other.fake.fault(|_| Some(Fault::Status(503)));
        let entry = format!("devices/{}.device", other.desk.id);
        let mut ask = || {
            std::fs::copy(other.server.join(&entry), other.phone_folder.join(&entry)).unwrap();
            Duration::from_millis(200)
        };
        let came = alarm_news_with(&other.phone.memory, &other.phone_folder, &other.phone.id, KEY, Some(other.fake.login()), &AGAIN, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST);
        other.fake.clear_faults();
        assert!(matches!(came, AlarmNews::SyncApp { ref heard } if heard.contains(&other.desk.id)), "{came:?}");
        let (wrong, _, doubts) = alarm_verdict(&other, &came, DUE + 60, DUE + 65);
        assert!(!wrong && matches!(doubts.as_slice(), [Doubt::Working { .. }] | [Doubt::Coming { .. }]), "the desk in use, as its entry says: {doubts:?}");
    }

    /// The clock set forward, a pull in this process, then set back: the
    /// alarm's own pull goes through and the desk is truly closed: counted,
    /// known, no needless doubt (review of battery part B, T1).
    #[test]
    fn a_clock_set_back_never_hides_the_alarms_own_pull() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = alarm_scene("alarm-forward-back", DUE - 3_000, false);
        assert!(w.pull(DUE + 600).problem.is_none());
        let mut never = || -> Duration { panic!("the alarm's own pull went through: the sync app is not asked") };
        let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut never, TEST);
        assert_eq!(came, AlarmNews::Pulled);
        assert_eq!(State::load(&w.phone.memory).last, DUE + 60, "the time from before the set-back gives way");
        let (_, _, doubts) = alarm_verdict(&w, &came, DUE + 60, DUE + 65);
        assert!(doubts.is_empty(), "{doubts:?}");
    }

    /// A pull that began with a round left to the sync app and found it again,
    /// grown, while the other process's save found it brought: still left, its
    /// own finding (review of battery part B, T3).
    #[test]
    fn a_round_found_again_grown_stays_left_in_the_merge() {
        let w = World::new("left-regrown");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        assert!(w.pull(DUE + 10).problem.is_none());
        let disk = State::load(&w.phone.memory);
        // The other process saved first: R brought, nothing left.
        State { left: BTreeMap::new(), tried: DUE + 20, last: DUE + 20, ..disk.clone() }.write(&w.phone.memory).unwrap();
        // This pull began with R left at 100 bytes; its look found it at 200.
        let loaded: BTreeMap<String, u64> = [("r-1.jsonl".to_string(), 100)].into();
        let ours = State { tried: DUE + 21, last: DUE + 21, left: [("r-1.jsonl".to_string(), 200)].into(), ..disk };
        ours.save_pulled(&w.phone.memory, DUE + 25, &loaded).unwrap();
        let merged = State::load(&w.phone.memory);
        assert_eq!(merged.left.get("r-1.jsonl"), Some(&200), "{:?}", merged.left);
        assert!(!merged.pulls_well(&w.phone_folder));
    }

    /// Only what the doses read of a device is news of its session (review of
    /// battery part B, fourth pass): its claim on another part (notices),
    /// brought alone, is none (U1); its stale entry merely touched, same bytes
    /// at a new time, is none either (U2). Either way the desk, opened since and
    /// the dose marked there, is said in doubt, never known closed.
    #[test]
    fn only_a_newer_entry_or_health_claim_is_news_of_a_device() {
        static ONE_PULL: Mutex<()> = Mutex::new(());
        let w = alarm_scene("alarm-notices", DUE - 3_000, true);
        crate::lease::renew(&w.server, &KEY, "notices", &w.desk.id, DUE + 20, DUE + 20, false, crate::lease::Rule::FollowsYou, crate::share::written(&w.desk.memory, &w.desk.id)).unwrap();
        w.fake.fault(|_| Some(Fault::Status(503)));
        let lease = format!("leases/notices/{}.lease", w.desk.id);
        let mut ask = || {
            let to = w.phone_folder.join(&lease);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(w.server.join(&lease), &to).unwrap();
            Duration::from_millis(200)
        };
        let came = alarm_news_with(&w.phone.memory, &w.phone_folder, &w.phone.id, KEY, Some(w.fake.login()), &ONE_PULL, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST);
        w.fake.clear_faults();
        assert_eq!(came, AlarmNews::SyncApp { heard: BTreeSet::new() }, "a notices claim is no news of the session");
        let (wrong, _, doubts) = alarm_verdict(&w, &came, DUE + 60, DUE + 65);
        assert!(!wrong && !doubts.is_empty(), "{doubts:?}");
        // The stale entry touched: same bytes, a new time.
        static AGAIN: Mutex<()> = Mutex::new(());
        let touched = alarm_scene("alarm-touched", DUE - 3_000, true);
        touched.fake.fault(|_| Some(Fault::Status(503)));
        let entry = touched.phone_folder.join(format!("devices/{}.device", touched.desk.id));
        let mut ask = || {
            std::fs::File::options().write(true).open(&entry).unwrap().set_modified(std::time::SystemTime::now()).unwrap();
            Duration::from_millis(200)
        };
        let came = alarm_news_with(&touched.phone.memory, &touched.phone_folder, &touched.phone.id, KEY, Some(touched.fake.login()), &AGAIN, DUE + 60, QUICK, Arc::new(|| DUE + 60), &mut ask, TEST);
        touched.fake.clear_faults();
        assert_eq!(came, AlarmNews::SyncApp { heard: BTreeSet::new() }, "the same entry at a new time is no news");
        let (wrong, _, doubts) = alarm_verdict(&touched, &came, DUE + 60, DUE + 65);
        assert!(!wrong && !doubts.is_empty(), "{doubts:?}");
    }

    /// Two pulls at once in two processes (the window's or an alarm's, and the
    /// background step's), here two threads without a common lock: neither
    /// undoes the other (review of battery part B, S3). The one begun later says
    /// when a pull was tried and went through; a round left to the sync app,
    /// found by the slower one, stays left after the other's save.
    #[test]
    fn two_pulls_at_once_keep_what_each_found() {
        let w = World::new("two-pulls");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        w.desk.write("health-state.toml", "[taken]\n\"dose@0\" = 1\n");
        w.desk.exchange(&w.server, DUE + 10);
        w.fake.fault(once(|r| r.method == "PROPFIND", Fault::Late(Duration::from_millis(450))));
        std::thread::scope(|scope| {
            let slow = scope.spawn(|| pull_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), DUE + 50, TEST));
            std::thread::sleep(Duration::from_millis(100));
            // This one's look (begun later, done first): rounds past 64 bytes are left to the sync app.
            ROUND_CAP.with(|cap| cap.set(Some(64)));
            // Begun a moment later by the same clock (within the slower one's run, as a real one would).
            let quick = pull_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), DUE + 51, TEST);
            ROUND_CAP.with(|cap| cap.set(None));
            assert!(quick.problem.is_none() && quick.left >= 1, "{quick:?}");
            let slow = slow.join().unwrap();
            assert!(slow.problem.is_none(), "{slow:?}");
        });
        let state = State::load(&w.phone.memory);
        assert_eq!((state.tried, state.last), (DUE + 51, DUE + 51), "the later begun says when");
        assert!(!state.left.is_empty() && !state.pulls_well(&w.phone_folder), "found left by the other, never erased by the slower one's save: {:?}", state.left);
        // A look that finds it brought (the slower one fetched it) clears it.
        assert!(w.pull(DUE + 120).problem.is_none());
        assert!(State::load(&w.phone.memory).pulls_well(&w.phone_folder));
    }

    /// A round on the server past what a pull brings (`LARGEST`; a small cap
    /// in this test) is left to the sync app, and said: the pull alone is no
    /// longer trusted while it lasts; once the sync app brought it, the pull is
    /// again (review of battery part B, F4).
    #[test]
    fn a_round_too_large_for_the_pull_is_said_and_left_to_the_sync_app() {
        let w = World::new("too-large");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        w.desk.write("health-state.toml", "[taken]\n\"dose@0\" = 1\n");
        w.desk.exchange(&w.server, DUE + 10);
        ROUND_CAP.with(|cap| cap.set(Some(64)));
        let pulled = w.pull(DUE + 20);
        assert!(pulled.problem.is_none() && pulled.left >= 1, "{pulled:?}");
        let state = State::load(&w.phone.memory);
        assert!(!state.left.is_empty() && !state.pulls_well(&w.phone_folder) && !state.went_through_since(&w.phone_folder, DUE + 20, DUE + 20), "{:?}", state.left);
        // The sync app brings it: nothing left there that is not here.
        w.down();
        let pulled = w.pull(DUE + 100);
        ROUND_CAP.with(|cap| cap.set(None));
        assert_eq!(pulled.left, 0, "{pulled:?}");
        assert!(State::load(&w.phone.memory).pulls_well(&w.phone_folder));
    }

    /// The phone's entry on the server put back to an older copy (another
    /// device's sync app, the server's versions): the next pull lists it of
    /// another size than the phone sent, and the next send sends it over that
    /// copy, rather than at the check of every six hours (review of battery
    /// part B, F6).
    #[test]
    fn the_phones_entry_put_back_older_on_the_server_is_sent_again() {
        let w = World::new("entry-back");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        let entry = format!("devices/{}.device", w.phone.id);
        let older = std::fs::read(w.phone_folder.join(&entry)).unwrap();
        w.phone.session(&w.phone_folder, |e| e.close(DUE + 10));
        assert!(phone_sends(&w, DUE + 20, false).problem.is_none());
        let current = std::fs::read(w.phone_folder.join(&entry)).unwrap();
        assert_eq!(std::fs::read(w.server.join(&entry)).unwrap(), current);
        assert_ne!(older.len(), current.len());
        // Put back there; the desk changes too, so that the folder's listing is looked at again.
        std::fs::write(w.server.join(&entry), &older).unwrap();
        w.desk.exchange(&w.server, DUE + 30);
        assert!(w.pull(DUE + 40).problem.is_none());
        assert_eq!(State::load(&w.phone.memory).own_entry.map(|(size, _)| size), Some(older.len() as u64));
        assert!(phone_sends(&w, DUE + 50, false).problem.is_none());
        assert_eq!(std::fs::read(w.server.join(&entry)).unwrap(), current, "sent over the copy put back");
    }

    /// The owner's phone, 6 October 2026, 23:30: the folder found on the
    /// server at night, the phone in the background, its folder telling the
    /// desk closed hours ago (eDrive left it so) while the desk is in use.
    /// The first pull comes at once; then the desk is known in use, and
    /// followed once a minute, night or not; closed, at the night's pace.
    #[test]
    fn found_at_night_the_first_pull_comes_at_once() {
        let w = World::new("night");
        w.desk.press(true, DUE + 30);
        w.desk.exchange(&w.server, DUE + 60);
        w.up();
        let in_use = |now: i64| crate::devices::all(&w.phone_folder, &KEY).0.iter().any(|e| e.id == w.desk.id && e.working && !e.left && now - e.exported <= sioul_core::health::FRESH + sioul_core::health::SKEW);
        let night = |now: i64| Pace { phone: true, shown: false, in_use: in_use(now), urgent: false, step: 15 * 60 };
        assert!(!in_use(DUE + 70), "the stale folder tells the desk not in use");
        let state = w.find(DUE + 70);
        assert!(state.confirmed_for(&w.phone_folder));
        attach(&w.phone_folder, &w.phone.memory);
        assert!(due(&State::load(&w.phone.memory), DUE + 80, night(DUE + 80)), "found: pulled at once, whatever the hour");
        let pulled = w.pull(DUE + 80);
        assert!(pulled.problem.is_none() && pulled.listed > 0 && pulled.fetched > 0, "{pulled:?}");
        assert!(in_use(DUE + 90), "the desk known in use, from the server");
        w.phone.exchange(&w.phone_folder, DUE + 90);
        assert!(w.phone.dnd().is_some_and(|p| p.on), "its do-not-disturb too");
        let state = State::load(&w.phone.memory);
        assert!(!due(&state, DUE + 100, night(DUE + 100)), "not twice in a minute");
        w.desk.exchange(&w.server, DUE + 130);
        assert!(w.pull(DUE + 140).problem.is_none(), "once a minute while the desk is in use, at night too");
        // The desk closes: at the night's pace again, fifteen minutes.
        w.desk.session(&w.server, |e| e.close(DUE + 150));
        assert!(w.pull(DUE + 200).problem.is_none());
        let state = State::load(&w.phone.memory);
        assert!(!in_use(DUE + 260) && !due(&state, DUE + 260, night(DUE + 260)));
        assert!(due(&state, DUE + 200 + 15 * 60, night(DUE + 200 + 15 * 60)), "a device opened at night is noticed within the step's fifteen minutes");
    }

    #[test]
    fn the_pace_of_pulls() {
        let state = State { confirmed: 1_000, tried: 1_000, ..State::default() };
        let pace = |phone, shown, in_use, urgent| Pace { phone, shown, in_use, urgent, step: 5 * 60 };
        // A computer, a phone on its screen or another device in use: once a minute.
        assert!(due(&state, 1_060, pace(false, false, false, false)) && !due(&state, 1_059, pace(false, false, false, false)));
        assert!(due(&state, 1_060, pace(true, true, false, false)));
        assert!(due(&state, 1_060, pace(true, false, true, false)));
        // A phone in the background, nobody at another device: at its step's pace.
        assert!(!due(&state, 1_120, pace(true, false, false, false)) && due(&state, 1_270, pace(true, false, false, false)));
        // Asked for news: twenty seconds after the last.
        assert!(due(&state, 1_020, pace(true, false, false, true)) && !due(&state, 1_010, pace(true, false, false, true)));
        // Found again since the last pull: at once.
        assert!(due(&State { confirmed: 1_005, ..state }, 1_006, pace(true, false, false, false)));
    }

    /// A phone's background step leaves the sync app alone only while its own
    /// pull works: on, confirmed for this folder, gone through, and not failing.
    #[test]
    fn the_pull_alone_is_trusted_only_while_it_goes_through() {
        let folder = Path::new("/storage/emulated/0/Documents/Sioul");
        let well = State { folder: shown(folder), url: "https://cloud.example.org/remote.php/dav/files/jane/Documents/Sioul/".into(), confirmed: 1_000, last: 1_200, tried: 1_200, ..State::default() };
        assert!(well.pulls_well(folder));
        assert!(!State { on: Some(false), ..well.clone() }.pulls_well(folder), "switched off here");
        assert!(State { on: Some(true), ..well.clone() }.pulls_well(folder));
        assert!(!State { confirmed: 0, ..well.clone() }.pulls_well(folder), "not confirmed");
        assert!(!State { last: 0, ..well.clone() }.pulls_well(folder), "never pulled yet");
        assert!(!State { said: "network:cloud.example.org".into(), ..well.clone() }.pulls_well(folder), "the last pull failed");
        assert!(!State { said: "not-found:cloud.example.org".into(), ..well.clone() }.pulls_well(folder), "looked for again, not found");
        assert!(!State { mode: MIRROR.into(), ..well.clone() }.pulls_well(folder), "Sioul keeping the folder: no sync app to ask anyway");
        assert!(!well.pulls_well(Path::new("/storage/emulated/0/Documents/Other")), "another folder");
    }

    /// A step's requests, a pull's then a send's (each a `Server` of its own,
    /// as `pull` and `send` make them, with their own limits), go through one
    /// connection while it stays open: the process's one agent. On mobile
    /// data, one TLS handshake a step instead of one per pull, per send and
    /// per file sent.
    #[test]
    fn a_steps_requests_go_through_one_connection() {
        use std::io::{BufRead, BufReader, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let (connections, requests) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (counted, served) = (Arc::clone(&connections), Arc::clone(&requests));
        // A server that keeps its connections open (HTTP/1.1's default), as Nextcloud's does.
        std::thread::spawn(move || {
            for stream in listener.incoming().filter_map(Result::ok) {
                counted.fetch_add(1, Ordering::SeqCst);
                let served = Arc::clone(&served);
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut stream = stream;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        let mut length = 0;
                        loop {
                            let mut header = String::new();
                            reader.read_line(&mut header).unwrap();
                            if header.trim().is_empty() {
                                break;
                            }
                            if let Some((name, value)) = header.split_once(':')
                                && name.eq_ignore_ascii_case("content-length")
                            {
                                length = value.trim().parse().unwrap();
                            }
                        }
                        let mut body = vec![0; length];
                        reader.read_exact(&mut body).unwrap();
                        served.fetch_add(1, Ordering::SeqCst);
                        let (status, answer) = match line.split_whitespace().next() {
                            Some("PROPFIND") => ("207 Multi-Status", super::fake::multistatus("")),
                            Some("GET") => ("200 OK", b"version = 1\n".to_vec()),
                            _ => ("201 Created", Vec::new()),
                        };
                        let head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nETag: \"e\"\r\n\r\n", answer.len());
                        if stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(&answer)).is_err() {
                            return;
                        }
                    }
                });
            }
        });
        let login = Login { account: "cloud".into(), url: format!("{base}/remote.php/dav/"), user: "jane".into(), password: Some(PASSWORD.into()) };
        let folder = format!("{base}/remote.php/dav/files/jane/Documents/Sioul/");
        // The pull: a listing and a small file.
        let pull = Server::new(&login, TEST).unwrap();
        pull.list(&folder).unwrap();
        assert!(pull.small(&format!("{folder}seal.toml")).unwrap().is_some());
        // The send after the exchange, with a send's limits: a look and a file sent whole.
        let send = Server::new(&login, Limits { wait: Duration::from_secs(2), stall: Duration::from_secs(3), ..TEST }).unwrap();
        assert!(send.stat(&format!("{folder}devices/")).unwrap().is_none_or(|item| item.etag.is_empty()));
        let (status, _) = send.put_file(&format!("{folder}phone.toml"), b"read = 1\n".to_vec(), None, 1, &sha1_hex(b"read = 1\n")).unwrap();
        assert_eq!(status, 201);
        // The second send of a step.
        let again = Server::new(&login, TEST).unwrap();
        again.list(&folder).unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 5);
        assert_eq!(connections.load(Ordering::SeqCst), 1, "one connection for the whole step");
    }

    #[test]
    fn where_the_folder_may_be_on_the_server() {
        let found = |folder: &str, configs: &[&str]| places(Path::new(folder), &configs.iter().map(|c| c.to_string()).collect::<Vec<_>>());
        // A phone: eDrive carries its storage as the cloud's root, whatever name the storage goes by.
        assert_eq!(found("/storage/emulated/0/Documents/Sioul", &[]), ["Documents/Sioul", "Sioul", "0/Documents/Sioul"]);
        assert_eq!(found("/sdcard/Documents/Sioul/", &[])[0], "Documents/Sioul");
        // The Nextcloud app's own folder: after its account's.
        assert_eq!(found("/storage/emulated/0/Android/media/com.nextcloud.client/nextcloud/jane@murena.io/Shared/Sioul", &[])[0], "Shared/Sioul");
        // A computer: the Nextcloud client's folders, by their target there.
        let cfg = "[Accounts]\n0\\Folders\\1\\localPath=/home/me/Nextcloud/\n0\\Folders\\1\\targetPath=/\n0\\Folders\\2\\localPath=/home/me/Work\n0\\Folders\\2\\targetPath=/Projects/Work\n0\\url=https://cloud.example.org\n";
        assert_eq!(found("/home/me/Nextcloud/Documents/Sioul", &[cfg])[0], "Documents/Sioul");
        assert_eq!(found("/home/me/Work/Sioul", &[cfg])[0], "Projects/Work/Sioul");
        assert_eq!(found("/home/me/Workshop/Sioul", &[cfg]), ["Sioul", "Workshop/Sioul", "me/Workshop/Sioul"], "not inside /home/me/Work");
        // Addresses and names.
        assert_eq!(encode_path("Mes documents/Sioul"), "Mes%20documents/Sioul");
        assert_eq!(host_of("https://jane@murena.io/remote.php/dav/"), "murena.io");
        assert_eq!(kind_of("devices/0f8fad5b-d9cb-469f-a165-70867728950e.device").map(|k| k.0), Some(Kind::Device));
        assert_eq!(kind_of("0f8fad5b-d9cb-469f-a165-70867728950e-12.jsonl").map(|k| k.0), Some(Kind::Round));
        assert_eq!(kind_of("0f8fad5b-d9cb-469f-a165-70867728950e-12 (conflicted copy).jsonl"), None);
        assert_eq!(kind_of("leases/health/desktop.lease").map(|k| k.0), Some(Kind::Claim));
    }

    #[test]
    fn found_on_the_account_given_by_hand_or_not_at_all() {
        let w = World::new("given");
        // Given by hand, a path in the account's files.
        State::choose(&w.phone.memory, |s| s.given = "Documents/Sioul".into()).unwrap();
        let state = find_with(&w.phone.memory, &w.phone_folder, &[w.fake.login()], &[], DUE, TEST);
        assert!(state.confirmed_for(&w.phone_folder), "{state:?}");
        // Given as an address.
        State::choose(&w.phone.memory, |s| s.given = w.fake.folder_url("Documents/Sioul")).unwrap();
        assert!(find_with(&w.phone.memory, &w.phone_folder, &[w.fake.login()], &[], DUE + 1, TEST).confirmed_for(&w.phone_folder));
        // Given wrong: not found, said, and nothing looked for by itself meanwhile.
        State::choose(&w.phone.memory, |s| s.given = "Elsewhere".into()).unwrap();
        let state = find_with(&w.phone.memory, &w.phone_folder, &[w.fake.login()], &places(&w.phone_folder, &[]), DUE + 2, TEST);
        assert!(state.said.starts_with("not-found:") && !state.confirmed_for(&w.phone_folder), "{state:?}");
        State::choose(&w.phone.memory, |s| s.given.clear()).unwrap();
        // A refused password, said; no password kept, said; no account, said.
        let wrong = Login { password: Some("wrong".into()), ..w.fake.login() };
        assert!(find_with(&w.phone.memory, &w.phone_folder, &[wrong], &places(&w.phone_folder, &[]), DUE + 3, TEST).said.starts_with("login:"));
        let none = Login { password: None, ..w.fake.login() };
        assert_eq!(find_with(&w.phone.memory, &w.phone_folder, &[none], &[], DUE + 4, TEST).said, "no-password:cloud");
        assert_eq!(find_with(&w.phone.memory, &w.phone_folder, &[], &[], DUE + 5, TEST).said, "no-account");
        // A server of another kind (no Nextcloud files): not looked on.
        let other = Login { url: format!("{}/dav/calendars/jane/", w.fake.base), ..w.fake.login() };
        assert_eq!(find_with(&w.phone.memory, &w.phone_folder, &[other], &places(&w.phone_folder, &[]), DUE + 6, TEST).said, "no-account");
        // When to look again: at once when asked, else six hours on.
        let state = State::load(&w.phone.memory);
        assert!(!state.look_due(&w.phone_folder, DUE + 7) && state.look_due(&w.phone_folder, DUE + 6 + LOOK_AGAIN));
        State::choose(&w.phone.memory, |s| s.asked = DUE + 8).unwrap();
        assert!(State::load(&w.phone.memory).look_due(&w.phone_folder, DUE + 9));
    }

    #[test]
    fn your_choices_are_never_written_over_by_a_pull() {
        let w = World::new("choices");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        // A pull holds the state while it works; you switch it off meanwhile: off stays.
        let held = State::load(&w.phone.memory);
        State::choose(&w.phone.memory, |s| s.on = Some(false)).unwrap();
        held.save(&w.phone.memory).unwrap();
        assert_eq!(State::load(&w.phone.memory).on, Some(false));
        State::choose(&w.phone.memory, |s| s.on = None).unwrap();
        assert!(State::load(&w.phone.memory).fetching());
    }

    // ------------------------------------------------ kept in step by Sioul itself

    /// Two devices with no sync app at all, each keeping its own copy of the
    /// folder in step with the server through Sioul alone (`MIRROR`).
    struct Pair {
        base: PathBuf,
        fake: Arc<Fake>,
        desk: Device,
        desk_folder: PathBuf,
        phone: Device,
        phone_folder: PathBuf,
    }

    impl Pair {
        fn new(name: &str) -> Pair {
            let base = scratch(name);
            let files = base.join("cloud");
            std::fs::create_dir_all(&files).unwrap();
            let fake = Fake::start(&files);
            let desk = Device::new(&base, "desk", COMPUTER);
            let phone = Device::new(&base, "phone", PHONE);
            let (desk_folder, phone_folder) = (mirror_of(&desk.memory), mirror_of(&phone.memory));
            let login = fake.login();
            // The desk starts sharing there: the folder made, its seal sent.
            let opened = open_with(&login, "Documents/Sioul", TEST).unwrap();
            assert!(opened.seal.is_none(), "nothing there yet");
            put(&desk_folder, "seal.toml", SEAL);
            create_with(&login, &opened, &desk_folder, TEST).unwrap();
            begin(&desk.memory, &desk_folder, &login, &opened, DUE - 7_300).unwrap();
            // A second start there at once finds it sealed: never sealed twice.
            assert_eq!(create_with(&login, &opened, &desk_folder, TEST), Err("sealed-meanwhile".to_string()));
            // The phone joins it: the seal brought into its own copy.
            let joined = open_with(&login, "/Documents/Sioul/", TEST).unwrap();
            assert_eq!(joined.seal.as_deref(), Some(SEAL.as_bytes()));
            put_bytes(&phone_folder, "seal.toml", joined.seal.as_deref().unwrap());
            begin(&phone.memory, &phone_folder, &login, &joined, DUE - 7_250).unwrap();
            let pair = Pair { base, fake, desk, desk_folder, phone, phone_folder };
            pair.desk.session(&pair.desk_folder, |e| e.start(DUE - 7_200));
            pair.phone.session(&pair.phone_folder, |e| e.start(DUE - 7_200));
            pair.turn(true, DUE - 7_100);
            pair.turn(false, DUE - 7_090);
            pair.turn(true, DUE - 7_080);
            pair
        }

        fn device(&self, desk: bool) -> (&Device, &Path) {
            if desk { (&self.desk, &self.desk_folder) } else { (&self.phone, &self.phone_folder) }
        }

        fn step(&self, desk: bool, now: i64, whole: bool) -> Pulled {
            let (device, folder) = self.device(desk);
            step_with(&device.memory, folder, &device.id, &self.fake.login(), now, whole, TEST)
        }

        /// As the window does each minute: the server looked through, an exchange, what it wrote sent.
        fn turn(&self, desk: bool, now: i64) -> Outcome {
            let before = self.step(desk, now, true);
            assert!(before.problem.is_none(), "{before:?}");
            let outcome = self.device(desk).0.exchange(self.device(desk).1, now);
            let after = self.step(desk, now, false);
            assert!(after.problem.is_none(), "{after:?}");
            outcome
        }
    }

    impl Drop for Pair {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn two_devices_without_a_sync_app_share_through_the_server_alone() {
        let p = Pair::new("mirror");
        // Do-not-disturb turned on at the desk: on the phone at its next turn.
        p.desk.press(true, DUE + 10);
        p.turn(true, DUE + 20);
        p.turn(false, DUE + 30);
        let press = p.phone.dnd().expect("the desk's press");
        assert!(press.on && press.from == p.desk.id, "{press:?}");
        // A dose marked taken on the phone: on the desk at its next turn.
        p.phone.write("health-state.toml", "[taken]\n\"levo@1800000000\" = 1800000040\n");
        p.turn(false, DUE + 40);
        p.turn(true, DUE + 50);
        assert!(p.desk.read("health-state.toml").contains("levo@1800000000"));
        // Turned off on the phone: off on the desk.
        p.phone.press(false, DUE + 60);
        p.turn(false, DUE + 70);
        p.turn(true, DUE + 80);
        assert!(!p.desk.dnd().unwrap().on);
        // Each knows how the other is, from the server alone.
        let phone = p.desk.sees(&p.desk_folder, &p.phone, DUE + 90);
        assert!(phone.complete && phone.said.as_ref().is_some_and(|s| s.working && s.exported >= DUE + 70), "{phone:?}");
        assert!(doubts_now(DUE - 100, DUE + 90, None, &[phone]).is_empty(), "a dose due before its last export is known");
        // Neither the backup nor its reading beside the folder ever run on a folder Sioul keeps itself.
        assert_eq!(pull_with(&p.phone.memory, &p.phone_folder, &p.phone.id, &p.fake.login(), DUE + 95, TEST).problem.as_deref(), Some("mirror"));
        assert!(attach(&p.phone_folder, &p.phone.memory).is_none());
    }

    #[test]
    fn a_device_never_writes_another_devices_file() {
        let p = Pair::new("own");
        for minute in 0..6 {
            for desk in [true, false] {
                let device = p.device(desk).0;
                if minute % 2 == 0 {
                    device.press(minute % 4 == 0, DUE + minute * 60 + 1);
                }
                p.fake.forget_seen();
                p.turn(desk, DUE + minute * 60 + if desk { 0 } else { 30 });
                for line in p.fake.seen() {
                    let path = line.split(' ').nth(1).unwrap_or_default();
                    if line.starts_with("PUT ") || line.starts_with("DELETE ") {
                        assert!(path.contains(&device.id) || path.contains("/blobs/"), "{line}");
                    } else {
                        assert!(line.starts_with("GET ") || line.starts_with("PROPFIND ") || line.starts_with("MKCOL "), "{line}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_refused_or_failed_upload_is_never_lost() {
        let p = Pair::new("refused");
        p.phone.write("health-state.toml", "[taken]\n\"levo@1800000000\" = 1800000040\n");
        for (n, fault) in [Fault::Status(412), Fault::Status(500), Fault::Drop].into_iter().enumerate() {
            p.fake.clear_faults();
            let id = p.phone.id.clone();
            p.fake.fault(move |r| (r.method == "PUT" && r.path.contains(&id) && r.path.ends_with(".jsonl")).then_some(fault));
            let now = DUE + 40 + n as i64 * 10;
            // Its step looks through the server, and sends what waits: refused again.
            let _ = p.step(false, now, true);
            p.phone.exchange(&p.phone_folder, now);
            let pushed = p.step(false, now, false);
            assert!(pushed.problem.is_some(), "{fault:?}: {pushed:?}");
            assert!(p.phone.read("health-state.toml").contains("levo@"), "nothing lost here");
            p.turn(true, now + 5);
            assert!(!p.desk.read("health-state.toml").contains("levo@"), "{fault:?}: not there yet");
        }
        // The server takes it again: it goes, and the desk has it.
        p.fake.clear_faults();
        p.turn(false, DUE + 100);
        p.turn(true, DUE + 110);
        assert!(p.desk.read("health-state.toml").contains("levo@1800000000"));
        let heard = crate::share::heard(&p.desk.memory, &p.desk.id);
        assert!(!heard.broken.contains_key(&p.phone.id), "{heard:?}");
    }

    #[test]
    fn its_copy_lost_a_device_goes_on_after_its_last_record() {
        let p = Pair::new("lost");
        p.phone.write("health-state.toml", "[taken]\n\"one@1800000000\" = 1800000010\n");
        p.turn(false, DUE + 20);
        p.turn(true, DUE + 25);
        // The phone's copy of the folder lost, its memory kept: its records come back from the server, numbering goes on.
        for entry in std::fs::read_dir(&p.phone_folder).unwrap().filter_map(Result::ok) {
            if entry.file_name() != "seal.toml" {
                let _ = std::fs::remove_dir_all(entry.path());
                let _ = std::fs::remove_file(entry.path());
            }
        }
        p.phone.write("health-state.toml", "[taken]\n\"one@1800000000\" = 1800000010\n\"two@1800000000\" = 1800000030\n");
        let outcome = p.turn(false, DUE + 40);
        assert!(outcome.problems.iter().all(|p| !p.starts_with("share-own")), "{:?}", outcome.problems);
        p.turn(true, DUE + 50);
        let record = p.desk.read("health-state.toml");
        assert!(record.contains("one@") && record.contains("two@"), "{record}");
        let heard = crate::share::heard(&p.desk.memory, &p.desk.id);
        assert!(!heard.broken.contains_key(&p.phone.id), "no gap nor cut on the desk: {heard:?}");
        // The others' files came back too.
        assert!(p.phone_folder.join(format!("devices/{}.device", p.desk.id)).exists());
    }

    #[test]
    fn sealed_files_travel_whole_and_go_when_swept() {
        let p = Pair::new("blobs");
        let source = p.base.join("note.md");
        std::fs::write(&source, "a note\n".repeat(1000)).unwrap();
        let (hash, _) = crate::blobs::hash_file(&source).unwrap();
        crate::blobs::put(&p.desk_folder, &KEY, &source, &hash).unwrap();
        let name = crate::blobs::name(&KEY, &hash);
        p.turn(true, DUE + 10);
        p.turn(false, DUE + 20);
        let sealed = std::fs::read(p.desk_folder.join("blobs").join(&name)).unwrap();
        assert_eq!(std::fs::read(p.phone_folder.join("blobs").join(&name)).unwrap(), sealed);
        // Never sent back by the phone, which did not make it.
        p.fake.forget_seen();
        p.turn(false, DUE + 30);
        assert!(p.fake.seen().iter().all(|l| !(l.starts_with("PUT ") && l.contains("/blobs/"))), "{:?}", p.fake.seen());
        // Swept on the desk: taken out there.
        std::fs::remove_file(p.desk_folder.join("blobs").join(&name)).unwrap();
        p.turn(true, DUE + 40);
        assert!(!p.fake.files.join("Documents/Sioul/blobs").join(&name).exists());
    }

    /// A sealed file going up slowly or not at all, or coming down slowly,
    /// never holds back a device's records: its own records go up first, the
    /// doses' answers among them, sealed files last (the spam filter's table
    /// here, made by a training on the desk).
    #[test]
    fn a_slow_or_failing_sealed_file_never_holds_back_the_records() {
        let p = Pair::new("sealed-last");
        let made = |seed: u32| {
            let mut x = seed.wrapping_mul(2_654_435_761) | 1;
            (0..600_000)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    x as u8
                })
                .collect::<Vec<u8>>()
        };
        let table = p.desk.roots.data.join("spam").join("table.bin");
        std::fs::create_dir_all(table.parent().unwrap()).unwrap();
        std::fs::write(&table, made(1)).unwrap();
        let mut taken = String::from("[taken]\n");
        // Late past the time a file's size asks (an upload's answer is awaited that long, `put_file`).
        for (n, fault) in [Fault::Status(500), Fault::Drop, Fault::Late(Duration::from_secs(6))].into_iter().enumerate() {
            p.fake.clear_faults();
            p.fake.fault(move |r| (r.method == "PUT" && r.path.contains("/blobs/")).then_some(fault));
            let now = DUE + 100 + n as i64 * 60;
            // A dose marked on the desk, beside its new table that cannot go up.
            taken.push_str(&format!("\"levo@{}\" = {}\n", 1_800_000_000 + n, 1_800_000_040 + n));
            p.desk.write("health-state.toml", &taken);
            let _ = p.step(true, now, true);
            p.desk.exchange(&p.desk_folder, now);
            let pushed = p.step(true, now, false);
            assert!(pushed.problem.is_some(), "{fault:?}: {pushed:?}");
            // The phone has the dose all the same; the table waits for its sealed file.
            p.turn(false, now + 30);
            assert!(p.phone.read("health-state.toml").contains(&format!("levo@{}", 1_800_000_000 + n)), "{fault:?}");
        }
        // The server takes it: it goes, and comes.
        p.fake.clear_faults();
        p.turn(true, DUE + 400);
        p.turn(false, DUE + 410);
        let phone_table = p.phone.roots.data.join("spam").join("table.bin");
        assert_eq!(std::fs::read(&phone_table).unwrap(), std::fs::read(&table).unwrap());
        // A new table coming down slowly to the phone: the phone's own answer went up before it.
        std::fs::write(&table, made(2)).unwrap();
        p.turn(true, DUE + 500);
        p.phone.write("health-state.toml", &format!("{taken}\"two@1800000000\" = 1800000090\n"));
        p.phone.exchange(&p.phone_folder, DUE + 505);
        p.fake.fault(|r| (r.method == "GET" && r.path.contains("/blobs/")).then_some(Fault::Late(Duration::from_secs(2))));
        let stepped = p.step(false, DUE + 510, true);
        assert!(stepped.problem.is_some(), "{stepped:?}");
        p.fake.clear_faults();
        p.turn(true, DUE + 520);
        assert!(p.desk.read("health-state.toml").contains("two@1800000000"));
        // The table comes at the phone's next step: looked for again at once.
        p.turn(false, DUE + 530);
        assert_eq!(std::fs::read(&phone_table).unwrap(), std::fs::read(&table).unwrap());
    }

    /// A device beside its sync app (the desk: its folder is the server's) and
    /// a phone with none, Sioul keeping its copy: one folder, both ways.
    #[test]
    fn a_device_with_a_sync_app_and_one_without_share_one_folder() {
        let w = World::new("mixed");
        let lone = Device::new(&w.base, "lone", PHONE);
        let folder = mirror_of(&lone.memory);
        let login = w.fake.login();
        let opened = open_with(&login, "Documents/Sioul", TEST).unwrap();
        put_bytes(&folder, "seal.toml", opened.seal.as_deref().expect("sealed by the desk"));
        begin(&lone.memory, &folder, &login, &opened, DUE).unwrap();
        lone.session(&folder, |e| e.start(DUE));
        let turn = |now: i64| {
            assert!(step_with(&lone.memory, &folder, &lone.id, &login, now, true, TEST).problem.is_none());
            lone.exchange(&folder, now);
            assert!(step_with(&lone.memory, &folder, &lone.id, &login, now, false, TEST).problem.is_none());
        };
        turn(DUE + 10);
        // Do-not-disturb turned on at the desk: the phone without a sync app has it.
        w.desk.press(true, DUE + 20);
        w.desk.exchange(&w.server, DUE + 30);
        turn(DUE + 40);
        assert!(lone.dnd().is_some_and(|p| p.on && p.from == w.desk.id));
        // A dose answered on that phone: the desk reads it in its own folder.
        lone.write("health-state.toml", "[taken]\n\"levo@1800000000\" = 1800000050\n");
        turn(DUE + 50);
        w.desk.exchange(&w.server, DUE + 60);
        assert!(w.desk.read("health-state.toml").contains("levo@1800000000"));
        let seen = w.desk.sees(&w.server, &lone, DUE + 70);
        assert!(seen.complete && seen.said.is_some_and(|s| s.working), "the desk knows how the phone is");
    }

    #[test]
    fn keeping_the_folder_is_inert_until_set_up() {
        let w = World::new("inert");
        w.fake.forget_seen();
        let stepped = step_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), DUE, true, TEST);
        assert_eq!(stepped.problem.as_deref(), Some("not-confirmed"));
        assert!(w.fake.seen().is_empty(), "not a request");
        // The backup confirmed is not Sioul keeping the folder: nothing sent either.
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        w.fake.forget_seen();
        assert_eq!(step_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), DUE + 1, false, TEST).problem.as_deref(), Some("not-confirmed"));
        assert!(w.fake.seen().is_empty());
    }

    // ------------------------------------------------ sent beside the sync app

    fn mtime(path: &Path) -> Option<SystemTime> {
        std::fs::metadata(path).and_then(|m| m.modified()).ok()
    }

    fn set_mtime(path: &Path, at: SystemTime) {
        if let Ok(file) = std::fs::File::options().write(true).open(path) {
            let _ = file.set_modified(at);
        }
    }

    /// A file written there by a sync app, as a `PUT` writes it: a new ETag, dated as the file here.
    fn sync_up(local: &Path, server: &Path) {
        std::fs::create_dir_all(server.parent().unwrap()).unwrap();
        std::fs::write(server, std::fs::read(local).unwrap()).unwrap();
        fake::bump(server);
        if let Some(at) = mtime(local) {
            set_mtime(server, at);
        }
    }

    /// A file brought down by a sync app: a new file put in its place, dated as the server's.
    fn sync_down(server: &Path, local: &Path) {
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        let temporary = local.with_file_name(format!(".{}.download", local.file_name().unwrap().to_string_lossy()));
        std::fs::write(&temporary, std::fs::read(server).unwrap()).unwrap();
        if let Some(at) = mtime(server) {
            set_mtime(&temporary, at);
        }
        std::fs::rename(&temporary, local).unwrap();
    }

    fn folder_of(relative: &str) -> &str {
        relative.rfind('/').map_or("", |at| &relative[..at])
    }

    /// Murena's eDrive as its source reads (eDrive `main`, 31 August 2026;
    /// docs/database.md, "Sent to the server too"): the server's files first,
    /// an ETag changed there brought down when the size differs, else taken
    /// as known; then the phone's, a time later than the one known sent up,
    /// `If-Match` the ETag it knows (the Documents folder is a "media"
    /// folder), replacing the download planned. A folder whose time did not
    /// change is not looked at here (a file appended in place changes none):
    /// the download planned then goes ahead over the phone's newer file. A
    /// refused upload holds the file back, and after four, for good. Never a
    /// conflicted copy.
    #[derive(Default)]
    struct EDrive {
        known: BTreeMap<String, (String, Option<SystemTime>)>,
        dirs: BTreeMap<String, Option<SystemTime>>,
        refused: BTreeMap<String, u32>,
        uploads: usize,
        refusals: usize,
        downloads_over_newer: usize,
    }

    impl EDrive {
        fn scan(&mut self, fake: &Fake, server: &Path, phone: &Path) {
            let _turn = fake.turn.lock().unwrap();
            let (there, here) = (walk(server), walk(phone));
            let mut downloads: BTreeSet<String> = BTreeSet::new();
            for (relative, bytes) in there.iter().filter(|(r, _)| *r != "seal.toml") {
                let etag = fake::etag_of(&server.join(relative));
                if self.known.get(relative).is_some_and(|(known, _)| *known == etag) {
                    continue;
                }
                if here.get(relative).map(Vec::len) == Some(bytes.len()) {
                    let time = self.known.get(relative).and_then(|(_, t)| *t);
                    self.known.insert(relative.clone(), (etag, time));
                } else {
                    downloads.insert(relative.clone());
                }
            }
            let mut uploads: Vec<String> = Vec::new();
            for relative in here.keys().filter(|r| *r != "seal.toml") {
                let dir = folder_of(relative).to_string();
                let looked = self.dirs.get(&dir).copied().flatten() != mtime(&phone.join(&dir));
                let later = self.known.get(relative).is_none_or(|(_, t)| mtime(&phone.join(relative)) > *t);
                if looked && later {
                    downloads.remove(relative);
                    uploads.push(relative.clone());
                }
            }
            for relative in downloads {
                if here.get(&relative).is_some_and(|mine| there[&relative].len() < mine.len()) {
                    self.downloads_over_newer += 1;
                }
                sync_down(&server.join(&relative), &phone.join(&relative));
                self.known.insert(relative.clone(), (fake::etag_of(&server.join(&relative)), mtime(&phone.join(&relative))));
            }
            for relative in uploads {
                self.upload(server, phone, &relative);
            }
            self.dirs = here.keys().map(|r| folder_of(r).to_string()).chain([String::new()]).map(|d| (d.clone(), mtime(&phone.join(&d)))).collect();
        }

        /// One file sent up, as eDrive sends it: `If-Match` the ETag it knows.
        fn upload(&mut self, server: &Path, phone: &Path, relative: &str) {
            if self.refused.get(relative).is_some_and(|n| *n >= 4) {
                return;
            }
            let there = server.join(relative);
            if let Some((known, _)) = self.known.get(relative)
                && there.is_file()
                && fake::etag_of(&there) != *known
            {
                self.refusals += 1;
                *self.refused.entry(relative.to_string()).or_insert(0) += 1;
                return;
            }
            sync_up(&phone.join(relative), &there);
            self.uploads += 1;
            self.known.insert(relative.to_string(), (fake::etag_of(&there), mtime(&phone.join(relative))));
        }

        /// A file closed after writing (`CLOSE_WRITE`), sent at once.
        fn closed(&mut self, fake: &Fake, server: &Path, phone: &Path, relative: &str) {
            let _turn = fake.turn.lock().unwrap();
            if phone.join(relative).is_file() {
                self.upload(server, phone, relative);
            }
        }
    }

    /// The Nextcloud desktop client as its source reads (v34; docs/database.md,
    /// "Sent to the server too"): a file changed on one side only goes the
    /// other way (`If-Match`); changed on both, the contents compared: the
    /// same, it takes the server's as its own; else the file here is renamed
    /// "(conflicted copy …)", which never goes up, and the server's takes its
    /// name. Hidden files left out.
    #[derive(Default)]
    struct Client {
        journal: BTreeMap<String, (String, u64, Option<SystemTime>)>,
        conflicts: usize,
        uploads: usize,
    }

    impl Client {
        fn stamp(path: &Path) -> Option<(u64, Option<SystemTime>)> {
            std::fs::metadata(path).ok().filter(|m| m.is_file()).map(|m| (m.len(), m.modified().ok()))
        }

        fn sync(&mut self, fake: &Fake, server: &Path, local: &Path) {
            let _turn = fake.turn.lock().unwrap();
            let names: BTreeSet<String> = walk(server).into_keys().chain(walk(local).into_keys()).filter(|r| r != "seal.toml" && !r.contains("(conflicted copy")).collect();
            for relative in names {
                let (there, here) = (server.join(&relative), local.join(&relative));
                let etag = there.is_file().then(|| fake::etag_of(&there));
                let stamp = Self::stamp(&here);
                let known = self.journal.get(&relative).cloned();
                let local_changed = known.as_ref().map(|(_, size, at)| Some((*size, *at))) != Some(stamp);
                let remote_changed = known.as_ref().map(|(e, ..)| e.clone()) != etag;
                let remember = |journal: &mut BTreeMap<String, (String, u64, Option<SystemTime>)>| {
                    let (size, at) = Self::stamp(&here).unwrap_or((0, None));
                    journal.insert(relative.clone(), (fake::etag_of(&there), size, at));
                };
                match (stamp.is_some(), etag.is_some()) {
                    (true, true) if !local_changed && !remote_changed => {}
                    (true, true) if !remote_changed => {
                        sync_up(&here, &there);
                        self.uploads += 1;
                        remember(&mut self.journal);
                    }
                    (true, true) if !local_changed => {
                        sync_down(&there, &here);
                        remember(&mut self.journal);
                    }
                    (true, true) => {
                        if std::fs::read(&here).ok() != std::fs::read(&there).ok() {
                            let name = here.file_name().unwrap().to_string_lossy().to_string();
                            let (stem, extension) = name.rsplit_once('.').unwrap_or((&name, ""));
                            std::fs::rename(&here, here.with_file_name(format!("{stem} (conflicted copy 20261008 104200).{extension}"))).unwrap();
                            sync_down(&there, &here);
                            self.conflicts += 1;
                        }
                        remember(&mut self.journal);
                    }
                    (true, false) if known.is_some() && !local_changed => {
                        let _ = std::fs::remove_file(&here);
                        self.journal.remove(&relative);
                    }
                    (true, false) => {
                        sync_up(&here, &there);
                        self.uploads += 1;
                        remember(&mut self.journal);
                    }
                    (false, true) if known.is_some() && !remote_changed => {
                        let _ = std::fs::remove_file(&there);
                        self.journal.remove(&relative);
                    }
                    (false, true) => {
                        sync_down(&there, &here);
                        remember(&mut self.journal);
                    }
                    (false, false) => {
                        self.journal.remove(&relative);
                    }
                }
            }
        }
    }

    /// The sizes of a device's records on the server, which must only grow.
    fn records_there(server: &Path, id: &str) -> BTreeMap<String, usize> {
        walk(server).into_iter().filter(|(r, _)| r.starts_with(id) && r.ends_with(".jsonl")).map(|(r, b)| (r, b.len())).collect()
    }

    fn never_shorter(before: &mut BTreeMap<String, usize>, now: BTreeMap<String, usize>, at: &str) {
        for (name, size) in &now {
            assert!(before.get(name).is_none_or(|was| size >= was), "{at}: {name} shorter there: {size} after {:?}", before.get(name));
        }
        before.extend(now);
    }

    /// The phone's own files sent beside its sync app.
    fn phone_sends(w: &World, now: i64, again: bool) -> Pulled {
        send_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), &BTreeMap::new(), now, again, TEST)
    }

    /// This device's own files go up at once, beside the sync app; the sync
    /// app sending the same file after is a no-op here; a file it sent before
    /// Sioul is taken as sent (`412`, then the same there); records never
    /// sent shorter; switched off, not a request; never another device's file.
    #[test]
    fn beside_a_sync_app_only_this_devices_files_go_up_and_never_twice() {
        let w = World::new("beside");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        // A dose answered on the phone: its records and entry on the server within the send.
        w.phone.write("health-state.toml", "[taken]\n\"levo@1800000000\" = 1800000040\n");
        w.phone.exchange(&w.phone_folder, DUE + 40);
        w.fake.forget_seen();
        let sent = phone_sends(&w, DUE + 41, false);
        assert!(sent.problem.is_none() && sent.sent >= 2, "{sent:?}");
        let mine = |line: &String| line.split(' ').nth(1).is_some_and(|path| path.contains(&w.phone.id));
        for line in w.fake.seen() {
            if line.starts_with("PUT ") || line.starts_with("DELETE ") {
                assert!(mine(&line), "never another device's file: {line}");
            }
        }
        for (relative, bytes) in walk(&w.phone_folder).into_iter().filter(|(r, _)| r.contains(&w.phone.id)) {
            assert_eq!(std::fs::read(w.server.join(&relative)).ok(), Some(bytes), "{relative} there as here");
        }
        let kept = std::fs::read_to_string(sending_path(&w.phone.memory)).unwrap();
        assert!(!kept.contains(PASSWORD) && !kept.contains("Basic"), "{kept}");
        w.desk.exchange(&w.server, DUE + 45);
        assert!(w.desk.read("health-state.toml").contains("levo@1800000000"), "the desk has it, no sync app between");
        // The sync app sends them again, the same: nothing more from Sioul, not a request.
        w.up();
        for relative in walk(&w.phone_folder).into_keys().filter(|r| r.contains(&w.phone.id)) {
            fake::bump(&w.server.join(relative));
        }
        w.fake.forget_seen();
        let again = phone_sends(&w, DUE + 50, false);
        assert_eq!((again.sent, &again.problem), (0, &None));
        assert!(w.fake.seen().is_empty(), "{:?}", w.fake.seen());
        // Another answer: the sync app sends it first; Sioul finds it there (412, the same) and sends nothing twice.
        w.phone.write("health-state.toml", "[taken]\n\"levo@1800000000\" = 1800000040\n\"iron@1800000000\" = 1800000060\n");
        w.phone.exchange(&w.phone_folder, DUE + 60);
        {
            let _turn = w.fake.turn.lock().unwrap();
            for relative in walk(&w.phone_folder).into_keys().filter(|r| r.contains(&w.phone.id)) {
                sync_up(&w.phone_folder.join(&relative), &w.server.join(&relative));
            }
        }
        w.fake.forget_seen();
        let raced = phone_sends(&w, DUE + 61, false);
        assert!(raced.problem.is_none() && raced.sent == 0 && raced.same >= 2, "{raced:?} {:?}", w.fake.seen());
        assert!(w.fake.seen().iter().all(|l| !(l.starts_with("PUT ") && !l.ends_with(" 412"))), "{:?}", w.fake.seen());
        // The phone's records put back shorter here (a sync app's older copy): never sent over the longer one there.
        let round = walk(&w.phone_folder).into_keys().find(|r| r.starts_with(&w.phone.id) && r.ends_with(".jsonl")).unwrap();
        let longer = std::fs::read(w.server.join(&round)).unwrap();
        let first = longer.iter().position(|b| *b == b'\n').unwrap() + 1;
        std::fs::write(w.phone_folder.join(&round), &longer[..first]).unwrap();
        w.fake.forget_seen();
        let cut = phone_sends(&w, DUE + 70, false);
        assert!(cut.problem.is_none(), "{cut:?}");
        assert_eq!(std::fs::read(w.server.join(&round)).unwrap(), longer, "never shorter there");
        assert!(w.fake.seen().iter().all(|l| !(l.starts_with("PUT ") && l.contains(&round))), "{:?}", w.fake.seen());
        // Listed (every six hours), found longer there; then a line appended here, still shorter: never sent either.
        assert_eq!(phone_sends(&w, DUE + 72, true).longer, 1);
        let mut appended = longer[..first].to_vec();
        appended.extend_from_slice(&longer[first..first + (longer.len() - first) / 2]);
        appended.push(b'\n');
        std::fs::write(w.phone_folder.join(&round), &appended).unwrap();
        w.fake.forget_seen();
        assert!(phone_sends(&w, DUE + 75, false).problem.is_none());
        assert_eq!(std::fs::read(w.server.join(&round)).unwrap(), longer, "an append here, still shorter, never sent over the longer copy");
        assert!(w.fake.seen().iter().all(|l| !(l.starts_with("PUT ") && l.contains(&round))), "{:?}", w.fake.seen());
        // Switched off: not a request; the backup's fetching goes on.
        State::choose(&w.phone.memory, |s| s.send = Some(false)).unwrap();
        w.fake.forget_seen();
        assert_eq!(phone_sends(&w, DUE + 80, true).problem.as_deref(), Some("off"));
        assert!(w.fake.seen().is_empty());
        assert!(State::load(&w.phone.memory).fetching());
    }

    /// Sealed files: only those this device sealed, whole, and taken out there once swept.
    #[test]
    fn sealed_files_go_up_whole_and_only_this_devices() {
        let w = World::new("beside-blobs");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        let source = w.base.join("note.md");
        std::fs::write(&source, "a note\n".repeat(500)).unwrap();
        let (hash, _) = crate::blobs::hash_file(&source).unwrap();
        crate::blobs::put(&w.phone_folder, &KEY, &source, &hash).unwrap();
        let name = crate::blobs::name(&KEY, &hash);
        let size = std::fs::metadata(w.phone_folder.join("blobs").join(&name)).unwrap().len();
        // Another device's sealed file, brought down by the sync app: never sent.
        put(&w.phone_folder, "blobs/0123456789abcdef", "another device's");
        // Not whole yet (the size sealed differs): not sent.
        let half: BTreeMap<String, u64> = [(name.clone(), size + 1)].into();
        send_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), &half, DUE + 10, false, TEST);
        assert!(!w.server.join("blobs").join(&name).exists());
        let whole: BTreeMap<String, u64> = [(name.clone(), size)].into();
        let sent = send_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), &whole, DUE + 20, false, TEST);
        assert!(sent.problem.is_none(), "{sent:?}");
        assert_eq!(std::fs::read(w.server.join("blobs").join(&name)).unwrap(), std::fs::read(w.phone_folder.join("blobs").join(&name)).unwrap());
        assert!(!w.server.join("blobs/0123456789abcdef").exists(), "never another device's");
        // Swept here: taken out there.
        std::fs::remove_file(w.phone_folder.join("blobs").join(&name)).unwrap();
        send_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), &BTreeMap::new(), DUE + 30, false, TEST);
        assert!(!w.server.join("blobs").join(&name).exists());
    }

    /// "Send everything again" after the server lost this device's files:
    /// each one goes back, said; a second time, nothing goes, all found there.
    #[test]
    fn send_everything_again_after_the_server_lost_this_devices_files() {
        let w = World::new("again");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        w.phone.write("health-state.toml", "[taken]\n\"levo@1800000000\" = 1800000040\n");
        w.phone.exchange(&w.phone_folder, DUE + 40);
        assert!(phone_sends(&w, DUE + 41, false).problem.is_none());
        let own: Vec<String> = walk(&w.phone_folder).into_keys().filter(|r| r.contains(&w.phone.id)).collect();
        assert!(own.len() >= 2, "{own:?}");
        // The server loses them (restored from a backup, a folder emptied by hand).
        for relative in &own {
            std::fs::remove_file(w.server.join(relative)).unwrap();
        }
        assert!(crate::devices::all(&w.server, &KEY).0.iter().all(|e| e.id != w.phone.id));
        // Nothing changed here: a quick send asks nothing, sends nothing.
        w.fake.forget_seen();
        assert_eq!(phone_sends(&w, DUE + 50, false).sent, 0);
        assert!(w.fake.seen().is_empty());
        // Sent again, whatever was sent before.
        let again = phone_sends(&w, DUE + 60, true);
        assert!(again.problem.is_none() && again.sent == own.len(), "{again:?}");
        for relative in &own {
            assert_eq!(std::fs::read(w.server.join(relative)).ok(), std::fs::read(w.phone_folder.join(relative)).ok(), "{relative}");
        }
        assert_eq!(Sending::load(&w.phone.memory).again, Again { at: DUE + 60, sent: own.len(), same: 0, said: String::new() });
        w.desk.exchange(&w.server, DUE + 70);
        assert!(w.desk.read("health-state.toml").contains("levo@1800000000"));
        assert!(crate::devices::all(&w.server, &KEY).0.iter().any(|e| e.id == w.phone.id), "its entry back");
        // Again: all there, nothing sent.
        w.fake.forget_seen();
        let twice = phone_sends(&w, DUE + 80, true);
        assert!(twice.problem.is_none() && twice.sent == 0 && twice.same == own.len(), "{twice:?}");
        assert!(w.fake.seen().iter().all(|l| !l.starts_with("PUT ")), "{:?}", w.fake.seen());
        // The server unreachable: said, nothing lost here.
        w.fake.fault(|_| Some(Fault::Status(503)));
        let failed = phone_sends(&w, DUE + 90, true);
        assert!(failed.problem.as_deref().is_some_and(|p| p.starts_with("server:")), "{failed:?}");
        assert!(Sending::load(&w.phone.memory).again.said.starts_with("server:"));
    }

    /// The doses' rule (docs/health.md, "Knowing") with the phone's own files
    /// sent by Sioul right after each exchange and by eDrive too, late, and
    /// now and then in the moment between a write and Sioul's send (eDrive
    /// bringing the older copy down over the phone's newer one, or refusing
    /// its own upload): what the desk takes as known is there, every dose
    /// comes, the phone's records there never shorter, no conflicted copy.
    #[test]
    fn doses_are_never_known_wrongly_with_edrive_sending_too() {
        let w = World::new("edrive-sends");
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        let mut edrive = EDrive::default();
        edrive.scan(&w.fake, &w.server, &w.phone_folder);
        let mut marked: Vec<(String, i64)> = Vec::new();
        let mut sizes = BTreeMap::new();
        for minute in 0..150 {
            let now = DUE + minute * 60;
            // Every ten minutes; and twice close together, the second where eDrive looks before Sioul sends.
            if (minute % 10 == 0 || [17, 19, 77, 79].contains(&minute)) && minute < 130 {
                marked.push((format!("dose@{minute}"), now));
                let record: String = marked.iter().map(|(d, at)| format!("\"{d}\" = {at}\n")).collect();
                w.phone.write("health-state.toml", &format!("[taken]\n{record}"));
            }
            w.phone.exchange(&w.phone_folder, now);
            let round = walk(&w.phone_folder).into_keys().filter(|r| r.starts_with(&w.phone.id) && r.ends_with(".jsonl")).max_by_key(|r| r.trim_end_matches(".jsonl").rsplit('-').next().unwrap().parse::<u32>().unwrap()).unwrap();
            match minute % 6 {
                // Between the write and Sioul's send: eDrive looks (asked by another app, its half hour come).
                1 => edrive.scan(&w.fake, &w.server, &w.phone_folder),
                // Its upload of the appended file at once (CLOSE_WRITE), before Sioul's.
                3 => edrive.closed(&w.fake, &w.server, &w.phone_folder, &round),
                _ => {}
            }
            let sent = phone_sends(&w, now + 1, false);
            assert!(sent.problem.is_none(), "minute {minute}: {sent:?}");
            // Then eDrive, asked to look after Sioul's send, or not at all for a while.
            if minute % 4 == 0 {
                edrive.scan(&w.fake, &w.server, &w.phone_folder);
            }
            never_shorter(&mut sizes, records_there(&w.server, &w.phone.id), &format!("minute {minute}"));
            let outcome = w.desk.exchange(&w.server, now + 20);
            assert!(outcome.problems.iter().all(|p| !p.starts_with("share-other-cut") && !p.starts_with("share-other-gap")), "minute {minute}: {:?}", outcome.problems);
            if let Some(entry) = crate::devices::all(&w.server, &KEY).0.into_iter().find(|e| e.id == w.phone.id)
                && let Some(wrote) = entry.wrote
                && crate::share::heard(&w.desk.memory, &w.desk.id).complete(&w.phone.id, wrote)
            {
                let record = w.desk.read("health-state.toml");
                for (dose, at) in marked.iter().filter(|(_, at)| *at <= entry.exported) {
                    assert!(record.contains(dose.as_str()), "minute {minute}: the desk takes {dose} ({at}) as known, its record lacks it:\n{record}");
                }
            }
        }
        let record = w.desk.read("health-state.toml");
        for (dose, _) in &marked {
            assert!(record.contains(dose.as_str()), "{dose} never came:\n{record}");
        }
        assert!(walk(&w.server).keys().all(|r| !r.contains("conflicted")));
        eprintln!("eDrive beside Sioul: {} uploads, {} refused, {} brought down over a newer copy", edrive.uploads, edrive.refusals, edrive.downloads_over_newer);
        assert!(edrive.downloads_over_newer > 0 && edrive.refusals > 0, "the races were run");
    }

    /// The same with the desk's folder carried by the Nextcloud client and
    /// the desk sending its own files too: the client looking between a write
    /// and Sioul's send makes a conflicted copy here and puts the older copy
    /// back; the desk starts a new round with all it holds; the phone never
    /// takes a dose as known wrongly, never reads a copy, gets every dose.
    #[test]
    fn doses_are_never_known_wrongly_with_the_nextcloud_client_sending_too() {
        let w = World::new("client-sends");
        let desk_folder = w.base.join("desk-folder").join("Sioul");
        copy_dir(&w.server, &desk_folder);
        State::choose(&w.desk.memory, |s| s.given = "Documents/Sioul".into()).unwrap();
        assert!(find_with(&w.desk.memory, &desk_folder, &[w.fake.login()], &[], DUE, TEST).confirmed_for(&desk_folder));
        let mut client = Client::default();
        client.sync(&w.fake, &w.server, &desk_folder);
        let phone_down = || {
            let _turn = w.fake.turn.lock().unwrap();
            for (relative, bytes) in walk(&w.server).into_iter().filter(|(r, _)| !r.contains(&w.phone.id)) {
                put_bytes(&w.phone_folder, &relative, &bytes);
            }
        };
        let phone_up = || {
            let _turn = w.fake.turn.lock().unwrap();
            for relative in walk(&w.phone_folder).into_keys().filter(|r| r.contains(&w.phone.id)) {
                sync_up(&w.phone_folder.join(&relative), &w.server.join(&relative));
            }
        };
        let mut marked: Vec<(String, i64)> = Vec::new();
        let mut sizes = BTreeMap::new();
        let mut cuts = 0;
        for minute in 0..150 {
            let now = DUE + minute * 60;
            // Every ten minutes; and right after, where the client looks before Sioul sends.
            if (minute % 10 == 0 || [21, 71].contains(&minute)) && minute < 130 {
                marked.push((format!("dose@{minute}"), now));
                let record: String = marked.iter().map(|(d, at)| format!("\"{d}\" = {at}\n")).collect();
                w.desk.write("health-state.toml", &format!("[taken]\n{record}"));
            }
            let outcome = w.desk.exchange(&desk_folder, now);
            cuts += outcome.problems.iter().filter(|p| *p == "share-own-cut").count();
            // Now and then the client looks before Sioul's send.
            if minute % 5 == 1 {
                client.sync(&w.fake, &w.server, &desk_folder);
            }
            let sent = send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), now + 1, false, TEST);
            assert!(sent.problem.is_none(), "minute {minute}: {sent:?}");
            if minute % 3 == 0 {
                client.sync(&w.fake, &w.server, &desk_folder);
            }
            never_shorter(&mut sizes, records_there(&w.server, &w.desk.id), &format!("minute {minute}"));
            phone_down();
            let read = w.phone.exchange(&w.phone_folder, now + 30);
            assert!(read.problems.iter().all(|p| !p.starts_with("share-other-cut") && !p.starts_with("share-other-gap")), "minute {minute}: {:?}", read.problems);
            phone_up();
            if let Some(entry) = crate::devices::all(&w.phone_folder, &KEY).0.into_iter().find(|e| e.id == w.desk.id)
                && let Some(wrote) = entry.wrote
                && crate::share::heard(&w.phone.memory, &w.phone.id).complete(&w.desk.id, wrote)
            {
                let record = w.phone.read("health-state.toml");
                for (dose, at) in marked.iter().filter(|(_, at)| *at <= entry.exported) {
                    assert!(record.contains(dose.as_str()), "minute {minute}: the phone takes {dose} ({at}) as known, its record lacks it:\n{record}");
                }
            }
        }
        let record = w.phone.read("health-state.toml");
        for (dose, _) in &marked {
            assert!(record.contains(dose.as_str()), "{dose} never came:\n{record}");
        }
        assert!(walk(&w.server).keys().all(|r| !r.contains("conflicted")), "a conflicted copy never goes up");
        eprintln!("Nextcloud client beside Sioul: {} uploads, {} conflicted copies here, {} rounds started again", client.uploads, client.conflicts, cuts);
        assert!(client.conflicts > 0 && cuts > 0, "the races were run");
    }

    // ------------------------------------------------ a slow uplink, a text to send, the cost

    /// The desk beside its own sync app, sending its files: its folder apart
    /// from the server's, found there under the same seal.
    fn desk_beside(w: &World) -> PathBuf {
        let desk_folder = w.base.join("desk-folder").join("Sioul");
        copy_dir(&w.server, &desk_folder);
        State::choose(&w.desk.memory, |s| s.given = "Documents/Sioul".into()).unwrap();
        assert!(find_with(&w.desk.memory, &desk_folder, &[w.fake.login()], &[], DUE, TEST).confirmed_for(&desk_folder));
        desk_folder
    }

    /// A desk holding much (its records' opening far past a megabyte).
    fn a_desk_holding_much(w: &World, folder: &Path, at: i64) {
        let many: String = (0..8000).map(|n| format!("sender{n}@example.org\n")).collect();
        put(&w.desk.roots.config, "known-senders.txt", &many);
        w.desk.exchange(folder, at);
    }

    fn desk_records(folder: &Path, id: &str) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = walk(folder).into_iter().filter(|(r, _)| r.starts_with(id) && r.ends_with(".jsonl")).map(|(r, b)| (r, b.len() as u64)).collect();
        out.sort_by_key(|(r, _)| r.trim_end_matches(".jsonl").rsplit('-').next().unwrap().parse::<u32>().unwrap());
        out
    }

    /// The owner's desk, 8 October 2026: a records file of 2.5 MB on a
    /// phone's hotspot (3G) never went, cut at a flat minute and tried again
    /// from zero every minute. Now a file is given the time its size asks
    /// (`FLOOR`), still going is not cut; a file too slow even so is said in
    /// detail and held back, never tried before its backoff.
    #[test]
    fn a_large_file_goes_through_a_slow_uplink() {
        let w = World::new("slow-uplink");
        let desk_folder = desk_beside(&w);
        a_desk_holding_much(&w, &desk_folder, DUE);
        // The first large one in order: records go in order, the next ones wait behind it.
        let (round, size) = desk_records(&desk_folder, &w.desk.id).into_iter().find(|(_, s)| *s > 1 << 20).unwrap();
        assert!(size > 1 << 20, "a large records file: {size}");
        // An uplink of 512 KiB a second; a flat second for a file, as a flat minute was on 3G.
        *w.fake.throttle.lock().unwrap() = Some(512 << 10);
        let flat = Limits { large: Duration::from_secs(1), floor: 1 << 30, ..TEST };
        let tried = send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), DUE + 10, false, flat);
        let failure = tried.failure.clone().expect("said in detail");
        assert!(failure.kind == "timeout" && failure.file == "records" && failure.sent > 0 && failure.total == size && failure.seconds >= 1, "{failure:?}");
        assert!(tried.held == 1, "{tried:?}");
        let sending = Sending::load(&w.desk.memory);
        assert_eq!(sending.failure.as_ref().map(|f| f.kind.as_str()), Some("timeout"));
        assert!(sending.held.get(&round).is_some_and(|h| h.tries == 1 && h.until == DUE + 10 + backoff(1)), "{:?}", sending.held);
        assert!(sending.rate > 0, "its rate measured from what went");
        // Before its backoff: not tried again, not a byte spent.
        let spent = w.fake.received.load(std::sync::atomic::Ordering::Relaxed);
        let early = send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), DUE + 40, false, flat);
        assert!(early.held == 0 && w.fake.received.load(std::sync::atomic::Ordering::Relaxed) - spent < size / 2, "{early:?}");
        assert!(std::fs::metadata(w.server.join(&round)).map_or(0, |m| m.len()) < size);
        // Given the time its size asks (a floor of 128 KiB a second here): it goes, slowly, whole.
        // A stall of seconds: a busy machine (macOS on CI) may pause the fake server longer than TEST's.
        let scaled = Limits { large: Duration::from_secs(1), floor: 128 << 10, stall: Duration::from_secs(3), ..TEST };
        let went = send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), DUE + 10 + backoff(1), false, scaled);
        assert!(went.problem.is_none() && went.held == 0, "{went:?}");
        assert_eq!(std::fs::read(w.server.join(&round)).unwrap(), std::fs::read(desk_folder.join(&round)).unwrap());
        let sending = Sending::load(&w.desk.memory);
        assert!(sending.held.is_empty() && sending.failure.is_none() && sending.last == DUE + 10 + backoff(1), "{sending:?}");
    }

    /// The server unreachable, or refusing: nothing tried again before the backoff.
    #[test]
    fn a_failed_send_waits_for_its_backoff() {
        let w = World::new("backoff");
        let desk_folder = desk_beside(&w);
        w.desk.write("health-state.toml", "[taken]\n\"levo@1800000000\" = 1800000040\n");
        w.desk.exchange(&desk_folder, DUE + 40);
        w.fake.fault(|r| (r.method == "PUT").then_some(Fault::Status(503)));
        let failed = send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), DUE + 41, false, TEST);
        assert!(failed.problem.as_deref().is_some_and(|p| p.starts_with("server:")), "{failed:?}");
        assert_eq!(Sending::load(&w.desk.memory).retry_at, DUE + 41 + backoff(1));
        w.fake.forget_seen();
        let early = send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), DUE + 41 + backoff(1) - 1, false, TEST);
        assert!(early.problem.as_deref().is_some_and(|p| p.starts_with("waiting:")) && w.fake.seen().is_empty(), "{early:?} {:?}", w.fake.seen());
        // Failing again: two minutes, then five.
        let again = send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), DUE + 41 + backoff(1), false, TEST);
        assert!(again.problem.is_some());
        assert_eq!(Sending::load(&w.desk.memory).retry_at, DUE + 41 + backoff(1) + backoff(2));
        assert_eq!((backoff(1), backoff(2), backoff(3), backoff(9)), (60, 120, 300, 3600));
        // The server back: it goes, and the count starts again.
        w.fake.clear_faults();
        let at = DUE + 41 + backoff(1) + backoff(2);
        assert!(send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), at, false, TEST).problem.is_none());
        let sending = Sending::load(&w.desk.memory);
        assert!(sending.tries == 0 && sending.retry_at == 0 && sending.failure.is_none());
    }

    /// A text written on the desk reaches the phone within a minute while a
    /// large records file is still crawling up: its small file of its own
    /// goes first and alone (`send_urgent`), the phone's pull brings it.
    #[test]
    fn a_text_to_send_reaches_the_phone_while_the_records_crawl() {
        let w = World::new("texts-lane");
        let desk_folder = desk_beside(&w);
        assert!(w.find(DUE).confirmed_for(&w.phone_folder));
        a_desk_holding_much(&w, &desk_folder, DUE);
        // An uplink of 256 KiB a second: the records take seconds to go, given the time.
        *w.fake.throttle.lock().unwrap() = Some(256 << 10);
        let slow = Limits { large: Duration::from_secs(2), floor: 64 << 10, stall: Duration::from_secs(5), pull: Duration::from_secs(120), ..TEST };
        let crawling = {
            let (memory, folder, id, login) = (w.desk.memory.clone(), desk_folder.clone(), w.desk.id.clone(), w.fake.login());
            std::thread::spawn(move || send_with(&memory, &folder, &id, &login, &BTreeMap::new(), DUE + 10, false, slow))
        };
        std::thread::sleep(Duration::from_millis(500));
        // A text written now: its lines sealed already, in the desk's own file of the part "texts".
        let source = w.base.join("desk-texts-send.jsonl");
        std::fs::write(&source, "a sealed request line\n").unwrap();
        let written = Instant::now();
        assert!(place_texts(&desk_folder, &w.desk.id, &source).unwrap());
        let urgent = send_urgent_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), DUE + 11, slow);
        assert!(urgent.problem.is_none() && urgent.sent == 1, "{urgent:?}");
        // The phone's next pull brings it; the records still on their way.
        let pulled = pull_with(&w.phone.memory, &w.phone_folder, &w.phone.id, &w.fake.login(), DUE + 20, TEST);
        assert!(pulled.problem.is_none(), "{pulled:?}");
        assert_eq!(std::fs::read(texts_file(&cache_of(&w.phone.memory), &w.desk.id)).unwrap(), b"a sealed request line\n");
        assert!(!crawling.is_finished(), "the records still going up");
        assert!(written.elapsed() < Duration::from_secs(10), "{:?}", written.elapsed());
        let crawled = crawling.join().unwrap();
        assert!(crawled.problem.is_none(), "{crawled:?}");
        // Never sent twice by the main lane; a change goes again; none here, none there.
        assert!(!own_plain(&desk_folder, &w.desk.id).iter().any(|r| r.starts_with("texts/")));
        assert!(!place_texts(&desk_folder, &w.desk.id, &source).unwrap(), "unchanged: nothing to place");
        std::fs::remove_file(&source).unwrap();
        assert!(place_texts(&desk_folder, &w.desk.id, &source).unwrap() && !texts_file(&desk_folder, &w.desk.id).exists());
    }

    /// What a computer writing each minute costs, sent beside its sync app:
    /// a change a minute for an hour, its entry and its claim renewed each
    /// minute. Its records' file stays small (`share::SEGMENT`): measured here.
    #[test]
    fn bytes_an_hour_of_a_computer_writing_each_minute() {
        let w = World::new("an-hour");
        let desk_folder = desk_beside(&w);
        a_desk_holding_much(&w, &desk_folder, DUE);
        assert!(send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), DUE + 1, false, TEST).problem.is_none());
        let start = w.fake.received.load(std::sync::atomic::Ordering::Relaxed);
        let mut minutes: String = String::new();
        for minute in 1..=60 {
            let now = DUE + minute * 60;
            minutes.push_str(&format!("\"dose@{minute}\" = {now}\n"));
            w.desk.write("health-state.toml", &format!("[taken]\n{minutes}"));
            w.desk.exchange(&desk_folder, now);
            crate::lease::renew(&desk_folder, &KEY, "health", &w.desk.id, now, now, false, crate::lease::Rule::FollowsYou, crate::share::written(&w.desk.memory, &w.desk.id)).unwrap();
            let sent = send_with(&w.desk.memory, &desk_folder, &w.desk.id, &w.fake.login(), &BTreeMap::new(), now + 1, false, TEST);
            assert!(sent.problem.is_none(), "minute {minute}: {sent:?}");
        }
        let hour = w.fake.received.load(std::sync::atomic::Ordering::Relaxed) - start;
        let largest = desk_records(&desk_folder, &w.desk.id).last().map_or(0, |(_, s)| *s);
        eprintln!("A computer writing each minute, sent beside its sync app: {hour} bytes in an hour; its current records' file {largest} bytes");
        assert!(hour < 3 << 20, "{hour} bytes an hour");
        assert!(largest < 64 << 10, "{largest}");
        // Every dose there for the others.
        w.phone.exchange(&w.server, DUE + 3_700);
        assert!(w.phone.read("health-state.toml").contains("dose@60"));
    }

    // ------------------------------------------------ sealed files waited for

    impl Device {
        /// An exchange of its notes too (`files`), kept in `notes`.
        fn exchange_notes(&self, folder: &Path, notes: &Path, now: i64) -> Outcome {
            let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
            let stores = crate::share::stores_of(&config, &self.roots, &|part| part == "notes" || crate::share::shared_by_default(part, &config));
            crate::share::exchange(&Sharing { folder, computer: &self.id, key: &KEY, memory: &self.memory, files: true, hurry: None }, &stores, now * 1000).unwrap()
        }
    }

    /// The desk and the phone sharing their notes, the backup found for the
    /// phone; the phone's sync app brings nothing down from now on (eDrive's
    /// half-hour, 10 October 2026). Their notes' folders.
    fn notes_world(name: &str) -> (World, PathBuf, PathBuf) {
        let w = World::new(name);
        let (desk_notes, phone_notes) = (w.base.join("desk-notes"), w.base.join("phone-notes"));
        for dir in [&desk_notes, &phone_notes] {
            std::fs::create_dir_all(dir).unwrap();
        }
        w.desk.exchange_notes(&w.server, &desk_notes, DUE + 10);
        w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 20);
        w.up();
        assert!(w.find(DUE + 30).confirmed_for(&w.phone_folder));
        (w, desk_notes, phone_notes)
    }

    /// A note written on the desk, its client carrying it to the server at once: its sealed file's name there.
    fn desk_writes(w: &World, desk_notes: &Path, file: &str, text: &str, now: i64) -> String {
        put(desk_notes, file, text);
        w.desk.exchange_notes(&w.server, desk_notes, now);
        let name = crate::blobs::name(&KEY, &crate::blobs::hash_file(&desk_notes.join(file)).unwrap().0);
        assert!(w.server.join("blobs").join(&name).exists());
        name
    }

    fn fetch(w: &World, now: i64, frugal: bool) -> Pulled {
        fetch_wanted_with(&w.phone.memory, &w.phone_folder, &w.fake.login(), &KEY, now, frugal, TEST)
    }

    fn blob_gets(w: &World) -> usize {
        w.fake.seen().iter().filter(|l| l.starts_with("GET ") && l.contains("/blobs/")).count()
    }

    /// A note written on the desk reaches a phone whose sync app brings no
    /// sealed file within one step: the pull brings the record, the exchange
    /// says the sealed file it waits for, the fetch brings that file alone,
    /// by its name, beside the synced folder, and the next exchange writes
    /// the note. Nothing listed in the server's `blobs/`, nothing sent; the
    /// copy fetched goes once the note is written.
    #[test]
    fn a_note_waited_for_is_fetched_by_its_name_and_written_within_one_step() {
        let (w, desk_notes, phone_notes) = notes_world("wanted");
        let name = desk_writes(&w, &desk_notes, "lease.md", "The lease, signed.\n", DUE + 60);
        w.fake.forget_seen();
        // The step: the pull, then the exchange.
        let pulled = w.pull(DUE + 70);
        assert!(pulled.problem.is_none(), "{pulled:?}");
        let first = w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 71);
        assert_eq!(first.waits.get("no-blob"), Some(&1), "{first:?}");
        assert_eq!((first.received, first.pending), (0, 1));
        assert!(!phone_notes.join("lease.md").exists());
        let wanted = crate::blobs::Wanted::load(&w.phone.memory);
        assert_eq!(wanted.blobs.keys().cloned().collect::<Vec<_>>(), [name.clone()]);
        assert!(wanted.blobs[&name].size == 19 && wanted.blobs[&name].since == DUE + 71, "{wanted:?}");
        assert!(wanted_due(&w.phone.memory, &w.phone_folder, DUE + 71));
        // Its sealed file fetched, then the exchange again.
        let fetched = fetch(&w, DUE + 72, false);
        assert_eq!((fetched.fetched, fetched.problem.as_deref()), (1, None), "{fetched:?}");
        assert!(w.cache().join("blobs").join(&name).exists());
        assert!(!w.phone_folder.join("blobs").join(&name).exists(), "never written into the synced folder");
        assert!(!wanted_due(&w.phone.memory, &w.phone_folder, DUE + 72), "fetched: no longer due");
        let second = w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 73);
        assert_eq!((second.received, second.pending), (1, 0), "{second:?}");
        assert!(second.waits.is_empty() && second.problems.is_empty(), "{second:?}");
        assert_eq!(std::fs::read_to_string(phone_notes.join("lease.md")).unwrap(), "The lease, signed.\n");
        assert!(!crate::blobs::Wanted::path(&w.phone.memory).exists(), "nothing waits");
        // By its name: one GET, no listing of `blobs/`, nothing sent.
        assert_eq!(blob_gets(&w), 1, "{:?}", w.fake.seen());
        for line in w.fake.seen() {
            assert!(!(line.starts_with("PROPFIND") && line.contains("/blobs")), "{line}");
            assert!(line.starts_with("PROPFIND ") || line.starts_with("GET "), "{line}");
        }
        // Nothing waits: no request.
        w.fake.forget_seen();
        assert_eq!(fetch(&w, DUE + 74, false), Pulled::default());
        assert!(w.fake.seen().is_empty());
        // Written: the copy fetched goes at the next tidying.
        tidy(&w.phone.memory, &w.phone_folder, &w.phone.id);
        assert!(!w.cache().join("blobs").join(&name).exists());
    }

    /// A copy fetched that does not open whole is never read: one damaged on
    /// this phone is taken out and fetched again; one damaged on the server
    /// is never kept, and asked again after a while; whole again there, it
    /// comes, and the note is written.
    #[test]
    fn a_damaged_copy_fetched_is_refused_and_fetched_again() {
        let (w, desk_notes, phone_notes) = notes_world("wanted-damaged");
        let name = desk_writes(&w, &desk_notes, "lease.md", "The lease, signed.\n", DUE + 60);
        assert!(w.pull(DUE + 70).problem.is_none());
        assert_eq!(w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 71).waits.get("no-blob"), Some(&1));
        // Damaged here, after it came: refused, taken out, waited for again.
        let whole = std::fs::read(w.server.join("blobs").join(&name)).unwrap();
        let mut damaged = whole.clone();
        let middle = damaged.len() / 2;
        damaged[middle] ^= 0x10;
        put_bytes(&w.cache(), &format!("blobs/{name}"), &damaged);
        let outcome = w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 72);
        assert_eq!(outcome.waits.get("no-blob"), Some(&1), "{outcome:?}");
        assert!(outcome.problems.is_empty(), "the copy fetched is not the folder's: nothing said damaged, {outcome:?}");
        assert!(!w.cache().join("blobs").join(&name).exists());
        assert!(!phone_notes.join("lease.md").exists());
        // Damaged on the server: fetched, refused, never kept; not asked again at once.
        std::fs::write(w.server.join("blobs").join(&name), &damaged).unwrap();
        let fetched = fetch(&w, DUE + 73, false);
        assert_eq!((fetched.fetched, fetched.held), (0, 1), "{fetched:?}");
        assert!(!w.cache().join("blobs").join(&name).exists());
        assert_eq!(crate::blobs::Wanted::load(&w.phone.memory).blobs[&name].again, DUE + 73 + 60);
        w.fake.forget_seen();
        assert_eq!(fetch(&w, DUE + 100, false).fetched, 0);
        assert_eq!(blob_gets(&w), 0, "within its wait: not asked");
        // Whole there again: it comes, the note is written.
        std::fs::write(w.server.join("blobs").join(&name), &whole).unwrap();
        assert_eq!(fetch(&w, DUE + 140, false).fetched, 1);
        let outcome = w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 141);
        assert_eq!(outcome.received, 1, "{outcome:?}");
        assert_eq!(std::fs::read_to_string(phone_notes.join("lease.md")).unwrap(), "The lease, signed.\n");
    }

    /// A sealed file the sync app is late with is not on the server yet
    /// either: asked again within minutes, never backed off for long; there,
    /// it comes at the next ask.
    #[test]
    fn a_sealed_file_not_on_the_server_yet_is_asked_again_within_minutes() {
        let (w, desk_notes, phone_notes) = notes_world("wanted-late");
        let name = desk_writes(&w, &desk_notes, "lease.md", "The lease, signed.\n", DUE + 60);
        let aside = w.base.join("aside-blob");
        std::fs::rename(w.server.join("blobs").join(&name), &aside).unwrap();
        assert!(w.pull(DUE + 70).problem.is_none());
        assert_eq!(w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 71).waits.get("no-blob"), Some(&1));
        let mut at = DUE + 72;
        for _ in 0..8 {
            let fetched = fetch(&w, at, false);
            assert!(fetched.fetched == 0 && fetched.problem.is_none(), "{fetched:?}");
            let again = crate::blobs::Wanted::load(&w.phone.memory).blobs[&name].again;
            assert!(again > at && again <= at + WANTED_AGAIN, "{again} after {at}");
            at = again;
        }
        std::fs::rename(&aside, w.server.join("blobs").join(&name)).unwrap();
        assert_eq!(fetch(&w, at, false).fetched, 1);
        assert_eq!(w.phone.exchange_notes(&w.phone_folder, &phone_notes, at + 1).received, 1);
    }

    /// A copy fetched, then the same brought by the sync app into the synced
    /// folder: the folder's is read, identical, and the copy fetched goes,
    /// even while the note still waits.
    #[test]
    fn a_sealed_file_fetched_then_brought_by_the_sync_app_is_kept_once() {
        let (w, desk_notes, phone_notes) = notes_world("wanted-brought");
        let name = desk_writes(&w, &desk_notes, "lease.md", "The lease, signed.\n", DUE + 60);
        assert!(w.pull(DUE + 70).problem.is_none());
        assert_eq!(w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 71).waits.get("no-blob"), Some(&1));
        assert_eq!(fetch(&w, DUE + 72, false).fetched, 1);
        // The sync app catches up before the next exchange.
        w.down();
        let (fetched, brought) = (std::fs::read(w.cache().join("blobs").join(&name)).unwrap(), std::fs::read(w.phone_folder.join("blobs").join(&name)).unwrap());
        assert_eq!(fetched, brought, "the same file");
        tidy(&w.phone.memory, &w.phone_folder, &w.phone.id);
        assert!(!w.cache().join("blobs").join(&name).exists(), "the synced folder holds it: the copy fetched goes");
        assert!(crate::blobs::Wanted::path(&w.phone.memory).exists(), "the note still waits");
        let outcome = w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 73);
        assert_eq!(outcome.received, 1, "{outcome:?}");
        assert!(!crate::blobs::Wanted::path(&w.phone.memory).exists());
    }

    /// What was fetched beside the folder never piles up: a copy no note
    /// waits for goes, one kept past a few days goes and is no longer waited
    /// for (an exchange that reads notes says it again if it still waits),
    /// all of them with the backup switched off.
    #[test]
    fn the_sealed_files_fetched_are_pruned() {
        let (w, desk_notes, phone_notes) = notes_world("wanted-pruned");
        let name = desk_writes(&w, &desk_notes, "lease.md", "The lease, signed.\n", DUE + 60);
        assert!(w.pull(DUE + 70).problem.is_none());
        assert_eq!(w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 71).waits.get("no-blob"), Some(&1));
        assert_eq!(fetch(&w, DUE + 72, false).fetched, 1);
        let blobs = w.cache().join("blobs");
        put(&blobs, "0123abcd", "a copy no note waits for");
        tidy(&w.phone.memory, &w.phone_folder, &w.phone.id);
        assert!(blobs.join(&name).exists(), "waited for: kept");
        assert!(!blobs.join("0123abcd").exists());
        set_mtime(&blobs.join(&name), SystemTime::now() - Duration::from_secs(4 * 86_400));
        tidy(&w.phone.memory, &w.phone_folder, &w.phone.id);
        assert!(!blobs.join(&name).exists(), "past a few days");
        assert!(!crate::blobs::Wanted::path(&w.phone.memory).exists(), "no longer waited for");
        w.fake.forget_seen();
        assert_eq!(fetch(&w, DUE + 80, false).fetched, 0);
        assert_eq!(blob_gets(&w), 0);
        // Still waiting, said again by the next exchange that reads notes: fetched again.
        assert_eq!(w.phone.exchange_notes(&w.phone_folder, &phone_notes, DUE + 81).waits.get("no-blob"), Some(&1));
        assert_eq!(fetch(&w, DUE + 82, false).fetched, 1);
        // The backup switched off: what was fetched goes.
        State::choose(&w.phone.memory, |s| s.on = Some(false)).unwrap();
        tidy(&w.phone.memory, &w.phone_folder, &w.phone.id);
        assert!(!blobs.join(&name).exists());
        assert_eq!(fetch(&w, DUE + 83, false).problem.as_deref(), Some("not-confirmed"));
    }

    /// Waited for: a step brings at most `WANTED_FILES` and `WANTED_BYTES`,
    /// the smallest first; on a metered or slow connection a note's sealed
    /// file comes at once, a large paper's at most every ten minutes.
    #[test]
    fn sealed_files_waited_for_come_bounded_and_paced() {
        let (w, _, _) = notes_world("wanted-paced");
        let made = |seed: u32, size: usize| {
            let mut x = seed.wrapping_mul(2_654_435_761) | 1;
            (0..size)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    x as u8
                })
                .collect::<Vec<u8>>()
        };
        // Sealed on the server by hand, said waited for as an exchange says it.
        let sealed = |seed: u32, size: usize| {
            let source = w.base.join(format!("source-{seed}"));
            std::fs::write(&source, made(seed, size)).unwrap();
            let (hash, size) = crate::blobs::hash_file(&source).unwrap();
            crate::blobs::put(&w.server, &KEY, &source, &hash).unwrap();
            let name = crate::blobs::name(&KEY, &hash);
            crate::blobs::Wanted::change(&w.phone.memory, |wanted| {
                wanted.blobs.insert(name.clone(), crate::blobs::Want { hash, size, since: DUE, ..Default::default() });
            })
            .unwrap();
            name
        };
        let papers = [sealed(1, 1_500_000), sealed(2, 1_200_000)];
        let note = sealed(3, 3_000);
        // Metered: the note and one paper; the other paper waits ten minutes.
        let first = fetch(&w, DUE + 100, true);
        assert_eq!((first.fetched, first.paced), (2, 1), "{first:?}");
        let blobs = w.cache().join("blobs");
        assert!(blobs.join(&note).exists() && blobs.join(&papers[1]).exists() && !blobs.join(&papers[0]).exists());
        w.fake.forget_seen();
        assert_eq!((fetch(&w, DUE + 400, true).fetched, blob_gets(&w)), (0, 0), "paced: not asked");
        assert_eq!(fetch(&w, DUE + 100 + PACE, true).fetched, 1);
        assert!(blobs.join(&papers[0]).exists());
        // Many notes at once: `WANTED_FILES` a step, the rest at the next.
        let notes: Vec<String> = (10..10 + WANTED_FILES as u32 + 2).map(|seed| sealed(seed, 500)).collect();
        assert_eq!(fetch(&w, DUE + 2_000, true).fetched, WANTED_FILES);
        assert_eq!(fetch(&w, DUE + 2_001, true).fetched, 2);
        assert!(notes.iter().all(|n| blobs.join(n).exists()));
        // `WANTED_BYTES` a step, the first whatever its size: on an unmetered connection, all at once otherwise.
        let large: Vec<String> = (40..43).map(|seed| sealed(seed, 7 << 20)).collect();
        assert_eq!(fetch(&w, DUE + 3_000, false).fetched, 2, "14 MiB, then the third past 16");
        assert_eq!(fetch(&w, DUE + 3_001, false).fetched, 1);
        assert!(large.iter().all(|n| blobs.join(n).exists()));
    }
}

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
//! - **Never**: anything written, moved or deleted on the server; anything
//!   written into the synced folder; this device's own files fetched or read
//!   from there; plain HTTP (a test's own stand-in apart); the password kept
//!   or said (it comes from the keyring, as the calendars' sync has it, for
//!   the requests only).

use crate::SyncError;
use crate::dav::{self, Budget, DAV};
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
}

pub(crate) const LIMITS: Limits = Limits { wait: Duration::from_secs(10), small: Duration::from_secs(10), large: Duration::from_secs(60), pull: Duration::from_secs(30) };

/// Looked for again this long after the last look, when not found (seconds).
pub const LOOK_AGAIN: i64 = 6 * 3600;
/// The server's seal read again this long after the last time, even unchanged (seconds).
const SEAL_AGAIN: i64 = 6 * 3600;
/// A folder whose ETag did not change is looked through again after this long all the same (seconds).
const LIST_AGAIN: i64 = 10 * 60;
/// A records' file is never fetched past this.
const LARGEST: u64 = 64 << 20;
/// A listing is never read past this.
const LISTING: u64 = 4 << 20;
/// The last line asked again, to check the copy here joins what is there, when shorter than this.
const OVERLAP: u64 = 1 << 20;

const XML: &str = "application/xml; charset=utf-8";
const LIST: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/><d:getetag/><d:getcontentlength/></d:prop></d:propfind>"#;
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
    /// The server's files as last fetched, or found here already, by their path in the folder.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, Fetched>,
    /// The server's folders as last looked through, by their path ("" the folder itself, "devices/").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub folders: BTreeMap<String, Looked>,
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
            State { on: there.on, given: there.given, asked: there.asked, ..self.clone() }.write(memory)
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
    state.folder == shown(folder) && seal_of(folder).is_some_and(|here| seal_of(&cache_of(memory)).is_some_and(|fetched| fetched == here))
}

/// Sharing stopped here: what was fetched and known goes, and nothing is read beside the folder.
pub fn forget(memory: &Path, folder: &Path) {
    let _ = std::fs::remove_dir_all(cache_of(memory));
    let _ = std::fs::remove_file(state_path(memory));
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
/// HTTP only to a test's own stand-in, `dav::allowed`), never redirected.
pub(crate) struct Server {
    small: ureq::Agent,
    large: ureq::Agent,
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
}

/// What came of a file asked from a place in it: where it starts, its bytes,
/// whether all came, and why not.
pub(crate) struct Got {
    start: u64,
    bytes: Vec<u8>,
    error: Option<SyncError>,
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
        let budget = |whole| Budget { connect: limits.wait, stall: limits.wait, whole };
        Ok(Server {
            small: dav::agent(&budget(limits.small)),
            large: dav::agent(&budget(limits.large)),
            authorization: format!("Basic {}", sioul_core::lines::base64_encode(format!("{}:{password}", login.user).as_bytes())),
            started: Instant::now(),
            limits,
        })
    }

    pub(crate) fn request(&self, large: bool, method: &str, url: &str, headers: &[(&str, &str)], body: Vec<u8>) -> Result<ureq::http::Response<ureq::Body>, SyncError> {
        dav::allowed(url)?;
        if self.started.elapsed() > self.limits.pull {
            return Err(SyncError::Network("out of time".into()));
        }
        let mut request = ureq::http::Request::builder().method(method).uri(url).header("Authorization", &self.authorization).header("User-Agent", "Sioul");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let request = request.body(body).map_err(|e| SyncError::Server(e.to_string()))?;
        let agent = if large { &self.large } else { &self.small };
        let response = agent.run(request).map_err(|e| failed(&e))?;
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
            listing.items.insert(name.to_string(), Item { dir: response.is(DAV, "collection"), etag, size });
        }
        Ok(listing)
    }

    /// A small file whole (`share::SMALL_FILE` at most) and its ETag; none
    /// when it is not there, or larger than any such file. A body that came
    /// cut, or whose end cannot be told, is an error: never taken as whole.
    pub(crate) fn small(&self, url: &str) -> Result<Option<(Vec<u8>, Option<String>)>, SyncError> {
        let mut response = self.request(false, "GET", url, &[("Accept-Encoding", "identity")], Vec::new())?;
        match response.status().as_u16() {
            200 => {}
            404 | 410 => return Ok(None),
            status => return Err(SyncError::Server(format!("GET: {status}"))),
        }
        let etag = header(&response, "ETag");
        let length = response.body().content_length();
        let chunked = header(&response, "Transfer-Encoding").is_some_and(|t| t.to_ascii_lowercase().contains("chunked"));
        if length.is_some_and(|l| l > crate::share::SMALL_FILE) {
            return Ok(None);
        }
        let bytes = match response.body_mut().with_config().limit(crate::share::SMALL_FILE).read_to_vec() {
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
}

/// A path in a folder of the server as an address's: each name percent-encoded ("Mes documents/Sioul" → "Mes%20documents/Sioul").
pub(crate) fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|name| name.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>())
        .collect::<Vec<_>>()
        .join("/")
}

/// Why, in a code the window says in words: "login:murena.io".
fn code(error: &SyncError, host: &str) -> String {
    let kind = match error {
        SyncError::Login(_) | SyncError::AppPassword(_) => "login",
        SyncError::Tls(_) => "tls",
        SyncError::Network(_) => "network",
        SyncError::NoPassword => "no-password",
        SyncError::Disk(_) => "disk",
        _ => "server",
    };
    format!("{kind}:{host}")
}

/// Where an account's files are on its server, Nextcloud's way
/// (`…/remote.php/dav/files/<user>/`, the user as the server names it in its
/// principal), from the address its calendars were found at; none for a
/// server of another kind.
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
    Ok(principal.and_then(|href| {
        let at = href.find("/principals/users/")?;
        let user = href[at + "/principals/users/".len()..].trim_end_matches('/');
        (!user.is_empty() && !user.contains('/')).then(|| dav::absolute(&base, &format!("{}/files/{user}/", &href[..at])))
    }))
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
    if state.folder != shown(folder) {
        // Another folder: what was fetched for the last one goes.
        let _ = std::fs::remove_dir_all(cache_of(memory));
        state = State { on: state.on, given: state.given.clone(), asked: state.asked, folder: shown(folder), ..State::default() };
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
    if state.tried < state.confirmed {
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

/// What a pull did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pulled {
    /// Files and folders the server's listings showed.
    pub listed: usize,
    /// Files that came down and are kept.
    pub fetched: usize,
    /// Bytes that came down.
    pub bytes: u64,
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
    if !state.confirmed_for(folder) || state.account != login.account {
        pulled.problem = Some("not-confirmed".into());
        return pulled;
    }
    let cache = cache_of(memory);
    // Switched off here: not a request; what the folder caught up with goes.
    if !state.fetching() {
        tidy_cache(&state, folder, &cache, own);
        pulled.problem = Some("off".into());
        return pulled;
    }
    // This folder's seal changed here (sharing started again): what was
    // fetched belongs to another sharing, and the server's folder is looked at again.
    if !usable(folder, memory) {
        let _ = std::fs::remove_dir_all(&cache);
        state = State { on: state.on, given: state.given.clone(), asked: state.asked, folder: state.folder.clone(), said: "no-seal".into(), ..State::default() };
        let _ = state.save(memory);
        pulled.problem = Some(state.said);
        return pulled;
    }
    state.tried = now;
    let host = state.host();
    let outcome = Server::new(login, limits).map_err(Stop::Failed).and_then(|server| Puller { server: &server, folder, into: &cache, own, state: &mut state, pulled: &mut pulled, now }.run());
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
    tidy_cache(&state, folder, &cache, own);
    let _ = state.save(memory);
    pulled.problem = (!state.said.is_empty()).then(|| state.said.clone());
    pulled
}

/// What is no longer needed in the cache, gone, without the network: this
/// device's own files (never fetched, never read from there), records the
/// synced folder caught up with, entries and notes the same as the folder's
/// and, the backup switched off, all of them; records longer than the
/// folder's stay until it catches up (read past its end here, a records'
/// file gone would read as cut, and the doses would doubt for hours).
pub fn tidy(memory: &Path, folder: &Path, own: &str) {
    let state = State::load(memory);
    if state.folder == shown(folder) {
        tidy_cache(&state, folder, &cache_of(memory), own);
    }
}

fn tidy_cache(state: &State, folder: &Path, cache: &Path, own: &str) {
    // What a crash left half written, an hour later.
    for dir in [cache.to_path_buf(), cache.join("devices")].into_iter().chain(std::fs::read_dir(cache.join("leases")).into_iter().flatten().filter_map(Result::ok).map(|e| e.path())) {
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
    out
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
}

impl Puller<'_> {
    fn url(&self, relative: &str) -> String {
        format!("{}{}", self.state.url, encode_path(relative))
    }

    /// A folder of the server not changed since it was last looked through, lately.
    fn unchanged(&self, relative: &str, etag: &str) -> bool {
        !etag.is_empty() && self.state.folders.get(relative).is_some_and(|l| l.etag == etag && self.now - l.at < LIST_AGAIN)
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
        if self.unchanged("", &root.etag) {
            return Ok(());
        }
        for (name, item) in root.items.iter().filter(|(_, item)| !item.dir) {
            match kind_of(name) {
                Some((Kind::Round, id)) if id != self.own => self.round(name, item)?,
                Some((Kind::Seen, id)) if id != self.own => self.small(name, item)?,
                _ => {}
            }
        }
        if let Some(devices) = root.items.get("devices").filter(|item| item.dir)
            && !self.unchanged("devices/", &devices.etag)
        {
            let listing = self.server.list(&self.url("devices/"))?;
            self.pulled.listed += listing.items.len();
            for (name, item) in listing.items.iter().filter(|(_, item)| !item.dir) {
                let relative = format!("devices/{name}");
                if kind_of(&relative).is_some_and(|(_, id)| id != self.own) {
                    self.small(&relative, item)?;
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
                for (name, item) in listing.items.iter().filter(|(_, item)| !item.dir) {
                    let relative = format!("{dir}{name}");
                    if kind_of(&relative).is_some_and(|(_, id)| id != self.own) {
                        self.small(&relative, item)?;
                    }
                }
                self.forget_gone(&dir, &listing);
                self.looked(&dir, &listing.etag);
            }
            self.looked("leases/", &parts.etag);
        }
        self.forget_gone("", &root);
        self.looked("", &root.etag);
        Ok(())
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
            if path != "seal.toml" && self.into != self.folder {
                let _ = std::fs::remove_file(self.into.join(&path));
            }
        }
    }

    /// A small file (notes, an entry, a claim) fetched whole when it changed
    /// there since it was last fetched; kept only when it differs from the
    /// synced folder's copy, and only when all of it came.
    fn small(&mut self, relative: &str, item: &Item) -> Result<(), Stop> {
        let known = Fetched { etag: item.etag.clone(), size: item.size };
        if !item.etag.is_empty() && self.state.files.get(relative) == Some(&known) {
            return Ok(());
        }
        if item.size > crate::share::SMALL_FILE {
            return Ok(());
        }
        let Some((bytes, _)) = self.server.small(&self.url(relative))? else {
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
        if item.size > LARGEST {
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
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
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
        faults: Mutex<Faults>,
        seen: Mutex<Vec<String>>,
        /// Serving one request at a time: a listing never sees a file half written by a test's own copy.
        pub(crate) turn: Mutex<()>,
    }

    impl Fake {
        pub(crate) fn start(files: &Path) -> Arc<Fake> {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
            let fake = Arc::new(Fake { files: files.to_path_buf(), base, faults: Mutex::new(Vec::new()), seen: Mutex::new(Vec::new()), turn: Mutex::new(()) });
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
            let Some(request) = read(&stream) else { return };
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
                "PROPFIND" => propfind(request, &relative, &path),
                "GET" => get(request, &path),
                _ => (405, Vec::new(), Vec::new()),
            }
        }
    }

    fn read(stream: &TcpStream) -> Option<Request> {
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
        reader.read_exact(&mut body).ok()?;
        Some(Request { method, path: crate::dav::path_key(&raw), headers, body })
    }

    pub(crate) fn multistatus(responses: &str) -> Vec<u8> {
        format!(r#"<?xml version="1.0" encoding="utf-8"?><d:multistatus xmlns:d="DAV:">{responses}</d:multistatus>"#).into_bytes()
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
        }
        format!("{:016x}", hasher.finish())
    }

    fn entry(href: &str, dir: bool, etag: &str, size: u64) -> String {
        let kind = if dir { "<d:resourcetype><d:collection/></d:resourcetype>" } else { "<d:resourcetype/>" };
        let length = if dir { String::new() } else { format!("<d:getcontentlength>{size}</d:getcontentlength>") };
        format!("<d:response><d:href>{href}</d:href><d:propstat><d:prop>{kind}<d:getetag>&quot;{etag}&quot;</d:getetag>{length}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>")
    }

    pub(crate) fn href(relative: &str, dir: bool) -> String {
        let path = super::encode_path(relative);
        match (relative.is_empty(), dir) {
            (true, _) => format!("{FILES}/"),
            (false, true) => format!("{FILES}/{path}/"),
            (false, false) => format!("{FILES}/{path}"),
        }
    }

    fn propfind(request: &Request, relative: &str, path: &Path) -> Reply {
        let Ok(meta) = std::fs::metadata(path) else { return (404, Vec::new(), Vec::new()) };
        let mut out = entry(&href(relative, meta.is_dir()), meta.is_dir(), &etag_of(path), meta.len());
        if meta.is_dir() && request.header("Depth") != Some("0") {
            let mut children: Vec<_> = std::fs::read_dir(path).into_iter().flatten().filter_map(Result::ok).collect();
            children.sort_by_key(|e| e.file_name());
            for child in children {
                let name = child.file_name().to_string_lossy().to_string();
                let inside = if relative.is_empty() { name } else { format!("{relative}/{name}") };
                let dir = child.file_type().is_ok_and(|t| t.is_dir());
                out += &entry(&href(&inside, dir), dir, &etag_of(&child.path()), child.metadata().map_or(0, |m| m.len()));
            }
        }
        (207, vec![("Content-Type", "application/xml; charset=utf-8".into())], multistatus(&out))
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
    const TEST: Limits = Limits { wait: Duration::from_millis(700), small: Duration::from_secs(3), large: Duration::from_secs(5), pull: Duration::from_secs(30) };
    const DUE: i64 = 1_800_000_000;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-remote-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
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
}

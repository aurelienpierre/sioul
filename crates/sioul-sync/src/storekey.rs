// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sealed store's root key (docs/store.md, "The key"): 256 random bits
//! per store, kept in the system's keyring as the sharing's key is
//! (`secret`), under `Sioul store <id>`, the store's id read from
//! `store.toml`. Here, not in the core: the core takes a key and never
//! fetches one, so that it gains no keyring, no D-Bus, no Argon2.
//!
//! - **At start** the key comes before the configuration: `root_key`.
//! - **A process started before the keyring opens** (the watcher at login)
//!   waits for it and never prompts: `wait_for_root_key`, which never asks
//!   the keyring to unlock (`secret::named_bytes_quietly`) and gives back
//!   `Locked` at its deadline, so that the caller waits again inside one
//!   start rather than fail (systemd gives up after five).
//! - **No keyring service** (`NoService`): the root key is kept wrapped in
//!   `store.toml` under a passphrase (Argon2id, 64 MiB and 3 passes, as the
//!   sharing's `seal.toml`; XChaCha20-Poly1305), asked at each start, the key
//!   held in memory only.
//! - **Tests and the demo**: with the `test-store-key` feature alone (never
//!   in Sioul's builds), `SIOUL_TEST_STORE_KEY` in the environment gives the
//!   fixed test key, and nothing reaches a keyring. The feature leaves the
//!   mark `SIOUL_TEST_STORE_KEY` in the binary, which the shipping check
//!   refuses (tools/check-test-marks.sh).

use crate::secret::{self, Quiet};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sioul_core::sealed::{Folders, Options, RootKey, Store, StoreFile, Way, Wrapped};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

/// Why the store's key is not had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// No store on this profile yet.
    NoStore,
    /// The keyring is there and not open yet: wait (never prompt).
    Locked,
    /// No keyring service on this system: the passphrase stands in.
    NoService,
    /// The keyring holds no key for this store: a way back is needed.
    Missing,
    /// Not the passphrase the key was wrapped with.
    WrongPassphrase,
    /// A recovery code that opens nothing kept here.
    WrongCode,
    /// No security key there, or not one a way back was sealed to, or its PIN refused.
    NoCard(String),
    Other(String),
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyError::NoStore => f.write_str("Sioul keeps nothing sealed on this device yet."),
            KeyError::Locked => f.write_str("Sioul waits for your keyring to open."),
            KeyError::NoService => f.write_str("This system has no keyring: Sioul's passphrase opens what it keeps."),
            KeyError::Missing => f.write_str("Your keyring holds no key for what Sioul keeps here: a way back is needed."),
            KeyError::WrongPassphrase => f.write_str("This is not the passphrase Sioul's key was kept with."),
            KeyError::WrongCode => f.write_str("This code opens nothing Sioul keeps here."),
            KeyError::NoCard(why) => write!(f, "No security key opened Sioul's key: {why}"),
            KeyError::Other(why) => f.write_str(why),
        }
    }
}

/// The keyring entry of a store's key.
pub fn entry_name(id: &str) -> String {
    format!("Sioul store {id}")
}

/// The test key, when this build may give it and the environment asks for it.
fn test_key() -> Option<RootKey> {
    #[cfg(feature = "test-store-key")]
    if std::env::var_os("SIOUL_TEST_STORE_KEY").is_some() {
        return Some(RootKey::test());
    }
    None
}

/// The store's key from the keyring, never prompting.
pub fn root_key(folders: &Folders) -> Result<RootKey, KeyError> {
    if let Some(key) = test_key() {
        return Ok(key);
    }
    let ids = Store::ids(folders).map_err(|e| KeyError::Other(e.to_string()))?;
    if ids.is_empty() {
        return Err(KeyError::NoStore);
    }
    // Each copy's id asked in turn: a damaged first copy never hides the second's entry.
    let mut answer = KeyError::Missing;
    for id in &ids {
        match secret::named_bytes_quietly(&entry_name(id)) {
            Quiet::Found(bytes) => {
                let bytes = Zeroizing::new(bytes);
                let key: [u8; 32] = bytes.as_slice().try_into().map_err(|_| KeyError::Other("the keyring's entry is not a key".to_string()))?;
                return Ok(RootKey::from_bytes(key));
            }
            Quiet::None => {}
            Quiet::Locked => answer = KeyError::Locked,
            Quiet::NoService => answer = KeyError::NoService,
            Quiet::Failed(why) if answer == KeyError::Missing => answer = KeyError::Other(why),
            Quiet::Failed(_) => {}
        }
    }
    // A key kept under a passphrase: for any answer but a key found, the
    // passphrase is the way (a process with no session bus at all included).
    if has_wrapped(folders) {
        return Err(KeyError::NoService);
    }
    // No keyring reached and no passphrase: the keyring that kept it is not reached yet, and Sioul waits for it.
    Err(if answer == KeyError::NoService { KeyError::Locked } else { answer })
}

/// Whether `store.toml` keeps the key wrapped under a passphrase.
fn has_wrapped(folders: &Folders) -> bool {
    StoreFile::load(folders).ok().flatten().is_some_and(|file| file.wrapped.is_some())
}

/// The store's key, waited for while the keyring is not open yet, asked
/// every `every` until `until`; never prompting. `Locked` at the deadline:
/// the caller waits again (the watcher, inside one start).
pub fn wait_for_root_key(folders: &Folders, until: Instant, every: Duration) -> Result<RootKey, KeyError> {
    loop {
        match root_key(folders) {
            Err(KeyError::Locked) if Instant::now() + every < until => std::thread::sleep(every),
            done => return done,
        }
    }
}

/// A new store: its key made and kept in the keyring (read back to be sure)
/// before anything is written, then the store made.
pub fn create(folders: &Folders, options: Options) -> Result<Store, KeyError> {
    if Store::id(folders).map_err(|e| KeyError::Other(e.to_string()))?.is_some() {
        return Err(KeyError::Other("what Sioul keeps on this device is sealed already".to_string()));
    }
    let id = sioul_core::sealed::new_id();
    let key = new_key_kept(&id)?;
    let store = Store::create_with(folders, &id, &key, options).map_err(|e| KeyError::Other(e.to_string()))?;
    remember(&key);
    Ok(store)
}

/// Argon2id as the sharing's (`share::key_for`): 64 MiB, 3 passes.
/// The key asked of the keyring, which may ask the person to open it (the
/// window, started by the person): each copy's id in turn.
fn root_key_asking(folders: &Folders) -> Result<RootKey, KeyError> {
    if let Some(key) = test_key() {
        return Ok(key);
    }
    let ids = Store::ids(folders).map_err(|e| KeyError::Other(e.to_string()))?;
    for id in &ids {
        if let Ok(Some(bytes)) = secret::named_bytes(&entry_name(id)) {
            let bytes = Zeroizing::new(bytes);
            let key: [u8; 32] = bytes.as_slice().try_into().map_err(|_| KeyError::Other("the keyring's entry is not a key".to_string()))?;
            return Ok(RootKey::from_bytes(key));
        }
    }
    root_key(folders)
}

/// The opener every program of Sioul sets as it starts (`files::set_opener`):
/// the profile's store opened at the first file call that needs it, its key
/// read from the keyring without ever asking it to open (the watcher started
/// at login, the command line, the agents' server). While the keyring is
/// locked, calls under Sioul's folders fail and are asked again a few seconds
/// later; nothing is ever read or written in clear beside the store.
///
/// Not on Android yet: Java reads several of Sioul's files there by their
/// paths (the doses' alarms, the home card, texts sent), so the phone's
/// files stay as they are until those readers go through the door too.
pub fn open_on_demand(origin: &'static str) {
    #[cfg(target_os = "android")]
    let _ = origin;
    #[cfg(not(target_os = "android"))]
    sioul_core::sealed::files::set_opener(Box::new(move |folders| {
        let key = root_key(folders).map_err(|e| match e {
            KeyError::Locked | KeyError::NoService | KeyError::Missing | KeyError::NoStore => sioul_core::sealed::Error::NotOpen,
            other => sioul_core::sealed::Error::Io(other.to_string()),
        })?;
        Store::open(folders, &key, Options::for_profile(origin))
    }));
}

/// What the window's start found (`begin`): ready, or what its screen
/// before the window must do first (`sioul-app`'s `storegate`).
#[derive(Debug)]
pub enum Begin {
    /// The store opened and mounted, nothing to move.
    Ready,
    /// The profile stays in clear this time (no keyring to keep a new key
    /// in, a phone, the demo, a folder a sync app carries): said, nothing lost.
    Plain(String),
    /// The profile is sealed and its key is not had yet: the keyring is
    /// locked (`Locked`), lost it (`Missing`), or there is none (`NoService`).
    NeedsKey(KeyError),
    /// The store is open (`made`: just made, its first sealing) and plain files wait to be moved in.
    Move { made: bool },
}

/// The root key this process opened the store with, kept for the ways back
/// made from the window (zeroed when replaced); none in other programs.
static ROOT: std::sync::Mutex<Option<Zeroizing<[u8; 32]>>> = std::sync::Mutex::new(None);

fn remember(key: &RootKey) {
    *ROOT.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Zeroizing::new(*key.bytes()));
}

/// The root key this process opened the store with, for a way back made now.
pub fn remembered() -> Option<RootKey> {
    ROOT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().map(|bytes| RootKey::from_bytes(**bytes))
}

/// The window's start, quick: never waits, never moves. Opens the profile's
/// store when its key is had at once, or makes it on a profile not sealed
/// yet (its key kept in the keyring first); says what is left to do.
pub fn begin(origin: &'static str) -> Begin {
    open_on_demand(origin);
    if cfg!(target_os = "android") {
        return Begin::Plain("a phone's files stay as they are until Java's readers go through the door".to_string());
    }
    // The sandbox's demo never reaches a keyring (its own D-Bus session): plain, unless the tests' key stands in.
    if std::env::var_os("SIOUL_DEMO").is_some() && test_key().is_none() {
        return Begin::Plain("the demo's profile".to_string());
    }
    let folders = Folders::of_profile();
    let ids = match Store::ids(&folders) {
        Ok(ids) => ids,
        Err(e) => return Begin::NeedsKey(KeyError::Other(e.to_string())),
    };
    if ids.is_empty() {
        // A profile a sync app carries: never sealed there (each device's store is its own), left as it was.
        if let Some(carried) = sioul_core::sealed::carried_by_sync(&folders, &Options::for_profile(origin)) {
            return Begin::Plain(format!("{} is carried by a sync app", carried.display()));
        }
        // Read in clear before the store exists. A configuration that does not
        // read would hide where the person's folders are: the sealing waits.
        let config_path = sioul_core::config::default_path();
        if crate::files::exists(&config_path) && sioul_core::config::Config::load(&config_path).is_err() {
            return Begin::Plain("the configuration does not read: the sealing waits for it".to_string());
        }
        return match create_remembered(&folders, Options::for_profile(origin)) {
            Ok(store) => {
                mount(store);
                Begin::Move { made: true }
            }
            Err(e) => Begin::Plain(e.to_string()),
        };
    }
    match root_key(&folders) {
        Ok(key) => match opened_with(&folders, &key, origin) {
            Ok(store) if store.plain_files_left() > 0 => Begin::Move { made: false },
            Ok(_) => Begin::Ready,
            Err(e) => Begin::NeedsKey(e),
        },
        Err(e) => Begin::NeedsKey(e),
    }
}

/// A new store (`create` remembers its key for the ways back).
fn create_remembered(folders: &Folders, options: Options) -> Result<Store, KeyError> {
    create(folders, options)
}

fn mount(store: Store) -> std::sync::Arc<Store> {
    let store = sioul_core::sealed::install(store);
    std::mem::forget(sioul_core::sealed::files::mount(store.clone()));
    store
}

/// The store opened with `key` (one the keyring gave, or a way back), installed for this process and mounted.
pub fn opened_with(folders: &Folders, key: &RootKey, origin: &'static str) -> Result<std::sync::Arc<Store>, KeyError> {
    let store = Store::open(folders, key, Options::for_profile(origin)).map_err(|e| match e {
        sioul_core::sealed::Error::WrongKey => KeyError::Missing,
        other => KeyError::Other(other.to_string()),
    })?;
    remember(key);
    Ok(mount(store))
}

/// The key asked of the keyring, which may ask the person to open it.
pub fn root_key_asking_now(folders: &Folders) -> Result<RootKey, KeyError> {
    root_key_asking(folders)
}

/// Plain files moved in (`Store::move_plain_files_said`), `progress` told as
/// it goes: the person's folders found inside a folder of Sioul's not moved
/// yet kept in clear, one placed there later sealed in place. The folders are
/// read from the configuration here, in the process that moves (the screen
/// before the window is one of its own), as it is on the disk now, sealed or
/// not (review-finish G2); a configuration that does not read stops the move
/// rather than seal the person's folders where they lie.
pub fn move_now(store: &Store, progress: &dyn Fn(usize, usize)) -> Result<sioul_core::sealed::Moved, KeyError> {
    let person = person_folders()?;
    store.move_plain_files_said(&person, progress).map_err(|e| KeyError::Other(e.to_string()))
}

/// The person's own folders, which the move never seals where it finds them:
/// the notes folder, the papers' and the letters' folders, where scans
/// arrive. The defaults when there is no configuration yet.
pub fn person_folders() -> Result<Vec<std::path::PathBuf>, KeyError> {
    let path = sioul_core::config::default_path();
    let config = if crate::files::exists(&path) { sioul_core::config::Config::load(&path).map_err(|e| KeyError::Other(format!("the configuration does not read, so where your folders are is not known: {e}")))? } else { sioul_core::config::Config::default() };
    let mut folders: Vec<std::path::PathBuf> = config.notes_root_path().into_iter().collect();
    folders.push(config.papers_folder());
    folders.push(config.letters_folder());
    folders.extend(config.letters_inboxes());
    Ok(folders.into_iter().map(|f| sioul_core::sealed::files::normal(&f)).collect())
}

// Ways back (docs/store.md, "Ways back"): the root key sealed under another
// secret, in store.toml, each an option the person chooses: a passphrase of
// their own (their sharing's, if they like), a recovery code written down, a
// security key. None is made without being asked; each says what it protects.

/// The ways back this profile's store keeps.
pub fn ways(folders: &Folders) -> Vec<Way> {
    StoreFile::load(folders).ok().flatten().map(|file| file.ways).unwrap_or_default()
}

/// What a way's key is bound to: its kind and the store's id.
fn way_bound(kind: &str, id: &str) -> String {
    format!("sioul-store way back\x1f{kind}\x1f{id}")
}

fn seal_with(wrapping: &[u8; 32], kind: &str, id: &str, key: &RootKey) -> Result<String, KeyError> {
    let cipher = XChaCha20Poly1305::new(wrapping.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, Payload { msg: key.bytes(), aad: way_bound(kind, id).as_bytes() }).map_err(|_| KeyError::Other("sealing".to_string()))?;
    let mut bytes = nonce.to_vec();
    bytes.extend(sealed);
    Ok(B64.encode(bytes))
}

fn open_with(wrapping: &[u8; 32], kind: &str, id: &str, sealed: &str) -> Option<RootKey> {
    let bytes = B64.decode(sealed.trim()).ok()?;
    if bytes.len() < 24 {
        return None;
    }
    let (nonce, rest) = bytes.split_at(24);
    let plain = Zeroizing::new(XChaCha20Poly1305::new(wrapping.into()).decrypt(XNonce::from_slice(nonce), Payload { msg: rest, aad: way_bound(kind, id).as_bytes() }).ok()?);
    let key: [u8; 32] = plain.as_slice().try_into().ok()?;
    Some(RootKey::from_bytes(key))
}

/// A way kept in both copies of store.toml, in place of one of the same kind
/// (and the same security key).
fn keep_way(folders: &Folders, key: &RootKey, way: Way) -> Result<(), KeyError> {
    if !sioul_core::sealed::key_opens(folders, key) {
        return Err(KeyError::Other("this key does not open what Sioul keeps here".to_string()));
    }
    let mut file = StoreFile::load(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    // A security key's way replaces the one of the same key (its serial), a stale one included.
    let ident = |w: &Way| toml::from_str::<sioul_core::securitykey::KnownCard>(&w.card).map(|c| c.ident.to_ascii_uppercase()).unwrap_or_else(|_| w.card.clone());
    file.ways.retain(|w| w.kind != way.kind || (way.kind == "card" && ident(w) != ident(&way)));
    file.ways.push(way);
    file.save(folders).map_err(|e| KeyError::Other(e.to_string()))
}

/// The way of `kind` taken out (every one of that kind).
pub fn forget_way(folders: &Folders, kind: &str) -> Result<(), KeyError> {
    let mut file = StoreFile::load(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    file.ways.retain(|w| w.kind != kind);
    file.save(folders).map_err(|e| KeyError::Other(e.to_string()))
}

/// The fewest characters a passphrase way takes: one guessed offline at
/// Argon2id's cost must still take long.
pub const PASSPHRASE_CHARS: usize = 12;

/// The root key kept under a passphrase of the person's own (Argon2id, as
/// the sharing's). `said`: what it is to them ("sharing" when it is their
/// sharing's passphrase), told again when it is asked.
pub fn keep_passphrase_way(folders: &Folders, key: &RootKey, passphrase: &str, said: &str) -> Result<(), KeyError> {
    if passphrase.chars().count() < PASSPHRASE_CHARS {
        return Err(KeyError::Other(format!("a passphrase of {PASSPHRASE_CHARS} characters at least")));
    }
    let id = Store::id(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let wrapping = derive(passphrase, &salt, MEMORY_KIB, PASSES)?;
    let sealed = seal_with(&wrapping, "passphrase", &id, key)?;
    keep_way(folders, key, Way { kind: "passphrase".into(), made_ms: now_ms(), salt: B64.encode(salt), memory_kib: MEMORY_KIB, passes: PASSES, key: sealed, said: said.to_string(), ..Way::default() })
}

/// The root key from one of `file`'s ways, `opens` telling the key that opens its store.
fn key_from(file: &StoreFile, given: Given, opens: &dyn Fn(&RootKey) -> bool) -> Result<RootKey, KeyError> {
    match given {
        Given::Passphrase(passphrase) => passphrase_key(file, passphrase, opens),
        Given::Code(code) => code_key_of(file, code, opens),
        Given::Card { pin, readers } => card_key(file, pin, readers, opens),
    }
}

/// The root key from a passphrase way: `WrongPassphrase` when it opens none, `Missing` when there is none.
pub fn recover_with_passphrase(folders: &Folders, passphrase: &str) -> Result<RootKey, KeyError> {
    let file = StoreFile::load(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    match passphrase_key(&file, passphrase, &|key| sioul_core::sealed::key_opens(folders, key)) {
        // The passphrase kept where there is no keyring (`wrapped`) is one too.
        Err(KeyError::WrongPassphrase | KeyError::Missing) if file.wrapped.is_some() => from_passphrase(folders, passphrase),
        done => done,
    }
}

fn passphrase_key(file: &StoreFile, passphrase: &str, opens: &dyn Fn(&RootKey) -> bool) -> Result<RootKey, KeyError> {
    let mut any = false;
    for way in file.ways.iter().filter(|w| w.kind == "passphrase") {
        any = true;
        let Ok(salt) = B64.decode(&way.salt) else { continue };
        if !cost_allowed(way.memory_kib, way.passes) {
            continue;
        }
        let wrapping = derive(passphrase, &salt, way.memory_kib, way.passes)?;
        if let Some(key) = open_with(&wrapping, "passphrase", &file.id, &way.key).filter(|key| opens(key)) {
            return Ok(key);
        }
    }
    Err(if any { KeyError::WrongPassphrase } else { KeyError::Missing })
}

/// Crockford's base32: no I, L, O or U, so that a code read aloud or copied by hand stays one.
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A recovery code made, the root key sealed under it: 32 random bytes,
/// shown as 13 groups of four letters and digits, once. Its key comes from
/// it alone (HKDF-SHA-256): it holds 256 bits, no stretching needed. A new
/// code replaces the one before.
pub fn make_recovery_code(folders: &Folders, key: &RootKey) -> Result<String, KeyError> {
    let id = Store::id(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    let mut secret = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(&mut secret[..]);
    let wrapping = code_key(&secret[..], &id);
    let sealed = seal_with(&wrapping, "code", &id, key)?;
    keep_way(folders, key, Way { kind: "code".into(), made_ms: now_ms(), key: sealed, ..Way::default() })?;
    Ok(code_text(&secret[..]))
}

fn code_key(secret: &[u8], id: &str) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    let _ = hkdf::Hkdf::<sha2::Sha256>::new(Some(id.as_bytes()), secret).expand(b"sioul-store recovery code", &mut out[..]);
    out
}

/// 32 bytes as 52 base32 letters and digits, in groups of four.
fn code_text(bytes: &[u8]) -> String {
    let mut bits = 0u32;
    let mut held = 0u32;
    let mut out = String::new();
    for &b in bytes {
        held = (held << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(CODE_ALPHABET[((held >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(CODE_ALPHABET[((held << (5 - bits)) & 31) as usize] as char);
    }
    out.as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).to_string()).collect::<Vec<_>>().join("-")
}

/// A code as typed back into 32 bytes: spaces, dashes and case ignored, O read 0, I and L read 1.
fn code_bytes(text: &str) -> Option<Zeroizing<Vec<u8>>> {
    let mut bits = 0u32;
    let mut held = 0u32;
    let mut out = Zeroizing::new(Vec::with_capacity(32));
    for c in text.chars().filter(|c| !c.is_whitespace() && *c != '-') {
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        let value = CODE_ALPHABET.iter().position(|&a| a as char == c)? as u32;
        held = (held << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((held >> bits) & 0xff) as u8);
        }
    }
    (out.len() == 32).then_some(out)
}

/// The root key from the recovery code: `WrongCode` when it opens nothing, `Missing` when no code was made.
pub fn recover_with_code(folders: &Folders, code: &str) -> Result<RootKey, KeyError> {
    let file = StoreFile::load(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    code_key_of(&file, code, &|key| sioul_core::sealed::key_opens(folders, key))
}

fn code_key_of(file: &StoreFile, code: &str, opens: &dyn Fn(&RootKey) -> bool) -> Result<RootKey, KeyError> {
    let ways: Vec<&Way> = file.ways.iter().filter(|w| w.kind == "code").collect();
    if ways.is_empty() {
        return Err(KeyError::Missing);
    }
    let secret = code_bytes(code).ok_or(KeyError::WrongCode)?;
    let wrapping = code_key(&secret, &file.id);
    ways.iter().find_map(|way| open_with(&wrapping, "code", &file.id, &way.key)).filter(|key| opens(key)).ok_or(KeyError::WrongCode)
}

/// The root key sealed to each security key of the person's known here that
/// can decrypt (its encryption key; no PIN or touch needed to seal): how
/// many. `store.toml` is in clear, so the way keeps only what finds the key
/// and opens the message: the card's serial and the fingerprints of its
/// certificate and encryption key, the certificate without its names and
/// addresses (review-finish G9).
pub fn keep_card_ways(folders: &Folders, key: &RootKey, keys: &sioul_core::pgp::Keys) -> Result<usize, KeyError> {
    let cards = sioul_core::pgp::cards_for_ways(keys);
    for (card, cert) in &cards {
        let message = sioul_core::pgp::seal_to_cert(key.bytes(), cert).map_err(KeyError::Other)?;
        let found_by = sioul_core::securitykey::KnownCard { ident: card.ident.clone(), decrypt: card.decrypt.clone(), cert: card.cert.clone(), ..Default::default() };
        let card = toml::to_string(&found_by).map_err(|e| KeyError::Other(e.to_string()))?;
        keep_way(folders, key, Way { kind: "card".into(), made_ms: now_ms(), key: message, card, cert: cert.clone(), ..Way::default() })?;
    }
    Ok(cards.len())
}

/// The security keys whose way no longer opens anything: the card known
/// here now (by its serial) decrypts with another key than the one its way
/// was sealed to (reset, or given a new encryption key). Their serials, for
/// the window to say once and offer to seal again (review-finish G10).
pub fn stale_cards(folders: &Folders, keys: &sioul_core::pgp::Keys) -> Vec<String> {
    ways(folders)
        .into_iter()
        .filter(|way| way.kind == "card")
        .filter_map(|way| toml::from_str::<sioul_core::securitykey::KnownCard>(&way.card).ok())
        .filter(|kept| keys.cards.iter().any(|now| now.ident.eq_ignore_ascii_case(&kept.ident) && !now.decrypt.is_empty() && !now.decrypt.eq_ignore_ascii_case(&kept.decrypt)))
        .map(|kept| kept.ident)
        .collect()
}

/// The root key from a security key: its PIN, then a touch where it asks
/// one; `NoCard` when none there opens a way (absent, another, PIN refused).
pub fn recover_with_card(folders: &Folders, pin: &str, readers: &dyn sioul_core::securitykey::Readers) -> Result<RootKey, KeyError> {
    let file = StoreFile::load(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    card_key(&file, pin, readers, &|key| sioul_core::sealed::key_opens(folders, key))
}

fn card_key(file: &StoreFile, pin: &str, readers: &dyn sioul_core::securitykey::Readers, opens: &dyn Fn(&RootKey) -> bool) -> Result<RootKey, KeyError> {
    let ways: Vec<&Way> = file.ways.iter().filter(|w| w.kind == "card").collect();
    if ways.is_empty() {
        return Err(KeyError::Missing);
    }
    let mut why = String::new();
    for way in ways {
        match sioul_core::pgp::open_with_card(&way.key, &way.cert, &way.card, readers, pin) {
            Ok(bytes) => {
                let bytes = Zeroizing::new(bytes);
                if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()).map(RootKey::from_bytes)
                    && opens(&key)
                {
                    return Ok(key);
                }
            }
            Err(e) => why = e,
        }
    }
    Err(KeyError::NoCard(why))
}

/// A key a way back gave, kept in the keyring again under the store's id,
/// so that the next start needs no way back.
pub fn keep_in_keyring(folders: &Folders, key: &RootKey) -> Result<(), KeyError> {
    if test_key().is_some() {
        return Ok(());
    }
    let id = Store::id(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    secret::save_named_bytes(&entry_name(&id), key.bytes()).map_err(|e| KeyError::Other(e.to_string()))
}

/// Starting again without the key: a new key kept in the keyring first (and
/// read back), so that nothing is set aside when the keyring cannot keep one;
/// then what is sealed here set aside whole, never removed
/// (`sealed-unopened-<when>` beside each `sealed/` folder, with its
/// `store.toml`), which a way back found later opens again
/// (`reopen_set_aside`); then the new store made. Mail, agendas and contacts
/// come back from their servers; what the sharing carries comes back from the
/// other devices once this one joins it again with its passphrase. Only for a
/// key truly gone (`Missing`): never while the keyring is merely locked
/// (review-finish G4).
pub fn start_again(folders: &Folders, options: Options) -> Result<Store, KeyError> {
    let id = sioul_core::sealed::new_id();
    let key = new_key_kept(&id)?;
    set_aside_sealed(folders)?;
    let store = Store::create_with(folders, &id, &key, options).map_err(|e| KeyError::Other(e.to_string()))?;
    remember(&key);
    Ok(store)
}

/// A new root key for store `id`, kept in the keyring and read back (the test key in tests).
fn new_key_kept(id: &str) -> Result<RootKey, KeyError> {
    if let Some(key) = test_key() {
        return Ok(key);
    }
    let key = RootKey::generate();
    secret::save_named_bytes(&entry_name(id), key.bytes()).map_err(|e| KeyError::Other(e.to_string()))?;
    match secret::named_bytes_quietly(&entry_name(id)) {
        Quiet::Found(bytes) if bytes.as_slice() == key.bytes() => Ok(key),
        Quiet::NoService => Err(KeyError::NoService),
        _ => Err(KeyError::Other("the keyring did not keep the store's key".to_string())),
    }
}

/// What is sealed here set aside whole (`start_again`'s second step): each
/// `sealed/` folder renamed `sealed-unopened-<when>`, `store.toml` with the
/// configuration's. Nothing is removed. Its `<when>`.
pub fn set_aside_sealed(folders: &Folders) -> Result<String, KeyError> {
    let now = jiff::Zoned::now().strftime("%Y%m%d-%H%M%S").to_string();
    // A name no store set aside holds yet (two in the same second: reopened, then set aside again).
    let taken = |when: &str| sioul_core::sealed::Root::ALL.iter().any(|root| crate::files::exists(folders.of(*root).join(format!("{ASIDE}{when}"))));
    let when = std::iter::once(now.clone()).chain((2..).map(|n| format!("{now}-{n}"))).find(|when| !taken(when)).unwrap_or(now);
    let io = |e: std::io::Error| KeyError::Other(e.to_string());
    for root in sioul_core::sealed::Root::ALL {
        let sealed = folders.sealed(root);
        if crate::files::exists(&sealed) {
            crate::files::rename(&sealed, folders.of(root).join(format!("{ASIDE}{when}"))).map_err(io)?;
        }
    }
    if crate::files::exists(folders.store_file()) {
        let aside = folders.of(sioul_core::sealed::Root::Config).join(format!("{ASIDE}{when}"));
        crate::files::create_dir_all(&aside).map_err(io)?;
        crate::files::rename(folders.store_file(), aside.join("store.toml")).map_err(io)?;
    }
    Ok(when)
}

/// The name before a set-aside store's `<when>`.
const ASIDE: &str = "sealed-unopened-";

/// The stores set aside here by "Start again", by their `<when>`, the newest first.
pub fn set_aside_stores(folders: &Folders) -> Vec<String> {
    let mut all: Vec<String> = crate::files::read_dir(&folders.config)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().and_then(|name| name.strip_prefix(ASIDE)).map(str::to_string))
        .filter(|when| set_aside_file(folders, when).is_some())
        .collect();
    all.sort();
    all.reverse();
    all
}

/// A set-aside store's `store.toml` (its first copy, else its second).
fn set_aside_file(folders: &Folders, when: &str) -> Option<StoreFile> {
    if when.is_empty() || when.contains(['/', '\\']) || when.contains("..") {
        return None;
    }
    let first = folders.config.join(format!("{ASIDE}{when}")).join("store.toml");
    let second = folders.data.join(format!("{ASIDE}{when}")).join("store.toml");
    [first, second].iter().find_map(|path| StoreFile::load_at(path).ok().flatten())
}

/// The ways back a set-aside store keeps (its own `store.toml`'s).
pub fn set_aside_ways(folders: &Folders, when: &str) -> Vec<Way> {
    set_aside_file(folders, when).map(|file| file.ways).unwrap_or_default()
}

/// A way back the person gives, to open a store.
pub enum Given<'a> {
    Passphrase(&'a str),
    Code(&'a str),
    Card { pin: &'a str, readers: &'a dyn sioul_core::securitykey::Readers },
}

/// The root key of a set-aside store, from one of its ways back.
pub fn set_aside_key(folders: &Folders, when: &str, given: Given) -> Result<RootKey, KeyError> {
    let file = set_aside_file(folders, when).ok_or(KeyError::NoStore)?;
    key_from(&file, given, &|key| file.opens_with(key))
}

/// A set-aside store put back as this profile's, opened with `key` (one of
/// its ways gave it): its key kept in the keyring first, the store here now
/// set aside in its turn (nothing removed), then its folders put back. So
/// "Start again" is never for good: what it set aside opens again, then
/// exports (`sioul store export`) or stays.
pub fn reopen_set_aside(folders: &Folders, when: &str, key: &RootKey) -> Result<(), KeyError> {
    let file = set_aside_file(folders, when).ok_or(KeyError::NoStore)?;
    if !file.opens_with(key) {
        return Err(KeyError::Missing);
    }
    if test_key().is_none() {
        secret::save_named_bytes(&entry_name(&file.id), key.bytes()).map_err(|e| KeyError::Other(e.to_string()))?;
    }
    put_back_set_aside(folders, when, key).map(|_| ())
}

/// `reopen_set_aside`'s folders, its key kept already: the store here now set
/// aside (its `<when>`, when there was one), then the one set aside at `when`
/// put back.
pub fn put_back_set_aside(folders: &Folders, when: &str, key: &RootKey) -> Result<Option<String>, KeyError> {
    let file = set_aside_file(folders, when).ok_or(KeyError::NoStore)?;
    if !file.opens_with(key) {
        return Err(KeyError::Missing);
    }
    let current = if Store::id(folders).map_err(|e| KeyError::Other(e.to_string()))?.is_some() { Some(set_aside_sealed(folders)?) } else { None };
    let io = |e: std::io::Error| KeyError::Other(e.to_string());
    for root in sioul_core::sealed::Root::ALL {
        let aside = folders.of(root).join(format!("{ASIDE}{when}"));
        if crate::files::exists(&aside) {
            crate::files::rename(&aside, folders.sealed(root)).map_err(io)?;
        }
    }
    let first = folders.sealed(sioul_core::sealed::Root::Config).join("store.toml");
    if crate::files::exists(&first) {
        crate::files::rename(&first, folders.store_file()).map_err(io)?;
    }
    Ok(current)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

const MEMORY_KIB: u32 = 64 * 1024;
const PASSES: u32 = 3;

/// The Argon2id cost a way may ask, read from `store.toml`: at most what
/// Sioul writes there, so that a damaged or forged file never makes a try
/// take gigabytes and minutes (review-finish G10).
fn cost_allowed(memory_kib: u32, passes: u32) -> bool {
    (8 * 1024..=MEMORY_KIB).contains(&memory_kib) && (1..=PASSES).contains(&passes)
}

fn derive(passphrase: &str, salt: &[u8], memory_kib: u32, passes: u32) -> Result<Zeroizing<[u8; 32]>, KeyError> {
    let params = argon2::Params::new(memory_kib, passes, 1, Some(32)).map_err(|e| KeyError::Other(e.to_string()))?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params).hash_password_into(passphrase.as_bytes(), salt, &mut key[..]).map_err(|e| KeyError::Other(e.to_string()))?;
    Ok(key)
}

fn bound(id: &str) -> String {
    format!("sioul-store wrapped\x1f{id}")
}

/// The store's key kept wrapped under a passphrase in `store.toml`, where no keyring keeps it.
pub fn keep_with_passphrase(folders: &Folders, key: &RootKey, passphrase: &str) -> Result<(), KeyError> {
    if passphrase.is_empty() {
        return Err(KeyError::Other("a passphrase is needed".to_string()));
    }
    let mut file = StoreFile::load(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let wrapping = derive(passphrase, &salt, MEMORY_KIB, PASSES)?;
    let cipher = XChaCha20Poly1305::new((&*wrapping).into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, Payload { msg: key.bytes(), aad: bound(&file.id).as_bytes() }).map_err(|_| KeyError::Other("sealing".to_string()))?;
    let mut bytes = nonce.to_vec();
    bytes.extend(sealed);
    file.wrapped = Some(Wrapped { salt: B64.encode(salt), memory_kib: MEMORY_KIB, passes: PASSES, key: B64.encode(bytes) });
    file.save(folders).map_err(|e| KeyError::Other(e.to_string()))
}

/// The store's key unwrapped with the passphrase (no keyring service).
pub fn from_passphrase(folders: &Folders, passphrase: &str) -> Result<RootKey, KeyError> {
    if let Some(key) = test_key() {
        return Ok(key);
    }
    let file = StoreFile::load(folders).map_err(|e| KeyError::Other(e.to_string()))?.ok_or(KeyError::NoStore)?;
    let wrapped = file.wrapped.as_ref().ok_or(KeyError::Missing)?;
    let damaged = || KeyError::Other("store.toml's wrapped key does not read".to_string());
    let salt = B64.decode(&wrapped.salt).map_err(|_| damaged())?;
    let bytes = B64.decode(&wrapped.key).map_err(|_| damaged())?;
    if bytes.len() < 24 || !cost_allowed(wrapped.memory_kib, wrapped.passes) {
        return Err(damaged());
    }
    let wrapping = derive(passphrase, &salt, wrapped.memory_kib, wrapped.passes)?;
    let cipher = XChaCha20Poly1305::new((&*wrapping).into());
    let (nonce, rest) = bytes.split_at(24);
    let plain = Zeroizing::new(cipher.decrypt(XNonce::from_slice(nonce), Payload { msg: rest, aad: bound(&file.id).as_bytes() }).map_err(|_| KeyError::WrongPassphrase)?);
    let key: [u8; 32] = plain.as_slice().try_into().map_err(|_| damaged())?;
    Ok(RootKey::from_bytes(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("sioul-ways-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        base
    }

    /// The keyring's entry lost (its key is gone): each way back the person
    /// kept gives the key again, a wrong one is told and opens nothing.
    #[test]
    fn ways_back_open_what_sioul_keeps_once_the_keyring_lost_its_key() {
        let base = scratch("lost");
        let folders = Folders::under(&base);
        let key = RootKey::from_bytes([9u8; 32]);
        drop(Store::create(&folders, &key, Options::default()).unwrap());
        // None kept yet: said, never a guess.
        assert_eq!(recover_with_passphrase(&folders, "four long words of mine").unwrap_err(), KeyError::Missing);
        assert_eq!(recover_with_code(&folders, "0000").unwrap_err(), KeyError::Missing);
        assert!(ways(&folders).is_empty());
        // A passphrase too short, or a key that does not open the store: refused.
        assert!(keep_passphrase_way(&folders, &key, "short", "").is_err());
        assert!(keep_passphrase_way(&folders, &RootKey::from_bytes([1u8; 32]), "four long words of mine", "").is_err());
        keep_passphrase_way(&folders, &key, "four long words of mine", "sharing").unwrap();
        let code = make_recovery_code(&folders, &key).unwrap();
        assert_eq!(code.split('-').count(), 13, "{code}");
        assert_eq!(ways(&folders).iter().map(|w| w.kind.as_str()).collect::<Vec<_>>(), ["passphrase", "code"]);
        let text = std::fs::read_to_string(folders.store_file()).unwrap();
        assert!(!text.contains("four long words") && !text.contains(&code), "no secret in clear");
        // Recovered: the same key, which opens the store.
        assert_eq!(recover_with_passphrase(&folders, "four long words of mine").unwrap().bytes(), key.bytes());
        assert_eq!(recover_with_passphrase(&folders, "four long words of yours").unwrap_err(), KeyError::WrongPassphrase);
        let typed = code.to_lowercase().replace('-', " ").replace('0', "o");
        let back = recover_with_code(&folders, &typed).unwrap();
        assert_eq!(back.bytes(), key.bytes(), "typed loosely: case, spaces, O for 0");
        assert!(Store::open(&folders, &back, Options::default()).is_ok());
        let mut wrong: Vec<char> = code.chars().collect();
        wrong[0] = if wrong[0] == 'A' { 'B' } else { 'A' };
        assert_eq!(recover_with_code(&folders, &wrong.into_iter().collect::<String>()).unwrap_err(), KeyError::WrongCode);
        assert_eq!(recover_with_code(&folders, "too short").unwrap_err(), KeyError::WrongCode);
        // A new code replaces the one before; a way taken out opens nothing.
        let second = make_recovery_code(&folders, &key).unwrap();
        assert_eq!(recover_with_code(&folders, &code).unwrap_err(), KeyError::WrongCode);
        assert!(recover_with_code(&folders, &second).is_ok());
        forget_way(&folders, "passphrase").unwrap();
        assert_eq!(recover_with_passphrase(&folders, "four long words of mine").unwrap_err(), KeyError::Missing);
        // A Sioul that knows no way back reads this store.toml all the same, passing over them.
        #[derive(serde::Deserialize)]
        struct Earlier {
            format: u32,
            id: String,
        }
        let earlier: Earlier = toml::from_str(&std::fs::read_to_string(folders.store_file()).unwrap()).unwrap();
        assert_eq!((earlier.format, earlier.id.is_empty()), (1, false));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// No security key there (a stand-in reader with none, never the person's own): told, nothing opened.
    #[test]
    fn a_security_key_absent_opens_nothing() {
        use sequoia_openpgp::cert::CertBuilder;
        use sequoia_openpgp::policy::StandardPolicy;
        struct NoKey;
        impl sioul_core::securitykey::Readers for NoKey {
            fn present(&self) -> Result<Vec<String>, sioul_core::securitykey::CardError> {
                Ok(Vec::new())
            }
            fn visit(&self, _: &mut dyn FnMut(&mut dyn sioul_core::securitykey::Card) -> sioul_core::securitykey::Visit) -> Result<sioul_core::securitykey::Visited, sioul_core::securitykey::CardError> {
                Ok(sioul_core::securitykey::Visited { opened: 0, busy: 0 })
            }
        }
        let base = scratch("card");
        let folders = Folders::under(&base);
        let key = RootKey::from_bytes([5u8; 32]);
        drop(Store::create(&folders, &key, Options::default()).unwrap());
        assert_eq!(recover_with_card(&folders, "123456", &NoKey).unwrap_err(), KeyError::Missing);
        // A key made here stands for the card's certificate (invented address).
        let (cert, _) = CertBuilder::general_purpose(Some("card@example.org")).generate().unwrap();
        let policy = StandardPolicy::new();
        let decrypt = cert.keys().with_policy(&policy, None).supported().for_transport_encryption().next().unwrap().key().fingerprint().to_hex();
        let card = sioul_core::securitykey::KnownCard { ident: "0006:00000001".into(), decrypt, cert: cert.fingerprint().to_hex(), ..Default::default() };
        let keys = sioul_core::pgp::Keys { own: vec![cert], cards: vec![card], ..Default::default() };
        assert_eq!(keep_card_ways(&folders, &key, &keys).unwrap(), 1);
        assert_eq!(ways(&folders)[0].kind, "card");
        assert!(matches!(recover_with_card(&folders, "123456", &NoKey), Err(KeyError::NoCard(_))));
        // None known here: nothing kept, said by the count.
        assert_eq!(keep_card_ways(&folders, &key, &sioul_core::pgp::Keys::default()).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Without any way back: what is sealed is set aside whole, a new store made.
    #[test]
    fn starting_again_sets_what_is_sealed_aside_and_removes_nothing() {
        let base = scratch("again");
        let folders = Folders::under(&base);
        let key = RootKey::from_bytes([3u8; 32]);
        let store = Store::create(&folders, &key, Options::default()).unwrap();
        store.write(&sioul_core::sealed::LogicalPath::new("data/records/a.toml").unwrap(), b"a", Default::default()).unwrap();
        drop(store);
        let files_before = walk_count(&base);
        // `start_again`'s first step (its second, `create`, keeps a new key in the keyring: never in a test).
        set_aside_sealed(&folders).unwrap();
        assert_eq!(Store::id(&folders).unwrap(), None, "no store here any more");
        assert_eq!(walk_count(&base), files_before, "nothing removed");
        let aside: Vec<_> = std::fs::read_dir(&folders.data).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("sealed-unopened-")).collect();
        assert_eq!(aside.len(), 1, "the old store set aside");
        // A new store beside it; the old one still opens with its key, once put back.
        let again = RootKey::from_bytes([4u8; 32]);
        drop(Store::create(&folders, &again, Options::default()).unwrap());
        assert!(sioul_core::sealed::key_opens(&folders, &again) && !sioul_core::sealed::key_opens(&folders, &key));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A security key's way opens the store with the card alone (a
    /// stand-in card, never the person's): its certificate kept without its
    /// names opens all the same. Then the card given a new encryption key is
    /// said stale, and an Argon2 cost larger than Sioul's is never run.
    #[test]
    fn a_security_key_s_way_opens_with_the_card_and_is_said_stale_once_renewed() {
        use sequoia_openpgp::policy::StandardPolicy;
        use sioul_core::securitykey::{KnownCard, SoftCard, SoftReaders};
        let base = scratch("card-opens");
        let folders = Folders::under(&base);
        let key = RootKey::from_bytes([6u8; 32]);
        drop(Store::create(&folders, &key, Options::default()).unwrap());
        let (card, public) = SoftCard::generate("0006:00000007", &["Card Holder <holder@example.org>"], "123456").unwrap();
        let cert = sioul_core::pgp::certificates_in(&public).remove(0);
        let policy = StandardPolicy::new();
        let decrypt = cert.keys().with_policy(&policy, None).supported().for_transport_encryption().next().unwrap().key().fingerprint().to_hex();
        let known = KnownCard { ident: "0006:00000007".into(), decrypt: decrypt.clone(), cert: cert.fingerprint().to_hex(), holder: "Holder<<Card".into(), addresses: vec!["holder@example.org".into()], ..Default::default() };
        let keys = sioul_core::pgp::Keys { own: vec![cert.clone()], cards: vec![known.clone()], ..Default::default() };
        assert_eq!(keep_card_ways(&folders, &key, &keys).unwrap(), 1);
        let text = std::fs::read_to_string(folders.store_file()).unwrap();
        assert!(!text.contains("Holder") && !text.contains("holder@example.org"), "no name in clear");
        let readers = SoftReaders::with(vec![card.with_x25519(|scalar, point| x25519_dalek::x25519(*scalar, *point))]);
        assert_eq!(recover_with_card(&folders, "123456", &readers).unwrap().bytes(), key.bytes(), "the card alone opens it");
        assert!(matches!(recover_with_card(&folders, "000000", &readers), Err(KeyError::NoCard(_))), "a PIN refused opens nothing");
        assert!(stale_cards(&folders, &keys).is_empty());
        let renewed = sioul_core::pgp::Keys { cards: vec![KnownCard { decrypt: "0".repeat(40), ..known }], ..Default::default() };
        assert_eq!(stale_cards(&folders, &renewed), ["0006:00000007"], "a card given a new encryption key: its way said stale");
        // A cost larger than Sioul writes, read from a file damaged or forged: never run.
        assert!(cost_allowed(MEMORY_KIB, PASSES) && !cost_allowed(4 * 1024 * 1024, 3) && !cost_allowed(MEMORY_KIB, 64));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// "Start again" set a store aside: listed, its ways read from its own
    /// `store.toml`, opened by its recovery code, and put back as the
    /// profile's, the store made meanwhile set aside in its turn; nothing removed.
    #[test]
    fn a_store_set_aside_opens_again_with_a_way_back() {
        let base = scratch("reopen");
        let folders = Folders::under(&base);
        let old = RootKey::from_bytes([3u8; 32]);
        let store = Store::create(&folders, &old, Options::default()).unwrap();
        store.write(&sioul_core::sealed::LogicalPath::new("data/records/a.toml").unwrap(), b"kept", Default::default()).unwrap();
        drop(store);
        let code = make_recovery_code(&folders, &old).unwrap();
        let when = set_aside_sealed(&folders).unwrap();
        assert_eq!(set_aside_stores(&folders), [when.clone()]);
        assert_eq!(set_aside_ways(&folders, &when).iter().map(|w| w.kind.as_str()).collect::<Vec<_>>(), ["code"]);
        // A new store meanwhile, made by "Start again".
        let new = RootKey::from_bytes([4u8; 32]);
        let store = Store::create(&folders, &new, Options::default()).unwrap();
        store.write(&sioul_core::sealed::LogicalPath::new("data/records/b.toml").unwrap(), b"new", Default::default()).unwrap();
        drop(store);
        assert_eq!(set_aside_key(&folders, &when, Given::Code("0000")).unwrap_err(), KeyError::WrongCode);
        let back = set_aside_key(&folders, &when, Given::Code(&code)).unwrap();
        assert_eq!(back.bytes(), old.bytes());
        assert_eq!(reopen_set_aside(&folders, &when, &new).unwrap_err(), KeyError::Missing, "only its own key puts it back, the keyring never asked");
        // Its folders put back (`reopen_set_aside` keeps its key in the keyring first: never in a test).
        let current = put_back_set_aside(&folders, &when, &back).unwrap().unwrap();
        let reopened = Store::open(&folders, &old, Options::default()).unwrap();
        assert_eq!(reopened.read(&sioul_core::sealed::LogicalPath::new("data/records/a.toml").unwrap()).unwrap().unwrap().bytes, b"kept");
        assert_eq!(set_aside_stores(&folders), [current.clone()], "the store made meanwhile set aside in its turn");
        assert!(set_aside_file(&folders, &current).is_some_and(|file| file.opens_with(&new)));
        let _ = std::fs::remove_dir_all(&base);
    }

    fn walk_count(dir: &std::path::Path) -> usize {
        let mut n = 0;
        let mut stack = vec![dir.to_path_buf()];
        while let Some(at) = stack.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    stack.push(entry.path());
                } else {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn a_passphrase_keeps_the_key_where_no_keyring_does() {
        let base = std::env::temp_dir().join(format!("sioul-storekey-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let folders = Folders::under(&base);
        let key = RootKey::from_bytes([42u8; 32]);
        let store = Store::create(&folders, &key, Options::default()).unwrap();
        drop(store);
        assert_eq!(from_passphrase(&folders, "x").unwrap_err(), KeyError::Missing);
        keep_with_passphrase(&folders, &key, "a long passphrase of one's own").unwrap();
        let text = std::fs::read_to_string(folders.store_file()).unwrap();
        assert!(!text.contains("a long passphrase") && text.contains("[wrapped]"));
        let back = from_passphrase(&folders, "a long passphrase of one's own").unwrap();
        assert_eq!(back.bytes(), key.bytes());
        assert!(Store::open(&folders, &back, Options::default()).is_ok());
        assert_eq!(from_passphrase(&folders, "another").unwrap_err(), KeyError::WrongPassphrase);
        // No store, no keyring asked (this test never reaches a keyring).
        assert_eq!(root_key(&Folders::under(&base.join("none"))).unwrap_err(), KeyError::NoStore);
        let _ = std::fs::remove_dir_all(&base);
    }

    mod review2_tests {
        use super::*;

        #[test]
        fn review2_a_security_key_way_keeps_no_name_in_clear() {
            use sequoia_openpgp::cert::CertBuilder;
            use sequoia_openpgp::parse::Parse;
            use sequoia_openpgp::policy::StandardPolicy;
            let base = std::env::temp_dir().join(format!("sioul-review2-card-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let folders = Folders::under(&base);
            let key = RootKey::from_bytes([5u8; 32]);
            drop(Store::create(&folders, &key, Options::default()).unwrap());
            let (cert, _) = CertBuilder::general_purpose(Some("Lise Martin <lise@example.org>")).generate().unwrap();
            let policy = StandardPolicy::new();
            let decrypt = cert.keys().with_policy(&policy, None).supported().for_transport_encryption().next().unwrap().key().fingerprint().to_hex();
            let card = sioul_core::securitykey::KnownCard { ident: "0006:00000001".into(), decrypt, cert: cert.fingerprint().to_hex(), holder: "Martin<<Lise".into(), addresses: vec!["lise@example.org".into()], ..Default::default() };
            let keys = sioul_core::pgp::Keys { own: vec![cert], cards: vec![card], ..Default::default() };
            assert_eq!(keep_card_ways(&folders, &key, &keys).unwrap(), 1);
            let text = std::fs::read_to_string(folders.store_file()).unwrap();
            let way = ways(&folders).remove(0);
            let uids: Vec<String> = sequoia_openpgp::Cert::from_bytes(way.cert.as_bytes()).unwrap().userids().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).collect();
            let _ = std::fs::remove_dir_all(&base);
            assert!(!text.contains("Lise") && !text.contains("lise@example.org") && uids.is_empty(), "store.toml, in clear on the disk: holder named {}, address {}, the certificate's user IDs {uids:?}", text.contains("Lise"), text.contains("lise@example.org"));
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Passwords live in the system keyring, never in a file Sioul writes: the
//! Secret Service on Linux (KWallet, GNOME Keyring), the Keychain on macOS,
//! the Credential Manager on Windows (Cargo.toml chooses the backend per
//! system). Each is filed under the service `sioul` and `<login> on
//! <server>`, so it is easy to find, check or remove there.
//!
//! On Linux, FreeBSD and OpenBSD (where the `keyring` crate keeps passwords in
//! the Secret Service) a read goes through one encrypted session with the
//! Secret Service kept for the process (`kept`), rather than a new one, its
//! Diffie–Hellman exchange included, at each read as the `keyring` crate
//! makes: the window's sharing reads a password every minute. Writes, and
//! what that session leaves aside, take the `keyring` crate's own way.

use crate::SyncError;
use keyring::Entry;
use sioul_core::config::Account;

const SERVICE: &str = "sioul";

/// The name an account's password is kept under: `<login> on <server>`.
fn user_of(account: &Account) -> Result<String, SyncError> {
    let host = account.host.as_deref().ok_or(SyncError::NoServer)?;
    let login = account.login().ok_or(SyncError::NoServer)?;
    Ok(format!("{login} on {host}"))
}

fn entry(account: &Account) -> Result<Entry, SyncError> {
    Entry::new(SERVICE, &user_of(account)?).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// The secret kept under `user`, as bytes: through the session kept for the
/// process where there is one (`kept`), else the `keyring` crate's own read,
/// which also gives every case that session leaves aside (an item locked,
/// two alike, one of the keyring's first version) as it always did.
fn read_secret(user: &str) -> Result<Vec<u8>, keyring::Error> {
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
    match kept::read(user) {
        kept::Read::Found(secret) => return Ok(secret),
        kept::Read::None => return Err(keyring::Error::NoEntry),
        kept::Read::Elsewhere => {}
    }
    Entry::new(SERVICE, user)?.get_secret()
}

/// The secret kept under `user`, as text (`read_secret`); one that is not
/// UTF-8 is refused by the `keyring` crate's own read, as before.
fn read_password(user: &str) -> Result<String, keyring::Error> {
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
    match kept::read(user) {
        kept::Read::Found(secret) => {
            if let Ok(text) = String::from_utf8(secret) {
                return Ok(text);
            }
        }
        kept::Read::None => return Err(keyring::Error::NoEntry),
        kept::Read::Elsewhere => {}
    }
    Entry::new(SERVICE, user)?.get_password()
}

/// Keeps the password for the account, replacing any older one.
pub fn save(account: &Account, password: &str) -> Result<(), SyncError> {
    if testing() {
        return Ok(());
    }
    entry(account)?.set_password(password).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// Whether the keyring is left alone: tests against a local server
/// (`SIOUL_TEST_PASSWORD`), and the tests of the crates built on this one
/// (the `no-keyring` feature); never in Sioul's builds, whose binaries the
/// final pass reads for this mark. On Windows, the one test of this crate
/// that checks the Credential Manager on purpose reaches it whatever the
/// feature says, on its own thread (`with_real_keyring`).
pub(crate) fn testing() -> bool {
    #[cfg(all(test, windows))]
    if REAL_KEYRING.with(std::cell::Cell::get) {
        return false;
    }
    #[cfg(feature = "insecure-test-tls")]
    if std::env::var_os("SIOUL_TEST_PASSWORD").is_some() {
        return true;
    }
    no_keyring()
}

#[cfg(test)]
thread_local! {
    /// Set by `with_real_keyring`, on its thread alone.
    static REAL_KEYRING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `f`, with the system's keyring reached on this thread on Windows alone:
/// the Windows job's own check of the Credential Manager
/// (`microsoft::tests::windows_keeps_a_4_kb_token`). CI tests core, sync and
/// the command line in one command (build.yml), and Cargo then turns the
/// command line's `no-keyring` on for this crate's tests too (run
/// 39216227853: the test found nothing kept, the feature keeping nothing).
/// Anywhere else the keyring is the person's own, and nothing changes; on
/// every other thread neither. In this crate's own tests only: never in a
/// build of Sioul, nor in another crate's tests.
#[cfg(test)]
pub(crate) fn with_real_keyring<T>(f: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            REAL_KEYRING.with(|real| real.set(false));
        }
    }
    REAL_KEYRING.with(|real| real.set(true));
    let _reset = Reset;
    f()
}

/// The `no-keyring` feature, its mark kept in the binary for the final pass to find.
#[cfg(feature = "no-keyring")]
fn no_keyring() -> bool {
    !std::hint::black_box("SIOUL_TEST_NO_KEYRING: this build keeps nothing in the keyring").is_empty()
}

#[cfg(not(feature = "no-keyring"))]
fn no_keyring() -> bool {
    false
}

/// The password kept for the account. A mail account signed in with Google
/// has none: what stands for it is its grant's mark (`sasl::OAuth::mark`),
/// unused by the servers, which are given an access token instead; no grant
/// kept here, Google's sign-in is asked again.
pub fn password(account: &Account) -> Result<String, SyncError> {
    if let Some(oauth) = crate::sasl::OAuth::of(account) {
        return oauth.mark(account.login().ok_or(SyncError::NoServer)?);
    }
    // Tests against a local server leave the keyring alone (never in Sioul's builds).
    #[cfg(feature = "insecure-test-tls")]
    if let Ok(password) = std::env::var("SIOUL_TEST_PASSWORD") {
        return Ok(password);
    }
    if testing() {
        return Err(SyncError::NoPassword);
    }
    match read_password(&user_of(account)?) {
        Ok(password) => Ok(password),
        Err(keyring::Error::NoEntry) => Err(SyncError::NoPassword),
        Err(e) => Err(SyncError::Keyring(e.to_string())),
    }
}

fn passphrase_entry(fingerprint: &str) -> Result<Entry, SyncError> {
    Entry::new(SERVICE, &passphrase_user(fingerprint)).map_err(|e| SyncError::Keyring(e.to_string()))
}

fn passphrase_user(fingerprint: &str) -> String {
    format!("OpenPGP key {fingerprint}")
}

/// The passphrase of your OpenPGP key, when the keyring keeps it.
pub fn pgp_passphrase(fingerprint: &str) -> Option<String> {
    #[cfg(feature = "insecure-test-tls")]
    if let Ok(passphrase) = std::env::var("SIOUL_TEST_PASSWORD") {
        return Some(passphrase);
    }
    // This crate's own tests, and those built on it, leave the keyring alone:
    // their stand-in keys have no passphrase.
    if cfg!(test) || testing() {
        return None;
    }
    read_password(&passphrase_user(fingerprint)).ok()
}

/// In a test build with `SIOUL_TEST_PASSWORD`, the passphrase every key gets
/// (the keyring is left alone); None in Sioul's builds.
pub fn test_passphrase() -> Option<String> {
    #[cfg(feature = "insecure-test-tls")]
    if let Ok(passphrase) = std::env::var("SIOUL_TEST_PASSWORD") {
        return Some(passphrase);
    }
    None
}

/// Keeps the passphrase of your OpenPGP key in the keyring.
pub fn save_pgp_passphrase(fingerprint: &str, passphrase: &str) -> Result<(), SyncError> {
    if testing() {
        return Ok(());
    }
    passphrase_entry(fingerprint)?.set_password(passphrase).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// A secret kept under a name of Sioul's ("Bitwarden device token for …").
pub fn named(name: &str) -> Option<String> {
    if testing() {
        return None;
    }
    read_password(name).ok()
}

/// A secret kept under a name of Sioul's, told apart from none kept: an
/// error when the keyring did not answer (locked, its service not running
/// for a moment), never taken for none.
pub fn named_checked(name: &str) -> Result<Option<String>, SyncError> {
    if testing() {
        return Ok(None);
    }
    match read_password(name) {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SyncError::Keyring(e.to_string())),
    }
}

/// Keeps a secret under a name of Sioul's.
pub fn save_named(name: &str, secret: &str) -> Result<(), SyncError> {
    if testing() {
        return Ok(());
    }
    Entry::new(SERVICE, name).and_then(|e| e.set_password(secret)).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// Forgets a secret kept under a name of Sioul's; nothing to do if there was none.
pub fn forget_named(name: &str) -> Result<(), SyncError> {
    if testing() {
        return Ok(());
    }
    match Entry::new(SERVICE, name).and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SyncError::Keyring(e.to_string())),
    }
}

/// The most a keyring entry holds on every system: Windows' Credential
/// Manager keeps 2560 bytes (`CRED_MAX_CREDENTIAL_BLOB_SIZE`), and a text
/// kept as a password there takes two bytes a character (UTF-16). A secret
/// kept as bytes (`save_named_bytes`) takes its UTF-8 length; this leaves a margin.
pub const ENTRY_BYTES: usize = 2400;

/// A secret kept as bytes under a name of Sioul's (`save_named_bytes`):
/// None when there is none, Err when the keyring did not answer.
pub fn named_bytes(name: &str) -> Result<Option<Vec<u8>>, SyncError> {
    if testing() {
        return Ok(None);
    }
    match read_secret(name) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SyncError::Keyring(e.to_string())),
    }
}

/// Keeps a secret as bytes (UTF-8 as it is, not Windows' UTF-16), replacing
/// any older one: at most `ENTRY_BYTES`, which a longer secret is split
/// into by its owner (`microsoft.rs` does for its long tokens).
pub fn save_named_bytes(name: &str, secret: &[u8]) -> Result<(), SyncError> {
    if testing() {
        return Ok(());
    }
    if secret.len() > ENTRY_BYTES {
        return Err(SyncError::Keyring(format!("{name}: {} bytes, more than an entry holds everywhere ({ENTRY_BYTES})", secret.len())));
    }
    Entry::new(SERVICE, name).and_then(|e| e.set_secret(secret)).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// Forgets the account's password; nothing to do if there was none.
pub fn forget(account: &Account) -> Result<(), SyncError> {
    if testing() {
        return Ok(());
    }
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SyncError::Keyring(e.to_string())),
    }
}

/// One session with the Secret Service, kept for the process (Linux, FreeBSD
/// and OpenBSD): reading a secret then costs a search and the secret's own
/// message, not a new connection and a new Diffie–Hellman exchange each time
/// (the `keyring` crate opens both at every read: the window's sharing read
/// one a minute, a tenth of its exchange's time, measured on 11 November 2026).
///
/// The session stays encrypted (`EncryptionType::Dh`), never plain; no
/// secret is held: each is asked anew, as before. The session's AES key stays
/// as long as the session, so for the process's life now (zeroed by the
/// library once dropped), where it lived one read: whoever could record this
/// process's messages to the keyring's service could read them with it, and
/// that is someone who can already ask the unlocked keyring itself. Its
/// connection is made by `dbus-secret-service`, the library the `keyring`
/// crate reads through, as before: through libdbus's own registration, once
/// for the process now rather than at each read (docs/android.md, "the
/// keyring is the exposure left").
///
/// The items, their attributes and where they are looked for are the
/// `keyring` crate's (`target` "default", `service`, `username`; then its
/// first version's, without `target`, in the default collection): what it
/// kept is found as before. This session answers two cases alone, one item
/// found unlocked (its secret) and none found anywhere (none kept); every
/// other case (an item locked, which the keyring's service may ask to unlock,
/// two items alike, an item of the first version, a secret that is not text
/// where text is asked, an error) goes the `keyring` crate's own way, as it
/// always did, with its own session. So does a read while another thread
/// holds this one: none waits behind another's unlocking.
///
/// The session is used only while the service that made it still has its
/// name on the bus (its owner's unique name, asked through `crate::bus` before
/// each read): a keyring service started again is never sent the session of
/// the one before, whatever it does with a session it does not know. A
/// session that fails is dropped, and made again once at once when the
/// failure was the service's (it forgot the session); a call the bus or the
/// service did not answer in time is left to the `keyring` crate's own way,
/// the session made again at the next read, never at once on a bus that may be
/// stuck.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
mod kept {
    use dbus_secret_service::{EncryptionType, Error, SecretService};
    use std::collections::HashMap;
    use std::sync::{Mutex, TryLockError};
    use std::time::Duration;

    /// The Secret Service's name on the bus.
    const SECRETS: &str = "org.freedesktop.secrets";

    /// The session kept, with the connection its owner is asked through.
    pub(super) struct Session {
        service: SecretService,
        /// A connection of Sioul's own (`crate::bus`), to ask who has the name.
        bus: dbus::channel::Channel,
        /// The unique name of the service the session was made with.
        owner: String,
    }

    /// Where a session is kept.
    pub(super) type Slot = Mutex<Option<Session>>;

    /// The session kept, once one was made.
    static SESSION: Slot = Mutex::new(None);

    #[cfg(test)]
    thread_local! {
        /// Tests only: how many sessions this thread made.
        pub(super) static MADE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        /// Tests only: how many sessions this thread dropped because the service's owner changed.
        pub(super) static OWNER_CHANGED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        /// Tests only: `look` panics on this thread, as a service sending what the library cannot read.
        pub(super) static PANICS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// What the kept session found for a name.
    pub(super) enum Read {
        /// One item, unlocked: its secret.
        Found(Vec<u8>),
        /// No item at all, where the `keyring` crate looks.
        None,
        /// Anything else: left to the `keyring` crate's own read.
        Elsewhere,
    }

    /// The secret kept under `user` (service `sioul`), through the session kept.
    pub(super) fn read(user: &str) -> Read {
        read_in(&SESSION, user)
    }

    /// `read`, with the session kept in `slot` (the process's, or a test's own).
    pub(super) fn read_in(slot: &Slot, user: &str) -> Read {
        let mut held = match slot.try_lock() {
            Ok(held) => held,
            Err(TryLockError::Poisoned(held)) => held.into_inner(),
            Err(TryLockError::WouldBlock) => return Read::Elsewhere,
        };
        for attempt in 0..2 {
            // The session kept, while the service that made it has the name.
            if let Some(session) = held.as_ref() {
                match owner(&session.bus) {
                    Some(now) if now == session.owner => {}
                    Some(_) => {
                        *held = None;
                        #[cfg(test)]
                        OWNER_CHANGED.with(|n| n.set(n.get() + 1));
                    }
                    // The bus or the name not answering: none made now.
                    None => {
                        *held = None;
                        return Read::Elsewhere;
                    }
                }
            }
            if held.is_none() {
                let Some(made) = make() else { return Read::Elsewhere };
                *held = Some(made);
                #[cfg(test)]
                MADE.with(|n| n.set(n.get() + 1));
            }
            let Some(session) = held.as_ref() else { return Read::Elsewhere };
            let looked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                #[cfg(test)]
                if PANICS.with(std::cell::Cell::get) {
                    panic!("a stand-in for a reply the library cannot read");
                }
                look(&session.service, user)
            }));
            match looked {
                Ok(Ok(read)) => return read,
                Ok(Err(e)) => {
                    // Never used again: the next read makes another.
                    *held = None;
                    if attempt > 0 || !made_again(&e) {
                        return Read::Elsewhere;
                    }
                }
                Err(_) => {
                    *held = None;
                    return Read::Elsewhere;
                }
            }
        }
        Read::Elsewhere
    }

    /// A session, its connection to ask the owner, and the owner's unique name.
    fn make() -> Option<Session> {
        let service = SecretService::connect(EncryptionType::Dh).ok()?;
        let bus = crate::bus::session().ok()?;
        let owner = owner(&bus)?;
        Some(Session { service, bus, owner })
    }

    /// Who has the Secret Service's name on the bus now (its unique name),
    /// asked of the bus itself, two seconds at most; none, nobody or no answer.
    fn owner(bus: &dbus::channel::Channel) -> Option<String> {
        let ask = dbus::Message::new_method_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner").ok()?.append1(SECRETS);
        bus.send_with_reply_and_block(ask, Duration::from_secs(2)).ok()?.read1::<String>().ok()
    }

    /// Whether a failure is mended by a new session at once: the service's
    /// answer that it does not know the session (it started again, or closed
    /// it), never a call the bus or the service did not answer in time, which a
    /// new connection would wait for too.
    pub(super) fn made_again(e: &Error) -> bool {
        match e {
            Error::Dbus(e) => !matches!(e.name(), Some("org.freedesktop.DBus.Error.NoReply" | "org.freedesktop.DBus.Error.Timeout" | "org.freedesktop.DBus.Error.TimedOut")),
            _ => false,
        }
    }

    /// `user`'s item where the `keyring` crate looks for it (its
    /// `map_matching_items`, then `map_matching_legacy_items`).
    fn look(session: &SecretService, user: &str) -> Result<Read, Error> {
        let found = session.search_items(HashMap::from([("target", "default"), ("service", super::SERVICE), ("username", user)]))?;
        match (found.unlocked.as_slice(), found.locked.len()) {
            ([item], 0) => Ok(Read::Found(item.get_secret()?)),
            ([], 0) => {
                // None: as the keyring does, an item of its first version, without a target, in the default collection.
                let legacy = session.get_default_collection().and_then(|collection| collection.search_items(HashMap::from([("service", super::SERVICE), ("username", user)])).map(|items| items.is_empty()));
                match legacy {
                    Ok(true) => Ok(Read::None),
                    Err(e @ Error::Dbus(_)) => Err(e),
                    _ => Ok(Read::Elsewhere),
                }
            }
            _ => Ok(Read::Elsewhere),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CI's Windows job tests core, sync and the command line in one
    /// command, which turns the command line's `no-keyring` on for this
    /// crate's tests too (run 39216227853, `windows_keeps_a_4_kb_token`:
    /// "the grant written did not read back whole"). The one test that
    /// checks Windows' Credential Manager reaches it there, on its own
    /// thread; on Linux and macOS, and on any other thread, the keyring is
    /// left as the build says, whatever that test asks.
    #[test]
    fn only_the_windows_test_of_the_keyring_reaches_it_and_only_there() {
        let built = testing();
        let inside = with_real_keyring(|| (testing(), std::thread::spawn(testing).join().unwrap()));
        assert_eq!(inside, (built && !cfg!(windows), built), "on Windows its own thread alone reaches the keyring");
        assert_eq!(testing(), built, "given back after");
    }
}

/// The session kept for reads (`kept`), against a GNOME keyring of the test's
/// own: started on the test's own bus (dbus-run-session), its files in a
/// scratch folder, and only when that bus has no Secret Service of its own,
/// nor one it could start, and is not the person's session bus. Elsewhere
/// (no such bus, no gnome-keyring-daemon) it says so and passes. Never the
/// person's keyring: on the person's session bus a Secret Service is there or
/// can be started, and the test goes no further.
#[cfg(all(test, any(target_os = "linux", target_os = "freebsd", target_os = "openbsd")))]
mod kept_tests {
    use super::*;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    const SECRETS: &str = "org.freedesktop.secrets";

    /// The tests that start a keyring of their own take turns: one name, one owner.
    static KEYRINGS: Mutex<()> = Mutex::new(());

    /// A question to the bus itself, its one answer.
    fn ask_bus<T>(method: &str, name: Option<&str>, read: impl FnOnce(&dbus::Message) -> Option<T>) -> Option<T> {
        let channel = crate::bus::session().ok()?;
        let mut call = dbus::Message::new_method_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", method).ok()?;
        if let Some(name) = name {
            call = call.append1(name);
        }
        let answer = channel.send_with_reply_and_block(call, Duration::from_secs(5)).ok()?;
        read(&answer)
    }

    /// The process that has the Secret Service's name on the bus now, if any.
    fn secrets_owner() -> Option<u32> {
        let owned = ask_bus("NameHasOwner", Some(SECRETS), |m| m.read1::<bool>().ok())?;
        if !owned {
            return None;
        }
        ask_bus("GetConnectionUnixProcessID", Some(SECRETS), |m| m.read1::<u32>().ok())
    }

    /// Whether the session bus is a test's own: not the person's (its usual
    /// place), no Secret Service on it, none it could start.
    fn a_test_bus() -> bool {
        let Some(address) = crate::bus::session_address() else { return false };
        if address.contains("/run/user/") {
            return false;
        }
        let startable = ask_bus("ListActivatableNames", None, |m| m.read1::<Vec<String>>().ok()).map(|names| names.iter().any(|n| n == SECRETS));
        startable == Some(false) && ask_bus("NameHasOwner", Some(SECRETS), |m| m.read1::<bool>().ok()) == Some(false)
    }

    /// A GNOME keyring of the test's own, its login collection unlocked with
    /// a stand-in password, its files in `dir`; stopped when dropped.
    struct StandInKeyring {
        child: Child,
    }

    impl StandInKeyring {
        fn start(dir: &Path) -> Option<StandInKeyring> {
            for sub in ["data", "config", "cache", "run"] {
                std::fs::create_dir_all(dir.join(sub)).ok()?;
            }
            let _ = std::fs::set_permissions(dir.join("run"), std::os::unix::fs::PermissionsExt::from_mode(0o700));
            let mut child = Command::new("gnome-keyring-daemon")
                .args(["--foreground", "--unlock", "--components=secrets"])
                .env("HOME", dir)
                .env("XDG_DATA_HOME", dir.join("data"))
                .env("XDG_CONFIG_HOME", dir.join("config"))
                .env("XDG_CACHE_HOME", dir.join("cache"))
                .env("XDG_RUNTIME_DIR", dir.join("run"))
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let written = child.stdin.take().map(|mut stdin| stdin.write_all(b"stand-in"));
            let keyring = StandInKeyring { child };
            written?.ok()?;
            let until = Instant::now() + Duration::from_secs(10);
            while Instant::now() < until {
                match secrets_owner() {
                    Some(pid) if pid == keyring.child.id() => return Some(keyring),
                    Some(pid) => panic!("another process ({pid}) has the Secret Service's name on the test's bus"),
                    None => std::thread::sleep(Duration::from_millis(50)),
                }
            }
            None
        }
    }

    impl Drop for StandInKeyring {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let until = Instant::now() + Duration::from_secs(5);
            while secrets_owner().is_some() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn found(read: kept::Read) -> Option<Vec<u8>> {
        match read {
            kept::Read::Found(secret) => Some(secret),
            _ => None,
        }
    }

    /// Three reads take one session; a name with nothing kept is told from
    /// the same session; the keyring's service started again, another process
    /// with another unique name on the bus, the next read sees its owner
    /// changed and makes one new session before it asks anything, and the
    /// reads after it keep that one. A read whose look panics (a reply the
    /// library cannot read) drops the session and is left to the `keyring`
    /// crate. What the session finds is what the `keyring` crate wrote and
    /// reads, under the same items and attributes.
    #[test]
    #[ignore = "starts a GNOME keyring of its own, whose calls would fail the final pass's keyring count: tools/final-pass.sh runs it apart, on a bus of its own"]
    fn one_session_serves_every_read_and_is_made_again_once_the_service_went() {
        let _turn = KEYRINGS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if !a_test_bus() {
            eprintln!("skipped: not on a test's own bus with no Secret Service (run under dbus-run-session)");
            return;
        }
        let scratch = Scratch(std::env::temp_dir().join(format!("sioul-kept-session-{}", std::process::id())));
        let Some(stand_in) = StandInKeyring::start(&scratch.0) else {
            eprintln!("skipped: no GNOME keyring could start (gnome-keyring-daemon)");
            return;
        };
        let user = "kept-session-test on example.org";
        keyring::Entry::new(SERVICE, user).unwrap().set_password("first secret").unwrap();
        let slot: kept::Slot = Mutex::new(None);
        let made = || kept::MADE.with(std::cell::Cell::get);
        let changed = || kept::OWNER_CHANGED.with(std::cell::Cell::get);
        let (before, changed_before) = (made(), changed());
        for _ in 0..3 {
            assert_eq!(found(kept::read_in(&slot, user)).as_deref(), Some(&b"first secret"[..]));
        }
        assert_eq!(made() - before, 1, "one session for three reads");
        assert!(matches!(kept::read_in(&slot, "nobody on example.org"), kept::Read::None), "nothing kept under that name");
        assert_eq!(made() - before, 1, "told from the same session");
        assert_eq!(keyring::Entry::new(SERVICE, user).unwrap().get_password().unwrap(), "first secret", "the keyring's own read agrees");

        // The service stopped and started again: the session kept is gone with it.
        drop(stand_in);
        let stand_in = StandInKeyring::start(&scratch.0).expect("the keyring started again");
        keyring::Entry::new(SERVICE, user).unwrap().set_password("second secret").unwrap();
        assert_eq!(found(kept::read_in(&slot, user)).as_deref(), Some(&b"second secret"[..]), "read through a new session");
        assert_eq!((made() - before, changed() - changed_before), (2, 1), "made again once, its owner having changed");
        assert_eq!(found(kept::read_in(&slot, user)).as_deref(), Some(&b"second secret"[..]));
        assert_eq!((made() - before, changed() - changed_before), (2, 1), "and kept again");

        // A look that panics: the session dropped, the read left to the keyring crate; the next makes another.
        kept::PANICS.with(|p| p.set(true));
        assert!(matches!(kept::read_in(&slot, user), kept::Read::Elsewhere));
        kept::PANICS.with(|p| p.set(false));
        assert!(slot.lock().unwrap().is_none(), "dropped after the panic");
        assert_eq!(found(kept::read_in(&slot, user)).as_deref(), Some(&b"second secret"[..]));
        assert_eq!(made() - before, 3);

        // No service at all: left to the keyring's own read, which says why.
        drop(stand_in);
        assert!(matches!(kept::read_in(&slot, user), kept::Read::Elsewhere));
        assert!(slot.lock().unwrap().is_none(), "no session kept with no service");
    }

    /// A call the bus or the service did not answer in time is never made
    /// again at once (a new connection would wait on the same bus, without
    /// limit); a service that answers it does not know the session is.
    #[test]
    fn a_call_not_answered_in_time_is_not_made_again_at_once() {
        let error = |name: &str| dbus_secret_service::Error::Dbus(dbus::Error::new_custom(name, "a stand-in"));
        assert!(!kept::made_again(&error("org.freedesktop.DBus.Error.NoReply")));
        assert!(!kept::made_again(&error("org.freedesktop.DBus.Error.Timeout")));
        assert!(kept::made_again(&error("org.freedesktop.Secret.Error.NoSession")));
        assert!(kept::made_again(&error("org.freedesktop.DBus.Error.UnknownObject")));
        assert!(!kept::made_again(&dbus_secret_service::Error::Locked));
    }

    /// What a read costs, through the `keyring` crate (a connection and a
    /// session each) and through the session kept: this thread's CPU time
    /// (Linux's `schedstat`) and the wall time, per read, against a GNOME
    /// keyring of the test's own. Ignored: run it under dbus-run-session with
    /// `cargo test --release -p sioul-sync kept_session_bench -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn kept_session_bench() {
        let _turn = KEYRINGS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if !a_test_bus() {
            eprintln!("skipped: not on a test's own bus with no Secret Service (run under dbus-run-session)");
            return;
        }
        let scratch = Scratch(std::env::temp_dir().join(format!("sioul-kept-bench-{}", std::process::id())));
        let Some(_stand_in) = StandInKeyring::start(&scratch.0) else { return };
        let user = "kept-session-bench on example.org";
        keyring::Entry::new(SERVICE, user).unwrap().set_password("a stand-in password").unwrap();
        let cpu_ns = || std::fs::read_to_string("/proc/thread-self/schedstat").ok().and_then(|t| t.split_whitespace().next().and_then(|n| n.parse::<u64>().ok())).unwrap_or(0);
        const READS: u32 = 50;
        let time = |what: &str, read: &dyn Fn() -> String| {
            let (cpu, wall) = (cpu_ns(), Instant::now());
            for _ in 0..READS {
                assert_eq!(read(), "a stand-in password");
            }
            println!("{what}: {:.2} ms of CPU, {:.2} ms per read", (cpu_ns() - cpu) as f64 / 1e6 / f64::from(READS), wall.elapsed().as_secs_f64() * 1e3 / f64::from(READS));
        };
        time("the keyring crate", &|| keyring::Entry::new(SERVICE, user).unwrap().get_password().unwrap());
        let slot: kept::Slot = Mutex::new(None);
        time("the session kept", &|| String::from_utf8(found(kept::read_in(&slot, user)).unwrap()).unwrap());
    }
}

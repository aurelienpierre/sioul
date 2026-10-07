// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Passwords live in the system keyring, never in a file Sioul writes: the
//! Secret Service on Linux (KWallet, GNOME Keyring), the Keychain on macOS,
//! the Credential Manager on Windows (Cargo.toml chooses the backend per
//! system). Each is filed under the service `sioul` and `<login> on
//! <server>`, so it is easy to find, check or remove there.

use crate::SyncError;
use keyring::Entry;
use sioul_core::config::Account;

const SERVICE: &str = "sioul";

fn entry(account: &Account) -> Result<Entry, SyncError> {
    let host = account.host.as_deref().ok_or(SyncError::NoServer)?;
    let login = account.login().ok_or(SyncError::NoServer)?;
    Entry::new(SERVICE, &format!("{login} on {host}")).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// Keeps the password for the account, replacing any older one.
pub fn save(account: &Account, password: &str) -> Result<(), SyncError> {
    if testing() {
        return Ok(());
    }
    entry(account)?.set_password(password).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// Tests against a local server leave the keyring alone (never in Sioul's builds).
fn testing() -> bool {
    #[cfg(feature = "insecure-test-tls")]
    if std::env::var_os("SIOUL_TEST_PASSWORD").is_some() {
        return true;
    }
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
    match entry(account)?.get_password() {
        Ok(password) => Ok(password),
        Err(keyring::Error::NoEntry) => Err(SyncError::NoPassword),
        Err(e) => Err(SyncError::Keyring(e.to_string())),
    }
}

fn passphrase_entry(fingerprint: &str) -> Result<Entry, SyncError> {
    Entry::new(SERVICE, &format!("OpenPGP key {fingerprint}")).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// The passphrase of your OpenPGP key, when the keyring keeps it.
pub fn pgp_passphrase(fingerprint: &str) -> Option<String> {
    #[cfg(feature = "insecure-test-tls")]
    if let Ok(passphrase) = std::env::var("SIOUL_TEST_PASSWORD") {
        return Some(passphrase);
    }
    passphrase_entry(fingerprint).ok()?.get_password().ok()
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
    Entry::new(SERVICE, name).ok()?.get_password().ok()
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

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The store's keys (docs/store.md, "The key"): one root key per store, 256
//! random bits kept in the system's keyring by `sioul_sync::storekey` (never
//! here: the core takes a key, it never fetches one). From it, HKDF-SHA-256
//! (salt: the store's id) makes the keys that work, so that the root key
//! seals nothing itself and two stores never share one: the sealing key (the
//! blobs' pieces), the naming key (opaque names, HMAC-SHA-256), the lines'
//! key (files that only grow, and the manifests' journals) and the check key
//! (`store.toml`'s value that tells a wrong key from damage).

use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{KeyInit, OsRng};
use chacha20poly1305::XChaCha20Poly1305;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

/// A store's root key: zeroed when let go, never printed.
pub struct RootKey(Box<[u8; 32]>);

impl RootKey {
    pub fn from_bytes(bytes: [u8; 32]) -> RootKey {
        let mut bytes = Zeroizing::new(bytes);
        RootKey(Box::new(std::mem::take(&mut *bytes)))
    }

    /// 256 random bits from the system.
    pub fn generate() -> RootKey {
        let mut key = Box::new([0u8; 32]);
        OsRng.fill_bytes(&mut key[..]);
        RootKey(key)
    }

    /// For the keyring and the ways back, nothing else.
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The fixed key of tests and of the bench: never a real one. Only in a
    /// test build (`test-store-key`), whose binary carries the mark the
    /// shipping check refuses (tools/check-test-marks.sh).
    #[cfg(any(test, feature = "test-store-key"))]
    pub fn test() -> RootKey {
        let _ = std::hint::black_box(MARK);
        RootKey::from_bytes(*b"sioul test store key, never real")
    }
}

/// What a build that can make the test key carries, for the shipping check to find.
#[cfg(any(test, feature = "test-store-key"))]
pub const MARK: &str = "SIOUL_TEST_STORE_KEY: this build can open a store with the fixed test key";

impl Drop for RootKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for RootKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RootKey(…)")
    }
}

/// The keys that work, made from the root key for one store.
pub(crate) struct Keys {
    pub sealing: XChaCha20Poly1305,
    pub naming: Hmac<Sha256>,
    pub lines: XChaCha20Poly1305,
    pub check: XChaCha20Poly1305,
}

impl Keys {
    pub fn derive(root: &RootKey, id: &str) -> Keys {
        let made = hkdf::Hkdf::<Sha256>::new(Some(id.as_bytes()), root.bytes());
        let key = |info: &[u8]| {
            let mut key = Zeroizing::new([0u8; 32]);
            made.expand(info, &mut key[..]).expect("32 bytes is a length HKDF gives");
            key
        };
        Keys {
            sealing: XChaCha20Poly1305::new((&*key(b"sioul-store 1: sealing")).into()),
            naming: <Hmac<Sha256> as Mac>::new_from_slice(&key(b"sioul-store 1: naming")[..]).expect("HMAC takes any key"),
            lines: XChaCha20Poly1305::new((&*key(b"sioul-store 1: lines")).into()),
            check: XChaCha20Poly1305::new((&*key(b"sioul-store 1: check")).into()),
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `store.toml`, in the configuration's folder, in clear on purpose: the
//! format, the store's id (what its key is kept under in the keyring), and a
//! value sealed with the check key, so that a wrong key is told before
//! anything is read and is never taken for damage. On a system without a
//! keyring, the root key wrapped under a passphrase (Argon2id, as the
//! sharing's `seal.toml`: `sioul_sync::storekey`). The move's progress is not
//! here: it is sealed in the store (`Store::moved`), and never goes back.

use super::disk;
use super::keys::Keys;
use super::{Error, Folders};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::{Aead, AeadCore, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};

/// The format of `store.toml` and of the store this Sioul writes.
pub const FORMAT: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreFile {
    pub format: u32,
    pub id: String,
    pub check: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapped: Option<Wrapped>,
    /// The ways back to the root key when the keyring lost it (docs/store.md,
    /// "Ways back"), each the key sealed under another secret: a passphrase,
    /// a recovery code, a security key. In clear on purpose, as `wrapped`:
    /// each opens only with its own secret. An older Sioul reading this file
    /// passes over them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ways: Vec<Way>,
}

/// One way back to the root key (`StoreFile::ways`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Way {
    /// "passphrase", "code" or "card".
    pub kind: String,
    /// When it was made (milliseconds since 1970).
    #[serde(default)]
    pub made_ms: i64,
    /// The passphrase's way: its Argon2id salt and cost; none for the others.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub salt: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub memory_kib: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub passes: u32,
    /// The root key sealed: a nonce and XChaCha20-Poly1305's output in base64;
    /// for a security key, an OpenPGP message to its encryption key, armored.
    pub key: String,
    /// The security key's way: the card as Sioul knows it (TOML) and its
    /// certificate (armored, public): what opening the message needs when
    /// everything sealed is out of reach.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub card: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cert: String,
    /// What the person was told it was (the sharing's passphrase, a passphrase of their own).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub said: String,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// The root key wrapped under a passphrase, where no keyring keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wrapped {
    pub salt: String,
    pub memory_kib: u32,
    pub passes: u32,
    pub key: String,
}

const CHECK: &str = "sioul-store check 1";

impl StoreFile {
    /// Both copies as they read: `store.toml`, then its second copy (in the
    /// data folder's `sealed/`, another folder than the first's). It exists
    /// nowhere else, and without its id the store's key cannot be found.
    pub(crate) fn load_both(folders: &Folders) -> Vec<StoreFile> {
        [folders.store_file(), second(folders)].iter().filter_map(|path| read_one(path).ok().flatten()).collect()
    }

    /// `store.toml` read, else its second copy: none when neither is there;
    /// `Damaged` when neither reads; `Refused` when a newer Sioul wrote it.
    pub fn load(folders: &Folders) -> Result<Option<StoreFile>, Error> {
        let first = match read_one(&folders.store_file()) {
            done @ (Ok(Some(_)) | Err(Error::Refused(_))) => return done,
            other => other,
        };
        match read_one(&second(folders)) {
            Ok(Some(file)) => Ok(Some(file)),
            Err(e @ Error::Refused(_)) => Err(e),
            second => first.and(second),
        }
    }

    /// One copy read where it lies (a store set aside by "Start again", whose
    /// folders are not where a store's are): none when it is not there.
    pub fn load_at(path: &std::path::Path) -> Result<Option<StoreFile>, Error> {
        read_one(path)
    }

    /// Whether `key` opens this file's check value: the key of its store.
    pub fn opens_with(&self, key: &super::RootKey) -> bool {
        self.opens(&Keys::derive(key, &self.id))
    }

    /// Written whole, twice: beside, flushed, renamed; the first copy, then the second.
    /// A store's file is written last when it is made: its lists are there before it.
    pub fn save(&self, folders: &Folders) -> Result<(), Error> {
        let text = toml::to_string(self).map_err(|e| Error::Io(e.to_string()))?;
        let text = format!("# Sioul's sealed store (docs/store.md). In clear on purpose: it tells nothing of what is kept.\n{text}");
        for path in [folders.store_file(), second(folders)] {
            disk::place(&path, |out| out.write_all(text.as_bytes())).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
        }
        Ok(())
    }

    /// A new store's file: a new id, and the check value sealed with its keys.
    pub(crate) fn new(id: String, keys: &Keys) -> StoreFile {
        let check = seal_check(&keys.check, &id);
        StoreFile { format: FORMAT, id, check, wrapped: None, ways: Vec::new() }
    }

    /// Whether these keys open the check value.
    pub(crate) fn opens(&self, keys: &Keys) -> bool {
        let Ok(bytes) = B64.decode(self.check.trim()) else { return false };
        if bytes.len() < 24 {
            return false;
        }
        let (nonce, rest) = bytes.split_at(24);
        keys.check.decrypt(XNonce::from_slice(nonce), Payload { msg: rest, aad: aad(&self.id).as_bytes() }).is_ok_and(|plain| plain == format!("{CHECK}\x1f{}", self.id).as_bytes())
    }
}

/// `store.toml`'s second copy, in the data folder's `sealed/` (another folder than the first's).
fn second(folders: &Folders) -> std::path::PathBuf {
    folders.sealed(super::Root::Data).join("store.toml")
}

fn read_one(path: &std::path::Path) -> Result<Option<StoreFile>, Error> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::Io(format!("{}: {e}", path.display()))),
    };
    let file: StoreFile = toml::from_str(&text).map_err(|e| Error::Damaged { path: "store.toml".to_string(), what: e.message().to_string() })?;
    if file.format > FORMAT {
        return Err(Error::Refused(format!("this store was made by a newer Sioul (format {})", file.format)));
    }
    Ok(Some(file))
}

/// Whether sealed files are there, in any of the four folders: then a missing
/// `store.toml` is damage, never a profile without a store.
pub(crate) fn has_blobs(folders: &Folders) -> bool {
    super::Root::ALL.iter().any(|root| !super::manifest::blob_places(&folders.sealed(*root)).is_empty())
}

fn aad(id: &str) -> String {
    format!("sioul-store check\x1f{id}")
}

fn seal_check(cipher: &XChaCha20Poly1305, id: &str) -> String {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, Payload { msg: format!("{CHECK}\x1f{id}").as_bytes(), aad: aad(id).as_bytes() }).unwrap_or_default();
    let mut bytes = nonce.to_vec();
    bytes.extend(sealed);
    B64.encode(bytes)
}

/// A new store's id: 128 random bits in hex.
pub fn new_id() -> String {
    use chacha20poly1305::aead::rand_core::RngCore;
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    super::format::hex(&bytes)
}


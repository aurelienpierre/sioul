// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! OpenPGP behind the window: protected messages opened for the reader, the
//! keys Autocrypt brings, what a draft can be (signed, encrypted), and your
//! keys: made, imported, exported, removed. Passphrases stay in the keyring;
//! decrypted messages stay in memory.

use crate::backend::{json, load_config, say, tr};
use sioul_core::compose::Draft;
use sioul_core::pgp::{self, Keys, PgpView};
use sioul_core::porch::Triaged;
use sioul_core::trust::Trust;
use sioul_sync::{SyncError, keys, secret};
use std::path::{Path, PathBuf};

/// Whether a message might be protected: cheap, before any key is read.
fn looks_protected(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(&raw[..raw.len().min(512 * 1024)]);
    text.contains("application/pgp-encrypted") || text.contains("application/pgp-signature") || text.contains("-----BEGIN PGP MESSAGE-----")
}

/// A protected message opened with your keys: what it reads once decrypted,
/// and what was found; None when it is not protected.
pub(crate) fn open(raw: &[u8]) -> Option<(Option<Vec<u8>>, PgpView)> {
    if !looks_protected(raw) {
        return None;
    }
    pgp::open(raw, &Keys::load(), &|fingerprint| secret::pgp_passphrase(&fingerprint.to_hex()))
}

/// The message as it reads: decrypted when it was encrypted to you.
pub(crate) fn readable(path: &Path) -> Option<Vec<u8>> {
    let raw = std::fs::read(path).ok()?;
    Some(open(&raw).and_then(|(opened, _)| opened).unwrap_or(raw))
}

/// Keeps the key a sender's Autocrypt header carries, only when the sender's
/// domain vouches for the message (the Porch's judgement): an unproven one
/// could plant a key for someone you write to, and read your answers.
pub(crate) fn learn_autocrypt(raw: &[u8], triaged: &Triaged) {
    if triaged.trust != Trust::Verified {
        return;
    }
    let Some(from) = triaged.card.from_address.as_deref() else { return };
    if let Some(header) = triaged.card.headers.first("Autocrypt") {
        let _ = raw;
        pgp::learn_autocrypt(header, from);
    }
}

/// What a draft can be, for the writing window's switches.
#[derive(serde::Serialize, Default)]
pub(crate) struct DraftProtection {
    pub can_sign: bool,
    pub can_encrypt: bool,
    /// Recipients without a known key: "Encrypt" waits for them.
    pub missing: Vec<String>,
}

pub(crate) fn draft_protection(draft: &Draft) -> DraftProtection {
    let config = load_config();
    let Some(address) = config.account(&draft.account).and_then(|a| a.address.clone()) else { return DraftProtection::default() };
    let keys = Keys::load();
    let missing: Vec<String> = draft.recipients().into_iter().filter(|r| keys.for_address(r).is_none()).collect();
    DraftProtection { can_sign: keys.own_for(&address).is_some(), can_encrypt: !draft.recipients().is_empty() && missing.is_empty(), missing }
}

/// Answering an encrypted message: the copy kept with the draft is the
/// decrypted one (in your drafts folder, as your own text is), and the answer
/// is encrypted when every key is known.
pub(crate) fn protect_answer(draft: &mut Draft) {
    let Some(original) = draft.original.clone() else { return };
    let Ok(raw) = std::fs::read(&original) else { return };
    let Some((opened, view)) = open(&raw) else { return };
    if let Some(bytes) = opened {
        let _ = std::fs::write(&original, bytes);
    }
    if view.encrypted {
        draft.encrypt = draft_protection(draft).can_encrypt;
    }
}

#[derive(serde::Serialize)]
struct KeysView {
    keys: Vec<pgp::KeySummary>,
    /// Your mail addresses without a key of yours: "Make a key" is offered for each.
    without_key: Vec<String>,
}

/// Your keys and others', and the addresses that could have one.
pub(crate) fn keys_view() -> String {
    let keys = Keys::load();
    let config = load_config();
    let without_key: Vec<String> = config.accounts.iter().filter(|a| a.syncs()).filter_map(|a| a.address.clone()).filter(|a| keys.own_for(a).is_none()).collect();
    json(&KeysView { keys: pgp::summaries(), without_key })
}

/// Makes a key for one of your addresses; its passphrase, random, goes to the keyring.
pub(crate) fn generate(address: &str) -> Result<String, String> {
    let config = load_config();
    let name = config.accounts.iter().find(|a| a.address.as_deref() == Some(address)).and_then(|a| a.name.clone()).unwrap_or_default();
    let passphrase = match secret::test_passphrase() {
        Some(fixed) => fixed,
        None => pgp::new_passphrase()?,
    };
    let made = pgp::generate(&name, address, &passphrase)?;
    secret::save_pgp_passphrase(&made.fingerprint, &passphrase).map_err(|e| e.sentence(tr(), address))?;
    Ok(say("pgp-made", &[("address", address.to_string())]))
}

/// Imports the keys of a file (a `file://` address); a secret key's passphrase
/// is checked, then kept in the keyring.
pub(crate) fn import(url: &str, passphrase: &str) -> Result<String, String> {
    let path = crate::backend::local_path(url);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    // Your keys before: one imported again with a mistyped passphrase stays yours.
    let had: Vec<String> = pgp::summaries().into_iter().filter(|k| k.own).map(|k| k.fingerprint).collect();
    let imported = pgp::import(&bytes)?;
    for key in imported.iter().filter(|k| k.own) {
        if !pgp::opens(&key.fingerprint, passphrase) {
            if !had.contains(&key.fingerprint) {
                let _ = pgp::remove(&key.fingerprint);
            }
            return Err(tr().text("pgp-wrong-passphrase", None));
        }
        secret::save_pgp_passphrase(&key.fingerprint, passphrase).map_err(|e| e.sentence(tr(), &key.fingerprint))?;
    }
    let mut args = tr().counted(imported.len());
    args.set("n", imported.len());
    Ok(tr().text("pgp-imported", Some(&args)))
}

/// Your public key into the downloads folder, to give to others.
pub(crate) fn export(fingerprint: &str, folder: &Path) -> Result<PathBuf, String> {
    let armored = pgp::export_public(fingerprint).ok_or_else(|| tr().text("pgp-no-key", None))?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let short: String = fingerprint.chars().rev().take(16).collect::<Vec<_>>().into_iter().rev().collect();
    let path = folder.join(format!("{short}.asc"));
    std::fs::write(&path, armored).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Looks up the keys a draft's recipients miss; returns what the status line says.
pub(crate) fn lookup(draft: &Draft) -> String {
    let missing = draft_protection(draft).missing;
    let mut found = 0;
    for address in &missing {
        match keys::lookup(address) {
            Ok(keys) if !keys.is_empty() => found += 1,
            Ok(_) => {}
            Err(SyncError::Network(e)) => return say("pgp-lookup-failed", &[("detail", e)]),
            Err(e) => return e.sentence(tr(), address),
        }
    }
    let still = missing.len() - found;
    if still == 0 { tr().text("pgp-lookup-all", None) } else { say("pgp-lookup-some", &[("missing", draft_protection(draft).missing.join(", "))]) }
}

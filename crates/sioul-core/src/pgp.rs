// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! OpenPGP (RFC 9580) for mail, with Sequoia and its pure-Rust cryptography.
//!
//! - **Your keys** live in `~/.local/share/sioul/pgp/own/`. A key made here
//!   keeps its revocation certificate beside it, `<fingerprint>.rev`, yours
//!   alone as the key is ([`revocation_path`]); revoking is not built. A key's private
//!   part is in one of two places ([`Place`]): here, encrypted with a
//!   passphrase the system keyring keeps (`sioul-sync` asks it), so nothing is
//!   asked at each message; or on a security key, which signs and opens itself
//!   after its PIN ([`crate::securitykey`]), Sioul keeping only its public
//!   certificate. Keys come from an import (a file, GnuPG's export), are made
//!   here, or are a security key's.
//! - **Others' keys** live in `…/pgp/others/`: imported, found in their
//!   messages (Autocrypt), or looked up on request.
//! - **Signing and opening** take Sequoia's own `Signer` and `Decryptor`
//!   traits: a key here or a security key, the same way. A message a security
//!   key opened keeps its session key in memory ([`SessionKeys`]), so that it
//!   opens again without the key until Sioul closes.
//! - **What this module does not do**: talk to the network, or to the
//!   keyring. Passphrases and PINs come in as arguments.

use crate::config::data_dir;
use crate::securitykey::{self, CardAccess, CardDecryptor, KnownCard};
use openpgp::cert::prelude::*;
use openpgp::crypto::{self, Password, SessionKey, mpi};
use openpgp::packet::{PKESK, SKESK, key};
use openpgp::parse::Parse;
use openpgp::parse::stream::{DecryptionHelper, DecryptorBuilder, DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper};
use openpgp::policy::StandardPolicy;
use openpgp::serialize::stream::{Armorer, Encryptor, LiteralWriter, Message, Signer};
use openpgp::serialize::{Serialize, SerializeInto};
use openpgp::types::{HashAlgorithm, SymmetricAlgorithm};
use openpgp::{Cert, KeyHandle};
/// A key's fingerprint, as the crates that do not use Sequoia themselves name it.
pub use openpgp::Fingerprint;
use sequoia_openpgp as openpgp;
use serde::Serialize as Serde;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

fn policy() -> StandardPolicy<'static> {
    StandardPolicy::new()
}

fn pgp_dir() -> PathBuf {
    data_dir().join("pgp")
}

fn own_dir() -> PathBuf {
    pgp_dir().join("own")
}

fn others_dir() -> PathBuf {
    pgp_dir().join("others")
}

/// The keys Sioul knows: yours (their secret parts here, or on a security
/// key), others', and your security keys.
#[derive(Default, Clone)]
pub struct Keys {
    pub own: Vec<Cert>,
    pub others: Vec<Cert>,
    /// Your security keys, as `pgp/cards.toml` lists them.
    pub cards: Vec<KnownCard>,
}

/// Where the private part of one of your keys is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Here: its secret part in `pgp/own`, encrypted with a passphrase the system keyring keeps.
    Here,
    /// On a security key, by its identifier ("0006:12345678").
    Card {
        /// The security key, as [`securitykey::CardInfo::ident`].
        ident: String,
    },
}

/// Your key for an address: its certificate, and where its private part is.
#[derive(Debug, Clone)]
pub struct Own<'a> {
    pub cert: &'a Cert,
    pub place: Place,
}

fn read_certs(dir: &std::path::Path) -> Vec<Cert> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "asc" || x == "pgp"))
        .filter_map(|p| std::fs::read(&p).ok())
        .flat_map(|bytes| CertParser::from_bytes(&bytes).map(|parser| parser.filter_map(Result::ok).collect::<Vec<_>>()).unwrap_or_default())
        .collect()
}

/// Whether a key holds `address` among its user IDs.
fn names(cert: &Cert, address: &str) -> bool {
    cert.userids().any(|u| u.userid().email().ok().flatten().is_some_and(|e| e.eq_ignore_ascii_case(address.trim())))
}

impl Keys {
    pub fn load() -> Keys {
        Keys::load_at(&pgp_dir())
    }

    /// The keys of the folder `root` (`pgp/`): yours, others', your security
    /// keys (`cards.toml`), and those your other devices made known
    /// (`cards/`, [`securitykey::shared_cards`]): a security key read on your
    /// computer, its certificate with it, so that a phone encrypts to you,
    /// hands your key by Autocrypt and says what waits for the key. A key
    /// this device read itself says what it is; its certificate is merged
    /// with the shared one (a renewal made on another computer).
    pub fn load_at(root: &std::path::Path) -> Keys {
        let mut keys = Keys { own: read_certs(&root.join("own")), others: read_certs(&root.join("others")), cards: securitykey::known_cards_in(&root.join("cards.toml")) };
        for (card, cert) in securitykey::shared_cards_in(&root.join(securitykey::SHARED_CARDS)) {
            if !keys.cards.iter().any(|known| known.ident.eq_ignore_ascii_case(&card.ident)) {
                keys.cards.push(card);
            }
            match keys.own.iter_mut().find(|own| own.fingerprint() == cert.fingerprint()) {
                Some(own) => {
                    if let Ok(merged) = own.clone().merge_public(cert) {
                        *own = merged;
                    }
                }
                None => keys.own.push(cert),
            }
        }
        keys
    }

    /// Your key for an address, able to sign now. A security key that serves
    /// the address comes first, and alone: a key kept here for the same
    /// address only opens older mail. When that security key's certificate
    /// expired, nothing signs for the address until it is renewed.
    pub fn own_for(&self, address: &str) -> Option<Own<'_>> {
        if let Some((card, cert)) = self.card_for(address) {
            return can_sign_with(cert, &card.sign).then(|| Own { cert, place: Place::Card { ident: card.ident.clone() } });
        }
        let policy = policy();
        self.own
            .iter()
            .find(|c| names(c, address) && c.keys().secret().with_policy(&policy, None).alive().revoked(false).for_signing().next().is_some())
            .map(|cert| Own { cert, place: Place::Here })
    }

    /// The security key that signs for `address`, and its certificate, even
    /// when it expired: to say so.
    pub fn card_for(&self, address: &str) -> Option<(&KnownCard, &Cert)> {
        self.cards.iter().filter(|card| card.serves(address)).find_map(|card| Some((card, self.cert(&card.cert)?)))
    }

    /// Your certificate of this fingerprint.
    pub fn cert(&self, fingerprint: &str) -> Option<&Cert> {
        self.own.iter().find(|c| c.fingerprint().to_hex().eq_ignore_ascii_case(fingerprint))
    }

    /// A key to encrypt to `address`: someone's, or yours. For one of your
    /// own addresses, your own key alone: never a certificate someone sent
    /// claiming it (an Autocrypt header, a lookup), which would read what you
    /// send yourself; none when yours cannot take encryption now.
    pub fn for_address(&self, address: &str) -> Option<&Cert> {
        let policy = policy();
        let encrypts = |c: &&Cert| c.keys().with_policy(&policy, None).alive().revoked(false).for_transport_encryption().next().is_some();
        if self.is_own_address(address) {
            return self.self_cert_for(address);
        }
        self.others.iter().chain(&self.own).filter(|c| names(c, address)).find(encrypts)
    }

    /// Your own certificate to encrypt to, for `address`: your security key's
    /// when one serves the address (its certificate as your computer read
    /// it), else a key of yours naming it; none when it cannot take
    /// encryption now (expired, revoked).
    pub fn self_cert_for(&self, address: &str) -> Option<&Cert> {
        let policy = policy();
        let encrypts = |c: &&Cert| c.keys().with_policy(&policy, None).alive().revoked(false).for_transport_encryption().next().is_some();
        if let Some((_, cert)) = self.card_for(address) {
            return Some(cert).filter(encrypts);
        }
        self.own.iter().filter(|c| names(c, address)).find(encrypts)
    }

    /// Whether `address` is yours by your keys: a security key serves it, or
    /// a key of yours names it.
    pub fn is_own_address(&self, address: &str) -> bool {
        self.cards.iter().any(|card| card.serves(address)) || self.own.iter().any(|c| names(c, address))
    }

    fn all(&self) -> impl Iterator<Item = &Cert> {
        self.own.iter().chain(&self.others)
    }

    fn by_handle(&self, handle: &KeyHandle) -> Option<&Cert> {
        self.all().find(|c| c.keys().any(|k| k.key().key_handle().aliases(handle)))
    }
}

/// Whether the key `fingerprint` of `cert` can sign now: valid, alive, not
/// revoked, allowed to sign.
fn can_sign_with(cert: &Cert, fingerprint: &str) -> bool {
    let policy = policy();
    cert.keys().with_policy(&policy, None).alive().revoked(false).for_signing().any(|k| k.key().fingerprint().to_hex().eq_ignore_ascii_case(fingerprint))
}

/// The key `fingerprint` of `cert`, its public part, primary or subkey.
pub fn key_of(cert: &Cert, fingerprint: &str) -> Option<securitykey::PublicKey> {
    cert.keys().find(|k| k.key().fingerprint().to_hex().eq_ignore_ascii_case(fingerprint)).map(|k| k.key().clone())
}

/// A key as the window lists it.
#[derive(Debug, Clone, Serde, PartialEq, Eq)]
pub struct KeySummary {
    pub fingerprint: String,
    /// "Jane Exemple <jane@example.org>", one per user ID.
    pub names: Vec<String>,
    pub own: bool,
    /// "2026-10-03".
    pub created: String,
    /// "2029-10-03", or empty when it does not expire.
    pub expires: String,
    pub can_encrypt: bool,
    /// The security key holding its private keys ("0006:12345678"); empty when they are here.
    pub card: String,
    /// Yours, made here: its revocation certificate is kept beside it (`revocation_path`).
    pub revocation: bool,
}

fn summary(cert: &Cert, own: bool) -> KeySummary {
    let policy = policy();
    let valid = cert.with_policy(&policy, None).ok();
    let date = |t: std::time::SystemTime| jiff::Timestamp::try_from(t).map(|t| t.strftime("%Y-%m-%d").to_string()).unwrap_or_default();
    KeySummary {
        fingerprint: cert.fingerprint().to_hex(),
        names: cert.userids().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).collect(),
        own,
        created: date(cert.primary_key().key().creation_time()),
        expires: valid.as_ref().and_then(|v| v.primary_key().key_expiration_time()).map(date).unwrap_or_default(),
        can_encrypt: cert.keys().with_policy(&policy, None).alive().revoked(false).for_transport_encryption().next().is_some(),
        card: String::new(),
        revocation: false,
    }
}

/// Every key, yours first; a security key's certificate names its key.
pub fn summaries() -> Vec<KeySummary> {
    let keys = Keys::load();
    let mut all: Vec<KeySummary> = keys.own.iter().map(|c| summary(c, true)).chain(keys.others.iter().map(|c| summary(c, false))).collect();
    for key in all.iter_mut().filter(|k| k.own) {
        if let Some(card) = keys.cards.iter().find(|c| c.cert.eq_ignore_ascii_case(&key.fingerprint)) {
            key.card = card.ident.clone();
        }
        key.revocation = revocation_path(&key.fingerprint).is_file();
    }
    all
}

/// Keeps a key, merged with what was known of it. Secret parts go with
/// yours, and so does the certificate of a security key of yours, whatever
/// brings it again (an Autocrypt header, a lookup).
fn store(cert: Cert) -> Result<KeySummary, String> {
    store_at(&pgp_dir(), cert)
}

/// [`store`] into the folder `root` (`pgp/`).
fn store_at(root: &std::path::Path, cert: Cert) -> Result<KeySummary, String> {
    let known_as_own = root.join("own").join(format!("{}.asc", cert.fingerprint().to_hex())).exists();
    let own = cert.is_tsk() || known_as_own;
    store_in(root, cert, own)
}

fn store_in(root: &std::path::Path, cert: Cert, own: bool) -> Result<KeySummary, String> {
    let dir = root.join(if own { "own" } else { "others" });
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.asc", cert.fingerprint().to_hex()));
    let merged = match std::fs::read(&path).ok().and_then(|bytes| Cert::from_bytes(&bytes).ok()) {
        Some(known) => known.merge_public_and_secret(cert).map_err(|e| e.to_string())?,
        None => cert,
    };
    let armored = if merged.is_tsk() { merged.as_tsk().armored().to_vec() } else { merged.armored().to_vec() }.map_err(|e| e.to_string())?;
    write_key(&path, &armored)?;
    // A security key's certificate kept again (renewed, a subkey added): your other devices follow.
    let fingerprint = merged.fingerprint().to_hex();
    if own && securitykey::known_cards_in(&root.join("cards.toml")).iter().any(|card| card.cert.eq_ignore_ascii_case(&fingerprint)) {
        securitykey::share_cards_in(&root.join("cards.toml"), &dir, &root.join(securitykey::SHARED_CARDS), false)?;
    }
    Ok(summary(&merged, own))
}

/// Keeps the certificate of your security key with your keys: public parts only.
pub fn keep_card_certificate(cert: &Cert) -> Result<KeySummary, String> {
    store_in(&pgp_dir(), cert.clone().strip_secret_key_material(), true)
}

/// Writes a key file beside, then moves it in place. On Unix it is readable by
/// you alone, whatever the umask: your secret keys, and whose keys you keep.
fn write_key(path: &std::path::Path, armored: &[u8]) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let temporary = path.with_extension("tmp");
    // A file left by an interrupted write keeps its permissions when truncated: it goes first.
    let _ = std::fs::remove_file(&temporary);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(fail)?;
    file.write_all(armored).and_then(|()| file.sync_all()).map_err(fail)?;
    drop(file);
    std::fs::rename(&temporary, path).map_err(fail)
}

/// The certificates of `bytes` (armored or binary), read, not kept.
pub fn certificates_in(bytes: &[u8]) -> Vec<Cert> {
    certificates_of(bytes).unwrap_or_default()
}

/// The certificates among `bytes` (armored or binary, one or several), as a
/// certificate from outside is read everywhere here (a file, GnuPG, a
/// server, a message): read as they came, and again with their signatures'
/// numbers written shortest (`minimal_mpis`), merged by fingerprint, in the
/// order they came; so the repair only ever adds, never takes a certificate
/// or a part away. Sequoia's words when neither reading reads.
pub fn certificates_of(bytes: &[u8]) -> Result<Vec<Cert>, String> {
    let read = |bytes: &[u8]| CertParser::from_bytes(bytes).map(|parser| parser.filter_map(Result::ok).collect::<Vec<Cert>>()).map_err(|e| e.to_string());
    let as_they_came = read(bytes);
    let shortest = match minimal_mpis(bytes) {
        std::borrow::Cow::Borrowed(_) => return as_they_came,
        std::borrow::Cow::Owned(repaired) => read(&repaired),
    };
    let (mut certs, shortest) = match (as_they_came, shortest) {
        (Ok(certs), Ok(shortest)) => (certs, shortest),
        (Ok(certs), Err(_)) => return Ok(certs),
        (Err(_), shortest) => return shortest,
    };
    for cert in shortest {
        match certs.iter().position(|kept| kept.fingerprint() == cert.fingerprint()) {
            Some(index) => {
                // The same key read both ways: every component either reading found.
                if let Ok(merged) = certs[index].clone().merge_public_and_secret(cert) {
                    certs[index] = merged;
                }
            }
            None => certs.push(cert),
        }
    }
    Ok(certs)
}

/// `bytes` (armored or binary) with the numbers of each signature written in
/// their shortest form, as RFC 9580 (3.2) asks and Sequoia insists
/// (`certificates_of` reads with it). GnuPG 2.4.4, Ubuntu 24.04's, writes
/// about one key signature in 128 with a number one octet too long (a
/// leading zero octet counted in its length); Sequoia then read that
/// signature as none, and the subkey it binds, or the whole key, as missing.
/// A signature's numbers are not part of what it signs: written shorter they
/// are the same numbers, and Sequoia still checks the signature. Only ever
/// shorter: a length said too short is left as it came. Armor is read block
/// by block, every key block kept, in order. The bytes as they came when none
/// needed it, or when they do not read as packets of known length; armored
/// bytes come back binary once changed, which every reader here takes.
pub fn minimal_mpis(bytes: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    use std::borrow::Cow;
    // A binary packet starts with its tag's high bit set: the packets as they come.
    if bytes.first().is_some_and(|b| b & 0x80 != 0) {
        return shortened(bytes).map_or(Cow::Borrowed(bytes), Cow::Owned);
    }
    // Armor: each key block on its own, dearmored, then shortened where it
    // needs it; every one kept, as Sequoia reads them all.
    let mut out = Vec::new();
    let mut changed = false;
    for block in armored_keys(bytes) {
        let mut reader = openpgp::armor::Reader::from_bytes(block, openpgp::armor::ReaderMode::Tolerant(None));
        let mut binary = Vec::new();
        if std::io::Read::read_to_end(&mut reader, &mut binary).is_err() || binary.is_empty() {
            return Cow::Borrowed(bytes);
        }
        match shortened(&binary) {
            Some(shorter) => {
                out.extend_from_slice(&shorter);
                changed = true;
            }
            None => out.extend_from_slice(&binary),
        }
    }
    if changed { Cow::Owned(out) } else { Cow::Borrowed(bytes) }
}

/// The armored key blocks of `text`, in order: each from its "-----BEGIN PGP
/// PUBLIC KEY BLOCK-----" (or PRIVATE) to the end of its "-----END PGP" line.
/// What lies around or between them is left out, a message's or a
/// signature's block among it.
fn armored_keys(text: &[u8]) -> Vec<&[u8]> {
    const BEGIN: &[u8] = b"-----BEGIN PGP ";
    const END: &[u8] = b"-----END PGP ";
    let find = |from: usize, needle: &[u8]| text.get(from..).and_then(|rest| rest.windows(needle.len()).position(|window| window == needle)).map(|i| from + i);
    let mut blocks = Vec::new();
    let mut at = 0;
    while let Some(begin) = find(at, BEGIN) {
        let Some(end) = find(begin + BEGIN.len(), END) else { break };
        let line_end = text[end..].iter().position(|b| *b == b'\n').map_or(text.len(), |i| end + i + 1);
        let kind = &text[begin + BEGIN.len()..];
        if kind.starts_with(b"PUBLIC KEY BLOCK-----") || kind.starts_with(b"PRIVATE KEY BLOCK-----") {
            blocks.push(&text[begin..line_end]);
        }
        at = line_end;
    }
    blocks
}

/// The packets of `binary` with each version 4 signature's numbers in their
/// shortest form; none when none needed it, or when `binary` does not read
/// as packets of known length (a partial length is data's, never a key's).
fn shortened(binary: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(binary.len());
    let mut changed = false;
    let mut at = 0;
    while at < binary.len() {
        let (tag, body) = packet_at(binary, at)?;
        let shorter = if tag == 2 && binary[body.clone()].first() == Some(&4) { signature_shortened(&binary[body.clone()]) } else { None };
        match shorter {
            // A signature, version 4 (RFC 9580, 5.2.3), whose numbers were written longer.
            Some(shorter) => {
                put_packet(&mut out, tag, &shorter);
                changed = true;
            }
            None => out.extend_from_slice(&binary[at..body.end]),
        }
        at = body.end;
    }
    changed.then_some(out)
}

/// The packet at `at` (RFC 9580, 4.2): its tag and where its body lies; none
/// for one cut short, or of partial length.
fn packet_at(bytes: &[u8], at: usize) -> Option<(u8, std::ops::Range<usize>)> {
    let ctb = *bytes.get(at)?;
    let number = |from: usize, octets: usize| bytes.get(from..from + octets).map(|b| b.iter().fold(0usize, |n, b| (n << 8) | usize::from(*b)));
    let (tag, start, length) = match ctb {
        // OpenPGP's format: the tag in six bits, then one, two or five octets of length.
        0xC0..=0xFF => match *bytes.get(at + 1)? {
            first @ 0..=191 => (ctb & 0x3F, at + 2, usize::from(first)),
            first @ 192..=223 => (ctb & 0x3F, at + 3, ((usize::from(first) - 192) << 8) + usize::from(*bytes.get(at + 2)?) + 192),
            255 => (ctb & 0x3F, at + 6, number(at + 2, 4)?),
            _ => return None,
        },
        // The legacy format: the tag in four bits, the length's size in two.
        0x80..=0xBF => match ctb & 3 {
            0 => ((ctb >> 2) & 0x0F, at + 2, number(at + 1, 1)?),
            1 => ((ctb >> 2) & 0x0F, at + 3, number(at + 1, 2)?),
            2 => ((ctb >> 2) & 0x0F, at + 5, number(at + 1, 4)?),
            // Up to the end.
            _ => ((ctb >> 2) & 0x0F, at + 1, bytes.len().checked_sub(at + 1)?),
        },
        _ => return None,
    };
    let end = start.checked_add(length)?;
    (end <= bytes.len()).then_some((tag, start..end))
}

/// A version 4 signature's body with its numbers in their shortest form (no
/// leading zero bits, RFC 9580, 3.2), and those of a signature embedded in
/// its unhashed area (a signing subkey's back-signature, where GnuPG writes
/// it); never anything in its hashed area, which the signature covers. None
/// when nothing needed it, when its algorithm's are not numbers (Ed25519's own
/// octets) or do not read whole, or when a length is said too short: only
/// ever shorter, as GnuPG's fault is.
fn signature_shortened(body: &[u8]) -> Option<Vec<u8>> {
    shortened_at(body, true)
}

/// `signature_shortened`, the signatures embedded in the unhashed area
/// looked into only when `descend`: one level, never more. An embedded
/// signature's own embedded ones stay as they came: Sequoia refuses a
/// signature embedded in an embedded one, and each level looked into would
/// take the stack of the thread reading a file from outside (a phone's
/// import runs on Qt's own, about 1 MiB), a crafted one nesting thousands.
fn shortened_at(body: &[u8], descend: bool) -> Option<Vec<u8>> {
    let numbers = match *body.get(2)? {
        // RSA.
        1 | 3 => 1,
        // ElGamal, DSA, ECDSA, EdDSA (the legacy one GnuPG makes).
        16 | 17 | 19 | 20 | 22 => 2,
        _ => return None,
    };
    let length_at = |at: usize| body.get(at..at + 2).map(|two| usize::from(u16::from_be_bytes([two[0], two[1]])));
    // Version, type, the key's and the hash's algorithms, then the hashed
    // area after its length: as it came.
    let unhashed = 4usize.checked_add(2 + length_at(4)?)?;
    let mut out = body.get(..unhashed)?.to_vec();
    // The unhashed area, its embedded signatures shortened (one level).
    let area = body.get(unhashed + 2..unhashed + 2 + length_at(unhashed)?)?;
    let (area_out, mut changed) = match descend.then(|| embedded_shortened(area)).flatten() {
        Some(shorter) => (shorter, true),
        None => (area.to_vec(), false),
    };
    out.extend_from_slice(&u16::try_from(area_out.len()).ok()?.to_be_bytes());
    out.extend_from_slice(&area_out);
    // The hash's first two octets, then the numbers.
    let mut at = unhashed + 2 + area.len();
    out.extend_from_slice(body.get(at..at + 2)?);
    at += 2;
    for _ in 0..numbers {
        let bits = length_at(at)?;
        let value = body.get(at + 2..at + 2 + bits.div_ceil(8))?;
        at += 2 + value.len();
        let value = &value[value.iter().position(|b| *b != 0).unwrap_or(value.len())..];
        let shortest = value.first().map_or(0, |first| value.len() * 8 - first.leading_zeros() as usize);
        if shortest > bits {
            return None;
        }
        changed |= shortest < bits;
        out.extend_from_slice(&u16::try_from(shortest).ok()?.to_be_bytes());
        out.extend_from_slice(value);
    }
    (changed && at == body.len()).then_some(out)
}

/// A signature's unhashed area (RFC 9580, 5.2.3.7) with each embedded
/// signature (subpacket 32) shortened, its own unhashed area left as it came
/// (`shortened_at`), its length said again; none when none needed it, or
/// when the area does not read as subpackets.
fn embedded_shortened(area: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(area.len());
    let mut changed = false;
    let mut at = 0;
    while at < area.len() {
        let (start, length) = match *area.get(at)? {
            first @ 0..=191 => (at + 1, usize::from(first)),
            first @ 192..=254 => (at + 2, ((usize::from(first) - 192) << 8) + usize::from(*area.get(at + 1)?) + 192),
            255 => (at + 5, usize::try_from(u32::from_be_bytes(area.get(at + 1..at + 5)?.try_into().ok()?)).ok()?),
        };
        // Its type (the critical bit aside), then its body.
        let subpacket = area.get(start..start.checked_add(length)?)?;
        let (&kind, inner) = subpacket.split_first()?;
        match (kind & 0x7F == 32 && inner.first() == Some(&4)).then(|| shortened_at(inner, false)).flatten() {
            Some(shorter) => {
                put_subpacket_length(&mut out, 1 + shorter.len());
                out.push(kind);
                out.extend_from_slice(&shorter);
                changed = true;
            }
            None => out.extend_from_slice(&area[at..start + length]),
        }
        at = start + length;
    }
    changed.then_some(out)
}

/// A subpacket's length (RFC 9580, 5.2.3.7), in one, two or five octets.
fn put_subpacket_length(out: &mut Vec<u8>, length: usize) {
    match length {
        0..=191 => out.push(length as u8),
        192..=16_319 => out.extend_from_slice(&[((length - 192) >> 8) as u8 + 192, (length - 192) as u8]),
        _ => {
            out.push(255);
            out.extend_from_slice(&(length as u32).to_be_bytes());
        }
    }
}

/// A packet in OpenPGP's format: its tag, its length, its body.
fn put_packet(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(0xC0 | tag);
    match body.len() {
        length @ 0..=191 => out.push(length as u8),
        length @ 192..=8383 => out.extend_from_slice(&[((length - 192) >> 8) as u8 + 192, (length - 192) as u8]),
        length => {
            out.push(255);
            out.extend_from_slice(&(length as u32).to_be_bytes());
        }
    }
    out.extend_from_slice(body);
}

/// Imports the keys of a file: armored or binary, public or secret.
pub fn import(bytes: &[u8]) -> Result<Vec<KeySummary>, String> {
    let certs = certificates_of(bytes)?;
    if certs.is_empty() {
        return Err("no OpenPGP key".into());
    }
    certs.into_iter().map(store).collect()
}

/// Whether `passphrase` opens the secret parts of your key `fingerprint`.
pub fn opens(fingerprint: &str, passphrase: &str) -> bool {
    let keys = Keys::load();
    let Some(cert) = keys.own.iter().find(|c| c.fingerprint().to_hex() == fingerprint) else { return false };
    let policy = policy();
    let password = Password::from(passphrase);
    cert.keys().secret().with_policy(&policy, None).for_signing().next().is_some_and(|k| k.key().clone().decrypt_secret(&password).is_ok())
}

/// A long random passphrase for a key made here: nobody types it, the keyring keeps it.
pub fn new_passphrase() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    openpgp::crypto::random(&mut bytes).map_err(|e| e.to_string())?;
    Ok(crate::lines::base64_encode(&bytes))
}

/// Makes a key for you: Curve25519, for signing and encrypting, valid three
/// years; its secret parts encrypted with `passphrase`. Its revocation
/// certificate, which Sequoia makes with it, is kept beside it
/// (`revocation_path`): the only way to revoke the key once it is lost.
pub fn generate(name: &str, address: &str, passphrase: &str) -> Result<KeySummary, String> {
    generate_at(&pgp_dir(), name, address, passphrase)
}

/// [`generate`] into the folder `root` (`pgp/`).
fn generate_at(root: &std::path::Path, name: &str, address: &str, passphrase: &str) -> Result<KeySummary, String> {
    let userid = if name.trim().is_empty() { format!("<{}>", address.trim()) } else { format!("{} <{}>", name.trim(), address.trim()) };
    let (cert, revocation) = CertBuilder::general_purpose(Some(userid))
        .set_cipher_suite(CipherSuite::Cv25519)
        .set_validity_period(std::time::Duration::from_secs(3 * 365 * 24 * 3600))
        .set_password(Some(Password::from(passphrase)))
        .generate()
        .map_err(|e| e.to_string())?;
    // The certificate first: a key is never kept without it.
    let own = root.join("own");
    std::fs::create_dir_all(&own).map_err(|e| e.to_string())?;
    let kept = own.join(format!("{}.rev", cert.fingerprint().to_hex()));
    write_key(&kept, revocation_text(&cert, revocation)?.as_bytes())?;
    match store_at(root, cert) {
        Ok(summary) => Ok(KeySummary { revocation: true, ..summary }),
        Err(e) => {
            let _ = std::fs::remove_file(&kept);
            Err(e)
        }
    }
}

/// Where the revocation certificate of your key made here is kept: beside
/// the key, `pgp/own/<fingerprint>.rev`, readable by you alone (`write_key`),
/// as GnuPG keeps its own in `openpgp-revocs.d`.
pub fn revocation_path(fingerprint: &str) -> PathBuf {
    own_dir().join(format!("{fingerprint}.rev"))
}

/// A revocation certificate as a file to keep: what it is, in a few lines,
/// then the signature armored, a colon before its first line so that it
/// cannot be imported by mistake (as GnuPG writes its own).
fn revocation_text(cert: &Cert, revocation: openpgp::packet::Signature) -> Result<String, String> {
    let mut armored = Vec::new();
    {
        let mut writer = openpgp::armor::Writer::with_headers(&mut armored, openpgp::armor::Kind::PublicKey, [("Comment", "This is a revocation certificate")]).map_err(|e| e.to_string())?;
        openpgp::Packet::from(revocation).serialize(&mut writer).map_err(|e| e.to_string())?;
        writer.finalize().map_err(|e| e.to_string())?;
    }
    let names: Vec<String> = cert.userids().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).collect();
    Ok(format!(
        "This is the revocation certificate of the OpenPGP key\n{}\n{}\n\n\
         Published, it tells everyone that this key must no longer be used, for\n\
         good: it cannot be taken back. Keep a copy where only you can reach it,\n\
         apart from the key, for the day the key is lost or stolen.\n\n\
         A colon stands before the five dashes below, so that the certificate\n\
         cannot be imported by mistake: take it away with a text editor before\n\
         you import the certificate and publish it.\n\n:{}",
        cert.fingerprint().to_hex(),
        names.join("\n"),
        String::from_utf8_lossy(&armored)
    ))
}

/// Copies the revocation certificate of your key `fingerprint` into
/// `folder`, as `<fingerprint>.rev` (a number added when that name is
/// taken), yours alone; none kept for that key, an error.
pub fn save_revocation(fingerprint: &str, folder: &std::path::Path) -> Result<PathBuf, String> {
    save_revocation_at(&pgp_dir(), fingerprint, folder)
}

/// [`save_revocation`] from the folder `root` (`pgp/`).
fn save_revocation_at(root: &std::path::Path, fingerprint: &str, folder: &std::path::Path) -> Result<PathBuf, String> {
    let text = std::fs::read(root.join("own").join(format!("{fingerprint}.rev"))).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let path = (1..)
        .map(|n| folder.join(if n == 1 { format!("{fingerprint}.rev") } else { format!("{fingerprint}-{n}.rev") }))
        .find(|p| !p.exists())
        .unwrap_or_else(|| folder.join(format!("{fingerprint}.rev")));
    write_key(&path, &text)?;
    Ok(path)
}

/// A key's public part, armored, to give to others.
pub fn export_public(fingerprint: &str) -> Option<String> {
    let keys = Keys::load();
    let cert = keys.all().find(|c| c.fingerprint().to_hex() == fingerprint)?;
    cert.armored().to_vec().ok().map(|b| String::from_utf8_lossy(&b).to_string())
}

/// Removes a key; yours are kept aside in `pgp/removed`, never deleted, with
/// their revocation certificate.
pub fn remove(fingerprint: &str) -> Result<(), String> {
    for (dir, own) in [(own_dir(), true), (others_dir(), false)] {
        let path = dir.join(format!("{fingerprint}.asc"));
        if !path.exists() {
            continue;
        }
        if own {
            let aside = data_dir().join("pgp").join("removed");
            std::fs::create_dir_all(&aside).map_err(|e| e.to_string())?;
            std::fs::rename(&path, aside.join(format!("{fingerprint}.asc"))).map_err(|e| e.to_string())?;
            let revocation = revocation_path(fingerprint);
            if revocation.exists() {
                std::fs::rename(&revocation, aside.join(format!("{fingerprint}.rev"))).map_err(|e| e.to_string())?;
            }
        } else {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// A signer of yours, as Sequoia takes it: a key here ([`here_signer`]) or
/// on a security key ([`securitykey::CardSigner`]).
pub type Signing<'a> = dyn crypto::Signer + Send + Sync + 'a;

/// Your key here that signs, its secret part opened with `passphrase`: the
/// signer of a key kept on this device.
pub fn here_signer(cert: &Cert, passphrase: &str) -> Result<crypto::KeyPair, String> {
    let policy = policy();
    let key = cert.keys().secret().with_policy(&policy, None).alive().revoked(false).for_signing().next().ok_or("no signing key")?.key().clone();
    let key = if key.has_unencrypted_secret() { key } else { key.decrypt_secret(&Password::from(passphrase)).map_err(|_| "wrong passphrase".to_string())? };
    key.into_keypair().map_err(|e| e.to_string())
}

/// A signer lent for one message: Sequoia's stream takes its signer by value,
/// and a security key's signer borrows what it needs to reach the key.
struct Lent<'r, 'o>(&'r mut Signing<'o>);

impl crypto::Signer for Lent<'_, '_> {
    fn public(&self) -> &openpgp::packet::Key<key::PublicParts, key::UnspecifiedRole> {
        self.0.public()
    }

    fn acceptable_hashes(&self) -> &[HashAlgorithm] {
        self.0.acceptable_hashes()
    }

    fn sign(&mut self, hash_algo: HashAlgorithm, digest: &[u8]) -> openpgp::Result<mpi::Signature> {
        self.0.sign(hash_algo, digest)
    }
}

/// A detached signature of `data`, armored, and the hash it used ("pgp-sha512"),
/// for multipart/signed (RFC 3156 §5). `signer` is your key here
/// ([`here_signer`]) or on a security key ([`securitykey::CardSigner`]).
pub fn sign_detached(data: &[u8], signer: &mut Signing<'_>) -> Result<(String, String), String> {
    let mut out = Vec::new();
    {
        let message = Message::new(&mut out);
        let message = Armorer::new(message).kind(openpgp::armor::Kind::Signature).build().map_err(|e| e.to_string())?;
        let mut signing = Signer::new(message, Lent(signer)).map_err(|e| e.to_string())?.detached().build().map_err(|e| e.to_string())?;
        signing.write_all(data).map_err(|e| e.to_string())?;
        signing.finalize().map_err(|e| e.to_string())?;
    }
    let micalg = openpgp::Packet::from_bytes(&out)
        .ok()
        .and_then(|p| match p {
            openpgp::Packet::Signature(s) => Some(format!("pgp-{}", s.hash_algo().to_string().to_ascii_lowercase())),
            _ => None,
        })
        .unwrap_or_else(|| "pgp-sha512".into());
    Ok((String::from_utf8_lossy(&out).to_string(), micalg))
}

/// `data` encrypted to the recipients' keys, signed by yours when given; armored.
pub fn encrypt(data: &[u8], recipients: &[&Cert], signer: Option<&mut Signing<'_>>) -> Result<String, String> {
    let policy = policy();
    let keys: Vec<_> = recipients.iter().flat_map(|c| c.keys().with_policy(&policy, None).supported().alive().revoked(false).for_transport_encryption()).collect();
    if keys.is_empty() {
        return Err("no encryption key".into());
    }
    let mut out = Vec::new();
    {
        let message = Message::new(&mut out);
        let message = Armorer::new(message).build().map_err(|e| e.to_string())?;
        let message = Encryptor::for_recipients(message, keys).build().map_err(|e| e.to_string())?;
        let message = match signer {
            Some(signer) => Signer::new(message, Lent(signer)).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?,
            None => message,
        };
        let mut literal = LiteralWriter::new(message).build().map_err(|e| e.to_string())?;
        literal.write_all(data).map_err(|e| e.to_string())?;
        literal.finalize().map_err(|e| e.to_string())?;
    }
    Ok(String::from_utf8_lossy(&out).to_string())
}

/// A signature, checked.
#[derive(Debug, Clone, Serde, PartialEq, Eq)]
pub struct Checked {
    pub good: bool,
    /// The signer's first user ID, when their key is known.
    pub signer: String,
    pub fingerprint: String,
    /// Why it is not good: "unknown key", "bad signature"…
    pub problem: String,
}

/// What opening a message found.
#[derive(Debug, Clone, Default)]
pub struct Opened {
    pub data: Vec<u8>,
    pub signatures: Vec<Checked>,
}

/// The session keys of messages a security key opened, by message: such a
/// message opens again, with its attachments and its answer, without the key,
/// until Sioul closes. In memory only, never on disk.
#[derive(Default)]
pub struct SessionKeys(Mutex<BTreeMap<[u8; 32], (Option<SymmetricAlgorithm>, SessionKey)>>);

impl SessionKeys {
    /// None yet.
    pub const fn new() -> SessionKeys {
        SessionKeys(Mutex::new(BTreeMap::new()))
    }

    /// The session's own: one for each running Sioul.
    pub fn global() -> &'static SessionKeys {
        static GLOBAL: SessionKeys = SessionKeys::new();
        &GLOBAL
    }

    fn get(&self, id: &[u8; 32]) -> Option<(Option<SymmetricAlgorithm>, SessionKey)> {
        self.0.lock().ok()?.get(id).cloned()
    }

    fn keep(&self, id: [u8; 32], algo: Option<SymmetricAlgorithm>, key: SessionKey) {
        if let Ok(mut kept) = self.0.lock() {
            kept.insert(id, (algo, key));
        }
    }

    /// Forgets them all: every message a security key opened needs it again.
    pub fn forget(&self) {
        if let Ok(mut kept) = self.0.lock() {
            kept.clear();
        }
    }
}

/// A message's name among the session keys: SHA-256 of its encrypted bytes.
fn message_id(message: &[u8]) -> Option<[u8; 32]> {
    let mut context = HashAlgorithm::SHA256.context().ok()?.for_digest();
    context.update(message);
    context.into_digest().ok()?.try_into().ok()
}

/// How your keys open what is encrypted to you.
pub struct Unlock<'a> {
    /// The passphrase of a key kept here, from the system keyring.
    pub passphrase: &'a dyn Fn(&Fingerprint) -> Option<String>,
    /// The session keys of messages a security key opened before.
    pub sessions: &'a SessionKeys,
    /// Your security key, when you asked to open with it now; without it, no
    /// security key is ever asked anything.
    pub card: Option<&'a CardAccess<'a>>,
    /// The security key a message waits for.
    wanted: RefCell<Option<String>>,
}

impl<'a> Unlock<'a> {
    /// Your keys here, with their passphrases, and the session keys kept.
    pub fn new(passphrase: &'a dyn Fn(&Fingerprint) -> Option<String>, sessions: &'a SessionKeys) -> Unlock<'a> {
        Unlock { passphrase, sessions, card: None, wanted: RefCell::new(None) }
    }

    /// And your security key, asked now.
    pub fn with_card(mut self, card: &'a CardAccess<'a>) -> Unlock<'a> {
        self.card = Some(card);
        self
    }

    /// The security key the last message waits for: it is encrypted to it, and
    /// it was not asked, or it did not open it.
    pub fn wanted(&self) -> Option<String> {
        self.wanted.borrow().clone()
    }
}

struct Helper<'a> {
    keys: &'a Keys,
    unlock: &'a Unlock<'a>,
    /// The message's name for its session key.
    id: Option<[u8; 32]>,
    checked: Vec<Checked>,
}

impl VerificationHelper for Helper<'_> {
    fn get_certs(&mut self, ids: &[KeyHandle]) -> openpgp::Result<Vec<Cert>> {
        Ok(ids.iter().filter_map(|id| self.keys.by_handle(id).cloned()).collect())
    }

    fn check(&mut self, structure: MessageStructure) -> openpgp::Result<()> {
        for layer in structure {
            let MessageLayer::SignatureGroup { results } = layer else { continue };
            for result in results {
                self.checked.push(match result {
                    Ok(good) => Checked {
                        good: true,
                        signer: good.ka.cert().userids().next().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).unwrap_or_default(),
                        fingerprint: good.ka.cert().fingerprint().to_hex(),
                        problem: String::new(),
                    },
                    Err(error) => {
                        use openpgp::parse::stream::VerificationError as E;
                        let (fingerprint, problem) = match &error {
                            E::MissingKey { sig } => (sig.issuer_fingerprints().next().map(|f| f.to_hex()).unwrap_or_default(), "unknown key"),
                            E::BadSignature { .. } => (String::new(), "bad signature"),
                            E::BadKey { .. } => (String::new(), "bad key"),
                            E::MalformedSignature { .. } => (String::new(), "malformed signature"),
                            E::UnboundKey { .. } => (String::new(), "unbound key"),
                            _ => (String::new(), "unknown"),
                        };
                        Checked { good: false, signer: String::new(), fingerprint, problem: problem.to_string() }
                    }
                });
            }
        }
        // Whatever the signatures say, the text is shown with what they say.
        Ok(())
    }
}

impl DecryptionHelper for Helper<'_> {
    fn decrypt(&mut self, pkesks: &[PKESK], _skesks: &[SKESK], sym_algo: Option<SymmetricAlgorithm>, decrypt: &mut dyn FnMut(Option<SymmetricAlgorithm>, &SessionKey) -> bool) -> openpgp::Result<Option<Cert>> {
        // Opened before with a security key: its session key, kept in memory.
        if let Some(id) = &self.id
            && let Some((algo, session)) = self.unlock.sessions.get(id)
            && decrypt(algo, &session)
        {
            return Ok(None);
        }
        let policy = policy();
        for pkesk in pkesks {
            for cert in &self.keys.own {
                let passphrase = (self.unlock.passphrase)(&cert.fingerprint()).unwrap_or_default();
                for key in cert.keys().secret().with_policy(&policy, None).for_transport_encryption().chain(cert.keys().secret().with_policy(&policy, None).for_storage_encryption()) {
                    let recipient_matches = pkesk.recipient().is_none_or(|r| key.key().key_handle().aliases(&r));
                    if !recipient_matches {
                        continue;
                    }
                    let secret = key.key().clone();
                    let secret = if secret.has_unencrypted_secret() { secret } else if let Ok(s) = secret.decrypt_secret(&Password::from(passphrase.as_str())) { s } else { continue };
                    let Ok(mut pair) = secret.into_keypair() else { continue };
                    if let Some((algo, session)) = pkesk.decrypt(&mut pair, sym_algo)
                        && decrypt(algo, &session)
                    {
                        return Ok(Some(cert.clone()));
                    }
                }
            }
        }
        // Keys on a security key: asked only when you asked to open with it;
        // otherwise the message says which key it waits for.
        for pkesk in pkesks {
            let Some(recipient) = pkesk.recipient() else { continue };
            for card in &self.keys.cards {
                let Ok(fingerprint) = card.decrypt.parse::<Fingerprint>() else { continue };
                if !KeyHandle::from(fingerprint).aliases(&recipient) {
                    continue;
                }
                let Some((cert, public)) = self.keys.cert(&card.cert).and_then(|cert| Some((cert, key_of(cert, &card.decrypt)?))) else { continue };
                *self.unlock.wanted.borrow_mut() = Some(card.ident.clone());
                let Some(access) = self.unlock.card else { continue };
                let mut decryptor = CardDecryptor::new(access, &card.ident, public);
                if let Some((algo, session)) = pkesk.decrypt(&mut decryptor, sym_algo)
                    && decrypt(algo, &session)
                {
                    *self.unlock.wanted.borrow_mut() = None;
                    if let Some(id) = self.id {
                        self.unlock.sessions.keep(id, algo, session);
                    }
                    return Ok(Some(cert.clone()));
                }
            }
        }
        Err(openpgp::Error::InvalidOperation("no key of yours opens this message".into()).into())
    }
}

/// The most a decrypted message may hold: anyone can encrypt to your key, and
/// a few kilobytes of compressed data can inflate to gigabytes.
const LARGEST_DECRYPTED: u64 = 256 * 1024 * 1024;

/// Decrypts an OpenPGP message (armored or not) with your keys, checking the
/// signatures inside. `unlock` gives the passphrase of a key of yours here,
/// the session keys kept, and your security key when you asked to use it.
pub fn decrypt(message: &[u8], keys: &Keys, unlock: &Unlock) -> Result<Opened, String> {
    use std::io::Read as _;
    let policy = policy();
    let helper = Helper { keys, unlock, id: message_id(message), checked: Vec::new() };
    let mut decryptor = DecryptorBuilder::from_bytes(message).map_err(|e| e.to_string())?.with_policy(&policy, None, helper).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    (&mut decryptor).take(LARGEST_DECRYPTED + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
    if data.len() as u64 > LARGEST_DECRYPTED {
        return Err(format!("larger than {LARGEST_DECRYPTED} bytes once decrypted"));
    }
    let helper = decryptor.into_helper();
    Ok(Opened { data, signatures: helper.checked })
}

/// Checks a detached signature of `data` (multipart/signed).
pub fn verify_detached(data: &[u8], signature: &[u8], keys: &Keys) -> Vec<Checked> {
    let policy = policy();
    let none = |_: &Fingerprint| None;
    let sessions = SessionKeys::new();
    let unlock = Unlock::new(&none, &sessions);
    let helper = Helper { keys, unlock: &unlock, id: None, checked: Vec::new() };
    let Ok(builder) = DetachedVerifierBuilder::from_bytes(signature) else { return Vec::new() };
    let Ok(mut verifier) = builder.with_policy(&policy, None, helper) else { return Vec::new() };
    let _ = verifier.verify_bytes(data);
    verifier.into_helper().checked
}

/// Where a domain's Web Key Directory keeps the key of `address`: the
/// advanced address, then the direct one (draft-koch-openpgp-webkey-service §3.1).
pub fn wkd_urls(address: &str) -> Option<[String; 2]> {
    let (local, domain) = address.trim().rsplit_once('@')?;
    let domain = domain.to_ascii_lowercase();
    // A name in the DNS, nothing else: an address answered may come from anyone,
    // and "x@192.168.1.1", "x@[::1]" or "x@host:8080/path?" would send the
    // request elsewhere than to a domain's Web Key Directory.
    let labels: Vec<&str> = domain.split('.').collect();
    let named = labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && !l.starts_with('-') && !l.ends_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && labels.last().is_some_and(|tld| tld.chars().any(|c| c.is_ascii_alphabetic()) && *tld != "localhost");
    if !named {
        return None;
    }
    let mut context = openpgp::types::HashAlgorithm::SHA1.context().ok()?.for_digest();
    context.update(local.to_lowercase().as_bytes());
    let digest = context.into_digest().ok()?;
    let hash = zbase32(&digest);
    let query = local.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    Some([
        format!("https://openpgpkey.{domain}/.well-known/openpgpkey/{domain}/hu/{hash}?l={query}"),
        format!("https://{domain}/.well-known/openpgpkey/hu/{hash}?l={query}"),
    ])
}

/// z-base-32 (RFC 6189 §5.1.6), as WKD names keys.
fn zbase32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ybndrfg8ejkmcpqxot1uwisza345h769";
    let mut out = String::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Keeps keys found for `address`: public parts only, and only those naming it.
pub fn learn(bytes: &[u8], address: &str) -> Vec<KeySummary> {
    certificates_of(bytes).unwrap_or_default().into_iter().filter(|c| names(c, address)).filter_map(|c| store(c.strip_secret_key_material()).ok()).collect()
}

/// What a protected message showed when opened, for the reader's line.
#[derive(Debug, Clone, Default, Serde, PartialEq, Eq)]
pub struct PgpView {
    pub encrypted: bool,
    /// Decrypted; false when no key of yours opens it.
    pub opened: bool,
    pub signatures: Vec<Checked>,
    /// Not opened, and encrypted to your security key of this identifier: the
    /// reader offers "Open with your security key".
    pub security_key: Option<String>,
    /// This device cannot use a security key yet (a phone): a message waiting
    /// for yours says to open it on a computer, and offers no button.
    pub key_elsewhere: bool,
}

/// Lone LF become CRLF: what was signed is the canonical form (RFC 3156 §5).
fn canonical(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 32);
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' && (i == 0 || bytes[i - 1] != b'\r') {
            out.push(b'\r');
        }
        out.push(b);
    }
    out
}

fn kind_of(part: &mail_parser::MessagePart) -> (String, String) {
    use mail_parser::MimeHeaders;
    part.content_type().map_or_else(Default::default, |t| {
        (format!("{}/{}", t.ctype(), t.subtype().unwrap_or("")).to_ascii_lowercase(), t.attribute("protocol").unwrap_or("").to_ascii_lowercase())
    })
}

/// The message's own header lines but its MIME ones, which the decrypted body brings.
fn outer_headers(raw: &[u8], body_offset: usize) -> Vec<u8> {
    let head = String::from_utf8_lossy(&raw[..body_offset.min(raw.len())]).to_string();
    let mut out = String::new();
    let mut keep = false;
    for line in head.split_inclusive('\n') {
        if !line.starts_with([' ', '\t']) {
            let name = line.split(':').next().unwrap_or("").trim().to_ascii_lowercase();
            keep = !name.is_empty() && !name.starts_with("content-") && name != "mime-version";
        }
        if keep {
            out.push_str(line);
        }
    }
    canonical(out.trim_end().as_bytes()).into_iter().chain(b"\r\nMIME-Version: 1.0\r\n".iter().copied()).collect()
}

/// Opens a protected message: PGP/MIME encrypted or signed (RFC 3156), or
/// inline PGP. Gives back the message as it reads once decrypted (its own
/// headers, then what was inside), when it was encrypted, and what was found;
/// None when nothing is protected. A message encrypted to your security key
/// opens only when `unlock` carries it, or from a session key kept; otherwise
/// its view names the key it waits for.
pub fn open(raw: &[u8], keys: &Keys, unlock: &Unlock) -> Option<(Option<Vec<u8>>, PgpView)> {
    let message = mail_parser::MessageParser::default().parse(raw)?;
    let root = message.parts.first()?;
    let (kind, protocol) = kind_of(root);
    let children: Vec<&mail_parser::MessagePart> = root.sub_parts().map(|ids| ids.iter().filter_map(|&id| message.parts.get(id as usize)).collect()).unwrap_or_default();
    let closed = || PgpView { encrypted: true, opened: false, signatures: Vec::new(), security_key: unlock.wanted(), key_elsewhere: false };
    if kind == "multipart/encrypted" && protocol == "application/pgp-encrypted" {
        let payload = children.iter().find(|p| kind_of(p).0 == "application/octet-stream").map(|p| p.contents().to_vec())?;
        return Some(match decrypt(&payload, keys, unlock) {
            Ok(opened) => {
                let mut virtual_message = outer_headers(raw, root.raw_body_offset() as usize);
                virtual_message.extend(canonical(&opened.data));
                let mut signatures = opened.signatures;
                // Signed, then encrypted as a multipart/signed (RFC 3156 §6.1): checked too.
                if let Some((_, inner)) = open(&virtual_message, keys, unlock) {
                    signatures.extend(inner.signatures);
                }
                (Some(virtual_message), PgpView { encrypted: true, opened: true, signatures, security_key: None, key_elsewhere: false })
            }
            Err(_) => (None, closed()),
        });
    }
    if kind == "multipart/signed" && protocol == "application/pgp-signature" && children.len() >= 2 {
        let signed = children[0];
        let (start, end) = (signed.raw_header_offset() as usize, signed.raw_end_offset() as usize);
        let data = canonical(raw.get(start..end)?);
        let signature = children.iter().find(|p| kind_of(p).0 == "application/pgp-signature")?.contents().to_vec();
        // The line break before the boundary belongs to the boundary; parsers
        // differ on where a part ends, so the part is checked with and without it.
        let mut checked = verify_detached(&data, &signature, keys);
        if !checked.iter().any(|c| c.good) && data.ends_with(b"\r\n") {
            let trimmed = verify_detached(&data[..data.len() - 2], &signature, keys);
            if trimmed.iter().any(|c| c.good) {
                checked = trimmed;
            }
        }
        return Some((None, PgpView { encrypted: false, opened: true, signatures: checked, security_key: None, key_elsewhere: false }));
    }
    // Inline: an armored message in the text.
    let text = message.body_text(0)?.to_string();
    let start = text.find("-----BEGIN PGP MESSAGE-----")?;
    let end = text[start..].find("-----END PGP MESSAGE-----").map(|e| start + e + "-----END PGP MESSAGE-----".len())?;
    Some(match decrypt(text[start..end].as_bytes(), keys, unlock) {
        Ok(opened) => {
            let mut virtual_message = outer_headers(raw, root.raw_body_offset() as usize);
            virtual_message.extend_from_slice(b"Content-Type: text/plain; charset=utf-8\r\n\r\n");
            virtual_message.extend(canonical(&opened.data));
            (Some(virtual_message), PgpView { encrypted: true, opened: true, signatures: opened.signatures, security_key: None, key_elsewhere: false })
        }
        Err(_) => (None, closed()),
    })
}

/// The keys a message is encrypted to, as its packets name them (key IDs or
/// fingerprints, hexadecimal), in its order: whom it is for, nothing opened.
pub fn recipients_of(raw: &[u8]) -> Vec<String> {
    let Some(message) = mail_parser::MessageParser::default().parse(raw) else { return Vec::new() };
    let payload = message.parts.iter().find(|p| kind_of(p).0 == "application/octet-stream").map(|p| p.contents().to_vec()).unwrap_or_else(|| raw.to_vec());
    let mut out = Vec::new();
    let Ok(mut parsing) = openpgp::parse::PacketParser::from_bytes(&payload) else { return out };
    while let openpgp::parse::PacketParserResult::Some(packet) = parsing {
        if let openpgp::Packet::PKESK(pkesk) = &packet.packet {
            out.push(pkesk.recipient().map(|r| r.to_hex()).unwrap_or_default());
        }
        match packet.next() {
            Ok((_, next)) => parsing = next,
            Err(_) => break,
        }
    }
    out
}

/// Autocrypt (Level 1, §2.1): the key a message's header carries for its sender.
pub fn autocrypt_key(header: &str, from: &str) -> Option<Cert> {
    let mut addr = None;
    let mut keydata = None;
    for part in header.split(';') {
        let (name, value) = part.split_once('=')?;
        match name.trim() {
            "addr" => addr = Some(value.trim().to_string()),
            "keydata" => keydata = Some(value.split_whitespace().collect::<String>()),
            _ => {}
        }
    }
    if !addr?.eq_ignore_ascii_case(from.trim()) {
        return None;
    }
    let bytes = crate::lines::base64_decode(&keydata?)?;
    // One certificate, as `Cert::from_bytes` reads it.
    let [cert] = <[Cert; 1]>::try_from(certificates_of(&bytes).ok()?).ok()?;
    names(&cert, from).then_some(cert)
}

/// Keeps a key from an Autocrypt header: only public parts, only when it names its sender.
pub fn learn_autocrypt(header: &str, from: &str) -> Option<KeySummary> {
    let cert = autocrypt_key(header, from)?;
    store(cert.strip_secret_key_material()).ok()
}

/// The Autocrypt header for your outgoing mail: your key, smallest form
/// (primary, encryption subkey, the user ID with your address).
pub fn autocrypt_header(cert: &Cert, address: &str) -> Option<String> {
    let policy = policy();
    let valid = cert.with_policy(&policy, None).ok()?;
    let userid = valid.userids().find(|u| u.userid().email().ok().flatten().is_some_and(|e| e.eq_ignore_ascii_case(address)))?;
    let encryption = valid.keys().subkeys().alive().revoked(false).for_transport_encryption().next()?;
    let mut packets: Vec<openpgp::Packet> = vec![cert.primary_key().key().clone().into()];
    packets.push(userid.userid().clone().into());
    packets.extend(userid.self_signatures().take(1).cloned().map(Into::into));
    packets.push(encryption.key().clone().into());
    packets.extend(encryption.self_signatures().take(1).cloned().map(Into::into));
    let minimal = Cert::from_packets(packets.into_iter()).ok()?;
    let mut bytes = Vec::new();
    minimal.serialize(&mut bytes).ok()?;
    // A space every 76 characters, where the header may be folded (Autocrypt §2.1):
    // a line holds 998 characters at most (RFC 5322 §2.1.1), and an RSA key
    // imported from GnuPG takes several thousand.
    let encoded: Vec<char> = crate::lines::base64_encode(&bytes).chars().collect();
    let keydata = encoded.chunks(76).map(|chunk| chunk.iter().collect::<String>()).collect::<Vec<_>>().join(" ");
    Some(format!("addr={address}; keydata={keydata}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-pgp-{}-{}", std::process::id(), std::thread::current().name().unwrap_or("t").replace("::", "-")));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn key(address: &str) -> Cert {
        CertBuilder::general_purpose(Some(format!("Someone <{address}>")))
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_password(Some(Password::from("secret words")))
            .generate()
            .unwrap()
            .0
    }

    #[test]
    fn signs_encrypts_decrypts_and_verifies() {
        let _ = temporary();
        let alice = key("alice@example.org");
        let bob = key("bob@example.org");
        let keys_of_bob = Keys { own: vec![bob.clone()], others: vec![alice.clone().strip_secret_key_material()], ..Keys::default() };
        let mut signer = here_signer(&alice, "secret words").unwrap();
        let sealed = encrypt(b"Content-Type: text/plain\r\n\r\nHello Bob.\r\n", &[&bob, &alice], Some(&mut signer)).unwrap();
        assert!(sealed.starts_with("-----BEGIN PGP MESSAGE-----"));
        let passphrase = |_: &Fingerprint| Some("secret words".to_string());
        let sessions = SessionKeys::new();
        let opened = decrypt(sealed.as_bytes(), &keys_of_bob, &Unlock::new(&passphrase, &sessions)).unwrap();
        assert_eq!(opened.data, b"Content-Type: text/plain\r\n\r\nHello Bob.\r\n");
        assert_eq!(opened.signatures.len(), 1);
        assert!(opened.signatures[0].good && opened.signatures[0].signer.contains("alice@example.org"));
        let wrong = |_: &Fingerprint| Some("other words".to_string());
        assert!(decrypt(sealed.as_bytes(), &keys_of_bob, &Unlock::new(&wrong, &sessions)).is_err());
        assert!(here_signer(&alice, "other words").is_err());
        let (signature, micalg) = sign_detached(b"signed part", &mut signer).unwrap();
        assert!(micalg.starts_with("pgp-sha"), "{micalg}");
        let checked = verify_detached(b"signed part", signature.as_bytes(), &keys_of_bob);
        assert!(checked[0].good);
        let tampered = verify_detached(b"signed part!", signature.as_bytes(), &keys_of_bob);
        assert!(!tampered[0].good);
        let stranger = verify_detached(b"signed part", signature.as_bytes(), &Keys::default());
        assert_eq!(stranger[0].problem, "unknown key");
    }

    #[test]
    fn key_files_are_yours_alone() {
        let path = temporary().join("key.asc");
        write_key(&path, b"-----BEGIN PGP PRIVATE KEY BLOCK-----").unwrap();
        write_key(&path, b"-----BEGIN PGP PRIVATE KEY BLOCK----- again").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"-----BEGIN PGP PRIVATE KEY BLOCK----- again");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_file(&path);
    }

    /// A key made here keeps its revocation certificate beside it, yours
    /// alone; a copy can be saved; removed, the key takes it aside with it.
    #[test]
    fn a_key_made_here_keeps_its_revocation_certificate() {
        let root = temporary().join("pgp-made");
        let _ = std::fs::remove_dir_all(&root);
        let made = generate_at(&root, "Jane Exemple", "jane@example.org", "secret words").unwrap();
        assert!(made.own && made.revocation);
        let kept = root.join("own").join(format!("{}.rev", made.fingerprint));
        let text = std::fs::read_to_string(&kept).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&kept).unwrap().permissions().mode() & 0o777, 0o600, "protected as the key is");
        }
        assert!(text.contains(&made.fingerprint) && text.contains("Jane Exemple <jane@example.org>"), "{text}");
        // Never imported by mistake: a colon before the armor; read as no key.
        assert!(text.contains("\n:-----BEGIN PGP PUBLIC KEY BLOCK-----") && certificates_in(text.as_bytes()).is_empty());
        // The colon taken away, it revokes this very key.
        let armored = &text[text.find(":-----BEGIN").unwrap() + 1..];
        let pile = openpgp::PacketPile::from_bytes(armored.as_bytes()).unwrap();
        let revocation = pile.descendants().find_map(|p| if let openpgp::Packet::Signature(s) = p { Some(s.clone()) } else { None }).unwrap();
        let key = Cert::from_bytes(&std::fs::read(root.join("own").join(format!("{}.asc", made.fingerprint))).unwrap()).unwrap();
        assert!(matches!(key.revocation_status(&policy(), None), openpgp::types::RevocationStatus::NotAsFarAsWeKnow));
        let (revoked, _) = key.insert_packets(revocation).unwrap();
        assert!(matches!(revoked.revocation_status(&policy(), None), openpgp::types::RevocationStatus::Revoked(_)));
        // A copy saved elsewhere, as kept, yours alone; a second one beside it.
        let elsewhere = root.join("saved");
        let copy = save_revocation_at(&root, &made.fingerprint, &elsewhere).unwrap();
        assert_eq!((copy.file_name().unwrap().to_string_lossy().to_string(), std::fs::read_to_string(&copy).unwrap()), (format!("{}.rev", made.fingerprint), text));
        assert!(save_revocation_at(&root, &made.fingerprint, &elsewhere).unwrap().ends_with(format!("{}-2.rev", made.fingerprint)));
        assert!(save_revocation_at(&root, "0000", &elsewhere).is_err(), "none kept for a key not made here");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn web_key_directory_names() {
        // The example of draft-koch-openpgp-webkey-service §3.1.
        let [advanced, direct] = wkd_urls("Joe.Doe@Example.ORG").unwrap();
        assert_eq!(direct, "https://example.org/.well-known/openpgpkey/hu/iy9q119eutrkn8s1mk4r39qejnbu3n5q?l=Joe.Doe");
        assert!(advanced.starts_with("https://openpgpkey.example.org/.well-known/openpgpkey/example.org/hu/iy9q119eutrkn8s1mk4r39qejnbu3n5q"));
        // Only a domain's own directory: no address, no local name, no port or path.
        for elsewhere in ["joe@192.168.1.1", "joe@[::1]", "joe@localhost", "joe@host.example:8080/x?", "joe@1.2.3"] {
            assert!(wkd_urls(elsewhere).is_none(), "{elsewhere}");
        }
    }

    /// A key as a security key holds it: the primary signs, a Curve25519 subkey decrypts.
    fn card_key(address: &str, made: Option<std::time::SystemTime>) -> Cert {
        let mut builder = CertBuilder::new()
            .add_userid(format!("Card Holder <{address}>"))
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_primary_key_flags(openpgp::types::KeyFlags::empty().set_certification().set_signing())
            .add_transport_encryption_subkey();
        if let Some(made) = made {
            builder = builder.set_creation_time(made).set_validity_period(std::time::Duration::from_secs(365 * 24 * 3600));
        }
        builder.generate().unwrap().0
    }

    fn record(card: &securitykey::SoftCard, cert: &Cert, address: &str) -> KnownCard {
        KnownCard {
            ident: card.info.ident.clone(),
            sign: card.info.sign.as_ref().map(|s| s.fingerprint.clone()).unwrap_or_default(),
            decrypt: card.info.decrypt.as_ref().map(|s| s.fingerprint.clone()).unwrap_or_default(),
            cert: cert.fingerprint().to_hex(),
            addresses: vec![address.to_string()],
            ..KnownCard::default()
        }
    }

    #[test]
    fn a_security_key_signs_alone_for_its_addresses() {
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        let here = key("me@example.org");
        let mut keys = Keys { own: vec![on_card.clone().strip_secret_key_material(), here.clone()], ..Keys::default() };
        assert_eq!(keys.own_for("me@example.org").unwrap().place, Place::Here);
        keys.cards.push(record(&card, &on_card, "me@example.org"));
        let own = keys.own_for("ME@example.org").unwrap();
        assert_eq!(own.place, Place::Card { ident: "0006:12345678".into() });
        assert_eq!(own.cert.fingerprint(), on_card.fingerprint());
        // Autocrypt announces the security key's certificate.
        assert!(autocrypt_header(own.cert, "me@example.org").is_some());
        assert!(keys.own_for("other@example.org").is_none());
        // Expired on the card: nothing signs for the address, and that is known.
        let late = card_key("late@example.org", Some(std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 365 * 24 * 3600)));
        let late_card = securitykey::SoftCard::new("0006:00000002", &late, "123456");
        let keys = Keys { own: vec![late.clone().strip_secret_key_material(), key("late@example.org")], cards: vec![record(&late_card, &late, "late@example.org")], ..Keys::default() };
        assert!(keys.own_for("late@example.org").is_none());
        assert!(keys.card_for("late@example.org").is_some());
    }

    /// The folders of a device's keys (`pgp/`), and its security keys made known to the others.
    fn read_here(root: &std::path::Path) {
        securitykey::share_cards_in(&root.join("cards.toml"), &root.join("own"), &root.join(securitykey::SHARED_CARDS), false).unwrap();
    }

    /// What the sharing carries from one device's `pgp/cards/` to another's.
    fn carry(from: &std::path::Path, to: &std::path::Path) {
        let (from, to) = (from.join(securitykey::SHARED_CARDS), to.join(securitykey::SHARED_CARDS));
        let _ = std::fs::remove_dir_all(&to);
        std::fs::create_dir_all(&to).unwrap();
        for entry in std::fs::read_dir(&from).unwrap().filter_map(Result::ok) {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }

    /// Batch A (18 October 2026): your security key's public half travels
    /// (`pgp/cards/`), written by the computer that read the key, read by a
    /// phone that never did: it then encrypts to you, hands your key by
    /// Autocrypt and knows what waits for the key. Never a secret; a
    /// certificate replaced, or another key's, is refused; a certificate
    /// claiming your address from outside is never yours.
    #[test]
    fn your_security_keys_certificate_travels_and_only_it_is_yours() {
        let root = temporary().join("cards-travel");
        let _ = std::fs::remove_dir_all(&root);
        let (computer, phone) = (root.join("computer"), root.join("phone"));
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        store_in(&computer, on_card.clone().strip_secret_key_material(), true).unwrap();
        securitykey::change_known_at(&computer.join("cards.toml"), |cards| cards.push(record(&card, &on_card, "me@example.org"))).unwrap();
        read_here(&computer);
        let shared = std::fs::read_to_string(computer.join("cards").join("0006-12345678.toml")).unwrap();
        assert!(shared.contains("BEGIN PGP PUBLIC KEY BLOCK") && !shared.contains("PRIVATE KEY"), "public parts only");
        // The phone: nothing of its own, the card's file carried by the sharing.
        carry(&computer, &phone);
        let keys = Keys::load_at(&phone);
        assert_eq!(keys.cards.len(), 1);
        let own = keys.own_for("me@example.org").expect("your key, known on the phone");
        assert_eq!(own.place, Place::Card { ident: "0006:12345678".into() });
        let yours = keys.self_cert_for("me@example.org").expect("yours to encrypt to");
        assert_eq!(yours.fingerprint(), on_card.fingerprint());
        assert!(autocrypt_header(yours, "me@example.org").is_some(), "Autocrypt from the phone too");
        // What waits for the key is known there.
        let none = |_: &Fingerprint| None;
        let sessions = SessionKeys::new();
        let unlock = Unlock::new(&none, &sessions);
        let sealed = encrypt(b"Hello me.", &[yours], None).unwrap();
        assert!(decrypt(sealed.as_bytes(), &keys, &unlock).is_err());
        assert_eq!(unlock.wanted().as_deref(), Some("0006:12345678"));
        // And your key opens it, wherever it can be used.
        let readers = securitykey::SoftReaders::with(vec![securitykey::SoftCard::new("0006:12345678", &on_card, "123456").with_x25519(securitykey::x25519::x25519)]);
        let pin = Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        assert_eq!(decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions).with_card(&access)).unwrap().data, b"Hello me.");
        // A certificate claiming your address from outside (an Autocrypt header) is never yours.
        let stranger = key("me@example.org");
        store_in(&phone, stranger.clone().strip_secret_key_material(), false).unwrap();
        let keys = Keys::load_at(&phone);
        assert_eq!(keys.for_address("me@example.org").map(Cert::fingerprint), Some(on_card.fingerprint()));
        assert!(keys.own.iter().all(|c| c.fingerprint() != stranger.fingerprint()));
        // A shared file whose certificate was replaced, or is another key's: refused.
        let mut replaced: securitykey::SharedCard = toml::from_str(&shared).unwrap();
        replaced.certificate = String::from_utf8(stranger.clone().strip_secret_key_material().armored().to_vec().unwrap()).unwrap();
        std::fs::write(phone.join("cards").join("0006-12345678.toml"), toml::to_string(&replaced).unwrap()).unwrap();
        let keys = Keys::load_at(&phone);
        assert!(keys.cards.is_empty() && keys.own_for("me@example.org").is_none() && keys.own.iter().all(|c| c.fingerprint() != stranger.fingerprint()), "replaced: refused");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A computer whose key changed (renewed, as on 8 October 2026) updates
    /// what the others know; a computer still holding the older certificate
    /// never undoes it; an expired certificate of yours takes no encryption.
    #[test]
    fn a_renewal_reaches_every_device_and_is_never_undone() {
        let root = temporary().join("cards-renewal");
        let _ = std::fs::remove_dir_all(&root);
        let (one, two, phone) = (root.join("one"), root.join("two"), root.join("phone"));
        let made = std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 3600);
        let on_card = card_key("me@example.org", Some(made));
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        for computer in [&one, &two] {
            store_in(computer, on_card.clone().strip_secret_key_material(), true).unwrap();
            securitykey::change_known_at(&computer.join("cards.toml"), |cards| cards.push(record(&card, &on_card, "me@example.org"))).unwrap();
            read_here(computer);
        }
        let expires = |keys: &Keys| keys.own_for("me@example.org").and_then(|o| o.cert.with_policy(&policy(), None).ok()?.primary_key().key_expiration_time());
        carry(&one, &phone);
        let before = expires(&Keys::load_at(&phone)).unwrap();
        // Renewed on the first computer: kept with its keys, the shared file follows.
        let mut signer = on_card.primary_key().key().clone().parts_into_secret().unwrap().into_keypair().unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2 * 365 * 24 * 3600);
        let signatures = on_card.set_expiration_time(&policy(), None, &mut signer, Some(later)).unwrap();
        let renewed = on_card.clone().insert_packets(signatures).unwrap().0;
        store_in(&one, renewed.strip_secret_key_material(), true).unwrap();
        carry(&one, &phone);
        let after = expires(&Keys::load_at(&phone)).unwrap();
        assert!(after > before, "renewed on the phone too");
        // The second computer, still on the older certificate, writes after: the renewal stays.
        carry(&one, &two);
        securitykey::change_known_at(&two.join("cards.toml"), |cards| cards[0].holder = "Card Holder".into()).unwrap();
        read_here(&two);
        carry(&two, &phone);
        assert_eq!(expires(&Keys::load_at(&phone)), Some(after), "never undone");
        // An expired certificate of yours: no encryption to it, said by the window (`self_cert_for` none).
        let old = card_key("old@example.org", Some(std::time::SystemTime::now() - std::time::Duration::from_secs(400 * 24 * 3600)));
        let old_card = securitykey::SoftCard::new("0006:00000001", &old, "123456");
        let keys = Keys { own: vec![old.clone().strip_secret_key_material()], cards: vec![record(&old_card, &old, "old@example.org")], ..Keys::default() };
        assert!(keys.is_own_address("old@example.org") && keys.self_cert_for("old@example.org").is_none() && keys.for_address("old@example.org").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_message_to_the_security_key_waits_for_it_then_opens_from_memory() {
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456").with_x25519(securitykey::x25519::x25519);
        let keys = Keys { own: vec![on_card.clone().strip_secret_key_material()], cards: vec![record(&card, &on_card, "me@example.org")], ..Keys::default() };
        let readers = securitykey::SoftReaders::with(vec![card]);
        let sealed = encrypt(b"Hello me.", &[&on_card], None).unwrap();
        let none = |_: &Fingerprint| None;
        let sessions = SessionKeys::new();
        // Shown: nothing is asked of the key, the message says which key it waits for.
        let unlock = Unlock::new(&none, &sessions);
        assert!(decrypt(sealed.as_bytes(), &keys, &unlock).is_err());
        assert_eq!(unlock.wanted().as_deref(), Some("0006:12345678"));
        assert_eq!(readers.card("0006:12345678", |c| c.operations), Some(0));
        // A wrong PIN: said, nothing opened.
        let wrong = Password::from("654321");
        let access = CardAccess::new(&readers, &wrong, &|| {});
        let unlock = Unlock::new(&none, &sessions).with_card(&access);
        assert!(decrypt(sealed.as_bytes(), &keys, &unlock).is_err());
        assert_eq!(access.failure(), Some(securitykey::CardError::WrongPin { left: 2 }));
        // "Open with your security key": opened, its session key kept.
        let pin = Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        let opened = decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions).with_card(&access)).unwrap();
        assert_eq!(opened.data, b"Hello me.");
        assert_eq!(readers.card("0006:12345678", |c| c.operations), Some(1));
        // Again, its attachments, its answer: from memory, the key left alone.
        let again = decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions)).unwrap();
        assert_eq!(again.data, b"Hello me.");
        assert_eq!(readers.card("0006:12345678", |c| c.operations), Some(1));
        // Another message still waits for the key; forgotten, this one too.
        let other = encrypt(b"Another.", &[&on_card], None).unwrap();
        assert!(decrypt(other.as_bytes(), &keys, &Unlock::new(&none, &sessions)).is_err());
        sessions.forget();
        assert!(decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions)).is_err());
    }

    #[test]
    fn an_expired_security_key_still_opens_older_mail() {
        let then = std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 365 * 24 * 3600);
        let late = card_key("late@example.org", Some(then));
        // Sent while the key was valid.
        let policy = policy();
        let sent = then + std::time::Duration::from_secs(24 * 3600);
        let recipients: Vec<_> = late.keys().with_policy(&policy, sent).alive().for_transport_encryption().collect();
        assert!(!recipients.is_empty());
        let mut sealed = Vec::new();
        {
            let message = Message::new(&mut sealed);
            let message = Encryptor::for_recipients(message, recipients).build().unwrap();
            let mut literal = LiteralWriter::new(message).build().unwrap();
            literal.write_all(b"Older mail.").unwrap();
            literal.finalize().unwrap();
        }
        let card = securitykey::SoftCard::new("0006:00000002", &late, "123456").with_x25519(securitykey::x25519::x25519);
        let keys = Keys { own: vec![late.clone().strip_secret_key_material()], cards: vec![record(&card, &late, "late@example.org")], ..Keys::default() };
        let readers = securitykey::SoftReaders::with(vec![card]);
        let pin = Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        let none = |_: &Fingerprint| None;
        let sessions = SessionKeys::new();
        let opened = decrypt(&sealed, &keys, &Unlock::new(&none, &sessions).with_card(&access)).unwrap();
        assert_eq!(opened.data, b"Older mail.");
        // But nobody can encrypt to it now.
        assert!(encrypt(b"New mail.", &[&late], None).is_err());
    }

    #[test]
    fn a_security_key_certificate_comes_back_to_your_keys() {
        let root = temporary().join("pgp-card");
        let _ = std::fs::remove_dir_all(&root);
        let on_card = card_key("me@example.org", None);
        let name = format!("{}.asc", on_card.fingerprint().to_hex());
        let kept = store_in(&root, on_card.clone().strip_secret_key_material(), true).unwrap();
        assert!(kept.own);
        let file = std::fs::read_to_string(root.join("own").join(&name)).unwrap();
        assert!(file.starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----"), "public only: {file}");
        // Brought again, by an Autocrypt header or a lookup: with yours, never with others'.
        let header = autocrypt_header(&on_card, "me@example.org").unwrap();
        let again = autocrypt_key(&header, "me@example.org").unwrap();
        assert!(store_at(&root, again.strip_secret_key_material()).unwrap().own);
        assert!(!root.join("others").join(&name).exists());
        // Someone else's goes with others'.
        let jane = key("jane@example.org").strip_secret_key_material();
        assert!(!store_at(&root, jane).unwrap().own);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn autocrypt_carries_the_smallest_key() {
        let alice = key("alice@example.org");
        let header = autocrypt_header(&alice, "alice@example.org").unwrap();
        assert!(header.starts_with("addr=alice@example.org; keydata="));
        assert!(header.split_whitespace().all(|word| word.len() <= 100), "foldable: {header}");
        let found = autocrypt_key(&header, "alice@example.org").unwrap();
        assert_eq!(found.fingerprint(), alice.fingerprint());
        assert!(!found.is_tsk());
        assert!(autocrypt_key(&header, "mallory@example.org").is_none());
    }

    /// A key made and exported by GnuPG 2.4.4 (Ubuntu 24.04), cut to its
    /// primary key, its user ID and two of its subkeys: the second subkey's
    /// binding signature has its second number written over 32 octets, the
    /// first a zero, its length said as 256 bits.
    const GNUPG_2_4_4: &[u8] = include_bytes!("../tests/fixtures/pgp/gnupg-2.4.4-non-minimal-mpi.asc");

    /// The CI's GnuPG wrote about one key signature in 128 so: Sequoia alone
    /// drops it, and a subkey (or the whole key, when it is the user ID's)
    /// reads as missing (securitykey's test failed once in a while on CI).
    /// Read here as its value, every signature still checked by Sequoia.
    #[test]
    fn a_signature_gnupg_wrote_long_is_read_as_its_value() {
        let strict = Cert::from_bytes(GNUPG_2_4_4).unwrap();
        assert_eq!(securitykey::parts(&strict).len(), 2, "Sequoia alone: the primary key and one subkey, the other's binding dropped");
        let read = certificates_in(GNUPG_2_4_4);
        assert_eq!(read.len(), 1);
        let parts = securitykey::parts(&read[0]);
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert!(parts[1..].iter().all(|part| part.role == securitykey::Role::Authentication), "{parts:?}");
        assert_eq!(securitykey::parts_in(GNUPG_2_4_4), parts);
        // Every signature there checked: the subkeys bound by the primary key.
        let policy = policy();
        let valid = read[0].with_policy(&policy, None).unwrap();
        assert_eq!(valid.keys().subkeys().count(), 2);
        // Read once, it is written shortest: nothing left to change.
        let written = read[0].to_vec().unwrap();
        assert!(matches!(minimal_mpis(&written), std::borrow::Cow::Borrowed(_)));
        assert!(matches!(minimal_mpis(GNUPG_2_4_4), std::borrow::Cow::Owned(_)));
    }

    /// The same fault made by hand on Sequoia's own key: each signature's
    /// last number written one octet longer, a zero first. Read back, the
    /// bytes are the key's own again; a key already shortest, a file that
    /// is no key and one cut short come back as they came.
    #[test]
    fn numbers_written_long_come_back_shortest() {
        let alice = key("alice@example.org").strip_secret_key_material();
        let binary = alice.to_vec().unwrap();
        let armored = String::from_utf8(alice.armored().to_vec().unwrap()).unwrap();
        assert!(matches!(minimal_mpis(&binary), std::borrow::Cow::Borrowed(_)));
        assert!(matches!(minimal_mpis(armored.as_bytes()), std::borrow::Cow::Borrowed(_)));
        let long = written_long(&binary);
        assert!(Cert::from_bytes(&long).ok().is_none_or(|strict| securitykey::parts(&strict).is_empty()), "Sequoia alone refuses what is written long");
        assert_eq!(minimal_mpis(&long).as_ref(), binary.as_slice());
        assert_eq!(certificates_in(&long), [alice]);
        // Nothing to read, or cut short: as it came.
        assert!(matches!(minimal_mpis(b"no key here"), std::borrow::Cow::Borrowed(_)));
        assert_eq!(minimal_mpis(b"no key here").as_ref(), b"no key here");
        let cut = &long[..long.len() - 5];
        assert_eq!(minimal_mpis(cut).as_ref(), cut);
        assert!(matches!(minimal_mpis(&[]), std::borrow::Cow::Borrowed(_)));
    }

    /// Where a version 4 signature body's numbers begin: past its two areas
    /// and the hash's first two octets.
    fn numbers_at(body: &[u8]) -> usize {
        let mut at = 4;
        for _ in 0..2 {
            at += 2 + usize::from(u16::from_be_bytes([body[at], body[at + 1]]));
        }
        at + 2
    }

    /// `body` (a signature's) with its last number written anew: `new` gets
    /// its length in bits and the octets from it on, and gives the length to
    /// say and the octets to write.
    fn last_number_as(body: &[u8], new: impl Fn(usize, &[u8]) -> (usize, Vec<u8>)) -> Vec<u8> {
        let mut at = numbers_at(body);
        let mut last = at;
        while at < body.len() {
            last = at;
            at += 2 + usize::from(u16::from_be_bytes([body[at], body[at + 1]])).div_ceil(8);
        }
        let bits = usize::from(u16::from_be_bytes([body[last], body[last + 1]]));
        let (said, octets) = new(bits, &body[last + 2..]);
        let mut out = body[..last].to_vec();
        out.extend_from_slice(&u16::try_from(said).unwrap().to_be_bytes());
        out.extend_from_slice(&octets);
        out
    }

    /// GnuPG 2.4.4's fault, made by hand: a number one octet longer, a zero
    /// first, its length said with it.
    fn one_octet_long(bits: usize, value: &[u8]) -> (usize, Vec<u8>) {
        let octets = bits.div_ceil(8);
        (octets * 8 + 8, std::iter::once(0).chain(value[..octets].iter().copied()).collect())
    }

    /// The packets of `binary`, each signature's body as `each` writes it.
    fn signatures_as(binary: &[u8], each: impl Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < binary.len() {
            let (tag, body) = packet_at(binary, at).unwrap();
            let packet = &binary[body.clone()];
            put_packet(&mut out, tag, &if tag == 2 { each(packet) } else { packet.to_vec() });
            at = body.end;
        }
        out
    }

    /// Each signature's last number written one octet longer.
    fn written_long(binary: &[u8]) -> Vec<u8> {
        signatures_as(binary, |body| last_number_as(body, one_octet_long))
    }

    /// Binary packets armored as a public key block.
    fn armored_block(binary: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut writer = openpgp::armor::Writer::new(&mut out, openpgp::armor::Kind::PublicKey).unwrap();
        writer.write_all(binary).unwrap();
        writer.finalize().unwrap();
        out
    }

    /// Several keys in one file, each its armored block, some needing the
    /// repair and some not, in every order, words between them: every key
    /// read, each whole, never fewer than Sequoia alone reads. Until 15
    /// October 2026 a first block that needed the repair was read alone,
    /// the next ones dropped, and a later one was not repaired.
    #[test]
    fn concatenated_armored_keys_are_each_read_and_repaired() {
        let gnupg = certificates_in(GNUPG_2_4_4).remove(0);
        let bob = key("bob@example.org").strip_secret_key_material();
        let carol = key("carol@example.org").strip_secret_key_material();
        let blocks: [(&Cert, Vec<u8>); 3] = [(&gnupg, GNUPG_2_4_4.to_vec()), (&bob, bob.armored().to_vec().unwrap()), (&carol, armored_block(&written_long(&carol.to_vec().unwrap())))];
        let orders: [&[usize]; 12] = [&[0, 1], &[1, 0], &[1, 2], &[2, 1], &[0, 2], &[2, 0], &[0, 1, 2], &[0, 2, 1], &[1, 0, 2], &[1, 2, 0], &[2, 0, 1], &[2, 1, 0]];
        for order in orders {
            let mut file = Vec::new();
            for &i in order {
                file.extend_from_slice(&blocks[i].1);
                file.extend_from_slice(b"\nSomething said between the keys.\n");
            }
            let read = certificates_of(&file).unwrap();
            let fingerprints: Vec<Fingerprint> = read.iter().map(Cert::fingerprint).collect();
            let wanted: Vec<Fingerprint> = order.iter().map(|&i| blocks[i].0.fingerprint()).collect();
            assert_eq!(fingerprints, wanted, "{order:?}");
            for (cert, &i) in read.iter().zip(order) {
                assert_eq!(securitykey::parts(cert), securitykey::parts(blocks[i].0), "{order:?}: key {i} whole");
            }
            let alone: Vec<Fingerprint> = CertParser::from_bytes(&file).unwrap().filter_map(Result::ok).map(|c| c.fingerprint()).collect();
            assert!(alone.iter().all(|f| fingerprints.contains(f)), "{order:?}: never fewer than Sequoia alone");
            assert_eq!(matches!(minimal_mpis(&file), std::borrow::Cow::Owned(_)), order.contains(&0) || order.contains(&2), "{order:?}");
        }
        // Bob's alone, nothing to repair: as it came.
        assert!(matches!(minimal_mpis(&blocks[1].1), std::borrow::Cow::Borrowed(_)));
    }

    /// A key made by GnuPG 2.4.9 with a subkey that signs: its binding carries
    /// the subkey's own signature back (the back-signature) in its unhashed area.
    const SIGNING_SUBKEY: &[u8] = include_bytes!("../tests/fixtures/pgp/gnupg-2.4.9-signing-subkey.asc");

    /// A signature `body` with each signature embedded in its unhashed area
    /// written as `each` says; with `to_hashed`, moved into its hashed area.
    fn embedded_as(body: &[u8], each: impl Fn(&[u8]) -> Vec<u8>, to_hashed: bool) -> Vec<u8> {
        let hashed_end = 6 + usize::from(u16::from_be_bytes([body[4], body[5]]));
        let area_len = usize::from(u16::from_be_bytes([body[hashed_end], body[hashed_end + 1]]));
        let area = &body[hashed_end + 2..hashed_end + 2 + area_len];
        let (mut kept, mut embedded) = (Vec::new(), Vec::new());
        let mut at = 0;
        while at < area.len() {
            // These subpackets are short: one octet of length.
            let length = usize::from(area[at]);
            assert!(length < 192);
            let (kind, inner) = (area[at + 1], &area[at + 2..at + 1 + length]);
            if kind & 0x7F == 32 {
                let written = each(inner);
                put_subpacket_length(&mut embedded, 1 + written.len());
                embedded.push(kind);
                embedded.extend_from_slice(&written);
            } else {
                kept.extend_from_slice(&area[at..at + 1 + length]);
            }
            at += 1 + length;
        }
        assert!(!embedded.is_empty(), "a back-signature there");
        let (hashed, unhashed) = if to_hashed { ([&body[6..hashed_end], &embedded[..]].concat(), kept) } else { (body[6..hashed_end].to_vec(), [&kept[..], &embedded[..]].concat()) };
        let mut out = body[..4].to_vec();
        out.extend_from_slice(&u16::try_from(hashed.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&hashed);
        out.extend_from_slice(&u16::try_from(unhashed.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&unhashed);
        out.extend_from_slice(&body[hashed_end + 2 + area_len..]);
        out
    }

    /// The binding's back-signature written long, as GnuPG 2.4.4 may write
    /// any of its signatures: Sequoia alone drops the binding, and the
    /// signing subkey with it; read here, the key is whole again. The same
    /// number written long in the hashed area, which the binding signs, is
    /// left as it came.
    #[test]
    fn a_back_signature_written_long_is_read_shortest_in_the_unhashed_area_only() {
        let canonical = Cert::from_bytes(SIGNING_SUBKEY).unwrap();
        let signs = |cert: &Cert| securitykey::parts(cert).iter().any(|part| part.role == securitykey::Role::Signing);
        assert!(signs(&canonical));
        let mut binary = Vec::new();
        std::io::Read::read_to_end(&mut openpgp::armor::Reader::from_bytes(SIGNING_SUBKEY, openpgp::armor::ReaderMode::Tolerant(None)), &mut binary).unwrap();
        let binding = |to_hashed: bool| move |body: &[u8]| if body[1] == 0x18 { embedded_as(body, |inner| last_number_as(inner, one_octet_long), to_hashed) } else { body.to_vec() };
        let long = signatures_as(&binary, binding(false));
        assert!(Cert::from_bytes(&long).ok().is_none_or(|strict| !signs(&strict)), "Sequoia alone drops the signing subkey");
        assert!(matches!(minimal_mpis(&long), std::borrow::Cow::Owned(_)));
        assert_eq!(Cert::from_bytes(minimal_mpis(&long).as_ref()).unwrap(), canonical);
        let read = certificates_in(&long);
        assert_eq!(read.len(), 1);
        assert!(signs(&read[0]));
        // In the hashed area instead: never touched.
        let hashed = signatures_as(&binary, binding(true));
        assert!(matches!(minimal_mpis(&hashed), std::borrow::Cow::Borrowed(_)), "the signed area as it came");
    }

    /// Only ever shorter: a number whose length is said one bit too short
    /// (a bit past it set, which Sequoia refuses and GnuPG never writes) is
    /// left as it came, and so is its whole signature, though another of its
    /// numbers be written long.
    #[test]
    fn a_length_said_too_short_is_left_as_it_came() {
        let dave = key("dave@example.org").strip_secret_key_material().to_vec().unwrap();
        // The first number said a bit short in as many octets; none when it cannot be.
        let short_first = |body: &[u8]| -> Option<Vec<u8>> {
            let first = numbers_at(body);
            let bits = usize::from(u16::from_be_bytes([body[first], body[first + 1]]));
            if bits < 2 || (bits - 1).div_ceil(8) != bits.div_ceil(8) {
                return None;
            }
            let mut out = body.to_vec();
            out[first..first + 2].copy_from_slice(&u16::try_from(bits - 1).unwrap().to_be_bytes());
            Some(out)
        };
        let mut checked = 0;
        let mut at = 0;
        while at < dave.len() {
            let (tag, body) = packet_at(&dave, at).unwrap();
            if let Some(short) = (tag == 2).then(|| short_first(&dave[body.clone()])).flatten() {
                checked += 1;
                assert_eq!(signature_shortened(&short), None, "said short: as it came");
                assert_eq!(signature_shortened(&last_number_as(&short, one_octet_long)), None, "beside a number written long: as it came");
            }
            at = body.end;
        }
        assert!(checked > 0);
        let short_key = signatures_as(&dave, |body| short_first(body).unwrap_or_else(|| body.to_vec()));
        assert!(matches!(minimal_mpis(&short_key), std::borrow::Cow::Borrowed(_)));
    }

    /// A signature body (EdDSA) whose unhashed area holds `inner`, embedded,
    /// if any, and whose two numbers are `numbers` (their lengths said).
    fn nesting(inner: Option<&[u8]>, numbers: &[u8]) -> Vec<u8> {
        let mut area = Vec::new();
        if let Some(inner) = inner {
            put_subpacket_length(&mut area, 1 + inner.len());
            area.push(32);
            area.extend_from_slice(inner);
        }
        let mut body = vec![4, 0x19, 22, 10, 0, 0];
        body.extend_from_slice(&u16::try_from(area.len()).unwrap().to_be_bytes());
        body.extend_from_slice(&area);
        body.extend_from_slice(&[0xAB, 0xCD]);
        body.extend_from_slice(numbers);
        body
    }

    /// Signatures embedded in embedded signatures, thousands deep (made by
    /// hand: no OpenPGP program writes them, Sequoia refuses them): looked
    /// into one level only, so the file comes back at once, as it came, on
    /// a thread with a small stack (a phone's import runs on Qt's, about
    /// 1 MiB), though the deepest's numbers are written long. One level
    /// down, a signature written long is still repaired (a back-signature);
    /// two levels down, never.
    #[test]
    fn signatures_nested_deep_are_looked_into_one_level_only() {
        // r and s, each the number 1: shortest, then one said in nine bits, a zero octet first.
        let shortest: &[u8] = &[0, 1, 1, 0, 1, 1];
        let long: &[u8] = &[0, 9, 0, 1, 0, 1, 1];
        let packet = |body: &[u8]| {
            let mut out = Vec::new();
            put_packet(&mut out, 2, body);
            out
        };
        let mut deep = nesting(None, long);
        let mut levels = 0;
        while deep.len() < 60_000 {
            deep = nesting(Some(&deep), shortest);
            levels += 1;
        }
        assert!(levels > 2_000, "{levels} levels");
        let file = packet(&nesting(Some(&deep), shortest));
        let read = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || matches!(minimal_mpis(&file), std::borrow::Cow::Borrowed(_))).unwrap();
        assert!(read.join().unwrap(), "as it came, at once");
        // One level down: repaired; two levels down: as it came.
        let one = packet(&nesting(Some(&nesting(None, long)), shortest));
        assert!(matches!(minimal_mpis(&one), std::borrow::Cow::Owned(_)));
        let two = packet(&nesting(Some(&nesting(Some(&nesting(None, long)), shortest)), shortest));
        assert!(matches!(minimal_mpis(&two), std::borrow::Cow::Borrowed(_)));
    }
}

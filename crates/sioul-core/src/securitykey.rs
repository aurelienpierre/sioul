// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your OpenPGP keys on a security key: an OpenPGP card, such as a YubiKey or
//! a Nitrokey (OpenPGP card specification 3.4).
//!
//! The private keys never leave the security key. Sioul hands it what must be
//! signed (a digest) or opened (a session key, wrapped), after its PIN, and
//! gets the result back; the key may also wait for a touch. Everything public
//! Sioul keeps itself, the certificate above all: a security key holds no
//! certificate, only its keys' fingerprints and public parts.
//!
//! - [`Readers`] is how Sioul reaches security keys: the system's smart card
//!   service (PC/SC) on a computer, a software stand-in ([`SoftReaders`]) in
//!   tests. A key is opened for one operation, then closed, which resets it:
//!   no other program finds it unlocked afterwards.
//! - [`CardSigner`] and [`CardDecryptor`] are Sequoia's own `Signer` and
//!   `Decryptor` for a key on a security key. Each opens the security key when
//!   Sequoia asks for a signature or a session key, not before. The glue
//!   follows Heiko Schaefer's `openpgp-card-sequoia`, as Sequoia's key store
//!   carries it (`sequoia-keystore-openpgp-card` 0.2.2, its `decrypt_ciphertext`
//!   and `sign`, MIT OR Apache-2.0).
//! - [`check`] tells whether a certificate belongs to a security key: its
//!   fingerprints, its public keys, what each key may do, until when.
//! - [`KnownCard`] is what Sioul remembers of a security key, in
//!   `pgp/cards.toml`: no secret.
//! - [`PinMemory`] holds a PIN in memory only, kept encrypted there by
//!   Sequoia, for fifteen minutes without use.
//! - [`CardError`] says each failure in words, with what to do.
//!
//! Sioul only uses a security key: it never asks for its Admin PIN, never
//! changes or unblocks a PIN, never loads or makes keys on it.

use crate::config::data_dir;
use crate::i18n::Translator;
use openpgp::Cert;
use openpgp::cert::prelude::*;
use openpgp::crypto::{self, SessionKey, mpi};
use openpgp::packet::{Key, key};
use openpgp::policy::StandardPolicy;
use openpgp::types::Curve;
use sequoia_openpgp as openpgp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// A certificate, as the crates that do not use Sequoia themselves name it.
pub use openpgp::Cert as Certificate;
/// A PIN, or a passphrase: kept encrypted in memory by Sequoia, wiped when dropped.
pub use openpgp::crypto::Password;
/// Bytes kept in protected memory, wiped when dropped: what a card opened.
pub use openpgp::crypto::mem::Protected;
/// A hash, as a card's RSA signature names it.
pub use openpgp::types::HashAlgorithm;

/// A public key as Sequoia holds it, whatever its role in its certificate.
pub type PublicKey = Key<key::PublicParts, key::UnspecifiedRole>;

/// Whether a key on the security key waits for a touch before it works: its
/// "user interaction flag", the touch policy of a YubiKey.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Touch {
    /// Never.
    #[default]
    Off,
    /// For each operation.
    On,
    /// For each operation, and it cannot be turned off.
    Fixed,
    /// Once for the operations of the next fifteen seconds.
    Cached,
    /// Cached, and it cannot be turned off.
    CachedFixed,
    /// A setting Sioul does not know: taken as asking.
    Unknown,
}

impl Touch {
    /// From the card's own number (OpenPGP card specification 3.4, §4.1.3.2,
    /// and YubiKey's additions).
    pub fn from_flag(flag: u8) -> Touch {
        match flag {
            0 => Touch::Off,
            1 => Touch::On,
            2 => Touch::Fixed,
            3 => Touch::Cached,
            4 => Touch::CachedFixed,
            _ => Touch::Unknown,
        }
    }

    /// The key may wait for a finger: Sioul says so before it asks.
    pub fn asked(self) -> bool {
        self != Touch::Off
    }
}

/// Which check of the PIN an operation needs. A card keeps two from the one
/// PIN: for signing (PW1 in mode 81), and for the rest (mode 82).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinMode {
    /// Before a signature.
    Sign,
    /// Before opening a session key.
    Decrypt,
}

/// What Sioul was doing with the key, for the words of a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Signing a message at Send.
    Sign,
    /// Opening an encrypted message.
    Open,
    /// Reading what the key says, for its setup.
    Read,
}

/// A key's public part as the card gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardPublic {
    /// RSA: its modulus and its exponent.
    Rsa {
        /// The modulus.
        n: Vec<u8>,
        /// The public exponent.
        e: Vec<u8>,
    },
    /// An elliptic curve's point (Curve25519's without the 0x40 that OpenPGP puts before it).
    Ecc {
        /// The point.
        point: Vec<u8>,
    },
}

/// One key of the security key, as it describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotInfo {
    /// Its fingerprint as the card keeps it: 40 hexadecimal digits, upper
    /// case (a card holds version 4 keys).
    pub fingerprint: String,
    /// When the key was made, as the card keeps it (Unix seconds).
    pub created: u32,
    /// "Ed25519", "Curve25519", "RSA 4096": its algorithm, by the card's attributes.
    pub algorithm: String,
    /// Whether it waits for a touch.
    pub touch: Touch,
    /// Its public part, when the card gives it.
    pub public: Option<CardPublic>,
}

/// What a security key says without its PIN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardInfo {
    /// Its maker's number and its serial number, as `openpgp-card` names a
    /// card: "0006:12345678".
    pub ident: String,
    /// Its maker's number (OpenPGP card specification, §4.2.1): 0006 is Yubico.
    pub manufacturer: u16,
    /// Its serial number.
    pub serial: u32,
    /// The name written on it, maybe empty.
    pub holder: String,
    /// The address of its public key, written on it, maybe empty.
    pub url: String,
    /// Its signing key, when it holds one.
    pub sign: Option<SlotInfo>,
    /// Its decryption key, when it holds one.
    pub decrypt: Option<SlotInfo>,
    /// Tries left for its PIN.
    pub pin_tries: u8,
    /// Tries left for its reset code: 0 when it has none.
    pub reset_tries: u8,
    /// Tries left for its Admin PIN.
    pub admin_tries: u8,
    /// The PIN is asked for each signature ("Signature PIN: forced").
    pub pin_each_signature: bool,
    /// Signatures it made so far.
    pub signatures: u32,
    /// The reader it is in, by the system's name: Sioul watches it to forget
    /// the PIN when the key goes.
    pub reader: String,
}

impl CardInfo {
    /// "YubiKey 12 345 678": the key as sentences name it.
    pub fn label(&self, tr: &Translator) -> String {
        label(self.manufacturer, self.serial, tr)
    }

    /// The key's slot of `fingerprint`, if it holds that key.
    pub fn slot_of(&self, fingerprint: &str) -> Option<&SlotInfo> {
        [&self.sign, &self.decrypt].into_iter().flatten().find(|s| s.fingerprint.eq_ignore_ascii_case(fingerprint))
    }
}

/// The name of a security key by its maker, then its serial number as printed
/// on it: "YubiKey 12 345 678", "Nitrokey 000F 1A2B".
pub fn label(manufacturer: u16, serial: u32, tr: &Translator) -> String {
    let maker = match manufacturer {
        0x0006 => "YubiKey".to_string(),
        0x000F => "Nitrokey".to_string(),
        0xF1D0 => "CanoKey".to_string(),
        0xF517 => "Gnuk".to_string(),
        0x0005 => "ZeitControl".to_string(),
        _ => tr.text("seckey-generic", None),
    };
    format!("{maker} {}", serial_text(serial))
}

/// A serial number as printed on the key. Yubico writes its serials in decimal
/// digits inside the card's hexadecimal ones ("12345678" reads 0x12345678), so
/// they are shown in groups of three, as a number; others stay hexadecimal.
pub fn serial_text(serial: u32) -> String {
    let digits = format!("{serial:08X}");
    let digits = digits.trim_start_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return digits.to_string();
    }
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('\u{202F}');
        }
        out.push(c);
    }
    out
}

/// "Ed25519", "Curve25519", "RSA 4096", "NIST P-256": a key's algorithm, as
/// GnuPG and YubiKey Manager name it.
pub fn algorithm_name(key: &PublicKey) -> String {
    let curve = |curve: &Curve| match curve {
        Curve::Ed25519 => "Ed25519".to_string(),
        Curve::Cv25519 => "Curve25519".to_string(),
        Curve::NistP256 => "NIST P-256".to_string(),
        Curve::NistP384 => "NIST P-384".to_string(),
        Curve::NistP521 => "NIST P-521".to_string(),
        Curve::BrainpoolP256 => "brainpoolP256r1".to_string(),
        Curve::BrainpoolP384 => "brainpoolP384r1".to_string(),
        Curve::BrainpoolP512 => "brainpoolP512r1".to_string(),
        other => other.to_string(),
    };
    match key.mpis() {
        mpi::PublicKey::RSA { n, .. } => format!("RSA {}", n.bits()),
        mpi::PublicKey::EdDSA { curve: c, .. } | mpi::PublicKey::ECDSA { curve: c, .. } | mpi::PublicKey::ECDH { curve: c, .. } => curve(c),
        mpi::PublicKey::Ed25519 { .. } => "Ed25519".into(),
        mpi::PublicKey::X25519 { .. } => "X25519".into(),
        _ => key.pk_algo().to_string(),
    }
}

/// Why a security key could not be used, each with its sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardError {
    /// This build of Sioul reaches no security key.
    Unsupported,
    /// The system's smart card service does not answer: pcscd on Linux, the
    /// Smart Card service on Windows.
    NoService,
    /// No security key is plugged in, or not the one wanted.
    Absent,
    /// Another program holds the key for itself: GnuPG, most often (a
    /// "sharing violation").
    Busy,
    /// The PIN was wrong; tries left.
    WrongPin {
        /// Tries left.
        left: u8,
    },
    /// The PIN is locked, after three wrong ones.
    Blocked,
    /// The key waited for a touch that did not come.
    TouchMissed,
    /// The key went away in the middle.
    Removed,
    /// What is plugged in holds no OpenPGP keys.
    NotOpenPgp,
    /// The key no longer holds the keys Sioul knows: its fingerprints changed.
    OtherKeys,
    /// The key signed, but not with the key its certificate names.
    BadSignature,
    /// An algorithm Sioul does not use with a security key.
    Algorithm(String),
    /// Anything else, as the system said it.
    Other(String),
}

/// What the window offers after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy {
    /// Nothing but to go on without the key.
    None,
    /// "Try again".
    Again,
    /// "Let GnuPG release it", then try again.
    Release,
    /// The PIN asked again.
    Pin,
}

impl CardError {
    /// The failure in words, with what to do.
    pub fn sentence(&self, tr: &Translator, purpose: Purpose) -> String {
        match self {
            CardError::Unsupported => tr.text("seckey-unsupported", None),
            CardError::NoService => say(tr, "seckey-no-service", &[("hint", no_service_hint(tr))]),
            CardError::Absent => tr.text(
                match purpose {
                    Purpose::Sign => "seckey-absent-sign",
                    Purpose::Open => "seckey-absent-open",
                    Purpose::Read => "seckey-absent",
                },
                None,
            ),
            CardError::Busy => tr.text("seckey-busy", None),
            CardError::WrongPin { left: 0 } | CardError::Blocked => tr.text("seckey-blocked", None),
            CardError::WrongPin { left } => tr.text("seckey-wrong-pin", Some(&tr.counted(usize::from(*left)))),
            CardError::TouchMissed => tr.text("seckey-touch-missed", None),
            CardError::Removed => tr.text(if purpose == Purpose::Sign { "seckey-removed-sign" } else { "seckey-removed" }, None),
            CardError::NotOpenPgp => tr.text("seckey-not-openpgp", None),
            CardError::OtherKeys => tr.text("seckey-other-keys", None),
            CardError::BadSignature => tr.text("seckey-bad-signature", None),
            CardError::Algorithm(algorithm) => say(tr, "seckey-algorithm", &[("algorithm", algorithm.clone())]),
            CardError::Other(detail) => say(tr, "seckey-other", &[("detail", detail.clone())]),
        }
    }

    /// What the window offers next.
    pub fn remedy(&self) -> Remedy {
        match self {
            CardError::Busy => Remedy::Release,
            CardError::WrongPin { left } if *left > 0 => Remedy::Pin,
            CardError::NoService | CardError::Absent | CardError::TouchMissed | CardError::Removed | CardError::Other(_) => Remedy::Again,
            _ => Remedy::None,
        }
    }
}

impl std::fmt::Display for CardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CardError {}

fn say(tr: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = crate::i18n::args();
    for (name, value) in pairs {
        args.set(*name, value.clone());
    }
    tr.text(id, Some(&args))
}

/// How to start the smart card service, for the system Sioul runs on.
fn no_service_hint(tr: &Translator) -> String {
    let id = if cfg!(windows) {
        "seckey-hint-windows"
    } else if cfg!(target_os = "macos") {
        "seckey-hint-macos"
    } else if Path::new("/.flatpak-info").exists() {
        "seckey-hint-flatpak"
    } else {
        hint_for_os_release(&std::fs::read_to_string("/etc/os-release").unwrap_or_default())
    };
    tr.text(id, None)
}

/// The hint for a Linux system, by its `/etc/os-release` (ID, then ID_LIKE).
fn hint_for_os_release(text: &str) -> &'static str {
    let value = |name: &str| text.lines().find_map(|l| l.strip_prefix(name)?.strip_prefix('=')).map(|v| v.trim_matches('"').to_ascii_lowercase()).unwrap_or_default();
    let family = format!("{} {}", value("ID"), value("ID_LIKE"));
    let has = |words: &[&str]| family.split_whitespace().any(|w| words.contains(&w));
    if has(&["fedora", "rhel", "centos"]) {
        "seckey-hint-fedora"
    } else if has(&["debian", "ubuntu"]) {
        "seckey-hint-debian"
    } else if has(&["arch"]) {
        "seckey-hint-arch"
    } else if has(&["suse", "opensuse", "opensuse-leap", "opensuse-tumbleweed"]) {
        "seckey-hint-suse"
    } else {
        "seckey-hint-linux"
    }
}

/// What the card signs: a digest as it is (EdDSA, ECDSA), or one the card
/// wraps in a DigestInfo before signing (RSA, specification §7.2.10.2).
#[derive(Debug, Clone, Copy)]
pub enum ToSign<'a> {
    /// EdDSA and ECDSA: the digest itself (cut to the curve's size for ECDSA).
    Digest(&'a [u8]),
    /// RSA: the digest and its hash.
    Rsa {
        /// SHA-256, SHA-384 or SHA-512.
        hash: HashAlgorithm,
        /// The digest.
        digest: &'a [u8],
    },
}

/// What the card opens (specification §7.2.11): RSA's ciphertext, giving the
/// session key; or, for ECDH, the sender's ephemeral point, giving the shared
/// secret, from which Sioul unwraps the session key.
#[derive(Debug, Clone, Copy)]
pub enum ToDecipher<'a> {
    /// RSA's ciphertext.
    Rsa(&'a [u8]),
    /// The ephemeral point (Curve25519's without its 0x40).
    Ecdh(&'a [u8]),
}

/// A security key, open for one operation: what it says, its PIN, and its two
/// private operations. The opening and the closing are the [`Readers`]'.
pub trait Card {
    /// What it says without its PIN.
    fn info(&mut self) -> Result<CardInfo, CardError>;
    /// Gives it its PIN; a wrong one costs a try.
    fn verify(&mut self, pin: &Password, mode: PinMode) -> Result<(), CardError>;
    /// Signs with its signing key: r ‖ s for EdDSA and ECDSA, s for RSA.
    fn sign(&mut self, input: ToSign<'_>) -> Result<Vec<u8>, CardError>;
    /// Opens with its decryption key.
    fn decipher(&mut self, input: ToDecipher<'_>) -> Result<Protected, CardError>;
}

/// Whether [`Readers::visit`] goes on to the next key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visit {
    /// The next key, if any.
    Next,
    /// Done: the others stay closed.
    Done,
}

/// What a visit met.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Visited {
    /// Keys opened and visited.
    pub opened: usize,
    /// Keys another program held for itself: not opened.
    pub busy: usize,
}

/// Where Sioul finds security keys: the system's smart card service on a
/// computer, a phone's NFC or USB, a software stand-in in tests.
pub trait Readers: Send + Sync {
    /// The readers holding a key now, by name, without opening any: to see a
    /// key go away.
    fn present(&self) -> Result<Vec<String>, CardError>;
    /// Opens each key present in turn, in shared mode, and gives it to `visit`
    /// until `visit` is done; each is closed afterwards, which resets it.
    fn visit(&self, visit: &mut dyn FnMut(&mut dyn Card) -> Visit) -> Result<Visited, CardError>;
}

/// A build without a way to security keys: each use says so.
pub struct NoReaders;

impl Readers for NoReaders {
    fn present(&self) -> Result<Vec<String>, CardError> {
        Err(CardError::Unsupported)
    }

    fn visit(&self, _visit: &mut dyn FnMut(&mut dyn Card) -> Visit) -> Result<Visited, CardError> {
        Err(CardError::Unsupported)
    }
}

/// What every security key present says.
pub fn cards(readers: &dyn Readers) -> Result<Vec<CardInfo>, CardError> {
    let mut found = Vec::new();
    let mut failure = None;
    let visited = readers.visit(&mut |card| {
        match card.info() {
            Ok(info) => found.push(info),
            Err(e) => failure = Some(e),
        }
        Visit::Next
    })?;
    if found.is_empty() {
        return Err(if visited.busy > 0 {
            CardError::Busy
        } else if let Some(e) = failure {
            e
        } else {
            CardError::Absent
        });
    }
    Ok(found)
}

/// Opens the key `ident` for `work`, given what it says; Absent when it is not
/// there, Busy when another program holds a key Sioul could not open.
pub fn with_card<R>(readers: &dyn Readers, ident: &str, work: impl FnOnce(&mut dyn Card, &CardInfo) -> Result<R, CardError>) -> Result<R, CardError> {
    let mut work = Some(work);
    let mut result = None;
    let visited = readers.visit(&mut |card| {
        let Ok(info) = card.info() else { return Visit::Next };
        if !info.ident.eq_ignore_ascii_case(ident) {
            return Visit::Next;
        }
        if let Some(work) = work.take() {
            result = Some(work(card, &info));
        }
        Visit::Done
    })?;
    match result {
        Some(result) => result,
        None if visited.busy > 0 => Err(CardError::Busy),
        None => Err(CardError::Absent),
    }
}

/// What it takes to use a security key now: where to find it, its PIN, and
/// what to say when it waits for a touch.
pub struct CardAccess<'a> {
    /// Where security keys are.
    pub readers: &'a dyn Readers,
    /// The PIN, given or held.
    pub pin: &'a Password,
    /// Called when the key is about to wait for a touch.
    pub touch: &'a (dyn Fn() + Sync),
    /// What went wrong: Sequoia carries only its own errors.
    failure: Mutex<Option<CardError>>,
    /// The reader the key was in, once used.
    reader: Mutex<Option<String>>,
    /// The key took the PIN, whatever came after.
    accepted: std::sync::atomic::AtomicBool,
}

impl<'a> CardAccess<'a> {
    /// Access to the security keys of `readers`, with `pin`.
    pub fn new(readers: &'a dyn Readers, pin: &'a Password, touch: &'a (dyn Fn() + Sync)) -> CardAccess<'a> {
        CardAccess { readers, pin, touch, failure: Mutex::new(None), reader: Mutex::new(None), accepted: std::sync::atomic::AtomicBool::new(false) }
    }

    /// What went wrong at the last use, taken.
    pub fn failure(&self) -> Option<CardError> {
        self.failure.lock().ok().and_then(|mut f| f.take())
    }

    /// The reader the key was in when it worked: to forget the PIN when it goes.
    pub fn reader(&self) -> Option<String> {
        self.reader.lock().ok().and_then(|r| r.clone())
    }

    /// Whether the key took the PIN, even when what came after failed (a
    /// touch missed): the PIN may then be held, not asked again.
    pub fn pin_accepted(&self) -> bool {
        self.accepted.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Keeps `error` for the caller, and gives it back for Sequoia to carry.
    fn fail(&self, error: CardError) -> CardError {
        if let Ok(mut failure) = self.failure.lock() {
            *failure = Some(error.clone());
        }
        error
    }

    /// Opens the key `ident`, checks that its slot still holds `public`, gives
    /// the PIN in `mode`, says when it waits for a touch, and runs `work`.
    fn run<R>(&self, ident: &str, public: &PublicKey, mode: PinMode, work: impl FnOnce(&mut dyn Card) -> Result<R, CardError>) -> Result<R, CardError> {
        let fingerprint = public.fingerprint().to_hex();
        with_card(self.readers, ident, |card, info| {
            let slot = match mode {
                PinMode::Sign => info.sign.as_ref(),
                PinMode::Decrypt => info.decrypt.as_ref(),
            };
            let slot = slot.ok_or(CardError::OtherKeys)?;
            if !slot.fingerprint.eq_ignore_ascii_case(&fingerprint) {
                return Err(CardError::OtherKeys);
            }
            card.verify(self.pin, mode)?;
            self.accepted.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Ok(mut reader) = self.reader.lock() {
                *reader = Some(info.reader.clone());
            }
            if slot.touch.asked() {
                (self.touch)();
            }
            work(card)
        })
    }
}

/// Signs with the key of a security key, as Sequoia's `Signer`: the key is
/// opened when Sequoia asks for the signature, and each signature is checked
/// with the certificate's key before it is given back.
pub struct CardSigner<'a> {
    access: &'a CardAccess<'a>,
    ident: String,
    public: PublicKey,
}

impl<'a> CardSigner<'a> {
    /// Signs with `public`, the key of the security key `ident`.
    pub fn new(access: &'a CardAccess<'a>, ident: &str, public: PublicKey) -> CardSigner<'a> {
        CardSigner { access, ident: ident.to_string(), public }
    }

    fn sign_on_card(&self, hash: HashAlgorithm, digest: &[u8]) -> Result<mpi::Signature, CardError> {
        let input = match self.public.mpis() {
            mpi::PublicKey::RSA { .. } => match hash {
                HashAlgorithm::SHA256 | HashAlgorithm::SHA384 | HashAlgorithm::SHA512 => ToSign::Rsa { hash, digest },
                other => return Err(CardError::Algorithm(format!("RSA, {other}"))),
            },
            mpi::PublicKey::EdDSA { .. } | mpi::PublicKey::Ed25519 { .. } => ToSign::Digest(digest),
            mpi::PublicKey::ECDSA { curve, .. } => {
                let size = curve.field_size().map_err(|e| CardError::Algorithm(e.to_string()))?;
                ToSign::Digest(&digest[..digest.len().min(size)])
            }
            _ => return Err(CardError::Algorithm(algorithm_name(&self.public))),
        };
        let raw = self.access.run(&self.ident, &self.public, PinMode::Sign, |card| card.sign(input))?;
        let half = raw.len() / 2;
        let signature = match self.public.mpis() {
            mpi::PublicKey::RSA { .. } => mpi::Signature::RSA { s: mpi::MPI::new(&raw) },
            mpi::PublicKey::EdDSA { .. } if raw.len() == 64 => mpi::Signature::EdDSA { r: mpi::MPI::new(&raw[..32]), s: mpi::MPI::new(&raw[32..]) },
            mpi::PublicKey::Ed25519 { .. } => mpi::Signature::Ed25519 { s: Box::new(<[u8; 64]>::try_from(raw.as_slice()).map_err(|_| CardError::BadSignature)?) },
            mpi::PublicKey::ECDSA { .. } if raw.len() % 2 == 0 => mpi::Signature::ECDSA { r: mpi::MPI::new(&raw[..half]), s: mpi::MPI::new(&raw[half..]) },
            _ => return Err(CardError::BadSignature),
        };
        // A key that signs with another key never sends a broken signature.
        self.public.verify(&signature, hash, digest).map_err(|_| CardError::BadSignature)?;
        Ok(signature)
    }
}

impl crypto::Signer for CardSigner<'_> {
    fn public(&self) -> &PublicKey {
        &self.public
    }

    fn acceptable_hashes(&self) -> &[HashAlgorithm] {
        match self.public.mpis() {
            // The card wraps the digest in a DigestInfo it knows: SHA-2 only.
            mpi::PublicKey::RSA { .. } => &[HashAlgorithm::SHA512, HashAlgorithm::SHA384, HashAlgorithm::SHA256],
            _ => &[HashAlgorithm::SHA512, HashAlgorithm::SHA384, HashAlgorithm::SHA256, HashAlgorithm::SHA224],
        }
    }

    fn sign(&mut self, hash_algo: HashAlgorithm, digest: &[u8]) -> openpgp::Result<mpi::Signature> {
        self.sign_on_card(hash_algo, digest).map_err(|e| self.access.fail(e).into())
    }
}

/// Opens session keys with the key of a security key, as Sequoia's
/// `Decryptor`: the key is opened when Sequoia asks.
pub struct CardDecryptor<'a> {
    access: &'a CardAccess<'a>,
    ident: String,
    public: PublicKey,
}

impl<'a> CardDecryptor<'a> {
    /// Opens with `public`, the decryption key of the security key `ident`.
    pub fn new(access: &'a CardAccess<'a>, ident: &str, public: PublicKey) -> CardDecryptor<'a> {
        CardDecryptor { access, ident: ident.to_string(), public }
    }

    fn decrypt_on_card(&self, ciphertext: &mpi::Ciphertext, plaintext_len: Option<usize>) -> Result<SessionKey, CardError> {
        match (ciphertext, self.public.mpis()) {
            (mpi::Ciphertext::RSA { c }, mpi::PublicKey::RSA { .. }) => {
                let opened = self.access.run(&self.ident, &self.public, PinMode::Decrypt, |card| card.decipher(ToDecipher::Rsa(c.value())))?;
                Ok(SessionKey::from(&opened[..]))
            }
            (mpi::Ciphertext::ECDH { e, .. }, mpi::PublicKey::ECDH { curve, .. }) => {
                // Curve25519's point is sent with OpenPGP's 0x40 before it, which the card does not take.
                let point = match curve {
                    Curve::Cv25519 => e.value().strip_prefix(&[0x40]).ok_or_else(|| CardError::Other("malformed Curve25519 point".into()))?,
                    _ => e.value(),
                };
                let shared = self.access.run(&self.ident, &self.public, PinMode::Decrypt, |card| card.decipher(ToDecipher::Ecdh(point)))?;
                // Gnuk gives back 0x04 ‖ x ‖ y for NIST P-256: the shared secret is x.
                let shared = if *curve == Curve::NistP256 && shared.len() == 65 && shared[0] == 0x04 { Protected::from(&shared[1..33]) } else { shared };
                crypto::ecdh::decrypt_unwrap(&self.public, &shared, ciphertext, plaintext_len).map_err(|e| CardError::Other(e.to_string()))
            }
            _ => Err(CardError::Algorithm(algorithm_name(&self.public))),
        }
    }
}

impl crypto::Decryptor for CardDecryptor<'_> {
    fn public(&self) -> &PublicKey {
        &self.public
    }

    fn decrypt(&mut self, ciphertext: &mpi::Ciphertext, plaintext_len: Option<usize>) -> openpgp::Result<SessionKey> {
        self.decrypt_on_card(ciphertext, plaintext_len).map_err(|e| self.access.fail(e).into())
    }
}

/// How long a PIN stays in memory after its last use (the owner's choice: 15 minutes).
pub const PIN_QUIET: Duration = Duration::from_secs(15 * 60);

/// The PIN of one security key, held in memory only, encrypted there by
/// Sequoia (`Password`), for [`PIN_QUIET`] after its last use. Never on disk,
/// in the keyring or in a log. Forgotten when Sioul closes, when the key goes
/// away (on a computer), or on "Forget the PIN now".
#[derive(Default)]
pub struct PinMemory {
    held: Option<Held>,
}

struct Held {
    ident: String,
    pin: Password,
    used: Instant,
    reader: String,
}

impl PinMemory {
    /// Nothing held.
    pub const fn new() -> PinMemory {
        PinMemory { held: None }
    }

    /// Holds `pin` for the key `ident`, in `reader`, from `now`.
    pub fn keep(&mut self, ident: &str, pin: Password, reader: &str, now: Instant) {
        self.held = Some(Held { ident: ident.to_string(), pin, used: now, reader: reader.to_string() });
    }

    /// The PIN held for `ident`, when its quiet time is not over.
    pub fn get(&mut self, ident: &str, now: Instant) -> Option<Password> {
        self.expire(now);
        self.held.as_ref().filter(|h| h.ident.eq_ignore_ascii_case(ident)).map(|h| h.pin.clone())
    }

    /// The PIN of `ident` worked again: its quiet time starts again.
    pub fn used(&mut self, ident: &str, now: Instant) {
        if let Some(held) = self.held.as_mut().filter(|h| h.ident.eq_ignore_ascii_case(ident)) {
            held.used = now;
        }
    }

    /// Forgets the PIN.
    pub fn forget(&mut self) {
        self.held = None;
    }

    /// Forgets the PIN when its quiet time is over; whether it did.
    pub fn expire(&mut self, now: Instant) -> bool {
        let stale = self.held.as_ref().is_some_and(|h| now.saturating_duration_since(h.used) >= PIN_QUIET);
        if stale {
            self.held = None;
        }
        stale
    }

    /// Forgets the PIN when its key's reader is no longer among those `present`; whether it did.
    pub fn went_away(&mut self, present: &[String]) -> bool {
        let gone = self.held.as_ref().is_some_and(|h| !h.reader.is_empty() && !present.iter().any(|r| *r == h.reader));
        if gone {
            self.held = None;
        }
        gone
    }

    /// The key whose PIN is held, if any.
    pub fn holder(&self) -> Option<&str> {
        self.held.as_ref().map(|h| h.ident.as_str())
    }
}

/// A security key Sioul knows, in `pgp/cards.toml`: which keys it holds, the
/// certificate they belong to and where it came from. No secret.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownCard {
    /// "0006:12345678", as [`CardInfo::ident`].
    pub ident: String,
    /// Its maker's number.
    #[serde(default)]
    pub manufacturer: u16,
    /// Its serial number.
    #[serde(default)]
    pub serial: u32,
    /// The name written on it.
    #[serde(default)]
    pub holder: String,
    /// The fingerprint of its signing key; empty when it holds none.
    #[serde(default)]
    pub sign: String,
    /// The fingerprint of its decryption key; empty when it holds none.
    #[serde(default)]
    pub decrypt: String,
    /// The certificate both belong to, by its primary key's fingerprint.
    pub cert: String,
    /// Your addresses it signs for (decision 2: it alone signs for them).
    #[serde(default)]
    pub addresses: Vec<String>,
    /// Where its certificate came from: an address, or "file".
    #[serde(default)]
    pub source: String,
    /// When the certificate was checked against the key (Unix seconds).
    #[serde(default)]
    pub checked: i64,
    /// The most PIN tries the key was seen with: fewer are said.
    #[serde(default)]
    pub pin_tries_full: u8,
    /// Whether its signing key waits for a touch, as last read.
    #[serde(default)]
    pub touch_sign: Touch,
    /// Whether its decryption key waits for a touch, as last read.
    #[serde(default)]
    pub touch_decrypt: Touch,
    /// The PIN is asked for each signature.
    #[serde(default)]
    pub pin_each_signature: bool,
}

impl KnownCard {
    /// Whether it signs for `address`.
    pub fn serves(&self, address: &str) -> bool {
        self.addresses.iter().any(|a| a.eq_ignore_ascii_case(address.trim()))
    }

    /// "YubiKey 12 345 678".
    pub fn label(&self, tr: &Translator) -> String {
        label(self.manufacturer, self.serial, tr)
    }

    /// What the key said now, kept: touch settings, the PIN for each
    /// signature, the most tries seen.
    pub fn update(&mut self, info: &CardInfo) {
        self.holder = info.holder.clone();
        self.touch_sign = info.sign.as_ref().map(|s| s.touch).unwrap_or_default();
        self.touch_decrypt = info.decrypt.as_ref().map(|s| s.touch).unwrap_or_default();
        self.pin_each_signature = info.pin_each_signature;
        self.pin_tries_full = self.pin_tries_full.max(info.pin_tries).max(3);
    }

    /// The tries left, said when fewer than the most seen: "2 tries left."; else nothing.
    pub fn tries_line(&self, left: u8, tr: &Translator) -> Option<String> {
        (left < self.pin_tries_full.max(3)).then(|| tr.text("seckey-tries-left", Some(&tr.counted(usize::from(left)))))
    }
}

#[derive(Default, Serialize, Deserialize)]
struct CardsFile {
    #[serde(default, rename = "card")]
    cards: Vec<KnownCard>,
}

/// Where the known security keys are listed.
pub fn cards_path() -> PathBuf {
    data_dir().join("pgp").join("cards.toml")
}

/// The security keys Sioul knows.
pub fn known_cards() -> Vec<KnownCard> {
    read_known(&cards_path())
}

fn read_known(path: &Path) -> Vec<KnownCard> {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str::<CardsFile>(&text).ok()).map(|f| f.cards).unwrap_or_default()
}

/// Changes the list of known security keys: read, changed, written, one writer at a time.
pub fn change_known<T>(change: impl FnOnce(&mut Vec<KnownCard>) -> T) -> Result<T, String> {
    change_known_at(&cards_path(), change)
}

fn change_known_at<T>(path: &Path, change: impl FnOnce(&mut Vec<KnownCard>) -> T) -> Result<T, String> {
    crate::filelock::with_lock(path, || {
        let mut cards = read_known(path);
        let result = change(&mut cards);
        let text = toml::to_string(&CardsFile { cards }).map_err(|e| e.to_string())?;
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(fail)?;
        }
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, text).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)?;
        Ok(result)
    })
}

/// A certificate checked against a security key: it belongs to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertCheck {
    /// The certificate's fingerprint.
    pub cert: String,
    /// The addresses its user IDs name, lower case.
    pub addresses: Vec<String>,
    /// When it stops being valid, its primary key or the keys on the
    /// security key, the earliest (Unix seconds); None when it does not expire.
    pub expires: Option<i64>,
    /// It already expired.
    pub expired: bool,
    /// The security key signs for it.
    pub signs: bool,
    /// The security key opens what is encrypted to it.
    pub decrypts: bool,
}

/// Why a certificate is not the one of a security key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// The security key holds no key to sign or to open with.
    Empty,
    /// The certificate lacks a key the security key holds.
    Missing,
    /// The certificate, or a key of it, was revoked.
    Revoked,
    /// The key in the security key's signing slot may not sign, by its certificate.
    NotForSigning,
    /// The key in the decryption slot may not encrypt, by its certificate.
    NotForEncryption,
    /// The fingerprints agree, but the public keys differ: the fingerprints on
    /// the key are only data written to it.
    OtherPublicKey,
    /// Sequoia's standard policy refuses the certificate (its reason).
    Invalid(String),
}

impl Mismatch {
    /// The reason in words.
    pub fn sentence(&self, tr: &Translator) -> String {
        match self {
            Mismatch::Empty => tr.text("seckey-mismatch-empty", None),
            Mismatch::Missing | Mismatch::OtherPublicKey => tr.text("seckey-mismatch", None),
            Mismatch::Revoked => tr.text("seckey-mismatch-revoked", None),
            Mismatch::NotForSigning | Mismatch::NotForEncryption => tr.text("seckey-mismatch-use", None),
            Mismatch::Invalid(why) => say(tr, "seckey-mismatch-invalid", &[("detail", why.clone())]),
        }
    }
}

/// Whether `cert` is the certificate of the security key `card`: each key on
/// the card is in it, alive or expired but not revoked, allowed to do what its
/// slot does, and its public part is the card's own.
pub fn check(cert: &Cert, card: &CardInfo) -> Result<CertCheck, Mismatch> {
    let policy = StandardPolicy::new();
    let valid = cert.with_policy(&policy, None).map_err(|e| Mismatch::Invalid(e.to_string()))?;
    if matches!(valid.revocation_status(), openpgp::types::RevocationStatus::Revoked(_)) {
        return Err(Mismatch::Revoked);
    }
    if card.sign.is_none() && card.decrypt.is_none() {
        return Err(Mismatch::Empty);
    }
    let unix = |t: SystemTime| t.duration_since(SystemTime::UNIX_EPOCH).ok().and_then(|d| i64::try_from(d.as_secs()).ok());
    let mut expires: Option<i64> = valid.primary_key().key_expiration_time().and_then(unix);
    let mut earliest = |at: Option<i64>| {
        if let Some(at) = at {
            expires = Some(expires.map_or(at, |e| e.min(at)));
        }
    };
    let mut alive = valid.alive().is_ok();
    for (slot, signing) in [(&card.sign, true), (&card.decrypt, false)] {
        let Some(slot) = slot else { continue };
        let key = valid.keys().find(|k| k.key().fingerprint().to_hex().eq_ignore_ascii_case(&slot.fingerprint)).ok_or(Mismatch::Missing)?;
        if matches!(key.revocation_status(), openpgp::types::RevocationStatus::Revoked(_)) {
            return Err(Mismatch::Revoked);
        }
        let allowed = if signing { key.for_signing() } else { key.for_transport_encryption() || key.for_storage_encryption() };
        if !allowed {
            return Err(if signing { Mismatch::NotForSigning } else { Mismatch::NotForEncryption });
        }
        if let Some(public) = &slot.public
            && !same_public(key.key(), public)
        {
            return Err(Mismatch::OtherPublicKey);
        }
        earliest(key.key_expiration_time().and_then(unix));
        alive &= key.alive().is_ok();
    }
    let mut addresses: Vec<String> = valid.userids().filter_map(|u| u.userid().email().ok().flatten().map(|e| e.to_ascii_lowercase())).collect();
    addresses.sort();
    addresses.dedup();
    let now = unix(SystemTime::now()).unwrap_or(0);
    Ok(CertCheck {
        cert: cert.fingerprint().to_hex(),
        addresses,
        expires,
        expired: !alive || expires.is_some_and(|at| at <= now),
        signs: card.sign.is_some(),
        decrypts: card.decrypt.is_some(),
    })
}

/// The certificate of the security key `card` among `bytes` (armored or
/// binary, one certificate or several), with what it is; else why none is.
pub fn certificate_in(bytes: &[u8], card: &CardInfo) -> Result<(Cert, CertCheck), Mismatch> {
    use openpgp::parse::Parse;
    let mut why = Mismatch::Missing;
    let parser = openpgp::cert::CertParser::from_bytes(bytes).map_err(|e| Mismatch::Invalid(e.to_string()))?;
    for cert in parser.filter_map(Result::ok) {
        match check(&cert, card) {
            Ok(found) => return Ok((cert.strip_secret_key_material(), found)),
            // A certificate with the key's fingerprints says more than a stranger's.
            Err(Mismatch::Missing) => {}
            Err(other) => why = other,
        }
    }
    Err(why)
}

/// Makes the security key `card` yours: its certificate kept with your keys
/// (public parts only), and the key in `cards.toml`, signing for `addresses`;
/// `source` says where the certificate came from. Gives what is kept.
pub fn adopt(card: &CardInfo, cert: &Cert, source: &str, addresses: Vec<String>, now: i64) -> Result<KnownCard, String> {
    crate::pgp::keep_card_certificate(cert)?;
    let mut known = KnownCard {
        ident: card.ident.clone(),
        manufacturer: card.manufacturer,
        serial: card.serial,
        sign: card.sign.as_ref().map(|s| s.fingerprint.clone()).unwrap_or_default(),
        decrypt: card.decrypt.as_ref().map(|s| s.fingerprint.clone()).unwrap_or_default(),
        cert: cert.fingerprint().to_hex(),
        addresses: addresses.into_iter().map(|a| a.trim().to_ascii_lowercase()).collect(),
        source: source.to_string(),
        checked: now,
        ..KnownCard::default()
    };
    known.update(card);
    let kept = known.clone();
    change_known(|cards| {
        cards.retain(|c| !c.ident.eq_ignore_ascii_case(&kept.ident));
        cards.push(kept);
    })?;
    Ok(known)
}

/// Stops using the security key `ident`: it leaves `cards.toml`, and its
/// certificate leaves your keys for `pgp/removed` (never deleted).
pub fn forget(ident: &str) -> Result<(), String> {
    let cert = change_known(|cards| {
        let cert = cards.iter().find(|c| c.ident.eq_ignore_ascii_case(ident)).map(|c| c.cert.clone());
        cards.retain(|c| !c.ident.eq_ignore_ascii_case(ident));
        cert
    })?;
    match cert {
        Some(cert) => crate::pgp::remove(&cert),
        None => Ok(()),
    }
}

/// Whether a certificate's key is the card's: the same public part.
fn same_public(key: &PublicKey, card: &CardPublic) -> bool {
    // OpenPGP puts 0x40 before a Curve25519 point (Ed25519, Cv25519); a card does not.
    fn native(point: &[u8]) -> &[u8] {
        if point.len() == 33 && point[0] == 0x40 { &point[1..] } else { point }
    }
    fn number(bytes: &[u8]) -> &[u8] {
        let zeros = bytes.iter().take_while(|&&b| b == 0).count();
        &bytes[zeros..]
    }
    match (key.mpis(), card) {
        (mpi::PublicKey::RSA { e, n }, CardPublic::Rsa { n: card_n, e: card_e }) => number(n.value()) == number(card_n) && number(e.value()) == number(card_e),
        (mpi::PublicKey::EdDSA { q, .. } | mpi::PublicKey::ECDSA { q, .. } | mpi::PublicKey::ECDH { q, .. }, CardPublic::Ecc { point }) => native(q.value()) == native(point),
        (mpi::PublicKey::Ed25519 { a }, CardPublic::Ecc { point }) => a[..] == *native(point),
        (mpi::PublicKey::X25519 { u }, CardPublic::Ecc { point }) => u[..] == *native(point),
        _ => false,
    }
}

/// When a certificate stops serving a security key: the earliest expiry of
/// its primary key and of its keys named by `fingerprints` (Unix seconds);
/// None when none of them expires, or the certificate is not valid at all.
pub fn expiry_of(cert: &Cert, fingerprints: &[&str]) -> Option<i64> {
    let policy = StandardPolicy::new();
    let valid = cert.with_policy(&policy, None).ok()?;
    let unix = |t: SystemTime| t.duration_since(SystemTime::UNIX_EPOCH).ok().and_then(|d| i64::try_from(d.as_secs()).ok());
    let keys = valid.keys().filter(|k| fingerprints.iter().any(|f| !f.is_empty() && k.key().fingerprint().to_hex().eq_ignore_ascii_case(f))).filter_map(|k| k.key_expiration_time().and_then(unix));
    valid.primary_key().key_expiration_time().and_then(unix).into_iter().chain(keys).min()
}

/// "3 March 2026", "3 mars 2026".
pub fn full_date(tr: &Translator, at: i64) -> String {
    let Ok(stamp) = jiff::Timestamp::from_second(at) else { return String::new() };
    let date = stamp.to_zoned(jiff::tz::TimeZone::system()).date();
    format!("{} {}", tr.day_month(date), date.year())
}

/// What the certificate's expiry says now: expired (and how to renew), soon
/// (within a month), or valid until a date; nothing when it does not expire.
pub fn expiry_line(tr: &Translator, expires: Option<i64>, fingerprint: &str, now: i64) -> Option<(String, bool)> {
    let at = expires?;
    let date = full_date(tr, at);
    let pairs = [("date", date), ("fingerprint", fingerprint.to_string())];
    Some(if at <= now {
        (say(tr, "seckey-expired", &pairs), true)
    } else if at - now <= 31 * 24 * 3600 {
        (say(tr, "seckey-expires-soon", &pairs), true)
    } else {
        (say(tr, "seckey-valid-until", &pairs), false)
    })
}

/// A security key made of software keys, behind the same traits as a real one,
/// for tests: Sioul itself never makes one. It counts its PIN tries, forgets a
/// verified PIN when closed, asks the PIN again after each signature when told
/// to, and can fail as a real key does ([`SoftCard::fault`]).
pub struct SoftCard {
    /// What it says.
    pub info: CardInfo,
    signer: Option<crypto::KeyPair>,
    decryptor: Option<crypto::KeyPair>,
    pin: Vec<u8>,
    /// X25519 for its Curve25519 key: Sequoia keeps its own private, so a test lends one.
    x25519: Option<fn(&[u8; 32], &[u8; 32]) -> [u8; 32]>,
    verified_sign: bool,
    verified_decrypt: bool,
    /// What its next signature or opening does instead of working: a touch
    /// missed, the key pulled out.
    pub fault: Option<CardError>,
    /// Signatures and openings it made.
    pub operations: usize,
    /// How long a person takes to touch it, when a key asks for a touch: none in
    /// tests; a few seconds in the demo, so that "Touch your security key" shows.
    pub touch_delay: Duration,
}

impl SoftCard {
    /// A key named `ident` holding the secret keys of `cert` (its first
    /// signing key and its first encryption key), with `pin`.
    pub fn new(ident: &str, cert: &Cert, pin: &str) -> SoftCard {
        let policy = StandardPolicy::new();
        let pair = |signing: bool| {
            cert.keys()
                .unencrypted_secret()
                .with_policy(&policy, None)
                .find(|k| if signing { k.for_signing() } else { k.for_transport_encryption() })
                .and_then(|k| k.key().clone().into_keypair().ok())
        };
        let (signer, decryptor) = (pair(true), pair(false));
        let slot = |pair: &Option<crypto::KeyPair>| {
            pair.as_ref().map(|p| SlotInfo {
                fingerprint: p.public().fingerprint().to_hex(),
                created: openpgp::types::Timestamp::try_from(p.public().creation_time()).map(u32::from).unwrap_or(0),
                algorithm: algorithm_name(p.public()),
                touch: Touch::Off,
                public: card_public(p.public()),
            })
        };
        let (manufacturer, serial) = ident.split_once(':').map_or((0, 0), |(m, s)| (u16::from_str_radix(m, 16).unwrap_or(0), u32::from_str_radix(s, 16).unwrap_or(0)));
        SoftCard {
            info: CardInfo {
                ident: ident.to_string(),
                manufacturer,
                serial,
                holder: String::new(),
                url: String::new(),
                sign: slot(&signer),
                decrypt: slot(&decryptor),
                pin_tries: 3,
                reset_tries: 0,
                admin_tries: 3,
                pin_each_signature: false,
                signatures: 0,
                reader: format!("Soft reader {ident}"),
            },
            signer,
            decryptor,
            pin: pin.as_bytes().to_vec(),
            x25519: None,
            verified_sign: false,
            verified_decrypt: false,
            fault: None,
            operations: 0,
            touch_delay: Duration::ZERO,
        }
    }

    /// A new key for tests (Curve25519: its primary key signs, a subkey
    /// decrypts), named by `userids`, on a software key `ident` with `pin`;
    /// and its certificate, armored, public parts only. For the crates that
    /// do not use Sequoia themselves.
    pub fn generate(ident: &str, userids: &[&str], pin: &str) -> Result<(SoftCard, Vec<u8>), String> {
        use openpgp::serialize::SerializeInto;
        let mut builder = CertBuilder::new().set_cipher_suite(CipherSuite::Cv25519).set_primary_key_flags(openpgp::types::KeyFlags::empty().set_certification().set_signing()).add_transport_encryption_subkey();
        for userid in userids {
            builder = builder.add_userid(*userid);
        }
        let (cert, _) = builder.generate().map_err(|e| e.to_string())?;
        let public = cert.clone().strip_secret_key_material().armored().to_vec().map_err(|e| e.to_string())?;
        Ok((SoftCard::new(ident, &cert, pin), public))
    }

    /// Lends it X25519, for its Curve25519 decryption key.
    pub fn with_x25519(mut self, x25519: fn(&[u8; 32], &[u8; 32]) -> [u8; 32]) -> SoftCard {
        self.x25519 = Some(x25519);
        self
    }

    /// Closed: a real key is reset, and its PIN must be given again.
    fn close(&mut self) {
        self.verified_sign = false;
        self.verified_decrypt = false;
    }
}

/// The card's form of a public key (its point without OpenPGP's 0x40).
fn card_public(key: &PublicKey) -> Option<CardPublic> {
    let native = |q: &mpi::MPI| {
        let v = q.value();
        if v.len() == 33 && v[0] == 0x40 { v[1..].to_vec() } else { v.to_vec() }
    };
    match key.mpis() {
        mpi::PublicKey::RSA { e, n } => Some(CardPublic::Rsa { n: n.value().to_vec(), e: e.value().to_vec() }),
        mpi::PublicKey::EdDSA { q, .. } | mpi::PublicKey::ECDSA { q, .. } | mpi::PublicKey::ECDH { q, .. } => Some(CardPublic::Ecc { point: native(q) }),
        mpi::PublicKey::Ed25519 { a } => Some(CardPublic::Ecc { point: a.to_vec() }),
        _ => None,
    }
}

impl Card for SoftCard {
    fn info(&mut self) -> Result<CardInfo, CardError> {
        Ok(self.info.clone())
    }

    fn verify(&mut self, pin: &Password, mode: PinMode) -> Result<(), CardError> {
        if self.info.pin_tries == 0 {
            return Err(CardError::Blocked);
        }
        if pin.map(|p| p[..] == self.pin[..]) {
            self.info.pin_tries = 3;
            match mode {
                PinMode::Sign => self.verified_sign = true,
                PinMode::Decrypt => self.verified_decrypt = true,
            }
            Ok(())
        } else {
            self.info.pin_tries -= 1;
            Err(CardError::WrongPin { left: self.info.pin_tries })
        }
    }

    fn sign(&mut self, input: ToSign<'_>) -> Result<Vec<u8>, CardError> {
        if !self.verified_sign {
            return Err(CardError::Other("security status not satisfied".into()));
        }
        if let Some(fault) = self.fault.take() {
            return Err(fault);
        }
        if self.info.sign.as_ref().is_some_and(|s| s.touch.asked()) {
            std::thread::sleep(self.touch_delay);
        }
        let pair = self.signer.as_mut().ok_or(CardError::NotOpenPgp)?;
        let (hash, digest) = match input {
            ToSign::Digest(digest) => (HashAlgorithm::SHA512, digest),
            ToSign::Rsa { hash, digest } => (hash, digest),
        };
        let signature = crypto::Signer::sign(pair, hash, digest).map_err(|e| CardError::Other(e.to_string()))?;
        let pad = |m: &mpi::MPI, size: usize| m.value_padded(size).map(|v| v.to_vec()).map_err(|e| CardError::Other(e.to_string()));
        let raw = match (&signature, pair.public().mpis()) {
            (mpi::Signature::RSA { s }, mpi::PublicKey::RSA { n, .. }) => pad(s, n.value().len())?,
            (mpi::Signature::EdDSA { r, s }, _) => [pad(r, 32)?, pad(s, 32)?].concat(),
            (mpi::Signature::Ed25519 { s }, _) => s.to_vec(),
            (mpi::Signature::ECDSA { r, s }, mpi::PublicKey::ECDSA { curve, .. }) => {
                let size = curve.field_size().map_err(|e| CardError::Other(e.to_string()))?;
                [pad(r, size)?, pad(s, size)?].concat()
            }
            _ => return Err(CardError::Algorithm(algorithm_name(pair.public()))),
        };
        if self.info.pin_each_signature {
            self.verified_sign = false;
        }
        self.info.signatures += 1;
        self.operations += 1;
        Ok(raw)
    }

    fn decipher(&mut self, input: ToDecipher<'_>) -> Result<Protected, CardError> {
        if !self.verified_decrypt {
            return Err(CardError::Other("security status not satisfied".into()));
        }
        if let Some(fault) = self.fault.take() {
            return Err(fault);
        }
        if self.info.decrypt.as_ref().is_some_and(|s| s.touch.asked()) {
            std::thread::sleep(self.touch_delay);
        }
        let pair = self.decryptor.as_mut().ok_or(CardError::NotOpenPgp)?;
        let opened = match (input, pair.public().mpis()) {
            (ToDecipher::Rsa(c), mpi::PublicKey::RSA { .. }) => {
                let session = crypto::Decryptor::decrypt(pair, &mpi::Ciphertext::RSA { c: mpi::MPI::new(c) }, None).map_err(|e| CardError::Other(e.to_string()))?;
                Protected::from(&session[..])
            }
            (ToDecipher::Ecdh(point), mpi::PublicKey::ECDH { curve: Curve::Cv25519, .. }) => {
                let x25519 = self.x25519.ok_or_else(|| CardError::Algorithm("Curve25519".into()))?;
                // Sequoia keeps the scalar reversed, as OpenPGP does; the card keeps X25519's own order.
                let scalar = pair.secret().map(|secret| match secret {
                    mpi::SecretKeyMaterial::ECDH { scalar } => {
                        let mut native = scalar.value_padded(32).to_vec();
                        native.reverse();
                        <[u8; 32]>::try_from(native.as_slice()).ok()
                    }
                    _ => None,
                });
                let scalar = scalar.ok_or_else(|| CardError::Other("no Curve25519 scalar".into()))?;
                let point = <[u8; 32]>::try_from(point).map_err(|_| CardError::Other("malformed point".into()))?;
                Protected::from(&x25519(&scalar, &point)[..])
            }
            _ => return Err(CardError::Algorithm(algorithm_name(pair.public()))),
        };
        self.operations += 1;
        Ok(opened)
    }
}

/// Readers holding [`SoftCard`]s, for tests: none, some, held by another
/// program, or no smart card service at all.
#[derive(Default)]
pub struct SoftReaders {
    /// The keys plugged in.
    pub cards: Mutex<Vec<SoftCard>>,
    /// Keys another program holds: present, never opened.
    pub busy: usize,
    /// The smart card service answers with this instead.
    pub failure: Option<CardError>,
}

impl SoftReaders {
    /// Readers holding `cards`.
    pub fn with(cards: Vec<SoftCard>) -> SoftReaders {
        SoftReaders { cards: Mutex::new(cards), busy: 0, failure: None }
    }

    /// Runs `f` on the key `ident`, to see or change it between operations.
    pub fn card<R>(&self, ident: &str, f: impl FnOnce(&mut SoftCard) -> R) -> Option<R> {
        let mut cards = self.cards.lock().ok()?;
        cards.iter_mut().find(|c| c.info.ident == ident).map(f)
    }
}

impl Readers for SoftReaders {
    fn present(&self) -> Result<Vec<String>, CardError> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        Ok(self.cards.lock().map(|c| c.iter().map(|c| c.info.reader.clone()).collect()).unwrap_or_default())
    }

    fn visit(&self, visit: &mut dyn FnMut(&mut dyn Card) -> Visit) -> Result<Visited, CardError> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let mut cards = self.cards.lock().map_err(|_| CardError::Other("poisoned".into()))?;
        let mut visited = Visited { opened: 0, busy: self.busy };
        for card in cards.iter_mut() {
            visited.opened += 1;
            let next = visit(card);
            card.close();
            if next == Visit::Done {
                break;
            }
        }
        Ok(visited)
    }
}

#[cfg(test)]
pub(crate) mod x25519 {
    //! X25519 (RFC 7748), after TweetNaCl's `crypto_scalarmult` (public
    //! domain): for the software stand-in's Curve25519 key in tests only, since
    //! Sequoia keeps its own private. Not constant-time; never used by Sioul.

    type Field = [i64; 16];

    const A24: Field = [0xDB41, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    fn carry(o: &mut Field) {
        for i in 0..16 {
            o[i] += 1 << 16;
            let c = o[i] >> 16;
            if i < 15 {
                o[i + 1] += c - 1;
            } else {
                o[0] += 38 * (c - 1);
            }
            o[i] -= c << 16;
        }
    }

    fn swap(p: &mut Field, q: &mut Field, bit: i64) {
        let c = !(bit - 1);
        for i in 0..16 {
            let t = c & (p[i] ^ q[i]);
            p[i] ^= t;
            q[i] ^= t;
        }
    }

    fn pack(n: &Field) -> [u8; 32] {
        let mut t = *n;
        carry(&mut t);
        carry(&mut t);
        carry(&mut t);
        for _ in 0..2 {
            let mut m = [0i64; 16];
            m[0] = t[0] - 0xffed;
            for i in 1..15 {
                m[i] = t[i] - 0xffff - ((m[i - 1] >> 16) & 1);
                m[i - 1] &= 0xffff;
            }
            m[15] = t[15] - 0x7fff - ((m[14] >> 16) & 1);
            let b = (m[15] >> 16) & 1;
            m[14] &= 0xffff;
            swap(&mut t, &mut m, 1 - b);
        }
        let mut o = [0u8; 32];
        for i in 0..16 {
            o[2 * i] = (t[i] & 0xff) as u8;
            o[2 * i + 1] = ((t[i] >> 8) & 0xff) as u8;
        }
        o
    }

    fn unpack(n: &[u8; 32]) -> Field {
        let mut o = [0i64; 16];
        for i in 0..16 {
            o[i] = i64::from(n[2 * i]) + (i64::from(n[2 * i + 1]) << 8);
        }
        o[15] &= 0x7fff;
        o
    }

    fn add(a: &Field, b: &Field) -> Field {
        std::array::from_fn(|i| a[i] + b[i])
    }

    fn sub(a: &Field, b: &Field) -> Field {
        std::array::from_fn(|i| a[i] - b[i])
    }

    fn mul(a: &Field, b: &Field) -> Field {
        let mut t = [0i64; 31];
        for i in 0..16 {
            for j in 0..16 {
                t[i + j] += a[i] * b[j];
            }
        }
        for i in 0..15 {
            t[i] += 38 * t[i + 16];
        }
        let mut o: Field = std::array::from_fn(|i| t[i]);
        carry(&mut o);
        carry(&mut o);
        o
    }

    fn invert(i: &Field) -> Field {
        let mut c = *i;
        for a in (0..=253).rev() {
            c = mul(&c, &c);
            if a != 2 && a != 4 {
                c = mul(&c, i);
            }
        }
        c
    }

    /// `scalar` times the point `u`.
    pub(crate) fn x25519(scalar: &[u8; 32], u: &[u8; 32]) -> [u8; 32] {
        let mut z = *scalar;
        z[31] = (z[31] & 127) | 64;
        z[0] &= 248;
        let x = unpack(u);
        let (mut a, mut b, mut c, mut d) = ([0i64; 16], x, [0i64; 16], [0i64; 16]);
        a[0] = 1;
        d[0] = 1;
        for i in (0..=254usize).rev() {
            let r = i64::from((z[i >> 3] >> (i & 7)) & 1);
            swap(&mut a, &mut b, r);
            swap(&mut c, &mut d, r);
            let mut e = add(&a, &c);
            a = sub(&a, &c);
            c = add(&b, &d);
            b = sub(&b, &d);
            d = mul(&e, &e);
            let f = mul(&a, &a);
            a = mul(&c, &a);
            c = mul(&b, &e);
            e = add(&a, &c);
            a = sub(&a, &c);
            b = mul(&a, &a);
            c = sub(&d, &f);
            a = mul(&c, &A24);
            a = add(&a, &d);
            c = mul(&c, &a);
            a = mul(&d, &f);
            d = mul(&b, &x);
            b = mul(&e, &e);
            swap(&mut a, &mut b, r);
            swap(&mut c, &mut d, r);
        }
        pack(&mul(&a, &invert(&c)))
    }

    #[test]
    fn rfc_7748_vector() {
        let hex = |s: &str| -> [u8; 32] { std::array::from_fn(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()) };
        let out = x25519(&hex("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"), &hex("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c"));
        assert_eq!(out, hex("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpgp::cert::prelude::*;
    use openpgp::crypto::Signer as _;
    use openpgp::types::KeyFlags;

    /// A key as the owner's card holds it: the primary signs, a Curve25519 subkey decrypts.
    fn primary_signs(address: &str) -> Cert {
        CertBuilder::new()
            .add_userid(format!("Card Holder <{address}>"))
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_primary_key_flags(KeyFlags::empty().set_certification().set_signing())
            .add_transport_encryption_subkey()
            .generate()
            .unwrap()
            .0
    }

    fn touchless(_: &[u8; 32], _: &[u8; 32]) -> [u8; 32] {
        [0; 32]
    }

    #[test]
    fn signs_and_checks_each_signature() {
        let cert = primary_signs("holder@example.org");
        let readers = SoftReaders::with(vec![SoftCard::new("0006:12345678", &cert, "123456")]);
        let pin = Password::from("123456");
        let touched = std::sync::atomic::AtomicUsize::new(0);
        let touch = || {
            touched.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        };
        let access = CardAccess::new(&readers, &pin, &touch);
        let public = cert.primary_key().key().clone().role_into_unspecified();
        let mut signer = CardSigner::new(&access, "0006:12345678", public.clone());
        let digest = [7u8; 64];
        let signature = signer.sign(HashAlgorithm::SHA512, &digest).unwrap();
        public.verify(&signature, HashAlgorithm::SHA512, &digest).unwrap();
        assert_eq!(touched.load(std::sync::atomic::Ordering::Relaxed), 0, "no touch asked: none said");
        assert_eq!(access.reader().as_deref(), Some("Soft reader 0006:12345678"));
        // Touch on: said once, just before the key waits.
        readers.card("0006:12345678", |c| c.info.sign.as_mut().unwrap().touch = Touch::On);
        signer.sign(HashAlgorithm::SHA512, &digest).unwrap();
        assert_eq!(touched.load(std::sync::atomic::Ordering::Relaxed), 1);
        // Not touched in time: said as such, and nothing comes back.
        readers.card("0006:12345678", |c| c.fault = Some(CardError::TouchMissed));
        let fresh = CardAccess::new(&readers, &pin, &touch);
        assert!(CardSigner::new(&fresh, "0006:12345678", public.clone()).sign(HashAlgorithm::SHA512, &digest).is_err());
        assert_eq!(fresh.failure(), Some(CardError::TouchMissed));
        // The PIN was taken before the touch was missed: it may be held.
        assert!(fresh.pin_accepted());
        // Another key plugged in: the one wanted is absent.
        let other = SoftReaders::with(vec![SoftCard::new("0006:87654321", &cert, "123456")]);
        let elsewhere = CardAccess::new(&other, &pin, &touch);
        assert!(CardSigner::new(&elsewhere, "0006:12345678", public).sign(HashAlgorithm::SHA512, &digest).is_err());
        assert_eq!(elsewhere.failure(), Some(CardError::Absent));
    }

    #[test]
    fn a_wrong_pin_costs_a_try_and_three_lock_the_key() {
        let cert = primary_signs("holder@example.org");
        let readers = SoftReaders::with(vec![SoftCard::new("0006:12345678", &cert, "123456")]);
        let public = cert.primary_key().key().clone().role_into_unspecified();
        let wrong = Password::from("000000");
        let access = CardAccess::new(&readers, &wrong, &|| {});
        let mut signer = CardSigner::new(&access, "0006:12345678", public.clone());
        for left in [2, 1, 0] {
            assert!(signer.sign(HashAlgorithm::SHA512, &[1; 64]).is_err());
            assert_eq!(access.failure(), Some(CardError::WrongPin { left }));
        }
        assert!(!access.pin_accepted());
        assert!(signer.sign(HashAlgorithm::SHA512, &[1; 64]).is_err());
        assert_eq!(access.failure(), Some(CardError::Blocked));
        // Locked: the right PIN does not open it either.
        let right = Password::from("123456");
        let access = CardAccess::new(&readers, &right, &|| {});
        assert!(CardSigner::new(&access, "0006:12345678", public).sign(HashAlgorithm::SHA512, &[1; 64]).is_err());
        assert_eq!(access.failure(), Some(CardError::Blocked));
        let tr = Translator::new("en");
        assert!(CardError::WrongPin { left: 2 }.sentence(&tr, Purpose::Sign).contains("2 tries left"));
        assert!(CardError::WrongPin { left: 1 }.sentence(&tr, Purpose::Sign).contains("one more mistake locks the key"));
        assert!(CardError::WrongPin { left: 0 }.sentence(&tr, Purpose::Sign).contains("Admin PIN"));
        assert_eq!(CardError::WrongPin { left: 1 }.remedy(), Remedy::Pin);
        assert_eq!(CardError::Blocked.remedy(), Remedy::None);
    }

    #[test]
    fn the_pin_for_each_signature_is_given_each_time() {
        let cert = primary_signs("holder@example.org");
        let mut card = SoftCard::new("0006:12345678", &cert, "123456");
        card.info.pin_each_signature = true;
        let pin = Password::from("123456");
        card.verify(&pin, PinMode::Sign).unwrap();
        card.sign(ToSign::Digest(&[1; 64])).unwrap();
        // The card wants it again; Sioul gives it before each signature, one per opening.
        assert!(card.sign(ToSign::Digest(&[1; 64])).is_err());
        let readers = SoftReaders::with(vec![card]);
        let access = CardAccess::new(&readers, &pin, &|| {});
        let mut signer = CardSigner::new(&access, "0006:12345678", cert.primary_key().key().clone().role_into_unspecified());
        signer.sign(HashAlgorithm::SHA512, &[2; 64]).unwrap();
        signer.sign(HashAlgorithm::SHA512, &[3; 64]).unwrap();
    }

    #[test]
    fn rsa_keys_sign_with_a_digest_info() {
        let (cert, _) = CertBuilder::new().add_userid("Card Holder <rsa@example.org>").set_cipher_suite(CipherSuite::RSA2k).set_primary_key_flags(KeyFlags::empty().set_certification().set_signing()).add_transport_encryption_subkey().generate().unwrap();
        let readers = SoftReaders::with(vec![SoftCard::new("000F:00001A2B", &cert, "123456")]);
        let pin = Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        let public = cert.primary_key().key().clone().role_into_unspecified();
        let mut signer = CardSigner::new(&access, "000F:00001A2B", public.clone());
        assert!(!signer.acceptable_hashes().contains(&HashAlgorithm::SHA1));
        let digest = [9u8; 32];
        let signature = signer.sign(HashAlgorithm::SHA256, &digest).unwrap();
        public.verify(&signature, HashAlgorithm::SHA256, &digest).unwrap();
        // RSA opens a session key on the card, PKCS#1 taken off there.
        let subkey = cert.keys().subkeys().next().unwrap().key().clone().role_into_unspecified();
        let session = SessionKey::from(&[1u8; 32][..]);
        let sealed = subkey.encrypt(&session).unwrap();
        let mut decryptor = CardDecryptor::new(&access, "000F:00001A2B", subkey);
        let opened = crypto::Decryptor::decrypt(&mut decryptor, &sealed, None).unwrap();
        assert_eq!(&opened[..], &session[..]);
    }

    #[test]
    fn curve25519_opens_through_the_cards_shared_secret() {
        let cert = primary_signs("holder@example.org");
        let readers = SoftReaders::with(vec![SoftCard::new("0006:12345678", &cert, "123456").with_x25519(x25519::x25519)]);
        let pin = Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        let subkey = cert.keys().subkeys().next().unwrap().key().clone().role_into_unspecified();
        let session = SessionKey::from(&[5u8; 32][..]);
        let sealed = subkey.encrypt(&session).unwrap();
        let mut decryptor = CardDecryptor::new(&access, "0006:12345678", subkey.clone());
        let opened = crypto::Decryptor::decrypt(&mut decryptor, &sealed, None).unwrap();
        assert_eq!(&opened[..], &session[..]);
        // A wrong shared secret does not unwrap: the glue's work is checked, not assumed.
        let wrong = SoftReaders::with(vec![SoftCard::new("0006:12345678", &cert, "123456").with_x25519(touchless)]);
        let access = CardAccess::new(&wrong, &pin, &|| {});
        let mut decryptor = CardDecryptor::new(&access, "0006:12345678", subkey);
        assert!(crypto::Decryptor::decrypt(&mut decryptor, &sealed, None).is_err());
    }

    #[test]
    fn a_key_held_elsewhere_or_no_service_is_said() {
        let cert = primary_signs("holder@example.org");
        let mut readers = SoftReaders::with(vec![]);
        assert_eq!(cards(&readers), Err(CardError::Absent));
        readers.busy = 1;
        assert_eq!(cards(&readers), Err(CardError::Busy));
        assert_eq!(with_card(&readers, "0006:12345678", |_, _| Ok(())), Err(CardError::Busy));
        readers.failure = Some(CardError::NoService);
        assert_eq!(cards(&readers), Err(CardError::NoService));
        let readers = SoftReaders::with(vec![SoftCard::new("0006:12345678", &cert, "123456")]);
        let found = cards(&readers).unwrap();
        assert_eq!(found[0].ident, "0006:12345678");
        assert_eq!(found[0].sign.as_ref().unwrap().algorithm, "Ed25519");
        assert_eq!(found[0].decrypt.as_ref().unwrap().algorithm, "Curve25519");
        let tr = Translator::new("en");
        assert_eq!(found[0].label(&tr), "YubiKey 12\u{202F}345\u{202F}678");
        assert_eq!(serial_text(0x0A1B2C3D), "A1B2C3D");
        assert_eq!(CardError::Busy.remedy(), Remedy::Release);
        assert!(CardError::NoService.sentence(&tr, Purpose::Sign).contains("smart card service"));
    }

    #[test]
    fn the_system_names_its_own_command() {
        assert_eq!(hint_for_os_release("NAME=\"Fedora Linux\"\nID=fedora\n"), "seckey-hint-fedora");
        assert_eq!(hint_for_os_release("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n"), "seckey-hint-debian");
        assert_eq!(hint_for_os_release("ID=endeavouros\nID_LIKE=arch\n"), "seckey-hint-arch");
        assert_eq!(hint_for_os_release("ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n"), "seckey-hint-suse");
        assert_eq!(hint_for_os_release("ID=gentoo\n"), "seckey-hint-linux");
    }

    #[test]
    fn a_certificate_is_checked_against_the_key() {
        let cert = primary_signs("holder@example.org");
        let card = SoftCard::new("0006:12345678", &cert, "123456");
        let found = check(&cert.clone().strip_secret_key_material(), &card.info).unwrap();
        assert_eq!(found.addresses, vec!["holder@example.org".to_string()]);
        assert!(found.signs && found.decrypts && !found.expired);
        assert_eq!(found.cert, cert.fingerprint().to_hex());
        // Another certificate: its keys are not those of the card.
        let stranger = primary_signs("holder@example.org");
        assert_eq!(check(&stranger, &card.info), Err(Mismatch::Missing));
        // The card's fingerprint written over another key's: the public parts tell.
        let mut forged = card.info.clone();
        forged.sign.as_mut().unwrap().public = card_public(stranger.primary_key().key().role_as_unspecified());
        assert_eq!(check(&cert, &forged), Err(Mismatch::OtherPublicKey));
        // A key in the signing slot that its certificate does not let sign.
        let mut swapped = card.info.clone();
        swapped.sign = card.info.decrypt.clone();
        assert_eq!(check(&cert, &swapped), Err(Mismatch::NotForSigning));
        // Revoked.
        let (revoked, revocation) = CertBuilder::new().add_userid("<gone@example.org>").set_cipher_suite(CipherSuite::Cv25519).set_primary_key_flags(KeyFlags::empty().set_certification().set_signing()).add_transport_encryption_subkey().generate().unwrap();
        let card_of_revoked = SoftCard::new("0006:00000001", &revoked, "123456");
        let revoked = revoked.insert_packets(revocation).unwrap().0;
        assert_eq!(check(&revoked, &card_of_revoked.info), Err(Mismatch::Revoked));
        // No user ID: still the key, naming no address.
        let bare = Cert::from_packets(cert.clone().strip_secret_key_material().into_packets().filter(|p| !matches!(p, openpgp::Packet::UserID(_)) && !matches!(p, openpgp::Packet::Signature(s) if s.typ() == openpgp::types::SignatureType::PositiveCertification))).unwrap();
        match check(&bare, &card.info) {
            Ok(found) => assert!(found.addresses.is_empty()),
            // Sequoia may want a binding to call the primary key valid: then said as such.
            Err(Mismatch::Invalid(_)) => {}
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_expired_key_is_still_its_key_and_says_so() {
        let made = SystemTime::now() - Duration::from_secs(3 * 365 * 24 * 3600);
        let (cert, _) = CertBuilder::new()
            .add_userid("Card Holder <late@example.org>")
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_creation_time(made)
            .set_validity_period(Duration::from_secs(365 * 24 * 3600))
            .set_primary_key_flags(KeyFlags::empty().set_certification().set_signing())
            .add_transport_encryption_subkey()
            .generate()
            .unwrap();
        let card = SoftCard::new("0006:12345678", &cert, "123456");
        let found = check(&cert, &card.info).unwrap();
        assert!(found.expired);
        let tr = Translator::new("en");
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64;
        let (line, warm) = expiry_line(&tr, found.expires, &found.cert, now).unwrap();
        assert!(warm && line.contains("expired on") && line.contains(&found.cert), "{line}");
        let (soon, warm) = expiry_line(&tr, Some(now + 10 * 24 * 3600), "F", now).unwrap();
        assert!(warm && soon.contains("expires on"), "{soon}");
        let (later, warm) = expiry_line(&tr, Some(now + 300 * 24 * 3600), "F", now).unwrap();
        assert!(!warm && later.contains("until"), "{later}");
        assert_eq!(expiry_line(&tr, None, "F", now), None);
    }

    #[test]
    fn the_pin_is_held_fifteen_quiet_minutes() {
        let start = Instant::now();
        let mut memory = PinMemory::new();
        memory.keep("0006:12345678", Password::from("123456"), "Yubico YubiKey", start);
        assert!(memory.get("0006:12345678", start + Duration::from_secs(60)).is_some());
        assert!(memory.get("0006:87654321", start).is_none());
        // Each use starts the quiet time again.
        memory.used("0006:12345678", start + Duration::from_secs(14 * 60));
        assert!(memory.get("0006:12345678", start + Duration::from_secs(28 * 60)).is_some());
        assert!(memory.get("0006:12345678", start + Duration::from_secs(29 * 60 + 1)).is_none());
        assert_eq!(memory.holder(), None);
        // The key pulled out: forgotten.
        memory.keep("0006:12345678", Password::from("123456"), "Yubico YubiKey", start);
        assert!(!memory.went_away(&["Yubico YubiKey".to_string()]));
        assert!(memory.went_away(&[]));
        assert_eq!(memory.holder(), None);
        memory.keep("0006:12345678", Password::from("123456"), "Yubico YubiKey", start);
        memory.forget();
        assert!(memory.get("0006:12345678", start).is_none());
    }

    #[test]
    fn known_keys_are_kept_without_secrets() {
        let dir = std::env::temp_dir().join(format!("sioul-cards-{}", std::process::id()));
        let path = dir.join("cards.toml");
        let _ = std::fs::remove_file(&path);
        change_known_at(&path, |cards| {
            cards.push(KnownCard { ident: "0006:12345678".into(), manufacturer: 6, serial: 0x12345678, cert: "ABCD".into(), addresses: vec!["Holder@Example.org".into()], touch_sign: Touch::On, ..KnownCard::default() });
        })
        .unwrap();
        let read = read_known(&path);
        assert_eq!(read.len(), 1);
        assert!(read[0].serves("holder@example.org") && !read[0].serves("other@example.org"));
        assert_eq!(read[0].touch_sign, Touch::On);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[[card]]") && text.contains("touch_sign = \"on\""), "{text}");
        let tr = Translator::new("en");
        let mut known = read[0].clone();
        assert_eq!(known.tries_line(3, &tr), None);
        assert!(known.tries_line(2, &tr).unwrap().contains("2 tries left"));
        known.pin_tries_full = 5;
        assert!(known.tries_line(4, &tr).is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_failure_has_words_in_each_language() {
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let failures = [
                CardError::Unsupported,
                CardError::NoService,
                CardError::Absent,
                CardError::Busy,
                CardError::WrongPin { left: 2 },
                CardError::WrongPin { left: 1 },
                CardError::Blocked,
                CardError::TouchMissed,
                CardError::Removed,
                CardError::NotOpenPgp,
                CardError::OtherKeys,
                CardError::BadSignature,
                CardError::Algorithm("X448".into()),
                CardError::Other("no reader".into()),
            ];
            for failure in failures {
                for purpose in [Purpose::Sign, Purpose::Open, Purpose::Read] {
                    let said = failure.sentence(&tr, purpose);
                    assert!(!said.is_empty() && !said.contains("seckey-") && !said.contains('!'), "{language} {failure:?}: {said}");
                }
            }
            for mismatch in [Mismatch::Empty, Mismatch::Missing, Mismatch::Revoked, Mismatch::NotForSigning, Mismatch::OtherPublicKey, Mismatch::Invalid("SHA-1".into())] {
                assert!(!mismatch.sentence(&tr).contains("seckey-"), "{language} {mismatch:?}");
            }
            for hint in ["seckey-hint-fedora", "seckey-hint-debian", "seckey-hint-arch", "seckey-hint-suse", "seckey-hint-linux", "seckey-hint-flatpak", "seckey-hint-windows", "seckey-hint-macos"] {
                assert_ne!(tr.text(hint, None), hint, "{language} {hint}");
            }
        }
    }
}

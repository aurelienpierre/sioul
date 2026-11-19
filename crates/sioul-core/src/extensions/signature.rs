// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The manifest's detached OpenPGP signature, checked against the one key
//! compiled in (docs/extensions.md, "The signature: exactly what is
//! checked"). The key is a primary key packet alone, Ed25519 version 4, with
//! its fingerprint: no certificate, so no user id, no subkey and no
//! self-signature is read, and the key's expiry, revocation and capabilities
//! are not checked. A change of key is a new release of Sioul.
//!
//! The signature is checked with Sequoia's packet-level functions, Sioul's
//! library for signed mail: one packet, a signature of a binary document,
//! issued by that key, its hash and critical subpackets accepted by the
//! standard policy, made no later than now and not expired, and holding over
//! the manifest's exact bytes.

use openpgp::packet::{Key, Packet, key};
use openpgp::parse::Parse;
use openpgp::policy::{HashAlgoSecurity, Policy, StandardPolicy};
use openpgp::types::SignatureType;
use openpgp::{Fingerprint, KeyHandle, PacketPile};
use sequoia_openpgp as openpgp;
use std::time::{Duration, SystemTime};

/// A key the manifest may be signed with: its primary key packet, and its fingerprint.
pub struct Signer {
    pub packet: &'static [u8],
    pub fingerprint: &'static str,
    /// What the key is, in the log and in the program (a test build's names it as such).
    pub said: &'static str,
}

/// The publisher's key, which signs the manifest by hand at each release of
/// the extensions: the primary public key packet of the key Sioul's releases
/// are signed with, alone. Release builds check the manifest with it.
pub const PUBLISHER: Signer = Signer {
    packet: &[
        0xc6, 0x33, 0x04, 0x66, 0x14, 0x21, 0x90, 0x16, 0x09, 0x2b, 0x06, 0x01, 0x04, 0x01, 0xda, 0x47, //
        0x0f, 0x01, 0x01, 0x07, 0x40, 0xa3, 0xf3, 0xa3, 0xd8, 0x1b, 0xb1, 0xb2, 0x02, 0xf2, 0x9b, 0x07, //
        0x7a, 0x59, 0xe6, 0x66, 0x00, 0x96, 0x74, 0x0f, 0x3f, 0x84, 0x7f, 0x86, 0x27, 0xb6, 0x60, 0x97, //
        0x53, 0xd9, 0xf6, 0xee, 0x35,
    ],
    fingerprint: "2EF12D28F49E347CAD7E6BF31549D8B45963166A",
    said: "the publisher's key",
};

/// The tests' own key, made for them from a seed written in the tests
/// (`TEST_SEED`), never a real one: test builds alone hold it, with its mark,
/// so that `tools/check-test-marks.sh` refuses a program that would ship it.
#[cfg(any(test, feature = "test-hooks"))]
pub const TEST_SIGNER: Signer = Signer {
    packet: &[
        0xc6, 0x33, 0x04, 0x69, 0x55, 0xb9, 0x00, 0x16, 0x09, 0x2b, 0x06, 0x01, 0x04, 0x01, 0xda, 0x47, //
        0x0f, 0x01, 0x01, 0x07, 0x40, 0x97, 0xf3, 0x33, 0xd5, 0x11, 0xfc, 0x44, 0x79, 0x8e, 0xe2, 0xc9, //
        0x75, 0x47, 0xd4, 0x15, 0x19, 0x92, 0x99, 0x0d, 0x90, 0x10, 0x94, 0xe1, 0xbc, 0xdf, 0xfb, 0xe4, //
        0x1a, 0xf5, 0x5a, 0xd3, 0xd7,
    ],
    fingerprint: "436523CD8B0B9CD5A90AD68F1789260C3961EE56",
    said: "SIOUL_TEST_EXTENSIONS_KEY: the tests' own key, never a real manifest's",
};

/// The seed of the tests' key (SHA-256 of a sentence: see the test that
/// makes it again), and its creation time (2026-01-01, UTC).
#[cfg(any(test, feature = "test-hooks"))]
pub const TEST_SEED: [u8; 32] = [
    0x05, 0xb2, 0xf4, 0x3c, 0x0b, 0xac, 0x15, 0x8d, 0xd5, 0x7a, 0xbb, 0x3a, 0xd7, 0x91, 0xb6, 0x30, //
    0x27, 0x11, 0x94, 0x88, 0x42, 0x20, 0x33, 0x6f, 0xf1, 0xea, 0xb7, 0xc0, 0xd0, 0x7c, 0x1f, 0x36,
];
#[cfg(any(test, feature = "test-hooks"))]
const TEST_CREATED: u64 = 1_767_225_600;

/// The key Sioul checks the manifest with: the publisher's in a release
/// build, the tests' own in a test build (`test-hooks`, which the crates'
/// tests turn on), whose mark is then in the program.
pub fn signer() -> &'static Signer {
    #[cfg(any(test, feature = "test-hooks"))]
    if !std::hint::black_box(TEST_SIGNER.said).is_empty() {
        return &TEST_SIGNER;
    }
    &PUBLISHER
}

/// Why a manifest's signature was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The compiled key does not read, or is not its fingerprint (Sioul's build).
    Key,
    /// Not exactly one OpenPGP packet, a signature.
    NotOneSignature,
    /// Not a signature of a binary document.
    NotBinary,
    /// Issued by another key, or naming none.
    OtherSigner,
    /// Its hash, or a critical subpacket, refused by the standard policy.
    Policy(String),
    /// Made later than now, or expired.
    Time,
    /// It does not hold over the manifest's bytes.
    DoesNotHold,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Key => write!(f, "the key compiled in does not read"),
            Self::NotOneSignature => write!(f, "not one OpenPGP signature"),
            Self::NotBinary => write!(f, "not a signature of a binary document"),
            Self::OtherSigner => write!(f, "signed by another key"),
            Self::Policy(why) => write!(f, "refused by the standard policy: {why}"),
            Self::Time => write!(f, "made later than now, or expired"),
            Self::DoesNotHold => write!(f, "it does not hold over the manifest"),
        }
    }
}

/// The signer's key read from its packet, when its fingerprint is the one compiled beside it.
fn key_of(signer: &Signer) -> Result<Key<key::PublicParts, key::PrimaryRole>, Refusal> {
    match Packet::from_bytes(signer.packet) {
        Ok(Packet::PublicKey(key)) if key.fingerprint().to_hex() == signer.fingerprint => Ok(key),
        _ => Err(Refusal::Key),
    }
}

/// Checks `signature` (binary or ASCII-armored) over `manifest`'s exact
/// bytes, made by `signer`'s key, at `now`.
pub fn verify(manifest: &[u8], signature: &[u8], signer: &Signer, now: SystemTime) -> Result<(), Refusal> {
    let key = key_of(signer)?;
    let fingerprint: Fingerprint = signer.fingerprint.parse().map_err(|_| Refusal::Key)?;
    let pile = PacketPile::from_bytes(signature).map_err(|_| Refusal::NotOneSignature)?;
    let packets: Vec<&Packet> = pile.descendants().collect();
    let [Packet::Signature(sig)] = packets.as_slice() else { return Err(Refusal::NotOneSignature) };
    if sig.typ() != SignatureType::Binary {
        return Err(Refusal::NotBinary);
    }
    // Issued by that key: its fingerprint named, and no issuer naming another.
    let handle = KeyHandle::from(&fingerprint);
    if !sig.issuer_fingerprints().any(|issuer| *issuer == fingerprint) || !sig.get_issuers().iter().all(|issuer| issuer.aliases(&handle)) {
        return Err(Refusal::OtherSigner);
    }
    StandardPolicy::at(now).signature(sig, HashAlgoSecurity::CollisionResistance).map_err(|e| Refusal::Policy(e.to_string()))?;
    sig.signature_alive(now, Duration::from_secs(60)).map_err(|_| Refusal::Time)?;
    sig.verify_message(&key, manifest).map_err(|_| Refusal::DoesNotHold)
}

/// A signature of `data` by the tests' key, of the type given, made at
/// `created`: for the tests of every crate, never in a release build.
#[cfg(any(test, feature = "test-hooks"))]
pub fn test_signature(data: &[u8], kind: SignatureType, created: SystemTime) -> Vec<u8> {
    test_signature_by(&TEST_SEED, data, kind, created)
}

/// The same by the key of another seed (a key Sioul does not know).
#[cfg(any(test, feature = "test-hooks"))]
pub fn test_signature_by(seed: &[u8; 32], data: &[u8], kind: SignatureType, created: SystemTime) -> Vec<u8> {
    use openpgp::packet::key::Key4;
    use openpgp::packet::signature::SignatureBuilder;
    use openpgp::serialize::SerializeInto;
    let made = SystemTime::UNIX_EPOCH + Duration::from_secs(TEST_CREATED);
    let key: Key<key::SecretParts, key::PrimaryRole> = Key4::import_secret_ed25519(seed, made).expect("an Ed25519 key from 32 bytes").into();
    let mut pair = key.clone().into_keypair().expect("a key pair");
    let signature = SignatureBuilder::new(kind)
        .set_signature_creation_time(created)
        .and_then(|b| b.set_issuer_fingerprint(key.fingerprint()))
        .and_then(|b| b.set_issuer(key.keyid()))
        .and_then(|b| b.sign_message(&mut pair, data))
        .expect("a signature");
    Packet::from(signature).to_vec().expect("a packet")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    const NOW: u64 = 1_794_996_000;

    #[test]
    fn both_keys_read_and_the_test_build_uses_the_tests_own() {
        assert_eq!(key_of(&PUBLISHER).map(|k| k.fingerprint().to_hex()), Ok(PUBLISHER.fingerprint.to_string()));
        assert_eq!(key_of(&TEST_SIGNER).map(|k| k.fingerprint().to_hex()), Ok(TEST_SIGNER.fingerprint.to_string()));
        assert_eq!(signer().fingerprint, TEST_SIGNER.fingerprint);
        assert!(TEST_SIGNER.said.starts_with("SIOUL_TEST_"), "the mark tools/check-test-marks.sh finds");
        // The seed is the one its comment says.
        use sha2::Digest;
        assert_eq!(sha2::Sha256::digest(b"Sioul's extensions: the tests' own key, never a real one").as_slice(), TEST_SEED);
        // One wrong byte in the packet, and the key is not the one named.
        let mut wrong = PUBLISHER.packet.to_vec();
        wrong[30] ^= 1;
        let leaked: &'static [u8] = Box::leak(wrong.into_boxed_slice());
        assert_eq!(key_of(&Signer { packet: leaked, ..PUBLISHER }).err(), Some(Refusal::Key));
    }

    #[test]
    fn a_manifest_holds_only_with_its_own_signature() {
        let manifest = b"format = 1\nserial = 1\n";
        let signed = test_signature(manifest, SignatureType::Binary, at(NOW - 60));
        assert_eq!(verify(manifest, &signed, &TEST_SIGNER, at(NOW)), Ok(()));
        // Armored, as `gpg --armor` writes it.
        let armored = {
            let mut writer = openpgp::armor::Writer::new(Vec::new(), openpgp::armor::Kind::Signature).unwrap();
            std::io::Write::write_all(&mut writer, &signed).unwrap();
            writer.finalize().unwrap()
        };
        assert_eq!(verify(manifest, &armored, &TEST_SIGNER, at(NOW)), Ok(()));
        // One byte changed in the manifest, or a line added.
        assert_eq!(verify(b"format = 1\nserial = 2\n", &signed, &TEST_SIGNER, at(NOW)), Err(Refusal::DoesNotHold));
        assert_eq!(verify(b"format = 1\nserial = 1\n\n", &signed, &TEST_SIGNER, at(NOW)), Err(Refusal::DoesNotHold));
        // The publisher's key does not take the tests' signature.
        assert_eq!(verify(manifest, &signed, &PUBLISHER, at(NOW)), Err(Refusal::OtherSigner));
        // Another key's signature.
        let other = test_signature_by(&[7; 32], manifest, SignatureType::Binary, at(NOW - 60));
        assert_eq!(verify(manifest, &other, &TEST_SIGNER, at(NOW)), Err(Refusal::OtherSigner));
        // A text signature, a signature from the future, two signatures, nothing.
        let text = test_signature(manifest, SignatureType::Text, at(NOW - 60));
        assert_eq!(verify(manifest, &text, &TEST_SIGNER, at(NOW)), Err(Refusal::NotBinary));
        let later = test_signature(manifest, SignatureType::Binary, at(NOW + 3600));
        assert_eq!(verify(manifest, &later, &TEST_SIGNER, at(NOW)), Err(Refusal::Time));
        let twice = [signed.clone(), signed.clone()].concat();
        assert_eq!(verify(manifest, &twice, &TEST_SIGNER, at(NOW)), Err(Refusal::NotOneSignature));
        assert_eq!(verify(manifest, b"", &TEST_SIGNER, at(NOW)), Err(Refusal::NotOneSignature));
        assert_eq!(verify(manifest, b"not a signature", &TEST_SIGNER, at(NOW)), Err(Refusal::NotOneSignature));
    }
}

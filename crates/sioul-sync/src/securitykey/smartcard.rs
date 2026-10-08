// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Security keys through the system's smart card service, PC/SC: pcscd
//! (pcsc-lite) on Linux, the Smart Card service on Windows, CryptoTokenKit on
//! macOS. The OpenPGP card protocol is `openpgp-card` 0.7's, its transport
//! `card-backend-pcsc` 0.5's (both Heiko Schaefer's, MIT OR Apache-2.0).
//!
//! - **Shared, one operation at a time.** Each reader is connected in shared
//!   mode, so that GnuPG may share the key when its `scdaemon.conf` says
//!   `pcsc-shared`; one transaction serves one operation, and the key is then
//!   disconnected, which resets it (`pcsc`'s own `Drop` disposes with
//!   `ResetCard`): its PIN, once given, does not stay given for another
//!   program.
//! - **Readers one by one.** `card-backend-pcsc` skips, silently, a reader
//!   it cannot connect to: a key that GnuPG holds for itself would read as no
//!   key. Sioul connects each reader itself, and counts a sharing violation
//!   as a busy key, which the window says, with "Let GnuPG release it".
//! - **Status words in words** ([`CardError`]): 63Cx a wrong PIN with x tries
//!   left, 6983 a locked PIN, 6600 (the specification's "security-related
//!   issues", kept for the touch button) a touch missed, and so is 6982 just
//!   after the PIN was taken, when the key asks for a touch; a card taken out
//!   while it worked, the key removed.
//! - **Nothing is logged here.** `openpgp-card` writes whole APDUs at the
//!   "trace" level, a VERIFY's PIN included, and the card's answers (session
//!   keys): Sioul installs no logger, and caps the level at "info" as each
//!   program starts ([`super::cap_logging`]).

use card_backend::SmartcardError;
use card_backend_pcsc::PcscBackend;
use openpgp_card::ocard::algorithm::{AlgorithmAttributes, Curve};
use openpgp_card::ocard::crypto::{Cryptogram, HashAlgo, PublicKeyMaterial, SigningAlgo};
use openpgp_card::ocard::data::UserInteractionFlag;
use openpgp_card::ocard::{KeyType, StatusBytes};
use openpgp_card::state::{Open, Transaction};
use sioul_core::securitykey::{Card, CardError, CardInfo, CardPublic, Password, PinMode, Protected, Readers, SlotInfo, ToDecipher, ToSign, Touch, Visit, Visited};
use std::ffi::CString;
use std::time::Duration;

/// The system's readers, through PC/SC.
pub struct PcscReaders;

/// The smart card service, reached; else why not.
fn establish() -> Result<pcsc::Context, CardError> {
    pcsc::Context::establish(pcsc::Scope::User).map_err(|e| match e {
        pcsc::Error::NoService | pcsc::Error::ServiceStopped | pcsc::Error::NoReadersAvailable => CardError::NoService,
        other => CardError::Other(other.to_string()),
    })
}

/// The readers the service knows: none is no error.
fn readers_of(context: &pcsc::Context) -> Result<Vec<CString>, CardError> {
    match context.list_readers_owned() {
        Ok(readers) => Ok(readers),
        Err(pcsc::Error::NoReadersAvailable) => Ok(Vec::new()),
        Err(pcsc::Error::NoService | pcsc::Error::ServiceStopped) => Err(CardError::NoService),
        Err(other) => Err(CardError::Other(other.to_string())),
    }
}

impl Readers for PcscReaders {
    fn present(&self) -> Result<Vec<String>, CardError> {
        let context = establish()?;
        let mut states: Vec<pcsc::ReaderState> = readers_of(&context)?.into_iter().map(|name| pcsc::ReaderState::new(name, pcsc::State::UNAWARE)).collect();
        if states.is_empty() {
            return Ok(Vec::new());
        }
        // Unaware of every state: the service says each at once, without opening any key.
        match context.get_status_change(Duration::ZERO, &mut states) {
            Ok(()) | Err(pcsc::Error::Timeout) => {}
            Err(pcsc::Error::NoService | pcsc::Error::ServiceStopped) => return Err(CardError::NoService),
            Err(other) => return Err(CardError::Other(other.to_string())),
        }
        Ok(states.iter().filter(|s| s.event_state().contains(pcsc::State::PRESENT)).map(|s| s.name().to_string_lossy().into_owned()).collect())
    }

    fn visit(&self, visit: &mut dyn FnMut(&mut dyn Card) -> Visit) -> Result<Visited, CardError> {
        let context = establish()?;
        let mut visited = Visited::default();
        for reader in readers_of(&context)? {
            let connected = match context.connect(&reader, pcsc::ShareMode::Shared, pcsc::Protocols::ANY) {
                Ok(card) => card,
                // Another program holds it for itself: GnuPG's scdaemon, most often.
                Err(pcsc::Error::SharingViolation) => {
                    visited.busy += 1;
                    continue;
                }
                // An empty reader, or one gone meanwhile.
                Err(_) => continue,
            };
            let Ok(backend) = PcscBackend::from_pcsc_card(&reader, pcsc::ShareMode::Shared, connected) else { continue };
            // Not an OpenPGP card (another kind of card in the reader): left alone.
            let Ok(mut opened) = openpgp_card::Card::<Open>::new(backend) else { continue };
            let Ok(transaction) = opened.transaction() else { continue };
            let mut card = PcscCard { tx: transaction, reader: reader.to_string_lossy().into_owned(), pin_taken: false, touch: (false, false) };
            visited.opened += 1;
            let next = visit(&mut card);
            // Closed: the transaction ends, then the connection, which resets the key.
            drop(card);
            drop(opened);
            if next == Visit::Done {
                break;
            }
        }
        Ok(visited)
    }
}

/// What a key's user interaction flag says of its touch (DO D6 for signing,
/// D7 for decrypting, D8 for authenticating: its first byte, the touch
/// policy). A card without the flag asks for none; a flag that does not read
/// is a touch maybe asked: said "If your key asks for a touch…", a wrong guess
/// costing nothing.
fn touch_of(flag: Result<Option<UserInteractionFlag>, openpgp_card::Error>) -> Touch {
    match flag {
        Ok(Some(flag)) => Touch::from_flag(u8::from(flag.touch_policy())),
        Ok(None) => Touch::Off,
        Err(_) => Touch::Unknown,
    }
}

/// One OpenPGP card, open in one transaction.
struct PcscCard<'a> {
    tx: openpgp_card::Card<Transaction<'a>>,
    reader: String,
    /// The PIN was taken in this transaction: a refusal after it is the touch's.
    pin_taken: bool,
    /// Whether the signing key and the decryption key ask for a touch.
    touch: (bool, bool),
}

impl PcscCard<'_> {
    /// One key slot, as the card describes it; None when it holds no key.
    fn slot(&mut self, key: KeyType, fingerprint: Option<String>, created: u32) -> Option<SlotInfo> {
        let fingerprint = fingerprint?;
        let algorithm = self.tx.algorithm_attributes(key).map(|a| algorithm_name(&a)).unwrap_or_default();
        let touch = touch_of(self.tx.user_interaction_flag(key));
        let public = match self.tx.public_key_material(key) {
            Ok(PublicKeyMaterial::R(rsa)) => Some(CardPublic::Rsa { n: rsa.n().to_vec(), e: rsa.v().to_vec() }),
            Ok(PublicKeyMaterial::E(ecc)) => Some(CardPublic::Ecc { point: ecc.data().to_vec() }),
            Err(_) => None,
        };
        Some(SlotInfo { fingerprint, created, algorithm, touch, public })
    }

    fn failed(&self, error: openpgp_card::Error, touch: bool) -> CardError {
        card_error(error, self.pin_taken && touch)
    }
}

impl Card for PcscCard<'_> {
    fn info(&mut self) -> Result<CardInfo, CardError> {
        let aid = self.tx.application_identifier().map_err(|e| card_error(e, false))?;
        let fingerprints = self.tx.fingerprints().map_err(|e| card_error(e, false))?;
        let times = self.tx.key_generation_times().ok();
        let status = self.tx.pw_status_bytes().map_err(|e| card_error(e, false))?;
        let created = |time: Option<&openpgp_card::ocard::data::KeyGenerationTime>| time.map_or(0, u32::from);
        let (sign_time, decrypt_time) = times.as_ref().map_or((0, 0), |t| (created(t.signature()), created(t.decryption())));
        let sign = self.slot(KeyType::Signing, fingerprints.signature().map(|f| f.to_hex().to_ascii_uppercase()), sign_time);
        let decrypt = self.slot(KeyType::Decryption, fingerprints.decryption().map(|f| f.to_hex().to_ascii_uppercase()), decrypt_time);
        self.touch = (sign.as_ref().is_some_and(|s| s.touch.asked()), decrypt.as_ref().is_some_and(|s| s.touch.asked()));
        Ok(CardInfo {
            ident: aid.ident(),
            manufacturer: aid.manufacturer(),
            serial: aid.serial(),
            holder: self.tx.cardholder_name().unwrap_or_default(),
            url: self.tx.url().unwrap_or_default(),
            sign,
            decrypt,
            pin_tries: status.err_count_pw1(),
            reset_tries: status.err_count_rc(),
            admin_tries: status.err_count_pw3(),
            pin_each_signature: status.pw1_cds_valid_once(),
            signatures: self.tx.digital_signature_count().unwrap_or(0),
            reader: self.reader.clone(),
        })
    }

    fn verify(&mut self, pin: &Password, mode: PinMode) -> Result<(), CardError> {
        // The PIN as the protocol takes it: a SecretString, wiped when dropped.
        let pin = pin.map(|p| std::str::from_utf8(p).map(secrecy::SecretString::from)).map_err(|_| CardError::Other("the PIN is not text".into()))?;
        let verified = match mode {
            PinMode::Sign => self.tx.verify_user_signing_pin(pin),
            PinMode::Decrypt => self.tx.verify_user_pin(pin),
        };
        verified.map_err(|e| card_error(e, false))?;
        self.pin_taken = true;
        Ok(())
    }

    fn sign(&mut self, input: ToSign<'_>) -> Result<Vec<u8>, CardError> {
        let (algorithm, digest) = match input {
            ToSign::Digest(digest) => (SigningAlgo::ECC, digest),
            ToSign::Rsa { hash, digest } => {
                let hash = match hash {
                    sioul_core::securitykey::HashAlgorithm::SHA256 => HashAlgo::SHA256,
                    sioul_core::securitykey::HashAlgorithm::SHA384 => HashAlgo::SHA384,
                    sioul_core::securitykey::HashAlgorithm::SHA512 => HashAlgo::SHA512,
                    other => return Err(CardError::Algorithm(format!("RSA, {other}"))),
                };
                (SigningAlgo::RSA(hash), digest)
            }
        };
        let touch = self.touch.0;
        self.tx.card().signature_for_hash(algorithm, digest).map_err(|e| self.failed(e, touch))
    }

    fn decipher(&mut self, input: ToDecipher<'_>) -> Result<Protected, CardError> {
        let cryptogram = match input {
            ToDecipher::Rsa(ciphertext) => Cryptogram::RSA(ciphertext),
            ToDecipher::Ecdh(point) => Cryptogram::ECDH(point),
        };
        let touch = self.touch.1;
        // What the key gives back (a session key, a shared secret) goes into protected memory at once.
        self.tx.card().decipher(cryptogram).map(Protected::from).map_err(|e| self.failed(e, touch))
    }
}

/// "Ed25519", "Curve25519", "RSA 4096", "NIST P-256": as GnuPG and YubiKey Manager name them.
fn algorithm_name(attributes: &AlgorithmAttributes) -> String {
    match attributes {
        AlgorithmAttributes::Rsa(rsa) => format!("RSA {}", rsa.len_n()),
        AlgorithmAttributes::Ecc(ecc) => match ecc.curve() {
            Curve::Ed25519 => "Ed25519".into(),
            Curve::Curve25519 => "Curve25519".into(),
            Curve::NistP256r1 => "NIST P-256".into(),
            Curve::NistP384r1 => "NIST P-384".into(),
            Curve::NistP521r1 => "NIST P-521".into(),
            Curve::BrainpoolP256r1 => "brainpoolP256r1".into(),
            Curve::BrainpoolP384r1 => "brainpoolP384r1".into(),
            Curve::BrainpoolP512r1 => "brainpoolP512r1".into(),
            Curve::Secp256k1 => "secp256k1".into(),
            Curve::Ed448 => "Ed448".into(),
            Curve::X448 => "X448".into(),
            Curve::Unknown(_) => String::new(),
        },
        AlgorithmAttributes::Unknown(_) => String::new(),
    }
}

/// What the card or the reader said, as Sioul says it. `touch_after_pin`: the
/// key took the PIN in this transaction and asks for a touch, so that its
/// refusal now is the touch's.
fn card_error(error: openpgp_card::Error, touch_after_pin: bool) -> CardError {
    match error {
        openpgp_card::Error::CardStatus(StatusBytes::PasswordNotChecked(left)) => CardError::WrongPin { left },
        openpgp_card::Error::CardStatus(StatusBytes::AuthenticationMethodBlocked) => CardError::Blocked,
        openpgp_card::Error::CardStatus(StatusBytes::SecurityRelatedIssues) => CardError::TouchMissed,
        openpgp_card::Error::CardStatus(StatusBytes::SecurityStatusNotSatisfied) if touch_after_pin => CardError::TouchMissed,
        openpgp_card::Error::Smartcard(SmartcardError::Error(said)) if ["RemovedCard", "NoSmartcard", "ReaderUnavailable", "UnknownReader"].iter().any(|w| said.contains(w)) => CardError::Removed,
        openpgp_card::Error::Smartcard(SmartcardError::ContextError(_)) => CardError::NoService,
        other => CardError::Other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cards_words_become_sioul_s() {
        assert_eq!(card_error(openpgp_card::Error::CardStatus(StatusBytes::PasswordNotChecked(2)), false), CardError::WrongPin { left: 2 });
        assert_eq!(card_error(openpgp_card::Error::CardStatus(StatusBytes::PasswordNotChecked(0)), false), CardError::WrongPin { left: 0 });
        assert_eq!(card_error(openpgp_card::Error::CardStatus(StatusBytes::AuthenticationMethodBlocked), false), CardError::Blocked);
        assert_eq!(card_error(openpgp_card::Error::CardStatus(StatusBytes::SecurityRelatedIssues), false), CardError::TouchMissed);
        // 6982 is a touch missed only just after the PIN was taken, for a key that asks for one.
        assert_eq!(card_error(openpgp_card::Error::CardStatus(StatusBytes::SecurityStatusNotSatisfied), true), CardError::TouchMissed);
        assert!(matches!(card_error(openpgp_card::Error::CardStatus(StatusBytes::SecurityStatusNotSatisfied), false), CardError::Other(_)));
        assert_eq!(card_error(openpgp_card::Error::Smartcard(SmartcardError::Error("Transmit failed: RemovedCard".into())), false), CardError::Removed);
        assert_eq!(card_error(openpgp_card::Error::Smartcard(SmartcardError::ContextError("no service".into())), false), CardError::NoService);
    }

    /// Reads the security keys plugged in, without their PIN: nothing is signed,
    /// nothing is opened, no PIN is given, so no try is spent. Never in a test
    /// run: the owner runs it himself, his key plugged in:
    /// `cargo test --release -p sioul-sync -- --ignored --nocapture a_real_key_reads_without_its_pin`.
    /// The touch settings, from user interaction flags made of invented bytes
    /// (policy, then features: 0x20 a button), as a card's DOs D6 to D8 give
    /// them; never a real card.
    #[test]
    fn the_touch_settings_read_from_the_flags() {
        let flag = |bytes: &[u8]| UserInteractionFlag::try_from(bytes.to_vec());
        assert_eq!(touch_of(flag(&[0x00, 0x20]).map(Some)), Touch::Off);
        assert_eq!(touch_of(flag(&[0x01, 0x20]).map(Some)), Touch::On, "a YubiKey with touch on: UIF Sign=on");
        assert_eq!(touch_of(flag(&[0x02, 0x20]).map(Some)), Touch::Fixed);
        assert_eq!(touch_of(flag(&[0x03, 0x20]).map(Some)), Touch::Cached);
        assert_eq!(touch_of(flag(&[0x04, 0x20]).map(Some)), Touch::CachedFixed);
        assert_eq!(touch_of(flag(&[0x7f, 0x20]).map(Some)), Touch::Unknown, "a setting Sioul does not know: asked");
        // Bytes that are no flag: not read, so said as maybe.
        assert_eq!(touch_of(flag(&[0x01]).map(Some)), Touch::Unknown);
        // A card without the flag (before version 3): no touch.
        assert_eq!(touch_of(Ok(None)), Touch::Off);
        let tr = sioul_core::i18n::Translator::new("en");
        assert!(touch_of(flag(&[0x01, 0x20]).map(Some)).words(&tr).is_some_and(|w| w.contains("hold your finger") && w.contains("15 seconds")));
        assert!(touch_of(flag(&[0x01]).map(Some)).words(&tr).is_some_and(|w| w.starts_with("If your key asks for a touch")));
        assert_eq!(touch_of(Ok(None)).words(&tr), None);
    }

    #[test]
    #[ignore = "reads a real security key: the owner runs it himself, his key plugged in"]
    fn a_real_key_reads_without_its_pin() {
        let tr = sioul_core::i18n::Translator::new("en");
        match sioul_core::securitykey::cards(&PcscReaders) {
            Ok(cards) => {
                for card in cards {
                    println!("{} ({}), holder {:?}, reader {:?}", card.label(&tr), card.ident, card.holder, card.reader);
                    for (what, slot) in [("signs", &card.sign), ("decrypts", &card.decrypt)] {
                        if let Some(slot) = slot {
                            println!("  {what}: {} {}, made {}, touch {:?}, public part read: {}", slot.algorithm, slot.fingerprint, slot.created, slot.touch, slot.public.is_some());
                        }
                    }
                    println!("  PIN tries {}, reset code {}, Admin PIN {}; PIN for each signature: {}; signatures made {}; URL {:?}", card.pin_tries, card.reset_tries, card.admin_tries, card.pin_each_signature, card.signatures, card.url);
                }
            }
            Err(e) => println!("{}", e.sentence(&tr, sioul_core::securitykey::Purpose::Read)),
        }
    }
}

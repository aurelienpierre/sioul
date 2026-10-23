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
//! - **The card itself** (its reading, its PIN, its signing and opening,
//!   its status words in words, nothing logged): [`super::ocard`], shared
//!   with the phone.

use super::ocard::OpenCard;
use card_backend_pcsc::PcscBackend;
use openpgp_card::state::Open;
use sioul_core::securitykey::{Card, CardError, Readers, Visit, Visited};
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
            let mut card = OpenCard::new(transaction, reader.to_string_lossy().into_owned());
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the security keys plugged in, without their PIN: nothing is signed,
    /// nothing is opened, no PIN is given, so no try is spent. Never in a test
    /// run: the owner runs it himself, his key plugged in:
    /// `cargo test --release -p sioul-sync -- --ignored --nocapture a_real_key_reads_without_its_pin`.
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

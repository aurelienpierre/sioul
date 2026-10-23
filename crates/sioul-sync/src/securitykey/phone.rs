// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A security key held against the phone (NFC), as Sioul's own backend of
//! `openpgp-card`: Android has no smart card service, so the card's APDUs
//! travel through a [`Link`] (the app's, over Android's `IsoDep`), and the
//! card itself is read, given its PIN and used by the same code as on a
//! computer ([`super::ocard`]). docs/android.md, "Your security key".
//!
//! - **One tap.** A key is in reach only while it is held: the PIN is asked
//!   first, from what Sioul knows of the key, then one tap reads it, checks
//!   it is the one known, gives its PIN and signs or opens, as long as it
//!   is held ([`PhoneReaders::taps`]).
//! - **What the phone carries.** Each exchange at most what its NFC controller
//!   takes; extended length only where the phone says it takes it
//!   ([`limit`]): else short APDUs, which `openpgp-card` chains and whose
//!   long answers it reads with GET RESPONSE.
//! - **Nothing left unlocked.** At the end of each opening, the PIN's two
//!   checks are undone (VERIFY with P1 = FF, OpenPGP card specification
//!   3.4.1, §7.2.2), before the key is let go: no other app finds it
//!   unlocked while it stays near the phone. Android then hands it to no
//!   other app until it has left the field (the app's link).
//! - **A key moved away** mid-way is said as such ([`CardError::Removed`]);
//!   a card that is no OpenPGP card (a bank card), as such.
//! - **Nothing is logged**, here or in the link: never an APDU's bytes.

use super::ocard::{card_error, OpenCard};
use card_backend::{CardBackend, CardCaps, CardTransaction, PinType, SmartcardError};
use openpgp_card::state::Open;
use sioul_core::securitykey::{Card, CardError, Readers, Visit, Visited};
use std::sync::OnceLock;
use std::time::Duration;

/// Whether this phone can reach a key now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// No NFC in this phone.
    NoNfc,
    /// NFC switched off.
    Off,
    /// Ready: a key held to the phone is read.
    Ready,
}

/// A key held against the phone, as the phone carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    /// The longest exchange the phone carries (IsoDep's `getMaxTransceiveLength`).
    pub max_transceive: usize,
    /// Whether the phone carries extended length APDUs.
    pub extended: bool,
}

/// Why an exchange failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// The key left the field.
    Lost,
    /// Anything else, as a code from the link: never the APDU's bytes.
    Other(String),
}

/// What carries APDUs between Sioul and a key held to the phone: the app's,
/// over Android's NFC; a stand-in in tests.
pub trait Link: Send + Sync {
    /// Whether the phone can reach a key now.
    fn state(&self) -> LinkState;
    /// The key held now, if any.
    fn held(&self) -> Option<Held>;
    /// One APDU to the key held; its answer, status words included.
    fn transceive(&self, command: &[u8]) -> Result<Vec<u8>, LinkError>;
    /// Waits at most `timeout` for a key to be held.
    fn wait(&self, timeout: Duration);
    /// A key is wanted now, or no longer: while it is, the phone reads keys
    /// in reader mode, ahead of other apps; no longer, the key held is let
    /// go (Android hands it to no other app until it leaves the field) and
    /// reader mode ends.
    fn want(&self, wanted: bool);
}

static LINK: OnceLock<&'static dyn Link> = OnceLock::new();

/// The app's link to keys held to the phone, given once as it starts.
pub fn set_link(link: &'static dyn Link) {
    let _ = LINK.set(link);
}

/// The link given, if any.
pub fn link() -> Option<&'static dyn Link> {
    LINK.get().copied()
}

/// Keys held against the phone, behind `openpgp-card`.
pub struct PhoneReaders {
    link: &'static dyn Link,
}

impl PhoneReaders {
    /// Keys reached through `link`.
    pub const fn new(link: &'static dyn Link) -> PhoneReaders {
        PhoneReaders { link }
    }

    fn ready(&self) -> Result<(), CardError> {
        match self.link.state() {
            LinkState::NoNfc => Err(CardError::NoNfc),
            LinkState::Off => Err(CardError::NfcOff),
            LinkState::Ready => Ok(()),
        }
    }
}

/// The name a key held to the phone goes by: none, since it leaves the field
/// after each use (the PIN memory never ties itself to it).
pub const HELD: &str = "";

impl Readers for PhoneReaders {
    fn present(&self) -> Result<Vec<String>, CardError> {
        self.ready()?;
        Ok(self.link.held().map(|_| vec![HELD.to_string()]).unwrap_or_default())
    }

    fn visit(&self, visit: &mut dyn FnMut(&mut dyn Card) -> Visit) -> Result<Visited, CardError> {
        self.ready()?;
        let Some(held) = self.link.held() else { return Ok(Visited::default()) };
        let backend: Box<dyn CardBackend + Send + Sync> = Box::new(PhoneBackend { link: self.link, held });
        let opened = openpgp_card::Card::<Open>::new(backend).map_err(|e| match card_error(e, false) {
            CardError::Removed => CardError::Removed,
            // It answered, but not as an OpenPGP card: a bank card, a transport card.
            _ => CardError::NotOpenPgp,
        });
        let result = opened.and_then(|mut opened| {
            let transaction = opened.transaction().map_err(|e| card_error(e, false))?;
            let mut card = OpenCard::new(transaction, HELD.to_string());
            let _ = visit(&mut card);
            Ok(Visited { opened: 1, busy: 0 })
        });
        unverify(self.link);
        result
    }

    fn wait_change(&self, timeout: Duration) {
        self.link.wait(timeout);
    }

    fn taps(&self) -> bool {
        true
    }

    fn want(&self, wanted: bool) {
        self.link.want(wanted);
    }
}

/// The PIN's two checks undone (VERIFY, P1 = FF, for PW1 in modes 81 and
/// 82), so that the key stays unlocked for no one once Sioul is done with it.
/// A key that does not take it answers so, and the end of the field ends its
/// checks anyway.
fn unverify(link: &dyn Link) {
    for mode in [0x81, 0x82] {
        let _ = link.transceive(&[0x00, 0x20, 0xFF, mode]);
    }
}

/// What a phone carries in one exchange, for `openpgp-card`: extended length
/// only where both the key and the phone take it, each exchange within the
/// phone's frame; else short APDUs (255 bytes of data, 256 of answer).
pub fn limit(caps: CardCaps, held: Held) -> CardCaps {
    let extended = caps.ext_support() && held.extended;
    // The header and the lengths of an extended APDU take nine bytes at most.
    let room = u16::try_from(held.max_transceive.saturating_sub(9)).unwrap_or(u16::MAX);
    let (command, answer) = if extended { (caps.max_cmd_bytes().min(room), caps.max_rsp_bytes().min(room)) } else { (caps.max_cmd_bytes().min(255), caps.max_rsp_bytes().min(256)) };
    CardCaps::new(extended, caps.chaining_support(), command, answer, caps.pw1_max_len(), caps.pw3_max_len())
}

struct PhoneBackend {
    link: &'static dyn Link,
    held: Held,
}

impl CardBackend for PhoneBackend {
    fn limit_card_caps(&self, caps: CardCaps) -> CardCaps {
        limit(caps, self.held)
    }

    fn transaction(&mut self, _reselect: Option<&[u8]>) -> Result<Box<dyn CardTransaction + Send + Sync + '_>, SmartcardError> {
        Ok(Box::new(PhoneTransaction { link: self.link }))
    }
}

struct PhoneTransaction {
    link: &'static dyn Link,
}

impl CardTransaction for PhoneTransaction {
    fn transmit(&mut self, command: &[u8], _buf_size: usize) -> Result<Vec<u8>, SmartcardError> {
        self.link.transceive(command).map_err(|e| match e {
            // Said as a reader says a card taken out (`ocard::card_error`).
            LinkError::Lost => SmartcardError::Error("RemovedCard: the key moved away from the phone".into()),
            LinkError::Other(code) => SmartcardError::Error(code),
        })
    }

    fn feature_pinpad_verify(&self) -> bool {
        false
    }

    fn feature_pinpad_modify(&self) -> bool {
        false
    }

    fn pinpad_verify(&mut self, _pin: PinType, _card_caps: &Option<CardCaps>) -> Result<Vec<u8>, SmartcardError> {
        Err(SmartcardError::Error("no PIN pad".into()))
    }

    fn pinpad_modify(&mut self, _pin: PinType, _card_caps: &Option<CardCaps>) -> Result<Vec<u8>, SmartcardError> {
        Err(SmartcardError::Error("no PIN pad".into()))
    }

    fn was_reset(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "phone_tests.rs"]
mod tests;

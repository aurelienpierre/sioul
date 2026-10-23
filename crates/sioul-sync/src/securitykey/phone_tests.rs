// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The phone's backend against a stand-in: an OpenPGP card that answers APDUs
//! (SELECT, GET DATA, the public keys' reading, VERIFY, PSO, command chaining,
//! GET RESPONSE) over the core's software key, held to a stand-in link that
//! can carry short APDUs only, lose the key after a given exchange, or say
//! NFC is off. Never a real key: invented keys, PIN 123456.

use super::*;
use sioul_core::securitykey::{self as core, CardAccess, CardPublic, CardSigner, KnownCard, Password, SoftCard, ToDecipher, ToSign, Touch};
use std::sync::Mutex;

/// The PIN of every stand-in key here.
const PIN: &str = "123456";

/// An OpenPGP card as its APDUs see it, over the core's software key.
struct ApduCard {
    soft: SoftCard,
    selected: bool,
    /// What a long answer left, for GET RESPONSE.
    pending: Vec<u8>,
    /// What a chained command brought so far.
    chained: Vec<u8>,
    /// PW1 checked, in mode 81 (signing) and 82 (the rest).
    verified: [bool; 2],
    /// Whether it answers as an OpenPGP card at all.
    openpgp: bool,
    /// What it met: chained commands, GET RESPONSEs, extended APDUs, un-verifications.
    chains: usize,
    get_responses: usize,
    extended: usize,
    unverified: usize,
    /// The ephemeral points PSO:DECIPHER was given (ECDH), as the card read them.
    points: Vec<Vec<u8>>,
}

impl ApduCard {
    fn new(soft: SoftCard) -> ApduCard {
        ApduCard { soft, selected: false, pending: Vec::new(), chained: Vec::new(), verified: [false; 2], openpgp: true, chains: 0, get_responses: 0, extended: 0, unverified: 0, points: Vec::new() }
    }

    /// One APDU in, its answer out (ISO/IEC 7816-4 short and extended forms).
    fn answer(&mut self, apdu: &[u8]) -> Vec<u8> {
        if apdu.len() < 4 {
            return vec![0x67, 0x00];
        }
        let (cla, ins, p1, p2) = (apdu[0], apdu[1], apdu[2], apdu[3]);
        let (data, short) = match apdu.len() {
            4 | 5 => (Vec::new(), true),
            _ if apdu[4] != 0 => (apdu[5..5 + usize::from(apdu[4])].to_vec(), true),
            _ if apdu.len() == 7 => (Vec::new(), false),
            _ => {
                let length = usize::from(apdu[5]) << 8 | usize::from(apdu[6]);
                (apdu[7..7 + length].to_vec(), false)
            }
        };
        if !short {
            self.extended += 1;
        }
        if cla & 0x10 != 0 {
            self.chains += 1;
            self.chained.extend_from_slice(&data);
            return vec![0x90, 0x00];
        }
        let data = [std::mem::take(&mut self.chained), data].concat();
        if ins == 0xC0 {
            self.get_responses += 1;
            let rest = std::mem::take(&mut self.pending);
            return self.reply(rest, true);
        }
        let (body, status) = self.command(ins, p1, p2, &data);
        if status != [0x90, 0x00] {
            return status.to_vec();
        }
        self.reply(body, short)
    }

    /// An answer, cut to 256 bytes for a short APDU, the rest given by GET RESPONSE.
    fn reply(&mut self, mut body: Vec<u8>, short: bool) -> Vec<u8> {
        if short && body.len() > 256 {
            self.pending = body.split_off(256);
            let left = u8::try_from(self.pending.len().min(255)).unwrap_or(0);
            body.extend_from_slice(&[0x61, if self.pending.len() >= 256 { 0 } else { left }]);
            return body;
        }
        body.extend_from_slice(&[0x90, 0x00]);
        body
    }

    fn command(&mut self, ins: u8, p1: u8, p2: u8, data: &[u8]) -> (Vec<u8>, [u8; 2]) {
        const OK: [u8; 2] = [0x90, 0x00];
        match ins {
            0xA4 if data == [0xD2, 0x76, 0x00, 0x01, 0x24, 0x01] && self.openpgp => {
                self.selected = true;
                (Vec::new(), OK)
            }
            0xA4 => (Vec::new(), [0x6A, 0x82]),
            _ if !self.selected => (Vec::new(), [0x69, 0x85]),
            0xCA => match (p1, p2) {
                (0x00, 0x6E) => (self.application_data(), OK),
                (0x00, 0x65) => (tlv(&[0x65], &tlv(&[0x5B], self.soft.info.holder.as_bytes())), OK),
                (0x5F, 0x50) => (self.soft.info.url.as_bytes().to_vec(), OK),
                (0x00, 0x7A) => (tlv(&[0x7A], &tlv(&[0x93], &self.soft.info.signatures.to_be_bytes()[1..])), OK),
                _ => (Vec::new(), [0x6A, 0x88]),
            },
            0x47 if p1 == 0x81 => {
                let slot = match data.first() {
                    Some(0xB6) => self.soft.info.sign.clone(),
                    Some(0xB8) => self.soft.info.decrypt.clone(),
                    _ => None,
                };
                match slot.and_then(|s| s.public) {
                    Some(CardPublic::Ecc { point }) => (tlv(&[0x7F, 0x49], &tlv(&[0x86], &point)), OK),
                    Some(CardPublic::Rsa { n, e }) => (tlv(&[0x7F, 0x49], &[tlv(&[0x81], &n), tlv(&[0x82], &e)].concat()), OK),
                    None => (Vec::new(), [0x6A, 0x88]),
                }
            }
            0x20 => {
                let index = usize::from(p2 == 0x82);
                if p1 == 0xFF {
                    self.verified[index] = false;
                    self.unverified += 1;
                    return (Vec::new(), OK);
                }
                if data.is_empty() {
                    return (Vec::new(), if self.verified[index] { OK } else { [0x63, 0xC0 | self.soft.info.pin_tries] });
                }
                let mode = if p2 == 0x81 { core::PinMode::Sign } else { core::PinMode::Decrypt };
                match core::Card::verify(&mut self.soft, &Password::from(data.to_vec()), mode) {
                    Ok(()) => {
                        self.verified[index] = true;
                        (Vec::new(), OK)
                    }
                    Err(CardError::WrongPin { left }) => (Vec::new(), [0x63, 0xC0 | left]),
                    Err(_) => (Vec::new(), [0x69, 0x83]),
                }
            }
            0x2A if (p1, p2) == (0x9E, 0x9A) => {
                if !self.verified[0] {
                    return (Vec::new(), [0x69, 0x82]);
                }
                let input = match digest_info(data) {
                    Some((hash, digest)) => ToSign::Rsa { hash, digest },
                    None => ToSign::Digest(data),
                };
                let signed = core::Card::sign(&mut self.soft, input);
                if self.soft.info.pin_each_signature {
                    self.verified[0] = false;
                }
                match signed {
                    Ok(signature) => (signature, OK),
                    Err(CardError::TouchMissed) => (Vec::new(), [0x66, 0x00]),
                    Err(_) => (Vec::new(), [0x6F, 0x00]),
                }
            }
            0x2A if (p1, p2) == (0x80, 0x86) => {
                if !self.verified[1] {
                    return (Vec::new(), [0x69, 0x82]);
                }
                let opened = match data.split_first() {
                    Some((0x00, ciphertext)) => core::Card::decipher(&mut self.soft, ToDecipher::Rsa(ciphertext)),
                    // The cipher DO, read as a card reads it (OpenPGP card 3.4.1, §7.2.11):
                    // A6 { 7F49 { 86 point } }, each length exact; Curve25519's point is
                    // its 32 bytes, without OpenPGP's 0x40, which a card refuses.
                    Some((0xA6, _)) => match ecdh_point(&data) {
                        Some(point) if point.len() == 32 => {
                            self.points.push(point.to_vec());
                            core::Card::decipher(&mut self.soft, ToDecipher::Ecdh(point))
                        }
                        _ => return (Vec::new(), [0x6A, 0x80]),
                    },
                    _ => return (Vec::new(), [0x6A, 0x80]),
                };
                match opened {
                    Ok(secret) => (secret.to_vec(), OK),
                    Err(CardError::TouchMissed) => (Vec::new(), [0x66, 0x00]),
                    Err(_) => (Vec::new(), [0x6F, 0x00]),
                }
            }
            _ => (Vec::new(), [0x6D, 0x00]),
        }
    }

    /// GET DATA 6E: what the card says of itself, as a YubiKey 5 NFC says it
    /// (openpgp-card's own sample, `tlv.rs`), with this key's fingerprints,
    /// times, algorithms, PIN counters and touch settings.
    fn application_data(&self) -> Vec<u8> {
        let info = &self.soft.info;
        let mut aid = vec![0xD2, 0x76, 0x00, 0x01, 0x24, 0x01, 0x03, 0x04];
        aid.extend_from_slice(&info.manufacturer.to_be_bytes());
        aid.extend_from_slice(&info.serial.to_be_bytes());
        aid.extend_from_slice(&[0x00, 0x00]);
        let fingerprint = |slot: &Option<core::SlotInfo>| slot.as_ref().map(|s| hex(&s.fingerprint)).unwrap_or_else(|| vec![0; 20]);
        let time = |slot: &Option<core::SlotInfo>| slot.as_ref().map_or(0, |s| s.created).to_be_bytes();
        let algorithm = |slot: &Option<core::SlotInfo>, decrypts: bool| match slot.as_ref().and_then(|s| s.public.clone()) {
            Some(CardPublic::Rsa { n, .. }) => {
                let bits = u16::try_from(n.len() * 8).unwrap_or(2048).to_be_bytes();
                vec![0x01, bits[0], bits[1], 0x00, 0x11, 0x00]
            }
            _ if decrypts => vec![0x12, 0x2B, 0x06, 0x01, 0x04, 0x01, 0x97, 0x55, 0x01, 0x05, 0x01],
            _ => vec![0x16, 0x2B, 0x06, 0x01, 0x04, 0x01, 0xDA, 0x47, 0x0F, 0x01],
        };
        let touch = |slot: &Option<core::SlotInfo>| {
            let policy = match slot.as_ref().map_or(Touch::Off, |s| s.touch) {
                Touch::Off => 0,
                Touch::On => 1,
                Touch::Fixed => 2,
                Touch::Cached => 3,
                Touch::CachedFixed => 4,
                Touch::Unknown => 0x7F,
            };
            vec![policy, 0x20]
        };
        let pw_status = vec![u8::from(!info.pin_each_signature), 0x7F, 0x7F, 0x7F, info.pin_tries, info.reset_tries, info.admin_tries];
        let discretionary = [
            // Extended capabilities: no KDF (its bit off), the rest as a YubiKey 5.
            tlv(&[0xC0], &[0x7C, 0x00, 0x0B, 0xFE, 0x08, 0x00, 0x00, 0xFF, 0x00, 0x00]),
            tlv(&[0xC1], &algorithm(&info.sign, false)),
            tlv(&[0xC2], &algorithm(&info.decrypt, true)),
            tlv(&[0xC3], &algorithm(&None, false)),
            tlv(&[0xC4], &pw_status),
            tlv(&[0xC5], &[fingerprint(&info.sign), fingerprint(&info.decrypt), vec![0; 20]].concat()),
            tlv(&[0xC6], &[0; 60]),
            tlv(&[0xCD], &[time(&info.sign), time(&info.decrypt), [0; 4]].concat()),
            tlv(&[0x7F, 0x66], &[0x02, 0x02, 0x0B, 0xFE, 0x02, 0x02, 0x0B, 0xFE]),
            tlv(&[0xD6], &touch(&info.sign)),
            tlv(&[0xD7], &touch(&info.decrypt)),
            tlv(&[0xD8], &[0x00, 0x20]),
        ]
        .concat();
        let inner = [tlv(&[0x4F], &aid), tlv(&[0x5F, 0x52], &[0x00, 0x73, 0x00, 0x00, 0xE0, 0x05, 0x90, 0x00]), tlv(&[0x7F, 0x74], &[0x81, 0x01, 0x20]), tlv(&[0x73], &discretionary)].concat();
        tlv(&[0x6E], &inner)
    }
}

/// A BER-TLV: the tag, the length (short or long form), the value.
fn tlv(tag: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    match value.len() {
        n if n < 0x80 => out.push(u8::try_from(n).unwrap_or(0)),
        n if n < 0x100 => out.extend_from_slice(&[0x81, u8::try_from(n).unwrap_or(0)]),
        n => out.extend_from_slice(&[0x82, u8::try_from(n >> 8).unwrap_or(0), u8::try_from(n & 0xFF).unwrap_or(0)]),
    }
    out.extend_from_slice(value);
    out
}

/// One BER-TLV of tag `tag` at the start of `data`, exactly: its value, and what follows it.
fn read_tlv<'a>(data: &'a [u8], tag: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let rest = data.strip_prefix(tag)?;
    let (length, rest) = match *rest.first()? {
        n if n < 0x80 => (usize::from(n), &rest[1..]),
        0x81 => (usize::from(*rest.get(1)?), &rest[2..]),
        0x82 => (usize::from(*rest.get(1)?) << 8 | usize::from(*rest.get(2)?), &rest[3..]),
        _ => return None,
    };
    (rest.len() >= length).then(|| rest.split_at(length))
}

/// The point of an ECDH cipher DO, A6 { 7F49 { 86 point } }, nothing around or after it.
fn ecdh_point(data: &[u8]) -> Option<&[u8]> {
    let (cipher, after) = read_tlv(data, &[0xA6])?;
    let (public, inside) = read_tlv(cipher, &[0x7F, 0x49])?;
    let (point, last) = read_tlv(public, &[0x86])?;
    (after.is_empty() && inside.is_empty() && last.is_empty()).then_some(point)
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len() / 2).filter_map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()).collect()
}

/// A DigestInfo (RSA's PSO:CDS input), split into its hash and its digest.
fn digest_info(data: &[u8]) -> Option<(sioul_core::securitykey::HashAlgorithm, &[u8])> {
    use sioul_core::securitykey::HashAlgorithm as H;
    for (hash, prefix) in [
        (H::SHA256, &[0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20][..]),
        (H::SHA384, &[0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02, 0x05, 0x00, 0x04, 0x30][..]),
        (H::SHA512, &[0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03, 0x05, 0x00, 0x04, 0x40][..]),
    ] {
        if let Some(digest) = data.strip_prefix(prefix) {
            return Some((hash, digest));
        }
    }
    None
}

/// A phone's NFC and USB-C port as a test sees them: a key held or plugged
/// in, or not, what the phone carries, the key lost after a given number of
/// exchanges, Android's refusal or another app holding a key plugged in.
struct StandIn {
    card: Mutex<Option<ApduCard>>,
    state: Mutex<LinkState>,
    held: Mutex<Option<Held>>,
    /// Whether the phone takes a key plugged in.
    usb_host: Mutex<bool>,
    /// A key plugged in, refused or held by another app, when none is in reach.
    plugged: Mutex<Reach>,
    /// Exchanges left before the key leaves the field.
    lose_after: Mutex<Option<usize>>,
    /// Every command sent, its first four bytes (never a PIN).
    headers: Mutex<Vec<[u8; 4]>>,
    /// A hostile card: every answer says more bytes wait (61FF), without end.
    endless: Mutex<bool>,
    wanted: Mutex<Vec<bool>>,
}

impl StandIn {
    fn holding(card: ApduCard, held: Held) -> &'static StandIn {
        Box::leak(Box::new(StandIn {
            card: Mutex::new(Some(card)),
            state: Mutex::new(LinkState::Ready),
            held: Mutex::new(Some(held)),
            usb_host: Mutex::new(held.via == Via::Usb),
            plugged: Mutex::new(Reach::None),
            lose_after: Mutex::new(None),
            headers: Mutex::new(Vec::new()),
            endless: Mutex::new(false),
            wanted: Mutex::new(Vec::new()),
        }))
    }

    fn card<R>(&self, f: impl FnOnce(&mut ApduCard) -> R) -> R {
        f(self.card.lock().unwrap().as_mut().unwrap())
    }
}

impl Link for StandIn {
    fn state(&self) -> LinkState {
        *self.state.lock().unwrap()
    }

    fn usb_host(&self) -> bool {
        *self.usb_host.lock().unwrap()
    }

    fn reach(&self) -> Reach {
        match *self.held.lock().unwrap() {
            Some(held) => Reach::Held(held),
            None => *self.plugged.lock().unwrap(),
        }
    }

    fn transceive(&self, command: &[u8]) -> Result<Vec<u8>, LinkError> {
        if self.held.lock().unwrap().is_none() {
            return Err(LinkError::Lost);
        }
        let mut lose = self.lose_after.lock().unwrap();
        if let Some(left) = lose.as_mut() {
            if *left == 0 {
                *self.held.lock().unwrap() = None;
                return Err(LinkError::Lost);
            }
            *left -= 1;
        }
        assert!(command.len() <= self.held.lock().unwrap().map_or(0, |h| h.max_transceive), "more than the phone carries: {} bytes", command.len());
        self.headers.lock().unwrap().push([command[0], command[1], command[2], command[3]]);
        if *self.endless.lock().unwrap() && command[1] != 0xA4 {
            return Ok([vec![0xAA; 255], vec![0x61, 0xFF]].concat());
        }
        Ok(self.card.lock().unwrap().as_mut().map(|card| card.answer(command)).unwrap_or_else(|| vec![0x6F, 0x00]))
    }

    fn wait(&self, _timeout: Duration) {}

    fn want(&self, wanted: bool) {
        self.wanted.lock().unwrap().push(wanted);
    }
}

/// A phone as most carry: short APDUs only, 261 bytes an exchange.
const SHORT: Held = Held { max_transceive: 261, extended: false, via: Via::Nfc };
/// A phone that carries extended length.
const LONG: Held = Held { max_transceive: 4096, extended: true, via: Via::Nfc };
/// A key plugged into the USB-C port: short APDUs, as SecurityKeyLink.java gives them over CCID.
const PLUGGED: Held = Held { max_transceive: 271, extended: false, via: Via::Usb };

fn ed25519_key(touch: Touch) -> (SoftCard, sequoia_openpgp::Cert) {
    let (mut card, public) = SoftCard::generate("0006:12345678", &["Noa <noa@example.org>"], PIN).unwrap();
    if let Some(slot) = card.info.sign.as_mut() {
        slot.touch = touch;
    }
    (card, sioul_core::pgp::certificates_in(&public).remove(0))
}

/// A usual kind of key: Ed25519 signs, Cv25519 decrypts, X25519 lent
/// to the stand-in (x25519-dalek; the core keeps its own for its tests).
fn ed25519_cv25519_key() -> (SoftCard, sequoia_openpgp::Cert) {
    let (card, cert) = ed25519_key(Touch::On);
    (card.with_x25519(|scalar, point| x25519_dalek::x25519(*scalar, *point)), cert)
}

fn rsa_key() -> (SoftCard, sequoia_openpgp::Cert) {
    let (card, public) = SoftCard::generate_rsa("0006:12345678", &["Noa <noa@example.org>"], PIN).unwrap();
    (card, sioul_core::pgp::certificates_in(&public).remove(0))
}

fn signing_key(cert: &sequoia_openpgp::Cert) -> core::PublicKey {
    let policy = sequoia_openpgp::policy::StandardPolicy::new();
    cert.keys().with_policy(&policy, None).for_signing().next().unwrap().key().clone().role_into_unspecified()
}

/// Signs a digest through `readers`, as Send does.
fn sign(readers: &PhoneReaders, cert: &sequoia_openpgp::Cert, pin: &str) -> Result<(), CardError> {
    use sequoia_openpgp::crypto::Signer as _;
    let pin = Password::from(pin);
    let access = CardAccess::new(readers, &pin, &|| {});
    let mut signer = CardSigner::new(&access, "0006:12345678", signing_key(cert));
    let digest = [7u8; 64];
    signer.sign(sequoia_openpgp::types::HashAlgorithm::SHA512, &digest).map(|_| ()).map_err(|_| access.failure().unwrap_or(CardError::Other("no failure kept".into())))
}

/// Opens a message encrypted to the key, as "Open with your security key" does.
fn open(readers: &PhoneReaders, cert: &sequoia_openpgp::Cert, pin: &str) -> Result<Vec<u8>, CardError> {
    use sioul_core::pgp::{self, Keys, SessionKeys, Unlock};
    let sealed = pgp::encrypt(b"The lease, signed.", &[cert], None).unwrap();
    let policy = sequoia_openpgp::policy::StandardPolicy::new();
    let fingerprint = |signing: bool| {
        cert.keys().with_policy(&policy, None).find(|k| if signing { k.for_signing() } else { k.for_transport_encryption() }).map(|k| k.key().fingerprint().to_hex()).unwrap_or_default()
    };
    let known = KnownCard { ident: "0006:12345678".into(), sign: fingerprint(true), decrypt: fingerprint(false), cert: cert.fingerprint().to_hex(), addresses: vec!["noa@example.org".into()], ..KnownCard::default() };
    let keys = Keys { own: vec![cert.clone()], cards: vec![known], ..Keys::default() };
    let pin = Password::from(pin);
    let access = CardAccess::new(readers, &pin, &|| {});
    let none = |_: &pgp::Fingerprint| None;
    let sessions = SessionKeys::new();
    match pgp::decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions).with_card(&access)) {
        Ok(opened) => Ok(opened.data),
        Err(_) => Err(access.failure().unwrap_or(CardError::Other("not opened".into()))),
    }
}

/// One tap, on a phone that carries short APDUs only: the key is read (its
/// long self-description through GET RESPONSE), signs (Ed25519) and, an RSA
/// key, opens a message whose ciphertext needs command chaining; its PIN's
/// checks are undone after each use.
#[test]
fn a_key_held_to_the_phone_signs_and_opens_in_short_apdus() {
    let (card, cert) = ed25519_key(Touch::On);
    let link = StandIn::holding(ApduCard::new(card), SHORT);
    let readers = PhoneReaders::new(link);
    assert!(readers.taps());
    let info = core::cards(&readers).unwrap().remove(0);
    assert_eq!(info.ident, "0006:12345678");
    assert_eq!(info.reader, HELD, "no reader to forget the PIN by: the key leaves after each use");
    assert_eq!(info.sign.as_ref().map(|s| s.touch), Some(Touch::On));
    assert!(info.sign.as_ref().is_some_and(|s| s.public.is_some()), "its public key read for the setup's check");
    assert!(link.card(|c| c.get_responses) > 0, "its long answer read through GET RESPONSE");
    sign(&readers, &cert, PIN).unwrap();
    assert_eq!(link.card(|c| (c.verified, c.extended)), ([false, false], 0), "checks undone after the tap; no extended APDU");
    let headers = link.headers.lock().unwrap().clone();
    assert_eq!(&headers[headers.len() - 2..], &[[0x00, 0x20, 0xFF, 0x81], [0x00, 0x20, 0xFF, 0x82]], "VERIFY P1=FF last, for both modes");
    let (card, cert) = rsa_key();
    let link = StandIn::holding(ApduCard::new(card), SHORT);
    let readers = PhoneReaders::new(link);
    assert_eq!(open(&readers, &cert, PIN).unwrap(), b"The lease, signed.");
    assert!(link.card(|c| c.chains) > 0, "a 257-byte command chained");
    sign(&readers, &cert, PIN).unwrap();
}

/// A key of that kind, Ed25519 and Cv25519, held to a phone of short APDUs:
/// a message Sioul encrypts opens end to end through PSO:DECIPHER and the
/// ECDH cipher DO (the ephemeral point, 32 bytes without its 0x40, the
/// shared secret back), and a signature made through PSO:CDS verifies with
/// Sequoia against the certificate; a point sent with its 0x40, or a cipher
/// DO with bytes after it, is refused as a card refuses it.
#[test]
fn an_ed25519_and_cv25519_key_opens_and_signs_end_to_end() {
    use sequoia_openpgp::packet::signature::SignatureBuilder;
    use sequoia_openpgp::types::SignatureType;
    let (card, cert) = ed25519_cv25519_key();
    let policy = sequoia_openpgp::policy::StandardPolicy::new();
    let decryption = cert.keys().with_policy(&policy, None).for_transport_encryption().next().unwrap().key().clone();
    assert_eq!(decryption.pk_algo(), sequoia_openpgp::types::PublicKeyAlgorithm::ECDH);
    let link = StandIn::holding(ApduCard::new(card), SHORT);
    let readers = PhoneReaders::new(link);
    let info = core::cards(&readers).unwrap().remove(0);
    assert_eq!((info.sign.as_ref().map(|s| s.algorithm.as_str()), info.decrypt.as_ref().map(|s| s.algorithm.as_str())), (Some("Ed25519"), Some("Curve25519")));
    // Opened: Sioul's own encryption, Sioul's own decryption, the card between.
    assert_eq!(open(&readers, &cert, PIN).unwrap(), b"The lease, signed.");
    let points = link.card(|c| c.points.clone());
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].len(), 32, "the ephemeral point without its 0x40");
    assert_eq!(link.card(|c| c.verified), [false, false], "checks undone after the tap");
    // A wrong PIN opens nothing, and says so.
    assert_eq!(open(&readers, &cert, "654321"), Err(CardError::WrongPin { left: 2 }));
    assert_eq!(open(&readers, &cert, PIN).unwrap(), b"The lease, signed.");
    // Signed through the card (PSO:CDS), verified by Sequoia with the certificate's signing key.
    let pin = Password::from(PIN);
    let access = CardAccess::new(&readers, &pin, &|| {});
    let mut signer = CardSigner::new(&access, "0006:12345678", signing_key(&cert));
    let message = b"Signed on the phone, held against it.";
    #[allow(unused_mut)]
    let mut signature = SignatureBuilder::new(SignatureType::Binary).sign_message(&mut signer, message).unwrap();
    let key = cert.keys().with_policy(&policy, None).for_signing().next().unwrap().key().clone();
    assert_eq!(key.pk_algo(), sequoia_openpgp::types::PublicKeyAlgorithm::EdDSA);
    signature.verify_message(&key, message).unwrap();
    let mut other = signature.clone();
    assert!(other.verify_message(&key, b"Another message.").is_err(), "the signature is this message's");
    other = signature;
    assert!(other.verify_message(&decryption, message).is_err(), "and this key's");
    // As a card reads the cipher DO: exact, the point without 0x40.
    let point = [9u8; 32];
    let good = tlv(&[0xA6], &tlv(&[0x7F, 0x49], &tlv(&[0x86], &point)));
    assert_eq!(ecdh_point(&good), Some(&point[..]));
    let prefixed = tlv(&[0xA6], &tlv(&[0x7F, 0x49], &tlv(&[0x86], &[&[0x40][..], &point].concat())));
    assert_eq!(ecdh_point(&prefixed).map(<[u8]>::len), Some(33));
    let mut card = ApduCard::new(ed25519_cv25519_key().0);
    (card.selected, card.verified) = (true, [true, true]);
    assert_eq!(card.command(0x2A, 0x80, 0x86, &prefixed).1, [0x6A, 0x80], "a point with its 0x40 refused");
    assert_eq!(card.command(0x2A, 0x80, 0x86, &[good.clone(), vec![0]].concat()).1, [0x6A, 0x80], "bytes after the cipher DO refused");
    assert_eq!(card.command(0x2A, 0x80, 0x86, &good[..good.len() - 1]).1, [0x6A, 0x80], "a cipher DO cut short refused");
}

/// A phone that carries extended length: no chaining, no GET RESPONSE.
#[test]
fn a_phone_that_carries_extended_length_uses_it() {
    let (card, cert) = rsa_key();
    let link = StandIn::holding(ApduCard::new(card), LONG);
    let readers = PhoneReaders::new(link);
    assert_eq!(open(&readers, &cert, PIN).unwrap(), b"The lease, signed.");
    assert_eq!(link.card(|c| (c.chains, c.get_responses)), (0, 0));
    assert!(link.card(|c| c.extended) > 0);
}

/// The key moved away before it finished: said as such, at any exchange
/// (while it is read, while it signs); a wrong PIN, with the tries left; a
/// card that is no OpenPGP card; another key than the one known; NFC off,
/// or none.
#[test]
fn what_goes_wrong_over_nfc_is_said() {
    for lost_at in [0, 1, 3, 6, 9] {
        let (card, cert) = ed25519_key(Touch::Off);
        let link = StandIn::holding(ApduCard::new(card), SHORT);
        *link.lose_after.lock().unwrap() = Some(lost_at);
        assert_eq!(sign(&PhoneReaders::new(link), &cert, PIN), Err(CardError::Removed), "lost after {lost_at} exchanges");
    }
    let (card, cert) = ed25519_key(Touch::Off);
    let link = StandIn::holding(ApduCard::new(card), SHORT);
    let readers = PhoneReaders::new(link);
    assert_eq!(sign(&readers, &cert, "654321"), Err(CardError::WrongPin { left: 2 }));
    assert_eq!(sign(&readers, &cert, PIN), Ok(()), "the right one then works, the counter back to full");
    let (card, _) = ed25519_key(Touch::Off);
    let mut bank = ApduCard::new(card);
    bank.openpgp = false;
    let readers = PhoneReaders::new(StandIn::holding(bank, SHORT));
    assert_eq!(core::cards(&readers), Err(CardError::NotOpenPgp));
    let (card, cert) = ed25519_key(Touch::Off);
    let mut other = card;
    other.info.ident = "0006:87654321".into();
    other.info.serial = 0x8765_4321;
    let readers = PhoneReaders::new(StandIn::holding(ApduCard::new(other), SHORT));
    assert_eq!(sign(&readers, &cert, PIN), Err(CardError::NotYours));
    let (card, cert) = ed25519_key(Touch::Off);
    let link = StandIn::holding(ApduCard::new(card), SHORT);
    // NFC off (nothing held then) on a phone that takes no key plugged in.
    *link.state.lock().unwrap() = LinkState::Off;
    *link.held.lock().unwrap() = None;
    assert_eq!(sign(&PhoneReaders::new(link), &cert, PIN), Err(CardError::NfcOff));
    *link.state.lock().unwrap() = LinkState::NoNfc;
    assert_eq!(PhoneReaders::new(link).present(), Err(CardError::NoNfc));
    // Nothing held: no key, as a computer with no key plugged in.
    *link.state.lock().unwrap() = LinkState::Ready;
    *link.held.lock().unwrap() = None;
    assert_eq!(PhoneReaders::new(link).present(), Ok(Vec::new()));
    assert_eq!(sign(&PhoneReaders::new(link), &cert, PIN), Err(CardError::Absent));
}

/// Review of 23 October 2026, F5: a card that answers "more bytes wait"
/// without end is let go after a bounded number of exchanges, said as no
/// OpenPGP card, never read until the key leaves.
#[test]
fn a_card_that_never_ends_its_answer_is_let_go() {
    let (card, _) = ed25519_key(Touch::Off);
    let link = StandIn::holding(ApduCard::new(card), SHORT);
    *link.endless.lock().unwrap() = true;
    let readers = PhoneReaders::new(link);
    assert_eq!(core::cards(&readers), Err(CardError::NotOpenPgp));
    let sent = link.headers.lock().unwrap().len();
    assert!(sent <= MOST_EXCHANGES + 2, "{sent} exchanges");
    assert!(sent >= MOST_EXCHANGES / 2, "{sent} exchanges: the cap was reached, not something else");
}

/// Plugged into the USB-C port: the same visit, PIN first, but the key's
/// touch asked as on a computer (the window's touch line, whatever the
/// setting), and unplugged mid-way said as such; every touch setting goes
/// through; its PIN's checks undone at the end.
#[test]
fn a_key_plugged_into_the_phone_signs_and_opens_its_touch_asked() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (card, cert) = ed25519_cv25519_key();
    let link = StandIn::holding(ApduCard::new(card), PLUGGED);
    let readers = PhoneReaders::new(link);
    assert!(readers.taps() && readers.by_cable());
    assert_eq!(readers.reach_note(), None);
    let pin = Password::from(PIN);
    let touched = AtomicUsize::new(0);
    let touch = || {
        touched.fetch_add(1, Ordering::SeqCst);
    };
    let access = CardAccess::new(&readers, &pin, &touch);
    let mut signer = CardSigner::new(&access, "0006:12345678", signing_key(&cert));
    sequoia_openpgp::crypto::Signer::sign(&mut signer, sequoia_openpgp::types::HashAlgorithm::SHA256, &[7u8; 32]).unwrap();
    assert_eq!(touched.load(Ordering::SeqCst), 1, "its touch asked, as on a computer");
    assert_eq!(open(&readers, &cert, PIN).unwrap(), b"The lease, signed.");
    assert_eq!(link.card(|c| c.verified), [false, false], "checks undone after each use");
    for touch in [Touch::Off, Touch::On, Touch::Fixed, Touch::Cached, Touch::CachedFixed, Touch::Unknown] {
        let (card, cert) = ed25519_key(touch);
        assert_eq!(sign(&PhoneReaders::new(StandIn::holding(ApduCard::new(card), PLUGGED)), &cert, PIN), Ok(()), "{touch:?}");
    }
    // Unplugged mid-way: said as a key gone, by cable.
    let (card, cert) = ed25519_key(Touch::Off);
    let link = StandIn::holding(ApduCard::new(card), PLUGGED);
    *link.lose_after.lock().unwrap() = Some(3);
    let readers = PhoneReaders::new(link);
    assert_eq!(sign(&readers, &cert, PIN), Err(CardError::Removed));
    assert!(readers.by_cable(), "the words say unplugged, not moved away");
    // Held over NFC afterwards: no longer by cable.
    *link.held.lock().unwrap() = Some(SHORT);
    assert!(!readers.by_cable());
}

/// The ways a phone reaches a key, said: Android's dialog refused, another
/// app holding the key plugged in; no NFC or NFC off while a key can still
/// be plugged in (a note for the waiting words, never a failure); neither
/// way at all.
#[test]
fn how_a_phone_reaches_a_key_is_said() {
    let (card, cert) = ed25519_key(Touch::Off);
    let link = StandIn::holding(ApduCard::new(card), PLUGGED);
    *link.held.lock().unwrap() = None;
    let readers = PhoneReaders::new(link);
    *link.plugged.lock().unwrap() = Reach::Refused;
    assert_eq!(readers.present(), Err(CardError::UsbRefused));
    assert_eq!(sign(&readers, &cert, PIN), Err(CardError::UsbRefused));
    *link.plugged.lock().unwrap() = Reach::Busy;
    assert_eq!(readers.present(), Err(CardError::UsbBusy));
    *link.plugged.lock().unwrap() = Reach::None;
    assert_eq!(readers.present(), Ok(Vec::new()));
    for (state, note) in [(LinkState::NoNfc, Some(CardError::NoNfc)), (LinkState::Off, Some(CardError::NfcOff)), (LinkState::Ready, None)] {
        *link.state.lock().unwrap() = state;
        assert_eq!((readers.present(), readers.reach_note()), (Ok(Vec::new()), note), "{state:?} with a USB-C port");
    }
    *link.usb_host.lock().unwrap() = false;
    for (state, said) in [(LinkState::NoNfc, Err(CardError::NoNfc)), (LinkState::Off, Err(CardError::NfcOff)), (LinkState::Ready, Ok(Vec::new()))] {
        *link.state.lock().unwrap() = state;
        assert_eq!((readers.present(), readers.reach_note()), (said, None), "{state:?} without one");
    }
    for error in [CardError::UsbRefused, CardError::UsbBusy] {
        assert_eq!(error.remedy(), core::Remedy::Again);
        let said = error.sentence(&sioul_core::i18n::Translator::new("en"), core::Purpose::Open);
        assert!(!said.is_empty() && !said.starts_with("seckey-"), "{error:?}");
    }
}

/// Every touch setting: over NFC the tap is the touch; a key that refuses
/// anyway (6600 after its PIN) is said as a touch missed, which the window
/// says in the phone's words.
#[test]
fn every_touch_setting_goes_through_and_a_refusal_is_said() {
    for touch in [Touch::Off, Touch::On, Touch::Fixed, Touch::Cached, Touch::CachedFixed, Touch::Unknown] {
        let (card, cert) = ed25519_key(touch);
        let readers = PhoneReaders::new(StandIn::holding(ApduCard::new(card), SHORT));
        assert_eq!(sign(&readers, &cert, PIN), Ok(()), "{touch:?}");
    }
    let (mut card, cert) = ed25519_key(Touch::On);
    card.fault = Some(CardError::TouchMissed);
    let readers = PhoneReaders::new(StandIn::holding(ApduCard::new(card), SHORT));
    assert_eq!(sign(&readers, &cert, PIN), Err(CardError::TouchMissed));
}

/// What a phone carries, given to openpgp-card: extended length only where
/// both take it, within the phone's frame; else short APDUs.
#[test]
fn the_phones_frame_caps_what_is_sent() {
    let card = CardCaps::new(true, true, 3070, 3070, 127, 127);
    let short = limit(card, SHORT);
    assert_eq!((short.ext_support(), short.chaining_support(), short.max_cmd_bytes(), short.max_rsp_bytes()), (false, true, 255, 256));
    let long = limit(card, Held { max_transceive: 1024, extended: true, via: Via::Nfc });
    assert_eq!((long.ext_support(), long.max_cmd_bytes(), long.max_rsp_bytes()), (true, 1015, 1015));
    let roomy = limit(card, Held { max_transceive: 65_279, extended: true, via: Via::Nfc });
    assert_eq!((roomy.max_cmd_bytes(), roomy.max_rsp_bytes()), (3070, 3070));
}

/// Nothing of a key in any log: the phone's backend prints and logs nothing.
#[test]
fn the_phones_backend_logs_nothing() {
    let source = include_str!("phone.rs");
    for word in ["println!", "eprintln!", "print!", "dbg!", "log::", "trace!", "debug!"] {
        assert!(!source.contains(word), "{word}");
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Texts sealed at rest (`sioul_core::texts::Sealer`; docs/texts.md, "Sealed
//! at rest"): each line of the texts' stores, on every device, is sealed by
//! the device that writes it, so that a computer's copy of your texts is
//! unreadable without the sharing key, which each device keeps in its
//! keyring (the Secret Service, the Credential Manager, the Keychain, or
//! Android's KeyStore). The sharing then seals the lines again in the folder,
//! as it seals every record.
//!
//! The key: made from the sharing key (HKDF-SHA-256, "sioul-texts 1"), so
//! that it seals nothing else and the sharing key itself seals no text. Each
//! line: XChaCha20-Poly1305, a random nonce, bound to its use; written
//! "t1:" and the nonce and the sealed bytes in base64, on one line. Each
//! media file of a multimedia message kept on a computer: the same, bound to
//! another use, the nonce and the sealed bytes (`seal_bytes`).

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sha2::Sha256;

/// What a line sealed this way starts with: its format.
const PREFIX: &str = "t1:";
/// What each line is bound to: a line sealed for another use does not open here.
const BOUND: &[u8] = b"sioul-texts line";

/// The sealer of the texts' lines, from the sharing key.
pub struct TextSeal {
    cipher: XChaCha20Poly1305,
}

impl TextSeal {
    pub fn new(sharing_key: &[u8; 32]) -> TextSeal {
        let mut key = [0u8; 32];
        // 32 bytes is always a length HKDF-SHA-256 gives.
        let _ = hkdf::Hkdf::<Sha256>::new(None, sharing_key).expand(b"sioul-texts 1", &mut key);
        TextSeal { cipher: XChaCha20Poly1305::new((&key).into()) }
    }
}

/// What a media file sealed this way is bound to.
const BOUND_FILE: &[u8] = b"sioul-texts media";

impl TextSeal {
    /// A media file's bytes sealed whole: the nonce, then the sealed bytes (a part of a multimedia message is small: carriers cap them).
    pub fn seal_bytes(&self, plain: &[u8]) -> Vec<u8> {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let Ok(sealed) = self.cipher.encrypt(&nonce, Payload { msg: plain, aad: BOUND_FILE }) else { return Vec::new() };
        let mut bytes = nonce.to_vec();
        bytes.extend(sealed);
        bytes
    }

    /// A media file opened; none when it does not open.
    pub fn open_bytes(&self, sealed: &[u8]) -> Option<Vec<u8>> {
        if sealed.len() < 24 {
            return None;
        }
        let (nonce, rest) = sealed.split_at(24);
        self.cipher.decrypt(XNonce::from_slice(nonce), Payload { msg: rest, aad: BOUND_FILE }).ok()
    }
}

impl sioul_core::texts::Sealer for TextSeal {
    fn seal(&self, plain: &str) -> String {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let Ok(sealed) = self.cipher.encrypt(&nonce, Payload { msg: plain.as_bytes(), aad: BOUND }) else { return String::new() };
        let mut bytes = nonce.to_vec();
        bytes.extend(sealed);
        format!("{PREFIX}{}", B64.encode(bytes))
    }

    fn open(&self, sealed: &str) -> Option<String> {
        let bytes = B64.decode(sealed.trim().strip_prefix(PREFIX)?).ok()?;
        if bytes.len() < 24 {
            return None;
        }
        let (nonce, rest) = bytes.split_at(24);
        let plain = self.cipher.decrypt(XNonce::from_slice(nonce), Payload { msg: rest, aad: BOUND }).ok()?;
        String::from_utf8(plain).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::texts::Sealer;

    #[test]
    fn a_line_opens_with_its_key_only_and_never_reads_plain() {
        let seal = TextSeal::new(&[7u8; 32]);
        let line = seal.seal("{\"body\":\"Votre rendez-vous est déplacé.\"}");
        assert!(line.starts_with(PREFIX) && !line.contains("rendez") && !line.contains('\n'));
        assert_eq!(seal.open(&line).as_deref(), Some("{\"body\":\"Votre rendez-vous est déplacé.\"}"));
        assert_ne!(seal.seal("same"), seal.seal("same"), "a nonce of its own each time");
        assert!(TextSeal::new(&[8u8; 32]).open(&line).is_none(), "another vault's key");
        assert!(seal.open("{\"body\":\"plain\"}").is_none());
        // One character changed inside: the seal does not open.
        let mid = line.len() / 2;
        let swapped = if &line[mid..=mid] == "A" { "B" } else { "A" };
        let broken = format!("{}{swapped}{}", &line[..mid], &line[mid + 1..]);
        assert!(seal.open(&broken).is_none());
        // A media file: whole, opened with its key only, never as a line.
        let picture = vec![0x89u8, b'P', b'N', b'G', 1, 2, 3];
        let sealed = seal.seal_bytes(&picture);
        assert!(!sealed.windows(3).any(|w| w == b"PNG"));
        assert_eq!(seal.open_bytes(&sealed), Some(picture));
        assert!(TextSeal::new(&[8u8; 32]).open_bytes(&sealed).is_none());
        assert!(seal.open(&String::from_utf8_lossy(&sealed)).is_none());
    }
}

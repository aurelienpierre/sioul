// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Ids given to what is made (docs/database.md, "The format of what
//! travels"): a money line, a split, a preset, a time session. Your devices
//! merge each by its id (`sioul_sync::share`, format 2), so that two the same
//! stay two, and one changed on two devices stays one.
//!
//! 128 bits, random, as the sharing's own ids are, shaped as a UUID
//! (version 4): two halves hashed by keys the system draws at random for
//! this process (`RandomState`, as `everywhere::new_id` does), over the time,
//! a counter and the process. Nothing of what the id names is in it.

use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

/// A new id: "1c0e5d2a-…-4…", never the same twice.
pub fn new() -> String {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let count = COUNT.fetch_add(1, Ordering::Relaxed);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let half = |which: u8| {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(now);
        hasher.write_u64(count);
        hasher.write_u32(std::process::id());
        hasher.write_u8(which);
        hasher.finish().to_le_bytes()
    };
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&half(0));
    bytes[8..].copy_from_slice(&half(1));
    // Version 4 (random), variant 1, as a UUID says it.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// The id of a money line recorded from a message (its `mid:` or its
/// `sioul:mail/` address), its amount in cents and its day, the same on every
/// device: one payment is recorded once (`budget::record_line`), and two
/// devices that each record it before they meet write one line, which the
/// sharing merges by this id; another part of the message (another amount
/// or day) is another line. Shaped as a UUID (version 5's form, SHA-256 in
/// place of SHA-1); the message's id cannot be read back from it.
pub fn of_message(key: &str, cents: i64, day: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(format!("sioul money line\u{1f}{}\u{1f}{cents}\u{1f}{}", key.trim(), day.trim()).as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_new_each_time_and_shaped_as_uuids() {
        let ids: std::collections::BTreeSet<String> = (0..10_000).map(|_| super::new()).collect();
        assert_eq!(ids.len(), 10_000);
        for id in ids.iter().take(50) {
            assert_eq!(id.len(), 36, "{id}");
            assert_eq!(id.as_bytes()[14], b'4', "{id}");
            assert!(matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'), "{id}");
            assert!(id.chars().all(|c| c == '-' || c.is_ascii_hexdigit()), "{id}");
        }
    }

    #[test]
    fn a_message_s_line_has_one_id_everywhere() {
        let id = super::of_message("mid:receipt-1@shop.example", -4250, "2026-10-21");
        assert_eq!(id, super::of_message("mid:receipt-1@shop.example", -4250, "2026-10-21"), "the same on every device");
        assert_ne!(id, super::of_message("mid:receipt-2@shop.example", -4250, "2026-10-21"));
        assert_ne!(id, super::of_message("mid:receipt-1@shop.example", -1000, "2026-10-21"), "another part, another line");
        assert!(id.len() == 36 && id.as_bytes()[14] == b'5' && !id.contains("receipt"), "{id}");
    }
}

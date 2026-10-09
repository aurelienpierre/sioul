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
}

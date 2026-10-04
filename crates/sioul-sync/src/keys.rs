// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Looking up someone's OpenPGP key, only when you ask: it tells a server to
//! whom you are writing. First their domain's own Web Key Directory
//! (draft-koch-openpgp-webkey-service), which only that domain can fill, then
//! keys.openpgp.org, which publishes a key only for an address that confirmed it.

use crate::SyncError;
use sioul_core::pgp::{self, KeySummary};
use std::time::Duration;

/// The keys found for `address`, kept with the others' keys.
pub fn lookup(address: &str) -> Result<Vec<KeySummary>, SyncError> {
    let address = address.trim();
    // HTTPS to the end: a key fetched in clear could be anyone's.
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).https_only(true).build().into();
    let mut urls: Vec<String> = pgp::wkd_urls(address).map(Vec::from).unwrap_or_default();
    let encoded: String = address.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    urls.push(format!("https://keys.openpgp.org/vks/v1/by-email/{encoded}"));
    let mut last_error = None;
    for url in urls {
        match agent.get(&url).call() {
            Ok(mut response) => {
                let bytes = response.body_mut().read_to_vec().unwrap_or_default();
                let found = pgp::learn(&bytes, address);
                if !found.is_empty() {
                    return Ok(found);
                }
            }
            Err(ureq::Error::StatusCode(_)) => {}
            Err(e) => last_error = Some(e.to_string()),
        }
    }
    match last_error {
        Some(e) => Err(SyncError::Network(e)),
        None => Ok(Vec::new()),
    }
}

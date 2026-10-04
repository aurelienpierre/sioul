// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The weather and its places, asked of Open-Meteo: a place's coordinates
//! (two decimals, about a kilometre) and nothing else. No key, no account.

use crate::SyncError;
use sioul_core::weather::{self, Forecast, Place};
use std::time::Duration;

fn get(url: &str) -> Result<String, SyncError> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into();
    let mut response = agent
        .get(url)
        .header("User-Agent", concat!("Sioul/", env!("CARGO_PKG_VERSION"), " (desktop mail and contacts client)"))
        .call()
        .map_err(|e| SyncError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    if status != 200 {
        return Err(SyncError::Server(format!("Open-Meteo: {status}")));
    }
    Ok(text)
}

/// The forecast at a place, from this hour on.
pub fn forecast(latitude: f64, longitude: f64, now: i64) -> Result<Forecast, SyncError> {
    weather::parse(&get(&weather::query(latitude, longitude))?, now).map_err(SyncError::Server)
}

/// Places by name, in your language.
pub fn places(name: &str, language: &str) -> Result<Vec<Place>, SyncError> {
    weather::parse_places(&get(&weather::place_query(name, language))?).map_err(SyncError::Server)
}

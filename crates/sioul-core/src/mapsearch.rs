// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The map's search service: where an address is (the contacts' map, an
//! event's map) and an office's place and hours (a lookup) are asked, only
//! when the person asks. OpenStreetMap's Nominatim by default. Its usage
//! policy asks that an application can be moved off it, or stopped, at the
//! Foundation's request without an update; Sioul reads nothing from afar, by
//! design, so the person holds that switch (decided 20 November 2026, review
//! 10): one setting (`map.search`, Contacts ⚙), an ordinary shared setting
//! that travels with the others, the same for both uses: its address, or
//! "none", which stops both. And the service's own refusal is obeyed: after
//! a 403 or a 429, it is asked nothing more until the next local day, for
//! both uses, kept in the state folder (`WAIT_FILE`), not in memory only.

use crate::config::MapSettings;
use jiff::civil::Date;
use std::path::{Path, PathBuf};

/// OpenStreetMap's Nominatim, the address asked when the setting says none.
pub const DEFAULT: &str = "https://nominatim.openstreetmap.org/search";
/// The setting's word that stops both uses.
pub const NONE: &str = "none";
/// The setting's key (`[map] search`).
pub const KEY: &str = "map.search";
/// Where a refusal is kept, in the state folder: its day and the address
/// that refused, `2026-11-20 https://nominatim.openstreetmap.org/search`.
pub const WAIT_FILE: &str = "map-search-wait";

/// What the setting says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Service {
    /// Asked at this address.
    At(String),
    /// Never asked: the person chose None.
    Off,
}

/// The service the setting names: none said, OpenStreetMap's; "none", off.
pub fn service(map: &MapSettings) -> Service {
    match map.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Service::At(DEFAULT.to_string()),
        Some(word) if word.eq_ignore_ascii_case(NONE) => Service::Off,
        Some(address) => Service::At(address.to_string()),
    }
}

/// A value typed for the setting, as it is kept: "" (OpenStreetMap's),
/// "none", or a web address that starts with `https://` and names a host.
/// Anything else is refused, in words.
pub fn checked(value: &str, tr: &crate::i18n::Translator) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case(NONE) {
        return Ok(value.to_lowercase());
    }
    if value == DEFAULT {
        return Ok(String::new());
    }
    let host = value.strip_prefix("https://").map(|rest| rest.split(['/', '?', '#']).next().unwrap_or(""));
    match host {
        Some(host) if !host.is_empty() && !host.contains(char::is_whitespace) => Ok(value.to_string()),
        _ => Err(tr.text("map-search-refused", None)),
    }
}

/// Why the service is not asked now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// The person chose None.
    Off,
    /// It refused today (403, 429): asked again tomorrow.
    Waiting,
}

/// The address to ask now, or why none is: the setting, then a refusal of
/// today kept in `state` (the state folder).
pub fn now(map: &MapSettings, state: &Path, today: Date) -> Result<String, Stopped> {
    match service(map) {
        Service::Off => Err(Stopped::Off),
        Service::At(address) if waiting(state, today, &address) => Err(Stopped::Waiting),
        Service::At(address) => Ok(address),
    }
}

fn wait_path(state: &Path) -> PathBuf {
    state.join(WAIT_FILE)
}

/// Whether the service at `address` refused on `today` (kept in `state`).
pub fn waiting(state: &Path, today: Date, address: &str) -> bool {
    let kept = crate::files::read_to_string(wait_path(state)).unwrap_or_default();
    let (day, refused_by) = kept.trim().split_once(' ').unwrap_or((kept.trim(), ""));
    day.parse::<Date>().ok() == Some(today) && refused_by.trim() == address.trim()
}

/// The service at `address` refused on `today`: kept, so that it is asked
/// nothing more before the next local day, whatever asks and whether Sioul
/// restarts. Another address chosen meanwhile is asked.
pub fn refused(state: &Path, today: Date, address: &str) -> std::io::Result<()> {
    crate::files::create_dir_all(state)?;
    crate::files::write(wait_path(state), format!("{today} {}\n", address.trim()))
}

/// Whether an answer's status is the service's refusal (403: blocked; 429: too many).
pub fn is_refusal(status: u16) -> bool {
    matches!(status, 403 | 429)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(search: Option<&str>) -> MapSettings {
        MapSettings { search: search.map(str::to_string), ..MapSettings::default() }
    }

    /// One setting for both uses: OpenStreetMap's by default, another
    /// address, or none; only an https address is kept.
    #[test]
    fn the_setting_names_the_service() {
        assert_eq!(service(&map(None)), Service::At(DEFAULT.into()));
        assert_eq!(service(&map(Some(""))), Service::At(DEFAULT.into()));
        assert_eq!(service(&map(Some("None"))), Service::Off);
        assert_eq!(service(&map(Some("https://geo.example.org/search"))), Service::At("https://geo.example.org/search".into()));
        let tr = crate::i18n::Translator::new("en");
        assert_eq!(checked(" https://geo.example.org/search ", &tr), Ok("https://geo.example.org/search".into()));
        assert_eq!(checked(DEFAULT, &tr), Ok(String::new()), "the default is kept as unsaid");
        assert_eq!(checked("NONE", &tr), Ok("none".into()));
        for refused in ["http://geo.example.org/search", "geo.example.org", "https://", "https://a b/"] {
            assert!(checked(refused, &tr).is_err(), "{refused}");
        }
    }

    /// A refusal stops both uses until the next local day, kept on disk;
    /// another service chosen meanwhile is asked.
    #[test]
    fn a_refusal_lasts_the_day() {
        let state = std::env::temp_dir().join(format!("sioul-mapsearch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&state);
        let today: Date = "2026-11-20".parse().unwrap();
        assert_eq!(now(&map(None), &state, today), Ok(DEFAULT.into()));
        refused(&state, today, DEFAULT).unwrap();
        assert_eq!(now(&map(None), &state, today), Err(Stopped::Waiting));
        assert_eq!(now(&map(Some("https://geo.example.org/search")), &state, today), Ok("https://geo.example.org/search".into()), "another service chosen: asked");
        assert_eq!(now(&map(None), &state, today.tomorrow().unwrap()), Ok(DEFAULT.into()), "the next day, asked again");
        assert_eq!(now(&map(Some("none")), &state, today.tomorrow().unwrap()), Err(Stopped::Off));
        assert!(is_refusal(403) && is_refusal(429) && !is_refusal(500) && !is_refusal(200));
        let _ = std::fs::remove_dir_all(&state);
    }
}

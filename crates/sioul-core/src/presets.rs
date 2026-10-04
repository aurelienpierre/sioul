// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sites people commonly keep, offered when adding one (docs/sites.md,
//! "Presets"): per country, per state or province where it matters, and for
//! everyone (chats, video calls, social networks, dating). Each says its type
//! (how it behaves) and the categories it comes with (what it is about). A plain JSON file anyone can update
//! without building Sioul (`presets/sites.json`; `tools/check-presets.py`
//! checks every address): the newest, by its `checked` date, of the copy
//! built in, one in `$XDG_DATA_DIRS/sioul/presets/`, and yours in
//! `$XDG_DATA_HOME/sioul/presets/`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Words in several languages: "en" → "Bank", "fr" → "Banque".
pub type Words = BTreeMap<String, String>;

/// The words in `language`, else in English, else any.
pub fn words(words: &Words, language: &str) -> String {
    words.get(language).or_else(|| words.get("en")).or_else(|| words.values().next()).cloned().unwrap_or_default()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Presets {
    #[serde(default)]
    pub version: u32,
    /// When the addresses were last checked: "2026-10-04". The newest file wins.
    #[serde(default)]
    pub checked: String,
    /// The categories presets come with, by id: their names in each language.
    #[serde(default)]
    pub tags: BTreeMap<String, Tag>,
    /// For everyone: chats, video calls, social networks, dating.
    #[serde(default)]
    pub international: Vec<Preset>,
    /// By ISO 3166-1 code: "FR", "CA", "US".
    #[serde(default)]
    pub countries: BTreeMap<String, Country>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Tag {
    pub name: Words,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Country {
    pub name: Words,
    #[serde(default)]
    pub sites: Vec<Preset>,
    /// States and provinces, by their ISO 3166-2 code without the country: "QC".
    #[serde(default)]
    pub regions: BTreeMap<String, Region>,
    /// What the regions are called, as a menu says it: "By state", "By province".
    #[serde(default)]
    pub regions_name: Words,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Region {
    pub name: Words,
    #[serde(default)]
    pub sites: Vec<Preset>,
}

/// One site offered.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    /// Where you sign in to your own space, not the home page.
    pub url: String,
    /// One of `sites::TYPES`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The categories it comes with, by id (`Presets::tags`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// What it is for, when not its type's: "work" for a work chat.
    #[serde(default)]
    pub area: String,
    /// The domains its mail comes from, when it announces something waiting there.
    #[serde(default)]
    pub announced_by: Vec<String>,
    #[serde(default)]
    pub about: Words,
    /// Voice and video calls in the browser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<bool>,
}

const BUILT_IN: &str = include_str!("../../../presets/sites.json");

/// The presets built into this Sioul.
pub fn built_in() -> Presets {
    serde_json::from_str(BUILT_IN).unwrap_or_default()
}

/// Where newer presets may be: yours first, then the system's.
fn places() -> Vec<PathBuf> {
    let mut places = vec![crate::config::data_dir().join("presets").join("sites.json")];
    // The system's folders: XDG's; on Windows, none unless said (its lists split at ";").
    let system = std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| if cfg!(windows) { Default::default() } else { "/usr/local/share:/usr/share".into() });
    places.extend(std::env::split_paths(&system).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join("sioul").join("presets").join("sites.json")));
    places
}

/// The newest presets: of the copy built in and those on disk, the one checked last.
pub fn load() -> Presets {
    let mut best = built_in();
    for path in places() {
        if let Some(found) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Presets>(&t).ok())
            && found.checked > best.checked
        {
            best = found;
        }
    }
    best
}

/// Where an offered site comes from: everyone's, a country's, a region's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Scope {
    pub country: String,
    pub region: String,
}

impl Presets {
    /// The sites offered for a country and a region (codes; "" for none): the
    /// region's, the country's, then everyone's, each with where it comes from.
    pub fn offered(&self, country: &str, region: &str) -> Vec<(&Preset, Scope)> {
        let mut out = Vec::new();
        if let Some(c) = self.countries.get(country) {
            if let Some(r) = c.regions.get(region) {
                out.extend(r.sites.iter().map(|p| (p, Scope { country: country.into(), region: region.into() })));
            }
            out.extend(c.sites.iter().map(|p| (p, Scope { country: country.into(), region: String::new() })));
        }
        out.extend(self.international.iter().map(|p| (p, Scope { country: String::new(), region: String::new() })));
        out
    }

    /// What a preset is for: its own area, else as its type goes.
    pub fn area_of(&self, preset: &Preset) -> crate::areas::Area {
        crate::areas::Area::parse(&preset.area).unwrap_or_else(|| crate::sites::usual_area(&preset.kind))
    }

    /// A preset's categories, in your words: "Banque", "Santé".
    pub fn tag_words(&self, preset: &Preset, language: &str) -> Vec<String> {
        preset.tags.iter().map(|id| self.tags.get(id).map_or_else(|| id.clone(), |t| words(&t.name, language))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_presets_read() {
        // Read strictly: a mistake in the file says where.
        let presets: Presets = serde_json::from_str(BUILT_IN).unwrap();
        assert_eq!(presets.version, 1);
        assert!(!presets.checked.is_empty());
        for tag in presets.tags.values() {
            assert!(!words(&tag.name, "fr").is_empty() && !words(&tag.name, "en").is_empty());
        }
        let all = presets.countries.values().flat_map(|c| c.sites.iter().chain(c.regions.values().flat_map(|r| r.sites.iter()))).chain(presets.international.iter());
        for preset in all {
            assert!(preset.url.starts_with("https://"), "{}: {}", preset.name, preset.url);
            assert!(crate::sites::TYPES.contains(&preset.kind.as_str()), "{}: {}", preset.name, preset.kind);
            assert!(preset.tags.iter().all(|t| presets.tags.contains_key(t)), "{}: {:?}", preset.name, preset.tags);
            assert!(preset.area.is_empty() || crate::areas::Area::parse(&preset.area).is_some(), "{}: {}", preset.name, preset.area);
        }
    }

    #[test]
    fn regions_then_the_country_then_everyone() {
        let presets: Presets = serde_json::from_str(r#"{"version":1,"checked":"2026-10-04","tags":{"health":{"name":{"en":"Health","fr":"Santé"}}},"international":[{"name":"Chat","url":"https://chat.example.org/","type":"chat"}],"countries":{"CA":{"name":{"en":"Canada"},"sites":[{"name":"Tax","url":"https://tax.example.org/","type":"mailbox"}],"regions":{"QC":{"name":{"fr":"Québec"},"sites":[{"name":"Health","url":"https://health.example.org/","type":"mailbox","tags":["health"]}]}}}}}"#).unwrap();
        let names: Vec<&str> = presets.offered("CA", "QC").iter().map(|(p, _)| p.name.as_str()).collect();
        assert_eq!(names, vec!["Health", "Tax", "Chat"]);
        assert_eq!(presets.offered("CA", "").len(), 2);
        assert_eq!(presets.offered("", "").len(), 1);
        assert_eq!(words(&presets.countries["CA"].regions["QC"].name, "en"), "Québec", "any language when the one asked is missing");
        assert_eq!(presets.area_of(&presets.international[0]), crate::areas::Area::LEISURE);
        let health = &presets.countries["CA"].regions["QC"].sites[0];
        assert_eq!(presets.tag_words(health, "fr"), vec!["Santé"]);
        assert_eq!(presets.area_of(health), crate::areas::Area::ADMIN, "a mailbox is your admin");
    }
}

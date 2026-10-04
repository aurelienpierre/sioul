// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Where postal addresses are, to show contacts on a map. An address is asked
//! once, when you allow it, of OpenStreetMap's geocoder; the answer (or that
//! there is none) is kept here (`$XDG_CACHE_HOME/sioul/places.toml`), so an
//! address leaves this computer once at most.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A point on the map.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub lat: f64,
    pub lon: f64,
}

/// What is known of each address asked: its place, or that none was found.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Places {
    #[serde(default)]
    found: BTreeMap<String, Place>,
    /// Asked, and not found: not asked again.
    #[serde(default)]
    unknown: Vec<String>,
}

/// An address as it is asked and kept: its lines joined, spaces tidied.
pub fn normalize(address: &str) -> String {
    address.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(", ")
}

impl Places {
    pub fn default_path() -> PathBuf {
        crate::config::cache_dir().join("places.toml")
    }

    pub fn load(path: &Path) -> Places {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
    }

    /// The place of an address, when found.
    pub fn get(&self, address: &str) -> Option<Place> {
        self.found.get(&normalize(address)).copied()
    }

    /// Whether the address was asked already, found or not.
    pub fn asked(&self, address: &str) -> bool {
        let key = normalize(address);
        self.found.contains_key(&key) || self.unknown.contains(&key)
    }

    pub fn set(&mut self, address: &str, place: Option<Place>) {
        let key = normalize(address);
        match place {
            Some(place) => {
                self.unknown.retain(|k| *k != key);
                self.found.insert(key, place);
            }
            None => {
                if !self.unknown.contains(&key) {
                    self.unknown.push(key);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_asked_once() {
        let mut places = Places::default();
        let address = "12 rue de l'Exemple\n  69000   Lyon\nFrance";
        assert_eq!(normalize(address), "12 rue de l'Exemple, 69000 Lyon, France");
        assert!(!places.asked(address));
        places.set(address, Some(Place { lat: 45.76, lon: 4.83 }));
        places.set("Nowhere 1", None);
        assert!(places.asked("12 rue de l'Exemple, 69000 Lyon, France") && places.asked("Nowhere 1"));
        let dir = std::env::temp_dir().join(format!("sioul-places-{}", std::process::id()));
        let path = dir.join("places.toml");
        places.save(&path).unwrap();
        let read = Places::load(&path);
        assert_eq!(read.get(address), Some(Place { lat: 45.76, lon: 4.83 }));
        assert!(read.asked("Nowhere 1") && read.get("Nowhere 1").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

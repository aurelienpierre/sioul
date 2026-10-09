// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Today's own choices: how the day is, and the steps put off for today.
//!
//! The day's weather ("clear", "haze", "fog") is chosen by you, never guessed
//! from what you do: it sets how much the plan puts in a day and, in fog,
//! shows only small steps (docs/tasks.md, after the brain-weather probes of
//! Chen, Meng & Nie, CSCW 2026, and energy accounting). "Not now" puts a step
//! off until tomorrow. A new day starts clean: nothing carries over.

use crate::config::state_dir;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// How the day is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weather {
    #[default]
    Clear,
    Haze,
    Fog,
}

impl Weather {
    /// How many heavy tasks the day takes: two when clear, one in haze, none in fog.
    pub fn heavy(self) -> u32 {
        match self {
            Weather::Clear => 2,
            Weather::Haze => 1,
            Weather::Fog => 0,
        }
    }

    /// The share of the usual room for tasks the day has, in percent.
    pub fn room(self) -> u32 {
        match self {
            Weather::Clear => 100,
            Weather::Haze => 60,
            Weather::Fog => 30,
        }
    }

    pub fn parse(text: &str) -> Weather {
        match text {
            "haze" => Weather::Haze,
            "fog" => Weather::Fog,
            _ => Weather::Clear,
        }
    }
}

/// The steps a fog day shows: this long at most.
pub const FOG_MINUTES: u32 = 15;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Today {
    /// The day these choices are for.
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub weather: Weather,
    /// UIDs put off for today ("Not now").
    #[serde(default)]
    pub aside: BTreeSet<String>,
    /// The file as it was read: saved, only what changed since is written (`filelock::save_merged`).
    #[serde(skip)]
    pub read: crate::filelock::Read,
}

impl Today {
    pub fn default_path() -> PathBuf {
        state_dir().join("today.toml")
    }

    /// Today's choices; yesterday's are forgotten.
    pub fn load(path: &Path, today: Date) -> Today {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let saved: Today = toml::from_str(&text).unwrap_or_default();
        let read = crate::filelock::Read::of(&text);
        if saved.date == today.to_string() { Today { read, ..saved } } else { Today { date: today.to_string(), read, ..Today::default() } }
    }

    /// Written under the file's lock, which the sharing takes too: only what
    /// changed since it was read, over what the file holds now.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        crate::filelock::save_merged(path, &self.read, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_day_starts_clean() {
        let path = std::env::temp_dir().join(format!("sioul-today-{}.toml", std::process::id()));
        let day: Date = "2026-10-03".parse().unwrap();
        let mut today = Today::load(&path, day);
        today.weather = Weather::Fog;
        today.aside.insert("letter".into());
        today.save(&path).unwrap();
        assert_eq!(Today::load(&path, day), today);
        let tomorrow = Today::load(&path, "2026-10-04".parse().unwrap());
        assert_eq!((tomorrow.weather, tomorrow.aside.len()), (Weather::Clear, 0));
        std::fs::remove_file(&path).unwrap();
    }
}

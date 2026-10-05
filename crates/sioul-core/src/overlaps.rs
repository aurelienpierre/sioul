// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Two events at once (docs/client.md, "Calendars"): found among
//! the coming days' events, the time kept before and after each counted
//! (`demands::Margins`): going from one to the other takes time too. Said
//! on the Porch for today and above the agenda, until set aside for good;
//! an event moved is a new question.

use crate::agenda::Occurrence;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Two events that overlap, the earlier first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Overlap {
    /// "<uid>@<start>|<uid>@<start>": what setting it aside keeps.
    pub key: String,
    pub first: Occurrence,
    pub second: Occurrence,
}

/// The time an event holds: from getting ready to coming back.
fn held(event: &Occurrence) -> (i64, i64) {
    (event.start - i64::from(event.margins.before) * 60, event.end + i64::from(event.margins.after) * 60)
}

/// The pairs of `events` that overlap: timed, not cancelled, lasting some
/// time (a reminder at 10:00 holds no time), each pair once.
pub fn overlaps(events: &[Occurrence]) -> Vec<Overlap> {
    let mut timed: Vec<&Occurrence> = events.iter().filter(|e| !e.cancelled && !e.all_day && e.end > e.start).collect();
    timed.sort_by(|a, b| (a.start, &a.uid).cmp(&(b.start, &b.uid)));
    let mut out = Vec::new();
    for (i, first) in timed.iter().enumerate() {
        let (from, to) = held(first);
        for second in &timed[i + 1..] {
            // The same event seen twice (in two calendars) is no double booking.
            if second.uid == first.uid && second.start == first.start {
                continue;
            }
            let (other_from, other_to) = held(second);
            if other_from < to && from < other_to {
                out.push(Overlap { key: format!("{}@{}|{}@{}", first.uid, first.start, second.uid, second.start), first: (*first).clone(), second: (*second).clone() });
            }
        }
    }
    out
}

/// The overlaps set aside for good, on this device
/// (`$XDG_STATE_HOME/sioul/overlaps-set-aside.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAside {
    #[serde(default)]
    pub keys: BTreeSet<String>,
}

impl SetAside {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("overlaps-set-aside.toml")
    }

    pub fn load(path: &Path) -> SetAside {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        std::fs::write(path, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demands::Margins;

    fn event(uid: &str, start: i64, end: i64, margins: Margins) -> Occurrence {
        Occurrence { uid: uid.into(), key: format!("{uid}.ics"), summary: uid.into(), start, end, margins, ..Occurrence::default() }
    }

    #[test]
    fn two_events_at_once() {
        let none = Margins::default();
        // 9–10 and 9:30–10:30 overlap; 10:30–11 only touches the second.
        let events = [event("a", 32_400, 36_000, none), event("b", 34_200, 37_800, none), event("c", 37_800, 39_600, none)];
        let found = overlaps(&events);
        assert_eq!(found.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["a@32400|b@34200"]);
        // With half an hour to get to the third, it overlaps the second too.
        let events = [event("b", 34_200, 37_800, none), event("c", 37_800, 39_600, Margins { before: 30, after: 0 })];
        assert_eq!(overlaps(&events).len(), 1);
        // Cancelled, all day, a moment (a reminder), the same one twice: none.
        let cancelled = Occurrence { cancelled: true, ..event("d", 32_400, 36_000, none) };
        let moment = event("e", 34_200, 34_200, none);
        let twice = [event("a", 32_400, 36_000, none), event("a", 32_400, 36_000, none)];
        assert!(overlaps(&[event("a", 32_400, 36_000, none), cancelled, moment]).is_empty());
        assert!(overlaps(&twice).is_empty());
        // Set aside for good: kept in a file.
        let dir = std::env::temp_dir().join(format!("sioul-overlaps-{}", std::process::id()));
        let path = dir.join("set-aside.toml");
        let mut aside = SetAside::load(&path);
        aside.keys.insert(found[0].key.clone());
        aside.save(&path).unwrap();
        assert!(SetAside::load(&path).keys.contains("a@32400|b@34200"));
        let _ = std::fs::remove_dir_all(dir);
    }
}

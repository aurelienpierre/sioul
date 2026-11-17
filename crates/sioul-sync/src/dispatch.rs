// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each device is for, as this device knows the others (docs/areas.md,
//! "What each device is for"): the lines of the devices that count, for what
//! this device shows and tells now (`sioul_core::areas::Lines`,
//! `quiet::Mode::on`) and for which device tells a thing (`lease::Dispatch`).
//!
//! Three sources, all on this device, none needing the sharing's key, so
//! that every process reads them alike (the window, a phone's background
//! step and its listener of other apps, the command line):
//! - **what you said of each device**, in the shared settings
//!   (`[device."<id>"]`, `sioul_core::device_said`);
//! - **this device's id** in the sharing (`share::Here`, `here.toml`);
//! - **what this device last kept of the others** (`peers::Peers`,
//!   `peers.json`, which the window learns for the doses): each one's newest
//!   entry, with its word that its Sioul follows what devices are for
//!   (`devices::Entry::areas`), and whether it still counts.
//!
//! A device **counts** as it does for the doses (`sioul_core::health::counts`:
//! not said off, still sharing, heard within a week) and while it is not
//! forgotten here. One that counts with an older Sioul is named apart
//! (`Known::older`): it tells by the rule before, and nothing it could be for
//! is counted as held by a device (`sioul_core::areas::Here::among`). With
//! sharing off, this device is alone, and its line is still read.
//!
//! Nothing here reads the doses, and the doses read nothing here.

use crate::lease::Dispatch;
use crate::peers::Peers;
use sioul_core::areas::{Line, Lines, Time, Week};
use sioul_core::config::Config;
use sioul_core::device_said::Said;
use std::collections::BTreeMap;
use std::path::Path;

/// The devices that count, as this device knows them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Known {
    /// This device's id in the sharing; "" before it has one.
    pub me: String,
    /// The line of each device that counts and whose Sioul follows what
    /// devices are for, by its id; this device always among them.
    pub lines: BTreeMap<String, Line>,
    /// The ids of the devices that count whose Sioul is older.
    pub older: Vec<String>,
}

impl Known {
    /// From what you said of each device (`said`) and what this device kept
    /// of the others (`kept`; none with sharing off), at `now` (this
    /// device's clock, Unix seconds).
    pub fn of(me: &str, said: &BTreeMap<String, Said>, kept: Option<&Peers>, now: i64) -> Known {
        let line = |id: &str| said.get(id).map(Said::line).unwrap_or_default();
        let mut lines = BTreeMap::from([(me.to_string(), line(me))]);
        let mut older = Vec::new();
        for (id, seen) in kept.iter().flat_map(|kept| kept.peers.iter()) {
            let counts = !id.is_empty() && id != me && seen.forgotten.is_none() && !seen.peer.forgotten && sioul_core::health::counts(&seen.peer, now);
            if !counts {
                continue;
            }
            if seen.entry.as_ref().is_some_and(|entry| entry.areas >= crate::devices::AREAS) {
                lines.insert(id.clone(), line(id));
            } else {
                older.push(id.clone());
            }
        }
        Known { me: me.to_string(), lines, older }
    }

    /// Read from this device's files (`state`: Sioul's state folder): its id,
    /// and, while a sharing folder is set, what it kept of the others.
    pub fn read(config: &Config, state: &Path, now: i64) -> Known {
        let here = crate::share::Here::load(state);
        let kept = here.folder_path().is_some().then(|| Peers::load(&Peers::path(state)));
        Known::of(&here.id, &config.devices, kept.as_ref(), now)
    }

    /// This device's line.
    pub fn mine(&self) -> Line {
        self.lines.get(&self.me).copied().unwrap_or_default()
    }

    /// The lines as the rule reads them on this device (`quiet::Mode::on`).
    pub fn lines(&self) -> Lines {
        Lines { mine: self.mine(), others: self.lines.iter().filter(|(id, _)| **id != self.me).map(|(_, line)| *line).collect() }
    }

    /// The same for the claims' reading, at a time (`lease::told_by`).
    pub fn dispatch(&self, time: Time, week: Week) -> Dispatch {
        Dispatch { time, week, lines: self.lines.clone() }
    }

    /// Whether the device `id` counts and follows what devices are for.
    pub fn follows(&self, id: &str) -> bool {
        self.lines.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::Entry;
    use crate::peers::{Mark, PeerSeen};
    use sioul_core::areas::Area;
    use sioul_core::health::{Peer, Said as Session};

    fn said(pairs: &[(&str, &str)]) -> BTreeMap<String, Said> {
        pairs.iter().map(|(id, area)| (id.to_string(), Said { area: Some(area.to_string()), ..Said::default() })).collect()
    }

    /// A device as kept here: heard at `heard`, its entry saying `areas`.
    fn seen(id: &str, heard: i64, areas: u32) -> PeerSeen {
        let session = Session { started: heard - 600, closed: 0, working: true, exported: heard, imported: heard, doses: true, left: false, phone: false };
        PeerSeen { peer: Peer { id: id.into(), name: id.into(), heard, seen: heard, said: Some(session), ..Peer::default() }, entry: Some(Entry { id: id.into(), working: true, exported: heard, areas, ..Entry::default() }), ..PeerSeen::default() }
    }

    /// The devices that count: this one, always; another while it is heard
    /// within a week, not said off, not forgotten, still sharing; one whose
    /// Sioul is older apart. With sharing off, this device alone.
    #[test]
    fn the_devices_that_count_are_those_the_doses_count() {
        let now = 1_800_000_000;
        let week = 7 * 86_400;
        let said = said(&[("desk", "work"), ("phone", "admin+leisure"), ("old", "leisure"), ("gone", "leisure")]);
        let mut kept = Peers::default();
        kept.peers.insert("phone".into(), seen("phone", now - 60, 1));
        kept.peers.insert("old".into(), seen("old", now - 60, 0));
        kept.peers.insert("gone".into(), seen("gone", now - week - 3600, 1));
        kept.peers.insert("off".into(), PeerSeen { peer: Peer { off: true, ..seen("off", now - 60, 1).peer }, ..seen("off", now - 60, 1) });
        kept.peers.insert("forgotten".into(), PeerSeen { forgotten: Some(Mark::default()), ..seen("forgotten", now - 60, 1) });
        let left = seen("left", now - 60, 1);
        kept.peers.insert("left".into(), PeerSeen { peer: Peer { said: left.peer.said.clone().map(|s| Session { left: true, ..s }), ..left.peer.clone() }, ..left });
        let known = Known::of("desk", &said, Some(&kept), now);
        assert_eq!(known.lines.keys().map(String::as_str).collect::<Vec<_>>(), ["desk", "phone"]);
        assert_eq!(known.older, ["old"]);
        assert_eq!((known.mine(), known.lines().others), (Line::for_area(Area::WORK), vec![Line::for_area(Area::PERSONAL)]));
        assert!(known.follows("phone") && !known.follows("old") && !known.follows("gone"));
        // The phone silent a week: only the desk counts, and what the phone was for is held by no device.
        let later = Known::of("desk", &said, Some(&kept), now + week + 3600);
        assert_eq!(later.lines.keys().map(String::as_str).collect::<Vec<_>>(), ["desk"]);
        let hours = Week { work_hours: true, admin_hours: true, ..Week::default() };
        assert_eq!((known.lines().here(hours).unheld, later.lines().here(hours).unheld), (Area::default(), Area::PERSONAL));
        // Sharing off: this device alone, its own line still read.
        let alone = Known::of("desk", &said, None, now);
        assert_eq!((alone.lines.len(), alone.mine(), alone.older.len()), (1, Line::for_area(Area::WORK), 0));
        // Nothing said of anyone: lines that say nothing.
        assert!(Known::of("desk", &BTreeMap::new(), Some(&kept), now).lines().say_nothing());
        // For the claims: the same lines, at a time.
        let dispatch = known.dispatch(Time::Leisure, hours);
        assert!(dispatch.brings("phone", Area::LEISURE) && !dispatch.brings("desk", Area::LEISURE) && !dispatch.brings("old", Area::LEISURE));
    }
}

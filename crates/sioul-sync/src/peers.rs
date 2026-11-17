// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What this device saw of your other devices, kept here and never shared
//! (`$XDG_STATE_HOME/sioul/share/peers.json`; docs/database.md, "Devices"):
//! each one's newest entry read, its claims, how late its news comes, and
//! what you said of it here (off, forgotten).
//!
//! The window learns it and writes it, for the doses (sioul-app's
//! `health::know`, under the file's lock: a phone's background service
//! learns too). It is read here as well, never written, for the table of
//! your devices (`device_table`), which the command line prints without a
//! window. The structures are the window's own, moved here as they were on
//! 17 November 2026: the same fields under the same names, so the file
//! reads and writes as before.

use serde::{Deserialize, Serialize};
use sioul_core::health::Peer;
use std::path::{Path, PathBuf};

/// What this computer saw of your other devices, for the doses: kept here
/// only (`$XDG_STATE_HOME/sioul/share/peers.json`), never shared.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Peers {
    #[serde(default)]
    pub peers: std::collections::BTreeMap<String, PeerSeen>,
    /// When this device last looked (its clock): an opening seen now is timed
    /// only when it looked a moment before, else its own absence would count.
    #[serde(default)]
    pub looked: i64,
}

/// One other device, as seen here: what the doses know of it (`peer`), and
/// what that was learned from.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PeerSeen {
    pub peer: Peer,
    /// Its last claim seen here: when it renewed it (its clock).
    #[serde(default)]
    pub renewed: i64,
    /// When this computer last looked (its clock): news is timed only while it watches.
    #[serde(default)]
    pub looked: i64,
    /// How late its last claims came here, in seconds.
    #[serde(default)]
    pub delays: Vec<i64>,
    /// Its newest entry in the devices' registry read here, by its own times
    /// (`devices::written`; `peer.seen`: when it came): never replaced by an
    /// older copy put back, nor taken from a clock more than `FUTURE` ahead.
    #[serde(default)]
    pub entry: Option<crate::devices::Entry>,
    /// The words (`Entry::acked`) of the copy of its entry read now, when one
    /// reads and is not set aside (`Peer::set_aside`): a phone's word stands
    /// here only where this copy says it as well as the newest one, so that a
    /// copy that is no news may take a word away, never give one (the final
    /// review before 0.0.6, F1H). Not kept from one reading to the next.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acked_now: Option<std::collections::BTreeMap<String, i64>>,
    /// Its export's date (its clock) less when it was seen here (this one's),
    /// the last sixty: how far its clock is ahead, at least, is the largest.
    #[serde(default)]
    pub aheads: Vec<i64>,
    /// You said it is off: what it had said then. Counted again once it says anything newer.
    #[serde(default)]
    pub off: Option<Mark>,
    /// You forgot it, silent: not listed until it says anything newer.
    #[serde(default)]
    pub forgotten: Option<Mark>,
    /// How late the news of its last openings came here, in seconds
    /// (`Peer::wake`: the longest), the last `WAKES`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wakes: Vec<i64>,
    /// Since when its claim on the doses ran, as last seen: an older Sioul's
    /// openings, timed as an entry's start is.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub since: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// What a device had said when you said it was off, or forgot it.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    #[serde(default)]
    pub started: i64,
    #[serde(default)]
    pub exported: i64,
    #[serde(default)]
    pub renewed: i64,
    /// When you said so (this device's clock).
    #[serde(default)]
    pub at: i64,
}


impl Peers {
    /// Where it is kept, in Sioul's state folder.
    pub fn path(state: &Path) -> PathBuf {
        state.join("share").join("peers.json")
    }

    /// As kept at `path`; nothing known when the file is missing or does not read.
    pub fn load(path: &Path) -> Peers {
        std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file keeps the names it had while these structures were the
    /// window's own: what a Sioul before the move wrote reads here, and
    /// what is written from here says the same fields.
    #[test]
    fn the_file_reads_and_writes_as_before_the_structures_moved() {
        let before = r#"{"peers":{"desk":{"peer":{"name":"desk","known_until":0,"closed":false,"delay":null,"broken":null,"heard":1700000300,"id":"desk","said":{"started":1700000000,"closed":0,"working":true,"exported":1700000300,"imported":1700000290,"doses":true,"left":false,"phone":false},"complete":true,"seen":1700000310,"ahead":0,"off":false},"renewed":1700000200,"looked":1700000310,"delays":[4],"entry":{"id":"desk","name":"desk","kind":"computer","version":"0.0.11","started":1700000000,"closed":0,"working":true,"imported":1700000290,"exported":1700000300,"doses":true},"aheads":[-10],"off":null,"forgotten":{"started":1700000000,"exported":1700000300,"renewed":1700000200,"at":1700000400}}},"looked":1700000310}"#;
        let kept: Peers = serde_json::from_str(before).unwrap();
        let desk = &kept.peers["desk"];
        assert_eq!((kept.looked, desk.renewed, desk.peer.heard, desk.peer.seen), (1_700_000_310, 1_700_000_200, 1_700_000_300, 1_700_000_310));
        assert_eq!(desk.entry.as_ref().map(|e| (e.kind.as_str(), e.working, e.exported)), Some(("computer", true, 1_700_000_300)));
        assert_eq!((desk.off.clone(), desk.forgotten.as_ref().map(|m| m.at)), (None, Some(1_700_000_400)));
        assert!(desk.acked_now.is_none() && desk.wakes.is_empty() && desk.since == 0);
        // Written again: the same names, the optional ones left out as before.
        let written: serde_json::Value = serde_json::to_value(&kept).unwrap();
        let seen = &written["peers"]["desk"];
        let mut names: Vec<&str> = seen.as_object().unwrap().keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["aheads", "delays", "entry", "forgotten", "looked", "off", "peer", "renewed"]);
        assert_eq!(seen["forgotten"], serde_json::json!({ "started": 1_700_000_000, "exported": 1_700_000_300, "renewed": 1_700_000_200, "at": 1_700_000_400 }));
        assert_eq!(serde_json::from_value::<Peers>(written).unwrap().peers["desk"].peer, desk.peer);
        // A file missing or broken: nothing known, never an error.
        assert!(Peers::load(Path::new("/nonexistent/sioul/peers.json")).peers.is_empty());
    }
}

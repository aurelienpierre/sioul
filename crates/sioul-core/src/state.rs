// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What the Porch remembers between windows: where you closed it.
//!
//! "Done for now" records, for each account, the newest message the Porch
//! showed (its UIDVALIDITY and UID). Everything up to it has been seen; anything
//! newer waits for the next window, even when it reached the server earlier and
//! was fetched later. The server is never told: its read flags stay yours.
//! Kept in `$XDG_STATE_HOME/sioul/porch.toml`.

use crate::card::ImapOrigin;
use crate::config::state_dir;
use crate::porch::Triaged;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PorchState {
    /// Per account, the newest message shown when the Porch was last closed.
    #[serde(default)]
    pub done: BTreeMap<String, ImapOrigin>,
    /// The file as it was read: saved, only what changed since is written (`filelock::save_merged`).
    #[serde(skip)]
    pub read: crate::filelock::Read,
}

impl PorchState {
    /// Where the Porch's memory lives by default.
    pub fn default_path() -> PathBuf {
        state_dir().join("porch.toml")
    }

    /// The saved state; an empty one when there is none yet or it cannot be read.
    pub fn load(path: &Path) -> PorchState {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut state: PorchState = toml::from_str(&text).unwrap_or_default();
        state.read = crate::filelock::Read::of(&text);
        state
    }

    /// Written beside, then moved (a crash halfway would leave a file that
    /// cannot be read, and the Porch would forget where it was closed), under
    /// the file's lock, which the sharing takes too: only what changed since it
    /// was read, over what the file holds now.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        // A mark never goes backwards: of one account at one validity, the newer message stays.
        crate::filelock::save_merged_by(path, &self.read, self, |merged, before| {
            let Some(before) = before.and_then(|b| b.get("done")).and_then(toml::Value::as_table) else { return };
            let Some(done) = merged.get_mut("done").and_then(toml::Value::as_table_mut) else { return };
            for (account, mark) in done.iter_mut() {
                let field = |m: &toml::Value, name: &str| m.get(name).and_then(toml::Value::as_integer);
                if let Some(there) = before.get(account.as_str())
                    && field(there, "validity") == field(mark, "validity")
                    && field(there, "uid") > field(mark, "uid")
                {
                    *mark = there.clone();
                }
            }
        })
    }

    /// Whether a message was shown before the Porch was last closed.
    pub fn is_done(&self, account: Option<&str>, origin: Option<ImapOrigin>) -> bool {
        match (account.and_then(|a| self.done.get(a)), origin) {
            (Some(mark), Some(o)) => o.validity == mark.validity && o.uid <= mark.uid,
            _ => false,
        }
    }

    /// The newest message of each account among those shown; never one of
    /// the review queue's, which waits for your word whatever you close (a
    /// message your filter moved is in another folder, numbered apart).
    pub fn newest_shown(shown: &[Triaged]) -> BTreeMap<String, ImapOrigin> {
        let mut newest: BTreeMap<String, ImapOrigin> = BTreeMap::new();
        for t in shown.iter().filter(|t| t.lane != crate::porch::Lane::Review) {
            let (Some(account), Some(origin)) = (&t.card.account, t.card.origin) else { continue };
            let mark = newest.entry(account.clone()).or_insert(origin);
            *mark = (*mark).max(origin);
        }
        newest
    }

    /// Closes the Porch on what was shown. A new UIDVALIDITY replaces the old mark,
    /// since the server renumbered its messages.
    pub fn close(&mut self, newest: &BTreeMap<String, ImapOrigin>) {
        for (account, origin) in newest {
            let mark = self.done.entry(account.clone()).or_insert(*origin);
            if mark.validity != origin.validity || mark.uid < origin.uid {
                *mark = *origin;
            }
        }
    }
}

/// The messages you said are not payments, by key (see `budget::MailLine`).
/// Kept in `$XDG_STATE_HOME/sioul/money.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoneyState {
    #[serde(default)]
    pub ignored: std::collections::BTreeSet<String>,
    /// The file as it was read: saved, only what changed since is written (`filelock::save_merged`).
    #[serde(skip)]
    pub read: crate::filelock::Read,
}

impl MoneyState {
    pub fn default_path() -> PathBuf {
        state_dir().join("money.toml")
    }

    pub fn load(path: &Path) -> MoneyState {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut state: MoneyState = toml::from_str(&text).unwrap_or_default();
        state.read = crate::filelock::Read::of(&text);
        state
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
    fn closing_marks_what_was_shown() {
        let mut state = PorchState::default();
        let mark = |validity, uid| ImapOrigin { validity, uid };
        state.close(&BTreeMap::from([("personal".to_string(), mark(5, 40))]));
        assert!(state.is_done(Some("personal"), Some(mark(5, 40))));
        assert!(!state.is_done(Some("personal"), Some(mark(5, 41))));
        assert!(!state.is_done(Some("other"), Some(mark(5, 1))));
        assert!(!state.is_done(Some("personal"), None));
        // An older snapshot never moves the mark back.
        state.close(&BTreeMap::from([("personal".to_string(), mark(5, 30))]));
        assert!(state.is_done(Some("personal"), Some(mark(5, 40))));
        // A renumbered folder starts over.
        state.close(&BTreeMap::from([("personal".to_string(), mark(6, 3))]));
        assert!(!state.is_done(Some("personal"), Some(mark(5, 1))));
        let text = toml::to_string(&state).unwrap();
        assert_eq!(toml::from_str::<PorchState>(&text).unwrap(), state);
    }
}

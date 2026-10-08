// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's own spam filter, the part every device runs
//! (docs/spam-filter.md): the tokenizer (`tokenize`), the header
//! features (`features`), the reduced table that scores both (`table`), the
//! label log and the log of what the filter moved (`labels`), and the filter
//! as the Porch asks it (`Filter`). The training, on one computer, by hand,
//! is the `sioul-learn` crate's; it writes the table with
//! `table::Table::write`, and shares these functions with the phone, so that
//! both read a message the same way.
//!
//! Where its verdict goes, and what it never touches (people you know, codes,
//! projects, your own mail), is the Porch's: `porch::triage`. What it does
//! with each verdict is yours to choose, class by class (`Actions`).

pub mod features;
pub mod labels;
pub mod table;
pub mod tokenize;

use crate::card::Card;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use table::Table;

/// From this probability on, a message is spam, unless said otherwise (`[spam] threshold_spam`).
pub const THRESHOLD_SPAM: f32 = 0.95;
/// From this one on, it is unsure (`[spam] threshold_unsure`).
pub const THRESHOLD_UNSURE: f32 = 0.5;

/// Where a probability falls between the two thresholds: probably spam,
/// maybe spam, probably not spam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Class {
    Spam,
    Unsure,
    Ham,
}

impl Class {
    /// In the order the matrix shows them: the surest spam first.
    pub const ALL: [Class; 3] = [Class::Spam, Class::Unsure, Class::Ham];

    /// "spam", "unsure", "ham".
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Spam => "spam",
            Class::Unsure => "unsure",
            Class::Ham => "ham",
        }
    }

    pub fn read(text: &str) -> Option<Class> {
        Class::ALL.into_iter().find(|c| c.as_str() == text.trim())
    }
}

/// What the filter does with a message of a class, as you chose it (the
/// matrix in Settings ▸ Mail ▸ Your own spam filter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Into the account's Junk folder on the server, as the Junk button does;
    /// it waits in the Porch's review queue, where "Not spam" brings it back
    /// to the inbox. Only mail as it arrives: what was there before stays put.
    Move,
    /// Where it is, marked, in the Porch's review queue; never notified.
    Flag,
    /// Nothing shows.
    Nothing,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::Move, Action::Flag, Action::Nothing];

    /// "move", "flag", "nothing".
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Move => "move",
            Action::Flag => "flag",
            Action::Nothing => "nothing",
        }
    }

    pub fn read(text: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.as_str() == text.trim())
    }
}

/// What the filter does with each class: the matrix {probably spam, maybe
/// spam, probably not spam} × {move to spam, flag only, do nothing}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actions {
    pub spam: Action,
    pub unsure: Action,
    pub ham: Action,
}

impl Default for Actions {
    /// Until you choose: spam and doubts flagged, waiting for your review; nothing moves.
    fn default() -> Actions {
        Actions { spam: Action::Flag, unsure: Action::Flag, ham: Action::Nothing }
    }
}

impl Actions {
    /// What an older Sioul's `mode` meant: "off", nothing at all; "say", spam
    /// and doubts flagged; "act", spam moved, doubts flagged.
    pub fn of_mode(mode: &str) -> Option<Actions> {
        let (spam, unsure) = match mode.trim() {
            "off" => (Action::Nothing, Action::Nothing),
            "say" => (Action::Flag, Action::Flag),
            "act" => (Action::Move, Action::Flag),
            _ => return None,
        };
        Some(Actions { spam, unsure, ham: Action::Nothing })
    }

    /// The action for a class.
    pub fn of(&self, class: Class) -> Action {
        match class {
            Class::Spam => self.spam,
            Class::Unsure => self.unsure,
            Class::Ham => self.ham,
        }
    }

    /// The same, one class's action changed.
    pub fn with(mut self, class: Class, action: Action) -> Actions {
        match class {
            Class::Spam => self.spam = action,
            Class::Unsure => self.unsure = action,
            Class::Ham => self.ham = action,
        }
        self
    }

    /// Nothing done with any class: no message is judged at all.
    pub fn idle(&self) -> bool {
        Class::ALL.iter().all(|c| self.of(*c) == Action::Nothing)
    }
}

/// The filter's verdict on one message: how likely it is spam, from 0 to 1.
/// What weighed stays in the table's score (`table::Score`), for `sioul spam
/// why` in your terminal; the Porch never says it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verdict {
    pub p: f32,
}

/// The table's place on this device: `$XDG_DATA_HOME/sioul/spam/table.bin`.
pub fn table_path() -> PathBuf {
    crate::config::data_dir().join("spam").join("table.bin")
}

/// The filter as the Porch asks it: your choices, and where its table is.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    /// What is done with each class.
    pub actions: Actions,
    /// From this probability on, spam; from `threshold_unsure` on, unsure.
    pub threshold_spam: f32,
    pub threshold_unsure: f32,
    /// The table's file; with none there, the filter says nothing.
    pub table: PathBuf,
}

impl Filter {
    /// The filter as the configuration sets it (`[spam]`), its table where this device keeps it.
    pub fn of(config: &crate::config::Config) -> Filter {
        let (threshold_spam, threshold_unsure) = config.spam.thresholds();
        Filter { actions: config.spam.actions(), threshold_spam, threshold_unsure, table: table_path() }
    }

    /// Where a probability falls.
    pub fn class(&self, p: f32) -> Class {
        if p >= self.threshold_spam {
            Class::Spam
        } else if p >= self.threshold_unsure {
            Class::Unsure
        } else {
            Class::Ham
        }
    }

    /// What is done with a verdict: its class's action.
    pub fn action(&self, verdict: Verdict) -> Action {
        self.actions.of(self.class(verdict.p))
    }

    /// How likely a message is spam; none when nothing is done with any
    /// class, or with no table yet. `trusted_ids`: its account's
    /// authserv-ids (`score`). Each stored message is read once per table:
    /// the Porch asks again at each look.
    pub fn judge(&self, card: &Card, trusted_ids: &[String]) -> Option<Verdict> {
        if self.actions.idle() {
            return None;
        }
        let table = Table::cached(&self.table)?;
        let key = card.path.as_ref().map(|p| p.to_string_lossy().into_owned());
        if let Some(verdict) = key.as_deref().and_then(|key| remembered(&table, key)) {
            return Some(verdict);
        }
        let verdict = score(&table, card, trusted_ids);
        if let Some(key) = key {
            remember(&table, key, verdict);
        }
        Some(verdict)
    }
}

/// A message's verdict by a table: its words, its header features (its
/// file's time standing for when its provider received it, as Sioul stores
/// mail), these reading Sioul's own checks first (its stamp, made as the
/// message was stored), your provider's second, as the training reads the
/// corpus's own checks (`features::auth_results`; `trusted_ids`: the
/// account's authserv-ids, Sioul's own first). The words are read with the
/// table's own (`Table::lexicon`).
pub fn score(table: &Table, card: &Card, trusted_ids: &[String]) -> Verdict {
    let tokens = tokenize::tokens_with(&table.lexicon(), &card.subject, &card.excerpt);
    let received = card
        .path
        .as_deref()
        .and_then(|p| std::fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_secs()).ok());
    let auth = features::auth_results(&card.headers, trusted_ids);
    let x = features::features(card, auth.as_ref(), received, &tokens.links);
    Verdict { p: table.score(&tokens.words, &x).p }
}

/// The verdicts given with the table in use, by message file: forgotten when the table changes.
type Remembered = Option<(Arc<Table>, HashMap<String, Verdict>)>;
static REMEMBERED: Mutex<Remembered> = Mutex::new(None);

/// At most this many verdicts kept, then forgotten at once.
const REMEMBERED_MOST: usize = 20_000;

fn remembered(table: &Arc<Table>, key: &str) -> Option<Verdict> {
    let kept = REMEMBERED.lock().unwrap_or_else(|e| e.into_inner());
    kept.as_ref().filter(|(t, _)| Arc::ptr_eq(t, table)).and_then(|(_, verdicts)| verdicts.get(key).copied())
}

fn remember(table: &Arc<Table>, key: String, verdict: Verdict) {
    let mut kept = REMEMBERED.lock().unwrap_or_else(|e| e.into_inner());
    if !kept.as_ref().is_some_and(|(t, verdicts)| Arc::ptr_eq(t, table) && verdicts.len() < REMEMBERED_MOST) {
        *kept = Some((Arc::clone(table), HashMap::new()));
    }
    if let Some((_, verdicts)) = kept.as_mut() {
        verdicts.insert(key, verdict);
    }
}

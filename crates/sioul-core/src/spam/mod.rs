// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's own spam filter, the part every device runs
//! (docs/spam-filter.md): the tokenizer (`tokenize`), the header
//! features (`features`), the reduced table that scores both (`table`), the
//! label log (`labels`), and the filter as the Porch asks it (`Filter`). The
//! training, on the desktop only, is the `sioul-learn` crate's; it writes the
//! table with `table::Table::write`, and shares these functions with the
//! phone, so that both read a message the same way.
//!
//! Where its verdict goes, and what it never touches (people you know, codes,
//! projects, your own mail), is the Porch's: `porch::triage`.

pub mod features;
pub mod labels;
pub mod table;
pub mod tokenize;

use crate::card::Card;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use table::Table;

/// From this probability on, a message is spam, unless said otherwise (`[spam] threshold_spam`).
pub const THRESHOLD_SPAM: f32 = 0.95;
/// From this one on, it is unsure (`[spam] threshold_unsure`).
pub const THRESHOLD_UNSURE: f32 = 0.5;

/// What the filter does with what it finds (`[spam] mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Nothing: no verdict, no word.
    Off,
    /// Its verdict and why, said beside the message; nothing moves. Where it
    /// starts once trained, until you choose.
    #[default]
    Say,
    /// Spam set aside, a doubt said.
    Act,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Off, Mode::Say, Mode::Act];

    /// "off", "say", "act"; none for anything else.
    pub fn read(text: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.as_str() == text.trim())
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Say => "say",
            Mode::Act => "act",
        }
    }
}

/// Where a probability falls between the two thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Spam,
    Unsure,
    Ham,
}

/// The filter's verdict on one message.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// How likely it is spam, from 0 to 1.
    pub p: f32,
    pub why: Why,
}

/// What weighed toward spam, the most first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Why {
    /// Up to five words or placeholders ("_PRICE_"), as the filter reads them.
    pub words: Vec<String>,
    /// Up to three header features, by their name (`features::NAMES`).
    pub signs: Vec<&'static str>,
}

impl Why {
    /// The words that pushed toward spam, then the header features, each only when it did.
    pub fn of(score: &table::Score) -> Why {
        Why {
            words: score.words.iter().filter(|(_, share)| *share > 0.0).take(5).map(|(word, _)| word.clone()).collect(),
            signs: score.signs.iter().filter(|(_, share)| *share > 0.0).take(3).filter_map(|(at, _)| features::NAMES.get(*at).copied()).collect(),
        }
    }
}

/// The table's place on this device: `$XDG_DATA_HOME/sioul/spam/table.bin`.
pub fn table_path() -> PathBuf {
    crate::config::data_dir().join("spam").join("table.bin")
}

/// The filter as the Porch asks it: your choices, and where its table is.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub mode: Mode,
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
        Filter { mode: config.spam.mode(), threshold_spam, threshold_unsure, table: table_path() }
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

    /// How likely a message is spam, and why; none when the filter is off or
    /// has no table yet. `trusted_ids`: its account's authserv-ids (`score`).
    /// Each stored message is read once per table: the Porch asks again at each look.
    pub fn judge(&self, card: &Card, trusted_ids: &[String]) -> Option<Verdict> {
        if self.mode == Mode::Off {
            return None;
        }
        let table = Table::cached(&self.table)?;
        let key = card.path.as_ref().map(|p| p.to_string_lossy().into_owned());
        if let Some(verdict) = key.as_deref().and_then(|key| remembered(&table, key)) {
            return Some(verdict);
        }
        let verdict = score(&table, card, trusted_ids);
        if let Some(key) = key {
            remember(&table, key, verdict.clone());
        }
        Some(verdict)
    }
}

/// A message's verdict by a table: its words, its header features (its
/// file's time standing for when its provider received it, as Sioul stores
/// mail), these reading your provider's authentication results alone, as the
/// training does (`features::provider_results`; `trusted_ids`: the account's
/// authserv-ids, Sioul's own among them or not).
pub fn score(table: &Table, card: &Card, trusted_ids: &[String]) -> Verdict {
    let tokens = tokenize::tokens(&card.subject, &card.excerpt);
    let received = card
        .path
        .as_deref()
        .and_then(|p| std::fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_secs()).ok());
    let auth = features::provider_results(&card.headers, trusted_ids);
    let x = features::features(card, auth.as_ref(), received, &tokens.links);
    let score = table.score(&tokens.words, &x);
    Verdict { p: score.p, why: Why::of(&score) }
}

/// The verdicts given with the table in use, by message file: forgotten when the table changes.
type Remembered = Option<(Arc<Table>, HashMap<String, Verdict>)>;
static REMEMBERED: Mutex<Remembered> = Mutex::new(None);

/// At most this many verdicts kept, then forgotten at once.
const REMEMBERED_MOST: usize = 20_000;

fn remembered(table: &Arc<Table>, key: &str) -> Option<Verdict> {
    let kept = REMEMBERED.lock().unwrap_or_else(|e| e.into_inner());
    kept.as_ref().filter(|(t, _)| Arc::ptr_eq(t, table)).and_then(|(_, verdicts)| verdicts.get(key).cloned())
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

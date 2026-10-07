// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a test said message by message (`sioul spam eval --errors`, `sioul
//! spam train --errors`, and the MCP's spam_eval and spam_train): the test
//! messages counted by account and by folder, their label (spam or ham, as
//! decided) against the filter's class (probably spam, maybe spam, probably
//! not spam); the numbers at a grid of thresholds, so that one can be
//! chosen; and the worst errors: ham the filter called spam or maybe spam,
//! the surest first, then spam it did not call spam, the least sure first.
//!
//! An error is said by where it is and by its card, as the Porch reads the
//! message (its sender, its subject, the start of its text): the command
//! line prints its sender and its subject, masked as the MCP masks them
//! (codes, sign-in links, account numbers), never its text. Nothing here is
//! written to a file: `trained.toml` keeps numbers only.

use crate::corpus::{self, Place};
use crate::eval::{self, AtThreshold};
use crate::external;
use crate::labels::{Evidence, Label};
use crate::{Dirs, LearnError};
use serde::Serialize;
use sioul_core::card::Card;
use sioul_core::spam::Class;
use std::collections::{BTreeMap, HashMap, HashSet};

/// The thresholds the grid gives numbers at.
pub const GRID: [f64; 10] = [0.5, 0.6, 0.7, 0.8, 0.85, 0.9, 0.95, 0.97, 0.98, 0.99];

/// Errors listed at most, of each kind.
pub const MOST_ERRORS: usize = 500;

/// One test message, as the filter judged it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tested {
    /// How likely spam, as the filter says.
    pub p: f64,
    /// What it is, as its label says.
    pub label: Label,
    /// When it arrived (Unix seconds).
    pub date: i64,
    pub origin: Origin,
    /// Ham in an inbox, come less than `UNSETTLED_DAYS` ago: its label may
    /// not be settled yet (spam nobody has looked at stays in an inbox as ham).
    pub unsettled: bool,
}

/// Days after which a message left in an inbox counts as said ham: before,
/// it may be spam nobody has looked at yet.
pub const UNSETTLED_DAYS: i64 = 30;

/// Whether a message's label may not be settled yet: ham in an inbox, come
/// less than `UNSETTLED_DAYS` before `now`.
pub fn unsettled(label: Label, role: sioul_core::folders::Role, date: i64, now: i64) -> bool {
    label == Label::Ham && role == sioul_core::folders::Role::Inbox && date > now - UNSETTLED_DAYS * 86_400
}

/// Where a test message comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Your own mail: the copy learned from, and what decided its label.
    Own { place: Place, evidence: Evidence },
    /// Outside material: its source, and its rank among that source's
    /// messages as `external::read_all` reads them, from 0.
    Outside { source: String, ordinal: u64 },
}

/// How many messages of one label the filter judged probably spam, maybe
/// spam, probably not spam.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Row {
    pub spam: u64,
    pub unsure: u64,
    pub ham: u64,
}

impl Row {
    pub fn total(&self) -> u64 {
        self.spam + self.unsure + self.ham
    }
}

/// Messages by their label, then by the filter's class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Confusion {
    pub ham: Row,
    pub spam: Row,
}

impl Confusion {
    fn add(&mut self, label: Label, class: Class) {
        let row = match label {
            Label::Ham => &mut self.ham,
            Label::Spam => &mut self.spam,
        };
        match class {
            Class::Spam => row.spam += 1,
            Class::Unsure => row.unsure += 1,
            Class::Ham => row.ham += 1,
        }
    }
}

/// One error: a ham the filter called spam or maybe spam, or a spam it did
/// not call spam.
#[derive(Debug, Clone)]
pub struct Wrong {
    pub p: f64,
    pub label: Label,
    pub class: Class,
    /// When it arrived (Unix seconds).
    pub date: i64,
    /// What decided its label: "folder", "junk-folder", "keyword", "log";
    /// "outside" for outside material.
    pub evidence: &'static str,
    /// Its account; the source's name for outside material.
    pub account: String,
    /// Its folder on the server (IMAP's modified UTF-7); empty for outside material.
    pub folder: String,
    pub outside: bool,
    /// The message as the Porch reads it; none when it could not be read again.
    pub card: Option<Card>,
}

/// The worst errors of a test set.
#[derive(Debug, Clone, Default)]
pub struct Errors {
    /// Ham judged spam or maybe spam, the most likely spam first.
    pub ham_called_spam: Vec<Wrong>,
    /// Spam judged maybe spam or not spam, the least likely spam first.
    pub spam_missed: Vec<Wrong>,
}

/// An outside source's held-out part, message by message.
#[derive(Debug, Clone, Default)]
pub struct OutsideDetail {
    pub confusion: Confusion,
    /// Its held-out part at each threshold of `GRID`.
    pub grid: Vec<AtThreshold>,
    /// Each held-out message's probability, and whether it is spam.
    pub scores: Vec<(f64, bool)>,
    pub errors: Errors,
}

/// What a test said message by message.
#[derive(Debug, Clone, Default)]
pub struct Detail {
    /// Your own mail's test messages, by account.
    pub by_account: BTreeMap<String, Confusion>,
    /// The same, by account, then by folder (its server name).
    pub by_folder: BTreeMap<String, BTreeMap<String, Confusion>>,
    /// Your own mail's test messages at each threshold of `GRID`.
    pub grid: Vec<AtThreshold>,
    /// The same, unsettled ham left out (`Tested::unsettled`).
    pub grid_settled: Vec<AtThreshold>,
    /// Each test message's probability, whether it is spam, whether its
    /// label is unsettled: numbers only, for a curve or another threshold.
    pub scores: Vec<(f64, bool, bool)>,
    pub errors: Errors,
    /// Each outside source's held-out part.
    pub outside: BTreeMap<String, OutsideDetail>,
}

/// What a label's evidence is called in the errors listed.
pub fn evidence_name(evidence: Evidence) -> &'static str {
    match evidence {
        Evidence::Folder => "folder",
        Evidence::JunkFolder => "junk-folder",
        Evidence::Keyword => "keyword",
        Evidence::Log => "log",
    }
}

/// The class a probability falls in, between the two thresholds.
pub fn class_of(p: f64, threshold_spam: f64, threshold_unsure: f64) -> Class {
    if p >= threshold_spam {
        Class::Spam
    } else if p >= threshold_unsure {
        Class::Unsure
    } else {
        Class::Ham
    }
}

/// The detail of a test: counts, the grid, and up to `errors` errors of
/// each kind for your own mail and for each outside source. The errors'
/// cards are read again from the corpus and the outside material, once.
pub fn of(dirs: &Dirs, tested: &[Tested], threshold_spam: f64, threshold_unsure: f64, errors: usize) -> Result<Detail, LearnError> {
    let class = |p: f64| class_of(p, threshold_spam, threshold_unsure);
    let mut detail = Detail::default();
    let mut own = Vec::new();
    let mut settled = Vec::new();
    let mut outside: BTreeMap<&str, Vec<(f64, bool)>> = BTreeMap::new();
    for t in tested {
        match &t.origin {
            Origin::Own { place, .. } => {
                detail.by_account.entry(place.account.clone()).or_default().add(t.label, class(t.p));
                detail.by_folder.entry(place.account.clone()).or_default().entry(place.folder.clone()).or_default().add(t.label, class(t.p));
                own.push((t.p, t.label == Label::Spam));
                detail.scores.push((t.p, t.label == Label::Spam, t.unsettled));
                if !t.unsettled {
                    settled.push((t.p, t.label == Label::Spam));
                }
            }
            Origin::Outside { source, .. } => {
                detail.outside.entry(source.clone()).or_default().confusion.add(t.label, class(t.p));
                outside.entry(source.as_str()).or_default().push((t.p, t.label == Label::Spam));
            }
        }
    }
    detail.grid = eval::grid(&own, &GRID);
    detail.grid_settled = eval::grid(&settled, &GRID);
    for (source, scored) in outside {
        let part = detail.outside.entry(source.to_string()).or_default();
        part.grid = eval::grid(&scored, &GRID);
        part.scores = scored;
    }
    let errors = errors.min(MOST_ERRORS);
    if errors == 0 {
        return Ok(detail);
    }
    // The worst of each kind, in each set: your own mail (None), each outside source.
    let mut sets: Vec<Option<&str>> = vec![None];
    sets.extend(detail.outside.keys().map(|s| Some(s.as_str())));
    let in_set = |t: &&Tested, set: Option<&str>| match (&t.origin, set) {
        (Origin::Own { .. }, None) => true,
        (Origin::Outside { source, .. }, Some(set)) => source == set,
        _ => false,
    };
    let mut chosen: Vec<(Option<String>, Vec<&Tested>, Vec<&Tested>)> = Vec::new();
    for set in sets {
        let mut ham: Vec<&Tested> = tested.iter().filter(|t| in_set(t, set) && t.label == Label::Ham && class(t.p) != Class::Ham).collect();
        ham.sort_by(|a, b| b.p.total_cmp(&a.p).then(b.date.cmp(&a.date)));
        ham.truncate(errors);
        let mut spam: Vec<&Tested> = tested.iter().filter(|t| in_set(t, set) && t.label == Label::Spam && class(t.p) != Class::Spam).collect();
        spam.sort_by(|a, b| a.p.total_cmp(&b.p).then(b.date.cmp(&a.date)));
        spam.truncate(errors);
        chosen.push((set.map(str::to_string), ham, spam));
    }
    // Their cards, read again: your own mail's from the corpus, outside material's from its files.
    let places: HashSet<&Place> = chosen.iter().flat_map(|(_, h, s)| h.iter().chain(s)).filter_map(|t| if let Origin::Own { place, .. } = &t.origin { Some(place) } else { None }).collect();
    let ranks: HashSet<(&str, u64)> =
        chosen.iter().flat_map(|(_, h, s)| h.iter().chain(s)).filter_map(|t| if let Origin::Outside { source, ordinal } = &t.origin { Some((source.as_str(), *ordinal)) } else { None }).collect();
    let mut cards: HashMap<Place, Card> = HashMap::new();
    if !places.is_empty() {
        corpus::read_all(dirs, |record| {
            let place = record.place();
            if places.contains(&place)
                && !cards.contains_key(&place)
                && let Some(card) = crate::spamcore::card_of(&record)
            {
                cards.insert(place, card);
            }
        })?;
    }
    let mut outside_cards: HashMap<(String, u64), Card> = HashMap::new();
    if !ranks.is_empty() {
        let mut counted: HashMap<String, u64> = HashMap::new();
        external::read_all(dirs, |source, message| {
            let rank = counted.entry(source.to_string()).or_default();
            let ordinal = *rank;
            *rank += 1;
            if ranks.contains(&(source, ordinal))
                && let Some(card) = message.card()
            {
                outside_cards.insert((source.to_string(), ordinal), card);
            }
        })?;
    }
    let wrong = |t: &Tested| -> Wrong {
        let (evidence, account, folder, outside, card) = match &t.origin {
            Origin::Own { place, evidence } => (evidence_name(*evidence), place.account.clone(), place.folder.clone(), false, cards.get(place).cloned()),
            Origin::Outside { source, ordinal } => ("outside", source.clone(), String::new(), true, outside_cards.get(&(source.clone(), *ordinal)).cloned()),
        };
        Wrong { p: t.p, label: t.label, class: class(t.p), date: t.date, evidence, account, folder, outside, card }
    };
    for (set, ham, spam) in chosen {
        let listed = Errors { ham_called_spam: ham.into_iter().map(wrong).collect(), spam_missed: spam.into_iter().map(wrong).collect() };
        match set {
            None => detail.errors = listed,
            Some(source) => detail.outside.entry(source).or_default().errors = listed,
        }
    }
    Ok(detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-learn-detail-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Counted by account and folder, the grid, and the worst errors of each
    /// kind with their cards read again; outside material apart.
    #[test]
    fn the_worst_errors_with_their_cards() {
        let root = scratch("errors");
        let dirs = Dirs::under(&root);
        let mail = synthetic::mailbox(5, 6, 4);
        let records: Vec<_> = mail.iter().enumerate().map(|(i, m)| synthetic::record_of(m, 1, i as u32 + 1)).collect();
        corpus::store(&dirs, &records).unwrap();
        let file = root.join("old.jsonl");
        std::fs::write(&file, "{\"label\":\"ham\",\"date\":1600000000,\"subject\":\"Lunch tomorrow\",\"text\":\"See you\",\"from\":\"Jane <jane@example.org>\"}\n{\"label\":\"spam\",\"date\":1600000100,\"subject\":\"You won\",\"text\":\"Claim it\"}\n").unwrap();
        external::import(&dirs, &file, None).unwrap();
        // Each message judged: ham at 0.1 but two, spam at 0.99 but one.
        let mut tested: Vec<Tested> = records
            .iter()
            .map(|r| {
                let label = if r.role == sioul_core::folders::Role::Junk || r.has_flag("$Junk") { Label::Spam } else { Label::Ham };
                Tested { p: if label == Label::Spam { 0.99 } else { 0.1 }, label, date: r.date, origin: Origin::Own { place: r.place(), evidence: Evidence::Folder }, unsettled: false }
            })
            .collect();
        let hams: Vec<usize> = (0..tested.len()).filter(|&i| tested[i].label == Label::Ham).collect();
        let spams: Vec<usize> = (0..tested.len()).filter(|&i| tested[i].label == Label::Spam).collect();
        tested[hams[0]].p = 0.97;
        tested[hams[1]].p = 0.6;
        tested[spams[0]].p = 0.2;
        tested.push(Tested { p: 0.8, label: Label::Ham, date: 1_600_000_000, origin: Origin::Outside { source: "old".into(), ordinal: 0 }, unsettled: false });
        tested.push(Tested { p: 0.9, label: Label::Spam, date: 1_600_000_100, origin: Origin::Outside { source: "old".into(), ordinal: 1 }, unsettled: false });
        let detail = of(&dirs, &tested, 0.95, 0.5, 5).unwrap();
        // Counted: every own message once by account and once by folder.
        let counted: u64 = detail.by_account.values().map(|c| c.ham.total() + c.spam.total()).sum();
        let by_folder: u64 = detail.by_folder.values().flat_map(|f| f.values()).map(|c| c.ham.total() + c.spam.total()).sum();
        assert_eq!((counted, by_folder), (records.len() as u64, records.len() as u64));
        assert_eq!(detail.grid.len(), GRID.len());
        assert_eq!(detail.grid[0].ham_called_spam.count, 2, "two ham from 0.5 on");
        assert_eq!(detail.grid[6].ham_called_spam.count, 1, "one from 0.95 on");
        // The worst first, each with its card: the ham at 0.97, then at 0.6; the spam at 0.2.
        let ham: Vec<f64> = detail.errors.ham_called_spam.iter().map(|w| w.p).collect();
        assert_eq!(ham, vec![0.97, 0.6]);
        assert_eq!(detail.errors.ham_called_spam[0].class, Class::Spam);
        assert_eq!(detail.errors.spam_missed.iter().map(|w| (w.p, w.class)).collect::<Vec<_>>(), vec![(0.2, Class::Ham)]);
        let card = detail.errors.ham_called_spam[0].card.as_ref().expect("read again");
        let record = &records[hams[0]];
        assert_eq!(card.message_id.as_deref().map(sioul_core::mailindex::bare_id), crate::labels::message_id(&record.header_bytes()));
        assert_eq!(detail.errors.ham_called_spam[0].evidence, "folder");
        // Outside material apart: its own counts and errors, its card made of its fields.
        let old = &detail.outside["old"];
        assert_eq!((old.confusion.ham.unsure, old.confusion.spam.unsure), (1, 1));
        let outside = &old.errors.ham_called_spam[0];
        assert!(outside.outside && outside.evidence == "outside" && outside.account == "old");
        let card = outside.card.as_ref().expect("made of its fields");
        assert_eq!((card.subject.as_str(), card.from_address.as_deref()), ("Lunch tomorrow", Some("jane@example.org")));
        assert_eq!(old.errors.spam_missed[0].card.as_ref().map(|c| c.subject.clone()), Some("You won".to_string()));
        // No errors asked: counts and the grid alone, no card read.
        let plain = of(&dirs, &tested, 0.95, 0.5, 0).unwrap();
        assert!(plain.errors.ham_called_spam.is_empty() && plain.outside["old"].errors.spam_missed.is_empty() && !plain.grid.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}

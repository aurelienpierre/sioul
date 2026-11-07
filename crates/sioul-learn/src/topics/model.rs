// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The topics' model, trained in the spam filter's weekly run (docs/topics.md,
//! "The model"), on the computer that trains:
//!
//! 1. **Labels** by the order of words (`sioul_core::topics::rules`): your
//!    word on a message, your rules for senders, conversations, the topics'
//!    seeds and their presets, every one at full weight. Then the last table's
//!    own sure topics on mail nothing else names, from a sender the topic
//!    already has, half of them (weight 0.5), never more than a topic's other
//!    examples, in the part it learns from only; its doubts never (a guess on
//!    a stranger's mail, taken as a label, drifted the stand-in's model into
//!    never being sure of two topics).
//! 2. **The gate**, topic by topic: a topic enters the table once it has 20
//!    examples from 3 senders, or 50 from fewer (one office). Below it, it
//!    has no class at all, and its mail is left out of the training (never
//!    learned as "no topic"): it is sorted by your words and its rules alone.
//! 3. **The guards**: in a topic of three senders or more, one sender makes at
//!    most 20 % of its examples; one conversation at most 20 % in any topic
//!    (its newest kept).
//! 4. **The model**: fastText's softmax over the topics through the gate and a
//!    class "no topic", learned from a quarter of the mail nothing names
//!    (`Method::NoneClass`); learned from the oldest 80 % in time order, its
//!    temperature and sure thresholds from the newest fifth of that, tested
//!    on the newest 20 %; folded into the table every device reads.
//! 5. **Replaced only when no worse**: on the newest mail neither table
//!    learned from, the new table must agree with the labels at least as well
//!    as the one in use, topic by topic (F1 of its sure topics, within half a
//!    point), and change fewer than 2 % of the sure topics the one in use
//!    shows on mail nothing else names (churn). The first table needs neither.

use super::{Data, Mail, Method, Options, Read, Way, fifths, fold, measure};
use crate::{Cancel, LearnError, Progress};
use serde::Serialize;
use sioul_core::topics::labels::Said;
use sioul_core::topics::presets::Matcher;
use sioul_core::topics::rules;
use sioul_core::topics::table::{Band, Precision, Table};
use sioul_core::topics::Topic;
use std::collections::{HashMap, HashSet};

/// A topic enters the table from this many examples…
pub const LEAST: u64 = 20;
/// …from this many senders…
pub const LEAST_SENDERS: u64 = 3;
/// …or from this many, whoever sent them (one office).
pub const LEAST_ALONE: u64 = 50;
/// One sender's, or one conversation's, share of a topic's examples at most.
pub const CAP: f64 = 0.2;
/// The share of the sure topics shown that a new table may change.
pub const CHURN: f64 = 0.02;
/// The share of the mail nothing names learned as "no topic".
pub const NONE_WEIGHT: f64 = 0.25;
/// Within this of the table in use, a topic's F1 is no worse.
const TIE: f64 = 0.005;

/// A topic as the training saw it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TopicState {
    pub id: String,
    /// Its examples by the order of words, and their distinct senders.
    pub examples: u64,
    pub senders: u64,
    /// Through the gate: a class of the table.
    pub learned: bool,
    /// Examples left out by the guards (one sender's or one conversation's share).
    pub capped: u64,
}

/// What a training ended with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "outcome")]
pub enum Outcome {
    /// No topic through the gate: no table; topics sort by your words and rules alone.
    NoTopic,
    /// Too little mail to learn and test in time order.
    TooFew,
    /// The first table.
    First,
    /// A new table, no worse than the one in use.
    Replaced,
    /// Kept the table in use: the new one does worse on this topic.
    Worse { topic: String },
    /// Kept the table in use: the new one would change too many topics shown.
    Churn,
}

/// A training: its outcome, its topics, the table (to write when it replaces), its measures.
#[derive(Debug, Clone, Serialize)]
pub struct Trained {
    #[serde(flatten)]
    pub outcome: Outcome,
    pub topics: Vec<TopicState>,
    #[serde(skip)]
    pub table: Option<Table>,
    /// The share of the sure topics shown that it changes, when a table is in use.
    pub churn: Option<f64>,
    /// Topic by topic on the newest mail: the table in use's F1, the new one's.
    pub compared: Vec<(String, Option<f64>, Option<f64>)>,
    /// The new model's numbers on the newest 20 %.
    pub macro_f1: Option<f64>,
    pub coverage: Option<f64>,
    pub seconds: f64,
}

impl Trained {
    /// Whether its table is to be written in place of the one in use.
    pub fn replaces(&self) -> bool {
        matches!(self.outcome, Outcome::First | Outcome::Replaced) && self.table.is_some()
    }
}

fn facts(m: &Mail) -> rules::Facts<'_> {
    rules::Facts {
        id: m.id.as_deref(),
        sender: m.proven.as_deref(),
        conversation: &m.conversation,
        folders: &m.folders,
        categories: &m.categories,
        verified_domain: m.verified_domain.as_deref(),
        list_id: m.list_id.as_deref(),
        is_list: m.is_list,
    }
}

/// Each message's topic by the order of words, an index into `topics`.
pub(crate) fn labels_by_words(read: &Read, topics: &[Topic], said: &Said, presets: Option<&Matcher>) -> Vec<Option<usize>> {
    read.mail.iter().map(|m| rules::topic_of(topics, said, &facts(m), presets).map(|t| t.topic)).collect()
}

/// Whether a topic of `examples` from `senders` goes through the gate.
pub fn through(examples: u64, senders: u64) -> bool {
    (examples >= LEAST && senders >= LEAST_SENDERS) || examples >= LEAST_ALONE
}

/// A topic's F1 of sure topics against the labels, on these messages; none without either.
fn f1(judged: &[(Option<&str>, Option<&str>)], topic: &str) -> Option<f64> {
    let right = judged.iter().filter(|(sure, label)| *sure == Some(topic) && *label == Some(topic)).count();
    let said = judged.iter().filter(|(sure, _)| *sure == Some(topic)).count();
    let truth = judged.iter().filter(|(_, label)| *label == Some(topic)).count();
    (truth > 0).then(|| 2.0 * right as f64 / (said + truth) as f64)
}

/// A table's sure topic of a message, by id.
fn sure_of<'t>(table: &'t Table, m: &Mail) -> Option<&'t str> {
    let judged = table.judge(&m.line(true));
    (judged.band == Band::Sure).then_some(judged.topic).flatten().and_then(|t| table.meta.topics.get(t)).map(String::as_str)
}

/// Trains the topics' model on the mail read (`topics::read` with its
/// words), your topics and your word on them, never on a table's own
/// guesses; `previous`, the table in use, against which the new one is
/// judged.
#[allow(clippy::too_many_arguments)]
pub fn train(dirs: &crate::Dirs, read: &Read, topics: &[Topic], said: &Said, presets: Option<&Matcher>, previous: Option<&Table>, options: &Options, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Trained, LearnError> {
    let started = std::time::Instant::now();
    let by_words = labels_by_words(read, topics, said, presets);
    // The gate, topic by topic.
    let mut states: Vec<TopicState> = topics.iter().map(|t| TopicState { id: t.id.clone(), ..TopicState::default() }).collect();
    let mut senders: Vec<HashSet<&str>> = vec![HashSet::new(); topics.len()];
    for (m, label) in by_words.iter().enumerate() {
        if let Some(t) = *label {
            states[t].examples += 1;
            if let Some(s) = read.mail[m].sender.as_deref() {
                senders[t].insert(s);
            }
        }
    }
    for (state, s) in states.iter_mut().zip(&senders) {
        state.senders = s.len() as u64;
        state.learned = through(state.examples, state.senders);
    }
    let classes: Vec<usize> = (0..topics.len()).filter(|&t| states[t].learned).collect();
    let class_of: HashMap<usize, usize> = classes.iter().enumerate().map(|(c, &t)| (t, c)).collect();
    let ids: Vec<String> = classes.iter().map(|&t| topics[t].id.clone()).collect();
    let mut trained = Trained { outcome: Outcome::NoTopic, topics: Vec::new(), table: None, churn: None, compared: Vec::new(), macro_f1: None, coverage: None, seconds: 0.0 };
    if classes.is_empty() {
        trained.topics = states;
        trained.seconds = started.elapsed().as_secs_f64();
        return Ok(trained);
    }
    // In time order, the mail of a topic under the gate left out.
    let mut order: Vec<usize> = (0..read.mail.len())
        .filter(|&m| !read.mail[m].words.is_empty() || !read.mail[m].senders.is_empty())
        .filter(|&m| by_words[m].is_none_or(|t| class_of.contains_key(&t)))
        .collect();
    order.sort_by_key(|&m| (read.mail[m].date, m));
    let (learned, tested) = fifths(&order);
    let (fit, validation) = fifths(&learned);
    // Your acts and rules only, never a table's own guesses (F4, review of 7
    // November 2026): two devices holding the same corpus and the same
    // labels make the same table at every training, whatever table each has
    // in use; `previous` only judges whether the new one replaces it.
    let labels: Vec<Option<usize>> = by_words.iter().map(|l| l.and_then(|t| class_of.get(&t).copied())).collect();
    // The guards, on the part learned from: one sender's and one conversation's share.
    let mut dropped: HashSet<usize> = HashSet::new();
    for (c, &t) in classes.iter().enumerate() {
        let mut examples: Vec<usize> = learned.iter().copied().filter(|&m| labels[m] == Some(c)).collect();
        examples.sort_by_key(|&m| std::cmp::Reverse(read.mail[m].date));
        let most = ((examples.len() as f64 * CAP).floor() as usize).max(1);
        let (mut by_sender, mut by_conversation): (HashMap<&str, usize>, HashMap<&str, usize>) = (HashMap::new(), HashMap::new());
        for &m in &examples {
            let mail = &read.mail[m];
            let sender = mail.sender.as_deref().unwrap_or("");
            let conversation = mail.conversation.first().or(mail.id.as_ref()).map_or("", String::as_str);
            let one = by_sender.entry(sender).or_default();
            let thread = by_conversation.entry(conversation).or_default();
            let over_sender = states[t].senders >= LEAST_SENDERS && !sender.is_empty() && *one >= most;
            let over_conversation = !conversation.is_empty() && *thread >= most;
            if over_sender || over_conversation {
                dropped.insert(m);
                states[t].capped += 1;
            } else {
                *one += 1;
                *thread += 1;
            }
        }
    }
    let keep = |part: Vec<usize>| -> Vec<usize> { part.into_iter().filter(|m| !dropped.contains(m)).collect() };
    let (learned, fit, validation) = (keep(learned), keep(fit), keep(validation));
    let labelled = |part: &[usize]| part.iter().any(|&m| labels[m].is_some());
    if !labelled(&fit) || validation.is_empty() || tested.is_empty() {
        trained.outcome = Outcome::TooFew;
        trained.topics = states;
        trained.seconds = started.elapsed().as_secs_f64();
        return Ok(trained);
    }
    let known: HashSet<&str> = learned.iter().filter_map(|&m| read.mail[m].sender.as_deref()).collect();
    let data = Data { mail: &read.mail, labels, topics: classes.len(), learned, fit, validation, tested, known };
    let way = Way { method: Method::NoneClass { weight: NONE_WEIGHT }, senders: true, bucket: options.buckets.first().copied().unwrap_or(50_000) };
    let (measured, calibration, model, _) = measure(dirs, &data, &way, None, options, progress, cancel)?;
    let mut table = fold(&model, &way, &calibration, &ids, options, Precision::F16);
    table.meta.messages = data.learned.len() as u64;
    table.meta.metrics.insert("macro_f1".into(), measured.test.macro_f1.unwrap_or(-1.0));
    table.meta.metrics.insert("coverage".into(), measured.test.coverage);
    for (n, id) in measured.topics.iter().zip(&ids) {
        if let Some(r) = n.recall {
            table.meta.metrics.insert(format!("recall.{id}"), r);
        }
    }
    trained.macro_f1 = measured.test.macro_f1;
    trained.coverage = Some(measured.test.coverage);
    let topic_of_label = |m: usize| data.labels[m].and_then(|c| ids.get(c)).map(String::as_str);
    let outcome = match previous {
        None => Outcome::First,
        Some(previous) => {
            // The newest mail neither table learned from: after the table in use was made, else the whole test.
            let newer: Vec<usize> = data.tested.iter().copied().filter(|&m| read.mail[m].date > previous.meta.trained_at).collect();
            let compare = if newer.iter().any(|&m| data.labels[m].is_some()) { newer } else { data.tested.clone() };
            let old: Vec<(Option<&str>, Option<&str>)> = compare.iter().map(|&m| (sure_of(previous, &read.mail[m]), topic_of_label(m))).collect();
            let new: Vec<(Option<&str>, Option<&str>)> = compare.iter().map(|&m| (sure_of(&table, &read.mail[m]), topic_of_label(m))).collect();
            let mut worse = None;
            for id in ids.iter().filter(|id| previous.meta.topics.contains(id)) {
                let (was, now) = (f1(&old, id), f1(&new, id));
                trained.compared.push((id.clone(), was, now));
                if let (Some(was), Some(now)) = (was, now)
                    && now < was - TIE
                    && worse.is_none()
                {
                    worse = Some(id.clone());
                }
            }
            // Churn: the sure topics the table in use shows, on mail nothing else names.
            let (mut shown, mut changed) = (0u64, 0u64);
            for (m, mail) in read.mail.iter().enumerate() {
                if by_words[m].is_some() || (mail.words.is_empty() && mail.senders.is_empty()) {
                    continue;
                }
                if let Some(was) = sure_of(previous, mail) {
                    shown += 1;
                    changed += u64::from(sure_of(&table, mail) != Some(was));
                }
            }
            let churn = (shown > 0).then(|| changed as f64 / shown as f64);
            trained.churn = churn;
            match (worse, churn) {
                (Some(topic), _) => Outcome::Worse { topic },
                (None, Some(share)) if share >= CHURN => Outcome::Churn,
                _ => Outcome::Replaced,
            }
        }
    };
    // The churn is said in what the training did (`Trained::churn`), never in
    // the table: the table depends on the corpus and the labels alone, not on
    // the table in use here (T1, review of 7 November 2026).
    trained.outcome = outcome;
    trained.table = Some(table);
    trained.topics = states;
    trained.seconds = started.elapsed().as_secs_f64();
    Ok(trained)
}

/// A training on this device (`train_here`): what it did, and whether its
/// table was written in place of the one in use.
#[derive(Debug, Clone, Serialize)]
pub struct Here {
    #[serde(flatten)]
    pub trained: Trained,
    pub written: bool,
}

/// Trains this device's topics from its own corpus since forever
/// (`topics::read_corpus`), your topics and your word on them, against the
/// table in use here; writes the new table where every view of topics reads
/// it when it replaces that one and `replace` says so, then judges the mail
/// kept here, `judge` messages at most (`judging::judge_kept`).
/// Deterministic: one thread, the spam filter's fixed seed, the corpus in a
/// fixed order, so that two devices holding the same corpus and the same
/// labels make the same table, and guess the same topics.
pub fn train_here(dirs: &crate::Dirs, config: &sioul_core::config::Config, replace: bool, judge: usize, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Here, LearnError> {
    use sioul_core::topics::judging;
    let mut options = Options { compare: false, ..Options::of(config) };
    options.spam.threads = 1;
    let read = super::read_corpus(dirs, &crate::train::trusted_ids(config), &super::Sources::of(config), &options, progress, cancel)?;
    let said = Said::load();
    let presets = Matcher::current();
    let path = judging::table_path();
    let previous = judging::cached(&path).map(|(t, _)| t);
    let trained = train(dirs, &read, &config.topics, &said, Some(&presets), previous.as_deref(), &options, progress, cancel)?;
    let written = replace && trained.replaces();
    if written && let Some(table) = &trained.table {
        judging::write_table(&path, table).map_err(LearnError::Io)?;
        let _ = judging::judge_kept(config, judge);
    }
    Ok(Here { trained, written })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topics::tests::{home, options, trusted};

    fn topic(id: &str, name: &str, folder: &str) -> Topic {
        Topic { id: id.into(), name: name.into(), seeds: sioul_core::topics::Seeds { folders: vec![folder.into()], categories: Vec::new() }, ..Topic::default() }
    }

    /// The gate: 20 examples from 3 senders, or 50 from one office.
    #[test]
    fn the_gate_wants_mail_and_senders() {
        assert!(through(20, 3) && through(50, 1) && through(80, 2));
        assert!(!through(19, 3) && !through(49, 2) && !through(0, 0));
    }

    /// The stand-in mailbox: the choir's, the tax office's and the doctor's
    /// topics through the gate, learned and folded; a small topic of two
    /// messages under it, no class, its mail never learned as "no topic";
    /// with the table as the one in use, the same training again is no worse
    /// and changes almost nothing; a table that knew another topic only
    /// keeps nothing from it.
    #[test]
    fn a_table_for_the_topics_through_the_gate() {
        let root = std::env::temp_dir().join(format!("sioul-topics-model-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (dirs, sources) = home(&root, "model");
        let options = Options { compare: false, ..options() };
        let read = crate::topics::read(&dirs, &trusted(), &sources, &options, true, None, &mut |_| {}, &Cancel::new()).unwrap();
        let topics = vec![topic("choir", "Chorale", "Chorale"), topic("tax", "Impôts", "Impôts"), topic("health", "Santé", "Santé"), topic("small", "Petit", "Nulle part")];
        // Two of the choir's messages given the small topic by your word: under the gate.
        let two: Vec<sioul_core::topics::labels::Act> = read.mail.iter().filter(|m| m.folders.iter().any(|f| f.ends_with("Chorale"))).filter_map(|m| m.id.clone()).take(2).map(|id| sioul_core::topics::labels::Act::Message { at: 1, message_id: id, topic: Some("small".into()), forget: false }).collect();
        let said = Said::of(&two);
        let first = train(&dirs, &read, &topics, &said, None, None, &options, &mut |_| {}, &Cancel::new()).unwrap();
        assert_eq!(first.outcome, Outcome::First, "{first:#?}");
        let small = first.topics.iter().find(|t| t.id == "small").unwrap();
        // Your two words, and a reply of yours that follows its conversation: under the gate.
        assert!((2..LEAST).contains(&small.examples) && !small.learned, "{small:?}");
        assert!(first.topics.iter().filter(|t| t.id != "small").all(|t| t.learned && t.examples >= LEAST), "{:#?}", first.topics);
        let table = first.table.clone().unwrap();
        assert_eq!(table.meta.topics, ["choir", "tax", "health"], "no class for the small topic");
        assert!(table.none && first.macro_f1.unwrap() > 0.6, "{first:#?}");
        // Read back from its bytes, as every device reads it.
        let again = Table::from_bytes(&table.to_bytes()).unwrap();
        assert_eq!(again.meta.topics, table.meta.topics);
        // The same again, the first table in use: each topic compared, the churn measured, the
        // outcome as the rule says; nothing learned from that table's guesses.
        let second = train(&dirs, &read, &topics, &said, None, Some(&again), &options, &mut |_| {}, &Cancel::new()).unwrap();
        assert_eq!(second.compared.iter().map(|(id, _, _)| id.as_str()).collect::<Vec<_>>(), ["choir", "tax", "health"], "{second:#?}");
        assert_eq!(second.topics.iter().map(|t| t.examples).collect::<Vec<_>>(), first.topics.iter().map(|t| t.examples).collect::<Vec<_>>(), "the same labels");
        let worse = second.compared.iter().find(|(_, was, now)| matches!((was, now), (Some(w), Some(n)) if n < &(w - TIE))).map(|(id, _, _)| id.clone());
        let churn = second.churn.unwrap();
        let expected = match worse {
            Some(topic) => Outcome::Worse { topic },
            None if churn >= CHURN => Outcome::Churn,
            None => Outcome::Replaced,
        };
        assert_eq!(second.outcome, expected, "{second:#?}");
        assert_eq!(second.replaces(), expected == Outcome::Replaced);
        // Nothing through the gate: no table.
        let none = train(&dirs, &read, &[topic("small", "Petit", "Nulle part")], &said, None, None, &options, &mut |_| {}, &Cancel::new()).unwrap();
        assert_eq!(none.outcome, Outcome::NoTopic);
        assert!(none.table.is_none() && none.topics[0].examples == small.examples);
        let _ = std::fs::remove_dir_all(root);
    }

    /// Topics learn from the corpus since forever (`read_corpus`), and the
    /// same way on every device: one thread, a fixed seed, the corpus in a
    /// fixed order. Two devices holding the same corpus (copied to another
    /// folder, as a second device downloads it) and the same labels make the
    /// same table, byte for byte but for when and where it was made.
    #[test]
    fn two_devices_with_the_same_corpus_make_the_same_table() {
        let root = std::env::temp_dir().join(format!("sioul-topics-same-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (dirs, sources) = home(&root, "same");
        let options = Options { compare: false, ..options() };
        let read = |dirs: &crate::Dirs| crate::topics::read_corpus(dirs, &trusted(), &sources, &options, &mut |_| {}, &Cancel::new()).unwrap();
        let here = read(&dirs);
        // The corpus's own mail: the Junk folder's and what the labels call spam stay out.
        assert!(here.mail.len() > 500 && here.mail.len() < 900, "{}", here.mail.len());
        assert!(here.mail.iter().all(|m| !m.folders.iter().any(|f| f == "Junk")));
        assert!(here.mail.iter().any(|m| m.folders.iter().any(|f| f == "Impôts")), "the server's names as you read them");
        assert!(here.seeds.iter().any(|s| s.name == "Chorale") && here.mail.iter().any(|m| m.proven.is_some()), "{:?}", here.seeds.iter().map(|s| &s.name).collect::<Vec<_>>());
        // A second device, the same corpus in its own folder.
        let other = crate::Dirs::under(&root.join("other"));
        copy_tree(&dirs.corpus(), &other.corpus());
        let there = read(&other);
        assert_eq!(here.mail.iter().map(|m| (m.id.clone(), m.date)).collect::<Vec<_>>(), there.mail.iter().map(|m| (m.id.clone(), m.date)).collect::<Vec<_>>(), "the same mail, in the same order");
        let topics = vec![topic("choir", "Chorale", "Chorale"), topic("tax", "Impôts", "Impôts"), topic("health", "Santé", "Santé")];
        let said = Said::default();
        let table = |dirs: &crate::Dirs, read: &Read| {
            let trained = train(dirs, read, &topics, &said, None, None, &options, &mut |_| {}, &Cancel::new()).unwrap();
            assert_eq!(trained.outcome, Outcome::First, "{trained:#?}");
            let mut table = trained.table.unwrap();
            (table.meta.trained_at, table.meta.device) = (0, String::new());
            table.to_bytes()
        };
        let (a, b) = (table(&dirs, &here), table(&other, &there));
        assert!(a == b, "two devices, two tables");
        assert_eq!(a, table(&dirs, &here), "and again on the same one");
        // T1 (review of 7 November 2026): the next training, each device with
        // another table in use (one none, one the first, dated long ago): the
        // same table again, nothing learned from either table's guesses.
        let mut older = Table::from_bytes(&a).unwrap();
        older.meta.trained_at = 0;
        let again = |dirs: &crate::Dirs, read: &Read, previous: Option<&Table>| {
            let trained = train(dirs, read, &topics, &said, None, previous, &options, &mut |_| {}, &Cancel::new()).unwrap();
            let mut table = trained.table.unwrap();
            (table.meta.trained_at, table.meta.device) = (0, String::new());
            table.to_bytes()
        };
        let (a2, b2) = (again(&dirs, &here, None), again(&other, &there, Some(&older)));
        assert!(a2 == b2, "a second training, another table in use on each: the same table");
        assert!(a2 == a, "the same corpus and labels: the same table as the first time");
        let _ = std::fs::remove_dir_all(root);
    }

    fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().filter_map(Result::ok) {
            let target = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
}

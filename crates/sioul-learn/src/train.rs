// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Training on demand: "Train now" in the mail settings, and
//! `sioul spam train` (docs/spam-filter.md, "Training on demand").
//!
//! 1. The corpus is brought up to date (`corpus::update`, by `run`).
//! 2. Each message gets one label (`labels`), and its words and header
//!    features from sioul-core's spam module, read as the Porch reads mail.
//! 3. fastText learns your mail's words: skip-gram, 100 dimensions,
//!    character n-grams of 3 to 6, 200 000 buckets, words seen 5 times.
//! 4. A message's vector is the plain mean of its words' vectors (fastText's
//!    `get_word_vector`), never normalized, so that the model folds; the
//!    header features follow it; each dimension is standardized with the
//!    training messages' mean and deviation.
//! 5. Oldest 80 % to learn, newest 20 % to test, by INTERNALDATE. The SVM's
//!    cost is chosen among 0.01, 0.1, 1 and 10 on the newest fifth of the
//!    learning part; ham weighs 5. Outside material (`external`), when
//!    imported, is split alike, source by source, by its own dates: its
//!    oldest 80 % joins the learning part at its dates, with its header
//!    features at the learning part's mean (0 once standardized: only its
//!    words weigh); its newest 20 % is held out, the baseline.
//! 6. Platt's A and B are fitted on scores each message got from an SVM
//!    trained on older mail only (forward folds).
//! 7. The numbers, on the newest 20 % of your own mail: aggregates only
//!    (`eval`); the same numbers on each outside source's held-out part,
//!    apart: the baseline.
//! 8. The model is folded into the reduced table (one scalar per word and per
//!    bucket, the header weights, the bias); the table must give the model's
//!    score within 1e-4 on every test message, or nothing is exported. It
//!    replaces `table.bin` only if it loses no more ham on the newest 20 %, at
//!    your spam threshold, than the table in place, on those of them the
//!    table in place never learned from (it learned from all mail before its
//!    training: on that mail it would be judged on what it was taught); the
//!    one before is kept as `table.prev.bin`. `trained.toml` says what
//!    happened, for the settings.
//! 9. Kept, the model is learned again, with the same cost, from every
//!    message, the newest month included (spam changes, and the filter that
//!    runs must have seen this month's): standardized anew, calibrated by
//!    Platt on forward folds over all of it (each fifth scored by an SVM that
//!    learned the fifths before it), folded and tested the same way. That is
//!    the table written; the numbers said are those of steps 5 to 7.
//!
//! A trial (`Options::replace` false: `sioul spam train --no-replace`, the
//! MCP's spam_train unless asked) stops after step 8's comparison: it says
//! what the table in place would become, and changes nothing of the filter,
//! the table, the one before, the language model nor `trained.toml`.
//! `train_with` also says what the test said message by message (`detail`).

use crate::corpus::{self, KEEP_FREE, Place, Record};
use crate::detail::{self, Detail, Origin, Tested};
use crate::eval::{self, Numbers};
use crate::external;
use crate::labels::{self, Copy, Evidence, Label};
use crate::platt::{self, Platt};
use crate::spamcore::{self as spam, N, Table};
use crate::svm::{self, HAM, Problem, SPAM};
use crate::{Cancel, Dirs, LearnError, Progress, Stage, io_error, now};
use serde::{Deserialize, Serialize};
use sioul_core::card::Card;
use sioul_core::config::Config;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufReader, BufWriter, Seek, Write};
use std::path::{Path, PathBuf};

/// How training goes; `Default` holds the values of docs/spam-filter.md.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// fastText: the vectors' dimension.
    pub dim: u32,
    /// fastText: passes over the corpus (5 to 10).
    pub epochs: u32,
    /// fastText: words seen fewer times are left out of the vocabulary.
    pub min_count: u32,
    /// fastText: the character n-grams' hash buckets.
    pub bucket: u32,
    /// fastText: character n-grams from `minn` to `maxn` characters.
    pub minn: u32,
    pub maxn: u32,
    /// Threads for fastText and the vectors; the computer's cores by default.
    pub threads: u32,
    /// The same seed and one thread give the same model.
    pub seed: u64,
    /// The SVM: ham's weight (calling ham spam costs this many times more).
    pub ham_weight: f64,
    /// The SVM's costs to choose from.
    pub costs: Vec<f64>,
    /// Your thresholds (`spam.threshold_spam`, `spam.threshold_unsure`).
    pub threshold_spam: f64,
    pub threshold_unsure: f64,
    /// Fewer messages of a kind than this: no training.
    pub least_of_each: u64,
    /// The table in place may be replaced (when no worse). False: a trial,
    /// which changes nothing of the filter and says what it would do.
    pub replace: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            dim: 100,
            epochs: 5,
            min_count: 5,
            bucket: 200_000,
            minn: 3,
            maxn: 6,
            threads: std::thread::available_parallelism().map_or(4, |n| n.get() as u32),
            seed: 1,
            ham_weight: 5.0,
            costs: vec![0.01, 0.1, 1.0, 10.0],
            threshold_spam: 0.95,
            threshold_unsure: 0.5,
            least_of_each: 20,
            replace: true,
        }
    }
}

impl Options {
    /// The defaults, with your thresholds (`[spam]`, read as the Porch reads them).
    pub fn of(config: &Config) -> Options {
        let (spam, unsure) = config.spam.thresholds();
        Options { threshold_spam: f64::from(spam), threshold_unsure: f64::from(unsure), ..Options::default() }
    }
}

/// The last training, as `trained.toml` keeps it for the settings: numbers only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// When it ended (Unix seconds), and how long it took.
    pub trained_at: i64,
    pub seconds: f64,
    pub tokenizer: u32,
    pub features: u32,
    /// The table took this training's model.
    pub replaced: bool,
    /// Why: "no-table" (none yet), "unreadable" (the one in place was made by
    /// another version, or damaged), "nothing-new" (no test message came after
    /// the one in place was trained), "no-worse", "worse" (kept).
    pub reason: String,
    pub threshold_spam: f64,
    pub threshold_unsure: f64,
    pub labels: labels::Summary,
    pub split: Split,
    /// Per account: ham and spam learned from, ham and spam tested on.
    pub accounts: BTreeMap<String, Counts>,
    pub model: ModelSummary,
    /// The numbers, on the newest 20 %, of the model learned from the oldest
    /// 80 % (`model`): what decided the replacement.
    pub test: Numbers,
    /// The same, ham an inbox holds since less than `detail::UNSETTLED_DAYS`
    /// left out: its label may not be settled (spam nobody looked at yet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_settled: Option<Numbers>,
    /// The table in place before, on the test messages it never learned from
    /// (those after its training: `compared`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<Numbers>,
    /// What the replacement compared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compared: Option<Compared>,
    /// The table written, when replaced: the same model learned again from every message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refit: Option<Refit>,
    /// Outside material learned from, by source (`external`): what each gave, and its baseline.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outside: BTreeMap<String, Outside>,
    /// A trial (`Options::replace` false): nothing was replaced nor written;
    /// `reason` says what the comparison found ("worse": the table in place
    /// would have stayed).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub trial: bool,
}

/// An outside source in a training: its oldest 80 % learned from, its
/// newest 20 % held out, and the numbers there (the baseline).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Outside {
    pub train_ham: u64,
    pub train_spam: u64,
    pub held_ham: u64,
    pub held_spam: u64,
    /// The numbers of the model learned from the oldest 80 % (yours and
    /// every source's), on this source's held-out part; none when nothing is held out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<Numbers>,
}

/// The replacement's comparison: the test messages the table in place never
/// learned from, and the ham each table sets aside among them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Compared {
    /// When the table in place was trained: only later messages are compared.
    pub since: i64,
    pub ham: u64,
    pub spam: u64,
    pub new_ham_lost: u64,
    pub current_ham_lost: u64,
}

/// The table written: the model learned again, with the same cost, from
/// every labelled message, the newest included.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Refit {
    pub ham: u64,
    pub spam: u64,
    /// Platt's A and B, from forward folds over every message: each fifth
    /// scored by an SVM learned from the fifths before it.
    pub platt_a: f64,
    pub platt_b: f64,
    pub calibration_scores: u64,
    /// The largest difference between the table's score and the model's on the test messages.
    pub fold_error: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Split {
    pub train_ham: u64,
    pub train_spam: u64,
    pub test_ham: u64,
    pub test_spam: u64,
    /// INTERNALDATE of the oldest test message (Unix seconds).
    pub test_from: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Counts {
    pub train_ham: u64,
    pub train_spam: u64,
    pub test_ham: u64,
    pub test_spam: u64,
}

/// The model learned from the oldest 80 %, whose numbers `test` gives, and
/// how it was asked to learn (`Options`: the command line's overrides too).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelSummary {
    pub dim: u32,
    /// Words fastText kept.
    pub vocabulary: u64,
    pub bucket: u32,
    pub minn: u32,
    pub maxn: u32,
    pub epochs: u32,
    /// Words seen fewer times were left out.
    #[serde(default)]
    pub min_count: u32,
    /// fastText's threads.
    #[serde(default)]
    pub threads: u32,
    /// The SVM's costs it chose from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub costs: Vec<f64>,
    /// The SVM's cost chosen, and the validation AUC each cost had.
    pub c: f64,
    pub validation_auc: BTreeMap<String, f64>,
    pub ham_weight: f64,
    pub platt_a: f64,
    pub platt_b: f64,
    /// Scores the calibration was fitted on.
    pub calibration_scores: u64,
    /// The largest difference between the table's score and the model's on the test messages.
    pub fold_error: f64,
    /// The size of the table written (the refit), or that would have been.
    pub table_bytes: u64,
    /// The classifier's weight on each header feature, standardized (its
    /// pull for one deviation of the feature): what the header facts learned.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub header_weights: BTreeMap<String, f64>,
}

/// The last training's summary; none before the first.
pub fn last(dirs: &Dirs) -> Option<Summary> {
    std::fs::read_to_string(dirs.trained()).ok().and_then(|t| toml::from_str(&t).ok())
}

/// "Train now": the corpus brought up to date from every account (each
/// password as the sync uses it), then `train` with your thresholds. For the
/// window: run it on a thread of its own; `progress` says each step,
/// `cancel` stops it at the next.
pub fn run(config: &Config, dirs: &Dirs, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<(corpus::Update, Summary), LearnError> {
    let update = corpus::update(&config.accounts, &|a| sioul_sync::secret::password(a), dirs, &corpus::free_space, progress, cancel);
    if update.cancelled {
        return Err(LearnError::Cancelled);
    }
    let summary = train(dirs, &trusted_ids(config), &Options::of(config), progress, cancel)?;
    Ok((update, summary))
}

/// The authserv-ids each account trusts, as the Porch reads them (Sioul's
/// own first, then the provider's: `Config::mail_sources`).
pub fn trusted_ids(config: &Config) -> BTreeMap<String, Vec<String>> {
    config.mail_sources().into_iter().filter_map(|s| Some((s.account?, s.trusted_ids))).collect()
}

/// One message on its way to the classifier.
struct Message {
    label: Label,
    date: i64,
    account: String,
    /// Your own mail: the copy learned from, and what decided its label.
    place: Option<Place>,
    evidence: Option<Evidence>,
    /// Outside material: its source (an index into the training's sources); none for your own mail.
    source: Option<usize>,
    /// Outside material: its rank in its source, as `external::read_all` reads it (`detail`).
    ordinal: u64,
    /// Ham in an inbox, too young for its label to be settled (`detail::unsettled`).
    unsettled: bool,
    /// Its header features are known (your own mail); outside material's are not: set to the mean.
    known: bool,
    features: [f32; N],
    /// Where its words are in the tokenized corpus.
    offset: u64,
    length: u64,
    words: u64,
}

/// The tokenized corpus, removed whatever happens.
struct TokenFile(PathBuf);

impl Drop for TokenFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// fastText reads lines of at most 1024 words: longer messages are cut in lines of this many.
const LINE_WORDS: usize = 1000;

/// Trains on the corpus as it is (see the module's steps). The table is
/// replaced only when no worse; `trained.toml` says what happened.
pub fn train(dirs: &Dirs, trusted: &BTreeMap<String, Vec<String>>, options: &Options, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Summary, LearnError> {
    train_with(dirs, trusted, options, 0, progress, cancel).map(|(summary, _)| summary)
}

/// `train`, and what its test said message by message (`detail`): the test
/// messages counted by account and folder, the grid of thresholds, and up
/// to `errors` errors of each kind, your own mail's and each outside
/// source's, as the model learned from the oldest 80 % judged them.
pub fn train_with(dirs: &Dirs, trusted: &BTreeMap<String, Vec<String>>, options: &Options, errors: usize, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<(Summary, Detail), LearnError> {
    let started = std::time::Instant::now();
    let started_at = now();
    let check = || if cancel.cancelled() { Err(LearnError::Cancelled) } else { Ok(()) };

    // Labels: a first pass over the corpus, keeping only what deciding needs.
    let mut copies = Vec::new();
    let mut texts = 0u64;
    corpus::read_all(dirs, |record| {
        texts += record.plain.as_ref().map_or(0, |t| t.text.len() as u64) + record.html.as_ref().map_or(0, |t| t.text.len() as u64) + record.header.len() as u64;
        copies.push(Copy::of(&record));
        if copies.len() % 1000 == 0 {
            progress(&Progress { stage: Stage::Labels, done: copies.len() as u64, total: 0, detail: String::new() });
        }
    })?;
    let log = labels::read_log(dirs);
    let moved = labels::read_moved(dirs);
    let (labeled, label_summary) = labels::decide(copies, &log, &moved);
    // Outside material, by source: a message also in your own mail is learned from yours.
    let own_keys: HashSet<String> = labeled.iter().map(|l| l.key.clone()).collect();
    let yours = |message: &external::Message| message.message_id().is_some_and(|id| own_keys.contains(&id));
    let mut outside: BTreeMap<String, Outside> = BTreeMap::new();
    external::read_all(dirs, |source, message| {
        if yours(&message) {
            return;
        }
        texts += (message.subject.len() + message.text.len() + message.html.as_ref().map_or(0, String::len)) as u64;
        let counts = outside.entry(source.to_string()).or_default();
        match message.label {
            Label::Ham => counts.train_ham += 1,
            Label::Spam => counts.train_spam += 1,
        }
    })?;
    let sources: Vec<String> = outside.keys().cloned().collect();
    progress(&Progress { stage: Stage::Labels, done: label_summary.records, total: label_summary.records, detail: String::new() });
    let (ham, spam) = outside.values().fold((label_summary.ham, label_summary.spam), |(h, s), o| (h + o.train_ham, s + o.train_spam));
    if ham < options.least_of_each || spam < options.least_of_each {
        return Err(LearnError::TooFew { ham, spam });
    }
    check()?;

    // Room: the tokenized corpus (at most the texts' size) and the language model.
    let model_bytes = (u64::from(options.bucket) + 2 * (label_summary.records + ham + spam).min(500_000)) * u64::from(options.dim) * 4;
    std::fs::create_dir_all(&dirs.cache).map_err(|e| io_error(&dirs.cache, e))?;
    if let Some(free) = corpus::free_space(&dirs.cache)
        && free < KEEP_FREE + texts + model_bytes
    {
        return Err(LearnError::Disk(format!("{} MB free, {} MB needed above the 1 GB kept free", free >> 20, (texts + model_bytes) >> 20)));
    }

    // Words and header features: a second pass, for the copies learned from.
    let chosen: HashMap<Place, usize> = labeled.iter().enumerate().map(|(i, l)| (l.place.clone(), i)).collect();
    let token_path = dirs.cache.join(format!("tokens-{}.txt", std::process::id()));
    let token_file = TokenFile(token_path.clone());
    let mut messages: Vec<Option<Message>> = (0..labeled.len()).map(|_| None).collect();
    let mut outside_messages: Vec<Message> = Vec::new();
    // Your own messages, then the outside material's.
    let tokens_total = labeled.len() as u64 + (ham + spam) - (label_summary.ham + label_summary.spam);
    {
        let file = std::fs::File::create(&token_path).map_err(|e| io_error(&token_path, e))?;
        let mut out = BufWriter::new(file);
        let mut offset = 0u64;
        let mut done = 0u64;
        let mut failure = None;
        corpus::read_all(dirs, |record| {
            if failure.is_some() {
                return;
            }
            let Some(&index) = chosen.get(&record.place()) else { return };
            if messages[index].is_some() {
                return;
            }
            let (words, features) = read_message(&record, trusted);
            let mut length = 0u64;
            for line in words.chunks(LINE_WORDS) {
                let text = line.join(" ");
                if let Err(e) = out.write_all(text.as_bytes()).and_then(|()| out.write_all(b"\n")) {
                    failure = Some(io_error(&token_path, e));
                    return;
                }
                length += text.len() as u64 + 1;
            }
            let l = &labeled[index];
            messages[index] = Some(Message {
                label: l.label,
                date: l.date,
                account: l.place.account.clone(),
                place: Some(l.place.clone()),
                evidence: Some(l.evidence),
                source: None,
                ordinal: 0,
                unsettled: detail::unsettled(l.label, l.role, l.date, started_at),
                known: true,
                features,
                offset,
                length,
                words: words.len() as u64,
            });
            offset += length;
            done += 1;
            if done % 500 == 0 {
                progress(&Progress { stage: Stage::Tokens, done, total: tokens_total, detail: String::new() });
            }
        })?;
        // Outside material after your own mail, in the same file: its words teach the language model too.
        let mut ranks: HashMap<String, u64> = HashMap::new();
        external::read_all(dirs, |source, message| {
            let rank = ranks.entry(source.to_string()).or_default();
            let ordinal = *rank;
            *rank += 1;
            if failure.is_some() || yours(&message) {
                return;
            }
            let words = message.words();
            let mut length = 0u64;
            for line in words.chunks(LINE_WORDS) {
                let text = line.join(" ");
                if let Err(e) = out.write_all(text.as_bytes()).and_then(|()| out.write_all(b"\n")) {
                    failure = Some(io_error(&token_path, e));
                    return;
                }
                length += text.len() as u64 + 1;
            }
            let source = sources.iter().position(|s| s == source);
            outside_messages.push(Message {
                label: message.label,
                date: message.date,
                account: String::new(),
                place: None,
                evidence: None,
                source,
                ordinal,
                unsettled: false,
                known: false,
                features: [0.0; N],
                offset,
                length,
                words: words.len() as u64,
            });
            offset += length;
            done += 1;
            if done % 500 == 0 {
                progress(&Progress { stage: Stage::Tokens, done, total: tokens_total, detail: String::new() });
            }
        })?;
        if let Some(e) = failure {
            return Err(e);
        }
        out.flush().map_err(|e| io_error(&token_path, e))?;
    }
    // A copy that could not be read again (the corpus changed meanwhile) is left out.
    let mut messages: Vec<Message> = messages.into_iter().flatten().collect();
    messages.append(&mut outside_messages);
    progress(&Progress { stage: Stage::Tokens, done: messages.len() as u64, total: messages.len() as u64, detail: String::new() });
    check()?;

    // fastText.
    progress(&Progress { stage: Stage::Language, done: 0, total: 0, detail: String::new() });
    let args = fasttext::args::Args {
        input: token_path.clone(),
        model: fasttext::args::ModelName::SkipGram,
        loss: fasttext::args::LossName::NegativeSampling,
        dim: options.dim as i32,
        epoch: options.epochs as i32,
        min_count: options.min_count as i32,
        bucket: options.bucket as i32,
        minn: options.minn as i32,
        maxn: options.maxn as i32,
        thread: options.threads.max(1) as i32,
        seed: options.seed as i32,
        verbose: 0,
        ..fasttext::args::Args::default()
    };
    let language = fasttext::FastText::train_with_abort(args, cancel.flag()).map_err(|e| LearnError::Language(e.to_string()))?;
    check()?;
    progress(&Progress { stage: Stage::Language, done: 1, total: 1, detail: String::new() });

    // Message vectors, read back from the tokenized corpus.
    let dim = options.dim as usize;
    let vectors = message_vectors(&language, &token_path, &messages, options.threads.max(1) as usize, progress, cancel)?;
    check()?;

    // Your own mail: each account's ham, and its spam, oldest 80 % to learn,
    // newest 20 % to test (`newest_fifths`); each outside source alike, its
    // ham and its spam, by their own dates, the newest 20 % held out (the
    // baseline). The test and held-out messages' words kept for their scores.
    let by_date = |ids: &mut Vec<usize>| ids.sort_by_key(|&i| (messages[i].date, i));
    let mut order: Vec<usize> = (0..messages.len()).collect();
    by_date(&mut order);
    let newest_fifth = |source: Option<usize>| -> (Vec<usize>, Vec<usize>) {
        let ids: Vec<usize> = order.iter().copied().filter(|&i| messages[i].source == source).collect();
        let (mut learned, mut tested) = newest_fifths(&ids, |i| (messages[i].account.as_str(), messages[i].label == Label::Spam));
        by_date(&mut learned);
        by_date(&mut tested);
        (learned, tested)
    };
    let (mut train_ids, test_ids) = newest_fifth(None);
    let mut held: Vec<Vec<usize>> = Vec::with_capacity(sources.len());
    for s in 0..sources.len() {
        let (learned, held_out) = newest_fifth(Some(s));
        train_ids.extend(learned);
        held.push(held_out);
    }
    by_date(&mut train_ids);
    let (train_ids, test_ids) = (train_ids.as_slice(), test_ids.as_slice());
    let test_words = words_of(&token_path, &messages, test_ids)?;
    let held_words = held.iter().map(|ids| words_of(&token_path, &messages, ids)).collect::<Result<Vec<_>, _>>()?;
    drop(token_file);
    let rows = Rows { vectors: &vectors, messages: &messages, dim };
    let width = rows.width();

    // The model of the oldest 80 %: its cost chosen on the newest fifth of
    // those, Platt on forward folds over them, then learned from all of them.
    let (mean, deviation) = rows.standardization(train_ids);
    let fits = (options.costs.len() + 5) as u64;
    let mut fitted = 0u64;
    let (c, validation_auc, model, sigmoid, calibration_scores) = {
        let train_x = rows.standardized(train_ids, &mean, &deviation);
        let train_y = rows.labels(train_ids);
        let n_train = train_ids.len();
        let fit_end = (n_train as f64 * 0.8).round() as usize;
        let mut validation_auc = BTreeMap::new();
        let mut best: Option<(f64, f64)> = None;
        for &c in &options.costs {
            check()?;
            let model = fit_prefix(&train_x, &train_y, width, fit_end, c, options, cancel)?;
            let scores: Vec<(f64, bool)> = (fit_end..n_train).map(|r| (model.decision(&train_x[r * width..(r + 1) * width]), train_y[r] == SPAM)).collect();
            // AUC first; without both kinds in the validation, the weighted hinge loss.
            let quality = eval::auc(&scores).unwrap_or_else(|| -hinge_loss(&scores, options.ham_weight));
            validation_auc.insert(format!("{c}"), quality);
            if best.is_none_or(|(_, q)| quality > q + 1e-9) {
                best = Some((c, quality));
            }
            fitted += 1;
            progress(&Progress { stage: Stage::Classifier, done: fitted, total: fits, detail: format!("C = {c}") });
        }
        let c = best.map_or(0.1, |(c, _)| c);
        let (model, sigmoid, calibration_scores) = fit_with_platt(&train_x, &train_y, width, c, options, cancel, &mut |detail| {
            fitted += 1;
            progress(&Progress { stage: Stage::Classifier, done: fitted, total: fits, detail: detail.to_string() });
        })?;
        (c, validation_auc, model, sigmoid, calibration_scores)
    };
    check()?;

    // Its table, and the fold's test on every test message; its numbers.
    progress(&Progress { stage: Stage::Evaluation, done: 0, total: test_ids.len() as u64, detail: String::new() });
    let mut split = Split::default();
    let mut accounts: BTreeMap<String, Counts> = BTreeMap::new();
    for &i in train_ids {
        if messages[i].source.is_some() {
            continue;
        }
        let counts = accounts.entry(messages[i].account.clone()).or_default();
        match messages[i].label {
            Label::Ham => (split.train_ham += 1, counts.train_ham += 1),
            Label::Spam => (split.train_spam += 1, counts.train_spam += 1),
        };
    }
    for &i in test_ids {
        let counts = accounts.entry(messages[i].account.clone()).or_default();
        match messages[i].label {
            Label::Ham => (split.test_ham += 1, counts.test_ham += 1),
            Label::Spam => (split.test_spam += 1, counts.test_spam += 1),
        };
    }
    split.test_from = test_ids.first().map_or(0, |&i| messages[i].date);
    // The table as every device will read it: its bytes, read back (scores in f32, words sorted by hash).
    let table = Table::from_bytes(&fold(&language, &model, &mean, &deviation, sigmoid, options, &split).to_bytes()).map_err(LearnError::Fold)?;
    let test_x = rows.standardized(test_ids, &mean, &deviation);
    let fold_error = fold_error(&table, &model, &test_x, width, &test_words, test_ids, &messages)?;
    let new_scored: Vec<(f64, bool)> = test_ids.iter().enumerate().map(|(row, &i)| (f64::from(table.score(&test_words[row], &messages[i].features).p), messages[i].label == Label::Spam)).collect();
    drop(test_x);
    let test = eval::evaluate(&new_scored, options.threshold_spam, options.threshold_unsure);
    // The same, ham an inbox holds since less than a month left out: it may be spam nobody has looked at.
    let settled: Vec<(f64, bool)> = test_ids.iter().zip(&new_scored).filter(|&(&i, _)| !messages[i].unsettled).map(|(_, &s)| s).collect();
    let test_settled = Some(eval::evaluate(&settled, options.threshold_spam, options.threshold_unsure));
    // Each outside source's held-out part: the baseline, its header features at the table's means (they weigh nothing).
    let lacking = lacking(&table);
    let mut tested: Vec<Tested> = test_ids.iter().zip(&new_scored).map(|(&i, &(p, _))| tested_of(&messages[i], p, &sources)).collect();
    for (s, name) in sources.iter().enumerate() {
        let scored: Vec<(f64, bool)> = held[s].iter().zip(&held_words[s]).map(|(&i, words)| (f64::from(table.score(words, &lacking).p), messages[i].label == Label::Spam)).collect();
        tested.extend(held[s].iter().zip(&scored).map(|(&i, &(p, _))| tested_of(&messages[i], p, &sources)));
        let counts = outside.entry(name.clone()).or_default();
        counts.held_ham = scored.iter().filter(|(_, spam)| !spam).count() as u64;
        counts.held_spam = scored.len() as u64 - counts.held_ham;
        counts.train_ham -= counts.held_ham;
        counts.train_spam -= counts.held_spam;
        counts.baseline = (!scored.is_empty()).then(|| eval::evaluate(&scored, options.threshold_spam, options.threshold_unsure));
    }
    // Message by message: counted by account and folder, the grid, and the worst errors, read again.
    let detail = detail::of(dirs, &tested, options.threshold_spam, options.threshold_unsure, errors)?;
    drop(tested);
    progress(&Progress { stage: Stage::Evaluation, done: test_ids.len() as u64, total: test_ids.len() as u64, detail: String::new() });

    // The table in place, on the same messages: those of them it never
    // learned from (it learned from every message before its training), so
    // that both are judged on mail they never saw.
    let current = match Table::read(&dirs.table()) {
        Ok(current) => Ok(Some(current)),
        Err(_) if !dirs.table().exists() => Ok(None),
        Err(e) => Err(e),
    };
    let (current_numbers, compared) = match &current {
        Ok(Some(current)) => {
            let since = current.meta.trained_at;
            let unseen: Vec<usize> = (0..test_ids.len()).filter(|&row| messages[test_ids[row]].date > since).collect();
            let label = |row: usize| messages[test_ids[row]].label == Label::Spam;
            let theirs: Vec<(f64, bool)> = unseen.iter().map(|&row| (f64::from(current.score(&test_words[row], &messages[test_ids[row]].features).p), label(row))).collect();
            let ours: Vec<(f64, bool)> = unseen.iter().map(|&row| new_scored[row]).collect();
            let compared = Compared { since, ham: theirs.iter().filter(|(_, s)| !s).count() as u64, spam: theirs.iter().filter(|(_, s)| *s).count() as u64, new_ham_lost: eval::evaluate(&ours, options.threshold_spam, options.threshold_unsure).at_spam.ham_called_spam.count, current_ham_lost: 0 };
            let numbers = eval::evaluate(&theirs, options.threshold_spam, options.threshold_unsure);
            (Some(numbers.clone()), Some(Compared { current_ham_lost: numbers.at_spam.ham_called_spam.count, ..compared }))
        }
        _ => (None, None),
    };
    let (replaced, reason) = match (&current, &compared) {
        (Err(_), _) => (true, "unreadable"),
        (Ok(None), _) => (true, "no-table"),
        (Ok(Some(_)), Some(c)) => replacement(c.new_ham_lost, c.current_ham_lost, c.ham + c.spam),
        (Ok(Some(_)), None) => (true, "no-table"),
    };
    check()?;

    // Kept: the same model learned again from every message, the newest month included, and written.
    // A trial stops here: nothing of the filter changes.
    let replaced = replaced && options.replace;
    let mut table_bytes = table.to_bytes().len() as u64;
    let refit = if replaced {
        let refits = 6u64;
        let mut done = 0u64;
        let mut step = |detail: &str| {
            done += 1;
            progress(&Progress { stage: Stage::Export, done, total: refits, detail: detail.to_string() });
        };
        let (mut written, refit) = refit_all(&language, &rows, &order, test_ids, &test_words, c, options, cancel, &mut step)?;
        written.meta.metrics = metrics(&test);
        check()?;
        // Written whole or not at all; the table it replaces is kept as table.prev.bin.
        written.write(&dirs.table()).map_err(|e| io_error(&dirs.table(), e))?;
        table_bytes = std::fs::metadata(dirs.table()).map_or(table_bytes, |m| m.len());
        let fresh = dirs.language().with_extension("bin.new");
        language.save_model(&fresh).map_err(|e| io_error(&fresh, e))?;
        std::fs::rename(&fresh, dirs.language()).map_err(|e| io_error(&dirs.language(), e))?;
        step("");
        Some(refit)
    } else {
        progress(&Progress { stage: Stage::Export, done: 1, total: 1, detail: String::new() });
        None
    };
    let summary = Summary {
        trained_at: now(),
        seconds: started.elapsed().as_secs_f64(),
        tokenizer: spam::TOKENIZER,
        features: spam::FEATURES,
        replaced,
        reason: reason.to_string(),
        threshold_spam: options.threshold_spam,
        threshold_unsure: options.threshold_unsure,
        labels: label_summary,
        split,
        accounts,
        model: ModelSummary {
            dim: options.dim,
            vocabulary: language.dict().nwords().max(1) as u64 - 1,
            bucket: options.bucket,
            minn: options.minn,
            maxn: options.maxn,
            epochs: options.epochs,
            min_count: options.min_count,
            threads: options.threads,
            costs: options.costs.clone(),
            c,
            validation_auc,
            ham_weight: options.ham_weight,
            platt_a: sigmoid.a,
            platt_b: sigmoid.b,
            calibration_scores,
            fold_error,
            table_bytes,
            header_weights: (0..N).map(|h| (spam::NAMES[h].to_string(), model.w[dim + h])).collect(),
        },
        test,
        test_settled,
        current: current_numbers,
        compared,
        refit,
        outside,
        trial: !options.replace,
    };
    // What the settings say of the last training: never a trial's.
    if options.replace {
        let text = toml::to_string(&summary).map_err(|e| io_error(&dirs.trained(), e))?;
        corpus::write_whole(&dirs.trained(), text.as_bytes())?;
    }
    Ok((summary, detail))
}

/// A test message as `detail` takes it: where it is, its label, how likely spam.
fn tested_of(message: &Message, p: f64, sources: &[String]) -> Tested {
    let origin = match (&message.place, message.source) {
        (Some(place), _) => Origin::Own { place: place.clone(), evidence: message.evidence.unwrap_or(Evidence::Folder) },
        (None, source) => Origin::Outside { source: source.and_then(|s| sources.get(s)).cloned().unwrap_or_default(), ordinal: message.ordinal },
    };
    Tested { p, label: message.label, date: message.date, origin, unsettled: message.unsettled }
}

/// The oldest 80 % and the newest 20 % of each group of `ids` (in time
/// order): an account's ham, its spam; an outside source's. Every group is
/// on both sides, so that a group whose mail all came late (a provider that
/// keeps spam a month) is learned from too, and none is told apart by when it came.
pub(crate) fn newest_fifths<'a>(ids: &[usize], group: impl Fn(usize) -> (&'a str, bool)) -> (Vec<usize>, Vec<usize>) {
    let mut groups: BTreeMap<(&str, bool), Vec<usize>> = BTreeMap::new();
    for &i in ids {
        groups.entry(group(i)).or_default().push(i);
    }
    let (mut learned, mut tested) = (Vec::new(), Vec::new());
    for (_, mut ids) in groups {
        let newest = ids.split_off(ids.len() - (ids.len() as f64 * 0.2).round() as usize);
        learned.extend(ids);
        tested.extend(newest);
    }
    (learned, tested)
}

/// The test's numbers the table carries, for the settings of every device.
fn metrics(test: &Numbers) -> BTreeMap<String, f64> {
    let mut metrics = BTreeMap::from([
        ("threshold_spam".to_string(), test.at_spam.threshold),
        ("threshold_unsure".to_string(), test.at_unsure.threshold),
        ("ham_called_spam".to_string(), test.at_spam.ham_called_spam.fraction()),
        ("spam_caught".to_string(), test.at_spam.spam_caught.fraction()),
        ("ham_called_unsure".to_string(), test.at_unsure.ham_called_spam.fraction()),
        ("spam_caught_unsure".to_string(), test.at_unsure.spam_caught.fraction()),
        ("unsure".to_string(), test.unsure.fraction()),
    ]);
    if let Some(auc) = test.auc {
        metrics.insert("auc".to_string(), auc);
    }
    metrics
}

/// Whether the new table replaces the one in place, and why, from the ham
/// each sets aside among the `compared` test messages the one in place never
/// learned from. None of them (nothing came since it was trained): nothing
/// says it is better; the new one learned from the same mail, and replaces it.
pub(crate) fn replacement(new_ham_lost: u64, current_ham_lost: u64, compared: u64) -> (bool, &'static str) {
    if compared == 0 {
        (true, "nothing-new")
    } else if new_ham_lost <= current_ham_lost {
        (true, "no-worse")
    } else {
        (false, "worse")
    }
}

/// A message's words and header features, from its corpus record, as the
/// Porch reads the message itself.
pub(crate) fn read_message(record: &Record, trusted: &BTreeMap<String, Vec<String>>) -> (Vec<String>, [f32; N]) {
    let Some(card) = spam::card_of(record) else { return (Vec::new(), [0.0; N]) };
    features_of(&card, trusted.get(&record.account).map_or(&[][..], Vec::as_slice), (record.date > 0).then_some(record.date))
}

/// A card's words and header features.
pub(crate) fn features_of(card: &Card, trusted: &[String], internal_date: Option<i64>) -> (Vec<String>, [f32; N]) {
    let tokens = spam::tokens(&card.subject, &card.excerpt);
    // Your provider's results alone, as the Porch's filter reads them: the corpus has no stamp of Sioul's.
    let auth = sioul_core::spam::features::provider_results(&card.headers, trusted);
    let features = spam::features(card, auth.as_ref(), internal_date, &tokens.links);
    // fastText splits on whitespace: a word never holds any.
    let words = tokens.words.into_iter().filter(|w| !w.is_empty() && !w.contains(char::is_whitespace)).collect();
    (words, features)
}

/// The classifier's rows: each message's vector, then its header features.
struct Rows<'a> {
    vectors: &'a [f64],
    messages: &'a [Message],
    dim: usize,
}

impl Rows<'_> {
    fn width(&self) -> usize {
        self.dim + N
    }

    fn raw(&self, i: usize) -> impl Iterator<Item = f64> + '_ {
        self.vectors[i * self.dim..(i + 1) * self.dim].iter().copied().chain(self.messages[i].features.iter().map(|&x| f64::from(x)))
    }

    /// The mean and the deviation of each column over some messages; the
    /// header features' over those that have them (your own mail), each
    /// over the messages where it is known (not missing: `f32::NAN`).
    fn standardization(&self, ids: &[usize]) -> (Vec<f64>, Vec<f64>) {
        standardization(|k| self.raw(ids[k]).collect(), |k| self.messages[ids[k]].known, ids.len(), self.width(), self.dim)
    }

    /// Some messages' rows, standardized, one after the other; a message's
    /// header features it does not have (outside material), or that are
    /// missing, at the mean: 0.
    fn standardized(&self, ids: &[usize], mean: &[f64], deviation: &[f64]) -> Vec<f32> {
        ids.iter()
            .flat_map(|&i| {
                let known = self.messages[i].known;
                self.raw(i)
                    .zip(mean)
                    .zip(deviation)
                    .enumerate()
                    .map(move |(c, ((x, m), s))| if *s > 0.0 && (known || c < self.dim) && !x.is_nan() { ((x - m) / s) as f32 } else { 0.0 })
                    .collect::<Vec<f32>>()
            })
            .collect()
    }

    fn labels(&self, ids: &[usize]) -> Vec<i8> {
        ids.iter().map(|&i| if self.messages[i].label == Label::Spam { SPAM } else { HAM }).collect()
    }
}

/// The mean and the deviation (population) of `width` columns over `n` rows,
/// in two passes, rows made one at a time; 0 for a column that never changes.
/// The columns from `dim` on (the header features) only over the rows that
/// have them (`known`); without any, 0. A missing value (NaN) counts in neither.
fn standardization(row: impl Fn(usize) -> Vec<f64>, known: impl Fn(usize) -> bool, n: usize, width: usize, dim: usize) -> (Vec<f64>, Vec<f64>) {
    let counted = |column: usize, k: usize, x: f64| (column < dim || known(k)) && !x.is_nan();
    let mut counts = vec![0usize; width];
    let mut mean = vec![0.0; width];
    for k in 0..n {
        for (c, ((m, x), count)) in mean.iter_mut().zip(row(k)).zip(counts.iter_mut()).enumerate() {
            if counted(c, k, x) {
                *m += x;
                *count += 1;
            }
        }
    }
    let counts: Vec<f64> = counts.iter().map(|&c| c.max(1) as f64).collect();
    mean.iter_mut().zip(&counts).for_each(|(m, n)| *m /= n);
    let mut deviation = vec![0.0; width];
    for k in 0..n {
        for (c, ((d, x), m)) in deviation.iter_mut().zip(row(k)).zip(&mean).enumerate() {
            if counted(c, k, x) {
                *d += (x - m) * (x - m);
            }
        }
    }
    deviation.iter_mut().zip(&counts).for_each(|(d, n)| {
        *d = (*d / n).sqrt();
        if *d < 1e-12 {
            *d = 0.0;
        }
    });
    (mean, deviation)
}

/// Header features at a table's means: what outside material, which has
/// none, is scored with (they weigh nothing).
fn lacking(table: &Table) -> [f32; N] {
    let mut features = [0.0; N];
    for (x, m) in features.iter_mut().zip(&table.means) {
        *x = *m;
    }
    features
}

/// The SVM learned from the first `end` rows of `x` (time order), cost `c`.
fn fit_prefix(x: &[f32], y: &[i8], width: usize, end: usize, c: f64, options: &Options, cancel: &Cancel) -> Result<svm::Model, LearnError> {
    let params = svm::Params { c, ham_weight: options.ham_weight, seed: options.seed, ..svm::Params::default() };
    svm::train(&Problem { x: &x[..end * width], dim: width, y: &y[..end] }, &params, &|| cancel.cancelled()).map_err(|_| LearnError::Cancelled)
}

/// The SVM learned from every row (time order), cost `c`, and Platt's
/// sigmoid from forward folds over them: each fifth scored by an SVM learned
/// from the fifths before it. When the folds give none (too few of a kind
/// early on), the model's own scores, which make it surer than it is.
/// `step` is told after each fit; the scores the sigmoid was fitted on are counted.
fn fit_with_platt(x: &[f32], y: &[i8], width: usize, c: f64, options: &Options, cancel: &Cancel, step: &mut dyn FnMut(&str)) -> Result<(svm::Model, Platt, u64), LearnError> {
    let n = y.len();
    let scores = |model: &svm::Model, rows: std::ops::Range<usize>| -> Vec<(f64, bool)> { rows.map(|r| (model.decision(&x[r * width..(r + 1) * width]), y[r] == SPAM)).collect() };
    let mut calibration = Vec::new();
    for k in 1..5 {
        let (start, end) = (n * k / 5, n * (k + 1) / 5);
        if y[..start].contains(&SPAM) && y[..start].contains(&HAM) && end > start {
            let model = fit_prefix(x, y, width, start, c, options, cancel)?;
            calibration.extend(scores(&model, start..end));
        }
        step("Platt");
    }
    let model = fit_prefix(x, y, width, n, c, options, cancel)?;
    step(&format!("C = {c}"));
    let calibration = if platt::fit(&calibration).is_some() { calibration } else { scores(&model, 0..n) };
    let sigmoid = platt::fit(&calibration).unwrap_or(Platt { a: -1.0, b: 0.0 });
    Ok((model, sigmoid, calibration.len() as u64))
}

/// The largest difference between a table's score and its model's on some
/// messages (`x`, their standardized rows); over 1e-4, the fold is broken.
fn fold_error(table: &Table, model: &svm::Model, x: &[f32], width: usize, words: &[Vec<String>], ids: &[usize], messages: &[Message]) -> Result<f64, LearnError> {
    let mut error = 0f64;
    for (row, &i) in ids.iter().enumerate() {
        let full = model.decision(&x[row * width..(row + 1) * width]);
        let score = table.score(&words[row], &messages[i].features);
        error = error.max((f64::from(score.f) - full).abs());
    }
    if error > 1e-4 {
        return Err(LearnError::Fold(format!("the table's score differs from the model's by {error:e}")));
    }
    Ok(error)
}

/// The model learned again, cost `c`, from every message (`order`, time
/// order), the newest included: standardized anew, Platt on forward folds over
/// all of it, folded; the fold tested on the test messages. The table to
/// write (its metadata counts what it learned from; its metrics are the
/// caller's), and what to say of it.
#[allow(clippy::too_many_arguments)]
fn refit_all(
    language: &fasttext::FastText,
    rows: &Rows,
    order: &[usize],
    test_ids: &[usize],
    test_words: &[Vec<String>],
    c: f64,
    options: &Options,
    cancel: &Cancel,
    step: &mut dyn FnMut(&str),
) -> Result<(Table, Refit), LearnError> {
    let width = rows.width();
    let (mean, deviation) = rows.standardization(order);
    let (model, sigmoid, calibration_scores) = {
        let x = rows.standardized(order, &mean, &deviation);
        let y = rows.labels(order);
        fit_with_platt(&x, &y, width, c, options, cancel, step)?
    };
    // What it learned from: every message, outside material included; tested on your newest.
    let ham_of = |ids: &[usize]| ids.iter().filter(|&&i| rows.messages[i].label == Label::Ham).count() as u64;
    let all = Split {
        train_ham: ham_of(order),
        train_spam: order.len() as u64 - ham_of(order),
        test_ham: ham_of(test_ids),
        test_spam: test_ids.len() as u64 - ham_of(test_ids),
        test_from: test_ids.first().map_or(0, |&i| rows.messages[i].date),
    };
    let table = Table::from_bytes(&fold(language, &model, &mean, &deviation, sigmoid, options, &all).to_bytes()).map_err(LearnError::Fold)?;
    let test_x = rows.standardized(test_ids, &mean, &deviation);
    let fold_error = fold_error(&table, &model, &test_x, width, test_words, test_ids, rows.messages)?;
    Ok((table, Refit { ham: all.train_ham, spam: all.train_spam, platt_a: sigmoid.a, platt_b: sigmoid.b, calibration_scores, fold_error }))
}

/// The mean weighted squared hinge loss of scores (lower is better).
fn hinge_loss(scores: &[(f64, bool)], ham_weight: f64) -> f64 {
    let total: f64 = scores
        .iter()
        .map(|&(f, spam)| {
            let (y, weight) = if spam { (1.0, 1.0) } else { (-1.0, ham_weight) };
            weight * (1.0 - y * f).max(0.0).powi(2)
        })
        .sum();
    total / scores.len().max(1) as f64
}

/// Every message's vector: the plain mean of its words' fastText vectors,
/// computed on `threads` threads, in f64, from the tokenized corpus.
fn message_vectors(language: &fasttext::FastText, path: &Path, messages: &[Message], threads: usize, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Vec<f64>, LearnError> {
    let dim = language.get_dimension() as usize;
    // The vocabulary's vectors once (most words are in it); the others on the way.
    let nwords = language.dict().nwords() as usize;
    let mut known = vec![0f32; nwords * dim];
    std::thread::scope(|s| {
        for (t, chunk) in known.chunks_mut(nwords.div_ceil(threads).max(1) * dim).enumerate() {
            s.spawn(move || {
                let first = t * nwords.div_ceil(threads).max(1);
                for (k, out) in chunk.chunks_mut(dim).enumerate() {
                    language.get_word_vector_into(language.dict().get_word((first + k) as i32), out);
                }
            });
        }
    });
    let mut vectors = vec![0f64; messages.len() * dim];
    let per = messages.len().div_ceil(threads).max(1);
    let done = std::sync::atomic::AtomicU64::new(0);
    let failed: std::sync::Mutex<Option<LearnError>> = std::sync::Mutex::new(None);
    std::thread::scope(|s| {
        let mut workers = Vec::new();
        for (t, out) in vectors.chunks_mut(per * dim).enumerate() {
            let (known, done, failed) = (&known, &done, &failed);
            workers.push(s.spawn(move || {
                let first = t * per;
                let mut file = match std::fs::File::open(path) {
                    Ok(file) => file,
                    Err(e) => {
                        *failed.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(io_error(path, e));
                        return;
                    }
                };
                let mut word = vec![0f32; dim];
                for (k, vector) in out.chunks_mut(dim).enumerate() {
                    if cancel.cancelled() {
                        return;
                    }
                    let message = &messages[first + k];
                    if message.words == 0 {
                        continue;
                    }
                    let mut text = vec![0u8; message.length as usize];
                    if let Err(e) = file.seek(std::io::SeekFrom::Start(message.offset)).and_then(|_| std::io::Read::read_exact(&mut file, &mut text)) {
                        *failed.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(io_error(path, e));
                        return;
                    }
                    let mut count = 0u64;
                    for token in String::from_utf8_lossy(&text).split_ascii_whitespace() {
                        let row = match language.get_word_id(token) {
                            Some(id) => &known[id as usize * dim..(id as usize + 1) * dim],
                            None => {
                                language.get_word_vector_into(token, &mut word);
                                &word[..]
                            }
                        };
                        for (v, &x) in vector.iter_mut().zip(row) {
                            *v += f64::from(x);
                        }
                        count += 1;
                    }
                    if count > 0 {
                        vector.iter_mut().for_each(|v| *v /= count as f64);
                    }
                    done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }));
        }
        // Progress from this thread while the others work.
        loop {
            let finished = workers.iter().all(|w| w.is_finished());
            progress(&Progress { stage: Stage::Features, done: done.load(std::sync::atomic::Ordering::Relaxed), total: messages.len() as u64, detail: String::new() });
            if finished {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
    if let Some(e) = failed.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner) {
        return Err(e);
    }
    if cancel.cancelled() {
        return Err(LearnError::Cancelled);
    }
    Ok(vectors)
}

/// The words of some messages, read back from the tokenized corpus.
fn words_of(path: &Path, messages: &[Message], ids: &[usize]) -> Result<Vec<Vec<String>>, LearnError> {
    let file = std::fs::File::open(path).map_err(|e| io_error(path, e))?;
    let mut reader = BufReader::new(file);
    ids.iter()
        .map(|&i| {
            let message = &messages[i];
            let mut text = vec![0u8; message.length as usize];
            reader.seek(std::io::SeekFrom::Start(message.offset)).and_then(|_| std::io::Read::read_exact(&mut reader, &mut text)).map_err(|e| io_error(path, e))?;
            Ok(String::from_utf8_lossy(&text).split_ascii_whitespace().map(str::to_string).collect())
        })
        .collect()
}

/// The model folded into the reduced table (spam-core-api.md): with
/// `u = w/σ` over the vector's dimensions, each vocabulary word's scalar is
/// `u · get_word_vector(word)`, each bucket's `u · its input row`; the
/// standardization's means become `text_mean = u · μ` and the header means.
fn fold(language: &fasttext::FastText, model: &svm::Model, mean: &[f64], deviation: &[f64], sigmoid: Platt, options: &Options, split: &Split) -> Table {
    let dim = options.dim as usize;
    let u: Vec<f64> = (0..dim).map(|d| if deviation[d] > 0.0 { model.w[d] / deviation[d] } else { 0.0 }).collect();
    let dot = |row: &[f32]| -> f64 { u.iter().zip(row).map(|(a, &b)| a * f64::from(b)).sum() };
    let (vocabulary, _) = language.get_vocab();
    let mut vector = vec![0f32; dim];
    let words: Vec<(u64, f32)> = vocabulary
        .iter()
        .filter(|w| w.as_str() != fasttext::dictionary::EOS)
        .map(|w| {
            language.get_word_vector_into(w, &mut vector);
            (spam::word_hash(w), dot(&vector) as f32)
        })
        .collect();
    let nwords = i64::from(language.dict().nwords());
    let input = language.input_matrix();
    let buckets: Vec<f32> = (0..i64::from(options.bucket)).map(|b| dot(input.row(nwords + b)) as f32).collect();
    let text_mean = u.iter().zip(&mean[..dim]).map(|(a, m)| a * m).sum::<f64>();
    let weights: Vec<f32> = (0..N).map(|h| if deviation[dim + h] > 0.0 { (model.w[dim + h] / deviation[dim + h]) as f32 } else { 0.0 }).collect();
    let means: Vec<f32> = (0..N).map(|h| mean[dim + h] as f32).collect();
    Table {
        tokenizer: spam::TOKENIZER,
        features: spam::FEATURES,
        dim: options.dim,
        minn: options.minn,
        maxn: options.maxn,
        bucket: options.bucket,
        words,
        buckets,
        weights,
        means,
        bias: model.b as f32,
        text_mean: text_mean as f32,
        platt_a: sigmoid.a as f32,
        platt_b: sigmoid.b as f32,
        meta: spam::Meta {
            trained_at: now(),
            ham: split.train_ham,
            spam: split.train_spam,
            test_ham: split.test_ham,
            test_spam: split.test_spam,
            metrics: BTreeMap::new(),
            device: sioul_sync::lease::host_name(),
        },
    }
}

/// The table in place, tested on the newest 20 % of the corpus as it is now.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub labels: labels::Summary,
    /// The test messages' counts (`train_*` stay 0).
    pub split: Split,
    pub numbers: Numbers,
    /// When the table was trained (Unix seconds), and on how much.
    pub table: spam::Meta,
    /// Each outside source's newest 20 %, the baseline, apart.
    pub outside: BTreeMap<String, Numbers>,
    /// The test messages that came after the table was trained: it never
    /// learned from them. The table a training writes learned from every
    /// message before it, the newest fifth included: on the others, it is
    /// judged on what it was taught. None when none came since.
    pub unseen: Option<Unseen>,
}

/// The numbers on the test messages the table in place never learned from.
#[derive(Debug, Clone, PartialEq)]
pub struct Unseen {
    /// When the table was trained: only later messages count.
    pub since: i64,
    pub numbers: Numbers,
}

/// Tests the table in place on the newest fifth of the corpus as it is now,
/// without training: aggregate numbers only (`sioul spam eval`). New mail
/// since the training makes it a test of how the table holds up.
pub fn evaluate(dirs: &Dirs, trusted: &BTreeMap<String, Vec<String>>, options: &Options, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Evaluation, LearnError> {
    evaluate_with(dirs, trusted, options, 0, progress, cancel).map(|(evaluation, _)| evaluation)
}

/// `evaluate`, and what the test said message by message (`detail`), with
/// up to `errors` errors of each kind (`sioul spam eval --errors`).
pub fn evaluate_with(dirs: &Dirs, trusted: &BTreeMap<String, Vec<String>>, options: &Options, errors: usize, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<(Evaluation, Detail), LearnError> {
    let table = Table::read(&dirs.table()).map_err(LearnError::NoTable)?;
    let mut copies = Vec::new();
    corpus::read_all(dirs, |record| copies.push(Copy::of(&record)))?;
    let (mut labeled, label_summary) = labels::decide(copies, &labels::read_log(dirs), &labels::read_moved(dirs));
    progress(&Progress { stage: Stage::Labels, done: label_summary.records, total: label_summary.records, detail: String::new() });
    if cancel.cancelled() {
        return Err(LearnError::Cancelled);
    }
    // The newest fifth of each account's ham and of its spam, as a training tests on (`newest_fifths`).
    labeled.sort_by(|a, b| (a.date, &a.key).cmp(&(b.date, &b.key)));
    let all: Vec<usize> = (0..labeled.len()).collect();
    let (_, newest) = newest_fifths(&all, |i| (labeled[i].place.account.as_str(), labeled[i].label == Label::Spam));
    let newest: HashSet<usize> = newest.into_iter().collect();
    let (tested_labels, labeled): (Vec<(usize, labels::Labeled)>, Vec<(usize, labels::Labeled)>) = labeled.into_iter().enumerate().partition(|(i, _)| newest.contains(i));
    let (tested_labels, labeled): (Vec<labels::Labeled>, Vec<labels::Labeled>) = (tested_labels.into_iter().map(|(_, l)| l).collect(), labeled.into_iter().map(|(_, l)| l).collect());
    let wanted: HashMap<Place, usize> = tested_labels.iter().enumerate().map(|(i, l)| (l.place.clone(), i)).collect();
    let mut scored: Vec<Option<(f64, bool)>> = vec![None; tested_labels.len()];
    let mut done = 0u64;
    corpus::read_all(dirs, |record| {
        let Some(&i) = wanted.get(&record.place()) else { return };
        if scored[i].is_some() || cancel.cancelled() {
            return;
        }
        let (words, features) = read_message(&record, trusted);
        scored[i] = Some((f64::from(table.score(&words, &features).p), tested_labels[i].label == Label::Spam));
        done += 1;
        if done % 500 == 0 {
            progress(&Progress { stage: Stage::Evaluation, done, total: tested_labels.len() as u64, detail: String::new() });
        }
    })?;
    if cancel.cancelled() {
        return Err(LearnError::Cancelled);
    }
    // Message by message, for the detail; then the numbers, and those of the messages it never learned from.
    let since = table.meta.trained_at;
    let mut tested: Vec<Tested> = tested_labels
        .iter()
        .zip(&scored)
        .filter_map(|(l, scored)| scored.map(|(p, _)| Tested { p, label: l.label, date: l.date, origin: Origin::Own { place: l.place.clone(), evidence: l.evidence }, unsettled: detail::unsettled(l.label, l.role, l.date, now()) }))
        .collect();
    let unseen: Vec<(f64, bool)> = tested.iter().filter(|t| t.date > since).map(|t| (t.p, t.label == Label::Spam)).collect();
    let unseen = (!unseen.is_empty()).then(|| Unseen { since, numbers: eval::evaluate(&unseen, options.threshold_spam, options.threshold_unsure) });
    let scored: Vec<(f64, bool)> = scored.into_iter().flatten().collect();
    let split = Split {
        test_ham: scored.iter().filter(|(_, s)| !s).count() as u64,
        test_spam: scored.iter().filter(|(_, s)| *s).count() as u64,
        test_from: tested_labels.first().map_or(0, |l| l.date),
        ..Split::default()
    };
    // Each outside source's newest fifth, as a training holds it out: the baseline.
    let own_keys: HashSet<String> = labeled.iter().chain(&tested_labels).map(|l| l.key.clone()).collect();
    let mut sources: BTreeMap<String, Vec<(u64, external::Message)>> = BTreeMap::new();
    let mut ranks: HashMap<String, u64> = HashMap::new();
    external::read_all(dirs, |source, message| {
        let rank = ranks.entry(source.to_string()).or_default();
        let ordinal = *rank;
        *rank += 1;
        if !message.message_id().is_some_and(|id| own_keys.contains(&id)) {
            sources.entry(source.to_string()).or_default().push((ordinal, message));
        }
    })?;
    let lacking = lacking(&table);
    let mut outside = BTreeMap::new();
    for (name, mut messages) in sources {
        // Its ham and its spam, each its newest fifth, as a training holds them out.
        messages.sort_by_key(|(_, m)| m.date);
        let all: Vec<usize> = (0..messages.len()).collect();
        let (_, newest) = newest_fifths(&all, |i| ("", messages[i].1.label == Label::Spam));
        let newest: HashSet<usize> = newest.into_iter().collect();
        let held: Vec<(u64, external::Message)> = messages.into_iter().enumerate().filter(|(i, _)| newest.contains(i)).map(|(_, m)| m).collect();
        let scored: Vec<(f64, bool)> = held.iter().map(|(_, m)| (f64::from(table.score(&m.words(), &lacking).p), m.label == Label::Spam)).collect();
        tested.extend(held.iter().zip(&scored).map(|((ordinal, m), &(p, _))| Tested { p, label: m.label, date: m.date, origin: Origin::Outside { source: name.clone(), ordinal: *ordinal }, unsettled: false }));
        outside.insert(name, eval::evaluate(&scored, options.threshold_spam, options.threshold_unsure));
    }
    let detail = detail::of(dirs, &tested, options.threshold_spam, options.threshold_unsure, errors)?;
    let numbers = eval::evaluate(&scored, options.threshold_spam, options.threshold_unsure);
    Ok((Evaluation { labels: label_summary, split, numbers, table: table.meta, outside, unseen }, detail))
}

/// What weighed in one message's verdict. It names words of the message: for
/// you, in your terminal (`sioul spam why`), never through MCP.
#[derive(Debug, Clone, PartialEq)]
pub struct Why {
    /// The probability of spam, and the score it comes from (positive for spam).
    pub p: f64,
    pub f: f64,
    /// Up to five words or placeholders that pushed toward spam, the most first, with their part of the score.
    pub words: Vec<(String, f64)>,
    /// Up to three header features that pushed toward spam, the most first, by name.
    pub headers: Vec<(String, f64)>,
}

/// The table's verdict on a whole message (a file's bytes), and why.
/// `trusted` are its account's authserv-ids; `internal_date` when it reached
/// the server (a Maildir file's modification time).
pub fn why(dirs: &Dirs, raw: &[u8], trusted: &[String], internal_date: Option<i64>) -> Result<Why, String> {
    let table = Table::read(&dirs.table())?;
    let card = Card::from_bytes(raw).ok_or_else(|| "not a message".to_string())?;
    let (words, features) = features_of(&card, trusted, internal_date);
    let score = table.score(&words, &features);
    // Each word's and each feature's share of the score, the most toward spam first, as the Porch says why (`spam::Why::of`).
    let words = score.words.iter().filter(|(_, s)| *s > 0.0).take(5).map(|(w, s)| (w.clone(), f64::from(*s))).collect();
    let headers = score.signs.iter().filter(|(_, s)| *s > 0.0).take(3).map(|&(h, s)| (spam::NAMES[h].to_string(), f64::from(s))).collect();
    Ok(Why { p: f64::from(score.p), f: f64::from(score.f), words, headers })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-learn-train-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Small enough to train in a second; one thread and a seed: the same model each time.
    fn small() -> Options {
        Options { dim: 16, epochs: 5, min_count: 2, bucket: 2000, threads: 1, seed: 7, ..Options::default() }
    }

    fn trusted() -> BTreeMap<String, Vec<String>> {
        ["home", "work"].into_iter().map(|a| (a.to_string(), vec!["mx.example.net".to_string()])).collect()
    }

    fn records(mail: &[synthetic::Mail]) -> Vec<Record> {
        mail.iter().enumerate().map(|(i, m)| synthetic::record_of(m, 1, i as u32 + 1)).collect()
    }

    /// Invented ham and spam: the SVM tells them apart, Platt rises, the table
    /// gives the model's scores, and the files are where the settings read them.
    #[test]
    fn learns_invented_spam_apart_from_ham() {
        let root = scratch("learns");
        let dirs = Dirs::under(&root);
        let mail = synthetic::mailbox(11, 500, 300);
        corpus::store(&dirs, &records(&mail)).unwrap();
        let mut stages = Vec::new();
        let summary = train(&dirs, &trusted(), &small(), &mut |p| stages.push(p.stage), &Cancel::new()).unwrap();
        assert_eq!((summary.labels.ham, summary.labels.spam), (500, 300), "{:?}", summary.labels);
        assert_eq!(summary.split.train_ham + summary.split.train_spam + summary.split.test_ham + summary.split.test_spam, 800);
        assert_eq!(summary.split.test_ham + summary.split.test_spam, 160, "the newest fifth");
        let test = &summary.test;
        assert!(test.auc.is_some_and(|a| a > 0.97), "{test:?}");
        assert!(test.at_unsure.spam_caught.fraction() > 0.9, "{test:?}");
        assert!(test.at_spam.ham_called_spam.fraction() < 0.02, "{test:?}");
        assert!(summary.model.platt_a < 0.0, "Platt rises with the score: {:?}", summary.model);
        assert!(summary.model.fold_error < 1e-4, "{:?}", summary.model);
        assert!(summary.model.vocabulary > 50 && summary.model.calibration_scores > 100, "{:?}", summary.model);
        assert!(summary.replaced && summary.reason == "no-table");
        assert!(dirs.table().exists() && dirs.language().exists() && !dirs.previous_table().exists());
        assert_eq!(last(&dirs).as_ref(), Some(&summary), "trained.toml says it all");
        // The table written learned again from every message, the newest included; the numbers are the 80 % model's.
        let refit = summary.refit.as_ref().expect("replaced: refit");
        assert_eq!(refit.ham + refit.spam, 800);
        assert_eq!(summary.split.train_ham + summary.split.train_spam, 640, "the numbers' model learned from the oldest 80 %");
        assert!(refit.fold_error < 1e-4 && refit.platt_a < 0.0 && refit.calibration_scores >= 600, "{refit:?}");
        let written = Table::read(&dirs.table()).unwrap();
        assert_eq!((written.meta.ham + written.meta.spam, written.meta.test_ham + written.meta.test_spam), (800, 160));
        assert_eq!(written.meta.metrics.get("auc").copied(), summary.test.auc);
        assert!(!written.meta.device.is_empty());
        // In order, every stage; the tokenized corpus is gone.
        let firsts: Vec<Stage> = stages.iter().fold(Vec::new(), |mut seen, s| {
            if seen.last() != Some(s) {
                seen.push(*s);
            }
            seen
        });
        assert_eq!(firsts, vec![Stage::Labels, Stage::Tokens, Stage::Language, Stage::Features, Stage::Classifier, Stage::Evaluation, Stage::Export]);
        assert_eq!(std::fs::read_dir(&dirs.cache).unwrap().count(), 0);
        // Why, for a spam it never saw: its spam words weigh most.
        let spam = synthetic::message(&mut crate::Rng::new(99), true, 9999, 1_790_000_000);
        let why = why(&dirs, &spam, &[], None).unwrap();
        assert!(why.p > 0.5 && why.f > 0.0, "{why:?}");
        assert_eq!(why.words.len(), 5);
        assert!(why.words.iter().chain(&why.headers).all(|(_, share)| *share > 0.0) && why.headers.len() <= 3, "{why:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    /// The table is replaced when the new one loses no more ham than the one
    /// in place, on the test messages that one never learned from; kept when
    /// it would lose more; the one before is kept as table.prev.bin.
    #[test]
    fn a_worse_table_never_replaces_a_better_one() {
        let root = scratch("replace");
        let dirs = Dirs::under(&root);
        let mut mail = synthetic::mailbox(5, 400, 250);
        // Ham that reads like spam, the newest of all: a table that calls them spam loses them.
        let mut rng = crate::Rng::new(5);
        let newest = mail.iter().map(|m| m.date).max().unwrap();
        for k in 0..15 {
            let date = newest + 3600 * (k + 1);
            mail.push(synthetic::Mail { account: "home", folder: "INBOX", role: sioul_core::folders::Role::Inbox, flags: vec![], date, raw: synthetic::message(&mut rng, true, 50_000 + k as u32, date) });
        }
        corpus::store(&dirs, &records(&mail)).unwrap();
        let first = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        assert!(first.replaced && first.reason == "no-table" && first.refit.is_some());
        assert!(first.test.at_spam.ham_called_spam.count > 0, "the spam-like ham is lost: {:?}", first.test);
        let first_table = std::fs::read(dirs.table()).unwrap();
        // Nothing came since: the table in place learned from all of it; replaced, the first kept as the previous.
        let second = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        assert!(second.replaced && second.reason == "nothing-new", "{} {:?}", second.reason, second.compared);
        assert_eq!(second.compared.map(|c| c.ham + c.spam), Some(0));
        assert_eq!(std::fs::read(dirs.previous_table()).unwrap(), first_table);
        // A table in place trained before the newest messages, which never calls anything spam:
        // it loses no ham among those; the new one loses the spam-like ham, and is kept out.
        let midway = mail.iter().map(|m| m.date).max().unwrap() - 3600 * 40;
        let mut never = Table::read(&dirs.table()).unwrap();
        never.bias = -1000.0;
        never.meta.trained_at = midway;
        never.write(&dirs.table()).unwrap();
        let kept = std::fs::read(dirs.table()).unwrap();
        let third = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        assert!(!third.replaced && third.reason == "worse" && third.refit.is_none(), "{} {:?}", third.reason, third.compared);
        let compared = third.compared.unwrap();
        assert_eq!(compared.since, midway);
        assert!(compared.ham >= 15 && compared.current_ham_lost == 0 && compared.new_ham_lost > 0, "{compared:?}");
        assert_eq!(third.current.as_ref().map(|c| c.ham + c.spam), Some(compared.ham + compared.spam), "the same messages");
        assert_eq!(std::fs::read(dirs.table()).unwrap(), kept, "the table in place stays");
        assert_eq!(last(&dirs).map(|s| s.reason), Some("worse".to_string()));
        // One that calls everything spam loses every such ham: the new one is no worse, replaced.
        let mut always = Table::read(&dirs.table()).unwrap();
        always.bias = 1000.0;
        always.write(&dirs.table()).unwrap();
        let fourth = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        assert!(fourth.replaced && fourth.reason == "no-worse", "{} {:?}", fourth.reason, fourth.compared);
        // A table made for another version cannot be compared: replaced, and said so.
        std::fs::write(dirs.table(), b"not a table").unwrap();
        let fifth = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        assert!(fifth.replaced && fifth.reason == "unreadable");
        let _ = std::fs::remove_dir_all(root);
    }

    /// Spam changes: a campaign of the newest weeks, its words never seen
    /// before, is in the table that runs, learned again from every message.
    #[test]
    fn the_newest_month_is_in_the_table_that_runs() {
        let root = scratch("newest");
        let dirs = Dirs::under(&root);
        let mut mail = synthetic::mailbox(17, 600, 300);
        let newest = mail.iter().map(|m| m.date).max().unwrap();
        let mut rng = crate::Rng::new(17);
        for k in 0..90u32 {
            let date = newest + 3600 * (i64::from(k) + 1);
            mail.push(synthetic::Mail { account: "work", folder: "Junk", role: sioul_core::folders::Role::Junk, flags: vec![], date, raw: synthetic::campaign(&mut rng, 60_000 + k, date) });
        }
        corpus::store(&dirs, &records(&mail)).unwrap();
        let summary = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        let refit = summary.refit.expect("replaced: refit");
        assert_eq!(refit.ham + refit.spam, 990, "every message, the campaign included");
        // A message of that campaign, never seen: spam to the table that runs.
        let fresh = synthetic::campaign(&mut crate::Rng::new(99), 70_000, newest + 86_400 * 3);
        let why = why(&dirs, &fresh, &[], None).unwrap();
        assert!(why.p >= 0.95, "{why:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    /// Outside material: its words join the language model's and the
    /// classifier's learning, its header features at the mean (0 once
    /// standardized: no pull on the header weights); split in time source by
    /// source, its newest fifth held out and measured apart, the baseline;
    /// your own mail tested as before. It alone is not enough of yours to
    /// test on, but enough to learn.
    #[test]
    fn outside_material_teaches_words_and_gives_a_baseline() {
        let root = scratch("outside");
        let dirs = Dirs::under(&root);
        corpus::store(&dirs, &records(&synthetic::mailbox(23, 300, 150))).unwrap();
        // Two years older than your own mail, from an older filter's dump: subject and text, no headers.
        let mut rng = crate::Rng::new(23);
        let lines: Vec<String> = (0..500)
            .map(|k| {
                let spam = k % 3 == 0;
                let date = 1_640_995_200 + 3600 * i64::from(k);
                let card = Card::from_bytes(&synthetic::message(&mut rng, spam, 90_000 + k, date)).unwrap();
                serde_json::json!({ "label": if spam { "spam" } else { "ham" }, "date": date, "subject": card.subject, "text": card.excerpt }).to_string()
            })
            .collect();
        let file = root.join("old-filter.jsonl");
        std::fs::write(&file, lines.join("\n")).unwrap();
        let imported = external::import(&dirs, &file, None).unwrap();
        assert_eq!((imported.sources["old-filter"].ham, imported.sources["old-filter"].spam), (333, 167));
        let summary = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        // Your own mail tested as before, on its newest fifth.
        assert_eq!(summary.split.test_ham + summary.split.test_spam, 90);
        assert_eq!(summary.split.train_ham + summary.split.train_spam, 360, "your own mail's oldest 80 %");
        // The outside source split by its own dates: 400 learned from, 100 held out and measured.
        let outside = &summary.outside["old-filter"];
        assert_eq!((outside.train_ham + outside.train_spam, outside.held_ham + outside.held_spam), (400, 100), "{outside:?}");
        let baseline = outside.baseline.as_ref().expect("held out: measured");
        assert!(baseline.auc.is_some_and(|a| a > 0.9) && baseline.ham + baseline.spam == 100, "{baseline:?}");
        // The table in use learned from all of it.
        assert_eq!(summary.refit.as_ref().map(|r| r.ham + r.spam), Some(450 + 500));
        // `sioul spam eval`: the same baseline on the table in place.
        let evaluation = evaluate(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        assert_eq!(evaluation.outside["old-filter"].ham + evaluation.outside["old-filter"].spam, 100);
        // Removed: the next training knows nothing of it.
        external::remove(&dirs, "old-filter").unwrap();
        let without = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        assert!(without.outside.is_empty() && without.refit.map(|r| r.ham + r.spam) == Some(450));
        let _ = std::fs::remove_dir_all(root);
    }

    /// A trial (`--no-replace`) says what the table would become and changes
    /// nothing of the filter; asked for errors, it gives the worst of each
    /// kind with their cards read again. `evaluate_with` says the same of the
    /// table in place, and apart the numbers of the messages it never learned from.
    #[test]
    fn a_trial_changes_nothing_and_names_its_errors() {
        let root = scratch("trial");
        let dirs = Dirs::under(&root);
        corpus::store(&dirs, &records(&synthetic::mailbox(31, 300, 200))).unwrap();
        // No table yet: a trial would write one, and writes nothing.
        let trial = Options { replace: false, ..small() };
        let (summary, detail) = train_with(&dirs, &trusted(), &trial, 5, &mut |_| {}, &Cancel::new()).unwrap();
        assert!(summary.trial && !summary.replaced && summary.reason == "no-table" && summary.refit.is_none(), "{} {}", summary.reason, summary.replaced);
        assert!(!dirs.table().exists() && !dirs.language().exists() && !dirs.trained().exists() && last(&dirs).is_none());
        assert_eq!(std::fs::read_dir(&dirs.cache).unwrap().count(), 0, "the tokenized corpus is gone");
        assert_eq!((summary.model.threads, summary.model.min_count, summary.model.costs.len()), (1, 2, 4), "how it was asked to learn");
        let tested: u64 = detail.by_account.values().map(|c| c.ham.total() + c.spam.total()).sum();
        assert_eq!(tested, summary.split.test_ham + summary.split.test_spam);
        assert_eq!(detail.grid.len(), detail::GRID.len());
        assert!(detail.errors.ham_called_spam.len() <= 5 && detail.errors.spam_missed.len() <= 5);
        for w in detail.errors.ham_called_spam.iter().chain(&detail.errors.spam_missed) {
            assert!(w.card.is_some() && !w.outside && !w.account.is_empty(), "{w:?}");
        }
        assert!(detail.errors.ham_called_spam.windows(2).all(|w| w[0].p >= w[1].p), "the surest first");
        assert!(detail.errors.spam_missed.windows(2).all(|w| w[0].p <= w[1].p), "the least sure first");
        // Trained for good: written, and trained.toml says it.
        let kept = train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()).unwrap();
        assert!(kept.replaced && !kept.trial && last(&dirs).is_some_and(|l| !l.trial));
        let in_place = std::fs::read(dirs.table()).unwrap();
        // The newest fifth was learned from by the table in place: nothing unseen.
        let (evaluation, detail) = evaluate_with(&dirs, &trusted(), &small(), 3, &mut |_| {}, &Cancel::new()).unwrap();
        assert!(evaluation.unseen.is_none(), "{:?}", evaluation.unseen);
        assert!(detail.errors.ham_called_spam.len() <= 3 && detail.errors.spam_missed.len() <= 3);
        // Mail since the table's training: tested apart.
        let mut rng = crate::Rng::new(31);
        let soon = now() + 3600;
        let newer: Vec<Record> = (0..40u32)
            .map(|k| {
                let spam = k % 2 == 0;
                let date = soon + i64::from(k) * 60;
                let (folder, role) = if spam { ("Junk", sioul_core::folders::Role::Junk) } else { ("INBOX", sioul_core::folders::Role::Inbox) };
                let mail = synthetic::Mail { account: "home", folder, role, flags: vec![], date, raw: synthetic::message(&mut rng, spam, 80_000 + k, date) };
                synthetic::record_of(&mail, 1, 20_000 + k)
            })
            .collect();
        corpus::store(&dirs, &newer).unwrap();
        let (evaluation, _) = evaluate_with(&dirs, &trusted(), &small(), 0, &mut |_| {}, &Cancel::new()).unwrap();
        let unseen = evaluation.unseen.expect("mail since the training");
        assert_eq!(unseen.since, Table::read(&dirs.table()).unwrap().meta.trained_at);
        assert_eq!(unseen.numbers.ham + unseen.numbers.spam, 40);
        // A trial with a table in place: compared, never replaced.
        let (second, _) = train_with(&dirs, &trusted(), &trial, 0, &mut |_| {}, &Cancel::new()).unwrap();
        assert!(second.trial && !second.replaced && second.compared.is_some(), "{second:?}");
        assert_eq!(std::fs::read(dirs.table()).unwrap(), in_place, "the table in place stays");
        assert!(!dirs.previous_table().exists());
        assert_eq!(last(&dirs).map(|l| l.trained_at), Some(kept.trained_at), "trained.toml stays the last training's");
        let _ = std::fs::remove_dir_all(root);
    }

    /// Outside material's header features, which it does not have, stand at
    /// the mean of your own mail's: standardized, exactly 0, whatever is
    /// written there; the mean and the deviation are your own mail's alone.
    #[test]
    fn what_outside_material_lacks_weighs_nothing() {
        let message = |known: bool, x: f32| Message { label: Label::Ham, date: 0, account: String::new(), place: None, evidence: None, source: (!known).then_some(0), ordinal: 0, unsettled: false, known, features: [x; N], offset: 0, length: 0, words: 0 };
        let messages = vec![message(true, 1.0), message(true, 3.0), message(false, 1000.0)];
        let vectors = vec![1.0, 2.0, 3.0];
        let rows = Rows { vectors: &vectors, messages: &messages, dim: 1 };
        let ids = [0, 1, 2];
        let (mean, deviation) = rows.standardization(&ids);
        assert_eq!((mean[1], deviation[1]), (2.0, 1.0), "your own mail's alone");
        assert_eq!((mean[0], deviation[0]), (2.0, (2.0f64 / 3.0).sqrt()), "the words' over all");
        let x = rows.standardized(&ids, &mean, &deviation);
        let width = rows.width();
        assert!(x[2 * width + 1..3 * width].iter().all(|v| *v == 0.0), "outside: 0 for every header feature");
        assert_ne!(x[2 * width], 0.0, "its words count");
        assert_eq!((x[width + 1], x[1]), (1.0, -1.0));
    }

    #[test]
    fn too_few_or_stopped() {
        let root = scratch("few");
        let dirs = Dirs::under(&root);
        corpus::store(&dirs, &records(&synthetic::mailbox(3, 100, 5))).unwrap();
        assert!(matches!(train(&dirs, &trusted(), &small(), &mut |_| {}, &Cancel::new()), Err(LearnError::TooFew { spam: 5, .. })));
        corpus::store(&dirs, &records(&synthetic::mailbox(4, 60, 60))).unwrap();
        let cancel = Cancel::new();
        cancel.cancel();
        assert_eq!(train(&dirs, &trusted(), &small(), &mut |_| {}, &cancel).err(), Some(LearnError::Cancelled));
        assert!(!dirs.table().exists());
        let _ = std::fs::remove_dir_all(root);
    }

    /// The phone's n-gram hashing (sioul-core) is fastText's: the same rows,
    /// in the same order, for words in the vocabulary and out of it, accents
    /// included (fastText's sign-extended bytes); and an unknown word's
    /// vector is the mean of those rows.
    #[test]
    fn the_phones_hashing_is_fasttexts() {
        let root = scratch("hashing");
        let text = root.join("text.txt");
        let mut rng = crate::Rng::new(1);
        let lines: Vec<String> = (0..300)
            .map(|_| {
                let spam = rng.unit() < 0.5;
                String::from_utf8(synthetic::message(&mut rng, spam, 1, 1_750_000_000)).unwrap().replace("\r\n", " ")
            })
            .collect();
        std::fs::write(&text, lines.join("\n") + "\n réunion naïve été\n").unwrap();
        let (minn, maxn, bucket) = (3, 6, 1000);
        let args = fasttext::args::Args {
            input: text.clone(),
            dim: 8,
            epoch: 1,
            min_count: 1,
            bucket: bucket as i32,
            minn: minn as i32,
            maxn: maxn as i32,
            thread: 1,
            ..fasttext::args::Args::default()
        };
        let model = fasttext::FastText::train(args).unwrap();
        let nwords = model.dict().nwords();
        let mut checked = (0, 0);
        let long = "z".repeat(40);
        let words = ["lottery", "bonjour", "réunion", "naïve", "zzqxv", "loterrie", "é", "a", "ab", "_URL_", "élève", "🙂ok", &long];
        for word in words {
            let ids = model.dict().get_subwords_for_string(word);
            let expected: Vec<i32> = spam::ngram_buckets(word, minn, maxn, bucket).iter().map(|&b| nwords + b as i32).collect();
            match model.get_word_id(word) {
                Some(id) => {
                    assert_eq!(ids[0], id, "{word}");
                    assert_eq!(ids[1..], expected[..], "{word}");
                    checked.0 += 1;
                }
                None => {
                    assert_eq!(ids, expected, "{word}");
                    let vector = model.get_word_vector(word);
                    let input = model.input_matrix();
                    for d in 0..8 {
                        let mean = expected.iter().map(|&id| f64::from(input.row(i64::from(id))[d])).sum::<f64>() / expected.len() as f64;
                        assert!((mean - f64::from(vector[d])).abs() < 1e-6, "{word}[{d}]");
                    }
                    checked.1 += 1;
                }
            }
        }
        assert!(checked.0 >= 4 && checked.1 >= 5, "in and out of the vocabulary: {checked:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    /// The sizes of docs/spam-filter.md (dimension 100, 200 000 buckets, words seen
    /// 5 times) on a larger invented mailbox: how big the table is, how long
    /// training takes. `cargo test --release -p sioul-learn -- --ignored measure --nocapture`.
    #[test]
    #[ignore]
    fn measure_the_spec_sizes() {
        let root = scratch("measure");
        let dirs = Dirs::under(&root);
        let mail = synthetic::mailbox(21, 6000, 3000);
        corpus::store(&dirs, &records(&mail)).unwrap();
        let started = std::time::Instant::now();
        let summary = train(&dirs, &trusted(), &Options::default(), &mut |_| {}, &Cancel::new()).unwrap();
        let seconds = started.elapsed().as_secs_f64();
        let bytes = std::fs::metadata(dirs.table()).unwrap().len();
        let language = std::fs::metadata(dirs.language()).unwrap().len();
        let corpus: u64 = std::fs::read_dir(dirs.corpus().join("home")).unwrap().chain(std::fs::read_dir(dirs.corpus().join("work")).unwrap()).filter_map(Result::ok).map(|e| e.metadata().unwrap().len()).sum();
        println!(
            "messages {}, vocabulary {}, table {bytes} bytes, language model {language} bytes, corpus {corpus} bytes, {seconds:.1} s, fold error {:e}, AUC {:?}, C {}",
            summary.labels.ham + summary.labels.spam,
            summary.model.vocabulary,
            summary.model.fold_error,
            summary.test.auc,
            summary.model.c
        );
        assert!(summary.model.fold_error < 1e-4);
        let _ = std::fs::remove_dir_all(root);
    }

    /// Lays out an invented profile for trying `sioul spam` by hand, in the
    /// folder `SIOUL_LEARN_DEMO` names: a configuration with two invented
    /// accounts (IMAP: never fetch with them, `--no-fetch`; no password is
    /// kept), an invented corpus, one invented spam (`spam.eml`). Point
    /// XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_STATE_HOME and XDG_CACHE_HOME at its
    /// config, data, state and cache folders.
    #[test]
    #[ignore]
    fn write_an_invented_profile() {
        let Some(root) = std::env::var_os("SIOUL_LEARN_DEMO").map(PathBuf::from) else { return };
        let config = "[[account]]\nid = \"home\"\nkind = \"imap\"\naddress = \"owner@example.org\"\nhost = \"imap.example.org\"\ntrusted_authserv_ids = [\"mx.example.org\"]\n\n[[account]]\nid = \"work\"\nkind = \"imap\"\naddress = \"owner@example.com\"\nhost = \"imap.example.com\"\n\n[spam]\nthreshold_spam = 0.9\nthreshold_unsure = 0.5\n";
        std::fs::create_dir_all(root.join("config/sioul")).unwrap();
        std::fs::write(root.join("config/sioul/config.toml"), config).unwrap();
        let dirs = Dirs { data: root.join("data/sioul/spam"), state: root.join("state/sioul/spam"), cache: root.join("cache/sioul/spam") };
        corpus::store(&dirs, &records(&synthetic::mailbox(13, 3000, 1500))).unwrap();
        std::fs::write(root.join("spam.eml"), synthetic::message(&mut crate::Rng::new(77), true, 77_777, 1_790_000_000)).unwrap();
    }

    #[test]
    fn the_table_is_replaced_only_when_no_worse() {
        assert_eq!(replacement(0, 0, 0), (true, "nothing-new"));
        assert_eq!(replacement(3, 3, 40), (true, "no-worse"));
        assert_eq!(replacement(2, 3, 40), (true, "no-worse"));
        assert_eq!(replacement(4, 3, 40), (false, "worse"));
    }

    #[test]
    fn standardization_of_columns() {
        let rows = [vec![1.0, 5.0], vec![3.0, 5.0]];
        let (mean, deviation) = standardization(|k| rows[k].clone(), |_| true, rows.len(), 2, 2);
        assert_eq!(mean, vec![2.0, 5.0]);
        assert_eq!(deviation, vec![1.0, 0.0], "a column that never changes has no deviation");
        // A header column (from 1 on) over the rows that have it: the third row's says nothing.
        let rows = [vec![1.0, 4.0], vec![3.0, 6.0], vec![5.0, 0.0]];
        let (mean, deviation) = standardization(|k| rows[k].clone(), |k| k < 2, rows.len(), 2, 1);
        assert_eq!((mean, deviation), (vec![3.0, 5.0], vec![(8.0f64 / 3.0).sqrt(), 1.0]));
        // A missing value counts in neither: the mean and deviation of those known.
        let rows = [vec![1.0, 4.0], vec![3.0, f64::NAN], vec![5.0, 6.0]];
        let (mean, deviation) = standardization(|k| rows[k].clone(), |_| true, rows.len(), 2, 1);
        assert_eq!((mean[1], deviation[1]), (5.0, 1.0));
    }
}

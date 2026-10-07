// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! fastText's classifier, the filter's model (`train::Model::Supervised`,
//! the default; docs/spam-filter.md, "The model, and how it folds"): each
//! message one line, its words, then its header facts as words
//! (`features::header_words`), two labels, softmax. Word pairs only when
//! asked (`word_ngrams` above 1), in a trial: the table reads none.
//!
//! It folds per row: with two labels, softmax's P(spam) is the sigmoid of
//! `f = (w_spam − w_ham) · h`, where `h` is the mean of every input row the
//! line reads (each known word's own, each of its character n-grams'
//! buckets, the end of the line's), so `f` is the mean of the rows' scalars
//! `(w_spam − w_ham) · row` (`row_scalars`). The table keeps each known
//! word's rows' sum, each bucket's scalar and the end of the line's
//! (`table::Kind::Rows`); it must give fastText's own prediction within
//! 1e-4 on the test messages, or nothing is exported.
//!
//! Calibrated by Platt on scores of mail the model never learned from: in
//! each account's and each outside source's mail of each label, the newest
//! fifth of the messages learned from, scored by a model of the rest (as
//! the test is split: `train::newest_fifths`), Platt fitted on your own
//! mail's part, the mail the filter will judge. In time order alone, that
//! newest fifth would be your own mail only, scored by a model of outside
//! material only, which has no header facts: its probabilities came out
//! far too sure of spam.
//!
//! After the tokens (`train::train_with`'s first steps, shared): the same
//! parts (`train::parts`); calibration, then the model of the oldest 80 %;
//! its table, checked; the numbers and the comparison with the table in
//! place, as the centroid's; kept, calibrated and learned again from every
//! message, the newest month included, folded, checked and written, the
//! model kept as the language model's file (private, never shared).

use crate::detail::{self, Detail, Tested};
use crate::eval;
use crate::labels::Label;
use crate::platt::{self, Platt};
use crate::spamcore::{self as spam, N, Table};
use crate::train::{self, Message, Options, Refit, Split, Summary};
use crate::{Cancel, Dirs, LearnError, Progress, Stage, io_error, now};
use fasttext::matrix::Matrix;
use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// fastText's label prefix here: capitals, which no word of a message has
/// (the tokenizer lowercases them), so that no sender's text reads as a label.
const LABEL: &str = "SIOUL_LABEL_";
const SPAM_LABEL: &str = "SIOUL_LABEL_SPAM";
const HAM_LABEL: &str = "SIOUL_LABEL_HAM";

/// The largest difference allowed between the table's probability and fastText's.
const FOLD_TOLERANCE: f64 = 1e-4;

/// A message as the classifier reads it: its words, then its header facts'
/// words; outside material, which has no headers, its words alone.
fn text_of(words: &[String], message: &Message) -> String {
    let mut text = words.join(" ");
    if message.known {
        for (word, _) in spam::header_words(&message.features) {
            text.push(' ');
            text.push_str(&word);
        }
    }
    text
}

/// The file a model learns from, removed whatever happens.
struct LearnFile(PathBuf);

impl Drop for LearnFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Writes the learning file of some messages, one line each, shuffled (the same seed, the same order).
fn write_learning(path: &Path, ids: &[usize], text: &dyn Fn(usize) -> String, messages: &[Message], seed: u64) -> Result<LearnFile, LearnError> {
    let file = std::fs::File::create(path).map_err(|e| io_error(path, e))?;
    let guard = LearnFile(path.to_path_buf());
    let mut out = BufWriter::new(file);
    let mut order: Vec<usize> = ids.to_vec();
    let mut rng = crate::Rng::new(seed);
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i + 1));
    }
    for &i in &order {
        let label = if messages[i].label == Label::Spam { SPAM_LABEL } else { HAM_LABEL };
        writeln!(out, "{label} {}", text(i)).map_err(|e| io_error(path, e))?;
    }
    out.flush().map_err(|e| io_error(path, e))?;
    Ok(guard)
}

/// fastText's classifier learned from the messages `ids`.
#[allow(clippy::too_many_arguments)]
fn learn(path: &Path, ids: &[usize], text: &dyn Fn(usize) -> String, messages: &[Message], options: &Options, word_ngrams: u32, lr: f64, cancel: &Cancel) -> Result<fasttext::FastText, LearnError> {
    let _file = write_learning(path, ids, text, messages, options.seed)?;
    let args = fasttext::args::Args {
        input: path.to_path_buf(),
        model: fasttext::args::ModelName::Supervised,
        loss: fasttext::args::LossName::Softmax,
        dim: options.dim as i32,
        epoch: options.epochs as i32,
        lr,
        word_ngrams: word_ngrams.max(1) as i32,
        min_count: options.min_count.max(1) as i32,
        bucket: options.bucket as i32,
        minn: options.minn as i32,
        maxn: options.maxn as i32,
        thread: options.threads.max(1) as i32,
        seed: options.seed as i32,
        label: LABEL.to_string(),
        verbose: 0,
        ..fasttext::args::Args::default()
    };
    fasttext::FastText::train_with_abort(args, cancel.flag()).map_err(|e| LearnError::Language(e.to_string()))
}

/// Each input row's scalar in a model of two labels: `(w_spam − w_ham) · row`.
fn row_scalars(model: &fasttext::FastText) -> Result<Vec<f32>, LearnError> {
    let (labels, _) = model.get_labels();
    let index = |name: &str| labels.iter().position(|l| l == name).ok_or_else(|| LearnError::Language(format!("no label {name}: both kinds are needed")));
    let (spam, ham) = (index(SPAM_LABEL)?, index(HAM_LABEL)?);
    let output = model.output_matrix();
    let direction: Vec<f32> = output.row(spam as i64).iter().zip(output.row(ham as i64)).map(|(s, h)| s - h).collect();
    let input = model.input_matrix();
    Ok((0..input.rows()).map(|r| input.row(r).iter().zip(&direction).map(|(a, b)| a * b).sum()).collect())
}

/// A text's decision value, as `predict` reads it: its rows (words, their
/// n-grams, word pairs), the end of the line's appended, their scalars' mean.
fn decision(model: &fasttext::FastText, scalars: &[f32], text: &str) -> f64 {
    let (mut rows, mut labels) = (Vec::new(), Vec::new());
    model.dict().get_line_from_str(text, &mut rows, &mut labels);
    if let Some(eos) = model.dict().get_id(fasttext::dictionary::EOS) {
        rows.push(eos);
    }
    if rows.is_empty() {
        return 0.0;
    }
    rows.iter().map(|&r| f64::from(scalars[r as usize])).sum::<f64>() / rows.len() as f64
}

/// Platt's sigmoid for a model of `ids`: in each account's and each outside
/// source's mail of each label, the newest fifth of `ids` scored by a model
/// of the rest; fitted on your own mail's part (all of it when your own
/// lacks a kind). With how many scores it was fitted on, and the AUC of all.
#[allow(clippy::too_many_arguments)]
fn calibrate(path: &Path, ids: &[usize], text: &dyn Fn(usize) -> String, messages: &[Message], sources: &[String], options: &Options, word_ngrams: u32, lr: f64, cancel: &Cancel) -> Result<(Platt, u64, Option<f64>), LearnError> {
    let group = |i: usize| (messages[i].source.map_or(messages[i].account.as_str(), |s| sources[s].as_str()), messages[i].label == Label::Spam);
    let (older, newer) = train::newest_fifths(ids, group);
    let both = |ids: &[usize]| ids.iter().any(|&i| messages[i].label == Label::Spam) && ids.iter().any(|&i| messages[i].label == Label::Ham);
    if !both(&older) || newer.is_empty() {
        return Ok((Platt { a: -1.0, b: 0.0 }, 0, None));
    }
    let model = learn(path, &older, text, messages, options, word_ngrams, lr, cancel)?;
    let scalars = row_scalars(&model)?;
    let scored: Vec<(usize, f64)> = newer.iter().map(|&i| (i, decision(&model, &scalars, &text(i)))).collect();
    let all: Vec<(f64, bool)> = scored.iter().map(|&(i, f)| (f, messages[i].label == Label::Spam)).collect();
    let own: Vec<(f64, bool)> = scored.iter().filter(|(i, _)| messages[*i].source.is_none()).map(|&(i, f)| (f, messages[i].label == Label::Spam)).collect();
    let (sigmoid, count) = match platt::fit(&own) {
        Some(sigmoid) => (sigmoid, own.len()),
        None => (platt::fit(&all).unwrap_or(Platt { a: -1.0, b: 0.0 }), all.len()),
    };
    Ok((sigmoid, count as u64, eval::auc(&all)))
}

/// The model folded into the classifier's table (`table::Kind::Rows`): each
/// known word's rows' sum (its own and its n-grams', as fastText lists them),
/// each bucket's scalar, the end of the line's as the bias.
fn fold(model: &fasttext::FastText, scalars: &[f32], sigmoid: Platt, options: &Options, split: &Split) -> Table {
    let dict = model.dict();
    let nwords = dict.nwords();
    let eos = fasttext::dictionary::EOS;
    let words: Vec<(u64, f32)> = (0..nwords)
        .filter(|&id| dict.get_word(id) != eos)
        .map(|id| (spam::word_hash(dict.get_word(id)), dict.get_subwords(id).iter().map(|&r| f64::from(scalars[r as usize])).sum::<f64>() as f32))
        .collect();
    let first = nwords as usize;
    let buckets: Vec<f32> = (0..options.bucket as usize).map(|b| scalars.get(first + b).copied().unwrap_or(0.0)).collect();
    Table {
        tokenizer: spam::TOKENIZER,
        features: spam::FEATURES,
        dim: options.dim,
        minn: options.minn,
        maxn: options.maxn,
        bucket: options.bucket,
        words,
        buckets,
        weights: Vec::new(),
        means: Vec::new(),
        bias: dict.get_id(eos).map_or(0.0, |id| scalars[id as usize]),
        text_mean: 0.0,
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

/// The largest difference between the table's probability, Platt aside, and
/// fastText's own prediction, on messages `ids` (500 at most); over 1e-4,
/// the fold is broken and nothing is exported.
fn checked_fold(table: &Table, model: &fasttext::FastText, ids: &[usize], words: &[Vec<String>], messages: &[Message]) -> Result<f64, LearnError> {
    let mut error = 0f64;
    for &i in ids.iter().take(500) {
        let features = if messages[i].known { messages[i].features } else { [f32::NAN; N] };
        let ours = 1.0 / (1.0 + (-f64::from(table.score(&words[i], &features).f)).exp());
        let theirs = model.predict(&text_of(&words[i], &messages[i]), 2, 0.0).into_iter().find(|p| p.label == SPAM_LABEL).map_or(0.0, |p| f64::from(p.prob));
        error = error.max((ours - theirs).abs());
    }
    if error > FOLD_TOLERANCE {
        return Err(LearnError::Fold(format!("the table differs from fastText's prediction by {error:e}")));
    }
    Ok(error)
}

/// What the classifier learned of each header fact: its word's rows' sum in
/// the table (toward spam above 0), by the fact's name ("dkim_signed",
/// "links_0"…); a fact never seen often enough, none.
fn header_weights(table: &Table) -> BTreeMap<String, f64> {
    spam::all_header_words()
        .into_iter()
        .filter_map(|(word, _)| {
            let at = table.words.binary_search_by_key(&spam::word_hash(&word), |(h, _)| *h).ok()?;
            Some((word.trim_start_matches("H:").to_ascii_lowercase(), f64::from(table.words[at].1)))
        })
        .collect()
}

/// The training, from the messages read (`train::train_with`, which hands
/// them here for this model): see the module.
#[allow(clippy::too_many_arguments)]
pub(crate) fn train(dirs: &Dirs, read: train::Read, options: &Options, word_ngrams: u32, lr: f64, errors: usize, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<(Summary, Detail), LearnError> {
    let train::Read { messages, words, sources, mut outside, labels, started } = read;
    let check = || if cancel.cancelled() { Err(LearnError::Cancelled) } else { Ok(()) };
    let train::Parts { order, train: train_ids, test: test_ids, held } = train::parts(&messages, sources.len());
    let text = |i: usize| text_of(&words[i], &messages[i]);
    std::fs::create_dir_all(&dirs.cache).map_err(|e| io_error(&dirs.cache, e))?;
    let path = dirs.cache.join(format!("classifier-{}.txt", std::process::id()));
    // The table reads no word pairs: with them, a trial scored by fastText itself.
    let exact = word_ngrams <= 1;

    // Calibration, then the model of the oldest 80 %.
    progress(&Progress { stage: Stage::Language, done: 0, total: 0, detail: String::new() });
    let (sigmoid, calibration_scores, calibration_auc) = calibrate(&path, &train_ids, &text, &messages, &sources, options, word_ngrams, lr, cancel)?;
    check()?;
    let model = learn(&path, &train_ids, &text, &messages, options, word_ngrams, lr, cancel)?;
    check()?;
    let scalars = row_scalars(&model)?;
    progress(&Progress { stage: Stage::Language, done: 1, total: 1, detail: String::new() });

    // Its table as every device will read it (its bytes, read back), checked; its numbers.
    progress(&Progress { stage: Stage::Evaluation, done: 0, total: test_ids.len() as u64, detail: String::new() });
    let (split, accounts) = train::counted(&messages, &train_ids, &test_ids);
    let table = Table::from_bytes(&fold(&model, &scalars, sigmoid, options, &split).to_bytes()).map_err(LearnError::Fold)?;
    let fold_error = if exact { checked_fold(&table, &model, &test_ids, &words, &messages)? } else { 0.0 };
    let lacking = [f32::NAN; N];
    let p = |i: usize| -> f64 {
        if exact {
            f64::from(table.score(&words[i], if messages[i].known { &messages[i].features } else { &lacking }).p)
        } else {
            sigmoid.probability(decision(&model, &scalars, &text(i)))
        }
    };
    let new_scored: Vec<(f64, bool)> = test_ids.iter().map(|&i| (p(i), messages[i].label == Label::Spam)).collect();
    let test = eval::evaluate(&new_scored, options.threshold_spam, options.threshold_unsure);
    let settled: Vec<(f64, bool)> = test_ids.iter().zip(&new_scored).filter(|&(&i, _)| !messages[i].unsettled).map(|(_, &s)| s).collect();
    let test_settled = Some(eval::evaluate(&settled, options.threshold_spam, options.threshold_unsure));
    let mut tested: Vec<Tested> = test_ids.iter().zip(&new_scored).map(|(&i, &(p, _))| train::tested_of(&messages[i], p, &sources)).collect();
    for (s, name) in sources.iter().enumerate() {
        let scored: Vec<(f64, bool)> = held[s].iter().map(|&i| (p(i), messages[i].label == Label::Spam)).collect();
        tested.extend(held[s].iter().zip(&scored).map(|(&i, &(p, _))| train::tested_of(&messages[i], p, &sources)));
        train::held_out(outside.entry(name.clone()).or_default(), &scored, options);
    }
    let detail = detail::of(dirs, &tested, options.threshold_spam, options.threshold_unsure, errors)?;
    drop(tested);
    progress(&Progress { stage: Stage::Evaluation, done: test_ids.len() as u64, total: test_ids.len() as u64, detail: String::new() });

    // The table in place, on the test messages it never learned from.
    let test_words: Vec<Vec<String>> = test_ids.iter().map(|&i| words[i].clone()).collect();
    let (current, compared, replaced, reason) = train::against_current(dirs, &test_ids, &test_words, &messages, &new_scored, options);
    check()?;

    // Kept: calibrated and learned again from every message, the newest month included, and written.
    // A trial stops here: nothing of the filter changes.
    let replaced = replaced && options.replace && exact;
    let mut table_bytes = table.to_bytes().len() as u64;
    let refit = if replaced {
        let step = |done: u64, progress: &mut dyn FnMut(&Progress)| progress(&Progress { stage: Stage::Export, done, total: 3, detail: String::new() });
        step(0, progress);
        let (sigmoid, calibration_scores, _) = calibrate(&path, &order, &text, &messages, &sources, options, word_ngrams, lr, cancel)?;
        check()?;
        step(1, progress);
        let model = learn(&path, &order, &text, &messages, options, word_ngrams, lr, cancel)?;
        check()?;
        step(2, progress);
        let scalars = row_scalars(&model)?;
        let ham_of = |ids: &[usize]| ids.iter().filter(|&&i| messages[i].label == Label::Ham).count() as u64;
        let all = Split { train_ham: ham_of(&order), train_spam: order.len() as u64 - ham_of(&order), ..split };
        let mut written = Table::from_bytes(&fold(&model, &scalars, sigmoid, options, &all).to_bytes()).map_err(LearnError::Fold)?;
        let refit_error = checked_fold(&written, &model, &test_ids, &words, &messages)?;
        written.meta.metrics = train::metrics(&test);
        check()?;
        // Written whole or not at all; the table it replaces is kept as table.prev.bin.
        written.write(&dirs.table()).map_err(|e| io_error(&dirs.table(), e))?;
        table_bytes = std::fs::metadata(dirs.table()).map_or(table_bytes, |m| m.len());
        let fresh = dirs.language().with_extension("bin.new");
        model.save_model(&fresh).map_err(|e| io_error(&fresh, e))?;
        std::fs::rename(&fresh, dirs.language()).map_err(|e| io_error(&dirs.language(), e))?;
        step(3, progress);
        Some(Refit { ham: all.train_ham, spam: all.train_spam, platt_a: sigmoid.a, platt_b: sigmoid.b, calibration_scores, fold_error: refit_error })
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
        labels,
        split,
        accounts,
        model: train::ModelSummary {
            kind: "classifier".to_string(),
            word_ngrams: word_ngrams.max(1),
            lr,
            dim: options.dim,
            vocabulary: u64::try_from(model.dict().nwords()).unwrap_or(0).saturating_sub(1),
            bucket: options.bucket,
            minn: options.minn,
            maxn: options.maxn,
            epochs: options.epochs,
            min_count: options.min_count.max(1),
            threads: options.threads,
            costs: Vec::new(),
            c: 0.0,
            validation_auc: calibration_auc.map(|a| ("calibration".to_string(), a)).into_iter().collect(),
            ham_weight: 0.0,
            platt_a: sigmoid.a,
            platt_b: sigmoid.b,
            calibration_scores,
            fold_error,
            table_bytes,
            header_weights: header_weights(&table),
        },
        test,
        test_settled,
        current,
        compared,
        refit,
        outside,
        trial: !options.replace,
    };
    // What the settings say of the last training: never a trial's.
    if options.replace {
        let text = toml::to_string(&summary).map_err(|e| io_error(&dirs.trained(), e))?;
        crate::corpus::write_whole(&dirs.trained(), text.as_bytes())?;
    }
    Ok((summary, detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table's word sums and buckets are fastText's own rows, and a
    /// message scores on the phone what fastText predicts: words it knows,
    /// words it never saw, header facts, outside material without them.
    #[test]
    fn the_table_is_fasttexts_prediction() {
        let root = std::env::temp_dir().join(format!("sioul-learn-classifier-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut rng = crate::Rng::new(3);
        let ham = ["meeting", "agenda", "invoice", "attached", "report", "thanks", "tomorrow", "project"];
        let spam = ["lottery", "winner", "prize", "claim", "bitcoin", "urgent", "verify", "account"];
        let line = |rng: &mut crate::Rng, pool: &[&str]| (0..12).map(|_| pool[rng.below(pool.len())].to_string()).collect::<Vec<String>>();
        let by_sender = spam::NAMES.iter().position(|n| *n == "dkim_signed_by_sender").unwrap();
        let mut messages = Vec::new();
        let mut words = Vec::new();
        for k in 0..240usize {
            let is_spam = k % 3 == 0;
            let mut features = [0.0f32; N];
            features[by_sender] = if is_spam { 0.0 } else { 1.0 };
            let known = k % 5 != 0;
            messages.push(Message {
                label: if is_spam { Label::Spam } else { Label::Ham },
                date: k as i64,
                account: "home".into(),
                place: None,
                evidence: None,
                source: (!known).then_some(0),
                ordinal: 0,
                unsettled: false,
                known,
                features,
                offset: 0,
                length: 0,
                words: 12,
            });
            words.push(line(&mut rng, if is_spam { &spam } else { &ham }));
        }
        let options = Options { dim: 8, epochs: 5, min_count: 2, bucket: 997, threads: 1, seed: 5, ..Options::default() };
        let text = |i: usize| text_of(&words[i], &messages[i]);
        let ids: Vec<usize> = (0..messages.len()).collect();
        let path = root.join("learn.txt");
        let model = learn(&path, &ids, &text, &messages, &options, 1, 0.1, &Cancel::new()).unwrap();
        assert!(!path.exists(), "the learning file is gone");
        let scalars = row_scalars(&model).unwrap();
        let table = Table::from_bytes(&fold(&model, &scalars, Platt { a: -1.0, b: 0.0 }, &options, &Split::default()).to_bytes()).unwrap();
        assert_eq!(table.kind(), sioul_core::spam::table::Kind::Rows);
        // Each known word: one row of its own and its n-grams', as the phone counts them.
        let dict = model.dict();
        for id in 0..dict.nwords() {
            let word = dict.get_word(id);
            if word != fasttext::dictionary::EOS {
                assert_eq!(dict.get_subwords(id).len(), 1 + sioul_core::spam::table::ngram_buckets(word, options.minn, options.maxn, options.bucket).len(), "{word}");
            }
        }
        // fastText's prediction, message by message, a word it never saw too.
        let mut unseen = words.clone();
        unseen[1].push("neverseenword".into());
        assert!(checked_fold(&table, &model, &ids, &unseen, &messages).unwrap() < FOLD_TOLERANCE);
        // A header fact is a word it learned: DKIM by the sender, toward ham.
        let weights = header_weights(&table);
        assert!(weights.get("dkim_signed_by_sender").is_some_and(|w| *w < 0.0), "{weights:?}");
        let _ = std::fs::remove_dir_all(root);
    }
}

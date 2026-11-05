// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul topics`: mail topics, measured on this computer (sioul-learn's
//! `topics`, docs/topics.md). `seeds` lists the topics your folders,
//! projects and contacts' categories would propose, read from the spam
//! filter's corpus; `eval` trains fastText's classifier over them and tests
//! it in time order, then says what to build with: how to say "no topic",
//! whether the sender's words stay, how many buckets, each topic's sure
//! threshold, the table's size, its churn. Names and numbers only, never a
//! message; nothing is written, moved or shared.

use crate::Session;
use crate::spam::report::{Report, day, decimal};
use crate::spam::{long, say};
use clap::Subcommand;
use serde_json::json;
use sioul_core::topics::seeds::{Kind, Topic};
use sioul_learn::model2vec::StaticModel;
use sioul_learn::topics::{self, Counts, Evaluation, Measured, Options, Scores, Seeded, Sources};
use sioul_learn::{Cancel, Dirs, LearnError, Progress, train};
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum TopicsCommand {
    /// The topics Sioul would propose from your folders, projects and
    /// contacts' categories, read from the spam filter's corpus (`sioul spam fetch`).
    Seeds {
        #[arg(long)]
        json: bool,
    },
    /// Trains fastText's classifier over the proposed topics and tests it in
    /// time order: each way to say "no topic", with and without the sender's
    /// words, at each bucket count; the way kept, folded and checked; its churn.
    Eval {
        /// fastText's threads (the computer's cores).
        #[arg(long)]
        threads: Option<u32>,
        /// The word vectors' dimension (100).
        #[arg(long)]
        dim: Option<u32>,
        /// fastText's passes over the mail (25).
        #[arg(long)]
        epochs: Option<u32>,
        /// Bucket counts, the first the table's, the others compared (50000,200000).
        #[arg(long, value_delimiter = ',')]
        buckets: Vec<u32>,
        /// Shares of the mail no seed names learned as "none", each tried (0.1,0.25,0.5).
        #[arg(long, value_delimiter = ',')]
        none_weights: Vec<f64>,
        /// Only the threshold, with the sender's words, at the first bucket count.
        #[arg(long)]
        quick: bool,
        /// Sure thresholds from confusions between topics alone: a topic on
        /// mail no seed names does not count against it.
        #[arg(long)]
        seeded_precision: bool,
        /// A Model2Vec folder (model.safetensors, tokenizer.json): its nearest centroid measured too.
        #[arg(long)]
        model2vec: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn run(s: &Session, command: TopicsCommand) -> Result<(), String> {
    let dirs = Dirs::standard();
    match command {
        TopicsCommand::Seeds { json } => long(s, json, None, |progress, cancel| seeds(s, &dirs, progress, cancel)),
        TopicsCommand::Eval { threads, dim, epochs, buckets, none_weights, quick, seeded_precision, model2vec, json } => {
            let mut options = Options::of(&s.config);
            if let Some(threads) = threads {
                options.spam.threads = threads.max(1);
            }
            options.dim = dim.unwrap_or(options.dim).max(1);
            options.epochs = epochs.unwrap_or(options.epochs).max(1);
            if !buckets.is_empty() {
                options.buckets = buckets.into_iter().map(|b| b.max(1)).collect();
            }
            if !none_weights.is_empty() {
                options.none_weights = none_weights.into_iter().map(|w| w.clamp(0.0, 1.0)).collect();
            }
            options.compare = !quick;
            options.count_unseeded = !seeded_precision;
            options.model2vec = model2vec;
            if json {
                return long(s, json, None, |progress, cancel| eval(s, &dirs, &options, progress, cancel));
            }
            say(Report { lines: vec![s.tr.text("topics-eval-start", None)], data: json!({}) }, false)?;
            long(s, json, None, |progress, cancel| eval(s, &dirs, &options, progress, cancel))
        }
    }
}

/// What went wrong, said plainly.
fn failure(s: &Session, e: &LearnError) -> String {
    match e {
        LearnError::Cancelled => s.tr.text("spam-stopped", None),
        LearnError::Disk(detail) => s.say("spam-disk", &[("detail", detail.clone())]),
        other => s.say("topics-error", &[("detail", other.to_string())]),
    }
}

fn read(s: &Session, dirs: &Dirs, options: &Options, model: Option<&mut StaticModel>, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<topics::Read, String> {
    topics::read(dirs, &train::trusted_ids(&s.config), &Sources::of(&s.config), options, model, progress, cancel).map_err(|e| failure(s, &e))
}

// Numbers in words.

fn dash() -> String {
    "—".to_string()
}

fn share(s: &Session, fraction: Option<f64>) -> String {
    fraction.map_or_else(dash, |f| s.say("spam-percent", &[("n", decimal(s, f * 100.0, 1))]))
}

fn number(s: &Session, value: Option<f64>, places: usize) -> String {
    value.map_or_else(dash, |v| decimal(s, v, places))
}

fn kilobytes(bytes: u64) -> String {
    (bytes.div_ceil(1024)).to_string()
}

// `sioul topics seeds`.

/// A seed in words: "the folder Chorale (30)".
fn seed_words(s: &Session, kind: Kind, name: &str, n: usize) -> String {
    let id = match kind {
        Kind::Folder => "topics-seed-folder",
        Kind::Category => "topics-seed-category",
        Kind::Project => "topics-seed-project",
    };
    s.say(id, &[("name", crate::one_line(name)), ("n", n.to_string())])
}

fn topic_lines(s: &Session, topic: &Topic, given_names: &dyn Fn(&str) -> String) -> Vec<String> {
    let mut lines = vec![format!(
        "  {}",
        s.say("topics-topic", &[("name", crate::one_line(&topic.name)), ("id", topic.id.clone()), ("count", topic.count.to_string()), ("senders", topic.senders.to_string())])
    )];
    let seeds: Vec<String> = topic.seeds.iter().map(|n| seed_words(s, n.kind, &n.name, n.messages)).collect();
    lines.push(format!("    {}", s.say("topics-from", &[("seeds", seeds.join(", "))])));
    if topic.kept > 0 {
        lines.push(format!("    {}", s.say("topics-kept", &[("n", topic.kept.to_string())])));
    }
    for (to, n) in &topic.given {
        lines.push(format!("    {}", s.say("topics-given", &[("n", n.to_string()), ("topic", given_names(to))])));
    }
    lines
}

fn seeded_lines(s: &Session, seeded: &Seeded, least: usize) -> Vec<String> {
    let proposal = &seeded.proposal;
    let name_of = |id: &str| proposal.topics.iter().chain(&proposal.beyond).find(|t| t.id == id).map_or_else(|| id.to_string(), |t| crate::one_line(&t.name));
    let mut lines = vec![s.say("topics-read", &[("n", seeded.messages.to_string()), ("named", seeded.named.to_string())])];
    if proposal.topics.is_empty() {
        lines.push(s.tr.text("topics-none", None));
    } else {
        lines.push(s.say("topics-proposed", &[("n", proposal.topics.len().to_string())]));
        for topic in &proposal.topics {
            lines.extend(topic_lines(s, topic, &name_of));
        }
    }
    if !proposal.beyond.is_empty() {
        lines.push(s.say("topics-beyond", &[("n", proposal.beyond.len().to_string())]));
        for topic in &proposal.beyond {
            lines.extend(topic_lines(s, topic, &name_of));
        }
    }
    if !proposal.waiting.is_empty() {
        lines.push(s.say("topics-waiting", &[("n", proposal.waiting.len().to_string())]));
        for topic in &proposal.waiting {
            lines.extend(topic_lines(s, topic, &name_of));
        }
    }
    let left = &seeded.left_out;
    lines.push(s.say("topics-left-out", &[("system", left.system.to_string()), ("small", left.small.to_string()), ("least", least.to_string()), ("unused", left.unused.to_string())]));
    lines
}

fn seeds(s: &Session, dirs: &Dirs, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Report, String> {
    let options = Options::of(&s.config);
    let read = read(s, dirs, &options, None, progress, cancel)?;
    let (seeded, _) = topics::seeded(&read, &options);
    let lines = seeded_lines(s, &seeded, options.rules.least);
    Ok(Report { lines, data: serde_json::to_value(&seeded).map_err(|e| e.to_string())? })
}

// `sioul topics eval`.

fn scores_line(s: &Session, id: &str, name: &str, scores: &Scores, measured: &Measured) -> String {
    s.say(
        id,
        &[
            ("name", name.to_string()),
            ("f1", number(s, scores.macro_f1, 3)),
            ("coverage", share(s, Some(scores.coverage))),
            ("precision", share(s, scores.precision)),
            ("recall", share(s, scores.recall)),
            ("fresh", share(s, measured.new_senders.recall)),
            ("small", number(s, measured.small, 3)),
        ],
    )
}

fn counts_words(s: &Session, counts: &Counts) -> String {
    s.say("topics-counts", &[("right", counts.right.to_string()), ("seeded", counts.seeded.to_string()), ("other", counts.other_topic.to_string()), ("unseeded", counts.no_seed.to_string()), ("maybe", counts.maybe.to_string())])
}

fn topics_lines(s: &Session, measured: &Measured, names: &dyn Fn(&str) -> String) -> Vec<String> {
    measured
        .topics
        .iter()
        .map(|t| {
            let sure = t.sure_from.map_or_else(|| s.tr.text("topics-never", None), |p| decimal(s, p, 3));
            format!(
                "  {}",
                s.say(
                    "topics-topic-numbers",
                    &[
                        ("name", names(&t.id)),
                        ("learned", t.learned.to_string()),
                        ("sure", sure),
                        ("precision", share(s, t.precision)),
                        ("seeded", share(s, t.precision_seeded)),
                        ("recall", share(s, t.recall)),
                        ("counts", counts_words(s, &t.test)),
                        ("fresh", share(s, t.new_senders.recall())),
                        ("fresh_n", t.new_senders.seeded.to_string()),
                    ]
                )
            )
        })
        .collect()
}

fn eval(s: &Session, dirs: &Dirs, options: &Options, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Report, String> {
    let mut model = match &options.model2vec {
        Some(folder) => Some(StaticModel::load(folder).map_err(|e| s.say("topics-error", &[("detail", e)]))?),
        None => None,
    };
    let read = read(s, dirs, options, model.as_mut(), progress, cancel)?;
    let evaluation = topics::evaluate(dirs, &read, options, model.as_ref(), progress, cancel).map_err(|e| failure(s, &e))?;
    let lines = eval_lines(s, &evaluation, options);
    Ok(Report { lines, data: serde_json::to_value(&evaluation).map_err(|e| e.to_string())? })
}

fn eval_lines(s: &Session, evaluation: &Evaluation, options: &Options) -> Vec<String> {
    let mut lines = seeded_lines(s, &evaluation.seeded, options.rules.least);
    let proposal = &evaluation.seeded.proposal;
    let names = |id: &str| proposal.topics.iter().find(|t| t.id == id).map_or_else(|| id.to_string(), |t| crate::one_line(&t.name));
    let Some(decided) = &evaluation.decided else {
        lines.push(s.tr.text("topics-nothing", None));
        return lines;
    };
    let split = &evaluation.split;
    lines.push(String::new());
    lines.push(s.say("topics-split", &[("learned", split.learned.to_string()), ("date", day(s, split.test_from)), ("tested", split.tested.to_string()), ("validation", split.validation.to_string())]));
    if !evaluation.none_weights.is_empty() {
        lines.push(s.tr.text("topics-none-shares", None));
        for (weight, scores) in &evaluation.none_weights {
            lines.push(format!("  {}", s.say("topics-none-share", &[("share", share(s, Some(*weight))), ("f1", number(s, scores.macro_f1, 3)), ("coverage", share(s, Some(scores.coverage)))])));
        }
    }
    lines.push(s.tr.text("topics-ways", None));
    for way in &evaluation.ways {
        lines.push(format!("  {}", scores_line(s, "topics-way", &way.name, &way.test, way)));
    }
    if let Some(centroid) = &evaluation.centroid {
        lines.push(format!("  {}", scores_line(s, "topics-way", &centroid.name, &centroid.test, centroid)));
    }
    let senders = s.tr.text(if decided.senders { "topics-senders-kept" } else { "topics-senders-left" }, None);
    lines.push(String::new());
    lines.push(s.say("topics-decided", &[("way", decided.way.clone()), ("senders", senders), ("bucket", decided.bucket.to_string()), ("t", decimal(s, decided.temperature, 3))]));
    if let Some(kept) = evaluation.ways.iter().find(|w| w.name == decided.way) {
        lines.extend(topics_lines(s, kept, &names));
        lines.push(s.say("topics-new-senders", &[("n", kept.new_senders.messages.to_string()), ("precision", share(s, kept.new_senders.precision)), ("recall", share(s, kept.new_senders.recall))]));
    }
    if let Some(fold) = &evaluation.fold {
        lines.push(s.say(
            "topics-fold",
            &[
                ("words", fold.words.to_string()),
                ("bucket", fold.bucket.to_string()),
                ("f16", kilobytes(fold.bytes_f16)),
                ("f32", kilobytes(fold.bytes_f32)),
                ("e32", format!("{:.1e}", fold.error_f32)),
                ("e16", format!("{:.1e}", fold.error_f16)),
                ("flips", fold.flips.to_string()),
                ("judged", fold.judged.to_string()),
            ],
        ));
        if fold.broken {
            lines.push(s.say("topics-fold-broken", &[("tolerance", format!("{:.0e}", topics::FOLD_TOLERANCE))]));
        }
    }
    match &evaluation.churn {
        Some(churn) => lines.push(s.say("topics-churn", &[("changed", churn.changed.to_string()), ("shown", churn.shown.to_string()), ("share", share(s, churn.share)), ("added", churn.added.to_string())])),
        None => lines.push(s.tr.text("topics-churn-none", None)),
    }
    if let (Some(centroid), Some(fold)) = (&evaluation.centroid, &evaluation.centroid_fold) {
        lines.push(String::new());
        lines.push(scores_line(s, "topics-centroid", &centroid.name, &centroid.test, centroid));
        lines.extend(topics_lines(s, centroid, &names));
        lines.push(s.say("topics-centroid-fold", &[("vocabulary", fold.vocabulary.to_string()), ("kb", kilobytes(fold.bytes_f16)), ("tokenizer", kilobytes(fold.tokenizer_bytes)), ("e16", format!("{:.1e}", fold.error_f16)), ("flips", fold.flips.to_string()), ("judged", fold.judged.to_string())]));
    }
    lines.push(s.say("topics-seconds", &[("seconds", format!("{:.0}", evaluation.seconds))]));
    lines
}

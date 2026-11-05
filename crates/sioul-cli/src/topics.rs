// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul topics`: mail topics, measured on this computer (sioul-learn's
//! `topics`, docs/topics.md), on every message kept here (Sent and the
//! archives included; junk, trash, drafts and spam out). `seeds` lists the
//! topics your folders and contacts' categories would propose, by their
//! headers alone; `eval` trains fastText's classifier over them and tests
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
use sioul_learn::groups;
use sioul_learn::model2vec::StaticModel;
use sioul_learn::topics::{self, Counts, Evaluation, Measured, Options, Scores, Seeded, Sources};
use sioul_learn::{Cancel, Dirs, LearnError, Progress, train};
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum TopicsCommand {
    /// The topics Sioul would propose from your folders, contacts'
    /// categories and the topic presets' senders (presets/topics.json), from
    /// every message kept here (Sent and archives included); each preset's
    /// matches by count.
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
    /// Groups of the mail kept here (spam out), four ways side by side, for
    /// you to name later: the proven senders' communities, Model2Vec with
    /// k-means (with --model2vec), LDA and NMF. Each group's size, senders,
    /// its senders' names, words and two subjects (masked), the name Sioul
    /// proposes and how, its coherence and the seed most of it is in.
    Groups {
        /// A Model2Vec folder (model.safetensors, tokenizer.json): its k-means too.
        #[arg(long)]
        model2vec: Option<PathBuf>,
        /// LDA's and NMF's topics (else k-means' k, else 10).
        #[arg(long)]
        k: Option<usize>,
        /// Groups smaller than this are not shown (10).
        #[arg(long)]
        least: Option<usize>,
        /// LDA's sweeps over the mail (200).
        #[arg(long)]
        iterations: Option<usize>,
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
        TopicsCommand::Groups { model2vec, k, least, iterations, json } => {
            let language = s.config.language.clone().unwrap_or_else(|| s.tr.language().to_string());
            let mut options = groups::Options { k, presets: Some(sioul_core::presets::load()), language, topic_presets: Some(sioul_core::topics::presets::Matcher::current()), ..groups::Options::default() };
            options.least = least.unwrap_or(options.least).max(1);
            options.lda_iterations = iterations.unwrap_or(options.lda_iterations).max(1);
            long(s, json, None, |progress, cancel| groups_report(s, &dirs, model2vec.as_deref(), &options, progress, cancel))
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

fn read(s: &Session, dirs: &Dirs, options: &Options, words: bool, model: Option<&mut StaticModel>, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<topics::Read, String> {
    topics::read(dirs, &train::trusted_ids(&s.config), &Sources::of(&s.config), options, words, model, progress, cancel).map_err(|e| failure(s, &e))
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
        Kind::Preset => "topics-seed-preset",
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
    let r = &seeded.reading;
    let n = |x: u64| x.to_string();
    let mut lines = vec![
        s.say("topics-reading", &[("messages", n(r.messages)), ("folders", n(r.folders)), ("files", n(r.files)), ("yours", n(r.yours))]),
        s.say("topics-reading-left", &[("folders", n(r.folders_left_out)), ("spam", n(r.spam)), ("forged", n(r.forged))]),
        s.say("topics-reading-seeds", &[("folders", n(r.folder_named)), ("folders_yours", n(r.folder_yours)), ("categories", n(r.category_named)), ("categories_yours", n(r.category_yours))]),
        s.say("topics-read", &[("named", n(seeded.named)), ("yours", n(seeded.named_yours))]),
    ];
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
    // The topic presets' matches, the most first: those of five or more would start on day one.
    let presets = sioul_core::topics::presets::Matcher::current();
    let language = s.config.language.clone().unwrap_or_else(|| s.tr.language().to_string());
    let mut matched: Vec<(&String, &sioul_core::topics::presets::Counted)> = r.presets.iter().collect();
    matched.sort_by(|a, b| b.1.total().cmp(&a.1.total()).then_with(|| a.0.cmp(b.0)));
    if matched.is_empty() {
        lines.push(s.tr.text("topics-presets-none", None));
    } else {
        lines.push(s.say("topics-presets", &[("least", sioul_core::topics::presets::DAY_ONE.to_string())]));
        for (id, counted) in matched {
            let name = presets.get(id).map_or_else(|| id.clone(), |p| sioul_core::topics::presets::name_in(p, &language));
            let mut line = s.say("topics-preset", &[("name", name), ("id", id.clone()), ("total", n(counted.total())), ("domain", n(counted.by_domain)), ("site", n(counted.by_site)), ("list", n(counted.by_list))]);
            if counted.total() >= sioul_core::topics::presets::DAY_ONE {
                line = format!("{line} {}", s.tr.text("topics-preset-starts", None));
            }
            lines.push(format!("  {line}"));
        }
    }
    lines
}

fn seeds(s: &Session, dirs: &Dirs, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Report, String> {
    let options = Options::of(&s.config);
    let read = read(s, dirs, &options, false, None, progress, cancel)?;
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
    let read = read(s, dirs, options, true, model.as_mut(), progress, cancel)?;
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

// `sioul topics groups`.

fn groups_report(s: &Session, dirs: &Dirs, folder: Option<&std::path::Path>, options: &groups::Options, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Report, String> {
    let topic_options = Options::of(&s.config);
    let mut model = match folder {
        Some(folder) => Some(StaticModel::load(folder).map_err(|e| s.say("topics-error", &[("detail", e)]))?),
        None => None,
    };
    let read = read(s, dirs, &topic_options, true, model.as_mut(), progress, cancel)?;
    let (seeded, labels) = topics::seeded(&read, &topic_options);
    let ids: Vec<String> = seeded.proposal.topics.iter().map(|t| t.id.clone()).collect();
    let options = groups::Options { started: groups::started(&s.config.topics, s.config.topics_day_one.is_some(), &read), ..options.clone() };
    let options = &options;
    let mut found = groups::groups(&read, &labels, &ids, model.as_ref(), options, progress, cancel).map_err(|e| failure(s, &e))?;
    // Subjects as an agent would see them: codes, sign-in links and account numbers hidden.
    let words = sioul_core::words::Words::of(&s.config);
    for way in &mut found.ways {
        for group in &mut way.groups {
            for subject in &mut group.subjects {
                *subject = crate::one_line(&crate::mcp::mask::message(&words, subject, "").0);
            }
        }
    }
    let name_of = |id: &str| seeded.proposal.topics.iter().find(|t| t.id == id).map_or_else(|| id.to_string(), |t| crate::one_line(&t.name));
    let n = |x: u64| x.to_string();
    let mut lines = vec![s.say("topics-groups-read", &[("messages", n(found.messages)), ("words", n(found.with_words)), ("vocabulary", n(found.vocabulary))])];
    for way in &found.ways {
        lines.push(String::new());
        if let Some(why) = &way.skipped {
            lines.push(s.say(&format!("topics-groups-skipped-{why}"), &[("way", way.name.clone())]));
            continue;
        }
        lines.push(s.say(
            "topics-groups-way",
            &[
                ("way", way.name.clone()),
                ("found", n(way.found)),
                ("shown", n(way.groups.len() as u64)),
                ("grouped", n(way.grouped)),
                ("coherence", number(s, way.coherence, 3)),
                ("agreement", share(s, way.agreement)),
            ],
        ));
        if let Some(silhouette) = way.silhouette {
            lines.push(format!("  {}", s.say("topics-groups-k", &[("k", n(way.k)), ("silhouette", decimal(s, silhouette, 3))])));
        }
        for (i, group) in way.groups.iter().enumerate() {
            let seed = match &group.seed {
                Some((id, part)) => s.say("topics-groups-seed", &[("topic", name_of(id)), ("share", share(s, Some(*part)))]),
                None => s.tr.text("topics-groups-no-seed", None),
            };
            lines.push(format!(
                "  {}",
                s.say("topics-groups-group", &[("n", n(i as u64 + 1)), ("size", n(group.size)), ("senders", n(group.senders)), ("yours", n(group.yours)), ("coherence", number(s, group.coherence, 3)), ("seed", seed)])
            ));
            lines.push(format!("    {}", s.say("topics-groups-names", &[("names", group.names.iter().map(|w| crate::one_line(w)).collect::<Vec<_>>().join(", "))])));
            lines.push(format!("    {}", proposed_line(s, group)));
            lines.push(format!("    {}", s.say("topics-groups-words", &[("words", group.words.iter().map(|w| crate::one_line(w)).collect::<Vec<_>>().join(", "))])));
            if !group.subjects.is_empty() {
                let quoted: Vec<String> = group.subjects.iter().map(|subject| s.say("topics-groups-quoted", &[("subject", subject.clone())])).collect();
                lines.push(format!("    {}", s.say("topics-groups-subjects", &[("subjects", quoted.join(" · "))])));
            }
        }
    }
    // What the window shows of them ("Groups Sioul noticed").
    let shown = groups::noticed(&found, options);
    lines.push(String::new());
    lines.push(s.say("topics-noticed", &[("shown", n(shown.groups.len() as u64)), ("weak", n(shown.weak)), ("floor", decimal(s, groups::COHERENCE_FLOOR, 1)), ("persons", n(shown.persons)), ("started", n(shown.started)), ("more", n(shown.more))]));
    for group in &shown.groups {
        let names = group.names.iter().map(|w| crate::one_line(w)).collect::<Vec<_>>().join(", ");
        lines.push(format!("  {}", s.say("topics-noticed-group", &[("size", n(group.size)), ("senders", n(group.senders)), ("coherence", number(s, group.coherence, 3)), ("names", names)])));
        lines.push(format!("    {}", proposed_line(s, group)));
    }
    if !shown.senders.is_empty() {
        lines.push(s.tr.text("topics-noticed-senders", None));
        for group in &shown.senders {
            let names = group.names.iter().map(|w| crate::one_line(w)).collect::<Vec<_>>().join(", ");
            lines.push(format!("  {}", s.say("topics-noticed-sender", &[("size", n(group.size)), ("names", names)])));
            lines.push(format!("    {}", proposed_line(s, group)));
        }
    }
    lines.push(s.say("topics-seconds", &[("seconds", format!("{:.0}", found.seconds))]));
    let mut data = serde_json::to_value(&found).map_err(|e| e.to_string())?;
    data["noticed"] = serde_json::to_value(&shown).map_err(|e| e.to_string())?;
    Ok(Report { lines, data })
}

/// A group's proposed name and how it was found, or that none is proposed ("Name it" asks).
fn proposed_line(s: &Session, group: &groups::Group) -> String {
    let how = match group.proposed_by {
        "preset" => "topics-named-preset",
        "sites" => "topics-named-sites",
        "sender" => "topics-named-sender",
        _ => return s.tr.text("topics-named-none", None),
    };
    s.say("topics-groups-proposed", &[("name", crate::one_line(&group.proposed)), ("how", s.tr.text(how, None))])
}

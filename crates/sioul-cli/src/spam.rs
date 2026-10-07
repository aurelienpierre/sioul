// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul spam`: the spam filter learned on this computer (crates/sioul-learn).
//!
//! `fetch` downloads the training corpus, `train` trains on demand (a trial
//! with `--no-replace`), `eval` tests the table in place, `status` says
//! where all stands, `dry-run` says what the filter would do now with the
//! mail in each inbox, `review` lists the review queue, `label` says a
//! message is spam or not, `job` follows what runs apart. Each says it in
//! lines, or as JSON with `--json` (`report`). A message is listed by its
//! sender's address and its subject, masked as the MCP masks them; never by
//! its text. `import` brings outside training material (JSON lines,
//! docs/spam-filter.md, "Outside material"), kept apart, never shared, and
//! takes it away. `why` names a message's words: it is for you, in your
//! terminal; the MCP server offers none of it (docs/ai.md).
//!
//! The MCP's spam tools (`mcp/spam.rs`) say the same through `report`; its
//! long ones start `sioul spam fetch|train --job <id>` apart (`jobs`).

pub(crate) mod jobs;
pub(crate) mod report;

use crate::Session;
use clap::Subcommand;
use report::Report;
use sioul_core::spam::Action;
use sioul_core::spam::labels::Label;
use sioul_learn::{Cancel, Dirs, Progress, Stage, external, train};
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum SpamCommand {
    /// Downloads the training corpus: every folder of every account (or one),
    /// what is new since the last time; nothing changes on the server.
    Fetch {
        #[arg(long)]
        account: Option<String>,
        /// What it did, as JSON.
        #[arg(long)]
        json: bool,
        /// Runs as the job of this id (`sioul spam job`): where it stands goes to its file.
        #[arg(long, hide = true)]
        job: Option<String>,
    },
    /// Brings the corpus up to date, then trains the filter; the table is
    /// replaced only if it sets aside no more ham than the one in place.
    Train {
        /// Train on the corpus as it is, without downloading first.
        #[arg(long)]
        no_fetch: bool,
        /// A trial: train and say what it would do, changing nothing (the
        /// table, the one before, the language model, the last training's summary).
        #[arg(long)]
        no_replace: bool,
        /// Also list the test's worst errors, this many of each kind, with
        /// the counts by account and folder and the numbers at other thresholds.
        #[arg(long, value_parser = clap::value_parser!(u64).range(0..=500))]
        errors: Option<u64>,
        /// Each test message's probability of spam, its label and whether
        /// that label is settled, in the JSON: numbers only, for a curve or
        /// another threshold.
        #[arg(long)]
        scores: bool,
        #[command(flatten)]
        settings: SettingsArgs,
        /// What it did, as JSON.
        #[arg(long)]
        json: bool,
        /// Runs as the job of this id (`sioul spam job`): where it stands goes to its file.
        #[arg(long, hide = true)]
        job: Option<String>,
    },
    /// Tests the table in place on the newest fifth of the corpus: its
    /// numbers, by account and folder, at other thresholds, and its worst errors.
    Eval {
        /// The worst errors listed, this many of each kind: ham called spam
        /// (the surest first), then spam missed (the least sure first).
        #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u64).range(0..=500))]
        errors: u64,
        #[arg(long)]
        json: bool,
    },
    /// What the corpus holds, the outside material, the table, the last training, the jobs.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// What your filter would do now with the mail in each inbox, as if it
    /// came now: counts per verdict and action, the messages it would move
    /// and those it would flag. Nothing is moved.
    DryRun {
        /// Only this account's inbox.
        #[arg(long)]
        account: Option<String>,
        /// Messages listed at most, in each list.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[command(flatten)]
        matrix: MatrixArgs,
        #[arg(long)]
        json: bool,
    },
    /// Each header feature's mean over the corpus, by account and label: those
    /// that tell an account apart rather than spam from ham. Numbers only.
    Features {
        /// Only the mail of this sender's domain (or one under it).
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// The review queue: what your filter flagged or moved, waiting for your word.
    Review {
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Says a message is spam or not, as the window's Spam and Not spam do,
    /// in this device's label log (every device and the next training read
    /// it). It only labels: `--move` also does the window's act on the server.
    Label {
        /// The message: its file, or its Message-ID (mid:…).
        message: String,
        /// spam, or ham (not spam).
        #[arg(value_parser = ["spam", "ham"])]
        label: String,
        /// The window's act too: the keyword on the server, and the move
        /// (spam into the Junk folder; not spam out of it, back to the inbox).
        #[arg(long = "move", overrides_with = "no_move")]
        moving: bool,
        /// Only the label (the default).
        #[arg(long)]
        no_move: bool,
        #[arg(long)]
        json: bool,
    },
    /// A job run apart (an agent's download or training): where it stands,
    /// then its result. Without an id, the jobs kept.
    Job {
        id: Option<String>,
        /// Asks it to stop at its next step.
        #[arg(long, requires = "id")]
        stop: bool,
        #[arg(long)]
        json: bool,
    },
    /// Imports outside training material: one JSON object per line
    /// (gzipped or not), with `label` ("spam" or "ham"), `date` (RFC 3339 or
    /// Unix seconds), `subject` and `text`; `from`, `headers`, `html` and
    /// `source` if known. Kept apart from your own corpus, never shared; a
    /// source imported again replaces the one before. Or, with --remove,
    /// takes a source away.
    Import {
        /// The file to import.
        #[arg(required_unless_present = "remove", conflicts_with = "remove")]
        file: Option<PathBuf>,
        /// The source's name, when its lines do not say it (else the file's name).
        #[arg(long)]
        source: Option<String>,
        /// Takes this source away: the next training learns without it.
        #[arg(long, value_name = "SOURCE")]
        remove: Option<String>,
    },
    /// Why the filter judges a message as it does: the words and header facts
    /// that weighed (for you: it shows the message's words).
    Why { file: PathBuf },
}

/// fastText's and the SVM's settings for one training, instead of the
/// defaults; recorded in what the training says (`trained.toml` when it replaces the table).
#[derive(clap::Args, Debug, Clone, Default)]
pub(crate) struct SettingsArgs {
    /// The model: fastText's classifier ("supervised", the filter's), or the
    /// language model's centroid weighed by an SVM ("centroid", the filter's
    /// before features version 3).
    #[arg(long, value_parser = ["supervised", "centroid"])]
    model: Option<String>,
    /// fastText's threads (the computer's cores).
    #[arg(long)]
    threads: Option<u32>,
    /// The word vectors' dimension (100).
    #[arg(long)]
    dim: Option<u32>,
    /// fastText's passes over the corpus (the classifier 25, the centroid's language model 5).
    #[arg(long)]
    epochs: Option<u32>,
    /// The character n-grams' hash buckets (200000).
    #[arg(long)]
    bucket: Option<u32>,
    /// The shortest character n-grams (3).
    #[arg(long)]
    minn: Option<u32>,
    /// The longest character n-grams (6; 0: none).
    #[arg(long)]
    maxn: Option<u32>,
    /// Words seen fewer times are left out of the vocabulary, and of the table (5).
    #[arg(long)]
    min_count: Option<u32>,
    /// The classifier's learning rate (0.1).
    #[arg(long)]
    lr: Option<f64>,
    /// The classifier's word pairs: words read together, up to this many (1:
    /// each alone). Above 1, a trial: the table reads no word pairs.
    #[arg(long, hide = true)]
    word_ngrams: Option<u32>,
    /// The centroid's SVM: its cost, fixed (else chosen among 0.01, 0.1, 1 and 10).
    #[arg(long)]
    c: Option<f64>,
    /// The centroid's SVM: how much more calling ham spam costs than missing spam (5).
    #[arg(long)]
    ham_weight: Option<f64>,
}

impl SettingsArgs {
    fn settings(&self) -> report::Settings {
        report::Settings {
            threads: self.threads,
            dim: self.dim,
            epochs: self.epochs,
            bucket: self.bucket,
            minn: self.minn,
            maxn: self.maxn,
            c: self.c,
            ham_weight: self.ham_weight,
            model: self.model.clone(),
            word_ngrams: self.word_ngrams,
            lr: self.lr,
            min_count: self.min_count,
        }
    }
}

/// What a dry run tries instead of your settings: an action for a verdict, a threshold.
#[derive(clap::Args, Debug, Clone, Default)]
pub(crate) struct MatrixArgs {
    /// What to do with probable spam: move, flag or nothing.
    #[arg(long, value_parser = ["move", "flag", "nothing"])]
    spam: Option<String>,
    /// What to do with maybe spam.
    #[arg(long, value_parser = ["move", "flag", "nothing"])]
    unsure: Option<String>,
    /// What to do with probable ham.
    #[arg(long, value_parser = ["move", "flag", "nothing"])]
    ham: Option<String>,
    /// Probably spam from this probability on (0 to 1).
    #[arg(long)]
    threshold_spam: Option<f32>,
    /// Maybe spam from this probability on (0 to 1), never above the other.
    #[arg(long)]
    threshold_unsure: Option<f32>,
}

pub(crate) fn run(s: &Session, command: SpamCommand) -> Result<(), String> {
    let dirs = Dirs::standard();
    match command {
        SpamCommand::Fetch { account, json, job } => long(s, json, job.as_deref(), |progress, cancel| report::fetch(s, &dirs, account.as_deref(), progress, cancel)),
        SpamCommand::Train { no_fetch, no_replace, errors, scores, settings, json, job } => {
            let ask = report::TrainAsk { fetch: !no_fetch, replace: !no_replace, errors: errors.map(|n| n as usize), scores, settings: settings.settings() };
            long(s, json, job.as_deref(), |progress, cancel| report::train(s, &dirs, &ask, progress, cancel))
        }
        SpamCommand::Eval { errors, json } => long(s, json, None, |progress, cancel| report::eval(s, &dirs, errors as usize, progress, cancel)),
        SpamCommand::Status { json } => say(report::status(s, &dirs)?, json),
        SpamCommand::DryRun { account, limit, matrix, json } => {
            let action = |text: &Option<String>| text.as_deref().and_then(Action::read);
            let ask = report::DryAsk {
                account,
                limit,
                spam: action(&matrix.spam),
                unsure: action(&matrix.unsure),
                ham: action(&matrix.ham),
                threshold_spam: matrix.threshold_spam,
                threshold_unsure: matrix.threshold_unsure,
            };
            say(report::dry_run(s, &ask)?, json)
        }
        SpamCommand::Features { domain, json } => say(report::features(s, &dirs, domain.as_deref())?, json),
        SpamCommand::Review { limit, json } => say(report::review(s, limit)?, json),
        SpamCommand::Label { message, label, moving, no_move: _, json } => {
            let label = if label == "spam" { Label::Spam } else { Label::Ham };
            say(report::label(s, &dirs, &message, label, moving)?, json)
        }
        SpamCommand::Job { id, stop, json } => say(report::job(s, id.as_deref(), stop)?, json),
        SpamCommand::Import { file, source, remove } => match (file, remove) {
            (_, Some(source)) => remove_source(s, &dirs, &source),
            (Some(file), None) => import(s, &dirs, &file, source.as_deref()),
            (None, None) => Ok(()),
        },
        SpamCommand::Why { file } => why(s, &dirs, &file),
    }
}

/// A report printed: its lines, or its data as JSON.
fn say(report: Report, json: bool) -> Result<(), String> {
    if json {
        println!("{}", serde_json::to_string_pretty(&report.data).map_err(|e| e.to_string())?);
    } else {
        for line in &report.lines {
            println!("{line}");
        }
    }
    Ok(())
}

/// A long command: its progress on standard error (one line per step or
/// folder when piped), its report at the end; or, as a job (`--job`), its
/// progress and its end written into the job's file, stopping when asked.
fn long(s: &Session, json: bool, job: Option<&str>, work: impl FnOnce(&mut dyn FnMut(&Progress), &Cancel) -> Result<Report, String>) -> Result<(), String> {
    match job {
        Some(id) => {
            let mut running = jobs::Running::open(id)?;
            let cancel = running.cancel();
            let outcome = {
                let mut see = |p: &Progress| running.see(s, p);
                work(&mut see, &cancel)
            };
            let failed = outcome.as_ref().err().cloned();
            running.finish(outcome.map(|report| (report.lines, report.data)))?;
            failed.map_or(Ok(()), Err)
        }
        None => {
            let mut shown = Shown::new();
            let outcome = work(&mut |p| shown.show(s, p), &Cancel::new());
            shown.end();
            say(outcome?, json)
        }
    }
}

/// Progress on one line that rewrites itself in a terminal; stages on lines
/// of their own. Piped (to a file, a program, an agent), one line per step
/// and per folder downloaded, at its end, never a carriage return.
struct Shown {
    stage: Option<Stage>,
    detail: String,
    terminal: bool,
}

impl Shown {
    fn new() -> Shown {
        Shown { stage: None, detail: String::new(), terminal: std::io::stderr().is_terminal() }
    }

    fn show(&mut self, s: &Session, p: &Progress) {
        let mut err = std::io::stderr();
        if self.stage != Some(p.stage) || self.detail != p.detail && p.stage == Stage::Corpus {
            if self.terminal && self.stage.is_some() {
                let _ = writeln!(err);
            }
            if self.stage != Some(p.stage) && p.stage != Stage::Corpus {
                let _ = writeln!(err, "{}", s.tr.text(&format!("spam-stage-{}", p.stage.as_str()), None));
            }
            // The download's first word: about how many messages are to come.
            if p.stage == Stage::Corpus && p.detail.is_empty() && p.total > 0 {
                let _ = writeln!(err, "{}", s.say("spam-fetch-estimate", &[("n", p.total.to_string())]));
            }
            self.stage = Some(p.stage);
            self.detail = p.detail.clone();
        }
        if self.terminal && p.total > 0 && !(p.stage == Stage::Corpus && p.detail.is_empty()) {
            let what = if p.detail.is_empty() { String::new() } else { format!("{}: ", crate::one_line(&p.detail)) };
            let _ = write!(err, "\r{what}{} / {}   ", p.done, p.total);
        } else if !self.terminal && p.stage == Stage::Corpus && !p.detail.is_empty() && p.total > 0 && p.done == p.total {
            let _ = writeln!(err, "{}: {} / {}", crate::one_line(&p.detail), p.done, p.total);
        }
    }

    fn end(&self) {
        if self.terminal && self.stage.is_some() {
            eprintln!();
        }
    }
}

/// Imports a file of outside material: counts only, never a message.
fn import(s: &Session, dirs: &Dirs, file: &std::path::Path, source: Option<&str>) -> Result<(), String> {
    let imported = external::import(dirs, file, source).map_err(|e| e.to_string())?;
    println!("{}", s.say("spam-import-lines", &[("n", imported.lines.to_string())]));
    for (source, counts) in &imported.sources {
        println!("{}", s.say("spam-import-done", &outside_pairs(s, source, counts)));
    }
    for (why, n) in &imported.refused {
        println!("{}", s.say("spam-import-refused", &[("n", n.to_string()), ("why", s.tr.text(&format!("spam-import-why-{why}"), None))]));
    }
    Ok(())
}

fn remove_source(s: &Session, dirs: &Dirs, source: &str) -> Result<(), String> {
    let name = external::source_name(source);
    if external::remove(dirs, &name).map_err(|e| e.to_string())? {
        println!("{}", s.say("spam-import-removed", &[("source", name)]));
        Ok(())
    } else {
        Err(s.say("spam-import-none", &[("source", name)]))
    }
}

/// A source's name, counts and dates, as the strings take them.
fn outside_pairs(s: &Session, source: &str, counts: &external::Counts) -> [(&'static str, String); 5] {
    [("source", crate::one_line(source)), ("ham", counts.ham.to_string()), ("spam", counts.spam.to_string()), ("first", report::day(s, counts.first)), ("last", report::day(s, counts.last))]
}

fn why(s: &Session, dirs: &Dirs, file: &std::path::Path) -> Result<(), String> {
    let raw = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    // Its account's trusted checks, as the Porch reads them; the file's time is when the server received it.
    let sources = s.config.mail_sources();
    let trusted = s.config.account_of(file).and_then(|a| sources.iter().find(|src| src.account.as_deref() == Some(a.id.as_str()))).map(|src| src.trusted_ids.clone()).unwrap_or_default();
    let received = std::fs::metadata(file).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64);
    let why = train::why(dirs, &raw, &trusted, received).map_err(|detail| s.say("spam-why-none", &[("detail", detail)]))?;
    println!("{}", s.say("spam-why-score", &[("p", report::decimal(s, why.p, 3)), ("f", report::decimal(s, why.f, 3))]));
    println!("{}", s.tr.text("spam-why-words", None));
    for (word, share) in &why.words {
        println!("  {:<24} {}", crate::one_line(word), signed(s, *share));
    }
    println!("{}", s.tr.text("spam-why-headers", None));
    for (name, share) in &why.headers {
        println!("  {name:<24} {}", signed(s, *share));
    }
    Ok(())
}

fn signed(s: &Session, value: f64) -> String {
    let text = report::decimal(s, value, 3);
    if value >= 0.0 { format!("+{text}") } else { text }
}

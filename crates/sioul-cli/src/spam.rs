// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul spam`: the spam filter learned on this computer (crates/sioul-learn).
//!
//! `fetch` downloads the training corpus, `train` trains on demand, `eval`
//! tests the table in place, `status` says where all stands: numbers only,
//! never a message. `import` brings outside training material (JSON lines,
//! docs/spam-filter.md, "Outside material"), kept apart, never shared, and
//! takes it away. `why` names a message's words: it is for you, in your
//! terminal, and the MCP server offers none of this (docs/ai.md).

use crate::Session;
use clap::Subcommand;
use sioul_learn::{Cancel, Dirs, LearnError, Progress, Stage, corpus, eval, external, train};
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum SpamCommand {
    /// Downloads the training corpus: every folder of every account (or one),
    /// what is new since the last time; nothing changes on the server.
    Fetch {
        #[arg(long)]
        account: Option<String>,
    },
    /// Brings the corpus up to date, then trains the filter; the table is
    /// replaced only if it sets aside no more ham than the one in place.
    Train {
        /// Train on the corpus as it is, without downloading first.
        #[arg(long)]
        no_fetch: bool,
    },
    /// Tests the table in place on the newest fifth of the corpus: numbers only.
    Eval,
    /// What the corpus holds, the outside material, the last training, the table.
    Status,
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

pub(crate) fn run(s: &Session, command: SpamCommand) -> Result<(), String> {
    let dirs = Dirs::standard();
    match command {
        SpamCommand::Fetch { account } => fetch(s, &dirs, account.as_deref()),
        SpamCommand::Train { no_fetch } => {
            if !no_fetch {
                fetch(s, &dirs, None)?;
            }
            learn(s, &dirs)
        }
        SpamCommand::Eval => evaluate(s, &dirs),
        SpamCommand::Status => status(s, &dirs),
        SpamCommand::Import { file, source, remove } => match (file, remove) {
            (_, Some(source)) => remove_source(s, &dirs, &source),
            (Some(file), None) => import(s, &dirs, &file, source.as_deref()),
            (None, None) => Ok(()),
        },
        SpamCommand::Why { file } => why(s, &dirs, &file),
    }
}

/// Progress on one line that rewrites itself in a terminal; stages on lines of their own.
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
            self.stage = Some(p.stage);
            self.detail = p.detail.clone();
        }
        if self.terminal && p.total > 0 {
            let what = if p.detail.is_empty() { String::new() } else { format!("{}: ", crate::one_line(&p.detail)) };
            let _ = write!(err, "\r{what}{} / {}   ", p.done, p.total);
        } else if !self.terminal && p.stage == Stage::Corpus && p.total > 0 && p.done == p.total {
            let _ = writeln!(err, "{}: {} / {}", crate::one_line(&p.detail), p.done, p.total);
        }
    }

    fn end(&self) {
        if self.terminal && self.stage.is_some() {
            eprintln!();
        }
    }
}

fn fetch(s: &Session, dirs: &Dirs, only: Option<&str>) -> Result<(), String> {
    let accounts: Vec<_> = s.config.accounts.iter().filter(|a| a.syncs() && only.is_none_or(|id| a.id == id)).cloned().collect();
    if accounts.is_empty() {
        return Err(match only {
            Some(id) => s.say("account-unknown", &[("id", id.to_string())]),
            None => s.tr.text("sync-nothing", None),
        });
    }
    let mut shown = Shown::new();
    let update = corpus::update(&accounts, &|a| sioul_sync::secret::password(a), dirs, &corpus::free_space, &mut |p| shown.show(s, p), &Cancel::new());
    shown.end();
    for (account, detail) in &update.failed {
        eprintln!("{}", s.say("spam-account-failed", &[("account", account.clone()), ("detail", crate::one_line(detail))]));
    }
    println!("{}", s.say("spam-fetch-added", &[("n", update.added.to_string())]));
    if let Some(held) = update.held {
        println!("{}", s.say("spam-held", &[("free", (held.free >> 20).to_string())]));
    }
    Ok(())
}

fn learn(s: &Session, dirs: &Dirs) -> Result<(), String> {
    let options = train::Options::of(&s.config);
    let mut shown = Shown::new();
    let result = train::train(dirs, &train::trusted_ids(&s.config), &options, &mut |p| shown.show(s, p), &Cancel::new());
    shown.end();
    let summary = result.map_err(|e| failure(s, &e, &options))?;
    print_summary(s, &summary);
    Ok(())
}

/// A training's failure, in words.
fn failure(s: &Session, e: &LearnError, options: &train::Options) -> String {
    match e {
        LearnError::Cancelled => s.tr.text("spam-stopped", None),
        LearnError::TooFew { ham, spam } => s.say("spam-too-few", &[("ham", ham.to_string()), ("spam", spam.to_string()), ("least", options.least_of_each.to_string())]),
        LearnError::Disk(detail) => s.say("spam-disk", &[("detail", detail.clone())]),
        LearnError::Fold(detail) => s.say("spam-fold", &[("detail", detail.clone())]),
        LearnError::NoTable(_) => s.tr.text("spam-no-table", None),
        other => s.say("spam-error", &[("detail", other.to_string())]),
    }
}

fn print_summary(s: &Session, summary: &train::Summary) {
    let outcome = match summary.reason.as_str() {
        "no-table" => s.tr.text("spam-replaced-no-table", None),
        "unreadable" => s.tr.text("spam-replaced-unreadable", None),
        "nothing-new" => s.tr.text("spam-replaced-nothing-new", None),
        reason => {
            let c = summary.compared.unwrap_or_default();
            let pairs = [("n", (c.ham + c.spam).to_string()), ("new", c.new_ham_lost.to_string()), ("old", c.current_ham_lost.to_string())];
            s.say(if reason == "worse" { "spam-kept-worse" } else { "spam-replaced-no-worse" }, &pairs)
        }
    };
    println!("{outcome}");
    if let Some(refit) = &summary.refit {
        println!("{}", s.say("spam-refit", &[("n", (refit.ham + refit.spam).to_string())]));
    }
    print_labels(s, &summary.labels);
    for (account, c) in &summary.accounts {
        println!(
            "{}",
            s.say(
                "spam-account-counts",
                &[
                    ("account", account.clone()),
                    ("trainham", c.train_ham.to_string()),
                    ("trainspam", c.train_spam.to_string()),
                    ("testham", c.test_ham.to_string()),
                    ("testspam", c.test_spam.to_string()),
                ]
            )
        );
    }
    print_numbers(s, &summary.test, summary.split.test_from);
    for (source, outside) in &summary.outside {
        let pairs = [
            ("source", crate::one_line(source)),
            ("trainham", outside.train_ham.to_string()),
            ("trainspam", outside.train_spam.to_string()),
            ("heldham", outside.held_ham.to_string()),
            ("heldspam", outside.held_spam.to_string()),
        ];
        println!("{}", s.say("spam-outside-learned", &pairs));
        if let Some(baseline) = &outside.baseline {
            print_measured(s, baseline);
        }
    }
    if let Some(current) = summary.current.as_ref().filter(|c| c.ham + c.spam > 0) {
        println!("{}", s.say("spam-current", &[("n", (current.ham + current.spam).to_string())]));
        print_numbers(s, current, summary.compared.map_or(summary.split.test_from, |c| c.since));
    }
    let m = &summary.model;
    println!(
        "{}",
        s.say(
            "spam-model",
            &[("vocabulary", m.vocabulary.to_string()), ("dim", m.dim.to_string()), ("c", decimal(s, m.c, 2)), ("seconds", decimal(s, summary.seconds, 0)), ("bytes", (m.table_bytes >> 10).to_string())]
        )
    );
}

fn print_labels(s: &Session, l: &sioul_learn::labels::Summary) {
    println!(
        "{}",
        s.say(
            "spam-labels",
            &[
                ("ham", l.ham.to_string()),
                ("spam", l.spam.to_string()),
                ("folder", l.by_folder.to_string()),
                ("junk", l.by_junk_folder.to_string()),
                ("keyword", l.by_keyword.to_string()),
                ("log", l.by_log.to_string()),
                ("ambiguous", l.ambiguous.to_string()),
            ]
        )
    );
    if l.moved > 0 {
        println!("{}", s.say("spam-labels-moved", &[("n", l.moved.to_string())]));
    }
}

fn print_numbers(s: &Session, n: &eval::Numbers, since: i64) {
    println!("{}", s.say("spam-tested", &[("ham", n.ham.to_string()), ("spam", n.spam.to_string()), ("since", day(s, since))]));
    print_measured(s, n);
}

/// The numbers at both thresholds, the share unsure, the AUC.
fn print_measured(s: &Session, n: &eval::Numbers) {
    for (at, what) in [(&n.at_spam, "spam-what-set-aside"), (&n.at_unsure, "spam-what-unsure")] {
        println!(
            "{}",
            s.say(
                "spam-at-threshold",
                &[
                    ("threshold", decimal(s, at.threshold, 2)),
                    ("what", s.tr.text(what, None)),
                    ("ham", percent(s, at.ham_called_spam.fraction())),
                    ("hamlow", percent(s, at.ham_called_spam.low)),
                    ("hamhigh", percent(s, at.ham_called_spam.high)),
                    ("spam", percent(s, at.spam_caught.fraction())),
                    ("spamlow", percent(s, at.spam_caught.low)),
                    ("spamhigh", percent(s, at.spam_caught.high)),
                ]
            )
        );
    }
    println!("{}", s.tr.text("spam-intervals", None));
    println!("{}", s.say("spam-unsure-share", &[("share", percent(s, n.unsure.fraction()))]));
    if let Some(auc) = n.auc {
        println!("{}", s.say("spam-auc", &[("auc", decimal(s, auc, 4))]));
    }
}

fn evaluate(s: &Session, dirs: &Dirs) -> Result<(), String> {
    let options = train::Options::of(&s.config);
    let mut shown = Shown::new();
    let result = train::evaluate(dirs, &train::trusted_ids(&s.config), &options, &mut |p| shown.show(s, p), &Cancel::new());
    shown.end();
    let evaluation = result.map_err(|e| failure(s, &e, &options))?;
    println!("{}", s.say("spam-table", &[("date", day(s, evaluation.table.trained_at)), ("ham", evaluation.table.ham.to_string()), ("spam", evaluation.table.spam.to_string())]));
    print_labels(s, &evaluation.labels);
    print_numbers(s, &evaluation.numbers, evaluation.split.test_from);
    for (source, numbers) in &evaluation.outside {
        println!("{}", s.say("spam-outside-baseline", &[("source", crate::one_line(source)), ("ham", numbers.ham.to_string()), ("spam", numbers.spam.to_string())]));
        print_measured(s, numbers);
    }
    Ok(())
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
    [("source", crate::one_line(source)), ("ham", counts.ham.to_string()), ("spam", counts.spam.to_string()), ("first", day(s, counts.first)), ("last", day(s, counts.last))]
}

fn status(s: &Session, dirs: &Dirs) -> Result<(), String> {
    let corpus = corpus::status(dirs);
    if corpus.accounts.is_empty() {
        println!("{}", s.tr.text("spam-corpus-empty", None));
    }
    for account in &corpus.accounts {
        let records: u64 = account.folders.iter().map(|f| f.records).sum();
        let size = decimal(s, account.bytes as f64 / f64::from(1u32 << 20), 1);
        println!("{}", s.say("spam-corpus-account", &[("account", account.id.clone()), ("records", records.to_string()), ("size", size), ("folders", account.folders.len().to_string())]));
        for folder in &account.folders {
            let name = crate::one_line(&sioul_core::folders::decode_utf7(&folder.name));
            println!("  {}", s.say("spam-corpus-folder", &[("folder", name), ("server", folder.on_server.to_string()), ("records", folder.records.to_string())]));
        }
    }
    if let Some(at) = corpus.last_run {
        println!("{}", s.say("spam-corpus-last", &[("date", day(s, at))]));
    }
    if let Some(held) = corpus.held {
        println!("{}", s.say("spam-held", &[("free", (held.free >> 20).to_string())]));
    }
    for (account, detail) in &corpus.errors {
        println!("{}", s.say("spam-account-failed", &[("account", account.clone()), ("detail", crate::one_line(detail))]));
    }
    // Outside material: what each source holds, kept apart, never shared.
    for source in external::sources(dirs) {
        let mut pairs = outside_pairs(s, &source.name, &source.counts).to_vec();
        pairs.push(("size", decimal(s, source.bytes as f64 / f64::from(1u32 << 20), 1)));
        println!("{}", s.say("spam-outside", &pairs));
    }
    // Whether your providers' own spam flags still count, as Sioul reads them now: totals only.
    for (account, v) in corpus::verdicts(&dirs).map_err(|e| e.to_string())? {
        println!(
            "{}",
            s.say("spam-verdicts", &[("account", account), ("records", v.records.to_string()), ("with", v.with_header.to_string()), ("read", v.read.to_string()), ("flagged", v.flagged.to_string())])
        );
    }
    match sioul_learn::train::last(dirs) {
        Some(summary) => {
            println!();
            println!("{}", s.say("spam-last-training", &[("date", day(s, summary.trained_at))]));
            print_summary(s, &summary);
        }
        None => println!("{}", s.tr.text("spam-never-trained", None)),
    }
    Ok(())
}

fn why(s: &Session, dirs: &Dirs, file: &std::path::Path) -> Result<(), String> {
    let raw = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    // Its account's trusted checks, as the Porch reads them; the file's time is when the server received it.
    let sources = s.config.mail_sources();
    let trusted = s.config.account_of(file).and_then(|a| sources.iter().find(|src| src.account.as_deref() == Some(a.id.as_str()))).map(|src| src.trusted_ids.clone()).unwrap_or_default();
    let received = std::fs::metadata(file).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64);
    let why = train::why(dirs, &raw, &trusted, received).map_err(|detail| s.say("spam-why-none", &[("detail", detail)]))?;
    println!("{}", s.say("spam-why-score", &[("p", decimal(s, why.p, 3)), ("f", decimal(s, why.f, 3))]));
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

/// A number with this language's decimal separator.
fn decimal(s: &Session, value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', &s.tr.text("decimal-separator", None))
}

fn signed(s: &Session, value: f64) -> String {
    let text = decimal(s, value, 3);
    if value >= 0.0 { format!("+{text}") } else { text }
}

/// A fraction as a percentage: "0.4 %" in this language's way.
fn percent(s: &Session, fraction: f64) -> String {
    s.say("spam-percent", &[("n", decimal(s, fraction * 100.0, 1))])
}

/// A day, in this language's way, with its year when not this year's; a dash for an unknown date.
fn day(s: &Session, seconds: i64) -> String {
    let today = jiff::Zoned::now().date();
    jiff::Timestamp::from_second(seconds).ok().filter(|_| seconds > 0).map(|t| s.tr.day_in(t.to_zoned(jiff::tz::TimeZone::system()).date(), today)).unwrap_or_else(|| "—".to_string())
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's spam filter, learned on the desktop from your own mail.
//!
//! Nothing here runs on the phone: sioul-cli and sioul-app depend on this crate
//! on desktops only. Nothing here shows a message to an AI agent: the numbers
//! it gives are aggregates; `sioul spam why` is for you, in your terminal.
//!
//! What every device runs (the tokenizer, the header features, the reduced
//! table that scores both, the label log) is sioul-core's
//! [`sioul_core::spam`]. This crate trains on your mail, then writes that
//! table. It reads mail through [sioul-sync](../sioul_sync/index.html); the
//! command line (`sioul spam`) and the window ("Train now") run it. The
//! design, its numbers and its reasons: docs/spam-filter.md, also on the
//! website: <https://aurelienpierre.github.io/sioul/dev/spam-filter.html>.
//!
//! Long jobs report where they stand through a callback ([`Progress`]) and
//! stop when asked ([`Cancel`]), so the window can run them on a thread.
//!
//! # Where to start reading
//!
//! - [`train::run`]: what "Train now" and `sioul spam train` do: the corpus
//!   brought up to date ([`corpus::update`]), then [`train::train`], step by
//!   step.
//! - [`labels::decide`]: the one label each message gets, spam or ham.
//! - [`Dirs`]: where the filter keeps its files.
//!
//! # Modules
//!
//! - [`consent`]: your consent, address by address, to learning from a Microsoft address's mail: the addresses named at the last press of Train now; never a work or school one's.
//! - [`corpus`]: what training reads, downloaded from every folder of every account and kept here, junk included after your provider purges it, with Sioul's own checks of each message (its signatures, its sender) made on the whole message.
//! - [`external`]: outside training material, mail labelled elsewhere, imported once (`sioul spam import`): more words to learn, and a baseline.
//! - [`labels`]: spam or ham for each message, from its folder, its keywords, your own filter's word and your own actions (every device's logs).
//! - [`auto`]: training again by itself, once a week, on the computer that made the table in use, when it is plugged in and idle: the rule, and what it knows.
//! - [`train`]: training on demand: fastText on your mail, the message vectors, the SVM, its calibration, the evaluation, and the reduced table every device reads, replaced only when no worse.
//! - [`svm`]: the linear SVM, solved as liblinear solves it.
//! - [`platt`]: the SVM's scores turned into probabilities (Platt scaling).
//! - [`eval`]: the filter's numbers, aggregates only, each with its interval.
//! - [`detail`]: what a test said message by message: counts by account and folder, a grid of thresholds, the worst errors (`sioul spam eval --errors`).
//! - [`diagnose`]: each header feature's mean by account and label (`sioul spam features`): which ones tell accounts apart rather than spam from ham.
//! - [`spamcore`](spamcore/index.html): the seam to sioul-core's spam module: a corpus record read as the Porch reads the message.
//! - `synthetic`, for tests only, this crate's and the command line's (so not shown here): invented mail on reserved domains.

pub mod auto;
pub mod consent;
pub mod corpus;
pub mod detail;
pub mod diagnose;
pub mod eval;
pub mod external;
pub mod labels;
pub mod platt;
pub(crate) mod spamcore;
pub(crate) mod supervised;
pub mod svm;
/// Invented mail on reserved domains, for the tests of this crate and of the command line.
#[doc(hidden)]
pub mod synthetic;
pub mod train;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Where the filter keeps its files. `standard()` is Sioul's own folders;
/// tests give their own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    /// `$XDG_DATA_HOME/sioul/spam`: the corpus, outside material, the language model, the table.
    pub data: PathBuf,
    /// `$XDG_STATE_HOME/sioul/spam`: every device's label log and log of
    /// what the filter moved, the last training's summary.
    pub state: PathBuf,
    /// `$XDG_CACHE_HOME/sioul/spam`: the tokenized corpus while fastText reads it.
    pub cache: PathBuf,
}

impl Dirs {
    pub fn standard() -> Dirs {
        use sioul_core::config;
        Dirs { data: config::data_dir().join("spam"), state: config::state_dir().join("spam"), cache: config::cache_dir().join("spam") }
    }

    /// Every folder under one root (tests).
    pub fn under(root: &std::path::Path) -> Dirs {
        Dirs { data: root.join("data"), state: root.join("state"), cache: root.join("cache") }
    }

    /// The training corpus: private, never shared, never shown to an AI agent.
    pub fn corpus(&self) -> PathBuf {
        self.data.join("corpus")
    }

    /// The addresses named under Train now at its last press (`consent`): this computer's only, never shared.
    pub fn consent(&self) -> PathBuf {
        self.data.join("consent.toml")
    }

    /// Outside training material (`external`): private, never shared, never shown to an AI agent.
    pub fn external(&self) -> PathBuf {
        self.data.join("external")
    }

    /// The fastText model: private, never shared (it holds the vocabulary in plain text).
    pub fn language(&self) -> PathBuf {
        self.data.join("language.bin")
    }

    /// The reduced table every device reads.
    pub fn table(&self) -> PathBuf {
        self.data.join("table.bin")
    }

    /// The table before the last one replaced it.
    pub fn previous_table(&self) -> PathBuf {
        self.data.join("table.prev.bin")
    }

    /// The label logs, one per device: your junk, not-junk, "Spam", "Not spam" and blocks.
    pub fn labels(&self) -> PathBuf {
        self.state.join(sioul_core::spam::labels::FOLDER)
    }

    /// What your own filter moved into a Junk folder, one log per device:
    /// learned as all Junk mail is, until you say otherwise.
    pub fn moved(&self) -> PathBuf {
        self.state.join(sioul_core::spam::labels::MOVED)
    }

    /// What your own filter flagged where it is, one log per device: a
    /// probable spam learned as spam, a maybe spam left out, until you say.
    pub fn flagged(&self) -> PathBuf {
        self.state.join(sioul_core::spam::labels::FLAGGED)
    }

    /// The last training's summary, for the settings.
    pub fn trained(&self) -> PathBuf {
        self.state.join("trained.toml")
    }

    /// Forgets what the corpus keeps of one account (the start of each of its
    /// messages), when the account is removed with its mail (Microsoft's
    /// accounts, docs/microsoft.md): once no download of it runs, in any
    /// process (the window's, the command line's, an AI agent's job), whose
    /// last batch would make the folder again; one under way is waited for,
    /// at most `FORGET_WAITS`. True when there was something; an id that is
    /// not one plain name names nothing.
    pub fn forget_account(&self, id: &str) -> std::io::Result<bool> {
        self.forget_account_within(id, FORGET_WAITS)
    }

    fn forget_account_within(&self, id: &str, within: std::time::Duration) -> std::io::Result<bool> {
        let plain = matches!(std::path::Path::new(id).components().collect::<Vec<_>>().as_slice(), [std::path::Component::Normal(_)]);
        let folder = self.corpus().join(id);
        if !plain || !folder.is_dir() {
            return Ok(false);
        }
        unlocked(&folder.join("lock"), within);
        std::fs::remove_dir_all(&folder)?;
        Ok(true)
    }
}

/// How long forgetting an account's corpus waits for a download of it under way.
const FORGET_WAITS: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Waits until nobody holds the lock at `path` (an account's download holds
/// it: `corpus.rs`), at most `within`; then lets it go at once, so that its
/// folder can be deleted (Windows deletes no open file). No file: none runs.
fn unlocked(path: &std::path::Path, within: std::time::Duration) {
    let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) else { return };
    let until = std::time::Instant::now() + within;
    while file.try_lock().is_err() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// The steps of a long job, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Downloading the corpus: `detail` is "account · folder".
    Corpus,
    /// Sioul's own checks of the messages kept without them, each fetched
    /// whole once (`corpus::verify`): `detail` is the account.
    Verify,
    /// Reading the corpus and deciding each message's label.
    Labels,
    /// Turning each message into words.
    Tokens,
    /// fastText learning your mail's words (no count: it says nothing until done).
    Language,
    /// Each message's vector and header features.
    Features,
    /// The SVM, one cost after another, then the calibration's folds.
    Classifier,
    /// The numbers on the newest fifth.
    Evaluation,
    /// The table written, or kept.
    Export,
}

impl Stage {
    /// A short name for logs and the command line ("corpus", "language").
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Corpus => "corpus",
            Stage::Verify => "verify",
            Stage::Labels => "labels",
            Stage::Tokens => "tokens",
            Stage::Language => "language",
            Stage::Features => "features",
            Stage::Classifier => "classifier",
            Stage::Evaluation => "evaluation",
            Stage::Export => "export",
        }
    }
}

/// Where a long job stands: `done` of `total` in this stage (`total` 0 when
/// unknown), and what it is on (an account and a folder; never a message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub stage: Stage,
    pub done: u64,
    pub total: u64,
    pub detail: String,
}

/// Asks a long job to stop at its next step; shared between threads.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// The flag itself, for fastText's own training loop.
    pub(crate) fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
    }
}

/// What went wrong, said plainly; never a message's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearnError {
    /// Stopped when asked.
    Cancelled,
    /// The disk would fall under the space kept free.
    Disk(String),
    /// Too few messages of one kind to learn from.
    TooFew { ham: u64, spam: u64 },
    /// A file could not be read or written.
    Io(String),
    /// The language model could not be trained.
    Language(String),
    /// The folded table does not give the model's scores: nothing is exported.
    Fold(String),
    /// No table to evaluate, or one this Sioul cannot read (why).
    NoTable(String),
}

impl std::fmt::Display for LearnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LearnError::Cancelled => write!(f, "stopped"),
            LearnError::Disk(d) => write!(f, "disk: {d}"),
            LearnError::TooFew { ham, spam } => write!(f, "too few messages: {ham} ham, {spam} spam"),
            LearnError::Io(d) => write!(f, "{d}"),
            LearnError::Language(d) => write!(f, "fastText: {d}"),
            LearnError::Fold(d) => write!(f, "fold: {d}"),
            LearnError::NoTable(d) => write!(f, "no table: {d}"),
        }
    }
}

impl std::error::Error for LearnError {}

pub(crate) fn io_error(path: &std::path::Path, e: impl std::fmt::Display) -> LearnError {
    LearnError::Io(format!("{}: {e}", path.display()))
}

/// A small, seeded random number generator (splitmix64): the same seed gives
/// the same visit order, so the same model.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in 0..n (n > 0).
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// A number in [0, 1).
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Now, in Unix seconds.
pub(crate) fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

#[cfg(test)]
mod forget_tests {
    use super::*;

    /// One account's corpus forgotten, the others' kept; an odd id names nothing.
    #[test]
    fn an_accounts_corpus_is_forgotten_alone() {
        let root = std::env::temp_dir().join(format!("sioul-learn-forget-{}", std::process::id()));
        let dirs = Dirs::under(&root);
        for account in ["work", "home"] {
            std::fs::create_dir_all(dirs.corpus().join(account)).unwrap();
            std::fs::write(dirs.corpus().join(account).join("state.toml"), "x").unwrap();
        }
        assert!(dirs.forget_account("work").unwrap());
        assert!(!dirs.corpus().join("work").exists() && dirs.corpus().join("home").is_dir());
        assert!(!dirs.forget_account("work").unwrap(), "nothing left");
        assert!(!dirs.forget_account("../corpus").unwrap() && !dirs.forget_account("").unwrap());
        assert!(dirs.corpus().join("home").is_dir());
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The review of 15 October 2026, L5: a download of the account under
    /// way (another process's job holds its lock) ends before its corpus is
    /// forgotten: its last batch never makes the folder again.
    #[test]
    fn a_download_under_way_ends_before_its_corpus_goes() {
        let root = std::env::temp_dir().join(format!("sioul-learn-forget-locked-{}", std::process::id()));
        let dirs = Dirs::under(&root);
        let folder = dirs.corpus().join("live");
        std::fs::create_dir_all(&folder).unwrap();
        let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(folder.join("lock")).unwrap();
        lock.lock().unwrap();
        let (held, wait) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| {
                held.send(()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(300));
                // Its last batch, then the lock let go.
                std::fs::write(folder.join("INBOX.jsonl.gz"), b"batch").unwrap();
                drop(lock);
            });
            wait.recv().unwrap();
            let started = std::time::Instant::now();
            assert!(dirs.forget_account("live").unwrap());
            assert!(started.elapsed() >= std::time::Duration::from_millis(250), "waited for the download");
        });
        assert!(!folder.exists(), "forgotten after its last batch, not before");
        // Held past the wait: forgotten all the same.
        std::fs::create_dir_all(&folder).unwrap();
        let stuck = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(folder.join("lock")).unwrap();
        stuck.lock().unwrap();
        #[cfg(unix)]
        assert!(dirs.forget_account_within("live", std::time::Duration::from_millis(100)).unwrap() && !folder.exists());
        drop(stuck);
        let _ = std::fs::remove_dir_all(&root);
    }
}

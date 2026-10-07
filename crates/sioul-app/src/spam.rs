// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's own spam filter in the window (docs/spam-filter.md): what the mail
//! settings say of it, under "Your own spam filter" (`SpamFilter.qml`), and on
//! a computer its training on demand, "Train now".
//!
//! Training brings the corpus up to date from every address, then learns
//! (crates/sioul-learn, `train::run`), on a thread of its own at a lower
//! priority, so that the computer stays usable while fastText takes every
//! core. Its progress is said as it goes (`spam_changed`), at most a few times
//! a second; "Stop" stops it at its next step, what was downloaded kept. A
//! phone never trains: it says where its table comes from (another device's,
//! brought sealed by the sharing: docs/database.md) and what it measured.
//!
//! Numbers only, never a message: the window shows what `sioul spam status`
//! prints, in your language.
//!
//! One computer trains, by hand: two computers training tables of their own
//! is not supported (the later one's table would win everywhere; the other,
//! kept by the sharing as a conflict copy beside it, is read by nothing).
//!
//! After each fetch, on every device (the window's watchers, a phone's
//! background step), what the filter judges as you chose "Move to spam" for
//! goes into the account's Junk folder on the server, and what it judges as
//! you chose "Flag only" for is written in this device's log of its flags,
//! which the training reads (`after_fetch`).

use crate::backend::{QtThread, Shared, say, tr};
use serde::Serialize;
use sioul_core::config::Account;
use sioul_core::porch::{self, Lane, Reason};
use sioul_core::spam::table::{self, Table};
use sioul_core::spam::{Action, Class};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Mail just fetched into an account's inbox (`new`; nothing on its first
/// fetch, which brings two weeks of it, nor what was there before), judged as
/// the Porch judges it, protections first: what your own filter judges, of a
/// class you chose "Move to spam" for, goes into the account's Junk folder on
/// the server, as the Junk button does but without a keyword nor a label; of
/// a class you chose "Flag only" for, it stays where it is. Each move and
/// each flag goes into this device's logs of what the filter did: the Porch
/// lists them among its catches ("Not spam" brings one back), and the
/// training learns from them until you say otherwise. It reaches the server:
/// never on the window's thread, nor inside a watcher's own. How many moved.
pub(crate) fn after_fetch(account: &Account, new: &[PathBuf], first: bool) -> usize {
    if first || new.is_empty() {
        return 0;
    }
    let config = crate::backend::load_config();
    let filter = sioul_core::spam::Filter::of(&config);
    // Nothing judged without a table, nor when nothing is done with any class.
    if filter.actions.idle() || !filter.table.exists() {
        return 0;
    }
    let ties = sioul_core::links::LocalLinks::load(&sioul_core::links::LocalLinks::default_path());
    let store = config.case_store_path().and_then(|root| sioul_core::cases::CaseStore::load(&root).ok()).map(|s| s.with_ties(&ties));
    let known = porch::KnownSenders::load(&config.known_senders_path());
    let senders = porch::Senders::load(&config);
    let judged = porch::judge(new, &config.mail_sources(), store.as_ref(), &known, &senders, jiff::Timestamp::now().as_second());
    // Flagged where it is: written here only, nothing changes on the server.
    for (file, class) in chosen(&judged, &filter, Action::Flag) {
        if let Err(e) = sioul_sync::mailbox::flagged(account, &file, class) {
            eprintln!("sioul: spam: {}: {e:?}", account.id);
        }
    }
    let to_move = chosen(&judged, &filter, Action::Move);
    if to_move.is_empty() {
        return 0;
    }
    let Ok(password) = sioul_sync::secret::password(account) else { return 0 };
    to_move
        .into_iter()
        .filter(|(file, class)| match sioul_sync::mailbox::act(account, &password, file, &sioul_sync::mailbox::Action::Filtered(*class)) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("sioul: spam: {}: {e:?}", account.id);
                false
            }
        })
        .count()
}

/// Among messages judged, those whose class you chose `action` for (move
/// them into a Junk folder, or flag them where they are): among the filter's
/// catches by its verdict, and not moved yet.
fn chosen(judged: &[porch::Triaged], filter: &sioul_core::spam::Filter, action: Action) -> Vec<(PathBuf, Class)> {
    judged
        .iter()
        .filter(|t| t.lane == Lane::Review && !t.reasons.contains(&Reason::MovedToJunk))
        .filter_map(|t| {
            let (class, _) = t.reasons.iter().find_map(Reason::learned)?;
            (filter.actions.of(class) == action).then_some((t.card.path.clone()?, class))
        })
        .collect()
}

/// What the settings show of the filter, every line said already (`SpamFilter.qml`).
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Status {
    /// This device trains (a computer); a phone only reads its table.
    trains: bool,
    /// The table in use: where it comes from, when, from what; else that there is none.
    table: String,
    /// What that table measured on the newest of your mail, at the spam threshold then; "" without numbers.
    measured: String,
    /// The newest table, which this Sioul cannot use, and why; "" when none was refused.
    refused: String,
    /// A training running.
    running: bool,
    /// Asked to stop: it stops at its next step.
    stopping: bool,
    /// What it does now, said; "" when none runs.
    progress: String,
    /// How far in that step, from 0 to 1; below 0 when it cannot be told (fastText says nothing until done).
    fraction: f64,
    /// How the last run here ended when it made no table: stopped, too few messages, the disk.
    ended: String,
    /// "Train again by itself" (`[spam] train_by_itself`, on unless said).
    by_itself: bool,
    /// This computer is the one that does it: the switch can be changed here (else greyed).
    by_itself_here: bool,
    /// Under the switch: which computer trains by itself, or what its last run here did; "" for nothing yet.
    by_itself_line: String,
    /// The last training here (`trained.toml`): what it did, then its numbers.
    last: Vec<String>,
    /// The corpus: what it holds, the last download, the room left, addresses it could not read.
    corpus: Vec<String>,
    /// Outside training material: what each source holds (`sioul spam import`).
    outside: Vec<String>,
}

/// The filter's state for the settings, as JSON.
pub(crate) fn status() -> String {
    serde_json::to_string(&status_now()).unwrap_or_default()
}

fn status_now() -> Status {
    let mut status = Status { trains: cfg!(not(target_os = "android")), fraction: -1.0, ..Status::default() };
    let (table, measured, refused) = table_lines(&sioul_core::spam::table_path(), status.trains);
    (status.table, status.measured, status.refused) = (table, measured, refused);
    #[cfg(not(target_os = "android"))]
    training::fill(&mut status);
    status
}

/// "Train now": the corpus brought up to date, then a training, on a thread of
/// its own; `spam_changed` says how it goes. Why it did not start, else "".
pub(crate) fn train(qt: QtThread, shared: Arc<Shared>) -> String {
    #[cfg(not(target_os = "android"))]
    {
        training::start(qt, shared)
    }
    #[cfg(target_os = "android")]
    {
        drop((qt, shared));
        tr().text("spam-app-phone-trains-not", None)
    }
}

/// The training running stops at its next step.
pub(crate) fn stop(qt: QtThread) {
    #[cfg(not(target_os = "android"))]
    training::stop(qt);
    #[cfg(target_os = "android")]
    drop(qt);
}

/// The window's minute, on a computer: training again by itself when
/// everything allows it (`sioul_learn::auto`), as a job apart (`sioul spam
/// train --by-itself`); the settings said again while one runs or just
/// ended. Never a notification. Off the window's thread: it may ask the system.
pub(crate) fn by_itself(qt: QtThread) {
    #[cfg(not(target_os = "android"))]
    training::by_itself(qt);
    #[cfg(target_os = "android")]
    drop(qt);
}

/// The table's lines, kept while its file and the one before stay as they
/// are: the settings ask often while a training runs, and it is two megabytes.
fn table_lines(path: &Path, trains: bool) -> (String, String, String) {
    type Said = Option<(Vec<Option<(u64, std::time::SystemTime)>>, (String, String, String))>;
    static SAID: Mutex<Said> = Mutex::new(None);
    let seal = |path: &Path| std::fs::metadata(path).ok().and_then(|m| Some((m.len(), m.modified().ok()?)));
    let sealed = vec![seal(path), seal(&table::previous(path))];
    let mut said = SAID.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((known, lines)) = said.as_ref()
        && *known == sealed
    {
        return lines.clone();
    }
    let lines = read_table_lines(path, trains);
    *said = Some((sealed, lines.clone()));
    lines
}

fn read_table_lines(path: &Path, trains: bool) -> (String, String, String) {
    // The table in use is the Porch's (`Table::cached`): this one, else the
    // one before it when this one is refused.
    let (used, refused) = match Table::read(path) {
        Ok(table) => (Some(table), None),
        Err(why) if path.exists() => (Table::read(&table::previous(path)).ok(), Some(why)),
        Err(_) => (None, None),
    };
    let refused = refused.map_or(String::new(), |why| {
        // Another version's tokenizer or header features, or another format: the same version on every device mends it.
        let other_version = ["tokenizer", "header features", "format"].iter().any(|w| why.contains(w));
        let why = tr().text(if other_version { "spam-app-refused-version" } else { "spam-app-refused-damaged" }, None);
        say(if used.is_some() { "spam-app-refused" } else { "spam-app-refused-none" }, &[("why", why)])
    });
    let Some(table) = used else {
        let none = if !refused.is_empty() { String::new() } else { tr().text(if trains { "spam-app-none" } else { "spam-app-none-phone" }, None) };
        return (none, String::new(), refused);
    };
    let meta = &table.meta;
    let here = sioul_sync::lease::host_name();
    let (id, device) = match meta.device.as_str() {
        "" => ("spam-app-table-somewhere", String::new()),
        device if device == here => ("spam-app-table-here", String::new()),
        device => ("spam-app-table", device.to_string()),
    };
    let line = say(id, &[("device", device), ("when", when(meta.trained_at)), ("ham", grouped(meta.ham)), ("spam", grouped(meta.spam))]);
    let metric = |name: &str| meta.metrics.get(name).copied().filter(|v| v.is_finite());
    let measured = match (metric("threshold_spam"), metric("ham_called_spam"), metric("spam_caught")) {
        (Some(threshold), Some(ham), Some(spam)) => say("spam-app-table-numbers", &[("threshold", percent(threshold, 0)), ("ham", percent(ham, 1)), ("spam", percent(spam, 1))]),
        _ => String::new(),
    };
    (line, measured, refused)
}

/// "on Tuesday 6 October at 10:00", in your language.
fn when(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds).ok().filter(|_| seconds > 0).map(|t| tr().when(&t.to_zoned(jiff::tz::TimeZone::system()))).unwrap_or_default()
}

/// A count with this language's thousands separator: "41,200", "41 200".
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let separator = tr().text("thousands-separator", None);
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push_str(&separator);
        }
        out.push(c);
    }
    out
}

/// A share as a percentage, with `places` decimals: "0.4%", "0,4 %".
fn percent(fraction: f64, places: usize) -> String {
    let n = format!("{:.places$}", fraction * 100.0).replace('.', &tr().text("decimal-separator", None));
    say("spam-percent", &[("n", n)])
}

/// The training, on a computer (crates/sioul-learn is never built for a phone).
#[cfg(not(target_os = "android"))]
mod training {
    use super::{Status, grouped, percent, say, tr, when};
    use crate::backend::{QtThread, Shared, load_config};
    use cxx_qt_lib::QString;
    use sioul_learn::{Cancel, Dirs, LearnError, Progress, Stage, auto, corpus, train};
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::{Duration, Instant};

    /// Progress said at most this often (and at each new step).
    const PACE: Duration = Duration::from_millis(300);

    /// The training running, or how the last one ended.
    struct Run {
        /// Some while one runs: what stops it.
        cancel: Option<Cancel>,
        stopping: bool,
        stage: Option<Stage>,
        progress: String,
        fraction: f64,
        ended: String,
        said: Option<Instant>,
    }

    static RUN: Mutex<Run> = Mutex::new(Run { cancel: None, stopping: false, stage: None, progress: String::new(), fraction: -1.0, ended: String::new(), said: None });

    /// The corpus download over every folder: each folder's count as it
    /// comes, and for the addresses not reached yet what the last download
    /// left on their server (`corpus::status`): "N of about M".
    struct Fetch {
        folders: BTreeMap<String, (u64, u64)>,
        left: BTreeMap<String, u64>,
        /// The whole download's size, when the corpus says it before starting.
        overall: Option<u64>,
    }

    impl Fetch {
        fn new(dirs: &Dirs) -> Fetch {
            let left = corpus::status(dirs).accounts.iter().map(|a| (a.id.clone(), a.folders.iter().map(|f| u64::from(f.on_server).saturating_sub(f.recorded)).sum())).collect();
            Fetch { folders: BTreeMap::new(), left, overall: None }
        }

        /// Fetched so far, and about how many in all.
        fn note(&mut self, p: &Progress) -> (u64, u64) {
            if p.detail.is_empty() {
                self.overall = (p.total > 0).then_some(p.total).or(self.overall);
            } else {
                self.folders.insert(p.detail.clone(), (p.done, p.total));
            }
            let done: u64 = self.folders.values().map(|(done, _)| done).sum();
            let seen: u64 = self.folders.values().map(|(_, total)| total).sum();
            let begun = |account: &str| self.folders.keys().any(|place| place.split(" · ").next() == Some(account));
            let rest: u64 = self.left.iter().filter(|(account, _)| !begun(account)).map(|(_, n)| n).sum();
            (done, self.overall.unwrap_or(seen + rest).max(done))
        }
    }

    /// The training's part of the settings: running or not, and what is kept on disk.
    pub(super) fn fill(status: &mut Status) {
        {
            let run = RUN.lock().unwrap_or_else(PoisonError::into_inner);
            status.running = run.cancel.is_some();
            status.stopping = run.stopping;
            if status.running {
                status.progress = run.progress.clone();
                status.fraction = run.fraction;
            }
            status.ended = run.ended.clone();
        }
        let dirs = Dirs::standard();
        status.by_itself = load_config().spam.trains_by_itself();
        (status.by_itself_here, status.by_itself_line) = by_itself_line(&dirs, &sioul_sync::lease::host_name());
        if let Some(summary) = train::last(&dirs) {
            status.last = last_lines(&summary);
        }
        status.corpus = corpus_lines(&dirs);
        status.outside = sioul_learn::external::sources(&dirs)
            .iter()
            .map(|source| say("spam-app-outside", &[("source", source.name.clone()), ("ham", grouped(source.counts.ham)), ("spam", grouped(source.counts.spam)), ("first", day(source.counts.first)), ("last", day(source.counts.last))]))
            .collect();
    }

    /// A day, in your language: "20 June 2023".
    fn day(seconds: i64) -> String {
        jiff::Timestamp::from_second(seconds).ok().map(|t| tr().day(t.to_zoned(jiff::tz::TimeZone::system()).date())).unwrap_or_default()
    }

    pub(super) fn start(qt: QtThread, shared: Arc<Shared>) -> String {
        // A training apart (by itself, or from the command line) ends on its own: never two at once.
        if auto::a_job_runs(&Dirs::standard()) {
            return tr().text("spam-app-busy-job", None);
        }
        let cancel = Cancel::new();
        {
            let mut run = RUN.lock().unwrap_or_else(PoisonError::into_inner);
            if run.cancel.is_some() {
                return tr().text("spam-app-busy", None);
            }
            *run = Run { cancel: Some(cancel.clone()), stopping: false, stage: None, progress: tr().text("spam-app-starting", None), fraction: -1.0, ended: String::new(), said: None };
        }
        let started = std::thread::Builder::new().name("spam-training".into()).spawn(move || {
            lower_priority();
            let config = load_config();
            let dirs = Dirs::standard();
            // Held while it trains: the minute of `sioul watch` sees a training runs (`auto::a_job_runs`).
            let _held = window_lock(&dirs);
            let mut fetch = Fetch::new(&dirs);
            let result = train::run(&config, &dirs, &mut |p: &Progress| moved(&qt, &mut fetch, p), &cancel);
            let ended = match result {
                Ok(_) => String::new(),
                Err(e) => failure(&e),
            };
            {
                let mut run = RUN.lock().unwrap_or_else(PoisonError::into_inner);
                (run.cancel, run.stopping, run.stage, run.ended) = (None, false, None, ended);
                run.progress.clear();
            }
            let status = super::status();
            let _ = qt.queue(move |mut sioul| sioul.as_mut().spam_changed(QString::from(&status)));
            // A new table: the Porch reads it at once.
            crate::backend::show(&qt, &shared);
        });
        match started {
            Ok(_) => String::new(),
            Err(e) => {
                RUN.lock().unwrap_or_else(PoisonError::into_inner).cancel = None;
                say("spam-app-error", &[("detail", e.to_string())])
            }
        }
    }

    pub(super) fn stop(qt: QtThread) {
        {
            let mut run = RUN.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(cancel) = run.cancel.as_ref() else { return };
            cancel.cancel();
            run.stopping = true;
        }
        let status = super::status();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().spam_changed(QString::from(&status)));
    }

    /// The window's minute (`super::by_itself`): what is known read, the
    /// system asked only when nothing else holds a training back; the run
    /// started apart when everything allows it.
    pub(super) fn by_itself(qt: QtThread) {
        // One look at a time: a minute's may still be asking the system when the next comes.
        static LOOKING: AtomicBool = AtomicBool::new(false);
        if LOOKING.swap(true, Ordering::SeqCst) {
            return;
        }
        let config = load_config();
        let (dirs, now) = (Dirs::standard(), jiff::Timestamp::now().as_second());
        let window_trains = RUN.lock().unwrap_or_else(PoisonError::into_inner).cancel.is_some();
        let focus = sioul_core::timelog::running().is_some();
        let facts = auto::facts(&dirs, config.spam.trains_by_itself(), &sioul_sync::lease::host_name(), window_trains, focus, now, sioul_sync::power::read);
        let started = match auto::decide(&facts, now) {
            auto::Decision::Train { fetch } => start_apart(fetch),
            auto::Decision::Wait(_) => false,
        };
        // While one runs, or just after it ended: the settings say it.
        let lately = facts.last_run.as_ref().is_some_and(|run| run.ended.is_none_or(|end| now - end < 120));
        if started || lately {
            let status = super::status();
            let _ = qt.queue(move |mut sioul| sioul.as_mut().spam_changed(QString::from(&status)));
        }
        LOOKING.store(false, Ordering::SeqCst);
    }

    /// `sioul spam train --by-itself` (`sioul`, beside this program or on
    /// the PATH): it starts the run as a job apart and leaves at once.
    fn start_apart(fetch: bool) -> bool {
        let Some(sioul) = crate::remind::command() else { return false };
        let mut command = std::process::Command::new(sioul);
        command.arg("--config").arg(crate::backend::config_path()).arg("--language").arg(tr().language());
        command.args(["spam", "train", "--by-itself", "--threads", &auto::threads().to_string()]);
        if !fetch {
            command.arg("--no-fetch");
        }
        command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        command.status().is_ok_and(|status| status.success())
    }

    /// Who trains it again by itself, and what its last run here did: whether
    /// this computer is the one (the switch can be changed here), and the
    /// sentence under the switch.
    fn by_itself_line(dirs: &Dirs, here: &str) -> (bool, String) {
        match auto::trainer(dirs, here) {
            auto::Trainer::Elsewhere(device) => (false, say("spam-app-by-itself-elsewhere", &[("device", device)])),
            auto::Trainer::Nobody => (true, tr().text("spam-app-by-itself-first", None)),
            auto::Trainer::Here => {
                let Some(run) = auto::Run::load(dirs) else { return (true, String::new()) };
                let end = run.ended.unwrap_or(run.started);
                // Just started, its process may not hold its lock yet: running too.
                let just = jiff::Timestamp::now().as_second() - run.started < 60;
                let (id, at) = match run.outcome {
                    None if just || auto::a_job_runs(dirs) => ("spam-app-by-itself-running", run.started),
                    Some(auto::Outcome::Replaced) => ("spam-app-by-itself-replaced", end),
                    Some(auto::Outcome::Kept) => ("spam-app-by-itself-kept", end),
                    Some(auto::Outcome::Stopped) => ("spam-app-by-itself-stopped", end),
                    Some(auto::Outcome::Failed) | None => ("spam-app-by-itself-failed", end),
                };
                (true, say(id, &[("when", when(at))]))
            }
        }
    }

    /// The window's own lock while it trains, beside the jobs' (`jobs/window.lock`).
    fn window_lock(dirs: &Dirs) -> Option<std::fs::File> {
        let jobs = dirs.state.join("jobs");
        let mut folder = std::fs::DirBuilder::new();
        folder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut folder, 0o700);
        folder.create(&jobs).ok()?;
        let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(jobs.join("window.lock")).ok()?;
        file.lock().ok()?;
        Some(file)
    }

    /// A step moved on: said, at most a few times a second, and at each new step.
    fn moved(qt: &QtThread, fetch: &mut Fetch, p: &Progress) {
        let (line, fraction) = match p.stage {
            Stage::Corpus => {
                let (done, total) = fetch.note(p);
                let place = p.detail.clone();
                let line = if total == 0 {
                    say("spam-app-fetching-start", &[("place", place)])
                } else {
                    say("spam-app-fetching", &[("done", grouped(done)), ("total", grouped(total)), ("place", place)])
                };
                (line, if total > 0 { done as f64 / total as f64 } else { -1.0 })
            }
            stage => {
                let name = tr().text(&format!("spam-stage-{}", stage.as_str()), None);
                if p.total > 0 { (say("spam-app-step", &[("stage", name), ("done", grouped(p.done)), ("total", grouped(p.total))]), p.done as f64 / p.total as f64) } else { (name, -1.0) }
            }
        };
        {
            let mut run = RUN.lock().unwrap_or_else(PoisonError::into_inner);
            let new_step = run.stage != Some(p.stage);
            (run.stage, run.progress, run.fraction) = (Some(p.stage), line, fraction);
            if !new_step && run.said.is_some_and(|at| at.elapsed() < PACE) {
                return;
            }
            run.said = Some(Instant::now());
        }
        let status = super::status();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().spam_changed(QString::from(&status)));
    }

    /// Why a run made no table, in a sentence.
    fn failure(e: &LearnError) -> String {
        match e {
            LearnError::Cancelled => tr().text("spam-app-stopped", None),
            LearnError::TooFew { ham, spam } => say("spam-app-too-few", &[("ham", grouped(*ham)), ("spam", grouped(*spam)), ("least", train::Options::default().least_of_each.to_string())]),
            LearnError::Disk(_) => tr().text("spam-app-disk", None),
            LearnError::Fold(_) => tr().text("spam-app-fold", None),
            other => say("spam-app-error", &[("detail", other.to_string())]),
        }
    }

    /// The last training (`trained.toml`): what it did with the table and
    /// why (the two tables compared on the newest messages the one in place
    /// never saw), what the table in use learned from (again from every
    /// message, when it replaced the other), and what the same model learned
    /// from the oldest 80 % measured on the newest 20 %, at the two thresholds.
    fn last_lines(summary: &train::Summary) -> Vec<String> {
        let at = when(summary.trained_at);
        let compared = summary.compared.unwrap_or_default();
        let counted = |id: &str| say(id, &[("when", at.clone()), ("n", grouped(compared.ham + compared.spam)), ("new", compared.new_ham_lost.to_string()), ("old", compared.current_ham_lost.to_string())]);
        let mut lines = vec![match summary.reason.as_str() {
            "no-table" => say("spam-app-last-new", &[("when", at.clone())]),
            "unreadable" => say("spam-app-last-unreadable", &[("when", at.clone())]),
            "nothing-new" => say("spam-app-last-nothing-new", &[("when", at.clone())]),
            "worse" => counted("spam-app-last-kept"),
            _ => counted("spam-app-last-replaced"),
        }];
        if let Some(refit) = &summary.refit {
            lines.push(say("spam-app-refit", &[("n", grouped(refit.ham + refit.spam))]));
        }
        let split = &summary.split;
        let since = jiff::Timestamp::from_second(split.test_from).ok().filter(|_| split.test_from > 0).map(|t| tr().day(t.to_zoned(jiff::tz::TimeZone::system()).date())).unwrap_or_default();
        lines.push(say(
            "spam-app-learned",
            &[("ham", grouped(split.train_ham)), ("spam", grouped(split.train_spam)), ("since", since), ("testham", grouped(split.test_ham)), ("testspam", grouped(split.test_spam))],
        ));
        for (at, id) in [(&summary.test.at_spam, "spam-app-at-spam"), (&summary.test.at_unsure, "spam-app-at-unsure")] {
            lines.push(say(
                id,
                &[
                    ("threshold", percent(at.threshold, 0)),
                    ("ham", percent(at.ham_called_spam.fraction(), 1)),
                    ("hamlow", percent(at.ham_called_spam.low, 1)),
                    ("hamhigh", percent(at.ham_called_spam.high, 1)),
                    ("spam", percent(at.spam_caught.fraction(), 1)),
                    ("spamlow", percent(at.spam_caught.low, 1)),
                    ("spamhigh", percent(at.spam_caught.high, 1)),
                ],
            ));
        }
        lines.push(tr().text("spam-intervals", None));
        // Outside material: what it gave, and its baseline on its held-out newest fifth.
        for (source, outside) in &summary.outside {
            lines.push(say(
                "spam-app-outside-learned",
                &[("source", source.clone()), ("trainham", grouped(outside.train_ham)), ("trainspam", grouped(outside.train_spam)), ("heldham", grouped(outside.held_ham)), ("heldspam", grouped(outside.held_spam))],
            ));
            if let Some(baseline) = &outside.baseline {
                let at = &baseline.at_spam;
                lines.push(say(
                    "spam-app-outside-baseline",
                    &[("threshold", percent(at.threshold, 0)), ("ham", percent(at.ham_called_spam.fraction(), 1)), ("spam", percent(at.spam_caught.fraction(), 1)), ("auc", baseline.auc.map_or_else(String::new, |a| format!("{a:.3}").replace('.', &tr().text("decimal-separator", None))))],
                ));
            }
        }
        // Measured at other thresholds than yours now: said, the numbers would differ.
        let (spam, unsure) = load_config().spam.thresholds();
        let moved = |then: f64, now: f32| (then - f64::from(now)).abs() > 0.005;
        if moved(summary.threshold_spam, spam) || moved(summary.threshold_unsure, unsure) {
            lines.push(say("spam-app-thresholds-then", &[("spam", percent(summary.threshold_spam, 0)), ("unsure", percent(summary.threshold_unsure, 0))]));
        }
        lines
    }

    /// What the corpus holds, its last download, the room left, and the addresses it could not read.
    fn corpus_lines(dirs: &Dirs) -> Vec<String> {
        let status = corpus::status(dirs);
        let mut lines = Vec::new();
        let records: u64 = status.accounts.iter().flat_map(|a| a.folders.iter()).map(|f| f.records).sum();
        let bytes: u64 = status.accounts.iter().map(|a| a.bytes).sum();
        if records == 0 {
            lines.push(tr().text("spam-app-corpus-none", None));
        } else {
            let last = status.last_run.map(when).unwrap_or_default();
            lines.push(say("spam-app-corpus", &[("records", grouped(records)), ("accounts", status.accounts.len().to_string()), ("size", grouped(bytes >> 20)), ("when", last)]));
        }
        if let Some(held) = status.held {
            lines.push(say("spam-held", &[("free", grouped(held.free >> 20))]));
        }
        if let Some(free) = corpus::free_space(&dirs.data).or_else(|| corpus::free_space(&sioul_core::config::data_dir())) {
            lines.push(say("spam-app-room", &[("free", format!("{:.1}", free as f64 / f64::from(1u32 << 30)).replace('.', &tr().text("decimal-separator", None)))]));
        }
        for (account, detail) in &status.errors {
            lines.push(say("spam-account-failed", &[("account", account.clone()), ("detail", detail.lines().next().unwrap_or_default().to_string())]));
        }
        lines
    }

    /// The training's thread, and fastText's threads it starts, below the
    /// window's: the computer stays usable while every core learns. Linux
    /// gives each thread its own priority, which the threads it starts take.
    fn lower_priority() {
        #[cfg(target_os = "linux")]
        // SAFETY: setpriority only reads its arguments; 0 is this thread on Linux.
        unsafe {
            libc::setpriority(libc::PRIO_PROCESS, 0, 10);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn place(detail: &str, done: u64, total: u64) -> Progress {
            Progress { stage: Stage::Corpus, done, total, detail: detail.into() }
        }

        /// "N of about M": each folder's count as it comes, the addresses not
        /// reached yet as the last download left them; the corpus's own
        /// estimate when it gives one; never fewer than what came.
        #[test]
        fn messages_fetched_of_about_all() {
            let mut fetch = Fetch { folders: BTreeMap::new(), left: BTreeMap::from([("home".to_string(), 100), ("work".to_string(), 50)]), overall: None };
            assert_eq!(fetch.note(&place("home · INBOX", 0, 30)), (0, 30 + 50));
            assert_eq!(fetch.note(&place("home · INBOX", 10, 30)), (10, 80));
            assert_eq!(fetch.note(&place("home · Archive", 5, 20)), (15, 100));
            assert_eq!(fetch.note(&place("work · INBOX", 1, 7)), (16, 57));
            // The corpus said how many in all, before its first folder.
            let mut fetch = Fetch { folders: BTreeMap::new(), left: BTreeMap::new(), overall: None };
            assert_eq!(fetch.note(&place("", 0, 400)), (0, 400));
            assert_eq!(fetch.note(&place("home · INBOX", 120, 300)), (120, 400));
            assert_eq!(fetch.note(&place("home · Junk", 300, 300)), (420, 420), "never fewer than what came");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::card::Card;
    use sioul_core::porch::Triaged;
    use sioul_core::spam::{Actions, Filter};

    /// After a fetch, what goes into the Junk folder and what is flagged
    /// where it is: among the filter's catches, by a verdict whose class you
    /// chose "Move to spam" or "Flag only" for, not moved yet.
    #[test]
    fn what_the_filter_moves_or_flags_after_a_fetch() {
        let triaged = |n: u32, lane: Lane, reasons: Vec<Reason>| {
            let mut card = Card::from_bytes(b"From: Prize <win@lottery.test>\r\nSubject: You won\r\n\r\nClaim it.\r\n").unwrap();
            card.path = Some(PathBuf::from(format!("/mail/home/new/1759400000.U1-{n}.sioul")));
            Triaged { card, lane, trust: sioul_core::trust::Trust::Unverified, code: None, reasons, priority: Default::default(), checks: None, assessment: None }
        };
        let filter = |spam: Action, unsure: Action| Filter { actions: Actions { spam, unsure, ham: Action::Nothing }, threshold_spam: 0.95, threshold_unsure: 0.5, table: PathBuf::new() };
        let judged = vec![
            triaged(1, Lane::Review, vec![Reason::LearnedSpam { p: 0.99 }]),
            triaged(2, Lane::Review, vec![Reason::Unsure { p: 0.6 }]),
            triaged(3, Lane::Review, vec![Reason::LearnedSpam { p: 0.99 }, Reason::MovedToJunk]),
            triaged(4, Lane::Screener, vec![Reason::FirstMessage]),
        ];
        let file = |n: u32| PathBuf::from(format!("/mail/home/new/1759400000.U1-{n}.sioul"));
        assert_eq!(chosen(&judged, &filter(Action::Move, Action::Flag), Action::Move), vec![(file(1), Class::Spam)]);
        assert_eq!(chosen(&judged, &filter(Action::Move, Action::Flag), Action::Flag), vec![(file(2), Class::Unsure)], "the doubt flagged");
        assert_eq!(chosen(&judged, &filter(Action::Move, Action::Move), Action::Move).len(), 2, "the doubt too, never twice");
        assert!(chosen(&judged, &filter(Action::Flag, Action::Flag), Action::Move).is_empty(), "flagged only: nothing moves");
        assert_eq!(chosen(&judged, &filter(Action::Flag, Action::Flag), Action::Flag), vec![(file(1), Class::Spam), (file(2), Class::Unsure)], "each flag once: what was moved is not flagged");
    }
}

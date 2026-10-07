// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Long jobs run apart (docs/mcp.md, "The spam filter's tools"): the MCP's
//! `spam_fetch` and `spam_train` start a detached `sioul spam fetch|train
//! --job <id>` and answer at once with its id. The job writes where it
//! stands, then its result, into `$XDG_STATE_HOME/sioul/spam/jobs/<id>.json`,
//! which `spam_job` and `sioul spam job` read.
//!
//! - Its process holds `<id>.lock` while it runs: a job whose file still
//!   says it runs, but whose lock nobody holds, ended without a word
//!   (killed, the computer stopped), and is said so.
//! - `<id>.stop`, made by `spam_job` with `stop` (or `sioul spam job <id>
//!   --stop`), stops it at its next step; what was downloaded stays.
//! - What it printed goes to `<id>.log` (its errors; never a message).
//! - The folder and its files are yours alone (0700, 0600); a job's files go
//!   a week after it ended. A training's result names its worst errors by
//!   their senders and subjects, masked: kept no longer than that.
//! - One job of each kind at a time.

use crate::Session;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sioul_learn::{Cancel, Progress, Stage};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The jobs that run apart.
pub(crate) const KINDS: [&str; 2] = ["fetch", "train"];
/// Days a job's files are kept after it ended.
const KEPT_DAYS: i64 = 7;
/// Seconds a job may stay "starting" before its process is looked for.
const GRACE: i64 = 60;

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum State {
    /// Started; its process has not taken it yet.
    Starting,
    Running,
    /// Ended with its result.
    Done,
    /// Ended on an error, said.
    Failed,
    /// Stopped when asked.
    Stopped,
    /// Its process is gone without a word: killed, or the computer stopped.
    Died,
}

impl State {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            State::Starting => "starting",
            State::Running => "running",
            State::Done => "done",
            State::Failed => "failed",
            State::Stopped => "stopped",
            State::Died => "died",
        }
    }

    fn ended(self) -> bool {
        !matches!(self, State::Starting | State::Running)
    }
}

/// What a job is on now: a stage of `sioul_learn`, how far, and a line saying it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Step {
    pub stage: String,
    pub done: u64,
    pub total: u64,
    /// An account and a folder while downloading; never a message.
    pub detail: String,
    /// The same, said in the person's language.
    pub line: String,
}

/// One job, as its file keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Job {
    pub id: String,
    /// "fetch" or "train".
    pub kind: String,
    /// What it was asked, as the command line takes it.
    pub args: Vec<String>,
    pub state: State,
    /// Its process, once it runs.
    #[serde(default)]
    pub pid: Option<u32>,
    /// Unix seconds.
    pub created: i64,
    #[serde(default)]
    pub started: Option<i64>,
    #[serde(default)]
    pub ended: Option<i64>,
    pub updated: i64,
    #[serde(default)]
    pub progress: Option<Step>,
    /// What it said at the end, in lines (the command's, in the person's language), and as data.
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
    /// Asked to stop: it stops at its next step.
    #[serde(default)]
    pub stop_asked: bool,
}

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// The jobs' folder: `$XDG_STATE_HOME/sioul/spam/jobs`.
pub(crate) fn folder() -> PathBuf {
    sioul_core::config::state_dir().join("spam").join("jobs")
}

fn file(id: &str, extension: &str) -> PathBuf {
    folder().join(format!("{id}.{extension}"))
}

/// Whether `id` can name a job: letters, digits and dashes, as `start`
/// makes them; anything else (a path, "..") names none.
pub(crate) fn valid(id: &str) -> bool {
    (1..=64).contains(&id.len()) && !id.starts_with('-') && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The folder, made yours alone.
fn private_folder() -> Result<(), String> {
    let folder = folder();
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(&folder).map_err(|e| format!("{}: {e}", folder.display()))
}

/// A file of the folder opened to write, yours alone.
fn private_file(path: &std::path::Path, truncate: bool) -> Result<std::fs::File, String> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(truncate);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A job's file written whole: beside, then moved, so that a reader never reads half of it.
fn write(job: &Job) -> Result<(), String> {
    use std::io::Write;
    let path = file(&job.id, "json");
    let beside = file(&job.id, "json.new");
    let text = serde_json::to_string_pretty(job).map_err(|e| e.to_string())?;
    private_file(&beside, true)?.write_all(text.as_bytes()).map_err(|e| format!("{}: {e}", beside.display()))?;
    std::fs::rename(&beside, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A job's file as written.
fn read(id: &str) -> Result<Job, String> {
    let path = file(id, "json");
    let text = std::fs::read_to_string(&path).map_err(|_| format!("No job “{id}”."))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Whether a process holds the job's lock: it runs.
fn held(id: &str) -> bool {
    let Ok(lock) = std::fs::OpenOptions::new().read(true).open(file(id, "lock")) else { return false };
    match lock.try_lock() {
        Ok(()) => {
            let _ = lock.unlock();
            false
        }
        Err(_) => true,
    }
}

/// The last lines its process printed, on one line each (an error's words).
fn log_tail(id: &str) -> String {
    let text = std::fs::read_to_string(file(id, "log")).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(3)..].iter().map(|l| crate::one_line(l)).collect::<Vec<_>>().join(" / ")
}

/// A job as it stands: one whose file says it runs, but whose process
/// holds its lock no more (after the grace a starting one has), ended
/// without a word: said "died", with the last words it printed.
pub(crate) fn look(id: &str) -> Result<Job, String> {
    if !valid(id) {
        return Err(format!("No job “{}”.", crate::one_line(&id.chars().take(64).collect::<String>())));
    }
    let mut job = read(id)?;
    if !job.state.ended() && !held(id) && !(job.state == State::Starting && now() - job.created < GRACE) {
        job.state = State::Died;
        let tail = log_tail(id);
        job.error = Some(if tail.is_empty() { "Its process ended without a word: killed, or the computer stopped.".to_string() } else { tail });
    }
    job.stop_asked |= file(id, "stop").exists();
    Ok(job)
}

/// Every job kept, the newest first.
pub(crate) fn list() -> Vec<Job> {
    let mut jobs: Vec<Job> = std::fs::read_dir(folder())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".json").map(str::to_string))
        .filter(|id| valid(id))
        .filter_map(|id| look(&id).ok())
        .collect();
    jobs.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
    jobs
}

/// The job of this kind that runs, if one does.
pub(crate) fn running(kind: &str) -> Option<Job> {
    list().into_iter().find(|j| j.kind == kind && !j.state.ended())
}

/// The files of jobs ended more than a week ago, gone.
fn prune() {
    let before = now() - KEPT_DAYS * 86_400;
    for job in list().into_iter().filter(|j| j.state.ended() && j.ended.unwrap_or(j.updated) < before) {
        for extension in ["json", "lock", "log", "stop", "json.new"] {
            let _ = std::fs::remove_file(file(&job.id, extension));
        }
    }
}

/// A new job's id: when, what, and four letters: "20261007-153012-train-kxqa".
fn new_id(kind: &str) -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut n = std::collections::hash_map::RandomState::new().build_hasher().finish();
    let letters: String = (0..4)
        .map(|_| {
            let letter = char::from(b'a' + (n % 26) as u8);
            n /= 26;
            letter
        })
        .collect();
    format!("{}-{kind}-{letters}", jiff::Zoned::now().strftime("%Y%m%d-%H%M%S"))
}

/// Starts a job: its file made ("starting", `create`), then `sioul spam
/// <kind> --job <id> <args>` started apart, in a group of its own (on
/// Windows, without a console), so that it goes on when the program that
/// started it ends. Answers at once; the job's own process says the rest in its file.
pub(crate) fn start(s: &Session, kind: &str, args: Vec<String>) -> Result<Job, String> {
    let job = create(s, kind, args)?;
    spawn(s, job)
}

/// A new job's file, "starting": one of each kind at a time; those ended a week ago go.
pub(crate) fn create(s: &Session, kind: &str, args: Vec<String>) -> Result<Job, String> {
    if !KINDS.contains(&kind) {
        return Err(format!("No such job: “{kind}”."));
    }
    private_folder()?;
    prune();
    if let Some(job) = running(kind) {
        return Err(s.say("spam-job-busy", &[("kind", kind.to_string()), ("id", job.id)]));
    }
    let created = now();
    let job = Job {
        id: new_id(kind),
        kind: kind.to_string(),
        args,
        state: State::Starting,
        pid: None,
        created,
        started: None,
        ended: None,
        updated: created,
        progress: None,
        lines: Vec::new(),
        result: None,
        error: None,
        stop_asked: false,
    };
    write(&job)?;
    Ok(job)
}

/// The job's process, started apart; the job said failed when it cannot start.
pub(crate) fn spawn(s: &Session, job: Job) -> Result<Job, String> {
    let started = (|| -> Result<std::process::Child, String> {
        let program = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut command = std::process::Command::new(program);
        command.arg("--config").arg(&s.config_path).arg("--language").arg(s.tr.language()).args(["spam", job.kind.as_str(), "--job", job.id.as_str()]).args(&job.args);
        let log = private_file(&file(&job.id, "log"), true)?;
        command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(log);
        detach(&mut command);
        command.spawn().map_err(|e| e.to_string())
    })();
    match started {
        Ok(mut child) => {
            let pid = child.id();
            // Its end waited for here, so that it leaves no zombie while this program runs.
            let _ = std::thread::Builder::new().name("spam-job".into()).spawn(move || {
                let _ = child.wait();
            });
            // Its own process writes its file from now on: this answer only says its id and process.
            Ok(Job { pid: Some(pid), ..job })
        }
        Err(e) => {
            let ended = now();
            let failed = Job { state: State::Failed, ended: Some(ended), updated: ended, error: Some(e.clone()), ..job };
            let _ = write(&failed);
            Err(e)
        }
    }
}

/// A process of its own, which the end of the one that started it does not end.
fn detach(command: &mut std::process::Command) {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);
    #[cfg(windows)]
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP.
    std::os::windows::process::CommandExt::creation_flags(command, 0x0000_0008 | 0x0000_0200);
}

/// Asks a running job to stop at its next step.
pub(crate) fn stop(id: &str) -> Result<Job, String> {
    let job = look(id)?;
    if !job.state.ended() {
        private_file(&file(id, "stop"), false)?;
    }
    look(id)
}

/// A job, from inside its own process: it holds the lock while it runs,
/// says where it stands at most once a second (and at each new step), and
/// stops when asked.
pub(crate) struct Running {
    job: Job,
    /// Held while the process lives: the lock says it runs.
    _lock: std::fs::File,
    said: Option<Instant>,
    cancel: Cancel,
}

impl Running {
    /// Takes the job `start` made: its lock, its state "running", its process.
    pub(crate) fn open(id: &str) -> Result<Running, String> {
        if !valid(id) {
            return Err(format!("No job “{}”.", crate::one_line(id)));
        }
        let mut job = read(id)?;
        let lock = private_file(&file(id, "lock"), false)?;
        lock.lock().map_err(|e| e.to_string())?;
        if job.state.ended() {
            return Err(format!("The job {id} has ended already."));
        }
        let started = now();
        (job.state, job.pid, job.started, job.updated) = (State::Running, Some(std::process::id()), Some(started), started);
        write(&job)?;
        let cancel = Cancel::new();
        // Asked to stop, it stops at its next step: fastText too, which says nothing while it learns.
        let (watched, stop) = (cancel.clone(), file(id, "stop"));
        let _ = std::thread::Builder::new().name("spam-job-stop".into()).spawn(move || {
            while !watched.cancelled() {
                if stop.exists() {
                    watched.cancel();
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        Ok(Running { job, _lock: lock, said: None, cancel })
    }

    /// What stops it.
    pub(crate) fn cancel(&self) -> Cancel {
        self.cancel.clone()
    }

    /// Where it stands, written at most once a second, and at each new step or folder.
    pub(crate) fn see(&mut self, s: &Session, p: &Progress) {
        let stage = p.stage.as_str().to_string();
        let new = self.job.progress.as_ref().is_none_or(|old| old.stage != stage || old.detail != p.detail);
        if !new && self.said.is_some_and(|at| at.elapsed() < Duration::from_secs(1)) {
            return;
        }
        self.said = Some(Instant::now());
        self.job.progress = Some(Step { stage, done: p.done, total: p.total, detail: crate::one_line(&p.detail), line: step_line(s, p) });
        self.job.updated = now();
        self.job.stop_asked |= self.cancel.cancelled();
        let _ = write(&self.job);
    }

    /// The end: its result (lines and data), or what went wrong; "stopped" when asked to stop.
    pub(crate) fn finish(mut self, outcome: Result<(Vec<String>, Value), String>) -> Result<(), String> {
        let ended = now();
        (self.job.ended, self.job.updated) = (Some(ended), ended);
        match outcome {
            Ok((lines, data)) => (self.job.state, self.job.lines, self.job.result) = (State::Done, lines, Some(data)),
            Err(e) => (self.job.state, self.job.error) = (if self.cancel.cancelled() { State::Stopped } else { State::Failed }, Some(e)),
        }
        self.job.stop_asked |= self.cancel.cancelled();
        // The watcher of the stop file ends with it.
        self.cancel.cancel();
        let _ = std::fs::remove_file(file(&self.job.id, "stop"));
        write(&self.job)
    }
}

/// A step said in the person's language: "home · INBOX: 1200 / 3000", "Learning your mail's words…".
pub(crate) fn step_line(s: &Session, p: &Progress) -> String {
    let counted = if p.total > 0 { format!(" {} / {}", p.done, p.total) } else { String::new() };
    match p.stage {
        Stage::Corpus if p.detail.is_empty() => s.say("spam-fetch-estimate", &[("n", p.total.to_string())]),
        Stage::Corpus => format!("{}:{counted}", crate::one_line(&p.detail)),
        stage => format!("{}{counted}", s.tr.text(&format!("spam-stage-{}", stage.as_str()), None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids that could name a file elsewhere name no job.
    #[test]
    fn ids_name_nothing_else() {
        assert!(valid("20261007-153012-train-kxqa"));
        for id in ["", "..", "../x", "a/b", "-x", "x.json", "a b", &"x".repeat(65)] {
            assert!(!valid(id), "{id}");
        }
        let id = new_id("fetch");
        assert!(valid(&id) && id.contains("-fetch-"), "{id}");
    }
}

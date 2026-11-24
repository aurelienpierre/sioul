// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The engine (docs/rules.md, "How rules run"): rules made to converge, not
//! to fire. For each rule that runs and each object among those it is
//! handed that matches its "when", its result must exist, and every device
//! comes to the same state:
//!
//! - **Once, on every device**: what a rule makes has an identifier worked
//!   out from the rule's id and the object's address (`task_uid`,
//!   `paper_id`), and its bytes are stamped from the message's arrival, never
//!   from a device's clock, so that two devices that both act write the same
//!   bytes; every act is written in the shared record (`acts`), which every
//!   device reads before acting: a rule never acts twice on one object.
//! - **Undone everywhere**: an act undone (`undo`), or said "not for this
//!   one", is a state of the record: at each pass, and when the record
//!   arrives from another device, each device takes back what it wrote of
//!   that pair (`take_back`), and no device writes it again.
//! - **Refusals wait**: a rule that matched but could not act for a reason
//!   that may pass (a write that failed, a list missing, the notices kept by
//!   another device) leaves a pending line, tried again at each pass for a
//!   week (`acts::PENDING_MS`), said once.
//! - **The guards, here and not in each action** (design, parts 3 and 5):
//!   never at an address's first fetch, never on what came before the rule
//!   (`Rule::since`) nor on mail more than a week old, never on what the
//!   person sent; only mail the Porch lets through from a sender something
//!   proves: never what it sets aside, holds (the screener, a public
//!   address), finds hostile, what your spam filter caught, nor what lies in
//!   a junk or a trash folder (`held`); never what sends, deletes, pays, or
//!   writes a setting, a rule or health (`declare::forbidden`), never a
//!   read-only list; never on what the same rule made, and a chain stops at
//!   `declare::CHAIN` steps.
//! - **Nothing kept from agents leaks through what a rule makes**: a task
//!   or a paper is in its message's project unless the rule names another,
//!   and consent follows what was made of a message (`consent`); a message
//!   holding a code gives no subject to a title, a name or a notice, and the
//!   numbers a mask hides are hidden there too (`masking`).
//! - **Said once, with Undo**: each act comes back as a `Done`, its line
//!   (`words::act_line`) and its record (`agentwrites::Said`) for the
//!   status line; `undo` puts it back while it is as the rule left it, and
//!   says "not for this one" either way.
//! - **What it would do** (`preview`), over the mail of the last 30 days,
//!   writes nothing and scans nothing; "also the last 30 days"
//!   (`for_already_here`) acts once on each, never where a rule doing the
//!   same acted already.

use crate::agentwrites::{FileChange, Said};
use crate::card::Card;
use crate::config::Config;
use crate::i18n::Translator;
use crate::links::{self, Edge, LocalLinks};
use crate::notice::{self, Notice};
use crate::porch::{Lane, SenderList, Senders, Triaged};
use crate::projects::ProjectStore;
use crate::routetasks::{self, Workdays};
use crate::storage::Places;
use crate::topics::labels;
use crate::vdir::Collection;
use crate::wiring::acts::{self, Act, Acts};
use crate::wiring::declare;
use crate::wiring::message::{self, Seen};
use crate::wiring::store::{self, Model, ModelRef, Param, Rule, Rules};
use crate::wiring::words::{self, Names};
use crate::words::Words;
use jiff::Zoned;
use jiff::tz::TimeZone;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// The line a task a rule made carries: the rule's id, so that its card
/// says "made by the rule …" (an X- property, which a phone's account
/// manager keeps as any).
pub const RULE: &str = "X-SIOUL-RULE";

/// How what a rule made of a message is tied to it in `links.toml` ("Made
/// from" / "Made into"), as a route's task is (`routetasks::MADE`).
pub const MADE: &str = routetasks::MADE;

/// How long "also for the messages already here" and the preview look back.
pub const LOOK_BACK_DAYS: u32 = 30;

/// Mail older than this, when it is fetched (a device offline for weeks, a
/// folder fetched late), is left alone by the rules: they act on what
/// comes, unless asked for the past (seconds).
pub const NEWEST: i64 = 7 * 86_400;

/// The largest file a rule keeps as a paper (bytes).
pub const LARGEST_FILE: usize = 25 << 20;

/// Files a rule keeps only when its "name" names them, whatever their
/// type: pages and pictures that run code, documents with macros.
const NAMED_ONLY: &[&str] = &["html", "htm", "xhtml", "mht", "mhtml", "svg", "svgz", "docm", "dotm", "xlsm", "xltm", "xlsb", "pptm", "potm", "ppsm"];

/// What the antivirus said of a file a rule would keep (sioul-sync's
/// `antivirus::for_rules` answers it on a computer and on a phone alike).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scanned {
    /// Read, and nothing found.
    Clean,
    /// Refused: never kept, and the rule never keeps that message's files;
    /// a paper of these bytes kept elsewhere unchecked is taken out.
    Infected(String),
    /// No antivirus can be asked where Sioul runs (a phone): kept with the
    /// mark `unchecked`, which a device with an antivirus reads later.
    NoAntivirus,
    /// None answered on a computer (none installed, none running, the file
    /// too large for it): kept with the mark `unchecked`, as on a phone.
    Unanswered(String),
}

/// Where the engine writes on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// This device's name in the sharing (sioul-sync's `share::Here::id`).
    pub device: String,
    /// The record of acts' folder (every device's logs).
    pub acts: PathBuf,
    /// `links.toml`.
    pub ties: PathBuf,
    /// The topics' state folder (`topics::labels`).
    pub topics: PathBuf,
}

impl Place {
    /// This device's places.
    pub fn here(device: &str) -> Place {
        Place { device: device.to_string(), acts: acts::folder(), ties: LocalLinks::default_path(), topics: labels::state() }
    }

    /// This device's own log of acts.
    pub fn log(&self) -> PathBuf {
        acts::own_log(&self.acts, &self.device)
    }
}

/// The UID of the task a rule makes of an object: the same on every device.
pub fn task_uid(rule: &str, object: &str) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("rule\n{rule}\n{object}"))
}

/// The id of the paper a rule keeps from an object's file of this digest.
pub fn paper_id(rule: &str, object: &str, digest: &str) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("rule-paper\n{rule}\n{object}\n{digest}"))
}

/// A text's or a file's digest, as an act records it.
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// What a rule does, as a short digest of its "then": two rules doing the
/// same act once on an object between them ("also the last 30 days" after
/// a rule was removed and made again).
pub fn same(rule: &Rule) -> String {
    digest(serde_json::to_string(&rule.then).unwrap_or_default().as_bytes())[..16].to_string()
}

/// A file's name from a message, made safe to keep and unique by its
/// content: its last part only; no control character, none Windows refuses,
/// no leading dot, no device's name; its stem 60 characters at most, its
/// extension in lower case; the first eight characters of its digest after
/// the stem (`bill-9e1c03ab.pdf`), so that two files of one name are two
/// files and two devices name the same bytes alike.
pub fn kept_name(name: &str, sum: &str) -> String {
    let clean = crate::papers::portable_name_of(name);
    let clean = clean.trim_start_matches(['.', '_', ' ']);
    let (stem, extension) = match clean.rsplit_once('.') {
        Some((stem, extension)) if (1..=8).contains(&extension.len()) && extension.chars().all(|c| c.is_ascii_alphanumeric()) => (stem, extension.to_ascii_lowercase()),
        _ => (clean, String::new()),
    };
    let stem: String = stem.chars().filter(|c| !c.is_control()).take(60).collect();
    let stem = stem.trim().trim_end_matches(['.', ' ']);
    let stem = if stem.is_empty() { "attachment" } else { stem };
    let short = &sum[..sum.len().min(8)];
    if extension.is_empty() { format!("{stem}-{short}") } else { format!("{stem}-{short}.{extension}") }
}

/// Whether a file's bytes are what its declared kind says (`rules::KINDS`):
/// a PDF begins as one, a picture as a picture (never SVG, which runs
/// code), an invitation holds a calendar, an archive or an office document
/// is a ZIP or an older office file; the other kinds by their name alone.
pub fn content_agrees(kind: &str, bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(16)];
    let starts = |magic: &[u8]| head.starts_with(magic);
    match kind {
        "pdf" => starts(b"%PDF-"),
        "image" => starts(b"\x89PNG") || starts(b"\xff\xd8\xff") || starts(b"GIF8") || starts(b"II*\0") || starts(b"MM\0*") || (starts(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")) || bytes.get(4..12).is_some_and(|b| b.starts_with(b"ftyp")),
        "calendar" => String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).contains("BEGIN:VCALENDAR"),
        "archive" => starts(b"PK\x03\x04") || starts(b"7z\xbc\xaf") || starts(b"\x1f\x8b") || starts(b"Rar!"),
        "document" => starts(b"PK\x03\x04") || starts(b"\xd0\xcf\x11\xe0") || starts(b"{\\rtf") || std::str::from_utf8(&bytes[..bytes.len().min(4096)]).is_ok(),
        _ => true,
    }
}

/// Whether the Porch keeps a message from every rule: set aside (forged,
/// blocked, a borrowed name, your provider's spam no route protects),
/// hostile, held (a stranger in the screener, a stranger to a public
/// address), what your own spam filter caught, what nothing proves comes
/// from its sender, and what lies in a junk or a trash folder.
pub fn held(judged: &Triaged) -> bool {
    let unproven = judged.trust == crate::trust::Trust::Forged || judged.reasons.contains(&crate::porch::Reason::NotAuthenticated);
    matches!(judged.lane, Lane::SetAside | Lane::Hostile | Lane::Review | Lane::Screener | Lane::Public(_)) || unproven || in_junk_or_trash(&judged.card)
}

/// Whether a message's file lies in its account's junk or trash folder.
fn in_junk_or_trash(card: &Card) -> bool {
    let Some(dir) = card.path.as_deref().and_then(Path::parent).and_then(Path::parent) else { return false };
    let named = card.account.as_deref().and_then(|account| crate::mailfind::folder_named(account, dir));
    let name = named.unwrap_or_else(|| dir.file_name().map(|n| n.to_string_lossy().trim_start_matches('.').to_string()).unwrap_or_default());
    matches!(crate::folders::role_of(&name, None), crate::folders::Role::Junk | crate::folders::Role::Trash)
}

/// When a message reached your provider (its file's time, `maildir::store`),
/// else its Date: what "it came before the rule" is judged by, and what a
/// rule stamps what it makes with, the same on every device.
pub fn arrived_at(card: &Card) -> Option<i64> {
    let file = card.path.as_ref().and_then(|p| std::fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).and_then(|d| i64::try_from(d.as_secs()).ok());
    file.or(card.date)
}

/// The refusals that pass by themselves or by a setting changed: tried again
/// for a week. The others are final: your own word, a message without an
/// identifier, what the rule made itself, a chain at its end, a file refused.
fn retried(why: &str) -> bool {
    !matches!(why, "rules-refused-your-word" | "rules-refused-no-id" | "rules-refused-own" | "rules-refused-chain" | "rules-refused-infected" | "rules-refused-code-told" | "rules-refused-no-attachment" | "rules-refused-too-large")
}

/// The papers this process asked an antivirus about already, by their
/// list's place, id and digest, whatever it answered: a device without one
/// asks once a run.
static SCANNED: std::sync::Mutex<Vec<(PathBuf, String, String)>> = std::sync::Mutex::new(Vec::new());

/// How a pass runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Mail just fetched: what came before a rule, or more than a week
    /// ago, is left alone.
    Arrival,
    /// A pending act tried again: as on arrival, its message found again.
    Retry,
    /// "Also for the messages already here", asked for one rule.
    Asked,
    /// What a rule would do: nothing written.
    Preview,
}

/// One act done, said once with Undo.
#[derive(Debug, Clone, Serialize)]
pub struct Done {
    pub act: Act,
    /// The status line's sentence (`words::act_line`).
    pub line: String,
    /// The record of the act with its Undo: the files it wrote, as they were
    /// before and after (a task's); a tie, a topic, a paper are put back by
    /// `undo`, which the window's Undo calls.
    pub said: Said,
}

/// A rule that matched but did not act, and why (a message id); `waits`:
/// it is tried again (a pending line).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refused {
    pub rule: String,
    pub object: String,
    pub why: &'static str,
    pub waits: bool,
}

/// What a pass did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Outcome {
    pub done: Vec<Done>,
    /// The notices of `tell`, for the caller to post where this device keeps
    /// the notices, as the moment allows.
    #[serde(skip)]
    pub notices: Vec<Notice>,
    /// Refused now: said once per rule by the caller.
    pub refused: Vec<Refused>,
    /// Papers taken out because an antivirus refused their file, by title:
    /// said once by the device that read them.
    pub taken_out: Vec<String>,
    /// What could not be written, said once.
    pub problems: Vec<String>,
}

impl Outcome {
    pub fn is_empty(&self) -> bool {
        self.done.is_empty() && self.notices.is_empty() && self.refused.is_empty() && self.taken_out.is_empty() && self.problems.is_empty()
    }
}

/// What a rule would have done to one message (`preview`).
#[derive(Debug, Clone, Serialize)]
pub struct Would {
    pub rule: String,
    pub object: String,
    pub subject: String,
    /// Its sender, as the message names them.
    pub from: String,
    /// When it came, Unix seconds.
    pub arrived: i64,
    pub act: Act,
    /// In words (`words::act_line`).
    pub line: String,
    /// The rule acted on it already, or was told not to.
    pub done_already: bool,
    /// Why it would not act, a message id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<&'static str>,
}

/// What Undo did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Undone {
    /// Put back, or gone already; false: it changed since and stays as it is.
    pub put_back: bool,
}

/// What one act would write.
#[derive(Debug, Clone, Default)]
struct Doing {
    act: Act,
    files: Vec<FileChange>,
    notice: Option<Notice>,
    ties: Vec<Edge>,
    topic: Option<labels::Act>,
}

/// What a pass gathers before writing.
struct Work {
    ties: LocalLinks,
    new_ties: Vec<Edge>,
    topics: Option<labels::Said>,
    topic_lines: Vec<labels::Act>,
    records: Vec<Act>,
    problems: Vec<String>,
    /// Digests of the papers' files kept here, by paper id, read when first asked.
    papers: Option<Vec<(String, u64, PathBuf)>>,
    taken_out: Vec<String>,
}

/// The engine, with what it reads on this device.
pub struct Engine<'a> {
    pub config: Config,
    pub rules: Rules,
    /// The general models and the projects' (`store::models`).
    pub models: Vec<ModelRef>,
    /// The projects, with the mail tied to them.
    pub store: Option<ProjectStore>,
    /// Where Sioul's lists and the papers' files are.
    pub places: Option<Places>,
    pub senders: Senders,
    pub words: std::sync::Arc<Words>,
    pub sources: Vec<crate::config::Source>,
    /// Every device's acts, as read when the engine was made: once per fetch.
    pub acts: Acts,
    pub place: Place,
    pub workdays: Workdays,
    /// The person's own addresses: what they send is never acted on.
    pub own: Vec<String>,
    /// This device keeps the notices (the "notices" lease): it tells; the
    /// others leave a pending line, told where the lease moves.
    pub tells_here: bool,
    /// The list a model's tasks go into (`routetasks::list_for`).
    pub lists: Box<dyn Fn(&Model) -> Option<Collection> + 'a>,
    /// A list by "account/id", for Undo; every task list, to find a task moved.
    pub find_list: Box<dyn Fn(&str) -> Option<Collection> + 'a>,
    pub all_lists: Box<dyn Fn() -> Vec<Collection> + 'a>,
    /// Whether a task of this UID is known (the window's tasks).
    pub known: Box<dyn Fn(&str) -> bool + 'a>,
    /// A message's topic's id (`topics::rules::Lookup`), asked only when a rule reads it.
    pub topic_of: Box<dyn Fn(&Card) -> Option<String> + 'a>,
    /// A message's file here by its address, for a pending act tried again.
    pub find_mail: Box<dyn Fn(&str) -> Option<PathBuf> + 'a>,
    /// The antivirus, for a file a rule would keep (`Scanned`).
    pub scan: Box<dyn Fn(&Path) -> Scanned + 'a>,
    /// Where files to scan are written first.
    pub scratch: PathBuf,
    pub zone: TimeZone,
    pub now: Zoned,
    /// What could not be read (the rules' list), said once.
    pub problems: Vec<String>,
}

impl Engine<'static> {
    /// The engine as this device has it now: its rules, models, projects,
    /// senders, acts. No antivirus until the caller gives one (`scan`):
    /// without it, a file a rule keeps is marked unchecked. `tells_here`
    /// false until the caller says.
    pub fn here(config: &Config, device: &str, now: &Zoned) -> Engine<'static> {
        let mut problems = Vec::new();
        let rules = Rules::load(&store::path()).unwrap_or_else(|e| {
            problems.push(e);
            Rules::default()
        });
        let ties = LocalLinks::load(&LocalLinks::default_path());
        let places = config.places();
        let store = places.as_ref().and_then(|p| ProjectStore::open(p).ok()).map(|s| s.with_ties(&ties));
        Engine::of(config, device, now, rules, store, Senders::load(config), problems)
    }
}

impl<'a> Engine<'a> {
    /// The engine from what the caller read already.
    pub fn of(config: &Config, device: &str, now: &Zoned, rules: Rules, store: Option<ProjectStore>, senders: Senders, problems: Vec<String>) -> Engine<'a> {
        let models = store::models(&rules, store.as_ref());
        let sources = config.mail_sources();
        let own = crate::porch::own_addresses(&sources);
        let for_lists = config.clone();
        let for_topics = config.clone();
        let for_mail = config.clone();
        let lookup: RefCell<Option<crate::topics::rules::Lookup>> = RefCell::new(None);
        Engine {
            models,
            places: config.places(),
            words: Words::of(config),
            sources,
            acts: Acts::read(&acts::folder()),
            place: Place::here(device),
            workdays: Workdays::of(config),
            own,
            tells_here: false,
            lists: Box::new(move |model: &Model| routetasks::list_for(&model.as_project_model(), &for_lists)),
            find_list: Box::new(|id: &str| crate::tasks::lists().into_iter().find(|c| format!("{}/{}", c.account, c.id) == id)),
            all_lists: Box::new(crate::tasks::lists),
            known: Box::new(|_: &str| false),
            topic_of: Box::new(move |card: &Card| {
                let mut kept = lookup.borrow_mut();
                let lookup = kept.get_or_insert_with(|| crate::topics::rules::Lookup::load(&for_topics));
                let folder = card.path.as_ref().and_then(|p| p.parent()?.parent()?.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                lookup.tagged(card, card.account.as_deref().unwrap_or_default(), &folder).map(|(topic, _)| topic.id.clone())
            }),
            find_mail: Box::new(move |address: &str| match crate::mailfind::find(&crate::mailindex::MailIndex::roots_of(&for_mail), &crate::mailfind::MailRef::of(address)) {
                crate::mailfind::Found::Here(here) => Some(here.path),
                _ => None,
            }),
            scan: Box::new(|_: &Path| Scanned::Unanswered("no antivirus given".to_string())),
            scratch: crate::config::cache_dir().join("attachments").join("rules"),
            zone: now.time_zone().clone(),
            now: now.clone(),
            config: config.clone(),
            rules,
            store,
            senders,
            problems,
        }
    }

    /// Messages' files as the Porch judges them (with your spam filter's verdict).
    pub fn judge(&self, files: &[PathBuf]) -> Vec<Triaged> {
        crate::porch::judge(files, &self.sources, self.store.as_ref(), &SenderList::default(), &self.senders, self.now.timestamp().as_second())
    }

    /// The rules that run, for mail just fetched: first what was undone
    /// elsewhere taken back, the pending acts tried again and the papers
    /// kept unchecked read by this device's antivirus; then nothing at an
    /// address's first fetch (`first`), what each rule takes from its
    /// `since` on and within the week; each act written, its record kept,
    /// said in the outcome.
    pub fn on_arrival(&self, judged: &[Triaged], first: bool, tr: &Translator) -> Outcome {
        let mut out = self.settle(tr);
        if first || self.rules.read_only() {
            return out;
        }
        let rules: Vec<&Rule> = self.rules.running().filter(|r| r.when.kind == "message" && r.when.event == "arrives").collect();
        let pass = self.pass(&rules, judged, Mode::Arrival, tr).0;
        out.absorb(pass);
        out
    }

    /// What every pass begins with, and what the caller runs when the
    /// rules' record or the papers arrive from another device: what this
    /// device wrote of pairs undone elsewhere taken back; the pending acts
    /// of the rules that run tried again; the papers kept unchecked read by
    /// this device's antivirus, those it refuses taken out.
    pub fn settle(&self, tr: &Translator) -> Outcome {
        let mut out = Outcome { problems: self.problems.clone(), ..Outcome::default() };
        if let Err(e) = self.take_back() {
            out.problems.push(e);
        }
        out.taken_out.extend(self.scan_unchecked());
        if !self.rules.read_only() {
            out.absorb(self.retry(tr));
        }
        out
    }

    /// What this device wrote of every pair undone or said "not for this
    /// one" on any device, put back here while it is as the rule left it (a
    /// task, a tie, a topic's word, a paper): Undo is a state every device
    /// comes to. What was put back.
    pub fn take_back(&self) -> Result<Vec<Act>, String> {
        let me = acts::device_name(&self.place.device);
        let mut back = Vec::new();
        for act in self.acts.to_take_back(&me) {
            if put_back(act, &self.place, self.places.as_ref(), &*self.find_list, &*self.all_lists, self.now.timestamp().as_millisecond())? {
                back.push(act.clone());
            }
        }
        Ok(back)
    }

    /// The pending acts of the rules that run, younger than a week, their
    /// messages found again here, tried again.
    fn retry(&self, tr: &Translator) -> Outcome {
        let now_ms = self.now.timestamp().as_millisecond();
        let pending = self.acts.pending(now_ms);
        let mut out = Outcome::default();
        for act in pending {
            let Some(rule) = self.rules.running().find(|r| r.id == act.rule) else { continue };
            let Some(file) = (self.find_mail)(&act.object) else { continue };
            let judged = self.judge(&[file]);
            out.absorb(self.pass(&[rule], &judged, Mode::Retry, tr).0);
        }
        out
    }

    /// "Also for the messages already here": one rule that runs, on the
    /// messages handed (the last 30 days: `recent_files`), each once; what
    /// came before the rule taken too, since it is asked; never where a rule
    /// doing the same acted already. Nothing told.
    pub fn for_already_here(&self, rule: &str, judged: &[Triaged], tr: &Translator) -> Outcome {
        let Some(rule) = self.rules.running().find(|r| r.id == rule) else { return Outcome { problems: self.problems.clone(), ..Outcome::default() } };
        self.pass(&[rule], judged, Mode::Asked, tr).0
    }

    /// What `rule` (saved or not, on or off) would do to the messages
    /// handed: nothing written, nothing told, no file scanned.
    pub fn preview(&self, rule: &Rule, judged: &[Triaged], tr: &Translator) -> Vec<Would> {
        let mut rule = rule.clone();
        rule.on = true;
        if rule.problem().is_some() {
            return Vec::new();
        }
        self.pass(&[&rule], judged, Mode::Preview, tr).1
    }

    /// The names an act's sentence gives, with the subjects of the messages at hand.
    fn names(&self, subjects: &[(String, String)]) -> Names {
        let mut names = Names::default();
        for project in self.store.as_ref().map(|s| s.projects.as_slice()).unwrap_or_default() {
            names.projects.insert(project.id.clone(), project.title.clone());
        }
        for topic in &self.config.topics {
            names.topics.insert(topic.id.clone(), topic.name.clone());
        }
        for model in &self.models {
            names.models.insert(model.id.clone(), model.model.name.clone());
        }
        names.subjects.extend(subjects.iter().cloned());
        names
    }

    /// The instant a message's arrival stamps what a rule makes of it with,
    /// in this device's zone; now for a message with no time at all.
    fn stamp(&self, arrived: Option<i64>) -> Zoned {
        arrived.and_then(|at| jiff::Timestamp::from_second(at).ok()).map_or_else(|| self.now.clone(), |t| t.to_zoned(self.zone.clone()))
    }

    /// A subject as a rule may copy it: none when the message holds a code
    /// (a sign-in link, a reset…), the numbers a mask hides hidden.
    fn shown_subject(seen: &Seen) -> Option<String> {
        let coded = seen.kinds.iter().any(|k| matches!(*k, "code" | "password" | "password-reset" | "sign-in-link" | "confirmation"));
        (!coded).then(|| crate::masking::text(seen.card().subject.trim()))
    }

    fn pass(&self, rules: &[&Rule], judged: &[Triaged], mode: Mode, tr: &Translator) -> (Outcome, Vec<Would>) {
        let mut out = Outcome::default();
        // What a preview would do, gathered before its words are made.
        let mut would: Vec<(String, String, String, String, i64, Act, bool, Option<&'static str>)> = Vec::new();
        if rules.is_empty() {
            return (out, Vec::new());
        }
        let mut work = Work { ties: LocalLinks::load(&self.place.ties), new_ties: Vec::new(), topics: None, topic_lines: Vec::new(), records: Vec::new(), problems: Vec::new(), papers: None, taken_out: Vec::new() };
        let needs_topic = rules.iter().any(|r| r.when.conditions.iter().any(|c| c.field == "topic"));
        let sames: Vec<String> = rules.iter().map(|r| same(r)).collect();
        let mut subjects: Vec<(String, String)> = Vec::new();
        let mut done: Vec<(Act, Vec<FileChange>)> = Vec::new();
        let now = self.now.timestamp().as_second();
        for t in judged {
            if held(t) {
                continue;
            }
            let Some((_, address)) = routetasks::message_of(&t.card) else { continue };
            if t.card.from_address.as_deref().is_some_and(|from| self.own.iter().any(|own| own.eq_ignore_ascii_case(from.trim()))) {
                continue;
            }
            let arrived = arrived_at(&t.card);
            if mode == Mode::Arrival && arrived.is_none_or(|at| at < now - NEWEST) {
                continue;
            }
            let mut seen: Option<Seen> = None;
            for (rule, rule_same) in rules.iter().zip(&sames) {
                if mode == Mode::Arrival && arrived.is_none_or(|at| at < rule.since) {
                    continue;
                }
                let before = self.acts.touched(&rule.id, &address) || work.records.iter().any(|a| a.rule == rule.id && a.object == address && a.pending.is_empty());
                if before && mode != Mode::Preview {
                    continue;
                }
                if mode == Mode::Asked && self.acts.same_done(&address, rule_same) {
                    continue;
                }
                let seen = seen.get_or_insert_with(|| {
                    let topic = if needs_topic { (self.topic_of)(&t.card) } else { None };
                    message::seen(t, &self.senders, self.store.as_ref(), &self.words, topic)
                });
                if !message::holds(&rule.when, seen) {
                    continue;
                }
                if self.acts.made_by(&address, &rule.id) {
                    out.refused.push(Refused { rule: rule.id.clone(), object: address.clone(), why: "rules-refused-own", waits: false });
                    continue;
                }
                let depth = self.acts.depth_of(&address);
                if depth >= declare::CHAIN {
                    out.refused.push(Refused { rule: rule.id.clone(), object: address.clone(), why: "rules-refused-chain", waits: false });
                    continue;
                }
                if subjects.iter().all(|(a, _)| *a != address) {
                    subjects.push((address.clone(), Self::shown_subject(seen).unwrap_or_default()));
                }
                let base = Act { at: self.now.timestamp().as_millisecond(), rule: rule.id.clone(), object: address.clone(), depth: depth + 1, same: rule_same.clone(), ..Act::default() };
                match self.act(rule, seen, base.clone(), arrived, mode, &mut work, tr) {
                    Ok(doings) => {
                        for doing in doings {
                            if mode == Mode::Preview {
                                would.push((rule.id.clone(), address.clone(), t.card.subject.clone(), t.card.from_name.clone().or_else(|| t.card.from_address.clone()).unwrap_or_default(), arrived.unwrap_or(0), doing.act, before, None));
                                continue;
                            }
                            work.new_ties.extend(doing.ties.iter().filter(|e| !work.ties.links.contains(e)).cloned());
                            if let Some(line) = doing.topic {
                                work.topic_lines.push(line);
                            }
                            if let Some(notice) = doing.notice {
                                out.notices.push(notice);
                            }
                            work.records.push(doing.act.clone());
                            done.push((doing.act, doing.files));
                        }
                    }
                    Err(why) if mode == Mode::Preview => {
                        let act = Act { act: rule.then.action.clone(), ..base };
                        would.push((rule.id.clone(), address.clone(), t.card.subject.clone(), t.card.from_address.clone().unwrap_or_default(), arrived.unwrap_or(0), act, before, Some(why)));
                    }
                    Err(why) => {
                        let waits = retried(why);
                        // Said once: a pending line waits for the pair, tried again at each pass for a week.
                        if waits && !self.acts.is_pending(&rule.id, &address) {
                            work.records.push(Act { act: rule.then.action.clone(), pending: why.to_string(), ..base });
                        }
                        out.refused.push(Refused { rule: rule.id.clone(), object: address.clone(), why, waits });
                    }
                }
            }
        }
        let names = self.names(&subjects);
        if mode == Mode::Preview {
            let would = would
                .into_iter()
                .map(|(rule, object, subject, from, arrived, act, done_already, refused)| Would { line: words::act_line(&act, &names, tr), rule, object, subject, from, arrived, act, done_already, refused })
                .collect();
            return (out, would);
        }
        // The ties and the topics' words written once for the whole pass; the
        // acts whose writing failed are not recorded, and will be done again.
        if !work.new_ties.is_empty()
            && let Err(e) = LocalLinks::add_all(&self.place.ties, &work.new_ties)
        {
            work.problems.push(e);
            work.records.retain(|a| a.act != "tie" || !a.pending.is_empty());
            done.retain(|(a, _)| a.act != "tie");
        }
        if !work.topic_lines.is_empty()
            && let Err(e) = labels::append(&labels::own_log(&self.place.topics, &self.place.device), &work.topic_lines)
        {
            work.problems.push(e);
            work.records.retain(|a| a.act != "topic" || !a.pending.is_empty());
            done.retain(|(a, _)| a.act != "topic");
        }
        if let Err(e) = acts::append(&self.place.log(), &work.records) {
            work.problems.push(e);
        }
        let at = self.now.timestamp().as_millisecond();
        out.done = done
            .into_iter()
            .filter(|(act, _)| !act.undone && act.pending.is_empty())
            .map(|(act, files)| {
                let line = words::act_line(&act, &names, tr);
                Done { said: Said { at, line: line.clone(), files }, line, act }
            })
            .collect();
        out.taken_out.extend(work.taken_out);
        out.problems.extend(work.problems);
        (out, Vec::new())
    }

    /// The fixed text of a parameter; none when it is not given, empty, or taken from the object.
    fn fixed(rule: &Rule, key: &str) -> Option<String> {
        rule.then.with.get(key).and_then(Param::text).filter(|t| !t.is_empty())
    }

    /// The project what a rule makes goes in: the one the rule names; else
    /// the message's, so that it is kept from agents as its message is; else
    /// its model's.
    fn project_for(&self, rule: &Rule, seen: &Seen, model: Option<&Model>) -> Result<Option<String>, &'static str> {
        match rule.then.with.get("project") {
            Some(Param::From { .. }) | None => Ok(seen.projects.first().cloned().or_else(|| model.map(|m| m.project.clone()).filter(|p| !p.is_empty()))),
            Some(given) => match given.text().filter(|t| !t.is_empty()) {
                Some(id) => self.store.as_ref().and_then(|s| s.get(&id)).map(|p| Some(p.id.clone())).ok_or("rules-refused-no-project"),
                None => Ok(seen.projects.first().cloned()),
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn act(&self, rule: &Rule, seen: &Seen, base: Act, arrived: Option<i64>, mode: Mode, work: &mut Work, tr: &Translator) -> Result<Vec<Doing>, &'static str> {
        let kind = declare::of(&rule.then.kind).ok_or("rules-problem-no-action")?;
        let action = kind.action(&rule.then.action).ok_or("rules-problem-no-action")?;
        if let Some(why) = declare::forbidden(action.writes) {
            return Err(why);
        }
        if !declare::available(action.phase) {
            return Err("rules-problem-coming");
        }
        match (kind.id, action.id) {
            ("message", "tie-to-project") => self.tie(rule, seen, base, work),
            ("message", "give-topic") => self.topic(rule, seen, base, arrived, work),
            ("message", "keep-attachments") => self.papers(rule, seen, base, arrived, mode, work, tr),
            ("task", "make") => self.task(rule, seen, base, arrived, mode, work),
            ("none", "tell") => self.tell(rule, seen, base, mode, tr),
            _ => Err("rules-problem-coming"),
        }
    }

    /// Tie it to a project: the tie the routes write (`how = "project"`),
    /// found when it is there already.
    fn tie(&self, rule: &Rule, seen: &Seen, base: Act, work: &Work) -> Result<Vec<Doing>, &'static str> {
        let id = match rule.then.with.get("project") {
            Some(Param::From { .. }) => seen.projects.first().cloned(),
            _ => Self::fixed(rule, "project"),
        }
        .ok_or("rules-refused-no-project")?;
        let project = self.store.as_ref().and_then(|s| s.get(&id)).map(|p| p.id.clone()).ok_or("rules-refused-no-project")?;
        let to = links::project_uri(&project);
        let object = base.object.clone();
        let there = work.ties.links.iter().chain(&work.new_ties).any(|e| e.how == "project" && ((e.from == object && e.to == to) || (e.to == object && e.from == to)));
        let ties = if there { Vec::new() } else { vec![Edge { from: object, to: to.clone(), how: "project".into() }] };
        Ok(vec![Doing { act: Act { act: "tie".into(), result: to, wrote: !there, ..base }, ties, ..Doing::default() }])
    }

    /// Give it a topic: a word on the message in this device's topics' log,
    /// stamped with its arrival, never over a word of yours on it.
    fn topic(&self, rule: &Rule, seen: &Seen, base: Act, arrived: Option<i64>, work: &mut Work) -> Result<Vec<Doing>, &'static str> {
        let topic = Self::fixed(rule, "topic").ok_or("rules-refused-no-topic")?;
        if !self.config.topics.iter().any(|t| t.id == topic) {
            return Err("rules-refused-no-topic");
        }
        let id = seen.card().message_id.as_deref().map(crate::mailindex::bare_id).filter(|id| !id.is_empty()).ok_or("rules-refused-no-id")?;
        let root = self.place.topics.clone();
        let said = work.topics.get_or_insert_with(|| labels::Said::of(&labels::read_all_in(&root)));
        let pending = work.topic_lines.iter().any(|line| matches!(line, labels::Act::Message { message_id, .. } if *message_id == id));
        if said.message(&id).is_some() || pending {
            return Err("rules-refused-your-word");
        }
        let line = labels::Act::Message { at: self.stamp(arrived).timestamp().as_second(), message_id: id, topic: Some(topic.clone()), forget: false };
        Ok(vec![Doing { act: Act { act: "topic".into(), result: topic, wrote: true, ..base }, topic: Some(line), ..Doing::default() }])
    }

    /// Make a task from a model: in the model's list (else the usual one),
    /// in the project the rule names, else the message's; its UID from the
    /// rule and the message, its stamps from the message's arrival, so that
    /// two devices write the same bytes; tied to its message as made; found
    /// when another device made it, or it is known, or its tie is there.
    fn task(&self, rule: &Rule, seen: &Seen, base: Act, arrived: Option<i64>, mode: Mode, work: &mut Work) -> Result<Vec<Doing>, &'static str> {
        let id = Self::fixed(rule, "model").ok_or("rules-refused-no-model")?;
        let model = store::model(&self.models, &id).map(|m| m.model.clone()).ok_or("rules-refused-no-model")?;
        let project = self.project_for(rule, seen, Some(&model))?;
        let subject = Self::shown_subject(seen);
        let title = match (rule.then.with.get("title"), &subject) {
            (Some(Param::Text(text)), _) if !text.trim().is_empty() => format!("{} {}", model.prefix.trim(), crate::masking::text(text.trim())).trim().to_string(),
            (_, Some(subject)) => {
                let words: Vec<String> = rule.when.conditions.iter().filter(|c| c.field == "subject" && c.test == "has-words").flat_map(|c| c.value.clone()).collect();
                routetasks::title(subject, &words, &model.prefix, &model.name)
            }
            // A message holding a code gives its title nothing of its subject.
            (_, None) => format!("{} {}", model.prefix.trim(), model.name.trim()).trim().to_string(),
        };
        let days = match rule.then.with.get("days") {
            Some(Param::Number(n)) => u32::try_from(*n).ok(),
            Some(Param::Text(t)) => t.trim().parse().ok(),
            _ => None,
        }
        .or(model.business_days)
        .unwrap_or(routetasks::UNSAID_DAYS);
        let list = (self.lists)(&model).ok_or("rules-refused-no-list")?;
        if list.read_only {
            return Err("rules-refused-read-only");
        }
        let uid = task_uid(&rule.id, &base.object);
        let task = links::task_uri(&uid);
        let path = list.dir.join(format!("{uid}.ics"));
        let tied = work.ties.links.iter().chain(&work.new_ties).any(|e| e.how == MADE && (e.from == task || e.to == task));
        let found = (self.known)(&uid) || path.exists() || tied;
        let stamp = self.stamp(arrived);
        let due = self.workdays.date_asked(stamp.date(), days);
        let ties = if tied { Vec::new() } else { vec![Edge { from: task.clone(), to: base.object.clone(), how: MADE.into() }] };
        let mut doing = Doing { act: Act { act: "task".into(), result: task, wrote: !found, list: format!("{}/{}", list.account, list.id), ..base }, ties, ..Doing::default() };
        if !found {
            let edit = crate::tasks::TaskEdit { title, due: due.to_string(), estimate: model.estimate.unwrap_or(0), projects: project.into_iter().collect(), supplies: model.supplies.clone(), ..crate::tasks::TaskEdit::default() };
            let text = crate::tasks::new_task(&edit, &uid, &self.zone, &stamp)
                .and_then(|text| crate::tasks::add_lines(&text, &[format!("{}:{}", routetasks::ORIGIN, crate::lines::escape(&doing.act.object)), format!("{RULE}:{}", crate::lines::escape(&rule.id))], &stamp))
                .map_err(|e| {
                    work.problems.push(e);
                    "rules-refused-write"
                })?;
            doing.act.digest = digest(text.as_bytes());
            if mode != Mode::Preview {
                crate::vdir::write_item(&path, &text).map_err(|e| {
                    work.problems.push(e);
                    "rules-refused-write"
                })?;
            }
            doing.files.push(FileChange { path, before: None, after: Some(text) });
        }
        Ok(vec![doing])
    }

    /// Tell: a notice to know about the message, as the moment allows, from
    /// the device that keeps the notices; another device leaves a pending
    /// line, told where the lease moves within the week; quietly, said and
    /// kept only. Never for mail already here, nor for a code, which its own
    /// notice tells.
    fn tell(&self, rule: &Rule, seen: &Seen, base: Act, mode: Mode, tr: &Translator) -> Result<Vec<Doing>, &'static str> {
        let level = Self::fixed(rule, "level").unwrap_or_else(|| "now".to_string());
        if !declare::LEVELS.iter().any(|l| l.id == level) {
            return Err("rules-problem-value");
        }
        if mode == Mode::Asked {
            return Ok(Vec::new());
        }
        // A message holding a code, a sign-in link, a reset: its own notice tells it.
        if Self::shown_subject(seen).is_none() {
            return Err("rules-refused-code-told");
        }
        if matches!(mode, Mode::Arrival | Mode::Retry) && level == "now" && !self.tells_here {
            return Err("rules-refused-elsewhere");
        }
        let notice = (level == "now").then(|| {
            let card = seen.card();
            let from = card.from_name.clone().filter(|n| !n.trim().is_empty() && seen.proven()).or_else(|| card.from_address.clone()).unwrap_or_default();
            let title = Self::fixed(rule, "words").map(|w| crate::masking::text(&w)).unwrap_or_else(|| tr.text("rules-tell-title", None));
            let mut args = crate::i18n::args();
            args.set("from", from);
            let subject = Self::shown_subject(seen);
            args.set("subject", subject.clone().unwrap_or_default());
            let body = tr.text(if subject.is_none() { "rules-tell-body-code" } else { "rules-tell-body" }, Some(&args));
            let hint = card.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            Notice::new(base.object.clone(), notice::Kind::Minor, title, body).hint(hint.clone()).action(tr.text("notify-open-message", None), notice::Act::Open { address: base.object.clone(), hint })
        });
        Ok(vec![Doing { act: Act { act: "tell".into(), result: level.clone(), wrote: level == "now", ..base }, notice, ..Doing::default() }])
    }

    /// Keep its attachments in Papers: the files the rule names (PDF unless
    /// it says another kind or any; whose name holds its words), each whose
    /// bytes are what its kind says, no larger than `LARGEST_FILE`, never a
    /// program, never a page, an SVG or a document with macros unless its
    /// "name" names it; each read by the antivirus first: one refused keeps
    /// none of the message's and the rule never keeps them since, and a
    /// paper of those bytes kept elsewhere unchecked is taken out; without
    /// an antivirus, kept with the mark `unchecked`. Each kept once, under a
    /// name carrying its digest, by a paper id from the rule, the message
    /// and its digest; a file kept as a paper already (by a rule, by hand)
    /// gets the tie instead, and is read by the antivirus if it never was.
    /// The message itself is never changed.
    #[allow(clippy::too_many_arguments)]
    fn papers(&self, rule: &Rule, seen: &Seen, base: Act, arrived: Option<i64>, mode: Mode, work: &mut Work, tr: &Translator) -> Result<Vec<Doing>, &'static str> {
        let places = self.places.as_ref().ok_or("rules-refused-no-papers")?;
        let card = seen.card();
        let raw = card.path.as_ref().and_then(|p| std::fs::read(p).ok()).ok_or("rules-refused-no-attachment")?;
        let wanted = Self::fixed(rule, "type").unwrap_or_else(|| "pdf".to_string());
        let named = Self::fixed(rule, "name").map(|n| crate::rules::fold(&n)).filter(|n| !n.is_empty());
        let names_it = |name: &str| named.as_ref().is_some_and(|n| crate::rules::fold(name).contains(n.as_str()));
        let extension = |name: &str| name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        let files: Vec<(String, Vec<u8>)> = attachments(&raw)
            .into_iter()
            .filter(|(name, _, _)| !links::is_program(name) && !links::is_phone_program(name))
            .filter(|(name, _, _)| named.is_none() || names_it(name))
            .filter(|(name, _, bytes)| names_it(name) || (!NAMED_ONLY.contains(&extension(name).as_str()) && !bytes.trim_ascii_start().starts_with(b"<")))
            .filter(|(name, mime, bytes)| {
                let kind = crate::rules::attachment_kind(name, mime);
                (wanted == "any" || kind == wanted) && content_agrees(kind, bytes)
            })
            .map(|(name, _, bytes)| (name, bytes))
            .collect();
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let files: Vec<(String, Vec<u8>)> = files.into_iter().filter(|(_, bytes)| bytes.len() <= LARGEST_FILE).collect();
        if files.is_empty() {
            return Err("rules-refused-too-large");
        }
        let message = base.object.clone();
        let made_tie = |paper: &str| Edge { from: links::paper_uri(paper), to: message.clone(), how: MADE.into() };
        let mut wallet = crate::papers::Wallet::load_at(places).map_err(|e| {
            work.problems.push(e);
            "rules-refused-no-papers"
        })?;
        let mut doings = Vec::new();
        // Each file: kept already (and whether it was ever read by an antivirus), or new.
        let mut found: Vec<(String, Vec<u8>, String, Option<String>)> = Vec::new();
        for (name, bytes) in files {
            let sum = digest(&bytes);
            let id = paper_id(&rule.id, &message, &sum);
            let kept = wallet.get(&id).map(|p| p.id.clone()).or_else(|| self.paper_with(&wallet, &sum, bytes.len() as u64, work));
            found.push((name, bytes, sum, kept));
        }
        if mode == Mode::Preview {
            for (_, _, sum, kept) in found {
                let id = kept.clone().unwrap_or_else(|| paper_id(&rule.id, &message, &sum));
                doings.push(Doing { act: Act { act: "paper".into(), result: links::paper_uri(&id), wrote: kept.is_none(), digest: sum, ..base.clone() }, ..Doing::default() });
            }
            return Ok(doings);
        }
        // Every file read by the antivirus before any is kept, but those kept
        // already and read by one: one refused keeps the message's files out,
        // for good, and takes out a paper of its bytes kept unchecked elsewhere.
        let folder = self.scratch.join(base.object.chars().filter(char::is_ascii_alphanumeric).take(24).collect::<String>());
        let mut scratch: Vec<PathBuf> = Vec::new();
        let clear = |paths: &[PathBuf]| {
            for path in paths {
                let _ = crate::filelock::remove_wiping(path);
            }
        };
        let mut unchecked: Vec<bool> = Vec::new();
        for (name, bytes, sum, kept) in &found {
            let read_before = kept.as_ref().is_some_and(|id| wallet.get(id).is_some_and(|p| !p.unchecked));
            if read_before {
                unchecked.push(false);
                continue;
            }
            let file = folder.join(kept_name(name, sum));
            if let Err(e) = std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&file, bytes)) {
                work.problems.push(format!("{}: {e}", file.display()));
                clear(&scratch);
                return Err("rules-refused-write");
            }
            scratch.push(file.clone());
            match (self.scan)(&file) {
                Scanned::Clean => unchecked.push(false),
                Scanned::NoAntivirus | Scanned::Unanswered(_) => unchecked.push(true),
                Scanned::Infected(_) => {
                    clear(&scratch);
                    // Its bytes kept unchecked as a paper elsewhere: taken out, on every device.
                    for (_, _, _, kept) in &found {
                        if let Some(id) = kept.as_ref().filter(|id| wallet.get(id).is_some_and(|p| p.unchecked)) {
                            work.taken_out.extend(take_out(places, &self.place, id).ok().flatten());
                            work.papers = None;
                        }
                    }
                    work.records.push(Act { act: "paper".into(), undone: true, ..base.clone() });
                    return Err("rules-refused-infected");
                }
            }
        }
        let movement = self.movement_of(places, &message);
        let person = Self::fixed(rule, "for").filter(|p| p.as_str() != crate::persons::ME);
        let stamp = self.stamp(arrived);
        for ((name, bytes, sum, kept), unchecked) in found.iter().zip(unchecked) {
            if let Some(id) = kept {
                // Kept already: tied to this message; read clean here now, its mark taken off.
                if !unchecked && wallet.get(id).is_some_and(|p| p.unchecked) {
                    let id = id.clone();
                    let _ = crate::papers::Wallet::change_at(places, |w| {
                        if let Some(mut paper) = w.get(&id).cloned() {
                            paper.unchecked = false;
                            w.put(paper);
                        }
                    });
                }
                let tie = made_tie(id);
                let there = work.ties.links.contains(&tie) || work.new_ties.contains(&tie);
                doings.push(Doing { act: Act { act: "paper".into(), result: links::paper_uri(id), wrote: false, digest: sum.clone(), ..base.clone() }, ties: if there { Vec::new() } else { vec![tie] }, ..Doing::default() });
                continue;
            }
            let id = paper_id(&rule.id, &message, sum);
            let title = self.paper_title(rule, seen, name, arrived, tr);
            let kind = Self::fixed(rule, "kind").map(|k| crate::papers::Kind::of(&k)).or_else(|| crate::papers::Kind::guess(&self.words, name)).or_else(|| Self::shown_subject(seen).and_then(|s| crate::papers::Kind::guess(&self.words, &s))).unwrap_or_default();
            let file = folder.join(kept_name(name, sum));
            if !file.exists() && std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&file, bytes)).is_err() {
                continue;
            }
            scratch.push(file.clone());
            let kept = keep_named(&wallet, &file, name, sum).and_then(|path| {
                let holder = person.as_ref().map(|p| {
                    let persons = crate::persons::Persons::load(&crate::persons::Persons::default_path());
                    crate::persons::holder_text(&persons, std::slice::from_ref(p), &[], "")
                });
                let paper = crate::papers::Paper { id: id.clone(), kind, title, file: path, holder: holder.unwrap_or_default(), added: Some(stamp.date()), unchecked, ..crate::papers::Paper::default() };
                crate::papers::Wallet::change_at(places, |w| w.put(paper))
            });
            if let Err(e) = kept {
                work.problems.push(e);
                continue;
            }
            if let Ok(again) = crate::papers::Wallet::load_at(places) {
                wallet = again;
            }
            // The papers' files read again when next asked: this one is among them now.
            work.papers = None;
            let mut ties = vec![made_tie(&id)];
            if let Some(movement) = &movement {
                ties.push(Edge { from: links::paper_uri(&id), to: links::movement_uri(movement), how: "link".into() });
            }
            if let Some(person) = &person {
                let path = self.place.ties.clone();
                let _ = LocalLinks::change(&path, |local| crate::persons::tie_paper(local, &id, std::slice::from_ref(person)));
            }
            doings.push(Doing { act: Act { act: "paper".into(), result: links::paper_uri(&id), wrote: true, digest: sum.clone(), ..base.clone() }, ties, ..Doing::default() });
        }
        clear(&scratch);
        let _ = std::fs::remove_dir(&folder);
        Ok(doings)
    }

    /// The papers kept unchecked whose file is here, read by this device's
    /// antivirus once a run: clean, their mark taken off; refused, taken out
    /// of the wallet with their file, on every device (the wallet and the
    /// papers' folder travel). Their titles, to say once.
    pub fn scan_unchecked(&self) -> Vec<String> {
        let Some(places) = self.places.as_ref() else { return Vec::new() };
        let Ok(wallet) = crate::papers::Wallet::load_at(places) else { return Vec::new() };
        let mut out = Vec::new();
        for paper in wallet.papers.iter().filter(|p| p.unchecked) {
            let Some(file) = wallet.file_path(paper).filter(|f| f.is_file()) else { continue };
            let Ok(bytes) = std::fs::read(&file) else { continue };
            let key = (places.records.clone(), paper.id.clone(), digest(&bytes));
            {
                let Ok(mut asked) = SCANNED.lock() else { continue };
                if asked.contains(&key) {
                    continue;
                }
                asked.push(key);
            }
            match (self.scan)(&file) {
                Scanned::Clean => {
                    let id = paper.id.clone();
                    let _ = crate::papers::Wallet::change_at(places, |w| {
                        if let Some(mut paper) = w.get(&id).cloned() {
                            paper.unchecked = false;
                            w.put(paper);
                        }
                    });
                }
                Scanned::Infected(_) => out.extend(take_out(places, &self.place, &paper.id).ok().flatten()),
                Scanned::NoAntivirus | Scanned::Unanswered(_) => {}
            }
        }
        out
    }

    /// The paper whose file holds these bytes (its digest), among those kept here.
    fn paper_with(&self, wallet: &crate::papers::Wallet, sum: &str, size: u64, work: &mut Work) -> Option<String> {
        let files = work.papers.get_or_insert_with(|| wallet.papers.iter().filter_map(|p| wallet.file_path(p).map(|path| (p.id.clone(), std::fs::metadata(&path).map_or(0, |m| m.len()), path))).collect());
        files.iter().filter(|(_, len, _)| *len == size).find(|(_, _, path)| std::fs::read(path).is_ok_and(|bytes| digest(&bytes) == sum)).map(|(id, _, _)| id.clone())
    }

    /// The movement the money reader joined this message to, when it did.
    fn movement_of(&self, places: &Places, message: &str) -> Option<String> {
        let store = crate::movements::Store::load(&places.records).ok()?;
        store.links.iter().find(|l| l.source == message).map(|l| l.movement.clone())
    }

    /// A paper's title: the rule's pattern with its inserts ({sender}: the
    /// sender's card's name, else the name it gives, else its address;
    /// {month}: the month and year it came; {year}; {subject}, nothing for a
    /// message holding a code; {file}: the file's name without its
    /// extension); without one, as "Keep in papers" guesses it, from the
    /// file's name. The numbers a mask hides, hidden.
    fn paper_title(&self, rule: &Rule, seen: &Seen, name: &str, arrived: Option<i64>, tr: &Translator) -> String {
        let card = seen.card();
        let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().replace(['_', '-'], " ")).unwrap_or_default();
        let subject = Self::shown_subject(seen).unwrap_or_default();
        let Some(pattern) = Self::fixed(rule, "title") else {
            return crate::masking::text(if stem.trim().is_empty() { &subject } else { &stem });
        };
        let sender = seen
            .cards
            .first()
            .and_then(|uid| self.senders.contacts.find(uid))
            .and_then(|i| self.senders.contacts.card(i))
            .map(|c| c.name.clone())
            .filter(|n| !n.trim().is_empty())
            .or_else(|| card.from_name.clone().filter(|n| !n.trim().is_empty() && seen.proven()))
            .or_else(|| card.from_address.clone())
            .unwrap_or_default();
        let day = self.stamp(arrived).date();
        let filled = pattern.replace("{sender}", sender.trim()).replace("{month}", &tr.month_year(day)).replace("{year}", &day.year().to_string()).replace("{subject}", subject.trim()).replace("{file}", stem.trim());
        let filled = filled.split_whitespace().collect::<Vec<_>>().join(" ");
        crate::masking::text(if filled.is_empty() { &stem } else { &filled })
    }
}

impl Outcome {
    /// Another pass's outcome added to this one.
    fn absorb(&mut self, other: Outcome) {
        self.done.extend(other.done);
        self.notices.extend(other.notices);
        self.refused.extend(other.refused);
        self.taken_out.extend(other.taken_out);
        for problem in other.problems {
            if !self.problems.contains(&problem) {
                self.problems.push(problem);
            }
        }
    }
}

/// A paper taken out of the wallet with its file (no other paper naming
/// it), on every device: its title. The act that made it, if a rule did,
/// stays recorded; a line says why it went.
fn take_out(places: &Places, place: &Place, id: &str) -> Result<Option<String>, String> {
    let wallet = crate::papers::Wallet::load_at(places)?;
    let Some(paper) = wallet.get(id).cloned() else { return Ok(None) };
    let file = wallet.file_path(&paper);
    let shared = wallet.papers.iter().any(|p| p.id != id && p.file == paper.file);
    crate::papers::Wallet::change_at(places, |w| w.remove(id))?;
    if let Some(file) = file.filter(|f| f.exists() && !shared) {
        crate::filelock::remove_wiping(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    acts::append(&place.log(), &[Act { at: jiff::Timestamp::now().as_millisecond(), object: links::paper_uri(id), act: "paper".into(), result: links::paper_uri(id), refused: "rules-refused-infected".into(), ..Act::default() }])?;
    Ok(Some(paper.title))
}

/// A message's attachments: each one's name (as a folder takes it), its
/// type as the message says it, its bytes.
pub fn attachments(raw: &[u8]) -> Vec<(String, String, Vec<u8>)> {
    let Some(message) = mail_parser::MessageParser::default().parse(raw) else { return Vec::new() };
    let mut out = Vec::new();
    for (index, part) in message.attachments().enumerate() {
        use mail_parser::MimeHeaders;
        let mime = part.content_type().map(|t| format!("{}/{}", t.ctype(), t.subtype().unwrap_or_default()).to_ascii_lowercase()).unwrap_or_default();
        let Some((name, bytes)) = crate::reading::attachment_from(raw, u32::try_from(index).unwrap_or(u32::MAX)) else { continue };
        out.push((name, mime, bytes));
    }
    out
}

/// A file copied into the papers' folder under its name carrying its digest
/// (`kept_name`), or found there already with the same bytes (another device
/// kept it, the folder came with the sharing); else under a free name. Its
/// path as the list names it.
fn keep_named(wallet: &crate::papers::Wallet, file: &Path, name: &str, sum: &str) -> Result<String, String> {
    let named = kept_name(name, sum);
    let there = wallet.docs.folder.join(&named);
    if std::fs::read(&there).is_ok_and(|bytes| digest(&bytes) == sum) {
        return Ok(wallet.docs.named(&there));
    }
    wallet.keep_file(file, &named)
}

/// The messages of the last `days` days in the mail index, one copy each,
/// for "also the last 30 days" and the preview.
pub fn recent_files(index: &crate::mailindex::MailIndex, now: i64, days: u32) -> Vec<PathBuf> {
    let since = now - i64::from(days) * 86_400;
    index.iter().map(|(_, mail)| mail).chain(index.without_id().map(|(_, mail)| mail)).filter(|mail| mail.date >= since).map(|mail| mail.path.clone()).collect()
}

/// A task's file by its UID: in the list the act names, else in any list
/// (moved since).
fn task_file(uid: &str, list: &str, find_list: &dyn Fn(&str) -> Option<Collection>, all_lists: &dyn Fn() -> Vec<Collection>) -> Option<PathBuf> {
    let name = format!("{uid}.ics");
    find_list(list).map(|l| l.dir.join(&name)).filter(|p| p.exists()).or_else(|| all_lists().into_iter().map(|l| l.dir.join(&name)).find(|p| p.exists()))
}

/// What one act wrote, put back while it is as the rule left it: a task
/// (wherever it moved), a tie, a topic's word still the rule's, a paper and
/// its file (neither when the paper was filled in since, nor its file
/// changed; the file kept when another paper names it).
/// Whether it was put back, or is gone already.
fn put_back(act: &Act, place: &Place, places: Option<&Places>, find_list: &dyn Fn(&str) -> Option<Collection>, all_lists: &dyn Fn() -> Vec<Collection>, now_ms: i64) -> Result<bool, String> {
    let object = act.object.clone();
    Ok(match act.act.as_str() {
        "task" if act.wrote => {
            let uid = links::id_of(&act.result);
            match task_file(&uid, &act.list, find_list, all_lists) {
                None => true,
                Some(path) => {
                    let as_left = std::fs::read(&path).is_ok_and(|bytes| digest(&bytes) == act.digest);
                    if as_left {
                        crate::vdir::remove_item(&path)?;
                        LocalLinks::change(&place.ties, |local| {
                            let before = local.links.len();
                            local.links.retain(|e| !(e.how == MADE && e.from == act.result && e.to == object));
                            before != local.links.len()
                        })?;
                    }
                    as_left
                }
            }
        }
        "tie" if act.wrote => {
            LocalLinks::change(&place.ties, |local| {
                let before = local.links.len();
                local.links.retain(|e| !(e.how == "project" && ((e.from == object && e.to == act.result) || (e.to == object && e.from == act.result))));
                before != local.links.len()
            })?;
            true
        }
        "topic" if act.wrote => {
            let said = labels::Said::of(&labels::read_all_in(&place.topics));
            let id = crate::mailindex::bare_id(&crate::notes::decode(object.strip_prefix("mid:").unwrap_or(&object)));
            match said.message(&id) {
                Some(Some(topic)) if topic == act.result => {
                    labels::append(&labels::own_log(&place.topics, &place.device), &[labels::Act::Message { at: now_ms / 1000, message_id: id, topic: None, forget: true }])?;
                    true
                }
                // Taken back already, here or by another device.
                None => true,
                _ => false,
            }
        }
        "paper" => {
            let id = links::id_of(&act.result);
            let tie = Edge { from: act.result.clone(), to: object.clone(), how: MADE.into() };
            let untie = || {
                LocalLinks::change(&place.ties, |local| {
                    let before = local.links.len();
                    local.links.retain(|e| *e != tie);
                    before != local.links.len()
                })
            };
            match places {
                Some(places) if act.wrote => {
                    let wallet = crate::papers::Wallet::load_at(places)?;
                    match wallet.get(&id).cloned() {
                        None => true,
                        Some(paper) => {
                            let file = wallet.file_path(&paper);
                            // Filled in since (its dates, a note, a renewal planned): yours now, it stays.
                            let filled = paper.issued.is_some() || paper.until.is_some() || !paper.notes.trim().is_empty() || !paper.renewal.is_empty();
                            let as_left = !filled && file.as_ref().is_none_or(|f| !f.exists() || std::fs::read(f).is_ok_and(|bytes| digest(&bytes) == act.digest));
                            if as_left {
                                let shared = wallet.papers.iter().any(|p| p.id != id && p.file == paper.file);
                                crate::papers::Wallet::change_at(places, |w| w.remove(&id))?;
                                if let Some(file) = file.filter(|f| f.exists() && !shared) {
                                    std::fs::remove_file(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                                }
                                LocalLinks::change(&place.ties, |local| {
                                    let before = local.links.len();
                                    local.links.retain(|e| !(e.from == act.result || e.to == act.result));
                                    before != local.links.len()
                                })?;
                            }
                            as_left
                        }
                    }
                }
                _ => {
                    untie()?;
                    true
                }
            }
        }
        _ => true,
    })
}

/// An act put back while it is as the rule left it (`put_back`), and "not
/// for this one" written either way: the rule never does it again for that
/// object, and every device takes back what it wrote of it (`take_back`).
/// `find_list` finds a task's list by "account/id"; a task moved since is
/// looked for in every list (`crate::tasks::lists`).
pub fn undo(act: &Act, place: &Place, places: Option<&Places>, find_list: &dyn Fn(&str) -> Option<Collection>, now_ms: i64) -> Result<Undone, String> {
    let put = put_back(act, place, places, find_list, &crate::tasks::lists, now_ms)?;
    not_for_this_one(&act.rule, &act.object, place, now_ms)?;
    Ok(Undone { put_back: put })
}

/// "Not for this one": the rule never acts on that object again, on any device.
pub fn not_for_this_one(rule: &str, object: &str, place: &Place, now_ms: i64) -> Result<(), String> {
    acts::append(&place.log(), &[Act { at: now_ms, rule: rule.to_string(), object: object.to_string(), undone: true, ..Act::default() }])
}

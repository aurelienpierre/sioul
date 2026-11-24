// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The engine (docs/rules.md, "How rules run"): rules made to converge, not
//! to fire. For each rule that runs and each object among those it is
//! handed that matches its "when", its result must exist:
//!
//! - **Once, on every device**: what a rule makes has an identifier worked
//!   out from the rule's id and the object's address (`task_uid`,
//!   `paper_id`), so that two devices that both act write the same thing;
//!   every act is written in the shared record (`acts`), which every device
//!   reads before acting: a rule never acts twice on one object.
//! - **Not for this one**: an act undone (`undo`) or refused for good (an
//!   attachment the antivirus refused) is remembered, and never redone.
//! - **The guards, here and not in each action** (design, parts 3 and 5):
//!   never at an address's first fetch (`on_arrival`), never on what came
//!   before the rule (`Rule::since`), never on what the person sent; only
//!   mail the Porch admits (never what it sets aside, nor what your spam
//!   filter caught: `set_aside`), a sender condition only on a sender
//!   something proves (`message`); never what sends, deletes, pays, or
//!   writes a setting, a rule or health (`declare::forbidden`), never a
//!   read-only list; never on what the same rule made, and a chain stops at
//!   `declare::CHAIN` steps.
//! - **Said once, with Undo**: each act comes back as a `Done`, its line
//!   (`words::act_line`) and its record (`agentwrites::Said`) for the
//!   status line where there is a window; `undo` puts it back while it is as
//!   the rule left it, and says "not for this one" either way.
//! - **What it would do** (`preview`), over the mail of the last 30 days,
//!   writes nothing; "also the last 30 days" (`for_already_here`) acts once
//!   on each.

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

/// What the antivirus said of a file a rule would keep (sioul-sync's
/// `antivirus::for_rules` answers it on a computer and on a phone alike).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scanned {
    /// Read, and nothing found.
    Clean,
    /// Refused: never kept, and the rule never keeps that message's files.
    Infected(String),
    /// No antivirus can be asked where Sioul runs (a phone): kept as "Keep in
    /// papers" keeps it there, unchecked, as Sioul says of every attachment there.
    NoAntivirus,
    /// None answered on a computer, or the file is too large for it: by hand
    /// you would be asked; a rule cannot ask, so it keeps nothing.
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

/// Whether the Porch keeps a message from every rule: set aside (forged,
/// blocked, a borrowed name, your provider's spam no route protects),
/// hostile, or what your own spam filter caught.
pub fn set_aside(judged: &Triaged) -> bool {
    matches!(judged.lane, Lane::SetAside | Lane::Hostile | Lane::Review)
}

/// When a message reached your provider (its file's time, `maildir::store`),
/// else its Date: what "it came before the rule" is judged by.
pub fn arrived_at(card: &Card) -> Option<i64> {
    let file = card.path.as_ref().and_then(|p| std::fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).and_then(|d| i64::try_from(d.as_secs()).ok());
    file.or(card.date)
}

/// How a pass runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Mail just fetched: what came before a rule is left alone.
    Arrival,
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

/// A rule that matched but did not act, and why (a message id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refused {
    pub rule: String,
    pub object: String,
    pub why: &'static str,
}

/// What a pass did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Outcome {
    pub done: Vec<Done>,
    /// The notices of `tell`, for the caller to post where this device keeps the notices.
    #[serde(skip)]
    pub notices: Vec<Notice>,
    pub refused: Vec<Refused>,
    /// What could not be written, said once.
    pub problems: Vec<String>,
}

impl Outcome {
    pub fn is_empty(&self) -> bool {
        self.done.is_empty() && self.notices.is_empty() && self.refused.is_empty() && self.problems.is_empty()
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
    /// Put back; false: it changed since and stays as it is.
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
    /// Every device's acts, as read when the engine was made.
    pub acts: Acts,
    pub place: Place,
    pub workdays: Workdays,
    /// The person's own addresses: what they send is never acted on.
    pub own: Vec<String>,
    /// This device keeps the notices (the "notices" lease): it tells; the
    /// others leave `tell` to it.
    pub tells_here: bool,
    /// The list a model's tasks go into (`routetasks::list_for`).
    pub lists: Box<dyn Fn(&Model) -> Option<Collection> + 'a>,
    /// A list by "account/id", for Undo.
    pub find_list: Box<dyn Fn(&str) -> Option<Collection> + 'a>,
    /// Whether a task of this UID is known (the window's tasks).
    pub known: Box<dyn Fn(&str) -> bool + 'a>,
    /// A message's topic's id (`topics::rules::Lookup`), asked only when a rule reads it.
    pub topic_of: Box<dyn Fn(&Card) -> Option<String> + 'a>,
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
    /// without it, a rule keeps no file. `tells_here` false until the caller says.
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
            known: Box::new(|_: &str| false),
            topic_of: Box::new(move |card: &Card| {
                let mut kept = lookup.borrow_mut();
                let lookup = kept.get_or_insert_with(|| crate::topics::rules::Lookup::load(&for_topics));
                let folder = card.path.as_ref().and_then(|p| p.parent()?.parent()?.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                lookup.tagged(card, card.account.as_deref().unwrap_or_default(), &folder).map(|(topic, _)| topic.id.clone())
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

    /// The rules that run, for mail just fetched: nothing at an address's
    /// first fetch (`first`), what each rule takes from its `since` on;
    /// each act written, its record kept, said in the outcome.
    pub fn on_arrival(&self, judged: &[Triaged], first: bool, tr: &Translator) -> Outcome {
        if first || self.rules.read_only() {
            return self.outcome_alone();
        }
        let rules: Vec<&Rule> = self.rules.running().filter(|r| r.when.kind == "message" && r.when.event == "arrives").collect();
        self.pass(&rules, judged, Mode::Arrival, tr).0
    }

    /// "Also for the messages already here": one rule that runs, on the
    /// messages handed (the last 30 days: `recent_files`), each once; what
    /// came before the rule taken too, since it is asked. Nothing told.
    pub fn for_already_here(&self, rule: &str, judged: &[Triaged], tr: &Translator) -> Outcome {
        let Some(rule) = self.rules.running().find(|r| r.id == rule) else { return self.outcome_alone() };
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

    fn outcome_alone(&self) -> Outcome {
        Outcome { problems: self.problems.clone(), ..Outcome::default() }
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

    fn pass(&self, rules: &[&Rule], judged: &[Triaged], mode: Mode, tr: &Translator) -> (Outcome, Vec<Would>) {
        let mut out = self.outcome_alone();
        // What a preview would do, gathered before its words are made.
        let mut would: Vec<(String, String, String, String, i64, Act, bool, Option<&'static str>)> = Vec::new();
        if rules.is_empty() {
            return (out, Vec::new());
        }
        let mut work = Work { ties: LocalLinks::load(&self.place.ties), new_ties: Vec::new(), topics: None, topic_lines: Vec::new(), records: Vec::new(), problems: Vec::new(), papers: None };
        let needs_topic = rules.iter().any(|r| r.when.conditions.iter().any(|c| c.field == "topic"));
        let mut subjects: Vec<(String, String)> = Vec::new();
        let mut done: Vec<(Act, Vec<FileChange>)> = Vec::new();
        for t in judged {
            if set_aside(t) {
                continue;
            }
            let Some((_, address)) = routetasks::message_of(&t.card) else { continue };
            if t.card.from_address.as_deref().is_some_and(|from| self.own.iter().any(|own| own.eq_ignore_ascii_case(from.trim()))) {
                continue;
            }
            let arrived = arrived_at(&t.card);
            let topic = if needs_topic { (self.topic_of)(&t.card) } else { None };
            let seen = message::seen(t, &self.senders, self.store.as_ref(), &self.words, topic);
            subjects.push((address.clone(), t.card.subject.clone()));
            for rule in rules {
                if mode == Mode::Arrival && arrived.is_none_or(|at| at < rule.since) {
                    continue;
                }
                let before = self.acts.touched(&rule.id, &address) || work.records.iter().any(|a| a.rule == rule.id && a.object == address);
                if before && mode != Mode::Preview {
                    continue;
                }
                if self.acts.made_by(&address, &rule.id) {
                    out.refused.push(Refused { rule: rule.id.clone(), object: address.clone(), why: "rules-refused-own" });
                    continue;
                }
                let depth = self.acts.depth_of(&address);
                if depth >= declare::CHAIN {
                    out.refused.push(Refused { rule: rule.id.clone(), object: address.clone(), why: "rules-refused-chain" });
                    continue;
                }
                if !message::holds(&rule.when, &seen) {
                    continue;
                }
                let base = Act { at: self.now.timestamp().as_millisecond(), rule: rule.id.clone(), object: address.clone(), depth: depth + 1, ..Act::default() };
                match self.act(rule, &seen, base, arrived, mode, &mut work, tr) {
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
                        let act = Act { rule: rule.id.clone(), object: address.clone(), act: rule.then.action.clone(), ..Act::default() };
                        would.push((rule.id.clone(), address.clone(), t.card.subject.clone(), t.card.from_address.clone().unwrap_or_default(), arrived.unwrap_or(0), act, before, Some(why)));
                    }
                    Err(why) => out.refused.push(Refused { rule: rule.id.clone(), object: address.clone(), why }),
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
            work.records.retain(|a| a.act != "tie");
            done.retain(|(a, _)| a.act != "tie");
        }
        if !work.topic_lines.is_empty()
            && let Err(e) = labels::append(&labels::own_log(&self.place.topics, &self.place.device), &work.topic_lines)
        {
            work.problems.push(e);
            work.records.retain(|a| a.act != "topic");
            done.retain(|(a, _)| a.act != "topic");
        }
        if let Err(e) = acts::append(&self.place.log(), &work.records) {
            work.problems.push(e);
        }
        let at = self.now.timestamp().as_millisecond();
        out.done = done
            .into_iter()
            .filter(|(act, _)| !act.undone)
            .map(|(act, files)| {
                let line = words::act_line(&act, &names, tr);
                Done { said: Said { at, line: line.clone(), files }, line, act }
            })
            .collect();
        out.problems.extend(work.problems);
        (out, Vec::new())
    }

    /// The fixed text of a parameter; none when it is not given, empty, or taken from the object.
    fn fixed(rule: &Rule, key: &str) -> Option<String> {
        rule.then.with.get(key).and_then(Param::text).filter(|t| !t.is_empty())
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
            ("message", "give-topic") => self.topic(rule, seen, base, work),
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
    /// never over a word of yours on it.
    fn topic(&self, rule: &Rule, seen: &Seen, base: Act, work: &mut Work) -> Result<Vec<Doing>, &'static str> {
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
        let line = labels::Act::Message { at: self.now.timestamp().as_second(), message_id: id, topic: Some(topic.clone()), forget: false };
        Ok(vec![Doing { act: Act { act: "topic".into(), result: topic, wrote: true, ..base }, topic: Some(line), ..Doing::default() }])
    }

    /// Make a task from a model: in the model's list (else the usual one),
    /// its UID from the rule and the message, tied to it as made; found
    /// when another device made it, or it is known, or its tie is there.
    fn task(&self, rule: &Rule, seen: &Seen, base: Act, arrived: Option<i64>, mode: Mode, work: &mut Work) -> Result<Vec<Doing>, &'static str> {
        let id = Self::fixed(rule, "model").ok_or("rules-refused-no-model")?;
        let model = store::model(&self.models, &id).map(|m| m.model.clone()).ok_or("rules-refused-no-model")?;
        let project = match rule.then.with.get("project") {
            Some(Param::From { .. }) => seen.projects.first().cloned(),
            Some(given) => match given.text().filter(|t| !t.is_empty()) {
                Some(id) => Some(self.store.as_ref().and_then(|s| s.get(&id)).map(|p| p.id.clone()).ok_or("rules-refused-no-project")?),
                None => None,
            },
            None => Some(model.project.clone()).filter(|p| !p.is_empty()),
        };
        let title = match rule.then.with.get("title") {
            Some(Param::Text(text)) if !text.trim().is_empty() => format!("{} {}", model.prefix.trim(), text.trim()).trim().to_string(),
            _ => {
                let words: Vec<String> = rule.when.conditions.iter().filter(|c| c.field == "subject" && c.test == "has-words").flat_map(|c| c.value.clone()).collect();
                routetasks::title(&seen.card().subject, &words, &model.prefix, &model.name)
            }
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
        let day = arrived.and_then(|at| jiff::Timestamp::from_second(at).ok()).map_or_else(|| self.now.date(), |t| t.to_zoned(self.zone.clone()).date());
        let due = self.workdays.date_asked(day, days);
        let ties = if tied { Vec::new() } else { vec![Edge { from: task.clone(), to: base.object.clone(), how: MADE.into() }] };
        let mut doing = Doing { act: Act { act: "task".into(), result: task, wrote: !found, list: format!("{}/{}", list.account, list.id), ..base }, ties, ..Doing::default() };
        if !found {
            let edit = crate::tasks::TaskEdit { title, due: due.to_string(), estimate: model.estimate.unwrap_or(0), projects: project.into_iter().collect(), supplies: model.supplies.clone(), ..crate::tasks::TaskEdit::default() };
            let text = crate::tasks::new_task(&edit, &uid, &self.zone, &self.now)
                .and_then(|text| crate::tasks::add_lines(&text, &[format!("{}:{}", routetasks::ORIGIN, crate::lines::escape(&doing.act.object)), format!("{RULE}:{}", crate::lines::escape(&rule.id))], &self.now))
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
    /// the device that keeps the notices; quietly, said and kept only.
    /// Never for mail already here.
    fn tell(&self, rule: &Rule, seen: &Seen, base: Act, mode: Mode, tr: &Translator) -> Result<Vec<Doing>, &'static str> {
        let level = Self::fixed(rule, "level").unwrap_or_else(|| "now".to_string());
        if !declare::LEVELS.iter().any(|l| l.id == level) {
            return Err("rules-problem-value");
        }
        if mode == Mode::Asked || (mode == Mode::Arrival && !self.tells_here) {
            return Ok(Vec::new());
        }
        let notice = (level == "now").then(|| {
            let card = seen.card();
            let from = card.from_name.clone().filter(|n| !n.trim().is_empty()).or_else(|| card.from_address.clone()).unwrap_or_default();
            let title = Self::fixed(rule, "words").unwrap_or_else(|| tr.text("rules-tell-title", None));
            let mut args = crate::i18n::args();
            args.set("from", from);
            args.set("subject", card.subject.clone());
            // A code is never said in a notice of Sioul's own: its own notice says it.
            let coded = seen.kinds.iter().any(|k| matches!(*k, "code" | "password" | "password-reset" | "sign-in-link" | "confirmation"));
            let body = tr.text(if coded { "rules-tell-body-code" } else { "rules-tell-body" }, Some(&args));
            let hint = card.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            Notice::new(base.object.clone(), notice::Kind::Minor, title, body).hint(hint.clone()).action(tr.text("notify-open-message", None), notice::Act::Open { address: base.object.clone(), hint })
        });
        Ok(vec![Doing { act: Act { act: "tell".into(), result: level.clone(), wrote: level == "now", ..base }, notice, ..Doing::default() }])
    }

    /// Keep its attachments in Papers: the files the rule names (of a type,
    /// PDF unless it says another or any; whose name holds its words),
    /// never a program; each checked by the antivirus first, one refused
    /// keeping none of the message's and the rule never keeping them since;
    /// each kept once, by a paper id from the rule, the message and the
    /// file's digest; a file kept as a paper already (by a rule, by hand)
    /// gets the tie instead. The message itself is never changed.
    #[allow(clippy::too_many_arguments)]
    fn papers(&self, rule: &Rule, seen: &Seen, base: Act, arrived: Option<i64>, mode: Mode, work: &mut Work, tr: &Translator) -> Result<Vec<Doing>, &'static str> {
        let places = self.places.as_ref().ok_or("rules-refused-no-papers")?;
        let card = seen.card();
        let raw = card.path.as_ref().and_then(|p| std::fs::read(p).ok()).ok_or("rules-refused-no-attachment")?;
        let wanted = Self::fixed(rule, "type").unwrap_or_else(|| "pdf".to_string());
        let named = Self::fixed(rule, "name").map(|n| crate::rules::fold(&n));
        let files: Vec<(String, Vec<u8>)> = attachments(&raw)
            .into_iter()
            .filter(|(name, _, _)| !links::is_program(name) && !links::is_phone_program(name))
            .filter(|(name, mime, _)| wanted == "any" || crate::rules::attachment_kind(name, mime) == wanted)
            .filter(|(name, _, _)| named.as_ref().is_none_or(|n| n.is_empty() || crate::rules::fold(name).contains(n.as_str())))
            .map(|(name, _, bytes)| (name, bytes))
            .collect();
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let message = base.object.clone();
        let made_tie = |paper: &str| Edge { from: links::paper_uri(paper), to: message.clone(), how: MADE.into() };
        let mut wallet = crate::papers::Wallet::load_at(places).map_err(|e| {
            work.problems.push(e);
            "rules-refused-no-papers"
        })?;
        let mut doings = Vec::new();
        let mut fresh: Vec<(String, Vec<u8>, String)> = Vec::new();
        for (name, bytes) in files {
            let sum = digest(&bytes);
            // Kept already as a paper: by this rule on another device, by another rule, by hand.
            let id = paper_id(&rule.id, &message, &sum);
            let kept = wallet.get(&id).map(|p| p.id.clone()).or_else(|| self.paper_with(&wallet, &sum, bytes.len() as u64, work));
            match kept {
                Some(paper) => {
                    let tie = made_tie(&paper);
                    let there = work.ties.links.contains(&tie) || work.new_ties.contains(&tie);
                    doings.push(Doing { act: Act { act: "paper".into(), result: links::paper_uri(&paper), wrote: false, digest: sum, ..base.clone() }, ties: if there { Vec::new() } else { vec![tie] }, ..Doing::default() });
                }
                None => fresh.push((name, bytes, sum)),
            }
        }
        if mode == Mode::Preview {
            for (_, _, sum) in fresh {
                let id = paper_id(&rule.id, &message, &sum);
                doings.push(Doing { act: Act { act: "paper".into(), result: links::paper_uri(&id), wrote: true, digest: sum, ..base.clone() }, ..Doing::default() });
            }
            return Ok(doings);
        }
        // Every file checked before any is kept: one refused keeps the message's files out, for good.
        let folder = self.scratch.join(&base.object.chars().filter(char::is_ascii_alphanumeric).take(24).collect::<String>());
        let mut checked: Vec<(String, PathBuf, String)> = Vec::new();
        let clear = |paths: &[(String, PathBuf, String)]| {
            for (_, path, _) in paths {
                let _ = crate::filelock::remove_wiping(path);
            }
        };
        for (name, bytes, sum) in fresh {
            let file = folder.join(crate::papers::portable_name_of(&name));
            if let Err(e) = std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&file, &bytes)) {
                work.problems.push(format!("{}: {e}", file.display()));
                clear(&checked);
                return Err("rules-refused-write");
            }
            checked.push((name, file.clone(), sum));
            match (self.scan)(&file) {
                Scanned::Clean | Scanned::NoAntivirus => {}
                Scanned::Infected(_) => {
                    clear(&checked);
                    work.records.push(Act { act: "paper".into(), undone: true, ..base.clone() });
                    return Err("rules-refused-infected");
                }
                Scanned::Unanswered(_) => {
                    clear(&checked);
                    return Err("rules-refused-unchecked");
                }
            }
        }
        let movement = self.movement_of(places, &message);
        let person = Self::fixed(rule, "for").filter(|p| p.as_str() != crate::persons::ME);
        for (name, file, sum) in &checked {
            let id = paper_id(&rule.id, &message, sum);
            let title = self.paper_title(rule, seen, name, arrived, tr);
            let kind = Self::fixed(rule, "kind").map(|k| crate::papers::Kind::of(&k)).or_else(|| crate::papers::Kind::guess(&self.words, name)).or_else(|| crate::papers::Kind::guess(&self.words, &card.subject)).unwrap_or_default();
            let kept = keep_named(&wallet, file, name, sum).and_then(|path| {
                let holder = person.as_ref().map(|p| {
                    let persons = crate::persons::Persons::load(&crate::persons::Persons::default_path());
                    crate::persons::holder_text(&persons, std::slice::from_ref(p), &[], "")
                });
                let paper = crate::papers::Paper { id: id.clone(), kind, title, file: path, holder: holder.unwrap_or_default(), added: Some(self.now.date()), ..crate::papers::Paper::default() };
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
        clear(&checked);
        let _ = std::fs::remove_dir(&folder);
        Ok(doings)
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
    /// {month}: the month and year it came; {year}; {subject}; {file}: the
    /// file's name without its extension); without one, as "Keep in papers"
    /// guesses it, from the file's name.
    fn paper_title(&self, rule: &Rule, seen: &Seen, name: &str, arrived: Option<i64>, tr: &Translator) -> String {
        let card = seen.card();
        let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().replace(['_', '-'], " ")).unwrap_or_default();
        let Some(pattern) = Self::fixed(rule, "title") else {
            return if stem.trim().is_empty() { card.subject.clone() } else { stem };
        };
        let sender = seen
            .cards
            .first()
            .and_then(|uid| self.senders.contacts.find(uid))
            .and_then(|i| self.senders.contacts.card(i))
            .map(|c| c.name.clone())
            .filter(|n| !n.trim().is_empty())
            .or_else(|| card.from_name.clone().filter(|n| !n.trim().is_empty()))
            .or_else(|| card.from_address.clone())
            .unwrap_or_default();
        let day = arrived.and_then(|at| jiff::Timestamp::from_second(at).ok()).map_or_else(|| self.now.date(), |t| t.to_zoned(self.zone.clone()).date());
        let filled = pattern.replace("{sender}", sender.trim()).replace("{month}", &tr.month_year(day)).replace("{year}", &day.year().to_string()).replace("{subject}", card.subject.trim()).replace("{file}", stem.trim());
        let filled = filled.split_whitespace().collect::<Vec<_>>().join(" ");
        if filled.is_empty() { stem } else { filled }
    }
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

/// A file copied into the papers' folder under its own name, or found there
/// already with the same bytes (another device kept it, the folder came
/// with the sharing); else under a free name. Its path as the list names it.
fn keep_named(wallet: &crate::papers::Wallet, file: &Path, name: &str, sum: &str) -> Result<String, String> {
    let there = wallet.docs.folder.join(crate::papers::portable_name_of(name));
    if std::fs::read(&there).is_ok_and(|bytes| digest(&bytes) == sum) {
        return Ok(wallet.docs.named(&there));
    }
    wallet.keep_file(file, name)
}

/// The messages of the last `days` days in the mail index, one copy each,
/// for "also the last 30 days" and the preview.
pub fn recent_files(index: &crate::mailindex::MailIndex, now: i64, days: u32) -> Vec<PathBuf> {
    let since = now - i64::from(days) * 86_400;
    index.iter().map(|(_, mail)| mail).chain(index.without_id().map(|(_, mail)| mail)).filter(|mail| mail.date >= since).map(|mail| mail.path.clone()).collect()
}

/// An act put back while it is as the rule left it (a task not changed since,
/// a tie, a topic still the rule's word, a paper and its file), and "not for
/// this one" written either way: the rule never does it again for that
/// object, on any device. `find_list` finds a task's list by "account/id".
pub fn undo(act: &Act, place: &Place, places: Option<&Places>, find_list: &dyn Fn(&str) -> Option<Collection>, now_ms: i64) -> Result<Undone, String> {
    let object = act.object.clone();
    let put_back = match act.act.as_str() {
        "task" if act.wrote => {
            let uid = links::id_of(&act.result);
            let path = find_list(&act.list).map(|list| list.dir.join(format!("{uid}.ics")));
            match path.filter(|p| p.exists()) {
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
            if said.message(&id) == Some(Some(act.result.as_str())) {
                labels::append(&labels::own_log(&place.topics, &place.device), &[labels::Act::Message { at: now_ms / 1000, message_id: id, topic: None, forget: true }])?;
                true
            } else {
                false
            }
        }
        "paper" => {
            let id = links::id_of(&act.result);
            let tie = Edge { from: act.result.clone(), to: object.clone(), how: MADE.into() };
            let mut put = true;
            if act.wrote
                && let Some(places) = places
            {
                let wallet = crate::papers::Wallet::load_at(places)?;
                if let Some(paper) = wallet.get(&id).cloned() {
                    let file = wallet.file_path(&paper);
                    let as_left = file.as_ref().is_none_or(|f| !f.exists() || std::fs::read(f).is_ok_and(|bytes| digest(&bytes) == act.digest));
                    if as_left {
                        crate::papers::Wallet::change_at(places, |w| w.remove(&id))?;
                        if let Some(file) = file.filter(|f| f.exists()) {
                            std::fs::remove_file(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                        }
                        LocalLinks::change(&place.ties, |local| {
                            let before = local.links.len();
                            local.links.retain(|e| !(e.from == act.result || e.to == act.result));
                            before != local.links.len()
                        })?;
                    }
                    put = as_left;
                }
            } else {
                LocalLinks::change(&place.ties, |local| {
                    let before = local.links.len();
                    local.links.retain(|e| *e != tie);
                    before != local.links.len()
                })?;
            }
            put
        }
        _ => true,
    };
    not_for_this_one(&act.rule, &object, place, now_ms)?;
    Ok(Undone { put_back })
}

/// "Not for this one": the rule never acts on that object again, on any device.
pub fn not_for_this_one(rule: &str, object: &str, place: &Place, now_ms: i64) -> Result<(), String> {
    acts::append(&place.log(), &[Act { at: now_ms, rule: rule.to_string(), object: object.to_string(), undone: true, ..Act::default() }])
}

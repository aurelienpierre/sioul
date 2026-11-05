// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Notices: Sioul's record of one new thing (docs/porch.md, "Notices").
//!
//! A notice is what the Porch shows for one thing that happened: a message,
//! a text, a call, an app's notification from the phone, a site's
//! notification, a letter, money due. Each source makes its own (the
//! producers: `view::porch` for mail, the window's for the others), with an
//! id made from the source alone, so that the same thing gives the same
//! notice on every device (`mail_id`, `text_id`, `call_id`…). The Porch lists
//! notices, never the data behind them. A system notification is one way a
//! notice may reach you: "notification" stays the system's word.
//!
//! **Its state** is New until a word is said of it, with when and on which
//! device (`State`):
//! - **Dismissed**: gone from the Porch of every device, nothing else touched;
//! - **Read**: its item read too, each kind through its own path (`ReadPath`:
//!   a message gets `\Seen` on its server, a text is read in Sioul's Texts);
//!   only for the kinds whose item has a read state (`Kind::readable`); a
//!   call has none, and Dismiss is its "forget";
//! - **Done**: an action settled it (replied, called back, filed).
//!
//! **The words travel** as each device's own log of lines,
//! `notices/<device>.jsonl` in the state folder, one line per press:
//! `{"at": <ms>, "ids": [<notice ids>], "state": "dismissed" | "read" | "done"}`.
//! Lines are appended, and taken out only to undo a press (`unsay`, within
//! its ten seconds) or once forgotten (`forget`). Every device reads every
//! log, and the newest word on an id wins (`States`). The calls' and the
//! phone messages' Seen logs (`calls::SEEN_LOG`) are the same lines without
//! a `state`: each is read as Dismissed (`States::add_seen`), and a word on a
//! call or a phone message is written there too, by its own id, for a Sioul
//! that knows only Seen (`say_seen`).
//!
//! **Sources change notices too**: a message read elsewhere is Read (its
//! `\Seen`, `derived`), an item that is gone (moved out of the inbox,
//! filed, deleted) gives no notice any more.
//!
//! **Kept**: a word is forgotten a week after it was said once its source
//! no longer holds the notice, another device's three days later
//! (`forget`); a notice its source never gives again is gone with it, after
//! the source's own time (the phone's messages: a week).

use crate::areas::Area;
use crate::calls;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The logs' folder, in the state folder; shared (`share::NOTICES`).
pub const FOLDER: &str = "notices";
/// A word is kept this long after it was said, once its notice is gone (days).
pub const KEPT_DAYS: i64 = 7;
/// Another device's words, kept this much longer (a device gone for good never forgets its own).
pub const LATE_DAYS: i64 = 3;

const DAY_MS: i64 = 86_400_000;

/// The logs' folder on this device.
pub fn folder() -> PathBuf {
    crate::config::state_dir().join(FOLDER)
}

// ---------------------------------------------------------------- the model

/// What a notice is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A message (`mail:`).
    Mail,
    /// A text in Sioul's Texts (`text:`).
    Text,
    /// A call a phone screened (`call:`).
    Call,
    /// A notification of another app, from the phone (`phone:`).
    Phone,
    /// A site's notification (`site:`).
    Site,
    /// A paper letter's scan (`letter:`).
    Letter,
    /// Money due or expected (`money:`).
    Money,
    /// A conflict in your agenda waiting for an answer (`agenda:`).
    Agenda,
}

impl Kind {
    pub const ALL: [Kind; 8] = [Kind::Mail, Kind::Text, Kind::Call, Kind::Phone, Kind::Site, Kind::Letter, Kind::Money, Kind::Agenda];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Mail => "mail",
            Kind::Text => "text",
            Kind::Call => "call",
            Kind::Phone => "phone",
            Kind::Site => "site",
            Kind::Letter => "letter",
            Kind::Money => "money",
            Kind::Agenda => "agenda",
        }
    }

    pub fn parse(word: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == word)
    }

    /// A notice's kind, by its id's first word.
    pub fn of_id(id: &str) -> Option<Kind> {
        Kind::parse(id.split(':').next().unwrap_or_default())
    }

    /// Whether its item has a read state of its own, which Read sets too: a
    /// message, a text, an app's notification from the phone. A call, a
    /// letter and money have none; a site's news is seen when the site opens.
    pub fn readable(self) -> bool {
        matches!(self, Kind::Mail | Kind::Text | Kind::Phone)
    }

    /// The older Seen log a word on it is written to as well, by the
    /// source's own id, for a Sioul that knows only Seen: the calls', the
    /// phone messages' (their folder under the state folder).
    pub fn seen_folder(self) -> Option<&'static str> {
        match self {
            Kind::Call => Some(calls::FOLDER),
            Kind::Phone => Some(crate::phonemsgs::FOLDER),
            _ => None,
        }
    }
}

/// The word last said of a notice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    /// Nothing said yet: the Porch shows it.
    #[default]
    New,
    /// Gone from the Porch of every device; its item untouched.
    Dismissed,
    /// Its item read too.
    Read,
    /// An action settled it.
    Done,
}

impl Mark {
    pub fn as_str(self) -> &'static str {
        match self {
            Mark::New => "new",
            Mark::Dismissed => "dismissed",
            Mark::Read => "read",
            Mark::Done => "done",
        }
    }

    pub fn parse(word: &str) -> Option<Mark> {
        [Mark::New, Mark::Dismissed, Mark::Read, Mark::Done].into_iter().find(|m| m.as_str() == word)
    }

    /// No longer shown on the Porch.
    pub fn settled(self) -> bool {
        self != Mark::New
    }
}

/// A notice's state: the word, when it was said (ms; 0 for New, or a word
/// its source said, `derived`), on which device (its name in the sharing; ""
/// for its source's).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct State {
    pub mark: Mark,
    pub at: i64,
    pub device: String,
}

impl State {
    /// A word its source says: a message read elsewhere is Read.
    pub fn derived(mark: Mark) -> State {
        State { mark, at: 0, device: String::new() }
    }

    /// The later of two words: the newest; at the same time, the further on (Done over Read over Dismissed).
    fn later(self, other: State) -> State {
        if (other.at, other.mark, &other.device) > (self.at, self.mark, &self.device) { other } else { self }
    }
}

/// One new thing, as the Porch shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Notice {
    /// Made from its source alone: the same on every device (`mail_id`…).
    pub id: String,
    pub kind: Option<Kind>,
    /// What opening it opens: a message's `mid:…`, a conversation, a site.
    pub address: String,
    /// When it happened (ms).
    pub at: i64,
    /// When, as Mail's rows say it ("14:10", "Mon 2 Nov").
    pub when: String,
    /// Who: a sender, a caller, a site, a letter's sender.
    pub who: String,
    /// What, short: a subject, a text's first words.
    pub what: String,
    /// Work, admin, leisure, together; "" unsaid.
    pub area: String,
    /// Its project's id; "" none.
    pub project: String,
    /// The Porch's section: for mail, the Porch's own decision
    /// (`project:<id>`, `people`…); for the others, their own (`phone`,
    /// `sites`, `money`, `letters`).
    pub section: String,
    /// Whether its item has a read state (`Kind::readable`): Mark read beside it.
    pub readable: bool,
    pub state: State,
}

impl Notice {
    /// A notice of `kind` by its id, New, in `section`.
    pub fn new(kind: Kind, id: String, section: &str) -> Notice {
        Notice { id, kind: Some(kind), readable: kind.readable(), section: section.to_string(), ..Notice::default() }
    }

    /// Its area, as files write it.
    pub fn with_area(mut self, area: Option<Area>) -> Notice {
        self.area = area.map(Area::id).unwrap_or_default();
        self
    }
}

// ---------------------------------------------------------------- ids

/// Sixteen hexadecimal digits of a text's SHA-256: a digest the same on every device.
pub fn digest(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// A part of an id: its `:` kept out, so that the id splits back.
fn part(text: &str) -> String {
    text.trim().replace(':', "_")
}

/// A message's notice: `mail:<account>:<digest of its Message-ID>`; one
/// without a Message-ID by the digest of its address's (`sioul:mail/…`,
/// `fallback`), which every device makes alike.
pub fn mail_id(account: &str, message_id: Option<&str>, fallback: &str) -> String {
    let id = message_id.map(crate::mailindex::bare_id).filter(|id| !id.is_empty());
    let digest = match id {
        Some(id) => digest(&id),
        None => fallback.rsplit('/').next().map(str::to_string).filter(|d| !d.is_empty()).unwrap_or_else(|| digest(fallback)),
    };
    format!("mail:{}:{digest}", part(account))
}

/// A message's notice, from its card.
pub fn mail_id_of(card: &crate::card::Card) -> String {
    let account = card.account.as_deref().unwrap_or_default();
    let fallback = if card.message_id.is_some() {
        String::new()
    } else {
        let address = crate::mailfind::address_of_card(card);
        if address.is_empty() { format!("{}\u{1f}{}\u{1f}{}\u{1f}{}", card.from_address.as_deref().unwrap_or_default(), card.to.join(","), card.date.unwrap_or(0), card.subject) } else { address }
    };
    mail_id(account, card.message_id.as_deref(), &fallback)
}

/// A text's notice: `text:<phone>:<thread>:<its row's name>`.
pub fn text_id(phone: &str, thread: &str, message: &str) -> String {
    format!("text:{}:{}:{}", part(phone), part(thread), part(message))
}

/// A call's notice: `call:<phone>:<the call's id>` (`calls::Held::id`).
pub fn call_id(phone: &str, call: &str) -> String {
    format!("call:{}:{}", part(phone), part(call))
}

/// An app's notification from the phone: `phone:<phone>:<its message's id>`
/// (`phonemsgs::Line::id`): one notification can bring several messages,
/// each its own notice, so that a message that comes later in the same
/// notification is new again.
pub fn phone_id(phone: &str, message: &str) -> String {
    format!("phone:{}:{}", part(phone), part(message))
}

/// A site's notification: `site:<site>:<when (s)>-<digest of its words>`.
pub fn site_id(site: &str, at: i64, words: &str) -> String {
    format!("site:{}:{at}-{}", part(site), &digest(words)[..8])
}

/// A paper letter: `letter:<its id>` (`letters::Letter::id`, a digest of its scan).
pub fn letter_id(letter: &str) -> String {
    format!("letter:{}", part(letter))
}

/// Money due or expected: `money:<the item>`.
pub fn money_id(item: &str) -> String {
    format!("money:{}", part(item))
}

/// A conflict of your agenda waiting for an answer (`overlaps::Overlap::key`):
/// `agenda:<digest of its key>`, the key holding the events' files.
pub fn agenda_id(conflict: &str) -> String {
    format!("agenda:{}", digest(conflict))
}

/// The source's own id inside a notice's (what an older Seen log names): a
/// call's `<at>-<number>`, a phone message's `<at>-<hash>`.
pub fn source_id(id: &str) -> &str {
    id.splitn(3, ':').nth(2).unwrap_or(id)
}

// ---------------------------------------------------------------- the log

/// One press on a device: when (ms), the notices, the word. A line without
/// a word is an older Sioul's Seen: Dismissed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub at: i64,
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default = "seen")]
    pub state: Mark,
}

fn seen() -> Mark {
    Mark::Dismissed
}

/// A device's own log in `folder`: `<device>.jsonl`.
pub fn own_file(folder: &Path, device: &str) -> PathBuf {
    folder.join(format!("{}.jsonl", calls::file_safe(device)))
}

/// A word said on this device (`device`) of `ids`: one line at the end of
/// its own log in `folder`, which the sharing carries. Returns the line's
/// time, to take it back (`unsay`). New is never written: it is the absence
/// of a word.
pub fn say(folder: &Path, device: &str, ids: &[String], mark: Mark, now_ms: i64) -> Result<i64, String> {
    if ids.is_empty() || mark == Mark::New {
        return Ok(0);
    }
    let path = own_file(folder, device);
    let text = serde_json::to_string(&Line { at: now_ms, ids: ids.to_vec(), state: mark }).map_err(|e| e.to_string())? + "\n";
    crate::filelock::with_lock(&path, || calls::append_private(&path, &text))?;
    Ok(now_ms)
}

/// The same word in an older Seen log (`root`: the calls' or the phone
/// messages' folder), by the source's own ids, at the same time, so that a
/// Sioul that knows only Seen takes those lines off its Porch too.
pub fn say_seen(root: &Path, device: &str, source_ids: &[String], mark: Mark, at: i64) -> Result<(), String> {
    if source_ids.is_empty() || mark == Mark::New {
        return Ok(());
    }
    let path = calls::own_file(root, calls::SEEN_LOG, device);
    let text = serde_json::to_string(&Line { at, ids: source_ids.to_vec(), state: mark }).map_err(|e| e.to_string())? + "\n";
    crate::filelock::with_lock(&path, || calls::append_private(&path, &text))
}

/// A press taken back: this device's own line written at `at` taken out of
/// its log in `folder` (and of an older Seen log beside, `seen_root`), and
/// so out of every device's once the sharing carries it. Another device's
/// word on the same notices stays. Whether a line went.
pub fn unsay(folder: &Path, device: &str, at: i64, seen_root: Option<&Path>) -> Result<bool, String> {
    if at == 0 {
        return Ok(false);
    }
    let pressed = |line: &str| serde_json::from_str::<Line>(line).is_ok_and(|l| l.at == at);
    let mut gone = calls::rewrite_lines(&own_file(folder, device), pressed)?;
    if let Some(root) = seen_root {
        gone += calls::rewrite_lines(&calls::own_file(root, calls::SEEN_LOG, device), pressed)?;
    }
    Ok(gone > 0)
}

/// Every word read, by notice: the newest on each id.
#[derive(Debug, Clone, Default)]
pub struct States {
    by: BTreeMap<String, State>,
}

impl States {
    /// Every device's log in `folder`, read together.
    pub fn read(folder: &Path) -> States {
        let mut states = States::default();
        for (device, path) in calls::device_files(folder) {
            for line in calls::read_lines::<Line>(&path) {
                for id in line.ids {
                    states.put(id, State { mark: line.state, at: line.at, device: device.clone() });
                }
            }
        }
        states
    }

    /// An older Seen log's lines (`root`: the calls' or the phone messages'
    /// folder), each Seen a Dismissed (or the word written there with it),
    /// its source's ids named as notices by `named` (None: not a notice here).
    pub fn add_seen(&mut self, root: &Path, named: impl Fn(&str) -> Option<String>) {
        for (device, path) in calls::device_files(&root.join(calls::SEEN_LOG)) {
            for line in calls::read_lines::<Line>(&path) {
                for raw in &line.ids {
                    if let Some(id) = named(raw) {
                        self.put(id, State { mark: line.state, at: line.at, device: device.clone() });
                    }
                }
            }
        }
    }

    fn put(&mut self, id: String, state: State) {
        match self.by.remove(&id) {
            Some(known) => {
                self.by.insert(id, known.later(state));
            }
            None => {
                self.by.insert(id, state);
            }
        }
    }

    /// A notice's state: the newest word said of it, else what its source
    /// says (`derived`: a message read elsewhere), whichever is further on
    /// when its source's is not New.
    pub fn of(&self, id: &str, derived: State) -> State {
        match self.by.get(id) {
            Some(said) if derived.mark == Mark::New || said.mark >= derived.mark => said.clone(),
            _ => derived,
        }
    }

    /// Every notice a word was said of.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.by.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.by.is_empty()
    }
}

/// Words forgotten in `folder`: this device's own lines (`here`) a week
/// after they were said, another device's three days later, of the kinds
/// `kinds` (each producer forgets its own), once none of their notices is
/// still given by its source here (`current`): a message dismissed while it
/// stays in your inbox stays dismissed. A line that does not read stays.
/// Returns how many lines went.
pub fn forget(folder: &Path, here: &str, kinds: &[Kind], current: &dyn Fn(&str) -> bool, now_ms: i64) -> Result<usize, String> {
    let gone = |days: i64| {
        move |text: &str| {
            serde_json::from_str::<Line>(text).is_ok_and(|line| {
                now_ms - line.at > days * DAY_MS && line.ids.iter().all(|id| Kind::of_id(id).is_some_and(|k| kinds.contains(&k))) && !line.ids.iter().any(|id| current(id))
            })
        }
    };
    let own = calls::file_safe(here);
    let mut went = 0;
    for (device, path) in calls::device_files(folder) {
        went += calls::rewrite_lines(&path, gone(if device == own { KEPT_DAYS } else { KEPT_DAYS + LATE_DAYS }))?;
    }
    Ok(went)
}

/// How long a Read word on mail holds against the message's own `\Seen`:
/// a day, while every copy of the message takes the flag (`forget_read_mail`).
pub const MAIL_READ_HOLDS_MS: i64 = DAY_MS;

/// Read words on mail given back to the messages' own `\Seen` once it said
/// the same for a day (`MAIL_READ_HOLDS_MS`), so that a word said here never
/// fights the server: this device's own (`here`), whose Read made the
/// message read here at once, so that what its flag says now is the
/// server's (an unread elsewhere since included); another device's once
/// every message it names is read here (`seen`), the server having said
/// the same. From then on the message's flag alone says: a later unread on
/// the server shows it again. Dismissed words stay as they are (`forget`).
/// A line that does not read stays. Returns how many lines went.
pub fn forget_read_mail(folder: &Path, here: &str, seen: &dyn Fn(&str) -> bool, now_ms: i64) -> Result<usize, String> {
    let own = calls::file_safe(here);
    let mut went = 0;
    for (device, path) in calls::device_files(folder) {
        let mine = device == own;
        went += calls::rewrite_lines(&path, |text| {
            serde_json::from_str::<Line>(text).is_ok_and(|line| {
                line.state == Mark::Read
                    && now_ms - line.at > MAIL_READ_HOLDS_MS
                    && !line.ids.is_empty()
                    && line.ids.iter().all(|id| Kind::of_id(id) == Some(Kind::Mail))
                    && (mine || line.ids.iter().all(|id| seen(id)))
            })
        })?;
    }
    Ok(went)
}

// ---------------------------------------------------------------- reading

/// What reading notices reads too, each kind through its item's own path:
/// a message's `\Seen` here and on its server, a text's conversation read
/// in Sioul's Texts. `targets` are the items as the rows name them: a
/// message's key, a conversation.
pub trait ReadPath {
    /// Read: the items marked read.
    fn read(&mut self, kind: Kind, targets: &[String]) -> Result<(), String>;
    /// Undone within its ten seconds: the items unread again, where that can be.
    fn unread(&mut self, kind: Kind, targets: &[String]);
}

/// What a row asks of its notices: their kind, their ids, their items (`ReadPath`'s targets).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Asked {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    pub targets: Vec<String>,
}

impl Asked {
    pub fn kind(&self) -> Option<Kind> {
        Kind::parse(&self.kind).or_else(|| self.ids.first().and_then(|id| Kind::of_id(id)))
    }
}

/// Where a device keeps its words (`folder`, the notices' log; `state`, the
/// state folder the older Seen logs are under) and its name in the sharing.
#[derive(Debug, Clone)]
pub struct Logs {
    pub folder: PathBuf,
    pub state: PathBuf,
    pub device: String,
}

impl Logs {
    /// This device's, under its state folder.
    pub fn here(device: &str) -> Logs {
        let state = crate::config::state_dir();
        Logs { folder: state.join(FOLDER), state, device: device.to_string() }
    }

    fn seen_root(&self, kind: Kind) -> Option<PathBuf> {
        kind.seen_folder().map(|f| self.state.join(f))
    }

    /// A word said of a row's notices (Dismiss, Read, Done): Read reads their
    /// items first (`path`), a kind without a read state refused; the line in
    /// this device's log, and in the older Seen log of its kind. Returns the
    /// line's time, to take it back (`take_back`).
    pub fn say(&self, asked: &Asked, mark: Mark, path: &mut dyn ReadPath, now_ms: i64) -> Result<i64, String> {
        let Some(kind) = asked.kind() else { return Err("not a notice".into()) };
        if mark == Mark::Read {
            if !kind.readable() {
                return Err(format!("a {} has no read state", kind.as_str()));
            }
            path.read(kind, &asked.targets)?;
        }
        let at = say(&self.folder, &self.device, &asked.ids, mark, now_ms)?;
        if let Some(root) = self.seen_root(kind) {
            let sources: Vec<String> = asked.ids.iter().map(|id| source_id(id).to_string()).collect();
            say_seen(&root, &self.device, &sources, mark, at)?;
        }
        Ok(at)
    }

    /// A press taken back within its ten seconds: its line out of both logs,
    /// and a Read's items unread again.
    pub fn take_back(&self, asked: &Asked, mark: Mark, at: i64, path: &mut dyn ReadPath) -> Result<bool, String> {
        let Some(kind) = asked.kind() else { return Ok(false) };
        let gone = unsay(&self.folder, &self.device, at, self.seen_root(kind).as_deref())?;
        if gone && mark == Mark::Read {
            path.unread(kind, &asked.targets);
        }
        Ok(gone)
    }

    /// Every word said on every device, the older Seen logs' named by `named`
    /// for their kind (a source's id → its notice).
    pub fn states(&self, named: &dyn Fn(Kind, &str) -> Option<String>) -> States {
        let mut states = States::read(&self.folder);
        for kind in [Kind::Call, Kind::Phone] {
            if let Some(root) = self.seen_root(kind) {
                states.add_seen(&root, |raw| named(kind, raw));
            }
        }
        states
    }
}

/// Whether every one of `ids` is settled: a word said of it (`states`), or its item gone or read (`settled_item`).
pub fn all_settled(ids: &[String], states: &States, settled_item: &dyn Fn(&str) -> bool) -> bool {
    !ids.is_empty() && ids.iter().all(|id| states.of(id, State::default()).mark.settled() || settled_item(id))
}

/// The notices a system notification of this device told, kept beside the
/// log (never shared), so that it is withdrawn here once each is settled on
/// any device (`all_settled`): the newest of each kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posted {
    /// Each kind's last notification: its notices, and what finds each item here (a message's file).
    #[serde(default)]
    pub told: BTreeMap<String, Told>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Told {
    pub at: i64,
    #[serde(default)]
    pub ids: Vec<String>,
    /// For each id, its item here (a message's file), "" unknown.
    #[serde(default)]
    pub items: Vec<String>,
}

impl Posted {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("notices-posted.json")
    }

    pub fn load(path: &Path) -> Posted {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Whether the copy of a stored message `file` names is still there unread:
/// found in its folder's `new/` or `cur/` by the start of its name (flags
/// rename it), without `S`. Gone, or read: settled.
pub fn message_unread(file: &Path) -> bool {
    let Some(name) = file.file_name().and_then(|n| n.to_str()) else { return false };
    let unique = crate::maildir::unique_part(name).to_string();
    let Some(folder) = file.parent().and_then(Path::parent) else { return false };
    let mut found = BTreeSet::new();
    for sub in ["new", "cur"] {
        for entry in std::fs::read_dir(folder.join(sub)).into_iter().flatten().filter_map(Result::ok) {
            let entry = entry.file_name().to_string_lossy().to_string();
            if crate::maildir::unique_part(&entry) == unique {
                found.insert(entry);
            }
        }
    }
    found.iter().any(|name| !crate::maildir::flags_of(Path::new(name)).contains('S'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-notices-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A recorder of what Read asked of each kind's path.
    #[derive(Default)]
    struct Recorder {
        read: Vec<(Kind, Vec<String>)>,
        unread: Vec<(Kind, Vec<String>)>,
    }

    impl ReadPath for Recorder {
        fn read(&mut self, kind: Kind, targets: &[String]) -> Result<(), String> {
            self.read.push((kind, targets.to_vec()));
            Ok(())
        }
        fn unread(&mut self, kind: Kind, targets: &[String]) {
            self.unread.push((kind, targets.to_vec()));
        }
    }

    const NOW: i64 = 1_793_000_000_000;

    /// The same thing gives the same notice on every device: its id is made
    /// from its source alone, never from a file's name or a device.
    #[test]
    fn ids_are_the_same_on_every_device() {
        let raw = b"Message-ID: <a1@example.org>\r\nFrom: Noa <noa@example.org>\r\nTo: you@example.net\r\nSubject: Lease\r\nDate: Tue, 03 Nov 2026 09:00:00 +0100\r\n\r\nHello.\r\n";
        let mut desk = crate::card::Card::from_bytes(raw).unwrap();
        desk.account = Some("home".into());
        desk.path = Some(PathBuf::from("/desk/Mail/home/INBOX/cur/1700.M1P2.desk,U5-40:2,"));
        let mut phone = crate::card::Card::from_bytes(raw).unwrap();
        phone.account = Some("home".into());
        phone.path = Some(PathBuf::from("/phone/mail/home/INBOX/new/1800.M9P9.phone,U5-40"));
        assert_eq!(mail_id_of(&desk), mail_id_of(&phone));
        assert_eq!(mail_id_of(&desk), mail_id("home", Some("<a1@example.org>"), ""));
        assert_eq!(mail_id_of(&desk), mail_id("home", Some("mid:a1@example.org"), ""), "a bare id or an address: the same");
        assert!(mail_id_of(&desk).starts_with("mail:home:") && mail_id_of(&desk).len() == "mail:home:".len() + 16);
        // Another account: another notice (the same message to two of your addresses).
        assert_ne!(mail_id("work", Some("<a1@example.org>"), ""), mail_id_of(&desk));
        // Without a Message-ID: its address's digest, which every device makes alike.
        assert_eq!(mail_id("home", None, "sioul:mail/home/0123456789abcdef"), "mail:home:0123456789abcdef");
        assert_eq!(text_id("phone-1", "12", "sms-40@1793000000000"), "text:phone-1:12:sms-40@1793000000000");
        assert_eq!(call_id("phone-1", "1793000000000-+33199001234"), "call:phone-1:1793000000000-+33199001234");
        assert_eq!(source_id(&call_id("phone-1", "1793000000000-+33199001234")), "1793000000000-+33199001234");
        assert_eq!(source_id(&phone_id("phone-1", "1793000000000-ab12")), "1793000000000-ab12");
        assert_eq!(site_id("bank", 1_793_000_000, "Your statement"), site_id("bank", 1_793_000_000, "Your statement"));
        assert_ne!(site_id("bank", 1_793_000_000, "Your statement"), site_id("bank", 1_793_000_000, "A message"));
        assert_eq!(Kind::of_id("letter:ab12"), Some(Kind::Letter));
        assert_eq!(Kind::of_id(&money_id("rent-2026-11")), Some(Kind::Money));
        // A conflict: by the digest of its key, which holds the events' files.
        let conflict = agenda_id("/cal/home/a.ics@1794000000|/cal/home/b.ics@1794001800");
        assert_eq!((Kind::of_id(&conflict), conflict.len()), (Some(Kind::Agenda), "agenda:".len() + 16));
        assert!(!conflict.contains("cal/home"), "no file in it");
        // A `:` inside a part never splits the id otherwise.
        assert_eq!(call_id("a:b", "c"), "call:a_b:c");
    }

    /// Each device writes its own log; the newest word on an id wins,
    /// whichever device said it; Undo takes this device's own line back only.
    #[test]
    fn the_newest_word_wins_and_undo_takes_back_only_its_own() {
        let dir = scratch("newest");
        let ids = vec![mail_id("home", Some("<a1@example.org>"), "")];
        say(&dir, "desk", &ids, Mark::Dismissed, NOW).unwrap();
        let read_at = say(&dir, "phone", &ids, Mark::Read, NOW + 5_000).unwrap();
        let state = States::read(&dir).of(&ids[0], State::default());
        assert_eq!((state.mark, state.at, state.device.as_str()), (Mark::Read, NOW + 5_000, "phone"));
        // An older word said later on another device's clock still loses to the newest.
        say(&dir, "desk", &ids, Mark::Dismissed, NOW + 1_000).unwrap();
        assert_eq!(States::read(&dir).of(&ids[0], State::default()).mark, Mark::Read);
        // The phone takes its Read back: the desk's newest word stands again.
        assert!(unsay(&dir, "phone", read_at, None).unwrap());
        let state = States::read(&dir).of(&ids[0], State::default());
        assert_eq!((state.mark, state.at, state.device.as_str()), (Mark::Dismissed, NOW + 1_000, "desk"));
        // The desk cannot take back the phone's line.
        let again = say(&dir, "phone", &ids, Mark::Done, NOW + 9_000).unwrap();
        assert!(!unsay(&dir, "desk", again, None).unwrap());
        assert_eq!(States::read(&dir).of(&ids[0], State::default()).mark, Mark::Done);
        // At the same time, the further word wins.
        let both = vec!["mail:home:0000000000000001".to_string()];
        say(&dir, "desk", &both, Mark::Done, NOW).unwrap();
        say(&dir, "phone", &both, Mark::Dismissed, NOW).unwrap();
        assert_eq!(States::read(&dir).of(&both[0], State::default()).mark, Mark::Done);
        // New is never written.
        assert_eq!(say(&dir, "desk", &ids, Mark::New, NOW).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What a source says counts: a message read elsewhere is Read even
    /// with no word said here; a word said here that goes further stands.
    #[test]
    fn a_source_s_word_counts_too() {
        let dir = scratch("derived");
        let id = "mail:home:0000000000000002".to_string();
        let states = States::read(&dir);
        assert_eq!(states.of(&id, State::derived(Mark::Read)).mark, Mark::Read);
        assert_eq!(states.of(&id, State::default()).mark, Mark::New);
        say(&dir, "desk", std::slice::from_ref(&id), Mark::Dismissed, NOW).unwrap();
        let states = States::read(&dir);
        assert_eq!(states.of(&id, State::derived(Mark::Read)).mark, Mark::Read, "read elsewhere after a dismiss: read");
        say(&dir, "desk", std::slice::from_ref(&id), Mark::Done, NOW + 1).unwrap();
        assert_eq!(States::read(&dir).of(&id, State::derived(Mark::Read)).mark, Mark::Done);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The calls' and the phone messages' Seen logs, as 705576a writes them
    /// (`{"at", "ids"}`, no word), are read as Dismissed; a word on a call
    /// is written there too, which such a Sioul reads as Seen, and taken back
    /// there with Undo.
    #[test]
    fn the_older_seen_log_is_still_read_and_written() {
        let state = scratch("seen");
        let calls_root = state.join(calls::FOLDER);
        // An older Sioul's Seen, through its own function.
        calls::mark_seen(&calls_root, "old-desk", &["1793000000000-+33199001234".to_string()], NOW).unwrap();
        let logs = Logs { folder: state.join(FOLDER), state: state.clone(), device: "desk".into() };
        let named = |kind: Kind, raw: &str| (kind == Kind::Call).then(|| call_id("phone-1", raw));
        let id = call_id("phone-1", "1793000000000-+33199001234");
        let read = logs.states(&named).of(&id, State::default());
        assert_eq!((read.mark, read.device.as_str()), (Mark::Dismissed, "old-desk"));
        // A word said here on another call: in both logs, by each one's own id.
        let other = call_id("phone-1", "1793000100000-+33261910000");
        let asked = Asked { kind: "call".into(), ids: vec![other.clone()], targets: Vec::new() };
        let mut path = Recorder::default();
        let at = logs.say(&asked, Mark::Dismissed, &mut path, NOW + 100).unwrap();
        assert!(calls::read_seen(&calls_root).seen.contains("1793000100000-+33261910000"), "an older Sioul reads it as Seen");
        assert_eq!(logs.states(&named).of(&other, State::default()).mark, Mark::Dismissed);
        // A call has no read state: Read is refused, nothing written, no path called.
        assert!(logs.say(&asked, Mark::Read, &mut path, NOW + 200).is_err());
        assert!(path.read.is_empty());
        // Undo: out of both.
        assert!(logs.take_back(&asked, Mark::Dismissed, at, &mut path).unwrap());
        assert!(!calls::read_seen(&calls_root).seen.contains("1793000100000-+33261910000"));
        assert_eq!(logs.states(&named).of(&other, State::default()).mark, Mark::New);
        let _ = std::fs::remove_dir_all(&state);
    }

    /// Read on a message calls the mail's read path with its items (the
    /// client's own `\Seen`), then says it; Undo unreads them and takes the
    /// word back. Dismiss calls no path.
    #[test]
    fn read_for_mail_calls_the_read_path() {
        let state = scratch("read");
        let logs = Logs { folder: state.join(FOLDER), state: state.clone(), device: "desk".into() };
        let id = mail_id("home", Some("<a1@example.org>"), "");
        let asked = Asked { kind: "mail".into(), ids: vec![id.clone()], targets: vec!["/m/home/INBOX/new/1700.M1.desk,U5-40".into()] };
        let mut path = Recorder::default();
        let at = logs.say(&asked, Mark::Read, &mut path, NOW).unwrap();
        assert_eq!(path.read, vec![(Kind::Mail, asked.targets.clone())]);
        assert_eq!(logs.states(&|_, _| None).of(&id, State::default()).mark, Mark::Read);
        assert!(logs.take_back(&asked, Mark::Read, at, &mut path).unwrap());
        assert_eq!(path.unread, vec![(Kind::Mail, asked.targets.clone())]);
        assert_eq!(logs.states(&|_, _| None).of(&id, State::default()).mark, Mark::New);
        logs.say(&asked, Mark::Dismissed, &mut path, NOW + 1).unwrap();
        assert_eq!(path.read.len(), 1, "Dismiss touches nothing else");
        // A mail's word goes to no Seen log.
        assert!(!state.join(calls::FOLDER).exists());
        let _ = std::fs::remove_dir_all(&state);
    }

    /// A word is kept a week after it was said, another device's three days
    /// more, and longer while its notice is still given here (a message
    /// dismissed and still in the inbox); each producer forgets its own kinds.
    #[test]
    fn words_are_forgotten_after_a_week_unless_still_needed() {
        let dir = scratch("forget");
        let (old, kept, phone_old, text) = ("mail:home:0000000000000003", "mail:home:0000000000000004", "mail:home:0000000000000005", "text:phone-1:12:sms-1@1");
        let eight = NOW - 8 * DAY_MS;
        say(&dir, "desk", &[old.to_string()], Mark::Dismissed, eight).unwrap();
        say(&dir, "desk", &[kept.to_string()], Mark::Dismissed, eight).unwrap();
        say(&dir, "phone", &[phone_old.to_string()], Mark::Read, eight).unwrap();
        say(&dir, "desk", &[text.to_string()], Mark::Dismissed, eight).unwrap();
        say(&dir, "desk", &["mail:home:0000000000000006".to_string()], Mark::Dismissed, NOW - DAY_MS).unwrap();
        let still = |id: &str| id == kept;
        assert_eq!(forget(&dir, "desk", &[Kind::Mail], &still, NOW).unwrap(), 1, "the desk's own old word, its message gone");
        let states = States::read(&dir);
        assert_eq!(states.of(old, State::default()).mark, Mark::New);
        assert_eq!(states.of(kept, State::default()).mark, Mark::Dismissed, "still in the inbox: still dismissed");
        assert_eq!(states.of(phone_old, State::default()).mark, Mark::Read, "another device's: three days more");
        assert_eq!(states.of(text, State::default()).mark, Mark::Dismissed, "a text's: its own producer forgets it");
        assert_eq!(forget(&dir, "desk", &[Kind::Mail], &still, NOW + 3 * DAY_MS).unwrap(), 1, "the phone's, three days later");
        assert_eq!(forget(&dir, "desk", &[Kind::Mail], &still, NOW + 7 * DAY_MS).unwrap(), 1, "the one said yesterday, a week on");
        assert_eq!(States::read(&dir).of(kept, State::default()).mark, Mark::Dismissed);
        assert_eq!(forget(&dir, "desk", &[Kind::Text], &|_| false, NOW).unwrap(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A Read word on mail never fights the server: a day on, it gives way
    /// to the message's own `\Seen`, after which an unread elsewhere shows
    /// the message again; a Dismiss stays, and so does another device's Read
    /// while its message is not read here yet.
    #[test]
    fn a_read_word_on_mail_gives_way_to_the_server_after_a_day() {
        let dir = scratch("read-mail");
        let (read, fresh, phones, waits, dismissed, text) = (
            "mail:home:00000000000000c1",
            "mail:home:00000000000000c2",
            "mail:home:00000000000000c3",
            "mail:home:00000000000000c4",
            "mail:home:00000000000000c5",
            "text:phone-1:12:sms-2@1",
        );
        let two_days = NOW - 2 * DAY_MS;
        say(&dir, "desk", &[read.to_string()], Mark::Read, two_days).unwrap();
        say(&dir, "desk", &[fresh.to_string()], Mark::Read, NOW - 3_600_000).unwrap();
        say(&dir, "phone", &[phones.to_string()], Mark::Read, two_days).unwrap();
        say(&dir, "phone", &[waits.to_string()], Mark::Read, two_days).unwrap();
        say(&dir, "desk", &[dismissed.to_string()], Mark::Dismissed, two_days).unwrap();
        say(&dir, "desk", &[text.to_string()], Mark::Read, two_days).unwrap();
        // Here, every message but the last the phone read has its \Seen.
        let seen = |id: &str| id != waits;
        assert_eq!(forget_read_mail(&dir, "desk", &seen, NOW).unwrap(), 2, "the desk's own day-old Read, the phone's its flag confirms");
        let states = States::read(&dir);
        // Marked unread in another client: the server's word is the message's again.
        assert_eq!(states.of(read, State::derived(Mark::New)).mark, Mark::New);
        assert_eq!(states.of(read, State::derived(Mark::Read)).mark, Mark::Read);
        assert_eq!(states.of(phones, State::derived(Mark::New)).mark, Mark::New);
        assert_eq!(states.of(fresh, State::derived(Mark::New)).mark, Mark::Read, "said an hour ago: it still holds");
        assert_eq!(states.of(waits, State::derived(Mark::New)).mark, Mark::Read, "the phone's, not read here yet");
        assert_eq!(states.of(dismissed, State::derived(Mark::New)).mark, Mark::Dismissed, "a Dismiss stays");
        assert_eq!(states.of(text, State::default()).mark, Mark::Read, "a text's Read is not mail's");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A system notification is withdrawn once each of its notices is
    /// settled: a word said anywhere, or its message read or gone here.
    #[test]
    fn a_notification_is_settled_once_each_of_its_notices_is() {
        let dir = scratch("settled");
        let (a, b) = ("mail:home:00000000000000aa".to_string(), "mail:home:00000000000000bb".to_string());
        let ids = vec![a.clone(), b.clone()];
        let unread_b = |id: &str| id != b;
        assert!(!all_settled(&ids, &States::read(&dir), &|_| false));
        say(&dir, "phone", std::slice::from_ref(&a), Mark::Dismissed, NOW).unwrap();
        assert!(!all_settled(&ids, &States::read(&dir), &|_| false), "one of two");
        assert!(all_settled(&ids, &States::read(&dir), &|id| !unread_b(id)), "the other read here");
        assert!(!all_settled(&[], &States::read(&dir), &|_| true), "nothing told: nothing to withdraw");
        // A stored message: unread while a copy without S is there.
        let inbox = dir.join("INBOX");
        std::fs::create_dir_all(inbox.join("new")).unwrap();
        std::fs::create_dir_all(inbox.join("cur")).unwrap();
        let file = inbox.join("new").join("1700.M1P2.desk,U5-40");
        // Flags after the platform's separator (":" is no file name on Windows).
        let flagged = |flags: &str| inbox.join("cur").join(format!("1700.M1P2.desk,U5-40{}2,{flags}", crate::maildir::INFO));
        std::fs::write(&file, "x").unwrap();
        assert!(message_unread(&file));
        std::fs::rename(&file, flagged("F")).unwrap();
        assert!(message_unread(&file), "flagged, still unread");
        std::fs::rename(flagged("F"), flagged("FS")).unwrap();
        assert!(!message_unread(&file), "read");
        std::fs::remove_file(flagged("FS")).unwrap();
        assert!(!message_unread(&file), "gone");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_line_without_a_word_is_a_dismiss() {
        let line: Line = serde_json::from_str(r#"{"at":1,"ids":["a"]}"#).unwrap();
        assert_eq!(line.state, Mark::Dismissed);
        let written = serde_json::to_string(&Line { at: 2, ids: vec!["b".into()], state: Mark::Read }).unwrap();
        assert_eq!(written, r#"{"at":2,"ids":["b"],"state":"read"}"#);
        // An older Sioul's own reading of it: a Seen of "b", its word passed over.
        let older: calls::SeenLine = serde_json::from_str(&written).unwrap();
        assert_eq!(older.ids, ["b"]);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The phone's messages on your computers (docs/android.md, "Messages on
//! your computers"; docs/database.md, "The phone's messages"; docs/porch.md,
//! "From your phone"; research: docs/research/sms.md, phase a).
//!
//! The listener (AppNotes.java, `appnotes`) sees each notification of
//! another app. For the apps switched on for your computers (the phone's SMS
//! app unless you say otherwise; any other app, a chat's, when you choose
//! it), and while the part "Messages from your phone" is on, the phone writes
//! a **line** per message in its own log, `phone-messages/log/<device>.jsonl`
//! (`lines_of`, `carry_into`), which the sharing carries, sealed, to your
//! other devices. A line says who wrote, when, in which conversation, and
//! the words, cut at 1,000 characters; or only who and when, for an app set
//! so; "a picture" for a picture, never the picture.
//!
//! **Never**: a code or an approval (its line says a code came, and that it
//! stays on the phone), a copy Android redacted (the same), anything of a
//! blocked sender or of a conversation held for good, a notification its
//! app marked secret, ongoing ones, Sioul's own, a button or a tap. Each line
//! is padded to a multiple of 256 bytes before it is sealed (`padded`): the
//! server learns no message's length.
//!
//! A computer's Porch lists them under "From your phone" (`lines`): one line
//! per conversation, part of the day and day, once the phone let the
//! notification through (`Line::shows`) and while its sender may reach you
//! by that device's matrix (`reaches_now`), never as a notification of its
//! own, never counted. **Seen** is each device's own log of lines,
//! `phone-messages/seen/<device>.jsonl`, written and read as the calls' are
//! (`calls::mark_seen`, `calls::read_seen` with this folder): seen on one
//! device, gone from all. A phone takes its own lines out after `KEPT_DAYS`;
//! any device takes another's out `LATE_DAYS` later (`trim_own`,
//! `trim_others`), so that a phone gone for good leaves nothing behind for
//! ever.
//!
//! What the phone sends, per app, is its own choice (`Choices`,
//! `phone-messages.toml` in its configuration), never shared.

use crate::appnotes::{self, Decision, Kind, Person, Posted, Sender, Why};
use crate::attention::{self, Attention, Column, Event, Level, Row, Source};
use crate::calls;
use crate::i18n::Translator;
use crate::phones::{self, Region};
use crate::reach::{Channel, Who};
use jiff::Zoned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The sharing's part that carries them.
pub const PART: &str = "phone-messages";
/// Their folder, in the state folder.
pub const FOLDER: &str = "phone-messages";
/// The folder of the phones' logs, one file per phone, written by that phone alone; shared.
pub const LOG: &str = "log";
/// The folder of the lines marked seen, one file per device; shared. The
/// calls' name: their functions read and write it under this folder too.
pub const SEEN_LOG: &str = calls::SEEN_LOG;
/// A phone takes its own lines out after this many days.
pub const KEPT_DAYS: i64 = 7;
/// Another device's lines are taken out by any device this many days later.
pub const LATE_DAYS: i64 = 3;
/// A message's words are cut after this many characters.
pub const TEXT_MAX: usize = 1000;
/// Each line's length, in bytes, is a multiple of this.
pub const PAD_STEP: usize = 256;
/// The phone's choices, in its configuration folder.
pub const CHOICES_FILE: &str = "phone-messages.toml";
/// Android's `Notification.VISIBILITY_SECRET`: kept off the lock screen, kept on the phone.
pub const SECRET: i32 = -1;
/// A name is cut after this many characters.
const NAME_MAX: usize = 80;
/// A day, in milliseconds.
const DAY_MS: i64 = 86_400_000;

/// The folder of the phones' messages.
pub fn folder() -> PathBuf {
    crate::config::state_dir().join(FOLDER)
}

// ---------------------------------------------------------------- the phone's choices

/// What an app's notifications send to your computers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Send {
    /// As Sioul does unless you say: the words for the phone's SMS app, nothing for every other app.
    #[default]
    Usual,
    /// Nothing leaves the phone.
    Off,
    /// Who wrote and when, without the words.
    Who,
    /// Who, when and the words.
    Words,
}

impl Send {
    pub const ALL: [Send; 4] = [Send::Usual, Send::Off, Send::Who, Send::Words];

    pub fn id(self) -> &'static str {
        match self {
            Send::Usual => "usual",
            Send::Off => "off",
            Send::Who => "who",
            Send::Words => "words",
        }
    }

    pub fn read(id: &str) -> Option<Send> {
        Send::ALL.into_iter().find(|s| s.id() == id)
    }

    /// What it sends: "as usual" made the words for the SMS app, nothing for the others.
    pub fn resolved(self, sms_app: bool) -> Send {
        match self {
            Send::Usual if sms_app => Send::Words,
            Send::Usual => Send::Off,
            other => other,
        }
    }
}

/// An app's choice, with its name as last seen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppChoice {
    #[serde(default)]
    pub send: Send,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

/// The phone's choices (`$XDG_CONFIG_HOME/sioul/phone-messages.toml`): never
/// shared, the phone decides what leaves it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choices {
    /// The phone's default SMS app, as the listener last saw it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sms_app: String,
    /// By the app's package; an app not here sends as usual.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub app: BTreeMap<String, AppChoice>,
}

impl Choices {
    pub fn default_path() -> PathBuf {
        crate::config::config_dir().join(CHOICES_FILE)
    }

    /// As kept; what does not read is no choice at all (each app as usual).
    pub fn load(path: &Path) -> Choices {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written whole beside, then put in place, yours alone.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let fresh = path.with_file_name(format!(".{name}.{}.new", std::process::id()));
        let _ = std::fs::remove_file(&fresh);
        calls::append_private(&fresh, &text)?;
        std::fs::rename(&fresh, path).map_err(|e| {
            let _ = std::fs::remove_file(&fresh);
            format!("{}: {e}", path.display())
        })
    }

    /// What an app sends (`sms_app`: Android says it is the phone's SMS app).
    pub fn send(&self, package: &str, sms_app: bool) -> Send {
        let chosen = self.app.get(package).map_or(Send::Usual, |c| c.send);
        chosen.resolved(sms_app || (!self.sms_app.is_empty() && package == self.sms_app))
    }

    /// An app's choice written; "as usual" takes it out.
    pub fn set(&mut self, package: &str, send: Send, label: &str) {
        if send == Send::Usual {
            self.app.remove(package);
        } else {
            self.app.insert(package.to_string(), AppChoice { send, label: label.to_string() });
        }
    }
}

// ---------------------------------------------------------------- what Java says besides

/// One message's own, besides `appnotes::Message`: when it was sent (ms, 0
/// unsaid) and whether it carries a picture.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ExtraMessage {
    pub at: i64,
    pub picture: bool,
}

/// What Java says of a notification besides `appnotes::Posted`, in the same
/// JSON (AppNotes.java, `describe`), which `Posted` passes over.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Extra {
    /// `Notification.visibility`: 1 public, 0 private, -1 secret.
    pub visibility: i32,
    /// `Notification.when` (ms); 0 unsaid.
    pub when: i64,
    /// When Android posted it (ms).
    pub posted: i64,
    /// A picture of its own (`BigPictureStyle`).
    pub picture: bool,
    /// Its messages', in `Posted::messages`' order.
    pub messages: Vec<ExtraMessage>,
}

impl Extra {
    /// From the listener's JSON; nothing said, when it does not read.
    pub fn read(json: &str) -> Extra {
        serde_json::from_str(json).unwrap_or_default()
    }
}

// ---------------------------------------------------------------- a line

/// A message as a phone's log keeps it and the sharing carries it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// When it was sent (ms, as the app said; else when the phone heard it).
    pub at: i64,
    /// `<at>-<hash>`: the same message, posted again, is the same line.
    pub id: String,
    /// The app's package, and its name as the phone shows it.
    pub app: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// "text" (an SMS app's), "chat", "mail" (a mail app's), "app" (another app's own).
    pub kind: String,
    /// The conversation's key (`appnotes::talk_key`); "" none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub talk: String,
    /// A group's title.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub group: bool,
    /// Who wrote, as the app named them.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Their number as Sioul keys it (`phones::key`), when the app gave one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    /// The row the phone judged them by: "safe", "neutral", "restricted",
    /// "stranger"; "automaton" (a service's text, an app's own), "at-once"
    /// (an app you let through at once); "" for a code's line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub who: String,
    /// The Always through list holds them (the phone's judgement).
    #[serde(default, skip_serializing_if = "is_false")]
    pub always: bool,
    /// The words, cut at `TEXT_MAX` characters.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// It carried a picture, which stayed on the phone.
    #[serde(default, skip_serializing_if = "is_false")]
    pub picture: bool,
    /// A code or an approval came: said, never carried.
    #[serde(default, skip_serializing_if = "is_false")]
    pub code: bool,
    /// Its words stayed on the phone: its app sends who and when only.
    #[serde(default, skip_serializing_if = "is_false")]
    pub withheld: bool,
    /// When the phone let its notification through (ms): no device shows it before.
    #[serde(default)]
    pub shows: i64,
    /// Spaces that make the line's length a multiple of `PAD_STEP` bytes (`padded`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pad: String,
    /// The phone that wrote it: its name in the sharing, its log's file's name. Never written.
    #[serde(skip)]
    pub device: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// A line as written: its JSON padded with spaces to a multiple of
/// `PAD_STEP` bytes, no newline. Two messages of different lengths under
/// the same step weigh the same once sealed.
pub fn padded(line: &Line) -> String {
    let bare = Line { pad: String::new(), device: String::new(), ..line.clone() };
    let text = serde_json::to_string(&bare).unwrap_or_default();
    if text.len() % PAD_STEP == 0 {
        return text;
    }
    // `,"pad":""` and the spaces inside it.
    let room = text.len() + 9;
    let spaces = (PAD_STEP - room % PAD_STEP) % PAD_STEP;
    serde_json::to_string(&Line { pad: " ".repeat(spaces), ..bare }).unwrap_or(text)
}

/// A name on one line, cut: an app's words never break Sioul's.
fn one_line(text: &str, max: usize) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    cut(&joined, max)
}

/// At most `max` characters, "…" said where it was cut.
fn cut(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", text[..at].trim_end()),
        None => text.to_string(),
    }
}

/// A message's words as they travel: trimmed, cut at `TEXT_MAX` characters.
pub fn words_of(text: &str) -> String {
    cut(text.trim(), TEXT_MAX)
}

// ---------------------------------------------------------------- what travels

/// A notification as the phone's listener decided it, with what you chose for its app.
pub struct Notice<'a> {
    pub posted: &'a Posted,
    pub extra: &'a Extra,
    pub kind: &'a Kind,
    /// Who wrote, as your lists say (`appnotes::decide`'s), none when nobody knows them.
    pub who: Option<Who>,
    /// The Always through list holds them.
    pub listed: bool,
    pub decision: &'a Decision,
    /// What its app sends, as chosen (`Choices::send`).
    pub send: Send,
    pub now_ms: i64,
    pub region: Option<&'static Region>,
    pub words: &'a crate::words::Words,
}

/// Whether a message's sender is the person themselves, as the conversation names them.
fn is_user(p: &Posted, sender: &Person) -> bool {
    p.user.as_ref().is_some_and(|user| {
        let (key, name) = (user.key.trim(), one_line(&user.name, NAME_MAX));
        (!key.is_empty() && key == sender.key.trim()) || (!name.is_empty() && name == one_line(&sender.name, NAME_MAX))
    })
}

/// A person's number as Sioul keys it: the app's `tel:`, else the phone's
/// address book's first, else their name when it is a number; "" none.
fn number_key(person: Option<&Person>, fallback_name: &str, region: Option<&Region>) -> String {
    let from_uri = person.and_then(|p| p.uri.trim().strip_prefix("tel:")).map(str::to_string);
    let from_book = person.and_then(|p| p.numbers.iter().find(|n| !n.trim().is_empty()).cloned());
    let from_name = appnotes::number_in(person.map_or(fallback_name, |p| p.name.as_str()));
    from_uri.or(from_book).or(from_name).map(|n| phones::key(&n.replace("%2B", "+"), region)).unwrap_or_default()
}

/// Who a code came from, as safe to say: the last sender's name, else the
/// conversation's, else the title; the app's name when that name may hold
/// the code itself (four digits in a row that are no phone number, a short
/// number included), or nothing.
fn code_sender(p: &Posted, words: &crate::words::Words) -> String {
    let last = p.messages.iter().rev().find_map(|m| m.sender.as_ref().filter(|s| !is_user(p, s))).map(|s| s.name.as_str());
    let named = [last.unwrap_or(""), p.people.first().map_or("", |s| s.name.as_str()), p.conversation.as_str(), p.title.as_str()].into_iter().map(|n| one_line(n, NAME_MAX)).find(|n| !n.is_empty()).unwrap_or_default();
    // Four digits in a row that are no phone number may be the code itself ("G-482913"): never said.
    let mut run = 0;
    let coded = named.chars().any(|c| {
        run = if c.is_ascii_digit() { run + 1 } else { 0 };
        run >= 4
    });
    if named.is_empty() || crate::codes::detect(&words.codes, &named, "").is_some() || (coded && appnotes::number_in(&named).is_none()) {
        one_line(&p.app, NAME_MAX)
    } else {
        named
    }
}

/// The lines a notification makes, none when nothing of it may leave the
/// phone: its app off for your computers, Sioul's own, an ongoing one, a
/// summary, a notification its app marked secret, a blocked sender's, a
/// conversation held for good, what is never held (calls, alarms), a missed
/// call (the calls' part carries calls). A code, an approval or a copy
/// Android redacted: one line that says a code came, without it. Otherwise
/// one line per message of someone else's (a conversation's earlier messages
/// are the same lines again: `carry_into` keeps each once), or one for the
/// notification; who and when only for an app set so; never older than the
/// lines are kept.
pub fn lines_of(n: &Notice) -> Vec<Line> {
    let p = n.posted;
    let send = n.send;
    if matches!(send, Send::Off | Send::Usual) || p.package.is_empty() || p.package == appnotes::OWN || p.ongoing || p.summary || p.full_screen || n.extra.visibility == SECRET {
        return Vec::new();
    }
    if n.who == Some(Who::Blocked) || n.decision.why == Why::Never {
        return Vec::new();
    }
    let heard = [n.extra.when, n.extra.posted].into_iter().find(|t| *t > 0).unwrap_or(n.now_ms);
    let shows = n.decision.until.map_or(n.now_ms, |until| until.saturating_mul(1000)).max(heard.min(n.now_ms));
    let sms = p.sms_app || matches!(n.kind, Kind::People(talk) if talk.sms);
    let label = one_line(&p.app, NAME_MAX);
    let base = Line { app: p.package.clone(), label: label.clone(), shows, ..Line::default() };
    let mut lines = match n.kind {
        Kind::Untouched => return Vec::new(),
        Kind::Code => {
            let name = code_sender(p, n.words);
            vec![Line { at: heard, kind: (if sms { "text" } else { "app" }).into(), name, code: true, shows: heard.min(n.now_ms), ..base }]
        }
        Kind::People(talk) => {
            if talk.via == Channel::Calls {
                return Vec::new();
            }
            let kind = if sms { "text" } else if talk.via == Channel::Mail { "mail" } else { "chat" };
            // A text from a sender named in letters that no address book knows: a service's.
            let service = sms && n.who.is_none() && matches!(talk.sender, Sender::Named { .. });
            let who = if service { "automaton".to_string() } else { n.who.unwrap_or(Who::Stranger).id().to_string() };
            let (talk_key, title, group) = talk.conversation.as_ref().map_or((String::new(), String::new(), false), |c| (c.key.clone(), if c.group { one_line(&c.title, NAME_MAX) } else { String::new() }, c.group));
            let line = Line { kind: kind.into(), talk: talk_key, title, group, who, always: n.listed && !group, ..base };
            messages_of(n, line, heard)
        }
        Kind::Automaton { .. } | Kind::AtOnce => {
            let who = if matches!(n.kind, Kind::AtOnce) { "at-once" } else { "automaton" };
            let line = Line { kind: (if sms { "text" } else { "app" }).into(), who: who.into(), ..base };
            messages_of(n, line, heard)
        }
    };
    for line in &mut lines {
        if send == Send::Who && !line.code {
            line.text.clear();
            line.picture = false;
            line.withheld = true;
        }
        line.id = format!("{}-{}", line.at, appnotes::talk_key(&line.app, &format!("{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}", line.talk, line.name, line.key, line.text, line.code)));
    }
    lines.retain(|l| l.at > 0 && n.now_ms - l.at <= KEPT_DAYS * DAY_MS);
    lines
}

/// A conversation's messages from others, each a line made from `line`;
/// without their own times (an older Java), its last one only, at `heard`.
/// No messages: the notification itself, its title the sender.
fn messages_of(n: &Notice, line: Line, heard: i64) -> Vec<Line> {
    let p = n.posted;
    let region = n.region;
    let from_others: Vec<(usize, &appnotes::Message, &Person)> = p.messages.iter().enumerate().filter_map(|(i, m)| m.sender.as_ref().filter(|s| !is_user(p, s)).map(|s| (i, m, s))).collect();
    if !from_others.is_empty() {
        let timed = from_others.iter().all(|(i, _, _)| n.extra.messages.get(*i).is_some_and(|e| e.at > 0));
        let chosen: Vec<&(usize, &appnotes::Message, &Person)> = if timed { from_others.iter().collect() } else { from_others.last().into_iter().collect() };
        // A conversation between two: its number is the conversation's, for Text back.
        let theirs = (!line.group).then(|| p.people.iter().find(|s| s.uri.trim().starts_with("tel:"))).flatten();
        return chosen
            .into_iter()
            .map(|(i, message, sender)| {
                let extra = n.extra.messages.get(*i).cloned().unwrap_or_default();
                let key = Some(number_key(Some(sender), "", region)).filter(|k| !k.is_empty()).unwrap_or_else(|| theirs.map(|t| number_key(Some(t), "", region)).unwrap_or_default());
                Line { at: if timed { extra.at } else { heard }, name: one_line(&sender.name, NAME_MAX), key, text: words_of(&message.text), picture: extra.picture, ..line.clone() }
            })
            .collect();
    }
    let person = p.people.first();
    let name = [person.map_or("", |s| s.name.as_str()), p.title.as_str()].into_iter().map(|t| one_line(t, NAME_MAX)).find(|t| !t.is_empty()).unwrap_or_default();
    let words = [p.big.as_str(), p.text.as_str()].into_iter().find(|t| !t.trim().is_empty()).map(str::to_string).unwrap_or_else(|| p.lines.join("\n"));
    vec![Line { at: heard, key: number_key(person, &name, region), name, text: words_of(&words), picture: n.extra.picture, ..line }]
}

// ---------------------------------------------------------------- the logs

/// The lines of one file, oldest first; a line that does not read (half written) is passed over.
pub fn read_file(path: &Path) -> Vec<Line> {
    let mut lines: Vec<Line> = calls::read_lines::<Line>(path).into_iter().filter(|l| l.at > 0 && !l.id.is_empty()).collect();
    lines.sort_by_key(|l| l.at);
    lines
}

/// Every phone's log under `root` (this folder), each line with its phone
/// (`device`), oldest first: never a blocked sender's, never one kept past
/// `KEPT_DAYS` before `now_ms` (a phone that stopped sharing never takes its
/// own out).
pub fn read_logs(root: &Path, now_ms: i64) -> Vec<Line> {
    let mut lines: Vec<Line> = calls::device_files(&root.join(LOG))
        .into_iter()
        .flat_map(|(device, path)| read_file(&path).into_iter().map(move |l| Line { device: device.clone(), ..l }))
        .filter(|l| l.who != "blocked" && now_ms - l.at <= KEPT_DAYS * DAY_MS)
        .collect();
    lines.sort_by(|a, b| (a.at, &a.device, &a.id).cmp(&(b.at, &b.device, &b.id)));
    lines
}

/// A phone's new lines added to its own log (`own`), under its lock, read
/// again there: those not in it yet (by id), none past `KEPT_DAYS` (taken
/// out already, it would come back), each padded. Two processes adding at
/// once never write a line twice. Returns how many were written.
pub fn carry_into(own: &Path, lines: &[Line], now_ms: i64) -> Result<usize, String> {
    let fresh: Vec<&Line> = lines.iter().filter(|l| l.at > 0 && !l.id.is_empty() && now_ms - l.at <= KEPT_DAYS * DAY_MS).collect();
    if fresh.is_empty() {
        return Ok(0);
    }
    crate::filelock::with_lock(own, || {
        let mut known: BTreeSet<String> = read_file(own).into_iter().map(|l| l.id).collect();
        let mut text = String::new();
        for line in fresh {
            if known.insert(line.id.clone()) {
                text.push_str(&padded(line));
                text.push('\n');
            }
        }
        let written = text.lines().count();
        if written > 0 {
            calls::append_private(own, &text)?;
        }
        Ok(written)
    })
}

/// A line older than `days` before `now_ms`, by its `at`; a line that does not read is kept.
fn older_than(days: i64, now_ms: i64) -> impl Fn(&str) -> bool {
    move |line: &str| serde_json::from_str::<serde_json::Value>(line).ok().and_then(|v| v["at"].as_i64()).is_some_and(|at| now_ms - at > days * DAY_MS)
}

/// This device's own lines kept past `KEPT_DAYS` taken out of its log and of
/// its seen log under `root`: the sharing then takes them out on every
/// device. Returns how many went.
pub fn trim_own(root: &Path, device: &str, now_ms: i64) -> Result<usize, String> {
    let old = older_than(KEPT_DAYS, now_ms);
    Ok(calls::rewrite_lines(&calls::own_file(root, LOG, device), &old)? + calls::rewrite_lines(&calls::own_file(root, SEEN_LOG, device), &old)?)
}

/// Another device's lines kept `LATE_DAYS` past their week taken out here
/// (`here`: this device): a phone gone for good never takes its own out.
/// While it shares, it took them out before (`trim_own`). Returns how many went.
pub fn trim_others(root: &Path, here: &str, now_ms: i64) -> Result<usize, String> {
    let old = older_than(KEPT_DAYS + LATE_DAYS, now_ms);
    let mut gone = 0;
    for folder in [LOG, SEEN_LOG] {
        for (device, path) in calls::device_files(&root.join(folder)) {
            if device != calls::file_safe(here) {
                gone += calls::rewrite_lines(&path, &old)?;
            }
        }
    }
    Ok(gone)
}

// ---------------------------------------------------------------- when a computer shows one

/// Whether a line's sender may reach you at `now` by this device's matrix
/// (`attention`): a code's line at any time; a service's text or an app's
/// own by the automatons' row (or At once's), shown at its times, gathered
/// included; someone's message by their Messages row (Mail's for a mail
/// app's), a group's by the groups' row, Always through's when the list
/// holds them, at once. The app's and the conversation's own rows, set on
/// the phone, are read over them. A row that lets them through at no time
/// shows them in work and admin time, as the calls' list does: no message is
/// never listed. `who`: as this device's lists judge their number now, else
/// as the phone judged them.
pub fn reaches_now(attention: &Attention, line: &Line, who: Option<Who>, always: bool, now: &attention::Now) -> bool {
    if line.code {
        return true;
    }
    let sources = attention::source_rows(&line.app, &line.talk);
    if matches!(line.who.as_str(), "automaton" | "at-once") {
        let kind = if line.who == "at-once" { attention::Kind::AppAtOnce } else { attention::Kind::AppAutomatons };
        return matches!(attention.decide(&Event::own(kind).from_sources(sources), now).level, Level::Now | Level::Quiet | Level::Gathered);
    }
    let who = who.or_else(|| Who::read(&line.who)).unwrap_or(Who::Stranger);
    if who == Who::Blocked {
        return false;
    }
    let via = if line.kind == "mail" { Channel::Mail } else { Channel::Messages };
    let always = always && !line.group;
    if attention.decide(&Event::of(Source::Message { via, who, group: line.group, always }).from_sources(sources.clone()), now).level == Level::Now {
        return true;
    }
    let person = if line.group { attention::Person::Groups } else { attention::Person::of(who) };
    let row = Row::People(via, person);
    let never = !always && !attention.has_sources(&sources) && Column::TIMES.iter().all(|c| attention.cell(row, *c) != Level::Now);
    never && !now.dnd && now.times.iter().any(|c| matches!(c, Column::Work | Column::Admin))
}

// ---------------------------------------------------------------- the Porch's lines

/// One message of a Porch's line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Said {
    /// "09:41".
    pub time: String,
    /// Who wrote it, in a group; "" otherwise.
    pub from: String,
    /// Its words; "a picture"; "" when they stayed on the phone.
    pub text: String,
}

/// A line of the Porch's "From your phone": a conversation, in one part of
/// one day, on one phone; or a code that came.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PorchLine {
    /// The messages it says, to mark them seen.
    pub ids: Vec<String>,
    /// "While you slept, Dr Martin's office wrote:"; "A code came from your
    /// bank at 09:12. It stays on your phone."
    pub head: String,
    pub said: Vec<Said>,
    /// "Their words stay on your phone: WhatsApp sends who and when only."; "" otherwise.
    pub note: String,
    /// "01 99 00 12 34"; "" when no number came with it.
    pub number: String,
    /// What Text back and Call back dial ("+33199001234"); "" none.
    pub dial: String,
    /// In your address books: no "Add to contacts".
    pub known: bool,
    /// A text from a whole number, between two: Text back and Call back offered.
    pub texted: bool,
    pub code: bool,
}

/// What the Porch's lines need besides the logs.
pub struct Lister<'a> {
    pub now: &'a Zoned,
    pub tr: &'a Translator,
    pub region: Option<&'static Region>,
    /// The name your address books give a number's key; none: not in them.
    pub name_of: &'a dyn Fn(&str) -> Option<String>,
    /// Whether this line's sender may reach you now (`reaches_now`).
    pub shows: &'a dyn Fn(&Line) -> bool,
    /// The phone a line came from, as this device says it: "" for this
    /// device's own, "your phone", "your phone (GS290)" (`calls::phone_words`).
    pub phone: &'a dyn Fn(&str) -> String,
    /// The part of the day a moment (ms) fell in: a column of the matrix ("work", "sleep"…); "" unknown.
    pub column: &'a dyn Fn(i64) -> String,
}

/// "while you slept": the part of the day, in a few words (the calls' words).
fn context(tr: &Translator, column: &str) -> String {
    match column {
        "work" | "admin" | "leisure" | "meals" | "sleep" | "pause" | "free" | "slot" | "dnd" => calls::context(tr, column),
        _ => tr.text("phonemsgs-context-any", None),
    }
}

/// Who wrote, in words: your address books' name for their number, else
/// the app's name for them (a number written as you read numbers), else "a
/// number not in your contacts", else "someone".
fn who_words(line: &Line, l: &Lister) -> (String, bool) {
    if let Some(name) = (!line.key.is_empty()).then(|| (l.name_of)(&line.key)).flatten() {
        return (name, true);
    }
    let name = line.name.trim();
    if !name.is_empty() && appnotes::number_in(name).is_none() {
        return (name.to_string(), false);
    }
    if !line.key.is_empty() || !name.is_empty() {
        return (l.tr.text("calls-who-stranger", None), false);
    }
    (l.tr.text("phonemsgs-who-someone", None), false)
}

/// A message's words as a Porch's line says them.
fn said_text(line: &Line, tr: &Translator) -> String {
    if line.withheld {
        return String::new();
    }
    match (line.text.trim().is_empty(), line.picture) {
        (true, true) => tr.text("phonemsgs-picture", None),
        (false, true) => {
            let mut args = crate::i18n::args();
            args.set("text", line.text.clone());
            tr.text("phonemsgs-with-picture", Some(&args))
        }
        (true, false) => tr.text("phonemsgs-no-words", None),
        (false, false) => line.text.clone(),
    }
}

/// The Porch's lines, oldest first: the messages of the last week not seen
/// on any device, once the phone let them through and while their sender
/// may reach you; never a blocked sender's. One line per phone,
/// conversation (or sender), part of the day and day; a code its own line.
/// `all` are every phone's (`read_logs`), oldest first.
pub fn lines(all: &[Line], seen: &calls::Seen, l: &Lister) -> Vec<PorchLine> {
    let now_ms = l.now.timestamp().as_millisecond();
    let shown: Vec<&Line> = all.iter().filter(|m| m.who != "blocked" && !seen.seen.contains(&m.id) && now_ms - m.at <= KEPT_DAYS * DAY_MS && m.at <= now_ms + 60_000 && now_ms >= m.shows && (l.shows)(m)).collect();
    type Group<'a> = (String, String, jiff::civil::Date, Vec<&'a Line>);
    let mut groups: Vec<Group> = Vec::new();
    for m in shown {
        let column = (l.column)(m.at);
        let date = calls::local(m.at, l.now).date();
        let who = if m.code {
            format!("code\u{1f}{}", m.id)
        } else if !m.talk.is_empty() {
            m.talk.clone()
        } else if !m.key.is_empty() {
            m.key.clone()
        } else {
            format!("{}\u{1f}{}", m.app, m.name)
        };
        let group_key = format!("{}\u{1f}{who}", m.device);
        match groups.iter_mut().find(|(k, c, d, _)| *k == group_key && *c == column && *d == date) {
            Some((_, _, _, group)) => group.push(m),
            None => groups.push((group_key, column, date, vec![m])),
        }
    }
    groups.into_iter().map(|(_, column, date, group)| porch_line(&group, &column, date, l)).collect()
}

fn porch_line(group: &[&Line], column: &str, date: jiff::civil::Date, l: &Lister) -> PorchLine {
    let tr = l.tr;
    let first = group[0];
    let time = |m: &Line| calls::local(m.at, l.now).strftime("%H:%M").to_string();
    let today = l.now.date();
    let ids: Vec<String> = group.iter().map(|m| m.id.clone()).collect();
    let keyed = group.iter().find(|m| !m.key.is_empty() && !m.group).map(|m| m.key.clone()).unwrap_or_default();
    let (who, known) = who_words(first, l);
    // Which phone, only among several: the section says "From your phone" already.
    let phone = Some((l.phone)(&first.device)).filter(|p| *p != tr.text("calls-phone-yours", None) && *p != tr.text("calls-phone-other", None)).unwrap_or_default();
    if first.code {
        let mut args = crate::i18n::args();
        args.set("who", who);
        args.set("time", time(first));
        let head = if date == today {
            tr.text("phonemsgs-code", Some(&args))
        } else {
            args.set("day", calls::day_words(tr, date, today));
            tr.text("phonemsgs-code-day", Some(&args))
        };
        return PorchLine { ids, head: calls::capitalized(&head), said: Vec::new(), note: String::new(), number: String::new(), dial: String::new(), known, texted: false, code: true };
    }
    let mut context_said = context(tr, column);
    if date != today {
        let mut args = crate::i18n::args();
        args.set("day", calls::day_words(tr, date, today));
        args.set("context", context_said);
        context_said = tr.text("calls-context-day", Some(&args));
    }
    // An SMS app needs no naming; a chat's or a mail app's does.
    let app = if first.kind == "text" { String::new() } else { first.label.clone() };
    let mut args = crate::i18n::args();
    args.set("context", context_said);
    args.set("who", who);
    args.set("group", first.title.clone());
    args.set("app", app.clone());
    args.set("phone", phone.clone());
    let shape = if first.group { "-group" } else { "" };
    let with_app = if app.is_empty() { "" } else { "-app" };
    let on = if phone.is_empty() { "" } else { "-on" };
    let head = calls::capitalized(&tr.text(&format!("phonemsgs-head{shape}{with_app}{on}"), Some(&args)));
    let said = group
        .iter()
        .map(|m| Said {
            time: time(m),
            from: if m.group { (!m.key.is_empty()).then(|| (l.name_of)(&m.key)).flatten().unwrap_or_else(|| m.name.clone()) } else { String::new() },
            text: said_text(m, tr),
        })
        .collect();
    let note = if group.iter().any(|m| m.withheld) {
        let mut args = crate::i18n::args();
        args.set("app", if first.label.is_empty() { first.app.clone() } else { first.label.clone() });
        tr.text("phonemsgs-withheld", Some(&args))
    } else {
        String::new()
    };
    let texted = first.kind == "text" && !first.group && first.who != "automaton" && phones::is_whole(&keyed);
    PorchLine {
        ids,
        head,
        said,
        note,
        number: if keyed.is_empty() { String::new() } else { calls::shown_number(&keyed, l.region) },
        dial: if phones::is_whole(&keyed) { keyed.clone() } else { String::new() },
        known: known || (!keyed.is_empty() && (l.name_of)(&keyed).is_some()),
        texted,
        code: false,
    }
}

/// A person's messages of the week (`keys`, their numbers as Sioul keys
/// them), newest first, from every phone: a sentence each, for their sheet,
/// never a count; codes said without them, words withheld said so. A
/// blocked sender's never come: they never leave the phone.
pub fn history(all: &[Line], keys: &BTreeSet<String>, l: &Lister) -> Vec<String> {
    let tr = l.tr;
    let now_ms = l.now.timestamp().as_millisecond();
    all.iter()
        .rev()
        .filter(|m| !m.key.is_empty() && keys.contains(&m.key) && m.who != "blocked" && now_ms - m.at <= KEPT_DAYS * DAY_MS && m.at <= now_ms + 60_000)
        .map(|m| {
            let at = calls::local(m.at, l.now);
            let mut args = crate::i18n::args();
            args.set("day", calls::day_words(tr, at.date(), l.now.date()));
            args.set("time", at.strftime("%H:%M").to_string());
            let when = tr.text("calls-history-when", Some(&args));
            let mut args = crate::i18n::args();
            args.set("when", when);
            args.set("app", if m.label.is_empty() { m.app.clone() } else { m.label.clone() });
            args.set("text", said_text(m, tr));
            let id = if m.code {
                "phonemsgs-history-code"
            } else if m.withheld {
                "phonemsgs-history-withheld"
            } else if m.kind == "text" {
                "phonemsgs-history-line"
            } else {
                "phonemsgs-history-app"
            };
            calls::capitalized(&tr.text(id, Some(&args)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appnotes::{Choices as AppChoices, Message};

    /// Thursday 8 October 2026, 10:00 in Paris.
    const NOW: i64 = 1_791_446_400_000;

    fn fr() -> Option<&'static Region> {
        phones::region_named("FR")
    }

    fn words() -> std::sync::Arc<crate::words::Words> {
        crate::words::Words::of(&crate::config::Config::default())
    }

    fn person(name: &str, uri: &str) -> Person {
        Person { name: name.into(), key: uri.into(), uri: uri.into(), ..Person::default() }
    }

    /// The phone's SMS app's notification of a conversation: its messages,
    /// each sent a minute apart, the first ten minutes before `NOW`.
    fn texted(from: &Person, texts: &[&str]) -> (Posted, Extra) {
        let posted = Posted {
            key: "0|foundation.e.message|1|null|10123".into(),
            package: "foundation.e.message".into(),
            app: "Message".into(),
            category: "msg".into(),
            template: "android.app.Notification$MessagingStyle".into(),
            title: from.name.clone(),
            text: texts.last().map_or(String::new(), |t| t.to_string()),
            messages: texts.iter().map(|t| Message { sender: Some(from.clone()), text: t.to_string() }).collect(),
            people: vec![Person { uri: from.uri.clone(), ..Person::default() }],
            sms_app: true,
            ..Posted::default()
        };
        let count = texts.len() as i64;
        let extra = Extra { visibility: 0, when: NOW - 60_000, posted: NOW - 59_000, picture: false, messages: (0..count).map(|i| ExtraMessage { at: NOW - 600_000 + i * 60_000, picture: false }).collect() };
        (posted, extra)
    }

    fn decided(posted: &Posted, extra: &Extra, who: Option<Who>, decision: Decision, send: Send) -> Vec<Line> {
        let words = words();
        let kind = appnotes::classify(posted, &AppChoices::default(), &words);
        lines_of(&Notice { posted, extra, kind: &kind, who, listed: false, decision: &decision, send, now_ms: NOW, region: fr(), words: &words })
    }

    fn now_through() -> Decision {
        Decision::through(Why::Allowed)
    }

    #[test]
    fn a_text_makes_a_line_per_message_with_its_number_and_words() {
        let martin = person("Cabinet du Dr Martin", "tel:+33199001234");
        let (posted, extra) = texted(&martin, &["Bonjour,", "Votre rendez-vous est déplacé à jeudi 10 h 30."]);
        let lines = decided(&posted, &extra, Some(Who::Neutral), now_through(), Send::Words);
        assert_eq!(lines.len(), 2);
        let last = &lines[1];
        assert_eq!((last.kind.as_str(), last.name.as_str(), last.key.as_str(), last.who.as_str()), ("text", "Cabinet du Dr Martin", "+33199001234", "neutral"));
        assert_eq!(last.text, "Votre rendez-vous est déplacé à jeudi 10 h 30.");
        assert_eq!((last.at, last.shows), (NOW - 540_000, NOW));
        assert!(last.id.starts_with(&format!("{}-", NOW - 540_000)));
        // Posted again with a new message: the same lines again, and the new one.
        let (again, extra) = texted(&martin, &["Bonjour,", "Votre rendez-vous est déplacé à jeudi 10 h 30.", "Merci de confirmer."]);
        let more = decided(&again, &extra, Some(Who::Neutral), now_through(), Send::Words);
        let dir = std::env::temp_dir().join(format!("sioul-phonemsgs-again-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let own = calls::own_file(&dir, LOG, "phone");
        assert_eq!(carry_into(&own, &lines, NOW).unwrap(), 2);
        assert_eq!(carry_into(&own, &more, NOW).unwrap(), 1, "only the new message");
        assert_eq!(read_file(&own).len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What never makes a line, or never carries its words: a code, a copy
    /// Android redacted, a notification marked secret, a blocked sender's,
    /// a conversation held for good, an app off for your computers, Sioul's
    /// own, an ongoing one; a picture's line says "a picture", never more.
    #[test]
    fn what_must_stay_on_the_phone_never_travels() {
        let bank = person("Ma Banque", "tel:+33465710042");
        // A code: one line, said, without it.
        let (code, extra) = texted(&bank, &["Votre code de validation est 482913. Ne le communiquez à personne."]);
        let lines = decided(&code, &extra, Some(Who::Neutral), Decision::through(Why::Code), Send::Words);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].code && lines[0].text.is_empty() && lines[0].name == "Ma Banque", "{lines:?}");
        assert!(!padded(&lines[0]).contains("482913"));
        // A code from a short number, or a title that may be the code: the app's name said instead.
        let short = Person { name: "38015".into(), ..Person::default() };
        let (code, extra) = texted(&short, &["G-482913 est votre code de vérification."]);
        let lines = decided(&code, &extra, None, Decision::through(Why::Code), Send::Words);
        assert_eq!(lines.iter().map(|l| (l.code, l.name.as_str())).collect::<Vec<_>>(), [(true, "Message")]);
        // A copy Android redacted: a code too, its words never carried.
        let (mut redacted, extra) = texted(&bank, &["Sensitive notification content hidden"]);
        redacted.redacted = true;
        let lines = decided(&redacted, &extra, Some(Who::Neutral), Decision::through(Why::Code), Send::Words);
        assert!(lines.iter().all(|l| l.code && l.text.is_empty()), "{lines:?}");
        // Marked secret by its app: nothing at all.
        let martin = person("Cabinet du Dr Martin", "tel:+33199001234");
        let (posted, mut extra) = texted(&martin, &["Résultats disponibles."]);
        extra.visibility = SECRET;
        assert!(decided(&posted, &extra, Some(Who::Neutral), now_through(), Send::Words).is_empty());
        // A blocked sender, a conversation held for good: nothing.
        let (posted, extra) = texted(&martin, &["Encore moi."]);
        assert!(decided(&posted, &extra, Some(Who::Blocked), Decision::held(NOW / 1000 + appnotes::FOR_GOOD, Why::Never), Send::Words).is_empty());
        assert!(decided(&posted, &extra, Some(Who::Neutral), Decision::held(NOW / 1000 + appnotes::FOR_GOOD, Why::Never), Send::Words).is_empty());
        // The app off, or as usual and not the SMS app: nothing.
        assert!(decided(&posted, &extra, Some(Who::Neutral), now_through(), Send::Off).is_empty());
        assert!(decided(&posted, &extra, Some(Who::Neutral), now_through(), Send::Usual).is_empty(), "lines_of takes a resolved choice");
        // Sioul's own, an ongoing one: nothing.
        let own = Posted { package: appnotes::OWN.into(), ..posted.clone() };
        assert!(decided(&own, &extra, Some(Who::Neutral), now_through(), Send::Words).is_empty());
        let ongoing = Posted { ongoing: true, ..posted.clone() };
        assert!(decided(&ongoing, &extra, Some(Who::Neutral), now_through(), Send::Words).is_empty());
        // A picture: said, never carried.
        let (posted, mut extra) = texted(&martin, &[""]);
        extra.messages[0].picture = true;
        let lines = decided(&posted, &extra, Some(Who::Neutral), now_through(), Send::Words);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].picture && lines[0].text.is_empty());
        assert!(!padded(&lines[0]).contains("content://"));
        // Who and when only: the words stay.
        let (posted, extra) = texted(&martin, &["Résultats disponibles."]);
        let lines = decided(&posted, &extra, Some(Who::Neutral), now_through(), Send::Who);
        assert!(lines.len() == 1 && lines[0].withheld && lines[0].text.is_empty() && lines[0].name == "Cabinet du Dr Martin");
    }

    #[test]
    fn a_service_s_text_travels_as_an_automaton_s_held_as_the_phone_holds_it() {
        let ameli = Person { name: "AMELI".into(), ..Person::default() };
        let (posted, extra) = texted(&ameli, &["Votre attestation est disponible sur votre compte."]);
        let at_13 = NOW / 1000 + 3 * 3600;
        let lines = decided(&posted, &extra, None, Decision::held(at_13, Why::Gathered), Send::Words);
        assert_eq!(lines.len(), 1);
        assert_eq!((lines[0].who.as_str(), lines[0].name.as_str(), lines[0].shows), ("automaton", "AMELI", at_13 * 1000));
        assert!(lines[0].key.is_empty(), "no number to text back");
    }

    #[test]
    fn long_words_are_cut_and_every_line_weighs_steps_of_256_bytes() {
        let martin = person("Cabinet du Dr Martin", "tel:+33199001234");
        let long = "é".repeat(1500);
        let (posted, extra) = texted(&martin, &[long.as_str()]);
        let lines = decided(&posted, &extra, Some(Who::Neutral), now_through(), Send::Words);
        assert_eq!(lines[0].text.chars().count(), TEXT_MAX + 1, "cut, with its ellipsis");
        for text in ["", "Oui", "Votre rendez-vous est déplacé à jeudi 10 h 30.", &"x".repeat(240), &"x".repeat(247), &"x".repeat(1000)] {
            let line = Line { at: NOW, id: format!("{NOW}-0123456789abcdef"), app: "foundation.e.message".into(), kind: "text".into(), text: text.into(), shows: NOW, ..Line::default() };
            let written = padded(&line);
            assert_eq!(written.len() % PAD_STEP, 0, "{} bytes for {} characters", written.len(), text.len());
            let back: Line = serde_json::from_str(&written).unwrap();
            assert_eq!(Line { pad: String::new(), ..back }, line);
        }
        // Two messages of 3 and 40 characters weigh the same.
        let weigh = |t: &str| padded(&Line { at: NOW, id: "x".into(), app: "a".into(), kind: "text".into(), text: t.into(), ..Line::default() }).len();
        assert_eq!(weigh("Oui"), weigh("Je serai là vers dix-huit heures, à demain"));
    }

    #[test]
    fn a_phone_trims_its_own_after_a_week_and_any_device_another_s_after_ten_days() {
        let dir = std::env::temp_dir().join(format!("sioul-phonemsgs-trim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let line = |at: i64| Line { at, id: format!("{at}-a"), app: "foundation.e.message".into(), kind: "text".into(), who: "neutral".into(), text: "Bonjour".into(), shows: at, ..Line::default() };
        carry_into(&calls::own_file(&dir, LOG, "phone"), &[line(NOW - 3_600_000)], NOW).unwrap();
        calls::mark_seen(&dir, "desk", &[format!("{}-a", NOW - 3_600_000)], NOW).unwrap();
        assert_eq!(read_logs(&dir, NOW).len(), 1);
        let week = NOW + KEPT_DAYS * DAY_MS;
        assert!(read_logs(&dir, week).is_empty(), "past its week, read nowhere");
        assert_eq!(trim_others(&dir, "desk", week).unwrap(), 0, "the phone's own week: another device waits");
        assert_eq!(trim_own(&dir, "phone", week).unwrap(), 1);
        carry_into(&calls::own_file(&dir, LOG, "phone"), &[line(NOW)], NOW).unwrap();
        let ten = NOW + (KEPT_DAYS + LATE_DAYS) * DAY_MS + 1;
        assert_eq!(trim_others(&dir, "desk", ten).unwrap(), 1, "the phone gone: the desk takes its line out");
        assert_eq!(trim_own(&dir, "desk", ten).unwrap(), 1, "the desk's own seen line too");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn lister_at<'a>(now: &'a Zoned, tr: &'a Translator, shows: &'a dyn Fn(&Line) -> bool, column: &'a dyn Fn(i64) -> String) -> Lister<'a> {
        Lister { now, tr, region: fr(), name_of: &|key: &str| (key == "+33199001234").then(|| "Dr Martin".to_string()), shows, phone: &|_: &str| String::new(), column }
    }

    #[test]
    fn the_porch_groups_a_conversation_by_part_of_the_day_and_says_codes_and_pictures() {
        let tr = Translator::new("en");
        let now = jiff::Timestamp::from_millisecond(NOW).unwrap().to_zoned(jiff::tz::TimeZone::get("Europe/Paris").unwrap());
        let m = |at: i64, text: &str| Line { at, id: format!("{at}-m"), app: "foundation.e.message".into(), label: "Message".into(), kind: "text".into(), talk: "t1".into(), name: "Cabinet du Dr Martin".into(), key: "+33199001234".into(), who: "neutral".into(), text: text.into(), shows: at, device: "phone".into(), ..Line::default() };
        let mut all = vec![m(NOW - 7_200_000, "Bonjour,"), m(NOW - 7_100_000, "Votre rendez-vous est déplacé.")];
        all.push(Line { picture: true, text: String::new(), ..m(NOW - 600_000, "") });
        all.push(Line { at: NOW - 300_000, id: "code".into(), app: "foundation.e.message".into(), kind: "text".into(), name: "Ma Banque".into(), code: true, shows: NOW - 300_000, device: "phone".into(), ..Line::default() });
        let column = |at: i64| (if at < NOW - 3_600_000 { "sleep" } else { "work" }).to_string();
        let all_shown = |_: &Line| true;
        let l = lister_at(&now, &tr, &all_shown, &column);
        let lines = lines(&all, &calls::Seen::default(), &l);
        assert_eq!(lines.len(), 3, "{lines:#?}");
        assert_eq!(lines[0].head, "While you slept, Dr Martin wrote:");
        assert_eq!(lines[0].said.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), ["Bonjour,", "Votre rendez-vous est déplacé."]);
        assert!(lines[0].texted && lines[0].known && lines[0].dial == "+33199001234" && lines[0].number == "01 99 00 12 34");
        assert_eq!(lines[1].said[0].text, "a picture");
        assert_eq!(lines[2].head, "A code came from Ma Banque at 09:55. It stays on your phone.");
        // Seen on another device: gone.
        let mut seen = calls::Seen::default();
        seen.seen.extend(lines[0].ids.iter().cloned());
        assert_eq!(super::lines(&all, &seen, &l).len(), 2);
        // Not before the phone let it through.
        let later = Line { shows: NOW + 3_600_000, ..m(NOW - 60_000, "Plus tard") };
        assert!(super::lines(&[later], &calls::Seen::default(), &l).is_empty());
    }

    /// A stranger's text in the evening shows at work the next morning, not
    /// during the night; a safe person's at once in the evening; a service's
    /// in work hours, as the automatons' row says; never while you sleep.
    #[test]
    fn the_porch_shows_each_at_the_time_its_sender_may_reach_you() {
        let attention = Attention::usual();
        let line = |who: &str| Line { at: NOW, id: "x".into(), app: "foundation.e.message".into(), kind: "text".into(), who: who.into(), text: "Bonjour".into(), shows: NOW, ..Line::default() };
        let at = |column: Column| attention::Now::time(column);
        assert!(!reaches_now(&attention, &line("stranger"), None, false, &at(Column::Leisure)));
        assert!(!reaches_now(&attention, &line("stranger"), None, false, &at(Column::Sleep)));
        assert!(reaches_now(&attention, &line("stranger"), None, false, &at(Column::Work)));
        assert!(reaches_now(&attention, &line("safe"), None, false, &at(Column::Leisure)));
        assert!(!reaches_now(&attention, &line("safe"), None, false, &at(Column::Sleep)));
        assert!(reaches_now(&attention, &line("stranger"), None, true, &at(Column::Sleep)), "Always through");
        assert!(reaches_now(&attention, &line("automaton"), None, false, &at(Column::Work)));
        assert!(!reaches_now(&attention, &line("automaton"), None, false, &at(Column::Sleep)));
        // This device's lists now: blocked here since, never.
        assert!(!reaches_now(&attention, &line("safe"), Some(Who::Blocked), false, &at(Column::Work)));
        // A code's line: whatever the time.
        assert!(reaches_now(&attention, &Line { code: true, ..line("") }, None, false, &at(Column::Sleep)));
    }

    #[test]
    fn a_person_s_sheet_lists_their_week() {
        let tr = Translator::new("en");
        let now = jiff::Timestamp::from_millisecond(NOW).unwrap().to_zoned(jiff::tz::TimeZone::get("Europe/Paris").unwrap());
        let all = vec![
            Line { at: NOW - DAY_MS, id: "a".into(), app: "foundation.e.message".into(), kind: "text".into(), key: "+33199001234".into(), who: "neutral".into(), text: "Bonjour".into(), ..Line::default() },
            Line { at: NOW - 600_000, id: "b".into(), app: "org.thoughtcrime.securesms".into(), label: "Signal".into(), kind: "chat".into(), key: "+33199001234".into(), who: "neutral".into(), withheld: true, ..Line::default() },
            Line { at: NOW - 300_000, id: "c".into(), app: "foundation.e.message".into(), kind: "text".into(), key: "+33465710042".into(), who: "neutral".into(), text: "Autre".into(), ..Line::default() },
        ];
        let column = |_: i64| String::new();
        let shows = |_: &Line| true;
        let l = lister_at(&now, &tr, &shows, &column);
        let said = history(&all, &BTreeSet::from(["+33199001234".to_string()]), &l);
        assert_eq!(said, ["Today at 09:50: a message on Signal; its words stay on your phone.", "Yesterday at 10:00: Bonjour"]);
    }

    /// The listener's JSON as Java writes it (AppNotes.describe with
    /// PhoneMessages, android/jvm-checks/MessagesCheck.java): what `Posted`
    /// reads is untouched by the fields beside, which `Extra` reads.
    #[test]
    fn what_java_says_beside_reads_here() {
        let json = r#"{"key":"0|foundation.e.message|7|null|10123","package":"foundation.e.message","app":"Message","category":"msg","title":"Cabinet du Dr Martin","text":"","messages":[{"text":"","sender":{"name":"Cabinet du Dr Martin","key":"","uri":"tel:+33199001234","bot":false},"at":1791446340000,"picture":true}],"people":[],"sms_app":true,"visibility":-1,"when":1791446340000,"posted":1791446341000,"picture":false}"#;
        let posted: Posted = serde_json::from_str(json).unwrap();
        assert_eq!((posted.package.as_str(), posted.messages.len(), posted.sms_app), ("foundation.e.message", 1, true));
        let extra = Extra::read(json);
        assert_eq!(extra, Extra { visibility: SECRET, when: 1_791_446_340_000, posted: 1_791_446_341_000, picture: false, messages: vec![ExtraMessage { at: 1_791_446_340_000, picture: true }] });
        // An older Java, which said none of it: nothing secret, no time, no picture.
        assert_eq!(Extra::read(r#"{"package":"foundation.e.message","messages":[{"text":"Bonjour"}]}"#), Extra { messages: vec![ExtraMessage::default()], ..Extra::default() });
    }

    #[test]
    fn the_phone_s_choices_by_app() {
        let mut choices = Choices::default();
        assert_eq!(choices.send("foundation.e.message", true), Send::Words, "the SMS app: on");
        assert_eq!(choices.send("org.thoughtcrime.securesms", false), Send::Off, "a chat app: off until chosen");
        choices.sms_app = "foundation.e.message".into();
        assert_eq!(choices.send("foundation.e.message", false), Send::Words, "as the listener noted it");
        choices.set("org.thoughtcrime.securesms", Send::Who, "Signal");
        choices.set("foundation.e.message", Send::Off, "Message");
        assert_eq!((choices.send("org.thoughtcrime.securesms", false), choices.send("foundation.e.message", true)), (Send::Who, Send::Off));
        choices.set("foundation.e.message", Send::Usual, "Message");
        assert!(!choices.app.contains_key("foundation.e.message"));
        let dir = std::env::temp_dir().join(format!("sioul-phonemsgs-choices-{}", std::process::id()));
        let path = dir.join(CHOICES_FILE);
        choices.save(&path).unwrap();
        assert_eq!(Choices::load(&path), choices);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

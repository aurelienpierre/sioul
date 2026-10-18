// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Replies from your computers to the messages your phone's notifications
//! brought, through each notification's own Reply, as the phone's shade sends
//! a reply typed under it (docs/android.md, "Replies from your computers").
//! The phone's side, here and in its listener (AppNotes.java, Replies.java;
//! crates/sioul-app/src/replies.rs):
//!
//! - **Which notification**: each line for your computers names the
//!   notification it came in by the phone's own name for it (`notice`: a
//!   hash of its app and Android's key; Android's key, whose tag may hold a
//!   number, never leaves the phone), and a reply names it back.
//! - **What it offers** (`Live`): for each notification it relays, the phone
//!   writes in its own shared file, `phone-messages/live/<device>.jsonl`,
//!   whether it offers a Reply the phone can fill and a Mark as read, and
//!   when it went; no words, no names. A computer offers Reply only while it
//!   can work; the phone checks everything again when it sends.
//! - **Handed over** (`Due`): the background step, which decides each
//!   request once, hands the listener what is due in a private file sealed
//!   with a key made from the sharing key (`due.jsonl`, in this phone's
//!   `replies` folder of its state). The listener alone holds the
//!   notifications: it takes each once (`take`: "taken" written and flushed
//!   before anything is sent), finds the live notification, has it checked
//!   (`check`: the same app, notification and conversation; their newer
//!   messages), sends through its Reply or its Mark as read alone, and says
//!   what happened (`Done`, never the words).
//! - **Asked, once** (`Ask`, `verdict`): each computer writes its requests
//!   in its own file, `phone-messages/ask/<computer>.jsonl`, each line padded
//!   and sealed with a key made from the sharing key (and copied to the
//!   sharing folder, sent on the lane texts use); the phone decides each once
//!   by the texts' rules (a ledger flushed before the listener is asked, a
//!   mark, a request's life of a quarter of an hour, the doubt after a
//!   crash), with two guards of its own; and says what became of each in its
//!   own `phone-messages/outcome/<phone>.jsonl` (`Outcome`), never the words,
//!   which a computer says in words (`state_of`), never a plain "Sent".

use crate::appnotes::{self, Kind, Posted};
use crate::calls;
use crate::phonemsgs::{self, Extra};
use crate::phones::Region;
use crate::texts::Sealer;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The phone's private folder for replies, in its state folder: never shared.
pub const FOLDER: &str = "replies";
/// What is due, sealed lines (`Due`), written by the background step.
pub const DUE: &str = "due.jsonl";
/// What the listener did with each (`Done`), plain lines without words.
pub const DONE: &str = "done.jsonl";
/// The folder of the live lines, each phone's own file, under the phone's
/// messages (`phonemsgs::folder`): shared with the part "Messages from your phone".
pub const LIVE: &str = "live";
/// A reply the listener did not take this long after the step handed it is
/// not sent (`take`: "late"): its listener was not bound, or had no access.
pub const TAKE_WITHIN_MS: i64 = 2 * 60_000;
/// How long after a reply was handed its app's notification is looked at
/// for your words shown as yours (`shows`).
pub const SHOWN_WITHIN_MS: i64 = 2 * 60_000;
/// A reply's words: at most this many characters.
pub const WORDS_MAX: usize = 2000;
/// A day, in milliseconds.
const DAY_MS: i64 = 86_400_000;

/// The phone's private folder for replies (`FOLDER`).
pub fn private_folder() -> PathBuf {
    crate::config::state_dir().join(FOLDER)
}

/// A notification's name off the phone: FNV-1a of its app and Android's key
/// for it (`appnotes::talk_key`, as AppNotes.talkKey makes it), 16 hex
/// digits. Android's key holds the app's tag, which may be a number.
pub fn notice(package: &str, key: &str) -> String {
    appnotes::talk_key(package, key)
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

// ---------------------------------------------------------------- what the step hands the listener

/// A reply or a Mark as read due now, as the background step claimed it
/// (in `DUE`, sealed): what the listener needs to find the notification and
/// check it, and the words.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Due {
    /// The request's key: 32 hex digits, made by the computer that wrote it.
    pub key: String,
    /// "reply", or "read" (the notification's Mark as read).
    pub verb: String,
    /// The notification, as the line answered named it (`notice`).
    pub notice: String,
    /// Its app's package.
    pub app: String,
    /// The conversation's key, as the line said it (`appnotes::talk_key`); "" none.
    pub talk: String,
    /// Who wrote the line answered: their number's key, and their name as the app gave it.
    pub from: String,
    pub name: String,
    /// When the newest message you answered was sent (ms): a message of
    /// theirs after it in the notification crossed the reply.
    pub line_at: i64,
    /// A reply's words; none for "read".
    pub words: String,
    /// When the step claimed it (ms, the phone's clock).
    pub at: i64,
}

/// What the listener did with one (`DONE`), never its words.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Done {
    pub key: String,
    /// "taken" (written before it is sent); "handed" (its app took it);
    /// "shown" (its notification then showed your words as yours); or why it
    /// was not sent: "gone" (no such notification now, or another
    /// conversation in it), "no-reply" / "no-read" (it offers none now),
    /// "closed" (its Reply cannot be filled by another app), "locked" (its
    /// app asks for an unlocked phone), "activity" (it would open the app on
    /// the phone), "late" (the listener did not take it in time), "failed"
    /// (Android refused the sending).
    pub state: String,
    /// When (ms, the phone's clock).
    pub at: i64,
    /// Their newer messages were in the notification answered.
    #[serde(skip_serializing_if = "is_false")]
    pub crossed: bool,
    /// Answered among the notifications Sioul holds: a newer message waits on the phone.
    #[serde(skip_serializing_if = "is_false")]
    pub held: bool,
}

impl Done {
    pub fn of(key: &str, state: &str, at: i64) -> Done {
        Done { key: key.to_string(), state: state.to_string(), at, ..Done::default() }
    }
}

/// What is due, as written; a line that does not open (another key, cut) is passed over.
pub fn read_due(folder: &Path, seal: &dyn Sealer) -> Vec<Due> {
    crate::texts::read_file(&folder.join(DUE), seal)
}

/// What the listener did, oldest first.
pub fn read_done(folder: &Path) -> Vec<Done> {
    calls::read_lines(&folder.join(DONE))
}

/// The private folder made, yours alone (0700 on Unix), before its lock is taken there.
fn private_dir(folder: &Path) -> Result<(), String> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(folder).map_err(|e| format!("{}: {e}", folder.display()))
}

/// What is due written whole (each line sealed), beside then put in place,
/// under the folder's lock: the listener reads it whole or not at all.
pub fn write_due(folder: &Path, due: &[Due], seal: &dyn Sealer) -> Result<(), String> {
    private_dir(folder)?;
    let path = folder.join(DUE);
    crate::filelock::with_lock(&path, || {
        let body: String = due.iter().filter_map(|d| serde_json::to_string(d).ok()).map(|plain| format!("{}\n", seal.seal(&plain))).collect();
        let fresh = folder.join(format!(".{DUE}.{}.new", std::process::id()));
        let _ = std::fs::remove_file(&fresh);
        calls::append_private(&fresh, &body)?;
        std::fs::rename(&fresh, &path).map_err(|e| {
            let _ = std::fs::remove_file(&fresh);
            format!("{}: {e}", path.display())
        })
    })
}

/// A line of what the listener did, appended and flushed to the disk, yours alone.
fn append_flushed(path: &Path, done: &Done) -> Result<(), String> {
    use std::io::Write;
    let line = serde_json::to_string(done).map_err(|e| e.to_string())? + "\n";
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    calls::append_private(path, "")?;
    let mut file = std::fs::OpenOptions::new().append(true).open(path).map_err(fail)?;
    file.write_all(line.as_bytes()).map_err(fail)?;
    file.sync_all().map_err(fail)
}

/// What the listener did with one, said: appended, flushed, under the folder's lock.
pub fn say(folder: &Path, done: &Done) -> Result<(), String> {
    private_dir(folder)?;
    crate::filelock::with_lock(&folder.join(DUE), || append_flushed(&folder.join(DONE), done))
}

/// What is due that the listener has not taken yet: each due line with no word of it.
pub fn waiting(folder: &Path, seal: &dyn Sealer) -> Vec<Due> {
    let done = read_done(folder);
    read_due(folder, seal).into_iter().filter(|d| !done.iter().any(|x| x.key == d.key)).collect()
}

/// Whether the listener may send one now (`take`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taken {
    /// "taken" written and flushed: send it, now.
    Go(Box<Due>),
    /// Not taken within `TAKE_WITHIN_MS` of the step's claim: said "late", never sent.
    Late,
    /// Not due, or done already (taken once, said otherwise): nothing.
    No,
}

/// One taken by the listener, once, under the folder's lock: "taken"
/// written and flushed before anything is sent, so that a crash after it
/// leaves the doubt, never a second sending. Not due, or done already:
/// nothing. Handed over more than `TAKE_WITHIN_MS` ago: "late", not sent.
pub fn take(folder: &Path, key: &str, seal: &dyn Sealer, now_ms: i64) -> Result<Taken, String> {
    private_dir(folder)?;
    crate::filelock::with_lock(&folder.join(DUE), || {
        if read_done(folder).iter().any(|d| d.key == key) {
            return Ok(Taken::No);
        }
        let Some(due) = read_due(folder, seal).into_iter().find(|d| d.key == key) else { return Ok(Taken::No) };
        if now_ms - due.at > TAKE_WITHIN_MS || due.at - now_ms > TAKE_WITHIN_MS {
            append_flushed(&folder.join(DONE), &Done::of(key, "late", now_ms))?;
            return Ok(Taken::Late);
        }
        append_flushed(&folder.join(DONE), &Done::of(key, "taken", now_ms))?;
        Ok(Taken::Go(Box::new(due)))
    })
}

// ---------------------------------------------------------------- the live notification, checked

/// What the live notification found for a due one says (`check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checked {
    /// The same app, notification and conversation: send it. `crossed`:
    /// their messages newer than the one you answered are in it.
    Go { crossed: bool },
    /// No longer that notification, or no longer that conversation in it: "gone".
    Gone,
}

/// Whether the notification found now by its name is still the one a
/// request answered: the same app and name (`notice`), and the same
/// conversation as the line answered, read as when the line was made
/// (`appnotes::classify`): its key; else, for a line without one, the same
/// sender's number or name. Exactly what the shade's own Reply guarantees:
/// the Reply under a notification is that conversation's (apps make one per
/// conversation, as Android tells them).
pub fn check(due: &Due, posted: &Posted, extra: &Extra, choices: &appnotes::Choices, words: &crate::words::Words, region: Option<&Region>) -> Checked {
    if posted.package != due.app || posted.key.is_empty() || notice(&posted.package, &posted.key) != due.notice {
        return Checked::Gone;
    }
    let same = if !due.talk.is_empty() {
        matches!(appnotes::classify(posted, choices, words), Kind::People(talk) if talk.conversation.as_ref().is_some_and(|c| c.key == due.talk))
    } else {
        let (numbers, names) = phonemsgs::writers(posted, region);
        if !due.from.is_empty() {
            numbers.contains(&due.from)
        } else {
            due.name.is_empty() || names.contains(&due.name)
        }
    };
    if !same {
        return Checked::Gone;
    }
    Checked::Go { crossed: phonemsgs::newest_from_others(posted, extra) > due.line_at }
}

/// A reply's words as the phone compares them, never kept: a hash.
pub fn words_hash(words: &str) -> String {
    appnotes::talk_key("replies", words.trim())
}

/// Whether a notification shows a reply's words as the person's own (its
/// hash, `words_hash`): a message of theirs in the conversation (no sender,
/// or the conversation's own user), or what went through its Reply lately
/// (`Extra::history`). Signal and Element add your reply so; the phone then
/// says "shown".
pub fn shows(posted: &Posted, extra: &Extra, hash: &str) -> bool {
    let own = posted.messages.iter().filter(|m| m.sender.as_ref().is_none_or(|s| phonemsgs::is_user(posted, s))).map(|m| m.text.as_str());
    own.chain(extra.history.iter().map(String::as_str)).any(|words| words_hash(words) == hash)
}

// ---------------------------------------------------------------- the live lines

/// What a relayed notification offers now, a line in its phone's own file
/// (`LIVE`), shared: whether a Reply the phone can fill and a Mark as read
/// are there, and when it went. No words, no names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Live {
    /// The notification (`notice`).
    pub notice: String,
    /// Its app's package.
    pub app: String,
    /// Its conversation's key, as its lines say it (`appnotes::talk_key`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub talk: String,
    #[serde(skip_serializing_if = "is_false")]
    pub reply: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub read: bool,
    /// When it went from the phone (ms; its app, or you, took it away); 0 while there.
    #[serde(skip_serializing_if = "is_zero")]
    pub gone: i64,
    /// When this was said (ms, the phone's clock).
    pub at: i64,
    /// The phone that wrote it: its file's name. Never written.
    #[serde(skip)]
    pub device: String,
}

/// A phone's own live file under `root` (the phone's messages' folder).
pub fn live_file(root: &Path, device: &str) -> PathBuf {
    calls::own_file(root, LIVE, device)
}

/// What a file of live lines said last of each notification.
fn last_of(path: &Path) -> BTreeMap<String, Live> {
    calls::read_lines::<Live>(path).into_iter().filter(|l| !l.notice.is_empty() && l.at > 0).map(|l| (l.notice.clone(), l)).collect()
}

/// What a relayed notification offers now, said in its phone's own file
/// (`own`) when it differs from what the file last said of it, under its
/// lock: one line per change, not per message. Whether a line was written.
pub fn offer(own: &Path, live: &Live) -> Result<bool, String> {
    if live.notice.is_empty() || live.at <= 0 {
        return Ok(false);
    }
    crate::filelock::with_lock(own, || {
        let same = last_of(own).get(&live.notice).is_some_and(|last| last.gone == 0 && last.reply == live.reply && last.read == live.read && last.talk == live.talk && last.app == live.app);
        if same {
            return Ok(false);
        }
        let line = serde_json::to_string(&Live { gone: 0, device: String::new(), ..live.clone() }).map_err(|e| e.to_string())? + "\n";
        calls::append_private(own, &line).map(|()| true)
    })
}

/// A relayed notification gone from the phone, said once in its phone's own
/// file (`own`), under its lock: only one the file says is there. Whether a
/// line was written.
pub fn went(own: &Path, notice: &str, now_ms: i64) -> Result<bool, String> {
    crate::filelock::with_lock(own, || {
        let Some(last) = last_of(own).remove(notice).filter(|l| l.gone == 0) else { return Ok(false) };
        let line = serde_json::to_string(&Live { reply: false, read: false, gone: now_ms, at: now_ms, ..last }).map_err(|e| e.to_string())? + "\n";
        calls::append_private(own, &line).map(|()| true)
    })
}

/// What every phone under `root` said last of each notification, by phone
/// and notice, within the week the lines are kept.
pub fn read_live(root: &Path, now_ms: i64) -> BTreeMap<(String, String), Live> {
    let mut out = BTreeMap::new();
    for (device, path) in calls::device_files(&root.join(LIVE)) {
        for (notice, live) in last_of(&path) {
            if now_ms - live.at <= phonemsgs::KEPT_DAYS * DAY_MS {
                out.insert((device.clone(), notice), Live { device: device.clone(), ..live });
            }
        }
    }
    out
}

// ---------------------------------------------------------------- the request, and never twice

/// The folder of each computer's requests (`Ask`), under the phone's
/// messages: each computer's own file, its lines padded and sealed; shared.
pub const ASK: &str = "ask";
/// The folder of each phone's word on each request (`Outcome`), its own
/// file: shared, never the words.
pub const OUTCOME: &str = "outcome";
/// The same words to the same notification are refused this long after the
/// first, unless the request sends that first one again on purpose.
pub const SAME_WORDS_MS: i64 = 10 * 60_000;
/// At most this many replies an hour, from all your computers together.
pub const PER_HOUR: usize = 60;
/// A request's line is padded to a multiple of this before it is sealed.
pub const PAD_STEP: usize = 256;
/// An hour, in milliseconds.
const HOUR_MS: i64 = 3_600_000;

/// A request a computer writes for a phone (decision 2 of the design): a
/// reply's words for a notification's Reply, a Mark as read, or a request
/// taken back. Nothing else can be asked: there is no field for an action,
/// an app's key or an address.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ask {
    /// Random, 32 hex digits: the request's one name everywhere. A "cancel"
    /// takes back the request of the same key, from the same computer.
    pub key: String,
    /// The phone that must send it: its name in the sharing.
    pub phone: String,
    /// "reply", "read" or "cancel".
    pub verb: String,
    /// The notification, by the name the phone gave it (`notice`).
    pub notice: String,
    /// Its app's package.
    pub app: String,
    /// The newest line you had in front of you, by its id in that phone's log.
    pub line: String,
    /// A reply's words, at most `WORDS_MAX` characters; none otherwise.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub words: String,
    /// When it was written (ms, the writer's clock), and how far the writer's
    /// clock may run ahead of the phone's (an upper bound, as texts measure it).
    pub written: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub ahead: i64,
    /// The key of the request this one sends again (Send again), if any.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub again: String,
    /// Spaces that make the line's length a multiple of `PAD_STEP` (`padded_ask`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub pad: String,
    /// The computer that wrote it: its file's name. Never written.
    #[serde(skip)]
    pub device: String,
}

/// A request as written: its JSON padded with spaces to a multiple of
/// `PAD_STEP` bytes, before it is sealed: no reply's length shows.
pub fn padded_ask(ask: &Ask) -> String {
    let bare = Ask { pad: String::new(), device: String::new(), ..ask.clone() };
    let text = serde_json::to_string(&bare).unwrap_or_default();
    if text.len() % PAD_STEP == 0 {
        return text;
    }
    // `,"pad":""` and the spaces inside it.
    let room = text.len() + 9;
    let spaces = (PAD_STEP - room % PAD_STEP) % PAD_STEP;
    serde_json::to_string(&Ask { pad: " ".repeat(spaces), ..bare }).unwrap_or(text)
}

/// A request added to its computer's own file under `root` (the phone's
/// messages' folder), padded and sealed, under its lock: once by its key and
/// verb.
pub fn write_ask(root: &Path, device: &str, ask: &Ask, seal: &dyn Sealer) -> Result<bool, String> {
    let own = calls::own_file(root, ASK, device);
    crate::filelock::with_lock(&own, || {
        if crate::texts::read_file::<Ask>(&own, seal).iter().any(|a| a.key == ask.key && a.verb == ask.verb) {
            return Ok(false);
        }
        calls::append_private(&own, &format!("{}\n", seal.seal(&padded_ask(ask)))).map(|()| true)
    })
}

/// Every computer's requests under each root (the records' copy, the
/// sharing folder's small files, the copy fetched from the server), each
/// with its computer: each key once, the first root's first; and the keys
/// each computer took back ("cancel"), by computer and key.
pub fn read_asks(roots: &[PathBuf], seal: &dyn Sealer) -> (Vec<Ask>, BTreeSet<(String, String)>) {
    let mut asks: Vec<Ask> = Vec::new();
    let mut cancelled = BTreeSet::new();
    for root in roots {
        for (device, path) in calls::device_files(&root.join(ASK)) {
            for ask in crate::texts::read_file::<Ask>(&path, seal) {
                let ask = Ask { device: device.clone(), ..ask };
                if ask.key.is_empty() {
                    continue;
                }
                if ask.verb == "cancel" {
                    cancelled.insert((device.clone(), ask.key.clone()));
                } else if !asks.iter().any(|known| known.key == ask.key) {
                    asks.push(ask);
                }
            }
        }
    }
    (asks, cancelled)
}

/// A phone's word on a request (decision 10): never the words.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Outcome {
    pub key: String,
    /// "taken", "handed", "shown", the listener's reasons it was not sent
    /// (`Done::state`), "doubt" (taken, and no word from its app after),
    /// "taken-back", "expired", "clocks", "refused" (`reason` says why).
    pub state: String,
    /// When (ms, the phone's clock).
    pub at: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
    #[serde(skip_serializing_if = "is_false")]
    pub crossed: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub held: bool,
    /// The phone that wrote it. Never written.
    #[serde(skip)]
    pub device: String,
}

/// Every phone's outcomes under `root`, each with its phone, oldest first.
pub fn read_outcomes(root: &Path) -> Vec<Outcome> {
    let mut all: Vec<Outcome> = calls::device_files(&root.join(OUTCOME)).into_iter().flat_map(|(device, path)| calls::read_lines::<Outcome>(&path).into_iter().map(move |o| Outcome { device: device.clone(), ..o })).collect();
    all.sort_by_key(|o| o.at);
    all
}

/// Outcomes added to a phone's own file (`own`), each (key, state) once,
/// under its lock. How many were written.
pub fn say_outcomes(own: &Path, outcomes: &[Outcome]) -> Result<usize, String> {
    if outcomes.is_empty() {
        return Ok(0);
    }
    crate::filelock::with_lock(own, || {
        let mut known: BTreeSet<(String, String)> = calls::read_lines::<Outcome>(own).into_iter().map(|o| (o.key, o.state)).collect();
        let mut text = String::new();
        for o in outcomes {
            if known.insert((o.key.clone(), o.state.clone())) {
                text.push_str(&serde_json::to_string(&Outcome { device: String::new(), ..o.clone() }).map_err(|e| e.to_string())?);
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

/// What the phone does with a request (decision 3), the texts' rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Claim it, then hand it to the listener.
    Send,
    /// Another phone's, or a "cancel" (read with what it takes back): nothing done, nothing said.
    NotMine,
    /// Its key is in the ledger already, whatever was decided: nothing again.
    Decided,
    /// Its computer took it back before the phone claimed it.
    TakenBack,
    /// Older than `texts::EXPIRY_MS` on the phone's clock.
    Expired,
    /// Dated further ahead of the phone's clock than `texts::AHEAD_MS`.
    Clocks,
    /// Refused, and why: "not-counted", "verb", "no-words", "too-long",
    /// "line", "code", "switched-off", "short-number", "before-mark",
    /// "same-words", "too-many".
    Refused(&'static str),
}

/// What the phone judges a request by.
pub struct Judge<'a> {
    /// This phone's name in the sharing.
    pub here: &'a str,
    pub now_ms: i64,
    /// Its private ledger of replies (`texts::Ledger`, in its replies folder).
    pub ledger: &'a crate::texts::Ledger,
    /// Its own log's lines: a request answers one of them.
    pub lines: &'a [phonemsgs::Line],
    /// Whether the sharing counts this device now (not left, forgotten,
    /// replaced, silent, or a stray file): only your own devices ask.
    pub counted: &'a dyn Fn(&str) -> bool,
    /// The keys each computer took back (`read_asks`).
    pub cancelled: &'a BTreeSet<(String, String)>,
    /// Whether an app's notifications may be answered from your computers (the phone's switch).
    pub answers: &'a dyn Fn(&str) -> bool,
}

/// The never-twice decision for a reply, step by step: another phone's (or
/// a "cancel"); decided already; taken back by its computer; asked by a
/// device the sharing no longer counts; refused (another verb, no words or
/// too many, a line not in this phone's log or of another notification, a
/// code's line, its app's answers switched off, a short number or a sender
/// named in letters in the SMS app); written before the ledger's mark;
/// dated ahead of the phone's clock; older than `texts::EXPIRY_MS`; the two
/// guards (the same words to the same notification within `SAME_WORDS_MS`
/// unless it sends the first again; `PER_HOUR` replies in the last hour);
/// else sent. The time written is corrected by how far the writer's clock
/// may run ahead.
pub fn verdict(ask: &Ask, j: &Judge) -> Verdict {
    if ask.phone != j.here || ask.key.is_empty() || ask.verb == "cancel" {
        return Verdict::NotMine;
    }
    if j.ledger.has(&ask.key) {
        return Verdict::Decided;
    }
    if j.cancelled.contains(&(ask.device.clone(), ask.key.clone())) {
        return Verdict::TakenBack;
    }
    if !(j.counted)(&ask.device) {
        return Verdict::Refused("not-counted");
    }
    let reply = ask.verb == "reply";
    if !reply && ask.verb != "read" {
        return Verdict::Refused("verb");
    }
    if reply && ask.words.trim().is_empty() {
        return Verdict::Refused("no-words");
    }
    if reply && ask.words.chars().count() > WORDS_MAX {
        return Verdict::Refused("too-long");
    }
    let Some(line) = j.lines.iter().find(|l| l.id == ask.line && !l.notice.is_empty()) else { return Verdict::Refused("line") };
    if line.notice != ask.notice || line.app != ask.app {
        return Verdict::Refused("line");
    }
    if line.code {
        return Verdict::Refused("code");
    }
    if !(j.answers)(&ask.app) {
        return Verdict::Refused("switched-off");
    }
    if line.kind == "text" && !line.group && (line.who == "automaton" || !crate::phones::is_whole(&line.key)) {
        return Verdict::Refused("short-number");
    }
    let written = ask.written - ask.ahead;
    if written < j.ledger.mark.since {
        return Verdict::Refused("before-mark");
    }
    if written > j.now_ms + crate::texts::AHEAD_MS {
        return Verdict::Clocks;
    }
    if j.now_ms - written > crate::texts::EXPIRY_MS {
        return Verdict::Expired;
    }
    if reply {
        let hash = words_hash(&ask.words);
        let replies: Vec<&crate::texts::Claim> = j.ledger.claims.iter().filter(|c| c.state == "claimed" && !c.hash.is_empty() && j.now_ms - c.at <= HOUR_MS).collect();
        if replies.iter().any(|c| c.to == ask.notice && c.hash == hash && j.now_ms - c.at <= SAME_WORDS_MS && c.key != ask.again) {
            return Verdict::Refused("same-words");
        }
        if replies.len() >= PER_HOUR {
            return Verdict::Refused("too-many");
        }
    }
    Verdict::Send
}

/// A request claimed, as the ledger keeps it: the notification's name and
/// the words' hash, for the guards; never the words.
pub fn claim_of(ask: &Ask, now_ms: i64) -> crate::texts::Claim {
    crate::texts::Claim { key: ask.key.clone(), at: now_ms, state: "claimed".into(), to: ask.notice.clone(), hash: if ask.verb == "reply" { words_hash(&ask.words) } else { String::new() } }
}

/// What the listener needs of a request claimed: the line it answers, read
/// from this phone's log (its conversation, who wrote, its time), the words.
pub fn due_of(ask: &Ask, line: &phonemsgs::Line, now_ms: i64) -> Due {
    Due { key: ask.key.clone(), verb: ask.verb.clone(), notice: ask.notice.clone(), app: ask.app.clone(), talk: line.talk.clone(), from: line.key.clone(), name: line.name.clone(), line_at: line.at, words: if ask.verb == "reply" { ask.words.clone() } else { String::new() }, at: now_ms }
}

/// What is due added to what the listener has not done yet (those done
/// more than a day ago left out), written whole under the folder's lock.
pub fn add_due(folder: &Path, fresh: &[Due], seal: &dyn Sealer, now_ms: i64) -> Result<(), String> {
    private_dir(folder)?;
    crate::filelock::with_lock(&folder.join(DUE), || {
        let done = read_done(folder);
        let mut due: Vec<Due> = read_due(folder, seal).into_iter().filter(|d| now_ms - d.at <= DAY_MS || !done.iter().any(|x| x.key == d.key)).filter(|d| !fresh.iter().any(|f| f.key == d.key)).collect();
        due.extend(fresh.iter().cloned());
        write_due(folder, &due, seal)
    })
}

/// The listener's words on claimed requests, for its outcomes; and, under
/// the folder's lock, what it never took within `TAKE_WITHIN_MS` of the
/// claim said "late" (its listener was not bound), and what it took without
/// a word after `texts::LOOK_AFTER_MS` said "doubt": its app may have it.
/// `claimed`: the keys the ledger says claimed, with when.
pub fn settle(folder: &Path, claimed: &[(String, i64)], now_ms: i64) -> Result<Vec<Done>, String> {
    private_dir(folder)?;
    crate::filelock::with_lock(&folder.join(DUE), || {
        let done = read_done(folder);
        for (key, at) in claimed {
            let words: Vec<&Done> = done.iter().filter(|d| d.key == *key).collect();
            if words.is_empty() && now_ms - at > TAKE_WITHIN_MS {
                append_flushed(&folder.join(DONE), &Done::of(key, "late", now_ms))?;
            } else if words.len() == 1 && words[0].state == "taken" && now_ms - words[0].at > crate::texts::LOOK_AFTER_MS {
                append_flushed(&folder.join(DONE), &Done::of(key, "doubt", now_ms))?;
            }
        }
        Ok(read_done(folder))
    })
}

/// Whether a state ends a request: no other word of the phone's follows,
/// but "shown" after "handed".
pub fn is_final(state: &str) -> bool {
    !matches!(state, "taken" | "handed")
}

/// What the phone did with each request, from the listener's words and the
/// step's verdicts: an outcome per state, never the words.
pub fn outcomes_of(done: &[Done]) -> Vec<Outcome> {
    done.iter().map(|d| Outcome { key: d.key.clone(), state: d.state.clone(), at: d.at, crossed: d.crossed, held: d.held, ..Outcome::default() }).collect()
}

/// The private files old lines are taken out of, a day on: what is due and
/// done; the ledger keeps its keys `texts::LEDGER_KEPT_MS`.
pub fn trim_private(folder: &Path, seal: &dyn Sealer, now_ms: i64) -> Result<(), String> {
    crate::filelock::with_lock(&folder.join(DUE), || {
        let due: Vec<Due> = read_due(folder, seal).into_iter().filter(|d| now_ms - d.at <= DAY_MS).collect();
        if folder.join(DUE).is_file() {
            write_due(folder, &due, seal)?;
        }
        calls::rewrite_lines(&folder.join(DONE), |line| serde_json::from_str::<Done>(line).is_ok_and(|d| now_ms - d.at > DAY_MS)).map(|_| ())
    })?;
    crate::texts::trim_ledger(folder, now_ms).map(|_| ())
}

/// The requests and outcomes kept past `phonemsgs::KEPT_DAYS` taken out of
/// this device's own files under `root`, those of other devices
/// `phonemsgs::LATE_DAYS` later (a computer gone for good never takes its
/// own out): the sharing then takes them out everywhere. The requests are
/// sealed: each opened to read its time, one that does not open kept.
pub fn trim_shared(root: &Path, here: &str, seal: &dyn Sealer, now_ms: i64) -> Result<usize, String> {
    let ask_old = |days: i64| move |line: &str| seal.open(line.trim()).and_then(|plain| serde_json::from_str::<Ask>(&plain).ok()).is_some_and(|a| now_ms - a.written > days * DAY_MS);
    let outcome_old = |days: i64| move |line: &str| serde_json::from_str::<Outcome>(line).is_ok_and(|o| now_ms - o.at > days * DAY_MS);
    let mut gone = 0;
    for (device, path) in calls::device_files(&root.join(ASK)) {
        let days = if device == calls::file_safe(here) { phonemsgs::KEPT_DAYS } else { phonemsgs::KEPT_DAYS + phonemsgs::LATE_DAYS };
        gone += calls::rewrite_lines(&path, ask_old(days))?;
    }
    for (device, path) in calls::device_files(&root.join(OUTCOME)) {
        let days = if device == calls::file_safe(here) { phonemsgs::KEPT_DAYS } else { phonemsgs::KEPT_DAYS + phonemsgs::LATE_DAYS };
        gone += calls::rewrite_lines(&path, outcome_old(days))?;
    }
    Ok(gone)
}

// ---------------------------------------------------------------- the states, in words

/// A request's state as a computer says it (decision 10): never red, never
/// "sent" nor "delivered", which the phone cannot know of another app.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct State {
    /// "Handed to Signal at 14:03."
    pub said: String,
    /// Still waiting for the phone: Take back offered.
    pub waiting: bool,
    /// Send again offered (a new key): after it expired, clocks apart, a doubt.
    pub again: bool,
    /// "Reply on your phone" fits: the notification gone, or its Reply closed to Sioul.
    pub on_phone: bool,
    /// The phone's last word on it: a state of `Outcome`, "" none.
    pub state: String,
}

/// How far a state goes, to say the furthest: shown, then handed, then an end, then taken.
fn rank(state: &str) -> u8 {
    match state {
        "shown" => 4,
        "handed" => 3,
        "taken" => 1,
        _ => 2,
    }
}

/// A request's state in words, from its phone's outcomes (`outcomes`, its
/// key's): the furthest it went, the latest of those. `app`: its app's name
/// as the phone shows it; `phone_shared`: when the phone last shared (ms),
/// for "Waiting for your phone"; `time_of`: a moment (ms) as the computer
/// says the time.
pub fn state_of(ask: &Ask, outcomes: &[&Outcome], app: &str, tr: &crate::i18n::Translator, phone_shared: Option<i64>, time_of: &dyn Fn(i64) -> String) -> State {
    let mut args = crate::i18n::args();
    args.set("app", app.to_string());
    let Some(last) = outcomes.iter().copied().max_by_key(|o| (rank(&o.state), o.at)) else {
        return match phone_shared {
            Some(at) => {
                args.set("time", time_of(at));
                State { said: tr.text("replies-waiting", Some(&args)), waiting: true, ..State::default() }
            }
            None => State { said: tr.text("replies-waiting-never", None), waiting: true, ..State::default() },
        };
    };
    args.set("time", time_of(last.at));
    let read = ask.verb == "read";
    let (id, again, on_phone) = match (last.state.as_str(), read) {
        ("handed", true) => ("replies-read-handed", false, false),
        ("handed", false) if last.held => ("replies-handed-held", false, false),
        ("handed", false) if last.crossed => ("replies-handed-crossed", false, false),
        ("handed", false) => ("replies-handed", false, false),
        ("shown", _) => ("replies-shown", false, false),
        ("taken", _) => ("replies-taken", false, false),
        ("gone", _) => ("replies-gone", false, true),
        ("no-reply", _) => ("replies-no-reply", false, true),
        ("no-read", _) => ("replies-no-read", false, true),
        ("closed", _) => ("replies-closed", false, true),
        ("locked", _) => ("replies-locked", false, true),
        ("activity", _) => ("replies-activity", false, true),
        ("late", _) => ("replies-late", true, false),
        ("failed", _) => ("replies-failed", true, false),
        ("expired", _) => ("replies-expired", true, false),
        ("clocks", _) => ("replies-clocks", true, false),
        ("doubt", _) => ("replies-doubt", true, false),
        ("taken-back", _) => ("replies-taken-back", false, false),
        ("refused", _) => match last.reason.as_str() {
            "same-words" => ("replies-refused-same-words", false, false),
            "too-many" => ("replies-refused-too-many", false, false),
            "too-long" => ("replies-refused-too-long", false, false),
            "no-words" => ("replies-refused-no-words", false, false),
            "line" => ("replies-refused-line", false, true),
            "code" => ("replies-refused-code", false, true),
            "switched-off" => ("replies-refused-switched-off", false, true),
            "short-number" => ("replies-refused-short-number", false, true),
            "not-counted" => ("replies-refused-not-counted", false, false),
            "before-mark" => ("replies-refused-before-mark", true, false),
            _ => ("replies-refused", false, true),
        },
        _ => ("replies-refused", false, true),
    };
    State { said: tr.text(id, Some(&args)), again, on_phone, state: last.state.clone(), ..State::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appnotes::{Message, Person};
    use crate::phonemsgs::ExtraMessage;

    /// Thursday 8 October 2026, 10:00 in Paris.
    const NOW: i64 = 1_791_446_400_000;
    const MINUTE: i64 = 60_000;

    /// A stand-in for the sealer: reversible, and visibly not plain.
    struct Stand;

    impl Sealer for Stand {
        fn seal(&self, plain: &str) -> String {
            format!("sealed:{}", plain.bytes().map(|b| format!("{:02x}", b ^ 0x5a)).collect::<String>())
        }
        fn open(&self, sealed: &str) -> Option<String> {
            let hex = sealed.strip_prefix("sealed:")?;
            let bytes: Option<Vec<u8>> = (0..hex.len()).step_by(2).map(|i| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()).map(|b| b ^ 0x5a)).collect();
            String::from_utf8(bytes?).ok()
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-replies-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn words() -> std::sync::Arc<crate::words::Words> {
        let config: crate::config::Config = toml::from_str("[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n").unwrap();
        crate::words::Words::of(&config)
    }

    fn fr() -> Option<&'static Region> {
        crate::phones::region_named("FR")
    }

    /// Signal's notification of a conversation with Alice (fiction numbers):
    /// her two messages, the second at `last`; with `mine`, your reply after them.
    fn signal(last: i64, mine: Option<&str>) -> (Posted, Extra) {
        let alice = Person { name: "Alice".into(), key: "alice".into(), uri: "tel:+33199001234".into(), ..Person::default() };
        let mut messages = vec![Message { sender: Some(alice.clone()), text: "Tu passes ?".into() }, Message { sender: Some(alice.clone()), text: "Je suis au café.".into() }];
        let mut times = vec![ExtraMessage { at: last - MINUTE, picture: false }, ExtraMessage { at: last, picture: false }];
        if let Some(mine) = mine {
            messages.push(Message { sender: None, text: mine.into() });
            times.push(ExtraMessage { at: last + MINUTE, picture: false });
        }
        let posted = Posted {
            key: "0|org.thoughtcrime.securesms|42|null|10123".into(),
            package: "org.thoughtcrime.securesms".into(),
            app: "Signal".into(),
            category: "msg".into(),
            template: "android.app.Notification$MessagingStyle".into(),
            title: "Alice".into(),
            shortcut: "alice-thread".into(),
            is_conversation: true,
            messages,
            people: vec![alice],
            ..Posted::default()
        };
        (posted, Extra { when: last, posted: last, messages: times, ..Extra::default() })
    }

    fn due_for(posted: &Posted, key: &str, line_at: i64, at: i64) -> Due {
        let talk = match appnotes::classify(posted, &appnotes::Choices::default(), &words()) {
            Kind::People(talk) => talk.conversation.map(|c| c.key).unwrap_or_default(),
            _ => String::new(),
        };
        Due { key: key.into(), verb: "reply".into(), notice: notice(&posted.package, &posted.key), app: posted.package.clone(), talk, from: "+33199001234".into(), name: "Alice".into(), line_at, words: "J'arrive.".into(), at }
    }

    /// The notification's name off the phone is a hash of its app and
    /// Android's key, the conversation's key's kind (AppNotes.talkKey checks
    /// the same values on a JVM), never the key.
    #[test]
    fn a_notification_is_named_by_a_hash_made_on_the_phone() {
        let named = notice("org.thoughtcrime.securesms", "0|org.thoughtcrime.securesms|42|null|10123");
        assert_eq!(named, appnotes::talk_key("org.thoughtcrime.securesms", "0|org.thoughtcrime.securesms|42|null|10123"));
        assert_eq!(named, "a2b7df14a4081c04", "as AppNotes.talkKey makes it (android/jvm-checks/RepliesCheck.java)");
        assert!(!named.contains("10123"));
        assert_ne!(named, notice("org.thoughtcrime.securesms", "0|org.thoughtcrime.securesms|43|null|10123"), "another notification, another name");
    }

    /// What is due goes to the listener sealed, is taken once ("taken"
    /// flushed first), never again after a word of it, and not at all when
    /// the listener comes late; what it did is said without the words.
    #[test]
    fn a_due_reply_is_taken_once_and_never_late() {
        let dir = scratch("take");
        let (posted, _) = signal(NOW - 5 * MINUTE, None);
        let first = due_for(&posted, "k1", NOW - 5 * MINUTE, NOW);
        let second = due_for(&posted, "k2", NOW - 5 * MINUTE, NOW - 3 * MINUTE);
        write_due(&dir, &[first.clone(), second.clone()], &Stand).unwrap();
        let on_disk = std::fs::read_to_string(dir.join(DUE)).unwrap();
        assert!(!on_disk.contains("J'arrive") && !on_disk.contains(&first.notice), "sealed at rest");
        assert_eq!(waiting(&dir, &Stand).len(), 2);
        assert_eq!(take(&dir, "k1", &Stand, NOW + 1_000).unwrap(), Taken::Go(Box::new(first)));
        assert_eq!(take(&dir, "k1", &Stand, NOW + 2_000).unwrap(), Taken::No, "once");
        assert_eq!(take(&dir, "k2", &Stand, NOW + 1_000).unwrap(), Taken::Late, "handed over three minutes before");
        assert_eq!(take(&dir, "k2", &Stand, NOW + 1_000).unwrap(), Taken::No);
        assert_eq!(take(&dir, "k3", &Stand, NOW).unwrap(), Taken::No, "not due");
        say(&dir, &Done { crossed: true, ..Done::of("k1", "handed", NOW + 3_000) }).unwrap();
        let done = read_done(&dir);
        assert_eq!(done.iter().map(|d| (d.key.as_str(), d.state.as_str())).collect::<Vec<_>>(), [("k1", "taken"), ("k2", "late"), ("k1", "handed")]);
        assert!(done[2].crossed);
        assert!(waiting(&dir, &Stand).is_empty());
        let said = std::fs::read_to_string(dir.join(DONE)).unwrap();
        assert!(!said.contains("J'arrive") && !said.contains("Alice"), "no words, no names: {said}");
        // Another key cannot open what is due: nothing is due then.
        struct Other;
        impl Sealer for Other {
            fn seal(&self, plain: &str) -> String {
                plain.to_string()
            }
            fn open(&self, _: &str) -> Option<String> {
                None
            }
        }
        assert!(read_due(&dir, &Other).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The notification found by its name is checked against the line
    /// answered: the same conversation goes, with whether their newer
    /// messages crossed the reply; another app, another notification or
    /// another conversation in it (an app that reuses one) is "gone"; a line
    /// without a conversation's key is matched by its sender's number, else
    /// name.
    #[test]
    fn the_live_notification_must_be_the_conversation_answered() {
        let words = words();
        let choices = appnotes::Choices::default();
        let (posted, extra) = signal(NOW - 5 * MINUTE, None);
        let due = due_for(&posted, "k1", NOW - 5 * MINUTE, NOW);
        assert!(!due.talk.is_empty());
        assert_eq!(check(&due, &posted, &extra, &choices, &words, fr()), Checked::Go { crossed: false });
        // She wrote again since: it crossed.
        let (newer, newer_extra) = signal(NOW - MINUTE, None);
        assert_eq!(check(&due, &newer, &newer_extra, &choices, &words, fr()), Checked::Go { crossed: true });
        // Another app, another notification, another conversation under the same notification.
        assert_eq!(check(&due, &Posted { package: "org.telegram.messenger".into(), ..posted.clone() }, &extra, &choices, &words, fr()), Checked::Gone);
        assert_eq!(check(&due, &Posted { key: "0|org.thoughtcrime.securesms|43|null|10123".into(), ..posted.clone() }, &extra, &choices, &words, fr()), Checked::Gone);
        assert_eq!(check(&due, &Posted { shortcut: "bob-thread".into(), ..posted.clone() }, &extra, &choices, &words, fr()), Checked::Gone);
        // A line without a conversation's key: her number, else her name.
        let bare = Due { talk: String::new(), ..due.clone() };
        assert_eq!(check(&bare, &posted, &extra, &choices, &words, fr()), Checked::Go { crossed: false });
        assert_eq!(check(&Due { from: "+33199009999".into(), ..bare.clone() }, &posted, &extra, &choices, &words, fr()), Checked::Gone);
        assert_eq!(check(&Due { from: String::new(), ..bare.clone() }, &posted, &extra, &choices, &words, fr()), Checked::Go { crossed: false });
        assert_eq!(check(&Due { from: String::new(), name: "Bob".into(), ..bare }, &posted, &extra, &choices, &words, fr()), Checked::Gone);
    }

    /// Your reply shown in its notification as yours: Signal's reposted
    /// conversation with your words last, or the Reply's own history; never
    /// someone else's words, never another reply's.
    #[test]
    fn a_reply_is_shown_when_its_notification_shows_your_words() {
        let hash = words_hash("J'arrive dans dix minutes.");
        let (before, extra) = signal(NOW, None);
        assert!(!shows(&before, &extra, &hash));
        let (after, extra) = signal(NOW, Some("J'arrive dans dix minutes. "));
        assert!(shows(&after, &extra, &hash), "your message, the trailing space aside");
        assert!(!shows(&after, &extra, &words_hash("Une autre réponse.")));
        let (plain, extra) = signal(NOW, None);
        assert!(shows(&plain, &Extra { history: vec!["J'arrive dans dix minutes.".into()], ..extra.clone() }, &hash), "the Reply's history");
        assert!(!shows(&plain, &extra, &words_hash("Je suis au café.")), "her words are not yours");
    }

    /// The live lines: one per change of what a notification offers, its
    /// going said once, what each phone said last read by any device; no
    /// words, no names.
    #[test]
    fn what_a_notification_offers_is_said_once_per_change() {
        let root = scratch("live");
        let own = live_file(&root, "phone");
        let live = Live { notice: "0123456789abcdef".into(), app: "org.thoughtcrime.securesms".into(), talk: "fedcba9876543210".into(), reply: true, read: true, at: NOW, ..Live::default() };
        assert!(offer(&own, &live).unwrap());
        assert!(!offer(&own, &Live { at: NOW + MINUTE, ..live.clone() }).unwrap(), "the same offer: no line");
        assert!(offer(&own, &Live { read: false, at: NOW + 2 * MINUTE, ..live.clone() }).unwrap(), "Mark as read gone");
        assert!(went(&own, &live.notice, NOW + 3 * MINUTE).unwrap());
        assert!(!went(&own, &live.notice, NOW + 4 * MINUTE).unwrap(), "said once");
        assert!(!went(&own, "aaaaaaaaaaaaaaaa", NOW).unwrap(), "never relayed: nothing");
        assert!(offer(&own, &Live { at: NOW + 5 * MINUTE, ..live.clone() }).unwrap(), "posted again");
        let text = std::fs::read_to_string(&own).unwrap();
        assert_eq!(text.lines().count(), 4);
        assert!(!text.contains("Alice"));
        let read = read_live(&root, NOW + 6 * MINUTE);
        let last = read.get(&("phone".to_string(), live.notice.clone())).unwrap();
        assert_eq!((last.reply, last.read, last.gone, last.device.as_str()), (true, true, 0, "phone"));
        assert!(read_live(&root, NOW + 8 * DAY_MS).is_empty(), "a week");
        // Trimmed with the log, by its phone after a week.
        assert_eq!(phonemsgs::trim_own(&root, "phone", NOW + 8 * DAY_MS).unwrap(), 4);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// This phone's line for Alice's last message in Signal (fiction numbers).
    fn alice_line() -> phonemsgs::Line {
        phonemsgs::Line { at: NOW - 5 * MINUTE, id: format!("{}-aa11", NOW - 5 * MINUTE), app: "org.thoughtcrime.securesms".into(), label: "Signal".into(), kind: "chat".into(), talk: "fedcba9876543210".into(), notice: "0123456789abcdef".into(), name: "Alice".into(), key: "+33199001234".into(), who: "neutral".into(), text: "Je suis au café.".into(), shows: NOW - 5 * MINUTE, ..phonemsgs::Line::default() }
    }

    fn ask(key: &str, words: &str, written: i64) -> Ask {
        Ask { key: key.into(), phone: "phone".into(), verb: "reply".into(), notice: "0123456789abcdef".into(), app: "org.thoughtcrime.securesms".into(), line: alice_line().id, words: words.into(), written, device: "desk".into(), ..Ask::default() }
    }

    /// A request is written padded and sealed, once by its key and verb, and
    /// read back with its computer; a "cancel" of the same key is set apart;
    /// each key once across the places it is read from.
    #[test]
    fn a_request_travels_padded_and_sealed() {
        let root = scratch("asks");
        let first = ask("k1", "J'arrive.", NOW);
        assert!(write_ask(&root, "desk", &first, &Stand).unwrap());
        assert!(!write_ask(&root, "desk", &first, &Stand).unwrap(), "once");
        assert!(write_ask(&root, "desk", &Ask { verb: "cancel".into(), words: String::new(), ..first.clone() }, &Stand).unwrap(), "taken back: a line of its own");
        let text = std::fs::read_to_string(calls::own_file(&root, ASK, "desk")).unwrap();
        assert!(!text.contains("arrive") && !text.contains("securesms"), "sealed");
        assert!(text.lines().all(|l| Stand.open(l).is_some_and(|plain| plain.len() % PAD_STEP == 0)), "padded");
        let other = scratch("asks-copy");
        write_ask(&other, "desk", &first, &Stand).unwrap();
        write_ask(&other, "laptop", &ask("k2", "Bien reçu.", NOW), &Stand).unwrap();
        let (asks, cancelled) = read_asks(&[root.clone(), other.clone()], &Stand);
        assert_eq!(asks.iter().map(|a| (a.key.as_str(), a.device.as_str())).collect::<Vec<_>>(), [("k1", "desk"), ("k2", "laptop")]);
        assert!(cancelled.contains(&("desk".to_string(), "k1".to_string())));
        assert_eq!(asks[0].words, "J'arrive.");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&other);
    }

    /// The never-twice decision, step by step, as for texts: another
    /// phone's; decided already; taken back; a device the sharing no longer
    /// counts; a verb, words or a line it does not take; a code; an app
    /// switched off; a short number in the SMS app; before the mark, ahead,
    /// expired; the same words within ten minutes unless sent again on
    /// purpose; sixty in an hour.
    #[test]
    fn a_request_is_decided_once_as_texts_are() {
        let dir = scratch("verdict");
        let ledger = crate::texts::Ledger::open(&dir, NOW - 60 * MINUTE).unwrap();
        let lines = vec![alice_line(), phonemsgs::Line { id: "code".into(), code: true, ..alice_line() }];
        let none = BTreeSet::new();
        let yes = |_: &str| true;
        let judge = |ledger: &crate::texts::Ledger, cancelled: &BTreeSet<(String, String)>, counted: &dyn Fn(&str) -> bool, answers: &dyn Fn(&str) -> bool, lines: &[phonemsgs::Line], a: &Ask| {
            verdict(a, &Judge { here: "phone", now_ms: NOW, ledger, lines, counted, cancelled, answers })
        };
        let v = |a: &Ask| judge(&ledger, &none, &yes, &yes, &lines, a);
        let good = ask("k1", "J'arrive.", NOW - MINUTE);
        assert_eq!(v(&good), Verdict::Send);
        assert_eq!(v(&Ask { phone: "other-phone".into(), ..good.clone() }), Verdict::NotMine);
        assert_eq!(v(&Ask { verb: "cancel".into(), ..good.clone() }), Verdict::NotMine, "read with what it takes back");
        let cancelled: BTreeSet<(String, String)> = [("desk".to_string(), "k1".to_string())].into();
        assert_eq!(judge(&ledger, &cancelled, &yes, &yes, &lines, &good), Verdict::TakenBack);
        assert_eq!(judge(&ledger, &cancelled, &yes, &yes, &lines, &Ask { device: "laptop".into(), ..good.clone() }), Verdict::Send, "another computer cannot take it back");
        assert_eq!(judge(&ledger, &none, &|d: &str| d != "desk", &yes, &lines, &good), Verdict::Refused("not-counted"));
        assert_eq!(v(&Ask { verb: "delete".into(), ..good.clone() }), Verdict::Refused("verb"));
        assert_eq!(v(&Ask { words: "  ".into(), ..good.clone() }), Verdict::Refused("no-words"));
        assert_eq!(v(&Ask { words: "é".repeat(WORDS_MAX + 1), ..good.clone() }), Verdict::Refused("too-long"));
        assert_eq!(v(&Ask { verb: "read".into(), words: String::new(), ..good.clone() }), Verdict::Send, "Mark as read: no words");
        assert_eq!(v(&Ask { line: "nowhere".into(), ..good.clone() }), Verdict::Refused("line"));
        assert_eq!(v(&Ask { notice: "ffffffffffffffff".into(), ..good.clone() }), Verdict::Refused("line"), "another notification than the line's");
        assert_eq!(v(&Ask { line: "code".into(), ..good.clone() }), Verdict::Refused("code"));
        assert_eq!(judge(&ledger, &none, &yes, &|app: &str| app != "org.thoughtcrime.securesms", &lines, &good), Verdict::Refused("switched-off"));
        let service = vec![phonemsgs::Line { kind: "text".into(), app: "foundation.e.message".into(), who: "automaton".into(), key: String::new(), name: "AMELI".into(), ..alice_line() }];
        assert_eq!(judge(&ledger, &none, &yes, &yes, &service, &Ask { app: "foundation.e.message".into(), ..good.clone() }), Verdict::Refused("short-number"));
        let short = vec![phonemsgs::Line { kind: "text".into(), app: "foundation.e.message".into(), key: "36179".into(), ..alice_line() }];
        assert_eq!(judge(&ledger, &none, &yes, &yes, &short, &Ask { app: "foundation.e.message".into(), ..good.clone() }), Verdict::Refused("short-number"));
        assert_eq!(v(&Ask { written: NOW - 2 * 60 * MINUTE, ..good.clone() }), Verdict::Refused("before-mark"));
        assert_eq!(v(&Ask { written: NOW + 5 * MINUTE, ..good.clone() }), Verdict::Clocks);
        assert_eq!(v(&Ask { written: NOW + 5 * MINUTE, ahead: 4 * MINUTE, ..good.clone() }), Verdict::Send, "the writer's clock ahead, corrected");
        assert_eq!(v(&Ask { written: NOW - 16 * MINUTE, ..good.clone() }), Verdict::Expired);
        // Claimed: decided for good, whatever is read again.
        crate::texts::record(&dir, &claim_of(&good, NOW - MINUTE)).unwrap();
        let ledger = crate::texts::Ledger::open(&dir, NOW).unwrap();
        let v = |a: &Ask| judge(&ledger, &none, &yes, &yes, &lines, a);
        assert_eq!(v(&good), Verdict::Decided);
        assert!(!std::fs::read_to_string(dir.join(crate::texts::LEDGER)).unwrap().contains("arrive"), "the ledger keeps a hash, never the words");
        // The same words to the same notification within ten minutes: refused, unless sent again on purpose.
        assert_eq!(v(&ask("k2", "J'arrive. ", NOW)), Verdict::Refused("same-words"));
        assert_eq!(v(&Ask { again: "k1".into(), ..ask("k2", "J'arrive.", NOW) }), Verdict::Send);
        assert_eq!(v(&ask("k2", "Dans cinq minutes.", NOW)), Verdict::Send);
        // Sixty replies in the last hour, from all computers.
        for i in 0..PER_HOUR {
            crate::texts::record(&dir, &claim_of(&ask(&format!("n{i}"), &format!("Réponse {i}"), NOW), NOW - 30 * MINUTE)).unwrap();
        }
        let ledger = crate::texts::Ledger::open(&dir, NOW).unwrap();
        assert_eq!(judge(&ledger, &none, &yes, &yes, &lines, &ask("k3", "Encore une.", NOW)), Verdict::Refused("too-many"));
        assert_eq!(judge(&ledger, &none, &yes, &yes, &lines, &Ask { verb: "read".into(), words: String::new(), ..ask("k4", "", NOW) }), Verdict::Send, "a Mark as read counts for none");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// From a claim to an outcome: what is due goes to the listener once;
    /// one never taken in time is "late", one taken and never said "doubt";
    /// each state an outcome once, without the words; old lines go.
    #[test]
    fn what_the_listener_did_becomes_the_phone_s_word_once() {
        let dir = scratch("settle");
        let line = alice_line();
        let (a, b, c) = (ask("k1", "J'arrive.", NOW), ask("k2", "Merci.", NOW), ask("k3", "Oui.", NOW));
        add_due(&dir, &[due_of(&a, &line, NOW), due_of(&b, &line, NOW), due_of(&c, &line, NOW)], &Stand, NOW).unwrap();
        assert_eq!(read_due(&dir, &Stand)[0].from, "+33199001234");
        assert_eq!(read_due(&dir, &Stand)[0].line_at, line.at);
        assert!(matches!(take(&dir, "k1", &Stand, NOW + 1_000).unwrap(), Taken::Go(_)));
        say(&dir, &Done::of("k1", "handed", NOW + 2_000)).unwrap();
        assert!(matches!(take(&dir, "k2", &Stand, NOW + 1_000).unwrap(), Taken::Go(_)));
        let claimed = vec![("k1".to_string(), NOW), ("k2".to_string(), NOW), ("k3".to_string(), NOW)];
        let done = settle(&dir, &claimed, NOW + 3 * MINUTE).unwrap();
        let states: Vec<(&str, &str)> = done.iter().map(|d| (d.key.as_str(), d.state.as_str())).collect();
        assert!(states.contains(&("k2", "doubt")) && states.contains(&("k3", "late")), "{states:?}");
        assert!(!states.contains(&("k1", "doubt")));
        assert_eq!(take(&dir, "k3", &Stand, NOW + 3 * MINUTE).unwrap(), Taken::No, "late is the end");
        // Each state once in the phone's own outcomes, without the words.
        let root = scratch("outcomes");
        let own = calls::own_file(&root, OUTCOME, "phone");
        assert_eq!(say_outcomes(&own, &outcomes_of(&done)).unwrap(), done.len());
        assert_eq!(say_outcomes(&own, &outcomes_of(&done)).unwrap(), 0, "once");
        let text = std::fs::read_to_string(&own).unwrap();
        assert!(!text.contains("arrive") && !text.contains("Alice"));
        let read = read_outcomes(&root);
        assert!(read.iter().all(|o| o.device == "phone"));
        // A day on, the private lines go; a week on, the phone's own outcomes, and another's ten days on.
        trim_private(&dir, &Stand, NOW + 2 * DAY_MS).unwrap();
        assert!(read_due(&dir, &Stand).is_empty() && read_done(&dir).is_empty());
        let other = calls::own_file(&root, OUTCOME, "old-phone");
        say_outcomes(&other, &[Outcome { key: "k9".into(), state: "handed".into(), at: NOW, ..Outcome::default() }]).unwrap();
        write_ask(&root, "phone", &a, &Stand).unwrap();
        assert_eq!(trim_shared(&root, "phone", &Stand, NOW + 8 * DAY_MS).unwrap(), done.len() + 1, "the phone's own, its outcomes and its request");
        assert_eq!(read_outcomes(&root).len(), 1, "another's waits three more days");
        assert_eq!(trim_shared(&root, "phone", &Stand, NOW + 11 * DAY_MS).unwrap(), 1);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Each state in words, in English and French, never "Sent": waiting
    /// (with Take back), taken, handed (crossed, held), shown, every reason
    /// it was not sent (with Send again or "Reply on your phone" where they
    /// fit); a Mark as read's own words.
    #[test]
    fn each_state_is_said_in_words() {
        let at = |o: &str, reason: &str| Outcome { key: "k1".into(), state: o.into(), at: NOW, reason: reason.into(), ..Outcome::default() };
        let time = |_: i64| "14:03".to_string();
        for language in ["en", "fr"] {
            let tr = crate::i18n::Translator::new(language);
            let a = ask("k1", "J'arrive.", NOW);
            let waiting = state_of(&a, &[], "Signal", &tr, Some(NOW), &time);
            assert!(waiting.waiting && waiting.said.contains("14:03"), "{language}: {waiting:?}");
            assert!(state_of(&a, &[], "Signal", &tr, None, &time).waiting);
            let states = ["taken", "handed", "shown", "gone", "no-reply", "no-read", "closed", "locked", "activity", "late", "failed", "expired", "clocks", "doubt", "taken-back"];
            for state in states {
                let o = at(state, "");
                let said = state_of(&a, &[&o], "Signal", &tr, Some(NOW), &time);
                assert!(!said.waiting && !said.said.starts_with("replies-") && !said.said.is_empty(), "{language} {state}: {said:?}");
                assert!(!said.said.to_lowercase().contains("sent at") && !said.said.contains("Envoyé"), "never a plain Sent: {language} {state}");
            }
            for reason in ["same-words", "too-many", "too-long", "no-words", "line", "code", "switched-off", "short-number", "not-counted", "before-mark"] {
                let o = at("refused", reason);
                let said = state_of(&a, &[&o], "Signal", &tr, Some(NOW), &time);
                assert!(!said.said.starts_with("replies-"), "{language} {reason}");
            }
            let (handed, shown) = (at("handed", ""), Outcome { at: NOW + 1, ..at("shown", "") });
            assert_eq!(state_of(&a, &[&handed, &shown], "Signal", &tr, None, &time).state, "shown", "the furthest it went");
            let gone = state_of(&a, &[&at("gone", "")], "Signal", &tr, None, &time);
            assert!(gone.on_phone && !gone.again);
            assert!(state_of(&a, &[&at("expired", "")], "Signal", &tr, None, &time).again);
            let crossed = Outcome { crossed: true, ..at("handed", "") };
            assert_ne!(state_of(&a, &[&crossed], "Signal", &tr, None, &time).said, state_of(&a, &[&handed], "Signal", &tr, None, &time).said, "crossed said");
            let read = Ask { verb: "read".into(), words: String::new(), ..a.clone() };
            assert_ne!(state_of(&read, &[&handed], "Signal", &tr, None, &time).said, state_of(&a, &[&handed], "Signal", &tr, None, &time).said, "a Mark as read's own words");
        }
    }
}

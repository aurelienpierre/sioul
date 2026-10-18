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
//!   and sealed with a key made from the sharing key, its writer sealed in it
//!   (and copied to the sharing folder, sent on the lane texts use); a line
//!   found in another device's file than its writer's is passed over. The
//!   ledger's mark is made at every phone step and when the listener relays
//!   a line for your computers (`mark_ledger`). The phone decides each once
//!   by the texts' rules (a ledger flushed before the listener is asked, a
//!   mark, a request's life of a quarter of an hour, the doubt after a
//!   crash), with two guards of its own; and says what became of each in its
//!   own `phone-messages/outcome/<phone>.jsonl` (`Outcome`), never the words,
//!   which a computer says in words (`state_of`), never a plain "Sent".
//! - **On a computer's Porch** (`answer_of`): a line offers Reply while its
//!   phone says the notification's Reply works, a text goes as one of
//!   Sioul's own where this computer reads them, the replies written there
//!   show under their messages with their states. A conversation answered
//!   comes through at once on the phone for twenty minutes after
//!   (`note_answered`, `answered_lately`), never during a pause.

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
    /// The line answered is a group's, and its title then (`phonemsgs::Line`):
    /// the live conversation must have the same shape (`check`).
    #[serde(skip_serializing_if = "is_false")]
    pub group: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
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
    /// was not sent: "gone" (no such notification now), "other" (another
    /// conversation in it now, or nothing that tells: `check`), "no-reply" /
    /// "no-read" (it offers none now),
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

/// The replies' ledger (`texts::Ledger`, in the private folder), its mark
/// made now when there is none; the folder made private first.
pub fn open_ledger(folder: &Path, now_ms: i64) -> Result<crate::texts::Ledger, String> {
    private_dir(folder)?;
    crate::texts::Ledger::open(folder, now_ms)
}

/// The ledger's mark made once, when there is none (a request written
/// before it is refused, as for texts: a ledger lost cannot make old
/// requests new). Made at every phone step, before the requests are read,
/// and when the listener relays a line for your computers
/// (`phonemsgs::note`): a reply answers such a line, so the mark is older
/// than the first reply. Never moved once made.
pub fn mark_ledger(folder: &Path, now_ms: i64) -> Result<(), String> {
    private_dir(folder)?;
    if folder.join(crate::texts::MARK).is_file() {
        return Ok(());
    }
    crate::filelock::with_lock(&folder.join(crate::texts::LEDGER), || crate::texts::Ledger::open(folder, now_ms).map(|_| ()))
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

/// Lines of what the listener did appended together and flushed once; nothing for none.
fn append_all_flushed(path: &Path, done: &[Done]) -> Result<(), String> {
    use std::io::Write;
    if done.is_empty() {
        return Ok(());
    }
    let mut text = String::new();
    for d in done {
        text.push_str(&serde_json::to_string(d).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    calls::append_private(path, "")?;
    let mut file = std::fs::OpenOptions::new().append(true).open(path).map_err(fail)?;
    file.write_all(text.as_bytes()).map_err(fail)?;
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
        // Its words are of no use there once taken: what is due written again
        // without them (left as they were, sealed, until the day's trim when it fails).
        let rest: Vec<Due> = read_due(folder, seal).into_iter().map(|d| if d.key == key { Due { words: String::new(), ..d } } else { d }).collect();
        let _ = write_due(folder, &rest, seal);
        Ok(Taken::Go(Box::new(due)))
    })
}

// ---------------------------------------------------------------- the live notification, checked

/// What the live notification found for a due one says (`check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checked {
    /// The same app, notification and conversation: send it. `crossed`:
    /// their messages newer than the one you answered are in it, by the
    /// messages' own times (none given: not said).
    Go { crossed: bool },
    /// No longer that notification: "gone".
    Gone,
    /// That notification, but another conversation in it now (another key,
    /// a group for a conversation between two or the other way round,
    /// another group's title, another sender), or nothing that tells which
    /// conversation the line was: "other", never a guess.
    Other,
}

/// Whether the notification found now by its name is still the one a
/// request answered: the same app and name (`notice`), and the same
/// conversation as the line answered, read as when the line was made
/// (`appnotes::classify`): the same shape (a group or not, a group's title),
/// then its key; else, for a line without one, the same sender's number,
/// else name; a line that names none of these never passes. What the shade's
/// own Reply guarantees, and more: the Reply under a notification is that
/// conversation's (apps make one per conversation, as Android tells them),
/// and an app that reuses one notification for another conversation is caught.
pub fn check(due: &Due, posted: &Posted, extra: &Extra, choices: &appnotes::Choices, words: &crate::words::Words, region: Option<&Region>) -> Checked {
    if posted.package != due.app || posted.key.is_empty() || notice(&posted.package, &posted.key) != due.notice {
        return Checked::Gone;
    }
    let conversation = match appnotes::classify(posted, choices, words) {
        Kind::People(talk) => talk.conversation,
        _ => None,
    };
    let (group, title) = conversation.as_ref().map_or((false, String::new()), |c| (c.group, phonemsgs::title_of(c)));
    if group != due.group || (due.group && title != due.title) {
        return Checked::Other;
    }
    // A group whose key came from its last writer names a person, not a group: two such groups where she wrote last look alike.
    if due.group && conversation.as_ref().is_some_and(|c| c.by_writer) {
        return Checked::Other;
    }
    let same = if !due.talk.is_empty() {
        conversation.as_ref().is_some_and(|c| c.key == due.talk)
    } else if !due.from.is_empty() {
        phonemsgs::writers(posted, region).0.contains(&due.from)
    } else if !due.name.is_empty() {
        phonemsgs::writers(posted, region).1.contains(&due.name)
    } else {
        false
    };
    if !same {
        return Checked::Other;
    }
    Checked::Go { crossed: phonemsgs::newest_timed_from_others(posted, extra) > due.line_at }
}

/// A reply's words as the phone compares them, never kept: a hash.
pub fn words_hash(words: &str) -> String {
    appnotes::talk_key("replies", words.trim())
}

/// Whether a notification shows a reply's words as the person's own (its
/// hash, `words_hash`), as Signal and Element add your reply; the phone then
/// says "shown". A message of yours in the conversation (no sender, or the
/// conversation's own user) timed after the reply was taken (`since`, the
/// phone's clock, a second's slack); where the notification gives no time,
/// its words there, or in what went through its Reply lately
/// (`Extra::history`), only when they were not there already before the
/// reply (`before`, `shown_already`): an earlier "Oui" of yours never makes
/// a later "Oui" shown.
pub fn shows(posted: &Posted, extra: &Extra, hash: &str, since: i64, before: bool) -> bool {
    let mut untimed = false;
    for (i, m) in posted.messages.iter().enumerate() {
        if m.sender.as_ref().is_some_and(|s| !phonemsgs::is_user(posted, s)) || words_hash(&m.text) != hash {
            continue;
        }
        match extra.messages.get(i).map(|e| e.at).filter(|at| *at > 0) {
            Some(at) if at >= since - 1_000 => return true,
            Some(_) => {}
            None => untimed = true,
        }
    }
    !before && (untimed || extra.history.iter().any(|words| words_hash(words) == hash))
}

/// Whether a notification shows those words as yours already, whatever
/// their time: looked at before the reply is sent (`shows`'s `before`).
pub fn shown_already(posted: &Posted, extra: &Extra, hash: &str) -> bool {
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
/// first, unless the request sends that first one again on purpose; one its
/// phone said was not sent never counts (`unsent`).
pub const SAME_WORDS_MS: i64 = 10 * 60_000;
/// What the listener says of a reply that did not go (`Done::state`): such a
/// reply never blocks the same words again.
pub const NOT_SENT: &[&str] = &["gone", "other", "no-reply", "no-read", "closed", "locked", "activity", "late", "failed"];
/// At most this many requests decided in one step (`verdict`, the rest at
/// the next), taken from each computer in turn (`in_turn`): a runaway
/// computer's thousands never hold a step long, nor keep another computer's
/// reply from its turn.
pub const STEP_MAX: usize = 100;

/// The requests for this phone (`here`), each computer's in its own order,
/// one computer's after another's in turn.
pub fn in_turn<'a>(asks: &'a [Ask], here: &str) -> Vec<&'a Ask> {
    let mut by: BTreeMap<&str, Vec<&Ask>> = BTreeMap::new();
    for ask in asks.iter().filter(|a| a.phone == here) {
        by.entry(ask.device.as_str()).or_default().push(ask);
    }
    let longest = by.values().map(Vec::len).max().unwrap_or(0);
    (0..longest).flat_map(|i| by.values().filter_map(move |list| list.get(i).copied())).collect()
}
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
    /// The device that wrote it, its name in the sharing, sealed with it
    /// (`write_ask`): a line read from another device's file than its
    /// writer's is passed over (`read_asks`), so that nothing (the server, a
    /// stray file) moves a request from one computer's name to another's. Any
    /// device that holds the passphrase can still write in any name.
    pub writer: String,
    /// Spaces that make the line's length a multiple of `PAD_STEP` (`padded_ask`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub pad: String,
    /// The computer that wrote it: its sealed writer, found in its own file. Never written.
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
/// messages' folder), its writer (`device`) sealed in it, padded and
/// sealed, under its lock: once by its key and verb.
pub fn write_ask(root: &Path, device: &str, ask: &Ask, seal: &dyn Sealer) -> Result<bool, String> {
    let own = calls::own_file(root, ASK, device);
    let ask = Ask { writer: device.to_string(), ..ask.clone() };
    crate::filelock::with_lock(&own, || {
        if crate::texts::read_file::<Ask>(&own, seal).iter().any(|a| a.key == ask.key && a.verb == ask.verb) {
            return Ok(false);
        }
        calls::append_private(&own, &format!("{}\n", seal.seal(&padded_ask(&ask)))).map(|()| true)
    })
}

/// Every computer's requests under each root (the records' copy, the
/// sharing folder's small files, the copy fetched from the server), each
/// with its computer, its sealed writer: each key once, the first root's
/// first; and the keys each computer took back ("cancel"), by computer and
/// key. A line whose sealed writer is not its file's device (moved from
/// another device's file, or written before the writer was sealed) is
/// passed over, a cancel too.
pub fn read_asks(roots: &[PathBuf], seal: &dyn Sealer) -> (Vec<Ask>, BTreeSet<(String, String)>) {
    let mut asks: Vec<Ask> = Vec::new();
    let mut known: BTreeSet<String> = BTreeSet::new();
    let mut cancelled = BTreeSet::new();
    for root in roots {
        for (device, path) in calls::device_files(&root.join(ASK)) {
            for ask in crate::texts::read_file::<Ask>(&path, seal) {
                if ask.key.is_empty() || ask.writer.is_empty() || calls::file_safe(&ask.writer) != device {
                    continue;
                }
                let ask = Ask { device: ask.writer.clone(), ..ask };
                if ask.verb == "cancel" {
                    cancelled.insert((ask.device.clone(), ask.key.clone()));
                } else if known.insert(ask.key.clone()) {
                    asks.push(ask);
                }
            }
        }
    }
    (asks, cancelled)
}

// ---------------------------------------------------------------- how far the phone is done with each file

/// How far the phone is done with each file of requests, in its private
/// folder (`ReadMark`): past every line decided, another phone's, or that
/// does not open (`moved_marks`). A step reads each file from there
/// (`read_asks_after`): a computer's thousands decided already are never
/// opened again.
pub const READ_MARKS: &str = "asks-read.json";

/// A file of requests done with up to `offset` bytes, its last line done
/// with by its hash (`line`): read from there while the file still holds
/// that line there, else whole again (rewritten, trimmed).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadMark {
    pub offset: u64,
    pub line: String,
}

/// A file's lines read at a step (`read_asks_after`), from `start`: each
/// with where it ends, its hash, the request's key and phone ("" for a line
/// that does not open, or another writer's).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileRead {
    pub path: String,
    pub start: u64,
    pub lines: Vec<(u64, String, String, String)>,
}

/// The marks as kept; none when the file does not read.
pub fn read_marks(folder: &Path) -> BTreeMap<String, ReadMark> {
    std::fs::read_to_string(folder.join(READ_MARKS)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// The marks written whole, beside then put in place.
pub fn save_read_marks(folder: &Path, marks: &BTreeMap<String, ReadMark>) -> Result<(), String> {
    private_dir(folder)?;
    let text = serde_json::to_string(marks).map_err(|e| e.to_string())?;
    let fresh = folder.join(format!(".{READ_MARKS}.{}.new", std::process::id()));
    let _ = std::fs::remove_file(&fresh);
    calls::append_private(&fresh, &text)?;
    std::fs::rename(&fresh, folder.join(READ_MARKS)).map_err(|e| format!("{}: {e}", folder.display()))
}

/// A sealed line's name for the marks: a hash of it as written.
fn line_hash(line: &str) -> String {
    appnotes::talk_key("asks", line)
}

/// Whether a file still holds, just before a mark's offset, the line the mark names.
fn holds(bytes: &[u8], mark: &ReadMark) -> bool {
    let end = usize::try_from(mark.offset).unwrap_or(usize::MAX);
    if end == 0 || end > bytes.len() || bytes[end - 1] != b'\n' {
        return false;
    }
    let before = &bytes[..end - 1];
    let start = before.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    line_hash(String::from_utf8_lossy(&before[start..]).trim()) == mark.line
}

/// `read_asks` for a phone's step, each file from its mark (`marks`, by its
/// path) when it still holds the line the mark names, else whole: the
/// requests past the marks, the cancels, and each file's lines read, to move
/// the marks once the step decided (`moved_marks`). A line not ended yet
/// (being written) waits for the next step.
pub fn read_asks_after(roots: &[PathBuf], seal: &dyn Sealer, marks: &BTreeMap<String, ReadMark>) -> ((Vec<Ask>, BTreeSet<(String, String)>), Vec<FileRead>) {
    let mut asks: Vec<Ask> = Vec::new();
    let mut known: BTreeSet<String> = BTreeSet::new();
    let mut cancelled = BTreeSet::new();
    let mut reads = Vec::new();
    for root in roots {
        for (device, path) in calls::device_files(&root.join(ASK)) {
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let name = path.display().to_string();
            let start = marks.get(&name).filter(|m| holds(&bytes, m)).map_or(0, |m| usize::try_from(m.offset).unwrap_or(0));
            let mut read = FileRead { path: name, start: start as u64, lines: Vec::new() };
            let mut at = start;
            for piece in bytes[start..].split_inclusive(|b| *b == b'\n') {
                if !piece.ends_with(b"\n") {
                    break;
                }
                at += piece.len();
                let text = String::from_utf8_lossy(piece);
                let line = text.trim();
                let opened = if line.is_empty() { None } else { seal.open(line).and_then(|plain| serde_json::from_str::<Ask>(&plain).ok()) };
                let (key, phone) = match opened {
                    Some(ask) if !ask.key.is_empty() && !ask.writer.is_empty() && calls::file_safe(&ask.writer) == device => {
                        let ask = Ask { device: ask.writer.clone(), ..ask };
                        let named = (ask.key.clone(), ask.phone.clone());
                        if ask.verb == "cancel" {
                            cancelled.insert((ask.device.clone(), ask.key.clone()));
                        } else if known.insert(ask.key.clone()) {
                            asks.push(ask);
                        }
                        named
                    }
                    _ => (String::new(), String::new()),
                };
                read.lines.push((at as u64, line_hash(line), key, phone));
            }
            reads.push(read);
        }
    }
    ((asks, cancelled), reads)
}

/// The marks moved past every line done with, in each file's order: a line
/// that does not open or is another writer's, or whose request is done
/// with (`done`, by its key and phone: decided, or another phone's); the
/// first line not done with stops its file's mark. Only the files read
/// now keep one.
pub fn moved_marks(reads: &[FileRead], done: &dyn Fn(&str, &str) -> bool, marks: &BTreeMap<String, ReadMark>) -> BTreeMap<String, ReadMark> {
    let mut out = BTreeMap::new();
    for read in reads {
        let mut mark = marks.get(&read.path).filter(|m| m.offset == read.start && read.start > 0).cloned().unwrap_or_default();
        for (end, hash, key, phone) in &read.lines {
            if !key.is_empty() && !done(key, phone) {
                break;
            }
            mark = ReadMark { offset: *end, line: hash.clone() };
        }
        if mark.offset > 0 {
            out.insert(read.path.clone(), mark);
        }
    }
    out
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
    /// "same-words" (the same words went), "same-words-pending" (they are on
    /// their way), "too-many", "too-many-read" (Mark as read, sixty an hour too).
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
    /// The requests its listener said were not sent (`unsent`).
    pub unsent: &'a BTreeSet<String>,
    /// The requests its listener said went (handed or shown: `went_keys`).
    pub went: &'a BTreeSet<String>,
}

/// The requests the listener said went: handed to their app, or shown.
pub fn went_keys(folder: &Path) -> BTreeSet<String> {
    read_done(folder).into_iter().filter(|d| matches!(d.state.as_str(), "handed" | "shown")).map(|d| d.key).collect()
}

/// The requests the listener said were not sent (`NOT_SENT`), from its words
/// in the private folder (a day of them: longer than the guards look back).
pub fn unsent(folder: &Path) -> BTreeSet<String> {
    read_done(folder).into_iter().filter(|d| NOT_SENT.contains(&d.state.as_str())).map(|d| d.key).collect()
}

/// The never-twice decision for a reply, step by step: another phone's (or
/// a "cancel"); decided already; taken back by its computer; asked by a
/// device the sharing no longer counts; refused (another verb, no words or
/// too many, a line not in this phone's log or of another notification, a
/// code's line, its app's answers switched off, a short number or a sender
/// named in letters in the SMS app); written before the ledger's mark;
/// dated ahead of the phone's clock; older than `texts::EXPIRY_MS`; the two
/// guards (the same words to the same notification within `SAME_WORDS_MS`,
/// from a reply that went or may still go, unless it sends the first again;
/// `PER_HOUR` replies claimed in the last hour); else sent. The time written
/// is corrected by how far the writer's clock may run ahead. `device` is
/// the request's sealed writer (`read_asks`).
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
    // The claims of the last hour: replies by their words' hash, Marks as read without one.
    let lately = j.ledger.claims.iter().filter(|c| c.state == "claimed" && j.now_ms - c.at <= HOUR_MS);
    if reply {
        let hash = words_hash(&ask.words);
        let replies: Vec<&crate::texts::Claim> = lately.filter(|c| !c.hash.is_empty()).collect();
        // A reply that went, or may still go (claimed, taken, handed, shown, in doubt); never one its phone said did not go.
        // The same notification, or another of the same conversation (its newer one, which Send again names as the
        // Porch does now): the same words never go twice (review of 18 October 2026, L-2).
        let same = |c: &crate::texts::Claim| c.to == ask.notice || (!c.talk.is_empty() && c.talk == line.talk);
        let twin = replies.iter().find(|c| !j.unsent.contains(&c.key) && same(c) && c.hash == hash && j.now_ms - c.at <= SAME_WORDS_MS && c.key != ask.again);
        if let Some(twin) = twin {
            // Said as it is known: went (its phone said so), or on its way.
            return Verdict::Refused(if j.went.contains(&twin.key) { "same-words" } else { "same-words-pending" });
        }
        if replies.len() >= PER_HOUR {
            return Verdict::Refused("too-many");
        }
    } else if lately.filter(|c| c.hash.is_empty()).count() >= PER_HOUR {
        return Verdict::Refused("too-many-read");
    }
    Verdict::Send
}

/// A request claimed, as the ledger keeps it: the notification's name, the
/// words' hash and the conversation of the line it answers (`talk`), for the
/// guards; never the words.
pub fn claim_of(ask: &Ask, talk: &str, now_ms: i64) -> crate::texts::Claim {
    crate::texts::Claim { key: ask.key.clone(), at: now_ms, state: "claimed".into(), to: ask.notice.clone(), hash: if ask.verb == "reply" { words_hash(&ask.words) } else { String::new() }, talk: talk.to_string() }
}

/// A request decided and ended at the step (taken back, expired, clocks,
/// refused and why), as the ledger keeps it: `refused:same-words` for a
/// refusal, its reason after the colon, so that its word can be said again
/// (`ends_of`).
pub fn end_of(key: &str, state: &str, reason: &str, now_ms: i64) -> crate::texts::Claim {
    let state = if reason.is_empty() { state.to_string() } else { format!("{state}:{reason}") };
    crate::texts::Claim { key: key.to_string(), at: now_ms, state, ..crate::texts::Claim::default() }
}

/// The step's decisions the ledger holds from the last day, as outcomes
/// (`end_of`), to say again those a step could not write: `say_outcomes`
/// writes each (key, state) once.
pub fn ends_of(ledger: &crate::texts::Ledger, now_ms: i64) -> Vec<Outcome> {
    ledger
        .claims
        .iter()
        .filter(|c| now_ms - c.at <= DAY_MS)
        .filter_map(|c| {
            let (state, reason) = c.state.split_once(':').unwrap_or((c.state.as_str(), ""));
            matches!(state, "taken-back" | "expired" | "clocks" | "refused").then(|| Outcome { key: c.key.clone(), state: state.to_string(), at: c.at, reason: reason.to_string(), ..Outcome::default() })
        })
        .collect()
}

/// What the listener needs of a request claimed: the line it answers, read
/// from this phone's log (its conversation, who wrote, its time), the words.
pub fn due_of(ask: &Ask, line: &phonemsgs::Line, now_ms: i64) -> Due {
    Due { key: ask.key.clone(), verb: ask.verb.clone(), notice: ask.notice.clone(), app: ask.app.clone(), talk: line.talk.clone(), from: line.key.clone(), name: line.name.clone(), group: line.group, title: line.title.clone(), line_at: line.at, words: if ask.verb == "reply" { ask.words.clone() } else { String::new() }, at: now_ms }
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
        let mut words: BTreeMap<&str, Vec<&Done>> = BTreeMap::new();
        for d in &done {
            words.entry(d.key.as_str()).or_default().push(d);
        }
        let mut ends = Vec::new();
        for (key, at) in claimed {
            let said = words.get(key.as_str()).map_or(&[][..], Vec::as_slice);
            if said.is_empty() && now_ms - at > TAKE_WITHIN_MS {
                ends.push(Done::of(key, "late", now_ms));
            } else if said.len() == 1 && said[0].state == "taken" && now_ms - said[0].at > crate::texts::LOOK_AFTER_MS {
                ends.push(Done::of(key, "doubt", now_ms));
            }
        }
        // The step's words together, one flush (a runaway computer's claims never cost one each).
        append_all_flushed(&folder.join(DONE), &ends)?;
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
/// for "Waiting for your phone"; `cancel`: this computer took it back
/// (Take back), said at once, and said too late when the phone had taken it
/// already; `time_of`: a moment (ms) as the computer says the time.
pub fn state_of(ask: &Ask, outcomes: &[&Outcome], app: &str, tr: &crate::i18n::Translator, phone_shared: Option<i64>, cancel: bool, time_of: &dyn Fn(i64) -> String) -> State {
    let mut args = crate::i18n::args();
    args.set("app", app.to_string());
    let Some(last) = outcomes.iter().copied().max_by_key(|o| (rank(&o.state), o.at)) else {
        if cancel {
            // Taken back here, the phone's word not come yet: no Take back again.
            return State { said: tr.text("replies-taking-back", None), ..State::default() };
        }
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
        ("handed", false) if last.held && last.crossed => ("replies-handed-held", false, false),
        ("handed", false) if last.crossed => ("replies-handed-crossed", false, false),
        ("handed", false) => ("replies-handed", false, false),
        ("shown", _) => ("replies-shown", false, false),
        ("taken", _) => ("replies-taken", false, false),
        ("gone", _) => ("replies-gone", false, true),
        ("other", _) => ("replies-other", false, true),
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
            // The first may yet fail: Send again goes only if it did.
            "same-words-pending" => ("replies-refused-same-words-pending", true, false),
            "too-many" => ("replies-refused-too-many", false, false),
            "too-many-read" => ("replies-refused-too-many-read", false, false),
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
    let mut said = tr.text(id, Some(&args));
    // Taken back, but the phone had taken it already: it went on, said so.
    if cancel && matches!(last.state.as_str(), "taken" | "handed" | "shown" | "doubt") {
        said = format!("{said} {}", tr.text("replies-take-back-late", None));
    }
    State { said, again, on_phone, state: last.state.clone(), ..State::default() }
}

// ---------------------------------------------------------------- a conversation answered

/// The conversations answered from a computer lately, in the phone's
/// private replies folder: each conversation's key (a hash) and until when
/// its messages come through at once (decision 18).
pub const ANSWERED: &str = "answered.json";
/// A conversation answered from a computer comes through at once this long
/// after the reply was handed, as a second call within a quarter of an hour
/// rings: their answer to your answer is not held. Never during a pause.
pub const ANSWERED_MS: i64 = 20 * 60_000;

/// The conversations answered lately, as kept (those past their time left out).
fn answered(folder: &Path, now_ms: i64) -> BTreeMap<String, i64> {
    std::fs::read_to_string(folder.join(ANSWERED)).ok().and_then(|t| serde_json::from_str::<BTreeMap<String, i64>>(&t).ok()).unwrap_or_default().into_iter().filter(|(_, until)| *until > now_ms).collect()
}

/// A conversation answered (its key, `appnotes::talk_key`): its messages
/// come through at once until `ANSWERED_MS` from now. Under the folder's lock.
pub fn note_answered(folder: &Path, talk: &str, now_ms: i64) -> Result<(), String> {
    if talk.is_empty() {
        return Ok(());
    }
    private_dir(folder)?;
    crate::filelock::with_lock(&folder.join(ANSWERED), || {
        let mut kept = answered(folder, now_ms);
        kept.insert(talk.to_string(), now_ms + ANSWERED_MS);
        let text = serde_json::to_string(&kept).map_err(|e| e.to_string())?;
        let fresh = folder.join(format!(".{ANSWERED}.{}.new", std::process::id()));
        let _ = std::fs::remove_file(&fresh);
        calls::append_private(&fresh, &text)?;
        std::fs::rename(&fresh, folder.join(ANSWERED)).map_err(|e| format!("{}: {e}", folder.display()))
    })
}

/// Whether a conversation was answered from a computer within `ANSWERED_MS`.
pub fn answered_lately(folder: &Path, talk: &str, now_ms: i64) -> bool {
    !talk.is_empty() && answered(folder, now_ms).contains_key(talk)
}

/// A number a computer texted (`phones::key`), as the conversations
/// answered keep it (`note_answered`): a hash, never the number. "" for none.
pub fn answered_number(number: &str) -> String {
    if number.is_empty() { String::new() } else { format!("tel:{}", appnotes::talk_key("tel", number)) }
}

/// What this computer answered lately, for its Porch (decision 18): the
/// conversations its replies went to, handed or shown within `ANSWERED_MS`
/// (the phone, the conversation's key), and the numbers it texted then.
/// Their lines show at once, as their phone lets them through, but in a pause.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answered {
    talks: BTreeSet<(String, String)>,
    numbers: BTreeSet<String>,
}

impl Answered {
    /// From this computer's own requests (`own`), every phone's outcomes and
    /// the phones' lines: a reply whose phone said, within `ANSWERED_MS`, that
    /// it handed it, or that its app shows it (the phone's clock may run up
    /// to `texts::AHEAD_MS` ahead of this one's).
    pub fn of(own: &[Ask], outcomes: &[Outcome], all: &[phonemsgs::Line], now_ms: i64) -> Answered {
        let lately: BTreeSet<(&str, &str)> = outcomes.iter().filter(|o| matches!(o.state.as_str(), "handed" | "shown") && (-crate::texts::AHEAD_MS..=ANSWERED_MS).contains(&(now_ms - o.at))).map(|o| (o.device.as_str(), o.key.as_str())).collect();
        let talks = own
            .iter()
            .filter(|a| a.verb == "reply" && lately.contains(&(a.phone.as_str(), a.key.as_str())))
            .filter_map(|a| all.iter().find(|l| l.device == a.phone && l.id == a.line))
            .filter(|l| !l.talk.is_empty())
            .map(|l| (l.device.clone(), l.talk.clone()))
            .collect();
        Answered { talks, numbers: BTreeSet::new() }
    }

    /// A number this computer texted lately (`phones::key`), its phone having taken it.
    pub fn texted(&mut self, number: String) {
        if !number.is_empty() {
            self.numbers.insert(number);
        }
    }

    /// Whether a line is of a conversation answered lately: its conversation
    /// on its phone, or a text from a number texted.
    pub fn holds(&self, line: &phonemsgs::Line) -> bool {
        (!line.talk.is_empty() && self.talks.contains(&(line.device.clone(), line.talk.clone()))) || (line.kind == "text" && !line.key.is_empty() && self.numbers.contains(&line.key))
    }
}

// ---------------------------------------------------------------- what a Porch's line offers

/// The apps whose Reply marks the conversation read, as their sources say
/// (Signal, Telegram, /e/OS's Message and the QKSMS it comes from): the
/// first reply in each says so.
pub const MARKS_READ: &[&str] = &["org.thoughtcrime.securesms", "org.telegram.messenger", "foundation.e.message", "com.moez.QKSMS", "dev.octoshrimpy.quik"];

/// What a Porch's line offers a reply from this computer (decision 14).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Answer {
    /// The phone the line came from, the notification answered (`notice`;
    /// the conversation's current one, when its app posted it anew), its
    /// app, its name as the phone shows it, and the newest line answered.
    pub phone: String,
    pub notice: String,
    pub app: String,
    pub label: String,
    pub line: String,
    /// The phone says its Reply works now (`Live`): else "Reply on your phone".
    pub offered: bool,
    /// Its Mark as read too.
    pub read: bool,
    /// A text from a whole number, between two, where this computer reads
    /// the texts: the reply goes as a text of Sioul's own (decision 12),
    /// with its own states.
    pub texts: bool,
    /// "Reply to Alice on Signal…", "Reply in École on WhatsApp…".
    pub placeholder: String,
    /// The first reply in an app that marks the conversation read says so; "" otherwise.
    pub note: String,
    /// "Mark as read in Signal", and its warning: "Signal may tell them that you read it."
    pub read_item: String,
    pub read_warning: String,
    /// "Reply on your phone."
    pub on_phone: String,
    /// The most a reply may hold, in characters (a text's own limit for texts).
    pub limit: usize,
    /// This computer's replies to it, oldest first.
    pub mine: Vec<Mine>,
}

/// A reply written on this computer, under the messages it answers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Mine {
    pub key: String,
    /// "14:03": when it was written.
    pub time: String,
    pub text: String,
    /// Its state, in words (`state_of`; a text's own words for texts).
    pub said: String,
    pub waiting: bool,
    pub again: bool,
    /// Went as a text of Sioul's own: Take back is not offered, Send again is the texts'.
    pub texts: bool,
}

/// What a computer knows for a Porch's line's answer.
pub struct Offer<'a> {
    /// What each phone said last of each notification (`read_live`), by phone and notice.
    pub live: &'a BTreeMap<(String, String), Live>,
    /// This computer's own requests (`read_asks` of its file).
    pub asks: &'a [Ask],
    /// Every phone's outcomes.
    pub outcomes: &'a [Outcome],
    pub tr: &'a crate::i18n::Translator,
    /// The keys this computer took back (its "cancel" lines, `read_asks`).
    pub taken_back: &'a BTreeSet<String>,
    /// Every phone's lines (`phonemsgs::read_logs`): a reply through a
    /// conversation's newer notification names that one's newest line.
    pub lines: &'a [phonemsgs::Line],
    /// Whether a phone sends texts for this computer (its texts read here),
    /// by its name: a text that came to it is answered as one of Sioul's own
    /// texts, from that phone, never another's.
    pub texts_from: &'a dyn Fn(&str) -> bool,
    /// When a phone last shared (ms), by its name.
    pub shared: &'a dyn Fn(&str) -> Option<i64>,
    /// A moment (ms) as this computer says the time.
    pub time_of: &'a dyn Fn(i64) -> String,
    /// The texts this computer wrote from a phone (its name) to a number since a moment (ms), as `Mine`.
    pub sent_texts: &'a dyn Fn(&str, &str, i64) -> Vec<Mine>,
}

/// A Porch's line's answer (`line`, of the messages `group`, oldest first):
/// none for a code's line, nor for lines from a phone that names no
/// notification (an older Sioul's: Text back as before).
pub fn answer_of(line: &phonemsgs::PorchLine, group: &[&phonemsgs::Line], o: &Offer) -> Option<Answer> {
    let newest = group.last()?;
    if line.code || newest.code || newest.notice.is_empty() {
        return None;
    }
    let tr = o.tr;
    let label = if newest.label.is_empty() { newest.app.clone() } else { newest.label.clone() };
    let mut args = crate::i18n::args();
    args.set("app", label.clone());
    args.set("who", line.who.clone());
    let placeholder = tr.text(if newest.group { "replies-placeholder-group" } else { "replies-placeholder" }, Some(&args));
    let base = Answer {
        phone: newest.device.clone(),
        app: newest.app.clone(),
        label: label.clone(),
        line: newest.id.clone(),
        placeholder,
        read_item: tr.text("replies-read-item", Some(&args)),
        read_warning: tr.text("replies-read-warning", Some(&args)),
        on_phone: tr.text("replies-on-phone", None),
        ..Answer::default()
    };
    // A text from a whole number, from a phone that sends texts for this computer: one of Sioul's own, from that phone.
    if line.texted && (o.texts_from)(&newest.device) {
        let since = group.first().map_or(newest.at, |l| l.at);
        return Some(Answer { notice: newest.notice.clone(), texts: true, offered: true, limit: crate::texts::WRITE_MAX, mine: (o.sent_texts)(&newest.device, &line.dial, since), ..base });
    }
    // The notification as the phone said it last; posted anew under another
    // name (its app's update), the conversation's newest that offers a Reply,
    // answered by its own newest line (the phone checks that the line is the
    // notification's: `verdict`); none of its lines here, not offered.
    let said = o.live.get(&(newest.device.clone(), newest.notice.clone()));
    let current = said.filter(|l| l.gone == 0).cloned().or_else(|| {
        (!newest.talk.is_empty()).then(|| o.live.values().filter(|l| l.device == newest.device && l.talk == newest.talk && l.gone == 0 && l.reply).max_by_key(|l| l.at).cloned()).flatten()
    });
    let mut answered = newest.id.clone();
    let (notice, offered, read) = match current {
        Some(l) if l.notice == newest.notice => (l.notice.clone(), l.reply, l.read),
        // Its newest line of this very conversation, never a code's: an app that reuses one notification for
        // another conversation never makes a reply to her go to him, even when the phone's live line was not written.
        // The same shape too (a group or not, its title): an untitled group keyed by its last writer is never answered
        // through her one-to-one's notification (review of 18 October 2026, L-1).
        Some(l) => match o.lines.iter().filter(|x| x.device == newest.device && x.notice == l.notice && x.talk == newest.talk && x.group == newest.group && x.title == newest.title && !x.code).max_by_key(|x| x.at) {
            Some(theirs) => {
                answered = theirs.id.clone();
                (l.notice.clone(), l.reply, l.read)
            }
            None => (newest.notice.clone(), false, false),
        },
        None => (newest.notice.clone(), false, false),
    };
    // A service's text (a short number, a sender named in letters): never from here, as the phone
    // refuses it (`verdict`: it may cost money); "Reply on your phone" in its place.
    let service = newest.kind == "text" && !newest.group && (newest.who == "automaton" || !crate::phones::is_whole(&newest.key));
    let (offered, read) = if service { (false, false) } else { (offered, read) };
    let note = if MARKS_READ.contains(&newest.app.as_str()) { tr.text("replies-note-marks-read", Some(&args)) } else { String::new() };
    let ids: BTreeSet<&str> = group.iter().map(|l| l.id.as_str()).collect();
    let mut mine: Vec<&Ask> = o.asks.iter().filter(|a| a.verb == "reply" && a.phone == newest.device && (ids.contains(a.line.as_str()) || a.notice == notice)).collect();
    mine.sort_by_key(|a| a.written);
    let mine = mine
        .into_iter()
        .map(|a| {
            let theirs: Vec<&Outcome> = o.outcomes.iter().filter(|x| x.key == a.key && x.device == a.phone).collect();
            let state = state_of(a, &theirs, &label, tr, (o.shared)(&a.phone), o.taken_back.contains(&a.key), o.time_of);
            Mine { key: a.key.clone(), time: (o.time_of)(a.written), text: a.words.clone(), said: state.said, waiting: state.waiting, again: state.again, texts: false }
        })
        .collect();
    Some(Answer { notice, offered, read, note, limit: WORDS_MAX, mine, line: answered, ..base })
}

/// Whether a Mark as read this computer asked for a notification was
/// handed to its app: the line is then Seen too (decision 13).
pub fn read_handed(asks: &[Ask], outcomes: &[Outcome], phone: &str, notice: &str) -> bool {
    asks.iter().filter(|a| a.verb == "read" && a.phone == phone && a.notice == notice).any(|a| outcomes.iter().any(|o| o.key == a.key && o.device == phone && o.state == "handed"))
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
        Due { key: key.into(), verb: "reply".into(), notice: notice(&posted.package, &posted.key), app: posted.package.clone(), talk, from: "+33199001234".into(), name: "Alice".into(), line_at, words: "J'arrive.".into(), at, ..Due::default() }
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
        assert_eq!(read_due(&dir, &Stand).iter().map(|d| (d.key.as_str(), d.words.as_str())).collect::<Vec<_>>(), [("k1", ""), ("k2", "J'arrive.")], "taken: its words no longer kept");
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
        assert_eq!(check(&due, &Posted { shortcut: "bob-thread".into(), ..posted.clone() }, &extra, &choices, &words, fr()), Checked::Other, "another conversation under that notification");
        // A line without a conversation's key: her number, else her name; nothing that tells: never a guess.
        let bare = Due { talk: String::new(), ..due.clone() };
        assert_eq!(check(&bare, &posted, &extra, &choices, &words, fr()), Checked::Go { crossed: false });
        assert_eq!(check(&Due { from: "+33199009999".into(), ..bare.clone() }, &posted, &extra, &choices, &words, fr()), Checked::Other);
        assert_eq!(check(&Due { from: String::new(), ..bare.clone() }, &posted, &extra, &choices, &words, fr()), Checked::Go { crossed: false }, "her name");
        assert_eq!(check(&Due { from: String::new(), name: "Bob".into(), ..bare.clone() }, &posted, &extra, &choices, &words, fr()), Checked::Other);
        assert_eq!(check(&Due { from: String::new(), name: String::new(), ..bare.clone() }, &posted, &extra, &choices, &words, fr()), Checked::Other, "nothing names the conversation");
        // The crossing by the messages' own times only: a notification without them says none.
        let untimed = Extra { messages: Vec::new(), ..newer_extra.clone() };
        assert_eq!(check(&due, &newer, &untimed, &choices, &words, fr()), Checked::Go { crossed: false }, "its own time moves with a silent repost");
    }

    /// An app that posts several conversations under one notification
    /// without shortcuts or titles: her conversation's key is her own, and
    /// so is an untitled group's where she wrote last. A reply to her never
    /// goes into the group, nor a group's reply to another group or to her.
    #[test]
    fn a_reply_never_goes_into_a_conversation_of_another_shape() {
        let words = words();
        let choices = appnotes::Choices::default();
        let alice = Person { name: "Alice".into(), key: "alice".into(), ..Person::default() };
        let bob = Person { name: "Bob".into(), key: "bob".into(), ..Person::default() };
        let base = Posted { key: "0|com.example.chat|7|null|10123".into(), package: "com.example.chat".into(), app: "Chat".into(), category: "msg".into(), template: "android.app.Notification$MessagingStyle".into(), is_conversation: true, ..Posted::default() };
        let one = Posted { title: "Alice".into(), messages: vec![Message { sender: Some(alice.clone()), text: "Tu passes ?".into() }], people: vec![alice.clone()], ..base.clone() };
        let group = Posted { messages: vec![Message { sender: Some(bob.clone()), text: "Qui vient ?".into() }, Message { sender: Some(alice.clone()), text: "Moi.".into() }], people: vec![bob.clone(), alice.clone()], ..base.clone() };
        let talk = |p: &Posted| match appnotes::classify(p, &choices, &words) {
            Kind::People(t) => t.conversation.unwrap(),
            other => panic!("{other:?}"),
        };
        let (mine, theirs) = (talk(&one), talk(&group));
        assert!(!mine.group && theirs.group);
        let due = Due { key: "k1".into(), verb: "reply".into(), notice: notice(&base.package, &base.key), app: base.package.clone(), talk: mine.key.clone(), name: "Alice".into(), words: "J'arrive.".into(), at: NOW, ..Due::default() };
        assert_eq!(check(&due, &one, &Extra::default(), &choices, &words, fr()), Checked::Go { crossed: false });
        assert_eq!(check(&due, &group, &Extra::default(), &choices, &words, fr()), Checked::Other, "her conversation, never the group");
        // An untitled group's key came from its last writer: it names her, and another untitled group where she
        // wrote last has it too: its reply never goes, through the notification.
        assert!(theirs.by_writer && mine.by_writer);
        let to_group = Due { talk: theirs.key.clone(), group: true, title: phonemsgs::title_of(&theirs), ..due.clone() };
        assert_eq!(check(&to_group, &group, &Extra::default(), &choices, &words, fr()), Checked::Other, "a group keyed by its last writer");
        assert_eq!(check(&to_group, &one, &Extra::default(), &choices, &words, fr()), Checked::Other, "the group's reply never goes to her alone");
        // A titled group: its key from its title; its reply goes, to it only.
        let titled = Posted { conversation: "École".into(), ..group.clone() };
        let school = talk(&titled);
        assert!(!school.by_writer);
        let to_school = Due { talk: school.key.clone(), group: true, title: phonemsgs::title_of(&school), ..due.clone() };
        assert_eq!(check(&to_school, &titled, &Extra::default(), &choices, &words, fr()), Checked::Go { crossed: false });
        assert_eq!(check(&Due { title: "Club".into(), ..to_school.clone() }, &titled, &Extra::default(), &choices, &words, fr()), Checked::Other, "another group's title");
        // From the line, as the phone wrote it.
        let line = phonemsgs::Line { group: true, title: "École".into(), talk: "aaaa".into(), ..alice_line() };
        let a = ask("k1", "J'arrive.", NOW);
        let d = due_of(&a, &line, NOW);
        assert_eq!((d.group, d.title.as_str()), (true, "École"));
    }

    /// Your reply shown in its notification as yours: Signal's reposted
    /// conversation with your words last, or the Reply's own history; never
    /// someone else's words, never another reply's.
    #[test]
    fn a_reply_is_shown_when_its_notification_shows_your_words() {
        let hash = words_hash("J'arrive dans dix minutes.");
        let taken = NOW + 30_000;
        let (before, extra) = signal(NOW, None);
        assert!(!shows(&before, &extra, &hash, taken, false));
        // Your message, timed after the reply was taken (signal puts it a minute after hers).
        let (after, extra) = signal(NOW, Some("J'arrive dans dix minutes. "));
        assert!(shows(&after, &extra, &hash, taken, false), "your message, the trailing space aside");
        assert!(!shows(&after, &extra, &words_hash("Une autre réponse."), taken, false));
        // The same words of yours from before the reply: never this one shown.
        assert!(!shows(&after, &extra, &hash, NOW + 5 * MINUTE, true), "an earlier message of yours, timed before");
        assert!(shown_already(&after, &extra, &hash));
        // No times: the words, or the Reply's history, only when they were not there before.
        let (plain, extra) = signal(NOW, None);
        let history = Extra { history: vec!["J'arrive dans dix minutes.".into()], ..extra.clone() };
        assert!(shows(&plain, &history, &hash, taken, false), "the Reply's history");
        assert!(!shows(&plain, &history, &hash, taken, true), "there already before the reply");
        let (untimed, timed) = signal(NOW, Some("J'arrive dans dix minutes."));
        let untimed_extra = Extra { messages: Vec::new(), ..timed };
        assert!(shows(&untimed, &untimed_extra, &hash, taken, false));
        assert!(!shows(&untimed, &untimed_extra, &hash, taken, true));
        assert!(!shows(&plain, &extra, &words_hash("Je suis au café."), taken, false), "her words are not yours");
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
        assert_eq!(asks.iter().map(|a| (a.key.as_str(), a.device.as_str(), a.writer.as_str())).collect::<Vec<_>>(), [("k1", "desk", "desk"), ("k2", "laptop", "laptop")]);
        assert!(cancelled.contains(&("desk".to_string(), "k1".to_string())));
        assert_eq!(asks[0].words, "J'arrive.");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&other);
    }

    /// Each request names its writer inside its seal: a line moved from one
    /// computer's file to another's (the server, a stray file), or written
    /// with no writer, is passed over, a "cancel" too; the request in its own
    /// file is the one read, with its own computer.
    #[test]
    fn a_request_moved_to_another_computer_s_file_is_passed_over() {
        let root = scratch("asks-moved");
        let first = ask("k1", "J'arrive.", NOW);
        write_ask(&root, "laptop", &first, &Stand).unwrap();
        write_ask(&root, "laptop", &Ask { verb: "cancel".into(), words: String::new(), ..first.clone() }, &Stand).unwrap();
        let moved = std::fs::read_to_string(calls::own_file(&root, ASK, "laptop")).unwrap();
        // The same sealed lines put in the desk's file, and a line with no writer.
        std::fs::write(calls::own_file(&root, ASK, "desk"), &moved).unwrap();
        let bare = format!("{}\n", Stand.seal(&serde_json::to_string(&Ask { key: "k2".into(), device: String::new(), ..ask("k2", "Oui.", NOW) }).unwrap()));
        std::fs::write(calls::own_file(&root, ASK, "phone"), bare).unwrap();
        let (asks, cancelled) = read_asks(std::slice::from_ref(&root), &Stand);
        assert_eq!(asks.iter().map(|a| (a.key.as_str(), a.device.as_str())).collect::<Vec<_>>(), [("k1", "laptop")], "the desk's moved copy and the bare line passed over");
        assert_eq!(cancelled.into_iter().collect::<Vec<_>>(), [("laptop".to_string(), "k1".to_string())]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Through a conversation's newer notification, the Porch names that
    /// notification's newest line of this very conversation, never a code's:
    /// an app that reuses one notification for another conversation, its new
    /// live line not written (or older than the log), never makes Alice's
    /// reply go to Bob.
    #[test]
    fn alice_s_reply_never_goes_to_bob() {
        let tr = crate::i18n::Translator::new("en");
        let line = phonemsgs::Line { device: "phone".into(), ..alice_line() };
        // The newer notification: its live line still says Alice's conversation; its newest line here is Bob's.
        let bob = phonemsgs::Line { at: NOW - MINUTE, id: format!("{}-bb33", NOW - MINUTE), notice: "1111222233334444".into(), talk: "0000111122223333".into(), name: "Bob".into(), key: "+33261910000".into(), text: "Salut !".into(), ..line.clone() };
        let code = phonemsgs::Line { at: NOW - MINUTE / 2, id: format!("{}-cc44", NOW - MINUTE / 2), notice: "1111222233334444".into(), code: true, text: String::new(), ..line.clone() };
        let porch = phonemsgs::PorchLine { ids: vec![line.id.clone()], head: String::new(), said: Vec::new(), note: String::new(), number: String::new(), dial: String::new(), known: true, texted: false, code: false, who: "Alice".into(), answer: None };
        let live = |notice: &str, reply: bool, gone: i64| (("phone".to_string(), notice.to_string()), Live { notice: notice.into(), app: line.app.clone(), talk: line.talk.clone(), reply, read: reply, gone, at: NOW, device: "phone".into() });
        let lives: BTreeMap<_, _> = [live(&line.notice, false, NOW), live("1111222233334444", true, 0)].into();
        let (shared, time, none, no_one, nobody) = (|_: &str| Some(NOW), |_: i64| "14:03".to_string(), |_: &str, _: &str, _: i64| Vec::new(), |_: &str| false, BTreeSet::new());
        for lines in [vec![line.clone(), bob.clone()], vec![line.clone(), code.clone()], vec![line.clone(), bob.clone(), code.clone()]] {
            let o = Offer { live: &lives, asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &lines, texts_from: &no_one, shared: &shared, time_of: &time, sent_texts: &none };
            let answer = answer_of(&porch, &[&line], &o).unwrap();
            assert!(!answer.offered, "no line of Alice's conversation in that notification: Reply on your phone");
            assert!(answer.line != bob.id && answer.line != code.id, "never Bob's line, never a code's: {}", answer.line);
        }
        // Alice's own newer line in it: that one.
        let hers = phonemsgs::Line { at: NOW - MINUTE / 4, id: format!("{}-dd55", NOW - MINUTE / 4), notice: "1111222233334444".into(), text: "Tu es où ?".into(), ..line.clone() };
        let lines = vec![line.clone(), bob.clone(), hers.clone(), code];
        let o = Offer { live: &lives, asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &lines, texts_from: &no_one, shared: &shared, time_of: &time, sent_texts: &none };
        let answer = answer_of(&porch, &[&line], &o).unwrap();
        assert_eq!((answer.offered, answer.line.as_str()), (true, hers.id.as_str()));
        // An untitled group keyed by its last writer (her key), its notification gone, and her one-to-one's live
        // with Reply: the group's words never go into her one-to-one (L-1).
        let group = phonemsgs::Line { group: true, title: "Alice".into(), ..line.clone() };
        let one = phonemsgs::Line { at: NOW - MINUTE / 4, id: format!("{}-ee66", NOW - MINUTE / 4), notice: "1111222233334444".into(), text: "Seule.".into(), ..line.clone() };
        let lines = vec![group.clone(), one];
        let gp = phonemsgs::PorchLine { who: "Alice".into(), ..porch.clone() };
        let o = Offer { live: &lives, asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &lines, texts_from: &no_one, shared: &shared, time_of: &time, sent_texts: &none };
        let answer = answer_of(&gp, &[&group], &o).unwrap();
        assert!(!answer.offered, "a group's reply never redirected to her one-to-one");
    }

    /// A file of requests is read once: past every line done with (decided,
    /// another phone's, one that does not open) a step reads it from where it
    /// was, never opening those again; a request not decided yet, or its
    /// cancel, stops the mark; a file rewritten (trimmed) is read whole again.
    #[test]
    fn a_file_of_requests_is_read_from_where_the_phone_was_done_with_it() {
        let root = scratch("read-marks");
        let mine = |key: &str| ask(key, "Oui.", NOW);
        write_ask(&root, "desk", &mine("k1"), &Stand).unwrap();
        write_ask(&root, "desk", &Ask { phone: "other-phone".into(), ..mine("k2") }, &Stand).unwrap();
        write_ask(&root, "desk", &mine("k3"), &Stand).unwrap();
        let roots = vec![root.clone()];
        let keys = |asked: &(Vec<Ask>, BTreeSet<(String, String)>)| asked.0.iter().map(|a| a.key.clone()).collect::<Vec<_>>();
        let (asked, reads) = read_asks_after(&roots, &Stand, &BTreeMap::new());
        assert_eq!(keys(&asked), ["k1", "k2", "k3"]);
        // k1 decided, k2 another phone's, k3 not decided yet: the mark stops before k3.
        let decided = |set: &'static [&'static str]| move |key: &str, phone: &str| phone != "phone" || set.contains(&key);
        let marks = moved_marks(&reads, &decided(&["k1"]), &BTreeMap::new());
        let (asked, reads) = read_asks_after(&roots, &Stand, &marks);
        assert_eq!(keys(&asked), ["k3"], "k1 and k2 never opened again");
        // A cancel for k3 written since: it stops the mark too, until k3 is decided.
        write_ask(&root, "desk", &Ask { verb: "cancel".into(), words: String::new(), ..mine("k3") }, &Stand).unwrap();
        let (asked, reads) = read_asks_after(&roots, &Stand, &moved_marks(&reads, &decided(&["k1"]), &marks));
        assert_eq!((keys(&asked), asked.1.len()), (vec!["k3".to_string()], 1));
        let marks = moved_marks(&reads, &decided(&["k1", "k3"]), &marks);
        let (asked, reads) = read_asks_after(&roots, &Stand, &marks);
        assert!(asked.0.is_empty() && asked.1.is_empty() && reads[0].lines.is_empty(), "done with the whole file");
        // A line that does not open (another key) is passed, as before.
        let own = calls::own_file(&root, ASK, "desk");
        std::fs::write(&own, format!("{}not a sealed line\n", std::fs::read_to_string(&own).unwrap())).unwrap();
        let marks = moved_marks(&reads, &decided(&["k1", "k3"]), &marks);
        let (_, reads) = read_asks_after(&roots, &Stand, &marks);
        let marks = moved_marks(&reads, &decided(&["k1", "k3"]), &marks);
        assert_eq!(marks.values().next().unwrap().offset, std::fs::metadata(&own).unwrap().len());
        // Rewritten (its first line trimmed): read whole again.
        let text = std::fs::read_to_string(&own).unwrap();
        std::fs::write(&own, text.split_inclusive('\n').skip(1).collect::<String>()).unwrap();
        let (asked, _) = read_asks_after(&roots, &Stand, &marks);
        assert_eq!(keys(&asked), ["k2", "k3"], "the mark's line is no longer there: the whole file");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Each computer's requests in turn: a runaway computer's many never keep another's from its turn.
    #[test]
    fn each_computer_s_requests_come_in_turn() {
        let of = |key: &str, device: &str| Ask { device: device.into(), ..ask(key, "Oui.", NOW) };
        let asks = vec![of("a1", "aaa"), of("a2", "aaa"), of("a3", "aaa"), of("d1", "desk"), Ask { phone: "other".into(), ..of("o1", "desk") }];
        assert_eq!(in_turn(&asks, "phone").iter().map(|a| a.key.as_str()).collect::<Vec<_>>(), ["a1", "d1", "a2", "a3"]);
        assert!(in_turn(&asks, "nobody").is_empty());
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
        let nothing = BTreeSet::new();
        let yes = |_: &str| true;
        let judge = |ledger: &crate::texts::Ledger, cancelled: &BTreeSet<(String, String)>, counted: &dyn Fn(&str) -> bool, answers: &dyn Fn(&str) -> bool, lines: &[phonemsgs::Line], a: &Ask| {
            verdict(a, &Judge { here: "phone", now_ms: NOW, ledger, lines, counted, cancelled, answers, unsent: &nothing, went: &nothing })
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
        crate::texts::record(&dir, &claim_of(&good, "", NOW - MINUTE)).unwrap();
        let ledger = crate::texts::Ledger::open(&dir, NOW).unwrap();
        let v = |a: &Ask| judge(&ledger, &none, &yes, &yes, &lines, a);
        assert_eq!(v(&good), Verdict::Decided);
        assert!(!std::fs::read_to_string(dir.join(crate::texts::LEDGER)).unwrap().contains("arrive"), "the ledger keeps a hash, never the words");
        // The same words to the same notification within ten minutes: refused, unless sent again on purpose.
        assert_eq!(v(&ask("k2", "J'arrive. ", NOW)), Verdict::Refused("same-words-pending"), "on its way: no word of the phone yet");
        assert_eq!(v(&Ask { again: "k1".into(), ..ask("k2", "J'arrive.", NOW) }), Verdict::Send);
        assert_eq!(v(&ask("k2", "Dans cinq minutes.", NOW)), Verdict::Send);
        // Sixty replies in the last hour, from all computers.
        for i in 0..PER_HOUR {
            crate::texts::record(&dir, &claim_of(&ask(&format!("n{i}"), &format!("Réponse {i}"), NOW), "", NOW - 30 * MINUTE)).unwrap();
        }
        let ledger = crate::texts::Ledger::open(&dir, NOW).unwrap();
        assert_eq!(judge(&ledger, &none, &yes, &yes, &lines, &ask("k3", "Encore une.", NOW)), Verdict::Refused("too-many"));
        assert_eq!(judge(&ledger, &none, &yes, &yes, &lines, &Ask { verb: "read".into(), words: String::new(), ..ask("k4", "", NOW) }), Verdict::Send, "a Mark as read counts for none");
        // Sixty Marks as read in the last hour: the next one refused too, in its own words.
        let read = |key: &str| Ask { verb: "read".into(), words: String::new(), ..ask(key, "", NOW) };
        let claims: Vec<crate::texts::Claim> = (0..PER_HOUR).map(|i| claim_of(&read(&format!("r{i}")), "", NOW - 20 * MINUTE)).collect();
        crate::texts::record_all(&dir, &claims).unwrap();
        let ledger = crate::texts::Ledger::open(&dir, NOW).unwrap();
        assert_eq!(judge(&ledger, &none, &yes, &yes, &lines, &read("r99")), Verdict::Refused("too-many-read"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The same words go again when the first never went: its phone said it
    /// was gone (the notification had gone, then hers came back under the
    /// same name), late, closed, failed…; one that went, may still go, or
    /// is in doubt holds them back ten minutes.
    #[test]
    fn the_same_words_go_again_when_the_first_never_went() {
        let dir = scratch("same-words");
        crate::texts::Ledger::open(&dir, NOW - 60 * MINUTE).unwrap();
        let lines = vec![alice_line()];
        let (none, yes) = (BTreeSet::new(), |_: &str| true);
        let first = ask("k1", "J'arrive.", NOW - 3 * MINUTE);
        crate::texts::record(&dir, &claim_of(&first, &alice_line().talk, NOW - 3 * MINUTE)).unwrap();
        let again = ask("k2", "J'arrive.", NOW);
        let v = |listener: &[(&str, &str)]| {
            let folder = scratch("same-words-done");
            for (key, state) in listener {
                say(&folder, &Done::of(key, state, NOW - 2 * MINUTE)).unwrap();
            }
            let (unsent, went) = (unsent(&folder), went_keys(&folder));
            let ledger = crate::texts::Ledger::open(&dir, NOW).unwrap();
            let said = verdict(&again, &Judge { here: "phone", now_ms: NOW, ledger: &ledger, lines: &lines, counted: &yes, cancelled: &none, answers: &yes, unsent: &unsent, went: &went });
            let _ = std::fs::remove_dir_all(&folder);
            said
        };
        // Said as it is known: on its way while the phone has not said it went, went once it did.
        assert_eq!(v(&[]), Verdict::Refused("same-words-pending"), "claimed, not said yet: it may go");
        for (word, said) in [("taken", "same-words-pending"), ("doubt", "same-words-pending"), ("handed", "same-words"), ("shown", "same-words")] {
            assert_eq!(v(&[("k1", "taken"), ("k1", word)]), Verdict::Refused(said), "{word}");
        }
        for not in NOT_SENT {
            assert_eq!(v(&[("k1", "taken"), ("k1", not)]), Verdict::Send, "{not}: it never went");
        }
        // Through the conversation's newer notification (its app posted it anew): the same words found there
        // too, a fresh reply or Send again of the refused one, while the first may still go (L-2).
        let newer = phonemsgs::Line { id: format!("{}-bb22", NOW - MINUTE), at: NOW - MINUTE, notice: "1111222233334444".into(), ..alice_line() };
        let lines = vec![alice_line(), newer.clone()];
        let ledger = crate::texts::Ledger::open(&dir, NOW).unwrap();
        let through_newer = |a: &Ask, unsent: &BTreeSet<String>| verdict(a, &Judge { here: "phone", now_ms: NOW, ledger: &ledger, lines: &lines, counted: &yes, cancelled: &none, answers: &yes, unsent, went: &none_keys() });
        let fresh = Ask { notice: newer.notice.clone(), line: newer.id.clone(), ..ask("k3", "J'arrive.", NOW) };
        assert_eq!(through_newer(&fresh, &BTreeSet::new()), Verdict::Refused("same-words-pending"), "a fresh reply through the newer notification");
        let again = Ask { again: "k2".into(), ..fresh.clone() };
        assert_eq!(through_newer(&again, &BTreeSet::new()), Verdict::Refused("same-words-pending"), "Send again of the refused one through the newer notification");
        let first_went: BTreeSet<String> = ["k1".to_string()].into();
        let through_newer_after = |a: &Ask| verdict(a, &Judge { here: "phone", now_ms: NOW, ledger: &ledger, lines: &lines, counted: &yes, cancelled: &none, answers: &yes, unsent: &BTreeSet::new(), went: &first_went });
        assert_eq!(through_newer_after(&again), Verdict::Refused("same-words"), "the first went: never twice");
        let gone: BTreeSet<String> = ["k1".to_string()].into();
        assert_eq!(through_newer(&again, &gone), Verdict::Send, "the first never went: it goes, once");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn none_keys() -> BTreeSet<String> {
        BTreeSet::new()
    }

    /// The ledger's mark is made once, at the first step or the first line
    /// relayed, in a folder made yours alone; never moved after: a request
    /// written after it goes, one written before it (a ledger lost and made
    /// again) is refused.
    #[test]
    fn the_mark_is_made_once_before_the_first_request() {
        let dir = scratch("mark").join("replies");
        mark_ledger(&dir, NOW - 10 * MINUTE).unwrap();
        mark_ledger(&dir, NOW).unwrap();
        let ledger = open_ledger(&dir, NOW).unwrap();
        assert_eq!(ledger.mark.since, NOW - 10 * MINUTE, "never moved");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700, "yours alone");
        }
        let lines = vec![alice_line()];
        let (none, nothing, yes) = (BTreeSet::new(), BTreeSet::new(), |_: &str| true);
        let v = |a: &Ask| verdict(a, &Judge { here: "phone", now_ms: NOW, ledger: &ledger, lines: &lines, counted: &yes, cancelled: &none, answers: &yes, unsent: &nothing, went: &nothing });
        assert_eq!(v(&ask("k1", "J'arrive.", NOW - 90_000)), Verdict::Send, "written after the mark");
        assert_eq!(v(&ask("k2", "J'arrive.", NOW - 11 * MINUTE)), Verdict::Refused("before-mark"));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
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
            let waiting = state_of(&a, &[], "Signal", &tr, Some(NOW), false, &time);
            assert!(waiting.waiting && waiting.said.contains("14:03"), "{language}: {waiting:?}");
            assert!(state_of(&a, &[], "Signal", &tr, None, false, &time).waiting);
            let states = ["taken", "handed", "shown", "gone", "other", "no-reply", "no-read", "closed", "locked", "activity", "late", "failed", "expired", "clocks", "doubt", "taken-back"];
            for state in states {
                let o = at(state, "");
                let said = state_of(&a, &[&o], "Signal", &tr, Some(NOW), false, &time);
                assert!(!said.waiting && !said.said.starts_with("replies-") && !said.said.is_empty(), "{language} {state}: {said:?}");
                assert!(!said.said.to_lowercase().contains("sent at") && !said.said.contains("Envoyé"), "never a plain Sent: {language} {state}");
            }
            for reason in ["same-words", "same-words-pending", "too-many", "too-many-read", "too-long", "no-words", "line", "code", "switched-off", "short-number", "not-counted", "before-mark"] {
                let o = at("refused", reason);
                let said = state_of(&a, &[&o], "Signal", &tr, Some(NOW), false, &time);
                assert!(!said.said.starts_with("replies-"), "{language} {reason}");
            }
            let (handed, shown) = (at("handed", ""), Outcome { at: NOW + 1, ..at("shown", "") });
            let said = |a: &Ask, o: &[&Outcome], cancel: bool| state_of(a, o, "Signal", &tr, None, cancel, &time);
            assert_eq!(said(&a, &[&handed, &shown], false).state, "shown", "the furthest it went");
            let gone = said(&a, &[&at("gone", "")], false);
            assert!(gone.on_phone && !gone.again);
            assert!(said(&a, &[&at("other", "")], false).on_phone);
            assert!(said(&a, &[&at("expired", "")], false).again);
            let crossed = Outcome { crossed: true, ..at("handed", "") };
            assert_ne!(said(&a, &[&crossed], false).said, said(&a, &[&handed], false).said, "crossed said");
            // Held, with nothing newer of theirs: plainly handed; "waits on your phone" only with a newer message.
            let held = Outcome { held: true, ..at("handed", "") };
            assert_eq!(said(&a, &[&held], false).said, said(&a, &[&handed], false).said, "{language}: held, nothing newer");
            let both = Outcome { held: true, crossed: true, ..at("handed", "") };
            assert_ne!(said(&a, &[&both], false).said, said(&a, &[&crossed], false).said);
            // A Mark as read handed: handed, never "marked".
            let read = Ask { verb: "read".into(), words: String::new(), ..a.clone() };
            let read_said = said(&read, &[&handed], false).said;
            assert_ne!(read_said, said(&a, &[&handed], false).said, "a Mark as read's own words");
            if language == "en" {
                assert_eq!(read_said, "Mark as read handed to Signal at 14:03.");
                assert_eq!(said(&a, &[&both], false).said, "Handed to Signal at 14:03. A newer message of theirs waits on your phone.");
            }
            // Take back: said at once, no Take back again; too late after the phone took it; taken back.
            let asked = said(&a, &[], true);
            assert!(!asked.waiting && !asked.said.is_empty() && !asked.said.starts_with("replies-"), "{language}: {asked:?}");
            for went in ["taken", "handed", "shown", "doubt"] {
                let late = said(&a, &[&at(went, "")], true);
                assert!(late.said.len() > said(&a, &[&at(went, "")], false).said.len(), "{language} {went}: too late said");
            }
            assert_eq!(said(&a, &[&at("taken-back", "")], true).said, said(&a, &[&at("taken-back", "")], false).said);
            if language == "en" {
                assert_eq!(asked.said, "Taking it back: your phone will not send it, unless it has taken it already.");
                assert_eq!(said(&a, &[&handed], true).said, "Handed to Signal at 14:03. Take back came too late: your phone had taken it already.");
            }
        }
    }

    /// A conversation answered comes through at once for twenty minutes, then no more.
    #[test]
    fn a_conversation_answered_is_let_through_for_twenty_minutes() {
        let dir = scratch("answered");
        assert!(!answered_lately(&dir, "fedcba9876543210", NOW));
        note_answered(&dir, "fedcba9876543210", NOW).unwrap();
        note_answered(&dir, "", NOW).unwrap();
        assert!(answered_lately(&dir, "fedcba9876543210", NOW + 19 * MINUTE));
        assert!(!answered_lately(&dir, "fedcba9876543210", NOW + 21 * MINUTE));
        assert!(!answered_lately(&dir, "0000000000000000", NOW), "another conversation");
        assert!(!answered_lately(&dir, "", NOW));
        // A number texted from a computer: by a hash, never the number.
        note_answered(&dir, &answered_number("+33199001234"), NOW).unwrap();
        assert!(answered_lately(&dir, &answered_number("+33199001234"), NOW + MINUTE));
        assert!(!answered_lately(&dir, &answered_number("+33261910000"), NOW + MINUTE));
        assert!(!std::fs::read_to_string(dir.join(ANSWERED)).unwrap().contains("199001234"));
        assert_eq!(answered_number(""), "");
        // What would wait comes at once; what is held for good stays held; what comes stays as it is.
        let held = appnotes::Decision::held(NOW / 1000 + 3600, appnotes::Why::Waiting);
        assert_eq!(held.answered(), appnotes::Decision::through(appnotes::Why::Answered));
        assert_eq!(appnotes::Decision::held(NOW / 1000 + 3600, appnotes::Why::Gathered).answered().why, appnotes::Why::Answered);
        let never = appnotes::Decision::held(NOW / 1000 + 3600, appnotes::Why::Never);
        assert_eq!(never.answered(), never, "the blocked, a conversation set to never");
        let through = appnotes::Decision::through(appnotes::Why::Allowed);
        assert_eq!(through.answered(), through);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// On a computer's Porch: a conversation this computer answered, its
    /// phone having handed or shown the reply within twenty minutes, and a
    /// number it texted then; never another computer's reply, one refused or
    /// still waiting, nor another phone's line of the same conversation.
    #[test]
    fn what_this_computer_answered_lately_shows_at_once() {
        let line = phonemsgs::Line { device: "phone".into(), ..alice_line() };
        let mine = ask("k1", "J'arrive.", NOW);
        let outcome = |key: &str, state: &str, at: i64| Outcome { key: key.into(), state: state.into(), at, device: "phone".into(), ..Outcome::default() };
        let all = [line.clone()];
        let answered = Answered::of(std::slice::from_ref(&mine), &[outcome("k1", "handed", NOW)], &all, NOW + 5 * MINUTE);
        assert!(answered.holds(&line));
        assert!(answered.holds(&phonemsgs::Line { id: "2-bb".into(), text: "Tu es où ?".into(), ..line.clone() }), "their answer, in the same conversation");
        assert!(!answered.holds(&phonemsgs::Line { device: "other".into(), ..line.clone() }), "another phone's");
        assert!(!answered.holds(&phonemsgs::Line { talk: "0000000000000000".into(), ..line.clone() }), "another conversation");
        assert!(Answered::of(std::slice::from_ref(&mine), &[outcome("k1", "shown", NOW)], &all, NOW + 19 * MINUTE).holds(&line));
        assert!(Answered::of(std::slice::from_ref(&mine), &[outcome("k1", "handed", NOW + MINUTE)], &all, NOW).holds(&line), "the phone's clock a minute ahead");
        assert!(!Answered::of(std::slice::from_ref(&mine), &[outcome("k1", "handed", NOW + 3 * MINUTE)], &all, NOW).holds(&line), "three minutes ahead: not that reply");
        assert!(!Answered::of(std::slice::from_ref(&mine), &[outcome("k1", "handed", NOW)], &all, NOW + 21 * MINUTE).holds(&line), "past twenty minutes");
        assert!(!Answered::of(std::slice::from_ref(&mine), &[outcome("k1", "refused", NOW)], &all, NOW).holds(&line));
        assert!(!Answered::of(std::slice::from_ref(&mine), &[], &all, NOW).holds(&line), "still waiting");
        assert!(!Answered::of(&[], &[outcome("k1", "handed", NOW)], &all, NOW).holds(&line), "not this computer's");
        let forged = Outcome { device: "other".into(), ..outcome("k1", "handed", NOW) };
        assert!(!Answered::of(std::slice::from_ref(&mine), &[forged], &all, NOW).holds(&line), "said by another phone");
        let read = Ask { verb: "read".into(), words: String::new(), ..mine.clone() };
        assert!(!Answered::of(&[read], &[outcome("k1", "handed", NOW)], &all, NOW).holds(&line), "a Mark as read is no answer");
        // A number texted: its texts, not a chat app's line from the same number.
        let mut texted = Answered::default();
        texted.texted("+33199001234".into());
        texted.texted(String::new());
        let text = phonemsgs::Line { kind: "text".into(), talk: String::new(), app: "foundation.e.message".into(), ..line.clone() };
        assert!(texted.holds(&text));
        assert!(!texted.holds(&line), "Signal's line from the same number");
        assert!(!texted.holds(&phonemsgs::Line { key: "+33261910000".into(), ..text.clone() }));
    }

    /// A Porch's line's answer: through the notification's Reply while the
    /// phone says it works (its conversation's newest notification, posted
    /// anew under another name, too, answered by that one's own line),
    /// "Reply on your phone" once it went; a text the texts' way from the
    /// phone it came to, where that phone sends texts for this computer;
    /// this computer's replies under it with their states, Take back said at
    /// once; none for an older phone's lines or a code's; a Mark as read
    /// handed makes the line seen.
    #[test]
    fn a_porch_line_offers_a_reply_while_the_phone_can_send_it() {
        let tr = crate::i18n::Translator::new("en");
        let line = phonemsgs::Line { device: "phone".into(), ..alice_line() };
        let porch = phonemsgs::PorchLine { ids: vec![line.id.clone()], head: "Alice wrote on Signal:".into(), said: Vec::new(), note: String::new(), number: String::new(), dial: String::new(), known: true, texted: false, code: false, who: "Alice".into(), answer: None };
        let live = |notice: &str, reply: bool, gone: i64, at: i64| (("phone".to_string(), notice.to_string()), Live { notice: notice.into(), app: line.app.clone(), talk: line.talk.clone(), reply, read: reply, gone, at, device: "phone".into() });
        let shared = |_: &str| Some(NOW);
        let time = |_: i64| "14:03".to_string();
        let none = |_: &str, _: &str, _: i64| Vec::new();
        let (no_one, nobody) = (|_: &str| false, BTreeSet::new());
        let all = vec![line.clone()];
        let offer = |live: &BTreeMap<(String, String), Live>, asks: &[Ask], outcomes: &[Outcome], taken_back: &BTreeSet<String>| {
            let o = Offer { live, asks, outcomes, tr: &tr, taken_back, lines: &all, texts_from: &no_one, shared: &shared, time_of: &time, sent_texts: &none };
            answer_of(&porch, &[&line], &o)
        };
        let open: BTreeMap<_, _> = [live(&line.notice, true, 0, NOW)].into();
        let answer = offer(&open, &[], &[], &nobody).unwrap();
        assert!(answer.offered && answer.read && !answer.texts);
        assert_eq!((answer.placeholder.as_str(), answer.label.as_str(), answer.phone.as_str(), answer.line.as_str()), ("Reply to Alice on Signal…", "Signal", "phone", line.id.as_str()));
        assert!(!answer.note.is_empty(), "Signal marks the conversation read when it replies");
        assert_eq!(answer.limit, WORDS_MAX);
        // Gone from the phone: "Reply on your phone".
        let gone: BTreeMap<_, _> = [live(&line.notice, false, NOW, NOW)].into();
        assert!(!offer(&gone, &[], &[], &nobody).unwrap().offered);
        // Posted anew under another name: that notification's Reply, by its own newest line; none of its lines here: not offered.
        let anew: BTreeMap<_, _> = [live(&line.notice, false, NOW, NOW), live("1111222233334444", true, 0, NOW + MINUTE)].into();
        assert!(!offer(&anew, &[], &[], &nobody).unwrap().offered, "no line of the newer notification here");
        let newer = phonemsgs::Line { at: NOW - MINUTE, id: format!("{}-bb22", NOW - MINUTE), notice: "1111222233334444".into(), text: "Tu es où ?".into(), ..line.clone() };
        let with_newer = vec![line.clone(), newer.clone()];
        let o = Offer { live: &anew, asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &with_newer, texts_from: &no_one, shared: &shared, time_of: &time, sent_texts: &none };
        let answer = answer_of(&porch, &[&line], &o).unwrap();
        assert_eq!((answer.offered, answer.notice.as_str(), answer.line.as_str()), (true, "1111222233334444", newer.id.as_str()));
        // Nothing said yet of it (an older phone's live lines): not offered.
        assert!(!offer(&BTreeMap::new(), &[], &[], &nobody).unwrap().offered);
        // A service's text (a sender named in letters, a short number): never from here.
        let service = phonemsgs::Line { kind: "text".into(), app: "foundation.e.message".into(), who: "automaton".into(), key: String::new(), name: "AMELI".into(), ..line.clone() };
        let o = Offer { live: &open, asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &all, texts_from: &no_one, shared: &shared, time_of: &time, sent_texts: &none };
        let said = answer_of(&porch, &[&service], &o).unwrap();
        assert!(!said.offered && !said.read, "the phone would refuse it");
        let short = phonemsgs::Line { who: "neutral".into(), key: "36179".into(), ..service.clone() };
        assert!(!answer_of(&porch, &[&short], &o).unwrap().offered);
        // This computer's reply under it, with its state; Take back said at once.
        let mine = Ask { phone: "phone".into(), ..ask("k1", "J'arrive.", NOW) };
        let handed = Outcome { key: "k1".into(), state: "handed".into(), at: NOW, device: "phone".into(), ..Outcome::default() };
        let answer = offer(&open, std::slice::from_ref(&mine), std::slice::from_ref(&handed), &nobody).unwrap();
        assert_eq!(answer.mine.iter().map(|m| (m.text.as_str(), m.said.as_str(), m.waiting)).collect::<Vec<_>>(), [("J'arrive.", "Handed to Signal at 14:03.", false)]);
        let waiting = offer(&open, std::slice::from_ref(&mine), &[], &nobody).unwrap();
        assert!(waiting.mine[0].waiting, "Take back while it waits");
        let back: BTreeSet<String> = ["k1".to_string()].into();
        let taking = offer(&open, std::slice::from_ref(&mine), &[], &back).unwrap();
        assert_eq!((taking.mine[0].waiting, taking.mine[0].said.as_str()), (false, "Taking it back: your phone will not send it, unless it has taken it already."));
        let late = offer(&open, std::slice::from_ref(&mine), std::slice::from_ref(&handed), &back).unwrap();
        assert!(late.mine[0].said.ends_with("Take back came too late: your phone had taken it already."));
        // A text from a whole number: the texts' way from the phone it came to, where it sends texts for this computer.
        let texted = phonemsgs::PorchLine { texted: true, dial: "+33199001234".into(), ..porch.clone() };
        let sent = |phone: &str, to: &str, _: i64| vec![Mine { key: "t1".into(), time: "14:00".into(), text: "Oui.".into(), said: format!("Sent from {phone} to {to}."), texts: true, ..Mine::default() }];
        let this_phone = |phone: &str| phone == "phone";
        let o = Offer { live: &BTreeMap::new(), asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &all, texts_from: &this_phone, shared: &shared, time_of: &time, sent_texts: &sent };
        let answer = answer_of(&texted, &[&line], &o).unwrap();
        assert!(answer.texts && answer.offered && answer.limit == crate::texts::WRITE_MAX && answer.phone == "phone");
        assert_eq!(answer.mine[0].said, "Sent from phone to +33199001234.");
        let other_phone = |phone: &str| phone == "other-phone";
        let o = Offer { live: &open, asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &all, texts_from: &other_phone, shared: &shared, time_of: &time, sent_texts: &sent };
        let answer = answer_of(&texted, &[&line], &o).unwrap();
        assert!(!answer.texts && answer.offered, "another phone sends texts: its own Reply, never the other's SIM");
        // None for a code's line, nor for an older phone's lines (no notification named).
        let older = phonemsgs::Line { notice: String::new(), ..line.clone() };
        let o = Offer { live: &open, asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &all, texts_from: &no_one, shared: &shared, time_of: &time, sent_texts: &none };
        assert!(answer_of(&porch, &[&older], &o).is_none());
        assert!(answer_of(&phonemsgs::PorchLine { code: true, ..porch.clone() }, &[&line], &o).is_none());
        // A Mark as read handed: the line seen too.
        let read = Ask { verb: "read".into(), words: String::new(), ..mine.clone() };
        assert!(read_handed(std::slice::from_ref(&read), std::slice::from_ref(&handed), "phone", &line.notice));
        assert!(!read_handed(std::slice::from_ref(&mine), std::slice::from_ref(&handed), "phone", &line.notice), "a reply is no Mark as read");
    }

    /// From the Porch to the phone: the request a Porch's answer writes is
    /// one the phone's verdict sends, its notification's own Reply or the
    /// conversation's newer notification's, each by its own line.
    #[test]
    fn what_the_porch_offers_the_phone_sends() {
        let tr = crate::i18n::Translator::new("en");
        let line = phonemsgs::Line { device: "phone".into(), ..alice_line() };
        let newer = phonemsgs::Line { at: NOW - MINUTE, id: format!("{}-bb22", NOW - MINUTE), notice: "1111222233334444".into(), text: "Tu es où ?".into(), ..line.clone() };
        let lines = vec![line.clone(), newer.clone()];
        let porch = phonemsgs::PorchLine { ids: vec![line.id.clone()], head: String::new(), said: Vec::new(), note: String::new(), number: String::new(), dial: String::new(), known: true, texted: false, code: false, who: "Alice".into(), answer: None };
        let live = |notice: &str, reply: bool, gone: i64| (("phone".to_string(), notice.to_string()), Live { notice: notice.into(), app: line.app.clone(), talk: line.talk.clone(), reply, read: reply, gone, at: NOW, device: "phone".into() });
        let (shared, time, none, no_one, nobody) = (|_: &str| Some(NOW), |_: i64| "14:03".to_string(), |_: &str, _: &str, _: i64| Vec::new(), |_: &str| false, BTreeSet::new());
        let dir = scratch("porch-to-phone");
        mark_ledger(&dir, NOW - 30 * MINUTE).unwrap();
        let ledger = open_ledger(&dir, NOW).unwrap();
        let (cancelled, unsent, yes) = (BTreeSet::new(), BTreeSet::new(), |_: &str| true);
        for (lives, expect) in [(vec![live(&line.notice, true, 0)], line.id.clone()), (vec![live(&line.notice, false, NOW), live(&newer.notice, true, 0)], newer.id.clone())] {
            let lives: BTreeMap<_, _> = lives.into_iter().collect();
            let o = Offer { live: &lives, asks: &[], outcomes: &[], tr: &tr, taken_back: &nobody, lines: &lines, texts_from: &no_one, shared: &shared, time_of: &time, sent_texts: &none };
            let answer = answer_of(&porch, &[&line], &o).unwrap();
            assert!(answer.offered);
            assert_eq!(answer.line, expect);
            let request = Ask { key: "k1".into(), phone: answer.phone.clone(), verb: "reply".into(), notice: answer.notice.clone(), app: answer.app.clone(), line: answer.line.clone(), words: "J'arrive.".into(), written: NOW - MINUTE, device: "desk".into(), ..Ask::default() };
            let judge = Judge { here: "phone", now_ms: NOW, ledger: &ledger, lines: &lines, counted: &yes, cancelled: &cancelled, answers: &yes, unsent: &unsent, went: &unsent };
            assert_eq!(verdict(&request, &judge), Verdict::Send, "{}", answer.notice);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

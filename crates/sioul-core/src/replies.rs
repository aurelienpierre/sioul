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

use crate::appnotes::{self, Kind, Posted};
use crate::calls;
use crate::phonemsgs::{self, Extra};
use crate::phones::Region;
use crate::texts::Sealer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
}

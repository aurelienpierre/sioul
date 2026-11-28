// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! New mail as a notification, at the times it may come (docs/porch.md,
//! "Notifications"). The Porch keeps mail until you look; what your lists let
//! through now is also told, once per batch, never once per message: the
//! count in words and the first senders with their subjects. Batched
//! notifications helped attention and mood where none at all made people more
//! anxious (Fitz et al. 2019): what helps is predictability, not silence.
//!
//! Mail that comes while it may not be told (its sender's row of the matrix
//! of what reaches you says later or shown, not told, now: as usual, sleep,
//! a pause, Free time, a slot of time for you; or the address it came to is
//! for another time, Always through and Reachable aside; `attention`) waits;
//! when a time begins in which some of it may come, one notification says
//! the Porch opens. Mail fetched late, that reached its server before the
//! moment was watched (a computer off all night: `Watch`), is announced the
//! same way, never as new. A row that says "not at all" now: never told, on
//! the Porch only.
//!
//! A row that says "at the gathered times" (◎, by day: decided 26 November
//! 2026) shows the mail on the Porch now and tells it at the next gathered
//! time (`[reminders] gathered`) after it reached its server, when the row
//! says so then and still now: mail that comes within two minutes after one
//! comes with it (`GATHERING_NEW`); what waited for it, and mail fetched late
//! that reached its server before it, is told at the first look after it,
//! however late (a computer asleep, a phone's late step), in one notification
//! (`gathered_at`, `waited_for`; the ledger's `gathered`, the last gathering
//! a look went through).
//!
//! Each letter a notification names opens its own message (`openers`). What
//! a device told is said in its told log, which the sharing carries
//! (`say_told`, `told_elsewhere`): the device that tells next does not tell
//! it again.
//!
//! Never told: what is set aside (forged, spam, a borrowed name, blocked),
//! what your own spam filter flagged or moved (the review queue: it waits
//! for your word, on every device), hostile mail, codes (they have their own
//! notification), what you send yourself, the less important accounts,
//! newsletters unless asked, and mail read elsewhere before it came here. Each message once, by its account and
//! Message-ID, told or waiting (`Ledger`), on this device.

use crate::i18n::Translator;
use crate::porch::{Lane, Reason, Triaged};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// How many letters a notification names: the count says the rest.
pub const NAMED: usize = 3;

/// A message's mark is kept this long: the Porch shows its last two weeks.
const KEPT: i64 = 30 * 86_400;

/// A letter as a notification names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letter {
    /// Its account and Message-ID, hashed (`key`).
    pub key: String,
    pub sender: String,
    pub subject: String,
    /// From someone Always through, told through the system's do-not-disturb
    /// (critical urgency on a computer).
    pub through: bool,
    /// Its address (`mid:…`, `sioul:mail/…`), kept in memory until it is told
    /// (the ledger keeps the hashed `key` alone): what the notification of
    /// one message opens (the unified mailbox's part 2).
    pub address: String,
    /// Its notice (`notices::mail_id_of`) and its file here, in memory: the
    /// notification is taken away once each of its notices is settled.
    pub notice: String,
    pub file: String,
    /// When it reached its server (its file's time, which the fetch sets to
    /// the server's INTERNALDATE), Unix seconds; 0 unknown: whether it
    /// waited (`Watch`).
    pub arrived: i64,
    /// The message itself, whichever of your addresses it came to
    /// (`message_key`): told once across your addresses and your devices.
    pub message: String,
}

impl Letter {
    pub fn of(t: &Triaged) -> Letter {
        Letter {
            key: key(t),
            sender: one_line(t.card.sender()),
            subject: one_line(&t.card.subject),
            through: false,
            address: crate::mailfind::address_of_card(&t.card),
            notice: crate::notices::mail_id_of(&t.card),
            file: t.card.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            arrived: t.card.path.as_deref().map_or(0, arrived_at),
            message: message_key(t),
        }
    }
}

/// A message's own key, whichever of your addresses it came to (review of
/// phase 2, M5: one message to two of them is told once): its
/// Message-ID's digest (`notices::digest`, as the notices' ids hold it),
/// `"id:<digest>"`; without a Message-ID, its key here (`key`), since no copy
/// elsewhere can be told apart from it.
pub fn message_key(t: &Triaged) -> String {
    t.card.message_id.as_deref().map(crate::mailindex::bare_id).filter(|id| !id.is_empty()).map_or_else(|| key(t), |id| format!("id:{}", crate::notices::digest(&id)))
}

/// When a message file reached its server: the file's time (`maildir::store`
/// sets it to the server's INTERNALDATE), Unix seconds; 0 unknown.
pub fn arrived_at(file: &Path) -> i64 {
    crate::files::metadata(file).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// What a notice of new mail opens, one per letter it names (`openers`): its
/// words, the message's address, and where this device keeps it (its file,
/// for the window to find it at once; never sent to a phone's Java).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opener {
    pub label: String,
    pub address: String,
    pub hint: String,
}

/// The longest words of an opener, in characters: a button holds a few words.
pub const LABEL: usize = 32;

/// Each letter a notice names (three at most) opening its own message (the
/// notifications' phase 2; the unified mailbox's rule: every mail event opens
/// the message, never the account): its button says "Open" when the notice
/// is of one message, else the letter as the notice names it ("Murena, Your
/// invoice"), cut to `LABEL`. A letter without an address has none. A click
/// on the notice itself opens the first; none at all, the mailbox at its top.
pub fn openers(tr: &Translator, letters: &[Letter]) -> Vec<Opener> {
    let one = letters.len() == 1;
    letters
        .iter()
        .take(NAMED)
        .filter(|l| !l.address.is_empty())
        .map(|l| Opener { label: if one { tr.text("reminder-open", None) } else { cut(&letter_words(tr, l), LABEL) }, address: l.address.clone(), hint: l.file.clone() })
        .collect()
}

/// `text` cut to `most` characters, an ellipsis ending what was cut.
fn cut(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_string();
    }
    let kept: String = text.chars().take(most.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// Since when this device has watched the moment it is in, without a gap:
/// new mail that reached its server before then waited, whatever the time it
/// is fetched (it came in another time, or while this device was off, asleep
/// or fetching nothing), and is announced as waited ("The Porch opens"),
/// never as new (the notifications' phase 2). Looked at by every process that
/// tells, at each of its minutes or steps, and kept on this device
/// (`$XDG_STATE_HOME/sioul/mail-watch.toml`, format 1), so that a process
/// started again (an install, a phone's service) goes on with the watch its
/// predecessor kept, when recent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watch {
    #[serde(default)]
    pub format: u32,
    #[serde(default)]
    moment: String,
    #[serde(default)]
    since: i64,
    #[serde(default)]
    last: i64,
}

/// The watch's format: 1.
pub const WATCH_FORMAT: u32 = 1;

/// A look written to the disk at least this often (seconds), the moment
/// unchanged: a minute's look otherwise rewrites nothing.
const WATCH_WRITTEN: i64 = 300;

/// A look this much later than the last one (seconds) begins a new watch:
/// what came between was not watched. Longer than a phone's mail rhythm
/// (fifteen minutes) and the delays Android adds to its alarms.
pub const GAP: i64 = 40 * 60;

impl Watch {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("mail-watch.toml")
    }

    /// As kept; none (a first look) when missing, unreadable or of another format.
    pub fn load(path: &Path) -> Watch {
        crate::files::read_to_string(path).ok().and_then(|t| toml::from_str::<Watch>(&t).ok()).filter(|w| w.format == WATCH_FORMAT).unwrap_or_default()
    }

    /// The moment (`moment`'s words) looked at `now`: since when it has been
    /// watched, this look included. The same moment, looked at within `GAP`:
    /// the watch goes on. Another moment within `GAP`: it began after the
    /// last look, so mail since then is new in it (review of phase 2, L1: a
    /// watch begun at the first look after a moment began made the mail of
    /// those minutes "waited"). A first look, a gap longer than `GAP`
    /// (asleep, suspended, off, nothing looked) or a clock gone back: a
    /// watch begins now.
    pub fn look(&mut self, moment: &str, now: i64) -> i64 {
        let watched = self.last > 0 && now >= self.last && now - self.last <= GAP;
        if !watched {
            *self = Watch { format: WATCH_FORMAT, moment: moment.to_string(), since: now, last: now };
        } else if self.moment != moment {
            *self = Watch { format: WATCH_FORMAT, moment: moment.to_string(), since: self.last, last: now };
        } else {
            self.last = now;
        }
        self.since
    }

    /// The watch kept in `path` looked at (`look`), under its lock, written
    /// back when the moment or its start changed or `WATCH_WRITTEN` passed:
    /// since when this device has watched the moment.
    pub fn look_kept(path: &Path, moment: &str, now: i64) -> i64 {
        crate::filelock::with_lock(path, || {
            let kept = Watch::load(path);
            let mut watch = kept.clone();
            let since = watch.look(moment, now);
            if watch.moment != kept.moment || watch.since != kept.since || watch.last - kept.last >= WATCH_WRITTEN {
                let written = toml::to_string(&watch).map_err(|e| e.to_string()).and_then(|text| crate::filelock::replace(path, text.as_bytes()).map_err(|e| e.to_string()));
                if let Err(e) = written {
                    eprintln!("sioul: mail watch: {e}");
                }
            }
            since
        })
    }
}

/// A letter that reached its server this little before the watch began
/// (seconds) is new still: the minute's look sees a new moment up to a
/// minute late, and the server's clock is not this device's.
pub const LATE: i64 = 120;

/// Whether any of `letters` waited: it reached its server before the watch
/// began (`since`, `LATE` aside); one whose arrival is unknown did not.
pub fn any_waited(letters: &[Letter], since: i64) -> bool {
    letters.iter().any(|l| l.arrived > 0 && l.arrived < since - LATE)
}

/// Where each device says which new mail it told, in the notices' store
/// (`notices::FOLDER`): `told/<device>.jsonl`, a line per notification,
/// `{"at": <ms>, "told": [<messages' keys>]}` (`message_key`: a digest of
/// each Message-ID, whichever address it came to). The sharing carries it
/// with the notices' words, so that a device that becomes the one that tells
/// (the "notices" lease moving) does not tell again what another told. An
/// older Sioul reads none of it.
pub const TOLD: &str = "told";

/// A device's word on what it told is kept as long as the ledger keeps its
/// own marks (milliseconds; review of phase 2, M4: a device away for a long
/// weekend must still know what the other told).
pub const TOLD_KEPT_MS: i64 = KEPT * 1000;

/// A line of a device's told log: the messages it told (`told`, their keys);
/// since 28 November 2026 the notices of Sioul's own it told too (`notices`,
/// each `notice::Notice::told_id`: a digest, never its words), which a
/// device on 608d8521 reads past (its line has `at` and `told` alone, and
/// such a line's `told` is empty).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToldLine {
    at: i64,
    #[serde(default)]
    told: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notices: Vec<String>,
}

/// A device's own told log in the notices' folder `notices`.
pub fn told_file(notices: &Path, device: &str) -> PathBuf {
    notices.join(TOLD).join(format!("{}.jsonl", crate::calls::file_safe(device)))
}

/// What this device (`device`) just told, by its letters' messages' keys: a line
/// at the end of its own told log, its lines older than `TOLD_KEPT_MS` taken out.
pub fn say_told(notices: &Path, device: &str, ids: &[String], now_ms: i64) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    say_line(notices, device, &ToldLine { at: now_ms, told: ids.to_vec(), notices: Vec::new() })
}

/// What this device (`device`) just told of Sioul's own notices, by their
/// ids (`notice::Notice::told_id`): a line of its told log, as `say_told`
/// writes one, so that another device drops its own copy of the same notice
/// (`notices_told_elsewhere`; the owner's yes of 26 November 2026).
pub fn say_notices_told(notices: &Path, device: &str, ids: &[String], now_ms: i64) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    say_line(notices, device, &ToldLine { at: now_ms, told: Vec::new(), notices: ids.to_vec() })
}

/// A line at the end of this device's told log, its lines older than `TOLD_KEPT_MS` taken out.
fn say_line(notices: &Path, device: &str, line: &ToldLine) -> Result<(), String> {
    let path = told_file(notices, device);
    let now_ms = line.at;
    crate::calls::rewrite_lines(&path, |kept| !serde_json::from_str::<ToldLine>(kept).is_ok_and(|l| now_ms - l.at <= TOLD_KEPT_MS))?;
    let text = serde_json::to_string(line).map_err(|e| e.to_string())? + "\n";
    crate::filelock::with_lock(&path, || crate::calls::append_private(&path, &text))
}

/// The notices of Sioul's own the other devices told from `since_ms` on, by
/// their ids (`notice::Notice::told_id`), as their told logs say (this
/// device's own, `device`, aside).
pub fn notices_told_elsewhere(notices: &Path, device: &str, since_ms: i64) -> BTreeSet<String> {
    let own = crate::calls::file_safe(device);
    crate::calls::device_files(&notices.join(TOLD))
        .into_iter()
        .filter(|(name, _)| *name != own)
        .flat_map(|(_, path)| crate::calls::read_lines::<ToldLine>(&path))
        .filter(|l| l.at >= since_ms)
        .flat_map(|l| l.notices)
        .collect()
}

/// What the other devices told in the last `TOLD_KEPT_MS`, by message key, as
/// their logs in `notices` say (this device's own, `device`, aside).
pub fn told_elsewhere(notices: &Path, device: &str, now_ms: i64) -> BTreeSet<String> {
    let own = crate::calls::file_safe(device);
    crate::calls::device_files(&notices.join(TOLD))
        .into_iter()
        .filter(|(name, _)| *name != own)
        .flat_map(|(_, path)| crate::calls::read_lines::<ToldLine>(&path))
        .filter(|l| now_ms - l.at <= TOLD_KEPT_MS)
        .flat_map(|l| l.told)
        .collect()
}

/// A sender's name or a subject on one line: a mail's words never break the notification's.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A message's key: its account and Message-ID, else its file's unique name
/// (which a change of flags keeps), hashed: the same message fetched again
/// keeps it, the same one in another account is another; and the ledger
/// holds no address and no Message-ID.
pub fn key(t: &Triaged) -> String {
    let account = t.card.account.as_deref().unwrap_or("");
    let id = t
        .card
        .message_id
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .or_else(|| t.card.path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).map(|n| format!("file:{}", crate::maildir::unique_part(n))))
        .unwrap_or_default();
    format!("{:016x}", fnv(&format!("{account}\u{1f}{id}")))
}

/// Read before it came here (the server said \Seen: read on a phone, in a
/// webmail): nothing to tell.
fn read_already(t: &Triaged) -> bool {
    t.card.path.as_deref().is_some_and(|p| crate::maildir::flags_of(p).contains('S'))
}

/// Whether a message is never told, whatever the time: set aside (forged,
/// spam, a borrowed name, blocked), in the review queue (your own filter
/// flagged it, or moved it into a Junk folder), hostile, a code (it has its
/// own notification), from yourself, in the less important accounts' lane, a
/// newsletter in the Filed lane unless `newsletters` (an automatic sender
/// there, a bill from no-reply, is told), or read already.
pub fn never(t: &Triaged, newsletters: bool) -> bool {
    match &t.lane {
        Lane::SetAside | Lane::Review | Lane::Hostile | Lane::RightNow | Lane::Low => return true,
        Lane::Filed if !newsletters && t.reasons.contains(&Reason::Newsletter) => return true,
        _ => {}
    }
    t.reasons.iter().any(|r| matches!(r, Reason::FromYourself | Reason::Blocked)) || read_already(t)
}

/// The moment as new mail sees it: what now is for, its columns and layers
/// in the matrix (a slot of time for you, do-not-disturb from its switch or
/// a focus session), Free time's "Nothing at all", and whether mail may be
/// told at all. When it changes, a time began: what waited and may come now
/// is told once (`waited`).
pub fn moment(now: &crate::attention::Now, may: bool) -> String {
    let columns: Vec<&str> = now.times.iter().map(|c| c.id()).collect();
    let moment = format!("{}:{}:{}:{}:{}:{may}", now.time.id(), columns.join("+"), now.slot, now.dnd, now.nothing);
    // Where a device narrows what comes (docs/areas.md, "Which device tells"), what comes forward
    // on the device read now, and what no device holds, are part of the moment: a cell changed,
    // a line said, a device that counts again or no more, begins a time as the hours do, and
    // what waited is judged again. With nothing said, the moment's words are as they were.
    if now.here.narrows() { format!("{moment}:{}:{}", crate::areas::Line::cell_id(now.here.cell(now.time, now.week)), crate::areas::Line::cell_id(now.here.unheld)) } else { moment }
}

/// How long after a gathered time new mail at the gathered times (◎) is told
/// at once: it comes with that gathering, as the sites' gathered notice is
/// made within two minutes of its time.
pub const GATHERING_NEW: i64 = 120;

/// How far back the last gathered time is looked for: a day and a half, so
/// that the latest one is always found, however late the first look after
/// it (a computer asleep all afternoon, a phone's step an hour late).
pub const GATHERING_LAST: i64 = 36 * 3600;

/// The latest gathered time (`[reminders] gathered`, read as
/// `appnotes::times_of` reads it) at or before `now` and at most `within`
/// seconds before it, as Unix seconds; none when no gathered time fell then.
pub fn gathered_at(times: &[String], now: &jiff::Zoned, within: i64) -> Option<i64> {
    let stamp = now.timestamp().as_second();
    let days = [Some(now.date()), now.date().yesterday().ok()];
    crate::appnotes::times_of(times)
        .into_iter()
        .flat_map(|time| days.into_iter().flatten().filter_map(move |day| day.to_datetime(time).to_zoned(now.time_zone().clone()).ok()))
        .map(|at| at.timestamp().as_second())
        .filter(|at| *at <= stamp && stamp - *at <= within)
        .max()
}

/// Whether a letter waiting since `since` (the ledger's `waiting`) waited for
/// the gathering at `gathered`: it began to wait before that gathered time.
pub fn waited_for(since: Option<i64>, gathered: Option<i64>) -> bool {
    matches!((since, gathered), (Some(since), Some(at)) if since < at)
}

/// What becomes of a new message now, as the matrix says of its sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// Told now.
    Now,
    /// Told now, through the system's do-not-disturb: someone Always through.
    Through,
    /// It waits for a time in which it is told.
    Later,
    /// Never told: its row says "not at all" now; it stays on the Porch.
    Never,
}

/// What was done with each new message, by key: told (or never to tell), or
/// waiting for a time it may come in; with when. On this device only, in
/// `$XDG_STATE_HOME/sioul/mail-notified.toml`, forgotten after 30 days.
/// Format 2 (14 November 2026) adds the messages told, whichever address
/// they came to (`messages`); a ledger of format 1 (no `format`) reads with
/// none, and an older Sioul passes them over.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub format: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub told: BTreeMap<String, i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub waiting: BTreeMap<String, i64>,
    /// The messages told, by their own key (`message_key`), with when: a copy
    /// of one at another of your addresses is told no more (review of phase
    /// 2, M5).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub messages: BTreeMap<String, i64>,
    /// The last gathered time a look went through (Unix seconds): the mail
    /// at the gathered times waiting since before a later one is told at the
    /// next look, however late (format 3, 27 November 2026; a ledger of an
    /// older format reads with none, and an older Sioul passes it over).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gathered: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// The ledger's format: 3, with `gathered` (2 added `messages`).
pub const LEDGER_FORMAT: u32 = 3;

impl Ledger {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("mail-notified.toml")
    }

    /// As kept; empty when missing or unreadable (nothing then is told twice
    /// but what was waiting, which waits no more).
    pub fn load(path: &Path) -> Ledger {
        crate::files::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written whole under another name, then renamed: never half a ledger.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            crate::files::create_dir_all(parent).map_err(fail)?;
        }
        let text = toml::to_string(&Ledger { format: LEDGER_FORMAT, ..self.clone() }).map_err(|e| e.to_string())?;
        let temporary = path.with_extension(format!("toml.{}", std::process::id()));
        crate::files::write(&temporary, text).map_err(fail)?;
        crate::files::rename(&temporary, path).map_err(fail)
    }

    /// Whether a message was dealt with already: told, or waiting.
    pub fn known(&self, key: &str) -> bool {
        self.told.contains_key(key) || self.waiting.contains_key(key)
    }

    /// Told (or never to tell): never again.
    pub fn tell(&mut self, key: &str, at: i64) {
        self.waiting.remove(key);
        self.told.insert(key.to_string(), at);
    }

    /// A message told, by its own key (`message_key`): a copy of it at
    /// another address is told no more.
    pub fn told_message(&mut self, message: &str, at: i64) {
        if !message.is_empty() {
            self.messages.entry(message.to_string()).or_insert(at);
        }
    }

    /// Waiting for a time it may come in, since `at`: when it reached its
    /// server, for the gathered times (`waited_for`).
    pub fn wait(&mut self, key: &str, at: i64) {
        if !self.told.contains_key(key) {
            self.waiting.entry(key.to_string()).or_insert(at);
        }
    }

    /// Marks older than 30 days forgotten: their mail left the Porch's view long ago.
    pub fn forget_old(&mut self, now: i64) {
        self.told.retain(|_, at| now - *at < KEPT);
        self.waiting.retain(|_, at| now - *at < KEPT);
        self.messages.retain(|_, at| now - *at < KEPT);
    }
}

/// A fetch's arrivals, sorted.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Sorted {
    /// To tell now, in the order they came.
    pub now: Vec<Letter>,
    /// Waiting for a time they may come in: their keys.
    pub later: Vec<String>,
    /// Never to tell: their keys.
    pub never: Vec<String>,
}

/// Sorts the inbox's arrivals of a fetch, as the Porch judged them: those
/// told now, those that wait, those never told (`fate`: what the matrix of
/// what reaches you says of each now, `attention::Attention::decide`; and
/// the lanes never told). A message the ledger knows is left out: told once.
/// A message told already at another of your addresses, or come to two of
/// them in this fetch, is told once: its other copies never.
pub fn sort(arrivals: &[Triaged], ledger: &Ledger, newsletters: bool, fate: impl Fn(&Triaged) -> Fate) -> Sorted {
    let mut out = Sorted::default();
    let mut seen = BTreeSet::new();
    let mut messages = BTreeSet::new();
    for t in arrivals {
        let key = key(t);
        if ledger.known(&key) || !seen.insert(key.clone()) {
            continue;
        }
        let message = message_key(t);
        if never(t, newsletters) || ledger.messages.contains_key(&message) {
            out.never.push(key);
            continue;
        }
        match fate(t) {
            Fate::Now | Fate::Through if messages.contains(&message) => out.never.push(key),
            Fate::Now => {
                messages.insert(message);
                out.now.push(Letter::of(t));
            }
            Fate::Through => {
                messages.insert(message);
                out.now.push(Letter { through: true, ..Letter::of(t) });
            }
            Fate::Later => out.later.push(key),
            Fate::Never => out.never.push(key),
        }
    }
    out
}

/// The mail that waited and may come now (`now`, as for `sort`): among the
/// Porch's messages (`items`), those waiting in the ledger, still unread and
/// not set aside since (a sender blocked meanwhile), each message once
/// (two addresses' copies one letter), none told at another address since.
pub fn waited(items: &[Triaged], ledger: &Ledger, newsletters: bool, now: impl Fn(&Triaged) -> bool) -> Vec<Letter> {
    let (mut seen, mut messages) = (BTreeSet::new(), BTreeSet::new());
    items
        .iter()
        .filter(|t| ledger.waiting.contains_key(&key(t)) && !never(t, newsletters) && !ledger.messages.contains_key(&message_key(t)) && now(t))
        .map(Letter::of)
        .filter(|l| seen.insert(l.key.clone()) && messages.insert(l.message.clone()))
        .collect()
}

/// Among the Porch's messages, those waiting that will never be told now:
/// read meanwhile, blocked, set aside, or told at another of your addresses.
/// Their marks become told.
pub fn gone_by(items: &[Triaged], ledger: &Ledger, newsletters: bool) -> Vec<String> {
    items.iter().filter(|t| never(t, newsletters) || ledger.messages.contains_key(&message_key(t))).map(key).filter(|k| ledger.waiting.contains_key(k)).collect()
}

/// The notification of new mail: "Two e-mails", and the first senders with
/// their subjects, "Murena, Your invoice · Alice, Dinner on Friday".
pub fn batch(tr: &Translator, letters: &[Letter]) -> (String, String) {
    (tr.text("mail-note-title", Some(&tr.counted(letters.len()))), named(tr, letters))
}

/// When a time begins in which mail that waited may come: "The Porch opens:
/// three e-mails wait for you.", the first senders below.
pub fn opens(tr: &Translator, letters: &[Letter]) -> (String, String) {
    (tr.text("mail-note-opens", Some(&tr.counted(letters.len()))), named(tr, letters))
}

/// The first letters, "Sender, Subject", joined by " · ".
fn named(tr: &Translator, letters: &[Letter]) -> String {
    letters.iter().take(NAMED).map(|l| letter_words(tr, l)).collect::<Vec<_>>().join(" · ")
}

/// A letter as a notification names it: "Sender, Subject".
fn letter_words(tr: &Translator, letter: &Letter) -> String {
    let mut args = crate::i18n::args();
    args.set("sender", letter.sender.clone());
    args.set("subject", if letter.subject.is_empty() { tr.text("mail-no-subject", None) } else { letter.subject.clone() });
    tr.text("mail-note-letter", Some(&args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{Area, Time, Week};
    use crate::porch::{SenderList, Senders};
    use crate::quiet::{Mode, Reason as Why};

    /// Told now or later, as a test's rule says.
    fn fate(now: bool) -> Fate {
        if now { Fate::Now } else { Fate::Later }
    }

    /// A message as the Porch judges it, fetched into `account`; `headers` before the subject.
    fn message(account: &str, from: &str, headers: &str, subject: &str, id: &str, senders: &Senders, priority: crate::config::Priority) -> Triaged {
        let raw = format!("From: {from}\r\nMessage-ID: <{id}>\r\n{headers}Subject: {subject}\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\n{subject}.\r\n");
        let known = SenderList::parse("alice@example.org");
        let trusted = ["mx.example.net".to_string()];
        let own = ["me@example.net".to_string()];
        let ctx = crate::porch::Context { projects: None, known: &known, senders, trusted_ids: &trusted, now: None, priority, own_domains: &[], shielded: false, assessments: None, words: None, own_addresses: &own, spam: None };
        let mut card = crate::card::Card::from_bytes(raw.as_bytes()).unwrap();
        card.account = Some(account.to_string());
        card.path = Some(PathBuf::from(format!("/mail/{account}/new/1759400000.U1-{}.sioul", fnv(id) % 10_000)));
        crate::porch::triage(card, &ctx)
    }

    fn mode(time: Time, reason: Why) -> Mode {
        Mode { quiet: false, time, week: Week { work_hours: true, admin_hours: false, meals: true, sleep: true }, reason, until: None, back: None, label: String::new(), here: crate::areas::Here::default() }
    }

    /// The gathered times as the mail at the gathered times (◎) reads them:
    /// the latest one within a span before now, yesterday's last too, never
    /// one to come; a letter waited for one when it began to wait before it.
    #[test]
    fn the_gathering_mail_waits_for() {
        let times = ["09:00".to_string(), "13:00".to_string(), "18:00".to_string()];
        let at = |text: &str| text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::UTC).unwrap();
        let stamp = |text: &str| at(text).timestamp().as_second();
        assert_eq!(gathered_at(&times, &at("2026-11-27T13:01:00"), GATHERING_NEW), Some(stamp("2026-11-27T13:00:00")));
        assert_eq!(gathered_at(&times, &at("2026-11-27T13:03:00"), GATHERING_NEW), None, "two minutes for new mail");
        assert_eq!(gathered_at(&times, &at("2026-11-27T13:15:00"), GATHERING_LAST), Some(stamp("2026-11-27T13:00:00")), "a phone's look a quarter of an hour on");
        assert_eq!(gathered_at(&times, &at("2026-11-27T17:40:00"), GATHERING_LAST), Some(stamp("2026-11-27T13:00:00")), "a computer asleep all afternoon: the last one, however late");
        assert_eq!(gathered_at(&times, &at("2026-11-27T08:59:00"), GATHERING_LAST), Some(stamp("2026-11-26T18:00:00")), "never one to come: yesterday's last");
        assert_eq!(gathered_at(&["23:55".to_string()], &at("2026-11-28T00:05:00"), GATHERING_LAST), Some(stamp("2026-11-27T23:55:00")), "across midnight");
        assert_eq!(gathered_at(&[], &at("2026-11-27T09:00:30"), GATHERING_NEW), Some(stamp("2026-11-27T09:00:00")), "none set: 09:00, 13:00, 18:00");
        let gathering = Some(stamp("2026-11-27T13:00:00"));
        assert!(waited_for(Some(stamp("2026-11-27T08:00:00")), gathering));
        assert!(!waited_for(Some(stamp("2026-11-27T13:05:00")), gathering), "come after it: the next one");
        assert!(!waited_for(None, gathering) && !waited_for(Some(1), None));
    }

    #[test]
    fn a_batch_once_at_its_time() {
        let senders = Senders { safe: SenderList::parse("alice@example.org"), blocked: SenderList::parse("spam@example.com"), ..Senders::default() };
        let usual = crate::config::Priority::default();
        let alice = message("home", "Alice <alice@example.org>", "", "Dinner on Friday", "1@example.org", &senders, usual);
        let bill = message("home", "Murena <no-reply@murena.example>", "", "Your invoice", "2@murena.example", &senders, usual);
        let bob = message("home", "Bob <bob@example.org>", "", "Photos", "3@example.org", &senders, usual);
        let news = message("home", "Shop <news@shop.example>", "List-Id: <news.shop.example>\r\nList-Unsubscribe: <mailto:u@shop.example>\r\n", "Sale", "4@shop.example", &senders, usual);
        let code = message("home", "Bank <codes@bank.example>", "", "Your verification code: 482913", "5@bank.example", &senders, usual);
        let forged = message("home", "Alice <alice@example.org>", "Authentication-Results: mx.example.net; dmarc=fail (p=reject) header.from=example.org\r\n", "Urgent", "6@example.org", &senders, usual);
        let own = message("home", "Me <me@example.net>", "Authentication-Results: mx.example.net; dmarc=pass header.from=example.net\r\n", "A file", "7@example.net", &senders, usual);
        let low = message("social", "Network <hello@social.example>", "", "Someone liked", "8@social.example", &senders, crate::config::Priority::Below);
        let mut read = message("home", "Carol <carol@example.org>", "", "Read on the phone", "9@example.org", &senders, usual);
        read.card.path = Some(PathBuf::from("/mail/home/cur/1759400000.U1-9.sioul:2,S"));
        assert_eq!(bill.lane, Lane::Filed, "an automatic sender is filed");
        assert_eq!((news.lane.clone(), code.lane.clone(), forged.lane.clone(), low.lane.clone()), (Lane::Filed, Lane::RightNow, Lane::SetAside, Lane::Low));
        let arrivals = vec![bill.clone(), alice.clone(), bob.clone(), news.clone(), code, forged, own, low, read];

        // In view: the safe sender always; the others as the matrix says (here: the bill and Bob too).
        let ledger = Ledger::default();
        let sorted = sort(&arrivals, &ledger, false, |t| fate(t.card.from_address.as_deref() != Some("bob@example.org")));
        assert_eq!(sorted.now.iter().map(|l| l.sender.as_str()).collect::<Vec<_>>(), ["Murena", "Alice"], "{sorted:?}");
        assert_eq!(sorted.later, [key(&bob)], "Bob waits for his list's times");
        assert_eq!(sorted.never.len(), 6, "the newsletter, the code, the forged one, your own, the less important account, the one read already");
        // Newsletters when asked.
        assert!(sort(&[news.clone()], &ledger, true, |_| Fate::Now).now.len() == 1);

        // The words: the count in words, the first senders with their subjects.
        let en = Translator::new("en");
        assert_eq!(batch(&en, &sorted.now), ("Two e-mails".to_string(), "Murena, Your invoice · Alice, Dinner on Friday".to_string()));
        let fr = Translator::new("fr");
        assert_eq!(batch(&fr, &sorted.now), ("Deux courriels".to_string(), "Murena, Your invoice · Alice, Dinner on Friday".to_string()));
        assert_eq!(batch(&fr, &sorted.now[..1]).0, "Un courriel");
        assert_eq!(batch(&en, &sorted.now[..1]).0, "One e-mail");
        // Past twelve, digits; three named at most; a message without a subject says so.
        let many: Vec<Letter> = (0..13).map(|i| Letter { key: i.to_string(), sender: format!("S{i}"), subject: if i == 0 { String::new() } else { format!("T{i}") }, through: false, address: format!("mid:letter-{i}@example.org"), notice: String::new(), file: format!("/mail/home/cur/{i}.sioul"), arrived: 0, message: format!("id:{i}") }).collect();
        assert_eq!(batch(&en, &many), ("13 e-mails".to_string(), "S0, (no subject) · S1, T1 · S2, T2".to_string()));

        // Marked once, by account and Message-ID: fetched again, nothing; the same message in another
        // account has its own mark, and is told once only when the message itself was told
        // (`told_message`, at the claim: one_message_to_two_addresses_is_told_once).
        let mut ledger = Ledger::default();
        for letter in &sorted.now {
            ledger.tell(&letter.key, 100);
        }
        for k in &sorted.never {
            ledger.tell(k, 100);
        }
        for k in &sorted.later {
            ledger.wait(k, 100);
        }
        assert!(sort(&arrivals, &ledger, false, |_| Fate::Now).now.is_empty(), "never twice");
        let elsewhere = message("work", "Alice <alice@example.org>", "", "Dinner on Friday", "1@example.org", &senders, usual);
        assert_eq!(sort(&[elsewhere.clone()], &ledger, false, |_| Fate::Now).now.len(), 1);
        ledger.told_message(&message_key(&alice), 100);
        assert!(sort(&[elsewhere], &ledger, false, |_| Fate::Now).now.is_empty(), "the message told: its copy in another account never");
        // A message without a Message-ID: its file's unique name, whatever its flags.
        let mut nameless = alice.clone();
        nameless.card.message_id = None;
        let flagged = Triaged { card: crate::card::Card { path: Some(PathBuf::from("/mail/home/cur/1759400000.U1-42.sioul:2,F")), ..nameless.card.clone() }, ..nameless.clone() };
        nameless.card.path = Some(PathBuf::from("/mail/home/new/1759400000.U1-42.sioul"));
        assert_eq!(key(&nameless), key(&flagged));

        // Asleep (or in a pause): nothing now, everything in view waits.
        let asleep = sort(&[bob.clone()], &Ledger::default(), false, |_| Fate::Later);
        // Its row saying "not at all" now: never told, on the Porch only.
        assert_eq!(sort(&[bob.clone()], &Ledger::default(), false, |_| Fate::Never).never.len(), 1);
        assert!(asleep.now.is_empty() && asleep.later.len() == 1);
        // A do-not-disturb you switched on: the people it lets through are told, the others wait.
        let gate = |t: &Triaged| t.card.from_address.as_deref() == Some("alice@example.org");
        let held = sort(&[alice.clone(), bob.clone()], &Ledger::default(), false, |t| fate(gate(t)));
        assert_eq!((held.now.len(), held.now[0].sender.as_str(), held.later.len()), (1, "Alice", 1));
        // Someone Always through: told through the system's do-not-disturb.
        let through = sort(&[alice.clone()], &Ledger::default(), false, |_| Fate::Through);
        assert!(through.now[0].through && !held.now[0].through);

        // A time begins: what waited and may come now, told once.
        let waiting = waited(&arrivals, &ledger, false, |_| true);
        assert_eq!(waiting.iter().map(|l| l.sender.as_str()).collect::<Vec<_>>(), ["Bob"]);
        assert_eq!(opens(&en, &waiting).0, "The Porch opens: one e-mail waits for you.");
        assert_eq!(opens(&fr, &[waiting[0].clone(), waiting[0].clone(), waiting[0].clone()]).0, "Le Porche ouvre\u{202f}: trois courriels vous attendent.");
        assert!(waited(&arrivals, &ledger, false, |_| false).is_empty(), "not while its list may not come");
        for letter in &waiting {
            ledger.tell(&letter.key, 200);
        }
        assert!(waited(&arrivals, &ledger, false, |_| true).is_empty(), "once per window");
        // Read meanwhile: no longer waiting.
        let mut ledger = Ledger::default();
        ledger.wait(&key(&bob), 100);
        let mut seen = bob.clone();
        seen.card.path = Some(PathBuf::from("/mail/home/cur/1759400000.U1-3.sioul:2,S"));
        assert!(waited(&[seen.clone()], &ledger, false, |_| true).is_empty());
        assert_eq!(gone_by(&[seen], &ledger, false), [key(&bob)]);
    }

    /// Each letter a notice names opens its own message (the notifications'
    /// phase 2): a button each, three at most, its file the hint; "Open" for
    /// one message alone; none for a letter without an address.
    #[test]
    fn each_letter_named_opens_itself() {
        let en = Translator::new("en");
        let letter = |i: usize, sender: &str, subject: &str| Letter { key: i.to_string(), sender: sender.into(), subject: subject.into(), through: false, address: format!("mid:letter-{i}@example.org"), notice: String::new(), file: format!("/mail/home/cur/{i}.sioul"), arrived: 0, message: format!("id:{i}") };
        let two = [letter(1, "Ana", "Lunch"), letter(2, "Noa", "The lease")];
        let open = openers(&en, &two);
        assert_eq!(open.iter().map(|o| (o.label.as_str(), o.address.as_str(), o.hint.as_str())).collect::<Vec<_>>(), [("Ana, Lunch", "mid:letter-1@example.org", "/mail/home/cur/1.sioul"), ("Noa, The lease", "mid:letter-2@example.org", "/mail/home/cur/2.sioul")]);
        assert_eq!(openers(&en, &two[..1]).iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), ["Open"], "one message: Open");
        let five: Vec<Letter> = (0..5).map(|i| letter(i, "Someone with a rather long name", "A subject that goes on and on")).collect();
        let open = openers(&en, &five);
        assert_eq!(open.len(), NAMED, "the letters named, no more");
        assert!(open.iter().all(|o| o.label.chars().count() <= LABEL && o.label.ends_with('…')), "{open:?}");
        let nameless = [Letter { address: String::new(), ..letter(1, "Ana", "Lunch") }, letter(2, "Noa", "The lease")];
        assert_eq!(openers(&en, &nameless).iter().map(|o| o.address.as_str()).collect::<Vec<_>>(), ["mid:letter-2@example.org"], "no address, no opener");
        assert!(openers(&en, &[Letter { address: String::new(), ..letter(1, "Ana", "Lunch") }]).is_empty());
        let fr = Translator::new("fr");
        assert_eq!(openers(&fr, &two[..1])[0].label, "Ouvrir");
    }

    /// Mail that reached its server before the watch of the moment began
    /// waited: announced as waited, never as new (the notifications' phase 2;
    /// review of phase 2, L1: a moment that began between two looks counts
    /// from the last look before it).
    #[test]
    fn mail_from_before_the_watch_waited() {
        let mut watch = Watch::default();
        // A first look at 08:30 (Unix 30_600 in this test's clock): the watch begins then.
        assert_eq!(watch.look("work", 30_600), 30_600);
        assert_eq!(watch.look("work", 30_660), 30_600, "the same moment, a minute on");
        assert_eq!(watch.look("work", 30_660 + 30 * 60), 30_600, "half an hour on: a phone's step late");
        // The night's mail, fetched after a computer was off: it waited.
        let at = |arrived: i64| Letter { key: arrived.to_string(), sender: "Ana".into(), subject: "Lunch".into(), through: false, address: String::new(), notice: String::new(), file: String::new(), arrived, message: format!("id:{arrived}") };
        assert!(any_waited(&[at(30_700), at(7_200)], 30_600));
        assert!(!any_waited(&[at(30_700), at(0)], 30_600), "arrived since, or when unknown: new");
        assert!(!any_waited(&[at(30_600 - LATE + 1)], 30_600), "a minute before the watch began: new still");
        // Another moment, looked at within GAP: it began after the last look (32_460), so what
        // came since then is new in it; what came before it waited.
        assert_eq!(watch.look("leisure", 32_700), 32_460);
        assert!(!any_waited(&[at(32_500)], 32_460) && any_waited(&[at(30_000)], 32_460));
        // A gap longer than GAP (asleep, suspended, nothing looked), or a clock gone back: a watch anew.
        assert_eq!(watch.look("leisure", 32_700 + GAP + 1), 32_700 + GAP + 1);
        assert_eq!(watch.look("leisure", 30_000), 30_000);
        // Kept on the device: a process started again goes on with the watch, when recent; a
        // minute's look rewrites nothing; the moment changing does.
        let path = std::env::temp_dir().join(format!("sioul-mail-watch-{}.toml", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(Watch::look_kept(&path, "work", 30_600), 30_600);
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("format = 1"), "{written}");
        assert_eq!(Watch::look_kept(&path, "work", 30_660), 30_600);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), written, "a minute on: not written again");
        assert_eq!(Watch::look_kept(&path, "work", 30_600 + 20 * 60), 30_600, "a new process twenty minutes on: the same watch");
        assert_eq!(Watch::look_kept(&path, "leisure", 30_600 + 21 * 60), 30_600 + 20 * 60);
        assert_eq!(Watch::look_kept(&path, "leisure", 30_600 + 21 * 60 + GAP + 1), 30_600 + 21 * 60 + GAP + 1, "off for long: anew");
        std::fs::write(&path, "format = 9\nmoment = \"x\"\nsince = 5\nlast = 5\n").unwrap();
        assert_eq!(Watch::load(&path), Watch::default(), "another format reads as none");
        let _ = std::fs::remove_file(&path);
    }

    /// One message sent to two of your addresses is told once (review of
    /// phase 2, M5): in one fetch, its second copy never; told at one address,
    /// its copy waiting at another is never told, and goes.
    #[test]
    fn one_message_to_two_addresses_is_told_once() {
        let senders = Senders { safe: SenderList::parse("ana@example.org"), ..Senders::default() };
        let usual = crate::config::Priority::default();
        let home = message("home", "Ana <ana@example.org>", "", "Dinner", "dinner-1@example.org", &senders, usual);
        let work = message("work", "Ana <ana@example.org>", "", "Dinner", "dinner-1@example.org", &senders, usual);
        assert_ne!(key(&home), key(&work));
        assert_eq!(message_key(&home), message_key(&work));
        // Both open now: told once.
        let both = sort(&[home.clone(), work.clone()], &Ledger::default(), false, |_| Fate::Now);
        assert_eq!((both.now.len(), both.never.clone()), (1, vec![key(&work)]));
        // The work address closed: the home copy now, the work copy waits.
        let split = sort(&[home.clone(), work.clone()], &Ledger::default(), false, |t| if t.card.account.as_deref() == Some("home") { Fate::Now } else { Fate::Later });
        assert_eq!((split.now.len(), split.later.clone()), (1, vec![key(&work)]));
        // The home copy told: at the work address's hours, nothing waited, and its mark goes.
        let mut ledger = Ledger::default();
        ledger.tell(&key(&home), 100);
        ledger.told_message(&message_key(&home), 100);
        ledger.wait(&key(&work), 100);
        assert!(waited(&[home.clone(), work.clone()], &ledger, false, |_| true).is_empty());
        assert_eq!(gone_by(&[work.clone()], &ledger, false), [key(&work)]);
        // Fetched again later at a third address: never told.
        let third = message("other", "Ana <ana@example.org>", "", "Dinner", "dinner-1@example.org", &senders, usual);
        assert!(sort(&[third], &ledger, false, |_| Fate::Now).now.is_empty());
        // Both waiting, both open at once: one letter.
        let mut ledger = Ledger::default();
        ledger.wait(&key(&home), 100);
        ledger.wait(&key(&work), 100);
        assert_eq!(waited(&[home.clone(), work.clone()], &ledger, false, |_| true).len(), 1);
        // Kept and read back in format 3 (its last gathering; 2 added the messages); a format 1 ledger reads with no messages, a format 2 one with no gathering.
        let path = std::env::temp_dir().join(format!("sioul-mail-ledger-{}.toml", std::process::id()));
        let mut kept = Ledger::default();
        kept.told_message("id:0123", 5);
        kept.gathered = 1_790_000_000;
        kept.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("format = 3") && text.contains("[messages]") && text.contains("gathered = 1790000000"), "{text}");
        assert_eq!((Ledger::load(&path).messages, Ledger::load(&path).gathered), (kept.messages.clone(), kept.gathered));
        std::fs::write(&path, "format = 2\n\n[messages]\n\"id:0123\" = 5\n").unwrap();
        assert_eq!(Ledger::load(&path).gathered, 0);
        std::fs::write(&path, "[told]\nabc = 5\n").unwrap();
        let older = Ledger::load(&path);
        assert!(older.known("abc") && older.messages.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    /// What a device told travels to the others in its told log; a device
    /// reads the others', never its own, within their time.
    #[test]
    fn what_one_device_told_another_reads() {
        let folder = std::env::temp_dir().join(format!("sioul-told-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let (phone, desk) = ("7c41d09e-0000-4000-8000-000000000001", "0b5e11a2-0000-4000-8000-000000000002");
        let day = 86_400_000;
        say_told(&folder, phone, &["mail:home:a".into(), "mail:home:b".into()], 10 * day).unwrap();
        say_told(&folder, desk, &["mail:home:c".into()], 10 * day).unwrap();
        assert_eq!(told_elsewhere(&folder, desk, 10 * day + 60_000).into_iter().collect::<Vec<_>>(), ["mail:home:a", "mail:home:b"]);
        assert_eq!(told_elsewhere(&folder, phone, 10 * day).into_iter().collect::<Vec<_>>(), ["mail:home:c"]);
        assert!(told_elsewhere(&folder, desk, 10 * day + TOLD_KEPT_MS + 1).is_empty(), "past their time");
        // Kept as long as the ledger's marks: a device away a long weekend still reads them.
        assert_eq!(told_elsewhere(&folder, desk, 14 * day).len(), 2);
        // Its own old lines taken out as it writes; nothing written for nothing told.
        say_told(&folder, phone, &["mail:home:d".into()], 41 * day).unwrap();
        say_told(&folder, phone, &[], 41 * day).unwrap();
        let own = std::fs::read_to_string(told_file(&folder, phone)).unwrap();
        assert_eq!(own.lines().count(), 1, "{own}");
        // Never read as the notices' words: a folder of its own.
        assert!(crate::notices::States::read(&folder).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The notices of Sioul's own one device told, in the same log (the
    /// owner's yes of 26 November 2026: a notice told on one device is
    /// dropped on the others): read by the others from a time on, never by
    /// the device itself, never as mail told; a device on 608d8521 beside
    /// reads such a line as a line that told no mail (its `ToldLine` has `at`
    /// and `told` alone, and serde passes over the rest), and its own lines
    /// still read here.
    #[test]
    fn a_notice_one_device_told_is_known_to_the_others() {
        let folder = std::env::temp_dir().join(format!("sioul-told-notices-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let (phone, desk) = ("7c41d09e-0000-4000-8000-000000000001", "0b5e11a2-0000-4000-8000-000000000002");
        let minute = 60_000;
        let night = "3f1c2a9d0b7e6a51".to_string();
        say_notices_told(&folder, phone, std::slice::from_ref(&night), 100 * minute).unwrap();
        say_notices_told(&folder, phone, &[], 100 * minute).unwrap();
        assert_eq!(notices_told_elsewhere(&folder, desk, 90 * minute).into_iter().collect::<Vec<_>>(), [night.clone()]);
        assert!(notices_told_elsewhere(&folder, desk, 101 * minute).is_empty(), "told before the time asked");
        assert!(notices_told_elsewhere(&folder, phone, 0).is_empty(), "never its own");
        assert!(told_elsewhere(&folder, desk, 100 * minute).is_empty(), "never as mail told");
        // As 608d8521 reads the phone's line: `at` and `told`, nothing told.
        #[derive(serde::Deserialize)]
        struct Older {
            #[allow(dead_code)]
            at: i64,
            #[serde(default)]
            told: Vec<String>,
        }
        let line = std::fs::read_to_string(told_file(&folder, phone)).unwrap();
        let older: Older = serde_json::from_str(line.lines().next().unwrap()).unwrap();
        assert!(older.told.is_empty(), "{line}");
        // A line 608d8521 writes (mail alone) reads here as before, no notice in it.
        std::fs::write(told_file(&folder, desk), "{\"at\":6000000,\"told\":[\"mail:home:a\"]}\n").unwrap();
        assert_eq!(told_elsewhere(&folder, phone, 100 * minute).into_iter().collect::<Vec<_>>(), ["mail:home:a"]);
        assert!(notices_told_elsewhere(&folder, phone, 0).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Where the person said what each device is for: a friend's mail to
    /// the personal address in the evening comes forward on the phone; on
    /// the work computer it waits in the ledger, shown on its Porch, never
    /// told there while the computer is for work alone. Read as the phone
    /// (the keeper of the notices: `Lines::mine` its cells), the computer
    /// finds it told now and claims it without a word (docs/areas.md, "Which
    /// device tells"). The moment's words are as they were on a device
    /// nothing is said of, and carry the cell read and what no device holds
    /// on one that narrows.
    #[test]
    fn mail_waits_on_a_device_that_is_not_for_its_address() {
        use crate::areas::{Area, Cells, Line, Lines, Other};
        use crate::attention::{Attention, Event, Lane as MailLane, Level, Now, Source};
        let senders = Senders { safe: crate::porch::SenderList::parse("ana@example.org"), ..Senders::default() };
        let ana = message("home", "Ana <ana@example.org>", "", "Dinner on Friday", "21@example.org", &senders, crate::config::Priority::default());
        let attention = Attention::usual();
        let (computer, phone) = (Line::for_area(Area::WORK), Line::for_area(Area::PERSONAL));
        let evening = |lines: Lines| mode(Time::Leisure, Why::Evening).on(&lines);
        let decided = |t: &Triaged, m: &Mode| attention.decide(&Event::of(Source::Mail { who: senders.who_of(&t.card), circle: false, always: false, lane: MailLane::Usual }).for_area(Some(Area::PERSONAL)), &Now::of(m));
        let fate = |m: Mode| {
            let decided = &decided;
            move |t: &Triaged| match decided(t, &m) {
                out if out.told => Fate::Now,
                out if out.level == Level::Never => Fate::Never,
                _ => Fate::Later,
            }
        };
        let on_phone = sort(&[ana.clone()], &Ledger::default(), false, fate(evening(Lines::of(phone, &[computer]))));
        assert_eq!(on_phone.now.iter().map(|l| l.sender.as_str()).collect::<Vec<_>>(), ["Ana"]);
        let on_computer = sort(&[ana.clone()], &Ledger::default(), false, fate(evening(Lines::of(computer, &[phone]))));
        assert_eq!((on_computer.now.len(), on_computer.later.clone(), on_computer.never.len()), (0, vec![key(&ana)], 0), "it waits there, shown on the Porch");
        // The computer read as the phone, which keeps the notices: told there now, so claimed here.
        let week = Week { work_hours: true, admin_hours: false, meals: true, sleep: true };
        let as_phone = Lines { mine: Cells::of(phone, week), key: None, others: vec![Other { cells: Cells::Line(computer), acts_on: None }] };
        assert_eq!(sort(&[ana.clone()], &Ledger::default(), false, fate(evening(as_phone))).now.len(), 1);
        // It never comes on the computer while it is for work alone: no time of its week tells it.
        let mut ledger = Ledger::default();
        ledger.wait(&key(&ana), 100);
        for (time, why) in [(Time::Work, Why::Working), (Time::Leisure, Why::Evening), (Time::Meals, Why::Meal)] {
            let there = mode(time, why).on(&Lines::of(computer, &[phone]));
            assert!(waited(&[ana.clone()], &ledger, false, |t| decided(t, &there).told).is_empty(), "{time:?}");
        }
        // The moment: as it was with nothing said; with the cell read and what no device holds on a device that narrows.
        let plain = mode(Time::Leisure, Why::Evening);
        assert_eq!(moment(&Now::of(&plain), true), "leisure:leisure:false:false:false:true");
        assert_eq!(moment(&Now::of(&plain.clone().on(&Lines::of(Line::default(), &[computer]))), true), "leisure:leisure:false:false:false:true");
        assert_eq!(moment(&Now::of(&evening(Lines::of(computer, &[phone]))), true), "leisure:leisure:false:false:false:true:none:none");
        assert_eq!(moment(&Now::of(&evening(Lines::of(phone, &[computer]))), true), "leisure:leisure:false:false:false:true:leisure:none");
        assert_eq!(moment(&Now::of(&evening(Lines::of(computer, &[]))), true), "leisure:leisure:false:false:false:true:leisure:admin+leisure", "the phone no longer counts: leisure comes here");
    }

    #[test]
    fn the_matrix_and_the_moment() {
        use crate::attention::{Attention, Event, Lane as MailLane, Level, Now, Source};
        let senders = Senders::default();
        let usual = crate::config::Priority::default();
        let stranger = message("work", "Someone <someone@elsewhere.example>", "", "A question", "10@elsewhere.example", &senders, usual);
        let attention = Attention::usual();
        let decided = |t: &Triaged, m: &Mode| attention.decide(&Event::of(Source::Mail { who: senders.who_of(&t.card), circle: false, always: false, lane: MailLane::Usual }).for_area(Some(Area::WORK)), &Now::of(m));
        let fate_at = |m: Mode| {
            let decided = &decided;
            move |t: &Triaged| match decided(t, &m) {
                out if out.told => Fate::Now,
                out if out.level == Level::Never => Fate::Never,
                _ => Fate::Later,
            }
        };
        // A stranger writing to a work address: told in work time, waiting in the evening.
        assert_eq!(sort(&[stranger.clone()], &Ledger::default(), false, fate_at(mode(Time::Work, Why::Working))).now.len(), 1);
        assert_eq!(sort(&[stranger.clone()], &Ledger::default(), false, fate_at(mode(Time::Leisure, Why::Evening))).later.len(), 1);
        // A safe sender writing to a work address in the evening (decision 2 of 13 November 2026):
        // it waits, shown on the Porch, and comes with "The Porch opens" once work begins.
        let safe_senders = Senders { safe: crate::porch::SenderList::parse("ana@example.org"), ..Senders::default() };
        let ana = message("work", "Ana <ana@example.org>", "", "The invoice", "11@example.org", &safe_senders, usual);
        let to_work = |t: &Triaged, m: &Mode| attention.decide(&Event::of(Source::Mail { who: safe_senders.who_of(&t.card), circle: false, always: false, lane: MailLane::Usual }).for_area(Some(Area::WORK)), &Now::of(m));
        let evening = to_work(&ana, &mode(Time::Leisure, Why::Evening));
        assert!(evening.shown && !evening.told, "{evening:?}");
        let fate_of = |out: crate::attention::Output| if out.told { Fate::Now } else if out.level == Level::Never { Fate::Never } else { Fate::Later };
        let sorted = sort(&[ana.clone()], &Ledger::default(), false, |t| fate_of(to_work(t, &mode(Time::Leisure, Why::Evening))));
        assert_eq!((sorted.now.len(), sorted.later.clone()), (0, vec![key(&ana)]));
        let mut ledger = Ledger::default();
        ledger.wait(&key(&ana), 100);
        assert!(waited(&[ana.clone()], &ledger, false, |t| to_work(t, &mode(Time::Leisure, Why::Evening)).told).is_empty(), "not in the evening");
        let morning = waited(&[ana.clone()], &ledger, false, |t| to_work(t, &mode(Time::Work, Why::Working)).told);
        assert_eq!(opens(&Translator::new("en"), &morning).0, "The Porch opens: one e-mail waits for you.");
        // As usual, sleep, the pause and Free time hold the telling of a safe sender's mail; work, the evening and meals do not.
        let safe_told = |m: &Mode| attention.decide(&Event::of(Source::Mail { who: crate::reach::Who::Safe, circle: false, always: false, lane: MailLane::Usual }), &Now::of(m)).told;
        assert!(!safe_told(&mode(Time::Sleep, Why::Sleep)) && !safe_told(&mode(Time::Sleep, Why::Paused)) && !safe_told(&mode(Time::Leisure, Why::FreeTime)));
        assert!(safe_told(&mode(Time::Work, Why::Working)) && safe_told(&mode(Time::Leisure, Why::Evening)) && safe_told(&mode(Time::Meals, Why::Meal)));
        // A time beginning is a change of moment; the same moment twice is none.
        let at = |time: Time, why: Why| Now::of(&mode(time, why));
        let evening = moment(&at(Time::Leisure, Why::Evening), true);
        assert_eq!(evening, moment(&at(Time::Leisure, Why::Evening), true));
        assert_ne!(evening, moment(&at(Time::Work, Why::Working), true));
        assert_ne!(moment(&at(Time::Sleep, Why::Sleep), false), moment(&at(Time::Leisure, Why::Evening), true), "waking");
        assert_ne!(moment(&at(Time::Work, Why::Working).layers(false, true), true), moment(&at(Time::Work, Why::Working), true), "a do-not-disturb ends");
        assert_ne!(moment(&at(Time::Work, Why::Working).layers(true, false), true), moment(&at(Time::Work, Why::Working), true), "a slot of time for you ends");
        // Kept and read back; old marks forgotten.
        let path = std::env::temp_dir().join(format!("sioul-mail-notified-{}.toml", std::process::id()));
        let mut ledger = Ledger::default();
        ledger.tell("a", 1_000);
        ledger.wait("b", 5_000_000);
        ledger.save(&path).unwrap();
        let mut back = Ledger::load(&path);
        assert_eq!(back, Ledger { format: LEDGER_FORMAT, ..ledger.clone() }, "written in its format");
        back.forget_old(1_000 + KEPT);
        assert!(!back.known("a") && back.known("b"));
        let _ = std::fs::remove_file(&path);
    }
}

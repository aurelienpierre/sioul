// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Do-not-disturb on every device (docs/do-not-disturb.md): why it holds now,
//! the switch pressed on any device, and the people who may reach you then.
//!
//! - **The reasons**, each read from the one place that already says it, and
//!   shared as it is: the pause and Free time (`quiet.toml`), the night and
//!   naps (Health), a focus session (`time/running.toml`), and the switch
//!   (`state/do-not-disturb.toml`, the only new record). Every device reads
//!   the same files and comes to the same answer. The latest change wins: the
//!   switch pressed off after a reason began holds that reason off until it
//!   ends; a reason beginning after the press holds again.
//! - **The switch**: each device writes its own table only, its last press
//!   stamped with a hybrid clock (never before a press already known), so a
//!   press made after seeing another always comes after it, whatever the
//!   clocks. Two presses that did not know of each other, less than a minute
//!   apart: "on" wins, the quieter choice; one more press settles it.
//! - **The list**, "Who may reach you during do-not-disturb"
//!   (`config/dnd-people.toml`): names, numbers and addresses, kept in the
//!   sharing itself, since the address books of two devices may not be in step.

use crate::config::{config_dir, state_dir};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The switch's file, in the state folder; shared with the settings ("state/do-not-disturb.toml").
pub const SWITCH_FILE: &str = "do-not-disturb.toml";
/// The list's file, in the configuration folder; shared with the senders ("config/dnd-people.toml").
pub const PEOPLE_FILE: &str = "dnd-people.toml";
/// Two presses on two devices closer than this (milliseconds), neither knowing
/// of the other: "on" wins. Clocks of devices that set their time from the
/// network differ by less than a second; this covers a clock left a little off.
pub const MARGIN_MS: i64 = 60_000;
/// A focus session holds do-not-disturb this long past the time chosen
/// (seconds): a session forgotten never keeps you unreachable for long.
pub const FOCUS_GRACE: i64 = 30 * 60;
/// …and a session without a time chosen, this long (Sioul's open-ended ceiling).
pub const FOCUS_OPEN: i64 = 180 * 60;
/// Other devices heard from within this many days count for "on every device".
pub const LIVE_DAYS: i64 = 7;

fn yes() -> bool {
    true
}

/// What turns do-not-disturb on (`[dnd]` in the configuration, shared with the settings).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DndSettings {
    /// The switch in the status line.
    #[serde(default = "yes")]
    pub button: bool,
    /// While a focus session counts.
    #[serde(default)]
    pub focus: bool,
    /// During the pause and Free time: the system's do-not-disturb too, beside
    /// Sioul's own notifications, which the pauses hold either way.
    #[serde(default = "yes")]
    pub pauses: bool,
    /// While you sleep: the night from winding down, and naps.
    #[serde(default)]
    pub sleep: bool,
    /// The people on the list get through (on a phone, as starred contacts);
    /// otherwise nobody (repeat callers neither).
    #[serde(default = "yes")]
    pub people: bool,
    /// Android: this device keeps in step in the background (its own, never shared).
    #[serde(default = "yes")]
    pub background: bool,
}

impl Default for DndSettings {
    fn default() -> Self {
        DndSettings { button: true, focus: false, pauses: true, sleep: false, people: true, background: true }
    }
}

/// Why do-not-disturb holds, in the order the first one is said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    Paused,
    FreeTime,
    Sleep,
    Focus,
    Manual,
}

impl Why {
    pub fn id(self) -> &'static str {
        match self {
            Why::Paused => "paused",
            Why::FreeTime => "free-time",
            Why::Sleep => "sleep",
            Why::Focus => "focus",
            Why::Manual => "manual",
        }
    }

    pub fn parse(id: &str) -> Option<Why> {
        [Why::Paused, Why::FreeTime, Why::Sleep, Why::Focus, Why::Manual].into_iter().find(|w| w.id() == id)
    }

    /// Silenced by the switch's own mode (sleep, focus, the switch), not a pause's.
    pub fn global(self) -> bool {
        matches!(self, Why::Sleep | Why::Focus | Why::Manual)
    }
}

// ---------------------------------------------------------------- the switch

/// One device's table in the switch's file, written by that device alone.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// "phone" or "computer".
    #[serde(default)]
    pub kind: String,
    /// A computer's host name; "" for a phone.
    #[serde(default)]
    pub name: String,
    /// Its last press of the switch (milliseconds, hybrid clock); 0: never pressed there.
    #[serde(default)]
    pub pressed: i64,
    /// What that press asked.
    #[serde(default)]
    pub on: bool,
    /// The end chosen with it (milliseconds); 0: until turned off.
    #[serde(default)]
    pub until: i64,
    /// The latest press of another device known here when pressing (milliseconds).
    #[serde(default)]
    pub seen: i64,
    /// What holds here now, as this device reads the reasons ("" none).
    #[serde(default)]
    pub why: String,
    /// Its system's do-not-disturb is on for it.
    #[serde(default)]
    pub silenced: bool,
    /// Its system's answer, in its words (the do-not-disturb layer's report).
    #[serde(default)]
    pub line: String,
    /// When this table was written (milliseconds).
    #[serde(default)]
    pub at: i64,
    /// What a later version writes here, kept as it is: written back unchanged.
    #[serde(flatten)]
    pub other: toml::Table,
}

/// The switch's file: a table per device.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Switch {
    #[serde(default)]
    pub device: BTreeMap<String, Device>,
    /// What a later version writes here, kept as it is.
    #[serde(flatten)]
    pub other: toml::Table,
}

/// The press that holds, as every device reads the switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Press {
    /// Milliseconds, the pressing device's hybrid clock.
    pub at: i64,
    pub on: bool,
    /// Milliseconds; 0: until turned off.
    pub until: i64,
    /// The device it was pressed on.
    pub from: String,
}

impl Switch {
    pub fn default_path() -> PathBuf {
        state_dir().join(SWITCH_FILE)
    }

    /// The file as it is; none there is no press anywhere yet. One that does
    /// not read is an error: never taken for no device at all.
    pub fn read(path: &Path) -> Result<Switch, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Switch::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// For a question (the status line, an apply): what cannot be read is no press.
    pub fn load(path: &Path) -> Switch {
        Switch::read(path).unwrap_or_default()
    }

    /// The latest press anywhere wins; one that did not know of an opposite
    /// press made on another device less than `MARGIN_MS` before it gives way
    /// to it if that one was "on".
    pub fn latest(&self) -> Option<Press> {
        let presses: Vec<(&String, &Device)> = self.device.iter().filter(|(_, d)| d.pressed > 0).collect();
        let (from, last) = presses.iter().max_by_key(|(id, d)| (d.pressed, (*id).clone()))?;
        let mut winner = (*from, *last);
        if !last.on {
            // An "on" that this "off" did not know of, close before it: a tie, "on" wins.
            let tied = presses.iter().filter(|(id, d)| *id != *from && d.on && last.pressed - d.pressed <= MARGIN_MS && last.seen < d.pressed).max_by_key(|(id, d)| (d.pressed, (*id).clone()));
            if let Some((id, d)) = tied {
                winner = (*id, *d);
            }
        }
        Some(Press { at: winner.1.pressed, on: winner.1.on, until: winner.1.until, from: winner.0.clone() })
    }

    /// The latest press known here, of any device but `except` (milliseconds).
    pub fn latest_known(&self, except: Option<&str>) -> i64 {
        self.device.iter().filter(|(id, _)| Some(id.as_str()) != except).map(|(_, d)| d.pressed).max().unwrap_or(0)
    }

    /// A press here, stamped after every press known (a hybrid clock): `until`
    /// in milliseconds, 0 for none. Returns its stamp.
    pub fn press(&mut self, here: &str, on: bool, until: i64, now_ms: i64) -> i64 {
        let stamp = now_ms.max(self.latest_known(None) + 1);
        let seen = self.latest_known(Some(here));
        let own = self.device.entry(here.to_string()).or_default();
        own.pressed = stamp;
        own.on = on;
        own.until = if on { until.max(0) } else { 0 };
        own.seen = seen;
        own.at = stamp;
        stamp
    }

    /// Written whole, beside it then put in place: never half a file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        write_whole(path, &toml::to_string(self).map_err(|e| e.to_string())?)
    }
}

/// This device's own table changed under the file's lock, the file read
/// again first: the other devices' tables, brought by the sharing meanwhile,
/// stay as they are. `change` says whether it changed anything; nothing is
/// written otherwise. A file that does not read is left alone, an error.
pub fn change_own(path: &Path, here: &str, change: impl FnOnce(&mut Switch) -> bool) -> Result<Switch, String> {
    crate::filelock::with_lock(path, || {
        let mut switch = Switch::read(path)?;
        if !here.is_empty() && change(&mut switch) {
            // Only this device's table may have changed: the others' are never written from here.
            switch.save(path)?;
        }
        Ok(switch)
    })
}

/// A file written beside its place under a hidden name, then renamed.
fn write_whole(path: &Path, text: &str) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let fresh = path.with_file_name(format!(".{name}.{}.new", std::process::id()));
    std::fs::write(&fresh, text).map_err(fail)?;
    std::fs::rename(&fresh, path).map_err(|e| {
        let _ = std::fs::remove_file(&fresh);
        fail(e)
    })
}

// ---------------------------------------------------------------- the reasons

/// What each reason's source says now (Unix seconds).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// The pause, since.
    pub paused: Option<i64>,
    /// Free time: since, and when it ends by itself (the night).
    pub free: Option<(i64, i64)>,
    /// Asleep, the night from winding down or a nap: since, until waking.
    pub sleep: Option<(i64, i64)>,
    /// A focus session counting: its start, and when it stops holding do-not-disturb (`focus`).
    pub focus: Option<(i64, i64)>,
}

/// A focus session as do-not-disturb reads it: counting (not paused), from
/// its start until 30 minutes past the time chosen (three hours without one);
/// none otherwise.
pub fn focus(running: &crate::timelog::Running, now: i64) -> Option<(i64, i64)> {
    if running.paused_at.is_some() || running.start <= 0 {
        return None;
    }
    let lasts = if running.planned > 0 { i64::from(running.planned) * 60 + FOCUS_GRACE } else { FOCUS_OPEN };
    let end = running.start + running.paused.max(0) + lasts;
    (now < end).then_some((running.start, end))
}

/// A reason that holds now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Held {
    pub why: Why,
    /// Unix seconds.
    pub since: i64,
    /// When it ends by itself (Unix seconds): the switch's end, waking, Free
    /// time's end, a focus session's limit; none for the pause and an open switch.
    pub until: Option<i64>,
    /// The device the switch was pressed on; "" for the other reasons.
    pub from: String,
}

/// Why do-not-disturb holds now: every reason that does, the one said first first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Now {
    pub holds: Vec<Held>,
}

impl Now {
    pub fn on(&self) -> bool {
        !self.holds.is_empty()
    }

    /// The reason said: the first that holds.
    pub fn why(&self) -> Option<Why> {
        self.holds.first().map(|h| h.why)
    }

    pub fn has(&self, why: Why) -> bool {
        self.holds.iter().any(|h| h.why == why)
    }

    /// Whether the switch's own mode holds (sleep, focus, the switch), beside the pauses'.
    pub fn global(&self) -> bool {
        self.holds.iter().any(|h| h.why.global())
    }

    /// The end said: when every reason that holds ends by itself, the last of
    /// them; a focus session's limit is never said (it is no promise).
    pub fn until(&self) -> Option<i64> {
        if self.holds.is_empty() || self.holds.iter().any(|h| h.until.is_none() || h.why == Why::Focus) {
            return None;
        }
        self.holds.iter().filter_map(|h| h.until).max()
    }

    /// The next time a reason that holds ends by itself (Unix seconds), to look again then.
    pub fn next_end(&self) -> Option<i64> {
        self.holds.iter().filter_map(|h| h.until).min()
    }

    /// The first reason that holds and is not a pause's (sleep, focus, the switch).
    pub fn global_why(&self) -> Option<Why> {
        self.holds.iter().map(|h| h.why).find(|w| w.global())
    }

    /// Whether Sioul's own notifications wait and mail comes only from the
    /// list: the switch or a focus session (sleep and the pauses keep their own
    /// rules, under which nothing is told).
    pub fn gates(&self) -> bool {
        self.has(Why::Focus) || self.has(Why::Manual)
    }
}

/// Why do-not-disturb holds at `now` (Unix seconds), from the reasons'
/// sources, the settings and the switch's latest press. A press "off" made
/// after a reason began holds that reason off until it ends; the pause first,
/// then Free time (never with the pause), sleep, focus, the switch.
pub fn now(settings: &DndSettings, sources: &Sources, press: Option<&Press>, now: i64) -> Now {
    // Pressed off after it began: held off. A press "on" never holds a reason off.
    let pressed_off_after = |since: i64| press.is_some_and(|p| !p.on && p.at > since.saturating_mul(1000));
    let mut holds = Vec::new();
    let mut add = |why: Why, since: i64, until: Option<i64>| {
        if !pressed_off_after(since) {
            holds.push(Held { why, since, until, from: String::new() });
        }
    };
    if settings.pauses {
        if let Some(since) = sources.paused {
            add(Why::Paused, since, None);
        } else if let Some((since, end)) = sources.free.filter(|(since, end)| *since <= now && now < *end) {
            add(Why::FreeTime, since, Some(end));
        }
    }
    if settings.sleep
        && let Some((since, until)) = sources.sleep.filter(|(_, until)| now < *until)
    {
        add(Why::Sleep, since, Some(until));
    }
    if settings.focus
        && let Some((since, end)) = sources.focus.filter(|(_, end)| now < *end)
    {
        add(Why::Focus, since, Some(end));
    }
    if let Some(p) = press.filter(|p| p.on) {
        let until = (p.until > 0).then_some(p.until / 1000);
        if until.is_none_or(|u| now < u) {
            holds.push(Held { why: Why::Manual, since: p.at / 1000, until, from: p.from.clone() });
        }
    }
    holds.sort_by_key(|h| h.why);
    Now { holds }
}

// ---------------------------------------------------------------- where it holds

/// How another device follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Follows {
    /// Do-not-disturb holds there and its system is silenced.
    Silenced,
    /// It holds there, but its system could not be silenced: its own words.
    Cannot(String),
    /// Its news has not come since do-not-disturb began: on its way, at the
    /// sharing's pace (about a minute with a Nextcloud server).
    Behind,
    /// It runs a Sioul that does not know do-not-disturb: no entry in the
    /// devices' registry, no table: it cannot follow until it is updated.
    Older,
}

/// Each other device that counts (heard from in the last `LIVE_DAYS` days, by
/// the sharing: `heard`, its id and when, Unix seconds), and how it follows
/// while do-not-disturb holds here, since `since` (milliseconds): a table
/// written before that (by more than `MARGIN_MS`, for clocks that disagree)
/// says what held there before, not now. `registered` says whether a device
/// has its entry in the devices' registry (a Sioul that knows do-not-disturb).
pub fn others(switch: &Switch, here: &str, heard: &[(String, i64)], now: i64, since: i64, registered: &dyn Fn(&str) -> bool) -> Vec<(String, Follows)> {
    let live = |at: i64| now - at <= LIVE_DAYS * 86_400;
    let mut ids: Vec<String> = heard.iter().filter(|(id, at)| id != here && live(*at)).map(|(id, _)| id.clone()).collect();
    // A device whose table here is recent counts too, even if the sharing's notes lag.
    ids.extend(switch.device.iter().filter(|(id, d)| *id != here && d.at > 0 && live(d.at / 1000)).map(|(id, _)| id.clone()));
    ids.sort();
    ids.dedup();
    ids.into_iter()
        .map(|id| {
            let follows = match switch.device.get(&id) {
                Some(d) if d.why.is_empty() || d.at.saturating_add(MARGIN_MS) < since => Follows::Behind,
                Some(d) if d.silenced => Follows::Silenced,
                Some(d) => Follows::Cannot(d.line.clone()),
                None if !registered(&id) => Follows::Older,
                None => Follows::Behind,
            };
            (id, follows)
        })
        .collect()
}

/// What the status line and the details say of do-not-disturb.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Said {
    /// One sentence: "Do not disturb, on every device, until 15:00."; "" when off.
    pub line: String,
    /// Why, in a few words: "While you focus on a task."
    pub why: String,
    /// A line per device: this one first, then the others.
    pub details: Vec<String>,
    /// Every device that counts follows, and this one is silenced.
    pub everywhere: bool,
    /// This device's system is silenced.
    pub here: bool,
}

/// The words of do-not-disturb as it holds now: where (`here_silenced` and
/// `here_line`, this device's system's answer; `others`, as `others` says),
/// until when (`until`, already worded: "15:00", "tomorrow at 07:00"), why.
/// `name_of` names a device by its id, inside a sentence ("your phone", a
/// computer's name, "another device").
pub fn said(now_: &Now, here_silenced: bool, here_line: &str, others: &[(String, Follows)], until: &str, name_of: &dyn Fn(&str) -> String, tr: &crate::i18n::Translator) -> Said {
    let Some(why) = now_.why() else { return Said::default() };
    let mut args = crate::i18n::args();
    args.set("until", until.to_string());
    let with_until = |key: &str| if until.is_empty() { tr.text(key, None) } else { tr.text(&format!("{key}-until"), Some(&args)) };
    let following = others.iter().filter(|(_, f)| *f == Follows::Silenced).count();
    let everywhere = here_silenced && following == others.len();
    // Every device not silenced yet is one whose news has not come: on its way,
    // not left out. One that cannot be silenced, or an older Sioul, is said as such.
    let not_following: Vec<&Follows> = others.iter().map(|(_, f)| f).filter(|f| **f != Follows::Silenced).collect();
    let waiting = !not_following.is_empty() && not_following.iter().all(|f| **f == Follows::Behind);
    let line = if !here_silenced {
        if following > 0 { with_until("dnd-elsewhere") } else { with_until("dnd-own-only") }
    } else if others.is_empty() {
        with_until("dnd-here-only")
    } else if everywhere {
        with_until("dnd-everywhere")
    } else if waiting {
        with_until("dnd-waiting")
    } else if following == 0 {
        with_until("dnd-here-only")
    } else {
        with_until("dnd-not-everywhere")
    };
    // "Here: silenced.", "On your phone: waiting for its news."
    let device = |key: &str, place: String, line: Option<&str>| {
        let mut args = crate::i18n::args();
        args.set("device", place);
        if let Some(line) = line {
            args.set("line", line.to_string());
        }
        tr.text(key, Some(&args))
    };
    let on = |id: &str| {
        let mut args = crate::i18n::args();
        args.set("name", name_of(id));
        tr.text("dnd-device-on", Some(&args))
    };
    let here = tr.text("dnd-device-here", None);
    let mut details = vec![if here_silenced { device("dnd-device-silenced", here, None) } else { device("dnd-device-cannot", here, Some(here_line)) }];
    for (id, follows) in others {
        details.push(match follows {
            Follows::Silenced => device("dnd-device-silenced", on(id), None),
            Follows::Cannot(words) if !words.is_empty() => device("dnd-device-cannot", on(id), Some(words)),
            Follows::Cannot(_) => device("dnd-device-unsilenced", on(id), None),
            Follows::Behind => device("dnd-device-behind", on(id), None),
            Follows::Older => device("dnd-device-older", on(id), None),
        });
    }
    let why_words = match why {
        Why::Manual => {
            let from = now_.holds.iter().find(|h| h.why == Why::Manual).map(|h| h.from.clone()).unwrap_or_default();
            if from.is_empty() {
                tr.text("dnd-why-manual", None)
            } else {
                let mut args = crate::i18n::args();
                args.set("device", name_of(&from));
                tr.text("dnd-why-manual-from", Some(&args))
            }
        }
        other => tr.text(&format!("dnd-why-{}", other.id()), None),
    };
    Said { line, why: why_words, details, everywhere, here: here_silenced }
}

// ---------------------------------------------------------------- the list

/// Someone who may reach you during do-not-disturb.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Person {
    /// Made once, the same on every device: one entry each in the sharing.
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phones: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<String>,
    /// The contact card they came from (its UID), to find it again.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub contact: String,
    /// What a later version writes here, kept as it is: written back unchanged,
    /// so that this device never sends another version's person as changed.
    #[serde(flatten)]
    pub other: toml::Table,
}

/// "Who may reach you during do-not-disturb".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct People {
    #[serde(default, rename = "person", skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<Person>,
    /// What a later version writes here, kept as it is.
    #[serde(flatten)]
    pub other: toml::Table,
}

/// An address as compared: its address part ("Jane <jane@example.org>"), trimmed, in lower case.
fn address_key(text: &str) -> String {
    let text = text.trim();
    let inner = text.rsplit_once('<').map_or(text, |(_, rest)| rest.trim_end_matches('>'));
    inner.trim().trim_start_matches("mailto:").to_lowercase()
}

/// A new id: the time and a hash keyed at random for this process.
pub fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(now.as_nanos());
    hasher.write_u64(COUNT.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    format!("{:x}{:016x}", now.as_secs(), hasher.finish())
}

impl People {
    pub fn default_path() -> PathBuf {
        config_dir().join(PEOPLE_FILE)
    }

    /// The list as it is; none there is nobody yet. One that does not read is
    /// an error, never an empty list (an edit would otherwise take everyone out
    /// on every device).
    pub fn read(path: &Path) -> Result<People, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(People::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// For a question (a mail to tell, a number to look up): what cannot be read is nobody.
    pub fn load(path: &Path) -> People {
        People::read(path).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        write_whole(path, &toml::to_string(self).map_err(|e| e.to_string())?)
    }

    /// Whether mail from this address may come during do-not-disturb.
    pub fn admits_address(&self, address: &str) -> bool {
        let key = address_key(address);
        !key.is_empty() && key.contains('@') && self.people.iter().any(|p| p.emails.iter().any(|e| address_key(e) == key))
    }

    /// Whether this number is someone's on the list (numbers compared as
    /// `phones::key` reads them, `region` for those written without a country).
    pub fn admits_number(&self, number: &str, region: Option<&crate::phones::Region>) -> bool {
        let key = crate::phones::key(number, region);
        crate::phones::is_whole(&key) && self.people.iter().any(|p| p.phones.iter().any(|n| crate::phones::key(n, region) == key))
    }

    /// A person added, or merged into the one already there with one of their
    /// addresses, numbers or their card: their id.
    pub fn add(&mut self, mut person: Person, region: Option<&crate::phones::Region>) -> String {
        person.emails.retain(|e| !address_key(e).is_empty());
        person.phones.retain(|n| !n.trim().is_empty());
        let same = |p: &Person| {
            (!person.contact.is_empty() && p.contact == person.contact)
                || person.emails.iter().any(|e| p.emails.iter().any(|x| address_key(x) == address_key(e)))
                || person.phones.iter().any(|n| {
                    let key = crate::phones::key(n, region);
                    crate::phones::is_whole(&key) && p.phones.iter().any(|x| crate::phones::key(x, region) == key)
                })
        };
        if let Some(known) = self.people.iter_mut().find(|p| same(p)) {
            for email in person.emails {
                if !known.emails.iter().any(|x| address_key(x) == address_key(&email)) {
                    known.emails.push(email.trim().to_string());
                }
            }
            for number in person.phones {
                let key = crate::phones::key(&number, region);
                if !known.phones.iter().any(|x| crate::phones::key(x, region) == key) {
                    known.phones.push(number.trim().to_string());
                }
            }
            if known.name.trim().is_empty() {
                known.name = person.name;
            }
            if known.contact.is_empty() {
                known.contact = person.contact;
            }
            return known.id.clone();
        }
        if person.id.is_empty() {
            person.id = new_id();
        }
        person.emails = person.emails.iter().map(|e| e.trim().to_string()).collect();
        person.phones = person.phones.iter().map(|n| n.trim().to_string()).collect();
        let id = person.id.clone();
        self.people.push(person);
        id
    }

    /// Someone taken off the list; false when they were not on it.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.people.len();
        self.people.retain(|p| p.id != id);
        self.people.len() != before
    }
}

/// The list changed under its lock, read again first: what the sharing
/// brought meanwhile stays. A file that does not read is left alone, an error.
pub fn change_people(path: &Path, change: impl FnOnce(&mut People)) -> Result<People, String> {
    crate::filelock::with_lock(path, || {
        let mut people = People::read(path)?;
        let before = people.clone();
        change(&mut people);
        if people != before {
            people.save(path)?;
        }
        Ok(people)
    })
}

/// Someone from a contact card: their name, numbers and addresses.
pub fn from_contact(contact: &crate::contacts::Contact) -> Person {
    Person {
        id: String::new(),
        name: contact.name.trim().to_string(),
        phones: contact.phones.iter().map(|p| p.value.trim().to_string()).filter(|v| !v.is_empty()).collect(),
        emails: contact.emails.iter().map(|e| e.value.trim().to_string()).filter(|v| !v.is_empty()).collect(),
        contact: contact.uid.clone(),
        other: toml::Table::new(),
    }
}

/// People from the Safe list: each address, with its card's name and numbers
/// when a card has it; everyone whose card is in a category on it
/// (`category:Friends`); a card on it (`contact:<UID>`); a number on it
/// (`tel:+33…`). Patterns (`@example.org`, `*@example.org`, `tel:+3319900*`)
/// name nobody: counted, left.
pub fn from_safe(entries: &[String], contacts: &[crate::contacts::Contact]) -> (Vec<Person>, usize) {
    let (mut found, mut patterns) = (Vec::new(), 0);
    for entry in entries {
        let entry = entry.trim();
        if let Some(category) = crate::porch::category_of(entry) {
            found.extend(contacts.iter().filter(|c| crate::contacts::in_category(c, category)).map(from_contact));
        } else if let Some(uid) = crate::porch::card_of(entry) {
            found.extend(contacts.iter().filter(|c| c.uid.trim() == uid).map(from_contact));
        } else if let Some(number) = crate::porch::number_of(entry).filter(|n| !n.contains('*')) {
            found.push(Person { phones: vec![number.to_string()], ..Person::default() });
        } else if entry.starts_with('@') || !entry.contains('@') || entry.contains('*') {
            patterns += 1;
        } else {
            match crate::contacts::by_address(contacts, entry) {
                Some(card) => found.push(from_contact(card)),
                None => found.push(Person { name: String::new(), emails: vec![entry.to_string()], ..Person::default() }),
            }
        }
    }
    (found, patterns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Translator;

    fn device(pressed: i64, on: bool, seen: i64) -> Device {
        Device { pressed, on, seen, ..Device::default() }
    }

    fn switch(devices: &[(&str, Device)]) -> Switch {
        Switch { device: devices.iter().map(|(id, d)| (id.to_string(), d.clone())).collect(), ..Switch::default() }
    }

    #[test]
    fn the_latest_press_wins_whatever_device_made_it() {
        // The desktop turned it on, the phone off later having seen it: off.
        let s = switch(&[("desk", device(1_000_000, true, 0)), ("phone", device(1_200_000, false, 1_000_000))]);
        let latest = s.latest().unwrap();
        assert!(!latest.on && latest.from == "phone");
        // Then the desktop on again: on.
        let s = switch(&[("desk", device(1_300_000, true, 1_200_000)), ("phone", device(1_200_000, false, 1_000_000))]);
        assert!(s.latest().unwrap().on);
        // Nobody pressed: nothing.
        assert_eq!(Switch::default().latest(), None);
    }

    #[test]
    fn a_press_after_seeing_another_comes_after_it_whatever_the_clocks() {
        let mut s = switch(&[("phone", device(5_000_000, true, 0))]);
        // This computer's clock is two minutes behind the phone's: its press
        // still comes after the one it saw.
        let stamp = s.press("desk", false, 0, 5_000_000 - 120_000);
        assert_eq!(stamp, 5_000_001);
        assert_eq!(s.device["desk"].seen, 5_000_000);
        let latest = s.latest().unwrap();
        assert!(!latest.on && latest.from == "desk");
    }

    #[test]
    fn two_presses_that_did_not_see_each_other_within_a_minute_turn_it_on() {
        // On at the desktop, off on the phone 40 s later without having seen it: on.
        let s = switch(&[("desk", device(10_000_000, true, 0)), ("phone", device(10_040_000, false, 0))]);
        let latest = s.latest().unwrap();
        assert!(latest.on && latest.from == "desk");
        // Two minutes apart: the later one.
        let s = switch(&[("desk", device(10_000_000, true, 0)), ("phone", device(10_120_000, false, 0))]);
        assert!(!s.latest().unwrap().on);
        // Within the minute but seen: the later one.
        let s = switch(&[("desk", device(10_000_000, true, 0)), ("phone", device(10_040_000, false, 10_000_000))]);
        assert!(!s.latest().unwrap().on);
        // Off first, on after, within the minute: on, as the later.
        let s = switch(&[("desk", device(10_000_000, false, 0)), ("phone", device(10_040_000, true, 0))]);
        assert!(s.latest().unwrap().on);
    }

    #[test]
    fn until_ends_it_by_each_devices_clock() {
        let settings = DndSettings::default();
        let press = Press { at: 1_000_000, on: true, until: 1_600_000, from: "desk".into() };
        let held = now(&settings, &Sources::default(), Some(&press), 1_500);
        assert_eq!(held.why(), Some(Why::Manual));
        assert_eq!(held.until(), Some(1_600));
        assert!(!now(&settings, &Sources::default(), Some(&press), 1_600).on());
    }

    #[test]
    fn each_trigger_is_a_setting_and_the_pause_comes_first() {
        let sources = Sources { paused: Some(100), free: Some((50, 900)), sleep: Some((80, 2000)), focus: Some((90, 1000)) };
        let none = DndSettings { pauses: false, sleep: false, focus: false, ..DndSettings::default() };
        assert!(!now(&none, &sources, None, 500).on());
        let all = DndSettings { pauses: true, sleep: true, focus: true, ..DndSettings::default() };
        let held = now(&all, &sources, None, 500);
        // Free time never with the pause.
        assert_eq!(held.holds.iter().map(|h| h.why).collect::<Vec<_>>(), vec![Why::Paused, Why::Sleep, Why::Focus]);
        assert_eq!(held.why(), Some(Why::Paused));
        assert!(held.global() && held.global_why() == Some(Why::Sleep));
        // The pause holds without an end: none said.
        assert_eq!(held.until(), None);
        // Free time alone, with its end.
        let free = Sources { free: Some((50, 900)), ..Sources::default() };
        let held = now(&all, &free, None, 500);
        assert_eq!((held.why(), held.until()), (Some(Why::FreeTime), Some(900)));
        assert!(!held.global());
        assert!(!now(&all, &free, None, 900).on(), "Free time over at its end");
    }

    #[test]
    fn pressed_off_after_a_reason_began_holds_it_off_until_it_ends() {
        let settings = DndSettings { focus: true, sleep: true, ..DndSettings::default() };
        let focus = Sources { focus: Some((1_000, 5_000)), ..Sources::default() };
        let off_after = Press { at: 2_000_000, on: false, until: 0, from: "phone".into() };
        assert!(!now(&settings, &focus, Some(&off_after), 3_000).on(), "the person turned it off during the session");
        // A session begun after the press holds again.
        let later = Sources { focus: Some((2_500, 6_000)), ..Sources::default() };
        assert!(now(&settings, &later, Some(&off_after), 3_000).has(Why::Focus));
        // A press "on" never holds a reason off; an "off" before it began neither.
        let on = Press { on: true, ..off_after.clone() };
        let held = now(&settings, &focus, Some(&on), 3_000);
        assert!(held.has(Why::Focus) && held.has(Why::Manual));
        let off_before = Press { at: 500_000, ..off_after };
        assert!(now(&settings, &focus, Some(&off_before), 3_000).has(Why::Focus));
    }

    #[test]
    fn mail_and_sioul_s_own_notifications_are_gated_by_the_switch_and_focus_only() {
        let settings = DndSettings { focus: true, sleep: true, ..DndSettings::default() };
        let press = Press { at: 1_000_000, on: true, until: 0, from: "desk".into() };
        assert!(now(&settings, &Sources::default(), Some(&press), 2_000).gates());
        assert!(now(&settings, &Sources { focus: Some((1_000, 9_000)), ..Sources::default() }, None, 2_000).gates());
        // Sleep and the pauses keep their own rules.
        assert!(!now(&settings, &Sources { sleep: Some((1_000, 9_000)), ..Sources::default() }, None, 2_000).gates());
        assert!(!now(&settings, &Sources { paused: Some(1_000), ..Sources::default() }, None, 2_000).gates());
        assert!(!now(&settings, &Sources::default(), None, 2_000).gates());
    }

    #[test]
    fn a_forgotten_focus_session_lets_you_be_reached_again() {
        let mut running = crate::timelog::Running { task: "t".into(), start: 1_000, planned: 25, paused_at: None, paused: 0 };
        assert_eq!(focus(&running, 1_100), Some((1_000, 1_000 + 25 * 60 + FOCUS_GRACE)));
        assert_eq!(focus(&running, 1_000 + 25 * 60 + FOCUS_GRACE), None);
        // Its own pauses push the limit on; paused, it does not hold.
        running.paused = 600;
        assert_eq!(focus(&running, 1_100).map(|(_, end)| end), Some(1_000 + 600 + 25 * 60 + FOCUS_GRACE));
        running.paused_at = Some(1_200);
        assert_eq!(focus(&running, 1_300), None);
        // Without a time chosen: three hours.
        let open = crate::timelog::Running { task: "t".into(), start: 1_000, ..crate::timelog::Running::default() };
        assert_eq!(focus(&open, 1_100).map(|(_, end)| end), Some(1_000 + FOCUS_OPEN));
    }

    #[test]
    fn the_status_says_where_it_holds() {
        let tr = Translator::new("en");
        let settings = DndSettings::default();
        let press = Press { at: 1_000_000, on: true, until: 0, from: "phone".into() };
        let held = now(&settings, &Sources::default(), Some(&press), 2_000);
        let name = |id: &str| if id == "phone" { "your phone".to_string() } else { id.to_string() };
        let all = [("phone".to_string(), Follows::Silenced)];
        let s = said(&held, true, "", &all, "15:00", &name, &tr);
        assert_eq!(s.line, "Do not disturb, on every device, until 15:00.");
        assert!(s.everywhere && s.here);
        assert_eq!(s.why, "Turned on from your phone.");
        assert_eq!(s.details, vec!["Here: silenced.", "On your phone: silenced."]);
        let s = said(&held, true, "", &[], "", &name, &tr);
        assert_eq!(s.line, "Do not disturb, here only.");
        // A device whose news has not come yet: on its way, never "here only".
        let behind = [("phone".to_string(), Follows::Behind), ("desk".to_string(), Follows::Silenced)];
        let s = said(&held, true, "", &behind, "", &name, &tr);
        assert_eq!(s.line, "Do not disturb here; your other devices' news is on its way.");
        assert_eq!(s.details[1], "On your phone: waiting for its news.");
        let s = said(&held, true, "", &[("phone".to_string(), Follows::Behind)], "15:00", &name, &tr);
        assert_eq!(s.line, "Do not disturb here until 15:00; your other devices' news is on its way.");
        // An older Sioul cannot follow: said as such, never waited for.
        let s = said(&held, true, "", &[("phone".to_string(), Follows::Older)], "", &name, &tr);
        assert_eq!(s.line, "Do not disturb, here only.");
        assert_eq!(s.details[1], "On your phone: an older Sioul, which cannot follow until it is updated.");
        let cannot = [("phone".to_string(), Follows::Cannot("Your phone is not silenced: Sioul does not have Android's “Do Not Disturb access”.".into()))];
        let s = said(&held, true, "", &cannot, "", &name, &tr);
        assert_eq!(s.line, "Do not disturb, here only.");
        assert_eq!(s.details[1], "On your phone: Your phone is not silenced: Sioul does not have Android's “Do Not Disturb access”.");
        // This device cannot be silenced (Windows): said with its own words.
        let s = said(&held, false, "This computer's notifications cannot be silenced by Sioul.", &all, "", &name, &tr);
        assert_eq!(s.line, "Do not disturb, on your other devices; this one keeps only Sioul's own notifications back.");
        assert!(!s.everywhere);
        // Off: nothing said.
        assert_eq!(said(&Now::default(), true, "", &all, "", &name, &tr), Said::default());
        // In French, with its spaces.
        let fr = Translator::new("fr");
        let s = said(&held, true, "", &all, "15:00", &name, &fr);
        assert_eq!(s.line, "Ne pas déranger, sur tous vos appareils, jusqu’à 15:00.");
        assert_eq!(s.details[0], "Ici\u{202f}: en silence.");
        assert_eq!(s.details[1], "Sur your phone\u{202f}: en silence.");
    }

    #[test]
    fn others_count_when_heard_lately_and_follow_by_their_own_table() {
        let now_s = 1_000_000;
        let mut s = Switch::default();
        s.device.insert("phone".into(), Device { why: "manual".into(), silenced: true, at: now_s * 1000, ..Device::default() });
        s.device.insert("old".into(), Device { why: "manual".into(), silenced: true, at: (now_s - 30 * 86_400) * 1000, ..Device::default() });
        s.device.insert("laptop".into(), Device { why: "manual".into(), silenced: false, line: "Windows".into(), at: now_s * 1000, ..Device::default() });
        let heard = vec![("phone".to_string(), now_s - 60), ("tablet".to_string(), now_s - 3600), ("gone".to_string(), now_s - 10 * 86_400), ("desk".to_string(), now_s)];
        let registered = |_: &str| true;
        let followed = others(&s, "desk", &heard, now_s, (now_s - 60) * 1000, &registered);
        assert_eq!(followed, vec![("laptop".to_string(), Follows::Cannot("Windows".into())), ("phone".to_string(), Follows::Silenced), ("tablet".to_string(), Follows::Behind)]);
        // Do-not-disturb begun later than their tables were written, a minute's margin aside: their news is on its way.
        let later = others(&s, "desk", &heard, now_s, (now_s + 120) * 1000, &registered);
        assert!(later.iter().all(|(_, f)| *f == Follows::Behind), "{later:?}");
        let close = others(&s, "desk", &heard, now_s, (now_s + 30) * 1000, &registered);
        assert!(close.iter().any(|(_, f)| *f == Follows::Silenced), "{close:?}");
        // A device with no table and no entry in the devices' registry runs an older Sioul: it cannot follow.
        let older = others(&s, "desk", &heard, now_s, (now_s - 60) * 1000, &|id: &str| id != "tablet");
        assert!(older.contains(&("tablet".to_string(), Follows::Older)), "{older:?}");
    }

    #[test]
    fn only_this_devices_table_is_written_and_a_broken_file_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("sioul-everywhere-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(SWITCH_FILE);
        // Another device's table, as the sharing brought it.
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "[device.\"phone-1\"]\nkind = \"phone\"\npressed = 7\non = true\n").unwrap();
        let s = change_own(&path, "desk-1", |s| {
            s.press("desk-1", false, 0, 1_000);
            true
        })
        .unwrap();
        assert_eq!(s.device.len(), 2);
        let again = Switch::read(&path).unwrap();
        assert_eq!(again.device["phone-1"].pressed, 7, "the other's table untouched");
        assert!(!again.device["desk-1"].on && again.device["desk-1"].seen == 7);
        // Broken: never written over.
        std::fs::write(&path, "[device.\"phone-1\"\nbroken").unwrap();
        assert!(change_own(&path, "desk-1", |_| true).is_err());
        assert!(std::fs::read_to_string(&path).unwrap().contains("broken"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_list_admits_its_people_by_address_and_number() {
        let fr = crate::phones::region_named("FR");
        let mut people = People::default();
        let alice = people.add(Person { name: "Alice".into(), phones: vec!["04 65 71 23 45".into()], emails: vec!["Alice@Example.org".into()], ..Person::default() }, fr);
        assert!(people.admits_address("alice@example.org"));
        assert!(people.admits_address("Alice Martin <ALICE@example.org>"));
        assert!(!people.admits_address("bob@example.org"));
        assert!(!people.admits_address(""));
        assert!(people.admits_number("+33 4 65 71 23 45", fr));
        assert!(people.admits_number("0465712345", fr));
        assert!(!people.admits_number("0465712346", fr));
        // A short code is nobody's.
        let mut short = People::default();
        short.add(Person { phones: vec!["3631".into()], ..Person::default() }, fr);
        assert!(!short.admits_number("3631", fr));
        // Added again with a new address: one person, merged.
        let again = people.add(Person { name: "A.".into(), phones: vec!["+33465712345".into()], emails: vec!["alice@work.example".into()], ..Person::default() }, fr);
        assert_eq!(again, alice);
        assert_eq!(people.people.len(), 1);
        assert_eq!(people.people[0].emails, vec!["Alice@Example.org", "alice@work.example"]);
        assert_eq!(people.people[0].name, "Alice", "the name kept");
        assert!(people.remove(&alice) && people.people.is_empty());
    }

    #[test]
    fn the_list_travels_one_person_at_a_time_and_is_never_read_empty() {
        let dir = std::env::temp_dir().join(format!("sioul-everywhere-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(PEOPLE_FILE);
        change_people(&path, |p| {
            p.add(Person { name: "Alice".into(), emails: vec!["alice@example.org".into()], ..Person::default() }, None);
        })
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[[person]]") && text.contains("alice@example.org"), "{text}");
        // Another device's person, brought by the sharing between two edits here: kept.
        std::fs::write(&path, format!("{text}\n[[person]]\nid = \"b\"\nname = \"Bob\"\nphones = [\"+33536490002\"]\n")).unwrap();
        let people = change_people(&path, |p| {
            p.add(Person { name: "Carol".into(), emails: vec!["carol@example.org".into()], ..Person::default() }, None);
        })
        .unwrap();
        assert_eq!(people.people.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Alice", "Bob", "Carol"]);
        // Unreadable: an error, never an empty list written over it.
        std::fs::write(&path, "[[person]\nbroken").unwrap();
        assert!(People::read(&path).is_err());
        assert!(change_people(&path, |p| p.people.clear()).is_err());
        assert!(std::fs::read_to_string(&path).unwrap().contains("broken"));
        assert!(People::load(&path).people.is_empty(), "a question gets nobody");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_safe_list_brings_its_people_with_their_cards() {
        let card = |name: &str, email: &str, phone: &str, category: &str| crate::contacts::Contact {
            uid: format!("uid-{name}"),
            name: name.into(),
            emails: vec![crate::contacts::Labeled { label: String::new(), value: email.into() }],
            phones: vec![crate::contacts::Labeled { label: String::new(), value: phone.into() }],
            categories: if category.is_empty() { Vec::new() } else { vec![category.into()] },
            ..crate::contacts::Contact::default()
        };
        let contacts = vec![card("Alice", "alice@example.org", "+33 4 65 71 23 45", ""), card("Bob", "bob@example.org", "+33 1 99 00 00 01", "Famille"), card("Eve", "eve@example.org", "", "")];
        let safe = ["alice@example.org", "category:famille", "@example.com", "*@example.net", "dan@example.net", "contact:uid-Eve", "tel:+33 3 53 01 00 07", "tel:+33899*"].map(String::from).to_vec();
        let (people, patterns) = from_safe(&safe, &contacts);
        assert_eq!(patterns, 3, "a domain, a pattern, a prefix");
        assert_eq!(people.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Alice", "Bob", "", "Eve", ""]);
        assert_eq!(people[0].phones, vec!["+33 4 65 71 23 45"]);
        assert_eq!(people[0].contact, "uid-Alice");
        assert_eq!(people[2].emails, vec!["dan@example.net"]);
        assert_eq!((people[3].contact.as_str(), people[4].phones.clone()), ("uid-Eve", vec!["+33 3 53 01 00 07".to_string()]));
    }

    #[test]
    fn settings_have_their_defaults_and_old_files_read() {
        let config: crate::config::Config = toml::from_str("").unwrap();
        assert_eq!(config.dnd, DndSettings::default());
        assert!(config.dnd.button && config.dnd.pauses && config.dnd.people && !config.dnd.focus && !config.dnd.sleep);
        let config: crate::config::Config = toml::from_str("[dnd]\nfocus = true\n").unwrap();
        assert!(config.dnd.focus && config.dnd.pauses);
        // A switch's file with fields this version does not know still reads, and keeps them.
        let s: Switch = toml::from_str("[device.a]\npressed = 5\non = true\nfuture = \"x\"\n").unwrap();
        assert!(s.latest().unwrap().on);
        assert!(toml::to_string(&s).unwrap().contains("future = \"x\""), "a later version's field written back");
        let p: People = toml::from_str("[[person]]\nid = \"a\"\nname = \"A\"\nring = \"loud\"\n").unwrap();
        assert!(toml::to_string(&p).unwrap().contains("ring = \"loud\""));
    }

    #[test]
    fn every_word_has_both_languages() {
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for key in [
                "dnd-everywhere", "dnd-everywhere-until", "dnd-here-only", "dnd-here-only-until", "dnd-waiting", "dnd-waiting-until", "dnd-not-everywhere", "dnd-not-everywhere-until", "dnd-elsewhere", "dnd-elsewhere-until", "dnd-own-only", "dnd-own-only-until",
                "dnd-device-silenced", "dnd-device-cannot", "dnd-device-unsilenced", "dnd-device-behind", "dnd-device-older", "dnd-device-here", "dnd-device-on", "dnd-device-phone", "dnd-device-other",
                "dnd-why-manual", "dnd-why-manual-from", "dnd-why-focus", "dnd-why-sleep", "dnd-why-paused", "dnd-why-free-time",
            ] {
                let mut args = crate::i18n::args();
                args.set("until", "15:00");
                args.set("device", "X");
                args.set("line", "Y.");
                args.set("name", "Z");
                let words = tr.text(key, Some(&args));
                assert!(!words.is_empty() && words != key && !words.contains('{'), "{language}: {key}: {words}");
            }
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Calls screened on a phone (docs/android.md, "Calls";
//! docs/research/call-screening.md, section 8).
//!
//! Android asks Sioul about each call, as its "Caller ID & spam app", and
//! waits for the answer before the phone rings: the answer must come in
//! milliseconds, from Java alone (android/…/Calls.java), never from Sioul's
//! library. So Sioul writes ahead, in its state folder, a **table**
//! (`calls/table.json`): who each number is (by `phones::key`), the floors
//! that always ring, "Let every call through" as your devices last said it,
//! and the coming days cut into **frames**, each with whether each row of the
//! Calls rows of the matrix of what reaches you rings then (`attention`):
//! the times, Free time's cells, and the layers (today's slots of time for
//! you, do-not-disturb from its switch or a focus session). Java decides in
//! this order: the floors (an emergency number or its callback; a day after
//! you call an emergency number); your blocked numbers, refused; "Let every
//! call through"; a second call within 15 minutes, which rings; then the
//! frame for now. No table, or past its frames: it rings. Always through
//! (`everywhere::People`) is no floor any more: its people's numbers are
//! written as rows of their own ("always-safe", their own row read where
//! Always through says "as their list"), never a blocked one: blocked beats
//! Always through (docs/attention.md, Q3).
//!
//! A refused call goes to your voicemail. Java writes **every incoming call it
//! screened**, declined or let ring (`rang`), in a file of its own
//! (`calls/held.jsonl`); Rust copies each into this phone's own **log**,
//! `calls/log/<device>.jsonl` (`carry_into`), which the sharing carries,
//! sealed, a line at a time, to your other devices (the part "Calls"). Every
//! device reads every phone's log (`read_logs`): the Porch lists the declined
//! calls calmly, at the next time those callers may reach you, never counted
//! (`lines`); a person's sheet shows their calls of the month (`history`).
//! **Seen** is each device's own log of lines (`calls/seen/<device>.jsonl`,
//! `mark_seen`), every device's read together (`read_seen`): seen on one,
//! gone from all. A call is kept `KEPT_DAYS`: each device takes its own lines
//! out after that (`trim_own`), and the sharing takes them out everywhere.
//!
//! **Let every call through** is pressed on any device: each device's own
//! table in do-not-disturb's switch file (`state/do-not-disturb.toml`,
//! shared) carries its last press, `[device.<id>.calls]`; the latest press
//! anywhere holds. The phone's notification writes its press in a file of
//! its own (`calls/through-here.json`) that Java reads at once and Rust
//! carries into the switch (`merge_local`).

use crate::attention::{self, Attention, Level, Person};
use crate::everywhere::{Device, People, Switch};
use crate::i18n::Translator;
use crate::phones::{self, Region};
use crate::reach::{Channel, Clock, Who};
use jiff::Zoned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The calls' folder, in the state folder.
pub const FOLDER: &str = "calls";
/// The table Java decides from.
pub const TABLE: &str = "table.json";
/// The calls screened, declined or let ring, one JSON line each (Java's,
/// this phone's alone: Rust copies them into its log, `LOG`).
pub const HELD: &str = "held.jsonl";
/// The folder of the phones' logs of the calls they screened, one file per
/// phone (`<device>.jsonl`), written by that phone alone; shared.
pub const LOG: &str = "log";
/// The folder of the calls marked seen, one file per device, written by
/// that device alone; shared.
pub const SEEN_LOG: &str = "seen";
/// This phone's press of "Let every call through" on the notification (Java's).
pub const THROUGH_HERE: &str = "through-here.json";
/// When this phone last called an emergency number (Java's).
pub const EMERGENCY: &str = "emergency.json";
/// The calls seen on the list, as this phone kept them before Seen was
/// shared: read once into its own seen log, then gone (`adopt_older_seen`).
pub const SEEN: &str = "seen.toml";
/// The table's shape: Java reads this one only (another one: every call rings).
pub const VERSION: u32 = 1;
/// A second call within this many minutes rings (research 7.3).
pub const REPEAT_MINUTES: i64 = 15;
/// Every call rings this many hours after you call an emergency number (research 7.1).
pub const AFTER_EMERGENCY_HOURS: i64 = 24;
/// How many days the table's frames cover, from today's midnight: it is
/// written again long before, at each change and each step.
pub const TABLE_DAYS: i64 = 4;
/// A call refused stays on the list this long at most (days), seen or not.
pub const LISTED_DAYS: i64 = 14;
/// A call is kept this long (days), on every device: its phone takes it out
/// of its log after that, and the sharing takes it out everywhere.
pub const KEPT_DAYS: i64 = 31;
/// Another device's line kept this long past `KEPT_DAYS` (days) is taken out
/// by any device: its phone, gone for good, never will (`trim_others`).
pub const LATE_DAYS: i64 = 14;
/// A day, in milliseconds.
const DAY_MS: i64 = 86_400_000;
/// The key of the calls' own part in a device's table of the switch's file.
const DEVICE_PART: &str = "calls";
/// The French plan's overseas numbers, by the three digits after the 0 (as `phones` reads them).
const FRENCH_OVERSEAS: [(&str, &str); 14] = [
    ("262", "262"),
    ("263", "262"),
    ("269", "262"),
    ("639", "262"),
    ("692", "262"),
    ("693", "262"),
    ("590", "590"),
    ("690", "590"),
    ("691", "590"),
    ("594", "594"),
    ("694", "594"),
    ("596", "596"),
    ("696", "596"),
    ("697", "596"),
];

/// The calls' folder.
pub fn folder() -> PathBuf {
    crate::config::state_dir().join(FOLDER)
}

// ---------------------------------------------------------------- the table

/// What Java reads (Calls.Table). Times are Unix milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub v: u32,
    pub made: i64,
    /// How the country of numbers written without one writes them; none: as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<RegionRules>,
    /// The country codes whose numbers lose a trunk "0" written after them ("+33 06…").
    #[serde(default)]
    pub trunk_zero: Vec<String>,
    /// Each number known, by its key: "safe", "neutral", "restricted", "blocked".
    #[serde(default)]
    pub numbers: BTreeMap<String, String>,
    /// The prefixes on your lists (`tel:+3346571*`), for a number no card and no
    /// entry of its own names: the longest first, then blocked before
    /// restricted before neutral before safe (`porch::Senders::judge_number`).
    #[serde(default)]
    pub prefixes: Vec<Prefix>,
    /// Who a number in the phone's own contacts is, when this table does not know it (research CS6).
    pub phone_contacts: String,
    pub floors: Floors,
    /// "Let every call through", the latest press of your devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub through: Option<Through>,
    pub frames: Vec<Frame>,
    pub repeat_minutes: i64,
    pub emergency_hours: i64,
}

/// A country's way of writing numbers, as Java needs it (`phones::Region`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionRules {
    pub code: String,
    pub calling: String,
    pub trunk: String,
    pub international: String,
    pub digits: [usize; 2],
    #[serde(default)]
    pub french: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overseas: BTreeMap<String, String>,
}

impl RegionRules {
    pub fn of(region: &Region) -> RegionRules {
        RegionRules {
            code: region.code.to_string(),
            calling: region.calling.to_string(),
            trunk: region.trunk.to_string(),
            international: region.international.to_string(),
            digits: [region.digits.0, region.digits.1],
            french: region.french,
            overseas: if region.french { FRENCH_OVERSEAS.iter().map(|(p, c)| (p.to_string(), c.to_string())).collect() } else { BTreeMap::new() },
        }
    }
}

/// A prefix on a list, and who a number starting with it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prefix {
    /// A key's start: "+3346571".
    pub prefix: String,
    pub who: String,
}

/// The numbers that always ring.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Floors {
    /// Emergency numbers and the emergency services' callback numbers, every country's.
    pub emergency: Vec<String>,
    /// Numbers that ring whatever else says, the blocked's aside: none written
    /// since Always through has rows of its own; kept for the Java that reads it.
    pub people: Vec<String>,
}

/// A press of "Let every call through": when (ms, the pressing device's
/// hybrid clock), on or off, until when (ms; 0: until turned off).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Through {
    #[serde(default)]
    pub pressed: i64,
    #[serde(default)]
    pub on: bool,
    #[serde(default)]
    pub until: i64,
}

impl Through {
    /// Whether it lets every call through at `now` (ms).
    pub fn holds(&self, now: i64) -> bool {
        self.on && (self.until <= 0 || now < self.until)
    }
}

/// A stretch of time and, for each row of the Calls matrix, whether it rings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub from: i64,
    pub until: i64,
    /// "work", "admin", "leisure", "meals", "sleep", "pause", "free"; a layer
    /// above them, "slot", "dnd": for the list's words.
    pub column: String,
    /// `ROWS` → rings.
    pub ring: BTreeMap<String, bool>,
}

/// The rows a frame says, as Java looks a caller up: the states, hidden
/// numbers, and Always through read against each state ("always-neutral":
/// someone on the list whose own row is neutral).
pub const ROWS: [&str; 9] = ["safe", "neutral", "restricted", "stranger", "hidden", "always-safe", "always-neutral", "always-restricted", "always-stranger"];

/// A frame's row: its person, on the Always through list or not.
fn person_of(row: &str) -> (Person, bool) {
    let (always, own) = match row.strip_prefix("always-") {
        Some(own) => (true, own),
        None => (false, row),
    };
    (Person::read(own).unwrap_or(Person::Stranger), always)
}

/// What holds on top of the clock's times, ahead (docs/attention.md, §1.6):
/// today's slots of time for you (`attention::Slots`), and do-not-disturb
/// from its switch or a focus session, from, and until when (none: until
/// turned off; the table is written again when it changes).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layers {
    pub slots: Vec<(i64, i64)>,
    pub dnd: Option<(i64, Option<i64>)>,
}

/// The column a moment is said by: a layer that holds, else the time's (the
/// pause, sleep, Free time, the hours; work when work and admin are both
/// open, or no hours are set).
fn column_of(now: &attention::Now) -> &'static str {
    if now.dnd {
        "dnd"
    } else if now.slot {
        "slot"
    } else {
        now.times.first().map_or("", |c| c.id())
    }
}

/// The frames from `from` to `until`, each with whether each row rings then
/// (`ROWS`): the clock's own frames (`reach::Clock::frames`), cut where a
/// layer begins or ends; two in a row that say the same are one.
pub fn frames(attention: &Attention, clock: &Clock, layers: &Layers, from: &Zoned, until: &Zoned) -> Vec<Frame> {
    let mut out: Vec<Frame> = Vec::new();
    for f in clock.frames(from, until) {
        let mut cuts = vec![f.start, f.end];
        let edges = layers.slots.iter().flat_map(|&(a, b)| [a, b]).chain(layers.dnd.iter().flat_map(|&(a, b)| [Some(a), b]).flatten());
        cuts.extend(edges.filter(|t| *t > f.start && *t < f.end));
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let slot = layers.slots.iter().any(|&(a, b)| a <= start && start < b);
            let dnd = layers.dnd.is_some_and(|(a, b)| a <= start && b.is_none_or(|b| start < b));
            let now = attention::Now::of_moment(&f.moment).layers(slot, dnd);
            let ring: BTreeMap<String, bool> = ROWS
                .iter()
                .map(|row| {
                    let (person, always) = person_of(row);
                    (row.to_string(), attention.person(Channel::Calls, person, always, &now) == Level::Now)
                })
                .collect();
            let column = column_of(&now).to_string();
            let (from_ms, until_ms) = (start.saturating_mul(1000), end.saturating_mul(1000));
            match out.last_mut() {
                Some(last) if last.until == from_ms && last.column == column && last.ring == ring => last.until = until_ms,
                _ => out.push(Frame { from: from_ms, until: until_ms, column, ring }),
            }
        }
    }
    out
}

/// The numbers of the people Always through, each as a row of its own read
/// against who the number is (`judge`: "always-safe"), whole numbers only;
/// never a blocked number: blocked beats Always through.
pub fn always_numbers(people: &People, region: Option<&Region>, judge: &dyn Fn(&str) -> Who) -> Vec<(String, String)> {
    people_keys(people, region).into_iter().filter_map(|key| match judge(&key) {
        Who::Blocked => None,
        who => Some((key, format!("always-{}", who.id()))),
    }).collect()
}

/// What a table is made of.
pub struct Made {
    /// When (ms).
    pub made: i64,
    pub region: Option<&'static Region>,
    /// Every number known, by key, and who it is ("safe"…, "blocked"; `porch::Senders::numbers`).
    pub numbers: Vec<(String, String)>,
    /// The lists' prefixes ("+3346571*") and who they make a number (`porch::Senders::prefixes`), in the order ties are decided.
    pub prefixes: Vec<(String, String)>,
    /// The Always through list's numbers, as rows of their own (`always_numbers`): after `numbers`, they win.
    pub always: Vec<(String, String)>,
    pub through: Option<Through>,
    pub frames: Vec<Frame>,
}

/// The table, as Java reads it.
pub fn table(made: Made) -> Table {
    let mut numbers = BTreeMap::new();
    for (key, who) in made.numbers {
        if !key.is_empty() && !who.is_empty() && who != "stranger" {
            numbers.insert(key, who);
        }
    }
    // Always through: its people's numbers, as their rows; the blocked kept blocked.
    for (key, row) in made.always {
        if !key.is_empty() && numbers.get(&key).is_none_or(|who| who != "blocked") {
            numbers.insert(key, row);
        }
    }
    // The longest first; at one length, as given (blocked before restricted before neutral before safe).
    let mut prefixes: Vec<Prefix> = made.prefixes.iter().filter_map(|(p, who)| Some(Prefix { prefix: p.trim_end_matches('*').to_string(), who: who.clone() }).filter(|p| !p.prefix.is_empty() && !p.prefix.contains('*') && !p.who.is_empty() && p.who != "stranger")).collect();
    prefixes.sort_by_key(|p| std::cmp::Reverse(p.prefix.len()));
    Table {
        v: VERSION,
        made: made.made,
        region: made.region.map(RegionRules::of),
        trunk_zero: trunk_zero(),
        numbers,
        prefixes,
        phone_contacts: "neutral".to_string(),
        floors: Floors { emergency: emergency_keys(made.region), people: Vec::new() },
        through: made.through,
        frames: made.frames,
        repeat_minutes: REPEAT_MINUTES,
        emergency_hours: AFTER_EMERGENCY_HOURS,
    }
}

/// The table as written, without its time: two tables that say the same are the same.
pub fn content(table: &Table) -> String {
    serde_json::to_string(&Table { made: 0, ..table.clone() }).unwrap_or_default()
}

/// The country codes whose numbers drop a trunk "0" (`phones`' table).
pub fn trunk_zero() -> Vec<String> {
    let mut codes: Vec<String> = phones::REGIONS.iter().filter(|r| r.trunk == "0").map(|r| r.calling.to_string()).collect();
    codes.sort();
    codes.dedup();
    codes
}

/// Emergency numbers, crisis lines and callback numbers of every country
/// Sioul knows (`data/crisis-lines.toml`), keyed as each country writes them,
/// and as the person's country would.
pub fn emergency_keys(region: Option<&Region>) -> Vec<String> {
    let lines = crate::pause::CrisisLines::built_in();
    let mut keys = BTreeSet::new();
    for country in &lines.country {
        let own = phones::region_named(&country.code);
        for line in &country.line {
            for key in [phones::key(&line.number, own.or(region)), phones::key(&line.number, region), line.number.chars().filter(char::is_ascii_digit).collect()] {
                if !key.is_empty() {
                    keys.insert(key);
                }
            }
        }
    }
    keys.into_iter().collect()
}

/// The numbers of the people on the do-not-disturb list, whole numbers only (`People::admits_number`).
pub fn people_keys(people: &People, region: Option<&Region>) -> Vec<String> {
    let keys: BTreeSet<String> = people.people.iter().flat_map(|p| p.phones.iter()).map(|n| phones::key(n, region)).filter(|k| phones::is_whole(k)).collect();
    keys.into_iter().collect()
}

/// Written beside, then put in place: Java never reads half a table.
pub fn write(path: &Path, table: &Table) -> Result<(), String> {
    let text = serde_json::to_string(table).map_err(|e| e.to_string())?;
    write_whole(path, &text)
}

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

// ---------------------------------------------------------------- the decision, as Java makes it

/// A call as Java sees it (Calls.decide), what it knows beside the table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Incoming {
    /// The caller ID as the network gave it; "" for none.
    pub number: String,
    /// No number shown (restricted, unknown, payphone, or no digits).
    pub hidden: bool,
    /// Placed by the person, not received.
    pub outgoing: bool,
    /// Now (ms).
    pub now: i64,
    /// When the person last called an emergency number (ms), as this phone saw it; 0 never.
    pub emergency_at: i64,
    /// This phone's own press of "Let every call through" (`through-here.json`).
    pub local: Option<Through>,
    /// The number's last call before this one (ms); 0 none.
    pub last_call: i64,
    /// The number is in the phone's own contacts (Android's), whatever Sioul's address books say.
    pub phone_contact: bool,
    /// Android's own list of emergency numbers has it (`TelephonyManager.isEmergencyNumber`).
    pub system_emergency: bool,
}

/// What Java answers, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub refuse: bool,
    /// "outgoing", "no-table", "emergency", "after-emergency", "through", "people", "blocked", "repeat", "no-frame", "no-row", "matrix".
    pub why: &'static str,
    /// The row decided by: "safe", "neutral", "restricted", "stranger", "hidden", "blocked"; "" before.
    pub who: String,
    pub column: String,
}

impl Table {
    /// Who a prefix on your lists makes this number (the longest first).
    pub fn by_prefix(&self, key: &str) -> Option<&str> {
        self.prefixes.iter().find(|p| key.starts_with(&p.prefix)).map(|p| p.who.as_str())
    }

    /// The frame `now` (ms) falls in; none before the first or past the last.
    pub fn frame_at(&self, now: i64) -> Option<&Frame> {
        self.frames.iter().find(|f| f.from <= now && now < f.until)
    }
}

/// Java's decision (Calls.decide), step for step: the order's reference,
/// tested here. Outgoing: rings (to an emergency number, Java marks the
/// moment). Incoming: what always rings (an emergency number, a day after
/// you called one, the do-not-disturb list); blocked; "Let every call
/// through"; a second call within 15 minutes; the frame for now. Nothing to
/// decide from: it rings.
pub fn decide(table: Option<&Table>, call: &Incoming) -> Decision {
    let ring = |why: &'static str, who: &str, column: &str| Decision { refuse: false, why, who: who.to_string(), column: column.to_string() };
    if call.outgoing {
        return ring("outgoing", "", "");
    }
    let Some(table) = table else { return ring("no-table", "", "") };
    let key = if call.hidden { String::new() } else { incoming_key(&call.number, table.region.as_ref(), &table.trunk_zero) };
    // 1. What always rings.
    if !call.hidden && (call.system_emergency || table.floors.emergency.contains(&key)) {
        return ring("emergency", "", "");
    }
    if call.emergency_at > 0 && call.now >= call.emergency_at && call.now - call.emergency_at < table.emergency_hours * 3_600_000 {
        return ring("after-emergency", "", "");
    }
    if !call.hidden && table.floors.people.contains(&key) {
        return ring("people", "", "");
    }
    // Who: the number's own, a prefix of your lists, a contact of the phone's only, else a stranger.
    let who = if call.hidden {
        "hidden".to_string()
    } else {
        table.numbers.get(&key).cloned().filter(|w| !w.is_empty()).or_else(|| table.by_prefix(&key).map(str::to_string)).unwrap_or_else(|| if call.phone_contact { table.phone_contacts.clone() } else { "stranger".to_string() })
    };
    // 2. Blocked ("blocked entirely": every call let through but theirs).
    if who == "blocked" {
        return Decision { refuse: true, why: "blocked", who, column: String::new() };
    }
    // 3. Let every call through: the later press of this phone's and the table's.
    let through = match (call.local, table.through) {
        (Some(local), Some(shared)) => Some(if local.pressed > shared.pressed { local } else { shared }),
        (local, shared) => local.or(shared),
    };
    if through.is_some_and(|t| t.holds(call.now)) {
        return ring("through", &who, "");
    }
    // 4. A second call within 15 minutes.
    if !call.hidden && call.last_call > 0 && call.now - call.last_call <= table.repeat_minutes * 60_000 {
        return ring("repeat", &who, "");
    }
    // 5. The frame for now.
    let Some(frame) = table.frame_at(call.now) else { return ring("no-frame", &who, "") };
    match frame.ring.get(&who) {
        Some(rings) => Decision { refuse: !rings, why: "matrix", who, column: frame.column.clone() },
        None => ring("no-row", &who, &frame.column),
    }
}

// ---------------------------------------------------------------- numbers as Java keys them

/// A caller ID as Java keys it (Calls.key, the same steps): `phones::key`
/// for what a caller ID holds (no extension, no "(0)"). The tests check that
/// the two agree.
pub fn incoming_key(raw: &str, region: Option<&RegionRules>, trunk_zero: &[String]) -> String {
    let mut value = raw.trim_matches(|c: char| c.is_whitespace() || spacing(c));
    if value.len() >= 4 && value.is_char_boundary(4) && value[..4].eq_ignore_ascii_case("tel:") {
        value = value[4..].trim();
    }
    let as_written = || value.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    let mut digits = String::new();
    for (i, c) in value.chars().enumerate() {
        match c {
            '+' if i == 0 => digits.push('+'),
            '0'..='9' => digits.push(c),
            '(' | ')' => {}
            c if spacing(c) => {}
            _ => return as_written(),
        }
    }
    if digits.is_empty() || digits == "+" {
        return as_written();
    }
    let drop_trunk = |digits: &str| -> String {
        for length in 1..=3usize.min(digits.len().saturating_sub(1)) {
            let (calling, rest) = digits.split_at(length);
            if rest.starts_with('0') && trunk_zero.iter().any(|t| t == calling) {
                return format!("{calling}{}", &rest[1..]);
            }
        }
        digits.to_string()
    };
    if let Some(international) = digits.strip_prefix('+') {
        return format!("+{}", drop_trunk(international));
    }
    let Some(region) = region else { return digits };
    for prefix in [region.international.as_str(), "00"] {
        if prefix.is_empty() {
            continue;
        }
        if let Some(rest) = digits.strip_prefix(prefix).filter(|rest| rest.len() >= 6 && !rest.starts_with('0')) {
            return format!("+{}", drop_trunk(rest));
        }
    }
    let length = digits.len();
    if length < region.digits[0] || length > region.digits[1] {
        return digits;
    }
    match region.trunk.as_str() {
        "" => format!("+{}{digits}", region.calling),
        "1" => match length {
            10 if !digits.starts_with(['0', '1']) => format!("+1{digits}"),
            11 if digits.starts_with('1') => format!("+{digits}"),
            _ => digits,
        },
        trunk => match digits.strip_prefix(trunk) {
            Some(national) if !national.is_empty() && !national.starts_with('0') => {
                let calling = if region.french { national.get(..3).and_then(|p| region.overseas.get(p)).cloned().unwrap_or_else(|| region.calling.clone()) } else { region.calling.clone() };
                format!("+{calling}{national}")
            }
            _ => digits,
        },
    }
}

/// The characters that only space a number out (as `phones` reads them).
fn spacing(c: char) -> bool {
    matches!(c, ' ' | '\t' | '.' | '-' | '/' | '\u{a0}' | '\u{202f}' | '\u{2009}' | '\u{2007}' | '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}')
        || matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

/// A key as shown: "01 99 00 12 34" for a number of your own country's
/// ten-digit plan, else its international form.
pub fn shown_number(key: &str, region: Option<&Region>) -> String {
    let Some(region) = region else { return key.to_string() };
    if region.french {
        // France and its overseas departments share one ten-digit plan.
        for calling in ["33", "262", "590", "594", "596"] {
            if let Some(national) = key.strip_prefix('+').and_then(|k| k.strip_prefix(calling))
                && national.len() == 9
                && national.chars().all(|c| c.is_ascii_digit())
                && (calling == "33" || FRENCH_OVERSEAS.iter().any(|(p, c)| *c == calling && national.starts_with(p)))
            {
                let digits = format!("0{national}");
                return digits.as_bytes().chunks(2).map(|pair| String::from_utf8_lossy(pair).to_string()).collect::<Vec<_>>().join(" ");
            }
        }
        return key.to_string();
    }
    match key.strip_prefix('+').and_then(|k| k.strip_prefix(region.calling)) {
        Some(national) if !region.trunk.is_empty() && region.trunk != "1" && national.chars().all(|c| c.is_ascii_digit()) => format!("{}{national}", region.trunk),
        _ => key.to_string(),
    }
}

// ---------------------------------------------------------------- "Let every call through"

/// The calls' part of one device's table in the switch's file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicePart {
    /// This device's last press (ms, hybrid clock); 0 never.
    #[serde(default)]
    pub pressed: i64,
    #[serde(default)]
    pub on: bool,
    /// Its end (ms); 0 until turned off.
    #[serde(default)]
    pub until: i64,
    /// The last press of the phone's notification carried here (ms, Java's stamp).
    #[serde(default)]
    pub local: i64,
    /// This device screens calls (a phone holding the role).
    #[serde(default)]
    pub screens: bool,
}

/// A device's calls part; nothing there: none pressed.
pub fn part_of(device: &Device) -> DevicePart {
    device.other.get(DEVICE_PART).and_then(|v| v.clone().try_into().ok()).unwrap_or_default()
}

/// A device's calls part written; a part with nothing to say is taken out.
pub fn set_part(device: &mut Device, part: DevicePart) {
    if part == DevicePart::default() {
        device.other.remove(DEVICE_PART);
        return;
    }
    if let Ok(value) = toml::Value::try_from(part) {
        device.other.insert(DEVICE_PART.to_string(), value);
    }
}

/// "Let every call through" as your devices say it: the latest press of any
/// (an "on" wins a tie); none when none pressed.
pub fn through_of(switch: &Switch) -> Option<Through> {
    switch
        .device
        .iter()
        .map(|(id, d)| (id, part_of(d)))
        .filter(|(_, p)| p.pressed > 0)
        .max_by_key(|(id, p)| (p.pressed, p.on, (*id).clone()))
        .map(|(_, p)| Through { pressed: p.pressed, on: p.on, until: if p.on { p.until } else { 0 } })
}

/// Whether some device screens calls: the switch's tables say.
pub fn screens_anywhere(switch: &Switch) -> bool {
    switch.device.values().any(|d| part_of(d).screens)
}

/// A press here (ms): stamped after every press known, so that the latest
/// wins everywhere whatever the clocks. Returns its stamp.
pub fn press(switch: &mut Switch, here: &str, on: bool, until: i64, now: i64) -> i64 {
    let latest = switch.device.values().map(|d| part_of(d).pressed).max().unwrap_or(0);
    let stamp = now.max(latest + 1);
    let own = switch.device.entry(here.to_string()).or_default();
    let mut part = part_of(own);
    part.pressed = stamp;
    part.on = on;
    part.until = if on { until.max(0) } else { 0 };
    set_part(own, part);
    stamp
}

/// The phone's own press, from its notification (`through-here.json`),
/// carried into its table once: with Java's stamp, which came after every
/// press the phone knew then. Whether anything changed.
pub fn merge_local(switch: &mut Switch, here: &str, local: &Through) -> bool {
    let own = switch.device.entry(here.to_string()).or_default();
    let mut part = part_of(own);
    if local.pressed <= part.local {
        return false;
    }
    part.local = local.pressed;
    if local.pressed > part.pressed {
        part.pressed = local.pressed;
        part.on = local.on;
        part.until = if local.on { local.until.max(0) } else { 0 };
    }
    set_part(own, part);
    true
}

/// This device screens calls, or no longer: written when it changed.
pub fn set_screens(switch: &mut Switch, here: &str, screens: bool) -> bool {
    let own = switch.device.entry(here.to_string()).or_default();
    let mut part = part_of(own);
    if part.screens == screens {
        return false;
    }
    part.screens = screens;
    set_part(own, part);
    true
}

/// A small JSON file of Java's; none when it is not there or does not read.
pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

// ---------------------------------------------------------------- the calls screened

/// A call screened, as Java wrote it (Calls.held) and as a phone's log keeps
/// it: declined (held, sent to voicemail) or let ring (`rang`). The log's line
/// leaves out what stays on the phone (the number as the network gave it, its
/// presentation) and every empty field (`logged`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    /// When (ms).
    pub at: i64,
    /// The number as Sioul keys it (`phones::key`); "" for a hidden number.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    /// As the network gave it; "" hidden. In Java's file only.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub number: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// Android's presentation of the number. In Java's file only.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub presentation: i32,
    /// The row it was judged by: "safe", "neutral", "restricted", "stranger",
    /// "hidden", "blocked", or "always-" and its own ("always-neutral"); ""
    /// when it rang before anyone was judged (an emergency number).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub who: String,
    /// The phone's contacts' name for it, when Sioul's address books did not know it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The time of day: a column of the matrix ("work", "sleep"…) or a layer
    /// ("slot", "dnd"); "" when no frame held (no table).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub column: String,
    /// Why it rang or not, as Java decided (`Decision::why`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub why: String,
    /// Let ring; false, declined and sent to voicemail (a line of Java's from
    /// before calls that rang were logged too was declined).
    #[serde(default)]
    pub rang: bool,
    /// The network's verification of the number: "passed", "failed"; "" not said.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub verified: String,
    /// The phone that screened it: its name in the sharing, its log's file's
    /// name; "" as read from Java's file. Never written in a line.
    #[serde(skip)]
    pub device: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

impl Held {
    /// Its name on the list: when, and who.
    pub fn id(&self) -> String {
        format!("{}-{}", self.at, if self.hidden { "hidden" } else { self.key.as_str() })
    }

    /// The line a phone's log keeps of it: without the number as the network
    /// gave it, nor its presentation; keyed as `region` writes numbers when
    /// Java could not key it (it had no table then).
    pub fn logged(&self, region: Option<&Region>) -> Held {
        let key = if self.hidden || !self.key.is_empty() { self.key.clone() } else { phones::key(&self.number, region) };
        Held { key, number: String::new(), presentation: 0, device: String::new(), ..self.clone() }
    }
}

/// The calls of one file, oldest first; a line that does not read (half written) is passed over.
pub fn read_held(path: &Path) -> Vec<Held> {
    let mut held: Vec<Held> = read_lines::<Held>(path).into_iter().filter(|h| h.at > 0).collect();
    held.sort_by_key(|h| h.at);
    held
}

/// Every line of a file of JSON lines that reads; a line cut by a crash, or
/// written by a later Sioul in a shape this one does not know, is passed over.
fn read_lines<T: serde::de::DeserializeOwned>(path: &Path) -> Vec<T> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// A device's own file in one of the calls' shared folders (`LOG`,
/// `SEEN_LOG`) under `root`, the calls' folder: `<folder>/<device>.jsonl`,
/// its name in the sharing (a UUID made once).
pub fn own_file(root: &Path, folder: &str, device: &str) -> PathBuf {
    root.join(folder).join(format!("{}.jsonl", file_safe(device)))
}

/// A device's name as a file's: letters, digits, `-` and `_`.
fn file_safe(device: &str) -> String {
    let safe: String = device.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if safe.is_empty() { "this-device".to_string() } else { safe }
}

/// Every device's file of a shared folder (`*.jsonl`), each with its device's name, by name.
fn device_files(folder: &Path) -> Vec<(String, PathBuf)> {
    let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "jsonl") && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .filter_map(|p| Some((p.file_stem()?.to_string_lossy().to_string(), p)))
        .collect();
    files.sort();
    files
}

/// Every phone's log under `root` (the calls' folder), each call with its
/// phone (`device`), oldest first: never a blocked caller's (their calls never
/// leave their phone), never one kept past `KEPT_DAYS` before `now_ms` (a
/// phone that stopped sharing never takes its own out).
pub fn read_logs(root: &Path, now_ms: i64) -> Vec<Held> {
    let mut calls: Vec<Held> = device_files(&root.join(LOG))
        .into_iter()
        .flat_map(|(device, path)| read_held(&path).into_iter().map(move |h| Held { device: device.clone(), ..h }))
        .filter(|h| h.who != "blocked" && now_ms - h.at <= KEPT_DAYS * DAY_MS)
        .collect();
    calls.sort_by(|a, b| (a.at, &a.device).cmp(&(b.at, &b.device)));
    calls
}

/// Java's new calls copied into this phone's own log (`own`), under its lock,
/// read again there: those whose line is not in it yet (by its id), never a
/// blocked caller's, nor one past `KEPT_DAYS` (taken out already, it would
/// come back). Two processes copying at once never write a line twice.
/// Returns how many lines were written.
pub fn carry_into(own: &Path, java: &[Held], region: Option<&Region>, now_ms: i64) -> Result<usize, String> {
    let fresh: Vec<Held> = java.iter().filter(|h| h.at > 0 && h.who != "blocked" && now_ms - h.at <= KEPT_DAYS * DAY_MS).map(|h| h.logged(region)).collect();
    if fresh.is_empty() {
        return Ok(0);
    }
    crate::filelock::with_lock(own, || {
        let mut known: BTreeSet<String> = read_held(own).iter().map(Held::id).collect();
        let mut text = String::new();
        let mut written = 0;
        for line in fresh {
            if known.insert(line.id()) {
                text.push_str(&serde_json::to_string(&line).map_err(|e| e.to_string())?);
                text.push('\n');
                written += 1;
            }
        }
        if written > 0 {
            append_private(own, &text)?;
        }
        Ok(written)
    })
}

/// Adds lines at the end of a file of this device's own, made yours alone
/// (0600, in a 0700 folder, on Unix). The caller holds its lock.
fn append_private(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(folder) = path.parent() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(folder).map_err(fail)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path).and_then(|mut file| file.write_all(text.as_bytes())).map_err(fail)
}

/// A file of the calls' kept but for the lines `drop` names: written beside
/// under a hidden name (yours alone), then put in place, under its lock.
/// Returns how many lines went.
fn rewrite_lines(path: &Path, drop: impl Fn(&str) -> bool) -> Result<usize, String> {
    crate::filelock::with_lock(path, || {
        let Ok(text) = std::fs::read_to_string(path) else { return Ok(0) };
        let (kept, gone): (Vec<&str>, Vec<&str>) = text.lines().filter(|l| !l.trim().is_empty()).partition(|l| !drop(l));
        if gone.is_empty() {
            return Ok(0);
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let fresh = path.with_file_name(format!(".{name}.{}.new", std::process::id()));
        let _ = std::fs::remove_file(&fresh);
        let body: String = kept.iter().map(|l| format!("{l}\n")).collect();
        append_private(&fresh, &body).and_then(|()| {
            std::fs::rename(&fresh, path).map_err(|e| {
                let _ = std::fs::remove_file(&fresh);
                format!("{}: {e}", path.display())
            })
        })?;
        Ok(gone.len())
    })
}

/// This device's own lines kept past `KEPT_DAYS` taken out of its log and of
/// its seen log under `root` (the calls' folder): the sharing then takes them
/// out on every device. A line that does not read stays. Returns how many went.
pub fn trim_own(root: &Path, device: &str, now_ms: i64) -> Result<usize, String> {
    let old = older_than(KEPT_DAYS, now_ms);
    Ok(rewrite_lines(&own_file(root, LOG, device), &old)? + rewrite_lines(&own_file(root, SEEN_LOG, device), &old)?)
}

/// Another device's lines kept `LATE_DAYS` past their month taken out here
/// (`here`: this device): a phone gone for good never takes its own out, and
/// its calls must not stay on your other devices for ever. While it shares,
/// it took them out long before (`trim_own`), and nothing differs. Returns
/// how many went.
pub fn trim_others(root: &Path, here: &str, now_ms: i64) -> Result<usize, String> {
    let old = older_than(KEPT_DAYS + LATE_DAYS, now_ms);
    let mut gone = 0;
    for folder in [LOG, SEEN_LOG] {
        for (device, path) in device_files(&root.join(folder)) {
            if device != file_safe(here) {
                gone += rewrite_lines(&path, &old)?;
            }
        }
    }
    Ok(gone)
}

/// A line older than `days` before `now_ms`, by its `at`; a line that does not read is kept.
fn older_than(days: i64, now_ms: i64) -> impl Fn(&str) -> bool {
    move |line: &str| serde_json::from_str::<serde_json::Value>(line).ok().and_then(|v| v["at"].as_i64()).is_some_and(|at| now_ms - at > days * DAY_MS)
}

/// One press of Seen on a device: when (ms), and the calls of its line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenLine {
    pub at: i64,
    #[serde(default)]
    pub ids: Vec<String>,
}

/// The calls seen, by id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    #[serde(default)]
    pub seen: BTreeSet<String>,
}

impl Seen {
    /// This phone's file from before Seen was shared (`SEEN`).
    pub fn load(path: &Path) -> Seen {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Those calls marked seen; ids older than the list forgotten (`now` in ms).
    pub fn add(&mut self, ids: &[String], now: i64) {
        self.seen.extend(ids.iter().cloned());
        let oldest = now - (LISTED_DAYS + 1) * DAY_MS;
        self.seen.retain(|id| id.split('-').next().and_then(|at| at.parse::<i64>().ok()).is_some_and(|at| at >= oldest));
    }
}

/// The calls seen on any device: every device's seen log under `root` (the
/// calls' folder), read together. Seen on one device, gone from the Porch of
/// every device.
pub fn read_seen(root: &Path) -> Seen {
    let mut seen = Seen::default();
    for (_, path) in device_files(&root.join(SEEN_LOG)) {
        for line in read_lines::<SeenLine>(&path) {
            seen.seen.extend(line.ids);
        }
    }
    seen
}

/// Calls marked seen on this device: one line in its own seen log under
/// `root`, which the sharing carries to the others. Returns the line's time,
/// to undo it (`unmark_seen`).
pub fn mark_seen(root: &Path, device: &str, ids: &[String], now_ms: i64) -> Result<i64, String> {
    let path = own_file(root, SEEN_LOG, device);
    let text = serde_json::to_string(&SeenLine { at: now_ms, ids: ids.to_vec() }).map_err(|e| e.to_string())? + "\n";
    crate::filelock::with_lock(&path, || append_private(&path, &text))?;
    Ok(now_ms)
}

/// A press of Seen undone: this device's own line written at `at` taken out
/// of its seen log, and so out of every device's once the sharing carries it.
/// Another device's Seen of the same calls stays. Whether a line went.
pub fn unmark_seen(root: &Path, device: &str, at: i64) -> Result<bool, String> {
    Ok(rewrite_lines(&own_file(root, SEEN_LOG, device), |line| serde_json::from_str::<SeenLine>(line).is_ok_and(|s| s.at == at))? > 0)
}

/// This phone's calls seen before Seen was shared (`SEEN`): one line of its
/// own seen log, then the older file goes. Nothing to do once it is gone.
pub fn adopt_older_seen(root: &Path, device: &str, now_ms: i64) -> Result<(), String> {
    let older = root.join(SEEN);
    if !older.exists() {
        return Ok(());
    }
    let ids: Vec<String> = Seen::load(&older).seen.into_iter().collect();
    if !ids.is_empty() {
        mark_seen(root, device, &ids, now_ms)?;
    }
    std::fs::remove_file(&older).map_err(|e| format!("{}: {e}", older.display()))
}

/// A voicemail left, as the list says it (`voicemail::Voicemail`, linked).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    /// "They left a message (0:42)."
    pub text: String,
    /// The mail that carries it, and its sound's place among the attachments; none in a notice without it.
    pub path: String,
    pub sound: Option<u32>,
}

/// One line of the list: a caller, in one context, one day, on one phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    /// The calls it says, to mark them seen.
    pub ids: Vec<String>,
    /// "While you slept, a number not in your contacts called at 09:30."; on
    /// another device, "…called your phone at 09:30."
    pub text: String,
    /// "01 99 00 12 34"; "" for a hidden number.
    pub number: String,
    /// What Call back and Text back dial ("+33199001234"); "" hidden.
    pub dial: String,
    /// In your address books or the phone's: no "Add to contacts".
    pub known: bool,
    pub hidden: bool,
    /// "They may have left a message.", when no voicemail is linked.
    pub doubt: String,
    pub message: Option<Message>,
    /// Why it went to voicemail, on request: "Numbers not in your contacts go to voicemail in your leisure time."
    pub why: String,
    /// "The same number called again at 06:48, and that call rang.": a call
    /// that rang from the same number after the line's last, the same day; "" none.
    pub again: String,
}

/// What the list needs besides the calls.
pub struct Lister<'a> {
    pub now: &'a Zoned,
    pub tr: &'a Translator,
    pub region: Option<&'static Region>,
    /// The name your address books give a number's key; none: not in them.
    pub name_of: &'a dyn Fn(&str) -> Option<String>,
    /// Whether this call's caller may reach you now (by phone or in writing).
    pub shows: &'a dyn Fn(&Held) -> bool,
    /// The phone a call came to, as this device says it (`Held::device`): ""
    /// for this device's own ("called at"), else "your phone", "your other
    /// phone", "your phone (GS290)" (`phone_words`).
    pub phone: &'a dyn Fn(&str) -> String,
}

/// The list, oldest first: the declined calls not seen yet on any device,
/// of the last two weeks, whose callers may reach you now; never the blocked;
/// one line per phone, caller, context and day. A call that rang is never a
/// line: it is said under the line of the declined calls it followed.
/// `calls` are every phone's (`read_logs`), oldest first; `messages` maps a
/// call's id to its voicemail.
pub fn lines(calls: &[Held], seen: &Seen, messages: &BTreeMap<String, Message>, l: &Lister) -> Vec<Line> {
    let now_ms = l.now.timestamp().as_millisecond();
    let mut shown: Vec<&Held> = calls.iter().filter(|h| !h.rang && h.who != "blocked" && !seen.seen.contains(&h.id()) && now_ms - h.at <= LISTED_DAYS * DAY_MS && h.at <= now_ms + 60_000 && (l.shows)(h)).collect();
    shown.sort_by_key(|h| h.at);
    // One line per phone, caller, context and day, in the order of their first call.
    let mut groups: Vec<(&str, String, &str, jiff::civil::Date, Vec<&Held>)> = Vec::new();
    for h in shown {
        let date = local(h.at, l.now).date();
        let caller = if h.hidden { "hidden".to_string() } else { h.key.clone() };
        match groups.iter_mut().find(|(d, c, col, day, _)| *d == h.device && *c == caller && *col == h.column && *day == date) {
            Some((_, _, _, _, group)) => group.push(h),
            None => groups.push((h.device.as_str(), caller, h.column.as_str(), date, vec![h])),
        }
    }
    groups.into_iter().map(|(device, _, column, date, group)| line(&group, calls, device, column, date, messages, l)).collect()
}

fn local(at_ms: i64, now: &Zoned) -> Zoned {
    jiff::Timestamp::from_millisecond(at_ms).map_or_else(|_| now.clone(), |t| t.to_zoned(now.time_zone().clone()))
}

/// The first call that rang from the same number after the last of `group`,
/// the same day, on any phone: none for a hidden number (two hidden calls
/// cannot be told apart).
fn rang_after<'a>(calls: &'a [Held], group: &[&Held], date: jiff::civil::Date, l: &Lister) -> Option<&'a Held> {
    let last = group.last()?;
    if last.hidden {
        return None;
    }
    let now_ms = l.now.timestamp().as_millisecond();
    calls.iter().filter(|h| h.rang && !h.hidden && h.key == last.key && h.at > last.at && h.at <= now_ms + 60_000).min_by_key(|h| h.at).filter(|h| local(h.at, l.now).date() == date)
}

/// "Monday 5 October", "yesterday", "today": a day, as the lines say it before
/// a capital is put to the sentence.
fn day_words(tr: &Translator, date: jiff::civil::Date, today: jiff::civil::Date) -> String {
    if date == today {
        tr.text("calls-day-today", None)
    } else if today.yesterday().is_ok_and(|y| y == date) {
        tr.text("calls-day-yesterday", None)
    } else {
        format!("{} {}", tr.text(&format!("weekday-{}", date.weekday().to_monday_one_offset()), None), tr.day_month(date))
    }
}

fn line(group: &[&Held], calls: &[Held], device: &str, column: &str, date: jiff::civil::Date, messages: &BTreeMap<String, Message>, l: &Lister) -> Line {
    let tr = l.tr;
    let first = group[0];
    let name = if first.hidden { None } else { (l.name_of)(&first.key).or_else(|| group.iter().map(|h| h.name.trim()).find(|n| !n.is_empty()).map(str::to_string)) };
    let who = if first.hidden {
        tr.text("calls-who-hidden", None)
    } else {
        name.clone().unwrap_or_else(|| tr.text("calls-who-stranger", None))
    };
    let context_words = context(tr, column);
    let today = l.now.date();
    let context_said = if date == today {
        context_words.clone()
    } else {
        let mut args = crate::i18n::args();
        args.set("day", day_words(tr, date, today));
        args.set("context", context_words.clone());
        tr.text("calls-context-day", Some(&args))
    };
    let time = |h: &Held| local(h.at, l.now).strftime("%H:%M").to_string();
    // This phone's own calls say "called at"; another phone's, which one it was.
    let phone = (l.phone)(device);
    let on = if phone.is_empty() { "" } else { "-on" };
    let mut args = crate::i18n::args();
    args.set("context", context_said);
    args.set("who", who);
    args.set("phone", phone.clone());
    let text = match group.len() {
        1 => {
            args.set("time", time(first));
            tr.text(&format!("calls-line-once{on}"), Some(&args))
        }
        2 => {
            args.set("first", time(first));
            args.set("second", time(group[1]));
            tr.text(&format!("calls-line-twice{on}"), Some(&args))
        }
        n => {
            args.set("count", tr.count(n, false, false));
            args.set("time", time(group[n - 1]));
            tr.text(&format!("calls-line-more{on}"), Some(&args))
        }
    };
    let rang = rang_after(calls, group, date, l);
    let again = rang
        .map(|h| {
            let mut args = crate::i18n::args();
            args.set("time", time(h));
            tr.text("calls-again-rang", Some(&args))
        })
        .unwrap_or_default();
    let ids: Vec<String> = group.iter().map(|h| h.id()).collect();
    // A message left after the declined calls, or after the call that rang next.
    let message = ids.iter().chain(rang.map(Held::id).iter()).find_map(|id| messages.get(id)).cloned();
    // Someone Always through, declined: their own row held them ("always-neutral": as the neutral).
    let row = if first.hidden { "hidden" } else if name.is_none() { "stranger" } else { first.who.trim_start_matches("always-") };
    let mut why_args = crate::i18n::args();
    why_args.set("context", context_words);
    let why = match row {
        "safe" | "neutral" | "restricted" | "stranger" | "hidden" => tr.text(&format!("calls-why-{row}"), Some(&why_args)),
        _ => String::new(),
    };
    Line {
        ids,
        text: capitalized(&text),
        number: if first.hidden { String::new() } else { shown_number(&first.key, l.region) },
        dial: if first.hidden { String::new() } else { first.key.clone() },
        known: name.is_some(),
        hidden: first.hidden,
        doubt: if message.is_none() { tr.text("calls-may-have-left", None) } else { String::new() },
        message,
        why,
        again,
    }
}

/// "while you slept", "during work": what the time was, in a few words.
fn context(tr: &Translator, column: &str) -> String {
    match column {
        "work" | "admin" | "leisure" | "meals" | "sleep" | "pause" | "free" | "slot" | "dnd" => tr.text(&format!("calls-context-{column}"), None),
        _ => tr.text("calls-context-any", None),
    }
}

/// The phone a call came to, said on another device: "your phone" when one
/// phone shares its calls with this device, "your other phone" on a phone
/// when one other does, else "your phone (GS290)" by its name among your
/// devices (`phones`: each phone's id with its name; "" for this device's own).
pub fn phone_words(tr: &Translator, device: &str, here: &str, phones: &[(String, String)], this_is_a_phone: bool) -> String {
    if device == here {
        return String::new();
    }
    let others: Vec<&(String, String)> = phones.iter().filter(|(id, _)| id != here).collect();
    let name = others.iter().find(|(id, _)| id == device).map(|(_, name)| name.trim()).unwrap_or("");
    if others.len() > 1 && !name.is_empty() {
        let mut args = crate::i18n::args();
        args.set("name", name.to_string());
        return tr.text("calls-phone-named", Some(&args));
    }
    tr.text(if this_is_a_phone { "calls-phone-other" } else { "calls-phone-yours" }, None)
}

/// A person's calls of the last month (`keys`, their numbers as Sioul keys
/// them), newest first, from every phone, rang or declined: one sentence each,
/// for their sheet, never a count. Each says the phone only when several
/// phones share their calls. A blocked caller's never come: their calls never
/// leave their phone.
pub fn history(calls: &[Held], keys: &BTreeSet<String>, l: &Lister) -> Vec<String> {
    let now_ms = l.now.timestamp().as_millisecond();
    let several = calls.iter().map(|h| h.device.as_str()).collect::<BTreeSet<_>>().len() > 1;
    calls.iter().rev().filter(|h| !h.hidden && keys.contains(&h.key) && h.who != "blocked" && now_ms - h.at <= KEPT_DAYS * DAY_MS && h.at <= now_ms + 60_000).map(|h| history_line(h, several, l)).collect()
}

/// "Tuesday 6 October at 23:10, while you slept: declined, sent to
/// voicemail.", "Today at 06:48, while you slept: it rang. A second call
/// within 15 minutes rings."
fn history_line(h: &Held, several: bool, l: &Lister) -> String {
    let tr = l.tr;
    let at = local(h.at, l.now);
    let mut args = crate::i18n::args();
    args.set("day", day_words(tr, at.date(), l.now.date()));
    args.set("time", at.strftime("%H:%M").to_string());
    let when = tr.text("calls-history-when", Some(&args));
    let head = if h.column.is_empty() {
        when
    } else {
        let mut args = crate::i18n::args();
        args.set("when", when);
        args.set("context", context(tr, &h.column));
        tr.text("calls-history-head", Some(&args))
    };
    let phone = if several {
        let said = (l.phone)(&h.device);
        if said.is_empty() { tr.text("calls-phone-this", None) } else { said }
    } else {
        String::new()
    };
    let outcome = if h.rang { "calls-history-rang" } else { "calls-history-declined" };
    let mut args = crate::i18n::args();
    args.set("phone", phone.clone());
    let outcome = tr.text(&format!("{outcome}{}", if phone.is_empty() { "" } else { "-on" }), Some(&args));
    let mut args = crate::i18n::args();
    args.set("head", head);
    args.set("outcome", outcome);
    let mut text = capitalized(&tr.text("calls-history-line", Some(&args)));
    if let Some(reason) = h.rang.then(|| rang_reason(tr, &h.why)).flatten() {
        text.push(' ');
        text.push_str(&reason);
    }
    text
}

/// Why a call rang when its row alone would not say it: a second call, Let
/// every call through, an emergency, nothing to decide from.
fn rang_reason(tr: &Translator, why: &str) -> Option<String> {
    let key = match why {
        "repeat" => "calls-rang-repeat",
        "through" => "calls-rang-through",
        "emergency" => "calls-rang-emergency",
        "after-emergency" => "calls-rang-after-emergency",
        "no-table" | "no-frame" | "no-row" | "error" => "calls-rang-undecided",
        _ => return None,
    };
    Some(tr.text(key, None))
}

/// The first letter in capital: a sentence that starts with a context's words.
pub fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// "0:42": a voicemail's length.
pub fn length(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// What the list says of a voicemail linked to a call.
pub fn message_words(tr: &Translator, seconds: Option<u32>) -> String {
    match seconds.filter(|s| *s > 0) {
        Some(seconds) => {
            let mut args = crate::i18n::args();
            args.set("length", length(seconds));
            tr.text("calls-left-message", Some(&args))
        }
        None => tr.text("calls-left-message-plain", None),
    }
}

// ---------------------------------------------------------------- what is said of it

/// "Let every call through" in words: "Every call rings until 15:30.",
/// "… until you turn it off.", or after an emergency call, "Every call rings
/// until tomorrow at 09:31: you called an emergency number."; "" otherwise.
/// `through` and `emergency_at` in ms; `when` words an instant (ms).
pub fn through_line(tr: &Translator, through: Option<&Through>, emergency_at: i64, now: i64, when: &dyn Fn(i64) -> String) -> String {
    let after = emergency_at + AFTER_EMERGENCY_HOURS * 3_600_000;
    if emergency_at > 0 && emergency_at <= now && now < after {
        let mut args = crate::i18n::args();
        args.set("until", when(after));
        return tr.text("calls-through-emergency", Some(&args));
    }
    match through.filter(|t| t.holds(now)) {
        Some(t) if t.until > 0 => {
            let mut args = crate::i18n::args();
            args.set("until", when(t.until));
            tr.text("calls-through-until", Some(&args))
        }
        Some(_) => tr.text("calls-through-on", None),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::everywhere::Person;

    fn fr() -> Option<&'static Region> {
        phones::region_named("FR")
    }

    #[test]
    fn java_keys_numbers_as_phones_does() {
        let zero = trunk_zero();
        // Numbers lent to fiction only: ARCEP's (01 99 00, 06 39 98), Ofcom's
        // drama ones (020 7946 0xxx, 07700 900xxx), North America's 555-01xx.
        // No country without a trunk prefix (Spain, Italy) lends any: a
        // country code nobody has (999, spare in E.164) reads them as they do,
        // the leading 0 kept as Italy's.
        let spare = Region { code: "XX", calling: "999", trunk: "", international: "00", digits: (9, 10), french: false };
        for (region, numbers) in [
            (phones::region_named("FR"), &["0199001234", "01 99 00 12 34", "+33199001234", "0033199001234", "+330199001234", "0639981234", "0692123456", "0590123456", "0696123456", "0594123456", "112", "15", "3114", "0800112112", "08 00 11 21 12", "+44 20 7946 0018", "0044 20 7946 0018", "+262692123456", "+262 6 39 98 12 34", "tel:+262639981234", "*#06#", "1-555-SIOUL", "639981234", "\u{202a}+33 1 99 00 12 34\u{202c}", "01\u{a0}99\u{202f}00 12 34"][..]),
            (phones::region_named("GB"), &["07700 900123", "020 7946 0018", "+44 20 7946 0018", "0044 7700 900123", "999"][..]),
            (phones::region_named("US"), &["(212) 555-0100", "1 212 555 0100", "011 33 6 39 98 12 34", "911"][..]),
            (Some(&spare), &["639 98 12 34", "0639 98 12 34", "+999 0639 98 12 34", "112"][..]),
        ] {
            let rules = region.map(RegionRules::of);
            let country = region.map_or("none", |r| r.code);
            for number in numbers {
                assert_eq!(incoming_key(number, rules.as_ref(), &zero), phones::key(number, region), "{country}: {number}");
            }
        }
        // The keys the branches give: a trunk 0 dropped, none to drop (the 0 kept), letters as written.
        assert_eq!(phones::key("07700 900123", phones::region_named("GB")), "+447700900123");
        assert_eq!((phones::key("639 98 12 34", Some(&spare)), phones::key("0639 98 12 34", Some(&spare))), ("+999639981234".to_string(), "+9990639981234".to_string()));
        assert_eq!(incoming_key("1-555-SIOUL", None, &zero), "1-555-sioul");
        // No country known: as written, as phones does.
        assert_eq!(incoming_key("06 39 98 12 34", None, &zero), phones::key("06 39 98 12 34", None));
        assert_eq!(incoming_key("+33 05 36 49 12 34", None, &zero), "+33536491234");
        // Every overseas prefix agrees with phones' own reading (Mayotte's 0639 98 is fiction's).
        let rules = RegionRules::of(fr().unwrap());
        for (prefix, calling) in FRENCH_OVERSEAS {
            let number = format!("0{prefix}981234");
            assert_eq!(phones::key(&number, fr()), format!("+{calling}{prefix}981234"), "{number}");
            assert_eq!(incoming_key(&number, Some(&rules), &zero), phones::key(&number, fr()));
        }
    }

    /// A table as Java reads it: Monday 5 October 2026, work 09:00–17:00, then
    /// leisure, the night from 22:00; the usual Calls matrix. The carer and
    /// a blocked number are on the Always through list.
    fn decision_table() -> Table {
        let mut people = People::default();
        people.add(Person { name: "Carer".into(), phones: vec!["01 99 00 00 09".into(), "01 99 00 00 04".into()], ..Person::default() }, fr());
        let blocked = |key: &str| if key == "+33199000004" { Who::Blocked } else { Who::Stranger };
        let work = (ms("2026-10-05T09:00[Europe/Paris]"), ms("2026-10-05T17:00[Europe/Paris]"));
        let leisure = (work.1, ms("2026-10-05T22:00[Europe/Paris]"));
        let sleep = (leisure.1, ms("2026-10-06T07:00[Europe/Paris]"));
        table(Made {
            made: 0,
            region: fr(),
            numbers: vec![
                ("+33199000001".into(), "safe".into()),
                ("+33199000002".into(), "neutral".into()),
                ("+33199000003".into(), "restricted".into()),
                ("+33199000004".into(), "blocked".into()),
            ],
            prefixes: vec![("+3346571*".into(), "blocked".into())],
            always: always_numbers(&people, fr(), &blocked),
            through: None,
            frames: vec![
                frame(ms("2026-10-05T00:00[Europe/Paris]"), work.0, "sleep", &["always-stranger"]),
                frame(work.0, work.1, "work", &["safe", "neutral", "restricted", "hidden", "always-stranger"]),
                frame(leisure.0, leisure.1, "leisure", &["safe", "always-stranger"]),
                frame(sleep.0, sleep.1, "sleep", &["always-stranger"]),
            ],
        })
    }

    fn call(number: &str, at: &str) -> Incoming {
        Incoming { number: number.into(), hidden: number.is_empty(), now: ms(&format!("2026-10-05T{at}[Europe/Paris]")), ..Incoming::default() }
    }

    #[test]
    fn who_rings_when_and_what_always_rings() {
        let t = decision_table();
        let said = |c: &Incoming| {
            let d = decide(Some(&t), c);
            (d.refuse, d.why, d.who)
        };
        // Who × time: the frame's row, as the matrix said it ahead.
        for (number, at, refused, who) in [
            ("01 99 00 00 01", "10:00", false, "safe"),
            ("01 99 00 00 01", "18:00", false, "safe"),
            ("01 99 00 00 01", "23:00", true, "safe"),
            ("01 99 00 00 02", "10:00", false, "neutral"),
            ("01 99 00 00 02", "18:00", true, "neutral"),
            ("+33 1 99 00 00 03", "10:00", false, "restricted"),
            ("0199000003", "18:00", true, "restricted"),
            ("01 99 00 55 55", "10:00", true, "stranger"),
            ("", "10:00", false, "hidden"),
            ("", "18:00", true, "hidden"),
        ] {
            let (refuse, why, w) = said(&call(number, at));
            assert_eq!((refuse, why, w.as_str()), (refused, "matrix", who), "{number} at {at}");
        }
        // Blocked: by its number, or a prefix on a list, at every time, a second call too.
        assert_eq!(said(&call("01 99 00 00 04", "10:00")), (true, "blocked", "blocked".to_string()));
        assert_eq!(said(&call("04 65 71 12 34", "10:00")), (true, "blocked", "blocked".to_string()));
        let again = Incoming { last_call: ms("2026-10-05T09:55[Europe/Paris]"), ..call("01 99 00 00 04", "10:00") };
        assert_eq!(said(&again).1, "blocked");
        // A contact of the phone's only: neutral.
        assert_eq!(said(&Incoming { phone_contact: true, ..call("01 99 00 55 55", "10:00") }), (false, "matrix", "neutral".to_string()));
        assert_eq!(said(&Incoming { phone_contact: true, ..call("01 99 00 55 55", "18:00") }), (true, "matrix", "neutral".to_string()));
        // A second call within 15 minutes rings; after 15, the matrix again; hidden numbers never count.
        let first = ms("2026-10-05T18:00[Europe/Paris]");
        assert_eq!(said(&Incoming { last_call: first, ..call("01 99 00 55 55", "18:10") }).1, "repeat");
        assert_eq!(said(&Incoming { last_call: first, ..call("01 99 00 55 55", "18:16") }), (true, "matrix", "stranger".to_string()));
        assert_eq!(said(&Incoming { last_call: first, ..call("", "18:10") }), (true, "matrix", "hidden".to_string()));
        // The floors: emergency numbers and callbacks, as the table or Android says; the do-not-disturb list.
        for number in ["112", "15", "0 800 112 112", "+33800112112"] {
            assert_eq!(said(&call(number, "23:00")).1, "emergency", "{number}");
        }
        assert_eq!(said(&Incoming { system_emergency: true, ..call("17", "23:00") }).1, "emergency");
        assert_eq!(said(&call("17", "23:00")).0, true, "a short code Android does not call an emergency number: a stranger");
        // Always through: their own row, at night too; blocked beats it (before: the list's floor rang it).
        assert_eq!(said(&call("01 99 00 00 09", "23:00")), (false, "matrix", "always-stranger".to_string()));
        assert_eq!(said(&call("01 99 00 00 04", "23:00")).1, "blocked");
        assert!(t.floors.people.is_empty(), "no floor before the blocked any more");
        // A day after you call an emergency number, every call rings, the blocked too; not after.
        let called = ms("2026-10-05T03:00[Europe/Paris]");
        assert_eq!(said(&Incoming { emergency_at: called, ..call("01 99 00 00 04", "23:00") }).1, "after-emergency");
        assert_eq!(said(&Incoming { emergency_at: called - 86_400_000, ..call("01 99 00 55 55", "23:00") }).1, "matrix");
        // Let every call through: the later press of this phone's and the table's.
        let on = Through { pressed: 10, on: true, until: 0 };
        let mut through = t.clone();
        through.through = Some(on);
        assert_eq!(decide(Some(&through), &call("01 99 00 55 55", "23:00")).why, "through");
        assert_eq!(decide(Some(&through), &call("01 99 00 00 04", "23:00")).why, "blocked", "blocked entirely: never let through");
        let off_here = Through { pressed: 11, on: false, until: 0 };
        assert!(decide(Some(&through), &Incoming { local: Some(off_here), ..call("01 99 00 55 55", "23:00") }).refuse, "turned off on the phone after");
        let ended = Through { pressed: 12, on: true, until: ms("2026-10-05T22:30[Europe/Paris]") };
        assert!(decide(Some(&t), &Incoming { local: Some(ended), ..call("01 99 00 55 55", "23:00") }).refuse, "an hour that ended");
        // Nothing to decide from: it rings.
        assert_eq!(decide(None, &call("01 99 00 55 55", "23:00")).why, "no-table");
        assert_eq!(said(&Incoming { now: ms("2026-10-06T08:00[Europe/Paris]"), ..call("01 99 00 55 55", "23:00") }), (false, "no-frame", "stranger".to_string()));
        assert_eq!(said(&Incoming { outgoing: true, ..call("112", "23:00") }).1, "outgoing");
    }

    #[test]
    fn frames_follow_the_day_and_its_change_of_hour() {
        use crate::needs::{Days, Needs};
        let config: crate::config::Config = toml::from_str(
            &["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| format!("[[window]]\nday = \"{d}\"\nstart = \"09:00\"\nend = \"17:00\"\n\n")).collect::<String>(),
        )
        .unwrap();
        let needs = Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() };
        let clock = Clock::new(&config, crate::quiet::Overrides::default(), needs, Days::default(), Vec::new());
        let attention = Attention::usual();
        let none = Layers::default();
        let from = at("2026-10-05T00:00[Europe/Paris]");
        let until = at("2026-10-06T00:00[Europe/Paris]");
        let frames = frames(&attention, &clock, &none, &from, &until);
        // From midnight to midnight, end to end.
        assert_eq!(frames.first().unwrap().from, from.timestamp().as_millisecond());
        assert_eq!(frames.last().unwrap().until, until.timestamp().as_millisecond());
        assert!(frames.windows(2).all(|w| w[0].until == w[1].from), "{frames:?}");
        let table = Table { frames, ..table(Made { made: 0, region: fr(), numbers: Vec::new(), prefixes: Vec::new(), always: Vec::new(), through: None, frames: Vec::new() }) };
        let at_ = |time: &str| table.frame_at(ms(&format!("2026-10-05T{time}[Europe/Paris]"))).unwrap();
        // The usual Calls matrix: safe all but sleep and the pause; neutral work and admin; restricted work; strangers never; hidden work and admin.
        for (time, column, ringing) in [
            ("03:00", "sleep", vec![]),
            ("08:00", "meals", vec!["safe"]),
            ("10:00", "work", vec!["safe", "neutral", "restricted", "hidden"]),
            ("12:30", "meals", vec!["safe"]),
            ("14:10", "sleep", vec![]),
            ("18:00", "leisure", vec!["safe"]),
            ("22:30", "sleep", vec![]),
        ] {
            let f = at_(time);
            assert_eq!(f.column, column, "{time}");
            let rings: Vec<&str> = ["safe", "neutral", "restricted", "stranger", "hidden"].into_iter().filter(|r| f.ring[*r]).collect();
            assert_eq!(rings, ringing, "{time}");
            assert!(f.ring["always-stranger"], "Always through rings at any time: {time}");
        }
        // The layers (Q11): do-not-disturb's switch from 18:00 to 19:00 sends the safe to voicemail, Always through rings;
        // a slot of time for you holds no call.
        let (dnd_from, dnd_until) = (at("2026-10-05T18:00[Europe/Paris]").timestamp().as_second(), at("2026-10-05T19:00[Europe/Paris]").timestamp().as_second());
        let layers = Layers { slots: vec![(dnd_until + 600, dnd_until + 1800)], dnd: Some((dnd_from, Some(dnd_until))) };
        let table = Table { frames: super::frames(&attention, &clock, &layers, &from, &until), ..table.clone() };
        let at_ = |time: &str| table.frame_at(ms(&format!("2026-10-05T{time}[Europe/Paris]"))).unwrap();
        assert_eq!((at_("18:30").column.as_str(), at_("18:30").ring["safe"], at_("18:30").ring["always-safe"]), ("dnd", false, true));
        assert_eq!((at_("17:30").column.as_str(), at_("17:30").ring["safe"]), ("leisure", true));
        assert_eq!((at_("19:15").column.as_str(), at_("19:15").ring["safe"]), ("slot", true));
        // Free time as cells: the safe ring in it, the others not; "Nothing at all", only Always through.
        let free = crate::quiet::Overrides { free_since: Some(at("2026-10-05T10:00[Europe/Paris]").timestamp().as_second()), free_nothing: Some(true), ..crate::quiet::Overrides::default() };
        let clock_free = Clock::new(&config, free, Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() }, Days::default(), Vec::new());
        let table = Table { frames: super::frames(&attention, &clock_free, &none, &from, &until), ..table.clone() };
        let f = table.frame_at(ms("2026-10-05T11:00[Europe/Paris]")).unwrap();
        assert_eq!((f.column.as_str(), f.ring["safe"], f.ring["always-neutral"]), ("free", false, true));
        // The night the clocks go back (25 October 2026): no gap, no overlap, 25 hours that day.
        let from = at("2026-10-25T00:00[Europe/Paris]");
        let until = at("2026-10-26T00:00[Europe/Paris]");
        let frames = super::frames(&attention, &clock, &none, &from, &until);
        assert!(frames.windows(2).all(|w| w[0].until == w[1].from));
        assert_eq!(frames.last().unwrap().until - frames.first().unwrap().from, 25 * 3_600_000);
        // In the pause: the pause's column to the end, nobody ringing.
        let paused = crate::quiet::Overrides { paused_since: Some(from.timestamp().as_second()), ..crate::quiet::Overrides::default() };
        let clock = Clock::new(&config, paused, Needs::default(), Days::default(), Vec::new());
        let frames = super::frames(&attention, &clock, &none, &from, &until);
        assert!(frames.iter().all(|f| f.column == "pause" && f.ring.iter().all(|(row, r)| *r == row.starts_with("always-"))), "{frames:?}");
        assert_eq!(frames.last().unwrap().until, until.timestamp().as_millisecond());
    }

    #[test]
    fn numbers_shown_as_people_write_them() {
        assert_eq!(shown_number("+33199001234", fr()), "01 99 00 12 34");
        assert_eq!(shown_number("+262692123456", fr()), "06 92 12 34 56");
        assert_eq!(shown_number("+442079460018", fr()), "+442079460018");
        assert_eq!(shown_number("112", fr()), "112");
        assert_eq!(shown_number("+442079460018", phones::region_named("GB")), "02079460018");
        assert_eq!(shown_number("+33199001234", None), "+33199001234");
    }

    #[test]
    fn the_floors_hold_emergency_numbers_and_the_list_s_people() {
        let keys = emergency_keys(fr());
        for number in ["112", "15", "114", "3114", "+33800112112", "999", "911", "988"] {
            assert!(keys.iter().any(|k| k == number), "{number} in {keys:?}");
        }
        let mut people = People::default();
        people.add(Person { name: "Alice".into(), phones: vec!["01 99 00 00 01".into(), "3114".into()], ..Person::default() }, fr());
        assert_eq!(people_keys(&people, fr()), vec!["+33199000001".to_string()], "a short code is nobody's");
        assert_eq!(always_numbers(&people, fr(), &|_| Who::Neutral), vec![("+33199000001".to_string(), "always-neutral".to_string())]);
        assert!(always_numbers(&people, fr(), &|_| Who::Blocked).is_empty(), "blocked beats Always through");
        // 0639 is Mayotte's plan: its own country code, as phones reads it.
        assert_eq!(phones::key("06 39 98 00 01", fr()), "+262639980001");
    }

    fn frame(from: i64, until: i64, column: &str, ringing: &[&str]) -> Frame {
        Frame { from, until, column: column.into(), ring: ROWS.iter().map(|r| (r.to_string(), ringing.contains(r))).collect() }
    }

    #[test]
    fn the_table_says_what_java_needs_and_nothing_twice() {
        let mut people = People::default();
        people.add(Person { name: "Alice".into(), phones: vec!["01 99 00 00 01".into()], ..Person::default() }, fr());
        let made = |at: i64| Made {
            made: at,
            region: fr(),
            numbers: vec![("+262639980002".into(), "safe".into()), ("+33199001234".into(), "blocked".into()), ("+33199009999".into(), "stranger".into()), (String::new(), "safe".into())],
            prefixes: vec![("+33465711*".into(), "blocked".into()), ("+3346571*".into(), "safe".into()), ("+334657112*".into(), "neutral".into()), ("*".into(), "blocked".into())],
            always: always_numbers(&people, fr(), &|_| Who::Safe),
            through: Some(Through { pressed: 5, on: true, until: 0 }),
            frames: vec![frame(0, 1_000, "work", &["safe", "neutral", "restricted", "hidden"]), frame(1_000, 2_000, "sleep", &[])],
        };
        let table = table(made(42));
        assert_eq!(table.v, VERSION);
        assert_eq!(table.numbers.len(), 3, "strangers and empty keys are not written: {:?}", table.numbers);
        assert_eq!((table.numbers["+33199001234"].as_str(), table.numbers["+33199000001"].as_str()), ("blocked", "always-safe"));
        // Prefixes without their star, the longest first; a bare star is nothing.
        assert_eq!(table.prefixes.iter().map(|p| (p.prefix.as_str(), p.who.as_str())).collect::<Vec<_>>(), [("+334657112", "neutral"), ("+33465711", "blocked"), ("+3346571", "safe")]);
        assert!(table.floors.people.is_empty());
        assert!(table.floors.emergency.contains(&"+33800112112".to_string()));
        assert_eq!(table.region.as_ref().map(|r| (r.calling.as_str(), r.trunk.as_str(), r.digits)), Some(("33", "0", [10, 10])));
        assert_eq!(table.region.as_ref().unwrap().overseas.get("692").map(String::as_str), Some("262"));
        assert!(table.trunk_zero.contains(&"33".to_string()) && !table.trunk_zero.contains(&"39".to_string()));
        assert_eq!((table.repeat_minutes, table.emergency_hours, table.phone_contacts.as_str()), (15, 24, "neutral"));
        // Made at another time but saying the same: the same content, not written again.
        assert_eq!(content(&table), content(&self::table(made(43))));
        // As Java reads it: the keys it looks for.
        let json: serde_json::Value = serde_json::from_str(&serde_json::to_string(&table).unwrap()).unwrap();
        for key in ["v", "made", "region", "trunk_zero", "numbers", "prefixes", "phone_contacts", "floors", "through", "frames", "repeat_minutes", "emergency_hours"] {
            assert!(json.get(key).is_some(), "{key}");
        }
        assert_eq!(json["frames"][0]["ring"]["hidden"], true);
        assert_eq!(json["frames"][1]["column"], "sleep");
        assert_eq!(json["through"]["on"], true);
        // Written whole, read back the same.
        let dir = std::env::temp_dir().join(format!("sioul-calls-table-{}", std::process::id()));
        let path = dir.join(TABLE);
        write(&path, &table).unwrap();
        let back: Table = read_json(&path).unwrap();
        assert_eq!(back, table);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_latest_press_anywhere_lets_calls_through() {
        let mut switch = Switch::default();
        assert_eq!(through_of(&switch), None);
        // The computer: every call for an hour.
        let stamp = press(&mut switch, "desk", true, 1_000 + 3_600_000, 1_000);
        assert_eq!(stamp, 1_000);
        let t = through_of(&switch).unwrap();
        assert!(t.holds(2_000) && !t.holds(1_000 + 3_600_000));
        // The phone's clock is behind: its "off" still comes after the press it knew of.
        let stamp = press(&mut switch, "phone", false, 0, 500);
        assert_eq!(stamp, 1_001);
        assert!(!through_of(&switch).unwrap().holds(2_000));
        // Until turned off.
        press(&mut switch, "desk", true, 0, 9_000);
        assert!(through_of(&switch).unwrap().holds(i64::MAX - 1));
        // The DND part of the device's table is untouched; the calls part reads back.
        assert!(switch.device["desk"].pressed == 0 && !switch.device["desk"].on);
        let text = toml::to_string(&switch).unwrap();
        assert!(text.contains("[device.desk.calls]"), "{text}");
        let back: Switch = toml::from_str(&text).unwrap();
        assert_eq!(through_of(&back), through_of(&switch));
    }

    #[test]
    fn the_phone_s_own_press_is_carried_once_with_its_own_time() {
        let mut switch = Switch::default();
        // The phone's notification pressed at 10:00; the computer turned it off at 10:30.
        let local = Through { pressed: 36_000_000, on: true, until: 36_000_000 + 3_600_000 };
        press(&mut switch, "desk", false, 0, 37_800_000);
        assert!(merge_local(&mut switch, "phone", &local));
        assert!(!merge_local(&mut switch, "phone", &local), "once");
        // Carried with its own time: the later "off" still holds.
        let held = through_of(&switch).unwrap();
        assert!(!held.on && held.pressed == 37_800_000);
        // A press after it, from the phone again: it holds.
        let later = Through { pressed: 38_000_000, on: true, until: 0 };
        assert!(merge_local(&mut switch, "phone", &later));
        assert!(through_of(&switch).unwrap().holds(40_000_000));
        // Which device screens: said in its part.
        assert!(!screens_anywhere(&switch));
        assert!(set_screens(&mut switch, "phone", true) && !set_screens(&mut switch, "phone", true));
        assert!(screens_anywhere(&switch));
    }

    fn held(at: i64, key: &str, who: &str, column: &str) -> Held {
        Held { at, key: key.into(), number: key.into(), hidden: key.is_empty(), who: if key.is_empty() { "hidden".into() } else { who.into() }, column: column.into(), why: "matrix".into(), ..Held::default() }
    }

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    fn ms(text: &str) -> i64 {
        at(text).timestamp().as_millisecond()
    }

    #[test]
    fn the_list_says_who_called_when_calmly() {
        let now = at("2026-10-07T10:00[Europe/Paris]");
        let calls = vec![
            held(ms("2026-10-07T06:10[Europe/Paris]"), "+33199001234", "stranger", "sleep"),
            held(ms("2026-10-07T06:21[Europe/Paris]"), "+33199001234", "stranger", "sleep"),
            held(ms("2026-10-07T07:00[Europe/Paris]"), "", "hidden", "sleep"),
            held(ms("2026-10-06T23:10[Europe/Paris]"), "+262639980002", "neutral", "sleep"),
            held(ms("2026-10-07T09:30[Europe/Paris]"), "+33199005555", "blocked", "work"),
            held(ms("2026-10-05T12:30[Europe/Paris]"), "+33199007777", "stranger", "meals"),
            held(ms("2026-10-05T12:31[Europe/Paris]"), "+33199007777", "stranger", "meals"),
            held(ms("2026-10-05T12:40[Europe/Paris]"), "+33199007777", "stranger", "meals"),
        ];
        let names = |key: &str| (key == "+262639980002").then(|| "Dr Martin's office".to_string());
        let all = |_: &Held| true;
        let here = |_: &str| String::new();
        for (language, expected) in [
            (
                "en",
                [
                    "Monday 5 October, during a meal, a number not in your contacts called three times, the last at 12:40.",
                    "Yesterday, while you slept, Dr Martin's office called at 23:10.",
                    "While you slept, a number not in your contacts called twice, at 06:10 and 06:21.",
                    "While you slept, a hidden number called at 07:00.",
                ],
            ),
            (
                "fr",
                [
                    "Lundi 5 octobre, pendant un repas, un numéro absent de vos contacts a appelé trois fois, la dernière à 12:40.",
                    "Hier, pendant votre sommeil, Dr Martin's office a appelé à 23:10.",
                    "Pendant votre sommeil, un numéro absent de vos contacts a appelé deux fois, à 06:10 et à 06:21.",
                    "Pendant votre sommeil, un numéro masqué a appelé à 07:00.",
                ],
            ),
        ] {
            let tr = Translator::new(language);
            let l = Lister { now: &now, tr: &tr, region: fr(), name_of: &names, shows: &all, phone: &here };
            let lines = lines(&calls, &Seen::default(), &BTreeMap::new(), &l);
            assert_eq!(lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), expected, "{language}");
            // Never the blocked.
            assert!(lines.iter().all(|l| !l.dial.contains("5555")));
            // The doubt said, never "no message".
            assert!(lines.iter().all(|l| !l.doubt.is_empty()));
        }
        let tr = Translator::new("en");
        let l = Lister { now: &now, tr: &tr, region: fr(), name_of: &names, shows: &all, phone: &here };
        let lines_en = lines(&calls, &Seen::default(), &BTreeMap::new(), &l);
        let stranger = &lines_en[2];
        assert_eq!((stranger.number.as_str(), stranger.dial.as_str(), stranger.known, stranger.hidden), ("01 99 00 12 34", "+33199001234", false, false));
        assert_eq!(stranger.doubt, "They may have left a message.");
        assert_eq!(stranger.why, "Numbers not in your contacts go to voicemail while you slept.");
        assert_eq!(stranger.ids.len(), 2);
        let hidden = &lines_en[3];
        assert!(hidden.hidden && hidden.number.is_empty() && hidden.dial.is_empty());
        assert!(lines_en[1].known);
        assert_eq!(lines_en[1].why, "Calls from your neutral contacts go to voicemail while you slept.");
        // Someone Always through, declined (their row as their own, which held them then): said as their own row.
        let always = [held(ms("2026-10-07T06:40[Europe/Paris]"), "+262639980002", "always-neutral", "sleep")];
        assert_eq!(lines(&always, &Seen::default(), &BTreeMap::new(), &l)[0].why, "Calls from your neutral contacts go to voicemail while you slept.");
        // Seen: gone; the others stay.
        let mut seen = Seen::default();
        seen.add(&stranger.ids, now.timestamp().as_millisecond());
        assert_eq!(lines(&calls, &seen, &BTreeMap::new(), &l).len(), 3);
        // Shown only when their callers may reach you now.
        let contacts_only = |h: &Held| h.who == "neutral";
        let l2 = Lister { shows: &contacts_only, ..l };
        assert_eq!(lines(&calls, &Seen::default(), &BTreeMap::new(), &l2).len(), 1);
        // A voicemail linked: said, and no doubt.
        let mut messages = BTreeMap::new();
        messages.insert(stranger.ids[1].clone(), Message { text: message_words(&tr, Some(42)), path: "/mail/1".into(), sound: Some(0) });
        let with = lines(&calls, &Seen::default(), &messages, &l);
        assert_eq!(with[2].message.as_ref().map(|m| m.text.as_str()), Some("They left a message (0:42)."));
        assert!(with[2].doubt.is_empty());
    }

    #[test]
    fn old_calls_leave_the_list_and_seen_ids_are_forgotten_with_them() {
        let now = at("2026-10-30T10:00[Europe/Paris]");
        let calls = vec![held(ms("2026-10-07T06:10[Europe/Paris]"), "+33199001234", "stranger", "sleep")];
        let tr = Translator::new("en");
        let all = |_: &Held| true;
        let none = |_: &str| None;
        let here = |_: &str| String::new();
        let l = Lister { now: &now, tr: &tr, region: fr(), name_of: &none, shows: &all, phone: &here };
        assert!(lines(&calls, &Seen::default(), &BTreeMap::new(), &l).is_empty(), "past two weeks");
        let mut seen = Seen::default();
        seen.add(&["1000-+33199001234".to_string(), format!("{}-x", now.timestamp().as_millisecond())], now.timestamp().as_millisecond());
        assert_eq!(seen.seen.len(), 1);
    }

    #[test]
    fn the_held_file_reads_whatever_java_left_in_it() {
        let dir = std::env::temp_dir().join(format!("sioul-calls-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(HELD);
        std::fs::write(&path, "{\"at\":2000,\"key\":\"+33199001234\",\"number\":\"0199001234\",\"hidden\":false,\"presentation\":1,\"who\":\"stranger\",\"name\":\"\",\"column\":\"sleep\",\"why\":\"matrix\",\"verified\":\"\",\"later\":1}\n{\"at\":1000,\"key\":\"\",\"hidden\":true,\"who\":\"hidden\"}\n{\"at\":30").unwrap();
        let held = read_held(&path);
        assert_eq!(held.iter().map(|h| h.at).collect::<Vec<_>>(), vec![1000, 2000], "oldest first, the half line passed over");
        assert_eq!(held[0].id(), "1000-hidden");
        assert_eq!(held[1].id(), "2000-+33199001234");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-calls-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A call that rang, or was declined, on a phone (`device`).
    fn call_on(device: &str, at: i64, key: &str, who: &str, column: &str, why: &str, rang: bool) -> Held {
        Held { device: device.into(), rang, why: why.into(), ..held(at, key, who, column) }
    }

    /// The line a phone's log keeps: what Java wrote, less the number as the
    /// network gave it and its presentation, every empty field left out; a
    /// line of Java's from before calls that rang were logged was declined.
    #[test]
    fn a_logged_line_reads_back() {
        let java: Held = serde_json::from_str(r#"{"at":1791346200000,"key":"+33199001234","number":"01 99 00 12 34","hidden":false,"presentation":1,"who":"stranger","name":"","column":"sleep","why":"matrix","verified":""}"#).unwrap();
        assert!(!java.rang, "declined, as every line was before");
        let logged = java.logged(fr());
        assert_eq!(serde_json::to_string(&logged).unwrap(), r#"{"at":1791346200000,"key":"+33199001234","who":"stranger","column":"sleep","why":"matrix","rang":false}"#);
        assert_eq!(logged.id(), java.id());
        // Java had no table to key it with: keyed as the country writes numbers.
        let unkeyed = Held { key: String::new(), why: "no-table".into(), rang: true, ..java.clone() };
        assert_eq!(unkeyed.logged(fr()).key, "+33199001234");
        // A hidden number keeps no key; a name and a verification are kept.
        let hidden = Held { at: 5, hidden: true, who: "hidden".into(), why: "matrix".into(), ..Held::default() }.logged(fr());
        assert_eq!(serde_json::to_string(&hidden).unwrap(), r#"{"at":5,"hidden":true,"who":"hidden","why":"matrix","rang":false}"#);
        let named = Held { name: "Cabinet du Dr Martin".into(), verified: "passed".into(), ..java }.logged(fr());
        let back: Held = serde_json::from_str(&serde_json::to_string(&named).unwrap()).unwrap();
        assert_eq!((back.name.as_str(), back.verified.as_str(), back.number.as_str()), ("Cabinet du Dr Martin", "passed", ""));
    }

    /// The lines Java writes (the JVM's LogCheck, where SIOUL_CALLS_SAMPLE
    /// says), read as Java's file is read here: declined; a second call that
    /// rang, its time of day said; hidden; no table, keyed here. Skipped
    /// without the sample.
    #[test]
    fn java_s_lines_read_here() {
        let Some(sample) = std::env::var_os("SIOUL_CALLS_SAMPLE") else { return };
        let calls = read_held(Path::new(&sample));
        assert_eq!(calls.iter().map(|h| (h.rang, h.why.as_str(), h.column.as_str(), h.hidden)).collect::<Vec<_>>(), [(false, "matrix", "sleep", false), (true, "repeat", "sleep", false), (false, "matrix", "sleep", true), (true, "no-table", "", false)]);
        assert_eq!((calls[0].key.as_str(), calls[0].who.as_str()), ("+33199001234", "stranger"));
        assert_eq!(calls[3].logged(fr()).key, "+33465710042");
        assert_eq!(calls[0].logged(fr()).number, "", "the number as given stays on the phone");
    }

    /// Every phone's log read together, each call with its phone, oldest
    /// first; never a blocked caller's, never one past the month.
    #[test]
    fn every_phone_s_log_is_read() {
        let root = scratch("logs");
        let now = ms("2026-10-07T10:00[Europe/Paris]");
        let write = |device: &str, lines: &[Held]| {
            let text: String = lines.iter().map(|h| serde_json::to_string(&h.logged(fr())).unwrap() + "\n").collect();
            std::fs::create_dir_all(root.join(LOG)).unwrap();
            std::fs::write(own_file(&root, LOG, device), text).unwrap();
        };
        write("phone-a", &[held(ms("2026-10-07T06:10[Europe/Paris]"), "+33199001234", "stranger", "sleep"), held(ms("2026-10-07T09:00[Europe/Paris]"), "+33199005555", "blocked", "work")]);
        write("phone-b", &[held(ms("2026-10-07T06:00[Europe/Paris]"), "+33465710042", "safe", "sleep"), held(ms("2026-09-01T09:00[Europe/Paris]"), "+33465710042", "safe", "work")]);
        std::fs::write(root.join(LOG).join(".phone-c.jsonl.lock"), "").unwrap();
        let calls = read_logs(&root, now);
        assert_eq!(calls.iter().map(|h| (h.device.as_str(), h.key.as_str())).collect::<Vec<_>>(), [("phone-b", "+33465710042"), ("phone-a", "+33199001234")]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// On the phone that screened, a line says "called at"; on another
    /// device, which phone: "your phone", "your other phone" on a phone, or
    /// its name when several phones share their calls.
    #[test]
    fn the_list_names_the_phone_elsewhere() {
        let now = at("2026-10-07T10:00[Europe/Paris]");
        let calls = vec![call_on("phone-a", ms("2026-10-07T06:10[Europe/Paris]"), "+33199001234", "stranger", "sleep", "matrix", false)];
        let none = |_: &str| None;
        let all = |_: &Held| true;
        let one = [("phone-a".to_string(), "A12".to_string())];
        let two = [("phone-a".to_string(), "A12".to_string()), ("phone-b".to_string(), "B7".to_string())];
        for (language, own, computer, other, named) in [
            (
                "en",
                "While you slept, a number not in your contacts called at 06:10.",
                "While you slept, a number not in your contacts called your phone at 06:10.",
                "While you slept, a number not in your contacts called your other phone at 06:10.",
                "While you slept, a number not in your contacts called your phone (A12) at 06:10.",
            ),
            (
                "fr",
                "Pendant votre sommeil, un numéro absent de vos contacts a appelé à 06:10.",
                "Pendant votre sommeil, un numéro absent de vos contacts a appelé votre téléphone à 06:10.",
                "Pendant votre sommeil, un numéro absent de vos contacts a appelé votre autre téléphone à 06:10.",
                "Pendant votre sommeil, un numéro absent de vos contacts a appelé votre téléphone (A12) à 06:10.",
            ),
        ] {
            let tr = Translator::new(language);
            let said = |here: &str, phones: &[(String, String)], phone: bool| {
                let words = |device: &str| phone_words(&tr, device, here, phones, phone);
                let l = Lister { now: &now, tr: &tr, region: fr(), name_of: &none, shows: &all, phone: &words };
                lines(&calls, &Seen::default(), &BTreeMap::new(), &l)[0].text.clone()
            };
            assert_eq!(said("phone-a", &one, true), own, "{language}: on the phone that screened");
            assert_eq!(said("desk", &one, false), computer, "{language}: on a computer");
            assert_eq!(said("desk", &[], false), computer, "{language}: without the devices' list");
            assert_eq!(said("phone-b", &two, true), other, "{language}: on the other phone");
            assert_eq!(said("desk", &two, false), named, "{language}: two phones, from a computer");
        }
    }

    /// A call that rang is never a line of its own; after declined calls of
    /// the same number the same day, it is said under their line; never for
    /// a hidden number. A message left after it goes with the line.
    #[test]
    fn a_call_that_rang_is_never_listed_but_said() {
        let now = at("2026-10-07T10:00[Europe/Paris]");
        let calls = vec![
            call_on("phone-a", ms("2026-10-07T06:10[Europe/Paris]"), "+33199001234", "stranger", "sleep", "matrix", false),
            call_on("phone-a", ms("2026-10-07T06:40[Europe/Paris]"), "+33199001234", "stranger", "sleep", "matrix", false),
            call_on("phone-a", ms("2026-10-07T06:48[Europe/Paris]"), "+33199001234", "stranger", "sleep", "repeat", true),
            call_on("phone-a", ms("2026-10-07T07:00[Europe/Paris]"), "", "hidden", "sleep", "matrix", false),
            call_on("phone-a", ms("2026-10-07T07:05[Europe/Paris]"), "", "hidden", "sleep", "repeat", true),
            call_on("phone-a", ms("2026-10-07T09:15[Europe/Paris]"), "+33465710042", "safe", "work", "matrix", true),
        ];
        let none = |_: &str| None;
        let all = |_: &Held| true;
        let here = |_: &str| String::new();
        for (language, first, again) in [
            ("en", "While you slept, a number not in your contacts called twice, at 06:10 and 06:40.", "The same number called again at 06:48, and that call rang."),
            ("fr", "Pendant votre sommeil, un numéro absent de vos contacts a appelé deux fois, à 06:10 et à 06:40.", "Le même numéro a rappelé à 06:48, et cet appel a sonné."),
        ] {
            let tr = Translator::new(language);
            let l = Lister { now: &now, tr: &tr, region: fr(), name_of: &none, shows: &all, phone: &here };
            let said = lines(&calls, &Seen::default(), &BTreeMap::new(), &l);
            assert_eq!(said.len(), 2, "{language}: the stranger's and the hidden number's; never a call that rang");
            assert_eq!((said[0].text.as_str(), said[0].again.as_str()), (first, again), "{language}");
            assert_eq!(said[1].again, "", "{language}: two hidden calls cannot be told apart");
        }
        // The message left after the call that rang goes with the line.
        let tr = Translator::new("en");
        let l = Lister { now: &now, tr: &tr, region: fr(), name_of: &none, shows: &all, phone: &here };
        let mut messages = BTreeMap::new();
        messages.insert(calls[2].id(), Message { text: message_words(&tr, Some(12)), path: "/mail/2".into(), sound: Some(0) });
        let said = lines(&calls, &Seen::default(), &messages, &l);
        assert_eq!((said[0].message.as_ref().map(|m| m.text.as_str()), said[0].doubt.as_str()), (Some("They left a message (0:12)."), ""));
        // The next day's call that rang is no longer said under that line.
        let later = [calls[0].clone(), call_on("phone-a", ms("2026-10-08T08:00[Europe/Paris]"), "+33199001234", "stranger", "work", "matrix", true)];
        let tomorrow = at("2026-10-08T10:00[Europe/Paris]");
        let l = Lister { now: &tomorrow, ..l };
        assert_eq!(lines(&later, &Seen::default(), &BTreeMap::new(), &l)[0].again, "");
    }

    /// Seen on one device, seen on all: each device's own log, every
    /// device's read together; Undo takes out this device's own line only.
    /// The phone's file from before Seen was shared becomes a line of its own log.
    #[test]
    fn seen_anywhere_is_seen_everywhere() {
        let root = scratch("seen");
        let a = "1791346200000-+33199001234".to_string();
        let b = "1791349200000-hidden".to_string();
        let pressed = mark_seen(&root, "desk", std::slice::from_ref(&a), 1_000).unwrap();
        mark_seen(&root, "phone-a", &[a.clone(), b.clone()], 2_000).unwrap();
        assert_eq!(read_seen(&root).seen, [a.clone(), b.clone()].into());
        // Undo on the desk: the phone's Seen stays.
        assert!(unmark_seen(&root, "desk", pressed).unwrap());
        assert!(!unmark_seen(&root, "desk", pressed).unwrap(), "once");
        assert_eq!(read_seen(&root).seen, [a.clone(), b.clone()].into());
        assert!(unmark_seen(&root, "phone-a", 2_000).unwrap());
        assert!(read_seen(&root).seen.is_empty());
        // The older file, this phone's alone: a line of its own log, then gone.
        std::fs::write(root.join(SEEN), "seen = [\"1791346200000-+33199001234\"]\n").unwrap();
        adopt_older_seen(&root, "phone-a", 3_000).unwrap();
        assert!(!root.join(SEEN).exists());
        assert_eq!(read_seen(&root).seen, [a].into());
        adopt_older_seen(&root, "phone-a", 4_000).unwrap();
        assert_eq!(std::fs::read_to_string(own_file(&root, SEEN_LOG, "phone-a")).unwrap().lines().count(), 1, "nothing twice");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(own_file(&root, SEEN_LOG, "phone-a")).unwrap().permissions().mode() & 0o777, 0o600, "yours alone");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A person's calls of the month on their sheet: newest first, rang or
    /// declined, why a call rang when its row alone would not say it, the
    /// phone named only when several share their calls; never a blocked
    /// caller's, nothing past the month.
    #[test]
    fn a_person_s_calls_of_the_month() {
        let now = at("2026-10-07T10:30[Europe/Paris]");
        let calls = vec![
            call_on("phone-a", ms("2026-09-01T09:00[Europe/Paris]"), "+262639980002", "neutral", "work", "matrix", true),
            call_on("phone-a", ms("2026-10-06T23:10[Europe/Paris]"), "+262639980002", "neutral", "sleep", "matrix", false),
            call_on("phone-a", ms("2026-10-07T06:48[Europe/Paris]"), "+262639980002", "neutral", "sleep", "repeat", true),
            call_on("phone-a", ms("2026-10-07T08:00[Europe/Paris]"), "+33199005555", "blocked", "work", "blocked", false),
            call_on("phone-a", ms("2026-10-07T10:05[Europe/Paris]"), "+262639980002", "neutral", "work", "matrix", true),
        ];
        let keys: BTreeSet<String> = ["+262639980002".to_string(), "+33199005555".to_string()].into();
        let none = |_: &str| None;
        let all = |_: &Held| true;
        for (language, expected) in [
            ("en", ["Today at 10:05, during work: it rang.", "Today at 06:48, while you slept: it rang. A second call within 15 minutes rings.", "Yesterday at 23:10, while you slept: declined, sent to voicemail."]),
            ("fr", ["Aujourd’hui à 10:05, pendant le travail\u{202f}: l’appel a sonné.", "Aujourd’hui à 06:48, pendant votre sommeil\u{202f}: l’appel a sonné. Un deuxième appel en moins de 15 minutes sonne.", "Hier à 23:10, pendant votre sommeil\u{202f}: refusé, envoyé sur votre messagerie."]),
        ] {
            let tr = Translator::new(language);
            let words = |device: &str| phone_words(&tr, device, "desk", &[("phone-a".to_string(), "A12".to_string())], false);
            let l = Lister { now: &now, tr: &tr, region: fr(), name_of: &none, shows: &all, phone: &words };
            assert_eq!(history(&calls, &keys, &l), expected, "{language}");
        }
        // Two phones: each line names its phone.
        let mut both = calls.clone();
        both.push(call_on("phone-b", ms("2026-10-07T09:00[Europe/Paris]"), "+262639980002", "neutral", "work", "matrix", false));
        both.sort_by_key(|h| h.at);
        let tr = Translator::new("en");
        let phones = [("phone-a".to_string(), "A12".to_string()), ("phone-b".to_string(), "B7".to_string())];
        let words = |device: &str| phone_words(&tr, device, "desk", &phones, false);
        let l = Lister { now: &now, tr: &tr, region: fr(), name_of: &none, shows: &all, phone: &words };
        let said = history(&both, &keys, &l);
        assert_eq!(&said[..2], ["Today at 10:05, during work: it rang on your phone (A12).", "Today at 09:00, during work: declined on your phone (B7), sent to voicemail."]);
    }

    /// Java's calls copied into this phone's own log once, whoever copies;
    /// never a blocked caller's, nor one past the month; this device's own
    /// lines taken out after the month, and never copied back.
    #[test]
    fn old_lines_leave_with_their_writer() {
        let root = scratch("carry");
        let own = own_file(&root, LOG, "phone-a");
        let now = ms("2026-10-07T10:00[Europe/Paris]");
        let java = vec![
            Held { number: "01 99 00 12 34".into(), ..held(ms("2026-10-07T06:10[Europe/Paris]"), "+33199001234", "stranger", "sleep") },
            Held { key: String::new(), number: "0465710042".into(), rang: true, why: "no-table".into(), ..held(ms("2026-10-07T09:15[Europe/Paris]"), "x", "", "") },
            held(ms("2026-10-07T09:30[Europe/Paris]"), "+33199005555", "blocked", "work"),
            held(ms("2026-08-20T09:30[Europe/Paris]"), "+33199007777", "stranger", "work"),
        ];
        assert_eq!(carry_into(&own, &java, fr(), now).unwrap(), 2);
        assert_eq!(carry_into(&own, &java, fr(), now).unwrap(), 0, "once");
        let logged = read_held(&own);
        assert_eq!(logged.iter().map(|h| (h.key.as_str(), h.rang, h.number.as_str())).collect::<Vec<_>>(), [("+33199001234", false, ""), ("+33465710042", true, "")]);
        // A month and a few days later: taken out of this device's own files, a line that does not read kept.
        mark_seen(&root, "phone-a", &[logged[0].id()], now).unwrap();
        std::fs::write(&own, std::fs::read_to_string(&own).unwrap() + "{\"at\":\n").unwrap();
        let later = now + 33 * DAY_MS;
        assert_eq!(trim_own(&root, "phone-a", later).unwrap(), 3);
        assert_eq!(std::fs::read_to_string(&own).unwrap(), "{\"at\":\n");
        assert!(read_seen(&root).seen.is_empty());
        assert_eq!(carry_into(&own, &java, fr(), later).unwrap(), 0, "never copied back");
        // Nobody else's file is touched, until a line of theirs is long past its month (its phone gone for good).
        std::fs::write(own_file(&root, LOG, "phone-b"), serde_json::to_string(&java[0].logged(fr())).unwrap() + "\n").unwrap();
        trim_own(&root, "phone-a", later).unwrap();
        assert_eq!(trim_others(&root, "phone-a", later).unwrap(), 0);
        assert_eq!(read_held(&own_file(&root, LOG, "phone-b")).len(), 1);
        assert_eq!(trim_others(&root, "phone-a", now + (KEPT_DAYS + LATE_DAYS + 1) * DAY_MS).unwrap(), 1);
        assert!(read_held(&own_file(&root, LOG, "phone-b")).is_empty());
        assert_eq!(std::fs::read_to_string(&own).unwrap(), "{\"at\":\n", "its own left to trim_own");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn letting_calls_through_is_said_with_its_end() {
        let now = 10_000_000;
        let when = |ms: i64| format!("<{ms}>");
        for (language, hour, open, emergency) in [
            ("en", "Every call rings until <13600000>.", "Every call rings until you turn it off.", "Every call rings until <92810000>: you called an emergency number."),
            ("fr", "Tous les appels sonnent jusqu’à <13600000>.", "Tous les appels sonnent jusqu’à ce que vous l’arrêtiez.", "Tous les appels sonnent jusqu’à <92810000>\u{202f}: vous avez appelé un numéro d’urgence."),
        ] {
            let tr = Translator::new(language);
            let on = Through { pressed: 1, on: true, until: now + 3_600_000 };
            assert_eq!(through_line(&tr, Some(&on), 0, now, &when), hour);
            assert_eq!(through_line(&tr, Some(&Through { until: 0, ..on }), 0, now, &when), open);
            assert_eq!(through_line(&tr, Some(&Through { on: false, ..on }), 0, now, &when), "");
            assert_eq!(through_line(&tr, None, now - 3_590_000, now, &when), emergency);
            assert_eq!(through_line(&tr, None, now - 25 * 3_600_000, now, &when), "", "a day after");
        }
    }

    #[test]
    fn every_word_has_both_languages() {
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let mut keys: Vec<String> = [
                "calls-who-hidden", "calls-who-stranger", "calls-context-day", "calls-day-yesterday", "calls-day-today", "calls-line-once", "calls-line-twice", "calls-line-more", "calls-line-once-on", "calls-line-twice-on", "calls-line-more-on", "calls-may-have-left", "calls-left-message", "calls-left-message-plain", "calls-through-emergency", "calls-through-until", "calls-through-on", "calls-context-any",
                "calls-phone-yours", "calls-phone-other", "calls-phone-named", "calls-phone-this", "calls-again-rang", "calls-history-title", "calls-history-when", "calls-history-head", "calls-history-line", "calls-history-declined", "calls-history-rang", "calls-history-declined-on", "calls-history-rang-on",
                "calls-rang-repeat", "calls-rang-through", "calls-rang-emergency", "calls-rang-after-emergency", "calls-rang-undecided",
            ]
            .iter()
            .map(|k| k.to_string())
            .collect();
            keys.extend(["work", "admin", "leisure", "meals", "sleep", "pause", "free", "slot", "dnd"].iter().map(|c| format!("calls-context-{c}")));
            keys.extend(["safe", "neutral", "restricted", "stranger", "hidden"].iter().map(|r| format!("calls-why-{r}")));
            for key in keys {
                let mut args = crate::i18n::args();
                for name in ["context", "day", "who", "time", "first", "second", "count", "length", "until", "phone", "name", "when", "head", "outcome"] {
                    args.set(name, "X");
                }
                let words = tr.text(&key, Some(&args));
                assert!(!words.is_empty() && words != key && !words.contains('{'), "{language}: {key}: {words}");
                // French typography: a narrow no-break space before « : ; ? ! », the apostrophe ’.
                if language == "fr" {
                    assert!(![" :", " ;", " ?", " !", "'"].iter().any(|bad| words.contains(bad)), "{key}: {words}");
                }
            }
        }
    }
}

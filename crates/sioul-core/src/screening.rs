// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The call screening (docs/android.md, "Calls"): what becomes of a call
//! from a number nobody knows. It is spam protection, not only
//! distraction protection, so it stands apart from do-not-disturb
//! and may be on at all hours.
//! Its rules are decisions 3 to 5 of the design
//! (docs/attention.md, §10).
//!
//! **Apart**: the people you know (your lists, your address books, the
//! phone's contacts, a number you called lately, Always through) are decided
//! by the matrix of what reaches you, its Calls rows, the time and its layers
//! (`attention`); everyone else by the screening alone, which neither
//! do-not-disturb nor any state of yours switches off. What always rings
//! (emergency numbers and their callback, the day after you call one, a
//! second call within 15 minutes) and the blocked come before both
//! (`calls::decide`).
//!
//! **What it screens**, each kind on its own: a number in no address book
//! (`unknown`), a hidden number (`hidden`), an automated caller (`automated`:
//! ARCEP's roots for machines), a number that cannot be a caller's
//! (`impossible`: one the plan forbids as a caller ID, 089), a number whose
//! verification failed (`unverified`). The last three it keeps out at all
//! hours, a second call too, while they go to voicemail. A number you called
//! in the last 14 days counts as your contacts do (`called`). Its hours: at
//! all hours, or only outside your work and admin hours (`hours`), for
//! numbers nobody knows and hidden numbers. Whatever it lets ring by itself
//! never rings where your contacts' row declines (do-not-disturb, the night,
//! a pause): only Expecting a call opens beyond (`calls::decide`; review of
//! part 2, 19 October 2026). ARCEP's list of the blocks it had attributed to
//! no operator on a day decides nothing: a block attributed since may hold a
//! new line. A declined call's line only says it, while the list counts
//! (`unattributed_since`).
//!
//! **Kept** in `[screening]`, shared with the settings; each key unsaid takes
//! its first value: unknown and hidden numbers from your Calls rows of
//! strangers and hidden numbers as they were before the screening was apart
//! (never ringing where the row declined), the rest as decided. Written a key
//! at a time, only when you change one; the first written also fixes unknown
//! and hidden numbers at their first values, which the Calls rows then move
//! no more (`set`). No preset nor any write of the matrix touches those rows
//! (`attention::screened`). An older Sioul keeps the section as it is. The
//! numbers known by their kind: `data/screening-numbers.toml`.

use crate::attention::{Attention, Column, Level, Person, Row};
use crate::config::{Config, SettingValue};
use crate::reach::Channel;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// A number you called in the last this many days counts as your contacts do
/// (a guess, research 8.3, step 6).
pub const CALLED_DAYS: i64 = 14;

/// When the screening screens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hours {
    /// At all hours (the default).
    #[default]
    Always,
    /// Only outside your work and admin hours: in them, numbers nobody knows
    /// and hidden numbers ring where your contacts may; what it keeps out as
    /// spam stays out.
    OutsideWork,
}

impl Hours {
    pub const ALL: [Hours; 2] = [Hours::Always, Hours::OutsideWork];

    pub fn id(self) -> &'static str {
        match self {
            Hours::Always => "always",
            Hours::OutsideWork => "outside-work",
        }
    }

    pub fn read(text: &str) -> Option<Hours> {
        Hours::ALL.into_iter().find(|h| h.id() == text.trim())
    }
}

/// What becomes of a call from a number in no address book, or of a hidden number.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rule {
    /// To your voicemail: declined, as you would by hand.
    #[default]
    Voicemail,
    /// It rings in your work and admin hours (a hospital's switchboard calls
    /// then), and goes to your voicemail otherwise.
    Work,
    /// It rings.
    Ring,
}

impl Rule {
    pub const ALL: [Rule; 3] = [Rule::Voicemail, Rule::Work, Rule::Ring];

    pub fn id(self) -> &'static str {
        match self {
            Rule::Voicemail => "voicemail",
            Rule::Work => "work",
            Rule::Ring => "ring",
        }
    }

    pub fn read(text: &str) -> Option<Rule> {
        Rule::ALL.into_iter().find(|r| r.id() == text.trim())
    }
}

/// What becomes of an automated caller, a number that cannot be a caller's,
/// a number whose verification failed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Spam {
    /// To your voicemail, even while you expect a call.
    #[default]
    Voicemail,
    /// As any number nobody knows.
    Unknown,
}

impl Spam {
    pub const ALL: [Spam; 2] = [Spam::Voicemail, Spam::Unknown];

    pub fn id(self) -> &'static str {
        match self {
            Spam::Voicemail => "voicemail",
            Spam::Unknown => "unknown",
        }
    }

    pub fn read(text: &str) -> Option<Spam> {
        Spam::ALL.into_iter().find(|s| s.id() == text.trim())
    }
}

/// `[screening]` as written, key by key: read leniently, so that a word
/// this version does not know (a later version's) never stops the
/// configuration from reading; that key then takes its first value.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct Settings(pub BTreeMap<String, toml::Value>);

impl Settings {
    fn word(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(toml::Value::as_str)
    }

    fn flag(&self, key: &str) -> Option<bool> {
        self.0.get(key).and_then(toml::Value::as_bool)
    }
}

/// The screening as it stands: `[screening]`, each key unsaid at its first value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Screening {
    /// Screening at all: off, every number nobody knows rings (the blocked aside).
    pub on: bool,
    pub hours: Hours,
    /// A number in none of your address books, Sioul's or the phone's.
    pub unknown: Rule,
    /// A call that shows no number.
    pub hidden: Rule,
    /// From ARCEP's roots for automated callers.
    pub automated: Spam,
    /// A number the plan forbids as a caller ID, or from a block attributed to nobody.
    pub impossible: Spam,
    /// A number whose verification by the network failed.
    pub unverified: Spam,
    /// A number you called in the last `CALLED_DAYS` days counts as your
    /// contacts do, and never rings less than a number nobody knows.
    pub called: bool,
}

impl Default for Screening {
    fn default() -> Screening {
        Screening::of_rows(&Attention::usual(), None)
    }
}

impl Screening {
    /// As your configuration says: `[screening]`, each key unsaid at its first
    /// value (`first_rule` of your Calls rows for unknown and hidden numbers).
    pub fn of(config: &Config) -> Screening {
        Screening::of_rows(&Attention::of(config), config.screening.as_ref())
    }

    fn of_rows(attention: &Attention, set: Option<&Settings>) -> Screening {
        let word = |key: &str| set.and_then(|s| s.word(key));
        let flag = |key: &str| set.and_then(|s| s.flag(key));
        Screening {
            on: flag("on").unwrap_or(true),
            hours: word("hours").and_then(Hours::read).unwrap_or_default(),
            unknown: word("unknown").and_then(Rule::read).unwrap_or_else(|| first_rule(attention, Person::Stranger)),
            hidden: word("hidden").and_then(Rule::read).unwrap_or_else(|| first_rule(attention, Person::Hidden)),
            automated: word("automated").and_then(Spam::read).unwrap_or_default(),
            impossible: word("impossible").and_then(Spam::read).unwrap_or_default(),
            unverified: word("unverified").and_then(Spam::read).unwrap_or_default(),
            called: flag("called").unwrap_or(true),
        }
    }

    /// Whether a number nobody knows, or a hidden one, rings at a time of the
    /// day (`hours`: work or admin hours then), as the screening says by
    /// itself: off, outside its hours, or its rule letting it ring. What it
    /// screens as spam aside, and Expecting a call, which holds for a while;
    /// never beyond your contacts' row then (`calls::decide`).
    pub fn rings(&self, hidden: bool, hours: bool) -> bool {
        if !self.on || (self.hours == Hours::OutsideWork && hours) {
            return true;
        }
        match if hidden { self.hidden } else { self.unknown } {
            Rule::Ring => true,
            Rule::Work => hours,
            Rule::Voicemail => false,
        }
    }
}

/// A rule's first value, from your Calls row of strangers or hidden numbers
/// as it was before the screening was apart: the closest rule that never
/// rings where the row declined. Ringing at every hour only when it rang at
/// every time; ringing in work and admin hours when it rang in either;
/// voicemail otherwise (review of part 2, F2: a row that rang in leisure alone
/// once gave ringing at night). Its layers (time for you, do-not-disturb) are
/// no part of the screening.
pub fn first_rule(attention: &Attention, person: Person) -> Rule {
    let row = Row::People(Channel::Calls, person);
    let rings = |column: Column| attention.cell(row, column) == Level::Now;
    if Column::TIMES.into_iter().all(|column| rings(column)) {
        Rule::Ring
    } else if rings(Column::Work) || rings(Column::Admin) {
        Rule::Work
    } else {
        Rule::Voicemail
    }
}

/// The keys of `[screening]` and the words each takes ("bool": on or off).
pub const KEYS: [(&str, &[&str]); 8] = [
    ("on", &["bool"]),
    ("hours", &["always", "outside-work"]),
    ("unknown", &["voicemail", "work", "ring"]),
    ("hidden", &["voicemail", "work", "ring"]),
    ("automated", &["voicemail", "unknown"]),
    ("impossible", &["voicemail", "unknown"]),
    ("unverified", &["voicemail", "unknown"]),
    ("called", &["bool"]),
];

/// One setting of the screening written (Settings ▸ This phone ▸ Call
/// screening), `key` as "screening.unknown": its value checked, then only that
/// key, in place (`config::set_value`); the others keep their first values
/// until you change them. Refused, said, for a key or a word it does not take.
pub fn set(path: &Path, key: &str, value: &SettingValue) -> Result<(), String> {
    let name = key.strip_prefix("screening.").unwrap_or(key);
    let Some((_, words)) = KEYS.iter().find(|(k, _)| *k == name) else { return Err(format!("{key}: no such setting of the call screening")) };
    let boolean = words.len() == 1 && words[0] == "bool";
    let checked = match value {
        SettingValue::Bool(on) if boolean => SettingValue::Bool(*on),
        SettingValue::Text(word) if !boolean && words.contains(&word.trim()) => SettingValue::Text(word.trim().to_string()),
        _ if boolean => return Err(format!("{key}: on or off")),
        _ => return Err(format!("{key}: {}", words.join(", "))),
    };
    // The first setting written fixes unknown and hidden numbers at their
    // first values: the Calls rows they come from move them no more.
    if path.exists() {
        let config = Config::load(path)?;
        let now = Screening::of(&config);
        for (unsaid, rule) in [("unknown", now.unknown), ("hidden", now.hidden)] {
            if unsaid != name && config.screening.as_ref().and_then(|s| s.word(unsaid)).and_then(Rule::read).is_none() {
                crate::config::set_value(path, &format!("screening.{unsaid}"), &SettingValue::Text(rule.id().to_string()))?;
            }
        }
    }
    crate::config::set_value(path, &format!("screening.{name}"), &checked)
}

// ---------------------------------------------------------------- the numbers known by their kind

const NUMBERS: &str = include_str!("../data/screening-numbers.toml");

/// The numbers the screening knows by their kind (`data/screening-numbers.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Numbers {
    #[serde(default)]
    pub group: Vec<Group>,
}

/// One group of prefixes, as the plan or ARCEP's data says them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Group {
    /// "automated", "never-shown", "unattributed".
    pub kind: String,
    #[serde(default)]
    pub territory: String,
    /// In national form ("0162").
    #[serde(default)]
    pub prefixes: Vec<String>,
    /// The rule in the source's own words, when quoted.
    #[serde(default)]
    pub about: String,
    pub source: String,
    /// The day it was read (YYYY-MM-DD).
    pub checked: String,
    /// It counts only so many days after `checked` (ARCEP's attributions move).
    #[serde(default)]
    pub valid_days: Option<i64>,
}

impl Group {
    /// Whether it counts on `today`: always, or within its days after it was read.
    pub fn counts(&self, today: jiff::civil::Date) -> bool {
        let Some(days) = self.valid_days else { return true };
        let Ok(read) = self.checked.parse::<jiff::civil::Date>() else { return false };
        read.checked_add(jiff::Span::new().days(days)).is_ok_and(|end| today < end) && today >= read
    }
}

impl Numbers {
    /// As Sioul ships it; none when the file does not read (the tests say it does).
    pub fn built_in() -> Numbers {
        toml::from_str(NUMBERS).unwrap_or_default()
    }

    fn of(&self, kinds: &[&str], today: jiff::civil::Date) -> Vec<String> {
        let mut out: Vec<String> = self.group.iter().filter(|g| kinds.contains(&g.kind.as_str()) && g.counts(today)).flat_map(|g| g.prefixes.iter().map(|p| p.trim().to_string())).filter(|p| p.len() >= 2 && p.starts_with('0') && p.bytes().all(|b| b.is_ascii_digit())).collect();
        out.sort();
        out.dedup();
        out
    }

    /// The automated callers' roots, in national form.
    pub fn automated(&self, today: jiff::civil::Date) -> Vec<String> {
        self.of(&["automated"], today)
    }

    /// What can never be a caller's number, in national form: what the plan
    /// forbids as a caller ID (089). The blocks attributed to nobody on a day
    /// are no part of it: a block attributed since may hold a new line
    /// (review of part 2, F1).
    pub fn never_shown(&self, today: jiff::civil::Date) -> Vec<String> {
        self.of(&["never-shown"], today)
    }

    /// The day ARCEP's list was read, when a number's key falls in a block it
    /// had attributed to no operator then and that list still counts on
    /// `today`: said on a declined call's line, deciding nothing.
    pub fn unattributed_since(&self, key: &str, today: jiff::civil::Date) -> Option<jiff::civil::Date> {
        let national = national(key)?;
        self.group.iter().filter(|g| g.kind == "unattributed" && g.counts(today)).find(|g| starts_with_any(&national, &g.prefixes)).and_then(|g| g.checked.parse().ok())
    }
}

/// The numbers as Sioul ships them, read once.
pub fn shipped() -> &'static Numbers {
    static SHIPPED: std::sync::OnceLock<Numbers> = std::sync::OnceLock::new();
    SHIPPED.get_or_init(Numbers::built_in)
}

/// A number's key (`phones::key`, "+33199001234") in the French plan's
/// national form ("0199001234"), its overseas departments' included (+262,
/// +590, +594, +596: "+262262123456" is "0262123456"); none for any other.
pub fn national(key: &str) -> Option<String> {
    ["+33", "+262", "+590", "+594", "+596"].iter().find_map(|code| key.strip_prefix(code).filter(|rest| rest.len() == 9 && rest.bytes().all(|b| b.is_ascii_digit())).map(|rest| format!("0{rest}")))
}

/// Whether a national number starts with one of these prefixes.
pub fn starts_with_any(national: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|p| !p.is_empty() && national.starts_with(p.as_str()))
}

// ---------------------------------------------------------------- the calls' table

/// What the calls' table says of the screening (version 2), as Java reads it
/// (Calls.Screening): the screening as it stands, and the prefixes it needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TablePart {
    pub on: bool,
    pub hours: Hours,
    pub unknown: Rule,
    pub hidden: Rule,
    pub automated: Spam,
    pub impossible: Spam,
    pub unverified: Spam,
    /// A number called in the last so many days counts as your contacts do; 0: none.
    pub called_days: i64,
    /// The automated callers' roots, in national form.
    #[serde(default)]
    pub automated_prefixes: Vec<String>,
    /// What can never be a caller's number, in national form: what the plan
    /// forbids as a caller ID (089).
    #[serde(default)]
    pub impossible_prefixes: Vec<String>,
}

impl TablePart {
    /// The screening and the numbers that count on `today`.
    pub fn of(screening: &Screening, numbers: &Numbers, today: jiff::civil::Date) -> TablePart {
        TablePart {
            on: screening.on,
            hours: screening.hours,
            unknown: screening.unknown,
            hidden: screening.hidden,
            automated: screening.automated,
            impossible: screening.impossible,
            unverified: screening.unverified,
            called_days: if screening.called { CALLED_DAYS } else { 0 },
            automated_prefixes: numbers.automated(today),
            impossible_prefixes: numbers.never_shown(today),
        }
    }

    /// The screening it says, without its prefixes.
    pub fn screening(&self) -> Screening {
        Screening { on: self.on, hours: self.hours, unknown: self.unknown, hidden: self.hidden, automated: self.automated, impossible: self.impossible, unverified: self.unverified, called: self.called_days > 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::Column as C;

    fn day(text: &str) -> jiff::civil::Date {
        text.parse().unwrap()
    }

    /// The shipped file reads; every group names its kind, its source with an
    /// address and the day it was read; prefixes in national form; ARCEP's
    /// roots for machines as the plan says them; 089 never shown; the
    /// never-attributed blocks counting 90 days only.
    #[test]
    fn screening_numbers_are_well_formed_and_sourced() {
        let numbers: Numbers = toml::from_str(NUMBERS).expect("data/screening-numbers.toml reads");
        assert!(numbers.group.len() >= 7, "{} groups", numbers.group.len());
        for g in &numbers.group {
            assert!(["automated", "never-shown", "unattributed"].contains(&g.kind.as_str()), "{}", g.kind);
            assert!(g.source.contains("ARCEP") && g.source.contains("https://"), "a source with its address: {}", g.source);
            assert!(g.checked.parse::<jiff::civil::Date>().is_ok(), "{}: the day read", g.checked);
            assert!(!g.territory.is_empty() && !g.prefixes.is_empty());
            for p in &g.prefixes {
                assert!(p.len() >= 3 && p.len() <= 6 && p.starts_with('0') && p.bytes().all(|b| b.is_ascii_digit()), "{p}");
            }
        }
        let today = day("2026-10-20");
        let automated = numbers.automated(today);
        for root in ["0162", "0163", "0270", "0271", "0377", "0378", "0424", "0425", "0568", "0569", "0948", "0949", "05987", "09475", "05988", "09476", "05989", "09477", "02688", "02689", "09478", "09479"] {
            assert!(automated.contains(&root.to_string()), "{root}");
        }
        assert_eq!(automated.len(), 22, "nothing else: the public-interest roots stay out");
        // What can never be a caller's: the plan's own rule, 089, and nothing of the snapshot (review of part 2, F1).
        assert_eq!(numbers.never_shown(today), vec!["089".to_string()]);
        let unattributed = numbers.group.iter().find(|g| g.kind == "unattributed").unwrap();
        // The public-interest multi-use numbers (plan § 2.3.10 a): a category of their own, as any number nobody
        // knows (CS12), never said attributed to nobody though most of their blocks are yet; nor machines.
        for root in ["01510", "02810", "03410", "04410", "05410", "09410", "059850", "059864", "094700", "094709", "02687", "050860"] {
            assert!(!unattributed.prefixes.iter().chain(&automated).any(|p| p.starts_with(root) || root.starts_with(p.as_str())), "{root}");
        }
        assert!(unattributed.prefixes.len() > 1_000, "the blocks attributed to nobody on the day read");
        // No root for machines among the numbers never shown.
        assert!(!automated.iter().any(|a| a.starts_with("089")));
        // The blocks attributed to nobody are said 90 days after the day read, then no more.
        assert_eq!(unattributed.valid_days, Some(90));
        let read = day(&unattributed.checked);
        assert!(unattributed.counts(read) && unattributed.counts(read.checked_add(jiff::Span::new().days(89)).unwrap()));
        assert!(!unattributed.counts(read.checked_add(jiff::Span::new().days(90)).unwrap()), "past its days");
        assert!(!unattributed.counts(read.yesterday().unwrap()), "a clock before the day read: not counted");
        assert_eq!(numbers.unattributed_since("+33353011234", read), Some(read), "a fiction block attributed to nobody that day");
        assert_eq!(numbers.unattributed_since("+33353011234", read.checked_add(jiff::Span::new().days(90)).unwrap()), None);
        assert_eq!(numbers.unattributed_since("+441632960000", read), None, "another country's");
        assert_eq!(Numbers::built_in(), numbers);
    }

    #[test]
    fn numbers_in_national_form() {
        assert_eq!(national("+33199001234").as_deref(), Some("0199001234"));
        assert_eq!(national("+262262123456").as_deref(), Some("0262123456"), "La Réunion's fixed numbers");
        assert_eq!(national("+590590123456").as_deref(), Some("0590123456"));
        assert_eq!(national("+3319900123"), None, "a digit short");
        assert_eq!(national("+441632960000"), None, "another country's");
        assert_eq!(national("0199001234"), None, "not a key");
        assert_eq!(national(""), None);
        let prefixes = vec!["0465".to_string(), "089".to_string()];
        assert!(starts_with_any("0465711234", &prefixes) && starts_with_any("0899123456", &prefixes) && !starts_with_any("0199001234", &prefixes));
        assert!(!starts_with_any("0199001234", &[String::new()]), "an empty prefix holds nothing");
    }

    /// The first values: your Calls rows of strangers and hidden numbers as
    /// they were, never screening more than they did; each key once written
    /// is yours, a word not known reads as unsaid; the decided defaults.
    #[test]
    fn first_values_from_the_calls_rows_and_each_key_once_written() {
        let usual = Screening::default();
        assert_eq!(usual, Screening { on: true, hours: Hours::Always, unknown: Rule::Voicemail, hidden: Rule::Work, automated: Spam::Voicemail, impossible: Spam::Voicemail, unverified: Spam::Voicemail, called: true });
        // A row of hidden numbers held at every time.
        let config: Config = toml::from_str("[attention]\n\"calls.hidden\" = [\"work:later\", \"admin:later\", \"leisure:later\", \"meals:later\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot:later\", \"dnd:later\"]\n").unwrap();
        assert_eq!(Screening::of(&config).hidden, Rule::Voicemail);
        // Strangers let ring at work and admin: ringing then; at leisure too: still only then, never at night.
        let mut attention = Attention::usual();
        attention.set(Row::People(Channel::Calls, Person::Stranger), C::Work, Level::Now).unwrap();
        attention.set(Row::People(Channel::Calls, Person::Stranger), C::Admin, Level::Now).unwrap();
        assert_eq!(first_rule(&attention, Person::Stranger), Rule::Work);
        attention.set(Row::People(Channel::Calls, Person::Stranger), C::Leisure, Level::Now).unwrap();
        assert_eq!(first_rule(&attention, Person::Stranger), Rule::Work, "never ringing where the row declined");
        // A layer is no part of it: time for you's cell lets strangers through as usual, voicemail all the same.
        assert_eq!(attention.cell(Row::People(Channel::Calls, Person::Stranger), C::Slot), Level::Now);
        assert_eq!(first_rule(&Attention::usual(), Person::Stranger), Rule::Voicemail);
        // Written keys are yours; a word not known reads as unsaid; a wrong type too.
        let config: Config = toml::from_str("[screening]\non = false\nhours = \"outside-work\"\nunknown = \"ring\"\nautomated = \"unknown\"\nimpossible = \"later-word\"\ncalled = \"yes\"\n").unwrap();
        let read = Screening::of(&config);
        assert_eq!((read.on, read.hours, read.unknown, read.hidden, read.automated, read.impossible, read.called), (false, Hours::OutsideWork, Rule::Ring, Rule::Work, Spam::Unknown, Spam::Voicemail, true));
    }

    /// A row of hidden numbers held at every time, as the review
    /// of part 2 tried it (19 October 2026).
    const HIDDEN_HELD: &str = "[attention]\n\"calls.hidden\" = [\"work:later\", \"admin:later\", \"leisure:later\", \"meals:later\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot:later\", \"dnd:later\"]\n";

    /// A configuration file of its own, in a folder of its own.
    fn scratch(name: &str, text: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("sioul-screening-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, text).unwrap();
        (dir, path)
    }

    /// F2 (review of part 2): no preset moves the call screening. Each leaves
    /// held hidden numbers to voicemail at all hours, their row as it
    /// was, and the screening as it stood.
    #[test]
    fn the_presets_never_move_the_screening() {
        for preset in crate::attention::Preset::ALL {
            let (dir, path) = scratch(preset.id(), HIDDEN_HELD);
            let before = Screening::of(&Config::load(&path).unwrap());
            crate::attention::apply_preset(&path, &Config::load(&path).unwrap(), preset).unwrap();
            let after = Screening::of(&Config::load(&path).unwrap());
            assert_eq!((after, after.hidden), (before, Rule::Voicemail), "{preset:?}");
            assert!(std::fs::read_to_string(&path).unwrap().contains("\"calls.hidden\" = [\"work:later\""), "{preset:?}: the row as it was");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// F2: the first setting of the screening written fixes unknown and hidden
    /// numbers at their first values: a Calls row changed afterwards (an
    /// older device's grid) moves the screening no more.
    #[test]
    fn the_first_key_written_fixes_unknown_and_hidden() {
        let (dir, path) = scratch("first", HIDDEN_HELD);
        set(&path, "screening.on", &SettingValue::Bool(true)).unwrap();
        let written = Config::load(&path).unwrap().screening.unwrap();
        assert_eq!((written.word("unknown"), written.word("hidden")), (Some("voicemail"), Some("voicemail")));
        let text = std::fs::read_to_string(&path).unwrap().replace("\"calls.hidden\" = [\"work:later\", \"admin:later\"", "\"calls.hidden\" = [\"work\", \"admin\"");
        std::fs::write(&path, text).unwrap();
        assert_eq!(Screening::of(&Config::load(&path).unwrap()).hidden, Rule::Voicemail);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F2: a row that rang in some hours out of work never makes its numbers
    /// ring at every hour: ringing in work and admin hours when it rang then,
    /// else voicemail; at every hour only when it rang at every time.
    #[test]
    fn a_row_that_rang_out_of_work_never_rings_at_all_hours() {
        let stranger = Row::People(Channel::Calls, Person::Stranger);
        let mut leisure = Attention::usual();
        leisure.set(stranger, C::Leisure, Level::Now).unwrap();
        assert_eq!(first_rule(&leisure, Person::Stranger), Rule::Voicemail);
        let mut both = leisure.clone();
        both.set(stranger, C::Work, Level::Now).unwrap();
        assert_eq!(first_rule(&both, Person::Stranger), Rule::Work);
        let mut every = Attention::usual();
        for column in C::TIMES {
            every.set(stranger, column, Level::Now).unwrap();
        }
        assert_eq!(first_rule(&every, Person::Stranger), Rule::Ring);
    }

    /// Whether a number nobody knows rings by the screening alone, at each
    /// hour: off, outside its hours, its rule.
    #[test]
    fn what_rings_by_the_screening_alone() {
        let usual = Screening::default();
        assert!(!usual.rings(false, true) && !usual.rings(false, false), "unknown numbers: voicemail at all hours");
        assert!(usual.rings(true, true) && !usual.rings(true, false), "hidden numbers: in work and admin hours");
        let off = Screening { on: false, ..usual };
        assert!(off.rings(false, false) && off.rings(true, false));
        let outside = Screening { hours: Hours::OutsideWork, hidden: Rule::Voicemail, ..usual };
        assert!(outside.rings(false, true) && outside.rings(true, true) && !outside.rings(false, false) && !outside.rings(true, false));
        let ring = Screening { unknown: Rule::Ring, ..usual };
        assert!(ring.rings(false, false));
    }

    /// Written a key at a time, checked: the others stay unsaid; a key or a
    /// word it does not take is refused, said; the rest of the file kept.
    #[test]
    fn settings_written_a_key_at_a_time() {
        let dir = std::env::temp_dir().join(format!("sioul-screening-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\n[dnd]\nfocus = true\n\n[screening]\nlater = \"a later version's\"\n").unwrap();
        set(&path, "screening.hidden", &SettingValue::Text("voicemail".into())).unwrap();
        set(&path, "screening.on", &SettingValue::Bool(true)).unwrap();
        assert!(set(&path, "screening.hidden", &SettingValue::Text("sometimes".into())).unwrap_err().contains("voicemail, work, ring"));
        assert!(set(&path, "screening.on", &SettingValue::Text("yes".into())).is_err());
        assert!(set(&path, "screening.color", &SettingValue::Text("red".into())).is_err());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# mine") && text.contains("focus = true") && text.contains("later = \"a later version's\""), "{text}");
        let config = Config::load(&path).unwrap();
        let read = Screening::of(&config);
        assert_eq!((read.on, read.hidden, read.unknown), (true, Rule::Voicemail, Rule::Voicemail));
        // The first key written fixed unknown numbers at their first value too; nothing else is written.
        let written = &config.screening.as_ref().unwrap().0;
        assert_eq!(written.get("unknown").and_then(toml::Value::as_str), Some("voicemail"));
        assert!(!written.contains_key("automated") && !written.contains_key("hours"), "only the keys changed, and the two fixed");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

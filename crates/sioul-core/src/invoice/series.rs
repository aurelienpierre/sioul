// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Invoice numbers: a pattern ("{YYYY}-{N:3}") and its reset (each year, or
//! never) make a sequence; each number given is one more than the largest
//! of the series journal's, the records' and every device's claim on
//! invoices (`sioul_sync::lease`), so that a number is never given twice,
//! even after a record was deleted by hand (docs/projects.md, "Invoices").
//!
//! The journal keeps each sequence's last number and its day, each device
//! in its own file, `invoices/series/<device>.toml` (no other device writes
//! it, so the sync never makes a conflicted copy, and an older Sioul, which
//! lists only the folder's top files, never takes it for an invoice); it is
//! written in the same step as the record, under the numbering's lock, and
//! read as the largest of every device's. A number of the journal's range
//! with no record is said, never given again.
//!
//! Since phase 3 the journal's entry of each number also keeps the SHA-256
//! of its record's bytes as they were written at issue (`records`): the
//! record is never written again (its life lives apart, `lives`), so a
//! record whose bytes differ from it was changed outside Sioul, which the
//! window says (`invoice::outside`). A record from before has no entry.

use super::record::Invoice;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// The journal's folder, in the invoices' folder: a file per device.
pub const FOLDER: &str = "series";
/// The journal of phase 1's first builds, one file beside the records: read too.
pub const OLD_JOURNAL: &str = "series.toml";
/// The numbering's lock, in the invoices' folder: one number at a time on this device.
pub const LOCK: &str = "numbering";
/// The usual pattern: the year, a dash, three digits at least.
pub const PATTERN: &str = "{YYYY}-{N:3}";
/// The format of the journals this Sioul writes.
pub const FORMAT: u32 = 1;

/// How often numbers start again from 1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Reset {
    #[default]
    Yearly,
    Never,
}

impl Reset {
    pub fn named(name: &str) -> Reset {
        if name == "never" { Reset::Never } else { Reset::Yearly }
    }
}

/// One part of a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Year4,
    Year2,
    Month,
    /// The counter, at least this many digits.
    N(usize),
}

/// A numbering pattern, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    parts: Vec<Part>,
    reset: Reset,
}

impl Pattern {
    /// `text` read: `{YYYY}`, `{YY}`, `{MM}`, `{N}` or `{N:k}` (k digits at
    /// least), the rest as written. Refused, in words, without exactly one
    /// counter, or restarting each year without the year in it.
    pub fn parse(text: &str, reset: Reset) -> Result<Pattern, String> {
        let mut parts = Vec::new();
        let mut rest = text;
        while !rest.is_empty() {
            match rest.find('{') {
                Some(0) => {
                    let end = rest.find('}').ok_or_else(|| format!("“{text}”: a brace is not closed"))?;
                    let token = &rest[1..end];
                    parts.push(match token {
                        "YYYY" => Part::Year4,
                        "YY" => Part::Year2,
                        "MM" => Part::Month,
                        "N" => Part::N(1),
                        _ => match token.strip_prefix("N:").and_then(|w| w.parse::<usize>().ok()) {
                            Some(width) if (1..=12).contains(&width) => Part::N(width),
                            _ => return Err(format!("“{text}”: {{{token}}} is not a part of a number")),
                        },
                    });
                    rest = &rest[end + 1..];
                }
                Some(at) => {
                    parts.push(Part::Text(rest[..at].to_string()));
                    rest = &rest[at..];
                }
                None => {
                    parts.push(Part::Text(rest.to_string()));
                    rest = "";
                }
            }
        }
        if parts.iter().filter(|p| matches!(p, Part::N(_))).count() != 1 {
            return Err(format!("“{text}”: a pattern has one counter, {{N}} or {{N:3}}"));
        }
        if reset == Reset::Yearly && !parts.iter().any(|p| matches!(p, Part::Year4 | Part::Year2)) {
            return Err(format!("“{text}”: numbers that start again each year have the year in them, {{YYYY}}"));
        }
        Ok(Pattern { parts, reset })
    }

    /// The pattern of Sioul before November 2026: an empty prefix, the year
    /// and three digits, again from 1 each January; a prefix of your own and
    /// three digits, never again from 1 (F-041, F-042…).
    pub fn of_prefix(prefix: &str) -> Pattern {
        let prefix = prefix.trim();
        if prefix.is_empty() {
            Pattern::parse(PATTERN, Reset::Yearly).expect("the usual pattern reads")
        } else {
            Pattern { parts: vec![Part::Text(prefix.to_string()), Part::N(3)], reset: Reset::Never }
        }
    }

    /// The sequence numbers of `day` belong to, as the journal names it: the
    /// pattern with the year written in when it restarts each year ("2026-{N:3}").
    pub fn sequence(&self, day: Date) -> String {
        self.parts
            .iter()
            .map(|p| match p {
                Part::Text(t) => t.replace('{', "{{").replace('}', "}}"),
                Part::Year4 if self.reset == Reset::Yearly => format!("{:04}", day.year()),
                Part::Year2 if self.reset == Reset::Yearly => format!("{:02}", day.year() % 100),
                Part::Year4 => "{YYYY}".into(),
                Part::Year2 => "{YY}".into(),
                Part::Month => "{MM}".into(),
                Part::N(1) => "{N}".into(),
                Part::N(w) => format!("{{N:{w}}}"),
            })
            .collect()
    }

    /// The number `n` of `day`'s sequence.
    pub fn number(&self, day: Date, n: u64) -> String {
        self.parts
            .iter()
            .map(|p| match p {
                Part::Text(t) => t.clone(),
                Part::Year4 => format!("{:04}", day.year()),
                Part::Year2 => format!("{:02}", day.year() % 100),
                Part::Month => format!("{:02}", day.month()),
                Part::N(w) => format!("{n:0w$}"),
            })
            .collect()
    }

    /// The counter of `number` when it belongs to `day`'s sequence.
    pub fn counter(&self, day: Date, number: &str) -> Option<u64> {
        let mut rest = number;
        let mut found = None;
        let year = |digits: usize, rest: &mut &str, wanted: Option<String>| -> Option<()> {
            let head = rest.get(..digits)?;
            if !head.bytes().all(|b| b.is_ascii_digit()) || wanted.is_some_and(|w| w != head) {
                return None;
            }
            *rest = &rest[digits..];
            Some(())
        };
        for (i, part) in self.parts.iter().enumerate() {
            match part {
                Part::Text(t) => rest = rest.strip_prefix(t.as_str())?,
                Part::Year4 => year(4, &mut rest, (self.reset == Reset::Yearly).then(|| format!("{:04}", day.year())))?,
                Part::Year2 => year(2, &mut rest, (self.reset == Reset::Yearly).then(|| format!("{:02}", day.year() % 100)))?,
                Part::Month => year(2, &mut rest, None)?,
                Part::N(_) => {
                    // The counter's digits run up to what the next part starts with.
                    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
                    let take = match self.parts.get(i + 1) {
                        Some(Part::Text(_)) | None => digits,
                        // A year or month right after the counter: the last digits are theirs.
                        Some(Part::Year4) => digits.checked_sub(4)?,
                        Some(Part::Year2 | Part::Month) => digits.checked_sub(2)?,
                        Some(Part::N(_)) => return None,
                    };
                    if take == 0 {
                        return None;
                    }
                    found = rest[..take].parse::<u64>().ok();
                    rest = &rest[take..];
                }
            }
        }
        if rest.is_empty() { found } else { None }
    }
}

/// One sequence's last number given.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Last {
    pub last: u64,
    pub number: String,
    pub day: String,
}

/// The series journal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    #[serde(default = "first_format")]
    pub format: u32,
    #[serde(default)]
    pub series: BTreeMap<String, Last>,
    /// Each number this device issued since phase 3: the SHA-256 of its
    /// record's bytes as written at issue. An older journal has none; a
    /// Sioul that does not know the table reads past it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub records: BTreeMap<String, String>,
}

fn first_format() -> u32 {
    1
}

impl Journal {
    /// Every device's journal of the invoices' folder `dir`, each sequence
    /// at the largest last number any says; empty when there is none yet
    /// (the records and the claims still count). A file that does not read
    /// counts for nothing.
    pub fn load(dir: &Path) -> Journal {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir.join(FOLDER)).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
        paths.sort();
        paths.push(dir.join(OLD_JOURNAL));
        let mut out = Journal::default();
        // A newer Sioul's journal read where it can be: its last numbers count all the same.
        for journal in paths.iter().filter_map(|p| std::fs::read_to_string(p).ok()).filter_map(|t| toml::from_str::<Journal>(&t).ok()) {
            for (sequence, last) in journal.series {
                if out.series.get(&sequence).is_none_or(|kept| kept.last < last.last) {
                    out.series.insert(sequence, last);
                }
            }
            // A number's hash as the device that issued it wrote it (one device issues a number).
            for (number, sha) in journal.records {
                out.records.entry(number).or_insert(sha);
            }
        }
        out
    }

    /// This device's own journal file.
    pub fn own_path(dir: &Path, device: &str) -> std::path::PathBuf {
        let name: String = device.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
        let name = if name.trim_matches('-').is_empty() { "here".to_string() } else { name };
        dir.join(FOLDER).join(format!("{name}.toml"))
    }

    /// This device's own journal: what it gave.
    pub fn own(dir: &Path, device: &str) -> Option<Journal> {
        std::fs::read_to_string(Journal::own_path(dir, device)).ok().and_then(|t| toml::from_str(&t).ok())
    }

    /// Written as this device's own journal, whole, through a file beside
    /// it; never over one a newer Sioul wrote.
    pub fn save(&self, dir: &Path, device: &str) -> Result<(), String> {
        let path = Journal::own_path(dir, device);
        if std::fs::read_to_string(&path).is_ok_and(|t| super::lives::format_of(&t) > FORMAT) {
            return Err(format!("{}: written by a newer Sioul; update Sioul on this device to issue invoices here.", path.display()));
        }
        let text = toml::to_string(&Journal { format: FORMAT, ..self.clone() }).map_err(|e| e.to_string())?;
        super::write_whole(&path, &text)
    }

    /// The journals of `dir` a newer Sioul wrote: read where they can be, never written here.
    pub fn newer(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir.join(FOLDER)).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
        paths.push(dir.join(OLD_JOURNAL));
        paths.retain(|p| std::fs::read_to_string(p).is_ok_and(|t| super::lives::format_of(&t) > FORMAT));
        paths.sort();
        paths
    }

    /// Each sequence's last number, as a device's claim carries them.
    pub fn lasts(&self) -> BTreeMap<String, u64> {
        self.series.iter().map(|(k, v)| (k.clone(), v.last)).collect()
    }
}

/// The next number of `pattern` on `day`, and its counter: one more than
/// the largest of the journal, of `records` (unreadable ones too) and of
/// `claimed` (every device's claim, `sioul_sync::lease::Claim::series`).
pub fn next(pattern: &Pattern, day: Date, journal: &Journal, records: &[Invoice], claimed: &BTreeMap<String, u64>) -> (String, u64) {
    let sequence = pattern.sequence(day);
    let in_records = records.iter().filter_map(|r| pattern.counter(day, &r.number)).max().unwrap_or(0);
    let in_journal = journal.series.get(&sequence).map_or(0, |l| l.last);
    let in_claims = claimed.get(&sequence).copied().unwrap_or(0);
    let n = in_records.max(in_journal).max(in_claims).saturating_add(1);
    (pattern.number(day, n), n)
}

/// A number of a sequence the journal gave, with no record here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub sequence: String,
    pub counter: u64,
    /// The number itself, when the sequence says it whole (its year written
    /// in, no month): else only its counter is known.
    pub number: Option<String>,
}

/// The numbers of each sequence of the journal, up to its last, with no
/// record in `records`: each said, never given again.
pub fn missing(journal: &Journal, records: &[Invoice]) -> Vec<Missing> {
    let mut out = Vec::new();
    for (sequence, last) in &journal.series {
        let Ok(day) = last.day.parse::<Date>() else { continue };
        // The sequence as a pattern of its own: its year written in when it restarts each year.
        let Ok(pattern) = Pattern::parse(sequence, Reset::Never) else { continue };
        let whole = !pattern.parts.iter().any(|p| matches!(p, Part::Year4 | Part::Year2 | Part::Month));
        let have: std::collections::BTreeSet<u64> = records.iter().filter_map(|r| pattern.counter(day, &r.number)).collect();
        out.extend((1..=last.last).filter(|n| !have.contains(n)).map(|n| Missing { sequence: sequence.clone(), counter: n, number: whole.then(|| pattern.number(day, n)) }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn numbered(numbers: &[&str]) -> Vec<Invoice> {
        numbers.iter().map(|n| Invoice { number: n.to_string(), ..Invoice::default() }).collect()
    }

    #[test]
    fn numbers_follow_their_pattern() {
        let usual = Pattern::parse(PATTERN, Reset::Yearly).unwrap();
        assert_eq!(usual, Pattern::of_prefix(""));
        assert_eq!(usual.sequence(day("2026-11-16")), "2026-{N:3}");
        assert_eq!(usual.number(day("2026-11-16"), 7), "2026-007");
        assert_eq!(usual.number(day("2026-11-16"), 1000), "2026-1000");
        assert_eq!(usual.counter(day("2026-11-16"), "2026-042"), Some(42));
        assert_eq!(usual.counter(day("2026-11-16"), "2025-042"), None, "last year's sequence");
        assert_eq!(usual.counter(day("2026-11-16"), "2026-042b"), None);
        let none = Journal::default();
        let claims = BTreeMap::new();
        assert_eq!(next(&usual, day("2026-11-16"), &none, &numbered(&["2026-001", "2025-007"]), &claims).0, "2026-002");
        assert_eq!(next(&usual, day("2027-01-02"), &none, &numbered(&["2026-001"]), &claims).0, "2027-001", "again from 1 in January");
        // A prefix of your own never starts again.
        let own = Pattern::of_prefix("F-");
        assert_eq!(next(&own, day("2027-01-02"), &none, &numbered(&["F-041"]), &claims).0, "F-042");
        // The year kept, counted on whatever the year.
        let on = Pattern::parse("F{YY}/{N:4}", Reset::Never).unwrap();
        assert_eq!(on.sequence(day("2026-01-01")), "F{YY}/{N:4}");
        assert_eq!(next(&on, day("2027-03-01"), &none, &numbered(&["F26/0041"]), &claims).0, "F27/0042");
        // The counter before the year.
        let before = Pattern::parse("{N:3}-{YYYY}", Reset::Yearly).unwrap();
        assert_eq!(before.counter(day("2026-05-05"), "012-2026"), Some(12));
        assert!(Pattern::parse("{YYYY}-001", Reset::Yearly).is_err(), "no counter");
        assert!(Pattern::parse("F-{N}", Reset::Yearly).is_err(), "restarting each year without the year");
        assert!(Pattern::parse("{YYYY}-{Q}-{N}", Reset::Yearly).is_err());
    }

    /// A number is never given twice: the journal remembers past a record
    /// deleted by hand, and another device's claim past a record not come yet.
    #[test]
    fn the_journal_and_the_claims_hold_the_numbers() {
        let usual = Pattern::of_prefix("");
        let today = day("2026-11-16");
        let mut journal = Journal::default();
        journal.series.insert("2026-{N:3}".into(), Last { last: 5, number: "2026-005".into(), day: "2026-11-10".into() });
        let records = numbered(&["2026-001", "2026-002", "2026-003", "2026-005"]);
        assert_eq!(next(&usual, today, &journal, &records, &BTreeMap::new()).0, "2026-006", "2026-005's record deleted: still given");
        let claimed = BTreeMap::from([("2026-{N:3}".to_string(), 8u64)]);
        assert_eq!(next(&usual, today, &journal, &records, &claimed).0, "2026-009");
        assert_eq!(missing(&journal, &records).into_iter().map(|m| m.number).collect::<Vec<_>>(), [Some("2026-004".to_string())]);
        // A month in the pattern: the missing counter is said, its month not guessed.
        let mut monthly = Journal::default();
        monthly.series.insert("F2026{MM}-{N:3}".into(), Last { last: 2, number: "F202611-002".into(), day: "2026-11-10".into() });
        let gone = missing(&monthly, &numbered(&["F202603-001"]));
        assert_eq!((gone.len(), gone[0].counter, gone[0].number.clone()), (1, 2, None));
        // Each device its own file, read as the largest of all.
        let dir = std::env::temp_dir().join(format!("sioul-series-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        journal.save(&dir, "desk").unwrap();
        let mut laptop = Journal::default();
        laptop.series.insert("2026-{N:3}".into(), Last { last: 3, number: "2026-003".into(), day: "2026-11-01".into() });
        laptop.save(&dir, "laptop").unwrap();
        assert_eq!(Journal::load(&dir).lasts(), BTreeMap::from([("2026-{N:3}".to_string(), 5u64)]));
        assert_eq!(Journal::own(&dir, "laptop").map(|j| j.lasts()), Some(BTreeMap::from([("2026-{N:3}".to_string(), 3u64)])));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

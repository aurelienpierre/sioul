// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Two events at once (docs/client.md, "Calendars"): found among
//! the coming days' events, the time kept before and after each counted
//! (`demands::Margins`): going from one to the other takes time too. Said
//! on the Porch for today and above the agenda, until set aside for good;
//! an event moved is a new question.

use crate::agenda::Occurrence;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Two events that overlap, or follow each other closer than a time you
/// set between them, the earlier first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Overlap {
    /// `<uid>@<start>|<uid>@<start>`: what setting it aside keeps.
    pub key: String,
    pub first: Occurrence,
    pub second: Occurrence,
    /// Minutes from the first's end to the second's start; negative when
    /// they overlap.
    pub gap: i64,
    /// Minutes you keep between them (0 when they overlap: none is needed to say it).
    pub needed: i64,
    /// Where that time is set: "after" (the first event's own After),
    /// "before" (the second's own Before), "setting" (Settings ▸ Agenda,
    /// Time kept between two events); "" when they overlap.
    pub source: String,
}

impl Overlap {
    /// Their times overlap: the only case called an overlap.
    pub fn at_once(&self) -> bool {
        self.gap < 0
    }
}

/// The pairs of `events` that clash: timed, not cancelled, lasting some time
/// (a reminder at 10:00 holds no time), each pair once. Two events overlap
/// only when their times do (22 October 2026). Closer than a time you set
/// yourself, they are said too close, never overlapping, that time named
/// with where you set it: the first's own After, the second's own Before, or
/// `between` (Settings ▸ Agenda, Time kept between two events; 0 unless you
/// set it), the longest of the three, never added. No other time is ever
/// put between two events: none from places, the plan or a default.
pub fn overlaps(events: &[Occurrence], between: u32) -> Vec<Overlap> {
    let mut timed: Vec<&Occurrence> = events.iter().filter(|e| !e.cancelled && !e.all_day && e.end > e.start).collect();
    timed.sort_by(|a, b| (a.start, &a.uid).cmp(&(b.start, &b.uid)));
    let mut out = Vec::new();
    for (i, first) in timed.iter().enumerate() {
        for second in &timed[i + 1..] {
            // The same event seen twice (in two calendars) is no double booking.
            if second.uid == first.uid && second.start == first.start {
                continue;
            }
            // Sorted by start: the second starts at or after the first.
            let key = format!("{}@{}|{}@{}", first.uid, first.start, second.uid, second.start);
            let gap = (second.start - first.end).div_euclid(60);
            if second.start < first.end {
                out.push(Overlap { key, first: (*first).clone(), second: (*second).clone(), gap, needed: 0, source: String::new() });
                continue;
            }
            let kept = [(first.margins.after, "after"), (second.margins.before, "before"), (between, "setting")];
            let Some((needed, source)) = kept.into_iter().filter(|(m, _)| *m > 0).max_by_key(|(m, _)| *m) else { continue };
            if second.start - first.end < i64::from(needed) * 60 {
                out.push(Overlap { key, first: (*first).clone(), second: (*second).clone(), gap, needed: i64::from(needed), source: source.to_string() });
            }
        }
    }
    out
}

/// The overlaps set aside for good, on this device
/// (`$XDG_STATE_HOME/sioul/overlaps-set-aside.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAside {
    #[serde(default)]
    pub keys: BTreeSet<String>,
}

impl SetAside {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("overlaps-set-aside.toml")
    }

    pub fn load(path: &Path) -> SetAside {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        std::fs::write(path, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demands::Margins;

    fn event(uid: &str, start: i64, end: i64, margins: Margins) -> Occurrence {
        Occurrence { uid: uid.into(), key: format!("{uid}.ics"), summary: uid.into(), start, end, margins, ..Occurrence::default() }
    }

    #[test]
    fn two_events_at_once() {
        let none = Margins::default();
        // 9–10 and 9:30–10:30 overlap; 10:30–11 only touches the second.
        let events = [event("a", 32_400, 36_000, none), event("b", 34_200, 37_800, none), event("c", 37_800, 39_600, none)];
        let found = overlaps(&events, 0);
        assert_eq!(found.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["a@32400|b@34200"]);
        // With half an hour to get to the third, it overlaps the second too.
        let events = [event("b", 34_200, 37_800, none), event("c", 37_800, 39_600, Margins { before: 30, after: 0 })];
        assert_eq!(overlaps(&events, 0).len(), 1);
        // Cancelled, all day, a moment (a reminder), the same one twice: none.
        let cancelled = Occurrence { cancelled: true, ..event("d", 32_400, 36_000, none) };
        let moment = event("e", 34_200, 34_200, none);
        let twice = [event("a", 32_400, 36_000, none), event("a", 32_400, 36_000, none)];
        assert!(overlaps(&[event("a", 32_400, 36_000, none), cancelled, moment], 0).is_empty());
        assert!(overlaps(&twice, 0).is_empty());
        // Set aside for good: kept in a file.
        let dir = std::env::temp_dir().join(format!("sioul-overlaps-{}", std::process::id()));
        let path = dir.join("set-aside.toml");
        let mut aside = SetAside::load(&path);
        aside.keys.insert(found[0].key.clone());
        aside.save(&path).unwrap();
        assert!(SetAside::load(&path).keys.contains("a@32400|b@34200"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Two events, 13:00–13:40 and
    /// 14:00–15:00 the same day, read from their files in every form a
    /// calendar writes (floating times, UTC, Europe/Paris with and without
    /// its VTIMEZONE, a DURATION), on a summer day, the day the clocks go
    /// back and a winter day: twenty minutes apart, they never overlap. Kept
    /// after the first and before the second, a quarter of an hour each is
    /// one way, not two, and fits; half an hour does not, and is said.
    #[test]
    fn twenty_minutes_apart_never_overlap_whatever_their_form() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
        use crate::vdir::{self, Kind};
        let paris = jiff::tz::TimeZone::get("Europe/Paris").unwrap();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Overlap forms", None, &["VEVENT"]).unwrap();
        let vtimezone = "BEGIN:VTIMEZONE\r\nTZID:Europe/Paris\r\nBEGIN:DAYLIGHT\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0200\r\nDTSTART:19700329T020000\r\nRRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\nEND:DAYLIGHT\r\nBEGIN:STANDARD\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nDTSTART:19701025T030000\r\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n";
        for (date, offset) in [("20261023", 2u32), ("20261025", 1), ("20261221", 1)] {
            let forms = |h: u32, m: u32, eh: u32, em: u32| -> Vec<(&str, String)> {
                let local = |h: u32, m: u32| format!("{date}T{h:02}{m:02}00");
                let utc = |h: u32, m: u32| format!("{date}T{:02}{m:02}00Z", h - offset);
                let minutes = (eh * 60 + em) - (h * 60 + m);
                vec![
                    ("floating", format!("DTSTART:{}\r\nDTEND:{}\r\n", local(h, m), local(eh, em))),
                    ("utc", format!("DTSTART:{}\r\nDTEND:{}\r\n", utc(h, m), utc(eh, em))),
                    ("paris", format!("DTSTART;TZID=Europe/Paris:{}\r\nDTEND;TZID=Europe/Paris:{}\r\n", local(h, m), local(eh, em))),
                    ("paris-vtimezone", format!("DTSTART;TZID=Europe/Paris:{}\r\nDTEND;TZID=Europe/Paris:{}\r\n", local(h, m), local(eh, em))),
                    ("paris-duration", format!("DTSTART;TZID=Europe/Paris:{}\r\nDURATION:PT{minutes}M\r\n", local(h, m))),
                    ("floating-duration", format!("DTSTART:{}\r\nDURATION:PT{minutes}M\r\n", local(h, m))),
                ]
            };
            for (first_form, first) in forms(13, 0, 13, 40) {
                for (second_form, second) in forms(14, 0, 15, 0) {
                    let read = |uid: &str, form: &str, body: &str, kept: &str| {
                        let zone_lines = if form == "paris-vtimezone" { vtimezone } else { "" };
                        let path = calendar.dir.join(format!("{uid}.ics"));
                        std::fs::write(&path, format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{zone_lines}BEGIN:VEVENT\r\nUID:{uid}\r\n{body}{kept}SUMMARY:{uid}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")).unwrap();
                        crate::agenda::file_occurrences(&path, &calendar, 1_770_000_000, 1_800_000_000, &paris)
                    };
                    let case = format!("{date} {first_form} × {second_form}");
                    let both = |a: &str, b: &str| [read("a", first_form, &first, a), read("b", second_form, &second, b)].concat();
                    let plain = both("", "");
                    assert_eq!(plain.len(), 2, "{case}");
                    assert_eq!((plain[0].end - plain[0].start, plain[1].end - plain[1].start, plain[1].start - plain[0].end), (2400, 3600, 1200), "{case}");
                    assert!(overlaps(&plain, 0).is_empty(), "{case}: twenty minutes apart, nothing set");
                    assert!(overlaps(&both("X-SIOUL-AFTER:15\r\n", "X-SIOUL-BEFORE:15\r\n"), 0).is_empty(), "{case}: a quarter of an hour each, never added, fits");
                    let said = |o: &[Overlap]| o.iter().map(|o| (o.at_once(), o.gap, o.needed, o.source.clone())).collect::<Vec<_>>();
                    assert_eq!(said(&overlaps(&both("X-SIOUL-AFTER:30\r\n", "X-SIOUL-BEFORE:10\r\n"), 0)), [(false, 20, 30, "after".to_string())], "{case}: too close, the first's After named");
                    assert_eq!(said(&overlaps(&both("", "X-SIOUL-BEFORE:45\r\n"), 0)), [(false, 20, 45, "before".to_string())], "{case}: the second's Before named");
                    assert_eq!(said(&overlaps(&plain, 60)), [(false, 20, 60, "setting".to_string())], "{case}: the time set in Settings ▸ Agenda named");
                    assert!(overlaps(&plain, 20).is_empty(), "{case}: twenty minutes set, twenty minutes there");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
}

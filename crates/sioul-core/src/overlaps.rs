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

/// Two events that overlap, or follow each other too closely to go from one
/// to the other, the earlier first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Overlap {
    /// `<uid>@<start>|<uid>@<start>`: what setting it aside keeps.
    pub key: String,
    pub first: Occurrence,
    pub second: Occurrence,
    /// Minutes from the first's end to the second's start; negative when
    /// they overlap themselves.
    pub gap: i64,
    /// Minutes the way from one to the other takes: the longer of the
    /// first's time after it and the second's time before it (0 when neither
    /// keeps any).
    pub needed: i64,
}

/// The pairs of `events` that clash: timed, not cancelled, lasting some time
/// (a reminder at 10:00 holds no time), each pair once. Two events clash
/// when they overlap, or when the time between them is shorter than the way
/// from one to the other: the longer of the first's time after it (coming
/// back) and the second's time before it (getting there), never both added
/// (22 October 2026: 13:00–13:40 and 14:00–15:00, twenty minutes apart, a
/// quarter of an hour kept after the first and before the second, were said
/// to overlap: going home and setting out again were counted between them).
pub fn overlaps(events: &[Occurrence]) -> Vec<Overlap> {
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
            let gap = (second.start - first.end).div_euclid(60);
            let needed = i64::from(first.margins.after.max(second.margins.before));
            let clash = second.start < first.end || (second.start - first.end) < needed * 60;
            if clash {
                out.push(Overlap { key: format!("{}@{}|{}@{}", first.uid, first.start, second.uid, second.start), first: (*first).clone(), second: (*second).clone(), gap, needed });
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
        let found = overlaps(&events);
        assert_eq!(found.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["a@32400|b@34200"]);
        // With half an hour to get to the third, it overlaps the second too.
        let events = [event("b", 34_200, 37_800, none), event("c", 37_800, 39_600, Margins { before: 30, after: 0 })];
        assert_eq!(overlaps(&events).len(), 1);
        // Cancelled, all day, a moment (a reminder), the same one twice: none.
        let cancelled = Occurrence { cancelled: true, ..event("d", 32_400, 36_000, none) };
        let moment = event("e", 34_200, 34_200, none);
        let twice = [event("a", 32_400, 36_000, none), event("a", 32_400, 36_000, none)];
        assert!(overlaps(&[event("a", 32_400, 36_000, none), cancelled, moment]).is_empty());
        assert!(overlaps(&twice).is_empty());
        // Set aside for good: kept in a file.
        let dir = std::env::temp_dir().join(format!("sioul-overlaps-{}", std::process::id()));
        let path = dir.join("set-aside.toml");
        let mut aside = SetAside::load(&path);
        aside.keys.insert(found[0].key.clone());
        aside.save(&path).unwrap();
        assert!(SetAside::load(&path).keys.contains("a@32400|b@34200"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The owner's two events of 22 October 2026, 13:00–13:40 and
    /// 14:00–15:00 the same day, read from their files in every form a
    /// calendar writes (floating times, UTC, Europe/Paris with and without
    /// its VTIMEZONE, a DURATION), on a summer day, the day the clocks go
    /// back and a winter day: twenty minutes apart, they never overlap. Kept
    /// after the first and before the second, a quarter of an hour each is
    /// one way, not two, and fits; half an hour does not, and is said.
    #[test]
    fn twenty_minutes_apart_never_overlap_whatever_their_form() {
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
                    assert!(overlaps(&plain).is_empty(), "{case}: twenty minutes apart");
                    assert!(overlaps(&both("X-SIOUL-AFTER:15\r\n", "X-SIOUL-BEFORE:15\r\n")).is_empty(), "{case}: one way, a quarter of an hour, fits");
                    let tight = overlaps(&both("X-SIOUL-AFTER:30\r\n", "X-SIOUL-BEFORE:10\r\n"));
                    assert_eq!(tight.iter().map(|o| (o.gap, o.needed)).collect::<Vec<_>>(), [(20, 30)], "{case}: half an hour to go, twenty minutes there");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
}

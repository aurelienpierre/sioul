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
            // A leg and its own appointment, or two legs of one, are one outing (part E): the
            // appointment's time kept before and after comes from its legs, never a clash with them.
            if one_outing(first, second) {
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

/// Whether two events are one outing: a leg and its appointment, or two
/// legs of one appointment (`anchors`).
fn one_outing(a: &Occurrence, b: &Occurrence) -> bool {
    fn of(o: &Occurrence) -> Option<&str> {
        o.anchor.as_ref().map(|x| x.appointment.as_str()).filter(|u| !u.is_empty())
    }
    of(a) == Some(b.uid.as_str()) || of(b) == Some(a.uid.as_str()) || (of(a).is_some() && of(a) == of(b))
}

/// An occurrence as a warning names it: `<uid>@<start>`, as in `Overlap::key`.
pub fn occurrence_id(event: &Occurrence) -> String {
    format!("{}@{}", event.uid, event.start)
}

/// An event of 24 hours or more: its overlaps are said once, as one warning
/// ("During …: 34 other events", `warnings`). A whole-day event never
/// overlaps (`overlaps` counts timed ones alone).
pub fn is_long(event: &Occurrence) -> bool {
    !event.all_day && event.end - event.start >= 86_400
}

/// A warning as it is said (the ongoing round's fix W1, 1 November 2026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// Two events, as before.
    Pair(Overlap),
    /// An event of 24 hours or more and every overlap it makes, in their order:
    /// one warning, never one per event it covers.
    During { event: Occurrence, overlaps: Vec<Overlap> },
}

/// The overlaps as they are said: an event of 24 hours or more once, with all
/// its overlaps (the earlier one when both are), the others pair by pair; each
/// warning where its first overlap was. What is found stays as `overlaps`
/// finds it: only the way it is said changes.
pub fn warnings(overlaps: Vec<Overlap>) -> Vec<Warning> {
    let mut out: Vec<Warning> = Vec::new();
    for overlap in overlaps {
        let long = if is_long(&overlap.first) { Some(overlap.first.clone()) } else { Some(overlap.second.clone()).filter(is_long) };
        let Some(long) = long else {
            out.push(Warning::Pair(overlap));
            continue;
        };
        let id = occurrence_id(&long);
        match out.iter_mut().find_map(|w| match w {
            Warning::During { event, overlaps } if occurrence_id(event) == id => Some(overlaps),
            _ => None,
        }) {
            Some(overlaps) => overlaps.push(overlap),
            None => out.push(Warning::During { event: long, overlaps: vec![overlap] }),
        }
    }
    out
}

/// The overlaps set aside for good, on this device
/// (`$XDG_STATE_HOME/sioul/overlaps-set-aside.toml`; the sharing does not
/// carry it): `keys`, each pair as `Overlap::key` names it, which every Sioul
/// reads; and `events` (1 November 2026), an event of 24 hours or more set
/// aside whole by its warning, as `occurrence_id` names it: its overlaps to
/// come too. A Sioul before reads `keys` alone and skips `events`, a name it
/// does not know: the pairs that warning held are written in `keys` too
/// (`set`), so that it says none of them again either.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAside {
    #[serde(default)]
    pub keys: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub events: BTreeSet<String>,
}

impl SetAside {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("overlaps-set-aside.toml")
    }

    pub fn load(path: &Path) -> SetAside {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Whether an overlap is set aside: its pair, or one of its events whole.
    pub fn holds(&self, overlap: &Overlap) -> bool {
        self.keys.contains(&overlap.key) || self.events.contains(&occurrence_id(&overlap.first)) || self.events.contains(&occurrence_id(&overlap.second))
    }

    /// What a warning's "Don't mention it again" gives, one per line: a pair's
    /// key; or `during:<uid>@<start>` (an event set aside whole) followed by the
    /// keys of the pairs its warning held, which a Sioul before reads.
    pub fn set(&mut self, given: &str) {
        for line in given.lines().map(str::trim).filter(|l| !l.is_empty()) {
            match line.strip_prefix("during:") {
                Some(event) => self.events.insert(event.to_string()),
                None => self.keys.insert(line.to_string()),
            };
        }
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

    /// A leg and its appointment are one outing (part E): the bus 15 minutes
    /// before the doctor, whose 40 minutes kept before come from the bus, is no
    /// clash; another event in between still is said too close.
    #[test]
    fn a_leg_and_its_appointment_are_one_outing() {
        let anchor = crate::anchors::Anchor { appointment: "doctor".into(), side: crate::anchors::Side::Before, gap: 900, timetable: true, was: Some(9 * 3600 + 1800), unconfirmed: None, cancelled_with: false };
        let doctor = event("doctor", 9 * 3600 + 1800, 10 * 3600, Margins { before: 40, after: 0 });
        let bus = Occurrence { anchor: Some(anchor.clone()), ..event("bus", 8 * 3600 + 3000, 9 * 3600 + 900, Margins::default()) };
        let walk = Occurrence { anchor: Some(anchor), ..event("walk", 9 * 3600 + 900, 9 * 3600 + 1500, Margins::default()) };
        assert!(overlaps(&[doctor.clone(), bus.clone(), walk.clone()], 0).is_empty(), "one outing");
        let call = event("call", 9 * 3600 + 1200, 9 * 3600 + 1500, Margins::default());
        assert_eq!(overlaps(&[doctor, bus, call], 0).iter().map(|o| (o.first.uid.as_str(), o.second.uid.as_str())).collect::<Vec<_>>(), [("call", "doctor")]);
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

    /// An event of 24 hours or more is one warning with all its overlaps (fix
    /// W1): a residency over the days of a dentist, a call and a concert, in
    /// the order found; two other events still a pair; both long, the earlier
    /// holds it. What `overlaps` finds is the same.
    #[test]
    fn a_long_event_is_one_warning() {
        let none = Margins::default();
        let day = 86_400;
        let residency = event("residency", 10 * 3600, 28 * day, none);
        let events = [
            residency.clone(),
            event("dentist", day + 11 * 3600, day + 12 * 3600, none),
            event("call", 2 * day + 9 * 3600, 2 * day + 10 * 3600, none),
            event("lunch", 2 * day + 9 * 3600 + 1800, 2 * day + 11 * 3600, none),
            event("trip", 3 * day, 6 * day, none),
        ];
        let found = overlaps(&events, 0);
        assert_eq!(found.len(), 5, "residency with dentist, call, lunch and trip; call with lunch");
        let said = warnings(found.clone());
        let shape: Vec<(String, Vec<String>)> = said
            .iter()
            .map(|w| match w {
                Warning::Pair(o) => ("pair".to_string(), vec![o.key.clone()]),
                Warning::During { event, overlaps } => (occurrence_id(event), overlaps.iter().map(|o| o.key.clone()).collect()),
            })
            .collect();
        let at = |uid: &str| events.iter().find(|e| e.uid == uid).map(occurrence_id).unwrap();
        let pair = |a: &str, b: &str| format!("{}|{}", at(a), at(b));
        assert_eq!(shape, [
            (at("residency"), vec![pair("residency", "dentist"), pair("residency", "call"), pair("residency", "lunch"), pair("residency", "trip")]),
            ("pair".to_string(), vec![pair("call", "lunch")]),
        ]);
        assert_eq!(said.iter().map(|w| match w { Warning::Pair(_) => 1, Warning::During { overlaps, .. } => overlaps.len() }).sum::<usize>(), found.len(), "every overlap said, none twice");
        assert!(is_long(&residency) && !is_long(&events[1]) && !is_long(&Occurrence { all_day: true, ..residency.clone() }));
    }

    /// A long event set aside whole (fix W1): its overlaps to come too
    /// (`events`), and its pairs of now in `keys`, which a Sioul before reads,
    /// skipping the name it does not know.
    #[test]
    fn an_event_set_aside_whole_stays_read_by_an_older_sioul() {
        let none = Margins::default();
        let residency = event("residency", 36_000, 30 * 86_400, none);
        let dentist = event("dentist", 86_400 + 39_600, 86_400 + 43_200, none);
        let later = event("later", 9 * 86_400, 9 * 86_400 + 3_600, none);
        let found = overlaps(&[residency.clone(), dentist], 0);
        let dir = std::env::temp_dir().join(format!("sioul-overlaps-whole-{}", std::process::id()));
        let path = dir.join("set-aside.toml");
        let mut aside = SetAside::load(&path);
        aside.set(&format!("during:{}\n{}\n", occurrence_id(&residency), found[0].key));
        aside.save(&path).unwrap();
        let read = SetAside::load(&path);
        assert!(read.holds(&found[0]), "its pair");
        let to_come = overlaps(&[residency.clone(), later], 0);
        assert!(read.holds(&to_come[0]), "an overlap made later, held by the event set aside");
        // As a Sioul before 1 November 2026 reads the file: `keys` alone.
        #[derive(Deserialize)]
        struct Before {
            #[serde(default)]
            keys: BTreeSet<String>,
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let before: Before = toml::from_str(&text).unwrap();
        assert!(before.keys.contains(&found[0].key), "the pair it held, for a Sioul before: {text}");
        assert!(!SetAside::default().holds(&found[0]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

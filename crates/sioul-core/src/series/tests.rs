// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Tests of the series that end and pause: invented days, stand-ins only.

use super::*;
use crate::agenda::{self, EventEdit};
use crate::subscribed::HolidayDays;

fn paris() -> TimeZone {
    TimeZone::get("Europe/Paris").unwrap()
}

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

fn at(text: &str) -> i64 {
    text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(paris()).unwrap().timestamp().as_second()
}

/// A weekly class on Wednesdays at 17:00, from 2 October 2030, pausing as `pauses` says.
fn class(pauses: &[&str]) -> String {
    let edit = EventEdit {
        title: "Swimming".into(),
        start: "2030-10-02T17:00".into(),
        end: "2030-10-02T18:00".into(),
        repeat: "weekly".into(),
        every: 1,
        pauses: pauses.iter().map(|p| p.to_string()).collect(),
        ..EventEdit::default()
    };
    agenda::new_event(&edit, &paris()).unwrap()
}

fn holidays(id: &str, kind: Holidays, periods: &[(&str, &str)], as_of: Option<i64>) -> HolidayDays {
    HolidayDays { id: id.into(), name: id.into(), holidays: kind, days_off: kind == Holidays::Public, periods: periods.iter().map(|(a, b)| (day(a), day(b), String::new())).collect(), as_of }
}

fn config(vacations: &[(&str, &str)]) -> Config {
    let mut config = Config::default();
    config.time_off = vacations.iter().map(|(a, b)| crate::config::TimeOff { id: String::new(), from: day(a), until: day(b), label: String::new(), kind: String::new() }).collect();
    config
}

/// The stretch kept: from 1 October 2030, eighteen months.
fn stretch() -> (i64, i64) {
    let from = at("2030-10-01T00:00");
    (from, from + AHEAD_DAYS * 86_400)
}

fn left_out(text: &str) -> Vec<i64> {
    let mut out = agenda::left_out_times(text, &paris());
    out.sort_unstable();
    out
}

/// A series ends never, on a day or after a number of times, written as
/// the standard rule: a day on the series' clock for a whole-day one, in
/// UTC for a timed one; read back as written; a rule's days written by
/// another program kept when its end changes; the card's words.
#[test]
fn a_series_ends_each_way() {
    let zone = paris();
    let timed = EventEdit { title: "Class".into(), start: "2030-10-02T17:00".into(), end: "2030-10-02T18:00".into(), repeat: "weekly".into(), every: 2, ends: "until".into(), until: "2031-06-25".into(), ..EventEdit::default() };
    let text = agenda::new_event(&timed, &zone).unwrap();
    assert!(text.contains("RRULE:FREQ=WEEKLY;INTERVAL=2;UNTIL=20310625T215959Z"), "{text}");
    let back = agenda::edit_of_text(&text, &zone).unwrap();
    assert_eq!((back.ends.as_str(), back.until.as_str(), back.count), ("until", "2031-06-25", 0));
    let whole = EventEdit { title: "Market".into(), start: "2030-10-05".into(), end: "2030-10-05".into(), all_day: true, repeat: "weekly".into(), every: 1, ends: "count".into(), count: 10, ..EventEdit::default() };
    let text = agenda::new_event(&whole, &zone).unwrap();
    assert!(text.contains("RRULE:FREQ=WEEKLY;COUNT=10"), "{text}");
    assert_eq!(agenda::edit_of_text(&text, &zone).unwrap().count, 10);
    let until_day = agenda::apply(&text, &EventEdit { ends: "until".into(), until: "2030-12-21".into(), count: 0, ..agenda::edit_of_text(&text, &zone).unwrap() }, &zone).unwrap();
    assert!(until_day.contains("RRULE:FREQ=WEEKLY;UNTIL=20301221") && !until_day.contains("COUNT"), "{until_day}");
    // Another program's days kept, its end changed by the form.
    let theirs = text.replace("RRULE:FREQ=WEEKLY;COUNT=10", "RRULE:FREQ=WEEKLY;BYDAY=SA,SU;COUNT=10");
    let never = agenda::apply(&theirs, &EventEdit { ends: String::new(), count: 0, ..agenda::edit_of_text(&theirs, &zone).unwrap() }, &zone).unwrap();
    assert!(never.contains("RRULE:FREQ=WEEKLY;BYDAY=SA,SU\r\n"), "{never}");
    // Nothing said of an end on what does not repeat.
    let once = agenda::new_event(&EventEdit { repeat: String::new(), ends: "count".into(), count: 3, pauses: vec!["public".into()], ..whole.clone() }, &zone).unwrap();
    assert!(!once.contains("RRULE") && !once.contains(PAUSE_DURING));
    let tr = crate::i18n::Translator::new("en");
    assert_eq!(rule_words(&back, day("2030-10-01"), &tr), "Every 2 weeks until Wednesday 25 June 2031");
    assert_eq!(rule_words(&agenda::edit_of_text(&text, &zone).unwrap(), day("2030-10-01"), &tr), "Every week, 10 times");
    let fr = crate::i18n::Translator::new("fr");
    assert!(rule_words(&back, day("2030-10-01"), &fr).starts_with("Toutes les 2 semaines jusqu"));
}

/// Pauses from each source, and from two together: each time in a day of a
/// source left out by the standard means and noted for that source; the
/// times outside the stretch left as they were; nothing changes the second time.
#[test]
fn pauses_from_each_source_and_together() {
    let zone = paris();
    let (from, to) = stretch();
    let public = holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(100));
    let school = holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-11-03")], Some(100));
    let days = Days::with(&config(&[("2031-02-10", "2031-02-16")]), &[public, school], 50, 200);
    // My vacations alone.
    let vacations = keep(&class(&["vacations"]), &days, from, to, &zone, 100).unwrap();
    assert_eq!(left_out(&vacations), [at("2031-02-12T17:00")]);
    assert!(vacations.contains("X-SIOUL-PAUSED;X-SOURCE=vacations:20310212T160000Z"), "{vacations}");
    assert!(vacations.contains("X-SIOUL-PAUSED-AS-OF:19700101T000140Z"));
    // School holidays alone: the two Wednesdays of the zone's holidays.
    let school_only = keep(&class(&["school/zone-a"]), &days, from, to, &zone, 100).unwrap();
    assert_eq!(left_out(&school_only), [at("2030-10-23T17:00"), at("2030-10-30T17:00")]);
    // Public holidays: Christmas falls on a Wednesday.
    let public_only = keep(&class(&["public"]), &days, from, to, &zone, 100).unwrap();
    assert_eq!(left_out(&public_only), [at("2030-12-25T17:00")]);
    // Two together, each noted for its own.
    let both = keep(&class(&["public", "school/zone-a"]), &days, from, to, &zone, 100).unwrap();
    assert_eq!(left_out(&both).len(), 3);
    let noted = paused_of(&both, &zone);
    assert_eq!((noted[&Source::Public].len(), noted[&Source::School("zone-a".into())].len()), (1, 2));
    assert_eq!(keep(&both, &days, from, to, &zone, 100), None, "settled: nothing to write");
    // The card: the next pause and when the series resumes.
    let tr = crate::i18n::Translator::new("en");
    let now = jiff::Timestamp::from_second(at("2030-10-15T09:00")).unwrap().to_zoned(zone.clone());
    let line = pause_line(&both, &now, &|s: &Source| s.id(), &tr);
    assert_eq!(line, "Paused from Wednesday 23 October (school/zone-a), back on Wednesday 6 November.");
    // Its occurrences shown: the paused ones are not.
    let shown = agenda::rule_times(&both, from, at("2030-11-10T00:00"), &zone).unwrap();
    assert_eq!(shown.len(), 6, "the rule's own times, its left-out ones among them");
}

/// A date left out by hand stays out: never noted, never put back, even
/// when a holiday comes and goes on it.
#[test]
fn a_date_left_out_by_hand_is_kept() {
    let zone = paris();
    let (from, to) = stretch();
    let mine = agenda::skip_occurrence_in(&class(&["public"]), at("2030-12-25T17:00"), &zone).unwrap();
    let christmas = Days::with(&Config::default(), &[holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(100))], 50, 200);
    assert_eq!(keep(&mine, &christmas, from, to, &zone, 100), None, "left out by hand already: nothing to add");
    let none = Days::with(&Config::default(), &[holidays("fr", Holidays::Public, &[], Some(300))], 50, 400);
    assert_eq!(keep(&mine, &none, from, to, &zone, 300), None, "never put back");
    assert_eq!(left_out(&mine), [at("2030-12-25T17:00")]);
}

/// A holiday moved: the time it left is put back, the time it reaches left
/// out; removed: put back. Times before the stretch stay as they were.
#[test]
fn a_holiday_moved_then_removed() {
    let zone = paris();
    let (from, to) = stretch();
    let first = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-10-26")], Some(100))], 50, 200);
    let paused = keep(&class(&["school/zone-a"]), &first, from, to, &zone, 100).unwrap();
    assert_eq!(left_out(&paused), [at("2030-10-23T17:00")]);
    let moved = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-27", "2030-11-02")], Some(150))], 50, 250);
    let after_move = keep(&paused, &moved, from, to, &zone, 150).unwrap();
    assert_eq!(left_out(&after_move), [at("2030-10-30T17:00")]);
    let gone = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[], Some(200))], 50, 300);
    let after_removal = keep(&after_move, &gone, from, to, &zone, 200).unwrap();
    assert!(left_out(&after_removal).is_empty());
    assert!(!after_removal.contains(PAUSED_AS_OF) && !after_removal.contains("X-SIOUL-PAUSED;"));
    assert!(after_removal.contains("X-SIOUL-PAUSE-DURING:school/zone-a"), "what it pauses for stays");
    // A vacation typed, moved, removed: the same.
    let typed = keep(&class(&["vacations"]), &Days::with(&config(&[("2030-11-04", "2030-11-08")]), &[], 50, 300), from, to, &zone, 300).unwrap();
    assert_eq!(left_out(&typed), [at("2030-11-06T17:00")]);
    let removed = keep(&typed, &Days::with(&config(&[]), &[], 60, 400), from, to, &zone, 400).unwrap();
    assert!(left_out(&removed).is_empty());
    // A time before the stretch stays as it was.
    let later = (at("2030-11-01T00:00"), at("2030-11-01T00:00") + AHEAD_DAYS * 86_400);
    assert_eq!(keep(&paused, &gone, later.0, later.1, &zone, 200), None, "October's pause lies before the stretch now");
}

/// Two devices that disagree: the one whose copy of the holidays is the
/// freshest writes; an older copy never undoes a newer one, whether the
/// newer was noted by another device or in the series itself; a device
/// that never read a calendar the series pauses for writes nothing.
#[test]
fn two_devices_the_freshest_writes() {
    let zone = paris();
    let (from, to) = stretch();
    let sources = [Source::School("zone-a".into())];
    // The desk read the zone's file after the holidays were added; the phone, before.
    let desk = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-10-26")], Some(1_000))], 50, 2_000);
    let phone = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[], Some(500))], 50, 2_000);
    let text = class(&["school/zone-a"]);
    assert!(may_write(desk.view(&sources), &[], &text));
    let written = keep(&text, &desk, from, to, &zone, crate::availability::as_of(desk.view(&sources).unwrap())).unwrap();
    assert_eq!(as_of(&written), Some(1_000));
    // The phone's older copy undoes nothing: neither by the desk's sealed entry, nor by the series' own line.
    assert!(!may_write(phone.view(&sources), &[1_000], &text));
    assert!(!may_write(phone.view(&sources), &[], &written));
    // Read again after the desk, the phone writes the same as the desk would.
    let phone_later = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-10-26")], Some(1_500))], 50, 2_000);
    assert!(may_write(phone_later.view(&sources), &[1_000], &written));
    assert_eq!(keep(&written, &phone_later, from, to, &zone, 1_500), None, "the same copy: nothing to write");
    // Never read here: nothing written.
    let never = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-10-26")], None)], 50, 2_000);
    assert_eq!(never.view(&sources), None);
    assert!(!may_write(never.view(&sources), &[], &text));
    // A school calendar stopped everywhere: no day, known now; its pauses put back.
    let stopped = Days::with(&Config::default(), &[], 50, 3_000);
    assert_eq!(stopped.view(&sources), Some((3_000, 3_000)));
    assert!(left_out(&keep(&written, &stopped, from, to, &zone, 3_000).unwrap()).is_empty());
}

/// School holidays that begin after class, and the official files'
/// zero-length events, as `subscribed::holiday_periods` shapes them: a
/// series pauses from the day after the departure, resumes on the morning
/// classes resume, and a zero-length event pauses nothing.
#[test]
fn school_holidays_begin_after_class() {
    let zone = paris();
    let (from, to) = stretch();
    // The departure Wednesday 23 October (after class), classes resume Wednesday 6 November: as shaped, 24 October to 5 November.
    let shaped = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-24", "2030-11-05")], Some(100))], 50, 200);
    let paused = keep(&class(&["school/zone-a"]), &shaped, from, to, &zone, 100).unwrap();
    assert_eq!(left_out(&paused), [at("2030-10-30T17:00")], "the departure day is a class day, the day classes resume is no pause");
    // A zero-length bridge gives no day: nothing paused, nothing written.
    let bridge = Days::with(&Config::default(), &[holidays("zone-a", Holidays::School, &[], Some(100))], 50, 200);
    assert_eq!(keep(&class(&["school/zone-a"]), &bridge, from, to, &zone, 100), None);
}

/// The pause lines written and read; a choice that does not read is passed over.
#[test]
fn pause_choices_are_read_and_written() {
    let text = class(&["public", "vacations", "school/zone-a"]);
    assert_eq!(sources_of(&text), [Source::Public, Source::School("zone-a".into()), Source::Vacations]);
    let fewer = with_sources(&text, &[Source::Vacations]).unwrap();
    assert_eq!(sources_of(&fewer), [Source::Vacations]);
    assert_eq!(Source::parse("school/../x"), None);
    assert_eq!(Source::parse("weekends"), None);
    let form = agenda::edit_of_text(&text, &paris()).unwrap();
    assert_eq!(form.pauses, ["public", "school/zone-a", "vacations"]);
    // A form unchanged writes nothing; a pause unticked writes its line alone.
    assert_eq!(agenda::apply(&text, &form, &paris()).unwrap(), text);
    let unticked = agenda::apply(&text, &EventEdit { pauses: vec!["public".into()], ..form.clone() }, &paris()).unwrap();
    assert_eq!(sources_of(&unticked), [Source::Public]);
    assert!(!unticked.contains("SEQUENCE:1"), "Sioul's own lines: no revision");
}

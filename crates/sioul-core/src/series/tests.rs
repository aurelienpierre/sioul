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

/// The owner id of the settings of the tests' person, and of another one's.
const MINE: &str = "a1b2c3";
const THEIRS: &str = "f9e8d7";

/// "vacations/<MINE>".
fn vacations() -> String {
    format!("vacations/{MINE}")
}

/// Settings holding these vacations, named `owner`'s, last edited at `edited` (Unix seconds).
fn config_of(owner: &str, vacations: &[(&str, &str)], edited: i64) -> Config {
    let mut config = Config::default();
    config.time_off = vacations.iter().map(|(a, b)| crate::config::TimeOff { id: String::new(), from: day(a), until: day(b), label: String::new(), kind: String::new() }).collect();
    config.vacations = crate::config::VacationsMark { owner: owner.into(), stamp: jiff::Timestamp::from_second(edited).unwrap().to_string() };
    config
}

fn config(vacations: &[(&str, &str)]) -> Config {
    config_of(MINE, vacations, 500)
}

/// What this device knows, with no former version of any calendar.
fn days_of(config: &Config, holidays: &[HolidayDays]) -> Days {
    Days::with(config, holidays, &HeldBefore::default())
}

/// The same, having held `before` (source id, former versions).
fn days_having_held(config: &Config, holidays: &[HolidayDays], before: &[(&str, Vec<String>)]) -> Days {
    Days::with(config, holidays, &HeldBefore { held: before.iter().map(|(s, v)| (s.to_string(), v.clone())).collect() })
}

/// The version a device holding these days of a calendar notes.
fn held_of(periods: &[(&str, &str)]) -> String {
    held(&spans(periods.iter().map(|(a, b)| (day(a), day(b)))))
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
/// source left out by the standard means and noted for that source, with
/// that source's version; the times outside the stretch left as they were;
/// nothing changes the second time.
#[test]
fn pauses_from_each_source_and_together() {
    let zone = paris();
    let (from, to) = stretch();
    let public = holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(100));
    let school = holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-11-03")], Some(100));
    let days = days_of(&config(&[("2031-02-10", "2031-02-16")]), &[public, school]);
    // My vacations alone.
    let mine = keep(&class(&[&vacations()]), &days, from, to, &zone).unwrap();
    assert_eq!(left_out(&mine), [at("2031-02-12T17:00")]);
    assert!(mine.contains(&format!("X-SIOUL-PAUSED;X-SOURCE=vacations/{MINE}:20310212T160000Z")), "{mine}");
    let stamp = format!("19700101T000820Z.{}", held_of(&[("2031-02-10", "2031-02-16")]));
    assert_eq!(versions_of(&mine)[&Source::Vacations(MINE.into())], stamp, "{mine}");
    // School holidays alone: the two Wednesdays of the zone's holidays.
    let school_only = keep(&class(&["school/zone-a"]), &days, from, to, &zone).unwrap();
    assert_eq!(left_out(&school_only), [at("2030-10-23T17:00"), at("2030-10-30T17:00")]);
    assert_eq!(versions_of(&school_only)[&Source::School("zone-a".into())], held_of(&[("2030-10-20", "2030-11-03")]));
    // Public holidays: Christmas falls on a Wednesday.
    let public_only = keep(&class(&["public"]), &days, from, to, &zone).unwrap();
    assert_eq!(left_out(&public_only), [at("2030-12-25T17:00")]);
    // Two together, each noted for its own.
    let both = keep(&class(&["public", "school/zone-a"]), &days, from, to, &zone).unwrap();
    assert_eq!(left_out(&both).len(), 3);
    let noted = paused_of(&both, &zone);
    assert_eq!((noted[&Source::Public].len(), noted[&Source::School("zone-a".into())].len()), (1, 2));
    assert_eq!(keep(&both, &days, from, to, &zone), None, "settled: nothing to write");
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
    let christmas = days_of(&Config::default(), &[holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(100))]);
    let pinned = keep(&mine, &christmas, from, to, &zone).unwrap();
    assert!(paused_of(&pinned, &zone).is_empty(), "left out by hand already: not noted as Sioul's");
    assert_eq!(keep(&pinned, &christmas, from, to, &zone), None);
    let none = days_having_held(&Config::default(), &[holidays("fr", Holidays::Public, &[], Some(300))], &[("public", vec![held_of(&[("2030-12-25", "2030-12-25")])])]);
    assert_eq!(keep(&pinned, &none, from, to, &zone).as_deref().map(left_out), None, "never put back");
    assert_eq!(left_out(&pinned), [at("2030-12-25T17:00")]);
}

/// A holiday moved: the time it left is put back, the time it reaches left
/// out; removed: put back. Times before the stretch stay as they were.
#[test]
fn a_holiday_moved_then_removed() {
    let zone = paris();
    let (from, to) = stretch();
    let v1 = [("2030-10-20", "2030-10-26")];
    let v2 = [("2030-10-27", "2030-11-02")];
    let first = days_of(&Config::default(), &[holidays("zone-a", Holidays::School, &v1, Some(100))]);
    let paused = keep(&class(&["school/zone-a"]), &first, from, to, &zone).unwrap();
    assert_eq!(left_out(&paused), [at("2030-10-23T17:00")]);
    let moved = days_having_held(&Config::default(), &[holidays("zone-a", Holidays::School, &v2, Some(150))], &[("school/zone-a", vec![held_of(&v1)])]);
    let after_move = keep(&paused, &moved, from, to, &zone).unwrap();
    assert_eq!(left_out(&after_move), [at("2030-10-30T17:00")]);
    let gone = days_having_held(&Config::default(), &[holidays("zone-a", Holidays::School, &[], Some(200))], &[("school/zone-a", vec![held_of(&v1), held_of(&v2)])]);
    let after_removal = keep(&after_move, &gone, from, to, &zone).unwrap();
    assert!(left_out(&after_removal).is_empty());
    assert!(!after_removal.contains("X-SIOUL-PAUSED;"));
    assert_eq!(versions_of(&after_removal)[&Source::School("zone-a".into())], held_of(&[]), "the version it was settled from");
    assert!(after_removal.contains("X-SIOUL-PAUSE-DURING:school/zone-a"), "what it pauses for stays");
    // A vacation typed, moved, removed: the same.
    let typed = keep(&class(&[&vacations()]), &days_of(&config_of(MINE, &[("2030-11-04", "2030-11-08")], 300), &[]), from, to, &zone).unwrap();
    assert_eq!(left_out(&typed), [at("2030-11-06T17:00")]);
    let removed = keep(&typed, &days_of(&config_of(MINE, &[], 400), &[]), from, to, &zone).unwrap();
    assert!(left_out(&removed).is_empty());
    // A time before the stretch stays as it was.
    let later = (at("2030-11-01T00:00"), at("2030-11-01T00:00") + AHEAD_DAYS * 86_400);
    assert_eq!(keep(&paused, &gone, later.0, later.1, &zone), None, "October's pause lies before the stretch now");
}

/// The review's F32: a series pausing for two sources whose versions are of
/// different ages is written again by the device that wrote it (the first
/// rule compared the oldest of one source with the newest of the other).
#[test]
fn a_series_of_two_sources_is_written_twice_by_one_device() {
    let zone = paris();
    let (from, to) = stretch();
    let christmas = [("2030-12-25", "2030-12-25")];
    let public = holidays("fr", Holidays::Public, &christmas, Some(1_000));
    let desk = days_of(&config_of(MINE, &[("2031-02-10", "2031-02-16")], 500), std::slice::from_ref(&public));
    let written = keep(&class(&[&vacations(), "public"]), &desk, from, to, &zone).unwrap();
    assert_eq!(left_out(&written), [at("2030-12-25T17:00"), at("2031-02-12T17:00")]);
    // A pause unticked a moment later on the same device: written again.
    let unticked = with_sources(&written, &[Source::Public]).unwrap();
    let again = keep(&unticked, &desk, from, to, &zone).expect("the device that wrote it writes it again");
    assert_eq!(left_out(&again), [at("2030-12-25T17:00")]);
    // A vacation added on that device the next week: written too.
    let later = days_of(&config_of(MINE, &[("2031-02-10", "2031-02-16"), ("2031-03-03", "2031-03-07")], 900), &[public]);
    let more = keep(&written, &later, from, to, &zone).unwrap();
    assert_eq!(left_out(&more), [at("2030-12-25T17:00"), at("2031-02-12T17:00"), at("2031-03-05T17:00")]);
}

/// The review's F33: a device, or another person's Sioul, that does not
/// know a source leaves its times out as they are: "not here" is not "no day".
#[test]
fn a_device_without_the_calendar_leaves_the_pauses() {
    let zone = paris();
    let (from, to) = stretch();
    let desk = days_of(&Config::default(), &[holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(1_000))]);
    let written = keep(&class(&["public"]), &desk, from, to, &zone).unwrap();
    assert_eq!(left_out(&written), [at("2030-12-25T17:00")]);
    // Another device with no calendar of public holidays at all; one that never read it.
    let other = days_of(&Config::default(), &[]);
    assert_eq!(keep(&written, &other, from, to, &zone), None, "nothing taken back");
    let unread = days_of(&Config::default(), &[holidays("fr", Holidays::Public, &[], None)]);
    assert_eq!(keep(&written, &unread, from, to, &zone), None, "never read here: nothing taken back");
    // A school calendar this device has no word of (stopped, not arrived, someone else's): the same.
    let school = keep(&class(&["school/zone-a"]), &days_of(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-10-26")], Some(100))]), from, to, &zone).unwrap();
    assert_eq!(keep(&school, &other, from, to, &zone), None);
}

/// Another person's Sioul, writing the same shared calendar: their
/// vacations are another source than yours, and theirs is never yours.
#[test]
fn another_persons_sioul_leaves_your_vacations() {
    let zone = paris();
    let (from, to) = stretch();
    let me = days_of(&config_of(MINE, &[("2031-02-10", "2031-02-16")], 500), &[]);
    let written = keep(&class(&[&vacations()]), &me, from, to, &zone).unwrap();
    // Their settings: other vacations, newer, under their own owner id.
    let them = days_of(&config_of(THEIRS, &[("2030-11-04", "2030-11-08")], 9_000), &[]);
    assert_eq!(keep(&written, &them, from, to, &zone), None, "your vacations are not theirs to settle");
    // A series of theirs pausing during their vacations: theirs alone are left out, and you leave them.
    let theirs = keep(&class(&[&format!("vacations/{THEIRS}")]), &them, from, to, &zone).unwrap();
    assert_eq!(left_out(&theirs), [at("2030-11-06T17:00")]);
    assert_eq!(keep(&theirs, &me, from, to, &zone), None);
}

/// Two sources on one series, each known on one device alone: each device
/// keeps its own source's times and leaves the other's; neither undoes the other.
#[test]
fn two_sources_two_devices_each_keeps_its_own() {
    let zone = paris();
    let (from, to) = stretch();
    let public = holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(100));
    let school = holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-10-26")], Some(100));
    let desk = days_of(&Config::default(), &[public]);
    let phone = days_of(&Config::default(), &[school]);
    let text = class(&["public", "school/zone-a"]);
    let by_desk = keep(&text, &desk, from, to, &zone).unwrap();
    assert_eq!(left_out(&by_desk), [at("2030-12-25T17:00")]);
    let by_phone = keep(&by_desk, &phone, from, to, &zone).unwrap();
    assert_eq!(left_out(&by_phone), [at("2030-10-23T17:00"), at("2030-12-25T17:00")], "the phone adds its own, keeps the desk's");
    assert_eq!(keep(&by_phone, &desk, from, to, &zone), None, "and the desk then writes nothing");
    assert_eq!(keep(&by_phone, &phone, from, to, &zone), None);
}

/// Versions: an older copy never undoes a newer one; a device that held the
/// version noted and holds a newer one now writes; one that never held the
/// version noted writes nothing until it holds the same; the same copy
/// writes nothing.
#[test]
fn a_newer_version_writes_an_older_one_does_not() {
    let zone = paris();
    let (from, to) = stretch();
    let v1: [(&str, &str); 0] = [];
    let v2 = [("2030-10-20", "2030-10-26")];
    let source = "school/zone-a";
    let text = class(&[source]);
    // The desk read the zone's file after the holidays were added; the phone, before.
    let desk = days_having_held(&Config::default(), &[holidays("zone-a", Holidays::School, &v2, Some(1_000))], &[(source, vec![held_of(&v1)])]);
    let phone = days_of(&Config::default(), &[holidays("zone-a", Holidays::School, &v1, Some(500))]);
    let written = keep(&text, &desk, from, to, &zone).unwrap();
    assert_eq!(left_out(&written), [at("2030-10-23T17:00")]);
    assert_eq!(keep(&written, &phone, from, to, &zone), None, "the phone's older copy undoes nothing");
    // Read again after the desk, the phone writes nothing: the same copy.
    let phone_later = days_having_held(&Config::default(), &[holidays("zone-a", Holidays::School, &v2, Some(1_500))], &[(source, vec![held_of(&v1)])]);
    assert_eq!(keep(&written, &phone_later, from, to, &zone), None);
    // Written by the phone from its older copy first: the desk, which held it, writes over it.
    let by_phone = keep(&text, &phone, from, to, &zone).unwrap();
    assert!(left_out(&by_phone).is_empty());
    assert_eq!(left_out(&keep(&by_phone, &desk, from, to, &zone).unwrap()), [at("2030-10-23T17:00")]);
    // A device that never held the version noted (subscribed later, or another person's other copy): nothing.
    let stranger = days_of(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-27", "2030-11-02")], Some(2_000))]);
    assert_eq!(keep(&written, &stranger, from, to, &zone), None);
}

/// Your vacations' version: edited later wins, the same days write nothing,
/// an older edit never undoes a newer one, and settings of the same time
/// that hold other days (changed by hand, or by an older Sioul) touch nothing.
#[test]
fn your_vacations_newer_settings_write() {
    let zone = paris();
    let (from, to) = stretch();
    let text = class(&[&vacations()]);
    let computer = days_of(&config_of(MINE, &[("2030-11-04", "2030-11-08")], 1_000), &[]);
    let written = keep(&text, &computer, from, to, &zone).unwrap();
    // The phone before the vacation reached it: older settings, nothing undone.
    let phone_before = days_of(&config_of(MINE, &[], 500), &[]);
    assert_eq!(keep(&written, &phone_before, from, to, &zone), None);
    // The phone once it arrived: the same, nothing to write.
    let phone_after = days_of(&config_of(MINE, &[("2030-11-04", "2030-11-08")], 1_000), &[]);
    assert_eq!(keep(&written, &phone_after, from, to, &zone), None);
    // The vacation removed on the phone later: put back by the phone, left so by the computer until it hears.
    let removed = keep(&written, &days_of(&config_of(MINE, &[], 2_000), &[]), from, to, &zone).unwrap();
    assert!(left_out(&removed).is_empty());
    assert_eq!(keep(&removed, &computer, from, to, &zone), None, "the computer's older settings undo nothing");
    // The same stamp, other days (an edit by hand): nothing either way.
    let by_hand = days_of(&config_of(MINE, &[("2030-11-11", "2030-11-15")], 1_000), &[]);
    assert_eq!(keep(&written, &by_hand, from, to, &zone), None);
}

/// A series saved on one device, then on the other: what each form writes
/// keeps the other's pauses, and each device's round then settles only
/// what it knows.
#[test]
fn a_series_saved_on_one_device_then_on_the_other() {
    let zone = paris();
    let (from, to) = stretch();
    let public = holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(100));
    let desk = days_of(&config(&[("2031-02-10", "2031-02-16")]), &[public]);
    let phone = days_of(&config(&[("2031-02-10", "2031-02-16")]), &[]);
    let saved = keep(&class(&[&vacations(), "public"]), &desk, from, to, &zone).unwrap();
    // On the phone, the title changed in the form: its pauses come back as they were.
    let form = agenda::edit_of_text(&saved, &zone).unwrap();
    assert_eq!(form.pauses, ["public".to_string(), vacations()]);
    let renamed = agenda::apply(&saved, &EventEdit { title: "Swimming club".into(), ..form }, &zone).unwrap();
    assert_eq!(left_out(&renamed), left_out(&saved));
    assert_eq!(keep(&renamed, &phone, from, to, &zone), None, "the phone knows your vacations as the desk does, and not the public holidays");
    // Moved to Thursdays on the phone: the phone settles your vacations' times, keeps the public holidays' noted.
    let form = agenda::edit_of_text(&renamed, &zone).unwrap();
    let thursdays = agenda::apply(&renamed, &EventEdit { start: "2030-10-03T17:00".into(), end: "2030-10-03T18:00".into(), ..form }, &zone).unwrap();
    let by_phone = keep(&thursdays, &phone, from, to, &zone).unwrap();
    assert_eq!(left_out(&by_phone), [at("2031-02-13T17:00")], "Wednesday's times are forgotten, the vacation's Thursday left out");
    // Then on the desk: Christmas is a Wednesday; Boxing Day, a Thursday, is no holiday here.
    assert_eq!(keep(&by_phone, &desk, from, to, &zone), None);
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
    let shaped = days_of(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-24", "2030-11-05")], Some(100))]);
    let paused = keep(&class(&["school/zone-a"]), &shaped, from, to, &zone).unwrap();
    assert_eq!(left_out(&paused), [at("2030-10-30T17:00")], "the departure day is a class day, the day classes resume is no pause");
    // A zero-length bridge gives no day: nothing paused, only the version noted.
    let bridge = days_of(&Config::default(), &[holidays("zone-a", Holidays::School, &[], Some(100))]);
    let pinned = keep(&class(&["school/zone-a"]), &bridge, from, to, &zone).unwrap();
    assert!(left_out(&pinned).is_empty() && !pinned.contains("X-SIOUL-PAUSED;"));
    assert_eq!(keep(&pinned, &bridge, from, to, &zone), None);
}

/// A floating series (no zone): its noted times are local times, so that a
/// device in another time zone still knows them for Sioul's own (F40).
#[test]
fn a_floating_series_notes_local_times() {
    let (from, to) = stretch();
    let zone = paris();
    let floating = class(&["public"]).replace("DTSTART;TZID=Europe/Paris:", "DTSTART:").replace("DTEND;TZID=Europe/Paris:", "DTEND:");
    assert!(floating.contains("DTSTART:20301002T170000"), "{floating}");
    let desk = days_of(&Config::default(), &[holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(100))]);
    let written = keep(&floating, &desk, from, to, &zone).unwrap();
    assert!(written.contains("X-SIOUL-PAUSED;X-SOURCE=public:20301225T170000\r\n"), "{written}");
    // Read in New York: the same local time, still Sioul's own; the holiday gone there, put back.
    let york = TimeZone::get("America/New_York").unwrap();
    let noted = paused_of(&written, &york);
    let local = jiff::civil::date(2030, 12, 25).at(17, 0, 0, 0).to_zoned(york.clone()).unwrap().timestamp().as_second();
    assert_eq!(noted[&Source::Public].iter().copied().collect::<Vec<_>>(), [local]);
    let gone = days_having_held(&Config::default(), &[holidays("fr", Holidays::Public, &[], Some(200))], &[("public", vec![held_of(&[("2030-12-25", "2030-12-25")])])]);
    let york_stretch = (jiff::civil::date(2030, 10, 1).to_zoned(york.clone()).unwrap().timestamp().as_second(), to);
    let back = keep(&written, &gone, york_stretch.0, york_stretch.1, &york).unwrap();
    assert!(agenda::left_out_times(&back, &york).is_empty(), "{back}");
}

/// The pause lines written and read; a choice that does not read is passed over.
#[test]
fn pause_choices_are_read_and_written() {
    let text = class(&["public", &vacations(), "school/zone-a"]);
    assert_eq!(sources_of(&text), [Source::Public, Source::School("zone-a".into()), Source::Vacations(MINE.into())]);
    let fewer = with_sources(&text, &[Source::Vacations(MINE.into())]).unwrap();
    assert_eq!(sources_of(&fewer), [Source::Vacations(MINE.into())]);
    assert_eq!(Source::parse("school/../x"), None);
    assert_eq!(Source::parse("weekends"), None);
    assert_eq!(Source::parse(MY_VACATIONS), None, "the form's word alone is named yours before it is written");
    assert_eq!(Source::parse("vacations/a b"), None);
    let form = agenda::edit_of_text(&text, &paris()).unwrap();
    assert_eq!(form.pauses, ["public".to_string(), "school/zone-a".into(), vacations()]);
    // A form unchanged writes nothing; a pause unticked writes its line alone.
    assert_eq!(agenda::apply(&text, &form, &paris()).unwrap(), text);
    let unticked = agenda::apply(&text, &EventEdit { pauses: vec!["public".into()], ..form.clone() }, &paris()).unwrap();
    assert_eq!(sources_of(&unticked), [Source::Public]);
    assert!(!unticked.contains("SEQUENCE:1"), "Sioul's own lines: no revision");
    // Spans: the same days, the same version, whatever the periods.
    assert_eq!(held(&spans([(day("2030-01-01"), day("2030-01-03")), (day("2030-01-04"), day("2030-01-05"))])), held(&spans([(day("2030-01-01"), day("2030-01-05"))])));
}

/// The review's F27: settings that do not read give the defaults, which
/// know neither your vacations (no owner id) nor any calendar of holidays:
/// nothing is taken back on their word (the window's round does not even
/// run on them, `config_read`).
#[test]
fn settings_that_do_not_read_take_nothing_back() {
    let zone = paris();
    let (from, to) = stretch();
    let desk = days_of(&config(&[("2031-02-10", "2031-02-16")]), &[holidays("fr", Holidays::Public, &[("2030-12-25", "2030-12-25")], Some(100))]);
    let written = keep(&class(&[&vacations(), "public"]), &desk, from, to, &zone).unwrap();
    assert_eq!(left_out(&written).len(), 2);
    assert_eq!(keep(&written, &days_of(&Config::default(), &[]), from, to, &zone), None);
}

/// The review's F35: a series that ends on a day, made a whole day: its
/// UNTIL takes the form of its DTSTART (RFC 5545 §3.3.10), and back; its
/// last day's time still comes.
#[test]
fn until_follows_the_start_when_it_changes_kind() {
    let zone = paris();
    let timed = EventEdit { title: "Class".into(), start: "2030-10-02T17:00".into(), end: "2030-10-02T18:00".into(), repeat: "weekly".into(), every: 1, ends: "until".into(), until: "2031-06-25".into(), ..EventEdit::default() };
    let text = agenda::new_event(&timed, &zone).unwrap();
    assert!(text.contains("UNTIL=20310625T215959Z"), "{text}");
    let form = agenda::edit_of_text(&text, &zone).unwrap();
    let whole = agenda::apply(&text, &EventEdit { all_day: true, start: "2030-10-02".into(), end: "2030-10-02".into(), clock: String::new(), ..form }, &zone).unwrap();
    assert!(whole.contains("DTSTART;VALUE=DATE:20301002"), "{whole}");
    assert!(whole.contains("UNTIL=20310625") && !whole.contains("UNTIL=20310625T"), "a date, as its start: {whole}");
    // The other way: a whole-day series until 25 June given a time keeps its last day's time.
    let form = agenda::edit_of_text(&whole, &zone).unwrap();
    assert_eq!((form.ends.as_str(), form.until.as_str()), ("until", "2031-06-25"));
    let again = agenda::apply(&whole, &EventEdit { all_day: false, start: "2030-10-02T17:00".into(), end: "2030-10-02T18:00".into(), ..form }, &zone).unwrap();
    let last = agenda::rule_times(&again, at("2031-06-25T00:00"), at("2031-06-26T00:00"), &zone).unwrap();
    assert_eq!(last.len(), 1, "the time of the last day still comes: {again}");
    // Another clock: the last second of the day on the new clock, in UTC.
    let form = agenda::edit_of_text(&again, &zone).unwrap();
    let york = agenda::apply(&again, &EventEdit { clock: "America/New_York".into(), ..form }, &zone).unwrap();
    assert!(york.contains("UNTIL=20310626T035959Z"), "{york}");
}

/// The review's F36 (the coordinator's choice): "after a number of times"
/// keeps the standard meaning, the dates in pauses counted, and the card
/// says it in plain words when some fall in one.
#[test]
fn a_counted_series_says_how_many_take_place() {
    let zone = paris();
    let (from, to) = stretch();
    let ten = agenda::new_event(&EventEdit { title: "Lessons".into(), start: "2030-10-02T17:00".into(), end: "2030-10-02T18:00".into(), repeat: "weekly".into(), every: 1, ends: "count".into(), count: 10, pauses: vec!["school/zone-a".into()], ..EventEdit::default() }, &zone).unwrap();
    assert!(ten.contains("COUNT=10"), "{ten}");
    let now = jiff::Timestamp::from_second(at("2030-09-15T09:00")).unwrap().to_zoned(zone.clone());
    let tr = crate::i18n::Translator::new("en");
    assert_eq!(count_words(&ten, &now, &tr), "", "nothing in a pause yet");
    let days = days_of(&Config::default(), &[holidays("zone-a", Holidays::School, &[("2030-10-20", "2030-11-03")], Some(100))]);
    let paused = keep(&ten, &days, from, to, &zone).unwrap();
    assert!(paused.contains("COUNT=10"), "the count stays the standard one: {paused}");
    assert_eq!(count_words(&paused, &now, &tr), "Every week: 10 dates, 2 of them in pauses: 8 take place, the last on Wednesday 4 December.");
    let fr = crate::i18n::Translator::new("fr");
    assert!(count_words(&paused, &now, &fr).contains("10 dates, dont 2 en pause"), "{}", count_words(&paused, &now, &fr));
}

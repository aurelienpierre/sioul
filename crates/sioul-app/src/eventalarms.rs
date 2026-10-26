// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Reminders before events on a phone (docs/android.md, "Events"): Android
//! freezes an app it does not show, so the coming reminders of events (their
//! own alarms, Sioul's before them, the working day before) are given ahead
//! to Android's alarm clock (android/package/src/com/aurelienpierre/sioul/
//! EventAlarms.java), as the doses are (`alarms`): at the window's minute,
//! when Sioul is put away, after each ring. At its time a receiver asks here
//! what to say (`sioul_event_decide`): the event read again (moved,
//! cancelled, told already?), sleep or a pause holding it or not, the time
//! left said as it is then; shown in the quiet "Events" channel, a tap
//! opening the event. Every device tells its own: one held back on the phone
//! because the computer was used a moment ago, then shown on an empty desk,
//! would be an appointment missed.
//!
//! Also here, Android's side of new mail's notification (`mail_note`,
//! MailNotes.java), and what a tapped notification opens (`opened`).
//! Elsewhere nothing: the window's minute and `sioul remind --watch` tell them.

use crate::backend::{load_config, tr};
use jiff::Zoned;
use jiff::tz::TimeZone;
use sioul_core::reminders::{self, Holds, Kind, Reminder};
use std::ffi::{CString, c_char};
use std::sync::Mutex;

#[cfg(target_os = "android")]
// SAFETY: declared as android/main.cpp defines them: extern "C", the same types.
unsafe extern "C" {
    /// The coming events' reminders handed to Java, which keeps them and sets their alarms (android/main.cpp).
    fn sioul_android_set_event_alarms(json: *const c_char);
    /// A reminder shown in Android's notifications, "Events" (android/main.cpp).
    fn sioul_android_event_note(json: *const c_char);
    /// New mail's notification, "New mail" (android/main.cpp).
    fn sioul_android_mail_note(json: *const c_char);
    /// The end of work's notice or the night's, with its buttons, "End of the day" (android/main.cpp).
    fn sioul_android_day_note(json: *const c_char);
    /// What a tapped notification opens, once, as JSON {kind, key}; false when nothing (android/main.cpp).
    fn sioul_android_take_reminder_opened(buffer: *mut c_char, size: i32) -> bool;
}

/// At most this many alarms given: Android keeps 500 per app, the doses' among them.
const AT_MOST: usize = 100;
/// Java asks again this long after the list was given, when nothing rang meanwhile…
const LOOK_AGAIN: i64 = 24 * 3600;
/// …in steps of this, so that the same list is not handed again each minute.
const LOOK_STEP: i64 = 6 * 3600;
/// A reminder held while nothing says when it may come (Free time ended, say) is asked about again this often.
const AGAIN: i64 = 15 * 60;

/// The list last handed to Java, so that it is handed again only when it changes.
static GIVEN: Mutex<String> = Mutex::new(String::new());

/// The words Java says, in your language, given with the list: Sioul may
/// not be running when a reminder is shown.
fn words() -> serde_json::Value {
    serde_json::json!({
        "channel": tr().text("events-channel", None),
        "mail": tr().text("mail-channel", None),
        "mail-through": tr().text("mail-through-channel", None),
        "alarms": tr().text("alarms-channel", None),
        "alarms-pause": tr().text("dnd-events-channel", None),
        "day": tr().text("review-channel", None),
        "open": tr().text("reminder-open", None),
    })
}

/// What Java is given: the events' reminders not told yet that still make
/// sense, nearest first, at most a hundred, each {key, at (Unix
/// milliseconds), title, body} (the words said if Sioul cannot be asked at
/// its time); one held now (you sleep) at the time it may come; `look`, when
/// to ask for the list again (a day on, in steps of six hours, so that the
/// list given is the same from one minute to the next); `words`.
pub(crate) fn coming(now: &Zoned) -> String {
    let (all, holds) = reminders::gather_events(&load_config(), tr(), now);
    listed(&all, &crate::hours::with_layers(holds), now)
}

/// The list Java is given, from reminders gathered already: the events' own,
/// then the times the end-of-work notice and the night's may come
/// (`reviews::coming_work`, `health::coming_night`), "day" their kind, with
/// no words: Rust alone says them, or nothing (`day_note`).
fn listed(all: &[Reminder], holds: &Holds, now: &Zoned) -> String {
    let stamp = now.timestamp().as_second();
    let dir = reminders::told_dir();
    let mut due: Vec<(i64, &Reminder)> = all
        .iter()
        .filter(|r| matches!(r.kind, Kind::Event | Kind::Before | Kind::Alarm) && r.until > stamp && !reminders::told(&dir, &r.key))
        .map(|r| (when(r, holds, stamp), r))
        .filter(|(at, r)| *at < r.until)
        .collect();
    due.sort_by_key(|(at, r)| (*at, r.key.clone()));
    let mut list: Vec<serde_json::Value> = due.iter().take(AT_MOST).map(|(at, r)| serde_json::json!({ "key": r.key, "kind": r.kind, "at": at * 1000, "title": r.title, "body": r.body })).collect();
    let days = crate::reviews::coming_work(now).into_iter().chain(crate::health::coming_night(now));
    list.extend(days.map(|(key, at)| serde_json::json!({ "key": key, "kind": "day", "at": at * 1000, "title": "", "body": "" })));
    let look = (stamp / LOOK_STEP + LOOK_AGAIN / LOOK_STEP) * LOOK_STEP;
    serde_json::json!({ "reminders": list, "look": look * 1000, "words": words() }).to_string()
}

/// When a reminder's alarm rings (Unix seconds): its time; past it, at once
/// when nothing holds it, else when the hold ends (waking).
fn when(reminder: &Reminder, holds: &Holds, stamp: i64) -> i64 {
    if reminder.at > stamp {
        reminder.at
    } else if reminder.ready_in(stamp, holds) {
        stamp + 1
    } else {
        again_at(holds, stamp)
    }
}

/// The list handed to Java when it changed since it was last.
fn hand(json: String) {
    let Ok(mut given) = GIVEN.lock() else { return };
    if *given == json {
        return;
    }
    let Ok(text) = CString::new(json.clone()) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_set_event_alarms(text.as_ptr())
    };
    let _ = text;
    *given = json;
}

/// The coming events' reminders given to Android's alarm clock when they
/// changed: as Sioul is put away, and after each ring. Nothing elsewhere.
pub(crate) fn schedule() {
    if !cfg!(target_os = "android") {
        return;
    }
    hand(coming(&Zoned::now()));
}

/// The same from the window's minute, with the reminders it gathered
/// (`remind::tick`): the calendars read once a minute, not twice.
pub(crate) fn schedule_from(all: &[Reminder], holds: &Holds, now: &Zoned) {
    if !cfg!(target_os = "android") {
        return;
    }
    hand(listed(all, holds, now));
}

/// A reminder shown in Android's notifications, "Events": its words, "Open",
/// a tap opening the event; gone from the shade an hour after the event began.
pub(crate) fn show(reminder: &Reminder) {
    let json = serde_json::json!({
        "key": reminder.key,
        "kind": reminder.kind,
        "title": reminder.title,
        "body": reminder.body,
        "event": reminder.target,
        "until": reminder.until * 1000,
        "words": words(),
    });
    let Ok(text) = CString::new(json.to_string()) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_event_note(text.as_ptr())
    };
    let _ = text;
}

/// New mail's notification on a phone (`mailnote`), in the quiet "New mail"
/// channel: one at a time, the next batch in its place; a tap opens the Porch.
/// `through`: someone Always through is in it, whom the system's
/// do-not-disturb should let through (a channel that passes, Java's to choose).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn mail_note(title: &str, body: &str, through: bool) {
    let json = serde_json::json!({ "title": title, "body": body, "through": through, "words": words() });
    let Ok(text) = CString::new(json.to_string()) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_mail_note(text.as_ptr())
    };
    let _ = text;
}

/// The end of work's notice or the night's on a phone (`reviews::decide_work`,
/// `health::decide_night`), in the quiet "End of the day" channel: its words,
/// and each button ({label, kind, key}) opening Sioul on what it offers, a tap
/// on the notice the first's (DayNotes.java); gone from the shade at `until`
/// (Unix seconds). Shown again in its place, by `key`, when it comes again.
pub(crate) fn day_note(key: &str, title: &str, body: &str, offered: &[crate::reviews::Offered], until: i64) {
    let json = day_note_json(key, title, body, offered, until, words());
    let Ok(text) = CString::new(json.to_string()) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_day_note(text.as_ptr())
    };
    let _ = text;
}

/// What DayNotes.java reads of a notice (`day_note`): {key, title, body,
/// buttons: [{label, kind, key}], until (Unix ms), words}.
fn day_note_json(key: &str, title: &str, body: &str, offered: &[crate::reviews::Offered], until: i64, words: serde_json::Value) -> serde_json::Value {
    let buttons: Vec<serde_json::Value> = offered.iter().map(|offer| serde_json::json!({ "label": offer.label, "kind": offer.kind, "key": offer.key })).collect();
    serde_json::json!({ "key": key, "title": title, "body": body, "buttons": buttons, "until": until * 1000, "words": words })
}

/// When a reminder held now may come: when the time now ends (waking, the
/// end of free time, the hours' end), a minute on at the soonest; while a
/// layer holds it (a slot of time for you, do-not-disturb) or the end is
/// unknown (a pause), at the next quarter of an hour (Unix seconds). In
/// steps, never "a minute from now": the list given to Java stays the same
/// from one call to the next, so it is not given again, its alarms set again,
/// every second.
fn again_at(holds: &Holds, stamp: i64) -> i64 {
    match holds.until {
        Some(until) if !holds.now.slot && !holds.now.dnd => until.max(step_after(stamp, 60)),
        _ => step_after(stamp, AGAIN),
    }
}

/// The first multiple of `step` seconds after `stamp`.
fn step_after(stamp: i64, step: i64) -> i64 {
    (stamp.div_euclid(step) + 1) * step
}

/// At a reminder's time (`EventAlarms`): {"shown": true} once told here; or
/// {"again_at": ms}, held now (you sleep) and still worth telling then; or
/// {} (moved, cancelled, told already, too late). The coming list is given
/// again after, as the calendars are now. The end of work's and the night's
/// ("day:work:…", "day:night:…") are theirs to decide.
fn decide(key: &str) -> String {
    let now = Zoned::now();
    if let Some(day) = key.strip_prefix("day:") {
        let answer = if day.starts_with("work:") {
            crate::reviews::decide_work(&now)
        } else if day.starts_with("night:") {
            crate::health::decide_night(&now)
        } else {
            serde_json::json!({})
        };
        schedule();
        return answer.to_string();
    }
    let stamp = now.timestamp().as_second();
    let (all, holds) = reminders::gather_events(&load_config(), tr(), &now);
    let holds = crate::hours::with_layers(holds);
    let dir = reminders::told_dir();
    let answer = match all.iter().find(|r| r.key == key) {
        Some(r) if r.ready_in(stamp, &holds) => {
            // Told by the window's minute already: nothing more.
            let shown = reminders::claim(&dir, &r.key);
            if shown {
                show(r);
            }
            serde_json::json!({ "shown": shown })
        }
        // Not yet due (an alarm a moment early), or held now (you sleep): asked again then.
        Some(r) if stamp < r.until && !reminders::told(&dir, &r.key) => {
            let again = when(r, &holds, stamp).max(stamp + 1);
            if again < r.until { serde_json::json!({ "again_at": again * 1000 }) } else { serde_json::json!({}) }
        }
        _ => serde_json::json!({}),
    };
    schedule();
    answer.to_string()
}

/// `decide`, for Java (android/main.cpp); a panic answers `null`, and Java
/// shows the words it was given with the list rather than nothing. Given back
/// to `sioul_string_free`.
///
/// # Safety
/// `key` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_event_decide(key: *const c_char) -> *mut c_char {
    // SAFETY: `key` is null or a C string valid for the call, as this function's contract says.
    let key = unsafe { crate::alarms::key_of(key) };
    let answer = std::panic::catch_unwind(|| decide(&key)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

/// The coming list, for Java (`EventAlarms.ask`: after a restart, an update,
/// a change of time or of zone, and once a day), in `zone`, the phone's as
/// Java has it; "" when it fails, and Java keeps its list.
///
/// # Safety
/// `zone` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_event_coming(zone: *const c_char) -> *mut c_char {
    // SAFETY: `zone` is null or a C string valid for the call, as this function's contract says.
    let zone = unsafe { crate::alarms::key_of(zone) };
    let answer = std::panic::catch_unwind(|| {
        let zone = TimeZone::get(&zone).unwrap_or_else(|_| TimeZone::system());
        let json = coming(&Zoned::now().with_time_zone(zone));
        if let Ok(mut given) = GIVEN.lock() {
            *given = json.clone();
        }
        json
    })
    .unwrap_or_default();
    // The calls' table too, at this daily look: its frames never run out while Sioul stays closed.
    let _ = std::panic::catch_unwind(|| crate::calls::refresh(false));
    crate::alarms::handed(answer)
}

/// What a notification tapped while Sioul was away opens, once: ("event",
/// its file), ("porch", ""), a closing sheet ("review", "work" or "night",
/// with its day: "work:2026-10-14"), or a block's question ("need", its key).
pub(crate) fn opened() -> Option<(String, String)> {
    #[cfg(target_os = "android")]
    {
        let mut buffer = vec![0u8; 4096];
        // SAFETY: android/main.cpp's; it writes at most `buffer.len()` bytes into `buffer`, its zero included.
        let found = unsafe { sioul_android_take_reminder_opened(buffer.as_mut_ptr().cast(), buffer.len() as i32) };
        if found {
            let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
            return serde_json::from_slice(&buffer[..end]).ok().and_then(|said| opened_of(&said));
        }
    }
    None
}

/// What a tapped notification's {kind, key} opens (ReminderOpener.java keeps
/// it): only what Sioul's own notifications open; none for anything else.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn opened_of(said: &serde_json::Value) -> Option<(String, String)> {
    let key = said["key"].as_str().unwrap_or_default();
    let kind = said["kind"].as_str().filter(|kind| match *kind {
        "event" | "porch" | "need" => true,
        "review" => {
            let (sheet, day) = key.split_once(':').map_or((key, None), |(sheet, day)| (sheet, Some(day)));
            matches!(sheet, "work" | "night") && day.is_none_or(|day| day.parse::<jiff::civil::Date>().is_ok())
        }
        _ => false,
    })?;
    Some((kind.to_string(), key.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_reminders_are_asked_about_again_when_the_hold_ends() {
        use sioul_core::reminders::Wait;
        let stamp = 1_800_000_000;
        let usual = Holds::usual;
        assert_eq!(again_at(&usual(Wait::Sleep { from: stamp - 3600, until: stamp + 7200 }), stamp), stamp + 7200, "at waking");
        assert_eq!(again_at(&usual(Wait::Free { until: stamp + 600 }), stamp), stamp + 600, "at the end of free time");
        assert_eq!(again_at(&usual(Wait::Free { until: stamp }), stamp), stamp + 60, "never at once again");
        assert_eq!(again_at(&usual(Wait::Paused), stamp), stamp + AGAIN);
        // In steps: the same from one second to the next, so the list given to Java stays the same.
        for later in 0..AGAIN {
            assert_eq!(again_at(&usual(Wait::Paused), stamp + later), stamp + AGAIN, "{later} s on");
        }
        for later in 0..60 {
            assert_eq!(again_at(&usual(Wait::Free { until: stamp }), stamp + later), stamp + 60, "{later} s on");
        }
        assert_eq!(again_at(&usual(Wait::Paused), stamp + AGAIN), stamp + 2 * AGAIN, "never now");
        // A reminder's alarm: its time; due and free, at once; held (a train after waking), at waking.
        let reminder = |at: i64, until: i64| Reminder { key: "before:train:1".into(), kind: Kind::Before, at, until, title: String::new(), body: String::new(), target: String::new(), work: false, starts: Some(until), names: None };
        let night = usual(Wait::Sleep { from: stamp - 8 * 3600, until: stamp + 900 });
        assert_eq!(when(&reminder(stamp + 600, stamp + 3600), &usual(Wait::Nothing), stamp), stamp + 600);
        assert_eq!(when(&reminder(stamp - 60, stamp + 3600), &usual(Wait::Nothing), stamp), stamp + 1);
        assert_eq!(when(&reminder(stamp - 60, stamp + 3600), &night, stamp), stamp + 900, "kept in the list given, at waking");
        // Held during do-not-disturb (the matrix changed): asked again in a quarter of an hour.
        let mut focus = usual(Wait::Nothing);
        focus.now.dnd = true;
        focus.attention.set(sioul_core::attention::Row::Own(sioul_core::attention::Kind::Before), sioul_core::attention::Column::Dnd, sioul_core::attention::Level::Later).unwrap();
        assert_eq!(when(&reminder(stamp - 60, stamp + 3600), &focus, stamp), stamp + AGAIN);
    }

    /// A tapped notification opens only what Sioul's own open: an event, the
    /// Porch, a closing sheet (the work day's or the whole day's: the end of
    /// work's and the night's notices), a block's question; nothing else.
    #[test]
    fn a_tapped_notice_opens_only_what_sioul_offers() {
        let of = |json: &str| opened_of(&serde_json::from_str(json).unwrap());
        assert_eq!(of(r#"{"kind":"review","key":"work"}"#), Some(("review".into(), "work".into())));
        assert_eq!(of(r#"{"kind":"review","key":"night"}"#), Some(("review".into(), "night".into())));
        assert_eq!(of(r#"{"kind":"need","key":"sleep"}"#), Some(("need".into(), "sleep".into())));
        assert_eq!(of(r#"{"kind":"event","key":"work/1.ics"}"#), Some(("event".into(), "work/1.ics".into())));
        assert_eq!(of(r#"{"kind":"porch"}"#), Some(("porch".into(), String::new())));
        assert_eq!(of(r#"{"kind":"review","key":"work:2026-10-14"}"#), Some(("review".into(), "work:2026-10-14".into())));
        assert_eq!(of(r#"{"kind":"review","key":"night:2026-10-14"}"#), Some(("review".into(), "night:2026-10-14".into())));
        assert_eq!(of(r#"{"kind":"review","key":"work:yesterday"}"#), None);
        assert_eq!(of(r#"{"kind":"review","key":"settings"}"#), None);
        assert_eq!(of(r#"{"kind":"settings:words","key":""}"#), None);
        assert_eq!(of(r#"{"key":"work"}"#), None);
    }

    /// A Java string literal's text: its `\"` and `\uXXXX` read.
    fn unescaped(literal: &str) -> String {
        let mut out = String::new();
        let mut chars = literal.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.extend(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32));
                }
                Some(other) => out.push(other),
                None => {}
            }
        }
        out
    }

    /// The end of work's notice and the night's as Rust gives them to Java
    /// (`day_note`), in English: the very words android/jvm-checks/
    /// DayNotesCheck.java checks DayNotes.buttons on (WORK, NIGHT, GIVEN),
    /// read from that file, so that neither side changes alone.
    #[test]
    fn the_days_notices_as_java_reads_them() {
        let check = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/jvm-checks/DayNotesCheck.java")).unwrap();
        let java = |name: &str| -> serde_json::Value {
            let start = format!("static final String {name} = \"");
            let line = check.lines().map(str::trim).find(|line| line.starts_with(&start)).unwrap();
            let literal = line[start.len()..].strip_suffix("\";").unwrap();
            serde_json::from_str(&unescaped(literal)).unwrap()
        };
        let en = sioul_core::i18n::Translator::new("en");
        let words = serde_json::json!({ "day": en.text("review-channel", None) });
        let offer = |label: &str, kind: &'static str, key: &str| crate::reviews::Offered { label: en.text(label, None), kind, key: key.into() };
        let until = 1_792_000_000;
        let work = day_note_json("day:work:2026-10-14", &en.text("review-notice-title", None), &en.text("review-notice-text", None), &[offer("review-close-work", "review", "work:2026-10-14")], until, words.clone());
        assert_eq!(work, java("WORK"));
        let night = day_note_json("day:night:2026-10-14", "Wind down", "22:30", &[offer("review-close-night", "review", "night:2026-10-14"), offer("need-open", "need", "sleep")], until, words.clone());
        assert_eq!(night, java("NIGHT"));
        let given = day_note_json("day:night:2026-10-14", "Wind down", "22:30", &[offer("need-open", "need", "sleep")], until, words);
        assert_eq!(given, java("GIVEN"));
    }
}

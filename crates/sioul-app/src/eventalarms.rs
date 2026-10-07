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
unsafe extern "C" {
    /// The coming events' reminders handed to Java, which keeps them and sets their alarms (android/main.cpp).
    fn sioul_android_set_event_alarms(json: *const c_char);
    /// A reminder shown in Android's notifications, "Events" (android/main.cpp).
    fn sioul_android_event_note(json: *const c_char);
    /// New mail's notification, "New mail" (android/main.cpp).
    fn sioul_android_mail_note(json: *const c_char);
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

/// The list Java is given, from reminders gathered already: the events' own.
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
    let list: Vec<serde_json::Value> = due.iter().take(AT_MOST).map(|(at, r)| serde_json::json!({ "key": r.key, "kind": r.kind, "at": at * 1000, "title": r.title, "body": r.body })).collect();
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
    unsafe {
        sioul_android_mail_note(text.as_ptr())
    };
    let _ = text;
}

/// When a reminder held now may come: when the time now ends (waking, the
/// end of free time, the hours' end); while a layer holds it (a slot of time
/// for you, do-not-disturb) or the end is unknown (a pause), in a quarter of
/// an hour (Unix seconds).
fn again_at(holds: &Holds, stamp: i64) -> i64 {
    match holds.until {
        Some(until) if !holds.now.slot && !holds.now.dnd => until.max(stamp + 60),
        _ => stamp + AGAIN,
    }
}

/// At a reminder's time (`EventAlarms`): {"shown": true} once told here; or
/// {"again_at": ms}, held now (you sleep) and still worth telling then; or
/// {} (moved, cancelled, told already, too late). The coming list is given
/// again after, as the calendars are now.
fn decide(key: &str) -> String {
    let now = Zoned::now();
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_event_decide(key: *const c_char) -> *mut c_char {
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_event_coming(zone: *const c_char) -> *mut c_char {
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
/// its file) or ("porch", "").
pub(crate) fn opened() -> Option<(String, String)> {
    #[cfg(target_os = "android")]
    {
        let mut buffer = vec![0u8; 4096];
        let found = unsafe { sioul_android_take_reminder_opened(buffer.as_mut_ptr().cast(), buffer.len() as i32) };
        if found {
            let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
            let said: serde_json::Value = serde_json::from_slice(&buffer[..end]).ok()?;
            let kind = said["kind"].as_str().filter(|k| *k == "event" || *k == "porch")?.to_string();
            return Some((kind, said["key"].as_str().unwrap_or_default().to_string()));
        }
    }
    None
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
        // A reminder's alarm: its time; due and free, at once; held (a train after waking), at waking.
        let reminder = |at: i64, until: i64| Reminder { key: "before:train:1".into(), kind: Kind::Before, at, until, title: String::new(), body: String::new(), target: String::new(), work: false, starts: Some(until) };
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
}

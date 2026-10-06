// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The alarm at waking, on a phone (docs/health.md, "The alarm at waking";
//! docs/android.md, "Waking"): it rings when the night ends, on the mornings
//! ticked with the night in the Health page's settings, each night as that
//! day has it (`needs::Needs::alarm_of`). Android's alarm clock rings it
//! (android/package/src/com/aurelienpierre/sioul/WakeAlarms.java): Sioul
//! hands Java the coming wakings, eight days ahead, whenever they change, and
//! Java asks again here at each ring, a little before each and as each night
//! starts (another device may have changed it while Android kept Sioul
//! frozen), after a restart, a change of time or of zone. Elsewhere nothing
//! rings: the setting travels with the night, and the page says when the
//! phone will ring. "Try the alarm" rings it ten seconds on, the same way,
//! handed apart (`trial`): nothing written, the next waking left as it is.

use crate::backend::{say, tr};
use jiff::Zoned;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use sioul_core::health::Health;
use sioul_core::i18n::Translator;
use sioul_core::needs::{Days, Needs, WAKINGS_AHEAD_DAYS};
use std::ffi::{CString, c_char};
use std::path::Path;

#[cfg(target_os = "android")]
unsafe extern "C" {
    /// The coming wakings handed to Java, which keeps them and arms the next (android/main.cpp).
    fn sioul_android_set_wake(json: *const c_char);
    /// What Android allows the alarm: 1 exact alarms, 2 the full screen, 4 notifications (android/main.cpp).
    fn sioul_android_wake_state() -> i32;
    /// Android's own page for one of them: "exact", "screen", "notifications" (android/main.cpp).
    fn sioul_android_wake_settings(which: *const c_char);
    /// "Try the alarm": rung as a waking, the next waking left as it is (android/main.cpp):
    /// 0 it rings; 1 exact alarms refused, 2 notifications, 3 the full screen; -1 not asked.
    fn sioul_android_wake_try(json: *const c_char) -> i32;
}

/// The wakings last handed to Java, so that they are handed again only when they change.
static GIVEN: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Minutes "10 min later" puts the alarm off by (WakeAlarms.LATER_MINUTES).
const LATER_MINUTES: u32 = 10;
/// Java asks again this many minutes before each ring, and as its night starts.
const LOOK_BEFORE_MINUTES: i64 = 10;
/// A try rings this many seconds after it is asked for (WakeAlarms.TRY_SECONDS).
const TRY_SECONDS: i64 = 10;
/// What a desktop answers a try: it never rings.
const NOT_A_PHONE: i32 = -2;

/// The usual night and its mornings, from Health's file at `path`: none when
/// the file is there but does not read, so that a file broken (or being
/// written by hand) never cancels an alarm: Java then keeps what it has.
fn read_needs(path: &Path) -> Option<Needs> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str::<Health>(&text).ok().map(|health| health.needs),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(Needs::default()),
        Err(_) => None,
    }
}

/// The words Java says, in the person's language, given ahead with the
/// wakings: Sioul may not run when the alarm rings (before the phone's first
/// unlock, above all). `{time}` is the time Java puts in.
fn words(tr: &Translator) -> serde_json::Value {
    let mut later = sioul_core::i18n::args();
    later.set("minutes", LATER_MINUTES);
    let mut again = sioul_core::i18n::args();
    again.set("time", "{time}");
    serde_json::json!({
        "channel": tr.text("wake-channel", None),
        "title": tr.text("wake-ringing", None),
        "stop": tr.text("wake-stop", None),
        "later": tr.text("wake-later", Some(&later)),
        "again": tr.text("wake-again", Some(&again)),
    })
}

/// What Java is handed: `rings`, the coming alarms (Unix milliseconds, eight
/// days ahead); `looks`, when to ask again (each night's start, and ten
/// minutes before each ring); `week`, the usual mornings (Monday first, the
/// waking or "" when it does not ring), repeated by Java should Sioul not
/// answer; `zone`, the one the times were made in; `words`.
pub(crate) fn coming(needs: &Needs, days: &Days, now: &Zoned, tr: &Translator) -> serde_json::Value {
    let stamp = now.timestamp().as_second();
    let alarms = needs.wakings(now, days, WAKINGS_AHEAD_DAYS);
    let rings: Vec<i64> = alarms.iter().map(|a| a.at * 1000).collect();
    let mut looks: Vec<i64> = alarms.iter().flat_map(|a| [a.night_start, a.at - LOOK_BEFORE_MINUTES * 60]).filter(|at| *at > stamp).map(|at| at * 1000).collect();
    looks.sort_unstable();
    looks.dedup();
    let usual = if needs.sleep_on { needs.sleep.wake.as_str() } else { "" };
    let week: Vec<&str> = needs.sleep.alarm.iter().map(|on| if *on { usual } else { "" }).collect();
    serde_json::json!({
        "rings": rings,
        "looks": looks,
        "week": week,
        "zone": now.time_zone().iana_name().unwrap_or_default(),
        "words": words(tr),
    })
}

/// Java's answer when the night's file does not read: it keeps what it has.
fn unknown() -> String {
    serde_json::json!({ "unknown": true }).to_string()
}

/// The wakings handed to Java when they changed since they were last.
fn hand(json: String) {
    let Ok(mut given) = GIVEN.lock() else { return };
    if *given == json {
        return;
    }
    let Ok(text) = CString::new(json.clone()) else { return };
    #[cfg(target_os = "android")]
    unsafe {
        sioul_android_set_wake(text.as_ptr())
    };
    let _ = text;
    *given = json;
}

/// The coming wakings handed to Android when they changed: at each minute
/// of the window, when the night, a day's change or the mornings ticked are
/// saved (here, or brought by the sharing, read at the next minute), and as
/// Sioul is put away. Nothing on a desktop: it never rings.
pub(crate) fn schedule() {
    if !cfg!(target_os = "android") {
        return;
    }
    let Some(needs) = read_needs(&Health::default_path()) else { return };
    hand(coming(&needs, &crate::health::days(), &Zoned::now(), tr()).to_string());
}

/// Java's question (`WakeAlarms.nativeNext`): at a ring; before one and as
/// a night starts (`fetch`: the sync app asked first to bring what your other
/// devices changed, and given twenty seconds); after a restart, an update, a
/// change of time or of zone. What the other devices wrote is read first;
/// then the wakings as `coming` makes them, in `zone`, the phone's, as Java
/// has it (Sioul's own may lag minutes behind a change of zone). Blocks: never
/// on Android's main thread.
fn next(zone: &str, fetch: bool) -> String {
    let _ = crate::share::exchange_here(fetch);
    let zone = TimeZone::get(zone).unwrap_or_else(|_| TimeZone::system());
    let Some(needs) = read_needs(&Health::default_path()) else { return unknown() };
    let json = coming(&needs, &crate::health::days(), &Zoned::now().with_time_zone(zone), tr()).to_string();
    if let Ok(mut given) = GIVEN.lock() {
        *given = json.clone();
    }
    json
}

/// `next`, for Java (android/main.cpp); a panic is answered as not known, so
/// that Java keeps the wakings it has. Given back to `sioul_string_free`.
///
/// # Safety
/// `zone` is null, or a zero-terminated text valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_wake_next(zone: *const c_char, fetch: bool) -> *mut c_char {
    let zone = unsafe { crate::alarms::key_of(zone) };
    let answer = std::panic::catch_unwind(|| next(&zone, fetch)).unwrap_or_else(|_| unknown());
    crate::alarms::handed(answer)
}

/// What Android allows the alarm, as bits: 1 exact alarms, 2 the full
/// screen, 4 notifications. All of it elsewhere, where nothing rings.
fn allowed() -> i32 {
    #[cfg(target_os = "android")]
    return unsafe { sioul_android_wake_state() };
    #[cfg(not(target_os = "android"))]
    7
}

/// The option as the Health page's settings show it (`health::needs_page`):
/// the weekdays' names and short names, Monday first; the next ring; on a
/// phone, what Android refuses it.
pub(crate) fn status(needs: &Needs) -> serde_json::Value {
    let now = Zoned::now();
    let monday = jiff::civil::date(2026, 10, 5);
    let week: Vec<Date> = (0..7).filter_map(|i| monday.checked_add(jiff::Span::new().days(i)).ok()).collect();
    let next = needs
        .wakings(&now, &crate::health::days(), WAKINGS_AHEAD_DAYS)
        .first()
        .and_then(|a| jiff::Timestamp::from_second(a.at).ok())
        .map(|at| say("wake-next", &[("when", tr().date(&at.to_zoned(now.time_zone().clone()), false))]))
        .unwrap_or_default();
    let allowed = allowed();
    serde_json::json!({
        "phone": cfg!(target_os = "android"),
        "names": (1..=7).map(|d| tr().text(&format!("weekday-{d}"), None)).collect::<Vec<_>>(),
        "short": week.iter().map(|d| tr().weekday_short(*d)).collect::<Vec<_>>(),
        "next": next,
        "exact": allowed & 1 != 0,
        "screen": allowed & 2 != 0,
        "notifications": allowed & 4 != 0,
    })
}

/// Android's own page where the alarm is allowed: "exact" (Alarms &
/// reminders), "screen" (full-screen notifications, Android 14 and later),
/// "notifications". Nothing elsewhere.
pub(crate) fn open_settings(which: &str) {
    let Ok(text) = CString::new(which) else { return };
    #[cfg(target_os = "android")]
    unsafe {
        sioul_android_wake_settings(text.as_ptr())
    };
    let _ = text;
}

/// What a try hands Java (`WakeAlarms.tryNow`): {at, words}, the time it
/// rings (Unix milliseconds, ten seconds on) and the words it says. Nothing
/// of the night, its list or its next ring: a try is handed apart from them
/// (`sioul_android_wake_try`, not `sioul_android_set_wake`) and rung by its
/// own action and code, so the next waking stays as it is.
pub(crate) fn trial(now: &Zoned, tr: &Translator) -> serde_json::Value {
    serde_json::json!({ "at": now.timestamp().as_millisecond() + TRY_SECONDS * 1000, "words": words(tr) })
}

/// A try handed to Java: its answer (`sioul_android_wake_try`); a desktop never rings.
fn hand_try(json: &str) -> i32 {
    let Ok(text) = CString::new(json) else { return -1 };
    #[cfg(target_os = "android")]
    return unsafe { sioul_android_wake_try(text.as_ptr()) };
    #[cfg(not(target_os = "android"))]
    {
        let _ = text;
        NOT_A_PHONE
    }
}

/// Java's answer to a try, said: {rings, line, fix, button}: it rings in ten
/// seconds; or what Android refuses (exact alarms, notifications, the full
/// screen), with its page ("exact", "notifications", "screen") and the
/// button's words, rather than a try that rings unseen or not at all.
fn tried(answer: i32, tr: &Translator) -> serde_json::Value {
    let (rings, line, fix) = match answer {
        0 => (true, "wake-try-rings", ""),
        1 => (false, "wake-exact-off", "exact"),
        2 => (false, "wake-notifications-off", "notifications"),
        3 => (false, "wake-try-screen-off", "screen"),
        NOT_A_PHONE => (false, "wake-try-phone", ""),
        _ => (false, "wake-try-failed", ""),
    };
    let button = match fix {
        "exact" => tr.text("wake-allow-exact", None),
        "notifications" => tr.text("wake-allow-notifications", None),
        "screen" => tr.text("wake-allow-screen", None),
        _ => String::new(),
    };
    serde_json::json!({ "rings": rings, "line": tr.text(line, None), "fix": fix, "button": button })
}

/// "Try the alarm" (the Health page's settings, on a phone): the alarm rung
/// ten seconds on, through the path a waking takes, over the lock screen.
/// Nothing written, the next waking left as it is. JSON, as `tried` says it.
pub(crate) fn try_now() -> String {
    tried(hand_try(&trial(&Zoned::now(), tr()).to_string()), tr()).to_string()
}

/// The alarm at the end of `night`'s night, for its row on the page: when
/// it rings ("07:00"), and whether "No alarm" was asked; ("", false) when
/// none would ring (no morning ticked, the night taken out).
pub(crate) fn of_night(needs: &Needs, night: Date, zone: &TimeZone, days: &Days) -> (String, bool) {
    needs.alarm_of(night, zone, days).and_then(|a| jiff::Timestamp::from_second(a.at).ok().map(|at| (at.to_zoned(zone.clone()).strftime("%H:%M").to_string(), a.skipped))).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::needs::Sleep;

    fn ticked(alarm: [bool; 7]) -> Needs {
        Needs { sleep_on: true, sleep: Sleep { alarm, ..Sleep::default() }, ..Needs::default() }
    }

    fn at(text: &str) -> i64 {
        text.parse::<Zoned>().unwrap().timestamp().as_millisecond()
    }

    #[test]
    fn what_java_is_handed() {
        // Sunday 4 October 2026, 22:30 in Paris, the night begun at 22:00; Monday to Friday mornings.
        let now: Zoned = "2026-10-04T22:30[Europe/Paris]".parse().unwrap();
        let needs = ticked([true, true, true, true, true, false, false]);
        let english = Translator::new("en");
        let handed = coming(&needs, &Days::default(), &now, &english);
        let rings: Vec<i64> = serde_json::from_value(handed["rings"].clone()).unwrap();
        assert_eq!(rings.len(), 6, "Monday to Friday, and Monday 12, eight days on: {handed}");
        assert_eq!(rings[0], at("2026-10-05T07:00[Europe/Paris]"));
        assert_eq!(rings[4], at("2026-10-09T07:00[Europe/Paris]"));
        assert_eq!(rings[5], at("2026-10-12T07:00[Europe/Paris]"));
        // Asked again ten minutes before each ring, and as each night starts (tonight's has).
        let looks: Vec<i64> = serde_json::from_value(handed["looks"].clone()).unwrap();
        assert_eq!(looks[0], at("2026-10-05T06:50[Europe/Paris]"));
        assert_eq!(looks[1], at("2026-10-05T22:00[Europe/Paris]"));
        assert_eq!(looks.len(), 11, "{looks:?}");
        assert!(looks.windows(2).all(|w| w[0] < w[1]));
        // The usual week, for when Sioul cannot answer; the zone; the words.
        assert_eq!(handed["week"], serde_json::json!(["07:00", "07:00", "07:00", "07:00", "07:00", "", ""]));
        assert_eq!(handed["zone"], "Europe/Paris");
        assert_eq!(handed["words"]["stop"], "Stop");
        assert_eq!(handed["words"]["later"], "10 min later");
        assert_eq!(handed["words"]["again"], "Rings again at {time}");
        assert_eq!(handed["words"]["channel"], "Waking");
        // In French, with its typography.
        let french = coming(&needs, &Days::default(), &now, &Translator::new("fr"));
        assert_eq!(french["words"]["stop"], "Arrêter");
        assert_eq!(french["words"]["later"], "10 min plus tard");
        assert_eq!(french["words"]["again"], "Sonne de nouveau à {time}");
        // No night set, or none ticked: nothing rings, nothing to repeat.
        for off in [Needs { sleep_on: false, ..needs.clone() }, ticked([false; 7])] {
            let handed = coming(&off, &Days::default(), &now, &english);
            assert_eq!(handed["rings"], serde_json::json!([]));
            assert_eq!(handed["looks"], serde_json::json!([]));
            assert!(handed["week"].as_array().unwrap().iter().all(|w| w == ""));
        }
        // A day's own waking, and a night with no alarm, follow.
        let mut days = Days::default();
        days.change(jiff::civil::date(2026, 10, 5), "sleep", |b| b.wake = "08:30".into());
        days.change(jiff::civil::date(2026, 10, 6), "sleep", |b| b.no_alarm = true);
        let rings: Vec<i64> = serde_json::from_value(coming(&needs, &days, &now, &english)["rings"].clone()).unwrap();
        assert_eq!(rings[..3], [at("2026-10-05T07:00[Europe/Paris]"), at("2026-10-06T08:30[Europe/Paris]"), at("2026-10-08T07:00[Europe/Paris]")]);
    }

    #[test]
    fn a_file_that_does_not_read_cancels_nothing() {
        let dir = std::env::temp_dir().join(format!("sioul-wake-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        // None yet: no night, nothing rings.
        assert_eq!(read_needs(&path), Some(Needs::default()));
        std::fs::write(&path, "[needs]\nsleep_on = true\n[needs.sleep]\nbed = \"23:00\"\nwake = \"06:45\"\nalarm = [true, false, false, false, false, false, true]\n").unwrap();
        let needs = read_needs(&path).unwrap();
        assert_eq!((needs.sleep.wake.as_str(), needs.sleep.alarm), ("06:45", [true, false, false, false, false, false, true]));
        // Broken (by hand, or half there): not known, and Java keeps its wakings.
        std::fs::write(&path, "[needs\nsleep_on = tru").unwrap();
        assert_eq!(read_needs(&path), None);
        assert_eq!(unknown(), r#"{"unknown":true}"#);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_try_hands_java_its_time_and_words_only() {
        let now: Zoned = "2026-10-06T21:15:30[Europe/Paris]".parse().unwrap();
        let english = Translator::new("en");
        let handed = trial(&now, &english);
        // Ten seconds on, in Unix milliseconds; the words a waking says, in the person's language.
        assert_eq!(handed["at"], now.timestamp().as_millisecond() + 10_000);
        assert_eq!(handed["at"], "2026-10-06T21:15:40[Europe/Paris]".parse::<Zoned>().unwrap().timestamp().as_millisecond());
        assert_eq!(handed["words"], words(&english));
        assert_eq!(trial(&now, &Translator::new("fr"))["words"]["stop"], "Arrêter");
        // Nothing of the night: no rings, looks, week or zone that could stand for the wakings' list.
        let keys: Vec<&String> = handed.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["at", "words"]);
    }

    #[test]
    fn a_try_leaves_the_next_waking_as_it_is() {
        // The wakings handed (Monday to Friday at 07:00), then a try: the next ring, the list and
        // what was handed are the same after it; a desktop rings nothing and says so.
        let now: Zoned = "2026-10-06T21:15[Europe/Paris]".parse().unwrap();
        let needs = ticked([true, true, true, true, true, false, false]);
        let english = Translator::new("en");
        let before = coming(&needs, &Days::default(), &now, &english);
        let given = before.to_string();
        *GIVEN.lock().unwrap() = given.clone();
        let answer = hand_try(&trial(&now, &english).to_string());
        assert_eq!(answer, NOT_A_PHONE);
        assert_eq!(*GIVEN.lock().unwrap(), given, "the wakings handed, untouched");
        assert_eq!(coming(&needs, &Days::default(), &now, &english), before);
        assert_eq!(before["rings"][0], "2026-10-07T07:00[Europe/Paris]".parse::<Zoned>().unwrap().timestamp().as_millisecond());
        assert_eq!(tried(answer, &english)["line"], "The alarm rings on a phone only.");
    }

    #[test]
    fn a_try_refused_says_why() {
        let english = Translator::new("en");
        let said = |answer: i32| tried(answer, &english);
        assert_eq!(said(0)["rings"], true);
        assert_eq!(said(0)["line"], "Rings in about ten seconds, as a waking would; nothing else changes.");
        assert_eq!(said(0)["fix"], "");
        // Refused: never rings unseen, says why, and which page of Android's allows it.
        for (answer, fix, button) in [(1, "exact", "Allow alarms…"), (2, "notifications", "Turn them on…"), (3, "screen", "Allow the full screen…")] {
            let said = said(answer);
            assert_eq!((said["rings"].as_bool(), said["fix"].as_str(), said["button"].as_str()), (Some(false), Some(fix), Some(button)), "{said}");
            assert!(!said["line"].as_str().unwrap().is_empty());
        }
        assert_eq!(said(1)["line"], english.text("wake-exact-off", None));
        assert_eq!(said(-1)["line"], "Android did not answer: nothing was set. Try again in a moment.");
        assert_eq!(tried(3, &Translator::new("fr"))["button"], "Autoriser le plein écran…");
    }

    #[test]
    fn the_night_s_row_says_its_alarm() {
        let needs = ticked([true, true, true, true, true, false, false]);
        let zone = TimeZone::get("Europe/Paris").unwrap();
        let mut days = Days::default();
        // Monday's night ends Tuesday at 07:00; Friday's, a Saturday: none.
        assert_eq!(of_night(&needs, jiff::civil::date(2026, 10, 5), &zone, &days), ("07:00".to_string(), false));
        assert_eq!(of_night(&needs, jiff::civil::date(2026, 10, 9), &zone, &days), (String::new(), false));
        days.change(jiff::civil::date(2026, 10, 5), "sleep", |b| b.no_alarm = true);
        assert_eq!(of_night(&needs, jiff::civil::date(2026, 10, 5), &zone, &days), ("07:00".to_string(), true));
    }
}

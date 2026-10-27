// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Doses on a phone, reminded while Sioul is not on the screen (docs/health.md,
//! "On a phone"): Android freezes an app in the background, so each coming
//! dose is given to Android's alarm clock (android/main.cpp); at its time a
//! receiver asks here what to say (`health::alarm_decide`), after reading what
//! your other devices marked: taken, nothing; not known, the doubt said first.
//! Elsewhere, nothing here: the window's minute reminds.

use std::ffi::{CStr, CString, c_char};

#[cfg(target_os = "android")]
// SAFETY: declared as android/main.cpp defines them: extern "C", the same types.
unsafe extern "C" {
    /// Android's alarm clock given the coming doses, replacing those given before (android/main.cpp).
    fn sioul_android_set_alarms(json: *const c_char);
    /// A tapped reminder's dose, once; false when there is none (android/main.cpp).
    fn sioul_android_take_opened(key: *mut c_char, size: i32) -> bool;
    /// Whether Android lets Sioul set exact alarms (android/main.cpp).
    fn sioul_android_exact_alarms() -> bool;
    /// Whether Android shows Sioul's notifications (and its "Doses" channel is on).
    fn sioul_android_notifications_allowed() -> bool;
    /// The reminder shown for a dose taken away (android/main.cpp).
    fn sioul_android_remove_reminder(key: *const c_char);
}

/// Doses marked whose reminder was taken away already, in this process,
/// each with when it had been reminded then (`HealthState::reminded`, 0
/// none): reminded again since (taken back, then reminded anew), it is taken
/// away again once marked, whichever process took the first away (the A3
/// dose model: a dose taken, taken back, reminded and taken again kept its
/// reminder on the phone).
static REMOVED: std::sync::Mutex<std::collections::BTreeMap<String, i64>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The reminder shown for a dose taken away: once it is marked, here or on
/// another device, a reminder left on the phone would say it is still to take.
pub(crate) fn remove_reminder(key: &str) {
    remove_reminder_of(key, crate::health::reminded_at(key));
}

fn remove_reminder_of(key: &str, reminded: i64) {
    let Ok(text) = CString::new(key) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_remove_reminder(text.as_ptr())
    };
    let _ = text;
    if let Ok(mut removed) = REMOVED.lock() {
        removed.insert(key.to_string(), reminded);
    }
}

/// The phone's time zone as Android last said it (`sioul_dose_zone`), kept
/// for both of its processes, the window's and the `:steps` one
/// (`$XDG_STATE_HOME/sioul/zone.toml`): the system's zone as each process
/// reads it is kept five minutes (jiff's own), and a hand made meanwhile
/// would key set-time doses in the old zone, in the process told (the A4
/// check, #2) or in the other (the A5 check, L2). With when it last changed,
/// and from which: a dose keyed in the earlier zone is never taken for one
/// moved (M3).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ZoneSaid {
    /// The zone Android said last ("Asia/Tokyo").
    #[serde(default)]
    pub zone: String,
    /// The zone before its last change.
    #[serde(default)]
    pub was: String,
    /// When it last changed (Unix seconds; 0: never seen to change).
    #[serde(default)]
    pub changed: i64,
    /// When Android last said it (Unix seconds).
    #[serde(default)]
    pub said: i64,
}

/// The zone's file, beside the doses' record.
fn zone_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("zone.toml")
}

/// The zone as Android last said it; none said yet, or its file not readable: nothing.
pub(crate) fn zone_said() -> ZoneSaid {
    #[cfg(test)]
    if let Some(said) = ZONE_SAID.with(|said| said.borrow().clone()) {
        return said;
    }
    read_zone(&zone_path())
}

fn read_zone(path: &std::path::Path) -> ZoneSaid {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// Android says its zone, at a change of zone or of the clock: kept, with
/// when it changed and from which when it is another than the one kept (none
/// kept yet: the one this process reads, the earlier while its cache lasts);
/// written whole, its bytes on the disk before its name.
fn note_zone(path: &std::path::Path, zone: &str, now: i64) -> ZoneSaid {
    let mut said = read_zone(path);
    let before = if said.zone.is_empty() { jiff::tz::TimeZone::system().iana_name().unwrap_or_default().to_string() } else { said.zone.clone() };
    if before != zone {
        said.was = before;
        said.changed = now;
    }
    said.zone = zone.to_string();
    said.said = now;
    if let Ok(text) = toml::to_string(&said) {
        let _ = sioul_core::health::write_synced(path, format!("# The phone's time zone as Android last said it (docs/android.md).\n{text}").as_bytes());
    }
    said
}

#[cfg(test)]
thread_local! {
    /// The zone as Android said it, in a test's thread that says one: never
    /// the file, which other tests read beside it.
    pub(crate) static ZONE_SAID: std::cell::RefCell<Option<ZoneSaid>> = const { std::cell::RefCell::new(None) };
}

/// Now, in the zone in force: Android's own for six minutes after it said it, else the system's.
pub(crate) fn now_here() -> jiff::Zoned {
    now_in(&zone_said(), jiff::Zoned::now())
}

/// `now` in the zone Android said, for six minutes after it said it (the
/// system's zone as Sioul reads it is kept five), else as it is; a zone
/// that does not read changes nothing.
fn now_in(said: &ZoneSaid, now: jiff::Zoned) -> jiff::Zoned {
    let fresh = (0..6 * 60).contains(&(now.timestamp().as_second() - said.said));
    match fresh.then(|| jiff::tz::TimeZone::get(&said.zone).ok()).flatten() {
        Some(zone) => now.with_time_zone(zone),
        None => now,
    }
}

/// The phone's zone changed, or its clock was set (`WakeReceiver`, at
/// TIMEZONE_CHANGED, TIME_SET and the first unlock): the zone kept for both
/// processes (`ZoneSaid`), and the doses handed at once, their keys made in
/// it, `zone` as Java has it (the A4 check, #2: until then only an alarm
/// keyed in the old zone handed them, when it rang).
///
/// # Safety
/// `zone` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_dose_zone(zone: *const c_char) {
    // SAFETY: `zone` is null or a C string valid for the call, as this function's contract says.
    let zone = unsafe { key_of(zone) };
    if !zone.is_empty() {
        let _ = std::panic::catch_unwind(|| note_zone(&zone_path(), &zone, jiff::Timestamp::now().as_second()));
    }
    let _ = std::panic::catch_unwind(schedule);
}

/// A reminder about to be shown again (`health::remind_shown`): never one
/// this process took away, whatever it took away before (the A4 check, #3).
pub(crate) fn forget_removed(key: &str) {
    if let Ok(mut removed) = REMOVED.lock() {
        removed.remove(key);
    }
}

/// Whether reminders can show at all: on a phone, only when Android lets
/// Sioul's notifications through; elsewhere, always.
pub(crate) fn notifications_allowed() -> bool {
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's, without arguments; it may be called from any thread.
    return unsafe { sioul_android_notifications_allowed() };
    #[cfg(not(target_os = "android"))]
    true
}

/// Whether reminders come at their minute: on a phone, only when Android lets
/// Sioul set exact alarms; elsewhere, always.
pub(crate) fn exact() -> bool {
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's, without arguments; it may be called from any thread.
    return unsafe { sioul_android_exact_alarms() };
    #[cfg(not(target_os = "android"))]
    true
}

/// Whether an exchange wrote the doses' files here (the medicines, the
/// marks, the moves taken back): what Android is to ring may have changed.
pub(crate) fn health_written(outcome: &sioul_sync::share::Outcome) -> bool {
    ["data/health.toml", "state/health-state.toml", "state/health-back.toml"].iter().any(|file| outcome.written.contains(*file))
}

/// The coming doses given to Android's alarm clock: at each minute of the
/// window, when it is put away, after a dose is marked or Health's settings
/// saved, after every exchange that wrote the doses' files, wherever it runs
/// (`share::exchanged`, `health_written`: the window's, its close's, the
/// background step's, a dose's alarm's, the waking's, a reminder's
/// button's), and when a dose's alarm finds its dose gone or marked
/// (`health::alarm_decide`);
/// the alarm at waking with them (`wake`), which keeps its own list, codes
/// and receiver. Handed each time: what Android holds is judged on Java's
/// side against the one record every process of Sioul reads fresh under one
/// lock (`DoseAlarms.Plan`), never against what this process last handed,
/// which another process may have changed since (the check of 1b6795e, H1);
/// a hand arms again only what changed, and at a process's first hand what
/// is due within the half hour too, never an alarm that rang
/// (`DoseAlarms.Plan`, `RANG`); every hand ends the quarter-hour look of
/// every dose reminded already, and takes away here only the reminders of
/// doses gone or marked: those gone noted so (`health::note_taken_away`),
/// to be armed and rung again if their dose comes back, and the dose that
/// replaced one, already past, rung once at once (`REPLACES`). A change of
/// zone or of the clock hands them at once (`sioul_dose_zone`).
pub(crate) fn schedule() {
    if !cfg!(target_os = "android") {
        return;
    }
    crate::wake::schedule();
    // The reminders of doses marked since (here, or come from another device) taken away.
    let gone: Vec<(String, i64)> = crate::health::marked_lately().into_iter().filter(|(key, reminded)| REMOVED.lock().is_ok_and(|removed| removed.get(key) != Some(reminded))).collect();
    for (key, reminded) in gone {
        remove_reminder_of(&key, reminded);
    }
    // The reminders of doses the schedule no longer holds (moved away, their
    // medicine changed, taken out or paused) taken away too: every hand
    // cancels the quarter-hour look at a reminder shown (`DoseAlarms.Plan`),
    // which took them away before (the chain check of e3505f7, must-fix 1);
    // nothing when nothing reads.
    // Noted as taken away, so that each comes back with its dose, and the
    // dose that replaced it rings once even past the half hour (the A4
    // check, #1 and #8).
    let taken_away = crate::health::reminded_gone().unwrap_or_default();
    for (key, reminded) in &taken_away {
        if REMOVED.lock().is_ok_and(|removed| removed.get(key) != Some(reminded)) {
            remove_reminder_of(key, *reminded);
        }
    }
    crate::health::note_taken_away(&taken_away);
    // Nothing reads here, the health file nor its last read: Android keeps the alarms it has.
    let Some(coming) = crate::health::alarms_coming() else { return };
    let Ok(text) = CString::new(coming) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_set_alarms(text.as_ptr())
    };
    let _ = text;
}

/// The dose of a reminder tapped while Sioul was away, once.
pub(crate) fn opened() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        let mut buffer = vec![0u8; 512];
        // SAFETY: android/main.cpp's; it writes at most `buffer.len()` bytes into `buffer`, its zero included.
        let found = unsafe { sioul_android_take_opened(buffer.as_mut_ptr().cast(), buffer.len() as i32) };
        if found {
            let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
            return String::from_utf8(buffer[..end].to_vec()).ok().filter(|k| !k.is_empty());
        }
    }
    None
}

/// A text handed to Android's side, freed by `sioul_string_free`.
pub(crate) fn handed(text: String) -> *mut c_char {
    CString::new(text).unwrap_or_default().into_raw()
}

/// A key from Android's side; "" when unreadable.
///
/// # Safety
/// `key` is null, or a zero-terminated text valid for the call.
pub(crate) unsafe fn key_of(key: *const c_char) -> String {
    if key.is_null() {
        return String::new();
    }
    // SAFETY: not null (above), and a C string valid for the call, as this function's contract says.
    unsafe { CStr::from_ptr(key) }.to_string_lossy().to_string()
}

/// At a dose's time: what to say (`health::alarm_decide`). Blocks up to half
/// a minute (the sync app given time to bring the others' marks): never on
/// Android's main thread.
///
/// # Safety
/// `key` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_alarm_decide(key: *const c_char) -> *mut c_char {
    // SAFETY: `key` is null or a C string valid for the call, as this function's contract says.
    let key = unsafe { key_of(key) };
    let answer = std::panic::catch_unwind(|| crate::health::alarm_decide(&key)).unwrap_or_else(|_| {
        // Something broke: never silence, never a claim. A dose is due, to be checked in Sioul.
        let body = std::panic::catch_unwind(|| crate::backend::tr().text("dose-alarm-fallback", None)).unwrap_or_else(|_| "A dose is due: open Sioul to check it.".into());
        serde_json::json!({ "show": true, "title": "Sioul", "body": body, "again_at": 0 }).to_string()
    });
    // The home screen's card says the doses as they are now (homecard.rs).
    let _ = std::panic::catch_unwind(crate::homecard::doses_changed);
    handed(answer)
}

/// "Taken" on a phone's reminder (`health::alarm_taken`).
///
/// # Safety
/// `key` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_alarm_taken(key: *const c_char) -> *mut c_char {
    // SAFETY: `key` is null or a C string valid for the call, as this function's contract says.
    let key = unsafe { key_of(key) };
    let answer = std::panic::catch_unwind(|| crate::health::alarm_taken(&key)).unwrap_or_else(|_| r#"{"done":false,"open":true,"line":""}"#.to_string());
    // The home screen's card says the doses as they are now (homecard.rs).
    let _ = std::panic::catch_unwind(crate::homecard::doses_changed);
    handed(answer)
}

/// A text handed by `sioul_alarm_decide`, `sioul_alarm_taken`,
/// `sioul_time_action` (timenote.rs) or `sioul_wake_next` (wake.rs), given back.
///
/// # Safety
/// `text` is null, or came from one of them and is given back once.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_string_free(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: `text` was made by `handed` (CString::into_raw) and is given back once, as the contract says.
        drop(unsafe { CString::from_raw(text) });
    }
}

#[cfg(test)]
mod tests {
    use super::{ZoneSaid, note_zone, now_in, read_zone};

    /// The A4 check, #2: just after a change of zone, the doses are keyed in
    /// the zone Android says, for six minutes; then, or with a zone that does
    /// not read, in the system's.
    #[test]
    fn the_zone_android_says_keys_the_doses_for_six_minutes() {
        let now = jiff::Zoned::now();
        let stamp = now.timestamp().as_second();
        let far = if now.time_zone().iana_name() == Some("Pacific/Auckland") { "America/Havana" } else { "Pacific/Auckland" };
        let said = |zone: &str, ago: i64| ZoneSaid { zone: zone.to_string(), said: stamp - ago, ..ZoneSaid::default() };
        let keyed = now_in(&said(far, 0), now.clone());
        assert_eq!(keyed.time_zone().iana_name(), Some(far));
        assert_eq!(keyed.timestamp(), now.timestamp(), "the same instant, in that zone");
        assert_eq!(now_in(&said(far, 5 * 60), now.clone()).time_zone().iana_name(), Some(far), "still within six minutes");
        assert_eq!(now_in(&said(far, 7 * 60), now.clone()).time_zone(), now.time_zone(), "past six minutes: the system's");
        assert_eq!(now_in(&said("Nowhere/Else", 0), now.clone()).time_zone(), now.time_zone(), "a zone that does not read");
        assert_eq!(now_in(&ZoneSaid::default(), now.clone()).time_zone(), now.time_zone());
    }

    /// The A5 check, L2 and M3: the zone Android says is kept in one file,
    /// which the other process reads as it is (its own cache of the system's
    /// zone never in the way), with when it changed and from which; the same
    /// zone said again (the clock set) changes nothing but when it was said.
    #[test]
    fn the_zone_is_kept_for_both_processes_with_when_it_changed() {
        sioul_core::config::stand_ins_only();
        let path = sioul_core::config::state_dir().join(format!("zone-test-{}.toml", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let first = note_zone(&path, "Europe/Paris", 1_000);
        assert_eq!((first.zone.as_str(), first.said), ("Europe/Paris", 1_000));
        let flown = note_zone(&path, "Asia/Tokyo", 2_000);
        assert_eq!(flown, ZoneSaid { zone: "Asia/Tokyo".into(), was: "Europe/Paris".into(), changed: 2_000, said: 2_000 });
        // Read by the other process: the same, from the file alone.
        assert_eq!(read_zone(&path), flown);
        assert_eq!(now_in(&read_zone(&path), jiff::Timestamp::from_second(2_060).unwrap().to_zoned(jiff::tz::TimeZone::UTC)).time_zone().iana_name(), Some("Asia/Tokyo"));
        // The clock set: the zone said again, no change.
        let set = note_zone(&path, "Asia/Tokyo", 3_000);
        assert_eq!((set.was.as_str(), set.changed, set.said), ("Europe/Paris", 2_000, 3_000));
        let _ = std::fs::remove_file(&path);
    }
}

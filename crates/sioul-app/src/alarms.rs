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
/// saved, and after every exchange without the window that wrote the doses'
/// files (`share::exchange_here`, `health_written`: the background step's, a
/// dose's alarm's, the waking's, a reminder's button's);
/// the alarm at waking with them (`wake`), which keeps its own list, codes
/// and receiver. Handed each time: what Android holds is judged on Java's
/// side against the one record every process of Sioul reads fresh under one
/// lock (`DoseAlarms.Plan`), never against what this process last handed,
/// which another process may have changed since (the check of 1b6795e, H1);
/// nothing changes there when nothing changed.
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

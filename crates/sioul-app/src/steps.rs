// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The phone in the background: "Sioul keeps your devices in step"
//! (docs/android.md, "In the background"; StepService.java). A foreground
//! service in a process of its own (":steps"), which holds Sioul's library
//! without Qt's window, asks Rust for a step at each of its alarms and when
//! the sync app writes another device's file into the sharing folder:
//!
//! 1. the other devices' news: the sync app asked to look first at an alarm,
//!    then a quick exchange (`share::exchange_here`, which also keeps this
//!    device's entry among the devices up to date; the service never says the
//!    phone is in use);
//! 2. do-not-disturb: what this device should ask of its system, compared
//!    with what was asked last; changed, Sioul's own process is asked to apply
//!    it (`DndReceiver`), where Android's modes of Sioul's are kept;
//! 3. mail at its rhythm, the inbox only, handed to the new-mail
//!    notifications (`mailnote`);
//! 4. the calls' table (`calls::step`): made again when what it is made of
//!    changed, and the notification's "Let every call through";
//! 5. when to look next: two minutes while another device is in use, five
//!    otherwise, fifteen while you sleep.
//!
//! On a computer nothing here runs: the window does all of it each minute.

use crate::backend::{load_config, tr};
use sioul_core::config::Config;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

/// In the background service's own process: set by Java before each step.
static IN_SERVICE: AtomicBool = AtomicBool::new(false);
/// Mail fetched in the background this often (seconds).
pub(crate) const MAIL_EVERY: i64 = 15 * 60;
/// Steps while another device is in use, while awake otherwise, while asleep (seconds).
pub(crate) const STEP_IN_USE: i64 = 2 * 60;
pub(crate) const STEP_AWAKE: i64 = 5 * 60;
pub(crate) const STEP_ASLEEP: i64 = 15 * 60;

/// Whether this process is the background service's.
pub(crate) fn in_service() -> bool {
    IN_SERVICE.load(Ordering::Relaxed)
}

#[cfg(target_os = "android")]
unsafe extern "C" {
    /// StepService.call (android/main.cpp): a verb and its JSON, a JSON answer or null.
    fn sioul_android_steps(verb: *const std::ffi::c_char, json: *const std::ffi::c_char) -> *mut std::ffi::c_char;
    /// An answer of `sioul_android_steps` given back.
    fn sioul_android_dnd_free(text: *mut std::ffi::c_char);
    /// Android's question for reading contacts, with the window only.
    fn sioul_android_ask_contacts();
}

/// Java's answer to `verb` (StepService.call); null elsewhere, or without one.
pub(crate) fn java(verb: &str, json: &str) -> serde_json::Value {
    #[cfg(target_os = "android")]
    {
        use std::ffi::{CStr, CString};
        let (Ok(verb), Ok(json)) = (CString::new(verb), CString::new(json)) else { return serde_json::Value::Null };
        // SAFETY: two zero-terminated texts, valid for the call.
        let answer = unsafe { sioul_android_steps(verb.as_ptr(), json.as_ptr()) };
        if answer.is_null() {
            return serde_json::Value::Null;
        }
        // SAFETY: a zero-terminated text from sioul_android_steps, read before it is given back.
        let text = unsafe { CStr::from_ptr(answer) }.to_string_lossy().to_string();
        // SAFETY: given back once, as main.cpp asks; not read after.
        unsafe { sioul_android_dnd_free(answer) };
        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (verb, json);
        serde_json::Value::Null
    }
}

/// Android's question for reading contacts (the window's). Nothing elsewhere.
pub(crate) fn ask_contacts() {
    #[cfg(target_os = "android")]
    // SAFETY: no arguments; main.cpp asks Qt, with the window only.
    unsafe {
        sioul_android_ask_contacts()
    };
}

/// On a phone, in Sioul's own process: Android's alarm back at `at` (Unix
/// seconds) to apply do-not-disturb again then (DndReceiver), Sioul closed
/// or not; none takes it away. Nothing elsewhere: the window's minute does it.
pub(crate) fn next(at: Option<i64>) {
    if !cfg!(target_os = "android") || in_service() {
        return;
    }
    static GIVEN: AtomicI64 = AtomicI64::new(-1);
    let millis = at.map_or(0, |a| a.saturating_mul(1000));
    if GIVEN.swap(millis, Ordering::Relaxed) != millis {
        java("next", &serde_json::json!({ "at": millis }).to_string());
    }
    follow_setting_once();
}

/// The service's words, in the person's language.
fn words() -> serde_json::Value {
    serde_json::json!({ "title": tr().text("dnd-steps-note", None), "channel": tr().text("dnd-steps-channel", None) })
}

/// What was last asked of the service here (on), and when (Unix seconds).
static FOLLOWED: Mutex<Option<(bool, i64)>> = Mutex::new(None);

/// The service started or stopped as this device's setting says, asked again
/// every ten minutes while it should run: Android lets an app start its
/// service only while its window is on the screen (or at a restart), and
/// may have stopped it meanwhile. Java does nothing when it runs already.
fn follow_setting_once() {
    follow(false);
}

/// The service started (with its words) or stopped as this device's setting says, now.
pub(crate) fn follow_setting() {
    follow(true);
}

fn follow(now: bool) {
    if !cfg!(target_os = "android") || in_service() {
        return;
    }
    let config = load_config();
    let on = config.dnd.background && crate::share::vault().is_some_and(|(_, vault)| vault.is_some());
    let at = jiff::Timestamp::now().as_second();
    {
        let mut last = FOLLOWED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if !now && last.is_some_and(|(was, when)| was == on && (!on || at - when < 600)) {
            return;
        }
        *last = Some((on, at));
    }
    java(if on { "start" } else { "stop" }, &words().to_string());
}

/// The setting turned on or off here: written (this device's own, never
/// shared), then the service follows.
pub(crate) fn set_background(on: bool) -> Result<(), String> {
    let config = load_config();
    sioul_core::settings::apply(&crate::backend::config_path(), &config, "dnd.background", &sioul_core::config::SettingValue::Bool(on))?;
    follow_setting();
    Ok(())
}

/// Settings ▸ Do not disturb, on a phone: {on, running, sharing, battery, line, battery_line}.
pub(crate) fn setup(config: &Config) -> serde_json::Value {
    let sharing = crate::share::vault().is_some_and(|(_, vault)| vault.is_some());
    let running = java("running", "{}") == true;
    let battery = java("battery", "{}") == true;
    serde_json::json!({
        "on": config.dnd.background,
        "running": running,
        "sharing": sharing,
        "battery": battery,
        "line": tr().text(if config.dnd.background && running { "dnd-steps-on" } else { "dnd-steps-off" }, None),
        "battery_line": tr().text(if battery { "dnd-steps-battery-ok" } else { "dnd-steps-battery" }, None),
    })
}

// ---------------------------------------------------------------- a step

/// What was last asked of Sioul's own process (`everywhere::wanted`); empty: nothing yet in this process.
static POKED: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));
/// When mail was last fetched in the background (Unix seconds).
static MAILED: AtomicI64 = AtomicI64::new(0);

/// How long until the next step (seconds): two minutes while another device
/// is in use (its news stays fresh, the doses' too), fifteen while you sleep,
/// five otherwise.
fn next_step(in_use: bool, asleep: bool) -> i64 {
    if in_use {
        STEP_IN_USE
    } else if asleep {
        STEP_ASLEEP
    } else {
        STEP_AWAKE
    }
}

/// Whether mail is fetched at this step: at its rhythm, not while you sleep
/// or pause unless the notification matrix lets new mail come then (as
/// usual, nothing is told then; it comes when Sioul opens or after), and
/// only when new mail is notified at all.
fn mail_due(config: &Config, now: i64, last: i64, asleep: bool, paused: bool) -> bool {
    use sioul_core::notify::{Cell, Column, Kind, Notify};
    let notify = Notify::of(config);
    let held = |column: Column| matches!(notify.cell(Kind::Mail, column), Cell::Later | Cell::Never);
    let resting = (asleep && held(Column::Sleep)) || (paused && held(Column::Pause));
    config.reminders.mail && !resting && now - last >= MAIL_EVERY - 30 && config.accounts.iter().any(sioul_core::config::Account::syncs)
}

/// The inbox of each account fetched, its arrivals handed to the new-mail
/// notifications; whether any came.
fn fetch_mail(config: &Config) -> bool {
    let mut any = false;
    for account in config.accounts.iter().filter(|a| a.syncs()) {
        let Ok(password) = sioul_sync::secret::password(account) else { continue };
        match sioul_sync::inbox(account, &password) {
            Ok(report) => {
                any |= !report.new.is_empty();
                crate::mailnote::arrived(&account.id, &report.new, report.first);
            }
            Err(e) => eprintln!("sioul: steps: {}: {e:?}", account.id),
        }
    }
    any
}

/// One step of the background service (see the module's words): its answer
/// for Java, {next (seconds), folder, own, line, title, channel, stop}.
fn step(reason: &str) -> serde_json::Value {
    let config = load_config();
    let Some((own, vault)) = crate::share::vault() else { return serde_json::json!({ "stop": true }) };
    let Some((folder, _)) = vault else {
        // Sharing off: nothing to keep in step; the service ends until it is on again.
        return serde_json::json!({ "stop": true });
    };
    if !config.dnd.background {
        return serde_json::json!({ "stop": true });
    }
    // 1. The others' news: at an alarm, the sync app asked first and given twenty
    // seconds, unless you sleep and no other device is in use (nothing is coming:
    // its own pace is enough); a file the sync app just wrote, read at once.
    let (asleep, paused) = crate::everywhere::rest_now();
    let asleep = asleep && !paused;
    let in_use = crate::devices::others_in_use();
    // "Let every call through" pressed on the notification: carried into the switch
    // and the calls' table first, and sent at once, the sync app not waited for.
    let pressed = reason == "calls";
    let calls = if pressed { Some(crate::calls::step(true)) } else { None };
    let fetch_first = reason != "folder" && !pressed && (in_use || !asleep);
    if let Some(Err(e)) = crate::share::exchange_here(fetch_first) {
        eprintln!("sioul: steps: {e}");
    }
    // The calls' table as your devices' news left it, and the notification's words for them.
    let calls = calls.unwrap_or_else(|| crate::calls::step(false));
    // 2. Do-not-disturb: Sioul's own process asked when what it should ask changed.
    let (signature, line) = crate::everywhere::wanted();
    if let Ok(mut poked) = POKED.lock()
        && (poked.0 != signature || poked.1 != line)
    {
        java("poke", "{}");
        *poked = (signature, line.clone());
    }
    // 3. Mail at its rhythm, then what waited and may come now.
    let now = jiff::Timestamp::now().as_second();
    if mail_due(&config, now, MAILED.load(Ordering::Relaxed), asleep, paused) {
        MAILED.store(now, Ordering::Relaxed);
        if fetch_mail(&config) {
            // New mail is told in one notification ten seconds after the last arrival: the step waits for it.
            std::thread::sleep(std::time::Duration::from_secs(12));
        }
        // What waited and may come now ("The Porch opens"), at the mail's rhythm.
        crate::mailnote::tick();
    }
    let next = next_step(in_use, asleep);
    let words = words();
    serde_json::json!({
        "next": next,
        "folder": folder.display().to_string(),
        "own": own,
        "line": line,
        "title": words["title"],
        "channel": words["channel"],
        "calls": calls,
        "stop": false,
    })
}

/// Java's step (StepService, on its worker thread, the phone kept awake).
///
/// # Safety
/// `reason` is null or a zero-terminated text valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_steps_step(reason: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    let reason = if reason.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(reason) }.to_string_lossy().to_string() };
    let answer = std::panic::catch_unwind(|| step(&reason)).unwrap_or_else(|_| serde_json::json!({ "next": STEP_AWAKE }));
    crate::alarms::handed(answer.to_string())
}

/// Java says this process is the background service's: Android's modes are
/// never touched from it (`everywhere::apply` runs in Sioul's own process).
#[unsafe(no_mangle)]
pub extern "C" fn sioul_steps_in_service() {
    IN_SERVICE.store(true, Ordering::Relaxed);
}

/// Do-not-disturb applied in Sioul's own process (DndReceiver): asked by the
/// service, at its next end, after a restart.
#[unsafe(no_mangle)]
pub extern "C" fn sioul_dnd_apply() -> *mut std::ffi::c_char {
    let answer = std::panic::catch_unwind(|| {
        crate::everywhere::apply();
        crate::everywhere::moment().to_string()
    })
    .unwrap_or_default();
    crate::alarms::handed(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rhythm_follows_the_other_devices_and_the_night() {
        assert_eq!(next_step(true, false), 120);
        assert_eq!(next_step(true, true), 120, "a device in use at night: its news stays fresh");
        assert_eq!(next_step(false, true), 900);
        assert_eq!(next_step(false, false), 300);
    }

    #[test]
    fn mail_comes_at_its_rhythm_never_asleep_nor_paused() {
        let mut config: Config = toml::from_str("[reminders]\nmail = true\n[[account]]\nid = \"a\"\nkind = \"imap\"\naddress = \"jane@example.org\"\nhost = \"imap.example.org\"\n").unwrap();
        assert!(config.accounts.iter().any(sioul_core::config::Account::syncs));
        let now = 10_000;
        assert!(mail_due(&config, now, 0, false, false));
        assert!(!mail_due(&config, now, now - 60, false, false), "fetched a minute ago");
        assert!(mail_due(&config, now, now - MAIL_EVERY, false, false));
        assert!(!mail_due(&config, now, 0, true, false), "asleep");
        assert!(!mail_due(&config, now, 0, false, true), "paused");
        // New mail told while you sleep, as the notification matrix now says: fetched then too.
        let mut told: Config = toml::from_str("[notify]\nmail = [\"sleep\"]\n").unwrap();
        told.accounts = config.accounts.clone();
        assert!(mail_due(&told, now, 0, true, false) && !mail_due(&told, now, 0, false, true));
        config.reminders.mail = false;
        assert!(!mail_due(&config, now, 0, false, false), "new mail not notified at all");
        config.reminders.mail = true;
        config.accounts.clear();
        assert!(!mail_due(&config, now, 0, false, false), "no account");
    }

    /// The coordinator's rule (6 October 2026): the list's stars are said and
    /// their contacts opened, never written. Checked in what the APK is made of.
    #[test]
    fn sioul_never_writes_a_contact_nor_a_star() {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/package");
        let manifest = std::fs::read_to_string(package.join("AndroidManifest.xml")).unwrap();
        assert!(manifest.contains("android.permission.READ_CONTACTS"));
        assert!(!manifest.contains("WRITE_CONTACTS"), "no permission to write contacts");
        let contacts = std::fs::read_to_string(package.join("src/com/aurelienpierre/sioul/DndContacts.java")).unwrap();
        for write in [".insert(", ".update(", ".delete(", "applyBatch", "ContentProviderOperation"] {
            assert!(!contacts.contains(write), "DndContacts.java writes: {write}");
        }
    }

    #[test]
    fn nothing_reaches_java_on_a_computer() {
        assert_eq!(java("running", "{}"), serde_json::Value::Null);
        next(Some(1));
        assert!(!in_service());
    }
}

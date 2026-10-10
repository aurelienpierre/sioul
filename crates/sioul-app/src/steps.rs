// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The phone in the background: "Sioul keeps your devices in step"
//! (docs/android.md, "In the background"; StepService.java). A foreground
//! service in a process of its own (":steps"), which holds Sioul's library
//! without Qt's window, asks Rust for a step at each of its alarms and when
//! the sync app writes another device's file into the sharing folder:
//!
//! 1. the other devices' news: at an alarm, Sioul's own pull from the server
//!    while it works (else the sync app asked to look first, and waited for),
//!    then a quick exchange (`share::exchange_here`, which also keeps this
//!    device's entry among the devices up to date; the service never says the
//!    phone is in use); a file the sync app wrote that the pull brought
//!    already, left out (`brought_already`); then the sealed files of the
//!    others' notes and papers this phone waits for, fetched by their names
//!    (`share::fetch_wanted_here`), for the window's exchange to write;
//! 2. do-not-disturb: what this device should ask of its system, compared
//!    with what was asked last; changed, Sioul's own process is asked to apply
//!    it (`DndReceiver`), where Android's modes of Sioul's are kept;
//! 3. mail at its rhythm, the inbox only, fetched from the step's start
//!    beside the exchange, every account at once, so that the step's network
//!    comes in one burst (on mobile data, the radio woken once): what your
//!    own spam filter moves as you chose, into the Junk folder
//!    (`spam::after_fetch`); what your mail filters do, on the server, to the
//!    arrivals still unread (`filters::after_fetch`); the rest handed to the
//!    new-mail notifications (`mailnote`), told as soon as every account is
//!    fetched, which never tell what the spam filter flagged or moved, nor
//!    what a mail filter moves out of the inbox or marks read;
//! 4. the calls' table (`calls::step`): made again when what it is made of
//!    changed, and the notification's "Let every call through";
//! 5. when to look next: two minutes while another device is in use, five
//!    otherwise, fifteen while you sleep.
//!
//! On a computer nothing here runs: the window does all of it each minute.

use crate::backend::{load_config, tr};
use sioul_core::config::Config;
use std::path::Path;
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
// SAFETY: declared as android/main.cpp defines them: extern "C", the same types.
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
/// or pause unless the matrix of what reaches you tells some mail then (as
/// usual, nothing is told then, your safe senders' and Always through's
/// shown, not told; it comes when Sioul opens or after), and only when new
/// mail is notified at all.
fn mail_due(config: &Config, now: i64, last: i64, asleep: bool, paused: bool) -> bool {
    use sioul_core::attention::{Attention, Column, Level, Row, persons};
    use sioul_core::reach::Channel;
    let attention = Attention::of(config);
    let held = |column: Column| !persons(Channel::Mail).iter().any(|p| attention.cell(Row::People(Channel::Mail, *p), column) == Level::Now);
    let resting = (asleep && held(Column::Sleep)) || (paused && held(Column::Pause));
    config.reminders.mail && !resting && now - last >= MAIL_EVERY - 30 && config.accounts.iter().any(sioul_core::config::Account::syncs)
}

/// The inbox of each account fetched, every account at once (each in a
/// thread of its own, as the window's watchers fetch them: one connection
/// each, the account's lock across processes), what your spam filter moves
/// moved, what your mail filters do done, its arrivals handed to the new-mail
/// notifications, told in one notification once every account is fetched
/// (`mailnote::close_now`), and to the home screen's card when any came
/// (`homecard::mail_came`); whether any came.
fn fetch_mail(config: &Config) -> bool {
    let any = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        for account in config.accounts.iter().filter(|a| a.syncs()) {
            let any = &any;
            scope.spawn(move || {
                let Ok(password) = sioul_sync::secret::password(account) else { return };
                match sioul_sync::inbox(account, &password) {
                    Ok(report) => {
                        let mut came = !report.new.is_empty();
                        crate::spam::after_fetch(account, &report.new, report.first);
                        // Your mail filters, as every device that fetches runs them (docs/client.md,
                        // "Filters"): what they would take quietly is held back from the
                        // notifications, and told as new mail if they could not act on it.
                        let filtered = crate::filters::after_fetch(account, &report.new, report.first);
                        if !filtered.problem.is_empty() {
                            eprintln!("sioul: steps: {}: {}", account.id, filtered.problem);
                        }
                        crate::mailnote::arrived(&account.id, &report.new, report.first);
                        crate::mailnote::unfiltered(&account.id, &filtered.tell);
                        came |= !filtered.tell.is_empty();
                        any.fetch_or(came, Ordering::Relaxed);
                    }
                    Err(e) => eprintln!("sioul: steps: {}: {e:?}", account.id),
                }
            });
        }
    });
    // Every account's arrivals known: told now, in one notification.
    crate::mailnote::close_now();
    let any = any.into_inner();
    if any {
        crate::homecard::mail_came();
    }
    any
}

/// The files another device wrote that the sync app just put in the sharing
/// folder (`StepService`'s watch: their names, "/" between them), each one
/// the pull brought already, the same here as fetched (`remote::overlay`):
/// whole for a small file, by its size for a larger one (a log's round, which
/// only grows). Nothing new to read: the folder's step is left out. Not when
/// nothing is fetched beside the folder, nor for a name not known.
fn brought_already(folder: &Path, names: &str) -> bool {
    const SMALL: u64 = 64 << 10;
    let Some(fetched) = sioul_sync::remote::overlay(folder) else { return false };
    let names: Vec<&str> = names.split('/').filter(|n| !n.is_empty() && *n != "." && *n != "..").collect();
    let same = |name: &str| {
        let (here, there) = (folder.join(name), fetched.join(name));
        match (std::fs::metadata(&here), std::fs::metadata(&there)) {
            (Ok(a), Ok(b)) if a.is_file() && b.is_file() && a.len() == b.len() => a.len() > SMALL || std::fs::read(&here).ok() == std::fs::read(&there).ok(),
            _ => false,
        }
    };
    !names.is_empty() && names.iter().all(|name| same(name))
}

/// The sharing's full rounds wait while the connection is measured slow
/// (`share::set_frugal`: a round restating the records, a megabyte or two
/// sent whole, waits while this device appended less than four), as the
/// window's exchanges have them. While a bulk import of texts goes on, none
/// starts at all: `share::exchange_here` says so before each exchange, an
/// alarm's too (`share::set_importing`).
fn rounds_wait() {
    let memory = sioul_core::config::state_dir().join("share").join("memory.json");
    sioul_sync::share::set_frugal(&memory, sioul_sync::remote::Sending::load(&memory).slow());
}

/// The background service's notification as things stand now (StepService;
/// docs/android.md, "In the background"): {title, text, calls}. Its title,
/// what now is for and until when; its text, do-not-disturb on or off, then
/// the calls, screened or every call ringing; `calls`, the words of its
/// buttons (`calls::words`). And when what now is for ends (Unix seconds).
pub(crate) fn note() -> (serde_json::Value, Option<i64>) {
    let (title, dnd, until) = crate::everywhere::note();
    let calls = crate::calls::words();
    let calls_line = match (calls["screening"] == true, calls["through"] == true) {
        (false, _) => String::new(),
        (true, true) => calls["line"].as_str().unwrap_or_default().to_string(),
        (true, false) => tr().text("steps-note-calls-screened", None),
    };
    let text = [dnd, calls_line].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
    let at = jiff::Timestamp::now().as_millisecond();
    (serde_json::json!({ "at": at, "title": title, "text": text, "calls": calls }), until)
}

/// The notification's words told to the background service at once
/// (StepService.call "note"; after each apply in Sioul's own process: a press
/// of the switch, the phone's own do-not-disturb heard, the tile, a mode's
/// end, another device's change, the calls' switch, the window's minute), so
/// that it says the state now without waiting for its next step. Java shows
/// them only when they changed, and never words older than those shown: no
/// copy kept here, which the service's own steps would make stale. Nothing
/// on a computer, nor from the service's own process, whose step says it.
pub(crate) fn tell_note() {
    if !cfg!(target_os = "android") || in_service() {
        return;
    }
    java("note", &note().0.to_string());
}

/// One step of the background service (see the module's words): its answer
/// for Java, {next (seconds), folder, own, title, text, channel, calls, stop},
/// or {skip} for a folder's step with nothing new (the alarm left as it is).
/// `reason`: `alarm`, `folder:<names>` ("/" between them), `messages`,
/// `heard`, `calls`, `start`, `restart`.
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
    let (reason, names) = reason.split_once(':').unwrap_or((reason, ""));
    // A file the sync app just wrote that the pull brought already: nothing new to read.
    if reason == "folder" && brought_already(&folder, names) {
        return serde_json::json!({ "skip": true });
    }
    // 1. The others' news: at an alarm, Sioul's own pull from the server while it
    // works, else the sync app asked first and given twenty seconds
    // (`share::exchange_here`), unless you sleep and no other device is in use
    // (nothing is coming: the pull's own pace is enough); a file the sync app just wrote, read at once.
    let (asleep, paused) = crate::everywhere::rest_now();
    let asleep = asleep && !paused;
    let in_use = crate::devices::others_in_use();
    // 3, begun now: mail at its rhythm, fetched beside the exchange, so that the
    // step's network comes in one burst; then what waited and may come now
    // ("The Porch opens"), at the mail's rhythm. Waited for before the answer.
    let started = jiff::Timestamp::now().as_second();
    let mail = mail_due(&config, started, MAILED.load(Ordering::Relaxed), asleep, paused).then(|| {
        MAILED.store(started, Ordering::Relaxed);
        std::thread::spawn(|| {
            fetch_mail(&load_config());
            crate::mailnote::tick();
        })
    });
    // "Let every call through" pressed on the notification: carried into the switch
    // and the calls' table first, and sent at once, the sync app not waited for.
    let pressed = reason == "calls";
    let calls = if pressed { Some(crate::calls::step(true)) } else { None };
    // A press heard from this phone's own do-not-disturb (DndReceiver): sent at once too; and
    // a message's line for your computers (`phonemsgs`, the listener's "messages").
    let fetch_first = reason != "folder" && reason != "heard" && reason != "messages" && !pressed && (in_use || !asleep);
    rounds_wait();
    if let Some(Err(e)) = crate::share::exchange_here(fetch_first) {
        eprintln!("sioul: steps: {e}");
    }
    // texts: the requests this exchange brought, decided and sent now, their outcomes shared at
    // once; a batch of the import too, the full rounds waiting while it goes on.
    if crate::texts::phone_step() {
        rounds_wait();
        if let Some(Err(e)) = crate::share::exchange_here(false) {
            eprintln!("sioul: steps: {e}");
        }
    }
    // The others' sealed notes and papers this phone waits for, fetched
    // from the server by their names beside the mail (`share::fetch_wanted_here`):
    // the window's exchange, or the next step, writes them. Waited for before
    // the answer, with the mail.
    let wanted = std::thread::spawn(crate::share::fetch_wanted_here);
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
    // The notification's words shown now, before mail ends, which may take a while.
    java("note", &note().0.to_string());
    // 3. Mail, done: every account fetched and its batch told (no wait after it).
    if let Some(mail) = mail
        && mail.join().is_err()
    {
        eprintln!("sioul: steps: mail stopped short");
    }
    // The sealed files waited for, twenty-five seconds more at most: a large
    // paper on a slow line goes on without holding the step (asked again
    // later if it is cut).
    let until = std::time::Instant::now() + std::time::Duration::from_secs(25);
    while !wanted.is_finished() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // The notification's words now; the next step no later than the end of what now is for,
    // so that its title changes with the time.
    let now = jiff::Timestamp::now().as_second();
    let (note, until) = note();
    let next = until.map(|u| u - now + 1).filter(|s| *s > 0).map_or(next_step(in_use, asleep), |s| s.min(next_step(in_use, asleep)));
    let words = words();
    serde_json::json!({
        "next": next,
        "folder": folder.display().to_string(),
        "own": own,
        "at": note["at"],
        "title": note["title"],
        "text": note["text"],
        "channel": words["channel"],
        "calls": calls,
        "stop": false,
    })
}

/// Java's step (StepService, on its worker thread, the phone kept awake).
///
/// # Safety
/// `reason` is null or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_steps_step(reason: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    // SAFETY: `reason` is not null here, and a C string valid for the call, as this function's contract says.
    let reason = if reason.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(reason) }.to_string_lossy().to_string() };
    let answer = std::panic::catch_unwind(|| step(&reason)).unwrap_or_else(|_| serde_json::json!({ "next": STEP_AWAKE }));
    crate::alarms::handed(answer.to_string())
}

/// Java says this process is the background service's: Android's modes are
/// never touched from it (`everywhere::apply` runs in Sioul's own process).
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub extern "C" fn sioul_steps_in_service() {
    IN_SERVICE.store(true, Ordering::Relaxed);
    // It exchanges every few minutes for days: the sharing's memory and its
    // logs of lines kept between exchanges, not read again at each (`share::set_keeping`).
    sioul_sync::share::set_keeping(true);
}

/// Do-not-disturb applied in Sioul's own process (DndReceiver): asked by the
/// service, at its next end, after a restart.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub extern "C" fn sioul_dnd_apply() -> *mut std::ffi::c_char {
    let answer = std::panic::catch_unwind(|| {
        crate::everywhere::apply();
        crate::everywhere::moment().to_string()
    })
    .unwrap_or_default();
    crate::alarms::handed(answer)
}

/// A change of this phone's own do-not-disturb, heard as it happened
/// (DndReceiver, in Sioul's own process): {on}, the system's state. A press of
/// the switch when it disagrees with what holds (`everywhere::heard_from_system`).
///
/// # Safety
/// `json` is null or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_dnd_heard(json: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    // SAFETY: `json` is not null here, and a C string valid for the call, as this function's contract says.
    let json = if json.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy().to_string() };
    let answer = std::panic::catch_unwind(|| {
        let asked: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        if let Some(on) = asked["on"].as_bool() {
            crate::everywhere::heard_from_system(on);
        }
        crate::everywhere::moment().to_string()
    })
    .unwrap_or_default();
    crate::alarms::handed(answer)
}

/// Sioul's switch pressed on the phone's quick-settings tile (DndTile, through
/// DndReceiver, in Sioul's own process).
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub extern "C" fn sioul_dnd_toggle() -> *mut std::ffi::c_char {
    let answer = std::panic::catch_unwind(|| {
        crate::everywhere::toggle_here();
        crate::everywhere::moment().to_string()
    })
    .unwrap_or_default();
    crate::alarms::handed(answer)
}

/// A press made on this phone outside the window (a system's change heard,
/// the tile): sent at once by the background service, or, while it does not
/// run, by this process itself (one exchange, the sync app not asked first).
/// Nothing on a computer: the window sends its own.
pub(crate) fn send_now() {
    if !cfg!(target_os = "android") {
        return;
    }
    if java("heard", "{}")["stepped"] != true
        && let Some(Err(e)) = crate::share::exchange_here(false)
    {
        eprintln!("sioul: do-not-disturb: {e}");
    }
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

    /// A file the sync app wrote that the pull brought already, the same
    /// here as fetched, brings no step; anything else does.
    #[test]
    fn a_file_the_pull_brought_already_brings_no_step() {
        let dir = std::env::temp_dir().join(format!("sioul-steps-brought-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (folder, state) = (dir.join("Sioul"), dir.join("state").join("share"));
        let fetched = state.join("remote");
        for d in [&folder, &fetched] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join("seal.toml"), "version = 1\n").unwrap();
        }
        let mut remote = toml::Table::new();
        remote.insert("folder".into(), folder.display().to_string().into());
        std::fs::write(state.join("remote.toml"), toml::to_string(&remote).unwrap()).unwrap();
        let both = |name: &str, here: &[u8], there: &[u8]| {
            std::fs::write(folder.join(name), here).unwrap();
            std::fs::write(fetched.join(name), there).unwrap();
        };
        both("7c41d09e-1.jsonl", b"line one\nline two\n", b"line one\nline two\n");
        both("7c41d09e.toml", b"read = 1\n", b"read = 1\n");
        // Nothing fetched beside the folder in this process yet: read as before.
        assert!(!brought_already(&folder, "7c41d09e-1.jsonl"));
        assert!(sioul_sync::remote::attach(&folder, &state.join("memory.json")).is_some());
        assert!(brought_already(&folder, "7c41d09e-1.jsonl/7c41d09e.toml"));
        // A small file the same size, otherwise: read.
        both("7c41d09e.toml", b"read = 2\n", b"read = 1\n");
        assert!(!brought_already(&folder, "7c41d09e-1.jsonl/7c41d09e.toml") && !brought_already(&folder, "7c41d09e.toml"));
        // A round longer here (the sync app newer than the pull): read.
        both("7c41d09e-1.jsonl", b"line one\nline two\nline three\n", b"line one\nline two\n");
        assert!(!brought_already(&folder, "7c41d09e-1.jsonl"));
        // A large round, the same size: brought (a round only grows).
        both("7c41d09e-2.jsonl", &vec![b'a'; 70 << 10], &vec![b'b'; 70 << 10]);
        assert!(brought_already(&folder, "7c41d09e-2.jsonl"));
        // Not fetched, no name, or a name that is no file's: read.
        std::fs::write(folder.join("f00d.toml"), b"x").unwrap();
        assert!(!brought_already(&folder, "f00d.toml") && !brought_already(&folder, "") && !brought_already(&folder, ".."));
        let _ = std::fs::remove_dir_all(&dir);
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

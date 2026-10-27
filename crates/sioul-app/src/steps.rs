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
//!    (`share::fetch_wanted_here`; Sioul keeping the folder itself, `blobs/`
//!    listed too when due, `remote::sealed_step`, and the contents of the
//!    notes this step's exchange left unread asked for), for the window's
//!    exchange to write, offline too; the spam filter's table, for the next
//!    step's;
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
//!    changed, and the notification's Expecting a call;
//! 5. when to look next: two minutes while the phone keeps close
//!    (`keeps_close`, read again after the step's pull: another device in
//!    use; while you sleep, a computer only with someone at it, or any while
//!    a dose is due and not marked), five otherwise, fifteen while you sleep.
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

/// How long until the next step (seconds): two minutes while the phone keeps
/// close (`keeps_close`: another device in use, its news fresh, the doses'
/// too, and do-not-disturb pressed there comes within them), fifteen while
/// you sleep, five otherwise.
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

/// Whether the phone keeps its two-minute steps (docs/android.md, "Its
/// rhythm"), as the devices' entries read now (the folder and the last
/// pull's copy): another device in use (`sioul_sync::devices::others_in_use`:
/// while you are awake, a computer whose Sioul runs; while you sleep, only one
/// someone is at); or, while you sleep, a dose due and not marked while
/// another device runs (`health::dose_open_now`): a mark made there comes
/// within the two minutes, not at the phone's next slow look (docs/health.md,
/// "Knowing"). Read before the step's pull, for whether it pulls at once, and
/// again after it, for when the next step comes (`step`).
fn keeps_close(asleep: bool) -> bool {
    let Some((own, entries)) = crate::devices::entries() else { return false };
    keeps_close_of(&entries, &own, jiff::Timestamp::now().as_second(), asleep, crate::health::dose_open_now)
}

/// `keeps_close`, from what was read: the devices' entries, this phone's
/// id, now, you asleep, and whether a dose is due and not marked (asked only
/// when it decides).
fn keeps_close_of(entries: &[sioul_sync::devices::Entry], own: &str, now: i64, asleep: bool, dose_open: impl FnOnce() -> bool) -> bool {
    use sioul_sync::devices::others_in_use;
    others_in_use(entries, own, now, asleep) || (asleep && others_in_use(entries, own, now, false) && dose_open())
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
    // (`share::exchange_here`), unless you sleep and the phone need not keep
    // close (nothing is coming: the pull's own pace is enough); a file the sync
    // app just wrote, read at once. Whether it keeps close, as the last pull left
    // the others' entries: read again after this step's pull, for the next step.
    let (asleep, paused) = crate::everywhere::rest_now();
    let asleep = asleep && !paused;
    let in_use = keeps_close(asleep);
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
    // Expecting a call pressed on the notification: carried into the switch
    // and the calls' table first, and sent at once, the sync app not waited for.
    let pressed = reason == "calls";
    let calls = if pressed { Some(crate::calls::step(true)) } else { None };
    // A press heard from this phone's own do-not-disturb (DndReceiver): sent at once too; and
    // a message's line for your computers (`phonemsgs`, the listener's "messages").
    let fetch_first = reason != "folder" && reason != "heard" && reason != "messages" && !pressed && (in_use || !asleep);
    rounds_wait();
    rearm_after(crate::share::exchange_here(fetch_first));
    // texts and replies: the requests this exchange brought, decided and handed over now, their
    // outcomes shared at once; a batch of the import too, the full rounds waiting while it goes on.
    let replied = crate::replies::phone_step();
    if crate::texts::phone_step() || replied {
        rounds_wait();
        rearm_after(crate::share::exchange_here(false));
    }
    // The others' sealed notes and papers this phone waits for, fetched
    // from the server by their names beside the mail (`share::fetch_wanted_here`;
    // Sioul keeping the folder itself, `blobs/` listed too when due): the
    // window's exchange writes them, offline too (this step's reads no notes);
    // the spam filter's table, the next step's. Waited for before the answer,
    // with the mail; never by a dose's alarm.
    let wanted = std::thread::spawn(crate::share::fetch_wanted_here);
    // The copies beside drafts that hold an encrypted message's text in
    // clear, and their earlier versions, encrypted again, after this step's
    // exchange and beside the mail: never on a dose's alarm (review of 19
    // October 2026, F9). Waited for before the answer, as the mail is.
    let look = std::thread::spawn(crate::crypto::seal_clear_drafts);
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
    while !(wanted.is_finished() && look.is_finished()) && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Whether the phone keeps close, read again now that this step's pull brought
    // the others' entries: someone back at a computer, a dose marked there, seen
    // from this step on, not one slow step later (review of battery part C, F1).
    let close = keeps_close(asleep);
    // The notification's words now; the next step no later than the end of what now is for,
    // so that its title changes with the time.
    let now = jiff::Timestamp::now().as_second();
    let (note, until) = note();
    let next = until.map(|u| u - now + 1).filter(|s| *s > 0).map_or(next_step(close, asleep), |s| s.min(next_step(close, asleep)));
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

/// A background step's exchange, said when it failed. One that brought a
/// medicine's change, a mark or a take back from another device has handed
/// Android the doses' alarms anew already, as every exchange that wrote the
/// health files does, wherever it runs (`share::exchanged`,
/// `alarms::health_written`; review of 36002ea's check, A2): a move made
/// elsewhere rings here at its new time, a dose marked there rings no more,
/// and the reminder of a dose gone is taken away, to come back with it.
fn rearm_after(exchanged: Option<Result<sioul_sync::share::Outcome, String>>) {
    if let Some(Err(e)) = exchanged {
        eprintln!("sioul: steps: {e}");
    }
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

    use sioul_sync::devices::{COMPUTER, Entry};

    /// The phone's id in the wake counts below.
    const PHONE_ID: &str = "7c41d09e-0000-4000-8000-000000000001";
    const DESK_ID: &str = "0b5e11a2-0000-4000-8000-000000000002";

    /// What one hour of the background service does on the network, as its
    /// steps pace it (`next_step`, `mail_due`, `remote::due`, `keeps_close_of`):
    /// its alarm steps, the pulls among them, the mail rounds fetched in their
    /// bursts (one burst a step: pull, send and mail together, on one
    /// connection), the steps a message for your computers or a text brings
    /// (twenty seconds after it, a burst of their own unless an alarm step
    /// comes within those seconds), the bursts in all (the modem woken once
    /// each), and the longest wait between two pulls (seconds): how late a
    /// computer's do-not-disturb may come.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct Hour {
        steps: u32,
        pulls: u32,
        mail: u32,
        messages: u32,
        wakes: u32,
        longest: i64,
    }

    /// An hour from `start`, after an hour run the same way (its rhythm
    /// settled), the desk as the server holds its entry at each moment
    /// (`desk`; none: no computer running), you asleep or not, a dose due and
    /// not marked or not, messages for your computers (or texts) heard at
    /// `heard` (seconds into the hour). In `step`'s order: whether the phone
    /// keeps close is read from the entry the last pull brought, before the
    /// step's pull (whether it pulls at once), and again after it (when the
    /// next step comes).
    fn an_hour(desk: &dyn Fn(i64) -> Option<Entry>, asleep: bool, dose: bool, heard: &[i64]) -> Hour {
        use sioul_sync::remote::{Pace, State, due};
        let config: Config = toml::from_str("[reminders]\nmail = true\n[[account]]\nid = \"a\"\nkind = \"imap\"\naddress = \"jane@example.org\"\nhost = \"imap.example.org\"\n").unwrap();
        let start = 1_800_000_000;
        let warm = start - 3_600;
        let mut state = State { confirmed: warm - 86_400, tried: warm - 3_600, last: warm - 3_600, ..State::default() };
        let (mut hour, mut mailed, mut at) = (Hour::default(), warm - 3_600, warm);
        let (mut known, mut pulls, mut alarms): (Vec<Entry>, Vec<i64>, Vec<i64>) = (Vec::new(), Vec::new(), Vec::new());
        while at < start + 3_600 {
            let close = keeps_close_of(&known, PHONE_ID, at, asleep, || dose);
            let fetch_first = close || !asleep;
            let in_use = sioul_sync::devices::others_in_use(&known, PHONE_ID, at, asleep);
            let pace = Pace { phone: true, shown: false, in_use, urgent: fetch_first, step: if asleep { STEP_ASLEEP } else { STEP_AWAKE } };
            let pulled = due(&state, at, pace);
            if pulled {
                (state.tried, state.last) = (at, at);
                known = desk(at).into_iter().collect();
                pulls.push(at);
            }
            let mail = mail_due(&config, at, mailed, asleep, false);
            if mail {
                mailed = at;
            }
            if at >= start {
                hour.steps += 1;
                hour.pulls += u32::from(pulled);
                hour.mail += u32::from(mail);
                hour.wakes += u32::from(pulled || mail);
                alarms.push(at);
            }
            at += next_step(keeps_close_of(&known, PHONE_ID, at, asleep, || dose), asleep);
        }
        // The longest wait between two pulls in the hour, the wait from its last one to the next step included.
        pulls.push(at);
        hour.longest = pulls.windows(2).filter(|w| w[1] > start).map(|w| w[1] - w[0]).max().unwrap_or(0);
        // A message's step, twenty seconds on (`StepService.SOON_MS`), the one
        // after it joining it (`Later.of`); an alarm step within them takes it along.
        let mut waiting: Option<i64> = None;
        for heard in heard.iter().map(|h| start + h) {
            if waiting.is_some_and(|at| heard <= at) {
                continue;
            }
            let at = heard + 20;
            waiting = Some(at);
            if !alarms.iter().any(|alarm| (heard..=at).contains(alarm)) {
                hour.messages += 1;
                hour.wakes += 1;
            }
        }
        hour
    }

    /// The desk's entry as the server holds it at `at`: in use since well
    /// before, someone at it or not (`active`; none: unsaid, as an older
    /// Sioul's or where its system says nothing), written at each export,
    /// each minute, whoever is at it.
    fn desk(active: Option<bool>) -> impl Fn(i64) -> Option<Entry> {
        move |at: i64| Some(Entry { id: DESK_ID.into(), kind: COMPUTER.into(), working: true, started: at - 86_400, exported: at - at.rem_euclid(60), active, ..Entry::default() })
    }

    /// The phone's network wakes in an hour, from the code, before and after
    /// battery part C (13 October 2026, and its review): with no computer
    /// running, a computer left on with nobody at it, a person at a computer,
    /// and a dose due and not marked. "Before" is the desk's entry as it was,
    /// saying nothing of anyone at it (as an older Sioul's still does): Sioul
    /// running counted. Only the sleep column changed: awake, a computer whose
    /// Sioul runs counts, as before.
    #[test]
    fn the_phone_wakes_as_often_as_someone_may_be_at_a_computer() {
        let none = |_: i64| None;
        let (before, nobody, someone) = (desk(None), desk(Some(false)), desk(Some(true)));
        let wakes = |desk: &dyn Fn(i64) -> Option<Entry>, asleep: bool| an_hour(desk, asleep, false, &[]);
        // No computer running: unchanged, every five minutes awake, every fifteen asleep.
        assert_eq!(wakes(&none, false), Hour { steps: 12, pulls: 12, mail: 4, messages: 0, wakes: 12, longest: 300 });
        assert_eq!(wakes(&none, true), Hour { steps: 4, pulls: 4, mail: 0, messages: 0, wakes: 4, longest: 900 });
        // A computer left on, nobody at it: thirty wakes an hour before, day and night alike;
        // now thirty while you are awake, as before, and four while you sleep.
        assert_eq!(wakes(&before, true), Hour { steps: 30, pulls: 30, mail: 0, messages: 0, wakes: 30, longest: 120 });
        assert_eq!(wakes(&before, false), Hour { steps: 30, pulls: 30, mail: 4, messages: 0, wakes: 30, longest: 120 });
        assert_eq!(wakes(&nobody, false), wakes(&before, false), "awake: Sioul running counts, as before");
        assert_eq!(wakes(&nobody, true), wakes(&none, true), "asleep: as with no computer running");
        // A dose due and not marked, a computer running: two-minute steps, while you sleep too;
        // with no other device, nothing to keep close to.
        assert_eq!(an_hour(&nobody, true, true, &[]), wakes(&before, true));
        assert_eq!(an_hour(&none, true, true, &[]), wakes(&none, true));
        // Someone at a computer: as before, every two minutes, asleep too.
        assert_eq!(wakes(&someone, false), wakes(&before, false));
        assert_eq!(wakes(&someone, true), wakes(&before, true));
        // A message for your computers (or a text) brings a step twenty seconds on, a burst of
        // its own unless an alarm step comes within them; two heard together go as one.
        assert_eq!(an_hour(&none, true, false, &[100, 110, 1_000]).wakes, 4 + 2);
        assert_eq!(an_hour(&someone, true, false, &[100, 110]).wakes, 30, "taken along by the alarm step at 120 s");
    }

    /// One press at the desk at `press` (do-not-disturb's switch, or a dose
    /// marked there), nobody at the desk before and someone from it on, its
    /// entry reaching the server `upload` seconds after each export (each
    /// minute, and at once at the press): as `step` runs it (`keeps_close_of`
    /// before its pull and again after it, `remote::due`, `next_step`), how
    /// long until the phone pulls it (`came`); how far on the step that pulled
    /// it sets the next (`then`: how late a second press, made just after, may
    /// come); from the press, when the phone's steps are two minutes apart
    /// again (`two`); and how long, meanwhile, a dose due an hour before reads
    /// known not marked elsewhere (`plain`: the card's plain line while the
    /// mark waits at the desk). Adapted from the review of battery part C (F1, F2).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Press {
        came: i64,
        then: i64,
        two: i64,
        plain: i64,
    }

    fn a_press(asleep: bool, dose: bool, press: i64, upload: i64) -> Press {
        use sioul_core::health::{Peer, doubts_now};
        use sioul_sync::remote::{Pace, State, due};
        let server = |t: i64| {
            let at = t - upload;
            let (active, exported) = if at >= press { (Some(true), (at - at.rem_euclid(60)).max(press)) } else { (Some(false), at - at.rem_euclid(60)) };
            Entry { id: DESK_ID.into(), kind: COMPUTER.into(), working: true, started: -86_400, exported, active, doses: true, ..Entry::default() }
        };
        let first = 0;
        let mut state = State { confirmed: first - 86_400, tried: first - 3_600, last: first - 3_600, ..State::default() };
        let mut known: Vec<Entry> = Vec::new();
        let (mut seen, mut next_at) = (first, first);
        let mut out = Press { came: -1, then: -1, two: -1, plain: 0 };
        let mut t = first;
        while t < press + 6 * 3_600 && (out.came < 0 || out.two < 0) {
            if t == next_at {
                // Whether a dose waits: until the mark made at the desk is pulled.
                let open = dose && out.came < 0;
                let close = keeps_close_of(&known, PHONE_ID, t, asleep, || open);
                let fetch_first = close || !asleep;
                let in_use = sioul_sync::devices::others_in_use(&known, PHONE_ID, t, asleep);
                let pace = Pace { phone: true, shown: false, in_use, urgent: fetch_first, step: if asleep { STEP_ASLEEP } else { STEP_AWAKE } };
                let mut brought = false;
                if due(&state, t, pace) {
                    (state.tried, state.last) = (t, t);
                    let fresh = server(t);
                    let state_of = |e: &Entry| (e.started, e.closed, e.working, e.exported, e.left);
                    if known.first().is_none_or(|k| state_of(k) != state_of(&fresh)) {
                        seen = t;
                    }
                    known = vec![fresh];
                    if out.came < 0 && t - upload >= press {
                        out.came = t - press;
                        brought = true;
                    }
                }
                let next = next_step(keeps_close_of(&known, PHONE_ID, t, asleep, || dose && out.came < 0), asleep);
                if brought {
                    out.then = next;
                }
                if out.came >= 0 && out.two < 0 && next == STEP_IN_USE {
                    out.two = t + next - press;
                }
                next_at = t + next;
            }
            // The card between looks: the dose marked at the desk at `press`, not pulled yet.
            if t >= press
                && out.came < 0
                && let Some(entry) = known.first()
            {
                let peer = Peer { id: DESK_ID.into(), name: "desk".into(), said: Some(entry.said()), complete: true, seen, heard: entry.exported, ..Peer::default() };
                if doubts_now(press - 3_600, t, None, std::slice::from_ref(&peer)).is_empty() {
                    out.plain += 1;
                }
            }
            t += 1;
        }
        out
    }

    /// The worst of each measure over presses made every seven seconds across
    /// two whole cycles of the phone's slowest steps, its rhythm settled hours before.
    fn worst(asleep: bool, dose: bool, upload: i64) -> Press {
        let presses: Vec<Press> = (4 * 3_600..4 * 3_600 + 2 * 900).step_by(7).map(|p| a_press(asleep, dose, p, upload)).collect();
        let max = |f: &dyn Fn(&Press) -> i64| presses.iter().map(f).max().unwrap();
        Press { came: max(&|p| p.came), then: max(&|p| p.then), two: max(&|p| p.two), plain: max(&|p| p.plain) }
    }

    /// Do-not-disturb pressed at a desk after a while away from it reaches the
    /// phone at its next look, and the step that brings it brings the
    /// two-minute steps with it: the step reads the others' entries again
    /// after its own pull (review of battery part C, F1; before, it went on at
    /// the slow pace, and while you slept never came back to two minutes).
    /// Awake, a computer whose Sioul runs counts whoever is at it: two minutes
    /// throughout, as before.
    #[test]
    fn a_press_at_a_desk_brings_the_two_minute_steps_from_the_look_that_reads_it() {
        for upload in [0, 30] {
            let awake = worst(false, false, upload);
            assert!(awake.came <= STEP_IN_USE + upload && awake.then == STEP_IN_USE && awake.two <= 2 * STEP_IN_USE + upload, "awake: {awake:?}");
            let asleep = worst(true, false, upload);
            assert!(asleep.came <= STEP_ASLEEP + upload, "asleep: within the night's step: {asleep:?}");
            assert_eq!(asleep.then, STEP_IN_USE, "asleep: the step that brings it brings the two-minute steps: {asleep:?}");
            assert!(asleep.two >= 0 && asleep.two <= STEP_ASLEEP + upload + STEP_IN_USE, "asleep: {asleep:?}");
        }
        // The step reads them again after its pull, for its next (read from the source).
        let source = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/steps.rs")).unwrap();
        let body = source.split("\nfn step(reason: &str)").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        let (pull, again, next) = (body.find("exchange_here(fetch_first)").unwrap(), body.find("let close = keeps_close(asleep);").unwrap(), body.find("next_step(close, asleep)").unwrap());
        assert!(pull < again && again < next, "the next step from what this step's pull brought");
    }

    /// A dose due and not marked, a computer running with nobody at it, you
    /// asleep: the phone keeps its two-minute steps, so that a mark made at
    /// the desk reaches it within them, and the card's plain line ("not marked
    /// elsewhere") lasts two minutes at most past the mark, the desk's upload
    /// aside, not the five that the doses' freshness allows (review of
    /// battery part C, F2). Without a dose waiting, the night's pace, the
    /// doses' rule bounding the plain line by `FRESH` all the same.
    #[test]
    fn a_dose_due_keeps_the_phone_close_while_a_computer_runs() {
        use sioul_core::health::FRESH;
        for upload in [0, 30] {
            let watched = worst(true, true, upload);
            assert!(watched.came <= STEP_IN_USE + upload, "{watched:?}");
            assert!(watched.plain <= STEP_IN_USE + upload, "the card's plain line past the mark: {watched:?}");
            let night = worst(true, false, upload);
            assert!(night.plain <= FRESH + upload, "{night:?}");
        }
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

    /// Calls are untouched by whether someone is at a computer: the calls'
    /// table and its screening (calls.rs, Calls.java, CallScreen.java) read
    /// neither the in-use rule nor what an entry says of it, and "Let every
    /// call through" pressed on the notification still steps at once, its
    /// exchange not waiting for a pull (read from the sources).
    #[test]
    fn calls_are_untouched_by_whether_someone_is_at_a_computer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for path in ["crates/sioul-app/src/calls.rs", "android/package/src/com/aurelienpierre/sioul/Calls.java", "android/package/src/com/aurelienpierre/sioul/CallScreen.java"] {
            let source = std::fs::read_to_string(root.join(path)).unwrap();
            for word in ["others_in_use", "keeps_close", "present_of", "present_since", ".active"] {
                assert!(!source.contains(word), "{path} reads {word}");
            }
        }
        let step = std::fs::read_to_string(root.join("crates/sioul-app/src/steps.rs")).unwrap();
        assert!(step.contains("let pressed = reason == \"calls\";") && step.contains("&& !pressed && (in_use || !asleep);"), "a press on the calls' notification: at once, no pull waited for");
    }

    /// Mail at night: a round only when the matrix of what reaches you tells
    /// some mail then (as usual it tells none while you sleep: no round, no
    /// wake); your Always through set to come at night keeps its rhythm, a
    /// round every fifteen minutes in the night's steps, and no IDLE.
    #[test]
    fn mail_at_night_only_when_the_matrix_tells_some_then() {
        let accounts = "[reminders]\nmail = true\n[[account]]\nid = \"a\"\nkind = \"imap\"\naddress = \"jane@example.org\"\nhost = \"imap.example.org\"\n";
        let usual: Config = toml::from_str(accounts).unwrap();
        let now = 1_800_000_000;
        assert!(!mail_due(&usual, now, 0, true, false), "asleep, as usual: no round");
        let always: Config = toml::from_str(&format!("{accounts}[attention]\n\"mail.always\" = [\"sleep\"]\n")).unwrap();
        assert!(mail_due(&always, now, 0, true, false), "Always through told at night: fetched");
        assert!(!mail_due(&always, now, now - 60, true, false) && mail_due(&always, now, now - MAIL_EVERY, true, false), "at its rhythm");
        // Its rounds fall in the night's steps (every fifteen minutes): four an hour, no wake of their own.
        let mut mailed = now - 3_600;
        let mut rounds = 0;
        for at in (0..4).map(|n| now + n * STEP_ASLEEP) {
            if mail_due(&always, at, mailed, true, false) {
                (mailed, rounds) = (at, rounds + 1);
            }
        }
        assert_eq!(rounds, 4);
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

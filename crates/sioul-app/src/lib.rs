// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul-app`: Sioul's window. The Porch, budgets and accounts, over the same
//! core as the command line; the window only lays out what the core decided.
//! A library: the desktop program (main.rs) runs it, and so does Android's
//! (android/main.cpp, docs/android.md).
//!
//! The window is made of QML pages, in `crates/sioul-app/qml/` (each one is
//! described in the QML reference:
//! <https://aurelienpierre.github.io/sioul/dev/qml.html>), and of this crate's
//! Rust. CXX-Qt joins the two: [`backend`](backend/index.html) declares the
//! QML type `Sioul`, whose properties the pages read and whose functions they
//! call, and [`desktop`](desktop/index.html) a second one, `Desktop`, for the
//! desktop's icons and title bar.
//! What the pages show comes from [sioul-core](../sioul_core/index.html), as
//! JSON with every sentence already in your language; what comes from servers
//! is fetched with [sioul-sync](../sioul_sync/index.html) on other threads,
//! so that the window never waits.
//!
//! Every module here is private: only the window uses them. This reference
//! shows them all.
//!
//! # Where to start reading
//!
//! - [`run`]: the window, from the program's start to its end.
//! - [`backend`](backend/index.html): the object `Sioul`. In its bridge, the
//!   module `qobject`, each `#[qproperty]` is a value the pages read, and are
//!   told about when it changes; each `#[qinvokable]` is a function a page
//!   calls, its name in camelCase in QML (`text_with` is
//!   `sioul.textWith(…)`). `SioulRust` holds what the object keeps.
//! - [`work`](work/index.html) (tasks, notes, links, focus),
//!   [`mail`](mail/index.html) and [`pim`](pim/index.html) (contacts and
//!   calendars): the same pattern, one area at a time. Most functions there
//!   return JSON for a page, or run their work off the window's thread and
//!   hand the result back to it.
//!
//! Names such as docs/android.md are the design notes in the repository's
//! `docs/` folder. The website shows them too, under the same name:
//! <https://aurelienpierre.github.io/sioul/dev/android.html>.
//!
//! # Modules, by theme
//!
//! ## The window's objects
//!
//! - [`backend`](backend/index.html): the object behind the window, `Sioul`, and the work it runs off the window's thread.
//! - [`desktop`](desktop/index.html): the desktop's icon theme, and where the desktop puts a window's buttons.
//! - [`hours`](hours/index.html): what now is for (work, admin, leisure, a meal, sleep), as the pages and notifications ask it.
//!
//! ## Mail
//!
//! - [`mail`](mail/index.html): the mail client: folders and their messages, what you do to them, drafts and sending.
//! - [`gmail`](gmail/index.html): Google's mail, signed in on Google's page.
//! - [`crypto`](crypto/index.html): OpenPGP: protected messages opened for the reader, drafts signed or encrypted, your keys.
//! - [`senders`](senders/index.html): who someone is to you: where they stand, and why.
//! - [`mailnote`](mailnote/index.html): new mail told as a notification, at the times it may come.
//! - [`letters`](letters/index.html): paper letters: scans read in the background, each a card in the Porch.
//! - [`outside`](outside/index.html): writing from other applications: shared files and text, `mailto:` links.
//! - [`sites`](sites/index.html): sites kept in Sioul: the list, their notifications, logins filled from Bitwarden.
//! - [`spam`](spam/index.html): Sioul's own spam filter in the mail settings, and on a computer "Train now" (docs/spam-filter.md).
//!
//! ## Tasks, projects and time
//!
//! - [`work`](work/index.html): tasks, notes, links and focus.
//! - [`blocks`](blocks/index.html): time blocks: a task pinned to a time, as an event in a calendar.
//! - [`capacity`](capacity/index.html): what a day holds, as the window uses it.
//! - [`projects`](projects/index.html): projects, time and invoices.
//! - [`github`](github/index.html): GitHub's issues and pull requests, in the local "GitHub" list.
//! - [`reviews`](reviews/index.html): the two rituals: closing the work day, and closing the day before sleep.
//! - [`remind`](remind/index.html): reminders before dates, each a quiet notification.
//! - [`timenote`](timenote/index.html): the time running, in the system's notifications.
//!
//! ## Contacts and calendars
//!
//! - [`pim`](pim/index.html): contacts and calendars.
//! - [`duplicates`](duplicates/index.html): duplicates in the contacts, found and cleaned when you ask.
//! - [`map`](map/index.html): contacts on a map.
//!
//! ## Money and papers
//!
//! - [`bank`](bank/index.html): the money watch: bank accounts, where each movement went, what passed and what did not.
//! - [`contracts`](contracts/index.html): contracts and subscriptions.
//! - [`papers`](papers/index.html): the papers wallet.
//!
//! ## Pauses, do-not-disturb and health
//!
//! - [`pauses`](pauses/index.html): the two pauses, Free time and Pause.
//! - [`dnd`](dnd/index.html): the system's do-not-disturb, during the pauses and Sioul's own, where the system lets an app set it.
//! - [`everywhere`](everywhere/index.html): do-not-disturb on every device, as this device applies it.
//! - [`health`](health/index.html): the Health page, its settings, and the minute that reminds a dose.
//!
//! ## Your devices
//!
//! - [`share`](share/index.html): sharing with your other devices: set up, one exchange a minute, versions put back.
//! - [`devices`](devices/index.html): this device's sessions, as your other devices learn them.
//!
//! ## On a phone
//!
//! - [`alarms`](alarms/index.html): doses reminded by Android's alarm clock while Sioul is away.
//! - [`eventalarms`](eventalarms/index.html): reminders before events, by Android's alarm clock.
//! - [`wake`](wake/index.html): the alarm at waking.
//! - [`steps`](steps/index.html): the phone in the background: a service that keeps your devices in step.
//! - [`homecard`](homecard/index.html): the card on the phone's home screen.
//! - [`calls`](calls/index.html): calls screened on a phone.
//! - [`appnotes`](appnotes/index.html): other apps' notifications on a phone: now, or how long they wait.
//! - [`reaches`](reaches/index.html): Settings ▸ What reaches you in words, and a person's sheet.

mod alarms;
mod appnotes;
mod backend;
mod bank;
mod blocks;
mod calls;
mod capacity;
mod contracts;
mod crypto;
mod desktop;
mod devices;
mod dnd;
mod eventalarms;
mod duplicates;
// Do-not-disturb on every device, and the phone in the background (docs/do-not-disturb.md).
mod everywhere;
mod github;
// Google's mail signed in on Google's page (docs/google.md, "Mail").
mod gmail;
mod health;
mod homecard;
mod hours;
mod mail;
mod mailnote;
mod letters;
mod map;
mod outside;
mod papers;
mod pauses;
mod pim;
mod projects;
// Settings ▸ What reaches you, in words, and a person's sheet (docs/attention.md).
mod reaches;
mod remind;
mod reviews;
mod senders;
mod share;
mod sites;
mod spam;
mod steps;
mod timenote;
mod wake;
mod work;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

unsafe extern "C" {
    /// Qt Quick drawn as Qt WebEngine (the Sites page) needs; before the application is
    /// made (cpp/webengine.cpp). Android has no Qt WebEngine: its Sites page opens sites in the browser.
    #[cfg(not(target_os = "android"))]
    fn sioul_start_web_engine();
    /// On a computer, Sioul's application, a widgets application (cpp/application.cpp):
    /// the system tray's menu is made of widgets on Plasma. `values` holds `count` C
    /// strings, the program's arguments, which it copies; made with `new`.
    #[cfg(not(target_os = "android"))]
    fn sioul_new_application(count: i32, values: *const *const std::ffi::c_char) -> *mut std::ffi::c_void;
    /// Sioul's own icon on its windows (cpp/appicon.cpp); once the application is made.
    #[cfg(not(target_os = "android"))]
    fn sioul_set_window_icon();
    /// SIGTERM, SIGINT and SIGHUP end Sioul as its window's close does
    /// (cpp/signals.cpp); once the application is made.
    #[cfg(not(target_os = "android"))]
    fn sioul_quit_on_signals();
    /// Sioul has quit: the sites' pages closed as a browser closes its tabs,
    /// before Qt WebEngine shuts down (cpp/webengine.cpp). `engine` is the
    /// QQmlApplicationEngine.
    #[cfg(not(target_os = "android"))]
    fn sioul_close_sites(engine: *mut std::ffi::c_void);
    /// On a phone, what the window draws with given back while Sioul is away
    /// (android/main.cpp); once the window is made.
    #[cfg(target_os = "android")]
    fn sioul_android_lean_window();
    /// Sioul's symbols font, and the system's fonts and image formats read on
    /// a thread of their own (cpp/warmup.cpp); once the application is made.
    fn sioul_warm_up();
    /// CXX-Qt's registration of the window's QML module, resources and types
    /// (cxx-qt-build); runs once however often it is called.
    fn cxx_qt_init_crate_sioul_app() -> bool;
}

/// When Sioul started, for `timing`.
static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// How long the start has taken so far, written to stderr (logcat on Android)
/// on Android and when SIOUL_TIMING is set: what to make faster, measured.
pub(crate) fn timing(what: &str) {
    if timed() {
        let ms = STARTED.get().map_or(0, |start| start.elapsed().as_millis());
        eprintln!("Sioul: {what} after {ms} ms");
    }
}

/// Whether the start is timed: always on Android, else with SIOUL_TIMING.
fn timed() -> bool {
    cfg!(target_os = "android") || std::env::var_os("SIOUL_TIMING").is_some()
}

/// How long ago the process started (Linux, Android): what came before
/// [`run`], the libraries loaded above all, for `timing`.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn process_age() -> Option<std::time::Duration> {
    #[repr(C)]
    struct Timespec {
        seconds: i64,
        nanoseconds: i64,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
    }
    // CLOCK_BOOTTIME: since the system started, its sleep included, as /proc counts.
    const BOOT_TIME: i32 = 7;
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // The fields after the program's name, which ends at the last ")": the
    // 22nd, when the process started, in hundredths of a second since boot.
    let started: f64 = stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.parse().ok()?;
    let mut now = Timespec { seconds: 0, nanoseconds: 0 };
    // SAFETY: `now` is a timespec, which clock_gettime fills.
    if unsafe { clock_gettime(BOOT_TIME, &mut now) } != 0 {
        return None;
    }
    let uptime = now.seconds as f64 + now.nanoseconds as f64 / 1e9;
    Some(std::time::Duration::from_secs_f64((uptime - started / 100.0).max(0.0)))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn process_age() -> Option<std::time::Duration> {
    None
}

/// The window, until it is closed; returns what the program returns.
pub fn run() -> i32 {
    STARTED.get_or_init(std::time::Instant::now);
    if let Some(age) = process_age().filter(|_| timed()) {
        eprintln!("Sioul: the process started {} ms before Sioul did", age.as_millis());
    }
    // Done as the program starts already; asked for by name too, so that a
    // linker reading each library once (GNU gold and ld) keeps it with the
    // window, which is a library.
    // SAFETY: CXX-Qt's own initializer, which does its work once (std::call_once).
    unsafe { cxx_qt_init_crate_sioul_app() };
    // Mail, keys, drafts and caches are kept in folders that are yours alone.
    sioul_core::config::make_private_dirs();
    // A mail link clicked (`sioul-app mailto:…`), or Sioul started again:
    // handed to the Sioul of this profile already open, if one is, which
    // opens the draft or comes forward; else this one opens, and listens for
    // the next (docs/client.md, "Writing from other applications").
    let addresses = outside::addresses();
    if outside::hand_over(&addresses) {
        return 0;
    }
    outside::listen();
    if !addresses.is_empty() {
        outside::hand(&addresses);
    }
    // Qt's QML engine looks, for each file it loads, for variants of it in
    // folders named after the language, the system and the style ("+fr_FR/",
    // "+android/", "+Material/"). Sioul has none, and on a phone the looking
    // took a fifth of the start. Qt's own dialogs, drawn where the system has
    // none, keep their plain look.
    // SAFETY: no other thread reads the environment yet.
    unsafe { std::env::set_var("QT_NO_BUILTIN_SELECTORS", "1") };
    // Qt makes the tip shown on hover from "import QtQuick.Controls", in the
    // system's style: KDE's Breeze and its Kirigami loaded at the first tip,
    // Material on Android, for one tip drawn unlike the rest of Sioul, which
    // is all in the Basic style. Basic, unless you chose another.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        // SAFETY: as above.
        unsafe { std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Basic") };
    }
    // Where the system puts a window's buttons, read meanwhile for Sioul's own
    // title bar (desktop.rs); after the environment is set, which it reads.
    #[cfg(not(target_os = "android"))]
    desktop::warm_up();
    // SAFETY: called once, before any Qt object exists, as Qt WebEngine asks.
    #[cfg(not(target_os = "android"))]
    unsafe {
        sioul_start_web_engine()
    };
    // On a computer, a widgets application: Plasma's system tray makes its menu
    // of widgets, and aborted Sioul without one. A phone has no tray.
    #[cfg(not(target_os = "android"))]
    let mut app = widgets_application();
    #[cfg(target_os = "android")]
    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Sioul"));
        app.as_mut().set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    }
    // SAFETY: the application exists; called once, on the main thread.
    unsafe { sioul_warm_up() };
    timing("the application made");
    // Lets the desktop match the window with its desktop file, com.aurelienpierre.Sioul.desktop
    // (the Flatpak's id too): its name and icon in the taskbar, under Wayland above all.
    QGuiApplication::set_desktop_file_name(&QString::from("com.aurelienpierre.Sioul"));
    // Android's windows have no icon.
    #[cfg(not(target_os = "android"))]
    // SAFETY: the application exists; called once, on the main thread.
    unsafe {
        sioul_set_window_icon()
    };
    // The session ending (SIGTERM), Ctrl+C, the terminal closed: Sioul quits as by its window.
    #[cfg(not(target_os = "android"))]
    // SAFETY: the application exists; called once, on the main thread.
    unsafe {
        sioul_quit_on_signals()
    };
    desktop::icons();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/aurelienpierre/sioul/qml/main.qml"));
    }
    #[cfg(target_os = "android")]
    // SAFETY: the window is made (above); called once, on the main thread.
    unsafe {
        sioul_android_lean_window()
    };
    timing("the window loaded");
    // A session begins: your other devices learn this one is working (docs/database.md,
    // "Devices"). Off the window's thread: the keyring may take a moment the first time.
    std::thread::spawn(devices::window_opened);
    let code = match app.as_mut() {
        Some(app) => app.exec(),
        None => 1,
    };
    // The sites' pages closed before Qt WebEngine shuts down with the
    // application: some sites write their login back only then (docs/sites.md, "Closing").
    #[cfg(not(target_os = "android"))]
    if !engine.is_null() {
        // SAFETY: the engine lives until this function ends; on the main thread, the windows gone.
        unsafe { sioul_close_sites(engine.as_mut_ptr().cast()) };
    }
    // The time running's notification goes with the window (a phone's stays).
    timenote::closing();
    // What was marked goes out, and your other devices learn this one closed.
    share::closing();
    // The next Sioul started opens by itself rather than knocking here.
    outside::stop_listening();
    code
}

/// A widgets application (QApplication), as the QGuiApplication CXX-Qt works
/// with, which it is: made by cpp/application.cpp from the program's
/// arguments, as `QGuiApplication::new` makes its own.
#[cfg(not(target_os = "android"))]
fn widgets_application() -> cxx::UniquePtr<QGuiApplication> {
    let arguments: Vec<std::ffi::CString> = std::env::args_os()
        .filter_map(|argument| {
            // Unix's arguments are bytes; Windows's, read as UTF-8, as CXX-Qt reads them.
            #[cfg(unix)]
            let bytes = std::os::unix::ffi::OsStrExt::as_bytes(argument.as_os_str()).to_vec();
            #[cfg(windows)]
            let bytes = argument.to_string_lossy().into_owned().into_bytes();
            std::ffi::CString::new(bytes).ok()
        })
        .collect();
    let pointers: Vec<*const std::ffi::c_char> = arguments.iter().map(|argument| argument.as_ptr()).collect();
    let count = i32::try_from(pointers.len()).unwrap_or(0);
    // SAFETY: `pointers` holds `count` C strings, alive during the call, which the
    // application copies. It returns a QApplication made with `new`, a QGuiApplication,
    // which the UniquePtr deletes as it deletes CXX-Qt's own; called once, on the main thread.
    unsafe { cxx::UniquePtr::from_raw(sioul_new_application(count, pointers.as_ptr()).cast()) }
}

/// Android: [`run`], for the program Qt for Android starts (android/main.cpp).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn sioul_app_run() -> i32 {
    run()
}

/// Android: the Java side lent to Sioul for Android's KeyStore (passwords)
/// and the network's DNS servers (sender checks); false if it could not be
/// taken, which the log says. Called by android/main.cpp, before [`run`].
///
/// # Safety
/// `vm` is the process's JavaVM and `context` a global reference to the
/// application's Context, never released; called once.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_android_init(vm: *mut std::ffi::c_void, context: *mut std::ffi::c_void) -> bool {
    // SAFETY: as the caller promises.
    match unsafe { sioul_sync::android::init(vm, context) } {
        Ok(()) => true,
        Err(e) => {
            eprintln!("Sioul: Android's keyring is not available: {e}");
            false
        }
    }
}

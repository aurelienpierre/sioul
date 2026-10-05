// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul-app`: Sioul's window. The Porch, budgets and accounts, over the same
//! core as the command line; the window only lays out what the core decided.
//! A library: the desktop program (main.rs) runs it, and so does Android's
//! (android/main.cpp, docs/android.md).

mod alarms;
mod backend;
mod bank;
mod contracts;
mod crypto;
mod desktop;
mod github;
mod health;
mod mail;
mod letters;
mod map;
mod papers;
mod pim;
mod projects;
mod remind;
mod share;
mod sites;
mod timenote;
mod work;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

unsafe extern "C" {
    /// Qt Quick drawn as Qt WebEngine (the Sites page) needs; before the application is
    /// made (cpp/webengine.cpp). Android has no Qt WebEngine: its Sites page opens sites in the browser.
    #[cfg(not(target_os = "android"))]
    fn sioul_start_web_engine();
    /// Sioul's own icon on its windows (cpp/appicon.cpp); once the application is made.
    #[cfg(not(target_os = "android"))]
    fn sioul_set_window_icon();
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
    // SAFETY: called once, before any Qt object exists, as Qt WebEngine asks.
    #[cfg(not(target_os = "android"))]
    unsafe {
        sioul_start_web_engine()
    };
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
    desktop::icons();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/aurelienpierre/sioul/qml/main.qml"));
    }
    timing("the window loaded");
    let code = match app.as_mut() {
        Some(app) => app.exec(),
        None => 1,
    };
    // The time running's notification goes with the window (a phone's stays).
    timenote::closing();
    // What was marked goes out, and your other devices learn this one closed.
    share::closing();
    code
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

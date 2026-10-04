// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul-app`: Sioul's window. The Porch, budgets and accounts, over the same
//! core as the command line; the window only lays out what the core decided.
//! A library: the desktop program (main.rs) runs it, and so does Android's
//! (android/main.cpp, docs/android.md).

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
mod work;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

unsafe extern "C" {
    /// Starts Qt WebEngine, for the Sites page; before the application is made (cpp/webengine.cpp).
    /// Android has no Qt WebEngine: its Sites page opens sites in the browser.
    #[cfg(not(target_os = "android"))]
    fn sioul_start_web_engine();
    /// Sioul's own icon on its windows (cpp/appicon.cpp); once the application is made.
    fn sioul_set_window_icon();
    /// CXX-Qt's registration of the window's QML module, resources and types
    /// (cxx-qt-build); runs once however often it is called.
    fn cxx_qt_init_crate_sioul_app() -> bool;
}

/// When Sioul started, for `timing`.
static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// How long the start has taken so far, written to stderr (logcat on Android)
/// on Android and when SIOUL_TIMING is set: what to make faster, measured.
pub(crate) fn timing(what: &str) {
    if cfg!(target_os = "android") || std::env::var_os("SIOUL_TIMING").is_some() {
        let ms = STARTED.get().map_or(0, |start| start.elapsed().as_millis());
        eprintln!("Sioul: {what} after {ms} ms");
    }
}

/// The window, until it is closed; returns what the program returns.
pub fn run() -> i32 {
    STARTED.get_or_init(std::time::Instant::now);
    // Done as the program starts already; asked for by name too, so that a
    // linker reading each library once (GNU gold and ld) keeps it with the
    // window, which is a library.
    // SAFETY: CXX-Qt's own initializer, which does its work once (std::call_once).
    unsafe { cxx_qt_init_crate_sioul_app() };
    // Mail, keys, drafts and caches are kept in folders that are yours alone.
    sioul_core::config::make_private_dirs();
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
    timing("the application made");
    // Lets the desktop match the window with its desktop file, com.aurelienpierre.Sioul.desktop
    // (the Flatpak's id too): its name and icon in the taskbar, under Wayland above all.
    QGuiApplication::set_desktop_file_name(&QString::from("com.aurelienpierre.Sioul"));
    // SAFETY: the application exists; called once, on the main thread.
    unsafe { sioul_set_window_icon() };
    desktop::icons();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/aurelienpierre/sioul/qml/main.qml"));
    }
    timing("the window loaded");
    match app.as_mut() {
        Some(app) => app.exec(),
        None => 1,
    }
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

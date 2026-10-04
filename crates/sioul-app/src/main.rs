// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul-app`: Sioul's window. The Porch, budgets and accounts, over the same
//! core as the command line; the window only lays out what the core decided.

// A window, not a terminal program: no console opens beside it on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

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
    fn sioul_start_web_engine();
    /// Sioul's own icon on its windows (cpp/appicon.cpp); once the application is made.
    fn sioul_set_window_icon();
}

fn main() {
    // Mail, keys, drafts and caches are kept in folders that are yours alone.
    sioul_core::config::make_private_dirs();
    // SAFETY: called once, before any Qt object exists, as Qt WebEngine asks.
    unsafe { sioul_start_web_engine() };
    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Sioul"));
        app.as_mut().set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    }
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
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}

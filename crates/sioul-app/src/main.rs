// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul-app`: Sioul's window, as a desktop program; the window itself is
//! the library beside it (lib.rs).

// A window, not a terminal program: no console opens beside it on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    sioul_app::run();
}

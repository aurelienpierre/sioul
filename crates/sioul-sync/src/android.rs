// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Android: what is reached through Java there. The keyring is Android's
//! KeyStore (android-keyring: a key kept by the KeyStore, never leaving it,
//! encrypts each password into the app's private storage), and the DNS
//! servers sender checks ask are the network's (hickory, through Android's
//! ConnectivityManager). Both need the process's JavaVM and the app's
//! Context, lent once at start (android/main.cpp, through sioul-app).

use std::ffi::c_void;

/// Takes the Java side Android lends, then makes Android's KeyStore the keyring.
///
/// # Safety
/// `vm` is the process's JavaVM and `context` a global reference to the
/// application's Context, valid for as long as the process lives; called
/// once, before any password is kept or read and before any sender check.
pub unsafe fn init(vm: *mut c_void, context: *mut c_void) -> Result<(), String> {
    // SAFETY: as the caller promises; ndk-context keeps both for every crate that asks.
    unsafe { ndk_context::initialize_android_context(vm, context) };
    android_keyring::set_android_keyring_credential_builder().map_err(|e| e.to_string())
}

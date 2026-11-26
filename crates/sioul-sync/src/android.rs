// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Android: what is reached through Java there. The keyring is Android's
//! KeyStore (android-keyring: a key kept by the KeyStore, never leaving it,
//! encrypts each password into the app's private storage), and the DNS
//! servers sender checks ask are the network's (hickory, through Android's
//! ConnectivityManager). Both need the process's JavaVM and the app's
//! Context, lent once at start (android/main.cpp, through sioul-app).

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether `init` made Android's KeyStore the keyring in this process: a
/// process Java gave no Context (main.cpp's `initialize`) has none, and its
/// keyring is left as it was, never asked of Java.
static READY: AtomicBool = AtomicBool::new(false);
/// The keyring's lock, in Sioul's data folder: the same file in each of its
/// processes (main.cpp's `setFolders` names the folders before `init`).
static LOCK: OnceLock<PathBuf> = OnceLock::new();

/// Takes the Java side Android lends, then makes Android's KeyStore the keyring.
///
/// # Safety
/// `vm` is the process's JavaVM and `context` a global reference to the
/// application's Context, valid for as long as the process lives; called
/// once, before any password is kept or read and before any sender check.
pub unsafe fn init(vm: *mut c_void, context: *mut c_void) -> Result<(), String> {
    // SAFETY: as the caller promises; ndk-context keeps both for every crate that asks.
    unsafe { ndk_context::initialize_android_context(vm, context) };
    android_keyring::set_android_keyring_credential_builder().map_err(|e| e.to_string())?;
    let _ = LOCK.set(sioul_core::config::data_dir().join(".keyring.lock"));
    READY.store(true, Ordering::Release);
    Ok(())
}

/// Where the keyring's lock is (`secret::guarded`); none in a process where
/// `init` did not run.
pub fn keyring_lock() -> Option<PathBuf> {
    READY.load(Ordering::Acquire).then(|| LOCK.get().cloned()).flatten()
}

/// The keyring's file read again when another of Sioul's processes changed
/// it: called with the keyring's lock held, before each read or write of a
/// secret (`secret::guarded`).
///
/// android-keyring keeps every secret in one SharedPreferences file, named
/// after the keyring's service ("sioul"), opened with `MODE_PRIVATE`: Android
/// keeps one copy of it per process and never reads it again. Sioul has
/// several processes (the window's, the background step's `:steps`), each
/// reading and writing secrets: an entry the window wrote (an extension's
/// sign-in) was not seen by `:steps` until it started again, and `:steps`'s
/// next write rewrote the whole file from its own copy, without that entry.
/// Asking for the same file with `MODE_MULTI_PROCESS` (deprecated, still
/// honoured: `ContextImpl.getSharedPreferences`) has Android compare the file
/// with what this process read and read it again when another changed it;
/// `contains` then waits for that reading, so it ends inside the lock.
///
/// Why the lock: Android's reading of a file another process is writing can
/// undo that write (seeing the writer's `.bak`, it puts the old file back),
/// and a reading between one thread's `edit()` and `commit()` can drop that
/// commit from memory. Under one lock that every read and write takes, in
/// every process and thread, no reading ever meets a write: each write is
/// whole on disk (android-keyring's `commit` is synchronous) before the lock
/// is let go. Anything that fails here is left aside: the secret is then
/// read or written as before.
pub fn keyring_fresh(service: &str) {
    use jni::objects::{JObject, JValue};
    /// `Context.MODE_MULTI_PROCESS`.
    const MODE_MULTI_PROCESS: i32 = 4;
    // No Context lent to this process: nothing to ask of Java (and ndk-context would panic).
    if !READY.load(Ordering::Acquire) {
        return;
    }
    let context = ndk_context::android_context();
    // SAFETY: the process's JavaVM that `init` lent ndk-context, alive as long as the process.
    let Ok(vm) = (unsafe { jni::JavaVM::from_raw(context.vm().cast()) }) else { return };
    let Ok(mut env) = vm.attach_current_thread() else { return };
    // SAFETY: a global reference to the application's Context, alive as long as the process
    // (`init`); a `JObject` made from it is never deleted when it is dropped.
    let app = unsafe { JObject::from_raw(context.context().cast()) };
    let _ = env.with_local_frame(4, |env| -> jni::errors::Result<()> {
        let name = env.new_string(service)?;
        let prefs = env.call_method(&app, "getSharedPreferences", "(Ljava/lang/String;I)Landroid/content/SharedPreferences;", &[JValue::Object(&name), JValue::Int(MODE_MULTI_PROCESS)])?.l()?;
        // Waits for the file read again, if it was: within the lock, never after it.
        let nothing = env.new_string("")?;
        env.call_method(&prefs, "contains", "(Ljava/lang/String;)Z", &[JValue::Object(&nothing)])?;
        Ok(())
    });
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
}

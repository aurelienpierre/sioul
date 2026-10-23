// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Android: your security key held against the phone or plugged into its
//! USB-C port, as the phone's link for `sioul_sync::securitykey::phone`
//! (docs/android.md, "Your security key"). Each call goes to
//! `SecurityKeyLink.java` through android/main.cpp: NFC's reader mode, and
//! the USB devices heard, while a band waits for the key and Sioul is in
//! front (never otherwise, so that other apps, OpenKeychain or Yubico
//! Authenticator, and Yubico OTP keep working); the key's APDUs over
//! `IsoDep`, or as CCID messages over its smart card interface alone (never
//! its keyboard, where OTP types); the key let go at the end: Android hands
//! a key held to no other app until it has left the field, a key plugged
//! in is powered off and given back. Java tells when a key comes, goes or
//! is refused, which wakes the band that waits. The APDUs' bytes, a PIN
//! among them, are never logged, and the copies the bridge made are wiped.

use sioul_sync::securitykey::phone::{Held, Link, LinkError, LinkState, Reach, Via};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// The phone's NFC, through Java.
pub(crate) struct AndroidLink {
    /// A key came since the last wait.
    came: Mutex<bool>,
    woken: Condvar,
}

/// The one link, given to the sharing's crate as Sioul starts.
pub(crate) static LINK: AndroidLink = AndroidLink { came: Mutex::new(false), woken: Condvar::new() };

/// Gives the link to `sioul_sync`, once, as Sioul starts on the phone.
pub(crate) fn start() {
    sioul_sync::securitykey::phone::set_link(&LINK);
}

// SAFETY: declared as android/main.cpp defines them: extern "C", the same types.
unsafe extern "C" {
    /// NFC: 0 none in this phone, 1 switched off, 2 ready.
    fn sioul_android_key_state() -> i32;
    /// Whether the phone takes a key plugged in: 1 or 0.
    fn sioul_android_key_usb() -> i32;
    /// The key in reach: 0 none; 1 a key, what the phone carries, plugged
    /// in or held; 2 Android refused Sioul the key plugged in; 3 another
    /// app holds it.
    fn sioul_android_key_held(max: *mut i32, extended: *mut i32, plugged: *mut i32) -> i32;
    /// One APDU: 0 and its answer (given back to `sioul_android_key_free`),
    /// 1 the key left the field, 2 another failure.
    fn sioul_android_key_transceive(command: *const u8, length: usize, answer: *mut *mut u8, answer_length: *mut usize) -> i32;
    /// An answer of `sioul_android_key_transceive`, wiped and given back.
    fn sioul_android_key_free(answer: *mut u8, length: usize);
    /// A key wanted now (1), or no longer (0).
    fn sioul_android_key_want(wanted: i32);
    /// Android's NFC settings, opened.
    fn sioul_android_key_settings();
}

/// Java's word that a key came: the band waiting wakes.
#[unsafe(no_mangle)]
pub extern "C" fn sioul_key_came() {
    if let Ok(mut came) = LINK.came.lock() {
        *came = true;
    }
    LINK.woken.notify_all();
}

/// "Open NFC settings".
pub(crate) fn open_settings() {
    // SAFETY: no arguments; Java opens the settings on its main thread.
    unsafe { sioul_android_key_settings() }
}

impl Link for AndroidLink {
    fn state(&self) -> LinkState {
        // SAFETY: no arguments.
        match unsafe { sioul_android_key_state() } {
            2 => LinkState::Ready,
            1 => LinkState::Off,
            _ => LinkState::NoNfc,
        }
    }

    fn usb_host(&self) -> bool {
        // SAFETY: no arguments.
        unsafe { sioul_android_key_usb() == 1 }
    }

    fn reach(&self) -> Reach {
        let (mut max, mut extended, mut plugged) = (0i32, 0i32, 0i32);
        // SAFETY: three integers for the call to write.
        match unsafe { sioul_android_key_held(&raw mut max, &raw mut extended, &raw mut plugged) } {
            1 => Reach::Held(Held { max_transceive: usize::try_from(max).unwrap_or(0), extended: extended == 1, via: if plugged == 1 { Via::Usb } else { Via::Nfc } }),
            2 => Reach::Refused,
            3 => Reach::Busy,
            _ => Reach::None,
        }
    }

    fn transceive(&self, command: &[u8]) -> Result<Vec<u8>, LinkError> {
        let mut answer: *mut u8 = std::ptr::null_mut();
        let mut length = 0usize;
        // SAFETY: the command's bytes, valid for the call; two places for the answer.
        let status = unsafe { sioul_android_key_transceive(command.as_ptr(), command.len(), &raw mut answer, &raw mut length) };
        match status {
            0 if !answer.is_null() => {
                // SAFETY: `length` bytes at `answer`, given by the call, read before they are given back.
                let bytes = unsafe { std::slice::from_raw_parts(answer, length) }.to_vec();
                // SAFETY: given back once, as the call asks; not read after.
                unsafe { sioul_android_key_free(answer, length) };
                Ok(bytes)
            }
            1 => Err(LinkError::Lost),
            other => Err(LinkError::Other(format!("NFC or USB failure {other}"))),
        }
    }

    fn wait(&self, timeout: Duration) {
        let Ok(came) = self.came.lock() else { return };
        let Ok((mut came, _)) = self.woken.wait_timeout_while(came, timeout, |came| !*came) else { return };
        *came = false;
    }

    fn want(&self, wanted: bool) {
        // SAFETY: an integer.
        unsafe { sioul_android_key_want(i32::from(wanted)) }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! This device's kind for what follows the person, a phone, a tablet or a
//! computer (`sioul_core::device_kind`, docs/android.md, "Tablets"), with
//! what Android says of itself: its smallest screen width, and whether it has
//! telephony at all (StepService.java's "device").
//!
//! - **The kind** (`kind`): your word in Settings ▸ This device, else the
//!   guess: Android under 600 dp a phone, from 600 dp a tablet, any other
//!   system a computer. The window reads it as `sioul.deviceKind`.
//! - **Telephony** (`telephony`): the system's, never the kind's. A tablet
//!   with a SIM has calls and texts; a tablet without one has neither, and
//!   the parts for them are not offered there, as on a computer.
//!
//! Neither decides the layout, which follows the window's size on every
//! system (`window.compact` in main.qml), nor the doses, which read the
//! system's word in the devices' entries (`devices::kind`).

use sioul_core::device_kind::DeviceKind;
use std::sync::Mutex;

/// What Android said of this device, kept once it answered: a process asks
/// once. Not kept while Java says nothing (its side not ready yet), so that
/// a later question asks again.
static SAID: Mutex<Option<serde_json::Value>> = Mutex::new(None);

/// Android's answer to "device" ({smallest_dp, telephony}); Null elsewhere or unanswered.
fn android_said() -> serde_json::Value {
    if !cfg!(target_os = "android") {
        return serde_json::Value::Null;
    }
    let mut said = SAID.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if said.is_none() {
        let answer = crate::steps::java("device", "{}");
        if answer.is_object() {
            *said = Some(answer);
        }
    }
    said.clone().unwrap_or(serde_json::Value::Null)
}

/// Android's smallest screen width in density-independent pixels; none elsewhere, or unanswered.
fn smallest_dp() -> Option<u32> {
    android_said()["smallest_dp"].as_u64().and_then(|dp| u32::try_from(dp).ok())
}

/// `SIOUL_DEVICE_KIND`, when it names a kind: the runner's pictures of a
/// tablet (tools/demo/run.sh … tablet) and the tests. Nothing else sets it.
fn forced() -> Option<DeviceKind> {
    std::env::var("SIOUL_DEVICE_KIND").ok().and_then(|word| DeviceKind::read(&word))
}

/// What the system says by itself, your word aside.
pub(crate) fn guessed() -> DeviceKind {
    DeviceKind::guessed(cfg!(target_os = "android"), smallest_dp())
}

/// This device's kind: your word (Settings ▸ This device), else the guess.
pub(crate) fn kind() -> DeviceKind {
    forced().unwrap_or_else(|| DeviceKind::of(crate::backend::load_config().device_kind.as_deref(), cfg!(target_os = "android"), smallest_dp()))
}

/// What this device is, said to every translator of the process, so that the
/// sentences naming it say "this tablet" (`sioul_core::i18n::set_form`):
/// again when Settings ▸ This device changes it.
pub(crate) fn name_in_words() {
    sioul_core::i18n::set_form(kind().word());
}

/// `name_in_words`, the first time a sentence is asked for in this process
/// (`backend::tr`), the background service's too. Never in this crate's
/// tests, whose sentences stay the phone's defaults whatever runs them.
pub(crate) fn name_in_words_once() {
    static NAMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if cfg!(test) || NAMED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    name_in_words();
}

/// Whether this device has calls and texts: Android with telephony (a
/// phone, a tablet with a SIM). Android's answer missing, yes, as every
/// Android was until it could say; never elsewhere.
pub(crate) fn telephony() -> bool {
    telephony_of(cfg!(target_os = "android"), &android_said())
}

/// `telephony`, from the system and what Android said.
fn telephony_of(android: bool, said: &serde_json::Value) -> bool {
    android && said["telephony"] != false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Telephony is the system's answer, never the kind's: Android without
    /// it (a tablet without a SIM) has no calls nor texts; Android silent
    /// keeps them, as before; a computer never has them.
    #[test]
    fn telephony_is_what_android_says() {
        assert!(telephony_of(true, &serde_json::json!({ "smallest_dp": 800, "telephony": true })), "a tablet with a SIM");
        assert!(!telephony_of(true, &serde_json::json!({ "smallest_dp": 800, "telephony": false })), "a tablet without one");
        assert!(telephony_of(true, &serde_json::Value::Null), "Java silent: as every Android was");
        assert!(!telephony_of(false, &serde_json::json!({ "telephony": true })), "a computer never");
        assert!(!telephony(), "this computer");
    }

    /// On a computer the kind is a computer unless you say otherwise, and
    /// the runner's tablet pictures say theirs (`SIOUL_DEVICE_KIND`).
    #[test]
    fn a_computer_is_a_computer_unless_said() {
        assert_eq!(guessed(), DeviceKind::Computer);
        assert_eq!(DeviceKind::of(None, cfg!(target_os = "android"), smallest_dp()), DeviceKind::Computer);
        assert_eq!(DeviceKind::of(Some("tablet"), cfg!(target_os = "android"), smallest_dp()), DeviceKind::Tablet);
        assert_eq!(DeviceKind::read("tablet"), Some(DeviceKind::Tablet), "what SIOUL_DEVICE_KIND=tablet reads as");
    }
}

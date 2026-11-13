// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The notices of this computer's window (docs/notifications.md): every
//! source makes one call, `tell`, about one thing (`sioul_core::notice`),
//! and `settle` takes its notice away once the thing is settled. Here:
//!
//! - **The moment** as a notice sees it (`moment`): someone at this computer
//!   (input in the last ten minutes and the session unlocked, as its entry in
//!   the sharing says: `devices::someone_here`), Sioul's pause, a
//!   do-not-disturb holding (Sioul's modes, and the desktop's own, which
//!   Sioul follows: `everywhere::now`).
//! - **The decision** (`sioul_core::notice::decide`), with the lifetime set
//!   for informative notices (`[notices]`, Settings ▸ What reaches you).
//! - **The posting**: `sioul_sync::notify::Teller`, one thread and one
//!   connection for every notice of the process; on Windows, a notice with
//!   something to open goes through Sioul's icon in the tray instead, whose
//!   message a click opens (main.qml's `onTrayNotice`).
//! - **The press**: a click on a notice or its "Open" opens its thing in the
//!   window through one route (`reminder_opened` with the address, main.qml's
//!   `open`), the desktop's activation token given to Qt first so that the
//!   window comes forward; a Copy puts a code on the clipboard; a verb of the
//!   window's own runs (`verb`).
//! - **A new run**: the notices the previous one left are taken away, and
//!   what is still to do among them is told again by its source (`start`).
//!
//! A phone posts none here: its notices are Java's (docs/android.md). The
//! doses keep their own path (`sioul_sync::notify::remind_dose`), untouched.

use crate::backend::{QtThread, load_config, tr};
use cxx_qt_lib::QString;
use sioul_core::attention::{Event, Kind as Row, Output};
use sioul_core::notice::{self, Act, Decision, Moment, Notice};
use std::sync::{Arc, Mutex, PoisonError};

/// Whether a notice's thing is settled now, looked at while it shows.
pub(crate) type Settled = sioul_sync::notify::Settled;

/// The window, once it started: a notice's press reaches it.
static WINDOW: Mutex<Option<QtThread>> = Mutex::new(None);

fn window() -> Option<QtThread> {
    WINDOW.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Where what shows is kept between two runs (`Teller::restart`): the server's
/// numbers and what each notice was about, never its words.
fn shown_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("notices-shown.json")
}

/// At the window's start: its presses heard, and the notices the previous
/// run left taken away (a notice of a run gone opens nothing), what is still
/// to do among them told again (a code not expired, an event's alarm before
/// its event).
pub(crate) fn start(qt: &QtThread) {
    if cfg!(target_os = "android") {
        return;
    }
    *WINDOW.lock().unwrap_or_else(PoisonError::into_inner) = Some(qt.clone());
    let teller = sioul_sync::notify::teller();
    let heard = qt.clone();
    teller.listen(Arc::new(move |press| pressed(&heard, press)));
    std::thread::spawn(move || {
        let left = teller.restart(&shown_path(), jiff::Timestamp::now().as_second());
        let codes: Vec<std::path::PathBuf> = left.iter().filter(|s| s.kind == notice::Kind::Own(Row::Codes) && !s.hint.is_empty()).map(|s| std::path::PathBuf::from(&s.hint)).collect();
        if !codes.is_empty() {
            crate::backend::retell_codes(&codes);
        }
        let alarms: Vec<String> = left.iter().filter(|s| s.kind == notice::Kind::Own(Row::Alarms)).map(|s| s.about.clone()).collect();
        if !alarms.is_empty() {
            crate::remind::retell(&alarms);
        }
    });
}

/// What the matrix says now of a kind of Sioul's own.
pub(crate) fn own(kind: Row) -> Output {
    crate::hours::attention().decide(&Event::own(kind), &crate::hours::attention_now())
}

/// The moment, as a notice sees it on this computer.
pub(crate) fn moment() -> Moment {
    Moment { present: crate::devices::someone_here(), paused: crate::hours::mode_now().paused(), silenced: crate::everywhere::now().on() }
}

/// Tells `notice` as the matrix says now: of its kind's row for one of
/// Sioul's own; as its source decided for new mail and the minor notices.
/// Whether it was posted.
pub(crate) fn tell(notice: Notice) -> bool {
    let out = match notice.kind {
        notice::Kind::Own(kind) => own(kind),
        notice::Kind::Mail | notice::Kind::Minor => notice::told(None),
    };
    tell_as(notice, out, None)
}

/// Tells `notice` as `out` says (`sioul_core::notice::decide`), taken away
/// once `settled` says its thing is settled (not posted when it is already).
/// Whether it was posted. Nothing on a phone.
pub(crate) fn tell_as(notice: Notice, out: Output, mut settled: Option<Settled>) -> bool {
    if cfg!(target_os = "android") {
        return false;
    }
    if settled.as_mut().is_some_and(|look| look()) {
        return false;
    }
    let shape = match notice::decide(&notice, &out, moment(), load_config().notices.minutes(), jiff::Timestamp::now().as_second()) {
        Decision::Post(shape) => shape,
        Decision::Drop(_) => return false,
    };
    if cfg!(windows)
        && !notice.about.is_empty()
        && let Some(qt) = window()
    {
        let (title, body, address, hint) = (notice.title, notice.body, notice.about, notice.hint);
        let _ = qt.queue(move |mut sioul| sioul.as_mut().tray_notice(QString::from(&title), QString::from(&body), QString::from(&address), QString::from(&hint)));
        return true;
    }
    sioul_sync::notify::teller().post(notice, shape, &tr().text("reminder-open", None), settled);
    true
}

/// Every notice about `about` taken away: its thing is settled (read, done,
/// withdrawn by its site).
pub(crate) fn settle(about: &str) {
    if cfg!(target_os = "android") {
        return;
    }
    sioul_sync::notify::teller().settle(about);
}

/// A press on a notice, on the teller's thread: its thing opened in the
/// window, the desktop's token given first; a code copied; a verb run.
fn pressed(qt: &QtThread, press: sioul_sync::notify::Press) {
    match press.act {
        Act::Open { address, hint } => {
            let token = press.token;
            let _ = qt.queue(move |mut sioul| {
                if let Some(token) = token {
                    given(&token);
                }
                sioul.as_mut().reminder_opened(QString::default(), QString::from(&address), QString::from(&hint));
            });
        }
        Act::Copy(text) => {
            let _ = qt.queue(move |sioul| sioul.copy_requested(QString::from(&text)));
        }
        Act::Verb(word) => verb(qt, &word),
    }
}

/// The desktop's activation token, given to Qt on the window's thread: Qt's
/// Wayland client hands it to the compositor at the next activation
/// (`bringBack`'s `requestActivate`), which then lets the window come forward.
fn given(token: &str) {
    #[cfg(not(target_os = "android"))]
    if let Ok(token) = std::ffi::CString::new(token) {
        // SAFETY: `token` is a C string alive for the call, which copies it;
        // on the window's thread, where Qt reads the variable (qputenv takes
        // Qt's lock on the environment).
        unsafe { crate::sioul_set_activation_token(token.as_ptr()) };
    }
    #[cfg(target_os = "android")]
    let _ = token;
}

/// A verb of the window's own, pressed on a notice: "read-language:<code>",
/// mail in that language read too (`languages`).
fn verb(qt: &QtThread, word: &str) {
    if let Some(code) = word.strip_prefix("read-language:") {
        crate::languages::read_also(qt, code);
    }
}

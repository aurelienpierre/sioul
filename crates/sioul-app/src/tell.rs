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
//! - **What waits**: a notice that cannot come now (nobody at the computer,
//!   a pause, a do-not-disturb) is never lost nor marked told: kept here, and
//!   told again at the window's minute (`again`) while it is still worth
//!   telling (`Notice::relevance`, the first answer of which is kept by its
//!   key: a reminder offered again each minute keeps the end it had); its
//!   source's mark (`Claim`: a reminder's claim, the end of work's day told,
//!   the sites' news named) made only once it is posted.
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
use sioul_core::notice::{self, Act, Decision, Dropped, Moment, Notice};
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
    let now = sioul_core::zone::now();
    Moment { present: crate::devices::someone_here(), paused: crate::hours::mode_now().paused(), silenced: crate::everywhere::now().on(), day_ends: Moment::end_of_day(&now) }
}

/// What became of a notice told (`tell_with`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fate {
    /// Posted (to the teller, or to the tray's icon), its source's mark made.
    Posted,
    /// Not now: kept, told again once someone is back or the do-not-disturb
    /// ends, while still worth telling; nothing marked.
    Waits,
    /// No longer worth telling (its thing ended, or its relevance): its
    /// source may mark it handled, so that it does not offer it again.
    Over,
    /// Not told: the model holds it, its thing is settled already, or it was
    /// told elsewhere meanwhile (its claim refused).
    Dropped,
}

/// The mark a source makes just before its notice is posted, now or once it
/// stopped waiting (a reminder's claim, the end of work's day told, the
/// sites' news named): false when it was told elsewhere meanwhile, and then
/// it is not posted.
pub(crate) type Claim = Box<dyn FnOnce() -> bool + Send>;

/// A notice waiting to be told.
struct Waiting {
    notice: Notice,
    out: Output,
    settled: Option<Settled>,
    claim: Option<Claim>,
}

/// The notices waiting (`Fate::Waits`), one per key, and the end of
/// relevance each key had when it first waited.
static WAITING: Mutex<Vec<Waiting>> = Mutex::new(Vec::new());
static RELEVANT: Mutex<Vec<(String, i64)>> = Mutex::new(Vec::new());

/// Tells `notice` as the matrix says now: of its kind's row for one of
/// Sioul's own; as its source decided for new mail and the minor notices.
pub(crate) fn tell(notice: Notice) -> Fate {
    let out = match notice.kind {
        notice::Kind::Own(kind) => own(kind),
        notice::Kind::Mail | notice::Kind::Minor => notice::told(None),
    };
    tell_as(notice, out, None)
}

/// Tells `notice` as `out` says, kept while it cannot come now; taken away
/// once `settled` says its thing is settled (not posted when it is already).
pub(crate) fn tell_as(notice: Notice, out: Output, settled: Option<Settled>) -> Fate {
    tell_with(notice, out, settled, None, true)
}

/// Tells `notice` as `out` says (`sioul_core::notice::decide`): posted now,
/// `claim` made just before; or, when it cannot come now, kept here and told
/// again at the window's minute while it is worth telling (`keep`; without
/// it, its source offers it again itself, as the reminders do each minute,
/// the end of its relevance kept by its key all the same). Nothing on a phone.
pub(crate) fn tell_with(mut notice: Notice, out: Output, mut settled: Option<Settled>, claim: Option<Claim>, keep: bool) -> Fate {
    if cfg!(target_os = "android") {
        return Fate::Dropped;
    }
    let key = notice.key();
    if settled.as_mut().is_some_and(|look| look()) {
        forget(&key);
        return Fate::Dropped;
    }
    let now = jiff::Timestamp::now().as_second();
    if notice.relevant.is_none() {
        notice.relevant = RELEVANT.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(k, _)| *k == key).map(|(_, at)| *at);
    }
    match notice::decide(&notice, &out, moment(), load_config().notices.minutes(), now) {
        Decision::Post(shape) => {
            forget(&key);
            if let Some(claim) = claim
                && !claim()
            {
                return Fate::Dropped;
            }
            post(notice, shape, settled);
            Fate::Posted
        }
        Decision::Wait(relevant) => {
            {
                let mut first = RELEVANT.lock().unwrap_or_else(PoisonError::into_inner);
                first.retain(|(_, at)| *at > now);
                if !first.iter().any(|(k, _)| *k == key) {
                    first.push((key.clone(), relevant));
                }
            }
            if keep {
                let mut waiting = WAITING.lock().unwrap_or_else(PoisonError::into_inner);
                waiting.retain(|w| w.notice.key() != key);
                waiting.push(Waiting { notice, out, settled, claim });
            }
            Fate::Waits
        }
        Decision::Drop(Dropped::NotTold | Dropped::NotOurs) => {
            forget(&key);
            Fate::Dropped
        }
        Decision::Drop(_) => {
            forget(&key);
            Fate::Over
        }
    }
}

/// A key no longer waiting.
fn forget(key: &str) {
    WAITING.lock().unwrap_or_else(PoisonError::into_inner).retain(|w| w.notice.key() != key);
    RELEVANT.lock().unwrap_or_else(PoisonError::into_inner).retain(|(k, _)| k != key);
}

/// At the window's minute: what waits told again, as the moment now lets it
/// (someone back, the do-not-disturb over), or kept, or gone once no longer
/// worth telling. Off the window's thread.
pub(crate) fn again() {
    if cfg!(target_os = "android") {
        return;
    }
    let waiting = std::mem::take(&mut *WAITING.lock().unwrap_or_else(PoisonError::into_inner));
    if waiting.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        for w in waiting {
            tell_with(w.notice, w.out, w.settled, w.claim, true);
        }
    });
}

/// The notices handed to the tray's icon on Windows, their shape kept: where
/// the tray shows no message, the window asks them posted as they were
/// decided (`plain`), never decided again.
static TRAYED: Mutex<Vec<(Notice, notice::Shape)>> = Mutex::new(Vec::new());

/// A notice decided, posted: on Windows through Sioul's icon in the tray when
/// it has a thing to open (a click on its message opens it), else through
/// the teller.
fn post(notice: Notice, shape: notice::Shape, settled: Option<Settled>) {
    if cfg!(windows)
        && !notice.about.is_empty()
        && let Some(qt) = window()
    {
        let (title, body, address, hint) = (notice.title.clone(), notice.body.clone(), notice.about.clone(), notice.hint.clone());
        {
            let mut trayed = TRAYED.lock().unwrap_or_else(PoisonError::into_inner);
            trayed.push((notice, shape));
            let excess = trayed.len().saturating_sub(16);
            trayed.drain(..excess);
        }
        let _ = qt.queue(move |mut sioul| sioul.as_mut().tray_notice(QString::from(&title), QString::from(&body), QString::from(&address), QString::from(&hint)));
        return;
    }
    sioul_sync::notify::teller().post(notice, shape, &tr().text("reminder-open", None), settled);
}

/// The tray shows no message (Windows): the notice it was given with these
/// words posted through the teller as it was decided; words of none, a minor
/// notice of their own, about nothing.
pub(crate) fn plain(title: &str, body: &str) {
    let kept = {
        let mut trayed = TRAYED.lock().unwrap_or_else(PoisonError::into_inner);
        trayed.iter().rposition(|(n, _)| n.title == title && n.body == body).map(|at| trayed.remove(at))
    };
    match kept {
        Some((notice, shape)) => sioul_sync::notify::teller().post(notice, shape, &tr().text("reminder-open", None), None),
        None => {
            tell(Notice::new("", notice::Kind::Minor, title, body));
        }
    }
}

/// Every notice about `about` taken away: its thing is settled (read, done,
/// withdrawn by its site).
pub(crate) fn settle(about: &str) {
    if cfg!(target_os = "android") {
        return;
    }
    WAITING.lock().unwrap_or_else(PoisonError::into_inner).retain(|w| w.notice.about != about);
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

/// A verb of the window's own, pressed on a notice: `"read-language:<code>"`,
/// mail in that language read too (`languages`).
fn verb(qt: &QtThread, word: &str) {
    if let Some(code) = word.strip_prefix("read-language:") {
        crate::languages::read_also(qt, code);
    }
}

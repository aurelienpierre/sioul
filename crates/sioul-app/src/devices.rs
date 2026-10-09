// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! This device's sessions, as your other devices learn them through the
//! sharing folder (docs/database.md, "Devices"; `sioul_sync::devices`): its
//! start, its clean close, and between them `working`, up.
//!
//! - On a computer, a session is the window's life: up when Sioul starts,
//!   down as the last step of a clean close (the window closed, or the stop
//!   signal: cpp/signals.cpp quits Qt), after the final export (`share::closing`).
//! - On a phone, a session is the time on the screen: up when Sioul comes
//!   back, down when it goes to the background, after its export (Android may
//!   end a process in the background without a word, so it goes down then).
//! - A reminder handled while no session is open here (a dose's alarm,
//!   "Taken" pressed on its notification) is a session of its own: up, what
//!   it records, its export, down (`receiver`). Only when the last one ends,
//!   the window's or a receiver's, does it go down.
//! - Each exchange says what it read and wrote (`exported`), never touching
//!   the session: a phone's background service exporting in its own process
//!   never undoes the window's.
//!
//! A crash leaves it up: your other devices then say they cannot tell what
//! was marked here, until it starts again or you say it is off.

use sioul_sync::devices::{COMPUTER, Entry, PHONE};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};

/// The sessions open in this process: the window's, and receivers at work.
struct Sessions {
    window: bool,
    receivers: u32,
}

static SESSIONS: Mutex<Sessions> = Mutex::new(Sessions { window: false, receivers: 0 });

/// When this process last recorded something of the doses (a mark, a record,
/// a reminder), and when its last export read the files (milliseconds, this
/// device's clock). A session goes down only once an export read the files
/// after the last of them: else an answer captured here may not have gone
/// out, and saying "closed" would make the others sure of what they lack.
static RECORDED: AtomicI64 = AtomicI64::new(0);
static EXPORTED: AtomicI64 = AtomicI64::new(0);

/// Something of the doses recorded here (`health::change`, `change_records`):
/// owed to the others until an export reads it.
pub(crate) fn recorded() {
    RECORDED.fetch_max(jiff::Timestamp::now().as_millisecond(), Ordering::SeqCst);
}

/// Whether everything recorded here went out with an export since.
fn all_out() -> bool {
    went_out(RECORDED.load(Ordering::SeqCst), EXPORTED.load(Ordering::SeqCst))
}

/// Whether what was recorded (`RECORDED`) went out with an export since
/// (`EXPORTED`): both 0 in a process just started, which knows nothing of an
/// earlier one's.
fn went_out(recorded: i64, exported: i64) -> bool {
    recorded < exported || recorded == 0
}

fn sessions() -> std::sync::MutexGuard<'static, Sessions> {
    SESSIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// This device's name, to say where: a computer's host name; a phone's
/// model (its host name is "localhost"), said "the phone" by the others.
pub(crate) fn name() -> String {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        if cfg!(target_os = "android") {
            let model = std::process::Command::new("/system/bin/getprop").arg("ro.product.model").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            model.filter(|m| !m.is_empty()).unwrap_or_else(|| PHONE.to_string())
        } else {
            sioul_sync::lease::host_name()
        }
    })
    .clone()
}

/// A phone, or a computer.
pub(crate) fn kind() -> &'static str {
    if cfg!(target_os = "android") { PHONE } else { COMPUTER }
}

/// This device's entry changed by `change`, its identity set: written here,
/// and into the folder when sharing is on. Never fails loudly: the doses of
/// the others say what they cannot know when it is not written.
fn write(change: impl FnOnce(&mut Entry)) {
    let Some((id, vault)) = crate::share::vault() else { return };
    let path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    // Its doses said known only while their part travels: held under a newer
    // format (`share::holds`), they are not, whatever this process recorded.
    let doses = doses_said(crate::share::shares("health"), crate::share::holds_doses());
    // A phone's notification access, as Android says it now: what it may hold of other apps.
    let notifications = crate::appnotes::access_here();
    let written = sioul_sync::devices::change(&path, vault.as_ref().map(|(folder, key)| (folder.as_path(), key)), |entry| {
        entry.id = id;
        entry.name = name();
        entry.kind = kind().into();
        // The build that writes it (docs/database.md, "Devices"): which code each device runs.
        entry.version = sioul_core::build::VERSION.into();
        entry.commit = sioul_core::build::COMMIT.into();
        entry.format = sioul_sync::share::FORMAT;
        entry.doses = doses;
        if notifications.is_some() {
            entry.notifications = notifications;
        }
        change(entry);
    });
    if let Err(e) = written {
        eprintln!("sioul: devices: {e}");
    }
}

/// The window opened, or a phone's Sioul back on the screen: up. Found up
/// already (the last session ended without closing: a crash), what it left
/// is owed an export before the next close.
pub(crate) fn window_opened() {
    let mut open = sessions();
    open.window = true;
    raise();
}

/// Up, said; a session found up already owes an export.
fn raise() {
    write(|entry| {
        if entry.working {
            recorded();
        }
        entry.start(now());
    });
}

/// Down, said, the last step of a session, once all it recorded went out;
/// else it stays up, and the others say what they cannot know until the
/// next session's export.
fn lower() -> bool {
    if !may_lower(all_out(), crate::share::holds_doses()) {
        eprintln!("sioul: devices: what was recorded here has not gone out; this device stays up");
        return false;
    }
    write(|entry| entry.close(now()));
    true
}

/// Whether this device's entry may say it shares its doses: it shares the
/// health part, and that part is not held here under a newer format (its
/// answers then wait unsent, and the others must not count them as known).
fn doses_said(shares: bool, held: bool) -> bool {
    shares && !held
}

/// Whether a session may go down (closed: everything it captured went out):
/// all it recorded went out, and the doses' part is not held here, which
/// this process's own counts cannot tell after a restart.
fn may_lower(all_out: bool, held: bool) -> bool {
    all_out && !held
}

/// The window's session ended, its final export done (`share::closing`):
/// down, unless a receiver is still at work, or `ended` says it is not over
/// after all (a phone's Sioul brought back meanwhile).
pub(crate) fn window_closed(ended: impl Fn() -> bool) {
    let mut open = sessions();
    if !ended() {
        return;
    }
    open.window = false;
    if open.receivers == 0 {
        lower();
    }
}

/// `work` (what a reminder records, then its export) as a session of its own
/// when none is open here: up first, so that a process ended in its midst
/// leaves the others in doubt rather than sure; down after, then the sync app
/// asked to carry it (`share::nudge_carriers`): the lowered entry goes up,
/// not the raised one.
pub(crate) fn receiver<T>(work: impl FnOnce() -> T) -> T {
    {
        let mut open = sessions();
        if !open.window && open.receivers == 0 {
            raise();
        }
        open.receivers += 1;
    }
    let out = work();
    let lowered = {
        let mut open = sessions();
        open.receivers = open.receivers.saturating_sub(1);
        !open.window && open.receivers == 0 && lower()
    };
    if lowered {
        crate::share::nudge_carriers();
    }
    out
}

/// Whether a reminder's own session is at work in this process (no window open):
/// the sync app is then asked once it is down, not by each exchange inside it.
pub(crate) fn in_receiver() -> bool {
    let open = sessions();
    !open.window && open.receivers > 0
}

/// An exchange done: what it read and wrote, said; the session as it is.
/// The first export of a session is carried at once (a phone's sync app
/// asked to look): the others see this device in use with fresh news, not
/// with what it last shared before it was put away.
pub(crate) fn exported(outcome: &sioul_sync::share::Outcome) {
    if outcome.looked <= 0 {
        return;
    }
    EXPORTED.fetch_max(outcome.looked, Ordering::SeqCst);
    let first = std::sync::atomic::AtomicBool::new(false);
    write(|entry| {
        first.store(entry.working && entry.exported < entry.started, std::sync::atomic::Ordering::Relaxed);
        entry.exported(outcome.looked, outcome.wrote, now());
    });
    if first.load(std::sync::atomic::Ordering::Relaxed) && !in_receiver() {
        crate::share::nudge_carriers();
    }
}

/// Sharing stopped here: down, and said to count no more.
pub(crate) fn left() {
    write(|entry| {
        entry.close(now());
        entry.left = true;
    });
}

/// Whether another device says it is in use, with an export dated lately
/// (`FRESH` and `SKEW`): for the rhythm of a phone's background service only;
/// the doses' knowing is `health::know`'s.
pub(crate) fn others_in_use() -> bool {
    let Some((id, Some((folder, key)))) = crate::share::vault() else { return false };
    let now = now();
    sioul_sync::devices::all(&folder, &key).0.iter().any(|e| e.id != id && e.working && !e.left && now - e.exported <= sioul_core::health::FRESH + sioul_core::health::SKEW)
}

/// A device's kind ("phone", "computer") and name, from its entry in the
/// folder; none when it has none (an older Sioul, or not arrived yet).
pub(crate) fn kind_and_name(id: &str) -> Option<(String, String)> {
    let (_, Some((folder, key))) = crate::share::vault()? else { return None };
    sioul_sync::devices::all(&folder, &key).0.into_iter().find(|e| e.id == id).map(|e| (e.kind, e.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A process started anew (an app restart; on Android, the background
    /// process killed and started again) knows nothing of what an earlier one
    /// recorded: its counts say all went out. While the doses' part is held
    /// under a newer format (`share::holds`, read from the disk), its entry
    /// still says its doses are not known, and its session never goes down as
    /// if they were: the others doubt, never take a dose as not taken
    /// (docs/health.md, "A newer Sioul's doses").
    #[test]
    fn a_held_doses_part_claims_no_knowledge_after_a_restart() {
        // The statics as a new process has them: nothing recorded, nothing exported.
        let (recorded, exported) = (0, 0);
        assert!(went_out(recorded, exported), "a new process: all went out, by its own counts");
        assert!(!may_lower(went_out(recorded, exported), true), "held: never down as if all went out");
        assert!(!doses_said(true, true), "held: its doses said not shared");
        assert!(may_lower(went_out(recorded, exported), false) && doses_said(true, false), "not held: as before");
        assert!(!may_lower(went_out(5, 3), false), "an answer recorded and not exported keeps it up, as before");
    }
}

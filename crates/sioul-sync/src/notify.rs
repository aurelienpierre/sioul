// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The one exception to the admin windows: a quiet desktop notification for a
//! verified code, with the code in it (docs/porch.md, "Right now"); a gentle
//! reminder; new mail at the times it may come, once per batch
//! (docs/porch.md, "Notifications"); and the time running, while a focus
//! session runs (docs/tasks.md).
//!
//! On Linux and the BSDs each notification goes to the session's
//! notification server over a D-Bus connection of its own, made by
//! `crate::bus`: a session bus that takes it and never answers holds one
//! thread of its own, never the caller nor libdbus's process-wide locks
//! (review of battery part C, G1). Its call is the one notify-rust's
//! `Notification` made before 13 October 2026, argument for argument
//! (`notify_message`), but for its urgency, said once (notify-rust's hints
//! are a set, where a mail's normal urgency and the critical one it started
//! from were two, sent in no fixed order). Windows and macOS: notify-rust.
//!
//! A dose's reminder (`remind_dose`) is built on one rule: never invite a
//! second dose, and when this device cannot know, say the doubt (docs/health.md,
//! "Knowing"). What became of its first try is said in three ways, recorded
//! as such: shown (the server answered while the tick waited), not shown
//! (surely not gone out), or maybe (sent, its fate not known), which a
//! phone's alarm then reminds with the doubt said first. Whatever shows late
//! on this computer (a try after the first, or the first answered late) says
//! the doubt first; tries after the first end before the phone's alarm could
//! ring.

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
use dbus::blocking::LocalConnection;
#[cfg(any(windows, target_os = "macos"))]
use notify_rust::{Notification, Timeout};
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
use std::cell::RefCell;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
use std::rc::Rc;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
use std::time::Instant;

/// How long the notification stays: about the life of a code.
#[cfg(not(target_os = "android"))]
const SHOWN: u32 = 10 * 60 * 1000;

/// How long the tick waits for what became of a dose reminder's first try
/// (`Sending::outcome`): its whole first try fits in it (`TRY`), the
/// connection's setup, its two matches, the server's two answers.
pub const FIRST_TRY: Duration = Duration::from_secs(10);

/// What a dose reminder's first try became, as the tick records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The notification server answered the Notify while the tick waited: shown.
    Shown,
    /// It surely did not go out (no connection, no notification server, the
    /// bus or the server said no, nothing sent in time): why.
    Unshown(String),
    /// Sent, and what became of it is not known (no answer in time, the bus
    /// stalled under it, the tick done waiting): it may be on the screen. Why.
    Maybe(String),
    /// Not sent: no longer to remind when it was about to go (marked meanwhile).
    Gone,
}

/// What a dose's reminder records, and asks, as it goes (docs/health.md,
/// "What became of a reminder"), so that another device knows what this one
/// did before it reminds the dose itself: each notice recorded before it
/// goes (`going`), what became of it after. Its own device's entry alone
/// (`sioul_core::dose_reminders`).
pub trait DoseRecord: Send {
    /// A notice of the dose is about to go: recorded so first ("maybe": it
    /// may be on the screen from now on), then its words now. `Say::Nothing`:
    /// no longer to remind (marked, answered or reminded anywhere read here,
    /// the doses kept elsewhere now): nothing sent, nothing more tried.
    fn going(&mut self, going: Going) -> Say;
    /// The notice just recorded as going was refused (the bus or the server
    /// said no, or it could not be sent): it did not show, taken back.
    fn refused(&mut self);
    /// One was shown: the server answered (late, or in time).
    fn shown(&mut self);
    /// The first try surely did not go out: nothing was sent.
    fn unshown(&mut self);
    /// The tries pause, a phone's alarm due meanwhile: promised, no notice
    /// until `until` (Unix seconds); false when no longer to remind (no more tries).
    fn pause(&mut self, until: i64) -> bool;
    /// No more tries (the grace's end): promised, no notice again.
    fn over(&mut self);
    /// Whether the dose was marked or answered on any device, as read here
    /// now: a notice of it still shown is taken away.
    fn settled(&mut self) -> bool;
}

/// Which notice is about to go (`DoseRecord::going`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Going {
    /// The first try, while the tick waits: its own words.
    First,
    /// Shown late, the doubt first: `Late::Retry` a try after the first, or
    /// the first once the tick stopped waiting; `Late::Unconfirmed` the first,
    /// on the screen since its time, answered late (said again in place).
    Late(Late),
}

/// Why a reminder shows late (`Going::Late`): its words differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Late {
    /// Its first try could not be shown in time.
    Retry,
    /// It was shown in time, but not confirmed in time.
    Unconfirmed,
}

/// What a notice about to go says (`DoseRecord::going`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Say {
    /// Its own words, as given to `remind_dose`.
    Own,
    /// These words: title and body.
    Words(String, String),
    /// Nothing: no longer to remind.
    Nothing,
}

/// What a dose's reminder does after its first try (`remind_dose`), Linux and
/// the BSDs; elsewhere the first try is the only one:
/// - not gone out: tried again on its own thread, the doubt first, five
///   seconds apart, then half a minute (`again_in`), until `end` (its
///   grace), then `DoseRecord::over`; during `quiet` (a phone's alarm rings
///   then), none: promised before it begins (`DoseRecord::pause`), resumed
///   after it while still to remind;
/// - sent and not answered in time: never sent again (it may be on the
///   screen), its answer still heard on its connection, however late, and
///   its button; answered, said again in place with the doubt first.
///
/// A notice shown is taken away once its dose is marked or answered anywhere
/// read here (`DoseRecord::settled`).
pub struct Again {
    pub end: std::time::Instant,
    /// From, to, and "to" in Unix seconds (the promise's words).
    pub quiet: Option<(std::time::Instant, std::time::Instant, i64)>,
    pub record: Box<dyn DoseRecord>,
}

/// A dose's reminder under way (`remind_dose`): what became of its first
/// try, said by its thread while the tick waits; once the tick stopped
/// waiting (`outcome` returned, or this dropped), what shows is late.
pub struct Sending(Arc<(Mutex<Slot>, Condvar)>);

/// Where a first try's outcome is left for the tick, and whether the tick
/// still waits for it.
#[derive(Default)]
struct Slot {
    outcome: Option<Outcome>,
    closed: bool,
}

impl Sending {
    fn new() -> Sending {
        Sending(Arc::default())
    }

    /// A first try whose outcome is known at once.
    #[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
    fn known(outcome: Outcome) -> Sending {
        let sending = Sending::new();
        let _ = told(&sending.0, outcome);
        sending
    }

    /// What the first try became, waited for `wait` at most: not said by
    /// then, `Maybe` (it may still show: late, the doubt said, `Again`). The
    /// tick waits no more after this.
    pub fn outcome(&self, wait: Duration) -> Outcome {
        let (slot, bell) = &*self.0;
        let Ok(guard) = slot.lock() else { return Outcome::Maybe("the notification's outcome could not be read".to_string()) };
        let Ok((mut guard, _)) = bell.wait_timeout_while(guard, wait, |slot| slot.outcome.is_none()) else { return Outcome::Maybe("the notification's outcome could not be read".to_string()) };
        guard.closed = true;
        guard.outcome.take().unwrap_or_else(|| Outcome::Maybe("the notification server did not answer in time".to_string()))
    }
}

impl Drop for Sending {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.0.0.lock() {
            slot.closed = true;
        }
    }
}

/// A first try's outcome left for the tick: whether the tick still waited
/// for it (else what shows now shows late).
fn told(sending: &Arc<(Mutex<Slot>, Condvar)>, outcome: Outcome) -> bool {
    let (slot, bell) = &**sending;
    let Ok(mut slot) = slot.lock() else { return false };
    if slot.closed {
        return false;
    }
    slot.outcome = Some(outcome);
    bell.notify_all();
    true
}

/// Sioul's bundle id on macOS, as `packaging/macos/Info.plist` names it
/// (`CFBundleIdentifier`). macOS shows a notification as coming from the
/// application whose id it is given; without one, notify-rust's macOS side
/// (mac-notification-sys) names another application.
pub const BUNDLE_ID: &str = "com.aurelienpierre.Sioul";

/// macOS: Sioul's notifications said to come from Sioul (`BUNDLE_ID`), once
/// per process, before the first is shown: macOS takes the application only
/// once.
#[cfg(target_os = "macos")]
fn as_sioul() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = notify_rust::set_application(BUNDLE_ID);
    });
}

/// Shows a code: `title` says what and from whom, `body` the code and how long
/// it lasts. No sound. With `copy`, the notification offers a button labelled
/// with its text, and its action runs when the button is pressed; the
/// notification then lives on its own thread, which waits for the button.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn code(title: &str, body: &str, copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    one_button(Notice::new(title, body, "dialog-password", CRITICAL), copy)
}

/// A gentle reminder: one notification, no sound, with one button whose
/// action runs when pressed (a dose's own: `remind_dose`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn remind(title: &str, body: &str, action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    one_button(Notice::new(title, body, "appointment-soon", CRITICAL), action)
}

/// A dose's reminder (docs/health.md, "Knowing"): what `remind` shows with
/// its button ("Taken"), on a thread of its own, whose first try fits in
/// `FIRST_TRY` and says what it became (`Sending::outcome`); then as `Again`
/// says. "Taken" runs once, however late it is pressed while the notice lives.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn remind_dose(title: &str, body: &str, button: (String, Box<dyn FnOnce() + Send>), again: Again) -> Sending {
    let mut notice = Notice::new(title, body, "appointment-soon", CRITICAL);
    let (label, run) = button;
    notice.buttons.push(("copy".to_string(), label));
    let pressed: Pressed = Box::new(move |key: &str| {
        if key == "copy" {
            run();
        }
    });
    let sending = Sending::new();
    let slot = Arc::clone(&sending.0);
    // The D-Bus connection cannot change threads, so it is made where it waits.
    std::thread::spawn(move || first_try(notice, again, pressed, &slot));
    sending
}

/// Elsewhere, the reminder as `remind` shows it, recorded before it goes,
/// its outcome known at once; no try after the first.
#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub fn remind_dose(title: &str, body: &str, button: (String, Box<dyn FnOnce() + Send>), mut again: Again) -> Sending {
    let (title, body) = match again.record.going(Going::First) {
        Say::Own => (title.to_string(), body.to_string()),
        Say::Words(title, body) => (title, body),
        Say::Nothing => return Sending::known(Outcome::Gone),
    };
    Sending::known(match remind(&title, &body, Some(button)) {
        Ok(()) => {
            again.record.shown();
            Outcome::Shown
        }
        Err(e) => {
            again.record.refused();
            again.record.unshown();
            Outcome::Unshown(e)
        }
    })
}

/// New mail at the times it may come (docs/porch.md, "Notifications"): one
/// notification for the batch, no sound, with one button ("Open": the
/// Porch) whose action runs when pressed. At normal urgency, unlike codes
/// and reminders: the desktop's own do-not-disturb holds it; but at critical
/// urgency when someone Always through is in it (`through`), which passes it
/// (docs/attention.md, Q10).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn mail(title: &str, body: &str, through: bool, action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    one_button(Notice::new(title, body, "mail-unread", if through { CRITICAL } else { NORMAL }), action)
}

/// Windows and macOS: the mail's words alone; their notifications carry no button here.
#[cfg(any(windows, target_os = "macos"))]
pub fn mail(title: &str, body: &str, _through: bool, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    as_sioul();
    Notification::new().appname("Sioul").summary(title).body(body).timeout(Timeout::Milliseconds(SHOWN)).show().map(drop).map_err(|e| e.to_string())
}

/// Android: made on the window's side, through Java (sioul-app's `eventalarms`).
#[cfg(target_os = "android")]
pub fn mail(_title: &str, _body: &str, _through: bool, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Err("notifications are made through Java on Android".to_string())
}

/// A gentle reminder with several buttons, each its label and its action:
/// the one pressed runs (the night's notice: "Close the day" and "Options…").
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn remind_choices(title: &str, body: &str, choices: Vec<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    if choices.is_empty() {
        return remind(title, body, None);
    }
    let mut notice = Notice::new(title, body, "appointment-soon", CRITICAL);
    let mut runs: Vec<Option<Box<dyn FnOnce() + Send>>> = Vec::new();
    for (index, (label, run)) in choices.into_iter().enumerate() {
        notice.buttons.push((format!("choice-{index}"), label));
        runs.push(Some(run));
    }
    notify(
        notice,
        Some(Box::new(move |key: &str| {
            if let Some(run) = key.strip_prefix("choice-").and_then(|n| n.parse::<usize>().ok()).and_then(|n| runs.get_mut(n)).and_then(Option::take) {
                run();
            }
        })),
    )
}

/// Elsewhere, as a reminder without buttons (`remind`).
#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub fn remind_choices(title: &str, body: &str, _choices: Vec<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    remind(title, body, None)
}

/// Windows and macOS: the reminder is in the text; no button here.
#[cfg(any(windows, target_os = "macos"))]
pub fn remind(title: &str, body: &str, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    as_sioul();
    Notification::new().appname("Sioul").summary(title).body(body).timeout(Timeout::Milliseconds(SHOWN)).show().map(drop).map_err(|e| e.to_string())
}

/// Windows and macOS: the code is in the text; their notifications carry no button here.
#[cfg(any(windows, target_os = "macos"))]
pub fn code(title: &str, body: &str, _copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    as_sioul();
    Notification::new().appname("Sioul").summary(title).body(body).timeout(Timeout::Milliseconds(SHOWN)).show().map(drop).map_err(|e| e.to_string())
}

/// Android: no notifications yet (Android's own are reached through Java);
/// the code shows in the window only.
#[cfg(target_os = "android")]
pub fn code(_title: &str, _body: &str, _copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Err("notifications are not made on Android yet".to_string())
}

/// Android: no notifications yet; a reminder is not shown (docs/android.md).
#[cfg(target_os = "android")]
pub fn remind(_title: &str, _body: &str, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Err("notifications are not made on Android yet".to_string())
}

/// The notification server, on the session bus.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const SERVER: &str = "org.freedesktop.Notifications";
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const SERVER_PATH: &str = "/org/freedesktop/Notifications";

/// Sioul's desktop file, named on each notification ("desktop-entry"): the
/// desktop knows them as Sioul's, and lists Sioul in its notification
/// settings (Plasma's "Show in do not disturb mode", for the doses during a
/// pause: docs/pauses.md).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const DESKTOP_ENTRY: &str = "com.aurelienpierre.Sioul";

/// How long a lasting notification's thread listens to the server before it
/// looks at what Sioul asked: a change shows within it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const LISTENED: Duration = Duration::from_millis(500);

/// A notification the server refused (one starting with the session, after
/// Sioul) is asked again this often.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const ASKED_AGAIN: Duration = Duration::from_secs(60);

/// How long the server may take to answer about a notification (notify-rust's own wait).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const ASKED: Duration = Duration::from_secs(2);

/// What a dose reminder's first try spends at most, its setup and all: the
/// tick's wait (`FIRST_TRY`) less half a second, for its word to reach the tick.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const TRY: Duration = Duration::from_millis(9_500);

/// How long each of a dose reminder's two matches (its button, its close,
/// heard on its connection) may take on its first try: the bus answers them
/// at once.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const MATCH: Duration = Duration::from_secs(1);

/// The same for a try after the first, which no tick waits for: a slow bus
/// (each answer seconds late, a machine swapping) still shows it, late, as
/// a connection of dbus-rs's own waits (five seconds a call).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const TRY_AGAIN: Duration = Duration::from_secs(15);

/// Each match's wait on a try after the first (`TRY_AGAIN`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const MATCH_AGAIN: Duration = Duration::from_secs(5);

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const _: () = assert!(
    TRY.as_millis() < FIRST_TRY.as_millis() && TRY.as_millis() > (2 * MATCH.as_millis() + ASKED.as_millis()) && TRY_AGAIN.as_millis() > (2 * MATCH_AGAIN.as_millis() + 2 * ASKED.as_millis())
);

/// Urgencies, as the specification numbers them: normal, and critical (what
/// Sioul sends always gets through: it filters its own notifications already).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const NORMAL: u8 = 1;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const CRITICAL: u8 = 2;

/// A notification as Sioul sends it on Linux and the BSDs (`notify_message`):
/// its words, its icon (a theme's name), its urgency, its buttons as (key, label).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
struct Notice {
    title: String,
    body: String,
    icon: &'static str,
    urgency: u8,
    buttons: Vec<(String, String)>,
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
impl Notice {
    fn new(title: &str, body: &str, icon: &'static str, urgency: u8) -> Notice {
        Notice { title: title.to_string(), body: body.to_string(), icon, urgency, buttons: Vec::new() }
    }
}

/// What runs with the key of the button pressed.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
type Pressed = Box<dyn FnOnce(&str) + Send>;

/// `notice` with one button at most, keyed "copy" (`code`, `remind`,
/// `mail`): its action runs when it is pressed.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn one_button(mut notice: Notice, button: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    let Some((label, run)) = button else { return notify(notice, None) };
    notice.buttons.push(("copy".to_string(), label));
    notify(
        notice,
        Some(Box::new(move |key: &str| {
            if key == "copy" {
                run();
            }
        })),
    )
}

/// Shows `notice`. Without `pressed`, on the caller's thread, the server's
/// refusal said. With it, on a thread of its own, which keeps the connection
/// that showed it (the server tells a press to that connection: Plasma
/// addresses it) and waits there (`wait_for_press`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn notify(notice: Notice, pressed: Option<Pressed>) -> Result<(), String> {
    let Some(pressed) = pressed else {
        let bus = session().map_err(|e| said(&e))?;
        return send(&bus, &notice).map(drop).map_err(|e| said(&e));
    };
    // The D-Bus connection cannot change threads, so it is made where it waits.
    std::thread::spawn(move || {
        let Ok(bus) = session() else { return };
        let heard = Rc::default();
        if listened(&bus, &heard).is_err() {
            return;
        }
        let Ok(id) = send(&bus, &notice) else { return };
        wait_for_press(&Shown { bus, heard, replies: Rc::default(), id, capabilities: Vec::new() }, pressed, None);
    });
    Ok(())
}

/// A notice sent: the connection that sent it (kept: the server tells a
/// press to it), what it hears, the answers to its own calls (`Replies`),
/// the server's number for it (0 while not known), what the server reads.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
struct Shown {
    bus: LocalConnection,
    heard: Rc<RefCell<Vec<Heard>>>,
    replies: Replies,
    id: u32,
    capabilities: Vec<String>,
}

/// A shown notice's button waited for on its connection: `pressed` runs with
/// the key of the first one pressed, and the wait ends then, or when the
/// notification closes, or the connection goes. What it heard before its
/// number was known is read now. A dose's reminder (`record`) is taken away
/// once its dose is marked or answered anywhere read here, looked at each
/// `LOOK` (review of battery part C, the fifth review's L2).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn wait_for_press(shown: &Shown, pressed: Pressed, mut record: Option<&mut dyn DoseRecord>) {
    let mut looked = Instant::now();
    loop {
        for item in std::mem::take(&mut *shown.heard.borrow_mut()) {
            match item {
                Heard::Pressed(at, key) if at == shown.id => {
                    pressed(&key);
                    return;
                }
                Heard::Closed(at) if at == shown.id => return,
                _ => {}
            }
        }
        if let Some(record) = record.as_deref_mut()
            && looked.elapsed() >= LOOK
        {
            looked = Instant::now();
            if record.settled() {
                if shown.id != 0 {
                    let _: Result<(), _> = shown.bus.with_proxy(SERVER, SERVER_PATH, ASKED).method_call(SERVER, "CloseNotification", (shown.id,));
                }
                return;
            }
        }
        if shown.bus.process(Duration::from_secs(1)).is_err() {
            return;
        }
    }
}

/// How often a dose's reminder still shown looks whether its dose was marked
/// or answered elsewhere (`wait_for_press`): the sharing reads the others
/// each minute.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android")), not(test)))]
const LOOK: Duration = Duration::from_secs(30);
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android")), test))]
const LOOK: Duration = Duration::from_secs(1);

/// How long a dose's notice sent and not answered is still listened for
/// (`wait_late`): its answer, however late (a bus or a server stalled for
/// minutes, hours), says it is shown, and it is then said again in place,
/// the doubt first, or taken away, never left in its first words after
/// another device reminded the dose. A day: past it, the dose is long asked
/// about as one of the past.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const LISTEN_LATE: Duration = Duration::from_secs(24 * 3600);

/// The Notify call, as notify-rust's `Notification` made it, argument for
/// argument: "Sioul", the notification it replaces (0, none), the icon, the
/// title as it is, the body as the server reads it (`body_text`), the
/// buttons, the hints (no sound, Sioul's desktop file, the urgency, each
/// once), ten minutes (`SHOWN`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn notify_message(notice: &Notice, replaces: u32, capabilities: &[String]) -> dbus::Message {
    use dbus::arg::{PropMap, RefArg, Variant};
    let mut hints = PropMap::new();
    hints.insert("suppress-sound".into(), Variant(Box::new(true) as Box<dyn RefArg>));
    hints.insert("desktop-entry".into(), Variant(Box::new(DESKTOP_ENTRY.to_string()) as Box<dyn RefArg>));
    hints.insert("urgency".into(), Variant(Box::new(notice.urgency) as Box<dyn RefArg>));
    let buttons: Vec<&str> = notice.buttons.iter().flat_map(|(key, label)| [key.as_str(), label.as_str()]).collect();
    let timeout = i32::try_from(SHOWN).unwrap_or(i32::MAX);
    let mut call = dbus::Message::new_method_call(SERVER, SERVER_PATH, SERVER, "Notify").expect("the notification server's names");
    call.append_all(("Sioul", replaces, notice.icon, notice.title.as_str(), body_text(&notice.body, capabilities), buttons, hints, timeout));
    call
}

/// What the server reads (`GetCapabilities`), asked `limit` at most; none:
/// not asked, or no answer (read as markup: entities at worst).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn capabilities(bus: &LocalConnection, limit: Duration) -> Vec<String> {
    if limit.is_zero() {
        return Vec::new();
    }
    bus.with_proxy(SERVER, SERVER_PATH, limit).method_call(SERVER, "GetCapabilities", ()).map(|(found,): (Vec<String>,)| found).unwrap_or_default()
}

/// `notice` shown on `bus`, its answer waited for `ASKED`: the server's number for it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn send(bus: &LocalConnection, notice: &Notice) -> Result<u32, dbus::Error> {
    use dbus::blocking::BlockingSender;
    let capabilities = capabilities(bus, ASKED);
    let answer = bus.send_with_reply_and_block(notify_message(notice, 0, &capabilities), ASKED)?;
    answer.read1::<u32>().map_err(|e| dbus::Error::new_failed(&e.to_string()))
}

/// What the server says of notifications, heard on `bus` from now on:
/// buttons pressed, notifications closed (`Heard`), kept in `heard`.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn listened(bus: &LocalConnection, heard: &Rc<RefCell<Vec<Heard>>>) -> Result<(), dbus::Error> {
    use dbus::message::MatchRule;
    let (pressed, closed) = (Rc::clone(heard), Rc::clone(heard));
    bus.add_match(MatchRule::new_signal(SERVER, "ActionInvoked").with_path(SERVER_PATH), move |(id, key): (u32, String), _: &LocalConnection, _: &dbus::Message| {
        pressed.borrow_mut().push(Heard::Pressed(id, key));
        true
    })?;
    bus.add_match(MatchRule::new_signal(SERVER, "NotificationClosed").with_path(SERVER_PATH), move |(id, _why): (u32, u32), _: &LocalConnection, _: &dbus::Message| {
        closed.borrow_mut().push(Heard::Closed(id));
        true
    })?;
    Ok(())
}

/// `listened`, each of its two matches waited for `each` at most (a dose's
/// first try fits in the tick's wait: `TRY`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn listened_within(bus: &LocalConnection, heard: &Rc<RefCell<Vec<Heard>>>, each: Duration) -> Result<(), dbus::Error> {
    use dbus::channel::MatchingReceiver;
    use dbus::message::MatchRule;
    let pressed = MatchRule::new_signal(SERVER, "ActionInvoked").with_path(SERVER_PATH);
    let closed = MatchRule::new_signal(SERVER, "NotificationClosed").with_path(SERVER_PATH);
    let bus_itself = bus.with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", each);
    for rule in [&pressed, &closed] {
        let () = bus_itself.method_call("org.freedesktop.DBus", "AddMatch", (rule.match_str(),))?;
    }
    let (on_press, on_close) = (Rc::clone(heard), Rc::clone(heard));
    bus.start_receive(
        pressed,
        Box::new(move |signal: dbus::Message, _: &LocalConnection| {
            if let Ok((id, key)) = signal.read2::<u32, String>() {
                on_press.borrow_mut().push(Heard::Pressed(id, key));
            }
            true
        }),
    );
    bus.start_receive(
        closed,
        Box::new(move |signal: dbus::Message, _: &LocalConnection| {
            if let Ok((id, _why)) = signal.read2::<u32, u32>() {
                on_close.borrow_mut().push(Heard::Closed(id));
            }
            true
        }),
    );
    Ok(())
}

/// An answer to a call of this connection's own sent without waiting
/// (`dbus::channel::Sender::send`): its return, or the error the bus or the
/// server gave, in a few words.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
enum Reply {
    Return(dbus::Message),
    Error(String),
}

/// The answers to this connection's calls sent without waiting, by serial,
/// kept as they come, late ones too (`heard_replies`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
type Replies = Rc<RefCell<std::collections::HashMap<u32, Reply>>>;

/// The answers to the calls sent without waiting on `bus`, heard from now on.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn heard_replies(bus: &LocalConnection) -> Replies {
    use dbus::channel::MatchingReceiver;
    use dbus::message::{MatchRule, MessageType};
    let replies: Replies = Rc::default();
    for kind in [MessageType::MethodReturn, MessageType::Error] {
        let kept = Rc::clone(&replies);
        let mut rule = MatchRule::new();
        rule.msg_type = Some(kind);
        bus.start_receive(
            rule,
            Box::new(move |mut answer: dbus::Message, _: &LocalConnection| {
                if let Some(serial) = answer.get_reply_serial() {
                    let failed = answer.as_result().err().map(|e| said(&e));
                    kept.borrow_mut().insert(serial, failed.map_or(Reply::Return(answer), Reply::Error));
                }
                true
            }),
        );
    }
    replies
}

/// The answer to `serial`, hearing on `bus` all that comes until `until`;
/// none by then, or the connection gone.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn answer_by(bus: &LocalConnection, replies: &Replies, serial: u32, until: Instant) -> Option<Reply> {
    loop {
        if let Some(reply) = replies.borrow_mut().remove(&serial) {
            return Some(reply);
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() || bus.process(left).is_err() {
            return None;
        }
    }
}

/// What one try of a dose's reminder became.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
enum Tried {
    /// Answered: shown, its number known.
    Shown(Shown),
    /// Surely not gone out: why.
    Unshown(String),
    /// Sent, not answered in time: why; its connection kept (number 0 until
    /// its answer comes), and the serial of its Notify.
    Maybe(String, Shown, u32),
    /// Not sent: no longer to remind, as its words said just before it would
    /// have gone (`Again::late`).
    Done,
}

/// One try of a dose's reminder: the connection (made now, within the
/// first try's `TRY`; or `bus`, a try after the first, within `TRY_AGAIN`),
/// its two matches, what the server reads, then its words as they are now
/// (`words`: shown late, the doubt first; false, no longer to remind), then
/// the Notify, sent only with its own wait (`ASKED`) still ahead, and that
/// wait. Not gone out when anything before the Notify fails or there is no
/// time left for it; sent and not answered in time, maybe.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn try_once(notice: &mut Notice, bus: Option<LocalConnection>, words: &mut dyn FnMut(&mut Notice) -> bool) -> Tried {
    let (all, matching) = if bus.is_none() { (TRY, MATCH) } else { (TRY_AGAIN, MATCH_AGAIN) };
    let deadline = Instant::now() + all;
    let left = || deadline.saturating_duration_since(Instant::now());
    let bus = match bus {
        Some(bus) => bus,
        None => {
            // Room kept after the setup for the two matches and the Notify's wait.
            let Some(limit) = left().checked_sub(2 * matching + ASKED).filter(|limit| !limit.is_zero()) else { return Tried::Unshown("no time left to connect".to_string()) };
            match session_within(limit.min(crate::bus::SETUP)) {
                Ok(bus) => bus,
                Err(e) => return Tried::Unshown(said(&e)),
            }
        }
    };
    let heard = Rc::default();
    let each = left().saturating_sub(ASKED).min(2 * matching) / 2;
    if each.is_zero() {
        return Tried::Unshown("no time left to listen".to_string());
    }
    if let Err(e) = listened_within(&bus, &heard, each) {
        return Tried::Unshown(said(&e));
    }
    let capabilities = capabilities(&bus, left().saturating_sub(ASKED).min(ASKED));
    // Its words read last, just before it goes: a dose marked meanwhile is not reminded.
    if !words(notice) {
        return Tried::Done;
    }
    if left() < ASKED {
        return Tried::Unshown("no time left to send".to_string());
    }
    let replies = heard_replies(&bus);
    use dbus::channel::Sender as _;
    let Ok(serial) = bus.send(notify_message(notice, 0, &capabilities)) else { return Tried::Unshown("the connection was gone".to_string()) };
    let answer = answer_by(&bus, &replies, serial, Instant::now() + ASKED);
    let mut shown = Shown { bus, heard, replies, id: 0, capabilities };
    match answer {
        Some(Reply::Return(answer)) => {
            shown.id = answer.read1::<u32>().unwrap_or(0);
            Tried::Shown(shown)
        }
        // The bus said no (no server: plasmashell starting again) or the server did.
        Some(Reply::Error(why)) => Tried::Unshown(why),
        None => Tried::Maybe("the notification server did not answer in time".to_string(), shown, serial),
    }
}

/// `notice`'s words as `record` says them now, recorded first as about to
/// go (`DoseRecord::going`); false when it is no longer to remind.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn said_now(notice: &mut Notice, record: &mut dyn DoseRecord, going: Going) -> bool {
    match record.going(going) {
        Say::Own => true,
        Say::Words(title, body) => {
            (notice.title, notice.body) = (title, body);
            true
        }
        Say::Nothing => false,
    }
}

/// Whether the tick still waits for a first try's outcome (`Sending`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn waited_for(slot: &Arc<(Mutex<Slot>, Condvar)>) -> bool {
    slot.0.lock().is_ok_and(|slot| !slot.closed)
}

/// A dose reminder's first try, on its own thread: recorded before it goes,
/// its outcome left for the tick (`told`), then as `Again` says. Sent while
/// the tick waits, in its own words; once the tick stopped waiting, in late
/// words (the doubt first), or not at all (no longer to remind). Answered
/// late, said again in place, the doubt first (`shown_late`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn first_try(mut notice: Notice, mut again: Again, pressed: Pressed, slot: &Arc<(Mutex<Slot>, Condvar)>) {
    let (mut sent_late, mut went) = (false, false);
    let tried = try_once(&mut notice, None, &mut |notice: &mut Notice| {
        let going = if waited_for(slot) { Going::First } else { Going::Late(Late::Retry) };
        sent_late = going != Going::First;
        went = said_now(notice, &mut *again.record, going);
        went
    });
    let outcome = match &tried {
        Tried::Shown(_) => Outcome::Shown,
        Tried::Unshown(why) => Outcome::Unshown(why.clone()),
        Tried::Maybe(why, ..) => Outcome::Maybe(why.clone()),
        Tried::Done => Outcome::Gone,
    };
    let waited = told(slot, outcome);
    // Gone out in its own words and known shown only after the tick stopped
    // waiting: on the screen since its time, its fate not said then.
    let late = if sent_late { Late::Retry } else { Late::Unconfirmed };
    match tried {
        Tried::Shown(mut shown) => {
            if waited || sent_late {
                again.record.shown();
            } else {
                shown_late(&mut shown, &mut notice, &mut *again.record, late);
            }
            wait_for_press(&shown, pressed, Some(&mut *again.record));
        }
        Tried::Maybe(_, shown, serial) => wait_late(shown, serial, &mut notice, again, pressed, late),
        Tried::Unshown(_) => {
            if went {
                again.record.refused();
            }
            again.record.unshown();
            show_again(notice, again, pressed);
        }
        Tried::Done => {}
    }
}

/// A notice sent and not answered in time: its answer still heard on its
/// connection, however late (`LISTEN_LATE`), never sent again; answered, it
/// was shown, late (`shown_late`), and its button counts; refused, or its
/// connection gone, it was not.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn wait_late(mut shown: Shown, serial: u32, notice: &mut Notice, mut again: Again, pressed: Pressed, late: Late) {
    let until = Instant::now() + LISTEN_LATE;
    match answer_by(&shown.bus, &shown.replies, serial, until) {
        Some(Reply::Return(answer)) => {
            shown.id = answer.read1::<u32>().unwrap_or(0);
            shown_late(&mut shown, notice, &mut *again.record, late);
            wait_for_press(&shown, pressed, Some(&mut *again.record));
        }
        Some(Reply::Error(_)) | None => {}
    }
}

/// A dose's reminder known shown late (answered late, or after the tick
/// stopped waiting). Its words read first (`DoseRecord::going`; they depend
/// on the record, which says it shown right after): still to remind, it is
/// recorded shown (`DoseRecord::shown`) and said again in place with the
/// doubt first (`Late::Unconfirmed`: "was not confirmed in time"), unless it
/// says so already, was closed meanwhile (seen), or its "Taken" was pressed
/// meanwhile (the press runs after, `wait_for_press`: never shown again over
/// it). The server's answer to that Notify gives its number from then on
/// (GNOME gives a new one to a notice no longer shown). No longer to remind
/// (marked, reminded elsewhere, kept elsewhere now), it is taken away, never
/// recorded shown: another device reads it sent, maybe shown, and says so.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn shown_late(shown: &mut Shown, notice: &mut Notice, record: &mut dyn DoseRecord, late: Late) {
    use dbus::blocking::BlockingSender;
    let id = shown.id;
    let pressed = shown.heard.borrow().iter().any(|heard| matches!(heard, Heard::Pressed(at, _) if *at == id));
    let closed = shown.heard.borrow().iter().any(|heard| matches!(heard, Heard::Closed(at) if *at == id));
    let before = (notice.title.clone(), notice.body.clone());
    if !said_now(notice, record, Going::Late(late)) {
        if id != 0 && !pressed {
            let _: Result<(), _> = shown.bus.with_proxy(SERVER, SERVER_PATH, ASKED).method_call(SERVER, "CloseNotification", (id,));
        }
        return;
    }
    record.shown();
    if id != 0 && !closed && !pressed && (&notice.title, &notice.body) != (&before.0, &before.1)
        && let Ok(answer) = shown.bus.send_with_reply_and_block(notify_message(notice, id, &shown.capabilities), ASKED)
        && let Ok(number) = answer.read1::<u32>()
        && number != 0
    {
        shown.id = number;
    }
}

/// A dose's reminder tried again after a first try that did not go out, the
/// doubt first, until `Again::end`, then no more (`DoseRecord::over`): the
/// bus first (a stalled one refuses at once, `bus`, and nothing else is read
/// meanwhile), then the server, its words read last, just before it goes,
/// recorded as going (none: no longer to remind, and no more tries). None
/// during `Again::quiet` (a phone's alarm rings then): promised before it
/// begins (`DoseRecord::pause`), the tries resumed after it while still to
/// remind. Sent and not answered in time: never again (`wait_late`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn show_again(mut notice: Notice, mut again: Again, pressed: Pressed) {
    let mut tries = 0;
    loop {
        let wait = again_in(tries);
        tries += 1;
        let next = Instant::now() + wait;
        if let Some((from, to, until)) = again.quiet
            && next >= from
        {
            again.quiet = None;
            if !again.record.pause(until) {
                return;
            }
            std::thread::sleep(to.saturating_duration_since(Instant::now()));
            tries = 0;
            continue;
        }
        if next >= again.end {
            again.record.over();
            return;
        }
        std::thread::sleep(wait);
        let Ok(bus) = session_within(crate::bus::SETUP) else { continue };
        let mut went = false;
        let tried = try_once(&mut notice, Some(bus), &mut |notice: &mut Notice| {
            went = said_now(notice, &mut *again.record, Going::Late(Late::Retry));
            went
        });
        match tried {
            Tried::Shown(shown) => {
                again.record.shown();
                return wait_for_press(&shown, pressed, Some(&mut *again.record));
            }
            Tried::Unshown(_) if went => again.record.refused(),
            Tried::Unshown(_) => {}
            Tried::Done => return,
            Tried::Maybe(_, shown, serial) => return wait_late(shown, serial, &mut notice, again, pressed, Late::Retry),
        }
    }
}

/// How long a dose's reminder waits before its try after `tries` others:
/// five seconds for two minutes (a bus that stalled a while: shown soon after
/// it answers), then half a minute.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn again_in(tries: u32) -> Duration {
    if tries < 24 { Duration::from_secs(5) } else { Duration::from_secs(30) }
}

/// A connection to the session's bus, made by `crate::bus` (`bus::SETUP` at most).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn session() -> Result<LocalConnection, dbus::Error> {
    crate::bus::session().map(LocalConnection::from)
}

/// The same, made within `limit`.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn session_within(limit: Duration) -> Result<LocalConnection, dbus::Error> {
    let address = crate::bus::session_address().ok_or_else(|| dbus::Error::new_failed("no session bus"))?;
    crate::bus::connect_within(&address, limit).map(LocalConnection::from)
}

/// A D-Bus error in a few words: its message, else its name.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn said(e: &dbus::Error) -> String {
    e.message().or(e.name()).unwrap_or("D-Bus").to_string()
}

/// The notification of something going on, the time running while a focus
/// session runs (docs/tasks.md): shown until it is closed, changed in place,
/// its buttons pressed any number of times. Its own thread holds the D-Bus
/// connection that shows it, and listens there: the server tells a press to
/// that connection alone (Plasma addresses it). Resident, kept when a button
/// is pressed; at normal urgency, for Plasma shows a low one nowhere unless
/// told to; without sound. A server that keeps what it shows ("persistence":
/// its history, its tray) takes it there after its popup, buttons alive;
/// elsewhere it does not expire. No session bus, no server: nothing shows,
/// nothing breaks. Dropped, it is taken away.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub struct Ongoing {
    changes: Sender<Change>,
}

/// What a lasting notification says: its buttons as (key, label), "default"
/// for a click on it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
struct Said {
    title: String,
    body: String,
    actions: Vec<(String, String)>,
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
enum Change {
    Show(Said),
    /// Taken away; said done on the sender, if any.
    Close(Option<Sender<()>>),
}

/// What the server said, read between two looks.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
enum Heard {
    Pressed(u32, String),
    Closed(u32),
}

/// Shows the notification of something going on. `actions` are its buttons
/// as (key, label), "default" for a click on it; `on_action` runs with the
/// key of each one pressed, on the notification's own thread.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn ongoing(title: &str, body: &str, actions: Vec<(String, String)>, on_action: Box<dyn Fn(&str) + Send>) -> Ongoing {
    let (changes, asked) = std::sync::mpsc::channel();
    let said = Said { title: title.to_string(), body: body.to_string(), actions };
    // The D-Bus connection cannot change threads, so it is made where it listens.
    std::thread::spawn(move || serve(said, &asked, &*on_action));
    Ongoing { changes }
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
impl Ongoing {
    /// Changes it in place; shown again if it was closed meanwhile.
    pub fn update(&self, title: &str, body: &str, actions: Vec<(String, String)>) {
        let _ = self.changes.send(Change::Show(Said { title: title.to_string(), body: body.to_string(), actions }));
    }

    /// Takes it away, waiting at most `wait` for the server to have done it:
    /// as Sioul quits, before the connection goes with the process.
    pub fn close(self, wait: Duration) {
        let (done, closed) = std::sync::mpsc::channel();
        if self.changes.send(Change::Close(Some(done))).is_ok() {
            let _ = closed.recv_timeout(wait);
        }
    }
}

/// A lasting notification, across its connections: what it says, whether
/// that changed since it was last sent, the server's number for it (0 while
/// none shows), when the server last refused it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
struct Lasting {
    said: Said,
    fresh: bool,
    id: u32,
    refused: Option<Instant>,
}

/// A lasting notification's thread: what Sioul asks shown, what the server
/// says heard, until it is taken away. Its connection is made again while
/// the bus does not give one (a stall: `bus` refuses at once meanwhile, five
/// seconds apart, `bus::SETUP`) and once it is lost, rather than the
/// notification being lost for the session (review of battery part C, L4);
/// what Sioul asks meanwhile is kept, the last of it, and a close ends it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn serve(said: Said, asked: &Receiver<Change>, on_action: &dyn Fn(&str)) {
    use std::sync::mpsc::RecvTimeoutError;
    let mut lasting = Lasting { said, fresh: true, id: 0, refused: None };
    loop {
        if let Ok(connection) = session()
            && let Served::Done = served(&mut lasting, asked, on_action, &connection)
        {
            return;
        }
        match asked.recv_timeout(crate::bus::SETUP) {
            Ok(Change::Show(next)) => (lasting.said, lasting.fresh) = (next, true),
            Ok(Change::Close(done)) => {
                if let Some(done) = done {
                    let _ = done.send(());
                }
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// What became of a lasting notification on one connection (`served`):
/// taken away, or its connection lost (made again, the notification shown
/// again in place of its number, when the server still knows it).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
enum Served {
    Done,
    Lost,
}

/// A lasting notification on `connection`: what Sioul asks shown, what the
/// server says heard, until it is taken away or the connection is lost. Each
/// Notify goes without waiting, its answer heard as it comes: while one is
/// not answered, none other is sent, so that a server answering late never
/// shows it twice (each shown with no number to replace, neither taken
/// away: review of battery part C, L1); its late answer gives the number,
/// and what changed meanwhile goes in its place.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn served(lasting: &mut Lasting, asked: &Receiver<Change>, on_action: &dyn Fn(&str), connection: &LocalConnection) -> Served {
    use dbus::channel::Sender as _;
    use std::sync::mpsc::TryRecvError;
    let heard: Rc<RefCell<Vec<Heard>>> = Rc::default();
    if listened(connection, &heard).is_err() {
        return Served::Lost;
    }
    let replies = heard_replies(connection);
    let server = connection.with_proxy(SERVER, SERVER_PATH, ASKED);
    // Asked again while not known: a server starting after Sioul answers later.
    let mut capabilities: Vec<String> = Vec::new();
    // A Notify sent and not answered yet: its serial.
    let mut unanswered: Option<u32> = None;
    loop {
        // What the server said meanwhile, all of it: answers, buttons pressed, notifications closed.
        while connection.process(Duration::ZERO).unwrap_or(false) {}
        if let Some(serial) = unanswered {
            let reply = replies.borrow_mut().remove(&serial);
            match reply {
                Some(Reply::Return(answer)) => {
                    (lasting.id, lasting.refused, unanswered) = (answer.read1::<u32>().unwrap_or(0), None, None);
                }
                Some(Reply::Error(_)) => (lasting.refused, unanswered) = (Some(Instant::now()), None),
                None => {}
            }
        }
        let news = std::mem::take(&mut *heard.borrow_mut());
        for item in news {
            match item {
                Heard::Pressed(at, key) if at == lasting.id && lasting.id != 0 => on_action(&key),
                // Closed by you, or by the server: shown again at the next change.
                Heard::Closed(at) if at == lasting.id => lasting.id = 0,
                _ => {}
            }
        }
        // What Sioul asked meanwhile: the last of it counts.
        loop {
            match asked.try_recv() {
                Ok(Change::Show(next)) => (lasting.said, lasting.fresh) = (next, true),
                Ok(Change::Close(done)) => {
                    close_lasting(connection, &server, &replies, unanswered, lasting.id);
                    if let Some(done) = done {
                        let _ = done.send(());
                    }
                    return Served::Done;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    close_lasting(connection, &server, &replies, unanswered, lasting.id);
                    return Served::Done;
                }
            }
        }
        if unanswered.is_none() && (lasting.fresh || lasting.refused.is_some_and(|at| at.elapsed() >= ASKED_AGAIN)) {
            if capabilities.is_empty() {
                capabilities = server.method_call(SERVER, "GetCapabilities", ()).map(|(found,): (Vec<String>,)| found).unwrap_or_default();
            }
            match connection.send(lasting_message(lasting.id, &lasting.said, &capabilities)) {
                Ok(serial) => unanswered = Some(serial),
                Err(()) => return Served::Lost,
            }
            lasting.fresh = false;
        }
        if connection.process(LISTENED).is_err() {
            return Served::Lost;
        }
    }
}

/// How long a lasting notification taken away while its Notify is not answered
/// waits for that answer, its number, to close it (`close_lasting`): a
/// server seconds late still gets it closed.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const CLOSE_LATE: Duration = Duration::from_secs(30);

/// The lasting notification's Notify, in place of `id` if not 0.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn lasting_message(id: u32, said: &Said, capabilities: &[String]) -> dbus::Message {
    use dbus::arg::{PropMap, RefArg, Variant};
    let mut hints = PropMap::new();
    hints.insert("resident".into(), Variant(Box::new(true) as Box<dyn RefArg>));
    hints.insert("suppress-sound".into(), Variant(Box::new(true) as Box<dyn RefArg>));
    hints.insert("urgency".into(), Variant(Box::new(1u8) as Box<dyn RefArg>));
    hints.insert("desktop-entry".into(), Variant(Box::new(DESKTOP_ENTRY.to_string()) as Box<dyn RefArg>));
    let actions: Vec<&str> = said.actions.iter().flat_map(|(key, label)| [key.as_str(), label.as_str()]).collect();
    // Kept by the server once its popup goes, or never expiring where nothing keeps it.
    let timeout: i32 = if capabilities.iter().any(|c| c == "persistence") { -1 } else { 0 };
    let mut call = dbus::Message::new_method_call(SERVER, SERVER_PATH, SERVER, "Notify").expect("the notification server's names");
    call.append_all(("Sioul", id, "chronometer", said.title.as_str(), body_text(&said.body, capabilities), actions, hints, timeout));
    call
}

/// Taken away: by its number; a Notify not answered yet, its answer waited
/// for first (`CLOSE_LATE` at most: its number), so that it does not stay
/// (the fifth review's L4); the close of a session goes on meanwhile.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn close_lasting(connection: &LocalConnection, server: &dbus::blocking::Proxy<'_, &LocalConnection>, replies: &Replies, unanswered: Option<u32>, id: u32) {
    let id = match unanswered.and_then(|serial| answer_by(connection, replies, serial, Instant::now() + CLOSE_LATE)) {
        Some(Reply::Return(answer)) => answer.read1::<u32>().unwrap_or(id),
        _ => id,
    };
    if id != 0 {
        let _: Result<(), _> = server.method_call(SERVER, "CloseNotification", (id,));
    }
}

/// Windows and macOS: no lasting notification yet, the focus window shows
/// the time. Android: Android's own, made on the window's side (sioul-app).
#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub struct Ongoing;

#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub fn ongoing(_title: &str, _body: &str, _actions: Vec<(String, String)>, _on_action: Box<dyn Fn(&str) + Send>) -> Ongoing {
    Ongoing
}

#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
impl Ongoing {
    pub fn update(&self, _title: &str, _body: &str, _actions: Vec<(String, String)>) {}

    pub fn close(self, _wait: Duration) {}
}

/// A body as the notification server reads it. Most read markup there
/// ("body-markup": `<b>`, `<a href>`, `<img src>`), and bodies carry a mail's
/// or a site's words: there, `&`, `<` and `>` are written as entities, so
/// that the words show as they came and nothing in them is a link or a
/// picture to fetch. A server that reads no markup shows the text as it is.
/// The summary is plain text for every server (the specification), and goes as it is.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn body_text(body: &str, capabilities: &[String]) -> String {
    // A server that could not be asked is taken to read markup: entities at worst.
    if capabilities.is_empty() || capabilities.iter().any(|c| c == "body-markup") {
        body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    } else {
        body.to_string()
    }
}

/// The bundle id macOS is given is the one Sioul's macOS bundle declares:
/// any other would attribute its notifications to another application.
#[cfg(test)]
mod bundle {
    #[test]
    fn macos_is_given_the_bundles_own_id() {
        let plist = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/macos/Info.plist")).unwrap();
        let declared = plist.split("<key>CFBundleIdentifier</key>").nth(1).and_then(|rest| rest.split("<string>").nth(1)).and_then(|rest| rest.split("</string>").next()).unwrap();
        assert_eq!(super::BUNDLE_ID, declared.trim());
    }
}

#[cfg(all(test, unix, not(any(target_os = "macos", target_os = "android"))))]
mod tests {
    use super::*;

    #[test]
    fn bodies_are_words_not_markup() {
        let markup = vec!["actions".to_string(), "body-markup".to_string()];
        let body = "Code 482913 <img src=\"https://example.org/t.png\"> & <a href=\"https://example.org\">here</a>";
        let shown = body_text(body, &markup);
        assert!(!shown.contains('<') && !shown.contains('>'), "{shown}");
        assert!(shown.starts_with("Code 482913 &lt;img") && shown.contains(" &amp; "), "{shown}");
        // Not asked, read as markup; a server without markup gets the words as they are.
        assert_eq!(body_text("a < b", &[]), "a &lt; b");
        assert_eq!(body_text("a < b & c", &["actions".to_string()]), "a < b & c");
    }

    /// A connection to the test's own session bus (dbus-run-session) owning
    /// the notification server's name, a stand-in of a test before given two
    /// seconds to leave it; none where a server is there already (the
    /// desktop's): never in a server's place, no replacing it, no waiting in line.
    fn stand_in_bus() -> Option<dbus::blocking::LocalConnection> {
        use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
        let until = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let bus = session().ok().filter(|bus| matches!(bus.request_name(SERVER, false, false, true), Ok(RequestNameReply::PrimaryOwner)));
            if bus.is_some() || std::time::Instant::now() >= until {
                return bus;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The hints' names a call carries, each as many times as it is there
    /// (a dictionary read as such would keep one of each).
    fn hint_names(call: &dbus::Message) -> Vec<String> {
        use dbus::arg::ArgType;
        let mut args = call.iter_init();
        for _ in 0..6 {
            args.next();
        }
        let mut names = Vec::new();
        if let Some(mut entries) = args.recurse(ArgType::Array) {
            while entries.arg_type() == ArgType::DictEntry {
                if let Some(mut entry) = entries.recurse(ArgType::DictEntry) {
                    names.extend(entry.get::<String>());
                }
                entries.next();
            }
        }
        names.sort();
        names
    }

    /// What a stand-in server was sent: when, the title, the number it replaces.
    type Notices = std::sync::mpsc::Receiver<(std::time::Instant, String, u32)>;

    /// A stand-in notification server on the bus at `address`, on a thread of
    /// its own until `stop`: "answers" answers each Notify and presses "copy"
    /// on each notice that has it, to the connection that showed it; "shows"
    /// answers, never presses; "late" shows it (says it) at once but answers
    /// three seconds later, then presses; "press-first" the same, pressed
    /// first, at one second, and kept on the screen after; "renumber" answers
    /// a new Notify three seconds late, never presses, and gives a notice
    /// replaced a number of its own (GNOME); "stall" answers as "answers",
    /// but its first GetCapabilities stops the bus (`bus_pid`, SIGSTOP)
    /// before its answer goes, for six seconds; "silent" takes each call and
    /// never answers. A CloseNotification answered with NotificationClosed,
    /// as a server does. Each Notify said on `told`: when it came, its title,
    /// the number it replaces; each CloseNotification, `CLOSED` and its
    /// number. Whether it got the name.
    fn notice_server(address: &str, mode: &'static str, told: std::sync::mpsc::Sender<(std::time::Instant, String, u32)>, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> bool {
        notice_server_on(address, mode, 0, told, stop)
    }

    /// `notice_server`, the bus's pid given for "stall".
    fn notice_server_on(address: &str, mode: &'static str, bus_pid: u32, told: std::sync::mpsc::Sender<(std::time::Instant, String, u32)>, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> bool {
        use dbus::blocking::LocalConnection;
        use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
        use dbus::channel::{MatchingReceiver, Sender as _};
        use dbus::message::MatchRule;
        let (ready, started) = std::sync::mpsc::channel::<bool>();
        let address = address.to_string();
        std::thread::spawn(move || {
            let Some(bus) = crate::bus::connect(&address).ok().map(LocalConnection::from).filter(|bus| matches!(bus.request_name(SERVER, false, false, true), Ok(RequestNameReply::PrimaryOwner))) else {
                let _ = ready.send(false);
                return;
            };
            // Answers and presses due later: when, what.
            let due: Rc<RefCell<Vec<(std::time::Instant, dbus::Message)>>> = Rc::default();
            let later = Rc::clone(&due);
            let mut next = 40u32;
            let mut stalled = false;
            bus.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |call: dbus::Message, bus: &LocalConnection| {
                    let member = call.member().map(|m| m.to_string()).unwrap_or_default();
                    if member == "Notify"
                        && let Ok((_, replaces, _, title)) = call.read4::<String, u32, String, String>()
                    {
                        let _ = told.send((std::time::Instant::now(), title, replaces));
                    }
                    if member == "CloseNotification"
                        && let Ok(id) = call.read1::<u32>()
                    {
                        let _ = told.send((std::time::Instant::now(), CLOSED.to_string(), id));
                        if mode != "silent" {
                            let mut closed = dbus::Message::new_signal(SERVER_PATH, SERVER, "NotificationClosed").unwrap().append2(id, 3u32);
                            closed.set_destination(call.sender().map(|s| s.into_static()));
                            let _ = bus.send(closed);
                        }
                    }
                    if mode == "silent" {
                        return true;
                    }
                    if mode == "stall" && member == "GetCapabilities" && !stalled {
                        stalled = true;
                        crate::bus::StandIns::signal(bus_pid, "-STOP");
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_secs(6));
                            crate::bus::StandIns::signal(bus_pid, "-CONT");
                        });
                    }
                    let (_, replaces) = if member == "Notify" { call.read2::<String, u32>().unwrap_or_default() } else { (String::new(), 0) };
                    let reply = match member.as_str() {
                        // GNOME's way, "renumber": a notice replaced gets a number of its own.
                        "Notify" if replaces != 0 && mode == "renumber" => call.method_return().append1(replaces + 100),
                        "Notify" => {
                            let id = if replaces != 0 {
                                replaces
                            } else {
                                next += 1;
                                next
                            };
                            call.method_return().append1(id)
                        }
                        "GetCapabilities" => call.method_return().append1(vec!["actions", "body"]),
                        "CloseNotification" => call.method_return(),
                        _ => return true,
                    };
                    let buttons: Vec<String> = if member == "Notify" { call.read_all::<(String, u32, String, String, String, Vec<String>, dbus::arg::PropMap, i32)>().map(|all| all.5).unwrap_or_default() } else { Vec::new() };
                    let press = buttons.iter().any(|key| key == "copy" && !matches!(mode, "shows" | "renumber")).then(|| {
                        let id = reply.read1::<u32>().unwrap_or(0);
                        let mut press = dbus::Message::new_signal(SERVER_PATH, SERVER, "ActionInvoked").unwrap().append2(id, "copy");
                        press.set_destination(call.sender().map(|s| s.into_static()));
                        press
                    });
                    let now = std::time::Instant::now();
                    if matches!(mode, "late" | "press-first" | "renumber") && member == "Notify" && replaces == 0 {
                        later.borrow_mut().push((now + Duration::from_secs(3), reply));
                        let pressed_at = if mode == "press-first" { Duration::from_secs(1) } else { Duration::from_millis(3_300) };
                        if let Some(press) = press {
                            later.borrow_mut().push((now + pressed_at, press));
                        }
                    } else if mode == "late" && member == "Notify" {
                        later.borrow_mut().push((now + Duration::from_secs(3), reply));
                    } else {
                        let _ = bus.send(reply);
                        if let Some(press) = press {
                            let _ = bus.send(press);
                        }
                    }
                    true
                }),
            );
            let _ = ready.send(true);
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = bus.process(Duration::from_millis(50));
                let now = std::time::Instant::now();
                let ready: Vec<dbus::Message> = {
                    let mut waiting = due.borrow_mut();
                    let (go, stay): (Vec<_>, Vec<_>) = std::mem::take(&mut *waiting).into_iter().partition(|(at, _)| *at <= now);
                    *waiting = stay;
                    go.into_iter().map(|(_, message)| message).collect()
                };
                for message in ready {
                    let _ = bus.send(message);
                }
            }
        });
        started.recv_timeout(Duration::from_secs(5)).unwrap_or(false)
    }

    /// What a stand-in server says of a CloseNotification on its `told`, the
    /// number closed beside it.
    const CLOSED: &str = "(closed)";

    /// What a stand-in server was sent within `within`: (title, replaces), in order.
    fn sent_within(notices: &Notices, within: Duration) -> Vec<(String, u32)> {
        let until = std::time::Instant::now() + within;
        let mut seen = Vec::new();
        while let Ok((_, title, replaces)) = notices.recv_timeout(until.saturating_duration_since(std::time::Instant::now())) {
            seen.push((title, replaces));
        }
        seen
    }

    /// A dose's record as health.rs keeps it, for the stand-ins: its first
    /// try's words its own and its late words from `late`, while still to
    /// remind (none in `late`: no longer to remind), none once recorded shown
    /// too (`late_reminder_wanted`); a late word unconfirmed says so in its
    /// body. What it records said on `log`, if any ("going First", "going
    /// Late(Retry)", "unshown", "pause", "over"); `settled`: marked elsewhere.
    struct StandIn {
        name: String,
        late: std::sync::Arc<std::sync::Mutex<Option<String>>>,
        shown: std::sync::mpsc::Sender<String>,
        log: Option<std::sync::mpsc::Sender<String>>,
        recorded: bool,
        settled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl StandIn {
        fn say(&self, what: String) {
            if let Some(log) = &self.log {
                let _ = log.send(what);
            }
        }
    }

    impl DoseRecord for StandIn {
        fn going(&mut self, going: Going) -> Say {
            self.say(format!("going {going:?}"));
            let words = if self.recorded { None } else { self.late.lock().unwrap().clone() };
            match (going, words) {
                (_, None) => Say::Nothing,
                (Going::First, Some(_)) => Say::Own,
                (Going::Late(Late::Retry), Some(title)) => Say::Words(title, "22:00. Could not be shown in time. Check before taking it.".into()),
                (Going::Late(Late::Unconfirmed), Some(title)) => Say::Words(title, "22:00. Not confirmed in time. Check before taking it.".into()),
            }
        }
        fn refused(&mut self) {
            self.say("refused".into());
        }
        fn shown(&mut self) {
            self.recorded = true;
            let _ = self.shown.send(self.name.clone());
        }
        fn unshown(&mut self) {
            self.say("unshown".into());
        }
        fn pause(&mut self, until: i64) -> bool {
            self.say(format!("pause {until}"));
            !self.recorded && self.late.lock().unwrap().is_some()
        }
        fn over(&mut self) {
            self.say("over".into());
        }
        fn settled(&mut self) -> bool {
            self.settled.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    /// A dose's reminder for the stand-ins: `name` its title; its button's
    /// action and what shows late said on channels; `late` its late words
    /// (none: no longer to remind), as health.rs gives them; tried until `until`.
    fn dose(name: &str, until: Duration, pressed: &std::sync::mpsc::Sender<String>, shown: &std::sync::mpsc::Sender<String>, late: std::sync::Arc<std::sync::Mutex<Option<String>>>) -> Sending {
        dose_as(name, Again { end: std::time::Instant::now() + until, quiet: None, record: Box::new(StandIn { name: name.into(), late, shown: shown.clone(), log: None, recorded: false, settled: std::sync::Arc::default() }) }, pressed)
    }

    /// The same, its record and its tries as given.
    fn dose_as(name: &str, again: Again, pressed: &std::sync::mpsc::Sender<String>) -> Sending {
        let (pressed, pressed_name) = (pressed.clone(), name.to_string());
        remind_dose(name, "22:00", ("Taken".into(), Box::new(move || drop(pressed.send(pressed_name.clone())))), again)
    }

    /// Dose reminders and the focus session's lasting notification against a
    /// stand-in session bus (`bus::StandIns`, never the session of whoever
    /// runs this) and a stand-in notification server on it (review of
    /// battery part C, H1 and L4 of the third review, the review's own
    /// scenarios):
    /// - healthy: shown, while the tick waits; its button counts once;
    /// - the bus stopped (SIGSTOP) before the dose's minute, let go 8 s later:
    ///   not gone out (the connection never came), said within `FIRST_TRY`;
    ///   tried again, shown once the bus answers, late (its words late's, the
    ///   doubt first), and its button counts;
    /// - stopped, reminders at 1, 10 and 20 s, let go at 30 s: none gone out,
    ///   all three shown once the bus answers;
    /// - a server that never answers: sent, so maybe, never sent again;
    /// - no server, then one (plasmashell starting again): not gone out, then
    ///   shown once it is there; none at all once no longer to remind;
    /// - the lasting notification, its connection stalled at first: shown
    ///   once the bus answers, not lost for the session.
    /// Sets this process's session bus address: run alone, by name:
    /// `cargo test --release -p sioul-sync notify::tests::reminders_say_their_outcome_and_show_late_after_a_stall -- --ignored --exact --nocapture`.
    #[test]
    #[ignore = "starts a stand-in dbus-daemon and sets the session bus address: run alone, by name"]
    fn reminders_say_their_outcome_and_show_late_after_a_stall() {
        use crate::bus::StandIns;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc::channel;
        use std::sync::{Arc, Mutex};
        use std::time::Instant;
        let mut stand_ins = StandIns::new("stand-in-notices");
        let (pid, address) = stand_ins.start("session");
        // SAFETY: run alone, by name; nothing else in this process reads it meanwhile.
        unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
        let (told, notices) = channel::<(Instant, String, u32)>();
        let stop = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "answers", told.clone(), Arc::clone(&stop)), "the stand-in server");
        let (pressed, presses) = channel::<String>();
        let (late, lates) = channel::<String>();
        // The late words: none once no longer to remind.
        let words = |title: &str| Arc::new(Mutex::new(Some(format!("Check first: {title}"))));

        // Healthy: shown at once, while the tick waits; its button pressed, once.
        let start = Instant::now();
        assert_eq!(dose("Magnesium · 300 mg", Duration::from_secs(60), &pressed, &late, words("Magnesium · 300 mg")).outcome(FIRST_TRY), Outcome::Shown);
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(presses.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Magnesium · 300 mg"));
        assert_eq!(sent_within(&notices, Duration::from_millis(300)), [("Magnesium · 300 mg".to_string(), 0)], "its own words");
        assert_eq!(lates.recv_timeout(Duration::from_secs(1)).as_deref(), Ok("Magnesium · 300 mg"), "recorded shown");

        // One stall: stopped at 0, the reminder at 1 s, let go at 9 s.
        StandIns::signal(pid, "-STOP");
        std::thread::sleep(Duration::from_secs(1));
        let start = Instant::now();
        let outcome = dose("Magnesium · 300 mg", Duration::from_secs(60), &pressed, &late, words("Magnesium · 300 mg")).outcome(FIRST_TRY);
        let took = start.elapsed();
        assert!(matches!(outcome, Outcome::Unshown(_)) && took <= FIRST_TRY, "the connection never came: not gone out, said within FIRST_TRY: {outcome:?} after {took:?}");
        std::thread::sleep(Duration::from_secs(8).saturating_sub(start.elapsed() + Duration::from_secs(1)));
        StandIns::signal(pid, "-CONT");
        assert_eq!(lates.recv_timeout(Duration::from_secs(20)).as_deref(), Ok("Magnesium · 300 mg"), "shown late once the bus answers");
        assert_eq!(presses.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Magnesium · 300 mg"), "its button works");
        assert_eq!(sent_within(&notices, Duration::from_millis(500)), [("Check first: Magnesium · 300 mg".to_string(), 0)], "once, the doubt first");

        // Three reminders over 30 s, the bus stopped all along.
        StandIns::signal(pid, "-STOP");
        let start = Instant::now();
        let mut sending = Vec::new();
        for (at, name) in [(1, "First"), (10, "Second"), (20, "Third")] {
            std::thread::sleep(Duration::from_secs(at).saturating_sub(start.elapsed()));
            let sent = dose(name, Duration::from_secs(90), &pressed, &late, words(name));
            assert!(matches!(sent.outcome(FIRST_TRY), Outcome::Unshown(_)), "{name}: not gone out");
            sending.push(sent);
        }
        std::thread::sleep(Duration::from_secs(30).saturating_sub(start.elapsed()));
        StandIns::signal(pid, "-CONT");
        let mut shown: Vec<String> = (0..3).filter_map(|_| lates.recv_timeout(Duration::from_secs(25)).ok()).collect();
        shown.sort();
        assert_eq!(shown, ["First", "Second", "Third"], "all three shown once the bus answers");
        let mut seen: Vec<String> = sent_within(&notices, Duration::from_secs(2)).into_iter().map(|(title, _)| title).collect();
        seen.sort();
        assert_eq!(seen, ["Check first: First", "Check first: Second", "Check first: Third"]);
        for _ in 0..3 {
            let _ = presses.recv_timeout(Duration::from_secs(5));
        }

        // A server that never answers: sent, so maybe; never sent again.
        stop.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));
        let quiet = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "silent", told.clone(), Arc::clone(&quiet)), "the silent server");
        let start = Instant::now();
        let outcome = dose("Magnesium · 300 mg", Duration::from_secs(60), &pressed, &late, words("Magnesium · 300 mg")).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Maybe(_)) && start.elapsed() < FIRST_TRY, "{outcome:?} after {:?}", start.elapsed());
        assert_eq!(sent_within(&notices, Duration::from_secs(15)), [("Magnesium · 300 mg".to_string(), 0)], "asked once, never again");
        assert!(lates.try_recv().is_err());

        // No server, then one: not gone out, then shown once it is there; none once no longer to remind.
        quiet.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));
        let outcome = dose("Magnesium · 300 mg", Duration::from_secs(60), &pressed, &late, words("Magnesium · 300 mg")).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Unshown(_)), "no server: {outcome:?}");
        std::thread::sleep(Duration::from_secs(6));
        let back = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "answers", told.clone(), Arc::clone(&back)));
        assert_eq!(lates.recv_timeout(Duration::from_secs(15)).as_deref(), Ok("Magnesium · 300 mg"), "shown once the server is there");
        let _ = presses.recv_timeout(Duration::from_secs(5));
        assert_eq!(sent_within(&notices, Duration::from_millis(500)), [("Check first: Magnesium · 300 mg".to_string(), 0)]);
        back.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));
        let not_any_more = words("Magnesium · 300 mg");
        let outcome = dose("Magnesium · 300 mg", Duration::from_secs(60), &pressed, &late, Arc::clone(&not_any_more)).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Unshown(_)));
        *not_any_more.lock().unwrap() = None;
        let again = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "answers", told.clone(), Arc::clone(&again)));
        assert!(sent_within(&notices, Duration::from_secs(12)).is_empty(), "marked meanwhile: never shown");
        assert!(lates.try_recv().is_err());

        // The lasting notification, its connection stalled at first: shown once the bus answers.
        StandIns::signal(pid, "-STOP");
        let lasting = ongoing("Write to the bank", "Since 14:02, for 25 min", vec![("stop".to_string(), "Stop".to_string())], Box::new(|_: &str| {}));
        std::thread::sleep(Duration::from_secs(8));
        StandIns::signal(pid, "-CONT");
        assert_eq!(sent_within(&notices, Duration::from_secs(20)).first().map(|(title, _)| title.as_str()), Some("Write to the bank"), "not lost for the session");
        lasting.close(Duration::from_secs(3));
        again.store(true, Ordering::Relaxed);
    }

    /// A dose's reminder whose fate the tick could not know (review of battery
    /// part C, the fourth review's H1 timelines), against a stand-in bus and
    /// server:
    /// - the server shows it at once and answers three seconds late: maybe
    ///   (sent, not answered within its two seconds); its late answer heard
    ///   on the connection kept: recorded shown (`Again::shown`), said again
    ///   in place with the doubt first (the same number), and its "Taken",
    ///   pressed after, counts once;
    /// - the same, the dose marked before the late answer: taken away by its
    ///   number, never recorded a reminder;
    /// - the bus stopped while the try was under way (after the connection,
    ///   before the server's answers): maybe; let go, shown, late: the doubt
    ///   first in place, "Taken" counts;
    /// - sent after the tick stopped waiting: the doubt first at once, one
    ///   notice, recorded shown, "Taken" counts; marked before it went: never
    ///   sent, nor tried again.
    /// Sets this process's session bus address: run alone, by name:
    /// `cargo test --release -p sioul-sync notify::tests::reminders_whose_fate_was_not_known_say_the_doubt_once_known_shown -- --ignored --exact --nocapture`.
    #[test]
    #[ignore = "starts a stand-in dbus-daemon and sets the session bus address: run alone, by name"]
    fn reminders_whose_fate_was_not_known_say_the_doubt_once_known_shown() {
        use crate::bus::StandIns;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc::channel;
        use std::sync::{Arc, Mutex};
        use std::time::Instant;
        let mut stand_ins = StandIns::new("stand-in-fates");
        let (pid, address) = stand_ins.start("session");
        // SAFETY: run alone, by name; nothing else in this process reads it meanwhile.
        unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
        let (told, notices) = channel::<(Instant, String, u32)>();
        let (pressed, presses) = channel::<String>();
        let (late, lates) = channel::<String>();
        let words = || Arc::new(Mutex::new(Some("Check first: Levothyroxine · 75 µg".to_string())));

        // The server answers three seconds late.
        let slow = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "late", told.clone(), Arc::clone(&slow)));
        let start = Instant::now();
        let outcome = dose("Levothyroxine · 75 µg", Duration::from_secs(60), &pressed, &late, words()).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Maybe(_)) && start.elapsed() < Duration::from_secs(3), "sent, not answered in time: maybe ({outcome:?} after {:?})", start.elapsed());
        assert_eq!(lates.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Levothyroxine · 75 µg"), "its late answer heard: recorded shown");
        let sent = sent_within(&notices, Duration::from_secs(2));
        assert_eq!(sent.len(), 2, "{sent:?}");
        assert_eq!(sent[0], ("Levothyroxine · 75 µg".to_string(), 0));
        assert!(sent[1].0 == "Check first: Levothyroxine · 75 µg" && sent[1].1 != 0, "said again in place, the doubt first: {sent:?}");
        assert_eq!(presses.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Levothyroxine · 75 µg"), "its Taken, pressed after, counts");
        assert!(presses.recv_timeout(Duration::from_secs(4)).is_err(), "once");
        slow.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));

        // The same, the dose marked before the late answer: taken away, never recorded a reminder.
        let slow = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "late", told.clone(), Arc::clone(&slow)));
        let marked = words();
        let start = Instant::now();
        let outcome = dose("Levothyroxine · 75 µg", Duration::from_secs(60), &pressed, &late, Arc::clone(&marked)).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Maybe(_)) && start.elapsed() < Duration::from_secs(3), "{outcome:?} after {:?}", start.elapsed());
        *marked.lock().unwrap() = None;
        let sent = sent_within(&notices, Duration::from_secs(3));
        assert_eq!(sent.first(), Some(&("Levothyroxine · 75 µg".to_string(), 0)), "{sent:?}");
        assert!(sent.iter().any(|(title, id)| title == CLOSED && *id != 0), "taken away by its number: {sent:?}");
        assert!(!sent.iter().any(|(title, _)| title.starts_with("Check first")), "{sent:?}");
        assert!(lates.recv_timeout(Duration::from_secs(1)).is_err(), "never recorded a reminder");
        assert!(presses.recv_timeout(Duration::from_secs(1)).is_err(), "its button gone with it");
        slow.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));

        // The bus stopped while the try is under way: at the server's GetCapabilities, for six seconds.
        let stalling = Arc::new(AtomicBool::new(false));
        assert!(notice_server_on(&address, "stall", pid, told.clone(), Arc::clone(&stalling)));
        let start = Instant::now();
        let outcome = dose("Levothyroxine · 75 µg", Duration::from_secs(60), &pressed, &late, words()).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Maybe(_)) && start.elapsed() < FIRST_TRY, "sent into the stall: maybe ({outcome:?} after {:?})", start.elapsed());
        assert_eq!(lates.recv_timeout(Duration::from_secs(10)).as_deref(), Ok("Levothyroxine · 75 µg"), "let go: shown, late");
        let sent = sent_within(&notices, Duration::from_secs(3));
        assert_eq!(sent.first(), Some(&("Levothyroxine · 75 µg".to_string(), 0)), "{sent:?}");
        assert!(sent.iter().any(|(title, replaces)| title == "Check first: Levothyroxine · 75 µg" && *replaces != 0), "the doubt first, in place: {sent:?}");
        assert_eq!(presses.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Levothyroxine · 75 µg"), "its Taken counts");
        stalling.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));

        // Sent after the tick stopped waiting (the bus stopped a moment while
        // it connects, the tick gone meanwhile): the doubt first at once, once.
        let quick = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "answers", told.clone(), Arc::clone(&quick)));
        StandIns::signal(pid, "-STOP");
        drop(dose("Levothyroxine · 75 µg", Duration::from_secs(60), &pressed, &late, words()));
        std::thread::sleep(Duration::from_millis(300));
        StandIns::signal(pid, "-CONT");
        assert_eq!(lates.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Levothyroxine · 75 µg"), "recorded shown");
        assert_eq!(sent_within(&notices, Duration::from_secs(2)), [("Check first: Levothyroxine · 75 µg".to_string(), 0)], "the doubt first at once, one notice");
        assert_eq!(presses.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Levothyroxine · 75 µg"), "its Taken counts");
        assert!(presses.recv_timeout(Duration::from_secs(1)).is_err(), "once");

        // Marked before it went, the tick gone: never sent, nor tried again.
        StandIns::signal(pid, "-STOP");
        let gone = words();
        let dropped = dose("Levothyroxine · 75 µg", Duration::from_secs(60), &pressed, &late, Arc::clone(&gone));
        *gone.lock().unwrap() = None;
        drop(dropped);
        std::thread::sleep(Duration::from_millis(300));
        StandIns::signal(pid, "-CONT");
        assert!(sent_within(&notices, Duration::from_secs(8)).is_empty(), "never sent");
        assert!(lates.try_recv().is_err());
        quick.store(true, Ordering::Relaxed);
    }

    /// A relay before the stand-in bus at `bus`, each answer of which comes
    /// `delay` late once its client is connected (after its "BEGIN"), as a
    /// bus on a machine swapping does (the fourth review's slow bus): its
    /// address. Ends with `stop`.
    fn slow_relay(bus: &str, delay: Duration, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> String {
        use std::io::{Read, Write};
        use std::os::unix::net::{UnixListener, UnixStream};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let target = crate::bus::unix_path(bus).expect("a unix:path= address");
        let socket = target.with_file_name("slow-relay");
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let Ok((client, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                };
                let _ = client.set_nonblocking(false);
                let Ok(upstream) = UnixStream::connect(&target) else { continue };
                let begun = Arc::new(AtomicBool::new(false));
                // The client's words go at once; its "BEGIN" ends its authentication.
                let (mut from_client, mut to_bus, seen) = (client.try_clone().unwrap(), upstream.try_clone().unwrap(), Arc::clone(&begun));
                std::thread::spawn(move || {
                    let (mut buffer, mut said) = ([0u8; 65_536], Vec::new());
                    while let Ok(n) = from_client.read(&mut buffer) {
                        if n == 0 {
                            break;
                        }
                        if !seen.load(Ordering::SeqCst) {
                            said.extend_from_slice(&buffer[..n]);
                            if said.windows(7).any(|w| w == b"BEGIN\r\n") {
                                seen.store(true, Ordering::SeqCst);
                            }
                        }
                        if to_bus.write_all(&buffer[..n]).is_err() {
                            break;
                        }
                    }
                    let _ = to_bus.shutdown(std::net::Shutdown::Both);
                });
                // The bus's words, in order, each `delay` late once connected.
                let (mut from_bus, mut to_client) = (upstream, client);
                let (queued, queue) = std::sync::mpsc::channel::<(std::time::Instant, Vec<u8>)>();
                std::thread::spawn(move || {
                    while let Ok((at, bytes)) = queue.recv() {
                        std::thread::sleep(at.saturating_duration_since(std::time::Instant::now()));
                        if to_client.write_all(&bytes).is_err() {
                            break;
                        }
                    }
                    let _ = to_client.shutdown(std::net::Shutdown::Both);
                });
                std::thread::spawn(move || {
                    let mut buffer = [0u8; 65_536];
                    while let Ok(n) = from_bus.read(&mut buffer) {
                        if n == 0 {
                            break;
                        }
                        let at = std::time::Instant::now() + if begun.load(Ordering::SeqCst) { delay } else { Duration::ZERO };
                        if queued.send((at, buffer[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                });
            }
        });
        format!("unix:path={}", crate::bus::escaped(&socket))
    }

    /// A slow session bus: every answer 3.4 s late once connected (the
    /// fourth review's relay: Hello and both matches late, each within the
    /// waits of before). The first try, its matches waited a second each so
    /// that it fits in the tick's wait, does not go out: not shown, said
    /// within `FIRST_TRY`, nothing sent. The try after it waits as a slow
    /// bus needs (`TRY_AGAIN`): sent, its answer late (maybe, then shown),
    /// shown once, the doubt first, recorded shown; its "Taken" counts once.
    /// Sets this process's session bus address: run alone, by name:
    /// `cargo test --release -p sioul-sync notify::tests::a_slow_bus_shows_the_reminder_late_the_doubt_first -- --ignored --exact --nocapture`.
    #[test]
    #[ignore = "starts a stand-in dbus-daemon and sets the session bus address: run alone, by name"]
    fn a_slow_bus_shows_the_reminder_late_the_doubt_first() {
        use crate::bus::StandIns;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc::channel;
        use std::sync::{Arc, Mutex};
        use std::time::Instant;
        let mut stand_ins = StandIns::new("stand-in-slow");
        let (_pid, bus) = stand_ins.start("session");
        let stop = Arc::new(AtomicBool::new(false));
        let address = slow_relay(&bus, Duration::from_millis(3_400), Arc::clone(&stop));
        // SAFETY: run alone, by name; nothing else in this process reads it meanwhile.
        unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
        let (told, notices) = channel::<(Instant, String, u32)>();
        // The server on the bus itself: only Sioul's side is slow.
        assert!(notice_server(&bus, "answers", told, Arc::clone(&stop)), "the stand-in server");
        let (pressed, presses) = channel::<String>();
        let (late, lates) = channel::<String>();
        let start = Instant::now();
        let words = Arc::new(Mutex::new(Some("Check first: Levothyroxine · 75 µg".to_string())));
        let outcome = dose("Levothyroxine · 75 µg", Duration::from_secs(120), &pressed, &late, words).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Unshown(_)) && start.elapsed() < FIRST_TRY, "its matches not answered in time, nothing sent: {outcome:?} after {:?}", start.elapsed());
        assert_eq!(lates.recv_timeout(Duration::from_secs(45)).as_deref(), Ok("Levothyroxine · 75 µg"), "shown by the try after it, recorded");
        assert_eq!(sent_within(&notices, Duration::from_secs(1)), [("Check first: Levothyroxine · 75 µg".to_string(), 0)], "once, the doubt first");
        assert_eq!(presses.recv_timeout(Duration::from_secs(10)).as_deref(), Ok("Levothyroxine · 75 µg"), "its Taken counts");
        assert!(presses.recv_timeout(Duration::from_secs(5)).is_err(), "once");
        stop.store(true, Ordering::Relaxed);
    }

    /// What a dose's reminder records and keeps to, against a stand-in bus
    /// and server (review of battery part C, the fifth review):
    /// - a try the bus refuses (no server): recorded going, then taken back;
    /// - its tries over at their end: promised never again (`over`);
    /// - a phone's alarm due: none during the quiet, promised before it
    ///   begins (`pause`), the tries resumed after it, the doubt first;
    /// - shown, its dose then marked elsewhere: taken away by its number (L2);
    /// - "Taken" pressed before a late answer, the notice kept by the server:
    ///   recorded shown, never said again over the press, which counts once (L3);
    /// - said again in place, the server giving it a number of its own
    ///   (GNOME): followed, closed by that number (L3);
    /// - the lasting notification taken away before its late answer: closed
    ///   once it comes (L4).
    /// Sets this process's session bus address: run alone, by name:
    /// `cargo test --release -p sioul-sync notify::tests::reminders_keep_their_word -- --ignored --exact --nocapture`.
    #[test]
    #[ignore = "starts a stand-in dbus-daemon and sets the session bus address: run alone, by name"]
    fn reminders_keep_their_word() {
        use crate::bus::StandIns;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc::channel;
        use std::sync::{Arc, Mutex};
        use std::time::Instant;
        let mut stand_ins = StandIns::new("stand-in-word");
        let (_pid, address) = stand_ins.start("session");
        // SAFETY: run alone, by name; nothing else in this process reads it meanwhile.
        unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
        let (told, notices) = channel::<(Instant, String, u32)>();
        let (pressed, presses) = channel::<String>();
        let (shown, shows) = channel::<String>();
        let (log, logged) = channel::<String>();
        let words = |title: &str| Arc::new(Mutex::new(Some(format!("Check first: {title}"))));
        let record = |name: &str, settled: &Arc<AtomicBool>| Box::new(StandIn { name: name.into(), late: words(name), shown: shown.clone(), log: Some(log.clone()), recorded: false, settled: Arc::clone(settled) });
        let drain = |within: Duration| -> Vec<String> {
            let until = Instant::now() + within;
            let mut seen = Vec::new();
            while let Ok(item) = logged.recv_timeout(until.saturating_duration_since(Instant::now())) {
                seen.push(item);
            }
            seen
        };
        let unsettled: Arc<AtomicBool> = Arc::default();

        // No server: the first try goes to the bus, which says no: taken back; its end comes: over.
        let start = Instant::now();
        let outcome = dose_as("Iron", Again { end: start + Duration::from_secs(7), quiet: None, record: record("Iron", &unsettled) }, &pressed).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Unshown(_)), "{outcome:?}");
        let seen = drain(Duration::from_secs(9));
        assert_eq!(seen.first().map(String::as_str), Some("going First"), "{seen:?}");
        assert_eq!(seen.get(1).map(String::as_str), Some("refused"), "{seen:?}");
        assert_eq!(seen.get(2).map(String::as_str), Some("unshown"), "{seen:?}");
        assert_eq!(seen.last().map(String::as_str), Some("over"), "{seen:?}");
        assert!(seen.iter().filter(|s| s.starts_with("going Late(Retry)")).count() >= 1, "tried again, the doubt first: {seen:?}");

        // A phone's alarm due from 8 to 16 s: promised, none meanwhile (a server there from 9 s), then shown, the doubt first.
        let start = Instant::now();
        let unix = 1_800_000_016;
        let again = Again { end: start + Duration::from_secs(40), quiet: Some((start + Duration::from_secs(8), start + Duration::from_secs(16), unix)), record: record("Zinc", &unsettled) };
        assert!(matches!(dose_as("Zinc", again, &pressed).outcome(FIRST_TRY), Outcome::Unshown(_)));
        std::thread::sleep(Duration::from_secs(9).saturating_sub(start.elapsed()));
        let up = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "answers", told.clone(), Arc::clone(&up)));
        assert_eq!(shows.recv_timeout(Duration::from_secs(20)).as_deref(), Ok("Zinc"), "shown after the quiet");
        assert!(start.elapsed() >= Duration::from_secs(16), "none during the quiet: {:?}", start.elapsed());
        let seen = drain(Duration::from_millis(300));
        assert!(seen.contains(&format!("pause {unix}")), "promised before it began: {seen:?}");
        assert_eq!(sent_within(&notices, Duration::from_millis(300)).first().map(|(title, _)| title.as_str()), Some("Check first: Zinc"));
        let _ = presses.recv_timeout(Duration::from_secs(3));
        up.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));

        // Shown, then marked elsewhere: taken away by its number.
        let quiet_server = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "shows", told.clone(), Arc::clone(&quiet_server)));
        let settled = Arc::new(AtomicBool::new(false));
        let outcome = dose_as("Folate", Again { end: Instant::now() + Duration::from_secs(60), quiet: None, record: record("Folate", &settled) }, &pressed).outcome(FIRST_TRY);
        assert_eq!(outcome, Outcome::Shown);
        let first = sent_within(&notices, Duration::from_millis(500));
        assert_eq!(first.len(), 1, "{first:?}");
        settled.store(true, Ordering::SeqCst);
        let closed = sent_within(&notices, Duration::from_secs(4));
        assert!(closed.iter().any(|(title, id)| title == CLOSED && *id != 0), "taken away: {closed:?}");
        quiet_server.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));
        let _ = shows.try_iter().count();

        // "Taken" pressed before a late answer, the notice kept: recorded shown, never said again, the press once.
        let pressing = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "press-first", told.clone(), Arc::clone(&pressing)));
        let outcome = dose("Iodine", Duration::from_secs(60), &pressed, &shown, words("Iodine")).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Maybe(_)), "{outcome:?}");
        assert_eq!(shows.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Iodine"), "recorded shown at its late answer");
        assert_eq!(presses.recv_timeout(Duration::from_secs(3)).as_deref(), Ok("Iodine"), "its Taken counts");
        assert!(presses.recv_timeout(Duration::from_secs(2)).is_err(), "once");
        let sent = sent_within(&notices, Duration::from_millis(500));
        assert_eq!(sent, [("Iodine".to_string(), 0)], "never said again over the press");
        pressing.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));

        // Said again in place, given a number of its own (GNOME): followed, closed by it.
        let renumbering = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "renumber", told.clone(), Arc::clone(&renumbering)));
        let settled = Arc::new(AtomicBool::new(false));
        let outcome = dose_as("Calcium", Again { end: Instant::now() + Duration::from_secs(60), quiet: None, record: record("Calcium", &settled) }, &pressed).outcome(FIRST_TRY);
        assert!(matches!(outcome, Outcome::Maybe(_)), "{outcome:?}");
        assert_eq!(shows.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Calcium"));
        let sent = sent_within(&notices, Duration::from_secs(1));
        let replaced = sent.iter().find(|(title, replaces)| title == "Check first: Calcium" && *replaces != 0).map(|(_, replaces)| *replaces);
        assert!(replaced.is_some(), "said again in place: {sent:?}");
        settled.store(true, Ordering::SeqCst);
        let closed = sent_within(&notices, Duration::from_secs(4));
        assert!(closed.iter().any(|(title, id)| title == CLOSED && Some(*id) == replaced.map(|r| r + 100)), "closed by its new number: {closed:?}");
        renumbering.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));

        // The lasting notification closed before its late answer: closed once the answer comes.
        let late_server = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "late", told.clone(), Arc::clone(&late_server)));
        let lasting = ongoing("Write to the bank", "Since 14:02, for 25 min", vec![("stop".to_string(), "Stop".to_string())], Box::new(|_: &str| {}));
        std::thread::sleep(Duration::from_millis(500));
        lasting.close(Duration::from_millis(500));
        let seen = sent_within(&notices, Duration::from_secs(6));
        assert_eq!(seen.first().map(|(title, _)| title.as_str()), Some("Write to the bank"), "{seen:?}");
        assert!(seen.iter().any(|(title, id)| title == CLOSED && *id != 0), "closed by its number once answered: {seen:?}");
        late_server.store(true, Ordering::Relaxed);
    }

    /// The focus session's lasting notification against a server that shows
    /// it at once and answers three seconds late, on a bus of the test's own
    /// (dbus-run-session; skipped where a server is there already): never sent
    /// again while not answered (shown twice, neither taken away: review of
    /// battery part C, the fourth review's L1); a change made meanwhile goes in
    /// place of the number the late answer gave; closed by that number.
    #[test]
    fn a_lasting_notice_answered_late_is_never_shown_twice() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let _alone = crate::bus::NOTIFICATION_SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let Some(address) = crate::bus::session_address() else {
            eprintln!("No bus of the test's own (dbus-run-session): skipped.");
            return;
        };
        let (told, notices) = std::sync::mpsc::channel();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        // Never in a server's place: a stand-in of a test before given a moment to leave.
        let until = std::time::Instant::now() + Duration::from_secs(2);
        while !notice_server(&address, "late", told.clone(), std::sync::Arc::clone(&stop)) {
            if std::time::Instant::now() >= until {
                eprintln!("No bus of the test's own (dbus-run-session), or a notification server on it already: skipped.");
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let lasting = ongoing("Write to the bank", "Since 14:02, for 25 min", vec![("stop".to_string(), "Stop".to_string())], Box::new(|_: &str| {}));
        std::thread::sleep(Duration::from_secs(1));
        lasting.update("Write to the bank", "Paused, 1 min so far", vec![("resume".to_string(), "Go on".to_string())]);
        let sent = sent_within(&notices, Duration::from_secs(9));
        assert_eq!(sent.first(), Some(&("Write to the bank".to_string(), 0)), "{sent:?}");
        assert!(sent.iter().skip(1).all(|(_, replaces)| *replaces != 0), "never a second with no number to replace: {sent:?}");
        assert!(sent.len() >= 2, "the change went, in place: {sent:?}");
        lasting.close(Duration::from_secs(5));
        stop.store(true, Ordering::Relaxed);
    }

    /// Codes, reminders and mail, against a notification server of the test's
    /// own on a bus of its own (run under dbus-run-session; skipped where a
    /// server is there already: nothing here may reach the desktop's). Each
    /// goes as notify-rust's `Notification` sent it (`send`): "Sioul", the
    /// icon, the title, the body as a server reading markup takes it, the
    /// buttons, no sound, Sioul's desktop file, ten minutes; its urgency said
    /// once (a mail at normal urgency, unless someone Always through is in
    /// it). A button pressed at the server runs its action, here, once.
    #[test]
    fn notifications_go_as_before_and_hear_their_button() {
        use dbus::blocking::LocalConnection;
        use dbus::channel::{MatchingReceiver, Sender as _};
        use dbus::message::MatchRule;
        use std::sync::mpsc::channel;

        /// What the stand-in server was sent.
        #[derive(Debug, PartialEq)]
        struct Sent {
            app: String,
            replaces: u32,
            icon: String,
            title: String,
            body: String,
            buttons: Vec<String>,
            hints: Vec<String>,
            sound_off: bool,
            entry: String,
            urgency: u8,
            timeout: i32,
        }
        let _alone = crate::bus::NOTIFICATION_SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let (told, sent) = channel::<Sent>();
        let (ready, started) = channel::<bool>();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            let Some(bus) = stand_in_bus() else {
                let _ = ready.send(false);
                return;
            };
            let mut next = 20u32;
            bus.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |call: dbus::Message, bus: &LocalConnection| {
                    let mut press = None;
                    let reply = match call.member().as_deref() {
                        Some("GetCapabilities") => call.method_return().append1(vec!["actions", "body", "body-markup"]),
                        Some("Notify") => {
                            let (app, replaces, icon, title, body, buttons, hints, timeout): (String, u32, String, String, String, Vec<String>, dbus::arg::PropMap, i32) = call.read_all().unwrap();
                            next += 1;
                            // The last button, pressed once shown.
                            if buttons.len() >= 2 {
                                press = Some(buttons[buttons.len() - 2].clone());
                            }
                            let _ = told.send(Sent {
                                app,
                                replaces,
                                icon,
                                title,
                                body,
                                buttons,
                                hints: hint_names(&call),
                                sound_off: dbus::arg::prop_cast::<bool>(&hints, "suppress-sound").copied().unwrap_or(false),
                                entry: dbus::arg::prop_cast::<String>(&hints, "desktop-entry").cloned().unwrap_or_default(),
                                urgency: dbus::arg::prop_cast::<u8>(&hints, "urgency").copied().unwrap_or(9),
                                timeout,
                            });
                            call.method_return().append1(next)
                        }
                        _ => return true,
                    };
                    let _ = bus.send(reply);
                    if let Some(key) = press {
                        let mut signal = dbus::Message::new_signal(SERVER_PATH, SERVER, "ActionInvoked").unwrap().append2(next, key);
                        // To the connection that showed it alone, as Plasma does.
                        signal.set_destination(call.sender().map(|s| s.into_static()));
                        let _ = bus.send(signal);
                    }
                    true
                }),
            );
            let _ = ready.send(true);
            let until = std::time::Instant::now() + Duration::from_secs(30);
            while !stopped.load(std::sync::atomic::Ordering::Relaxed) && std::time::Instant::now() < until {
                let _ = bus.process(Duration::from_millis(100));
            }
        });
        if !started.recv_timeout(Duration::from_secs(5)).unwrap_or(false) {
            eprintln!("No bus of the test's own (dbus-run-session), or a notification server on it already: skipped.");
            return;
        }
        let wait = Duration::from_secs(5);
        let as_before = |icon: &str, title: &str, body: &str, buttons: &[&str], urgency: u8| Sent {
            app: "Sioul".into(),
            replaces: 0,
            icon: icon.into(),
            title: title.into(),
            body: body.into(),
            buttons: buttons.iter().map(|b| b.to_string()).collect(),
            hints: vec!["desktop-entry".into(), "suppress-sound".into(), "urgency".into()],
            sound_off: true,
            entry: DESKTOP_ENTRY.into(),
            urgency,
            timeout: 600_000,
        };
        // A code with its button: pressed at the server, copied here.
        let (copied, copies) = channel::<()>();
        code("Code from example.org", "482913 <b>now</b> & then", Some(("Copy".into(), Box::new(move || {
            let _ = copied.send(());
        })))).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("dialog-password", "Code from example.org", "482913 &lt;b&gt;now&lt;/b&gt; &amp; then", &["copy", "Copy"], CRITICAL));
        assert_eq!(copies.recv_timeout(wait), Ok(()));
        // Mail: normal urgency, said once; critical when someone Always through is in it.
        mail("New mail", "From Noa", false, None).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("mail-unread", "New mail", "From Noa", &[], NORMAL));
        mail("New mail", "From Noa", true, None).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("mail-unread", "New mail", "From Noa", &[], CRITICAL));
        // A reminder; and one with choices, the one pressed run alone.
        remind("Magnesium", "300 mg", None).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("appointment-soon", "Magnesium", "300 mg", &[], CRITICAL));
        let (chose, choices) = channel::<&str>();
        let other = chose.clone();
        remind_choices("The night", "Close the day?", vec![("Close the day".into(), Box::new(move || {
                let _ = chose.send("close");
            })), ("Options…".into(), Box::new(move || {
                let _ = other.send("options");
            }))]).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("appointment-soon", "The night", "Close the day?", &["choice-0", "Close the day", "choice-1", "Options…"], CRITICAL));
        assert_eq!(choices.recv_timeout(wait), Ok("options"));
        assert!(choices.recv_timeout(Duration::from_millis(300)).is_err(), "one choice run");
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// The time running, against a notification server of the test's own on a
    /// bus of its own: run under dbus-run-session. Where a server is there
    /// already (the desktop's), skipped: nothing here may reach it.
    #[test]
    fn a_lasting_notification_changes_in_place_hears_its_buttons_and_goes() {
        use dbus::blocking::LocalConnection;
        use dbus::channel::{MatchingReceiver, Sender as _};
        use dbus::message::MatchRule;
        use std::sync::mpsc::channel;

        /// What the stand-in server was asked.
        #[derive(Debug, PartialEq)]
        enum Asked {
            Notify { replaces: u32, title: String, body: String, actions: Vec<String>, resident: bool, urgency: u8, timeout: i32 },
            Close(u32),
        }
        let _alone = crate::bus::NOTIFICATION_SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let (told, asked) = channel::<Asked>();
        let (ready, started) = channel::<bool>();
        std::thread::spawn(move || {
            // Never in a server's place: no replacing it, no waiting in line.
            let Some(bus) = stand_in_bus() else {
                let _ = ready.send(false);
                return;
            };
            let finished = std::rc::Rc::new(std::cell::Cell::new(false));
            let over = std::rc::Rc::clone(&finished);
            let mut notified = 0;
            bus.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |call: dbus::Message, bus: &LocalConnection| {
                    let mut presses: Vec<(u32, &str)> = Vec::new();
                    let reply = match call.member().as_deref() {
                        Some("GetCapabilities") => call.method_return().append1(vec!["actions", "body", "body-markup", "persistence"]),
                        Some("Notify") => {
                            let (_, replaces, _, title, body, actions, hints, timeout): (String, u32, String, String, String, Vec<String>, dbus::arg::PropMap, i32) = call.read_all().unwrap();
                            let resident = dbus::arg::prop_cast::<bool>(&hints, "resident").copied().unwrap_or(false);
                            let urgency = dbus::arg::prop_cast::<u8>(&hints, "urgency").copied().unwrap_or(9);
                            let _ = told.send(Asked::Notify { replaces, title, body, actions, resident, urgency, timeout });
                            notified += 1;
                            // Pressed once shown; then another notification's button, and this one's.
                            presses = match notified {
                                1 => vec![(7, "pause")],
                                2 => vec![(8, "stop"), (7, "resume")],
                                _ => Vec::new(),
                            };
                            call.method_return().append1(7u32)
                        }
                        Some("CloseNotification") => {
                            let (id,): (u32,) = call.read_all().unwrap();
                            let _ = told.send(Asked::Close(id));
                            over.set(true);
                            call.method_return()
                        }
                        _ => return true,
                    };
                    let _ = bus.send(reply);
                    for (id, key) in presses {
                        let mut press = dbus::Message::new_signal(SERVER_PATH, SERVER, "ActionInvoked").unwrap().append2(id, key);
                        // To the connection that showed it alone, as Plasma does.
                        press.set_destination(call.sender().map(|s| s.into_static()));
                        let _ = bus.send(press);
                    }
                    true
                }),
            );
            let _ = ready.send(true);
            let until = std::time::Instant::now() + Duration::from_secs(20);
            while !finished.get() && std::time::Instant::now() < until {
                let _ = bus.process(Duration::from_millis(100));
            }
        });
        if !started.recv_timeout(Duration::from_secs(5)).unwrap_or(false) {
            eprintln!("No bus of the test's own (dbus-run-session), or a notification server on it already: skipped.");
            return;
        }
        let wait = Duration::from_secs(5);
        let buttons = |first: &str, label: &str| vec![(first.to_string(), label.to_string()), ("stop".to_string(), "Stop".to_string()), ("default".to_string(), "Open".to_string())];
        let (pressed, presses) = channel::<String>();
        let shown = ongoing("Write to the bank", "Since 14:02, for 25 min", buttons("pause", "Pause"), Box::new(move |key: &str| drop(pressed.send(key.to_string()))));
        let first = Asked::Notify {
            replaces: 0,
            title: "Write to the bank".into(),
            body: "Since 14:02, for 25 min".into(),
            actions: ["pause", "Pause", "stop", "Stop", "default", "Open"].map(String::from).to_vec(),
            resident: true,
            urgency: 1,
            // The server keeps it once its popup goes: its own time.
            timeout: -1,
        };
        assert_eq!(asked.recv_timeout(wait).unwrap(), first);
        // A button pressed at the server, heard here.
        assert_eq!(presses.recv_timeout(wait).unwrap(), "pause");
        // Changed in place; another notification's button is not this one's.
        shown.update("Write to the bank", "Paused, 3 min so far", buttons("resume", "Go on"));
        match asked.recv_timeout(wait).unwrap() {
            Asked::Notify { replaces, body, actions, .. } => assert_eq!((replaces, body.as_str(), actions[0].as_str()), (7, "Paused, 3 min so far", "resume")),
            other => panic!("{other:?}"),
        }
        assert_eq!(presses.recv_timeout(wait).unwrap(), "resume");
        // Taken away before Sioul goes on.
        shown.close(wait);
        assert_eq!(asked.recv_timeout(wait).unwrap(), Asked::Close(7));
        assert!(presses.try_recv().is_err(), "nothing else was pressed");
    }
}

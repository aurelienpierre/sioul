// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Do-not-disturb on every device, as this device applies and says it
//! (docs/do-not-disturb.md; the rules are `sioul_core::everywhere`). The one
//! caller of the do-not-disturb layer (`crate::dnd`): the pauses
//! (`pauses::apply`), the switch, the focus timer and the minute all come
//! here, and every device reads the same shared files: `quiet.toml` (the
//! pauses), `time/running.toml` (focus), Health (the night),
//! `state/do-not-disturb.toml` (the switch) and the `[dnd]` settings.
//!
//! - **Apply**: the system's do-not-disturb on for each reason that holds,
//!   off for each that does not; nothing redone when nothing changed (the
//!   layer keeps what it did). On Android, only in the window's process (where
//!   the layer's modes are kept): the background service asks it to
//!   (`steps`), and an alarm comes back at the next end (`steps::next`).
//! - **Say**: this device's table in the switch's file tells the others what
//!   holds here and whether its system is silenced; the status line, the
//!   phone's card and the service's notification say where it holds.
//! - **The list**: Always through (« Passent toujours »), edited in
//!   Settings ▸ Do not disturb; its people get through as its rows of the
//!   matrix of what reaches you say (`sioul_core::attention`), and on a phone
//!   Sioul says who is not starred there.
//! - **Both ways**: a change of this device's own do-not-disturb, heard as it
//!   happens, presses the switch when it disagrees with what holds here
//!   (`heard_from_system`); Sioul's going off here turns the system's own off too, where
//!   Sioul can (`apply`), and says what stays.

use crate::backend::qobject::Sioul;
use crate::backend::{QtThread, Shared, load_config, mode_json, say, tr};
use crate::dnd::{Ask, Report, Which};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use jiff::Zoned;
use std::pin::Pin;
use sioul_core::config::Config;
use sioul_core::everywhere::{self as rules, Now, People, Person, Sources, Switch, Why};
use sioul_core::quiet::Overrides;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// This device's name in the sharing (share/here.toml), made once.
fn here_id() -> String {
    sioul_sync::share::Here::load(&sioul_core::config::state_dir()).id
}

/// What the reasons' sources say now.
fn sources(overrides: &Overrides, blocks: &sioul_core::quiet::Blocks, now: &Zoned) -> Sources {
    let stamp = now.timestamp().as_second();
    let free = overrides.free_from().map(|since| (since, sioul_core::pause::free_until(since, blocks, now.time_zone())));
    let sleep = blocks.at(stamp, &["sleep", "nap"]).map(|k| (k.start, k.end));
    let focus = sioul_core::timelog::running().and_then(|r| rules::focus(&r, stamp));
    Sources { paused: overrides.paused_from(), free, sleep, focus }
}

/// Health's nights, naps and meals: as the window has them; in the background
/// service's process, without the day's events (only the meals move for them,
/// and reading every calendar at each step would cost a phone's battery).
fn blocks(now: &Zoned) -> sioul_core::quiet::Blocks {
    if crate::steps::in_service() { sioul_core::quiet::Blocks::read(now, &[]) } else { crate::hours::blocks(now) }
}

/// Asleep (the night from winding down, a nap), and paused, now: for the
/// background service's rhythm, whatever the settings of do-not-disturb.
pub(crate) fn rest_now() -> (bool, bool) {
    let now = Zoned::now();
    let paused = Overrides::load(&Overrides::default_path()).paused_from().is_some();
    (blocks(&now).at(now.timestamp().as_second(), &["sleep", "nap"]).is_some(), paused)
}

/// What is read for one look: the settings, the pauses, the switch, why it
/// holds, and what now is for (the time under do-not-disturb's layer).
struct Look {
    config: Config,
    overrides: Overrides,
    switch: Switch,
    state: Now,
    mode: sioul_core::quiet::Mode,
    now: Zoned,
}

impl Look {
    fn now() -> Look {
        let config = load_config();
        let now = Zoned::now();
        let overrides = Overrides::load(&Overrides::default_path());
        let switch = Switch::load(&Switch::default_path());
        let blocks = blocks(&now);
        let state = rules::now(&config.dnd, &sources(&overrides, &blocks, &now), switch.latest().as_ref(), now.timestamp().as_second());
        let mode = sioul_core::quiet::mode(&config.week_hours(), &config.time_off, &overrides, &blocks, &now);
        Look { config, overrides, switch, state, mode, now }
    }

    /// What each mode of Sioul's asks of the system now; none: off. Whom it
    /// lets through is the matrix of what reaches you's (`attention::silence_at`):
    /// at the pause's column; at Free time's ("Nothing at all" holding all but
    /// Always through); do-not-disturb's at sleep's, or at the time now under
    /// the switch's or a focus session's layer, as the matrix reads a layer.
    /// While this phone screens calls, every contact's call goes through the
    /// mode, Sioul having declined the others already, else the starred; and
    /// anyone's while "Let every call through" holds.
    fn asks(&self) -> [(Which, Option<Ask>); 3] {
        use sioul_core::attention::{Column, Now as Moment, Senders};
        let (config, state) = (&self.config, &self.state);
        let gnome = config.pause.gnome;
        let paused = state.has(Why::Paused);
        let free = state.has(Why::FreeTime) && !paused;
        let attention = sioul_core::attention::Attention::of(config);
        let phone = phone_does();
        let mode = &self.mode;
        let through = self.through();
        let moment = |times: Vec<Column>, dnd: bool, nothing: bool| {
            let mut moment = Moment::of(mode);
            moment.times = times;
            moment.dnd = dnd;
            moment.nothing = nothing;
            moment.through = through;
            moment
        };
        let ask = |which: Which, moment: Moment| {
            let silence = attention.silence_at(&moment, phone);
            Ask { which, people: silence.calls != Senders::None, doses: silence.doses, desktop: gnome, silence: Some(silence), stack: true }
        };
        // The switch's mode adds nothing over this device's own do-not-disturb
        // when that alone turned it on (docs/do-not-disturb.md, "Both ways").
        let latest = self.switch.latest();
        let from_system = latest.as_ref().is_some_and(|p| p.on && p.via == rules::VIA_SYSTEM);
        let stack = !(from_system && rules::held_by_system(state, latest.as_ref(), &here_id()));
        let global = if state.has(Why::Sleep) { vec![Column::Sleep] } else { Moment::of(mode).times };
        [
            (Which::Paused, paused.then(|| ask(Which::Paused, moment(vec![Column::Pause], false, false)))),
            (Which::FreeTime, free.then(|| ask(Which::FreeTime, moment(vec![Column::Free], false, sioul_core::pause::nothing_now(&self.overrides, &config.free_time))))),
            (Which::Global, state.global().then(|| Ask { stack, ..ask(Which::Global, moment(global, state.gates(), false)) })),
        ]
    }

    /// "Let every call through" holds now, as this device knows it.
    fn through(&self) -> bool {
        crate::calls::through_now(&self.switch).is_some_and(|t| t.holds(self.now.timestamp().as_millisecond()))
    }

    /// What this device asks, in a few words: the background service compares
    /// it with what was applied last, and asks the window's process only on a change.
    fn signature(&self) -> String {
        self.asks().iter().map(|(which, ask)| format!("{which:?}:{}", ask.as_ref().map_or_else(|| "-".to_string(), |a| format!("{}{}{}{}{}", u8::from(a.people), u8::from(a.doses), u8::from(a.desktop), u8::from(a.stack), a.silence.map(|s| format!("{:?}{:?}{}{}{}", s.calls, s.messages, u8::from(s.repeat), u8::from(s.conversations), u8::from(s.events))).unwrap_or_default())))).collect::<Vec<_>>().join(",")
    }

    /// When to look again by itself: a reason's end, the next night or nap
    /// when sleep counts, the time's end while do-not-disturb's layer reads
    /// it, the end of "Let every call through" (Unix seconds).
    fn next_change(&self) -> Option<i64> {
        let stamp = self.now.timestamp().as_second();
        let night = if self.config.dnd.sleep { blocks(&self.now).kept.iter().filter(|k| (k.kind == "sleep" || k.kind == "nap") && k.start > stamp).map(|k| k.start).min() } else { None };
        let time = if self.state.gates() { self.mode.until.as_ref().map(|u| u.timestamp().as_second()) } else { None };
        let through = crate::calls::through_now(&self.switch).filter(|t| t.holds(stamp * 1000) && t.until > 0).map(|t| (t.until + 999) / 1000);
        [self.state.next_end(), night, time, through].into_iter().flatten().filter(|t| *t > stamp).min()
    }
}

/// What this phone does itself, for whom Sioul's modes let through: it
/// screens calls (Sioul holds the role). A computer nothing. Nothing of
/// the listener, though it may hold other apps' notifications:
/// Android plays a notification's sound before any listener hears of it
/// (docs/android.md, "Notifications from other apps", Sound), so a contact's
/// message let through the mode and then held by Sioul would ring once in a
/// pause or Free time; the mode is what keeps it silent. Calls differ: the
/// screening answers before the phone rings.
fn phone_does() -> sioul_core::attention::Phone {
    if !cfg!(target_os = "android") {
        return sioul_core::attention::Phone::default();
    }
    sioul_core::attention::Phone { screens: crate::calls::screens_here() }
}

/// Why do-not-disturb holds now, on every device alike.
pub(crate) fn now() -> Now {
    Look::now().state
}

/// Whether the switch or a focus session holds do-not-disturb now: a layer
/// of the matrix of what reaches you (`hours::attention_now`), which says
/// what waits then (as usual, the sites' notifications and the suggestions
/// to move; doses, codes asked for, events' reminders and Health's notices
/// come, and Always through). Read again at most every five seconds: asked
/// at every notification.
pub(crate) fn holds_others() -> bool {
    static KEPT: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    if let Ok(kept) = KEPT.lock()
        && let Some((at, holds)) = *kept
        && at.elapsed() < Duration::from_secs(5)
    {
        return holds;
    }
    let holds = now().gates();
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((Instant::now(), holds));
    }
    holds
}

/// While do-not-disturb holds from its switch or a focus session (a layer
/// of the matrix of what reaches you): when it ends, when that is known
/// (none: until turned off, or a session open-ended); none otherwise.
pub(crate) fn gate() -> Option<Option<i64>> {
    let state = now();
    let gates: Vec<&sioul_core::everywhere::Held> = state.holds.iter().filter(|h| matches!(h.why, Why::Focus | Why::Manual)).collect();
    let until = if gates.iter().any(|h| h.until.is_none()) { None } else { gates.iter().filter_map(|h| h.until).max() };
    (!gates.is_empty()).then_some(until)
}

// ---------------------------------------------------------------- applying

/// Asked again while an apply runs: it goes once more, with what changed meanwhile.
static AGAIN: AtomicBool = AtomicBool::new(false);
static BUSY: Mutex<()> = Mutex::new(());

/// The system's do-not-disturb as the reasons say now, on this device: on
/// for each of Sioul's modes that holds, off for the others (the layer does
/// nothing when nothing changed); this device's table written when what it
/// says changed; the phone's card told; on Android, an alarm at the next end.
/// Called at the start, each minute, at once on a press, and after an
/// exchange brought a change. Never in the background service's process
/// (Android), which asks the window's process instead (`steps`).
pub(crate) fn apply() {
    AGAIN.store(true, Ordering::SeqCst);
    while AGAIN.load(Ordering::SeqCst) {
        let Some(busy) = crate::backend::one_at_a_time(&BUSY) else { return };
        if !AGAIN.swap(false, Ordering::SeqCst) {
            return;
        }
        apply_once();
        drop(busy);
    }
}

fn apply_once() {
    let look = Look::now();
    let mut reports: Vec<(Which, Report)> = Vec::new();
    for (which, ask) in look.asks() {
        match ask {
            Some(ask) => reports.push((which, crate::dnd::enter(&ask))),
            None => {
                crate::dnd::leave(which);
            }
        }
    }
    // This device's words: those of the mode of the reason said first.
    let said = match look.state.why() {
        Some(Why::Paused) => Which::Paused,
        Some(Why::FreeTime) => Which::FreeTime,
        _ => Which::Global,
    };
    let report = reports.iter().find(|(w, _)| *w == said).or(reports.first()).map(|(_, r)| r.clone()).unwrap_or_default();
    let why = look.state.why().map(Why::id).unwrap_or_default().to_string();
    // Both ways (docs/do-not-disturb.md): Sioul's do-not-disturb gone off here
    // turns the system's own off too, once, where Sioul can; while it is off,
    // what the system still holds here is said.
    let on = look.state.on();
    crate::dnd::tell_system(on);
    let was = crate::dnd::on_here(on);
    let acted = was && !on && crate::dnd::system_off(look.config.pause.gnome);
    let still = if on || acted { None } else { crate::dnd::still_on() };
    write_own(|own| {
        // Sioul runs here (again): it follows.
        own.closed = false;
        own.why = why.clone();
        own.silenced = report.on && !why.is_empty();
        own.turned_off = report.turned_off && !why.is_empty();
        own.system_on = still.is_some();
        own.line = match &still {
            _ if !why.is_empty() => report.line.clone(),
            Some(still) => still.line.clone(),
            None => String::new(),
        };
    });
    crate::homecard::dnd_seen(moment());
    // On a phone, Android's alarm comes back at the next end, Sioul closed or not.
    crate::steps::next(look.next_change());
    // The calls' table follows the pauses, the times and the switch at once (`calls::refresh`:
    // written again only when what it is made of changed, or every ten minutes).
    crate::calls::refresh(false);
}

/// What this device asks of its system now, in a few words, and the status
/// line: the background service pokes the window's process when either
/// changed since it last did (`steps`).
pub(crate) fn wanted() -> (String, String) {
    let look = Look::now();
    (look.signature(), moment_of(&look)["line"].as_str().unwrap_or_default().to_string())
}

/// This device's own table changed (its kind and name always set), under
/// the file's lock; written only when it changed. Sharing off, it is kept
/// here all the same: the switch works on this device alone.
fn write_own(change: impl FnOnce(&mut rules::Device)) {
    let here = here_id();
    let now = jiff::Timestamp::now().as_millisecond();
    let result = rules::change_own(&Switch::default_path(), &here, |switch| {
        let after = switch.latest_known(None) + 1;
        let own = switch.device.entry(here.clone()).or_default();
        let before = own.clone();
        change(own);
        own.kind = crate::devices::kind().to_string();
        own.name = if own.kind == "phone" { String::new() } else { crate::devices::name() };
        let changed = *own != before;
        if changed {
            // Never before a press already known here: the others read it as written after it.
            own.at = now.max(after);
        }
        changed
    });
    if let Err(e) = result {
        eprintln!("sioul: do-not-disturb: {e}");
    }
}

/// The switch pressed in the window: on for `minutes` (0: until turned off;
/// -1: until what now is for ends), or off. Applied here, the status line
/// shown again, and the other devices told at once.
pub(crate) fn toggle(mut sioul: Pin<&mut Sioul>, on: bool, minutes: i32) {
    let now = Zoned::now();
    let until = match minutes {
        m if m > 0 => Some(now.timestamp().as_second() + i64::from(m) * 60),
        -1 => crate::hours::mode_at(&now).until.map(|u| u.timestamp().as_second()),
        _ => None,
    };
    if let Err(e) = press(on, until.filter(|_| on)) {
        sioul.as_mut().set_status(QString::from(&e));
        return;
    }
    let (qt, shared) = (sioul.qt_thread(), sioul.shared());
    std::thread::spawn(move || {
        apply();
        let _ = qt.queue(|mut sioul| sioul.as_mut().set_mode(QString::from(&mode_json())));
        crate::share::exchange(&qt, &shared);
    });
}

/// The switch pressed here: on until `until` (Unix seconds; none, until
/// turned off), or off; on every device once the sharing carries it. Applied
/// by the caller (`apply`).
pub(crate) fn press(on: bool, until: Option<i64>) -> Result<(), String> {
    press_via(on, until, "")
}

/// A press here, saying where it came from (`rules::VIA_SYSTEM`: this
/// device's own do-not-disturb, heard as it changed; "": the switch).
fn press_via(on: bool, until: Option<i64>, via: &str) -> Result<(), String> {
    let here = here_id();
    let now = jiff::Timestamp::now().as_millisecond();
    rules::change_own(&Switch::default_path(), &here, |switch| {
        switch.press_via(&here, on, until.map_or(0, |u| u.saturating_mul(1000)), now, via);
        let own = switch.device.entry(here.clone()).or_default();
        own.kind = crate::devices::kind().to_string();
        own.name = if own.kind == "phone" { String::new() } else { crate::devices::name() };
        true
    })?;
    Ok(())
}

// ---------------------------------------------------------------- both ways

/// One heard change at a time: two words of the same change (Android's
/// status and its filter, two of Plasma's) press once.
static HEARING: Mutex<()> = Mutex::new(());

/// A change of this device's own do-not-disturb, heard as it happened
/// (Android's receiver, Plasma's `Inhibited`, GNOME's switch), whatever
/// caused it: a press of the switch here when it disagrees with what holds
/// (`rules::heard`), applied, shown and sent at once; nothing when it agrees,
/// which is what Sioul's own changes look like when they come back
/// (docs/do-not-disturb.md, "Both ways"). Whether it pressed. Any thread.
pub(crate) fn heard_from_system(system_on: bool) -> bool {
    let _one = HEARING.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(on) = rules::heard(now().on(), system_on) else { return false };
    if let Err(e) = press_via(on, None, rules::VIA_SYSTEM) {
        eprintln!("sioul: do-not-disturb: {e}");
        return false;
    }
    apply();
    told();
    true
}

/// The window, when it runs in this process: what is heard or pressed
/// outside it shows there at once, and is sent with its exchange.
static WINDOW: Mutex<Option<(QtThread, Arc<Shared>)>> = Mutex::new(None);

/// The window runs (at the start and each minute, from the pauses' tick):
/// kept, and this system's own do-not-disturb followed (`dnd::listen`), once.
pub(crate) fn started(qt: &QtThread, shared: &Arc<Shared>) {
    let mut window = WINDOW.lock().unwrap_or_else(PoisonError::into_inner);
    if window.is_none() {
        *window = Some((qt.clone(), Arc::clone(shared)));
        crate::dnd::listen(Arc::new(|on| {
            std::thread::spawn(move || heard_from_system(on));
        }));
    }
}

/// A press made outside the window (a system's change heard, the phone's
/// tile): the window's status line at once and the other devices told;
/// without the window (a phone's receiver), the background service sends it.
fn told() {
    let window = WINDOW.lock().ok().and_then(|window| window.clone());
    match window {
        Some((qt, shared)) => {
            let _ = qt.queue(|mut sioul| sioul.as_mut().set_mode(QString::from(&mode_json())));
            crate::share::exchange(&qt, &shared);
        }
        None => crate::steps::send_now(),
    }
}

/// Sioul quits on a computer (its window closed, the tray's Quit, SIGTERM):
/// this device's table as it then holds, for the other devices to say
/// "On laptop: Sioul is closed." and never "silenced" by an inhibition that
/// ended with Sioul; sent by the exchange that follows (`share::closing`).
/// Nothing on a phone, whose modes stay while Sioul is closed.
pub(crate) fn closing() {
    if cfg!(target_os = "android") {
        return;
    }
    let still = crate::dnd::still_after_quit();
    write_own(|own| own.closing(still));
}

/// The switch pressed from the phone's quick settings (Sioul's tile): on
/// until turned off, or off, as it is not now; applied and sent at once.
pub(crate) fn toggle_here() {
    if let Err(e) = press(!now().on(), None) {
        eprintln!("sioul: do-not-disturb: {e}");
        return;
    }
    apply();
    told();
}

/// "Silence this device again": each of Sioul's modes that holds here left
/// and entered afresh, the person's earlier "off" in the system set aside. A
/// switch "on" taken from this device's own system becomes the switch's own,
/// so that Sioul's mode silences the device whatever the system does.
pub(crate) fn again() {
    if Look::now().switch.latest().is_some_and(|p| p.on && p.via == rules::VIA_SYSTEM)
        && let Err(e) = press(true, None)
    {
        eprintln!("sioul: do-not-disturb: {e}");
    }
    for ask in Look::now().asks().into_iter().filter_map(|(_, ask)| ask) {
        crate::dnd::again(&ask);
    }
    apply();
    told();
}

// ---------------------------------------------------------------- saying

/// A device named inside a sentence: "your phone", a computer's name, "another device".
fn name_of(switch: &Switch, id: &str) -> String {
    let (kind, name) = switch.device.get(id).filter(|d| !d.kind.is_empty()).map(|d| (d.kind.clone(), d.name.clone())).or_else(|| crate::devices::kind_and_name(id)).unwrap_or_default();
    match kind.as_str() {
        "phone" => tr().text("dnd-device-phone", None),
        _ if !name.trim().is_empty() => name,
        _ => tr().text("dnd-device-other", None),
    }
}

/// The other devices sharing, and when each was last heard from (Unix seconds).
fn heard(here: &str) -> Vec<(String, i64)> {
    let Some((_, Some((folder, _)))) = crate::share::vault() else { return Vec::new() };
    sioul_sync::share::others(&folder, here).into_iter().map(|o| (o.id, o.heard)).collect()
}

/// "17:00", "tomorrow at 07:00".
fn when(stamp: i64, now: &Zoned) -> String {
    jiff::Timestamp::from_second(stamp).map(|t| sioul_core::quiet::until_text(tr(), &t.to_zoned(now.time_zone().clone()), now)).unwrap_or_default()
}

/// Do-not-disturb as the status line, the phone's card and the background
/// service's notification say it: {on, why, global, manual, until, until_at
/// (0: until switched off), line, why_line, details, everywhere, here,
/// button, end_time, end_at}.
pub(crate) fn moment() -> serde_json::Value {
    moment_of(&Look::now())
}

fn moment_of(look: &Look) -> serde_json::Value {
    let state = &look.state;
    let now = &look.now;
    let stamp = now.timestamp().as_second();
    // The end of what now is for, for "Until 17:00" in the switch's menu (the window's alone).
    let end = if crate::steps::in_service() { None } else { crate::hours::mode_at(now).until.map(|u| u.timestamp().as_second()).filter(|u| *u > stamp && *u - stamp < 24 * 3600) };
    let base = serde_json::json!({
        "button": look.config.dnd.button,
        "end_time": end.map(|e| when(e, now)).unwrap_or_default(),
        "end_at": end.unwrap_or(0),
    });
    let here = here_id();
    let own = look.switch.device.get(&here);
    if !state.on() {
        // Sioul's off, this device's own still on: said, with what may be opened (both ways).
        let system_line = own.filter(|d| d.system_on && d.why.is_empty()).map(|d| d.line.clone()).unwrap_or_default();
        let mut off = serde_json::json!({
            "on": false, "why": "", "global": false, "manual": false, "until": "", "until_at": 0, "line": "", "why_line": "", "details": [], "everywhere": false, "here": false,
            "here_off": false, "here_offers": [],
            "system_on": !system_line.is_empty(), "system_offers": system_offers(&system_line), "system_line": system_line,
            "last_off": last_off(look),
        });
        merge(&mut off, base);
        return off;
    }
    // What this device's system said for the reason that holds; not applied yet: as Sioul's own alone.
    let (silenced, line) = own.filter(|d| !d.why.is_empty()).map_or((false, String::new()), |d| (d.silenced, d.line.clone()));
    // Since the reason said first began: a device's table older than that says what held before.
    let since = state.holds.first().map_or(0, |h| h.since.saturating_mul(1000));
    // A device with its entry in the devices' registry knows do-not-disturb; one without runs an older Sioul.
    let others = rules::others(&look.switch, &here, &heard(&here), stamp, since, &|id| crate::devices::kind_and_name(id).is_some());
    let until = state.until();
    let until_words = until.map(|u| when(u, now)).unwrap_or_default();
    let turned_off = own.is_some_and(|d| d.turned_off && !d.why.is_empty());
    let said = rules::said(state, rules::Here { silenced, line: &line, turned_off }, &others, &until_words, &|id| name_of(&look.switch, id), tr());
    let mut on = serde_json::json!({
        "on": true,
        "why": state.why().map(Why::id).unwrap_or_default(),
        "global": state.global(),
        "manual": state.has(Why::Manual),
        "until": until_words,
        "until_at": until.unwrap_or(0),
        "line": said.line,
        "why_line": said.why,
        "details": said.details,
        "everywhere": said.everywhere,
        "here": said.here,
        "here_off": turned_off,
        "here_offers": system_offers(if turned_off { &line } else { "" }),
        "system_on": false, "system_line": "", "system_offers": [], "last_off": "",
    });
    merge(&mut on, base);
    on
}

/// The buttons beside a line of this device's own do-not-disturb: on a phone,
/// Android's do-not-disturb settings, where the person turns it off or on again.
fn system_offers(line: &str) -> serde_json::Value {
    if line.is_empty() || !cfg!(target_os = "android") {
        return serde_json::json!([]);
    }
    serde_json::json!([{ "key": "modes", "label": crate::dnd::offer_label("modes") }])
}

/// "Turned off on your phone at 14:02, outside Sioul.": the switch's latest
/// press, when it is an "off" heard from a device's own do-not-disturb within
/// the last twelve hours; "" otherwise.
fn last_off(look: &Look) -> String {
    let Some(press) = look.switch.latest().filter(|p| !p.on && p.via == rules::VIA_SYSTEM) else { return String::new() };
    let at = press.at / 1000;
    if look.now.timestamp().as_second() - at > 12 * 3600 {
        return String::new();
    }
    let Ok(time) = jiff::Timestamp::from_second(at) else { return String::new() };
    let time = time.to_zoned(look.now.time_zone().clone()).strftime("%H:%M").to_string();
    say("dnd-last-off", &[("device", name_of(&look.switch, &press.from)), ("time", time)])
}

fn merge(into: &mut serde_json::Value, from: serde_json::Value) {
    if let (Some(into), serde_json::Value::Object(from)) = (into.as_object_mut(), from) {
        into.extend(from);
    }
}

// ---------------------------------------------------------------- Settings ▸ Do not disturb

/// The numbers' country, for numbers written without one.
fn region(config: &Config) -> Option<&'static sioul_core::phones::Region> {
    sioul_core::phones::chosen(config.contacts.region.as_deref(), &tr().text("qt-locale", None))
}

/// The tab's own part, as JSON: what this device can do (the layer's line
/// and buttons, GNOME's consent, Plasma's word on the list's mail), the
/// list, and on a phone who is starred and the background service.
pub(crate) fn setup() -> String {
    setup_with(String::new())
}

fn setup_with(said: String) -> String {
    let config = load_config();
    let mut here = crate::dnd::can().json();
    let mut lines = Vec::new();
    let offers: Vec<String> = here["offers"].as_array().map(|o| o.iter().filter_map(|x| x["key"].as_str().map(str::to_string)).collect()).unwrap_or_default();
    if !cfg!(target_os = "android") && here["on"] == true {
        // Plasma hiding even critical notifications: the list's mail neither shows meanwhile.
        lines.push(tr().text(if offers.iter().any(|o| o == "plasma") { "dnd-setup-plasma-mail" } else { "dnd-setup-critical" }, None));
    }
    here["lines"] = serde_json::json!(lines);
    let people = People::read(&People::default_path());
    let problem = people.as_ref().err().map(|why| say("dnd-setup-unreadable", &[("why", why.clone())])).unwrap_or_default();
    let people: Vec<serde_json::Value> = people
        .unwrap_or_default()
        .people
        .iter()
        .map(|p| {
            let mut notes = Vec::new();
            if p.phones.is_empty() {
                notes.push(tr().text("dnd-setup-no-number", None));
            }
            if p.emails.is_empty() {
                notes.push(tr().text("dnd-setup-no-email", None));
            }
            serde_json::json!({ "id": p.id, "name": p.name, "phones": p.phones, "emails": p.emails, "notes": notes })
        })
        .collect();
    let mut setup = serde_json::json!({
        "here": here,
        "gnome": config.pause.gnome,
        "people": people,
        "problem": problem,
        "said": said,
        "android": cfg!(target_os = "android"),
    });
    if cfg!(target_os = "android") {
        setup["stars"] = stars(&config);
        setup["steps"] = crate::steps::setup(&config);
    }
    setup.to_string()
}

/// Who on the list is starred on this phone (READ_CONTACTS, never written):
/// {permission, summary, people: [{id, name, state, contact}]}, the state
/// "starred", "not-starred", "unknown" (no contact has their number),
/// "no-number".
fn stars(config: &Config) -> serde_json::Value {
    let people = People::load(&People::default_path());
    let asked: Vec<serde_json::Value> = people.people.iter().map(|p| serde_json::json!({ "id": p.id, "phones": p.phones })).collect();
    let answer = crate::steps::java("stars", &serde_json::json!({ "people": asked }).to_string());
    let _ = config;
    stars_of(&people, &answer, tr())
}

/// Java's answer about the list's people, in words: who is not starred here.
fn stars_of(people: &People, answer: &serde_json::Value, tr: &sioul_core::i18n::Translator) -> serde_json::Value {
    if answer["permission"] != true {
        return serde_json::json!({ "permission": false, "summary": tr.text("dnd-stars-permission", None), "people": [] });
    }
    let found = answer["people"].as_array().cloned().unwrap_or_default();
    let mut missing = 0;
    let listed: Vec<serde_json::Value> = people
        .people
        .iter()
        .map(|p| {
            let seen = found.iter().find(|f| f["id"] == p.id.as_str());
            let state = if p.phones.is_empty() { "no-number" } else { seen.and_then(|f| f["state"].as_str()).unwrap_or("unknown") };
            if state == "not-starred" || state == "unknown" {
                missing += 1;
            }
            serde_json::json!({ "id": p.id, "name": if p.name.is_empty() { p.emails.first().cloned().unwrap_or_default() } else { p.name.clone() }, "state": state, "contact": seen.and_then(|f| f["contact"].as_str()).unwrap_or_default() })
        })
        .collect();
    let summary = if missing == 0 {
        tr.text("dnd-stars-all", None)
    } else {
        tr.text("dnd-stars-missing", Some(&tr.counted(missing)))
    };
    serde_json::json!({ "permission": true, "summary": summary, "missing": missing, "people": listed })
}

/// An action of the tab: the list changed, a contact opened, a permission
/// asked… (`verb`, its JSON). Answers the tab again, with a line said, and
/// whether something to share changed (the list, this device's setting).
pub(crate) fn change(verb: &str, json: &str) -> (String, bool) {
    let changes = matches!(verb, "add-safe" | "add-contact" | "add" | "edit" | "remove" | "gnome");
    let answer = change_here(verb, json);
    if verb == "gnome" {
        std::thread::spawn(apply);
    }
    (answer, changes)
}

fn change_here(verb: &str, json: &str) -> String {
    let asked: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let config = load_config();
    let region = region(&config);
    let path = People::default_path();
    let text = |key: &str| asked[key].as_str().unwrap_or_default().trim().to_string();
    let lines = |key: &str| asked[key].as_str().unwrap_or_default().lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect::<Vec<_>>();
    let said = match verb {
        // Everyone on the Safe list: their cards' names and numbers; the
        // categories, cards and numbers on it too, as its lines write them.
        "add-safe" => {
            let list = sioul_core::porch::SenderList::load(&config.safe_senders_path());
            let entries: Vec<String> = list
                .entries()
                .into_iter()
                .chain(list.categories().iter().map(|c| format!("{}{c}", sioul_core::porch::CATEGORY)))
                .chain(list.cards().iter().map(|c| format!("{}{c}", sioul_core::porch::CONTACT)))
                .chain(list.numbers().iter().map(|n| format!("{}{n}", sioul_core::porch::TEL)))
                .collect();
            let (found, patterns) = rules::from_safe(&entries, &sioul_core::contacts::all());
            let mut added = 0;
            match rules::change_people(&path, |people| {
                for person in found {
                    let before = people.people.len();
                    people.add(person, region);
                    added += people.people.len() - before;
                }
            }) {
                Ok(_) => {
                    let mut said = tr().text("dnd-setup-added-safe", Some(&tr().counted(added)));
                    if patterns > 0 {
                        said = format!("{said} {}", tr().text("dnd-setup-patterns", Some(&tr().counted(patterns))));
                    }
                    said
                }
                Err(why) => say("dnd-setup-unreadable", &[("why", why)]),
            }
        }
        // A contact picked from the address books; off the blocked list (Q3).
        "add-contact" => {
            let uid = text("uid");
            match sioul_core::contacts::all().into_iter().find(|c| c.uid == uid) {
                Some(card) => {
                    let person = rules::from_contact(&card);
                    let unblocked = unblock(&config, &person);
                    let said = edit_people(&path, |people| {
                        people.add(person, region);
                    });
                    with_unblocked(said, unblocked)
                }
                None => String::new(),
            }
        }
        // By hand, or changed: a name, numbers and addresses, one per line.
        "add" | "edit" => {
            let person = Person { id: text("id"), name: text("name"), phones: lines("phones"), emails: lines("emails"), ..Person::default() };
            if person.name.is_empty() && person.phones.is_empty() && person.emails.is_empty() {
                String::new()
            } else {
                let unblocked = unblock(&config, &person);
                let said = edit_people(&path, |people| match people.people.iter_mut().find(|p| verb == "edit" && p.id == person.id) {
                    Some(known) => {
                        known.name = person.name;
                        known.phones = person.phones;
                        known.emails = person.emails;
                    }
                    None => {
                        people.add(Person { id: String::new(), ..person }, region);
                    }
                });
                with_unblocked(said, unblocked)
            }
        }
        "remove" => edit_people(&path, |people| {
            people.remove(&text("id"));
        }),
        // The contacts that match, to pick one (twenty at most).
        "search" => {
            let all = sioul_core::contacts::all();
            let query = text("query");
            let found: Vec<serde_json::Value> = sioul_core::contacts::search(&all, &query)
                .into_iter()
                .filter(|c| !c.phones.is_empty() || !c.emails.is_empty())
                .take(20)
                .map(|c| serde_json::json!({ "uid": c.uid, "name": c.name, "detail": c.phones.first().map(|p| p.value.clone()).or_else(|| c.emails.first().map(|e| e.value.clone())).unwrap_or_default() }))
                .collect();
            return serde_json::json!({ "found": found, "none": if found.is_empty() { tr().text("dnd-setup-no-match", None) } else { String::new() } }).to_string();
        }
        // A system page: Android's access, Plasma's settings, the starred list.
        "open" => {
            crate::dnd::open(&text("key"));
            String::new()
        }
        // "Silence this device again" (the switch's menu): the person's earlier "off" in the system set aside.
        "again" => {
            std::thread::spawn(again);
            String::new()
        }
        // On a phone: a person's contact (to star it), Android's form to add one, the permission.
        "open-contact" => {
            crate::steps::java("open-contact", &serde_json::json!({ "uri": text("contact") }).to_string());
            String::new()
        }
        "add-to-contacts" => {
            if let Some(p) = People::load(&path).people.into_iter().find(|p| p.id == text("id")) {
                crate::steps::java("add-contact", &serde_json::json!({ "name": p.name, "phones": p.phones, "emails": p.emails }).to_string());
            }
            String::new()
        }
        "allow-contacts" => {
            crate::steps::ask_contacts();
            String::new()
        }
        "battery" => {
            crate::steps::java("ask-battery", "{}");
            String::new()
        }
        // This phone kept in step in the background, or not (its own setting).
        "background" => crate::steps::set_background(asked["on"] == true).err().unwrap_or_default(),
        // GNOME's consent, the same one the pauses use.
        "gnome" => sioul_core::settings::apply(&crate::backend::config_path(), &config, "pause.gnome", &sioul_core::config::SettingValue::Bool(asked["on"] == true)).err().unwrap_or_default(),
        _ => String::new(),
    };
    setup_with(said)
}

/// Someone put on Always through comes off the blocked list: blocked and
/// Always through exclude each other (docs/attention.md, Q3). Whether they were on it.
fn unblock(config: &Config, person: &Person) -> bool {
    sioul_core::porch::unblock_person(config, &person.emails, &person.phones, &person.contact).unwrap_or_else(|e| {
        eprintln!("sioul: {e}");
        false
    })
}

/// What a change said, and that someone came off the blocked list, when they did.
fn with_unblocked(said: String, unblocked: bool) -> String {
    if !unblocked {
        return said;
    }
    [said, tr().text("attention-unblocked", None)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The list changed; "" said, or why it could not be.
fn edit_people(path: &std::path::Path, change: impl FnOnce(&mut People)) -> String {
    match rules::change_people(path, change) {
        Ok(_) => String::new(),
        Err(why) => say("dnd-setup-unreadable", &[("why", why)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_follows_what_is_asked() {
        let look = |paused: bool, manual: bool| {
            let config = Config::default();
            let press = manual.then(|| rules::Press { at: 1_000, on: true, until: 0, from: "x".into(), via: String::new() });
            let sources = Sources { paused: paused.then_some(1), ..Sources::default() };
            let state = rules::now(&config.dnd, &sources, press.as_ref(), 100);
            let now = Zoned::now();
            let mode = sioul_core::quiet::mode(&config.week_hours(), &config.time_off, &Overrides::default(), &sioul_core::quiet::Blocks::default(), &now);
            Look { config, overrides: Overrides::default(), switch: Switch::default(), state, mode, now }
        };
        let off = look(false, false).signature();
        assert_eq!(off, "Paused:-,FreeTime:-,Global:-");
        assert_ne!(look(false, true).signature(), off);
        assert_ne!(look(true, false).signature(), look(false, true).signature());
        assert_eq!(look(true, true).asks().iter().filter(|(_, a)| a.is_some()).count(), 2, "the pause and the switch, each its own mode");
        // "Let every call through" pressed: anyone's call through the pause's mode, said in what it asks.
        let mut through = look(true, false);
        sioul_core::calls::press(&mut through.switch, "phone", true, 0, Zoned::now().timestamp().as_millisecond());
        let calls = |look: &Look| look.asks()[0].1.as_ref().and_then(|a| a.silence).map(|s| s.calls);
        assert_eq!((calls(&through), calls(&look(true, false))), (Some(sioul_core::attention::Senders::Anyone), Some(sioul_core::attention::Senders::Starred)));
        assert_ne!(through.signature(), look(true, false).signature());
    }

    #[test]
    fn the_phone_says_who_on_the_list_is_not_starred_there() {
        let mut people = People::default();
        for (name, phone) in [("Alice", "+262639980001"), ("Bob", "+262639980002"), ("Carol", "+262639980003"), ("Dan", "")] {
            people.add(Person { name: name.into(), phones: if phone.is_empty() { Vec::new() } else { vec![phone.into()] }, emails: vec![format!("{name}@example.org")], ..Person::default() }, None);
        }
        let id = |name: &str| people.people.iter().find(|p| p.name == name).unwrap().id.clone();
        let answer = serde_json::json!({ "permission": true, "people": [
            { "id": id("Alice"), "state": "starred", "contact": "" },
            { "id": id("Bob"), "state": "not-starred", "contact": "content://com.android.contacts/contacts/lookup/x/1" },
            { "id": id("Carol"), "state": "unknown", "contact": "" },
        ] });
        let en = sioul_core::i18n::Translator::new("en");
        let said = stars_of(&people, &answer, &en);
        assert_eq!(said["missing"], 2);
        assert_eq!(said["summary"], "Two people on your list are not starred on this phone.");
        let states: Vec<&str> = said["people"].as_array().unwrap().iter().map(|p| p["state"].as_str().unwrap()).collect();
        assert_eq!(states, ["starred", "not-starred", "unknown", "no-number"]);
        assert_eq!(said["people"][1]["contact"], "content://com.android.contacts/contacts/lookup/x/1");
        // Everyone starred, or no permission: said so.
        let all = serde_json::json!({ "permission": true, "people": [{ "id": id("Alice"), "state": "starred" }, { "id": id("Bob"), "state": "starred" }, { "id": id("Carol"), "state": "starred" }] });
        assert_eq!(stars_of(&people, &all, &en)["summary"], "Everyone on your list with a number is starred on this phone: their calls and messages ring during do-not-disturb.");
        let refused = stars_of(&people, &serde_json::json!({ "permission": false }), &en);
        assert_eq!(refused["permission"], false);
        assert!(refused["summary"].as_str().unwrap().contains("read your contacts"));
    }
}

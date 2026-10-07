// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The two pauses as the window has them (docs/pauses.md; the rules are
//! `sioul_core::pause`): **Free time**, a switch in the bottom row, and
//! **Pause**, a button apart, which covers the window. Their state is in
//! `quiet.toml`, which the sharing carries to your other devices: a pause
//! pressed on one is a pause on all (P8). Each device puts its own system's
//! do-not-disturb on and off (`dnd`): at the press, at each minute (what came
//! from another device, what a crash left), when the window comes back.

use crate::backend::qobject::Sioul;
use crate::backend::{QtThread, Shared, load_config, mode_json, say, show, tr};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use jiff::Zoned;
use sioul_core::pause::{self, After, Day};
use sioul_core::quiet::Overrides;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// The overrides as they were before the pause pressed here, for "Undo" after
/// coming back: as if it had not been pressed (an accidental press costs
/// nothing). With the press's stamp, so that another pause never takes it.
static BEFORE_PRESS: Mutex<Option<(i64, Overrides)>> = Mutex::new(None);

fn load() -> Overrides {
    Overrides::load(&Overrides::default_path())
}

fn save(sioul: &mut Pin<&mut Sioul>, overrides: &Overrides) -> bool {
    match overrides.save(&Overrides::default_path()) {
        Ok(()) => true,
        Err(e) => {
            sioul.as_mut().set_status(QString::from(&e));
            false
        }
    }
}

/// Today as the evening's rules read it: hours, events, meals, the night.
fn today(now: &Zoned) -> Day {
    let midnight = now.date().to_zoned(now.time_zone().clone()).map_or(0, |z| z.timestamp().as_second());
    Day::of(&load_config(), crate::hours::blocks(now), &sioul_core::agenda::occurrences(midnight, midnight + 26 * 3600))
}

/// "17:00", on the clock.
fn hm(stamp: i64, now: &Zoned) -> String {
    jiff::Timestamp::from_second(stamp).map(|t| t.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default()
}

/// Today's usual end of work, when today has working hours and no time off.
fn usual_end(config: &sioul_core::config::Config, now: &Zoned) -> Option<i64> {
    if sioul_core::quiet::time_off_on(&config.time_off, now.date()).is_some() {
        return None;
    }
    sioul_core::window::hours_on(&config.working_hours(), now.date(), now.time_zone()).map(|(_, closing)| closing.timestamp().as_second())
}

/// Every page shown again for the time it is, the other devices told at once.
fn shown_again(mut sioul: Pin<&mut Sioul>) {
    sioul.as_mut().set_mode(QString::from(&mode_json()));
    let (qt, shared) = (sioul.qt_thread(), sioul.shared());
    show(&qt, &shared);
    crate::work::show_work(&qt, &shared);
    crate::share::exchange(&qt, &shared);
    std::thread::spawn(apply);
}

/// The system's do-not-disturb as the pauses are now, on this device: the one
/// shared do-not-disturb state decides it (`everywhere`, docs/do-not-disturb.md),
/// the pauses among its reasons, read from `quiet.toml` as here: the pause's
/// own mode while paused, Free time's while not.
pub(crate) fn apply() {
    crate::everywhere::apply();
}

/// Free time gone past the night's start ended by itself (sleep comes
/// first): said in the file, so that every device agrees, nothing said.
fn settle(now: &Zoned) {
    let path = Overrides::default_path();
    let mut overrides = Overrides::load(&path);
    let Some(since) = overrides.free_from() else { return };
    let blocks = crate::hours::blocks(now);
    let end = pause::free_until(since, &blocks, now.time_zone());
    if now.timestamp().as_second() < end {
        return;
    }
    let Ok(at) = jiff::Timestamp::from_second(end).map(|t| t.to_zoned(now.time_zone().clone())) else { return };
    let day = today(&at);
    pause::end_free(&mut overrides, &day.evening(), &at, load_config().free_time.moves);
    if let Err(e) = overrides.save(&path) {
        eprintln!("{e}");
    }
}

/// Each minute, at the start, and when the window comes back: Free time
/// that ended by itself, the do-not-disturb, and on a phone the "Pause"
/// pressed on the quick-settings tile or the home screen's shortcut.
pub(crate) fn tick(qt: &QtThread, _shared: &Arc<Shared>) {
    let qt = qt.clone();
    std::thread::spawn(move || {
        settle(&Zoned::now());
        apply();
        if crate::dnd::take_pause_pressed() {
            let _ = qt.queue(|sioul| press(sioul));
        }
    });
}

/// What the status line and the bottom row need of the pauses (`mode_json`).
pub(crate) fn moment(config: &sioul_core::config::Config, overrides: &Overrides, now: &Zoned) -> serde_json::Value {
    let usual = usual_end(config, now);
    let moved = overrides.moved_end(now.date());
    let free = overrides.free_from().is_some();
    // "Keep my usual end": in free time on a working day, or while today's end is moved.
    let can_keep = config.free_time.moves && usual.is_some() && overrides.keep_end_on != Some(now.date()) && (free || moved.is_some());
    serde_json::json!({
        "nothing": pause::nothing_now(overrides, &config.free_time),
        "usual_end": usual.map(|u| hm(u, now)).unwrap_or_default(),
        "moved_end": moved.map(|m| hm(m, now)).unwrap_or_default(),
        "can_keep": can_keep,
    })
}

/// Free time on: leisure whatever the hour (GP1, GP5). A focus session
/// running stops and counts; true when one did, for the window to offer the
/// line on where you stopped (GP3). Off: back, the end of work moved as said
/// once (GP10, GP16): the line said, else "".
pub(crate) fn set_free(mut sioul: Pin<&mut Sioul>, on: bool) -> QString {
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    // A free time that ended by itself (the night came) is said ended first.
    settle(&now);
    let mut overrides = load();
    if on {
        if overrides.paused_from().is_some() || overrides.free_from().is_some() {
            return QString::from("false");
        }
        pause::start_free(&mut overrides, stamp);
        if !save(&mut sioul, &overrides) {
            return QString::from("false");
        }
        let (qt, shared) = (sioul.qt_thread(), sioul.shared());
        let stopped = sioul_core::timelog::running().is_some();
        if stopped {
            crate::work::focus_stop(&qt, &shared, false, "");
        }
        sioul.as_mut().set_status(QString::default());
        shown_again(sioul);
        return QString::from(if stopped { "true" } else { "false" });
    }
    let config = load_config();
    let day = today(&now);
    let moved = pause::end_free(&mut overrides, &day.evening(), &now, config.free_time.moves);
    if !save(&mut sioul, &overrides) {
        return QString::default();
    }
    // Said once, never as a countdown: when today's work ends now.
    let line = moved.map(|end| say("free-back-moved", &[("time", hm(end, &now))])).unwrap_or_default();
    sioul.as_mut().set_status(QString::from(&line));
    shown_again(sioul);
    QString::from(&line)
}

/// "Keep my usual end" (GP11, GP19): no reason asked; what no longer fits
/// goes to later days.
pub(crate) fn keep_usual_end(mut sioul: Pin<&mut Sioul>) {
    let now = Zoned::now();
    let mut overrides = load();
    pause::keep_usual_end(&mut overrides, now.date());
    if !save(&mut sioul, &overrides) {
        return;
    }
    let line = usual_end(&load_config(), &now).map(|end| say("free-kept", &[("time", hm(end, &now))])).unwrap_or_default();
    sioul.as_mut().set_status(QString::from(&line));
    shown_again(sioul);
}

/// "Nothing at all" for this free time (GP6), or back to the safe list.
pub(crate) fn set_free_nothing(mut sioul: Pin<&mut Sioul>, on: bool) {
    let mut overrides = load();
    overrides.free_nothing = Some(on);
    if save(&mut sioul, &overrides) {
        shown_again(sioul);
    }
}

/// The Pause: it asks nothing (P6). Everything Sioul shows is held, on every
/// device (P7–P8); free time ends there and today's end of work stays as
/// usual: a pause never costs an evening (GP19–GP20); a session running
/// stops and counts. Pressed while paused: the screen, nothing else.
pub(crate) fn press(mut sioul: Pin<&mut Sioul>) {
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let mut overrides = load();
    if overrides.paused_from().is_none() {
        let before = overrides.clone();
        if overrides.free_from().is_some() {
            overrides.free_ended = Some(stamp);
        }
        pause::keep_usual_end(&mut overrides, now.date());
        pause::start_pause(&mut overrides, stamp);
        if !save(&mut sioul, &overrides) {
            return;
        }
        if let Ok(mut kept) = BEFORE_PRESS.lock() {
            *kept = Some((stamp, before));
        }
        let (qt, shared) = (sioul.qt_thread(), sioul.shared());
        if sioul_core::timelog::running().is_some() {
            crate::work::focus_stop(&qt, &shared, false, "");
        }
    }
    sioul.as_mut().set_status(QString::default());
    shown_again(sioul);
}

/// When the Porch opens again after a pause (P24): at the next admin hours,
/// else the next working hours, else tomorrow morning.
fn porch_back(config: &sioul_core::config::Config, now: &Zoned) -> Option<i64> {
    let admin: Vec<_> = config.week_hours().into_iter().filter(|w| w.kind() == "admin").collect();
    let hours = if admin.is_empty() { config.week_hours() } else { admin };
    let morning = i8::try_from(config.agenda.day_start.unwrap_or(7)).unwrap_or(7);
    let next = sioul_core::quiet::next_work(&hours, &config.time_off, now).unwrap_or_else(|| sioul_core::quiet::after_today(&[], &[], now, morning));
    Some(next.timestamp().as_second())
}

/// Coming back (P22–P26): the rest of today lighter, or closed, as set up;
/// the Porch resting until the next admin hours; "Undo" ten seconds in the
/// status line, as if the pause had not been pressed. No question, no
/// count. The screen after it, as JSON: its few lines and the one offer.
pub(crate) fn come_back(mut sioul: Pin<&mut Sioul>) -> QString {
    let config = load_config();
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let before = load();
    let Some(since) = before.paused_from() else { return QString::default() };
    let mut overrides = before.clone();
    let after = config.pause.after();
    let morning = i8::try_from(config.agenda.day_start.unwrap_or(7)).unwrap_or(7);
    let back = sioul_core::quiet::after_today(&config.working_hours(), &config.time_off, &now, morning);
    let rest = (after == After::Rest).then(|| back.timestamp().as_second());
    pause::end_pause(&mut overrides, &now, after, rest, porch_back(&config, &now));
    if !save(&mut sioul, &overrides) {
        return QString::default();
    }
    // Undo: as before the press when it was pressed here, else as now without this pause.
    let mut undone = BEFORE_PRESS.lock().ok().and_then(|kept| kept.as_ref().filter(|(at, _)| *at == since).map(|(_, o)| o.clone())).unwrap_or(before);
    pause::forget(&mut undone);
    let (qt, shared) = (sioul.qt_thread(), sioul.shared());
    let title = tr().text("pause-back-title", None);
    crate::mail::rest(&qt, &shared, undone, title.clone());
    // Words only where they change something: today's work not over yet, a working day tomorrow.
    let work_left = usual_end(&config, &now).is_some_and(|end| end > stamp);
    let day = match after {
        After::Lighter if work_left => tr().text("pause-back-lighter", None),
        After::Rest if work_left => say("pause-back-rest", &[("back", sioul_core::quiet::until_text(tr(), &back, &now))]),
        _ => String::new(),
    };
    let tomorrow = now.date().tomorrow().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok());
    let working_tomorrow = tomorrow.is_some_and(|t| config.week_hours().is_empty() || usual_end(&config, &t).is_some());
    shown_again(sioul);
    QString::from(
        &serde_json::json!({
            "title": title,
            "day": day,
            "tomorrow": if working_tomorrow { tr().text("pause-back-tomorrow", None) } else { String::new() },
            "lighten": tr().text("pause-lighten-tomorrow", None),
            "go": tr().text("pause-back-go", None),
        })
        .to_string(),
    )
}

/// "Lighten tomorrow", the one offer coming back (default: no, P23).
pub(crate) fn lighten_tomorrow(mut sioul: Pin<&mut Sioul>) {
    let now = Zoned::now();
    let Ok(tomorrow) = now.date().tomorrow() else { return };
    let mut overrides = load();
    pause::lighten(&mut overrides, tomorrow);
    if save(&mut sioul, &overrides) {
        sioul.as_mut().set_status(QString::from(&tr().text("pause-tomorrow-lighter", None)));
        shown_again(sioul);
    }
}

/// "Forget the last pause" (P26): when it began and ended, gone.
pub(crate) fn forget(mut sioul: Pin<&mut Sioul>) {
    let mut overrides = load();
    if overrides.paused_from().is_some() {
        return;
    }
    pause::forget(&mut overrides);
    if save(&mut sioul, &overrides) {
        sioul.as_mut().set_status(QString::from(&tr().text("pause-setup-forgotten", None)));
    }
}

/// The pause's screen (P11–P17), as JSON: your list, the breathing guide and
/// the grounding line as set up, the numbers of your country; whether doses
/// still come, as the notification matrix says.
pub(crate) fn screen() -> String {
    let config = load_config();
    let mut screen = pause::screen(&config.pause, config.contacts.region.as_deref(), tr());
    screen.doses = sioul_core::notify::Notify::of(&config).cell(sioul_core::notify::Kind::Doses, sioul_core::notify::Column::Pause) == sioul_core::notify::Cell::Now;
    serde_json::to_string(&screen).unwrap_or_default()
}

/// The setup's do-not-disturb part, as JSON: what this device can do, its
/// buttons and GNOME's consent (`dnd::Report::json`), and the consent as set.
/// A system round trip: asked when the tab opens, never per frame.
pub(crate) fn setup() -> String {
    let mut report = crate::dnd::can().json();
    report["gnome"] = serde_json::Value::Bool(load_config().pause.gnome);
    report["forgettable"] = serde_json::Value::Bool({
        let overrides = load();
        overrides.paused_since.is_some() && overrides.paused_from().is_none()
    });
    report.to_string()
}

/// After a pause, the Porch rests until the next admin hours unless opened
/// (P24): only the codes you asked for, and when it opens.
pub(crate) fn porch_rests(porch: &mut sioul_core::view::PorchView, opened_anyway: bool, now: &Zoned) {
    let overrides = load();
    let stamp = now.timestamp().as_second();
    if opened_anyway || !overrides.porch_rests(stamp) {
        return;
    }
    let Some(until) = overrides.porch_rests_until.and_then(|u| jiff::Timestamp::from_second(u).ok()).map(|t| t.to_zoned(now.time_zone().clone())) else { return };
    porch.open = false;
    porch.closed = Some(say("porch-rests", &[("when", tr().when(&until))]));
    porch.summary.clear();
    porch.status.clear();
    porch.lanes.clear();
}

/// In free time, the leisure the Tasks page offers (GP7): open tasks in view
/// now, joy and someday included, never ranked, at most eight; never a list
/// to finish (no count, no next step). Empty outside free time.
pub(crate) fn offers(tasks: &[sioul_core::tasks::Task], quiet: Option<&sioul_core::quiet::QuietTasks>, free: bool) -> Vec<serde_json::Value> {
    let Some(quiet) = quiet.filter(|_| free) else { return Vec::new() };
    let mut open: Vec<&sioul_core::tasks::Task> = tasks.iter().filter(|t| t.status.is_open() && quiet.keeps(t)).collect();
    open.sort_by_key(|t| t.created);
    open.into_iter().take(8).map(|t| serde_json::json!({ "uid": t.uid, "title": t.title })).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn porch_back_without_hours_is_tomorrow_morning() {
        let now: jiff::Zoned = "2026-10-05T15:00[Europe/Paris]".parse().unwrap();
        let config = sioul_core::config::Config::default();
        let back = super::porch_back(&config, &now).unwrap();
        assert_eq!(super::hm(back, &now), "07:00");
    }
}

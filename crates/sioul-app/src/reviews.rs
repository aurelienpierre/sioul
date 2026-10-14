// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The two rituals, for the window (docs/reviews.md): closing the work day
//! ("Done for today") and closing the day before sleep, each with how the day
//! felt, all of it optional. The sheet's view (`DayReview.qml`), the answers
//! kept (`sioul_core::reviews`), the status line's offer, the one notice at the
//! end of the day's last work or admin hours, and a day's words for the Health
//! page. Words only: no count, no score, nothing about what was not done.

use crate::backend::{QtThread, Shared, load_config, mode_json, say, tr};
use crate::backend::qobject::Sioul;
use cxx_qt_lib::QString;
use jiff::Zoned;
use jiff::civil::Date;
use sioul_core::quiet::{Mode, Overrides};
use sioul_core::reviews::{self, Felt, Kind, Mix, Moment, Offer, Review, Reviews};
use std::pin::Pin;
use std::sync::Arc;

/// The end-of-work notice comes within this long of the hours' end, or not at all.
const NOTICE_GRACE: i64 = 10 * 60;
/// The notes of the last days, read back in the sheet.
const BACK_DAYS: i64 = 14;

/// Today's last hours of work or admin end then; none on a day without them,
/// or in time off.
fn work_end(config: &sioul_core::config::Config, now: &Zoned) -> Option<Zoned> {
    work_end_on(config, now.date(), now.time_zone())
}

/// `date`'s last hours of work or admin end then, in `zone`; none on a day
/// without them, or in time off.
fn work_end_on(config: &sioul_core::config::Config, date: Date, zone: &jiff::tz::TimeZone) -> Option<Zoned> {
    if sioul_core::quiet::time_off_on(&config.time_off, date).is_some() {
        return None;
    }
    sioul_core::window::hours_on(&config.week_hours(), date, zone).map(|(_, closing)| closing)
}

/// The night going on or coming next, as Health keeps it: (winding down from, bed at).
fn night(now: &Zoned) -> Option<(i64, i64)> {
    let stamp = now.timestamp().as_second();
    crate::hours::blocks(now).kept.iter().filter(|k| k.kind == "sleep" && k.end > stamp).min_by_key(|k| k.start).map(|k| (k.start, k.at))
}

/// Where the day stands at `now`, as the offer reads it: today's end of work,
/// the night, whether the work day was closed, and the reviews of yesterday and today.
fn standing(now: &Zoned) -> (Option<Zoned>, Option<(i64, i64)>, bool, Reviews) {
    let config = load_config();
    let overrides = Overrides::load(&Overrides::default_path());
    let today = now.date();
    let reviews = Reviews::load_between(&Reviews::default_path(), today.yesterday().unwrap_or(today), today);
    (work_end(&config, now), night(now), overrides.closed_today(now), reviews)
}

/// What the status line offers at `now`, in `mode`.
fn offer_at(now: &Zoned, mode: &Mode) -> Option<Offer> {
    let (work_end, night, closed_today, reviews) = standing(now);
    reviews::offer(&Moment { now, mode, work_end, night, closed_today }, &reviews)
}

/// What the notice "Work hours are over" would offer at `now`, as the status
/// line does, whatever the time now is for: the matrix of what reaches you
/// says apart when it may come (`work_notice`).
fn notice_offer(now: &Zoned, mode: &Mode) -> Option<Offer> {
    let (work_end, night, closed_today, reviews) = standing(now);
    reviews::offer_any_time(&Moment { now, mode, work_end, night, closed_today }, &reviews)
}

/// A sheet asked for: "work" or "night", and the day it is for when a
/// notice's press carries one ("work:2026-10-14"); none for words that are
/// neither.
fn asked(what: &str) -> Option<(Kind, Option<Date>)> {
    match what.split_once(':') {
        Some((kind, date)) => Some((Kind::parse(kind)?, Some(date.parse().ok()?))),
        None => Some((Kind::parse(what)?, None)),
    }
}

/// For the status line (`mode_json`): {kind, date, line, button}; kind "" when
/// nothing is offered: at work, while you sleep, once said.
pub(crate) fn offer_json(now: &Zoned, mode: &Mode) -> serde_json::Value {
    match offer_at(now, mode) {
        Some(offer) => serde_json::json!({
            "kind": offer.kind.id(),
            "date": offer.date.to_string(),
            "line": tr().text(&format!("review-offer-{}", offer.kind.id()), None),
            "button": tr().text(&format!("review-close-{}", offer.kind.id()), None),
        }),
        None => serde_json::json!({ "kind": "", "date": "", "line": "", "button": "" }),
    }
}

/// The day a review closes, given now: the night's day (after midnight, the
/// evening before); the work day's, today (before 05:00, yesterday).
fn day_of(kind: Kind, now: &Zoned) -> Date {
    let mode = crate::hours::mode_at(now);
    let config = load_config();
    let moment = Moment { now, mode: &mode, work_end: work_end(&config, now), night: night(now), closed_today: false };
    match kind {
        Kind::Night => moment.evening().1,
        Kind::Work if now.hour() < reviews::DAY_TURNS => now.date().yesterday().unwrap_or(now.date()),
        Kind::Work => now.date(),
    }
}

/// The first letter capitalized ("lundi" → "Lundi").
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// "Tomorrow", "Monday", "Monday 19 October": the nearer, the shorter.
fn day_name(day: Date, today: Date) -> String {
    let name = if today.tomorrow().is_ok_and(|t| t == day) {
        tr().text("day-tomorrow", None)
    } else if day > today && day.since(today).is_ok_and(|s| s.get_days() < 7) {
        tr().text(&format!("weekday-{}", day.weekday().to_monday_one_offset()), None)
    } else {
        tr().day(day)
    };
    capitalized(&name)
}

/// The first step named when the work day closed, for the day work comes back:
/// "Tomorrow starts with: open the form (15 min)". A plan for what remains
/// helps sleep come (Scullin et al. 2018); none when none was named.
fn first_step_line(date: Date) -> Option<String> {
    let overrides = Overrides::load(&Overrides::default_path());
    let step = overrides.first_step.as_deref().map(str::trim).filter(|s| !s.is_empty())?;
    let day = overrides.first_step_day.filter(|d| *d > date)?;
    Some(say("review-first-step", &[("day", day_name(day, date)), ("step", step.to_string())]))
}

/// The day's costs and gains in words, for the night: each scale whose level is
/// known that day ("Thinking: heavy for you."), the gain ("Some time gave
/// back."), then the plan's own lines when the day changes tomorrow's. Never a
/// number, a percentage or a gauge (criteria CB45–49, G20).
fn balance_lines(date: Date) -> Vec<String> {
    // The capacity's own reading of the day: its numbers (load, budget, good hours, minimum) stay there.
    balance_words(&crate::capacity::day_balance(date), tr())
}

/// The balance in words, in `tr`'s language: a scale not known that day says
/// nothing (CB12), the total is the scales' own; the gain in two words.
fn balance_words(balance: &sioul_core::capacity::DayBalance, tr: &sioul_core::i18n::Translator) -> Vec<String> {
    let mut lines: Vec<String> = balance
        .costs
        .iter()
        .filter(|scale| !scale.level.is_empty() && scale.scale != "total")
        .map(|scale| {
            let mut args = sioul_core::i18n::args();
            args.set("scale", tr.text(&format!("review-scale-{}", scale.scale), None));
            args.set("level", tr.text(&format!("review-level-{}", scale.level), None));
            tr.text("review-scale", Some(&args))
        })
        .collect();
    match balance.gain.level {
        "below" => lines.push(tr.text("review-gain-little", None)),
        "around" | "above" => lines.push(tr.text("review-gain-some", None)),
        _ => {}
    }
    lines.extend(balance.said.iter().cloned());
    lines
}

/// A review's note as the sheet and the Health page show it: Markdown read as Notes reads it.
fn memo_html(review: Option<&Review>) -> Option<String> {
    review.map(|r| r.memo.trim()).filter(|m| !m.is_empty()).map(sioul_core::compose::markdown_html)
}

/// The sheet for `what` ("work", "night"), as JSON: its title and the day it
/// closes; how the day started and went, in words; for the night the work's
/// review, tomorrow's first step and the day's balance; the answers already
/// given (closed once, the sheet opens on them); the notes of the last days.
/// A notice's press carries its day ("work:2026-10-14"): pressed once the
/// day turned (05:00 for the work day, the evening for the night), it opens
/// nothing ("null"), rather than the sheet of a day not begun.
pub(crate) fn view(what: &str) -> String {
    let (kind, for_day) = match asked(what) {
        Some(asked) => asked,
        None if what.contains(':') => return "null".into(),
        None => (Kind::Work, None),
    };
    let now = Zoned::now();
    let date = day_of(kind, &now);
    if for_day.is_some_and(|day| day != date) {
        return "null".into();
    }
    let from = date.checked_sub(jiff::Span::new().days(BACK_DAYS)).unwrap_or(date);
    let all = Reviews::load_between(&Reviews::default_path(), from, date);
    let day = all.day(date);
    let mut lines = reviews::morning_lines(day, tr());
    if kind == Kind::Night {
        if let Some(words) = day.and_then(|d| d.work.as_ref()).and_then(|r| reviews::said_words(r, tr())) {
            lines.push(say("review-work-said", &[("words", words)]));
        }
        lines.extend(first_step_line(date));
    }
    let balance = if kind == Kind::Night { balance_lines(date) } else { Vec::new() };
    let given = day.and_then(|d| d.review(kind));
    let back: Vec<serde_json::Value> = all
        .recent(date, BACK_DAYS)
        .into_iter()
        .map(|(date, day)| {
            let memos: Vec<String> = [memo_html(day.work.as_ref()), memo_html(day.night.as_ref())].into_iter().flatten().collect();
            serde_json::json!({ "day": capitalized(&tr().day_in(date, now.date())), "line": reviews::back_line(day, tr()), "memos": memos })
        })
        .collect();
    serde_json::json!({
        "kind": kind.id(),
        "date": date.to_string(),
        "title": tr().text(&format!("review-{}-title", kind.id()), None),
        "lines": lines,
        "balance": balance,
        "felt": given.and_then(|r| r.felt).map_or("", Felt::id),
        "mix": given.and_then(|r| r.mix).map_or("", Mix::id),
        "memo": given.map(|r| r.memo.clone()).unwrap_or_default(),
        "close": tr().text(&format!("review-close-{}", kind.id()), None),
        "back": back,
    })
    .to_string()
}

/// The review kept for `date`, the day its sheet was opened for (`view`),
/// though the day turned meanwhile (a sheet opened at 04:58 and closed at
/// 05:01 closes the day it showed; none given, the day of `kind` now); then
/// for the work day the day closed as "Done for today" always did, unless it
/// was already today, or the sheet was an earlier day's: the closing
/// screen's JSON, else "". The night: the status line says the day is
/// closed. A review that cannot be kept is said; the work day closes all
/// the same.
pub(crate) fn close(mut sioul: Pin<&mut Sioul>, kind: &str, date: &str, felt: &str, mix: &str, memo: &str) -> QString {
    let Some(kind) = Kind::parse(kind) else { return QString::default() };
    let now = Zoned::now();
    let today = day_of(kind, &now);
    let date = date.parse::<Date>().unwrap_or(today);
    let review = Review { at: now.timestamp().as_second(), felt: Felt::parse(felt), mix: Mix::parse(mix), memo: memo.trim().to_string() };
    let kept = reviews::save(&Reviews::default_path(), date, kind, &review, now.time_zone());
    let closing = match kind {
        Kind::Work if closes_the_work_day(date, today, Overrides::load(&Overrides::default_path()).closed_today(&now)) => sioul.as_mut().done_for_the_day(),
        Kind::Work => {
            sioul.as_mut().set_status(QString::from(&tr().text("review-kept", None)));
            QString::default()
        }
        Kind::Night => {
            sioul.as_mut().set_status(QString::from(&tr().text("review-closed-night", None)));
            QString::default()
        }
    };
    if let Err(e) = kept {
        sioul.as_mut().set_status(QString::from(&e));
    }
    // What the status line offers follows at once.
    sioul.as_mut().set_mode(QString::from(&mode_json()));
    closing
}

/// Whether closing a work day's sheet closes the work day itself, as "Done
/// for today" always did: only today's, not closed yet; an earlier day's
/// sheet (opened before the turn of the day, closed after it) keeps its
/// review alone, and never closes a day not begun.
fn closes_the_work_day(sheet: Date, today: Date, closed_today: bool) -> bool {
    sheet == today && !closed_today
}

/// The weather said for today, copied as the morning's (`reviews::note_weather`).
pub(crate) fn weather_said(weather: sioul_core::today::Weather) {
    let now = Zoned::now();
    if let Err(e) = reviews::note_weather(&Reviews::default_path(), now.date(), weather, now.timestamp().as_second(), now.time_zone()) {
        eprintln!("{e}");
    }
}

/// A day on the Health page, as JSON: its words when some were said ("How the
/// day went: Morning: haze · End of work: heavy…") and its notes; today, the
/// button to close it while the evening offers it, and during the night.
pub(crate) fn line(date: &str) -> String {
    let now = Zoned::now();
    let Ok(date) = date.parse::<Date>() else { return "null".into() };
    let reviews = Reviews::load_between(&Reviews::default_path(), date, date);
    let day = reviews.day(date);
    let words = day.filter(|d| d.said()).map(|d| reviews::back_line(d, tr())).unwrap_or_default();
    let memos: Vec<String> = day.map(|d| [memo_html(d.work.as_ref()), memo_html(d.night.as_ref())].into_iter().flatten().collect()).unwrap_or_default();
    let mode = crate::hours::mode_at(&now);
    let offered = offer_at(&now, &mode).is_some_and(|o| o.kind == Kind::Night && o.date == date);
    // Winding down or asleep in this day's night, the page still offers it: you came to it.
    let tonight = mode.sleeps() && night(&now).is_some_and(|(from, bed)| from <= now.timestamp().as_second() && reviews::night_day(bed, now.time_zone()) == Some(date));
    let open = day.is_none_or(|d| d.night.is_none()) && (offered || tonight);
    serde_json::json!({
        "said": if words.is_empty() { String::new() } else { tr().text("review-line-said", None) },
        "words": words,
        "memos": memos,
        "close": open,
        "button": tr().text("review-close-night", None),
    })
    .to_string()
}

/// Whether an event goes on now (not a whole day's, not cancelled).
fn meeting(stamp: i64) -> bool {
    sioul_core::agenda::occurrences(stamp - 86_400, stamp + 60).iter().any(|e| !e.cancelled && !e.all_day && e.start <= stamp && e.end > stamp)
}

/// The day the end-of-work notice was given, on this device.
fn told_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("review-told.toml")
}

fn told(date: Date) -> bool {
    std::fs::read_to_string(told_path()).ok().and_then(|t| t.parse::<toml::Table>().ok()).and_then(|t| t.get("work").and_then(|v| v.as_str()).map(str::to_string)).is_some_and(|d| d == date.to_string())
}

fn mark_told(date: Date) {
    let _ = std::fs::create_dir_all(sioul_core::config::state_dir());
    if let Err(e) = std::fs::write(told_path(), format!("work = \"{date}\"\n")) {
        eprintln!("{}: {e}", told_path().display());
    }
}

/// What the end-of-work notice does at a moment (`work_notice`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkNotice {
    /// It comes now, for that day.
    Comes(Date),
    /// Kept here only lately: asked again in a minute, once the others know.
    Soon,
    /// Held by the matrix of what reaches you now (a pause, do-not-disturb):
    /// it may come once it lets it, while its ten minutes last.
    Held,
    /// Not now, or not from here.
    No,
}

/// The end-of-work notice at `now`: at the end of the day's last work or
/// admin hours, within ten minutes of it, once on this device (`told`); at
/// the times the matrix of what reaches you lets it come (as usual, never
/// while you sleep or work); never during a meeting, nor once the day was
/// closed or its review given, nor when the evening's offer takes its place:
/// then it is told for the day, never asked again; from the device you are
/// at, as `keeper` says it (the "notices" lease), asked last. The window's
/// minute and a phone's alarm both decide it here (`tick`, `decide_work`).
pub(crate) fn work_notice(now: &Zoned, keeper: impl FnOnce() -> sioul_sync::lease::Keeper) -> WorkNotice {
    let stamp = now.timestamp().as_second();
    let Some(end) = work_end(&load_config(), now) else { return WorkNotice::No };
    if !still_due(stamp, end.timestamp().as_second()) || told(now.date()) {
        return WorkNotice::No;
    }
    if !crate::hours::comes(sioul_core::attention::Kind::WorkOver) {
        return WorkNotice::Held;
    }
    let mode = crate::hours::mode_at(now);
    // Closed early, its review given, or the evening's offer instead: it cannot come today.
    let Some(Offer { kind: Kind::Work, date }) = notice_offer(now, &mode) else {
        mark_told(now.date());
        return WorkNotice::No;
    };
    // In a meeting, nothing is said: the status line still offers it.
    if meeting(stamp) {
        mark_told(date);
        return WorkNotice::No;
    }
    let keeper = keeper();
    // Kept here only lately: the next minute, once the others know.
    if keeper.mine && !keeper.settled {
        return WorkNotice::Soon;
    }
    mark_told(date);
    if keeper.mine { WorkNotice::Comes(date) } else { WorkNotice::No }
}

/// Each minute: the end-of-work notice when it comes (`work_notice`), "Work
/// hours are over", with "Close the work day". On a phone its own alarm says
/// it, Sioul shown or not (`decide_work`). Off the window's thread.
pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    if cfg!(target_os = "android") {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        static BUSY: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let Some(_busy) = crate::backend::one_at_a_time(&BUSY) else { return };
        let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
        let keeper = || crate::share::keeper("notices", sioul_sync::lease::Rule::FollowsYou, active, false).0;
        if let WorkNotice::Comes(date) = work_notice(&Zoned::now(), keeper) {
            let close = Offered { label: tr().text("review-close-work", None), kind: "review", key: format!("work:{date}") };
            if let Err(e) = show_notice(&qt, &tr().text("review-notice-title", None), &tr().text("review-notice-text", None), vec![close]) {
                eprintln!("{e}");
            }
        }
    });
}

/// A button of a closing notice: its words, and what it opens in the window
/// (`reminder_opened`: "review" with "work" or "night" and the day it is for,
/// "work:2026-10-14", or "need" with a block's key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Offered {
    pub label: String,
    pub kind: &'static str,
    pub key: String,
}

/// A closing notice (the end of work's, the night's), as the system shows it
/// (docs/reviews.md, "The notices"): on Linux and macOS with its buttons
/// (`notify::remind_offering`), each opening what it offers; on Windows
/// through Sioul's icon in the tray (main.qml's `onTrayNotice`, Tray.qml),
/// whose message carries no button: a click opens what the first one offers.
/// A phone's are its alarm's (`decide_work`, `health::decide_night`).
pub(crate) fn show_notice(qt: &QtThread, title: &str, body: &str, offered: Vec<Offered>) -> Result<(), String> {
    if cfg!(windows) {
        let (kind, key) = offered.into_iter().next().map(|first| (first.kind, first.key)).unwrap_or(("", String::new()));
        let (title, body) = (title.to_string(), body.to_string());
        let _ = qt.queue(move |mut sioul| sioul.as_mut().tray_notice(QString::from(&title), QString::from(&body), QString::from(kind), QString::from(&key)));
        return Ok(());
    }
    let choices = offered
        .into_iter()
        .map(|offer| {
            let qt = qt.clone();
            let open: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = qt.queue(move |mut sioul| sioul.as_mut().reminder_opened(QString::from(offer.kind), QString::default(), QString::from(&offer.key)));
            });
            (offer.label, open)
        })
        .collect();
    sioul_sync::notify::remind_offering(title, body, choices)
}

/// A phone's notice stays in the shade this long after it comes at most,
/// then goes (the work's at the turn of the day, 05:00, if sooner; the
/// night's at waking): its press carries its day, and opens nothing once the
/// day turned (`view`).
pub(crate) const PHONE_NOTICE_KEPT: i64 = 6 * 3600;

/// The turn of the day (05:00) next after `end`, when the work day it ended
/// is no longer the one the sheet closes (`day_of`), Unix seconds; six hours
/// on where the zone has no such time.
fn next_turn(end: &Zoned) -> i64 {
    let at = |date: Date| date.at(reviews::DAY_TURNS, 0, 0, 0).to_zoned(end.time_zone().clone()).ok().map(|z| z.timestamp().as_second());
    let stamp = end.timestamp().as_second();
    at(end.date()).filter(|turn| *turn > stamp).or_else(|| end.date().tomorrow().ok().and_then(at)).unwrap_or(stamp + PHONE_NOTICE_KEPT)
}

/// Whether the end-of-work notice may still come at `stamp`, the hours
/// having ended at `end` (Unix seconds): within its ten minutes, never before.
fn still_due(stamp: i64, end: i64) -> bool {
    (0..NOTICE_GRACE).contains(&(stamp - end))
}

/// When the end-of-work notice may come in the next days, for a phone's
/// alarms (`eventalarms::coming`): at the end of each day's last work or
/// admin hours, today's while it may still come; each (`day:work:<date>`,
/// Unix seconds). A time just past stays as it was, so that the list stays
/// the same from one minute to the next: Android rings it at once.
pub(crate) fn coming_work(now: &Zoned) -> Vec<(String, i64)> {
    let config = load_config();
    let stamp = now.timestamp().as_second();
    let mut out = Vec::new();
    let mut date = now.date();
    for _ in 0..3 {
        if let Some(end) = work_end_on(&config, date, now.time_zone()) {
            let at = end.timestamp().as_second();
            if stamp - at < NOTICE_GRACE && !told(date) {
                out.push((format!("day:work:{date}"), at));
            }
        }
        let Ok(next) = date.tomorrow() else { break };
        date = next;
    }
    out
}

/// A phone's alarm at the end of work (`eventalarms`, "day:work:…"): the
/// notice as the window's minute decides it on a computer (`work_notice`),
/// who keeps the notices read, never claimed, from there: Sioul may be put
/// away, or not running (`share::looked`). Shown in Android's notifications
/// with "Close the work day", which opens Sioul on the sheet. {"shown": true}
/// once shown, its press carrying its day; {"again_at": ms} rung early, kept
/// here only lately, or held by the matrix while its ten minutes last, asked
/// each minute as a computer's minute asks it; {} when it does not come, nor
/// asked again that day once it cannot (`work_notice`).
pub(crate) fn decide_work(now: &Zoned) -> serde_json::Value {
    let stamp = now.timestamp().as_second();
    let end = work_end(&load_config(), now);
    let end_at = end.as_ref().map(|end| end.timestamp().as_second());
    if let Some(end) = end_at
        && (1..3600).contains(&(end - stamp))
    {
        return serde_json::json!({ "again_at": end * 1000 });
    }
    match work_notice(now, || crate::share::looked("notices", sioul_sync::lease::Rule::FollowsYou)) {
        WorkNotice::Comes(date) => {
            let close = Offered { label: tr().text("review-close-work", None), kind: "review", key: format!("work:{date}") };
            let until = end.as_ref().map_or(stamp + PHONE_NOTICE_KEPT, |end| next_turn(end).min(stamp + PHONE_NOTICE_KEPT));
            crate::eventalarms::day_note(&format!("day:work:{date}"), &tr().text("review-notice-title", None), &tr().text("review-notice-text", None), &[close], until);
            serde_json::json!({ "shown": true })
        }
        WorkNotice::Soon | WorkNotice::Held if end_at.is_some_and(|end| still_due(stamp + 60, end)) => serde_json::json!({ "again_at": (stamp + 60) * 1000 }),
        WorkNotice::Soon | WorkNotice::Held | WorkNotice::No => serde_json::json!({}),
    }
}

/// Whether the night's review of the night `bed` belongs to is still to give:
/// its notice then offers "Close the day".
pub(crate) fn night_open(bed: i64, now: &Zoned) -> bool {
    let Some(date) = reviews::night_day(bed, now.time_zone()) else { return false };
    Reviews::load_between(&Reviews::default_path(), date, date).day(date).is_none_or(|d| d.night.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::capacity::{DayBalance, GainBalance, ScaleBalance};
    use sioul_core::i18n::Translator;

    fn scale(scale: &'static str, level: &'static str) -> ScaleBalance {
        ScaleBalance { scale, level, load: 12.5, budget: 20.0 }
    }

    #[test]
    fn the_days_balance_in_words_only() {
        let en = Translator::new("en");
        let balance = DayBalance {
            date: "2026-10-06".into(),
            costs: vec![scale("cognitive", "heavy"), scale("emotional", ""), scale("anxiety", "light"), scale("body", "usual"), scale("total", "heavy")],
            gain: GainBalance { level: "below", good_hours: 0.4, minimum: 1.0 },
            said: vec!["The plan keeps tomorrow lighter.".into()],
        };
        let words = balance_words(&balance, &en);
        assert_eq!(words, ["Thinking: heavy for you.", "Anxiety: light for you.", "Body and senses: usual for you.", "Little time gave back.", "The plan keeps tomorrow lighter."]);
        assert!(words.iter().all(|w| !w.chars().any(|c| c.is_ascii_digit() || c == '%')), "never a number: {words:?}");
        // Nothing known that day: nothing said, rather than "light".
        let unknown = DayBalance { date: "2026-10-05".into(), costs: vec![scale("cognitive", ""), scale("total", "")], gain: GainBalance { level: "", good_hours: 0.0, minimum: 1.0 }, said: Vec::new() };
        assert!(balance_words(&unknown, &en).is_empty());
        let fr = Translator::new("fr");
        let some = DayBalance { date: "2026-10-04".into(), costs: vec![scale("emotional", "heavy")], gain: GainBalance { level: "around", good_hours: 1.2, minimum: 1.0 }, said: Vec::new() };
        assert_eq!(balance_words(&some, &fr), ["Émotions\u{202f}: charge lourde pour vous.", "Du temps vous a ressourcé."]);
    }

    /// The end-of-work notice comes within ten minutes of the hours' end,
    /// never before them: a phone's alarm rung hours early (an old day's,
    /// after a restart) is not asked again each minute until then.
    #[test]
    fn the_end_of_work_notice_comes_within_its_ten_minutes_only() {
        let end = 1_792_000_000;
        assert!(still_due(end, end));
        assert!(still_due(end + NOTICE_GRACE - 1, end));
        assert!(!still_due(end + NOTICE_GRACE, end));
        assert!(!still_due(end - 1, end));
        assert!(!still_due(end - 8 * 3600, end), "hours before: not due, not asked again");
    }

    /// A notice's press carries its day: the sheet is asked for that day,
    /// and only words that are a sheet and a day are read so.
    #[test]
    fn a_press_asks_for_its_own_days_sheet() {
        let day: Date = "2026-10-14".parse().unwrap();
        assert_eq!(asked("work"), Some((Kind::Work, None)));
        assert_eq!(asked("night"), Some((Kind::Night, None)));
        assert_eq!(asked("work:2026-10-14"), Some((Kind::Work, Some(day))));
        assert_eq!(asked("night:2026-10-14"), Some((Kind::Night, Some(day))));
        assert_eq!(asked("work:yesterday"), None);
        assert_eq!(asked("settings:2026-10-14"), None);
        // Words that are no day: nothing, read before anything is.
        assert_eq!(view("work:yesterday"), "null");
    }

    /// The work notice on a phone goes at the turn of the day (05:00) after
    /// the hours' end, or six hours after it came if sooner: work ending at
    /// 23:30 goes at 05:00, not at 05:30.
    #[test]
    fn the_work_notice_goes_at_the_turn_of_the_day() {
        let zone = jiff::tz::TimeZone::fixed(jiff::tz::offset(2));
        let at = |text: &str| text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap();
        assert_eq!(next_turn(&at("2026-10-14T17:30")), at("2026-10-15T05:00").timestamp().as_second());
        assert_eq!(next_turn(&at("2026-10-14T23:30")), at("2026-10-15T05:00").timestamp().as_second());
        assert_eq!(next_turn(&at("2026-10-15T01:00")), at("2026-10-15T05:00").timestamp().as_second());
        assert_eq!(next_turn(&at("2026-10-15T05:00")), at("2026-10-16T05:00").timestamp().as_second());
        let end = at("2026-10-14T23:30");
        let shown = end.timestamp().as_second() + 60;
        assert_eq!(next_turn(&end).min(shown + PHONE_NOTICE_KEPT), at("2026-10-15T05:00").timestamp().as_second());
    }

    /// A sheet opened before the turn of the day and closed after it closes
    /// the day it showed: its review kept, the new day never closed.
    #[test]
    fn a_sheet_closed_after_the_turn_of_the_day_closes_its_own() {
        let (yesterday, today): (Date, Date) = ("2026-10-13".parse().unwrap(), "2026-10-14".parse().unwrap());
        assert!(closes_the_work_day(today, today, false));
        assert!(!closes_the_work_day(today, today, true), "closed already");
        assert!(!closes_the_work_day(yesterday, today, false), "opened at 04:58, closed at 05:01: its review alone");
    }
}

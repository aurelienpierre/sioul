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

/// What the notice "Work hours are over" offers at `now`: as the status
/// line, at the times the matrix of what reaches you lets it come (as usual,
/// the status line's own: never at work, asleep, paused or in Free time).
fn notice_at(now: &Zoned, mode: &Mode) -> Option<Offer> {
    if !crate::hours::comes(sioul_core::attention::Kind::WorkOver) {
        return None;
    }
    let (work_end, night, closed_today, reviews) = standing(now);
    reviews::offer_any_time(&Moment { now, mode, work_end, night, closed_today }, &reviews)
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

/// The sheet for `kind` ("work", "night"), as JSON: its title and the day it
/// closes; how the day started and went, in words; for the night the work's
/// review, tomorrow's first step and the day's balance; the answers already
/// given (closed once, the sheet opens on them); the notes of the last days.
pub(crate) fn view(kind: &str) -> String {
    let kind = Kind::parse(kind).unwrap_or(Kind::Work);
    let now = Zoned::now();
    let date = day_of(kind, &now);
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

/// The review kept, then for the work day the day closed as "Done for today"
/// always did, unless it was already today: the closing screen's JSON, else
/// "". The night: the status line says the day is closed. A review that
/// cannot be kept is said; the work day closes all the same.
pub(crate) fn close(mut sioul: Pin<&mut Sioul>, kind: &str, felt: &str, mix: &str, memo: &str) -> QString {
    let Some(kind) = Kind::parse(kind) else { return QString::default() };
    let now = Zoned::now();
    let date = day_of(kind, &now);
    let review = Review { at: now.timestamp().as_second(), felt: Felt::parse(felt), mix: Mix::parse(mix), memo: memo.trim().to_string() };
    let kept = reviews::save(&Reviews::default_path(), date, kind, &review, now.time_zone());
    let closing = match kind {
        Kind::Work if !Overrides::load(&Overrides::default_path()).closed_today(&now) => sioul.as_mut().done_for_the_day(),
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
    /// Not now, or not from here.
    No,
}

/// The end-of-work notice at `now`: at the end of the day's last work or
/// admin hours, within ten minutes of it, once on this device (`told`); at
/// the times the matrix of what reaches you lets it come (as usual, never
/// while you sleep or work); never during a meeting, nor once the day was
/// closed or its review given; from the device you are at, as `keeper` says
/// it (the "notices" lease), asked last. The window's minute and a phone's
/// alarm both decide it here (`tick`, `decide_work`).
pub(crate) fn work_notice(now: &Zoned, keeper: impl FnOnce() -> sioul_sync::lease::Keeper) -> WorkNotice {
    let stamp = now.timestamp().as_second();
    let Some(end) = work_end(&load_config(), now) else { return WorkNotice::No };
    if !still_due(stamp, end.timestamp().as_second()) || told(now.date()) {
        return WorkNotice::No;
    }
    let mode = crate::hours::mode_at(now);
    let Some(Offer { kind: Kind::Work, date }) = notice_at(now, &mode) else { return WorkNotice::No };
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
        if let WorkNotice::Comes(_) = work_notice(&Zoned::now(), keeper) {
            let close = Offered { label: tr().text("review-close-work", None), kind: "review", key: "work".into() };
            if let Err(e) = show_notice(&qt, &tr().text("review-notice-title", None), &tr().text("review-notice-text", None), vec![close]) {
                eprintln!("{e}");
            }
        }
    });
}

/// A button of a closing notice: its words, and what it opens in the window
/// (`reminder_opened`: "review" with "work" or "night", "need" with a block's key).
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

/// A phone's notice stays in the shade this long after it comes, then goes:
/// pressed later, its sheet would close another day.
pub(crate) const PHONE_NOTICE_KEPT: i64 = 6 * 3600;

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
/// once shown; {"again_at": ms} rung early, kept here only lately, or not
/// yet (the matrix holding it) while its ten minutes last, asked each
/// minute as a computer's minute asks it; {} when it does not come.
pub(crate) fn decide_work(now: &Zoned) -> serde_json::Value {
    let stamp = now.timestamp().as_second();
    let end = work_end(&load_config(), now).map(|end| end.timestamp().as_second());
    if let Some(end) = end
        && (1..3600).contains(&(end - stamp))
    {
        return serde_json::json!({ "again_at": end * 1000 });
    }
    match work_notice(now, || crate::share::looked("notices", sioul_sync::lease::Rule::FollowsYou)) {
        WorkNotice::Comes(date) => {
            let close = Offered { label: tr().text("review-close-work", None), kind: "review", key: "work".into() };
            crate::eventalarms::day_note(&format!("day:work:{date}"), &tr().text("review-notice-title", None), &tr().text("review-notice-text", None), &[close], stamp + PHONE_NOTICE_KEPT);
            serde_json::json!({ "shown": true })
        }
        WorkNotice::Soon => serde_json::json!({ "again_at": (stamp + 60) * 1000 }),
        WorkNotice::No if end.is_some_and(|end| still_due(stamp + 60, end)) && !told(now.date()) => serde_json::json!({ "again_at": (stamp + 60) * 1000 }),
        WorkNotice::No => serde_json::json!({}),
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
}

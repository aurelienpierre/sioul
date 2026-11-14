// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's own notices on a phone (docs/android.md, "Notifications"): each
//! one a notice of the model (`sioul_core::notice`), handed to one poster in
//! Java (android/package/src/com/aurelienpierre/sioul/Notices.java) with its
//! class, its group, how long it lasts once it could be seen and when it
//! goes at the latest (`notice_json`): Rust says each notice's life, Java
//! only counts it from the phone's next unlock.
//!
//! The time to leave for an event: Android freezes an app it does not show,
//! so the coming ones are given ahead to Android's alarm clock
//! (EventAlarms.java), as the doses are (`alarms`): at the window's minute,
//! when Sioul is put away, after each ring. At its time a receiver asks here
//! what to say (`sioul_event_decide`): the event read again (moved,
//! cancelled, told already?), sleep or a pause holding it or not, the time
//! left said as it is then. Every device tells its own: one held back on the
//! phone because the computer was used a moment ago, then shown on an empty
//! desk, would be an appointment missed. An event's own alarm rings in the
//! phone's calendar app, where one rings it (the notifications' decision 3:
//! `calendar_app`); where none does, here, as before, a notice to do. Which
//! of the two is said once on each device (`handover`).
//!
//! The same list holds when the meals, the night's notice and the end of
//! work may come ("day" its kind): decided here at their alarm, and told on
//! the phone only when nobody is at a computer (`devices::computer_in_use`).
//!
//! Also here, Android's side of new mail's notification (`mail_note`,
//! MailNotes.java), of a code's (`code_note`), and what a tapped
//! notification opens (`opened`). Elsewhere nothing: the window's minute and
//! `sioul remind --watch` tell them.

use crate::backend::{load_config, tr};
use jiff::Zoned;
use jiff::tz::TimeZone;
use sioul_core::notice::{Act, Lasting, Notice};
use sioul_core::reminders::{self, Holds, Kind, Reminder};
use std::ffi::{CString, c_char};
use std::sync::Mutex;

#[cfg(target_os = "android")]
// SAFETY: declared as android/main.cpp defines them: extern "C", the same types.
unsafe extern "C" {
    /// The coming events' reminders handed to Java, which keeps them and sets their alarms (android/main.cpp).
    fn sioul_android_set_event_alarms(json: *const c_char);
    /// The time to leave for an event shown (EventAlarms.show, through Notices; android/main.cpp).
    fn sioul_android_event_note(json: *const c_char);
    /// New mail's notification (MailNotes.show, through Notices; android/main.cpp).
    fn sioul_android_mail_note(json: *const c_char);
    /// The end of work's notice, the night's or a need's, with its buttons (DayNotes.show, through Notices; android/main.cpp).
    fn sioul_android_day_note(json: *const c_char);
    /// What tapped notifications open, once, as a JSON list [{kind, key}]; false when nothing (android/main.cpp).
    fn sioul_android_take_reminder_opened(buffer: *mut c_char, size: i32) -> bool;
}

/// At most this many alarms given: Android keeps 500 per app, the doses' among them.
const AT_MOST: usize = 100;
/// Java asks again this long after the list was given, when nothing rang meanwhile…
const LOOK_AGAIN: i64 = 24 * 3600;
/// …in steps of this, so that the same list is not handed again each minute.
const LOOK_STEP: i64 = 6 * 3600;
/// A reminder held while nothing says when it may come (Free time ended, say) is asked about again this often.
const AGAIN: i64 = 15 * 60;

/// The list last handed to Java, so that it is handed again only when it changes.
static GIVEN: Mutex<String> = Mutex::new(String::new());

/// The words Java says, in your language, given with the list and with each
/// notice: Sioul may not be running when a reminder is shown. The channels'
/// names: "Notices", "To act on" and its twin that passes one of Sioul's
/// modes; new mail's own that passes, for someone Always through.
fn words() -> serde_json::Value {
    serde_json::json!({
        "notices": tr().text("notices-channel", None),
        "act": tr().text("act-channel", None),
        "act-pause": tr().text("act-pause-channel", None),
        "mail-through": tr().text("mail-through-channel", None),
        "open": tr().text("reminder-open", None),
        // A reminder on a lock screen that hides private content: nothing of what it is about.
        "public": tr().text("reminder-public", None),
    })
}

/// A notice as Notices.java posts it (docs/android.md, "Notifications"), at
/// `now` (Unix seconds), or none once it no longer lasts (its thing ended,
/// its relevance past: `notice::lasting`): {tag (Android's key: the next
/// notice with it takes its place), about (its thing's address, which a tap
/// opens), kind (the matrix's row, "mail", "minor"), class ("informative",
/// "actionable"), group (its class: Android groups nothing of Sioul's by
/// itself then), lasts_ms (how long it stays once it could be seen; 0, until
/// settled), ends (Unix ms, when it goes at the latest; 0, none), pierce
/// (one to do that passes one of Sioul's modes, as the matrix said), title,
/// body, actions ([{label, open: address}] or [{label, copy: text}]), public
/// ({title, body} as the lock screen shows it, without what it is about;
/// none: Sioul's words for a reminder), words}.
pub(crate) fn notice_json(tag: &str, notice: &Notice, pierce: bool, public: Option<(&str, &str)>, now: i64) -> Option<serde_json::Value> {
    let zoned = sioul_core::zone::now();
    let moment = sioul_core::notice::Moment { day_ends: sioul_core::notice::Moment::end_of_day(&zoned), ..Default::default() };
    let lasting = sioul_core::notice::lasting(notice, moment, load_config().notices.minutes(), now)?;
    Some(notice_value(tag, notice, lasting, pierce, public, words()))
}

/// `notice_json` once its life is known (`lasting`): pure.
fn notice_value(tag: &str, notice: &Notice, lasting: Lasting, pierce: bool, public: Option<(&str, &str)>, words: serde_json::Value) -> serde_json::Value {
    use sioul_core::notice::Class;
    let class = match lasting.class {
        Class::Informative => "informative",
        Class::Actionable => "actionable",
    };
    let actions: Vec<serde_json::Value> = notice
        .actions
        .iter()
        .filter_map(|action| match &action.act {
            Act::Open { address, .. } if !address.is_empty() => Some(serde_json::json!({ "label": action.label, "open": address })),
            Act::Copy(text) => Some(serde_json::json!({ "label": action.label, "copy": text })),
            // The window's own verbs: none on a phone.
            _ => None,
        })
        .collect();
    serde_json::json!({
        "tag": tag,
        "about": notice.about,
        "kind": notice.kind.id(),
        "class": class,
        "group": class,
        "lasts_ms": lasting.lasts.saturating_mul(1000),
        "ends": lasting.ends.map_or(0, |ends| ends.saturating_mul(1000)),
        // Only one to do passes one of Sioul's modes (docs/notifications.md, "Two classes").
        "pierce": pierce && lasting.class == Class::Actionable,
        "title": notice.title,
        "body": notice.body,
        "actions": actions,
        "public": public.map(|(title, body)| serde_json::json!({ "title": title, "body": body })),
        "words": words,
    })
}

/// How long a notice to know lasts once it could be seen (milliseconds), as
/// set (`[notices] informative_minutes`): given with the list, for the
/// notices Java says from the words it keeps (Rust silent at a reminder's
/// time) and new mail's when its own words do not say it.
fn informative_ms() -> i64 {
    i64::from(load_config().notices.minutes()) * 60_000
}

/// What Java is given: the times to leave not told yet that still make
/// sense, nearest first, at most a hundred, each {key, at (Unix
/// milliseconds), title, body, about (the event's address and time), ends
/// (Unix ms, its start)} (the words said if Sioul cannot be asked at its
/// time); one held now (you sleep) at the time it may come; `look`, when to
/// ask for the list again (a day on, in steps of six hours, so that the list
/// given is the same from one minute to the next); `lasts_ms`, how long a
/// notice to know lasts once seen; `words`.
pub(crate) fn coming(now: &Zoned) -> String {
    let (all, holds) = reminders::gather_events(&load_config(), tr(), now);
    listed(&all, &crate::hours::with_layers(holds), now)
}

/// Whether a reminder is one Android tells here, and its alarm clock is
/// given: the time to leave for an event (Sioul's "before" it); an event's
/// own alarm only where no calendar app rings it in Sioul's place
/// (`calendar_rings`: `calendar_app`, the notifications' decision 3). The
/// working day before is made no more (decision 3 too).
pub(crate) fn told_on_android(kind: Kind, calendar_rings: bool) -> bool {
    kind == Kind::Before || (kind == Kind::Alarm && !calendar_rings)
}

/// The calendar app that rings an event's own alarms on this device in
/// Sioul's place, by its package and its name (CalendarApps.java's rule: an
/// app answers Android's calendar category, and an account type syncs
/// agendas into it); none where there is none, and Sioul rings them itself,
/// as before. Asked of Android once every ten minutes in a process at most.
/// None off Android.
pub(crate) fn calendar_app() -> Option<(String, String)> {
    if !cfg!(target_os = "android") {
        return None;
    }
    type Kept = Option<(std::time::Instant, Option<(String, String)>)>;
    static KEPT: Mutex<Kept> = Mutex::new(None);
    let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((at, app)) = kept.as_ref()
        && at.elapsed() < std::time::Duration::from_secs(600)
    {
        return app.clone();
    }
    let said = crate::steps::java("calendar-app", "{}");
    // No answer (an older Java, a refusal): Sioul rings them, never none does.
    let package = said["package"].as_str().unwrap_or_default().to_string();
    let app = (!package.is_empty()).then(|| (package.clone(), said["label"].as_str().filter(|l| !l.is_empty()).unwrap_or(&package).to_string()));
    *kept = Some((std::time::Instant::now(), app.clone()));
    app
}

/// Where what a device said of an event's own alarms is kept (`handover`).
fn handover_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("event-alarms-said.txt")
}

/// Which of the two a device says of an event's own alarms (`handover`):
/// "calendar:<package>" where a calendar app rings them, "sioul" where
/// Sioul does. Pure.
fn handover_word(app: Option<&(String, String)>) -> String {
    app.map_or_else(|| "sioul".to_string(), |(package, _)| format!("calendar:{package}"))
}

/// Said once on each device, and again only once it changes (a calendar app
/// installed or gone): who rings an event's own alarms here. A calendar app
/// does (the notifications' decision 3): a notice to know, with a button to
/// that app's notifications in Android's settings, for whoever silenced it
/// there once Sioul rang them too. None does: Sioul rings them here, and
/// says so. Kept as said only once posted (`handover_path`). Android only.
fn handover(app: Option<&(String, String)>) {
    let word = handover_word(app);
    let path = handover_path();
    if std::fs::read_to_string(&path).is_ok_and(|said| said.trim() == word) {
        return;
    }
    let now = jiff::Timestamp::now().as_second();
    let Some(json) = handover_json(app, now) else { return };
    if crate::steps::java("notice", &json.to_string()) == true {
        let _ = std::fs::create_dir_all(sioul_core::config::state_dir());
        if let Err(e) = std::fs::write(&path, &word) {
            eprintln!("{}: {e}", path.display());
        }
    }
}

/// The notice `handover` posts (`notice_json`): a minor notice to know,
/// about nothing of Sioul's (a tap brings Sioul up); a calendar app named,
/// with a button to its notifications in Android's settings.
fn handover_json(app: Option<&(String, String)>, now: i64) -> Option<serde_json::Value> {
    use sioul_core::notice::Kind as Of;
    let notice = match app {
        Some((_, label)) => Notice::new("", Of::Minor, tr().text("event-alarms-calendar-title", None), crate::backend::say("event-alarms-calendar-body", &[("app", label.clone())])),
        None => Notice::new("", Of::Minor, tr().text("event-alarms-sioul-title", None), tr().text("event-alarms-sioul-body", None)),
    };
    let mut json = notice_json("event-alarms", &notice, false, None, now)?;
    if let (Some((package, _)), Some(actions)) = (app, json["actions"].as_array_mut()) {
        actions.push(serde_json::json!({ "label": tr().text("event-alarms-calendar-settings", None), "settings": package }));
    }
    Some(json)
}

/// The list Java is given, from reminders gathered already: the times to
/// leave, then the times the end-of-work notice, the night's and the meals'
/// may come (`reviews::coming_work`, `health::coming_night`,
/// `health::coming_needs`), "day" their kind, with no words: Rust alone says
/// them, or nothing (`day_note`).
fn listed(all: &[Reminder], holds: &Holds, now: &Zoned) -> String {
    let stamp = now.timestamp().as_second();
    let rings = calendar_app().is_some();
    let dir = reminders::told_dir();
    let mut due: Vec<(i64, &Reminder)> = all
        .iter()
        .filter(|r| told_on_android(r.kind, rings) && r.until > stamp && !reminders::told(&dir, &r.key))
        .map(|r| (when(r, holds, stamp), r))
        .filter(|(at, r)| *at < r.until)
        .collect();
    due.sort_by_key(|(at, r)| (*at, r.key.clone()));
    let mut list: Vec<serde_json::Value> = due.iter().take(AT_MOST).map(|(at, r)| listed_entry(r, *at)).collect();
    let days = crate::reviews::coming_work(now).into_iter().chain(crate::health::coming_night(now)).chain(crate::health::coming_needs(now));
    list.extend(days.map(|(key, at)| serde_json::json!({ "key": key, "kind": "day", "at": at * 1000, "title": "", "body": "" })));
    let look = (stamp / LOOK_STEP + LOOK_AGAIN / LOOK_STEP) * LOOK_STEP;
    serde_json::json!({ "reminders": list, "look": look * 1000, "lasts_ms": informative_ms(), "words": words() }).to_string()
}

/// A reminder as the list gives it, its alarm at `at` (Unix seconds): its
/// words, said if Sioul cannot be asked then, with its event's address and
/// time, which a tap on that notice opens too, and when that notice goes:
/// the time to leave as its event starts, an event's own alarm a quarter of
/// an hour after (`Reminder::until`).
fn listed_entry(reminder: &Reminder, at: i64) -> serde_json::Value {
    let about = reminder.notice("").about;
    let ends = if reminder.kind == Kind::Alarm { reminder.until } else { reminder.starts.unwrap_or(reminder.until) };
    serde_json::json!({ "key": reminder.key, "kind": reminder.kind, "at": at * 1000, "title": reminder.title, "body": reminder.body, "about": about, "ends": ends * 1000 })
}

/// When a reminder's alarm rings (Unix seconds): its time; past it, at once
/// when nothing holds it, else when the hold ends (waking).
fn when(reminder: &Reminder, holds: &Holds, stamp: i64) -> i64 {
    if reminder.at > stamp {
        reminder.at
    } else if reminder.ready_in(stamp, holds) {
        stamp + 1
    } else {
        again_at(holds, stamp)
    }
}

/// The list handed to Java when it changed since it was last.
fn hand(json: String) {
    let Ok(mut given) = GIVEN.lock() else { return };
    if *given == json {
        return;
    }
    let Ok(text) = CString::new(json.clone()) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_set_event_alarms(text.as_ptr())
    };
    let _ = text;
    *given = json;
}

/// The coming events' reminders given to Android's alarm clock when they
/// changed: as Sioul is put away, and after each ring. Nothing elsewhere.
pub(crate) fn schedule() {
    if !cfg!(target_os = "android") {
        return;
    }
    hand(coming(&sioul_core::zone::now()));
    handover(calendar_app().as_ref());
}

/// The same from the window's minute, with the reminders it gathered
/// (`remind::tick`): the calendars read once a minute, not twice.
pub(crate) fn schedule_from(all: &[Reminder], holds: &Holds, now: &Zoned) {
    if !cfg!(target_os = "android") {
        return;
    }
    hand(listed(all, holds, now));
    handover(calendar_app().as_ref());
}

/// A reminder shown in Android's notifications (`shown`): a notice to know,
/// its words, "Open", a tap opening the event; the time to leave gone from
/// the shade as its event starts. On a tablet, the dates, waits, payments,
/// papers and contracts too, each opening its thing.
pub(crate) fn show(reminder: &Reminder) {
    let Some(json) = shown(reminder, jiff::Timestamp::now().as_second()) else { return };
    let Ok(text) = CString::new(json.to_string()) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_event_note(text.as_ptr())
    };
    let _ = text;
}

/// New mail's notification on a phone (`mailnote`), a notice to know in the
/// quiet "Notices" channel (MailNotes.java): one at a time, the next batch in
/// its place; each message it names with its own opener (`opens`), a tap
/// opening the first; an older MailNotes.java reads `open` alone, the address
/// of its one message (`mail_note_json`), else the mailbox at its top.
/// `through`: someone Always through is in it, whom the system's
/// do-not-disturb should let through (a channel that passes, Java's to choose).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn mail_note(title: &str, body: &str, through: bool, open: &str, opens: &[sioul_core::mailnote::Opener]) {
    let json = mail_note_json(title, body, through, open, opens, words());
    let Ok(text) = CString::new(json.to_string()) else { return };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_mail_note(text.as_ptr())
    };
    let _ = text;
}

/// What MailNotes.java reads of new mail's notification: {title, body,
/// through, open, opens: [{label, address}], words}. `opens`: one per message
/// the notice names (three at most), its words and its address (`mid:…`,
/// `sioul:mail/…`), which a tap on it opens through `ReminderOpener`
/// ("mail"); the notice's own tap opens the first (the notifications' phase
/// 2, read by the phase 3's Notices.java). `open`, kept for an older
/// MailNotes.java: the address of its one message, or "" for several (the
/// mailbox). Never a file: an address that is none is left out.
fn mail_note_json(title: &str, body: &str, through: bool, open: &str, opens: &[sioul_core::mailnote::Opener], words: serde_json::Value) -> serde_json::Value {
    let open = if is_mail_address(open) { open } else { "" };
    let opens: Vec<serde_json::Value> = opens.iter().filter(|o| is_mail_address(&o.address)).map(|o| serde_json::json!({ "label": o.label, "address": o.address })).collect();
    serde_json::json!({ "title": title, "body": body, "through": through, "open": open, "opens": opens, "words": words })
}

/// The end of work's notice, the night's or a meal's on a phone
/// (`reviews::decide_work`, `health::decide_night`, `health::decide_need`):
/// a notice to know (`notice_json`), its words, each button opening Sioul on
/// what it offers by its address, a tap on the notice its thing (DayNotes.java);
/// shown in its place, by `key` (the alarm's: "day:work:2026-10-14"), when it
/// comes again. Whether it was shown: false once it no longer lasts.
pub(crate) fn day_note(key: &str, notice: &Notice) -> bool {
    let Some(json) = notice_json(key, notice, false, None, jiff::Timestamp::now().as_second()) else { return false };
    let Ok(text) = CString::new(json.to_string()) else { return false };
    #[cfg(target_os = "android")]
    // SAFETY: android/main.cpp's; `text` is a C string alive until the call returns, and it copies it.
    unsafe {
        sioul_android_day_note(text.as_ptr())
    };
    let _ = text;
    true
}

/// A one-time code or a temporary password on a phone (`backend::notify_codes`):
/// a notice to do (`notice_json`) about its message, Copy putting the code on
/// the clipboard and Open the message, a tap opening it; on the lock screen
/// its title alone, never the code; gone when the code expires, once copied,
/// or once its message is read on any device (`notices::withdraw_codes`).
/// `pierce`: it passes one of Sioul's modes, as the matrix said. Whether it
/// was shown.
pub(crate) fn code_note(notice: &Notice, pierce: bool) -> bool {
    let Some(json) = notice_json(&notice.key(), notice, pierce, Some((&notice.title, "")), jiff::Timestamp::now().as_second()) else { return false };
    crate::steps::java("notice", &json.to_string());
    cfg!(target_os = "android")
}

/// A notice shown by `code_note`, taken away on this phone: its thing is
/// settled (Notices.java, every notice about `about`). Nothing elsewhere.
pub(crate) fn withdraw(about: &str) {
    if cfg!(target_os = "android") {
        crate::steps::java("notice-withdraw", &serde_json::json!({ "about": about }).to_string());
    }
}

/// The phone is seen in use (Sioul back on the screen): the notices to know
/// posted while it was not start their minutes now (Notices.java; the
/// background service hears the phone's unlock itself). Nothing elsewhere.
pub(crate) fn seen() {
    if cfg!(target_os = "android") {
        crate::steps::java("notices-seen", "{}");
    }
}

/// When a reminder held now may come: when the time now ends (waking, the
/// end of free time, the hours' end), a minute on at the soonest; while a
/// layer holds it (a slot of time for you, do-not-disturb) or the end is
/// unknown (a pause), at the next quarter of an hour (Unix seconds). In
/// steps, never "a minute from now": the list given to Java stays the same
/// from one call to the next, so it is not given again, its alarms set again,
/// every second.
fn again_at(holds: &Holds, stamp: i64) -> i64 {
    match holds.until {
        Some(until) if !holds.now.slot && !holds.now.dnd => until.max(step_after(stamp, 60)),
        _ => step_after(stamp, AGAIN),
    }
}

/// The first multiple of `step` seconds after `stamp`.
fn step_after(stamp: i64, step: i64) -> i64 {
    (stamp.div_euclid(step) + 1) * step
}

/// At a reminder's time (`EventAlarms`): {"shown": true} once told here; or
/// {"again_at": ms}, held now (you sleep) and still worth telling then; or
/// {} (moved, cancelled, told already, too late). The coming list is given
/// again after, as the calendars are now. The end of work's, the night's and
/// the meals' ("day:work:…", "day:night:…", "day:need:…") are theirs to decide.
fn decide(key: &str) -> String {
    let now = sioul_core::zone::now();
    if let Some(day) = key.strip_prefix("day:") {
        let answer = if day.starts_with("work:") {
            crate::reviews::decide_work(&now)
        } else if day.starts_with("night:") {
            crate::health::decide_night(&now)
        } else if let Some(need) = day.strip_prefix("need:") {
            crate::health::decide_need(&now, need)
        } else {
            serde_json::json!({})
        };
        schedule();
        return answer.to_string();
    }
    let stamp = now.timestamp().as_second();
    let (all, holds) = reminders::gather_events(&load_config(), tr(), &now);
    let holds = crate::hours::with_layers(holds);
    let dir = reminders::told_dir();
    // An older build's alarm of an event's own, armed before the list changed: a calendar app rings it now.
    let rings = calendar_app().is_some();
    let answer = match all.iter().find(|r| r.key == key && told_on_android(r.kind, rings)) {
        Some(r) if r.ready_in(stamp, &holds) => {
            // Told by the window's minute already: nothing more.
            let shown = reminders::claim(&dir, &r.key);
            if shown {
                show(r);
            }
            serde_json::json!({ "shown": shown })
        }
        // Not yet due (an alarm a moment early), or held now (you sleep): asked again then.
        Some(r) if stamp < r.until && !reminders::told(&dir, &r.key) => {
            let again = when(r, &holds, stamp).max(stamp + 1);
            if again < r.until { serde_json::json!({ "again_at": again * 1000 }) } else { serde_json::json!({}) }
        }
        _ => serde_json::json!({}),
    };
    schedule();
    answer.to_string()
}

/// `decide`, for Java (android/main.cpp); a panic answers `null`, and Java
/// shows the words it was given with the list rather than nothing. Given back
/// to `sioul_string_free`.
///
/// # Safety
/// `key` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_event_decide(key: *const c_char) -> *mut c_char {
    // SAFETY: `key` is null or a C string valid for the call, as this function's contract says.
    let key = unsafe { crate::alarms::key_of(key) };
    let answer = std::panic::catch_unwind(|| decide(&key)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

/// The coming list, for Java (`EventAlarms.ask`: after a restart, an update,
/// a change of time or of zone, and once a day), in `zone`, the phone's as
/// Java has it; "" when it fails, and Java keeps its list.
///
/// # Safety
/// `zone` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_event_coming(zone: *const c_char) -> *mut c_char {
    // SAFETY: `zone` is null or a C string valid for the call, as this function's contract says.
    let zone = unsafe { crate::alarms::key_of(zone) };
    let answer = std::panic::catch_unwind(|| {
        // The zone the phone just said, unless one is chosen in Settings (`zone::chosen_or`).
        let zone = sioul_core::zone::chosen_or(TimeZone::get(&zone).unwrap_or_else(|_| TimeZone::system()));
        let json = coming(&Zoned::now().with_time_zone(zone));
        if let Ok(mut given) = GIVEN.lock() {
            *given = json.clone();
        }
        json
    })
    .unwrap_or_default();
    // The calls' table too, at this daily look: its frames never run out while Sioul stays closed.
    let _ = std::panic::catch_unwind(|| crate::calls::refresh(false));
    crate::alarms::handed(answer)
}

/// A reminder as Android's notification shows it (EventAlarms.java, through
/// Notices): the notice of the model (`Reminder::notice`, `notice_json`),
/// tagged by its key, about the event it opens, by its address and its time
/// (`sioul:event/<UID>#1793520000`, `remind::event_time`; review of 590eb18,
/// later 6), never its file (the leftovers round): Android's intent carries
/// no path of Sioul's files. Given back when the notification is tapped,
/// and found again by its UID (`opened`). With `key`, for EventAlarms to
/// take its alarm away. None once it no longer lasts (its event begun).
fn shown(reminder: &Reminder, now: i64) -> Option<serde_json::Value> {
    let notice = reminder.notice(&tr().text("reminder-open", None));
    // An event's own alarm Sioul rings here (no calendar app does): to do, passing as its row pierces.
    let mut json = notice_json(&reminder.key, &notice, reminder.kind == Kind::Alarm, None, now)?;
    json["key"] = reminder.key.clone().into();
    Some(json)
}

/// What notifications tapped while Sioul was away, or before it started,
/// open, each once, in the order they were tapped (ReminderOpener.java keeps
/// them five minutes at most): ("", its thing's address, where this phone
/// has it: an event's file and time, found by its address, `event_file`),
/// which the window opens by its one route (`open`); or as an older Sioul's
/// notice said it: ("event", its file and time), ("porch", ""),
/// ("mailbox", ""), a closing sheet ("review", "work:2026-10-14"), a block's
/// question ("need", its key), a message ("mail", its address).
pub(crate) fn opened() -> Vec<(String, String, String)> {
    #[cfg(target_os = "android")]
    {
        let mut buffer = vec![0u8; 4096];
        // SAFETY: android/main.cpp's; it writes at most `buffer.len()` bytes into `buffer`, its zero included.
        let found = unsafe { sioul_android_take_reminder_opened(buffer.as_mut_ptr().cast(), buffer.len() as i32) };
        if found {
            let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
            let said: Vec<serde_json::Value> = serde_json::from_slice(&buffer[..end]).unwrap_or_default();
            let refs = std::sync::OnceLock::new();
            return said
                .iter()
                .filter_map(opened_of)
                .filter_map(|(kind, key)| match kind.as_str() {
                    // A thing by its address; an event's with its file and time on this phone, read now (gone meanwhile: the agenda).
                    "notice" => {
                        let hint = if is_event_address(&key) { event_file(&key, refs.get_or_init(sioul_core::agenda::event_refs)).unwrap_or_default() } else { String::new() };
                        Some((String::new(), key, hint))
                    }
                    // An event by its address: its file on this phone, read now (an event gone meanwhile opens nothing).
                    "event" => event_file(&key, refs.get_or_init(sioul_core::agenda::event_refs)).map(|file| (kind, String::new(), file)),
                    _ => Some((kind, String::new(), key)),
                })
                .collect();
        }
    }
    Vec::new()
}

/// What a tapped notification's {kind, key} opens (ReminderOpener.java keeps
/// it): only what Sioul's own notifications open; none for anything else.
/// "notice": a thing by its address (`is_notice_address`), what every notice
/// of this Sioul carries. As an older Sioul's notice said it: a message
/// ("mail") by its address alone (`mid:…`, `sioul:mail/…`), never a file
/// (new mail's notification of one message: the unified mailbox's part 2);
/// an event, the Porch, the mailbox, a block's question, a closing sheet.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn opened_of(said: &serde_json::Value) -> Option<(String, String)> {
    let key = said["key"].as_str().unwrap_or_default();
    let kind = said["kind"].as_str().filter(|kind| match *kind {
        "notice" => is_notice_address(key),
        // "mailbox": new mail's notification of several on a phone (the unified mailbox's part 5).
        "event" | "porch" | "need" | "mailbox" => true,
        "mail" => is_mail_address(key),
        "review" => {
            let (sheet, day) = key.split_once(':').map_or((key, None), |(sheet, day)| (sheet, Some(day)));
            matches!(sheet, "work" | "night") && day.is_none_or(|day| day.parse::<jiff::civil::Date>().is_ok())
        }
        _ => false,
    })?;
    Some((kind.to_string(), key.to_string()))
}

/// A thing's address as a notice of Sioul's carries it on a phone
/// (`notice::address`, the links'), never a file: an event's
/// (`is_event_address`), a message's (`is_mail_address`; a code's is its
/// message's), the mailbox, the Porch, a need (`sioul:need/<block>`), the end
/// of the work day or of the whole day (`sioul:day/work:2026-10-14`), or
/// another thing of Sioul's by its kind and id (a task, a paper, a contract,
/// a budget's line: a tablet's dates), its parts never "." nor "..".
/// ReminderOpener.isNoticeAddress reads the same (EventOpenCheck).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn is_notice_address(key: &str) -> bool {
    if key.len() > 1024 {
        return false;
    }
    if is_event_address(key) || is_mail_address(key) || key == sioul_core::notice::address::MAILBOX || key == sioul_core::notice::address::PORCH {
        return true;
    }
    if let Some(day) = key.strip_prefix("sioul:day/") {
        let Some((sheet, date)) = day.split_once(':') else { return false };
        return matches!(sheet, "work" | "night") && date.len() == 10 && date.parse::<jiff::civil::Date>().is_ok();
    }
    let Some((kind, rest)) = key.strip_prefix("sioul:").and_then(|rest| rest.split_once('/')) else { return false };
    let plain = |part: &str| !part.is_empty() && part != "." && part != ".." && part.bytes().all(|b| b.is_ascii_graphic() && b != b'\\');
    let known = matches!(kind, "need" | "task" | "paper" | "contract" | "budget" | "movement" | "letter");
    // A need's block is one part ("meal:0"); the others' ids may hold a slash of their own.
    known && if kind == "need" { plain(rest) && !rest.contains('/') } else { rest.split('/').all(plain) }
}

/// An event's address as its reminder carries it (`shown`):
/// `sioul:event/<UID>` (the UID as links write it: a slash, a "#" encoded),
/// then "#" and the time it reminds of when known; never a file.
/// ReminderOpener.isEventAddress reads the same (EventOpenCheck).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn is_event_address(key: &str) -> bool {
    let Some(rest) = key.strip_prefix("sioul:event/") else { return false };
    let (uid, start) = rest.split_once('#').map_or((rest, None), |(uid, start)| (uid, Some(start)));
    let plain = !uid.is_empty() && !uid.contains(['/', '\\']) && uid != "." && uid != "..";
    plain && start.is_none_or(|start| !start.is_empty() && start.bytes().all(|b| b.is_ascii_digit()))
}

/// What an event's tapped reminder opens on this device: its file and the
/// time it reminds of ("…/yoga.ics#1793520000", as `CardHost.eventTime`
/// reads it), the event found by the UID its address names (`is_event_address`),
/// in `refs` (`agenda::event_refs`); none when no event has it (deleted
/// meanwhile). A notice an older Sioul posted names the file: as before.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn event_file(key: &str, refs: &[sioul_core::agenda::EventRef]) -> Option<String> {
    if !is_event_address(key) {
        return Some(key.to_string());
    }
    let (address, start) = key.split_once('#').map_or((key, None), |(address, start)| (address, start.parse::<i64>().ok()));
    let uid = sioul_core::links::id_of(address);
    let event = refs.iter().find(|r| r.uid == uid)?;
    Some(crate::remind::event_time(&event.key, start))
}

/// A message's address as a notification may carry it: `mid:<id>` (as links
/// write it: a slash in an id is encoded) or `sioul:mail/<account>/<digest>`;
/// never a file.
pub(crate) fn is_mail_address(key: &str) -> bool {
    use sioul_core::mailfind::{Address, DIGEST_SCHEME};
    let named = (key.starts_with("mid:") && !key.contains('/')) || key.starts_with(DIGEST_SCHEME);
    named && Address::parse(key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// New mail's notification of one message opens it on a phone (the
    /// unified mailbox's part 2): what Java is given names it by its address,
    /// never a file; a tap kept for Sioul opens "mail" with an address, and a
    /// path or anything else under "mail" opens nothing.
    #[test]
    fn a_mail_notification_opens_its_message_by_its_address() {
        let note = |open: &str| mail_note_json("New mail", "Ana, Lunch", false, open, &[], serde_json::json!({}))["open"].as_str().unwrap_or_default().to_string();
        assert_eq!(note("mid:lunch-1@example.org"), "mid:lunch-1@example.org");
        assert_eq!(note("sioul:mail/home/0123456789abcdef"), "sioul:mail/home/0123456789abcdef");
        assert_eq!(note(""), "", "several: no message named (Java opens the mailbox)");
        assert_eq!(note("/data/user/0/sioul/files/mail/home/cur/1.U1-1.sioul:2,S"), "", "never a file");
        let tap = |key: &str| opened_of(&serde_json::json!({ "kind": "mail", "key": key }));
        assert_eq!(tap("mid:lunch-1@example.org"), Some(("mail".into(), "mid:lunch-1@example.org".into())));
        assert_eq!(tap("sioul:mail/home/0123456789abcdef"), Some(("mail".into(), "sioul:mail/home/0123456789abcdef".into())));
        for refused in ["/data/user/0/sioul/files/mail/home/cur/1.U1-1.sioul:2,S", "mid:../../etc/passwd", "file:///sdcard/x.eml", "lunch-1@example.org", ""] {
            assert_eq!(tap(refused), None, "{refused}");
        }
        assert_eq!(opened_of(&serde_json::json!({ "kind": "porch", "key": "" })), Some(("porch".into(), String::new())), "an older build's notice of several: the Porch, as before");
        assert_eq!(opened_of(&serde_json::json!({ "kind": "mailbox" })), Some(("mailbox".into(), String::new())), "several: the mailbox (part 5)");
    }

    /// A reminder's notice as Notices.java is given it, its life known: pure
    /// (`notice_value`), the minutes five, the day ending in ten hours.
    fn posted(reminder: &Reminder, now: i64) -> Option<serde_json::Value> {
        let moment = sioul_core::notice::Moment { day_ends: now + 36_000, ..Default::default() };
        let notice = reminder.notice("Open");
        sioul_core::notice::lasting(&notice, moment, 5, now).map(|lasting| notice_value(&reminder.key, &notice, lasting, false, None, serde_json::json!({})))
    }

    /// A batch of two on a phone: each message its own opener, its words and
    /// its address, never its file (the notifications' phase 2); `open` as an
    /// older MailNotes.java reads it, none for several.
    #[test]
    fn a_batch_of_two_opens_each_message() {
        use sioul_core::mailnote::{Letter, openers};
        let letter = |i: usize, sender: &str, subject: &str| Letter { key: i.to_string(), sender: sender.into(), subject: subject.into(), through: false, address: format!("mid:letter-{i}@example.org"), notice: format!("mail:home:{i}"), file: format!("/data/user/0/sioul/files/mail/home/cur/{i}.sioul"), arrived: 0, message: format!("id:{i}") };
        let two = [letter(1, "Ana", "Lunch"), letter(2, "Noa", "The lease")];
        let mut opens = openers(&sioul_core::i18n::Translator::new("en"), &two);
        opens.push(sioul_core::mailnote::Opener { label: "A file".into(), address: "/data/user/0/sioul/files/mail/home/cur/3.sioul".into(), hint: String::new() });
        let json = mail_note_json("Two e-mails", "Ana, Lunch · Noa, The lease", false, "", &opens, serde_json::json!({}));
        assert_eq!(json["open"], "", "several: no one message for an older Java");
        assert_eq!(json["opens"], serde_json::json!([{ "label": "Ana, Lunch", "address": "mid:letter-1@example.org" }, { "label": "Noa, The lease", "address": "mid:letter-2@example.org" }]), "never a file");
        let text = json.to_string();
        assert!(!text.contains("/data/user") && !text.contains("hint"), "{text}");
        // Each opens through ReminderOpener as "mail", by its address.
        for open in json["opens"].as_array().unwrap() {
            let address = open["address"].as_str().unwrap();
            assert_eq!(opened_of(&serde_json::json!({ "kind": "mail", "key": address })), Some(("mail".into(), address.to_string())));
        }
        // One message: `open` names it, and `opens` too.
        let one = mail_note_json("One e-mail", "Ana, Lunch", false, "mid:letter-1@example.org", &openers(&sioul_core::i18n::Translator::new("en"), &two[..1]), serde_json::json!({}));
        assert_eq!((one["open"].as_str(), one["opens"][0]["label"].as_str(), one["opens"][0]["address"].as_str()), (Some("mid:letter-1@example.org"), Some("Open"), Some("mid:letter-1@example.org")));
    }

    /// A phone's notification of an event opens that time of it when tapped:
    /// its address and its time, given back as they are (review of 590eb18,
    /// later 6), never its file (the leftovers round); found again by its UID
    /// as its file and time, as the window opens it. A notice an older Sioul
    /// posted, its file in its intent, opens as before. The time to leave is
    /// a notice to know, tagged by its reminder's key, that lasts until its
    /// event starts, then is no more.
    #[test]
    fn an_event_s_notification_names_its_address_and_time() {
        let reminder = Reminder { key: "before:yoga/1#x:1793520000".into(), kind: sioul_core::reminders::Kind::Before, at: 1_793_519_100, until: 1_793_520_000, title: "Yoga".into(), body: String::new(), target: "/c/yoga.ics".into(), work: false, starts: Some(1_793_520_000), names: None, event: Some(sioul_core::links::event_uri("yoga/1#x")) };
        let now = 1_793_519_100;
        let note = posted(&reminder, now).unwrap();
        let said = note["about"].as_str().unwrap_or_default().to_string();
        assert_eq!(said, "sioul:event/yoga%2F1%23x#1793520000");
        assert!(!said.contains("yoga.ics") && is_event_address(&said) && is_notice_address(&said));
        assert_eq!(note["actions"], serde_json::json!([{ "label": "Open", "open": said }]), "Open: the same thing");
        assert_eq!((note["tag"].as_str(), note["class"].as_str(), note["group"].as_str(), note["kind"].as_str()), (Some(reminder.key.as_str()), Some("informative"), Some("informative"), Some("before")));
        assert_eq!((note["lasts_ms"].as_i64(), note["ends"].as_i64(), note["pierce"].as_bool()), (Some(900_000), Some(1_793_520_000_000), Some(false)), "until its event starts");
        assert_eq!(posted(&reminder, 1_793_520_000), None, "its event begun: nothing");
        assert_eq!(posted(&Reminder { starts: None, ..reminder.clone() }, now).unwrap()["about"], "sioul:event/yoga%2F1%23x", "no time known: its address");
        let without = posted(&Reminder { event: None, ..reminder.clone() }, now).unwrap();
        assert_eq!((without["about"].as_str(), without["actions"].as_array().map(Vec::len)), (Some(""), Some(0)), "no address: none, never its file (Sioul as by its icon)");
        // The list's entry: its words, its address and its start, for the words Java says when Rust is silent.
        let entry = listed_entry(&reminder, reminder.at);
        assert_eq!((entry["about"].as_str(), entry["ends"].as_i64(), entry["at"].as_i64()), (Some(said.as_str()), Some(1_793_520_000_000), Some(1_793_519_100_000)));
        let tapped = opened_of(&serde_json::json!({ "kind": "notice", "key": said })).unwrap();
        assert_eq!(tapped, ("notice".to_string(), said.clone()), "given back as it was");
        let refs = [sioul_core::agenda::EventRef { key: "/c/other.ics".into(), uid: "other".into(), ..Default::default() }, sioul_core::agenda::EventRef { key: "/c/yoga.ics".into(), uid: "yoga/1#x".into(), ..Default::default() }];
        assert_eq!(event_file(&tapped.1, &refs).as_deref(), Some("/c/yoga.ics#1793520000"), "its file and time, as the window opens it");
        assert_eq!(event_file("sioul:event/yoga%2F1%23x", &refs).as_deref(), Some("/c/yoga.ics"));
        assert_eq!(event_file("sioul:event/gone#1793520000", &refs), None, "an event gone meanwhile: nothing");
        // A notice an older Sioul (4d81220) posted: its file and time, opened as before.
        let older = opened_of(&serde_json::json!({ "kind": "event", "key": "/c/yoga.ics#1793520000" })).unwrap();
        assert_eq!(event_file(&older.1, &refs).as_deref(), Some("/c/yoga.ics#1793520000"));
        assert_eq!(opened_of(&serde_json::json!({ "kind": "event", "key": said })), Some(("event".to_string(), said.clone())), "an older build's notice by its address");
        for refused in ["/c/yoga.ics#1", "sioul:event/", "sioul:event/a/b", "sioul:event/..", "sioul:event/a#b", "sioul:event/a#1#2", "sioul:event/a\\b", "sioul:task/a", "sioul:event/a#"] {
            assert!(!is_event_address(refused), "{refused}");
        }
    }

    /// A phone tells the time to leave, and an event's own alarm only where
    /// no calendar app rings it in Sioul's place (the notifications' decision
    /// 3); the working day before is made no more; nothing else is given to
    /// Android's alarm clock. Which of the two is said once, and again only
    /// once it changes; the calendar app's notice opens its notifications.
    #[test]
    fn a_phone_rings_an_event_s_own_alarm_only_where_no_calendar_app_does() {
        use sioul_core::reminders::Kind as Of;
        assert!(told_on_android(Of::Before, true) && told_on_android(Of::Before, false));
        assert!(!told_on_android(Of::Alarm, true), "a calendar app rings it");
        assert!(told_on_android(Of::Alarm, false), "none does: Sioul, as before");
        for kind in [Of::Event, Of::Asked, Of::Wait, Of::Payment, Of::Paper, Of::Contract, Of::Money] {
            assert!(!told_on_android(kind, false) && !told_on_android(kind, true), "{kind:?}");
        }
        let calendar = ("org.example.calendar".to_string(), "Calendar".to_string());
        assert_eq!(handover_word(Some(&calendar)), "calendar:org.example.calendar");
        assert_eq!(handover_word(None), "sioul");
        let said = handover_json(Some(&calendar), 1_794_000_000).unwrap();
        assert_eq!((said["tag"].as_str(), said["class"].as_str(), said["kind"].as_str(), said["about"].as_str()), (Some("event-alarms"), Some("informative"), Some("minor"), Some("")));
        assert!(said["body"].as_str().is_some_and(|b| b.contains("Calendar")), "{said}");
        assert_eq!(said["actions"][0]["settings"], "org.example.calendar", "its notifications in Android's settings");
        let sioul = handover_json(None, 1_794_000_000).unwrap();
        assert_eq!(sioul["actions"].as_array().map(Vec::len), Some(0));
        // An event's own alarm Sioul rings: to do, until a quarter of an hour after its event began.
        let alarm = Reminder { key: "alarm:yoga:1793520000:0".into(), kind: Of::Alarm, at: 1_793_520_000, until: 1_793_520_900, title: "Yoga".into(), body: String::new(), target: "/c/yoga.ics".into(), work: false, starts: Some(1_793_520_000), names: None, event: Some(sioul_core::links::event_uri("yoga")) };
        let note = posted(&alarm, 1_793_520_000).unwrap();
        assert_eq!((note["class"].as_str(), note["lasts_ms"].as_i64(), note["ends"].as_i64()), (Some("actionable"), Some(0), Some(1_793_520_900_000)));
        assert_eq!(listed_entry(&alarm, alarm.at)["ends"].as_i64(), Some(1_793_520_900_000));
    }

    /// The addresses EventOpenCheck.java holds ReminderOpener.isEventAddress
    /// and ReminderOpener.isNoticeAddress to, read from that file: Rust's
    /// `is_event_address` and `is_notice_address` agree on each, so that
    /// neither side changes alone.
    #[test]
    fn rust_and_java_read_an_event_s_address_alike() {
        let check = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/jvm-checks/EventOpenCheck.java")).unwrap();
        let listed = |name: &str| -> Vec<String> {
            let start = format!("String[] {name} = {{ ");
            let at = check.find(&start).unwrap() + start.len();
            let end = at + check[at..].find(" };").unwrap();
            check[at..end].split(',').map(str::trim).filter(|t| t.starts_with('"')).map(|t| unescaped(t.trim_matches('"'))).collect()
        };
        let (taken, refused) = (listed("taken"), listed("refused"));
        assert!(taken.len() >= 4 && refused.len() >= 10, "{taken:?} {refused:?}");
        for key in &taken {
            assert!(is_event_address(key), "taken in Java: {key}");
        }
        for key in &refused {
            assert!(!is_event_address(key), "refused in Java: {key}");
        }
        // Every thing a notice is about: ReminderOpener.isNoticeAddress's lists, read alike.
        let (taken, refused) = (listed("noticeTaken"), listed("noticeRefused"));
        assert!(taken.len() >= 10 && refused.len() >= 15, "{taken:?} {refused:?}");
        for key in &taken {
            assert!(is_notice_address(key), "taken in Java: {key}");
        }
        for key in &refused {
            assert!(!is_notice_address(key), "refused in Java: {key}");
        }
    }

    #[test]
    fn held_reminders_are_asked_about_again_when_the_hold_ends() {
        use sioul_core::reminders::Wait;
        let stamp = 1_800_000_000;
        let usual = Holds::usual;
        assert_eq!(again_at(&usual(Wait::Sleep { from: stamp - 3600, until: stamp + 7200 }), stamp), stamp + 7200, "at waking");
        assert_eq!(again_at(&usual(Wait::Free { until: stamp + 600 }), stamp), stamp + 600, "at the end of free time");
        assert_eq!(again_at(&usual(Wait::Free { until: stamp }), stamp), stamp + 60, "never at once again");
        assert_eq!(again_at(&usual(Wait::Paused), stamp), stamp + AGAIN);
        // In steps: the same from one second to the next, so the list given to Java stays the same.
        for later in 0..AGAIN {
            assert_eq!(again_at(&usual(Wait::Paused), stamp + later), stamp + AGAIN, "{later} s on");
        }
        for later in 0..60 {
            assert_eq!(again_at(&usual(Wait::Free { until: stamp }), stamp + later), stamp + 60, "{later} s on");
        }
        assert_eq!(again_at(&usual(Wait::Paused), stamp + AGAIN), stamp + 2 * AGAIN, "never now");
        // A reminder's alarm: its time; due and free, at once; held (a train after waking), at waking.
        let reminder = |at: i64, until: i64| Reminder { key: "before:train:1".into(), kind: Kind::Before, at, until, title: String::new(), body: String::new(), target: String::new(), work: false, starts: Some(until), names: None, event: None };
        let night = usual(Wait::Sleep { from: stamp - 8 * 3600, until: stamp + 900 });
        assert_eq!(when(&reminder(stamp + 600, stamp + 3600), &usual(Wait::Nothing), stamp), stamp + 600);
        assert_eq!(when(&reminder(stamp - 60, stamp + 3600), &usual(Wait::Nothing), stamp), stamp + 1);
        assert_eq!(when(&reminder(stamp - 60, stamp + 3600), &night, stamp), stamp + 900, "kept in the list given, at waking");
        // Held during do-not-disturb (the matrix changed): asked again in a quarter of an hour.
        let mut focus = usual(Wait::Nothing);
        focus.now.dnd = true;
        focus.attention.set(sioul_core::attention::Row::Own(sioul_core::attention::Kind::Before), sioul_core::attention::Column::Dnd, sioul_core::attention::Level::Later).unwrap();
        assert_eq!(when(&reminder(stamp - 60, stamp + 3600), &focus, stamp), stamp + AGAIN);
    }

    /// A tapped notification opens only what Sioul's own open: a thing by
    /// its address (a notice of this Sioul: an event, a message, a code's
    /// message, the mailbox, the Porch, a need, the end of the work day or of
    /// the whole day, a tablet's task or paper); as an older Sioul's notice
    /// said it, an event, the Porch, a closing sheet, a block's question;
    /// nothing else, never a file.
    #[test]
    fn a_tapped_notice_opens_only_what_sioul_offers() {
        let of = |json: &str| opened_of(&serde_json::from_str(json).unwrap());
        assert_eq!(of(r#"{"kind":"review","key":"work"}"#), Some(("review".into(), "work".into())));
        assert_eq!(of(r#"{"kind":"review","key":"night"}"#), Some(("review".into(), "night".into())));
        assert_eq!(of(r#"{"kind":"need","key":"sleep"}"#), Some(("need".into(), "sleep".into())));
        assert_eq!(of(r#"{"kind":"event","key":"work/1.ics"}"#), Some(("event".into(), "work/1.ics".into())));
        assert_eq!(of(r#"{"kind":"porch"}"#), Some(("porch".into(), String::new())));
        assert_eq!(of(r#"{"kind":"review","key":"work:2026-10-14"}"#), Some(("review".into(), "work:2026-10-14".into())));
        assert_eq!(of(r#"{"kind":"review","key":"night:2026-10-14"}"#), Some(("review".into(), "night:2026-10-14".into())));
        assert_eq!(of(r#"{"kind":"review","key":"work:yesterday"}"#), None);
        assert_eq!(of(r#"{"kind":"review","key":"settings"}"#), None);
        assert_eq!(of(r#"{"kind":"settings:words","key":""}"#), None);
        assert_eq!(of(r#"{"key":"work"}"#), None);
        let notice = |key: &str| of(&serde_json::json!({ "kind": "notice", "key": key }).to_string());
        for taken in ["sioul:event/yoga-1@example.org#1793520000", "mid:code-1@example.org", "sioul:mail/home/0123456789abcdef", "sioul:mailbox", "sioul:porch", "sioul:need/meal:0", "sioul:need/sleep", "sioul:day/work:2026-11-14", "sioul:day/night:2026-11-14", "sioul:task/a%2Fb", "sioul:budget/#rent", "sioul:paper/passport"] {
            assert_eq!(notice(taken), Some(("notice".into(), taken.into())), "{taken}");
        }
        for refused in ["/data/user/0/com.aurelienpierre.sioul/files/mail/home/cur/1", "file:///sdcard/x.eml", "sioul:need/", "sioul:need/../x", "sioul:need/a/b", "sioul:day/work:yesterday", "sioul:day/lunch:2026-11-14", "sioul:day/work", "sioul:task/..", "sioul:task/a/../b", "sioul:settings/words", "sioul:move", "sioul:site/chat#1", "sioul:dose/a", "mid:../../etc/passwd", "sioul:task/a\\b", ""] {
            assert_eq!(notice(refused), None, "{refused}");
        }
    }

    /// A Java string literal's text: its `\"` and `\uXXXX` read.
    fn unescaped(literal: &str) -> String {
        let mut out = String::new();
        let mut chars = literal.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.extend(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32));
                }
                Some(other) => out.push(other),
                None => {}
            }
        }
        out
    }

    /// A one-time code's notice on a phone as Rust gives it (`code_note`):
    /// something to do, until it expires, passing one of Sioul's modes as
    /// the matrix said, Copy and Open the message, its lock screen its title
    /// alone; the very words android/jvm-checks/NoticesCheck.java checks
    /// Notices on (CODE), read from that file. An expired code is none.
    #[test]
    fn a_code_s_notice_as_java_reads_it() {
        use sioul_core::notice::Kind as Of;
        let check = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/jvm-checks/NoticesCheck.java")).unwrap();
        let start = "static final String CODE = \"";
        let line = check.lines().map(str::trim).find(|line| line.starts_with(start)).unwrap();
        let java: serde_json::Value = serde_json::from_str(&unescaped(line[start.len()..].strip_suffix("\";").unwrap())).unwrap();
        let now = 1_794_000_000;
        let notice = Notice::new("mid:code-1@example.org", Of::Own(sioul_core::attention::Kind::Codes), "A code from example.org", "482913, valid about 10 minutes")
            .until(Some(now + 600))
            .action("Copy the code", Act::Copy("482913".into()))
            .action("Open the message", Act::Open { address: "mid:code-1@example.org".into(), hint: "/m/cur/1".into() });
        let moment = sioul_core::notice::Moment { day_ends: now + 36_000, ..Default::default() };
        let lasting = sioul_core::notice::lasting(&notice, moment, 5, now).unwrap();
        let said = notice_value(&notice.key(), &notice, lasting, true, Some((&notice.title, "")), serde_json::json!({}));
        assert_eq!(said, java);
        assert!(!said["public"].to_string().contains("482913"), "never the code on the lock screen");
        assert!(!said.to_string().contains("/m/cur/1"), "never its file");
        assert_eq!(sioul_core::notice::lasting(&notice, moment, 5, now + 600), None, "expired: none");
        // Something to know never pierces, whatever its source says.
        let leave = Notice::new("sioul:event/a#1794000600", Of::Own(sioul_core::attention::Kind::Before), "Leave", "").until(Some(now + 600));
        let lasting = sioul_core::notice::lasting(&leave, moment, 5, now).unwrap();
        assert_eq!(notice_value("before:a", &leave, lasting, true, None, serde_json::json!({}))["pierce"], false);
    }

    /// The end of work's notice, the night's and a meal's as Rust gives them
    /// to Java (`day_note`, `notice_value`), in English: the very words
    /// android/jvm-checks/DayNotesCheck.java checks Notices on (WORK, NIGHT,
    /// GIVEN, MEAL), read from that file, so that neither side changes alone.
    #[test]
    fn the_days_notices_as_java_reads_them() {
        use sioul_core::attention::Kind as Row;
        use sioul_core::notice::{Class, address};
        let check = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/jvm-checks/DayNotesCheck.java")).unwrap();
        let java = |name: &str| -> serde_json::Value {
            let start = format!("static final String {name} = \"");
            let line = check.lines().map(str::trim).find(|line| line.starts_with(&start)).unwrap();
            let literal = line[start.len()..].strip_suffix("\";").unwrap();
            serde_json::from_str(&unescaped(literal)).unwrap()
        };
        let en = sioul_core::i18n::Translator::new("en");
        let offer = |label: &str, kind: &'static str, key: &str| crate::reviews::Offered { label: en.text(label, None), kind, key: key.into() };
        let lasting = Lasting { class: Class::Informative, lasts: 300, ends: Some(1_792_000_000) };
        let note = |key: &str, notice: Notice| notice_value(key, &notice, lasting, false, None, serde_json::json!({}));
        let work = crate::reviews::closing_notice(&address::work_day("2026-10-14"), Row::WorkOver, &en.text("review-notice-title", None), &en.text("review-notice-text", None), vec![offer("review-close-work", "review", "work:2026-10-14")], None);
        assert_eq!(note("day:work:2026-10-14", work), java("WORK"));
        let night = crate::reviews::closing_notice(&address::need("sleep"), Row::Needs, "Wind down", "22:30", vec![offer("review-close-night", "review", "night:2026-10-14"), offer("need-open", "need", "sleep")], None);
        assert_eq!(note("day:night:2026-10-14", night), java("NIGHT"));
        let given = crate::reviews::closing_notice(&address::need("sleep"), Row::Needs, "Wind down", "22:30", vec![offer("need-open", "need", "sleep")], None);
        assert_eq!(note("day:night:2026-10-14", given), java("GIVEN"));
        let meal = crate::reviews::closing_notice(&address::need("meal:0"), Row::Needs, "Lunch", "12:30", vec![offer("need-open", "need", "meal:0")], None);
        assert_eq!(note("day:need:1792000000:meal:0", meal), java("MEAL"));
    }
}

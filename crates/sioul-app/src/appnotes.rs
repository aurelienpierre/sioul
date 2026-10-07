// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Other apps' notifications on a phone (docs/android.md, "Notifications from
//! other apps"). Android's listener (AppNotes.java, in a process of its own,
//! ":listener") asks here, for each notification of another app, whether it
//! comes now or how long it waits (`sioul_appnotes_decide`), and, when
//! something moved the times (a pause ended, the hours or the choices
//! changed), how long what it holds waits now (`sioul_appnotes_review`). The
//! rules are `sioul_core::appnotes`; who may reach you when, `sioul_core::reach`.
//!
//! Also here, for the window: Settings ▸ Other apps (`setup`, `change`), and
//! the Porch's line on what waits and what came back (`porch_line`). On a
//! computer, nothing of it runs: the listener is the phone's.

use crate::backend::{load_config, tr};
use jiff::Zoned;
use serde_json::{Value, json};
use sioul_core::appnotes::{self, AppKind, Ask, Choices, Decision, Gate, Kind, Ledger, Live, Posted, Sender, Through, Why};
use sioul_core::areas::Area;
use sioul_core::config::Config;
use sioul_core::everywhere::People;
use sioul_core::porch::{By, Senders};
use sioul_core::reach::{Clock, Reach, Who};
use std::ffi::c_char;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

#[cfg(target_os = "android")]
unsafe extern "C" {
    /// AppNotes.call (android/main.cpp): a verb and its JSON, a JSON answer or null.
    fn sioul_android_appnotes(verb: *const c_char, json: *const c_char) -> *mut c_char;
    /// An answer of `sioul_android_appnotes` given back.
    fn sioul_android_dnd_free(text: *mut c_char);
}

/// Java's answer to `verb` (AppNotes.call); null elsewhere, or without one.
fn java(verb: &str, json: &str) -> Value {
    #[cfg(target_os = "android")]
    {
        use std::ffi::{CStr, CString};
        let (Ok(verb), Ok(json)) = (CString::new(verb), CString::new(json)) else { return Value::Null };
        // SAFETY: two zero-terminated texts, valid for the call.
        let answer = unsafe { sioul_android_appnotes(verb.as_ptr(), json.as_ptr()) };
        if answer.is_null() {
            return Value::Null;
        }
        // SAFETY: a zero-terminated text from sioul_android_appnotes, read before it is given back.
        let text = unsafe { CStr::from_ptr(answer) }.to_string_lossy().to_string();
        // SAFETY: given back once, as main.cpp asks; not read after.
        unsafe { sioul_android_dnd_free(answer) };
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (verb, json);
        Value::Null
    }
}

/// When a file last changed; none when it is not there.
fn changed(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The clock and the matrix as they stand, kept a minute in the listener's
/// process: read again sooner when the pause or free time, Health, the hours
/// or the settings change. Asked at every notification.
fn live(config: &Config, now: &Zoned) -> Live {
    type Key = (i64, [Option<SystemTime>; 4]);
    static KEPT: Mutex<Option<(Key, Live)>> = Mutex::new(None);
    let files = [
        sioul_core::quiet::Overrides::default_path(),
        sioul_core::health::Health::default_path(),
        sioul_core::needs::Days::default_path(),
        sioul_core::config::config_dir().join("config.toml"),
    ];
    let key: Key = (now.timestamp().as_second() / 60, files.each_ref().map(|f| changed(f)));
    if let Ok(kept) = KEPT.lock()
        && let Some((known, live)) = kept.as_ref()
        && *known == key
    {
        return live.clone();
    }
    let live = Live { clock: Clock::load(config, now), reach: Reach::load(config) };
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((key, live.clone()));
    }
    live
}

/// Who wrote, as your address books and lists say (`porch::Senders`): none
/// when nobody knows a name; and whether do-not-disturb's list holds them,
/// while it holds (`gate`).
fn who_of(sender: &Sender, senders: &Senders, gate: Option<&People>, config: &Config) -> (Option<Who>, bool) {
    match sender {
        Sender::Known { numbers, emails, contact } => {
            let by_number = numbers.iter().map(|n| senders.judge_number(n)).find(|j| j.by != By::Default).map(|j| j.who);
            let by_address = emails.iter().map(|e| senders.who(e)).find(|w| *w != Who::Stranger);
            let region = sioul_core::reach::region(config);
            let admitted = gate.is_some_and(|people| numbers.iter().any(|n| people.admits_number(n, region)) || emails.iter().any(|e| people.admits_address(e)));
            // In the phone's address book though not in Sioul's: someone you know.
            let unknown = if *contact { Who::Neutral } else { Who::Stranger };
            (Some(by_number.or(by_address).unwrap_or(unknown)), admitted)
        }
        Sender::Named { name, book } => (senders.judge_name(name).map(|j| appnotes::named(Some(j.who), *book)), false),
        Sender::Unknown => (None, false),
    }
}

/// What a notification's source is for: the app's, as you set it; else, for
/// mail, the address it came to, when it is one of your accounts' (unsaid: work).
fn area_of(posted: &Posted, kind: &Kind, choices: &Choices, config: &Config) -> Option<Area> {
    choices.app(&posted.package).area.or_else(|| match kind {
        Kind::People(talk) if !talk.to.is_empty() => config
            .accounts
            .iter()
            .find(|a| a.address.as_deref().is_some_and(|address| address.trim().eq_ignore_ascii_case(&talk.to)))
            .map(|a| a.area.as_deref().and_then(Area::parse).unwrap_or(Area::WORK)),
        _ => None,
    })
}

/// Do-not-disturb's switch or a focus session, holding now: its list, and
/// when it ends when that is known.
fn gate() -> Option<(People, Option<i64>)> {
    crate::everywhere::mail_gate().map(|people| (people, crate::everywhere::now().until()))
}

/// One notification decided (`AppNotes.decide`): {"hold": milliseconds (0:
/// it comes now), "why"}. What it says is read here and dropped; the ledger
/// keeps its key hashed, its app's and conversation's names, and its time.
fn decide(json: &str) -> String {
    let Ok(posted) = serde_json::from_str::<Posted>(json) else { return "null".into() };
    let config = load_config();
    let choices = Choices::load(&Choices::default_path());
    let kind = appnotes::classify(&posted, &choices);
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let gate = gate();
    let (who, admitted) = match &kind {
        Kind::People(talk) => who_of(&talk.sender, &Senders::load(&config), gate.as_ref().map(|(people, _)| people), &config),
        _ => (None, false),
    };
    let area = area_of(&posted, &kind, &choices, &config);
    let live = live(&config, &now);
    let gathered = config.reminders.gathered_times();
    let slot = |at: &Zoned| crate::capacity::in_gain_slot(at);
    // What comes when, kind by kind (Settings ▸ Reminders and notifications).
    let notify = sioul_core::notify::Notify::of(&config);
    let ask = Ask { now: &now, clock: &live, reach: &live, notify: &notify, gathered: &gathered, area, gate: gate.as_ref().map(|(_, until)| Gate { until: *until }), slot_at: &slot, also: &[] };
    let hash = appnotes::key_hash(&posted.key);
    let path = Ledger::default_path();
    let decision = sioul_core::filelock::with_lock(&path, || {
        let mut ledger = Ledger::load(&path);
        let returning = ledger.returning(&hash, stamp);
        let mut decision = appnotes::decide(&kind, who, admitted, &choices, &ask);
        // Back at its time and held again within minutes: not again and again.
        if returning
            && let Some(until) = decision.until
            && decision.why != Why::Never
        {
            let short = until - stamp < appnotes::AGAIN_SHORT;
            let again = if short { ledger.held.get(&hash).map_or(0, |h| h.again) + 1 } else { 0 };
            if again > appnotes::AGAIN_MAX {
                decision = Decision::through(Why::Again);
            } else if let Some(held) = ledger.held.get_mut(&hash) {
                held.again = again;
            }
        }
        if returning && decision.until.is_none() {
            ledger.came_back(&hash, stamp);
        }
        ledger.note(&posted, &kind, who, admitted, area, &decision, stamp);
        ledger.forget_old(stamp, &choices);
        if let Err(e) = ledger.save(&path) {
            eprintln!("Notes: {e}");
        }
        decision
    });
    let hold = decision.until.map_or(0, |until| (until - stamp).max(1) * 1000);
    json!({ "hold": hold, "why": decision.why.id() }).to_string()
}

/// What Android holds for Sioul now, worked out again (`AppNotes.review`):
/// {"snooze": [{"key", "ms"}]}, those whose time moved by more than a minute
/// (1 s: it may come now). Marks of notifications Android no longer holds
/// (their app took them away, or they came back unseen) are let go.
fn review(json: &str) -> String {
    let asked: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let keys: Vec<String> = asked["snoozed"].as_array().map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let config = load_config();
    let choices = Choices::load(&Choices::default_path());
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let gate = gate();
    let live = live(&config, &now);
    let gathered = config.reminders.gathered_times();
    let slot = |at: &Zoned| crate::capacity::in_gain_slot(at);
    let notify = sioul_core::notify::Notify::of(&config);
    let path = Ledger::default_path();
    let again = sioul_core::filelock::with_lock(&path, || {
        let mut ledger = Ledger::load(&path);
        let present: std::collections::BTreeSet<String> = keys.iter().map(|k| appnotes::key_hash(k)).collect();
        ledger.held.retain(|hash, held| present.contains(hash) || stamp - held.since < 60);
        let mut again = Vec::new();
        for key in &keys {
            let hash = appnotes::key_hash(key);
            let Some(held) = ledger.held.get(&hash).cloned() else { continue };
            let ask = Ask { now: &now, clock: &live, reach: &live, notify: &notify, gathered: &gathered, area: held.area, gate: gate.as_ref().map(|(_, until)| Gate { until: *until }), slot_at: &slot, also: &[] };
            let until = appnotes::again(&held, &choices, &ask).until.unwrap_or(stamp);
            if (until - held.until).abs() > 60 {
                if let Some(kept) = ledger.held.get_mut(&hash) {
                    kept.until = until;
                }
                again.push(json!({ "key": key, "ms": (until - stamp).max(1) * 1000 }));
            }
        }
        ledger.forget_old(stamp, &choices);
        if let Err(e) = ledger.save(&path) {
            eprintln!("Notes: {e}");
        }
        again
    });
    json!({ "snooze": again }).to_string()
}

/// `decide`, for Java (android/main.cpp); a panic answers "null", and the
/// notification comes as it was sent. Given back to `sioul_string_free`.
///
/// # Safety
/// `json` is null, or a zero-terminated text valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_appnotes_decide(json: *const c_char) -> *mut c_char {
    let json = unsafe { crate::alarms::key_of(json) };
    let answer = std::panic::catch_unwind(|| decide(&json)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

/// `review`, for Java; "null" when it fails, and what is held keeps its time.
///
/// # Safety
/// `json` is null, or a zero-terminated text valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_appnotes_review(json: *const c_char) -> *mut c_char {
    let json = unsafe { crate::alarms::key_of(json) };
    let answer = std::panic::catch_unwind(|| review(&json)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

// ---------------------------------------------------------------- the window

/// The Porch's line on other apps (`appnotes::porch_line`); "" when nothing
/// waits nor came back lately, and on a computer (no listener there).
pub(crate) fn porch_line() -> String {
    let path = Ledger::default_path();
    if !path.exists() {
        return String::new();
    }
    appnotes::porch_line(tr(), &Ledger::load(&path), &Zoned::now())
}

/// The Porch's JSON with its line on other apps (`apps_line`), when there is one.
pub(crate) fn with_line(porch: String) -> String {
    let line = porch_line();
    if line.is_empty() {
        return porch;
    }
    match serde_json::from_str::<Value>(&porch) {
        Ok(Value::Object(mut fields)) => {
            fields.insert("apps_line".into(), Value::String(line));
            Value::Object(fields).to_string()
        }
        _ => porch,
    }
}

/// A row of the tab, as `SettingRow.qml` shows it: {key, kind, label, help, value, choices}.
fn row(key: &str, kind: &str, label: &str, help: &str, value: Value, choices: Vec<(Value, String)>) -> Value {
    json!({
        "key": key,
        "kind": kind,
        "label": label,
        "help": help,
        "value": value,
        "choices": choices.into_iter().map(|(value, label)| json!({ "value": value, "label": label })).collect::<Vec<_>>(),
    })
}

/// "09:00, 13:00 and 18:00".
fn times_in_words(times: &[String]) -> String {
    let times: Vec<String> = appnotes::times_of(times).iter().map(|t| t.strftime("%H:%M").to_string()).collect();
    match times.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), tr().text("word-and", None)),
    }
}

/// Settings ▸ Other apps, as JSON (AppNotesSetup.qml): whether this is a
/// phone, the access and its steps, the switch, the times, what rang while
/// held, then a row for each app, conversation and site seen.
pub(crate) fn setup() -> String {
    let android = cfg!(target_os = "android");
    let config = load_config();
    let choices = Choices::load(&Choices::default_path());
    let ledger = Ledger::load(&Ledger::default_path());
    let now = Zoned::now();
    let tr = tr();
    let text = |id: &str| tr.text(id, None);
    let mut times = sioul_core::i18n::args();
    times.set("times", times_in_words(&config.reminders.gathered_times()));
    let heard = if ledger.heard > 0 {
        let mut args = sioul_core::i18n::args();
        let at = jiff::Timestamp::from_second(ledger.heard).map(|t| t.to_zoned(now.time_zone().clone())).unwrap_or_else(|_| now.clone());
        args.set("when", tr.when(&at));
        tr.text("appnotes-heard", Some(&args))
    } else {
        String::new()
    };
    let mut rang: Vec<Value> = Vec::new();
    for (package, app) in &ledger.apps {
        for (channel, r) in &app.rang {
            let mut args = sioul_core::i18n::args();
            args.set("app", app.label.clone());
            args.set("channel", if r.name.is_empty() { channel.clone() } else { r.name.clone() });
            rang.push(json!({ "package": package, "channel": channel, "line": tr.text("appnotes-rang-line", Some(&args)) }));
        }
    }
    let kinds: Vec<(Value, String)> = AppKind::ALL.iter().map(|k| (json!(k.id()), text(&format!("appnotes-kind-{}", k.id())))).collect();
    let mut apps: Vec<(&String, &appnotes::SeenApp)> = ledger.apps.iter().collect();
    apps.sort_by_cached_key(|(_, a)| sioul_core::text::fold(&a.label).into_iter().collect::<String>());
    let apps: Vec<Value> = apps
        .into_iter()
        .map(|(package, seen)| {
            let chosen = choices.app(package);
            let why = appnotes::why_text(tr, &seen.why, seen.until, &now);
            json!({
                "package": package,
                "kind": row(&format!("app.{package}.kind"), "choice", &seen.label, &why, json!(chosen.kind.id()), kinds.clone()),
                "area": row(&format!("app.{package}.area"), "areas", &text("appnotes-area"), "", json!(chosen.area.map(Area::id).unwrap_or_default()), Vec::new()),
            })
        })
        .collect();
    let throughs: Vec<(Value, String)> = Through::ALL.iter().map(|t| (json!(t.id()), text(&format!("appnotes-through-{}", t.id())))).collect();
    let mut talks: Vec<(&String, &appnotes::SeenTalk)> = ledger.conversations.iter().collect();
    talks.sort_by_key(|(_, t)| -t.seen);
    let conversations: Vec<Value> = talks
        .into_iter()
        .map(|(key, seen)| {
            let label = format!("{} · {}", seen.title, seen.label);
            let help = if seen.group { text("appnotes-group") } else { String::new() };
            row(&format!("conversation.{key}"), "choice", &label, &help, json!(choices.through(key).id()), throughs.clone())
        })
        .collect();
    let at_once = vec![(json!(false), text("appnotes-site-gathered")), (json!(true), text("appnotes-site-at-once"))];
    let sites: Vec<Value> = ledger
        .sites
        .iter()
        .map(|(host, seen)| {
            let app = ledger.apps.get(&seen.app).map_or(seen.app.as_str(), |a| a.label.as_str());
            row(&format!("site.{host}"), "choice", &format!("{host} · {app}"), "", json!(choices.site.get(host).is_some_and(|s| s.at_once)), at_once.clone())
        })
        .collect();
    json!({
        "android": android,
        "access": java("access", "{}") == Value::Bool(true),
        "restricted": java("restricted", "{}") == Value::Bool(true),
        "contacts": java("contacts", "{}") == Value::Bool(true),
        "hold": row("hold", "bool", &text("appnotes-hold"), &text("appnotes-hold-help"), json!(choices.hold), Vec::new()),
        "times": tr.text("appnotes-times", Some(&times)),
        "heard": heard,
        "rang": rang,
        "apps": apps,
        "conversations": conversations,
        "sites": sites,
    })
    .to_string()
}

/// A choice of the tab, or one of Android's pages (`AppNotesSetup.act`): the tab again, as JSON.
/// "set" {key, value}: "hold", `app.<package>.kind`, `app.<package>.area`,
/// `conversation.<key>`, `site.<host>`; "open-access", "open-info",
/// "open-app" {package}, "open-channel" {package, channel}; "contacts".
pub(crate) fn change(verb: &str, json: &str) -> String {
    let asked: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    match verb {
        "set" => {
            let key = asked["key"].as_str().unwrap_or_default().to_string();
            let value = asked["value"].clone();
            let path = Choices::default_path();
            let result = sioul_core::filelock::with_lock(&path, || {
                let mut choices = Choices::load(&path);
                let ledger = Ledger::load(&Ledger::default_path());
                set(&mut choices, &ledger, &key, &value);
                choices.save(&path)
            });
            if let Err(e) = result {
                eprintln!("Notes: {e}");
            }
        }
        "open-access" | "open-info" | "open-app" | "open-channel" => {
            java(verb, json);
        }
        "contacts" => crate::steps::ask_contacts(),
        _ => {}
    }
    setup()
}

/// One choice written: the app's kind or area, a conversation's way
/// through, a site at once. What says "as usual" is taken out.
fn set(choices: &mut Choices, ledger: &Ledger, key: &str, value: &Value) {
    if key == "hold" {
        choices.hold = value.as_bool().unwrap_or(true);
    } else if let Some(rest) = key.strip_prefix("app.") {
        let Some((package, field)) = rest.rsplit_once('.') else { return };
        let mut chosen = choices.app(package);
        match field {
            "kind" => chosen.kind = value.as_str().and_then(AppKind::read).unwrap_or_default(),
            "area" => chosen.area = value.as_str().and_then(Area::parse),
            _ => return,
        }
        chosen.label = ledger.apps.get(package).map(|a| a.label.clone()).unwrap_or(chosen.label);
        if chosen.kind == AppKind::Usual && chosen.area.is_none() {
            choices.app.remove(package);
        } else {
            choices.app.insert(package.to_string(), chosen);
        }
    } else if let Some(conversation) = key.strip_prefix("conversation.") {
        let through = value.as_str().and_then(Through::read).unwrap_or_default();
        if through == Through::Usual {
            choices.conversation.remove(conversation);
        } else {
            let seen = ledger.conversations.get(conversation);
            let title = seen.map(|s| s.title.clone()).or_else(|| choices.conversation.get(conversation).map(|c| c.title.clone())).unwrap_or_default();
            let app = seen.map(|s| s.label.clone()).or_else(|| choices.conversation.get(conversation).map(|c| c.app.clone())).unwrap_or_default();
            choices.conversation.insert(conversation.to_string(), appnotes::TalkChoice { through, title, app });
        }
    } else if let Some(host) = key.strip_prefix("site.") {
        if value.as_bool().unwrap_or(false) {
            choices.site.insert(host.to_string(), appnotes::SiteChoice { at_once: true });
        } else {
            choices.site.remove(host);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_set_from_the_tab() {
        let mut choices = Choices::default();
        let mut ledger = Ledger::default();
        ledger.apps.insert("com.discord".into(), appnotes::SeenApp { label: "Discord".into(), ..Default::default() });
        ledger.conversations.insert("c1".into(), appnotes::SeenTalk { app: "com.whatsapp".into(), label: "WhatsApp".into(), title: "École".into(), group: true, seen: 1 });
        set(&mut choices, &ledger, "app.com.discord.kind", &json!("automaton"));
        set(&mut choices, &ledger, "app.com.discord.area", &json!("leisure"));
        assert_eq!(choices.app("com.discord"), appnotes::AppChoice { kind: AppKind::Automaton, area: Some(Area::LEISURE), label: "Discord".into() });
        set(&mut choices, &ledger, "app.com.discord.kind", &json!("usual"));
        set(&mut choices, &ledger, "app.com.discord.area", &json!(""));
        assert!(choices.app.is_empty(), "as usual: taken out");
        set(&mut choices, &ledger, "conversation.c1", &json!("always"));
        assert_eq!(choices.conversation["c1"], appnotes::TalkChoice { through: Through::Always, title: "École".into(), app: "WhatsApp".into() });
        set(&mut choices, &ledger, "conversation.c1", &json!("usual"));
        assert!(choices.conversation.is_empty());
        set(&mut choices, &ledger, "site.forum.example.net", &json!(true));
        assert!(choices.site["forum.example.net"].at_once);
        set(&mut choices, &ledger, "site.forum.example.net", &json!(false));
        assert!(choices.site.is_empty());
        set(&mut choices, &ledger, "hold", &json!(false));
        assert!(!choices.hold);
    }

    #[test]
    fn nothing_reaches_java_on_a_computer() {
        assert_eq!(java("access", "{}"), Value::Null);
        assert_eq!(times_in_words(&["18:00".into(), "9:00".into(), "13:00".into()]), format!("09:00, 13:00 {} 18:00", tr().text("word-and", None)));
    }
}

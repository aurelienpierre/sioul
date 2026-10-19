// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Other apps' notifications on a phone (docs/android.md, "Notifications from
//! other apps"). Android's listener (AppNotes.java, in a process of its own,
//! ":listener") asks here, for each notification of another app, whether it
//! comes now or how long it waits (`sioul_appnotes_decide`), and, when
//! something moved the times (a pause ended, the hours or the choices
//! changed), how long what it holds waits now (`sioul_appnotes_review`). The
//! rules are `sioul_core::appnotes`; what reaches you when, `sioul_core::attention`,
//! with today's slots of time for you from their file (`hours::slots`): Time
//! for you holds in this process too.
//!
//! Also here, for the window: Settings ▸ Other apps (`setup`, `change`), and
//! the Porch's line on what waits and what came back (`porch_line`). On a
//! computer, nothing of it runs: the listener is the phone's.

use crate::backend::{load_config, tr};
use crate::everywhere::Gated;
use jiff::Zoned;
use serde_json::{Value, json};
use sioul_core::appnotes::{self, AppKind, Ask, Cause, Choices, Decision, Gate, Gathering, Kind, Ledger, Live, Own, Posted, Sender, Through, Why};
use sioul_core::areas::Area;
use sioul_core::attention::Column;
use sioul_core::config::Config;
use sioul_core::everywhere::People;
use sioul_core::porch::{By, Senders};
use sioul_core::reach::{Channel, Clock, Who};
use std::ffi::c_char;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

#[cfg(target_os = "android")]
// SAFETY: declared as android/main.cpp defines them: extern "C", the same types.
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

/// Whether Android lets Sioul hold other apps' notifications here
/// (notification access): on a phone, as Java says now; None elsewhere, or
/// when Java cannot say (its library not loaded in this process).
pub(crate) fn access_here() -> Option<bool> {
    java("access", "{}").as_bool()
}

/// When a file last changed; none when it is not there.
fn changed(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The clock as it stands, kept a minute in the listener's process: read
/// again sooner when the pause or free time, Health, the hours or the
/// settings change. Asked at every notification.
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
    let live = Live { clock: Clock::load(config, now) };
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((key, live.clone()));
    }
    live
}

/// Who wrote, as your address books and lists say (`porch::Senders`): none
/// when nobody knows a name; and whether the Always through list holds them
/// (by a number or an address, never a name, which anyone can take).
fn who_of(sender: &Sender, senders: &Senders, always: &People, config: &Config) -> (Option<Who>, bool) {
    match sender {
        Sender::Known { numbers, emails, contact } => {
            let by_number = numbers.iter().map(|n| senders.judge_number(n)).find(|j| j.by != By::Default).map(|j| j.who);
            let by_address = emails.iter().map(|e| senders.who(e)).find(|w| *w != Who::Stranger);
            let region = sioul_core::reach::region(config);
            let admitted = numbers.iter().any(|n| always.admits_number(n, region)) || emails.iter().any(|e| always.admits_address(e));
            // In the phone's address book though not in Sioul's: someone you know.
            let unknown = if *contact { Who::Neutral } else { Who::Stranger };
            (Some(by_number.or(by_address).unwrap_or(unknown)), admitted)
        }
        Sender::Named { name, book } => (senders.judge_name(name).map(|j| appnotes::named(Some(j.who), *book)), false),
        Sender::Unknown => (None, false),
    }
}

/// The why of a text gathered because its number is one ARCEP keeps for
/// machines (`appnotes::why_text`, "appnotes-why-machine").
const MACHINE: &str = "machine";

/// Whether a sender's number comes from a root ARCEP keeps for automated
/// systems (`screening::Numbers`, the call screening's data), while the call
/// screening sends automated callers to voicemail (its `automated`).
fn machine_number(sender: &Sender, config: &Config, now: &Zoned) -> bool {
    let Sender::Known { numbers, .. } = sender else { return false };
    if sioul_core::screening::Screening::of(config).automated != sioul_core::screening::Spam::Voicemail {
        return false;
    }
    let roots = sioul_core::screening::shipped().automated(now.date());
    let region = sioul_core::reach::region(config);
    numbers.iter().any(|n| sioul_core::screening::national(&sioul_core::phones::key(n, region)).is_some_and(|national| sioul_core::screening::starts_with_any(&national, &roots)))
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

/// Do-not-disturb's switch or a focus session, holding now: when it ends,
/// when that is known (a layer of the matrix), and which of them holds (for
/// the log).
fn gate() -> Option<Gated> {
    crate::everywhere::gated()
}

/// One notification decided (`AppNotes.decide`): {"hold": milliseconds (0:
/// it comes now), "why"}. What it says is read here and dropped; the ledger
/// keeps its key hashed, its app's and conversation's names, and its time.
/// Only for the apps you send to your computers, while that part of the
/// sharing is on, its messages also go, sealed (`phonemsgs::note`; "shared":
/// a line was written, Java asks for a step soon).
fn decide(json: &str) -> String {
    let Ok(posted) = serde_json::from_str::<Posted>(json) else { return "null".into() };
    let config = load_config();
    let choices = Choices::load(&Choices::default_path());
    let kind = appnotes::classify(&posted, &choices, &sioul_core::words::Words::of(&config));
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let gate = gate();
    let (who, admitted) = match &kind {
        Kind::People(talk) => who_of(&talk.sender, &Senders::load(&config), &People::load(&People::default_path()), &config),
        _ => (None, false),
    };
    // A text from a number nobody knows, from a root ARCEP keeps for machines: an automaton's, gathered
    // with them, as the call screening sends their calls to voicemail (`screening`, unless set otherwise);
    // said so in its why and its line (review of part 2, F11).
    let machine = matches!(&kind, Kind::People(talk) if talk.sms && who == Some(Who::Stranger) && machine_number(&talk.sender, &config, &now));
    let kind = if machine { Kind::Automaton { site: String::new() } } else { kind };
    let (who, admitted) = if matches!(kind, Kind::People(_)) { (who, admitted) } else { (None, false) };
    let area = area_of(&posted, &kind, &choices, &config);
    let live = live(&config, &now);
    let gathered = config.reminders.gathered_times();
    // Today's slots of time for you, as the window wrote them; where they begin and end.
    let slots = crate::hours::slots();
    let slot = |at: &Zoned| slots.at(at);
    let also = slots.edges();
    // What reaches you when (Settings ▸ What reaches you); Free time's "Nothing at all".
    let attention = sioul_core::attention::Attention::of(&config);
    let nothing = sioul_core::pause::nothing_now(&sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path()), &config.free_time);
    // Its own rows in the matrix, when set: its conversation's, then its app's.
    let conversation = match &kind {
        Kind::People(talk) => talk.conversation.as_ref().map(|c| c.key.clone()).unwrap_or_default(),
        _ => String::new(),
    };
    let sources = sioul_core::attention::source_rows(&posted.package, &conversation);
    let ask = Ask { now: &now, clock: &live, attention: &attention, gathered: &gathered, area, gate: gate.map(|g| Gate { until: g.until }), slot_at: &slot, nothing, also: &also, sources: &sources };
    let hash = appnotes::key_hash(&posted.key);
    // A conversation you answered from a computer in the last twenty minutes, or a number
    // texted from one, by the SMS app: at once, but in a pause (`replies`).
    let answered = !live.clock.mode(&now).paused()
        && match &kind {
            Kind::People(talk) => {
                talk.conversation.as_ref().is_some_and(|c| crate::replies::answered_lately(&c.key))
                    || ((talk.sms || posted.sms_app) && matches!(&talk.sender, Sender::Known { numbers, .. } if crate::replies::texted_lately(&numbers.iter().map(|n| sioul_core::phones::key(n, sioul_core::reach::region(&config))).collect::<Vec<_>>())))
            }
            _ => false,
        };
    let path = Ledger::default_path();
    let (decision, released, returning) = sioul_core::filelock::with_lock(&path, || {
        let mut ledger = Ledger::load(&path);
        let returning = ledger.returning(&hash, stamp);
        let mut decision = appnotes::decide(&kind, who, admitted, &choices, &ask);
        if answered {
            decision = decision.answered();
        }
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
        // Back at its time and let come: one released, said in the log with what is still held.
        let mut released = None;
        if returning && decision.until.is_none() {
            let kind = ledger.held.get(&hash).map_or("other apps", kind_of);
            ledger.came_back(&hash, stamp);
            let mut counted = vec![Counted { kind, until: None, why: decision.why }];
            counted.extend(ledger.held.values().filter(|h| h.until > stamp).map(|h| Counted::held(h, h.until)));
            released = Some(counted);
        }
        ledger.note(&posted, &kind, who, admitted, area, &decision, stamp);
        if machine && decision.why == Why::Gathered
            && let Some(app) = ledger.apps.get_mut(&posted.package)
        {
            app.why = MACHINE.to_string();
        }
        ledger.forget_old(stamp, &choices);
        if let Err(e) = ledger.save(&path) {
            eprintln!("Notes: {e}");
        }
        (decision, released, returning)
    });
    // The phone's messages on your computers: its lines written when its app sends them (`phonemsgs`);
    // a reply from a computer handed lately, looked for in it as shown (`replies`).
    let shared = crate::phonemsgs::note(json, &posted, &kind, who, admitted, &decision) | crate::replies::heard(json, &posted);
    let hold = decision.until.map_or(0, |until| (until - stamp).max(1) * 1000);
    let why = if machine && decision.why == Why::Gathered { MACHINE } else { decision.why.id() };
    let mut answer = json!({ "hold": hold, "why": why, "shared": shared });
    if let Some(counted) = released {
        let columns = column_words(&columns_now(&live, &now, slot(&now), gate.is_some()), nothing && live.clock.mode(&now).free(), gate.as_ref());
        answer["line"] = json!(log_line("release", &counted, &columns.iter().map(String::as_str).collect::<Vec<_>>(), &now));
    } else if decision.until.is_some() {
        // Held: a line of its own, with what decided it (`appnotes::cause`), a machine's number said.
        let cause = appnotes::cause(&posted.package, &kind, who, admitted, &choices, &ask).map(|mut cause| {
            if machine && cause.gathered == Some(Gathering::Automaton) {
                cause.gathered = Some(Gathering::Machine);
            }
            cause
        });
        answer["line"] = json!(hold_line(if returning { "held again" } else { "held" }, kind_word(&kind), &decision, cause.as_ref(), gate.as_ref(), &now));
    }
    answer.to_string()
}

/// What Android holds for Sioul now, worked out again (`AppNotes.review`):
/// {"snooze": [{"key", "ms"}], "line"}, those whose time moved by more than a
/// minute (1 s: it may come now), and the line the listener writes in the
/// phone's log (`log_line`). Marks of notifications Android no longer holds
/// (their app took them away, or they came back unseen) are let go.
fn review(json: &str) -> String {
    let asked: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let keys: Vec<String> = asked["snoozed"].as_array().map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
    // Why the review runs, as Java says it ("files", "connected"): a word of its own, never a name.
    let reason: String = asked["reason"].as_str().unwrap_or_default().chars().filter(char::is_ascii_lowercase).take(16).collect();
    let config = load_config();
    let choices = Choices::load(&Choices::default_path());
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let gate = gate();
    let live = live(&config, &now);
    let gathered = config.reminders.gathered_times();
    let slots = crate::hours::slots();
    let slot = |at: &Zoned| slots.at(at);
    let also = slots.edges();
    let attention = sioul_core::attention::Attention::of(&config);
    let nothing = sioul_core::pause::nothing_now(&sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path()), &config.free_time);
    // A conversation answered from a computer in the last twenty minutes: what Sioul holds of it comes now, but in a pause (`replies`).
    let paused = live.clock.mode(&now).paused();
    let path = Ledger::default_path();
    let (again, counted) = sioul_core::filelock::with_lock(&path, || {
        let mut ledger = Ledger::load(&path);
        let present: std::collections::BTreeSet<String> = keys.iter().map(|k| appnotes::key_hash(k)).collect();
        ledger.held.retain(|hash, held| present.contains(hash) || stamp - held.since < 60);
        let mut again = Vec::new();
        let mut counted = Vec::new();
        for key in &keys {
            let hash = appnotes::key_hash(key);
            let Some(held) = ledger.held.get(&hash).cloned() else { continue };
            let sources = sioul_core::attention::source_rows(&held.app, &held.conversation);
            let ask = Ask { now: &now, clock: &live, attention: &attention, gathered: &gathered, area: held.area, gate: gate.map(|g| Gate { until: g.until }), slot_at: &slot, nothing, also: &also, sources: &sources };
            let mut decided = appnotes::again(&held, &choices, &ask);
            if !paused && crate::replies::answered_lately(&held.conversation) {
                decided = decided.answered();
            }
            let until = decided.until.unwrap_or(stamp);
            counted.push(Counted { why: decided.why, ..Counted::held(&held, until) });
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
        (again, counted)
    });
    let event = if reason.is_empty() { "review".to_string() } else { format!("review ({reason})") };
    let columns = column_words(&columns_now(&live, &now, slot(&now), gate.is_some()), nothing && live.clock.mode(&now).free(), gate.as_ref());
    let line = log_line(&event, &counted, &columns.iter().map(String::as_str).collect::<Vec<_>>(), &now);
    json!({ "snooze": again, "line": line }).to_string()
}

// ---------------------------------------------------------------- the log

/// A notification Sioul holds, as the listener's log line counts it
/// (`log_line`): the kind of its channel, until when it waits (none, or a
/// time past: it comes now), and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counted {
    kind: &'static str,
    until: Option<i64>,
    why: Why,
}

impl Counted {
    /// As the ledger keeps it, waiting until `until`: held for good (the
    /// blocked, a conversation set to never) or until its time.
    fn held(held: &appnotes::Held, until: i64) -> Counted {
        Counted { kind: kind_of(held), until: Some(until), why: if held.kind == "never" { Why::Never } else { Why::Waiting } }
    }
}

/// The kind of channel a held notification came on, as the log says it:
/// calls, messages, mail, or other apps (an automaton's, an app set to come
/// at once); never its app nor who wrote.
fn kind_of(held: &appnotes::Held) -> &'static str {
    match held.via.as_str() {
        "calls" => "calls",
        "messages" => "messages",
        "mail" => "mail",
        _ => "other apps",
    }
}

/// What reaches you now, as the matrix reads it: the time's column (or two,
/// work and admin at once), then its layers, a slot of time for you and
/// do-not-disturb (its switch or a focus session).
fn columns_now(live: &Live, now: &Zoned, slot: bool, dnd: bool) -> Vec<Column> {
    let moment = sioul_core::attention::Now::of(&live.clock.mode(now)).layers(slot, dnd);
    let mut columns = moment.times.clone();
    if moment.slot {
        columns.push(Column::Slot);
    }
    if moment.dnd {
        columns.push(Column::Dnd);
    }
    columns
}

/// The matrix's columns as the log says them: their ids ("leisure", "slot"),
/// Free time's "Nothing at all" when it holds ("free (nothing at all)"), and
/// do-not-disturb with what holds it ("dnd (focus session)", "dnd (switch)").
fn column_words(columns: &[Column], nothing: bool, gated: Option<&Gated>) -> Vec<String> {
    columns
        .iter()
        .map(|column| match column {
            Column::Free if nothing => "free (nothing at all)".to_string(),
            Column::Dnd => match gated {
                Some(Gated { focus: true, switch: true, .. }) => "dnd (switch and focus session)".to_string(),
                Some(Gated { focus: true, .. }) => "dnd (focus session)".to_string(),
                Some(Gated { switch: true, .. }) => "dnd (switch)".to_string(),
                _ => column.id().to_string(),
            },
            _ => column.id().to_string(),
        })
        .collect()
}

/// A time as the log says it: "22:00" today, "tomorrow 07:00", else "2026-10-20 09:00".
fn at_words(at: i64, now: &Zoned) -> String {
    match jiff::Timestamp::from_second(at).map(|t| t.to_zoned(now.time_zone().clone())) {
        Err(_) => "an unknown time".to_string(),
        Ok(at) if at.date() == now.date() => at.strftime("%H:%M").to_string(),
        Ok(at) if now.date().tomorrow().is_ok_and(|tomorrow| tomorrow == at.date()) => at.strftime("tomorrow %H:%M").to_string(),
        Ok(at) => at.strftime("%Y-%m-%d %H:%M").to_string(),
    }
}

/// The kind of channel a notification came on, as the log says it (`kind_of`).
fn kind_word(kind: &Kind) -> &'static str {
    match kind {
        Kind::People(talk) => match talk.via {
            Channel::Calls => "calls",
            Channel::Messages => "messages",
            Channel::Mail => "mail",
        },
        _ => "other apps",
    }
}

/// The listener's line at a hold (docs/android.md, "In the phone's log"):
/// the kind of its channel, until when it waits, or for good, and what
/// decided it (`appnotes::cause`): its conversation's or its app's own row,
/// a gathering and what gathers it, what it is for, the columns of the
/// matrix that hold it now (do-not-disturb with what holds it). "Notes:
/// held: messages 1 until 10:50; why: dnd (focus session)." Held again on
/// its return at its time: "held again". Never a name, a number, an app nor
/// who wrote.
fn hold_line(event: &str, kind: &str, decision: &Decision, cause: Option<&Cause>, gated: Option<&Gated>, now: &Zoned) -> String {
    let until = match decision.until {
        _ if decision.why == Why::Never => "for good".to_string(),
        Some(until) => format!("until {}", at_words(until, now)),
        None => "until now".to_string(),
    };
    let mut why: Vec<String> = Vec::new();
    if let Some(cause) = cause {
        if cause.for_good {
            why.push("never".into());
        }
        match cause.own {
            Some(Own::Conversation) => why.push("its conversation's row".into()),
            Some(Own::App) => why.push("its app's row".into()),
            None => {}
        }
        match cause.gathered {
            Some(Gathering::Automaton) => why.push("gathered".into()),
            Some(Gathering::App) => why.push("gathered (its app)".into()),
            Some(Gathering::Conversation) => why.push("gathered (its conversation)".into()),
            Some(Gathering::Named) => why.push("gathered (a sender named only)".into()),
            Some(Gathering::Machine) => why.push("gathered (a number kept for machines)".into()),
            None => {}
        }
        if let Some(area) = cause.area {
            why.push(format!("area ({})", area.id()));
        }
        if !cause.columns.is_empty() {
            why.push(column_words(&cause.columns, cause.nothing, gated).join(" + "));
        }
    }
    let why = if why.is_empty() { "unknown".to_string() } else { why.join(", ") };
    format!("Notes: {event}: {kind} 1 {until}; why: {why}.")
}

/// The listener's line in the phone's log (`adb logcat -s sioul`), at each
/// review of what is held and at each release (docs/android.md,
/// "Notifications from other apps"): how many notifications are held, and how
/// many come now, by the kind of their channel; why, the matrix's column now
/// and its layers ("leisure + dnd (switch)"), or Always through when that let
/// one come; when the next one comes back. Never a name, a number, an app nor
/// a word of a message: kinds, counts and times only. At each hold, a line of
/// its own (`hold_line`).
fn log_line(event: &str, counted: &[Counted], columns: &[&str], now: &Zoned) -> String {
    let stamp = now.timestamp().as_second();
    let (released, held): (Vec<&Counted>, Vec<&Counted>) = counted.iter().partition(|c| c.until.is_none_or(|until| until <= stamp));
    let by_kind = |items: &[&Counted]| {
        let kinds: Vec<String> = ["calls", "messages", "mail", "other apps"].iter().map(|kind| (kind, items.iter().filter(|c| c.kind == *kind).count())).filter(|(_, n)| *n > 0).map(|(kind, n)| format!("{kind} {n}")).collect();
        if kinds.is_empty() { items.len().to_string() } else { format!("{} ({})", items.len(), kinds.join(", ")) }
    };
    let why = if released.iter().any(|c| c.why == Why::Always) {
        "Always through".to_string()
    } else if released.iter().any(|c| c.why == Why::Answered) {
        "answered from a computer".to_string()
    } else if columns.is_empty() {
        "unknown".to_string()
    } else {
        columns.join(" + ")
    };
    // Held for good (the blocked, a conversation set to never) comes back at no time of the matrix's.
    let next = held.iter().filter(|c| c.why != Why::Never).filter_map(|c| c.until).min().filter(|at| jiff::Timestamp::from_second(*at).is_ok()).map_or_else(|| "none".to_string(), |at| at_words(at, now));
    format!("Notes: {event}: held {}; released {}; why: {why}; next release {next}.", by_kind(&held), by_kind(&released))
}

/// `decide`, for Java (android/main.cpp); a panic answers "null", and the
/// notification comes as it was sent. Given back to `sioul_string_free`.
///
/// # Safety
/// `json` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_appnotes_decide(json: *const c_char) -> *mut c_char {
    // SAFETY: `json` is null or a C string valid for the call, as this function's contract says.
    let json = unsafe { crate::alarms::key_of(json) };
    let answer = std::panic::catch_unwind(|| decide(&json)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

/// `review`, for Java; "null" when it fails, and what is held keeps its time.
///
/// # Safety
/// `json` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_appnotes_review(json: *const c_char) -> *mut c_char {
    // SAFETY: `json` is null or a C string valid for the call, as this function's contract says.
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

/// The table of other apps by time (What reaches you ▸ Exceptions): each app
/// seen lately a row of the matrix, by name, and the apps' rows set that this
/// device has not seen (a computer's, set on the phone: by the name written
/// with them); then the conversations chosen for (a row or a way through)
/// and the ten seen last ("Title · App"), never a wall of them. Whether
/// some were left out. `AttentionGrid.qml` draws it.
fn by_time(attention: &sioul_core::attention::Attention, ledger: &Ledger, choices: &Choices) -> (Value, bool) {
    use sioul_core::attention::{APP_ROW, CONVERSATION_ROW, SourceLine};
    let tr = tr();
    let apps_group = tr.text("attention-source-group-apps", None);
    let talks_group = tr.text("attention-source-group-conversations", None);
    let mut apps: Vec<SourceLine> = ledger.apps.iter().map(|(package, seen)| SourceLine { id: format!("{APP_ROW}{package}"), label: seen.label.clone(), group: apps_group.clone(), group_id: "app-rows".into() }).collect();
    // Set elsewhere (on the phone, for a computer), or no longer seen here: by the name written with it.
    for id in attention.sources().keys().filter(|id| id.starts_with(APP_ROW)) {
        if !apps.iter().any(|a| &a.id == id) {
            let name = attention.source_name(id);
            let label = if name.is_empty() { id[APP_ROW.len()..].to_string() } else { name.to_string() };
            apps.push(SourceLine { id: id.clone(), label, group: apps_group.clone(), group_id: "app-rows".into() });
        }
    }
    apps.sort_by_cached_key(|a| sioul_core::text::fold(&a.label).into_iter().collect::<String>());
    let mut talks: Vec<(&String, &appnotes::SeenTalk)> = ledger.conversations.iter().collect();
    talks.sort_by_key(|(_, t)| -t.seen);
    let total = talks.len();
    let chosen = |key: &str| choices.conversation.contains_key(key) || attention.source(&format!("{CONVERSATION_ROW}{key}")).is_some();
    let talks: Vec<(&String, &appnotes::SeenTalk)> = talks.into_iter().enumerate().filter(|(i, (key, _))| *i < 10 || chosen(key)).map(|(_, t)| t).collect();
    let more = talks.len() < total;
    let mut lines = apps;
    lines.extend(talks.into_iter().map(|(key, seen)| SourceLine { id: format!("{CONVERSATION_ROW}{key}"), label: format!("{} · {}", seen.title, seen.label), group: talks_group.clone(), group_id: "conversation-rows".into() }));
    (serde_json::to_value(sioul_core::attention::source_grid(attention, tr, &lines)).unwrap_or(Value::Null), more)
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
    let attention = sioul_core::attention::Attention::of(&config);
    let (grid, more) = by_time(&attention, &ledger, &choices);
    let mut apps: Vec<(&String, &appnotes::SeenApp)> = ledger.apps.iter().collect();
    apps.sort_by_cached_key(|(_, a)| sioul_core::text::fold(&a.label).into_iter().collect::<String>());
    let apps: Vec<Value> = apps
        .into_iter()
        .map(|(package, seen)| {
            let chosen = choices.app(package);
            let why = appnotes::why_text(tr, &seen.why, seen.until, &now);
            json!({
                "package": package,
                // Its own row of the matrix, in one sentence: "At once: Work; held: Leisure…".
                "times": sioul_core::attention::source_sentence(&attention, tr, &format!("{}{package}", sioul_core::attention::APP_ROW)),
                "kind": row(&format!("app.{package}.kind"), "choice", &seen.label, &why, json!(chosen.kind.id()), kinds.clone()),
                "area": row(&format!("app.{package}.area"), "areas", &text("appnotes-area"), "", json!(chosen.area.map(Area::id).unwrap_or_default()), Vec::new()),
            })
        })
        .collect();
    let throughs: Vec<(Value, String)> = Through::ALL.iter().map(|t| (json!(t.id()), text(&format!("appnotes-through-{}", t.id())))).collect();
    let mut talks: Vec<(&String, &appnotes::SeenTalk)> = ledger.conversations.iter().collect();
    talks.sort_by_key(|(_, t)| -t.seen);
    // What Android says of each: marked Priority there (it rings through Sioul's modes), its page.
    let android_says = if android { java("conversations", "{}") } else { Value::Null };
    let conversations: Vec<Value> = talks
        .into_iter()
        .map(|(key, seen)| {
            let label = format!("{} · {}", seen.title, seen.label);
            let help = if seen.group { text("appnotes-group") } else { String::new() };
            let mut said = row(&format!("conversation.{key}"), "choice", &label, &help, json!(choices.through(key).id()), throughs.clone());
            said["talk"] = json!(key);
            said["important"] = json!(android_says["conversations"][key.as_str()]["important"] == true);
            said
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
        "grid": grid,
        "more": more,
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
        // Android's pages; a conversation's by its key (`talk`), to mark it Priority there.
        "open-access" | "open-info" | "open-app" | "open-channel" | "open-conversation" => {
            java(verb, json);
        }
        "contacts" => crate::steps::ask_contacts(),
        // A row of the table by time: its words whole ("attention.app.<package>", nine words), an app's name with them.
        "row" => {
            let key = asked["key"].as_str().unwrap_or_default();
            let mut words: Vec<String> = asked["words"].as_array().map(|w| w.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default();
            if let Some(package) = key.strip_prefix("attention.").and_then(|id| id.strip_prefix(sioul_core::attention::APP_ROW))
                && let Some(seen) = Ledger::load(&Ledger::default_path()).apps.get(package)
            {
                words.push(format!("{}{}", sioul_core::attention::NAME, seen.label));
            }
            // What the row showed: only the cells changed from it are set, over the row as it is now.
            let shown: Option<Vec<String>> = asked["shown"].as_array().map(|w| w.iter().filter_map(|w| w.as_str().map(str::to_string)).collect());
            let shown = shown.map(|mut shown| {
                shown.extend(words.iter().filter(|w| w.starts_with(sioul_core::attention::NAME)).cloned());
                sioul_core::config::SettingValue::Texts(shown)
            });
            if let Err(e) = sioul_core::attention::apply_change(&crate::backend::config_path(), key, shown.as_ref(), &sioul_core::config::SettingValue::Texts(words)) {
                eprintln!("Notes: {e}");
            }
        }
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

    /// The table of other apps by time: each app seen, by name, and an app's row
    /// set elsewhere by the name written with it; the conversations chosen for
    /// and the ten seen last, never more; each row's cells as set or as usual.
    #[test]
    fn the_table_by_time() {
        let mut ledger = Ledger::default();
        ledger.apps.insert("com.discord".into(), appnotes::SeenApp { label: "Discord".into(), ..Default::default() });
        ledger.apps.insert("org.example.news".into(), appnotes::SeenApp { label: "Actualités".into(), ..Default::default() });
        for i in 0..14 {
            ledger.conversations.insert(format!("c{i:02}"), appnotes::SeenTalk { app: "com.discord".into(), label: "Discord".into(), title: format!("Salon {i}"), group: true, seen: i });
        }
        let mut choices = Choices::default();
        choices.conversation.insert("c00".into(), appnotes::TalkChoice { through: Through::Always, ..Default::default() });
        let mut attention = sioul_core::attention::Attention::usual();
        attention.set_source("app.com.discord", sioul_core::attention::Column::Work, sioul_core::attention::Level::Now).unwrap();
        attention.set_source("app.org.example.chat", sioul_core::attention::Column::Leisure, sioul_core::attention::Level::Later).unwrap();
        attention.name_source("app.org.example.chat", "Chat");
        attention.set_source("conversation.c01", sioul_core::attention::Column::Sleep, sioul_core::attention::Level::Later).unwrap();
        let (grid, more) = by_time(&attention, &ledger, &choices);
        let ids: Vec<&str> = grid["rows"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();
        let labels: Vec<&str> = grid["rows"].as_array().unwrap().iter().map(|r| r["label"].as_str().unwrap()).collect();
        assert_eq!(&ids[..3], ["app.org.example.news", "app.org.example.chat", "app.com.discord"], "by name: Actualités, Chat, Discord");
        assert_eq!(&labels[..3], ["Actualités", "Chat", "Discord"]);
        // The ten seen last, and the two chosen for among the older ones.
        let talks: Vec<&str> = ids.iter().filter(|id| id.starts_with("conversation.")).copied().collect();
        assert_eq!(talks.len(), 12);
        assert!(talks.contains(&"conversation.c00") && talks.contains(&"conversation.c01") && !talks.contains(&"conversation.c02") && talks[0] == "conversation.c13");
        assert!(more, "two conversations left out");
        let discord = &grid["rows"][2];
        assert_eq!(discord["cells"][0]["value"], "now");
        assert_eq!(discord["cells"][1]["value"], "as");
        assert_eq!(discord["choices"].as_array().unwrap().len(), 4);
        assert_eq!(grid["columns"].as_array().unwrap().len(), 9);
        // Nothing seen, nothing set: no table.
        let (none, more) = by_time(&sioul_core::attention::Attention::usual(), &Ledger::default(), &Choices::default());
        assert!(none["rows"].as_array().unwrap().is_empty() && !more);
    }

    /// The listener's line in the phone's log: how many are held and how many
    /// come now, by the kind of their channel, why (the matrix's column and
    /// layers, or Always through), and the next release; never a name, a
    /// number, an app nor a conversation, whatever the ledger holds.
    #[test]
    fn the_log_line_says_kinds_counts_and_times_only() {
        let now: Zoned = "2026-10-11T21:30:00+02:00[Europe/Paris]".parse().unwrap();
        let stamp = now.timestamp().as_second();
        let held = |app: &str, label: &str, via: &str, who: &str, conversation: &str, kind: &str, until: i64| appnotes::Held { app: app.into(), label: label.into(), kind: kind.into(), via: via.into(), who: who.into(), conversation: conversation.into(), since: stamp - 3_600, until, ..Default::default() };
        let ledger = [
            held("org.example.chat", "Chat de Noa", "messages", "safe", "c0ffee12", "people", stamp + 30 * 60),
            held("org.example.chat", "Chat de Noa", "messages", "neutral", "c0ffee13", "people", stamp + 9 * 3_600 + 30 * 60),
            held("org.example.mail", "Courrier 01 99 00", "mail", "stranger", "", "people", stamp + 10 * 3_600),
            held("org.example.news", "Actualités", "", "", "", "gathered", stamp + 11 * 3_600),
            held("org.example.spam", "Bloqué", "messages", "blocked", "", "never", stamp + 7 * 86_400),
        ];
        let counted: Vec<Counted> = ledger.iter().map(|h| Counted::held(h, h.until)).collect();
        let review = log_line("review (files)", &counted, &["leisure", "dnd"], &now);
        assert_eq!(review, "Notes: review (files): held 5 (messages 3, mail 1, other apps 1); released 0; why: leisure + dnd; next release 22:00.");
        // One come back by Always through, the rest still held; the next release tomorrow (held for good aside).
        let mut released = vec![Counted { kind: "messages", until: None, why: Why::Always }];
        released.extend(counted.iter().skip(1).copied());
        let release = log_line("release", &released, &["sleep"], &now);
        assert_eq!(release, "Notes: release: held 4 (messages 2, mail 1, other apps 1); released 1 (messages 1); why: Always through; next release tomorrow 07:00.");
        // A review that lets one come at its time (a pause over): released now, the column why.
        let mut over = counted.clone();
        over[3].until = Some(stamp);
        assert!(log_line("review (files)", &over, &["leisure"], &now).contains("released 1 (other apps 1); why: leisure;"));
        // Nothing held: said so, with the columns now.
        assert_eq!(log_line("review (connected)", &[], &["work", "admin"], &now), "Notes: review (connected): held 0; released 0; why: work + admin; next release none.");
        // Never a name, a number, an app, a conversation nor who wrote.
        for line in [review, release] {
            for private in ["org.example", "Noa", "Courrier", "01 99 00", "Actualités", "Bloqué", "c0ffee", "safe", "neutral", "stranger", "blocked"] {
                assert!(!line.contains(private), "{line} says {private}");
            }
        }
        // Do-not-disturb with what holds it, Free time's "Nothing at all".
        let switch = Gated { until: None, focus: false, switch: true };
        let columns = column_words(&[Column::Free, Column::Dnd], true, Some(&switch));
        assert_eq!(columns, ["free (nothing at all)", "dnd (switch)"]);
        assert_eq!(column_words(&[Column::Leisure, Column::Dnd], true, None), ["leisure", "dnd"], "nothing at all says nothing outside Free time");
    }

    /// The listener's line at each hold (decision 2 of the overrides, 18
    /// October 2026): the kind of its channel, until when it waits or for
    /// good, and what decided it, never who: the proposal's case (a
    /// contact's text at work held by a focus session), the switch and both,
    /// a text named only, an automaton in the night, an app's and a
    /// conversation's own rows, what it is for, held for good, held again.
    /// Then a real hold, decided and said: no name, number, app, conversation
    /// nor list in it, whatever the notification said.
    #[test]
    fn the_hold_line_says_what_decided_it_and_names_no_one() {
        let now: Zoned = "2026-10-14T10:20:00+02:00[Europe/Paris]".parse().unwrap();
        let stamp = now.timestamp().as_second();
        let focus = Gated { until: Some(stamp + 30 * 60), focus: true, switch: false };
        let both = Gated { until: None, focus: true, switch: true };
        let line = |event: &str, kind: &str, until: Option<i64>, why: Why, cause: Cause, gated: Option<&Gated>| hold_line(event, kind, &Decision { until, why }, Some(&cause), gated, &now);
        let columns = |columns: &[Column]| Cause { columns: columns.to_vec(), ..Cause::default() };
        let in_minutes = |m: i64| Some(stamp + m * 60);
        assert_eq!(line("held", "messages", in_minutes(30), Why::Waiting, columns(&[Column::Dnd]), Some(&focus)), "Notes: held: messages 1 until 10:50; why: dnd (focus session).");
        assert_eq!(line("held", "calls", in_minutes(30), Why::Waiting, columns(&[Column::Dnd]), Some(&both)), "Notes: held: calls 1 until 10:50; why: dnd (switch and focus session).");
        assert_eq!(line("held", "messages", in_minutes(100), Why::Gathered, Cause { gathered: Some(Gathering::Named), ..Cause::default() }, None), "Notes: held: messages 1 until 12:00; why: gathered (a sender named only).");
        assert_eq!(line("held", "messages", in_minutes(100), Why::Gathered, Cause { gathered: Some(Gathering::Machine), ..Cause::default() }, None), "Notes: held: messages 1 until 12:00; why: gathered (a number kept for machines).");
        assert_eq!(line("held", "other apps", Some(stamp + 22 * 3600 + 40 * 60), Why::Gathered, Cause { gathered: Some(Gathering::Automaton), columns: vec![Column::Sleep], ..Cause::default() }, None), "Notes: held: other apps 1 until tomorrow 09:00; why: gathered, sleep.");
        assert_eq!(line("held", "messages", in_minutes(400), Why::Times, Cause { own: Some(Own::App), columns: vec![Column::Work, Column::Admin], ..Cause::default() }, None), "Notes: held: messages 1 until 17:00; why: its app's row, work + admin.");
        assert_eq!(line("held", "messages", in_minutes(400), Why::Times, Cause { own: Some(Own::Conversation), gathered: None, columns: vec![Column::Work], ..Cause::default() }, None), "Notes: held: messages 1 until 17:00; why: its conversation's row, work.");
        assert_eq!(line("held", "messages", in_minutes(100), Why::Gathered, Cause { gathered: Some(Gathering::Conversation), ..Cause::default() }, None), "Notes: held: messages 1 until 12:00; why: gathered (its conversation).");
        assert_eq!(line("held", "other apps", in_minutes(100), Why::Gathered, Cause { gathered: Some(Gathering::App), ..Cause::default() }, None), "Notes: held: other apps 1 until 12:00; why: gathered (its app).");
        assert_eq!(line("held", "mail", in_minutes(400), Why::Waiting, Cause { area: Some(Area::LEISURE), ..Cause::default() }, None), "Notes: held: mail 1 until 17:00; why: area (leisure).");
        assert_eq!(line("held", "messages", Some(stamp + 7 * 86_400), Why::Never, Cause { for_good: true, ..Cause::default() }, None), "Notes: held: messages 1 for good; why: never.");
        assert_eq!(line("held again", "messages", Some(stamp + 3 * 86_400), Why::Waiting, columns(&[Column::Pause]), None), "Notes: held again: messages 1 until 2026-10-17 10:20; why: pause.");
        assert_eq!(hold_line("held", "messages", &Decision { until: in_minutes(5), why: Why::Waiting }, None, None, &now), "Notes: held: messages 1 until 10:25; why: unknown.");
        // A real one: a contact's text at work, held by a focus session, decided and said as the listener does.
        struct Work;
        impl appnotes::TimeSource for Work {
            fn mode(&self, _: &Zoned) -> sioul_core::quiet::Mode {
                let week = sioul_core::areas::Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
                sioul_core::quiet::Mode { quiet: false, time: sioul_core::areas::Time::Work, week, reason: sioul_core::quiet::Reason::Working, until: None, back: None, label: String::new() }
            }
            fn starts(&self, _: &Zoned, _: &Zoned) -> Vec<Zoned> {
                Vec::new()
            }
        }
        let posted: Posted = serde_json::from_str(r#"{"key":"0|org.example.chat|7|c0ffee12|10100","package":"org.example.chat","app":"Chat de Noa","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Noa Martin","shortcut":"c0ffee12","messages":[{"sender":{"name":"Noa Martin","uri":"tel:%2B33199001234"},"text":"Tu passes ce soir ?"}]}"#).unwrap();
        let choices = Choices::default();
        let kind = appnotes::classify(&posted, &choices, sioul_core::words::Words::builtin_ref());
        let attention = sioul_core::attention::Attention::usual();
        let gathered = vec!["09:00".to_string(), "12:00".into(), "15:00".into(), "18:00".into()];
        let never = |_: &Zoned| false;
        let sources = sioul_core::attention::source_rows(&posted.package, "");
        let ask = Ask { now: &now, clock: &Work, attention: &attention, gathered: &gathered, area: None, gate: Some(Gate { until: focus.until }), slot_at: &never, nothing: false, also: &[], sources: &sources };
        let decision = appnotes::decide(&kind, Some(Who::Neutral), false, &choices, &ask);
        assert_eq!(decision.until, focus.until, "held until the session's end");
        let cause = appnotes::cause(&posted.package, &kind, Some(Who::Neutral), false, &choices, &ask);
        let said = hold_line("held", kind_word(&kind), &decision, cause.as_ref(), Some(&focus), &now);
        assert_eq!(said, "Notes: held: messages 1 until 10:50; why: dnd (focus session).");
        for private in ["org.example", "Noa", "Martin", "01 99 00", "+33", "Chat", "c0ffee", "ce soir", "safe", "neutral", "stranger", "blocked", "restricted"] {
            assert!(!said.contains(private), "{said} says {private}");
        }
    }

    /// A text from a root ARCEP keeps for machines is an automaton's, while
    /// the call screening sends automated callers to voicemail: 01 63, a
    /// root for machines that ARCEP had attributed to no operator yet (so the
    /// number is nobody's); a fiction number is no machine's. The numbers'
    /// country set, as a French phone's language says it (with no locale, as
    /// on CI, it would be Britain's, where 01 63 is no machine's).
    #[test]
    fn a_machine_s_text_is_an_automaton_s() {
        let now: Zoned = "2026-10-20T10:00:00+02:00[Europe/Paris]".parse().unwrap();
        let config: Config = toml::from_str("[contacts]\nregion = \"FR\"\n").unwrap();
        let machine = Sender::Known { numbers: vec!["01 63 12 34 56".into()], emails: Vec::new(), contact: false };
        assert!(machine_number(&machine, &config, &now));
        assert!(!machine_number(&Sender::Known { numbers: vec!["01 99 00 12 34".into()], emails: Vec::new(), contact: false }, &config, &now));
        assert!(!machine_number(&Sender::Named { name: "AMELI".into(), book: true }, &config, &now));
        let as_unknown: Config = toml::from_str("[contacts]\nregion = \"FR\"\n[screening]\nautomated = \"unknown\"\n").unwrap();
        assert!(!machine_number(&machine, &as_unknown, &now), "set as any number nobody knows");
    }

    #[test]
    fn nothing_reaches_java_on_a_computer() {
        assert_eq!(java("access", "{}"), Value::Null);
        assert_eq!(access_here(), None, "a computer says nothing of a phone's access");
        assert_eq!(times_in_words(&["18:00".into(), "9:00".into(), "13:00".into()]), format!("09:00, 13:00 {} 18:00", tr().text("word-and", None)));
    }
}

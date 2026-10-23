// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The phone's messages on your computers, as this device writes and shows
//! them (docs/android.md, "Messages on your computers"; the rules are
//! `sioul_core::phonemsgs`).
//!
//! - **On the phone**, in the listener's process: after each notification
//!   decided (`appnotes::decide`), the lines it makes are written to this
//!   phone's own log while the part "Messages from your phone" is on here
//!   (`note`), and what it offers a reply from your computers to its live
//!   lines (`replies::offered`); Java then asks the background service for a
//!   step soon, which shares them. Rust writes the shared files; Java never does.
//! - **On every device**, before each exchange: its own lines taken out
//!   after their week, another's three days later (`before_exchange`).
//! - **On a computer's Porch**, "From your phone" (`view`), while the part
//!   is on there: each line once the phone let it through and while its
//!   sender may reach you; Seen on any device, gone from all (`act`).
//! - **A person's sheet**: their messages of the week (`history`).
//! - **The phone's choices**, Settings ▸ Other apps ▸ On your computers
//!   (`setup`, the "set" and "part" verbs of `call`).

use crate::backend::{load_config, tr};
use jiff::Zoned;
use serde_json::{Value, json};
use sioul_core::appnotes::{Decision, Kind, Ledger, Posted};
use sioul_core::calls;
use sioul_core::config::{Config, state_dir};
use sioul_core::everywhere::People;
use sioul_core::phonemsgs::{self as rules, Choices, Extra, Line, Send};
use sioul_core::porch::{By, Senders, Standing};
use sioul_core::reach::{Clock, Who};
use std::sync::Mutex;

/// This device's name in the sharing (share/here.toml), made once.
fn here_id() -> String {
    sioul_sync::share::Here::load(&state_dir()).id
}

/// Whether this device's listener writes lines: a phone. SIOUL_PHONE_MESSAGES
/// makes a computer's do it too, to look at them.
fn phone() -> bool {
    cfg!(target_os = "android") || std::env::var_os("SIOUL_PHONE_MESSAGES").is_some()
}

// ---------------------------------------------------------------- the phone's side

/// A notification decided (`appnotes::decide`, in the listener's process):
/// its lines written to this phone's own log, when the part is on here and
/// its app sends to your computers (`Choices`). `json` is what Java said
/// (`sioul_core::phonemsgs::Extra` reads what `Posted` passes over); `who`
/// and `listed` as the decision judged the sender. Whether a line was
/// written: Java asks for a step soon then.
pub(crate) fn note(json: &str, posted: &Posted, kind: &Kind, who: Option<Who>, listed: bool, decision: &Decision) -> bool {
    if !phone() {
        return false;
    }
    let path = Choices::default_path();
    let mut choices = Choices::load(&path);
    // The phone's SMS app noted, for the tab, when it changed.
    if posted.sms_app && choices.sms_app != posted.package {
        note_sms_app(&posted.package);
        choices.sms_app = posted.package.clone();
    }
    let config = load_config();
    let here = sioul_sync::share::Here::load(&state_dir());
    if here.id.is_empty() || here.folder.is_none() || !here.shares(rules::PART, &config) {
        return false;
    }
    let send = choices.send(&posted.package, posted.sms_app);
    if send == Send::Off {
        return false;
    }
    let extra = Extra::read(json);
    let words = sioul_core::words::Words::of(&config);
    let now_ms = jiff::Timestamp::now().as_millisecond();
    let notice = rules::Notice { posted, extra: &extra, kind, who, listed, decision, send, now_ms, region: sioul_core::reach::region(&config), words: &words };
    let lines = rules::lines_of(&notice);
    if lines.is_empty() {
        return false;
    }
    // The replies' ledger's mark, before any reply can answer these lines (`replies`).
    crate::replies::relayed(now_ms);
    // What it offers a reply from your computers, said when that changed (a Reply, a Mark as read),
    // as the phone's switch for its app says.
    let offered = crate::replies::offered(&here.id, posted, &lines, &extra, choices.answers(&posted.package), now_ms);
    match rules::carry_into(&calls::own_file(&rules::folder(), rules::LOG, &here.id), &lines, now_ms) {
        Ok(0) => offered,
        Ok(written) => {
            // Never what it says, nor who: how many, and the app.
            eprintln!("sioul: phone messages: {written} line(s) for your computers ({})", posted.package);
            true
        }
        Err(e) => {
            eprintln!("sioul: phone messages: {e}");
            offered
        }
    }
}

/// The phone's default SMS app written down, under the choices' lock: its
/// usual is to send its words, which the tab says.
fn note_sms_app(package: &str) {
    let path = Choices::default_path();
    let noted = sioul_core::filelock::with_lock(&path, || {
        let mut kept = Choices::load(&path);
        if kept.sms_app == package {
            return Ok(());
        }
        kept.sms_app = package.to_string();
        kept.save(&path)
    });
    if let Err(e) = noted {
        eprintln!("sioul: phone messages: {e}");
    }
}

/// When this device last took its old lines out (ms, its clock): once a day.
static TRIMMED: Mutex<i64> = Mutex::new(0);

/// Before an exchange, on every device that has the folder, once a day: its
/// own lines taken out after their week, another's three days later (a
/// phone gone for good never takes its own out).
pub(crate) fn before_exchange() {
    let root = rules::folder();
    if !root.is_dir() {
        return;
    }
    let now = jiff::Timestamp::now().as_millisecond();
    let due = TRIMMED.lock().map(|mut last| {
        let due = now - *last > 86_400_000;
        if due {
            *last = now;
        }
        due
    });
    if !due.unwrap_or(false) {
        return;
    }
    let here = here_id();
    if here.is_empty() {
        return;
    }
    if let Err(e) = rules::trim_own(&root, &here, now).and_then(|_| rules::trim_others(&root, &here, now)) {
        eprintln!("sioul: phone messages: {e}");
    }
}

// ---------------------------------------------------------------- the Porch

/// Who a number is now, as this device's lists and address books say;
/// none when they say nothing of it (the phone's judgement stands).
fn who_now(senders: &Senders, key: &str) -> Option<Who> {
    if key.is_empty() {
        return None;
    }
    let judged = senders.judge_number(key);
    (judged.by != By::Default).then_some(judged.who)
}

/// The part of the day a moment fell in, by this device's clock: a column of
/// the matrix ("work", "sleep"…).
fn column_at(clock: &Clock, at_ms: i64, now: &Zoned) -> String {
    let Ok(at) = jiff::Timestamp::from_millisecond(at_ms) else { return String::new() };
    let mode = clock.mode(&at.to_zoned(now.time_zone().clone()));
    sioul_core::attention::Now::of(&mode).times.first().map_or(String::new(), |c| c.id().to_string())
}

/// The Porch's "From your phone" (PhoneMessagesSection.qml): {lines, links:
/// {tel, sms}}; nothing on a phone (its notifications are there), nothing
/// while the part is off here.
pub(crate) fn view() -> String {
    // The notices of texts not read yet, whether or not the phone's notifications come here.
    let empty = || json!({ "lines": [], "texts": crate::texts::readable(), "notices": crate::texts::notices() }).to_string();
    let root = rules::folder();
    if cfg!(target_os = "android") || !root.join(rules::LOG).is_dir() || !crate::share::shares(rules::PART) {
        return empty();
    }
    let now = Zoned::now();
    let all = rules::read_logs(&root, now.timestamp().as_millisecond());
    if all.is_empty() {
        return empty();
    }
    let config = load_config();
    // A text the Texts page holds is one item (23 October 2026): its notice,
    // opening it there; the SMS app's notification of it is folded into it.
    let all: Vec<Line> = match crate::texts::folding() {
        Some(folds) => all.into_iter().filter(|l| !(l.kind == "text" && !l.group && folds(&l.key, l.at))).collect(),
        None => all,
    };
    let lines = porch_lines(&config, &all, &now);
    let (tel, sms) = crate::calls::link_apps();
    // texts: Text back opens the conversation in Sioul when texts are read on this computer.
    json!({ "lines": lines, "links": { "tel": tel, "sms": sms }, "texts": crate::texts::readable(), "notices": crate::texts::notices() }).to_string()
}

/// The phone's notifications of a conversation's texts seen, on every
/// device (`keys`: its people's numbers' keys): its thread opened, or
/// deleted, in Texts. The last week's, which is all a phone keeps.
pub(crate) fn seen_for(keys: &[String]) {
    let root = rules::folder();
    if keys.is_empty() || !root.join(rules::LOG).is_dir() {
        return;
    }
    let now = Zoned::now().timestamp().as_millisecond();
    let seen = calls::read_seen(&root);
    let ids: Vec<String> = rules::read_logs(&root, now).into_iter().filter(|l| l.kind == "text" && keys.contains(&l.key) && !seen.seen.contains(&l.id)).map(|l| l.id).collect();
    if !ids.is_empty()
        && let Err(e) = calls::mark_seen(&root, &here_id(), &ids, now)
    {
        eprintln!("sioul: phone messages: {e}");
    }
}

/// A phone's text's sender's own rows, as the phone read them (decision 16):
/// by their number; by their name, as the phone's address book names people,
/// when the app gave no number (review of part 6, F8).
fn line_persons(senders: &Senders, line: &Line) -> Vec<String> {
    if !line.key.is_empty() {
        senders.person_rows(std::slice::from_ref(&line.key), &[])
    } else if !line.name.trim().is_empty() && !line.group {
        senders.person_rows_named(&line.name)
    } else {
        Vec::new()
    }
}

fn porch_lines(config: &Config, all: &[Line], now: &Zoned) -> Vec<rules::PorchLine> {
    let here = here_id();
    let senders = Senders::load(config);
    let attention = sioul_core::attention::Attention::of(config);
    let moment = crate::hours::attention_now();
    let people = People::load(&People::default_path());
    let clock = Clock::load(config, now);
    let region = senders.region();
    // A conversation this computer answered in the last twenty minutes shows at once, as its phone
    // lets it through, but in a pause (`replies`).
    let answered = if moment.times.contains(&sioul_core::attention::Column::Pause) { sioul_core::replies::Answered::default() } else { crate::replies::answered_here(all, now) };
    let shows = |line: &Line| {
        let always = line.always || (!line.key.is_empty() && people.admits_number(&line.key, region));
        let who = who_now(&senders, &line.key);
        let persons = line_persons(&senders, line);
        rules::reaches_now(&attention, line, who, always, &persons, &moment) || (who != Some(Who::Blocked) && answered.holds(line))
    };
    let name_of = |key: &str| {
        let judged = senders.judge_number(key);
        (!judged.card.trim().is_empty()).then_some(judged.card)
    };
    let phones = crate::calls::phone_names();
    let phone = |device: &str| calls::phone_words(tr(), device, &here, &phones, cfg!(target_os = "android"));
    let column = |at: i64| column_at(&clock, at, now);
    let lister = rules::Lister { now, tr: tr(), region, name_of: &name_of, shows: &shows, phone: &phone, column: &column };
    let mut lines = rules::lines(all, &calls::read_seen(&rules::folder()), &lister);
    // What each line offers a reply from this computer (`replies`).
    crate::replies::fill(&mut lines, all, now);
    lines
}

/// A person's messages of the week (`numbers`: theirs, as written), for
/// their sheet (`reaches::person`): a sentence each, newest first; none
/// where no phone shares them, or while the part is off here.
pub(crate) fn history(numbers: &[String]) -> Vec<String> {
    let root = rules::folder();
    if numbers.is_empty() || !root.join(rules::LOG).is_dir() || !crate::share::shares(rules::PART) {
        return Vec::new();
    }
    let config = load_config();
    let region = sioul_core::reach::region(&config);
    let keys: std::collections::BTreeSet<String> = numbers.iter().map(|n| sioul_core::phones::key(n.trim().trim_start_matches(sioul_core::porch::TEL), region)).filter(|k| sioul_core::phones::is_whole(k)).collect();
    if keys.is_empty() {
        return Vec::new();
    }
    let now = Zoned::now();
    let all = rules::read_logs(&root, now.timestamp().as_millisecond());
    let (none, every, nowhere, unknown) = (|_: &str| None, |_: &Line| true, |_: &str| String::new(), |_: i64| String::new());
    let lister = rules::Lister { now: &now, tr: tr(), region, name_of: &none, shows: &every, phone: &nowhere, column: &unknown };
    rules::history(&all, &keys, &lister)
}

// ---------------------------------------------------------------- the phone's choices

/// The phone's tab, Settings ▸ This phone ▸ On your computers
/// (PhoneMessagesSetup.qml): {android, sharing (a sharing folder is set),
/// part, apps}, each a row as `SettingRow.qml` shows it ({key, kind, label,
/// help, value, choices}): the part's switch here, then each app seen
/// lately, the SMS app first, then by name.
fn setup() -> Value {
    let config = load_config();
    let here = sioul_sync::share::Here::load(&state_dir());
    // The phone's SMS app, as Android says it now (it may have changed since the listener last saw one).
    if cfg!(target_os = "android")
        && let Some(sms) = crate::steps::java("sms-app", "{}").as_str().filter(|p| !p.is_empty())
    {
        note_sms_app(sms);
    }
    let choices = Choices::load(&Choices::default_path());
    let ledger = Ledger::load(&Ledger::default_path());
    let tr = tr();
    let text = |id: &str| tr.text(id, None);
    let mut apps: Vec<(String, String)> = ledger.apps.iter().map(|(package, seen)| (package.clone(), seen.label.clone())).collect();
    for (package, chosen) in &choices.app {
        if !apps.iter().any(|(p, _)| p == package) {
            apps.push((package.clone(), if chosen.label.is_empty() { package.clone() } else { chosen.label.clone() }));
        }
    }
    apps.retain(|(package, _)| package != sioul_core::appnotes::OWN);
    apps.sort_by_cached_key(|(package, label)| (*package != choices.sms_app, sioul_core::text::fold(label).into_iter().collect::<String>()));
    let offered: Vec<Value> = [Send::Off, Send::Who, Send::Words].iter().map(|s| json!({ "value": s.id(), "label": text(&format!("phonemsgs-send-{}", s.id())) })).collect();
    // Each app's choice, and under each that sends, "Answer from your computers" (on unless switched off).
    let apps: Vec<Value> = apps
        .into_iter()
        .flat_map(|(package, label)| {
            let help = if package == choices.sms_app { text("phonemsgs-setup-sms") } else { String::new() };
            let send = choices.send(&package, false);
            let mut rows = vec![json!({ "key": format!("app.{package}"), "kind": "choice", "label": if label.is_empty() { package.clone() } else { label }, "help": help, "value": send.id(), "choices": offered })];
            if send != Send::Off {
                rows.push(json!({ "key": format!("answers.{package}"), "kind": "bool", "label": text("phonemsgs-setup-answers"), "help": text("phonemsgs-setup-answers-help"), "value": choices.answers(&package), "choices": [] }));
            }
            rows
        })
        .collect();
    json!({
        "android": cfg!(target_os = "android"),
        "sharing": here.folder.is_some(),
        "part": { "key": "part", "kind": "bool", "label": text("phonemsgs-setup-part"), "help": text("share-part-phone-messages-carries"), "value": here.shares(rules::PART, &config), "choices": [] },
        "apps": apps,
    })
}

/// An app's choice written (`value`: "off", "who", "words"): what its usual
/// says is kept as usual, so that the SMS app changed stays followed.
fn set(package: &str, value: &str) -> Result<(), String> {
    let Some(send) = Send::read(value) else { return Ok(()) };
    let path = Choices::default_path();
    sioul_core::filelock::with_lock(&path, || {
        let mut choices = Choices::load(&path);
        let sms = package == choices.sms_app;
        let send = if Send::Usual.resolved(sms) == send { Send::Usual } else { send };
        let label = Ledger::load(&Ledger::default_path()).apps.get(package).map(|a| a.label.clone()).or_else(|| choices.app.get(package).map(|a| a.label.clone())).unwrap_or_default();
        choices.set(package, send, &label);
        choices.save(&path)
    })
}

/// "Answer from your computers" switched for an app, under the choices' lock.
fn set_answers(package: &str, on: bool) -> Result<(), String> {
    let path = Choices::default_path();
    sioul_core::filelock::with_lock(&path, || {
        let mut choices = Choices::load(&path);
        choices.set_answers(package, on);
        choices.save(&path)
    })
}

/// What the window asks (`Sioul::phone_messages`): "view" (the Porch's
/// section), "seen" {ids}, "unseen" {at} (within its ten seconds), "block"
/// {number}, "setup" (the phone's tab), "set" {key: `app.<package>` or `answers.<package>`, value}, "part" {on};
/// "reply" {phone, verb ("reply", "read"), notice, app, line, words, again}
/// (a reply from this computer, `replies::ask`: {key, said}), "take-back" {key}.
/// Answers its JSON, and whether what travels changed (to share now).
pub(crate) fn call(verb: &str, json: &str) -> (String, bool) {
    let asked: Value = serde_json::from_str(json).unwrap_or_default();
    let text = |key: &str| asked[key].as_str().unwrap_or_default().trim().to_string();
    let answer = |said: String, shared: bool, at: i64| (json!({ "said": said, "shared": shared, "at": at }).to_string(), shared);
    match verb {
        "view" => (view(), false),
        // In this device's own seen log, which the sharing carries: gone from every device's Porch.
        "seen" => {
            let ids: Vec<String> = asked["ids"].as_array().map(|a| a.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()).unwrap_or_default();
            match calls::mark_seen(&rules::folder(), &here_id(), &ids, jiff::Timestamp::now().as_millisecond()) {
                Ok(at) => answer(tr().text("phonemsgs-seen-said", None), true, at),
                Err(e) => answer(e, false, 0),
            }
        }
        "unseen" => match calls::unmark_seen(&rules::folder(), &here_id(), asked["at"].as_i64().unwrap_or(0)) {
            Ok(gone) => answer(String::new(), gone, 0),
            Err(e) => answer(e, false, 0),
        },
        // On your blocked list, as a number: nothing of theirs leaves the phone again.
        "block" => {
            let number = text("number");
            if number.is_empty() {
                return answer(String::new(), false, 0);
            }
            match sioul_core::porch::set_standing(&load_config(), &format!("{}{number}", sioul_core::porch::TEL), Standing::Blocked) {
                Ok(()) => answer(tr().text("phonemsgs-blocked-said", None), true, 0),
                Err(e) => answer(e, false, 0),
            }
        }
        "setup" => (setup().to_string(), false),
        // A reply from this computer to a phone's notification, or a Mark as read: a request the phone decides once.
        "reply" => match crate::replies::ask(&text("phone"), &text("verb"), &text("notice"), &text("app"), &text("line"), asked["words"].as_str().unwrap_or_default(), &text("again")) {
            Ok(key) => (json!({ "key": key, "said": "", "shared": true }).to_string(), true),
            Err(said) => answer(said, false, 0),
        },
        // Taken back while it waits for the phone.
        "take-back" => match crate::replies::take_back(&text("key")) {
            Ok(()) => answer(String::new(), true, 0),
            Err(e) => answer(e, false, 0),
        },
        "set" => {
            let key = text("key");
            let done = match key.strip_prefix("answers.") {
                Some(package) => set_answers(package, asked["value"] == true),
                None => set(key.strip_prefix("app.").unwrap_or(&key), &text("value")),
            };
            if let Err(e) = done {
                eprintln!("sioul: phone messages: {e}");
            }
            (setup().to_string(), false)
        }
        "part" => {
            let on = asked["on"].as_bool().unwrap_or(false);
            let problem = crate::share::set_part(rules::PART, on);
            let mut tab = setup();
            tab["said"] = json!(problem);
            (tab.to_string(), problem.is_empty())
        }
        _ => (json!({}).to_string(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review of part 6, F8: a phone's text's sender's own rows, by their
    /// number, else by their name as the phone's address book names people;
    /// a group's, none.
    #[test]
    fn a_text_reads_its_sender_s_rows_by_number_or_name() {
        use sioul_core::porch::Book;
        let card = sioul_core::contacts::Contact { uid: "noa-1".into(), key: "/books/noa-1.vcf".into(), name: "Noa".into(), phones: vec![sioul_core::contacts::Labeled { label: String::new(), value: "+33 1 99 00 12 34".into() }], ..Default::default() };
        let senders = Senders { contacts: std::sync::Arc::new(Book::of(&[card], None)), ..Senders::default() };
        let line = |key: &str, name: &str, group: bool| Line { key: key.into(), name: name.into(), group, ..Line::default() };
        assert_eq!(line_persons(&senders, &line("+33199001234", "", false)), ["person.contact:noa-1", "person.tel:+33199001234"]);
        assert_eq!(line_persons(&senders, &line("", "Noa", false)), ["person.contact:noa-1"], "by name, the app giving no number");
        assert!(line_persons(&senders, &line("", "Noa", true)).is_empty(), "a group's title names nobody");
    }

    /// On a computer without SIOUL_PHONE_MESSAGES and no phone's log here:
    /// nothing listed, nothing written for a notification.
    #[test]
    fn a_computer_writes_nothing_and_lists_nothing_of_its_own() {
        if phone() || rules::folder().join(rules::LOG).is_dir() {
            return;
        }
        // No line; the texts' notices only where texts are read here (none in a test).
        assert_eq!(view(), json!({ "lines": [], "texts": crate::texts::readable(), "notices": crate::texts::notices() }).to_string());
        assert!(history(&["+33199001234".to_string()]).is_empty());
        let posted = Posted { package: "foundation.e.message".into(), sms_app: true, ..Posted::default() };
        assert!(!note("{}", &posted, &Kind::Untouched, None, false, &Decision::through(sioul_core::appnotes::Why::Allowed)));
    }
}

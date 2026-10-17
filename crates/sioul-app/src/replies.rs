// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Replies from your computers to your phone's notifications, on the phone
//! (docs/android.md, "Replies from your computers"; the rules are
//! `sioul_core::replies`). The listener's side, in its own process
//! (AppNotes.java, Replies.java), through `sioul_replies_listener`:
//!
//! - **What is due** ("due"): what the background step handed over, sealed
//!   in the phone's private replies folder, not taken yet: its key, its verb
//!   and its notification's name, never the words.
//! - **Checked** ("check"): the live notification Java found by that name is
//!   still the conversation answered (`replies::check`), and whether their
//!   newer messages crossed the reply.
//! - **Taken** ("taken"): written and flushed before Java sends anything;
//!   the words only then, in memory.
//! - **What happened** ("result"): said in the private folder, never the
//!   words; a reply handed is looked for in its app's next notification, and
//!   said "shown" when its words are there as yours (`heard`).
//! - **Gone** ("removed"): a relayed notification gone from the phone, said
//!   in its live lines (`replies::went`), so that your computers no longer
//!   offer its Reply.
//!
//! And for the lines the listener writes (`phonemsgs::note`): what each
//! relayed notification offers, in this phone's live lines (`offered`).

use crate::backend::load_config;
use serde_json::{Value, json};
use sioul_core::appnotes::Posted;
use sioul_core::config::state_dir;
use sioul_core::phonemsgs::{self, Extra, Line};
use sioul_core::replies::{self as rules, Checked, Done, Live, Taken};
use sioul_sync::textseal::ReplySeal;
use std::ffi::c_char;
use std::sync::Mutex;

fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

/// The replies' seal at rest, from the sharing key; none where sharing is not set up here.
fn seal() -> Option<ReplySeal> {
    let (_, vault) = crate::share::vault()?;
    let (_, key) = vault?;
    Some(ReplySeal::new(&key))
}

/// The states the listener may say (`replies::Done::state`); anything else is "failed".
const STATES: &[&str] = &["handed", "gone", "no-reply", "no-read", "closed", "locked", "activity", "failed"];

/// Taken and not said yet, in the listener's process: the key, its
/// notification's name and its words' hash (`replies::words_hash`), never the words.
static TAKEN: Mutex<Vec<(String, String, String)>> = Mutex::new(Vec::new());
/// Handed lately: the key, its notification's name, its words' hash, when;
/// looked for in its app's next notification (`heard`) for `SHOWN_WITHIN_MS`.
static HANDED: Mutex<Vec<(String, String, String, i64)>> = Mutex::new(Vec::new());

/// The listener's question (AppNotes.nativeReply): its verb and its JSON, a JSON answer.
fn listener(verb: &str, json: &str) -> Value {
    let asked: Value = serde_json::from_str(json).unwrap_or_default();
    let key = asked["key"].as_str().unwrap_or_default();
    let folder = rules::private_folder();
    match verb {
        "due" => {
            let Some(seal) = seal() else { return json!({ "due": [] }) };
            let due: Vec<Value> = rules::waiting(&folder, &seal).iter().map(|d| json!({ "key": d.key, "verb": d.verb, "notice": d.notice })).collect();
            json!({ "due": due })
        }
        "check" => check(key, &asked["posted"]),
        "taken" => taken(key),
        "result" => result(key, &asked),
        "removed" => json!({ "shared": removed(asked["package"].as_str().unwrap_or_default(), asked["key"].as_str().unwrap_or_default()) }),
        _ => Value::Null,
    }
}

/// The live notification Java found for one due: still the same app,
/// notification and conversation; whether their newer messages crossed it.
fn check(key: &str, posted: &Value) -> Value {
    let gone = json!({ "go": false, "state": "gone" });
    let Some(seal) = seal() else { return gone };
    let Some(due) = rules::read_due(&rules::private_folder(), &seal).into_iter().find(|d| d.key == key) else { return gone };
    let Ok(read) = serde_json::from_value::<Posted>(posted.clone()) else { return gone };
    let extra = Extra::read(&posted.to_string());
    let config = load_config();
    let choices = sioul_core::appnotes::Choices::load(&sioul_core::appnotes::Choices::default_path());
    let words = sioul_core::words::Words::of(&config);
    match rules::check(&due, &read, &extra, &choices, &words, sioul_core::reach::region(&config)) {
        Checked::Go { crossed } => json!({ "go": true, "crossed": crossed }),
        Checked::Gone => gone,
    }
}

/// One taken, once: "taken" written and flushed before Java sends it; its
/// words given to Java then, in memory. Late, said so, never sent.
fn taken(key: &str) -> Value {
    let Some(seal) = seal() else { return json!({ "go": false }) };
    match rules::take(&rules::private_folder(), key, &seal, now_ms()) {
        Ok(Taken::Go(due)) => {
            if let Ok(mut taken) = TAKEN.lock() {
                taken.retain(|(k, _, _)| k != key);
                taken.push((key.to_string(), due.notice.clone(), rules::words_hash(&due.words)));
            }
            json!({ "go": true, "words": due.words })
        }
        Ok(Taken::Late) => {
            eprintln!("sioul: replies: one came to the listener too late, not sent");
            json!({ "go": false, "said": true })
        }
        Ok(Taken::No) => json!({ "go": false }),
        Err(e) => {
            // Not written: nothing is sent.
            eprintln!("sioul: replies: {e}");
            json!({ "go": false })
        }
    }
}

/// What became of one, as Java says it, written in the private folder;
/// one handed is looked for in its app's next notification (`heard`).
fn result(key: &str, asked: &Value) -> Value {
    let state = asked["state"].as_str().filter(|s| STATES.contains(s)).unwrap_or("failed");
    let now = now_ms();
    let done = Done { crossed: asked["crossed"] == true, held: asked["held"] == true, ..Done::of(key, state, now) };
    if let Err(e) = rules::say(&rules::private_folder(), &done) {
        eprintln!("sioul: replies: {e}");
    }
    let taken = TAKEN.lock().ok().and_then(|mut taken| {
        let at = taken.iter().position(|(k, _, _)| k == key)?;
        Some(taken.remove(at))
    });
    if state == "handed"
        && let Some((key, notice, hash)) = taken
        && let Ok(mut handed) = HANDED.lock()
    {
        handed.retain(|h| now - h.3 <= rules::SHOWN_WITHIN_MS);
        handed.push((key, notice, hash, now));
    }
    json!({ "said": true })
}

/// A reply handed lately looked for in its app's notification posted now
/// (`appnotes::decide`, in the listener's process): its words there as
/// yours, it is said "shown". Whether that was said (a step soon).
pub(crate) fn heard(json: &str, posted: &Posted) -> bool {
    let Ok(mut handed) = HANDED.lock() else { return false };
    if handed.is_empty() || posted.key.is_empty() {
        return false;
    }
    let now = now_ms();
    handed.retain(|h| now - h.3 <= rules::SHOWN_WITHIN_MS);
    let notice = rules::notice(&posted.package, &posted.key);
    let extra = Extra::read(json);
    let shown: Vec<String> = handed.iter().filter(|h| h.1 == notice && rules::shows(posted, &extra, &h.2)).map(|h| h.0.clone()).collect();
    handed.retain(|h| !shown.contains(&h.0));
    drop(handed);
    for key in &shown {
        if let Err(e) = rules::say(&rules::private_folder(), &Done::of(key, "shown", now)) {
            eprintln!("sioul: replies: {e}");
        }
    }
    !shown.is_empty()
}

/// This phone's own live lines (`replies::LIVE`), while the part "Messages
/// from your phone" is on here; none elsewhere.
fn own_live() -> Option<std::path::PathBuf> {
    let here = sioul_sync::share::Here::load(&state_dir());
    (!here.id.is_empty() && here.folder.is_some() && here.shares(phonemsgs::PART, &load_config())).then(|| rules::live_file(&phonemsgs::folder(), &here.id))
}

/// What a relayed notification offers a reply from your computers, said
/// in this phone's live lines when it changed (`replies::offer`): written
/// with its lines (`phonemsgs::note`). Whether a line was written.
pub(crate) fn offered(device: &str, posted: &Posted, lines: &[Line], extra: &Extra, now_ms: i64) -> bool {
    let Some(first) = lines.first().filter(|l| !l.notice.is_empty()) else { return false };
    let live = Live { notice: first.notice.clone(), app: posted.package.clone(), talk: first.talk.clone(), reply: extra.offer.reply, read: extra.offer.read, at: now_ms, ..Live::default() };
    match rules::offer(&rules::live_file(&phonemsgs::folder(), device), &live) {
        Ok(written) => written,
        Err(e) => {
            eprintln!("sioul: replies: {e}");
            false
        }
    }
}

/// A relayed notification gone from the phone, said in its live lines;
/// nothing for one never relayed. Whether a line was written.
fn removed(package: &str, key: &str) -> bool {
    if package.is_empty() || key.is_empty() {
        return false;
    }
    let Some(own) = own_live() else { return false };
    match rules::went(&own, &rules::notice(package, key), now_ms()) {
        Ok(written) => written,
        Err(e) => {
            eprintln!("sioul: replies: {e}");
            false
        }
    }
}

/// `listener`, for Java (android/main.cpp, AppNotes.nativeReply); a panic
/// answers "null", and nothing is sent. Given back to `sioul_string_free`.
///
/// # Safety
/// `verb` and `json` are null, or zero-terminated texts valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_replies_listener(verb: *const c_char, json: *const c_char) -> *mut c_char {
    // SAFETY: each null or a C string valid for the call, as this function's contract says.
    let (verb, json) = unsafe { (crate::alarms::key_of(verb), crate::alarms::key_of(json)) };
    let answer = std::panic::catch_unwind(|| listener(&verb, &json).to_string()).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A relayed notification's offer goes into this phone's live lines
    /// (in the test's own state folder) once per change; a line from an older
    /// phone, without its notification's name, says nothing.
    #[test]
    fn a_relayed_notification_says_what_it_offers() {
        let device = format!("phone-{}", std::process::id());
        let posted = Posted { key: "0|org.thoughtcrime.securesms|42|null|10123".into(), package: "org.thoughtcrime.securesms".into(), ..Posted::default() };
        let line = Line { at: 1, id: "1-a".into(), notice: rules::notice(&posted.package, &posted.key), talk: "fedcba9876543210".into(), ..Line::default() };
        let extra = Extra { offer: phonemsgs::Offer { reply: true, read: false }, ..Extra::default() };
        let now = now_ms();
        assert!(offered(&device, &posted, std::slice::from_ref(&line), &extra, now));
        assert!(!offered(&device, &posted, std::slice::from_ref(&line), &extra, now + 1_000), "the same offer");
        assert!(!offered(&device, &posted, &[Line { notice: String::new(), ..line.clone() }], &extra, now + 2_000), "an older line, no name");
        assert!(!offered(&device, &posted, &[], &extra, now + 3_000));
        let live = rules::read_live(&phonemsgs::folder(), now + 4_000);
        let said = live.get(&(device.clone(), line.notice.clone())).unwrap();
        assert_eq!((said.reply, said.read, said.talk.as_str(), said.app.as_str()), (true, false, "fedcba9876543210", "org.thoughtcrime.securesms"));
        let _ = std::fs::remove_file(rules::live_file(&phonemsgs::folder(), &device));
    }
}

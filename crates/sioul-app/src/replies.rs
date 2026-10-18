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
//!
//! The phone's step (`phone_step`, and before each exchange): each request
//! for this phone decided once by the texts' rules (`replies::verdict`),
//! claimed in its private ledger and flushed, then handed to the listener;
//! the listener's words copied into its outcomes. On a computer: a reply
//! written there (`ask`), or taken back while it waits (`take_back`).

use crate::backend::load_config;
use serde_json::{Value, json};
use sioul_core::appnotes::Posted;
use sioul_core::calls;
use sioul_core::config::state_dir;
use sioul_core::phonemsgs::{self, Extra, Line};
use sioul_core::replies::{self as rules, Ask, Checked, Done, Judge, Live, Outcome, Taken, Verdict};
use sioul_core::texts::{Claim, Ledger};
use sioul_sync::textseal::ReplySeal;
use std::ffi::c_char;
use std::path::{Path, PathBuf};
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

// ---------------------------------------------------------------- the request, and never twice

/// Whether this device is a phone whose listener sends replies; SIOUL_PHONE_MESSAGES makes a computer's step run too, to look at it.
fn phone() -> bool {
    cfg!(target_os = "android") || std::env::var_os("SIOUL_PHONE_MESSAGES").is_some()
}

/// This device's name, the sharing folder and its key, while the part
/// "Messages from your phone" is on here; none otherwise.
fn sharing() -> Option<(String, PathBuf, [u8; 32])> {
    let (here, vault) = crate::share::vault()?;
    let (folder, key) = vault?;
    crate::share::shares(phonemsgs::PART).then_some((here, folder, key))
}

/// When this device last took its old requests and outcomes out (ms): once a day.
static TRIMMED: Mutex<i64> = Mutex::new(0);

/// Before each exchange (share.rs), with the part on: on a phone, its step
/// (`phone_step`); once a day, on every device, the requests and outcomes
/// past their week out (another device's ten days on: a computer gone for
/// good never takes its own out), and on a phone its private files' old lines.
pub(crate) fn before_exchange() {
    let Some((here, folder, key)) = sharing() else { return };
    let now = now_ms();
    if phone() {
        step(&here, &folder, &key, now);
    }
    let due = TRIMMED.lock().map(|mut last| {
        let due = now - *last > 86_400_000;
        if due {
            *last = now;
        }
        due
    });
    if due.unwrap_or(false) {
        let seal = ReplySeal::new(&key);
        let trimmed = rules::trim_shared(&phonemsgs::folder(), &here, &seal, now).and_then(|_| if phone() { rules::trim_private(&rules::private_folder(), &seal, now) } else { Ok(()) });
        if let Err(e) = trimmed {
            eprintln!("sioul: replies: {e}");
        }
    }
}

/// The phone's step for replies, after an exchange that may have brought
/// requests (steps.rs, as texts'): decided, claimed and handed to the
/// listener; what the listener did said in this phone's outcomes. Whether an
/// outcome was written (to share now).
pub(crate) fn phone_step() -> bool {
    if !phone() {
        return false;
    }
    let Some((here, folder, key)) = sharing() else { return false };
    step(&here, &folder, &key, now_ms())
}

/// `phone_step`'s work: the requests under every root (the records' copy,
/// the sharing folder's small files, the copy fetched from the server),
/// from the devices the sharing counts now (asked only when one is for
/// this phone).
fn step(here: &str, folder: &Path, key: &[u8; 32], now: i64) -> bool {
    let seal = ReplySeal::new(key);
    let root = phonemsgs::folder();
    let roots: Vec<PathBuf> = std::iter::once(root.clone()).chain(crate::share::replies_copies()).collect();
    let own = calls::own_file(&root, rules::OUTCOME, here);
    let private = rules::private_folder();
    let asked = rules::read_asks(&roots, &seal);
    let decided = asked.0.iter().any(|a| a.phone == here) && {
        let counted = sioul_sync::share::counting(folder, key, &crate::share::memory_path(), here, now);
        decide(here, &asked, &root, &private, &seal, &|device: &str| counted.contains(device), &own, now)
    };
    let settled = settle(&private, &own, now);
    decided || settled
}

/// Each request for this phone decided once (`replies::verdict`), under the
/// ledger's lock both its processes take: claimed and flushed before the
/// listener can see it, then handed over in the private file it watches
/// (`replies::add_due`); the others said, and written in the ledger, so
/// that each is decided once. Whether an outcome was written.
#[allow(clippy::too_many_arguments)]
fn decide(here: &str, asked: &(Vec<Ask>, std::collections::BTreeSet<(String, String)>), root: &Path, private: &Path, seal: &ReplySeal, counted: &dyn Fn(&str) -> bool, own: &Path, now: i64) -> bool {
    let (asks, cancelled) = asked;
    if asks.iter().all(|a| a.phone != here) {
        return false;
    }
    let lines = phonemsgs::read_file(&calls::own_file(root, phonemsgs::LOG, here));
    let ledger_path = private.join(sioul_core::texts::LEDGER);
    let decided = sioul_core::filelock::with_lock(&ledger_path, || -> Result<(Vec<rules::Due>, Vec<Outcome>), String> {
        let ledger = Ledger::open(private, now)?;
        // The phone's switch per app, "Answer from your computers", comes with the Porch's box.
        let judge = Judge { here, now_ms: now, ledger: &ledger, lines: &lines, counted, cancelled, answers: &|_: &str| true };
        let (mut due, mut said) = (Vec::new(), Vec::new());
        for ask in asks {
            let (state, reason) = match rules::verdict(ask, &judge) {
                Verdict::NotMine | Verdict::Decided => continue,
                Verdict::Send => {
                    sioul_core::texts::record(private, &rules::claim_of(ask, now))?;
                    if let Some(line) = lines.iter().find(|l| l.id == ask.line) {
                        due.push(rules::due_of(ask, line, now));
                    }
                    continue;
                }
                Verdict::TakenBack => ("taken-back", ""),
                Verdict::Expired => ("expired", ""),
                Verdict::Clocks => ("clocks", ""),
                Verdict::Refused(why) => ("refused", why),
            };
            sioul_core::texts::record(private, &Claim { key: ask.key.clone(), at: now, state: state.into(), ..Claim::default() })?;
            said.push(Outcome { key: ask.key.clone(), state: state.into(), at: now, reason: reason.into(), ..Outcome::default() });
        }
        Ok((due, said))
    });
    match decided {
        Ok((due, said)) => {
            if !due.is_empty() {
                // Claimed already: not handed over (a full disk), each is said "late" in two minutes, never sent.
                match rules::add_due(private, &due, seal, now) {
                    Ok(()) => eprintln!("sioul: replies: {} handed to the listener", due.len()),
                    Err(e) => eprintln!("sioul: replies: {e}"),
                }
            }
            match rules::say_outcomes(own, &said) {
                Ok(written) => written > 0,
                Err(e) => {
                    eprintln!("sioul: replies: {e}");
                    false
                }
            }
        }
        Err(e) => {
            // A claim not flushed: nothing is handed over.
            eprintln!("sioul: replies: {e}");
            false
        }
    }
}

/// What the listener did said in this phone's outcomes, each state once;
/// a claim it never took within two minutes said "late", one it took with
/// no word after "doubt" (`replies::settle`); each claim ended so written
/// in the ledger. Whether an outcome was written.
fn settle(private: &Path, own: &Path, now: i64) -> bool {
    if !private.join(sioul_core::texts::LEDGER).is_file() {
        return false;
    }
    let Ok(ledger) = Ledger::open(private, now) else { return false };
    let mut last: std::collections::BTreeMap<&str, &Claim> = std::collections::BTreeMap::new();
    for claim in &ledger.claims {
        last.insert(claim.key.as_str(), claim);
    }
    let claimed: Vec<(String, i64)> = last.values().filter(|c| c.state == "claimed").map(|c| (c.key.clone(), c.at)).collect();
    let done = match rules::settle(private, &claimed, now) {
        Ok(done) => done,
        Err(e) => {
            eprintln!("sioul: replies: {e}");
            return false;
        }
    };
    let written = rules::say_outcomes(own, &rules::outcomes_of(&done)).unwrap_or_else(|e| {
        eprintln!("sioul: replies: {e}");
        0
    });
    for (key, _) in &claimed {
        if done.iter().any(|d| d.key == *key && d.state != "taken") {
            let _ = sioul_core::texts::record(private, &Claim { key: key.clone(), at: now, state: "done".into(), ..Claim::default() });
        }
    }
    written > 0
}

/// A reply, or a Mark as read, written on this computer for a phone's
/// notification (the Porch's line): refused here at once for what the phone
/// would refuse anyway (no words, too long), else a request with a key of
/// its own in this device's file, which the sharing carries, sent now on
/// the lane of the texts to send. Its key, or why not, in words.
pub(crate) fn ask(phone: &str, verb: &str, notice: &str, app: &str, line: &str, words: &str, again: &str) -> Result<String, String> {
    let tr = crate::backend::tr();
    let Some((here, _, key)) = sharing() else { return Err(tr.text("replies-refused", None)) };
    let words = if verb == "reply" { words.trim().to_string() } else { String::new() };
    if verb == "reply" && words.is_empty() {
        return Err(tr.text("replies-refused-no-words", None));
    }
    if words.chars().count() > rules::WORDS_MAX {
        return Err(tr.text("replies-refused-too-long", None));
    }
    let ask = Ask { key: sioul_core::texts::new_key(), phone: phone.to_string(), verb: verb.to_string(), notice: notice.to_string(), app: app.to_string(), line: line.to_string(), words, written: now_ms(), ahead: crate::texts::ahead_of(phone), again: again.to_string(), ..Ask::default() };
    rules::write_ask(&phonemsgs::folder(), &here, &ask, &ReplySeal::new(&key))?;
    crate::share::replies_now();
    Ok(ask.key)
}

/// A request taken back while it waits for the phone (Take back): a
/// "cancel" of its key from this device, which the phone files as taken
/// back unless it claimed it already (its outcome then says what became of it).
pub(crate) fn take_back(key: &str) -> Result<(), String> {
    let Some((here, _, sharing_key)) = sharing() else { return Ok(()) };
    let seal = ReplySeal::new(&sharing_key);
    let root = phonemsgs::folder();
    let (asks, _) = rules::read_asks(std::slice::from_ref(&root), &seal);
    let Some(asked) = asks.into_iter().find(|a| a.key == key && a.device == here) else { return Ok(()) };
    let cancel = Ask { verb: "cancel".into(), words: String::new(), written: now_ms(), ahead: crate::texts::ahead_of(&asked.phone), again: String::new(), device: String::new(), ..asked };
    rules::write_ask(&root, &here, &cancel, &seal)?;
    crate::share::replies_now();
    Ok(())
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

    /// A phone's step, on stand-in folders: a request decided once, claimed
    /// before the listener sees it, handed over with its words in the sealed
    /// due file; the listener's word copied into the phone's outcomes, the
    /// claim ended; one taken back, one from a device the sharing no longer
    /// counts, one the listener never took (late): each said once, without
    /// the words.
    #[test]
    fn the_phone_decides_each_request_once_and_says_what_became_of_it() {
        let base = std::env::temp_dir().join(format!("sioul-app-replies-step-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (root, private) = (base.join("phone-messages"), base.join("private"));
        let seal = ReplySeal::new(&[9u8; 32]);
        let now = now_ms();
        let line = Line { at: now - 300_000, id: format!("{}-aa", now - 300_000), app: "org.thoughtcrime.securesms".into(), kind: "chat".into(), talk: "fedcba9876543210".into(), notice: "0123456789abcdef".into(), name: "Alice".into(), key: "+33199001234".into(), who: "neutral".into(), text: "Tu passes ?".into(), shows: now - 300_000, ..Line::default() };
        phonemsgs::carry_into(&calls::own_file(&root, phonemsgs::LOG, "phone"), std::slice::from_ref(&line), now).unwrap();
        Ledger::open(&private, now - 60_000).unwrap();
        let ask = |key: &str, device: &str| (device.to_string(), Ask { key: key.into(), phone: "phone".into(), verb: "reply".into(), notice: line.notice.clone(), app: line.app.clone(), line: line.id.clone(), words: format!("Réponse {key}."), written: now, ..Ask::default() });
        for (device, a) in [ask("k1", "desk"), ask("k2", "desk"), ask("k3", "laptop"), ask("k4", "desk")] {
            rules::write_ask(&root, &device, &a, &seal).unwrap();
        }
        rules::write_ask(&root, "desk", &Ask { verb: "cancel".into(), words: String::new(), ..ask("k2", "desk").1 }, &seal).unwrap();
        let own = calls::own_file(&root, rules::OUTCOME, "phone");
        let counted = |device: &str| device == "desk";
        let roots = vec![root.clone()];
        let asked = || rules::read_asks(&roots, &seal);
        assert!(decide("phone", &asked(), &root, &private, &seal, &counted, &own, now), "taken back and not counted, said");
        let due = rules::read_due(&private, &seal);
        assert_eq!(due.iter().map(|d| (d.key.as_str(), d.words.as_str(), d.talk.as_str())).collect::<Vec<_>>(), [("k1", "Réponse k1.", "fedcba9876543210"), ("k4", "Réponse k4.", "fedcba9876543210")]);
        let states = |o: &[Outcome]| o.iter().map(|o| (o.key.clone(), o.state.clone(), o.reason.clone())).collect::<Vec<_>>();
        let said = rules::read_outcomes(&root);
        assert_eq!(states(&said), [("k2".to_string(), "taken-back".to_string(), String::new()), ("k3".into(), "refused".into(), "not-counted".into())]);
        // Read again: decided once.
        assert!(!decide("phone", &asked(), &root, &private, &seal, &counted, &own, now + 1_000));
        assert_eq!(rules::read_due(&private, &seal).len(), 2);
        // The listener takes k1 and hands it; k4 it never takes.
        assert!(matches!(rules::take(&private, "k1", &seal, now + 2_000).unwrap(), rules::Taken::Go(_)));
        rules::say(&private, &Done::of("k1", "handed", now + 3_000)).unwrap();
        assert!(settle(&private, &own, now + 4_000));
        assert!(rules::read_outcomes(&root).iter().any(|o| o.key == "k1" && o.state == "handed"));
        assert!(settle(&private, &own, now + 3 * 60_000), "k4, never taken: late");
        let all = rules::read_outcomes(&root);
        assert!(all.iter().any(|o| o.key == "k4" && o.state == "late"), "{all:?}");
        assert!(!settle(&private, &own, now + 4 * 60_000), "each once; the claims ended");
        let ledger = Ledger::open(&private, now).unwrap();
        assert_eq!((ledger.state("k1"), ledger.state("k4")), (Some("done"), Some("done")));
        let text = std::fs::read_to_string(&own).unwrap();
        assert!(!text.contains("Réponse") && !text.contains("Alice"), "no words, no names");
        let _ = std::fs::remove_dir_all(&base);
    }
}

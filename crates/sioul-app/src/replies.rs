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
use sioul_core::texts::Claim;
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
const STATES: &[&str] = &["handed", "gone", "other", "no-reply", "no-read", "closed", "locked", "activity", "failed"];

/// A reply on its way, in the listener's process, never its words: its key,
/// its notification's name, its words' hash (`replies::words_hash`), its
/// conversation's key (to let their answer through: `replies::note_answered`),
/// when it was taken, and whether its words were in the notification
/// already before it went (`replies::shown_already`: then only a message of
/// yours timed after it says it shown).
#[derive(Debug, Clone)]
struct Sent {
    key: String,
    notice: String,
    hash: String,
    talk: String,
    taken: i64,
    before: bool,
}

/// Checked and not taken yet: the key and whether its words were shown already (`check`).
static CHECKED: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());
/// Taken and not said yet.
static TAKEN: Mutex<Vec<Sent>> = Mutex::new(Vec::new());
/// Handed lately, looked for in its app's next notification (`heard`) for `SHOWN_WITHIN_MS`.
static HANDED: Mutex<Vec<(Sent, i64)>> = Mutex::new(Vec::new());

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
        Checked::Go { crossed } => {
            // Its words there already, before it goes (an earlier reply of yours): said "shown" only by a newer message of yours.
            let before = due.verb == "reply" && rules::shown_already(&read, &extra, &rules::words_hash(&due.words));
            if let Ok(mut checked) = CHECKED.lock() {
                checked.retain(|(k, _)| k != key);
                checked.push((key.to_string(), before));
            }
            json!({ "go": true, "crossed": crossed })
        }
        Checked::Gone => gone,
        Checked::Other => json!({ "go": false, "state": "other" }),
    }
}

/// One taken, once: "taken" written and flushed before Java sends it; its
/// words given to Java then, in memory. Late, said so, never sent.
fn taken(key: &str) -> Value {
    let Some(seal) = seal() else { return json!({ "go": false }) };
    let now = now_ms();
    match rules::take(&rules::private_folder(), key, &seal, now) {
        Ok(Taken::Go(due)) => {
            let before = CHECKED.lock().ok().and_then(|mut checked| {
                let at = checked.iter().position(|(k, _)| k == key)?;
                Some(checked.remove(at).1)
            });
            if let Ok(mut taken) = TAKEN.lock() {
                taken.retain(|s| s.key != key);
                let talk = if due.verb == "reply" { due.talk.clone() } else { String::new() };
                taken.push(Sent { key: key.to_string(), notice: due.notice.clone(), hash: rules::words_hash(&due.words), talk, taken: now, before: before.unwrap_or(true) });
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
        let at = taken.iter().position(|s| s.key == key)?;
        Some(taken.remove(at))
    });
    if state == "handed"
        && let Some(sent) = taken
    {
        // Their answer to your answer comes through at once for twenty minutes (decision 18).
        if let Err(e) = rules::note_answered(&rules::private_folder(), &sent.talk, now) {
            eprintln!("sioul: replies: {e}");
        }
        if let Ok(mut handed) = HANDED.lock() {
            handed.retain(|h| now - h.1 <= rules::SHOWN_WITHIN_MS);
            handed.push((sent, now));
        }
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
    handed.retain(|h| now - h.1 <= rules::SHOWN_WITHIN_MS);
    let notice = rules::notice(&posted.package, &posted.key);
    let extra = Extra::read(json);
    let shown: Vec<String> = handed.iter().filter(|(s, _)| s.notice == notice && rules::shows(posted, &extra, &s.hash, s.taken, s.before)).map(|(s, _)| s.key.clone()).collect();
    handed.retain(|(s, _)| !shown.contains(&s.key));
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
pub(crate) fn offered(device: &str, posted: &Posted, lines: &[Line], extra: &Extra, answers: bool, now_ms: i64) -> bool {
    let Some(first) = lines.first().filter(|l| !l.notice.is_empty()) else { return false };
    let live = Live { notice: first.notice.clone(), app: posted.package.clone(), talk: first.talk.clone(), reply: answers && extra.offer.reply, read: answers && extra.offer.read, at: now_ms, ..Live::default() };
    match rules::offer(&rules::live_file(&phonemsgs::folder(), device), &live) {
        Ok(written) => written,
        Err(e) => {
            eprintln!("sioul: replies: {e}");
            false
        }
    }
}

/// A line relayed for your computers (`phonemsgs::note`): the replies'
/// ledger's mark made now when there is none (`replies::mark_ledger`), so
/// that a reply to it is never written before the mark.
pub(crate) fn relayed(now_ms: i64) {
    if let Err(e) = rules::mark_ledger(&rules::private_folder(), now_ms) {
        eprintln!("sioul: replies: {e}");
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
    // The ledger's mark, at every step, before any request is read (as texts do): the first reply is written after it.
    if let Err(e) = rules::mark_ledger(&private, now) {
        eprintln!("sioul: replies: {e}");
    }
    // Each file from where this phone was done with it: what it decided is never opened again.
    let marks = rules::read_marks(&private);
    let (asked, reads) = rules::read_asks_after(&roots, &seal, &marks);
    let decided = asked.0.iter().any(|a| a.phone == here) && {
        let counted = sioul_sync::share::counting(folder, key, &crate::share::memory_path(), here, now);
        decide(here, &asked, &root, &private, &seal, &|device: &str| counted.contains(device), &own, now)
    };
    remember_reads(here, &private, &reads, &marks, now);
    let settled = settle(&private, &own, now);
    decided || settled
}

/// The marks moved past what this step is done with (`replies::moved_marks`):
/// each line decided now, by the ledger, or another phone's.
fn remember_reads(here: &str, private: &Path, reads: &[rules::FileRead], marks: &std::collections::BTreeMap<String, rules::ReadMark>, now: i64) {
    let Ok(ledger) = rules::open_ledger(private, now) else { return };
    let moved = rules::moved_marks(reads, &|key: &str, phone: &str| phone != here || ledger.has(key), marks);
    if moved != *marks
        && let Err(e) = rules::save_read_marks(private, &moved)
    {
        eprintln!("sioul: replies: {e}");
    }
}

/// Each request for this phone decided once (`replies::verdict`), at most
/// `replies::STEP_MAX` a step, under the ledger's lock both its processes
/// take: every decision of the step written in the ledger together and
/// flushed once, the claims among them before the listener can see one,
/// then handed over in the private file it watches (`replies::add_due`);
/// the others said. Each request is decided once; one decided and not said
/// (its word could not be written) is said at the next step (`settle`).
/// Whether an outcome was written.
#[allow(clippy::too_many_arguments)]
fn decide(here: &str, asked: &(Vec<Ask>, std::collections::BTreeSet<(String, String)>), root: &Path, private: &Path, seal: &ReplySeal, counted: &dyn Fn(&str) -> bool, own: &Path, now: i64) -> bool {
    let (asks, cancelled) = asked;
    if asks.iter().all(|a| a.phone != here) {
        return false;
    }
    let lines = phonemsgs::read_file(&calls::own_file(root, phonemsgs::LOG, here));
    // The replies its listener said were not sent: they never hold the same words back; those it said went.
    let (unsent, went) = (rules::unsent(private), rules::went_keys(private));
    let ledger_path = private.join(sioul_core::texts::LEDGER);
    let decided = sioul_core::filelock::with_lock(&ledger_path, || -> Result<(Vec<rules::Due>, Vec<Outcome>), String> {
        let mut ledger = rules::open_ledger(private, now)?;
        // The phone's switch per app, "Answer from your computers" (Settings ▸ This phone ▸ On your computers).
        let choices = phonemsgs::Choices::load(&phonemsgs::Choices::default_path());
        let answers = |app: &str| choices.answers(app);
        let (mut lines_out, mut due, mut said) = (Vec::new(), Vec::new(), Vec::new());
        for ask in rules::in_turn(asks, here) {
            if lines_out.len() >= rules::STEP_MAX {
                break;
            }
            let judge = Judge { here, now_ms: now, ledger: &ledger, lines: &lines, counted, cancelled, answers: &answers, unsent: &unsent, went: &went };
            let (state, reason) = match rules::verdict(ask, &judge) {
                Verdict::NotMine | Verdict::Decided => continue,
                Verdict::Send => {
                    let claim = rules::claim_of(ask, now);
                    // Seen by this step's next requests (the guards), as the ledger will hold it.
                    ledger.add(claim.clone());
                    lines_out.push(claim);
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
            let end = rules::end_of(&ask.key, state, reason, now);
            ledger.add(end.clone());
            lines_out.push(end);
            said.push(Outcome { key: ask.key.clone(), state: state.into(), at: now, reason: reason.into(), ..Outcome::default() });
        }
        // Every decision flushed, the claims among them, before anything is handed over or said.
        sioul_core::texts::record_all(private, &lines_out)?;
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
                    // Decided and in the ledger: said at the next step (`settle`).
                    eprintln!("sioul: replies: {e}");
                    false
                }
            }
        }
        Err(e) => {
            // Nothing flushed: nothing handed over, nothing said; decided again at the next step.
            eprintln!("sioul: replies: {e}");
            false
        }
    }
}

/// What the listener did said in this phone's outcomes, each state once;
/// a claim it never took within two minutes said "late", one it took with
/// no word after "doubt" (`replies::settle`); each claim ended so written
/// in the ledger; and a decision the ledger holds without its word (one
/// that could not be written at its step), said now (`replies::ends_of`).
/// Whether an outcome was written.
fn settle(private: &Path, own: &Path, now: i64) -> bool {
    if !private.join(sioul_core::texts::LEDGER).is_file() {
        return false;
    }
    let Ok(ledger) = rules::open_ledger(private, now) else { return false };
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
    let mut outcomes = rules::outcomes_of(&done);
    outcomes.extend(rules::ends_of(&ledger, now));
    let written = rules::say_outcomes(own, &outcomes).unwrap_or_else(|e| {
        eprintln!("sioul: replies: {e}");
        0
    });
    // Each claim ended so, written together: one flush a step.
    let ended: std::collections::BTreeSet<&str> = done.iter().filter(|d| d.state != "taken").map(|d| d.key.as_str()).collect();
    let ends: Vec<Claim> = claimed.iter().filter(|(key, _)| ended.contains(key.as_str())).map(|(key, _)| Claim { key: key.clone(), at: now, state: "done".into(), ..Claim::default() }).collect();
    if let Err(e) = sioul_core::texts::record_all(private, &ends) {
        eprintln!("sioul: replies: {e}");
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

/// Whether a conversation was answered from a computer within twenty
/// minutes (`replies::answered_lately`): `appnotes::decide` lets its
/// messages through at once then, but in a pause.
pub(crate) fn answered_lately(talk: &str) -> bool {
    rules::answered_lately(&rules::private_folder(), talk, now_ms())
}

/// A text a computer asked for, handed to Android (`texts`): an answer as
/// a reply is, by a hash of the number (`replies::answered_number`), so
/// that their next texts come through at once for twenty minutes too.
pub(crate) fn texted(number: &str) {
    if let Err(e) = rules::note_answered(&rules::private_folder(), &rules::answered_number(number), now_ms()) {
        eprintln!("sioul: replies: {e}");
    }
}

/// Whether one of these numbers (`phones::key`) was texted from a computer within twenty minutes.
pub(crate) fn texted_lately(numbers: &[String]) -> bool {
    let (folder, now) = (rules::private_folder(), now_ms());
    numbers.iter().any(|n| rules::answered_lately(&folder, &rules::answered_number(n), now))
}

/// What this computer answered lately, for its Porch (`replies::Answered`):
/// its replies handed or shown within twenty minutes, the numbers it texted then.
pub(crate) fn answered_here(all: &[Line], now: &jiff::Zoned) -> rules::Answered {
    let Some((here, _, key)) = sharing() else { return rules::Answered::default() };
    let root = phonemsgs::folder();
    let now_ms = now.timestamp().as_millisecond();
    let own: Vec<Ask> = rules::read_asks(std::slice::from_ref(&root), &ReplySeal::new(&key)).0.into_iter().filter(|a| a.device == here).collect();
    let mut answered = rules::Answered::of(&own, &rules::read_outcomes(&root), all, now_ms);
    // The texts this computer sent, read only where a phone's line is a text.
    if all.iter().any(|l| l.kind == "text") {
        for number in crate::texts::texted_since(now_ms - rules::ANSWERED_MS) {
            answered.texted(number);
        }
    }
    answered
}

/// The Porch's lines' answers on a computer with the part on
/// (`replies::answer_of`): what each phone says of its notifications, this
/// computer's own requests and every phone's outcomes, the texts this
/// computer wrote; a line whose Mark as read was handed is seen, here and on
/// every device, and leaves the Porch (decision 13).
pub(crate) fn fill(lines: &mut Vec<sioul_core::phonemsgs::PorchLine>, all: &[Line], now: &jiff::Zoned) {
    let Some((here, _, key)) = sharing() else { return };
    let root = phonemsgs::folder();
    let now_ms = now.timestamp().as_millisecond();
    let live = rules::read_live(&root, now_ms);
    let seal = ReplySeal::new(&key);
    let (asks, cancels) = rules::read_asks(std::slice::from_ref(&root), &seal);
    let own: Vec<Ask> = asks.into_iter().filter(|a| a.device == here).collect();
    // What this computer took back (Take back): said at once.
    let taken_back: std::collections::BTreeSet<String> = cancels.into_iter().filter(|(device, _)| *device == here).map(|(_, key)| key).collect();
    let outcomes = rules::read_outcomes(&root);
    let phones = crate::texts::phones();
    let shared = |device: &str| phones.iter().find(|p| p.id == device).filter(|p| p.exported > 0).map(|p| p.exported * 1000);
    let time_of = |ms: i64| sioul_core::texts::time_of_day(ms, now);
    let sent = |phone: &str, dial: &str, since: i64| crate::texts::sent_to(phone, dial, since);
    let texts_from = |phone: &str| crate::texts::sends_texts(phone);
    let offer = rules::Offer { live: &live, asks: &own, outcomes: &outcomes, tr: crate::backend::tr(), taken_back: &taken_back, lines: all, texts_from: &texts_from, shared: &shared, time_of: &time_of, sent_texts: &sent };
    let by_id: std::collections::BTreeMap<&str, &Line> = all.iter().map(|l| (l.id.as_str(), l)).collect();
    let mut seen: Vec<String> = Vec::new();
    for line in lines.iter_mut() {
        let group: Vec<&Line> = line.ids.iter().filter_map(|id| by_id.get(id.as_str()).copied()).collect();
        line.answer = rules::answer_of(line, &group, &offer);
        if line.answer.as_ref().is_some_and(|a| rules::read_handed(&own, &outcomes, &a.phone, &a.notice)) {
            seen.extend(line.ids.iter().cloned());
        }
    }
    if !seen.is_empty() {
        if let Err(e) = calls::mark_seen(&root, &here, &seen, now_ms) {
            eprintln!("sioul: replies: {e}");
        }
        lines.retain(|l| !l.ids.iter().any(|id| seen.contains(id)));
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
        assert!(offered(&device, &posted, std::slice::from_ref(&line), &extra, true, now));
        assert!(!offered(&device, &posted, std::slice::from_ref(&line), &extra, true, now + 1_000), "the same offer");
        assert!(!offered(&device, &posted, &[Line { notice: String::new(), ..line.clone() }], &extra, true, now + 2_000), "an older line, no name");
        assert!(!offered(&device, &posted, &[], &extra, true, now + 3_000));
        let live = rules::read_live(&phonemsgs::folder(), now + 4_000);
        let said = live.get(&(device.clone(), line.notice.clone())).unwrap();
        assert_eq!((said.reply, said.read, said.talk.as_str(), said.app.as_str()), (true, false, "fedcba9876543210", "org.thoughtcrime.securesms"));
        // "Answer from your computers" switched off for the app: offered no more.
        assert!(offered(&device, &posted, std::slice::from_ref(&line), &extra, false, now + 5_000));
        assert!(!rules::read_live(&phonemsgs::folder(), now + 6_000).get(&(device.clone(), line.notice.clone())).unwrap().reply);
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
        // The listener relayed the line a minute before: the ledger's mark made then.
        rules::mark_ledger(&private, now - 60_000).unwrap();
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
        // Each computer's requests in turn (`replies::in_turn`): by key here.
        let states = |o: &[Outcome]| {
            let mut said = o.iter().map(|o| (o.key.clone(), o.state.clone(), o.reason.clone())).collect::<Vec<_>>();
            said.sort();
            said
        };
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
        let ledger = rules::open_ledger(&private, now).unwrap();
        assert_eq!((ledger.state("k1"), ledger.state("k4"), ledger.state("k3")), (Some("done"), Some("done"), Some("refused:not-counted")));
        let text = std::fs::read_to_string(&own).unwrap();
        assert!(!text.contains("Réponse") && !text.contains("Alice"), "no words, no names");
        // A decision whose word was lost (its file gone): said again at the next step, once.
        std::fs::remove_file(&own).unwrap();
        assert!(settle(&private, &own, now + 5 * 60_000));
        let again = rules::read_outcomes(&root);
        assert!(again.iter().any(|o| o.key == "k3" && o.state == "refused" && o.reason == "not-counted"), "{again:?}");
        assert!(!settle(&private, &own, now + 6 * 60_000), "once");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The first reply after the install, from an empty private folder: the
    /// listener relays the line (the ledger's mark made then, the only
    /// thing in the folder), the reply is written a minute and a half later,
    /// the phone's first step after it sends it: never "before-mark". Its
    /// notification gone when the listener came, the same words written again
    /// go too: the first never went.
    #[test]
    fn the_first_reply_after_install_goes_and_one_gone_can_go_again() {
        let base = std::env::temp_dir().join(format!("sioul-app-replies-first-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (root, private) = (base.join("phone-messages"), base.join("private"));
        let seal = ReplySeal::new(&[9u8; 32]);
        let t0 = now_ms() - 10 * 60_000;
        let line = Line { at: t0 - 60_000, id: format!("{}-aa", t0 - 60_000), app: "org.thoughtcrime.securesms".into(), kind: "chat".into(), talk: "fedcba9876543210".into(), notice: "0123456789abcdef".into(), name: "Alice".into(), key: "+33199001234".into(), who: "neutral".into(), text: "Tu passes ?".into(), shows: t0, ..Line::default() };
        assert!(!private.exists(), "an empty private folder");
        rules::mark_ledger(&private, t0).unwrap();
        phonemsgs::carry_into(&calls::own_file(&root, phonemsgs::LOG, "phone"), std::slice::from_ref(&line), t0).unwrap();
        let ask = |key: &str, at: i64| Ask { key: key.into(), phone: "phone".into(), verb: "reply".into(), notice: line.notice.clone(), app: line.app.clone(), line: line.id.clone(), words: "J'arrive.".into(), written: at, ..Ask::default() };
        rules::write_ask(&root, "desk", &ask("k1", t0 + 90_000), &seal).unwrap();
        let own = calls::own_file(&root, rules::OUTCOME, "phone");
        let counted = |device: &str| device == "desk";
        let roots = vec![root.clone()];
        // A phone's step, as `step` runs it on the phone's own folders.
        let step = |at: i64| {
            rules::mark_ledger(&private, at).unwrap();
            let decided = decide("phone", &rules::read_asks(&roots, &seal), &root, &private, &seal, &counted, &own, at);
            settle(&private, &own, at) || decided
        };
        step(t0 + 180_000);
        assert_eq!(rules::read_due(&private, &seal).iter().map(|d| d.key.as_str()).collect::<Vec<_>>(), ["k1"], "sent: {:?}", rules::read_outcomes(&root));
        assert!(matches!(rules::take(&private, "k1", &seal, t0 + 200_000).unwrap(), rules::Taken::Go(_)));
        rules::say(&private, &Done::of("k1", "gone", t0 + 201_000)).unwrap();
        step(t0 + 240_000);
        assert!(rules::read_outcomes(&root).iter().any(|o| o.key == "k1" && o.state == "gone"));
        // Her next message brings the notification back; the same words again: they go.
        rules::write_ask(&root, "desk", &ask("k2", t0 + 300_000), &seal).unwrap();
        step(t0 + 360_000);
        let said = rules::read_outcomes(&root);
        assert!(!said.iter().any(|o| o.key == "k2"), "{said:?}");
        assert!(rules::read_due(&private, &seal).iter().any(|d| d.key == "k2"));
        let _ = std::fs::remove_dir_all(&base);
    }

    fn copy_all(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                copy_all(&path, &to.join(entry.file_name()));
            } else {
                std::fs::copy(&path, to.join(entry.file_name())).unwrap();
            }
        }
    }

    /// A dose's step (`step`'s work, which a dose's alarm runs before its
    /// pull) with a runaway computer's 10,000 requests decided already and
    /// 150 claims the listener never took, settled "late" now: a measure,
    /// printed; run with `cargo test --release -p sioul-app -- --ignored
    /// a_dose_step --nocapture`, empty folders of its own.
    #[test]
    #[ignore]
    fn a_dose_step_with_a_runaway_backlog_stays_within_tens_of_milliseconds() {
        let base = std::env::temp_dir().join(format!("sioul-app-replies-dose-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let template = base.join("template");
        let (root, private) = (template.join("phone-messages"), template.join("private"));
        let seal = ReplySeal::new(&[9u8; 32]);
        let now = now_ms();
        rules::mark_ledger(&private, now - 86_400_000).unwrap();
        let line = Line { at: now - 600_000, id: format!("{}-aa", now - 600_000), app: "org.thoughtcrime.securesms".into(), kind: "chat".into(), talk: "fedcba9876543210".into(), notice: "0123456789abcdef".into(), name: "Alice".into(), key: "+33199001234".into(), who: "neutral".into(), text: "Tu passes ?".into(), shows: now - 600_000, ..Line::default() };
        phonemsgs::carry_into(&calls::own_file(&root, phonemsgs::LOG, "phone"), std::slice::from_ref(&line), now).unwrap();
        // The runaway computer's 10,000 Marks as read, sealed as `write_ask` writes them, all decided already.
        let mut file = String::new();
        let mut ends = Vec::new();
        for i in 0..10_000 {
            let key = format!("{i:032x}");
            let ask = Ask { key: key.clone(), phone: "phone".into(), verb: "read".into(), notice: line.notice.clone(), app: line.app.clone(), line: line.id.clone(), written: now - 20 * 60_000, writer: "runaway".into(), ..Ask::default() };
            file.push_str(&sioul_core::texts::Sealer::seal(&seal, &rules::padded_ask(&ask)));
            file.push('\n');
            ends.push(rules::end_of(&key, "expired", "", now - 10 * 60_000));
        }
        std::fs::create_dir_all(root.join(rules::ASK)).unwrap();
        std::fs::write(calls::own_file(&root, rules::ASK, "runaway"), file).unwrap();
        // Their words said already, as their step said them.
        let said: Vec<Outcome> = ends.iter().map(|c| Outcome { key: c.key.clone(), state: "expired".into(), at: c.at, ..Outcome::default() }).collect();
        rules::say_outcomes(&calls::own_file(&root, rules::OUTCOME, "phone"), &said).unwrap();
        // And 150 claims the listener never took, three minutes ago.
        ends.extend((0..150).map(|i| Claim { key: format!("c{i:031x}"), at: now - 3 * 60_000, state: "claimed".into(), to: line.notice.clone(), ..Claim::default() }));
        sioul_core::texts::record_all(&private, &ends).unwrap();
        let counted = |device: &str| device == "runaway";
        let mut times = Vec::new();
        for run in 0..7 {
            let at = base.join(format!("run-{run}"));
            copy_all(&template, &at);
            let (root, private) = (at.join("phone-messages"), at.join("private"));
            let own = calls::own_file(&root, rules::OUTCOME, "phone");
            // The step that decided them moved the marks past them (by their paths, this copy's).
            let (_, reads) = rules::read_asks_after(std::slice::from_ref(&root), &seal, &std::collections::BTreeMap::new());
            remember_reads("phone", &private, &reads, &std::collections::BTreeMap::new(), now);
            let started = std::time::Instant::now();
            rules::mark_ledger(&private, now).unwrap();
            let marks = rules::read_marks(&private);
            let (asked, reads) = rules::read_asks_after(std::slice::from_ref(&root), &seal, &marks);
            let read = started.elapsed();
            let decided = asked.0.iter().any(|a| a.phone == "phone") && decide("phone", &asked, &root, &private, &seal, &counted, &own, now);
            remember_reads("phone", &private, &reads, &marks, now);
            let settled = settle(&private, &own, now);
            let took = started.elapsed();
            assert!(!decided && settled, "nothing new decided; the 150 said late");
            assert!(asked.0.is_empty(), "the 10,000 decided never opened again");
            times.push((took.as_secs_f64() * 1000.0, read.as_secs_f64() * 1000.0));
            let late = rules::read_outcomes(&root).iter().filter(|o| o.state == "late").count();
            assert_eq!(late, 150);
        }
        times.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (median, max) = (times[times.len() / 2], times[times.len() - 1]);
        println!("a dose's step, 10,000 requests decided and 150 claims never taken: {:.1} ms median (reading the requests {:.1} ms), {:.1} ms at most", median.0, median.1, max.0);
        assert!(median.0 < 1_000.0, "{times:?}");
        let _ = std::fs::remove_dir_all(&base);
    }
}

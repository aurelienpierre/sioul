// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The call-back rules (docs/porch.md, "Calls"), each a switch of Porch ⚙ ▸
//! Calls, on unless said: a missed call from someone in your address books
//! makes a task, "Call back <name>"; a call of yours to them that did not
//! reach them (not answered, or shorter than the voicemail threshold), "Call
//! <name> again". Each goes in the usual list, for the call's day, tied to
//! their card, of the kind "call"; no reminder: tasks never nag.
//!
//! - **One open task per person**: another such call adds its time to the
//!   task's line ("also at 14:10"), never a count; the line is written again
//!   only while it is still the one the rules wrote (yours, once you changed
//!   it, stays).
//! - **It closes by itself** once you spoke with them after its first call:
//!   a call of theirs you answered, however short, or one of yours answered
//!   and at least as long as the voicemail threshold (`Call::spoke`), done at
//!   that call's end, as docs/tasks.md's
//!   "What closes by itself" does it: nothing said, nothing counted. A task
//!   you open again after that stays yours to close: no call closes it again.
//! - **Written once, by one device**: the phone that had the call, when it
//!   first reads it from its history (`calls::carry_history` gives each call
//!   once), so that two devices never make the same task; stamped with the
//!   call's own time, so that what it writes is the same whenever it writes.
//!   A call that came while its rule was off never makes a task later.
//!
//! A task the rules made says so in lines of its own, which other programs
//! keep: `X-SIOUL-CALL-BACK:<their card's address>`, `X-SIOUL-CALLS:<each
//! call, its start (ms) and way>` ("1793000000000-in,1793003600000-in"), and,
//! once closed by a call, `X-SIOUL-CALLED:<that call>`. Nothing of a phone
//! number is written in it: the card says who.

use crate::calls::{Call, Direction};
use crate::config::Config;
use crate::i18n::Translator;
use crate::lines;
use crate::tasks::{self, ContactRef, Status, TaskEdit};
use jiff::Zoned;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The line naming whom a task the rules made is for: their card's address.
pub const PERSON: &str = "X-SIOUL-CALL-BACK";
/// The line of the calls a task was made from, each its start and way.
pub const CALLS: &str = "X-SIOUL-CALLS";
/// The line of the call that closed a task, kept once it is opened again.
pub const CALLED: &str = "X-SIOUL-CALLED";

/// The rules as your settings say them (`[porch]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// A missed call makes "Call back …".
    pub missed: bool,
    /// A call of yours that did not reach them makes "Call … again".
    pub unreached: bool,
    /// The voicemail threshold (seconds).
    pub threshold: u32,
}

impl Rules {
    pub fn of(config: &Config) -> Rules {
        let threshold = config.porch.voicemail_seconds;
        Rules { missed: config.porch.call_back, unreached: config.porch.call_again, threshold: if threshold == 0 { crate::calls::VOICEMAIL_DEFAULT } else { threshold } }
    }
}

/// A call as a task's lines say it: when it began (ms), which way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mark {
    pub at: i64,
    pub out: bool,
}

impl Mark {
    pub fn of(call: &Call) -> Mark {
        Mark { at: call.at, out: call.direction == Direction::Outgoing }
    }

    /// "1793000000000-in".
    pub fn word(&self) -> String {
        format!("{}-{}", self.at, if self.out { "out" } else { "in" })
    }

    pub fn read(word: &str) -> Option<Mark> {
        let (at, way) = word.trim().split_once('-')?;
        let out = match way {
            "in" => false,
            "out" => true,
            _ => return None,
        };
        Some(Mark { at: at.parse().ok()?, out })
    }
}

/// A task the rules made, as its file holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub file: PathBuf,
    pub text: String,
    pub uid: String,
    /// Whom it is for (`PERSON`): their card's address.
    pub person: String,
    /// Not done, not dropped.
    pub open: bool,
    /// Closed once by a call (`CALLED`): open again, it is yours to close.
    pub called: bool,
    /// Its calls (`CALLS`), oldest first.
    pub calls: Vec<Mark>,
}

impl Kept {
    /// A task file's text, when the rules made it (it has a `PERSON` line).
    pub fn read(file: &Path, text: &str, zone: &TimeZone) -> Option<Kept> {
        if !text.contains(PERSON) {
            return None;
        }
        let unfolded = lines::unfold(text);
        let value = |name: &str| unfolded.iter().find(|l| lines::name(l) == name).map(|l| lines::unescape(lines::value(l).trim()));
        let person = value(PERSON).filter(|p| !p.is_empty())?;
        let task = tasks::task_of_text(text, zone)?;
        let mut calls: Vec<Mark> = value(CALLS).map(|v| v.split(',').filter_map(Mark::read).collect()).unwrap_or_default();
        calls.sort();
        Some(Kept {
            file: file.to_path_buf(),
            text: text.to_string(),
            uid: task.uid,
            person,
            open: matches!(task.status, Status::NeedsAction | Status::InProcess),
            called: value(CALLED).is_some(),
            calls,
        })
    }
}

/// What the rules write: a new task (its UID and its text, for the usual
/// list), or a task's file as it becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Write {
    New { uid: String, text: String },
    Changed { file: PathBuf, text: String },
}

/// What this phone's new calls (`calls`, oldest first, each with their
/// card when your address books have the number) make of the tasks the
/// rules made (`kept`), each call in turn: a missed call (`Rules::missed`)
/// or a call of yours that did not reach them (`Rules::unreached`) opens a
/// task, or adds its time to their open one; a call in which you spoke
/// closes their open one, made before it. Nothing for a call with nobody of
/// your address books. A task that cannot be written is left as it is.
pub fn apply(calls: &[Call], kept: &[Kept], rules: &Rules, tr: &Translator, zone: &TimeZone) -> Vec<Write> {
    let mut tasks: Vec<Kept> = kept.to_vec();
    let mut made: BTreeSet<String> = BTreeSet::new();
    let mut changed: BTreeSet<String> = BTreeSet::new();
    for call in calls {
        let Some(contact) = call.contact.as_ref().filter(|c| !c.uri.trim().is_empty()) else { continue };
        let open = tasks.iter_mut().find(|k| k.open && k.person == contact.uri);
        let asks = (rules.missed && call.missed()) || (rules.unreached && call.not_reached(rules.threshold));
        if asks {
            match open {
                Some(task) => {
                    if task.calls.contains(&Mark::of(call)) {
                        continue;
                    }
                    if let Ok(text) = with_call(task, call, tr, zone) {
                        task.text = text;
                        task.calls.push(Mark::of(call));
                        task.calls.sort();
                        changed.insert(task.uid.clone());
                    }
                }
                None => {
                    if let Ok(task) = new_task(call, contact, tr, zone) {
                        made.insert(task.uid.clone());
                        tasks.push(task);
                    }
                }
            }
        } else if call.spoke(rules.threshold)
            && let Some(task) = open
            && !task.called
            && task.calls.first().is_some_and(|first| first.at < call.at)
            && let Ok(text) = closed(task, call, zone)
        {
            task.text = text;
            task.open = false;
            task.called = true;
            changed.insert(task.uid.clone());
        }
    }
    tasks
        .into_iter()
        .filter_map(|task| {
            if made.contains(&task.uid) {
                Some(Write::New { uid: task.uid, text: task.text })
            } else if changed.contains(&task.uid) {
                Some(Write::Changed { file: task.file, text: task.text })
            } else {
                None
            }
        })
        .collect()
}

/// A call's start in `zone`.
fn zoned(at_ms: i64, zone: &TimeZone) -> Zoned {
    jiff::Timestamp::from_millisecond(at_ms).unwrap_or(jiff::Timestamp::UNIX_EPOCH).to_zoned(zone.clone())
}

/// The task a call opens: "Call back Noa" for a missed call, "Call Noa
/// again" for one of yours; for its day, its UID made of the person and the
/// call, so that it is the same whenever it is made.
fn new_task(call: &Call, contact: &ContactRef, tr: &Translator, zone: &TimeZone) -> Result<Kept, String> {
    let at = zoned(call.at, zone);
    let mark = Mark::of(call);
    let uid = format!("sioul-call-{}-{}", crate::notices::digest(&contact.uri), call.at);
    let mut args = crate::i18n::args();
    args.set("name", contact.name.trim().to_string());
    let title = tr.text(if mark.out { "call-task-again" } else { "call-task-back" }, Some(&args));
    let due = at.date();
    let edit = TaskEdit { title, notes: line(&[mark], due, tr, zone), due: due.to_string(), kind: "call".into(), contacts: vec![contact.clone()], ..TaskEdit::default() };
    let text = tasks::new_task(&edit, &uid, zone, &at)?;
    let text = tasks::add_lines(&text, &[format!("{PERSON}:{}", lines::escape(&contact.uri)), format!("{CALLS}:{}", mark.word())], &at)?;
    Ok(Kept { file: PathBuf::new(), text, uid, person: contact.uri.clone(), open: true, called: false, calls: vec![mark] })
}

/// An open task with one more call: its calls' line, and its own line
/// while it is still the one the rules wrote; stamped with the call's time.
fn with_call(task: &Kept, call: &Call, tr: &Translator, zone: &TimeZone) -> Result<String, String> {
    let at = zoned(call.at, zone);
    let read = tasks::task_of_text(&task.text, zone).ok_or("no task")?;
    let first = task.calls.first().map_or(call.at, |m| m.at);
    let due = read.due.get(..10).and_then(|d| d.parse::<Date>().ok()).unwrap_or_else(|| zoned(first, zone).date());
    let before = line(&task.calls, due, tr, zone);
    let mut marks = task.calls.clone();
    marks.push(Mark::of(call));
    marks.sort();
    let ours = task.calls.is_empty() || read.notes.trim() == before.trim();
    let mut added = vec![format!("{CALLS}:{}", marks.iter().map(Mark::word).collect::<Vec<_>>().join(","))];
    if ours {
        added.push(format!("DESCRIPTION:{}", lines::escape(&line(&marks, due, tr, zone))));
    }
    tasks::replace_lines(
        &task.text,
        |l| {
            let name = lines::name(l);
            name == CALLS || (ours && name == "DESCRIPTION")
        },
        added,
        &at,
    )
}

/// An open task done by a call in which you spoke: done at the call's end,
/// the call named (`CALLED`).
fn closed(task: &Kept, call: &Call, zone: &TimeZone) -> Result<String, String> {
    let end = zoned(call.at + call.seconds.max(0) * 1000, zone);
    let done = tasks::set_status(&task.text, Status::Completed, zone, &end)?;
    tasks::add_lines(&done, &[format!("{CALLED}:{}", Mark::of(call).word())], &end)
}

/// The task's line: its first call, then each other one ("also at 14:10";
/// a call the other way, "they called at 15:02", "you called at 15:02"),
/// a call of another day than the task's with its day.
pub fn line(marks: &[Mark], due: Date, tr: &Translator, zone: &TimeZone) -> String {
    let Some(first) = marks.first() else { return String::new() };
    let when = |m: &Mark| {
        let at = zoned(m.at, zone);
        let mut args = crate::i18n::args();
        args.set("time", at.strftime("%H:%M").to_string());
        if at.date() == due {
            tr.text("call-task-at", Some(&args))
        } else {
            args.set("day", tr.day_short(at.date()));
            tr.text("call-task-on", Some(&args))
        }
    };
    let said = |id: &str, m: &Mark| {
        let mut args = crate::i18n::args();
        args.set("when", when(m));
        tr.text(id, Some(&args))
    };
    let mut parts = vec![said(if first.out { "call-task-unreached" } else { "call-task-missed" }, first)];
    for m in &marks[1..] {
        let id = match (m.out == first.out, m.out) {
            (true, _) => "call-task-also",
            (false, true) => "call-task-you-called",
            (false, false) => "call-task-they-called",
        };
        parts.push(said(id, m));
    }
    format!("{}.", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calls::{HistoryRow, Logged};
    use crate::notices::State;

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    /// 4 November 2026 at a time, in Paris (ms).
    fn at(hhmm: &str) -> i64 {
        format!("2026-11-04T{hhmm}:00+01:00[Europe/Paris]").parse::<Zoned>().unwrap().timestamp().as_millisecond()
    }

    fn noa() -> ContactRef {
        ContactRef { name: "Noa Linden".into(), uri: "sioul:contact/noa-1".into() }
    }

    /// A call of the phone's history: Android's row (`kind`, its type), with
    /// Noa's card for the number 01 99 00 12 34, none for any other.
    fn call(hhmm: &str, kind: i32, seconds: i64, number: &str) -> Call {
        let row = HistoryRow { at: at(hhmm), seconds, kind, number: number.into(), presentation: 1 };
        let logged = Logged { device: "phone-1".into(), ..Logged::of(&row, crate::phones::region_named("FR")).unwrap() };
        let contact = (logged.key == "+33199001234").then(noa);
        Call::of(&logged, contact, State::default())
    }

    const NOA: &str = "01 99 00 12 34";

    fn rules() -> Rules {
        Rules { missed: true, unreached: true, threshold: 90 }
    }

    fn tr() -> Translator {
        Translator::new("en")
    }

    /// The writes as the tasks they leave: each new one with no file yet.
    fn kept_after(writes: &[Write], kept: &[Kept]) -> Vec<Kept> {
        let mut out: Vec<Kept> = kept.to_vec();
        for write in writes {
            match write {
                Write::New { uid, text } => out.push(Kept::read(Path::new(&format!("{uid}.ics")), text, &paris()).unwrap()),
                Write::Changed { file, text } => {
                    let task = Kept::read(file, text, &paris()).unwrap();
                    out.retain(|k| k.uid != task.uid);
                    out.push(task);
                }
            }
        }
        out
    }

    /// A call from the phone's history, as Android's types say it: missed
    /// (or declined by hand), not reached (not answered, or shorter than the
    /// threshold), spoken (a call of theirs answered, however short; one of
    /// yours answered and at least the threshold); never a voicemail's row
    /// nor a blocked call.
    #[test]
    fn calls_are_said_from_the_phone_s_history() {
        let fr = crate::phones::region_named("FR");
        let missed = call("09:30", 3, 0, NOA);
        assert!(missed.missed() && !missed.not_reached(90) && !missed.spoke(90));
        assert_eq!(missed.id, format!("{}-+33199001234-in", at("09:30")));
        assert_eq!(missed.notice(), format!("call:phone-1:{}-+33199001234-in", at("09:30")));
        assert!(call("09:31", 5, 0, NOA).missed(), "declined by hand on the phone");
        let unanswered = call("10:00", 2, 0, NOA);
        assert!(unanswered.not_reached(90) && !unanswered.answered && unanswered.direction == Direction::Outgoing);
        let voicemail = call("10:05", 2, 40, NOA);
        assert!(voicemail.answered && voicemail.not_reached(90) && voicemail.not_reached(60) && !voicemail.not_reached(30));
        let spoken = call("10:10", 2, 95, NOA);
        assert!(spoken.spoke(90) && !spoken.not_reached(90) && !spoken.spoke(120));
        let taken = call("10:20", 1, 300, NOA);
        assert!(taken.spoke(90) && !taken.missed());
        assert!(call("10:25", 1, 40, NOA).spoke(90), "a call of theirs you answered, however short: you picked up");
        assert!(!call("10:30", 7, 0, NOA).missed(), "answered on another device of the line");
        // Not calls to say: a voicemail's row, a blocked call (the log has Sioul's).
        for kind in [4, 6, 9] {
            assert!(Logged::of(&HistoryRow { at: at("11:00"), kind, number: NOA.into(), presentation: 1, ..HistoryRow::default() }, fr).is_none());
        }
        // A hidden number: no key, no person.
        let hidden = Call::of(&Logged::of(&HistoryRow { at: at("11:00"), kind: 3, number: String::new(), presentation: 2, ..HistoryRow::default() }, fr).unwrap(), None, State::default());
        assert!(hidden.hidden && hidden.number.is_empty() && hidden.person().is_none() && hidden.id.ends_with("-hidden-in"));
        assert_eq!(missed.person().as_deref(), Some("sioul:contact/noa-1"));
        assert_eq!(call("11:00", 3, 0, "04 65 71 00 00").person().as_deref(), Some("+33465710000"), "nobody's card: their number");
    }

    /// Rule 1: a missed call from someone in your address books makes "Call
    /// back …", for its day, tied to their card, of the kind "call", with no
    /// reminder; nothing for a number in nobody's card, nor with the rule off.
    #[test]
    fn a_missed_call_makes_a_task_to_call_back() {
        let writes = apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris());
        let [Write::New { uid, text }] = writes.as_slice() else { panic!("{writes:?}") };
        assert_eq!(*uid, format!("sioul-call-{}-{}", crate::notices::digest("sioul:contact/noa-1"), at("09:30")));
        let task = tasks::task_of_text(text, &paris()).unwrap();
        assert_eq!(task.title, "Call back Noa Linden");
        assert_eq!(task.notes, "Missed call at 09:30.");
        assert_eq!(task.due.get(..10), Some("2026-11-04"));
        assert_eq!(task.kind, "call");
        assert_eq!(task.contacts, vec![noa()]);
        assert!(!text.contains("BEGIN:VALARM"), "tasks never nag");
        assert!(text.contains("X-SIOUL-CALL-BACK:sioul:contact/noa-1") && text.contains(&format!("X-SIOUL-CALLS:{}-in", at("09:30"))));
        assert!(!text.contains("99 00") && !text.contains("199001234"), "no number in the task");
        // Stamped with the call: the same bytes whenever it is made.
        assert_eq!(apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris()), writes);
        assert!(apply(&[call("09:30", 3, 0, "04 65 71 00 00")], &[], &rules(), &tr(), &paris()).is_empty(), "in nobody's card");
        assert!(apply(&[call("09:30", 3, 0, NOA)], &[], &Rules { missed: false, ..rules() }, &tr(), &paris()).is_empty(), "the rule off");
    }

    /// Rule 2: a call of yours that did not reach them makes "Call … again":
    /// not answered, or shorter than the voicemail threshold (90 s, or as
    /// set); nothing once you spoke, nor with the rule off.
    #[test]
    fn a_call_that_did_not_reach_them_makes_a_task_to_call_again() {
        for (seconds, threshold, makes) in [(0, 90, true), (40, 90, true), (95, 90, false), (95, 120, true), (65, 60, false)] {
            let writes = apply(&[call("14:10", 2, seconds, NOA)], &[], &Rules { threshold, ..rules() }, &tr(), &paris());
            assert_eq!(writes.len(), usize::from(makes), "{seconds} s, threshold {threshold}");
            if let [Write::New { text, .. }] = writes.as_slice() {
                let task = tasks::task_of_text(text, &paris()).unwrap();
                assert_eq!((task.title.as_str(), task.notes.as_str()), ("Call Noa Linden again", "Your call at 14:10 did not reach them."));
            }
        }
        assert!(apply(&[call("14:10", 2, 0, NOA)], &[], &Rules { unreached: false, ..rules() }, &tr(), &paris()).is_empty());
    }

    /// One open task per person: another such call adds its time to the
    /// task's line, either rule, never a count; a line you wrote yourself
    /// stays yours; once the task is done, the next call makes a new one.
    #[test]
    fn one_open_task_per_person() {
        let first = apply(&[call("09:30", 3, 0, NOA), call("09:41", 3, 0, NOA)], &[], &rules(), &tr(), &paris());
        assert_eq!(first.len(), 1, "{first:?}");
        let kept = kept_after(&first, &[]);
        assert_eq!(tasks::task_of_text(&kept[0].text, &paris()).unwrap().notes, "Missed call at 09:30, also at 09:41.");
        // Later, your call that did not reach them: the same task, its way said.
        let later = apply(&[call("14:10", 2, 0, NOA)], &kept, &rules(), &tr(), &paris());
        let kept = kept_after(&later, &kept);
        assert_eq!(kept.len(), 1);
        let task = tasks::task_of_text(&kept[0].text, &paris()).unwrap();
        assert_eq!((task.title.as_str(), task.notes.as_str()), ("Call back Noa Linden", "Missed call at 09:30, also at 09:41, you called at 14:10."));
        assert_eq!(kept[0].calls.len(), 3);
        // The same call read again: nothing.
        assert!(apply(&[call("14:10", 2, 0, NOA)], &kept, &rules(), &tr(), &paris()).is_empty());
        // The next day: its day said.
        let tomorrow = Call { at: at("09:30") + 86_400_000, id: "next".into(), ..call("09:30", 3, 0, NOA) };
        let next = kept_after(&apply(&[tomorrow], &kept, &rules(), &tr(), &paris()), &kept);
        assert_eq!(tasks::task_of_text(&next[0].text, &paris()).unwrap().notes, "Missed call at 09:30, also at 09:41, you called at 14:10, also on Thu 5 Nov at 09:30.");
        // A line you wrote: kept; the calls' own line still grows.
        let yours = Kept { text: tasks::replace_lines(&kept[0].text, |l| lines::name(l) == "DESCRIPTION", vec!["DESCRIPTION:Ask about Saturday".into()], &zoned(at("15:00"), &paris())).unwrap(), ..kept[0].clone() };
        let after = kept_after(&apply(&[call("16:00", 3, 0, NOA)], &[yours], &rules(), &tr(), &paris()), &[]);
        assert_eq!(tasks::task_of_text(&after[0].text, &paris()).unwrap().notes, "Ask about Saturday");
        assert_eq!(after[0].calls.len(), 4);
        // Done by hand: the next missed call makes a new task.
        let done = Kept { text: tasks::set_status(&kept[0].text, Status::Completed, &paris(), &zoned(at("15:00"), &paris())).unwrap(), open: false, ..kept[0].clone() };
        let again = apply(&[call("16:00", 3, 0, NOA)], &[done], &rules(), &tr(), &paris());
        assert!(matches!(again.as_slice(), [Write::New { .. }]), "{again:?}");
    }

    /// The task closes by itself once you spoke with them after its first
    /// call (a call of theirs you answered, however short; one of yours at
    /// least the threshold): done at that call's end, the call named, the
    /// same bytes whenever; never by a call before it, nor a short one of
    /// yours, and never again once you opened it again.
    #[test]
    fn the_task_closes_by_itself_once_you_spoke() {
        let kept = kept_after(&apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris()), &[]);
        assert!(apply(&[call("10:00", 2, 40, NOA)], &kept, &rules(), &tr(), &paris()).iter().all(|w| matches!(w, Write::Changed { .. })), "short: the task grows, stays open");
        assert!(apply(&[Call { at: at("09:00"), ..call("09:00", 1, 300, NOA) }], &kept, &rules(), &tr(), &paris()).is_empty(), "a call before the task's first");
        for spoken in [call("11:00", 2, 95, NOA), call("11:00", 1, 300, NOA)] {
            let writes = apply(std::slice::from_ref(&spoken), &kept, &rules(), &tr(), &paris());
            let [Write::Changed { text, .. }] = writes.as_slice() else { panic!("{writes:?}") };
            let task = tasks::task_of_text(text, &paris()).unwrap();
            assert_eq!(task.status, Status::Completed);
            let end = zoned(spoken.at + spoken.seconds * 1000, &paris());
            assert_eq!(task.completed, Some(end.timestamp().as_second()));
            assert!(text.contains(&format!("X-SIOUL-CALLED:{}", Mark::of(&spoken).word())));
            assert_eq!(apply(std::slice::from_ref(&spoken), &kept, &rules(), &tr(), &paris()), writes, "the same bytes");
        }
        // A call of theirs you answered closes it, however short: you picked up.
        let writes = apply(&[call("10:30", 1, 40, NOA)], &kept, &rules(), &tr(), &paris());
        let [Write::Changed { text, .. }] = writes.as_slice() else { panic!("{writes:?}") };
        assert_eq!(tasks::task_of_text(text, &paris()).unwrap().status, Status::Completed, "answered 0:40");
        // Opened again after a call closed it: yours to close; a new missed call still joins it.
        let closed = kept_after(&apply(&[call("11:00", 1, 300, NOA)], &kept, &rules(), &tr(), &paris()), &kept);
        let reopened = Kept { text: tasks::set_status(&closed[0].text, Status::NeedsAction, &paris(), &zoned(at("12:00"), &paris())).unwrap(), open: true, ..closed[0].clone() };
        assert!(reopened.called);
        assert!(apply(&[call("13:00", 1, 300, NOA)], &[reopened.clone()], &rules(), &tr(), &paris()).is_empty(), "never closed again by a call");
        let joined = apply(&[call("13:30", 3, 0, NOA)], &[reopened], &rules(), &tr(), &paris());
        assert!(matches!(joined.as_slice(), [Write::Changed { .. }]), "{joined:?}");
    }

    #[test]
    fn a_task_s_calls_read_back() {
        assert_eq!(Mark::read("1793000000000-in"), Some(Mark { at: 1_793_000_000_000, out: false }));
        assert_eq!(Mark::read("1793000000000-out").map(|m| m.word()).as_deref(), Some("1793000000000-out"));
        assert_eq!(Mark::read("1793000000000"), None);
        assert_eq!(Mark::read("x-in"), None);
    }
}

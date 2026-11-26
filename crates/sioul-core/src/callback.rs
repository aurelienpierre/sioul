// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The tasks a call makes (docs/porch.md, "Calls"; docs/rules.md, "Calls"),
//! as the rules ask for them (`wiring::call`): the presets "missed-call-task"
//! and "call-closes-task" (Settings ▸ Rules ▸ Presets, each on unless said),
//! and your own rules copied from them. A missed call from someone in your
//! address books makes a task, `Call back <name>`; a call of yours to them
//! that did not reach them (not answered, or shorter than the voicemail
//! threshold), `Call <name> again`. Each goes in the usual list, for the
//! call's day, tied to their card, of the kind "call"; no reminder: tasks
//! never nag.
//!
//! - **One open task per person**: another such call adds its time to the
//!   task's line ("also at 14:10"), never a count; the line is written again
//!   only while it is still the one Sioul wrote (yours, once you changed it,
//!   stays).
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
//! A task made so says it in lines of its own, which other programs keep,
//! and which every Sioul since 0.0.12 reads the same way:
//! `X-SIOUL-CALL-BACK:<their card's address>`, `X-SIOUL-CALLS:<each call,
//! its start (ms) and way>` ("1793000000000-in,1793003600000-in"), and, once
//! closed by a call, `X-SIOUL-CALLED:<that call>`. Nothing of a phone number
//! is written in it: the card says who. Its calls' lines are how its card
//! lists the calls it was made from (`ties`), each by its address. Its
//! `CONTACT` names the card as every task's does since 26 November 2026,
//! `urn:uuid:<UID>` (`tasks::contact_line`); its `X-SIOUL-CALL-BACK` keeps
//! Sioul's own address (`sioul:contact/<UID>`), which older devices match
//! exactly, and is read in either form (`same_person`).

use crate::calls::{Call, Direction};
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

/// The start of every such task's UID: `sioul-call-<their card's digest>-<its first call (ms)>`.
pub const UID_PREFIX: &str = "sioul-call-";

/// A call as a task's lines say it: when it began (ms), which way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
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
    /// That call, when its line reads.
    pub closed_by: Option<Mark>,
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
            closed_by: value(CALLED).and_then(|v| Mark::read(&v)),
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

/// What a call did to the tasks made so (`Book::call_back`): a task made, or
/// the call's time added to their open one; each by the task's UID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Did {
    Made(String),
    Joined(String),
}

/// The tasks made so (`Kept`), as one call after another changes them: what
/// the rules ask of each call (`wiring::call::run`), in turn, then what to
/// write (`writes`). A task that cannot be written is left as it is.
#[derive(Debug, Clone, Default)]
pub struct Book {
    tasks: Vec<Kept>,
    made: BTreeSet<String>,
    changed: BTreeSet<String>,
}

impl Book {
    /// The tasks made so, as their files hold them now.
    pub fn of(kept: &[Kept]) -> Book {
        Book { tasks: kept.to_vec(), ..Book::default() }
    }

    /// A call that asks for a task (missed, or of yours that did not reach
    /// them): their open task gets its time, else a task is made. Nothing for
    /// a call with nobody of your address books, nor one already in their
    /// task. What it did.
    pub fn call_back(&mut self, call: &Call, tr: &Translator, zone: &TimeZone) -> Option<Did> {
        let contact = call.contact.as_ref().filter(|c| !c.uri.trim().is_empty())?;
        match self.tasks.iter_mut().find(|k| k.open && same_person(&k.person, &contact.uri)) {
            Some(task) => {
                if task.calls.contains(&Mark::of(call)) {
                    return None;
                }
                let text = with_call(task, call, tr, zone).ok()?;
                task.text = text;
                task.calls.push(Mark::of(call));
                task.calls.sort();
                self.changed.insert(task.uid.clone());
                Some(Did::Joined(task.uid.clone()))
            }
            None => {
                let task = new_task(call, contact, tr, zone).ok()?;
                let uid = task.uid.clone();
                self.made.insert(uid.clone());
                self.tasks.push(task);
                Some(Did::Made(uid))
            }
        }
    }

    /// A call that closes their task: their open one, made before it, done
    /// at the call's end; never one a call closed once and you opened again.
    /// The task it closed, by its UID.
    pub fn close(&mut self, call: &Call, zone: &TimeZone) -> Option<String> {
        let contact = call.contact.as_ref().filter(|c| !c.uri.trim().is_empty())?;
        let task = self.tasks.iter_mut().find(|k| k.open && same_person(&k.person, &contact.uri))?;
        if task.called || !task.calls.first().is_some_and(|first| first.at < call.at) {
            return None;
        }
        let text = closed(task, call, zone).ok()?;
        task.text = text;
        task.open = false;
        task.called = true;
        task.closed_by = Some(Mark::of(call));
        self.changed.insert(task.uid.clone());
        Some(task.uid.clone())
    }

    /// A task's text as it stands now in the book (what is written), by its UID.
    pub fn text(&self, uid: &str) -> Option<&str> {
        self.tasks.iter().find(|k| k.uid == uid).map(|k| k.text.as_str())
    }

    /// What to write: each task made (its UID and text, for the usual list),
    /// each task changed (its file and its text).
    pub fn writes(self) -> Vec<Write> {
        let (made, changed) = (self.made, self.changed);
        self.tasks
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
}

/// Whether a task's `PERSON` line names whom a call was with: the same
/// address, or the same card in either form (`contacts::same_card`): Sioul
/// writes Sioul's own address there (`sioul:contact/<UID>`), and reads a
/// card's `urn:uuid:` as well, as a task's `CONTACT` holds it since 26
/// November 2026.
fn same_person(person: &str, contact: &str) -> bool {
    person.trim() == contact.trim() || crate::contacts::same_card(person, contact)
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
    let uid = format!("{UID_PREFIX}{}-{}", crate::notices::digest(&contact.uri), call.at);
    let mut args = crate::i18n::args();
    args.set("name", contact.name.trim().to_string());
    let title = tr.text(if mark.out { "call-task-again" } else { "call-task-back" }, Some(&args));
    let due = at.date();
    let edit = TaskEdit { title, notes: line(&[mark], due, tr, zone), due: due.to_string(), kind: "call".into(), contacts: vec![contact.clone()], ..TaskEdit::default() };
    let text = tasks::new_task(&edit, &uid, zone, &at)?;
    let text = tasks::add_lines(&text, &[format!("{PERSON}:{}", lines::escape(&contact.uri)), format!("{CALLS}:{}", mark.word())], &at)?;
    Ok(Kept { file: PathBuf::new(), text, uid, person: contact.uri.clone(), open: true, called: false, closed_by: None, calls: vec![mark] })
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

    /// 4 November 2026 at a time, in Paris (ms).
    fn at(hhmm: &str) -> i64 {
        format!("2026-11-04T{hhmm}:00+01:00[Europe/Paris]").parse::<Zoned>().unwrap().timestamp().as_millisecond()
    }

    fn noa() -> tasks::ContactRef {
        tasks::ContactRef { name: "Noa Linden".into(), uri: "sioul:contact/noa-1".into() }
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

    /// The task a missed call makes names the card as every task does since
    /// 26 November 2026 (`CONTACT;ALTREP="urn:uuid:<UID>"`) and keeps Sioul's
    /// own address on its `X-SIOUL-CALL-BACK` line, which every device
    /// matches. Whom a task is for reads in either form: the next call joins
    /// it and the call in which you spoke closes it, whichever form its lines
    /// have; an older task, written again for its call, names its card the
    /// new way.
    #[test]
    fn whom_a_task_is_for_reads_in_either_form() {
        let tr = Translator::new("en");
        let zone = TimeZone::get("Europe/Paris").unwrap();
        let mut book = Book::default();
        let Some(Did::Made(uid)) = book.call_back(&call("09:30", 3, 0, NOA), &tr, &zone) else { panic!("no task made") };
        let text = book.text(&uid).unwrap().to_string();
        assert!(text.contains("CONTACT;ALTREP=\"urn:uuid:noa-1\":Noa Linden\r\n") && !text.contains("ALTREP=\"sioul:"), "{text}");
        assert!(text.contains("X-SIOUL-CALL-BACK:sioul:contact/noa-1\r\n"), "{text}");
        assert_eq!(uid, format!("{UID_PREFIX}{}-{}", crate::notices::digest("sioul:contact/noa-1"), at("09:30")), "its UID as before: the same on every device");
        let older = text.replace("ALTREP=\"urn:uuid:noa-1\"", "ALTREP=\"sioul:contact/noa-1\"");
        for (person, written) in [("sioul:contact/noa-1", &older), ("urn:uuid:noa-1", &text)] {
            let written = written.replace("X-SIOUL-CALL-BACK:sioul:contact/noa-1", &format!("X-SIOUL-CALL-BACK:{person}"));
            let kept = Kept::read(Path::new("noa.ics"), &written, &zone).unwrap();
            assert_eq!(kept.person, person);
            let mut book = Book::of(&[kept]);
            assert_eq!(book.call_back(&call("11:00", 3, 0, NOA), &tr, &zone), Some(Did::Joined(uid.clone())), "{person}");
            let joined = book.text(&uid).unwrap().to_string();
            assert!(joined.contains("CONTACT;ALTREP=\"urn:uuid:noa-1\":Noa Linden\r\n") && !joined.contains("ALTREP=\"sioul:"), "{joined}");
            assert_eq!(joined.matches("CONTACT").count(), 1, "{joined}");
            assert_eq!(book.close(&call("12:00", 1, 120, NOA), &zone), Some(uid.clone()), "{person}");
        }
    }

    #[test]
    fn a_task_s_calls_read_back() {
        assert_eq!(Mark::read("1793000000000-in"), Some(Mark { at: 1_793_000_000_000, out: false }));
        assert_eq!(Mark::read("1793000000000-out").map(|m| m.word()).as_deref(), Some("1793000000000-out"));
        assert_eq!(Mark::read("1793000000000"), None);
        assert_eq!(Mark::read("x-in"), None);
    }
}

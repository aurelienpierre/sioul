// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A task made an appointment, an appointment made a task (21 October 2026).
//! An item's kind is the person's to say, never guessed from its times: a
//! task may have a time and a length and still be a task. What one says the
//! other keeps: its title, place, notes, repeat, margins and costs; a time
//! becomes the event's, or the task's time block (`blocks`). The files are
//! swapped by `vdir::replace_items`: never both, never neither.

use crate::agenda::EventEdit;
use crate::tasks::{Status, Task, TaskEdit};
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

/// An appointment made from a task without a length lasts an hour.
pub const HOUR_MINUTES: i64 = 60;

/// The longest estimate a task takes (`TaskEdit::estimate`, as the form and the MCP cap it).
const LONGEST: i64 = 6000;

fn local(seconds: i64, zone: &TimeZone) -> Option<Zoned> {
    Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()))
}

fn minute_text(z: &Zoned) -> String {
    z.strftime("%Y-%m-%dT%H:%M").to_string()
}

/// When the appointment a task becomes takes place, Unix seconds: its time
/// block's time (`block`), else the time it was given by a drag before blocks,
/// else its start when that has a time, for its estimate or an hour. None: it
/// has no time, the form asks one.
pub fn time_of(task: &Task, block: Option<(i64, i64)>, zone: &TimeZone) -> Option<(i64, i64)> {
    if let Some((start, end)) = block.filter(|(s, e)| e > s) {
        return Some((start, end));
    }
    let length = if task.estimate > 0 { i64::from(task.estimate) } else { HOUR_MINUTES };
    [task.at.as_str(), task.start.as_str()].into_iter().filter(|t| t.contains('T')).find_map(|text| {
        let start = text.parse::<DateTime>().ok()?.to_zoned(zone.clone()).ok()?.timestamp().as_second();
        Some((start, start + length * 60))
    })
}

/// The event form of a task made an appointment: at `time` (`time_of`);
/// without one, on its day to start, else its date asked, else `today`, at
/// 9:00 for an hour, for the person to say.
pub fn event_of(task: &Task, time: Option<(i64, i64)>, today: Date, zone: &TimeZone) -> EventEdit {
    let (start, end) = match time.and_then(|(s, e)| Some((local(s, zone)?, local(e, zone)?))) {
        Some((start, end)) => (minute_text(&start), minute_text(&end)),
        None => {
            let day = task.start_date().or(task.due_date()).unwrap_or(today);
            (format!("{day}T09:00"), format!("{day}T10:00"))
        }
    };
    EventEdit {
        title: task.title.clone(),
        location: task.location.clone(),
        notes: task.notes.clone(),
        start,
        end,
        all_day: false,
        repeat: task.repeat.clone(),
        margins: task.margins,
        demands: task.demands,
        remind: String::new(),
    }
}

/// Minutes between two form times ("2026-10-05T09:00"); None for days or nonsense.
fn minutes_between(start: &str, end: &str) -> Option<i64> {
    let (start, end) = (start.parse::<DateTime>().ok()?, end.parse::<DateTime>().ok()?);
    let span = end.since(start).ok()?;
    Some(span.total(jiff::Unit::Minute).ok()? as i64)
}

/// The task an appointment becomes: its title, place and notes; its day as
/// the day it can start; its length as its estimate (none for a whole day);
/// its repeat, margins and costs. Its time is its block's, made beside it.
pub fn task_of(event: &EventEdit) -> TaskEdit {
    let estimate = if event.all_day { 0 } else { minutes_between(&event.start, &event.end).unwrap_or(0).clamp(0, LONGEST) };
    TaskEdit {
        title: event.title.clone(),
        notes: event.notes.clone(),
        location: event.location.clone(),
        status: Status::NeedsAction,
        start: event.start.get(..10).unwrap_or_default().to_string(),
        estimate: u32::try_from(estimate).unwrap_or(0),
        repeat: event.repeat.clone(),
        margins: event.margins,
        demands: event.demands,
        ..TaskEdit::default()
    }
}

/// What a task holds that an appointment cannot, by word ids
/// (`appointment-lost-*`): said before it becomes one (the review of 22
/// October 2026, F5). Its title, place, notes, repeat, margins, costs and
/// links go along.
pub fn task_losses(task: &Task) -> Vec<&'static str> {
    let mut lost = Vec::new();
    for (has, word) in [
        (!task.due.is_empty(), "due"),
        (task.priority > 0, "priority"),
        (!task.categories.is_empty(), "tags"),
        (!task.contacts.is_empty(), "contacts"),
        (!task.kind.is_empty(), "kind"),
        (!task.projects.is_empty(), "projects"),
    ] {
        if has {
            lost.push(word);
        }
    }
    lost
}

/// What an event holds that a task cannot, by word ids: its own reminder,
/// its alarms. Its title, place, notes, repeat, margins, costs, length and
/// links go along.
pub fn event_losses(text: &str) -> Vec<&'static str> {
    let source = crate::lines::unfold(text);
    let mut lost = Vec::new();
    if source.iter().any(|l| crate::lines::name(l) == crate::reminders::REMIND) {
        lost.push("remind");
    }
    if source.iter().any(|l| crate::lines::name(l) == "BEGIN" && crate::lines::value(l).trim().eq_ignore_ascii_case("VALARM")) {
        lost.push("alarms");
    }
    lost
}

/// Another task's text once the thing it names by `old` became `new`, an
/// appointment (`became_event`) or a task: its links to it named again; its
/// waits and ties of steps on it dropped when it became an appointment,
/// which nothing waits for. None when it names nothing of it.
pub fn retarget(text: &str, old: &str, new: &str, became_event: bool, now: &Zoned) -> Option<String> {
    use crate::tasks::{link_line, link_of};
    let olds = [format!("uid:{old}"), crate::links::task_uri(old), crate::links::event_uri(old)];
    let names_it = |l: &str| match crate::lines::name(l).as_str() {
        "LINK" => olds.contains(&link_of(l).uri),
        "RELATED-TO" => crate::lines::value(l).trim() == old,
        _ => false,
    };
    let source = crate::lines::unfold(text);
    let hits: Vec<&String> = source.iter().filter(|l| names_it(l)).collect();
    if hits.is_empty() {
        return None;
    }
    let added: Vec<String> = hits
        .iter()
        .filter_map(|l| {
            if crate::lines::name(l) == "LINK" {
                let link = link_of(l);
                let uri = if link.uri.starts_with("uid:") { format!("uid:{new}") } else if became_event { crate::links::event_uri(new) } else { crate::links::task_uri(new) };
                Some(link_line(&crate::tasks::Link { uri, ..link }))
            } else if became_event {
                None
            } else {
                let value = crate::lines::value(l);
                Some(format!("{}{new}", &l[..l.len() - value.len()]))
            }
        })
        .collect();
    crate::tasks::replace_lines(text, |l| names_it(l), added, now).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> i64 {
        text.parse::<DateTime>().unwrap().to_zoned(zone()).unwrap().timestamp().as_second()
    }

    /// Its block's time first; a time of its own, for its estimate or an
    /// hour; none: the form asks, on its day at 9:00.
    #[test]
    fn a_task_made_an_appointment_takes_its_time() {
        let today: Date = "2026-10-21".parse().unwrap();
        let task = Task { title: "Dentist".into(), location: "12 rue de l'Exemple".into(), notes: "Bring the card".into(), start: "2026-10-23T14:00".into(), ..Task::default() };
        let block = (at("2026-10-23T15:00"), at("2026-10-23T15:45"));
        assert_eq!(time_of(&task, Some(block), &zone()), Some(block));
        assert_eq!(time_of(&task, None, &zone()), Some((at("2026-10-23T14:00"), at("2026-10-23T15:00"))), "an hour without a length");
        let estimated = Task { estimate: 30, ..task.clone() };
        assert_eq!(time_of(&estimated, None, &zone()), Some((at("2026-10-23T14:00"), at("2026-10-23T14:30"))));
        let dated = Task { start: "2026-10-24".into(), ..task.clone() };
        assert_eq!(time_of(&dated, None, &zone()), None, "a day is no time: never guessed");
        let event = event_of(&task, Some(block), today, &zone());
        assert_eq!((event.title.as_str(), event.start.as_str(), event.end.as_str(), event.location.as_str(), event.notes.as_str()), ("Dentist", "2026-10-23T15:00", "2026-10-23T15:45", "12 rue de l'Exemple", "Bring the card"));
        let asked = event_of(&dated, None, today, &zone());
        assert_eq!((asked.start.as_str(), asked.end.as_str()), ("2026-10-24T09:00", "2026-10-24T10:00"));
        assert_eq!(event_of(&Task::default(), None, today, &zone()).start, "2026-10-21T09:00");
    }

    /// What one kind holds that the other cannot is said; another task's ties
    /// follow the new UID, its waits dropped when it became an appointment (F5).
    #[test]
    fn what_cannot_go_along_is_said_and_ties_follow() {
        let task = Task { due: "2026-10-30".into(), priority: 2, categories: vec!["admin".into()], ..Task::default() };
        assert_eq!(task_losses(&task), ["due", "priority", "tags"]);
        assert!(task_losses(&Task::default()).is_empty());
        let event = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:e\r\nX-SIOUL-REMIND:30\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!(event_losses(event), ["remind", "alarms"]);
        let now: Zoned = "2026-10-22T12:00[Europe/Paris]".parse().unwrap();
        let other = "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:other\r\nRELATED-TO;RELTYPE=DEPENDS-ON:old\r\nLINK;LINKREL=related;VALUE=URI:sioul:task/old\r\nLINK;LINKREL=related;VALUE=UID:old\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let as_event = retarget(other, "old", "new", true, &now).unwrap();
        assert!(!as_event.contains("RELATED-TO") && as_event.contains("sioul:event/new") && as_event.contains("VALUE=UID:new") && !as_event.contains("old\r\n"), "{as_event}");
        let as_task = retarget(other, "old", "new", false, &now).unwrap();
        assert!(as_task.contains("RELATED-TO;RELTYPE=DEPENDS-ON:new") && as_task.contains("sioul:task/new"), "{as_task}");
        assert!(retarget(other, "nobody", "new", true, &now).is_none());
    }

    /// An appointment made a task: its day to start, its length as its estimate.
    #[test]
    fn an_appointment_made_a_task_keeps_its_length() {
        let event = EventEdit { title: "Bank".into(), location: "Example Street".into(), start: "2026-10-23T10:00".into(), end: "2026-10-23T11:30".into(), repeat: "weekly".into(), ..EventEdit::default() };
        let task = task_of(&event);
        assert_eq!((task.title.as_str(), task.location.as_str(), task.start.as_str(), task.estimate, task.repeat.as_str()), ("Bank", "Example Street", "2026-10-23", 90, "weekly"));
        let whole = EventEdit { all_day: true, start: "2026-10-23".into(), end: "2026-10-23".into(), ..event };
        assert_eq!((task_of(&whole).estimate, task_of(&whole).start.as_str()), (0, "2026-10-23"));
    }
}

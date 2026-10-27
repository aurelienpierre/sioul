// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a phone's account manager can send back (27 October 2026,
//! docs/compatibility.md, "A phone's ical4j"): Murena's account manager (a
//! DAVx⁵, on ical4j 3.2.13 and ical4android 1.1.2) validates every task and
//! event it uploads, and ical4j 3.2.13 crashes on RFC 9253's LINK, CONCEPT
//! and REFID (their `validate()` gives nothing); while one such task waits
//! on the phone, its whole list syncs neither way. It turns any RELATED-TO
//! other than a parent, a child or a sibling into a parent, dropping GAP,
//! and drops a DTSTART that is not before DUE.
//!
//! So Sioul writes those under its own names (`lines::OWN_NAMES`, the
//! writers in `tasks`), keeps a task's start beside DTSTART where the phone
//! would drop it (`tasks::START`), and reads both forms. The tasks and events
//! of a collection a CalDAV account syncs are rewritten so once
//! (`migrate_folder`, at that collection's first sync with this Sioul, its
//! format said in its sync state): line by line (`own_form`), only what
//! changes, the same lines on every device that does it.

use crate::lines;
use crate::vdir::{self, Kind};
use std::path::{Path, PathBuf};

/// The form the tasks and events of a collection a CalDAV account syncs are
/// written in, kept with its sync state once they were rewritten: 1, Sioul's
/// own names and the start kept (27 October 2026). 0, unsaid: written before.
pub const FORMAT: u32 = 1;

/// A line's head (its name and parameters) and its value: split at the
/// first colon outside quotes; a line without one is all head.
fn split(line: &str) -> (&str, &str) {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => return (&line[..i], &line[i + 1..]),
            _ => {}
        }
    }
    (line, "")
}

/// A head's parts, as written: its name, then each parameter (`LINKREL="…"`),
/// split at the semicolons outside quotes.
fn parts(head: &str) -> Vec<&str> {
    let (mut out, mut quoted, mut from) = (Vec::new(), false, 0);
    for (i, c) in head.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                out.push(&head[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    out.push(&head[from..]);
    out
}

/// A parameter's name, uppercase, and its value without quotes.
fn param_of(part: &str) -> (String, &str) {
    match part.split_once('=') {
        Some((name, value)) => (name.trim().to_ascii_uppercase(), value.trim().trim_matches('"')),
        None => (part.trim().to_ascii_uppercase(), ""),
    }
}

/// A link relation that is a registered type's token ("describedby",
/// "related"), which every client writes back as it is; anything else is a
/// URI, which ical4j 3.2.13 writes back without its quotes.
fn is_token(rel: &str) -> bool {
    !rel.is_empty() && rel.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// One content line in the form Sioul writes it now, byte for byte the same
/// otherwise: LINK, CONCEPT and REFID under Sioul's own names, with their
/// parameters as they are (a CONCEPT given `VALUE=URI` when it says no type,
/// RFC 9253's own for it: as text, ical4j would send its comma back
/// escaped); a link's relation that is a URI in `X-SIOUL-REL`, beside
/// `LINKREL=related`; a RELATED-TO other than a parent, a child or a sibling
/// as `X-SIOUL-RELATED-TO`. Any other line, one already so included, as it is.
pub fn own_form(line: &str) -> String {
    let (head, value) = split(line);
    let all = parts(head);
    let Some((name, params)) = all.split_first() else { return line.to_string() };
    let standard = name.trim().to_ascii_uppercase();
    let own = match standard.as_str() {
        "LINK" => crate::tasks::LINK,
        "CONCEPT" => crate::tasks::CONCEPT,
        "REFID" => crate::tasks::REFID,
        "RELATED-TO" => {
            let kind = params.iter().map(|p| param_of(p)).find(|(n, _)| n == "RELTYPE").map(|(_, v)| v.to_ascii_uppercase()).unwrap_or_else(|| "PARENT".into());
            if matches!(kind.as_str(), "PARENT" | "CHILD" | "SIBLING") {
                return line.to_string();
            }
            crate::tasks::RELATED_TO_OTHER
        }
        _ => return line.to_string(),
    };
    let mut out: Vec<String> = vec![own.to_string()];
    for part in params {
        let (param, raw) = param_of(part);
        if standard == "LINK" && param == "LINKREL" && !is_token(raw) {
            out.push("LINKREL=related".into());
            out.push(format!("{}=\"{raw}\"", crate::tasks::REL));
        } else {
            out.push((*part).to_string());
        }
    }
    if standard == "CONCEPT" && !params.iter().any(|p| param_of(p).0 == "VALUE") {
        out.push("VALUE=URI".into());
    }
    format!("{}:{value}", out.join(";"))
}

/// What `migrate_text` did to an item.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mended {
    /// Lines under Sioul's own names now.
    pub renamed: usize,
    /// A task's start kept by Sioul (`tasks::START`), beside DTSTART or in its
    /// place: equal to its DUE, of another value type, or after it.
    pub start_kept: bool,
}

/// An item's text in the form Sioul writes it now, the same on every
/// device that does it: each line of its tasks and events as `own_form` has
/// it; a task's start where DUE is not after it kept by Sioul too, and one
/// of another value type than DUE (a day to start, a time it is due, which a
/// CalDAV server refuses) or after it kept by Sioul alone, DTSTART gone (the
/// start after its due becomes the due). Nothing else changes: not its
/// stamps; SEQUENCE moved on only when DTSTART changed, which is no line of
/// Sioul's own. None when nothing changes.
pub fn migrate_text(text: &str) -> Option<(String, Mended)> {
    let source = lines::unfold(text);
    let mut mended = Mended::default();
    let mut out: Vec<String> = Vec::with_capacity(source.len() + 1);
    let mut started_changed = false;
    // Each VTODO's DTSTART and DUE, by the place of its BEGIN, read first.
    let mut todo: Option<(usize, Option<usize>, Option<usize>, Option<usize>)> = None;
    let mut todos: Vec<(usize, Option<usize>, Option<usize>, Option<usize>)> = Vec::new();
    let mut nested = 0usize;
    for (i, line) in source.iter().enumerate() {
        let name = lines::raw_name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        match (name.as_str(), todo.as_mut()) {
            ("BEGIN", None) if value == "VTODO" => todo = Some((i, None, None, None)),
            ("BEGIN", Some(_)) => nested += 1,
            ("END", Some(_)) if nested > 0 => nested -= 1,
            ("END", Some(_)) if value == "VTODO" => todos.extend(todo.take()),
            ("DTSTART", Some(t)) if nested == 0 => t.1 = Some(i),
            ("DUE", Some(t)) if nested == 0 => t.2 = Some(i),
            (n, Some(t)) if nested == 0 && n == crate::tasks::START => t.3 = Some(i),
            _ => {}
        }
    }
    // What each VTODO's start becomes: (DTSTART's line, or none; the start kept by Sioul).
    let mut starts: std::collections::BTreeMap<usize, (Option<String>, String)> = std::collections::BTreeMap::new();
    // A line's name, swapped for another, its parameters and value as written.
    let renamed = |line: &str, name: &str| format!("{name}{}", &line[line.find([';', ':']).unwrap_or(line.len())..]);
    for (_, start, due, kept) in &todos {
        let (Some(start), Some(due), None) = (*start, *due, kept) else { continue };
        let (start_line, due_line) = (&source[start], &source[due]);
        let is_date = |l: &str| lines::param(l, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE")) || lines::value(l).trim().len() == 8;
        let (start_value, due_value) = (lines::value(start_line).trim(), lines::value(due_line).trim());
        let same_type = is_date(start_line) == is_date(due_line);
        // After its due: by day when the types differ; two times as written, in one time zone.
        let after = start_value.get(..8) > due_value.get(..8) || (same_type && start_value > due_value && lines::param(start_line, "TZID") == lines::param(due_line, "TZID"));
        let change = match (after, same_type) {
            // The start after its due becomes the due, as Sioul's writer has it (`tasks::dates_lines`).
            (true, true) => (Some(renamed(due_line, "DTSTART")), renamed(due_line, crate::tasks::START)),
            (true, false) => (None, format!("{};VALUE=DATE:{}", crate::tasks::START, due_value.get(..8).unwrap_or(due_value))),
            (false, false) => (None, renamed(start_line, crate::tasks::START)),
            (false, true) if start_value == due_value => (Some(start_line.clone()), renamed(start_line, crate::tasks::START)),
            (false, true) => continue,
        };
        started_changed |= change.0.as_deref() != Some(start_line.as_str());
        starts.insert(start, change);
    }
    let present: std::collections::BTreeSet<&String> = source.iter().collect();
    for (i, line) in source.iter().enumerate() {
        if let Some((start, kept)) = starts.get(&i) {
            out.extend(start.iter().cloned());
            out.push(kept.clone());
            mended.start_kept = true;
            continue;
        }
        let own = own_form(line);
        if own != *line {
            mended.renamed += 1;
            // Its own form there already (an older build wrote the standard line again beside it): dropped, never doubled.
            if present.contains(&own) || out.contains(&own) {
                continue;
            }
        }
        out.push(own);
    }
    if mended == Mended::default() {
        return None;
    }
    // A standard line changed (DTSTART): the item's SEQUENCE moved on, as any writer's.
    if started_changed {
        for line in out.iter_mut().filter(|l| lines::raw_name(l) == "SEQUENCE") {
            let next = lines::value(line).trim().parse::<u32>().unwrap_or(0).saturating_add(1);
            *line = format!("SEQUENCE:{next}");
        }
    }
    Some((lines::fold(&out), mended))
}

/// What `migrate_folder` did in one collection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Migrated {
    /// Items rewritten in the form Sioul writes now.
    pub rewritten: usize,
    /// Events moved out of a list that holds tasks only, into the calendar
    /// for new events (`agenda::default_calendar`), their UID kept: from, to.
    pub moved: Vec<(PathBuf, PathBuf)>,
    /// Meetings with others left as their organizer wrote them.
    pub meetings: usize,
    /// Items of a kind the collection does not hold (an event where tasks
    /// only go, with no calendar for new events to take it; a task where
    /// events only go), left as they are: no writer may write them there,
    /// and their server's refusal of them is said at each sync.
    pub stranded: usize,
    /// What could not be written, said.
    pub problems: Vec<String>,
}

/// Every task and event of a collection's folder, a calendar a CalDAV
/// account syncs, written in the form Sioul writes now (`migrate_text`),
/// each through the one door of every writer (`vdir::write_item`): never a
/// meeting with others (its organizer's: `agenda::with_others`), never a
/// calendar you only read (its caller's check, and the door's); an event in
/// a list that holds tasks only (`components`) moved to the calendar for new
/// events (`events_to`: `agenda::default_calendar`), its UID and its file's
/// name kept (`vdir::move_item`: its sync sends it there, then takes it out
/// of the list); an item of a kind the collection does not hold otherwise
/// left as it is (`Migrated::stranded`). Its caller keeps that it was done
/// (`FORMAT`), so that it is never done twice. `rewrite` false: the events
/// moved out alone, the items left as they are, for a sync that sends each
/// one rewritten over its server's copy (`dav::upgrade`), never as a change
/// made here (the interop check of 49a9978, #1: a rewrite here met a change
/// there as "changed on both sides").
pub fn migrate_folder(dir: &Path, components: &[String], events_to: Option<&vdir::Collection>, rewrite: bool) -> Migrated {
    let mut done = Migrated::default();
    let holds = |component: &str| components.is_empty() || components.iter().any(|c| c.eq_ignore_ascii_case(component));
    let tasks_only = !holds("VEVENT");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == Kind::Calendars.extension()) && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))).collect();
    files.sort();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let event = text.lines().any(|l| l.trim().eq_ignore_ascii_case("BEGIN:VEVENT"));
        if event && crate::agenda::with_others(&text) {
            done.meetings += 1;
            continue;
        }
        let mut path = path;
        // An event where tasks only go: to the calendar for new events first, as it is.
        if event
            && tasks_only
            && let Some(calendar) = events_to
            && calendar.dir != dir
            && let Some(name) = path.file_name()
        {
            let to = calendar.dir.join(name);
            match vdir::move_item(&path, &to) {
                Ok(()) => {
                    done.moved.push((path.clone(), to.clone()));
                    path = to;
                }
                Err(e) => {
                    done.problems.push(e);
                    continue;
                }
            }
        }
        // Of a kind this collection does not hold, and staying in it: as it is.
        let task = text.lines().any(|l| l.trim().eq_ignore_ascii_case("BEGIN:VTODO"));
        if path.parent() == Some(dir) && ((event && !holds("VEVENT")) || (task && !event && !holds("VTODO"))) {
            done.stranded += 1;
            continue;
        }
        if rewrite && let Some((migrated, _)) = migrate_text(&text) {
            match vdir::write_item(&path, &migrated) {
                Ok(()) => done.rewritten += 1,
                Err(e) => done.problems.push(e),
            }
        }
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{self, Link};

    /// A task as Sioul wrote them before 27 October 2026, with every kind of
    /// line the tasks of the check had (fiction): links with and without a
    /// label, to a note and to a message, by URI and by UID; a kind and the
    /// office hours as concepts; a project; a step's parent, a wait with and
    /// without a gap; a contact; an estimate; another application's line.
    pub(crate) const BEFORE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Sioul//Sioul//EN\r\nBEGIN:VTODO\r\nUID:stand-in-1\r\nDTSTAMP:20261020T080000Z\r\nCREATED:20261001T080000Z\r\nLAST-MODIFIED:20261020T080000Z\r\nSEQUENCE:3\r\nSUMMARY:Send the form\\, signed\r\nSTATUS:NEEDS-ACTION\r\nDTSTART;VALUE=DATE:20261028\r\nDUE;VALUE=DATE:20261030\r\nESTIMATED-DURATION:PT20M\r\nCATEGORIES:admin\r\nCONCEPT:tag:aurelienpierre.com,2026:sioul/task-type/online\r\nCONCEPT:tag:aurelienpierre.com,2026:sioul/needs/office-hours\r\nRELATED-TO;RELTYPE=PARENT:big-one\r\nRELATED-TO;RELTYPE=DEPENDS-ON:papers\r\nRELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:answer\r\nLINK;LINKREL=describedby;LABEL=\"Outbox: 2\";VALUE=URI:sioul:note/admin/form.md\r\nLINK;LINKREL=describedby;VALUE=URI:sioul:note/admin/rules.md\r\nLINK;LINKREL=related;VALUE=URI:mid:letter-1@example.org\r\nLINK;LINKREL=related;VALUE=UID:event-7\r\nCONTACT;ALTREP=\"sioul:contact/office\":The office\\, counter\r\nREFID:paperwork\r\nX-APPLE-SORT-ORDER:2\r\nX-OTHER-APP:keep me\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Form\r\nTRIGGER:-PT1H\r\nEND:VALARM\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

    fn zone() -> jiff::tz::TimeZone {
        jiff::tz::TimeZone::get("Europe/Paris").unwrap()
    }

    /// Each line in Sioul's form now: the same parameters and value, under
    /// its own name; a parent, a child and a sibling, and every other line,
    /// as they are; a relation that is a URI in its own parameter, quoted.
    #[test]
    fn each_line_takes_sioul_s_own_name_and_keeps_its_parameters() {
        let cases = [
            ("LINK;LINKREL=describedby;LABEL=\"Outbox: 2\";VALUE=URI:sioul:note/admin/form.md", "X-SIOUL-LINK;LINKREL=describedby;LABEL=\"Outbox: 2\";VALUE=URI:sioul:note/admin/form.md"),
            ("LINK;LINKREL=related;VALUE=URI:mid:letter-1@example.org", "X-SIOUL-LINK;LINKREL=related;VALUE=URI:mid:letter-1@example.org"),
            ("LINK;LINKREL=related;VALUE=UID:event-7", "X-SIOUL-LINK;LINKREL=related;VALUE=UID:event-7"),
            (
                "LINK;LINKREL=\"tag:aurelienpierre.com,2026:sioul/rel/appointment\";X-SIOUL-DUE=20261029;VALUE=URI:sioul:event/doctor",
                "X-SIOUL-LINK;LINKREL=related;X-SIOUL-REL=\"tag:aurelienpierre.com,2026:sioul/rel/appointment\";X-SIOUL-DUE=20261029;VALUE=URI:sioul:event/doctor",
            ),
            ("CONCEPT:tag:aurelienpierre.com,2026:sioul/task-type/call", "X-SIOUL-CONCEPT;VALUE=URI:tag:aurelienpierre.com,2026:sioul/task-type/call"),
            ("CONCEPT;VALUE=TEXT:money", "X-SIOUL-CONCEPT;VALUE=TEXT:money"),
            ("REFID:taxes-2025", "X-SIOUL-REFID:taxes-2025"),
            ("RELATED-TO;RELTYPE=DEPENDS-ON:papers", "X-SIOUL-RELATED-TO;RELTYPE=DEPENDS-ON:papers"),
            ("RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:answer", "X-SIOUL-RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:answer"),
            ("RELATED-TO;RELTYPE=PARENT:big-one", "RELATED-TO;RELTYPE=PARENT:big-one"),
            ("RELATED-TO;RELTYPE=child:small-one", "RELATED-TO;RELTYPE=child:small-one"),
            ("RELATED-TO:big-one", "RELATED-TO:big-one"),
            ("CONTACT;ALTREP=\"sioul:contact/office\":The office", "CONTACT;ALTREP=\"sioul:contact/office\":The office"),
            ("X-OTHER-APP:keep me", "X-OTHER-APP:keep me"),
        ];
        for (before, after) in cases {
            assert_eq!(own_form(before), after, "{before}");
            assert_eq!(own_form(after), after, "already so: {after}");
        }
        // Sioul's writers write exactly that form.
        let pair = Link { uri: "sioul:event/doctor".into(), label: String::new(), rel: crate::pairs::APPOINTMENT.into() };
        assert_eq!(tasks::link_line_with(&pair, ";X-SIOUL-DUE=20261029"), cases[3].1);
        assert_eq!(tasks::link_line(&Link { uri: "sioul:note/admin/form.md".into(), label: "Outbox: 2".into(), rel: "describedby".into() }), cases[0].1);
        assert_eq!(tasks::relation_line("FINISHTOSTART", "answer", 14 * 1440), cases[8].1);
        assert_eq!(tasks::relation_line("PARENT", "big-one", 0), cases[9].1);
        assert_eq!(tasks::project_line("taxes-2025"), cases[6].1);
    }

    /// The whole task: only its lines of these kinds change, its stamps and
    /// its SEQUENCE stay (Sioul's own lines alone), it reads exactly as
    /// before, and once is enough: the same text again changes nothing, on
    /// any device (no time, no device in it).
    #[test]
    fn a_task_rewritten_reads_the_same_and_is_rewritten_once() {
        let (after, mended) = migrate_text(BEFORE).unwrap();
        assert_eq!(mended, Mended { renamed: 9, start_kept: false }, "{after}");
        for kept in ["DTSTAMP:20261020T080000Z", "LAST-MODIFIED:20261020T080000Z", "SEQUENCE:3", "RELATED-TO;RELTYPE=PARENT:big-one", "X-OTHER-APP:keep me", "TRIGGER:-PT1H", "DTSTART;VALUE=DATE:20261028"] {
            assert!(lines::unfold(&after).iter().any(|l| l == kept), "{kept} kept: {after}");
        }
        for gone in ["LINK;", "CONCEPT:", "REFID:", "RELATED-TO;RELTYPE=DEPENDS-ON", "RELATED-TO;RELTYPE=FINISHTOSTART"] {
            assert!(!lines::unfold(&after).iter().any(|l| l.starts_with(gone)), "{gone} renamed: {after}");
        }
        let (old, new) = (tasks::task_of_text(BEFORE, &zone()).unwrap(), tasks::task_of_text(&after, &zone()).unwrap());
        assert_eq!(old, new, "it reads the same");
        assert!(new.office_hours && new.kind == "online" && new.projects == ["paperwork"] && new.relations.len() == 3 && new.links.len() == 4, "{new:?}");
        assert_eq!(migrate_text(&after), None, "once is enough");
        assert_eq!(migrate_text(BEFORE).unwrap().0, after, "two devices write the same lines");
    }

    /// A link and a project an older build wrote again in RFC 9253's form,
    /// beside Sioul's own: read once, and the rewrite keeps one line of each,
    /// never two (the interop check of 49a9978).
    #[test]
    fn both_forms_of_one_line_are_one() {
        let both = BEFORE.replace("REFID:paperwork\r\n", "REFID:paperwork\r\nX-SIOUL-REFID:paperwork\r\nX-SIOUL-LINK;LINKREL=describedby;VALUE=URI:sioul:note/admin/rules.md\r\n");
        let read = tasks::task_of_text(&both, &zone()).unwrap();
        assert_eq!((read.projects.len(), read.links.len()), (1, 4), "{read:?}");
        let (after, _) = migrate_text(&both).unwrap();
        let count = |line: &str| lines::unfold(&after).iter().filter(|l| *l == line).count();
        assert_eq!((count("X-SIOUL-REFID:paperwork"), count("X-SIOUL-LINK;LINKREL=describedby;VALUE=URI:sioul:note/admin/rules.md")), (1, 1), "{after}");
        // The same, the links in another order (the line kept is the later one).
        let sorted = |mut t: tasks::Task| {
            t.links.sort_by(|a, b| a.uri.cmp(&b.uri));
            t
        };
        assert_eq!(sorted(tasks::task_of_text(&after, &zone()).unwrap()), sorted(read));
    }

    /// A start the phone would drop (the same day as the due) is kept by
    /// Sioul too; one of another value type than its due (a day to start, a
    /// time due: refused by Murena) is kept by Sioul alone, DTSTART gone and
    /// SEQUENCE moved on, a standard line having changed; each reads its
    /// start as before, and the phone's copy without DTSTART reads it still.
    #[test]
    fn a_start_the_phone_would_drop_or_the_server_refuses_is_kept() {
        let same = BEFORE.replace("DTSTART;VALUE=DATE:20261028", "DTSTART;VALUE=DATE:20261030");
        let (after, mended) = migrate_text(&same).unwrap();
        assert!(mended.start_kept && after.contains("DTSTART;VALUE=DATE:20261030\r\nX-SIOUL-START;VALUE=DATE:20261030\r\n") && after.contains("SEQUENCE:3"), "{after}");
        let phone = after.replace("DTSTART;VALUE=DATE:20261030\r\n", "");
        assert_eq!(tasks::task_of_text(&phone, &zone()).unwrap().start, "2026-10-30", "the phone dropped DTSTART: still read");
        let mixed = BEFORE.replace("DUE;VALUE=DATE:20261030", "DUE:20261030T150000Z");
        let (after, _) = migrate_text(&mixed).unwrap();
        assert!(!after.contains("DTSTART") && after.contains("X-SIOUL-START;VALUE=DATE:20261028") && after.contains("SEQUENCE:4") && after.contains("DUE:20261030T150000Z"), "{after}");
        let read = tasks::task_of_text(&after, &zone()).unwrap();
        assert_eq!((read.start.as_str(), read.due.as_str()), ("2026-10-28", "2026-10-30T16:00"));
        assert_eq!(migrate_text(&after), None);
        // The due first: the start becomes the due, kept by Sioul too.
        let backwards = BEFORE.replace("DTSTART;VALUE=DATE:20261028", "DTSTART;VALUE=DATE:20261101");
        let (after, _) = migrate_text(&backwards).unwrap();
        assert!(after.contains("DTSTART;VALUE=DATE:20261030\r\nX-SIOUL-START;VALUE=DATE:20261030\r\n") && after.contains("SEQUENCE:4"), "{after}");
        // A start Sioul kept, after the due another client set: not read.
        let later = phone.replace("DUE;VALUE=DATE:20261030", "DUE;VALUE=DATE:20261029");
        assert_eq!(tasks::task_of_text(&later, &zone()).unwrap().start, "");
    }

    /// A gap the phone gives back in weeks reads as the days Sioul wrote.
    #[test]
    fn a_gap_in_weeks_reads_as_its_days() {
        let task = tasks::task_of_text(&BEFORE.replace("GAP=P14D", "GAP=P2W"), &zone()).unwrap();
        assert!(task.relations.iter().any(|r| r.kind == "FINISHTOSTART" && r.gap == 14 * 1440), "{:?}", task.relations);
    }

    /// Sioul's writer: a start and a due of two value types, or the due
    /// first, never written so (the one rule of a task's dates).
    #[test]
    fn the_writer_never_writes_dates_a_server_refuses() {
        let now = jiff::Zoned::now();
        let edit = |start: &str, due: &str| tasks::TaskEdit { title: "Dated".into(), start: start.into(), due: due.into(), ..tasks::TaskEdit::default() };
        let mixed = tasks::new_task(&edit("2026-10-28", "2026-10-30T16:00"), "w-1", &zone(), &now).unwrap();
        assert!(!mixed.contains("DTSTART") && mixed.contains("X-SIOUL-START;VALUE=DATE:20261028") && mixed.contains("DUE:20261030T150000Z"), "{mixed}");
        let read = tasks::task_of_text(&mixed, &zone()).unwrap();
        assert_eq!((read.start.as_str(), read.due.as_str()), ("2026-10-28", "2026-10-30T16:00"));
        let same = tasks::new_task(&edit("2026-10-30", "2026-10-30"), "w-2", &zone(), &now).unwrap();
        assert!(same.contains("DTSTART;VALUE=DATE:20261030") && same.contains("X-SIOUL-START;VALUE=DATE:20261030"), "{same}");
        let backwards = tasks::new_task(&edit("2026-11-02", "2026-10-30"), "w-3", &zone(), &now).unwrap();
        assert!(backwards.contains("DTSTART;VALUE=DATE:20261030") && !backwards.contains("20261102"), "{backwards}");
        let apart = tasks::new_task(&edit("2026-10-28", "2026-10-30"), "w-4", &zone(), &now).unwrap();
        assert!(apart.contains("DTSTART;VALUE=DATE:20261028") && !apart.contains("X-SIOUL-START"), "{apart}");
        // Changed in the form: its old start lines go with it.
        let task = tasks::task_of_text(&same, &zone()).unwrap();
        let moved = tasks::apply(&same, &tasks::TaskEdit { due: "2026-11-03".into(), ..tasks::TaskEdit::of(&task) }, &zone(), &now).unwrap();
        assert!(moved.contains("DTSTART;VALUE=DATE:20261030") && !moved.contains("X-SIOUL-START") && moved.contains("DUE;VALUE=DATE:20261103"), "{moved}");
        assert_eq!(migrate_text(&moved), None, "Sioul's writer writes the form the rewrite makes");
        assert_eq!(migrate_text(&mixed), None);
        assert_eq!(migrate_text(&same), None);
    }

    /// The folder of a list a CalDAV account syncs, holding tasks only: its
    /// tasks rewritten, a meeting with others left as its organizer wrote it,
    /// and an event there moved to the calendar for new events, its UID and
    /// its file's name kept; a calendar takes no item it cannot hold, through
    /// the one door of every writer.
    #[test]
    fn a_list_s_folder_is_rewritten_and_its_event_moved_out() {
        crate::config::stand_ins_only();
        let account = format!("interop-{}", std::process::id());
        let list = vdir::create(Kind::Calendars, &account, "Errands", None, &["VTODO"]).unwrap();
        let calendar = vdir::create(Kind::Calendars, &account, "Personal", None, &["VEVENT"]).unwrap();
        std::fs::write(list.dir.join("task.ics"), BEFORE).unwrap();
        let event = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:wrong-place\r\nDTSTAMP:20261021T080000Z\r\nDTSTART:20261029T090000Z\r\nDTEND:20261029T100000Z\r\nSUMMARY:Dentist\r\nLINK;LINKREL=related;VALUE=URI:sioul:task/stand-in-1\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        std::fs::write(list.dir.join("wrong-place.ics"), event).unwrap();
        let meeting = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:meeting\r\nDTSTAMP:20261021T080000Z\r\nDTSTART:20261030T090000Z\r\nDTEND:20261030T100000Z\r\nSUMMARY:Board\r\nORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:someone@example.org\r\nREFID:paperwork\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        std::fs::write(calendar.dir.join("meeting.ics"), meeting).unwrap();
        // The door: no event where tasks only go, no task where events only go.
        assert!(vdir::write_item(&list.dir.join("new-event.ics"), event).is_err());
        assert!(vdir::write_item(&calendar.dir.join("new-task.ics"), BEFORE).is_err());
        let done = migrate_folder(&list.dir, &list.components, Some(&calendar), true);
        assert!(done.problems.is_empty(), "{:?}", done.problems);
        assert_eq!((done.rewritten, done.moved.len()), (2, 1), "{done:?}");
        assert!(!list.dir.join("wrong-place.ics").exists());
        let moved = std::fs::read_to_string(calendar.dir.join("wrong-place.ics")).unwrap();
        assert!(moved.contains("UID:wrong-place") && moved.contains("X-SIOUL-LINK;LINKREL=related;VALUE=URI:sioul:task/stand-in-1"), "{moved}");
        assert_eq!(migrate_text(&std::fs::read_to_string(list.dir.join("task.ics")).unwrap()), None);
        let theirs = migrate_folder(&calendar.dir, &calendar.components, None, true);
        assert_eq!((theirs.meetings, theirs.rewritten), (1, 0), "the meeting left, the moved event rewritten already: {theirs:?}");
        assert_eq!(std::fs::read_to_string(calendar.dir.join("meeting.ics")).unwrap(), meeting);
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(&account));
    }

    /// The tasks of the check of 26 October 2026 whose upload crashed the
    /// phone's account manager, by how many LINK, CONCEPT and REFID lines
    /// each held (its `crashing-tasks.tsv`, the counts alone).
    const CRASHING: [(usize, usize, usize); 54] = [
        (0, 0, 1), (1, 0, 1), (2, 0, 1), (1, 0, 1), (1, 0, 0), (1, 0, 0), (1, 0, 0), (2, 0, 1), (1, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1), (2, 0, 1), (2, 0, 0),
        (1, 0, 0), (1, 0, 0), (1, 0, 0), (1, 0, 0), (1, 0, 0), (1, 0, 0), (1, 0, 0), (0, 0, 1), (1, 0, 0), (1, 0, 0), (0, 0, 1), (0, 0, 1), (1, 1, 1), (1, 0, 0),
        (1, 0, 1), (1, 0, 0), (1, 0, 0), (1, 0, 1), (1, 0, 0), (1, 0, 0), (1, 0, 0), (1, 0, 0), (1, 0, 0), (1, 0, 0), (2, 0, 1), (2, 0, 0), (2, 0, 0), (1, 0, 0),
        (1, 0, 0), (1, 0, 0), (0, 1, 0), (0, 1, 0), (0, 1, 0), (0, 1, 0), (0, 1, 0), (0, 1, 0), (0, 1, 0), (0, 1, 0), (0, 1, 0), (0, 1, 0),
    ];

    /// Stand-ins of a real list's shapes alone, as that check found them
    /// (fiction): tasks with LINK lines (some with a label, some to a
    /// message), CONCEPT and REFID lines that crash, some starting and
    /// due the same day; tasks waiting on another, some after a gap,
    /// steps of a parent, contacts, estimates, some done
    /// or with an alarm; then the four items Murena refused: three tasks with
    /// a day to start and a time due, each with a kind, and an event in the
    /// list, its link by UID. Each as (file name, text).
    fn stand_in_list() -> Vec<(String, String)> {
        let kinds = ["call", "write", "online", "out", "read", "think"];
        let wrap = |component: &str, uid: &str, lines: &[String]| {
            let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".into(), "PRODID:-//Sioul//Sioul//EN".into(), format!("BEGIN:{component}"), format!("UID:{uid}"), "DTSTAMP:20261020T080000Z".into()];
            out.extend(lines.iter().cloned());
            out.extend([format!("END:{component}"), "END:VCALENDAR".into()]);
            lines::fold(&out)
        };
        let mut items = Vec::new();
        let mut links = 0usize;
        for i in 0..109usize {
            let mut l = vec![format!("SEQUENCE:{}", i % 4), format!("SUMMARY:Task {i}\\, a stand-in")];
            if i % 6 == 4 {
                l.extend(["STATUS:COMPLETED".into(), "COMPLETED:20261025T100000Z".into(), "PERCENT-COMPLETE:100".into()]);
            } else {
                l.push("STATUS:NEEDS-ACTION".into());
            }
            let (link_lines, concepts, refids) = CRASHING.get(i).copied().unwrap_or_default();
            // The tasks of a kind starting and due the same day; the others' dates as they came.
            if (44..52).contains(&i) {
                let day = format!("202610{}", 26 + i % 5);
                l.extend([format!("DTSTART;VALUE=DATE:{day}"), format!("DUE;VALUE=DATE:{day}")]);
            } else {
                match i % 5 {
                    0 => l.extend(["DTSTART;VALUE=DATE:20261101".into(), "DUE;VALUE=DATE:20261106".into()]),
                    1 => l.push("DUE;VALUE=DATE:20261110".into()),
                    2 => l.push("DUE:20261105T090000Z".into()),
                    _ => {}
                }
            }
            if i % 4 == 0 {
                l.push(format!("DESCRIPTION:Notes for task {i}\\, with a comma\\nand a second line"));
            }
            if i % 3 == 0 && i <= 96 {
                l.push(format!("ESTIMATED-DURATION:PT{}M", 15 + i % 4 * 15));
            }
            if i % 7 == 3 {
                l.push("CATEGORIES:admin".into());
            }
            // Two ratings in one line, raw as written before 27 October 2026, or escaped as since.
            if i % 9 == 4 {
                l.push(if i % 2 == 0 { "X-SIOUL-COST:COGNITIVE=3;EMOTIONAL=5".into() } else { "X-SIOUL-COST:COGNITIVE=3\\;EMOTIONAL=5".into() });
                l.push("X-SIOUL-FELT-COST;X-SIOUL-ON=20261020:ANXIETY=3\\;BODY=1".into());
            }
            for n in 0..concepts {
                let uri = if i == 26 { tasks::OFFICE_CONCEPT.to_string() } else { format!("{}{}", tasks::KIND_CONCEPT, kinds[(i + n) % kinds.len()]) };
                l.push(format!("CONCEPT:{uri}"));
            }
            if i % 4 == 2 && i <= 98 {
                l.push(format!("RELATED-TO;RELTYPE=DEPENDS-ON:task-{:03}", (i + 1) % 109));
            }
            if i == 3 || i == 6 {
                l.push(format!("RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:task-{:03}", i + 10));
            }
            if i % 7 == 5 && i <= 96 {
                l.push(format!("RELATED-TO;RELTYPE=PARENT:task-{:03}", i / 2));
            }
            for _ in 0..link_lines {
                l.push(match links {
                    0..10 => format!("LINK;LINKREL=describedby;LABEL=\"Outbox: {links}\";VALUE=URI:sioul:note/admin/letter-{links}.md"),
                    10 | 11 => format!("LINK;LINKREL=related;VALUE=URI:mid:letter-{links}.{i}@example.org"),
                    _ => format!("LINK;LINKREL=describedby;VALUE=URI:sioul:note/notes/stand-in-{links}.md"),
                });
                links += 1;
            }
            if i % 7 == 1 && i <= 106 {
                l.push(format!("CONTACT;ALTREP=\"sioul:contact/office-{i}\":The office {i}\\, counter"));
            }
            for n in 0..refids {
                l.push(format!("REFID:project-{}", (i + n) % 13));
            }
            l.push(format!("X-APPLE-SORT-ORDER:{i}"));
            if i % 11 == 0 {
                l.extend(["BEGIN:VALARM".into(), "ACTION:DISPLAY".into(), format!("DESCRIPTION:Task {i}"), "TRIGGER:-PT1H".into(), "END:VALARM".into()]);
            }
            items.push((format!("task-{i:03}.ics"), wrap("VTODO", &format!("task-{i:03}"), &l)));
        }
        for n in 1..=3 {
            let l = [format!("SUMMARY:Imported task {n}"), "STATUS:NEEDS-ACTION".into(), "DTSTART;VALUE=DATE:20261028".into(), "DUE:20261030T150000Z".into(), format!("CONCEPT:{}{}", tasks::KIND_CONCEPT, kinds[n])];
            items.push((format!("task-mixed-{n}.ics"), wrap("VTODO", &format!("task-mixed-{n}"), &l)));
        }
        let l = ["DTSTART:20261029T090000Z".to_string(), "DTEND:20261029T100000Z".into(), "SUMMARY:Appointment".into(), "LINK;LINKREL=related;VALUE=UID:task-001".into()];
        items.push(("task-event.ics".into(), wrap("VEVENT", "task-event", &l)));
        items
    }

    /// Whether a phone's account manager on ical4j 3.2.13 would crash
    /// uploading the item, or turn a line into another: LINK, CONCEPT,
    /// REFID, a RELATED-TO other than a parent, a child or a sibling.
    fn crashes_or_bends(text: &str) -> bool {
        lines::unfold(text).iter().any(|l| {
            let name = lines::raw_name(l);
            ["LINK", "CONCEPT", "REFID"].contains(&name.as_str()) || (name == "RELATED-TO" && lines::param(l, "RELTYPE").is_some_and(|k| !["PARENT", "CHILD", "SIBLING"].contains(&k.to_ascii_uppercase().as_str())))
        })
    }

    /// The list's shapes, every item: the stand-ins hold what the check
    /// counted, and once rewritten none holds a line the phone crashes on or
    /// bends, each reads as before (its start too, kept by Sioul where the
    /// phone would drop it or the server refuses it), and once is enough.
    /// `SIOUL_ICAL4J_OUT` naming a folder made for tests, the items are
    /// written there as they were and as rewritten (`before/`, `after/`,
    /// `tasks.txt`, `events.txt`), for the ical4j 3.2.13 checker
    /// (docs/compatibility.md, "A phone's ical4j").
    #[test]
    fn every_shape_of_the_stand_in_list_is_rewritten_so_the_phone_sends_it() {
        let items = stand_in_list();
        let count = |name: &str| items.iter().map(|(_, t)| lines::unfold(t).iter().filter(|l| lines::raw_name(l) == name).count()).sum::<usize>();
        assert_eq!((items.len(), count("LINK"), count("CONCEPT"), count("REFID")), (113, 47, 14, 15));
        let crashing = items.iter().take(109).filter(|(_, t)| lines::unfold(t).iter().any(|l| ["LINK", "CONCEPT", "REFID"].contains(&lines::raw_name(l).as_str()))).count();
        assert_eq!(crashing, 54);
        let mut rewritten = Vec::new();
        for (name, before) in &items {
            let after = migrate_text(before).map(|(t, _)| t).unwrap_or_else(|| before.clone());
            assert!(!crashes_or_bends(&after), "{name}: {after}");
            assert_eq!(migrate_text(&after), None, "{name}: once is enough");
            if before.contains("BEGIN:VTODO") {
                let (old, new) = (tasks::task_of_text(before, &zone()).unwrap(), tasks::task_of_text(&after, &zone()).unwrap());
                assert_eq!(old, new, "{name} reads the same");
            }
            rewritten.push((name.clone(), before.clone(), after));
        }
        for (name, _, after) in rewritten.iter().filter(|(n, _, _)| n.starts_with("task-mixed-")) {
            assert!(!after.contains("DTSTART") && after.contains("X-SIOUL-START;VALUE=DATE:20261028"), "{name}: {after}");
        }
        assert_eq!(rewritten.iter().filter(|(_, _, a)| a.contains("X-SIOUL-START")).count(), 8 + 3, "the same-day starts and the three refused");
        // What the phone sends back after two of its round trips, marked done
        // there (the checker's `rounds`, its copies in `SIOUL_ICAL4J_BACK`):
        // Sioul reads every task as it wrote it, its waits, gaps (P2W for
        // P14D) and starts (DTSTART dropped the same day) included.
        if let Some(back) = std::env::var_os("SIOUL_ICAL4J_BACK").map(std::path::PathBuf::from) {
            let mut read = 0;
            for (name, _, after) in rewritten.iter().filter(|(_, b, _)| b.contains("BEGIN:VTODO")) {
                let theirs = std::fs::read_to_string(back.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert!(!crashes_or_bends(&theirs), "{name}: {theirs}");
                let (mine, theirs) = (tasks::task_of_text(after, &zone()).unwrap(), tasks::task_of_text(&theirs, &zone()).unwrap());
                // In any order: the phone writes the parents before the lines it keeps unread.
                let sorted = |v: Vec<String>| {
                    let mut v = v;
                    v.sort();
                    v
                };
                let what = |t: &tasks::Task| {
                    let relations = sorted(t.relations.iter().map(|r| format!("{} {} {}", r.kind, r.uid, r.gap)).collect());
                    let links = sorted(t.links.iter().map(|l| format!("{} {} {}", l.rel, l.label, l.uri)).collect());
                    (t.title.clone(), t.notes.clone(), t.start.clone(), t.due.clone(), t.kind.clone(), t.office_hours, sorted(t.projects.clone()), relations, links, t.contacts.len(), t.estimate, (t.categories.clone(), format!("{:?} {:?}", t.demands, t.felt)))
                };
                assert_eq!(what(&mine), what(&theirs), "{name}: as the phone sends it back");
                read += 1;
            }
            assert_eq!(read, 112);
        }
        let Some(out) = std::env::var_os("SIOUL_ICAL4J_OUT").map(std::path::PathBuf::from) else { return };
        assert!(out.join(crate::config::STAND_IN_MARK).exists(), "{}: a folder made for tests only", out.display());
        for side in ["before", "after"] {
            std::fs::create_dir_all(out.join(side)).unwrap();
        }
        let (mut todos, mut events) = (String::new(), String::new());
        for (name, before, after) in &rewritten {
            std::fs::write(out.join("before").join(name), before).unwrap();
            std::fs::write(out.join("after").join(name), after).unwrap();
            let list = if before.contains("BEGIN:VEVENT") { &mut events } else { &mut todos };
            list.push_str(name);
            list.push('\n');
        }
        std::fs::write(out.join("tasks.txt"), todos).unwrap();
        std::fs::write(out.join("events.txt"), events).unwrap();
    }
}

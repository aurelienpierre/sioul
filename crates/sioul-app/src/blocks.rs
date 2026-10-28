// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Time blocks behind the window (docs/tasks.md, "Pinned to a time"): a task
//! pinned to a time is an event in a calendar, made, moved and taken away
//! here, the task tied to it; the plan reads them (`sioul_core::blocks`).
//!
//! - "Do at…" and a drag in the Tasks day view move the task's block, or make
//!   one in the calendar for blocks (made at first use); "Undo" puts back what
//!   was there.
//! - "Let the plan place it" takes its blocks to come away after ten seconds
//!   ("Undo" keeps them); one under way ends then.
//! - Done or dropped here: its blocks not begun go, one under way ends then,
//!   those over stay, the record of when the work was done. Deleted: its
//!   blocks to come go with it, under the same "Undo".
//! - Each time the plan is made, what changed elsewhere is set right
//!   (`upkeep`), and a time given by a drag before blocks becomes a block, once.

use crate::backend::{QtThread, Shared, load_config, say, tr};
use crate::{mail, pim, work};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use sioul_core::agenda::Occurrence;
use sioul_core::blocks::{self, Blocks, Closing, Fix, Pin, Place};
use sioul_core::tasks::Task;
use sioul_core::vdir::{self, Collection, Kind};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The blocks found when the plan was last made: what a pin moves, what
/// "Let the plan place it" takes away, what `upkeep` sets right.
static FOUND: Mutex<Vec<Occurrence>> = Mutex::new(Vec::new());

/// The calendars made or chosen for blocks, as last read: (folder, what its
/// files were, the day read, its blocks). Read again when a file in it
/// changed, came or went, and each day.
type Read = Vec<(PathBuf, u64, jiff::civil::Date, Vec<Occurrence>)>;
static READ: Mutex<Read> = Mutex::new(Vec::new());

/// What a calendar's files are, cheaply: their names, sizes and times.
fn fingerprint(calendar: &Collection) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for path in calendar.items() {
        path.hash(&mut hasher);
        if let Ok(meta) = std::fs::metadata(&path) {
            meta.len().hash(&mut hasher);
            meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// The blocks of the calendars made or chosen for them, from two days ago to
/// as far as the plan goes: beyond the four weeks of events the plan reads.
fn read_own(chosen: &str, now: &Zoned) -> Vec<Occurrence> {
    let stamp = now.timestamp().as_second();
    let mut kept = READ.lock().unwrap_or_else(|e| e.into_inner());
    let calendars = blocks::calendars(chosen);
    kept.retain(|(dir, ..)| calendars.iter().any(|c| &c.dir == dir));
    let mut out = Vec::new();
    for calendar in calendars {
        let print = fingerprint(&calendar);
        if let Some((.., found)) = kept.iter().find(|(dir, p, day, _)| *dir == calendar.dir && *p == print && *day == now.date()) {
            out.extend(found.iter().cloned());
            continue;
        }
        let found = blocks::read(std::slice::from_ref(&calendar), stamp - 2 * 86_400, stamp + blocks::AHEAD_DAYS * 86_400, now.time_zone());
        kept.retain(|(dir, ..)| *dir != calendar.dir);
        kept.push((calendar.dir.clone(), print, now.date(), found.clone()));
        out.extend(found);
    }
    out
}

/// The tasks pinned to a time as of `now`: their blocks among `events` (the
/// plan's weeks, every calendar) and in the calendars made or chosen for
/// blocks, further ahead; those whose deletion waits ("Undo") aside. The
/// blocks found are kept for what moves them.
pub(crate) fn pins(shared: &Shared, events: &[Occurrence], tasks: &[Task], now: &Zoned) -> Blocks {
    let chosen = load_config().tasks.blocks.unwrap_or_default();
    let (removed, _) = mail::hidden_pim(shared);
    let mut found: Vec<Occurrence> = events.iter().filter(|e| !e.task.is_empty()).cloned().collect();
    found.extend(read_own(&chosen, now));
    let mut seen = BTreeSet::new();
    found.retain(|b| !removed.contains(Path::new(&b.key)) && seen.insert((b.key.clone(), b.start)));
    let sorted = Blocks::of(&found, tasks, now.timestamp().as_second(), now.time_zone());
    *FOUND.lock().unwrap_or_else(|e| e.into_inner()) = found;
    sorted
}

/// The blocks of a task found when the plan was last made, each file once.
/// Never a block given a guest: a meeting with others now, its organizer's
/// (`agenda::may_change`), it is neither moved, ended, renamed nor taken out.
fn found_of(uid: &str) -> Vec<Occurrence> {
    let mut seen = BTreeSet::new();
    FOUND.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|b| b.task == uid && !b.with_others && seen.insert(b.key.clone())).cloned().collect()
}

/// A file written, and its account told to send it.
fn write(shared: &Shared, path: &Path, text: &str) -> Result<(), String> {
    vdir::write_item(path, text)?;
    if let Some(account) = pim::account_of(path) {
        pim::nudge(shared, &account);
    }
    Ok(())
}

/// A file deleted here, through the one rule (`vdir::remove_item`); its account deletes it there at the next sync.
fn remove(shared: &Shared, path: &Path) -> Result<(), String> {
    vdir::remove_item(path)?;
    if let Some(account) = pim::account_of(path) {
        pim::nudge(shared, &account);
    }
    Ok(())
}

/// "today, 14:00–14:45", "Thursday 8 October, 10:00–10:30".
fn when(start: i64, end: i64) -> String {
    let zone = TimeZone::system();
    let at = |seconds: i64| Timestamp::from_second(seconds).map(|t| t.to_zoned(zone.clone())).ok();
    let (Some(from), Some(to)) = (at(start), at(end)) else { return String::new() };
    let (first, last) = (from.strftime("%H:%M").to_string(), to.strftime("%H:%M").to_string());
    let today = Zoned::now().date();
    if from.date() == today {
        say("task-pinned-today", &[("from", first), ("to", last)])
    } else {
        say("task-pinned-day", &[("day", tr().day_in(from.date(), today)), ("from", first), ("to", last)])
    }
}

/// The calendar a task's blocks go into, made when it is the calendar for
/// blocks and not there yet (`blocks::place_for`).
pub(crate) fn calendar_for(task: &Task) -> Result<Collection, String> {
    let config = load_config();
    let chosen = config.tasks.blocks.clone().unwrap_or_default();
    let account = task.list_id.split('/').next().unwrap_or("").to_string();
    let makes = sioul_core::capabilities::makes_collections(sioul_core::capabilities::provider_of(config.account(&account)));
    match blocks::place_for(&chosen, &account, &vdir::collections(Kind::Calendars), makes) {
        Place::Found(calendar) => Ok(calendar),
        Place::Make(account) => blocks::make_calendar(&account, &tr().text("blocks-calendar", None)),
    }
}

/// A new block for `task`, `start` to `end`: written, its account told; its file and its UID.
fn make(shared: &Shared, task: &Task, start: i64, end: i64, now: &Zoned) -> Result<(PathBuf, String), String> {
    let calendar = calendar_for(task)?;
    let uid = vdir::new_name();
    let text = blocks::new_block(task, &uid, start, end, load_config().tasks.block_alarms, now)?;
    let path = calendar.dir.join(format!("{uid}.ics"));
    write(shared, &path, &text)?;
    Ok((path, uid))
}

/// A file as it was before a change, and as the change wrote it (`None`: there was none, or there is none now).
struct Written {
    path: PathBuf,
    before: Option<String>,
    after: Option<String>,
}

/// What was written put back, unless something changed it since (a sync,
/// another device): says what went wrong, else "".
fn put_back(shared: &Shared, written: &[Written]) -> String {
    if written.iter().any(|w| std::fs::read_to_string(&w.path).ok() != w.after) {
        return tr().text("undo-too-late", None);
    }
    for w in written {
        let done = match &w.before {
            Some(text) => write(shared, &w.path, text),
            None => remove(shared, &w.path),
        };
        if let Err(e) = done {
            return e;
        }
    }
    String::new()
}

/// Whether a step pinned at `start` (Unix seconds) would start before the
/// five minutes `now` is in: a step is given a time from now on.
fn before_now(start: i64, now: i64) -> bool {
    start < now.div_euclid(300) * 300
}

/// Whether a pin may start at `start`, `now`, its task's block to come being
/// `current`: from the current five minutes on, or the block's own start,
/// under way, its length changed (review of 1880bce, L5).
fn may_start(start: i64, now: i64, current: Option<&Pin>) -> bool {
    !before_now(start, now) || current.is_some_and(|pin| pin.start == start)
}

/// A task pinned to `at` ("2026-10-06T14:30", your time zone) for `minutes`
/// (0: as long as its block is, else what the plan lays for it): its block
/// moved there, or made in the calendar for blocks; the task tied to it, and
/// to it alone. Told in the status line, "Undo" offered: both put back.
/// Returns what went wrong, else "".
pub(crate) fn pin(qt: &QtThread, shared: &Arc<Shared>, uid: &str, at: &str, minutes: u32) -> String {
    let zone = TimeZone::system();
    let now = Zoned::now();
    let task = match work::find_task(shared, uid) {
        Ok(task) => task,
        Err(e) => return e,
    };
    if task.read_only {
        return tr().text("task-read-only", None);
    }
    let Some(start) = at.parse::<jiff::civil::DateTime>().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map(|z| z.timestamp().as_second()) else { return format!("{at}?") };
    // The block it is pinned to, else none: one is made. One another application made repeat is left as it is.
    let own: Vec<Occurrence> = found_of(uid).into_iter().filter(|b| !b.recurring).collect();
    let current = Blocks::of(&own, std::slice::from_ref(&task), now.timestamp().as_second(), &zone).pins.remove(uid).filter(|p| Path::new(&p.key).is_file());
    // Never pinned before the current five minutes, whoever asks (the agenda
    // offers no drag there; the writer says it all the same); but a block under
    // way keeps its own start as its length changes ("Do at…" prefills it so:
    // review of 1880bce, L5).
    if !may_start(start, now.timestamp().as_second(), current.as_ref()) {
        return tr().text("task-pin-before-now", None);
    }
    if current.as_ref().is_some_and(|p| p.read_only) {
        return tr().text("task-pinned-read-only", None);
    }
    let minutes = match (minutes, &current) {
        (0, Some(pin)) => pin.minutes(),
        (0, None) => blocks::default_minutes(&task, work::laid(shared, uid), load_config().tasks.estimate.unwrap_or(30)),
        (m, _) => m,
    }
    .max(blocks::SHORTEST);
    let end = start + i64::from(minutes) * 60;
    let mut written = Vec::new();
    let block_uid = match &current {
        Some(pin) => {
            let path = PathBuf::from(&pin.key);
            let Ok(before) = std::fs::read_to_string(&path) else { return tr().text("agenda-gone", None) };
            let moved = blocks::moved(&before, start, end, &zone).map(|text| blocks::titled(&text, &task.title, &zone).unwrap_or(text));
            let text = match moved {
                Ok(text) => text,
                Err(e) => return e,
            };
            if let Err(e) = write(shared, &path, &text) {
                return e;
            }
            written.push(Written { path, before: Some(before), after: Some(text) });
            pin.uid.clone()
        }
        None => match make(shared, &task, start, end, &now) {
            Ok((path, uid)) => {
                written.push(Written { after: std::fs::read_to_string(&path).ok(), path, before: None });
                uid
            }
            Err(e) => return e,
        },
    };
    // The task tied to it, and to it alone; a time given by a drag before blocks taken out.
    let task_path = PathBuf::from(&task.key);
    if let Ok(before) = std::fs::read_to_string(&task_path)
        && let Ok(text) = blocks::task_pinned(&before, Some(&block_uid), &now)
        && text != before
    {
        if let Err(e) = write(shared, &task_path, &text) {
            return e;
        }
        written.push(Written { path: task_path, before: Some(before), after: Some(text) });
    }
    let line = say("drag-done", &[("what", task.title.clone()), ("time", when(start, end))]);
    mail::offer_back(qt, shared, line, move |qt, shared| {
        let problem = put_back(shared, &written);
        work::show_work(qt, shared);
        pim::show_pim(qt, shared);
        problem
    });
    work::show_work(qt, shared);
    pim::show_pim(qt, shared);
    String::new()
}

/// "Let the plan place it": the task's blocks not begun deleted after ten
/// seconds ("Undo" keeps them), one under way ending now; its link to its
/// block, and a time given by a drag before blocks, taken out at once, put
/// back by "Undo". Returns what went wrong, else "".
pub(crate) fn unpin(qt: &QtThread, shared: &Arc<Shared>, uid: &str) -> String {
    let zone = TimeZone::system();
    let now = Zoned::now();
    let task = match work::find_task(shared, uid) {
        Ok(task) => task,
        Err(e) => return e,
    };
    if task.read_only {
        return tr().text("task-read-only", None);
    }
    let stamp = now.timestamp().as_second();
    let mut removals = Vec::new();
    let mut written = Vec::new();
    for block in found_of(uid).into_iter().filter(|b| !b.read_only && !b.recurring) {
        let path = PathBuf::from(&block.key);
        match blocks::closing(block.start, block.end, stamp) {
            Closing::Keep => {}
            Closing::Delete => removals.push((pim::account_of(&path).unwrap_or_default(), path)),
            Closing::EndAt(end) => {
                let Ok(before) = std::fs::read_to_string(&path) else { continue };
                if let Ok(text) = blocks::ended(&before, end, &zone)
                    && write(shared, &path, &text).is_ok()
                {
                    written.push(Written { path, before: Some(before), after: Some(text) });
                }
            }
        }
    }
    let task_path = PathBuf::from(&task.key);
    if let Ok(before) = std::fs::read_to_string(&task_path)
        && let Ok(text) = blocks::task_pinned(&before, None, &now)
        && text != before
    {
        if let Err(e) = write(shared, &task_path, &text) {
            return e;
        }
        written.push(Written { path: task_path, before: Some(before), after: Some(text) });
    }
    if removals.is_empty() && written.is_empty() {
        return String::new();
    }
    let line = say("day-let-plan-done", &[("title", task.title.clone())]);
    let back = move |qt: &QtThread, shared: &Arc<Shared>| {
        let problem = put_back(shared, &written);
        work::show_work(qt, shared);
        pim::show_pim(qt, shared);
        problem
    };
    mail::schedule_removals(qt, shared, removals, Some(Box::new(back)), line);
    // Laid again at once, even when only the task changed (a time given before blocks).
    work::show_work(qt, shared);
    pim::show_pim(qt, shared);
    String::new()
}

/// A task done or dropped at `at` (Unix seconds), or a turn of a repeating one
/// done: its blocks not begun deleted, their time freed; one under way ending
/// then; those over kept, the record of when the work was done.
pub(crate) fn closed(qt: &QtThread, shared: &Arc<Shared>, uid: &str, at: i64) {
    let zone = TimeZone::system();
    let mut changed = false;
    for block in found_of(uid).into_iter().filter(|b| !b.read_only && !b.recurring) {
        let path = PathBuf::from(&block.key);
        let done = match blocks::closing(block.start, block.end, at) {
            Closing::Keep => continue,
            Closing::Delete => remove(shared, &path),
            Closing::EndAt(end) => std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|text| blocks::ended(&text, end, &zone)).and_then(|text| write(shared, &path, &text)),
        };
        changed |= done.is_ok();
    }
    // The agenda and the day without them at once.
    if changed {
        work::show_work(qt, shared);
        pim::show_pim(qt, shared);
    }
}

/// The blocks to delete with a task deleted, (account, file): those not
/// begun. One under way, or over, stays: an event of its own once the task is gone.
pub(crate) fn with_task(uid: &str) -> Vec<(String, PathBuf)> {
    let stamp = Timestamp::now().as_second();
    found_of(uid).into_iter().filter(|b| !b.read_only && !b.recurring && b.start > stamp).map(|b| (pim::account_of(Path::new(&b.key)).unwrap_or_default(), PathBuf::from(&b.key))).collect()
}

/// A task's title or its dates changed here: its blocks to come follow, the
/// title as its own, a repeating task's turn as its dates say now (the task
/// read from its file: the window's copy is read again later).
pub(crate) fn follow(qt: &QtThread, shared: &Arc<Shared>, uid: &str) {
    let zone = TimeZone::system();
    let Ok(known) = work::find_task(shared, uid) else { return };
    let Some(task) = std::fs::read_to_string(&known.key).ok().and_then(|text| sioul_core::tasks::task_of_text(&text, &zone)) else { return };
    let stamp = Timestamp::now().as_second();
    let turn = blocks::turn_of(&task);
    for block in found_of(uid).into_iter().filter(|b| !b.read_only && !b.recurring && b.end > stamp) {
        let path = PathBuf::from(&block.key);
        let Ok(before) = std::fs::read_to_string(&path) else { continue };
        let mut text = blocks::titled(&before, &task.title, &zone).unwrap_or_else(|| before.clone());
        if !block.turn.is_empty() && !turn.is_empty() && block.turn != turn {
            text = blocks::turned(&text, &turn).unwrap_or(text);
        }
        if text != before && write(shared, &path, &text).is_ok() {
            pim::show_pim(qt, shared);
        }
    }
}

/// What changed elsewhere set right, and the times given by a drag before
/// blocks made blocks, once each (`blocks::upkeep`, `blocks::given_times`):
/// only what differs is written. Returns whether anything was.
pub(crate) fn upkeep(shared: &Shared, tasks: &[Task], now: &Zoned) -> bool {
    let zone = now.time_zone();
    let mut wrote = false;
    // A time given by a drag before blocks, today or later: a block, once; the line goes from the task.
    for (task, start) in blocks::given_times(tasks, now.date(), zone).into_iter().filter(|(t, _)| !t.read_only) {
        let minutes = blocks::default_minutes(task, None, load_config().tasks.estimate.unwrap_or(30));
        let Ok(before) = std::fs::read_to_string(&task.key) else { continue };
        let Ok((block, uid)) = make(shared, task, start, start + i64::from(minutes) * 60, now) else { continue };
        // The task tied to its block, the line gone; failing that, no block either: never one made twice.
        match blocks::task_pinned(&before, Some(&uid), now).and_then(|text| write(shared, Path::new(&task.key), &text)) {
            Ok(()) => wrote = true,
            Err(_) => {
                let _ = remove(shared, &block);
            }
        }
    }
    let (removed, _) = mail::hidden_pim(shared);
    let found: Vec<Occurrence> = FOUND.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|b| !b.read_only && !removed.contains(Path::new(&b.key))).cloned().collect();
    for (key, fix) in blocks::upkeep(&found, tasks, now.timestamp().as_second()) {
        let path = PathBuf::from(&key);
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        // What would be written, when it differs: nothing is written twice.
        let changed = match &fix {
            Fix::Delete => None,
            Fix::EndAt(end) => blocks::ended(&text, *end, zone).ok().filter(|t| *t != text),
            Fix::Title(title) => blocks::titled(&text, title, zone),
        };
        let done = match (&fix, changed) {
            (Fix::Delete, _) => remove(shared, &path).is_ok(),
            (_, Some(new)) => write(shared, &path, &new).is_ok(),
            (_, None) => false,
        };
        wrote |= done;
    }
    // A block kept on this device alone for a task whose list travels: moved
    // to that list's account, so that every device's agenda shows the task's
    // time, as it shows the task (21 October 2026; `blocks::kept_back`).
    let chosen = load_config().tasks.blocks.unwrap_or_default();
    let stamp = now.timestamp().as_second();
    let mut seen = BTreeSet::new();
    for block in found.iter().filter(|b| b.end > stamp && !b.recurring && !b.with_others) {
        let Some(task) = tasks.iter().find(|t| t.uid == block.task && t.status.is_open() && !t.read_only) else { continue };
        let path = PathBuf::from(&block.key);
        if !seen.insert(path.clone()) || !path.is_file() {
            continue;
        }
        let Some(calendar) = pim::collection_of(&path) else { continue };
        if !blocks::kept_back(&calendar, task.list_id.split('/').next().unwrap_or(""), &chosen) {
            continue;
        }
        let Ok(to) = calendar_for(task) else { continue };
        let Some(name) = path.file_name() else { continue };
        let target = to.dir.join(name);
        if to.account == vdir::LOCAL || to.dir == calendar.dir || target.exists() {
            continue;
        }
        if vdir::move_item(&path, &target).is_ok() {
            pim::nudge(shared, &to.account);
            wrote = true;
        }
    }
    wrote
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A time block given a guest is a meeting with others now (26 October
    /// 2026): none of the window's task changes takes it (`found_of`, read by
    /// Done, Delete, "Make it an appointment", a rename, a pin), while the
    /// task's block of its own still goes; and the door refuses it anyway.
    #[test]
    fn a_block_given_a_guest_is_left_to_its_meeting() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Blocks given a guest", None, &["VEVENT"]).unwrap();
        let now = Zoned::now();
        let day = now.date().tomorrow().unwrap().strftime("%Y%m%d").to_string();
        let block = |uid: &str, extra: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTART:{day}T100000Z\r\nDTEND:{day}T110000Z\r\nSUMMARY:Guest task\r\n{}:guest-task-1\r\n{extra}END:VEVENT\r\nEND:VCALENDAR\r\n", blocks::TASK);
        let (own, met) = (calendar.dir.join("own.ics"), calendar.dir.join("met.ics"));
        std::fs::write(&own, block("own-block", "")).unwrap();
        let meeting = block("met-block", "ORGANIZER:mailto:jane@example.org\r\nATTENDEE:mailto:paul@example.org\r\n");
        std::fs::write(&met, &meeting).unwrap();
        let task = Task { uid: "guest-task-1".into(), title: "Guest task".into(), ..Task::default() };
        let stamp = now.timestamp().as_second();
        let events = sioul_core::agenda::occurrences(stamp - 86_400, stamp + 3 * 86_400);
        let shared = Shared::default();
        let _ = pins(&shared, &events, std::slice::from_ref(&task), &now);
        assert_eq!(with_task("guest-task-1").into_iter().map(|(_, p)| p).collect::<Vec<_>>(), vec![own.clone()], "its own block goes with it, never the meeting");
        assert!(remove(&shared, &met).is_err() && write(&shared, &met, &meeting.replace("SUMMARY:Guest task", "SUMMARY:Renamed")).is_err());
        assert_eq!(std::fs::read_to_string(&met).unwrap(), meeting, "byte for byte");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// A step is pinned from the current five minutes on, never before them.
    #[test]
    fn a_step_is_never_pinned_before_now() {
        let now: i64 = 1_791_200_000 + 7 * 60;
        let turn = now.div_euclid(300) * 300;
        assert!(!before_now(turn, now), "the current five minutes");
        assert!(!before_now(now + 600, now));
        assert!(before_now(turn - 1, now), "before them");
        // A block under way keeps its own start as its length changes from its card; another time before now is refused.
        let under_way = Pin { key: "b.ics".into(), uid: "b".into(), start: turn - 1_800, end: turn + 1_800, date: jiff::civil::date(2026, 10, 5), before: 0, after: 0, read_only: false };
        assert!(may_start(turn - 1_800, now, Some(&under_way)));
        assert!(!may_start(turn - 900, now, Some(&under_way)) && !may_start(turn - 1_800, now, None));
        assert!(may_start(turn + 600, now, None));
    }
}

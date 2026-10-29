// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Legs: a ride or a walk around an appointment, an event of its own that
//! moves with it (composed tasks, part E; docs/tasks.md, "Getting there and
//! coming back"). A leg says what it is in its own file, since the
//! appointment may be one you cannot write (an invitation):
//!
//! ```text
//! RELATED-TO;RELTYPE=PARENT:<the appointment's UID>
//! X-SIOUL-ANCHOR;GAP=PT25M;TIMETABLE=TRUE;WAS=20261029T130000Z:BEFORE
//! X-SIOUL-UNCONFIRMED:20261029T140000Z
//! ```
//!
//! - `BEFORE` or `AFTER`: which side of the appointment it is on. `GAP`: from
//!   the leg's end to the appointment's start (before), from the
//!   appointment's end to the leg's start (after). `WAS`: the appointment's
//!   start (before) or end (after) the leg was last set for. `TIMETABLE`: its
//!   time comes from a timetable. `X-SIOUL-UNCONFIRMED`: a timetable leg moved
//!   with its appointment, its timetable not checked (the instant it was moved
//!   for), said plainly, never asked about.
//! - **It follows** by the one rule of what follows an appointment (`moves`):
//!   a move this device saw shifts it by as much (a leg before by the start's
//!   move, a leg after by the end's), in absolute time, its own length and its
//!   distance kept. A leg moved by hand (anywhere but its own `WAS − GAP`)
//!   keeps its time: its side and distance are taken again from the
//!   appointment as it is. Both at once (two devices, offline): the hand wins.
//!   Running twice writes nothing; the stamps are the appointment's, so two
//!   devices write the same bytes.
//! - **Cancelled** with its appointment: the legs to come are cancelled with
//!   a mark (`X-SIOUL-CANCELLED-WITH`), and come back when it does.
//! - **Never** for an appointment gone, repeating or of a whole day, nor a
//!   leg of a calendar you only read or a meeting with others: nothing is
//!   written, nothing lost.
//! - **Read**: the appointment's time kept before and after comes from its
//!   legs while it has some (`mark`): from the earliest leg's start, to the
//!   latest leg's end; its own margins come back without legs.
//!
//! Other programs see a plain event: they keep the lines and do nothing with
//! them (docs/compatibility.md).

use crate::agenda::{self, EventRef, Occurrence};
use crate::moves::{self, Instants, Pull, Verdict};
use jiff::Zoned;
use jiff::tz::TimeZone;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// A leg's distance to its appointment, and which side it is on.
pub const ANCHOR: &str = "X-SIOUL-ANCHOR";
/// A timetable leg moved with its appointment, its timetable not checked.
pub const UNCONFIRMED: &str = "X-SIOUL-UNCONFIRMED";
/// A leg cancelled with its appointment: the appointment's UID, to bring it back with it.
pub const CANCELLED_WITH: &str = "X-SIOUL-CANCELLED-WITH";

/// Which side of its appointment a leg is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Before,
    After,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Side::Before => "BEFORE",
            Side::After => "AFTER",
        }
    }
}

/// What a leg's file says of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// The appointment's UID (`RELATED-TO;RELTYPE=PARENT`).
    pub appointment: String,
    pub side: Side,
    /// Seconds: from the leg's end to the appointment's start (before), from
    /// the appointment's end to the leg's start (after).
    pub gap: i64,
    pub timetable: bool,
    /// The appointment's start (before) or end (after) it was last set for.
    pub was: Option<i64>,
    /// Moved with the appointment, its timetable not checked: the instant it was moved for.
    pub unconfirmed: Option<i64>,
    /// Cancelled with its appointment.
    pub cancelled_with: bool,
}

/// The lines of the file's first VEVENT or VTODO, its alarms aside.
fn own_lines(text: &str) -> Vec<String> {
    let source = crate::lines::unfold(text);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut inside = false;
    for line in source {
        let name = crate::lines::name(&line);
        let value = crate::lines::value(&line).trim().to_ascii_uppercase();
        match name.as_str() {
            "BEGIN" if !inside && (value == "VEVENT" || value == "VTODO") => inside = true,
            "BEGIN" if inside => depth += 1,
            "END" if inside && depth > 0 => depth -= 1,
            "END" if inside => break,
            _ if inside && depth == 0 => out.push(line),
            _ => {}
        }
    }
    out
}

/// A leg's anchor, read from its file; None for anything else (no anchor,
/// or no appointment it belongs to).
pub fn parse(text: &str) -> Option<Anchor> {
    let lines = own_lines(text);
    let line = lines.iter().find(|l| crate::lines::name(l) == ANCHOR)?;
    let side = match crate::lines::value(line).trim().to_ascii_uppercase().as_str() {
        "AFTER" => Side::After,
        _ => Side::Before,
    };
    let appointment = lines.iter().filter(|l| crate::lines::name(l) == "RELATED-TO").map(|l| crate::tasks::relation_of(l)).find(|r| r.kind == "PARENT")?.uid;
    let gap = crate::lines::param(line, "GAP").and_then(|g| crate::tasks::minutes_of(&g)).unwrap_or(0) * 60;
    let timetable = crate::lines::param(line, "TIMETABLE").is_some_and(|t| t.trim().eq_ignore_ascii_case("TRUE"));
    let was = crate::lines::param(line, "WAS").and_then(|w| moves::read_utc(&w));
    let unconfirmed = lines.iter().find(|l| crate::lines::name(l) == UNCONFIRMED).and_then(|l| moves::read_utc(crate::lines::value(l)));
    let cancelled_with = lines.iter().any(|l| crate::lines::name(l) == CANCELLED_WITH);
    Some(Anchor { appointment, side, gap, timetable, was, unconfirmed, cancelled_with })
}

/// The anchor's line.
pub fn line(anchor: &Anchor) -> String {
    let timetable = if anchor.timetable { ";TIMETABLE=TRUE" } else { "" };
    let was = anchor.was.map(|w| format!(";WAS={}", moves::utc(w))).unwrap_or_default();
    format!("{ANCHOR};GAP={}{timetable}{was}:{}", crate::tasks::duration_of(anchor.gap.div_euclid(60)), anchor.side.word())
}

/// The appointment's instant a leg on `side` follows: its start, or its end.
fn instant(side: Side, appointment: Instants) -> i64 {
    match side {
        Side::Before => appointment.start,
        Side::After => appointment.end,
    }
}

/// Whether the leg is where its anchor last set it (not moved by hand since).
fn untouched(leg: Instants, anchor: &Anchor) -> bool {
    match (anchor.side, anchor.was) {
        (Side::Before, Some(was)) => leg.end == was - anchor.gap,
        (Side::After, Some(was)) => leg.start == was + anchor.gap,
        (_, None) => true,
    }
}

/// Where an untouched leg will be once followed (`appointment` as it is
/// now): every read uses it, so that a leg not yet written never makes a
/// reminder or a margin late. A leg moved by hand stays where it is.
pub fn effective(leg: Instants, anchor: &Anchor, appointment: Instants) -> Instants {
    match anchor.was {
        Some(was) if untouched(leg, anchor) => {
            let by = instant(anchor.side, appointment) - was;
            Instants { start: leg.start + by, end: leg.end + by }
        }
        _ => leg,
    }
}

/// The anchor for a leg at `leg` beside `appointment`, as it stands now:
/// its side (before when it ends by the start, after when it starts from the
/// end, else as it was), its distance, `WAS` the instant it follows.
fn anchored(leg: Instants, appointment: Instants, was: &Anchor) -> Anchor {
    let side = if leg.end <= appointment.start {
        Side::Before
    } else if leg.start >= appointment.end {
        Side::After
    } else {
        was.side
    };
    let gap = match side {
        Side::Before => appointment.start - leg.end,
        Side::After => leg.start - appointment.end,
    };
    Anchor { side, gap, was: Some(instant(side, appointment)), unconfirmed: None, ..was.clone() }
}

/// An appointment as its legs follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Followed {
    pub uid: String,
    pub path: PathBuf,
    pub instants: Instants,
    pub cancelled: bool,
    pub recurring: bool,
    pub all_day: bool,
    /// Its LAST-MODIFIED, else its DTSTAMP, in UTC: the stamps a leg moved with it takes.
    pub stamp: String,
}

impl Followed {
    /// The appointment of the event file `path`, as it is now; None when it does not read.
    pub fn read(path: &Path, zone: &TimeZone) -> Option<Followed> {
        let text = std::fs::read_to_string(path).ok()?;
        Followed::of(&text, path, zone)
    }

    pub fn of(text: &str, path: &Path, zone: &TimeZone) -> Option<Followed> {
        let reference = agenda::event_ref(text, &path.display().to_string(), zone)?;
        let (start, end) = agenda::first_times(text, zone)?;
        let all_day = agenda::edit_of_text(text, zone).is_some_and(|e| e.all_day);
        let lines = own_lines(text);
        let stamp_of = |name: &str| lines.iter().find(|l| crate::lines::name(l) == name).map(|l| crate::lines::value(l).trim().to_string()).filter(|v| moves::read_utc(v).is_some());
        let stamp = stamp_of("LAST-MODIFIED").or_else(|| stamp_of("DTSTAMP")).unwrap_or_default();
        Some(Followed { uid: reference.uid, path: path.to_path_buf(), instants: Instants { start, end }, cancelled: reference.cancelled, recurring: reference.recurring, all_day, stamp })
    }

    /// Whether legs follow it at all: one time, at a time.
    fn followable(&self) -> bool {
        !self.recurring && !self.all_day
    }
}

/// The leg's text with its times `to` and its anchor `anchor`, the
/// appointment's stamps on it; None when nothing changes or it does not read.
fn rewritten(text: &str, to: Instants, anchor: &Anchor, stamp: &str, zone: &TimeZone) -> Option<String> {
    let (start, end) = agenda::first_times(text, zone)?;
    let timed = if (start, end) == (to.start, to.end) { text.to_string() } else { agenda::retimed_text(text, to.start, to.end, Some(stamp).filter(|s| !s.is_empty()), zone)? };
    let ours = |l: &str| matches!(crate::lines::name(l).as_str(), n if n == ANCHOR || n == UNCONFIRMED);
    let without = agenda::remove_lines(&timed, ours)?;
    let mut added = vec![line(anchor)];
    added.extend(anchor.unconfirmed.map(|at| format!("{UNCONFIRMED}:{}", moves::utc(at))));
    let out = agenda::add_lines(&without, &added)?;
    (out != text).then_some(out)
}

/// The leg's text cancelled with its appointment, or brought back with it;
/// None when nothing changes.
fn cancelled(text: &str, appointment: &Followed, now: i64, zone: &TimeZone) -> Option<String> {
    let anchor = parse(text)?;
    let (_, end) = agenda::first_times(text, zone)?;
    let status = own_lines(text).iter().find(|l| crate::lines::name(l) == "STATUS").map(|l| crate::lines::value(l).trim().to_ascii_uppercase()).unwrap_or_default();
    if appointment.cancelled && status != "CANCELLED" && end > now {
        let without = agenda::remove_lines(text, |l| crate::lines::name(l) == "STATUS")?;
        return agenda::add_lines(&without, &["STATUS:CANCELLED".into(), format!("{CANCELLED_WITH}:{}", appointment.uid)]);
    }
    if !appointment.cancelled && anchor.cancelled_with {
        return agenda::remove_lines(text, |l| matches!(crate::lines::name(l).as_str(), "STATUS") || crate::lines::name(l) == CANCELLED_WITH);
    }
    None
}

/// What becomes of one leg at a pass of the follow machinery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// As it should be: nothing written.
    Nothing,
    /// Its new text.
    Write(String),
    /// Set for an instant this device never saw: nothing written yet.
    Wait,
}

/// One leg (`text`) at a pass of the follow machinery (`moves::upkeep`), its
/// appointment as it is (`appointment`), as this device last saw it (`seen`):
/// moved by hand, its anchor taken again; untouched and its appointment moved,
/// as the one rule says (`moves::verdict`: `since`, `pull`); cancelled or
/// brought back with it.
pub fn upkept(text: &str, appointment: &Followed, seen: Option<Instants>, since: i64, pull: Pull, now: &Zoned) -> Step {
    let zone = now.time_zone();
    let Some(anchor) = parse(text) else { return Step::Nothing };
    if anchor.appointment != appointment.uid || !appointment.followable() {
        return Step::Nothing;
    }
    if let Some(changed) = cancelled(text, appointment, now.timestamp().as_second(), zone) {
        return Step::Write(changed);
    }
    let Some((start, end)) = agenda::first_times(text, zone) else { return Step::Nothing };
    let leg = Instants { start, end };
    // Over: what happened stays as it was, whatever its appointment's record becomes (review of ab7daa4, later 9).
    if end <= now.timestamp().as_second() {
        return Step::Nothing;
    }
    let at = instant(anchor.side, appointment.instants);
    if !untouched(leg, &anchor) {
        // Moved by hand elsewhere, maybe with its appointment whose copy is not here yet: taken
        // again only once a pull begun after the doubt went through, as a follow waits (review of
        // ab7daa4, later 1), never set from an older copy.
        if moves::verdict(false, false, since, pull) == Verdict::Wait {
            return Step::Wait;
        }
        // Moved by hand: its time kept, its side and distance from the appointment now; the hand wins.
        let set = anchored(leg, appointment.instants, &anchor);
        return rewritten(text, leg, &set, &appointment.stamp, zone).map_or(Step::Nothing, Step::Write);
    }
    let Some(was) = anchor.was else {
        // Set before it kept an instant: as it stands now.
        let set = anchored(leg, appointment.instants, &anchor);
        return rewritten(text, leg, &set, &appointment.stamp, zone).map_or(Step::Nothing, Step::Write);
    };
    let saw = seen.is_some_and(|s| instant(anchor.side, s) == was);
    match moves::verdict(was == at, saw, since, pull) {
        Verdict::Consistent => Step::Nothing,
        Verdict::Wait => Step::Wait,
        Verdict::Follow => followed(text, &anchor, leg, appointment).map_or(Step::Nothing, Step::Write),
    }
}

/// The untouched leg moved with its appointment: by as much as the instant
/// it follows, its `WAS` that instant, a timetable leg marked unconfirmed.
fn followed(text: &str, anchor: &Anchor, leg: Instants, appointment: &Followed) -> Option<String> {
    let at = instant(anchor.side, appointment.instants);
    let by = at - anchor.was?;
    let to = Instants { start: leg.start + by, end: leg.end + by };
    let set = Anchor { was: Some(at), unconfirmed: if anchor.timetable { Some(at) } else { None }, ..anchor.clone() };
    // The appointment's zone for the times: the leg's own form is kept by `retimed_text`.
    rewritten(text, to, &set, &appointment.stamp, &TimeZone::UTC)
}

/// A leg moved with its appointment in one gesture (`pim::move_event`, the
/// event's form, the MCP's `update_event`): from `was` (the appointment as it
/// was) to `appointment` (as written now); a leg moved by hand meanwhile
/// keeps its time, its anchor taken again. None when nothing changes.
pub fn moved_with(text: &str, was: Instants, appointment: &Followed, zone: &TimeZone) -> Option<String> {
    moved_with_at(text, was, appointment, zone, jiff::Timestamp::now().as_second())
}

/// `moved_with` at `now` (Unix seconds): a leg already over stays as it was.
fn moved_with_at(text: &str, was: Instants, appointment: &Followed, zone: &TimeZone, now: i64) -> Option<String> {
    let anchor = parse(text)?;
    if anchor.appointment != appointment.uid || !appointment.followable() {
        return None;
    }
    let (start, end) = agenda::first_times(text, zone)?;
    if end <= now {
        return None;
    }
    let leg = Instants { start, end };
    let before = Anchor { was: anchor.was.or(Some(instant(anchor.side, was))), ..anchor.clone() };
    if !untouched(leg, &before) {
        return rewritten(text, leg, &anchored(leg, appointment.instants, &anchor), &appointment.stamp, zone);
    }
    followed(text, &before, leg, appointment)
}

/// A leg's anchor after it was moved by hand here (a drag, its form): its
/// time as it is now, its side and distance taken from `appointment`, its
/// doubt gone. None when nothing changes.
pub fn set_by_hand(text: &str, appointment: Instants, zone: &TimeZone) -> Option<String> {
    let anchor = parse(text)?;
    let (start, end) = agenda::first_times(text, zone)?;
    let leg = Instants { start, end };
    let lines = own_lines(text);
    let stamp = lines.iter().find(|l| crate::lines::name(l) == "LAST-MODIFIED").map(|l| crate::lines::value(l).trim().to_string()).unwrap_or_default();
    rewritten(text, leg, &anchored(leg, appointment, &anchor), &stamp, zone)
}

/// A new leg: the event `edit`, belonging to `appointment` on `side`, its
/// distance from the times given, in the appointment's projects (a leg is
/// its appointment's, for consent too).
pub fn new_leg(edit: &agenda::EventEdit, appointment: &Followed, projects: &[String], side: Side, timetable: bool, zone: &TimeZone) -> Result<String, String> {
    if !appointment.followable() {
        return Err("leg-repeating".into());
    }
    let text = agenda::new_event(edit, zone)?;
    let (start, end) = agenda::first_times(&text, zone).ok_or("leg-times")?;
    let leg = Instants { start, end };
    let base = Anchor { appointment: appointment.uid.clone(), side, gap: 0, timetable, was: None, unconfirmed: None, cancelled_with: false };
    let anchor = Anchor { side, ..anchored(leg, appointment.instants, &base) };
    let anchor = Anchor {
        gap: match side {
            Side::Before => appointment.instants.start - leg.end,
            Side::After => leg.start - appointment.instants.end,
        },
        was: Some(instant(side, appointment.instants)),
        ..anchor
    };
    let mut added = vec![format!("RELATED-TO;RELTYPE=PARENT:{}", appointment.uid), line(&anchor)];
    added.extend(projects.iter().map(|p| crate::tasks::project_line(p)));
    agenda::add_lines(&text, &added).ok_or_else(|| "leg-times".to_string())
}

/// A new leg made a step's time (composed tasks §4.2, the unified agenda's
/// part 7): the leg the step's time block (`blocks::TASK` and its
/// `block-of` link), the step pinned to it (`blocks::task_pinned`, its other
/// blocks' links out): the leg and the step's text, for one change. Owned by
/// its step, the leg is never moved by the plan nor taken out when the step
/// is done (`blocks::upkeep` leaves a leg alone). Only once every device
/// keeps such a leg (`steps_offered`). Refused for a step done or read only.
pub fn leg_for_step(leg: &str, step: &crate::tasks::Task, step_text: &str, now: &Zoned) -> Result<(String, String), String> {
    if !step.status.is_open() {
        return Err("leg-step-done".into());
    }
    if step.read_only {
        return Err("leg-step-read-only".into());
    }
    let uid = agenda::event_ref(leg, "", now.time_zone()).map(|r| r.uid).ok_or("leg-times")?;
    let block_of = crate::tasks::link_line(&crate::tasks::Link { uri: format!("uid:{}", step.uid), label: String::new(), rel: crate::blocks::BLOCK_OF.to_string() });
    let leg = agenda::add_lines(leg, &[format!("{}:{}", crate::blocks::TASK, step.uid), block_of]).ok_or("leg-times")?;
    let pinned = crate::blocks::task_pinned(step_text, Some(&uid), now)?;
    Ok((leg, pinned))
}

/// Whether a leg may be a step's time here, as the window last found it
/// (the sharing's devices: `sioul_sync::share::legs_as_steps`), written for
/// `sioul mcp` too: no word written, not yet (an older Sioul on another
/// device would take such a leg out once its step is done).
pub fn steps_offered() -> bool {
    std::fs::read_to_string(steps_flag()).is_ok_and(|t| t.trim() == "yes")
}

/// `steps_offered` said, written only when it changes.
pub fn say_steps_offered(offered: bool) {
    let word = if offered { "yes" } else { "no" };
    if std::fs::read_to_string(steps_flag()).is_ok_and(|t| t.trim() == word) {
        return;
    }
    let path = steps_flag();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, format!("{word}\n"));
}

fn steps_flag() -> PathBuf {
    crate::config::state_dir().join("legs-as-steps")
}

/// A leg untied from its appointment: an event of its own, its anchor and
/// its belonging gone. None when it is no leg.
pub fn untied(text: &str) -> Option<String> {
    let anchor = parse(text)?;
    let ours = |l: &str| {
        let name = crate::lines::name(l);
        name == ANCHOR || name == UNCONFIRMED || name == CANCELLED_WITH || (name == "RELATED-TO" && crate::tasks::relation_of(l).kind == "PARENT" && crate::tasks::relation_of(l).uid == anchor.appointment)
    };
    agenda::remove_lines(text, ours)
}

/// A leg as its appointment's view reads it (`pairs::appointment_view`):
/// its file, title, times as written, and anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegSeen {
    pub uid: String,
    pub key: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub anchor: Anchor,
}

/// The legs of the appointment `uid` among occurrences read together (the agenda's).
pub fn legs_seen_in(uid: &str, occurrences: &[Occurrence]) -> Vec<LegSeen> {
    occurrences.iter().filter_map(|o| o.anchor.as_ref().filter(|a| a.appointment == uid && !o.cancelled).map(|a| LegSeen { uid: o.uid.clone(), key: o.key.clone(), title: o.summary.clone(), start: o.start, end: o.end, anchor: a.clone() })).collect()
}

/// The legs of the appointment `uid` among `events`, each read from its file
/// (an appointment's card, read alone).
pub fn legs_seen_of(uid: &str, events: &[EventRef], zone: &TimeZone) -> Vec<LegSeen> {
    legs_of(uid, events)
        .into_iter()
        .filter(|leg| !leg.cancelled)
        .filter_map(|leg| {
            let text = std::fs::read_to_string(&leg.key).ok()?;
            let (start, end) = agenda::first_times(&text, zone)?;
            Some(LegSeen { uid: leg.uid.clone(), key: leg.key.clone(), title: leg.summary.clone(), start, end, anchor: leg.anchor.clone()? })
        })
        .collect()
}

/// What a leg is to its appointment, as its own card says it: "Before Doctor
/// (14:00) · moves with it", and its doubt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegOf {
    pub uid: String,
    pub key: String,
    pub title: String,
    pub start: i64,
    pub side: Side,
    pub unconfirmed: bool,
    /// "Before Doctor (14:00) · moves with it".
    pub line: String,
}

/// The leg `text` to its appointment, found among `events`; None for an
/// event that is no leg, or whose appointment is not here.
pub fn leg_of(text: &str, events: &[EventRef], tr: &crate::i18n::Translator, zone: &TimeZone) -> Option<LegOf> {
    let anchor = parse(text)?;
    let appointment = events.iter().find(|e| e.uid == anchor.appointment && e.anchor.is_none())?;
    let followed = Followed::read(Path::new(&appointment.key), zone)?;
    let clock = jiff::Timestamp::from_second(followed.instants.start).ok()?.to_zoned(zone.clone()).strftime("%H:%M").to_string();
    let mut args = crate::i18n::args();
    args.set("appointment", appointment.summary.clone());
    args.set("time", clock);
    let side = tr.text(if anchor.side == Side::Before { "leg-before" } else { "leg-after" }, Some(&args));
    Some(LegOf { uid: appointment.uid.clone(), key: appointment.key.clone(), title: appointment.summary.clone(), start: followed.instants.start, side: anchor.side, unconfirmed: anchor.unconfirmed.is_some(), line: format!("{side} · {}", tr.text("leg-moves-with", None)) })
}

/// A step's text said to come after its appointment (`after`), or before it
/// (the usual, nothing written): `X-SIOUL-ANCHOR:AFTER` (part E6; another
/// program keeps the line as it is, an older Sioul too, and lays the step
/// before, as it lays every step).
pub fn step_side(text: &str, after: bool, now: &Zoned) -> Result<String, String> {
    crate::tasks::replace_lines(text, |l| crate::lines::name(l) == ANCHOR, if after { vec![format!("{ANCHOR}:AFTER")] } else { Vec::new() }, now)
}

/// The legs of the appointment `uid` among `events`.
pub fn legs_of<'a>(uid: &str, events: &'a [EventRef]) -> Vec<&'a EventRef> {
    events.iter().filter(|e| e.anchor.as_ref().is_some_and(|a| a.appointment == uid)).collect()
}

/// Each appointment's time kept before and after from its legs, among
/// `occurrences` read together (`agenda::occurrences`): from the earliest
/// leg's start before it, to the latest leg's end after it, each where it
/// will be once followed (`effective`); its own stored margins without legs
/// (D18). A leg with its appointment there is marked so (`leg_of`), and the
/// appointment says when a leg's timetable was not checked (`legs_doubt`).
/// A leg cancelled, of an appointment repeating or of a whole day, counts for
/// nothing.
pub fn mark(occurrences: &mut [Occurrence]) {
    let appointments: Vec<(usize, String, Instants)> = occurrences.iter().enumerate().filter(|(_, o)| !o.recurring && !o.all_day && o.anchor.is_none()).map(|(i, o)| (i, o.uid.clone(), Instants { start: o.start, end: o.end })).collect();
    let mut found: Vec<(usize, usize, Instants, bool)> = Vec::new();
    for (j, leg) in occurrences.iter().enumerate() {
        let Some(anchor) = leg.anchor.as_ref() else { continue };
        if leg.cancelled || leg.all_day {
            continue;
        }
        let Some(&(i, _, at)) = appointments.iter().find(|(_, uid, _)| *uid == anchor.appointment) else { continue };
        found.push((i, j, effective(Instants { start: leg.start, end: leg.end }, anchor, at), anchor.unconfirmed.is_some()));
    }
    for &(i, j, _, _) in &found {
        let uid = occurrences[i].uid.clone();
        occurrences[j].leg_of = uid;
    }
    for &(i, _, _, _) in &found {
        let o = &occurrences[i];
        let (start, end) = (o.start, o.end);
        let before: Vec<&(usize, usize, Instants, bool)> = found.iter().filter(|f| f.0 == i && f.2.end <= start).collect();
        let after: Vec<&(usize, usize, Instants, bool)> = found.iter().filter(|f| f.0 == i && f.2.start >= end).collect();
        let doubt = found.iter().any(|f| f.0 == i && f.3);
        let o = &mut occurrences[i];
        if let Some(earliest) = before.iter().map(|f| f.2.start).min() {
            o.margins.before = u32::try_from((start - earliest).max(0) / 60).unwrap_or(u32::MAX);
        }
        if let Some(latest) = after.iter().map(|f| f.2.end).max() {
            o.margins.after = u32::try_from((latest - end).max(0) / 60).unwrap_or(u32::MAX);
        }
        o.legs_doubt = doubt;
    }
}

/// The follow machinery's pass over the legs of one appointment, for
/// `moves::upkeep`: each leg written as `upkept` says, through the writer's
/// one rule (`composed::Change::write`: never a meeting with others, nor a
/// calendar you only read). The files written; whether every leg is settled
/// (written or needing nothing), and the instants of those that wait (each its
/// own doubt, `since` giving when it began).
pub fn follow_legs(legs: &[&EventRef], appointment: &Followed, seen: Option<Instants>, since: &dyn Fn(i64) -> i64, pull: Pull, now: &Zoned) -> (Vec<PathBuf>, bool, Vec<i64>) {
    let mut written = Vec::new();
    let (mut settled, mut waits) = (true, Vec::new());
    for leg in legs {
        let path = PathBuf::from(&leg.key);
        let Ok(text) = std::fs::read_to_string(&path) else {
            settled = false;
            continue;
        };
        // A leg you cannot change here (a meeting with others, a calendar you only read) stays.
        if agenda::guard(&path, agenda::Change::Write(&text)).is_err() {
            continue;
        }
        // The doubt of this leg's own instant (`WAS`): a leg written later has its own.
        let set_for = parse(&text).and_then(|a| a.was).unwrap_or(0);
        match upkept(&text, appointment, seen, since(set_for), pull, now) {
            Step::Nothing => {}
            Step::Wait => waits.push(set_for),
            Step::Write(new) => match (crate::composed::Change { written: vec![(path.clone(), new)], ..crate::composed::Change::default() }).write() {
                Ok(_) => written.push(path),
                Err(_) => settled = false,
            },
        }
    }
    (written, settled, waits)
}

/// Every leg of the appointments the gesture moved (`moved`: the appointment's
/// UID, as it was, its file as written now): the legs' new texts, for the same
/// change and the same Undo (`moved_with`), and `seen` noted at the new
/// instants, so that the next pass writes nothing.
pub fn with_moved(events: &[EventRef], uid: &str, was: Instants, appointment: &Followed, zone: &TimeZone) -> Vec<(PathBuf, String)> {
    legs_of(uid, events)
        .into_iter()
        .filter_map(|leg| {
            let path = PathBuf::from(&leg.key);
            let text = std::fs::read_to_string(&path).ok()?;
            agenda::guard(&path, agenda::Change::Write(&text)).ok()?;
            moved_with(&text, was, appointment, zone).map(|new| (path, new))
        })
        .collect()
}

/// What a gesture on one event writes (part E2: a drag, its form, an
/// agent's change), beside its own file: its own text as written (a leg moved
/// by hand: its anchor taken again from its appointment, its doubt gone), and
/// each leg of an appointment whose times changed, moved with it in the same
/// change, for one Undo (`with_moved`). Each leg then says where it is set
/// for, so the next pass of the follow machinery writes nothing for it; the
/// tasks tied to the appointment follow at that pass, a move this device saw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gesture {
    pub own: String,
    pub legs: Vec<(PathBuf, String)>,
}

/// The gesture on `path` from `before` to `after` (see `Gesture`), the legs
/// found among `events` (each event's ties: `EventRef::anchor`).
pub fn gesture(path: &Path, before: &str, after: &str, events: &[EventRef], zone: &TimeZone) -> Gesture {
    let moved = agenda::first_times(before, zone) != agenda::first_times(after, zone);
    let mut out = Gesture { own: after.to_string(), legs: Vec::new() };
    if !moved {
        return out;
    }
    if let Some(anchor) = parse(after) {
        // A leg moved by hand: its distance from its appointment as it is.
        if let Some(appointment) = events.iter().find(|e| e.uid == anchor.appointment && e.anchor.is_none()).and_then(|e| Followed::read(Path::new(&e.key), zone))
            && let Some(set) = set_by_hand(after, appointment.instants, zone)
        {
            out.own = set;
        }
        return out;
    }
    let (Some((start, end)), Some(appointment)) = (agenda::first_times(before, zone), Followed::of(after, path, zone)) else { return out };
    out.legs = with_moved(events, &appointment.uid, Instants { start, end }, &appointment, zone);
    out
}

/// The legs of the appointment `uid` to come at `now` that can be changed
/// (never one of a calendar you only read, nor a meeting with others): what
/// goes with it when it is deleted, under the same Undo (part E2). Those
/// already over stay.
pub fn to_come(uid: &str, events: &[EventRef], now: i64, zone: &TimeZone) -> Vec<PathBuf> {
    legs_of(uid, events)
        .into_iter()
        .map(|leg| PathBuf::from(&leg.key))
        .filter(|path| {
            let Ok(text) = std::fs::read_to_string(path) else { return false };
            agenda::guard(path, agenda::Change::Delete).is_ok() && agenda::first_times(&text, zone).is_some_and(|(start, _)| start > now)
        })
        .collect()
}

/// A leg's title as a gesture says it moved: "Bus 24", or "Bus 24 (timetable
/// not checked)" for a timetable leg moved by arithmetic (`legs-unconfirmed`).
pub fn moved_name(text: &str, tr: &crate::i18n::Translator) -> String {
    let title = agenda::edit_of_text(text, &TimeZone::UTC).map(|e| e.title).unwrap_or_default();
    if parse(text).is_some_and(|a| a.unconfirmed.is_some()) {
        let mut args = crate::i18n::args();
        args.set("leg", title);
        tr.text("legs-unconfirmed", Some(&args))
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vdir::{self, Kind};

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn t(at: &str) -> i64 {
        at.parse::<jiff::civil::DateTime>().unwrap().to_zoned(paris()).unwrap().timestamp().as_second()
    }

    fn now() -> Zoned {
        "2026-10-25T10:00:00+01:00[Europe/Paris]".parse().unwrap()
    }

    fn event(uid: &str, start: &str, end: &str, lines: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261020T100000Z\r\nLAST-MODIFIED:20261020T100000Z\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{uid}\r\n{lines}END:VEVENT\r\nEND:VCALENDAR\r\n", moves::utc(t(start)), moves::utc(t(end)))
    }

    fn appointment(start: &str, end: &str) -> Followed {
        let text = event("doctor", start, end, "");
        Followed::of(&text, Path::new("/c/doctor.ics"), &paris()).unwrap()
    }

    /// A bus before the doctor (14:00–14:30 on 29 October), 25 minutes before it.
    fn bus() -> String {
        event("bus", "2026-10-29T13:10", "2026-10-29T13:35", &format!("RELATED-TO;RELTYPE=PARENT:doctor\r\nX-SIOUL-ANCHOR;GAP=PT25M;TIMETABLE=TRUE;WAS={}:BEFORE\r\n", moves::utc(t("2026-10-29T14:00"))))
    }

    fn times(text: &str) -> (i64, i64) {
        agenda::first_times(text, &paris()).unwrap()
    }

    #[test]
    fn an_anchor_reads_and_writes_the_same() {
        let a = parse(&bus()).unwrap();
        assert_eq!((a.appointment.as_str(), a.side, a.gap, a.timetable, a.was), ("doctor", Side::Before, 25 * 60, true, Some(t("2026-10-29T14:00"))));
        assert_eq!(line(&a), format!("X-SIOUL-ANCHOR;GAP=PT25M;TIMETABLE=TRUE;WAS={}:BEFORE", moves::utc(t("2026-10-29T14:00"))));
        assert!(parse(&event("plain", "2026-10-29T13:10", "2026-10-29T13:35", "")).is_none());
    }

    /// Moves of the start, the end, the day and across the clock change: a leg
    /// before follows the start, a leg after the end, by as much in absolute
    /// time, its length and distance kept; a timetable leg is marked unconfirmed.
    #[test]
    fn legs_follow_the_start_the_end_and_the_day() {
        let seen = Some(Instants { start: t("2026-10-29T14:00"), end: t("2026-10-29T14:30") });
        let pull = Pull { syncs: true, began: None };
        // The start moved to 15:00: the bus at 14:10–14:35, unconfirmed.
        let later = appointment("2026-10-29T15:00", "2026-10-29T15:30");
        let Step::Write(moved) = upkept(&bus(), &later, seen, 0, pull, &now()) else { panic!("followed") };
        assert_eq!(times(&moved), (t("2026-10-29T14:10"), t("2026-10-29T14:35")));
        let a = parse(&moved).unwrap();
        assert_eq!((a.was, a.unconfirmed, a.gap), (Some(t("2026-10-29T15:00")), Some(t("2026-10-29T15:00")), 25 * 60));
        assert!(moved.contains("DTSTAMP:20261020T100000Z"), "the appointment's stamps: {moved}");
        // Twice: nothing.
        assert_eq!(upkept(&moved, &later, Some(later.instants), 0, pull, &now()), Step::Nothing);
        // The end alone moved: a leg before stays.
        let longer = appointment("2026-10-29T14:00", "2026-10-29T15:00");
        assert_eq!(upkept(&bus(), &longer, seen, 0, pull, &now()), Step::Nothing);
        // A leg after follows the end.
        let home = event("home", "2026-10-29T14:40", "2026-10-29T15:05", &format!("RELATED-TO;RELTYPE=PARENT:doctor\r\nX-SIOUL-ANCHOR;GAP=PT10M;WAS={}:AFTER\r\n", moves::utc(t("2026-10-29T14:30"))));
        let Step::Write(home_moved) = upkept(&home, &longer, seen, 0, pull, &now()) else { panic!("the leg after follows the end") };
        assert_eq!(times(&home_moved), (t("2026-10-29T15:10"), t("2026-10-29T15:35")));
        assert!(parse(&home_moved).unwrap().unconfirmed.is_none(), "no timetable: no doubt");
        // To another day, across the clock change (28 March 2027, an hour lost; a leg still to
        // come, since one over stays): absolute time.
        let seen_27 = Some(Instants { start: t("2027-03-27T14:00"), end: t("2027-03-27T14:30") });
        let bus_27 = event("bus", "2027-03-27T13:10", "2027-03-27T13:35", &format!("RELATED-TO;RELTYPE=PARENT:doctor\r\nX-SIOUL-ANCHOR;GAP=PT25M;WAS={}:BEFORE\r\n", moves::utc(t("2027-03-27T14:00"))));
        let next = appointment("2027-03-29T14:00", "2027-03-29T14:30");
        let Step::Write(across) = upkept(&bus_27, &next, seen_27, 0, pull, &now()) else { panic!("across the change") };
        assert_eq!(times(&across), (t("2027-03-29T13:10"), t("2027-03-29T13:35")), "13:10 on the clock, both sides of the change");
    }

    /// The four who-moved cases: neither (nothing), the appointment (follow),
    /// the leg by hand (its anchor taken again, its doubt gone), both (the hand wins).
    #[test]
    fn who_moved_in_the_four_cases() {
        let seen = Some(Instants { start: t("2026-10-29T14:00"), end: t("2026-10-29T14:30") });
        let pull = Pull { syncs: true, began: Some(1) };
        let same = appointment("2026-10-29T14:00", "2026-10-29T14:30");
        assert_eq!(upkept(&bus(), &same, seen, 0, pull, &now()), Step::Nothing, "neither");
        let by_hand = bus().replace(&moves::utc(t("2026-10-29T13:10")), &moves::utc(t("2026-10-29T13:00"))).replace(&moves::utc(t("2026-10-29T13:35")), &moves::utc(t("2026-10-29T13:25")));
        let Step::Write(kept) = upkept(&by_hand, &same, seen, 0, pull, &now()) else { panic!("its anchor taken again") };
        assert_eq!(times(&kept), (t("2026-10-29T13:00"), t("2026-10-29T13:25")), "its time kept");
        assert_eq!(parse(&kept).unwrap().gap, 35 * 60);
        // Both: the appointment moved to 15:00 and the leg by hand: the hand wins.
        let later = appointment("2026-10-29T15:00", "2026-10-29T15:30");
        let Step::Write(both) = upkept(&by_hand, &later, seen, 0, pull, &now()) else { panic!("the hand wins") };
        assert_eq!(times(&both), (t("2026-10-29T13:00"), t("2026-10-29T13:25")));
        let a = parse(&both).unwrap();
        assert_eq!((a.gap, a.was, a.unconfirmed), (95 * 60, Some(t("2026-10-29T15:00")), None));
        assert_eq!(upkept(&both, &later, Some(later.instants), 0, pull, &now()), Step::Nothing, "twice: nothing");
    }

    /// A leg moved by hand elsewhere together with its appointment, the leg's
    /// file come first (review of ab7daa4, later 1): it waits for a pull begun
    /// after, then is set from the appointment as it came, written once, where
    /// it was put; on a calendar that does not sync, at once.
    #[test]
    fn a_leg_moved_by_hand_with_its_appointment_waits_for_it() {
        let stale = appointment("2026-10-29T14:00", "2026-10-29T14:30");
        let by_hand = bus().replace(&moves::utc(t("2026-10-29T13:10")), &moves::utc(t("2026-10-29T13:00"))).replace(&moves::utc(t("2026-10-29T13:35")), &moves::utc(t("2026-10-29T13:25")));
        assert_eq!(upkept(&by_hand, &stale, Some(stale.instants), 100, Pull { syncs: true, began: Some(50) }, &now()), Step::Wait, "never set from the older copy");
        let came = appointment("2026-10-29T15:00", "2026-10-29T15:30");
        let Step::Write(set) = upkept(&by_hand, &came, Some(stale.instants), 100, Pull { syncs: true, began: Some(200) }, &now()) else { panic!("set once the pull went through") };
        assert_eq!(times(&set), (t("2026-10-29T13:00"), t("2026-10-29T13:25")), "where it was put");
        assert_eq!(parse(&set).unwrap().gap, 95 * 60);
        assert_eq!(upkept(&set, &came, Some(came.instants), 100, Pull { syncs: true, began: Some(200) }, &now()), Step::Nothing, "written once");
        assert!(matches!(upkept(&by_hand, &stale, Some(stale.instants), 100, Pull::default(), &now()), Step::Write(_)), "a calendar that does not sync: at once");
    }

    /// A leg already over stays as it was, whatever its appointment's record
    /// becomes (review of ab7daa4, later 9).
    #[test]
    fn a_leg_over_does_not_follow() {
        let later = appointment("2026-10-29T15:00", "2026-10-29T15:30");
        let after_it: Zoned = "2026-10-29T18:00:00+01:00[Europe/Paris]".parse().unwrap();
        assert_eq!(upkept(&bus(), &later, Some(Instants { start: t("2026-10-29T14:00"), end: t("2026-10-29T14:30") }), 0, Pull { syncs: true, began: None }, &after_it), Step::Nothing);
        assert_eq!(moved_with_at(&bus(), Instants { start: t("2026-10-29T14:00"), end: t("2026-10-29T14:30") }, &later, &paris(), after_it.timestamp().as_second()), None, "nor with a gesture");
    }

    /// A leg set for an instant this device never saw (another device moved the
    /// appointment and its legs, the appointment's copy not here yet) waits, then
    /// writes nothing once the appointment comes; still behind after a pull begun
    /// since, the copy here is current: it follows, once.
    #[test]
    fn a_leg_ahead_of_its_appointment_waits() {
        let ahead = bus().replace(&moves::utc(t("2026-10-29T13:10")), &moves::utc(t("2026-10-29T14:10"))).replace(&moves::utc(t("2026-10-29T13:35")), &moves::utc(t("2026-10-29T14:35"))).replace(&format!("WAS={}", moves::utc(t("2026-10-29T14:00"))), &format!("WAS={}", moves::utc(t("2026-10-29T15:00"))));
        let behind = appointment("2026-10-29T14:00", "2026-10-29T14:30");
        let seen = Some(behind.instants);
        assert_eq!(upkept(&ahead, &behind, seen, 100, Pull { syncs: true, began: Some(50) }, &now()), Step::Wait);
        let came = appointment("2026-10-29T15:00", "2026-10-29T15:30");
        assert_eq!(upkept(&ahead, &came, seen, 100, Pull { syncs: true, began: Some(50) }, &now()), Step::Nothing, "it came: as it should be");
        let Step::Write(back) = upkept(&ahead, &behind, seen, 100, Pull { syncs: true, began: Some(200) }, &now()) else { panic!("the copy here is current") };
        assert_eq!(times(&back), (t("2026-10-29T13:10"), t("2026-10-29T13:35")));
    }

    /// Cancelled with its appointment, a leg to come is cancelled with a mark;
    /// brought back with it; never one for an appointment repeating or of a whole day.
    #[test]
    fn cancelled_and_brought_back_with_its_appointment() {
        let pull = Pull { syncs: true, began: None };
        let mut off = appointment("2026-10-29T14:00", "2026-10-29T14:30");
        off.cancelled = true;
        let Step::Write(cancelled) = upkept(&bus(), &off, Some(off.instants), 0, pull, &now()) else { panic!("cancelled") };
        assert!(cancelled.contains("STATUS:CANCELLED") && cancelled.contains("X-SIOUL-CANCELLED-WITH:doctor"), "{cancelled}");
        assert_eq!(upkept(&cancelled, &off, Some(off.instants), 0, pull, &now()), Step::Nothing);
        let on = appointment("2026-10-29T14:00", "2026-10-29T14:30");
        let Step::Write(back) = upkept(&cancelled, &on, Some(on.instants), 0, pull, &now()) else { panic!("back") };
        assert!(!back.contains("STATUS:CANCELLED") && !back.contains(CANCELLED_WITH));
        let mut repeating = appointment("2026-10-29T15:00", "2026-10-29T15:30");
        repeating.recurring = true;
        assert_eq!(upkept(&bus(), &repeating, Some(on.instants), 0, pull, &now()), Step::Nothing);
    }

    /// A gesture moves the legs with the appointment: the same rule, a leg moved by
    /// hand meanwhile keeping its time.
    #[test]
    fn a_gesture_moves_the_legs_with_it() {
        let was = Instants { start: t("2026-10-29T14:00"), end: t("2026-10-29T14:30") };
        let to = appointment("2026-10-30T09:00", "2026-10-30T09:30");
        let at = now().timestamp().as_second();
        let moved = moved_with_at(&bus(), was, &to, &paris(), at).unwrap();
        assert_eq!(times(&moved), (t("2026-10-30T08:10"), t("2026-10-30T08:35")));
        assert_eq!(moved_with_at(&moved, to.instants, &to, &paris(), at), None, "the same: nothing");
    }

    /// Margins from the legs: from the earliest leg's start, to the latest leg's
    /// end; a stale leg counted where it will be; the stored ones without legs.
    #[test]
    fn margins_come_from_the_legs() {
        let o = |uid: &str, start: &str, end: &str, anchor: Option<Anchor>| Occurrence { uid: uid.into(), start: t(start), end: t(end), anchor, margins: crate::demands::Margins { before: 5, after: 5 }, ..Occurrence::default() };
        let mut all = vec![
            o("doctor", "2026-10-29T15:00", "2026-10-29T15:30", None),
            // Set for 14:00, not followed yet: counted as if moved.
            o("bus", "2026-10-29T13:10", "2026-10-29T13:35", parse(&bus())),
            o("walk", "2026-10-29T15:40", "2026-10-29T16:00", Some(Anchor { appointment: "doctor".into(), side: Side::After, gap: 600, timetable: false, was: Some(t("2026-10-29T15:30")), unconfirmed: None, cancelled_with: false })),
            o("lone", "2026-10-29T10:00", "2026-10-29T10:30", None),
        ];
        mark(&mut all);
        assert_eq!((all[0].margins.before, all[0].margins.after), (50, 30), "from 14:10 to 16:00");
        assert_eq!((all[1].leg_of.as_str(), all[2].leg_of.as_str()), ("doctor", "doctor"));
        assert_eq!((all[3].margins.before, all[3].margins.after), (5, 5), "no legs: its own");
    }

    /// The pass over the files: written through the writer's rule, twice nothing,
    /// a leg in a meeting with others never written.
    #[test]
    fn the_pass_writes_through_the_writers_rule() {
        crate::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "legs pass", None, &["VEVENT"]).unwrap();
        let path = calendar.dir.join("bus.ics");
        std::fs::write(&path, bus()).unwrap();
        let meeting = calendar.dir.join("meeting-leg.ics");
        std::fs::write(&meeting, bus().replace("UID:bus", "UID:meeting-leg").replace("SUMMARY:bus", "SUMMARY:bus\r\nORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:jane@example.org")).unwrap();
        let before = std::fs::read_to_string(&meeting).unwrap();
        let zone = paris();
        let refs: Vec<EventRef> = [&path, &meeting].iter().map(|p| agenda::event_ref(&std::fs::read_to_string(p).unwrap(), &p.display().to_string(), &zone).unwrap()).collect();
        let legs = legs_of("doctor", &refs);
        let later = appointment("2026-10-29T15:00", "2026-10-29T15:30");
        let seen = Some(Instants { start: t("2026-10-29T14:00"), end: t("2026-10-29T14:30") });
        let (written, settled, waits) = follow_legs(&legs, &later, seen, &|_| 0, Pull::default(), &now());
        assert_eq!((written, settled, waits), (vec![path.clone()], true, Vec::new()));
        assert_eq!(std::fs::read_to_string(&meeting).unwrap(), before, "a meeting with others is never written");
        let (again, _, _) = follow_legs(&legs, &later, Some(later.instants), &|_| 0, Pull::default(), &now());
        assert!(again.is_empty(), "twice: nothing");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    #[test]
    fn a_new_leg_and_one_untied() {
        let doctor = appointment("2026-10-29T14:00", "2026-10-29T14:30");
        let edit = agenda::EventEdit { title: "Bus 24".into(), start: "2026-10-29T13:10".into(), end: "2026-10-29T13:35".into(), ..agenda::EventEdit::default() };
        let text = new_leg(&edit, &doctor, &["health-admin".to_string()], Side::Before, true, &paris()).unwrap();
        let a = parse(&text).unwrap();
        assert_eq!((a.appointment.as_str(), a.side, a.gap, a.timetable, a.was), ("doctor", Side::Before, 25 * 60, true, Some(t("2026-10-29T14:00"))));
        assert!(text.contains("REFID:health-admin"), "in its appointment's projects: {text}");
        let alone = untied(&text).unwrap();
        assert!(parse(&alone).is_none() && !alone.contains("RELATED-TO"), "{alone}");
        let mut repeating = doctor.clone();
        repeating.recurring = true;
        assert!(new_leg(&edit, &repeating, &[], Side::Before, false, &paris()).is_err());
    }
}

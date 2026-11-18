// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! You, marked busy (docs/do-not-disturb.md, "While busy"; docs/client.md,
//! "Sharing your availability"). One mark, « Indisponible » in French:
//!
//! - **On an event** it is the standard `TRANSP`, the one Google's "Busy /
//!   Free" writes, so that it travels to every server and back; read as
//!   Google reads it: an event with a time is busy unless it says free
//!   (`TRANSP:TRANSPARENT`, or its calendar transparent as a whole); a
//!   whole-day event is free unless it says busy (`TRANSP:OPAQUE`).
//! - **On a task** it is Sioul's own `X-SIOUL-BUSY:TRUE` on the VTODO, since
//!   tasks have no such field; absent, the task is not busy. The person is
//!   busy while its timer runs, from its start until it stops; a timer
//!   nobody stops ends as a focus session's hold always did
//!   (`everywhere::focus`: 30 minutes past the time chosen, three hours
//!   without one). The window notes the mark in the running timer
//!   (`timelog::Running::busy`) when it starts it and when the mark changes,
//!   so that no reader opens every task to know it.
//!
//! **The stretches** (`stretches`) are the person's own events marked busy
//! (never a calendar someone else shares, an availability-only agenda, an
//! event declined or cancelled, nor a task's time block, which is its
//! task's time) and a busy task's timer: what turns do-not-disturb on "While
//! busy" (`everywhere::Sources::busy`) and what each phone's calls table
//! reads for its column "While busy".

use crate::agenda::Occurrence;

/// A task's mark, on its VTODO: `X-SIOUL-BUSY:TRUE`; absent, not busy.
pub const TASK_MARK: &str = "X-SIOUL-BUSY";

/// A day, in seconds: how far back and ahead `holding` reads.
const DAY: i64 = 86_400;

/// Whether an event marks you busy, read as Google does: an event with a
/// time is busy unless it says free; a whole-day event is free unless it
/// says busy. Never one cancelled or declined.
pub fn event_marked(event: &Occurrence) -> bool {
    !event.cancelled && !event.declined && !event.transparent && (!event.all_day || event.opaque)
}

/// Whether an event is the person's own: of an agenda of theirs, not one
/// someone else shares (`theirs`) nor one that gives its availability only
/// (`Occurrence::busy`), and no task's time block (`task`), which is its task's.
pub fn own_event(event: &Occurrence) -> bool {
    !event.theirs && !event.busy && event.task.is_empty()
}

/// A running timer as a busy stretch: from its start until it stops holding
/// (`everywhere::focus`), when its task is marked busy; none otherwise, or paused.
pub fn timer(running: &crate::timelog::Running, now: i64) -> Option<(i64, i64)> {
    running.busy.then(|| crate::everywhere::focus(running, now)).flatten()
}

/// The stretches during which the person is marked busy between `from` and
/// `to` (Unix seconds), from what is given (the pure part of `stretches`):
/// `events` as read (the person's own marked busy are kept), and the busy
/// task's timer, when one runs; each cut to the window, sorted, and those
/// that touch or overlap made one.
pub fn stretches_of(events: &[Occurrence], timer: Option<(i64, i64)>, from: i64, to: i64) -> Vec<(i64, i64)> {
    let mut spans: Vec<(i64, i64)> = events.iter().filter(|e| own_event(e) && event_marked(e)).map(|e| (e.start, e.end)).collect();
    spans.extend(timer);
    merged(spans, from, to)
}

/// Spans cut to `from`..`to`, the empty ones dropped, sorted, and those that touch or overlap made one.
pub fn merged(spans: Vec<(i64, i64)>, from: i64, to: i64) -> Vec<(i64, i64)> {
    let mut cut: Vec<(i64, i64)> = spans.into_iter().map(|(a, b)| (a.max(from), b.min(to))).filter(|(a, b)| a < b).collect();
    cut.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(cut.len());
    for (a, b) in cut {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// The stretches during which the person is marked busy, between two instants
/// (Unix seconds), sorted and merged: the person's own events marked busy, and
/// a task marked busy while its timer runs.
pub fn stretches(from: i64, to: i64) -> Vec<(i64, i64)> {
    if from >= to {
        return Vec::new();
    }
    let events = crate::agenda::occurrences(from, to);
    let now = crate::zone::now().timestamp().as_second();
    let timer = crate::timelog::running().and_then(|r| timer(&r, now));
    stretches_of(&events, timer, from, to)
}

/// The busy stretch holding at `now`, if one does: from its start (read a
/// day back at most) to its end (a day ahead at most).
pub fn holding(now: i64) -> Option<(i64, i64)> {
    holding_in(&stretches(now - DAY, now + DAY), now)
}

/// The stretch of `stretches` that holds at `now`.
pub fn holding_in(stretches: &[(i64, i64)], now: i64) -> Option<(i64, i64)> {
    stretches.iter().copied().find(|(a, b)| *a <= now && now < *b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(uid: &str, start: i64, end: i64) -> Occurrence {
        Occurrence { uid: uid.into(), summary: uid.into(), start, end, ..Occurrence::default() }
    }

    /// Events: busy by default when timed, free by default when whole-day;
    /// the mark read either way; never declined, cancelled, someone else's
    /// or a task's block.
    #[test]
    fn events_mark_you_busy_as_google_reads_them() {
        let events = vec![
            event("meeting", 1_000, 2_000),
            Occurrence { transparent: true, ..event("free", 3_000, 4_000) },
            Occurrence { all_day: true, ..event("holiday", 5_000, 5_000 + DAY) },
            Occurrence { all_day: true, opaque: true, ..event("away", 200_000, 200_000 + DAY) },
            Occurrence { declined: true, ..event("declined", 6_000, 7_000) },
            Occurrence { cancelled: true, ..event("cancelled", 8_000, 9_000) },
            Occurrence { theirs: true, ..event("partner", 10_000, 11_000) },
            Occurrence { busy: true, ..event("partner-busy-only", 12_000, 13_000) },
            Occurrence { task: "t1".into(), ..event("block", 14_000, 15_000) },
        ];
        assert_eq!(stretches_of(&events, None, 0, 400_000), vec![(1_000, 2_000), (200_000, 200_000 + DAY)]);
    }

    /// A busy task's timer counts from its start until it stops holding, as
    /// a focus session did; a task not marked busy, or a paused timer, never.
    #[test]
    fn a_busy_timer_counts_until_it_stops() {
        let running = crate::timelog::Running { task: "t1".into(), start: 1_000, planned: 25, busy: true, ..Default::default() };
        let end = 1_000 + 25 * 60 + crate::everywhere::FOCUS_GRACE;
        assert_eq!(timer(&running, 1_100), Some((1_000, end)));
        assert_eq!(timer(&running, end), None);
        assert_eq!(timer(&crate::timelog::Running { busy: false, ..running.clone() }, 1_100), None);
        assert_eq!(timer(&crate::timelog::Running { paused_at: Some(1_050), ..running.clone() }, 1_100), None);
        // With an event it overlaps: one stretch.
        let events = vec![event("call", 1_500, 1_000 + 25 * 60 + 60 * 60)];
        assert_eq!(stretches_of(&events, timer(&running, 1_100), 0, 100_000), vec![(1_000, 1_000 + 25 * 60 + 60 * 60)]);
    }

    /// Cut to the window, sorted, touching ones merged; the stretch holding now found.
    #[test]
    fn stretches_are_cut_sorted_and_merged() {
        assert_eq!(merged(vec![(50, 120), (0, 10), (10, 20), (30, 40), (35, 45)], 5, 100), vec![(5, 20), (30, 45), (50, 100)]);
        assert_eq!(holding_in(&[(5, 20), (30, 45)], 30), Some((30, 45)));
        assert_eq!(holding_in(&[(5, 20), (30, 45)], 20), None);
        assert!(stretches(10, 10).is_empty());
    }
}

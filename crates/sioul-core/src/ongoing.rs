// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Events over several days, drawn once (the ongoing events, 1 November
//! 2026). `timeline::days` places a thing on each day it touches, which is
//! what the plan's days and a grid's hours need; a list that wrote it again
//! under every day made a trip of three days look like three things, and an
//! event typed with a wrong end date like a month of them. Here, from the
//! items a view lays out (`spans`), the drawing layer's own model; `days`
//! stays as it is, and the plan, the warning for two events at once and the
//! reminders read the events as they are:
//! - a list writes such an event once, on its first day shown, with its
//!   whole span, then a thin line in its calendar's colour runs down the
//!   list's left edge through every day it covers, to an end mark on its
//!   last day (`Through`); three lines side by side at most, then "+2";
//! - a grid draws one of 24 hours or more, and a whole-day event over several
//!   days, as one bar across its days (`Bar`), each week's row its own piece;
//!   an evening past midnight (under 24 hours) keeps its hour blocks there,
//!   as other calendars draw it: a bar would hide its hours.
//!
//! A view gives `timeline::days` its items without those drawn here (a list:
//! every spanning event; a grid: its bars), so that a day's other things
//! keep their whole width. The lists and the grids draw what this says; they
//! compute no lane and no span.

use crate::i18n::Translator;
use crate::timeline::{clock, midnight, Item, Kind, Placed, Source, Time};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use serde::Serialize;

/// Lines side by side at most in a list, and bars stacked in a grid's row; the rest counted ("+2").
pub const LANES: u32 = 3;

/// The events over several days among a view's days, as the lists and the grids draw them.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Spans {
    /// Each such event once, as a list's row says it: its whole span in `when`
    /// ("Mon 2 Nov 10:00 → Mon 30 Nov 10:15"); what a line's tap opens, by its id.
    pub events: Vec<Placed>,
    /// For each day shown, in order: what passes through it (`Through`).
    pub days: Vec<Through>,
    /// The grids' bars: an event of 24 hours or more, or of several whole days, one per week's row.
    pub bars: Vec<Bar>,
    /// For each day shown, the bars passing through it beyond `LANES` ("+2").
    pub bars_more: Vec<u32>,
    /// Every event a grid draws as a bar, by its item's id, those beyond
    /// `LANES` too: none of them fills a day's hours or head.
    #[serde(skip)]
    pub barred: Vec<String>,
}

/// A day of a list: the lines passing through it, how many more, and what it writes for them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Through {
    /// The lines passing through the day, lanes 0 to 2.
    pub lines: Vec<Line>,
    /// Those passing through it beyond the three lanes.
    pub more: u32,
    /// What the day writes for its events over several days, in time order:
    /// an event's row on its first day shown, its end mark on its last.
    pub rows: Vec<SpanRow>,
}

/// An event's line through a day of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    /// The event's item id (`Spans::events`).
    pub id: String,
    pub uid: String,
    /// Its calendar's colour; "" for none.
    pub colour: String,
    /// 0, 1 or 2, from the list's left edge.
    pub lane: u32,
    /// The day is its first shown: the line starts at its row.
    pub first: bool,
    /// The day is its last: the line ends at its end mark.
    pub last: bool,
}

/// What a row of a list says of an event over several days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    /// Its first day: its row, its whole span.
    First,
    /// The list's first day, the event begun before: "Ongoing since Mon 2 Nov, until Mon 30 Nov".
    Since,
    /// Its last day: its end mark, "until 10:15".
    End,
}

/// A row a list writes for an event over several days on one day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpanRow {
    /// The event's item id (`Spans::events`): its title, its colour, what a tap opens.
    pub id: String,
    pub mark: Mark,
    /// Its time, as a list's time column says it: its start ("10:00", "All
    /// day"), its end ("10:15"); "" for "Ongoing since" and a last whole day.
    pub when: String,
    /// Its words, under its title: the span ("Mon 2 Nov 10:00 → Mon 30 Nov
    /// 10:15"), "Ongoing since …", "until 10:15", "last day".
    pub line: String,
    /// Where it sits among the day's rows, minutes from midnight; -1 at the day's head.
    pub minute: i32,
    /// Its line's lane, `LANES` and over when it has none (counted in "+2").
    pub lane: u32,
}

/// An event's bar across a week's row of a grid (the Week's head band, the Month's row, the Day's head line).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bar {
    pub id: String,
    pub uid: String,
    pub title: String,
    pub colour: String,
    /// The week's row, from 0 (the Week and the Day: 0; the Month: 0 to 5).
    pub row: u32,
    /// Its first and last columns in that row (0 to 6), the days it covers there.
    pub from_column: u32,
    pub to_column: u32,
    /// From the row's top, 0 to 2 in a week's or a month's rows, `LANES` and over
    /// counted in `bars_more` there; in the Day, any: it lists them all.
    pub lane: u32,
    /// Its start time at its start ("10:00"), its end time at its end ("10:15"):
    /// "" where the row cuts it, and for whole days.
    pub from: String,
    pub to: String,
    /// It began before this row, it goes on after it.
    pub cut_before: bool,
    pub cut_after: bool,
    /// Its line as the Day's head says it: "Ongoing: Residency, until Mon 30 Nov 10:15";
    /// on its first day, "Residency, Mon 2 Nov 10:00 → Mon 30 Nov 10:15".
    pub line: String,
}

impl Spans {
    /// Whether an item (by its id) is an event over several days drawn here, in a list.
    pub fn spanning(&self, id: &str) -> bool {
        self.events.iter().any(|e| e.item.id == id)
    }

    /// Whether an item (by its id) is a grid's bar (counted in "+2" beyond three lanes):
    /// never in a day's hours or head there.
    pub fn bar(&self, id: &str) -> bool {
        self.barred.iter().any(|b| b == id)
    }
}

/// An event's UID, from its calendar's file.
fn uid_of(item: &Item) -> String {
    match &item.source {
        Source::Calendar { uid, .. } => uid.clone(),
        _ => String::new(),
    }
}

/// One event over several days among the days shown.
struct Found<'a> {
    item: &'a Item,
    at: i64,
    until: i64,
    /// Whole days (a holiday).
    whole: bool,
    first_date: Date,
    last_date: Date,
    /// The first and last days shown it covers, by their place.
    first: usize,
    last: usize,
}

impl Found<'_> {
    /// Its own first day and last day are among those shown.
    fn starts_here(&self, from: Date) -> bool {
        date_at(from, self.first) == Some(self.first_date)
    }

    fn ends_here(&self, from: Date) -> bool {
        date_at(from, self.last) == Some(self.last_date)
    }

    /// A bar in a grid: 24 hours or more, or several whole days.
    fn is_bar(&self) -> bool {
        self.whole || self.until - self.at >= 86_400
    }
}

fn date_at(from: Date, offset: usize) -> Option<Date> {
    from.checked_add(jiff::Span::new().days(i64::try_from(offset).ok()?)).ok()
}

/// "Mon 2 Nov", "lun. 2 nov.".
pub fn short_day(tr: &Translator, day: Date) -> String {
    let mut args = crate::i18n::args();
    args.set("weekday", tr.weekday_short(day));
    args.set("day", day.day());
    args.set("month", tr.month_short(day));
    tr.text("date-day", Some(&args))
}

fn said(tr: &Translator, id: &str, pairs: &[(&'static str, String)]) -> String {
    let mut args = crate::i18n::args();
    for (name, value) in pairs {
        args.set(*name, value.clone());
    }
    tr.text(id, Some(&args))
}

fn zoned(at: i64, zone: &TimeZone) -> Option<jiff::Zoned> {
    Timestamp::from_second(at).ok().map(|t| t.to_zoned(zone.clone()))
}

/// An event's whole span, as its one row says it: "Mon 2 Nov 10:00 → Mon 30 Nov 10:15";
/// an evening past midnight (under 24 hours) "22:00 → 02:00 (Tue)"; whole days
/// "Sat 7 Nov → Mon 9 Nov".
fn span_words(f: &Found, zone: &TimeZone, tr: &Translator) -> String {
    span_text(f.at, f.until, f.whole, f.first_date, f.last_date, zone, tr)
}

fn span_text(at: i64, until: i64, whole: bool, first_date: Date, last_date: Date, zone: &TimeZone, tr: &Translator) -> String {
    if whole {
        return said(tr, "agenda-span", &[("start", short_day(tr, first_date)), ("end", short_day(tr, last_date))]);
    }
    if until - at < 86_400 {
        return said(tr, "agenda-span-night", &[("start", clock(at, zone)), ("end", clock(until, zone)), ("weekday", tr.weekday_short(last_date))]);
    }
    let (Some(start), Some(end)) = (zoned(at, zone), zoned(until, zone)) else { return String::new() };
    said(tr, "agenda-span", &[("start", tr.date(&start, true)), ("end", tr.date(&end, true))])
}

/// An event's whole span when it runs past its first day ("Mon 2 Nov 10:00 →
/// Mon 30 Nov 10:15", "22:00 → 02:00 (Tue)", whole days "Sat 7 Nov → Mon 9
/// Nov"), as its list row says it; None within its day. What a warning for
/// two events at once says of it rather than its clock times alone (fix W2).
pub fn span_of(at: i64, until: i64, whole: bool, zone: &TimeZone, tr: &Translator) -> Option<String> {
    let first_date = zoned(at, zone)?.date();
    let last_date = zoned(until.max(at + 1) - 1, zone)?.date();
    if last_date <= first_date {
        return None;
    }
    Some(span_text(at, until, whole, first_date, last_date, zone, tr))
}

/// Where it ends, as "until …" says it: "Mon 30 Nov 10:15"; whole days, the last day ("Mon 9 Nov").
fn end_words(f: &Found, zone: &TimeZone, tr: &Translator) -> String {
    if f.whole {
        return short_day(tr, f.last_date);
    }
    zoned(f.until, zone).map(|end| tr.date(&end, true)).unwrap_or_default()
}

/// "Ongoing: Residency, until Mon 30 Nov 10:15": the Day's head line and the
/// home card on a day after an event's first.
pub fn ongoing_line(title: &str, until: &str, tr: &Translator) -> String {
    said(tr, "agenda-ongoing", &[("title", title.to_string()), ("end", until.to_string())])
}

/// The lowest lane free over each one's days, in the order given: a line keeps its lane from its first day to its last.
fn lanes(spans: &[(usize, usize)]) -> Vec<u32> {
    let mut taken: Vec<(usize, usize, u32)> = Vec::new();
    let mut out = Vec::new();
    for &(first, last) in spans {
        let mut lane = 0;
        while taken.iter().any(|&(f, l, t)| t == lane && f <= last && first <= l) {
            lane += 1;
        }
        taken.push((first, last, lane));
        out.push(lane);
    }
    out
}

/// Whether an item is an event of a calendar (or a task's time block) that
/// runs past its first day: its last moment on a later day than its start (an
/// end at midnight belongs to the day before), or whole days over two or more.
fn over_days(item: &Item, zone: &TimeZone) -> Option<(i64, i64, bool, Date, Date)> {
    if !matches!(item.kind, Kind::Event | Kind::Block) || !matches!(item.source, Source::Calendar { .. }) {
        return None;
    }
    let (at, until) = item.time.proper(zone);
    let (whole, first, last) = match item.time {
        Time::Day { date, until } => (true, date, until),
        Time::Span { .. } => (false, zoned(at, zone)?.date(), zoned(until.max(at + 1) - 1, zone)?.date()),
        Time::At { .. } => return None,
    };
    (last > first).then_some((at, until, whole, first, last))
}

/// The events over several days among `count` days from `from` (a view's
/// days, `timeline::days`' own), Show's hidden ones left out (`hidden`):
/// their rows, lines and bars, lanes given in the order they start (the
/// longer first, then by id).
pub fn spans(items: &[Item], from: Date, count: i64, zone: &TimeZone, tr: &Translator, hidden: &dyn Fn(&Item) -> bool) -> Spans {
    let shown: Vec<(i64, i64)> = (0..count.max(0))
        .filter_map(|i| from.checked_add(jiff::Span::new().days(i)).ok())
        .map(|d| (midnight(d, zone), d.tomorrow().map_or(midnight(d, zone) + 86_400, |n| midnight(n, zone))))
        .collect();
    let mut found: Vec<Found> = Vec::new();
    for item in items.iter().filter(|i| !hidden(i)) {
        let Some((at, until, whole, first_date, last_date)) = over_days(item, zone) else { continue };
        let covered: Vec<usize> = shown.iter().enumerate().filter(|&(_, &(start, end))| at < end && until > start).map(|(i, _)| i).collect();
        let (Some(&first), Some(&last)) = (covered.first(), covered.last()) else { continue };
        // One occurrence once: a copy of it in the items (a merge, an adapter's second look) is the same line.
        if found.iter().any(|f| f.item.id == item.id) {
            continue;
        }
        found.push(Found { item, at, until, whole, first_date, last_date, first, last });
    }
    found.sort_by(|a, b| (a.at, std::cmp::Reverse(a.until), &a.item.id).cmp(&(b.at, std::cmp::Reverse(b.until), &b.item.id)));
    let list_lanes = lanes(&found.iter().map(|f| (f.first, f.last)).collect::<Vec<_>>());
    let bars_found: Vec<&Found> = found.iter().filter(|f| f.is_bar()).collect();
    let bar_lanes = lanes(&bars_found.iter().map(|f| (f.first, f.last)).collect::<Vec<_>>());

    let mut out = Spans { days: vec![Through::default(); shown.len()], bars_more: vec![0; shown.len()], ..Spans::default() };
    for (f, &lane) in found.iter().zip(&list_lanes) {
        let colour = f.item.colour.clone().unwrap_or_default();
        let id = f.item.id.clone();
        let (start, _) = shown[f.first];
        let minute = |at: i64, day: usize| i32::try_from((at - shown[day].0) / 60).unwrap_or(0).clamp(0, 1440);
        let when = span_words(f, zone, tr);
        out.events.push(Placed {
            item: f.item.clone(),
            when: when.clone(),
            both: String::new(),
            from_minute: if f.at > start { minute(f.at, f.first) } else { 0 },
            to_minute: 1440,
            all_day: f.whole,
            column: 0,
            columns: 1,
            edge: "",
        });
        let (starts, ends) = (f.starts_here(from), f.ends_here(from));
        for day in f.first..=f.last {
            let through = &mut out.days[day];
            if lane < LANES {
                through.lines.push(Line { id: id.clone(), uid: uid_of(f.item), colour: colour.clone(), lane, first: day == f.first, last: ends && day == f.last });
            } else {
                through.more += 1;
            }
        }
        let first = &mut out.days[f.first].rows;
        if starts {
            let at = if f.whole { tr.text("agenda-all-day", None) } else { clock(f.at, zone) };
            first.push(SpanRow { id: id.clone(), mark: Mark::First, when: at, line: when, minute: if f.whole { -1 } else { minute(f.at, f.first) }, lane });
        } else {
            let since = said(tr, "agenda-ongoing-since", &[("start", short_day(tr, f.first_date)), ("end", short_day(tr, f.last_date))]);
            first.push(SpanRow { id: id.clone(), mark: Mark::Since, when: String::new(), line: since, minute: -1, lane });
        }
        if ends {
            let (at, words) = if f.whole { (String::new(), tr.text("agenda-span-last-day", None)) } else { (clock(f.until, zone), said(tr, "agenda-span-end", &[("time", clock(f.until, zone))])) };
            out.days[f.last].rows.push(SpanRow { id, mark: Mark::End, when: at, line: words, minute: if f.whole { -1 } else { minute(f.until, f.last) }, lane });
        }
    }
    for through in &mut out.days {
        through.rows.sort_by_key(|r| (r.minute, r.mark == Mark::End, r.lane));
        through.lines.sort_by_key(|l| l.lane);
    }
    // A single day (the Day) says every one at its head, one line each: none beyond three
    // there (fix W3); a week's or a month's rows three lanes, then "+2".
    let capped = shown.len() > 1;
    for (f, &lane) in bars_found.iter().zip(&bar_lanes) {
        out.barred.push(f.item.id.clone());
        let until = end_words(f, zone, tr);
        let (starts, ends) = (f.starts_here(from), f.ends_here(from));
        let beyond = capped && lane >= LANES;
        for day in f.first..=f.last {
            if beyond {
                out.bars_more[day] += 1;
            }
        }
        if beyond {
            continue;
        }
        let (first_row, last_row) = (f.first / 7, f.last / 7);
        for row in first_row..=last_row {
            let (row_first, row_last) = ((row * 7).max(f.first), (row * 7 + 6).min(f.last));
            let cut_before = !(starts && row_first == f.first);
            let cut_after = !(ends && row_last == f.last);
            let line = if starts && row_first == f.first {
                said(tr, "agenda-bar-line", &[("title", f.item.title.clone()), ("span", span_words(f, zone, tr))])
            } else {
                ongoing_line(&f.item.title, &until, tr)
            };
            out.bars.push(Bar {
                id: f.item.id.clone(),
                uid: uid_of(f.item),
                title: f.item.title.clone(),
                colour: f.item.colour.clone().unwrap_or_default(),
                row: u32::try_from(row).unwrap_or(0),
                from_column: u32::try_from(row_first - row * 7).unwrap_or(0),
                to_column: u32::try_from(row_last - row * 7).unwrap_or(0),
                lane,
                from: if cut_before || f.whole { String::new() } else { clock(f.at, zone) },
                to: if cut_after || f.whole { String::new() } else { clock(f.until, zone) },
                cut_before,
                cut_after,
                line,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::agenda::Occurrence;
    use jiff::tz::TimeZone;
    use jiff::Zoned;

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> Zoned {
        text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(paris()).unwrap()
    }

    fn event(uid: &str, title: &str, from: &str, to: &str, all_day: bool) -> Occurrence {
        Occurrence {
            key: format!("/stand-in/calendars/local/club/{uid}.ics"),
            uid: uid.into(),
            summary: title.into(),
            start: at(from).timestamp().as_second(),
            end: at(to).timestamp().as_second(),
            all_day,
            calendar: "Club".into(),
            color: Some("#3d6b8f".into()),
            alarms: vec![-900],
            ..Occurrence::default()
        }
    }

    /// The stand-ins of the round: a month-long timed event (an end date typed
    /// wrong), a trip over three days, a holiday of three whole days, an
    /// evening past midnight, and, on Friday 6 and Saturday 7 November, five
    /// at once (two of them whole days); a dentist for company.
    pub(crate) fn stand_ins() -> Vec<Occurrence> {
        vec![
            event("residency", "Residency", "2026-11-02T10:00", "2026-11-30T10:15", false),
            event("dentist", "Dentist", "2026-11-03T11:00", "2026-11-03T12:00", false),
            event("concert", "Concert", "2026-11-03T22:00", "2026-11-04T02:00", false),
            event("trip", "Trip to the coast", "2026-11-04T08:00", "2026-11-06T19:00", false),
            event("exhibition", "Exhibition", "2026-11-05T09:00", "2026-11-15T18:00", false),
            event("course", "Course", "2026-11-05T14:00", "2026-11-09T12:00", false),
            event("guests", "Guests", "2026-11-06T00:00", "2026-11-09T00:00", true),
            event("holiday", "Holiday", "2026-11-07T00:00", "2026-11-10T00:00", true),
        ]
    }

    fn day(text: &str) -> jiff::civil::Date {
        text.parse().unwrap()
    }

    /// The stand-ins as a view's items, Show hiding the uids in `hidden`.
    fn spans_of(from: &str, count: i64, hidden: &[&str]) -> super::Spans {
        let zone = paris();
        let mut occurrences = stand_ins();
        // Ending at midnight: within its day, never over two.
        occurrences.push(event("late", "Late film", "2026-11-10T20:00", "2026-11-11T00:00", false));
        let items = crate::agenda::items(&occurrences, &zone, &|_| false);
        let hide = |item: &crate::timeline::Item| matches!(&item.source, crate::timeline::Source::Calendar { uid, .. } if hidden.contains(&uid.as_str()));
        super::spans(&items, day(from), count, &zone, &crate::i18n::Translator::new("en"), &hide)
    }

    fn uid_of<'a>(spans: &'a super::Spans, id: &str) -> &'a str {
        match &spans.events.iter().find(|e| e.item.id == id).unwrap().item.source {
            crate::timeline::Source::Calendar { uid, .. } => uid,
            _ => "",
        }
    }

    /// Each day's rows, as (uid, mark, words, lane), and its lines as (uid, lane, first, last), and "+n".
    fn listed(spans: &super::Spans) -> Vec<(Vec<(String, super::Mark, String, u32)>, Vec<(String, u32, bool, bool)>, u32)> {
        spans
            .days
            .iter()
            .map(|d| {
                (
                    d.rows.iter().map(|r| (uid_of(spans, &r.id).to_string(), r.mark, r.line.clone(), r.lane)).collect(),
                    d.lines.iter().map(|l| (l.uid.clone(), l.lane, l.first, l.last)).collect(),
                    d.more,
                )
            })
            .collect()
    }

    /// A list writes each event over several days once, on its first day, with
    /// its whole span; its line runs through every day it covers in its own
    /// lane, to an end mark on its last day; five at once take three lanes and
    /// "+2"; a single day, and an evening ending at midnight, are not lines.
    #[test]
    fn an_event_over_several_days_is_one_row_and_a_line() {
        use super::Mark::{End, First};
        let spans = spans_of("2026-11-02", 14, &[]);
        let mut uids: Vec<&str> = spans.events.iter().map(|e| uid_of(&spans, &e.item.id)).collect();
        uids.sort_unstable();
        assert_eq!(uids, ["concert", "course", "exhibition", "guests", "holiday", "residency", "trip"], "the dentist and the late film are within their days");
        let days = listed(&spans);
        let row = |uid: &str, mark, words: &str, lane| (uid.to_string(), mark, words.to_string(), lane);
        assert_eq!(days[0].0, [row("residency", First, "Mon 2 Nov 10:00 → Mon 30 Nov 10:15", 0)]);
        assert_eq!(days[1].0, [row("concert", First, "22:00 → 02:00 (Wed)", 1)]);
        assert_eq!(days[2].0, [row("concert", End, "until 02:00", 1), row("trip", First, "Wed 4 Nov 08:00 → Fri 6 Nov 19:00", 2)]);
        assert_eq!(days[3].0, [row("exhibition", First, "Thu 5 Nov 09:00 → Sun 15 Nov 18:00", 1), row("course", First, "Thu 5 Nov 14:00 → Mon 9 Nov 12:00", 3)]);
        assert_eq!(days[4].0, [row("guests", First, "Fri 6 Nov → Sun 8 Nov", 4), row("trip", End, "until 19:00", 2)]);
        assert_eq!(days[5].0, [row("holiday", First, "Sat 7 Nov → Mon 9 Nov", 2)]);
        assert_eq!(days[6].0, [row("guests", End, "last day", 4)]);
        assert_eq!(days[7].0, [row("holiday", End, "last day", 2), row("course", End, "until 12:00", 3)]);
        assert!(days[8..13].iter().all(|d| d.0.is_empty()), "nothing written on the days between");
        // The time column: its start, "All day" for whole days, its end; none for a last whole day.
        let times: Vec<&str> = [0, 2, 4, 6].iter().flat_map(|&i| spans.days[i].rows.iter().map(|r| r.when.as_str())).collect();
        assert_eq!(times, ["10:00", "02:00", "08:00", "All day", "19:00", ""]);
        // Each event's first row once, over the whole list.
        let firsts: Vec<&String> = days.iter().flat_map(|d| d.0.iter()).filter(|r| r.1 == First).map(|r| &r.0).collect();
        assert_eq!(firsts.len(), 7);
        // The line: on every day it covers, in its lane, first and last marked.
        for (i, d) in days.iter().enumerate() {
            let residency = d.1.iter().find(|l| l.0 == "residency").unwrap();
            assert_eq!((residency.1, residency.2, residency.3), (0, i == 0, false), "the month goes on past the list, day {i}");
        }
        let line = |i: usize, uid: &str| days[i].1.iter().find(|l| l.0 == uid).map(|l| (l.1, l.2, l.3));
        assert_eq!([line(1, "concert"), line(2, "concert"), line(3, "concert")], [Some((1, true, false)), Some((1, false, true)), None]);
        assert_eq!([line(2, "trip"), line(3, "trip"), line(4, "trip"), line(5, "trip")], [Some((2, true, false)), Some((2, false, false)), Some((2, false, true)), None]);
        assert_eq!([line(5, "holiday"), line(6, "holiday"), line(7, "holiday"), line(8, "holiday")], [Some((2, true, false)), Some((2, false, false)), Some((2, false, true)), None]);
        // Five at once on Friday and Saturday: three lanes, then "+2" (the course and the guests).
        assert_eq!(days[4].1.iter().map(|l| (l.0.as_str(), l.1)).collect::<Vec<_>>(), [("residency", 0), ("exhibition", 1), ("trip", 2)]);
        assert_eq!((days[4].2, days[5].2, days[3].2, days[8].2), (2, 2, 1, 0));
        assert!(days[7].1.iter().all(|l| l.0 != "course"), "the course beyond three has no line: {:?}", days[7].1);
        // The event once, its row's words its whole span, what a line's tap opens.
        let residency = spans.events.iter().find(|e| e.item.title == "Residency").unwrap();
        assert_eq!((residency.when.as_str(), residency.from_minute, residency.all_day), ("Mon 2 Nov 10:00 → Mon 30 Nov 10:15", 600, false));
    }

    /// A list that starts after an event began says, on its first day, "Ongoing
    /// since …, until …"; its line starts there; its end mark on its last day.
    #[test]
    fn a_list_starting_mid_event_says_ongoing_since() {
        use super::Mark::{End, Since};
        let spans = spans_of("2026-11-11", 7, &[]);
        let days = listed(&spans);
        let row = |uid: &str, mark, words: &str, lane| (uid.to_string(), mark, words.to_string(), lane);
        assert_eq!(days[0].0, [row("residency", Since, "Ongoing since Mon 2 Nov, until Mon 30 Nov", 0), row("exhibition", Since, "Ongoing since Thu 5 Nov, until Sun 15 Nov", 1)]);
        assert_eq!(days[4].0, [row("exhibition", End, "until 18:00", 1)]);
        assert_eq!(days[0].1, [("residency".to_string(), 0, true, false), ("exhibition".to_string(), 1, true, false)]);
        let fr = {
            let zone = paris();
            let items = crate::agenda::items(&stand_ins(), &zone, &|_| false);
            super::spans(&items, day("2026-11-11"), 7, &zone, &crate::i18n::Translator::new("fr"), &|_| false)
        };
        assert_eq!(fr.days[0].rows[0].line, "En cours depuis le lun. 2 nov., jusqu’au lun. 30 nov.");
        assert_eq!((fr.days[4].rows[0].when.as_str(), fr.days[4].rows[0].line.as_str()), ("18:00", "jusqu’à 18:00"));
    }

    /// The grids: an event of 24 hours or more, or of several whole days, is one
    /// bar per week's row, its times at its two ends; the evening (four hours
    /// past midnight) is no bar: it keeps its hour blocks.
    #[test]
    fn the_grids_draw_one_bar_per_week_row() {
        let spans = spans_of("2026-11-02", 7, &[]);
        let bar = |uid: &str| spans.bars.iter().filter(|b| b.uid == uid).map(|b| (b.row, b.from_column, b.to_column, b.lane, b.from.as_str(), b.to.as_str(), b.cut_before, b.cut_after)).collect::<Vec<_>>();
        assert_eq!(bar("residency"), [(0, 0, 6, 0, "10:00", "", false, true)]);
        assert_eq!(bar("trip"), [(0, 2, 4, 1, "08:00", "19:00", false, false)]);
        assert!(bar("guests").is_empty(), "the guests, in a fifth lane, counted beyond three");
        let id_of = |title: &str| spans.events.iter().find(|e| e.item.title == title).unwrap().item.id.clone();
        assert!(spans.bar(&id_of("Guests")) && spans.bar(&id_of("Course")), "beyond three, still out of the hours");
        assert_eq!(bar("holiday"), [(0, 5, 6, 1, "", "", false, true)]);
        assert!(bar("concert").is_empty(), "the evening keeps its blocks");
        assert!(!spans.bar(&spans.events.iter().find(|e| e.item.title == "Concert").unwrap().item.id));
        assert_eq!(spans.bars_more, [0, 0, 0, 1, 2, 2, 2], "the course (from Thursday) and the guests (from Friday) beyond three");
        assert_eq!(spans.bars.iter().find(|b| b.uid == "residency").unwrap().line, "Residency, Mon 2 Nov 10:00 → Mon 30 Nov 10:15");
        // The Month, six weeks from Monday 26 October: the month-long event once per week's row, its end time on its last.
        let month = spans_of("2026-10-26", 42, &[]);
        let residency: Vec<_> = month.bars.iter().filter(|b| b.uid == "residency").map(|b| (b.row, b.from_column, b.to_column, b.to.as_str(), b.title.as_str())).collect();
        assert_eq!(residency, [(1, 0, 6, "", "Residency"), (2, 0, 6, "", "Residency"), (3, 0, 6, "", "Residency"), (4, 0, 6, "", "Residency"), (5, 0, 0, "10:15", "Residency")]);
        assert_eq!(month.bars.iter().filter(|b| b.uid == "trip").count(), 1);
        assert_eq!(month.bars.iter().filter(|b| b.uid == "exhibition").map(|b| b.row).collect::<Vec<_>>(), [1, 2]);
        // The Day lists every one, five at once on Friday: no "+2" there (fix W3).
        let friday = spans_of("2026-11-06", 1, &[]);
        let mut titles: Vec<&str> = friday.bars.iter().map(|b| b.title.as_str()).collect();
        titles.sort_unstable();
        assert_eq!(titles, ["Course", "Exhibition", "Guests", "Residency", "Trip to the coast"]);
        assert_eq!(friday.bars_more, [0]);
        // The Day: one bar, its head line.
        let tuesday = spans_of("2026-11-10", 1, &[]);
        let lines: Vec<&str> = tuesday.bars.iter().map(|b| b.line.as_str()).collect();
        assert_eq!(lines, ["Ongoing: Residency, until Mon 30 Nov 10:15", "Ongoing: Exhibition, until Sun 15 Nov 18:00"]);
    }

    /// A warning for two events at once names an event over several days by its
    /// span, as its row does (fix W2); one within its day, by its clock times.
    #[test]
    fn a_warning_says_an_event_s_span() {
        let (zone, tr) = (paris(), crate::i18n::Translator::new("en"));
        let second = |text: &str| at(text).timestamp().as_second();
        assert_eq!(super::span_of(second("2026-11-03T22:00"), second("2026-11-04T02:00"), false, &zone, &tr).as_deref(), Some("22:00 → 02:00 (Wed)"));
        assert_eq!(super::span_of(second("2026-11-02T10:00"), second("2026-11-30T10:15"), false, &zone, &tr).as_deref(), Some("Mon 2 Nov 10:00 → Mon 30 Nov 10:15"));
        assert_eq!(super::span_of(second("2026-11-07T00:00"), second("2026-11-10T00:00"), true, &zone, &tr).as_deref(), Some("Sat 7 Nov → Mon 9 Nov"));
        assert_eq!(super::span_of(second("2026-11-03T11:00"), second("2026-11-04T00:00"), false, &zone, &tr), None, "ending at midnight: within its day");
    }

    /// A calendar Show hides takes no lane: the others close up.
    #[test]
    fn a_hidden_calendar_takes_no_lane() {
        let spans = spans_of("2026-11-02", 7, &["residency"]);
        assert!(spans.events.iter().all(|e| e.item.title != "Residency"));
        let days = listed(&spans);
        assert_eq!(days[4].1.iter().map(|l| (l.0.as_str(), l.1)).collect::<Vec<_>>(), [("exhibition", 0), ("trip", 1), ("course", 2)]);
        assert_eq!(days[4].2, 1, "the guests alone beyond three");
    }

    /// What the plan, the warning for two events at once and the reminders
    /// answer on the stand-ins, written as text: the plan's busy spans, each
    /// day's layout from Monday 2 to Tuesday 10 November with three tasks to
    /// place, the overlaps, and the reminders of Monday 2 November.
    fn group_two_answers() -> String {
        use crate::areas::TaskAreas;
        use crate::plan::{plan, Settings};
        use crate::window::AdminWindow;
        use std::collections::{BTreeMap, BTreeSet};
        let events = stand_ins();
        let now = at("2026-11-02T08:00");
        let window = |day: &str| AdminWindow { day: day.into(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: Some("work".into()) };
        let hours: Vec<AdminWindow> = ["monday", "tuesday", "wednesday", "thursday", "friday"].into_iter().map(window).collect();
        let settings = Settings::of_hours(&hours, TaskAreas::usual()).with_events(&now, &events);
        let task = |uid: &str, estimate: u32| crate::tasks::Task { uid: uid.into(), title: uid.into(), estimate, ..crate::tasks::Task::default() };
        let tasks = vec![task("form", 30), task("call", 45), task("report", 240)];
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        let mut out = String::new();
        out.push_str(&format!("event_spans: {:?}\n", crate::plan::event_spans(&events, 0)));
        out.push_str(&format!("event_spans with a pause: {:?}\n", crate::plan::event_spans(&events, 10)));
        let mut date = now.date();
        for _ in 0..9 {
            let view = crate::dayview::lay_on(date, &now, &events, &made, &tasks, &settings, crate::dayview::Drawn::ALL);
            out.push_str(&format!("lay_on {date}: {view:?}\n"));
            date = date.tomorrow().unwrap();
        }
        out.push_str(&format!("day: {:?}\n", crate::dayview::day(&now, &events, &made, &tasks, &settings)));
        for between in [0, 30] {
            out.push_str(&format!("overlaps {between}: {:?}\n", crate::overlaps::overlaps(&events, between)));
        }
        let config: crate::config::Config = toml::from_str(&hours.iter().map(|w| format!("[[window]]\nday = \"{}\"\nstart = \"09:00\"\nend = \"17:00\"\n", w.day)).collect::<String>()).unwrap();
        let tr = crate::i18n::Translator::new("en");
        for moment in ["2026-11-02T08:00", "2026-11-04T07:00", "2026-11-06T12:00"] {
            let reminders = crate::reminders::all(&config, &tr, &at(moment), &events, &tasks, None, &[], &[], None, |_| false);
            out.push_str(&format!("reminders {moment}: {reminders:?}\n"));
        }
        out
    }

    /// The plan, the warning for two events at once and the reminders read the
    /// events as they are: their answers on the stand-ins are those recorded on
    /// 6820489, before events over several days were drawn once
    /// (tests/ongoing-unchanged.txt; `SIOUL_RECORD_ONGOING=1` writes it).
    #[test]
    fn the_plan_the_warning_and_the_reminders_read_the_events_as_they_are() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ongoing-unchanged.txt");
        let answers = group_two_answers();
        if std::env::var_os("SIOUL_RECORD_ONGOING").is_some() {
            std::fs::write(&path, &answers).unwrap();
        }
        let recorded = std::fs::read_to_string(&path).unwrap();
        assert!(answers.lines().count() == 17 && answers.contains("Residency"), "the answers hold the stand-ins");
        for (line, (now, then)) in answers.lines().zip(recorded.lines()).enumerate() {
            assert_eq!(now, then, "line {} differs from the answer recorded on 6820489", line + 1);
        }
        assert_eq!(answers.lines().count(), recorded.lines().count());
    }
}

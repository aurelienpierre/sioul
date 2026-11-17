// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The grid of your devices (docs/areas.md, "What each device is for"): for
//! each row of the table (`device_table`), its five cells, one per time, the
//! areas that come forward on that device then, in words; and around them,
//! what now brings on this device, the areas no device is for, and what a
//! device's areas never keep back. Settings ▸ Devices and `sioul devices`
//! show the same, worded here.
//!
//! A cell says the device's line as your settings here say it
//! (`sioul_core::areas::Line::cell`): what the time brings, kept to what the
//! device is for, or the cell you changed; only the times your week has (a
//! time it lacks never comes: review of phase 2, M2). What no device is for
//! comes on every device as the hours say (`Here::unheld`): that is said
//! once, under the grid, and in the sentence of now, which says what really
//! comes here. A device whose Sioul is older has no cell in force: one
//! sentence says everything still comes there. A device whose own entry says
//! other cells (it does not share your settings, or not yet) follows its
//! own: one sentence under its cells says so (`Row::cells_apart`).

use crate::device_table::Row;
use crate::dispatch::Known;
use serde::Serialize;
use sioul_core::areafilter::areas_in_words;
use sioul_core::areas::{Area, Cells, Line, Time};
use sioul_core::device_said::Said;
use sioul_core::i18n::Translator;
use sioul_core::quiet::Mode;
use std::collections::BTreeMap;

/// One cell: a device at a time. Its three areas and its words are what
/// the window's boxes read (AreaFilter.qml: `work`, `admin`, `leisure`,
/// `label`, `own`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Cell {
    /// The time, as written in the settings: "work", "admin", "leisure", "meals", "sleep".
    pub time: String,
    /// The time's name: "Work".
    pub title: String,
    pub work: bool,
    pub admin: bool,
    pub leisure: bool,
    /// The areas as written: "work+leisure"; "" for nothing.
    pub areas: String,
    /// The same in words: "Work · Leisure"; "Nothing".
    pub label: String,
    /// You changed this cell: drawn apart, "As its areas say" puts it back.
    pub own: bool,
    /// The cell in one sentence, for a screen reader and a line of text:
    /// "Work desk, Work: Work · Leisure, as you changed it".
    pub name: String,
    /// The time and what comes, for a line of a card or of a terminal:
    /// "Work: Work · Leisure (as you changed it)".
    pub line: String,
}

/// What is said around the cells.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Grid {
    /// The five times' names, in the cells' order.
    pub times: Vec<String>,
    /// What now brings on this device: "Now, leisure until 22:00: leisure
    /// comes here."; "" in a pause, which holds everything.
    pub now: String,
    /// The areas no device that counts is for, when some line says
    /// something: "No device is for leisure: until one is, that comes on
    /// every device, as the hours say."; "" when every area has its device.
    pub unheld: String,
    /// What a device's areas never keep back, in one sentence.
    pub never: String,
}

fn said(words: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = sioul_core::i18n::args();
    for (key, value) in pairs {
        args.set(key.to_string(), value.clone());
    }
    words.text(id, Some(&args))
}

/// A cell's areas in words: "Work · Leisure"; "Nothing".
pub fn cell_label(areas: Area, words: &Translator) -> String {
    if areas.is_empty() { words.text("devices-cell-nothing", None) } else { sioul_core::areafilter::label(words, areas) }
}

/// A time's name, as the matrix of what reaches you names its columns.
pub fn time_title(time: Time, words: &Translator) -> String {
    words.text(&format!("attention-column-{}", time.id()), None)
}

/// The cells of a device's line, one per time the week `mode` knows has;
/// `device`: the name the device is shown under.
pub fn cells(line: Line, device: &str, mode: &Mode, words: &Translator) -> Vec<Cell> {
    times(mode)
        .into_iter()
        .map(|time| {
            let areas = line.cell(time, mode.week);
            let own = line.changed(time).is_some();
            let (title, label) = (time_title(time, words), cell_label(areas, words));
            let pairs = [("device", device.to_string()), ("time", title.clone()), ("areas", label.clone())];
            Cell {
                time: time.id().to_string(),
                work: areas.work,
                admin: areas.admin,
                leisure: areas.leisure,
                areas: areas.id(),
                own,
                name: said(words, if own { "devices-cell-name-changed" } else { "devices-cell-name" }, &pairs),
                line: said(words, if own { "devices-cell-line-changed" } else { "devices-cell-line" }, &pairs[1..]),
                title,
                label,
            }
        })
        .collect()
}

/// The times the week has, in the cells' order (`Time::STATES`).
fn times(mode: &Mode) -> Vec<Time> {
    Time::STATES.into_iter().filter(|time| mode.week.has(*time)).collect()
}

/// Each row given its cells (`Row::cells`), or, for a device whose Sioul is
/// older, the sentence said in their place (`Row::cells_older`); and what is
/// said around them. `said`: what you said of each device; `known`: the
/// devices that count, as this device knows them; `mode`: what now is for on
/// this device (`quiet::Mode::on` with `known.lines()`), at `at`.
pub fn grid(rows: &mut [Row], said_of: &BTreeMap<String, Said>, known: &Known, mode: &Mode, at: &jiff::Zoned, words: &Translator) -> Grid {
    for row in rows.iter_mut() {
        // This device runs this Sioul, whatever its entry last said.
        row.follows |= row.here;
        if row.follows {
            let line = said_of.get(&row.id).map(Said::line).unwrap_or_default();
            row.cells = cells(line, &row.name, mode, words);
            // Another device's own word on its cells, when it says other ones than these.
            let apart = known.others.get(&row.id).is_some_and(|seen| seen.cells != Cells::Line(line).words(mode.week));
            row.cells_apart = if apart && !row.here { words.text("devices-cells-apart", None) } else { String::new() };
        } else {
            row.cells = Vec::new();
            row.cells_older = words.text("devices-older", None);
        }
    }
    let here = mode.here;
    let now = if mode.paused() {
        String::new()
    } else {
        // A variant's name holds no "+": working and admin hours at once are "work-admin".
        let time = said(words, "devices-now-time", &[("time", mode.time.id().replace('+', "-"))]);
        let comes = areas_in_words(words, "device-areas-come", here.cell(mode.time, mode.week));
        match &mode.until {
            Some(until) => said(words, "devices-now", &[("time", time), ("until", sioul_core::quiet::until_text(words, until, at)), ("comes", comes)]),
            None => said(words, "devices-now-open", &[("time", time), ("comes", comes)]),
        }
    };
    let some_said = known.some_said();
    let unheld = if some_said && !here.unheld.is_empty() { said(words, "devices-unheld", &[("areas", areas_in_words(words, "device-areas", here.unheld))]) } else { String::new() };
    Grid { times: times(mode).into_iter().map(|time| time_title(time, words)).collect(), now, unheld, never: words.text("devices-never-held", None) }
}

/// The grid's own lines for the command line, under the table: what now
/// brings here, what no device is for, what is never kept back.
pub fn plain(grid: &Grid) -> Vec<String> {
    [&grid.now, &grid.unheld, &grid.never].into_iter().filter(|line| !line.is_empty()).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Seen;
    use sioul_core::areas::{Lines, Week};
    use sioul_core::quiet::Reason;

    fn mode(time: Time, reason: Reason, until: Option<&str>, lines: &Lines) -> Mode {
        let week = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        Mode { quiet: !time.works(), time, week, reason, until: until.map(|u| u.parse().unwrap()), back: None, label: String::new(), here: sioul_core::areas::Here::default() }.on(lines)
    }

    fn rows() -> Vec<Row> {
        vec![
            Row { id: "desk".into(), here: true, name: "Work desk".into(), ..Row::default() },
            Row { id: "phone".into(), name: "The phone (Moss)".into(), follows: true, ..Row::default() },
            Row { id: "tablet".into(), name: "The tablet (Fern)".into(), follows: false, ..Row::default() },
        ]
    }

    /// Another device as its entry says it: its cells in `week`, and that it acts on the cells of `of`.
    fn seen_by(said_of: &BTreeMap<String, Said>, id: &str, of: &str, week: Week) -> Seen {
        let words = |id: &str| Cells::Line(said_of.get(id).map(Said::line).unwrap_or_default()).words(week);
        Seen { cells: words(id), known: [(of.to_string(), Cells::key(&words(of)))].into(), carried: false }
    }

    fn said(pairs: &[(&str, &str, &[(&str, &str)])]) -> BTreeMap<String, Said> {
        pairs.iter().map(|(id, area, at)| (id.to_string(), Said { area: (!area.is_empty()).then(|| area.to_string()), at: at.iter().map(|(time, cell)| (time.to_string(), cell.to_string())).collect(), ..Said::default() })).collect()
    }

    /// The grid in words: a computer for work, a phone for admin and leisure
    /// where friends come during work and nothing at meals, a tablet on an
    /// older Sioul; what now brings here; in French too.
    #[test]
    fn the_grid_says_each_cell_in_words() {
        let english = Translator::new("en");
        let said_of = said(&[("desk", "work", &[]), ("phone", "admin+leisure", &[("work", "work+leisure"), ("meals", "none")]), ("tablet", "leisure", &[])]);
        let week = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        let known = Known { me: "desk".into(), mine: said_of["desk"].line(), others: [("phone".to_string(), seen_by(&said_of, "phone", "desk", week))].into(), older: vec!["tablet".into()] };
        let at: jiff::Zoned = "2026-10-02T19:00[Europe/Paris]".parse().unwrap();
        let evening = mode(Time::Leisure, Reason::Evening, Some("2026-10-02T22:00[Europe/Paris]"), &known.lines());
        let mut rows = rows();
        let grid = grid(&mut rows, &said_of, &known, &evening, &at, &english);
        assert_eq!(grid.times, ["Work", "Admin", "Leisure", "Meals", "Sleep"]);
        fn labels(row: &Row) -> Vec<&str> {
            row.cells.iter().map(|c| c.label.as_str()).collect()
        }
        assert_eq!(labels(&rows[0]), ["Work", "Nothing", "Nothing", "Nothing", "Nothing"]);
        assert_eq!(labels(&rows[1]), ["Work · Leisure", "Admin", "Leisure", "Nothing", "Leisure"]);
        assert_eq!(rows[1].cells.iter().map(|c| c.own).collect::<Vec<_>>(), [true, false, false, true, false]);
        assert_eq!((rows[1].cells[0].time.as_str(), rows[1].cells[0].areas.as_str(), rows[1].cells[3].areas.as_str()), ("work", "work+leisure", ""));
        assert_eq!((rows[1].cells[0].work, rows[1].cells[0].admin, rows[1].cells[0].leisure), (true, false, true));
        assert_eq!(rows[1].cells[0].name, "The phone (Moss), Work: Work · Leisure, as you changed it");
        assert_eq!(rows[0].cells[1].name, "Work desk, Admin: Nothing");
        assert_eq!((rows[1].cells[3].line.as_str(), rows[1].cells[2].line.as_str()), ("Meals: Nothing (as you changed it)", "Leisure: Leisure"));
        // An older Sioul: no cell in force, one sentence.
        assert!(rows[0].follows && rows[1].follows && !rows[2].follows && rows[2].cells.is_empty());
        assert_eq!(rows[2].cells_older, "Its Sioul is older: everything still comes there.");
        assert_eq!((rows[0].cells_apart.as_str(), rows[1].cells_apart.as_str()), ("", ""), "each acts on the line shown");
        // The phone's own entry says other cells (it does not share your settings): said under its cells.
        let apart = Known { others: [("phone".to_string(), Seen::default())].into(), ..known.clone() };
        let mut other_rows = rows.clone();
        super::grid(&mut other_rows, &said_of, &apart, &evening, &at, &english);
        assert_eq!(other_rows[1].cells_apart, "Its own settings say otherwise for now: it follows those until yours reach it.");
        // Now, on the computer for work, in the evening: nothing; every area has its device.
        assert_eq!(grid.now, "Now, leisure until 22:00: nothing comes here.");
        assert_eq!(grid.unheld, "");
        assert!(grid.never.contains("doses") && grid.never.contains("Calls and texts keep their own table"));
        // On the phone, the same evening.
        let on_phone = Known { me: "phone".into(), mine: said_of["phone"].line(), others: [("desk".to_string(), seen_by(&said_of, "desk", "phone", week))].into(), older: vec!["tablet".into()] };
        let mut rows = self::rows();
        let there = super::grid(&mut rows, &said_of, &on_phone, &mode(Time::Leisure, Reason::Evening, Some("2026-10-02T22:00[Europe/Paris]"), &on_phone.lines()), &at, &english);
        assert_eq!(there.now, "Now, leisure until 22:00: leisure comes here.");
        // At work on the phone, as its changed cell says; no end known.
        let working = super::grid(&mut rows, &said_of, &on_phone, &mode(Time::Work, Reason::Working, None, &on_phone.lines()), &at, &english);
        assert_eq!(working.now, "Now, work: work and leisure come here.");
        // In a pause, which holds everything: no sentence of now.
        assert_eq!(super::grid(&mut rows, &said_of, &on_phone, &mode(Time::Sleep, Reason::Paused, None, &on_phone.lines()), &at, &english).now, "");
        // In French.
        let french = Translator::new("fr");
        let mut rows = self::rows();
        let grille = super::grid(&mut rows, &said_of, &on_phone, &mode(Time::Leisure, Reason::Evening, Some("2026-10-02T22:00[Europe/Paris]"), &on_phone.lines()), &at, &french);
        assert_eq!(grille.now, "En ce moment, les loisirs jusqu’à 22:00\u{202f}: vos loisirs arrivent ici.");
        assert_eq!(rows[1].cells[3].label, "Rien");
        assert_eq!(rows[2].cells_older, "Son Sioul est plus ancien\u{202f}: tout y arrive encore.");
    }

    /// An area no device is for: said once, under the grid, and it comes
    /// here as the hours say; with nothing said of any device, nothing is said.
    #[test]
    fn an_area_no_device_is_for_is_said_under_the_grid() {
        let english = Translator::new("en");
        let at: jiff::Zoned = "2026-10-02T19:00[Europe/Paris]".parse().unwrap();
        // The computer for work, alone: admin and leisure are for no device.
        let said_of = said(&[("desk", "work", &[])]);
        let alone = Known { me: "desk".into(), mine: said_of["desk"].line(), ..Known::default() };
        let mut rows = vec![rows().remove(0)];
        let grid = grid(&mut rows, &said_of, &alone, &mode(Time::Leisure, Reason::Evening, Some("2026-10-02T22:00[Europe/Paris]"), &alone.lines()), &at, &english);
        assert_eq!(grid.unheld, "No device is for your admin and leisure: until one is, that comes on every device, as the hours say.");
        assert_eq!(grid.now, "Now, leisure until 22:00: leisure comes here.");
        assert_eq!(rows[0].cells.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["Work", "Nothing", "Nothing", "Nothing", "Nothing"], "the cells say the device's own line");
        assert_eq!(plain(&grid).len(), 3);
        // Nothing said of any device: every cell as the hours say, nothing unheld to say.
        let nothing = Known { me: "desk".into(), ..Known::default() };
        let mut rows = vec![self::rows().remove(0)];
        let plain_grid = super::grid(&mut rows, &BTreeMap::new(), &nothing, &mode(Time::Leisure, Reason::Evening, None, &nothing.lines()), &at, &english);
        assert_eq!(rows[0].cells.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["Work", "Admin", "Leisure", "Leisure", "Leisure"]);
        assert!(rows[0].cells.iter().all(|c| !c.own));
        assert_eq!((plain_grid.unheld.as_str(), plain_grid.now.as_str()), ("", "Now, leisure: leisure comes here."));
    }

    /// Only the times the week has get a cell (review of phase 2, M2, L5):
    /// admin without hours of its own has none, work's cell holding admin
    /// as the time lends it; a phone whose only admin was in that missing
    /// cell is for admin at no time, and the sentence under the grid says so.
    #[test]
    fn a_time_the_week_lacks_has_no_cell() {
        let english = Translator::new("en");
        let at: jiff::Zoned = "2026-10-02T10:00[Europe/Paris]".parse().unwrap();
        let lent = Week { work_hours: true, admin_hours: false, meals: true, sleep: true };
        let said_of = said(&[("desk", "work", &[]), ("phone", "admin+leisure", &[("work", "leisure")])]);
        let known = Known { me: "desk".into(), mine: said_of["desk"].line(), others: [("phone".to_string(), seen_by(&said_of, "phone", "desk", lent))].into(), older: Vec::new() };
        let mode = Mode { quiet: false, time: Time::Work, week: lent, reason: Reason::Working, until: None, back: None, label: String::new(), here: sioul_core::areas::Here::default() }.on(&known.lines());
        let mut rows = self::rows()[..2].to_vec();
        let grid = grid(&mut rows, &said_of, &known, &mode, &at, &english);
        assert_eq!(grid.times, ["Work", "Leisure", "Meals", "Sleep"]);
        assert_eq!(rows[1].cells.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["Leisure", "Leisure", "Leisure", "Leisure"]);
        assert_eq!(grid.unheld, "No device is for your admin: until one is, that comes on every device, as the hours say.");
        assert_eq!(grid.now, "Now, work: work and your admin come here.");
    }
}

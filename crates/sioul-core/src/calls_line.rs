// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Calls, per phone (docs/android.md, "Calls, per phone"): what one phone
//! lets ring, said on its card in Settings ▸ Devices, from any device.
//!
//! Kept in the shared configuration beside the device's name and areas, as
//! **one value** (`[device."<id>"] calls = "…"`, `device_said`), read by that
//! phone alone, which builds its calls' table from it (`calls::build`). The
//! value is a table written whole at each change (JSON in a string), so that
//! two devices writing at once leave one of the two tables, never a mix
//! (review of `phones`, L3, L16):
//! - **a profile** (`profile`): professional, personal, both, as-android
//!   (Sioul screens nothing on that phone), or `none` once a cell or a switch
//!   was changed by hand;
//! - **one small table** (`rows.<row>`, four letters): the rows Safe, Your
//!   contacts, Restricted, Everyone else (numbers nobody knows), hidden
//!   numbers, and any group of your contacts given a row (`groups.<name>`, a
//!   category as your cards write it); the columns work hours, the rest of
//!   the day, the night, while busy. Each cell is `R` (rings) or `V`
//!   (voicemail); while busy also `U`, as usual: as the moment's column says,
//!   or as do-not-disturb says where being busy turns it on (H2, M1). Always
//!   through is a row of its own that always rings, never written;
//! - **five switches**: commercial and automated callers never ring
//!   (`commercial`, the shared screening's `automated`), numbers that cannot
//!   be a caller's (`impossible`), a failed verification (`unverified`), a
//!   second call within 15 minutes rings (`twice`), numbers called in the
//!   last 14 days ring as your contacts do (`called`). Each stands for one
//!   setting (M3).
//!
//! Read leniently: a value that does not read, or a line missing a row or a
//! switch, is no line (a partial line reads as no line); a field or a group
//! this Sioul does not read is passed over. **A line that says nothing
//! leaves the phone as it was**: its table is the shared `[screening]` and
//! the matrix's Calls rows, as before (`calls::build`). The card then shows
//! what the phone does today (`Today`), and the first change of one cell
//! writes that cell with whatever reproduces what the phone did: the busy
//! column as usual, the switches as the screening's; a cell that varied
//! within its column starts ringing, said with Undo.

use crate::areas::Area;
use serde::Serialize;
use std::path::Path;

/// The key of the calls' value in a device's line.
pub const KEY: &str = "calls";
/// The value's format.
pub const FORMAT: u64 = 1;
/// What starts a group's row in the window: `group:School`.
pub const GROUP: &str = "group:";
/// The five switches' keys, in the order shown; the last two under "More".
pub const SWITCHES: [&str; 5] = ["commercial", "twice", "called", "impossible", "unverified"];
/// The switches shown folded, under "More" (M3).
pub const MORE: [&str; 2] = ["impossible", "unverified"];

/// Where a phone's table starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// A phone for work: numbers nobody knows ring in work hours, commercial
    /// callers never, everything but Always through to voicemail outside.
    Professional,
    /// A phone of your own: your contacts ring, numbers nobody knows go to
    /// voicemail unless they call twice.
    Personal,
    /// One phone for both: work's rows in work hours, the personal ones outside.
    Both,
    /// As Android does: Sioul screens nothing on that phone (the blocked aside).
    AsAndroid,
    /// Cells or switches changed by hand ("none" in the value).
    Own,
}

impl Profile {
    /// The profiles offered, in the order shown.
    pub const OFFERED: [Profile; 4] = [Profile::Professional, Profile::Personal, Profile::Both, Profile::AsAndroid];

    pub fn id(self) -> &'static str {
        match self {
            Profile::Professional => "professional",
            Profile::Personal => "personal",
            Profile::Both => "both",
            Profile::AsAndroid => "as-android",
            Profile::Own => "none",
        }
    }

    /// A word of the value; none for any other.
    pub fn read(word: &str) -> Option<Profile> {
        Some(match word.trim().to_lowercase().as_str() {
            "professional" | "pro" | "work" => Profile::Professional,
            "personal" | "perso" => Profile::Personal,
            "both" => Profile::Both,
            "as-android" | "android" => Profile::AsAndroid,
            "none" | "own" => Profile::Own,
            _ => return None,
        })
    }

    /// The table it fills (design of 18 November 2026, section 2; the owner's
    /// three examples are the tests). While busy, each row as usual: no
    /// profile's sentence says otherwise (decision on H2). `Own` has no table
    /// of its own: everything rings.
    pub fn grid(self) -> Grid {
        use Cell::{Rings as R, Usual as U, Voicemail as V};
        // Columns: work hours, the rest of the day, the night, while busy.
        // Rows: Safe, Your contacts, Restricted, Everyone else, hidden numbers.
        let on = Switches { commercial: true, impossible: true, unverified: true, twice: true, called: true };
        let (fixed, switches) = match self {
            Profile::Professional => ([[R, V, V, U]; 5], Switches { twice: false, ..on }),
            Profile::Personal => ([[R, R, V, U], [R, R, V, U], [R, V, V, U], [V, V, V, U], [R, V, V, U]], on),
            Profile::Both => ([[R, R, V, U], [R, R, V, U], [R, V, V, U], [R, V, V, U], [R, V, V, U]], on),
            Profile::AsAndroid => ([[R, R, R, U]; 5], Switches { commercial: false, impossible: false, unverified: false, ..on }),
            Profile::Own => ([[R, R, R, U]; 5], on),
        };
        Grid { profile: self, fixed, groups: Vec::new(), switches }
    }

    /// The profile offered first to a phone said for `area`: work alone,
    /// Professional; admin or leisure without work, Personal; else Both.
    pub fn offered(area: Option<Area>) -> Profile {
        match area {
            Some(area) if area.work && !area.admin && !area.leisure => Profile::Professional,
            Some(area) if !area.work && (area.admin || area.leisure) => Profile::Personal,
            _ => Profile::Both,
        }
    }
}

/// A column of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum When {
    /// Your work hours.
    Work,
    /// The rest of the day: your admin, leisure, meals, Free time.
    Day,
    /// The night, from winding down to waking, and naps.
    Night,
    /// While you are busy: an event or a task marked busy (`busy::stretches`),
    /// as do-not-disturb reads it (none held off, none while Reachable).
    Busy,
}

impl When {
    pub const ALL: [When; 4] = [When::Work, When::Day, When::Night, When::Busy];

    pub fn id(self) -> &'static str {
        match self {
            When::Work => "work",
            When::Day => "day",
            When::Night => "night",
            When::Busy => "busy",
        }
    }

    pub fn read(word: &str) -> Option<When> {
        When::ALL.into_iter().find(|w| w.id() == word.trim())
    }

    /// Its place in a row's cells.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The moment's column of a time as the matrix reads it: the night
    /// (sleep, naps), else work hours, else the rest of the day. Read by the
    /// line's frames and by today's table alike.
    pub fn of(now: &crate::attention::Now) -> When {
        use crate::attention::Column;
        if now.times.contains(&Column::Sleep) {
            When::Night
        } else if now.times.contains(&Column::Work) {
            When::Work
        } else {
            When::Day
        }
    }
}

/// A row of the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Row {
    Safe,
    Contacts,
    Restricted,
    /// Everyone else: numbers nobody knows.
    Unknown,
    Hidden,
    /// A group of your contacts (a category of their cards), as written.
    Group(String),
}

impl Row {
    /// The rows every table has, Always through aside.
    pub const FIXED: [Row; 5] = [Row::Safe, Row::Contacts, Row::Restricted, Row::Unknown, Row::Hidden];

    pub fn id(&self) -> String {
        match self {
            Row::Safe => "safe".into(),
            Row::Contacts => "contacts".into(),
            Row::Restricted => "restricted".into(),
            Row::Unknown => "unknown".into(),
            Row::Hidden => "hidden".into(),
            Row::Group(name) => format!("{GROUP}{name}"),
        }
    }

    /// A row's key; none for one this Sioul does not read.
    pub fn read(key: &str) -> Option<Row> {
        Some(match key.trim() {
            "safe" => Row::Safe,
            "contacts" => Row::Contacts,
            "restricted" => Row::Restricted,
            "unknown" => Row::Unknown,
            "hidden" => Row::Hidden,
            other => Row::Group(group_name(other.strip_prefix(GROUP)?)?),
        })
    }

    fn fixed_index(&self) -> Option<usize> {
        Row::FIXED.iter().position(|r| r == self)
    }
}

/// A group's name as written, its spaces made single; none when empty or
/// holding a control character.
fn group_name(name: &str) -> Option<String> {
    let name: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty() && !name.chars().any(char::is_control)).then_some(name)
}

/// One cell: rings or voicemail; while busy also as usual (H2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cell {
    Rings,
    Voicemail,
    /// While busy: as the moment's column says, or as do-not-disturb says
    /// where being busy turns it on.
    Usual,
}

impl Cell {
    pub fn id(self) -> &'static str {
        match self {
            Cell::Rings => "rings",
            Cell::Voicemail => "voicemail",
            Cell::Usual => "usual",
        }
    }

    /// The window's word or the value's letter; none for any other.
    pub fn read(word: &str) -> Option<Cell> {
        Some(match word.trim().to_lowercase().as_str() {
            "rings" | "ring" | "r" => Cell::Rings,
            "voicemail" | "v" => Cell::Voicemail,
            "usual" | "u" => Cell::Usual,
            _ => return None,
        })
    }

    fn letter(self) -> char {
        match self {
            Cell::Rings => 'R',
            Cell::Voicemail => 'V',
            Cell::Usual => 'U',
        }
    }
}

/// A row's four cells, by `When`.
pub type Cells = [Cell; 4];

/// A group's row when added: rings but at night, as usual while busy.
pub const GROUP_CELLS: Cells = [Cell::Rings, Cell::Rings, Cell::Voicemail, Cell::Usual];

/// Four letters, "RRVU": none unless each reads, as usual only while busy.
fn cells_of(text: &str) -> Option<Cells> {
    let letters: Vec<Cell> = text.trim().chars().map(|c| Cell::read(&c.to_string())).collect::<Option<_>>()?;
    let cells: Cells = letters.try_into().ok()?;
    cells[..3].iter().all(|c| *c != Cell::Usual).then_some(cells)
}

fn cells_text(cells: &Cells) -> String {
    cells.iter().map(|c| c.letter()).collect()
}

/// The five switches, each one setting (M3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Switches {
    /// Commercial and automated callers never ring (ARCEP's roots for machines).
    pub commercial: bool,
    /// Numbers that cannot be a caller's (France's numbering plan) never ring.
    pub impossible: bool,
    /// A number whose verification by the network failed never rings.
    pub unverified: bool,
    /// A second call within 15 minutes rings.
    pub twice: bool,
    /// A number you called in the last 14 days rings as your contacts do.
    pub called: bool,
}

impl Switches {
    pub fn get(&self, key: &str) -> Option<bool> {
        Some(match key {
            "commercial" => self.commercial,
            "impossible" => self.impossible,
            "unverified" => self.unverified,
            "twice" => self.twice,
            "called" => self.called,
            _ => return None,
        })
    }

    fn set(&mut self, key: &str, on: bool) -> bool {
        let slot = match key {
            "commercial" => &mut self.commercial,
            "impossible" => &mut self.impossible,
            "unverified" => &mut self.unverified,
            "twice" => &mut self.twice,
            "called" => &mut self.called,
            _ => return false,
        };
        *slot = on;
        true
    }

    /// As the shared screening says them: each switch its one setting.
    pub fn of_screening(screening: &crate::screening::Screening) -> Switches {
        use crate::screening::Spam;
        Switches { commercial: screening.automated == Spam::Voicemail, impossible: screening.impossible == Spam::Voicemail, unverified: screening.unverified == Spam::Voicemail, twice: true, called: screening.called }
    }
}

/// A phone's table as it holds: what it reads as, every row, the switches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Grid {
    /// The profile it reads as: the one written while every fixed cell and
    /// switch is that profile's, else `Own`; As Android does whatever the cells.
    pub profile: Profile,
    /// The fixed rows, in `Row::FIXED`'s order.
    pub fixed: [Cells; 5],
    /// The groups given a row, by name as written, in their names' order.
    pub groups: Vec<(String, Cells)>,
    pub switches: Switches,
}

impl Grid {
    /// A row's cells; none for a group with no row.
    pub fn cells(&self, row: &Row) -> Option<Cells> {
        match row {
            Row::Group(name) => {
                let key = crate::porch::category_key(name);
                self.groups.iter().find(|(n, _)| crate::porch::category_key(n) == key).map(|(_, c)| *c)
            }
            fixed => fixed.fixed_index().map(|i| self.fixed[i]),
        }
    }

    /// A row's cell in a column; none for a group with no row.
    pub fn cell(&self, row: &Row, when: When) -> Option<Cell> {
        self.cells(row).map(|c| c[when.index()])
    }

    /// Whether a row's cell says Rings (a group with no row, or as usual, does not).
    pub fn rings(&self, row: &Row, when: When) -> bool {
        self.cell(row, when) == Some(Cell::Rings)
    }

    /// Sioul screens nothing on that phone.
    pub fn screens_nothing(&self) -> bool {
        self.profile == Profile::AsAndroid
    }

    /// The profile it reads as, from the profile written: that profile while
    /// its cells and switches are all the profile's, else `Own`.
    fn reading_as(mut self, written: Profile) -> Grid {
        self.profile = match written {
            Profile::AsAndroid => Profile::AsAndroid,
            Profile::Own => Profile::Own,
            named => {
                let table = named.grid();
                if table.fixed == self.fixed && table.switches == self.switches { named } else { Profile::Own }
            }
        };
        self
    }

    /// The one value written in the device's line.
    pub fn text(&self) -> String {
        let rows: serde_json::Map<String, serde_json::Value> = Row::FIXED.iter().zip(&self.fixed).map(|(r, c)| (r.id(), serde_json::Value::String(cells_text(c)))).collect();
        let groups: serde_json::Map<String, serde_json::Value> = self.groups.iter().map(|(n, c)| (n.clone(), serde_json::Value::String(cells_text(c)))).collect();
        let switches: serde_json::Map<String, serde_json::Value> = SWITCHES.iter().map(|k| (k.to_string(), serde_json::Value::Bool(self.switches.get(k).unwrap_or(true)))).collect();
        serde_json::json!({ "v": FORMAT, "profile": self.profile.id(), "rows": rows, "groups": groups, "switches": switches }).to_string()
    }
}

/// What a phone's line says of calls, as read: its whole table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Said {
    pub grid: Grid,
}

impl Said {
    /// Whether the line says anything: a line that reads always does (a
    /// partial one does not read).
    pub fn says(&self) -> bool {
        true
    }

    /// The table it holds.
    pub fn grid(&self) -> Grid {
        self.grid.clone()
    }
}

/// `[device."<id>"] calls` read leniently: a value that is no string, does
/// not read, or misses a row or a switch is no line; a group this Sioul does
/// not read is passed over. An older line written as a table (never
/// released) is no line either.
pub fn read(value: &toml::Value) -> Option<Said> {
    let json: serde_json::Value = serde_json::from_str(value.as_str()?).ok()?;
    let rows = json.get("rows")?;
    let mut fixed = [[Cell::Rings; 4]; 5];
    for (i, row) in Row::FIXED.iter().enumerate() {
        fixed[i] = cells_of(rows.get(row.id())?.as_str()?)?;
    }
    let listed_switches = json.get("switches")?;
    let mut switches = Switches { commercial: true, impossible: true, unverified: true, twice: true, called: true };
    for key in SWITCHES {
        switches.set(key, listed_switches.get(key)?.as_bool()?);
    }
    let mut groups: Vec<(String, Cells)> = Vec::new();
    if let Some(listed) = json.get("groups").and_then(serde_json::Value::as_object) {
        for (name, cells) in listed {
            let (Some(name), Some(cells)) = (group_name(name), cells.as_str().and_then(cells_of)) else { continue };
            if !groups.iter().any(|(n, _)| crate::porch::category_key(n) == crate::porch::category_key(&name)) {
                groups.push((name, cells));
            }
        }
    }
    groups.sort_by_key(|(n, _)| crate::porch::category_key(n));
    let written = json.get("profile").and_then(serde_json::Value::as_str).and_then(Profile::read).unwrap_or(Profile::Own);
    Some(Said { grid: Grid { profile: written, fixed, groups, switches }.reading_as(written) })
}

// ---------------------------------------------------------------- today, for a line that says nothing

/// One cell of what a phone does today while its line says nothing: rings
/// or voicemail through its whole column, or varies within it (`rings`
/// none), with the moments it rings in and those it does not, by the
/// matrix's columns ("admin", "leisure", "meals", "free", "work", "sleep").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TodayCell {
    pub rings: Option<bool>,
    pub ringing: Vec<&'static str>,
    pub quiet: Vec<&'static str>,
}

/// What a phone whose line says nothing does today (review of `reach`, R3;
/// of `phones`, M5): read from the very moments its table is made of (the
/// clock's frames over a week, `Clock::frames`) as the phone reads them,
/// the matrix's lent cells and Free time's "Nothing at all" included. Each
/// moment goes to the line's column (`When::of`); a pause and "Nothing at
/// all" give way to the rule every phone follows (`calls::line_frames`), so
/// they count in no column; Free time, which may be taken at any time, is
/// one of the rest of the day's. The people you know by the matrix's Calls rows;
/// Everyone else and hidden numbers by the shared screening, never where
/// your contacts' row declines (`calls::decide`). While busy: as usual. The
/// switches as the shared screening says each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Today {
    /// The fixed rows, in `Row::FIXED`'s order; work hours, the rest of the day, the night.
    pub fixed: [[TodayCell; 3]; 5],
    pub switches: Switches,
}

impl Today {
    pub fn of(attention: &crate::attention::Attention, screening: &crate::screening::Screening, clock: &crate::reach::Clock, from: &jiff::Zoned, until: &jiff::Zoned) -> Today {
        use crate::attention::{Column, Level, Person};
        use crate::reach::Channel;
        let mut fixed: [[TodayCell; 3]; 5] = Default::default();
        // The week's moments, and Free time, which you may take at any time of the rest of the day.
        let moments = clock.frames(from, until).into_iter().map(|f| crate::attention::Now::of_moment(&f.moment)).chain([crate::attention::Now::time(Column::Free)]);
        for now in moments {
            if now.is(Column::Pause) || (now.nothing && now.is(Column::Free)) {
                continue;
            }
            let when = When::of(&now).index();
            let label = now.times.first().map_or("", |c| c.id());
            let hours = now.times.iter().any(|c| matches!(c, Column::Work | Column::Admin));
            let rings = |person: Person| attention.person(Channel::Calls, person, false, &now) == Level::Now;
            let neutral = rings(Person::Neutral);
            let said = [rings(Person::Safe), neutral, rings(Person::Restricted), screening.rings(false, hours) && neutral, screening.rings(true, hours) && neutral];
            for (row, rang) in said.into_iter().enumerate() {
                let cell = &mut fixed[row][when];
                let list = if rang { &mut cell.ringing } else { &mut cell.quiet };
                if !list.contains(&label) {
                    list.push(label);
                }
            }
        }
        for row in &mut fixed {
            for cell in row.iter_mut() {
                cell.rings = match (cell.ringing.is_empty(), cell.quiet.is_empty()) {
                    // Rings whenever it comes, or never comes (no work hours): rings.
                    (_, true) => Some(true),
                    (true, false) => Some(false),
                    (false, false) => None,
                };
            }
        }
        Today { fixed, switches: Switches::of_screening(screening) }
    }

    /// The table a first change starts from: each cell as today, a cell that
    /// varies ringing (a doubt rings; said with Undo, `louder`), as usual
    /// while busy (never written otherwise: H2), the switches as today.
    pub fn grid(&self) -> Grid {
        let fixed = self.fixed.clone().map(|row| {
            let [work, day, night] = row.map(|c| if c.rings.unwrap_or(true) { Cell::Rings } else { Cell::Voicemail });
            [work, day, night, Cell::Usual]
        });
        Grid { profile: Profile::Own, fixed, groups: Vec::new(), switches: self.switches }
    }

    /// What a table started from today rings more than the phone did: each
    /// fixed row and column that varied, with the moments it did not ring in.
    /// `but`: the cell the change itself sets, never counted.
    pub fn louder(&self, but: Option<(&Row, When)>) -> Vec<(Row, When, Vec<&'static str>)> {
        let mut out = Vec::new();
        for (i, row) in Row::FIXED.iter().enumerate() {
            for (w, cell) in self.fixed[i].iter().enumerate() {
                let when = When::ALL[w];
                if cell.rings.is_none() && but != Some((row, when)) {
                    out.push((row.clone(), when, cell.quiet.clone()));
                }
            }
        }
        out
    }
}

/// What `louder` says, in sentences: "A doubt rings: … Your contacts: now
/// also in Free time." in the person's words; empty for none.
pub fn louder_words(louder: &[(Row, When, Vec<&'static str>)], tr: &crate::i18n::Translator) -> String {
    if louder.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = louder
        .iter()
        .map(|(row, _, quiet)| {
            let moments: Vec<String> = quiet.iter().map(|id| tr.text(&format!("attention-column-{id}"), None)).collect();
            let mut args = crate::i18n::args();
            args.set("row", tr.text(&format!("phone-calls-row-{}", row.id()), None));
            args.set("moments", moments.join(", "));
            tr.text("phone-calls-louder-part", Some(&args))
        })
        .collect();
    let mut args = crate::i18n::args();
    args.set("what", parts.join(" "));
    tr.text("phone-calls-louder", Some(&args))
}

// ---------------------------------------------------------------- in words

/// What a phone's card needs to say beside its line.
#[derive(Clone, Copy)]
pub struct Card<'a> {
    /// What the phone is for, as you said it (`device_said::Said::area`).
    pub area: Option<Area>,
    /// Whether the phone screens calls, as it says (its `screens` in
    /// do-not-disturb's switch file); none: it says nothing yet.
    pub screens: Option<bool>,
    /// Every category of your contacts' cards, as first written: the groups a row may be given to.
    pub categories: &'a [String],
    /// What the phone does today while its line says nothing (`Today::of`).
    pub today: Option<&'a Today>,
    /// Being busy turns do-not-disturb on (the box "While busy"): what "as usual" means while busy.
    pub busy_is_dnd: bool,
    /// Who rings in a pause and under do-not-disturb, as on every phone (M4), in a sentence; empty: none said.
    pub held: &'a str,
    /// The phone's Sioul is older than the first that reads a line (M12).
    pub older: bool,
}

/// One phone's section Calls, in words, for its card in Settings ▸ Devices:
/// {set, profile, offered, profiles: [{id, label, said, chosen}], said, columns:
/// [{id, label}], rows: [{id, label, fixed, group, cells: [{when, cell,
/// rings, word, varies}]}], switches: [{key, label, on, more}], groups
/// (categories with no row yet), screens, sims, always, here, today,
/// details, busy, held, older}. A line that says nothing shows what the
/// phone does today (`Card::today`; a cell that varies within its column
/// says so, and `details` when it rings), nothing chosen; without it, the
/// profile offered.
pub fn view(said: Option<&Said>, card: &Card, tr: &crate::i18n::Translator) -> serde_json::Value {
    use serde_json::json;
    let set = said.is_some();
    let offered = Profile::offered(card.area);
    let today = if set { None } else { card.today };
    let grid = match (said, today) {
        (Some(said), _) => said.grid(),
        (None, Some(today)) => today.grid(),
        (None, None) => offered.grid(),
    };
    let chosen = set.then_some(grid.profile);
    // Each profile with what it does in a line; Professional's says that a second call does not ring outside work hours.
    let profile = |p: Profile, chosen: bool| json!({ "id": p.id(), "label": tr.text(&format!("phone-calls-profile-{}", p.id()), None), "said": tr.text(&format!("phone-calls-profile-{}-said", p.id()), None), "chosen": chosen });
    let mut profiles: Vec<serde_json::Value> = Profile::OFFERED.iter().map(|p| profile(*p, chosen == Some(*p))).collect();
    if chosen == Some(Profile::Own) {
        profiles.push(profile(Profile::Own, true));
    }
    let said_words = match chosen {
        None if today.is_some() => tr.text("phone-calls-today", None),
        None => tr.text("phone-calls-unsaid", None),
        Some(Profile::AsAndroid) => tr.text("phone-calls-android", None),
        Some(_) => String::new(),
    };
    let word = |cell: Cell| tr.text(&format!("phone-calls-{}", cell.id()), None);
    let cells = |cells: Cells| -> Vec<serde_json::Value> {
        When::ALL
            .iter()
            .map(|w| {
                let c = cells[w.index()];
                json!({ "when": w.id(), "cell": c.id(), "rings": c == Cell::Rings, "word": word(c) })
            })
            .collect()
    };
    let row = |id: String, label: String, fixed: bool, group: bool, c: Cells| json!({ "id": id, "label": label, "fixed": fixed, "group": group, "cells": cells(c) });
    // A cell of today that varies within its column: "Varies", and in a line under the table, when it rings.
    let moments = |ids: &[&str]| ids.iter().map(|id| tr.text(&format!("attention-column-{id}"), None)).collect::<Vec<_>>().join(", ");
    let mut details: Vec<String> = Vec::new();
    let mut fixed_row = |r: Row| {
        let id = r.id();
        let label = tr.text(&format!("phone-calls-row-{id}"), None);
        let c = grid.cells(&r).unwrap_or(GROUP_CELLS);
        let mut shown = row(id.clone(), label.clone(), false, false, c);
        let Some((today, i)) = today.zip(r.fixed_index()) else {
            return shown;
        };
        for (w, cell) in today.fixed[i].iter().enumerate().filter(|(_, cell)| cell.rings.is_none()) {
            let mut args = crate::i18n::args();
            args.set("row", label.clone());
            args.set("when", tr.text(&format!("phone-calls-when-{}", When::ALL[w].id()), None));
            args.set("rings", moments(&cell.ringing));
            args.set("voicemail", moments(&cell.quiet));
            shown["cells"][w]["word"] = json!(tr.text("phone-calls-varies", None));
            shown["cells"][w]["varies"] = json!(true);
            details.push(tr.text("phone-calls-varies-detail", Some(&args)));
        }
        shown
    };
    let mut rows = vec![row("always".into(), tr.text("phone-calls-row-always", None), true, false, [Cell::Rings; 4]), fixed_row(Row::Safe), fixed_row(Row::Contacts)];
    rows.extend(grid.groups.iter().map(|(name, c)| row(Row::Group(name.clone()).id(), name.clone(), false, true, *c)));
    rows.extend([fixed_row(Row::Restricted), fixed_row(Row::Unknown), fixed_row(Row::Hidden)]);
    let today_shown = today.is_some();
    let switches: Vec<serde_json::Value> = SWITCHES.iter().map(|key| json!({ "key": key, "label": tr.text(&format!("phone-calls-{key}"), None), "on": grid.switches.get(key).unwrap_or(true), "more": MORE.contains(key) })).collect();
    let taken: Vec<String> = grid.groups.iter().map(|(n, _)| crate::porch::category_key(n)).collect();
    let groups: Vec<&String> = card.categories.iter().filter(|c| !taken.contains(&crate::porch::category_key(c))).collect();
    // "As usual" while busy, in a sentence, wherever a row says it (M1).
    let usual = grid.fixed.iter().chain(grid.groups.iter().map(|(_, c)| c)).any(|c| c[When::Busy.index()] == Cell::Usual);
    let busy = if usual && !grid.screens_nothing() { tr.text(if card.busy_is_dnd { "phone-calls-usual-dnd" } else { "phone-calls-usual-time" }, None) } else { String::new() };
    json!({
        "set": set,
        "profile": chosen.map_or("", Profile::id),
        "offered": offered.id(),
        "profiles": profiles,
        "said": said_words,
        "columns": When::ALL.iter().map(|w| json!({ "id": w.id(), "label": tr.text(&format!("phone-calls-when-{}", w.id()), None) })).collect::<Vec<_>>(),
        "rows": rows,
        // Shown from what the phone does today: each cell that varies, when it rings.
        "today": today_shown,
        "details": details,
        "busy": busy,
        "held": if grid.screens_nothing() { "" } else { card.held },
        "older": if card.older { tr.text("phone-calls-older", None) } else { String::new() },
        "switches": switches,
        "groups": groups,
        "screens": card.screens,
        "not_screening": if card.screens == Some(true) { String::new() } else { tr.text("phone-calls-not-screening", None) },
        "sims": tr.text("phone-calls-sims", None),
        "always": tr.text("phone-calls-always", None),
        // What reaches you's presets leave calls as they are: they are set here.
        "here": tr.text("phone-calls-here", None),
    })
}

// ---------------------------------------------------------------- writing

/// The value as written for the phone `id` (for Undo); none when there is none.
pub fn raw(path: &Path, id: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let table: toml::Table = text.parse().ok()?;
    table.get("device")?.get(id.trim())?.get(KEY)?.as_str().map(str::to_string)
}

/// The phone `id`'s value put back as it was (Undo): `value`, or none.
pub fn put_raw(path: &Path, id: &str, value: Option<&str>) -> Result<(), String> {
    edit(path, id, |line| {
        match value {
            Some(text) => {
                line.insert(KEY, toml_edit::value(text));
            }
            None => {
                line.remove(KEY);
            }
        }
        Ok(())
    })
}

/// The phone `id`'s line in the device's table, under the file's lock: the
/// table changed by `change`, what is left empty taken away.
fn edit(path: &Path, id: &str, change: impl FnOnce(&mut dyn toml_edit::TableLike) -> Result<(), String>) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() || id.chars().any(char::is_control) {
        return Err("a device's id is needed".to_string());
    }
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        if doc.get("device").is_none_or(|item| !item.is_table_like()) {
            let mut devices = toml_edit::Table::new();
            devices.set_implicit(true);
            doc.insert("device", toml_edit::Item::Table(devices));
        }
        let devices = doc.get_mut("device").and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{}: device is not a table", path.display()))?;
        if devices.get(id).is_none_or(|line| !line.is_table_like()) {
            let mut line = toml_edit::Table::new();
            line.set_implicit(true);
            devices.insert(id, toml_edit::Item::Table(line));
        }
        let line = devices.get_mut(id).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: not a table"))?;
        change(line)?;
        if line.is_empty() {
            devices.remove(id);
        }
        if devices.is_empty() {
            doc.remove("device");
        }
        crate::config::write_document(path, &doc)
    })
}

/// The phone `id`'s table changed whole, under the file's lock: read as it
/// is (none: `start`, the table the change starts from), changed by
/// `change`, written in one value. Answers the table before (none: the line
/// said nothing).
fn change(path: &Path, id: &str, start: &Grid, change: impl FnOnce(&mut Grid) -> Result<(), String>) -> Result<Option<Grid>, String> {
    let mut before: Option<Grid> = None;
    edit(path, id, |line| {
        before = line.get(KEY).and_then(|item| item.as_value()).and_then(|v| v.as_str()).and_then(|text| read(&toml::Value::String(text.to_string()))).map(|s| s.grid);
        let mut grid = before.clone().unwrap_or_else(|| start.clone());
        change(&mut grid)?;
        line.insert(KEY, toml_edit::value(grid.text()));
        Ok(())
    })?;
    Ok(before)
}

/// A profile chosen for the phone `id`: its whole table written, the groups'
/// rows kept. `Own` is no choice: a cell or a switch makes it.
pub fn set_profile(path: &Path, id: &str, profile: Profile, start: &Grid) -> Result<Option<Grid>, String> {
    if profile == Profile::Own {
        return Err("none: change a cell or a switch".to_string());
    }
    change(path, id, start, |grid| {
        *grid = Grid { groups: std::mem::take(&mut grid.groups), ..profile.grid() };
        Ok(())
    })
}

/// One cell of the phone `id`'s table changed by hand: "as usual" only while
/// busy. The table is then the person's own.
pub fn set_cell(path: &Path, id: &str, row: &Row, when: When, cell: Cell, start: &Grid) -> Result<Option<Grid>, String> {
    if cell == Cell::Usual && when != When::Busy {
        return Err(format!("{}: as usual only while busy", when.id()));
    }
    change(path, id, start, |grid| {
        match row {
            Row::Group(name) => {
                let key = crate::porch::category_key(name);
                let Some((_, cells)) = grid.groups.iter_mut().find(|(n, _)| crate::porch::category_key(n) == key) else {
                    return Err(format!("{name}: no row"));
                };
                cells[when.index()] = cell;
            }
            fixed => {
                let i = fixed.fixed_index().ok_or_else(|| "no row".to_string())?;
                grid.fixed[i][when.index()] = cell;
            }
        }
        if grid.profile != Profile::AsAndroid {
            grid.profile = Profile::Own;
        }
        Ok(())
    })
}

/// One switch of the phone `id`'s table turned on or off by hand.
pub fn set_switch(path: &Path, id: &str, key: &str, on: bool, start: &Grid) -> Result<Option<Grid>, String> {
    change(path, id, start, |grid| {
        if !grid.switches.set(key, on) {
            return Err(format!("{key}?"));
        }
        if grid.profile != Profile::AsAndroid {
            grid.profile = Profile::Own;
        }
        Ok(())
    })
}

/// A group of your contacts given a row on the phone `id` (rings but at
/// night, as usual while busy), once: a group that has one keeps it.
pub fn add_group(path: &Path, id: &str, name: &str, start: &Grid) -> Result<Option<Grid>, String> {
    let name = group_name(name).ok_or_else(|| "a group's name is needed".to_string())?;
    change(path, id, start, |grid| {
        let key = crate::porch::category_key(&name);
        if !grid.groups.iter().any(|(n, _)| crate::porch::category_key(n) == key) {
            grid.groups.push((name.clone(), GROUP_CELLS));
            grid.groups.sort_by_key(|(n, _)| crate::porch::category_key(n));
        }
        Ok(())
    })
}

/// A group's row taken off the phone `id` (as categories compare); a line
/// that says nothing stays so.
pub fn remove_group(path: &Path, id: &str, name: &str) -> Result<Option<Grid>, String> {
    if raw(path, id).is_none() {
        return Ok(None);
    }
    let wanted = crate::porch::category_key(name);
    change(path, id, &Profile::Own.grid(), |grid| {
        grid.groups.retain(|(n, _)| crate::porch::category_key(n) != wanted);
        Ok(())
    })
}

/// The phone `id`'s line taken out: it screens as every phone does again.
pub fn clear(path: &Path, id: &str) -> Result<(), String> {
    put_raw(path, id, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Cell::{Rings as R, Usual as U, Voicemail as V};

    const PHONE: &str = "7b1c9e04-52a8-4c0d-8f0e-93c1b2a4d5e6";

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-calls-line-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    fn said_in(path: &Path) -> Option<Said> {
        crate::config::Config::load(path).unwrap().devices.get(PHONE).and_then(|s| s.calls.clone())
    }

    fn at(text: &str) -> jiff::Zoned {
        format!("{text}[Europe/Paris]").parse().unwrap()
    }

    /// Weekdays 09:00 to 17:00 at work (or none), admin on Saturday
    /// mornings; the night and a nap after lunch (Needs).
    fn clock(work: bool) -> crate::reach::Clock {
        let days = if work { ["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| format!("[[window]]\nday = \"{d}\"\nstart = \"09:00\"\nend = \"17:00\"\n\n")).collect::<String>() } else { String::new() };
        let config: crate::config::Config = toml::from_str(&format!("{days}[[window]]\nday = \"saturday\"\nstart = \"09:00\"\nend = \"12:00\"\nkind = \"admin\"\n")).unwrap();
        let needs = crate::needs::Needs { meals_on: true, naps_on: true, sleep_on: true, ..crate::needs::Needs::default() };
        crate::reach::Clock::new(&config, crate::quiet::Overrides::default(), needs, crate::needs::Days::default(), Vec::new())
    }

    fn today(attention: &crate::attention::Attention, screening: &crate::screening::Screening, work: bool) -> Today {
        Today::of(attention, screening, &clock(work), &at("2026-11-16T00:00"), &at("2026-11-23T00:00"))
    }

    fn value(json: &serde_json::Value) -> toml::Value {
        toml::Value::String(json.to_string())
    }

    /// The one value: written whole, read back the same; a partial or
    /// unreadable value, or an older table, is no line; a group that does
    /// not read is passed over; as usual only while busy.
    #[test]
    fn the_line_is_one_value_read_leniently() {
        let mut grid = Profile::Personal.grid();
        grid.groups.push(("School".into(), GROUP_CELLS));
        let back = read(&toml::Value::String(grid.text())).unwrap().grid;
        assert_eq!(back, grid);
        assert_eq!(back.profile, Profile::Personal, "a profile's table reads as that profile");
        let whole: serde_json::Value = serde_json::from_str(&grid.text()).unwrap();
        let mut broken = Vec::new();
        for (path, new) in [("/rows/safe", Some("RRV")), ("/rows/safe", Some("RUVU")), ("/switches/called", None), ("/rows/hidden", None)] {
            let mut json = whole.clone();
            match new {
                Some(text) => *json.pointer_mut(path).unwrap() = serde_json::json!(text),
                None => {
                    let (parent, key) = path.rsplit_once('/').unwrap();
                    json.pointer_mut(parent).unwrap().as_object_mut().unwrap().remove(key);
                }
            }
            broken.push(value(&json));
        }
        broken.push(toml::Value::String("{".into()));
        broken.push(toml::Value::Integer(3));
        for b in &broken {
            assert_eq!(read(b), None, "a partial line reads as no line: {b}");
        }
        let mut odd = whole.clone();
        odd["groups"]["Lunch"] = serde_json::json!("ZZZZ");
        odd["groups"][""] = serde_json::json!("RRRR");
        odd["later"] = serde_json::json!(3);
        assert_eq!(read(&value(&odd)).unwrap().grid.groups, grid.groups, "groups that do not read are passed over, fields this Sioul does not know too");
        let older: crate::config::Config = toml::from_str(&format!("[device.\"{PHONE}\".calls]\nprofile = \"personal\"\n")).unwrap();
        assert_eq!(older.devices[PHONE].calls, None, "a table of before is no line");
        let mut changed = Profile::Personal.grid();
        changed.fixed[0][2] = R;
        assert_eq!(read(&toml::Value::String(changed.text())).unwrap().grid.profile, Profile::Own, "cells apart from the profile's: your own");
    }

    /// The card's words, in both languages: a line that says nothing shows
    /// the profile offered, nothing chosen; a line by hand reads "Your own";
    /// As Android does says Sioul screens nothing; the rows in the design's
    /// order, Always through fixed; the groups not given a row offered; a
    /// phone that does not screen says so; as usual while busy said in a
    /// sentence, one for each meaning; the switches each one setting, two
    /// under "More"; an older phone said.
    #[test]
    fn the_card_says_the_line_in_words() {
        for language in ["en", "fr"] {
            let tr = crate::i18n::Translator::new(language);
            let categories = ["School".to_string(), "Clients".to_string()];
            let card = Card { area: Some(Area::WORK), screens: Some(false), categories: &categories, today: None, busy_is_dnd: false, held: "", older: false };
            let unsaid = view(None, &card, &tr);
            assert_eq!((unsaid["set"].as_bool(), unsaid["profile"].as_str(), unsaid["offered"].as_str()), (Some(false), Some(""), Some("professional")));
            assert!(unsaid["profiles"].as_array().unwrap().iter().all(|p| p["chosen"] == false));
            assert!(!unsaid["not_screening"].as_str().unwrap().is_empty());
            let mut own = Profile::Personal.grid();
            own.fixed[0][2] = R;
            own.groups.push(("School".into(), GROUP_CELLS));
            let own = read(&toml::Value::String(own.text())).unwrap();
            let view_own = view(Some(&own), &Card { screens: Some(true), busy_is_dnd: true, older: true, ..card }, &tr);
            assert_eq!(view_own["profiles"].as_array().unwrap().last().unwrap()["id"], "none", "a cell changed by hand: Your own");
            assert!(view_own["profiles"].as_array().unwrap()[..4].iter().all(|p| p["chosen"] == false), "no profile shown chosen once a cell was changed by hand");
            let ids: Vec<&str> = view_own["rows"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();
            assert_eq!(ids, ["always", "safe", "contacts", "group:School", "restricted", "unknown", "hidden"]);
            assert_eq!(view_own["rows"][0]["fixed"], true);
            assert_eq!(view_own["rows"][1]["cells"][3]["cell"], "usual");
            assert_eq!(view_own["groups"], serde_json::json!(["Clients"]), "School has its row");
            assert_eq!(view_own["not_screening"], "");
            assert_ne!(view_own["busy"], unsaid["busy"], "as usual while busy: what do-not-disturb lets through when being busy turns it on");
            assert!(!view_own["older"].as_str().unwrap().is_empty());
            let keys: Vec<(&str, bool)> = view_own["switches"].as_array().unwrap().iter().map(|s| (s["key"].as_str().unwrap(), s["more"] == true)).collect();
            assert_eq!(keys, [("commercial", false), ("twice", false), ("called", false), ("impossible", true), ("unverified", true)]);
            let android = view(Some(&Said { grid: Profile::AsAndroid.grid() }), &card, &tr);
            assert!(!android["said"].as_str().unwrap().is_empty());
            let professional = &unsaid["profiles"][0];
            assert_eq!(professional["id"], "professional");
            if language == "en" {
                assert!(professional["said"].as_str().unwrap().contains("a second call within 15 minutes does not ring outside work hours"), "{professional}");
            }
            let mut words: Vec<String> = Vec::new();
            for v in [&unsaid, &view_own, &android] {
                words.extend(v["profiles"].as_array().unwrap().iter().map(|p| p["said"].as_str().unwrap().to_string()));
                for key in ["said", "sims", "always", "not_screening", "busy", "older", "here"] {
                    words.push(v[key].as_str().unwrap().to_string());
                }
                for list in ["profiles", "columns", "rows", "switches"] {
                    words.extend(v[list].as_array().unwrap().iter().filter_map(|x| x["label"].as_str().map(str::to_string)));
                }
                words.extend(v["rows"].as_array().unwrap().iter().flat_map(|r| r["cells"].as_array().unwrap().iter().map(|c| c["word"].as_str().unwrap().to_string())));
            }
            for w in words.iter().filter(|w| !w.is_empty()) {
                assert!(!w.starts_with("phone-calls-"), "{language}: {w}");
                if language == "fr" {
                    assert!(![" :", " ;", " ?", " !", "'"].iter().any(|bad| w.contains(bad)), "{w}");
                }
            }
        }
    }

    /// A phone whose line says nothing shows what it does today, from the
    /// very moments its table is made of (R3, M5): the safe ring at work and
    /// in the day, to voicemail at night; your contacts' rest of the day
    /// varies, said with when they ring; while busy, as usual. Without work
    /// hours, admin time lends work's cells, as the phone reads them.
    #[test]
    fn an_unsaid_line_shows_what_the_phone_does_today() {
        let attention = crate::attention::Attention::usual();
        let screening = crate::screening::Screening::default();
        let shown = today(&attention, &screening, true);
        assert_eq!(shown.fixed[0][0].rings, Some(true), "safe at work");
        assert_eq!(shown.fixed[0][2].rings, Some(false), "safe at night");
        assert_eq!(shown.fixed[1][1].rings, None, "your contacts in the rest of the day: {:?}", shown.fixed[1][1]);
        assert_eq!(shown.fixed[3][2].rings, Some(false), "nobody known at night");
        let grid = shown.grid();
        assert!(grid.fixed.iter().all(|row| row[3] == U), "while busy: as usual, never written otherwise");
        for language in ["en", "fr"] {
            let tr = crate::i18n::Translator::new(language);
            let categories: Vec<String> = Vec::new();
            let card = Card { area: None, screens: Some(true), categories: &categories, today: Some(&shown), busy_is_dnd: false, held: "", older: false };
            let view = view(None, &card, &tr);
            assert!(view["profiles"].as_array().unwrap().iter().all(|p| p["chosen"] == false));
            let contacts = view["rows"].as_array().unwrap().iter().find(|r| r["id"] == "contacts").unwrap().clone();
            assert_eq!(contacts["cells"][1]["varies"], true);
            assert_eq!(view["today"], true);
            let details: Vec<&str> = view["details"].as_array().unwrap().iter().filter_map(serde_json::Value::as_str).collect();
            assert!(!details.is_empty(), "{language}: {details:?}");
            for detail in details {
                assert!(!detail.contains("phone-calls-") && !detail.contains("attention-column-") && !detail.contains('{'), "{language}: {detail}");
            }
            let louder = louder_words(&shown.louder(Some((&Row::Safe, When::Night))), &tr);
            assert!(!louder.is_empty() && !louder.contains("phone-calls-") && !louder.contains('{'), "{language}: {louder}");
        }
        // No work hours: admin time lends work's cells (M5), as the phone reads them.
        use crate::attention::{Column, Level, Person, Row as MatrixRow};
        let mut lending = crate::attention::Attention::usual();
        let restricted = MatrixRow::People(crate::reach::Channel::Calls, Person::Restricted);
        lending.set(restricted, Column::Work, Level::Now).unwrap();
        lending.set(restricted, Column::Admin, Level::Later).unwrap();
        let without_work = today(&lending, &screening, false);
        assert!(without_work.fixed[2][1].ringing.contains(&"admin"), "admin time takes work's cell while work has no hours: {:?}", without_work.fixed[2][1]);
    }

    /// The first change of one cell writes that cell and reproduces the rest
    /// (H2, the reviewer's test): the busy column stays as usual, so that
    /// whoever rang during an event before still does; the switches as the
    /// screening's, each its one setting (M3); a cell that varied starts
    /// ringing, and `louder` says which.
    #[test]
    fn a_first_change_writes_that_cell_and_keeps_the_rest() {
        let screening = crate::screening::Screening { unverified: crate::screening::Spam::Unknown, ..crate::screening::Screening::default() };
        let shown = today(&crate::attention::Attention::usual(), &screening, true);
        let path = scratch("first");
        std::fs::write(&path, "language = \"en\"\n").unwrap();
        let before = set_cell(&path, PHONE, &Row::Safe, When::Night, R, &shown.grid()).unwrap();
        assert_eq!(before, None, "the line said nothing");
        let grid = said_in(&path).unwrap().grid;
        assert!(grid.rings(&Row::Safe, When::Night), "the cell changed");
        assert!(grid.fixed.iter().all(|row| row[3] == U), "while busy as usual: contacts and restricted people rang during an event, and still do");
        assert!(grid.switches.commercial && grid.switches.impossible && !grid.switches.unverified, "each switch as its one setting");
        assert_eq!(grid.profile, Profile::Own);
        assert!(!shown.louder(Some((&Row::Safe, When::Night))).is_empty(), "the varying cells now ring, to be said");
        // A raw value put back (Undo): the line says nothing again.
        put_raw(&path, PHONE, None).unwrap();
        assert_eq!(said_in(&path), None);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// The owner's three examples, and the other profiles: as usual while busy.
    #[test]
    fn each_profile_fills_the_table() {
        let pro = Profile::Professional.grid();
        assert!(pro.rings(&Row::Unknown, When::Work) && !pro.rings(&Row::Unknown, When::Day) && !pro.switches.twice && pro.switches.commercial);
        let personal = Profile::Personal.grid();
        assert!(personal.rings(&Row::Contacts, When::Day) && !personal.rings(&Row::Unknown, When::Work) && personal.switches.twice);
        let both = Profile::Both.grid();
        assert_eq!(both.fixed[3], [R, V, V, U], "work's row in work hours, the personal one outside");
        for p in Profile::OFFERED {
            assert!(p.grid().fixed.iter().all(|row| row[3] == U), "{p:?}: as usual while busy");
        }
        assert!(Profile::AsAndroid.grid().screens_nothing());
        assert_eq!(Profile::offered(Some(Area::WORK)), Profile::Professional);
    }

    /// Written whole at each change: a profile keeps the groups; a cell by
    /// hand makes the table your own; a switch, a group added once and taken
    /// off; "as usual" refused outside the busy column; Clear takes the line
    /// out; nothing else of the file touched.
    #[test]
    fn the_line_is_written_whole() {
        let path = scratch("whole");
        std::fs::write(&path, format!("# mine\nlanguage = \"en\"\n\n[device.\"{PHONE}\"]\nname = \"Pocket\"\n")).unwrap();
        let start = Profile::Both.grid();
        set_profile(&path, PHONE, Profile::Personal, &start).unwrap();
        add_group(&path, PHONE, "  School ", &start).unwrap();
        add_group(&path, PHONE, "school", &start).unwrap();
        assert_eq!(said_in(&path).unwrap().grid.groups.len(), 1);
        set_profile(&path, PHONE, Profile::Professional, &start).unwrap();
        let grid = said_in(&path).unwrap().grid;
        assert_eq!((grid.profile, grid.groups.len()), (Profile::Professional, 1), "a profile keeps the groups' rows");
        assert!(set_cell(&path, PHONE, &Row::Safe, When::Work, U, &start).is_err(), "as usual only while busy");
        set_cell(&path, PHONE, &Row::Group("School".into()), When::Busy, V, &start).unwrap();
        set_switch(&path, PHONE, "unverified", false, &start).unwrap();
        let grid = said_in(&path).unwrap().grid;
        assert_eq!((grid.profile, grid.switches.unverified, grid.groups[0].1[3]), (Profile::Own, false, V));
        remove_group(&path, PHONE, "SCHOOL").unwrap();
        assert!(said_in(&path).unwrap().grid.groups.is_empty());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# mine") && text.contains("name = \"Pocket\""), "{text}");
        clear(&path, PHONE).unwrap();
        assert_eq!(said_in(&path), None);
        assert!(std::fs::read_to_string(&path).unwrap().contains("name = \"Pocket\""));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}

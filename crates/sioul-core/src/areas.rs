// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What things are for, and which time is for what (docs/areas.md).
//!
//! Two axes. Who someone is (a stranger, blocked, or safe, neutral,
//! restricted: `reach::Who`, from the lists and your address books,
//! `porch::Senders`) says who may reach you, and one matrix
//! (`attention::Attention`) when.
//! Areas say what a source (an address, a site, a chat) or a task is for:
//! work, your own admin, or leisure; and the time says what now is for. Five
//! times: work and admin (the hours you set), meals and sleep (from Health),
//! and leisure, every other time. So work does not reach your evenings, and
//! admin waits for its own hours instead of spreading over your free time:
//! recovery needs detachment from work (Sonnentag & Fritz 2007, 2015), a
//! real risk for whoever works from home or for themselves.

use serde::{Deserialize, Serialize};

/// What a source or a task is for: work, your own admin, leisure; any of
/// them together. An address for everything personal is for admin and
/// leisure; the bank account a freelancer uses for both, work and admin.
/// None at all: unsaid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Area {
    pub work: bool,
    /// Your own admin: bills, letters, offices, health errands.
    pub admin: bool,
    /// What gives back: friends, family, chats, what you enjoy.
    pub leisure: bool,
}

impl Area {
    pub const WORK: Area = Area { work: true, admin: false, leisure: false };
    pub const ADMIN: Area = Area { work: false, admin: true, leisure: false };
    pub const LEISURE: Area = Area { work: false, admin: false, leisure: true };
    /// Yours, admin or leisure.
    pub const PERSONAL: Area = Area { work: false, admin: true, leisure: true };
    /// Work and your own admin.
    pub const MIXED: Area = Area { work: true, admin: true, leisure: false };
    /// Every kind: hours open to anything (no hours set).
    pub const ALL: Area = Area { work: true, admin: true, leisure: true };

    /// "work", "admin", "leisure", joined by "+" or ","; "personal" is admin
    /// and leisure, "mixed" work and admin; French words too. None when it says none.
    pub fn parse(text: &str) -> Option<Area> {
        let mut area = Area::default();
        for word in text.split(['+', ',', ' ', '/']).map(str::trim).filter(|w| !w.is_empty()) {
            match word.to_lowercase().as_str() {
                "work" | "travail" | "pro" => area.work = true,
                "admin" | "démarches" | "demarches" => area.admin = true,
                "leisure" | "loisirs" | "fun" | "rest" => area.leisure = true,
                "personal" | "perso" | "personnel" => (area.admin, area.leisure) = (true, true),
                "mixed" => (area.work, area.admin) = (true, true),
                _ => {}
            }
        }
        (!area.is_empty()).then_some(area)
    }

    /// As written in files: "work", "admin+leisure"; "" for none.
    pub fn id(self) -> String {
        [(self.work, "work"), (self.admin, "admin"), (self.leisure, "leisure")].iter().filter(|(on, _)| *on).map(|(_, w)| *w).collect::<Vec<_>>().join("+")
    }

    pub fn is_empty(self) -> bool {
        !(self.work || self.admin || self.leisure)
    }

    /// Both together.
    pub fn with(self, other: Area) -> Area {
        Area { work: self.work || other.work, admin: self.admin || other.admin, leisure: self.leisure || other.leisure }
    }

    /// Whether the two share a kind.
    pub fn meets(self, other: Area) -> bool {
        (self.work && other.work) || (self.admin && other.admin) || (self.leisure && other.leisure)
    }

    /// How many kinds it holds, none to three.
    pub fn count(self) -> u32 {
        u32::from(self.work) + u32::from(self.admin) + u32::from(self.leisure)
    }

    /// The kinds both hold.
    pub fn both(self, other: Area) -> Area {
        Area { work: self.work && other.work, admin: self.admin && other.admin, leisure: self.leisure && other.leisure }
    }

    /// Its kinds that `other` does not hold.
    pub fn but(self, other: Area) -> Area {
        Area { work: self.work && !other.work, admin: self.admin && !other.admin, leisure: self.leisure && !other.leisure }
    }

    /// Each kind it holds, alone, in their order.
    pub fn each(self) -> impl Iterator<Item = Area> {
        [(self.work, Area::WORK), (self.admin, Area::ADMIN), (self.leisure, Area::LEISURE)].into_iter().filter(|(on, _)| *on).map(|(_, one)| one)
    }
}

impl Serialize for Area {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.id())
    }
}

impl<'de> serde::Deserialize<'de> for Area {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Area, D::Error> {
        let text = String::deserialize(deserializer)?;
        Area::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("{text}: work, admin, leisure, joined by +")))
    }
}

/// What now is for (docs/areas.md): one of five times, or several hours at
/// once, or no hours set at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Time {
    /// Working hours, and the work you asked for outside them ("A little longer", "Work now").
    Work,
    /// Hours for your own admin.
    Admin,
    /// Every other time: evenings, days without hours, time off, a day closed
    /// early. What you enjoy; work and admin wait.
    Leisure,
    /// A meal, from getting it ready to its end (Health).
    Meals,
    /// The night, from winding down to waking, and naps (Health): nothing disturbs.
    Sleep,
    /// No working or admin hours set at all: everything comes, as before any
    /// were set (meals and sleep still keep their time).
    #[default]
    Any,
    /// Working and admin hours at once: what each of them brings comes, all together.
    Several(Area),
}

impl Time {
    /// The five times, in the order the matrix of who may write to you shows them.
    pub const STATES: [Time; 5] = [Time::Work, Time::Admin, Time::Leisure, Time::Meals, Time::Sleep];

    pub fn parse(text: &str) -> Option<Time> {
        match text.trim().to_lowercase().as_str() {
            "work" | "travail" => Some(Time::Work),
            "admin" | "démarches" | "demarches" => Some(Time::Admin),
            // "rest" and "personal" were the words for time outside every hours set.
            "leisure" | "loisirs" | "free" | "rest" | "personal" | "perso" => Some(Time::Leisure),
            "meals" | "meal" | "repas" => Some(Time::Meals),
            "sleep" | "sommeil" | "night" | "nuit" => Some(Time::Sleep),
            "any" => Some(Time::Any),
            several if several.contains('+') => Area::parse(several).map(Time::of),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Time::Work => "work",
            Time::Admin => "admin",
            Time::Leisure => "leisure",
            Time::Meals => "meals",
            Time::Sleep => "sleep",
            Time::Any => "any",
            // Only work and admin have hours: they are the ones to overlap.
            Time::Several(_) => "work+admin",
        }
    }

    /// The time when the hours of `open` are open: work's, admin's, or both
    /// at once; neither is leisure. Only work and admin have hours.
    pub fn of(open: Area) -> Time {
        match (open.work, open.admin) {
            (false, false) => Time::Leisure,
            (true, false) => Time::Work,
            (false, true) => Time::Admin,
            (true, true) => Time::Several(Area::MIXED),
        }
    }

    /// Whether work's hours are among those open now.
    pub fn works(self) -> bool {
        matches!(self, Time::Work) || matches!(self, Time::Several(open) if open.work)
    }

    /// Whether offices keep hours now: work's or admin's.
    pub fn offices(self) -> bool {
        matches!(self, Time::Work | Time::Admin | Time::Any) || matches!(self, Time::Several(open) if open.work || open.admin)
    }
}

/// Which times your week holds: work and admin when they have hours (until
/// admin has hours of its own, it comes in work time as it always did; until
/// work has some, it comes in admin's), meals and sleep when Health sets them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Week {
    pub work_hours: bool,
    pub admin_hours: bool,
    /// Meals are set (Health).
    pub meals: bool,
    /// A night or naps are set (Health).
    pub sleep: bool,
}

impl Week {
    /// Whether `time` comes in this week: leisure always does.
    pub fn has(self, time: Time) -> bool {
        match time {
            Time::Work => self.work_hours,
            Time::Admin => self.admin_hours,
            Time::Meals => self.meals,
            Time::Sleep => self.sleep,
            Time::Leisure | Time::Any => true,
            Time::Several(open) => (open.work && self.work_hours) || (open.admin && self.admin_hours),
        }
    }

    /// Whether working or admin hours are set at all: without either, the
    /// day is `Time::Any`, and no work or admin cell is ever in force.
    pub fn has_hours(self) -> bool {
        self.work_hours || self.admin_hours
    }
}

/// What `time` brings (docs/areas.md, "What each time brings"): the areas
/// that come forward then. Work brings work, and admin while admin has no
/// hours of its own; admin brings admin, and work while work has none;
/// leisure, meals and sleep bring what is for leisure (a chat with friends),
/// nothing else: a meal is a break, and during sleep nothing notifies anyway,
/// as usual (`attention`); no task then (`quiet::QuietTasks`). No hours set
/// at all: everything. Working and admin hours at once: what each of them
/// brings.
pub fn brought(time: Time, week: Week) -> Area {
    match time {
        Time::Work => Area { work: true, admin: !week.admin_hours, leisure: false },
        Time::Admin => Area { work: !week.work_hours, admin: true, leisure: false },
        Time::Leisure | Time::Meals | Time::Sleep => Area::LEISURE,
        Time::Any => Area::ALL,
        Time::Several(open) => {
            let at = |on: bool, time: Time| if on { brought(time, week) } else { Area::default() };
            at(open.work, Time::Work).with(at(open.admin, Time::Admin))
        }
    }
}

/// Whether something for `area` comes forward in `time`, by the hours alone:
/// its area meets what the time brings (`brought`). With no hours set at
/// all, everything comes, a thing whose area is unsaid too. Mail has its own
/// rule, by who wrote (`attention::Attention::mail`).
///
/// This is the person's week, the same on every device: the plan and "this
/// row and this address never meet in your week" read it. What a device
/// shows or tells now reads `Here::brings` instead, which narrows it to what
/// that device is for (docs/areas.md, "What each device is for").
pub fn in_view(area: Area, time: Time, week: Week) -> bool {
    matches!(time, Time::Any) || area.meets(brought(time, week))
}

/// What the person said of one device (docs/areas.md, "What each device is
/// for"; `[device."<id>"]` in the shared settings): what it is for, and the
/// cells of its line they changed, one per time.
///
/// A device has five cells, one per time: the areas that come forward on it
/// then. A cell is what the time brings (`brought`) narrowed to what the
/// device is for; a cell the person changed is the whole answer for that
/// device at that time: it may hold an area the time does not bring (friends
/// reaching a phone during work), and it may be nothing. A line that says
/// nothing narrows nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Line {
    /// What the device is for; none: everything.
    pub area: Option<Area>,
    /// The cells the person changed, in the order of `Time::STATES`; an
    /// empty area is their word for nothing.
    pub at: [Option<Area>; 5],
}

/// The word for a cell that holds nothing, as written in the settings
/// (`[device."<id>".at] work = "none"`).
pub const NOTHING: &str = "none";

impl Line {
    /// A device for `area`, no cell changed.
    pub fn for_area(area: Area) -> Line {
        Line { area: Some(area).filter(|a| !a.is_empty()), at: [None; 5] }
    }

    /// The same line with the cell of `time` changed to `areas` (none: as its areas say).
    pub fn with_cell(mut self, time: Time, areas: Option<Area>) -> Line {
        if let Some(slot) = slot(time) {
            self.at[slot] = areas;
        }
        self
    }

    /// Nothing said of it: no area, no cell changed.
    pub fn says_nothing(self) -> bool {
        self.area.is_none() && self.at.iter().all(Option::is_none)
    }

    /// What it is for: everything while unsaid.
    pub fn is_for(self) -> Area {
        self.area.unwrap_or(Area::ALL)
    }

    /// The cell the person changed for one of the five times, if any.
    pub fn changed(self, time: Time) -> Option<Area> {
        slot(time).and_then(|slot| self.at[slot])
    }

    /// A cell as the device's areas say, the person's change aside: what the
    /// time brings, narrowed to what the device is for.
    pub fn usual(self, time: Time, week: Week) -> Area {
        brought(time, week).both(self.is_for())
    }

    /// The areas that come forward on the device in `time`: the cell the
    /// person changed, else as its areas say (`usual`). Working and admin
    /// hours at once: the cells of both. No hours set at all: what the
    /// device is for, since no time is known to read a cell for.
    pub fn cell(self, time: Time, week: Week) -> Area {
        match time {
            Time::Any => self.is_for(),
            Time::Several(open) => {
                let at = |on: bool, time: Time| if on { self.cell(time, week) } else { Area::default() };
                at(open.work, Time::Work).with(at(open.admin, Time::Admin))
            }
            one => self.changed(one).unwrap_or_else(|| self.usual(one, week)),
        }
    }

    /// Every area that comes forward on the device at some time of `week`:
    /// the cells of the times the week has (a time it lacks never comes, so
    /// its cell is in force nowhere: review of phase 2, M2), each with what
    /// the time lends (admin in work's cells while admin has no hours of its
    /// own, work in admin's while work has none: `brought`), and, while the
    /// week sets no hours at all, what it is for (the day is `Time::Any`).
    pub fn holds(self, week: Week) -> Area {
        let cells = Time::STATES.into_iter().filter(|time| week.has(*time)).fold(Area::default(), |all, time| all.with(self.cell(time, week)));
        if week.has_hours() { cells } else { cells.with(self.is_for()) }
    }

    /// A cell as the settings write it: "work", "admin+leisure", `NOTHING`.
    pub fn cell_id(areas: Area) -> String {
        if areas.is_empty() { NOTHING.to_string() } else { areas.id() }
    }

    /// A cell read from the settings: the word for nothing, or areas as
    /// everywhere (`Area::parse`); none when it says neither (the cell is
    /// then as the device's areas say).
    pub fn read_cell(text: &str) -> Option<Area> {
        match text.trim().to_lowercase().as_str() {
            "none" | "nothing" | "rien" | "aucun" | "-" => Some(Area::default()),
            other => Area::parse(other),
        }
    }
}

/// Where one of the five times sits in `Time::STATES`.
fn slot(time: Time) -> Option<usize> {
    Time::STATES.iter().position(|t| *t == time)
}

/// What comes forward on one device at each time: its line, read in the
/// week of whoever reads it, or the cells it says of itself in its sealed
/// entry (docs/areas.md, "Which device tells"; sioul-sync's
/// `devices::Entry::cells`). `Cells::default()` is a line nothing is said
/// in: what the hours bring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cells {
    /// A line (`Line::cell`): this device's own, as its settings say.
    Line(Line),
    /// What a device says of itself: each of the five times
    /// (`Time::STATES`) its own week has, the areas then (none for a time
    /// its week lacks); and what it is for (`any`), which is what comes
    /// while no hours are set at all.
    Said { at: [Option<Area>; 5], any: Area },
}

impl Default for Cells {
    fn default() -> Cells {
        Cells::Line(Line::default())
    }
}

impl From<Line> for Cells {
    fn from(line: Line) -> Cells {
        Cells::Line(line)
    }
}

impl Cells {
    /// What a device says of itself, its line read in its own week: the
    /// times its week has, and what it is for.
    pub fn of(line: Line, week: Week) -> Cells {
        Cells::Said { at: Time::STATES.map(|time| week.has(time).then(|| line.cell(time, week))), any: line.is_for() }
    }

    /// Whether it narrows anything: a line something is said in, or a
    /// device that says its cells.
    pub fn narrows(self) -> bool {
        match self {
            Cells::Line(line) => !line.says_nothing(),
            Cells::Said { .. } => true,
        }
    }

    /// The areas that come forward on the device in `time`. Said cells: a
    /// time its own week lacks brings nothing there; no hours known, what it
    /// is for; working and admin hours at once, both cells.
    pub fn cell(self, time: Time, week: Week) -> Area {
        match self {
            Cells::Line(line) => line.cell(time, week),
            Cells::Said { at, any } => match time {
                Time::Any => any,
                Time::Several(open) => {
                    let at = |on: bool, time: Time| if on { slot(time).and_then(|s| at[s]).unwrap_or_default() } else { Area::default() };
                    at(open.work, Time::Work).with(at(open.admin, Time::Admin))
                }
                one => slot(one).and_then(|s| at[s]).unwrap_or_default(),
            },
        }
    }

    /// Every area that comes forward on the device at some time of its
    /// week (`Line::holds`): said cells, those its week has, and what it is
    /// for when that week sets no hours.
    pub fn holds(self, week: Week) -> Area {
        match self {
            Cells::Line(line) => line.holds(week),
            Cells::Said { at, any } => {
                let cells = at.iter().flatten().fold(Area::default(), |all, cell| all.with(*cell));
                if at[0].is_some() || at[1].is_some() { cells } else { cells.with(any) }
            }
        }
    }

    /// As a device's entry says them (`devices::Entry::cells`): by time,
    /// `Line::cell_id` ("work+leisure", "none"); `any`, what it is for. Empty
    /// for a line nothing is said in: the entry then says nothing of its cells.
    pub fn words(self, week: Week) -> std::collections::BTreeMap<String, String> {
        if !self.narrows() {
            return std::collections::BTreeMap::new();
        }
        let Cells::Said { at, any } = (match self {
            Cells::Line(line) => Cells::of(line, week),
            said => said,
        }) else {
            return std::collections::BTreeMap::new();
        };
        let mut words: std::collections::BTreeMap<String, String> = Time::STATES.into_iter().zip(at).filter_map(|(time, cell)| cell.map(|cell| (time.id().to_string(), Line::cell_id(cell)))).collect();
        words.insert(ANY.to_string(), Line::cell_id(any));
        words
    }

    /// Read from an entry's words (`words`): none said, a line nothing is
    /// said in; a time or words this Sioul does not know, passed over (that
    /// time brings nothing there); `any` missing, everything.
    pub fn read(words: &std::collections::BTreeMap<String, String>) -> Cells {
        if words.is_empty() {
            return Cells::default();
        }
        let at = Time::STATES.map(|time| words.get(time.id()).and_then(|cell| Line::read_cell(cell)));
        Cells::Said { at, any: words.get(ANY).and_then(|cell| Line::read_cell(cell)).unwrap_or(Area::ALL) }
    }

    /// The words as one line, the same for the same cells on every device:
    /// what a device says it acts on of another (`devices::Entry::known`);
    /// "" for a line nothing is said in.
    pub fn key(words: &std::collections::BTreeMap<String, String>) -> String {
        words.iter().map(|(time, cell)| format!("{time}={cell}")).collect::<Vec<_>>().join(";")
    }
}

/// The key of what comes while no hours are set, in a device's said cells.
pub const ANY: &str = "any";

/// This device among the person's devices (docs/areas.md, "What each device
/// is for"): its cells, and what the cells of every device that counts say
/// together. `Here::default()` is a device nobody said anything of, alone:
/// it narrows nothing, and every answer is the hours' own (`in_view`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Here {
    /// This device's own line, as its settings say it (the task pages read
    /// whether a cell was changed: `quiet::QuietTasks`); a line that says
    /// nothing when the device is read as another one, the keeper of the
    /// notices (`Lines::mine`).
    pub line: Line,
    /// What comes forward on the device read: its line, or the cells
    /// another device says of itself.
    pub mine: Cells,
    /// For each of the five times (`Time::STATES`), the areas that come
    /// forward then on some device that counts, this one included.
    pub somewhere: [Area; 5],
    /// The areas no device that counts brings at any time of the week: no
    /// device narrows them, so that a phone lost or forgotten never makes
    /// what was for it vanish from every device.
    pub unheld: Area,
}

/// What a device knows of the devices that count, to read one of them
/// (docs/areas.md, "Which device tells"): the device read (`mine`: this
/// one's own line, or the cells the keeper of the notices says of itself),
/// and the others that count with it. A device whose Sioul is older is left
/// out by whoever lists them: it never holds a thing for later.
/// `Lines::default()`: nothing said of any device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lines {
    pub mine: Cells,
    /// The device read's cells as the others name them (`Cells::key` of
    /// what its entry says); none: its line's own, in the week read.
    pub key: Option<String>,
    pub others: Vec<Other>,
}

/// Another device that counts, as the device reading knows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Other {
    /// What comes forward on it, as its entry says.
    pub cells: Cells,
    /// What it says it acts on of the device read (`Cells::key`); none:
    /// not asked. It holds a thing for later only once it acts on the
    /// device read's own cells: until then it might claim on a word the
    /// device read no longer says, and the device read tells for it
    /// (review of phase 2, H1 and M3).
    pub acts_on: Option<String>,
}

impl Lines {
    /// A device's own line among others' lines (tests, and a device alone).
    pub fn of(mine: Line, others: &[Line]) -> Lines {
        Lines { mine: Cells::Line(mine), key: None, others: others.iter().map(|line| Other { cells: Cells::Line(*line), acts_on: None }).collect() }
    }

    /// The device read among them, in a week: with the others that act on its own cells.
    pub fn here(&self, week: Week) -> Here {
        let key = self.key.clone().unwrap_or_else(|| Cells::key(&self.mine.words(week)));
        let counted: Vec<Cells> = self.others.iter().filter(|other| other.acts_on.as_ref().is_none_or(|on| *on == key)).map(|other| other.cells).collect();
        Here::among(self.mine, &counted, week)
    }

    /// Nothing said of the device read: it narrows nothing.
    pub fn say_nothing(&self) -> bool {
        !self.mine.narrows()
    }
}

impl Here {
    /// A device (`mine`) among the `others` that count, in a week. A
    /// device whose Sioul does not follow what devices are for yet is left
    /// out by whoever lists them: it never keeps a thing back for later, so
    /// nothing it could be for is counted as held by it.
    pub fn among(mine: Cells, others: &[Cells], week: Week) -> Here {
        let all = || std::iter::once(mine).chain(others.iter().copied());
        let somewhere = Time::STATES.map(|time| if week.has(time) { all().fold(Area::default(), |held, cells| held.with(cells.cell(time, week))) } else { Area::default() });
        let held = all().fold(Area::default(), |held, cells| held.with(cells.holds(week)));
        let line = match mine {
            Cells::Line(line) => line,
            Cells::Said { .. } => Line::default(),
        };
        Here { line, mine, somewhere, unheld: Area::ALL.but(held) }
    }

    /// This device alone: no other counts.
    pub fn alone(mine: Line, week: Week) -> Here {
        Here::among(Cells::Line(mine), &[], week)
    }

    /// Whether this device narrows anything: the person said something of
    /// it, or it is read as a device that says its cells.
    pub fn narrows(&self) -> bool {
        self.mine.narrows()
    }

    /// The areas that come forward on this device in `time`: its cell
    /// (`Cells::cell`), and what the time brings of the areas no device is
    /// for (`unheld`).
    pub fn cell(&self, time: Time, week: Week) -> Area {
        self.mine.cell(time, week).with(self.unheld.both(brought(time, week)))
    }

    /// The areas that come forward in `time` on some device that counts
    /// (one of the five times; working and admin hours at once, both; no
    /// hours set, what some device is for).
    pub fn anywhere(&self, time: Time, week: Week) -> Area {
        let held = match time {
            Time::Any => Area::ALL.but(self.unheld),
            Time::Several(open) => {
                let at = |on: bool, time: Time| if on { slot(time).map_or(Area::default(), |s| self.somewhere[s]) } else { Area::default() };
                at(open.work, Time::Work).with(at(open.admin, Time::Admin))
            }
            one => slot(one).map_or(Area::default(), |s| self.somewhere[s]),
        };
        held.with(self.unheld.both(brought(time, week)))
    }

    /// Whether something for `area` comes forward on this device in `time`:
    /// its area meets this device's cell for that time. A device nobody said
    /// anything of narrows nothing, and a thing whose area is unsaid keeps
    /// the hours' answer: both are `in_view`.
    pub fn brings(&self, area: Area, time: Time, week: Week) -> bool {
        if area.is_empty() || !self.narrows() {
            return in_view(area, time, week);
        }
        area.meets(self.cell(time, week))
    }

    /// Whether this device is for something for `area` at some time of the
    /// week: its cells hold it, or no device that counts does. A device
    /// nobody said anything of, and a thing whose area is unsaid: yes.
    pub fn is_for(&self, area: Area, week: Week) -> bool {
        !self.narrows() || area.is_empty() || area.meets(self.mine.holds(week).with(self.unheld))
    }

    /// Whether this device, and not the hour, keeps something for `area`
    /// back in `time`: the hours bring it, this device does not.
    pub fn keeps_back(&self, area: Area, time: Time, week: Week) -> bool {
        in_view(area, time, week) && !self.brings(area, time, week)
    }
}

/// How tasks without an area of their own get one: by their categories and
/// projects. Work first; then what is yours to enjoy; then what is yours either
/// way (health: an errand, but never set aside on holidays); the rest is admin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskAreas {
    pub work_categories: Vec<String>,
    pub leisure_categories: Vec<String>,
    pub personal_categories: Vec<String>,
    /// The projects that give their tasks an area, by id, with that area
    /// (`Project::area_of`): those whose record says one, and those for a
    /// client, work's while unsaid (with the older box, work's and the box's).
    pub projects: Vec<(String, Area)>,
}

impl TaskAreas {
    /// The usual words: those in use (`words::current`; the packs built in,
    /// French and English, until a configuration is read).
    pub fn usual() -> TaskAreas {
        TaskAreas::usual_in(&crate::words::current().tasks)
    }

    /// The usual words of these lists: work, leisure, and what is yours outside work.
    pub fn usual_in(words: &crate::words::TaskWords) -> TaskAreas {
        TaskAreas {
            work_categories: words.work.clone(),
            leisure_categories: words.leisure.clone(),
            personal_categories: words.usual_personal.clone(),
            projects: Vec::new(),
        }
    }

    /// As your settings say: the categories of work and those that are yours
    /// (`[quiet]`, else the languages' own: `words.tasks`; leisure among them
    /// goes to leisure), and your projects' areas.
    pub fn of_config(config: &crate::config::Config, projects: &[crate::projects::Project]) -> TaskAreas {
        let words = crate::words::Words::of(config);
        let leisure: Vec<String> = words.tasks.leisure.iter().map(|c| crate::words::folded(c)).collect();
        let (leisure_categories, personal_categories): (Vec<String>, Vec<String>) = words.tasks.personal.iter().cloned().partition(|c| leisure.contains(&crate::words::folded(c)));
        TaskAreas {
            work_categories: words.tasks.work.clone(),
            leisure_categories,
            personal_categories,
            projects: TaskAreas::of_projects(projects),
        }
    }

    /// The projects that give their tasks an area, with it, by id
    /// (`Project::area_of`): an unsaid project without a client gives none.
    pub fn of_projects(projects: &[crate::projects::Project]) -> Vec<(String, Area)> {
        projects.iter().filter_map(|c| c.area_of().map(|area| (c.id.clone(), area))).collect()
    }

    /// A task's area: its own (`X-SIOUL-AREA`), else what its categories,
    /// projects and list say, together; else your own admin.
    pub fn of(&self, task: &crate::tasks::Task) -> Area {
        Area::parse(&task.area).unwrap_or_else(|| self.by_tags(task))
    }

    /// What a task's categories, projects and list say, its own area aside.
    pub fn by_tags(&self, task: &crate::tasks::Task) -> Area {
        let mut area = self.by_categories(task);
        // Each of its projects' areas, as the project says it (`Project::area_of`).
        for (_, of) in self.projects.iter().filter(|(id, _)| task.projects.contains(id)) {
            area = area.with(*of);
        }
        if area.is_empty() { Area::ADMIN } else { area }
    }

    /// What a task's categories and list alone say: nothing when they say nothing.
    fn by_categories(&self, task: &crate::tasks::Task) -> Area {
        let fold = |s: &str| crate::text::fold(s.trim()).into_iter().collect::<String>();
        let has = |list: &[String]| {
            let list: Vec<String> = list.iter().map(|c| fold(c)).collect();
            task.categories.iter().any(|c| list.contains(&fold(c)))
        };
        let mut area = Area::default();
        if has(&self.work_categories) || task.list_id.starts_with("local/github") {
            area = area.with(Area::WORK);
        }
        if has(&self.leisure_categories) {
            area = area.with(Area::LEISURE);
        }
        if has(&self.personal_categories) {
            area = area.with(Area::PERSONAL);
        }
        area
    }

    /// Where a task leans, for a sum by area that counts each task once (the
    /// Time page's comparison): its own area when it says one; else, among
    /// the areas it is in, those its categories alone say (a leisure task in
    /// a personal project leans to leisure); else every area it is in.
    pub fn leaning(&self, task: &crate::tasks::Task) -> Area {
        let whole = self.of(task);
        if Area::parse(&task.area).is_some() {
            return whole;
        }
        let said = self.by_categories(task).both(whole);
        if said.is_empty() { whole } else { said }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_leans_to_what_its_categories_say() {
        let areas = TaskAreas {
            work_categories: vec!["work".into()],
            leisure_categories: vec!["leisure".into()],
            personal_categories: vec!["health".into()],
            projects: vec![("choir".into(), Area::PERSONAL), ("lumen".into(), Area::WORK)],
        };
        let task = |categories: &[&str], projects: &[&str], own: &str| crate::tasks::Task {
            categories: categories.iter().map(|c| c.to_string()).collect(),
            projects: projects.iter().map(|p| p.to_string()).collect(),
            area: own.into(),
            ..Default::default()
        };
        // A leisure task in a personal project: in admin and leisure, leaning to leisure.
        let score = task(&["leisure"], &["choir"], "");
        assert_eq!(areas.of(&score), Area::PERSONAL);
        assert_eq!(areas.leaning(&score), Area::LEISURE);
        // Nothing but the project: every area it is in.
        assert_eq!(areas.leaning(&task(&[], &["choir"], "")), Area::PERSONAL);
        // A work category in a personal project leans to work.
        assert_eq!(areas.leaning(&task(&["work"], &["choir"], "")), Area::WORK);
        // Its own area, as said, whatever its categories.
        assert_eq!(areas.leaning(&task(&["leisure"], &["choir"], "admin")), Area::ADMIN);
        // Categories outside the areas it is in say nothing more: an own area wins.
        assert_eq!(areas.leaning(&task(&[], &["lumen"], "")), Area::WORK);
        assert_eq!(areas.leaning(&task(&[], &[], "")), Area::ADMIN);
    }

    #[test]
    fn what_comes_when() {
        let set = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        assert!(in_view(Area::WORK, Time::Work, set));
        assert!(!in_view(Area::WORK, Time::Leisure, set), "never in leisure");
        assert!(!in_view(Area::WORK, Time::Meals, set) && !in_view(Area::WORK, Time::Sleep, set), "nor while you eat or sleep");
        assert!(!in_view(Area::ADMIN, Time::Work, set), "admin has its own hours");
        assert!(in_view(Area::ADMIN, Time::Work, Week::default()), "until it has some");
        assert!(in_view(Area::ADMIN, Time::Admin, set) && !in_view(Area::ADMIN, Time::Leisure, set), "leisure is free");
        assert!(in_view(Area::LEISURE, Time::Sleep, set) && in_view(Area::LEISURE, Time::Meals, set), "a chat with friends");
        assert!(in_view(Area::LEISURE, Time::Leisure, set) && !in_view(Area::LEISURE, Time::Admin, set) && !in_view(Area::LEISURE, Time::Work, set));
        assert!(in_view(Area::PERSONAL, Time::Leisure, set) && in_view(Area::PERSONAL, Time::Admin, set) && !in_view(Area::PERSONAL, Time::Work, set));
        assert!(in_view(Area::MIXED, Time::Work, set) && in_view(Area::MIXED, Time::Admin, set) && !in_view(Area::MIXED, Time::Leisure, set));
        // Work and leisure together, a chat for both: in both their times.
        let both = Area::WORK.with(Area::LEISURE);
        assert!(in_view(both, Time::Work, set) && in_view(both, Time::Leisure, set) && !in_view(both, Time::Admin, set));
        let no_work = Week { admin_hours: true, ..Week::default() };
        assert!(in_view(Area::WORK, Time::Admin, no_work), "no work hours: work comes in admin's");
        assert!(!in_view(Area::WORK, Time::Leisure, no_work) && !in_view(Area::WORK, Time::Sleep, no_work), "leisure and sleep stay free");
        // Working and admin hours at once: both come.
        let both = Time::of(Area::MIXED);
        assert_eq!(both, Time::Several(Area::MIXED));
        assert!(in_view(Area::ADMIN, both, set) && in_view(Area::WORK, both, set) && !in_view(Area::LEISURE, both, set));
        assert_eq!((both.id(), Time::parse("work+admin")), ("work+admin", Some(both)));
        assert!(both.offices() && both.works());
        // Leisure has no hours: a window for it is none.
        assert_eq!(Time::of(Area::LEISURE), Time::Leisure);
        assert_eq!(Time::of(Area::PERSONAL), Time::Admin);
        assert!(in_view(Area::LEISURE, Time::Any, Week::default()) && in_view(Area::WORK, Time::Any, Week::default()));
        // The older words read; each time says its own.
        assert_eq!((Time::parse("rest"), Time::parse("personal"), Time::parse("repas"), Time::parse("Sommeil")), (Some(Time::Leisure), Some(Time::Leisure), Some(Time::Meals), Some(Time::Sleep)));
        assert_eq!(Time::STATES.map(Time::id), ["work", "admin", "leisure", "meals", "sleep"]);
        assert!(!Time::Meals.offices() && !Time::Sleep.offices() && !Time::Leisure.works());
        assert!(set.has(Time::Sleep) && !Week::default().has(Time::Meals) && Week::default().has(Time::Leisure));
    }

    /// Every area, every time (working and admin hours at once in each of
    /// their combinations too) and every week: what the tests below walk.
    fn every() -> (Vec<Area>, Vec<Time>, Vec<Week>) {
        let bits = |n: u8| Area { work: n & 1 != 0, admin: n & 2 != 0, leisure: n & 4 != 0 };
        let areas: Vec<Area> = (0..8).map(bits).collect();
        let times: Vec<Time> = Time::STATES.into_iter().chain([Time::Any]).chain(areas.iter().map(|open| Time::Several(*open))).collect();
        let weeks: Vec<Week> = (0..16u8).map(|n| Week { work_hours: n & 1 != 0, admin_hours: n & 2 != 0, meals: n & 4 != 0, sleep: n & 8 != 0 }).collect();
        (areas, times, weeks)
    }

    /// Every line a device may have with one cell changed at most, and a few with more.
    fn lines() -> Vec<Line> {
        let (areas, _, _) = every();
        let said: Vec<Option<Area>> = std::iter::once(None).chain(areas.iter().filter(|a| !a.is_empty()).map(|a| Some(*a))).collect();
        let mut lines = Vec::new();
        for area in &said {
            let line = Line { area: *area, at: [None; 5] };
            lines.push(line);
            for time in Time::STATES {
                for cell in &areas {
                    lines.push(line.with_cell(time, Some(*cell)));
                }
            }
            lines.push(line.with_cell(Time::Work, Some(Area::WORK.with(Area::LEISURE))).with_cell(Time::Sleep, Some(Area::default())).with_cell(Time::Meals, Some(Area::ADMIN)));
        }
        lines
    }

    /// `in_view` as it was before each time said what it brings, kept here
    /// word for word: the rule the devices build on must give its answers.
    fn as_before(area: Area, time: Time, week: Week) -> bool {
        match time {
            Time::Work => area.work || (!week.admin_hours && area.admin),
            Time::Admin => area.admin || (!week.work_hours && area.work),
            Time::Leisure | Time::Meals | Time::Sleep => area.leisure,
            Time::Any => true,
            Time::Several(open) => (open.work && as_before(area, Time::Work, week)) || (open.admin && as_before(area, Time::Admin, week)),
        }
    }

    /// What each time brings is the rule as it was: for every area, time and
    /// week, `in_view` answers as before, and but with no hours set at all,
    /// where everything comes, a thing with no area too, that answer is "the
    /// area meets what the time brings".
    #[test]
    fn what_each_time_brings_is_the_rule_as_it_was() {
        let (areas, times, weeks) = every();
        let mut walked = 0;
        for week in &weeks {
            for time in &times {
                for area in &areas {
                    assert_eq!(in_view(*area, *time, *week), as_before(*area, *time, *week), "{area:?} {time:?} {week:?}");
                    if *time == Time::Any {
                        assert_eq!(brought(*time, *week), Area::ALL);
                        assert!(in_view(*area, *time, *week), "no hours set: everything, {area:?}");
                    } else {
                        assert_eq!(as_before(*area, *time, *week), area.meets(brought(*time, *week)), "{area:?} {time:?} {week:?}");
                    }
                    walked += 1;
                }
            }
        }
        assert_eq!(walked, 8 * 14 * 16);
        // In words, once.
        let set = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        assert_eq!((brought(Time::Work, set), brought(Time::Work, Week::default())), (Area::WORK, Area::MIXED));
        assert_eq!((brought(Time::Admin, set), brought(Time::Admin, Week { admin_hours: true, ..Week::default() })), (Area::ADMIN, Area::MIXED));
        assert_eq!(Time::STATES.map(|t| brought(t, set))[2..], [Area::LEISURE; 3]);
        assert_eq!(brought(Time::Several(Area::LEISURE), set), Area::default(), "leisure has no hours to open");
    }

    /// A device's line among others' lines, in a week.
    fn among(mine: Line, others: &[Line], week: Week) -> Here {
        Lines::of(mine, others).here(week)
    }

    /// With nothing said of this device, nothing changes: whatever the other
    /// devices' lines say, every answer is the hours' own. A thing with no
    /// area keeps the hours' answer on every device, whatever its line says.
    #[test]
    fn a_device_nothing_is_said_of_narrows_nothing() {
        let (areas, times, weeks) = every();
        let lines = lines();
        let others: Vec<Vec<Line>> = vec![Vec::new(), vec![Line::for_area(Area::WORK)], vec![Line::for_area(Area::PERSONAL), Line::default()], lines.iter().copied().step_by(7).collect()];
        for week in &weeks {
            for time in &times {
                for area in &areas {
                    let hours = in_view(*area, *time, *week);
                    assert_eq!(Here::default().brings(*area, *time, *week), hours);
                    assert!(!Here::default().keeps_back(*area, *time, *week));
                    for others in &others {
                        assert_eq!(among(Line::default(), others, *week).brings(*area, *time, *week), hours, "{area:?} {time:?} {week:?} {others:?}");
                    }
                }
                for line in &lines {
                    let here = among(*line, &[Line::for_area(Area::LEISURE)], *week);
                    assert_eq!(here.brings(Area::default(), *time, *week), in_view(Area::default(), *time, *week), "a thing with no area: {line:?} {time:?}");
                }
            }
        }
    }

    /// The grid of the design: a computer for work, a phone for admin and
    /// leisure, a device nothing is said of; then cells the person changed.
    #[test]
    fn a_device_brings_what_its_cells_say() {
        let own = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        let lent = Week { admin_hours: false, ..own };
        let (computer, phone, unsaid) = (Line::for_area(Area::WORK), Line::for_area(Area::PERSONAL), Line::default());
        let cells = |line: Line, week: Week| Time::STATES.map(|t| line.cell(t, week).id());
        assert_eq!(cells(computer, own), ["work", "", "", "", ""]);
        assert_eq!(cells(phone, own), ["", "admin", "leisure", "leisure", "leisure"]);
        assert_eq!(cells(phone, lent), ["admin", "admin", "leisure", "leisure", "leisure"], "admin comes at work while it has no hours of its own");
        assert_eq!(cells(unsaid, own), ["work", "admin", "leisure", "leisure", "leisure"], "as the hours say");
        assert_eq!(Time::STATES.map(|t| unsaid.cell(t, lent)), Time::STATES.map(|t| brought(t, lent)));
        // Both at once: the cells of both; no hours at all: what the device is for.
        assert_eq!((phone.cell(Time::Several(Area::MIXED), own), computer.cell(Time::Several(Area::MIXED), own)), (Area::ADMIN, Area::WORK));
        assert_eq!((phone.cell(Time::Any, Week::default()), unsaid.cell(Time::Any, Week::default())), (Area::PERSONAL, Area::ALL));
        // On each device, a thing comes forward when its area meets the cell.
        let at = |mine: Line, others: &[Line]| among(mine, others, own);
        let (on_computer, on_phone) = (at(computer, &[phone]), at(phone, &[computer]));
        assert!(on_computer.brings(Area::WORK, Time::Work, own) && !on_phone.brings(Area::WORK, Time::Work, own));
        assert!(!on_computer.brings(Area::LEISURE, Time::Leisure, own) && on_phone.brings(Area::LEISURE, Time::Leisure, own));
        assert!(on_computer.keeps_back(Area::LEISURE, Time::Leisure, own), "the device is the reason");
        assert!(!on_computer.keeps_back(Area::LEISURE, Time::Work, own), "the hour is the reason");
        assert!(on_phone.brings(Area::PERSONAL, Time::Admin, own) && !on_computer.brings(Area::PERSONAL, Time::Admin, own));
        assert!(on_computer.brings(Area::MIXED, Time::Work, own) && !on_phone.brings(Area::MIXED, Time::Work, own) && on_phone.brings(Area::MIXED, Time::Work, lent), "a thing for two areas comes where one of them does");
        // A cell the person changed is the whole answer there: friends reach
        // the phone during work, which the time does not bring; nothing at meals.
        let friends = phone.with_cell(Time::Work, Some(Area::WORK.with(Area::LEISURE))).with_cell(Time::Meals, Some(Area::default()));
        let on_friends = at(friends, &[computer]);
        assert_eq!(cells(friends, own), ["work+leisure", "admin", "leisure", "", "leisure"]);
        assert!(on_friends.brings(Area::LEISURE, Time::Work, own) && !in_view(Area::LEISURE, Time::Work, own));
        assert!(on_friends.brings(Area::WORK, Time::Work, own), "and work, which the phone is not for otherwise");
        assert!(!on_friends.brings(Area::LEISURE, Time::Meals, own) && on_friends.brings(Area::LEISURE, Time::Sleep, own));
        assert_eq!(friends.with_cell(Time::Work, None), phone.with_cell(Time::Meals, Some(Area::default())), "as its areas say again");
        // Both hours open: the changed cell of work and admin's own, together; no hours: the cells are not read.
        assert_eq!(friends.cell(Time::Several(Area::MIXED), own), Area::ALL);
        assert_eq!(friends.cell(Time::Any, Week::default()), Area::PERSONAL);
        // As written in the settings, and read back.
        assert_eq!((Line::cell_id(Area::default()), Line::cell_id(Area::PERSONAL)), ("none".to_string(), "admin+leisure".to_string()));
        assert_eq!((Line::read_cell("none"), Line::read_cell(" Rien "), Line::read_cell("work+leisure"), Line::read_cell("?")), (Some(Area::default()), Some(Area::default()), Some(Area::WORK.with(Area::LEISURE)), None));
        assert_eq!(Line::for_area(Area::default()), Line::default(), "nothing ticked: everything");
        assert!(Line::default().says_nothing() && !phone.says_nothing() && !Line::default().with_cell(Time::Sleep, Some(Area::default())).says_nothing());
    }

    /// An area no device that counts is for, at any time, is narrowed by no
    /// device: the phone lost or forgotten, leisure comes on the work
    /// computer as the hours say, and goes back to the phone once it counts
    /// again. A device alone narrows nothing; a cell the person changed
    /// still brings more.
    #[test]
    fn an_area_no_device_is_for_is_narrowed_by_none() {
        let (areas, times, weeks) = every();
        let (computer, phone) = (Line::for_area(Area::WORK), Line::for_area(Area::PERSONAL));
        for week in &weeks {
            let alone = Here::alone(computer, *week);
            assert_eq!(alone.unheld, Area::PERSONAL);
            let with_phone = among(computer, &[phone], *week);
            assert_eq!(with_phone.unheld, Area::default());
            for time in &times {
                for area in &areas {
                    assert_eq!(alone.brings(*area, *time, *week), in_view(*area, *time, *week), "alone: {area:?} {time:?} {week:?}");
                    assert!(!with_phone.brings(*area, *time, *week) || area.work || *time == Time::Any && area.is_empty(), "{area:?} {time:?} {week:?}");
                }
            }
        }
        let own = Week { work_hours: true, admin_hours: true, meals: false, sleep: true };
        // A phone for leisure alone beside it: admin is for no device, and comes as the hours say on both.
        let leisure_phone = Line::for_area(Area::LEISURE);
        let (on_computer, on_phone) = (among(computer, &[leisure_phone], own), among(leisure_phone, &[computer], own));
        assert_eq!((on_computer.unheld, on_phone.unheld), (Area::ADMIN, Area::ADMIN));
        assert!(on_computer.brings(Area::ADMIN, Time::Admin, own) && on_phone.brings(Area::ADMIN, Time::Admin, own) && !on_computer.brings(Area::ADMIN, Time::Leisure, own));
        assert!(!on_computer.brings(Area::LEISURE, Time::Leisure, own) && on_phone.brings(Area::LEISURE, Time::Leisure, own));
        assert_eq!(on_computer.anywhere(Time::Admin, own), Area::ADMIN);
        assert_eq!((on_computer.anywhere(Time::Leisure, own), on_computer.anywhere(Time::Work, own), on_computer.anywhere(Time::Several(Area::MIXED), own)), (Area::LEISURE, Area::WORK, Area::MIXED));
        // A cell that holds an area counts as a device being for it: leisure
        // during work on the computer alone makes it for leisure then, and only then.
        let both = computer.with_cell(Time::Work, Some(Area::WORK.with(Area::LEISURE)));
        let here = Here::alone(both, own);
        assert_eq!(here.unheld, Area::ADMIN);
        assert!(here.brings(Area::LEISURE, Time::Work, own) && !here.brings(Area::LEISURE, Time::Leisure, own));
        // A device nothing is said of beside it: everything comes there, so nothing is unheld.
        assert_eq!(among(computer, &[Line::default()], own).unheld, Area::default());
        // No hours at all: what each device is for counts.
        assert_eq!(among(computer.with_cell(Time::Work, Some(Area::default())), &[], Week::default()).unheld, Area::PERSONAL);
    }

    /// A cell in force only at a time the week lacks holds nothing (review
    /// of phase 2, M2): admin without hours of its own, a phone whose work
    /// cell was changed to leisure is for admin at no time the week has, so
    /// admin is for no device and comes as the hours say on each; the times
    /// lent count (admin in work's cell while it has no hours, and work in
    /// admin's while work has none); meals unset, a leisure kept to meals is
    /// held nowhere.
    #[test]
    fn only_the_times_the_week_has_hold_an_area() {
        let (computer, phone) = (Line::for_area(Area::WORK), Line::for_area(Area::PERSONAL));
        let lent = Week { work_hours: true, admin_hours: false, meals: true, sleep: true };
        let friends = phone.with_cell(Time::Work, Some(Area::LEISURE));
        assert_eq!(friends.holds(lent), Area::LEISURE, "its admin cell is in force at no time of this week");
        let (on_computer, on_phone) = (among(computer, &[friends], lent), among(friends, &[computer], lent));
        assert_eq!((on_computer.unheld, on_phone.unheld), (Area::ADMIN, Area::ADMIN));
        assert!(on_computer.brings(Area::ADMIN, Time::Work, lent) && on_phone.brings(Area::ADMIN, Time::Work, lent), "comes as the hours say on both");
        // Lent: admin in work's cell; and work in admin's while work has no hours.
        assert_eq!(phone.holds(lent), Area::PERSONAL);
        assert_eq!(computer.holds(Week { work_hours: false, admin_hours: true, ..lent }), Area::WORK);
        // Meals unset: a leisure that comes only at meals comes at no time.
        let at_meals = Line::for_area(Area::LEISURE).with_cell(Time::Leisure, Some(Area::default())).with_cell(Time::Sleep, Some(Area::default()));
        let no_meals = Week { meals: false, ..lent };
        assert_eq!((at_meals.holds(lent), at_meals.holds(no_meals)), (Area::LEISURE, Area::default()));
        assert_eq!(among(computer, &[at_meals], no_meals).unheld, Area::PERSONAL);
    }

    /// What a device says of itself in its entry (`Cells::words`): the
    /// cells of the times its week has, and what it is for; nothing for a
    /// line nothing is said in. Read back, the same answers as its line,
    /// whatever week reads them; a time its week lacks brings nothing there;
    /// unknown words passed over.
    #[test]
    fn a_device_says_its_cells_and_is_read_by_them() {
        let own = Week { work_hours: true, admin_hours: true, meals: false, sleep: true };
        let phone = Line::for_area(Area::PERSONAL).with_cell(Time::Work, Some(Area::LEISURE));
        let words = Cells::Line(phone).words(own);
        assert_eq!(words.iter().map(|(t, c)| format!("{t}={c}")).collect::<Vec<_>>(), ["admin=admin", "any=admin+leisure", "leisure=leisure", "sleep=leisure", "work=leisure"]);
        assert_eq!(Cells::key(&words), "admin=admin;any=admin+leisure;leisure=leisure;sleep=leisure;work=leisure");
        let said = Cells::read(&words);
        assert_eq!(said, Cells::of(phone, own));
        for time in [Time::Work, Time::Admin, Time::Leisure, Time::Sleep, Time::Any, Time::Several(Area::MIXED)] {
            assert_eq!(said.cell(time, own), phone.cell(time, own), "{time:?}");
        }
        assert_eq!((said.cell(Time::Meals, own), said.holds(own)), (Area::default(), phone.holds(own)));
        // Nothing said: nothing in the entry, read as the hours.
        assert!(Cells::Line(Line::default()).words(own).is_empty() && Cells::key(&std::collections::BTreeMap::new()).is_empty());
        assert_eq!(Cells::read(&std::collections::BTreeMap::new()), Cells::default());
        assert!(!Cells::default().narrows() && said.narrows());
        // Words this Sioul does not know: passed over.
        let odd: std::collections::BTreeMap<String, String> = [("work", "often"), ("later", "work"), ("leisure", "leisure")].into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        assert_eq!(Cells::read(&odd), Cells::Said { at: [None, None, Some(Area::LEISURE), None, None], any: Area::ALL });
        // Read as the device it is, among others: the same as its own line there.
        let computer = Line::for_area(Area::WORK);
        for time in [Time::Work, Time::Admin, Time::Leisure] {
            for area in [Area::WORK, Area::ADMIN, Area::LEISURE] {
                let (from_line, from_said) = (among(phone, &[computer], own), Lines { mine: said, key: None, others: vec![Other { cells: Cells::Line(computer), acts_on: None }] }.here(own));
                assert_eq!(from_line.brings(area, time, own), from_said.brings(area, time, own), "{area:?} {time:?}");
            }
        }
    }

    /// Another device holds a thing for later only once it says it acts on
    /// the cells of the device read (review of phase 2, H1, M3): a phone
    /// that still acts on the computer's line before it changed, or on no
    /// word of it, is left out, and what only it holds comes on the computer
    /// as the hours say; once it acts on the computer's own cells, it holds
    /// leisure. Read as the keeper (`key` said), the key is the one its
    /// entry says, even with words this Sioul does not know.
    #[test]
    fn another_device_holds_a_thing_only_once_it_acts_on_these_cells() {
        let week = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        let (computer, phone) = (Line::for_area(Area::WORK), Line::for_area(Area::PERSONAL));
        let key = Cells::key(&Cells::Line(computer).words(week));
        let with = |acts_on: Option<&str>| Lines { mine: Cells::Line(computer), key: None, others: vec![Other { cells: Cells::Line(phone), acts_on: acts_on.map(str::to_string) }] }.here(week);
        assert_eq!(with(Some(&key)).unheld, Area::default());
        for stale in [Some(""), Some("any=all"), Some("work=work;any=work")] {
            let here = with(stale);
            assert_eq!(here.unheld, Area::PERSONAL, "{stale:?}");
            assert!(here.brings(Area::LEISURE, Time::Leisure, week), "{stale:?}: told on the computer");
        }
        assert_eq!(with(None).unheld, Area::default(), "not asked: it counts as it is");
        // Read as the keeper: its entry's own key.
        let said: std::collections::BTreeMap<String, String> = [("work", "work"), ("any", "work"), ("later", "x")].into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        let as_keeper = |acts_on: &str| Lines { mine: Cells::read(&said), key: Some(Cells::key(&said)), others: vec![Other { cells: Cells::Line(phone), acts_on: Some(acts_on.to_string()) }] }.here(week);
        assert_eq!((as_keeper(&Cells::key(&said)).unheld, as_keeper("any=work;work=work").unheld), (Area::default(), Area::PERSONAL));
    }

    /// A source file's code: what comes before its tests, its comment lines left out.
    fn code_of(path: &std::path::Path) -> String {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let code = text.split("\n#[cfg(test)]\nmod ").next().unwrap_or_default();
        code.lines().filter(|line| !line.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
    }

    /// Every source of every crate, by its path from `crates/`.
    fn sources() -> Vec<(String, String)> {
        fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, String)>) {
            let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    walk(&path, root, out);
                } else if path.extension().is_some_and(|x| x == "rs") && path.file_name().is_some_and(|n| n != "tests.rs") {
                    out.push((path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/"), code_of(&path)));
                }
            }
        }
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut out = Vec::new();
        for one in ["sioul-core", "sioul-sync", "sioul-learn", "sioul-cli", "sioul-app"] {
            walk(&crates.join(one).join("src"), &crates, &mut out);
        }
        assert!(out.len() > 200, "the sources were not found: {}", out.len());
        out
    }

    /// Every caller of the rule, sorted (docs/areas.md, "What each device is
    /// for"): a caller about what this device shows or tells now asks the
    /// device (`Here::brings`, `keeps_back`, `anywhere`, `is_for`,
    /// `elsewhere`, `for_another`, a cell read, through `Mode` or `Now`); one about the
    /// person's own week asks the hours (`in_view`); and each time made
    /// (`quiet::mode`, `Situation::now`, `reach::Clock::load`) is given this
    /// device's lines (`.on(..)`) or is named here as one read by the hours
    /// alone (review of phase 2, L3). Each file that does either is named
    /// with its counts and why; a new call fails until it is sorted, and the
    /// failure gives the line to add or change. With nothing said of a
    /// device, the two rules answer alike
    /// (`a_device_nothing_is_said_of_narrows_nothing`), so that nothing a
    /// caller shows or tells changes until the person says what a device is for.
    #[test]
    fn every_caller_of_the_rule_is_sorted() {
        // (file, calls of the hours' rule, calls of the device's, times made, of which given the lines, why)
        let sorted: [(&str, usize, usize, usize, usize, &str); 36] = [
            ("sioul-core/src/areas.rs", 2, 9, 0, 0, "the rule itself: `Here::brings` falls back on the hours with nothing said of the device, or for a thing with no area; `keeps_back` asks both to say which is the reason; `cell`, `anywhere`, `is_for` read the cells"),
            ("sioul-core/src/plan.rs", 3, 0, 0, 0, "the plan gives a task room in the hours of its area: the person's week, the same on every device"),
            ("sioul-core/src/attention.rs", 4, 12, 0, 0, "hours: this row and this address never meet in your week (`meets_at`); what you set or asked for, and what Reachable lets through, held by the hour alone (`own`, `site`); the hour's answer beside the device's, to say which holds a message (`letter`). Device: `Now::brings`, `keeps_back`, step 8 for dates, mail, other apps' messages and sites; `meets_somewhere` and a task reminder's devices (`is_for`)"),
            ("sioul-core/src/quiet.rs", 0, 8, 0, 0, "`Mode::brings`, `keeps_back`, `elsewhere` and `for_another`; what the task pages keep now (`QuietTasks`, the office's cell)"),
            ("sioul-core/src/areafilter.rs", 0, 2, 0, 0, "the filter's ticks, for Mail, Budgets, and Projects and Time (one filter for both); the status line's sentence (`elsewhere`)"),
            ("sioul-core/src/mailbox.rs", 0, 2, 0, 0, "an address rests, and says whether the hour or this device is the reason"),
            ("sioul-core/src/view.rs", 0, 1, 0, 0, "the Mail page's accounts: an address rests"),
            ("sioul-core/src/appnotes.rs", 1, 2, 0, 0, "an app's notifications with a For, on a phone, and why one waits; while Reachable, the hours alone (`area_fits`)"),
            ("sioul-core/src/mailnote.rs", 0, 1, 0, 0, "the moment's words carry the cell read now, so that a cell changed begins a time"),
            ("sioul-core/src/invoice/samples.rs", 0, 1, 0, 0, "not the rule: an invoice's own `elsewhere`"),
            ("sioul-core/src/agendas.rs", 0, 0, 1, 0, "the agenda's filter by the hours of its agendas (`agendas::Hours`): the person's week"),
            ("sioul-core/src/notice.rs", 0, 0, 1, 0, "the moment a notice is decided in, read from the files by a process that tells nothing of an area (`moment_from_files`)"),
            ("sioul-core/src/reach.rs", 0, 0, 2, 1, "a clock's times for other apps on this device (`Clock::on`); the calls' and texts' moments by the hours alone (calls and texts keep their own matrix)"),
            ("sioul-core/src/reminders.rs", 0, 0, 2, 0, "which tasks get a reminder at all, and when (the hours' `work` flag); their devices are chosen by `Holds::on` and each task's area"),
            ("sioul-sync/src/device_grid.rs", 0, 2, 0, 0, "the grid: each cell of a line, and what comes here now"),
            ("sioul-sync/src/fetch.rs", 0, 1, 0, 0, "not the rule: another `brings`, which mail a fetch brings (`fetch::Control`)"),
            ("sioul-cli/src/devices.rs", 0, 1, 2, 1, "`sioul devices`: the grid on this device; `devices at` says a cell in the week's hours"),
            ("sioul-cli/src/tasks.rs", 0, 0, 1, 1, "`sioul tasks`: the lists on this device"),
            ("sioul-cli/src/accounts.rs", 0, 0, 1, 0, "the codes' watch of the command line: no area"),
            ("sioul-cli/src/mcp/read.rs", 0, 0, 1, 0, "agents read the hours alone"),
            ("sioul-cli/src/mcp/records.rs", 0, 0, 1, 0, "agents read the hours alone"),
            ("sioul-app/src/hours.rs", 0, 0, 1, 0, "`hours_mode_at`, by the hours alone: the doses, Health, calls and texts; `mode_at` gives it this device's lines"),
            ("sioul-app/src/work.rs", 0, 0, 1, 1, "the task pages' situation on this device: what Now and What comes keep; the full views and today's layout read none"),
            ("sioul-app/src/homecard.rs", 0, 0, 2, 2, "the home card's next step and its frames on this device"),
            ("sioul-app/src/appnotes.rs", 0, 0, 1, 1, "another app's notifications on a phone, on this device (`Clock::on`)"),
            ("sioul-app/src/agenda.rs", 0, 0, 1, 0, "the agenda's filter by time in What comes (the Day, the Week and the Month by hand alone): the hours of your agendas, the person's week, on every device alike"),
            ("sioul-app/src/calls.rs", 0, 0, 1, 0, "calls keep their own matrix: never a device's"),
            ("sioul-app/src/phonemsgs.rs", 0, 0, 1, 0, "the phone's texts keep their own matrix: never a device's"),
            ("sioul-app/src/everywhere.rs", 0, 0, 1, 0, "do-not-disturb's look at the hours: no area"),
            ("sioul-app/src/persons.rs", 0, 0, 1, 0, "whether offices are open now, to call one: the person's week"),
            ("sioul-app/src/sites.rs", 1, 3, 0, 0, "the sites listed, their news on the Porch, the gathered notification; while Reachable, gathered by the hours alone"),
            ("sioul-app/src/notices.rs", 0, 5, 0, 0, "the Porch: sites, money, the books' items, the agenda's conflicts, invoices numbered twice"),
            ("sioul-app/src/pim.rs", 0, 1, 0, 0, "the agenda's conflicts said now"),
            ("sioul-app/src/books.rs", 0, 1, 0, 0, "the books' thresholds wait while the hours bring work and this computer is not for it"),
            ("sioul-app/src/dispatch.rs", 0, 0, 0, 0, "the devices that count: no rule asked"),
            ("sioul-app/src/mail.rs", 0, 1, 0, 0, "a message opened through a door on a device not for its address, read here only, and \"Done for now\" kept to what this device is for (`read_here_only`, `Mode::for_another`)"),
        ];
        // A call by its name: not the end of a longer name (`pin_view(`).
        let calls = |code: &str, name: &str| code.match_indices(name).filter(|(at, _)| !code[..*at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_')).count();
        let device_calls = [".brings(", ".keeps_back(", ".anywhere(", ".is_for(", ".elsewhere(", ".for_another(", "here.cell(", "line.cell(", "mine.cell(", "cells.cell(", "said.cell("];
        // A time made, and whether its statement gives it lines: up to its `;` or the closing brace of its block.
        let statement = |code: &str, at: usize| -> String {
            let rest = &code[at..];
            let semicolon = rest.find(';').unwrap_or(rest.len());
            let close = rest.match_indices('\n').map(|(i, _)| i).find(|i| rest[i + 1..].trim_start_matches(' ').starts_with('}')).unwrap_or(rest.len());
            rest[..semicolon.min(close)].to_string()
        };
        let mut found: Vec<(String, usize, usize, usize, usize)> = Vec::new();
        for (file, code) in sources() {
            // The rule's own definition is no call of it.
            let hours = calls(&code, "in_view(") - code.matches("fn in_view(").count();
            let device = device_calls.iter().map(|name| code.matches(name).count()).sum::<usize>();
            let made: Vec<String> = ["quiet::mode(", "Situation::now(", "Clock::load("].iter().flat_map(|name| code.match_indices(name).map(|(at, _)| statement(&code, at)).collect::<Vec<_>>()).collect();
            let on = made.iter().filter(|s| s.contains(".on(")).count();
            if hours + device + made.len() > 0 {
                found.push((file, hours, device, made.len(), on));
            }
        }
        for (file, hours, device, made, on) in &found {
            let named = sorted.iter().find(|(name, ..)| name == file);
            assert_eq!(
                named.map(|(_, h, d, m, o, _)| (*h, *d, *m, *o)),
                Some((*hours, *device, *made, *on)),
                "{file} calls the hours' rule {hours} times and the device's {device} times, and makes {made} times of which {on} are given this device's lines: sort each (the device's for what this device shows or tells now, the hours' for the person's own week; a time with `.on(lines)` when it shows or tells, without when it is the person's week) and name it in this test as (\"{file}\", {hours}, {device}, {made}, {on}, why)"
            );
        }
        for (file, hours, device, made, ..) in sorted.iter().filter(|(_, hours, device, made, ..)| hours + device + made > 0) {
            assert!(found.iter().any(|(name, ..)| name == file), "{file} is named with {hours}, {device} and {made} calls and holds none");
        }
    }

    /// Reachable comes before every device step (review of phase 2, N5):
    /// each function of the pipeline and of other apps' notifications that
    /// asks a device (`.brings(`, `.keeps_back(`, `.is_for(`), and the
    /// window's two that tell by the keeper (the sites' gathered news, the
    /// books' thresholds), reads Reachable; the wrappers that only pass the
    /// question on are left aside. A new device step without it fails.
    #[test]
    fn reachable_comes_before_every_device_step() {
        let sources = sources();
        let functions = |code: &str| -> Vec<(String, String)> {
            let mut out: Vec<(String, String)> = Vec::new();
            for line in code.lines() {
                let trimmed = line.trim_start();
                if let Some(rest) = ["pub(crate) fn ", "pub fn ", "fn "].iter().find_map(|head| trimmed.strip_prefix(head)) {
                    out.push((rest.split(['(', '<']).next().unwrap_or_default().to_string(), String::new()));
                }
                if let Some((_, body)) = out.last_mut() {
                    body.push_str(line);
                    body.push('\n');
                }
            }
            out
        };
        let asks = |body: &str| [".brings(", ".keeps_back(", ".is_for("].iter().any(|step| body.contains(step));
        let mut checked = 0;
        for (file, only) in [("sioul-core/src/attention.rs", None), ("sioul-core/src/appnotes.rs", None), ("sioul-app/src/sites.rs", Some("gather_tick")), ("sioul-app/src/books.rs", Some("tick"))] {
            let code = &sources.iter().find(|(name, _)| name == file).unwrap_or_else(|| panic!("{file} is gone")).1;
            for (name, body) in functions(code) {
                if ["brings", "keeps_back", "is_for"].contains(&name.as_str()) || only.is_some_and(|only| only != name) || !asks(&body) {
                    continue;
                }
                assert!(body.contains("reachable"), "{file}: {name} asks a device without reading Reachable first");
                checked += 1;
            }
        }
        assert!(checked >= 8, "{checked} functions checked");
    }

    /// The doses and everything of Health never read what a device is for:
    /// neither the rule, nor a device's line, nor the entry's word that its
    /// Sioul follows devices (`areas`). The health lease and what the doses
    /// read of an entry (`Entry::said`) stay as they were.
    #[test]
    fn the_health_path_reads_no_device_cell() {
        let health = ["sioul-core/src/health.rs", "sioul-core/src/doses.rs", "sioul-core/src/dose_backs.rs", "sioul-core/src/dose_reminders.rs", "sioul-core/src/needs.rs", "sioul-app/src/health.rs", "sioul-app/src/alarms.rs", "sioul-app/src/wake.rs"];
        let never = [".brings(", ".keeps_back(", "in_view(", "areas::Line", "areas::Here", "areas::Lines", "Lines {", "dispatch::", "Dispatch", "keeper_for(", "told_by(", ".areas", ".elsewhere(", "areas::Cells", ".anywhere(", ".is_for(", "as_keeper(", "acts_on(", ".on(&"];
        let sources = sources();
        for file in health {
            let code = &sources.iter().find(|(name, _)| name == file).unwrap_or_else(|| panic!("{file} is not among the sources")).1;
            assert!(code.len() > 1000, "{file} reads empty");
            for word in never {
                assert!(!code.contains(word), "{file} holds {word}: the doses and Health never read what a device is for");
            }
        }
        // The window's helpers the dose path calls (`crate::hours::…`, `crate::tell::…`), and every
        // helper of those two files they call in turn, read the hours alone (`hours_mode_at`): never
        // this device's lines (review of phase 2, L1 and N6).
        let files = [("hours", "sioul-app/src/hours.rs"), ("tell", "sioul-app/src/tell.rs")];
        let code_of_file = |file: &str| sources.iter().find(|(name, _)| name == file).unwrap_or_else(|| panic!("{file} is gone")).1.clone();
        let body = |file: &str, name: &str| -> Option<String> {
            let code = code_of_file(file);
            let start = code.match_indices(&format!("fn {name}(")).map(|(at, _)| at).find(|at| !code[..*at].ends_with('_'))?;
            let rest = &code[start..];
            Some(rest[..rest.find("\n}\n").map_or(rest.len(), |end| end + 2)].to_string())
        };
        // A call by its whole name: `mode_at(` is not called by `hours_mode_at(`.
        let calls = |code: &str, name: &str| code.match_indices(&format!("{name}(")).any(|(at, _)| !code[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_'));
        let defined = |file: &str| -> Vec<String> {
            let code = code_of_file(file);
            code.match_indices("fn ").filter_map(|(at, _)| code[at + 3..].split('(').next().filter(|n| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_')).map(str::to_string)).collect()
        };
        // The calls a piece of code makes into those files, and into its own.
        let called = |code: &str, own: Option<&str>| -> Vec<(String, String)> {
            let mut out = Vec::new();
            for (module, file) in files {
                let prefix = format!("crate::{module}::");
                out.extend(code.match_indices(&prefix).map(|(at, _)| (file.to_string(), code[at + prefix.len()..].split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or_default().to_string())));
            }
            if let Some(own) = own {
                out.extend(defined(own).into_iter().filter(|name| calls(code, name)).map(|name| (own.to_string(), name)));
            }
            out
        };
        let mut to_read: Vec<(String, String)> = health.iter().flat_map(|file| called(&code_of_file(file), None)).collect();
        assert!(to_read.iter().any(|(_, name)| name == "doses_silent") && to_read.iter().any(|(file, _)| file.ends_with("tell.rs")), "the dose path's helpers were not found: {to_read:?}");
        let mut reached: Vec<(String, String)> = Vec::new();
        while let Some((file, name)) = to_read.pop() {
            if reached.contains(&(file.clone(), name.clone())) {
                continue;
            }
            let Some(code) = body(&file, &name) else { continue };
            for word in ["dispatch::", ".on(&", "attention_here(", "with_layers("] {
                assert!(!code.contains(word), "{file}: {name}, on the dose path, holds {word}: the doses never read what a device is for");
            }
            for device_aware in ["mode_at", "mode_now"] {
                assert!(!calls(&code, device_aware), "{file}: {name}, on the dose path, calls {device_aware}");
            }
            to_read.extend(called(&code, Some(&file)).into_iter().filter(|call| *call != (file.clone(), name.clone())));
            reached.push((file, name));
        }
        assert!(reached.iter().any(|(_, name)| name == "hours_mode_at") && reached.iter().any(|(file, name)| file.ends_with("tell.rs") && name == "moment"), "{reached:?}");
        // What the doses read of an entry (`Entry::said`) names no area.
        let devices = &sources.iter().find(|(name, _)| name == "sioul-sync/src/devices.rs").expect("the entries").1;
        let said = devices.split("pub fn said(").nth(1).and_then(|rest| rest.split("\n    }\n").next()).expect("Entry::said");
        assert!(!said.contains("areas"), "what the doses read of an entry names no area:\n{said}");
    }

    #[test]
    fn areas_read() {
        assert_eq!(Area::parse(" Démarches "), Some(Area::ADMIN));
        assert_eq!(Area::parse("work"), Some(Area::WORK));
        assert_eq!(Area::parse("personal"), Some(Area::PERSONAL), "the older word");
        assert_eq!(Area::parse("admin+leisure"), Some(Area::PERSONAL));
        assert_eq!(Area::parse("LEISURE,WORK"), Some(Area::WORK.with(Area::LEISURE)));
        assert_eq!(Area::parse(""), None);
        assert_eq!(Area::MIXED.id(), "work+admin");
        let areas = TaskAreas::usual();
        let task = |categories: &[&str], area: &str| crate::tasks::Task { categories: categories.iter().map(|c| c.to_string()).collect(), area: area.into(), ..crate::tasks::Task::default() };
        assert_eq!(areas.of(&task(&["Famille"], "")), Area::LEISURE);
        assert_eq!(areas.of(&task(&["client"], "")), Area::WORK);
        assert_eq!(areas.of(&task(&["santé"], "")), Area::PERSONAL);
        assert_eq!(areas.of(&task(&["admin"], "")), Area::ADMIN);
        assert_eq!(areas.of(&task(&[], "")), Area::ADMIN, "the rest is admin");
        assert_eq!(areas.of(&task(&["client", "famille"], "")), Area::WORK.with(Area::LEISURE), "together");
        assert_eq!(areas.of(&task(&["client"], "leisure")), Area::LEISURE, "its own first");
    }
}

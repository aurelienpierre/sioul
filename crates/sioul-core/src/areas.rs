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

    /// Every area the device is for at some time: its five cells together,
    /// and, while the week sets no hours at all, what it is for.
    pub fn holds(self, week: Week) -> Area {
        let cells = Time::STATES.into_iter().fold(Area::default(), |all, time| all.with(self.cell(time, week)));
        if week.work_hours || week.admin_hours { cells } else { cells.with(self.is_for()) }
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

/// This device among the person's devices (docs/areas.md, "What each device
/// is for"): its line, and what the lines of every device that counts say
/// together. `Here::default()` is a device nobody said anything of, alone:
/// it narrows nothing, and every answer is the hours' own (`in_view`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Here {
    /// What the person said of this device.
    pub line: Line,
    /// For each of the five times (`Time::STATES`), the areas that come
    /// forward then on some device that counts, this one included.
    pub somewhere: [Area; 5],
    /// The areas no device that counts is for, at any time: no device
    /// narrows them, so that a phone lost or forgotten never makes what was
    /// for it vanish from every device.
    pub unheld: Area,
}

/// The lines of the devices that count, as this device knows them: its own,
/// and the others' whose Sioul follows what devices are for (an older one
/// is left out: `Here::among`). `Lines::default()`: nothing said of any device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lines {
    pub mine: Line,
    pub others: Vec<Line>,
}

impl Lines {
    /// This device among them, in a week.
    pub fn here(&self, week: Week) -> Here {
        Here::among(self.mine, &self.others, week)
    }

    /// Nothing said of this device: it narrows nothing.
    pub fn say_nothing(&self) -> bool {
        self.mine.says_nothing()
    }
}

impl Here {
    /// This device (`mine`) among the `others` that count, in a week. A
    /// device whose Sioul does not follow what devices are for yet is left
    /// out: it tells by the rule before, so nothing it could be for is
    /// counted as held, and an older Sioul never leaves a thing for nobody.
    pub fn among(mine: Line, others: &[Line], week: Week) -> Here {
        let all = || std::iter::once(mine).chain(others.iter().copied());
        let somewhere = Time::STATES.map(|time| all().fold(Area::default(), |held, line| held.with(line.cell(time, week))));
        let held = all().fold(Area::default(), |held, line| held.with(line.holds(week)));
        Here { line: mine, somewhere, unheld: Area::ALL.but(held) }
    }

    /// This device alone: no other counts.
    pub fn alone(mine: Line, week: Week) -> Here {
        Here::among(mine, &[], week)
    }

    /// The same devices, read as another of them (`line`): what comes
    /// forward there.
    pub fn as_device(self, line: Line) -> Here {
        Here { line, ..self }
    }

    /// Whether this device narrows anything: the person said something of it.
    pub fn narrows(&self) -> bool {
        !self.line.says_nothing()
    }

    /// The areas that come forward on this device in `time`: its cell
    /// (`Line::cell`), and what the time brings of the areas no device is
    /// for (`unheld`).
    pub fn cell(&self, time: Time, week: Week) -> Area {
        self.line.cell(time, week).with(self.unheld.both(brought(time, week)))
    }

    /// The areas that come forward in `time` on some device that counts
    /// (one of the five times; working and admin hours at once, both).
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
    /// Projects for a client, and those marked work.
    pub work_projects: Vec<String>,
    /// Projects marked personal (family, friends, leisure, health).
    pub personal_projects: Vec<String>,
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
            work_projects: Vec::new(),
            personal_projects: Vec::new(),
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
            work_projects: projects.iter().filter(|c| c.client.is_some() || c.area.as_deref() == Some("work")).map(|c| c.id.clone()).collect(),
            personal_projects: projects.iter().filter(|c| c.area.as_deref() == Some("personal")).map(|c| c.id.clone()).collect(),
        }
    }

    /// A task's area: its own (`X-SIOUL-AREA`), else what its categories,
    /// projects and list say, together; else your own admin.
    pub fn of(&self, task: &crate::tasks::Task) -> Area {
        Area::parse(&task.area).unwrap_or_else(|| self.by_tags(task))
    }

    /// What a task's categories, projects and list say, its own area aside.
    pub fn by_tags(&self, task: &crate::tasks::Task) -> Area {
        let fold = |s: &str| crate::text::fold(s.trim()).into_iter().collect::<String>();
        let has = |list: &[String]| {
            let list: Vec<String> = list.iter().map(|c| fold(c)).collect();
            task.categories.iter().any(|c| list.contains(&fold(c)))
        };
        let mut area = Area::default();
        if has(&self.work_categories) || task.projects.iter().any(|c| self.work_projects.contains(c)) || task.list_id.starts_with("local/github") {
            area = area.with(Area::WORK);
        }
        if has(&self.leisure_categories) {
            area = area.with(Area::LEISURE);
        }
        if has(&self.personal_categories) || task.projects.iter().any(|c| self.personal_projects.contains(c)) {
            area = area.with(Area::PERSONAL);
        }
        if area.is_empty() { Area::ADMIN } else { area }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                        assert_eq!(Here::among(Line::default(), others, *week).brings(*area, *time, *week), hours, "{area:?} {time:?} {week:?} {others:?}");
                    }
                }
                for line in &lines {
                    let here = Here::among(*line, &[Line::for_area(Area::LEISURE)], *week);
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
        let at = |mine: Line, others: &[Line]| Here::among(mine, others, own);
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
            let with_phone = Here::among(computer, &[phone], *week);
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
        let (on_computer, on_phone) = (Here::among(computer, &[leisure_phone], own), Here::among(leisure_phone, &[computer], own));
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
        assert_eq!(Here::among(computer, &[Line::default()], own).unheld, Area::default());
        // No hours at all: what each device is for counts.
        assert_eq!(Here::among(computer.with_cell(Time::Work, Some(Area::default())), &[], Week::default()).unheld, Area::PERSONAL);
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

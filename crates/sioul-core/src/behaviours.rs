// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What reaches you, read as a few choices (docs/attention.md, "The page,
//! a reading of the matrix"). The matrix (`attention`) stays the store, the
//! same nine cells per row on every device; Settings ▸ What reaches you reads
//! it as behaviours and writes a behaviour back as cells, so that nothing
//! migrates and an older Sioul keeps reading every row.
//!
//! - **A behaviour** is a named pattern over a row's nine cells (the seven
//!   times, time for you, do not disturb): Always, When awake, In work and
//!   admin hours, In work hours, At the gathered times, Never told
//!   (`Behaviour`, `pattern`). Mail and messages have their own
//!   patterns, since a phone cannot show what it holds and mail is shown on
//!   the Porch without a word; the circle changes two things only, as the
//!   matrix shipped them: outside a behaviour's times your safe senders' and
//!   Always through's mail is shown, the others' waits out of sight; in Free
//!   time your safe people's messages still come when awake.
//! - **A row reads a behaviour** when it acts as the pattern at every time
//!   and layer (`same_effect`: a layer's cell that can change nothing is not
//!   compared); otherwise it reads "Your own" (`reading` gives none).
//! - **Circles** (`Circle`): Always through, Safe, Your contacts,
//!   Restricted, Everyone else, on mail and messages; a group conversation's
//!   row (`messages.groups`) goes with your contacts' messages, as it ships
//!   and as every preset moves it. Calls leave the page: their rows stay in
//!   the store and the engine.
//! - **Sioul's own notices, in four groups** (`Group`): what you set or
//!   asked for (one choice, the doses' cell for sleep alone: while you sleep
//!   or at waking; codes, the alarm at waking and an event's alarms are never
//!   written by it), reminders
//!   (when awake or off), care (four switches), sites and apps (at once or at
//!   the gathered times). A group whose rows match no choice reads "Your own".
//!
//! Every write goes through `attention::apply_rows`: per row, the cells a
//! choice names (`Cells`: a cell it does not name stays as the file has it),
//! under the configuration's lock, a fixed cell never moved.

use crate::attention::{self, Attention, Column, Kind, Level, Person, Row};
use crate::reach::Channel;
use serde::Serialize;

/// What a choice writes on a row: each cell set, or kept as the file has it (none).
pub type Cells = [Option<Level>; 9];

/// A whole row's cells, as a write.
fn whole(cells: [Level; 9]) -> Cells {
    cells.map(Some)
}

/// The doses' cell a choice of "What you set or asked for" names: sleep.
const DOSES_CELL: Column = Column::Sleep;

// ---------------------------------------------------------------- behaviours

/// A named pattern of a row's cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Behaviour {
    /// At every time, the night, a pause and do-not-disturb included (mail
    /// is shown without a word at night and in a pause, as Always through's).
    Always,
    /// In work, admin, leisure and meals; mail shown, messages in the shade,
    /// under the layers.
    Awake,
    /// In work and admin hours.
    WorkAdmin,
    /// In work hours.
    Work,
    /// At the gathered times (Sioul's own rows that take them).
    Gathered,
    /// Never told, at any time: mail shown on the Porch without a word, a
    /// message held. One choice for what the matrix says two ways for mail
    /// (shown, or out of sight), both of which never tell.
    Never,
}

impl Behaviour {
    pub const ALL: [Behaviour; 6] = [Behaviour::Always, Behaviour::Awake, Behaviour::WorkAdmin, Behaviour::Work, Behaviour::Gathered, Behaviour::Never];

    pub fn id(self) -> &'static str {
        match self {
            Behaviour::Always => "always",
            Behaviour::Awake => "awake",
            Behaviour::WorkAdmin => "work-admin",
            Behaviour::Work => "work",
            Behaviour::Gathered => "gathered",
            Behaviour::Never => "never",
        }
    }

    pub fn read(text: &str) -> Option<Behaviour> {
        Behaviour::ALL.into_iter().find(|b| b.id() == text.trim())
    }
}

/// Who, as the page groups people: one line per circle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Circle {
    /// The Always through list.
    Always,
    Safe,
    /// Everyone in your address books, with nothing to set (`neutral`).
    Contacts,
    Restricted,
    /// In no address book and on no list (`stranger`).
    Others,
}

impl Circle {
    pub const ALL: [Circle; 5] = [Circle::Always, Circle::Safe, Circle::Contacts, Circle::Restricted, Circle::Others];

    pub fn id(self) -> &'static str {
        match self {
            Circle::Always => "always",
            Circle::Safe => "safe",
            Circle::Contacts => "contacts",
            Circle::Restricted => "restricted",
            Circle::Others => "others",
        }
    }

    pub fn read(text: &str) -> Option<Circle> {
        Circle::ALL.into_iter().find(|c| c.id() == text.trim())
    }

    /// Its row of the matrix.
    pub fn person(self) -> Person {
        match self {
            Circle::Always => Person::Always,
            Circle::Safe => Person::Safe,
            Circle::Contacts => Person::Neutral,
            Circle::Restricted => Person::Restricted,
            Circle::Others => Person::Stranger,
        }
    }
}

/// The page's channels: mail and messages (calls are set per phone).
pub const CHANNELS: [Channel; 2] = [Channel::Mail, Channel::Messages];

/// The rows a circle's choice writes on a channel: its person's, and for
/// your contacts' messages the group conversations' too.
pub fn rows(channel: Channel, circle: Circle) -> Vec<Row> {
    let mut out = vec![Row::People(channel, circle.person())];
    if channel == Channel::Messages && circle == Circle::Contacts {
        out.push(Row::People(Channel::Messages, Person::Groups));
    }
    out
}

/// A row's nine cells, in the columns' order.
pub fn cells_of(attention: &Attention, row: Row) -> [Level; 9] {
    Column::ALL.map(|column| attention.cell(row, column))
}

/// The cells of `behaviour` on `row`, in the columns' order; none where the
/// row cannot hold it (a value not offered there, a fixed cell that differs,
/// a channel off the page). Exactly, W A L M S P F Y D:
///
/// | Mail | cells |
/// |---|---|
/// | Always | ● ● ● ● ◑ ◑ ● ● ● |
/// | When awake | ● ● ● ● o o o ◑ ◑ |
/// | Work and admin | ● ● o o o o o ◑ ◑ |
/// | Work | ● o o o o o o ◑ ◑ |
/// | Never told | ◑ everywhere |
///
/// where o is ◑ for Always through and Safe (shown on the Porch, untold) and
/// ○ for the others (out of sight).
///
/// | Messages | cells |
/// |---|---|
/// | Always | ● everywhere |
/// | When awake | ● ● ● ● ○ ○ f ● ◑ |
/// | Work and admin | ● ● ○ ○ ○ ○ ○ ● ○ |
/// | Work | ● ○ ○ ○ ○ ○ ○ ● ○ |
/// | Never told | ○ everywhere |
///
/// where f is ● for Safe (Free time lets your safe people through) and ○ for
/// the others. Gathered is no people's row's; a message cannot be shown.
pub fn pattern(row: Row, behaviour: Behaviour) -> Option<[Level; 9]> {
    use Level::{Later, Now, Quiet};
    let Row::People(channel, person) = row else { return None };
    if person == Person::Blocked || attention::screened(row) {
        return None;
    }
    let cells: [Level; 9] = match channel {
        Channel::Mail => {
            let off = if matches!(person, Person::Always | Person::Safe) { Quiet } else { Later };
            match behaviour {
                Behaviour::Always => [Now, Now, Now, Now, Quiet, Quiet, Now, Now, Now],
                Behaviour::Awake => [Now, Now, Now, Now, off, off, off, Quiet, Quiet],
                Behaviour::WorkAdmin => [Now, Now, off, off, off, off, off, Quiet, Quiet],
                Behaviour::Work => [Now, off, off, off, off, off, off, Quiet, Quiet],
                Behaviour::Never => [Quiet; 9],
                Behaviour::Gathered => return None,
            }
        }
        Channel::Messages => {
            let free = if person == Person::Safe { Now } else { Later };
            match behaviour {
                Behaviour::Always => [Now; 9],
                Behaviour::Awake => [Now, Now, Now, Now, Later, Later, free, Now, Quiet],
                Behaviour::WorkAdmin => [Now, Now, Later, Later, Later, Later, Later, Now, Later],
                Behaviour::Work => [Now, Later, Later, Later, Later, Later, Later, Now, Later],
                Behaviour::Never => [Later; 9],
                Behaviour::Gathered => return None,
            }
        }
        Channel::Calls => return None,
    };
    holds(row, &cells).then_some(cells)
}

/// Whether `row` may take these cells: each one offered, a fixed one unchanged.
fn holds(row: Row, cells: &[Level; 9]) -> bool {
    let usual = Attention::usual();
    Column::ALL.iter().zip(cells).all(|(column, level)| match attention::lock(row, *column) {
        Some(_) => usual.cell(row, *column) == *level,
        None => attention::choices(row, *column).contains(level),
    })
}

/// What a row does at each time with each layer on or off, as
/// `Attention::level` reads it (the least strict time, the stricter of it and
/// each layer that holds; a layer's ☆ lets through as ●): two rows that act
/// the same have the same.
fn effect(cells: &[Level; 9]) -> [[Level; 4]; 7] {
    let read = |level: Level| if level == Level::Through { Level::Now } else { level };
    let (slot, dnd) = (read(cells[7]), read(cells[8]));
    let mut out = [[Level::Now; 4]; 7];
    for (at, time) in out.iter_mut().enumerate() {
        let base = read(cells[at]);
        *time = [base, base.max(slot), base.max(dnd), base.max(slot).max(dnd)];
    }
    out
}

/// Whether two rows act the same at every time and layer: a layer's cell
/// that changes nothing (a row that waits at every time) is not compared.
pub fn same_effect(a: &[Level; 9], b: &[Level; 9]) -> bool {
    effect(a) == effect(b)
}

/// The behaviours a circle may take on a channel: those every one of its rows can hold.
pub fn offered(channel: Channel, circle: Circle) -> Vec<Behaviour> {
    Behaviour::ALL.into_iter().filter(|b| rows(channel, circle).iter().all(|r| pattern(*r, *b).is_some())).collect()
}

/// The behaviour a circle reads on a channel: the one each of its rows acts
/// as; none, "Your own", when a row acts as none, or its rows as different ones.
pub fn reading(attention: &Attention, channel: Channel, circle: Circle) -> Option<Behaviour> {
    offered(channel, circle).into_iter().find(|b| rows(channel, circle).iter().all(|r| pattern(*r, *b).is_some_and(|p| same_effect(&cells_of(attention, *r), &p))))
}

/// The cells a behaviour writes for a circle on a channel; refused when it is not offered there.
pub fn writes(channel: Channel, circle: Circle, behaviour: Behaviour) -> Result<Vec<(Row, Cells)>, String> {
    rows(channel, circle)
        .into_iter()
        .map(|row| pattern(row, behaviour).map(|cells| (row, whole(cells))).ok_or_else(|| format!("{}: {}: not offered", row.id(), behaviour.id())))
        .collect()
}

// ---------------------------------------------------------------- Sioul's own, in four groups

/// Sioul's own notices as the page groups them (design 1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Group {
    /// Codes, doses, the alarm at waking, an event's alarms; one choice, the
    /// doses' cell for sleep alone: while you sleep or at waking.
    Asked,
    /// Before an event, the working day before, dates and waits: when awake, or off.
    Reminders,
    /// Meals, naps and the night; the pause to move; "Work hours are over";
    /// the time running: a switch each.
    Care,
    /// Sites' news (a computer), other apps' automatons (a phone): at once or gathered.
    Sites,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Asked, Group::Reminders, Group::Care, Group::Sites];

    pub fn id(self) -> &'static str {
        match self {
            Group::Asked => "asked",
            Group::Reminders => "reminders",
            Group::Care => "care",
            Group::Sites => "sites",
        }
    }

    pub fn read(text: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|g| g.id() == text.trim())
    }

    /// The kinds it holds; its choice reads and writes all of them, but
    /// What you set or asked for's, which reads and writes the doses' cell
    /// for sleep alone (`group_reading`, `group_writes`).
    pub fn kinds(self) -> &'static [Kind] {
        match self {
            Group::Asked => &[Kind::Codes, Kind::Doses, Kind::Wake, Kind::Alarms],
            Group::Reminders => &[Kind::Before, Kind::DayBefore, Kind::Dates],
            Group::Care => &[Kind::Needs, Kind::Move, Kind::WorkOver, Kind::Time],
            Group::Sites => &[Kind::Sites, Kind::AppAutomatons],
        }
    }

    /// Its choices, by id (care's are its switches, "on" and "off" each).
    pub fn choices(self) -> &'static [&'static str] {
        match self {
            Group::Asked => &["sleep", "waking"],
            Group::Reminders => &["awake", "off"],
            Group::Care => &["on", "off"],
            Group::Sites => &["now", "gathered"],
        }
    }
}

/// A kind's cells for one of its group's choices; none for a choice it does not have.
pub fn own_pattern(kind: Kind, choice: &str) -> Option<[Level; 9]> {
    use Level::{Gathered, Later, Never, Now};
    let row = Row::Own(kind);
    let usual = cells_of(&Attention::usual(), row);
    let group = Group::ALL.into_iter().find(|g| g.kinds().contains(&kind))?;
    if !group.choices().contains(&choice) {
        return None;
    }
    let cells = match (group, choice) {
        // Read and written by the doses' sleep cell alone (`doses_choice`).
        (Group::Asked, _) => return None,
        (Group::Reminders, "awake") | (Group::Care, "on") | (Group::Sites, "gathered") => usual,
        (Group::Reminders, _) => [Later; 9],
        (Group::Care, _) => [Never; 9],
        (Group::Sites, _) => {
            // At once in the hours its gathering would have told it, held in the rests and the layers.
            usual.map(|level| if level == Gathered { Now } else { level })
        }
    };
    holds(row, &cells).then_some(cells)
}

/// What a group reads: the choice every one of its kinds acts as; none,
/// "Your own", otherwise. Care is read switch by switch (`care`).
pub fn group_reading(attention: &Attention, group: Group) -> Option<&'static str> {
    if group == Group::Asked {
        // The doses' cell for sleep alone: the group's other rows and the doses' other cells are not read.
        return match attention.cell(Row::Own(Kind::Doses), DOSES_CELL) {
            Level::Now => Some("sleep"),
            Level::Later => Some("waking"),
            _ => None,
        };
    }
    if group == Group::Care {
        let all: Vec<Option<bool>> = group.kinds().iter().map(|k| care(attention, *k)).collect();
        return match all.as_slice() {
            values if values.iter().all(|v| *v == Some(true)) => Some("on"),
            values if values.iter().all(|v| *v == Some(false)) => Some("off"),
            _ => None,
        };
    }
    group.choices().iter().copied().find(|choice| group.kinds().iter().all(|k| own_pattern(*k, choice).is_some_and(|p| same_effect(&cells_of(attention, Row::Own(*k)), &p))))
}

/// A care switch: on (as Sioul ships it), off (not at all), none for your own cells.
pub fn care(attention: &Attention, kind: Kind) -> Option<bool> {
    let cells = cells_of(attention, Row::Own(kind));
    ["on", "off"].iter().find(|choice| own_pattern(kind, choice).is_some_and(|p| same_effect(&cells, &p))).map(|choice| *choice == "on")
}

/// The cells a group's choice writes, every kind of it; for care, one switch (`kind`).
pub fn group_writes(group: Group, choice: &str, kind: Option<Kind>) -> Result<Vec<(Row, Cells)>, String> {
    if group == Group::Asked {
        // One cell: the doses' for sleep; nothing else of the group, nothing else of the doses.
        let level = match choice {
            "sleep" => Level::Now,
            "waking" => Level::Later,
            _ => return Err(format!("{}: {choice}: not offered", group.id())),
        };
        let mut cells: Cells = [None; 9];
        let at = Column::ALL.iter().position(|c| *c == DOSES_CELL).unwrap_or(4);
        cells[at] = Some(level);
        return Ok(vec![(Row::Own(Kind::Doses), cells)]);
    }
    let kinds: Vec<Kind> = match (group, kind) {
        (Group::Care, Some(kind)) if group.kinds().contains(&kind) => vec![kind],
        (Group::Care, _) => return Err(format!("{}: which switch?", group.id())),
        _ => group.kinds().to_vec(),
    };
    kinds.into_iter().map(|k| own_pattern(k, choice).map(|cells| (Row::Own(k), whole(cells))).ok_or_else(|| format!("{}: {choice}: not offered", group.id()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::Preset;

    fn row(attention: &Attention, channel: Channel, person: Person) -> [Level; 9] {
        cells_of(attention, Row::People(channel, person))
    }

    /// Each pattern written out, W A L M S P F Y D, so that a change to one
    /// is seen here first.
    #[test]
    fn each_pattern_is_exact() {
        use Level::{Later as L, Now as N, Quiet as Q};
        let mail = |p: Person, b: Behaviour| pattern(Row::People(Channel::Mail, p), b);
        let messages = |p: Person, b: Behaviour| pattern(Row::People(Channel::Messages, p), b);
        assert_eq!(mail(Person::Safe, Behaviour::Always), Some([N, N, N, N, Q, Q, N, N, N]));
        assert_eq!(mail(Person::Safe, Behaviour::Awake), Some([N, N, N, N, Q, Q, Q, Q, Q]));
        assert_eq!(mail(Person::Neutral, Behaviour::Awake), Some([N, N, N, N, L, L, L, Q, Q]));
        assert_eq!(mail(Person::Safe, Behaviour::WorkAdmin), Some([N, N, Q, Q, Q, Q, Q, Q, Q]));
        assert_eq!(mail(Person::Stranger, Behaviour::WorkAdmin), Some([N, N, L, L, L, L, L, Q, Q]));
        assert_eq!(mail(Person::Restricted, Behaviour::Work), Some([N, L, L, L, L, L, L, Q, Q]));
        assert_eq!(mail(Person::Neutral, Behaviour::Never), Some([Q; 9]));
        assert_eq!(mail(Person::Neutral, Behaviour::Gathered), None);
        assert_eq!(messages(Person::Always, Behaviour::Always), Some([N; 9]));
        assert_eq!(messages(Person::Safe, Behaviour::Awake), Some([N, N, N, N, L, L, N, N, Q]));
        assert_eq!(messages(Person::Neutral, Behaviour::Awake), Some([N, N, N, N, L, L, L, N, Q]));
        assert_eq!(messages(Person::Stranger, Behaviour::WorkAdmin), Some([N, N, L, L, L, L, L, N, L]));
        assert_eq!(messages(Person::Restricted, Behaviour::Work), Some([N, L, L, L, L, L, L, N, L]));
        assert_eq!(messages(Person::Neutral, Behaviour::Never), Some([L; 9]));
        // What a row cannot hold is no pattern: Always through's messages only Always (they
        // are never held), the gathered times for no person; the blocked and the calls are no
        // page's rows.
        assert_eq!(messages(Person::Always, Behaviour::Awake), None);
        assert_eq!(messages(Person::Always, Behaviour::Never), None);
        assert_eq!(mail(Person::Always, Behaviour::Never), Some([Q; 9]));
        assert_eq!(mail(Person::Blocked, Behaviour::Never), None);
        assert_eq!(pattern(Row::People(Channel::Calls, Person::Safe), Behaviour::Always), None);
    }

    /// As Sioul ships the matrix, every circle reads a behaviour: none reads
    /// "Your own" out of the box.
    #[test]
    fn every_shipped_row_reads_a_behaviour() {
        let usual = Attention::usual();
        let read = |channel: Channel, circle: Circle| reading(&usual, channel, circle);
        assert_eq!(read(Channel::Mail, Circle::Always), Some(Behaviour::Always));
        assert_eq!(read(Channel::Mail, Circle::Safe), Some(Behaviour::Awake));
        assert_eq!(read(Channel::Mail, Circle::Contacts), Some(Behaviour::WorkAdmin));
        assert_eq!(read(Channel::Mail, Circle::Restricted), Some(Behaviour::Work));
        assert_eq!(read(Channel::Mail, Circle::Others), Some(Behaviour::WorkAdmin));
        assert_eq!(read(Channel::Messages, Circle::Always), Some(Behaviour::Always));
        assert_eq!(read(Channel::Messages, Circle::Safe), Some(Behaviour::Awake));
        assert_eq!(read(Channel::Messages, Circle::Contacts), Some(Behaviour::Awake));
        assert_eq!(read(Channel::Messages, Circle::Restricted), Some(Behaviour::Work));
        assert_eq!(read(Channel::Messages, Circle::Others), Some(Behaviour::WorkAdmin));
        for group in Group::ALL {
            assert!(group_reading(&usual, group).is_some(), "{group:?}");
        }
        assert_eq!(group_reading(&usual, Group::Asked), Some("sleep"));
        assert_eq!(group_reading(&usual, Group::Reminders), Some("awake"));
        assert_eq!(group_reading(&usual, Group::Care), Some("on"));
        assert_eq!(group_reading(&usual, Group::Sites), Some("gathered"));
    }

    /// The presets read as behaviours where their rows are one, "Your own"
    /// elsewhere (handoff's table).
    #[test]
    fn the_presets_read_as_far_as_they_go() {
        let quieter = Preset::Quieter.matrix();
        assert_eq!(reading(&quieter, Channel::Mail, Circle::Safe), Some(Behaviour::WorkAdmin));
        assert_eq!(reading(&quieter, Channel::Messages, Circle::Contacts), Some(Behaviour::WorkAdmin));
        assert_eq!(reading(&quieter, Channel::Messages, Circle::Safe), None);
        assert_eq!(group_reading(&quieter, Group::Reminders), None);
        let reachable = Preset::Reachable.matrix();
        assert_eq!(reading(&reachable, Channel::Mail, Circle::Contacts), Some(Behaviour::Awake));
        assert_eq!(reading(&reachable, Channel::Mail, Circle::Others), None);
        assert_eq!(group_reading(&reachable, Group::Sites), None);
    }

    /// A row changed in the grid reads "Your own"; a cell that changes nothing does not.
    #[test]
    fn your_own_is_read_where_no_pattern_acts_alike() {
        let mut attention = Attention::usual();
        attention.set(Row::People(Channel::Mail, Person::Safe), Column::Meals, Level::Later).unwrap();
        assert_eq!(reading(&attention, Channel::Mail, Circle::Safe), None);
        // A group conversation's row on its own: your contacts' messages read as yours.
        let mut groups = Attention::usual();
        groups.set(Row::People(Channel::Messages, Person::Groups), Column::Leisure, Level::Later).unwrap();
        assert_eq!(reading(&groups, Channel::Messages, Circle::Contacts), None);
        // A layer's cell that holds nothing more, on a row that waits at every time: still Never told.
        let mut never = Attention::usual();
        for (row, cells) in writes(Channel::Messages, Circle::Others, Behaviour::Never).unwrap() {
            for (column, level) in Column::ALL.iter().zip(cells).filter_map(|(c, l)| l.map(|l| (c, l))) {
                never.set(row, *column, level).unwrap();
            }
        }
        never.set(Row::People(Channel::Messages, Person::Stranger), Column::Slot, Level::Now).unwrap();
        assert_eq!(reading(&never, Channel::Messages, Circle::Others), Some(Behaviour::Never));
        // Doses held in a pause: the group's choice reads the doses' sleep cell alone, still theirs.
        let mut doses = Attention::usual();
        doses.set(Row::Own(Kind::Doses), Column::Pause, Level::Later).unwrap();
        assert_eq!(group_reading(&doses, Group::Asked), Some("sleep"));
        doses.set(Row::Own(Kind::Doses), Column::Sleep, Level::Later).unwrap();
        assert_eq!(group_reading(&doses, Group::Asked), Some("waking"));
        // One care switch off, the others on: the group is yours, each switch says its own.
        let mut care_off = Attention::usual();
        for (row, cells) in group_writes(Group::Care, "off", Some(Kind::Move)).unwrap() {
            for (column, level) in Column::ALL.iter().zip(cells).filter_map(|(c, l)| l.map(|l| (c, l))) {
                care_off.set(row, *column, level).unwrap();
            }
        }
        assert_eq!(group_reading(&care_off, Group::Care), None);
        assert_eq!(care(&care_off, Kind::Move), Some(false));
        assert_eq!(care(&care_off, Kind::Needs), Some(true));
    }

    /// Every behaviour offered writes cells the row takes, and reads back as itself.
    #[test]
    fn a_write_reads_back() {
        for channel in CHANNELS {
            for circle in Circle::ALL {
                for behaviour in offered(channel, circle) {
                    let mut attention = Attention::usual();
                    for (row, cells) in writes(channel, circle, behaviour).unwrap() {
                        for (column, level) in Column::ALL.iter().zip(cells).filter_map(|(c, l)| l.map(|l| (c, l))) {
                            attention.set(row, *column, level).unwrap_or_else(|e| panic!("{channel:?} {circle:?} {behaviour:?}: {e}"));
                        }
                    }
                    assert_eq!(reading(&attention, channel, circle), Some(behaviour), "{channel:?} {circle:?}");
                }
            }
        }
        for group in Group::ALL {
            for choice in group.choices() {
                let mut attention = Attention::usual();
                let kinds: Vec<Option<Kind>> = if group == Group::Care { group.kinds().iter().map(|k| Some(*k)).collect() } else { vec![None] };
                for kind in kinds {
                    for (row, cells) in group_writes(group, choice, kind).unwrap() {
                        for (column, level) in Column::ALL.iter().zip(cells).filter_map(|(c, l)| l.map(|l| (c, l))) {
                            attention.set(row, *column, level).unwrap_or_else(|e| panic!("{group:?} {choice}: {e}"));
                        }
                    }
                }
                assert_eq!(group_reading(&attention, group), Some(*choice), "{group:?}");
            }
        }
        // Writing never moves a fixed cell: the doses, codes, the alarm at waking and the blocked.
        for (row, cells) in group_writes(Group::Asked, "waking", None).unwrap() {
            for (column, level) in Column::ALL.iter().zip(cells).filter_map(|(c, l)| l.map(|l| (c, l))) {
                if attention::lock(row, *column).is_some() {
                    assert_eq!(level, Attention::usual().cell(row, *column), "{row:?} {column:?}");
                }
            }
        }
    }

    /// "Doses while you sleep / at waking" writes the doses' cell for sleep and
    /// nothing else, through the page's writer, on a file (review R2): codes
    /// set to come at night, an event's alarms set to ring at night, and doses
    /// held in a pause all stay as they were set.
    #[test]
    fn the_doses_choice_writes_their_sleep_cell_alone() {
        use crate::config::{Config, SettingValue};
        let dir = std::env::temp_dir().join(format!("sioul-behaviours-doses-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "language = \"en\"\n").unwrap();
        let words = |w: &[&str]| SettingValue::Texts(w.iter().map(|w| w.to_string()).collect());
        let set = |key: &str, w: &[&str]| attention::apply(&path, &Config::load(&path).unwrap(), key, &words(w)).unwrap();
        let choose = |choice: &str| attention::apply_rows(&path, &Config::load(&path).unwrap(), &group_writes(Group::Asked, choice, None).unwrap()).unwrap();
        let read = || Attention::of(&Config::load(&path).unwrap());
        // The one write names one cell.
        let written = group_writes(Group::Asked, "waking", None).unwrap();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].0, Row::Own(Kind::Doses));
        assert_eq!(written[0].1.iter().filter(|c| c.is_some()).count(), 1);
        // 1. Codes at once while asleep, then "Doses at waking": codes stay.
        set("attention.codes", &["sleep"]);
        choose("waking");
        let a = read();
        assert_eq!(a.cell(Row::Own(Kind::Codes), Column::Sleep), Level::Now, "codes kept");
        assert_eq!(a.cell(Row::Own(Kind::Doses), Column::Sleep), Level::Later);
        assert_eq!(group_reading(&a, Group::Asked), Some("waking"));
        // 2. An event's alarms ringing at night whatever the event: kept through either choice.
        set("attention.alarms", &["sleep"]);
        choose("sleep");
        choose("waking");
        let a = read();
        assert_eq!(a.cell(Row::Own(Kind::Alarms), Column::Sleep), Level::Now, "alarms kept");
        assert_eq!(a.cell(Row::Own(Kind::Codes), Column::Sleep), Level::Now, "codes still kept");
        // 3. Doses held in a pause, then "Doses while you sleep": the pause's cell stays.
        set("attention.doses", &["pause:later"]);
        choose("sleep");
        let a = read();
        assert_eq!(a.cell(Row::Own(Kind::Doses), Column::Pause), Level::Later, "the pause's cell kept");
        assert_eq!(a.cell(Row::Own(Kind::Doses), Column::Sleep), Level::Now);
        assert_eq!(group_reading(&a, Group::Asked), Some("sleep"));
        // The doses' other cells, codes' and the alarm at waking's never moved.
        for column in Column::ALL {
            if !matches!(column, Column::Sleep | Column::Pause) {
                assert_eq!(a.cell(Row::Own(Kind::Doses), column), Attention::usual().cell(Row::Own(Kind::Doses), column), "{column:?}");
            }
            assert_eq!(a.cell(Row::Own(Kind::Wake), column), Attention::usual().cell(Row::Own(Kind::Wake), column), "{column:?}");
        }
        assert!(group_writes(Group::Asked, "now", None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The offers per circle: what each line of the page proposes.
    #[test]
    fn what_each_line_offers() {
        use Behaviour::*;
        assert_eq!(offered(Channel::Mail, Circle::Always), vec![Always, Awake, WorkAdmin, Work, Never]);
        assert_eq!(offered(Channel::Mail, Circle::Safe), vec![Always, Awake, WorkAdmin, Work, Never]);
        assert_eq!(offered(Channel::Messages, Circle::Always), vec![Always]);
        assert_eq!(offered(Channel::Messages, Circle::Contacts), vec![Always, Awake, WorkAdmin, Work, Never]);
        // Two rows set apart from each other read as yours, whatever each says.
        let mut split = Attention::usual();
        for (column, level) in Column::ALL.iter().zip(pattern(Row::People(Channel::Messages, Person::Groups), Work).unwrap()) {
            split.set(Row::People(Channel::Messages, Person::Groups), *column, level).unwrap();
        }
        assert_eq!(reading(&split, Channel::Messages, Circle::Contacts), None);
        assert_eq!(row(&split, Channel::Messages, Person::Neutral), row(&Attention::usual(), Channel::Messages, Person::Neutral));
    }
}

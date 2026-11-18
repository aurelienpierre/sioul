// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each device is for, as this device knows the devices that count
//! (docs/areas.md, "Which device tells"): what this device shows and tells
//! now (`Known::lines`, for `sioul_core::quiet::Mode::on`), and what the
//! keeper of the notices tells when it is another device (`Known::as_keeper`).
//!
//! One device tells: the one used last keeps the "notices" lease, as before
//! devices. It tells what comes forward on it now, by its own cells. What
//! does not waits, and is told ("The Porch opens") when a device it comes
//! forward on is the one used. Every other device judges each thing as the
//! keeper would (`Known::judge`), and claims it without a word only on
//! positive knowledge that the keeper tells it: the keeper's own cells, read
//! from its entry as the folder holds it at that moment, bring it in the
//! same hours as here; or the keeper narrows nothing; or, for a keeper on an
//! older Sioul, it arrived while that keeper kept the notices. Otherwise it
//! keeps the thing, and tells it once it keeps the notices itself, unless
//! the told log says it was told. Never lost comes before never doubled
//! (review of phase 2, N1).
//!
//! Each device says its own cells in its sealed entry (`devices::Entry::cells`),
//! as it reads its own line of the settings in its own week: the others read
//! that, never the shared settings, which a device may not share, or not yet.
//! It says too which cells it acts on of each other device
//! (`devices::Entry::known`): the keeper lets a thing wait for a device only
//! once that device acts on the keeper's own cells; until then, the keeper
//! tells it (`sioul_core::areas::Other::acts_on`).
//!
//! Three sources, all on this device, none needing the sharing's key, so
//! that every process reads them alike (the window, a phone's background
//! step and its listener of other apps, the command line): this device's own
//! line, in its settings (`[device."<id>"]`, `sioul_core::device_said`); its
//! id in the sharing (`share::Here`, `here.toml`); and what it last kept of
//! the others (`peers::Peers`, `peers.json`, which the window learns for the
//! doses): each one's newest entry, and whether it still counts.
//!
//! A device **counts** as it does for the doses (`sioul_core::health::counts`:
//! not said off, still sharing, heard within a week) and while it is not
//! forgotten here. One that counts with an older Sioul (`Known::older`) is
//! read as bringing nothing: it claims every letter without a word while
//! another device keeps the notices, as before devices, so a thing left
//! waiting for it would never be told. Keeping them, it tells everything.
//! With sharing off, this device is alone, and its own line is still read.
//!
//! Nothing here reads the doses, and the doses read nothing here.

use crate::devices::Entry;
use crate::peers::Peers;
use sioul_core::areas::{Cells, Line, Lines, Other};
use sioul_core::config::Config;
use sioul_core::device_said::Said;
use std::collections::BTreeMap;
use std::path::Path;

/// Another device that counts and follows what devices are for, as its
/// newest entry kept here says it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    /// Its cells' words (`devices::Entry::cells`); empty: nothing said of it.
    pub cells: BTreeMap<String, String>,
    /// The line your settings say of it, when it will follow that line once
    /// it runs: it shares the part "settings", and the line was said after
    /// what it last said of itself (`Said::changed` later than
    /// `Entry::stated`; review of phase 2, N3). None: read by its entry.
    pub follows: Option<Line>,
    /// The cells it acts on of each other device (`devices::Entry::known`).
    pub known: BTreeMap<String, String>,
    /// A phone: it tells no date reminder (its form, `devices::Entry::form_or_kind`).
    pub carried: bool,
}

impl Seen {
    /// What comes forward on it, as the device reading takes it.
    fn read(&self) -> Cells {
        self.follows.map(Cells::Line).unwrap_or_else(|| Cells::read(&self.cells))
    }

    fn other(&self, of: &str) -> Other {
        Other { cells: self.read(), acts_on: Some(self.known.get(of).cloned().unwrap_or_default()) }
    }
}

/// Whether a device that does not keep the notices may claim, without a
/// word, what its reading of the keeper tells now (`Judged::sure`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sure {
    /// This device keeps the notices: it tells.
    Mine,
    /// The keeper tells it: it narrows nothing, or its own cells bring it in
    /// the hours of this device, which both share.
    Told,
    /// What arrived since then (Unix seconds), when the keeper took the
    /// notices: a keeper on an older Sioul tells everything the hours bring
    /// while it keeps them, and claimed everything before, without a word.
    /// The arrival is the server's clock, the time the keeper's: what
    /// arrived within the margin of two clocks after it (`CLOCKS`) is kept.
    Since(i64),
    /// Nothing known: the keeper's entry does not read here, or its hours
    /// may not be this device's (one of the two does not share the part
    /// "settings"). The thing is kept, and told once this device keeps the
    /// notices, unless the told log says it was told: at worst twice.
    Keep,
}

/// The margin within which two clocks are not told apart, as the sharing
/// sets aside a device whose clock runs more than ten minutes ahead.
pub const CLOCKS: i64 = 10 * 60;

impl Sure {
    /// Whether a thing that arrived at `arrived` (Unix seconds; 0 when not
    /// known) may be claimed when the reading tells it. After an older
    /// keeper took the notices, only by more than `CLOCKS`: a server's clock
    /// ahead of the keeper's never turns a thing kept into one claimed.
    pub fn holds(self, arrived: i64) -> bool {
        match self {
            Sure::Mine | Sure::Told => true,
            Sure::Since(since) => arrived > 0 && arrived >= since.saturating_add(CLOCKS),
            Sure::Keep => false,
        }
    }
}

/// How this device judges a thing while a device keeps the notices (`Known::judge`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    /// The device read: this device, when it keeps them; else the keeper's
    /// own cells, nothing widened by what no device holds (what it tells by
    /// them, it tells whatever it knows of the others), or the hours alone
    /// for a keeper that narrows nothing, is older, or is not known.
    pub lines: Lines,
    pub sure: Sure,
    /// Whether the keeper is part of the moment (`mailnote::Seen::moment`),
    /// so that the lease moving judges again what waited: something is said
    /// of some device, an older Sioul counts, or nothing is known of the
    /// keeper. With nothing said and every device on this build, it is not,
    /// as before devices.
    pub marks: bool,
}

/// The devices that count, as this device knows them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Known {
    /// This device's id in the sharing; "" before it has one.
    pub me: String,
    /// This device's own line, as its settings say it.
    pub mine: Line,
    /// The others that count and follow what devices are for, by id.
    pub others: BTreeMap<String, Seen>,
    /// The ids of the devices that count whose Sioul is older.
    pub older: Vec<String>,
}

/// Whether an entry says its device shares the part "settings".
fn shares_settings(entry: &Entry) -> bool {
    entry.parts.as_ref().is_some_and(|parts| parts.iter().any(|part| part == "settings"))
}

impl Known {
    /// From what this device's settings say of the devices (`said`, by id)
    /// and what it kept of the others (`kept`; none with sharing off), at
    /// `now` (this device's clock, Unix seconds).
    pub fn of(me: &str, said: &BTreeMap<String, Said>, kept: Option<&Peers>, now: i64) -> Known {
        let mut known = Known { me: me.to_string(), mine: said.get(me).map(Said::line).unwrap_or_default(), ..Known::default() };
        for (id, seen) in kept.iter().flat_map(|kept| kept.peers.iter()) {
            let counts = !id.is_empty() && id != me && seen.forgotten.is_none() && !seen.peer.forgotten && sioul_core::health::counts(&seen.peer, now);
            if !counts {
                continue;
            }
            match seen.entry.as_ref().filter(|entry| entry.areas >= crate::devices::AREAS) {
                Some(entry) => {
                    let carried = entry.form_or_kind() == crate::devices::PHONE;
                    // A line said for it after it last said its own, which it will follow once it runs (N3).
                    let follows = said.get(id).filter(|said| shares_settings(entry) && said.changed.is_some_and(|at| at > entry.stated)).map(Said::line);
                    known.others.insert(id.clone(), Seen { cells: entry.cells.clone(), follows, known: entry.known.clone(), carried });
                }
                None => known.older.push(id.clone()),
            }
        }
        known
    }

    /// Read from this device's files (`state`: Sioul's state folder): its id,
    /// and, while a sharing folder is set, what it kept of the others.
    pub fn read(config: &Config, state: &Path, now: i64) -> Known {
        let here = crate::share::Here::load(state);
        let kept = here.folder_path().is_some().then(|| Peers::load(&Peers::path(state)));
        Known::of(&here.id, &config.devices, kept.as_ref(), now)
    }

    /// This device read among the others that count: what it shows and,
    /// keeping the notices, tells (`quiet::Mode::on`).
    pub fn lines(&self) -> Lines {
        Lines { mine: Cells::Line(self.mine), key: None, others: self.others.values().map(|seen| seen.other(&self.me)).collect() }
    }

    /// The same, for the date reminders: a phone tells none, so it holds
    /// none for later (`reminders::Holds::on`).
    pub fn lines_for_dates(&self) -> Lines {
        Lines { others: self.others.values().filter(|seen| !seen.carried).map(|seen| seen.other(&self.me)).collect(), ..self.lines() }
    }

    /// How this device judges a thing while `keeper` keeps the notices
    /// (its id; this device's, or "" with sharing off: this device tells):
    /// `since`, when that keeper took them (the later of its claim's and
    /// this process's word); `entry`, its entry as the folder holds it now
    /// (`devices::one`), none when it does not read; `settings`, whether
    /// this device shares the part "settings" (its hours are then the
    /// others'). Never lost comes before never doubled: what it cannot be
    /// sure the keeper tells, it keeps. Sure only by the very copy this
    /// device acts on (`acts_on`, kept from `peers.json`): a copy put back
    /// older in the folder, or a newer one not yet read here, is no
    /// knowledge, since the keeper waits for this device by the copy this
    /// device says it acts on (review of phase 2, N7).
    pub fn judge(&self, keeper: &str, since: i64, entry: Option<&Entry>, settings: bool) -> Judged {
        let marks = self.some_said() || !self.older.is_empty();
        if keeper.is_empty() || keeper == self.me {
            return Judged { lines: self.lines(), sure: Sure::Mine, marks };
        }
        let Some(entry) = entry else { return Judged { lines: Lines::default(), sure: Sure::Keep, marks: true } };
        if entry.areas < crate::devices::AREAS {
            // An older Sioul, as this device knows it too; else no knowledge.
            let sure = if self.older.iter().any(|id| id == keeper) { Sure::Since(since) } else { Sure::Keep };
            return Judged { lines: Lines::default(), sure, marks: true };
        }
        // The copy read is the one this device acts on, or nothing is sure.
        let acknowledged = self.others.get(keeper).is_some_and(|seen| Cells::key(&seen.cells) == Cells::key(&entry.cells));
        if entry.cells.is_empty() {
            // It narrows nothing: it tells what the hours bring, in its own hours, and never waits for another device.
            return Judged { lines: Lines::default(), sure: if acknowledged { Sure::Told } else { Sure::Keep }, marks };
        }
        // Its own cells, nothing widened by what no device holds: an area it brings now, it tells.
        let lines = Lines { mine: Cells::read(&entry.cells), key: Some(Cells::key(&entry.cells)), others: vec![Other { cells: Cells::default(), acts_on: None }] };
        let sure = if acknowledged && settings && shares_settings(entry) { Sure::Told } else { Sure::Keep };
        Judged { lines, sure, marks: true }
    }

    /// What this device says in its entry of the others' cells it acts on
    /// (`devices::Entry::known`): each one's key, as its entry says them.
    pub fn acts_on(&self) -> BTreeMap<String, String> {
        self.others.iter().map(|(id, seen)| (id.clone(), Cells::key(&seen.cells))).collect()
    }

    /// Whether the device `id` counts and follows what devices are for (this one always does).
    pub fn follows(&self, id: &str) -> bool {
        id == self.me || self.others.contains_key(id)
    }

    /// Whether anything is said of any device that counts: this one's line,
    /// or another's cells.
    pub fn some_said(&self) -> bool {
        !self.mine.says_nothing() || self.others.values().any(|seen| seen.read().narrows())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::AREAS;
    use crate::peers::{Mark, PeerSeen};
    use sioul_core::areas::{Area, Time, Week};
    use sioul_core::attention::{Attention, Event, Lane, Now, Source};
    use sioul_core::health::{Peer, Said as Session};
    use sioul_core::quiet::{Mode, Reason};
    use sioul_core::reach::Who;
    use std::collections::BTreeSet;

    fn said(pairs: &[(&str, &str)]) -> BTreeMap<String, Said> {
        pairs.iter().map(|(id, area)| (id.to_string(), Said { area: Some(area.to_string()), ..Said::default() })).collect()
    }

    /// A device as kept here: heard at `heard`, its entry saying `areas`.
    pub(crate) fn seen(id: &str, heard: i64, areas: u32) -> PeerSeen {
        let session = Session { started: heard - 600, closed: 0, working: true, exported: heard, imported: heard, doses: true, left: false, phone: false };
        PeerSeen { peer: Peer { id: id.into(), name: id.into(), heard, seen: heard, said: Some(session), ..Peer::default() }, entry: Some(Entry { id: id.into(), working: true, exported: heard, areas, ..Entry::default() }), ..PeerSeen::default() }
    }

    fn hours() -> Week {
        Week { work_hours: true, admin_hours: true, meals: true, sleep: true }
    }

    /// The devices that count: this one, always; another while it is heard
    /// within a week, not said off, not forgotten, still sharing; one whose
    /// Sioul is older apart. With sharing off, this device alone. The
    /// others' cells are read from their entries, never from the settings.
    #[test]
    fn the_devices_that_count_are_those_the_doses_count() {
        let now = 1_800_000_000;
        let week = 7 * 86_400;
        let said = said(&[("desk", "work"), ("phone", "admin+leisure"), ("old", "leisure"), ("gone", "leisure")]);
        let mut kept = Peers::default();
        kept.peers.insert("phone".into(), seen("phone", now - 60, 1));
        kept.peers.insert("old".into(), seen("old", now - 60, 0));
        kept.peers.insert("gone".into(), seen("gone", now - week - 3600, 1));
        kept.peers.insert("off".into(), PeerSeen { peer: Peer { off: true, ..seen("off", now - 60, 1).peer }, ..seen("off", now - 60, 1) });
        kept.peers.insert("forgotten".into(), PeerSeen { forgotten: Some(Mark::default()), ..seen("forgotten", now - 60, 1) });
        let left = seen("left", now - 60, 1);
        kept.peers.insert("left".into(), PeerSeen { peer: Peer { said: left.peer.said.clone().map(|s| Session { left: true, ..s }), ..left.peer.clone() }, ..left });
        let known = Known::of("desk", &said, Some(&kept), now);
        assert_eq!(known.others.keys().map(String::as_str).collect::<Vec<_>>(), ["phone"]);
        assert_eq!((known.older.clone(), known.mine), (vec!["old".to_string()], Line::for_area(Area::WORK)));
        assert!(known.follows("phone") && known.follows("desk") && !known.follows("old") && !known.follows("gone"));
        // The phone's entry says nothing of its cells: it brings what the hours bring, whatever the
        // settings say of it here; it holds nothing for later until it says it acts on the desk's cells.
        let hours = hours();
        assert!(!Cells::read(&known.others["phone"].cells).narrows());
        assert_eq!(known.lines().here(hours).unheld, Area::PERSONAL, "it acts on no word of the desk yet");
        let desk_key = Cells::key(&Cells::Line(known.mine).words(hours));
        kept.peers.get_mut("phone").unwrap().entry.as_mut().unwrap().known = [("desk".to_string(), desk_key)].into();
        let known = Known::of("desk", &said, Some(&kept), now);
        assert_eq!(known.lines().here(hours).unheld, Area::default(), "it acts on the desk's cells: it holds what the hours bring");
        // The phone silent a week: only the desk counts, and what the desk is not for is held by no device.
        let later = Known::of("desk", &said, Some(&kept), now + week + 3600);
        assert!(later.others.is_empty());
        assert_eq!(later.lines().here(hours).unheld, Area::PERSONAL);
        // Sharing off: this device alone, its own line still read.
        let alone = Known::of("desk", &said, None, now);
        assert_eq!((alone.others.len(), alone.mine, alone.older.len()), (0, Line::for_area(Area::WORK), 0));
        // Nothing said of anyone: lines that say nothing.
        assert!(Known::of("desk", &BTreeMap::new(), Some(&kept), now).lines().say_nothing());
        assert!(!Known::of("desk", &BTreeMap::new(), Some(&kept), now).some_said() && known.some_said());
    }

    /// How a device that does not keep the notices judges the keeper, from
    /// the keeper's entry as the folder holds it: its own cells, nothing
    /// widened, sure only in the same hours (both share the settings) and
    /// only by the very copy this device acts on (review of phase 2, N7); one
    /// that narrows nothing tells what the hours bring; an older Sioul, only
    /// what arrived well after it took the notices; an entry that does not
    /// read, nothing known: keep. Nothing said and every device on this
    /// build: the keeper is no part of the moment, as before devices.
    #[test]
    fn the_keeper_is_judged_as_its_entry_says() {
        let now = 1_800_000_000;
        let week = hours();
        let said: BTreeMap<String, Said> = [("phone".to_string(), Said { area: Some("admin+leisure".into()), ..Said::default() })].into();
        let desk = Entry { id: "desk".into(), working: true, areas: AREAS, cells: Cells::Line(Line::for_area(Area::WORK)).words(week), parts: Some(vec!["settings".into()]), ..Entry::default() };
        // What the phone kept of the desk (`peers.json`): the copy it acts on.
        let knowing = |entry: &Entry| {
            let mut kept = Peers::default();
            let mut peer = seen("desk", now - 60, entry.areas);
            peer.entry = Some(entry.clone());
            kept.peers.insert("desk".into(), peer);
            Known::of("phone", &said, Some(&kept), now)
        };
        let known = knowing(&desk);
        let judged = known.judge("desk", 100, Some(&desk), true);
        assert_eq!(judged.sure, Sure::Told);
        let here = judged.lines.here(week);
        assert!(here.brings(Area::WORK, Time::Work, week) && !here.brings(Area::LEISURE, Time::Leisure, week) && here.unheld.is_empty(), "its own cells, nothing widened");
        assert!(judged.marks);
        // Hours that may differ: keep.
        assert_eq!(known.judge("desk", 100, Some(&desk), false).sure, Sure::Keep);
        assert_eq!(known.judge("desk", 100, Some(&Entry { parts: Some(Vec::new()), ..desk.clone() }), true).sure, Sure::Keep);
        // A copy other than the one acted on (an older one put back, a newer one not read here yet): keep.
        let wider = Entry { cells: Cells::Line(Line::for_area(Area::WORK.with(Area::LEISURE))).words(week), ..desk.clone() };
        let bare = Entry { cells: BTreeMap::new(), ..desk.clone() };
        assert_eq!((known.judge("desk", 100, Some(&wider), true).sure, known.judge("desk", 100, Some(&bare), true).sure), (Sure::Keep, Sure::Keep));
        assert_eq!(Known::of("phone", &said, None, now).judge("desk", 100, Some(&desk), true).sure, Sure::Keep, "a keeper not known here");
        // Nothing said of the keeper, as this device knows it too: what the hours bring.
        let plain = knowing(&bare).judge("desk", 100, Some(&bare), false);
        assert_eq!((plain.sure, plain.lines.say_nothing()), (Sure::Told, true));
        // An older Sioul, as this device knows it too: what arrived well after it took the notices.
        let older = Entry { areas: 0, cells: BTreeMap::new(), ..desk.clone() };
        assert_eq!(knowing(&older).judge("desk", 100, Some(&older), true).sure, Sure::Since(100));
        assert_eq!(known.judge("desk", 100, Some(&older), true).sure, Sure::Keep, "an older copy put back: no knowledge");
        // Its entry does not read: keep.
        assert_eq!(known.judge("desk", 100, None, true).sure, Sure::Keep);
        // This device keeps them.
        assert_eq!(known.judge("phone", 100, None, true), Judged { lines: known.lines(), sure: Sure::Mine, marks: true });
        assert_eq!(known.judge("", 100, None, true).sure, Sure::Mine);
        // What may be claimed: after an older keeper took the notices, only by more than the two clocks' margin.
        assert!(Sure::Told.holds(0) && Sure::Mine.holds(0) && Sure::Since(100).holds(100 + CLOCKS) && !Sure::Since(100).holds(100 + CLOCKS - 1) && !Sure::Since(100).holds(50) && !Sure::Since(100).holds(0) && !Sure::Keep.holds(150));
        // Nothing said of any device, every device on this build: no part of the moment.
        let nothing = Known::of("phone", &BTreeMap::new(), None, now);
        assert!(!nothing.judge("desk", 100, Some(&bare), true).marks && !nothing.judge("phone", 100, None, true).marks);
        assert!(nothing.judge("desk", 100, Some(&older), true).marks, "an older keeper is");
    }

    /// A line said for a device that is off (review of phase 2, N3): the
    /// keeper reads it by the settings once the line was said after the
    /// device last said its own and the device shares the settings, since it
    /// will follow it when it runs; by its own entry otherwise.
    #[test]
    fn a_line_said_for_an_idle_device_is_read_once_it_will_follow_it() {
        let now = 1_800_000_000;
        let week = hours();
        let tablet = |parts: Vec<String>, stated: i64| {
            let mut kept = Peers::default();
            let mut seen = seen("tablet", now - 3 * 86_400, 1);
            let entry = seen.entry.as_mut().unwrap();
            entry.cells = Cells::Line(Line::for_area(Area::LEISURE)).words(week);
            entry.known = [("desk".to_string(), Cells::key(&Cells::Line(Line::for_area(Area::WORK)).words(week)))].into();
            entry.parts = Some(parts);
            entry.stated = stated;
            kept.peers.insert("tablet".into(), seen);
            kept
        };
        let said = |changed: i64| -> BTreeMap<String, Said> {
            [("desk".to_string(), Said { area: Some("work".into()), ..Said::default() }), ("tablet".to_string(), Said { area: Some("work".into()), changed: Some(changed), ..Said::default() })].into()
        };
        let leisure_here = |kept: &Peers, said: &BTreeMap<String, Said>| Known::of("desk", said, Some(kept), now).lines().here(week).brings(Area::LEISURE, Time::Leisure, week);
        // Said later, and it shares the settings: no longer for leisure, so leisure is for no device and the desk tells it.
        assert!(leisure_here(&tablet(vec!["settings".into()], now - 3 * 86_400), &said(now - 60)));
        // It does not share the settings, or the line is older than what it said of itself: its entry, leisure held by it.
        assert!(!leisure_here(&tablet(Vec::new(), now - 3 * 86_400), &said(now - 60)));
        assert!(!leisure_here(&tablet(vec!["settings".into()], now - 60), &said(now - 3 * 86_400)));
    }

    /// One device of the model: its own line (its settings), whether its
    /// Sioul is older, whether it shares the part "settings", the others'
    /// entries as it last read them, its ledger, and what it told.
    struct Device {
        id: &'static str,
        line: Line,
        form: &'static str,
        older: bool,
        settings: bool,
        read: BTreeMap<String, Entry>,
        waiting: BTreeSet<&'static str>,
        told: Vec<&'static str>,
    }

    /// Several devices that share: the keeper of the notices and since when,
    /// when each thing arrived, and the told logs written so far.
    struct Model {
        devices: Vec<Device>,
        week: Week,
        keeper: &'static str,
        since: i64,
        clock: i64,
        arrived: BTreeMap<&'static str, i64>,
        /// What the address each message came to is for.
        areas: BTreeMap<&'static str, Area>,
        told_log: BTreeMap<&'static str, &'static str>,
        /// A copy of an entry the folder holds in place of the one its device writes (a sync app put an older one back).
        folder: BTreeMap<&'static str, Entry>,
    }

    const NOW: i64 = 1_800_000_000;

    fn said_of(line: Line) -> Said {
        let at = Time::STATES.into_iter().filter_map(|time| line.changed(time).map(|cell| (time.id().to_string(), Line::cell_id(cell)))).collect();
        Said { area: line.area.map(Area::id), at, ..Said::default() }
    }

    impl Model {
        /// The devices, the first keeping the notices, every entry read by every device.
        fn new(lines: &[(&'static str, Line, &'static str)]) -> Model {
            let devices = lines.iter().map(|(id, line, form)| Device { id, line: *line, form, older: false, settings: true, read: BTreeMap::new(), waiting: Default::default(), told: Vec::new() }).collect();
            let mut model = Model { devices, week: hours(), keeper: lines[0].0, since: 1, clock: 10, arrived: BTreeMap::new(), areas: BTreeMap::new(), told_log: BTreeMap::new(), folder: BTreeMap::new() };
            model.exchange_all();
            model.exchange_all();
            model
        }

        /// A device changed (older, settings), every entry then read again by every device.
        fn with(mut self, id: &str, change: impl FnOnce(&mut Device)) -> Model {
            change(self.devices.iter_mut().find(|d| d.id == id).unwrap());
            self.exchange_all();
            self.exchange_all();
            self
        }

        fn device(&self, id: &str) -> &Device {
            self.devices.iter().find(|d| d.id == id).unwrap()
        }

        fn device_mut(&mut self, id: &str) -> &mut Device {
            self.devices.iter_mut().find(|d| d.id == id).unwrap()
        }

        fn known(&self, id: &str) -> Known {
            let me = self.device(id);
            let mut kept = Peers::default();
            for (other, entry) in &me.read {
                let mut peer = seen(other, NOW - 60, entry.areas);
                peer.entry = Some(entry.clone());
                kept.peers.insert(other.clone(), peer);
            }
            let said: BTreeMap<String, Said> = [(id.to_string(), said_of(me.line))].into();
            Known::of(id, &said, Some(&kept), NOW)
        }

        /// The entry a device writes now (an older Sioul's: no word of devices).
        fn entry(&self, id: &str) -> Entry {
            let me = self.device(id);
            if me.older {
                return Entry { id: id.into(), working: true, form: me.form.into(), ..Entry::default() };
            }
            let parts = if me.settings { vec!["settings".to_string()] } else { Vec::new() };
            Entry { id: id.into(), working: true, areas: AREAS, form: me.form.into(), parts: Some(parts), cells: Cells::Line(me.line).words(self.week), known: self.known(id).acts_on(), ..Entry::default() }
        }

        /// `reader` reads the entry `of` writes now.
        fn exchange(&mut self, reader: &str, of: &str) {
            let entry = self.entry(of);
            self.device_mut(reader).read.insert(of.to_string(), entry);
        }

        fn exchange_all(&mut self) {
            let ids: Vec<&'static str> = self.devices.iter().map(|d| d.id).collect();
            for reader in &ids {
                for of in &ids {
                    if reader != of {
                        self.exchange(reader, of);
                    }
                }
            }
        }

        /// The person uses `id`: it keeps the notices from now.
        fn take(&mut self, id: &'static str) {
            self.keeper = id;
            self.since = self.clock;
            self.clock += 1;
        }

        /// `message` reaches its server now.
        fn arrive(&mut self, message: &'static str) {
            self.arrived.insert(message, self.clock);
            self.clock += 1;
        }

        /// `message` reaches its server now, its time set by a server clock `ahead` of the devices'.
        fn arrive_ahead(&mut self, message: &'static str, ahead: i64) {
            self.arrived.insert(message, self.clock + ahead);
            self.clock += 1;
        }

        /// Time passes.
        fn later(&mut self, seconds: i64) {
            self.clock += seconds;
        }

        /// The keeper's entry as the folder holds it: the one it writes, or a copy put back.
        fn in_folder(&self, id: &'static str) -> Entry {
            self.folder.get(id).cloned().unwrap_or_else(|| self.entry(id))
        }

        /// Whether `id` finds a safe sender's mail to an address for `area`
        /// told now (in `time`, its own hours), and whether it may claim it:
        /// judged as the keeper does, from the keeper's entry as the folder
        /// holds it now (`Known::judge`). An older Sioul judges by the hours
        /// and claims whatever they tell (main's rule).
        fn told_now(&self, id: &str, area: Area, time: Time) -> (bool, Sure) {
            let me = self.device(id);
            let (lines, sure) = if me.older {
                (Lines::default(), if self.keeper == id { Sure::Mine } else { Sure::Told })
            } else {
                let judged = self.known(id).judge(self.keeper, self.since, Some(&self.in_folder(self.keeper)), me.settings);
                (judged.lines, judged.sure)
            };
            let reason = match time {
                Time::Work => Reason::Working,
                Time::Admin => Reason::AdminTime,
                _ => Reason::Evening,
            };
            let mode = Mode { quiet: time != Time::Work, time, week: self.week, reason, until: None, back: None, label: String::new(), here: Default::default() }.on(&lines);
            let event = Event::of(Source::Mail { who: Who::Safe, always: false, lane: Lane::Usual }).for_area(Some(area));
            (Attention::usual().decide(&event, &Now::of(&mode)).told, sure)
        }

        /// Whether `id` reads in a told log that another device told `message`:
        /// the log travels with the part "settings", on both devices.
        fn in_log(&self, id: &str, message: &str) -> bool {
            self.told_log.get(message).is_some_and(|by| *by != id && self.device(by).settings && self.device(id).settings)
        }

        fn tell(&mut self, id: &'static str, message: &'static str) {
            self.device_mut(id).told.push(message);
            self.told_log.insert(message, id);
        }

        /// A fetch on `id` of `message`: the keeper tells what it finds told
        /// now; another device claims it without a word only when sure; the
        /// rest waits in its ledger.
        fn fetch(&mut self, id: &'static str, message: &'static str, area: Area, time: Time) {
            self.areas.insert(message, area);
            let (told, sure) = self.told_now(id, area, time);
            if told && self.keeper == id {
                self.tell(id, message);
            } else if !(told && sure.holds(self.arrived[message])) {
                self.device_mut(id).waiting.insert(message);
            }
        }

        /// A moment begins on `id` (a time, the keeper changing): what waited
        /// and another device's log says was told is claimed; what is told
        /// now is told by the keeper, claimed by another device when sure.
        fn tick(&mut self, id: &'static str, time: Time) {
            let waiting: Vec<&'static str> = self.device(id).waiting.iter().copied().collect();
            for message in waiting {
                if self.in_log(id, message) {
                    self.device_mut(id).waiting.remove(message);
                    continue;
                }
                let (told, sure) = self.told_now(id, self.areas[message], time);
                if told && self.keeper == id {
                    self.device_mut(id).waiting.remove(message);
                    self.tell(id, message);
                } else if told && sure.holds(self.arrived[message]) {
                    self.device_mut(id).waiting.remove(message);
                }
            }
        }

        fn tells(&self, message: &str) -> usize {
            self.devices.iter().flat_map(|d| &d.told).filter(|m| **m == message).count()
        }

        fn waits(&self, message: &str) -> bool {
            self.devices.iter().any(|d| d.waiting.contains(message))
        }
    }

    /// A desk that does not share the settings (review of phase 2, H1): the
    /// person said on the phone that the desk is for work and the phone for
    /// admin and leisure; the desk's own settings say nothing of any device.
    /// Work's mail during work, the phone keeping the notices, waits on
    /// both, and the desk tells it once used; a friend's in the evening, the
    /// desk keeping them, is told by the desk, which narrows nothing, and
    /// claimed by the phone. Each once.
    #[test]
    fn a_desk_that_does_not_share_the_settings_and_a_phone_tell_each_thing_once() {
        let mut m = Model::new(&[("desk", Line::default(), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")]).with("desk", |d| d.settings = false);
        m.take("phone");
        m.arrive("work-mail");
        m.fetch("phone", "work-mail", Area::WORK, Time::Work);
        m.fetch("desk", "work-mail", Area::WORK, Time::Work);
        assert!(m.tells("work-mail") == 0 && m.device("phone").waiting.contains("work-mail") && m.device("desk").waiting.contains("work-mail"), "waits on both");
        m.take("desk");
        m.tick("desk", Time::Work);
        m.tick("phone", Time::Work);
        assert_eq!((m.tells("work-mail"), m.waits("work-mail")), (1, false));
        m.arrive("friend");
        m.fetch("desk", "friend", Area::PERSONAL, Time::Leisure);
        m.fetch("phone", "friend", Area::PERSONAL, Time::Leisure);
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false));
    }

    /// A line still travelling (H1): the desk's line reaches the desk before
    /// the phone acts on it. The desk counts the phone as holding nothing
    /// and tells the evening's mail itself; the phone, reading the desk's
    /// entry as the folder holds it, is not sure and keeps it, and claims it
    /// by the desk's told log once it keeps the notices. Once the phone acts
    /// on the desk's cells, the desk leaves leisure for it, and it tells it.
    #[test]
    fn a_line_still_travelling_never_loses_a_thing() {
        let mut m = Model::new(&[("desk", Line::default(), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")]);
        m.device_mut("desk").line = Line::for_area(Area::WORK);
        m.exchange("desk", "phone");
        m.arrive("friend");
        m.fetch("desk", "friend", Area::PERSONAL, Time::Leisure);
        m.fetch("phone", "friend", Area::PERSONAL, Time::Leisure);
        assert_eq!(m.tells("friend"), 1, "the desk tells it");
        assert!(m.device("phone").waiting.contains("friend"), "kept, never claimed on a doubt");
        m.take("phone");
        m.tick("phone", Time::Leisure);
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false), "its told log: claimed");
        // The phone acts on the desk's cells, and the desk reads that: leisure waits for it.
        m.exchange_all();
        m.exchange_all();
        m.take("desk");
        m.arrive("later");
        m.fetch("desk", "later", Area::PERSONAL, Time::Leisure);
        m.fetch("phone", "later", Area::PERSONAL, Time::Leisure);
        assert_eq!(m.tells("later"), 0);
        m.take("phone");
        m.tick("phone", Time::Leisure);
        m.tick("desk", Time::Leisure);
        assert_eq!((m.tells("later"), m.waits("later")), (1, false));
    }

    /// Stale peers (M3): the phone holds the desk's entry from before its
    /// update, or none. It reads the keeper's entry as the folder holds it,
    /// so it keeps the friend's mail; acting on no cells of the desk, it is
    /// not waited for, and the desk tells it. Once, never lost.
    #[test]
    fn stale_peers_never_lose_a_thing() {
        for stale in [true, false] {
            let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")]);
            let phone = m.device_mut("phone");
            if stale {
                phone.read.get_mut("desk").unwrap().areas = 0;
            } else {
                phone.read.remove("desk");
            }
            m.exchange("desk", "phone");
            m.arrive("friend");
            m.fetch("desk", "friend", Area::PERSONAL, Time::Leisure);
            m.fetch("phone", "friend", Area::PERSONAL, Time::Leisure);
            m.take("phone");
            m.tick("phone", Time::Leisure);
            assert_eq!((m.tells("friend"), m.waits("friend")), (1, false), "stale: {stale}");
        }
    }

    /// Two devices put away (M4): the desk for work keeps the notices; a
    /// phone and a tablet for leisure both keep a friend's mail. The first
    /// used tells it; the other and the desk claim it. Once.
    #[test]
    fn two_devices_put_away_tell_a_thing_once() {
        let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::LEISURE), "phone"), ("tablet", Line::for_area(Area::LEISURE), "tablet")]);
        m.arrive("friend");
        for id in ["desk", "phone", "tablet"] {
            m.fetch(id, "friend", Area::LEISURE, Time::Leisure);
        }
        assert!(m.tells("friend") == 0 && ["desk", "phone", "tablet"].iter().all(|id| m.device(id).waiting.contains("friend")));
        m.take("phone");
        for id in ["phone", "tablet", "desk"] {
            m.tick(id, Time::Leisure);
        }
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false));
    }

    /// A time boundary between two fetches (M4): the desk, for work and
    /// admin, tells mail to the personal address in admin's last seconds;
    /// the phone, in leisure's first, keeps it, and claims it by the desk's
    /// told log once it keeps the notices. Without the log, it tells it
    /// again: at worst twice, never lost.
    #[test]
    fn a_time_boundary_between_two_fetches_tells_a_thing_once_with_the_told_log() {
        for log in [true, false] {
            let mut m = Model::new(&[("desk", Line::for_area(Area::WORK.with(Area::ADMIN)), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")]);
            m.arrive("boundary");
            m.fetch("desk", "boundary", Area::PERSONAL, Time::Admin);
            m.fetch("phone", "boundary", Area::PERSONAL, Time::Leisure);
            assert!(m.tells("boundary") == 1 && m.device("phone").waiting.contains("boundary"), "kept, never lost");
            if !log {
                m.told_log.clear();
            }
            m.take("phone");
            m.tick("phone", Time::Leisure);
            assert_eq!((m.tells("boundary"), m.waits("boundary")), (if log { 1 } else { 2 }, false), "told log: {log}");
        }
    }

    /// An older Sioul takes the notices while a friend's mail waits for the
    /// phone (review of phase 2, N1): it claimed the mail at its fetch, as an
    /// older Sioul claims everything while another device tells. The phone
    /// and the desk are not sure (it arrived before the older laptop took
    /// the notices) and keep it; the phone tells it once used. What arrives
    /// while the older laptop keeps the notices, it tells, and the others
    /// claim. Each once.
    #[test]
    fn an_older_keeper_never_loses_what_waited() {
        let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone"), ("old", Line::default(), "computer")]).with("old", |d| d.older = true);
        assert!(m.known("desk").older == ["old"] && m.known("phone").older == ["old"]);
        m.arrive("friend");
        for id in ["desk", "phone", "old"] {
            m.fetch(id, "friend", Area::PERSONAL, Time::Leisure);
        }
        assert!(m.device("desk").waiting.contains("friend") && m.device("phone").waiting.contains("friend") && !m.device("old").waiting.contains("friend"), "the older laptop claimed it");
        m.take("old");
        for id in ["phone", "desk", "old"] {
            m.tick(id, Time::Leisure);
        }
        assert!(m.tells("friend") == 0 && m.device("phone").waiting.contains("friend"), "kept, never claimed on the older keeper's word");
        // What arrives while it keeps the notices, well past the clocks' margin: it tells, the others claim.
        m.later(CLOCKS + 60);
        m.arrive("while-old");
        for id in ["old", "desk", "phone"] {
            m.fetch(id, "while-old", Area::PERSONAL, Time::Leisure);
        }
        assert_eq!((m.tells("while-old"), m.waits("while-old")), (1, false));
        // The person picks up the phone: it tells what waited.
        m.take("phone");
        m.tick("phone", Time::Leisure);
        m.tick("desk", Time::Leisure);
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false));
    }

    /// A laptop on this build with a stale copy of the desk's entry (N1):
    /// it reads the desk's entry as the folder holds it, so it does not
    /// claim what the desk leaves for the phone; opened, it keeps the
    /// notices and tells it, and the others claim it. Once.
    #[test]
    fn a_stale_device_that_becomes_keeper_tells_what_waited() {
        let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone"), ("laptop", Line::default(), "computer")]);
        m.device_mut("laptop").read.get_mut("desk").unwrap().areas = 0;
        m.exchange("desk", "laptop");
        m.exchange("phone", "laptop");
        m.arrive("friend");
        for id in ["desk", "phone", "laptop"] {
            m.fetch(id, "friend", Area::PERSONAL, Time::Leisure);
        }
        assert!(["desk", "phone", "laptop"].iter().all(|id| m.device(id).waiting.contains("friend")), "nobody claims it on a doubt");
        m.take("laptop");
        for id in ["laptop", "phone", "desk"] {
            m.tick(id, Time::Leisure);
        }
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false));
    }

    /// The lease moves to a device that claimed something at its fetch: it
    /// claimed it only because the keeper told it by its own cells, so
    /// nothing waits for it, and nothing is told again.
    #[test]
    fn the_lease_moving_to_a_device_that_claimed_tells_nothing_twice() {
        let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::ALL), "phone"), ("tablet", Line::for_area(Area::LEISURE), "tablet")]);
        m.arrive("work-mail");
        m.arrive("friend");
        for id in ["desk", "phone", "tablet"] {
            m.fetch(id, "work-mail", Area::WORK, Time::Work);
            m.fetch(id, "friend", Area::LEISURE, Time::Leisure);
        }
        assert!(m.tells("work-mail") == 1 && !m.device("phone").waiting.contains("work-mail"), "the desk told it, the phone claimed it");
        m.take("phone");
        for id in ["phone", "tablet", "desk"] {
            m.tick(id, Time::Work);
            m.tick(id, Time::Leisure);
        }
        assert_eq!((m.tells("work-mail"), m.waits("work-mail"), m.tells("friend"), m.waits("friend")), (1, false, 1, false));
    }

    /// A phone that does not share the settings (review of phase 2, N2):
    /// its hours run an hour later, and the desk's told log never reaches
    /// it. The desk tells a friend's mail in its leisure; the phone, not
    /// sure in hours that may not be the desk's, keeps it and tells it again
    /// once it keeps the notices: at worst twice, never lost. With the
    /// desk's work cell changed to bring leisure, the phone, in its own
    /// working hours, would read the desk as telling what the desk in fact
    /// leaves for it: it keeps it all the same, and tells it once.
    #[test]
    fn hours_that_may_differ_tell_at_worst_twice_and_lose_nothing() {
        let mut m = Model::new(&[("desk", Line::for_area(Area::WORK.with(Area::LEISURE)), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")]).with("phone", |d| d.settings = false);
        m.arrive("friend");
        m.fetch("desk", "friend", Area::LEISURE, Time::Leisure);
        m.fetch("phone", "friend", Area::LEISURE, Time::Work);
        assert!(m.tells("friend") == 1 && m.device("phone").waiting.contains("friend"));
        m.take("phone");
        m.tick("phone", Time::Leisure);
        assert_eq!(m.tells("friend"), 2, "at worst twice");
        let friends_at_work = Line::for_area(Area::WORK).with_cell(Time::Work, Some(Area::WORK.with(Area::LEISURE)));
        let mut m = Model::new(&[("desk", friends_at_work, "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")]).with("phone", |d| d.settings = false);
        m.arrive("friend");
        m.fetch("desk", "friend", Area::LEISURE, Time::Leisure);
        m.fetch("phone", "friend", Area::LEISURE, Time::Work);
        assert!(m.tells("friend") == 0 && m.device("phone").waiting.contains("friend"), "left for the phone, kept there");
        m.take("phone");
        m.tick("phone", Time::Leisure);
        assert_eq!(m.tells("friend"), 1, "never lost");
    }

    /// An older copy of the keeper's entry put back in the folder by a sync
    /// app (review of phase 2, N7): the phone acts on the desk's current
    /// cells, so the desk leaves a friend's mail for it; judged by the copy
    /// put back (no cells, or work and leisure), the phone would be sure the
    /// desk tells it. That copy is not the one the phone acts on: it keeps
    /// the mail, and tells it once it keeps the notices. Once.
    #[test]
    fn an_older_keeper_entry_put_back_is_never_taken_as_sure() {
        let week = hours();
        let olders = [BTreeMap::new(), Cells::Line(Line::for_area(Area::WORK.with(Area::LEISURE))).words(week)];
        for cells in olders {
            let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")]);
            let put_back = Entry { cells: cells.clone(), ..m.entry("desk") };
            m.folder.insert("desk", put_back);
            m.arrive("friend");
            m.fetch("desk", "friend", Area::PERSONAL, Time::Leisure);
            m.fetch("phone", "friend", Area::PERSONAL, Time::Leisure);
            assert!(m.tells("friend") == 0 && m.device("phone").waiting.contains("friend"), "{cells:?}: kept on the phone");
            m.take("phone");
            m.tick("phone", Time::Leisure);
            m.tick("desk", Time::Leisure);
            assert_eq!((m.tells("friend"), m.waits("friend")), (1, false), "{cells:?}");
        }
    }

    /// A server's clock ahead of the devices' (the low note on `Since`): a
    /// friend's mail reaches its server just before an older laptop takes
    /// the notices, stamped five minutes later than it was. The older laptop
    /// claimed it at its fetch; the phone and the desk, within the clocks'
    /// margin of its taking the notices, keep it, and the phone tells it once
    /// used. Once.
    #[test]
    fn a_server_clock_ahead_never_turns_a_kept_thing_into_a_claimed_one() {
        let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone"), ("old", Line::default(), "computer")]).with("old", |d| d.older = true);
        m.arrive_ahead("friend", 5 * 60);
        for id in ["desk", "phone", "old"] {
            m.fetch(id, "friend", Area::PERSONAL, Time::Leisure);
        }
        m.take("old");
        for id in ["phone", "desk"] {
            m.tick(id, Time::Leisure);
        }
        assert!(m.tells("friend") == 0 && m.device("phone").waiting.contains("friend"), "kept, its time within the margin");
        m.take("phone");
        m.tick("phone", Time::Leisure);
        m.tick("desk", Time::Leisure);
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false));
    }
}

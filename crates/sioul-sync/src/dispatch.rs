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
//! forward on is the one used. Every other device reads each thing as the
//! keeper would, from the cells the keeper says of itself: what the keeper
//! tells now, it claims without a word; what the keeper leaves waiting, it
//! keeps waiting, to tell once it keeps the notices itself.
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
    /// The cells it acts on of each other device (`devices::Entry::known`).
    pub known: BTreeMap<String, String>,
    /// A phone: it tells no date reminder (its form, `devices::Entry::form_or_kind`).
    pub carried: bool,
}

impl Seen {
    fn other(&self, of: &str) -> Other {
        Other { cells: Cells::read(&self.cells), acts_on: Some(self.known.get(of).cloned().unwrap_or_default()) }
    }
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

impl Known {
    /// From what this device's settings say of it (`said`, by id) and what
    /// it kept of the others (`kept`; none with sharing off), at `now`
    /// (this device's clock, Unix seconds).
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
                    known.others.insert(id.clone(), Seen { cells: entry.cells.clone(), known: entry.known.clone(), carried });
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

    /// The keeper of the notices (`keeper`, its id) read as it reads
    /// itself, among the others that act on its cells, this device among
    /// them with its own line: what it tells now, this device claims
    /// without a word. This device itself: `lines`. A device whose Sioul is
    /// older, or that does not count here, or that this device knows
    /// nothing of: what the hours bring, all of it, as before devices.
    pub fn as_keeper(&self, keeper: &str) -> Lines {
        if keeper == self.me || keeper.is_empty() {
            return self.lines();
        }
        let Some(seen) = self.others.get(keeper) else { return Lines::default() };
        let mut others = vec![Other { cells: Cells::Line(self.mine), acts_on: None }];
        others.extend(self.others.iter().filter(|(id, _)| id.as_str() != keeper).map(|(_, other)| other.other(keeper)));
        Lines { mine: Cells::read(&seen.cells), key: Some(Cells::key(&seen.cells)), others }
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
        !self.mine.says_nothing() || self.others.values().any(|seen| !seen.cells.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::Entry;
    use crate::peers::{Mark, PeerSeen};
    use sioul_core::areas::{Area, Time, Week};
    use sioul_core::attention::{Attention, Event, Lane, Now, Source};
    use sioul_core::reach::Who;
    use sioul_core::health::{Peer, Said as Session};
    use sioul_core::quiet::{Mode, Reason};

    fn said(pairs: &[(&str, &str)]) -> BTreeMap<String, Said> {
        pairs.iter().map(|(id, area)| (id.to_string(), Said { area: Some(area.to_string()), ..Said::default() })).collect()
    }

    /// A device as kept here: heard at `heard`, its entry saying `areas`.
    pub(crate) fn seen(id: &str, heard: i64, areas: u32) -> PeerSeen {
        let session = Session { started: heard - 600, closed: 0, working: true, exported: heard, imported: heard, doses: true, left: false, phone: false };
        PeerSeen { peer: Peer { id: id.into(), name: id.into(), heard, seen: heard, said: Some(session), ..Peer::default() }, entry: Some(Entry { id: id.into(), working: true, exported: heard, areas, ..Entry::default() }), ..PeerSeen::default() }
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
        let hours = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
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

    /// Read as the keeper: its own cells, as its entry says them; the others
    /// that act on them, this device among them. A keeper whose Sioul is
    /// older, or that this device does not know: what the hours bring.
    #[test]
    fn the_keeper_is_read_as_its_entry_says() {
        let now = 1_800_000_000;
        let week = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        let desk_words = Cells::Line(Line::for_area(Area::WORK)).words(week);
        let mut kept = Peers::default();
        let mut on_desk = seen("desk", now - 60, 1);
        on_desk.entry.as_mut().unwrap().cells = desk_words.clone();
        kept.peers.insert("desk".into(), on_desk);
        kept.peers.insert("old".into(), seen("old", now - 60, 0));
        let said: BTreeMap<String, Said> = [("phone".to_string(), Said { area: Some("admin+leisure".into()), ..Said::default() })].into();
        let known = Known::of("phone", &said, Some(&kept), now);
        let keeper = known.as_keeper("desk").here(week);
        assert!(keeper.brings(Area::WORK, Time::Work, week) && !keeper.brings(Area::LEISURE, Time::Leisure, week), "the desk tells work's, leaves leisure waiting for the phone");
        assert_eq!(keeper.unheld, Area::default());
        assert_eq!(known.acts_on(), [("desk".to_string(), Cells::key(&desk_words))].into());
        // Older, or unknown: everything the hours bring.
        for other in ["old", "nobody"] {
            assert!(known.as_keeper(other).say_nothing(), "{other}");
        }
        // This device keeps them: its own line.
        assert_eq!(known.as_keeper("phone"), known.lines());
    }

    /// One device of the model: its own line (its settings), its week, the
    /// others' entries as it last read them, its ledger, and what it told.
    struct Device {
        id: &'static str,
        line: Line,
        form: &'static str,
        /// The others' entries as this device last read them, by id.
        read: BTreeMap<String, Entry>,
        waiting: std::collections::BTreeSet<&'static str>,
        told: Vec<&'static str>,
    }

    /// Several devices that share (`Model::tells` counts each thing told).
    struct Model {
        devices: Vec<Device>,
        week: Week,
        /// The told logs as exchanged so far (`mailnote::told_elsewhere`): by message, who told it.
        told_log: BTreeMap<&'static str, &'static str>,
    }

    impl Model {
        fn new(lines: &[(&'static str, Line, &'static str)], week: Week) -> Model {
            let devices = lines.iter().map(|(id, line, form)| Device { id, line: *line, form, read: BTreeMap::new(), waiting: Default::default(), told: Vec::new() }).collect();
            let mut model = Model { devices, week, told_log: BTreeMap::new() };
            model.exchange_all();
            model.exchange_all();
            model
        }

        fn device(&self, id: &str) -> &Device {
            self.devices.iter().find(|d| d.id == id).unwrap()
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

        /// The entry a device writes now: its cells, what it acts on of the others.
        fn entry(&self, id: &str) -> Entry {
            let me = self.device(id);
            Entry { id: id.into(), working: true, areas: crate::devices::AREAS, form: me.form.into(), cells: Cells::Line(me.line).words(self.week), known: self.known(id).acts_on(), ..Entry::default() }
        }

        /// `reader` reads the entry `of` writes now.
        fn exchange(&mut self, reader: &str, of: &str) {
            let entry = self.entry(of);
            self.devices.iter_mut().find(|d| d.id == reader).unwrap().read.insert(of.to_string(), entry);
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

        /// Whether `id`, reading as the keeper does, finds a safe sender's mail to an address for `area` told now.
        fn told_now(&self, id: &str, keeper: &str, area: Area, time: Time) -> bool {
            let known = self.known(id);
            let lines = if keeper == id { known.lines() } else { known.as_keeper(keeper) };
            let reason = match time {
                Time::Work => Reason::Working,
                Time::Admin => Reason::AdminTime,
                _ => Reason::Evening,
            };
            let mode = Mode { quiet: time != Time::Work, time, week: self.week, reason, until: None, back: None, label: String::new(), here: Default::default() }.on(&lines);
            // A safe sender's: told where it comes forward, shown and kept elsewhere.
            let event = Event::of(Source::Mail { who: Who::Safe, always: false, lane: Lane::Usual }).for_area(Some(area));
            Attention::usual().decide(&event, &Now::of(&mode)).told
        }

        /// A fetch on `id` of `message`, while `keeper` keeps the notices:
        /// the keeper tells what it finds told now; the others claim it
        /// without a word; what is not told now waits in the ledger.
        fn fetch(&mut self, id: &str, keeper: &str, message: &'static str, area: Area, time: Time) {
            let told = self.told_now(id, keeper, area, time);
            let device = self.devices.iter_mut().find(|d| d.id == id).unwrap();
            match (told, keeper == id) {
                (true, true) => device.told.push(message),
                (true, false) => {}
                (false, _) => {
                    device.waiting.insert(message);
                }
            }
            if told && keeper == id {
                self.told_log.insert(message, device.id);
            }
        }

        /// A moment begins on `id` (a time, the keeper changing): what waited
        /// and is told now is told by the keeper, claimed by the others; what
        /// another device told (its log, exchanged) is claimed without a word.
        fn tick(&mut self, id: &'static str, keeper: &str, area: Area, time: Time) {
            let waiting: Vec<&'static str> = self.device(id).waiting.iter().copied().collect();
            for message in waiting {
                let told = self.told_now(id, keeper, area, time);
                let elsewhere = self.told_log.get(message).is_some_and(|by| *by != id);
                if !told {
                    continue;
                }
                let device = self.devices.iter_mut().find(|d| d.id == id).unwrap();
                device.waiting.remove(message);
                if keeper == id && !elsewhere {
                    device.told.push(message);
                    self.told_log.insert(message, id);
                }
            }
        }

        /// How many times `message` was told, all devices together.
        fn tells(&self, message: &str) -> usize {
            self.devices.iter().flat_map(|d| &d.told).filter(|m| **m == message).count()
        }

        /// Whether `message` still waits on a device it could yet be told on.
        fn waits(&self, message: &str) -> bool {
            self.devices.iter().any(|d| d.waiting.contains(message))
        }
    }

    const NOW: i64 = 1_800_000_000;

    fn said_of(line: Line) -> Said {
        let at = Time::STATES.into_iter().filter_map(|time| line.changed(time).map(|cell| (time.id().to_string(), Line::cell_id(cell)))).collect();
        Said { area: line.area.map(Area::id), at, ..Said::default() }
    }

    fn hours() -> Week {
        Week { work_hours: true, admin_hours: true, meals: true, sleep: true }
    }

    /// A desk that does not share the settings (review of phase 2, H1): the
    /// person said on the phone that the desk is for work and the phone for
    /// admin and leisure; the desk's own settings say nothing of any device,
    /// and its entry says so. Each device reads the other's entry, never the
    /// settings: work's mail during work, the phone used last, waits on both,
    /// and the desk tells it once used; a friend's in the evening, the desk
    /// used last, is told by the desk alone. Each thing once.
    #[test]
    fn a_desk_that_does_not_share_the_settings_and_a_phone_tell_each_thing_once() {
        let mut m = Model::new(&[("desk", Line::default(), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")], hours());
        m.fetch("phone", "phone", "work-mail", Area::WORK, Time::Work);
        m.fetch("desk", "phone", "work-mail", Area::WORK, Time::Work);
        assert_eq!(m.tells("work-mail"), 0);
        assert!(m.device("phone").waiting.contains("work-mail") && m.device("desk").waiting.contains("work-mail"), "waits on both");
        // The person goes to the desk: it keeps the notices, tells it; the phone, read as the desk, claims it.
        m.tick("desk", "desk", Area::WORK, Time::Work);
        m.tick("phone", "desk", Area::WORK, Time::Work);
        assert_eq!(m.tells("work-mail"), 1);
        assert!(!m.waits("work-mail"));
        // Evening, the desk used last: a friend's mail, once.
        m.fetch("desk", "desk", "friend", Area::PERSONAL, Time::Leisure);
        m.fetch("phone", "desk", "friend", Area::PERSONAL, Time::Leisure);
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false));
    }

    /// A line still travelling (H1): the desk's line, said on the phone,
    /// reaches the desk; until the phone has read the desk's new entry, it
    /// still acts on the old one, so the desk counts it as holding nothing
    /// and tells the evening's mail itself, which the phone claims on the
    /// old word. Once the phone acts on the desk's new cells, the desk
    /// leaves leisure waiting for it, and the phone tells it once used.
    #[test]
    fn a_line_still_travelling_never_loses_a_thing() {
        let mut m = Model::new(&[("desk", Line::default(), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")], hours());
        // The settings reach the desk: its own line, its entry written; the phone has not read it yet.
        m.devices[0].line = Line::for_area(Area::WORK);
        m.exchange("desk", "phone");
        m.fetch("desk", "desk", "friend", Area::PERSONAL, Time::Leisure);
        m.fetch("phone", "desk", "friend", Area::PERSONAL, Time::Leisure);
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false), "told by the desk, claimed by the phone");
        // The phone reads the desk's entry and says it acts on it; the desk reads that.
        m.exchange("phone", "desk");
        m.exchange("desk", "phone");
        m.fetch("desk", "desk", "later", Area::PERSONAL, Time::Leisure);
        m.fetch("phone", "desk", "later", Area::PERSONAL, Time::Leisure);
        assert_eq!(m.tells("later"), 0);
        assert!(m.device("phone").waiting.contains("later"), "left for the phone");
        m.tick("phone", "phone", Area::PERSONAL, Time::Leisure);
        m.tick("desk", "phone", Area::PERSONAL, Time::Leisure);
        assert_eq!(m.tells("later"), 1);
    }

    /// Stale peers (M3): the phone still holds the desk's entry from before
    /// its update, or none at all. The desk keeps the notices; the phone,
    /// put away, fetches a friend's mail. The phone acts on no cells of the
    /// desk, so the desk tells it; the phone, the desk an older Sioul or
    /// unknown by its book, claims it. Once, never lost.
    #[test]
    fn stale_peers_never_lose_a_thing() {
        for stale in [true, false] {
            let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")], hours());
            let phone = &mut m.devices[1];
            if stale {
                phone.read.get_mut("desk").unwrap().areas = 0;
            } else {
                phone.read.remove("desk");
            }
            m.exchange("desk", "phone");
            m.fetch("desk", "desk", "friend", Area::PERSONAL, Time::Leisure);
            m.fetch("phone", "desk", "friend", Area::PERSONAL, Time::Leisure);
            assert_eq!((m.tells("friend"), m.waits("friend")), (1, false), "stale: {stale}");
        }
    }

    /// Two phones put away (M4): the desk for work keeps the notices; a
    /// phone and a tablet for leisure both fetch a friend's mail in the
    /// evening. Neither tells it while the desk keeps the notices; the first
    /// used tells it, the other and the desk claim it. Once.
    #[test]
    fn two_devices_put_away_tell_a_thing_once() {
        let mut m = Model::new(&[("desk", Line::for_area(Area::WORK), "computer"), ("phone", Line::for_area(Area::LEISURE), "phone"), ("tablet", Line::for_area(Area::LEISURE), "tablet")], hours());
        for id in ["desk", "phone", "tablet"] {
            m.fetch(id, "desk", "friend", Area::LEISURE, Time::Leisure);
        }
        assert_eq!(m.tells("friend"), 0);
        assert!(["desk", "phone", "tablet"].iter().all(|id| m.device(id).waiting.contains("friend")));
        for id in ["phone", "tablet", "desk"] {
            m.tick(id, "phone", Area::LEISURE, Time::Leisure);
        }
        assert_eq!((m.tells("friend"), m.waits("friend")), (1, false));
    }

    /// A time boundary between two fetches (M4): the desk, for work and
    /// admin, keeps the notices and fetches mail to the personal address in
    /// admin's last seconds: it tells it. The phone fetches it in leisure's
    /// first: read as the desk, which brings no leisure, it keeps it. Its
    /// told log reaching the phone before it keeps the notices, the phone
    /// does not tell it again, as at any time boundary before devices.
    /// Never lost either way.
    #[test]
    fn a_time_boundary_between_two_fetches_tells_a_thing_once() {
        let mut m = Model::new(&[("desk", Line::for_area(Area::WORK.with(Area::ADMIN)), "computer"), ("phone", Line::for_area(Area::PERSONAL), "phone")], hours());
        m.fetch("desk", "desk", "boundary", Area::PERSONAL, Time::Admin);
        m.fetch("phone", "desk", "boundary", Area::PERSONAL, Time::Leisure);
        assert_eq!(m.tells("boundary"), 1);
        assert!(m.device("phone").waiting.contains("boundary"), "kept, never lost");
        m.tick("phone", "phone", Area::PERSONAL, Time::Leisure);
        assert_eq!((m.tells("boundary"), m.waits("boundary")), (1, false));
    }
}

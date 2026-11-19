// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Starred people and Always through, kept in step both ways
//! (docs/do-not-disturb.md, "Starred people"; the owner's decision of 18
//! November 2026, which takes back the rule of 6 October that Sioul never
//! writes a star).
//!
//! A **place** keeps stars of its own: a phone's contacts (Android's star),
//! or an address book on a server that keeps a favourite per card
//! (Nextcloud's `{http://nextcloud.com/ns}favorite`). Each look at a place
//! (`plan`) reads every contact it stars and every contact of someone on the
//! Always through list (`Seen`), with what that place's memory holds
//! (`Memory`, the place's own, never shared): the stars Sioul wrote there,
//! with the person each stood for, and the contacts seen starred at the last
//! look. It says what to do, by two rules (review of `phones`, 19 November
//! 2026): **nothing changes until the person changes it**, and **absence
//! proves nothing**:
//! - a contact starred since the last look adds its person to the list, on
//!   every device (**never lost**: a star seen anywhere adds the person);
//! - someone on the list whose contact the place holds unstarred is starred
//!   there, by Sioul, which remembers it;
//! - a person leaves the list only when a star Sioul wrote is read there,
//!   present and unstarred: the person's own act, seen positively. A star
//!   the person set by hand and took off takes nobody off. A look that
//!   would take two people or more off at once takes nobody off: their
//!   stars went together, which no hand does; one informative notice names
//!   them and opens the list (H3);
//! - a star Sioul wrote for someone taken off the list in Sioul is taken
//!   off there; a star of the person's own is left as it is.
//!
//! A contact read nowhere, a look that read nothing, a contact that matches
//! nobody any more: nothing changes. Someone no contact of the place holds
//! cannot be starred there: the window says so. A blocked number is never
//! added. Turning starred people both ways off forgets every memory; turned
//! on again, the first look adds and stars, and takes nobody off. Sioul
//! writes the star and nothing else: never a contact, a name, a number.

use crate::everywhere::{People, Person};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A contact as one place holds it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    /// The place's own id for it: a phone's lookup key, a card's address on its server.
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub phones: Vec<String>,
    #[serde(default)]
    pub emails: Vec<String>,
    /// The card's UID, when the place is one of your address books.
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub starred: bool,
}

/// What a place's looks remember (H3): the stars Sioul wrote there, by the
/// place's ids, with the person each stood for (by their id on the list);
/// the contacts seen starred at the last look; for a server, whether it
/// keeps favourites (true once it answered one as its own; none: not known,
/// and nothing is written there), and when it was looked at. An older file's
/// fields (`starred`, every star seen) are passed over: a first look.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub written: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub seen: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<bool>,
    /// The last look (Unix seconds).
    #[serde(default)]
    pub looked: i64,
}

impl Memory {
    /// What the place's file holds; nothing yet, or a file that does not read: a first look.
    pub fn load(path: &Path) -> Memory {
        std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    /// Written beside, then put in place.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Starred people both ways turned off: the memory forgotten, so that a
    /// look once it is on again is a first look (adding only: H3). Whether a
    /// file was there.
    pub fn forget(path: &Path) -> bool {
        std::fs::remove_file(path).is_ok()
    }
}

/// People kept on the list at a place whose stars went at once (H3), waiting
/// to be told on this device's window: `kept.json` in the memories' folder,
/// [{place, names}], this device's own.
pub fn kept_path() -> PathBuf {
    folder().join("kept.json")
}

/// People kept at `place` noted, for the window to tell (`take_kept`).
pub fn note_kept(place: &str, names: &[String]) -> Result<(), String> {
    if names.is_empty() {
        return Ok(());
    }
    let path = kept_path();
    let mut waiting: Vec<(String, Vec<String>)> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    waiting.push((place.to_string(), names.to_vec()));
    let text = serde_json::to_string(&waiting).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    crate::filelock::replace(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// What waits to be told (`note_kept`), taken: the file gone.
pub fn take_kept() -> Vec<(String, Vec<String>)> {
    let path = kept_path();
    let waiting = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    waiting
}

/// The memories' folder, in the state folder: `phone.json` for this phone's
/// contacts, `<account>/<book>.json` for a book on a server.
pub fn folder() -> PathBuf {
    crate::config::state_dir().join("stars")
}

/// What to do so that a place and the list agree.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// People to add to the list: starred there, on it nowhere.
    pub add: Vec<Person>,
    /// People to take off the list, by their ids: a star Sioul wrote, unstarred there.
    pub remove: Vec<String>,
    /// People whose stars Sioul wrote went at once, two or more: none taken
    /// off, said in a notice (H3), by their ids.
    pub kept: Vec<String>,
    /// Contacts to star there, by the place's ids, with the person each is starred for.
    pub star: Vec<String>,
    /// Contacts to unstar there: a star Sioul wrote for someone taken off the list in Sioul.
    pub unstar: Vec<String>,
    /// The memory once the plan is done.
    pub memory: Memory,
}

impl Plan {
    /// Nothing to change anywhere.
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty() && self.star.is_empty() && self.unstar.is_empty()
    }

    /// The next memory once some writes at the place failed: a star not
    /// written is neither Sioul's nor starred there; an unstarring not
    /// written still is both.
    pub fn memory_after(&self, failed: &[String], before: &Memory) -> Memory {
        let mut memory = self.memory.clone();
        for id in failed {
            if self.star.contains(id) {
                memory.written.remove(id);
                memory.seen.remove(id);
            }
            if self.unstar.contains(id) {
                if let Some(person) = before.written.get(id) {
                    memory.written.insert(id.clone(), person.clone());
                }
                memory.seen.insert(id.clone());
            }
        }
        memory
    }
}

fn address_key(text: &str) -> String {
    let text = text.trim();
    let inner = text.rsplit_once('<').map_or(text, |(_, rest)| rest.trim_end_matches('>'));
    inner.trim().trim_start_matches("mailto:").to_lowercase()
}

/// Whether a contact of a place is this person: their card, a whole number, an address.
fn same(person: &Person, seen: &Seen, region: Option<&crate::phones::Region>) -> bool {
    if !seen.uid.trim().is_empty() && person.contact.trim() == seen.uid.trim() {
        return true;
    }
    let keys: Vec<String> = seen.phones.iter().map(|n| crate::phones::key(n, region)).filter(|k| crate::phones::is_whole(k)).collect();
    if person.phones.iter().any(|n| keys.contains(&crate::phones::key(n, region))) {
        return true;
    }
    let addresses: Vec<String> = seen.emails.iter().map(|e| address_key(e)).filter(|a| a.contains('@')).collect();
    person.emails.iter().any(|e| addresses.contains(&address_key(e)))
}

/// Someone for the list from a starred contact.
fn person_of(seen: &Seen) -> Person {
    Person {
        id: String::new(),
        name: seen.name.trim().to_string(),
        phones: seen.phones.iter().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect(),
        emails: seen.emails.iter().map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect(),
        contact: seen.uid.trim().to_string(),
        starred: std::collections::BTreeMap::new(),
        other: toml::Table::new(),
    }
}

/// What to do at a place (the module's rules), from the list as it is, what
/// the place holds now, and its memory. `blocked`: whether a contact is
/// blocked (never added).
pub fn plan(people: &People, seen: &[Seen], memory: &Memory, region: Option<&crate::phones::Region>, blocked: &dyn Fn(&Seen) -> bool) -> Plan {
    let matches: Vec<Vec<usize>> = seen.iter().map(|c| people.people.iter().enumerate().filter(|(_, p)| same(p, c, region)).map(|(i, _)| i).collect()).collect();
    let on_list = |id: &str| people.people.iter().any(|p| p.id == id);
    // Someone a contact of this place still stars: never lost (M7).
    let still_starred = |person: &str| seen.iter().zip(&matches).any(|(c, m)| c.starred && m.iter().any(|&i| people.people[i].id == person));
    // A star Sioul wrote, read there present and unstarred: its person, still on the list, leaves it.
    let mut leaving: Vec<String> = Vec::new();
    for c in seen.iter().filter(|c| !c.starred) {
        if let Some(person) = memory.written.get(&c.id)
            && on_list(person)
            && !still_starred(person)
            && !leaving.contains(person)
        {
            leaving.push(person.clone());
        }
    }
    // Absence proves nothing: what a contact not read this time was is kept.
    let mut plan = Plan { memory: memory.clone(), ..Plan::default() };
    // Two or more at once: no hand does that; nobody leaves, and it is said (H3).
    if leaving.len() >= 2 {
        plan.kept = leaving;
    } else {
        plan.remove = leaving;
    }
    let removing = plan.remove.clone();
    let leaves = |i: usize| removing.contains(&people.people[i].id);
    for (c, m) in seen.iter().zip(&matches) {
        let written_for = memory.written.get(&c.id);
        if c.starred {
            if m.is_empty() && !memory.seen.contains(&c.id) && written_for.is_none() {
                // Starred since the last look, on the list nowhere: added; a blocked one is left as it is.
                if !blocked(c) {
                    plan.add.push(person_of(c));
                }
            }
            if let Some(person) = written_for
                && !on_list(person)
            {
                // A star Sioul wrote for someone taken off the list in Sioul: taken off there.
                plan.unstar.push(c.id.clone());
                plan.memory.written.remove(&c.id);
                plan.memory.seen.remove(&c.id);
                continue;
            }
            plan.memory.seen.insert(c.id.clone());
        } else {
            // Read present and unstarred: no longer Sioul's star, nor seen starred, whatever follows.
            plan.memory.written.remove(&c.id);
            plan.memory.seen.remove(&c.id);
            // Someone on the list, held here without a star, never unstarred here since the last look: starred.
            let unstarred_here = memory.seen.contains(&c.id) || written_for.is_some();
            if let Some(&i) = m.iter().find(|&&i| !leaves(i))
                && !unstarred_here
            {
                plan.star.push(c.id.clone());
                plan.memory.written.insert(c.id.clone(), people.people[i].id.clone());
                plan.memory.seen.insert(c.id.clone());
            }
        }
    }
    plan
}

/// The list changed as a plan says, under its lock: people taken off, then
/// people added (merged with whoever is there by a number, an address or a
/// card). Whether it changed.
pub fn apply_to_list(path: &Path, plan: &Plan, region: Option<&crate::phones::Region>) -> Result<bool, String> {
    if plan.add.is_empty() && plan.remove.is_empty() {
        return Ok(false);
    }
    let before = People::read(path)?;
    let after = crate::everywhere::change_people(path, |people| {
        for id in &plan.remove {
            people.remove(id);
        }
        for person in &plan.add {
            people.add(person.clone(), region);
        }
    })?;
    Ok(after != before)
}

/// Where each person on the list is starred, for the list's block: a place's
/// name with "starred", "not-starred" (held there without a star), or
/// "absent" (no contact of that place holds them).
pub fn where_starred(people: &People, seen: &[Seen], region: Option<&crate::phones::Region>) -> Vec<(String, &'static str)> {
    people
        .people
        .iter()
        .map(|p| {
            let held: Vec<&Seen> = seen.iter().filter(|c| same(p, c, region)).collect();
            let state = if held.iter().any(|c| c.starred) {
                "starred"
            } else if held.is_empty() {
                "absent"
            } else {
                "not-starred"
            };
            (p.id.clone(), state)
        })
        .collect()
}

/// The stars written at a place: `star` and `unstar` by the place's ids;
/// answers the ids it could not write.
pub type Write<'a> = &'a mut dyn FnMut(&[String], &[String]) -> Vec<String>;

/// What one look did: whether the list changed, and the names of the people
/// kept on it whose stars went at once (H3), for a notice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Looked {
    pub changed: bool,
    pub kept: Vec<String>,
}

/// One look at a place, done (the module's rules): the plan made from the
/// list and `memory`, the list changed, the stars written (`write`; none:
/// this place may be read only, every star of the plan left for a later
/// look), `memory` brought to what the place holds now (its caller saves
/// it), and where each person is starred there recorded in their entries.
pub fn look(list: &Path, memory: &mut Memory, place: &str, seen: &[Seen], region: Option<&crate::phones::Region>, blocked: &dyn Fn(&Seen) -> bool, write: Option<Write>) -> Result<Looked, String> {
    let people = People::read(list)?;
    let plan = plan(&people, seen, memory, region, blocked);
    let changed = apply_to_list(list, &plan, region)?;
    let failed: Vec<String> = match write {
        _ if plan.star.is_empty() && plan.unstar.is_empty() => Vec::new(),
        Some(write) => write(&plan.star, &plan.unstar),
        None => plan.star.iter().chain(&plan.unstar).cloned().collect(),
    };
    let before = memory.clone();
    *memory = plan.memory_after(&failed, &before);
    // What the place holds now, for where each person is starred.
    let now: Vec<Seen> = seen.iter().map(|c| Seen { starred: memory.seen.contains(&c.id), ..c.clone() }).collect();
    let recorded = record(list, place, &now, region)?;
    let kept = plan.kept.iter().filter_map(|id| people.people.iter().find(|p| &p.id == id).map(|p| p.name.clone())).collect();
    Ok(Looked { changed: changed || recorded, kept })
}

/// What one place said of each person on the list (`where_starred`),
/// written in their entries (`Person::starred`) when it changed, so that
/// every device shows where each is starred. Whether the list changed.
pub fn record(path: &Path, place: &str, people_seen: &[Seen], region: Option<&crate::phones::Region>) -> Result<bool, String> {
    let now = People::read(path)?;
    let states = where_starred(&now, people_seen, region);
    let differs = now.people.iter().zip(&states).any(|(p, (_, state))| p.starred.get(place).map(String::as_str) != Some(*state));
    if !differs {
        return Ok(false);
    }
    crate::everywhere::change_people(path, |people| {
        let states = where_starred(people, people_seen, region);
        for (person, (_, state)) in people.people.iter_mut().zip(states) {
            person.starred.insert(place.to_string(), state.to_string());
        }
    })?;
    Ok(true)
}

/// A place gone (a phone forgotten, a book taken away): its word taken out of every entry.
pub fn forget_place(path: &Path, place: &str) -> Result<bool, String> {
    if !People::read(path)?.people.iter().any(|p| p.starred.contains_key(place)) {
        return Ok(false);
    }
    crate::everywhere::change_people(path, |people| {
        for person in &mut people.people {
            person.starred.remove(place);
        }
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fr() -> Option<&'static crate::phones::Region> {
        crate::phones::region_named("FR")
    }

    fn seen(id: &str, phone: &str, starred: bool) -> Seen {
        Seen { id: id.into(), name: id.into(), phones: vec![phone.into()], starred, ..Seen::default() }
    }

    fn listed(people: &mut People, name: &str, phone: &str) -> String {
        people.add(Person { name: name.into(), phones: vec![phone.into()], ..Person::default() }, fr())
    }

    fn nobody(_: &Seen) -> bool {
        false
    }

    fn ids(set: &BTreeSet<String>) -> Vec<&str> {
        set.iter().map(String::as_str).collect()
    }

    /// A memory where Sioul wrote `written` (contact, person) and saw them starred.
    fn wrote(written: &[(&str, &str)]) -> Memory {
        Memory { written: written.iter().map(|(c, p)| (c.to_string(), p.to_string())).collect(), seen: written.iter().map(|(c, _)| c.to_string()).collect(), ..Memory::default() }
    }

    /// The first look: every starred contact joins the list; everyone on the
    /// list the phone holds is starred, Sioul remembering it wrote it;
    /// someone it does not hold is left.
    #[test]
    fn a_first_look_adds_the_starred_and_stars_the_list() {
        let mut people = People::default();
        let claire = listed(&mut people, "Claire", "01 99 00 00 06");
        listed(&mut people, "Away", "01 99 00 00 09");
        let place = [seen("mum", "+33 1 99 00 00 01", true), seen("claire", "0199000006", false), seen("fred", "01 99 00 00 02", false)];
        let plan = plan(&people, &place, &Memory::default(), fr(), &nobody);
        assert_eq!(plan.add.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["mum"]);
        assert_eq!((plan.star.as_slice(), plan.remove.is_empty(), plan.unstar.is_empty()), (&["claire".to_string()][..], true, true));
        assert_eq!(ids(&plan.memory.seen), ["claire", "mum"]);
        assert_eq!(plan.memory.written.get("claire"), Some(&claire), "Sioul's own star, for Claire");
        assert!(!plan.memory.written.contains_key("mum"), "a star of the person's own is not Sioul's");
        let states = where_starred(&people, &place, fr());
        assert_eq!(states[0], (claire, "not-starred"));
        assert_eq!(states[1].1, "absent", "no contact of the phone holds them");
    }

    /// Only Sioul's stars take someone off (H3): a star Sioul wrote, read
    /// unstarred, takes its person off; a star of the person's own,
    /// unstarred, takes nobody off; a star Sioul wrote for someone taken off
    /// in Sioul is unstarred; one of the person's own is left.
    #[test]
    fn only_sioul_s_stars_take_someone_off() {
        let mut people = People::default();
        let claire = listed(&mut people, "Claire", "01 99 00 00 06");
        let ann = listed(&mut people, "Ann", "01 99 00 00 07");
        let mut memory = wrote(&[("claire", &claire), ("bob", "gone-bob")]);
        memory.seen.insert("ann".into());
        memory.seen.insert("own".into());
        // Claire unstarred where Sioul starred her; Ann unstarred, her star her own; Bob still starred,
        // taken off the list in Sioul; a star of the person's own whose person was taken off in Sioul.
        let place = [seen("claire", "01 99 00 00 06", false), seen("ann", "01 99 00 00 07", false), seen("bob", "01 99 00 00 08", true), seen("own", "01 99 00 00 11", true)];
        let plan = plan(&people, &place, &memory, fr(), &nobody);
        assert_eq!(plan.remove, [claire]);
        assert_eq!(plan.unstar, ["bob"]);
        assert!(plan.add.is_empty() && plan.star.is_empty(), "Ann unstarred her own: left so, and on the list: {plan:?}");
        assert!(people.people.iter().any(|p| p.id == ann));
        assert_eq!(ids(&plan.memory.seen), ["own"]);
        assert!(plan.memory.written.is_empty());
    }

    /// Stars lost all at once (an account synced again, the reviewer's H3
    /// test): nobody taken off, said; the next look stars them again.
    #[test]
    fn stars_lost_all_at_once_remove_nobody() {
        let mut people = People::default();
        let mut place = Vec::new();
        let mut written = Vec::new();
        for n in 1..=8 {
            let phone = format!("01 99 00 00 {:02}", n + 10);
            let id = listed(&mut people, &format!("p{n}"), &phone);
            place.push(seen(&format!("k{n}"), &phone, false));
            written.push((format!("k{n}"), id));
        }
        let memory = Memory { written: written.iter().cloned().collect(), seen: written.iter().map(|(c, _)| c.clone()).collect(), ..Memory::default() };
        let plan = plan(&people, &place, &memory, fr(), &nobody);
        assert!(plan.remove.is_empty(), "eight stars gone at once are not eight people taken off: {plan:?}");
        assert_eq!(plan.kept.len(), 8, "named in a notice");
        let again = super::plan(&people, &place, &plan.memory, fr(), &nobody);
        assert_eq!(again.star.len(), 8, "starred again at the next look");
        assert!(again.remove.is_empty());
        // One alone is the person's act: taken off.
        let one = super::plan(&people, &place[..1], &memory, fr(), &nobody);
        assert_eq!(one.remove.len(), 1);
    }

    /// Turned off then on again: the memory forgotten, the first look adds
    /// and stars and takes nobody off (H3, the switch's path).
    #[test]
    fn turned_on_again_a_first_look_takes_nobody_off() {
        let dir = std::env::temp_dir().join(format!("sioul-stars-forget-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("phone.json");
        let mut people = People::default();
        let claire = listed(&mut people, "Claire", "01 99 00 00 06");
        wrote(&[("claire", &claire)]).save(&path).unwrap();
        assert!(Memory::forget(&path), "switched off: forgotten");
        // Meanwhile unstarred by hand: on again, she is starred again, not taken off.
        let plan = plan(&people, &[seen("claire", "01 99 00 00 06", false)], &Memory::load(&path), fr(), &nobody);
        assert!(plan.remove.is_empty() && plan.star == ["claire"], "{plan:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Never lost: a star seen anew wins over an unstarring of the same
    /// person; a contact that never was starred is no unstarring; a person
    /// the phone does not hold stays; a blocked number is never added.
    #[test]
    fn a_star_is_never_lost() {
        let mut people = People::default();
        listed(&mut people, "Claire", "01 99 00 00 06");
        let both = Person { name: "Dan".into(), phones: vec!["01 99 00 00 10".into(), "06 39 98 99 10".into()], ..Person::default() };
        let dan = people.add(both, fr());
        // Dan's home number, starred by Sioul, unstarred here; his mobile starred anew in another contact: he stays.
        let memory = wrote(&[("dan-home", &dan)]);
        let place = [seen("dan-home", "01 99 00 00 10", false), seen("dan-mobile", "06 39 98 99 10", true), seen("never", "01 99 00 00 11", false), seen("pest", "01 99 00 00 05", true)];
        let blocked = |c: &Seen| c.id == "pest";
        let plan = plan(&people, &place, &memory, fr(), &blocked);
        assert!(plan.remove.is_empty(), "never lost: {plan:?}");
        assert!(plan.add.is_empty() && plan.unstar.is_empty(), "the blocked are never added, and their star is left as it is");
        assert!(plan.star.is_empty(), "the unstarred contact is left as it was left");
        assert!(people.people.iter().any(|p| p.id == dan));
        // Nothing changed since: nothing to do.
        let again = super::plan(&people, &[seen("dan-mobile", "06 39 98 99 10", true)], &plan.memory, fr(), &blocked);
        assert!(again.is_empty(), "{again:?}");
    }

    /// Two phones that disagree, in turn, through the list they share: a
    /// star on one is added and starred on the other by Sioul; unstarred on
    /// the other, where Sioul starred it, it leaves the list; the first
    /// phone's star, the person's own, is left and adds no one back; a write
    /// that failed is tried again at the next look.
    #[test]
    fn two_phones_that_disagree_meet_through_the_list() {
        let dir = std::env::temp_dir().join(format!("sioul-stars-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let list = dir.join("dnd-people.toml");
        let (mut a, mut b) = (Memory::default(), Memory::default());
        // Phone A stars Claire; B holds her unstarred.
        let on_a = |starred: bool| [seen("a-claire", "01 99 00 00 06", starred)];
        let on_b = |starred: bool| [seen("b-claire", "+33199000006", starred)];
        let step = |memory: &mut Memory, place: &[Seen]| {
            let plan = plan(&People::load(&list), place, memory, fr(), &nobody);
            apply_to_list(&list, &plan, fr()).unwrap();
            *memory = plan.memory.clone();
            plan
        };
        let first = step(&mut a, &on_a(true));
        assert_eq!(first.add.len(), 1);
        assert_eq!(People::load(&list).people.len(), 1);
        let before = b.clone();
        let second = step(&mut b, &on_b(false));
        assert_eq!(second.star, ["b-claire"], "starred on the other phone");
        // The star not written on B: tried again at the next look.
        b = second.memory_after(&second.star, &before);
        assert_eq!(step(&mut b, &on_b(false)).star, ["b-claire"]);
        // Written: B holds her starred, nothing to do.
        assert!(step(&mut b, &on_b(true)).is_empty());
        // Unstarred on B by hand, where Sioul starred her: off the list.
        let gone = step(&mut b, &on_b(false));
        assert_eq!(gone.remove.len(), 1, "unstarred where Sioul starred her");
        assert!(People::load(&list).people.is_empty());
        let after = step(&mut a, &on_a(true));
        assert!(after.unstar.is_empty(), "A's star is the person's own: left");
        assert!(People::load(&list).people.is_empty(), "and adds no one back");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

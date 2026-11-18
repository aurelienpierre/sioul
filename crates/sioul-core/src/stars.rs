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
//! Always through list (`Seen`), with what that place starred at its last
//! look (`Memory`, the place's own, never shared), and says what to do:
//! - a contact starred since the last look adds its person to the list, on
//!   every device (**never lost**: a star seen anywhere adds the person);
//! - someone on the list whose contact the place holds unstarred is starred
//!   there;
//! - a contact the place starred at its last look and no longer stars takes
//!   its person off the list: unstarred where Sioul knew it starred;
//! - a contact still starred whose person is no longer on the list was taken
//!   off in Sioul: unstarred there.
//!
//! A starred contact seen anew wins over an unstarring of the same person
//! (never lost comes first). Someone no contact of the place holds cannot be
//! starred there: the window says so. A blocked number is never added. Sioul
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

/// What a place starred at its last look, by its ids; for a server, whether
/// it keeps a star at all (none: not known yet), and when it was looked at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub starred: BTreeSet<String>,
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
    /// People to take off the list, by their ids: unstarred there.
    pub remove: Vec<String>,
    /// Contacts to star there, by the place's ids: someone on the list.
    pub star: Vec<String>,
    /// Contacts to unstar there: taken off the list in Sioul.
    pub unstar: Vec<String>,
    /// What the place stars once the plan is done: its next memory.
    pub starred: BTreeSet<String>,
}

impl Plan {
    /// Nothing to change anywhere.
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty() && self.star.is_empty() && self.unstar.is_empty()
    }

    /// The next memory once some writes at the place failed: a star not
    /// written is not starred there; an unstarring not written still is.
    pub fn starred_after(&self, failed: &[String]) -> BTreeSet<String> {
        let mut starred = self.starred.clone();
        for id in failed {
            if self.star.contains(id) {
                starred.remove(id);
            }
            if self.unstar.contains(id) {
                starred.insert(id.clone());
            }
        }
        starred
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
/// the place holds now, and what it starred at its last look. `blocked`:
/// whether a contact is blocked (never added).
pub fn plan(people: &People, seen: &[Seen], last: &BTreeSet<String>, region: Option<&crate::phones::Region>, blocked: &dyn Fn(&Seen) -> bool) -> Plan {
    let matches: Vec<Vec<usize>> = seen.iter().map(|c| people.people.iter().enumerate().filter(|(_, p)| same(p, c, region)).map(|(i, _)| i).collect()).collect();
    let newly = |c: &Seen| c.starred && !last.contains(&c.id);
    let unstarred = |c: &Seen| !c.starred && last.contains(&c.id);
    // Unstarred here where Sioul knew it starred: off the list, unless a star of theirs is seen anew (never lost).
    let mut removed: BTreeSet<usize> = BTreeSet::new();
    for (c, m) in seen.iter().zip(&matches) {
        if unstarred(c) {
            removed.extend(m.iter().copied());
        }
    }
    for (c, m) in seen.iter().zip(&matches) {
        if newly(c) {
            for i in m {
                removed.remove(i);
            }
        }
    }
    let mut plan = Plan { remove: removed.iter().map(|i| people.people[*i].id.clone()).collect(), ..Plan::default() };
    for (c, m) in seen.iter().zip(&matches) {
        let kept: Vec<usize> = m.iter().copied().filter(|i| !removed.contains(i)).collect();
        if c.starred {
            if !kept.is_empty() {
                plan.starred.insert(c.id.clone());
            } else if newly(c) && m.is_empty() {
                // Starred since the last look, on the list nowhere: added; a blocked one is left as it is.
                if !blocked(c) {
                    plan.add.push(person_of(c));
                    plan.starred.insert(c.id.clone());
                }
            } else {
                // Starred before and still, its person no longer on the list (taken off in Sioul), or unstarred elsewhere.
                plan.unstar.push(c.id.clone());
            }
        } else if !kept.is_empty() && !unstarred(c) {
            // Someone on the list, held here without a star: starred.
            plan.star.push(c.id.clone());
            plan.starred.insert(c.id.clone());
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

/// One look at a place, done (the module's rules): the plan made from the
/// list and `memory`, the list changed, the stars written (`write`; none:
/// this place may be read only, every star of the plan left for a later
/// look), `memory` brought to what the place stars now (its caller saves
/// it), and where each person is starred there recorded in their entries.
/// Whether the list changed.
pub fn look(list: &Path, memory: &mut Memory, place: &str, seen: &[Seen], region: Option<&crate::phones::Region>, blocked: &dyn Fn(&Seen) -> bool, write: Option<Write>) -> Result<bool, String> {
    let people = People::read(list)?;
    let plan = plan(&people, seen, &memory.starred, region, blocked);
    let changed = apply_to_list(list, &plan, region)?;
    let failed: Vec<String> = match write {
        _ if plan.star.is_empty() && plan.unstar.is_empty() => Vec::new(),
        Some(write) => write(&plan.star, &plan.unstar),
        None => plan.star.iter().chain(&plan.unstar).cloned().collect(),
    };
    memory.starred = plan.starred_after(&failed);
    // What the place holds now, for where each person is starred.
    let now: Vec<Seen> = seen.iter().map(|c| Seen { starred: memory.starred.contains(&c.id), ..c.clone() }).collect();
    let recorded = record(list, place, &now, region)?;
    Ok(changed || recorded)
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

    /// The first look: every starred contact joins the list; everyone on the
    /// list the phone holds is starred; someone it does not hold is left.
    #[test]
    fn a_first_look_adds_the_starred_and_stars_the_list() {
        let mut people = People::default();
        let claire = listed(&mut people, "Claire", "01 99 00 00 06");
        listed(&mut people, "Away", "01 99 00 00 09");
        let place = [seen("mum", "+33 1 99 00 00 01", true), seen("claire", "0199000006", false), seen("fred", "01 99 00 00 02", false)];
        let plan = plan(&people, &place, &BTreeSet::new(), fr(), &nobody);
        assert_eq!(plan.add.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["mum"]);
        assert_eq!((plan.star.as_slice(), plan.remove.is_empty(), plan.unstar.is_empty()), (&["claire".to_string()][..], true, true));
        assert_eq!(ids(&plan.starred), ["claire", "mum"]);
        let states = where_starred(&people, &place, fr());
        assert_eq!(states[0], (claire, "not-starred"));
        assert_eq!(states[1].1, "absent", "no contact of the phone holds them");
    }

    /// Removed: unstarred on the phone where Sioul knew it starred, the
    /// person leaves the list; taken off in Sioul, the phone unstars them.
    #[test]
    fn unstarring_and_taking_off_go_both_ways() {
        let mut people = People::default();
        let claire = listed(&mut people, "Claire", "01 99 00 00 06");
        listed(&mut people, "Ann", "01 99 00 00 07");
        let last: BTreeSet<String> = ["claire", "ann", "bob"].into_iter().map(String::from).collect();
        // Claire unstarred on the phone; Ann still starred; Bob still starred but taken off the list in Sioul.
        let place = [seen("claire", "01 99 00 00 06", false), seen("ann", "01 99 00 00 07", true), seen("bob", "01 99 00 00 08", true)];
        let plan = plan(&people, &place, &last, fr(), &nobody);
        assert_eq!(plan.remove, [claire]);
        assert_eq!(plan.unstar, ["bob"]);
        assert!(plan.add.is_empty() && plan.star.is_empty());
        assert_eq!(ids(&plan.starred), ["ann"]);
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
        // Dan's home number unstarred here, his mobile starred anew in another contact: he stays, starred.
        let last: BTreeSet<String> = ["dan-home"].into_iter().map(String::from).collect();
        let place = [seen("dan-home", "01 99 00 00 10", false), seen("dan-mobile", "06 39 98 99 10", true), seen("never", "01 99 00 00 11", false), seen("pest", "01 99 00 00 05", true)];
        let blocked = |c: &Seen| c.id == "pest";
        let plan = plan(&people, &place, &last, fr(), &blocked);
        assert!(plan.remove.is_empty(), "never lost: {plan:?}");
        assert!(plan.add.is_empty() && plan.unstar.is_empty(), "the blocked are never added, and their star is left as it is");
        assert!(plan.star.is_empty(), "the unstarred contact is left as you left it");
        assert_eq!(ids(&plan.starred), ["dan-mobile"]);
        assert!(people.people.iter().any(|p| p.id == dan));
        // Nothing changed since: nothing to do.
        let again = super::plan(&people, &[seen("dan-mobile", "06 39 98 99 10", true)], &plan.starred, fr(), &blocked);
        assert!(again.is_empty(), "{again:?}");
    }

    /// Two phones that disagree, in turn, through the list they share: a
    /// star on one is added and starred on the other; unstarred on the
    /// other, where Sioul knew it, it leaves the list and the first phone
    /// unstars it too; a write that failed is tried again at the next look.
    #[test]
    fn two_phones_that_disagree_meet_through_the_list() {
        let dir = std::env::temp_dir().join(format!("sioul-stars-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let list = dir.join("dnd-people.toml");
        let (mut a, mut b): (BTreeSet<String>, BTreeSet<String>) = (BTreeSet::new(), BTreeSet::new());
        // Phone A stars Claire; B holds her unstarred.
        let on_a = |starred: bool| [seen("a-claire", "01 99 00 00 06", starred)];
        let on_b = |starred: bool| [seen("b-claire", "+33199000006", starred)];
        let step = |memory: &mut BTreeSet<String>, place: &[Seen]| {
            let plan = plan(&People::load(&list), place, memory, fr(), &nobody);
            apply_to_list(&list, &plan, fr()).unwrap();
            *memory = plan.starred.clone();
            plan
        };
        let first = step(&mut a, &on_a(true));
        assert_eq!(first.add.len(), 1);
        assert_eq!(People::load(&list).people.len(), 1);
        let second = step(&mut b, &on_b(false));
        assert_eq!(second.star, ["b-claire"], "starred on the other phone");
        // The star not written on B: tried again at the next look.
        b = second.starred_after(&second.star);
        assert_eq!(step(&mut b, &on_b(false)).star, ["b-claire"]);
        // Written: B holds her starred, nothing to do.
        assert!(step(&mut b, &on_b(true)).is_empty());
        // Unstarred on B by hand, where Sioul knew it starred: off the list; A then unstars her.
        let gone = step(&mut b, &on_b(false));
        assert_eq!(gone.remove.len(), 1, "unstarred where Sioul knew it starred");
        assert!(People::load(&list).people.is_empty());
        let after = step(&mut a, &on_a(true));
        assert_eq!(after.unstar, ["a-claire"], "taken off: the other phone unstars her");
        assert!(People::load(&list).people.is_empty(), "and adds no one back");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Starred people both ways on this phone (`sioul_core::stars`;
//! docs/do-not-disturb.md, "Starred people"): while `[dnd] stars` is on, at
//! each look of do-not-disturb (`everywhere::apply`, every five minutes at
//! most, at once when asked), Java says what this phone's contacts star and
//! hold of the Always through list (DndContacts.look), the core decides, and
//! Java writes the stars it decided (DndContacts.write), and nothing else.
//! Without the permission to write contacts, the stars seen still add their
//! people; the stars to write wait until it is given. The phone keeps what it
//! starred at its last look (`stars/phone.json`, its own). Your people whose
//! calls ring under do-not-disturb are starred by the same path
//! (`your_people`, `stars::plan_with`'s `also`), never added to the list.

use crate::backend::load_config;
use sioul_core::everywhere::People;
use sioul_core::stars::{self, Memory, Seen};
use std::sync::Mutex;

/// A look at most this often (seconds), unless asked.
const EVERY: i64 = 5 * 60;
static LAST: Mutex<i64> = Mutex::new(0);

/// This phone among the places stars are kept: its id in the sharing.
pub(crate) fn place() -> String {
    sioul_sync::share::Here::load(&sioul_core::config::state_dir()).id
}

/// The memory of this phone's last look.
fn memory_path() -> std::path::PathBuf {
    stars::folder().join("phone.json")
}

/// Your people to star on a phone, as the list writes people (ids
/// `stars::CIRCLE` and their id in persons.toml): each one of Contacts ▸ Your
/// people with a card linked, by their cards' numbers and addresses, while
/// their calls ring under do-not-disturb (the matrix's Calls row Your
/// people, its column do-not-disturb at once: decision 5 of 20 November
/// 2026). Nobody otherwise: the stars Sioul wrote for them are then taken off.
pub(crate) fn your_people(config: &sioul_core::config::Config) -> People {
    use sioul_core::attention::{Column, Level, Person, Row};
    let mut out = People::default();
    let attention = sioul_core::attention::Attention::of(config);
    if attention.cell(Row::People(sioul_core::reach::Channel::Calls, Person::People), Column::Dnd) != Level::Now {
        return out;
    }
    let persons = sioul_core::persons::Persons::cached();
    let contacts = sioul_core::contacts::all();
    for person in persons.circle(sioul_core::zone::now().date()) {
        let cards: Vec<&sioul_core::contacts::Contact> = person.cards.iter().filter_map(|uid| contacts.iter().find(|c| !c.uid.trim().is_empty() && c.uid.trim() == uid.trim())).collect();
        let Some(first) = cards.first() else { continue };
        out.people.push(sioul_core::everywhere::Person {
            id: format!("{}{}", stars::CIRCLE, person.id),
            name: person.name.clone(),
            phones: cards.iter().flat_map(|c| c.phones.iter().map(|p| p.value.clone())).collect(),
            emails: cards.iter().flat_map(|c| c.emails.iter().map(|e| e.value.clone())).collect(),
            contact: first.uid.trim().to_string(),
            ..sioul_core::everywhere::Person::default()
        });
    }
    out
}

/// One look at this phone's stars, when on and due (`force`: now). Whether
/// the Always through list changed.
pub(crate) fn here(force: bool) -> bool {
    if !cfg!(target_os = "android") {
        return false;
    }
    let config = load_config();
    if !config.dnd.stars {
        // Off: the memory forgotten, so that turned on again the first look takes nobody off (H3).
        Memory::forget(&memory_path());
        return false;
    }
    let now = jiff::Timestamp::now().as_second();
    {
        let mut last = LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if !force && (0..EVERY).contains(&(now - *last)) {
            return false;
        }
        *last = now;
    }
    let list = People::default_path();
    let Ok(people) = People::read(&list) else { return false };
    let also = your_people(&config);
    let asked: Vec<serde_json::Value> = people.people.iter().chain(&also.people).map(|p| serde_json::json!({ "id": p.id, "phones": p.phones })).collect();
    let answer = crate::steps::java("stars-look", &serde_json::json!({ "people": asked }).to_string());
    if answer["permission"] != true {
        return false;
    }
    let seen: Vec<Seen> = serde_json::from_value(answer["contacts"].clone()).unwrap_or_default();
    let senders = sioul_core::porch::Senders::load(&config);
    let blocked = |c: &Seen| c.phones.iter().any(|n| senders.judge_number(n).who == sioul_core::reach::Who::Blocked);
    let mut write = |star: &[String], unstar: &[String]| -> Vec<String> {
        let answer = crate::steps::java("stars-write", &serde_json::json!({ "star": star, "unstar": unstar }).to_string());
        match answer["failed"].as_array() {
            Some(failed) => failed.iter().filter_map(|f| f.as_str().map(str::to_string)).collect(),
            // No answer: nothing known written.
            None => star.iter().chain(unstar).cloned().collect(),
        }
    };
    let path = memory_path();
    let mut memory = Memory::load(&path);
    let writable = answer["write"] == true;
    let result = stars::look_with(&list, &also, &mut memory, &place(), &seen, senders.region(), &blocked, if writable { Some(&mut write) } else { None });
    memory.looked = now;
    if let Err(e) = memory.save(&path) {
        eprintln!("sioul: stars: {e}");
    }
    match result {
        Ok(looked) => {
            if let Err(e) = stars::note_kept(&place(), &looked.kept) {
                eprintln!("sioul: stars: {e}");
            }
            looked.changed
        }
        Err(e) => {
            eprintln!("sioul: stars: {e}");
            false
        }
    }
}

/// People kept on the list whose stars went at once at a place (H3), told
/// once, informative: their names, where, and that the list is theirs to
/// change; it opens the list. On a phone through Java's notices, on a
/// computer through the window's.
pub(crate) fn tell_kept() {
    let waiting = stars::take_kept();
    if waiting.is_empty() {
        return;
    }
    let config = load_config();
    let books = sioul_core::vdir::collections(sioul_core::vdir::Kind::Contacts);
    let here = place();
    let tr = crate::backend::tr();
    for (at, names) in waiting {
        let mut args = sioul_core::i18n::args();
        args.set("names", names.join(", "));
        args.set("place", place_name(&at, &here, &config, &books, tr));
        let notice = sioul_core::notice::Notice::new("sioul:settings/dnd.people", sioul_core::notice::Kind::Minor, tr.text("stars-kept-title", None), tr.text("stars-kept", Some(&args)));
        if cfg!(target_os = "android") {
            if let Some(json) = crate::eventalarms::notice_json(&notice.key(), &notice, false, None, jiff::Timestamp::now().as_second()) {
                crate::steps::java("notice", &json.to_string());
            }
        } else {
            let _ = crate::tell::tell(notice);
        }
    }
}

/// Whether this phone may write stars (READ_CONTACTS and WRITE_CONTACTS); false elsewhere.
pub(crate) fn writable_here() -> bool {
    cfg!(target_os = "android") && crate::steps::java("stars-look", "{\"people\":[]}")["write"] == true
}

/// Starred people both ways turned on or off, on every device. On this
/// phone, turned on: Android is asked first for the permission to modify
/// contacts, when it was not given; the first look comes once it is.
pub(crate) fn turn(on: bool) -> Result<(), String> {
    sioul_core::config::set_value(&crate::backend::config_path(), "dnd.stars", &sioul_core::config::SettingValue::Bool(on))?;
    if !on {
        // Forgotten at once here; each device forgets its own at its next look (H3).
        let _ = std::fs::remove_dir_all(stars::folder());
    }
    if on && cfg!(target_os = "android") && !writable_here() {
        crate::steps::java("calls-ask-stars", "{}");
    }
    if on {
        std::thread::spawn(|| {
            if here(true) {
                crate::calls::refresh(true);
            }
        });
    }
    Ok(())
}

/// A place's name, as the list's block says it: "this phone", a phone by
/// the name you gave it, else "a phone of yours"; a book by its name and its
/// account's address.
fn place_name(place: &str, here: &str, config: &sioul_core::config::Config, books: &[sioul_core::vdir::Collection], tr: &sioul_core::i18n::Translator) -> String {
    if let Some(book) = place.strip_prefix("book:") {
        let (account, id) = book.split_once('/').unwrap_or((book, ""));
        let name = books.iter().find(|b| b.account == account && b.id == id).map_or_else(|| id.to_string(), |b| b.name.clone());
        let address = config.accounts.iter().find(|a| a.id == account).and_then(|a| a.address.clone()).unwrap_or_else(|| account.to_string());
        let mut args = sioul_core::i18n::args();
        args.set("book", name);
        args.set("account", address);
        return tr.text("stars-place-book", Some(&args));
    }
    if place == here {
        return tr.text("stars-place-this-phone", None);
    }
    sioul_core::device_said::name_of(config, place).map_or_else(|| tr.text("stars-place-phone", None), str::to_string)
}

/// Where one person on the list is starred, in sentences, from what each
/// place said at its last look (`Person::starred`): starred, not starred,
/// or held by no contact there (Sioul cannot star them there).
pub(crate) fn places(person: &sioul_core::everywhere::Person, here: &str, config: &sioul_core::config::Config, books: &[sioul_core::vdir::Collection], tr: &sioul_core::i18n::Translator) -> Vec<String> {
    person
        .starred
        .iter()
        .map(|(place, state)| {
            let mut args = sioul_core::i18n::args();
            args.set("place", place_name(place, here, config, books, tr));
            let key = match state.as_str() {
                "starred" => "stars-starred-on",
                "absent" => "stars-absent-on",
                _ => "stars-not-starred-on",
            };
            tr.text(key, Some(&args))
        })
        .collect()
}

/// The books of yours looked at whose favourites cannot be kept in step yet
/// (no card answered as a favourite there: `sioul_sync::dav::keep_stars`),
/// each in a sentence.
pub(crate) fn without_stars(config: &sioul_core::config::Config, books: &[sioul_core::vdir::Collection], tr: &sioul_core::i18n::Translator) -> Vec<String> {
    books
        .iter()
        .filter(|b| {
            let memory = Memory::load(&stars::folder().join(&b.account).join(format!("{}.json", b.id)));
            memory.looked > 0 && memory.kept != Some(true)
        })
        .map(|b| {
            let mut args = sioul_core::i18n::args();
            args.set("place", place_name(&format!("book:{}/{}", b.account, b.id), "", config, books, tr));
            tr.text("stars-no-star", Some(&args))
        })
        .collect()
}

/// The switch and what it says, for the list's block: {on, label, said, why
/// (the sentence before Android's question, on a phone), servers (the books
/// whose favourites cannot be kept in step yet)}.
pub(crate) fn switch_view(config: &sioul_core::config::Config) -> serde_json::Value {
    let tr = crate::backend::tr();
    let books = sioul_core::vdir::collections(sioul_core::vdir::Kind::Contacts);
    // The contacts this device's places could not match to one person at their last look (M8).
    let unmatched: usize = std::iter::once(memory_path()).chain(books.iter().map(|b| stars::folder().join(&b.account).join(format!("{}.json", b.id)))).map(|path| Memory::load(&path).unmatched).sum();
    let mut args = sioul_core::i18n::args();
    args.set("n", unmatched);
    serde_json::json!({
        "on": config.dnd.stars,
        "label": tr.text("stars-switch", None),
        "said": tr.text("stars-switch-said", None),
        "why": if cfg!(target_os = "android") { tr.text("phone-stars-why", None) } else { String::new() },
        "servers": without_stars(config, &books, tr),
        "unmatched": if config.dnd.stars && unmatched > 0 { tr.text("stars-unmatched", Some(&args)) } else { String::new() },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a person is starred, in words: this phone, another phone by its
    /// name or as a phone of yours, a book by its name and account; each
    /// state its sentence, in both languages.
    #[test]
    fn where_each_person_is_starred_is_said() {
        let config: sioul_core::config::Config = toml::from_str("[[account]]\nid = \"cloud\"\nkind = \"dav\"\naddress = \"alex@cloud.example.org\"\n\n[device.\"other\"]\nname = \"Work phone\"\n").unwrap();
        let book = sioul_core::vdir::Collection { kind: sioul_core::vdir::Kind::Contacts, account: "cloud".into(), id: "contacts".into(), dir: std::path::PathBuf::new(), name: "Contacts".into(), color: None, read_only: false, components: Vec::new() };
        let person = sioul_core::everywhere::Person { starred: [("here", "starred"), ("other", "absent"), ("third", "not-starred"), ("book:cloud/contacts", "starred")].into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..sioul_core::everywhere::Person::default() };
        for language in ["en", "fr"] {
            let tr = sioul_core::i18n::Translator::new(language);
            let said = places(&person, "here", &config, std::slice::from_ref(&book), &tr);
            assert_eq!(said.len(), 4);
            for line in &said {
                assert!(!line.starts_with("stars-") && !line.contains('{'), "{language}: {line}");
            }
            if language == "en" {
                assert!(said.iter().any(|l| l.contains("Contacts") && l.contains("alex@cloud.example.org")), "{said:?}");
                assert!(said.iter().any(|l| l.contains("Work phone")), "{said:?}");
                assert!(said.iter().any(|l| l.contains("this phone")), "{said:?}");
            }
        }
    }
}

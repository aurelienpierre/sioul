// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An office's opening hours looked up (design of 20 November 2026, §4):
//! only when the person presses "Look them up…" on a card that is not a
//! person's, never by itself, by batch or by timer. What is sent: the
//! office's name and where it is, nothing else; what comes back: a short
//! list, "Is it this one?", which the person picks from (`opening` writes
//! the hours on the card, with their source and the day).
//!
//! Two sources (docs/research.md, "Offices' opening hours"):
//!
//! - **The French administration's directory**, "API Annuaire de
//!   l'administration et des services publics" (the DILA, open licence 2.0,
//!   no key): an Opendatasoft "explore" API (v2.1) over the dataset
//!   `api-lannuaire-administration`. Its `plage_ouverture` is a JSON array
//!   written as a string, each item a run of days (`nom_jour_debut`,
//!   `nom_jour_fin`, in French) with two ranges at most
//!   (`valeur_heure_debut_1`, `valeur_heure_fin_1`, `…_2`, "09:00:00") and a
//!   comment; `commentaire_plage_ouverture` comments them all. Asked for
//!   public services in France.
//! - **OpenStreetMap through Nominatim**, its search with `extratags=1`: a
//!   place's `opening_hours` in the map's own syntax. Its usage policy:
//!   one request per press, at most one a second for the whole program
//!   (`sioul_sync::geocode`'s turn), the user agent naming Sioul, the source
//!   named beside the hours ("© OpenStreetMap contributors").
//!
//! The answers are read here, the requests made in `sioul_sync::hours`.

use crate::contacts::Contact;
use serde::{Deserialize, Serialize};

/// The directory's records.
pub const DIRECTORY_ENDPOINT: &str = "https://api-lannuaire.service-public.gouv.fr/api/explore/v2.1/catalog/datasets/api-lannuaire-administration/records";
/// OpenStreetMap's search.
pub const MAP_ENDPOINT: &str = "https://nominatim.openstreetmap.org/search";
/// Five answers at most from each.
pub const MOST: usize = 5;

/// What is looked for: the office's name and where it is, as the person
/// sees them before anything is sent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asked {
    /// Its organisation, else the name its card is shown by: "Mairie de Exempleville".
    #[serde(default)]
    pub name: String,
    /// Where: "99999 Exempleville", or its street too; "" when the card has no address.
    #[serde(default)]
    pub place: String,
    /// Whether the administration's directory is asked too: an address in
    /// France, or none and the person's region France.
    #[serde(default)]
    pub france: bool,
}

/// A five-digit word at the start of a line: a French postal code, and the town after it.
fn code_and_town(place: &str) -> Option<(String, String)> {
    place.lines().map(str::trim).find_map(|line| {
        let (code, town) = line.split_once(' ').unwrap_or((line, ""));
        (code.len() == 5 && code.chars().all(|c| c.is_ascii_digit())).then(|| (code.to_string(), town.trim().to_string()))
    })
}

impl Asked {
    /// What a card looks for: its organisation, else its name, and its first
    /// postal address. None for a person's card: a person's name is never
    /// sent anywhere. `region_france`: the person's region is France.
    pub fn of(contact: &Contact, region_france: bool) -> Option<Asked> {
        if contact.card == "person" {
            return None;
        }
        let name = if contact.org.trim().is_empty() { contact.name.trim() } else { contact.org.trim() };
        if name.is_empty() {
            return None;
        }
        let place = contact.addresses.first().map(|a| a.value.trim().to_string()).unwrap_or_default();
        // In France: a five-digit postal code, its line the last, or "France" after it.
        let last = place.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").to_lowercase();
        let france = if place.is_empty() { region_france } else { code_and_town(&place).is_some() && (last == "france" || code_and_town(&last).is_some()) };
        Some(Asked { name: name.to_string(), place, france })
    }

    /// The words of the name the directory looks for, what says nothing left
    /// out ("de", "la", "l’"), quotes and backslashes taken out.
    fn name_words(&self) -> String {
        const SAY_NOTHING: &[&str] = &["de", "du", "des", "la", "le", "les", "l", "d", "et", "à", "a", "au", "aux", "en", "-", "–"];
        self.name
            .replace(['’', '\''], " ")
            .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\\' | '(' | ')' | ',' | ';'))
            .filter(|w| !w.is_empty() && !SAY_NOTHING.contains(&w.to_lowercase().as_str()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The directory's question: its name's words (`nom LIKE`, every word,
    /// any order), and its postal code when the card says one, as the
    /// publisher's documentation asks for one (`suggest(adresse, "code_postal …")`).
    pub fn directory_query(&self) -> Vec<(&'static str, String)> {
        let mut clause = format!("nom LIKE \"{}\"", self.name_words());
        match code_and_town(&self.place) {
            Some((code, _)) => clause.push_str(&format!(" AND suggest(adresse, \"code_postal {code}\")")),
            None if !self.place.trim().is_empty() => {
                let town: String = self.place.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").chars().filter(|c| !matches!(c, '"' | '\\')).collect();
                if !town.is_empty() {
                    clause.push_str(&format!(" AND adresse LIKE \"{town}\""));
                }
            }
            None => {}
        }
        vec![("where", clause), ("select", "nom,adresse,telephone,plage_ouverture,commentaire_plage_ouverture".into()), ("limit", MOST.to_string())]
    }

    /// Nominatim's question: its name and where, in one line; its tags asked for (`extratags`).
    pub fn map_query(&self) -> Vec<(&'static str, String)> {
        let place = self.place.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(", ");
        let q = if place.is_empty() { self.name.trim().to_string() } else { format!("{}, {place}", self.name.trim()) };
        vec![("q", q), ("format", "jsonv2".into()), ("extratags", "1".into()), ("limit", MOST.to_string())]
    }
}

/// One office found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Found {
    /// `opening::DIRECTORY` or `opening::MAP`.
    pub source: &'static str,
    pub name: String,
    /// On its lines.
    pub address: String,
    /// Its first number; "" when it gives none.
    pub phone: String,
    /// Its hours as Sioul writes them on a card (`opening::written`); "" when
    /// the source gives none it can count.
    pub hours: String,
    /// What it says that Sioul cannot count, for the free line.
    pub note: String,
}

/// A JSON array the directory writes as a string, read; nothing when it is not one.
fn inner(value: &serde_json::Value) -> Vec<serde_json::Value> {
    match value {
        serde_json::Value::String(text) => serde_json::from_str::<Vec<serde_json::Value>>(text).unwrap_or_default(),
        serde_json::Value::Array(items) => items.clone(),
        _ => Vec::new(),
    }
}

fn text_of(value: &serde_json::Value, key: &str) -> String {
    value.get(key).and_then(serde_json::Value::as_str).unwrap_or("").trim().to_string()
}

/// The directory's `plage_ouverture` as windows, its comments apart.
fn directory_hours(record: &serde_json::Value) -> (Vec<crate::window::AdminWindow>, Vec<String>) {
    let mut windows = Vec::new();
    let mut notes = Vec::new();
    for run in inner(record.get("plage_ouverture").unwrap_or(&serde_json::Value::Null)) {
        let first = text_of(&run, "nom_jour_debut");
        let last = text_of(&run, "nom_jour_fin");
        let (Some(from), to) = (crate::window::day_index(&first), crate::window::day_index(if last.is_empty() { &first } else { &last })) else { continue };
        let Some(to) = to else { continue };
        let clock = |key: &str| {
            let text = text_of(&run, key);
            crate::window::tidy_time(text.get(..5).unwrap_or(&text))
        };
        let ranges: Vec<(String, String)> = [("valeur_heure_debut_1", "valeur_heure_fin_1"), ("valeur_heure_debut_2", "valeur_heure_fin_2")].iter().filter_map(|(s, e)| Some((clock(s)?, clock(e)?))).filter(|(s, e)| s < e).collect();
        let mut day = from;
        loop {
            for (start, end) in &ranges {
                windows.push(crate::window::AdminWindow { day: crate::window::NAMES[day].into(), start: start.clone(), end: Some(end.clone()), minutes: 0, kind: None });
            }
            if day == to {
                break;
            }
            day = (day + 1) % 7;
        }
        let comment = text_of(&run, "commentaire");
        if !comment.is_empty() && !notes.contains(&comment) {
            notes.push(comment);
        }
    }
    let all = record.get("commentaire_plage_ouverture").and_then(serde_json::Value::as_str).unwrap_or("").trim().to_string();
    if !all.is_empty() && !notes.contains(&all) {
        notes.push(all);
    }
    (windows, notes)
}

/// The directory's answer (`{"total_count", "results": [record]}`) read:
/// each office with its address, number and hours.
pub fn directory_answer(json: &str) -> Result<Vec<Found>, String> {
    let answer: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let results = answer.get("results").and_then(serde_json::Value::as_array).ok_or("no results")?;
    Ok(results
        .iter()
        .take(MOST)
        .map(|record| {
            let addresses = inner(record.get("adresse").unwrap_or(&serde_json::Value::Null));
            let address = addresses.iter().find(|a| text_of(a, "type_adresse") == "Adresse").or_else(|| addresses.first()).map(|a| {
                let street = [text_of(a, "complement1"), text_of(a, "complement2"), text_of(a, "numero_voie")].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n");
                let town = [text_of(a, "code_postal"), text_of(a, "nom_commune")].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
                [street, town].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n")
            });
            let phone = inner(record.get("telephone").unwrap_or(&serde_json::Value::Null)).first().map(|t| text_of(t, "valeur")).unwrap_or_default();
            let (windows, notes) = directory_hours(record);
            Found { source: crate::opening::DIRECTORY, name: text_of(record, "nom"), address: address.unwrap_or_default(), phone, hours: if windows.is_empty() { String::new() } else { crate::opening::written(&windows, true) }, note: notes.join("; ") }
        })
        .filter(|f| !f.name.is_empty())
        .collect())
}

/// Nominatim's answer (`format=jsonv2`, `extratags=1`) read: each place,
/// its `opening_hours` as Sioul writes them, what it cannot count apart; a
/// value it does not understand at all whole in the note, no hours.
pub fn map_answer(json: &str) -> Result<Vec<Found>, String> {
    let answer: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(answer
        .iter()
        .take(MOST)
        .map(|place| {
            let display = text_of(place, "display_name");
            let name = Some(text_of(place, "name")).filter(|n| !n.is_empty()).unwrap_or_else(|| display.split(',').next().unwrap_or("").trim().to_string());
            let address = display.strip_prefix(&name).unwrap_or(&display).trim_start_matches([',', ' ']).to_string();
            let tags = place.get("extratags").cloned().unwrap_or(serde_json::Value::Null);
            let phone = [text_of(&tags, "phone"), text_of(&tags, "contact:phone")].into_iter().find(|p| !p.is_empty()).unwrap_or_default();
            let (hours, note) = crate::opening::tidied(&text_of(&tags, "opening_hours"));
            Found { source: crate::opening::MAP, name, address, phone, hours: if hours.is_empty() { String::new() } else { hours }, note }
        })
        .filter(|f| !f.name.is_empty())
        .collect())
}

/// A card's hours once `found` is picked, on `today`: its hours as found,
/// with their source and the day; the card's own kept where the source has
/// none (a lookup never empties hours that are there); what the source says
/// that Sioul cannot count added to the free line, never in the place of
/// what is there. The phone's hours stay as they are.
pub fn picked(card: &crate::opening::CardHours, found: &Found, today: jiff::civil::Date) -> crate::opening::CardHours {
    let mut out = card.clone();
    if !found.hours.trim().is_empty() {
        out.open = Some(crate::opening::Hours { value: found.hours.trim().to_string(), source: found.source.to_string(), checked: today.to_string() });
    }
    let note = found.note.trim();
    if !note.is_empty() && !out.note.contains(note) {
        out.note = if out.note.trim().is_empty() { note.to_string() } else { format!("{}; {note}", out.note.trim()) };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The directory's answer as it came on 20 November 2026 (a town hall
    /// and a prefecture, each asked once while building), turned into
    /// invented offices: its shape kept, every value made up.
    const DIRECTORY: &str = include_str!("../tests/fixtures/hours-directory.json");
    /// Nominatim's answer with `extratags`, after the documentation's own
    /// example of one (nominatim.org/release-docs/latest/api/Search/), for
    /// invented places: never asked while building.
    const MAP: &str = include_str!("../tests/fixtures/hours-nominatim.json");

    fn office(name: &str, org: &str, address: &str, card: &'static str) -> Contact {
        Contact { name: name.into(), org: org.into(), card, addresses: if address.is_empty() { Vec::new() } else { vec![crate::contacts::Labeled { label: String::new(), value: address.into() }] }, ..Contact::default() }
    }

    #[test]
    fn what_is_sent_and_to_whom() {
        // A person's card: nothing is ever looked up.
        assert_eq!(Asked::of(&office("Jeanne Exemple", "", "", "person"), true), None);
        let hall = Asked::of(&office("Mairie de Exempleville", "", "1 place de l’Exemple\n99999 Exempleville", ""), false).unwrap();
        assert!(hall.france, "a French postal code");
        let query = hall.directory_query();
        assert_eq!(query[0], ("where", "nom LIKE \"Mairie Exempleville\" AND suggest(adresse, \"code_postal 99999\")".to_string()));
        assert_eq!(hall.map_query()[0], ("q", "Mairie de Exempleville, 1 place de l’Exemple, 99999 Exempleville".to_string()));
        // Its organisation, when it has one, before its name.
        assert_eq!(Asked::of(&office("Accueil", "Example tax office", "", "org"), true).unwrap().name, "Example tax office");
        // Abroad: the map alone; no address, the person's region decides.
        assert!(!Asked::of(&office("Example office", "", "1 Example Street\nExampletown\nUnited Kingdom", "org"), true).unwrap().france);
        assert!(!Asked::of(&office("Example office", "", "Rue de l’Exemple 1\n1000 Exempleville\nBelgique", "org"), true).unwrap().france);
        assert!(Asked::of(&office("Example office", "", "", "org"), true).unwrap().france);
        assert!(!Asked::of(&office("Example office", "", "", "org"), false).unwrap().france);
        // Quotes cannot break the question.
        let odd = Asked { name: "Mairie \"x\" OR 1".into(), ..Asked::default() };
        assert_eq!(odd.directory_query()[0].1, "nom LIKE \"Mairie x OR 1\"");
    }

    #[test]
    fn the_directory_answer_read() {
        let found = directory_answer(DIRECTORY).unwrap();
        assert_eq!(found.len(), 2);
        let hall = &found[0];
        assert_eq!((hall.name.as_str(), hall.phone.as_str(), hall.address.as_str()), ("Mairie - Exempleville", "01 23 45 67 89", "Place de l'Exemple\n99999 Exempleville"));
        assert_eq!(hall.hours, "Mo-Fr 09:00-17:00; PH off");
        assert_eq!(hall.note, "État civil ouvert le jeudi de 11h30 à 19h.");
        let prefecture = &found[1];
        assert_eq!(prefecture.hours, "Mo-Fr 08:30-12:00,13:30-15:00; PH off");
        assert_eq!(prefecture.note, "Uniquement sur rendez-vous");
        assert!(directory_answer("{\"total_count\": 0, \"results\": []}").unwrap().is_empty());
        assert!(directory_answer("<html>").is_err());
    }

    #[test]
    fn the_map_answer_read() {
        let found = map_answer(MAP).unwrap();
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].name, "Example town hall");
        assert_eq!(found[0].hours, "Mo-Fr 08:30-12:00,13:30-17:00; Sa 09:00-12:00; PH off");
        assert_eq!(found[0].phone, "+33 1 23 45 67 89");
        assert!(found[0].address.starts_with("1, Place de l'Exemple"), "{}", found[0].address);
        // What it cannot count: in the note.
        assert_eq!((found[1].hours.as_str(), found[1].note.as_str()), ("Mo-Th 09:00-12:00; PH off", "Aug off"));
        // No hours on the map: none, nothing guessed.
        assert_eq!((found[2].hours.as_str(), found[2].note.as_str()), ("", ""));
        assert!(map_answer("[]").unwrap().is_empty());
    }

    #[test]
    fn a_pick_never_empties_what_is_there() {
        let today: jiff::civil::Date = "2026-11-20".parse().unwrap();
        let typed = crate::opening::CardHours { open: Some(crate::opening::Hours { value: "Mo-Fr 09:00-12:00; PH off".into(), source: "typed".into(), checked: "2025-01-02".into() }), phone: Some(crate::opening::Hours { value: "Mo 14:00-16:00".into(), ..Default::default() }), note: "Closed in August".into() };
        let found = Found { source: crate::opening::DIRECTORY, hours: "Mo-Fr 08:30-12:00; PH off".into(), note: "Uniquement sur rendez-vous".into(), ..Found::default() };
        let after = picked(&typed, &found, today);
        assert_eq!(after.open.as_ref().unwrap(), &crate::opening::Hours { value: "Mo-Fr 08:30-12:00; PH off".into(), source: crate::opening::DIRECTORY.into(), checked: "2026-11-20".into() });
        assert_eq!(after.phone, typed.phone, "the phone's hours stay");
        assert_eq!(after.note, "Closed in August; Uniquement sur rendez-vous");
        // A place with no hours: the card's hours stay, nothing doubled.
        let none = Found { source: crate::opening::MAP, note: "Closed in August".into(), ..Found::default() };
        assert_eq!(picked(&typed, &none, today), typed);
    }
}

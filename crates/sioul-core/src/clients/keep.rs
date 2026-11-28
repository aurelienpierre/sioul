// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Keeping, giving and erasing a client's file (docs/clients.md, "Keeping,
//! giving, erasing"), and joining two clients that are one (Same as…).
//!
//! - **Archive** takes a client out of the lists; Sioul archives by itself
//!   only once the person set keeping times and "archive by itself"
//!   (`due_to_archive`), and **never erases by itself**: `reached_end` only
//!   counts those whose keeping time is over, for a yearly sentence.
//! - **Export for them…**: the notes in Markdown, the rows as JSON (GDPR
//!   art. 20), the attachments, in the folder tree (`export`); the readable
//!   PDF is the view's.
//! - **Erase…** goes through the store's purge: the file and its earlier
//!   versions on this device; what stays is said (issued invoices, the
//!   cards, past events). Refused within keeping times set by a health
//!   professional. A small mark, `data/clients/erased/<id>.toml` (an id and
//!   a day, no name), travels with the part: each other device purges that
//!   client's folder when it reads it (`apply_erased`), and again whatever
//!   an older copy brings later, so that an erased client never comes back.

use super::door::Door;
use super::record::{self, Client};
use super::template::Settings;
use super::ties::{Index, TieHow};
use super::{Error, file};
use jiff::civil::Date;
use std::collections::BTreeSet;

/// Every file of a client in the store, its record last.
fn files_of(door: &dyn Door, id: &str) -> Result<Vec<String>, Error> {
    let folder = record::folder(id);
    let mut out = Vec::new();
    for f in door.folders(&folder)? {
        out.extend(door.list(&f)?);
    }
    out.extend(door.list(&folder)?.into_iter().filter(|p| !p.ends_with("/client.toml")));
    out.sort();
    out.dedup();
    out.push(record::record_path(id));
    Ok(out)
}

// ---------------------------------------------------------------- archive

/// Archived on `day`, or brought back.
pub fn archive(door: &dyn Door, id: &str, day: Option<Date>) -> Result<Client, Error> {
    record::archive(door, id, day)
}

fn years_after(day: Date, years: u32) -> Option<Date> {
    day.checked_add(jiff::Span::new().years(i64::from(years))).ok()
}

/// The clients Sioul archives by itself today: only while "archive by
/// itself" is on and an active time is set, those whose last contact is
/// that long ago. A client with no dated tie is never archived by itself.
pub fn due_to_archive(clients: &[Client], index: &Index, settings: &Settings, today: Date) -> Vec<String> {
    let p = &settings.protect;
    if !p.archive_by_itself || p.active_years == 0 {
        return Vec::new();
    }
    clients
        .iter()
        .filter(|c| c.listed() && c.archived.is_none())
        .filter(|c| {
            let facts = index.facts(&c.id, today);
            facts.next.is_none() && facts.last.and_then(|l| years_after(l, p.active_years)).is_some_and(|end| end <= today)
        })
        .map(|c| c.id.clone())
        .collect()
}

/// The clients whose keeping time set is over: said once a year with
/// **Erase them**; never erased by Sioul.
pub fn reached_end(clients: &[Client], index: &Index, settings: &Settings, today: Date) -> Vec<String> {
    let years = settings.protect.kept_years;
    if years == 0 {
        return Vec::new();
    }
    clients
        .iter()
        .filter(|c| c.listed() && c.archived.is_some())
        .filter(|c| index.facts(&c.id, today).last.and_then(|l| years_after(l, years)).is_some_and(|end| end <= today))
        .map(|c| c.id.clone())
        .collect()
}

// ---------------------------------------------------------------- erase

/// What stays once a client is erased, said before and after.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stays {
    /// Issued invoices naming it, by number: kept ten years, never erased.
    pub invoices: Vec<String>,
    /// Drafts naming it, by address: they keep their frozen client and name no client after.
    pub drafts: Vec<String>,
    /// Projects naming it, by address: their `client` names nothing after.
    pub projects: Vec<String>,
    /// Its cards: kept, offered to be removed apart.
    pub cards: Vec<String>,
    /// Past events tied to it: kept, offered apart.
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Erased {
    pub removed: usize,
    pub stays: Stays,
}

/// What stays if client `id` is erased.
pub fn stays(client: &Client, index: &Index) -> Stays {
    let mut out = Stays { cards: client.cards.clone(), ..Stays::default() };
    for tie in index.of_client(&client.id) {
        match tie.how {
            TieHow::Invoice if tie.other.starts_with("sioul:invoice/") => out.invoices.push(tie.title.clone()),
            TieHow::Invoice => out.drafts.push(tie.other.clone()),
            TieHow::Project => out.projects.push(tie.other.clone()),
            TieHow::Appointment => out.events.push(tie.other.clone()),
            _ => {}
        }
    }
    for list in [&mut out.invoices, &mut out.drafts, &mut out.projects] {
        list.sort();
        list.dedup();
    }
    out.events.sort();
    out.events.dedup();
    out
}

/// Client `id` erased through the store's purge: everything ever written
/// under its folder, parts removed earlier included, with their earlier
/// versions; the records of clients joined into it; its profile in
/// `profiles` (`sioul-clients.toml`), so that no device makes it again. What
/// stays is returned. Refused within the keeping time set, for a health
/// professional's file (`Error::KeptUntil`).
pub fn erase(door: &dyn Door, id: &str, settings: &Settings, index: &Index, today: Date, profiles: Option<&std::path::Path>) -> Result<Erased, Error> {
    let client = record::read(door, id)?.ok_or(Error::NotFound)?;
    let p = &settings.protect;
    if p.health_profession() && p.kept_years > 0
        && let Some(end) = index.facts(id, today).last.and_then(|l| years_after(l, p.kept_years))
        && end > today
        && file::has_file(door, id)?
    {
        return Err(Error::KeptUntil(end));
    }
    let stays = stays(&client, index);
    let joined: Vec<Client> = record::every(door)?.items.into_iter().filter(|c| c.joined == id).collect();
    let all: Vec<&Client> = std::iter::once(&client).chain(joined.iter()).collect();
    // The marks first: an erase cut half way is finished by `apply_erased`, here and on every device.
    for c in &all {
        door.write(&erased_path(&c.id), format!("day = \"{today}\"\n").as_bytes(), super::door::Expect::Any)?;
    }
    super::moving::note_moved(door, &all.iter().map(|c| format!("client:{}", c.id)).collect::<Vec<_>>())?;
    let mut removed = 0;
    for c in &all {
        removed += door.purge_under(&record::folder(&c.id))?;
    }
    // Their profiles out of the file: no move makes them again, here or on a device that reads it.
    if let Some(file) = profiles {
        for c in all.iter().filter(|c| !c.profile.is_empty()) {
            super::remove(file, &c.profile).map_err(Error::profiles)?;
        }
    }
    // An extension's person erased stays erased, the joined ones' too: no refresh makes them clients again.
    for c in all.iter().filter(|c| !c.mark.is_empty()) {
        super::extension::mark_declined(door, &c.mark, today)?;
    }
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(Erased { removed, stays })
}

/// The folder of the marks of clients erased (`erase`): not a client's.
pub const ERASED: &str = "erased";

/// Where the mark of client `id` erased is kept.
pub fn erased_path(id: &str) -> String {
    format!("{}/{ERASED}/{id}.toml", record::ROOT)
}

/// Every client erased here or on another device, by id.
pub fn erased(door: &dyn Door) -> Result<Vec<String>, Error> {
    let folder = format!("{}/{ERASED}", record::ROOT);
    Ok(door.list(&folder)?.iter().filter_map(|p| super::door::name_of(p).strip_suffix(".toml")).filter(|id| super::address::valid_id(id)).map(str::to_string).collect())
}

/// Which erased clients this device purged already, with the day: this
/// device's own note (in its sealed state, never shared), so that each is
/// purged here once, its earlier versions with it, and again only when a
/// file of it comes back.
pub const PURGED_HERE: &str = "state/clients/purged.toml";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct PurgedHere {
    purged: std::collections::BTreeMap<String, String>,
}

/// What a mark of a client erased asks, on this device: everything of that
/// client's folder purged, its earlier versions with it (another device's
/// erase, whose removals arrived as removals; an erase cut half way), once,
/// and again whenever a file of it comes back (an older copy, a backup put
/// back). The ids purged now.
pub fn apply_erased(door: &dyn Door) -> Result<Vec<String>, Error> {
    let erased = erased(door)?;
    if erased.is_empty() {
        return Ok(Vec::new());
    }
    let here: PurgedHere = match door.read_text(PURGED_HERE)? {
        Some((text, _)) => toml::from_str(&text).unwrap_or_default(),
        None => PurgedHere::default(),
    };
    let mut purged = Vec::new();
    for id in erased {
        let folder = record::folder(&id);
        let back = !door.list(&folder)?.is_empty() || !door.folders(&folder)?.is_empty();
        if back || !here.purged.contains_key(&id) {
            door.purge_under(&folder)?;
            purged.push(id);
        }
    }
    if !purged.is_empty() {
        let day = jiff::Zoned::now().date().to_string();
        door.update(PURGED_HERE, &mut |now| {
            let mut file: PurgedHere = now.and_then(|b| std::str::from_utf8(b).ok()).and_then(|t| toml::from_str(t).ok()).unwrap_or_default();
            for id in &purged {
                file.purged.entry(id.clone()).or_insert_with(|| day.clone());
            }
            toml::to_string(&file).map(|t| Some(t.into_bytes())).map_err(|_| Error::Refused("record"))
        })?;
        crate::changes::said(crate::changes::Kind::Clients);
    }
    Ok(purged)
}

// ---------------------------------------------------------------- export

/// A name safe as a file's in any system, never empty.
fn safe(name: &str, fallback: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c }).collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    let cleaned: String = cleaned.chars().take(100).collect();
    if cleaned.is_empty() { fallback.to_string() } else { cleaned }
}

/// A file's path made unique among those taken: "Notes (2).md".
fn unique(taken: &mut BTreeSet<String>, path: String) -> String {
    if taken.insert(path.clone()) {
        return path;
    }
    let (stem, ext) = match path.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && !e.contains('/') => (s.to_string(), format!(".{e}")),
        _ => (path.clone(), String::new()),
    };
    let found = (2..).map(|n| format!("{stem} ({n}){ext}")).find(|p| !taken.contains(p)).expect("a free name");
    taken.insert(found.clone());
    found
}

/// What an export holds: each file's path in the folder tree, and its bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Export {
    pub files: Vec<(String, Vec<u8>)>,
}

/// A TOML value as JSON.
fn json(value: &toml::Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

/// Export for them…: `client.json` (its name and billing identity), the
/// description, the notes in Markdown and the attachments in their folders
/// (named by the template's words), and each kind's rows as JSON, its
/// fields by their names. A file that does not open is left out and counted
/// in `client.json`.
pub fn export(door: &dyn Door, id: &str, settings: &Settings, profiles: &super::Clients, contacts: &[crate::contacts::Contact]) -> Result<Export, Error> {
    let client = record::find(door, id)?.ok_or(Error::NotFound)?;
    let id = client.id.clone();
    let mut out = Export::default();
    let mut taken = BTreeSet::new();
    let folder_name = |folder: &str| -> String {
        if folder.is_empty() {
            return String::new();
        }
        let def = settings.folder.get(folder).or_else(|| client.own_folders.get(folder));
        let name = safe(def.map_or(folder, |f| f.name.as_str()), folder);
        match def.map(|f| f.parent.as_str()).filter(|p| !p.is_empty()) {
            Some(parent) => format!("{}/{name}/", safe(settings.folder.get(parent).map_or(parent, |f| f.name.as_str()), parent)),
            None => format!("{name}/"),
        }
    };
    // A damaged part costs itself, said in `client.json`, never the whole export.
    let mut damaged = 0;
    match file::description(door, &id) {
        Ok(Some(d)) => out.files.push((unique(&mut taken, "description.md".into()), d.text.into_bytes())),
        Ok(None) => {}
        Err(Error::Damaged(_)) => damaged += 1,
        Err(e) => return Err(e),
    }
    let notes = file::notes(door, &id)?;
    damaged += notes.damaged;
    for head in notes.items {
        let note = match file::note(door, &id, &head.id) {
            Ok(Some(note)) => note,
            Ok(None) => continue,
            Err(Error::Damaged(_)) => {
                damaged += 1;
                continue;
            }
            Err(e) => return Err(e),
        };
        let name = safe(if note.title.trim().is_empty() { &note.id } else { &note.title }, &note.id);
        let path = unique(&mut taken, format!("{}{name}.md", folder_name(&note.folder)));
        let body = if note.title.trim().is_empty() { note.text.clone() } else { format!("# {}\n\n{}", note.title.trim(), note.text) };
        out.files.push((path, body.into_bytes()));
    }
    let attachments = file::attachments(door, &id)?;
    damaged += attachments.damaged;
    for a in attachments.items {
        match file::attachment_bytes(door, &id, &a.id) {
            Ok(bytes) => {
                let path = unique(&mut taken, format!("{}{}", folder_name(&a.folder), safe(&a.name, &a.id)));
                out.files.push((path, bytes));
            }
            Err(Error::Damaged(_)) | Err(Error::NotFound) => damaged += 1,
            Err(e) => return Err(e),
        }
    }
    let heads = file::rows(door, &id)?;
    damaged += heads.damaged;
    let kinds: BTreeSet<String> = heads.items.iter().map(|h| h.kind.clone()).collect();
    for kind in kinds {
        let rows = file::rows_of_kind(door, &id, &kind)?;
        damaged += rows.damaged;
        let def = settings.kind.get(&kind);
        let list: Vec<serde_json::Value> = rows
            .items
            .iter()
            .map(|r| {
                let fields: serde_json::Map<String, serde_json::Value> = r.fields.iter().map(|(f, v)| (def.and_then(|k| k.field.get(f)).map_or_else(|| f.clone(), |d| d.name.clone()), json(v))).collect();
                serde_json::json!({ "id": r.id, "when": r.when, "fields": fields })
            })
            .collect();
        let name = safe(def.map_or(kind.as_str(), |k| k.name.as_str()), &kind);
        let path = unique(&mut taken, format!("records/{name}.json"));
        out.files.push((path, serde_json::to_vec_pretty(&serde_json::json!({ "kind": def.map_or(kind.as_str(), |k| k.name.as_str()), "rows": list })).unwrap_or_default()));
    }
    // Its billing identity where it lives now: its profile's until the gate (`billing_of`).
    let billing = serde_json::to_value(client.billing_of(profiles)).unwrap_or(serde_json::Value::Null);
    let head = serde_json::json!({ "name": client.shown_name(contacts), "billing": billing, "since_archived": client.archived.map(|d| d.to_string()), "left_out": damaged });
    out.files.insert(0, (unique(&mut taken, "client.json".into()), serde_json::to_vec_pretty(&head).unwrap_or_default()));
    Ok(out)
}

// ---------------------------------------------------------------- Same as…

/// Same as…: two clients that are one human joined. **The smaller id is
/// always kept**, whichever way the person said it, so that two joins said
/// in opposite directions on two devices end as one client. The other's
/// cards, rows, notes, attachments and ties move to the one kept; its
/// description is added below; every key of its record is merged (the mark
/// kept from agents if either has it, the earlier agreement, its own
/// folders, its profile, mark and origin where the kept one has none, its
/// billing where the kept one has none); clients joined into it now redirect
/// to the kept one; its record only redirects from then on (`joined`).
pub fn same_as(door: &dyn Door, a: &str, b: &str) -> Result<Client, Error> {
    if a == b {
        return Err(Error::Refused("the same client"));
    }
    let (keep, other) = if a < b { (a, b) } else { (b, a) };
    let from = record::read(door, other)?.ok_or(Error::NotFound)?;
    let into = record::read(door, keep)?.ok_or(Error::NotFound)?;
    let (src, dst) = (record::folder(other), record::folder(keep));
    for path in files_of(door, other)? {
        let rest = &path[src.len()..];
        if rest == "/client.toml" || rest.starts_with("/ties/") || rest == "/rows.toml" {
            continue;
        }
        let target = format!("{dst}{rest}");
        if rest == "/description.md" {
            let theirs = door.read_text(&path)?.map(|(t, _)| t).unwrap_or_default();
            door.update(&target, &mut |now| {
                let mine = now.map(|b| String::from_utf8_lossy(b).to_string()).unwrap_or_default();
                if mine.contains(theirs.trim()) {
                    return Ok(None);
                }
                Ok(Some(if mine.trim().is_empty() { theirs.clone() } else { format!("{}\n\n---\n\n{theirs}", mine.trim_end()) }.into_bytes()))
            })?;
        } else if door.read(&target)?.is_none()
            && let Some(opened) = door.read(&path)?
        {
            door.write(&target, &opened.bytes, super::door::Expect::Absent)?;
        }
    }
    super::ties::move_ties(door, other, keep)?;
    file::rebuild_rows(door, keep)?;
    let fill = |mine: &str, theirs: &str| (mine.trim().is_empty() && !theirs.trim().is_empty()).then(|| theirs.to_string());
    let agreed = match (into.agreed, from.agreed) {
        (Some(x), Some(y)) => Some(Some(x.min(y))),
        (None, Some(y)) => Some(Some(y)),
        _ => None,
    };
    let merged = record::Change {
        add_cards: from.cards.clone(),
        name: fill(&into.name, &from.name),
        profile: fill(&into.profile, &from.profile),
        mark: fill(&into.mark, &from.mark),
        kept_from_agents: (from.kept_from_agents && !into.kept_from_agents).then_some(true),
        agreed,
        own_folders: from.own_folders.iter().filter(|(id, _)| !into.own_folders.contains_key(*id)).map(|(id, f)| (id.clone(), Some(f.clone()))).collect(),
        billing_whole: (into.billing == record::Billing::default() && from.billing != record::Billing::default()).then(|| from.billing.clone()),
        ..record::Change::default()
    };
    record::change(door, keep, &merged)?;
    if into.from.is_empty() && !from.from.is_empty() {
        door.update(&record::record_path(keep), &mut |now| {
            let Some(now) = now else { return Ok(None) };
            let mut doc: toml_edit::DocumentMut = String::from_utf8_lossy(now).parse().map_err(|_| Error::Damaged(record::record_path(keep)))?;
            doc["from"] = toml_edit::value(from.from.as_str());
            Ok(Some(doc.to_string().into_bytes()))
        })?;
    }
    for joined in record::every(door)?.items.iter().filter(|c| c.joined == other) {
        record::change(door, &joined.id, &record::Change { joined: Some(keep.to_string()), ..record::Change::default() })?;
    }
    // Removed, never purged: a device that has not joined yet may have
    // changed one of them, and its earlier versions stay to go back to (the
    // second review, D); an erase purges joined folders whole.
    for path in files_of(door, other)? {
        if !path.ends_with("/client.toml") {
            door.remove(&path)?;
        }
    }
    record::change(door, other, &record::Change { joined: Some(keep.to_string()), ..record::Change::default() })?;
    record::read(door, keep)?.ok_or(Error::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::address::Address;
    use crate::clients::door::Memory;
    use crate::clients::record::Billing;
    use crate::clients::ties::{self, Said, Sources};
    use crate::clients::template::{FieldDef, FieldType, FolderDef, KindDef};

    fn settings() -> Settings {
        let mut s = Settings { kinds_version: 1, folders_version: 1, ..Settings::default() };
        s.folder.insert("consents".into(), FolderDef { name: "Consents".into(), place: 1, ..FolderDef::default() });
        let mut kind = KindDef { name: "Treatment".into(), ..KindDef::default() };
        kind.field.insert("skin".into(), FieldDef { name: "Skin".into(), kind: FieldType::Text, health: true, ..FieldDef::default() });
        s.kind.insert("treatment".into(), kind);
        s
    }

    /// Export gives the notes in Markdown, the rows as JSON with their fields'
    /// names and the attachments, in the folder tree.
    #[test]
    fn an_export_holds_the_whole_file_readably() {
        let door = Memory::new();
        let s = settings();
        let c = record::make(&door, "Marie Exemple", &[], Billing::default()).unwrap();
        file::write_description(&door, &c.id, "Allergic to latex.", None, &s).unwrap();
        file::write_note(&door, &c.id, &file::Note { title: "First visit".into(), folder: "consents".into(), text: "Signed.\n".into(), ..Default::default() }, &s).unwrap();
        file::attach(&door, &c.id, "Consent: 2026.pdf", "consents", "", "application/pdf", b"%PDF", &s).unwrap();
        file::add_row(&door, &c.id, "treatment", "2026-11-03", [("skin".to_string(), toml::Value::from("Redness"))].into(), &s).unwrap();
        let export = export(&door, &c.id, &s, &crate::clients::Clients::default(), &[]).unwrap();
        let paths: Vec<&str> = export.files.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths, vec!["client.json", "description.md", "Consents/First visit.md", "Consents/Consent- 2026.pdf", "records/Treatment.json"]);
        let rows: serde_json::Value = serde_json::from_slice(&export.files[4].1).unwrap();
        assert_eq!(rows["rows"][0]["fields"]["Skin"], "Redness");
    }

    /// Erase goes through the purge and says what stays; refused within the
    /// keeping time of a health professional's file; Sioul archives by
    /// itself only when told, and never erases by itself.
    #[test]
    fn erasing_keeping_and_archiving() {
        let door = Memory::new();
        let mut s = settings();
        let c = record::make(&door, "", &["c-1".into()], Billing::default()).unwrap();
        file::write_description(&door, &c.id, "x", None, &s).unwrap();
        ties::tie(&door, &Address::Client { id: c.id.clone() }, "sioul:event/e-1", &Said { day: Some("2026-11-03".parse().unwrap()), title: "Facial".into() }).unwrap();
        let invoice = crate::invoice::Invoice { number: "2026-014".into(), date: "2026-11-03".into(), client_ref: c.uri(), ..Default::default() };
        let invoices = vec![invoice];
        let clients = record::list(&door).unwrap().items;
        let index = ties::Index::build(&door, &clients, &Sources { invoices: &invoices, ..Sources::default() });
        let today: Date = "2027-01-10".parse().unwrap();
        assert!(due_to_archive(&clients, &index, &s, "2040-01-01".parse().unwrap()).is_empty(), "off until set");
        s.protect.archive_by_itself = true;
        s.protect.active_years = 3;
        assert!(due_to_archive(&clients, &index, &s, today).is_empty());
        assert_eq!(due_to_archive(&clients, &index, &s, "2029-11-04".parse().unwrap()), vec![c.id.clone()]);
        s.protect.health_profession = "yes".into();
        s.protect.kept_years = 20;
        assert_eq!(erase(&door, &c.id, &s, &index, today, None), Err(Error::KeptUntil("2046-11-03".parse().unwrap())));
        s.protect.health_profession = "no".into();
        // A note removed earlier keeps its earlier versions until the erase; a client joined into it, its record.
        let gone = file::write_note(&door, &c.id, &file::Note { text: "removed before".into(), ..Default::default() }, &s).unwrap();
        file::remove_note(&door, &c.id, &gone.id).unwrap();
        let joined = record::make(&door, "Joined", &[], Billing::default()).unwrap();
        record::change(&door, &joined.id, &record::Change { joined: Some(c.id.clone()), ..Default::default() }).unwrap();
        // Its profile in the file goes too, so that no device's move makes it again.
        let records = std::env::temp_dir().join(format!("sioul-clients-erase-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&records);
        std::fs::create_dir_all(&records).unwrap();
        let profiles = crate::clients::file_in(&records);
        crate::clients::save(&profiles, &crate::clients::Profile { id: "p-1".into(), name: "Studio".into(), ..Default::default() }).unwrap();
        record::change(&door, &c.id, &record::Change { profile: Some("p-1".into()), ..Default::default() }).unwrap();
        let erased = erase(&door, &c.id, &s, &index, today, Some(&profiles)).unwrap();
        assert_eq!(erased.stays, Stays { invoices: vec!["2026-014".into()], cards: vec!["c-1".into()], events: vec!["sioul:event/e-1".into()], ..Stays::default() });
        assert_eq!(erased.removed, 4, "the description, the tie, the record, and the joined client's record");
        // Nothing of it left here but the marks that travel: erased (an id and a day each), and moved once.
        let mut marks = vec![erased_path(&c.id), erased_path(&joined.id), super::super::moving::MOVED.to_string()];
        marks.sort();
        assert_eq!(door.paths(), marks, "nothing of it left here");
        assert!(door.read_text(&erased_path(&c.id)).unwrap().unwrap().0.trim() == format!("day = \"{today}\""), "an id and a day, no name");
        let mut both = vec![c.id.clone(), joined.id.clone()];
        both.sort();
        assert_eq!(apply_erased(&door).unwrap(), both, "purged once more here, then noted");
        assert!(apply_erased(&door).unwrap().is_empty(), "once");
        assert!(record::make(&door, "Studio", &["c-1".into()], Billing::default()).is_ok_and(|again| again.id != c.id), "a card whose client was erased gives a client of its own id");
        assert!(door.removed_kept().is_empty(), "nor the earlier versions of what was removed before");
        assert!(crate::clients::Clients::read_file(&records).unwrap().list.is_empty(), "its profile gone");
        let _ = std::fs::remove_dir_all(&records);
    }

    /// Same as…: one client of two, every part moved and every key merged,
    /// the smaller id kept whichever way it was said, the other redirecting.
    #[test]
    fn same_as_joins_two_clients() {
        let s = settings();
        let make = |door: &Memory| {
            let a = record::make(door, "Marie", &["c-1".into()], Billing::default()).unwrap();
            let billing = Billing { name: "Marie Exemple EI".into(), ..Billing::default() };
            let b = record::make(door, "", &["c-2".into()], billing).unwrap();
            record::change(door, &b.id, &record::Change { kept_from_agents: Some(true), agreed: Some(Some("2026-10-01".parse().unwrap())), own_folders: vec![("x-rays".into(), Some(crate::clients::template::FolderDef { name: "X-rays".into(), ..Default::default() }))], ..Default::default() }).unwrap();
            file::write_description(door, &a.id, "From the computer.", None, &s).unwrap();
            file::write_description(door, &b.id, "From the phone.", None, &s).unwrap();
            let row = file::add_row(door, &b.id, "treatment", "2026-11-03", [("skin".to_string(), toml::Value::from("Dry"))].into(), &s).unwrap();
            ties::tie(door, &Address::Row { client: b.id.clone(), kind: "treatment".into(), row: row.id.clone() }, "sioul:event/e-1", &Said::default()).unwrap();
            (a, b, row)
        };
        let (here, there) = (Memory::new(), Memory::new());
        let (a, b, row) = make(&here);
        make(&there);
        let kept = same_as(&here, &a.id, &b.id).unwrap();
        let kept_there = same_as(&there, &b.id, &a.id).unwrap();
        assert_eq!(kept.id, kept_there.id, "the smaller id, whichever way it was said");
        assert_eq!(kept.id, a.id.clone().min(b.id.clone()));
        let other = if kept.id == a.id { &b.id } else { &a.id };
        let mut cards = kept.cards.clone();
        cards.sort();
        assert_eq!(cards, vec!["c-1".to_string(), "c-2".to_string()]);
        assert!(kept.kept_from_agents, "kept from agents if either was");
        assert_eq!(kept.agreed.map(|d| d.to_string()), Some("2026-10-01".into()));
        assert!(kept.own_folders.contains_key("x-rays"));
        assert_eq!(kept.billing.name, "Marie Exemple EI", "filled where empty");
        assert_eq!(kept.name, "Marie", "filled where empty");
        assert_eq!(record::find(&here, other).unwrap().unwrap().id, kept.id, "the other redirects");
        assert_eq!(record::list(&here).unwrap().items.len(), 1);
        assert_eq!(file::rows(&here, &kept.id).unwrap().items.len(), 1);
        let text = file::description(&here, &kept.id).unwrap().unwrap().text;
        assert!(text.contains("From the computer.") && text.contains("From the phone."), "{text}");
        let t = ties::ties_of(&here, &kept.id).unwrap().items;
        assert_eq!(t[0].at, crate::clients::address::row_uri(&kept.id, "treatment", &row.id));
        assert!(ties::ties_of(&here, other).unwrap().items.is_empty());
    }
}

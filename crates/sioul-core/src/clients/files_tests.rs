// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The clients' files on invented data: the file's parts, every evolution
//! of the templates, ties from both ends.

use super::door::{Door, Expect, Memory};
use super::file::{self, RowChange, Value};
use super::record::{self, Billing};
use super::template::{self, FolderState, Settings};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// A configuration file of a test's own.
fn config_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sioul-clients-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "# Mine.\nlanguage = \"en\"\n").unwrap();
    path
}

fn settings(path: &std::path::Path) -> Settings {
    let text = std::fs::read_to_string(path).unwrap();
    let config: crate::config::Config = toml::from_str(&text).unwrap();
    config.clients
}

fn fields(pairs: &[(&str, toml::Value)]) -> BTreeMap<String, toml::Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// A file's parts: nothing until used; each part a file of its own; a
/// damaged part costs that one.
#[test]
fn a_file_appears_when_used_and_each_part_stands_alone() {
    let door = Memory::new();
    let path = config_file("parts");
    let client = record::make(&door, "Marie Exemple", &[], Billing::default()).unwrap();
    assert!(!file::has_file(&door, &client.id).unwrap(), "lean: no file until used");
    let none = settings(&path);
    // A description, a note, an attachment: no template needed.
    file::write_description(&door, &client.id, "Allergic to latex.\n", None, &none).unwrap();
    assert!(file::has_file(&door, &client.id).unwrap());
    let note = file::write_note(&door, &client.id, &file::Note { title: "First: a visit".into(), text: "Came on time.\n".into(), ties: vec!["sioul:event/e-1".into()], ..Default::default() }, &none).unwrap();
    let read = file::note(&door, &client.id, &note.id).unwrap().unwrap();
    assert_eq!((read.title.as_str(), read.text.as_str(), read.ties.clone()), ("First: a visit", "Came on time.\n", vec!["sioul:event/e-1".to_string()]));
    let second = file::write_note(&door, &client.id, &file::Note { title: "Second".into(), text: "x".into(), ..Default::default() }, &none).unwrap();
    let pdf = file::attach(&door, &client.id, "Consent 2026.pdf", "", "", "application/pdf", b"%PDF-1.7", &none).unwrap();
    assert_eq!(file::attachment_bytes(&door, &client.id, &pdf.id).unwrap(), b"%PDF-1.7");
    // A copy of a message's attachment names its source; one without is refused.
    let copy = file::attach_copy(&door, &client.id, "Prescription.pdf", "", "", "application/pdf", b"%PDF", "mid:rx@example.org", &none).unwrap();
    assert_eq!(file::attachments(&door, &client.id).unwrap().items.iter().find(|a| a.id == copy.id).map(|a| a.source.as_str()), Some("mid:rx@example.org"));
    assert_eq!(file::attach_copy(&door, &client.id, "x", "", "", "", b"x", " ", &none), Err(super::Error::Refused("a copy names its source")));
    // No name on the disk: every path is made of ids.
    for p in door.paths() {
        assert!(!p.contains("Consent") && !p.contains("Marie") && !p.contains("visit"), "{p}");
    }
    // A damaged note costs that note.
    door.damage(&file::note_path(&client.id, &second.id));
    let notes = file::notes(&door, &client.id).unwrap();
    assert_eq!((notes.items.len(), notes.damaged), (1, 1));
    // Written over a change made meanwhile: refused, never written over.
    let stale = file::Note { text: "older".into(), ..read.clone() };
    file::write_note(&door, &client.id, &file::Note { text: "newer".into(), ..read }, &none).unwrap();
    assert_eq!(file::write_note(&door, &client.id, &stale, &none), Err(super::Error::Changed));
}

/// Every evolution of the folder tree: added (everywhere, empty), renamed
/// (id stays), removed while a file holds something (stays there only),
/// unknown to an older device.
#[test]
fn the_folder_tree_evolves_without_rewriting_files() {
    let path = config_file("folders");
    let consents = template::add_folder(&path, "Consents", "").unwrap();
    let pictures = template::add_folder(&path, "Pictures", "").unwrap();
    assert_eq!((consents.id.as_str(), settings(&path).folders_version), ("consents", 2));
    assert!(template::add_folder(&path, "Inside pictures", "nope").is_err(), "one level, under a top folder");
    template::rename_folder(&path, &consents.id, "Signed consents").unwrap();
    let s = settings(&path);
    assert_eq!((s.folder["consents"].name.as_str(), s.folders_version), ("Signed consents", 3));
    template::remove_folder(&path, &pictures.id).unwrap();
    let s = settings(&path);
    let used: BTreeSet<String> = ["pictures".to_string(), "from-a-newer-device".to_string()].into();
    let shown = template::folders_of(&BTreeMap::new(), &s, &used);
    let state = |id: &str| shown.iter().find(|f| f.id == id).map(|f| f.state);
    assert_eq!(state("consents"), Some(FolderState::Template));
    assert_eq!(state("pictures"), Some(FolderState::NoLongerInTemplate), "kept where it holds something");
    assert_eq!(state("from-a-newer-device"), Some(FolderState::NotInTemplateYet));
    assert_eq!(template::folders_of(&BTreeMap::new(), &s, &BTreeSet::new()).iter().filter(|f| f.id == "pictures").count(), 0, "gone where it holds nothing");
    // Move its contents…: a note and an attachment out of the removed folder.
    let door = Memory::new();
    let client = record::make(&door, "", &[], Billing::default()).unwrap();
    file::write_note(&door, &client.id, &file::Note { folder: "pictures".into(), text: "x".into(), ..Default::default() }, &s).unwrap();
    file::attach(&door, &client.id, "a.jpg", "pictures", "", "image/jpeg", b"jpg", &s).unwrap();
    assert_eq!(file::move_folder_contents(&door, &client.id, "pictures", "consents", &s).unwrap(), 2);
    assert!(file::notes(&door, &client.id).unwrap().items.iter().all(|n| n.folder == "consents"));
    assert!(file::attachments(&door, &client.id).unwrap().items.iter().all(|a| a.folder == "consents"));
    // Undo puts the part back, the version raised again.
    let removed = template::remove_folder(&path, "consents").unwrap();
    template::undo(&path, &removed).unwrap();
    let s = settings(&path);
    assert_eq!((s.folder["consents"].removed, s.folders_version), (0, 6));
    assert!(std::fs::read_to_string(&path).unwrap().contains("# Mine."), "comments kept");
}

/// Every evolution of a kind: a field added (no default written), renamed,
/// its type changed (a value kept as written), removed and brought back; a
/// choice added, renamed, archived; the kind archived; each row keeping
/// the version it was written with; an older device reading only.
#[test]
fn the_kinds_evolve_and_rows_keep_their_version() {
    let door = Memory::new();
    let path = config_file("kinds");
    let client = record::make(&door, "", &["c-1".into()], Billing::default()).unwrap();
    let kind = template::add_kind(&path, "Acte de soin").unwrap().id;
    assert_eq!(kind, "acte-de-soin");
    let notes = template::add_field(&path, &kind, "Notes", "text").unwrap().id;
    let minutes = template::add_field(&path, &kind, "Minutes", "line").unwrap().id;
    assert!(template::add_field(&path, &kind, "Odd", "hologram").is_err(), "a type this Sioul does not know is never made");
    let s = settings(&path);
    let old = file::add_row(&door, &client.id, &kind, "2026-11-03T16:00:00+01:00", fields(&[(&notes, "Fine".into()), (&minutes, "forty".into())]), &s).unwrap();
    assert_eq!(old.schema, s.kinds_version);
    assert_eq!(file::add_row(&door, &client.id, &kind, "2026-11-04", fields(&[(&notes, " ".into())]), &s), Err(super::Error::Empty), "a row saved untouched is not kept");

    // A field added: the older row shows it empty, nothing written into it.
    let price = template::add_field(&path, &kind, "Price", "amount").unwrap().id;
    let s = settings(&path);
    let before = door.read(&file::row_path(&client.id, &kind, &old.id)).unwrap().unwrap();
    let row = file::row(&door, &client.id, &kind, &old.id).unwrap().unwrap();
    assert_eq!(row.value(s.kind.get(&kind), &price), Value::Empty);
    assert_eq!(door.read(&file::row_path(&client.id, &kind, &old.id)).unwrap().unwrap(), before, "no default written");
    // Renamed: its id, and the value, stay.
    template::rename_field(&path, &kind, &notes, "Session notes").unwrap();
    let s = settings(&path);
    assert_eq!(row.value(s.kind.get(&kind), &notes), Value::Text("Fine".into()));
    // Its type changed: a value that no longer fits is kept as written.
    template::retype_field(&path, &kind, &minutes, "number").unwrap();
    let s = settings(&path);
    assert_eq!(row.value(s.kind.get(&kind), &minutes), Value::AsWrittenBefore("forty".into()));
    assert_eq!(file::row(&door, &client.id, &kind, &old.id).unwrap().unwrap().fields[&minutes], toml::Value::String("forty".into()), "kept as written");
    // Removed, then brought back: the value was kept all along.
    template::remove_field(&path, &kind, &notes).unwrap();
    assert!(settings(&path).kind[&kind].field[&notes].removed > 0);
    template::bring_back_field(&path, &kind, &notes).unwrap();
    let s = settings(&path);
    assert_eq!((s.kind[&kind].field[&notes].removed, row.value(s.kind.get(&kind), &notes)), (0, Value::Text("Fine".into())));
    // Choices: added, renamed, archived.
    let skin = template::add_field(&path, &kind, "Skin", "choice").unwrap().id;
    let dry = template::add_choice(&path, &kind, &skin, "Dry").unwrap().id;
    template::rename_choice(&path, &kind, &skin, &dry, "Very dry").unwrap();
    template::archive_choice(&path, &kind, &skin, &dry).unwrap();
    let s = settings(&path);
    let choice = &s.kind[&kind].field[&skin].choice[&dry];
    assert_eq!((choice.name.as_str(), choice.archived > 0), ("Very dry", true));
    // Each row keeps the version it was written with; a later one has its own.
    let later = file::add_row(&door, &client.id, &kind, "2026-11-10", fields(&[(&skin, "dry".into()), (&minutes, 30.into())]), &s).unwrap();
    assert!(later.schema > old.schema);
    assert_eq!(file::row(&door, &client.id, &kind, &old.id).unwrap().unwrap().schema, old.schema);
    assert_eq!(later.value(s.kind.get(&kind), &skin), Value::Choices(vec!["dry".into()]), "an archived choice still reads");
    // A kind removed is archived: its rows stay; no new one is made.
    template::archive_kind(&path, &kind).unwrap();
    let s = settings(&path);
    assert_eq!(file::rows(&door, &client.id).unwrap().items.len(), 2);
    assert_eq!(file::add_row(&door, &client.id, &kind, "2026-11-11", fields(&[(&notes, "x".into())]), &s), Err(super::Error::NotFound));
    template::bring_back_kind(&path, &kind).unwrap();
    let s = settings(&path);

    // An older device: its template lacks a field the row holds; it reads, never writes.
    let mut older = s.clone();
    older.kind.get_mut(&kind).unwrap().field.remove(&skin);
    let newer_row = file::row(&door, &client.id, &kind, &later.id).unwrap().unwrap();
    assert!(newer_row.read_only(&older));
    // A version number alone never makes a row read only: it travels as a setting and may go back.
    assert!(!newer_row.read_only(&Settings { kinds_version: 1, ..s.clone() }));
    // A choice its list lacks does.
    let mut no_choice = s.clone();
    no_choice.kind.get_mut(&kind).unwrap().field.get_mut(&skin).unwrap().choice.clear();
    assert!(newer_row.read_only(&no_choice));
    assert_eq!(file::change_row(&door, &client.id, &kind, &later.id, &RowChange { fields: vec![(notes.clone(), Some("x".into()))], ..Default::default() }, &older), Err(super::Error::NewerTemplate));
    // A field this device does not know at all: "fields of a newer template".
    let mut unknown_kind = s.kind[&kind].clone();
    unknown_kind.field.remove(&skin);
    assert!(matches!(newer_row.value(Some(&unknown_kind), &skin), Value::Unknown(_)));
    // Written field by field: what a newer device wrote stays.
    door.update(&file::row_path(&client.id, &kind, &old.id), &mut |now| Ok(Some(format!("future = \"kept\"\n{}", String::from_utf8_lossy(now.unwrap())).into_bytes()))).unwrap();
    file::change_row(&door, &client.id, &kind, &old.id, &RowChange { fields: vec![(minutes.clone(), Some(45.into()))], ..Default::default() }, &s).unwrap();
    let text = door.read_text(&file::row_path(&client.id, &kind, &old.id)).unwrap().unwrap().0;
    assert!(text.contains("future = \"kept\"") && text.contains("Fine"), "{text}");
    assert_eq!(file::row(&door, &client.id, &kind, &old.id).unwrap().unwrap().value(s.kind.get(&kind), &minutes), Value::Number { value: 45.0, unit: String::new() });
}

/// Starts by trade are data: each writes its kinds and folders where their
/// ids are free, in the window's language, and proposes, never sets, what protects.
#[test]
fn a_start_by_trade_is_data() {
    let starts = template::starts("fr");
    assert_eq!(starts.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["beauty", "care", "consulting", "craft"]);
    assert_eq!(starts[1].name, "Cabinet de soins");
    let path = config_file("starts");
    template::add_kind(&path, "Session").unwrap();
    let (info, written) = template::start_from(&path, "care", "fr").unwrap();
    assert_eq!(info.proposes, vec!["keep-tied", "stay-here", "keeping"]);
    assert!(!written.contains(&"kind.session".to_string()), "the person's own kind kept, never written over");
    assert!(written.contains(&"folder.consents".to_string()));
    let s = settings(&path);
    assert_eq!((s.kind["session"].name.as_str(), s.folder["received"].name.as_str(), s.folder["received"].health), ("Session", "Documents reçus", true));
    assert_eq!(s.protect, super::protect::Protect::default(), "nothing set: proposed only");
    let (_, written) = template::start_from(&path, "beauty", "en").unwrap();
    let s = settings(&path);
    assert!(written.contains(&"kind.treatment".to_string()));
    assert_eq!(s.kind["treatment"].field["price"].kind, template::FieldType::Amount);
    assert!(s.kind["treatment"].default, "the default of appointments, none being one");
    template::start_from(&path, "consulting", "en").unwrap();
    let s = settings(&path);
    assert!(!s.kind["meeting"].default && s.kind["treatment"].default, "one default kind: the first stays");
}

/// A row removed: its ties move to the client, its notes become the file's.
#[test]
fn a_row_removed_leaves_its_ties_to_the_client() {
    use super::address::Address;
    use super::ties::{self, Said};
    let door = Memory::new();
    let path = config_file("rowties");
    let client = record::make(&door, "", &[], Billing::default()).unwrap();
    let kind = template::add_kind(&path, "Session").unwrap().id;
    let field = template::add_field(&path, &kind, "Notes", "text").unwrap().id;
    let s = settings(&path);
    let row = file::add_row(&door, &client.id, &kind, "2026-11-03T16:00:00+01:00", fields(&[(&field, "x".into())]), &s).unwrap();
    let at = Address::Row { client: client.id.clone(), kind: kind.clone(), row: row.id.clone() };
    let said = Said { day: Some("2026-11-03".parse().unwrap()), title: "Facial".into() };
    ties::tie(&door, &at, "sioul:event/e-1", &said).unwrap();
    ties::tie(&door, &at, "sioul:event/e-1", &said).unwrap();
    assert_eq!(ties::ties_of(&door, &client.id).unwrap().items.len(), 1, "made twice, one file");
    let note = file::write_note(&door, &client.id, &file::Note { row: format!("{kind}/{}", row.id), text: "about the row".into(), ..Default::default() }, &s).unwrap();
    file::remove_row(&door, &client.id, &kind, &row.id).unwrap();
    let ties = ties::ties_of(&door, &client.id).unwrap().items;
    assert_eq!((ties.len(), ties[0].at.clone(), ties[0].title.as_str()), (1, client.uri(), "Facial"));
    assert_eq!(file::note(&door, &client.id, &note.id).unwrap().unwrap().row, "", "the note stays, the file's");
    assert!(file::rows(&door, &client.id).unwrap().items.is_empty());
}

/// The rows' list is rebuilt from the rows when it does not open, and a row
/// it lacks (arrived from another device first) is read.
#[test]
fn the_rows_list_is_rebuilt_from_the_rows() {
    let door = Memory::new();
    let path = config_file("rowslist");
    let client = record::make(&door, "", &[], Billing::default()).unwrap();
    let kind = template::add_kind(&path, "Order").unwrap().id;
    let field = template::add_field(&path, &kind, "Item", "line").unwrap().id;
    let s = settings(&path);
    let a = file::add_row(&door, &client.id, &kind, "2026-11-01", fields(&[(&field, "Bowl".into())]), &s).unwrap();
    // A row written by another device, not yet in this list.
    let b = format!("id = \"b-row\"\nkind = \"{kind}\"\nschema = 1\nwhen = \"2026-11-02\"\n[fields]\n{field} = \"Cup\"\n");
    door.write(&file::row_path(&client.id, &kind, "b-row"), b.as_bytes(), Expect::Any).unwrap();
    let rows = file::rows(&door, &client.id).unwrap().items;
    assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["b-row", a.id.as_str()], "newest first");
    door.damage(&file::rows_path(&client.id));
    assert_eq!(file::rows(&door, &client.id).unwrap().items.len(), 2, "read from the rows");
    assert_eq!(file::rebuild_rows(&door, &client.id).unwrap(), 2);
}

/// A tie file keeps what this version does not know through every change
/// (`refresh_said`, a row removed, Same as…), reads `[shown]` over what the
/// event said, and an event absent on this device removes or rewrites nothing.
#[test]
fn a_tie_keeps_what_it_does_not_know_and_outlives_an_absent_event() {
    use super::address::Address;
    use super::ties::{self, Said, Sources};
    let door = Memory::new();
    let path = config_file("tiekeys");
    let client = record::make(&door, "", &[], Billing::default()).unwrap();
    let kind = template::add_kind(&path, "Session").unwrap().id;
    let field = template::add_field(&path, &kind, "Notes", "text").unwrap().id;
    let s = settings(&path);
    let row = file::add_row(&door, &client.id, &kind, "2026-11-03", fields(&[(&field, "x".into())]), &s).unwrap();
    let at = Address::Row { client: client.id.clone(), kind: kind.clone(), row: row.id.clone() };
    ties::tie(&door, &at, "sioul:event/e-1", &Said { day: Some("2026-11-03".parse().unwrap()), title: "Appointment".into() }).unwrap();
    let tie_path = door.paths().into_iter().find(|p| p.contains("/ties/")).unwrap();
    let text = door.read_text(&tie_path).unwrap().unwrap().0;
    assert!(text.contains("format = 1"), "{text}");
    // A newer Sioul's key and table, and the real words over a neutral event.
    door.write(&tie_path, format!("{text}future = \"kept\"\n\n[shown]\ntitle = \"Facial, Marie Exemple\"\n\n[later]\nflag = true\n").as_bytes(), Expect::Any).unwrap();
    // Made twice: the file there stays as it is.
    ties::tie(&door, &at, "sioul:event/e-1", &Said::default()).unwrap();
    ties::refresh_said(&door, &at, "sioul:event/e-1", &Said { day: Some("2026-11-04".parse().unwrap()), title: "Appointment".into() }).unwrap();
    let read = ties::ties_of(&door, &client.id).unwrap().items;
    assert_eq!((read[0].words(), read[0].title.as_str(), read[0].day.map(|d| d.to_string())), ("Facial, Marie Exemple", "Appointment", Some("2026-11-04".into())));
    let kept = |door: &Memory| {
        let p = door.paths().into_iter().find(|p| p.contains("/ties/")).unwrap();
        let t = door.read_text(&p).unwrap().unwrap().0;
        assert!(t.contains("future = \"kept\"") && t.contains("[later]") && t.contains("Facial, Marie Exemple"), "{t}");
    };
    kept(&door);
    // An event absent on this device: the index lists the tie from its day and title, and nothing is written.
    let before: Vec<_> = door.paths().iter().map(|p| (p.clone(), door.read(p).unwrap())).collect();
    let clients = record::list(&door).unwrap().items;
    let index = ties::Index::build(&door, &clients, &Sources::default());
    let seen = index.clients_of("sioul:event/e-1");
    assert_eq!(seen, vec![(client.id.clone(), Some(at.uri()))]);
    assert_eq!(index.of_client(&client.id)[0].day.map(|d| d.to_string()), Some("2026-11-04".into()));
    let after: Vec<_> = door.paths().iter().map(|p| (p.clone(), door.read(p).unwrap())).collect();
    assert_eq!(before, after, "an absent event rewrites nothing");
    // A row removed, then Same as…: the tie moves with every key it holds.
    file::remove_row(&door, &client.id, &kind, &row.id).unwrap();
    kept(&door);
    let other = record::make(&door, "", &[], Billing::default()).unwrap();
    let joined = super::keep::same_as(&door, &other.id, &client.id).unwrap();
    kept(&door);
    assert_eq!(ties::ties_of(&door, &joined.id).unwrap().items.len(), 1);
}

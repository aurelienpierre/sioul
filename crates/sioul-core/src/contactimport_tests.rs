// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The contacts' import on fixtures of fictional people only
//! (`tests/fixtures/contacts/`): vCard 2.1, 3.0 and 4.0; a CSV file as a
//! French spreadsheet saves it and as Google exports it.

use super::*;
use crate::contactbatch;
use crate::vdir::{self, Kind, LOCAL};
use std::sync::atomic::AtomicBool;

const V21: &[u8] = include_bytes!("../tests/fixtures/contacts/version-2.1.vcf");
const V30: &[u8] = include_bytes!("../tests/fixtures/contacts/version-3.0.vcf");
const V40: &[u8] = include_bytes!("../tests/fixtures/contacts/version-4.0.vcf");
const FRENCH: &[u8] = include_bytes!("../tests/fixtures/contacts/french.csv");
const GOOGLE: &[u8] = include_bytes!("../tests/fixtures/contacts/google.csv");

fn france() -> Option<&'static Region> {
    phones::region_named("FR")
}

fn shown(text: &str) -> Contact {
    contacts::contact(&lines::unfold(text), Path::new("/x.vcf"), &book_stub())
}

fn book_stub() -> Collection {
    Collection { kind: Kind::Contacts, account: "a".into(), id: "b".into(), dir: PathBuf::from("/"), name: "B".into(), color: None, read_only: false, components: vec![] }
}

fn columns_and_exports_are_well_formed() {
    let read: Columns = toml::from_str(COLUMNS).expect("data/contact-columns.toml reads");
    assert!(read.column.len() >= 20 && read.export.len() == 3);
    let mut seen: Vec<String> = Vec::new();
    for column in &read.column {
        for heading in &column.headings {
            let key = heading_key(heading);
            assert!(!seen.contains(&key) || column.headings.iter().filter(|h| heading_key(h) == key).count() > 1, "{heading}: named twice");
            seen.push(key);
        }
    }
}

/// The three versions: 2.1's quoted-printable and charsets read and the card
/// written as 3.0; 3.0's folded lines; 4.0 kept as it is; each keeps its UID
/// or gets one; a card never closed named by its line.
#[test]
fn vcards_of_the_three_versions() {
    columns_and_exports_are_well_formed();
    let old = read_vcf(&crate::csv::decode(V21));
    assert_eq!((old.cards.len(), old.broken.len()), (2, 0), "{old:?}");
    let lea = shown(&old.cards[0].1);
    assert!(old.cards[0].1.contains("VERSION:3.0"), "{}", old.cards[0].1);
    assert_eq!((lea.name.as_str(), lea.prefix.as_str(), lea.given.as_str(), lea.family.as_str()), ("Mme Léa Exemple", "Mme", "Léa", "Exemple"));
    assert_eq!(lea.notes.replace('\r', ""), "Vient le mardi\npour le thé");
    assert_eq!((lea.phones[0].label.as_str(), lea.emails[0].value.as_str()), ("cell", "lea@example.org"));
    assert!(!lea.uid.is_empty(), "given a UID");
    assert_eq!(shown(&old.cards[1].1).name, "André Guérin", "ISO-8859-1 read");
    let three = read_vcf(&crate::csv::decode(V30));
    assert_eq!((three.cards.len(), three.broken.clone()), (2, vec![18]), "the card never closed, by its first line");
    assert_eq!(three.cards[0].0, 1);
    let paul = shown(&three.cards[0].1);
    assert_eq!((paul.uid.as_str(), paul.notes.as_str()), ("card-three-1", "Une note qui tient sur une ligne trop longue pour une seule ligne de carte, pliée en deux"));
    assert_eq!(three.cards[0].1, "BEGIN:VCARD\r\nVERSION:3.0\r\nPRODID:-//Example//Phone//EN\r\nUID:card-three-1\r\nFN:Paul Exemple\r\nN:Exemple;Paul;;;\r\nTEL;TYPE=CELL:06 39 98 00 03\r\nEMAIL;TYPE=INTERNET,WORK:paul@example.org\r\nCATEGORIES:Clients\r\nNOTE:Une note qui tient sur une ligne trop longue pour une seule ligne de c\r\n arte\\, pliée en deux\r\nEND:VCARD\r\n", "kept as the file holds it");
    assert!(shown(&three.cards[1].1).uid.starts_with("sioul-"), "one without a UID given one");
    let four = read_vcf(&crate::csv::decode(V40));
    let sam = shown(&four.cards[0].1);
    assert_eq!((sam.uid.as_str(), sam.name.as_str()), ("urn:uuid:4fbe8971-0bc3-424c-9c26-36c3e1eff6b1", "Dr Sam Example"));
    assert!(four.cards[0].1.contains("VERSION:4.0") && four.cards[0].1.contains("KIND:individual"));
}

/// A French spreadsheet's file: the mapping guessed from its French
/// headings; the row over two lines read whole; a title column into the
/// title field; numbers international; the line that does not read named.
#[test]
fn a_french_spreadsheet_s_file() {
    let table = crate::csv::read(&crate::csv::decode(FRENCH));
    let fields = guess(&table.headings);
    use Field::*;
    assert_eq!(fields, [Title, Family, Given, Phone, Mobile, Email, Street, Postcode, Town, Birthday, Company, Note]);
    assert_eq!(export_of(&table.headings), None, "no preset: nothing assumed");
    let cards = read_csv(&table, &fields);
    assert_eq!((cards.cards.len(), cards.broken.clone()), (2, vec![6]), "{cards:?}");
    let lea = shown(&international(&cards.cards[0].1, france()));
    assert_eq!((lea.name.as_str(), lea.prefix.as_str(), lea.given.as_str(), lea.family.as_str()), ("Léa Exemple", "Mme", "Léa", "Exemple"));
    assert_eq!(lea.phones.iter().map(|p| (p.label.as_str(), p.value.as_str())).collect::<Vec<_>>(), [("", "+33465000010"), ("cell", "+33600000001")]);
    assert_eq!((lea.addresses[0].value.as_str(), lea.birthday.as_str(), lea.notes.as_str()), ("3 rue de l'Exemple\n69000 Lyon", "1984-05-12", "Vient le mardi\npour le thé"));
    let jean = shown(&cards.cards[1].1);
    assert_eq!((jean.name.as_str(), jean.prefix.as_str(), jean.org.as_str(), cards.cards[1].0), ("Jean Martin", "M.", "Atelier Exemple", 4));
}

/// Google's export recognised; its labels as groups (its own aside), its
/// prefix as the title, its role as the job.
#[test]
fn google_s_export() {
    let table = crate::csv::read(&crate::csv::decode(GOOGLE));
    assert_eq!(export_of(&table.headings).as_deref(), Some("Google Contacts"));
    let fields = guess(&table.headings);
    let cards = read_csv(&table, &fields);
    let jane = shown(&cards.cards[0].1);
    assert_eq!((jane.name.as_str(), jane.prefix.as_str(), jane.org.as_str(), jane.title.as_str()), ("Jane Example", "Ms", "Example Ltd", "Accountant"));
    assert_eq!((jane.categories.clone(), jane.addresses[0].value.as_str()), (vec!["Work".to_string()], "1 Example Street\nLondon"));
    assert!(shown(&cards.cards[1].1).categories.is_empty());
    let outlook = ["First Name", "Last Name", "Title", "E-mail Address", "Mobile Phone", "Job Title", "Business Phone"].map(String::from);
    assert_eq!(export_of(&outlook).as_deref(), Some("Outlook"));
    assert_eq!(guess(&outlook), [Field::Given, Field::Family, Field::Title, Field::Email, Field::Mobile, Field::Role, Field::Phone]);
}

/// A mapping kept under a name, found again for the same headings
/// (written otherwise), never for others; kept again under that name, replaced.
#[test]
fn a_mapping_kept_and_found_again() {
    let path = std::env::temp_dir().join(format!("sioul-import-mappings-{}.toml", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let headings = ["Client", "Tél. portable", "Remarque"].map(String::from);
    let mut fields = guess(&headings);
    assert_eq!(fields, [Field::Name, Field::Mobile, Field::Note]);
    fields[2] = Field::Skip;
    Mappings::keep(&path, "Planning", &headings, &fields).unwrap();
    let kept = Mappings::load(&path);
    let found = kept.found(&["CLIENT".to_string(), "tel portable".into(), "remarque".into()]).expect("the same headings, written otherwise");
    assert_eq!((found.name.as_str(), found.fields.clone()), ("Planning", fields.clone()));
    assert!(kept.found(&["Client".to_string()]).is_none());
    Mappings::keep(&path, "planning", &headings, &[Field::Name, Field::Phone, Field::Note]).unwrap();
    assert_eq!(Mappings::load(&path).kept.len(), 1, "one per name and per headings");
    assert!(Mappings::keep(&path, " ", &headings, &fields).is_err());
    let _ = std::fs::remove_file(&path);
}

/// Those already here: matched by an e-mail address or a number written
/// otherwise, completed with what the file adds, never doubled, nothing
/// overwritten; a look-alike by name alone imported; the whole import undone.
#[test]
fn a_match_completed_never_doubled_then_undone() {
    crate::config::stand_ins_only();
    let id = "import-here";
    let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(LOCAL).join(id));
    let dir = vdir::prepare(Kind::Contacts, LOCAL, id, "Here", None).unwrap();
    let lea = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:here-lea\r\nFN:Léa E.\r\nN:E.;Léa;;;\r\nTEL;TYPE=CELL:+33 6 00 00 00 01\r\nNOTE:Déjà là\r\nEND:VCARD\r\n";
    std::fs::write(dir.join("lea.vcf"), lea).unwrap();
    let book = vdir::collections(Kind::Contacts).into_iter().find(|c| c.account == LOCAL && c.id == id).unwrap();
    let existing: Vec<Contact> = book.items().iter().filter_map(|p| contacts::read(p, &book)).collect();
    let table = crate::csv::read(&crate::csv::decode(FRENCH));
    let cards = read_csv(&table, &guess(&table.headings));
    let imported = plan(&cards, &existing, &book, &["Clients".into()], false, france());
    assert_eq!((imported.new, imported.completed, imported.same, imported.broken.clone()), (1, 1, 0, vec![6]));
    // The same file twice: the second time adds nothing, doubles nothing.
    let outcome = contactbatch::run(&crate::config::state_dir().join("import-records"), "import", imported.plan.steps, None, 1_790_000_100, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
    assert_eq!(outcome.done, 2);
    let after: Vec<Contact> = book.items().iter().filter_map(|p| contacts::read(p, &book)).collect();
    assert_eq!(after.len(), 2);
    let completed = after.iter().find(|c| c.uid == "here-lea").unwrap();
    assert_eq!((completed.name.as_str(), completed.notes.as_str(), completed.prefix.as_str()), ("Léa E.", "Déjà là", "Mme"), "nothing overwritten, what lacked added");
    assert_eq!(completed.phones.len(), 2, "the mobile written otherwise not doubled, the landline added: {:?}", completed.phones);
    assert_eq!((completed.emails[0].value.as_str(), completed.categories.clone()), ("lea@example.org", vec!["Clients".to_string()]));
    let again = plan(&cards, &after, &book, &["Clients".into()], false, france());
    assert_eq!((again.new, again.completed, again.same), (0, 0, 2), "{:?}", again.plan.steps);
    // A look-alike by name alone is a new card: Duplicates offers it.
    let twin = Cards { cards: vec![(1, "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:twin\r\nFN:Jean Martin\r\nEMAIL:other@example.org\r\nEND:VCARD\r\n".into())], broken: vec![] };
    assert_eq!(plan(&twin, &after, &book, &[], false, france()).new, 1);
    // Undone as a whole: the new card gone, the completed one as it was.
    contactbatch::undo(&crate::config::state_dir().join("import-records"), &outcome.id, &Kind::Contacts.root(), &AtomicBool::new(false), &mut |_, _| {}).unwrap();
    assert_eq!(book.items().len(), 1);
    assert_eq!(std::fs::read_to_string(dir.join("lea.vcf")).unwrap(), lea);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file read half: each row that does not read named by its line, the others imported.
#[test]
fn a_half_read_file_is_reported_line_by_line() {
    let text = "Nom;Prénom;E-mail\nExemple;Lou;lou@example.org\n;;\n;;not-an-address\nMartin;Sam;\n\"Exemple;Kim;kim@example.org\n";
    let table = crate::csv::read(text);
    let cards = read_csv(&table, &guess(&table.headings));
    assert_eq!(cards.cards.iter().map(|(line, _)| *line).collect::<Vec<_>>(), [2, 5]);
    assert_eq!(cards.broken, [4, 6], "a row of nothing usable, a quote never closed");
    assert_eq!(birthday("1984-5-2"), "1984-05-02");
    assert_eq!(birthday("printemps"), "printemps");
}

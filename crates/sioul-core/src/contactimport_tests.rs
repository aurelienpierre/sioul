// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The contacts' import on fixtures of fictional people only
//! (`tests/fixtures/contacts/`): vCard 2.1, 3.0 and 4.0; a CSV file as a
//! French spreadsheet saves it and as Google exports it.

use super::*;
use crate::contactbatch;
use crate::vdir::{self, Kind, LOCAL};
use std::sync::atomic::AtomicBool;

fn tr() -> crate::i18n::Translator {
    crate::i18n::Translator::new("en")
}

/// A book of this device's own with these files, made for one test.
fn here_book(id: &str, cards: &[(&str, &str)]) -> Collection {
    crate::config::stand_ins_only();
    let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(LOCAL).join(id));
    let dir = vdir::prepare(Kind::Contacts, LOCAL, id, id, None).unwrap();
    for (file, text) in cards {
        std::fs::write(dir.join(file), text).unwrap();
    }
    vdir::collections(Kind::Contacts).into_iter().find(|c| c.account == LOCAL && c.id == id).unwrap()
}

fn in_book(book: &Collection) -> Vec<Contact> {
    book.items().iter().filter_map(|p| contacts::read(p, book)).collect()
}

fn run_it(id: &str, steps: Vec<(String, Step)>, now: i64) -> contactbatch::Outcome {
    contactbatch::run(&crate::config::state_dir().join(format!("{id}-records")), "import", steps, None, now, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap()
}

const V21: &[u8] = include_bytes!("../tests/fixtures/contacts/version-2.1.vcf");
const V30: &[u8] = include_bytes!("../tests/fixtures/contacts/version-3.0.vcf");
const V40: &[u8] = include_bytes!("../tests/fixtures/contacts/version-4.0.vcf");
const FRENCH: &[u8] = include_bytes!("../tests/fixtures/contacts/french.csv");
const GOOGLE: &[u8] = include_bytes!("../tests/fixtures/contacts/google.csv");

fn france() -> Option<&'static Region> {
    phones::region_named("FR")
}

fn shown(text: &str) -> Contact {
    contacts::contact_quiet(&lines::unfold(text), Path::new("/x.vcf"), &book_stub())
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
/// title field; numbers international when asked (06 39 98 is ARCEP's
/// fiction block, which the numbering plan reads as Mayotte's); a two-way
/// date read day first in a French table; the line that does not read named.
#[test]
fn a_french_spreadsheet_s_file() {
    let (text, encoding) = crate::csv::decode_with(FRENCH);
    assert_eq!(encoding, crate::csv::Encoding::Windows1252);
    let table = crate::csv::read(&text);
    let fields = guess(&table.headings);
    use Field::*;
    assert_eq!(fields, [Title, Family, Given, Phone, Mobile, Email, Street, Postcode, Town, Birthday, Company, Note]);
    assert_eq!(export_of(&table.headings), None, "no preset: nothing assumed");
    let cards = read_csv(&table, &fields, &tr());
    assert_eq!((cards.cards.len(), cards.broken.clone(), table.unclosed), (2, vec![], Some((6, 1))), "{cards:?}");
    let (written, rewritten) = international(&cards.cards[0].1, france());
    let lea = shown(&written);
    assert_eq!((lea.name.as_str(), lea.prefix.as_str(), lea.given.as_str(), lea.family.as_str()), ("Léa Exemple", "Mme", "Léa", "Exemple"));
    assert_eq!((lea.phones.iter().map(|p| (p.label.as_str(), p.value.as_str())).collect::<Vec<_>>(), rewritten), (vec![("", "+33465710010"), ("cell", "+262639980001")], 2));
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
    let cards = read_csv(&table, &fields, &tr());
    let jane = shown(&cards.cards[0].1);
    assert_eq!((jane.name.as_str(), jane.prefix.as_str(), jane.org.as_str(), jane.title.as_str()), ("Jane Example", "Ms", "Example Ltd", "Accountant"));
    assert_eq!((jane.categories.clone(), jane.addresses[0].value.as_str(), jane.birthday.as_str()), (vec!["Work".to_string()], "1 Example Street\nLondon", "1990-02-03"));
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

/// Review F14: a card already here (the same name and a number) is left as
/// it is unless completing is asked, what the file would add listed first;
/// completed, nothing it holds is overwritten; the groups go to new cards
/// only unless asked; the whole import undone.
#[test]
fn a_card_already_here_is_changed_only_when_asked() {
    let lea = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:here-lea\r\nFN:Léa Exemple\r\nN:Exemple;Léa;;;\r\nTEL;TYPE=CELL:06 39 98 00 01\r\nNOTE:Déjà là\r\nEND:VCARD\r\n";
    let book = here_book("import-here", &[("lea.vcf", lea)]);
    let table = crate::csv::read(&crate::csv::decode(FRENCH));
    let cards = read_csv(&table, &guess(&table.headings), &tr());
    let mut options = Options { groups: vec!["Clients".into()], international: true, ..Options::default() };
    let imported = plan(&cards, &in_book(&book), &book, &options, france());
    assert_eq!((imported.new, imported.here, imported.completed, imported.apart), (1, 1, 0, 0));
    let adds: Vec<(String, String)> = imported.matches[0].adds.clone();
    assert!(adds.contains(&("email".into(), "lea@example.org".into())) && adds.contains(&("phone".into(), "+33465710010".into())) && adds.contains(&("title".into(), "Mme".into())), "{adds:?}");
    assert!(!adds.iter().any(|(k, _)| k == "group"), "the import's groups go to new cards only");
    assert!(!adds.iter().any(|(k, _)| k == "note"), "a note already there stays: {adds:?}");
    // Asked: completed, the note and the name kept, nothing doubled.
    options.complete = true;
    options.groups_here = true;
    let imported = plan(&cards, &in_book(&book), &book, &options, france());
    assert_eq!((imported.new, imported.here, imported.completed), (1, 1, 1));
    let outcome = run_it("import-here", imported.plan.steps, 1_790_000_100);
    assert_eq!(outcome.done, 2);
    let after = in_book(&book);
    let completed = after.iter().find(|c| c.uid == "here-lea").unwrap();
    assert_eq!((completed.name.as_str(), completed.notes.as_str(), completed.prefix.as_str(), completed.phones.len(), completed.categories.clone()), ("Léa Exemple", "Déjà là", "Mme", 2, vec!["Clients".to_string()]));
    let again = plan(&cards, &after, &book, &options, france());
    assert_eq!((again.new, again.here, again.completed), (0, 2, 0), "the same file again: nothing new, nothing doubled");
    // Undone as a whole: the new card gone, the completed one as it was.
    contactbatch::undo(&crate::config::state_dir().join("import-here-records"), &outcome.id, &Kind::Contacts.root(), &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap();
    assert_eq!(book.items().len(), 1);
    assert_eq!(std::fs::read_to_string(book.dir.join("lea.vcf")).unwrap(), lea);
    let _ = std::fs::remove_dir_all(&book.dir);
}

/// Review F14: two people sharing a number or an address stay two, in a
/// file and against a card here; counted as imported apart.
#[test]
fn two_people_sharing_a_number_stay_two() {
    let text = "Nom;Prénom;Portable;E-mail\nMartin;Jeanne;06 39 98 00 21;jeanne@example.org\nMartin;Paul;06 39 98 00 21;paul@example.org\nExemple;Lou;06 39 98 00 22;\n";
    let table = crate::csv::read(text);
    let cards = read_csv(&table, &guess(&table.headings), &tr());
    let imported = plan(&cards, &[], &book_stub(), &Options::default(), france());
    assert_eq!((imported.new, imported.apart, imported.twice), (3, 1, 0));
    let jean = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:here-jean\r\nFN:Jean Dupont\r\nTEL;TYPE=HOME:04 65 71 00 01\r\nCATEGORIES:Famille\r\nEND:VCARD\r\n";
    let book = here_book("import-apart", &[("jean.vcf", jean)]);
    let table = crate::csv::read("Civilité;Nom;Prénom;Téléphone;E-mail;Groupes\nMme;Martin;Jeanne;04 65 71 00 01;jeanne@example.org;Urgences\n");
    let cards = read_csv(&table, &guess(&table.headings), &tr());
    let options = Options { groups: vec!["Clients".into()], complete: true, groups_here: true, ..Options::default() };
    let imported = plan(&cards, &in_book(&book), &book, &options, france());
    assert_eq!((imported.new, imported.here, imported.apart, imported.completed), (1, 0, 1, 0));
    let Step::Write { before: None, after, .. } = &imported.plan.steps[0].1 else { panic!("a new card") };
    assert!(after.contains("FN:Jeanne Martin") && after.contains("Urgences") && after.contains("Clients"), "{after}");
    let _ = std::fs::remove_dir_all(&book.dir);
}

/// Review F13: a card whose UID is here is that card: the same file twice
/// never writes it a second time, whatever it holds.
#[test]
fn the_same_file_twice_never_doubles_a_card() {
    let atelier = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:u-1\r\nFN:Atelier Exemple\r\nADR:;;3 rue de l'Exemple;Lyon;;69000;France\r\nEND:VCARD\r\n";
    let book = here_book("import-twice", &[]);
    let cards = read_vcf(atelier);
    let first = plan(&cards, &in_book(&book), &book, &Options::default(), france());
    run_it("import-twice", first.plan.steps, 1_790_000_110);
    let second = plan(&cards, &in_book(&book), &book, &Options::default(), france());
    assert_eq!((second.new, second.here), (0, 1));
    let mut twice = cards.clone();
    twice.cards.push(twice.cards[0].clone());
    let in_one_file = plan(&twice, &[], &book_stub(), &Options::default(), france());
    assert_eq!((in_one_file.new, in_one_file.twice), (1, 1), "one UID twice in a file: one card");
    let _ = std::fs::remove_dir_all(&book.dir);
}

/// Review F21: completing keeps a card's several organisations and roles.
#[test]
fn completing_keeps_every_line_of_a_single_value() {
    let sam = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:here-sam\r\nFN:Sam Exemple\r\nORG:Atelier Exemple\r\nORG:Autre Atelier\r\nTITLE:Gérant\r\nTITLE:Formateur\r\nEMAIL:sam@example.org\r\nEND:VCARD\r\n";
    let book = here_book("import-single", &[("sam.vcf", sam)]);
    let cards = read_vcf("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:file-sam\r\nFN:Sam Exemple\r\nEMAIL:sam@example.org\r\nEND:VCARD\r\n");
    let imported = plan(&cards, &in_book(&book), &book, &Options { complete: true, ..Options::default() }, france());
    assert_eq!((imported.new, imported.here, imported.completed), (0, 1, 0), "nothing to add: nothing written");
    let cards = read_vcf("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:file-sam\r\nFN:Sam Exemple\r\nEMAIL:sam@example.org\r\nURL:https://example.org/sam\r\nEND:VCARD\r\n");
    let imported = plan(&cards, &in_book(&book), &book, &Options { complete: true, ..Options::default() }, france());
    let Step::Write { after, .. } = &imported.plan.steps[0].1 else { panic!() };
    assert!(after.contains("ORG:Atelier Exemple\r\n") && after.contains("ORG:Autre Atelier\r\n") && after.contains("TITLE:Formateur\r\n") && after.contains("URL:https://example.org/sam"), "{after}");
    let _ = std::fs::remove_dir_all(&book.dir);
}

/// A file read half: each row that does not read named by its line, the
/// others imported; birthdays read only when they read one way; an address
/// written with its name; numbers given as an address left as they are.
#[test]
fn a_half_read_file_and_what_is_kept_as_written() {
    let text = "Nom;Prénom;E-mail\nExemple;Lou;Lou <lou@example.org>\n;;\n;;not-an-address\nMartin;Sam;\n\"Exemple;Kim;kim@example.org\n";
    let table = crate::csv::read(text);
    let cards = read_csv(&table, &guess(&table.headings), &tr());
    assert_eq!(cards.cards.iter().map(|(line, _)| *line).collect::<Vec<_>>(), [2, 5]);
    assert_eq!((cards.broken.clone(), table.unclosed), (vec![4], Some((6, 1))), "a row of nothing usable, a quote never closed");
    assert_eq!(shown(&cards.cards[0].1).emails[0].value, "lou@example.org");
    assert_eq!(birthday("1984-5-2", false).as_deref(), Some("1984-05-02"));
    assert_eq!(birthday("05/13/1984", false).as_deref(), Some("1984-05-13"), "a month first, said by its day");
    assert_eq!(birthday("13/05/1984", false).as_deref(), Some("1984-05-13"));
    assert_eq!((birthday("5/12/1984", false), birthday("5/12/1984", true).as_deref()), (None, Some("1984-12-05")), "two ways: a French table's day first, else as written");
    assert_eq!((birthday("12/05/84", true), birthday("printemps", true)), (None, None));
    let english = crate::csv::read("Name;Birthday\nLou Exemple;5/12/1984\n");
    let cards = read_csv(&english, &guess(&english.headings), &tr());
    let lou = shown(&cards.cards[0].1);
    assert_eq!((lou.birthday.as_str(), cards.birthdays_as_written), ("", 1));
    assert!(lou.notes.contains("5/12/1984"), "{}", lou.notes);
    let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:n-1\r\nFN:Lou Exemple\r\nTEL;VALUE=uri:tel:0465711234\r\nTEL:04 65 71 12 34\r\nEND:VCARD\r\n";
    let (written, n) = international(card, france());
    assert!(written.contains("TEL;VALUE=uri:tel:0465711234\r\n") && written.contains("TEL:+33465711234\r\n") && n == 1, "{written}");
}

/// Review F7: a card too large, and a file of more cards than read, each said.
#[test]
fn the_limits_of_a_vcard_file() {
    let big = format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Lou\r\nNOTE:{}\r\nEND:VCARD\r\nBEGIN:VCARD\r\nVERSION:3.0\r\nFN:Kim\r\nEND:VCARD\r\n", "x".repeat(MOST_CARD));
    let read = read_vcf(&big);
    assert_eq!((read.cards.len(), read.too_big.clone()), (1, vec![1]));
    let many = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nEND:VCARD\r\n".repeat(MOST_CARDS + 2);
    let read = read_vcf(&many);
    assert_eq!((read.cards.len(), read.cut), (MOST_CARDS, Some(MOST_CARDS * 4 + 1)));
    let nameless = read_vcf("BEGIN:VCARD\r\nVERSION:3.0\r\nNOTE:nothing else\r\nEND:VCARD\r\n");
    assert_eq!((nameless.cards.len(), nameless.broken.clone()), (0, vec![1]), "a card of nobody does not read");
}

/// Review V5: two titles that differ are two people at one number ("M.
/// Martin", "Mme Martin"); a card whose UID is here under another name is
/// listed apart, left alone, completed only when its own box is ticked.
#[test]
fn two_titles_or_two_names_under_one_identifier_stay_two() {
    let table = crate::csv::read("Civilité;Nom;Téléphone;E-mail\nM.;Martin;04 65 71 00 01;paul@example.org\nMme;Martin;04 65 71 00 01;jeanne@example.org\n");
    let cards = read_csv(&table, &guess(&table.headings), &tr());
    let both = plan(&cards, &[], &book_stub(), &Options { complete: true, ..Options::default() }, france());
    assert_eq!((both.new, both.twice, both.apart), (2, 0, 1), "{:?}", both.matches);
    let here = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:1\r\nFN:Paul Exemple\r\nEMAIL:paul@example.org\r\nEND:VCARD\r\n";
    let book = here_book("import-v5", &[("paul.vcf", here)]);
    let file = read_vcf("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:1\r\nFN:Jeanne Martin\r\nEMAIL:jeanne@example.org\r\nTEL:+33465710042\r\nEND:VCARD\r\n");
    let completing = plan(&file, &in_book(&book), &book, &Options { complete: true, ..Options::default() }, france());
    assert_eq!((completing.new, completing.here, completing.other_names, completing.completed), (0, 0, 1, 0));
    let found = &completing.matches[0];
    assert_eq!((found.name.as_str(), found.other_name.as_str()), ("Paul Exemple", "Jeanne Martin"));
    let ticked = plan(&file, &in_book(&book), &book, &Options { complete: true, complete_also: vec![found.key.clone()], ..Options::default() }, france());
    assert_eq!(ticked.completed, 1, "its own box ticked");
    let _ = std::fs::remove_dir_all(&book.dir);
}

/// Review V6: the groups a file gives its own new cards are said, with how
/// many; one that a list of the person's reads is kept only when asked.
#[test]
fn a_file_s_own_groups_are_said_and_listed_ones_kept_only_when_asked() {
    let file = read_vcf("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:g-1\r\nFN:Sam Inconnu\r\nTEL:+33465710077\r\nCATEGORIES:Famille,Voisins\r\nEND:VCARD\r\n");
    let listed: std::collections::BTreeSet<String> = [crate::porch::category_key("Famille")].into_iter().collect();
    let held = plan(&file, &[], &book_stub(), &Options { listed: listed.clone(), ..Options::default() }, france());
    assert_eq!(held.file_groups, [FileGroup { name: "Famille".into(), cards: 1, listed: true }, FileGroup { name: "Voisins".into(), cards: 1, listed: false }]);
    let Step::Write { after, .. } = &held.plan.steps[0].1 else { panic!("a new card") };
    assert!(!after.contains("Famille") && after.contains("Voisins"), "{after}");
    let kept = plan(&file, &[], &book_stub(), &Options { listed, keep_listed: true, ..Options::default() }, france());
    let Step::Write { after, .. } = &kept.plan.steps[0].1 else { panic!("a new card") };
    assert!(after.contains("Famille") && after.contains("Voisins"), "{after}");
}

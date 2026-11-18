// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Address books on a stand-in CardDAV server (wave D, 18 November 2026,
//! `stand_in::Dav::start_books`, never a real server): a book made, renamed,
//! removed empty, removed with its cards deleted, merged into another by its
//! cards moved, shared and unshared; a card moved between two books by the
//! server's own MOVE and by a copy, its UID kept, and who may reach you
//! unchanged by the move.

use super::stand_in::{Dav, Fault, home, nth};
use super::*;
use sioul_core::contactbatch;
use sioul_core::contacts;
use sioul_core::porch::{Book, SenderList, Senders};
use std::sync::atomic::AtomicBool;

/// A contacts-and-calendars account of the test's own, its contacts' home on the stand-in.
fn account(id: &str, home_url: &str) -> Account {
    home();
    sioul_core::config::stand_ins_only();
    Homes { calendars: None, contacts: Some(home_url.to_string()) }.save(id).unwrap();
    toml::from_str(&format!("id = \"{id}\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"127.0.0.1\"\n")).unwrap()
}

fn client() -> Client {
    Client::new("jane", "secret")
}

fn card(uid: &str, name: &str, extra: &str) -> String {
    format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\nFN:{name}\r\n{extra}END:VCARD\r\n")
}

fn book(account: &str, id: &str) -> Option<vdir::Collection> {
    vdir::collections(Kind::Contacts).into_iter().find(|c| c.account == account && c.id == id)
}

fn cards_of(account: &str, id: &str) -> Vec<contacts::Contact> {
    book(account, id).map(|b| b.items().iter().filter_map(|p| contacts::read(p, &b)).collect()).unwrap_or_default()
}

fn uids(dav: &Dav, book: &str) -> Vec<String> {
    let mut found: Vec<String> = dav.items(book).values().filter_map(|t| sioul_core::contactbatch::uid_of(t)).collect();
    found.sort();
    found
}

fn tr() -> sioul_core::i18n::Translator {
    sioul_core::i18n::Translator::new("en")
}

/// Made here, then on the server by an extended MKCOL at the next sync;
/// renamed (PROPPATCH); removed empty (DELETE); removed with its cards, a
/// dated copy of them kept first; merged into another, its cards moved there
/// by MOVE, then deleted once its server holds none.
#[test]
fn a_book_made_renamed_removed_and_merged_on_its_server() {
    let (dav, home_url) = Dav::start_books(&["contacts"]);
    let account = account("books-life", &home_url);
    let config: sioul_core::config::Config = toml::from_str("[[account]]\nid = \"books-life\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"127.0.0.1\"\n").unwrap();
    dav.put("contacts", "lea.vcf", &card("lea-1", "Léa Exemple", ""));
    sync_with(&client(), &account).unwrap();
    assert_eq!(cards_of("books-life", "contacts").len(), 1);
    // Made: MKCOL with its name, yours.
    let made = sioul_core::addressbooks::make(&config, "books-life", "Professional", &tr()).unwrap();
    sync_with(&client(), &account).unwrap();
    assert_eq!(dav.books().get("professional").map(String::as_str), Some("Professional"), "{:?}", dav.seen());
    assert!(dav.seen().iter().any(|s| s.starts_with("MKCOL /cal/professional/ 201")));
    assert_eq!(State::load(&made.state_path()).owned, Some(true));
    // Renamed.
    vdir::rename_collection(&book("books-life", "professional").unwrap(), "Pro").unwrap();
    sync_with(&client(), &account).unwrap();
    assert_eq!(dav.books().get("professional").map(String::as_str), Some("Pro"));
    // Removed empty.
    vdir::delete_collection(&book("books-life", "professional").unwrap()).unwrap();
    sync_with(&client(), &account).unwrap();
    assert!(!dav.books().contains_key("professional") && book("books-life", "professional").is_none());
    // Removed with its cards: a copy kept, then deleted there whatever it holds.
    dav.put("contacts", "old.vcf", &card("old-1", "Sam Exemple", ""));
    let old = sioul_core::addressbooks::make(&config, "books-life", "Old", &tr()).unwrap();
    sync_with(&client(), &account).unwrap();
    vdir::write_item(&old.dir.join("kim.vcf"), &card("kim-1", "Kim Exemple", "")).unwrap();
    sync_with(&client(), &account).unwrap();
    assert_eq!(uids(&dav, "old"), ["kim-1"]);
    let copy = vdir::remove_collection(&book("books-life", "old").unwrap()).unwrap();
    assert!(copy.extension().is_some_and(|e| e == "vcf") && std::fs::read_to_string(&copy).unwrap().contains("UID:kim-1"), "the copy first");
    sync_with(&client(), &account).unwrap();
    assert!(!dav.books().contains_key("old"), "{:?}", dav.seen());
    // Merged: every card moved into the other by MOVE, then the emptied book deleted, never before.
    let merged = sioul_core::addressbooks::make(&config, "books-life", "Merged", &tr()).unwrap();
    sync_with(&client(), &account).unwrap();
    for (uid, name) in [("m-1", "Alex Exemple"), ("m-2", "Noa Exemple")] {
        vdir::write_item(&merged.dir.join(format!("{uid}.vcf")), &card(uid, name, "")).unwrap();
    }
    sync_with(&client(), &account).unwrap();
    let from = book("books-life", "merged").unwrap();
    let into = book("books-life", "contacts").unwrap();
    let everyone = cards_of("books-life", "merged");
    let plan = contactbatch::plan_move(&everyone.iter().collect::<Vec<_>>(), &into, sioul_core::capabilities::Provider::Dav, &tr());
    let done = contactbatch::run(&sioul_core::config::state_dir().join("books-life-records"), "merge", plan.steps, Some(contactbatch::Removed { place: "books-life/merged".into(), name: "Merged".into(), here_only: false }), 1_790_000_200, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap();
    assert_eq!(done.done, 2);
    vdir::delete_collection(&from).unwrap();
    sync_with(&client(), &account).unwrap();
    assert_eq!(uids(&dav, "contacts"), ["lea-1", "m-1", "m-2", "old-1"], "{:?}", dav.seen());
    assert!(dav.seen().iter().filter(|s| s.starts_with("MOVE /cal/merged/")).count() == 2, "{:?}", dav.seen());
    let deleted_at = dav.seen().iter().position(|s| s.starts_with("DELETE /cal/merged/ "));
    let moved_at = dav.seen().iter().rposition(|s| s.starts_with("MOVE /cal/merged/")).unwrap();
    assert!(deleted_at.is_none_or(|d| d > moved_at), "never deleted before its cards moved out: {:?}", dav.seen());
    sync_with(&client(), &account).unwrap();
    assert!(!dav.books().contains_key("merged"), "deleted once its server holds none: {:?}", dav.seen());
    assert!(!dav.seen().iter().any(|s| s.starts_with("PUT /cal/contacts/m-") || s.starts_with("DELETE /cal/merged/m-")), "moved, never copied: {:?}", dav.seen());
}

/// Who may reach you, by the lists, for a card: by its address, its UID.
fn judged(account: &str, address: &str, uid: &str) -> Vec<String> {
    let mine: Vec<contacts::Contact> = contacts::all().into_iter().filter(|c| c.place.starts_with(&format!("{account}/"))).collect();
    let senders = Senders { safe: SenderList::parse(&format!("category:Clients\ncontact:{uid}")), contacts: std::sync::Arc::new(Book::of(&mine, None)), ..Senders::default() };
    let by_address = senders.judge(address);
    let by_card = senders.judge_card(uid).map(|j| format!("{:?} {:?} {}", j.who, j.by, j.name)).unwrap_or_default();
    vec![format!("{:?} {:?} {}", by_address.who, by_address.by, by_address.name), by_card]
}

/// A card moved between two books by the server's own MOVE, then back by a
/// copy where the server refuses MOVE: its UID kept each way, never in both
/// books there, never in neither; the lists of who may reach you judge it
/// the same before and after (its groups decide, never its book).
#[test]
fn a_card_moved_by_move_and_by_copy_keeps_its_uid() {
    let (dav, home_url) = Dav::start_books(&["contacts", "pro"]);
    let account = account("books-move", &home_url);
    dav.put("contacts", "jeanne.vcf", &card("jeanne-1", "Jeanne Martin", "EMAIL:jeanne@example.org\r\nCATEGORIES:Clients\r\n"));
    sync_with(&client(), &account).unwrap();
    let before = judged("books-move", "jeanne@example.org", "jeanne-1");
    assert!(before[0].starts_with("Safe") && before[1].starts_with("Safe"), "{before:?}");
    // By MOVE.
    let jeanne = cards_of("books-move", "contacts");
    let plan = contactbatch::plan_move(&jeanne.iter().collect::<Vec<_>>(), &book("books-move", "pro").unwrap(), sioul_core::capabilities::Provider::Dav, &tr());
    contactbatch::run(&sioul_core::config::state_dir().join("books-move-records"), "move", plan.steps, None, 1_790_000_300, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap();
    sync_with(&client(), &account).unwrap();
    assert_eq!((uids(&dav, "contacts"), uids(&dav, "pro")), (vec![], vec!["jeanne-1".to_string()]), "{:?}", dav.seen());
    assert!(dav.seen().iter().any(|s| s.starts_with("MOVE /cal/contacts/jeanne.vcf 201")));
    assert!(!dav.seen().iter().any(|s| s.starts_with("PUT /cal/pro/") || s.starts_with("DELETE /cal/contacts/")), "no copy: {:?}", dav.seen());
    assert_eq!(judged("books-move", "jeanne@example.org", "jeanne-1"), before, "who may reach you: unchanged by the move");
    // Its new ETag, unsaid by the MOVE, comes at the next pull; nothing is set aside.
    let report = sync_with(&client(), &account).unwrap();
    let state = State::load(&vdir::state_path("books-move", Kind::Contacts, "pro"));
    assert!(state.items.iter().all(|i| !i.etag.is_empty()) && report.conflicts.is_empty(), "{state:?} {report:?}");
    // Back by a copy: the server refuses MOVE; the new copy first, then the old one deleted.
    dav.refuse_moves();
    let jeanne = cards_of("books-move", "pro");
    let plan = contactbatch::plan_move(&jeanne.iter().collect::<Vec<_>>(), &book("books-move", "contacts").unwrap(), sioul_core::capabilities::Provider::Dav, &tr());
    contactbatch::run(&sioul_core::config::state_dir().join("books-move-records"), "move", plan.steps, None, 1_790_000_301, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap();
    for _ in 0..2 {
        sync_with(&client(), &account).unwrap();
    }
    assert_eq!((uids(&dav, "contacts"), uids(&dav, "pro")), (vec!["jeanne-1".to_string()], vec![]), "{:?}", dav.seen());
    let seen = dav.seen();
    let put = seen.iter().rposition(|s| s.starts_with("PUT /cal/contacts/") && s.ends_with(" 201")).expect("sent anew");
    let delete = seen.iter().rposition(|s| s.starts_with("DELETE /cal/pro/")).expect("then deleted");
    assert!(put < delete, "the new copy first: {seen:?}");
    assert_eq!(judged("books-move", "jeanne@example.org", "jeanne-1"), before);
}

/// Shared with a person and a group of the server, read back, then one
/// taken back: through the calendars' own sharing calls, which take any
/// collection's address (`calendars::dav_share`).
#[test]
fn a_book_shared_and_unshared() {
    let (dav, home_url) = Dav::start_books(&["pro"]);
    let account = account("books-share", &home_url);
    let url = format!("{home_url}pro/");
    let client = client();
    let read = || crate::calendars::dav_sharing(&client, &url).unwrap();
    assert_eq!((read().sharer, read().people.len()), (crate::calendars::Sharer::Nextcloud, 0));
    crate::calendars::dav_share(&client, &url, &crate::calendars::principal("sam", false).unwrap(), true).unwrap();
    crate::calendars::dav_share(&client, &url, &crate::calendars::principal("office", true).unwrap(), false).unwrap();
    let shared = read();
    assert_eq!(shared.people.iter().map(|p| (p.name.as_str(), p.access, p.group)).collect::<Vec<_>>(), [("sam", crate::calendars::Access::Write, false), ("office", crate::calendars::Access::Read, true)]);
    // The listing says who else may write it (the owner's answer of 18 November 2026): its groups
    // count for who may reach you only once you say so for them.
    sync_with(&client, &account).unwrap();
    let writers = || State::load(&vdir::state_path("books-share", Kind::Contacts, "pro")).writers;
    assert_eq!(writers(), ["sam"], "the group that reads it is no writer");
    let config: sioul_core::config::Config = toml::from_str("[[account]]\nid = \"books-share\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"127.0.0.1\"\n").unwrap();
    assert!(!sioul_core::addressbooks::groups_count(&book("books-share", "pro").unwrap(), &config).count);
    crate::calendars::dav_unshare(&client, &url, "principal:principals/users/sam").unwrap();
    assert_eq!(dav.shares("pro"), [("principal:principals/groups/office".to_string(), false)]);
    assert_eq!(read().people.len(), 1);
    sync_with(&client, &account).unwrap();
    assert!(writers().is_empty() && sioul_core::addressbooks::groups_count(&book("books-share", "pro").unwrap(), &config).count, "yours alone again: its groups count");
}

/// A book of two cards on the stand-in, merged here into "contacts": its
/// cards moved, the book marked deleted; then another person adds a card to it.
fn merged_while_someone_adds(id: &str) -> (std::sync::Arc<Dav>, Account) {
    let (dav, home_url) = Dav::start_books(&["contacts", "merged"]);
    let account = account(id, &home_url);
    dav.put("merged", "a.vcf", &card("a-1", "Alex Exemple", ""));
    dav.put("merged", "b.vcf", &card("b-1", "Noa Exemple", ""));
    sync_with(&client(), &account).unwrap();
    let from = book(id, "merged").unwrap();
    let everyone = cards_of(id, "merged");
    let plan = contactbatch::plan_move(&everyone.iter().collect::<Vec<_>>(), &book(id, "contacts").unwrap(), sioul_core::capabilities::Provider::Dav, &tr());
    let removed = contactbatch::Removed { place: format!("{id}/merged"), name: "merged".into(), here_only: false };
    contactbatch::run(&sioul_core::config::state_dir().join(format!("{id}-records")), "merge", plan.steps, Some(removed), 1_790_200_000, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap();
    vdir::delete_collection(&from).unwrap();
    dav.put("merged", "theirs.vcf", &card("theirs-1", "Sam Exemple", ""));
    (dav, account)
}

/// Review F1: the listing before a merged book's deletion answered 503: the
/// book is kept, asked again; once a whole listing shows a card this device
/// never had, the book stays, shown again here with that card, and says why.
#[test]
fn a_merged_book_is_deleted_only_after_a_whole_listing_shows_it_empty() {
    let (dav, account) = merged_while_someone_adds("books-f1");
    dav.fault(|r: &super::stand_in::Request| (r.method == "PROPFIND" && r.path.trim_end_matches('/') == "/cal/merged").then_some(Fault::Status(503)));
    for _ in 0..3 {
        let _ = sync_with(&client(), &account);
    }
    assert!(dav.books().contains_key("merged") && uids(&dav, "merged") == ["theirs-1"], "kept, with the other person's card: {:?}", dav.seen());
    assert!(!dav.seen().iter().any(|s| s.starts_with("DELETE /cal/merged/ ")), "never deleted on a failed listing");
    assert!(book("books-f1", "merged").is_none(), "still hidden here, asked again");
}

/// Review F1, the reviewer's own case: once with the first listing of the
/// book answered 503, once without; both keep the card added there.
#[test]
fn a_merged_book_holding_a_card_added_there_stays_and_says_why() {
    for (id, once) in [("books-f1-held", false), ("books-f1-once", true)] {
        let (dav, account) = merged_while_someone_adds(id);
        if once {
            dav.fault(nth(1, |r: &super::stand_in::Request| r.method == "PROPFIND" && r.path.trim_end_matches('/') == "/cal/merged", Fault::Status(503)));
        }
        for _ in 0..3 {
            let _ = sync_with(&client(), &account);
        }
        assert_eq!((uids(&dav, "merged"), uids(&dav, "contacts")), (vec!["theirs-1".to_string()], vec!["a-1".to_string(), "b-1".to_string()]), "{id}: {:?}", dav.seen());
        let shown = book(id, "merged").unwrap_or_else(|| panic!("{id}: shown again here"));
        assert_eq!(cards_of(id, "merged").len(), 1);
        assert_eq!(State::load(&shown.state_path()).kept, "held");
        let config: sioul_core::config::Config = toml::from_str(&format!("[[account]]\nid = \"{id}\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"127.0.0.1\"\n")).unwrap();
        assert_eq!(sioul_core::addressbooks::line(&shown, &config, &tr()).waiting, tr().text("book-kept-held", None));
    }
}

/// Review F5: a book removed with its cards, while another person added one
/// there: the book stays on its server and comes back here, said.
#[test]
fn a_book_removed_with_its_cards_stays_when_its_server_holds_more() {
    let (dav, home_url) = Dav::start_books(&["contacts", "pro"]);
    let account = account("books-f5", &home_url);
    dav.put("pro", "a.vcf", &card("a-1", "Alex Exemple", ""));
    sync_with(&client(), &account).unwrap();
    State { owned: Some(true), ..State::load(&vdir::state_path("books-f5", Kind::Contacts, "pro")) }.save(&vdir::state_path("books-f5", Kind::Contacts, "pro")).unwrap();
    vdir::remove_collection(&book("books-f5", "pro").unwrap()).unwrap();
    dav.put("pro", "theirs.vcf", &card("theirs-1", "Sam Exemple", ""));
    for _ in 0..2 {
        let _ = sync_with(&client(), &account);
    }
    assert_eq!(uids(&dav, "pro"), ["a-1", "theirs-1"], "{:?}", dav.seen());
    assert!(book("books-f5", "pro").is_some_and(|b| State::load(&b.state_path()).kept == "held"));
}

/// Review F15, the design's own steps: the title moved into its field with a
/// group (step 1), then, before that change was sent, the same card moved to
/// another book (step 2). By the server's MOVE and by a copy alike, the card
/// arrives as changed, there and here.
#[test]
fn a_change_not_yet_sent_reaches_the_new_book() {
    for (id, by_move) in [("books-f15-move", true), ("books-f15-copy", false)] {
        let (dav, home_url) = Dav::start_books(&["contacts", "pro"]);
        let account = account(id, &home_url);
        if !by_move {
            dav.refuse_moves();
        }
        dav.put("contacts", "jeanne.vcf", &card("jeanne-1", "Mme Jeanne Martin", "EMAIL:jeanne@example.org\r\n"));
        sync_with(&client(), &account).unwrap();
        let records = sioul_core::config::state_dir().join(format!("{id}-records"));
        let all = cards_of(id, "contacts");
        let plan = contactbatch::plan_title(&all.iter().collect::<Vec<_>>(), Some("Professional"), &tr());
        contactbatch::run(&records, "title", plan.steps, None, 1_790_300_000, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap();
        let all = cards_of(id, "contacts");
        let plan = contactbatch::plan_move(&all.iter().collect::<Vec<_>>(), &book(id, "pro").unwrap(), sioul_core::capabilities::Provider::Dav, &tr());
        contactbatch::run(&records, "move", plan.steps, None, 1_790_300_001, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap();
        let mut conflicts = 0;
        for _ in 0..3 {
            conflicts += sync_with(&client(), &account).map(|r| r.conflicts.len()).unwrap_or(0);
        }
        let there: String = dav.items("pro").values().cloned().collect();
        let here: String = book(id, "pro").map(|b| b.items().iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect()).unwrap_or_default();
        for (side, text) in [("there", &there), ("here", &here)] {
            assert!(text.contains("FN:Jeanne Martin") && text.contains("N:Martin;Jeanne;;Mme;") && text.contains("CATEGORIES:Professional"), "{id}, {side}: {text}\n{:?}", dav.seen());
        }
        assert!(dav.items("contacts").is_empty() && conflicts == 0, "{id}");
        if by_move {
            assert!(dav.seen().iter().any(|s| s.starts_with("MOVE /cal/contacts/jeanne.vcf 201")), "{:?}", dav.seen());
        }
    }
}

/// Review F16: a card someone changed on the server before this device's
/// move reached it is kept in both books, never taken off the server; said.
#[test]
fn a_card_changed_elsewhere_then_moved_here_is_kept_in_both_books() {
    let (dav, home_url) = Dav::start_books(&["contacts", "pro"]);
    let account = account("books-f16", &home_url);
    dav.put("contacts", "jeanne.vcf", &card("jeanne-1", "Jeanne Martin", "EMAIL:jeanne@example.org\r\n"));
    sync_with(&client(), &account).unwrap();
    dav.put("contacts", "jeanne.vcf", &card("jeanne-1", "Jeanne Martin", "EMAIL:jeanne@example.org\r\nTEL:+33465710042\r\n"));
    let jeanne = cards_of("books-f16", "contacts");
    let plan = contactbatch::plan_move(&jeanne.iter().collect::<Vec<_>>(), &book("books-f16", "pro").unwrap(), sioul_core::capabilities::Provider::Dav, &tr());
    contactbatch::run(&sioul_core::config::state_dir().join("books-f16-records"), "move", plan.steps, None, 1_790_200_100, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap();
    let mut said = Vec::new();
    for _ in 0..4 {
        if let Ok(report) = sync_with(&client(), &account) {
            said.extend(report.kept_both);
        }
    }
    let theirs: String = dav.items("contacts").values().cloned().collect();
    assert!(theirs.contains("+33465710042"), "the other person's number still on the server: {:?}", dav.seen());
    assert_eq!(uids(&dav, "pro"), ["jeanne-1"]);
    assert_eq!(said, [("contact".to_string(), "Jeanne Martin".to_string())]);
    assert!(cards_of("books-f16", "contacts").iter().any(|c| c.phones.len() == 1), "back here in its old book as its server holds it");
}

/// Review F6: a new book its server answers 405 for, with no address book
/// there after it, is said refused, never taken for made; nothing moves into it.
#[test]
fn a_new_book_its_server_does_not_make_is_said_refused() {
    let (dav, home_url) = Dav::start_books(&["contacts"]);
    let account = account("books-f6", &home_url);
    let config: sioul_core::config::Config = toml::from_str("[[account]]\nid = \"books-f6\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"127.0.0.1\"\n").unwrap();
    dav.put("contacts", "a.vcf", &card("a-1", "Alex Exemple", ""));
    sync_with(&client(), &account).unwrap();
    dav.fault(|r: &super::stand_in::Request| (r.method == "MKCOL").then_some(Fault::Status(405)));
    let made = sioul_core::addressbooks::make(&config, "books-f6", "Professional", &tr()).unwrap();
    let report = sync_with(&client(), &account).unwrap();
    let state = State::load(&made.state_path());
    assert!(state.refused && !report.problems.is_empty(), "{state:?}");
    assert_eq!(sioul_core::addressbooks::line(&made, &config, &tr()).waiting, tr().text("book-refused", None));
    let everyone = cards_of("books-f6", "contacts");
    let plan = contactbatch::plan_move(&everyone.iter().collect::<Vec<_>>(), &book("books-f6", "professional").unwrap(), sioul_core::capabilities::Provider::Dav, &tr());
    assert!(plan.steps.is_empty() && plan.cannot.len() == 1);
    let before = dav.seen().iter().filter(|s| s.starts_with("MKCOL")).count();
    sync_with(&client(), &account).unwrap();
    assert_eq!(dav.seen().iter().filter(|s| s.starts_with("MKCOL")).count(), before, "never asked again");
}

/// Review F13 on a server: the same card imported again is that card, never written a second time.
#[test]
fn a_card_imported_twice_is_written_once() {
    let (dav, home_url) = Dav::start_books(&["contacts"]);
    let account = account("books-f13", &home_url);
    dav.one_uid_each(409, true);
    let atelier = card("u-1", "Atelier Exemple", "ADR:;;3 rue de l'Exemple;Lyon;;69000;France\r\n");
    dav.put("contacts", "atelier.vcf", &atelier);
    sync_with(&client(), &account).unwrap();
    let here = book("books-f13", "contacts").unwrap();
    let cards = sioul_core::contactimport::read_vcf(&atelier);
    let imported = sioul_core::contactimport::plan(&cards, &cards_of("books-f13", "contacts"), &here, &sioul_core::contactimport::Options::default(), None);
    assert_eq!((imported.new, imported.here, imported.plan.steps.len()), (0, 1, 0));
}

/// Review V4: a merged book is deleted only on a listing that names it,
/// answers each member, and leaves out no card this device holds a row for:
/// a 207 naming the book alone, or no response at all, keeps it; the
/// other person's card stays, and the book shows again once a whole listing
/// says so.
#[test]
fn a_listing_that_cannot_tell_keeps_the_book() {
    for (how, id) in [(1u8, "books-v4-alone"), (2u8, "books-v4-none")] {
        let (dav, account) = merged_while_someone_adds(id);
        dav.lists(how);
        for _ in 0..2 {
            let _ = sync_with(&client(), &account);
        }
        assert!(!dav.seen().iter().any(|s| s.starts_with("DELETE /cal/merged/ ")), "{id}: {:?}", dav.seen());
        assert_eq!(uids(&dav, "merged"), ["theirs-1"], "{id}");
        dav.lists(0);
        for _ in 0..2 {
            let _ = sync_with(&client(), &account);
        }
        assert!(book(id, "merged").is_some_and(|b| State::load(&b.state_path()).kept == "held"), "{id}: shown again, said");
    }
}

/// Review V4: what the deletion reads as a whole listing: never an empty
/// body, a multistatus of no response, a page that is no listing, nor one
/// whose member is answered with an error inside it.
#[test]
fn what_a_whole_listing_is() {
    let url = "http://127.0.0.1/cal/merged/";
    assert!(multistatus("").is_err());
    let (none, _) = multistatus(r#"<d:multistatus xmlns:d="DAV:"/>"#).unwrap();
    assert!(whole_listing(url, &none).is_none());
    let refused = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/cal/merged/</d:href><d:propstat><d:prop><d:getetag/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response><d:response><d:href>/cal/merged/a.vcf</d:href><d:status>HTTP/1.1 403 Forbidden</d:status></d:response></d:multistatus>"#;
    let (responses, _) = multistatus(refused).unwrap();
    assert!(whole_listing(url, &responses).is_none(), "a member refused inside the 207");
    let empty = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/cal/merged/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#;
    let (responses, _) = multistatus(empty).unwrap();
    assert_eq!(whole_listing(url, &responses).map(|m| m.len()), Some(0), "the book itself, no member: empty");
}

/// Point 8 of the verification: a book whose server says, by the standard
/// `DAV:owner` alone, that it is someone else's has its groups off until
/// said; a server that says neither whose a book is nor whom it is shared
/// with keeps its groups counting, and its line says so.
#[test]
fn whose_a_book_is_and_a_server_that_says_nothing() {
    let (dav, home_url) = Dav::start_books(&["contacts", "theirs"]);
    dav.owned_by("theirs", "sam");
    let owner_account = account("books-owner", &home_url);
    sync_with(&client(), &owner_account).unwrap();
    let config: sioul_core::config::Config = toml::from_str("[[account]]\nid = \"books-owner\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"127.0.0.1\"\n").unwrap();
    let theirs = book("books-owner", "theirs").unwrap();
    assert_eq!(State::load(&theirs.state_path()).owned, Some(false));
    assert!(!sioul_core::addressbooks::groups_count(&theirs, &config).count, "someone else's: off until said");
    let mine = book("books-owner", "contacts").unwrap();
    assert!(sioul_core::addressbooks::groups_count(&mine, &config).count && sioul_core::addressbooks::line(&mine, &config, &tr()).groups_note.is_empty());
    let (plain, plain_url) = Dav::start_books(&["contacts"]);
    plain.says_little();
    let quiet_account = account("books-plain", &plain_url);
    sync_with(&client(), &quiet_account).unwrap();
    let config: sioul_core::config::Config = toml::from_str("[[account]]\nid = \"books-plain\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"127.0.0.1\"\n").unwrap();
    let quiet = book("books-plain", "contacts").unwrap();
    assert!(sioul_core::addressbooks::groups_count(&quiet, &config).count, "as today");
    assert_eq!(sioul_core::addressbooks::line(&quiet, &config, &tr()).groups_note, tr().text("book-writers-unsaid", None));
}

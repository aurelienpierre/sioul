// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Address books on a stand-in CardDAV server (wave D, 18 November 2026,
//! `stand_in::Dav::start_books`, never a real server): a book made, renamed,
//! removed empty, removed with its cards deleted, merged into another by its
//! cards moved, shared and unshared; a card moved between two books by the
//! server's own MOVE and by a copy, its UID kept, and who may reach you
//! unchanged by the move.

use super::stand_in::{Dav, home};
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
    let done = contactbatch::run(&sioul_core::config::state_dir().join("books-life-records"), "merge", plan.steps, Some("books-life/merged".into()), 1_790_000_200, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
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
    contactbatch::run(&sioul_core::config::state_dir().join("books-move-records"), "move", plan.steps, None, 1_790_000_300, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
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
    contactbatch::run(&sioul_core::config::state_dir().join("books-move-records"), "move", plan.steps, None, 1_790_000_301, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
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
    let _ = account("books-share", &home_url);
    let url = format!("{home_url}pro/");
    let client = client();
    let read = || crate::calendars::dav_sharing(&client, &url).unwrap();
    assert_eq!((read().sharer, read().people.len()), (crate::calendars::Sharer::Nextcloud, 0));
    crate::calendars::dav_share(&client, &url, &crate::calendars::principal("sam", false).unwrap(), true).unwrap();
    crate::calendars::dav_share(&client, &url, &crate::calendars::principal("office", true).unwrap(), false).unwrap();
    let shared = read();
    assert_eq!(shared.people.iter().map(|p| (p.name.as_str(), p.access, p.group)).collect::<Vec<_>>(), [("sam", crate::calendars::Access::Write, false), ("office", crate::calendars::Access::Read, true)]);
    crate::calendars::dav_unshare(&client, &url, "principal:principals/users/sam").unwrap();
    assert_eq!(dav.shares("pro"), [("principal:principals/groups/office".to_string(), false)]);
    assert_eq!(read().people.len(), 1);
}

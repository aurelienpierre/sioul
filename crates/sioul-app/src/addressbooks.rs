// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Address books behind Contacts ⚙ ▸ Address books (wave D, 18 November
//! 2026; `AddressBooksSetup.qml`): each book's line from the core
//! (`sioul_core::addressbooks`), made, renamed (with Undo), merged into
//! another (its cards moved as one batch, then the emptied book removed,
//! undone as a whole), removed (its cards moved, or deleted with it after a
//! dated copy, with Undo while its server has not deleted it), left when
//! someone else's; its sharing asked of its server in a thread of its own,
//! through the calendars' sharing calls (Nextcloud's `oc:share`), the answer
//! the `bookShared` signal, and kept for the questions before a merge or a
//! move into it (who will see the cards, review F4).
//! What is made, renamed or removed is written here and sent by the next sync.
//! Only the window does any of this: agents never change an address book.

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use serde::Deserialize;
use sioul_core::addressbooks::{self, Removal};
use sioul_core::contactbatch::{self, Outcome};
use sioul_core::contacts;
use sioul_core::vdir::{self, State};
use sioul_sync::calendars::{self, Access, Sharer, Sharing};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Each book's sharing as its server last answered, by place: the names of
/// those who see it (none: shared with nobody). Asked again when its line opens.
static SHARED_WITH: Mutex<BTreeMap<String, Vec<String>>> = Mutex::new(BTreeMap::new());

/// Who sees a book, as its server last answered; None when not asked yet.
pub(crate) fn shared_with(place: &str) -> Option<Vec<String>> {
    SHARED_WITH.lock().ok().and_then(|known| known.get(place).cloned())
}

/// Why a book's line refuses a change, in words: its server did not make it,
/// Google's own, or not said to be yours.
fn refusal(book: &vdir::Collection, line: &addressbooks::BookLine, not_yours: &str) -> String {
    if State::load(&book.state_path()).refused {
        tr().text("book-refused", None)
    } else if !line.fixed.is_empty() {
        line.fixed.clone()
    } else {
        tr().text(not_yours, None)
    }
}

/// Contacts ⚙'s block, as JSON: {books: [line], homes: [{id, name}]}.
pub(crate) fn setup() -> String {
    let config = load_config();
    json(&serde_json::json!({ "books": addressbooks::lines(&config, tr()), "homes": addressbooks::homes(&config, tr()) }))
}

#[derive(Deserialize)]
struct NewBook {
    account: String,
    name: String,
}

/// A new address book (JSON {account, name}): made here, on its server by the next sync. "" done, else why not.
pub(crate) fn make(qt: &QtThread, shared: &Arc<Shared>, text: &str) -> String {
    let Ok(asked) = serde_json::from_str::<NewBook>(text) else { return tr().text("book-needs-name", None) };
    match addressbooks::make(&load_config(), &asked.account, &asked.name, tr()) {
        Ok(made) => {
            crate::pim::nudge(shared, &made.account);
            crate::pim::show_pim(qt, shared);
            tell(qt, shared, say("books-made", &[("name", made.name)]));
            String::new()
        }
        Err(problem) => problem,
    }
}

/// An address book renamed here, sent by the next sync; with Undo. "" done, else why not.
pub(crate) fn rename(qt: &QtThread, shared: &Arc<Shared>, place: &str, name: &str) -> String {
    let config = load_config();
    let Some(book) = addressbooks::find(place) else { return tr().text("books-gone", None) };
    let line = addressbooks::line(&book, &config, tr());
    if !line.can_rename {
        return refusal(&book, &line, "book-not-yours-to-rename");
    }
    if name.trim().is_empty() {
        return tr().text("book-needs-name", None);
    }
    let was = book.name.clone();
    if vdir::rename_collection(&book, name).is_err() {
        return tr().text("books-rename-failed", None);
    }
    crate::pim::nudge(shared, &book.account);
    crate::pim::show_pim(qt, shared);
    let place = place.to_string();
    crate::mail::offer_back(qt, shared, say("books-renamed", &[("name", name.trim().to_string())]), move |qt, shared| match addressbooks::find(&place) {
        Some(book) => {
            let back = vdir::rename_collection(&book, &was).err().map(|_| tr().text("books-rename-failed", None)).unwrap_or_default();
            crate::pim::nudge(shared, &book.account);
            crate::pim::show_pim(qt, shared);
            back
        }
        None => tr().text("undo-too-late", None),
    });
    String::new()
}

/// "Its groups decide who may reach me" set for a book (the owner's answer
/// of 18 November 2026), for the writers it has now, in the settings every
/// device reads; the lists read it at once. "" done, else why not.
pub(crate) fn set_groups(qt: &QtThread, shared: &Arc<Shared>, place: &str, on: bool) -> String {
    let Some(book) = addressbooks::find(place) else { return tr().text("books-gone", None) };
    if addressbooks::set_groups_count(&crate::backend::config_path(), &book, on).is_err() {
        return tr().text("books-groups-failed", None);
    }
    crate::pim::show_pim(qt, shared);
    tell(qt, shared, say(if on { "books-groups-on" } else { "books-groups-off" }, &[("name", book.name.clone())]));
    String::new()
}

/// What removing a book would do, asked first: JSON {act, sentence, count, into: [[place, name]], note}.
pub(crate) fn removal(place: &str) -> String {
    let Some(book) = addressbooks::find(place) else { return json(&serde_json::json!({ "act": "none", "sentence": tr().text("books-gone", None), "count": 0, "into": [], "note": "" })) };
    let asked: Removal = addressbooks::removal(&book, &load_config(), tr());
    json(&asked)
}

/// A book removed as `how` says: "empty" (none held), "delete" (its cards
/// with it, a dated copy first, with Undo while its server has not deleted
/// it), "leave" (someone else's), or "move:<place>" (its cards moved there
/// as one batch, then it, undone as a whole). "" once done or started, else
/// why not.
pub(crate) fn remove(qt: &QtThread, shared: &Arc<Shared>, place: &str, how: &str) -> String {
    let Some(book) = addressbooks::find(place) else { return tr().text("books-gone", None) };
    let config = load_config();
    let line = addressbooks::line(&book, &config, tr());
    let failed = |_: String| tr().text("books-remove-failed", None);
    let said = match how {
        "leave" if line.can_leave => vdir::stop_showing(&book).map(|()| say("books-left", &[("name", book.name.clone())])).map_err(failed),
        "empty" if line.can_remove => vdir::delete_collection(&book).map(|()| say("books-removed-empty", &[("name", book.name.clone())])).map_err(failed),
        "delete" if line.can_remove => match vdir::remove_collection(&book) {
            Ok(copy) => {
                crate::pim::nudge(shared, &book.account);
                crate::pim::show_pim(qt, shared);
                let (kind, account, id, name) = (book.kind, book.account.clone(), book.id.clone(), book.name.clone());
                let here_only = book.account == vdir::LOCAL || State::load(&book.state_path()).pending;
                crate::mail::offer_back(qt, shared, say("books-removed", &[("name", book.name.clone()), ("copy", copy.display().to_string())]), move |qt, shared| {
                    let back = if here_only { restore_from_copy(kind, &account, &id, &name, &copy) } else { vdir::unremove_collection(kind, &account, &id).map_err(|_| say("books-remove-too-late", &[("copy", copy.display().to_string())])) };
                    crate::pim::nudge(shared, &account);
                    crate::pim::show_pim(qt, shared);
                    match back {
                        Ok(()) => {
                            tell(qt, shared, say("books-remove-undone", &[("name", name.clone())]));
                            String::new()
                        }
                        Err(problem) => problem,
                    }
                });
                return String::new();
            }
            Err(_) => Err(tr().text("books-remove-failed", None)),
        },
        _ => match how.strip_prefix("move:") {
            Some(into) if line.can_remove => return merge(qt, shared, place, into),
            _ => Err(refusal(&book, &line, "book-not-yours-to-remove")),
        },
    };
    match said {
        Ok(said) => {
            crate::pim::nudge(shared, &book.account);
            crate::pim::show_pim(qt, shared);
            tell(qt, shared, said);
            String::new()
        }
        Err(problem) => problem,
    }
}

/// A book of this device (or made here and never sent) whose folder went
/// with its removal, made again from the copy kept first: each card written
/// back, its text and UID as they were (review F5).
fn restore_from_copy(kind: vdir::Kind, account: &str, id: &str, name: &str, copy: &std::path::Path) -> Result<(), String> {
    let late = || say("books-remove-too-late", &[("copy", copy.display().to_string())]);
    let text = std::fs::read_to_string(copy).map_err(|_| late())?;
    if kind.root().join(account).join(id).is_dir() {
        return Err(late());
    }
    vdir::restore_collection(kind, account, id, name).map_err(|_| late())?;
    let book = addressbooks::find(&format!("{account}/{id}")).ok_or_else(late)?;
    for card in cards_of_copy(&text) {
        vdir::write_item(&contacts::new_path(&book), &card).map_err(|_| late())?;
    }
    Ok(())
}

/// Each card of one vCard file, from its `BEGIN:VCARD` to its `END:VCARD`, as written.
fn cards_of_copy(text: &str) -> Vec<String> {
    let mut cards = Vec::new();
    let mut current: Option<String> = None;
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end();
        if bare.eq_ignore_ascii_case("BEGIN:VCARD") {
            current = Some(String::new());
        }
        if let Some(card) = current.as_mut() {
            card.push_str(line);
        }
        if bare.eq_ignore_ascii_case("END:VCARD")
            && let Some(card) = current.take()
        {
            cards.push(card);
        }
    }
    cards
}

/// What merging `from` into `into` would do, asked before it runs (review
/// F4): the count and the target, what its server would not keep, who sees
/// it; JSON {sentence, losses, shared, cannot}.
pub(crate) fn merge_question(from: &str, into: &str) -> String {
    let config = load_config();
    let (Some(source), Some(target)) = (addressbooks::find(from), addressbooks::find(into)) else {
        return json(&serde_json::json!({ "sentence": tr().text("books-gone", None), "losses": "", "shared": "", "cannot": [] }));
    };
    if source.dir == target.dir {
        return json(&serde_json::json!({ "sentence": tr().text("books-merge-itself", None), "losses": "", "shared": "", "cannot": [] }));
    }
    let known = shared_with(into);
    json(&addressbooks::merge_question(&source, &target, &config, tr(), known.as_deref()))
}

/// Every card of `from` moved into `into`, each keeping its UID (the
/// server's own MOVE where it has one, `dav::move_on_server`), as one
/// batch; then `from`, emptied, removed here, and on its server once its
/// cards are gone from it there. Undone as a whole: the book shown again
/// (made again when it was on this device alone), its cards moved back. ""
/// once started, else why not.
pub(crate) fn merge(qt: &QtThread, shared: &Arc<Shared>, from: &str, into: &str) -> String {
    let config = load_config();
    let (Some(source), Some(target)) = (addressbooks::find(from), addressbooks::find(into)) else { return tr().text("books-gone", None) };
    if source.dir == target.dir {
        return tr().text("books-merge-itself", None);
    }
    let line = addressbooks::line(&source, &config, tr());
    if !line.can_remove {
        return refusal(&source, &line, "book-not-yours-to-remove");
    }
    let cards: Vec<contacts::Contact> = source.items().iter().filter_map(|p| contacts::read(p, &source)).collect();
    let provider = sioul_core::capabilities::provider_of(config.account(&target.account));
    let plan = contactbatch::plan_move(&cards.iter().collect::<Vec<_>>(), &target, provider, tr());
    if !plan.cannot.is_empty() {
        return say("books-merge-cannot", &[("n", plan.cannot.len().to_string()), ("why", plan.cannot.iter().map(|c| format!("{}: {}", c.name, c.why)).collect::<Vec<_>>().join("; "))]);
    }
    let (name, place) = (source.name.clone(), from.to_string());
    let kept = say("books-merge-kept", &[("name", name.clone())]);
    let removed = Box::new(move |outcome: &Outcome| {
        if outcome.stopped || !outcome.failed.is_empty() {
            return kept;
        }
        match addressbooks::find(&place).map(|book| vdir::delete_collection(&book)) {
            Some(Ok(())) => say("books-removed-empty", &[("name", name.clone())]),
            Some(Err(_)) => kept,
            None => String::new(),
        }
    });
    if plan.steps.is_empty() {
        return match addressbooks::find(from).map(|book| vdir::delete_collection(&book)) {
            Some(Ok(())) => {
                crate::pim::show_pim(qt, shared);
                tell(qt, shared, say("books-removed-empty", &[("name", source.name.clone())]));
                String::new()
            }
            Some(Err(_)) => say("books-merge-kept", &[("name", source.name.clone())]),
            None => tr().text("books-gone", None),
        };
    }
    // Made again by its Undo when its folder goes at once: on this device alone, or never sent.
    let here_only = source.account == vdir::LOCAL || State::load(&source.state_path()).pending;
    let removed_book = contactbatch::Removed { place: from.to_string(), name: source.name.clone(), here_only };
    crate::contactbatch::start(qt, shared, "merge", plan.steps, Some(removed_book), removed)
}

/// A change of a book's sharing.
pub(crate) enum Change {
    Read,
    /// With a user of the same Nextcloud (`group:<name>` a group of it).
    Share { who: String, write: bool },
    /// A share taken back, by its principal.
    Unshare(String),
}

/// One change asked of the book's server, then its sharing read again.
fn ask(place: &str, change: &Change) -> Result<Sharing, String> {
    let config = load_config();
    let book = addressbooks::find(place).ok_or_else(|| tr().text("books-gone", None))?;
    if crate::backend::offline() {
        return Ok(Sharing { sharer: Sharer::None, people: Vec::new(), link: None, links: false });
    }
    let line = addressbooks::line(&book, &config, tr());
    if !line.fixed.is_empty() || book.account == vdir::LOCAL {
        return Ok(Sharing { sharer: Sharer::None, people: Vec::new(), link: None, links: false });
    }
    let account = config.every_account().find(|a| a.id == book.account).cloned().ok_or_else(|| tr().text("books-gone", None))?;
    let url = State::load(&book.state_path()).url;
    if url.is_empty() {
        return Err(tr().text("books-share-not-yet", None));
    }
    let e = |e: sioul_sync::SyncError| crate::servers::server_said(&e, &account.id);
    let client = sioul_sync::dav::client_for(&account).map_err(e)?;
    match change {
        Change::Read => {}
        Change::Share { who, write } => {
            if !line.can_share {
                return Err(refusal(&book, &line, "book-not-yours-to-share"));
            }
            let (name, group) = who.strip_prefix("group:").map_or((who.as_str(), false), |name| (name, true));
            let Some(principal) = calendars::principal(name, group) else { return Err(tr().text("books-share-elsewhere", None)) };
            calendars::dav_share(&client, &url, &principal, *write).map_err(e)?;
        }
        Change::Unshare(who) => calendars::dav_unshare(&client, &url, who).map_err(e)?,
    }
    calendars::dav_sharing(&client, &url).map_err(e)
}

/// A book's writers as its server answers now, kept in its state at once (the
/// next listing says the same): when they change and its groups stop
/// counting for who may reach the person, said in one sentence with how to
/// turn them on again.
fn writers_now(qt: &QtThread, shared: &Arc<Shared>, place: &str, now: &Sharing) {
    let Some(book) = addressbooks::find(place) else { return };
    if now.sharer == Sharer::None {
        return;
    }
    let config = load_config();
    let path = book.state_path();
    let mut state = State::load(&path);
    // Named as the listing names them (`dav::Prop::writers`): the last segment of their principal.
    let mut writers: Vec<String> = now
        .people
        .iter()
        .filter(|p| p.access == Access::Write)
        .map(|p| {
            let href = p.id.trim().trim_end_matches('/');
            let name = href.rsplit('/').next().unwrap_or(href).to_string();
            if href.contains("/groups/") { format!("group:{name}") } else { name }
        })
        .collect();
    writers.sort();
    if state.writers == writers {
        return;
    }
    state.writers = writers;
    state.writers_known = true;
    if state.save(&path).is_err() {
        return;
    }
    tell_groups_stopped();
    crate::pim::show_pim(qt, shared);
}

/// One notice, of the kind for things to know, for each book whose groups
/// stopped deciding who reaches the person since this device last told of
/// it (review V3): when its writers change, when it is shared to write here
/// or elsewhere, and once at the update for a book already so. It names the
/// book and who can now write it, and says where to turn its groups on;
/// marked told just before it is posted, so one per book and change.
/// Posted where someone is at the computer, as Sioul's informative notices
/// are; a phone posts none.
pub(crate) fn tell_groups_stopped() {
    use sioul_core::notice::{Kind, Notice, address};
    let config = load_config();
    for (book, writers) in addressbooks::groups_to_tell(&config) {
        let who: Vec<String> = writers.iter().map(|w| if w == "owner" { tr().text("book-groups-owner", None) } else { w.strip_prefix("group:").unwrap_or(w).to_string() }).collect();
        let pairs = [("name", book.name.clone()), ("who", who.join(", "))];
        let notice = Notice::new(address::settings("addressbooks"), Kind::Minor, say("books-groups-notice-title", &pairs), say("books-groups-notice-body", &pairs));
        let claim: crate::tell::Claim = Box::new(move || match addressbooks::mark_groups_told(&book, &writers) {
            Ok(()) => true,
            Err(e) => {
                // Not written: told by none, rather than again and again.
                eprintln!("Sioul: address books: {e}");
                false
            }
        });
        crate::tell::tell_with(notice, sioul_core::notice::told(None), None, Some(claim), true);
    }
}

/// Who a book is shared with, in one line: "Only you.", "Sam can read it. The office can change it."
fn sharing_line(sharing: &Sharing) -> String {
    if sharing.sharer == Sharer::None {
        return tr().text("books-share-not-here", None);
    }
    if sharing.people.is_empty() {
        return tr().text("books-shared-none", None);
    }
    sharing.people.iter().map(|p| say(if p.access == Access::Write { "books-shared-write" } else { "books-shared-read" }, &[("who", p.name.clone())])).collect::<Vec<_>>().join(" ")
}

/// Reads or changes a book's sharing in a thread of its own; its answer comes
/// as `book_shared(place, JSON {line, sharer, people, problem})`. Taking a
/// share back is said with Undo (it shares again).
pub(crate) fn sharing(qt: &QtThread, shared: &Arc<Shared>, place: &str, change: Change) {
    let (qt, shared, place) = (qt.clone(), Arc::clone(shared), place.to_string());
    std::thread::spawn(move || {
        let undo = match &change {
            Change::Unshare(who) => ask(&place, &Change::Read).ok().and_then(|s| s.people.into_iter().find(|p| p.id == *who)).map(|p| {
                let name = who.rsplit(['/', ':']).next().unwrap_or(who).to_string();
                (if p.group { format!("group:{name}") } else { name }, p.access == Access::Write)
            }),
            _ => None,
        };
        let answer = match ask(&place, &change) {
            Ok(now) => {
                if let Ok(mut known) = SHARED_WITH.lock() {
                    known.insert(place.clone(), now.people.iter().map(|p| p.name.clone()).collect());
                }
                writers_now(&qt, &shared, &place, &now);
                if let Some((who, write)) = undo {
                    let back = place.clone();
                    let name = who.strip_prefix("group:").unwrap_or(&who).to_string();
                    crate::mail::offer_back(&qt, &shared, say("books-unshared", &[("who", name)]), move |qt, shared| {
                        sharing(qt, shared, &back, Change::Share { who: who.clone(), write });
                        String::new()
                    });
                }
                json(&serde_json::json!({ "line": sharing_line(&now), "sharer": now.sharer, "people": now.people, "problem": "" }))
            }
            Err(problem) => json(&serde_json::json!({ "problem": problem })),
        };
        let _ = qt.queue(move |mut sioul| sioul.as_mut().book_shared(cxx_qt_lib::QString::from(&place), cxx_qt_lib::QString::from(&answer)));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Delete them with it" undone for a book of this device (review F5):
    /// made again from the copy kept first, with its name, each card's text
    /// and UID as they were; never twice.
    #[test]
    fn a_book_of_this_device_comes_back_from_its_copy() {
        sioul_core::config::stand_ins_only();
        let id = "app-restored";
        let place = format!("{}/{id}", vdir::LOCAL);
        let _ = std::fs::remove_dir_all(vdir::Kind::Contacts.root().join(vdir::LOCAL).join(id));
        let dir = vdir::prepare(vdir::Kind::Contacts, vdir::LOCAL, id, "Home", None).unwrap();
        for (uid, name) in [("h-1", "Lou Exemple"), ("h-2", "Sam Exemple")] {
            std::fs::write(dir.join(format!("{uid}.vcf")), format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\nFN:{name}\r\nEND:VCARD\r\n")).unwrap();
        }
        let copy = vdir::remove_collection(&addressbooks::find(&place).unwrap()).unwrap();
        assert!(addressbooks::find(&place).is_none(), "its folder gone at once");
        restore_from_copy(vdir::Kind::Contacts, vdir::LOCAL, id, "Home", &copy).unwrap();
        let back = addressbooks::find(&place).expect("made again");
        let mut uids: Vec<String> = back.items().iter().filter_map(|p| std::fs::read_to_string(p).ok()).filter_map(|t| contactbatch::uid_of(&t)).collect();
        uids.sort();
        assert_eq!((back.name.as_str(), uids), ("Home", vec!["h-1".to_string(), "h-2".to_string()]));
        assert!(restore_from_copy(vdir::Kind::Contacts, vdir::LOCAL, id, "Home", &copy).is_err(), "never twice");
        assert_eq!(cards_of_copy("BEGIN:VCARD\nFN:A\nEND:VCARD\nnoise\nBEGIN:VCARD\r\nFN:B\r\nEND:VCARD\r\n").len(), 2);
        let _ = std::fs::remove_dir_all(vdir::Kind::Contacts.root().join(vdir::LOCAL).join(id));
    }
}

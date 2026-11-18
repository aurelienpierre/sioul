// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Address books behind Contacts ⚙ ▸ Address books (wave D, 18 November
//! 2026; `AddressBooksSetup.qml`): each book's line from the core
//! (`sioul_core::addressbooks`), made, renamed (with Undo), merged into
//! another (its cards moved as one batch, then the emptied book removed,
//! undone as a whole), removed (its cards moved, or deleted with it after a
//! dated copy), left when someone else's; its sharing asked of its server
//! in a thread of its own, through the calendars' sharing calls (Nextcloud's
//! `oc:share`), the answer the `bookShared` signal.
//! What is made, renamed or removed is written here and sent by the next sync.
//! Only the window does any of this: agents never change an address book.

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use serde::Deserialize;
use sioul_core::addressbooks::{self, Removal};
use sioul_core::contactbatch::{self, Outcome};
use sioul_core::contacts;
use sioul_core::vdir::{self, State};
use sioul_sync::calendars::{self, Access, Sharer, Sharing};
use std::sync::Arc;

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
    if !addressbooks::line(&book, &config, tr()).can_rename {
        return tr().text("book-not-yours-to-remove", None);
    }
    let was = book.name.clone();
    if let Err(e) = vdir::rename_collection(&book, name) {
        return e;
    }
    crate::pim::nudge(shared, &book.account);
    crate::pim::show_pim(qt, shared);
    let place = place.to_string();
    crate::mail::offer_back(qt, shared, say("books-renamed", &[("name", name.trim().to_string())]), move |qt, shared| match addressbooks::find(&place) {
        Some(book) => {
            let back = vdir::rename_collection(&book, &was).err().unwrap_or_default();
            crate::pim::nudge(shared, &book.account);
            crate::pim::show_pim(qt, shared);
            back
        }
        None => tr().text("undo-too-late", None),
    });
    String::new()
}

/// What removing a book would do, asked first: JSON {act, sentence, count, into: [[place, name]], note}.
pub(crate) fn removal(place: &str) -> String {
    let Some(book) = addressbooks::find(place) else { return json(&serde_json::json!({ "act": "none", "sentence": tr().text("books-gone", None), "count": 0, "into": [], "note": "" })) };
    let asked: Removal = addressbooks::removal(&book, &load_config(), tr());
    json(&asked)
}

/// A book removed as `how` says: "empty" (none held), "delete" (its cards
/// with it, a dated copy first), "leave" (someone else's), or
/// "move:<place>" (its cards moved there as one batch, then it, undone as a
/// whole). "" once done or started, else why not.
pub(crate) fn remove(qt: &QtThread, shared: &Arc<Shared>, place: &str, how: &str) -> String {
    let Some(book) = addressbooks::find(place) else { return tr().text("books-gone", None) };
    let config = load_config();
    let line = addressbooks::line(&book, &config, tr());
    let said = match how {
        "leave" if line.can_leave => vdir::stop_showing(&book).map(|()| say("books-left", &[("name", book.name.clone())])),
        "empty" if line.can_remove => vdir::delete_collection(&book).map(|()| say("books-removed-empty", &[("name", book.name.clone())])),
        "delete" if line.can_remove => vdir::remove_collection(&book).map(|copy| say("books-removed", &[("name", book.name.clone()), ("copy", copy.display().to_string())])),
        _ => match how.strip_prefix("move:") {
            Some(into) if line.can_remove => return merge(qt, shared, place, into),
            _ => Err(tr().text("book-not-yours-to-remove", None)),
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

/// Every card of `from` moved into `into`, each keeping its UID (the
/// server's own MOVE where it has one, `dav::move_on_server`), as one
/// batch; then `from`, emptied, removed here, and on its server once its
/// cards are gone from it there. Undone as a whole: the book shown again,
/// its cards moved back. "" once started, else why not.
pub(crate) fn merge(qt: &QtThread, shared: &Arc<Shared>, from: &str, into: &str) -> String {
    let config = load_config();
    let (Some(source), Some(target)) = (addressbooks::find(from), addressbooks::find(into)) else { return tr().text("books-gone", None) };
    if source.dir == target.dir || !addressbooks::line(&source, &config, tr()).can_remove {
        return tr().text("book-not-yours-to-remove", None);
    }
    let cards: Vec<contacts::Contact> = source.items().iter().filter_map(|p| contacts::read(p, &source)).collect();
    let provider = sioul_core::capabilities::provider_of(config.account(&target.account));
    let plan = contactbatch::plan_move(&cards.iter().collect::<Vec<_>>(), &target, provider, tr());
    if !plan.cannot.is_empty() {
        return say("books-merge-cannot", &[("n", plan.cannot.len().to_string()), ("why", plan.cannot.iter().map(|c| format!("{}: {}", c.name, c.why)).collect::<Vec<_>>().join("; "))]);
    }
    let (name, place) = (source.name.clone(), from.to_string());
    let removed = Box::new(move |outcome: &Outcome| {
        if outcome.stopped || !outcome.failed.is_empty() {
            return say("books-merge-kept", &[("name", name.clone())]);
        }
        match addressbooks::find(&place).map(|book| vdir::delete_collection(&book)) {
            Some(Ok(())) => say("books-removed-empty", &[("name", name.clone())]),
            Some(Err(problem)) => problem,
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
            Some(Err(problem)) => problem,
            None => tr().text("books-gone", None),
        };
    }
    crate::contactbatch::start(qt, shared, "merge", plan.steps, Some(from.to_string()), removed)
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
                return Err(tr().text("book-not-yours-to-remove", None));
            }
            let (name, group) = who.strip_prefix("group:").map_or((who.as_str(), false), |name| (name, true));
            let Some(principal) = calendars::principal(name, group) else { return Err(tr().text("books-share-elsewhere", None)) };
            calendars::dav_share(&client, &url, &principal, *write).map_err(e)?;
        }
        Change::Unshare(who) => calendars::dav_unshare(&client, &url, who).map_err(e)?,
    }
    calendars::dav_sharing(&client, &url).map_err(e)
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

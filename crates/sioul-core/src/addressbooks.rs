// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Address books, as the Contacts page's ⚙ manages them (wave D, 18 November
//! 2026): each book's line (its name, where it is kept, how many cards, whose
//! it is, whether it can be written) and what may be done with it on its
//! server: made, renamed, merged into another, removed (its cards moved to
//! another book or deleted with it, a dated copy first), shared on a
//! Nextcloud. A server with one fixed book (Google) offers only what it
//! allows, and says so.
//!
//! An address book says where a card is kept and with whom it is shared.
//! Who may reach you is decided by a card's groups, never by its book
//! (porch.rs), with one exception, about trust (the owner's answer of 18
//! November 2026): the groups of a book someone else can write (one shared
//! to write, one that is someone else's) count only once you say so for
//! that book ("Its groups decide who may reach me", `groups_count`), since
//! someone who can write it could otherwise add a card in a group that a
//! list lets through. Its cards stay your contacts, their own lines on a
//! list (UID, address, number) still count; only their groups are held back.
//!
//! The acts themselves go through the generic collections' layer (`vdir`:
//! `create`, `rename_collection`, `delete_collection`, `remove_collection`,
//! `stop_showing`) and are sent by the next sync (`dav::send_changes`,
//! `create_pending`); cards move through the batches (`contactbatch`).

use crate::capabilities::{Provider, provider_of_collection};
use crate::config::Config;
use crate::i18n::Translator;
use crate::vdir::{self, Collection, Kind, State};
use serde::Serialize;

/// One address book, as its line says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BookLine {
    /// "account/id".
    pub place: String,
    pub account: String,
    pub name: String,
    /// Where it is kept: "example.org", "Google", "on this device alone".
    pub kept: String,
    pub count: usize,
    /// "42 contacts".
    pub counted: String,
    /// "Yours", "Shared with you by sam", "Its server does not say whose it is".
    pub whose: String,
    pub writable: bool,
    /// "Can be written", "Read only".
    pub access: String,
    /// Made here, not on its server yet: "Made here; on its server at the next sync."
    pub waiting: String,
    pub can_rename: bool,
    /// Merged into another, or removed, by you: yours, and written.
    pub can_remove: bool,
    /// Shared with others from here: yours, on a server (Nextcloud's sharing is asked when its line opens).
    pub can_share: bool,
    /// Someone else's, its server says: no longer shown here, its owner's book untouched.
    pub can_leave: bool,
    /// A server with one fixed book: what it allows, in a sentence; else "".
    pub fixed: String,
    /// "Its groups decide who may reach me", as it reads now (`groups_count`).
    pub groups: bool,
    /// Why its groups stopped counting and how to turn them on, when someone
    /// else can write it and nothing was said for those writers; else "".
    pub groups_note: String,
}

/// Every address book, by account then name.
pub fn lines(config: &Config, tr: &Translator) -> Vec<BookLine> {
    vdir::collections(Kind::Contacts).into_iter().map(|c| line(&c, config, tr)).collect()
}

/// The owner a Nextcloud names in a book shared with you ("contacts_shared_by_sam"), when it does.
fn owner_in_id(id: &str) -> Option<&str> {
    id.rsplit_once("_shared_by_").map(|(_, owner)| owner).filter(|o| !o.is_empty())
}

/// Whether a book is Google's: one fixed book, made, renamed, shared and
/// deleted on Google's own pages only (developers.google.com/people/carddav).
pub fn is_google(collection: &Collection, config: &Config) -> bool {
    provider_of_collection(config.account(&collection.account), collection) == Provider::Google
}

/// A book's line.
pub fn line(collection: &Collection, config: &Config, tr: &Translator) -> BookLine {
    let state = State::load(&collection.state_path());
    let trust = groups_count(collection, config);
    let local = collection.account == vdir::LOCAL;
    let google = is_google(collection, config);
    let place = collection.place(config);
    let count = collection.items().len();
    let mut args = tr.counted(count);
    args.set("n", count as i64);
    let yours = local || state.pending || state.owned == Some(true);
    let whose = if yours {
        tr.text("book-whose-yours", None)
    } else if state.owned == Some(false) {
        match owner_in_id(&collection.id) {
            Some(owner) => {
                let mut args = crate::i18n::args();
                args.set("owner", owner.to_string());
                tr.text("book-whose-shared-by", Some(&args))
            }
            None => tr.text("book-whose-shared", None),
        }
    } else {
        tr.text("book-whose-unknown", None)
    };
    let writable = !collection.read_only;
    BookLine {
        place: format!("{}/{}", collection.account, collection.id),
        account: collection.account.clone(),
        name: collection.name.clone(),
        kept: if local { tr.text("book-kept-here", None) } else { place },
        count,
        counted: tr.text("book-count", Some(&args)),
        whose,
        writable,
        access: tr.text(if writable { "book-writable" } else { "book-read-only" }, None),
        waiting: if state.refused {
            tr.text("book-refused", None)
        } else if state.pending && !local {
            tr.text("book-waiting", None)
        } else if state.kept == "held" {
            tr.text("book-kept-held", None)
        } else {
            String::new()
        },
        can_rename: writable && !google && !state.refused && (yours || state.owned.is_none()),
        can_remove: writable && !google && yours,
        can_share: writable && !google && !local && !state.refused && !state.pending && yours,
        can_leave: !local && !google && state.owned == Some(false) && !state.url.is_empty(),
        fixed: if google { tr.text("book-google-fixed", None) } else { String::new() },
        groups: trust.count,
        groups_note: if !local && !google && state.owned != Some(false) && !state.writers_known && !state.url.is_empty() {
            // Its groups count, as before; said, since nothing tells who else can write it.
            tr.text("book-writers-unsaid", None)
        } else if trust.said || trust.writers.is_empty() {
            String::new()
        } else {
            let mut args = crate::i18n::args();
            let named: Vec<String> = trust.writers.iter().map(|w| if w == "owner" { tr.text("book-groups-owner", None) } else { w.strip_prefix("group:").unwrap_or(w).to_string() }).collect();
            args.set("who", named.join(", "));
            tr.text("book-groups-stopped", Some(&args))
        },
    }
}

/// Whether a book's groups decide who may reach the person, and why.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupsTrust {
    pub count: bool,
    /// Who else can write it now (`writers`).
    pub writers: Vec<String>,
    /// The person said it for these writers; else the default holds.
    pub said: bool,
}

/// Who else can write a book now: its owner when it is someone else's, and
/// those it is shared with to write, as its server last listed them
/// (`State::writers`), sorted. None for a book of this device or Google's.
/// A server that says neither whose a book is nor whom it is shared with
/// gives none: the book is taken as yours alone, as its line says
/// ("Its server does not say whose it is").
pub fn writers(collection: &Collection, state: &State) -> Vec<String> {
    if collection.account == vdir::LOCAL {
        return Vec::new();
    }
    let mut who: Vec<String> = state.writers.clone();
    if state.owned == Some(false) {
        who.push(owner_in_id(&collection.id).map_or_else(|| "owner".to_string(), str::to_string));
    }
    who.sort();
    who.dedup();
    who
}

/// "Its groups decide who may reach me" for a book (the owner's answer of 18
/// November 2026): as the person said it for the writers it has now; unsaid,
/// or said for other writers, on for a book only the person can write, off
/// for one someone else can write.
pub fn groups_count(collection: &Collection, config: &Config) -> GroupsTrust {
    let state = State::load(&collection.state_path());
    let writers = writers(collection, &state);
    let place = format!("{}/{}", collection.account, collection.id);
    let said = config.contacts.books.get(&place).and_then(|b| {
        let mut theirs = b.writers.clone();
        theirs.sort();
        theirs.dedup();
        b.groups.filter(|_| theirs == writers)
    });
    GroupsTrust { count: said.unwrap_or(writers.is_empty()), said: said.is_some(), writers }
}

/// A list's name in a sentence: "Safe", "Sûrs".
fn list_name(standing: crate::porch::Standing, tr: &Translator) -> String {
    tr.text(&format!("sender-list-{}", standing.as_str()), None)
}

/// "Safe: Clients, Amis; Blocked: Spam".
fn lists_said(found: &[(crate::porch::Standing, Vec<String>)], tr: &Translator) -> String {
    found.iter().map(|(standing, groups)| format!("{}: {}", list_name(*standing, tr), groups.join(", "))).collect::<Vec<_>>().join("; ")
}

/// Adds a card's standing through a group to counts kept in the lists' order.
fn count_in(counts: &mut Vec<(crate::porch::Standing, usize, Vec<String>)>, standing: crate::porch::Standing, group: String) {
    match counts.iter_mut().find(|(s, _, _)| *s == standing) {
        Some((_, n, groups)) => {
            *n += 1;
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
        None => counts.push((standing, 1, vec![group])),
    }
    counts.sort_by_key(|(s, _, _)| crate::porch::Standing::ALL.iter().position(|a| a == s));
}

/// What moving these cards into `target` does to who may reach the person
/// (review V1), in sentences, counted by the lists' own step for groups
/// (`porch::Senders::standing_by_groups`): into a book whose groups are held
/// back, the cards that a group puts on a list count as plain contacts
/// there; out of one into a book whose groups count, the cards whose group
/// a list reads are let through as that list says. When nothing crosses that
/// line: "Who may reach you stays as its groups say."
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Reach {
    /// Some card crosses the line: the act is asked first, these sentences said.
    pub crosses: bool,
    pub said: Vec<String>,
}

pub fn reach_of_move(cards: &[&crate::contacts::Contact], target: &Collection, config: &Config, senders: &crate::porch::Senders, tr: &Translator) -> Reach {
    let held = groups_held_back(config);
    let into = !held.contains(&format!("{}/{}", target.account, target.id));
    let (mut kept, mut let_in) = (Vec::new(), Vec::new());
    for card in cards {
        let from = !held.contains(&card.place);
        if from == into {
            continue;
        }
        if let Some((standing, group)) = senders.standing_by_groups(card, &card.categories) {
            if from { count_in(&mut kept, standing, group) } else { count_in(&mut let_in, standing, group) }
        }
    }
    let mut said: Vec<String> = kept
        .iter()
        .map(|(standing, n, _)| {
            let mut args = tr.counted(*n);
            args.set("n", *n as i64);
            args.set("list", list_name(*standing, tr));
            tr.text("batch-reach-held", Some(&args))
        })
        .collect();
    if !let_in.is_empty() {
        let n: usize = let_in.iter().map(|(_, n, _)| n).sum();
        let mut args = tr.counted(n);
        args.set("n", n as i64);
        args.set("lists", lists_said(&let_in.iter().map(|(s, _, g)| (*s, g.clone())).collect::<Vec<_>>(), tr));
        said.push(tr.text("batch-reach-let", Some(&args)));
    }
    let crosses = !said.is_empty();
    if !crosses {
        said.push(tr.text("batch-reach-stays", None));
    }
    Reach { crosses, said }
}

/// What giving these cards the groups `added` does where their book's groups
/// are held back (review V1): there the group decides nothing until its
/// groups are turned on. "" when no such card would stand on a list by it.
pub fn reach_of_groups(cards: &[&crate::contacts::Contact], added: &[String], config: &Config, senders: &crate::porch::Senders, tr: &Translator) -> String {
    let held = groups_held_back(config);
    let mut counts = Vec::new();
    for card in cards.iter().filter(|c| held.contains(&c.place)) {
        if let Some((standing, group)) = senders.standing_by_groups(card, added) {
            count_in(&mut counts, standing, group);
        }
    }
    if counts.is_empty() {
        return String::new();
    }
    let n: usize = counts.iter().map(|(_, n, _)| n).sum();
    let mut args = tr.counted(n);
    args.set("n", n as i64);
    args.set("lists", lists_said(&counts.iter().map(|(s, _, g)| (*s, g.clone())).collect::<Vec<_>>(), tr));
    tr.text("batch-reach-group-held", Some(&args))
}

/// What an import's new cards do in `target` when its groups are held back
/// (review V1): those a group would put on a list count as plain contacts
/// there. "" when the book's groups count, or no such card.
pub fn reach_of_import(cards: &[crate::contacts::Contact], target: &Collection, config: &Config, senders: &crate::porch::Senders, tr: &Translator) -> String {
    if !groups_held_back(config).contains(&format!("{}/{}", target.account, target.id)) {
        return String::new();
    }
    let mut counts = Vec::new();
    for card in cards {
        if let Some((standing, group)) = senders.standing_by_groups(card, &card.categories) {
            count_in(&mut counts, standing, group);
        }
    }
    if counts.is_empty() {
        return String::new();
    }
    let n: usize = counts.iter().map(|(_, n, _)| n).sum();
    let mut args = tr.counted(n);
    args.set("n", n as i64);
    args.set("book", target.name.clone());
    args.set("lists", lists_said(&counts.iter().map(|(s, _, g)| (*s, g.clone())).collect::<Vec<_>>(), tr));
    tr.text("import-reach-held", Some(&args))
}

/// Where this device notes the books whose groups it said stopped deciding,
/// with the writers it said it for (review V3): `$XDG_STATE_HOME/sioul/address-books-told.toml`.
pub fn told_path() -> std::path::PathBuf {
    crate::config::state_dir().join("address-books-told.toml")
}

/// The books whose groups stopped deciding who may reach the person because
/// someone else can now write them, not said for these writers, and not yet
/// told on this device for them (review V3): each with its writers. At the
/// update, a book already shared to write is one of them, once.
pub fn groups_to_tell(config: &Config) -> Vec<(Collection, Vec<String>)> {
    let told: std::collections::BTreeMap<String, Vec<String>> = std::fs::read_to_string(told_path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    vdir::collections(Kind::Contacts)
        .into_iter()
        .filter_map(|c| {
            let trust = groups_count(&c, config);
            let place = format!("{}/{}", c.account, c.id);
            (!trust.count && !trust.said && !trust.writers.is_empty() && told.get(&place) != Some(&trust.writers)).then_some((c, trust.writers))
        })
        .collect()
}

/// A book told of for these writers, on this device: not told again until they change.
pub fn mark_groups_told(collection: &Collection, writers: &[String]) -> Result<(), String> {
    let path = told_path();
    crate::filelock::with_lock(&path, || {
        let mut told: std::collections::BTreeMap<String, Vec<String>> = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        told.insert(format!("{}/{}", collection.account, collection.id), writers.to_vec());
        let text = toml::to_string(&told).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        crate::filelock::replace(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    })
}

/// The books whose groups the lists of who may reach the person do not
/// read now, by place ("account/id"): what `porch::Book` holds back.
pub fn groups_held_back(config: &Config) -> std::collections::BTreeSet<String> {
    vdir::collections(Kind::Contacts).into_iter().filter(|c| !groups_count(c, config).count).map(|c| format!("{}/{}", c.account, c.id)).collect()
}

/// "Its groups decide who may reach me" set for a book, with the writers it
/// has now, into the settings (`[contacts.books."<place>"]`), comments kept;
/// every device reads it.
pub fn set_groups_count(path: &std::path::Path, collection: &Collection, on: bool) -> Result<(), String> {
    let state = State::load(&collection.state_path());
    let writers = writers(collection, &state);
    let place = format!("{}/{}", collection.account, collection.id);
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        if !doc.contains_key("contacts") {
            doc["contacts"] = toml_edit::table();
        }
        let contacts = doc["contacts"].as_table_mut().ok_or_else(|| format!("{}: contacts is not a table", path.display()))?;
        if !contacts.contains_key("books") {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            contacts.insert("books", toml_edit::Item::Table(table));
        }
        let books = contacts.get_mut("books").and_then(toml_edit::Item::as_table_mut).ok_or_else(|| format!("{}: contacts.books is not a table", path.display()))?;
        let mut table = toml_edit::Table::new();
        table.insert("groups", toml_edit::value(on));
        let mut list = toml_edit::Array::new();
        for writer in &writers {
            list.push(writer.as_str());
        }
        table.insert("writers", toml_edit::value(list));
        books.insert(&place, toml_edit::Item::Table(table));
        crate::config::write_document(path, &doc)
    })
}

/// An account a new address book can go into, as the form offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Home {
    pub id: String,
    pub name: String,
}

/// Where a new address book can go: each contacts-and-calendars account
/// whose server makes them (not Google's), then this device alone.
pub fn homes(config: &Config, tr: &Translator) -> Vec<Home> {
    let mut out: Vec<Home> = config
        .accounts
        .iter()
        .filter(|a| a.is_dav() && a.auth.as_deref() != Some("google"))
        .map(|a| Home { id: a.id.clone(), name: a.address.clone().unwrap_or_else(|| a.id.clone()) })
        .collect();
    out.push(Home { id: vdir::LOCAL.into(), name: tr.text("book-kept-here", None) });
    out
}

/// A new address book: made here at once, on its server by the next sync
/// (an extended MKCOL, RFC 5689). Never on Google, never on an account that
/// is no contacts-and-calendars one.
pub fn make(config: &Config, account: &str, name: &str, tr: &Translator) -> Result<Collection, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(tr.text("book-needs-name", None));
    }
    if account != vdir::LOCAL && !homes(config, tr).iter().any(|h| h.id == account) {
        return Err(tr.text("book-no-home", None));
    }
    vdir::create(Kind::Contacts, account, name, None, &[])
}

/// What removing a book would do, asked first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Removal {
    /// "empty": removed at once; "cards": its cards moved to another book or
    /// deleted with it; "leave": someone else's, no longer shown; "none".
    pub act: &'static str,
    pub sentence: String,
    pub count: usize,
    /// The books its cards can go to: [{place, name}].
    pub into: Vec<(String, String)>,
    /// What the deletion keeps: "A copy of its cards is kept on this device first."
    pub note: String,
}

/// What removing `collection` would do, in words.
pub fn removal(collection: &Collection, config: &Config, tr: &Translator) -> Removal {
    let line = line(collection, config, tr);
    let mut args = tr.counted(line.count);
    args.set("name", collection.name.clone());
    args.set("n", line.count as i64);
    let none = |sentence: String| Removal { act: "none", sentence, count: line.count, into: Vec::new(), note: String::new() };
    if line.can_leave {
        return Removal { act: "leave", sentence: tr.text("book-leave-question", Some(&args)), count: line.count, into: Vec::new(), note: tr.text("book-leave-note", None) };
    }
    if !line.fixed.is_empty() {
        return none(line.fixed);
    }
    if !line.can_remove {
        return none(tr.text("book-not-yours-to-remove", None));
    }
    if line.count == 0 {
        return Removal { act: "empty", sentence: tr.text("book-remove-empty", Some(&args)), count: 0, into: Vec::new(), note: String::new() };
    }
    let into = vdir::collections(Kind::Contacts).into_iter().filter(|c| !c.read_only && c.dir != collection.dir && !State::load(&c.state_path()).refused).map(|c| (format!("{}/{}", c.account, c.id), c.label(config, tr))).collect();
    // The count and the copy are this device's: on a server, the book goes for everyone it is
    // shared with, and what was added there since the last sync keeps it (review F5).
    let note = if collection.account == vdir::LOCAL { tr.text("book-remove-copy-here", None) } else { tr.text("book-remove-copy", Some(&args)) };
    Removal { act: "cards", sentence: tr.text("book-remove-cards", Some(&args)), count: line.count, into, note }
}

/// What merging `from` into `into` would do, said before it runs (review F4):
/// how many contacts go where, what the target's server would not keep, and
/// who sees the target (`shared_with`, as its server last answered; none
/// when not asked yet).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergeQuestion {
    pub sentence: String,
    /// "“Google” would not keep: …", or "".
    pub losses: String,
    /// Who sees the target, or that it may be shared; "" for a book of this device.
    pub shared: String,
    /// What the merge does to who may reach the person (`reach_of_move`).
    pub reach: Vec<String>,
    /// Cards that cannot go there, in words, card by card; the merge waits while any.
    pub cannot: Vec<crate::contactbatch::Cannot>,
}

/// The question before a merge.
pub fn merge_question(from: &Collection, into: &Collection, config: &Config, tr: &Translator, shared_with: Option<&[String]>) -> MergeQuestion {
    let cards: Vec<crate::contacts::Contact> = from.items().iter().filter_map(|p| crate::contacts::read(p, from)).collect();
    let provider = crate::capabilities::provider_of(config.account(&into.account));
    let plan = crate::contactbatch::plan_move(&cards.iter().collect::<Vec<_>>(), into, provider, tr);
    let mut args = tr.counted(cards.len());
    args.set("n", cards.len() as i64);
    args.set("name", from.name.clone());
    args.set("into", into.name.clone());
    let losses = if plan.losses.is_empty() {
        String::new()
    } else {
        let mut args = crate::i18n::args();
        args.set("book", into.name.clone());
        args.set("list", plan.losses.join(", "));
        tr.text("batch-ask-losses", Some(&args))
    };
    let senders = crate::porch::Senders::load(config);
    let reach = reach_of_move(&cards.iter().collect::<Vec<_>>(), into, config, &senders, tr).said;
    MergeQuestion { sentence: tr.text("books-merge-ask", Some(&args)), losses, shared: shared_line(into, tr, shared_with), reach, cannot: plan.cannot }
}

/// Who sees what goes into `book`: the people its server named, that it may
/// be shared when not asked yet, its owner's when someone else's; "" for a
/// book of this device, or one shared with nobody.
pub fn shared_line(book: &Collection, tr: &Translator, shared_with: Option<&[String]>) -> String {
    let state = State::load(&book.state_path());
    let mut args = crate::i18n::args();
    args.set("book", book.name.clone());
    if book.account == vdir::LOCAL {
        return String::new();
    }
    if state.owned == Some(false) {
        args.set("owner", owner_in_id(&book.id).unwrap_or("").to_string());
        return tr.text("batch-shared-theirs", Some(&args));
    }
    match shared_with {
        Some([]) => String::new(),
        Some(people) => {
            args.set("who", people.join(", "));
            tr.text("batch-shared-with", Some(&args))
        }
        None => tr.text("batch-shared-maybe", Some(&args)),
    }
}

/// A book by its place, "account/id", among those shown.
pub fn find(place: &str) -> Option<Collection> {
    vdir::collections(Kind::Contacts).into_iter().find(|c| format!("{}/{}", c.account, c.id) == place)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each book's line: yours, someone else's (named by Nextcloud's own
    /// id), read only, made here; what each lets you do. Google's says it
    /// is fixed. Removing asks what becomes of the cards.
    #[test]
    fn each_book_says_whose_it_is_and_what_it_allows() {
        crate::config::stand_ins_only();
        let tr = Translator::new("en");
        let config: Config = toml::from_str("[[account]]\nid = \"lines-cloud\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"cloud.example.org\"\n").unwrap();
        let made = |id: &str, state: State, cards: usize| {
            let dir = vdir::prepare(Kind::Contacts, "lines-cloud", id, id, None).unwrap();
            state.save(&vdir::state_path("lines-cloud", Kind::Contacts, id)).unwrap();
            for n in 0..cards {
                std::fs::write(dir.join(format!("{n}.vcf")), format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{id}-{n}\r\nFN:N{n}\r\nEND:VCARD\r\n")).unwrap();
            }
        };
        made("pro", State { url: "https://cloud.example.org/pro/".into(), owned: Some(true), ..State::default() }, 2);
        made("contacts_shared_by_sam", State { url: "https://cloud.example.org/s/".into(), owned: Some(false), read_only: true, ..State::default() }, 1);
        made("unknown", State { url: "https://cloud.example.org/u/".into(), ..State::default() }, 0);
        let all = lines(&config, &tr);
        let get = |id: &str| all.iter().find(|l| l.place == format!("lines-cloud/{id}")).unwrap().clone();
        let pro = get("pro");
        assert_eq!((pro.whose.as_str(), pro.counted.as_str(), pro.kept.as_str(), pro.access.as_str()), ("Yours", "2 contacts", "example.org", "Can be written"));
        assert!(pro.can_rename && pro.can_remove && pro.can_share && !pro.can_leave && pro.fixed.is_empty());
        let sam = get("contacts_shared_by_sam");
        assert_eq!((sam.whose.as_str(), sam.access.as_str()), ("Shared with you by sam", "Read only"));
        assert!(!sam.can_rename && !sam.can_remove && !sam.can_share && sam.can_leave);
        let unknown = get("unknown");
        assert!(unknown.can_rename && !unknown.can_remove && !unknown.can_share, "whose it is unsaid: renamed, never removed from here");
        // Removing: with its cards, the books they can go to; empty, at once; someone else's, left.
        let pro_book = find("lines-cloud/pro").unwrap();
        let asked = removal(&pro_book, &config, &tr);
        assert_eq!((asked.act, asked.count), ("cards", 2));
        assert!(asked.into.iter().any(|(p, _)| p == "lines-cloud/unknown") && !asked.into.iter().any(|(p, _)| p.starts_with("lines-cloud/") && p.contains("shared_by")), "{:?}", asked.into);
        assert_eq!(removal(&find("lines-cloud/contacts_shared_by_sam").unwrap(), &config, &tr).act, "leave");
        // Made here: a home among the accounts, never Google's; a name needed.
        assert_eq!(homes(&config, &tr).iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["lines-cloud", "local"]);
        assert!(make(&config, "lines-cloud", "  ", &tr).is_err() && make(&config, "elsewhere", "Pro", &tr).is_err());
        let new = make(&config, "lines-cloud", "Professional", &tr).unwrap();
        let line = line(&new, &config, &tr);
        assert_eq!((line.whose.as_str(), line.waiting.is_empty()), ("Yours", false));
        assert!(State::load(&new.state_path()).pending, "made on its server by the next sync");
        let _ = std::fs::remove_dir_all(Kind::Contacts.root().join("lines-cloud"));
        let _ = std::fs::remove_dir_all(crate::config::state_dir().join("dav").join("lines-cloud"));
    }

    /// Review V1: a move, a group given, an import, said with what they do
    /// to who may reach the person across a book whose groups are held back,
    /// counted by the lists' own step; "stays as its groups say" only when
    /// nothing crosses.
    #[test]
    fn what_a_move_does_to_who_may_reach_you_is_said_first() {
        use crate::porch::{SenderList, Senders};
        crate::config::stand_ins_only();
        let tr = Translator::new("en");
        let account = "reach-cloud";
        let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(account));
        let made = |id: &str, writers: &[&str]| {
            vdir::prepare(Kind::Contacts, account, id, id, None).unwrap();
            State { url: format!("https://cloud.example.org/{id}/"), owned: Some(true), writers: writers.iter().map(|w| w.to_string()).collect(), writers_known: true, ..State::default() }.save(&vdir::state_path(account, Kind::Contacts, id)).unwrap();
            find(&format!("{account}/{id}")).unwrap()
        };
        let (mine, other, team) = (made("mine", &[]), made("other", &[]), made("team", &["sam"]));
        let card = |book: &Collection, uid: &str, group: &str| crate::contacts::Contact { uid: uid.into(), name: uid.into(), place: format!("{}/{}", book.account, book.id), categories: vec![group.into()], ..crate::contacts::Contact::default() };
        let senders = Senders { safe: SenderList::parse("category:Clients\ncontact:own-line"), ..Senders::default() };
        let config = Config::default();
        let (lou, noa, own) = (card(&mine, "lou", "Clients"), card(&mine, "noa", "Clients"), card(&mine, "own-line", "Clients"));
        let into = reach_of_move(&[&lou, &noa, &own], &team, &config, &senders, &tr);
        assert!(into.crosses && into.said == [tr.text("batch-reach-held", Some(&{ let mut a = tr.counted(2); a.set("n", 2i64); a.set("list", "Safe"); a }))], "{:?}", into.said);
        let planted = card(&team, "planted", "Clients");
        let out = reach_of_move(&[&planted], &mine, &config, &senders, &tr);
        assert!(out.crosses && out.said[0].contains("(Safe: Clients)"), "{:?}", out.said);
        let stays = reach_of_move(&[&lou], &other, &config, &senders, &tr);
        assert!(!stays.crosses && stays.said == [tr.text("batch-reach-stays", None)]);
        // A group given where the book's groups are held back; an import into that book.
        assert!(reach_of_groups(&[&planted], &["Clients".to_string()], &config, &senders, &tr).contains("Safe: Clients"));
        assert!(reach_of_groups(&[&lou], &["Clients".to_string()], &config, &senders, &tr).is_empty(), "its book's groups count: the group decides, as asked");
        assert!(reach_of_import(std::slice::from_ref(&planted), &team, &config, &senders, &tr).contains("“team”"));
        assert!(reach_of_import(std::slice::from_ref(&lou), &mine, &config, &senders, &tr).is_empty());
        let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(account));
    }

    /// Review L1: the count judges each address and number apart, as the
    /// lists do. A client whose personal address is blocked by itself still
    /// has the work address and the number let through by the group: the
    /// move into a book whose groups are held back is said to change that,
    /// and it does.
    #[test]
    fn the_count_judges_each_address_and_number_apart() {
        use crate::porch::{Book, SenderList, Senders};
        use crate::reach::Who;
        crate::config::stand_ins_only();
        let tr = Translator::new("en");
        let account = "apart-cloud";
        let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(account));
        let made = |id: &str, writers: &[&str]| {
            vdir::prepare(Kind::Contacts, account, id, id, None).unwrap();
            State { url: format!("https://cloud.example.org/{id}/"), owned: Some(true), writers: writers.iter().map(|w| w.to_string()).collect(), writers_known: true, ..State::default() }.save(&vdir::state_path(account, Kind::Contacts, id)).unwrap();
            find(&format!("{account}/{id}")).unwrap()
        };
        let (mine, team) = (made("mine", &[]), made("team", &["sam"]));
        std::fs::write(mine.dir.join("lou.vcf"), "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:apart-1\r\nFN:Lou Client\r\nEMAIL:lou.perso@example.org\r\nEMAIL:lou.travail@example.org\r\nTEL:+33465710011\r\nCATEGORIES:Clients\r\nEND:VCARD\r\n").unwrap();
        let config = Config::default();
        let senders = || {
            let cards: Vec<crate::contacts::Contact> = crate::contacts::all().into_iter().filter(|c| c.place.starts_with("apart-cloud/")).collect();
            Senders { safe: SenderList::parse("category:Clients"), blocked: SenderList::parse("lou.perso@example.org"), contacts: std::sync::Arc::new(Book::trusting(&cards, None, &groups_held_back(&config))), ..Senders::default() }
        };
        let judged = |s: &Senders| (s.judge("lou.perso@example.org").who, s.judge("lou.travail@example.org").who, s.judge_number("+33465710011").who);
        assert_eq!(judged(&senders()), (Who::Blocked, Who::Safe, Who::Safe));
        let cards: Vec<crate::contacts::Contact> = mine.items().iter().filter_map(|p| crate::contacts::read(p, &mine)).collect();
        let said = reach_of_move(&cards.iter().collect::<Vec<_>>(), &team, &config, &senders(), &tr);
        assert!(said.crosses && said.said[0].starts_with("One of these is on your Safe list through a group"), "{:?}", said.said);
        vdir::move_item(std::path::Path::new(&cards[0].key), &crate::contacts::new_path(&team)).unwrap();
        assert_eq!(judged(&senders()), (Who::Blocked, Who::Neutral, Who::Neutral), "what the question said");
        // Every address of it on one list: its own choice decides, the groups nothing.
        let every = Senders { safe: SenderList::parse("category:Clients"), blocked: SenderList::parse("lou.perso@example.org\nlou.travail@example.org"), ..Senders::default() };
        assert!(every.standing_by_groups(&cards[0], &cards[0].categories).is_none());
        let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(account));
    }

    /// "Its groups decide who may reach me" (the owner's answer of 18
    /// November 2026): on for a book only you write; off, its line saying
    /// why, for one someone else can write (shared to write, or someone
    /// else's); switched on, kept in the settings with its writers; off again
    /// when they change.
    #[test]
    fn a_book_s_groups_count_once_you_trust_who_writes_it() {
        crate::config::stand_ins_only();
        let tr = Translator::new("en");
        let account = "trust-cloud";
        let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(account));
        let made = |id: &str, state: State| {
            vdir::prepare(Kind::Contacts, account, id, id, None).unwrap();
            state.save(&vdir::state_path(account, Kind::Contacts, id)).unwrap();
            find(&format!("{account}/{id}")).unwrap()
        };
        let mine = made("mine", State { url: "https://cloud.example.org/mine/".into(), owned: Some(true), writers_known: true, ..State::default() });
        let team = made("team", State { url: "https://cloud.example.org/team/".into(), owned: Some(true), writers: vec!["sam".into(), "group:office".into()], writers_known: true, ..State::default() });
        let theirs = made("contacts_shared_by_lou", State { url: "https://cloud.example.org/lou/".into(), owned: Some(false), ..State::default() });
        let dir = std::env::temp_dir().join(format!("sioul-book-trust-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\n[contacts]\nregion = \"FR\"\n").unwrap();
        let config = || crate::config::Config::load(&path).unwrap();
        assert!(groups_count(&mine, &config()).count && !groups_count(&team, &config()).count && !groups_count(&theirs, &config()).count);
        assert_eq!(writers(&theirs, &State::load(&theirs.state_path())), ["lou"]);
        let team_line = line(&team, &config(), &tr);
        assert!(!team_line.groups && team_line.groups_note.starts_with("office, sam can write"), "{}", team_line.groups_note);
        assert!(line(&mine, &config(), &tr).groups_note.is_empty());
        assert_eq!(groups_held_back(&config()).into_iter().filter(|p| p.starts_with(account)).collect::<Vec<_>>(), ["trust-cloud/contacts_shared_by_lou", "trust-cloud/team"]);
        // Switched on for these writers: kept in the settings, every device reading it; its note gone.
        set_groups_count(&path, &team, true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# mine\n") && text.contains("region = \"FR\"") && text.contains("[contacts.books.\"trust-cloud/team\"]") && text.contains("groups = true"), "{text}");
        assert!(groups_count(&team, &config()).count && line(&team, &config(), &tr).groups_note.is_empty());
        // Its writers change: what was said no longer holds.
        State { url: "https://cloud.example.org/team/".into(), owned: Some(true), writers: vec!["sam".into()], writers_known: true, ..State::default() }.save(&team.state_path()).unwrap();
        assert!(!groups_count(&team, &config()).count && !line(&team, &config(), &tr).groups_note.is_empty());
        // Told once per book and writers on this device (review V3): someone else's book, the team's for sam.
        let to_tell = |c: &Config| groups_to_tell(c).into_iter().filter(|(b, _)| b.account == account).map(|(b, w)| (b.id, w)).collect::<Vec<_>>();
        assert_eq!(to_tell(&config()), [("contacts_shared_by_lou".to_string(), vec!["lou".to_string()]), ("team".to_string(), vec!["sam".to_string()])]);
        mark_groups_told(&team, &["sam".to_string()]).unwrap();
        assert_eq!(to_tell(&config()).len(), 1, "the team's told");
        State { url: "https://cloud.example.org/team/".into(), owned: Some(true), writers: vec!["office".into(), "sam".into()], writers_known: true, ..State::default() }.save(&team.state_path()).unwrap();
        assert_eq!(to_tell(&config()).len(), 2, "its writers changed again: told again");
        // A server that says neither whose a book is nor who can write it (point 8): its groups count, said on its line.
        let quiet = made("quiet", State { url: "https://cloud.example.org/quiet/".into(), ..State::default() });
        assert!(groups_count(&quiet, &config()).count);
        assert_eq!(line(&quiet, &config(), &tr).groups_note, tr.text("book-writers-unsaid", None));
        // Turned off for a book only you write: said, kept.
        set_groups_count(&path, &mine, false).unwrap();
        assert!(!groups_count(&mine, &config()).count && line(&mine, &config(), &tr).groups_note.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(account));
    }
}

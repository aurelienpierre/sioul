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
//! (porch.rs): nothing here touches a list, an area or a notification.
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
        waiting: if state.pending && !local { tr.text("book-waiting", None) } else { String::new() },
        can_rename: writable && !google && (yours || state.owned.is_none()),
        can_remove: writable && !google && yours,
        can_share: writable && !google && !local && yours,
        can_leave: !local && !google && state.owned == Some(false) && !state.url.is_empty(),
        fixed: if google { tr.text("book-google-fixed", None) } else { String::new() },
    }
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
    let into = vdir::collections(Kind::Contacts).into_iter().filter(|c| !c.read_only && c.dir != collection.dir).map(|c| (format!("{}/{}", c.account, c.id), c.label(config, tr))).collect();
    Removal { act: "cards", sentence: tr.text("book-remove-cards", Some(&args)), count: line.count, into, note: tr.text("book-remove-copy", None) }
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
        assert!(asked.into.iter().any(|(p, _)| p == "lines-cloud/unknown") && !asked.into.iter().any(|(p, _)| p.contains("shared_by")), "{:?}", asked.into);
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
}

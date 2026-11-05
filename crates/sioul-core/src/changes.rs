// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What changed, said by the one place that writes each kind of object
//! (docs/client.md, "Every view shows a change at once"): the persons'
//! store when it is written (`Persons::save`), a calendar's or an address
//! book's item at the vdir's one door (`vdir::counted`), and so on for each
//! kind. The window listens (`listen`, once, as it starts) and tells every
//! view that shows that kind. A program without a window (the command
//! line, the agents' tools) has nobody listening: it leaves a stamp per
//! kind instead (`stamp_for_the_window`, `$XDG_STATE_HOME/sioul/changes/`),
//! which the window watches; what another device brought, the sharing says.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// A kind of object whose views read it again when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// The persons of `persons.toml` (docs/persons.md).
    Persons,
    /// What each agenda is (`[agenda.calendars]`: its role, its person, its
    /// area; its stand-ins, rhythms, sets and exceptions), your availabilities
    /// (`[[agenda.availabilities]]`, and the single share of before,
    /// `[agenda.availability]`), and the calendars themselves (made, renamed,
    /// recoloured, removed, left).
    Agendas,
    /// A calendar's item: an event, a task, a time block.
    Calendars,
    /// An address book's item: a contact card; and the address books.
    Contacts,
    /// The wallet's papers (`sioul-papers.toml`).
    Papers,
    /// Paper letters (`letters/letters.toml`).
    Letters,
    /// Money: budgets and their lines, bank accounts and movements, contracts.
    Money,
    /// A note of the notes folder.
    Notes,
    /// Ties between things (`links.toml`; a note's or an item's own links).
    Links,
    /// Your sites (`[[site]]`).
    Sites,
    /// Projects (`sioul-projects.toml`): their routes, time and invoices too.
    Projects,
    /// Mail and calendar accounts (`[[account]]`).
    Accounts,
    /// Mail's topics: the list (`[[topic]]`) and your word on messages and
    /// senders (`topics::labels`' logs).
    Topics,
}

impl Kind {
    pub const ALL: [Kind; 13] = [Kind::Persons, Kind::Agendas, Kind::Calendars, Kind::Contacts, Kind::Papers, Kind::Letters, Kind::Money, Kind::Notes, Kind::Links, Kind::Sites, Kind::Projects, Kind::Accounts, Kind::Topics];

    /// Its place among the kinds, for a set of them as bits.
    pub fn bit(self) -> u32 {
        1 << (self as u32)
    }

    /// Its name, its stamp's file name.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Persons => "persons",
            Kind::Agendas => "agendas",
            Kind::Calendars => "calendars",
            Kind::Contacts => "contacts",
            Kind::Papers => "papers",
            Kind::Letters => "letters",
            Kind::Money => "money",
            Kind::Notes => "notes",
            Kind::Links => "links",
            Kind::Sites => "sites",
            Kind::Projects => "projects",
            Kind::Accounts => "accounts",
            Kind::Topics => "topics",
        }
    }
}

type Listener = Box<dyn Fn(Kind) + Send + Sync>;

static LISTENER: OnceLock<Listener> = OnceLock::new();

/// This program leaves stamps for the window (`stamp_for_the_window`).
static STAMPS: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// What this thread said, while a test asks (`heard`).
    static HEARD: RefCell<Option<Vec<Kind>>> = const { RefCell::new(None) };
}

/// The window's ear, set once as it starts; whether it was the first.
pub fn listen(listener: impl Fn(Kind) + Send + Sync + 'static) -> bool {
    LISTENER.set(Box::new(listener)).is_ok()
}

/// `kind` changed: said to the window, when one listens; else, in a
/// program that leaves stamps, stamped. Called by the one place that writes
/// that kind, after it wrote.
pub fn said(kind: Kind) {
    HEARD.with(|heard| {
        if let Some(heard) = heard.borrow_mut().as_mut() {
            heard.push(kind);
        }
    });
    if let Some(listener) = LISTENER.get() {
        listener(kind);
    } else if STAMPS.load(Ordering::Relaxed) {
        // Nothing is lost when it cannot be written: the window reads it all again at its next look.
        let _ = stamp_in(&stamps_dir(), kind);
    }
}

/// A program without a window (the command line, `sioul mcp`) says what it
/// changes by a stamp per kind, which the window watches: called once, as it starts.
pub fn stamp_for_the_window() {
    STAMPS.store(true, Ordering::Relaxed);
}

/// Where the stamps are: `$XDG_STATE_HOME/sioul/changes/`, this device's own.
pub fn stamps_dir() -> PathBuf {
    crate::config::state_dir().join("changes")
}

/// `kind` stamped in `dir`: its file written anew, its time now.
pub fn stamp_in(dir: &Path, kind: Kind) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(kind.id()), jiff::Timestamp::now().as_millisecond().to_string())
}

/// Each kind's stamp in `dir` as it is now: its file's time and size; a kind
/// never stamped is left out.
pub fn stamped_in(dir: &Path) -> BTreeMap<Kind, (std::time::SystemTime, u64)> {
    Kind::ALL
        .iter()
        .filter_map(|kind| {
            let meta = std::fs::metadata(dir.join(kind.id())).ok()?;
            Some((*kind, (meta.modified().ok()?, meta.len())))
        })
        .collect()
}

/// What `run` said on this thread, in order (tests: a writer says its kind).
pub fn heard<R>(run: impl FnOnce() -> R) -> (R, Vec<Kind>) {
    let before = HEARD.with(|heard| heard.borrow_mut().replace(Vec::new()));
    let out = run();
    let said = HEARD.with(|heard| std::mem::replace(&mut *heard.borrow_mut(), before)).unwrap_or_default();
    (out, said)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer's word is heard on its own thread alone, nested asks each
    /// hearing their own; the kinds are distinct bits.
    #[test]
    fn what_is_said_is_heard_where_it_was_said() {
        let ((inner, ()), outer) = heard(|| {
            said(Kind::Persons);
            let inner = heard(|| said(Kind::Agendas));
            std::thread::spawn(|| said(Kind::Persons)).join().unwrap();
            (inner.1, ())
        });
        assert_eq!(outer, [Kind::Persons]);
        assert_eq!(inner, [Kind::Agendas]);
        let bits: Vec<u32> = Kind::ALL.iter().map(|k| k.bit()).collect();
        assert!(bits.iter().enumerate().all(|(i, b)| bits.iter().skip(i + 1).all(|c| b & c == 0)));
        let ids: std::collections::BTreeSet<&str> = Kind::ALL.iter().map(|k| k.id()).collect();
        assert_eq!(ids.len(), Kind::ALL.len(), "each kind its own stamp");
    }

    /// A program without a window stamps each kind it changes; the window
    /// sees a stamp written again, and nothing for a kind never stamped.
    #[test]
    fn a_stamp_says_a_kind_changed_elsewhere() {
        let dir = std::env::temp_dir().join(format!("sioul-stamps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(stamped_in(&dir).is_empty());
        stamp_in(&dir, Kind::Calendars).unwrap();
        let first = stamped_in(&dir);
        assert_eq!(first.keys().copied().collect::<Vec<_>>(), [Kind::Calendars]);
        std::thread::sleep(std::time::Duration::from_millis(20));
        stamp_in(&dir, Kind::Calendars).unwrap();
        stamp_in(&dir, Kind::Notes).unwrap();
        let again = stamped_in(&dir);
        assert_ne!(again[&Kind::Calendars], first[&Kind::Calendars], "written again: another time or size");
        assert!(again.contains_key(&Kind::Notes));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Each kind's one writer says it, once written: a calendar made and its
    /// item written and taken out, an address book's card, a paper, a tie, a
    /// note, a budget, an account; a configuration written with the same
    /// accounts says none.
    #[test]
    fn every_writer_says_its_kind() {
        crate::config::stand_ins_only();
        let root = std::env::temp_dir().join(format!("sioul-changes-writers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let said = |run: &dyn Fn()| heard(run).1;
        let calendar = crate::vdir::create(crate::vdir::Kind::Calendars, "changes-test", "Said", None, &["VEVENT", "VTODO"]).unwrap();
        let item = calendar.dir.join("said.ics");
        assert_eq!(said(&|| crate::vdir::write_item(&item, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:said\r\nSUMMARY:Said\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap()), [Kind::Calendars]);
        assert_eq!(said(&|| crate::vdir::remove_item(&item).unwrap()), [Kind::Calendars]);
        assert_eq!(said(&|| crate::vdir::rename_collection(&calendar, "Said again").unwrap()), [Kind::Agendas]);
        let book = crate::vdir::create(crate::vdir::Kind::Contacts, "changes-test", "Book", None, &[]).unwrap();
        assert_eq!(said(&|| crate::vdir::write_item(&book.dir.join("c.vcf"), "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:c\r\nFN:Camille Exemple\r\nEND:VCARD\r\n").unwrap()), [Kind::Contacts]);
        assert_eq!(said(&|| crate::papers::Wallet::change(&root, |_| ()).unwrap()), [Kind::Papers]);
        let links = root.join("links.toml");
        assert_eq!(said(&|| { crate::links::LocalLinks::add_all(&links, &[crate::links::Edge { from: "sioul:task/a".into(), to: "sioul:note/b.md".into(), how: "link".into() }]).unwrap(); }), [Kind::Links]);
        assert_eq!(said(&|| { crate::notes::write(&root, "said.md", "# Said\n").unwrap(); }), [Kind::Notes]);
        let ledger = root.join(crate::budget::LEDGER);
        assert_eq!(said(&|| { crate::budget::save_budget(&ledger, "", &crate::budget::BudgetEdit { title: "Food".into(), period: "month".into(), ..Default::default() }).unwrap(); }), [Kind::Money]);
        let config = root.join("config.toml");
        std::fs::write(&config, "language = \"en\"\n").unwrap();
        let write = |text: &str| crate::config::write_document(&config, &text.parse().unwrap()).unwrap();
        assert_eq!(said(&|| write("language = \"en\"\n\n[[account]]\nid = \"home\"\n")), [Kind::Accounts]);
        assert_eq!(said(&|| write("language = \"fr\"\n\n[[account]]\nid = \"home\"\n")), [], "the same accounts: nothing said of them");
        for c in crate::vdir::every_collection(crate::vdir::Kind::Calendars).into_iter().chain(crate::vdir::every_collection(crate::vdir::Kind::Contacts)).filter(|c| c.account == "changes-test") {
            let _ = std::fs::remove_dir_all(&c.dir);
            let _ = std::fs::remove_file(c.state_path());
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}

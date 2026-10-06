// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Duplicates in the contacts, behind the Contacts page's "Duplicates": what
//! the core finds (`sioul_core::duplicates`) said in words, and what the
//! person chooses: these cards cleaned, this pair merged or kept apart, what
//! was done undone. Each change goes to the address books' folders at once
//! and their accounts' sync sends it; nothing changes without a click.

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use crate::{mail, pim};
use cxx_qt_lib::QString;
use serde::Serialize;
use sioul_core::contacts::{self, Contact};
use sioul_core::duplicates::{self, Cleaning, Done, NotTheSame, Pair};
use sioul_core::i18n::Translator;
use sioul_core::phones::{self, Region};
use sioul_core::vdir::Kind;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The country numbers written without one are read as: the settings', else the system's.
pub(crate) fn region() -> Option<&'static Region> {
    phones::chosen(load_config().contacts.region.as_deref(), &tr().text("qt-locale", None))
}

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// The cards looked at: every one, those waiting for their deletion aside.
fn everyone(shared: &Shared) -> Vec<Contact> {
    let (removed, _) = mail::hidden_pim(shared);
    contacts::all().into_iter().filter(|c| !removed.contains(Path::new(&c.key))).collect()
}

/// Looks may finish out of order: only the newest is shown.
static LOOKS: AtomicU64 = AtomicU64::new(0);

/// Looks for duplicates on a thread; the page reads them in `duplicates`.
pub(crate) fn find(qt: &QtThread, shared: &Arc<Shared>) {
    let look = LOOKS.fetch_add(1, Ordering::Relaxed) + 1;
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let everyone = everyone(&shared);
        let region = region();
        let cleanings = duplicates::cleanings(&everyone, region);
        let pairs = duplicates::pairs(&everyone, region, &NotTheSame::load(&NotTheSame::default_path()));
        let done = duplicates::done(&duplicates::undo_dir(), now());
        let text = json(&view(&everyone, &cleanings, &pairs, &done, region, tr()));
        let _ = qt.queue(move |mut sioul| {
            if LOOKS.load(Ordering::Relaxed) == look {
                sioul.as_mut().set_duplicates(QString::from(&text));
            }
        });
    });
}

/// What the Duplicates view shows, in words.
#[derive(Debug, Serialize)]
struct DuplicatesView {
    /// "Country of numbers written without one: France…".
    region: String,
    /// "Two cards hold a number or an address twice…", or that none does.
    within: String,
    cleanings: Vec<CleaningView>,
    /// "Three pairs of cards may be one person…", or that none may.
    between: String,
    pairs: Vec<PairView>,
    /// What was done lately, the newest first, to undo it.
    done: Vec<DoneView>,
    /// "Each is kept thirty days…".
    kept: String,
}

#[derive(Debug, Serialize)]
struct CleaningView {
    key: String,
    name: String,
    /// "06 08 12 34 56, the same as +33 6 08 12 34 56", a line each.
    lines: Vec<String>,
}

/// One of a pair's cards, as the view compares them.
#[derive(Debug, Default, Serialize)]
struct Side {
    key: String,
    name: String,
    photo: String,
    /// "In Contacts".
    book: String,
    /// Role and organisation: "Accountant · Exemple SARL".
    work: String,
    phones: Vec<String>,
    emails: Vec<String>,
    /// Each postal address on one line.
    addresses: Vec<String>,
    categories: Vec<String>,
    birthday: String,
    notes: String,
}

#[derive(Debug, Serialize)]
struct PairView {
    first: Side,
    second: Side,
    /// "Both have the same name, the number 06 08 12 34 56."
    share: String,
    /// The side whose name is offered first: the fuller name.
    lead: &'static str,
}

#[derive(Debug, Serialize)]
struct DoneView {
    id: String,
    /// "Merged: Jean Dupont, J. Dupont".
    said: String,
    /// When, as the mail says dates.
    when: String,
}

fn side(contact: Option<&Contact>, tr: &Translator) -> Side {
    let Some(c) = contact else { return Side::default() };
    let mut book = sioul_core::i18n::args();
    book.set("book", c.book.clone());
    Side {
        key: c.key.clone(),
        name: c.name.clone(),
        photo: c.photo.clone(),
        book: tr.text("dup-in-book", Some(&book)),
        work: [c.title.as_str(), c.org.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" · "),
        phones: c.phones.iter().map(|p| p.value.clone()).collect(),
        emails: c.emails.iter().map(|e| e.value.clone()).collect(),
        addresses: c.addresses.iter().map(|a| a.value.replace('\n', ", ")).collect(),
        categories: c.categories.clone(),
        birthday: c.birthday.clone(),
        notes: c.notes.clone(),
    }
}

/// The view, from what the core found.
fn view(everyone: &[Contact], cleanings: &[Cleaning], pairs: &[Pair], done: &[Done], region: Option<&Region>, tr: &Translator) -> DuplicatesView {
    let text = |id: &str, pairs: &[(&str, String)]| {
        let mut args = sioul_core::i18n::args();
        for (key, value) in pairs {
            args.set(key.to_string(), value.clone());
        }
        tr.text(id, Some(&args))
    };
    let counted = |id: &str, n: usize| tr.text(id, Some(&tr.counted(n)));
    let region = match region {
        Some(r) => text("dup-region", &[("country", tr.text(&format!("country-{}", r.code.to_ascii_lowercase()), None))]),
        None => tr.text("dup-region-none", None),
    };
    let by_key: BTreeMap<&str, &Contact> = everyone.iter().map(|c| (c.key.as_str(), c)).collect();
    let pairs: Vec<PairView> = pairs
        .iter()
        .map(|p| {
            let mut what: Vec<String> = Vec::new();
            if p.same_name {
                what.push(tr.text("dup-share-name", None));
            }
            what.extend(p.phones.iter().map(|v| text("dup-share-phone", &[("value", v.clone())])));
            what.extend(p.emails.iter().map(|v| text("dup-share-email", &[("value", v.clone())])));
            let (first, second) = (side(by_key.get(p.first.as_str()).copied(), tr), side(by_key.get(p.second.as_str()).copied(), tr));
            // The fuller name first: "Jean Dupont" before "J. Dupont".
            let fuller = |name: &str| (name.split_whitespace().filter(|w| w.trim_end_matches('.').chars().count() > 1).count(), name.chars().count());
            let lead = if fuller(&second.name) > fuller(&first.name) { "second" } else { "first" };
            PairView { first, second, share: text("dup-share", &[("what", what.join(", "))]), lead }
        })
        .collect();
    DuplicatesView {
        region,
        within: if cleanings.is_empty() { tr.text("dup-within-none", None) } else { counted("dup-within-some", cleanings.len()) },
        cleanings: cleanings
            .iter()
            .map(|c| CleaningView { key: c.key.clone(), name: c.name.clone(), lines: c.removed.iter().map(|r| text("dup-removed", &[("value", r.value.clone()), ("kept", r.kept.clone())])).collect() })
            .collect(),
        between: if pairs.is_empty() { tr.text("dup-pairs-none", None) } else { counted("dup-pairs-some", pairs.len()) },
        pairs,
        done: done
            .iter()
            .map(|d| {
                let names = d.names.join(", ");
                let said = if d.kind == "merge" {
                    text("dup-done-merge", &[("names", names)])
                } else {
                    let mut args = tr.counted(d.changes.len());
                    args.set("names", names);
                    tr.text("dup-done-clean", Some(&args))
                };
                DoneView { id: d.id.clone(), said, when: sioul_core::view::date(tr, Some(d.at)) }
            })
            .collect(),
        kept: tr.text("dup-done-help", None),
    }
}

/// A card Sioul may change: in its address books, not read-only.
fn writable(key: &str) -> Option<PathBuf> {
    pim::ours(key).filter(|path| pim::collection_of(path).is_some_and(|c| !c.read_only))
}

/// After a change: each account concerned syncs now, the pages and the duplicates again.
fn changed(qt: &QtThread, shared: &Arc<Shared>, paths: &[PathBuf]) {
    let accounts: BTreeSet<String> = paths.iter().filter_map(|p| pim::account_of(p)).collect();
    for account in accounts {
        pim::nudge(shared, &account);
    }
    pim::show_pim(qt, shared);
    crate::work::show_work(qt, shared);
    find(qt, shared);
}

/// Takes the duplicates off these cards (a JSON list of their files), one
/// write each; returns what went wrong, else "".
pub(crate) fn clean(qt: &QtThread, shared: &Arc<Shared>, keys: &str) -> String {
    let keys: Vec<String> = serde_json::from_str(keys).unwrap_or_default();
    let paths: Vec<PathBuf> = keys.iter().filter_map(|k| writable(k)).collect();
    match duplicates::clean_files(&paths, region(), &duplicates::undo_dir(), now()) {
        Ok(Some(done)) => {
            changed(qt, shared, &done.changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>());
            tell(qt, shared, say("dup-cleaned", &[("n", done.changes.len().to_string()), ("count", tr().count(done.changes.len(), false, false)), ("countf", tr().count(done.changes.len(), false, true))]));
            String::new()
        }
        Ok(None) => {
            find(qt, shared);
            tr().text("dup-changed", None)
        }
        Err(e) => e,
    }
}

/// Merges two cards, `lead` keeping its name; returns {"key"} (the card kept) or {"error"}.
pub(crate) fn merge(qt: &QtThread, shared: &Arc<Shared>, lead: &str, other: &str) -> String {
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let (Some(lead), Some(other)) = (writable(lead), writable(other)) else { return fail(tr().text("contact-read-only", None)) };
    match duplicates::merge_files(&lead, &other, region(), &duplicates::undo_dir(), now()) {
        Ok(done) => {
            changed(qt, shared, &[lead.clone(), other]);
            tell(qt, shared, say("dup-merged", &[("names", done.names.join(", "))]));
            serde_json::json!({ "key": lead.display().to_string() }).to_string()
        }
        Err(e) => fail(e),
    }
}

/// Two cards are not one person: never offered again here; returns what went wrong, else "".
pub(crate) fn not_same(qt: &QtThread, shared: &Arc<Shared>, first: &str, second: &str) -> String {
    let everyone = everyone(shared);
    let (Some(a), Some(b)) = (everyone.iter().find(|c| c.key == first), everyone.iter().find(|c| c.key == second)) else { return tr().text("contact-gone", None) };
    let path = NotTheSame::default_path();
    let mut apart = NotTheSame::load(&path);
    apart.remember(a, b);
    if let Err(e) = apart.save(&path) {
        return e;
    }
    tell(qt, shared, say("dup-kept-apart", &[("names", format!("{}, {}", a.name, b.name))]));
    find(qt, shared);
    String::new()
}

/// Puts the cards back as they were before what was done (`id`); returns what went wrong, else "".
pub(crate) fn undo(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let dir = duplicates::undo_dir();
    match duplicates::undo(&dir, id, &Kind::Contacts.root()) {
        Ok((touched, aside)) => {
            changed(qt, shared, &touched);
            let line = if aside > 0 { say("dup-undone-aside", &[("folder", dir.display().to_string())]) } else { tr().text("undo-done", None) };
            tell(qt, shared, line);
            String::new()
        }
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(key: &str, name: &str, phones: &[&str]) -> Contact {
        Contact {
            key: key.into(),
            uid: key.into(),
            name: name.into(),
            book: "Contacts".into(),
            phones: phones.iter().map(|p| contacts::Labeled { label: String::new(), value: p.to_string() }).collect(),
            ..Contact::default()
        }
    }

    #[test]
    fn the_view_says_it_in_words() {
        let tr = Translator::new("en");
        let everyone = [contact("/a.vcf", "J. Dupont", &["06 08 12 34 56"]), contact("/b.vcf", "Jean Dupont", &["+33 6 08 12 34 56"])];
        let france = phones::region_named("FR");
        let pairs = duplicates::pairs(&everyone, france, &NotTheSame::default());
        let cleanings = vec![Cleaning { key: "/a.vcf".into(), name: "J. Dupont".into(), removed: vec![duplicates::Removed { kind: "phone".into(), value: "06 08 12 34 56".into(), kept: "+33 6 08 12 34 56".into() }] }];
        let done = vec![Done { id: "1-merge".into(), at: 1_791_000_000, kind: "merge".into(), names: vec!["Jean Dupont".into(), "J. Dupont".into()], changes: Vec::new() }];
        let shown = view(&everyone, &cleanings, &pairs, &done, france, &tr);
        assert_eq!(shown.region, "Country of numbers written without one: France. The settings (⚙) change it.");
        assert_eq!(shown.within, "One card holds a number or an address twice. Ticked, it keeps one of each:");
        assert_eq!(shown.cleanings[0].lines, ["06 08 12 34 56, the same as +33 6 08 12 34 56"]);
        assert_eq!(shown.between, "One pair of cards may be one person.");
        assert_eq!(shown.pairs[0].share, "Both have the number 06 08 12 34 56.");
        // The fuller name is offered first.
        assert_eq!((shown.pairs[0].first.name.as_str(), shown.pairs[0].lead), ("J. Dupont", "second"));
        assert_eq!((shown.pairs[0].first.book.as_str(), shown.done[0].said.as_str()), ("In Contacts", "Merged: Jean Dupont, J. Dupont"));
        // In French, in words, and without a country.
        let fr = Translator::new("fr");
        let many: Vec<Cleaning> = (0..3).map(|n| Cleaning { key: format!("/{n}.vcf"), name: String::new(), removed: Vec::new() }).collect();
        let shown = view(&[], &many, &[], &[], None, &fr);
        assert!(shown.within.starts_with("Trois fiches contiennent"), "{}", shown.within);
        assert!(shown.region.starts_with("Les numéros écrits sans leur pays"), "{}", shown.region);
        assert_eq!(shown.between, "Aucune paire de fiches ne semble être une seule personne.");
    }
}

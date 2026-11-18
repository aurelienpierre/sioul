// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Many contacts at once (wave D, 18 November 2026): what the Contacts page's
//! selection does to its cards, and what an import or a merge of two address
//! books writes. Each is one batch:
//!
//! 1. **Planned** first, nothing written: the cards it will change (`Plan::steps`),
//!    those it cannot change and why, card by card (`Plan::cannot`), the
//!    count said before it runs.
//! 2. **Kept** before anything changes: the whole plan, each card as it was,
//!    in one record (`$XDG_STATE_HOME/sioul/contacts-batches/<id>.json`, kept
//!    thirty days).
//! 3. **Run** one card after the other through the doors of every writer
//!    (`vdir::write_item`, `remove_item`, `move_item`): a card of a book you
//!    only read is refused there too. Its progress told; it can be stopped
//!    between two cards; what a door refused named, card by card.
//! 4. **Undone** as a whole (`undo`): each step done is taken back, a step
//!    never done (stopped, refused) left alone, a card changed since set
//!    aside beside the record first. A move goes back by a move: the card
//!    keeps its UID either way.
//!
//! A move keeps each card's text and UID (`vdir::move_item`): everything that
//! points at a card (the sender lists' `contact:<UID>`, tasks, ties, persons)
//! still does. Who may reach you follows its groups, never its book.

use crate::contacts::{self, Contact, TitleMove};
use crate::i18n::Translator;
use crate::vdir::{self, Collection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// One card's change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "lowercase")]
pub enum Step {
    /// Written as `after`: made (`before` none) or changed.
    Write { path: PathBuf, before: Option<String>, after: String },
    /// Taken out; `before` what it held.
    Remove { path: PathBuf, before: String },
    /// Moved to another address book, its text and UID as they are.
    Move { from: PathBuf, to: PathBuf },
}

impl Step {
    /// The file it acts on first: what a refusal is said about.
    pub fn path(&self) -> &Path {
        match self {
            Step::Write { path, .. } | Step::Remove { path, .. } => path,
            Step::Move { from, .. } => from,
        }
    }
}

/// A card the batch leaves as it is, and why, in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cannot {
    pub name: String,
    pub why: String,
}

/// What a batch will do, before it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Plan {
    /// Each card's name and its step.
    pub steps: Vec<(String, Step)>,
    pub cannot: Vec<Cannot>,
    /// For the title's move: each card's name before and after.
    pub previews: Vec<(String, String)>,
    /// What the book a move goes to would not keep, in words, each once.
    pub losses: Vec<String>,
}

/// What a batch did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// Its record, to undo it.
    pub id: String,
    pub done: usize,
    /// What a door refused, card by card.
    pub failed: Vec<Cannot>,
    /// Stopped before its end: the cards after it were left as they were.
    pub stopped: bool,
}

/// What the Undo of a batch did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Undone {
    /// The cards put back.
    pub back: usize,
    /// Versions changed since, set aside beside the record before it went.
    pub aside: usize,
    pub failed: Vec<Cannot>,
}

/// A batch as kept to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    #[serde(skip)]
    pub id: String,
    /// When, in seconds since 1970.
    pub at: i64,
    /// "move", "groups", "title", "delete", "merge", "import".
    pub kind: String,
    pub steps: Vec<(String, Step)>,
    /// A book a merge emptied and deleted, "account/id": shown again by its Undo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_book: Option<String>,
}

/// How long a batch can be undone: thirty days, as Duplicates' records.
pub const KEPT_DAYS: i64 = 30;

/// `$XDG_STATE_HOME/sioul/contacts-batches`.
pub fn records_dir() -> PathBuf {
    crate::config::state_dir().join("contacts-batches")
}

fn is_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

// ---------------------------------------------------------------- plans

/// Each card as its file holds it now; one gone said as such.
fn read(contact: &Contact, tr: &Translator) -> Result<String, Cannot> {
    std::fs::read_to_string(&contact.key).map_err(|_| Cannot { name: contact.name.clone(), why: tr.text("batch-why-gone", None) })
}

fn cannot(contact: &Contact, why: String) -> Cannot {
    Cannot { name: contact.name.clone(), why }
}

/// Moved to `target`: each card's text and UID as they are, under a new file
/// name there. Left: one of a book you only read, one in that book already,
/// one whose UID the book holds already (its server would refuse it, RFC 6352
/// §6.3.2.1). `losses`, what the target's server would not keep (Google's
/// vCard 3.0), said before it runs.
pub fn plan_move(contacts: &[&Contact], target: &Collection, provider: crate::capabilities::Provider, tr: &Translator) -> Plan {
    let mut plan = Plan::default();
    if target.read_only {
        plan.cannot = contacts.iter().map(|c| cannot(c, tr.text("batch-why-target-read-only", None))).collect();
        return plan;
    }
    let held: Vec<String> = target.items().iter().filter_map(|p| std::fs::read_to_string(p).ok()).filter_map(|t| uid_of(&t)).collect();
    let mut taken: Vec<PathBuf> = Vec::new();
    for contact in contacts {
        if contact.read_only {
            plan.cannot.push(cannot(contact, tr.text("batch-why-read-only", None)));
            continue;
        }
        if Path::new(&contact.key).parent() == Some(target.dir.as_path()) {
            plan.cannot.push(cannot(contact, tr.text("batch-why-there-already", None)));
            continue;
        }
        let text = match read(contact, tr) {
            Ok(text) => text,
            Err(c) => {
                plan.cannot.push(c);
                continue;
            }
        };
        if uid_of(&text).is_some_and(|uid| held.contains(&uid)) {
            plan.cannot.push(cannot(contact, tr.text("batch-why-same-uid", None)));
            continue;
        }
        for loss in crate::capabilities::contact_losses(&text, provider) {
            let said = tr.text(&format!("loss-{loss}"), None);
            if !plan.losses.contains(&said) {
                plan.losses.push(said);
            }
        }
        let mut to = contacts::new_path(target);
        while taken.contains(&to) {
            to = contacts::new_path(target);
        }
        taken.push(to.clone());
        plan.steps.push((contact.name.clone(), Step::Move { from: PathBuf::from(&contact.key), to }));
    }
    plan
}

/// Groups added to each card, and others taken out: a card that has them all
/// already (or none of those to take out) is not counted.
pub fn plan_groups(contacts: &[&Contact], add: &[String], remove: &[String], tr: &Translator) -> Plan {
    let mut plan = Plan::default();
    for contact in contacts {
        if contact.read_only {
            plan.cannot.push(cannot(contact, tr.text("batch-why-read-only", None)));
            continue;
        }
        let text = match read(contact, tr) {
            Ok(text) => text,
            Err(c) => {
                plan.cannot.push(c);
                continue;
            }
        };
        if let Some(after) = contacts::with_categories(&text, add, remove) {
            plan.steps.push((contact.name.clone(), Step::Write { path: PathBuf::from(&contact.key), before: Some(text), after }));
        }
    }
    plan
}

/// The title at the start of each card's name moved into its title field
/// (`contacts::move_title`), each shown before and after; with `group`, every
/// card that has a title is added to it in the same step, those whose title
/// was in its field already too. A name with no title is not counted.
pub fn plan_title(contacts: &[&Contact], group: Option<&str>, tr: &Translator) -> Plan {
    let mut plan = Plan::default();
    let add: Vec<String> = group.map(str::trim).filter(|g| !g.is_empty()).map(str::to_string).into_iter().collect();
    for contact in contacts {
        if contact.read_only {
            if contacts::has_title(contact) {
                plan.cannot.push(cannot(contact, tr.text("batch-why-read-only", None)));
            }
            continue;
        }
        let text = match read(contact, tr) {
            Ok(text) => text,
            Err(c) => {
                plan.cannot.push(c);
                continue;
            }
        };
        let moved = match contacts::move_title(&text) {
            TitleMove::Moves { before, after, text: moved, .. } => {
                plan.previews.push((before, after));
                Some(moved)
            }
            TitleMove::Differs(held) => {
                let mut args = crate::i18n::args();
                args.set("title", held);
                plan.cannot.push(cannot(contact, tr.text("batch-why-other-title", Some(&args))));
                continue;
            }
            // Its title in its field already: only the group, when one is asked.
            TitleMove::None if !contact.prefix.trim().is_empty() && !add.is_empty() => None,
            TitleMove::None => continue,
        };
        let base = moved.as_deref().unwrap_or(&text);
        let after = if add.is_empty() { moved.clone() } else { contacts::with_categories(base, &add, &[]).or(moved.clone()) };
        if let Some(after) = after {
            plan.steps.push((contact.name.clone(), Step::Write { path: PathBuf::from(&contact.key), before: Some(text), after }));
        }
    }
    plan
}

/// Each card deleted, its text kept in the record to undo it.
pub fn plan_delete(contacts: &[&Contact], tr: &Translator) -> Plan {
    let mut plan = Plan::default();
    for contact in contacts {
        if contact.read_only {
            plan.cannot.push(cannot(contact, tr.text("batch-why-read-only", None)));
            continue;
        }
        match read(contact, tr) {
            Ok(before) => plan.steps.push((contact.name.clone(), Step::Remove { path: PathBuf::from(&contact.key), before })),
            Err(c) => plan.cannot.push(c),
        }
    }
    plan
}

/// A card's UID, as its first `UID` line says.
pub fn uid_of(text: &str) -> Option<String> {
    crate::lines::unfold(text).iter().find(|l| crate::lines::name(l) == "UID").map(|l| crate::lines::value(l).trim().to_string()).filter(|u| !u.is_empty())
}

// ---------------------------------------------------------------- run and undo

/// Keeps the plan, then runs it, one card after the other through the doors,
/// `progress(done, total)` after each; `stop` set, it ends before the next
/// card. `deleted_book`: the book a merge deletes once emptied, kept in the
/// record for its Undo. Nothing to do: no record, an empty outcome.
pub fn run(dir: &Path, kind: &str, steps: Vec<(String, Step)>, deleted_book: Option<String>, now: i64, stop: &AtomicBool, progress: &mut dyn FnMut(usize, usize)) -> Result<Outcome, String> {
    if steps.is_empty() {
        return Ok(Outcome::default());
    }
    let fail = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(fail)?;
    let id = std::iter::once(format!("{now}-{kind}")).chain((2..1000).map(|n| format!("{now}-{kind}-{n}"))).find(|id| !dir.join(format!("{id}.json")).exists()).ok_or_else(|| format!("{}: full", dir.display()))?;
    let record = Record { id: id.clone(), at: now, kind: kind.to_string(), steps, deleted_book };
    let text = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    crate::filelock::replace(&dir.join(format!("{id}.json")), text.as_bytes()).map_err(fail)?;
    let total = record.steps.len();
    let mut outcome = Outcome { id, ..Outcome::default() };
    for (n, (name, step)) in record.steps.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            outcome.stopped = true;
            break;
        }
        let done = match step {
            Step::Write { path, after, .. } => vdir::write_item(path, after),
            Step::Remove { path, .. } => vdir::remove_item(path),
            Step::Move { from, to } => vdir::move_item(from, to),
        };
        match done {
            Ok(()) => outcome.done += 1,
            Err(why) => outcome.failed.push(Cannot { name: name.clone(), why }),
        }
        progress(n + 1, total);
    }
    Ok(outcome)
}

/// A batch's record, by its id.
pub fn record(dir: &Path, id: &str) -> Option<Record> {
    if !is_id(id) {
        return None;
    }
    let mut record: Record = std::fs::read_to_string(dir.join(format!("{id}.json"))).ok().and_then(|t| serde_json::from_str(&t).ok())?;
    record.id = id.to_string();
    Some(record)
}

/// Puts back what a batch did, the last step first: a step done is taken
/// back, one never done (stopped, refused) is left alone; a card changed
/// since is set aside first (`<id>.<n>.vcf` beside the record), then put
/// back. Files outside `root` (the address books) are never touched. The
/// book a merge deleted is shown again first, while its server still has it.
/// Then the record goes.
pub fn undo(dir: &Path, id: &str, root: &Path, stop: &AtomicBool, progress: &mut dyn FnMut(usize, usize)) -> Result<Undone, String> {
    let record = record(dir, id).ok_or_else(|| format!("{id}: nothing to undo"))?;
    if let Some((account, book)) = record.deleted_book.as_deref().and_then(|b| b.split_once('/')) {
        vdir::undelete_collection(vdir::Kind::Contacts, account, book)?;
    }
    let inside = |path: &Path| path.starts_with(root) && !path.components().any(|c| c == std::path::Component::ParentDir);
    let mut undone = Undone::default();
    let total = record.steps.len();
    for (n, (name, step)) in record.steps.iter().enumerate().rev() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let back: Result<bool, String> = match step {
            Step::Write { path, before, after } if inside(path) => {
                let now = std::fs::read_to_string(path).ok();
                if now.as_deref() == before.as_deref() {
                    Ok(false)
                } else {
                    if now.as_ref().is_some_and(|now| now != after) {
                        std::fs::write(dir.join(format!("{id}.{n}.vcf")), now.unwrap_or_default()).map_err(|e| format!("{}: {e}", dir.display()))?;
                        undone.aside += 1;
                    }
                    match before {
                        Some(before) => vdir::write_item(path, before).map(|()| true),
                        None => vdir::remove_item(path).map(|()| true),
                    }
                }
            }
            Step::Remove { path, before } if inside(path) => {
                if path.exists() || path.parent().is_some_and(|p| !p.is_dir()) {
                    Ok(false)
                } else {
                    vdir::write_item(path, before).map(|()| true)
                }
            }
            Step::Move { from, to } if inside(from) && inside(to) => {
                if to.exists() && !from.exists() {
                    vdir::move_item(to, from).map(|()| true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        };
        match back {
            Ok(true) => undone.back += 1,
            Ok(false) => {}
            Err(why) => undone.failed.push(Cannot { name: name.clone(), why }),
        }
        progress(total - n, total);
    }
    if !stop.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(dir.join(format!("{id}.json")));
    }
    Ok(undone)
}

/// Records older than thirty days go, with what their undoing set aside.
pub fn forget_old(dir: &Path, now: i64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for path in entries.filter_map(Result::ok).map(|e| e.path()) {
        let old = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()).and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).is_some_and(|t| now - t.as_secs() as i64 > KEPT_DAYS * 86_400);
        if old {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vdir::{Kind, LOCAL};

    fn tr() -> Translator {
        Translator::new("en")
    }

    /// A book of this device's own with these cards (name, extra lines), made for one test.
    fn book(id: &str, cards: &[(&str, &str)]) -> Collection {
        crate::config::stand_ins_only();
        let _ = std::fs::remove_dir_all(Kind::Contacts.root().join(LOCAL).join(id));
        let dir = vdir::prepare(Kind::Contacts, LOCAL, id, id, None).unwrap();
        for (n, (name, extra)) in cards.iter().enumerate() {
            std::fs::write(dir.join(format!("c{n}.vcf")), format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{id}-{n}\r\nFN:{name}\r\n{extra}END:VCARD\r\n")).unwrap();
        }
        vdir::collections(Kind::Contacts).into_iter().find(|c| c.account == LOCAL && c.id == id).unwrap()
    }

    fn cards(book: &Collection) -> Vec<Contact> {
        book.items().iter().filter_map(|p| contacts::read(p, book)).collect()
    }

    fn texts(book: &Collection) -> Vec<String> {
        book.items().iter().map(|p| std::fs::read_to_string(p).unwrap()).collect()
    }

    /// Groups added to a selection, then the batch undone as a whole: every
    /// card as it was, byte for byte.
    #[test]
    fn groups_added_then_undone() {
        let book = book("batch-groups", &[("Lou", "CATEGORIES:Amis\r\n"), ("Sam", ""), ("Kim", "CATEGORIES:Clients\r\n")]);
        let before = texts(&book);
        let all = cards(&book);
        let chosen: Vec<&Contact> = all.iter().collect();
        let plan = plan_groups(&chosen, &["Clients".into()], &["Amis".into()], &tr());
        assert_eq!((plan.steps.len(), plan.cannot.len()), (2, 0), "Kim has it already: not counted");
        let dir = crate::config::state_dir().join("batch-groups-records");
        let outcome = run(&dir, "groups", plan.steps, None, 1_790_000_000, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        assert_eq!((outcome.done, outcome.failed.len(), outcome.stopped), (2, 0, false));
        assert!(cards(&book).iter().all(|c| c.categories == ["Clients"]), "{:?}", cards(&book));
        let undone = undo(&dir, &outcome.id, &Kind::Contacts.root(), &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        assert_eq!((undone.back, undone.aside), (2, 0));
        assert_eq!(texts(&book), before);
        assert!(record(&dir, &outcome.id).is_none(), "the record goes with its Undo");
        let _ = std::fs::remove_dir_all(&book.dir);
    }

    /// A move between two books stopped half-way, then undone: the cards moved
    /// come back, those never moved stay, each keeps its UID; never in neither
    /// book, never in both.
    #[test]
    fn a_batch_stopped_half_way_then_undone() {
        let from = book("batch-from", &[("A", ""), ("B", ""), ("C", ""), ("D", "")]);
        let to = book("batch-to", &[]);
        let before = texts(&from);
        let all = cards(&from);
        let chosen: Vec<&Contact> = all.iter().collect();
        let plan = plan_move(&chosen, &to, crate::capabilities::Provider::Dav, &tr());
        assert_eq!(plan.steps.len(), 4);
        let stop = AtomicBool::new(false);
        let dir = crate::config::state_dir().join("batch-stopped-records");
        let outcome = run(&dir, "move", plan.steps, None, 1_790_000_001, &stop, &mut |done, _| {
            if done == 2 {
                stop.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        assert_eq!((outcome.done, outcome.stopped), (2, true));
        assert_eq!((from.items().len(), to.items().len()), (2, 2), "half moved, none lost, none twice");
        let uids = |b: &Collection| {
            let mut u: Vec<String> = texts(b).iter().filter_map(|t| uid_of(t)).collect();
            u.sort();
            u
        };
        let mut all_uids = [uids(&from), uids(&to)].concat();
        all_uids.sort();
        assert_eq!(all_uids, ["batch-from-0", "batch-from-1", "batch-from-2", "batch-from-3"]);
        let undone = undo(&dir, &outcome.id, &Kind::Contacts.root(), &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        assert_eq!((undone.back, undone.failed.len()), (2, 0));
        assert_eq!((to.items().len(), texts(&from).len()), (0, 4));
        let mut now = texts(&from);
        now.sort();
        let mut was = before.clone();
        was.sort();
        assert_eq!(now, was, "each card back, its text and UID as they were");
        let _ = std::fs::remove_dir_all(&from.dir);
        let _ = std::fs::remove_dir_all(&to.dir);
    }

    /// The title moved, with a group in the same step; a card changed after
    /// the batch is set aside before its Undo; a deletion undone comes back.
    #[test]
    fn titles_and_deletions_undone() {
        let book = book("batch-titles", &[("Mme Jeanne Martin", "N:Martin;Mme Jeanne;;;\r\n"), ("Meredith Example", ""), ("Lou Exemple", "N:Exemple;Lou;;Dr;\r\n")]);
        let all = cards(&book);
        let chosen: Vec<&Contact> = all.iter().collect();
        let plan = plan_title(&chosen, Some("Professional"), &tr());
        assert_eq!(plan.previews, [("Mme Jeanne Martin".to_string(), "Jeanne Martin".to_string())]);
        assert_eq!(plan.steps.len(), 2, "Jeanne moved, Lou's title in its field already: its group alone; Meredith none");
        let dir = crate::config::state_dir().join("batch-titles-records");
        let outcome = run(&dir, "title", plan.steps, None, 1_790_000_002, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        let after = cards(&book);
        let jeanne = after.iter().find(|c| c.uid == "batch-titles-0").unwrap();
        assert_eq!((jeanne.name.as_str(), jeanne.prefix.as_str(), jeanne.categories.clone()), ("Jeanne Martin", "Mme", vec!["Professional".to_string()]));
        // Changed since: set aside, then put back.
        std::fs::write(&jeanne.key, std::fs::read_to_string(&jeanne.key).unwrap().replace("END:VCARD", "NOTE:later\r\nEND:VCARD")).unwrap();
        let undone = undo(&dir, &outcome.id, &Kind::Contacts.root(), &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        assert_eq!((undone.back, undone.aside), (2, 1));
        assert!(cards(&book).iter().all(|c| c.categories.is_empty()) && cards(&book).iter().any(|c| c.name == "Mme Jeanne Martin"));
        // Deleted, then undone: back under its file name.
        let all = cards(&book);
        let chosen: Vec<&Contact> = all.iter().take(2).collect();
        let plan = plan_delete(&chosen, &tr());
        let outcome = run(&dir, "delete", plan.steps, None, 1_790_000_003, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        assert_eq!((outcome.done, book.items().len()), (2, 1));
        undo(&dir, &outcome.id, &Kind::Contacts.root(), &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        assert_eq!(book.items().len(), 3);
        let _ = std::fs::remove_dir_all(&book.dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A book you only read is refused at the plan and at the doors
    /// themselves: a write, a removal, a move out of it or into it.
    #[test]
    fn a_read_only_book_is_refused_at_each_door() {
        let book = book("batch-read-only", &[("Lou", "")]);
        let other = book_named("batch-writable");
        let card = book.items()[0].clone();
        let before = std::fs::read_to_string(&card).unwrap();
        // Its server says read only (`State::read_only`, as a sync writes it).
        vdir::State { read_only: true, ..vdir::State::default() }.save(&book.state_path()).unwrap();
        let refused = crate::agenda::translator().text("contact-read-only", None);
        assert_eq!(vdir::write_item(&card, "BEGIN:VCARD\r\nEND:VCARD\r\n"), Err(refused.clone()));
        assert_eq!(vdir::remove_item(&card), Err(refused.clone()));
        assert_eq!(vdir::move_item(&card, &other.dir.join("x.vcf")), Err(refused.clone()));
        let into = other.dir.join("y.vcf");
        std::fs::write(&into, &before).unwrap();
        assert_eq!(vdir::move_item(&into, &book.dir.join("y.vcf")), Err(refused.clone()), "never into it either");
        assert_eq!(std::fs::read_to_string(&card).unwrap(), before, "untouched");
        // The plans leave its cards out, said; a move into it leaves every card.
        let read_only = vdir::collections(Kind::Contacts).into_iter().find(|c| c.id == "batch-read-only").unwrap();
        let all = cards(&read_only);
        let chosen: Vec<&Contact> = all.iter().collect();
        for plan in [plan_groups(&chosen, &["X".into()], &[], &tr()), plan_delete(&chosen, &tr()), plan_move(&chosen, &other, crate::capabilities::Provider::Dav, &tr())] {
            assert!(plan.steps.is_empty() && plan.cannot.len() == 1, "{plan:?}");
        }
        let into_plan = plan_move(&cards(&other).iter().collect::<Vec<_>>(), &read_only, crate::capabilities::Provider::Dav, &tr());
        assert!(into_plan.steps.is_empty());
        let _ = std::fs::remove_file(book.state_path());
        let _ = std::fs::remove_dir_all(&book.dir);
        let _ = std::fs::remove_dir_all(&other.dir);
    }

    fn book_named(id: &str) -> Collection {
        book(id, &[])
    }

    /// A card whose UID the target holds already is left, said: its server would refuse it.
    #[test]
    fn a_move_never_doubles_a_uid() {
        let from = book("batch-uid-from", &[("Lou", "")]);
        let to = book("batch-uid-to", &[]);
        std::fs::write(to.dir.join("same.vcf"), "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:batch-uid-from-0\r\nFN:Lou\r\nEND:VCARD\r\n").unwrap();
        let all = cards(&from);
        let plan = plan_move(&all.iter().collect::<Vec<_>>(), &to, crate::capabilities::Provider::Dav, &tr());
        assert_eq!((plan.steps.len(), plan.cannot[0].why.clone()), (0, tr().text("batch-why-same-uid", None)));
        let _ = std::fs::remove_dir_all(&from.dir);
        let _ = std::fs::remove_dir_all(&to.dir);
    }
}

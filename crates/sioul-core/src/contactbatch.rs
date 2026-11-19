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
//!    Each card is checked before its step: one changed since the plan was
//!    made is left as it is, and said (review F17). Each step done is noted
//!    beside the record (`<id>.log`), so that a batch cut short knows how far
//!    it went.
//! 4. **Undone** as a whole (`undo`), from the status line at once or from
//!    Contacts ⚙ ▸ Address books for thirty days (`list`): each step done is
//!    taken back while its card is as the batch left it; a card changed,
//!    moved or deleted since is left as it is, and said; a card deleted is
//!    never brought back beside the same card elsewhere. A move goes back by
//!    a move: the card keeps its UID either way. A book a merge removed is
//!    made again first, here and, at the next sync, on its server while it
//!    is still there.
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

/// One card of the title's move, shown before it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preview {
    /// Its file: the box of each card says which run.
    pub key: String,
    pub before: String,
    pub after: String,
    /// Ticked at first: the title cannot be anything else (`titles::sure`).
    pub sure: bool,
    /// What else is said of it ("its title is in its field already: the group alone"), or "".
    pub note: String,
}

/// What a batch will do, before it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Plan {
    /// Each card's name and its step.
    pub steps: Vec<(String, Step)>,
    pub cannot: Vec<Cannot>,
    /// For the title's move: each card, before and after.
    pub previews: Vec<Preview>,
    /// What the book a move goes to would not keep, in words, each once.
    pub losses: Vec<String>,
}

/// The book a merge removes once emptied: made again by its Undo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Removed {
    /// "account/id".
    pub place: String,
    pub name: String,
    /// Made here and not on its server yet, or kept on this device alone: its folder goes at once.
    pub here_only: bool,
}

/// What a batch did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// Its record, to undo it.
    pub id: String,
    pub done: usize,
    /// What a door refused, or what changed since it was planned, card by card.
    pub failed: Vec<Cannot>,
    /// Stopped before its end: the cards after it were left as they were.
    pub stopped: bool,
}

/// What the Undo of a batch did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Undone {
    /// The cards put back.
    pub back: usize,
    /// Those left as they are, and why: changed, moved or deleted since.
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
    /// A book a merge emptied and removed: made again by its Undo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<Removed>,
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
    // A book its server did not make (review F6): its cards would stay on this device alone.
    if vdir::State::load(&target.state_path()).refused {
        plan.cannot = contacts.iter().map(|c| cannot(c, tr.text("batch-why-target-refused", None))).collect();
        return plan;
    }
    // The target's UIDs, and those of the cards planned before (review F8: two cards of one UID moved together).
    let mut held: Vec<String> = target.items().iter().filter_map(|p| std::fs::read_to_string(p).ok()).filter_map(|t| uid_of(&t)).collect();
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
        if let Some(uid) = uid_of(&text) {
            if held.contains(&uid) {
                plan.cannot.push(cannot(contact, tr.text("batch-why-same-uid", None)));
                continue;
            }
            held.push(uid);
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
            TitleMove::Moves { before, after, sure, text: moved, .. } => {
                plan.previews.push(Preview { key: contact.key.clone(), before, after, sure, note: String::new() });
                Some(moved)
            }
            TitleMove::Differs(held) => {
                let mut args = crate::i18n::args();
                args.set("title", held);
                plan.cannot.push(cannot(contact, tr.text("batch-why-other-title", Some(&args))));
                continue;
            }
            // Two people in one name: never changed (review F18).
            TitleMove::Household => {
                plan.cannot.push(cannot(contact, tr.text("batch-why-household", None)));
                continue;
            }
            // Its title in its field already: only the group, when one is asked.
            TitleMove::None if !contact.prefix.trim().is_empty() && !add.is_empty() => {
                plan.previews.push(Preview { key: contact.key.clone(), before: contact.name.clone(), after: contact.name.clone(), sure: true, note: tr.text("batch-title-group-only", None) });
                None
            }
            // A title shown at the start of its name, with no name line to take it from.
            TitleMove::None if crate::titles::split(&contact.name).is_some() && contact.prefix.trim().is_empty() => {
                plan.cannot.push(cannot(contact, tr.text("batch-why-no-name-line", None)));
                continue;
            }
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

/// The words of a step left as it is.
fn why(tr: &Translator, key: &str) -> String {
    tr.text(key, None)
}

/// A step noted beside the record, as it happens: "doing" before it,
/// "done" or "left" after, so that how far a batch went is known even when
/// Sioul stops in the middle of a step (review V8). False when it cannot be
/// written: the batch then stops before that step.
fn note(dir: &Path, id: &str, n: usize, word: &str) -> bool {
    use std::io::Write;
    std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{id}.log"))).and_then(|mut log| writeln!(log, "{n} {word}").and_then(|()| log.sync_data())).is_ok()
}

/// Whether a step's effect shows on the files: its card written as it says,
/// taken out, or moved.
fn shows_done(step: &Step) -> bool {
    match step {
        Step::Write { path, after, .. } => std::fs::read_to_string(path).is_ok_and(|now| now == *after),
        Step::Remove { path, .. } => !path.exists(),
        Step::Move { from, to } => to.exists() && !from.exists(),
    }
}

/// The steps a batch did, by their place in its plan: those noted done, and
/// one begun and never noted after (Sioul stopped in it) whose effect shows.
fn steps_done(dir: &Path, record: &Record) -> std::collections::BTreeSet<usize> {
    let log = std::fs::read_to_string(dir.join(format!("{}.log", record.id))).unwrap_or_default();
    let words: Vec<(usize, &str)> = log.lines().filter_map(|l| l.split_once(' ').and_then(|(n, w)| Some((n.parse().ok()?, w)))).collect();
    let ended = |n: usize| words.iter().any(|(m, w)| *m == n && (*w == "done" || *w == "left"));
    words
        .iter()
        .filter(|(n, word)| *word == "done" || (*word == "doing" && !ended(*n) && record.steps.get(*n).is_some_and(|(_, step)| shows_done(step))))
        .map(|(n, _)| *n)
        .collect()
}

/// Whether a step's card is still as the plan read it, else why it is left.
fn as_planned(step: &Step, tr: &Translator) -> Result<(), String> {
    let now = |path: &Path| std::fs::read_to_string(path).ok();
    match step {
        Step::Write { path, before: Some(before), .. } | Step::Remove { path, before } => match now(path) {
            Some(text) if &text == before => Ok(()),
            Some(_) => Err(why(tr, "batch-why-changed-since")),
            None => Err(why(tr, "batch-why-gone")),
        },
        Step::Write { path, before: None, .. } => if path.exists() { Err(why(tr, "batch-why-taken")) } else { Ok(()) },
        Step::Move { from, to } => {
            if !from.exists() {
                Err(why(tr, "batch-why-gone"))
            } else if to.exists() {
                Err(why(tr, "batch-why-taken"))
            } else {
                Ok(())
            }
        }
    }
}

/// Keeps the plan, then runs it, one card after the other through the doors,
/// `progress(done, total)` after each; `stop` set, it ends before the next
/// card. A card no longer as the plan read it is left, and said. `removed`:
/// the book a merge removes once emptied, kept in the record for its Undo.
/// Nothing to do: no record, an empty outcome.
#[allow(clippy::too_many_arguments)]
pub fn run(dir: &Path, kind: &str, steps: Vec<(String, Step)>, removed: Option<Removed>, now: i64, stop: &AtomicBool, tr: &Translator, progress: &mut dyn FnMut(usize, usize)) -> Result<Outcome, String> {
    if steps.is_empty() {
        return Ok(Outcome::default());
    }
    let fail = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(fail)?;
    let id = std::iter::once(format!("{now}-{kind}")).chain((2..1000).map(|n| format!("{now}-{kind}-{n}"))).find(|id| !dir.join(format!("{id}.json")).exists()).ok_or_else(|| format!("{}: full", dir.display()))?;
    let record = Record { id: id.clone(), at: now, kind: kind.to_string(), steps, removed };
    let text = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    crate::filelock::replace(&dir.join(format!("{id}.json")), text.as_bytes()).map_err(fail)?;
    let total = record.steps.len();
    let mut outcome = Outcome { id: id.clone(), ..Outcome::default() };
    for (n, (name, step)) in record.steps.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            outcome.stopped = true;
            break;
        }
        // Noted before it runs: a step that cannot be noted is not run (review V8).
        if !note(dir, &id, n, "doing") {
            outcome.stopped = true;
            outcome.failed.push(Cannot { name: name.clone(), why: why(tr, "batch-why-unrecorded") });
            break;
        }
        let done = as_planned(step, tr).and_then(|()| match step {
            Step::Write { path, after, .. } => vdir::write_item(path, after),
            Step::Remove { path, .. } => vdir::remove_item(path),
            Step::Move { from, to } => vdir::move_item(from, to),
        });
        note(dir, &id, n, if done.is_ok() { "done" } else { "left" });
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

/// Whether a card of this UID is in an address book here now.
pub(crate) fn uid_here(uid: &str) -> bool {
    vdir::collections(vdir::Kind::Contacts).iter().flat_map(|c| c.items()).any(|p| std::fs::read_to_string(p).ok().and_then(|t| uid_of(&t)).as_deref() == Some(uid))
}

/// The book a merge removed, made again when its folder went (on this device
/// alone, or made here and never sent); shown again when it waits for its
/// deletion there. A server's book whose folder went is gone from its server.
fn restore(removed: &Removed, tr: &Translator) -> Result<(), String> {
    let Some((account, id)) = removed.place.split_once('/') else { return Err(why(tr, "batch-why-book-gone")) };
    if vdir::Kind::Contacts.root().join(account).join(id).is_dir() {
        return vdir::undelete_collection(vdir::Kind::Contacts, account, id).map_err(|_| why(tr, "batch-why-book-gone"));
    }
    if !removed.here_only {
        return Err(why(tr, "batch-why-book-gone"));
    }
    vdir::restore_collection(vdir::Kind::Contacts, account, id, &removed.name).map_err(|_| why(tr, "batch-why-book-gone"))
}

/// Puts back what a batch did, the last step first, each step that was done
/// while its card is as the batch left it; a card changed, moved or deleted
/// since is left as it is, and said, never written over nor brought back
/// beside itself (review F17). Files outside `root` (the address books) are
/// never touched. The book a merge removed is made again first. Then the
/// record goes, unless stopped.
pub fn undo(dir: &Path, id: &str, root: &Path, stop: &AtomicBool, tr: &Translator, progress: &mut dyn FnMut(usize, usize)) -> Result<Undone, String> {
    let record = record(dir, id).ok_or_else(|| why(tr, "batch-why-undo-gone"))?;
    let mut undone = Undone::default();
    if let Some(removed) = &record.removed
        && let Err(why) = restore(removed, tr)
    {
        undone.failed.push(Cannot { name: removed.name.clone(), why });
    }
    let done = steps_done(dir, &record);
    let inside = |path: &Path| path.starts_with(root) && !path.components().any(|c| c == std::path::Component::ParentDir);
    let total = record.steps.len();
    for (n, (name, step)) in record.steps.iter().enumerate().rev() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let left = |key: &str| Err::<bool, String>(why(tr, key));
        let back: Result<bool, String> = match step {
            Step::Write { path, before, after } if inside(path) => {
                let now = std::fs::read_to_string(path).ok();
                if now.as_deref() == Some(after.as_str()) {
                    match before {
                        Some(before) => vdir::write_item(path, before).map(|()| true),
                        None => vdir::remove_item(path).map(|()| true),
                    }
                } else if now.as_deref() == before.as_deref() || !done.contains(&n) {
                    Ok(false)
                } else if now.is_none() {
                    left("batch-why-gone-since")
                } else {
                    left("batch-why-changed-since-undo")
                }
            }
            Step::Remove { path, before } if inside(path) => {
                if path.exists() || !done.contains(&n) {
                    Ok(false)
                } else if path.parent().is_some_and(|p| !p.is_dir()) {
                    left("batch-why-book-gone")
                } else if uid_of(before).is_some_and(|uid| uid_here(&uid)) {
                    left("batch-why-elsewhere-now")
                } else {
                    vdir::write_item(path, before).map(|()| true)
                }
            }
            Step::Move { from, to } if inside(from) && inside(to) => {
                if to.exists() && !from.exists() {
                    if from.parent().is_some_and(|p| !p.is_dir()) { left("batch-why-book-gone") } else { vdir::move_item(to, from).map(|()| true) }
                } else if done.contains(&n) && !from.exists() {
                    left("batch-why-gone-since")
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
        let _ = std::fs::remove_file(dir.join(format!("{id}.log")));
    }
    Ok(undone)
}

/// A batch of the last thirty days, as Contacts ⚙ ▸ Address books lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listed {
    pub id: String,
    /// "move", "groups", "title", "delete", "merge", "import".
    pub kind: String,
    pub at: i64,
    /// How many cards it planned, and how many it changed.
    pub planned: usize,
    pub done: usize,
    /// Whether its Undo can still do something, and why not when it cannot ("" when it can).
    pub why_not: String,
}

/// The batches of the last thirty days, the newest first.
pub fn list(dir: &Path, now: i64, tr: &Translator) -> Vec<Listed> {
    forget_old(dir, now);
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<Listed> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(str::to_string))
        .filter_map(|id| record(dir, &id))
        .map(|r| {
            let done = steps_done(dir, &r).len();
            let why_not = match &r.removed {
                Some(removed) if !removed.here_only && removed.place.split_once('/').is_none_or(|(a, i)| !vdir::Kind::Contacts.root().join(a).join(i).is_dir()) => why(tr, "batch-why-book-gone"),
                _ if done == 0 => why(tr, "batch-why-nothing-done"),
                _ => String::new(),
            };
            Listed { id: r.id.clone(), kind: r.kind.clone(), at: r.at, planned: r.steps.len(), done, why_not }
        })
        .collect();
    found.sort_by(|a, b| b.at.cmp(&a.at).then(b.id.cmp(&a.id)));
    found
}

/// Records older than thirty days go, with their notes.
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

    fn go(dir: &Path, kind: &str, steps: Vec<(String, Step)>, removed: Option<Removed>, now: i64) -> Outcome {
        run(dir, kind, steps, removed, now, &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap()
    }

    fn back(dir: &Path, id: &str) -> Undone {
        undo(dir, id, &Kind::Contacts.root(), &AtomicBool::new(false), &tr(), &mut |_, _| {}).unwrap()
    }

    /// Groups added to a selection, then the batch undone as a whole: every
    /// card as it was, byte for byte; listed for thirty days until then.
    #[test]
    fn groups_added_then_undone() {
        let book = book("batch-groups", &[("Lou", "CATEGORIES:Amis\r\n"), ("Sam", ""), ("Kim", "CATEGORIES:Clients\r\n")]);
        let before = texts(&book);
        let all = cards(&book);
        let chosen: Vec<&Contact> = all.iter().collect();
        let plan = plan_groups(&chosen, &["Clients".into()], &["Amis".into()], &tr());
        assert_eq!((plan.steps.len(), plan.cannot.len()), (2, 0), "Kim has it already: not counted");
        let dir = crate::config::state_dir().join("batch-groups-records");
        let outcome = go(&dir, "groups", plan.steps, None, 1_790_000_000);
        assert_eq!((outcome.done, outcome.failed.len(), outcome.stopped), (2, 0, false));
        assert!(cards(&book).iter().all(|c| c.categories == ["Clients"]), "{:?}", cards(&book));
        let listed = list(&dir, 1_790_000_100, &tr());
        assert_eq!(listed.iter().map(|l| (l.kind.as_str(), l.planned, l.done, l.why_not.as_str())).collect::<Vec<_>>(), [("groups", 2, 2, "")]);
        let undone = back(&dir, &outcome.id);
        assert_eq!((undone.back, undone.failed.len()), (2, 0));
        assert_eq!(texts(&book), before);
        assert!(record(&dir, &outcome.id).is_none() && list(&dir, 1_790_000_100, &tr()).is_empty(), "the record goes with its Undo");
        let _ = std::fs::remove_dir_all(&book.dir);
    }

    /// Review V8: Sioul stopped between a card's removal and its note: the
    /// step begun ("doing", never "done") whose effect shows is counted done,
    /// and its Undo puts the card back; a step that cannot be noted is not run.
    #[test]
    fn a_step_begun_and_never_noted_is_read_by_its_effect() {
        let b = book("batch-begun", &[("Lou Exemple", ""), ("Sam Exemple", "")]);
        let all = cards(&b);
        let plan = plan_delete(&all.iter().collect::<Vec<_>>(), &tr());
        let dir = crate::config::state_dir().join("batch-begun-records");
        let outcome = go(&dir, "delete", plan.steps, None, 1_790_000_050);
        assert_eq!((outcome.done, b.items().len()), (2, 0));
        let log = dir.join(format!("{}.log", outcome.id));
        std::fs::write(&log, "0 doing\n0 done\n1 doing\n").unwrap();
        assert_eq!(list(&dir, 1_790_000_100, &tr()).iter().find(|l| l.id == outcome.id).map(|l| l.done), Some(2));
        let undone = back(&dir, &outcome.id);
        assert_eq!((undone.back, undone.failed.len(), b.items().len()), (2, 0, 2));
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
        let outcome = run(&dir, "move", plan.steps, None, 1_790_000_001, &stop, &tr(), &mut |done, _| {
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
        assert_eq!(list(&dir, 1_790_000_100, &tr())[0].done, 2, "how far it went, kept beside the record");
        let undone = back(&dir, &outcome.id);
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

    /// Review F17: a card the stopped batch never wrote, changed since by a
    /// sync, is left as it is by the Undo; a card changed after the batch
    /// wrote it is left too, and said; a card moved since is never written
    /// back in its old book.
    #[test]
    fn undo_never_writes_over_what_changed_since() {
        let book = book("batch-since", &[("A", ""), ("B", ""), ("C", "")]);
        let all = cards(&book);
        let plan = plan_groups(&all.iter().collect::<Vec<_>>(), &["Clients".into()], &[], &tr());
        let stop = AtomicBool::new(false);
        let dir = crate::config::state_dir().join("batch-since-records");
        let outcome = run(&dir, "groups", plan.steps, None, 1_790_000_010, &stop, &tr(), &mut |done, _| {
            if done == 2 {
                stop.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        assert_eq!(outcome.done, 2);
        let (first, second, third) = (book.dir.join("c0.vcf"), book.dir.join("c1.vcf"), book.dir.join("c2.vcf"));
        // Another device's change to the third (never written by the batch), and to the second (written by it).
        let theirs = std::fs::read_to_string(&third).unwrap().replace("END:VCARD", "TEL:+33465710003\r\nEND:VCARD");
        std::fs::write(&third, &theirs).unwrap();
        let changed = std::fs::read_to_string(&second).unwrap().replace("END:VCARD", "NOTE:later\r\nEND:VCARD");
        std::fs::write(&second, &changed).unwrap();
        // The first moved to another book since.
        let other = book_named("batch-since-other");
        vdir::move_item(&first, &other.dir.join("moved.vcf")).unwrap();
        let undone = back(&dir, &outcome.id);
        assert_eq!(undone.back, 0);
        assert_eq!(undone.failed.iter().map(|f| f.why.clone()).collect::<Vec<_>>(), [tr().text("batch-why-changed-since-undo", None), tr().text("batch-why-gone-since", None)]);
        assert_eq!((std::fs::read_to_string(&third).unwrap(), std::fs::read_to_string(&second).unwrap()), (theirs, changed), "both left as they are");
        assert!(!first.exists(), "never written back beside the moved card");
        let _ = std::fs::remove_dir_all(&book.dir);
        let _ = std::fs::remove_dir_all(&other.dir);
    }

    /// Review F17: a plan run later meets a card changed since it was made: left, and said.
    #[test]
    fn a_plan_run_later_leaves_what_changed_since() {
        let book = book("batch-later", &[("Lou", "")]);
        let all = cards(&book);
        let plan = plan_groups(&all.iter().collect::<Vec<_>>(), &["Clients".into()], &[], &tr());
        let path = book.dir.join("c0.vcf");
        let theirs = std::fs::read_to_string(&path).unwrap().replace("END:VCARD", "NOTE:Rappeler mardi\r\nEND:VCARD");
        std::fs::write(&path, &theirs).unwrap();
        let outcome = go(&crate::config::state_dir().join("batch-later-records"), "groups", plan.steps, None, 1_790_000_011);
        assert_eq!((outcome.done, outcome.failed[0].why.clone()), (0, tr().text("batch-why-changed-since", None)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), theirs);
        let _ = std::fs::remove_dir_all(&book.dir);
    }

    /// The title moved, with a group in the same step; a name of two people
    /// left alone; each card's box ticked when the title is sure; a deletion
    /// undone comes back, never beside the same card elsewhere.
    #[test]
    fn titles_and_deletions_undone() {
        let book = book("batch-titles", &[("Mme Jeanne Martin", "N:Martin;Mme Jeanne;;;\r\n"), ("Meredith Example", ""), ("Lou Exemple", "N:Exemple;Lou;;Dr;\r\n"), ("M. et Mme Dupont", ""), ("Me Camille Exemple", "N:Exemple;Camille;;;\r\n")]);
        let all = cards(&book);
        let chosen: Vec<&Contact> = all.iter().collect();
        let plan = plan_title(&chosen, Some("Professional"), &tr());
        let previews: Vec<(String, String, bool)> = plan.previews.iter().map(|p| (p.before.clone(), p.after.clone(), p.sure)).collect();
        assert_eq!(
            previews,
            [("Mme Jeanne Martin".to_string(), "Jeanne Martin".to_string(), true), ("Lou Exemple".into(), "Lou Exemple".into(), true), ("Me Camille Exemple".into(), "Camille Exemple".into(), false)],
            "Jeanne sure, Lou's group alone, Maître unsure"
        );
        assert_eq!(plan.cannot.iter().map(|c| (c.name.as_str(), c.why.clone())).collect::<Vec<_>>(), [("M. et Mme Dupont", tr().text("batch-why-household", None))]);
        let steps: Vec<(String, Step)> = plan.steps.into_iter().filter(|(_, s)| !s.path().ends_with("c4.vcf")).collect();
        let dir = crate::config::state_dir().join("batch-titles-records");
        let outcome = go(&dir, "title", steps, None, 1_790_000_002);
        let after = cards(&book);
        let jeanne = after.iter().find(|c| c.uid == "batch-titles-0").unwrap();
        assert_eq!((jeanne.name.as_str(), jeanne.prefix.as_str(), jeanne.categories.clone()), ("Jeanne Martin", "Mme", vec!["Professional".to_string()]));
        assert!(after.iter().any(|c| c.name == "Me Camille Exemple"), "unticked: unchanged");
        let undone = back(&dir, &outcome.id);
        assert_eq!(undone.back, 2);
        assert!(cards(&book).iter().all(|c| c.categories.is_empty()) && cards(&book).iter().any(|c| c.name == "Mme Jeanne Martin"));
        // Deleted, then undone: back under its file name; one whose UID is elsewhere now stays out.
        let all = cards(&book);
        let chosen: Vec<&Contact> = all.iter().filter(|c| c.uid == "batch-titles-1" || c.uid == "batch-titles-2").collect();
        let plan = plan_delete(&chosen, &tr());
        let outcome = go(&dir, "delete", plan.steps, None, 1_790_000_003);
        assert_eq!((outcome.done, book.items().len()), (2, 3));
        let other = book_named("batch-titles-other");
        std::fs::write(other.dir.join("again.vcf"), "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:batch-titles-2\r\nFN:Lou Exemple\r\nEND:VCARD\r\n").unwrap();
        let undone = back(&dir, &outcome.id);
        assert_eq!((undone.back, undone.failed.iter().map(|f| f.why.clone()).collect::<Vec<_>>()), (1, vec![tr().text("batch-why-elsewhere-now", None)]));
        assert_eq!(book.items().len(), 4);
        let _ = std::fs::remove_dir_all(&book.dir);
        let _ = std::fs::remove_dir_all(&other.dir);
    }

    /// Review F3: a merge of a book kept on this device alone is undone: the
    /// book made again with its name, its cards moved back.
    #[test]
    fn a_merge_of_this_device_s_book_is_undone() {
        let from = book("batch-merge-from", &[("Lou", "")]);
        let to = book("batch-merge-to", &[]);
        let all = cards(&from);
        let plan = plan_move(&all.iter().collect::<Vec<_>>(), &to, crate::capabilities::Provider::Dav, &tr());
        let dir = crate::config::state_dir().join("batch-merge-records");
        let removed = Removed { place: format!("{LOCAL}/batch-merge-from"), name: "Old book".into(), here_only: true };
        let outcome = go(&dir, "merge", plan.steps, Some(removed), 1_790_000_040);
        vdir::delete_collection(&from).unwrap();
        assert!(!from.dir.exists());
        let undone = back(&dir, &outcome.id);
        assert_eq!((undone.back, undone.failed.len()), (1, 0), "{undone:?}");
        let again = vdir::collections(Kind::Contacts).into_iter().find(|c| c.account == LOCAL && c.id == "batch-merge-from").expect("made again");
        assert_eq!((again.name.as_str(), again.items().len(), to.items().len()), ("Old book", 1, 0));
        let _ = std::fs::remove_dir_all(&from.dir);
        let _ = std::fs::remove_dir_all(&to.dir);
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

    /// A card whose UID the target holds already is left, said: its server
    /// would refuse it; so is the second of two cards of one UID moved
    /// together (review F8).
    #[test]
    fn a_move_never_doubles_a_uid() {
        let from = book("batch-uid-from", &[("Lou", "")]);
        let to = book("batch-uid-to", &[]);
        std::fs::write(to.dir.join("same.vcf"), "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:batch-uid-from-0\r\nFN:Lou\r\nEND:VCARD\r\n").unwrap();
        let all = cards(&from);
        let plan = plan_move(&all.iter().collect::<Vec<_>>(), &to, crate::capabilities::Provider::Dav, &tr());
        assert_eq!((plan.steps.len(), plan.cannot[0].why.clone()), (0, tr().text("batch-why-same-uid", None)));
        let twin = book("batch-uid-twin", &[]);
        std::fs::write(twin.dir.join("same.vcf"), "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:batch-uid-from-0\r\nFN:Lou\r\nEND:VCARD\r\n").unwrap();
        let third = book("batch-uid-third", &[]);
        let mut both = cards(&from);
        both.extend(cards(&twin));
        let plan = plan_move(&both.iter().collect::<Vec<_>>(), &third, crate::capabilities::Provider::Dav, &tr());
        assert_eq!((plan.steps.len(), plan.cannot.len()), (1, 1));
        for b in [from, to, twin, third] {
            let _ = std::fs::remove_dir_all(&b.dir);
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Composed tasks changed (docs/tasks.md, "Composed tasks"): a task made a
//! step of another, a step made a task of its own, an item of a checklist
//! made a step or a task, a step folded back into its bigger task's list;
//! and a bigger task closing with its last step. The window, the command
//! line and the MCP do them through these, the same way.
//!
//! Each change is worked out first, as the files it writes (`Change`), or
//! refused with its reason (`Refusal`); then written once (`Change::write`),
//! which says what each file was, for Undo (`agentwrites::FileChange`).
//!
//! - **A step of another** (`demote`): never of itself nor of one of its own
//!   steps; never a task of a list you only read, nor one done. A step stays
//!   in its bigger task's list: from another list, it moves there with its
//!   own steps, through the move journal (`vdir::move_item`), what that list
//!   would not keep said first. Its waits, projects, blocks and links stay.
//!   It goes after the bigger task's other steps. Google Tasks keeps one
//!   level of steps.
//! - **A task of its own** (`promote`): its PARENT line goes (a CHILD line
//!   the bigger task holds for it too); its place stays, harmless.
//! - **An item made a step** (`item_to_step`): its words its title, its
//!   detail its notes, at the end of the sequence, its tick kept (done, a
//!   step done; not needed, a step dropped). **An item made a task**
//!   (`item_to_task`): the same, a task of its own tied to what held the list.
//! - **A step folded into the list** (`step_to_item`): refused when the step
//!   holds more than words and a tick (an estimate, a wait, steps, a time
//!   block…), naming what would go, unless asked again.
//! - **Closing with its last step** (`closing_with`): a bigger task without
//!   an estimate of its own and without an open wait is done once its last
//!   step is, a step done among them; further up too.

use crate::agentwrites::FileChange;
use crate::capabilities::{self, Provider};
use crate::checklist::{self, Mark};
use crate::i18n::Translator;
use crate::tasks::{self, Status, Task, TaskEdit};
use crate::vdir::{self, Collection};
use jiff::Zoned;
use jiff::tz::TimeZone;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Why a change was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A task named is not there (deleted meanwhile).
    Gone,
    /// A task made a step of itself, or of one of its own steps.
    Loop,
    /// In a list you only read.
    ReadOnly,
    /// Done or dropped.
    Done,
    /// It is no step of another.
    NotAStep,
    /// Google Tasks keeps one level of steps.
    OneLevel,
    /// What it would lose, as `loss-…` words' keys: said, and done when asked again.
    Losses(Vec<&'static str>),
    /// The list changed since it was read.
    Stale,
}

impl Refusal {
    /// In your words.
    pub fn said(&self, tr: &Translator) -> String {
        match self {
            Refusal::Gone => tr.text("task-gone", None),
            Refusal::Loop => tr.text("composed-loop", None),
            Refusal::ReadOnly => tr.text("task-read-only", None),
            Refusal::Done => tr.text("composed-done", None),
            Refusal::NotAStep => tr.text("composed-not-a-step", None),
            Refusal::OneLevel => tr.text("composed-one-level", None),
            Refusal::Stale => tr.text("checklist-stale", None),
            Refusal::Losses(what) => {
                let mut args = crate::i18n::args();
                args.set("what", what.iter().map(|w| tr.text(&format!("loss-{w}"), None)).collect::<Vec<_>>().join(", "));
                tr.text("composed-would-lose", Some(&args))
            }
        }
    }
}

/// The files a change writes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Change {
    /// Written again where they are: (file, its new text).
    pub written: Vec<(PathBuf, String)>,
    /// Moved to another list through the move journal: (from, to, its text there).
    pub moved: Vec<(PathBuf, PathBuf, String)>,
    /// New: (file, text).
    pub made: Vec<(PathBuf, String)>,
    /// Taken out.
    pub gone: Vec<PathBuf>,
}

impl Change {
    pub fn is_empty(&self) -> bool {
        self.written.is_empty() && self.moved.is_empty() && self.made.is_empty() && self.gone.is_empty()
    }

    /// Every file it touches, for the syncs to send and Undo to put back.
    pub fn files(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self.written.iter().map(|(p, _)| p.clone()).collect();
        out.extend(self.moved.iter().flat_map(|(from, to, _)| [from.clone(), to.clone()]));
        out.extend(self.made.iter().map(|(p, _)| p.clone()));
        out.extend(self.gone.iter().cloned());
        out
    }

    /// Done once: each file as it was before and as it is now, for Undo.
    /// What is new first: cut short, nothing is left pointing at what was not made.
    pub fn write(&self) -> Result<Vec<FileChange>, String> {
        let mut out = Vec::new();
        for (path, text) in &self.made {
            vdir::write_item(path, text)?;
            out.push(FileChange { path: path.clone(), before: None, after: Some(text.clone()) });
        }
        for (path, text) in &self.written {
            let before = std::fs::read_to_string(path).ok();
            vdir::write_item(path, text)?;
            out.push(FileChange { path: path.clone(), before, after: Some(text.clone()) });
        }
        for (from, to, text) in &self.moved {
            let before = std::fs::read_to_string(from).ok();
            vdir::move_item(from, to)?;
            if before.as_deref() != Some(text.as_str()) {
                vdir::write_item(to, text)?;
            }
            out.push(FileChange { path: from.clone(), before, after: None });
            out.push(FileChange { path: to.clone(), before: None, after: Some(text.clone()) });
        }
        for path in &self.gone {
            let Ok(before) = std::fs::read_to_string(path) else { continue };
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
            out.push(FileChange { path: path.clone(), before: Some(before), after: None });
        }
        Ok(out)
    }
}

fn find<'a>(tasks: &'a [Task], uid: &str) -> Result<&'a Task, Refusal> {
    tasks.iter().find(|t| t.uid == uid).ok_or(Refusal::Gone)
}

fn text_of(task: &Task) -> Result<String, Refusal> {
    std::fs::read_to_string(&task.key).map_err(|_| Refusal::Gone)
}

/// The task and every step under it, however far down, each once.
fn family<'a>(tasks: &'a [Task], top: &'a Task) -> Vec<&'a Task> {
    let mut out: Vec<&Task> = Vec::new();
    let mut queue = vec![top];
    while let Some(task) = queue.pop() {
        if out.iter().any(|t| t.uid == task.uid) {
            continue;
        }
        out.push(task);
        queue.extend(tasks.iter().filter(|t| t.parent() == Some(task.uid.as_str())));
    }
    out
}

fn list_id(list: &Collection) -> String {
    format!("{}/{}", list.account, list.id)
}

/// The GitHub list: written by its sync only (docs/github.md); steps of an issue go in your own lists.
fn is_github(list: &Collection) -> bool {
    list.account == vdir::LOCAL && list.id == crate::github::LIST_ID
}

/// `uid` made a step of `bigger` (the module's rules). `lists`: the task
/// lists there are; `provider`: who keeps each. `confirm`: what the bigger
/// task's list would not keep was said, and the move goes on.
pub fn demote(tasks: &[Task], uid: &str, bigger: &str, lists: &[Collection], provider: &dyn Fn(&Collection) -> Provider, confirm: bool, now: &Zoned) -> Result<Change, Refusal> {
    let task = find(tasks, uid)?;
    let whole = find(tasks, bigger)?;
    if task.read_only {
        return Err(Refusal::ReadOnly);
    }
    if !task.status.is_open() {
        return Err(Refusal::Done);
    }
    // Never of itself, nor of one of its own steps, however far down.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut at = Some(whole);
    while let Some(up) = at {
        if up.uid == uid {
            return Err(Refusal::Loop);
        }
        if !seen.insert(up.uid.as_str()) {
            break;
        }
        at = up.parent().and_then(|p| tasks.iter().find(|t| t.uid == p));
    }
    if task.parent() == Some(bigger) {
        return Ok(Change::default());
    }
    // In the bigger task's list when it can be written (not GitHub's: your steps of an issue stay in yours).
    let own = lists.iter().find(|c| list_id(c) == task.list_id);
    let target = lists.iter().find(|c| !c.read_only && !is_github(c) && list_id(c) == whole.list_id).or(own);
    let keeper = target.map(provider).unwrap_or(Provider::Dav);
    let members = family(tasks, task);
    if keeper == Provider::GoogleTasks && (whole.parent().is_some() || members.len() > 1) {
        return Err(Refusal::OneLevel);
    }
    let moving = target.filter(|c| Path::new(&task.key).parent() != Some(c.dir.as_path()));
    if let Some(list) = moving {
        if members.iter().any(|t| t.read_only) {
            return Err(Refusal::ReadOnly);
        }
        let mut losses: Vec<&'static str> = members.iter().flat_map(|t| capabilities::task_losses(t, provider(list))).collect();
        losses.sort_unstable();
        losses.dedup();
        if !losses.is_empty() && !confirm {
            return Err(Refusal::Losses(losses));
        }
    }
    let zone = TimeZone::system();
    let edit = TaskEdit { parent: bigger.to_string(), ..TaskEdit::of(task) };
    let placed = tasks::next_in_sequence(tasks::steps_of(tasks, bigger).into_iter().filter(|t| t.uid != uid), now);
    let text = tasks::apply(&text_of(task)?, &edit, &zone, now).map_err(|_| Refusal::Gone)?;
    let text = tasks::set_sequence(&text, placed, now).map_err(|_| Refusal::Gone)?;
    let mut change = Change::default();
    match moving {
        Some(list) => {
            for member in members {
                let written = if member.uid == uid { text.clone() } else { text_of(member)? };
                change.moved.push((PathBuf::from(&member.key), list.dir.join(format!("{}.ics", vdir::new_name())), written));
            }
        }
        None => change.written.push((PathBuf::from(&task.key), text)),
    }
    Ok(change)
}

/// The step `uid` made a task of its own: its PARENT line out, and a CHILD
/// line its bigger task holds for it.
pub fn promote(tasks: &[Task], uid: &str, now: &Zoned) -> Result<Change, Refusal> {
    let task = find(tasks, uid)?;
    let bigger = task.parent().ok_or(Refusal::NotAStep)?;
    if task.read_only {
        return Err(Refusal::ReadOnly);
    }
    let zone = TimeZone::system();
    let mut change = Change::default();
    let edit = TaskEdit { parent: String::new(), ..TaskEdit::of(task) };
    change.written.push((PathBuf::from(&task.key), tasks::apply(&text_of(task)?, &edit, &zone, now).map_err(|_| Refusal::Gone)?));
    if let Ok(whole) = find(tasks, bigger)
        && whole.relations.iter().any(|r| r.kind == "CHILD" && r.uid == uid)
    {
        if whole.read_only {
            return Err(Refusal::ReadOnly);
        }
        let child = |l: &str| crate::lines::name(l) == "RELATED-TO" && tasks::relation_of(l).kind == "CHILD" && tasks::relation_of(l).uid == uid;
        change.written.push((PathBuf::from(&whole.key), tasks::remove_lines(&text_of(whole)?, child, now).map_err(|_| Refusal::Gone)?));
    }
    Ok(change)
}

/// A step's state from an item's mark: done, done; not needed, dropped.
fn status_of(mark: Mark) -> Status {
    match mark {
        Mark::Done => Status::Completed,
        Mark::Skipped => Status::Cancelled,
        Mark::Open => Status::NeedsAction,
    }
}

/// An item's mark from a task's state.
fn mark_of(status: Status) -> Mark {
    match status {
        Status::Completed => Mark::Done,
        Status::Cancelled => Mark::Skipped,
        _ => Mark::Open,
    }
}

/// The item on `line` of a task's checklist made a step of it: its words
/// its title, its detail its notes, after the other steps, its tick kept.
/// The new step's UID with the change.
pub fn item_to_step(tasks: &[Task], holder: &str, line: usize, now: &Zoned) -> Result<(Change, String), Refusal> {
    let whole = find(tasks, holder)?;
    if whole.read_only {
        return Err(Refusal::ReadOnly);
    }
    let item = checklist::items(&whole.notes).into_iter().find(|i| i.line == line).ok_or(Refusal::Stale)?;
    let zone = TimeZone::system();
    let notes = checklist::removed(&whole.notes, line).map_err(|_| Refusal::Stale)?;
    let mut change = Change::default();
    change.written.push((PathBuf::from(&whole.key), tasks::apply(&text_of(whole)?, &TaskEdit { notes, ..TaskEdit::of(whole) }, &zone, now).map_err(|_| Refusal::Gone)?));
    let edit = TaskEdit {
        title: item.text.clone(),
        notes: item.detail.join("\n"),
        parent: whole.uid.clone(),
        status: status_of(item.mark),
        sort_order: Some(tasks::next_in_sequence(tasks::steps_of(tasks, &whole.uid), now)),
        ..TaskEdit::default()
    };
    let uid = vdir::new_name();
    let dir = Path::new(&whole.key).parent().ok_or(Refusal::Gone)?;
    change.made.push((dir.join(format!("{uid}.ics")), tasks::new_task(&edit, &uid, &zone, now).map_err(|_| Refusal::Gone)?));
    Ok((change, uid))
}

/// What holds a checklist whose item becomes a task: a task, or an event's file.
pub enum Holder<'a> {
    Task(&'a Task),
    /// An event: its file, its text, its UID.
    Event { path: &'a Path, text: &'a str, uid: &'a str },
}

/// The item on `line` made a task of its own in `list`, tied to what held
/// it (a `related` link), its words its title, its detail its notes, its tick
/// kept; the item taken out of the list. The new task's UID with the change.
pub fn item_to_task(holder: &Holder, line: usize, list: &Collection, now: &Zoned) -> Result<(Change, String), Refusal> {
    let zone = TimeZone::system();
    let mut change = Change::default();
    let (notes, tie) = match holder {
        Holder::Task(task) => {
            if task.read_only {
                return Err(Refusal::ReadOnly);
            }
            (task.notes.clone(), task.uid.to_string())
        }
        Holder::Event { text, uid, .. } => (crate::agenda::edit_of_text(text, &zone).map(|e| e.notes).ok_or(Refusal::Gone)?, uid.to_string()),
    };
    let item = checklist::items(&notes).into_iter().find(|i| i.line == line).ok_or(Refusal::Stale)?;
    let without = checklist::removed(&notes, line).map_err(|_| Refusal::Stale)?;
    match holder {
        Holder::Task(task) => {
            let edit = TaskEdit { notes: without, ..TaskEdit::of(task) };
            change.written.push((PathBuf::from(&task.key), tasks::apply(&text_of(task)?, &edit, &zone, now).map_err(|_| Refusal::Gone)?));
        }
        Holder::Event { path, text, .. } => {
            let mut edit = crate::agenda::edit_of_text(text, &zone).ok_or(Refusal::Gone)?;
            edit.notes = without;
            change.written.push((path.to_path_buf(), crate::agenda::apply(text, &edit, &zone).map_err(|_| Refusal::Gone)?));
        }
    }
    let edit = TaskEdit {
        title: item.text.clone(),
        notes: item.detail.join("\n"),
        status: status_of(item.mark),
        links: vec![tasks::Link { uri: format!("uid:{tie}"), label: String::new(), rel: "related".into() }],
        ..TaskEdit::default()
    };
    let uid = vdir::new_name();
    change.made.push((list.dir.join(format!("{uid}.ics")), tasks::new_task(&edit, &uid, &zone, now).map_err(|_| Refusal::Gone)?));
    Ok((change, uid))
}

/// What a step holds beyond its words and its tick, as `loss-…` keys: what
/// folding it into the list would lose. `blocks`: its time blocks to come.
pub fn beyond_words(tasks: &[Task], step: &Task, blocks: &[PathBuf]) -> Vec<&'static str> {
    let mut lost = Vec::new();
    let mut add = |used: bool, what: &'static str| {
        if used {
            lost.push(what);
        }
    };
    let waited_on = tasks.iter().any(|t| t.uid != step.uid && t.status.is_open() && t.relations.iter().any(|r| r.uid == step.uid && r.kind != "PARENT" && r.kind != "CHILD"));
    add(step.relations.iter().any(|r| r.kind != "PARENT" && r.kind != "CHILD") || waited_on, "waits");
    add(tasks.iter().any(|t| t.parent() == Some(step.uid.as_str())), "steps");
    add(!blocks.is_empty(), "block");
    add(step.estimate > 0, "estimate");
    add(!step.start.is_empty(), "start");
    add(!step.due.is_empty(), "due");
    add(!step.repeat.is_empty(), "repeat");
    add(step.priority > 0, "priority");
    add(!step.kind.is_empty() || step.office_hours, "kind");
    add(!step.links.is_empty(), "links");
    add(!step.contacts.is_empty(), "contacts");
    add(!step.categories.is_empty(), "categories");
    add(!step.margins.is_empty(), "margins");
    add(step.demands != crate::demands::Demands::default(), "costs");
    lost
}

/// The step `uid` folded into its bigger task's checklist: an item at the end
/// of the list, its words, its notes its detail, its tick kept; the step and
/// its time blocks to come (`blocks`) taken out. What it holds beyond words
/// and a tick refuses it (`beyond_words`) unless `confirm`.
pub fn step_to_item(tasks: &[Task], uid: &str, blocks: &[PathBuf], confirm: bool, now: &Zoned) -> Result<Change, Refusal> {
    let step = find(tasks, uid)?;
    let whole = find(tasks, step.parent().ok_or(Refusal::NotAStep)?)?;
    if step.read_only || whole.read_only {
        return Err(Refusal::ReadOnly);
    }
    let lost = beyond_words(tasks, step, blocks);
    if !lost.is_empty() && !confirm {
        return Err(Refusal::Losses(lost));
    }
    let detail: Vec<String> = step.notes.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
    let notes = checklist::appended(&whole.notes, &step.title, mark_of(step.status), &detail);
    let zone = TimeZone::system();
    let mut change = Change::default();
    change.written.push((PathBuf::from(&whole.key), tasks::apply(&text_of(whole)?, &TaskEdit { notes, ..TaskEdit::of(whole) }, &zone, now).map_err(|_| Refusal::Gone)?));
    change.gone.push(PathBuf::from(&step.key));
    change.gone.extend(blocks.iter().cloned());
    Ok(change)
}

/// Whether a task waits for one still open: its own DEPENDS-ON, or another's
/// FINISHTOSTART or NEXT that points at it.
fn waits_open(tasks: &[Task], task: &Task) -> bool {
    let open = |uid: &str| tasks.iter().any(|t| t.uid == uid && t.status.is_open());
    task.depends_on().any(|r| open(&r.uid)) || tasks.iter().any(|t| t.status.is_open() && t.relations.iter().any(|r| matches!(r.kind.as_str(), "FINISHTOSTART" | "NEXT") && r.uid == task.uid))
}

/// The bigger tasks that close with `done`, once it is done (docs/tasks.md,
/// "Composed tasks"): its bigger task, when it has no estimate of its own,
/// no open wait, does not come back, can be written, and none of its steps is
/// left open, a step done among them (all dropped, nothing was done); then
/// the same further up. `tasks` as they were before `done` was.
pub fn closing_with<'a>(tasks: &'a [Task], done: &str) -> Vec<&'a Task> {
    let mut out: Vec<&Task> = Vec::new();
    let mut closed: BTreeSet<String> = BTreeSet::from([done.to_string()]);
    let mut at = tasks.iter().find(|t| t.uid == done).and_then(|t| t.parent()).and_then(|p| tasks.iter().find(|t| t.uid == p));
    while let Some(whole) = at {
        if !whole.status.is_open() || whole.estimate > 0 || !whole.repeat.is_empty() || whole.read_only || closed.contains(&whole.uid) {
            break;
        }
        let steps = tasks::steps_of(tasks, &whole.uid);
        let finished = |t: &Task| closed.contains(&t.uid) || !t.status.is_open();
        if steps.is_empty() || !steps.iter().all(|s| finished(s)) || !steps.iter().any(|s| closed.contains(&s.uid) || s.status == Status::Completed) {
            break;
        }
        if waits_open(tasks, whole) {
            break;
        }
        out.push(whole);
        closed.insert(whole.uid.clone());
        at = whole.parent().and_then(|p| tasks.iter().find(|t| t.uid == p));
    }
    out
}

/// Those bigger tasks marked done (`closing_with`).
pub fn close_with(tasks: &[Task], done: &str, now: &Zoned) -> Result<Change, Refusal> {
    let zone = TimeZone::system();
    let mut change = Change::default();
    for whole in closing_with(tasks, done) {
        change.written.push((PathBuf::from(&whole.key), tasks::set_status(&text_of(whole)?, Status::Completed, &zone, now).map_err(|_| Refusal::Gone)?));
    }
    Ok(change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::Relation;

    fn now() -> Zoned {
        "2026-10-25T10:00:00+01:00[Europe/Paris]".parse().unwrap()
    }

    /// A list on disk, its tasks written by `new_task`, read back.
    struct Desk {
        list: Collection,
        other: Collection,
    }

    impl Desk {
        fn new(name: &str) -> Desk {
            let list = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, &format!("{name} errands"), None, &["VTODO"]).unwrap();
            let other = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, &format!("{name} work"), None, &["VTODO"]).unwrap();
            Desk { list, other }
        }

        fn add(&self, list: &Collection, uid: &str, edit: TaskEdit) {
            vdir::write_item(&list.dir.join(format!("{uid}.ics")), &tasks::new_task(&TaskEdit { title: uid.into(), ..edit }, uid, &TimeZone::system(), &now()).unwrap()).unwrap();
        }

        fn tasks(&self) -> Vec<Task> {
            [&self.list, &self.other].iter().flat_map(|c| c.items().into_iter().filter_map(|p| tasks::read(&p, c, &TimeZone::system()))).collect()
        }

        fn task(&self, uid: &str) -> Task {
            self.tasks().into_iter().find(|t| t.uid == uid).unwrap()
        }

        fn lists(&self) -> Vec<Collection> {
            vec![self.list.clone(), self.other.clone()]
        }
    }

    fn step(parent: &str) -> TaskEdit {
        TaskEdit { parent: parent.into(), ..TaskEdit::default() }
    }

    fn plain(_: &Collection) -> Provider {
        Provider::Dav
    }

    #[test]
    fn made_a_step_and_a_task_of_its_own() {
        let desk = Desk::new("demote");
        desk.add(&desk.list, "doctor", TaskEdit::default());
        desk.add(&desk.list, "brush", step("doctor"));
        desk.add(&desk.list, "bus", TaskEdit { waits_for: vec!["brush".into()], projects: vec!["health".into()], ..TaskEdit::default() });
        desk.add(&desk.other, "bag", TaskEdit::default());
        desk.add(&desk.other, "bag-step", step("bag"));
        // A step of the doctor's visit: after its steps, its wait and project kept.
        let change = demote(&desk.tasks(), "bus", "doctor", &desk.lists(), &plain, false, &now()).unwrap();
        let undo = change.write().unwrap();
        let bus = desk.task("bus");
        assert_eq!(bus.parent(), Some("doctor"));
        assert!(bus.sequence() > desk.task("brush").sequence());
        assert_eq!((bus.depends_on().count(), bus.projects.clone()), (1, vec!["health".to_string()]));
        // Already: nothing. Of itself, of its own step: refused.
        assert!(demote(&desk.tasks(), "bus", "doctor", &desk.lists(), &plain, false, &now()).unwrap().is_empty());
        assert_eq!(demote(&desk.tasks(), "doctor", "doctor", &desk.lists(), &plain, false, &now()), Err(Refusal::Loop));
        assert_eq!(demote(&desk.tasks(), "doctor", "brush", &desk.lists(), &plain, false, &now()), Err(Refusal::Loop));
        // Undo: as it was.
        assert!(crate::agentwrites::Said { files: undo, ..Default::default() }.put_back());
        assert_eq!(desk.task("bus").parent(), None);
        // From another list: it moves there with its own step, through the journal.
        let change = demote(&desk.tasks(), "bag", "doctor", &desk.lists(), &plain, false, &now()).unwrap();
        assert_eq!(change.moved.len(), 2);
        change.write().unwrap();
        assert!(desk.other.items().is_empty(), "the family moved");
        assert_eq!(desk.task("bag").list_id, format!("local/{}", desk.list.id));
        assert_eq!(desk.task("bag-step").parent(), Some("bag"));
        // Its own again.
        promote(&desk.tasks(), "bag", &now()).unwrap().write().unwrap();
        assert_eq!(desk.task("bag").parent(), None);
        assert_eq!(promote(&desk.tasks(), "bag", &now()), Err(Refusal::NotAStep));
        // Done: refused.
        desk.add(&desk.list, "old", TaskEdit { status: Status::Completed, ..TaskEdit::default() });
        assert_eq!(demote(&desk.tasks(), "old", "doctor", &desk.lists(), &plain, false, &now()), Err(Refusal::Done));
    }

    #[test]
    fn a_google_list_keeps_one_level_and_says_what_it_loses() {
        let desk = Desk::new("google");
        let google = |c: &Collection| if c.id == desk.other.id { Provider::GoogleTasks } else { Provider::Dav };
        desk.add(&desk.other, "groceries", TaskEdit::default());
        desk.add(&desk.other, "groceries-step", step("groceries"));
        desk.add(&desk.list, "milk", TaskEdit { estimate: 10, ..TaskEdit::default() });
        desk.add(&desk.list, "eggs", TaskEdit::default());
        desk.add(&desk.list, "eggs-step", step("eggs"));
        // Two levels there: refused.
        assert_eq!(demote(&desk.tasks(), "milk", "groceries-step", &desk.lists(), &google, false, &now()), Err(Refusal::OneLevel));
        assert_eq!(demote(&desk.tasks(), "eggs", "groceries", &desk.lists(), &google, false, &now()), Err(Refusal::OneLevel));
        // What it would lose said first, then done when asked again.
        assert_eq!(demote(&desk.tasks(), "milk", "groceries", &desk.lists(), &google, false, &now()), Err(Refusal::Losses(vec!["estimate"])));
        demote(&desk.tasks(), "milk", "groceries", &desk.lists(), &google, true, &now()).unwrap().write().unwrap();
        assert_eq!(desk.task("milk").parent(), Some("groceries"));
    }

    #[test]
    fn an_item_made_a_step_and_folded_back() {
        let desk = Desk::new("items");
        desk.add(&desk.list, "doctor", TaskEdit { notes: "Bring:\n- [x] Carte Vitale\n  The new one.\n- [ ] Mutuelle card".into(), ..TaskEdit::default() });
        desk.add(&desk.list, "brush", step("doctor"));
        // The ticked item: a step done, after the others, its detail its notes.
        let (change, uid) = item_to_step(&desk.tasks(), "doctor", 1, &now()).unwrap();
        change.write().unwrap();
        let made = desk.task(&uid);
        assert_eq!((made.title.as_str(), made.status, made.notes.as_str(), made.parent()), ("Carte Vitale", Status::Completed, "The new one.", Some("doctor")));
        assert!(made.sequence() > desk.task("brush").sequence());
        assert_eq!(desk.task("doctor").notes, "Bring:\n- [ ] Mutuelle card");
        assert_eq!(item_to_step(&desk.tasks(), "doctor", 0, &now()).map(|_| ()), Err(Refusal::Stale), "a line that is no item");
        // Folded back: the same words, the same tick, the detail under it; the step gone.
        step_to_item(&desk.tasks(), &uid, &[], false, &now()).unwrap().write().unwrap();
        assert_eq!(desk.task("doctor").notes, "Bring:\n- [ ] Mutuelle card\n- [x] Carte Vitale\n  The new one.");
        assert!(!desk.tasks().iter().any(|t| t.uid == uid));
        // A step with more than words: what it would lose said.
        desk.add(&desk.list, "bus", TaskEdit { estimate: 25, due: "2026-10-29".into(), ..step("doctor") });
        assert_eq!(step_to_item(&desk.tasks(), "bus", &[], false, &now()), Err(Refusal::Losses(vec!["estimate", "due"])));
        assert_eq!(step_to_item(&desk.tasks(), "doctor", &[], false, &now()), Err(Refusal::NotAStep));
        // An item made a task of its own, tied to what held it.
        let (change, uid) = item_to_task(&Holder::Task(&desk.task("doctor")), 1, &desk.other, &now()).unwrap();
        change.write().unwrap();
        let task = desk.task(&uid);
        assert_eq!((task.title.as_str(), task.parent(), task.links[0].uri.as_str()), ("Mutuelle card", None, "uid:doctor"));
        assert_eq!(desk.task("doctor").notes, "Bring:\n- [x] Carte Vitale\n  The new one.");
    }

    #[test]
    fn a_bigger_task_closes_with_its_last_step() {
        let desk = Desk::new("close");
        desk.add(&desk.list, "doctor", TaskEdit::default());
        desk.add(&desk.list, "brush", step("doctor"));
        desk.add(&desk.list, "bus", step("doctor"));
        desk.add(&desk.list, "trip", TaskEdit::default());
        desk.add(&desk.list, "trip-doctor", TaskEdit { ..step("trip") });
        let uids = |found: Vec<&Task>| found.iter().map(|t| t.uid.clone()).collect::<Vec<_>>();
        // A step left open: nothing closes.
        assert!(closing_with(&desk.tasks(), "brush").is_empty());
        let mut tasks = desk.tasks();
        tasks.iter_mut().find(|t| t.uid == "brush").unwrap().status = Status::Completed;
        assert_eq!(uids(closing_with(&tasks, "bus")), ["doctor"]);
        // Further up: a step of a step.
        let mut nested = tasks.clone();
        nested.iter_mut().find(|t| t.uid == "doctor").unwrap().relations.push(Relation { kind: "PARENT".into(), uid: "trip".into(), gap: 0 });
        nested.iter_mut().find(|t| t.uid == "trip-doctor").unwrap().status = Status::Completed;
        assert_eq!(uids(closing_with(&nested, "bus")), ["doctor", "trip"]);
        // Its own estimate, an open wait, steps all dropped: it stays.
        let mut estimated = tasks.clone();
        estimated.iter_mut().find(|t| t.uid == "doctor").unwrap().estimate = 15;
        assert!(closing_with(&estimated, "bus").is_empty());
        let mut waiting = tasks.clone();
        waiting.iter_mut().find(|t| t.uid == "doctor").unwrap().relations.push(Relation { kind: "DEPENDS-ON".into(), uid: "trip".into(), gap: 0 });
        assert!(closing_with(&waiting, "bus").is_empty());
        let mut dropped = desk.tasks();
        dropped.iter_mut().find(|t| t.uid == "brush").unwrap().status = Status::Cancelled;
        assert_eq!(uids(closing_with(&dropped, "bus")), ["doctor"], "one step dropped, the other done: it is done");
        assert!(closing_with(&dropped, "nothing").is_empty());
        // Written: done.
        close_with(&tasks, "bus", &now()).unwrap().write().unwrap();
        assert_eq!(desk.task("doctor").status, Status::Completed);
    }
}

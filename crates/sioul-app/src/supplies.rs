// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Supplies in the window (docs/supplies.md): the page (its calm view, what
//! now is for, and its full list), a supply's card, the movements said by
//! hand ("Received…", "Count again…"), a task's supplies (its form's lines,
//! what it used, said in one press after a tick), "How many came in?" when a
//! task to resupply is done, and the upkeep after each re-plan: the finished
//! tasks applied (`supplies::settle`) and the tasks to resupply made.

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use serde::Serialize;
use sioul_core::supplies::{self, Change, Kind, List, Qty, Supply, Use};
use sioul_core::tasks::{Status, Task};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Where the supplies' list and movements are: the notes folder, or `records/` once moved.
fn root() -> Option<PathBuf> {
    load_config().records_root()
}

/// This device's id in the sharing: its own movements' file.
fn device() -> String {
    crate::notices::here_id()
}

/// The turns of repeating tasks done in this run, by UID: the stamp their
/// use was written under (`supplies::turn_done`), for "Change…" in the
/// status line. The task itself comes back open and keeps nothing of it.
static TURNS: Mutex<BTreeMap<String, i64>> = Mutex::new(BTreeMap::new());

/// A quantity in words, its unit after it: "12", "200 g".
fn amount(quantity: Qty, unit: &str) -> String {
    supplies::amount(quantity, unit, tr())
}

/// One supply as the page and its card show it.
#[derive(Debug, Clone, Default, Serialize)]
struct Row {
    id: String,
    name: String,
    category: String,
    area: String,
    unit: String,
    buy: String,
    sell: String,
    low_at: String,
    bought_by: String,
    project: String,
    project_title: String,
    archived: bool,
    stock: f64,
    planned: f64,
    free: f64,
    stock_text: String,
    planned_text: String,
    free_text: String,
    /// "low", "none left", or "": quiet words, never a colour.
    word: String,
    /// What is free, "4 planned", the word: one line.
    line: String,
}

fn row(supply: &Supply, standing: &supplies::Standing, projects: &[(String, String)]) -> Row {
    let unit = supply.unit.as_str();
    let planned_text = if standing.planned.is_zero() { String::new() } else { say("supplies-planned", &[("quantity", amount(standing.planned, unit))]) };
    let free_text = say("supplies-left", &[("quantity", amount(standing.free, unit))]);
    let word = if standing.none_left { tr().text("supplies-none-left", None) } else if standing.low { tr().text("supplies-low", None) } else { String::new() };
    let line = [free_text.clone(), planned_text.clone(), word.clone()].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" · ");
    Row {
        id: supply.id.clone(),
        name: supply.name_shown().to_string(),
        category: supply.category.clone(),
        area: supply.area_of().id(),
        unit: supply.unit.clone(),
        buy: supply.buy.clone(),
        sell: supply.sell.clone(),
        low_at: supply.low_at.map(|q| q.shown(tr())).unwrap_or_default(),
        bought_by: supply.bought_by.map(|q| q.shown(tr())).unwrap_or_default(),
        project: supply.project.clone(),
        project_title: projects.iter().find(|(id, _)| *id == supply.project).map(|(_, t)| t.clone()).unwrap_or_else(|| supply.project.clone()),
        archived: supply.archived,
        stock: standing.stock.as_f64(),
        planned: standing.planned.as_f64(),
        free: standing.free.as_f64(),
        stock_text: amount(standing.stock, unit),
        planned_text,
        free_text,
        word,
        line,
    }
}

/// The supplies of one category, in the order of their names.
#[derive(Debug, Clone, Default, Serialize)]
struct Group {
    category: String,
    supplies: Vec<Row>,
}

fn grouped(rows: Vec<Row>) -> Vec<Group> {
    let mut groups: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for row in rows {
        groups.entry(row.category.trim().to_lowercase()).or_default().push(row);
    }
    let other = tr().text("supplies-no-category", None);
    let mut out: Vec<Group> = groups
        .into_values()
        .map(|mut rows| {
            rows.sort_by_key(|r| r.name.to_lowercase());
            let category = if rows[0].category.trim().is_empty() { other.clone() } else { rows[0].category.clone() };
            Group { category, supplies: rows }
        })
        .collect();
    // Those without a category last.
    out.sort_by_key(|g| (g.supplies[0].category.trim().is_empty(), g.category.to_lowercase()));
    out
}

/// The page, as JSON.
#[derive(Debug, Default, Serialize)]
struct Page {
    /// A place for Sioul's lists here (a notes folder, or the lists moved).
    store: bool,
    problem: String,
    /// The person keeps supplies: some are listed, archived or not.
    keeps: bool,
    /// What now is for: its supplies, by category.
    calm: Vec<Group>,
    /// Every supply kept, by category, whatever the hour.
    all: Vec<Group>,
    archived: Vec<Row>,
    /// How many are kept for another time than now (the calm view's quiet line).
    elsewhere: usize,
    categories: Vec<String>,
    projects: Vec<Choice>,
}

#[derive(Debug, Clone, Default, Serialize)]
struct Choice {
    id: String,
    title: String,
}

/// The projects to choose from, by id and title.
fn projects() -> Vec<(String, String)> {
    crate::projects::store().map(|s| s.projects.iter().map(|p| (p.id.clone(), p.title.clone())).collect()).unwrap_or_default()
}

/// The list and every device's movements, read; a list that does not read said.
fn read_all(root: &std::path::Path) -> (List, supplies::Moves, String) {
    let (list, mut problem) = match List::load(root) {
        Ok(list) => (list, String::new()),
        Err(e) => (List::default(), say("supplies-unreadable", &[("what", e)])),
    };
    let moves = supplies::read_moves(root, &device());
    if problem.is_empty() && moves.own_unreadable {
        problem = say("supplies-moves-unreadable", &[("what", supplies::device_file(&device()))]);
    }
    (list, moves, problem)
}

/// The Supplies page (`SuppliesPage.qml`), as JSON.
pub(crate) fn page(shared: &Shared) -> String {
    let Some(root) = root() else { return json(&Page::default()) };
    let (list, moves, problem) = read_all(&root);
    let loaded = crate::work::loaded(shared);
    let latest = supplies::latest(&moves.lines);
    let plans = supplies::planned(&loaded.tasks);
    let projects = projects();
    let now = sioul_core::areafilter::hours_say(&crate::hours::mode_now());
    let mut calm = Vec::new();
    let mut all = Vec::new();
    let mut archived = Vec::new();
    let mut elsewhere = 0;
    for supply in &list.supplies {
        let row = row(supply, &supplies::standing(supply, &latest, &plans), &projects);
        if supply.archived {
            archived.push(row);
            continue;
        }
        if supply.area_of().meets(now) {
            calm.push(row.clone());
        } else {
            elsewhere += 1;
        }
        all.push(row);
    }
    archived.sort_by_key(|r| r.name.to_lowercase());
    let mut categories: Vec<String> = Vec::new();
    for supply in &list.supplies {
        if !supply.category.trim().is_empty() && !categories.iter().any(|c| c.to_lowercase() == supply.category.trim().to_lowercase()) {
            categories.push(supply.category.trim().to_string());
        }
    }
    categories.sort_by_key(|c| c.to_lowercase());
    json(&Page {
        store: true,
        problem,
        keeps: !list.supplies.is_empty(),
        calm: grouped(calm),
        all: grouped(all),
        archived,
        elsewhere,
        categories,
        projects: projects.into_iter().map(|(id, title)| Choice { id, title }).collect(),
    })
}

/// A line of a supply's history.
#[derive(Debug, Clone, Default, Serialize)]
struct Moved {
    id: String,
    kind: String,
    /// When it happened, in words.
    when: String,
    /// What it says: "Counted: 12", "Used: 1".
    line: String,
    /// The stock after it.
    after: String,
    task: String,
    task_title: String,
}

/// A task tied to a supply: an open task that plans it, or the task to resupply.
#[derive(Debug, Clone, Default, Serialize)]
struct Tied {
    uid: String,
    title: String,
    quantity: String,
    day: String,
}

/// A supply's card, as JSON.
#[derive(Debug, Default, Serialize)]
struct Card {
    #[serde(flatten)]
    row: Row,
    /// The open tasks that plan to use it.
    planned_tasks: Vec<Tied>,
    /// Its movements, newest first, each saying why.
    history: Vec<Moved>,
    /// A task to resupply done whose arrival no device said: asked once ("How many came in?").
    resupplied: Option<Tied>,
    /// Its task to resupply open now.
    resupply: Option<Tied>,
    /// A stock under zero, said as it is, with "Count again…".
    under_zero: bool,
}

/// A supply's card (`SupplyCard.qml`), as JSON; "null" when it is gone.
pub(crate) fn card(shared: &Shared, id: &str) -> String {
    let Some(root) = root() else { return "null".into() };
    let (list, moves, _) = read_all(&root);
    let Some(supply) = list.get(id) else { return "null".into() };
    let loaded = crate::work::loaded(shared);
    let latest = supplies::latest(&moves.lines);
    let plans = supplies::planned(&loaded.tasks);
    let standing = supplies::standing(supply, &latest, &plans);
    let projects = projects();
    let zone = sioul_core::zone::current();
    let today = sioul_core::zone::now().date();
    let day = |at: i64| jiff::Timestamp::from_second(at).map(|t| tr().day_in(t.to_zoned(zone.clone()).date(), today)).unwrap_or_default();
    let title_of = |uid: &str| loaded.tasks.iter().find(|t| t.uid == uid).map(|t| t.title.clone()).unwrap_or_default();
    let planned_tasks = loaded
        .tasks
        .iter()
        .filter(|t| t.status.is_open())
        .filter_map(|t| t.supplies.iter().find(|u| u.id == supply.id).map(|u| Tied { uid: t.uid.clone(), title: t.title.clone(), quantity: amount(u.quantity, &supply.unit), day: String::new() }))
        .collect();
    // The history in the order things happened, the stock after each; shown newest first.
    let mut stock = Qty::ZERO;
    let mut history: Vec<Moved> = Vec::new();
    for line in supplies::lines_of(&latest, &supply.id) {
        stock = match line.kind {
            Kind::Count => line.quantity,
            Kind::Received => stock + line.quantity,
            Kind::Used => stock - line.quantity,
        };
        // A use given back (a task set back to open) said nothing happened: left out.
        if line.quantity.is_zero() && line.kind != Kind::Count {
            continue;
        }
        let key = match line.kind {
            Kind::Count => "supplies-moved-count",
            Kind::Received => "supplies-moved-received",
            Kind::Used => "supplies-moved-used",
        };
        history.push(Moved {
            id: line.id.clone(),
            kind: format!("{:?}", line.kind).to_lowercase(),
            when: day(line.when()),
            line: say(key, &[("quantity", amount(line.quantity, &supply.unit))]),
            after: amount(stock, &supply.unit),
            task: line.task.clone(),
            task_title: title_of(&line.task),
        });
    }
    history.reverse();
    let resupply = loaded.tasks.iter().find(|t| t.status.is_open() && t.resupply == supply.id).map(|t| Tied { uid: t.uid.clone(), title: t.title.clone(), ..Tied::default() });
    // A task to resupply done (in another program, its supply not saying how many it brings) that no device said the arrival of.
    let resupplied = loaded
        .tasks
        .iter()
        .filter(|t| t.status == Status::Completed && t.resupply == supply.id)
        .filter_map(|t| t.completed.map(|stamp| (t, stamp)))
        .filter(|(t, stamp)| !latest.contains_key(&supplies::received_id(&t.uid, &supply.id, *stamp)))
        .max_by_key(|(_, stamp)| *stamp)
        .map(|(t, stamp)| Tied { uid: t.uid.clone(), title: t.title.clone(), day: day(stamp), quantity: String::new() });
    json(&Card { row: row(supply, &standing, &projects), planned_tasks, history, resupplied, resupply, under_zero: standing.stock < Qty::ZERO })
}

/// A supply saved from its card's fields (`supplies::Change`, JSON), new
/// when `id` is empty; with a first count for a new one (`first`, as
/// typed, "" for none). Returns {id} or {error}.
pub(crate) fn save(id: &str, change: &str, first: &str) -> String {
    let answer = |id: String, error: String| json(&serde_json::json!({ "id": id, "error": error }));
    let Some(root) = root() else { return answer(String::new(), tr().text("error-no-store", None)) };
    let change: Change = match serde_json::from_str(change) {
        Ok(change) => change,
        Err(e) => return answer(String::new(), e.to_string()),
    };
    // A first count that does not read is said before anything is made.
    let first = first.trim();
    let count = if first.is_empty() { None } else { Some(Qty::parse(first).filter(|q| *q >= Qty::ZERO)) };
    if count == Some(None) {
        return answer(String::new(), say("supplies-number-unreadable", &[("what", first.to_string())]));
    }
    let saved = match supplies::change(&root, id, &change) {
        Ok(saved) => saved,
        Err(refused) => return answer(String::new(), refused.sentence(tr())),
    };
    if let Some(Some(count)) = count {
        let now = sioul_core::zone::now().timestamp().as_second();
        if let Err(e) = supplies::record(&root, &device(), &[supplies::by_hand(&saved, Kind::Count, count, now)], &[]) {
            return answer(saved, e);
        }
    }
    answer(saved, String::new())
}

/// "Received…" (`kind` "received": so many more) or "Count again…" ("count":
/// there are so many), said by hand on a supply's card; returns what went
/// wrong, else "".
pub(crate) fn moved(qt: &QtThread, shared: &Arc<Shared>, id: &str, kind: &str, quantity: &str) -> String {
    let Some(root) = root() else { return tr().text("error-no-store", None) };
    let Some(quantity) = Qty::parse(quantity).filter(|q| *q >= Qty::ZERO) else { return say("supplies-number-unreadable", &[("what", quantity.trim().to_string())]) };
    let kind = if kind == "count" { Kind::Count } else { Kind::Received };
    let now = sioul_core::zone::now().timestamp().as_second();
    match supplies::record(&root, &device(), &[supplies::by_hand(id, kind, quantity, now)], &[]) {
        Ok(()) => {
            // Low or no longer: a task to resupply may be due (`upkeep`, after the plan).
            crate::work::show_work(qt, shared);
            String::new()
        }
        Err(e) => e,
    }
}

/// One line of a task's supplies, as the panel and the status line show it.
#[derive(Debug, Clone, Default, Serialize)]
struct UseRow {
    id: String,
    name: String,
    unit: String,
    quantity: f64,
    text: String,
}

fn use_rows(uses: &[Use], list: &List) -> Vec<UseRow> {
    uses.iter()
        .map(|u| {
            let supply = list.get(&u.id);
            UseRow { id: u.id.clone(), name: supply.map_or(u.id.as_str(), |s| s.name_shown()).to_string(), unit: supply.map(|s| s.unit.clone()).unwrap_or_default(), quantity: u.quantity.as_f64(), text: supplies::named(u, list, tr()) }
        })
        .collect()
}

/// A task's supplies, as JSON: whether the person keeps supplies, the
/// supplies to choose from (those kept, not archived), what it plans, what
/// it used (done, or a repeating task's turn done in this run), and, for a
/// task to resupply, its supply and what it is bought by.
#[derive(Debug, Default, Serialize)]
struct OfTask {
    keeps: bool,
    choices: Vec<UseRow>,
    planned: Vec<UseRow>,
    used: Vec<UseRow>,
    done: bool,
    /// A repeating task's turn done in this run: what it used is its turn's.
    turn: bool,
    /// "Used 1 Stamps, 1 Envelopes." for the status line; "" when nothing.
    said: String,
    resupply: Option<UseRow>,
}

fn of_task_value(task: &Task) -> OfTask {
    let Some(root) = root() else { return OfTask::default() };
    let list = List::load(&root).unwrap_or_default();
    let done = task.status == Status::Completed;
    let turn = TURNS.lock().ok().and_then(|t| t.get(&task.uid).copied());
    let used: Vec<Use> = match (done, turn) {
        (true, _) => supplies::used_by(task).to_vec(),
        (false, Some(stamp)) => {
            let moves = supplies::read_moves(&root, &device());
            supplies::latest(&moves.lines).into_values().filter(|m| m.task == task.uid && m.kind == Kind::Used && m.when() == stamp).map(|m| Use { id: m.supply, quantity: m.quantity }).collect()
        }
        _ => Vec::new(),
    };
    let said = supplies::said_used(&used, &list, tr());
    let resupply = list.get(&task.resupply).map(|s| UseRow { id: s.id.clone(), name: s.name_shown().to_string(), unit: s.unit.clone(), quantity: s.bought_by.map_or(0.0, Qty::as_f64), text: s.bought_by.map(|q| q.shown(tr())).unwrap_or_default() });
    OfTask {
        keeps: !list.supplies.is_empty(),
        choices: list.supplies.iter().filter(|s| !s.archived).map(|s| UseRow { id: s.id.clone(), name: s.name_shown().to_string(), unit: s.unit.clone(), quantity: 0.0, text: s.name_shown().to_string() }).collect(),
        planned: use_rows(&task.supplies, &list),
        used: use_rows(&used, &list),
        done,
        turn: turn.is_some() && !done,
        said,
        resupply,
    }
}

/// A task's supplies (`OfTask`), for its panel.
pub(crate) fn of_task(task: &Task) -> serde_json::Value {
    serde_json::to_value(of_task_value(task)).unwrap_or_default()
}

/// A task's supplies by its UID, for the status line after a tick; "null" when it is gone.
pub(crate) fn task_supplies(shared: &Shared, uid: &str) -> String {
    match crate::work::find_task(shared, uid).ok().and_then(|t| crate::work::from_its_file(&t)) {
        Some(task) => json(&of_task_value(&task)),
        None => "null".into(),
    }
}

/// What a task used, said again (JSON [{id, quantity}]): on a task done, its
/// lines of what it used written anew and the movements following; on a
/// repeating task's turn done in this run, that turn's movements. Returns
/// what went wrong, else "".
pub(crate) fn set_used(qt: &QtThread, shared: &Arc<Shared>, uid: &str, uses: &str) -> String {
    let Some(root) = root() else { return tr().text("error-no-store", None) };
    let uses: Vec<Use> = match serde_json::from_str::<Vec<Use>>(uses) {
        Ok(uses) => uses.into_iter().filter(|u| supplies::is_id(&u.id) && u.quantity >= Qty::ZERO).collect(),
        Err(e) => return e.to_string(),
    };
    let Ok(task) = crate::work::find_task(shared, uid) else { return tr().text("task-gone", None) };
    let now = sioul_core::zone::now();
    let turn = TURNS.lock().ok().and_then(|t| t.get(uid).copied());
    if task.status != Status::Completed {
        return match turn {
            Some(stamp) => supplies::turn_used(&root, &device(), uid, stamp, &uses, now.timestamp().as_second()).map(|()| crate::work::show_work(qt, shared)).err().unwrap_or_default(),
            None => tr().text("supplies-not-done", None),
        };
    }
    if let Err(e) = crate::work::change_task(qt, shared, uid, |text| supplies::with_used(text, &uses, &now)) {
        return e;
    }
    // The movements follow at once, before the plan is made again.
    if let Some(fresh) = crate::work::from_its_file(&task) {
        let _ = supplies::settle(&root, &device(), std::slice::from_ref(&fresh), now.timestamp().as_second());
    }
    String::new()
}

/// "How many came in?" answered for a task to resupply done: its arrival
/// written (`supplies::arrived`). Returns what went wrong, else "".
pub(crate) fn arrived(qt: &QtThread, shared: &Arc<Shared>, uid: &str, quantity: &str) -> String {
    let Some(root) = root() else { return tr().text("error-no-store", None) };
    let Some(quantity) = Qty::parse(quantity).filter(|q| *q >= Qty::ZERO) else { return say("supplies-number-unreadable", &[("what", quantity.trim().to_string())]) };
    let Some(task) = crate::work::find_task(shared, uid).ok().and_then(|t| crate::work::from_its_file(&t)) else { return tr().text("task-gone", None) };
    match supplies::arrived(&root, &device(), &task, quantity, sioul_core::zone::now().timestamp().as_second()) {
        Ok(()) => {
            crate::work::show_work(qt, shared);
            String::new()
        }
        Err(_) => tr().text("supplies-not-done", None),
    }
}

/// A task just finished in the window (`work::set_status`), `before` as it
/// was: a repeating one's turn written now, its stamp kept for "Change…";
/// any other applied at once from its file (the re-plan's `upkeep` would
/// too, a moment later). Returns what the status line adds: "Used 1 Stamps."
pub(crate) fn finished(before: &Task, after: Option<&Task>) -> String {
    let (Some(root), Some(after)) = (root(), after) else { return String::new() };
    let now = sioul_core::zone::now().timestamp().as_second();
    let turn = !before.repeat.is_empty() && after.status != Status::Completed && !before.supplies.is_empty();
    match supplies::after_status(&root, &device(), before, after, true, now) {
        Ok(_) if turn => {
            if let Ok(mut turns) = TURNS.lock() {
                turns.insert(before.uid.clone(), now);
            }
            of_task_value(after).said
        }
        Ok(_) => of_task_value(after).said,
        Err(e) => {
            eprintln!("sioul: supplies: {e}");
            String::new()
        }
    }
}

/// After each re-plan, on its thread: the finished tasks applied
/// (`supplies::settle`, only what is missing written, in this device's own
/// file), then the tasks to resupply made, each said once; their account
/// told, and the plan made again when one was made.
pub(crate) fn upkeep(qt: &QtThread, shared: &Arc<Shared>, tasks: &[Task]) {
    let Some(root) = root() else { return };
    let device = device();
    let now = sioul_core::zone::now();
    if let Err(e) = supplies::settle(&root, &device, tasks, now.timestamp().as_second()) {
        eprintln!("sioul: supplies: {e}");
    }
    let config = load_config();
    // No list at all: one on this device, as Health's errands get theirs.
    let list = match supplies::resupply_list(&config) {
        Some(list) => list,
        None if supplies::List::load(&root).is_ok_and(|l| l.supplies.iter().any(|s| s.low_at.is_some())) => match sioul_core::vdir::create(sioul_core::vdir::Kind::Calendars, sioul_core::vdir::LOCAL, &tr().text("supplies-list-name", None), None, &["VTODO"]) {
            Ok(list) => list,
            Err(_) => return,
        },
        None => return,
    };
    let title = |supply: &Supply| say("supplies-resupply-title", &[("name", supply.name_shown().to_string())]);
    match supplies::make_resupplies(&root, &device, tasks, &list, &title, &sioul_core::zone::current(), &now) {
        Ok(made) if !made.is_empty() => {
            crate::pim::nudge(shared, &list.account);
            for uid in &made {
                let named = sioul_core::tasks::read(&list.dir.join(format!("{uid}.ics")), &list, &sioul_core::zone::current()).map(|t| t.title).unwrap_or_default();
                tell(qt, shared, say("supplies-resupply-made", &[("title", named), ("list", list.name.clone())]));
            }
            crate::work::show_work(qt, shared);
        }
        Ok(_) => {}
        Err(e) => eprintln!("sioul: supplies: {e}"),
    }
}

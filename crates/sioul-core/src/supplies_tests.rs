// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Supplies' tests (docs/supplies.md): the stock from movements, free and
//! low, applying a finished task once on any number of devices, the task to
//! resupply made once, the list and the files read leniently.

use super::*;
use crate::tasks::{TaskEdit, task_of_text};

/// A folder of its own for one test.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sioul-supplies-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn q(text: &str) -> Qty {
    Qty::parse(text).unwrap()
}

fn line(id: &str, at: i64, supply: &str, kind: Kind, quantity: &str) -> Move {
    Move { id: id.into(), at, on: None, supply: supply.into(), kind, quantity: q(quantity), task: String::new(), device: String::new() }
}

fn stock(root: &Path, device: &str, supply: &str) -> Qty {
    let moves = read_moves(root, device);
    stock_of(&lines_of(&latest(&moves.lines), supply))
}

/// A list of three supplies, invented: stamps, envelopes, gift cards.
fn three(root: &Path) {
    std::fs::write(
        file_in(root),
        "# Invented supplies.\n[[supply]]\nid = \"stamps\"\nname = \"Stamps\"\ncategory = \"Post\"\narea = \"work\"\nlow_at = 10\nbought_by = 50\n\n[[supply]]\nid = \"envelopes\"\nname = \"Envelopes\"\ncategory = \"Post\"\nlow_at = 10\n\n[[supply]]\nid = \"cards\"\nname = \"Gift cards\"\ncategory = \"Shop\"\nlow_at = 5\n",
    )
    .unwrap();
}

/// A task done at `stamp`, planning `uses`.
fn done(uid: &str, stamp: i64, uses: &[(&str, &str)]) -> Task {
    Task { uid: uid.into(), title: uid.into(), status: Status::Completed, completed: Some(stamp), modified: stamp, supplies: uses.iter().map(|(id, n)| Use { id: id.to_string(), quantity: q(n) }).collect(), ..Task::default() }
}

/// An open task planning `uses`.
fn open(uid: &str, uses: &[(&str, &str)]) -> Task {
    Task { uid: uid.into(), title: uid.into(), supplies: uses.iter().map(|(id, n)| Use { id: id.to_string(), quantity: q(n) }).collect(), ..Task::default() }
}

#[test]
fn quantities_keep_what_was_typed_and_show_no_trailing_zeros() {
    assert_eq!(q("12").text(), "12");
    assert_eq!(q("1,5").text(), "1.5");
    assert_eq!(q(" 1 000 ").text(), "1000");
    assert_eq!(q("0.250").text(), "0.25");
    assert_eq!(q("−2").text(), "-2");
    assert_eq!((q("0.1") + q("0.2")).text(), "0.3", "no float's drift");
    assert!(Qty::parse("twelve").is_none() && Qty::parse("").is_none() && Qty::parse("1.2.3").is_none());
    let tr = crate::i18n::Translator::new("fr");
    assert_eq!(q("1.5").shown(&tr), "1,5");
}

/// The stock is the latest count, plus what came in after it, less what
/// was used after it; of several lines of one id the latest says.
#[test]
fn the_stock_comes_from_its_movements() {
    let root = folder("stock");
    record(&root, "desk", &[line("c1", 100, "stamps", Kind::Count, "12"), line("r1", 200, "stamps", Kind::Received, "50"), line("u1", 300, "stamps", Kind::Used, "3")], &[]).unwrap();
    assert_eq!(stock(&root, "desk", "stamps").text(), "59");
    // The same use said again later, at 2: the latest word wins, the file keeps it once.
    record(&root, "desk", &[line("u1", 400, "stamps", Kind::Used, "2")], &[]).unwrap();
    assert_eq!(stock(&root, "desk", "stamps").text(), "60");
    let text = std::fs::read_to_string(moves_dir(&root).join(device_file("desk"))).unwrap();
    assert_eq!(text.matches("\"u1\"").count(), 1, "{text}");
    // A quantity of 0: used nothing after all.
    record(&root, "desk", &[line("u1", 500, "stamps", Kind::Used, "0")], &[]).unwrap();
    assert_eq!(stock(&root, "desk", "stamps").text(), "62");
    // A later count says what there is: what came before it counts no more.
    record(&root, "desk", &[line("c2", 600, "stamps", Kind::Count, "7"), line("u2", 700, "stamps", Kind::Used, "1")], &[]).unwrap();
    assert_eq!(stock(&root, "desk", "stamps").text(), "6");
    // Another device's file read with this one's: its use counts, and a line it said later on an id wins.
    record(&root, "phone", &[line("u3", 800, "stamps", Kind::Used, "2"), line("u2", 900, "stamps", Kind::Used, "3")], &[]).unwrap();
    assert_eq!(stock(&root, "desk", "stamps").text(), "2");
    assert!(!moves_dir(&root).join(device_file("desk")).with_extension("toml.new").exists());
    let _ = std::fs::remove_dir_all(&root);
}

/// A correction said after a count counts where the use was: the count already holds it.
#[test]
fn a_use_put_right_after_a_count_counts_where_it_was() {
    let root = folder("after-count");
    let used = |at: i64, n: &str| Move { on: Some(150), ..line("u1", at, "cards", Kind::Used, n) };
    record(&root, "desk", &[line("c1", 100, "cards", Kind::Count, "10"), used(150, "1"), line("c2", 200, "cards", Kind::Count, "9")], &[]).unwrap();
    record(&root, "desk", &[used(300, "2")], &[]).unwrap();
    assert_eq!(stock(&root, "desk", "cards").text(), "9");
    let _ = std::fs::remove_dir_all(&root);
}

/// Free is the stock less what open tasks plan; low and none left are said of it.
#[test]
fn low_is_said_of_what_is_free() {
    let root = folder("free");
    three(&root);
    record(&root, "desk", &[line("c1", 100, "cards", Kind::Count, "8")], &[]).unwrap();
    let list = List::load(&root).unwrap();
    let moves = read_moves(&root, "desk");
    let cards = list.get("cards").unwrap();
    let tasks: Vec<Task> = (0..3).map(|n| open(&format!("sale-{n}"), &[("cards", "1")])).collect();
    let now = standing(cards, &latest(&moves.lines), &planned(&tasks));
    assert_eq!((now.stock.text(), now.planned.text(), now.free.text(), now.low, now.none_left), ("8".into(), "3".into(), "5".into(), true, false));
    // Ten sales come in with eight cards: none left, before any card is used.
    let ten: Vec<Task> = (0..10).map(|n| open(&format!("sale-{n}"), &[("cards", "1")])).collect();
    let now = standing(cards, &latest(&moves.lines), &planned(&ten));
    assert!(now.none_left && !now.low && now.free == Qty::whole(-2));
    // A task done plans nothing more; a supply without `low_at` is never low.
    let mut tasks = tasks;
    tasks[0].status = Status::Completed;
    assert_eq!(planned(&tasks).get("cards").copied(), Some(q("2")));
    let envelopes = Supply { low_at: None, ..list.get("envelopes").unwrap().clone() };
    assert!(!standing(&envelopes, &BTreeMap::new(), &BTreeMap::new()).low);
    let _ = std::fs::remove_dir_all(&root);
}

/// A task done is applied once, whichever devices read it done; changing
/// what it used writes one line; set back to open, its supplies come back.
#[test]
fn a_finished_task_is_applied_once_on_every_device() {
    let root = folder("settle");
    three(&root);
    record(&root, "desk", &[line("c1", 100, "stamps", Kind::Count, "12"), line("c2", 100, "cards", Kind::Count, "12")], &[]).unwrap();
    // Finished elsewhere (no line of what it used): it used what it planned.
    let sale = done("sale", 1000, &[("stamps", "1"), ("cards", "1")]);
    let written = settle(&root, "desk", std::slice::from_ref(&sale), 1100).unwrap();
    assert_eq!(written.len(), 2);
    assert!(settle(&root, "desk", std::slice::from_ref(&sale), 1200).unwrap().is_empty(), "nothing twice");
    // The phone reads it done too: the same ids, nothing to write, one use.
    assert!(settle(&root, "phone", std::slice::from_ref(&sale), 1300).unwrap().is_empty());
    assert_eq!((stock(&root, "phone", "stamps").text(), stock(&root, "phone", "cards").text()), ("11".into(), "11".into()));
    // Two devices reading it before either wrote: two lines of one id, one use.
    let other = done("other", 2000, &[("stamps", "2")]);
    settle(&root, "laptop", std::slice::from_ref(&other), 2100).unwrap();
    record(&root, "tablet", &[Move { id: used_id("other", "stamps", 2000), on: Some(2000), task: "other".into(), ..line("x", 2100, "stamps", Kind::Used, "2") }], &[]).unwrap();
    assert_eq!(stock(&root, "desk", "stamps").text(), "9");
    // Put right: a second stamp used on the sale.
    let mut righted = sale.clone();
    righted.used_said = true;
    righted.supplies_used = vec![Use { id: "stamps".into(), quantity: q("2") }, Use { id: "cards".into(), quantity: q("1") }];
    righted.modified = 3000;
    assert_eq!(settle(&root, "desk", std::slice::from_ref(&righted), 3000).unwrap().len(), 1);
    assert_eq!(stock(&root, "desk", "stamps").text(), "8");
    // A device that read the sale before it was put right never says it back.
    assert!(settle(&root, "phone", std::slice::from_ref(&sale), 3100).unwrap().is_empty());
    assert_eq!(stock(&root, "phone", "stamps").text(), "8");
    // Set back to open: its supplies come back, by lines at 0.
    let reopened = Task { status: Status::NeedsAction, completed: None, modified: 4000, ..righted.clone() };
    assert_eq!(settle(&root, "desk", std::slice::from_ref(&reopened), 4000).unwrap().len(), 2);
    assert_eq!((stock(&root, "desk", "stamps").text(), stock(&root, "desk", "cards").text()), ("10".into(), "12".into()));
    // Done again later: a new stamp, a new use.
    let again = done("sale", 5000, &[("stamps", "1"), ("cards", "1")]);
    settle(&root, "desk", std::slice::from_ref(&again), 5000).unwrap();
    assert_eq!(stock(&root, "desk", "stamps").text(), "9");
    // A supply taken off it while done: given back.
    let fewer = Task { used_said: true, supplies_used: vec![Use { id: "stamps".into(), quantity: q("1") }], modified: 6000, ..again.clone() };
    settle(&root, "desk", std::slice::from_ref(&fewer), 6000).unwrap();
    assert_eq!(stock(&root, "desk", "cards").text(), "12");
    let _ = std::fs::remove_dir_all(&root);
}

/// Nothing of supplies anywhere: nothing read, nothing written.
#[test]
fn no_supplies_writes_nothing() {
    let root = folder("none");
    assert!(settle(&root, "desk", &[Task { uid: "plain".into(), status: Status::Completed, completed: Some(1), ..Task::default() }], 2).unwrap().is_empty());
    assert!(!moves_dir(&root).exists());
    let _ = std::fs::remove_dir_all(&root);
}

/// A repeating task's turn is written when done, and never given back by
/// `settle`, which cannot see the turn in the task.
#[test]
fn a_turn_of_a_repeating_task_is_written_when_done() {
    let root = folder("turn");
    three(&root);
    record(&root, "desk", &[line("c1", 100, "stamps", Kind::Count, "12")], &[]).unwrap();
    let weekly = Task { repeat: "weekly".into(), ..open("weekly", &[("stamps", "2")]) };
    turn_done(&root, "desk", &weekly, 1000, 1000).unwrap();
    assert!(settle(&root, "desk", std::slice::from_ref(&weekly), 1100).unwrap().is_empty());
    assert_eq!(stock(&root, "desk", "stamps").text(), "10");
    turn_used(&root, "desk", "weekly", 1000, &[Use { id: "stamps".into(), quantity: q("3") }], 1200).unwrap();
    assert_eq!(stock(&root, "desk", "stamps").text(), "9");
    let _ = std::fs::remove_dir_all(&root);
}

/// The task to resupply: its UID the same on two devices, made once,
/// never again while one is open nor once made (even deleted), and again
/// after the next count or arrival.
#[test]
fn the_task_to_resupply_is_made_once() {
    let root = folder("resupply");
    three(&root);
    record(&root, "desk", &[line("c1", 100, "stamps", Kind::Count, "12"), line("c2", 100, "envelopes", Kind::Count, "40"), line("c3", 100, "cards", Kind::Count, "40")], &[]).unwrap();
    let list = List::load(&root).unwrap();
    let sales: Vec<Task> = (0..3).map(|n| open(&format!("sale-{n}"), &[("stamps", "1")])).collect();
    let desk = to_resupply(&list, &read_moves(&root, "desk"), &sales);
    let phone = to_resupply(&list, &read_moves(&root, "phone"), &sales);
    assert_eq!(desk.iter().map(|r| r.supply.id.as_str()).collect::<Vec<_>>(), ["stamps"]);
    assert_eq!(desk[0].uid, phone[0].uid, "the same UID on every device");
    assert_eq!(desk[0].uid, resupply_uid("stamps", "c1"));
    // Its file: a task to resupply, tagged, for the supply's area, marked with its id.
    let zone = jiff::tz::TimeZone::UTC;
    let now = jiff::Timestamp::from_second(1000).unwrap().to_zoned(zone.clone());
    let text = resupply_text(&desk[0], "Resupply: Stamps", &zone, &now).unwrap();
    let task = task_of_text(&text, &zone).unwrap();
    assert_eq!((task.uid.as_str(), task.title.as_str(), task.resupply.as_str(), task.area.as_str(), task.categories.clone()), (desk[0].uid.as_str(), "Resupply: Stamps", "stamps", "work", vec![TAG.to_string()]));
    // One is open: no second. Made, then deleted: not again.
    let mut with = sales.clone();
    with.push(Task { resupply: "stamps".into(), uid: "another".into(), ..Task::default() });
    assert!(to_resupply(&list, &read_moves(&root, "desk"), &with).is_empty());
    record(&root, "phone", &[], &[Made { task: desk[0].uid.clone(), supply: "stamps".into(), at: 1000 }]).unwrap();
    assert!(to_resupply(&list, &read_moves(&root, "desk"), &sales).is_empty(), "made on another device");
    // Done: what it brought comes in (bought by 50) once, by whichever device reads it first.
    let resupplied = Task { resupply: "stamps".into(), ..done(&desk[0].uid, 2000, &[]) };
    let mut tasks = sales.clone();
    tasks.push(resupplied.clone());
    let written = settle(&root, "desk", &tasks, 2100).unwrap();
    assert_eq!(written.iter().map(|m| (m.kind, m.quantity.text())).collect::<Vec<_>>(), [(Kind::Received, "50".to_string())]);
    assert!(settle(&root, "phone", &tasks, 2200).unwrap().is_empty());
    // "How many came in?" 40: the answer replaces it, and nothing puts it back.
    arrived(&root, "desk", &resupplied, q("40"), 2300).unwrap();
    assert!(settle(&root, "phone", &tasks, 2400).unwrap().is_empty());
    assert_eq!(stock(&root, "desk", "stamps").text(), "52");
    let _ = std::fs::remove_dir_all(&root);
}

/// An archived supply keeps its history, and no task to resupply is made for it.
#[test]
fn an_archived_supply_keeps_its_history_and_asks_for_nothing() {
    let root = folder("archived");
    three(&root);
    record(&root, "desk", &[line("c1", 100, "stamps", Kind::Count, "1")], &[]).unwrap();
    change(&root, "stamps", &Change { archived: Some(true), ..Change::default() }).unwrap();
    let list = List::load(&root).unwrap();
    assert!(list.get("stamps").unwrap().archived);
    assert!(to_resupply(&list, &read_moves(&root, "desk"), &[]).iter().all(|r| r.supply.id != "stamps"));
    assert_eq!(stock(&root, "desk", "stamps").text(), "1");
    change(&root, "stamps", &Change { archived: Some(false), ..Change::default() }).unwrap();
    assert!(!List::load(&root).unwrap().get("stamps").unwrap().archived);
    let _ = std::fs::remove_dir_all(&root);
}

/// The list: a new supply's id made from its name once, unique; a change
/// key by key, the comment and the other supplies kept; a supply that does
/// not read passed over; a file that does not read never written over.
#[test]
fn the_list_is_read_leniently_and_never_written_over() {
    let root = folder("list");
    three(&root);
    let id = change(&root, "", &Change { name: Some("Gift cards".into()), category: Some("Shop".into()), low_at: Some("3".into()), ..Change::default() }).unwrap();
    assert_eq!(id, "gift-cards");
    assert_eq!(change(&root, "", &Change { name: Some("Gift cards".into()), ..Change::default() }).unwrap(), "gift-cards-2");
    assert_eq!(id_for("Élastiques à tête", &[]), "elastiques-a-tete");
    change(&root, "stamps", &Change { name: Some("Stamps, green".into()), low_at: Some(String::new()), buy: Some("1,39".into()), ..Change::default() }).unwrap();
    let text = std::fs::read_to_string(file_in(&root)).unwrap();
    assert!(text.starts_with("# Invented supplies.") && text.contains("bought_by = 50"), "{text}");
    let list = List::load(&root).unwrap();
    let stamps = list.get("stamps").unwrap();
    assert_eq!((stamps.name.as_str(), stamps.low_at, stamps.buy.as_str(), stamps.id.as_str()), ("Stamps, green", None, "1,39", "stamps"), "its id never changes");
    assert_eq!(change(&root, "cards", &Change { low_at: Some("many".into()), ..Change::default() }).unwrap_err().word, "supplies-number-unreadable");
    assert_eq!(change(&root, "", &Change::default()).unwrap_err().word, "supplies-name-needed");
    // Passed over: no id, an id twice, a field of another type read as unsaid.
    let lenient = "[[supply]]\nname = \"No id\"\n\n[[supply]]\nid = \"tape\"\nname = \"Tape\"\nlow_at = \"ten\"\narchived = \"yes\"\n\n[[supply]]\nid = \"tape\"\nname = \"Tape again\"\n";
    let list = List::of_text(lenient).unwrap();
    assert_eq!((list.supplies.len(), list.skipped, list.supplies[0].low_at, list.supplies[0].archived), (1, 2, None, false));
    // No TOML: refused, the file as it was.
    std::fs::write(file_in(&root), "[[supply]\nid = ").unwrap();
    assert!(List::load(&root).is_err());
    assert_eq!(change(&root, "", &Change { name: Some("Ink".into()), ..Change::default() }).unwrap_err().word, "supplies-unreadable");
    assert_eq!(std::fs::read_to_string(file_in(&root)).unwrap(), "[[supply]\nid = ");
    let _ = std::fs::remove_dir_all(&root);
}

/// A device's movements' file that does not read is never written over; another device's is passed over.
#[test]
fn a_movements_file_that_does_not_read_is_never_written_over() {
    let root = folder("moves-unread");
    std::fs::create_dir_all(moves_dir(&root)).unwrap();
    std::fs::write(moves_dir(&root).join(device_file("desk")), "[[move]\n").unwrap();
    std::fs::write(moves_dir(&root).join(device_file("phone")), "[[move]]\nid = \"c1\"\nat = 100\nsupply = \"stamps\"\nkind = \"count\"\nquantity = 4\n\n[[move]]\nid = \"bad\"\nat = \"soon\"\nsupply = \"stamps\"\nkind = \"used\"\nquantity = 1\n").unwrap();
    let moves = read_moves(&root, "desk");
    assert!(moves.own_unreadable && moves.unreadable == [device_file("desk")]);
    assert_eq!(moves.lines.len(), 1, "a line that does not read is passed over");
    assert!(record(&root, "desk", &[line("c2", 200, "stamps", Kind::Count, "3")], &[]).is_err());
    assert!(settle(&root, "desk", &[done("t", 1, &[("stamps", "1")])], 2).is_err());
    assert_eq!(std::fs::read_to_string(moves_dir(&root).join(device_file("desk"))).unwrap(), "[[move]\n");
    assert_eq!(device_file("a/b:c"), "a_b_c.toml");
    assert_eq!(device_file(""), "this-device.toml");
    let _ = std::fs::remove_dir_all(&root);
}

/// A task's supplies: planned lines from its form, what it used written
/// when Sioul finishes it, taken out when it is open again; a tick on a
/// task done already keeps what was put right since.
#[test]
fn a_task_says_what_it_plans_and_what_it_used() {
    let zone = jiff::tz::TimeZone::UTC;
    let now = jiff::Timestamp::from_second(1_800_000_000).unwrap().to_zoned(zone.clone());
    let edit = TaskEdit { title: "Send a gift card".into(), supplies: vec![Use { id: "stamps".into(), quantity: q("1") }, Use { id: "cards".into(), quantity: q("1.5") }, Use { id: "bad id".into(), quantity: q("1") }], ..TaskEdit::default() };
    let text = crate::tasks::new_task(&edit, "sale", &zone, &now).unwrap();
    assert!(text.contains("X-SIOUL-SUPPLY;X-SIOUL-ID=stamps:1\r\n") && text.contains("X-SIOUL-SUPPLY;X-SIOUL-ID=cards:1.5\r\n") && !text.contains("bad"), "{text}");
    let task = task_of_text(&text, &zone).unwrap();
    assert_eq!(task.supplies.len(), 2);
    assert!(!task.used_said);
    // Saved unchanged by the form: nothing written.
    assert_eq!(crate::tasks::apply(&text, &TaskEdit::of(&task), &zone, &now).unwrap(), text);
    // Finished by Sioul: what it planned, said on it.
    let finished = crate::tasks::set_status(&text, Status::Completed, &zone, &now).unwrap();
    let task = task_of_text(&finished, &zone).unwrap();
    assert!(task.used_said && task.supplies_used == task.supplies, "{finished}");
    assert_eq!(finished.matches("SEQUENCE:").count(), 1);
    // Put right: two stamps, no card; the card at 0, so that it never reads as planned.
    let righted = with_used(&finished, &[Use { id: "stamps".into(), quantity: q("2") }], &now).unwrap();
    let task = task_of_text(&righted, &zone).unwrap();
    assert_eq!(used_by(&task), [Use { id: "stamps".into(), quantity: q("2") }, Use { id: "cards".into(), quantity: Qty::ZERO }]);
    // Ticked done again: kept.
    assert_eq!(crate::tasks::set_status(&righted, Status::Completed, &zone, &now).unwrap(), righted);
    // Open again: what it used goes; its plan stays.
    let reopened = crate::tasks::set_status(&righted, Status::NeedsAction, &zone, &now).unwrap();
    let task = task_of_text(&reopened, &zone).unwrap();
    assert!(!task.used_said && task.supplies.len() == 2, "{reopened}");
    // A supply taken off its plan by the form.
    let fewer = TaskEdit { supplies: vec![Use { id: "cards".into(), quantity: q("1") }], ..TaskEdit::of(&task) };
    let changed = crate::tasks::apply(&reopened, &fewer, &zone, &now).unwrap();
    assert!(!changed.contains("ID=stamps") && changed.contains("X-SIOUL-SUPPLY;X-SIOUL-ID=cards:1\r\n"), "{changed}");
    // A task with no supplies done: no line of what it used.
    let plain = crate::tasks::new_task(&TaskEdit { title: "Plain".into(), ..TaskEdit::default() }, "plain", &zone, &now).unwrap();
    assert!(!crate::tasks::set_status(&plain, Status::Completed, &zone, &now).unwrap().contains(USED));
}

/// The lines a phone's account manager sends back (an X- property, its
/// parameter and its value kept as text, escaped or not) read the same.
#[test]
fn a_task_s_supplies_read_back_as_another_program_writes_them() {
    let zone = jiff::tz::TimeZone::UTC;
    let text = "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:x\r\nSUMMARY:x\r\nX-SIOUL-SUPPLY;X-SIOUL-ID=\"stamps\":1\r\nx-sioul-supply;x-sioul-id=cards:2\r\nX-SIOUL-SUPPLY;X-SIOUL-ID=cards:3\r\nX-SIOUL-SUPPLY;X-SIOUL-ID=ink:lots\r\nX-SIOUL-RESUPPLY:tape\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
    let task = task_of_text(text, &zone).unwrap();
    assert_eq!(task.supplies, [Use { id: "stamps".into(), quantity: q("1") }, Use { id: "cards".into(), quantity: q("3") }]);
    assert_eq!(task.resupply, "tape");
}

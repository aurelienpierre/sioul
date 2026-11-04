// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The money movements' tests (`movements`), on invented budgets, banks and letters.

use super::*;
use crate::bank::{Account, Bank};

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

fn ledger(text: &str) -> Ledger {
    toml::from_str(text).unwrap()
}

fn line(account: &str, id: &str, date: &str, cents: i64, label: &str) -> bank::Movement {
    bank::Movement { account: account.into(), date: day(date), amount: Money(cents), label: label.into(), id: id.into() }
}

fn derived(ledger: &Ledger, bank: &Bank, contracts: &[Contract], letters: &[Letter], store: &Store, from: &str, to: &str, today: &str) -> Vec<Movement> {
    let words = crate::words::Words::builtin();
    derive(&Input { ledger, bank, contracts, letters, store, words: &words }, day(from), day(to), day(today))
}

fn one<'a>(all: &'a [Movement], key: &str) -> &'a Movement {
    all.iter().find(|m| m.has(key)).unwrap_or_else(|| panic!("no movement {key}: {all:#?}"))
}

/// A folder of its own for a test that writes the store.
fn folder(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sioul-movements-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

const BUDGETS: &str = "[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n\
    [[budget]]\nid = 'leisure'\ntitle = 'Leisure'\nperiod = 'month'\narea = 'leisure'\n\
    [[budget]]\nid = 'work'\ntitle = 'Work'\nperiod = 'month'\narea = 'work'\n";

/// Presets become occurrences, each with the contract that names it; a line
/// standing for one joins it with the real amount; other lines are
/// movements of their own; estimates are envelopes, not movements; "about"
/// and a range are kept; two lines just alike without ids stay two.
#[test]
fn lines_and_presets_become_movements() {
    let ledger = ledger(&format!(
        "{BUDGETS}\
         [[preset]]\nid = 'rent'\nbudget = 'home'\nlabel = 'Rent'\namount = -600\nevery = 'month'\nday = 1\n\
         [[preset]]\nid = 'phone'\nbudget = 'home'\nlabel = 'Phone'\namount = -19.99\nevery = 'month'\nday = 12\n\
         [[preset]]\nid = 'energy'\nbudget = 'home'\nlabel = 'EDF'\namount = -52\nevery = 'month'\nday = 5\nabout = true\n\
         [[preset]]\nid = 'water'\nbudget = 'home'\nlabel = 'Water'\namount = -60\nevery = 'month'\nday = 20\nlow = 50\nhigh = 70\n\
         [[preset]]\nid = 'food'\nbudget = 'home'\nlabel = 'Food'\namount = -300\nevery = 'month'\nday = 15\nestimate = true\n\
         [[line]]\nid = 'l1'\nbudget = 'home'\ndate = 2026-11-12\namount = -29.99\nlabel = 'Phone bill'\npreset = 'phone'\nlinks = ['mid:bill@telecom.example', 'sioul:project/flat']\n\
         [[line]]\nid = 'l2'\nbudget = 'leisure'\ndate = 2026-11-08\namount = -45\nlabel = 'Cinema'\n\
         [[line]]\nid = 'l3'\nbudget = 'home'\ndate = 2026-11-25\namount = -120\nlabel = 'Plumber'\nplanned = true\n\
         [[line]]\nbudget = 'home'\ndate = 2026-11-03\namount = -12.5\nlabel = 'Market'\n\
         [[line]]\nbudget = 'home'\ndate = 2026-11-03\namount = -12.5\nlabel = 'Market'\n"
    ));
    let contracts = vec![Contract { id: "c-rent".into(), title: "Lease".into(), preset: "rent".into(), ..Contract::default() }];
    let all = derived(&ledger, &Bank::default(), &contracts, &[], &Store::default(), "2026-11-01", "2026-11-30", "2026-11-02");
    // The rent, planned, with its contract; no export tells it passed: still planned.
    let rent = one(&all, "preset:rent/2026-11-01");
    assert_eq!((rent.id.as_str(), rent.state, rent.sum, rent.way), ("preset:rent/2026-11-01", State::Planned, Sum::Exact(Money(-60000)), Way::Out));
    assert_eq!(rent.sources.iter().map(|s| (s.kind, s.key.as_str())).collect::<Vec<_>>(), vec![(Kind::Preset, "preset:rent/2026-11-01"), (Kind::Contract, "sioul:contract/c-rent")]);
    // The phone bill stands for its occurrence: one movement, the real amount, its mail announcing it.
    let phone = one(&all, "preset:phone/2026-11-12");
    assert_eq!((phone.id.as_str(), phone.state, phone.sum, phone.label.as_str()), ("line:l1", State::Announced, Sum::Exact(Money(-2999)), "Phone"));
    assert!(phone.has("mid:bill@telecom.example") && phone.links == vec!["sioul:project/flat".to_string()], "{phone:#?}");
    assert_eq!(all.iter().filter(|m| m.has("preset:phone/2026-11-12") || m.has("line:l1")).count(), 1);
    // About, and a range: its middle said, both ends kept.
    assert_eq!(one(&all, "preset:energy/2026-11-05").sum.bounds(), (Money(-5720), Money(-4680)));
    let water = one(&all, "preset:water/2026-11-20");
    assert_eq!((water.sum, water.sum.amount(), water.sum.about()), (Sum::Between(Money(-5000), Money(-7000)), Money(-6000), true));
    // By hand: done (cash), planned, in their budget and its area.
    let cinema = one(&all, "line:l2");
    assert_eq!((cinema.state, cinema.place.clone(), cinema.area), (State::Done, Place::Budget("leisure".into()), Area::LEISURE));
    assert_eq!(one(&all, "line:l3").state, State::Planned);
    // The food envelope is no movement.
    assert!(!all.iter().any(|m| m.label == "Food"), "{all:#?}");
    // Two lines alike, without ids: two movements, named apart, the same on every device.
    let markets: Vec<&str> = all.iter().filter(|m| m.label == "Market").map(|m| m.id.as_str()).collect();
    assert_eq!(markets.len(), 2);
    assert_ne!(markets[0], markets[1]);
    assert_eq!(markets.iter().map(|k| k.starts_with("line:~")).collect::<Vec<_>>(), vec![true, true]);
    assert_eq!(line_keys(&ledger), line_keys(&ledger.clone()));
    // Ids are unique, days in order, each found by its address.
    let ids: BTreeSet<&str> = all.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids.len(), all.len());
    assert!(all.windows(2).all(|w| w[0].day <= w[1].day));
    let movements = Movements { list: all.clone() };
    assert_eq!(movements.find(&crate::links::movement_uri("preset:phone/2026-11-12")).map(|m| m.id.as_str()), Some("line:l1"));
}

/// The five states: planned (to come), announced (a mail, a letter), not
/// final yet (a card hold), done (the bank's line), not seen (its window
/// passed inside what the exports cover, with no line). Without any export,
/// a planned movement whose day passed stays planned.
#[test]
fn the_five_states() {
    let ledger = ledger(&format!(
        "{BUDGETS}\
         [[preset]]\nid = 'rent'\nbudget = 'home'\nlabel = 'Rent'\namount = -600\nevery = 'month'\nday = 1\n\
         [[preset]]\nid = 'edf'\nbudget = 'home'\nlabel = 'EDF'\namount = -52\nevery = 'month'\nday = 5\n\
         [[preset]]\nid = 'insurance'\nbudget = 'home'\nlabel = 'Accident insurance'\namount = -15\nevery = 'month'\nday = 10\n\
         [[preset]]\nid = 'wages'\nbudget = 'home'\nlabel = 'Wages'\namount = 1200\nevery = 'month'\nday = 28\n\
         [[line]]\nid = 'tax'\nbudget = 'home'\ndate = 2026-11-15\namount = -80\nlabel = 'Tax'\nplanned = true\nlinks = ['mid:tax@impots.example']\n\
         [[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home']\nexports = ['0123456A']\n"
    ));
    let mut bank = Bank::default();
    bank.accounts.push(Account { id: "0123456A".into(), title: String::new(), balance: Some((day("2026-11-02"), Money(90000))) });
    bank.movements = vec![
        line("0123456A", "A1", "2026-10-01", -60000, "PRLV SEPA RENT AGENCY"),
        line("0123456A", "A2", "2026-10-06", -5200, "PRLV SEPA EDF"),
        line("0123456A", "A3", "2026-10-28", 120000, "VIR WAGES EXEMPLE"),
        line("0123456A", "A4", "2026-11-01", -60000, "PRLV SEPA RENT AGENCY"),
        line("0123456A", "A5", "2026-11-02", -400, "CB BAKERY"),
    ];
    let store: Store = toml::from_str("format = 1\n[[hold]]\naccount = '0123456A'\nid = 'H1'\ndate = 2026-11-03\namount = -45.0\nlabel = 'CB STATION EXEMPLE'\n").unwrap();
    let tax_notice = Letter {
        id: "2026-11-02-1a2b".into(),
        status: "new".into(),
        reading: crate::letters::Reading { sender: "Tax office".into(), kind: crate::letters::Kind::TaxNotice, amount: Some(Money(32000)), deadline: Some(day("2026-11-20")), ..crate::letters::Reading::default() },
        ..Letter::default()
    };
    let all = derived(&ledger, &bank, &[], std::slice::from_ref(&tax_notice), &store, "2026-10-01", "2026-11-30", "2026-11-04");
    // Done: the bank's line joins the occurrence, its day and amount kept; the day planned beside it.
    let edf = one(&all, "preset:edf/2026-10-05");
    assert_eq!((edf.state, edf.day, edf.expected, edf.account.as_deref()), (State::Done, day("2026-10-06"), Some(day("2026-10-05")), Some("main")));
    assert!(edf.has("bank:main/A2"));
    assert_eq!(one(&all, "preset:rent/2026-11-01").state, State::Done);
    assert_eq!(one(&all, "preset:wages/2026-10-28").state, State::Done);
    // Not seen: the insurance on 10 October, inside what the exports cover.
    assert_eq!(one(&all, "preset:insurance/2026-10-10").state, State::NotSeen);
    // Planned: what is to come.
    assert_eq!(one(&all, "preset:insurance/2026-11-10").state, State::Planned);
    assert_eq!(one(&all, "preset:edf/2026-11-05").state, State::Planned);
    // Not final yet: the card hold, about its amount, in the account's first budget.
    let hold = one(&all, "hold:main/H1");
    assert_eq!((hold.state, hold.sum, hold.place.clone()), (State::NotFinal, Sum::About(Money(-4500)), Place::Budget("home".into())));
    // Announced: a bill by mail, a tax notice by letter (in no budget yet).
    assert_eq!(one(&all, "line:tax").state, State::Announced);
    let letter = one(&all, "sioul:letter/2026-11-02-1a2b");
    assert_eq!((letter.state, letter.place.clone(), letter.sum, letter.day), (State::Announced, Place::Unplaced, Sum::Exact(Money(-32000)), day("2026-11-20")));
    // The words of each state, as the Porch reads them.
    assert_eq!([State::Planned, State::Announced, State::NotFinal, State::Done, State::NotSeen].map(State::id), ["planned", "announced", "not-final", "done", "not-seen"]);
    // The card payment that stands for nothing: done, of its own.
    assert_eq!(one(&all, "bank:main/A5").state, State::Done);
    // No export at all: the day passed, nothing can tell, so it stays planned.
    let alone = derived(&ledger, &Bank::default(), &[], &[], &Store::default(), "2026-10-01", "2026-10-31", "2026-11-04");
    assert_eq!(one(&alone, "preset:insurance/2026-10-10").state, State::Planned);
    // A hold the bank posted since (its line, a few days later, another amount): gone, the line done.
    let mut posted = bank.clone();
    posted.movements.push(line("0123456A", "A6", "2026-11-05", -4210, "CB STATION EXEMPLE"));
    let after = derived(&ledger, &posted, &[], &[], &store, "2026-10-01", "2026-11-30", "2026-11-06");
    assert!(!after.iter().any(|m| m.state == State::NotFinal), "{after:#?}");
    assert_eq!(one(&after, "bank:main/A6").state, State::Done);
}

/// Money moved from the bank to PayPal is read on both accounts' exports:
/// one movement with two sides, between your own accounts, in no budget.
#[test]
fn a_transfer_is_one_movement_with_two_sides() {
    let ledger = ledger(&format!(
        "{BUDGETS}\
         [[bank_account]]\nid = 'bank'\ntitle = 'My bank'\nfills = ['home']\nexports = ['0123456A']\n\
         [[bank_account]]\nid = 'paypal'\ntitle = 'PayPal'\nkind = 'paypal'\nfills = ['work']\n"
    ));
    let mut bank = Bank::default();
    bank.movements = vec![
        line("0123456A", "B1", "2026-10-06", -3000, "PAYPAL EUROPE"),
        line("paypal", "P1", "2026-10-06", 3000, "Approvisionnement · Virement bancaire"),
        line("paypal", "P2", "2026-10-07", -3000, "Jean Exemple · Paiement envoyé"),
    ];
    let all = derived(&ledger, &bank, &[], &[], &Store::default(), "2026-10-01", "2026-10-31", "2026-10-10");
    let moved: Vec<&Movement> = all.iter().filter(|m| m.way == Way::Between).collect();
    assert_eq!(moved.len(), 1, "{all:#?}");
    let moved = moved[0];
    assert_eq!(moved.place, Place::Transfer { from: "bank".into(), to: "paypal".into() });
    assert!(moved.has("bank:bank/B1") && moved.has("bank:paypal/P1"), "{moved:#?}");
    assert_eq!((moved.sum, moved.account.as_deref(), moved.place.budgets().len()), (Sum::Exact(Money(-3000)), Some("bank"), 0));
    assert_eq!(moved.area, Area::ADMIN.with(Area::WORK), "the area of both accounts");
    // The payment out of PayPal is a movement of its own, in work.
    assert_eq!(one(&all, "bank:paypal/P2").place, Place::Budget("work".into()));
}

/// The store says its format; what it says holds ("Not the same", a source
/// joined by hand, a movement moved or in parts, the holds of one account);
/// an entry keeps what a later Sioul wrote in it; a store of a later format
/// is read for what is known and never written over.
#[test]
fn the_store_says_its_format_and_its_corrections_hold() {
    let root = folder("store");
    // A new store says its format first.
    Store::edit(&root, |edit| edit.apart("line:l1", "bank:main/A1")).unwrap();
    let text = std::fs::read_to_string(Store::path(&root)).unwrap();
    assert!(text.contains("format = 1"), "{text}");
    let store = Store::load(&root).unwrap();
    assert_eq!((store.format, store.apart.clone()), (1, vec![Apart { a: "bank:main/A1".into(), b: "line:l1".into() }]));
    // The SHOP line from mail and the bank's line are one payment, until "Not the same".
    let ledger = ledger(&format!(
        "{BUDGETS}\
         [[preset]]\nid = 'insurance'\nbudget = 'home'\nlabel = 'Insurance'\namount = -15\nevery = 'month'\nday = 10\n\
         [[line]]\nid = 'l1'\nbudget = 'leisure'\ndate = 2026-10-01\namount = -23.98\nlabel = 'SHOP'\nlinks = ['mid:shop@pay.example']\n\
         [[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home', 'leisure']\nexports = ['0123456A']\n"
    ));
    let mut bank = Bank::default();
    bank.movements = vec![line("0123456A", "A1", "2026-10-03", -2398, "CB SHOP")];
    let together = derived(&ledger, &bank, &[], &[], &Store::default(), "2026-10-01", "2026-10-31", "2026-10-20");
    let shop = one(&together, "line:l1");
    assert!(shop.has("bank:main/A1") && shop.state == State::Done, "{shop:#?}");
    let apart = derived(&ledger, &bank, &[], &[], &store, "2026-10-01", "2026-10-31", "2026-10-20");
    assert_ne!(one(&apart, "line:l1").id, one(&apart, "bank:main/A1").id);
    assert_eq!(one(&apart, "line:l1").state, State::Announced);
    // A letter joined by hand; the occurrence moved to leisure; "Not the same" undone.
    Store::edit(&root, |edit| {
        edit.link("preset:insurance/2026-10-10", "sioul:letter/2026-10-02-9f", "hand", None);
        edit.place("preset:insurance/2026-10-10", Some("leisure"), &[], false);
        assert!(edit.together("bank:main/A1", "line:l1"));
    })
    .unwrap();
    let store = Store::load(&root).unwrap();
    let all = derived(&ledger, &bank, &[], &[], &store, "2026-10-01", "2026-10-31", "2026-10-05");
    let insurance = one(&all, "preset:insurance/2026-10-10");
    assert!(insurance.has("sioul:letter/2026-10-02-9f"), "{insurance:#?}");
    assert_eq!((insurance.state, insurance.place.clone(), insurance.area), (State::Announced, Place::Budget("leisure".into()), Area::LEISURE));
    assert!(one(&all, "line:l1").has("bank:main/A1"), "together again");
    // In parts: 10 to work, the rest where it was asked.
    Store::edit(&root, |edit| edit.place("line:l1", Some("leisure"), &[("work".into(), Money(-1000))], false)).unwrap();
    let all = derived(&ledger, &bank, &[], &[], &Store::load(&root).unwrap(), "2026-10-01", "2026-10-31", "2026-10-05");
    assert_eq!(one(&all, "line:l1").place, Place::Parts(vec![("work".into(), Money(-1000)), ("leisure".into(), Money(-1398))]));
    // What a later Sioul wrote in an entry is kept when this one changes it.
    let text = std::fs::read_to_string(Store::path(&root)).unwrap().replace("movement = \"line:l1\"", "movement = \"line:l1\"\nnote = \"kept\"");
    std::fs::write(Store::path(&root), &text).unwrap();
    Store::edit(&root, |edit| edit.place("line:l1", None, &[], true)).unwrap();
    let text = std::fs::read_to_string(Store::path(&root)).unwrap();
    assert!(text.contains("note = \"kept\"") && text.contains("none = true") && !text.contains("parts"), "{text}");
    // An account's holds replace its own, never another account's.
    let hold = |account: &str, id: &str| Hold { account: account.into(), id: id.into(), date: day("2026-10-04"), amount: Money(-500), label: "CB".into() };
    Store::edit(&root, |edit| {
        edit.holds("0123456A", &[hold("0123456A", "H1"), hold("0123456A", "H2")]);
        edit.holds("paypal", &[hold("paypal", "H9")]);
        edit.holds("0123456A", &[hold("0123456A", "H3")]);
    })
    .unwrap();
    let holds: Vec<(String, String)> = Store::load(&root).unwrap().holds.into_iter().map(|h| (h.account, h.id)).collect();
    assert_eq!(holds, vec![("paypal".to_string(), "H9".to_string()), ("0123456A".to_string(), "H3".to_string())]);
    // A store of a later format: read for what this Sioul knows, never written over.
    let later = "format = 2\nsomething = \"new\"\n\n[[apart]]\na = \"bank:main/A1\"\nb = \"line:l1\"\nwhy = \"later\"\n";
    std::fs::write(Store::path(&root), later).unwrap();
    assert_eq!(Store::load(&root).unwrap().apart.len(), 1);
    assert!(Store::edit(&root, |edit| edit.unlink("x")).is_err());
    assert_eq!(std::fs::read_to_string(Store::path(&root)).unwrap(), later, "left as it is");
    let _ = std::fs::remove_dir_all(&root);
}

/// A budget file as 705576a and 7208782 write it reads as before: the same
/// budgets, presets and lines, the same figures; the fields this Sioul adds
/// (a line's id, a preset's "about" and range) change none of them, and a
/// field a later Sioul adds anywhere is passed over (no `deny_unknown_fields`).
#[test]
fn an_older_budget_file_reads_as_before() {
    let older = include_str!("../../../examples/sioul-budgets.toml");
    let before: Ledger = toml::from_str(older).unwrap();
    let today = day("2026-10-02");
    let figures = |ledger: &Ledger| ledger.budgets.iter().map(|b| {
        let s = ledger.status(b, today);
        (b.id.clone(), s.so_far, s.to_come, s.projected, s.verdict, s.reserve_transfer)
    }).collect::<Vec<_>>();
    assert_eq!(
        figures(&before),
        vec![
            ("home".to_string(), Money(-60000), Money(50000), Money(-10000), crate::budget::Verdict::AsPlanned, Some(("savings".to_string(), Money(10000)))),
            ("projects".to_string(), Money::ZERO, Money::ZERO, Money::ZERO, crate::budget::Verdict::AsPlanned, None),
        ]
    );
    // With what this Sioul writes, and what a later one might.
    let newer = older
        .replace("label = \"Rent\"", "label = \"Rent\"\nabout = true\nlow = 590\nhigh = 610\nlater = \"passed over\"")
        .replace("[[line]]\n", "[[line]]\nid = \"5b2f0c1e-0000-4000-8000-000000000001\"\nlater = 1\n")
        .replace("id = \"home\"", "id = \"home\"\nlater = [1, 2]");
    let after: Ledger = toml::from_str(&newer).unwrap();
    assert_eq!(figures(&after), figures(&before));
    assert_eq!((after.presets[0].about, after.presets[0].low, after.lines[0].id.as_str()), (true, Some(Money(59000)), "5b2f0c1e-0000-4000-8000-000000000001"));
    assert_eq!(before.lines[0].id, "", "an older line has none");
    // The movements read both alike, but for the rent's range.
    let derive_it = |ledger: &Ledger| derived(ledger, &Bank::default(), &[], &[], &Store::default(), "2026-10-01", "2026-10-31", "2026-10-02");
    let (a, b) = (derive_it(&before), derive_it(&after));
    assert_eq!(a.len(), b.len());
    assert_eq!(one(&b, "preset:#0/2026-10-01").sum, Sum::Between(Money(-59000), Money(-61000)));
}

/// What is kept names a movement by its steadiest key, never by the digest
/// of a line written by hand, which changes as soon as the line is edited:
/// a hand-written line standing for its occurrence, moved, then its label
/// edited, stays moved; one the bank's line joined, the same.
#[test]
fn a_move_names_the_steadiest_key() {
    let root = folder("steady");
    let text = |label: &str| {
        format!(
            "{BUDGETS}\
             [[preset]]\nid = 'phone'\nbudget = 'home'\nlabel = 'Phone'\namount = -19.99\nevery = 'month'\nday = 12\n\
             [[line]]\nbudget = 'home'\ndate = 2026-10-12\namount = -29.99\nlabel = '{label}'\npreset = 'phone'\n\
             [[line]]\nbudget = 'home'\ndate = 2026-10-03\namount = -23.98\nlabel = '{label} shop'\n\
             [[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home', 'leisure']\nexports = ['0123456A']\n"
        )
    };
    let mut bank = Bank::default();
    bank.movements = vec![line("0123456A", "A1", "2026-10-04", -2398, "CB SHOP")];
    let read = |label: &str, store: &Store| derived(&ledger(&text(label)), &bank, &[], &[], store, "2026-10-01", "2026-10-31", "2026-10-20");
    let before = read("Phone bill", &Store::default());
    let phone = one(&before, "preset:phone/2026-10-12");
    assert!(phone.id.starts_with("line:~"), "its id, for this reading: the line's digest");
    assert_eq!(phone.steady_key(), "preset:phone/2026-10-12", "kept by its occurrence");
    let shop = one(&before, "bank:main/A1");
    assert_eq!(shop.steady_key(), "bank:main/A1", "kept by its bank line");
    assert_eq!(shop.address(), crate::links::movement_uri("bank:main/A1"));
    // Both moved to leisure, each by its steadiest key, as the window writes it.
    Store::edit(&root, |edit| {
        edit.place(phone.steady_key(), Some("leisure"), &[], false);
        edit.place(shop.steady_key(), Some("leisure"), &[], false);
    })
    .unwrap();
    let store = Store::load(&root).unwrap();
    // The labels edited by hand: new digests, the moves hold.
    let after = read("Mobile", &store);
    let (phone, shop) = (one(&after, "preset:phone/2026-10-12"), one(&after, "bank:main/A1"));
    assert!(!after.iter().any(|m| m.id == before.iter().find(|m| m.has("preset:phone/2026-10-12")).unwrap().id), "the digest changed");
    assert_eq!((phone.place.clone(), shop.place.clone()), (Place::Budget("leisure".into()), Place::Budget("leisure".into())));
    assert_eq!(phone.area, Area::LEISURE);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- part 2: matching, moving, mail

use crate::budget::{MailLine, MailState};
use crate::payments::{Payment as Told, PaymentKind};

fn mail(key: &str, on: &str, kind: PaymentKind, cents: i64, currency: &'static str, party: &str) -> MailLine {
    MailLine {
        key: key.into(),
        path: None,
        date: day(on),
        payment: Told { kind, amount: Some(crate::money::Amount { money: Money(cents), currency }), party: party.into() },
        subject: String::new(),
        sender: party.into(),
        sender_domain: None,
        budget: None,
        preset: None,
        label: party.into(),
        state: MailState::Proposed,
        doubtful: false,
        refs: Vec::new(),
        due: None,
        instalments: None,
    }
}

/// The keys (research §5.2): a mandate's reference, learnt from October's
/// debit, holds November's, whose amount changed; an invoice's number in a
/// client's transfer pays it.
#[test]
fn each_key_makes_a_pair_sure() {
    let ledger = ledger(&format!(
        "{BUDGETS}\
         [[preset]]\nid = 'energy'\nbudget = 'home'\nlabel = 'Energy'\namount = -52\nevery = 'month'\nday = 5\n\
         [[line]]\nid = 'inv'\nbudget = 'work'\ndate = 2026-11-30\namount = 480\nlabel = 'Client A'\nplanned = true\nlinks = ['sioul:invoice/2026-002']\n\
         [[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home', 'work']\nexports = ['0123456A']\n"
    ));
    let mut bank = Bank::default();
    bank.movements = vec![
        line("0123456A", "A1", "2026-10-05", -5200, "PRLV SEPA ENERGIE EXEMPLE RUM EX-778899 ICS FR12ZZZ123456"),
        line("0123456A", "A2", "2026-11-05", -7430, "PRLV SEPA EXPL RUM EX-778899 ICS FR12ZZZ123456"),
        line("0123456A", "A3", "2026-11-10", 48000, "VIR CLIENT A FACTURE 2026-002"),
        line("0123456A", "A4", "2026-11-12", -100, "CB BAKERY"),
    ];
    let all = derived(&ledger, &bank, &[], &[], &Store::default(), "2026-10-01", "2026-11-30", "2026-11-20");
    assert!(one(&all, "preset:energy/2026-10-05").has("bank:main/A1"));
    let november = one(&all, "preset:energy/2026-11-05");
    assert!(november.has("bank:main/A2"), "the mandate's key holds a changed amount: {all:#?}");
    assert_eq!((november.sum, november.state), (Sum::Exact(Money(-7430)), State::Done));
    assert!(!november.sources.iter().any(|s| s.probable), "sure: the key says it");
    let invoice = one(&all, "line:inv");
    assert!(invoice.has("bank:main/A3") && invoice.state == State::Done, "{invoice:#?}");
}

/// The traps of research §5.3, one each: equal amounts the names tell apart;
/// a monthly schedule beside the year's balance; an order charged in parts;
/// two coffees and one receipt (distinct bank ids, never duplicates); a
/// deferred-debit card's sum; a hold the bank posted with another amount.
#[test]
fn the_traps() {
    let ledger = ledger(&format!(
        "{BUDGETS}\
         [[preset]]\nid = 'stream'\nbudget = 'leisure'\nlabel = 'Exemplestream'\namount = -9.99\nevery = 'month'\nday = 5\n\
         [[preset]]\nid = 'music'\nbudget = 'leisure'\nlabel = 'Musicexemple'\namount = -9.99\nevery = 'month'\nday = 5\n\
         [[preset]]\nid = 'energy'\nbudget = 'home'\nlabel = 'Energy'\namount = -52\nevery = 'month'\nday = 5\n\
         [[line]]\nid = 'order'\nbudget = 'home'\ndate = 2026-11-02\namount = -100\nlabel = 'Shop order'\nlinks = ['mid:order@shop.example']\n\
         [[line]]\nid = 'balance'\nbudget = 'home'\ndate = 2026-11-20\namount = -84.20\nlabel = 'Energy, the year balance'\nplanned = true\nlinks = ['mid:balance@energy.example']\n\
         [[line]]\nid = 'coffee'\nbudget = 'leisure'\ndate = 2026-11-06\namount = -2.50\nlabel = 'Cafe Exemple'\nlinks = ['mid:coffee@cafe.example']\n\
         [[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home', 'leisure']\nexports = ['0123456A']\n"
    ));
    let mut bank = Bank::default();
    bank.movements = vec![
        line("0123456A", "D1", "2026-10-07", -2000, "CB LIBRAIRIE EXEMPLE"),
        line("0123456A", "D2", "2026-10-20", -3550, "CB PHARMACIE EXEMPLE"),
        line("0123456A", "O1", "2026-11-03", -6000, "CB SHOP"),
        line("0123456A", "O2", "2026-11-04", -4000, "CB SHOP"),
        line("0123456A", "M1", "2026-11-05", -999, "CB MUSICEXEMPLE"),
        line("0123456A", "S1", "2026-11-05", -999, "CB EXEMPLESTREAM"),
        line("0123456A", "E1", "2026-11-05", -5200, "PRLV SEPA ENERGY"),
        line("0123456A", "DT", "2026-11-05", -5550, "FACTURE CARTE DU 05/11"),
        line("0123456A", "C1", "2026-11-06", -250, "CB CAFE EXEMPLE"),
        line("0123456A", "C2", "2026-11-06", -250, "CB CAFE EXEMPLE"),
        line("0123456A", "P1", "2026-11-14", -4210, "CB STATION EXEMPLE"),
        line("0123456A", "E2", "2026-11-23", -8420, "PRLV SEPA ENERGY"),
    ];
    let store: Store = toml::from_str("format = 1\n[[hold]]\naccount = '0123456A'\nid = 'H1'\ndate = 2026-11-12\namount = -45.0\nlabel = 'CB STATION EXEMPLE'\n").unwrap();
    let all = derived(&ledger, &bank, &[], &[], &store, "2026-10-01", "2026-11-30", "2026-11-25");
    // Equal amounts by chance: the names decide.
    assert!(one(&all, "preset:stream/2026-11-05").has("bank:main/S1") && one(&all, "preset:music/2026-11-05").has("bank:main/M1"), "{all:#?}");
    // A monthly schedule, and the year's balance its own: each its own line of the bank.
    assert!(one(&all, "preset:energy/2026-11-05").has("bank:main/E1"));
    let balance = one(&all, "line:balance");
    assert!(balance.has("bank:main/E2") && balance.state == State::Done, "{balance:#?}");
    // An order charged in parts: one movement, its two charges probably together, the receipt's amount.
    let order = one(&all, "line:order");
    assert!(order.has("bank:main/O1") && order.has("bank:main/O2"), "{order:#?}");
    assert_eq!((order.sum, order.day), (Sum::Exact(Money(-10000)), day("2026-11-03")));
    assert!(order.sources.iter().filter(|s| s.kind == Kind::Bank).all(|s| s.probable));
    // Two coffees, one receipt: one of them, probably; the other a movement of its own.
    let coffee = one(&all, "line:coffee");
    let joined: Vec<&str> = coffee.sources.iter().filter(|s| s.kind == Kind::Bank).map(|s| s.key.as_str()).collect();
    assert_eq!(joined.len(), 1, "{coffee:#?}");
    assert!(coffee.sources.iter().any(|s| s.kind == Kind::Bank && s.probable));
    let other = if joined[0] == "bank:main/C1" { "bank:main/C2" } else { "bank:main/C1" };
    assert_eq!(one(&all, other).state, State::Done);
    // A deferred-debit card's sum settles October's card payments: in no budget, together with them.
    let total = one(&all, "bank:main/DT");
    assert_eq!(total.place, Place::Nowhere);
    assert_eq!(total.together.len(), 2, "{total:#?}");
    assert!(one(&all, "bank:main/D1").together.contains(&total.steady_key().to_string()));
    // A hold the bank posted, a little less: no longer "not final yet"; the line itself done.
    assert!(!all.iter().any(|m| m.state == State::NotFinal));
    assert_eq!(one(&all, "bank:main/P1").state, State::Done);
}

/// Mail about money joins what is counted, without a question (research
/// §6, recommendation 5): a refund five weeks later under its processor's
/// name; a receipt in dollars by its payee's name; an invoice by its
/// Factur-X number; a sender not verified stays proposed, to be checked
/// first; what matches nothing stays proposed. Kept as links, read back.
#[test]
fn mail_about_money_joins_what_is_counted() {
    let root = folder("mails");
    std::fs::write(root.join(crate::budget::LEDGER), format!("{BUDGETS}[[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home', 'leisure']\nexports = ['0123456A']\n")).unwrap();
    std::fs::write(
        root.join(crate::bank::MANIFEST),
        "[[movement]]\naccount = '0123456A'\ndate = 2026-11-30\namount = 23.98\nlabel = 'PAYPAL *SHOP REMBOURSEMENT'\nid = 'R1'\n\
         [[movement]]\naccount = '0123456A'\ndate = 2026-11-03\namount = -18.47\nlabel = 'CB EXEMPLEAI SAN FRANCISCO'\nid = 'U1'\n\
         [[movement]]\naccount = '0123456A'\ndate = 2026-11-21\namount = -84.20\nlabel = 'PRLV SEPA EXPL FA-2026-0042'\nid = 'F1'\n",
    )
    .unwrap();
    let words = crate::words::Words::builtin();
    let mut factur_x = mail("mid:bill@energy.example", "2026-11-02", PaymentKind::Bill, 8420, "EUR", "Énergie Exemple");
    factur_x.refs = vec!["ref:FA20260042".into()];
    factur_x.due = Some(day("2026-11-20"));
    let mut doubtful = mail("mid:fake@pay.example", "2026-11-03", PaymentKind::Paid, 1847, "EUR", "Exempleai");
    doubtful.doubtful = true;
    let mut lines = vec![
        mail("mid:refund@shop.example", "2026-10-25", PaymentKind::Refund, 2398, "EUR", "Shop"),
        mail("mid:receipt@ai.example", "2026-11-02", PaymentKind::Paid, 2000, "USD", "Exempleai"),
        factur_x,
        doubtful,
        mail("mid:nothing@shop.example", "2026-11-02", PaymentKind::Paid, 777, "EUR", "Elsewhere"),
    ];
    let today = day("2026-12-01");
    assert_eq!(acts::join_mails(&root, &words, &mut lines, today).unwrap(), 3);
    let state = |key: &str| lines.iter().find(|l| l.key == key).unwrap().state.clone();
    assert!(matches!(state("mid:refund@shop.example"), MailState::Joined { .. }));
    assert!(matches!(state("mid:receipt@ai.example"), MailState::Joined { .. }));
    assert!(matches!(state("mid:bill@energy.example"), MailState::Joined { probable: false, .. }), "the invoice's number makes it sure");
    assert_eq!((state("mid:fake@pay.example"), state("mid:nothing@shop.example")), (MailState::Proposed, MailState::Proposed));
    // Kept: the store holds each link with what the message said, and the movements show it.
    let store = Store::load(&root).unwrap();
    let refund = store.links.iter().find(|l| l.source == "mid:refund@shop.example").unwrap();
    assert_eq!((refund.movement.as_str(), refund.amount, refund.day), ("bank:main/R1", Some(Money(2398)), Some(day("2026-10-25"))));
    let movements = load(&root, &words, day("2026-10-01"), day("2026-12-31"), today);
    assert!(movements.find("bank:main/R1").unwrap().has("mid:refund@shop.example"));
    // Again: nothing new is written; what was joined is said so from its link.
    let mut again: Vec<MailLine> = lines.iter().map(|l| MailLine { state: if l.doubtful || l.key.contains("nothing") { l.state.clone() } else { MailState::Proposed }, ..l.clone() }).collect();
    assert_eq!(acts::join_mails(&root, &words, &mut again, today).unwrap(), 0);
    assert!(matches!(again.iter().find(|l| l.key == "mid:refund@shop.example").unwrap().state, MailState::Joined { .. }));
    let _ = std::fs::remove_dir_all(&root);
}

/// An export taken in twice adds nothing (research §5.3: the bank's own ids):
/// its booked lines once, its hold replaced, its pair with the rent kept as a link once.
#[test]
fn an_export_taken_in_twice_adds_nothing() {
    let root = folder("take-in");
    std::fs::write(root.join(crate::budget::LEDGER), format!("{BUDGETS}[[preset]]\nid = 'rent'\nbudget = 'home'\nlabel = 'Rent'\namount = -600\nevery = 'month'\nday = 1\n[[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home']\nexports = ['FR7600000000000000000000000']\n")).unwrap();
    let camt = r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02"><BkToCstmrStmt><Stmt><Acct><Id><IBAN>FR7600000000000000000000000</IBAN></Id></Acct>
        <Bal><Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp><Amt Ccy="EUR">900.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Dt><Dt>2026-11-03</Dt></Dt></Bal>
        <Ntry><Amt Ccy="EUR">600.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2026-11-02</Dt></BookgDt><AcctSvcrRef>R1</AcctSvcrRef><NtryDtls><TxDtls><RltdPties><Cdtr><Nm>RENT AGENCY</Nm></Cdtr></RltdPties></TxDtls></NtryDtls></Ntry>
        <Ntry><Amt Ccy="EUR">4.50</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2026-11-02</Dt></BookgDt><AcctSvcrRef>R2</AcctSvcrRef><AddtlNtryInf>CB BAKERY</AddtlNtryInf></Ntry>
        <Ntry><Amt Ccy="EUR">45.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>PDNG</Sts><BookgDt><Dt>2026-11-03</Dt></BookgDt><AcctSvcrRef>R3</AcctSvcrRef><AddtlNtryInf>CB STATION</AddtlNtryInf></Ntry>
        </Stmt></BkToCstmrStmt></Document>"#;
    let words = crate::words::Words::builtin();
    let statement = crate::bank::read(&words.bank, camt).unwrap();
    assert_eq!((statement.movements.len(), statement.pending.len()), (2, 1), "booked apart from pending");
    let today = day("2026-11-04");
    let first = acts::take_in(&root, &statement, "Main", &words, today).unwrap();
    assert_eq!(first, acts::TakenIn { read: 2, new: 2, holds: 1, joined: 1 });
    let store = Store::load(&root).unwrap();
    assert_eq!(store.links.iter().map(|l| (l.movement.as_str(), l.source.as_str(), l.how.as_str())).collect::<Vec<_>>(), vec![("preset:rent/2026-11-01", "bank:main/R1", "sure")]);
    let again = acts::take_in(&root, &statement, "Main", &words, today).unwrap();
    assert_eq!(again, acts::TakenIn { read: 2, new: 0, holds: 1, joined: 0 });
    let store = Store::load(&root).unwrap();
    assert_eq!((store.links.len(), store.holds.len()), (1, 1));
    assert_eq!(Bank::load(&root).unwrap().movements.len(), 2);
    // The hold is not final yet; the rent done; the bakery its own.
    let all = load(&root, &words, day("2026-11-01"), day("2026-11-30"), today);
    assert_eq!(all.find("hold:main/R3").map(|m| m.state), Some(State::NotFinal));
    assert_eq!(all.find("bank:main/R1").map(|m| m.id.as_str()), Some("preset:rent/2026-11-01"));
    let _ = std::fs::remove_dir_all(&root);
}

/// Every budget is a choice for a line, whatever the hour (the owner's
/// report): the page shows what the hour admits, a line goes anywhere.
#[test]
fn every_budget_is_a_choice_at_any_hour() {
    let root = folder("hidden");
    std::fs::write(root.join(crate::budget::LEDGER), format!("{BUDGETS}[[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home']\nexports = ['0123456A']\n")).unwrap();
    std::fs::write(root.join(crate::bank::MANIFEST), "[[movement]]\naccount = '0123456A'\ndate = 2026-11-03\namount = 480.0\nlabel = 'VIR CLIENT B'\nid = 'W1'\n").unwrap();
    let ledger = Ledger::load(&root).unwrap();
    let tr = crate::i18n::Translator::new("en");
    // In leisure's hours, work's budget is not shown, and still chosen.
    let page = crate::view::MoneyPage { ledger: &ledger, bank: &crate::bank::Bank::default(), movements: &[], mail: &[], shown: crate::areas::Area::LEISURE, own: false };
    let view = crate::view::budgets_page(&page, &tr, day("2026-11-04"));
    assert!(!view.budgets.iter().any(|c| c.id == "work") && view.choices.iter().any(|c| c.id == "work"));
    let words = crate::words::Words::builtin();
    let today = day("2026-11-04");
    let all = load(&root, &words, day("2026-11-01"), day("2026-11-30"), today);
    let client = all.find("bank:main/W1").unwrap().clone();
    acts::move_to(&root, &words, &client, &Place::Budget("work".into())).unwrap();
    let all = load(&root, &words, day("2026-11-01"), day("2026-11-30"), today);
    assert_eq!(all.find("bank:main/W1").unwrap().place, Place::Budget("work".into()));
    assert!(std::fs::read_to_string(root.join(crate::budget::LEDGER)).unwrap().contains("budget = \"work\""), "in the file's [[assign]], which every version reads");
    let _ = std::fs::remove_dir_all(&root);
}

/// Moving a movement says its effect ("Home holds €12 more"), asks once
/// whether the payee's next ones go there too, and a yes is a rule of the
/// budget file (research §6, recommendation 6: the person's act is the consent).
#[test]
fn a_move_says_its_effect_and_asks_once() {
    let root = folder("move");
    std::fs::write(root.join(crate::budget::LEDGER), format!("{BUDGETS}[[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home', 'leisure']\nexports = ['0123456A']\n")).unwrap();
    std::fs::write(
        root.join(crate::bank::MANIFEST),
        "[[movement]]\naccount = '0123456A'\ndate = 2026-10-02\namount = -12.0\nlabel = 'CB CINEMA LE PALACE'\nid = 'C1'\n\
         [[movement]]\naccount = '0123456A'\ndate = 2026-10-20\namount = -12.0\nlabel = 'CB CINEMA LE PALACE'\nid = 'C2'\n\
         [[movement]]\naccount = '0123456A'\ndate = 2026-10-21\namount = -30.0\nlabel = 'CB LIBRAIRIE EXEMPLE'\nid = 'L1'\n",
    )
    .unwrap();
    let words = crate::words::Words::builtin();
    let today = day("2026-10-25");
    let read = || load(&root, &words, day("2026-10-01"), day("2026-10-31"), today);
    let first = read().find("bank:main/C1").unwrap().clone();
    let moved = acts::move_to(&root, &words, &first, &Place::Budget("leisure".into())).unwrap();
    assert_eq!((moved.budget.as_str(), moved.more, moved.day), ("home", Money(1200), day("2026-10-02")), "Home holds €12 more");
    assert_eq!(moved.ask.as_deref(), Some("cinema palace"));
    acts::decide(&root, "cinema palace", "leisure", Way::Out, false).unwrap();
    let second = read().find("bank:main/C2").unwrap().clone();
    assert_eq!(acts::move_to(&root, &words, &second, &Place::Budget("leisure".into())).unwrap().ask, None, "asked once");
    // A yes: the payee's next movements go there by a rule of the file.
    let book = read().find("bank:main/L1").unwrap().clone();
    let ask = acts::move_to(&root, &words, &book, &Place::Budget("leisure".into())).unwrap().ask.unwrap();
    acts::decide(&root, &ask, "leisure", Way::Out, true).unwrap();
    let ledger = Ledger::load(&root).unwrap();
    assert!(ledger.splits.iter().any(|s| s.words == vec!["librairie exemple".to_string()] && s.budget.as_deref() == Some("leisure")), "{:?}", ledger.splits);
    // Split between two budgets, from its row: in the file's [[assign]], in parts.
    let parts = vec![("home".to_string(), Money(-1000)), ("leisure".to_string(), Money(-2000))];
    acts::move_to(&root, &words, &read().find("bank:main/L1").unwrap().clone(), &Place::Parts(parts.clone())).unwrap();
    assert_eq!(read().find("bank:main/L1").unwrap().place, Place::Parts(parts));
    let _ = std::fs::remove_dir_all(&root);
}

/// "A payment…" from any mail (research §6, recommendation 12): a short form
/// filled from what it says, any budget, the mail its source; the same from
/// a letter; without a budget, nothing is written.
#[test]
fn a_payment_made_from_a_mail() {
    let root = folder("payment");
    std::fs::write(root.join(crate::budget::LEDGER), BUDGETS).unwrap();
    let words = crate::words::Words::builtin();
    let raw = "From: Shop Exemple <shop@shop.example>\r\nSubject: Votre reçu\r\nDate: Mon, 02 Nov 2026 10:00:00 +0100\r\nMessage-ID: <r1@shop.example>\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\nMerci de votre visite.\r\nTotal 42,50 €\r\n";
    let card = crate::card::Card::from_bytes(raw.as_bytes()).unwrap();
    let mut payment = acts::Payment::from_card(&words, &card, "mid:r1@shop.example", false);
    assert_eq!((payment.amount, payment.day.as_str(), payment.source.as_str()), (-42.5, "2026-11-02", "mid:r1@shop.example"));
    assert!(payment.record(&root, day("2026-11-04"), "From mail").is_err(), "a budget is needed");
    payment.budget = "leisure".into();
    let id = payment.record(&root, day("2026-11-04"), "From mail: Votre reçu").unwrap();
    let ledger = Ledger::load(&root).unwrap();
    let line = ledger.lines.iter().find(|l| l.id == id).unwrap();
    assert_eq!((line.amount, line.planned, line.links.clone()), (Money(-4250), false, vec!["mid:r1@shop.example".to_string()]));
    let all = load(&root, &words, day("2026-11-01"), day("2026-11-30"), day("2026-11-04"));
    let made = all.find(&format!("line:{id}")).unwrap();
    assert!(made.state == State::Announced && made.has("mid:r1@shop.example"), "{made:#?}");
    // A letter's bill: its amount to pay, its date asked.
    let letter = Letter { id: "2026-11-03-7c".into(), reading: crate::letters::Reading { sender: "Tax office".into(), amount: Some(Money(32000)), deadline: Some(day("2026-11-20")), ..Default::default() }, ..Letter::default() };
    let from_letter = acts::Payment::from_letter(&letter);
    assert_eq!((from_letter.amount, from_letter.day.as_str(), from_letter.source.as_str()), (-320.0, "2026-11-20", "sioul:letter/2026-11-03-7c"));
    let _ = std::fs::remove_dir_all(&root);
}

/// The budgets count each movement once (`counted`): the rent's bank line in
/// place of its preset; a receipt with its tip; a planned line the bank
/// settled, done; a card payment of its own.
#[test]
fn budgets_count_each_movement_once() {
    let ledger = ledger(&format!(
        "{BUDGETS}\
         [[preset]]\nid = 'rent'\nbudget = 'home'\nlabel = 'Rent'\namount = -600\nevery = 'month'\nday = 1\n\
         [[line]]\nid = 'shop'\nbudget = 'home'\ndate = 2026-10-01\namount = -23.98\nlabel = 'Shop'\nlinks = ['mid:shop@pay.example']\n\
         [[line]]\nid = 'plumber'\nbudget = 'home'\ndate = 2026-10-25\namount = -120\nlabel = 'Plumber'\nplanned = true\n\
         [[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home']\nexports = ['0123456A']\n"
    ));
    let mut bank = Bank::default();
    bank.movements = vec![
        line("0123456A", "A1", "2026-10-01", -60000, "PRLV SEPA RENT AGENCY"),
        line("0123456A", "A2", "2026-10-03", -2500, "CB SHOP"),
        line("0123456A", "A3", "2026-10-26", -12000, "CB PLUMBER"),
        line("0123456A", "A4", "2026-10-10", -1200, "CB CINEMA"),
    ];
    let today = day("2026-10-31");
    let all = derived(&ledger, &bank, &[], &[], &Store::default(), "2026-09-01", "2026-12-31", "2026-10-31");
    let counted = counted(&ledger, &all, today);
    let home = counted.budgets.iter().find(|b| b.id == "home").unwrap();
    let status = counted.status(home, today);
    assert_eq!((status.so_far, status.to_come), (Money(-60000 - 2500 - 12000 - 1200), Money::ZERO), "{:#?}", counted.lines);
    // The file's lines keep their places: their addresses hold.
    assert_eq!(counted.lines[..ledger.lines.len()].iter().map(|l| l.label.as_str()).collect::<Vec<_>>(), ["Shop", "Plumber"]);
}

/// A CSV export (PayPal's, in French) taken in twice: its pending row is a
/// hold, never a movement; the second time adds nothing, and its hold is
/// replaced, not doubled (research §5.3: holds; the bank's own ids).
#[test]
fn a_csv_taken_in_twice_adds_nothing_and_keeps_its_hold_apart() {
    let root = folder("csv");
    std::fs::write(root.join(crate::budget::LEDGER), format!("{BUDGETS}[[bank_account]]\nid = 'paypal'\ntitle = 'PayPal'\nkind = 'paypal'\nfills = ['leisure']\n")).unwrap();
    let csv = "\u{feff}\"Date\",\"Heure\",\"Fuseau horaire\",\"Nom\",\"Type\",\"État\",\"Devise\",\"Avant commission\",\"Commission\",\"Net\",\"Solde\",\"Numéro de transaction\"\n\
        \"02/11/2026\",\"10:02:11\",\"Europe/Paris\",\"Shop Exemple\",\"Paiement Express Checkout\",\"Terminé\",\"EUR\",\"-23,98\",\"0,00\",\"-23,98\",\"76,02\",\"T1\"\n\
        \"03/11/2026\",\"09:30:00\",\"Europe/Paris\",\"Station Exemple\",\"Paiement\",\"En attente\",\"EUR\",\"-45,00\",\"0,00\",\"-45,00\",\"76,02\",\"T2\"\n\
        \"03/11/2026\",\"09:40:00\",\"Europe/Paris\",\"Someone\",\"Paiement envoyé\",\"Refusé\",\"EUR\",\"-5,00\",\"0,00\",\"-5,00\",\"76,02\",\"T3\"\n";
    let words = crate::words::Words::builtin();
    let mut statement = crate::bank::read(&words.bank, csv).unwrap();
    statement.account = "paypal".into();
    assert_eq!((statement.movements.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), statement.pending.iter().map(|m| m.id.as_str()).collect::<Vec<_>>()), (vec!["T1"], vec!["T2"]), "refused left out, pending apart");
    assert_eq!(statement.balance, Some((day("2026-11-02"), Money(7602))), "a pending row sets no balance");
    let today = day("2026-11-04");
    assert_eq!(acts::take_in(&root, &statement, "PayPal", &words, today).unwrap(), acts::TakenIn { read: 1, new: 1, holds: 1, joined: 0 });
    assert_eq!(acts::take_in(&root, &statement, "PayPal", &words, today).unwrap(), acts::TakenIn { read: 1, new: 0, holds: 1, joined: 0 });
    assert_eq!((Bank::load(&root).unwrap().movements.len(), Store::load(&root).unwrap().holds.len()), (1, 1));
    let all = load(&root, &words, day("2026-11-01"), day("2026-11-30"), today);
    assert_eq!(all.find("hold:paypal/T2").map(|m| m.state), Some(State::NotFinal));
    let _ = std::fs::remove_dir_all(&root);
}

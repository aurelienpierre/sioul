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

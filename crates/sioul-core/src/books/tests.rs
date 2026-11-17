// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The books, on fictional businesses of example.org.

use super::*;
use crate::config::{ContributionSetting, TaxSetting, ThresholdSetting};
use crate::invoice::{Address, Cited, Client, Identifier, Line, Payment};
use crate::money::Money as Cents;
use crate::movements::{Input as MoneyInput, Store, derive_with_gone};

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

fn en() -> Translator {
    Translator::new("en")
}

/// A French EI micro-entrepreneur, BNC, as Settings says it.
fn french(scheme: &str) -> InvoiceSettings {
    InvoiceSettings {
        preset: "fr".into(),
        legal_form: "ei".into(),
        regime: "micro".into(),
        name: "Camille Exemple".into(),
        activity: vec!["bnc".into()],
        tax: vec![TaxSetting { from: String::new(), scheme: scheme.into(), mention: String::new() }],
        ..InvoiceSettings::default()
    }
}

fn client(name: &str, country: &str) -> Client {
    Client { kind: "business".into(), name: name.into(), address: Address { lines: vec!["1 Example Street".into()], postcode: "1000".into(), city: "Example".into(), country: country.into() }, ..Client::default() }
}

/// A line of `cents` of `nature`, taxed as `category` at `rate`.
fn line(description: &str, cents: i64, nature: Nature, category: &str, rate: i64) -> Line {
    Line { quantity: 1000, unit: "C62".into(), price: cents, base: 1, nature, ..Line::of_time(description, 0, 0, category, rate) }
}

/// An issued record, its totals computed, with its payments.
fn issued(number: &str, date: &str, who: Client, mut lines: Vec<Line>, scheme: &str, payments: &[(&str, i64, &str)]) -> Invoice {
    let totals = crate::invoice::totals::compute(&mut lines, 0, &|_, _| (String::new(), String::new()));
    Invoice {
        format: 2,
        kind: Kind::Invoice,
        number: number.into(),
        date: date.into(),
        period_to: date.into(),
        due: date.into(),
        currency: "EUR".into(),
        project: "lumen".into(),
        client: who,
        issuer: crate::invoice::Issuer { preset: "fr".into(), scheme: scheme.into(), ..crate::invoice::Issuer::default() },
        lines,
        totals,
        payments: payments.iter().enumerate().map(|(i, (date, amount, means))| Payment { id: format!("p-{number}-{i}"), date: (*date).into(), amount: *amount, means: (*means).into(), movement: String::new() }).collect(),
        ..Invoice::default()
    }
}

fn input<'a>(settings: &'a Settings, invoices: &'a [Invoice], booked: &'a BTreeMap<String, i64>, money: Option<&'a Money>) -> Input<'a> {
    Input { settings, invoices, booked, money, budget: None }
}

/// The money of a work budget that declares: a bank line that paid
/// invoice 2026-002 (matched, no payment written on it), a platform's
/// payout no invoice describes, and a line that paid a format-1 invoice
/// ticked paid in its time.
fn money() -> Money {
    let ledger: Ledger = toml::from_str(
        "[[budget]]\nid = 'work'\ntitle = 'Work'\nperiod = 'month'\narea = 'work'\nset_aside = 22\n\
         [[bank_account]]\nid = 'main'\ntitle = 'My bank'\nfills = ['work']\nexports = ['0123456A']\n\
         [[line]]\nid = 'b'\nbudget = 'work'\ndate = 2026-11-30\namount = 300\nlabel = 'Client B'\nplanned = true\nlinks = ['sioul:invoice/2026-002']\n\
         [[line]]\nid = 'c'\nbudget = 'work'\ndate = 2026-10-20\namount = 120\nlabel = 'Workshop seats'\n\
         [[line]]\nid = 'o'\nbudget = 'work'\ndate = 2026-12-30\namount = 450\nlabel = 'Old client'\nplanned = true\nlinks = ['sioul:invoice/2026-001']\n",
    )
    .unwrap();
    let mut bank = crate::bank::Bank::default();
    bank.movements.push(crate::bank::Movement { account: "0123456A".into(), date: day("2026-11-10"), amount: Cents(30000), label: "VIR CLIENT B FACTURE 2026-002".into(), id: "B1".into() });
    bank.movements.push(crate::bank::Movement { account: "0123456A".into(), date: day("2026-12-28"), amount: Cents(45000), label: "VIR OLD CLIENT FACTURE 2026-001".into(), id: "B2".into() });
    let store = Store::default();
    let words = crate::words::Words::builtin();
    let input = MoneyInput { ledger: &ledger, bank: &bank, contracts: &[], letters: &[], store: &store, words: &words };
    let (movements, gone) = derive_with_gone(&input, day("2026-01-01"), day("2027-03-31"), day("2027-01-10"));
    Money { counted: ledger.clone(), ledger, bank, movements, gone, from: day("2026-01-01"), to: day("2027-03-31") }
}

/// A work budget that declares, its bank account, and these bank lines (day, cents, words, id).
fn money_with(lines: &str, bank_lines: &[(&str, i64, &str, &str)]) -> Money {
    let ledger: Ledger = toml::from_str(&format!(
        "[[budget]]\nid = 'work'\ntitle = 'Work'\nperiod = 'month'\narea = 'work'\nset_aside = 22\n\
         [[bank_account]]\nid = 'main'\ntitle = 'My bank'\nfills = ['work']\nexports = ['0123456A']\n{lines}"
    ))
    .unwrap();
    let mut bank = crate::bank::Bank::default();
    for (date, cents, label, id) in bank_lines {
        bank.movements.push(crate::bank::Movement { account: "0123456A".into(), date: day(date), amount: Cents(*cents), label: (*label).into(), id: (*id).into() });
    }
    let store = Store::default();
    let words = crate::words::Words::builtin();
    let input = MoneyInput { ledger: &ledger, bank: &bank, contracts: &[], letters: &[], store: &store, words: &words };
    let (movements, gone) = derive_with_gone(&input, day("2026-01-01"), day("2027-03-31"), day("2027-01-10"));
    Money { counted: ledger.clone(), ledger, bank, movements, gone, from: day("2026-01-01"), to: day("2027-03-31") }
}

/// Every euro of a bank line counts once (the review of phase 4, P4-3,
/// P4-12): a payment in another currency said in euros joins the bank's
/// line of those euros; what a line brought beyond the invoice it paid (an
/// overpayment) is a receipt of its own, "beyond" that invoice.
#[test]
fn every_euro_of_a_bank_line_counts_once() {
    let settings = Settings::of(&french("franchise"), None);
    let tr = en();
    // A USD invoice, its payment said as €910, and the bank's €910 the next day.
    let money = money_with("", &[("2026-12-16", 91_000, "VIR EXAMPLE INC", "U1")]);
    let usd = vec![Invoice { currency: "USD".into(), ..issued("2026-130", "2026-12-04", client("Example Inc.", "US"), vec![line("Consulting", 100_000, Nature::Service, "O", 0)], "franchise", &[("2026-12-15", 100_000, "transfer")]) }];
    let booked: BTreeMap<String, i64> = [("p-2026-130-0".to_string(), 91_000)].into();
    let rows = rows(&input(&settings, &usd, &booked, Some(&money)), day("2026-12-01"), day("2026-12-31"), &tr);
    assert_eq!(rows.iter().map(|r| (r.day.to_string(), r.amount)).collect::<Vec<_>>(), [("2026-12-15".to_string(), Some(91_000))], "one row of €910, not two");
    // The client pays €1,250 for an invoice of €1,200: the €50 is a receipt of its own.
    let money = money_with("", &[("2026-11-10", 125_000, "VIR STUDIO LUMEN FACTURE 2026-131", "O1")]);
    let address = money.movements.iter().find(|m| m.label.contains("2026-131")).map(|m| m.address()).unwrap();
    let mut paid = issued("2026-131", "2026-11-02", client("Studio Lumen", "FR"), vec![line("Design", 120_000, Nature::Service, "E", 0)], "franchise", &[]);
    paid.payments = vec![Payment { id: "b1".into(), date: "2026-11-10".into(), amount: 120_000, means: "transfer".into(), movement: address }];
    let none = BTreeMap::new();
    let rows = super::rows(&input(&settings, &[paid], &none, Some(&money)), day("2026-11-01"), day("2026-11-30"), &tr);
    assert_eq!(rows.iter().map(|r| (r.invoice.as_str(), r.amount, r.beyond.clone())).collect::<Vec<_>>(), [("", Some(5_000), vec!["2026-131".to_string()]), ("2026-131", Some(120_000), Vec::new())]);
}

/// The franchise counts what the book counts (the review of phase 4,
/// P4-14): a bank line the matching tied to an invoice with no payment
/// written; a format-1 record that said no category counts as located here.
#[test]
fn the_franchise_counts_what_the_book_counts() {
    let settings = Settings::of(&french("franchise"), None);
    let tr = en();
    let money = money();
    let tied = vec![issued("2026-002", "2026-11-02", client("Studio Lumen", "FR"), vec![line("Design", 30_000, Nature::Service, "E", 0)], "franchise", &[])];
    let none = BTreeMap::new();
    let w = thresholds::watch(&input(&settings, &tied, &none, Some(&money)), day("2026-12-31"), &tr);
    let services = w.iter().find(|w| w.id == "franchise-services").unwrap();
    assert!(services.turnover >= 30_000, "the €300 the bank's line paid counts: {services:?}");
    let mut older = issued("2026-003", "2026-06-02", client("Studio Lumen", "FR"), vec![line("Design", 40_000, Nature::Service, "O", 0)], "none", &[("2026-06-20", 40_000, "")]);
    older.format = 1;
    let w = thresholds::watch(&input(&settings, &[older], &none, None), day("2026-12-31"), &tr);
    assert_eq!(w.iter().find(|w| w.id == "franchise-services").map(|w| w.turnover), Some(40_000), "a format-1 record's line, no category said, is located here");
}

/// A deposit counts once, as what it pays for (the review of phase 4, P4-2,
/// P4-7): the DES lists a deposit and its final invoice for the final's
/// lines in all; a deposit on goods is declared under sales, and the
/// franchise never counts more than what was sold.
#[test]
fn a_deposit_counts_once_as_what_it_pays_for() {
    let tr = en();
    let none = BTreeMap::new();
    let gmbh = Client { ids: vec![Identifier { kind: "vat".into(), value: "DE123456789".into(), printed: true }], ..client("Beispiel GmbH", "DE") };
    let franchise = Settings::of(&french("franchise"), None);
    let deposit = Invoice { kind: Kind::Deposit, ..issued("2026-140", "2026-10-05", gmbh.clone(), vec![line("Deposit", 300_000, Nature::Service, "AE", 0)], "franchise", &[("2026-10-06", 300_000, "transfer")]) };
    let mut last = issued("2026-147", "2026-11-20", gmbh, vec![line("Consulting", 1_000_000, Nature::Service, "AE", 0)], "franchise", &[]);
    last.deposits = vec![Cited { number: "2026-140".into(), date: "2026-10-05".into(), amount: 300_000 }];
    last.totals.deposits = 300_000;
    last.totals.due -= 300_000;
    let invoices = vec![deposit, last];
    let months = des::between(&input(&franchise, &invoices, &none, None), day("2026-10-01"), day("2026-11-30"));
    assert_eq!(months.iter().map(|d| d.total).sum::<i64>(), 1_000_000, "the deposit and the final invoice: the final's lines, once");
    // A seller of goods: its deposit is a sale, and the count never passes what was sold.
    let seller = Settings::of(&InvoiceSettings { activity: vec!["goods-bic".into(), "services-bic".into()], ..french("franchise") }, None);
    let deposit = Invoice { kind: Kind::Deposit, ..issued("2026-150", "2026-03-09", client("Studio Lumen", "FR"), vec![line("Deposit", 300_000, Nature::Service, "E", 0)], "franchise", &[("2026-03-10", 300_000, "transfer")]) };
    let mut furniture = issued("2026-151", "2026-06-01", client("Studio Lumen", "FR"), vec![line("Furniture", 1_000_000, Nature::Goods, "E", 0)], "franchise", &[("2026-06-20", 700_000, "transfer")]);
    furniture.deposits = vec![Cited { number: "2026-150".into(), date: "2026-03-09".into(), amount: 300_000 }];
    furniture.totals.deposits = 300_000;
    furniture.totals.due -= 300_000;
    let invoices = vec![deposit, furniture];
    let i = input(&seller, &invoices, &none, None);
    let q1 = declarations::holding(&i, day("2026-03-10"), &tr).unwrap();
    assert_eq!(q1.lines.iter().map(|l| (l.activity.as_str(), l.turnover)).collect::<Vec<_>>(), [("goods-bic", 300_000), ("services-bic", 0)], "{q1:?}");
    for today in ["2026-06-01", "2026-06-10", "2026-06-20", "2026-12-31"] {
        let w = thresholds::watch(&i, day(today), &tr);
        assert!(w.iter().find(|w| w.id == "franchise-goods").unwrap().turnover <= 1_000_000, "{today}: {w:?}");
    }
}

/// A line's activity is apart from its nature (the review of phase 4,
/// P4-13): meals eaten on the spot are services for VAT and sales for
/// URSSAF and the thresholds.
#[test]
fn a_meal_on_the_spot_is_a_sale() {
    let tr = en();
    let none = BTreeMap::new();
    let snack = Settings::of(&InvoiceSettings { activity: vec!["goods-bic".into()], ..french("franchise") }, None);
    let meals = vec![issued("2026-160", "2026-11-02", Client { kind: "consumer".into(), ..client("Robin Example", "FR") }, vec![line("Lunches", 400_000, Nature::Service, "E", 0)], "franchise", &[("2026-11-02", 400_000, "card")])];
    let i = input(&snack, &meals, &none, None);
    let q4 = declarations::holding(&i, day("2026-11-02"), &tr).unwrap();
    assert_eq!((q4.lines[0].activity.as_str(), q4.lines[0].turnover, q4.unsaid), ("goods-bic", 400_000, 0));
    assert_eq!(q4.lines[0].social.map(|p| p.rate), Some(1230));
    let w = thresholds::watch(&i, day("2026-11-30"), &tr);
    assert_eq!(w.iter().find(|w| w.id == "franchise-goods").map(|w| w.turnover), Some(400_000));
    assert!(w.iter().all(|w| w.id != "franchise-services"), "no services sold: no services' threshold");
    // Said on the line, whatever the other activities.
    let both = Settings::of(&InvoiceSettings { activity: vec!["goods-bic".into(), "services-bic".into()], ..french("franchise") }, None);
    let mut said = meals[0].clone();
    said.lines[0].activity = "goods-bic".into();
    let q4 = declarations::holding(&input(&both, &[said], &none, None), day("2026-11-02"), &tr).unwrap();
    assert_eq!(q4.lines.iter().map(|l| (l.activity.as_str(), l.turnover)).collect::<Vec<_>>(), [("goods-bic", 400_000), ("services-bic", 0)]);
}

/// A credit note goes to the DES of the month it is issued, below zero, its
/// month said to be the customs' guide's; an invoice in another currency is
/// listed, not added, and said (the review of phase 4, P4-18, P4-17).
#[test]
fn the_des_lists_a_credit_note_in_its_own_month() {
    let settings = Settings::of(&french("franchise"), None);
    let none = BTreeMap::new();
    let tr = en();
    let gmbh = Client { ids: vec![Identifier { kind: "vat".into(), value: "DE123456789".into(), printed: true }], ..client("Beispiel GmbH", "DE") };
    let mut october = issued("2026-170", "2026-10-30", gmbh.clone(), vec![line("Consulting", 120_000, Nature::Service, "AE", 0)], "franchise", &[]);
    october.period_to = "2026-10-30".into();
    let credit = Invoice { kind: Kind::CreditNote, corrects: Some(Cited { number: "2026-170".into(), date: "2026-10-30".into(), amount: 120_000 }), period_to: "2026-10-30".into(), ..issued("2026-175", "2026-12-05", gmbh.clone(), vec![line("Consulting", 20_000, Nature::Service, "AE", 0)], "franchise", &[]) };
    let dollars = Invoice { currency: "USD".into(), ..issued("2026-176", "2026-12-07", gmbh, vec![line("Consulting", 50_000, Nature::Service, "AE", 0)], "franchise", &[]) };
    let all = des::between(&input(&settings, &[october, credit, dollars], &none, None), day("2026-10-01"), day("2026-12-31"));
    assert_eq!(all.iter().map(|d| (d.key(), d.total, d.foreign)).collect::<Vec<_>>(), [("2026-10".to_string(), 120_000, 0), ("2026-12".to_string(), -20_000, 1)]);
    let said = des::sentences(&tr, &all[1], "EUR");
    assert_eq!(said[0], "Services to businesses in other EU countries, December 2026 (DES): −€200, and one invoice in another currency not counted, to say in euros; to declare by 14 January 2027.");
    assert!(said.iter().any(|s| s.starts_with("2026-175 credits an invoice of another month")), "{said:?}");
}

/// A credit note for what the client never paid needs no refund: it is
/// never named as waiting for one; one on an invoice paid in full is (the
/// review of phase 4, P4-10).
#[test]
fn a_credit_note_waits_for_a_refund_only_when_money_is_owed_back() {
    let settings = Settings::of(&french("liable"), None);
    let none = BTreeMap::new();
    let disputed = issued("2026-180", "2026-10-10", client("Studio Lumen", "FR"), vec![line("Design", 100_000, Nature::Service, "S", 2000)], "liable", &[("2026-11-04", 96_000, "transfer")]);
    let credit = Invoice { kind: Kind::CreditNote, corrects: Some(Cited { number: "2026-180".into(), date: "2026-10-10".into(), amount: 120_000 }), ..issued("2026-181", "2026-11-02", client("Studio Lumen", "FR"), vec![line("Design", 20_000, Nature::Service, "S", 2000)], "liable", &[]) };
    let months = vat::between(&input(&settings, &[disputed.clone(), credit.clone()], &none, None), day("2026-11-01"), day("2026-11-30"));
    assert!(months[0].credits_not_counted.is_empty(), "the client paid what was left: nothing to give back");
    let paid_in_full = Invoice { payments: vec![Payment { id: "x".into(), date: "2026-10-20".into(), amount: 120_000, means: "transfer".into(), movement: String::new() }], ..disputed };
    let months = vat::between(&input(&settings, &[paid_in_full, credit], &none, None), day("2026-11-01"), day("2026-11-30"));
    assert_eq!(months[0].credits_not_counted, ["2026-181"]);
}

/// The receipts book (design §6): one row per payment received, on its
/// day, its invoice's client, nature, number and means, cash said apart;
/// a bank line the matching tied to an invoice without a payment written
/// counts once, its client the invoice's; a format-1 invoice ticked paid
/// takes its bank line's day; a receipt no invoice describes stays as the
/// register had it; a payment in another currency counts once what it
/// came to here is said; quarter and year totals; CSV in French, a page.
#[test]
fn the_book_follows_payments() {
    // Sales of prints and liberal services: goods and services apart.
    let settings = Settings::of(&InvoiceSettings { activity: vec!["goods-bic".into(), "bnc".into()], ..french("franchise") }, None);
    let mut older = issued("2026-001", "2026-12-01", client("Old Client", "FR"), vec![line("Photos", 45_000, Nature::Service, "E", 0)], "franchise", &[]);
    // A format-1 record's own "paid", read on its due day by the life.
    older.payments = vec![Payment { id: "older:2026-001".into(), date: "2026-12-01".into(), amount: 45_000, ..Payment::default() }];
    let invoices = vec![
        older,
        issued("2026-002", "2026-11-02", client("Studio Lumen", "FR"), vec![line("Design", 30_000, Nature::Service, "E", 0)], "franchise", &[]),
        issued("2026-003", "2026-11-05", client("Atelier Nord", "FR"), vec![line("Prints", 20_000, Nature::Goods, "E", 0), line("Framing", 10_000, Nature::Service, "E", 0)], "franchise", &[("2026-11-20", 15_000, "cash"), ("2026-12-03", 15_000, "cheque")]),
        Invoice { currency: "USD".into(), ..issued("2026-004", "2026-12-04", client("Example Inc.", "US"), vec![line("Consulting", 150_000, Nature::Service, "O", 0)], "franchise", &[("2026-12-15", 100_000, "transfer"), ("2026-12-20", 50_000, "transfer")]) },
    ];
    let booked: BTreeMap<String, i64> = [("p-2026-004-0".to_string(), 91_000)].into();
    let money = money();
    let tr = en();
    let book = book(&input(&settings, &invoices, &booked, Some(&money)), day("2026-10-01"), day("2026-12-31"), &tr);
    let rows: Vec<(String, &str, &str, &str, Option<i64>, bool)> = book.rows.iter().map(|r| (r.day.to_string(), r.client.as_str(), r.invoice.as_str(), r.nature.as_str(), r.amount, r.cash)).collect();
    assert_eq!(
        rows,
        vec![
            ("2026-10-20".into(), "Workshop seats", "", "", Some(12_000), false),
            ("2026-11-10".into(), "Studio Lumen", "2026-002", "service", Some(30_000), false),
            ("2026-11-20".into(), "Atelier Nord", "2026-003", "both", Some(15_000), true),
            ("2026-12-03".into(), "Atelier Nord", "2026-003", "both", Some(15_000), false),
            ("2026-12-15".into(), "Example Inc.", "2026-004", "service", Some(91_000), false),
            ("2026-12-20".into(), "Example Inc.", "2026-004", "service", None, false),
            ("2026-12-28".into(), "Old Client", "2026-001", "service", Some(45_000), false),
        ],
        "{book:#?}"
    );
    // Goods and services apart: two thirds of Atelier Nord's are prints.
    assert_eq!((book.rows[2].goods, book.rows[2].services()), (10_000, 5_000));
    assert_eq!(book.rows[5].foreign, Some((50_000, "USD".into())));
    let q4 = book.totals.iter().find(|t| t.kind == "quarter").unwrap();
    assert_eq!((q4.amount, q4.without_tax, q4.goods, q4.services, q4.foreign, q4.at_least), (208_000, 208_000, 20_000, 176_000, 1, false), "€120 of no nature said");
    let year = book.totals.iter().find(|t| t.kind == "year").unwrap();
    assert_eq!((year.from, year.to, year.amount), (day("2026-01-01"), day("2026-12-31"), 208_000));
    assert_eq!(sentences(&tr, &book)[0], "Fourth quarter 2026: €2,080 received.");
    assert!(sentences(&tr, &book).last().unwrap().starts_with("One payment in USD is not counted"), "{:?}", sentences(&tr, &book));
    // The book made in French, as the window makes it in its language.
    let fr = Translator::new("fr");
    let csv = csv(&fr, &super::book(&input(&settings, &invoices, &booked, Some(&money)), day("2026-10-01"), day("2026-12-31"), &fr));
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "Jour;Client;Nature;Facture;Moyen de paiement;Espèces;Encaissé;Biens;Services;Note");
    assert_eq!(lines[1], "2026-10-20;Workshop seats;;;écrit à la main;;120,00;;;", "a receipt no invoice describes: goods and services not said, {csv}");
    assert_eq!(lines[3], "2026-11-20;Atelier Nord;Livraisons de biens et prestations de services;2026-003;Espèces;espèces;150,00;100,00;50,00;", "{csv}");
    assert!(lines.iter().any(|l| l.contains("Quatrième trimestre 2026") && l.contains("2080,00")), "{csv}");
    assert!(lines.last().unwrap().contains("Année 2026"), "{csv}");
    assert!(html(&tr, &book, "Camille Exemple").contains("Receipts book of Camille Exemple, 1 October 2026 to 31 December 2026"));
    // Another budget's movements only, as `sioul receipts --budget` asks: the
    // invoices' payments alone, the format-1 one on the day its life gives.
    let only = Input { budget: Some("none"), ..input(&settings, &invoices, &booked, Some(&money)) };
    let alone = rows_of(&only, &tr);
    assert_eq!(alone.len(), 5);
    assert_eq!(alone[1].day, day("2026-12-01"));
}

fn rows_of(input: &Input, tr: &Translator) -> Vec<Row> {
    rows(input, day("2026-10-01"), day("2026-12-31"), tr)
}

/// Under VAT, a row keeps both amounts, and the CSV both columns.
#[test]
fn a_taxed_row_keeps_both_amounts() {
    let settings = Settings::of(&french("liable"), None);
    let invoices = vec![issued("2026-010", "2026-11-02", client("Studio Lumen", "FR"), vec![line("Design", 100_000, Nature::Service, "S", 2000)], "liable", &[("2026-11-15", 60_000, "transfer")])];
    let tr = en();
    let book = book(&input(&settings, &invoices, &BTreeMap::new(), None), day("2026-11-01"), day("2026-11-30"), &tr);
    assert_eq!((book.rows[0].amount, book.rows[0].without_tax), (Some(60_000), Some(50_000)));
    assert!(taxed(&book));
    assert_eq!(csv(&tr, &book).lines().next().unwrap(), "Day,Client,Nature,Invoice,Means of payment,Cash,Received,Excluding VAT,Note");
    assert_eq!(sentences(&tr, &book)[0], "Fourth quarter 2026, until 30 November 2026: €600 received with VAT, €500 excluding it.");
}

/// Declarations (design §6): per period and activity, the turnover
/// received excluding VAT, contributions at the rates of 2026 (BNC 25.6 %,
/// VL 2.2 %, CFP 0.2 %), the day to declare by moved off a weekend (31
/// January 2027 is a Sunday), zero periods said; a rate of your own (ACRE's)
/// over the preset's while it holds.
#[test]
fn declarations_per_period_and_activity() {
    // Begun in 2025: every period of 2026 is said, zero ones too.
    let s = InvoiceSettings { vl: true, started: "2025-01-01".into(), ..french("franchise") };
    let settings = Settings::of(&s, None);
    assert_eq!((settings.every, settings.trade.as_str()), (Every::Quarter, "liberal"));
    let invoices = vec![issued("2026-020", "2026-10-02", client("Studio Lumen", "FR"), vec![line("Design", 694_000, Nature::Service, "E", 0)], "franchise", &[("2026-11-10", 694_000, "transfer")])];
    let none = BTreeMap::new();
    let tr = en();
    let i = input(&settings, &invoices, &none, None);
    let q4 = declarations::holding(&i, day("2026-11-15"), &tr).unwrap();
    assert_eq!((q4.from, q4.to, q4.opens, q4.due, q4.key()), (day("2026-10-01"), day("2026-12-31"), day("2027-01-01"), day("2027-02-01"), "2026-Q4".to_string()));
    assert_eq!(q4.lines[0].social, Some(declarations::Part { rate: 2560, amount: 177_664 }));
    assert_eq!(q4.lines[0].vl, Some(declarations::Part { rate: 220, amount: 15_268 }));
    assert_eq!(q4.cfp, Some(declarations::Part { rate: 20, amount: 1_388 }));
    assert_eq!(q4.contributions, 177_664 + 15_268 + 1_388);
    let said = declarations::sentences(&tr, &q4, "EUR");
    assert_eq!(said[0], "From 1 October 2026 to 31 December 2026: €6,940 received excluding VAT, to declare by 1 February 2027.");
    assert_eq!(said[1], "Other services, liberal (BNC): €6,940; contributions about €1,776.64 (25.6%). Income tax paid with them (versement libératoire): about €152.68 (2.2%).");
    let q3 = declarations::holding(&i, day("2026-08-01"), &tr).unwrap();
    assert_eq!(declarations::sentences(&tr, &q3, "EUR")[0], "Nothing received from 1 July 2026 to 30 September 2026: declare €0 by 2 November 2026.");
    let fr = Translator::new("fr");
    assert_eq!(declarations::sentences(&fr, &q3, "EUR")[0], "Rien d’encaissé du 1er juillet 2026 au 30 septembre 2026\u{202f}: déclarez 0\u{a0}€ au plus tard le 2 novembre 2026.");
    // Monthly: due the last day of the next month, off a weekend.
    let monthly = Settings::of(&InvoiceSettings { declare: "month".into(), ..s.clone() }, None);
    let m = declarations::holding(&input(&monthly, &invoices, &none, None), day("2026-11-10"), &tr).unwrap();
    assert_eq!((m.key(), m.due, m.turnover), ("2026-11".to_string(), day("2026-12-31"), 694_000));
    // ACRE's rate, your own, from the start until the end of the third quarter after.
    let acre = Settings::of(&InvoiceSettings { contributions: vec![ContributionSetting { kind: "social".into(), activity: "bnc".into(), rate: 12.8, from: "2026-03-01".into(), until: "2026-12-31".into() }], ..s.clone() }, None);
    let q4 = declarations::holding(&input(&acre, &invoices, &none, None), day("2026-11-15"), &tr).unwrap();
    assert_eq!(q4.lines[0].social, Some(declarations::Part { rate: 1280, amount: 88_832 }));
    // The quarters of the year until today's, each said.
    let year = declarations::of_year(&i, 2026, day("2026-11-17"), &tr);
    assert_eq!(year.iter().map(|d| d.key()).collect::<Vec<_>>(), ["2026-Q1", "2026-Q2", "2026-Q3", "2026-Q4"]);
    // The budgets' Overview says the same, from the books.
    assert_eq!(declarations::overview_line(&i, day("2026-11-17"), &tr).as_deref(), Some("Turnover received this quarter, excluding VAT: €6,940, to declare by 1 February 2027."));
    // For the Porch: Q3's, opened on 1 October, until 60 days after its day.
    assert_eq!(declarations::open_now(&i, day("2026-11-17"), 60, &tr).iter().map(|d| d.key()).collect::<Vec<_>>(), ["2026-Q3"]);
    assert!(declarations::open_now(&i, day("2027-01-05"), 60, &tr).iter().any(|d| d.key() == "2026-Q4"));
}

/// A mixed activity splits each payment by its lines' nature; an activity
/// not said leaves its turnover counted, without contributions, and says so.
#[test]
fn a_mixed_activity_splits_by_nature() {
    let s = InvoiceSettings { activity: vec!["goods-bic".into(), "services-bic".into()], ..french("franchise") };
    let settings = Settings::of(&s, None);
    assert_eq!(settings.trade, "", "BIC services may be commercial or craft: only you can say");
    let invoices = vec![issued("2026-030", "2026-11-02", client("Atelier Nord", "FR"), vec![line("Prints", 30_000, Nature::Goods, "E", 0), line("Framing", 10_000, Nature::Service, "E", 0)], "franchise", &[("2026-11-10", 40_000, "card")])];
    let none = BTreeMap::new();
    let tr = en();
    let d = declarations::holding(&input(&settings, &invoices, &none, None), day("2026-11-10"), &tr).unwrap();
    assert_eq!(d.lines.iter().map(|l| (l.activity.as_str(), l.turnover)).collect::<Vec<_>>(), [("goods-bic", 30_000), ("services-bic", 10_000)]);
    assert_eq!(d.lines[0].social.unwrap().amount, 3_690, "12.3 % of €300");
    assert!(declarations::sentences(&tr, &d, "EUR").iter().any(|l| l.starts_with("The training contribution is not counted")));
    let unsaid = Settings::of(&InvoiceSettings { activity: Vec::new(), ..s }, None);
    let d = declarations::holding(&input(&unsaid, &invoices, &none, None), day("2026-11-10"), &tr).unwrap();
    assert_eq!((d.turnover, d.unsaid, d.contributions), (40_000, 40_000, 0));
}

/// The first declaration gathers the start's period and the ones after it
/// (the review of phase 4, P4-1; URSSAF's tables, justice.fr's fiche
/// "Micro-entrepreneur : déclarer son chiffre d'affaires", propulsebyca.fr's
/// monthly table): monthly, the start month and the three after, due the
/// last day of the fourth month after the start month, moved off a weekend;
/// quarterly, the start quarter and the next.
#[test]
fn the_first_declaration_gathers_its_first_periods() {
    let monthly = |started: &str| Settings::of(&InvoiceSettings { declare: "month".into(), started: started.into(), ..french("franchise") }, None);
    let first = |started: &str| {
        let settings = monthly(started);
        let (from, to, gathered) = declarations::schedule(&settings, day("2020-01-01"), day("2030-12-31"))[0];
        (from, to, gathered, declarations::due_of(&settings, to))
    };
    // January: January to April, due 31 May (a Sunday in 2026: 1 June).
    assert_eq!(first("2026-01-15"), (day("2026-01-15"), day("2026-04-30"), true, day("2026-06-01")));
    // March (justice.fr: a start on 6 March 2026, by 31 July 2026): March to June.
    assert_eq!(first("2026-03-10"), (day("2026-03-10"), day("2026-06-30"), true, day("2026-07-31")));
    assert_eq!(first("2026-03-06").3, day("2026-07-31"));
    // May: due 30 September.
    assert_eq!(first("2026-05-02"), (day("2026-05-02"), day("2026-08-31"), true, day("2026-09-30")));
    // October: due the last day of February (28 February 2027 is a Sunday: 1 March).
    assert_eq!(first("2026-10-15"), (day("2026-10-15"), day("2027-01-31"), true, day("2027-03-01")));
    // A leap year: January 2028, due 31 May 2028; October 2027, due 29 February 2028.
    assert_eq!(first("2028-01-15").3, day("2028-05-31"));
    assert_eq!(first("2027-10-04"), (day("2027-10-04"), day("2028-01-31"), true, day("2028-02-29")));
    // Then each month as any.
    let s = declarations::schedule(&monthly("2026-03-10"), day("2026-01-01"), day("2026-08-10"));
    assert_eq!(s[1..].iter().map(|p| (p.0.to_string(), p.2)).collect::<Vec<_>>(), [("2026-07-01".to_string(), false), ("2026-08-01".to_string(), false)]);
    let quarterly = |started: &str| Settings::of(&InvoiceSettings { started: started.into(), ..french("franchise") }, None);
    let s = declarations::schedule(&quarterly("2026-02-10"), day("2026-01-01"), day("2026-11-17"));
    assert_eq!(s[0], (day("2026-02-10"), day("2026-06-30"), true));
    assert_eq!(declarations::due_of(&quarterly("2026-02-10"), day("2026-06-30")), day("2026-07-31"));
    assert_eq!(s.iter().map(|p| p.0.to_string()).collect::<Vec<_>>(), ["2026-02-10", "2026-07-01", "2026-10-01"]);
    // justice.fr: quarterly, a start on 12 April 2026, by 31 October 2026 (a Saturday: 2 November).
    let s = declarations::schedule(&quarterly("2026-04-12"), day("2026-01-01"), day("2026-12-31"));
    assert_eq!(s[0], (day("2026-04-12"), day("2026-09-30"), true));
    assert_eq!(declarations::due_of(&quarterly("2026-04-12"), day("2026-09-30")), day("2026-11-02"));
    // Nothing before the start, nothing declared under another regime.
    assert!(declarations::schedule(&quarterly("2026-02-10"), day("2025-01-01"), day("2025-12-31")).is_empty());
    let reel = Settings::of(&InvoiceSettings { regime: "reel".into(), ..french("franchise") }, None);
    assert!(!reel.declares());
}

/// The books begin with what Sioul knows (the review of phase 4, P4-17):
/// without a start of activity said, no period before the first invoice
/// issued is said, not even "declare €0"; nothing known, nothing said. A
/// period with more given back than received declares €0, and says the
/// rest is not taken off. A total cut at the day shown says so. A setting
/// that does not read is said.
#[test]
fn the_books_begin_with_what_sioul_knows() {
    let settings = Settings::of(&french("franchise"), None);
    let none = BTreeMap::new();
    let tr = en();
    assert!(declarations::open_now(&input(&settings, &[], &none, None), day("2026-11-17"), 60, &tr).is_empty(), "nothing known: nothing said");
    let invoices = vec![issued("2026-110", "2026-10-02", client("Studio Lumen", "FR"), vec![line("Design", 50_000, Nature::Service, "E", 0)], "franchise", &[("2026-10-20", 50_000, "transfer")])];
    let i = input(&settings, &invoices, &none, None);
    assert_eq!(begin(&i), Some(day("2026-10-02")));
    assert!(declarations::holding(&i, day("2026-08-01"), &tr).is_none(), "the third quarter, before anything was known, says nothing");
    assert_eq!(declarations::of_year(&i, 2026, day("2026-11-17"), &tr).iter().map(|d| d.key()).collect::<Vec<_>>(), ["2026-Q4"]);
    // More given back than received: €0 declared, the rest said.
    let refund = Invoice { kind: Kind::CreditNote, ..issued("2027-001", "2027-01-05", client("Studio Lumen", "FR"), vec![line("Design", 50_000, Nature::Service, "E", 0)], "franchise", &[("2027-01-06", 50_000, "transfer")]) };
    let both = vec![invoices[0].clone(), refund];
    let q1 = declarations::holding(&input(&settings, &both, &none, None), day("2027-02-01"), &tr).unwrap();
    assert_eq!((q1.turnover, q1.declared, q1.contributions), (-50_000, 0, 0));
    assert_eq!(declarations::sentences(&tr, &q1, "EUR")[0], "From 1 January 2027 to 31 March 2027: €0 to declare by 30 April 2027; €500 given back beyond what was received is not taken off: Sioul carries nothing over to another period.");
    assert_eq!(declarations::sentences(&tr, &q1, "EUR")[1], "Other services, liberal (BNC): −€500, more given back than received: €0 declared.");
    // A total cut at the day shown.
    let book = book(&i, day("2026-10-01"), day("2026-10-31"), &tr);
    assert_eq!(total_name(&tr, &book.totals[0]), "Fourth quarter 2026, until 31 October 2026");
    // A start that does not read is said, not dropped.
    let odd = Settings::of(&InvoiceSettings { started: "2026-6-12".into(), ..french("franchise") }, None);
    assert_eq!(unread_sentences(&tr, &odd), ["In [invoice], started = “2026-6-12” does not read, and is left aside: a day is written 2026-06-12, a period month or quarter, a trade commerce, craft or liberal."]);
}

/// A payment in another currency not said in euros is never "nothing
/// received" (the review of phase 4, P4-9): the period says what came in
/// and asks its euros, on the Porch's note and the Overview too, its amount "at least".
#[test]
fn a_foreign_payment_is_never_nothing_received() {
    let settings = Settings::of(&french("franchise"), None);
    let invoices = vec![Invoice { currency: "USD".into(), ..issued("2026-120", "2026-12-04", client("Example Inc.", "US"), vec![line("Consulting", 100_000, Nature::Service, "O", 0)], "franchise", &[("2026-12-15", 100_000, "transfer")]) }];
    let none = BTreeMap::new();
    let tr = en();
    let i = input(&settings, &invoices, &none, None);
    let q4 = declarations::holding(&i, day("2026-12-20"), &tr).unwrap();
    assert!(q4.least() && q4.declared == 0);
    assert_eq!(declarations::sentences(&tr, &q4, "EUR")[0], "From 1 October 2026 to 31 December 2026: $1,000 received: say what it came to in euros; to declare by 1 February 2027.");
    assert_eq!(declarations::porch_note(&tr, &q4, "EUR"), "$1,000 received: say what it came to in euros (sioul invoices received).");
    assert_eq!(declarations::overview_line(&i, day("2026-12-20"), &tr).as_deref(), Some("Turnover received this quarter, excluding VAT: at least €0, to declare by 1 February 2027. $1,000 received: say what it came to in euros (sioul invoices received)."));
}

/// The VAT summary (design §6): services' VAT on the payments received,
/// goods' on the invoice's day, a deposit's on its payment, a final
/// invoice counting what its deposit did not; the reverse charge said with
/// no tax; a credit note on goods taken off on its day, one on services
/// said, not counted; services on the invoice's day under the option for debits.
#[test]
fn the_vat_summary_follows_when_vat_is_due() {
    let settings = Settings::of(&french("liable"), None);
    let deposit = Invoice { kind: Kind::Deposit, ..issued("2026-041", "2026-10-05", client("Studio Lumen", "FR"), vec![line("Deposit", 20_000, Nature::Goods, "S", 2000)], "liable", &[("2026-10-06", 24_000, "transfer")]) };
    let mut last = issued("2026-044", "2026-11-03", client("Studio Lumen", "FR"), vec![line("Prints", 50_000, Nature::Goods, "S", 2000)], "liable", &[]);
    last.deposits = vec![Cited { number: "2026-041".into(), date: "2026-10-05".into(), amount: 24_000 }];
    last.totals.deposits = 24_000;
    last.totals.due -= 24_000;
    let credit = Invoice {
        kind: Kind::CreditNote,
        corrects: Some(Cited { number: "2026-042".into(), date: "2026-10-10".into(), amount: 120_000 }),
        ..issued("2026-046", "2026-11-20", client("Studio Lumen", "FR"), vec![line("Prints returned", 10_000, Nature::Goods, "S", 2000), line("Design", 5_000, Nature::Service, "S", 2000)], "liable", &[])
    };
    let invoices = vec![
        deposit,
        issued("2026-042", "2026-10-10", client("Studio Lumen", "FR"), vec![line("Design", 100_000, Nature::Service, "S", 2000)], "liable", &[("2026-10-20", 60_000, "transfer"), ("2026-11-04", 60_000, "transfer")]),
        issued("2026-043", "2026-10-12", Client { ids: vec![Identifier { kind: "vat".into(), value: "DE123456789".into(), printed: true }], ..client("Beispiel GmbH", "DE") }, vec![line("Consulting", 80_000, Nature::Service, "AE", 0)], "liable", &[("2026-10-30", 80_000, "transfer")]),
        last,
        credit,
        issued("2026-047", "2026-11-25", client("Studio Lumen", "FR"), vec![line("Book", 1_000, Nature::Goods, "S", 550)], "franchise", &[]),
    ];
    let none = BTreeMap::new();
    let i = input(&settings, &invoices, &none, None);
    let months = vat::between(&i, day("2026-10-01"), day("2026-11-30"));
    let entries = |m: &vat::Summary| m.entries.iter().map(|e| (e.category.clone(), e.rate, e.base, e.tax)).collect::<Vec<_>>();
    // October: the deposit's €200 (paid), €500 of design paid, €800 reverse charged.
    assert_eq!(entries(&months[0]), [("S".to_string(), 2000, 70_000, 14_000), ("AE".into(), 0, 80_000, 0)]);
    // November: €500 more of design; the prints' €500 less the deposit's €200; €100 of prints credited.
    assert_eq!(entries(&months[1]), [("S".to_string(), 2000, 70_000, 14_000)]);
    assert_eq!(months[1].credits_not_counted, ["2026-046"]);
    let tr = en();
    assert_eq!(vat::sentences(&tr, &months[0], "EUR")[0], "October 2026: €140 of VAT collected, on €1,500 excluding VAT.");
    assert_eq!(vat::sentences(&tr, &months[0], "EUR")[2], "Reverse charge (the client owes the VAT): €800, no VAT here.");
    // Under the option for debits, as each invoice printed it (the review of
    // phase 4, P4-8): design's VAT on its invoice's day, the credit note's too.
    let with_debits: Vec<Invoice> = invoices.iter().map(|i| Invoice { issuer: crate::invoice::Issuer { options: vec!["debits".into()], ..i.issuer.clone() }, ..i.clone() }).collect();
    let months = vat::between(&input(&settings, &with_debits, &none, None), day("2026-10-01"), day("2026-11-30"));
    assert_eq!(entries(&months[0])[0], ("S".to_string(), 2000, 120_000, 24_000));
    assert_eq!(entries(&months[1])[0], ("S".to_string(), 2000, 15_000, 3_000));
    assert!(months[1].credits_not_counted.is_empty());
    // Today's settings never move a past summary: the option taken now changes nothing printed before.
    let debits_now = Settings::of(&InvoiceSettings { options: vec!["debits".into()], ..french("liable") }, None);
    let months = vat::between(&input(&debits_now, &invoices, &none, None), day("2026-10-01"), day("2026-11-30"));
    assert_eq!(entries(&months[0]), [("S".to_string(), 2000, 70_000, 14_000), ("AE".into(), 0, 80_000, 0)]);
    // Under the franchise, no summary.
    assert!(vat::between(&input(&Settings::of(&french("franchise"), None), &invoices[5..], &none, None), day("2026-10-01"), day("2026-11-30")).is_empty());
}

/// Money given back on a credit note (a refund paid back) is a negative
/// receipt on its own day: it lowers the book's totals, the declaration
/// and the micro ceiling's turnover of its period, the VAT collected on
/// services on that day, the franchise's turnover; a credit note on paid
/// services whose refund is not written yet is said by the VAT summary.
#[test]
fn a_refund_lowers_its_own_period() {
    let s = InvoiceSettings { vl: true, ..french("liable") };
    let settings = Settings::of(&s, None);
    let paid = issued("2026-090", "2026-09-10", client("Studio Lumen", "FR"), vec![line("Design", 100_000, Nature::Service, "S", 2000)], "liable", &[("2026-09-20", 120_000, "transfer")]);
    let refund = Invoice {
        kind: Kind::CreditNote,
        corrects: Some(Cited { number: "2026-090".into(), date: "2026-09-10".into(), amount: 120_000 }),
        ..issued("2026-091", "2026-10-05", client("Studio Lumen", "FR"), vec![line("Design, in part", 30_000, Nature::Service, "S", 2000)], "liable", &[("2026-10-08", 36_000, "transfer")])
    };
    let invoices = vec![paid, refund];
    let none = BTreeMap::new();
    let tr = en();
    let i = input(&settings, &invoices, &none, None);
    let q4 = book(&i, day("2026-10-01"), day("2026-12-31"), &tr);
    assert_eq!(q4.rows.iter().map(|r| (r.day.to_string(), r.invoice.as_str(), r.amount, r.without_tax)).collect::<Vec<_>>(), [("2026-10-08".to_string(), "2026-091", Some(-36_000), Some(-30_000))]);
    assert_eq!(q4.totals[0].without_tax, -30_000);
    let d = declarations::holding(&i, day("2026-10-08"), &tr).unwrap();
    // More given back than received in the period: nothing below zero is declared (the review of phase 4, P4-17).
    assert_eq!((d.turnover, d.declared, d.lines[0].social.map(|p| p.amount)), (-30_000, 0, Some(0)));
    let q3 = declarations::holding(&i, day("2026-09-20"), &tr).unwrap();
    assert_eq!(q3.turnover, 100_000, "the payment's period keeps what it received");
    let months = vat::between(&i, day("2026-09-01"), day("2026-10-31"));
    assert_eq!(months[0].entries[0].tax, 20_000);
    assert_eq!((months[1].entries[0].base, months[1].entries[0].tax), (-30_000, -6_000));
    // Without its refund written, said, not counted.
    let unpaid = vec![invoices[0].clone(), Invoice { payments: Vec::new(), ..invoices[1].clone() }];
    let months = vat::between(&input(&settings, &unpaid, &none, None), day("2026-10-01"), day("2026-10-31"));
    assert_eq!(months[0].credits_not_counted, ["2026-091"]);
    // Under the franchise, the refund lowers the turnover counted.
    let franchise = Settings::of(&french("franchise"), None);
    let pay = issued("2026-092", "2026-03-02", client("Studio Lumen", "FR"), vec![line("Design", 3_100_000, Nature::Service, "E", 0)], "franchise", &[("2026-03-02", 3_100_000, "transfer")]);
    let back = Invoice { kind: Kind::CreditNote, ..issued("2026-093", "2026-04-01", client("Studio Lumen", "FR"), vec![line("Design", 200_000, Nature::Service, "E", 0)], "franchise", &[("2026-04-02", 200_000, "transfer")]) };
    let w = thresholds::watch(&input(&franchise, &[pay, back], &none, None), day("2026-05-01"), &tr);
    let base = w.iter().find(|w| w.id == "franchise-services").unwrap();
    assert_eq!((base.turnover, base.near), (2_900_000, Some(day("2026-03-02"))), "80 % reached on 2 March, even if a refund came after");
}

/// The franchise's thresholds (BOI-TVA-DECLA-40-10-10 §180): operations
/// located in France, services on payment, goods on their invoice's day;
/// services to an EU business left out. 80 % said once, then the base
/// crossed ("from 1 January"), then the higher one: to do, until the tax
/// scheme says VAT from that day; a step told is never told again, and
/// an earlier step is never told after a later one.
#[test]
fn the_franchise_s_thresholds_are_said_once() {
    let settings = Settings::of(&french("franchise"), None);
    let pay = |number: &str, date: &str, cents: i64, category: &str| issued(number, date, client("Studio Lumen", "FR"), vec![line("Design", cents, Nature::Service, category, 0)], "franchise", &[(date, cents, "transfer")]);
    let mut invoices = vec![pay("2026-050", "2026-03-02", 2_000_000, "E"), pay("2026-051", "2026-05-04", 1_000_000, "E"), pay("2026-052", "2026-06-01", 2_000_000, "AE")];
    let none = BTreeMap::new();
    let tr = en();
    let watch = |invoices: &[Invoice], today: &str| thresholds::watch(&input(&settings, invoices, &none, None), day(today), &tr);
    let w = watch(&invoices, "2026-06-10");
    let base = w.iter().find(|w| w.id == "franchise-services").unwrap();
    assert_eq!((base.turnover, base.amount, base.near, base.crossed), (3_000_000, 3_750_000, Some(day("2026-05-04")), None), "€20,000 reverse charged left out");
    assert!(w.iter().all(|w| !w.id.starts_with("franchise-goods")), "no goods sold: no goods' thresholds");
    let told = BTreeSet::new();
    let tell = |w: &[thresholds::Watch], told: &BTreeSet<String>, today: &str| thresholds::to_tell(thresholds::notices(w, &[], &tr, "EUR", day(today)), told, &settings);
    let notices = tell(&w, &told, "2026-06-10");
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].key, "books:franchise-services:2026:near");
    assert_eq!(notices[0].body, "€30,000 counted this year, 80% of the VAT franchise's threshold for services, €37,500. Past it, VAT applies from 1 January 2027.");
    assert!(!notices[0].actionable);
    // Past the base: from 1 January; the 80 % already told is not told again.
    invoices.push(pay("2026-053", "2026-09-01", 800_000, "E"));
    let w = watch(&invoices, "2026-09-02");
    let told: BTreeSet<String> = ["books:franchise-services:2026:near".to_string()].into();
    let notices = tell(&w, &told, "2026-09-02");
    assert_eq!(notices.iter().map(|n| n.key.as_str()).collect::<Vec<_>>(), ["books:franchise-services:2026:crossed"]);
    assert_eq!(notices[0].title, "VAT applies from 1 January 2027");
    assert!(notices[0].body.ends_with("VAT applies from 1 January 2027: change the tax scheme then (Settings ▸ Invoices)."), "{}", notices[0].body);
    // Past the higher one today: to do, the corrective invoices said.
    invoices.push(pay("2026-054", "2026-10-14", 400_000, "E"));
    let w = watch(&invoices, "2026-10-14");
    let told: BTreeSet<String> = ["books:franchise-services:2026:near".to_string(), "books:franchise-services:2026:crossed".to_string()].into();
    let notices = tell(&w, &told, "2026-10-14");
    let higher = notices.iter().find(|n| n.step == thresholds::Step::Higher).unwrap();
    assert_eq!(higher.title, "VAT applies from today, 14 October 2026");
    assert_eq!(higher.body, "€42,000 counted this year, above the VAT franchise's higher threshold for services, €41,250: switch the tax scheme (Settings ▸ Invoices), and invoices to businesses since then get a corrective invoice.");
    assert!(higher.actionable && !thresholds::settled(&settings, higher));
    let switched = Settings::of(&InvoiceSettings { tax: vec![TaxSetting { from: String::new(), scheme: "franchise".into(), mention: String::new() }, TaxSetting { from: "2026-10-14".into(), scheme: "liable".into(), mention: String::new() }], ..french("franchise") }, None);
    assert!(thresholds::settled(&switched, higher), "the scheme says VAT from that day");
    // One payment past both thresholds: the higher one is said alone, the
    // base's steps marked with it (the review of phase 4, P4-19).
    let notices = tell(&watch(&invoices, "2026-10-14"), &BTreeSet::new(), "2026-10-14");
    assert_eq!(notices.iter().map(|n| n.key.as_str()).collect::<Vec<_>>(), ["books:franchise-services-higher:2026:higher"]);
    assert_eq!(notices[0].also, ["books:franchise-services:2026:near", "books:franchise-services:2026:crossed"]);
    // Told, it stays to do: told again at each start until the scheme says VAT (P4-4).
    let told: BTreeSet<String> = ["books:franchise-services-higher:2026:higher".to_string()].into();
    let again = |settings: &Settings| thresholds::again(thresholds::notices(&watch(&invoices, "2026-10-20"), &[], &tr, "EUR", day("2026-10-20")), &told, settings);
    assert_eq!(again(&settings).len(), 1);
    assert!(again(&switched).is_empty(), "the scheme switched: done");
    // Not watched once VAT applies.
    assert!(thresholds::watch(&input(&switched, &invoices, &none, None), day("2026-10-20"), &tr).iter().all(|w| !w.kind.starts_with("franchise")));
}

/// The year change (the review of phase 4, P4-5): the franchise's base
/// threshold crossed last year makes VAT apply on this year's 1 January,
/// said that day, to do until the scheme says so; a crossing of last year
/// learned after New Year is said at once; meanwhile this year's franchise
/// steps are not said. Another preset's thresholds are not watched (P4-16).
#[test]
fn the_year_change_is_said() {
    let settings = Settings::of(&french("franchise"), None);
    let none = BTreeMap::new();
    let tr = en();
    let pay = |number: &str, date: &str, cents: i64| issued(number, date, client("Studio Lumen", "FR"), vec![line("Design", cents, Nature::Service, "E", 0)], "franchise", &[(date, cents, "transfer")]);
    let invoices = vec![pay("2026-190", "2026-09-20", 3_820_000), pay("2027-001", "2027-01-15", 3_000_000)];
    let i = input(&settings, &invoices, &none, None);
    let last = thresholds::watch_year(&i, 2026, day("2026-12-31"), &tr);
    let notices = thresholds::notices(&thresholds::watch(&i, day("2027-01-01"), &tr), &last, &tr, "EUR", day("2027-01-01"));
    let effect = notices.iter().find(|n| n.key == "books:franchise-services:2026:effect").unwrap();
    assert_eq!(effect.title, "From today, VAT applies");
    assert_eq!(effect.body, "Your turnover of 2026, €38,200, passed the VAT franchise's threshold for services, €37,500. VAT applies from 1 January 2027: switch the tax scheme (Settings ▸ Invoices).");
    assert!(effect.actionable && !thresholds::settled(&settings, effect));
    let liable = Settings::of(&InvoiceSettings { tax: vec![TaxSetting { from: String::new(), scheme: "franchise".into(), mention: String::new() }, TaxSetting { from: "2027-01-01".into(), scheme: "liable".into(), mention: String::new() }], ..french("franchise") }, None);
    assert!(thresholds::settled(&liable, effect));
    // Later in January, this year's franchise steps are not said meanwhile.
    let notices = thresholds::notices(&thresholds::watch(&i, day("2027-01-20"), &tr), &last, &tr, "EUR", day("2027-01-20"));
    assert_eq!(notices.iter().find(|n| n.key.ends_with(":effect")).map(|n| n.title.as_str()), Some("VAT has applied since 1 January 2027"));
    assert!(notices.iter().all(|n| !n.key.contains(":2027:")), "{notices:?}");
    // A crossing of 30 December, its bank export taken in on 4 January: said at once.
    let late = vec![pay("2026-191", "2026-03-02", 3_800_000), pay("2026-192", "2026-12-30", 400_000)];
    let i = input(&settings, &late, &none, None);
    let last = thresholds::watch_year(&i, 2026, day("2026-12-31"), &tr);
    let notices = thresholds::to_tell(thresholds::notices(&thresholds::watch(&i, day("2027-01-04"), &tr), &last, &tr, "EUR", day("2027-01-04")), &BTreeSet::new(), &settings);
    let higher = notices.iter().find(|n| n.key == "books:franchise-services-higher:2026:higher").unwrap();
    assert_eq!((higher.title.as_str(), higher.actionable), ("VAT applies from 30 December 2026", true));
    // Germany's thresholds are not researched: none watched.
    let de = Settings::of(&InvoiceSettings { preset: "de".into(), tax: vec![TaxSetting { from: String::new(), scheme: "franchise".into(), mention: String::new() }], ..InvoiceSettings::default() }, None);
    assert!(thresholds::watch(&input(&de, &late, &none, None), day("2026-12-31"), &tr).is_empty());
}

/// A payment in another currency counts toward the franchise once what
/// reached you is said, by the share of its invoice's lines located here: a
/// service to a business outside the EU is not.
#[test]
fn a_payment_in_another_currency_counts_where_it_is_located() {
    let settings = Settings::of(&french("franchise"), None);
    let abroad = Invoice { currency: "USD".into(), ..issued("2026-100", "2026-03-02", client("Example Inc.", "US"), vec![line("Consulting", 2_000_000, Nature::Service, "O", 0)], "franchise", &[("2026-03-03", 2_000_000, "transfer")]) };
    let here = Invoice { currency: "USD".into(), ..issued("2026-101", "2026-03-02", Client { kind: "consumer".into(), ..client("Robin Example", "FR") }, vec![line("Prints", 1_000_000, Nature::Service, "E", 0)], "franchise", &[("2026-03-04", 1_000_000, "card")]) };
    let booked: BTreeMap<String, i64> = [("p-2026-100-0".to_string(), 1_800_000), ("p-2026-101-0".to_string(), 900_000)].into();
    let w = thresholds::watch(&input(&settings, &[abroad, here], &booked, None), day("2026-04-01"), &en());
    assert_eq!(w.iter().find(|w| w.id == "franchise-services").map(|w| w.turnover), Some(900_000));
}

/// The first year's thresholds are prorated (BOFiP: a start on 12 June,
/// €22,942 higher and €20,856 base for services); a threshold of your own
/// stands over the preset's.
#[test]
fn the_first_year_is_prorated() {
    let s = InvoiceSettings { started: "2026-06-12".into(), ..french("franchise") };
    let settings = Settings::of(&s, None);
    let invoices = vec![issued("2026-060", "2026-07-01", client("Studio Lumen", "FR"), vec![line("Prints", 1_000, Nature::Goods, "E", 0), line("Design", 1_000, Nature::Service, "E", 0)], "franchise", &[("2026-07-02", 2_000, "transfer")])];
    let none = BTreeMap::new();
    let tr = en();
    let s2 = InvoiceSettings { activity: vec!["goods-bic".into(), "bnc".into()], ..s.clone() };
    let mixed = Settings::of(&s2, None);
    let w = thresholds::watch(&input(&mixed, &invoices, &none, None), day("2026-08-01"), &tr);
    let amount = |id: &str| w.iter().find(|w| w.id == id).map(|w| w.amount);
    assert_eq!((amount("franchise-services"), amount("franchise-services-higher")), (Some(2_085_600), Some(2_294_200)));
    assert_eq!((amount("franchise-goods"), amount("franchise-goods-higher")), (Some(4_727_400), Some(5_200_100)));
    // The goods' threshold counts the whole turnover, the services' only services.
    let turnover = |id: &str| w.iter().find(|w| w.id == id).map(|w| w.turnover);
    assert_eq!((turnover("franchise-goods"), turnover("franchise-services")), (Some(2_000), Some(1_000)));
    assert!(thresholds::sentence(&tr, w.iter().find(|w| w.id == "franchise-services").unwrap(), "EUR").ends_with("Prorated to your days of activity since 12 June 2026: €37,500 for a whole year."));
    let _ = settings;
    let own = Settings::of(&InvoiceSettings { thresholds: vec![ThresholdSetting { id: "franchise-services".into(), amount: 40_000.0, from: "2026-01-01".into() }], ..french("franchise") }, None);
    let w = thresholds::watch(&input(&own, &invoices, &none, None), day("2026-08-01"), &tr);
    assert_eq!(w.iter().find(|w| w.id == "franchise-services").map(|w| w.amount), Some(4_000_000));
}

/// The micro ceilings, under the micro regime only: the turnover received;
/// prorated in the first year (URSSAF: a start on 1 March, services, €83,600
/// × 306 / 365 = €70,087); a second year above in a row ends the regime on
/// 1 January after it.
#[test]
fn the_micro_ceilings_over_two_years() {
    let pay = |number: &str, date: &str, cents: i64| issued(number, date, client("Studio Lumen", "FR"), vec![line("Design", cents, Nature::Service, "E", 0)], "liable", &[(date, cents, "transfer")]);
    let none = BTreeMap::new();
    let tr = en();
    let first = Settings::of(&InvoiceSettings { started: "2026-03-01".into(), ..french("liable") }, None);
    let w = thresholds::watch(&input(&first, &[], &none, None), day("2026-08-01"), &tr);
    assert_eq!(w.iter().find(|w| w.id == "micro-services").map(|w| w.amount), Some(7_008_700));
    let settings = Settings::of(&french("liable"), None);
    let invoices = vec![pay("2026-070", "2026-05-02", 9_000_000), pay("2027-001", "2027-03-01", 8_400_000)];
    let w = thresholds::watch(&input(&settings, &invoices, &none, None), day("2026-06-01"), &tr);
    let ceiling = w.iter().find(|w| w.id == "micro-services").unwrap();
    assert_eq!((ceiling.crossed, ceiling.last_year_above), (Some(day("2026-05-02")), false));
    let tell = |w: &[thresholds::Watch], last: &[thresholds::Watch], today: &str| thresholds::to_tell(thresholds::notices(w, last, &tr, "EUR", day(today)), &BTreeSet::new(), &settings);
    let notices = tell(&w, &[], "2026-06-01");
    assert_eq!(notices[0].title, "Your turnover passed the micro regime's ceiling");
    let w = thresholds::watch(&input(&settings, &invoices, &none, None), day("2027-03-02"), &tr);
    let ceiling = w.iter().find(|w| w.id == "micro-services").unwrap();
    assert!(ceiling.last_year_above && ceiling.crossed.is_some());
    let notices = tell(&w, &[], "2027-03-02");
    assert_eq!(notices[0].title, "The micro regime ends on 1 January 2028");
    // On 1 January 2028, said once more: it takes effect (the review of phase 4, P4-5).
    let i = input(&settings, &invoices, &none, None);
    let last = thresholds::watch_year(&i, 2027, day("2027-12-31"), &tr);
    let notices = tell(&thresholds::watch(&i, day("2028-01-01"), &tr), &last, "2028-01-01");
    let effect = notices.iter().find(|n| n.key == "books:micro-services:2027:effect").unwrap();
    assert_eq!(effect.title, "From today, the micro regime ends");
    assert!(effect.actionable);
    let reel_now = Settings::of(&InvoiceSettings { regime: "reel".into(), ..french("liable") }, None);
    assert!(thresholds::settled(&reel_now, effect));
    // 2025's ceiling is in the preset: above in 2025 and 2026, the regime ends on 1 January 2027 (P4-6).
    let twice = vec![pay("2025-090", "2025-11-02", 8_000_000), pay("2026-091", "2026-10-02", 8_400_000)];
    let w = thresholds::watch(&input(&settings, &twice, &none, None), day("2026-11-17"), &tr);
    assert!(w.iter().find(|w| w.id == "micro-services").unwrap().last_year_above, "€80,000 in 2025, above €77,700");
    // Under the real regime, no ceiling watched.
    let reel = Settings::of(&InvoiceSettings { regime: "reel".into(), ..french("liable") }, None);
    assert!(thresholds::watch(&input(&reel, &invoices, &none, None), day("2027-03-02"), &tr).is_empty());
}

/// The DES (research-fr §10.1): services to businesses in other member
/// states, month by month, by the 10th working day of the next (13
/// November 2026, weekends counted out), listing each invoice with its
/// client's VAT number; a credit note taken off.
#[test]
fn the_des_lists_its_month_s_invoices() {
    let settings = Settings::of(&french("franchise"), None);
    let gmbh = Client { ids: vec![Identifier { kind: "vat".into(), value: "DE123456789".into(), printed: true }], ..client("Beispiel GmbH", "DE") };
    let invoices = vec![
        issued("2026-080", "2026-10-30", gmbh.clone(), vec![line("Consulting", 120_000, Nature::Service, "AE", 0)], "franchise", &[]),
        Invoice { kind: Kind::CreditNote, ..issued("2026-081", "2026-10-31", gmbh, vec![line("Consulting", 20_000, Nature::Service, "AE", 0)], "franchise", &[]) },
        issued("2026-082", "2026-10-31", client("Studio Lumen", "FR"), vec![line("Design", 50_000, Nature::Service, "E", 0)], "franchise", &[]),
    ];
    let none = BTreeMap::new();
    let i = input(&settings, &invoices, &none, None);
    let all = des::between(&i, day("2026-10-01"), day("2026-10-31"));
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].key(), all[0].opens, all[0].due, all[0].total), ("2026-10".to_string(), day("2026-11-01"), day("2026-11-13"), 100_000));
    let tr = en();
    assert_eq!(des::sentences(&tr, &all[0], "EUR"), ["Services to businesses in other EU countries, October 2026 (DES): €1,000, to declare by 13 November 2026.", "2026-080 · Beispiel GmbH (DE123456789) · €1,200", "2026-081 · Beispiel GmbH (DE123456789) · −€200", "Each value is declared in euros, rounded to the nearest euro."]);
    assert_eq!(des::open_now(&i, day("2026-11-05"), 60).len(), 1);
    assert!(des::open_now(&i, day("2026-10-31"), 60).is_empty(), "its month is not over");
}

/// The books' line adds up every budget that declares: Overview says it
/// once, on the first such budget, never the whole again on the next (the
/// review of phase 4, P4-17).
#[test]
fn the_overview_says_the_books_once() {
    let tr = en();
    let money = money_with("[[budget]]\nid = 'studio'\ntitle = 'Studio'\nperiod = 'month'\narea = 'work'\nset_aside = 10\n", &[]);
    let page = crate::view::MoneyPage { ledger: &money.ledger, bank: &money.bank, movements: &money.movements, mail: &[], shown: crate::areas::Area::ALL, own: false, books: Some(&money), imports: &[] };
    let mut view = crate::view::budgets_page(&page, &tr, day("2026-11-17"));
    assert_eq!(view.budgets.iter().filter(|c| !c.declare.is_empty()).count(), 2, "both declare");
    view.say_declared("Turnover received this quarter, excluding VAT: €6,940, to declare by 1 February 2027.");
    let said: Vec<(&str, &str)> = view.budgets.iter().map(|c| (c.id.as_str(), c.declare.as_str())).collect();
    assert_eq!(said, [("work", "Turnover received this quarter, excluding VAT: €6,940, to declare by 1 February 2027."), ("studio", "")]);
    let grouped: Vec<&str> = view.groups.iter().flat_map(|g| g.budgets.iter()).filter(|c| !c.declare.is_empty()).map(|c| c.id.as_str()).collect();
    assert_eq!(grouped, ["work"], "the Budgets tab's cards alike");
}

/// What a payment in another currency came to, and the notices told, are
/// written in each device's life and read by every device.
#[test]
fn the_books_write_in_the_invoices_life() {
    let dir = std::env::temp_dir().join(format!("sioul-books-life-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    book_payment(&dir, "desk-1", "p-1", 91_000, 1_000).unwrap();
    book_payment(&dir, "phone-2", "p-1", 92_000, 2_000).unwrap();
    mark_told(&dir, "desk-1", &["books:franchise-services:2026:near".into()], day("2026-06-10")).unwrap();
    mark_told(&dir, "desk-1", &["books:franchise-services:2026:near".into()], day("2026-06-11")).unwrap();
    let lives = lives::Lives::load(&dir);
    assert_eq!(lives.booked.get("p-1"), Some(&92_000), "the last said");
    assert!(lives.told.contains("books:franchise-services:2026:near"));
    let text = std::fs::read_to_string(dir.join("life").join("desk-1.toml")).unwrap();
    assert_eq!(text.matches("[[told]]").count(), 1, "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The preset's rules of the books read: the declaration's, the DES's, the
/// thresholds' counting.
#[test]
fn the_preset_says_the_books_rules() {
    let fr = crate::invoice::presets::get("fr");
    let rule = fr.declaration.as_ref().unwrap();
    assert_eq!((rule.regime.as_str(), rule.periods.clone(), rule.months_after, rule.first_periods.month, rule.first_periods.quarter), ("micro", vec!["quarter".to_string(), "month".into()], 1, 4, 2));
    assert_eq!(fr.des.as_ref().map(|d| d.working_day), Some(10));
    assert!(fr.threshold.iter().filter(|t| t.id.contains("goods")).all(|t| t.counts == "all" && t.prorated));
    assert!(fr.threshold.iter().filter(|t| t.kind == "franchise-higher").all(|t| t.corrective));
    assert!(crate::invoice::presets::get("de").declaration.is_none());
}

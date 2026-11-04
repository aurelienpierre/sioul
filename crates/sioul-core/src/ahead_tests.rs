// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The budgets page from one income to the next, on stand-in folders: the
//! opening sentences with a pay day, an irregular income and none; each budget
//! in words; What is coming; the area filter and the shortfall it never hides;
//! the reminders; the share set aside for taxes; `sioul budgets`' lines.

use super::*;
use crate::i18n::Translator;
use crate::movements::{self, State};
use crate::view::{self, MoneyPage};
use std::path::PathBuf;

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

fn folder(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sioul-ahead-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// A household paid on the 28th: rent, phone and energy, food as an
/// estimate, leisure and a small studio's work; one account topped up by a Livret A.
const SALARIED: &str = "[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\ntarget = 300\narea = 'admin'\n\
    [[budget]]\nid = 'fun'\ntitle = 'Leisure'\nperiod = 'month'\ntarget = -80\narea = 'leisure'\n\
    [[budget]]\nid = 'studio'\ntitle = 'Studio'\nperiod = 'month'\narea = 'work'\n\
    [[preset]]\nid = 'pay'\nbudget = 'home'\nlabel = 'Salary'\namount = 2000\nevery = 'month'\nday = 28\n\
    [[preset]]\nid = 'rent'\nbudget = 'home'\nlabel = 'Rent'\namount = -760\nevery = 'month'\nday = 26\n\
    [[preset]]\nid = 'phone'\nbudget = 'home'\nlabel = 'Phone'\namount = -19.99\nevery = 'month'\nday = 12\nby_hand = true\n\
    [[preset]]\nid = 'energy'\nbudget = 'home'\nlabel = 'Energy'\namount = -64\nevery = 'month'\nday = 5\n\
    [[preset]]\nid = 'food'\nbudget = 'home'\nlabel = 'Food'\namount = -300\nevery = 'month'\nday = 31\nestimate = true\n\
    [[preset]]\nid = 'gym'\nbudget = 'fun'\nlabel = 'Climbing gym'\namount = -35\nevery = 'month'\nday = 3\n\
    [[reserve]]\nid = 'livret'\ntitle = 'Livret A'\nbalance = 3000\nas_of = 2026-11-01\nfloor = 500\n\
    [[bank_account]]\nid = 'main'\ntitle = 'Main account'\nfills = ['home', 'fun', 'studio']\nexports = ['0123456A']\ntopped_up_by = ['livret']\n\
    [[split]]\nwords = ['market', 'greengrocer']\npreset = 'food'\n";

/// Its bank's file: the balance on the 9th, the energy, the gym and two
/// days of food passed, as the matching joins them.
const SALARIED_BANK: &str = "[[account]]\nid = '0123456A'\nbalance = 1240.0\nas_of = 2026-11-09\n\
    [[movement]]\naccount = '0123456A'\ndate = 2026-10-28\namount = 2000.0\nlabel = 'VIR SALARY EXAMPLE SA'\nid = 'B0'\n\
    [[movement]]\naccount = '0123456A'\ndate = 2026-11-03\namount = -35.0\nlabel = 'PRLV CLIMBING GYM'\nid = 'B1'\n\
    [[movement]]\naccount = '0123456A'\ndate = 2026-11-05\namount = -64.0\nlabel = 'PRLV ENERGY CO'\nid = 'B2'\n\
    [[movement]]\naccount = '0123456A'\ndate = 2026-11-06\namount = -42.3\nlabel = 'CB MARKET HALL'\nid = 'B3'\n\
    [[movement]]\naccount = '0123456A'\ndate = 2026-11-08\namount = -18.2\nlabel = 'CB GREENGROCER'\nid = 'B4'\n";

fn write(root: &Path, ledger: &str, bank: &str) {
    std::fs::write(root.join(crate::budget::LEDGER), ledger).unwrap();
    std::fs::write(root.join(crate::bank::MANIFEST), bank).unwrap();
}

fn aheads(root: &Path, today: Date) -> Vec<Ahead> {
    of_folder(root, &crate::words::Words::builtin(), today)
}

/// The page as the window makes it, the filter showing `shown`.
fn page(root: &Path, today: Date, shown: Area, own: bool, tr: &Translator) -> view::BudgetsView {
    let books = movements::books(root, &crate::words::Words::builtin(), today);
    let page = MoneyPage { ledger: &books.counted, bank: &books.bank, movements: &books.movements, mail: &[], shown, own };
    view::budgets_page(&page, tr, today)
}

/// With a pay day: until the pay, the account holds every payment; what
/// is left for everyday spending, and a day; the lowest point, by its day and
/// the payment that brings it, its amount said once.
#[test]
fn the_account_holds_every_payment_until_the_pay() {
    let root = folder("pay");
    write(&root, SALARIED, SALARIED_BANK);
    let today = day("2026-11-10");
    let all = aheads(&root, today);
    let main = &all[0];
    assert_eq!(main.income.as_ref().map(|i| (i.day.to_string(), i.label.as_str(), i.certain)), Some(("2026-11-28".to_string(), "Salary", true)));
    assert_eq!(main.now, Money(124_000));
    assert_eq!(main.short, None);
    // 1,240 less the phone on the 12th and the rent on the 26th, over eighteen days.
    assert_eq!((main.left, main.days), (Money(46_001), 18));
    assert_eq!(main.low.as_ref().map(|l| (l.day.to_string(), l.payment.as_str())), Some(("2026-11-26".to_string(), "Rent")));
    let tr = Translator::new("en");
    assert_eq!(
        sentences(main, &tr, today),
        [
            "Until Salary comes in on Saturday 28 November, the account holds every payment.",
            "€460 is left for everyday spending, about €25 a day.",
            "The lowest point comes on Thursday 26 November, after Rent.",
        ]
    );
    let fr = Translator::new("fr");
    assert_eq!(sentences(main, &fr, today)[0], "Jusqu’à l’arrivée de Salary le samedi 28 novembre, le compte couvre chaque paiement.");
    assert!(sentences(main, &fr, today)[1].starts_with("Reste à vivre\u{202f}: 460\u{a0}€"), "{:?}", sentences(main, &fr, today));
    // The chart: a step at each movement to a few days past the pay, the pay marked.
    let chart = chart(main, today);
    assert_eq!((chart.from.as_str(), chart.to.as_str()), ("2026-11-10", "2026-12-01"));
    assert!(chart.marks.iter().any(|m| m.income && m.label == "Salary") && chart.marks.iter().any(|m| !m.income && m.label == "Rent"), "{:?}", chart.marks);
    assert!(chart.without.is_empty(), "a sure pay: one line");
    let _ = std::fs::remove_dir_all(&root);
}

/// Irregular income: the next client's payment is said with its doubt, and
/// what the account would do without it, when that changes the answer; the
/// chart draws it as a lighter second line.
#[test]
fn an_irregular_income_is_said_with_its_doubt() {
    let root = folder("irregular");
    let ledger = "[[budget]]\nid = 'studio'\ntitle = 'Studio'\nperiod = 'month'\narea = 'work'\n\
        [[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n\
        [[preset]]\nid = 'rent'\nbudget = 'home'\nlabel = 'Rent'\namount = -620\nevery = 'month'\nday = 25\n\
        [[line]]\nid = 'inv16'\nbudget = 'studio'\ndate = 2026-11-20\namount = 720\nlabel = 'Client A'\nplanned = true\n\
        [[line]]\nid = 'inv17'\nbudget = 'studio'\ndate = 2026-12-18\namount = 900\nlabel = 'Client B'\nplanned = true\n\
        [[bank_account]]\nid = 'main'\ntitle = 'Main account'\nfills = ['home', 'studio']\nexports = ['0123456A']\n";
    write(&root, ledger, "[[account]]\nid = '0123456A'\nbalance = 400.0\nas_of = 2026-11-09\n");
    let today = day("2026-11-10");
    let main = &aheads(&root, today)[0];
    assert_eq!(main.income.as_ref().map(|i| (i.label.as_str(), i.certain)), Some(("Client A", false)));
    // Without Client A, the rent on the 25th would leave it €220 short.
    assert_eq!(main.without.as_ref().map(|w| (w.day.to_string(), w.balance)), Some(("2026-11-25".to_string(), Money(-22_000))));
    let tr = Translator::new("en");
    let said = sentences(main, &tr, today);
    assert_eq!(said[0], "If Client A comes in by Friday 20 November, the account holds every payment until then.");
    assert_eq!(said.last().unwrap(), "Without it, after Rent on Wednesday 25 November, €220 would be missing.");
    let chart = chart(main, today);
    assert!(!chart.without.is_empty() && chart.marks.iter().any(|m| m.uncertain), "the doubt as a lighter second line: {chart:?}");
    let _ = std::fs::remove_dir_all(&root);
}

/// No income within two months: how long the account and its savings
/// last at your own pace, in weeks.
#[test]
fn with_no_income_the_account_lasts_so_many_weeks() {
    let root = folder("lasts");
    let ledger = "[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n\
        [[reserve]]\nid = 'livret'\ntitle = 'Livret A'\nbalance = 1500\nas_of = 2026-08-01\nfloor = 0\n\
        [[bank_account]]\nid = 'main'\ntitle = 'Main account'\nfills = ['home']\nexports = ['0123456A']\ntopped_up_by = ['livret']\n";
    // Thirteen weeks of €100 a week spent; €900 left.
    let mut bank = String::from("[[account]]\nid = '0123456A'\nbalance = 900.0\nas_of = 2026-11-09\n");
    for week in 0..13 {
        let d = day("2026-08-11").checked_add(jiff::Span::new().weeks(week)).unwrap();
        bank.push_str(&format!("[[movement]]\naccount = '0123456A'\ndate = {d}\namount = -100.0\nlabel = 'CB MARKET'\nid = 'W{week}'\n"));
    }
    write(&root, ledger, &bank);
    let today = day("2026-11-10");
    let main = &aheads(&root, today)[0];
    assert_eq!(main.income, None);
    assert_eq!(main.lasts, Some(24), "(900 + 1,500) at €100 a week");
    let tr = Translator::new("en");
    assert_eq!(sentences(main, &tr, today), ["At your usual pace, the account and Livret A last about 24 weeks."]);
    assert_eq!(sentences(main, &Translator::new("fr"), today), ["À votre rythme habituel, le compte et Livret A tiennent environ 24 semaines."]);
    let _ = std::fs::remove_dir_all(&root);
}

/// Each budget in words, its period from one pay to the next when it
/// starts with its income: what was paid, what is still to come (named, its
/// amount and day), what is left of the food; one sentence about its end.
/// No percentage anywhere.
#[test]
fn each_budget_says_what_is_paid_and_what_is_left() {
    let root = folder("words");
    write(&root, &SALARIED.replace("target = 300\narea = 'admin'", "target = 300\narea = 'admin'\nstarts_with = 'pay'"), SALARIED_BANK);
    let today = day("2026-11-10");
    let tr = Translator::new("en");
    let view = page(&root, today, Area::ALL, false, &tr);
    let home = view.budgets.iter().find(|b| b.id == "home").unwrap();
    assert_eq!(home.period, "28 October to 27 November", "from one pay to the next");
    assert_eq!(home.words, ["Paid: Energy.", "Still to come: Phone, €19.99, Thursday 12 November; Rent, €760, Thursday 26 November.", "Came in: Salary.", "Food: €239.50 left until 27 November."]);
    assert!(home.verdict.starts_with("By 27 November, it should end"), "{}", home.verdict);
    let text = view::budgets_lines(&view).join("\n");
    assert!(!text.contains('%'), "no percentage: {text}");
    let _ = std::fs::remove_dir_all(&root);
}

/// What is coming: every movement expected from today, by week, in day
/// order; the pay as plain as the rent, each with its budget and its state
/// in words; nothing done; the rows the filter keeps.
#[test]
fn what_is_coming_week_by_week() {
    let root = folder("coming");
    write(&root, SALARIED, SALARIED_BANK);
    let today = day("2026-11-10");
    let tr = Translator::new("en");
    let view = page(&root, today, Area::ALL, false, &tr);
    let titles: Vec<&str> = view.coming.iter().map(|w| w.title.as_str()).collect();
    // Nothing in the week of the 16th: no week said for it.
    assert_eq!(&titles[..3], ["This week", "Week of Monday 23 November", "Week of Monday 30 November"]);
    let rows: Vec<(&str, &str, &str, &str)> = view.coming.iter().flat_map(|w| w.rows.iter()).take(4).map(|r| (r.date.as_str(), r.who.as_str(), r.way, r.state.as_str())).collect();
    assert_eq!(
        rows,
        [
            ("Thu 12 Nov", "Phone", "out", "expected on Thursday 12 November"),
            ("Thu 26 Nov", "Rent", "out", "expected on Thursday 26 November"),
            ("Sat 28 Nov", "Salary", "in", "expected on Saturday 28 November"),
            ("Thu 3 Dec", "Climbing gym", "out", "expected on Thursday 3 December"),
        ]
    );
    let phone = &view.coming[0].rows[0];
    assert_eq!((phone.amount.as_str(), phone.budget.as_str(), phone.from.as_str()), ("€19.99", "Home", "From the recurring payment."));
    // Six weeks at least: the next pay's month is there.
    assert!(view.coming.iter().flat_map(|w| w.rows.iter()).any(|r| r.day == "2026-12-26" && r.who == "Rent"));
    // In leisure's hours, the household's rows wait.
    let leisure = page(&root, today, Area::LEISURE, false, &tr);
    assert!(leisure.coming.iter().flat_map(|w| w.rows.iter()).all(|r| r.budget == "Leisure"), "{:?}", leisure.coming);
    let _ = std::fs::remove_dir_all(&root);
}

/// The filter: budgets show by area as the hours or your ticks say, and the
/// page says what it leaves out in words, never how many; an account short
/// before its income shows whatever the filter, and is the Porch's notice
/// in every area.
#[test]
fn the_filter_hides_budgets_never_a_shortfall() {
    let root = folder("filter");
    // The rent grown past what the account holds: short on the 26th.
    write(&root, &SALARIED.replace("amount = -760", "amount = -1260").replace("fills = ['home', 'fun', 'studio']", "fills = ['studio']"), SALARIED_BANK);
    let today = day("2026-11-10");
    let tr = Translator::new("en");
    let leisure = page(&root, today, Area::LEISURE, false, &tr);
    assert_eq!(leisure.budgets.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["fun"]);
    assert_eq!(leisure.left_out, ["Work budgets are kept for your work hours.", "Admin budgets are kept for your admin hours."]);
    let ticked = page(&root, today, Area::LEISURE, true, &tr);
    assert_eq!(ticked.left_out[0], "Work budgets are not ticked.");
    // A work account, at leisure: hidden, but short, so shown.
    let account = leisure.accounts.iter().find(|a| a.id == "main").expect("a shortfall shows whatever the filter");
    assert!(account.short && account.sentences[0].contains("does not hold every payment"), "{:?}", account.sentences);
    assert!(account.sentences.iter().any(|s| s.starts_with("Livret A can cover it: move €")), "{:?}", account.sentences);
    // The Porch's notice: in every area, the money section, the budgets page opened.
    let notices = shortfall_notices(&root, &crate::words::Words::builtin(), &tr, today);
    assert_eq!(notices.len(), 1);
    let notice = &notices[0];
    assert_eq!((notice.area.as_str(), notice.section.as_str(), notice.address.as_str(), notice.who.as_str()), ("", "money", "sioul:budget/", "Main account"));
    assert_eq!(notice.id, crate::notices::money_id("short/main/2026-11-26"));
    assert!(notice.what.starts_with("After Rent on Thursday 26 November, the account would be €"), "{}", notice.what);
    let _ = std::fs::remove_dir_all(&root);
}

/// Reminders from movements: the phone paid by hand, a few working days
/// before, saying the account holds it; the rent (a direct debit) none
/// while it holds; told when it will not, five working days before.
#[test]
fn reminders_come_from_movements() {
    let root = folder("remind");
    write(&root, SALARIED, SALARIED_BANK);
    let today = day("2026-11-09");
    let books = movements::books(&root, &crate::words::Words::builtin(), today);
    let all = of(&books.counted, &books.bank, &books.movements, today);
    let money = crate::reminders::Money { movements: &books.movements, aheads: &all };
    let tr = Translator::new("en");
    // Admin hours on weekdays, nine to five: the reminders' working days.
    let windows: String = ["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| format!("[[window]]\nday = \"{d}\"\nstart = \"09:00\"\nend = \"17:00\"\n")).collect();
    let config: crate::config::Config = toml::from_str(&windows).unwrap();
    let now = today.at(8, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap();
    let said = crate::reminders::all(&config, &tr, &now, &[], &[], Some(&books.counted), &[], &[], Some(&money), |_| false);
    let payments: Vec<(&str, &str, &str)> = said.iter().filter(|r| r.kind == crate::reminders::Kind::Payment).map(|r| (r.key.as_str(), r.title.as_str(), r.body.as_str())).collect();
    assert_eq!(payments, [("payment:preset:phone/2026-11-12", "Phone · €19.99", "Planned for Thursday 12 November; the account will hold it.")]);
    assert!(!said.iter().any(|r| r.key.starts_with("short:")), "it holds: nothing about the debits");
    // The rent grown past what the account holds: told once, five working days before.
    write(&root, &SALARIED.replace("amount = -760", "amount = -1260"), SALARIED_BANK);
    let books = movements::books(&root, &crate::words::Words::builtin(), today);
    let all = of(&books.counted, &books.bank, &books.movements, today);
    let money = crate::reminders::Money { movements: &books.movements, aheads: &all };
    let said = crate::reminders::all(&config, &tr, &now, &[], &[], Some(&books.counted), &[], &[], Some(&money), |_| false);
    let short = said.iter().find(|r| r.key.starts_with("short:")).expect("the rent the account will not hold");
    assert_eq!(short.title, "The account may not hold Rent on Thursday 26 November");
    let _ = std::fs::remove_dir_all(&root);
}

/// The tax set-aside: off until set; set, each income of the work budget
/// makes one planned movement per declaration, on its day, its share of
/// the period's incomes; counted in the budget, said in its words and in What is coming.
#[test]
fn a_share_of_each_income_is_set_aside_for_the_declaration() {
    let root = folder("aside");
    let ledger = "[[budget]]\nid = 'studio'\ntitle = 'Studio'\nperiod = 'month'\narea = 'work'\nset_aside = 22\ndeclare = 'quarter'\n\
        [[line]]\nid = 'a'\nbudget = 'studio'\ndate = 2026-10-15\namount = 1000\nlabel = 'Client A'\n\
        [[line]]\nid = 'b'\nbudget = 'studio'\ndate = 2026-11-20\namount = 500\nlabel = 'Client B'\nplanned = true\n\
        [[line]]\nid = 'c'\nbudget = 'studio'\ndate = 2027-01-10\namount = 800\nlabel = 'Client C'\nplanned = true\n";
    write(&root, ledger, "");
    let today = day("2026-11-10");
    let books = movements::books(&root, &crate::words::Words::builtin(), today);
    let aside: Vec<(String, i64)> = books.movements.iter().filter(|m| m.sources.iter().any(|s| s.kind == movements::Kind::SetAside)).map(|m| (m.day.to_string(), m.sum.amount().cents())).collect();
    // The fourth quarter's two incomes, declared by 31 January; the first quarter's by 30 April.
    assert_eq!(aside, [("2027-01-31".to_string(), -33_000), ("2027-04-30".to_string(), -17_600)]);
    assert!(books.movements.iter().filter(|m| m.sources.iter().any(|s| s.kind == movements::Kind::SetAside)).all(|m| m.state == State::Planned));
    // Counted as planned in the budget: the period after its day sees it.
    assert!(books.counted.lines.iter().any(|l| l.planned && l.amount == Money(-33_000) && l.links.iter().any(|k| k.starts_with("setaside:"))));
    let tr = Translator::new("en");
    let view = page(&root, today, Area::ALL, false, &tr);
    let studio = view.budgets.iter().find(|b| b.id == "studio").unwrap();
    assert!(studio.words.iter().any(|w| w == "Set aside for taxes and contributions: about €330 for the declaration of Sunday 31 January 2027."), "{:?}", studio.words);
    // Off until set.
    write(&root, &ledger.replace("set_aside = 22\n", ""), "");
    let books = movements::books(&root, &crate::words::Words::builtin(), today);
    assert!(!books.movements.iter().any(|m| m.sources.iter().any(|s| s.kind == movements::Kind::SetAside)));
    // The declaration's days, monthly and quarterly.
    assert_eq!((movements::declaration_day(day("2026-11-15"), false), movements::declaration_day(day("2026-02-01"), true)), (day("2026-12-31"), day("2026-04-30")));
    let _ = std::fs::remove_dir_all(&root);
}

/// `sioul budgets`' lines and the MCP's: each account's sentences, each
/// budget in words, What is coming by week; no percentage, in both languages.
#[test]
fn sioul_budgets_says_it_in_words() {
    let root = folder("cli");
    write(&root, SALARIED, SALARIED_BANK);
    let today = day("2026-11-10");
    for lang in ["en", "fr"] {
        let tr = Translator::new(lang);
        let lines = view::budgets_lines(&page(&root, today, Area::ALL, false, &tr));
        assert!(lines.iter().all(|l| !l.contains('%') && !l.to_lowercase().contains("debit") && !l.to_lowercase().contains("crédit")), "{lines:#?}");
        if lang == "en" {
            assert_eq!(&lines[..3], ["Budgets, November 2026", "  Main account", "    Until Salary comes in on Saturday 28 November, the account holds every payment."]);
            assert!(lines.contains(&"What is coming".to_string()) && lines.contains(&"    Thu 12 Nov · Phone · €19.99 · Home · expected on Thursday 12 November".to_string()), "{lines:#?}");
            assert!(lines.contains(&"    Sat 28 Nov · Salary · +€2,000 · Home · expected on Saturday 28 November".to_string()), "{lines:#?}");
        } else {
            assert!(lines.contains(&"Ce qui arrive".to_string()), "{lines:#?}");
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// The example budget file as `sioul budgets` says it on 2 October 2026,
/// as docs/accounting.md ("At a glance") shows it: no export, so no account;
/// each budget in words, What is coming.
#[test]
fn the_example_file_in_words() {
    let root = folder("example");
    let example = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sioul-budgets.toml")).unwrap();
    write(&root, &example, "");
    let lines = view::budgets_lines(&page(&root, day("2026-10-02"), Area::ALL, false, &Translator::new("en")));
    println!("{}", lines.join("\n"));
    assert_eq!(lines[0], "Budgets, October 2026");
    assert!(lines.iter().any(|l| l == "    Paid: Rent."), "{lines:#?}");
    assert!(lines.iter().any(|l| l.starts_with("    Daily spending (estimate): €500 left until 31 October")), "{lines:#?}");
    // With no export, the rent of the 1st counts as paid: it is not awaited in What is coming.
    assert!(!lines.iter().any(|l| l.contains("Thu 1 Oct")) && lines.contains(&"  Week of Monday 19 October".to_string()), "{lines:#?}");
    let _ = std::fs::remove_dir_all(&root);
}

/// What the budgets cost on a big stand-in folder (`SIOUL_PERF_FOLDER`:
/// five years of a busy account, about 6,000 bank lines, 40 presets, two
/// accounts, letters, a store with links), timed by hand in a release build:
/// `SIOUL_PERF_FOLDER=… cargo test --release -p sioul-core --lib ahead::tests::timing -- --ignored --nocapture`.
#[test]
#[ignore]
fn timing() {
    let Ok(root) = std::env::var("SIOUL_PERF_FOLDER") else { return };
    let root = PathBuf::from(root);
    let words = crate::words::Words::builtin();
    let today = day("2026-11-04");
    let tr = Translator::new("en");
    let clock = std::time::Instant::now();
    let books = movements::books(&root, &words, today);
    let cold = clock.elapsed();
    let clock = std::time::Instant::now();
    let again = movements::books(&root, &words, today);
    let warm = clock.elapsed();
    assert!(std::sync::Arc::ptr_eq(&books, &again), "kept while no file changed");
    // Another process: the kept copy read, not derived again.
    movements::forget_books();
    let clock = std::time::Instant::now();
    let read = movements::books(&root, &words, today);
    let disk = clock.elapsed();
    assert_eq!(read.movements, books.movements, "the kept copy is the same");
    let clock = std::time::Instant::now();
    let shown = page(&root, today, Area::ALL, false, &tr);
    let view = clock.elapsed();
    let clock = std::time::Instant::now();
    let all = of(&books.counted, &books.bank, &books.movements, today);
    let money = crate::reminders::Money { movements: &books.movements, aheads: &all };
    let said = crate::reminders::all(&crate::config::Config::default(), &tr, &today.at(9, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap(), &[], &[], Some(&books.counted), &[], &[], Some(&money), |_| false);
    let reminders = clock.elapsed();
    let clock = std::time::Instant::now();
    let lines = view::budgets_lines(&shown);
    let text = clock.elapsed();
    println!(
        "{} bank lines, {} movements: books cold {cold:?}, warm {warm:?}, from the kept copy {disk:?}; the page {view:?}; reminders {reminders:?} ({} said); the lines {text:?} ({} lines)",
        books.bank.movements.len(),
        books.movements.len(),
        said.len(),
        lines.len()
    );
}

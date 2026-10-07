// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The money watch, for the window (docs/accounting.md, "The bank" and "Bank
//! accounts"): your bank accounts (a current account, PayPal, Stripe), each
//! with its exports taken in, the budgets it fills, where each movement went
//! and why, what tops it up; then in words what passed and what did not, the
//! week's payments, and the balance carried forward a month.

use crate::backend::{json, load_config, say, tr};
use serde::Serialize;
use sioul_core::bank::{Bank, Finding};
use sioul_core::budget::Ledger;
use sioul_core::money::Money;

#[derive(Serialize, Default)]
struct View {
    store: bool,
    /// Movements or a balance were read once at least.
    any: bool,
    /// "€845.10 on Saturday 3 October, from your bank's file."
    balance: String,
    findings: Vec<String>,
    /// The week's payments: "Thursday 8 October · EDF · €62.00".
    coming: Vec<String>,
    /// The balance, day by day: {day ("2026-10-03"), cents}.
    forecast: Vec<Point>,
    /// For the Porch: the week's payments in one line, and whether the account holds them.
    week: String,
    /// What the watch found, for the Porch to point at the Budgets page.
    attention: usize,
    problem: String,
    /// Your bank accounts, as declared in the budget file.
    accounts: Vec<AccountView>,
    /// What a movement can go to: the budgets, the reserves, the accounts.
    budgets: Vec<Named>,
    reserves: Vec<ReserveView>,
}

#[derive(Serialize, Clone)]
struct Named {
    id: String,
    title: String,
}

#[derive(Serialize)]
struct ReserveView {
    id: String,
    title: String,
    balance: f64,
    as_of: String,
    floor: f64,
    delay_days: u32,
}

#[derive(Serialize)]
struct AccountView {
    id: String,
    title: String,
    /// "bank", "paypal", "stripe", "other".
    kind: String,
    /// "€845.10 on Saturday 3 October", or "" before its first export.
    balance: String,
    /// The budgets it fills, the one taking what nothing else places first.
    fills: Vec<Named>,
    floor: f64,
    /// "€100.00", or "" without a floor.
    floor_text: String,
    topped_up_by: Vec<Named>,
    /// What tops it up this month, in sentences.
    top_ups: Vec<String>,
    /// Newest first.
    movements: Vec<MovementView>,
    /// Movements nothing placed: the account fills no budget.
    unplaced: usize,
    /// The rules that read its movements (its own and those for every account), in file order.
    rules: Vec<RuleView>,
}

#[derive(Serialize)]
struct MovementView {
    id: String,
    date: String,
    label: String,
    amount: String,
    cents: i64,
    /// Where it went, in words.
    place: String,
    /// "hand", "mail", "rule", "between", "preset", "first", "unknown".
    why: String,
    /// Your choice for it: `budget:<id>`, `none`, or `""` (as the rules say).
    chosen: String,
}

#[derive(Serialize)]
struct RuleView {
    /// Its place among the rules of the file.
    place: usize,
    words: Vec<String>,
    /// "credit", "debit", "".
    direction: String,
    /// `budget:<id>`, `reserve:<id>`, `transfer:<id>`, `preset:<id>`.
    to: String,
    to_title: String,
    /// It reads every account's movements, not this one's alone.
    everywhere: bool,
}

#[derive(Serialize)]
struct Point {
    day: String,
    cents: i64,
}

fn finding_text(finding: &Finding, today: jiff::civil::Date) -> String {
    let day = |d: jiff::civil::Date| tr().day_in(d, today);
    let money = |m: Money| tr().money(m.abs());
    match finding {
        Finding::Missed { label, amount, date } => {
            let title = say(if amount.is_negative() { "money-missed-out" } else { "money-missed-in" }, &[("label", label.clone()), ("amount", money(*amount))]);
            format!("{title}. {}", say("money-missed-why", &[("date", day(*date))]))
        }
        Finding::Changed { label, expected, actual, date } => say("money-changed", &[("label", label.clone()), ("actual", money(*actual)), ("date", day(*date)), ("expected", money(*expected))]),
        Finding::Short { label, amount, date, short, reserve } => {
            let title = say("money-short", &[("label", label.clone()), ("date", day(*date)), ("amount", money(*amount))]);
            let body = match reserve {
                Some(r) => say("money-short-reserve", &[("short", money(*short)), ("reserve", r.clone())]),
                None => say("money-short-ask", &[("short", money(*short))]),
            };
            format!("{title}. {body}")
        }
    }
}

/// The bank's part of the Budgets page, as JSON.
pub(crate) fn view() -> String {
    let Some(root) = load_config().case_store_path() else { return json(&View { problem: tr().text("papers-no-store", None), ..View::default() }) };
    let bank = match Bank::load(&root) {
        Ok(bank) => bank,
        Err(problem) => return json(&View { store: true, problem, ..View::default() }),
    };
    let today = jiff::Zoned::now().date();
    let mut view = View { store: true, any: !bank.movements.is_empty() || bank.balance().is_some(), ..View::default() };
    if let Some((date, amount)) = bank.balance() {
        view.balance = say("bank-balance", &[("amount", tr().money(amount)), ("date", tr().day_in(date, today))]);
    }
    let ledger = Ledger::load(&root).unwrap_or_default();
    let watch = sioul_core::bank::watch(&bank, &ledger, today);
    view.findings = watch.findings.iter().map(|f| finding_text(f, today)).collect();
    view.coming = watch.coming.iter().map(|e| format!("{} · {} · {}", tr().day_in(e.date, today), e.label, tr().money(e.amount.abs()))).collect();
    view.forecast = watch.forecast.iter().map(|(d, m)| Point { day: d.to_string(), cents: m.cents() }).collect();
    view.attention = watch.findings.len();
    accounts_into(&mut view, &bank, &ledger, today);
    if !watch.coming.is_empty() {
        let list = watch.coming.iter().take(4).map(|e| format!("{} {} ({})", e.label, tr().money(e.amount.abs()), tr().weekday_short(e.date))).collect::<Vec<_>>().join(", ");
        let week = today.checked_add(jiff::Span::new().days(7)).unwrap_or(today);
        let held = watch.forecast.iter().filter(|(d, _)| *d <= week).all(|(_, m)| !m.is_negative());
        view.week = say(if held { "bank-week-held" } else { "bank-week-short" }, &[("list", list)]);
    }
    json(&view)
}

/// The bank accounts' part of the view: each with its balance, its budgets,
/// its movements placed and why, its rules, what tops it up.
fn accounts_into(view: &mut View, bank: &Bank, ledger: &Ledger, today: jiff::civil::Date) {
    use sioul_core::accounts::{self, Place, Why};
    let budget_title = |id: &str| ledger.budgets.iter().find(|b| b.id == id).map_or(id.to_string(), |b| b.title.clone());
    let reserve_title = |id: &str| ledger.reserves.iter().find(|r| r.id == id).map_or(id.to_string(), |r| r.title.clone());
    let account_title = |id: &str| ledger.bank_accounts.iter().find(|a| a.id == id).map_or(id.to_string(), |a| a.title.clone());
    let preset_label = |key: &str| {
        let index = key.strip_prefix('#').and_then(|i| i.parse::<usize>().ok());
        ledger.presets.iter().enumerate().find(|(i, p)| p.id.as_deref() == Some(key) || index == Some(*i)).map(|(_, p)| p.label.clone()).unwrap_or_default()
    };
    view.budgets = ledger.budgets.iter().map(|b| Named { id: b.id.clone(), title: b.title.clone() }).collect();
    view.reserves = ledger
        .reserves
        .iter()
        .map(|r| ReserveView { id: r.id.clone(), title: r.title.clone(), balance: r.balance.cents() as f64 / 100.0, as_of: r.as_of.to_string(), floor: r.floor.cents() as f64 / 100.0, delay_days: r.delay_days })
        .collect();
    let placed = accounts::place(ledger, bank);
    let ups = accounts::top_ups(ledger, bank, today);
    for account in &ledger.bank_accounts {
        let balance = accounts::balance_of(bank, account).map(|(date, amount)| say("bank-balance", &[("amount", tr().money(amount)), ("date", tr().day_in(date, today))])).unwrap_or_default();
        let mine: Vec<&accounts::Placed> = placed.iter().filter(|p| p.account == account.id).collect();
        let movements = mine
            .iter()
            .rev()
            .take(120)
            .map(|p| {
                let place = match &p.place {
                    Place::Budgets(shares) => shares
                        .iter()
                        .map(|s| {
                            let preset = s.preset.as_deref().map(preset_label).filter(|l| !l.is_empty());
                            let title = budget_title(&s.budget);
                            let title = match preset {
                                Some(label) => format!("{title} · {label}"),
                                None => title,
                            };
                            if shares.len() > 1 { format!("{title} {}", tr().money(s.amount.abs())) } else { title }
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    Place::Reserve { reserve, budget } => say("bank-place-reserve", &[("reserve", reserve_title(reserve)), ("budget", budget_title(budget))]),
                    Place::Transfer(other) => say("bank-place-transfer", &[("account", account_title(other))]),
                    Place::Known(line) => say("bank-place-known", &[("label", ledger.lines.get(*line).map(|l| l.label.clone()).unwrap_or_default())]),
                    Place::Nowhere => tr().text("bank-place-none", None),
                };
                let why = match p.why {
                    Why::Hand => "hand",
                    Why::Mail => "mail",
                    Why::Rule => "rule",
                    Why::Between => "between",
                    Why::Preset => "preset",
                    Why::First => "first",
                    Why::Unknown => "unknown",
                };
                let chosen = ledger
                    .assignments
                    .iter()
                    .find(|a| account.owns(&a.account) && a.movement == p.movement.id)
                    .map(|a| if a.none { "none".to_string() } else { a.budget.as_deref().map(|b| format!("budget:{b}")).unwrap_or_default() })
                    .unwrap_or_default();
                MovementView {
                    id: p.movement.id.clone(),
                    date: tr().day_in(p.movement.date, today),
                    label: p.movement.label.clone(),
                    amount: tr().money(p.movement.amount),
                    cents: p.movement.amount.cents(),
                    place,
                    why: why.to_string(),
                    chosen,
                }
            })
            .collect();
        let rules = ledger
            .splits
            .iter()
            .enumerate()
            .filter(|(_, s)| s.account.is_empty() || account.owns(&s.account))
            .map(|(place, s)| {
                let (to, to_title) = if let Some(other) = &s.transfer {
                    (format!("transfer:{other}"), say("bank-place-transfer", &[("account", account_title(other))]))
                } else if let Some(reserve) = &s.reserve {
                    (format!("reserve:{reserve}"), reserve_title(reserve))
                } else if let Some(budget) = &s.budget {
                    (format!("budget:{budget}"), budget_title(budget))
                } else if let Some(preset) = &s.preset {
                    (format!("preset:{preset}"), preset_label(preset))
                } else {
                    (String::new(), String::new())
                };
                let direction = match s.direction {
                    Some(sioul_core::budget::Direction::Credit) => "credit",
                    Some(sioul_core::budget::Direction::Debit) => "debit",
                    None => "",
                };
                RuleView { place, words: s.words.clone(), direction: direction.to_string(), to, to_title, everywhere: s.account.is_empty() }
            })
            .collect();
        let top_ups = ups
            .iter()
            .filter(|u| u.account == account.id)
            .map(|u| {
                let delay = ledger.reserves.iter().find(|r| r.id == u.reserve).map_or(0, |r| r.delay_days);
                let args = [
                    ("reserve", reserve_title(&u.reserve)),
                    ("amount", tr().money(u.amount)),
                    ("date", tr().day_in(u.needed_by, today)),
                    ("payment", u.payment.clone()),
                    ("ask", tr().day_in(u.ask_by, today)),
                    ("days", delay.to_string()),
                ];
                say(if u.late { "money-topup-late" } else if delay == 0 { "money-topup-now" } else { "money-topup-ask" }, &args)
            })
            .collect();
        view.accounts.push(AccountView {
            id: account.id.clone(),
            title: account.title.clone(),
            kind: account.kind().to_string(),
            balance,
            fills: account.fills.iter().map(|b| Named { id: b.clone(), title: budget_title(b) }).collect(),
            floor: account.floor.cents() as f64 / 100.0,
            floor_text: if account.floor.cents() > 0 { tr().money(account.floor) } else { String::new() },
            topped_up_by: account.topped_up_by.iter().map(|r| Named { id: r.clone(), title: reserve_title(r) }).collect(),
            top_ups,
            movements,
            unplaced: mine.iter().filter(|p| p.why == Why::Unknown).count(),
            rules,
        });
    }
}

/// The budget file, where bank accounts, rules and choices are written.
fn ledger_path() -> Result<std::path::PathBuf, String> {
    load_config().case_store_path().map(|root| root.join(sioul_core::budget::LEDGER)).ok_or_else(|| tr().text("papers-no-store", None))
}

pub(crate) fn save_account(id: &str, edit: &str) -> String {
    let result = ledger_path().and_then(|path| {
        let edit: sioul_core::accounts::BankAccountEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
        sioul_core::accounts::save_bank_account(&path, id, &edit)
    });
    result.err().unwrap_or_default()
}

pub(crate) fn remove_account(id: &str) -> String {
    ledger_path().and_then(|path| sioul_core::accounts::remove_bank_account(&path, id)).err().unwrap_or_default()
}

/// One movement placed by your hand: `budget:<id>`, `none`, or `""` (as the rules say).
pub(crate) fn place_movement(account: &str, movement: &str, choice: &str) -> String {
    ledger_path().and_then(|path| sioul_core::accounts::set_assignment(&path, account, movement, choice)).err().unwrap_or_default()
}

/// A rule made (`place` below zero) or changed.
pub(crate) fn save_rule(place: i32, edit: &str) -> String {
    let result = ledger_path().and_then(|path| {
        let edit: sioul_core::accounts::SplitEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
        sioul_core::accounts::save_split(&path, usize::try_from(place).ok(), &edit)
    });
    result.err().unwrap_or_default()
}

pub(crate) fn remove_rule(place: i32) -> String {
    let Ok(place) = usize::try_from(place) else { return tr().text("bank-rule-gone", None) };
    ledger_path().and_then(|path| sioul_core::accounts::remove_split(&path, place)).err().unwrap_or_default()
}

pub(crate) fn save_reserve(id: &str, edit: &str) -> String {
    let result = ledger_path().and_then(|path| {
        let edit: sioul_core::accounts::ReserveEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
        sioul_core::accounts::save_reserve(&path, id, &edit, jiff::Zoned::now().date())
    });
    result.err().unwrap_or_default()
}

/// A bank export taken in; returns what it added, in a sentence, or what went wrong.
pub(crate) fn import(file: &str) -> (bool, String) {
    import_into(file, "")
}

/// An export taken in as one bank account's (`account` empty: as the export
/// names itself). The number the export gives is kept with the account, so
/// that what was read under it before is the account's too.
pub(crate) fn import_into(file: &str, account: &str) -> (bool, String) {
    let path = crate::backend::local_path(file);
    let Some(root) = load_config().case_store_path() else { return (false, tr().text("papers-no-store", None)) };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => return (false, format!("{}: {e}", path.display())),
    };
    // Some banks still write Latin-1.
    let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
    let statement = match sioul_core::bank::read(&text) {
        Ok(s) => s,
        Err(_) => return (false, say("bank-unreadable", &[("file", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())])),
    };
    let mut bank = match Bank::load(&root) {
        Ok(bank) => bank,
        Err(e) => return (false, e),
    };
    let title = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut statement = statement;
    if !account.is_empty() {
        // Kept under its own number when it gives one, which the account learns; else under the account.
        if statement.account.is_empty() {
            statement.account = account.to_string();
        } else if let Err(e) = ledger_path().and_then(|p| sioul_core::accounts::add_export_name(&p, account, &statement.account)) {
            return (false, e);
        }
    }
    let added = bank.import(&statement, &title);
    if let Err(e) = bank.save() {
        return (false, e);
    }
    let mut args = sioul_core::i18n::args();
    args.set("read", added.read as i64);
    args.set("new", added.new as i64);
    (true, tr().text("bank-imported", Some(&args)))
}

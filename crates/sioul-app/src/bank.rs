// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The money watch, for the window (docs/accounting.md, "The bank" and "Bank
//! accounts"): your bank accounts (a current account, PayPal, Stripe), each
//! with its exports taken in, the budgets it fills, where each movement went
//! and why, what tops it up; then in words what passed and what did not, the
//! week's payments, and the balance carried forward a month.

use crate::backend::{json, load_config, say, tr};
use serde::Serialize;
use sioul_core::bank::Bank;
use sioul_core::budget::Ledger;
use sioul_core::money::Money;

#[derive(Serialize, Default)]
struct View {
    store: bool,
    /// Movements or a balance were read once at least.
    any: bool,
    /// "€845.10 on Saturday 3 October, from your bank's file."
    balance: String,
    /// What the movements found, calmly: a payment not seen, an amount that
    /// changed, a payment the account would not hold.
    findings: Vec<String>,
    /// The week's payments: "Thursday 8 October · EDF · €62.00".
    coming: Vec<String>,
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
    /// Two of its exports' balances on one day differ: two real accounts
    /// under one (`accounts::two_accounts_in_one`), said calmly.
    two_accounts: bool,
    /// The budgets it fills, the one taking what nothing else places first.
    fills: Vec<Named>,
    floor: f64,
    /// "€100.00", or "" without a floor.
    floor_text: String,
    topped_up_by: Vec<Named>,
    /// What tops it up before its next income, in sentences (`ahead::Cover`).
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
    /// The bank's id, by which `place_movement` names it.
    id: String,
    /// Its steadiest key, by which a split, "Not the same" or "The same as…" name it.
    key: String,
    date: String,
    label: String,
    amount: String,
    cents: i64,
    /// Where it went, in words.
    place: String,
    /// "hand", "mail", "rule", "between", "preset", "first", "unknown", or "same" (joined to what was planned or told).
    why: String,
    /// Your choice for it: `budget:<id>`, `none`, or `""` (as the rules say).
    chosen: String,
    /// What tells of it, the bank's line with the rest: the mail's invoice, the letter, the contract.
    sources: Vec<sioul_core::moneyview::SourceView>,
    /// A source probably joined: "Not the same" beside it.
    probable: bool,
}

#[derive(Serialize)]
struct RuleView {
    /// Its id, by which a change names it; empty for a rule an older Sioul made.
    id: String,
    /// Its place among the rules of the file (a rule without an id is named by it).
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

/// The bank's part of the Budgets page, as JSON: from the money movements
/// (`movements::books`) and each account's stretch to its next income
/// (`ahead`), as the budgets page reads them.
pub(crate) fn view() -> String {
    let config = load_config();
    let looked = sioul_core::words::Words::of(&config);
    let Some(root) = config.notes_root_path() else { return json(&View { problem: tr().text("papers-no-store", None), ..View::default() }) };
    if let Err(problem) = Bank::load(&root) {
        return json(&View { store: true, problem, ..View::default() });
    }
    let today = sioul_core::zone::now().date();
    let books = sioul_core::movements::books(&root, &looked, today);
    let (bank, ledger) = (&books.bank, &books.counted);
    let aheads = sioul_core::ahead::of(ledger, bank, &books.movements, today);
    // Each account once, whatever names its exports gave it (`Bank::balance_in`).
    let mut view = View { store: true, any: !bank.movements.is_empty() || bank.balance_in(ledger).is_some(), ..View::default() };
    if let Some((date, amount)) = bank.balance_in(ledger) {
        view.balance = say("bank-balance", &[("amount", tr().money(amount)), ("date", tr().day_in(date, today))]);
    }
    view.findings = findings(&books.ledger, &books.movements, &aheads, today);
    view.attention = view.findings.len();
    let week = today.checked_add(jiff::Span::new().days(7)).unwrap_or(today);
    let coming: Vec<&sioul_core::ahead::Flow> = aheads.iter().flat_map(|a| a.flows.iter()).filter(|f| f.day <= week && f.amount.is_negative()).collect();
    view.coming = coming.iter().map(|f| format!("{} · {} · {}", tr().day_in(f.day, today), f.label, tr().money(Money(-f.amount.cents())))).collect();
    if !coming.is_empty() {
        let list = coming.iter().take(4).map(|f| format!("{} {} ({})", f.label, tr().money(Money(-f.amount.cents())), tr().weekday_short(f.day))).collect::<Vec<_>>().join(", ");
        let held = aheads.iter().all(|a| a.short.as_ref().is_none_or(|s| s.day > week));
        view.week = say(if held { "bank-week-held" } else { "bank-week-short" }, &[("list", list)]);
    }
    accounts_into(&mut view, &books, &aheads, today);
    json(&view)
}

/// What the movements found, in words, once each on the reminders' side:
/// a payment not seen on its account, a recurring payment that took another
/// amount, a payment an account would not hold before its next income.
fn findings(file: &Ledger, movements: &[sioul_core::movements::Movement], aheads: &[sioul_core::ahead::Ahead], today: jiff::civil::Date) -> Vec<String> {
    use sioul_core::movements::{Kind, State, Way};
    let day = |d: jiff::civil::Date| tr().day_in(d, today);
    let month_ago = today.checked_sub(jiff::Span::new().days(31)).unwrap_or(today);
    let mut out = Vec::new();
    for m in movements.iter().filter(|m| m.day >= month_ago && m.day <= today) {
        let amount = tr().money(Money(m.sum.amount().cents().abs()));
        if m.state == State::NotSeen {
            let title = say(if m.way == Way::In { "money-missed-in" } else { "money-missed-out" }, &[("label", m.label.clone()), ("amount", amount)]);
            out.push(format!("{title}. {}", say("money-missed-why", &[("date", day(m.day))])));
            continue;
        }
        // A recurring payment that took another amount: by a twentieth, and €2 at least.
        let preset = m.sources.iter().find(|s| s.kind == Kind::Preset).and_then(|s| s.key.strip_prefix("preset:")).and_then(|k| k.rsplit_once('/')).and_then(|(p, _)| (0..file.presets.len()).find(|&i| sioul_core::movements::preset_key(file, i) == p)).map(|i| &file.presets[i]);
        if m.state == State::Done
            && m.sources.iter().any(|s| s.kind == Kind::Bank)
            && let Some(preset) = preset.filter(|p| !p.about && p.low.is_none())
        {
            let gap = (m.sum.amount().cents() - preset.amount.cents()).abs();
            if gap > preset.amount.cents().abs() / 20 && gap > 200 {
                out.push(say("money-changed", &[("label", preset.label.clone()), ("actual", amount), ("date", day(m.day)), ("expected", tr().money(Money(preset.amount.cents().abs())))]));
            }
        }
    }
    out.extend(sioul_core::ahead::shortfalls(aheads).iter().map(|s| s.sentence(tr(), today)));
    out
}

/// The bank accounts' part of the view: each with its balance, its budgets,
/// its movements placed and why, with what else tells of each, its rules,
/// what tops it up.
fn accounts_into(view: &mut View, books: &sioul_core::movements::Books, aheads: &[sioul_core::ahead::Ahead], today: jiff::civil::Date) {
    use sioul_core::accounts::{self, Why};
    use sioul_core::movements::{Kind, Place, State};
    let (bank, ledger) = (&books.bank, &books.ledger);
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
    for account in &ledger.bank_accounts {
        let balance = accounts::balance_of(bank, account).map(|(date, amount)| say("bank-balance", &[("amount", tr().money(amount)), ("date", tr().day_in(date, today))])).unwrap_or_default();
        // Its own lines, as the matching left them: each with whatever else tells of it.
        let mine: Vec<&sioul_core::movements::Movement> = books.movements.iter().filter(|m| m.account.as_deref() == Some(account.id.as_str()) && m.sources.iter().any(|s| matches!(s.kind, Kind::Bank | Kind::Hold)) && matches!(m.state, State::Done | State::NotFinal)).collect();
        let movements = mine
            .iter()
            .rev()
            .take(120)
            .map(|m| {
                let bank_source = m.sources.iter().find(|s| matches!(s.kind, Kind::Bank | Kind::Hold));
                let id = bank_source.and_then(|s| s.key.rsplit_once('/')).map(|(_, id)| id.to_string()).unwrap_or_default();
                let place = match &m.place {
                    Place::Budget(b) => {
                        let preset = m.stands_for().map(preset_label).filter(|l| !l.is_empty());
                        match preset {
                            Some(label) => format!("{} · {label}", budget_title(b)),
                            None => budget_title(b),
                        }
                    }
                    Place::Parts(parts) => parts.iter().map(|(b, a)| format!("{} {}", budget_title(b), tr().money(Money(a.cents().abs())))).collect::<Vec<_>>().join(", "),
                    Place::Reserve { reserve, budget } => say("bank-place-reserve", &[("reserve", reserve_title(reserve)), ("budget", budget_title(budget))]),
                    Place::Transfer { from, to } => say("bank-place-transfer", &[("account", account_title(if *from == account.id { to } else { from }))]),
                    Place::Nowhere | Place::Unplaced => tr().text("bank-place-none", None),
                };
                // Joined to what was planned or told: said as the same payment.
                let joined = m.sources.iter().any(|s| !matches!(s.kind, Kind::Bank | Kind::Hold | Kind::Contract));
                let why = match m.why_placed() {
                    _ if joined => "same",
                    Some(Why::Hand) => "hand",
                    Some(Why::Mail) => "mail",
                    Some(Why::Rule) => "rule",
                    Some(Why::Between) => "between",
                    Some(Why::Preset) => "preset",
                    Some(Why::First) => "first",
                    Some(Why::Unknown) | None => "unknown",
                };
                let chosen = ledger
                    .assignments
                    .iter()
                    .find(|a| account.owns(&a.account) && a.movement == id)
                    .map(|a| if a.none { "none".to_string() } else { a.budget.as_deref().map(|b| format!("budget:{b}")).unwrap_or_default() })
                    .unwrap_or_default();
                let row = sioul_core::moneyview::coming_row(ledger, m, &Default::default(), tr(), today);
                MovementView {
                    id,
                    key: m.steady_key().to_string(),
                    date: tr().day_in(m.day, today),
                    label: row.who.clone(),
                    amount: tr().money(m.sum.amount()),
                    cents: m.sum.amount().cents(),
                    place,
                    why: why.to_string(),
                    chosen,
                    probable: row.probable,
                    sources: row.sources,
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
                RuleView { id: s.id.clone(), place, words: s.words.clone(), direction: direction.to_string(), to, to_title, everywhere: s.account.is_empty() }
            })
            .collect();
        let stretch = aheads.iter().find(|a| a.account == account.id);
        let top_ups = stretch
            .and_then(|a| a.short.as_ref().map(|short| (a, short)))
            .map(|(a, short)| {
                a.covers
                    .iter()
                    .map(|c| {
                        let args = [
                            ("reserve", c.title.clone()),
                            ("amount", tr().money(c.amount)),
                            ("date", tr().day_in(short.day, today)),
                            ("payment", short.payment.clone()),
                            ("ask", tr().day_in(c.by, today)),
                            ("days", c.delay.to_string()),
                        ];
                        say(if c.late { "money-topup-late" } else if c.delay == 0 { "money-topup-now" } else { "money-topup-ask" }, &args)
                    })
                    .collect()
            })
            .unwrap_or_default();
        view.accounts.push(AccountView {
            id: account.id.clone(),
            title: account.title.clone(),
            kind: account.kind().to_string(),
            balance,
            two_accounts: accounts::two_accounts_in_one(bank, account),
            fills: account.fills.iter().map(|b| Named { id: b.clone(), title: budget_title(b) }).collect(),
            floor: account.floor.cents() as f64 / 100.0,
            floor_text: if account.floor.cents() > 0 { tr().money(account.floor) } else { String::new() },
            topped_up_by: account.topped_up_by.iter().map(|r| Named { id: r.clone(), title: reserve_title(r) }).collect(),
            top_ups,
            unplaced: mine.iter().filter(|m| m.why_placed() == Some(Why::Unknown) && matches!(m.place, Place::Nowhere | Place::Unplaced)).count(),
            movements,
            rules,
        });
    }
}

/// The budget file, where bank accounts, rules and choices are written.
fn ledger_path() -> Result<std::path::PathBuf, String> {
    load_config().notes_root_path().map(|root| root.join(sioul_core::budget::LEDGER)).ok_or_else(|| tr().text("papers-no-store", None))
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

/// A rule made (`shown` empty) or changed: `shown` the rule as the window
/// showed it (JSON, `RuleView`), found by its id, or at its place while it
/// still holds what was shown (`accounts::save_split`).
pub(crate) fn save_rule(shown: &str, edit: &str) -> String {
    let result = ledger_path().and_then(|path| {
        let edit: sioul_core::accounts::SplitEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
        let shown: Option<sioul_core::accounts::SplitShown> = (!shown.trim().is_empty()).then(|| serde_json::from_str(shown)).transpose().map_err(|e| e.to_string())?;
        sioul_core::accounts::save_split(&path, shown.as_ref(), &edit)
    });
    match result {
        Err(e) if e == "no such rule" => tr().text("bank-rule-gone", None),
        other => other.err().unwrap_or_default(),
    }
}

/// The rule the window showed (JSON, `RuleView`) taken out.
pub(crate) fn remove_rule(shown: &str) -> String {
    let Ok(shown) = serde_json::from_str::<sioul_core::accounts::SplitShown>(shown) else { return tr().text("bank-rule-gone", None) };
    match ledger_path().and_then(|path| sioul_core::accounts::remove_split(&path, &shown)) {
        Err(e) if e == "no such rule" => tr().text("bank-rule-gone", None),
        other => other.err().unwrap_or_default(),
    }
}

pub(crate) fn save_reserve(id: &str, edit: &str) -> String {
    let result = ledger_path().and_then(|path| {
        let edit: sioul_core::accounts::ReserveEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
        sioul_core::accounts::save_reserve(&path, id, &edit, sioul_core::zone::now().date())
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
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return (false, tr().text("papers-no-store", None)) };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => return (false, format!("{}: {e}", path.display())),
    };
    // Some banks still write Latin-1.
    let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
    let statement = match sioul_core::bank::read(&sioul_core::words::Words::of(&config).bank, &text) {
        Ok(s) => s,
        Err(_) => return (false, say("bank-unreadable", &[("file", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())])),
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
    // Taken into the movements as the file holds them now, under its lock, which the
    // sharing takes too; its holds kept apart, its new lines resolved against what
    // was planned and announced (`movements::acts::take_in`).
    let words = sioul_core::words::Words::of(&config);
    let added = match sioul_core::movements::acts::take_in(&root, &statement, &title, &words, sioul_core::zone::now().date()) {
        Ok(added) => added,
        Err(e) => return (false, e),
    };
    let mut args = sioul_core::i18n::args();
    args.set("read", added.read as i64);
    args.set("new", added.new as i64);
    (true, tr().text("bank-imported", Some(&args)))
}

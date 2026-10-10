// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Bank accounts and budgets (docs/accounting.md, "Bank accounts"). Where
//! money is (a current account, PayPal, Stripe) and what it is for (the
//! budgets) are two things: each movement an account's exports bring goes to
//! a budget, in parts between budgets, with a reserve, or to none (money moved
//! between your own accounts). By your hand for that movement; else the same
//! payment as a line the budgets hold already (from mail), counted there once;
//! else money moved to or from PayPal or Stripe, when their own exports cover
//! the day; else the first rule that names it (or, without words, every
//! movement its way); else the recurring payment it stands for; else the
//! account's first budget. A card payment of daily life
//! is spent from its budget's estimate (food, clothes), never added on top of
//! it. Reserves top the accounts up, in your order, each with its own delay.

use crate::bank::{self, Bank, Expected, Movement};
use crate::budget::{BankAccount, Direction, Ledger, Line, Period};
use crate::money::Money;
use jiff::ToSpan;
use jiff::civil::Date;
use std::collections::BTreeMap;

/// A movement's share in one budget, and the preset it stands for or is spent from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub budget: String,
    pub amount: Money,
    pub preset: Option<String>,
}

/// Where a movement goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Into budgets, each its share.
    Budgets(Vec<Share>),
    /// Money moved with a savings account, through a budget.
    Reserve { reserve: String, budget: String },
    /// Moved between your own accounts: in no budget.
    Transfer(String),
    /// The same payment as this line of the budget file (from mail, by hand): counted there.
    Known(usize),
    /// In none: the account fills no budget, or you said so.
    Nowhere,
}

/// Why it went there, for the window to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Hand,
    Mail,
    Rule,
    Between,
    Preset,
    First,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Placed {
    /// The bank account's id.
    pub account: String,
    pub movement: Movement,
    pub place: Place,
    pub why: Why,
}

/// The bank account a movement kept under `name` belongs to.
pub fn account_of<'a>(ledger: &'a Ledger, name: &str) -> Option<&'a BankAccount> {
    ledger.bank_accounts.iter().find(|a| a.owns(name))
}

/// A preset's name for lines: its id, else its place in the file (`#3`).
fn key(ledger: &Ledger, index: usize) -> String {
    ledger.presets[index].id.clone().unwrap_or_else(|| format!("#{index}"))
}

fn folded(text: &str) -> String {
    crate::text::fold(text).into_iter().collect()
}


/// Every movement of the declared bank accounts, placed (see the module's
/// notes), oldest first. Movements of no declared account are left out: they
/// only feed the money watch.
/// `looked`: the words of labels that name nothing, and what PayPal and
/// Stripe write for money moved to or from a bank (`words::AccountWords`).
pub fn place(ledger: &Ledger, bank: &Bank, looked: &crate::words::Words) -> Vec<Placed> {
    let mut movements: Vec<(&BankAccount, &Movement)> = bank.movements.iter().filter_map(|m| account_of(ledger, &m.account).map(|a| (a, m))).collect();
    movements.sort_by(|a, b| (a.1.date, &a.1.id).cmp(&(b.1.date, &b.1.id)));
    // A movement read under two names of one account (an export first read
    // without its number, later with it) is one: counted once.
    let mut seen: std::collections::BTreeSet<(&str, &str)> = std::collections::BTreeSet::new();
    movements.retain(|(account, m)| m.id.is_empty() || seen.insert((account.id.as_str(), m.id.as_str())));
    // What each account's exports cover: a movement elsewhere names it only inside.
    let mut covered: BTreeMap<&str, (Date, Date)> = BTreeMap::new();
    for (account, m) in &movements {
        let span = covered.entry(account.id.as_str()).or_insert((m.date, m.date));
        span.0 = span.0.min(m.date);
        span.1 = span.1.max(m.date);
    }
    let mut known_taken = vec![false; ledger.lines.len()];
    let mut preset_taken: Vec<(usize, Date)> = Vec::new();
    let mut out = Vec::new();
    for (account, m) in movements {
        let first = account.fills.first().cloned();
        let whole = |budget: String, preset: Option<String>| Place::Budgets(vec![Share { budget, amount: m.amount, preset }]);
        let label = folded(&m.label);
        // 1. Your hand.
        if let Some(a) = ledger.assignments.iter().find(|a| account.owns(&a.account) && a.movement == m.id) {
            let place = if a.none {
                Place::Nowhere
            } else if a.parts.is_empty() {
                a.budget.clone().or(first.clone()).map_or(Place::Nowhere, |b| whole(b, None))
            } else {
                let mut shares: Vec<Share> = a.parts.iter().map(|p| Share { budget: p.budget.clone(), amount: p.amount, preset: None }).collect();
                let rest = m.amount - shares.iter().map(|s| s.amount).sum::<Money>();
                if rest != Money::ZERO
                    && let Some(budget) = a.budget.clone().or(first.clone())
                {
                    shares.push(Share { budget, amount: rest, preset: None });
                }
                Place::Budgets(shares)
            };
            out.push(Placed { account: account.id.clone(), movement: m.clone(), place, why: Why::Hand });
            continue;
        }
        // 2. A line the budgets hold already: read from mail on the day, the bank a few days after.
        let from = m.date.checked_sub(7.days()).unwrap_or(m.date);
        let to = m.date.checked_add(3.days()).unwrap_or(m.date);
        let known = ledger.lines.iter().enumerate().position(|(i, l)| {
            !known_taken[i] && !l.planned && l.reserve.is_none() && l.amount == m.amount && l.date >= from && l.date <= to && (account.fills.is_empty() || account.fills.contains(&l.budget))
        });
        if let Some(i) = known {
            known_taken[i] = true;
            out.push(Placed { account: account.id.clone(), movement: m.clone(), place: Place::Known(i), why: Why::Mail });
            continue;
        }
        // 3. Money moved with PayPal or Stripe, when their own exports cover the day:
        // both sides read, it counts in no budget, whatever a rule says of it.
        let partner = ledger.bank_accounts.iter().filter(|other| other.id != account.id).find(|other| {
            let named = match other.kind() {
                "paypal" => label.contains("paypal"),
                "stripe" => label.contains("stripe"),
                _ => matches!(account.kind(), "paypal" | "stripe") && looked.accounts.between.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && label.contains(&w)),
            };
            named && covered.get(other.id.as_str()).is_some_and(|(a, b)| m.date >= a.checked_sub(10.days()).unwrap_or(*a) && m.date <= b.checked_add(10.days()).unwrap_or(*b))
        });
        if let Some(other) = partner {
            out.push(Placed { account: account.id.clone(), movement: m.clone(), place: Place::Transfer(other.id.clone()), why: Why::Between });
            continue;
        }
        // 4. The first rule that names it; a rule without words takes every movement its way.
        let rule = ledger.splits.iter().find(|s| {
            (s.account.is_empty() || account.owns(&s.account))
                && s.direction.is_none_or(|d| (d == Direction::Credit) == !m.amount.is_negative())
                && ((s.words.iter().all(|w| w.trim().is_empty()) && s.direction.is_some()) || s.words.iter().any(|w| !w.trim().is_empty() && label.contains(&folded(w.trim()))))
        });
        if let Some(rule) = rule {
            let preset = rule.preset.as_deref().and_then(|id| ledger.presets.iter().position(|p| p.id.as_deref() == Some(id)));
            let place = if let Some(other) = &rule.transfer {
                Place::Transfer(other.clone())
            } else if let Some(reserve) = &rule.reserve {
                rule.budget.clone().or(first.clone()).map_or(Place::Nowhere, |budget| Place::Reserve { reserve: reserve.clone(), budget })
            } else {
                let budget = rule.budget.clone().or_else(|| preset.map(|i| ledger.presets[i].budget.clone())).or(first.clone());
                budget.map_or(Place::Nowhere, |b| whole(b, preset.map(|i| key(ledger, i))))
            };
            out.push(Placed { account: account.id.clone(), movement: m.clone(), place, why: Why::Rule });
            continue;
        }
        // 5. The recurring payment it stands for, in a budget it fills.
        let preset = ledger.presets.iter().enumerate().filter(|(_, p)| !p.estimate && (account.fills.is_empty() || account.fills.contains(&p.budget))).find_map(|(i, p)| {
            let words = bank::words(&looked.bank.filler, &p.label);
            let around = (m.date.checked_sub(10.days()).ok()?, m.date.checked_add(5.days()).ok()?);
            p.occurrences(around.0, around.1).into_iter().find(|d| !preset_taken.contains(&(i, *d)) && bank::matches(&looked.bank.filler, m, &words, p.amount, *d)).map(|d| (i, d))
        });
        if let Some((i, date)) = preset {
            preset_taken.push((i, date));
            out.push(Placed { account: account.id.clone(), movement: m.clone(), place: whole(ledger.presets[i].budget.clone(), Some(key(ledger, i))), why: Why::Preset });
            continue;
        }
        // 6. The account's first budget.
        let (place, why) = match first {
            Some(budget) => (whole(budget, None), Why::First),
            None => (Place::Nowhere, Why::Unknown),
        };
        out.push(Placed { account: account.id.clone(), movement: m.clone(), place, why });
    }
    spend_from_estimates(ledger, &mut out);
    out
}

/// Payments out that stand for nothing are spent from their budget's
/// estimates (food, clothes), the one with the most left in its period first;
/// past them, they count on top.
fn spend_from_estimates(ledger: &Ledger, placed: &mut [Placed]) {
    // What each estimate's envelope holds per period, less what lines of the file spend from it.
    let mut left: BTreeMap<(usize, Date), i64> = BTreeMap::new();
    let room = |left: &mut BTreeMap<(usize, Date), i64>, i: usize, date: Date| -> Option<(Date, i64)> {
        let p = &ledger.presets[i];
        let (start, end) = p.every.bounds(date);
        if p.occurrences(start, end).is_empty() {
            return None;
        }
        let entry = left.entry((i, start)).or_insert_with(|| {
            let spent: i64 = ledger.lines.iter().filter(|l| l.preset.is_some() && l.preset == p.id && l.date >= start && l.date <= end).map(|l| l.amount.cents()).sum();
            (-p.amount.cents() + spent).max(0)
        });
        Some((start, *entry))
    };
    for item in placed.iter_mut() {
        let Place::Budgets(shares) = &mut item.place else { continue };
        for share in shares.iter_mut().filter(|s| s.preset.is_none() && s.amount.is_negative()) {
            let date = item.movement.date;
            let best = (0..ledger.presets.len())
                .filter(|&i| ledger.presets[i].estimate && ledger.presets[i].budget == share.budget && ledger.presets[i].amount.is_negative())
                .filter_map(|i| room(&mut left, i, date).map(|(start, cents)| (i, start, cents)))
                .filter(|(_, _, cents)| *cents > 0)
                .max_by_key(|(_, _, cents)| *cents);
            if let Some((i, start, _)) = best {
                share.preset = Some(key(ledger, i));
                if let Some(cents) = left.get_mut(&(i, start)) {
                    *cents -= share.amount.cents().abs();
                }
            }
        }
    }
}

impl Ledger {
    /// The ledger with its bank accounts' movements as lines, done on their
    /// day, for the budgets to count (`budget::Ledger::status`): the presets
    /// each stands for are named (`#3` for one without an id). Kept in memory
    /// only; the budget file is not written.
    pub fn with_bank(&self, bank: &Bank, looked: &crate::words::Words) -> Ledger {
        let mut out = self.clone();
        if self.bank_accounts.is_empty() {
            return out;
        }
        for (i, preset) in out.presets.iter_mut().enumerate() {
            if preset.id.is_none() {
                preset.id = Some(format!("#{i}"));
            }
        }
        for item in place(self, bank, looked) {
            let link = format!("bank:{}/{}", item.account, item.movement.id);
            let line = |budget: String, amount: Money, preset: Option<String>, reserve: Option<String>| Line {
                budget,
                date: item.movement.date,
                amount,
                label: item.movement.label.clone(),
                planned: false,
                reserve,
                links: vec![link.clone()],
                preset,
            };
            match item.place {
                Place::Budgets(shares) => out.lines.extend(shares.into_iter().map(|s| line(s.budget, s.amount, s.preset, None))),
                Place::Reserve { reserve, budget } => out.lines.push(line(budget, item.movement.amount, None, Some(reserve))),
                Place::Transfer(_) | Place::Known(_) | Place::Nowhere => {}
            }
        }
        out
    }
}

/// Money a reserve should send to a bank account before it runs under its floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopUp {
    pub account: String,
    pub reserve: String,
    pub amount: Money,
    /// The day the account would go under its floor, and the payment then.
    pub needed_by: Date,
    pub payment: String,
    /// The last day to ask, the reserve's delay before `needed_by`; today when already past.
    pub ask_by: Date,
    /// Asked today, it still arrives after `needed_by`.
    pub late: bool,
}

/// A bank account's balance known, and its day: the newest of its exports',
/// whatever name each gave it (`BankAccount::exports`: its number, its IBAN,
/// none). Each name is the same account, so their balances are one balance
/// at different days, never added: an export first read without a number,
/// later with one, counted the same money twice (review of 5 October 2026).
/// Two real accounts are two cards.
pub fn balance_of(bank: &Bank, account: &BankAccount) -> Option<(Date, Money)> {
    bank.accounts.iter().filter(|a| account.owns(&a.id)).filter_map(|a| a.balance).max_by_key(|(day, _)| *day)
}

/// A bank account's movements under every name its exports gave it, each
/// once: the same movement read under two names (the bank's own id) is one.
pub fn movements_of<'a>(bank: &'a Bank, account: &BankAccount) -> Vec<&'a Movement> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    bank.movements.iter().filter(|m| account.owns(&m.account)).filter(|m| m.id.is_empty() || seen.insert(m.id.as_str())).collect()
}

/// The budgets whose payments a bank account carries forward: those it is the
/// first account to fill (a budget filled by two counts on the first).
pub fn carried_by<'a>(ledger: &'a Ledger, account: &BankAccount) -> Vec<String> {
    account.fills.iter().filter(|b| ledger.bank_accounts.iter().find(|a| a.fills.contains(b)).is_some_and(|a| a.id == account.id)).cloned().collect()
}

/// What tops each bank account up in the coming month: its balance carried
/// forward with its own movements and its budgets' recurring payments and
/// planned lines; on the first day it would go under its floor, what is
/// missing (rounded up to ten), taken from its reserves in order, each as far
/// as it holds above its own floor, asked its delay ahead.
pub fn top_ups(ledger: &Ledger, bank: &Bank, today: Date, looked: &crate::words::Words) -> Vec<TopUp> {
    let horizon = today.checked_add(31.days()).unwrap_or(today);
    // The reserves' balances count the money already moved, read from the exports.
    let ledger = &ledger.with_bank(bank, looked);
    let mut out = Vec::new();
    for account in ledger.bank_accounts.iter().filter(|a| !a.topped_up_by.is_empty()) {
        let Some((as_of, start)) = balance_of(bank, account) else { continue };
        let movements: Vec<&Movement> = movements_of(bank, account);
        let budgets = carried_by(ledger, account);
        let flows: Vec<Expected> = bank::expected(&movements, ledger, Some(&budgets), as_of, today, horizon, &looked.bank.filler);
        let (_, below) = bank::carry(start, as_of, &flows, today, horizon, account.floor);
        let Some((day, at, balance)) = below else { continue };
        let missing = account.floor.cents() - balance.cents();
        let mut missing = (missing + 999) / 1000 * 1000;
        for id in &account.topped_up_by {
            let Some(reserve) = ledger.reserves.iter().find(|r| &r.id == id) else { continue };
            let room = (ledger.reserve_status(reserve, today).balance_now - reserve.floor).cents();
            if room <= 0 || missing <= 0 {
                continue;
            }
            let amount = missing.min(room);
            missing -= amount;
            let delay = i64::from(reserve.delay_days);
            let ask_by = day.checked_sub(delay.days()).unwrap_or(day).max(today);
            let late = today.checked_add(delay.days()).unwrap_or(today) > day;
            out.push(TopUp { account: account.id.clone(), reserve: reserve.id.clone(), amount: Money(amount), needed_by: day, payment: flows[at].label.clone(), ask_by, late });
        }
    }
    out
}

/// The period a movement's month is in, for the window: "October 2026" groups.
pub fn month_of(date: Date) -> Date {
    Period::Month.bounds(date).0
}

impl Ledger {
    /// The budget file at the root of a notes folder, with its bank accounts'
    /// movements counted (`with_bank`) when it declares some.
    pub fn load_with_bank(root: &std::path::Path, looked: &crate::words::Words) -> Result<Ledger, String> {
        let ledger = Ledger::load(root)?;
        if ledger.bank_accounts.is_empty() {
            return Ok(ledger);
        }
        Ok(match Bank::load(root) {
            Ok(bank) => ledger.with_bank(&bank, looked),
            Err(_) => ledger,
        })
    }
}

// ---- Written into the budget file, its comments and other fields kept.

use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};

/// The budget file, empty when there is none yet; one that cannot be read
/// is an error, never an empty file written over it.
fn open(path: &std::path::Path) -> Result<DocumentMut, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    text.parse().map_err(|e: toml_edit::TomlError| format!("{}: {e}", path.display()))
}

fn write(path: &std::path::Path, doc: &DocumentMut) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let temporary = path.with_extension("toml.new");
    std::fs::write(&temporary, doc.to_string()).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)
}

fn tables<'a>(doc: &'a mut DocumentMut, name: &str) -> Result<&'a mut ArrayOfTables, String> {
    doc.entry(name).or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| format!("`{name}` is not a list of tables"))
}

fn words_array(list: &[String]) -> Array {
    list.iter().map(|w| w.trim()).filter(|w| !w.is_empty()).collect()
}

fn money_value(m: f64) -> Item {
    value((m * 100.0).round() / 100.0)
}

fn date_value(d: Date) -> Item {
    value(toml_edit::Datetime { date: Some(toml_edit::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None })
}

/// A bank account as the window gives it.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct BankAccountEdit {
    pub title: String,
    #[serde(default)]
    pub kind: String,
    /// The budgets it fills, the one taking what nothing else places first.
    #[serde(default)]
    pub fills: Vec<String>,
    #[serde(default)]
    pub floor: f64,
    /// The reserves that top it up, the first first.
    #[serde(default)]
    pub topped_up_by: Vec<String>,
}

/// Makes a bank account (`id` empty) or changes one; returns its id.
pub fn save_bank_account(path: &std::path::Path, id: &str, edit: &BankAccountEdit) -> Result<String, String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let title = edit.title.trim();
        if title.is_empty() {
            return Err(format!("{}: a bank account needs a name", path.display()));
        }
        let mut doc = open(path)?;
        let accounts = tables(&mut doc, "bank_account")?;
        let taken: Vec<String> = accounts.iter().filter_map(|t| t.get("id").and_then(Item::as_str).map(str::to_string)).collect();
        let id = if id.is_empty() { crate::projects::new_id(title, &taken) } else { id.to_string() };
        if !taken.contains(&id) {
            let mut table = Table::new();
            table["id"] = value(id.as_str());
            accounts.push(table);
        }
        let table = accounts.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id.as_str())).ok_or_else(|| format!("no bank account {id}"))?;
        table["title"] = value(title);
        match edit.kind.as_str() {
            "paypal" | "stripe" | "other" => table["kind"] = value(edit.kind.as_str()),
            _ => {
                table.remove("kind");
            }
        }
        table["fills"] = value(words_array(&edit.fills));
        if edit.floor > 0.0 {
            table["floor"] = money_value(edit.floor);
        } else {
            table.remove("floor");
        }
        table["topped_up_by"] = value(words_array(&edit.topped_up_by));
        write(path, &doc)?;
        Ok(id)
    })
}

/// Takes a bank account out; its movements stay in `sioul-bank.toml`, its rules and choices here.
pub fn remove_bank_account(path: &std::path::Path, id: &str) -> Result<(), String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let mut doc = open(path)?;
        let accounts = tables(&mut doc, "bank_account")?;
        let before = accounts.len();
        accounts.retain(|t| t.get("id").and_then(Item::as_str) != Some(id));
        if accounts.len() == before {
            return Err(format!("no bank account {id}"));
        }
        write(path, &doc)
    })
}

/// Another name the exports give a bank account (an account number), kept so
/// that its movements read under it are its own.
pub fn add_export_name(path: &std::path::Path, id: &str, name: &str) -> Result<(), String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let mut doc = open(path)?;
        let accounts = tables(&mut doc, "bank_account")?;
        let table = accounts.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id)).ok_or_else(|| format!("no bank account {id}"))?;
        let mut names: Vec<String> = table.get("exports").and_then(Item::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
        if names.iter().any(|n| n == name) || name == id {
            return Ok(());
        }
        names.push(name.to_string());
        table["exports"] = value(words_array(&names));
        write(path, &doc)
    })
}

/// Where one movement goes, by your hand: `budget:<id>`, `none` (in no budget),
/// or `""` (as the rules say again).
pub fn set_assignment(path: &std::path::Path, account: &str, movement: &str, choice: &str) -> Result<(), String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let mut doc = open(path)?;
        let all = tables(&mut doc, "assign")?;
        all.retain(|t| !(t.get("account").and_then(Item::as_str) == Some(account) && t.get("movement").and_then(Item::as_str) == Some(movement)));
        if !choice.is_empty() {
            let mut table = Table::new();
            table["account"] = value(account);
            table["movement"] = value(movement);
            match choice.strip_prefix("budget:") {
                Some(budget) => table["budget"] = value(budget),
                None if choice == "none" => table["none"] = value(true),
                None => return Err(format!("{choice}: a budget or none")),
            }
            all.push(table);
        }
        write(path, &doc)
    })
}

/// A rule as the window gives it: words, which way, and where to: `budget:<id>`,
/// `reserve:<id>`, `transfer:<bank account>`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct SplitEdit {
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub words: Vec<String>,
    /// "credit", "debit", or either.
    #[serde(default)]
    pub direction: String,
    pub to: String,
}

/// A rule as the window showed it: its id (empty for one an older Sioul
/// made), its place then, and what it held then.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct SplitShown {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub place: usize,
    #[serde(default)]
    pub words: Vec<String>,
    /// "credit", "debit", "".
    #[serde(default)]
    pub direction: String,
    /// `budget:<id>`, `reserve:<id>`, `transfer:<id>`, `preset:<id>`.
    #[serde(default)]
    pub to: String,
}

/// Where the rule the window showed is among the rules now: by its id; one
/// without (an older Sioul's) at its place, only while it still holds what
/// was shown (another device's change may have moved the rules since).
fn split_at(all: &toml_edit::ArrayOfTables, shown: &SplitShown) -> Option<usize> {
    let text = |t: &Table, key: &str| t.get(key).and_then(toml_edit::Item::as_str).unwrap_or_default().to_string();
    if !shown.id.is_empty() {
        return all.iter().position(|t| text(t, "id") == shown.id);
    }
    let table = all.get(shown.place)?;
    let words: Vec<String> = table.get("words").and_then(toml_edit::Item::as_array).map(|a| a.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let to = ["budget", "reserve", "transfer", "preset"].iter().find_map(|key| table.get(key).and_then(toml_edit::Item::as_str).map(|v| format!("{key}:{v}"))).unwrap_or_default();
    let holds = text(table, "id").is_empty() && words == shown.words && text(table, "direction") == shown.direction && to == shown.to;
    holds.then_some(shown.place)
}

/// Makes a rule (`shown` None, at the end) or changes the one the window
/// showed (`split_at`): read, changed and written under the file's lock.
pub fn save_split(path: &std::path::Path, shown: Option<&SplitShown>, edit: &SplitEdit) -> Result<(), String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let words: Vec<String> = edit.words.iter().map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect();
        // Without words, a rule takes every movement one way: it needs that way.
        if words.is_empty() && !matches!(edit.direction.as_str(), "credit" | "debit") {
            return Err(format!("{}: a rule needs a word, or a way (in or out)", path.display()));
        }
        let mut table = Table::new();
        if !edit.account.is_empty() {
            table["account"] = value(edit.account.as_str());
        }
        table["words"] = value(words_array(&words));
        if matches!(edit.direction.as_str(), "credit" | "debit") {
            table["direction"] = value(edit.direction.as_str());
        }
        let (key, target) = edit.to.split_once(':').ok_or_else(|| format!("{}: where to?", edit.to))?;
        match key {
            "budget" | "reserve" | "transfer" | "preset" => table[key] = value(target),
            _ => return Err(format!("{}: where to?", edit.to)),
        }
        let mut doc = open(path)?;
        let all = tables(&mut doc, "split")?;
        let place = match shown {
            Some(shown) => Some(split_at(all, shown).ok_or("no such rule")?),
            None => None,
        };
        // Its id (`ids`), by which your devices merge it: kept when the rule is
        // changed, given when it is made (or changed, made by an older Sioul).
        let id = place.and_then(|at| all.get(at)).and_then(|old| old.get("id").and_then(toml_edit::Item::as_str).map(str::to_string)).unwrap_or_else(crate::ids::new);
        let mut table = table;
        table.insert("id", value(id));
        match place {
            Some(at) if at < all.len() => *all.get_mut(at).ok_or("no such rule")? = table,
            Some(_) => return Err("no such rule".into()),
            None => all.push(table),
        }
        write(path, &doc)
    })
}

/// Takes out the rule the window showed (`split_at`), under the file's lock.
pub fn remove_split(path: &std::path::Path, shown: &SplitShown) -> Result<(), String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let mut doc = open(path)?;
        let all = tables(&mut doc, "split")?;
        let place = split_at(all, shown).ok_or("no such rule")?;
        all.remove(place);
        write(path, &doc)
    })
}

/// A reserve as the window gives it.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ReserveEdit {
    pub title: String,
    pub balance: f64,
    /// "2026-10-01"; today when empty.
    #[serde(default)]
    pub as_of: String,
    #[serde(default)]
    pub floor: f64,
    #[serde(default)]
    pub delay_days: u32,
}

/// Makes a reserve (`id` empty) or changes one; returns its id.
pub fn save_reserve(path: &std::path::Path, id: &str, edit: &ReserveEdit, today: Date) -> Result<String, String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let title = edit.title.trim();
        if title.is_empty() {
            return Err(format!("{}: a reserve needs a name", path.display()));
        }
        let as_of = if edit.as_of.trim().is_empty() { today } else { edit.as_of.trim().parse::<Date>().map_err(|e| format!("{}: {e}", edit.as_of))? };
        let mut doc = open(path)?;
        let reserves = tables(&mut doc, "reserve")?;
        let taken: Vec<String> = reserves.iter().filter_map(|t| t.get("id").and_then(Item::as_str).map(str::to_string)).collect();
        let id = if id.is_empty() { crate::projects::new_id(title, &taken) } else { id.to_string() };
        if !taken.contains(&id) {
            let mut table = Table::new();
            table["id"] = value(id.as_str());
            reserves.push(table);
        }
        let table = reserves.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id.as_str())).ok_or_else(|| format!("no reserve {id}"))?;
        table["title"] = value(title);
        table["balance"] = money_value(edit.balance);
        table["as_of"] = date_value(as_of);
        if edit.floor > 0.0 {
            table["floor"] = money_value(edit.floor);
        } else {
            table.remove("floor");
        }
        if edit.delay_days > 0 {
            table["delay_days"] = value(i64::from(edit.delay_days));
        } else {
            table.remove("delay_days");
        }
        write(path, &doc)?;
        Ok(id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank::Account;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn ledger() -> Ledger {
        toml::from_str(
            "[[budget]]\nid = 'duties'\ntitle = 'Duties'\nperiod = 'month'\nsince = 2026-09-01\n\
             [[budget]]\nid = 'leisure'\ntitle = 'Leisure'\nperiod = 'month'\nsince = 2026-09-01\n\
             [[budget]]\nid = 'work'\ntitle = 'Work'\nperiod = 'month'\nsince = 2026-09-01\n\
             [[preset]]\nbudget = 'duties'\nlabel = 'Loyer SCI Exemple'\namount = -250\nevery = 'month'\nday = 1\n\
             [[preset]]\nbudget = 'duties'\nlabel = 'Food'\namount = -300\nevery = 'month'\nday = 15\nestimate = true\n\
             [[line]]\nbudget = 'leisure'\ndate = 2026-10-01\namount = -23.98\nlabel = 'SHOP'\nlinks = ['mid:x']\n\
             [[reserve]]\nid = 'livret-a'\ntitle = 'Livret A'\nbalance = 300\nas_of = 2026-10-01\n\
             [[reserve]]\nid = 'av'\ntitle = 'Assurance vie'\nbalance = 50000\nas_of = 2026-10-01\ndelay_days = 10\n\
             [[bank_account]]\nid = 'mybank'\ntitle = 'My bank'\nfills = ['duties', 'leisure']\nexports = ['0123456A']\nfloor = 100\ntopped_up_by = ['livret-a', 'av']\n\
             [[bank_account]]\nid = 'paypal'\ntitle = 'PayPal'\nkind = 'paypal'\nfills = ['work', 'leisure']\n\
             [[split]]\naccount = 'mybank'\nwords = ['cinema']\nbudget = 'leisure'\n\
             [[split]]\nwords = ['livret a']\nreserve = 'livret-a'\n\
             [[split]]\naccount = 'paypal'\ndirection = 'debit'\nbudget = 'leisure'\n\
             [[split]]\naccount = 'mybank'\nwords = ['paypal']\nbudget = 'work'\n\
             [[assign]]\naccount = 'mybank'\nmovement = 'm5'\nparts = [{ budget = 'leisure', amount = -20 }]\n",
        )
        .unwrap()
    }

    fn bank() -> Bank {
        let m = |account: &str, id: &str, date: &str, cents: i64, label: &str| Movement { account: account.into(), date: day(date), amount: Money(cents), label: label.into(), id: id.into() };
        Bank {
            root: std::path::PathBuf::new(),
            accounts: vec![Account { id: "0123456A".into(), title: String::new(), balance: Some((day("2026-10-02"), Money(40000))) }],
            movements: vec![
                m("0123456A", "m1", "2026-10-01", -25000, "PRLV SCI EXEMPLE"),
                m("0123456A", "m2", "2026-10-02", -1200, "CB CINEMA LE PALACE"),
                m("0123456A", "m3", "2026-10-03", -2398, "CB SHOP"),
                m("0123456A", "m4", "2026-10-03", -4500, "CB CARREFOUR"),
                m("0123456A", "m5", "2026-10-04", -6000, "CB FNAC"),
                m("0123456A", "m6", "2026-10-05", 50000, "VIR LIVRET A"),
                m("0123456A", "m7", "2026-10-06", -3000, "PAYPAL EUROPE"),
                m("paypal", "p1", "2026-10-06", 3000, "Approvisionnement · Virement bancaire"),
                m("paypal", "p2", "2026-10-06", -3000, "Jean Exemple · Paiement envoyé"),
                m("elsewhere", "x1", "2026-10-06", -999, "NOT DECLARED"),
            ],
        }
    }

    #[test]
    fn each_movement_finds_its_budget() {
        let placed = place(&ledger(), &bank(), crate::words::Words::builtin_ref());
        let of = |id: &str| placed.iter().find(|p| p.movement.id == id).map(|p| (p.place.clone(), p.why)).unwrap();
        // The rent stands for its preset (#0); the cinema by a rule; the SHOP line from mail counts once.
        assert_eq!(of("m1"), (Place::Budgets(vec![Share { budget: "duties".into(), amount: Money(-25000), preset: Some("#0".into()) }]), Why::Preset));
        assert_eq!(of("m2").1, Why::Rule);
        assert_eq!(of("m3"), (Place::Known(0), Why::Mail));
        // A card payment with nothing to say: spent from the food envelope, never on top of it.
        assert_eq!(of("m4"), (Place::Budgets(vec![Share { budget: "duties".into(), amount: Money(-4500), preset: Some("#1".into()) }]), Why::First));
        // In parts by your hand: 20 for leisure, the rest to the first budget (and its envelope).
        let Place::Budgets(parts) = of("m5").0 else { panic!() };
        assert_eq!(parts.iter().map(|s| (s.budget.as_str(), s.amount)).collect::<Vec<_>>(), vec![("leisure", Money(-2000)), ("duties", Money(-4000))]);
        // Money from the Livret A; money moved to PayPal, both ends.
        assert_eq!(of("m6").0, Place::Reserve { reserve: "livret-a".into(), budget: "duties".into() });
        assert_eq!(of("m7"), (Place::Transfer("paypal".into()), Why::Between));
        assert_eq!(of("p1"), (Place::Transfer("mybank".into()), Why::Between));
        // Every payment out of PayPal: leisure, by a rule without words; the bank's "PAYPAL"
        // went to PayPal anyway, both sides read, whatever the rule on that word says.
        assert_eq!(of("p2"), (Place::Budgets(vec![Share { budget: "leisure".into(), amount: Money(-3000), preset: None }]), Why::Rule));
        assert!(!placed.iter().any(|p| p.movement.id == "x1"), "an account not declared feeds only the watch");
    }

    #[test]
    fn budgets_count_the_movements_once() {
        let ledger = ledger();
        let with = ledger.with_bank(&bank(), crate::words::Words::builtin_ref());
        let duties = with.budgets.iter().find(|b| b.id == "duties").unwrap();
        let status = with.status(duties, day("2026-10-10"));
        // Rent 250 (the real one, standing for the preset), food 300 (an envelope: 45 + 40
        // spent from it, the rest to come), the FNAC's 40 and the Livret A's 500 in.
        assert_eq!(status.so_far, Money(-25000) + Money(-4500) + Money(-4000) + Money(50000), "{status:?}");
        assert_eq!(status.to_come, Money(-30000 + 4500 + 4000));
        let leisure = with.budgets.iter().find(|b| b.id == "leisure").unwrap();
        // The cinema, the FNAC's 20, the SHOP line once (from mail), and the payment out of PayPal.
        assert_eq!(with.status(leisure, day("2026-10-10")).so_far, Money(-1200 - 2000 - 2398 - 3000));
        // Nothing declared: the budgets are as the file says.
        let mut bare = ledger.clone();
        bare.bank_accounts.clear();
        assert_eq!(bare.with_bank(&bank(), crate::words::Words::builtin_ref()).lines.len(), bare.lines.len());
    }

    #[test]
    fn written_in_the_file() {
        let path = std::env::temp_dir().join(format!("sioul-accounts-{}.toml", std::process::id()));
        std::fs::write(&path, "# My budgets.\n[[budget]]\nid = 'duties'\ntitle = 'Duties'\nperiod = 'month'\n").unwrap();
        let edit = BankAccountEdit { title: "My bank".into(), kind: "bank".into(), fills: vec!["duties".into()], floor: 100.0, topped_up_by: vec!["livret-a".into()] };
        let id = save_bank_account(&path, "", &edit).unwrap();
        assert_eq!(id, "my-bank");
        add_export_name(&path, &id, "0123456A").unwrap();
        save_split(&path, None, &SplitEdit { account: id.clone(), words: vec!["cinema".into(), " ".into()], direction: "debit".into(), to: "budget:duties".into() }).unwrap();
        set_assignment(&path, &id, "m1", "none").unwrap();
        set_assignment(&path, &id, "m2", "budget:duties").unwrap();
        set_assignment(&path, &id, "m1", "").unwrap();
        save_reserve(&path, "", &ReserveEdit { title: "Livret A".into(), balance: 1200.5, as_of: String::new(), floor: 0.0, delay_days: 0 }, day("2026-10-04")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# My budgets."), "{text}");
        let ledger: Ledger = toml::from_str(&text).unwrap();
        let account = &ledger.bank_accounts[0];
        assert_eq!((account.fills.clone(), account.exports.clone(), account.floor), (vec!["duties".to_string()], vec!["0123456A".to_string()], Money(10000)));
        assert_eq!((ledger.splits[0].words.clone(), ledger.splits[0].direction, ledger.splits[0].budget.as_deref()), (vec!["cinema".to_string()], Some(Direction::Debit), Some("duties")));
        assert_eq!(ledger.assignments.iter().map(|a| a.movement.as_str()).collect::<Vec<_>>(), vec!["m2"]);
        assert_eq!((ledger.reserves[0].id.as_str(), ledger.reserves[0].balance, ledger.reserves[0].as_of), ("livret-a", Money(120050), day("2026-10-04")));
        // Named by its id: another rule put before it (another device's) moves nothing.
        let first = ledger.splits[0].id.clone();
        assert!(!first.is_empty());
        let text = std::fs::read_to_string(&path).unwrap().replace("[[split]]", "[[split]]\nid = \"other\"\naccount = \"\"\nwords = [\"rent\"]\nbudget = \"duties\"\n\n[[split]]");
        std::fs::write(&path, text).unwrap();
        save_split(&path, Some(&SplitShown { id: first.clone(), place: 0, ..SplitShown::default() }), &SplitEdit { account: id.clone(), words: vec!["cinéma".into()], direction: "debit".into(), to: "budget:duties".into() }).unwrap();
        let ledger: Ledger = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(ledger.splits.iter().map(|s| (s.id.as_str(), s.words.join(","))).collect::<Vec<_>>(), vec![("other", "rent".to_string()), (first.as_str(), "cinéma".to_string())]);
        // A rule without an id (an older Sioul's), at its place only while it holds what was shown.
        let gone = SplitShown { place: 0, words: vec!["rent".into()], to: "budget:duties".into(), ..SplitShown::default() };
        assert!(remove_split(&path, &gone).is_err(), "the rule there has an id: not the one shown");
        remove_split(&path, &SplitShown { id: "other".into(), ..SplitShown::default() }).unwrap();
        remove_split(&path, &SplitShown { id: first, ..SplitShown::default() }).unwrap();
        remove_bank_account(&path, &id).unwrap();
        let ledger: Ledger = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(ledger.splits.is_empty() && ledger.bank_accounts.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reserves_top_up_in_order_with_their_delay() {
        let mut ledger = ledger();
        ledger.reserves[0].balance = Money(100000);
        // A tax on 25 October that the account will not hold.
        ledger.lines.push(toml::from_str::<Ledger>("[[line]]\nbudget = 'duties'\ndate = 2026-10-25\namount = -1500\nlabel = 'Tax'\nplanned = true\n").unwrap().lines.remove(0));
        let ups = top_ups(&ledger, &bank(), day("2026-10-08"), crate::words::Words::builtin_ref());
        // The Livret A first, as far as it holds (1,000 less the 500 moved out on the 5th), at once;
        // then the assurance vie for the rest, asked ten days ahead.
        assert_eq!(ups.iter().map(|u| u.reserve.as_str()).collect::<Vec<_>>(), vec!["livret-a", "av"], "{ups:#?}");
        assert_eq!((ups[0].amount, ups[0].ask_by, ups[0].late), (Money(50000), day("2026-10-25"), false));
        assert_eq!((ups[1].ask_by, ups[1].late, ups[1].payment.as_str()), (day("2026-10-15"), false, "Tax"));
        assert_eq!(ups[1].amount.cents() % 1000, 0, "rounded up to ten");
        // Asked on the 20th, the assurance vie arrives after the tax.
        assert!(top_ups(&ledger, &bank(), day("2026-10-20"), crate::words::Words::builtin_ref()).iter().any(|u| u.reserve == "av" && u.late));
    }

    /// The review of 5 October 2026 (money): one account read under two names
    /// (an export first read without its number, kept under its card's id;
    /// later ones with it) has one balance, the newest, never the sum, and a
    /// movement read under both is counted once, in the budgets and the top-ups.
    #[test]
    fn one_account_under_two_names_counts_its_money_once() {
        let ledger = ledger();
        let mut bank = bank();
        bank.accounts.push(Account { id: "mybank".into(), title: String::new(), balance: Some((day("2026-09-30"), Money(39000))) });
        let first = bank.movements[0].clone();
        bank.movements.push(Movement { account: "mybank".into(), ..first });
        let card = &ledger.bank_accounts[0];
        assert_eq!(balance_of(&bank, card), Some((day("2026-10-02"), Money(40000))), "the newest, never the sum");
        assert_eq!(bank.balance_in(&ledger), Some((day("2026-10-02"), Money(40000))), "the money watch's total too");
        assert_eq!(movements_of(&bank, card).iter().filter(|m| m.id == "m1").count(), 1);
        assert_eq!(place(&ledger, &bank, crate::words::Words::builtin_ref()).iter().filter(|p| p.movement.id == "m1").count(), 1, "placed once");
    }
}

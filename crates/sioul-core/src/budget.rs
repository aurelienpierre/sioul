// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Budgets, reserves, and the lines between them.
//!
//! A budget is an envelope with a goal: credits come in and debits go out over
//! a month or a year, and the balance should reach its target by the end of
//! the period. Lines come from mail (invoices, payment notifications, through
//! mail rules), from presets (automatic payments that send no mail: rent,
//! insurance, wages), and by hand. Reserves (savings accounts) cover the
//! budgets assigned to them: at the end of a period a shortfall is drawn from
//! the reserve, and a surplus can be swept back into it.
//!
//! A budget keeps its balance from one period to the next, as an envelope
//! keeps what was not spent: money set aside in October for a bill due in
//! December is still there in December, and no reserve is drawn twice.
//!
//! "On track" does not assume money flows evenly: the balance carried in, what
//! happened so far, and what is known to come before the end of the period
//! (presets and planned lines) are compared with the target (docs/accounting.md).

use crate::projects::Route;
use crate::money::{Amount, Money};
use crate::payments::{self, Payment, PaymentKind};
use crate::porch::{Lane, Triaged};
use crate::trust::Trust;
use jiff::civil::Date;
use jiff::{Span, Timestamp};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};

/// The file's name, at the root of the notes folder.
pub const LEDGER: &str = "sioul-budgets.toml";

/// TOML writes dates bare (`2026-10-01`) and jiff reads them as text: both are accepted.
pub(crate) mod dates {
    use jiff::civil::Date;
    use serde::{Deserialize, Deserializer, de::Error};

    fn parse(value: toml::Value) -> Result<Date, String> {
        let text = match value {
            toml::Value::Datetime(d) => d.to_string(),
            toml::Value::String(s) => s,
            other => return Err(format!("not a date: {other}")),
        };
        text.parse::<Date>().map_err(|e| e.to_string())
    }

    pub fn required<'de, D: Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        parse(toml::Value::deserialize(d)?).map_err(D::Error::custom)
    }

    pub fn optional<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Date>, D::Error> {
        Option::<toml::Value>::deserialize(d)?.map(parse).transpose().map_err(D::Error::custom)
    }

    /// A moment as the files write it (`2026-11-05T10:20:00Z`), a TOML date-time or a string; none when it does not read.
    pub fn moment<'de, D: Deserializer<'de>>(d: D) -> Result<Option<jiff::Timestamp>, D::Error> {
        let value = Option::<toml::Value>::deserialize(d)?;
        Ok(value.and_then(|v| match v {
            toml::Value::Datetime(d) => d.to_string().parse().ok(),
            toml::Value::String(s) => s.parse().ok(),
            _ => None,
        }))
    }

    /// A moment as the files write it: a TOML offset date-time in UTC, to the second.
    pub fn moment_item(at: jiff::Timestamp) -> toml_edit::Item {
        let utc = at.to_zoned(jiff::tz::TimeZone::UTC);
        let datetime = toml_edit::Datetime {
            date: Some(toml_edit::Date { year: utc.year() as u16, month: utc.month() as u8, day: utc.day() as u8 }),
            time: Some(toml_edit::Time { hour: utc.hour() as u8, minute: utc.minute() as u8, second: Some(utc.second() as u8), nanosecond: None }),
            offset: Some(toml_edit::Offset::Z),
        };
        toml_edit::value(datetime)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Month,
    Year,
}

impl Period {
    /// The first and the last day of the period containing `day`.
    pub fn bounds(self, day: Date) -> (Date, Date) {
        match self {
            Period::Month => (day.first_of_month(), day.last_of_month()),
            Period::Year => (day.first_of_year(), day.last_of_year()),
        }
    }

    /// The first day of the next period.
    fn next(self, start: Date) -> Option<Date> {
        let step = match self {
            Period::Month => Span::new().months(1),
            Period::Year => Span::new().years(1),
        };
        start.checked_add(step).ok()
    }
}

/// The period from an income's day in the month holding `day` (or the
/// month before, when that day is still to come) to the day before its day
/// in the next month (`Ledger::bounds`).
fn income_bounds(income: &Preset, day: Date) -> (Date, Date) {
    let month = |d: Date, months: i64| d.first_of_month().checked_add(Span::new().months(months)).ok();
    let start = match income.date_in(day.first_of_month()) {
        Some(start) if start <= day => start,
        _ => month(day, -1).and_then(|m| income.date_in(m)).unwrap_or(day.first_of_month()),
    };
    let end = month(start, 1).and_then(|m| income.date_in(m)).and_then(|next| next.yesterday().ok()).unwrap_or(start.last_of_month());
    (start, end)
}

/// An envelope with a goal.
#[derive(Debug, Clone, Deserialize)]
pub struct Budget {
    pub id: String,
    pub title: String,
    pub period: Period,
    /// The balance to reach by the end of each period; 0 is breaking even.
    #[serde(default)]
    pub target: Money,
    /// The day the budget's balance starts from; nothing before it counts. Without it,
    /// the budget starts with the current period.
    #[serde(default, deserialize_with = "dates::optional")]
    pub since: Option<Date>,
    /// Projects, notes, tasks this budget belongs with.
    #[serde(default)]
    pub links: Vec<String>,
    /// "personal" for leisure money: it stays in view in quiet time.
    #[serde(default)]
    pub area: Option<String>,
    /// Its period starts on this income's day: a monthly money-in preset's
    /// id, the pay or the benefits that fill it; from one income to the next
    /// (docs/accounting.md, "From one income to the next"). Without it, or
    /// once that preset is gone, the calendar month; an older Sioul reads the
    /// calendar month.
    #[serde(default)]
    pub starts_with: Option<String>,
    /// A share of each income set aside for taxes and contributions, in
    /// percent (a work budget's, docs/accounting.md, "Work money"); none or 0: off.
    #[serde(default)]
    pub set_aside: Option<f64>,
    /// How often that share is declared: "month", or "quarter" (the default).
    #[serde(default)]
    pub declare: Option<String>,
}

impl Budget {
    /// What it is for (docs/areas.md): its `area`; "personal", the word for
    /// leisure money before areas had three, is leisure; unsaid, your admin.
    pub fn area_of(&self) -> crate::areas::Area {
        match self.area.as_deref() {
            Some("personal") => crate::areas::Area::LEISURE,
            Some(other) => crate::areas::Area::parse(other).unwrap_or(crate::areas::Area::ADMIN),
            None => crate::areas::Area::ADMIN,
        }
    }
}

/// A payment that comes back: rent, insurance, wages; or an estimate of what is
/// spent without a trace (food, cash). Mail can stand for one of its occurrences:
/// a line linked to it (`preset`) replaces a fixed preset's amount, and is spent
/// from an estimate's envelope, never added on top.
#[derive(Debug, Clone, Deserialize)]
pub struct Preset {
    /// The name lines and mail rules use to stand for it.
    #[serde(default)]
    pub id: Option<String>,
    pub budget: String,
    pub label: String,
    /// Positive for money in, negative for money out.
    pub amount: Money,
    pub every: Period,
    /// The day of the month; 31 means the last day, whatever the month.
    pub day: i8,
    /// For yearly presets, the month (1 to 12).
    #[serde(default)]
    pub month: Option<i8>,
    #[serde(default, deserialize_with = "dates::optional")]
    pub from: Option<Date>,
    #[serde(default, deserialize_with = "dates::optional")]
    pub until: Option<Date>,
    /// An estimate (daily spending), not a fixed payment.
    #[serde(default)]
    pub estimate: bool,
    /// Its amount is "about" that much (an energy bill): matched within a
    /// tenth, and said "about" (`movements::Sum`). An older Sioul reads the
    /// preset without it, as an exact amount.
    #[serde(default)]
    pub about: bool,
    /// The range it falls in, when known ("between €50 and €70"), in
    /// euros, its sign taken from `amount`; an older Sioul reads `amount` alone.
    #[serde(default)]
    pub low: Option<Money>,
    #[serde(default)]
    pub high: Option<Money>,
    #[serde(default)]
    pub links: Vec<String>,
    /// You pay it yourself (money out), or send its invoice (money in): one
    /// reminder a few days before (docs/reminders.md). Without it, it leaves
    /// or comes by itself (a direct debit, a standing order), and is told
    /// only when the account will not hold it. An older Sioul reads it without.
    #[serde(default)]
    pub by_hand: bool,
}

impl Preset {
    /// The dates this preset falls on, between `from` and `to` included.
    pub fn occurrences(&self, from: Date, to: Date) -> Vec<Date> {
        let mut dates = Vec::new();
        let mut period_start = self.every.bounds(from).0;
        while period_start <= to {
            if let Some(date) = self.date_in(period_start).filter(|d| *d >= from && *d <= to && self.active(*d)) {
                dates.push(date);
            }
            let Some(next) = self.every.next(period_start) else { break };
            period_start = next;
        }
        dates
    }

    /// This preset's date in the period starting on `period_start`.
    fn date_in(&self, period_start: Date) -> Option<Date> {
        let month = match self.every {
            Period::Month => period_start.month(),
            Period::Year => self.month.unwrap_or(1),
        };
        let first = Date::new(period_start.year(), month, 1).ok()?;
        Date::new(first.year(), month, self.day.clamp(1, first.days_in_month())).ok()
    }

    fn active(&self, date: Date) -> bool {
        self.from.is_none_or(|f| date >= f) && self.until.is_none_or(|u| date <= u)
    }
}

/// One credit or debit of a budget.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Line {
    /// Its id (`ids`), given when Sioul writes it (`record_line`), by which
    /// your devices merge it and a money movement names it
    /// (`movements::Source`); empty for a line written by hand or by an
    /// older Sioul, until the sharing gives it one.
    #[serde(default)]
    pub id: String,
    pub budget: String,
    #[serde(deserialize_with = "dates::required")]
    pub date: Date,
    /// Positive for money in, negative for money out.
    pub amount: Money,
    pub label: String,
    /// Known to come, not happened yet.
    #[serde(default)]
    pub planned: bool,
    /// A transfer with this reserve: positive moves money from the reserve into the budget.
    #[serde(default)]
    pub reserve: Option<String>,
    /// What it comes from or belongs with: `mid:` for a message, a file, a task.
    #[serde(default)]
    pub links: Vec<String>,
    /// The preset this line stands for in its period (see `Preset`).
    #[serde(default)]
    pub preset: Option<String>,
    /// What the payer paid (money in), or the price (money out), signed as
    /// `amount`, which is what moved the account: `amount = gross − fees`
    /// (docs/accounting.md, "Fees"). None: the same as `amount`.
    #[serde(default)]
    pub gross: Option<Money>,
    /// Its fees, each a cost without sign: `{ kind = "processor", amount = 1.86, about = true }`.
    #[serde(default)]
    pub fees: Vec<LineFee>,
    /// The day it was paid at the processor, which a declaration counts; none: its `date`.
    #[serde(default, deserialize_with = "dates::optional")]
    pub paid_on: Option<Date>,
    /// Your own words about it.
    #[serde(default)]
    pub note: String,
    /// When Sioul wrote it, and when it was last changed (`2026-11-05T10:20:00Z`).
    #[serde(default, deserialize_with = "dates::moment")]
    pub made: Option<jiff::Timestamp>,
    #[serde(default, deserialize_with = "dates::moment")]
    pub changed: Option<jiff::Timestamp>,
}

/// A fee of a line of the budget file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct LineFee {
    /// "processor", "commission", "conversion", "contributions", "bank" (`movements::FeeKind`).
    #[serde(default)]
    pub kind: String,
    pub amount: Money,
    /// An estimate (a rule), not what was kept.
    #[serde(default)]
    pub about: bool,
}

/// A savings account that covers budgets: an assurance vie, a Livret A.
#[derive(Debug, Clone, Deserialize)]
pub struct Reserve {
    pub id: String,
    pub title: String,
    /// The balance on `as_of`; transfers after that date are counted from it.
    pub balance: Money,
    #[serde(deserialize_with = "dates::required")]
    pub as_of: Date,
    /// Never planned below this.
    #[serde(default)]
    pub floor: Money,
    /// How many days money asked from it takes to arrive: 0 for a Livret A,
    /// about ten for an assurance vie.
    #[serde(default)]
    pub delay_days: u32,
    #[serde(default)]
    pub links: Vec<String>,
}

/// Where money actually is: a current account, PayPal, Stripe. Its movements,
/// read from its exports (`bank::Bank`), fill the budgets it pays for
/// (docs/accounting.md, "Bank accounts").
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BankAccount {
    pub id: String,
    pub title: String,
    /// "bank", "paypal", "stripe", "other".
    #[serde(default)]
    pub kind: Option<String>,
    /// The budgets it pays for and is paid into; the first takes what nothing else places.
    #[serde(default)]
    pub fills: Vec<String>,
    /// What its exports call it (an account number, an IBAN), besides its id.
    #[serde(default)]
    pub exports: Vec<String>,
    /// Kept on it at least: below, the reserves top it up, in `topped_up_by` order.
    #[serde(default)]
    pub floor: Money,
    /// The reserves that top it up, the first first: a Livret A (at once), an assurance vie (after its delay).
    #[serde(default)]
    pub topped_up_by: Vec<String>,
    #[serde(default)]
    pub links: Vec<String>,
}

impl BankAccount {
    /// Whether a movement kept under `name` (`bank::Movement::account`) is this account's.
    pub fn owns(&self, name: &str) -> bool {
        self.id == name || self.exports.iter().any(|e| e == name)
    }

    pub fn kind(&self) -> &str {
        self.kind.as_deref().unwrap_or("bank")
    }
}

/// Where a bank account's movement goes, when its label holds one of `words`:
/// a budget (and the recurring payment it stands for), a reserve (money moved
/// with a savings account), or another bank account (money moved between
/// yours, counted in no budget). The first rule that holds wins.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Split {
    /// Its id (`ids`), by which your devices merge it and the window names it;
    /// empty for a rule an older Sioul made, until the sharing gives it one.
    #[serde(default)]
    pub id: String,
    /// The bank account it reads; empty for all.
    #[serde(default)]
    pub account: String,
    /// Any of these in the label, case and accents aside.
    #[serde(default)]
    pub words: Vec<String>,
    /// Money in, money out, or either.
    #[serde(default)]
    pub direction: Option<Direction>,
    #[serde(default)]
    pub budget: Option<String>,
    /// The preset such movements are spent from or stand for: the food envelope.
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub reserve: Option<String>,
    /// Another of your bank accounts: a transfer, in no budget.
    #[serde(default)]
    pub transfer: Option<String>,
}

/// One movement placed by your hand: in a budget, in parts between budgets, or
/// in none (a transfer between your accounts, a refund already counted).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Assignment {
    pub account: String,
    /// The movement's id (`bank::Movement::id`).
    pub movement: String,
    #[serde(default)]
    pub budget: Option<String>,
    /// In parts: each budget its share; what they leave goes to `budget`.
    #[serde(default)]
    pub parts: Vec<Part>,
    /// In no budget.
    #[serde(default)]
    pub none: bool,
}

/// A movement's share in one budget.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Part {
    pub budget: String,
    pub amount: Money,
}

/// Which reserve covers which budget's shortfall, at the end of each period.
#[derive(Debug, Clone, Deserialize)]
pub struct Cover {
    pub reserve: String,
    pub budget: String,
    /// Surpluses go back into the reserve too.
    #[serde(default)]
    pub sweep: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Credit,
    Debit,
}

/// Mail that becomes a line: an invoice, a payment notification.
#[derive(Debug, Clone, Deserialize)]
pub struct MailRule {
    pub budget: String,
    pub direction: Direction,
    /// Which messages: the same conditions as a project's routes (docs/notes-folder.md).
    #[serde(flatten)]
    pub route: Route,
    /// A fixed amount; else the amount the message states.
    #[serde(default)]
    pub amount: Option<Money>,
    #[serde(default)]
    pub label: Option<String>,
    /// Added without asking; otherwise proposed.
    #[serde(default)]
    pub automatic: bool,
    /// The preset such a message stands for: the phone bill for the phone preset.
    #[serde(default)]
    pub preset: Option<String>,
}

/// Everything in `sioul-budgets.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Ledger {
    #[serde(rename = "budget", default)]
    pub budgets: Vec<Budget>,
    #[serde(rename = "preset", default)]
    pub presets: Vec<Preset>,
    #[serde(rename = "line", default)]
    pub lines: Vec<Line>,
    #[serde(rename = "reserve", default)]
    pub reserves: Vec<Reserve>,
    #[serde(rename = "cover", default)]
    pub covers: Vec<Cover>,
    #[serde(rename = "mail_rule", default)]
    pub mail_rules: Vec<MailRule>,
    #[serde(rename = "bank_account", default)]
    pub bank_accounts: Vec<BankAccount>,
    #[serde(rename = "split", default)]
    pub splits: Vec<Split>,
    #[serde(rename = "assign", default)]
    pub assignments: Vec<Assignment>,
}

/// "As planned" within this many cents of the target, the same for `sioul
/// budgets` (`status`) and the window's verdict (`pace`): 5 % of what the
/// period moves (each of its `flows`, in or out, done or planned, and what it
/// still `needed` beyond what is scheduled), never closer than 10 units
/// (docs/accounting.md, "On track, without assuming money flows evenly").
/// Wide enough that a budget whose money comes later in the month is not
/// "short" in its first days.
fn tolerance(flows: &[Flow], needed: Money) -> i64 {
    let moved: i64 = flows.iter().map(|f| f.amount.cents().abs()).sum::<i64>() + needed.cents().abs();
    (moved / 20).max(1000)
}

/// What the period still needs, beyond what is scheduled in it, to reach the
/// target from `opening` (`pace`, `tolerance`).
fn still_needed(budget: &Budget, opening: Money, flows: &[Flow]) -> Money {
    let scheduled: Money = flows.iter().filter(|f| f.scheduled).map(|f| f.amount).sum();
    budget.target - opening - scheduled
}

/// One flow of money in a budget, whatever its source.
#[derive(Debug, Clone, Copy)]
struct Flow {
    amount: Money,
    planned: bool,
    /// Known in advance: a preset's occurrence or a planned line. The rest
    /// (what simply happened) is judged by its pace.
    scheduled: bool,
}

/// How a budget's period is going, against its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Better,
    AsPlanned,
    Short,
}

/// A budget's current period, at a glance.
#[derive(Debug, Clone)]
pub struct BudgetStatus {
    pub id: String,
    pub start: Date,
    pub end: Date,
    /// The balance carried in from the previous periods.
    pub opening: Money,
    /// What happened so far this period (presets dated up to today count as done).
    pub so_far: Money,
    /// What is known to come before the end.
    pub to_come: Money,
    /// The balance expected at the end: opening + so far + to come.
    pub projected: Money,
    /// The part of a surplus already set aside for planned lines after this period
    /// (a bill, a tax due later): not "better than planned", only kept.
    pub earmarked: Money,
    pub target: Money,
    /// `projected - target`: positive is better than planned.
    pub gap: Money,
    pub verdict: Verdict,
    /// At the end of the period: drawn from a reserve (positive), or swept into it (negative).
    pub reserve_transfer: Option<(String, Money)>,
}

/// A reserve, at a glance. Amounts leaving the reserve are positive.
#[derive(Debug, Clone)]
pub struct ReserveStatus {
    pub id: String,
    pub balance_now: Money,
    pub month_done: Money,
    pub month_planned: Money,
    pub year_done: Money,
    pub year_planned: Money,
    pub year_end: Money,
    /// How long it lasts above its floor at the coming year's average draw.
    pub months_left: Option<i64>,
}

impl Ledger {
    /// The period of `budget` holding `day`: its calendar month or year; for
    /// a monthly budget that starts on its income's day (`starts_with`), from
    /// that income's day to the day before its next one (Zhang et al. 2022:
    /// periods follow pay; docs/accounting.md, "From one income to the next").
    pub fn bounds(&self, budget: &Budget, day: Date) -> (Date, Date) {
        match self.income_of(budget) {
            Some(income) => income_bounds(income, day),
            None => budget.period.bounds(day),
        }
    }

    /// The income a budget's period starts with (`Budget::starts_with`): a
    /// monthly preset of money in, fixed; none for a yearly budget.
    pub fn income_of(&self, budget: &Budget) -> Option<&Preset> {
        let id = budget.starts_with.as_deref().filter(|id| !id.trim().is_empty())?;
        if budget.period != Period::Month {
            return None;
        }
        self.presets.iter().find(|p| p.id.as_deref() == Some(id) && p.every == Period::Month && !p.estimate && !p.amount.is_negative())
    }

    /// The period a preset's occurrence on `date` counts in, its lines
    /// standing for it there: its budget's, when that budget starts on its
    /// income's day and the preset comes each month; else its own month or year.
    pub fn preset_bounds(&self, preset: &Preset, date: Date) -> (Date, Date) {
        let income = (preset.every == Period::Month).then(|| self.budgets.iter().find(|b| b.id == preset.budget).and_then(|b| self.income_of(b))).flatten();
        match income {
            Some(income) => income_bounds(income, date),
            None => preset.every.bounds(date),
        }
    }

    /// A preset's dates from `from` to `to`, included: its own days; a
    /// monthly estimate of a budget that starts on its income's day is spent
    /// over that period, and falls on its last day.
    pub fn occurrences_of(&self, preset: &Preset, from: Date, to: Date) -> Vec<Date> {
        let income = (preset.estimate && preset.every == Period::Month).then(|| self.budgets.iter().find(|b| b.id == preset.budget).and_then(|b| self.income_of(b))).flatten();
        let Some(income) = income else { return preset.occurrences(from, to) };
        let mut dates = Vec::new();
        let (_, mut end) = income_bounds(income, from);
        while end <= to {
            if preset.active(end) {
                dates.push(end);
            }
            let Ok(next) = end.tomorrow() else { break };
            end = income_bounds(income, next).1;
        }
        dates
    }

    /// Reads `sioul-budgets.toml` at the root of a notes folder.
    pub fn load(root: &Path) -> Result<Ledger, String> {
        Ledger::load_file(&root.join(LEDGER))
    }

    /// Reads a budget file anywhere.
    pub fn load_file(path: &Path) -> Result<Ledger, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A budget's title by id, else the id.
    pub fn budget_title<'a>(&'a self, id: &'a str) -> &'a str {
        self.budgets.iter().find(|b| b.id == id).map_or(id, |b| b.title.as_str())
    }

    /// A reserve's title by id, else the id.
    pub fn reserve_title<'a>(&'a self, id: &'a str) -> &'a str {
        self.reserves.iter().find(|r| r.id == id).map_or(id, |r| r.title.as_str())
    }

    /// How a budget's current period is going.
    pub fn status(&self, budget: &Budget, today: Date) -> BudgetStatus {
        let (start, end) = self.bounds(budget, today);
        self.status_for(budget, start, end, today, self.opening(budget, start, today))
    }

    /// The balance carried into the period starting on `start`: everything since the
    /// budget's start, as it happened (transfers from reserves included as lines).
    fn opening(&self, budget: &Budget, start: Date, today: Date) -> Money {
        let since = budget.since.unwrap_or(start);
        let before = start.yesterday().unwrap_or(start);
        if since > before {
            return Money::ZERO;
        }
        self.flows(budget, since, before, today).iter().map(|f| f.amount).sum()
    }

    fn status_for(&self, budget: &Budget, start: Date, end: Date, today: Date, opening: Money) -> BudgetStatus {
        let from = budget.since.map_or(start, |s| s.max(start));
        let flows = self.flows(budget, from, end, today);
        let so_far: Money = flows.iter().filter(|f| !f.planned).map(|f| f.amount).sum();
        let to_come: Money = flows.iter().filter(|f| f.planned).map(|f| f.amount).sum();
        let projected = opening + so_far + to_come;
        let surplus = Money((projected - budget.target).cents().max(0));
        let earmarked = Money(surplus.cents().min(self.planned_later(budget, end).cents()));
        let gap = projected - earmarked - budget.target;
        let tolerance = tolerance(&flows, still_needed(budget, opening, &flows));
        let verdict = match gap.cents() {
            g if g > tolerance => Verdict::Better,
            g if g < -tolerance => Verdict::Short,
            _ => Verdict::AsPlanned,
        };
        let reserve_transfer = self.covers.iter().find(|c| c.budget == budget.id).and_then(|c| {
            let draw = gap.is_negative() || (c.sweep && gap.cents() > 0);
            draw.then(|| (c.reserve.clone(), -gap))
        });
        BudgetStatus { id: budget.id.clone(), start, end, opening, so_far, to_come, projected, earmarked, target: budget.target, gap, verdict, reserve_transfer }
    }

    /// What the planned lines of the coming year will take, after `end` (as a positive amount).
    fn planned_later(&self, budget: &Budget, end: Date) -> Money {
        let horizon = end.checked_add(Span::new().years(1)).unwrap_or(end);
        let out: Money = self
            .lines
            .iter()
            .filter(|l| l.budget == budget.id && l.planned && l.date > end && l.date <= horizon)
            .map(|l| l.amount)
            .sum();
        Money((-out.cents()).max(0))
    }

    /// Every flow of a budget between two dates: lines and preset occurrences.
    /// A line that stands for a preset counts through it (see `occurrence`).
    fn flows(&self, budget: &Budget, from: Date, to: Date, today: Date) -> Vec<Flow> {
        let budget = budget.id.as_str();
        let lines = self
            .lines
            .iter()
            .filter(|l| l.budget == budget && l.date >= from && l.date <= to && self.absorbing(l).is_none())
            .map(|l| Flow { amount: l.amount, planned: l.planned, scheduled: l.planned });
        let presets = self
            .presets
            .iter()
            .filter(|p| p.budget == budget)
            .flat_map(|p| self.occurrences_of(p, from, to).into_iter().flat_map(move |d| self.occurrence(p, d, today)));
        lines.chain(presets).collect()
    }

    /// The preset occurrence a line stands for, if its preset falls in the line's period.
    fn absorbing(&self, line: &Line) -> Option<&Preset> {
        let id = line.preset.as_deref()?;
        let preset = self.presets.iter().find(|p| p.id.as_deref() == Some(id) && p.budget == line.budget)?;
        let (start, end) = self.preset_bounds(preset, line.date);
        (!self.occurrences_of(preset, start, end).is_empty()).then_some(preset)
    }

    /// One occurrence of a preset, with the lines standing for it in its period.
    /// Without any, a preset dated up to today is assumed done. With some, a
    /// fixed preset is replaced by them (the real amount); an estimate is an
    /// envelope they are spent from, and what remains of it is still to come.
    fn occurrence(&self, preset: &Preset, date: Date, today: Date) -> Vec<Flow> {
        let (start, end) = self.preset_bounds(preset, date);
        let actual: Option<Money> = preset.id.as_deref().and_then(|id| {
            let linked: Vec<&Line> = self
                .lines
                .iter()
                .filter(|l| l.preset.as_deref() == Some(id) && l.budget == preset.budget && l.date >= start && l.date <= end)
                .collect();
            (!linked.is_empty()).then(|| linked.iter().map(|l| l.amount).sum())
        });
        let Some(actual) = actual else { return vec![Flow { amount: preset.amount, planned: date > today, scheduled: true }] };
        let mut flows = vec![Flow { amount: actual, planned: false, scheduled: true }];
        let remaining = preset.amount - actual;
        let same_way = remaining.cents().signum() == preset.amount.cents().signum() && remaining != Money::ZERO;
        if preset.estimate && same_way {
            flows.push(Flow { amount: remaining, planned: date > today, scheduled: true });
        }
        flows
    }

    /// A reserve's balance and its planned draws.
    pub fn reserve_status(&self, reserve: &Reserve, today: Date) -> ReserveStatus {
        let (month_start, month_end) = Period::Month.bounds(today);
        let (year_start, year_end) = Period::Year.bounds(today);
        let done_since = |from: Date| -> Money {
            self.lines
                .iter()
                .filter(|l| l.reserve.as_deref() == Some(&reserve.id) && !l.planned && l.date >= from && l.date <= today)
                .map(|l| l.amount)
                .sum()
        };
        let after_as_of = reserve.as_of.tomorrow().unwrap_or(reserve.as_of);
        let balance_now = reserve.balance - done_since(after_as_of);
        let in_a_year = today.checked_add(Span::new().years(1)).unwrap_or(year_end);
        let year_planned = self.planned_draws(reserve, today, year_end);
        let draw_12 = self.planned_draws(reserve, today, in_a_year);
        let available = balance_now - reserve.floor;
        // Under its floor already: it lasts no month, never fewer.
    let months_left = (draw_12.cents() > 0).then(|| (available.cents().saturating_mul(12) / draw_12.cents()).max(0));
        ReserveStatus {
            id: reserve.id.clone(),
            balance_now,
            month_done: done_since(month_start),
            month_planned: self.planned_draws(reserve, today, month_end),
            year_done: done_since(year_start),
            year_planned,
            year_end: balance_now - year_planned,
            months_left,
        }
    }

    /// What will leave a reserve after `today`, up to `until`: planned transfers,
    /// and the covers of every budget period ending in that time.
    fn planned_draws(&self, reserve: &Reserve, today: Date, until: Date) -> Money {
        let transfers: Money = self
            .lines
            .iter()
            .filter(|l| l.reserve.as_deref() == Some(&reserve.id) && l.planned && l.date > today && l.date <= until)
            .map(|l| l.amount)
            .sum();
        let covers: Money = self
            .covers
            .iter()
            .filter(|c| c.reserve == reserve.id)
            .filter_map(|c| self.budgets.iter().find(|b| b.id == c.budget))
            .map(|b| self.covers_ending(b, today, until))
            .sum();
        transfers + covers
    }

    /// The reserve transfers of a budget's periods, from the current one to those
    /// ending by `until`; each period opens with the last one's balance after its transfer.
    fn covers_ending(&self, budget: &Budget, today: Date, until: Date) -> Money {
        let mut total = Money::ZERO;
        let mut status = self.status(budget, today);
        while status.end <= until {
            let transfer = status.reserve_transfer.as_ref().map_or(Money::ZERO, |(_, m)| *m);
            total += transfer;
            let Ok(next) = status.end.tomorrow() else { break };
            let (s, e) = self.bounds(budget, next);
            // A sweep takes only the surplus beyond what is set aside (`gap` excludes it).
            status = self.status_for(budget, s, e, today, status.projected + transfer);
        }
        total
    }
}

/// A message about money, and what becomes of it in the budgets.
#[derive(Debug, Clone)]
pub struct MailLine {
    /// The message: `mid:<Message-ID>`, else `file:<path>`. Lines recorded from it carry it in `links`.
    pub key: String,
    pub path: Option<PathBuf>,
    pub date: Date,
    pub payment: Payment,
    pub subject: String,
    pub sender: String,
    pub sender_domain: Option<String>,
    /// The budget a mail rule gives it.
    pub budget: Option<String>,
    /// The preset it stands for, from its rule.
    pub preset: Option<String>,
    /// The label its line takes: the rule's, else who was paid or who paid.
    pub label: String,
    pub state: MailState,
    /// From someone new and not verified: a fake "your payment" is a classic trick.
    pub doubtful: bool,
    /// Its references, for the matching's keys (`matching::references`): an
    /// invoice's, an order's, a mandate's, a creditor's; its attached
    /// invoice's own (`payments::invoice_of`).
    pub refs: Vec<String>,
    /// The day it is due, when its attached invoice says it.
    pub due: Option<Date>,
    /// A purchase paid in instalments: how many (`payments::instalments`).
    pub instalments: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailState {
    /// In the budget file already.
    Recorded,
    /// You said it is not a payment.
    Ignored,
    /// The same payment as another message (its key): a merchant's order
    /// confirmation and the processor's payment, a bill and its settlement, one
    /// message received at two addresses.
    Duplicate(String),
    /// Waits for you: add it, or not.
    Proposed,
    /// The same payment as a movement already counted, which it joins as a
    /// source without a question (`movements::acts::join_mails`): its
    /// address, who it names, its day; probably so when not sure.
    Joined { address: String, label: String, day: Date, probable: bool },
}

impl MailLine {
    /// The amount with its sign: money in is positive.
    pub fn signed(&self) -> Option<Money> {
        let money = self.payment.amount.as_ref()?.money;
        Some(if self.payment.kind.is_credit() { money } else { -money })
    }

    /// The line it becomes in a budget; none without an amount. A
    /// processor's mail that states its fee becomes what arrives, with what
    /// the payer paid and the fee beside it (docs/accounting.md, "Fees").
    pub fn line(&self, budget: &str) -> Option<Line> {
        let signed = self.signed()?;
        let (amount, gross, fees) = match (&self.payment.fee, &self.payment.net) {
            (Some(fee), Some(net)) if !signed.is_negative() => (net.money, Some(signed), vec![LineFee { kind: "processor".into(), amount: fee.money, about: false }]),
            _ => (signed, None, Vec::new()),
        };
        Some(Line {
            id: String::new(),
            budget: budget.to_string(),
            date: self.date,
            amount,
            label: self.label.clone(),
            planned: false,
            reserve: None,
            links: vec![self.key.clone()],
            preset: self.preset.clone(),
            gross,
            fees,
            ..Line::default()
        })
    }
}

/// Every message about money among `items`, newest first, with what becomes of
/// it. Mail set aside (forged, spam) and the review queue's (what your own spam
/// filter flagged or moved) are never read; a rule gives the budget and
/// the preset; `ignored` holds the keys you said are not payments.
pub fn mail_lines(ledger: &Ledger, items: &[Triaged], ignored: &BTreeSet<String>, words: &crate::words::Words) -> Vec<MailLine> {
    let recorded: BTreeSet<&str> = ledger.lines.iter().flat_map(|l| l.links.iter().map(String::as_str)).collect();
    let mut lines: Vec<MailLine> = items.iter().filter(|t| t.lane != Lane::SetAside && t.lane != Lane::Review).filter_map(|t| mail_line(ledger, t, words)).collect();
    lines.sort_by_key(|l| l.date);
    // A message without a Message-ID that an older Sioul keyed by its file
    // ("file:<path>"): the same message while its name's unique part is.
    let unique = |key: &str| key.strip_prefix("file:").and_then(|p| Path::new(p).file_name()).map(|n| crate::maildir::unique_part(&n.to_string_lossy()).to_string());
    let older: BTreeSet<String> = recorded.iter().filter_map(|k| unique(k)).collect();
    let older_ignored: BTreeSet<String> = ignored.iter().filter_map(|k| unique(k)).collect();
    for line in &mut lines {
        let file = line.path.as_ref().and_then(|p| p.file_name()).map(|n| crate::maildir::unique_part(&n.to_string_lossy()).to_string());
        let old = |known: &BTreeSet<String>| !line.key.starts_with("mid:") && file.as_ref().is_some_and(|f| known.contains(f));
        line.state = if recorded.contains(line.key.as_str()) || old(&older) {
            MailState::Recorded
        } else if ignored.contains(&line.key) || old(&older_ignored) {
            MailState::Ignored
        } else {
            MailState::Proposed
        };
    }
    mark_duplicates(&mut lines);
    lines.reverse();
    lines
}

fn mail_line(ledger: &Ledger, t: &Triaged, words: &crate::words::Words) -> Option<MailLine> {
    let card = &t.card;
    let rule = ledger.mail_rules.iter().find(|r| r.route.explain(card).is_some());
    let detected = payments::detect(words, card);
    let mut payment = match (detected, rule) {
        (Some(payment), _) => payment,
        (None, Some(rule)) => payments::read(words, card, kind_of(rule.direction), &payments::start(card)),
        (None, None) => return None,
    };
    if let Some(rule) = rule {
        // The rule knows which way the money goes better than the wording does.
        if payment.kind.is_credit() != (rule.direction == Direction::Credit) {
            payment.kind = kind_of(rule.direction);
        }
        if let Some(fixed) = rule.amount {
            payment.amount = Some(Amount { money: fixed.abs(), currency: "EUR" });
        }
    }
    let date = Timestamp::from_second(card.date?).ok()?.to_zoned(crate::zone::current()).date();
    // Its Message-ID; without one, its digest in its account (`sioul:mail/…`,
    // the same on every device and after a renumbering), else its file.
    let digest = || crate::mailfind::address_of_file(card.account.as_deref()?, card.path.as_deref()?);
    let key = card.message_id.as_ref().map(|id| format!("mid:{id}")).or_else(digest).or_else(|| card.path.as_ref().map(|p| format!("file:{}", p.display())))?;
    // What it says for the matching, and what its attached invoice says of itself.
    let text = format!("{}\n{}", card.subject, payments::start(card));
    let mut refs = crate::matching::references(&words.payments.references, &text);
    let invoice = payments::invoice_of(card);
    if let Some(invoice) = &invoice {
        refs.extend(invoice.references());
        refs.sort();
        refs.dedup();
        // Its amount, when the message gave none, in a currency Sioul names (`money`); else none.
        let currency = match invoice.currency.to_ascii_uppercase().as_str() {
            "EUR" => Some("EUR"),
            "USD" => Some("USD"),
            "GBP" => Some("GBP"),
            "CHF" => Some("CHF"),
            "CAD" => Some("CAD"),
            _ => None,
        };
        if payment.amount.is_none()
            && let (Some(money), Some(currency)) = (invoice.amount, currency)
        {
            payment.amount = Some(Amount { money: money.abs(), currency });
        }
        if payment.party == card.sender() && !invoice.seller.trim().is_empty() {
            payment.party = invoice.seller.trim().to_string();
        }
    }
    Some(MailLine {
        key,
        path: card.path.clone(),
        date,
        refs,
        due: invoice.as_ref().and_then(|i| i.due),
        instalments: payments::instalments(words, &text),
        label: rule.and_then(|r| r.label.clone()).unwrap_or_else(|| payment.party.clone()),
        subject: card.subject.clone(),
        sender: card.sender().to_string(),
        sender_domain: card.sender_domain().map(str::to_ascii_lowercase),
        budget: rule.map(|r| r.budget.clone()),
        preset: rule.and_then(|r| r.preset.clone()),
        doubtful: t.lane == Lane::Screener && t.trust != Trust::Verified,
        payment,
        state: MailState::Proposed,
    })
}

fn kind_of(direction: Direction) -> PaymentKind {
    match direction {
        Direction::Credit => PaymentKind::Received,
        Direction::Debit => PaymentKind::Paid,
    }
}

/// Days within which a merchant's confirmation and the processor's payment are one payment.
const SAME_PAYMENT_DAYS: i32 = 3;
/// Days within which a bill and the message saying it was settled belong together.
const SETTLED_DAYS: i32 = 40;

/// Marks the second message of each pair that tells the same payment: the same
/// message at two addresses; an order and the payment for it (same amount, a
/// few days apart); a bill and its settlement (same sender). What you recorded
/// stays counted, and its partner becomes the duplicate.
fn mark_duplicates(lines: &mut [MailLine]) {
    let n = lines.len();
    for i in 0..n {
        for j in 0..n {
            if i == j || lines[i].state != MailState::Proposed || lines[j].state == MailState::Ignored {
                continue;
            }
            if matches!(&lines[j].state, MailState::Duplicate(_)) {
                continue;
            }
            if same_payment(&lines[i], &lines[j]) {
                let other = lines[j].key.clone();
                lines[i].state = MailState::Duplicate(other);
                break;
            }
        }
    }
}

/// Whether `a` tells the payment `b` tells, `b` being the one that counts.
fn same_payment(a: &MailLine, b: &MailLine) -> bool {
    let days = (b.date - a.date).get_days().abs();
    let amounts = |x: &MailLine, y: &MailLine| match (x.signed(), y.signed()) {
        (Some(x), Some(y)) => x == y,
        _ => true,
    };
    let b_counts = b.state == MailState::Recorded || b.date < a.date || (b.date == a.date && b.key < a.key);
    let same_message = a.key == b.key && b_counts;
    // An order and a payment are matched by their amounts: both must have one.
    let order_paid = a.payment.kind == PaymentKind::Order
        && b.payment.kind == PaymentKind::Paid
        && days <= SAME_PAYMENT_DAYS
        && a.signed().is_some()
        && a.signed() == b.signed();
    let bill_settled = a.payment.kind == PaymentKind::Bill
        && b.payment.kind == PaymentKind::Paid
        && a.sender_domain.is_some()
        && a.sender_domain == b.sender_domain
        && days <= SETTLED_DAYS
        && amounts(a, b);
    // Two donations of the same amount from two people are two payments: the party must match too.
    let recorded_twin = b.state == MailState::Recorded
        && a.payment.kind == b.payment.kind
        && a.payment.party == b.payment.party
        && days <= SAME_PAYMENT_DAYS
        && a.signed().is_some()
        && amounts(a, b);
    same_message || order_paid || bill_settled || recorded_twin
}

/// A budget as its form gives it.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
pub struct BudgetEdit {
    pub title: String,
    /// "month" or "year".
    pub period: String,
    /// The balance to reach by the end of each period, in the ledger's currency.
    #[serde(default)]
    pub target: f64,
    #[serde(default)]
    pub personal: bool,
    /// What it is for: "work", "admin", "leisure", joined by "+" (docs/areas.md); written over `personal` when given.
    #[serde(default)]
    pub area: Option<String>,
    /// The income its period starts with (`Budget::starts_with`): a preset's
    /// id, "" for the calendar month; None leaves the file's as it is.
    #[serde(default)]
    pub starts_with: Option<String>,
    /// The share set aside for taxes, in percent (`Budget::set_aside`); 0
    /// takes it out; None leaves the file's as it is.
    #[serde(default)]
    pub set_aside: Option<f64>,
    /// "month" or "quarter" (`Budget::declare`).
    #[serde(default)]
    pub declare: Option<String>,
    /// Whom it is for (docs/persons.md): a person's id, written as
    /// `sioul:person/<id>` among its `links`, which every version keeps; ""
    /// takes it out; None leaves the file's as it is.
    #[serde(default)]
    pub person: Option<String>,
}

/// The person a budget is for: the first `sioul:person/<id>` among its links (docs/persons.md).
pub fn person_of(budget: &Budget) -> Option<String> {
    budget.links.iter().find(|l| crate::links::kind_of(l) == crate::links::Kind::Person).map(|l| crate::links::id_of(l))
}

fn write_ledger(path: &Path, doc: &DocumentMut) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let temporary = path.with_extension("toml.new");
    std::fs::write(&temporary, doc.to_string()).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)
}

/// Makes a budget (`id` empty) or changes one, in place, its comments and
/// its other fields kept; returns its id.
pub fn save_budget(path: &Path, id: &str, edit: &BudgetEdit) -> Result<String, String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let fail = |e: String| format!("{}: {e}", path.display());
        let title = edit.title.trim();
        if title.is_empty() {
            return Err(fail("a budget needs a name".into()));
        }
        if !edit.target.is_finite() {
            return Err(fail("a target is a number".into()));
        }
        // The first budget makes the file; a file that cannot be read is never written over.
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(fail(e.to_string())),
        };
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let budgets = doc.entry("budget").or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| fail("`budget` is not a list of tables".into()))?;
        let taken: Vec<String> = budgets.iter().filter_map(|t| t.get("id").and_then(Item::as_str).map(str::to_string)).collect();
        let id = if id.is_empty() { crate::projects::new_id(title, &taken) } else { id.to_string() };
        if !budgets.iter().any(|t| t.get("id").and_then(Item::as_str) == Some(id.as_str())) {
            let mut table = Table::new();
            table["id"] = value(id.as_str());
            budgets.push(table);
        }
        let table = budgets.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id.as_str())).ok_or_else(|| fail(format!("no budget {id}")))?;
        table["title"] = value(title);
        table["period"] = value(if edit.period == "year" { "year" } else { "month" });
        table["target"] = value((edit.target * 100.0).round() / 100.0);
        match edit.area.as_deref().map(str::trim) {
            Some(area) if !area.is_empty() => table["area"] = value(crate::areas::Area::parse(area).map_or(area.to_string(), |a| a.id())),
            Some(_) => {
                table.remove("area");
            }
            None if edit.personal => table["area"] = value("personal"),
            None => {
                table.remove("area");
            }
        }
        match edit.starts_with.as_deref().map(str::trim) {
            Some("") => {
                table.remove("starts_with");
            }
            Some(income) => table["starts_with"] = value(income),
            None => {}
        }
        match edit.set_aside {
            Some(share) if share.is_finite() && share > 0.0 => {
                table["set_aside"] = value((share.min(100.0) * 100.0).round() / 100.0);
                table["declare"] = value(if edit.declare.as_deref() == Some("month") { "month" } else { "quarter" });
            }
            Some(_) => {
                table.remove("set_aside");
                table.remove("declare");
            }
            None => {}
        }
        // Whom it is for: one person's address among its links, the others kept as they are.
        if let Some(person) = edit.person.as_deref().map(str::trim) {
            let mut links: Vec<String> = table.get("links").and_then(Item::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
            links.retain(|l| crate::links::kind_of(l) != crate::links::Kind::Person);
            if !person.is_empty() {
                links.push(crate::links::person_uri(person));
            }
            if links.is_empty() {
                table.remove("links");
            } else {
                let mut list = Array::new();
                for link in links {
                    list.push(link);
                }
                table["links"] = value(list);
            }
        }
        write_ledger(path, &doc)?;
        Ok(id)
    })
}

/// Takes a budget out of the file. Its lines stay, written as they were,
/// and no longer count anywhere.
pub fn remove_budget(path: &Path, id: &str) -> Result<(), String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let fail = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let budgets = doc.get_mut("budget").and_then(Item::as_array_of_tables_mut).ok_or_else(|| fail(format!("no budget {id}")))?;
        let before = budgets.len();
        budgets.retain(|t| t.get("id").and_then(Item::as_str) != Some(id));
        if budgets.len() == before {
            return Err(fail(format!("no budget {id}")));
        }
        write_ledger(path, &doc)
    })
}

/// A line of the file as the page showed it, to find it again: lines are named
/// by their place among the lines (`sioul:budget/<budget>/<place>`), and the
/// file may have changed since the page was made (a line added or taken out
/// on another device, or by hand). Never changed by its place alone.
#[derive(Debug, Clone, PartialEq)]
pub struct SeenLine {
    pub budget: String,
    pub date: Date,
    pub amount: Money,
    pub label: String,
}

impl SeenLine {
    /// The line an address names (`sioul:budget/<budget>/<place>`), with what
    /// the page showed of it (JSON `{"day", "value", "label"}`, as the budget
    /// page's rows give them): its place, and the line to find there.
    pub fn from_page(uri: &str, shown: &str) -> Option<(usize, SeenLine)> {
        #[derive(Deserialize)]
        struct Shown {
            day: String,
            value: f64,
            label: String,
        }
        let (budget, place) = uri.strip_prefix("sioul:budget/")?.rsplit_once('/')?;
        let shown: Shown = serde_json::from_str(shown).ok()?;
        if !shown.value.is_finite() {
            return None;
        }
        let line = SeenLine { budget: crate::notes::decode(budget), date: shown.day.parse().ok()?, amount: Money((shown.value * 100.0).round() as i64), label: shown.label };
        Some((place.parse().ok()?, line))
    }

    /// Whether `table`, a `[[line]]` of the file, is this line, read as the ledger reads it.
    fn is(&self, table: &Table) -> bool {
        #[derive(Deserialize)]
        struct One {
            line: Vec<Line>,
        }
        let mut doc = DocumentMut::new();
        let mut one = ArrayOfTables::new();
        one.push(table.clone());
        doc.insert("line", Item::ArrayOfTables(one));
        toml::from_str::<One>(&doc.to_string()).is_ok_and(|one| one.line.first().is_some_and(|l| l.budget == self.budget && l.date == self.date && l.amount == self.amount && l.label == self.label))
    }
}

/// Where the line `seen` is in the file now: at `place` when it is still
/// there, else the one line of the file that is it. None when it is gone,
/// changed, or not told apart from another line just like it.
fn find_line(lines: &ArrayOfTables, place: usize, seen: &SeenLine) -> Option<usize> {
    if lines.get(place).is_some_and(|t| seen.is(t)) {
        return Some(place);
    }
    let mut found = lines.iter().enumerate().filter(|(_, t)| seen.is(t)).map(|(i, _)| i);
    match (found.next(), found.next()) {
        (Some(i), None) => Some(i),
        _ => None,
    }
}

/// Takes one line out of the file: the one at `place` among the lines (as
/// `sioul:budget/<budget>/<place>` names it) when it is still the line
/// `seen`, else the one line that is (`find_line`). Ok(false) when the line
/// is not found as it was: nothing is changed, for the page to say so.
pub fn remove_line(path: &Path, place: usize, seen: &SeenLine) -> Result<bool, String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let fail = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let Some(lines) = doc.get_mut("line").and_then(Item::as_array_of_tables_mut) else { return Ok(false) };
        let Some(place) = find_line(lines, place, seen) else { return Ok(false) };
        lines.remove(place);
        write_ledger(path, &doc)?;
        Ok(true)
    })
}

/// A line of the file changed in place: its label, amount and date; its links,
/// its budget and the comment saying where it came from stay. A date to come
/// makes it planned. The line is found as `remove_line` finds it; Ok(false)
/// when it is not found as it was (`seen`): nothing is changed.
pub fn change_line(path: &Path, place: usize, seen: &SeenLine, label: &str, amount: f64, date: jiff::civil::Date, today: jiff::civil::Date) -> Result<bool, String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let fail = |e: String| format!("{}: {e}", path.display());
        if amount == 0.0 || !amount.is_finite() {
            return Err(fail("an amount is needed".into()));
        }
        let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let Some(lines) = doc.get_mut("line").and_then(Item::as_array_of_tables_mut) else { return Ok(false) };
        let Some(place) = find_line(lines, place, seen) else { return Ok(false) };
        let Some(line) = lines.get_mut(place) else { return Ok(false) };
        line["label"] = value(label.trim());
        line["amount"] = value((amount * 100.0).round() / 100.0);
        line["date"] = value(toml_edit::Datetime { date: Some(toml_edit::Date { year: date.year() as u16, month: date.month() as u8, day: date.day() as u8 }), time: None, offset: None });
        if date > today {
            line["planned"] = value(true);
        } else {
            line.remove("planned");
        }
        write_ledger(path, &doc)?;
        Ok(true)
    })
}

/// Writes a line at the end of the budget file, keeping its comments, with a
/// comment saying where it came from.
pub fn record_line(path: &Path, line: &Line, origin: &str) -> Result<(), String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let fail = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let mut table = Table::new();
        // Its own id, given now (`ids`) unless the line came with one: your
        // devices merge each line by it, so that two lines the same stay two
        // (docs/database.md, "The format of what travels").
        table["id"] = value(if line.id.trim().is_empty() { crate::ids::new() } else { line.id.trim().to_string() });
        table["budget"] = value(line.budget.as_str());
        let date = toml_edit::Date { year: line.date.year() as u16, month: line.date.month() as u8, day: line.date.day() as u8 };
        table["date"] = value(toml_edit::Datetime { date: Some(date), time: None, offset: None });
        table["amount"] = value(line.amount.cents() as f64 / 100.0);
        table["label"] = value(line.label.as_str());
        if let Some(preset) = &line.preset {
            table["preset"] = value(preset.as_str());
        }
        if line.planned {
            table["planned"] = value(true);
        }
        table["links"] = value(line.links.iter().map(String::as_str).collect::<Array>());
        // What the payer paid and the fees, the day it was paid, your note
        // (docs/accounting.md, "Fees"): older versions pass over them.
        if let Some(gross) = line.gross.filter(|g| *g != line.amount) {
            table["gross"] = value(gross.cents() as f64 / 100.0);
        }
        if !line.fees.is_empty() {
            let mut fees = Array::new();
            for fee in &line.fees {
                let mut one = toml_edit::InlineTable::new();
                one.insert("kind", fee.kind.as_str().into());
                one.insert("amount", (fee.amount.cents().abs() as f64 / 100.0).into());
                if fee.about {
                    one.insert("about", true.into());
                }
                fees.push(one);
            }
            table["fees"] = value(fees);
        }
        if let Some(paid) = line.paid_on.filter(|d| *d != line.date) {
            table["paid_on"] = value(toml_edit::Datetime { date: Some(toml_edit::Date { year: paid.year() as u16, month: paid.month() as u8, day: paid.day() as u8 }), time: None, offset: None });
        }
        if !line.note.trim().is_empty() {
            table["note"] = value(line.note.trim());
        }
        table["made"] = dates::moment_item(line.made.unwrap_or_else(jiff::Timestamp::now));
        let origin: String = origin.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        let trailing = doc.trailing().as_str().unwrap_or("").trim_end().to_string();
        let lead = if trailing.trim().is_empty() { String::new() } else { format!("{trailing}\n") };
        table.decor_mut().set_prefix(format!("{lead}\n# {origin}\n"));
        doc.set_trailing("");
        let lines = doc.entry("line").or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
        lines.as_array_of_tables_mut().ok_or_else(|| fail("`line` is not a list of tables".into()))?.push(table);
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
        std::fs::rename(&temporary, path).map_err(|e| fail(e.to_string()))
    })
}

/// How a budget's period is going, at its pace: what is known in advance
/// (presets, planned lines) counts in full; the rest is judged against the
/// part of the period gone. Ten days into thirty, a third of what the period
/// still needs should have come (or no more than a third of what it can
/// spend should be spent): then it is on track.
#[derive(Debug, Clone, PartialEq)]
pub struct Pace {
    pub start: Date,
    pub end: Date,
    /// The part of the period gone, today included: 0 to 1.
    pub gone: f64,
    pub opening: Money,
    /// Presets and planned lines of the whole period, done or to come.
    pub scheduled: Money,
    /// What else happened so far.
    pub unscheduled: Money,
    /// What the rest of the period must bring to reach the target; negative: what it may spend.
    pub needed: Money,
    /// `unscheduled - gone × needed`: positive is ahead of the pace.
    pub ahead: Money,
    pub verdict: Verdict,
    /// The balance expected at the end: at the pace of the past periods when
    /// there are some, else the difference of today carried to the end.
    pub realistic: Money,
    /// Pessimistic and optimistic ends, from the past periods: three at least.
    pub range: Option<(Money, Money)>,
    /// Past periods with movements, to judge from.
    pub history: usize,
    pub target: Money,
}

impl Ledger {
    /// Unscheduled movements of each past period (the newest twelve with any).
    fn unscheduled_history(&self, budget: &Budget, start: Date, today: Date) -> Vec<Money> {
        let mut out = Vec::new();
        let mut period_end = start.yesterday().ok();
        while let Some(end) = period_end {
            if out.len() >= 12 || budget.since.is_some_and(|s| end < s) {
                break;
            }
            let (from, _) = self.bounds(budget, end);
            let flows = self.flows(budget, budget.since.map_or(from, |s| s.max(from)), end, today);
            let lines = self.lines.iter().filter(|l| l.budget == budget.id && !l.planned && l.date >= from && l.date <= end).count();
            if lines > 0 {
                out.push(flows.iter().filter(|f| !f.scheduled).map(|f| f.amount).sum());
            } else if budget.since.is_none() && from < today.checked_sub(Span::new().years(2)).unwrap_or(today) {
                break;
            }
            period_end = from.yesterday().ok();
        }
        out
    }

    /// The current period at its pace (see `Pace`).
    pub fn pace(&self, budget: &Budget, today: Date) -> Pace {
        let (start, end) = self.bounds(budget, today);
        let opening = self.opening(budget, start, today);
        let from = budget.since.map_or(start, |s| s.max(start));
        let flows = self.flows(budget, from, end, today);
        let scheduled: Money = flows.iter().filter(|f| f.scheduled).map(|f| f.amount).sum();
        let unscheduled: Money = flows.iter().filter(|f| !f.scheduled).map(|f| f.amount).sum();
        let days = |a: Date, b: Date| (b - a).get_days() as f64 + 1.0;
        let gone = (days(start, today) / days(start, end)).clamp(0.0, 1.0);
        let needed = still_needed(budget, opening, &flows);
        let expected_now = Money((needed.cents() as f64 * gone).round() as i64);
        let ahead = unscheduled - expected_now;
        // The same tolerance as `status`: one "as planned" everywhere.
        let tolerance = tolerance(&flows, needed);
        let verdict = match ahead.cents() {
            g if g > tolerance => Verdict::Better,
            g if g < -tolerance => Verdict::Short,
            _ => Verdict::AsPlanned,
        };
        let history = self.unscheduled_history(budget, start, today);
        let rest = |per_period: Money| Money((per_period.cents() as f64 * (1.0 - gone)).round() as i64);
        let known = opening + scheduled + unscheduled;
        let (realistic, range) = if history.len() >= 3 {
            let mut sorted: Vec<i64> = history.iter().map(|m| m.cents()).collect();
            sorted.sort_unstable();
            let at = |q: f64| Money(sorted[((sorted.len() - 1) as f64 * q).round() as usize]);
            let (low, middle, high) = if sorted.len() >= 10 { (at(0.1), at(0.5), at(0.9)) } else { (at(0.0), at(0.5), at(1.0)) };
            (known + rest(middle), Some((known + rest(low), known + rest(high))))
        } else {
            // Nothing to judge from: what is ahead or behind today stays so at the end.
            (budget.target + ahead, None)
        };
        Pace { start, end, gone, opening, scheduled, unscheduled, needed, ahead, verdict, realistic, range, history: history.len(), target: budget.target }
    }
}

/// How a budget's current period should end (the budgets page's one
/// sentence about it, docs/accounting.md, "Each budget in words"): its last
/// day, the balance then against its target, if the rest goes as planned, or
/// at the pace of its past periods when there are three at least
/// (`Pace::realistic`); and the reserve that covers a gap or takes a surplus.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodEnd {
    pub end: Date,
    /// Positive: better than planned; negative: short.
    pub gap: Money,
    pub verdict: Verdict,
    /// Drawn from a reserve (positive), or swept into it (negative).
    pub reserve: Option<(String, Money)>,
}

impl Ledger {
    /// How `budget`'s current period should end (`PeriodEnd`): at the pace
    /// of its past periods when there are three, else what is ahead or behind
    /// its pace today carried to the end (`Pace::realistic`), so that a budget
    /// whose money comes later in its period is not said short in its first days.
    pub fn period_end(&self, budget: &Budget, today: Date) -> PeriodEnd {
        self.period_end_of(budget, &self.status(budget, today), &self.pace(budget, today), today)
    }

    /// `period_end`, from its period's status and pace already made.
    pub fn period_end_of(&self, budget: &Budget, status: &BudgetStatus, pace: &Pace, today: Date) -> PeriodEnd {
        let (gap, verdict) = if pace.history >= 3 {
            let gap = pace.realistic - status.earmarked - budget.target;
            let from = budget.since.map_or(status.start, |s| s.max(status.start));
            let flows = self.flows(budget, from, status.end, today);
            let tolerance = tolerance(&flows, still_needed(budget, status.opening, &flows));
            let verdict = match gap.cents() {
                g if g > tolerance => Verdict::Better,
                g if g < -tolerance => Verdict::Short,
                _ => Verdict::AsPlanned,
            };
            (gap, verdict)
        } else {
            (pace.ahead, pace.verdict)
        };
        let reserve = self.covers.iter().find(|c| c.budget == budget.id).and_then(|c| (gap.is_negative() || (c.sweep && gap.cents() > 0)).then(|| (c.reserve.clone(), -gap)));
        PeriodEnd { end: status.end, gap, verdict, reserve }
    }

    /// What is left of an estimate (food, outings) in its current period,
    /// and that period's last day: its amount less what was spent from it
    /// (the lines standing for it), in its sign; past zero, more than
    /// planned. None for a fixed preset, or one not running now.
    pub fn estimate_left(&self, preset: &Preset, today: Date) -> Option<(Money, Date)> {
        if !preset.estimate {
            return None;
        }
        let (start, end) = self.preset_bounds(preset, today);
        if self.occurrences_of(preset, start, end).is_empty() {
            return None;
        }
        let spent: Money = preset.id.as_deref().map_or(Money::ZERO, |id| self.lines.iter().filter(|l| l.preset.as_deref() == Some(id) && l.budget == preset.budget && l.date >= start && l.date <= end).map(|l| l.amount).sum());
        Some((preset.amount - spent, end))
    }
}

/// One movement of a budget's period, as its ledger lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct Movement {
    pub date: Date,
    pub label: String,
    pub amount: Money,
    /// Still to come.
    pub planned: bool,
    /// A recurring movement (a preset's occurrence).
    pub recurring: bool,
    /// Its line in the file, when it is one: `sioul:budget/<budget>/<position>`.
    pub line: Option<usize>,
    /// A work budget's share set aside for its declaration (`movements::Kind::SetAside`), said as such.
    pub set_aside: bool,
}

/// Whether a line was counted in memory (`movements::counted`), never written
/// in the file: a bank account's, a placement's, a share set aside.
fn in_memory(line: &Line) -> bool {
    line.links.iter().any(|k| k.starts_with("bank:") || k.starts_with("hold:") || k.starts_with("move:") || k.starts_with("setaside:"))
}

/// The balance on a day, and whether that day is still to come.
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub date: Date,
    pub balance: Money,
    pub future: bool,
}

impl Ledger {
    /// Every movement of a budget between two dates, oldest first: its lines
    /// and its recurring movements.
    pub fn movements(&self, budget: &Budget, from: Date, to: Date, today: Date) -> Vec<Movement> {
        let mut out: Vec<Movement> = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.budget == budget.id && l.date >= from && l.date <= to)
            // A bank account's movement is no line of the file: changed where it was placed.
            .map(|(i, l)| Movement { date: l.date, label: l.label.clone(), amount: l.amount, planned: l.planned, recurring: false, line: (!in_memory(l)).then_some(i), set_aside: l.links.iter().any(|k| k.starts_with("setaside:")) })
            .collect();
        for preset in self.presets.iter().filter(|p| p.budget == budget.id) {
            for date in self.occurrences_of(preset, from, to) {
                // A line standing for this occurrence is listed itself.
                let stood_for = preset.id.as_deref().is_some_and(|id| {
                    let (start, end) = self.preset_bounds(preset, date);
                    self.lines.iter().any(|l| l.preset.as_deref() == Some(id) && l.budget == budget.id && l.date >= start && l.date <= end)
                });
                if !stood_for {
                    out.push(Movement { date, label: preset.label.clone(), amount: preset.amount, planned: date > today, recurring: true, line: None, set_aside: false });
                }
            }
        }
        out.sort_by(|a, b| a.date.cmp(&b.date).then(a.label.cmp(&b.label)));
        out
    }

    /// The balance at the end of each step ("day", "week", "month", "year")
    /// from `from` to `to`, planned movements included.
    pub fn balance_series(&self, budget: &Budget, from: Date, to: Date, step: &str, today: Date) -> Vec<Point> {
        let since = budget.since.unwrap_or(from);
        let start_balance = if since < from {
            let before = from.yesterday().unwrap_or(from);
            self.flows(budget, since, before, today).iter().map(|f| f.amount).sum()
        } else {
            Money::ZERO
        };
        let moves = self.movements(budget, since.max(from), to, today);
        let next = |d: Date| -> Option<Date> {
            match step {
                "week" => d.checked_add(Span::new().weeks(1)).ok(),
                "month" => d.checked_add(Span::new().months(1)).ok(),
                "year" => d.checked_add(Span::new().years(1)).ok(),
                _ => d.tomorrow().ok(),
            }
        };
        let mut points = Vec::new();
        let mut balance = start_balance;
        let mut taken = 0;
        let mut day = from;
        loop {
            let end = next(day).and_then(|n| n.yesterday().ok()).unwrap_or(to).min(to);
            while taken < moves.len() && moves[taken].date <= end {
                balance += moves[taken].amount;
                taken += 1;
            }
            points.push(Point { date: end, balance, future: end > today });
            match next(day) {
                Some(n) if n <= to => day = n,
                _ => break,
            }
        }
        points
    }
}

/// A recurring movement, as its form gives it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, Deserialize)]
pub struct PresetEdit {
    pub budget: String,
    pub label: String,
    /// Positive in, negative out, in units: -650.0.
    pub amount: f64,
    /// "month" or "year".
    pub every: String,
    pub day: i8,
    #[serde(default)]
    pub month: Option<i8>,
    /// "2026-10-01", or "" for from now on.
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub until: String,
    #[serde(default)]
    pub estimate: bool,
    /// You pay it yourself, or send its invoice (`Preset::by_hand`).
    #[serde(default)]
    pub by_hand: bool,
}

/// Adds a recurring movement to the budget file, its comments kept.
pub fn record_preset(path: &Path, preset: &PresetEdit, origin: &str) -> Result<(), String> {
    // Read, changed and written under the file's lock, which the sharing takes too.
    crate::filelock::with_lock(path, || {
        let fail = |e: String| format!("{}: {e}", path.display());
        if !preset.amount.is_finite() {
            return Err(fail("an amount is needed".into()));
        }
        let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let day_value = |text: &str| -> Option<toml_edit::Datetime> {
            let d: Date = text.parse().ok()?;
            Some(toml_edit::Datetime { date: Some(toml_edit::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None })
        };
        let mut table = Table::new();
        // Its own id (`ids`): lines and rules name it by it, and your devices merge it by it.
        table["id"] = value(crate::ids::new());
        table["budget"] = value(preset.budget.as_str());
        table["label"] = value(preset.label.trim());
        table["amount"] = value((preset.amount * 100.0).round() / 100.0);
        table["every"] = value(if preset.every == "year" { "year" } else { "month" });
        table["day"] = value(i64::from(preset.day.clamp(1, 31)));
        if preset.every == "year" {
            table["month"] = value(i64::from(preset.month.unwrap_or(1).clamp(1, 12)));
        }
        if let Some(from) = day_value(&preset.from) {
            table["from"] = value(from);
        }
        if let Some(until) = day_value(&preset.until) {
            table["until"] = value(until);
        }
        if preset.estimate {
            table["estimate"] = value(true);
        } else if preset.by_hand {
            table["by_hand"] = value(true);
        }
        let origin: String = origin.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        let trailing = doc.trailing().as_str().unwrap_or("").trim_end().to_string();
        let lead = if trailing.trim().is_empty() { String::new() } else { format!("{trailing}\n") };
        table.decor_mut().set_prefix(format!("{lead}\n# {origin}\n"));
        doc.set_trailing("");
        let presets = doc.entry("preset").or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
        presets.as_array_of_tables_mut().ok_or_else(|| fail("`preset` is not a list of tables".into()))?.push(table);
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
        std::fs::rename(&temporary, path).map_err(|e| fail(e.to_string()))
    })
}

/// What Paid found on the line an invoice names (`settle_line`): the day it
/// had, and whether it was planned, which Paid alone then made real.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settled {
    pub date: Option<Date>,
    pub planned: bool,
}

/// The planned line `link` names (an invoice, "sioul:invoice/2026-002") made
/// real: paid on `date`, no longer planned. A line already real (dated in
/// Budgets, matched with the bank) is left as it is. Returns what the line
/// was, for an Undo (`unsettle_paid`); None when no line names it.
pub fn settle_line(path: &Path, link: &str, date: Date) -> Result<Option<Settled>, String> {
    change_linked(path, link, |line| {
        let was = Settled { date: date_of(line), planned: line.get("planned").and_then(Item::as_bool).unwrap_or(false) };
        if was.planned {
            line.remove("planned");
            line["date"] = value(day_of(date));
        }
        was
    })
}

/// Paid's Undo on its line (`settle_line` undone): planned again on the day
/// it had, only when Paid made it real (`was`) and it still holds what Paid
/// wrote (real, dated `wrote`). Whether it was put back: false when Paid
/// changed nothing on it, or something changed it since ("too late").
pub fn unsettle_paid(path: &Path, link: &str, was: Settled, wrote: Date) -> Result<bool, String> {
    if !was.planned {
        return Ok(false);
    }
    change_linked(path, link, |line| {
        let as_paid = !line.get("planned").and_then(Item::as_bool).unwrap_or(false) && date_of(line) == Some(wrote);
        if as_paid {
            if let Some(date) = was.date {
                line["date"] = value(day_of(date));
            }
            line["planned"] = value(true);
        }
        as_paid
    })
    .map(|put| put.unwrap_or(false))
}

/// A line's day, as the ledger writes it.
fn date_of(line: &Table) -> Option<Date> {
    line.get("date").and_then(Item::as_datetime).and_then(|d| d.date).and_then(|d| Date::new(i16::try_from(d.year).ok()?, i8::try_from(d.month).ok()?, i8::try_from(d.day).ok()?).ok())
}

/// The line `link` names planned again, on `date`, where it is in the file:
/// an invoice marked not paid after all (`settle_line` undone). Returns
/// whether there was one.
pub fn unsettle_line(path: &Path, link: &str, date: Date) -> Result<bool, String> {
    change_linked(path, link, |line| {
        line["date"] = value(day_of(date));
        line["planned"] = value(true);
    })
    .map(|changed| changed.is_some())
}

/// A day as the ledger writes it.
fn day_of(date: Date) -> toml_edit::Datetime {
    toml_edit::Datetime { date: Some(toml_edit::Date { year: date.year() as u16, month: date.month() as u8, day: date.day() as u8 }), time: None, offset: None }
}

/// The first line naming `link` changed by `change` in place, read and
/// written under the file's lock, which the sharing takes too: what `change`
/// returned, None when no line names it.
fn change_linked<T>(path: &Path, link: &str, change: impl FnOnce(&mut Table) -> T) -> Result<Option<T>, String> {
    crate::filelock::with_lock(path, || {
        let fail = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let Some(lines) = doc.get_mut("line").and_then(Item::as_array_of_tables_mut) else { return Ok(None) };
        let Some(line) = lines.iter_mut().find(|t| t.get("links").and_then(Item::as_array).is_some_and(|a| a.iter().any(|v| v.as_str() == Some(link)))) else { return Ok(None) };
        let out = change(line);
        // Written only when something changed.
        let changed = doc.to_string();
        if changed != text {
            let temporary = path.with_extension("toml.new");
            std::fs::write(&temporary, changed).map_err(|e| fail(e.to_string()))?;
            std::fs::rename(&temporary, path).map_err(|e| fail(e.to_string()))?;
        }
        Ok(Some(out))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An invoice paid, then not paid after all ("Undo", or Not paid): its
    /// line planned again on the day it had, where it was in the file; the
    /// budget reads as it was. A line no link names: nothing changed.
    #[test]
    fn paid_then_not_paid_leaves_the_budget_as_it_was() {
        let path = std::env::temp_dir().join(format!("sioul-budget-paid-{}.toml", std::process::id()));
        let day = |text: &str| text.parse::<Date>().unwrap();
        std::fs::write(&path, "[[budget]]\nid = \"studio\"\ntitle = \"Studio\"\nperiod = \"month\"\n\n[[line]]\nbudget = \"studio\"\ndate = 2026-11-15\namount = 480.0\nlabel = \"Invoice 2026-002\"\nplanned = true\nlinks = [\"sioul:invoice/2026-002\", \"sioul:project/studio\"]\n\n[[line]]\nbudget = \"studio\"\ndate = 2026-10-02\namount = -12.5\nlabel = \"Paper\"\n").unwrap();
        let lines = || Ledger::load_file(&path).unwrap().lines.iter().map(|l| format!("{:?}", (l.date, l.amount.cents(), &l.label, l.planned, &l.links))).collect::<Vec<_>>();
        let before = lines();
        let (link, today) = ("sioul:invoice/2026-002", day("2026-10-26"));
        let was = settle_line(&path, link, today).unwrap().unwrap();
        assert_eq!(was, Settled { date: Some(day("2026-11-15")), planned: true });
        let paid = Ledger::load_file(&path).unwrap();
        assert!(!paid.lines[0].planned && paid.lines[0].date == today, "made real, dated today");
        // Paid's Undo: planned again on its day, where it was.
        assert!(unsettle_paid(&path, link, was, today).unwrap());
        assert_eq!(lines(), before, "as it was");
        assert!(std::fs::read_to_string(&path).unwrap().find("Invoice 2026-002") < std::fs::read_to_string(&path).unwrap().find("Paper"), "where it was");
        // Not paid, later: planned again on the day given.
        settle_line(&path, link, today).unwrap();
        assert!(unsettle_line(&path, link, day("2026-11-15")).unwrap());
        assert_eq!(lines(), before);
        // A line already real (dated in Budgets): Paid leaves it, and its Undo too.
        std::fs::write(&path, std::fs::read_to_string(&path).unwrap().replace("planned = true\n", "").replace("2026-11-15", "2026-10-20")).unwrap();
        let real = lines();
        let was = settle_line(&path, link, today).unwrap().unwrap();
        assert_eq!(was, Settled { date: Some(day("2026-10-20")), planned: false });
        assert_eq!(lines(), real, "a payment already real stays as it is");
        assert!(!unsettle_paid(&path, link, was, today).unwrap());
        assert_eq!(lines(), real, "never made planned by an Undo");
        // Paid, then the line changed in Budgets before Undo: too late, left as it is.
        std::fs::write(&path, std::fs::read_to_string(&path).unwrap().replace("2026-10-20", "2026-11-15").replace("label = \"Invoice 2026-002\"", "label = \"Invoice 2026-002\"\nplanned = true")).unwrap();
        let was = settle_line(&path, link, today).unwrap().unwrap();
        std::fs::write(&path, std::fs::read_to_string(&path).unwrap().replace("2026-10-26", "2026-10-27")).unwrap();
        let moved = lines();
        assert!(!unsettle_paid(&path, link, was, today).unwrap(), "too late");
        assert_eq!(lines(), moved);
        assert!(!unsettle_line(&path, "sioul:invoice/1999-001", day("2026-11-15")).unwrap());
        assert_eq!(settle_line(&path, "sioul:invoice/1999-001", today).unwrap(), None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn budgets_made_changed_and_taken_out() {
        let path = std::env::temp_dir().join(format!("sioul-budget-edit-{}.toml", std::process::id()));
        std::fs::write(&path, "# mine\n[[budget]]\nid = \"food\"\ntitle = \"Food\"\nperiod = \"month\"\n\n[[line]]\nbudget = \"food\"\ndate = 2026-10-01\namount = -12.5\nlabel = \"Market\"\n").unwrap();
        let id = save_budget(&path, "", &BudgetEdit { title: "Holidays".into(), period: "year".into(), target: 300.0, personal: true, ..BudgetEdit::default() }).unwrap();
        assert_eq!(id, "holidays");
        let ledger = Ledger::load_file(&path).unwrap();
        let holidays = ledger.budgets.iter().find(|b| b.id == "holidays").unwrap();
        assert_eq!((holidays.period, holidays.target.cents(), holidays.area.as_deref()), (Period::Year, 30_000, Some("personal")));
        save_budget(&path, "food", &BudgetEdit { title: "Groceries".into(), period: "month".into(), target: 0.0, personal: false, ..BudgetEdit::default() }).unwrap();
        assert_eq!(Ledger::load_file(&path).unwrap().budget_title("food"), "Groceries");
        // For a person (docs/persons.md): their address among its links, other links kept, one person at a time.
        let text = std::fs::read_to_string(&path).unwrap().replace("title = \"Groceries\"", "title = \"Groceries\"\nlinks = [\"sioul:project/home\"]");
        std::fs::write(&path, text).unwrap();
        let for_whom = |person: Option<&str>| save_budget(&path, "food", &BudgetEdit { title: "Groceries".into(), period: "month".into(), person: person.map(str::to_string), ..BudgetEdit::default() }).unwrap();
        let links = || Ledger::load_file(&path).unwrap().budgets.iter().find(|b| b.id == "food").unwrap().links.clone();
        for_whom(Some("p-lea"));
        for_whom(Some("p-tom"));
        assert_eq!(links(), ["sioul:project/home", "sioul:person/p-tom"]);
        assert_eq!(person_of(Ledger::load_file(&path).unwrap().budgets.iter().find(|b| b.id == "food").unwrap()).as_deref(), Some("p-tom"));
        for_whom(None);
        assert_eq!(links().len(), 2, "unsaid leaves it as it is");
        for_whom(Some(""));
        assert_eq!(links(), ["sioul:project/home"]);
        let market = SeenLine { budget: "food".into(), date: date("2026-10-01"), amount: Money(-1250), label: "Market".into() };
        assert!(remove_line(&path, 0, &market).unwrap());
        remove_budget(&path, "holidays").unwrap();
        let ledger = Ledger::load_file(&path).unwrap();
        assert!(ledger.lines.is_empty() && ledger.budgets.len() == 1);
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("# mine"));
        std::fs::remove_file(&path).unwrap();
    }

    /// Lines are named by their place in the file, which may change after the
    /// page was made: a line added before it, one taken out, an edit by hand.
    /// The line changed or taken out is the one the page showed, found again
    /// by its content, or none, and nothing changes.
    #[test]
    fn a_line_is_changed_only_when_found_as_it_was() {
        let path = std::env::temp_dir().join(format!("sioul-budget-lines-{}.toml", std::process::id()));
        let line = |date: &str, amount: &str, label: &str| format!("\n[[line]]\nbudget = \"food\"\ndate = {date}\namount = {amount}\nlabel = \"{label}\"\n");
        let head = "# mine\n[[budget]]\nid = \"food\"\ntitle = \"Food\"\nperiod = \"month\"\n";
        let seen = |date: &str, cents: i64, label: &str| SeenLine { budget: "food".into(), date: date.parse().unwrap(), amount: Money(cents), label: label.into() };
        let today = date("2026-10-09");
        let labels = || Ledger::load_file(&path).unwrap().lines.iter().map(|l| (l.label.clone(), l.amount.cents())).collect::<Vec<_>>();
        // The page showed "Bakery" at place 1; meanwhile a line came before it.
        std::fs::write(&path, format!("{head}{}{}{}", line("2026-10-01", "-3", "Demo early"), line("2026-10-02", "-12.5", "Market"), line("2026-10-03", "-4.2", "Bakery"))).unwrap();
        let bakery = seen("2026-10-03", -420, "Bakery");
        assert!(change_line(&path, 1, &bakery, "Bakery, bread", -4.4, date("2026-10-03"), today).unwrap());
        assert_eq!(labels(), [("Demo early".to_string(), -300), ("Market".into(), -1250), ("Bakery, bread".into(), -440)], "Market, now at place 1, is left as it was");
        // Taken out the same way: the line shown, not the one now at its place.
        let market = seen("2026-10-02", -1250, "Market");
        assert!(remove_line(&path, 0, &market).unwrap());
        assert_eq!(labels(), [("Demo early".to_string(), -300), ("Bakery, bread".into(), -440)]);
        // Changed meanwhile (by hand, or on another device): not found as it was, nothing changes.
        let before = std::fs::read_to_string(&path).unwrap();
        assert!(!change_line(&path, 1, &bakery, "Anything", -1.0, date("2026-10-03"), today).unwrap());
        assert!(!remove_line(&path, 1, &bakery).unwrap());
        assert!(!remove_line(&path, 5, &market).unwrap(), "gone");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before, "nothing written");
        // Two lines just alike, neither at its place: not told apart, nothing changes.
        std::fs::write(&path, format!("{head}{}{}{}", line("2026-10-01", "-3", "Demo early"), line("2026-10-05", "-2", "Coffee"), line("2026-10-05", "-2", "Coffee"))).unwrap();
        let coffee = seen("2026-10-05", -200, "Coffee");
        assert!(!remove_line(&path, 0, &coffee).unwrap());
        // At its place, one of the two is as good as the other.
        assert!(remove_line(&path, 2, &coffee).unwrap());
        assert_eq!(labels().len(), 2);
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("# mine"));
        std::fs::remove_file(&path).unwrap();
        // As the page names it: its address and what its row showed.
        let uri = crate::links::budget_uri("food & drink", 3);
        assert_eq!(SeenLine::from_page(&uri, r#"{"day": "2026-10-05", "value": -2.1, "label": "Coffee"}"#), Some((3, SeenLine { budget: "food & drink".into(), date: date("2026-10-05"), amount: Money(-210), label: "Coffee".into() })));
        assert_eq!(SeenLine::from_page("sioul:budget/food", r#"{"day": "2026-10-05", "value": -2, "label": "Coffee"}"#), None, "a budget, not a line");
        assert_eq!(SeenLine::from_page(&uri, "{}"), None, "nothing shown: never by its place alone");
    }

    #[test]
    fn an_unreadable_file_is_never_written_over() {
        let path = std::env::temp_dir().join(format!("sioul-budget-unreadable-{}.toml", std::process::id()));
        // Latin-1, as another editor may save it: not UTF-8.
        let latin1: &[u8] = b"# Mes budgets\n[[budget]]\nid = \"d\"\ntitle = \"D\xe9penses\"\nperiod = \"month\"\n";
        std::fs::write(&path, latin1).unwrap();
        assert!(save_budget(&path, "", &BudgetEdit { title: "Holidays".into(), period: "year".into(), ..BudgetEdit::default() }).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), latin1, "left as it was");
        assert!(save_budget(&path, "", &BudgetEdit { title: "Holidays".into(), period: "year".into(), target: f64::INFINITY, ..BudgetEdit::default() }).is_err());
        std::fs::remove_file(&path).unwrap();
        // No file yet: the first budget makes it.
        assert_eq!(save_budget(&path, "", &BudgetEdit { title: "Holidays".into(), period: "year".into(), target: 300.004, ..BudgetEdit::default() }).unwrap(), "holidays");
        assert_eq!(Ledger::load_file(&path).unwrap().budgets[0].target, Money(30000));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn on_track_at_its_pace() {
        // A budget that breaks even; its subscriptions are taken on the 1st; its income comes when it comes.
        let text = "[[budget]]\nid = \"work\"\ntitle = \"Work\"\nperiod = \"month\"\n\n[[preset]]\nbudget = \"work\"\nlabel = \"Subscriptions\"\namount = -110\nevery = \"month\"\nday = 1\n";
        let ledger: Ledger = toml::from_str(text).unwrap();
        let budget = &ledger.budgets[0];
        let third = date("2026-10-03");
        let pace = ledger.pace(budget, third);
        assert_eq!((pace.scheduled, pace.unscheduled, pace.needed), (Money(-11_000), Money::ZERO, Money(11_000)));
        assert_eq!(pace.verdict, Verdict::AsPlanned, "on the 3rd, nothing has come yet and that is on track: {pace:?}");
        assert_eq!(ledger.status(budget, third).verdict, Verdict::Short, "the plain projection said short");
        assert!(pace.range.is_none());
        // Halfway with nothing: behind its pace, said once.
        assert_eq!(ledger.pace(budget, date("2026-10-16")).verdict, Verdict::Short);
        // With three months of income behind it: a realistic end, and a range.
        let lines = "\n[[line]]\nbudget = \"work\"\ndate = 2026-07-10\namount = 150\nlabel = \"Client A\"\n\n[[line]]\nbudget = \"work\"\ndate = 2026-08-12\namount = 90\nlabel = \"Client B\"\n\n[[line]]\nbudget = \"work\"\ndate = 2026-09-20\namount = 120\nlabel = \"Client A\"\n\n[[line]]\nbudget = \"work\"\ndate = 2026-10-02\namount = 30\nlabel = \"Client C\"\n";
        let ledger: Ledger = toml::from_str(&format!("{text}{lines}")).unwrap();
        let pace = ledger.pace(&ledger.budgets[0], third);
        assert_eq!((pace.history, pace.unscheduled), (3, Money(3_000)));
        // Known so far (-110 + 30), then the median month (120) for the 28 days left.
        let left: f64 = 1.0 - 3.0 / 31.0;
        assert_eq!(pace.realistic, Money(((-80.0 + 120.0 * left) * 100.0).round() as i64));
        let (low, high) = pace.range.unwrap();
        assert!(low < pace.realistic && pace.realistic < high, "{low:?} {:?} {high:?}", pace.realistic);
        assert_eq!(pace.verdict, Verdict::Better, "30 in on the 3rd is ahead of the pace");
        // The ledger of the month, and its balance day by day.
        let moves = ledger.movements(&ledger.budgets[0], date("2026-10-01"), date("2026-10-31"), third);
        assert_eq!(moves.iter().map(|m| (m.label.as_str(), m.recurring)).collect::<Vec<_>>(), vec![("Subscriptions", true), ("Client C", false)]);
        let days = ledger.balance_series(&ledger.budgets[0], date("2026-10-01"), date("2026-10-31"), "day", third);
        assert_eq!(days.len(), 31);
        let opening = ledger.pace(&ledger.budgets[0], third).opening;
        assert_eq!((days[0].balance, days[1].balance), (opening + Money(-11_000), opening + Money(-8_000)));
        assert!(!days[2].future && days[3].future);
        assert_eq!(ledger.balance_series(&ledger.budgets[0], date("2026-01-01"), date("2026-12-31"), "month", third).len(), 12);
    }

    /// One tolerance says "as planned" in `sioul budgets` (`status`) and in the
    /// window (`pace`): 5 % of what the period moves, never closer than 10
    /// (docs/accounting.md). `sioul budgets` used 2 % of what goes out, and
    /// called "better" what the window called "as planned".
    #[test]
    fn one_tolerance_says_as_planned_everywhere() {
        let rent = "[[budget]]\nid = \"home\"\ntitle = \"Home\"\nperiod = \"month\"\n\n[[preset]]\nbudget = \"home\"\nlabel = \"Rent\"\namount = -1000\nevery = \"month\"\nday = 1\n";
        let with_income = |amount: &str| -> Ledger { toml::from_str(&format!("{rent}\n[[line]]\nbudget = \"home\"\ndate = 2026-10-10\namount = {amount}\nlabel = \"Demo income\"\n")).unwrap() };
        let end = date("2026-10-31");
        let both = |ledger: &Ledger| (ledger.pace(&ledger.budgets[0], end).verdict, ledger.status(&ledger.budgets[0], end).verdict);
        // 1,000 out (the rent), 1,200 in, 1,000 still needed beyond the rent: 3,200 moved, 160 tolerated. 200 above: better, in both.
        let ahead = with_income("1200");
        let budget = &ahead.budgets[0];
        let flows = ahead.flows(budget, date("2026-10-01"), end, end);
        assert_eq!(tolerance(&flows, still_needed(budget, Money::ZERO, &flows)), 16_000);
        assert_eq!(both(&ahead), (Verdict::Better, Verdict::Better));
        // 100 above, within 155: as planned, in both (2 % of what goes out, 20, said "better").
        assert_eq!(both(&with_income("1100")), (Verdict::AsPlanned, Verdict::AsPlanned));
        // 200 below, beyond 140: short, in both.
        assert_eq!(both(&with_income("800")), (Verdict::Short, Verdict::Short));
        // Little moving: never closer than 10.
        let small: Ledger = toml::from_str("[[budget]]\nid = \"b\"\ntitle = \"B\"\nperiod = \"month\"\n\n[[line]]\nbudget = \"b\"\ndate = 2026-10-02\namount = -50\nlabel = \"Demo\"\n").unwrap();
        let flows = small.flows(&small.budgets[0], date("2026-10-01"), end, end);
        assert_eq!(tolerance(&flows, still_needed(&small.budgets[0], Money::ZERO, &flows)), 1_000);
    }

    #[test]
    fn recurring_movements_by_hand() {
        let dir = std::env::temp_dir().join(format!("sioul-presets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(LEDGER);
        std::fs::write(&path, "# Budgets\n[[budget]]\nid = \"home\"\ntitle = \"Home\"\nperiod = \"month\"\n").unwrap();
        record_preset(&path, &PresetEdit { budget: "home".into(), label: "Rent".into(), amount: -650.0, every: "month".into(), day: 5, from: "2026-11-01".into(), ..PresetEdit::default() }, "Added in Sioul").unwrap();
        let ledger = Ledger::load_file(&path).unwrap();
        let rent = &ledger.presets[0];
        assert_eq!((rent.amount, rent.day, rent.from), (Money(-65_000), 5, Some(date("2026-11-01"))));
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("# Budgets"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn date(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn presets_fall_on_real_days() {
        let rent: Preset = toml::from_str("budget='b'\nlabel='rent'\namount=-500\nevery='month'\nday=31").unwrap();
        let days = rent.occurrences(date("2027-01-01"), date("2027-04-30"));
        let days: Vec<String> = days.iter().map(ToString::to_string).collect();
        assert_eq!(days, vec!["2027-01-31", "2027-02-28", "2027-03-31", "2027-04-30"]);
        let water: Preset = toml::from_str("budget='b'\nlabel='water'\namount=-500\nevery='year'\nmonth=12\nday=15\nfrom=2026-11-01").unwrap();
        let days: Vec<String> = water.occurrences(date("2026-01-01"), date("2027-12-31")).iter().map(ToString::to_string).collect();
        assert_eq!(days, vec!["2026-12-15", "2027-12-15"]);
    }

    const LEDGER_TEXT: &str = r#"
        [[budget]]
        id = "home"
        title = "Home"
        period = "month"
        target = 0

        [[budget]]
        id = "life"
        title = "Life"
        period = "month"

        [[preset]]
        budget = "home"
        label = "Wages"
        amount = 1500
        every = "month"
        day = 25

        [[preset]]
        budget = "home"
        label = "Rent"
        amount = -600
        every = "month"
        day = 1

        [[line]]
        budget = "home"
        date = 2026-10-10
        amount = -200
        label = "Groceries"

        [[preset]]
        budget = "life"
        label = "Rent"
        amount = -500
        every = "month"
        day = 1

        [[preset]]
        budget = "life"
        label = "Daily spending"
        amount = -300
        every = "month"
        day = 15
        estimate = true

        [[reserve]]
        id = "savings"
        title = "Savings"
        balance = 10000
        as_of = 2026-10-01

        [[cover]]
        reserve = "savings"
        budget = "life"
    "#;

    #[test]
    fn a_budget_on_its_way() {
        let ledger: Ledger = toml::from_str(LEDGER_TEXT).unwrap();
        let today = date("2026-10-15");
        let home = ledger.status(&ledger.budgets[0], today);
        assert_eq!((home.so_far, home.to_come, home.projected), (Money(-80000), Money(150000), Money(70000)));
        assert_eq!(home.verdict, Verdict::Better);
        assert!(home.reserve_transfer.is_none());
        let life = ledger.status(&ledger.budgets[1], today);
        assert_eq!(life.verdict, Verdict::Short);
        assert_eq!(life.reserve_transfer, Some(("savings".to_string(), Money(80000))));
    }

    #[test]
    fn a_reserve_that_covers() {
        let ledger: Ledger = toml::from_str(LEDGER_TEXT).unwrap();
        let status = ledger.reserve_status(&ledger.reserves[0], date("2026-10-15"));
        assert_eq!(status.balance_now, Money(1_000_000));
        assert_eq!(status.month_planned, Money(80000));
        assert_eq!(status.year_planned, Money(240000));
        assert_eq!(status.year_end, Money(760000));
        assert_eq!(status.months_left, Some(12));
        // Under its floor already: no month left, never fewer.
        let mut low = ledger.reserves[0].clone();
        low.floor = Money(2_000_000);
        assert_eq!(ledger.reserve_status(&low, date("2026-10-15")).months_left, Some(0));
    }

    #[test]
    fn money_set_aside_is_kept() {
        let ledger: Ledger = toml::from_str(
            r#"
            [[budget]]
            id = "home"
            title = "Home"
            period = "month"
            since = 2026-10-01
            [[line]]
            budget = "home"
            date = 2026-10-20
            amount = 1000
            label = "Withdrawal"
            reserve = "savings"
            planned = true
            [[preset]]
            budget = "home"
            label = "Rent"
            amount = -600
            every = "month"
            day = 1
            from = 2026-11-01
            [[reserve]]
            id = "savings"
            title = "Savings"
            balance = 5000
            as_of = 2026-10-01
            [[cover]]
            reserve = "savings"
            budget = "home"
            "#,
        )
        .unwrap();
        let today = date("2026-10-15");
        // October keeps the €1,000; November spends €600 of it; December needs €200 more.
        let reserve = ledger.reserve_status(&ledger.reserves[0], today);
        assert_eq!(reserve.year_planned, Money(120000));
        assert_eq!(reserve.year_end, Money(380000));
        let november = ledger.status(&ledger.budgets[0], date("2026-11-15"));
        assert_eq!(november.opening, Money(100000));
    }

    fn triaged(from: &str, subject: &str, date: &str, id: &str, body: &str) -> Triaged {
        use crate::porch::{self, Context, KnownSenders};
        let raw = format!("From: {from}\r\nSubject: {subject}\r\nDate: {date}\r\nMessage-ID: <{id}>\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{body}\r\n");
        let known = KnownSenders::default();
        porch::triage(crate::card::Card::from_bytes(raw.as_bytes()).unwrap(), &Context { projects: None, known: &known, senders: &crate::porch::Senders::default(), trusted_ids: &[], now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, words: None, own_addresses: &[], spam: None })
    }

    const PAY: &str = "Pay Exemple <service@pay.example>";

    #[test]
    fn mail_becomes_a_line() {
        let ledger: Ledger = toml::from_str("[[mail_rule]]\nbudget = 'projects'\ndirection = 'credit'\nfrom_domains = ['pay.example']\nsubject_contains = ['paiement reçu']").unwrap();
        let mail = [triaged(PAY, "Notification de paiement reçu", "Fri, 02 Oct 2026 10:00:00 +0200", "p1@pay.example", "Vous avez reçu un paiement de 25,00 € EUR de Jean Exemple (jean@example.org).")];
        let lines = mail_lines(&ledger, &mail, &BTreeSet::new(), crate::words::Words::builtin_ref());
        assert_eq!(lines.len(), 1);
        let line = lines[0].line("projects").unwrap();
        assert_eq!((line.amount, line.label.as_str()), (Money(2500), "Jean Exemple"));
        assert_eq!(line.links, vec!["mid:p1@pay.example".to_string()]);
        assert_eq!((lines[0].budget.as_deref(), &lines[0].state), (Some("projects"), &MailState::Proposed));
        // Once in the file, it is counted, not proposed again.
        let recorded = Ledger { lines: vec![line], ..ledger };
        assert_eq!(mail_lines(&recorded, &mail, &BTreeSet::new(), crate::words::Words::builtin_ref())[0].state, MailState::Recorded);
        assert_eq!(mail_lines(&Ledger::default(), &mail, &BTreeSet::from(["mid:p1@pay.example".to_string()]), crate::words::Words::builtin_ref())[0].state, MailState::Ignored);
    }

    /// A message without a Message-ID is keyed by its digest in its account
    /// (`sioul:mail/…`, the same on every device and in any of its files);
    /// a row an older Sioul recorded or set aside by its file ("file:…")
    /// still counts while the file's unique part is the same.
    #[test]
    fn mail_without_an_id_is_keyed_by_its_address() {
        let ledger: Ledger = toml::from_str("[[mail_rule]]\nbudget = 'projects'\ndirection = 'credit'\nfrom_domains = ['pay.example']\nsubject_contains = ['paiement reçu']").unwrap();
        let dir = std::env::temp_dir().join(format!("sioul-budget-no-id-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("cur")).unwrap();
        let raw = format!("From: {PAY}\r\nSubject: Notification de paiement reçu\r\nDate: Fri, 02 Oct 2026 10:00:00 +0200\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\nVous avez reçu un paiement de 25,00 € EUR de Jean Exemple (jean@example.org).\r\n");
        let path = dir.join("cur").join(format!("1777000000.U7-1.sioul{}2,S", crate::maildir::INFO));
        std::fs::write(&path, &raw).unwrap();
        let mut t = triaged(PAY, "x", "Fri, 02 Oct 2026 10:00:00 +0200", "x@example.org", "x");
        t.card = crate::card::Card::from_bytes(raw.as_bytes()).unwrap();
        t.card.path = Some(path.clone());
        t.card.account = Some("home".into());
        let mail = [t];
        let lines = mail_lines(&ledger, &mail, &BTreeSet::new(), crate::words::Words::builtin_ref());
        assert_eq!(lines[0].key, crate::links::digest_uri("home", &crate::mailcache::digest(raw.as_bytes())));
        // Recorded, or set aside, by an older Sioul under its file (its flags changed since): the same row.
        let older = format!("file:{}", path.with_file_name("1777000000.U7-1.sioul:2,").display());
        let mut line = lines[0].line("projects").unwrap();
        line.links = vec![older.clone()];
        assert_eq!(mail_lines(&Ledger { lines: vec![line], ..ledger.clone() }, &mail, &BTreeSet::new(), crate::words::Words::builtin_ref())[0].state, MailState::Recorded);
        assert_eq!(mail_lines(&ledger, &mail, &BTreeSet::from([older]), crate::words::Words::builtin_ref())[0].state, MailState::Ignored);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_payment_told_twice_counts_once() {
        let mail = [
            triaged(PAY, "SHOP: 23,98 € EUR", "Wed, 30 Sep 2026 22:36:00 +0200", "pay@pay.example", "Vous avez autorisé un paiement de 23,98 € EUR en faveur de SHOP"),
            triaged("Shop <noreply@shop.example>", "Votre commande est validée !", "Wed, 30 Sep 2026 23:35:00 +0200", "order@shop.example", "Article 19,99 €\r\nLivraison 3,99 €\r\nTotal 23,98 €"),
            triaged("Telecom <factures@telecom.example>", "Votre facture mobile est disponible", "Sat, 26 Sep 2026 00:08:00 +0200", "bill@telecom.example", "Rendez-vous dans votre espace."),
            triaged("Telecom <factures@telecom.example>", "Le règlement de votre facture a bien été effectué", "Fri, 02 Oct 2026 17:30:00 +0200", "settled@telecom.example", "Merci."),
        ];
        // The processor's authorisation is its own wording, added as its users add it.
        let lines = mail_lines(&Ledger::default(), &mail, &BTreeSet::new(), &crate::words::with_processor());
        let state = |key: &str| lines.iter().find(|l| l.key == key).unwrap().state.clone();
        assert_eq!(state("mid:pay@pay.example"), MailState::Proposed);
        assert_eq!(state("mid:order@shop.example"), MailState::Duplicate("mid:pay@pay.example".into()));
        assert_eq!(state("mid:bill@telecom.example"), MailState::Duplicate("mid:settled@telecom.example".into()));
        assert_eq!(state("mid:settled@telecom.example"), MailState::Proposed);
        // Newest first.
        assert_eq!(lines[0].key, "mid:settled@telecom.example");
    }

    #[test]
    fn recorded_lines_keep_the_file() {
        let dir = std::env::temp_dir().join(format!("sioul-ledger-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(LEDGER);
        // As in a real file: lines in the middle, reserves after them, notes at the end.
        let text = "# My budgets.\n[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n\n\
            [[line]]\nbudget = 'home'\ndate = 2026-10-20\namount = -100\nlabel = 'Planned'\nplanned = true\n\n\
            [[reserve]]\nid = 'savings'\ntitle = 'Savings'\nbalance = 1000\nas_of = 2026-10-01\n\n# Notes at the end.\n";
        std::fs::write(&path, text).unwrap();
        let line = Line {
            id: String::new(),
            budget: "home".into(),
            date: date("2026-10-02"),
            amount: Money(-2999),
            label: "Telecom".into(),
            planned: false,
            reserve: None,
            links: vec!["mid:settled@telecom.example".into()],
            preset: Some("phone".into()),
            ..Line::default()
        };
        record_line(&path, &line, "From mail: “Le règlement de votre facture” from Telecom.").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# My budgets.\n"), "{text}");
        assert!(text.find("# Notes at the end.").unwrap() < text.find("# From mail").unwrap(), "{text}");
        let ledger = Ledger::load_file(&path).unwrap();
        assert_eq!((ledger.lines.len(), ledger.reserves.len()), (2, 1));
        let back = &ledger.lines[1];
        assert_eq!((back.amount, back.date, back.preset.as_deref()), (Money(-2999), date("2026-10-02"), Some("phone")));
        assert_eq!(back.links, vec!["mid:settled@telecom.example".to_string()]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_line_stands_for_its_preset() {
        let text = "[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n\
            [[preset]]\nid = 'phone'\nbudget = 'home'\nlabel = 'Phone'\namount = -20\nevery = 'month'\nday = 10\n\
            [[preset]]\nid = 'food'\nbudget = 'home'\nlabel = 'Food'\namount = -300\nevery = 'month'\nday = 28\nestimate = true\n";
        let mut ledger: Ledger = toml::from_str(text).unwrap();
        let today = date("2026-10-20");
        let home = ledger.budgets[0].clone();
        assert_eq!(ledger.status(&home, today).projected, Money(-32000));
        let line = |amount: f64, preset: &str| Line {
            id: String::new(),
            budget: "home".into(),
            date: date("2026-10-12"),
            amount: Money::from_units(amount),
            label: preset.into(),
            planned: false,
            reserve: None,
            links: Vec::new(),
            preset: Some(preset.into()),
            ..Line::default()
        };
        // The real phone bill replaces the preset's amount.
        ledger.lines.push(line(-29.99, "phone"));
        let status = ledger.status(&home, today);
        assert_eq!((status.so_far, status.projected), (Money(-2999), Money(-32999)));
        // Spending within the food envelope does not add to it; beyond it, it does.
        ledger.lines.push(line(-50.0, "food"));
        assert_eq!(ledger.status(&home, today).projected, Money(-32999));
        ledger.lines.push(line(-400.0, "food"));
        assert_eq!(ledger.status(&home, today).projected, Money(-2999 - 45000));
    }
}

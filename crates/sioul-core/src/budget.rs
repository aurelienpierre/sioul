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

use crate::cases::Route;
use crate::money::{Amount, Money};
use crate::payments::{self, Payment, PaymentKind};
use crate::porch::{Lane, Triaged};
use crate::trust::Trust;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};

/// The file's name, at the root of the case store.
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
    /// Cases, notes, tasks this budget belongs with.
    #[serde(default)]
    pub links: Vec<String>,
    /// "personal" for leisure money: it stays in view in quiet time.
    #[serde(default)]
    pub area: Option<String>,
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
    #[serde(default)]
    pub links: Vec<String>,
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
#[derive(Debug, Clone, Deserialize)]
pub struct Line {
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
#[derive(Debug, Clone, Default, Deserialize)]
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
    /// Which messages: the same conditions as a case's routes (docs/case-store.md).
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
    /// Reads `sioul-budgets.toml` at the root of a case store.
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
        let (start, end) = budget.period.bounds(today);
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
        let out: Money = flows.iter().filter(|f| f.amount.is_negative()).map(|f| f.amount.abs()).sum();
        // "As planned" within 2 % of what goes out, and never closer than 10 units.
        let tolerance = (out.cents() / 50).max(1000);
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
            .flat_map(|p| p.occurrences(from, to).into_iter().flat_map(move |d| self.occurrence(p, d, today)));
        lines.chain(presets).collect()
    }

    /// The preset occurrence a line stands for, if its preset falls in the line's period.
    fn absorbing(&self, line: &Line) -> Option<&Preset> {
        let id = line.preset.as_deref()?;
        let preset = self.presets.iter().find(|p| p.id.as_deref() == Some(id) && p.budget == line.budget)?;
        let (start, end) = preset.every.bounds(line.date);
        (!preset.occurrences(start, end).is_empty()).then_some(preset)
    }

    /// One occurrence of a preset, with the lines standing for it in its period.
    /// Without any, a preset dated up to today is assumed done. With some, a
    /// fixed preset is replaced by them (the real amount); an estimate is an
    /// envelope they are spent from, and what remains of it is still to come.
    fn occurrence(&self, preset: &Preset, date: Date, today: Date) -> Vec<Flow> {
        let (start, end) = preset.every.bounds(date);
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
            let Some(next) = budget.period.next(status.start) else { break };
            let (s, e) = budget.period.bounds(next);
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
}

impl MailLine {
    /// The amount with its sign: money in is positive.
    pub fn signed(&self) -> Option<Money> {
        let money = self.payment.amount.as_ref()?.money;
        Some(if self.payment.kind.is_credit() { money } else { -money })
    }

    /// The line it becomes in a budget; none without an amount.
    pub fn line(&self, budget: &str) -> Option<Line> {
        Some(Line {
            budget: budget.to_string(),
            date: self.date,
            amount: self.signed()?,
            label: self.label.clone(),
            planned: false,
            reserve: None,
            links: vec![self.key.clone()],
            preset: self.preset.clone(),
        })
    }
}

/// Every message about money among `items`, newest first, with what becomes of
/// it. Mail set aside (forged, spam) is never read; a rule gives the budget and
/// the preset; `ignored` holds the keys you said are not payments.
pub fn mail_lines(ledger: &Ledger, items: &[Triaged], ignored: &BTreeSet<String>) -> Vec<MailLine> {
    let recorded: BTreeSet<&str> = ledger.lines.iter().flat_map(|l| l.links.iter().map(String::as_str)).collect();
    let mut lines: Vec<MailLine> = items.iter().filter(|t| t.lane != Lane::SetAside).filter_map(|t| mail_line(ledger, t)).collect();
    lines.sort_by_key(|l| l.date);
    for line in &mut lines {
        line.state = if recorded.contains(line.key.as_str()) {
            MailState::Recorded
        } else if ignored.contains(&line.key) {
            MailState::Ignored
        } else {
            MailState::Proposed
        };
    }
    mark_duplicates(&mut lines);
    lines.reverse();
    lines
}

fn mail_line(ledger: &Ledger, t: &Triaged) -> Option<MailLine> {
    let card = &t.card;
    let rule = ledger.mail_rules.iter().find(|r| r.route.explain(card).is_some());
    let detected = payments::detect(card);
    let mut payment = match (detected, rule) {
        (Some(payment), _) => payment,
        (None, Some(rule)) => payments::read(card, kind_of(rule.direction), &payments::start(card)),
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
    let date = Timestamp::from_second(card.date?).ok()?.to_zoned(TimeZone::system()).date();
    let key = card.message_id.as_ref().map(|id| format!("mid:{id}")).or_else(|| card.path.as_ref().map(|p| format!("file:{}", p.display())))?;
    Some(MailLine {
        key,
        path: card.path.clone(),
        date,
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
    let id = if id.is_empty() { crate::cases::new_id(title, &taken) } else { id.to_string() };
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
    write_ledger(path, &doc)?;
    Ok(id)
}

/// Takes a budget out of the file. Its lines stay, written as they were,
/// and no longer count anywhere.
pub fn remove_budget(path: &Path, id: &str) -> Result<(), String> {
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
}

/// Takes one line out of the file, by its place among the lines (as
/// `sioul:budget/<budget>/<place>` names it).
pub fn remove_line(path: &Path, place: usize) -> Result<(), String> {
    let fail = |e: String| format!("{}: {e}", path.display());
    let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    let lines = doc.get_mut("line").and_then(Item::as_array_of_tables_mut).ok_or_else(|| fail("no line".into()))?;
    if place >= lines.len() {
        return Err(fail("no such line".into()));
    }
    lines.remove(place);
    write_ledger(path, &doc)
}

/// A line of the file changed in place: its label, amount and date; its links,
/// its budget and the comment saying where it came from stay. A date to come
/// makes it planned.
pub fn change_line(path: &Path, place: usize, label: &str, amount: f64, date: jiff::civil::Date, today: jiff::civil::Date) -> Result<(), String> {
    let fail = |e: String| format!("{}: {e}", path.display());
    if amount == 0.0 || !amount.is_finite() {
        return Err(fail("an amount is needed".into()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    let lines = doc.get_mut("line").and_then(Item::as_array_of_tables_mut).ok_or_else(|| fail("no line".into()))?;
    let line = lines.get_mut(place).ok_or_else(|| fail("no such line".into()))?;
    line["label"] = value(label.trim());
    line["amount"] = value((amount * 100.0).round() / 100.0);
    line["date"] = value(toml_edit::Datetime { date: Some(toml_edit::Date { year: date.year() as u16, month: date.month() as u8, day: date.day() as u8 }), time: None, offset: None });
    if date > today {
        line["planned"] = value(true);
    } else {
        line.remove("planned");
    }
    write_ledger(path, &doc)
}

/// Writes a line at the end of the budget file, keeping its comments, with a
/// comment saying where it came from.
pub fn record_line(path: &Path, line: &Line, origin: &str) -> Result<(), String> {
    let fail = |e: String| format!("{}: {e}", path.display());
    let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    let mut table = Table::new();
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
            let (from, _) = budget.period.bounds(end);
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
        let (start, end) = budget.period.bounds(today);
        let opening = self.opening(budget, start, today);
        let from = budget.since.map_or(start, |s| s.max(start));
        let flows = self.flows(budget, from, end, today);
        let scheduled: Money = flows.iter().filter(|f| f.scheduled).map(|f| f.amount).sum();
        let unscheduled: Money = flows.iter().filter(|f| !f.scheduled).map(|f| f.amount).sum();
        let days = |a: Date, b: Date| (b - a).get_days() as f64 + 1.0;
        let gone = (days(start, today) / days(start, end)).clamp(0.0, 1.0);
        let needed = budget.target - opening - scheduled;
        let expected_now = Money((needed.cents() as f64 * gone).round() as i64);
        let ahead = unscheduled - expected_now;
        // Within 5 % of what the period moves, and never closer than 10 units.
        let moved: i64 = flows.iter().map(|f| f.amount.cents().abs()).sum::<i64>() + needed.cents().abs();
        let tolerance = (moved / 20).max(1000);
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
            .map(|(i, l)| Movement { date: l.date, label: l.label.clone(), amount: l.amount, planned: l.planned, recurring: false, line: (!l.links.iter().any(|k| k.starts_with("bank:"))).then_some(i) })
            .collect();
        for preset in self.presets.iter().filter(|p| p.budget == budget.id) {
            for date in preset.occurrences(from, to) {
                // A line standing for this occurrence is listed itself.
                let stood_for = preset.id.as_deref().is_some_and(|id| {
                    let (start, end) = preset.every.bounds(date);
                    self.lines.iter().any(|l| l.preset.as_deref() == Some(id) && l.budget == budget.id && l.date >= start && l.date <= end)
                });
                if !stood_for {
                    out.push(Movement { date, label: preset.label.clone(), amount: preset.amount, planned: date > today, recurring: true, line: None });
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
}

/// Adds a recurring movement to the budget file, its comments kept.
pub fn record_preset(path: &Path, preset: &PresetEdit, origin: &str) -> Result<(), String> {
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
}

/// The planned line `link` names (an invoice, "sioul:invoice/2026-002") made
/// real: paid on `date`, no longer planned. Returns whether there was one.
pub fn settle_line(path: &Path, link: &str, date: Date) -> Result<bool, String> {
    let fail = |e: String| format!("{}: {e}", path.display());
    let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    let Some(lines) = doc.get_mut("line").and_then(Item::as_array_of_tables_mut) else { return Ok(false) };
    let Some(line) = lines.iter_mut().find(|t| t.get("links").and_then(Item::as_array).is_some_and(|a| a.iter().any(|v| v.as_str() == Some(link)))) else { return Ok(false) };
    line.remove("planned");
    let day = toml_edit::Date { year: date.year() as u16, month: date.month() as u8, day: date.day() as u8 };
    line["date"] = value(toml_edit::Datetime { date: Some(day), time: None, offset: None });
    let temporary = path.with_extension("toml.new");
    std::fs::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
    std::fs::rename(&temporary, path).map_err(|e| fail(e.to_string()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_made_changed_and_taken_out() {
        let path = std::env::temp_dir().join(format!("sioul-budget-edit-{}.toml", std::process::id()));
        std::fs::write(&path, "# mine\n[[budget]]\nid = \"food\"\ntitle = \"Food\"\nperiod = \"month\"\n\n[[line]]\nbudget = \"food\"\ndate = 2026-10-01\namount = -12.5\nlabel = \"Market\"\n").unwrap();
        let id = save_budget(&path, "", &BudgetEdit { title: "Holidays".into(), period: "year".into(), target: 300.0, personal: true, area: None }).unwrap();
        assert_eq!(id, "holidays");
        let ledger = Ledger::load_file(&path).unwrap();
        let holidays = ledger.budgets.iter().find(|b| b.id == "holidays").unwrap();
        assert_eq!((holidays.period, holidays.target.cents(), holidays.area.as_deref()), (Period::Year, 30_000, Some("personal")));
        save_budget(&path, "food", &BudgetEdit { title: "Groceries".into(), period: "month".into(), target: 0.0, personal: false, area: None }).unwrap();
        assert_eq!(Ledger::load_file(&path).unwrap().budget_title("food"), "Groceries");
        remove_line(&path, 0).unwrap();
        remove_budget(&path, "holidays").unwrap();
        let ledger = Ledger::load_file(&path).unwrap();
        assert!(ledger.lines.is_empty() && ledger.budgets.len() == 1);
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("# mine"));
        std::fs::remove_file(&path).unwrap();
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
        porch::triage(crate::card::Card::from_bytes(raw.as_bytes()).unwrap(), &Context { cases: None, known: &known, senders: &crate::porch::Senders::default(), trusted_ids: &[], now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &[], spam: None })
    }

    const PAY: &str = "Pay Exemple <service@pay.example>";

    #[test]
    fn mail_becomes_a_line() {
        let ledger: Ledger = toml::from_str("[[mail_rule]]\nbudget = 'projects'\ndirection = 'credit'\nfrom_domains = ['pay.example']\nsubject_contains = ['paiement reçu']").unwrap();
        let mail = [triaged(PAY, "Notification de paiement reçu", "Fri, 02 Oct 2026 10:00:00 +0200", "p1@pay.example", "Vous avez reçu un paiement de 25,00 € EUR de Jean Exemple (jean@example.org).")];
        let lines = mail_lines(&ledger, &mail, &BTreeSet::new());
        assert_eq!(lines.len(), 1);
        let line = lines[0].line("projects").unwrap();
        assert_eq!((line.amount, line.label.as_str()), (Money(2500), "Jean Exemple"));
        assert_eq!(line.links, vec!["mid:p1@pay.example".to_string()]);
        assert_eq!((lines[0].budget.as_deref(), &lines[0].state), (Some("projects"), &MailState::Proposed));
        // Once in the file, it is counted, not proposed again.
        let recorded = Ledger { lines: vec![line], ..ledger };
        assert_eq!(mail_lines(&recorded, &mail, &BTreeSet::new())[0].state, MailState::Recorded);
        assert_eq!(mail_lines(&Ledger::default(), &mail, &BTreeSet::from(["mid:p1@pay.example".to_string()]))[0].state, MailState::Ignored);
    }

    #[test]
    fn one_payment_told_twice_counts_once() {
        let mail = [
            triaged(PAY, "SHOP: 23,98 € EUR", "Wed, 30 Sep 2026 22:36:00 +0200", "pay@pay.example", "Vous avez autorisé un paiement de 23,98 € EUR en faveur de SHOP"),
            triaged("Shop <noreply@shop.example>", "Votre commande est validée !", "Wed, 30 Sep 2026 23:35:00 +0200", "order@shop.example", "Article 19,99 €\r\nLivraison 3,99 €\r\nTotal 23,98 €"),
            triaged("Telecom <factures@telecom.example>", "Votre facture mobile est disponible", "Sat, 26 Sep 2026 00:08:00 +0200", "bill@telecom.example", "Rendez-vous dans votre espace."),
            triaged("Telecom <factures@telecom.example>", "Le règlement de votre facture a bien été effectué", "Fri, 02 Oct 2026 17:30:00 +0200", "settled@telecom.example", "Merci."),
        ];
        let lines = mail_lines(&Ledger::default(), &mail, &BTreeSet::new());
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
            budget: "home".into(),
            date: date("2026-10-02"),
            amount: Money(-2999),
            label: "Telecom".into(),
            planned: false,
            reserve: None,
            links: vec!["mid:settled@telecom.example".into()],
            preset: Some("phone".into()),
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
            budget: "home".into(),
            date: date("2026-10-12"),
            amount: Money::from_units(amount),
            label: preset.into(),
            planned: false,
            reserve: None,
            links: Vec::new(),
            preset: Some(preset.into()),
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

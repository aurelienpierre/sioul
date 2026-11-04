// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! From one income to the next (docs/accounting.md, "From one income to the
//! next"): each bank account's stretch until its next money in, read from the
//! money movements (`movements`), for the budgets page's opening sentences,
//! its one chart, the reminders and the Porch.
//!
//! The question a person asks of an account is shaped by time: until the next
//! income, will the account hold every payment, what is left for everyday
//! spending, where is the lowest point (research/budgets.md §4.1; Stephens
//! 2003, Gelman et al. 2014: spending and bills follow income). So the
//! stretch ends at the next income, not at the month's end:
//! - **the balance now**: the bank's last balance, with the account's lines
//!   read after it;
//! - **what is expected**: every movement of the account still to come
//!   (planned, announced, not final yet), a day passed and not yet in an
//!   export counted today; everyday estimates are not payments, and what is
//!   left is for them;
//! - **the next income**: the first money in expected that is neither moved
//!   between your own accounts nor a refund, worth at least a fifth of the
//!   largest coming (and €50): the pay, the benefits, a client's payment. A
//!   recurring one, or one a letter or a mail announced, is sure; a planned
//!   line or an invoice of yours is said with its doubt ("if Client A pays by
//!   the 20th"; Berman et al. 2016, research §2.6);
//! - **the lowest point** of the balance carried with those payments until
//!   the income (GnuCash's "Future Minimum"), the payment that brings it there;
//! - **what is left for everyday spending**: that lowest point above what the
//!   account keeps (its floor), less the past year's unusual spending for
//!   those days (Sussman & Alter 2012: exceptional expenses are forgotten),
//!   and per day;
//! - **short**: the first payment the balance would not hold, and what tops
//!   it up (the reserves in their order, each above its floor, asked its delay ahead);
//! - with no income expected within two months, how long the account and its
//!   reserves last at the person's own pace, in weeks (Farrell et al. 2019:
//!   a buffer counted in weeks).
//!
//! Nothing here is stored: it is computed again from the files each time.

use crate::accounts;
use crate::areas::Area;
use crate::bank::Bank;
use crate::budget::{BankAccount, Ledger};
use crate::i18n::{self, Translator};
use crate::matching::Evidence;
use crate::money::Money;
use crate::movements::{Kind, Movement, Place, State, Way};
use jiff::civil::Date;
use std::path::Path;

/// An income further than this many days is none expected: the account's
/// sentence says how long it lasts instead.
pub const INCOME_DAYS: i64 = 62;
/// What is coming covers six weeks at least…
pub const COMING_DAYS: i64 = 42;
/// …and reaches the income after next, within this many days.
pub const COMING_MOST: i64 = 75;
/// The smallest income that ends a stretch, in cents: a refund of a few euros is no pay.
const INCOME_FLOOR: i64 = 5_000;

/// An income expected: the day it should come, what it is, its amount, and
/// whether it is sure (a recurring one, or announced) or said with its doubt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Income {
    pub day: Date,
    pub label: String,
    pub amount: Money,
    pub certain: bool,
    /// Its movement's address (`Movement::address`).
    pub address: String,
}

/// A point of the balance ahead: its day, the balance after it, and the
/// payment that brings it there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Low {
    pub day: Date,
    pub balance: Money,
    pub payment: String,
}

/// One movement still to come on an account, as the stretch carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flow {
    pub day: Date,
    pub amount: Money,
    pub label: String,
    /// It is an income (`Income`).
    pub income: bool,
    /// An income said with its doubt.
    pub uncertain: bool,
}

/// Money a reserve should send to an account so that a payment passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub reserve: String,
    pub title: String,
    pub amount: Money,
    /// The last day to ask it, its delay before the payment; today at the earliest.
    pub by: Date,
    pub delay: u32,
    /// Asked today, it still arrives after the payment.
    pub late: bool,
}

/// One account's stretch to its next income.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ahead {
    /// The bank account's id; "" for the one account of exports when none is declared.
    pub account: String,
    pub title: String,
    /// What it is for: the areas of the budgets it fills (docs/areas.md).
    pub area: Area,
    pub floor: Money,
    /// The balance today.
    pub now: Money,
    pub income: Option<Income>,
    /// The income after it, for What is coming and a doubtful income.
    pub after: Option<Income>,
    /// The lowest point before the income, when a payment brings it below today's balance.
    pub low: Option<Low>,
    /// The first payment before the income the account would not hold above its floor.
    pub short: Option<Low>,
    /// With a doubtful income that holds: where it would go short without it.
    pub without: Option<Low>,
    /// Left for everyday spending until the income, and for how many days.
    pub left: Money,
    pub days: i64,
    /// The past twelve months' unusual spending, a month.
    pub unusual: Money,
    /// With no income expected: how long the account and its reserves last, in weeks.
    pub lasts: Option<i64>,
    /// The reserves that top it up, by title, in their order.
    pub reserves: Vec<String>,
    /// What tops it up for its shortfall.
    pub covers: Vec<Cover>,
    /// What is expected on it from today, in date order.
    pub flows: Vec<Flow>,
}

impl Ahead {
    /// The balance on `day` as the stretch carries it.
    pub fn balance_on(&self, day: Date) -> Money {
        self.now + self.flows.iter().filter(|f| f.day <= day).map(|f| f.amount).sum::<Money>()
    }
}

fn shifted(day: Date, days: i64) -> Date {
    day.checked_add(jiff::Span::new().days(days)).unwrap_or(day)
}

fn days_between(a: Date, b: Date) -> i64 {
    i64::from((b - a).get_days())
}

/// Whether a movement is an account's: its own, or, without one (a letter's
/// bill in no budget yet), the first account's.
fn of_account(m: &Movement, account: Option<&BankAccount>, first: bool) -> bool {
    match account {
        Some(a) => m.account.as_deref() == Some(a.id.as_str()) || (m.account.is_none() && first),
        None => true,
    }
}

/// What a movement still to come takes or brings, prudently: the most a
/// payment may take, the least an income may bring.
fn prudent(m: &Movement) -> Money {
    let (low, high) = m.sum.bounds();
    if m.way == Way::In { low.min(high).max(Money::ZERO) } else { low.min(high) }
}

/// Whether a movement may be an income: money in, in a budget, planned or
/// announced by what plans or tells of an income, never a refund.
fn may_be_income(m: &Movement) -> bool {
    m.way == Way::In
        && matches!(m.place, Place::Budget(_) | Place::Parts(_))
        && matches!(m.state, State::Planned | State::Announced)
        && m.evidence != Some(Evidence::Refund)
        && m.sources.iter().any(|s| matches!(s.kind, Kind::Preset | Kind::Line | Kind::Invoice | Kind::Letter | Kind::Mail))
}

/// An income is sure when it comes back (a preset) or was announced; a
/// planned line or an invoice of yours waits on someone else's payment.
fn certain(m: &Movement) -> bool {
    m.state == State::Announced || m.sources.iter().any(|s| s.kind == Kind::Preset)
}

/// The past twelve months' unusual spending, a month: the account's
/// payments out that stand for no recurring payment and are at least three
/// times its usual payment (and €50), summed over the months its lines
/// cover (three at least), per month (Sussman & Alter 2012; research §2.6).
fn unusual(done: &[&Movement], today: Date) -> Money {
    let year = shifted(today, -365);
    let out: Vec<&Movement> = done.iter().copied().filter(|m| m.day >= year && m.day < today && m.way == Way::Out && !matches!(m.place, Place::Transfer { .. } | Place::Reserve { .. })).collect();
    let Some(first) = out.iter().map(|m| m.day).min() else { return Money::ZERO };
    let months = (days_between(first, today) as f64 / 30.44).min(12.0);
    if months < 3.0 {
        return Money::ZERO;
    }
    let mut sizes: Vec<i64> = out.iter().map(|m| -m.sum.amount().cents()).collect();
    sizes.sort_unstable();
    let usual = sizes[sizes.len() / 2];
    let threshold = (usual * 3).max(5_000);
    let rare: i64 = out.iter().filter(|m| m.stands.is_none() && !m.sources.iter().any(|s| s.kind == Kind::Preset)).map(|m| -m.sum.amount().cents()).filter(|c| *c >= threshold).sum();
    Money((rare as f64 / months).round() as i64)
}

/// What the person spends a week from an account: its payments out of the
/// past thirteen weeks (moves between accounts and savings aside); without
/// any, what is expected over the next four weeks.
fn weekly(done: &[&Movement], flows: &[Flow], today: Date) -> Money {
    let from = shifted(today, -91);
    let past: Vec<i64> = done.iter().filter(|m| m.day >= from && m.day < today && m.way == Way::Out && !matches!(m.place, Place::Transfer { .. } | Place::Reserve { .. })).map(|m| -m.sum.amount().cents()).collect();
    if let Some(first) = done.iter().filter(|m| m.day >= from).map(|m| m.day).min()
        && !past.is_empty()
    {
        let weeks = (days_between(first, today) as f64 / 7.0).clamp(1.0, 13.0);
        return Money((past.iter().sum::<i64>() as f64 / weeks).round() as i64);
    }
    let month = shifted(today, 28);
    Money(-flows.iter().filter(|f| f.day <= month && f.amount.is_negative()).map(|f| f.amount.cents()).sum::<i64>() / 4)
}

/// The first day the balance, from `now`, carried with `flows` before
/// `until`, goes under `floor` on a payment; and its lowest point.
fn carry(now: Money, flows: &[Flow], until: Date, floor: Money) -> (Option<Low>, Option<Low>) {
    let mut balance = now;
    let mut low: Option<Low> = None;
    let mut short = None;
    for f in flows.iter().filter(|f| f.day < until) {
        balance += f.amount;
        if !f.amount.is_negative() {
            continue;
        }
        if low.as_ref().is_none_or(|l| balance < l.balance) && balance < now {
            low = Some(Low { day: f.day, balance, payment: f.label.clone() });
        }
        if short.is_none() && balance < floor {
            short = Some(Low { day: f.day, balance, payment: f.label.clone() });
        }
    }
    (low, short)
}

/// Each bank account's stretch to its next income (`Ahead`), from the
/// books' counted budget file (the reserves' balances), the bank's balances
/// and the money movements. With no account declared and exports read, one
/// account: the exports' own.
pub fn of(ledger: &Ledger, bank: &Bank, movements: &[Movement], today: Date) -> Vec<Ahead> {
    let declared: Vec<Option<&BankAccount>> = if ledger.bank_accounts.is_empty() {
        if bank.movements.is_empty() && bank.accounts.iter().all(|a| a.balance.is_none()) { Vec::new() } else { vec![None] }
    } else {
        ledger.bank_accounts.iter().map(Some).collect()
    };
    let budget_area = |id: &str| ledger.budgets.iter().find(|b| b.id == id).map_or(Area::ADMIN, crate::budget::Budget::area_of);
    let mut out = Vec::new();
    for (index, account) in declared.into_iter().enumerate() {
        let first = index == 0;
        let balance = match account {
            Some(a) => accounts::balance_of(bank, a),
            None => bank.balance_in(ledger),
        };
        let Some((as_of, known)) = balance else { continue };
        let mine: Vec<&Movement> = movements.iter().filter(|m| of_account(m, account, first)).collect();
        let done: Vec<&Movement> = mine.iter().copied().filter(|m| m.state == State::Done).collect();
        // Today's balance: the bank's, with the lines read after it.
        let now = known + done.iter().filter(|m| m.day > as_of && m.day <= today && m.sources.iter().any(|s| s.kind == Kind::Bank)).map(|m| m.sum.amount()).sum::<Money>();
        let horizon = shifted(today, COMING_MOST + 31);
        // Still to come: what its day has not reached yet, or what the exports could not show
        // yet (a week before their last day at most: further back, it would be not seen).
        let coming: Vec<&Movement> = mine.iter().copied().filter(|m| matches!(m.state, State::Planned | State::Announced | State::NotFinal) && m.day > shifted(as_of, -7) && m.day <= horizon && m.way != Way::Between).collect();
        let largest = coming.iter().filter(|m| may_be_income(m) && m.day <= shifted(today, INCOME_DAYS)).map(|m| prudent(m).cents()).max().unwrap_or(0);
        let is_income = |m: &Movement| may_be_income(m) && prudent(m).cents() >= (largest / 5).max(INCOME_FLOOR);
        let mut flows: Vec<(Flow, &Movement)> = coming
            .iter()
            .map(|m| {
                let income = is_income(m);
                (Flow { day: m.day.max(today), amount: prudent(m), label: m.label.clone(), income, uncertain: income && !certain(m) }, *m)
            })
            .filter(|(f, _)| f.amount != Money::ZERO)
            .collect();
        flows.sort_by(|(a, _), (b, _)| (a.day, a.amount).cmp(&(b.day, b.amount)));
        let incomes: Vec<Income> = flows
            .iter()
            .filter(|(f, _)| f.income && f.day <= shifted(today, INCOME_DAYS))
            .map(|(f, m)| Income { day: f.day, label: m.label.clone(), amount: f.amount, certain: !f.uncertain, address: m.address() })
            .collect();
        let income = incomes.first().cloned();
        let after = income.as_ref().and_then(|i| incomes.iter().find(|j| j.day > i.day).cloned());
        let flows: Vec<Flow> = flows.into_iter().map(|(f, _)| f).collect();
        let floor = account.map_or(Money::ZERO, |a| a.floor);
        let unusual = unusual(&done, today);
        let reserve_room = |id: &str| ledger.reserves.iter().find(|r| r.id == id).map(|r| (r, (ledger.reserve_status(r, today).balance_now - r.floor).cents().max(0)));
        let topped: Vec<&str> = account.map(|a| a.topped_up_by.iter().map(String::as_str).collect()).unwrap_or_default();
        let mut ahead = Ahead {
            account: account.map(|a| a.id.clone()).unwrap_or_default(),
            title: account.map(|a| a.title.clone()).unwrap_or_default(),
            area: account.map(|a| a.fills.iter().fold(Area::default(), |area, b| area.with(budget_area(b)))).filter(|a| !a.is_empty()).unwrap_or(Area::ADMIN),
            floor,
            now,
            income: income.clone(),
            after,
            low: None,
            short: None,
            without: None,
            left: Money::ZERO,
            days: 0,
            unusual,
            lasts: None,
            reserves: topped.iter().filter_map(|id| ledger.reserves.iter().find(|r| r.id == *id)).map(|r| r.title.clone()).collect(),
            covers: Vec::new(),
            flows,
        };
        match &income {
            Some(income) => {
                let (low, short) = carry(now, &ahead.flows, income.day, floor);
                ahead.days = days_between(today, income.day).max(1);
                let lowest = low.as_ref().map_or(now, |l| l.balance).min(now);
                let unusual_share = Money((unusual.cents() as f64 * ahead.days as f64 / 30.44).round() as i64);
                ahead.left = Money((lowest - floor - unusual_share).cents().max(0));
                ahead.low = low;
                ahead.short = short;
                // A doubtful income: where the account would stand without it, until the next sure one.
                if !income.certain && ahead.short.is_none() {
                    let until = ahead.after.as_ref().map_or(shifted(today, INCOME_DAYS), |a| a.day);
                    let others: Vec<Flow> = ahead.flows.iter().filter(|f| !(f.income && f.day == income.day && f.label == income.label)).cloned().collect();
                    ahead.without = carry(now, &others, until, floor).1;
                }
            }
            None => {
                let pace = weekly(&done, &ahead.flows, today);
                let room = (now - floor).cents().max(0) + topped.iter().filter_map(|id| reserve_room(id)).map(|(_, room)| room).sum::<i64>();
                ahead.lasts = (pace.cents() > 0).then(|| room / pace.cents());
                ahead.short = carry(now, &ahead.flows, shifted(today, INCOME_DAYS), floor).1;
            }
        }
        // What tops it up: what is missing, rounded up to ten, from the reserves in order.
        if let Some(short) = &ahead.short {
            let mut missing = ((floor - short.balance).cents() + 999) / 1000 * 1000;
            for id in &topped {
                let Some((reserve, room)) = reserve_room(id) else { continue };
                if room <= 0 || missing <= 0 {
                    continue;
                }
                let amount = missing.min(room);
                missing -= amount;
                let delay = i64::from(reserve.delay_days);
                ahead.covers.push(Cover {
                    reserve: reserve.id.clone(),
                    title: reserve.title.clone(),
                    amount: Money(amount),
                    by: shifted(short.day, -delay).max(today),
                    delay: reserve.delay_days,
                    late: shifted(today, delay) > short.day,
                });
            }
        }
        out.push(ahead);
    }
    out
}

/// The stretches of a notes folder's accounts, from its books (`movements::books`).
pub fn of_folder(root: &Path, words: &crate::words::Words, today: Date) -> Vec<Ahead> {
    let books = crate::movements::books(root, words, today);
    of(&books.counted, &books.bank, &books.movements, today)
}

/// A shortfall ahead on an account: the payment it would not hold, and what covers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortfall {
    pub account: String,
    pub title: String,
    pub day: Date,
    pub payment: String,
    /// How much is missing under what the account keeps.
    pub missing: Money,
    pub covers: Vec<Cover>,
}

/// The shortfalls ahead, one per account: a shortfall concerns every area,
/// whatever a page's filter shows (research §6, recommendation 7; Caflisch
/// et al. 2018: alerts that come by default).
pub fn shortfalls(aheads: &[Ahead]) -> Vec<Shortfall> {
    aheads
        .iter()
        .filter_map(|a| {
            let short = a.short.as_ref()?;
            Some(Shortfall { account: a.account.clone(), title: a.title.clone(), day: short.day, payment: short.payment.clone(), missing: a.floor - short.balance, covers: a.covers.clone() })
        })
        .collect()
}

impl Shortfall {
    /// Its key, the same on every device while it is the same shortfall: told once.
    pub fn key(&self) -> String {
        format!("short/{}/{}", if self.account.is_empty() { "-" } else { &self.account }, self.day)
    }

    /// In words: "The rent on Wednesday 26 November would leave the account
    /// €60 short. Instant savings can cover it: move €60 by Tuesday 25 November."
    pub fn sentence(&self, tr: &Translator, today: Date) -> String {
        let mut args = i18n::args();
        args.set("payment", self.payment.clone());
        args.set("when", tr.day_in(self.day, today));
        args.set("missing", tr.money(self.missing));
        let mut said = vec![tr.text("ahead-shortfall", Some(&args))];
        said.extend(cover_sentences(&self.covers, tr, today, self.day));
        said.join(" ")
    }
}

/// What tops an account up, in words: each reserve, what to move or ask, by when.
pub fn cover_sentences(covers: &[Cover], tr: &Translator, today: Date, needed: Date) -> Vec<String> {
    if covers.is_empty() {
        return vec![tr.text("ahead-cover-none", None)];
    }
    covers
        .iter()
        .map(|c| {
            let mut args = i18n::args();
            args.set("reserve", c.title.clone());
            args.set("amount", tr.money(c.amount));
            args.set("by", tr.day_in(c.by, today));
            args.set("when", tr.day_in(needed, today));
            args.set("days", i64::from(c.delay));
            tr.text(if c.late { "ahead-cover-late" } else if c.delay == 0 { "ahead-cover-now" } else { "ahead-cover-ask" }, Some(&args))
        })
        .collect()
}

/// The shortfalls of a notes folder as the Porch's notices (docs/porch.md,
/// "Notices"; docs/accounting.md, "A shortfall concerns every area"): one per
/// account and day, in every area (its area unsaid), in the money section,
/// opening the budgets page. The Porch shows them at the next admin window,
/// whatever the budgets' filter; each is the same notice on every device
/// while the shortfall is the same (`notices::money_id`).
pub fn shortfall_notices(root: &Path, words: &crate::words::Words, tr: &Translator, today: Date) -> Vec<crate::notices::Notice> {
    let aheads = of_folder(root, words, today);
    shortfalls(&aheads)
        .iter()
        .map(|s| {
            let id = crate::notices::money_id(&s.key());
            crate::notices::Notice {
                address: "sioul:budget/".into(),
                at: s.day.to_zoned(crate::zone::current()).map_or(0, |z| z.timestamp().as_millisecond()),
                when: tr.day_in(s.day, today),
                who: if s.title.is_empty() { tr.text("ahead-your-account", None) } else { s.title.clone() },
                what: s.sentence(tr, today),
                ..crate::notices::Notice::new(crate::notices::Kind::Money, id, "money")
            }
        })
        .collect()
}

/// "about a week", "about 7 weeks", "more than a year".
pub fn weeks_text(tr: &Translator, weeks: i64) -> String {
    if weeks >= 52 {
        return tr.text("ahead-weeks-year", None);
    }
    let mut args = i18n::args();
    args.set("weeks", weeks.max(1));
    tr.text("ahead-weeks", Some(&args))
}

/// About so much, rounded as people say it: to the euro under €20, to five
/// under €100, to ten above.
pub fn about(m: Money) -> Money {
    let cents = m.cents().abs();
    let step = if cents < 2_000 { 100 } else if cents < 10_000 { 500 } else { 1_000 };
    let rounded = ((cents + step / 2) / step * step).max(if cents > 0 { 100 } else { 0 });
    Money(if m.is_negative() { -rounded } else { rounded })
}

/// The account's opening sentences (docs/accounting.md, "From one income to
/// the next"; research §6, recommendation 1): until when it holds every
/// payment, what is left for everyday spending and a day, the lowest point;
/// or where it would go short and what covers it; with no income expected,
/// how long it lasts.
pub fn sentences(a: &Ahead, tr: &Translator, today: Date) -> Vec<String> {
    let mut said = Vec::new();
    let day = |d: Date| tr.day_in(d, today);
    let Some(income) = &a.income else {
        if let Some(short) = &a.short {
            let mut args = i18n::args();
            args.set("payment", short.payment.clone());
            args.set("when", day(short.day));
            args.set("missing", tr.money(a.floor - short.balance));
            said.push(tr.text("ahead-shortfall", Some(&args)));
            said.extend(cover_sentences(&a.covers, tr, today, short.day));
        }
        if let Some(weeks) = a.lasts {
            let mut args = i18n::args();
            args.set("weeks", weeks_text(tr, weeks));
            if a.reserves.is_empty() {
                said.push(tr.text("ahead-lasts", Some(&args)));
            } else {
                args.set("reserves", list(tr, &a.reserves));
                said.push(tr.text("ahead-lasts-with", Some(&args)));
            }
        }
        return said;
    };
    let mut args = i18n::args();
    args.set("income", income.label.clone());
    args.set("day", day(income.day));
    match &a.short {
        Some(short) => {
            args.set("payment", short.payment.clone());
            args.set("when", day(short.day));
            args.set("missing", tr.money(a.floor - short.balance));
            said.push(tr.text(if income.certain { "ahead-short" } else { "ahead-short-if" }, Some(&args)));
            said.extend(cover_sentences(&a.covers, tr, today, short.day));
            return said;
        }
        None => said.push(tr.text(if income.certain { "ahead-holds" } else { "ahead-holds-if" }, Some(&args))),
    }
    if a.left.cents() > 0 {
        let mut args = i18n::args();
        args.set("left", tr.money(about(a.left)));
        args.set("per_day", tr.money(about(Money(a.left.cents() / a.days.max(1)))));
        said.push(tr.text("ahead-left", Some(&args)));
    } else {
        said.push(tr.text("ahead-left-none", None));
    }
    if let Some(low) = &a.low {
        let mut args = i18n::args();
        args.set("amount", tr.money(about(low.balance)));
        args.set("when", day(low.day));
        args.set("payment", low.payment.clone());
        // Its amount said once: the lowest point is what is left when nothing else counts.
        let same = about(low.balance - a.floor) == about(a.left);
        said.push(tr.text(if same { "ahead-low-day" } else { "ahead-low" }, Some(&args)));
    }
    if let Some(without) = &a.without {
        let mut args = i18n::args();
        args.set("payment", without.payment.clone());
        args.set("when", day(without.day));
        args.set("missing", tr.money(a.floor - without.balance));
        said.push(tr.text("ahead-without", Some(&args)));
    }
    said
}

/// "Instant savings", "Instant savings and Long-term savings", "A, B and C".
pub fn list(tr: &Translator, items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => {
            let mut args = i18n::args();
            args.set("rest", rest.join(", "));
            args.set("last", last.clone());
            tr.text("ahead-and", Some(&args))
        }
    }
}

/// The balance ahead as the page's one chart draws it (research §4.1): a
/// step at each movement, from today to a few days after the next income
/// (three weeks without one); a doubtful income's absence as a second line.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Chart {
    /// Each step: {day, cents}.
    pub points: Vec<Step>,
    /// Without the doubtful income, from its day on; empty when there is none.
    pub without: Vec<Step>,
    pub floor_cents: i64,
    /// The incomes and the large payments, marked with their names.
    pub marks: Vec<Mark>,
    pub low_cents: i64,
    pub high_cents: i64,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Step {
    pub day: String,
    pub cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Mark {
    pub day: String,
    pub cents: i64,
    pub label: String,
    pub income: bool,
    pub uncertain: bool,
}

/// The account's chart (`Chart`).
pub fn chart(a: &Ahead, today: Date) -> Chart {
    let to = a.income.as_ref().map_or(shifted(today, 21), |i| shifted(i.day, 3));
    let steps = |flows: &[&Flow]| -> Vec<Step> {
        let mut balance = a.now;
        let mut points = vec![Step { day: today.to_string(), cents: balance.cents() }];
        for f in flows.iter().filter(|f| f.day <= to) {
            balance += f.amount;
            match points.last_mut() {
                Some(last) if last.day == f.day.to_string() => last.cents = balance.cents(),
                _ => points.push(Step { day: f.day.to_string(), cents: balance.cents() }),
            }
        }
        points.push(Step { day: to.to_string(), cents: balance.cents() });
        points
    };
    let all: Vec<&Flow> = a.flows.iter().collect();
    let points = steps(&all);
    let doubtful = a.flows.iter().find(|f| f.uncertain && f.day <= to);
    let without = doubtful.map_or_else(Vec::new, |d| {
        let others: Vec<&Flow> = a.flows.iter().filter(|f| !std::ptr::eq(*f, d)).collect();
        steps(&others).into_iter().filter(|s| s.day >= d.day.to_string()).collect()
    });
    // Large: a payment of a tenth of today's balance, €100 at least.
    let large = (a.now.cents().abs() / 10).max(10_000);
    let mut balance = a.now;
    let mut marks = Vec::new();
    for f in a.flows.iter().filter(|f| f.day <= to) {
        balance += f.amount;
        if f.income || -f.amount.cents() >= large {
            marks.push(Mark { day: f.day.to_string(), cents: balance.cents(), label: f.label.clone(), income: f.income, uncertain: f.uncertain });
        }
    }
    let all_cents = points.iter().chain(without.iter()).map(|p| p.cents).chain([a.floor.cents(), 0]);
    let (low, high) = all_cents.fold((i64::MAX, i64::MIN), |(lo, hi), c| (lo.min(c), hi.max(c)));
    Chart { points, without, floor_cents: a.floor.cents(), marks, low_cents: low, high_cents: high, from: today.to_string(), to: to.to_string() }
}

#[cfg(test)]
#[path = "ahead_tests.rs"]
mod tests;

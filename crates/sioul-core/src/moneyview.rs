// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The budgets page in words (docs/accounting.md, "The budgets page"): each
//! account from one income to the next (`ahead`), each budget as what is
//! paid, what is still to come and what is left, and what is coming over the
//! next weeks; no percentage, no colour, no bar (research/budgets.md §4.2;
//! Bazley et al. 2021: red changes expectations). The words are research
//! §4.3's: money in and out, expected, not final yet, has left the account,
//! the same payment; never debit, credit, cleared or pending.

use crate::ahead::{self, Ahead};
use crate::areas::Area;
use crate::budget::{Budget, Ledger, Verdict};
use crate::i18n::{self, Translator};
use crate::money::Money;
use crate::movements::{Kind, Movement, Place, State, Sum, Way};
use jiff::civil::Date;
use serde::Serialize;
use std::collections::BTreeSet;

/// One account's opening sentences.
#[derive(Debug, Clone, Serialize)]
pub struct AccountSay {
    pub id: String,
    pub title: String,
    pub sentences: Vec<String>,
    /// It would not hold a payment: said whatever the filter shows.
    pub short: bool,
}

/// The page's one chart, for the first account shown.
#[derive(Debug, Clone, Serialize)]
pub struct ChartView {
    pub account: String,
    /// What it shows, for a screen reader: "The balance ahead, until Friday 28 November".
    pub name: String,
    pub floor_label: String,
    pub chart: ahead::Chart,
}

/// What is coming in one week.
#[derive(Debug, Clone, Serialize)]
pub struct WeekView {
    pub title: String,
    pub rows: Vec<ComingRow>,
}

/// One source of a movement, as its row shows it: what it is, and what opens it.
#[derive(Debug, Clone, Serialize)]
pub struct SourceView {
    pub kind: Kind,
    pub words: String,
    /// What opening it opens ("mid:…", "sioul:letter/…", "sioul:budget/…"); "" for none.
    pub address: String,
    /// Joined as probably the same payment: "Not the same" beside it.
    pub probable: bool,
    /// Its key, by which "Not the same" names it.
    pub key: String,
}

/// One movement of What is coming.
#[derive(Debug, Clone, Serialize)]
pub struct ComingRow {
    pub address: String,
    /// Its steadiest key, by which a move or a correction names it.
    pub key: String,
    pub account: String,
    /// "2026-11-28", and as Mail's rows say a day: "Fri 28 Nov".
    pub day: String,
    pub date: String,
    pub who: String,
    /// "€19.99", "about €60", "€50 to €70".
    pub amount: String,
    pub cents: i64,
    /// "in", "out", "between".
    pub way: &'static str,
    pub way_words: String,
    /// Its budgets' titles; "" in none.
    pub budget: String,
    pub budget_id: String,
    pub state: String,
    pub sources: Vec<SourceView>,
    /// "From a mail and the contract."
    pub from: String,
    /// A source probably joined: "Not the same" offered.
    pub probable: bool,
}

/// "€19.99", "about €60", "€50 to €70", its sign aside.
pub fn sum_words(tr: &Translator, sum: Sum) -> String {
    match sum {
        Sum::Exact(m) => tr.money(Money(m.cents().abs())),
        Sum::About(m) => {
            let mut args = i18n::args();
            args.set("amount", tr.money(ahead::about(Money(m.cents().abs()))));
            tr.text("coming-about", Some(&args))
        }
        Sum::Between(a, b) => {
            let (low, high) = (a.cents().abs().min(b.cents().abs()), a.cents().abs().max(b.cents().abs()));
            let mut args = i18n::args();
            args.set("low", tr.money(Money(low)));
            args.set("high", tr.money(Money(high)));
            tr.text("coming-between", Some(&args))
        }
    }
}

/// A day as Mail's rows say it: "Fri 28 Nov", "ven. 28 nov.".
pub fn short_day(tr: &Translator, day: Date) -> String {
    format!("{} {} {}", tr.weekday_short(day), day.day(), tr.month_short(day))
}

/// A budget's share of a movement: the whole, or its part there.
fn share(m: &Movement, budget: &str) -> Option<Money> {
    match &m.place {
        Place::Budget(b) | Place::Reserve { budget: b, .. } if b == budget => Some(m.sum.amount()),
        Place::Parts(parts) => parts.iter().find(|(b, _)| b == budget).map(|(_, a)| *a),
        _ => None,
    }
}

/// Planned or told before it happened: a recurring payment, a line, a letter, an invoice, a contract.
fn planned_kind(m: &Movement) -> bool {
    m.sources.iter().any(|s| matches!(s.kind, Kind::Preset | Kind::Line | Kind::Letter | Kind::Invoice | Kind::Contract | Kind::SetAside))
}

/// Who a movement is, as a row or a list says it: a share set aside by its words ("Taxes and contributions, 22 %").
pub fn who(tr: &Translator, m: &Movement) -> String {
    if m.sources.iter().any(|s| s.kind == Kind::SetAside) {
        let mut args = i18n::args();
        args.set("share", m.label.clone());
        tr.text("set-aside-label", Some(&args))
    } else {
        m.label.clone()
    }
}

/// "rent, energy"; each once.
fn names(items: &[String]) -> String {
    let mut seen = BTreeSet::new();
    items.iter().filter(|i| seen.insert(i.to_lowercase())).cloned().collect::<Vec<_>>().join(", ")
}

/// Each budget in words (docs/accounting.md, "Each budget in words";
/// research §6, recommendation 2): what was paid and what is still to come,
/// named; what came in and what is still to come in; what is left of each
/// everyday estimate until the period's end; what is set aside for taxes;
/// then one sentence about the period's end, with the reserve that covers it.
/// `assumed` says whether a past day planned and not on any export counts as
/// done (no export of its account: the budgets count it so).
pub fn budget_words(ledger: &Ledger, budget: &Budget, end_of: &crate::budget::PeriodEnd, movements: &[Movement], tr: &Translator, today: Date, exported: &dyn Fn(Option<&str>) -> bool) -> (Vec<String>, String) {
    let (start, end) = ledger.bounds(budget, today);
    let mut paid = Vec::new();
    let mut to_come = Vec::new();
    let mut not_seen = Vec::new();
    let mut came_in = Vec::new();
    let mut to_come_in = Vec::new();
    let mut aside = Vec::new();
    for m in movements.iter().filter(|m| m.day >= start && m.day <= end) {
        let Some(amount) = share(m, &budget.id) else { continue };
        // A share set aside still to declare; paid early, it is said paid.
        if m.sources.iter().any(|s| s.kind == Kind::SetAside) && m.state != State::Done {
            aside.push(m);
            continue;
        }
        if !planned_kind(m) || m.way == Way::Between {
            continue;
        }
        let label = who(tr, m);
        let item = || {
            let mut args = i18n::args();
            args.set("label", label.clone());
            args.set("amount", if m.sum.about() { sum_words(tr, m.sum) } else { tr.money(Money(amount.cents().abs())) });
            args.set("day", tr.day_in(m.day, today));
            tr.text("bw-item", Some(&args))
        };
        let out = amount.is_negative();
        let waiting = matches!(m.state, State::Planned | State::Announced | State::NotFinal);
        match m.state {
            State::Done => (if out { &mut paid } else { &mut came_in }).push(label.clone()),
            State::NotSeen => not_seen.push(label.clone()),
            _ if waiting && m.day < today && !exported(m.account.as_deref()) => (if out { &mut paid } else { &mut came_in }).push(label.clone()),
            _ if waiting && m.day < today && m.state != State::NotFinal => not_seen.push(label.clone()),
            _ => (if out { &mut to_come } else { &mut to_come_in }).push(item()),
        }
    }
    let mut said = Vec::new();
    let mut list = |id: &str, items: Vec<String>, joined: &dyn Fn(&[String]) -> String| {
        if !items.is_empty() {
            let mut args = i18n::args();
            args.set("list", joined(&items));
            said.push(tr.text(id, Some(&args)));
        }
    };
    // "Phone, €19.99, Thursday 12 November; Rent, …": each item has its commas.
    let separator = tr.text("bw-separator", None);
    let each = |items: &[String]| items.join(&separator);
    list("bw-paid", paid, &names);
    list("bw-to-come", to_come, &each);
    list("bw-not-seen", not_seen, &names);
    list("bw-in", came_in, &names);
    list("bw-to-come-in", to_come_in, &each);
    // Each everyday estimate: what is left of it, until when; past it, how much more than planned.
    for preset in ledger.presets.iter().filter(|p| p.budget == budget.id && p.estimate) {
        let Some((rest, until)) = ledger.estimate_left(preset, today) else { continue };
        let mut args = i18n::args();
        args.set("what", preset.label.clone());
        args.set("end", tr.day_month(until));
        let (left, over) = if preset.amount.is_negative() { (-rest.cents(), rest.cents()) } else { (rest.cents(), -rest.cents()) };
        if left > 0 {
            args.set("left", tr.money(Money(left)));
            said.push(tr.text("bw-left", Some(&args)));
        } else if over > 0 {
            args.set("over", tr.money(Money(over)));
            said.push(tr.text("bw-over", Some(&args)));
        }
    }
    for m in movements.iter().filter(|m| m.sources.iter().any(|s| s.kind == Kind::SetAside) && m.state != State::Done && share(m, &budget.id).is_some() && m.day >= today && m.day > end) {
        aside.push(m);
    }
    if let Some(next) = aside.iter().filter(|m| m.day >= today).min_by_key(|m| m.day) {
        let mut args = i18n::args();
        args.set("amount", sum_words(tr, next.sum));
        args.set("day", tr.day_in(next.day, today));
        said.push(tr.text("bw-set-aside", Some(&args)));
    }
    (said, verdict_sentence(ledger, end_of, tr))
}

/// One sentence about the period's end (`Ledger::period_end`), with the
/// reserve that covers a gap or takes a surplus: "By 30 November it should
/// end €40 better than planned, which goes to Long-term savings."
pub fn verdict_sentence(ledger: &Ledger, end: &crate::budget::PeriodEnd, tr: &Translator) -> String {
    let mut args = i18n::args();
    args.set("end", tr.day_month(end.end));
    args.set("gap", tr.money(Money(end.gap.cents().abs())));
    let reserve = end.reserve.as_ref().map(|(id, _)| ledger.reserve_title(id).to_string());
    if let Some(reserve) = &reserve {
        args.set("reserve", reserve.clone());
    }
    let id = match (end.verdict, reserve.is_some()) {
        (Verdict::Better, true) => "bw-end-better-swept",
        (Verdict::Better, false) => "bw-end-better",
        (Verdict::AsPlanned, _) => "bw-end-as-planned",
        (Verdict::Short, true) => "bw-end-short-drawn",
        (Verdict::Short, false) => "bw-end-short",
    };
    tr.text(id, Some(&args))
}

/// What the hour or your ticks leave out, in words, never how many
/// (research §6, recommendation 7): "Work budgets are kept for your work
/// hours." Said for each area left out that some budget is for.
pub fn left_out(ledger: &Ledger, shown: Area, own: bool, tr: &Translator) -> Vec<String> {
    let hidden: Vec<Area> = ledger.budgets.iter().map(Budget::area_of).filter(|a| !a.meets(shown)).collect();
    [(Area::WORK, "work"), (Area::ADMIN, "admin"), (Area::LEISURE, "leisure")]
        .iter()
        .filter(|(area, _)| hidden.iter().any(|h| h.meets(*area)))
        .map(|(_, id)| tr.text(&format!("{}-{id}", if own { "ahead-unticked" } else { "ahead-kept" }), None))
        .collect()
}

/// The words for a source's kind in a row: "a mail", "the contract".
fn source_words(tr: &Translator, kind: Kind) -> String {
    let id = match kind {
        Kind::Line => "source-line",
        Kind::Preset => "source-preset",
        Kind::Bank => "source-bank",
        Kind::Hold => "source-hold",
        Kind::Invoice => "source-invoice",
        Kind::Letter => "source-letter",
        Kind::Mail => "source-mail",
        Kind::Contract => "source-contract",
        Kind::SetAside => "source-set-aside",
    };
    tr.text(id, None)
}

/// What opening a source opens: the mail, the letter, the contract, the
/// invoice, the budget's line; nothing for the bank's line or an occurrence.
fn source_address(source: &crate::movements::Source) -> String {
    if !source.address.is_empty() {
        return source.address.clone();
    }
    match source.kind {
        Kind::Mail | Kind::Letter | Kind::Contract | Kind::Invoice => source.key.clone(),
        _ => String::new(),
    }
}

/// One row of What is coming.
pub fn coming_row(ledger: &Ledger, m: &Movement, doubtful: &BTreeSet<String>, tr: &Translator, today: Date) -> ComingRow {
    let budgets: Vec<&str> = m.place.budgets();
    let way = match m.way {
        Way::In => "in",
        Way::Out => "out",
        Way::Between => "between",
    };
    let mut args = i18n::args();
    args.set("day", tr.day_in(m.day, today));
    let state = match m.state {
        State::Announced => tr.text("coming-announced", Some(&args)),
        State::NotFinal => tr.text("coming-not-final", None),
        State::NotSeen => tr.text("coming-not-seen", Some(&args)),
        State::Planned if doubtful.contains(&m.address()) => tr.text("coming-if", Some(&args)),
        State::Planned if m.day < today => tr.text("coming-not-seen", Some(&args)),
        State::Planned => tr.text("coming-planned", Some(&args)),
        State::Done => tr.text(if m.way == Way::In { "coming-arrived" } else { "coming-left" }, Some(&args)),
    };
    let mut seen = BTreeSet::new();
    let sources: Vec<SourceView> = m
        .sources
        .iter()
        .filter(|s| seen.insert((s.kind, s.key.clone())))
        .map(|s| SourceView { kind: s.kind, words: source_words(tr, s.kind), address: source_address(s), probable: s.probable, key: s.key.clone() })
        .collect();
    let mut words: Vec<String> = Vec::new();
    for s in &sources {
        if !words.contains(&s.words) {
            words.push(s.words.clone());
        }
    }
    let mut from = i18n::args();
    from.set("list", ahead::list(tr, &words));
    let who = who(tr, m);
    ComingRow {
        address: m.address(),
        key: m.steady_key().to_string(),
        account: m.account.clone().unwrap_or_default(),
        day: m.day.to_string(),
        date: short_day(tr, m.day),
        who,
        amount: sum_words(tr, m.sum),
        cents: m.sum.amount().cents(),
        way,
        way_words: tr.text(&format!("coming-way-{way}"), None),
        budget: budgets.iter().map(|b| ledger.budget_title(b).to_string()).collect::<Vec<_>>().join(", "),
        budget_id: budgets.first().map(|b| b.to_string()).unwrap_or_default(),
        state,
        probable: sources.iter().any(|s| s.probable),
        sources,
        from: tr.text("coming-from", Some(&from)),
    }
}

/// What is coming (research §6, recommendation 3): every movement expected
/// from today over six weeks, or to the income after next when that is
/// further (75 days at most), in date order and grouped by week; what the
/// last week's exports did not show yet first. `shown` keeps the rows of the
/// areas in view; `exported` says whether an account has exports.
pub fn coming(ledger: &Ledger, movements: &[Movement], aheads: &[Ahead], tr: &Translator, today: Date, shown: &dyn Fn(&Movement) -> bool, exported: &dyn Fn(Option<&str>) -> bool) -> Vec<WeekView> {
    let shifted = |d: Date, n: i64| d.checked_add(jiff::Span::new().days(n)).unwrap_or(d);
    let further = aheads.iter().filter_map(|a| a.after.as_ref().map(|i| i.day)).max().unwrap_or(today);
    let horizon = shifted(today, ahead::COMING_DAYS).max(further).min(shifted(today, ahead::COMING_MOST));
    let week_ago = shifted(today, -7);
    let doubtful: BTreeSet<String> = aheads.iter().flat_map(|a| a.income.iter().chain(a.after.iter())).filter(|i| !i.certain).map(|i| i.address.clone()).collect();
    let mut rows: Vec<&Movement> = movements
        .iter()
        .filter(|m| match m.state {
            // A day passed: still awaited only where its account's exports could show it; without any, it counts as done.
            State::Planned | State::Announced => (m.day >= today || exported(m.account.as_deref())) && m.day >= week_ago && m.day <= horizon,
            State::NotFinal => m.day >= week_ago,
            State::NotSeen => m.day >= week_ago,
            State::Done => false,
        })
        .filter(|m| shown(m))
        .collect();
    rows.sort_by(|a, b| (a.day, &a.label).cmp(&(b.day, &b.label)));
    let monday = |d: Date| shifted(d, -i64::from(d.weekday().to_monday_zero_offset()));
    let this_week = monday(today);
    let mut weeks: Vec<WeekView> = Vec::new();
    let mut current: Option<Date> = None;
    for m in rows {
        let week = monday(m.day).max(this_week);
        if current != Some(week) {
            current = Some(week);
            let title = if week == this_week {
                tr.text("coming-this-week", None)
            } else if week == shifted(this_week, 7) {
                tr.text("coming-next-week", None)
            } else {
                let mut args = i18n::args();
                args.set("day", tr.day_in(week, today));
                tr.text("coming-week-of", Some(&args))
            };
            weeks.push(WeekView { title, rows: Vec::new() });
        }
        if let Some(last) = weeks.last_mut() {
            last.rows.push(coming_row(ledger, m, &doubtful, tr, today));
        }
    }
    weeks
}

/// The past year's unusual spending, as What is coming says it after its rows.
pub fn unusual_line(aheads: &[Ahead], tr: &Translator) -> String {
    let monthly: i64 = aheads.iter().map(|a| a.unusual.cents()).sum();
    if monthly <= 0 {
        return String::new();
    }
    let mut args = i18n::args();
    args.set("amount", tr.money(ahead::about(Money(monthly))));
    tr.text("coming-unusual", Some(&args))
}

/// Each account shown, in sentences; one hidden by the hour or your ticks
/// shows all the same when it would not hold a payment.
pub fn accounts(aheads: &[Ahead], shown: Area, tr: &Translator, today: Date) -> Vec<AccountSay> {
    aheads
        .iter()
        .filter(|a| a.area.meets(shown) || a.short.is_some())
        .map(|a| AccountSay { id: a.account.clone(), title: if a.title.is_empty() { tr.text("ahead-your-account", None) } else { a.title.clone() }, sentences: ahead::sentences(a, tr, today), short: a.short.is_some() })
        .collect()
}

/// The page's one chart: the first account shown that has something coming.
pub fn chart(aheads: &[Ahead], shown: Area, tr: &Translator, today: Date) -> Option<ChartView> {
    let a = aheads.iter().find(|a| (a.area.meets(shown) || a.short.is_some()) && !a.flows.is_empty())?;
    let chart = ahead::chart(a, today);
    let mut args = i18n::args();
    args.set("day", tr.day_in(chart.to.parse().unwrap_or(today), today));
    Some(ChartView { account: a.account.clone(), name: tr.text("ahead-chart-name", Some(&args)), floor_label: tr.text("ahead-chart-floor", None), chart })
}

/// What the person spends a week, all accounts together (their payments
/// out of the past thirteen weeks, moves between accounts and savings
/// aside): reserves are said in weeks of it (research §6, recommendation 10).
pub fn own_pace(movements: &[Movement], today: Date) -> Money {
    let from = today.checked_sub(jiff::Span::new().days(91)).unwrap_or(today);
    let out: Vec<&Movement> = movements.iter().filter(|m| m.state == State::Done && m.day >= from && m.day < today && m.way == Way::Out && !matches!(m.place, Place::Transfer { .. } | Place::Reserve { .. })).collect();
    let Some(first) = out.iter().map(|m| m.day).min() else { return Money::ZERO };
    let weeks = (f64::from((today - first).get_days()) / 7.0).clamp(1.0, 13.0);
    Money((-out.iter().map(|m| m.sum.amount().cents()).sum::<i64>() as f64 / weeks).round() as i64)
}

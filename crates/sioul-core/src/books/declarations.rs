// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The declarations of turnover a regime asks (France: the micro regime's,
//! to URSSAF; autoentrepreneur.urssaf.fr, research-fr §7): per period,
//! monthly or quarterly as set, and per activity (sales of goods, BIC
//! services, BNC, CIPAV), the turnover received excluding VAT; the
//! contributions estimated at the preset's rates of the days it was
//! received (social contributions, the versement libératoire when chosen,
//! the training contribution by trade); the day to declare by: the last day
//! of the month after the period, moved off a weekend. Zero periods are
//! declared too, and said. The first declaration gathers the periods from
//! the start of activity until a due day at least 90 days after it.
//!
//! An estimate: URSSAF counts, Sioul prepares. ACRE and the chambers' tax
//! are not in the preset; a rate of your own (`[[invoice.contribution]]`)
//! stands for ACRE's.

use super::{Every, Input, Row, Settings, days_between, long_day, next_working, period_of, shift_days, shift_months};
use crate::i18n::Translator;
use crate::money::Money;
use jiff::civil::Date;
use serde::Serialize;

/// An amount at a rate (basis points).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Part {
    pub rate: i64,
    pub amount: i64,
}

/// One activity's line of a declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub activity: String,
    /// As the preset names it, in the language asked.
    pub name: String,
    /// Received excluding VAT.
    pub turnover: i64,
    pub social: Option<Part>,
    /// The versement libératoire, when chosen.
    pub vl: Option<Part>,
}

/// One declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Declaration {
    pub every: Every,
    pub from: Date,
    pub to: Date,
    /// The day it can be made: the day after its period.
    pub opens: Date,
    /// The day to declare by.
    pub due: Date,
    /// The first one, gathering the periods since the start of activity.
    pub first: bool,
    /// One per activity said, zero ones too.
    pub lines: Vec<Line>,
    /// Received excluding VAT, in all.
    pub turnover: i64,
    /// Received in no activity said: counted in `turnover`, no contribution estimated for it.
    pub unsaid: i64,
    /// The training contribution, on the whole, at the trade's rate; none when the trade is not known.
    pub cfp: Option<Part>,
    /// Every contribution estimated, added.
    pub contributions: i64,
    /// A receipt's amount not known: at least this much.
    pub at_least: bool,
    /// Payments in another currency whose amount here is not said: not counted.
    pub foreign: usize,
}

impl Declaration {
    /// Its key, the same on every device: "2026-Q4", "2026-10", the first one's by its days.
    pub fn key(&self) -> String {
        if self.first {
            return format!("{}..{}", self.from, self.to);
        }
        match self.every {
            Every::Month => format!("{}-{:02}", self.from.year(), self.from.month()),
            Every::Quarter => format!("{}-Q{}", self.from.year(), super::quarter_of(self.from)),
        }
    }
}

/// The last day of the month `months_after` months after `to`'s, before
/// it is moved off a weekend.
fn regular_due(to: Date, months_after: u32) -> Date {
    shift_months(to.first_of_month(), i64::from(months_after)).last_of_month()
}

/// The day a period ending `to` is declared by: the last day of the month
/// after it (as the preset says), the next working day when it is not one.
pub fn due_of(settings: &Settings, to: Date) -> Date {
    let months = settings.preset.declaration.as_ref().map_or(1, |d| d.months_after);
    next_working(regular_due(to, months))
}

/// The periods declared, from the first that holds `from`'s days to the
/// one that holds `until`: each period's days, and whether it is the first
/// declaration. With the start of activity said, nothing before it, and
/// the first declaration gathers the periods until a due day at least the
/// preset's days after the end of the start's period (URSSAF: a monthly
/// start in January, due 31 May; a quarterly start in the first quarter,
/// due 31 July).
pub fn schedule(settings: &Settings, from: Date, until: Date) -> Vec<(Date, Date, bool)> {
    let every = settings.every;
    let rule = settings.preset.declaration.as_ref();
    let months_after = rule.map_or(1, |d| d.months_after);
    let mut out = Vec::new();
    let mut start = period_of(every, from).0;
    if let Some(started) = settings.started {
        if until < started {
            return out;
        }
        let (p0, p0_end) = period_of(every, started);
        let reach = shift_days(p0_end, rule.map_or(0, |d| d.first_after_days));
        let mut end = p0_end;
        while regular_due(end, months_after) < reach {
            end = period_of(every, shift_days(end, 1)).1;
        }
        // The first declaration of one period only, begun on its first day, is a period as any.
        if start <= end {
            out.push((started, end, p0 != started || end != p0_end));
        }
        start = start.max(shift_days(end, 1));
    }
    let mut p = start;
    while p <= until {
        let (a, b) = period_of(every, p);
        out.push((a, b, false));
        p = shift_days(b, 1);
    }
    out
}

/// The declaration of the days `from` to `to` (a period, or the first one).
pub fn of_days(input: &Input, from: Date, to: Date, first: bool, tr: &Translator) -> Declaration {
    of_rows(input.settings, &super::rows(input, from, to, tr), from, to, first, tr)
}

/// The declaration of the days `from` to `to`, from the receipts book's
/// `rows` (those of other days left aside): an agent's are those open to it.
pub fn of_rows(settings: &Settings, rows: &[Row], from: Date, to: Date, first: bool, tr: &Translator) -> Declaration {
    let rows: Vec<&Row> = rows.iter().filter(|r| r.day >= from && r.day <= to).collect();
    let mut lines: Vec<Line> = settings.activities.iter().map(|a| Line { activity: a.clone(), name: settings.activity_name(a, tr.language()), turnover: 0, social: None, vl: None }).collect();
    let mut social: Vec<i64> = vec![0; lines.len()];
    let mut vl: Vec<i64> = vec![0; lines.len()];
    let mut unsaid = 0;
    let mut cfp = 0;
    let rate = |kind: &str, activity: &str, day: Date| settings.rate_on(kind, activity, day);
    let at = |amount: i64, bp: i64| i64::try_from(crate::invoice::record::rounded(i128::from(amount) * i128::from(bp), 10_000)).unwrap_or(0);
    for r in rows.iter().copied() {
        let Some(without) = r.without_tax else { continue };
        // Its parts: the goods', the services'; a receipt no invoice describes, all in the first activity said.
        let parts: Vec<(Option<&str>, i64)> = if r.nature.is_empty() { vec![(settings.activity_unsaid(), without)] } else { vec![(settings.activity_for("goods"), r.goods), (settings.activity_for("service"), without - r.goods)] };
        for (activity, amount) in parts.into_iter().filter(|(_, a)| *a != 0) {
            let Some(i) = activity.and_then(|a| lines.iter().position(|l| l.activity == a)) else {
                unsaid += amount;
                continue;
            };
            lines[i].turnover += amount;
            social[i] += rate("social", &lines[i].activity, r.day).map_or(0, |bp| at(amount, bp));
            if settings.vl {
                vl[i] += rate("vl", &lines[i].activity, r.day).map_or(0, |bp| at(amount, bp));
            }
        }
        if !settings.trade.is_empty() {
            cfp += rate("cfp", &settings.trade, r.day).map_or(0, |bp| at(without, bp));
        }
    }
    for (i, line) in lines.iter_mut().enumerate() {
        line.social = rate("social", &line.activity, to).map(|bp| Part { rate: bp, amount: social[i] });
        line.vl = if settings.vl { rate("vl", &line.activity, to).map(|bp| Part { rate: bp, amount: vl[i] }) } else { None };
    }
    let cfp = if settings.trade.is_empty() { None } else { rate("cfp", &settings.trade, to).map(|bp| Part { rate: bp, amount: cfp }) };
    let turnover = lines.iter().map(|l| l.turnover).sum::<i64>() + unsaid;
    let contributions = lines.iter().map(|l| l.social.map_or(0, |p| p.amount) + l.vl.map_or(0, |p| p.amount)).sum::<i64>() + cfp.map_or(0, |p| p.amount);
    Declaration {
        every: settings.every,
        from,
        to,
        opens: shift_days(to, 1),
        due: due_of(settings, to),
        first,
        lines,
        turnover,
        unsaid,
        cfp,
        contributions,
        at_least: rows.iter().any(|r| r.foreign.is_none() && r.without_tax.is_none()),
        foreign: rows.iter().filter(|r| r.foreign.is_some() && r.without_tax.is_none()).count(),
    }
}

/// The declarations of `periods` (`schedule`'s), the receipts book read once for them all.
fn of_periods(input: &Input, periods: &[(Date, Date, bool)], tr: &Translator) -> Vec<Declaration> {
    let (Some(first), Some(last)) = (periods.iter().map(|p| p.0).min(), periods.iter().map(|p| p.1).max()) else { return Vec::new() };
    let rows = super::rows(input, first, last, tr);
    periods.iter().map(|(a, b, first)| of_rows(input.settings, &rows, *a, *b, *first, tr)).collect()
}

/// The declarations whose days touch `from` to `until` (both counted), the
/// oldest first, each whole; none when the preset asks none under the regime.
pub fn between(input: &Input, from: Date, until: Date, tr: &Translator) -> Vec<Declaration> {
    if !input.settings.declares() {
        return Vec::new();
    }
    let periods: Vec<(Date, Date, bool)> = schedule(input.settings, from, until).into_iter().filter(|(a, b, _)| *b >= from && *a <= until).collect();
    of_periods(input, &periods, tr)
}

/// The periods of `year` until the one that holds `today`.
pub fn periods_of_year(settings: &Settings, year: i16, today: Date) -> Vec<(Date, Date, bool)> {
    let (first, last) = (Date::new(year, 1, 1).unwrap_or(today), Date::new(year, 12, 31).unwrap_or(today));
    // From the year before: a first declaration begun then may reach into this year.
    let from = Date::new(year - 1, 1, 1).unwrap_or(first);
    if !settings.declares() {
        return Vec::new();
    }
    schedule(settings, from, last.min(today)).into_iter().filter(|(a, b, _)| *b >= first && *a <= last).collect()
}

/// The declarations of `year` until the period that holds `today`.
pub fn of_year(input: &Input, year: i16, today: Date, tr: &Translator) -> Vec<Declaration> {
    of_periods(input, &periods_of_year(input.settings, year, today), tr)
}

/// The declaration that holds `day`.
pub fn holding(input: &Input, day: Date, tr: &Translator) -> Option<Declaration> {
    between(input, day, day, tr).into_iter().next()
}

/// The declarations to make now, for the Porch: opened by `today` (their
/// period is over), and either still before their day or within `after`
/// days past it. Which ones were said done is the Porch's (its notices' words).
pub fn open_now(input: &Input, today: Date, after: i64, tr: &Translator) -> Vec<Declaration> {
    if !input.settings.declares() {
        return Vec::new();
    }
    let from = Date::new(today.year() - 1, 1, 1).unwrap_or(today);
    let periods: Vec<(Date, Date, bool)> = schedule(input.settings, from, shift_days(today, -1)).into_iter().filter(|(_, b, _)| shift_days(*b, 1) <= today && days_between(due_of(input.settings, *b), today) <= after).collect();
    of_periods(input, &periods, tr)
}

/// The period's name: "October 2026", "Fourth quarter 2026"; the first
/// declaration's, by its days: "15 January 2026 to 30 April 2026".
pub fn period_name(tr: &Translator, d: &Declaration) -> String {
    if d.first {
        let mut args = crate::i18n::args();
        args.set("from", long_day(tr, d.from));
        args.set("to", long_day(tr, d.to));
        return tr.text("books-period-days", Some(&args));
    }
    match d.every {
        Every::Month => tr.month_year(d.from),
        Every::Quarter => {
            let mut args = crate::i18n::args();
            args.set("quarter", i64::from(super::quarter_of(d.from)));
            args.set("year", d.from.year().to_string());
            tr.text("books-total-quarter", Some(&args))
        }
    }
}

/// What a declaration says, sentence by sentence: what was received and the
/// day to declare by ("Nothing received from 1 October to 31 December: declare
/// €0 by 1 February 2027."); each activity's turnover and contributions;
/// the training contribution; the whole estimate; what is not counted, and why.
pub fn sentences(tr: &Translator, d: &Declaration, currency: &str) -> Vec<String> {
    let money = |m: i64| tr.money_in(Money(m), currency);
    let mut args = crate::i18n::args();
    args.set("period", period_name(tr, d));
    args.set("from", long_day(tr, d.from));
    args.set("to", long_day(tr, d.to));
    args.set("due", long_day(tr, d.due));
    args.set("zero", money(0));
    args.set("turnover", money(d.turnover));
    args.set("least", if d.at_least { "yes" } else { "no" });
    let mut out = vec![tr.text(if d.turnover == 0 && !d.at_least { "books-declare-nothing" } else { "books-declare" }, Some(&args))];
    for line in d.lines.iter().filter(|l| l.turnover != 0 || d.lines.len() > 1) {
        let mut a = crate::i18n::args();
        a.set("name", line.name.clone());
        a.set("turnover", money(line.turnover));
        match line.social {
            Some(p) => {
                a.set("social", money(p.amount));
                a.set("rate", tr.percent_bp(p.rate));
                let mut said = tr.text("books-declare-line", Some(&a));
                if let Some(v) = line.vl {
                    let mut b = crate::i18n::args();
                    b.set("vl", money(v.amount));
                    b.set("rate", tr.percent_bp(v.rate));
                    said = format!("{} {}", said, tr.text("books-declare-vl", Some(&b)));
                }
                out.push(said);
            }
            None => out.push(tr.text("books-declare-line-no-rate", Some(&a))),
        }
    }
    if d.turnover != 0 {
        match d.cfp {
            Some(p) => {
                let mut a = crate::i18n::args();
                a.set("cfp", money(p.amount));
                a.set("rate", tr.percent_bp(p.rate));
                out.push(tr.text("books-declare-cfp", Some(&a)));
            }
            None => out.push(tr.text("books-declare-cfp-unsaid", None)),
        }
    }
    if d.unsaid != 0 {
        let mut a = crate::i18n::args();
        a.set("amount", money(d.unsaid));
        out.push(tr.text("books-declare-unsaid", Some(&a)));
    }
    if d.contributions != 0 {
        let mut a = crate::i18n::args();
        a.set("total", money(d.contributions));
        out.push(tr.text("books-declare-estimate", Some(&a)));
    }
    if d.foreign > 0 {
        let mut a = crate::i18n::args();
        a.set("n", d.foreign as i64);
        out.push(tr.text("books-declare-foreign", Some(&a)));
    }
    out
}

/// The budgets' Overview line beside a budget that declares, from the books
/// (so that it says what the declaration view says): "Turnover received this
/// quarter, excluding VAT: €6,940, to declare by 1 February 2027."; none
/// when the preset asks no declaration under the regime.
pub fn overview_line(input: &Input, today: Date, tr: &Translator) -> Option<String> {
    let d = holding(input, today, tr)?;
    let mut args = crate::i18n::args();
    args.set("every", d.every.id());
    args.set("turnover", tr.money_in(Money(d.turnover), &input.settings.currency));
    args.set("least", if d.at_least { "yes" } else { "no" });
    args.set("due", long_day(tr, d.due));
    Some(tr.text("books-overview", Some(&args)))
}

/// One line for the Porch's row: "Contributions estimated: €1,777."; "" when there is none to say.
pub fn estimate_line(tr: &Translator, d: &Declaration, currency: &str) -> String {
    if d.contributions == 0 {
        return String::new();
    }
    let mut a = crate::i18n::args();
    a.set("total", tr.money_in(Money(d.contributions), currency));
    tr.text("books-declare-estimate", Some(&a))
}

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
//! declared too, and said; periods before the books begin (the start of
//! activity, else the first thing Sioul knows of) say nothing. The first
//! declaration gathers the start's period and the ones after it, as many
//! as the preset says (France: monthly, the start month and the three
//! after; quarterly, the start quarter and the next: URSSAF's tables,
//! justice.fr, propulsebyca.fr). A period with more given back than
//! received declares nothing below zero, and says so.
//!
//! Money paid back on a credit note (a refund) is counted in the period its
//! money was received in while that declaration is not made (neither said
//! "Declared" nor past its day); after, it lowers no period, and the
//! declaration of its own period says to correct the earlier one, as
//! URSSAF's sheet "Déterminer mon chiffre d'affaires" says (read only as a
//! search snippet: docs/research/money-ledger.md, for a person to confirm).
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
    /// Received excluding VAT, in all (below zero when more was given back).
    pub turnover: i64,
    /// What is declared: each activity's turnover, none below zero.
    pub declared: i64,
    /// Received in no activity said: counted in `turnover`, no contribution estimated for it.
    pub unsaid: i64,
    /// The training contribution, on the whole, at the trade's rate; none when the trade is not known.
    pub cfp: Option<Part>,
    /// Every contribution estimated, added.
    pub contributions: i64,
    /// A receipt's amount not known: at least this much.
    pub at_least: bool,
    /// Payments in another currency whose amount here is not said, not
    /// counted: what each received, in its currency. The period is never
    /// said "nothing received" while one is.
    pub foreign: Vec<(i64, String)>,
    /// Refunds of this period for money of a period declared already:
    /// counted in neither; that declaration is to be corrected.
    pub corrections: Vec<Correction>,
}

/// A refund for money received in a period declared already: the
/// declaration to correct, the amount given back excluding VAT, its day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Correction {
    pub every: Every,
    pub from: Date,
    pub to: Date,
    pub first: bool,
    /// Given back, excluding VAT (above zero).
    pub amount: i64,
    pub day: Date,
}

/// Where the declarations count a refund (`place_refund`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    /// On its own day: its money came in the same period, or no period holds it.
    Here,
    /// On this day, the one its money came in: that period is not declared yet.
    In(Date),
    /// In none: the declaration of these days (the first or not) was made, and is to be corrected.
    Correct(Date, Date, bool),
}

/// Where a refund given on `day` of money received on `received` is
/// counted: in its own period when the money came in it; in the period the
/// money came in while that declaration is not made (not said "Declared"
/// by `day`, nor past its day to declare by); else in none, and the
/// declaration of that period is to be corrected (URSSAF's sheet
/// "Déterminer mon chiffre d'affaires", read as a search snippet). `begin`
/// is the books' first day (`books::begin`).
pub fn place_refund(settings: &Settings, declared: &std::collections::BTreeMap<String, Date>, begin: Option<Date>, received: Date, day: Date) -> Placed {
    if !settings.declares() || received >= day {
        return Placed::Here;
    }
    let Some(begin) = begin else { return Placed::Here };
    let Some((from, to, first)) = schedule(settings, received.max(begin), received).into_iter().find(|(a, b, _)| *a <= received && received <= *b) else { return Placed::Here };
    if day <= to {
        return Placed::Here;
    }
    let said = declared.get(&format!("declaration:{}", key_of(settings.every, from, to, first))).is_some_and(|on| *on <= day);
    if said || day > due_of(settings, to) { Placed::Correct(from, to, first) } else { Placed::In(received) }
}

/// A declaration's key from its days, the same on every device: "2026-Q4",
/// "2026-10", the first one's by its days.
pub fn key_of(every: Every, from: Date, to: Date, first: bool) -> String {
    if first {
        return format!("{from}..{to}");
    }
    match every {
        Every::Month => format!("{}-{:02}", from.year(), from.month()),
        Every::Quarter => format!("{}-Q{}", from.year(), super::quarter_of(from)),
    }
}

impl Declaration {
    /// Whether what it says is a least: a receipt not known, or one in another currency not said in euros.
    pub fn least(&self) -> bool {
        self.at_least || !self.foreign.is_empty()
    }
}

impl Declaration {
    /// Its key, the same on every device: "2026-Q4", "2026-10", the first one's by its days.
    pub fn key(&self) -> String {
        key_of(self.every, self.from, self.to, self.first)
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
/// the first declaration gathers the start's period and the ones after it,
/// as many as the preset says (`first_periods`: France's monthly, the
/// start month and the three after, due the last day of the fourth month
/// after the start month; quarterly, the start quarter and the next).
pub fn schedule(settings: &Settings, from: Date, until: Date) -> Vec<(Date, Date, bool)> {
    let every = settings.every;
    let rule = settings.preset.declaration.as_ref();
    let mut out = Vec::new();
    let mut start = period_of(every, from).0;
    if let Some(started) = settings.started {
        if until < started {
            return out;
        }
        let (p0, p0_end) = period_of(every, started);
        let gathered = rule.map_or(1, |d| match every {
            Every::Month => d.first_periods.month,
            Every::Quarter => d.first_periods.quarter,
        });
        let mut end = p0_end;
        for _ in 1..gathered.max(1) {
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
    // Each row on the day it is counted (a refund's may be its money's, or none: `place_refund`).
    let corrections: Vec<Correction> = rows
        .iter()
        .filter(|r| r.counted_on.is_none() && r.day >= from && r.day <= to)
        .filter_map(|r| r.corrects.map(|(a, b, f)| Correction { every: settings.every, from: a, to: b, first: f, amount: -r.without_tax.unwrap_or(0), day: r.day }))
        .collect();
    let rows: Vec<&Row> = rows.iter().filter(|r| r.counted_on.is_some_and(|d| d >= from && d <= to)).collect();
    let mut lines: Vec<Line> = settings.activities.iter().map(|a| Line { activity: a.clone(), name: settings.activity_name(a, tr.language()), turnover: 0, social: None, vl: None }).collect();
    let mut social: Vec<i64> = vec![0; lines.len()];
    let mut vl: Vec<i64> = vec![0; lines.len()];
    let mut unsaid = 0;
    let mut cfp = 0;
    let rate = |kind: &str, activity: &str, day: Date| settings.rate_on(kind, activity, day);
    let at = |amount: i64, bp: i64| i64::try_from(crate::invoice::record::rounded(i128::from(amount) * i128::from(bp), 10_000)).unwrap_or(0);
    for r in rows.iter().copied() {
        let Some(without) = r.without_tax else { continue };
        // Its parts by the activity of each line (`Row::activities`): a meal
        // eaten on the spot under sales, whatever its nature; a receipt no
        // invoice describes in the first activity said.
        for (activity, _, amount) in r.activities.iter().filter(|(_, _, a)| *a != 0) {
            let amount = *amount;
            let Some(i) = lines.iter().position(|l| l.activity == *activity) else {
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
    // Nothing below zero is declared: a line with more given back than
    // received declares nothing, and estimates no contribution.
    for line in lines.iter_mut().filter(|l| l.turnover <= 0) {
        line.social = line.social.map(|p| Part { amount: 0, ..p });
        line.vl = line.vl.map(|p| Part { amount: 0, ..p });
    }
    let turnover = lines.iter().map(|l| l.turnover).sum::<i64>() + unsaid;
    let declared = lines.iter().map(|l| l.turnover.max(0)).sum::<i64>() + unsaid.max(0);
    // The training contribution on what is declared: each payment's day's rate, or, a line below zero, the rate of the period's end.
    let cfp = if settings.trade.is_empty() { None } else { rate("cfp", &settings.trade, to).map(|bp| Part { rate: bp, amount: if turnover == declared { cfp } else { at(declared, bp) } }) };
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
        declared,
        unsaid,
        cfp,
        contributions,
        at_least: rows.iter().any(|r| r.foreign.is_none() && r.without_tax.is_none()),
        foreign: rows.iter().filter(|r| r.without_tax.is_none()).filter_map(|r| r.foreign.clone()).collect(),
        corrections,
    }
}

/// What came in other currencies and is not said in euros, by currency:
/// "$1,000", "$1,000 and CHF 200".
pub fn foreign_words(tr: &Translator, foreign: &[(i64, String)]) -> String {
    let mut by: std::collections::BTreeMap<&str, i64> = std::collections::BTreeMap::new();
    for (amount, code) in foreign {
        *by.entry(code.as_str()).or_default() += amount;
    }
    let said: Vec<String> = by.into_iter().map(|(code, amount)| tr.money_in(Money(amount), code)).collect();
    let mut args = crate::i18n::args();
    args.set("list", said.join(", "));
    args.set("n", said.len() as i64);
    tr.text("books-foreign-amounts", Some(&args))
}

/// The declarations of `periods` (`schedule`'s), the receipts book read once for them all.
fn of_periods(input: &Input, periods: &[(Date, Date, bool)], tr: &Translator) -> Vec<Declaration> {
    let (Some(first), Some(last)) = (periods.iter().map(|p| p.0).min(), periods.iter().map(|p| p.1).max()) else { return Vec::new() };
    // Up to the last period's day to declare by: a refund made before it,
    // of money of that period, is counted in it (`place_refund`).
    let rows = super::rows(input, first, due_of(input.settings, last).max(last), tr);
    periods.iter().map(|(a, b, first)| of_rows(input.settings, &rows, *a, *b, *first, tr)).collect()
}

/// The periods whose days touch `from` to `until`, each whole, none
/// before the books begin (`books::begin`: the start of activity, else the
/// first thing Sioul knows of) and none when the preset asks no declaration
/// under the regime: a period Sioul knew nothing of says nothing.
pub fn periods(input: &Input, from: Date, until: Date) -> Vec<(Date, Date, bool)> {
    if !input.settings.declares() {
        return Vec::new();
    }
    let Some(begin) = super::begin(input) else { return Vec::new() };
    schedule(input.settings, from.max(begin), until).into_iter().filter(|(a, b, _)| *b >= from && *a <= until).collect()
}

/// The declarations whose days touch `from` to `until` (both counted), the
/// oldest first, each whole (`periods`).
pub fn between(input: &Input, from: Date, until: Date, tr: &Translator) -> Vec<Declaration> {
    of_periods(input, &periods(input, from, until), tr)
}

/// The periods of `year` until the one that holds `today`; a first
/// declaration begun the year before is whole.
pub fn periods_of_year(input: &Input, year: i16, today: Date) -> Vec<(Date, Date, bool)> {
    let (first, last) = (Date::new(year, 1, 1).unwrap_or(today), Date::new(year, 12, 31).unwrap_or(today));
    periods(input, first, last.min(today))
}

/// The declarations of `year` until the period that holds `today`.
pub fn of_year(input: &Input, year: i16, today: Date, tr: &Translator) -> Vec<Declaration> {
    of_periods(input, &periods_of_year(input, year, today), tr)
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
    let open: Vec<(Date, Date, bool)> = periods(input, from, shift_days(today, -1)).into_iter().filter(|(_, b, _)| shift_days(*b, 1) <= today && days_between(due_of(input.settings, *b), today) <= after).collect();
    of_periods(input, &open, tr)
}

/// The period's name: "October 2026", "Fourth quarter 2026"; the first
/// declaration's, by its days: "15 January 2026 to 30 April 2026".
pub fn period_name(tr: &Translator, d: &Declaration) -> String {
    period_words(tr, d.every, d.from, d.to, d.first)
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
    args.set("turnover", money(d.declared));
    args.set("least", if d.least() { "yes" } else { "no" });
    args.set("beyond", money(d.declared - d.turnover));
    args.set("foreign", foreign_words(tr, &d.foreign));
    // Never "nothing received" while a payment in another currency waits for its euros.
    let head = if !d.foreign.is_empty() && d.declared == 0 && !d.at_least {
        "books-declare-foreign-only"
    } else if d.declared != d.turnover {
        "books-declare-beyond"
    } else if d.turnover == 0 && !d.least() {
        "books-declare-nothing"
    } else {
        "books-declare"
    };
    let mut out = vec![tr.text(head, Some(&args))];
    for line in d.lines.iter().filter(|l| l.turnover != 0 || d.lines.len() > 1) {
        let mut a = crate::i18n::args();
        a.set("name", line.name.clone());
        a.set("turnover", money(line.turnover));
        a.set("zero", money(0));
        if line.turnover < 0 {
            out.push(tr.text("books-declare-line-below", Some(&a)));
            continue;
        }
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
    if !d.foreign.is_empty() && head != "books-declare-foreign-only" {
        let mut a = crate::i18n::args();
        a.set("foreign", foreign_words(tr, &d.foreign));
        out.push(tr.text("books-declare-foreign", Some(&a)));
    }
    out.extend(d.corrections.iter().map(|c| correction_sentence(tr, c, currency)));
    out
}

/// A refund of money of a period declared already: "URSSAF's sheet says to
/// correct the declaration of the third quarter 2026: €400 given back on 3
/// November 2026, excluding VAT, counted in no period here."
pub fn correction_sentence(tr: &Translator, c: &Correction, currency: &str) -> String {
    let mut a = crate::i18n::args();
    a.set("period", period_words(tr, c.every, c.from, c.to, c.first));
    a.set("amount", tr.money_in(Money(c.amount), currency));
    a.set("day", long_day(tr, c.day));
    tr.text("books-declare-correct", Some(&a))
}

/// A period's name from its days: "October 2026", "Fourth quarter 2026",
/// the first declaration's by its days.
pub fn period_words(tr: &Translator, every: Every, from: Date, to: Date, first: bool) -> String {
    if first {
        let mut args = crate::i18n::args();
        args.set("from", long_day(tr, from));
        args.set("to", long_day(tr, to));
        return tr.text("books-period-days", Some(&args));
    }
    match every {
        Every::Month => tr.month_year(from),
        Every::Quarter => {
            let mut args = crate::i18n::args();
            args.set("quarter", i64::from(super::quarter_of(from)));
            args.set("year", from.year().to_string());
            tr.text("books-total-quarter", Some(&args))
        }
    }
}

/// The budgets' Overview line beside a budget that declares, from the books
/// (so that it says what the declaration view says): "Turnover received this
/// quarter, excluding VAT: €6,940, to declare by 1 February 2027."; none
/// when the preset asks no declaration under the regime.
pub fn overview_line(input: &Input, today: Date, tr: &Translator) -> Option<String> {
    let d = holding(input, today, tr)?;
    let mut args = crate::i18n::args();
    args.set("every", d.every.id());
    args.set("turnover", tr.money_in(Money(d.declared), &input.settings.currency));
    args.set("least", if d.least() { "yes" } else { "no" });
    args.set("due", long_day(tr, d.due));
    let said = tr.text("books-overview", Some(&args));
    Some(if d.foreign.is_empty() { said } else { format!("{said} {}", foreign_sentence(tr, &d)) })
}

/// What a period's payments in another currency ask: "$1,000 received: say what it came to in euros."
pub fn foreign_sentence(tr: &Translator, d: &Declaration) -> String {
    let mut a = crate::i18n::args();
    a.set("foreign", foreign_words(tr, &d.foreign));
    tr.text("books-declare-foreign", Some(&a))
}

/// One line for the Porch's row: what waits to be said (a payment in
/// another currency), else "Contributions estimated: €1,777."; a period
/// with nothing received, its sentence; "" when there is none to say.
pub fn porch_note(tr: &Translator, d: &Declaration, currency: &str) -> String {
    if !d.foreign.is_empty() {
        return foreign_sentence(tr, d);
    }
    if d.turnover == 0 && !d.at_least {
        return sentences(tr, d, currency).swap_remove(0);
    }
    if d.contributions == 0 {
        return String::new();
    }
    let mut a = crate::i18n::args();
    a.set("total", tr.money_in(Money(d.contributions), currency));
    tr.text("books-declare-estimate", Some(&a))
}

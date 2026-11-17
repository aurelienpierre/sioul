// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The thresholds watched (research-fr §8; design §6), as each preset gives
//! them, dated, each overridable (`[[invoice.threshold]]`):
//!
//! - **The VAT franchise's**, only while the franchise holds: the turnover
//!   of the calendar year as the franchise counts it, excluding VAT,
//!   operations located in the country only (services to businesses of
//!   other member states, the reverse charge, an export, outside the scope
//!   left out), goods on their invoice's day, services on their payment's
//!   (BOI-TVA-DECLA-40-10-10 §180). Its base threshold crossed, VAT applies
//!   from 1 January next year; its higher one crossed, from that very day
//!   (BOI-TVA-DECLA-40-10-20 §160), and invoices to businesses since then
//!   are corrected (§170) where the preset says so.
//! - **The micro regime's ceilings**, only under it: the turnover received
//!   in the calendar year, excluding VAT (the receipts book); two
//!   consecutive years above end the regime on 1 January after the second
//!   (SP F32353).
//! - **The first year**, the thresholds the preset says are prorated: the
//!   amount × the days of activity / 365, to the euro (BOI-TVA-DECLA-40-10-10
//!   §285–295; SP F32353).
//!
//! Only the thresholds a preset marks `watched` are watched: France's,
//! whose counting is researched; Germany's are not, until research says
//! how § 19 UStG counts (the review of phase 4, P4-16).
//!
//! Each step is said once: at 80 % of a base threshold or a ceiling (to
//! know), on crossing it (to know), and on crossing a higher threshold (to
//! do). What takes effect on 1 January is said on the first start of the
//! year (to do): the franchise lost, the micro regime ended; a crossing of
//! last year learned after New Year, at once. The steps told are marked in
//! the invoices' life (`books::mark_told`), whichever device told them:
//! never told again as new, on any device; one to do is told again at each
//! start until it is done, the tax scheme saying VAT from its day or the
//! income regime changed.

use super::{Input, Row, Settings, days_between, long_day, share};
use crate::i18n::Translator;
use crate::invoice::{Invoice, Kind};
use crate::invoice::presets::Threshold;
use jiff::civil::Date;
use serde::Serialize;
use std::collections::BTreeSet;

/// VAT due elsewhere or outside its scope: not located in the country, not counted by the franchise.
const ELSEWHERE: [&str; 4] = ["AE", "O", "K", "G"];

/// Where a threshold stands this year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Watch {
    /// The preset's id ("franchise-services"), and its kind ("micro", "franchise", "franchise-higher").
    pub id: String,
    pub kind: String,
    pub year: i16,
    /// What it counts: "all", "goods", "service".
    pub counts: &'static str,
    /// The amount this year, prorated in the first year when the preset says so.
    pub amount: i64,
    /// Before it was prorated.
    pub full: i64,
    /// The day the activity started, when this year's amount is prorated.
    pub prorated_from: Option<Date>,
    /// Counted so far this year.
    pub turnover: i64,
    /// A receipt's amount is not known: at least this much.
    pub at_least: bool,
    /// The day the turnover reached 80 % of it (a base threshold, a ceiling).
    pub near: Option<Date>,
    /// The day the turnover went above it.
    pub crossed: Option<Date>,
    /// A ceiling of the micro regime: whether last year's turnover was above last year's.
    pub last_year_above: bool,
    /// Crossed, invoices to businesses since then are corrected (France's higher thresholds).
    pub corrective: bool,
}

/// One step of a watch to tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    /// 80 % of a base threshold or a ceiling.
    Near,
    /// A base threshold or a ceiling crossed.
    Crossed,
    /// A higher threshold crossed.
    Higher,
}

impl Step {
    pub fn id(self) -> &'static str {
        match self {
            Step::Near => "near",
            Step::Crossed => "crossed",
            Step::Higher => "higher",
        }
    }
}

/// A step's key, the same on every device: "books:franchise-services:2026:near".
pub fn key(id: &str, year: i16, step: Step) -> String {
    format!("books:{id}:{year}:{}", step.id())
}

/// One amount counted, on its day, of a kind: sales ("goods": goods, food
/// sold or eaten on the spot, lodging) or services, as its activity says
/// (the review of phase 4, P4-13); with no activity said, as its nature.
#[derive(Debug, Clone)]
struct Counted {
    day: Date,
    nature: &'static str,
    amount: i64,
}

/// A row of the receipts book as amounts of each kind (`Row::activities`).
fn of_row(r: &Row) -> Vec<Counted> {
    r.activities.iter().map(|(_, kind, amount)| Counted { day: r.day, nature: kind, amount: *amount }).collect()
}

/// Whether a record's line of `category` is located in the country: not
/// the reverse charge, an intra-EU delivery, an export or outside the
/// scope. A format-1 record (before November 2026) said no category, only
/// its VAT mention in words: its lines are located here, never read as
/// "outside the scope" (the review of phase 4, P4-14).
fn located(invoice: &Invoice, category: &str) -> bool {
    invoice.format == 1 || !ELSEWHERE.contains(&category)
}

/// The turnover as the franchise counts it from `from` to `to`, from the
/// receipts book's own rows, so that a bank line counts as the book counts
/// it (the review of phase 4, P4-14): each row of an invoice, its share of
/// the invoice's lines located here that count on payment (services, a
/// deposit's whole), on the row's day, whether a payment was written or the
/// bank's line tied to the invoice stands for it; each invoice's goods
/// located here on its day (a deposit's on its payments; an invoice in
/// another currency's on its payments, its amount here known then only);
/// the receipts no invoice describes, on their day. A credit note takes its
/// goods off on its day, its services on the days its refund is paid back.
fn franchise_counted(input: &Input, rows: &[Row], from: Date, to: Date) -> Vec<Counted> {
    let settings = input.settings;
    let within = |d: Date| d >= from && d <= to;
    let mut out = Vec::new();
    let ours = |i: &Invoice| i.currency.trim().is_empty() || i.currency.eq_ignore_ascii_case(&settings.currency);
    let invoices: std::collections::BTreeMap<&str, &Invoice> = input.invoices.iter().filter(|i| !i.number.is_empty() && !i.unreadable).map(|i| (i.number.as_str(), i)).collect();
    for invoice in invoices.values().filter(|i| ours(i) && i.kind != Kind::Deposit) {
        let Some(day) = invoice.date.get(..10).and_then(|d| d.parse::<Date>().ok()).filter(|d| within(*d)) else { continue };
        for ((nature, activity, category, _), (base, _)) in super::vat::parts(invoice, input.invoices, settings).iter().filter(|((n, ..), _)| *n == "goods") {
            if located(invoice, category) {
                out.push(Counted { day, nature: settings.kind_of(nature, activity), amount: *base });
            }
        }
    }
    for r in rows.iter().filter(|r| within(r.day)) {
        let Some(without) = r.without_tax else { continue };
        let Some(invoice) = invoices.get(r.invoice.as_str()) else {
            out.extend(of_row(r));
            continue;
        };
        let parts = super::vat::parts(invoice, input.invoices, settings);
        let whole: i64 = parts.values().map(|(base, _)| base).sum();
        if whole == 0 {
            continue;
        }
        let on_payment = |nature: &str| nature == "service" || invoice.kind == Kind::Deposit || !ours(invoice);
        for ((nature, activity, category, _), (base, _)) in parts.iter().filter(|((n, ..), _)| on_payment(n)) {
            if located(invoice, category) {
                out.push(Counted { day: r.day, nature: settings.kind_of(nature, activity), amount: share(*base, without, whole) });
            }
        }
    }
    out
}

/// The receipts book's rows from `from` to `to`, as amounts of each kind.
fn received_counted(rows: &[Row], from: Date, to: Date) -> Vec<Counted> {
    rows.iter().filter(|r| r.day >= from && r.day <= to).flat_map(of_row).collect()
}

/// The natures the issuer sells: its activities'; none said, those its invoices of `year` sell; else services.
fn natures(input: &Input) -> BTreeSet<&'static str> {
    let settings = input.settings;
    let said: BTreeSet<&'static str> = settings.activities.iter().filter_map(|a| settings.nature_of(a)).collect();
    if !said.is_empty() {
        return said;
    }
    let mut seen: BTreeSet<&'static str> = input.invoices.iter().flat_map(|i| i.lines.iter()).map(|l| if l.nature == crate::invoice::Nature::Goods { "goods" } else { "service" }).collect();
    if seen.is_empty() {
        seen.insert("service");
    }
    seen
}

/// The thresholds of `kind` the preset gives on `day`, yours over them.
fn thresholds_on(settings: &Settings, kind: &str, day: Date) -> Vec<Threshold> {
    settings
        .preset
        .thresholds_on(kind, day)
        .into_iter()
        .map(|t| {
            let mut t = t.clone();
            if let Some(own) = settings.thresholds.iter().filter(|o| o.id == t.id && o.from.is_none_or(|f| f <= day)).max_by_key(|o| o.from) {
                t.amount = own.amount;
            }
            t
        })
        .collect()
}

/// A threshold's amount in `year`: prorated to the days of activity when
/// the preset says so and the activity started that year (amount × days /
/// 365, to the euro, never above the whole).
fn amount_in(t: &Threshold, settings: &Settings, year: i16) -> (i64, Option<Date>) {
    match settings.started.filter(|s| s.year() == year && t.prorated) {
        Some(started) => {
            let end = Date::new(year, 12, 31).unwrap_or(started);
            let days = days_between(started, end) + 1;
            let euros = share(days, t.amount, 365 * 100);
            ((euros * 100).min(t.amount), Some(started))
        }
        None => (t.amount, None),
    }
}

/// What a threshold counts: everything, or the nature of its activities.
fn counts_of(t: &Threshold, settings: &Settings) -> &'static str {
    if t.counts == "all" || t.activity.is_empty() {
        return "all";
    }
    let natures: BTreeSet<&'static str> = t.activity.iter().filter_map(|a| settings.nature_of(a)).collect();
    if natures.len() == 1 { natures.into_iter().next().unwrap_or("all") } else { "all" }
}

/// Whether a threshold applies to the issuer: no activity named, or one
/// of the natures it sells.
fn applies(t: &Threshold, settings: &Settings, sold: &BTreeSet<&'static str>) -> bool {
    t.activity.is_empty() || t.activity.iter().filter_map(|a| settings.nature_of(a)).any(|n| sold.contains(n))
}

/// The days the running sum of `counted` (what `counts` counts) first
/// reached `near` and went above `limit`, and the sum at the end.
fn walk(counted: &[Counted], counts: &str, near: i64, limit: i64) -> (i64, Option<Date>, Option<Date>) {
    let mut sorted: Vec<&Counted> = counted.iter().filter(|c| counts == "all" || c.nature == counts).collect();
    sorted.sort_by_key(|c| c.day);
    let (mut sum, mut reached, mut above) = (0, None, None);
    let mut i = 0;
    while i < sorted.len() {
        // A day's amounts together: the day is what crossed.
        let day = sorted[i].day;
        while i < sorted.len() && sorted[i].day == day {
            sum += sorted[i].amount;
            i += 1;
        }
        if reached.is_none() && sum >= near {
            reached = Some(day);
        }
        if above.is_none() && sum > limit {
            above = Some(day);
        }
    }
    (sum, reached, above)
}

/// Where each threshold watched stands in `today`'s year, until `today`
/// (`watch_year`).
pub fn watch(input: &Input, today: Date, tr: &Translator) -> Vec<Watch> {
    watch_year(input, today.year(), today, tr)
}

/// Where each threshold watched stood in `year`, until `until` (the year's
/// last day for a year gone): the franchise's while the franchise held then,
/// the micro regime's ceilings under it. Only the thresholds the preset
/// says are watched (`watched`: France's, researched; the review of phase
/// 4, P4-16). A crossing counts in the year of its day, whatever day it is
/// learned on.
pub fn watch_year(input: &Input, year: i16, until: Date, tr: &Translator) -> Vec<Watch> {
    let settings = input.settings;
    let first = Date::new(year, 1, 1).unwrap_or(until);
    let sold = natures(input);
    let franchise = settings.scheme_on(until) == "franchise";
    let micro = settings.regime == "micro";
    let mut out = Vec::new();
    let watched = |t: &&Threshold| t.watched;
    let want_franchise = franchise && settings.preset.threshold.iter().filter(watched).any(|t| t.kind.starts_with("franchise"));
    let want_micro = micro && settings.preset.threshold.iter().filter(watched).any(|t| t.kind == "micro");
    if !want_franchise && !want_micro {
        return out;
    }
    let last_first = Date::new(year - 1, 1, 1).unwrap_or(first);
    let rows = super::rows(input, last_first, until, tr);
    let at_least = rows.iter().any(|r| r.day >= first && r.without_tax.is_none());
    if want_franchise {
        let counted = franchise_counted(input, &rows, first, until);
        for kind in ["franchise", "franchise-higher"] {
            for t in thresholds_on(settings, kind, until).into_iter().filter(|t| t.watched).filter(|t| t.scheme.is_empty() || t.scheme == "franchise").filter(|t| applies(t, settings, &sold)) {
                let (amount, prorated_from) = amount_in(&t, settings, year);
                let counts = counts_of(&t, settings);
                let near = if kind == "franchise" { share(80, amount, 100) } else { i64::MAX };
                let (turnover, reached, above) = walk(&counted, counts, near, amount);
                out.push(Watch { id: t.id.clone(), kind: kind.into(), year, counts, amount, full: t.amount, prorated_from, turnover, at_least, near: reached, crossed: above, last_year_above: false, corrective: t.corrective });
            }
        }
    }
    if want_micro {
        let counted = received_counted(&rows, first, until);
        let last_end = Date::new(year - 1, 12, 31).unwrap_or(first);
        let before = received_counted(&rows, last_first, last_end);
        for t in thresholds_on(settings, "micro", until).into_iter().filter(|t| t.watched).filter(|t| t.regime.is_empty() || t.regime == settings.regime).filter(|t| applies(t, settings, &sold)) {
            let (amount, prorated_from) = amount_in(&t, settings, year);
            let counts = counts_of(&t, settings);
            let (turnover, reached, above) = walk(&counted, counts, share(80, amount, 100), amount);
            // Last year: above last year's ceiling, unless the activity started this year.
            let last = thresholds_on(settings, "micro", last_end).into_iter().find(|l| l.id == t.id).map(|l| amount_in(&l, settings, year - 1).0);
            let started_now = settings.started.is_some_and(|s| s.year() >= year);
            let last_year_above = !started_now && last.is_some_and(|limit| walk(&before, counts, i64::MAX, limit).2.is_some());
            out.push(Watch { id: t.id.clone(), kind: "micro".into(), year, counts, amount, full: t.amount, prorated_from, turnover, at_least, near: reached, crossed: above, last_year_above, corrective: false });
        }
    }
    out
}

/// What settles a notice to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Settle {
    /// To know: nothing to settle.
    Nothing,
    /// The tax scheme says VAT from that day.
    Scheme(Date),
    /// The income regime is no longer the micro regime.
    Regime,
}

/// A notice of the books to tell: its key (and the earlier steps marked
/// told with it), its words, whether it is to do, its day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notice {
    pub key: String,
    /// Earlier steps of the same watch, marked told with it, never told after it.
    pub also: Vec<String>,
    pub step: Step,
    /// To do: it stays until done (`settle`), and is told again at each
    /// start while it is not (the review of phase 4, P4-4).
    pub actionable: bool,
    pub settle: Settle,
    pub title: String,
    pub body: String,
    /// The day it happened (an effect: the 1 January it takes effect).
    pub day: Date,
}

/// Whether a notice to do is settled: the tax scheme says VAT on its day;
/// the regime is no longer the micro regime.
pub fn settled(settings: &Settings, notice: &Notice) -> bool {
    match notice.settle {
        Settle::Nothing => false,
        Settle::Scheme(day) => settings.scheme_on(day) == "liable",
        Settle::Regime => settings.regime != "micro",
    }
}

/// A threshold's name in words: "the VAT franchise's threshold for services".
pub fn name(tr: &Translator, w: &Watch) -> String {
    let family = if w.kind == "micro" { "micro" } else { "franchise" };
    let higher = if w.kind == "franchise-higher" { "-higher" } else { "" };
    tr.text(&format!("books-threshold-{family}{higher}-{}", w.counts), None)
}

/// Every notice the watches give, told or not, settled or not (`to_tell`,
/// `again` choose): this year's steps, one per watch, its most advanced (a
/// crossing said, its 80 % is never said after it); a higher threshold
/// crossed is said alone, the base's steps of the same count marked with it
/// (the review of phase 4, P4-19); and from last year (`last`, its whole
/// year): what takes effect on this year's 1 January (the franchise lost,
/// the micro regime's end after two years above), and a crossing learned
/// after New Year, said at once (P4-5). While last year's crossing ends the
/// franchise, this year's franchise steps are not said.
pub fn notices(this: &[Watch], last: &[Watch], tr: &Translator, currency: &str, today: Date) -> Vec<Notice> {
    let money = |m: i64| tr.money_in(crate::money::Money(m), currency);
    let mut out = Vec::new();
    let year = today.year();
    let january = Date::new(year, 1, 1).unwrap_or(today);
    let args_of = |w: &Watch, day: Date| {
        let mut args = crate::i18n::args();
        args.set("name", name(tr, w));
        args.set("turnover", money(w.turnover));
        args.set("amount", money(w.amount));
        args.set("day", long_day(tr, day));
        args.set("today", if day == today { "yes" } else { "no" });
        args.set("year", year.to_string());
        args.set("next", (w.year + 1).to_string());
        args.set("last", w.year.to_string());
        args.set("before", (w.year - 1).to_string());
        args.set("corrective", if w.corrective { "yes" } else { "no" });
        args
    };
    let higher_of = |watches: &[Watch], w: &Watch| watches.iter().find(|h| h.kind == "franchise-higher" && h.counts == w.counts && h.year == w.year).cloned();
    // Last year: the effects on this year's 1 January, a crossing learned late.
    let mut ended_franchise = false;
    for w in last {
        match w.kind.as_str() {
            "franchise" if w.crossed.is_some() && higher_of(last, w).is_none_or(|h| h.crossed.is_none()) => {
                ended_franchise = true;
                let args = args_of(w, january);
                let title = if today == january { "books-franchise-effect-title-today" } else { "books-franchise-effect-title" };
                out.push(Notice { key: key(&w.id, w.year, Step::Crossed).replace(":crossed", ":effect"), also: vec![key(&w.id, w.year, Step::Near), key(&w.id, w.year, Step::Crossed)], step: Step::Crossed, actionable: true, settle: Settle::Scheme(january), title: tr.text(title, Some(&args)), body: tr.text("books-franchise-effect", Some(&args)), day: january });
            }
            "franchise-higher" if w.crossed.is_some() => {
                ended_franchise = true;
                let day = w.crossed.unwrap_or(january);
                let args = args_of(w, day);
                let base: Vec<String> = last.iter().filter(|b| b.kind == "franchise" && b.counts == w.counts).flat_map(|b| [key(&b.id, b.year, Step::Near), key(&b.id, b.year, Step::Crossed)]).collect();
                out.push(Notice { key: key(&w.id, w.year, Step::Higher), also: base, step: Step::Higher, actionable: true, settle: Settle::Scheme(day), title: tr.text("books-franchise-higher-title", Some(&args)), body: tr.text("books-franchise-higher", Some(&args)), day });
            }
            "micro" if w.crossed.is_some() && w.last_year_above => {
                let args = args_of(w, january);
                let title = if today == january { "books-micro-effect-title-today" } else { "books-micro-effect-title" };
                out.push(Notice { key: key(&w.id, w.year, Step::Crossed).replace(":crossed", ":effect"), also: vec![key(&w.id, w.year, Step::Near), key(&w.id, w.year, Step::Crossed)], step: Step::Crossed, actionable: true, settle: Settle::Regime, title: tr.text(title, Some(&args)), body: tr.text("books-micro-effect", Some(&args)), day: january });
            }
            "micro" if w.crossed.is_some() => {
                let args = args_of(w, january);
                out.push(Notice { key: key(&w.id, w.year, Step::Crossed), also: vec![key(&w.id, w.year, Step::Near)], step: Step::Crossed, actionable: false, settle: Settle::Nothing, title: tr.text("books-micro-crossed-late-title", Some(&args)), body: tr.text("books-micro-crossed-late", Some(&args)), day: w.crossed.unwrap_or(january) });
            }
            _ => {}
        }
    }
    for w in this {
        if ended_franchise && w.kind.starts_with("franchise") {
            continue;
        }
        let steps: Vec<(Step, Date)> = match w.kind.as_str() {
            "franchise-higher" => w.crossed.map(|d| (Step::Higher, d)).into_iter().collect(),
            _ => [w.near.map(|d| (Step::Near, d)), w.crossed.map(|d| (Step::Crossed, d))].into_iter().flatten().collect(),
        };
        let Some(&(step, day)) = steps.last() else { continue };
        // The higher threshold of the same count crossed: said alone, these steps with it.
        if w.kind == "franchise" && higher_of(this, w).is_some_and(|h| h.crossed.is_some()) {
            continue;
        }
        let mut also: Vec<String> = steps.iter().filter(|(s, _)| *s != step).map(|(s, _)| key(&w.id, w.year, *s)).collect();
        if w.kind == "franchise-higher" {
            also.extend(this.iter().filter(|b| b.kind == "franchise" && b.counts == w.counts).flat_map(|b| [key(&b.id, b.year, Step::Near), key(&b.id, b.year, Step::Crossed)]));
        }
        let args = args_of(w, day);
        let family = if w.kind == "micro" { "micro" } else { "franchise" };
        let (title, body) = match (step, family) {
            (Step::Near, _) => (format!("books-near-{family}-title"), format!("books-near-{family}")),
            (Step::Crossed, "micro") if w.last_year_above => ("books-micro-ends-title".into(), "books-micro-ends".into()),
            (Step::Crossed, "micro") => ("books-micro-crossed-title".into(), "books-micro-crossed".into()),
            (Step::Crossed, _) => ("books-franchise-crossed-title".into(), "books-franchise-crossed".into()),
            (Step::Higher, _) => ("books-franchise-higher-title".into(), "books-franchise-higher".into()),
        };
        let settle = if step == Step::Higher { Settle::Scheme(day) } else { Settle::Nothing };
        out.push(Notice { key: key(&w.id, w.year, step), also, step, actionable: step == Step::Higher, settle, title: tr.text(&title, Some(&args)), body: tr.text(&body, Some(&args)), day });
    }
    out
}

/// The notices not told yet (`told`) nor settled: told once, whichever
/// device tells them.
pub fn to_tell(notices: Vec<Notice>, told: &BTreeSet<String>, settings: &Settings) -> Vec<Notice> {
    notices.into_iter().filter(|n| !told.contains(&n.key) && !settled(settings, n)).collect()
}

/// The notices to do told already and not settled: told again at each
/// start, so that one never goes before it is done (the review of phase 4, P4-4).
pub fn again(notices: Vec<Notice>, told: &BTreeSet<String>, settings: &Settings) -> Vec<Notice> {
    notices.into_iter().filter(|n| n.actionable && told.contains(&n.key) && !settled(settings, n)).collect()
}

/// What the books say of a watch: "The VAT franchise's threshold for
/// services: €30,100 counted this year, of €37,500." and, prorated, why.
pub fn sentence(tr: &Translator, w: &Watch, currency: &str) -> String {
    let money = |m: i64| tr.money_in(crate::money::Money(m), currency);
    let mut args = crate::i18n::args();
    args.set("name", name(tr, w));
    args.set("turnover", money(w.turnover));
    args.set("amount", money(w.amount));
    args.set("least", if w.at_least { "yes" } else { "no" });
    let mut said = tr.text(if w.crossed.is_some() { "books-threshold-above" } else { "books-threshold-status" }, Some(&args));
    if let Some(started) = w.prorated_from {
        let mut a = crate::i18n::args();
        a.set("full", money(w.full));
        a.set("started", long_day(tr, started));
        said = format!("{said} {}", tr.text("books-threshold-prorated", Some(&a)));
    }
    said
}

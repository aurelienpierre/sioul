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
//! Each step is said once: at 80 % of a base threshold or a ceiling (to
//! know), on crossing it (to know), and on crossing a higher threshold (to
//! do, until the tax scheme says VAT from that day). The steps told are
//! marked in the invoices' life (`books::mark_told`), whichever device told
//! them: never told again, on any device.

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

/// One amount counted, on its day, of a nature.
#[derive(Debug, Clone)]
struct Counted {
    day: Date,
    nature: &'static str,
    amount: i64,
}

/// The nature a receipt no invoice describes counts as: the first activity
/// said's, else a service.
fn unsaid_nature(settings: &Settings) -> &'static str {
    settings.activity_unsaid().and_then(|a| settings.nature_of(a)).unwrap_or("service")
}

/// A row of the receipts book as amounts of each nature.
fn of_row(r: &Row, settings: &Settings) -> Vec<Counted> {
    let Some(without) = r.without_tax else { return Vec::new() };
    if r.nature.is_empty() {
        return vec![Counted { day: r.day, nature: unsaid_nature(settings), amount: without }];
    }
    vec![Counted { day: r.day, nature: "goods", amount: r.goods }, Counted { day: r.day, nature: "service", amount: without - r.goods }]
}

/// The turnover as the franchise counts it from `from` to `to`: each
/// invoice's lines located here, goods on its day (a deposit's on its
/// payments), services on each payment's day, its share; the receipts no
/// invoice in the books' currency describes, on their day. A credit note
/// takes its goods off on its day, its services on the days its refund is
/// paid back.
fn franchise_counted(input: &Input, rows: &[Row], from: Date, to: Date) -> Vec<Counted> {
    let settings = input.settings;
    let within = |d: Date| d >= from && d <= to;
    let mut out = Vec::new();
    let ours = |i: &Invoice| !i.number.is_empty() && !i.unreadable && (i.currency.trim().is_empty() || i.currency.eq_ignore_ascii_case(&settings.currency));
    for invoice in input.invoices.iter().filter(|i| ours(i)) {
        let Some(day) = invoice.date.get(..10).and_then(|d| d.parse::<Date>().ok()) else { continue };
        let parts = super::vat::parts(invoice, input.invoices);
        let deposit = invoice.kind == Kind::Deposit;
        let located = |category: &str| !ELSEWHERE.contains(&category);
        if !deposit && within(day) {
            for (_, (base, _)) in parts.iter().filter(|((n, c, _), _)| *n == "goods" && located(c)) {
                out.push(Counted { day, nature: "goods", amount: *base });
            }
        }
        if invoice.totals.due <= 0 {
            continue;
        }
        for p in super::received_of(invoice).into_iter().filter(|p| within(p.day)) {
            for ((nature, _, _), (base, _)) in parts.iter().filter(|((n, c, _), _)| located(c) && (*n == "service" || deposit)) {
                out.push(Counted { day: p.day, nature, amount: share(*base, p.entry, invoice.totals.due) });
            }
        }
    }
    // What no invoice in the books' currency describes: as the receipts book
    // has it, on its day; an invoice in another currency, the share of its
    // lines located here, goods and services alike on the payment's day (its
    // amount here is known then only).
    let numbers: BTreeSet<&str> = input.invoices.iter().filter(|i| ours(i)).map(|i| i.number.as_str()).collect();
    for r in rows.iter().filter(|r| within(r.day) && !numbers.contains(r.invoice.as_str())) {
        let foreign = (!r.invoice.is_empty()).then(|| input.invoices.iter().find(|i| i.number == r.invoice && !i.unreadable)).flatten();
        match (foreign, r.without_tax) {
            (Some(invoice), Some(without)) => {
                let parts = super::vat::parts(invoice, input.invoices);
                let whole: i64 = parts.values().map(|(base, _)| base).sum();
                for nature in ["goods", "service"] {
                    let here: i64 = parts.iter().filter(|((n, c, _), _)| *n == nature && !ELSEWHERE.contains(&c.as_str())).map(|(_, (base, _))| base).sum();
                    if here != 0 && whole != 0 {
                        out.push(Counted { day: r.day, nature, amount: share(here, without, whole) });
                    }
                }
            }
            (Some(_), None) => {}
            (None, _) => out.extend(of_row(r, settings)),
        }
    }
    out
}

/// The receipts book's rows from `from` to `to`, as amounts of each nature.
fn received_counted(rows: &[Row], settings: &Settings, from: Date, to: Date) -> Vec<Counted> {
    rows.iter().filter(|r| r.day >= from && r.day <= to).flat_map(|r| of_row(r, settings)).collect()
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

/// Where each threshold watched stands in `today`'s year, until `today`:
/// the franchise's while the franchise holds today, the micro regime's
/// ceilings under it. None for a preset without thresholds.
pub fn watch(input: &Input, today: Date, tr: &Translator) -> Vec<Watch> {
    let settings = input.settings;
    let year = today.year();
    let first = Date::new(year, 1, 1).unwrap_or(today);
    let sold = natures(input);
    let franchise = settings.scheme_on(today) == "franchise";
    let micro = settings.regime == "micro";
    let mut out = Vec::new();
    let want_franchise = franchise && settings.preset.threshold.iter().any(|t| t.kind.starts_with("franchise"));
    let want_micro = micro && settings.preset.threshold.iter().any(|t| t.kind == "micro");
    if !want_franchise && !want_micro {
        return out;
    }
    let last_first = Date::new(year - 1, 1, 1).unwrap_or(first);
    let rows = super::rows(input, last_first, today, tr);
    let at_least = rows.iter().any(|r| r.day >= first && r.without_tax.is_none());
    if want_franchise {
        let counted = franchise_counted(input, &rows, first, today);
        for kind in ["franchise", "franchise-higher"] {
            for t in thresholds_on(settings, kind, today).into_iter().filter(|t| t.scheme.is_empty() || t.scheme == "franchise").filter(|t| applies(t, settings, &sold)) {
                let (amount, prorated_from) = amount_in(&t, settings, year);
                let counts = counts_of(&t, settings);
                let near = if kind == "franchise" { share(80, amount, 100) } else { i64::MAX };
                let (turnover, reached, above) = walk(&counted, counts, near, amount);
                out.push(Watch { id: t.id.clone(), kind: kind.into(), year, counts, amount, full: t.amount, prorated_from, turnover, at_least, near: reached, crossed: above, last_year_above: false, corrective: t.corrective });
            }
        }
    }
    if want_micro {
        let counted = received_counted(&rows, settings, first, today);
        let last_end = Date::new(year - 1, 12, 31).unwrap_or(first);
        let before = received_counted(&rows, settings, last_first, last_end);
        for t in thresholds_on(settings, "micro", today).into_iter().filter(|t| t.regime.is_empty() || t.regime == settings.regime).filter(|t| applies(t, settings, &sold)) {
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

/// A notice of the books to tell: its key (and the earlier steps marked
/// told with it), its words, whether it is to do, its day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notice {
    pub key: String,
    /// Earlier steps of the same watch, marked told with it, never told after it.
    pub also: Vec<String>,
    pub step: Step,
    /// To do (a higher threshold crossed): it stays until the tax scheme
    /// says VAT from `day` (`settled`).
    pub actionable: bool,
    pub title: String,
    pub body: String,
    /// The day it happened.
    pub day: Date,
}

/// Whether a notice to do is settled: the tax scheme says VAT on its day.
pub fn settled(settings: &Settings, notice: &Notice) -> bool {
    notice.actionable && settings.scheme_on(notice.day) == "liable"
}

/// A threshold's name in words: "the VAT franchise's threshold for services".
pub fn name(tr: &Translator, w: &Watch) -> String {
    let family = if w.kind == "micro" { "micro" } else { "franchise" };
    let higher = if w.kind == "franchise-higher" { "-higher" } else { "" };
    tr.text(&format!("books-threshold-{family}{higher}-{}", w.counts), None)
}

/// The steps of `watches` not told yet (`told`), one per watch, the most
/// advanced: a crossing said, its 80 % is never said after it.
pub fn to_tell(watches: &[Watch], told: &BTreeSet<String>, tr: &Translator, currency: &str, today: Date) -> Vec<Notice> {
    let money = |m: i64| tr.money_in(crate::money::Money(m), currency);
    let mut out = Vec::new();
    for w in watches {
        let steps: Vec<(Step, Date)> = match w.kind.as_str() {
            "franchise-higher" => w.crossed.map(|d| (Step::Higher, d)).into_iter().collect(),
            _ => [w.near.map(|d| (Step::Near, d)), w.crossed.map(|d| (Step::Crossed, d))].into_iter().flatten().collect(),
        };
        let Some(&(step, day)) = steps.last() else { continue };
        let k = key(&w.id, w.year, step);
        if told.contains(&k) {
            continue;
        }
        let also: Vec<String> = steps.iter().filter(|(s, _)| *s != step).map(|(s, _)| key(&w.id, w.year, *s)).collect();
        let mut args = crate::i18n::args();
        args.set("name", name(tr, w));
        args.set("turnover", money(w.turnover));
        args.set("amount", money(w.amount));
        args.set("day", long_day(tr, day));
        args.set("today", if day == today { "yes" } else { "no" });
        args.set("next", (w.year + 1).to_string());
        args.set("corrective", if w.corrective { "yes" } else { "no" });
        let family = if w.kind == "micro" { "micro" } else { "franchise" };
        let (title, body) = match (step, family) {
            (Step::Near, _) => (format!("books-near-{family}-title"), format!("books-near-{family}")),
            (Step::Crossed, "micro") if w.last_year_above => ("books-micro-ends-title".into(), "books-micro-ends".into()),
            (Step::Crossed, "micro") => ("books-micro-crossed-title".into(), "books-micro-crossed".into()),
            (Step::Crossed, _) => ("books-franchise-crossed-title".into(), "books-franchise-crossed".into()),
            (Step::Higher, _) => ("books-franchise-higher-title".into(), "books-franchise-higher".into()),
        };
        out.push(Notice { key: k, also, step, actionable: step == Step::Higher, title: tr.text(&title, Some(&args)), body: tr.text(&body, Some(&args)), day });
    }
    out
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

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The books in the window (docs/projects.md, "The books"; `sioul_core::books`):
//!
//! - **The Porch's dated items** (`porch_items`): each declaration of
//!   turnover whose period is over, from the day it opens, and each DES,
//!   quiet rows of the Money section, each its day to declare by; they stay
//!   until dismissed on any device (the notices' words; a Done would settle
//!   them too), or until two months past their day. Nothing says "late".
//! - **The thresholds told once** (`tick`): each step of a threshold (80 %,
//!   crossed, the higher one crossed) and what takes effect on 1 January,
//!   told by the device that keeps the notices, marked in the invoices'
//!   life as it is posted, so that no device tells it as new again; a notice
//!   to do (the higher one crossed, the franchise lost, the micro regime
//!   ended) stays until it is done, and is told again at each start until
//!   then.

use crate::backend::{Shared, load_config, tr};
use sioul_core::books::{self, declarations, des, thresholds};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

/// How long after its day a declaration's or a DES's item stays on the
/// Porch, unless said done or dismissed.
pub(crate) const AFTER_DAYS: i64 = 60;

/// The books as the files say them now.
pub(crate) fn loaded(today: jiff::civil::Date) -> books::Loaded {
    books::load(&load_config(), &sioul_core::invoice::folder(), today)
}

/// The Overview's line beside a budget that declares, from the books
/// (`declarations::overview_line`), with the money already read.
pub(crate) fn overview_line(config: &sioul_core::config::Config, money: &sioul_core::movements::Books, today: jiff::civil::Date) -> Option<String> {
    let folder = sioul_core::invoice::folder();
    let settings = books::Settings::of(&config.invoice, Some(&money.ledger));
    let invoices = sioul_core::invoice::all_in(&folder);
    let lives = sioul_core::invoice::lives::Lives::load(&folder);
    let declared = books::declared_days(&lives);
    declarations::overview_line(&books::Input { settings: &settings, invoices: &invoices, booked: &lives.booked, declared: &declared, money: Some(money), budget: None }, today, tr())
}

/// One dated item of the Porch's Money.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    /// Its notice's id, the same on every device: "money:books/declaration/2026-Q4".
    pub id: String,
    /// The day it opens, and its day to declare by.
    pub opens: jiff::civil::Date,
    pub due: jiff::civil::Date,
    pub who: String,
    pub what: String,
    pub amount: String,
    /// What it holds, under the row: the contributions estimated, the invoices a DES covers.
    pub note: String,
    /// A day of its period: the declaration view opens there (the Porch's row's Open).
    pub anchor: jiff::civil::Date,
}

/// The declarations and the DES to make now (`declarations::open_now`, `des::open_now`), as the Porch's items.
pub(crate) fn porch_items(loaded: &books::Loaded, today: jiff::civil::Date) -> Vec<Item> {
    let tr = tr();
    let input = loaded.input();
    let currency = loaded.settings.currency.clone();
    let money = |m: i64| tr.money_in(sioul_core::money::Money(m), &currency);
    let mut out = Vec::new();
    for d in declarations::open_now(&input, today, AFTER_DAYS, tr) {
        // Said "Declared", here or on another device: done.
        if loaded.declared.contains_key(&books::declaration_key(&d)) {
            continue;
        }
        let mut args = sioul_core::i18n::args();
        args.set("period", declarations::period_name(tr, &d));
        args.set("due", tr.day_short(d.due));
        out.push(Item {
            id: sioul_core::notices::money_id(&format!("books/declaration/{}", d.key())),
            opens: d.opens,
            due: d.due,
            who: tr.text("books-porch-declaration", None),
            what: tr.text("books-porch-declare-what", Some(&args)),
            // What to declare, "at least" while a receipt is not known or a payment in another currency not said in euros.
            amount: if d.least() { tr.text("books-at-least-amount", Some(&{ let mut a = sioul_core::i18n::args(); a.set("amount", money(d.declared)); a })) } else { money(d.declared) },
            // A payment in another currency to say in euros; a zero period said as such ("Nothing received…: declare €0 by…"); else the estimate.
            note: declarations::porch_note(tr, &d, &currency),
            anchor: d.from,
        });
    }
    for d in des::open_now(&input, today, AFTER_DAYS) {
        if loaded.declared.contains_key(&books::des_key(&d)) {
            continue;
        }
        let mut args = sioul_core::i18n::args();
        args.set("month", tr.month_year(d.month));
        args.set("due", tr.day_short(d.due));
        // An invoice in another currency is listed, not added: the total is "at least", and said why.
        let mut note: Vec<String> = Vec::new();
        if d.foreign > 0 {
            let mut a = sioul_core::i18n::args();
            a.set("foreign", d.foreign as i64);
            note.push(tr.text("books-des-foreign-note", Some(&a)));
        }
        note.extend(d.invoices.iter().map(|c| des::covered_words(tr, c)));
        out.push(Item {
            id: sioul_core::notices::money_id(&format!("books/des/{}", d.key())),
            opens: d.opens,
            due: d.due,
            who: tr.text("books-porch-des", None),
            what: tr.text("books-porch-des-what", Some(&args)),
            amount: if d.foreign > 0 { tr.text("books-at-least-amount", Some(&{ let mut a = sioul_core::i18n::args(); a.set("amount", money(d.total)); a })) } else { money(d.total) },
            note: note.join("\n"),
            anchor: d.month,
        });
    }
    out
}

/// When the thresholds were last looked at (Unix seconds).
static LOOKED: AtomicI64 = AtomicI64::new(0);
static BUSY: Mutex<()> = Mutex::new(());
/// The thresholds are looked at this often, at most: a payment's day changes them, never a minute.
const EVERY: i64 = 15 * 60;

/// Whether the steps told before were said again since this run began (`tick`).
static TOLD_AGAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Each minute of the window: at most every quarter of an hour, off the
/// window's thread, by the device that keeps the notices (as new mail: the
/// one you used last), the thresholds' steps not told yet told, each marked
/// told in this device's invoices' life once posted; last year's read too
/// (what takes effect on 1 January, a crossing learned after New Year). At
/// the run's first look, a step to do told before and not done yet is told
/// again (the review of phase 4, P4-4). With sharing on, only a device that
/// shares Drafts and invoices tells, once its files were exchanged since it
/// started: its marks travel, and it reads every other device's (P4-17).
/// Never on a phone.
pub(crate) fn tick(shared: &Arc<Shared>) {
    if cfg!(target_os = "android") {
        return;
    }
    let now = jiff::Timestamp::now().as_second();
    if now - LOOKED.load(Ordering::Relaxed) < EVERY {
        return;
    }
    let active = shared.active.load(Ordering::Relaxed);
    if crate::share::configured() && (!crate::share::shares("drafts") || !crate::share::last_exchange().is_some_and(|(_, ok)| ok)) {
        return;
    }
    LOOKED.store(now, Ordering::Relaxed);
    std::thread::spawn(move || {
        let Some(_busy) = crate::backend::one_at_a_time(&BUSY) else { return };
        // The books are the business's: among the computers work comes forward on now, the one
        // used last looks; while it comes forward on none (the evening), the one used last of all.
        let keeper = crate::dispatch::Tellers::now(Some(active)).keeper(Some(sioul_core::areas::Area::WORK));
        if !keeper.mine || !keeper.settled {
            return;
        }
        let today = sioul_core::zone::now().date();
        let loaded = loaded(today);
        let input = loaded.input();
        let last_day = jiff::civil::Date::new(today.year() - 1, 12, 31).unwrap_or(today);
        let notices = thresholds::notices(&thresholds::watch(&input, today, tr()), &thresholds::watch_year(&input, today.year() - 1, last_day, tr()), tr(), &loaded.settings.currency, today);
        if !TOLD_AGAIN.swap(true, Ordering::Relaxed) {
            for notice in thresholds::again(notices.clone(), &loaded.told, &loaded.settings) {
                tell(notice, today, false);
            }
        }
        for notice in thresholds::to_tell(notices, &loaded.told, &loaded.settings) {
            tell(notice, today, true);
        }
    });
}

/// One threshold's step told (`sioul_core::notice`): a date's notice, as
/// the matrix of what reaches you says the dates; to do when a higher
/// threshold is crossed, taken away once the tax scheme says VAT from its
/// day. About the budgets, each step by its own key (as the money watch's
/// warnings), so that two told at one look never replace each other;
/// opening it opens Budgets, its words saying where the scheme changes.
/// Marked told (its key and the earlier steps') just before it is posted
/// (`mark`; a step told again is told already); one that waits is offered
/// again at the next look.
fn tell(notice: thresholds::Notice, today: jiff::civil::Date, mark: bool) {
    use sioul_core::attention::Kind as Row;
    use sioul_core::notice::{Class, Kind, Notice};
    let about = sioul_core::notice::address::keyed("sioul:budget/", &notice.key);
    let mut told = Notice::new(about, Kind::Own(Row::Dates), notice.title.clone(), notice.body.clone());
    let mut keys = vec![notice.key.clone()];
    keys.extend(notice.also.iter().cloned());
    let claim: crate::tell::Claim = Box::new(move || {
        if !mark {
            return true;
        }
        match books::mark_told(&sioul_core::invoice::folder(), &crate::notices::here_id(), &keys, today) {
            Ok(()) => true,
            Err(e) => {
                // Not written: told by none, rather than again and again.
                eprintln!("sioul: books: {e}");
                false
            }
        }
    });
    let settled: Option<crate::tell::Settled> = if notice.actionable {
        told = told.of_class(Class::Actionable);
        Some(Box::new(move || {
            let settings = books::Settings::of(&load_config().invoice, None);
            thresholds::settled(&settings, &notice)
        }))
    } else {
        None
    };
    let _ = crate::tell::tell_with(told, crate::tell::own(Row::Dates), settled, Some(claim), true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::books::{Input, Settings};
    use sioul_core::config::{InvoiceSettings, TaxSetting};
    use sioul_core::invoice::{Address, Client, Invoice, Kind, Line, Nature, Payment};

    fn day(text: &str) -> jiff::civil::Date {
        text.parse().unwrap()
    }

    /// The Porch's items: the third quarter's declaration from 1 October,
    /// its day 2 November (31 October is a Saturday), the contributions
    /// under it; the DES of October from 1 November, its invoices under it.
    #[test]
    fn the_porch_says_each_declaration_and_des() {
        let s = InvoiceSettings { preset: "fr".into(), legal_form: "ei".into(), regime: "micro".into(), activity: vec!["bnc".into()], tax: vec![TaxSetting { from: String::new(), scheme: "franchise".into(), mention: String::new() }], ..InvoiceSettings::default() };
        let mut lines = vec![Line { quantity: 1000, unit: "C62".into(), price: 120_000, base: 1, nature: Nature::Service, ..Line::of_time("Consulting", 0, 0, "AE", 0) }];
        let totals = sioul_core::invoice::totals::compute(&mut lines, 0, &|_, _| (String::new(), String::new()));
        let client = Client { kind: "business".into(), name: "Beispiel GmbH".into(), address: Address { country: "DE".into(), city: "Berlin".into(), ..Address::default() }, ..Client::default() };
        let invoice = Invoice { format: 2, kind: Kind::Invoice, number: "2026-031".into(), date: "2026-09-30".into(), period_to: "2026-10-02".into(), currency: "EUR".into(), issuer: sioul_core::invoice::Issuer { preset: "fr".into(), ..Default::default() }, client, lines, totals, payments: vec![Payment { id: "p".into(), date: "2026-09-30".into(), amount: 120_000, means: "transfer".into(), ..Payment::default() }], ..Invoice::default() };
        let loaded = books::Loaded { settings: Settings::of(&s, None), invoices: vec![invoice], booked: Default::default(), told: Default::default(), declared: Default::default(), money: None };
        let _: Input = loaded.input();
        let items = porch_items(&loaded, day("2026-11-05"));
        assert_eq!(items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["money:books/declaration/2026-Q3", "money:books/des/2026-10"]);
        assert_eq!((items[0].opens, items[0].due), (day("2026-10-01"), day("2026-11-02")));
        assert_eq!(items[1].due, day("2026-11-13"));
        assert!(items[1].note.starts_with("2026-031 · Beispiel GmbH"), "{}", items[1].note);
        assert!(porch_items(&loaded, day("2027-01-05")).iter().all(|i| i.id != "money:books/declaration/2026-Q3"), "two months past its day, it goes");
    }

    /// An invoice in another currency is listed in the DES, never added: the
    /// Porch's amount reads "at least", and its note says why before the
    /// invoices (the review of phase 4, P4-17).
    #[test]
    fn the_porch_s_des_says_what_it_does_not_count() {
        let s = InvoiceSettings { preset: "fr".into(), legal_form: "ei".into(), regime: "micro".into(), activity: vec!["bnc".into()], tax: vec![TaxSetting { from: String::new(), scheme: "franchise".into(), mention: String::new() }], ..InvoiceSettings::default() };
        let reverse = |number: &str, currency: &str, price: i64| {
            let mut lines = vec![Line { quantity: 1000, unit: "C62".into(), price, base: 1, nature: Nature::Service, ..Line::of_time("Consulting", 0, 0, "AE", 0) }];
            let totals = sioul_core::invoice::totals::compute(&mut lines, 0, &|_, _| (String::new(), String::new()));
            let client = Client { kind: "business".into(), name: "Beispiel GmbH".into(), address: Address { country: "DE".into(), city: "Berlin".into(), ..Address::default() }, ..Client::default() };
            Invoice { format: 2, kind: Kind::Invoice, number: number.into(), date: "2026-10-30".into(), period_to: "2026-10-30".into(), currency: currency.into(), issuer: sioul_core::invoice::Issuer { preset: "fr".into(), ..Default::default() }, client, lines, totals, ..Invoice::default() }
        };
        let loaded = books::Loaded { settings: Settings::of(&s, None), invoices: vec![reverse("2026-031", "EUR", 120_000), reverse("2026-032", "USD", 50_000)], booked: Default::default(), told: Default::default(), declared: Default::default(), money: None };
        let items = porch_items(&loaded, day("2026-11-05"));
        let des = items.iter().find(|i| i.id == "money:books/des/2026-10").expect("October's DES");
        let tr = tr();
        let mut a = sioul_core::i18n::args();
        a.set("amount", tr.money_in(sioul_core::money::Money(120_000), "EUR"));
        assert_eq!(des.amount, tr.text("books-at-least-amount", Some(&a)), "the dollars are not in the total");
        let note: Vec<&str> = des.note.lines().collect();
        let mut f = sioul_core::i18n::args();
        f.set("foreign", 1i64);
        assert_eq!(note.len(), 3, "{}", des.note);
        assert_eq!(note[0], tr.text("books-des-foreign-note", Some(&f)));
        assert!(note[2].starts_with("2026-032 · Beispiel GmbH"), "{}", des.note);
        assert_eq!(des.anchor, day("2026-10-01"), "Open: the declaration view at its month");
        // Said "Declared" (on any device): no longer on the Porch.
        let declared = books::Loaded { declared: std::collections::BTreeMap::from([("des:2026-10".to_string(), day("2026-11-04"))]), ..loaded };
        assert!(porch_items(&declared, day("2026-11-05")).iter().all(|i| i.id != "money:books/des/2026-10"));
    }
}

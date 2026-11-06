// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The full ledger (design §4, docs/accounting.md, "Movements tab"): every
//! movement, newest first, in pages of 60, found by its filters (the days,
//! the account, the budget, in or out, the amount, what it is for, what
//! told of it) and by words; a sum only when the filters ask a question
//! that has one ("Money in, work, October: €2,340 paid, €2,281 arrived").
//! People find things again by scrolling the recent past and by searching,
//! not by filing (Whittaker et al. 2011; Bergman et al. 2008): the list
//! starts at the latest movements, a typed search reaches every area, and no
//! running total, count or red is shown. The window, the agents and the
//! command line read the same list (`query`, `get`), and say its rows with
//! the same words (`row_words`, `detail_words`).

use crate::areas::Area;
use crate::budget::Ledger;
use crate::i18n::Translator;
use crate::money::Money;
use crate::movements::{Books, Fee, FeeFrom, Kind, Movement, State, Way};
use jiff::civil::Date;

/// Rows in a page, as Mail's list holds them.
pub const PAGE: usize = 60;

/// What told of a movement, for a filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToldBy {
    /// The bank's line, or a card hold.
    Bank,
    /// A message, a letter, an invoice, a fee invoice.
    Mail,
    /// A recurring payment, its contract, a share set aside.
    Plan,
    /// A line of yours.
    Hand,
}

impl ToldBy {
    pub fn parse(word: &str) -> Option<ToldBy> {
        match word.trim() {
            "bank" => Some(ToldBy::Bank),
            "mail" => Some(ToldBy::Mail),
            "plan" => Some(ToldBy::Plan),
            "hand" => Some(ToldBy::Hand),
            _ => None,
        }
    }

    fn of(self, m: &Movement) -> bool {
        m.sources.iter().any(|s| match self {
            ToldBy::Bank => matches!(s.kind, Kind::Bank | Kind::Hold),
            ToldBy::Mail => matches!(s.kind, Kind::Mail | Kind::Letter | Kind::Invoice | Kind::FeeInvoice),
            ToldBy::Plan => matches!(s.kind, Kind::Preset | Kind::Contract | Kind::SetAside),
            ToldBy::Hand => s.kind == Kind::Line,
        })
    }
}

/// The ledger's filters: none given, every movement.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    pub from: Option<Date>,
    pub to: Option<Date>,
    /// A bank account, by its id.
    pub account: Option<String>,
    /// A budget, by its id.
    pub budget: Option<String>,
    pub way: Option<Way>,
    /// What it is for; none: every area. A typed search reaches every area whatever this says.
    pub areas: Option<Area>,
    /// The amount without its sign, at least and at most.
    pub at_least: Option<Money>,
    pub at_most: Option<Money>,
    /// Words, each found in who, the note, the budget's title or the amount.
    pub text: String,
    /// What told of it; empty: anything.
    pub told_by: Vec<ToldBy>,
    /// The page, from 0.
    pub page: usize,
}

/// What a question with a sum adds up to: what was paid and what moved the accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Total {
    pub way: Way,
    pub gross: Money,
    pub net: Money,
}

/// One page of the ledger.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub movements: Vec<Movement>,
    pub page: usize,
    /// Another page follows.
    pub more: bool,
    /// Only when the filters ask a question that has one: one way, and days.
    pub total: Option<Total>,
}

/// Folded words, as searches compare them.
fn folded(text: &str) -> String {
    crate::text::fold(text).into_iter().collect()
}

/// Whether `m` answers the filters (`ledger`: the budgets' titles).
pub fn matches(ledger: &Ledger, q: &Query, m: &Movement) -> bool {
    if q.from.is_some_and(|d| m.day < d) || q.to.is_some_and(|d| m.day > d) {
        return false;
    }
    if q.account.as_deref().is_some_and(|a| m.account.as_deref() != Some(a) && !matches!(&m.place, crate::movements::Place::Transfer { from, to } if from == a || to == a)) {
        return false;
    }
    if q.budget.as_deref().is_some_and(|b| !m.place.budgets().contains(&b)) {
        return false;
    }
    if q.way.is_some_and(|w| m.way != w) {
        return false;
    }
    let amount = m.sum.amount().abs();
    if q.at_least.is_some_and(|a| amount < a.abs()) || q.at_most.is_some_and(|a| amount > a.abs()) {
        return false;
    }
    if !q.told_by.is_empty() && !q.told_by.iter().any(|t| t.of(m)) {
        return false;
    }
    let words: Vec<String> = q.text.split_whitespace().map(folded).filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        // The hour's areas while nothing is typed: a typed search reaches every area.
        return q.areas.is_none_or(|areas| m.area.is_empty() || m.area.meets(areas));
    }
    let titles: Vec<&str> = m.place.budgets().iter().map(|b| ledger.budget_title(b)).collect();
    let cents = m.sum.amount().cents().abs();
    let amounts = [format!("{}.{:02}", cents / 100, cents % 100), format!("{},{:02}", cents / 100, cents % 100)];
    let haystack = folded(&format!("{} {} {} {} {}", m.label, m.note, titles.join(" "), amounts.join(" "), m.gross.map(|g| format!("{}.{:02}", g.cents().abs() / 100, g.cents().abs() % 100)).unwrap_or_default()));
    words.iter().all(|w| haystack.contains(w.as_str()))
}

/// The ledger's movements the filters keep, newest first, page `q.page` of
/// 60; a sum only when the filters ask a question that has one (one way,
/// some days): what was paid, what moved the accounts.
pub fn query(books: &Books, q: &Query) -> Page {
    query_in(&books.ledger, books.movements.iter(), q)
}

/// `query` over some movements (those an agent may see).
pub fn query_in<'a>(ledger: &Ledger, movements: impl Iterator<Item = &'a Movement>, q: &Query) -> Page {
    let mut all: Vec<&Movement> = movements.filter(|m| matches(ledger, q, m)).collect();
    all.sort_by(|a, b| (b.day, &b.id).cmp(&(a.day, &a.id)));
    let start = q.page * PAGE;
    let movements: Vec<Movement> = all.iter().skip(start).take(PAGE).map(|m| (*m).clone()).collect();
    let more = all.len() > start + PAGE;
    let total = match q.way {
        Some(way) if way != Way::Between && (q.from.is_some() || q.to.is_some()) => Some(Total { way, gross: all.iter().map(|m| m.gross_or_net()).sum(), net: all.iter().map(|m| m.sum.amount()).sum() }),
        _ => None,
    };
    Page { movements, page: q.page, more, total }
}

/// One movement by its address, or any of its sources' keys; one you
/// removed too, said so (`true`), so that it can be put back.
pub fn get<'a>(books: &'a Books, address: &str) -> Option<(&'a Movement, bool)> {
    let key = address.strip_prefix("sioul:movement/").map_or_else(|| address.to_string(), crate::notes::decode);
    let named = |m: &&Movement| m.has(&key) || m.address() == address;
    books.movements.iter().find(named).map(|m| (m, false)).or_else(|| books.gone.iter().find(named).map(|m| (m, true)))
}

/// An amount with its sign, as a row says it: "+€504.57", "−€12.00".
pub fn signed(tr: &Translator, amount: Money) -> String {
    let sign = if amount.is_negative() { "−" } else if amount == Money::ZERO { "" } else { "+" };
    format!("{sign}{}", tr.money(amount.abs()))
}

/// A row in words: its day, who, what moved with its sign, its budget, its
/// state; then, only when there are some, its fees beside what was paid, or
/// what a payout carried, or the payout that carried it; then your note.
pub fn row_words(tr: &Translator, ledger: &Ledger, m: &Movement, today: Date) -> Vec<String> {
    let row = crate::moneyview::coming_row(ledger, m, &Default::default(), tr, today);
    let budget = if row.budget.is_empty() { row.way_words.clone() } else { row.budget.clone() };
    let mut lines = vec![[row.date.clone(), row.who.clone(), signed(tr, m.sum.amount()), budget, row.state].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ")];
    if let Some(second) = second_line(tr, m) {
        lines.push(second);
    }
    if !m.note.trim().is_empty() {
        lines.push(format!("“{}”", m.note.trim()));
    }
    lines
}

/// "paid €520.00 · fees €15.43", "carries 3 payments", "part of the payout
/// of Thu 12 Nov"; none for a movement of one amount.
pub fn second_line(tr: &Translator, m: &Movement) -> Option<String> {
    let mut args = crate::i18n::args();
    if !m.carried.is_empty() && m.carried.iter().any(|c| c.how != "export") || m.carried.len() > 1 {
        args.set("count", m.carried.len() as i64);
        let carries = tr.text("movement-carries", Some(&args));
        return Some(match fees_words(tr, m) {
            Some(fees) => format!("{fees} · {carries}"),
            None => carries,
        });
    }
    if let Some(fees) = fees_words(tr, m) {
        return Some(fees);
    }
    m.carried_by.as_ref().map(|_| {
        args.set("day", m.paid_on.map_or_else(|| crate::moneyview::short_day(tr, m.day), |d| crate::moneyview::short_day(tr, d)));
        tr.text("movement-carried-by", Some(&args))
    })
}

/// What was paid and its fees, when it has fees or they are not known.
fn fees_words(tr: &Translator, m: &Movement) -> Option<String> {
    let gross = m.gross?;
    let shown = match &m.gross_currency {
        Some(currency) => format!("{currency} {}.{:02}", gross.cents().abs() / 100, gross.cents().abs() % 100),
        None => tr.money(gross.abs()),
    };
    let mut args = crate::i18n::args();
    args.set("gross", shown);
    if m.fees_unknown {
        return Some(tr.text("movement-paid-unknown", Some(&args)));
    }
    if m.fees.is_empty() {
        return None;
    }
    args.set("fees", tr.money(m.fee_total()));
    let id = if m.fees.iter().all(|f| f.about) {
        "movement-paid-about"
    } else if m.way == Way::Out {
        "movement-paid-out"
    } else {
        "movement-paid-in"
    };
    Some(tr.text(id, Some(&args)))
}

/// One fee in words: its kind, its amount, where it was read.
pub fn fee_words(tr: &Translator, fee: &Fee) -> String {
    let mut args = crate::i18n::args();
    args.set("kind", tr.text(&format!("movement-fee-kind-{}", fee.kind.id()), None));
    args.set("amount", tr.money(fee.amount));
    args.set("from", tr.text(&format!("movement-fee-from-{}", if fee.about && fee.from != FeeFrom::Rate { "rate" } else { fee.from.id() }), None));
    tr.text("movement-fee-line", Some(&args))
}

/// A movement in full, as its detail shows it: its row, each fee and where
/// it was read, the day it was paid, what told of it, what it is tied to,
/// each payment a payout carried, when Sioul was told and when it changed.
pub fn detail_words(tr: &Translator, ledger: &Ledger, m: &Movement, today: Date) -> Vec<String> {
    let mut lines = row_words(tr, ledger, m, today);
    for fee in &m.fees {
        lines.push(fee_words(tr, fee));
    }
    let mut args = crate::i18n::args();
    if let Some(paid) = m.paid_on {
        args.set("day", tr.day_in(paid, today));
        lines.push(tr.text("movement-paid-on", Some(&args)));
    }
    let row = crate::moneyview::coming_row(ledger, m, &Default::default(), tr, today);
    let mut kinds: Vec<String> = Vec::new();
    for s in &row.sources {
        if !kinds.contains(&s.words) {
            kinds.push(s.words.clone());
        }
    }
    if !kinds.is_empty() {
        args.set("list", crate::ahead::list(tr, &kinds));
        lines.push(tr.text("movement-sources", Some(&args)));
    }
    if !m.links.is_empty() {
        args.set("list", m.links.join(", "));
        lines.push(tr.text("movement-ties", Some(&args)));
    }
    for c in &m.carried {
        let mut parts = vec![crate::moneyview::short_day(tr, c.day), c.label.clone(), signed(tr, c.gross)];
        if let Some(fee) = &c.fee {
            parts.push(fee_words(tr, fee));
        }
        lines.push(format!("  {}", parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ")));
    }
    if let Some(made) = m.made {
        args.set("made", tr.day_in(made, today));
        match m.changed.filter(|c| *c != made) {
            Some(changed) => {
                args.set("changed", tr.day_in(changed, today));
                lines.push(tr.text("movement-told-changed", Some(&args)));
            }
            None => lines.push(tr.text("movement-told", Some(&args))),
        }
    }
    lines
}

/// What a question with a sum adds up to, in words.
pub fn total_words(tr: &Translator, total: &Total) -> String {
    let mut args = crate::i18n::args();
    args.set("gross", tr.money(total.gross.abs()));
    args.set("net", tr.money(total.net.abs()));
    tr.text(if total.way == Way::In { "movements-sum-in" } else { "movements-sum-out" }, Some(&args))
}

/// Units as JSON says them: 504.57.
fn units(m: Money) -> f64 {
    m.cents() as f64 / 100.0
}

/// A movement as data, for the agents and the command line: its address,
/// its days, who, what moved (signed), what was paid, its fees and where each
/// was read, its budgets, account, area, state, note, ties, sources, when it
/// was told and changed, what a payout carried and the payout that carried it.
pub fn data(ledger: &Ledger, m: &Movement) -> serde_json::Value {
    let way = match m.way {
        Way::In => "in",
        Way::Out => "out",
        Way::Between => "between",
    };
    let fee = |f: &Fee| serde_json::json!({ "kind": f.kind.id(), "amount": units(f.amount), "from": f.from.id(), "about": f.about });
    let budgets: Vec<serde_json::Value> = match &m.place {
        crate::movements::Place::Parts(parts) => parts.iter().map(|(b, a)| serde_json::json!({ "id": b, "title": ledger.budget_title(b), "amount": units(*a) })).collect(),
        place => place.budgets().iter().map(|b| serde_json::json!({ "id": b, "title": ledger.budget_title(b), "amount": units(m.sum.amount()) })).collect(),
    };
    serde_json::json!({
        "address": m.address(),
        "day": m.day.to_string(),
        "expected": m.expected.map(|d| d.to_string()),
        "who": m.label,
        "way": way,
        "amount": units(m.sum.amount()),
        "about": m.sum.about(),
        "currency": m.currency,
        "gross": m.gross.map(units),
        "gross_currency": m.gross_currency,
        "fees": m.fees.iter().map(fee).collect::<Vec<_>>(),
        "fees_unknown": m.fees_unknown,
        "paid_on": m.paid_on.map(|d| d.to_string()),
        "budgets": budgets,
        "account": m.account,
        "area": m.area.id(),
        "state": m.state.id(),
        "note": m.note,
        "ties": m.links,
        "sources": m.sources.iter().map(|s| serde_json::json!({ "kind": s.kind, "key": s.key, "address": s.address, "probable": s.probable })).collect::<Vec<_>>(),
        "made": m.made.map(|d| d.to_string()),
        "changed": m.changed.map(|d| d.to_string()),
        "carried": m.carried.iter().map(|c| serde_json::json!({ "key": c.key, "day": c.day.to_string(), "who": c.label, "gross": units(c.gross), "fee": c.fee.as_ref().map(fee), "how": c.how })).collect::<Vec<_>>(),
        "carried_by": m.carried_by.as_ref().map(|k| crate::links::movement_uri(k)),
    })
}

/// Whether a movement is still to come (planned, announced, not final yet).
pub fn to_come(m: &Movement) -> bool {
    matches!(m.state, State::Planned | State::Announced | State::NotFinal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movements::{Input, Store, derive_with_gone};

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn books() -> Books {
        let ledger: Ledger = toml::from_str(
            "[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n\
             [[budget]]\nid = 'work'\ntitle = 'Work'\nperiod = 'month'\narea = 'work'\n\
             [[bank_account]]\nid = 'main'\ntitle = 'My bank'\nfills = ['home', 'work']\nexports = ['0123456A']\n\
             [[line]]\nid = 'w1'\nbudget = 'work'\ndate = 2026-10-03\namount = 504.57\ngross = 520\nfees = [{ kind = 'processor', amount = 15.43 }]\nlabel = 'Client A'\nnote = 'The October lesson'\n\
             [[line]]\nid = 'w2'\nbudget = 'work'\ndate = 2026-10-20\namount = 300\nlabel = 'Client B'\n\
             [[line]]\nid = 'h1'\nbudget = 'home'\ndate = 2026-10-12\namount = -52\nlabel = 'Energy'\n\
             [[line]]\nid = 'h2'\nbudget = 'home'\ndate = 2026-11-02\namount = -12.5\nlabel = 'Bakery'\n",
        )
        .unwrap();
        let bank = crate::bank::Bank::default();
        let store = Store::default();
        let words = crate::words::Words::builtin();
        let input = Input { ledger: &ledger, bank: &bank, contracts: &[], letters: &[], store: &store, words: &words };
        let (movements, gone) = derive_with_gone(&input, day("2026-09-01"), day("2026-12-31"), day("2026-11-05"));
        Books { counted: ledger.clone(), ledger, bank, movements, gone, from: day("2026-09-01"), to: day("2026-12-31") }
    }

    /// Newest first; each filter; a typed search reaches every area; a sum
    /// only for one way over some days; pages of 60.
    #[test]
    fn the_ledger_is_found_by_its_filters_and_its_words() {
        let books = books();
        let keys = |q: &Query| query(&books, q).movements.iter().map(|m| m.steady_key().to_string()).collect::<Vec<_>>();
        assert_eq!(keys(&Query::default()), ["line:h2", "line:w2", "line:h1", "line:w1"]);
        assert_eq!(keys(&Query { budget: Some("work".into()), ..Query::default() }), ["line:w2", "line:w1"]);
        assert_eq!(keys(&Query { way: Some(Way::Out), at_least: Some(Money(2000)), ..Query::default() }), ["line:h1"]);
        assert_eq!(keys(&Query { from: Some(day("2026-10-10")), to: Some(day("2026-10-31")), ..Query::default() }), ["line:w2", "line:h1"]);
        assert_eq!(keys(&Query { told_by: vec![ToldBy::Bank], ..Query::default() }), Vec::<String>::new());
        // Leisure's hours show no work; a typed search finds it anyway.
        let leisure = Some(Area::PERSONAL);
        assert_eq!(keys(&Query { areas: leisure, ..Query::default() }), ["line:h2", "line:h1"]);
        assert_eq!(keys(&Query { areas: leisure, text: "october lesson".into(), ..Query::default() }), ["line:w1"]);
        assert_eq!(keys(&Query { text: "504,57".into(), ..Query::default() }), ["line:w1"]);
        // A sum only when the question has one.
        assert_eq!(query(&books, &Query::default()).total, None);
        let october = query(&books, &Query { way: Some(Way::In), from: Some(day("2026-10-01")), to: Some(day("2026-10-31")), ..Query::default() });
        assert_eq!(october.total, Some(Total { way: Way::In, gross: Money(82000), net: Money(80457) }));
        let tr = Translator::new("en");
        assert_eq!(total_words(&tr, october.total.as_ref().unwrap()), "Money in: €820 paid, €804.57 arrived.");
        assert!(!query(&books, &Query { page: 1, ..Query::default() }).more);
        assert!(query(&books, &Query { page: 1, ..Query::default() }).movements.is_empty());
    }

    /// A row's words: one line, a second only for its fees, then your note;
    /// the detail adds each fee and where it was read.
    #[test]
    fn a_row_says_its_fees_inside_it() {
        let books = books();
        let tr = Translator::new("en");
        let (a, gone) = get(&books, "sioul:movement/line:w1").unwrap();
        assert!(!gone);
        let lines = row_words(&tr, &books.ledger, a, day("2026-11-05"));
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines[0].contains("Client A") && lines[0].contains("+€504.57") && lines[0].contains("Work"), "{lines:?}");
        assert_eq!(lines[1], "paid €520 · fees €15.43");
        assert_eq!(lines[2], "“The October lesson”");
        let plain = row_words(&tr, &books.ledger, get(&books, "line:h1").unwrap().0, day("2026-11-05"));
        assert_eq!(plain.len(), 1, "one amount, one line: {plain:?}");
        assert!(plain[0].contains("−€52"));
        let detail = detail_words(&tr, &books.ledger, a, day("2026-11-05"));
        assert!(detail.iter().any(|l| l == "The processor's fee: €15.43, said by you"), "{detail:?}");
        let fr = Translator::new("fr");
        assert_eq!(row_words(&fr, &books.ledger, a, day("2026-11-05"))[1], "payé 520\u{a0}€ · frais 15,43\u{a0}€");
    }
}

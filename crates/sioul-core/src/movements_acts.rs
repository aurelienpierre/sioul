// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What an export, the mail and your hand do to money movements
//! (docs/accounting.md, "Matching", "Moving a movement", "A payment from any mail"):
//!
//! - **An export taken in** (`take_in`): its booked lines into the bank's
//!   file, by their ids, so the same file twice adds nothing; its pending
//!   ones as the account's card holds; each new line then resolved against
//!   what was planned and announced, its pair kept as a link of the store so
//!   that it holds whatever comes later (`resolve`).
//! - **A message about money** that tells of a movement already counted
//!   joins it as a source without a question (`join_mails`); one that
//!   matches nothing stays proposed ("Add", "Not a payment"), and one from a
//!   sender not verified says to check it first.
//! - **A movement moved or split** from its row (`move_to`): written where
//!   every version reads it when it can be (a bank line's `[[assign]]`),
//!   else in the store by its steadiest key; the effect said; the payee's
//!   next movements asked about once (`decide`), your answer the consent.
//! - **Corrections**: "Not the same" (`not_same`) keeps two apart for good;
//!   the same by your hand (`same`) teaches the payee's name.
//! - **"A payment…"** from any message or letter (`Payment`): a short form
//!   filled from what it says, any budget, the message or letter its source.

use super::*;
use crate::bank::Statement;
use crate::budget::{MailLine, MailState};
use crate::payments::PaymentKind;

/// What an export taken in did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TakenIn {
    /// Its lines read, and those new.
    pub read: usize,
    pub new: usize,
    /// Its card holds, not final yet.
    pub holds: usize,
    /// New lines the matching joined to what was planned or announced.
    pub joined: usize,
}

/// An export taken in (see the module's notes). `statement` as `bank::read`
/// gives it, its account named already (a card's own name when the export
/// gives none).
pub fn take_in(root: &Path, statement: &Statement, title: &str, words: &crate::words::Words, today: Date) -> Result<TakenIn, String> {
    let account = if statement.account.is_empty() { "account".to_string() } else { statement.account.clone() };
    let known: BTreeSet<(String, String)> = Bank::load(root)?.movements.iter().map(|m| (m.account.clone(), m.id.clone())).collect();
    let added = Bank::change(root, |bank| bank.import(statement, title))?;
    // Its holds: what this export says is not final yet, in place of what the last one said.
    let holds: Vec<Hold> = statement.pending.iter().map(|m| Hold { account: account.clone(), id: m.id.clone(), date: m.date, amount: m.amount, label: m.label.clone() }).collect();
    let held = Store::load(root).map(|s| s.holds.iter().any(|h| h.account == account)).unwrap_or(false);
    if !holds.is_empty() || held {
        Store::edit(root, |edit| edit.holds(&account, &holds))?;
    }
    let new: Vec<(String, String)> = Bank::load(root)?.movements.iter().map(|m| (m.account.clone(), m.id.clone())).filter(|line| !known.contains(line)).collect();
    let joined = resolve(root, words, &new, today)?;
    Ok(TakenIn { read: added.read, new: added.new, holds: holds.len(), joined })
}

/// Each of the bank's lines `new` (an export's name of its account, the
/// bank's id) that the matching joins to what was planned or announced, kept
/// as a link of the store (sure or probable) by that one's steadiest key,
/// unless a link holds it already: it stays joined whatever the matching
/// says later, until you correct it. Returns how many were kept.
pub fn resolve(root: &Path, words: &crate::words::Words, new: &[(String, String)], today: Date) -> Result<usize, String> {
    if new.is_empty() {
        return Ok(0);
    }
    let ledger = Ledger::load(root).unwrap_or_default();
    let bank = Bank::load(root)?;
    let days: Vec<Date> = bank.movements.iter().filter(|m| new.contains(&(m.account.clone(), m.id.clone()))).map(|m| m.date).collect();
    let (Some(first), Some(last)) = (days.iter().min().copied(), days.iter().max().copied()) else { return Ok(0) };
    let contracts = crate::contracts::Contracts::load(root).map(|c| c.list).unwrap_or_default();
    let letters = crate::letters::Letters::load(root).map(|l| l.list).unwrap_or_default();
    let store = Store::load(root)?;
    let input = Input { ledger: &ledger, bank: &bank, contracts: &contracts, letters: &letters, store: &store, words };
    let all = derive(&input, shifted(first, -70), shifted(last, 70), today);
    // A line's key as the movements name it: its declared account's id, else the export's name.
    let key_of = |(account, id): &(String, String)| bank_key(&accounts::account_of(&ledger, account).map_or_else(|| account.clone(), |a| a.id.clone()), id);
    let linked: BTreeSet<&str> = store.links.iter().map(|l| l.source.as_str()).collect();
    let mut keep: Vec<(String, String, bool)> = Vec::new();
    for line in new {
        let key = key_of(line);
        if linked.contains(key.as_str()) {
            continue;
        }
        let Some(m) = all.iter().find(|m| m.has(&key)) else { continue };
        if !m.sources.iter().any(|s| matches!(s.kind, Kind::Preset | Kind::Line | Kind::Letter)) {
            continue;
        }
        let Some(expected) = steadiest(m.sources.iter().filter(|s| !matches!(s.kind, Kind::Bank | Kind::Hold))) else { continue };
        let probable = m.sources.iter().find(|s| s.key == key).is_some_and(|s| s.probable);
        keep.push((expected.to_string(), key, probable));
    }
    if keep.is_empty() {
        return Ok(0);
    }
    Store::edit(root, |edit| {
        for (movement, source, probable) in &keep {
            edit.link(movement, source, if *probable { "probable" } else { "sure" }, None);
        }
    })?;
    Ok(keep.len())
}

/// How a message about money is matched: a receipt or an order as a card
/// payment, a bill or a debit notice by its message's day, money received
/// as a transfer, money given back as a refund.
fn evidence_of(kind: PaymentKind) -> Evidence {
    match kind {
        PaymentKind::Paid | PaymentKind::Order => Evidence::Card,
        PaymentKind::Bill => Evidence::Notice,
        PaymentKind::Received => Evidence::Transfer,
        PaymentKind::Refund => Evidence::Refund,
    }
}

/// The messages about money in `mail` that tell of a movement already
/// counted join it as a source, without a question: each pair kept as a
/// link of the store with what the message said (its amount, its day, who),
/// and the message said "the same payment as …" (`MailState::Joined`). A
/// purchase in instalments says how many on its link; an order charged in
/// two or three parts joins the first, the others together with it. What is
/// recorded already, said not to be a payment, or the same as another
/// message is left as it is; a message from a sender not verified stays
/// proposed, to be checked first. Returns how many links were written.
pub fn join_mails(root: &Path, words: &crate::words::Words, mail: &mut [MailLine], today: Date) -> Result<usize, String> {
    let open = |l: &MailLine| l.state == MailState::Proposed && !l.doubtful && l.signed().is_some();
    if !mail.iter().any(open) {
        return Ok(0);
    }
    let days: Vec<Date> = mail.iter().filter(|l| open(l)).map(|l| l.date).collect();
    let (from, to) = (shifted(days.iter().min().copied().unwrap_or(today), -70), shifted(days.iter().max().copied().unwrap_or(today).max(today), 70));
    // The books' movements (`books`, made again only when a file changed), around the mail's days.
    let movements = Movements { list: super::books(root, words, today).between(from, to).cloned().collect() };
    let store = Store::load(root).unwrap_or_default();
    let joined_state = |m: &Movement, probable: bool| MailState::Joined { address: m.address(), label: m.label.clone(), day: m.day, probable };
    // Joined before: said so at once.
    for line in mail.iter_mut().filter(|l| open(l)) {
        if let Some(link) = store.links.iter().find(|l| l.source == line.key)
            && let Some(m) = movements.find(&link.movement)
        {
            line.state = joined_state(m, link.how == "probable");
        }
    }
    let waiting: Vec<usize> = (0..mail.len()).filter(|&i| open(&mail[i])).collect();
    if waiting.is_empty() {
        return Ok(0);
    }
    let bank_words = &words.bank;
    let facts: Vec<usize> = (0..movements.list.len()).filter(|&j| movements.list[j].way != Way::Between && movements.list[j].place != Place::Unplaced).collect();
    let expected: Vec<crate::matching::Side> = waiting
        .iter()
        .map(|&i| {
            let l = &mail[i];
            let signed = l.signed().unwrap_or_default();
            crate::matching::Side {
                way: way_of(signed),
                sum: Sum::Exact(signed),
                currency: l.payment.amount.as_ref().map_or("EUR", |a| a.currency).to_string(),
                day: l.due.unwrap_or(l.date),
                evidence: if l.due.is_some() { Evidence::Bill } else { evidence_of(l.payment.kind) },
                accounts: Vec::new(),
                names: [l.payment.party.as_str(), l.label.as_str()].iter().map(|n| crate::matching::tokens(bank_words, n)).collect(),
                refs: l.refs.clone(),
            }
        })
        .collect();
    let sides: Vec<crate::matching::Side> = facts
        .iter()
        .map(|&j| {
            let m = &movements.list[j];
            crate::matching::Side { way: m.way, sum: m.sum, currency: m.currency.clone(), day: m.day, evidence: Evidence::Transfer, accounts: Vec::new(), names: m.names.iter().map(|n| crate::matching::tokens(bank_words, n)).collect(), refs: m.refs.clone() }
        })
        .collect();
    let idf = crate::matching::Idf::of(expected.iter().chain(sides.iter()).flat_map(|s| s.names.iter()));
    let pairs = crate::matching::assign(&expected, &sides, &idf, &|_, f| !movements.list[facts[f]].has_kind(Kind::Mail) || movements.list[facts[f]].sources.iter().all(|s| s.kind != Kind::Mail || !store.links.iter().any(|l| l.source == s.key)));
    let mut links: Vec<(String, String, bool, Money, Date, String, Vec<String>, Option<u32>)> = Vec::new();
    let mut taken: BTreeSet<usize> = BTreeSet::new();
    for pair in &pairs {
        let (l, m) = (&mail[waiting[pair.expected]], &movements.list[facts[pair.fact]]);
        taken.insert(facts[pair.fact]);
        links.push((m.steady_key().to_string(), l.key.clone(), pair.outcome == crate::matching::Outcome::Probable, l.signed().unwrap_or_default(), l.date, l.label.clone(), Vec::new(), l.instalments));
    }
    // An order charged in two or three parts: the receipt joins the first, the others together with it.
    let paired: BTreeSet<usize> = pairs.iter().map(|p| p.expected).collect();
    for (e, &i) in waiting.iter().enumerate().filter(|(e, _)| !paired.contains(e)) {
        let l = &mail[i];
        if expected[e].evidence != Evidence::Card {
            continue;
        }
        let (start, until) = Evidence::Card.window(l.date);
        let candidates: Vec<(usize, Money)> = facts
            .iter()
            .enumerate()
            .filter(|&(f, &j)| !taken.contains(&j) && movements.list[j].has_kind(Kind::Bank) && sides[f].way == expected[e].way && sides[f].day >= start && sides[f].day <= until)
            .filter(|(f, _)| sides[*f].names.iter().any(|n| expected[e].names.iter().any(|x| crate::matching::similarity(x, n, &idf) >= 0.5)))
            .map(|(_, &j)| (j, movements.list[j].sum.amount()))
            .collect();
        if let Some(set) = crate::matching::adding_up(l.signed().unwrap_or_default(), &candidates, 100) {
            let mut set = set;
            set.sort_by_key(|&j| movements.list[j].day);
            taken.extend(set.iter().copied());
            let others: Vec<String> = set.iter().skip(1).map(|&j| movements.list[j].steady_key().to_string()).collect();
            links.push((movements.list[set[0]].steady_key().to_string(), l.key.clone(), true, l.signed().unwrap_or_default(), l.date, l.label.clone(), others, None));
        }
    }
    if links.is_empty() {
        return Ok(0);
    }
    Store::edit(root, |edit| {
        for (movement, source, probable, amount, day, label, others, instalments) in &links {
            edit.link(movement, source, if *probable { "probable" } else { "sure" }, Some((*amount, *day, label.as_str())));
            if !others.is_empty() || instalments.is_some() {
                edit.link_together(source, others, *instalments);
            }
        }
    })?;
    for (movement, source, probable, ..) in &links {
        if let (Some(line), Some(m)) = (mail.iter_mut().find(|l| &l.key == source), movements.find(movement)) {
            line.state = joined_state(m, *probable);
        }
    }
    Ok(links.len())
}

/// What moving a movement did, for the page to say: the budget that holds
/// more now ("Home holds €58 more for November"), how much, from the day
/// whose period it is; and the payee to ask about, once, when it was never
/// asked (`decide`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub budget: String,
    pub more: Money,
    pub day: Date,
    pub ask: Option<String>,
}

/// The payee's name a movement's next ones are known by, as the matching
/// compares names: its bank line's label, else its own.
pub fn payee(words: &crate::words::Words, m: &Movement) -> String {
    crate::matching::tokens(&words.bank, m.bank_label.as_deref().unwrap_or(&m.label)).join(" ")
}

/// Each budget's share of a movement placed so.
fn shares(place: &Place, amount: Money) -> Vec<(String, Money)> {
    match place {
        Place::Budget(b) | Place::Reserve { budget: b, .. } => vec![(b.clone(), amount)],
        Place::Parts(parts) => parts.clone(),
        Place::Transfer { .. } | Place::Nowhere | Place::Unplaced => Vec::new(),
    }
}

/// A movement placed by your hand (`to`: a budget, parts across budgets, or
/// none), written where it holds: a declared account's line as the budget
/// file's `[[assign]]`, which every version reads; anything else as the
/// store's `[[move]]`, by its steadiest key. Any budget may be chosen,
/// whatever the hour: what a page shows is no limit on what a movement may be given to.
pub fn move_to(root: &Path, words: &crate::words::Words, movement: &Movement, to: &Place) -> Result<Moved, String> {
    let path = root.join(crate::budget::LEDGER);
    let ledger = Ledger::load(root).unwrap_or_default();
    let known = |budget: &str| ledger.budgets.iter().any(|b| b.id == budget);
    if let Some(unknown) = to.budgets().into_iter().find(|b| !known(b)) {
        return Err(format!("{}: no budget {unknown}", path.display()));
    }
    let line = movement.sources.iter().find(|s| s.kind == Kind::Bank).and_then(|s| {
        let (account, id) = s.key.strip_prefix("bank:")?.rsplit_once('/')?;
        ledger.bank_accounts.iter().any(|a| a.id == account).then(|| (account.to_string(), id.to_string()))
    });
    match (&line, to) {
        (Some((account, id)), Place::Budget(b)) => accounts::set_assignment(&path, account, id, &format!("budget:{b}"))?,
        (Some((account, id)), Place::Nowhere) => accounts::set_assignment(&path, account, id, "none")?,
        (Some((account, id)), Place::Parts(parts)) => accounts::set_parts(&path, account, id, parts)?,
        (None, Place::Budget(b)) => Store::edit(root, |e| e.place(movement.steady_key(), Some(b), &[], false))?,
        (None, Place::Parts(parts)) => Store::edit(root, |e| e.place(movement.steady_key(), None, parts, false))?,
        (None, Place::Nowhere) => Store::edit(root, |e| e.place(movement.steady_key(), None, &[], true))?,
        _ => return Err("a budget, parts between budgets, or none".into()),
    }
    // The effect: what each budget holds more, or less, now.
    let amount = movement.sum.amount();
    let mut more: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    for (budget, share) in shares(&movement.place, amount) {
        *more.entry(budget).or_insert(0) -= share.cents();
    }
    for (budget, share) in shares(to, amount) {
        *more.entry(budget).or_insert(0) += share.cents();
    }
    let (budget, cents) = more.into_iter().max_by_key(|(_, cents)| *cents).unwrap_or_default();
    let payee = payee(words, movement);
    let store = Store::load(root).unwrap_or_default();
    let ask = (!payee.is_empty() && matches!(to, Place::Budget(_)) && !store.decided.iter().any(|d| d.payee == payee)).then_some(payee);
    Ok(Moved { budget, more: Money(cents.max(0)), day: movement.day, ask })
}

/// Your answer, once, on whether a payee's next movements go to `budget`
/// too: kept, so it is never asked again; a yes is also a rule of the budget
/// file (`[[split]]`: the payee's words, its way, to that budget), which every
/// version reads and the bank's rules list shows.
pub fn decide(root: &Path, payee: &str, budget: &str, way: Way, next: bool) -> Result<(), String> {
    Store::edit(root, |e| e.decide(payee, budget, next))?;
    if next && !payee.trim().is_empty() {
        let direction = match way {
            Way::In => "credit",
            Way::Out => "debit",
            Way::Between => "",
        };
        accounts::save_split(&root.join(crate::budget::LEDGER), None, &accounts::SplitEdit { account: String::new(), words: vec![payee.to_string()], direction: direction.into(), to: format!("budget:{budget}") })?;
    }
    Ok(())
}

/// "Not the same": `a` and `b` (a movement's steadiest key, a source's key)
/// are never one movement again, whatever the matching says.
pub fn not_same(root: &Path, a: &str, b: &str) -> Result<(), String> {
    Store::edit(root, |e| e.apart(a, b))
}

/// The same payment, by your hand: `other` joins `movement`; the bank's name
/// of the payee is taught the name it goes by, for its next ones.
pub fn same(root: &Path, words: &crate::words::Words, movement: &Movement, other: &Movement) -> Result<(), String> {
    let (bank, known) = match (movement.bank_label.as_ref(), other.bank_label.as_ref()) {
        (Some(_), None) => (payee(words, movement), other.label.clone()),
        (None, Some(_)) => (payee(words, other), movement.label.clone()),
        _ => (String::new(), String::new()),
    };
    Store::edit(root, |e| {
        e.together(movement.steady_key(), other.steady_key());
        e.link(movement.steady_key(), other.steady_key(), "hand", None);
        if !bank.is_empty() && !known.trim().is_empty() && crate::matching::tokens(&words.bank, &known).join(" ") != bank {
            e.name(&bank, known.trim());
        }
    })
}

/// "A payment…": what a message or a letter says of a payment, for a short
/// form to fill (docs/accounting.md, "A payment from any mail"): its amount
/// (signed: + in, − out; 0 when it says none), its day, who, and its
/// source's address; any budget may be chosen. From a sender not verified,
/// it says to check it first.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Payment {
    pub amount: f64,
    pub day: String,
    pub label: String,
    pub source: String,
    #[serde(default)]
    pub budget: String,
    #[serde(default)]
    pub doubtful: bool,
}

impl Payment {
    /// From a message, whatever it is: what it says of a payment
    /// (`payments::detect`, else its amount read as a payment made), what its
    /// attached invoice says of itself, its day.
    pub fn from_card(words: &crate::words::Words, card: &crate::card::Card, key: &str, doubtful: bool) -> Payment {
        let payment = crate::payments::detect(words, card).unwrap_or_else(|| crate::payments::read(words, card, PaymentKind::Paid, &crate::payments::start(card)));
        let invoice = crate::payments::invoice_of(card);
        let amount = payment.amount.as_ref().map(|a| a.money).or_else(|| invoice.as_ref().and_then(|i| i.amount)).map_or(0.0, |m| m.abs().cents() as f64 / 100.0);
        let signed = if payment.kind.is_credit() { amount } else { -amount };
        let sent = card.date.and_then(|s| jiff::Timestamp::from_second(s).ok()).map(|t| t.to_zoned(crate::zone::current()).date());
        let day = invoice.as_ref().and_then(|i| i.due).or(sent).map(|d| d.to_string()).unwrap_or_default();
        let label = if payment.party == card.sender() { invoice.as_ref().map(|i| i.seller.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or(payment.party) } else { payment.party };
        Payment { amount: signed, day, label, source: key.to_string(), budget: String::new(), doubtful }
    }

    /// From a mail line the budgets page lists (`budget::mail_lines`): its amount, day, who, and the budget its rule gives.
    pub fn from_mail(line: &MailLine) -> Payment {
        Payment {
            amount: line.signed().map_or(0.0, |m| m.cents() as f64 / 100.0),
            day: line.due.unwrap_or(line.date).to_string(),
            label: line.label.clone(),
            source: line.key.clone(),
            budget: line.budget.clone().unwrap_or_default(),
            doubtful: line.doubtful,
        }
    }

    /// From a paper letter: its amount to pay, its date asked (else the day it came), who sent it.
    pub fn from_letter(letter: &Letter) -> Payment {
        let day = letter.reading.deadline.or(letter.received).map(|d| d.to_string()).unwrap_or_default();
        Payment { amount: letter.reading.amount.map_or(0.0, |m| -(m.abs().cents() as f64) / 100.0), day, label: letter.reading.sender.clone(), source: crate::links::letter_uri(&letter.id), budget: String::new(), doubtful: false }
    }

    /// Written into the budget file as a line of `budget`, planned when its
    /// day is to come, the message or letter in its links, with a comment
    /// saying where it came from (`origin`). Returns the line's id.
    pub fn record(&self, root: &Path, today: Date, origin: &str) -> Result<String, String> {
        let path = root.join(crate::budget::LEDGER);
        let fail = |e: &str| format!("{}: {e}", path.display());
        if !self.amount.is_finite() || self.amount == 0.0 {
            return Err(fail("an amount is needed"));
        }
        let day: Date = self.day.trim().parse().map_err(|_| fail("a day is needed"))?;
        let ledger = Ledger::load(root).map_err(|e| fail(&e))?;
        if !ledger.budgets.iter().any(|b| b.id == self.budget) {
            return Err(fail("a budget is needed"));
        }
        let id = crate::ids::new();
        let label = if self.label.trim().is_empty() { self.source.clone() } else { self.label.trim().to_string() };
        let line = Line {
            id: id.clone(),
            budget: self.budget.clone(),
            date: day,
            amount: Money::from_units(self.amount),
            label,
            planned: day > today,
            reserve: None,
            links: vec![self.source.clone()],
            preset: None,
        };
        crate::budget::record_line(&path, &line, origin)?;
        Ok(id)
    }
}

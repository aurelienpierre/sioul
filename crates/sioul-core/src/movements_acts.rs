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
    // What its rows say of their fees, by their keys in the store (never in
    // `sioul-bank.toml`, which every version writes whole), and the export
    // itself, whose day tells when Sioul was told of each of its lines.
    let ledger = Ledger::load(root).unwrap_or_default();
    let card = accounts::account_of(&ledger, &account).map_or_else(|| account.clone(), |a| a.id.clone());
    let processor = accounts::account_of(&ledger, &account).and_then(|a| crate::processors::known().by_id(a.kind())).map(|p| p.id.clone());
    let days: Vec<Date> = statement.movements.iter().map(|m| m.date).collect();
    if let (Some(first), Some(last)) = (days.iter().min().copied(), days.iter().max().copied()) {
        Store::edit(root, |edit| {
            // Its new lines' only: the same export twice changes nothing.
            for row in statement.fees.iter().filter(|r| !known.contains(&(account.clone(), r.id.clone()))) {
                let kind = if row.bank { FeeKind::Bank } else { FeeKind::Processor };
                let told = FeeTold { gross: row.gross, currency: None, fees: vec![Fee { kind, amount: row.fee, from: FeeFrom::Export, about: false }], paid_on: None, processor: processor.clone() };
                edit.fee(&bank_key(&card, &row.id), &told);
            }
            edit.import(&card, first, last);
        })?;
    }
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
    // A payment recorded from its mail waits for the bank too: a payout may carry it.
    let recorded = |l: &MailLine| l.state == MailState::Recorded && !l.doubtful && l.signed().is_some();
    recorded_from_processors(root, mail)?;
    if !mail.iter().any(|l| open(l) || recorded(l)) {
        return Ok(0);
    }
    let days: Vec<Date> = mail.iter().filter(|l| open(l) || recorded(l)).map(|l| l.date).collect();
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
            continue;
        }
        // Carried by a payout before.
        if let Some(entry) = store.carried.iter().find(|c| c.payments.iter().any(|p| p.source == line.key))
            && let Some(m) = movements.find(&entry.payout)
        {
            line.state = joined_state(m, entry.how != "hand");
        }
    }
    let waiting: Vec<usize> = (0..mail.len()).filter(|&i| open(&mail[i])).collect();
    if waiting.is_empty() && !mail.iter().any(recorded) {
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
            // A processor's mail tells what the payer paid: compared with what was paid, not with what arrived.
            let sum = m.gross.filter(|_| m.gross_currency.is_none()).map_or(m.sum, Sum::Exact);
            crate::matching::Side { way: m.way, sum, currency: m.currency.clone(), day: m.day, evidence: Evidence::Transfer, accounts: Vec::new(), names: m.names.iter().map(|n| crate::matching::tokens(bank_words, n)).collect(), refs: m.refs.clone() }
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
    // What a processor's mail told of the payments it joined: what the payer
    // paid, the day, its own fee line when it states one.
    let processors = crate::processors::known();
    let processor_of = |l: &MailLine| l.sender_domain.as_deref().and_then(|d| processors.by_domain(d));
    let mut told: Vec<(String, FeeTold)> = Vec::new();
    for pair in &pairs {
        let l = &mail[waiting[pair.expected]];
        let processor = processor_of(l);
        if processor.is_none() && l.payment.fee.is_none() {
            continue;
        }
        let fees = l.payment.fee.iter().map(|f| Fee { kind: FeeKind::Processor, amount: f.money, from: FeeFrom::Mail, about: false }).collect();
        let paid_on = (l.payment.kind == PaymentKind::Received).then_some(l.date);
        told.push((l.key.clone(), FeeTold { gross: l.signed(), currency: None, fees, paid_on, processor: processor.map(|p| p.id.clone()) }));
    }
    let used: BTreeSet<usize> = pairs.iter().map(|p| waiting[p.expected]).chain(links.iter().filter_map(|(_, source, ..)| mail.iter().position(|l| &l.key == source))).collect();
    let fact_taken: BTreeSet<usize> = taken.clone();
    let platform = platform_invoices(words, mail, &used, &movements, &fact_taken, &store);
    for (movement, service, fee) in &platform.pairs {
        let (s, f) = (&mail[*service], &mail[*fee]);
        let gross = s.payment.amount.as_ref().map_or(Money::ZERO, |a| a.money.abs());
        let kept = f.payment.amount.as_ref().map_or(Money::ZERO, |a| a.money.abs());
        let processor = processor_of(s).map(|p| p.id.clone());
        links.push((movement.clone(), s.key.clone(), false, gross, s.date, s.label.clone(), Vec::new(), None));
        links.push((movement.clone(), format!("{FEE_INVOICE}{}", f.key), false, -kept, f.date, f.label.clone(), Vec::new(), None));
        told.push((s.key.clone(), FeeTold { gross: Some(gross), currency: None, fees: Vec::new(), paid_on: None, processor: processor.clone() }));
        told.push((format!("{FEE_INVOICE}{}", f.key), FeeTold { gross: None, currency: None, fees: vec![Fee { kind: FeeKind::Commission, amount: kept, from: FeeFrom::Invoice, about: false }], paid_on: None, processor }));
    }
    let used: BTreeSet<usize> = used.into_iter().chain(platform.pairs.iter().flat_map(|(_, s, f)| [*s, *f])).collect();
    let carried = payouts_carried(mail, &used, &movements, &fact_taken.into_iter().chain(platform.payouts).collect(), &store);
    if links.is_empty() && carried.is_empty() {
        return Ok(0);
    }
    Store::edit(root, |edit| {
        for (movement, source, probable, amount, day, label, others, instalments) in &links {
            edit.link(movement, source, if *probable { "probable" } else { "sure" }, Some((*amount, *day, label.as_str())));
            if !others.is_empty() || instalments.is_some() {
                edit.link_together(source, others, *instalments);
            }
        }
        for (source, told) in &told {
            edit.fee(source, told);
        }
        for (payout, payments) in &carried {
            edit.carry(payout, "oldest", payments);
        }
    })?;
    for (movement, source, probable, ..) in &links {
        // A fee invoice is its message.
        let source = source.strip_prefix(FEE_INVOICE).unwrap_or(source);
        if let (Some(line), Some(m)) = (mail.iter_mut().find(|l| l.key == source), movements.find(movement)) {
            line.state = joined_state(m, *probable);
        }
    }
    for (payout, payments) in &carried {
        let Some(m) = movements.find(payout) else { continue };
        for p in payments {
            if let Some(line) = mail.iter_mut().find(|l| l.key == p.source) {
                line.state = joined_state(m, true);
            }
        }
    }
    Ok(links.len() + carried.iter().map(|(_, p)| p.len()).sum::<usize>())
}

/// A payment you recorded from a processor's mail (money received): what
/// the payer paid, its day and the processor kept once (`[[fee]]`), so that
/// until its payout comes its line says what may arrive, about, after the
/// processor's fee (design §1.3: "on PayPal, not on My bank yet").
fn recorded_from_processors(root: &Path, mail: &[MailLine]) -> Result<(), String> {
    let processors = crate::processors::known();
    let recorded: Vec<&MailLine> = mail.iter().filter(|l| l.state == MailState::Recorded && l.payment.kind == PaymentKind::Received && !l.doubtful).collect();
    if recorded.is_empty() {
        return Ok(());
    }
    let store = Store::load(root).unwrap_or_default();
    let told: Vec<(String, FeeTold)> = recorded
        .into_iter()
        .filter(|l| !store.fees.iter().any(|f| f.source == l.key))
        .filter_map(|l| {
            let p = processors.by_domain(l.sender_domain.as_deref()?)?;
            let fees = l.payment.fee.iter().map(|f| Fee { kind: FeeKind::Processor, amount: f.money, from: FeeFrom::Mail, about: false }).collect();
            Some((l.key.clone(), FeeTold { gross: l.signed(), currency: None, fees, paid_on: Some(l.date), processor: Some(p.id.clone()) }))
        })
        .collect();
    if told.is_empty() {
        return Ok(());
    }
    Store::edit(root, |edit| {
        for (source, told) in &told {
            edit.fee(source, told);
        }
    })
}

/// A platform that bills its commission apart (Malt; `Processor::fee_invoice`):
/// its service invoice (what the client paid) and its fee invoice (what it
/// kept), a few days apart, whose difference is to the cent a payout of its
/// on the bank's line. The mails, by their places in `mail`, and the payouts, by theirs.
struct Platform {
    pairs: Vec<(String, usize, usize)>,
    payouts: Vec<usize>,
}

fn platform_invoices(words: &crate::words::Words, mail: &[MailLine], used: &BTreeSet<usize>, movements: &Movements, taken: &BTreeSet<usize>, store: &Store) -> Platform {
    let processors = crate::processors::known();
    let fee_words: Vec<String> = words.payments.fee.iter().map(|w| crate::words::folded(w)).filter(|w| !w.is_empty()).collect();
    let is_fee = |l: &MailLine| {
        let text: Vec<char> = crate::text::fold(&format!("{} {}", l.subject, l.label));
        fee_words.iter().any(|w| crate::text::find_word(&text, w, 0).is_some())
    };
    let usable = |i: usize| !used.contains(&i) && !mail[i].doubtful && matches!(mail[i].state, MailState::Proposed) && mail[i].payment.amount.as_ref().is_some_and(|a| a.currency == "EUR" && a.money.cents() != 0);
    let mut out = Platform { pairs: Vec::new(), payouts: Vec::new() };
    let mut gone: BTreeSet<usize> = BTreeSet::new();
    for p in processors.processors.iter().filter(|p| p.fee_invoice) {
        let theirs: Vec<usize> = (0..mail.len()).filter(|&i| usable(i) && mail[i].sender_domain.as_deref().is_some_and(|d| p.sends_from(d))).collect();
        let (fees, services): (Vec<usize>, Vec<usize>) = theirs.into_iter().partition(|&i| is_fee(&mail[i]));
        let payouts = (0..movements.list.len()).filter(|&j| !taken.contains(&j) && is_payout(&movements.list[j], p, store));
        for j in payouts {
            let m = &movements.list[j];
            let net = m.sum.amount().cents();
            let found = services.iter().filter(|s| !gone.contains(s)).find_map(|&s| {
                let (gross, day) = (mail[s].payment.amount.as_ref()?.money.abs().cents(), mail[s].date);
                if day > shifted(m.day, 1) || day < shifted(m.day, -62) {
                    return None;
                }
                fees.iter().filter(|f| !gone.contains(f)).find(|&&f| mail[f].payment.amount.as_ref().is_some_and(|a| (gross - a.money.abs().cents() - net).abs() <= 1) && days_apart(mail[f].date, day) <= 10).map(|&f| (s, f))
            });
            if let Some((s, f)) = found {
                gone.extend([s, f]);
                out.pairs.push((m.steady_key().to_string(), s, f));
                out.payouts.push(j);
            }
        }
    }
    out
}

/// Whether `m` is one of `p`'s payouts that nothing joined yet: money in,
/// the bank's line alone, its label naming the processor, not carried yet.
fn is_payout(m: &Movement, p: &crate::processors::Processor, store: &Store) -> bool {
    m.way == Way::In && m.sources.len() == 1 && m.has_kind(Kind::Bank) && m.bank_label.as_deref().is_some_and(|l| p.named_in(l)) && !store.carried.iter().any(|c| m.has(&c.payout))
}

/// The payouts on the bank's line of a processor whose export you do not
/// take in (docs/accounting.md, "Payouts"), each carrying the payments its
/// mail told of since its previous payout, oldest first, as far as its
/// amount goes, the fees within the processor's bounds
/// (`matching::carried_prefix`). Messages already joined, said not to be
/// payments, or from a sender not verified are never carried. Each payout
/// by its steadiest key, with what each payment's message said.
fn payouts_carried(mail: &[MailLine], used: &BTreeSet<usize>, movements: &Movements, taken: &BTreeSet<usize>, store: &Store) -> Vec<(String, Vec<CarriedPayment>)> {
    let processors = crate::processors::known();
    let carried_before: BTreeSet<&str> = store.carried.iter().flat_map(|c| c.payments.iter().map(|p| p.source.as_str())).collect();
    // A recorded payment waits for the bank while its line's movement has no bank line yet.
    let waits = |l: &MailLine| match l.state {
        MailState::Proposed => true,
        MailState::Recorded => movements.list.iter().find(|m| m.has(&l.key)).is_some_and(expects),
        _ => false,
    };
    let mut out = Vec::new();
    for p in &processors.processors {
        let mut told: Vec<usize> = (0..mail.len())
            .filter(|&i| {
                let l = &mail[i];
                !used.contains(&i) && !l.doubtful && waits(l) && !carried_before.contains(l.key.as_str()) && l.payment.kind.is_credit() && l.payment.kind != PaymentKind::Refund
            })
            .filter(|&i| mail[i].sender_domain.as_deref().is_some_and(|d| p.sends_from(d)) && mail[i].payment.amount.as_ref().is_some_and(|a| a.currency == "EUR" && a.money.cents() > 0))
            .collect();
        if told.is_empty() {
            continue;
        }
        told.sort_by_key(|&i| (mail[i].date, i));
        let mut payouts: Vec<usize> = (0..movements.list.len()).filter(|&j| !taken.contains(&j) && is_payout(&movements.list[j], p, store)).collect();
        payouts.sort_by_key(|&j| movements.list[j].day);
        let mut left = told;
        for j in payouts {
            let m = &movements.list[j];
            let before: Vec<usize> = left.iter().copied().filter(|&i| mail[i].date <= m.day).collect();
            let grosses: Vec<Money> = before.iter().map(|&i| mail[i].payment.amount.as_ref().map_or(Money::ZERO, |a| a.money)).collect();
            let Some(k) = crate::matching::carried_prefix(m.sum.amount(), &grosses, &|g, n| p.bounds.of(g, n)) else { continue };
            let payments: Vec<CarriedPayment> = before[..k]
                .iter()
                .map(|&i| {
                    let l = &mail[i];
                    CarriedPayment { source: l.key.clone(), gross: l.payment.amount.as_ref().map_or(Money::ZERO, |a| a.money.abs()), day: l.date, label: l.label.clone(), fee: l.payment.fee.as_ref().map(|f| f.money.abs()) }
                })
                .collect();
            left.retain(|i| !before[..k].contains(i));
            out.push((m.steady_key().to_string(), payments));
        }
    }
    out
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
        // A share set aside has no payee's name to teach.
        _ if movement.has_kind(Kind::SetAside) || other.has_kind(Kind::SetAside) => (String::new(), String::new()),
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
            ..Line::default()
        };
        crate::budget::record_line(&path, &line, origin)?;
        Ok(id)
    }
}

// ---------------------------------------------------------------- fees and payouts

/// The payments a payout carried as the store says it (`[[carried]]`):
/// each one's own movement (a line recorded from its message) made one with
/// the payout, counted once, or its message a source of the payout; the
/// payout says what it carried. A payment you said is not part of it ("Not
/// a payout", `[[apart]]` with the payout) is left out.
pub(super) fn carry_from_store(out: &mut Vec<Movement>, store: &Store, apart: &dyn Fn(&str, &str) -> bool) {
    for entry in &store.carried {
        if !out.iter().any(|m| m.has(&entry.payout) && m.has_kind(Kind::Bank)) {
            continue;
        }
        let probable = entry.how != "hand";
        let mut carried = Vec::new();
        for pay in entry.payments.iter().filter(|p| !apart(&entry.payout, &p.source)) {
            let Some(at) = out.iter().position(|m| m.has(&entry.payout)) else { break };
            match out.iter().position(|m| m.has(&pay.source)) {
                Some(own) if own == at => {}
                Some(own) => {
                    let other = out.remove(own);
                    let at = if own < at { at - 1 } else { at };
                    merge(&mut out[at], other, probable);
                }
                None => {
                    if let Some(mut source) = Source::of_key(&pay.source) {
                        source.probable = probable;
                        out[at].sources.push(source);
                    }
                }
            }
            let fee = pay.fee.map(|f| Fee { kind: FeeKind::Processor, amount: f.abs(), from: FeeFrom::Mail, about: false });
            carried.push(Carried { key: pay.source.clone(), day: pay.day, label: pay.label.clone(), gross: pay.gross.abs(), fee, how: entry.how.clone() });
        }
        if let Some(m) = out.iter_mut().find(|m| m.has(&entry.payout)) {
            m.carried = carried;
        }
    }
}

/// One record of what was paid and kept, from one source, the most exact first (`FeeFrom`).
struct Record {
    rank: FeeFrom,
    gross: Option<Money>,
    currency: Option<String>,
    fees: Vec<Fee>,
    paid_on: Option<Date>,
    processor: Option<String>,
}

/// What every movement was paid, its fees and the day it was paid
/// (docs/accounting.md, "Fees"), from the most exact record at hand: your
/// hand (a line's own), the processor's export, a fee invoice, the mail's own
/// fee line, a payout's difference; the published rule, or your own rate on
/// the account, only as "about", for what is still expected. Each kind of
/// fee from its most exact record. Figures that do not add up keep the
/// bank's net and say the fees are not known yet: nothing is made up. Then a
/// payout of a processor's account says the payments its export shows it
/// carried; your words (`[[edit]]`); the days it was made and changed.
pub(super) fn settle_fees(out: &mut [Movement], input: &Input, lines: &HashMap<String, &Line>, today: Date) {
    let store = input.store;
    let processors = crate::processors::known();
    let country = crate::processors::country_of(input.words);
    let entries: HashMap<&str, &FeeEntry> = store.fees.iter().map(|e| (e.source.as_str(), e)).collect();
    let account_processor = |account: Option<&str>| -> Option<&crate::processors::Processor> {
        let card = input.ledger.bank_accounts.iter().find(|a| Some(a.id.as_str()) == account)?;
        processors.by_id(card.kind())
    };
    // Each account's own rate: the median of its past year's exact fees, as a share of what was paid.
    let mut rates: HashMap<String, Vec<f64>> = HashMap::new();
    // What is still expected, for its fee about once the rates are known.
    let mut expected: Vec<(usize, Money, Option<&crate::processors::Processor>)> = Vec::new();
    for (index, m) in out.iter_mut().enumerate() {
        let mut records: Vec<Record> = Vec::new();
        for source in &m.sources {
            if let Some(entry) = entries.get(source.key.as_str()) {
                let fees: Vec<Fee> = entry.fees.iter().map(FeePart::fee).collect();
                let rank = fees.iter().map(|f| f.from).min().unwrap_or(match source.kind {
                    Kind::Bank | Kind::Hold => FeeFrom::Export,
                    Kind::FeeInvoice | Kind::Invoice => FeeFrom::Invoice,
                    Kind::Line => FeeFrom::Hand,
                    _ => FeeFrom::Mail,
                });
                records.push(Record { rank, gross: entry.gross, currency: entry.currency.clone(), fees, paid_on: entry.paid_on, processor: entry.processor.clone() });
            }
            if source.kind == Kind::Line
                && let Some(line) = lines.get(&source.key)
                && (line.gross.is_some() || !line.fees.is_empty() || line.paid_on.is_some())
            {
                let fees = line.fees.iter().map(|f| Fee { kind: FeeKind::parse(&f.kind), amount: f.amount.abs(), from: if f.about { FeeFrom::Rate } else { FeeFrom::Hand }, about: f.about }).collect();
                records.push(Record { rank: FeeFrom::Hand, gross: line.gross, currency: None, fees, paid_on: line.paid_on, processor: None });
            }
        }
        records.sort_by_key(|r| r.rank);
        let net = m.sum.amount();
        let done = matches!(m.state, State::Done | State::NotFinal);
        let mut gross = records.iter().find(|r| r.gross.is_some()).map(|r| (r.gross.unwrap_or(net), r.currency.clone().filter(|c| !c.eq_ignore_ascii_case(&m.currency))));
        // Each kind of fee from its most exact record.
        let mut fees: Vec<Fee> = Vec::new();
        for record in &records {
            for fee in &record.fees {
                if !fees.iter().any(|f| f.kind == fee.kind) {
                    fees.push(*fee);
                }
            }
        }
        let processor = records.iter().find_map(|r| r.processor.as_deref()).and_then(|p| processors.by_id(p)).or_else(|| account_processor(m.account.as_deref())).or_else(|| m.bank_label.as_deref().and_then(|l| processors.by_label(l)));
        // A payout without its processor's export: what it carried is what was paid.
        if gross.is_none() && !m.carried.is_empty() {
            gross = Some((Money(m.carried.iter().map(|c| c.gross.cents()).sum()), None));
        }
        m.fees_unknown = false;
        match &gross {
            Some((g, None)) => {
                let all: Money = fees.iter().map(|f| f.amount).sum();
                let exact = fees.iter().all(|f| !f.about);
                if fees.is_empty() && *g != net {
                    let difference = *g - net;
                    // A payout's or a processor's row: its difference is its fees, within the processor's bounds.
                    let n = m.carried.len().max(1);
                    let bounded = processor.is_some_and(|p| {
                        let (low, high) = p.bounds.of(*g, n);
                        difference >= low && difference <= high
                    });
                    if done && (bounded || !m.carried.is_empty()) && difference.cents() > 0 {
                        fees.push(Fee { kind: if processor.is_some_and(|p| p.kind == "platform") { FeeKind::Commission } else { FeeKind::Processor }, amount: difference, from: FeeFrom::Payout, about: false });
                    } else if !done && records.first().is_some_and(|r| r.rank == FeeFrom::Hand) && difference.cents() > 0 {
                        // Your gross and your amount: the difference is what you said the fees are.
                        fees.push(Fee { kind: FeeKind::Processor, amount: difference, from: FeeFrom::Hand, about: false });
                    } else if done {
                        m.fees_unknown = true;
                    }
                } else if !fees.is_empty() && exact && *g - all != net && done {
                    fees.clear();
                    m.fees_unknown = true;
                }
            }
            Some((_, Some(_))) => {}
            None if !fees.is_empty() && fees.iter().all(|f| !f.about) => {
                gross = Some((net + fees.iter().map(|f| f.amount).sum::<Money>(), None));
            }
            None => {}
        }
        if !done && fees.is_empty() && m.way == Way::In && processor.is_some() {
            expected.push((index, gross.as_ref().map_or(net, |(g, _)| *g), processor));
        }
        if done && let (Some((g, None)), Some(account)) = (&gross, m.account.as_deref()) {
            let exact: Money = fees.iter().filter(|f| f.kind == FeeKind::Processor && !f.about && matches!(f.from, FeeFrom::Export | FeeFrom::Hand)).map(|f| f.amount).sum();
            if exact.cents() > 0 && g.cents() > 0 && m.day >= shifted(today, -365) {
                rates.entry(account.to_string()).or_default().push(exact.cents() as f64 / g.cents() as f64);
            }
        }
        // A payout's fees shared out among what it carried: exact for one
        // payment or one its message said, else each its share, about, by the
        // processor's rule scaled to the payout.
        if !m.carried.is_empty() {
            let total = fees.iter().filter(|f| matches!(f.kind, FeeKind::Processor | FeeKind::Commission)).map(|f| f.amount).sum::<Money>();
            share_out(&mut m.carried, total, processor, country);
        }
        m.paid_on = records.iter().find_map(|r| r.paid_on).or_else(|| (m.carried.len() == 1).then(|| m.carried[0].day)).filter(|d| *d != m.day);
        m.gross = gross.as_ref().map(|(g, _)| *g).filter(|g| *g != net || !fees.is_empty() || m.fees_unknown);
        m.gross_currency = gross.and_then(|(_, c)| c);
        m.fees = fees;
    }
    // What is still expected: about, by your own rate on its account, else the processor's rule.
    for (index, paid, processor) in expected {
        let m = &mut out[index];
        let own = m.account.as_deref().and_then(|a| rates.get(a)).and_then(|r| median(r));
        let about = own.map(|share| Money((paid.cents() as f64 * share).round() as i64)).or_else(|| processor.and_then(|p| p.about_fee(paid, country)));
        if let Some(about) = about.filter(|a| a.cents() > 0) {
            m.fees.push(Fee { kind: FeeKind::Processor, amount: about, from: FeeFrom::Rate, about: true });
            m.gross = Some(paid);
        }
    }
    carried_by_exports(out, input);
    // Your words, then the days it was made and changed.
    let zone = crate::zone::current();
    let day_of = |at: &Option<jiff::Timestamp>| at.map(|t| t.to_zoned(zone.clone()).date());
    for m in out.iter_mut() {
        let edits: Vec<&EditEntry> = store.edits.iter().filter(|e| m.has(&e.movement)).collect();
        for edit in edits {
            if let Some(note) = &edit.note {
                m.note = note.clone();
            }
            if let Some(paid) = edit.paid_on {
                m.paid_on = Some(paid).filter(|d| *d != m.day);
            }
            if let Some(label) = edit.label.as_ref().filter(|l| !l.trim().is_empty()) {
                m.label = label.trim().to_string();
            }
        }
        let mut told: Vec<Date> = Vec::new();
        for source in &m.sources {
            match source.kind {
                Kind::Bank | Kind::Hold => {
                    let account = source.key.strip_prefix("bank:").or_else(|| source.key.strip_prefix("hold:")).and_then(|r| r.rsplit_once('/')).map(|(a, _)| a);
                    told.extend(store.imports.iter().filter(|i| Some(i.account.as_str()) == account && i.first <= m.day && m.day <= i.last).filter_map(|i| day_of(&i.at)));
                }
                Kind::Line => told.extend(lines.get(&source.key).and_then(|l| day_of(&l.made))),
                Kind::Mail | Kind::FeeInvoice => {
                    let key = source.key.strip_prefix(FEE_INVOICE).unwrap_or(&source.key);
                    told.extend(store.links.iter().filter(|l| l.source == source.key).filter_map(|l| l.day));
                    told.extend(m.carried.iter().filter(|c| c.key == key).map(|c| c.day));
                }
                Kind::Letter => told.extend(input.letters.iter().filter(|l| crate::links::letter_uri(&l.id) == source.key).filter_map(|l| l.received)),
                _ => {}
            }
        }
        m.made = told.into_iter().min();
        let names = |key: &str| m.has(key);
        let mut corrected: Vec<Date> = Vec::new();
        corrected.extend(store.edits.iter().filter(|e| names(&e.movement)).filter_map(|e| day_of(&e.at)));
        corrected.extend(store.moves.iter().filter(|e| names(&e.movement)).filter_map(|e| day_of(&e.at)));
        corrected.extend(store.links.iter().filter(|l| l.how == "hand" && (names(&l.movement) || names(&l.source))).filter_map(|l| day_of(&l.at)));
        corrected.extend(store.apart.iter().filter(|a| names(&a.a) || names(&a.b)).filter_map(|a| day_of(&a.at)));
        corrected.extend(store.carried.iter().filter(|c| c.how == "hand" && names(&c.payout)).filter_map(|c| day_of(&c.at)));
        corrected.extend(m.sources.iter().filter_map(|s| entries.get(s.key.as_str())).filter(|e| e.fees.iter().any(|f| f.from == "hand")).filter_map(|e| day_of(&e.at)));
        corrected.extend(m.sources.iter().filter(|s| s.kind == Kind::Line).filter_map(|s| lines.get(&s.key)).filter_map(|l| day_of(&l.changed)));
        m.changed = corrected.into_iter().max().or(m.made);
    }
}

/// The middle of some shares.
fn median(values: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 })
}

/// A payout's fees (`total`) shared out among the payments it carried: a
/// fee a message said kept as it is; the rest exact for one payment, else
/// each its share, about, by the processor's rule (else by what each paid)
/// scaled to what is left.
fn share_out(carried: &mut [Carried], total: Money, processor: Option<&crate::processors::Processor>, country: &str) {
    let stated: Money = carried.iter().filter_map(|c| c.fee.map(|f| f.amount)).sum();
    let left = total - stated;
    let open: Vec<usize> = (0..carried.len()).filter(|&i| carried[i].fee.is_none()).collect();
    if open.is_empty() || left.cents() < 0 {
        return;
    }
    if open.len() == 1 {
        carried[open[0]].fee = Some(Fee { kind: FeeKind::Processor, amount: left, from: FeeFrom::Payout, about: carried.len() > 1 && stated == Money::ZERO });
        return;
    }
    let weight = |c: &Carried| processor.and_then(|p| p.about_fee(c.gross, country)).map_or(c.gross.cents().abs(), |f| f.cents().max(1)) as f64;
    let all: f64 = open.iter().map(|&i| weight(&carried[i])).sum();
    if all <= 0.0 {
        return;
    }
    let mut given = 0i64;
    for (n, &i) in open.iter().enumerate() {
        let share = if n + 1 == open.len() { left.cents() - given } else { (left.cents() as f64 * weight(&carried[i]) / all).round() as i64 };
        given += share;
        carried[i].fee = Some(Fee { kind: FeeKind::Processor, amount: Money(share), from: FeeFrom::Payout, about: true });
    }
}

/// A move out of a processor's account (PayPal's withdrawal, Stripe's
/// payout), its export read: it carried that account's money in since its
/// previous one, as its running balance between them says; each payment
/// says the payout that carried it.
fn carried_by_exports(out: &mut [Movement], input: &Input) {
    let processors = crate::processors::known();
    let processor_account = |id: &str| input.ledger.bank_accounts.iter().find(|a| a.id == id).is_some_and(|a| a.kind() != "bank" && processors.by_id(a.kind()).is_some());
    let mut payouts: Vec<(usize, String, Date)> = out.iter().enumerate().filter_map(|(i, m)| match &m.place {
        Place::Transfer { from, .. } if m.way == Way::Between && processor_account(from) => Some((i, from.clone(), m.day)),
        _ => None,
    }).collect();
    payouts.sort_by_key(|(_, _, day)| *day);
    let mut since: HashMap<String, Date> = HashMap::new();
    for (at, account, day) in payouts {
        let start = since.get(&account).copied();
        let carried: Vec<usize> = (0..out.len())
            .filter(|&j| j != at && out[j].account.as_deref() == Some(account.as_str()) && out[j].way == Way::In && out[j].state == State::Done && out[j].day <= day && start.is_none_or(|s| out[j].day > s) && out[j].carried_by.is_none())
            .collect();
        since.insert(account, day);
        let key = out[at].steady_key().to_string();
        let parts: Vec<Carried> = carried
            .iter()
            .map(|&j| {
                let m = &out[j];
                let fee = m.fees.iter().find(|f| matches!(f.kind, FeeKind::Processor | FeeKind::Commission)).copied();
                Carried { key: m.steady_key().to_string(), day: m.paid_day(), label: m.label.clone(), gross: m.gross_or_net(), fee, how: "export".into() }
            })
            .collect();
        for &j in &carried {
            out[j].carried_by = Some(key.clone());
        }
        out[at].carried = parts;
    }
}

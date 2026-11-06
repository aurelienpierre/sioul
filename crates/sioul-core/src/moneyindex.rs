// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The money index and the history of money mail (the money ledger's design
//! §5, docs/accounting.md, "From mail"): every message about money ever
//! kept on this device, found once, and what became of each.
//!
//! - **Found once** (`Reader`, one of `mailpass`'s readers): a message is
//!   opened only when what the mail cache says of it makes money possible (a
//!   processor's domain, a sender a mail rule names, a money word or an
//!   amount in its subject, an attachment named like an invoice); what it
//!   says of money is then what the budgets page reads of it
//!   (`budget::mail_line_of`), kept in `$XDG_CACHE_HOME/sioul/money-index/`.
//!   Its stamp covers the payment words and the mail rules: when they
//!   change, only this index is made again. It stays on each device and is
//!   never shared; the ties it shows come from the shared store.
//! - **The history** (`history`): every money mail of every address, Sent
//!   mail included, never what is set aside as spam or forged, nor what lies
//!   in the junk or the trash; newest first, in pages of 60, with the
//!   ledger's kind of filters; each row saying in words what became of it
//!   (the same payment as a movement, part of a payout, counted in a
//!   budget, not counted as you said, not tied to any movement). "Tied or
//!   not" is a filter, never the default, never counted.
//! - **"Not a payment" learns the shape** of a sender's mail (`shape`): its
//!   subject with its digits and amounts taken out, kept beside the
//!   messages you said are not payments (`MoneyState::ignored`, as
//!   `shape:<sender>␟<shape>`), so that the next mail of that shape is not
//!   counted either, until you count it again; nothing is asked.

use crate::budget::{Ledger, MailLine, MailRule, MailState};
use crate::i18n::Translator;
use crate::mailcache;
use crate::money::{Amount, Money};
use crate::movements::{Books, Store, Way};
use crate::payments::{Payment, PaymentKind};
use jiff::civil::Date;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The index's folder under the cache.
pub const NAME: &str = "money-index";

/// Rows in a page of the history, as Mail's list holds them.
pub const PAGE: usize = 60;

/// What separates a list column's entries.
const SEP: char = '\u{1f}';

/// The money index's reader (`mailpass::Reader`): the payment words, the
/// mail rules of the budget file, the processors' domains.
pub struct Reader {
    rules: Vec<MailRule>,
    words: crate::words::Words,
    stamp: String,
}

impl Reader {
    pub fn new(rules: &[MailRule], words: &crate::words::Words) -> Reader {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_string(&words.payments).unwrap_or_default().as_bytes());
        hasher.update(serde_json::to_string(&words.money).unwrap_or_default().as_bytes());
        hasher.update(format!("{rules:?}").as_bytes());
        let stamp = hasher.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect();
        Reader { rules: rules.to_vec(), words: words.clone(), stamp }
    }
}

/// Whether an attachment's name looks like an invoice's: a bill's word in
/// it (`Payments::attachments`), or an e-invoice's XML.
fn invoice_named(words: &crate::words::Words, name: &str) -> bool {
    let folded = crate::words::folded(name);
    folded.ends_with(".xml") || words.payments.attachments.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && folded.contains(&w))
}

impl crate::mailpass::Reader for Reader {
    fn name(&self) -> &'static str {
        NAME
    }

    fn stamp(&self) -> String {
        self.stamp.clone()
    }

    /// Money is possible: a processor's domain, a sender a mail rule names, a
    /// payment's word or an amount in the subject, an attachment named like
    /// an invoice. List mail too: a processor's receipts often are.
    fn wanted(&self, line: &mailcache::Line) -> bool {
        let domain = line.from_address.as_deref().and_then(|a| a.rsplit_once('@')).map(|(_, d)| d.to_ascii_lowercase()).unwrap_or_default();
        if crate::processors::known().by_domain(&domain).is_some() {
            return true;
        }
        if line.attachments.iter().any(|a| invoice_named(&self.words, a)) {
            return true;
        }
        if !self.rules.is_empty() {
            let card = line.card(Path::new(""));
            if self.rules.iter().any(|r| r.route.explain(&card).is_some()) {
                return true;
            }
        }
        let subject = crate::text::fold(&line.subject);
        let p = &self.words.payments;
        let said = [&p.paid, &p.received, &p.order, &p.bill, &p.refund, &p.order_words].iter().any(|list| list.iter().any(|w| crate::text::find_word(&subject, w, 0).is_some()));
        said || crate::money::payment_amount(&self.words.money, &line.subject, "").is_some()
    }

    fn read(&self, line: &mailcache::Line, path: &Path, raw: &[u8]) -> Option<Vec<String>> {
        let mut card = crate::card::Card::from_bytes(raw)?;
        card.path = Some(path.to_path_buf());
        let _ = line;
        let found = crate::budget::mail_line_of(&self.rules, &card, &self.words)?;
        Some(columns(&found))
    }
}

fn cents(m: Option<&Amount>) -> String {
    m.map(|a| a.money.cents().to_string()).unwrap_or_default()
}

/// What the index keeps of a message about money: its kind, amount and
/// currency, fee and net, who, the label and the budget and preset its rule
/// gives, its references, the day due, its instalments.
fn columns(l: &MailLine) -> Vec<String> {
    let p = &l.payment;
    vec![
        p.kind.message_id().trim_start_matches("kind-").to_string(),
        cents(p.amount.as_ref()),
        p.amount.as_ref().map(|a| a.currency.to_string()).unwrap_or_default(),
        cents(p.fee.as_ref()),
        cents(p.net.as_ref()),
        p.party.clone(),
        l.label.clone(),
        l.budget.clone().unwrap_or_default(),
        l.preset.clone().unwrap_or_default(),
        l.refs.join(&SEP.to_string()),
        l.due.map(|d| d.to_string()).unwrap_or_default(),
        l.instalments.map(|n| n.to_string()).unwrap_or_default(),
    ]
}

fn kind_of(id: &str) -> Option<PaymentKind> {
    [PaymentKind::Received, PaymentKind::Paid, PaymentKind::Order, PaymentKind::Bill, PaymentKind::Refund].into_iter().find(|k| k.message_id().trim_start_matches("kind-") == id)
}

/// A currency Sioul names, as the money's own (`money::Amount` keeps a name it knows).
fn currency(code: &str) -> &'static str {
    ["EUR", "USD", "GBP", "CHF", "CAD"].into_iter().find(|c| c.eq_ignore_ascii_case(code)).unwrap_or("EUR")
}

/// The mail line the index's columns say, for the message of `card`; none
/// when they do not read (a file of another version: read again then).
pub fn mail_line(columns: &[String], card: &crate::card::Card, doubtful: bool) -> Option<MailLine> {
    if columns.len() < 12 {
        return None;
    }
    let amount = |at: usize| columns[at].parse::<i64>().ok().map(|c| Amount { money: Money(c), currency: currency(&columns[2]) });
    let date = jiff::Timestamp::from_second(card.date?).ok()?.to_zoned(crate::zone::current()).date();
    let digest = || crate::mailfind::address_of_file(card.account.as_deref()?, card.path.as_deref()?);
    let key = card.message_id.as_ref().map(|id| format!("mid:{id}")).or_else(digest).or_else(|| card.path.as_ref().map(|p| format!("file:{}", p.display())))?;
    let text = |at: usize| Some(columns[at].clone()).filter(|t| !t.is_empty());
    Some(MailLine {
        key,
        path: card.path.clone(),
        date,
        payment: Payment { kind: kind_of(&columns[0])?, amount: amount(1), party: columns[5].clone(), fee: amount(3), net: amount(4) },
        subject: card.subject.clone(),
        sender: card.sender().to_string(),
        sender_domain: card.sender_domain().map(str::to_ascii_lowercase),
        budget: text(7),
        preset: text(8),
        label: columns[6].clone(),
        state: MailState::Proposed,
        doubtful,
        refs: if columns[9].is_empty() { Vec::new() } else { columns[9].split(SEP).map(str::to_string).collect() },
        due: columns[10].parse().ok(),
        instalments: columns[11].parse().ok(),
    })
}

/// A subject's shape (design §5, "What corrections teach"): folded, its
/// digits, amounts and currency marks taken out, its spaces made one.
/// "Votre facture n° 2026-042 de 52,00 €" and "… n° 2026-043 de 61,30 €" are one shape.
pub fn shape(subject: &str) -> String {
    let folded: String = crate::text::fold(subject).into_iter().collect();
    let mut out = String::with_capacity(folded.len());
    let mut in_number = false;
    for c in folded.chars() {
        if c.is_ascii_digit() || (in_number && matches!(c, ',' | '.' | '-' | '/' | '\u{a0}' | '\u{202f}')) {
            if !in_number {
                out.push('#');
            }
            in_number = true;
            continue;
        }
        in_number = false;
        if matches!(c, '€' | '$' | '£') {
            continue;
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whose shapes are kept: the sender's domain, else its name, folded.
fn sender_of(domain: Option<&str>, sender: &str) -> String {
    domain.map(str::to_ascii_lowercase).filter(|d| !d.is_empty()).unwrap_or_else(|| crate::words::folded(sender))
}

/// The entry "Not a payment" keeps for mail of this shape from this sender
/// (`MoneyState::ignored`), which older versions never take for a message's key.
pub fn shape_key(domain: Option<&str>, sender: &str, subject: &str) -> String {
    format!("shape:{}{SEP}{}", sender_of(domain, sender), shape(subject))
}

/// Whether "Not a payment" said of mail of this shape keeps a line out: its
/// own key said so, or its shape (`shape_key`).
pub fn ignored(ignored: &BTreeSet<String>, line: &MailLine) -> Option<Ignored> {
    if ignored.contains(&line.key) {
        return Some(Ignored::Itself);
    }
    ignored.contains(&shape_key(line.sender_domain.as_deref(), &line.sender, &line.subject)).then_some(Ignored::Shape)
}

/// Why a message is not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ignored {
    /// You said this one is no payment.
    Itself,
    /// You said so of mail of this shape from its sender.
    Shape,
}

// ---------------------------------------------------------------- the history

/// The history's filters (design §5): none given, every money mail.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MailQuery {
    pub from: Option<Date>,
    pub to: Option<Date>,
    /// The address it came to, by its account's id.
    pub address: Option<String>,
    /// The account of the movement it tells of.
    pub account: Option<String>,
    pub budget: Option<String>,
    /// Tied to a movement or not.
    pub tied: Option<bool>,
    pub way: Option<Way>,
    pub at_least: Option<Money>,
    pub at_most: Option<Money>,
    /// What it is for; a typed search reaches every area.
    pub areas: Option<crate::areas::Area>,
    pub kind: Option<PaymentKind>,
    pub text: String,
    pub page: usize,
}

/// What became of a money mail, in words and as data.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Became {
    /// The same payment as a movement (its address, who, its day).
    Joined { address: String, who: String, day: Date },
    /// Part of a payout (its address, who, its day).
    Carried { address: String, who: String, day: Date },
    /// The fees of a movement (a fee invoice).
    Fees { address: String, who: String, day: Date },
    /// A line of the budget file, in that budget.
    Recorded { address: String, budget: String },
    /// Not counted: you said so of it, or of mail of its shape.
    Ignored { shape: bool },
    /// Not tied to any movement.
    Untied,
}

/// One money mail of the history.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MailRow {
    /// Its address (`mid:…`, `sioul:mail/…`), and its file now.
    pub key: String,
    pub path: PathBuf,
    /// The address it came to, by its account's id.
    pub address: String,
    pub date: Date,
    /// `PaymentKind::message_id` without its prefix: received, paid, order, bill, refund.
    pub kind: String,
    pub who: String,
    pub sender: String,
    /// Its sender's domain, by which "Not a payment" learns its shape.
    pub domain: Option<String>,
    pub subject: String,
    /// Signed: + money in, − money out; none when it says no amount.
    pub amount: Option<Money>,
    pub currency: String,
    pub fee: Option<Money>,
    pub became: Became,
    /// What it is for: its movement's, else its address's.
    #[serde(skip)]
    pub area: crate::areas::Area,
    #[serde(skip)]
    pub movement_account: Option<String>,
    #[serde(skip)]
    pub budgets: Vec<String>,
    /// From a sender not verified: never joined without your word.
    pub doubtful: bool,
}

/// A page of the history.
#[derive(Debug, Clone, PartialEq)]
pub struct MailPage {
    pub rows: Vec<MailRow>,
    pub page: usize,
    pub more: bool,
    /// Messages not looked at yet (a first index under way): said in one calm line.
    pub waiting: usize,
}

/// The addresses whose mail the history reads: their ids and Maildirs, portals aside.
pub fn addresses(config: &crate::config::Config) -> Vec<(String, PathBuf)> {
    config.accounts.iter().filter(|a| a.kind != crate::config::AccountKind::Portal).map(|a| (a.id.clone(), a.maildir_path())).collect()
}

/// Whether mail lies in a folder the history never reads: the junk, the trash.
fn set_aside(words: &crate::words::Words, account: &str, local: &str) -> bool {
    // The Maildir itself is the inbox, whatever the names of other folders fold to.
    if local.is_empty() {
        return false;
    }
    let listed = crate::folders::saved(account);
    let special = listed.iter().find(|f| f.local == local && f.special).map(|f| f.role);
    let role = crate::folders::role_in(&words.folders, local.trim_start_matches('.'), special);
    matches!(role, crate::folders::Role::Junk | crate::folders::Role::Trash)
}

/// Whether a cached message is forged, as the mailbox judges it (`view::mail_item`).
fn forged(card: &crate::card::Card, trusted: &[String], own_domains: &[String]) -> bool {
    let results = crate::trust::read_auth_results(&card.headers, trusted);
    let excused = card.is_list.then(|| crate::trust::list_excused(results.as_ref(), &card.headers, &crate::trust::sealers(trusted, own_domains))).flatten();
    crate::trust::judge_sender(results.as_ref(), excused.as_ref(), card.sender_domain()).0 == crate::trust::Trust::Forged
}

/// Every money mail kept on this device (see the module's notes), each with
/// what became of it, the filters of `q` applied, newest first, page
/// `q.page` of 60. At most `fill` messages not looked at yet are opened now;
/// the rest wait (`MailPage::waiting`).
pub fn history(config: &crate::config::Config, books: &Books, store: &Store, ignored_keys: &BTreeSet<String>, q: &MailQuery, fill: usize) -> MailPage {
    let words = crate::words::Words::of(config);
    let reader = Reader::new(&books.ledger.mail_rules, &words);
    let (trusted, own) = (config.all_trusted_ids(), config.own_domains());
    let mut rows: Vec<MailRow> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut waiting = 0;
    for (account, local, pass) in crate::mailpass::every(&reader, &addresses(config), fill) {
        waiting += pass.waiting;
        if set_aside(&words, &account, &local) {
            continue;
        }
        for found in pass.found {
            let Some(columns) = &found.columns else { continue };
            if found.line.spam {
                continue;
            }
            let mut card = found.line.card(&found.path);
            card.account = Some(account.clone());
            if forged(&card, &trusted, &own) {
                continue;
            }
            let Some(line) = mail_line(columns, &card, false) else { continue };
            if !seen.insert(line.key.clone()) {
                continue;
            }
            rows.push(row(config, books, store, ignored_keys, &account, &found.path, line));
        }
    }
    rows.retain(|r| keeps(&books.ledger, q, r));
    rows.sort_by(|a, b| (b.date, &b.key).cmp(&(a.date, &a.key)));
    let start = q.page * PAGE;
    let more = rows.len() > start + PAGE;
    let rows = rows.into_iter().skip(start).take(PAGE).collect();
    MailPage { rows, page: q.page, more, waiting }
}

/// One money mail's row: what became of it, from the shared store and the budget file.
fn row(config: &crate::config::Config, books: &Books, store: &Store, ignored_keys: &BTreeSet<String>, account: &str, path: &Path, line: MailLine) -> MailRow {
    let find = |key: &str| books.movements.iter().find(|m| m.has(key));
    let fee_key = format!("{}{}", crate::movements::FEE_INVOICE, line.key);
    let recorded = books.ledger.lines.iter().position(|l| l.links.contains(&line.key));
    let became = if let Some(m) = store.carried.iter().find(|c| c.payments.iter().any(|p| p.source == line.key)).and_then(|c| find(&c.payout)) {
        Became::Carried { address: m.address(), who: m.label.clone(), day: m.day }
    } else if let Some(m) = store.links.iter().find(|l| l.source == fee_key).and_then(|l| find(&l.movement)) {
        Became::Fees { address: m.address(), who: m.label.clone(), day: m.day }
    } else if let Some(m) = store.links.iter().find(|l| l.source == line.key).and_then(|l| find(&l.movement)).or_else(|| find(&line.key).filter(|_| recorded.is_none())) {
        Became::Joined { address: m.address(), who: m.label.clone(), day: m.day }
    } else if let Some(at) = recorded {
        let l = &books.ledger.lines[at];
        let address = find(&line.key).map(|m| m.address()).unwrap_or_default();
        Became::Recorded { address, budget: l.budget.clone() }
    } else if let Some(why) = ignored(ignored_keys, &line) {
        Became::Ignored { shape: why == Ignored::Shape }
    } else {
        Became::Untied
    };
    let movement = match &became {
        Became::Joined { address, .. } | Became::Carried { address, .. } | Became::Fees { address, .. } | Became::Recorded { address, .. } => books.movements.iter().find(|m| m.address() == *address),
        _ => None,
    };
    let account_area = config.account(account).and_then(|a| a.area.as_deref()).and_then(crate::areas::Area::parse).unwrap_or(crate::areas::Area::ADMIN);
    MailRow {
        key: line.key.clone(),
        path: path.to_path_buf(),
        address: account.to_string(),
        date: line.date,
        kind: line.payment.kind.message_id().trim_start_matches("kind-").to_string(),
        who: line.label.clone(),
        sender: line.sender.clone(),
        domain: line.sender_domain.clone(),
        subject: line.subject.clone(),
        amount: line.signed(),
        currency: line.payment.amount.as_ref().map_or("EUR", |a| a.currency).to_string(),
        fee: line.payment.fee.as_ref().map(|f| f.money),
        area: movement.map_or(account_area, |m| m.area),
        movement_account: movement.and_then(|m| m.account.clone()),
        budgets: movement.map(|m| m.place.budgets().iter().map(|b| b.to_string()).collect()).unwrap_or_else(|| line.budget.clone().into_iter().collect()),
        became,
        doubtful: line.doubtful,
    }
}

/// Whether a row answers the filters.
fn keeps(ledger: &Ledger, q: &MailQuery, r: &MailRow) -> bool {
    if q.from.is_some_and(|d| r.date < d) || q.to.is_some_and(|d| r.date > d) {
        return false;
    }
    if q.address.as_deref().is_some_and(|a| r.address != a) || q.account.as_deref().is_some_and(|a| r.movement_account.as_deref() != Some(a)) {
        return false;
    }
    if q.budget.as_deref().is_some_and(|b| !r.budgets.iter().any(|x| x == b)) {
        return false;
    }
    let tied = !matches!(r.became, Became::Untied | Became::Ignored { .. });
    if q.tied.is_some_and(|t| t != tied) {
        return false;
    }
    let way = match r.amount {
        Some(a) if a.is_negative() => Way::Out,
        _ => Way::In,
    };
    if q.way.is_some_and(|w| w != way) {
        return false;
    }
    let amount = r.amount.map_or(Money::ZERO, Money::abs);
    if q.at_least.is_some_and(|a| amount < a.abs()) || q.at_most.is_some_and(|a| amount > a.abs()) {
        return false;
    }
    if q.kind.is_some_and(|k| k.message_id().trim_start_matches("kind-") != r.kind) {
        return false;
    }
    let words: Vec<String> = q.text.split_whitespace().map(crate::words::folded).filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return q.areas.is_none_or(|areas| r.area.is_empty() || r.area.meets(areas));
    }
    let titles: Vec<&str> = r.budgets.iter().map(|b| ledger.budget_title(b)).collect();
    let cents = amount.cents();
    let haystack = crate::words::folded(&format!("{} {} {} {} {}.{:02} {},{:02}", r.who, r.sender, r.subject, titles.join(" "), cents / 100, cents % 100, cents / 100, cents % 100));
    words.iter().all(|w| haystack.contains(w.as_str()))
}

/// What became of it, in words: "the same payment as Rent, Sun 1 Nov".
pub fn became_words(tr: &Translator, ledger: &Ledger, r: &MailRow, sender: &str) -> String {
    let mut args = crate::i18n::args();
    let id = match &r.became {
        Became::Joined { who, day, .. } | Became::Carried { who, day, .. } | Became::Fees { who, day, .. } => {
            args.set("who", who.clone());
            args.set("day", crate::moneyview::short_day(tr, *day));
            match r.became {
                Became::Joined { .. } => "money-mail-joined",
                Became::Carried { .. } => "money-mail-carried",
                _ => "money-mail-fees",
            }
        }
        Became::Recorded { budget, .. } => {
            args.set("budget", ledger.budget_title(budget).to_string());
            "money-mail-recorded"
        }
        Became::Ignored { shape: false } => "money-mail-ignored",
        Became::Ignored { shape: true } => {
            args.set("sender", sender.to_string());
            "money-mail-ignored-shape"
        }
        Became::Untied => "money-mail-untied",
    };
    tr.text(id, Some(&args))
}

/// A row in words: its day, its kind, who, the amount (and the fee when the
/// mail states one), the subject, and what became of it.
pub fn row_words(tr: &Translator, ledger: &Ledger, r: &MailRow) -> Vec<String> {
    let mut head = vec![crate::moneyview::short_day(tr, r.date), tr.text(&format!("kind-{}", r.kind), None), r.who.clone()];
    if let Some(amount) = r.amount {
        head.push(crate::moneylist::signed(tr, amount));
    }
    if let Some(fee) = r.fee {
        let mut args = crate::i18n::args();
        args.set("fees", tr.money(fee));
        head.push(tr.text("money-mail-fee", Some(&args)));
    }
    vec![head.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · "), format!("“{}”", r.subject), became_words(tr, ledger, r, &r.sender)]
}

/// "A payment…" of a mail its shape kept out, or "Count it again": the
/// shape counts again (`MoneyState::ignored` loses it). Whether it was there.
pub fn count_again(ignored_keys: &mut BTreeSet<String>, domain: Option<&str>, sender: &str, subject: &str) -> bool {
    ignored_keys.remove(&shape_key(domain, sender, subject))
}

/// "Not a payment": this message, and mail of its shape from its sender.
pub fn not_a_payment(ignored_keys: &mut BTreeSet<String>, key: &str, domain: Option<&str>, sender: &str, subject: &str) {
    ignored_keys.insert(key.to_string());
    ignored_keys.insert(shape_key(domain, sender, subject));
}

#[cfg(test)]
#[path = "moneyindex_tests.rs"]
mod tests;

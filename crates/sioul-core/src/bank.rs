// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The money watch (docs/accounting.md, "The bank"): your bank's own export
//! (OFX, CAMT.053, or its CSV) read into `sioul-bank.toml`, then held against
//! the recurring payments of your budgets. Nobody looks when it goes badly (the
//! ostrich effect, Olafsson & Pagel 2017), and a debit that stops does not say
//! so: Sioul notices, and says it calmly and early.
//! - **Missed**: a payment expected (rent, a standing order, wages) that did not
//!   pass in the days around its date.
//! - **Changed**: one that passed with another amount.
//! - **Short**: the balance, carried forward with what is expected, would go
//!   below zero on a day; how much is missing then, and the reserve that covers it.
//!
//! No red balance, no "late" badge: a sentence, and what to do about it.

use crate::budget::Ledger;
use crate::money::Money;
use jiff::ToSpan;
use jiff::civil::Date;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "sioul-bank.toml";

/// One movement of an account: money in positive, out negative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movement {
    pub account: String,
    pub date: Date,
    pub amount: Money,
    pub label: String,
    /// The bank's own id (OFX FITID, CAMT reference), else one made from the rest.
    pub id: String,
}

/// What an export says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statement {
    pub account: String,
    pub movements: Vec<Movement>,
    /// The balance and its day, when the export gives one.
    pub balance: Option<(Date, Money)>,
}

/// An amount as banks write it: "-62,00", "1 234,56", "1,234.56", "+780.00 EUR", "(62.00)".
pub fn parse_amount(text: &str) -> Option<Money> {
    let mut t: String = text.chars().filter(|c| !c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{202f}' | '€' | '$' | '£' | '\'')).collect();
    for unit in ["EUR", "eur", "EUROS", "euros"] {
        t = t.replace(unit, "");
    }
    let negative = t.starts_with('-') || (t.starts_with('(') && t.ends_with(')')) || t.ends_with('-');
    let t = t.trim_matches(|c| matches!(c, '-' | '+' | '(' | ')'));
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.') {
        return None;
    }
    // The last separator followed by one or two digits is the decimal one.
    let last = t.rfind([',', '.']);
    let (whole, cents) = match last {
        Some(at) if t.len() - at - 1 <= 2 => (&t[..at], &t[at + 1..]),
        _ => (t, ""),
    };
    let whole: String = whole.chars().filter(char::is_ascii_digit).collect();
    let whole: i64 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
    let cents: i64 = match cents.len() {
        0 => 0,
        1 => cents.parse::<i64>().ok()? * 10,
        _ => cents.parse().ok()?,
    };
    // A run of digits too long for any amount is no amount.
    let value = whole.checked_mul(100)?.checked_add(cents)?;
    Some(Money(if negative { -value } else { value }))
}

/// A date as banks write it: "20261002", "2026-10-02", "02/10/2026", "02.10.2026", "02/10/26".
pub fn parse_date(text: &str) -> Option<Date> {
    parse_date_ordered(text, false)
}

/// The same, the month first when `month_first` ("10/13/2026", as PayPal's American download writes it).
fn parse_date_ordered(text: &str, month_first: bool) -> Option<Date> {
    let t = text.trim();
    // Bytes, not characters: "Total général" in a date column is no date, and no panic.
    if t.len() >= 8 && t.as_bytes()[..8].iter().all(u8::is_ascii_digit) {
        return Date::new(t[..4].parse().ok()?, t[4..6].parse().ok()?, t[6..8].parse().ok()?).ok();
    }
    if let Ok(date) = t.get(..10).unwrap_or(t).parse::<Date>() {
        return Some(date);
    }
    let parts: Vec<&str> = t.split(['/', '.', '-']).collect();
    if parts.len() == 3 {
        let first: i8 = parts[0].trim().parse().ok()?;
        let second: i8 = parts[1].trim().parse().ok()?;
        let (day, month) = if month_first { (second, first) } else { (first, second) };
        let year: i16 = parts[2].trim().get(..4.min(parts[2].trim().len()))?.parse().ok()?;
        let year = if year < 100 { 2000 + year } else { year };
        return Date::new(year, month, day).ok();
    }
    None
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// Reads an export, whatever its form.
pub fn read(text: &str) -> Result<Statement, String> {
    let head: String = text.chars().take(2000).collect::<String>().to_ascii_uppercase();
    let mut statement = if head.contains("<OFX>") || head.contains("OFXHEADER") {
        read_ofx(text)
    } else if head.contains("CAMT.053") || head.contains("<BKTOCSTMR") {
        read_camt(text)?
    } else {
        read_csv(text)?
    };
    // Ids made where the bank gives none: the same movement read twice keeps its id.
    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for m in &mut statement.movements {
        m.account = statement.account.clone();
        if m.id.is_empty() {
            let base = format!("{}|{}|{}", m.date, m.amount.cents(), m.label);
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            m.id = format!("{:016x}", fnv(&format!("{base}|{n}")));
        }
    }
    if statement.movements.is_empty() && statement.balance.is_none() {
        return Err("no movement".into());
    }
    Ok(statement)
}

/// OFX, SGML (1.x) or XML (2.x): tags read in order, closing tags or not.
fn read_ofx(text: &str) -> Statement {
    let mut statement = Statement::default();
    let mut current: Option<(Option<Date>, Option<Money>, String, String, String)> = None;
    let (mut balance_amount, mut balance_date) = (None, None);
    let mut in_ledger = false;
    let finish = |current: &mut Option<(Option<Date>, Option<Money>, String, String, String)>, out: &mut Vec<Movement>| {
        if let Some((Some(date), Some(amount), name, memo, id)) = current.take() {
            let label = match (name.is_empty(), memo.is_empty()) {
                (false, false) if !name.contains(&memo) => format!("{name} {memo}"),
                (false, _) => name,
                _ => memo,
            };
            out.push(Movement { account: String::new(), date, amount, label, id });
        }
    };
    for piece in text.split('<').skip(1) {
        let Some((tag, rest)) = piece.split_once('>') else { continue };
        let tag = tag.trim().to_ascii_uppercase();
        // XML's own entities: "M&amp;S" is M&S.
        let value = rest.lines().next().unwrap_or("").trim().replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");
        match tag.as_str() {
            "STMTTRN" => {
                finish(&mut current, &mut statement.movements);
                current = Some((None, None, String::new(), String::new(), String::new()));
            }
            "/STMTTRN" => finish(&mut current, &mut statement.movements),
            "LEDGERBAL" => in_ledger = true,
            "/LEDGERBAL" | "AVAILBAL" => in_ledger = false,
            "BALAMT" if in_ledger => balance_amount = parse_amount(&value),
            "DTASOF" if in_ledger => balance_date = parse_date(&value),
            "ACCTID" if statement.account.is_empty() => statement.account = value,
            _ => {
                if let Some((date, amount, name, memo, id)) = current.as_mut() {
                    match tag.as_str() {
                        "DTPOSTED" => *date = parse_date(&value),
                        "TRNAMT" => *amount = parse_amount(&value),
                        "NAME" => *name = value,
                        "MEMO" => *memo = value,
                        "FITID" => *id = value,
                        _ => {}
                    }
                }
            }
        }
    }
    finish(&mut current, &mut statement.movements);
    statement.balance = balance_date.zip(balance_amount);
    statement
}

type XmlNode<'a, 'input> = roxmltree::Node<'a, 'input>;

/// The element down `names`, by local names (namespaces aside).
fn path<'a, 'input>(node: XmlNode<'a, 'input>, names: &[&str]) -> Option<XmlNode<'a, 'input>> {
    names.iter().try_fold(node, |n, name| n.children().find(|c| c.tag_name().name() == *name))
}

/// The text of the element down `names`; none when it holds only spaces (the
/// line break before a child, as in `<Sts>` around its `<Cd>` from camt.053.001.08 on).
fn text_at(node: XmlNode, names: &[&str]) -> Option<String> {
    path(node, names).and_then(|n| n.text()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// An amount with its side: a debit (DBIT) negative.
fn signed(node: XmlNode) -> Option<Money> {
    let amount = parse_amount(&text_at(node, &["Amt"])?)?;
    Some(if text_at(node, &["CdtDbtInd"]).as_deref() == Some("DBIT") { Money(-amount.cents().abs()) } else { amount })
}

/// ISO 20022 camt.053: booked entries, and the closing booked balance. A file
/// may hold several statements (one a day): every one of the first one's
/// account is read, and the newest closing balance kept.
fn read_camt(text: &str) -> Result<Statement, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    let mut statement = Statement::default();
    let account_of = |stmt: XmlNode| text_at(stmt, &["Acct", "Id", "IBAN"]).or_else(|| text_at(stmt, &["Acct", "Id", "Othr", "Id"])).unwrap_or_default();
    let statements: Vec<XmlNode> = doc.descendants().filter(|n| n.tag_name().name() == "Stmt").collect();
    let Some(first) = statements.first() else { return Err("no statement".into()) };
    statement.account = account_of(*first);
    for stmt in statements.iter().copied().filter(|s| account_of(*s) == statement.account) {
        let mut closing: Option<(Date, Money)> = None;
        for balance in stmt.children().filter(|c| c.tag_name().name() == "Bal") {
            let kind = text_at(balance, &["Tp", "CdOrPrtry", "Cd"]).unwrap_or_default();
            if kind == "CLBD" || (kind == "CLAV" && closing.is_none()) {
                let date = text_at(balance, &["Dt", "Dt"]).or_else(|| text_at(balance, &["Dt", "DtTm"])).and_then(|d| parse_date(&d));
                closing = date.zip(signed(balance));
            }
        }
        if let Some((date, _)) = closing
            && statement.balance.is_none_or(|(newest, _)| date >= newest)
        {
            statement.balance = closing;
        }
        for entry in stmt.children().filter(|c| c.tag_name().name() == "Ntry") {
            if text_at(entry, &["Sts"]).or_else(|| text_at(entry, &["Sts", "Cd"])).is_some_and(|s| s != "BOOK") {
                continue;
            }
            let Some(date) = text_at(entry, &["BookgDt", "Dt"]).or_else(|| text_at(entry, &["ValDt", "Dt"])).and_then(|d| parse_date(&d)) else { continue };
            let Some(amount) = signed(entry) else { continue };
            let details = path(entry, &["NtryDtls", "TxDtls"]);
            // A party's name, under `Pty` from camt.053.001.08 on.
            let name = |d: XmlNode, side: &str| text_at(d, &["RltdPties", side, "Nm"]).or_else(|| text_at(d, &["RltdPties", side, "Pty", "Nm"]));
            let label = details
                .and_then(|d| text_at(d, &["RmtInf", "Ustrd"]).or_else(|| name(d, "Cdtr")).or_else(|| name(d, "Dbtr")))
                .or_else(|| text_at(entry, &["AddtlNtryInf"]))
                .unwrap_or_default();
            let party = details.and_then(|d| name(d, if amount.is_negative() { "Cdtr" } else { "Dbtr" })).unwrap_or_default();
            let label = if !party.is_empty() && !label.contains(&party) { format!("{party} {label}") } else { label };
            let id = text_at(entry, &["AcctSvcrRef"]).or_else(|| text_at(entry, &["NtryRef"])).unwrap_or_default();
            statement.movements.push(Movement { account: String::new(), date, amount, label, id });
        }
    }
    Ok(statement)
}

/// A CSV's columns, found by their names.
#[derive(Debug, Clone, Copy, Default)]
struct Columns {
    date: usize,
    label: usize,
    amount: Option<usize>,
    debit: Option<usize>,
    credit: Option<usize>,
    /// PayPal's "Type", Stripe's: said after the label.
    kind: Option<usize>,
    /// PayPal's "État": what was refused, cancelled or is still pending is left out.
    status: Option<usize>,
    /// PayPal's "Impact sur le solde": a "Mémo" row moves nothing (a hold, a note).
    impact: Option<usize>,
    /// The balance after each row (PayPal).
    balance: Option<usize>,
    time: Option<usize>,
    currency: Option<usize>,
    id: Option<usize>,
}

/// A bank's CSV, PayPal's and Stripe's too: the header row found by its words
/// (date, label or name, amount or net, or debit and credit), `;` or `,`,
/// decimal commas; a balance written above it read too (as some French banks
/// write it: "Solde (EUROS) ;1 234,56" under "Date ;03/10/2026"), or one on each row
/// (PayPal). Net amounts, fees deducted, when given; rows in another currency
/// than most of the file's, and those refused, cancelled or pending, left out.
fn read_csv(text: &str) -> Result<Statement, String> {
    let text = text.trim_start_matches('\u{feff}');
    let separator = if text.lines().take(15).map(|l| l.matches(';').count()).sum::<usize>() >= text.lines().take(15).map(|l| l.matches(',').count()).sum::<usize>() / 2 { ';' } else { ',' };
    let cells = |line: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut cell = String::new();
        let mut quoted = false;
        for c in line.chars() {
            match c {
                '"' => quoted = !quoted,
                c if c == separator && !quoted => out.push(std::mem::take(&mut cell).trim().to_string()),
                c => cell.push(c),
            }
        }
        out.push(cell.trim().to_string());
        out
    };
    let fold = |s: &str| crate::text::fold(s).into_iter().collect::<String>();
    let mut statement = Statement::default();
    let (mut pre_date, mut pre_balance) = (None, None);
    let mut header: Option<Columns> = None;
    let mut rows: Vec<(Vec<String>, Columns)> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let row = cells(line);
        let Some(columns) = header else {
            let names: Vec<String> = row.iter().map(|c| fold(c)).collect();
            let find = |words: &[&str]| names.iter().position(|n| words.iter().any(|w| n.starts_with(w)));
            let exact = |words: &[&str]| names.iter().position(|n| words.iter().any(|w| n == w));
            let date = find(&["date", "created", "cree"]);
            let label = find(&["libelle", "label", "description", "intitule", "detail", "operation", "nature"]).or_else(|| exact(&["name", "nom"]));
            // Net of fees first (PayPal, Stripe), else the amount.
            let amount = exact(&["net"]).or_else(|| find(&["montant", "amount", "somme"]));
            let (debit, credit) = (find(&["debit"]), find(&["credit"]));
            if let (Some(date), Some(label)) = (date, label)
                && (amount.is_some() || debit.is_some() || credit.is_some())
                && row.len() >= 3
            {
                let kind = exact(&["type"]).filter(|k| *k != label);
                // Stripe's Description beside its Type; PayPal's Name.
                let label = if names.get(label).is_some_and(|n| n.starts_with("description")) { label } else { exact(&["name", "nom"]).unwrap_or(label) };
                header = Some(Columns {
                    date,
                    label,
                    amount,
                    debit,
                    credit,
                    kind,
                    status: exact(&["status", "etat", "statut"]),
                    impact: exact(&["impact sur le solde", "balance impact"]),
                    balance: exact(&["balance", "solde"]),
                    time: exact(&["time", "heure"]),
                    currency: exact(&["currency", "devise"]),
                    id: exact(&["id", "transaction id", "numero de transaction", "id de transaction"]),
                });
                continue;
            }
            // Above the header: an account, a balance, its day.
            let first = names.first().cloned().unwrap_or_default();
            let second = row.get(1).cloned().unwrap_or_default();
            if first.starts_with("solde") {
                pre_balance = parse_amount(&second);
            } else if first.starts_with("date") {
                pre_date = parse_date(&second);
            } else if (first.starts_with("numero") || first.starts_with("compte") || first.starts_with("account")) && statement.account.is_empty() {
                statement.account = second;
            }
            continue;
        };
        rows.push((row, columns));
    }
    let Some(columns) = header else { return Err("no header".into()) };
    // Day or month first, from the dates themselves: PayPal's American download
    // writes 10/13/2026. A first part above 12 is a day; a second, a month's day.
    let month_first = {
        let (mut day_first, mut month_first) = (false, false);
        for (row, c) in &rows {
            let parts: Vec<u8> = row.get(c.date).map(|d| d.split(['/', '.']).take(2).filter_map(|p| p.trim().parse().ok()).collect()).unwrap_or_default();
            if let [first, second] = parts[..] {
                day_first |= first > 12;
                month_first |= second > 12;
            }
        }
        month_first && !day_first
    };
    // The file's currency: the one most rows are in.
    let main_currency = columns.currency.and_then(|c| {
        let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (row, _) in &rows {
            *counts.entry(row.get(c).cloned().unwrap_or_default().to_uppercase()).or_insert(0) += 1;
        }
        counts.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c)
    });
    let mut latest: Option<((Date, String), Money)> = None;
    for (row, c) in rows {
        let Some(date) = row.get(c.date).and_then(|d| parse_date_ordered(d, month_first)) else { continue };
        if let (Some(col), Some(main)) = (c.currency, &main_currency)
            && row.get(col).map(|v| v.to_uppercase()).as_ref() != Some(main)
        {
            continue;
        }
        if let Some(col) = c.status {
            let status = fold(row.get(col).map_or("", String::as_str));
            if ["pending", "en attente", "refuse", "denied", "annule", "cancel", "reversed", "failed", "echoue"].iter().any(|w| status.starts_with(w)) {
                continue;
            }
        }
        if c.impact.and_then(|i| row.get(i)).is_some_and(|v| fold(v).starts_with("memo")) {
            continue;
        }
        let amount = match (c.amount, c.debit, c.credit) {
            (Some(a), _, _) => row.get(a).and_then(|v| parse_amount(v)),
            (None, debit, credit) => {
                let debit = debit.and_then(|d| row.get(d)).and_then(|v| parse_amount(v)).map(|m| Money(-m.cents().abs()));
                let credit = credit.and_then(|c| row.get(c)).and_then(|v| parse_amount(v)).map(|m| Money(m.cents().abs()));
                debit.or(credit)
            }
        };
        let Some(amount) = amount else { continue };
        let name = row.get(c.label).cloned().unwrap_or_default();
        let kind = c.kind.and_then(|k| row.get(k)).cloned().unwrap_or_default();
        let label = match (name.is_empty(), kind.is_empty()) {
            (false, false) => format!("{name} · {kind}"),
            (true, _) => kind,
            (false, true) => name,
        };
        if let Some(b) = c.balance.and_then(|b| row.get(b)).and_then(|v| parse_amount(v)) {
            let when = (date, c.time.and_then(|t| row.get(t)).cloned().unwrap_or_default());
            if latest.as_ref().is_none_or(|(w, _)| when >= *w) {
                latest = Some((when, b));
            }
        }
        let id = c.id.and_then(|i| row.get(i)).cloned().unwrap_or_default();
        statement.movements.push(Movement { account: String::new(), date, amount, label, id });
    }
    // The day of a balance written above: the day said, else the newest movement's.
    if let Some(amount) = pre_balance {
        let day = pre_date.or_else(|| statement.movements.iter().map(|m| m.date).max());
        statement.balance = day.map(|d| (d, amount));
    } else if let Some(((date, _), amount)) = latest {
        statement.balance = Some((date, amount));
    }
    Ok(statement)
}

/// An account as kept: its last balance known.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub title: String,
    pub balance: Option<(Date, Money)>,
}

/// What `sioul-bank.toml` holds.
#[derive(Debug, Clone, Default)]
pub struct Bank {
    pub root: PathBuf,
    pub accounts: Vec<Account>,
    pub movements: Vec<Movement>,
}

/// What an import added.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Imported {
    pub read: usize,
    pub new: usize,
}

impl Bank {
    pub fn load(root: &Path) -> Result<Bank, String> {
        let path = root.join(MANIFEST);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Bank { root: root.to_path_buf(), ..Bank::default() }),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| format!("{}: {e}", path.display()))?;
        let date_of = |v: Option<&toml::Value>| v.and_then(|v| match v {
            toml::Value::Datetime(d) => parse_date(&d.to_string()),
            toml::Value::String(s) => parse_date(s),
            _ => None,
        });
        let money_of = |v: Option<&toml::Value>| v.and_then(|v| match v {
            toml::Value::Float(f) => Some(Money::from_units(*f)),
            toml::Value::Integer(i) => Some(Money(i * 100)),
            toml::Value::String(s) => parse_amount(s),
            _ => None,
        });
        let text_of = |v: Option<&toml::Value>| v.and_then(toml::Value::as_str).unwrap_or("").to_string();
        let list = |name: &str| table.get(name).and_then(toml::Value::as_array).cloned().unwrap_or_default();
        let accounts = list("account")
            .iter()
            .filter_map(|a| {
                let a = a.as_table()?;
                Some(Account { id: text_of(a.get("id")), title: text_of(a.get("title")), balance: date_of(a.get("as_of")).zip(money_of(a.get("balance"))) })
            })
            .collect();
        let movements = list("movement")
            .iter()
            .filter_map(|m| {
                let m = m.as_table()?;
                Some(Movement { account: text_of(m.get("account")), date: date_of(m.get("date"))?, amount: money_of(m.get("amount"))?, label: text_of(m.get("label")), id: text_of(m.get("id")) })
            })
            .collect();
        Ok(Bank { root: root.to_path_buf(), accounts, movements })
    }

    /// A statement taken in: new movements added (by id), the balance kept when newer.
    pub fn import(&mut self, statement: &Statement, title: &str) -> Imported {
        let account = if statement.account.is_empty() { "account".to_string() } else { statement.account.clone() };
        let mut new = 0;
        for m in &statement.movements {
            if !self.movements.iter().any(|old| old.account == account && old.id == m.id) {
                self.movements.push(Movement { account: account.clone(), ..m.clone() });
                new += 1;
            }
        }
        self.movements.sort_by(|a, b| (a.date, &a.account, &a.id).cmp(&(b.date, &b.account, &b.id)));
        let place = match self.accounts.iter().position(|a| a.id == account) {
            Some(at) => at,
            None => {
                self.accounts.push(Account { id: account.clone(), title: title.to_string(), balance: None });
                self.accounts.len() - 1
            }
        };
        if let Some((date, amount)) = statement.balance
            && self.accounts[place].balance.is_none_or(|(old, _)| date >= old)
        {
            self.accounts[place].balance = Some((date, amount));
        }
        Imported { read: statement.movements.len(), new }
    }

    pub fn save(&self) -> Result<(), String> {
        let mut doc = toml_edit::DocumentMut::new();
        doc.decor_mut().set_prefix("# Your bank's movements, read from its exports (docs/accounting.md). Kept here only.\n\n");
        let date = |d: Date| toml_edit::value(toml_edit::Datetime { date: Some(toml_edit::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None });
        let money = |m: Money| toml_edit::value(m.cents() as f64 / 100.0);
        let mut accounts = toml_edit::ArrayOfTables::new();
        for a in &self.accounts {
            let mut t = toml_edit::Table::new();
            t["id"] = toml_edit::value(&a.id);
            if !a.title.is_empty() {
                t["title"] = toml_edit::value(&a.title);
            }
            if let Some((d, m)) = a.balance {
                t["balance"] = money(m);
                t["as_of"] = date(d);
            }
            accounts.push(t);
        }
        doc.insert("account", toml_edit::Item::ArrayOfTables(accounts));
        let mut movements = toml_edit::ArrayOfTables::new();
        for m in &self.movements {
            let mut t = toml_edit::Table::new();
            t["account"] = toml_edit::value(&m.account);
            t["date"] = date(m.date);
            t["amount"] = money(m.amount);
            t["label"] = toml_edit::value(&m.label);
            t["id"] = toml_edit::value(&m.id);
            movements.push(t);
        }
        doc.insert("movement", toml_edit::Item::ArrayOfTables(movements));
        let path = self.root.join(MANIFEST);
        let temporary = self.root.join(format!("{MANIFEST}.new"));
        std::fs::write(&temporary, doc.to_string()).and_then(|()| std::fs::rename(&temporary, &path)).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The balance of every account together, and the newest day among them.
    pub fn balance(&self) -> Option<(Date, Money)> {
        let known: Vec<(Date, Money)> = self.accounts.iter().filter_map(|a| a.balance).collect();
        let day = known.iter().map(|(d, _)| *d).max()?;
        Some((day, known.iter().map(|(_, m)| *m).sum()))
    }
}

/// What the watch found, each said in a sentence by the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// Expected around `date`, not seen.
    Missed { label: String, amount: Money, date: Date },
    /// Seen on `date` with another amount.
    Changed { label: String, expected: Money, actual: Money, date: Date },
    /// The balance would go below zero on `date`, with `label` leaving then;
    /// `short` is missing; the reserve that covers it, if one does.
    Short { label: String, amount: Money, date: Date, short: Money, reserve: Option<String> },
}

/// One payment expected ahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    pub date: Date,
    pub label: String,
    pub amount: Money,
}

/// The watch: its findings, the balance known, the days ahead.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Watch {
    pub balance: Option<(Date, Money)>,
    pub findings: Vec<Finding>,
    /// The next seven days' payments.
    pub coming: Vec<Expected>,
    /// The balance, day by day, until a month from today.
    pub forecast: Vec<(Date, Money)>,
}

impl Watch {
    /// The balance expected on `date`, when the forecast reaches it.
    pub fn balance_on(&self, date: Date) -> Option<Money> {
        self.forecast.iter().rev().find(|(d, _)| *d <= date).map(|(_, m)| *m)
    }
}

/// Words that name nothing: "the", "pour", and what every bank line says
/// ("PRLV SEPA", "CARTE", "VIREMENT"). "Choir, the year's fee" is not "CB
/// BAKERY OF THE PORT".
const FILLER: &[&str] = &[
    "the", "and", "for", "from", "with", "your", "our", "you", "its", "this", "that", "are", "was", "not", "but", "all", "any", "per", "via", "off", "out",
    "les", "des", "une", "pour", "par", "sur", "aux", "avec", "dans", "est", "pas", "que", "qui", "vos", "votre", "nos", "notre", "son", "ses", "leur", "chez", "sans", "entre",
    "prlv", "sepa", "vir", "virement", "prelevement", "paiement", "carte", "achat", "retrait", "payment", "transfer", "card", "purchase", "debit", "credit", "ref", "reference",
];

/// The words of a label that can name a payment: folded, three letters at least, no number, no filler.
pub(crate) fn words(text: &str) -> Vec<String> {
    crate::text::fold(text)
        .into_iter()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !w.chars().all(|c| c.is_ascii_digit()) && !FILLER.contains(w))
        .map(str::to_string)
        .collect()
}

/// Whether a movement stands for a payment expected on `date`: a word of the
/// payment's name in the movement's, or the same amount within days.
pub(crate) fn matches(movement: &Movement, label: &[String], amount: Money, date: Date) -> bool {
    let near = movement.date >= date.checked_sub(3.days()).unwrap_or(date) && movement.date <= date.checked_add(7.days()).unwrap_or(date);
    if !near || movement.amount.is_negative() != amount.is_negative() {
        return false;
    }
    let theirs = words(&movement.label);
    let named = label.iter().any(|w| theirs.contains(w));
    let same = (movement.amount.cents() - amount.cents()).abs() <= amount.cents().abs() / 100 + 1;
    named || (same && (movement.date.since(date).map_or(99, |s| s.get_days().abs())) <= 3)
}

/// Holds the movements against the ledger's recurring payments and planned lines.
pub fn watch(bank: &Bank, ledger: &Ledger, today: Date) -> Watch {
    let mut out = Watch { balance: bank.balance(), ..Watch::default() };
    let first = bank.movements.iter().map(|m| m.date).min();
    let last = bank.movements.iter().map(|m| m.date).max();
    let mut used: Vec<bool> = vec![false; bank.movements.len()];
    // Past occurrences inside what the exports cover: seen, changed, or missed.
    if let (Some(first), Some(last)) = (first, last) {
        let from = first.checked_add(3.days()).unwrap_or(first);
        let to = last.checked_sub(7.days()).unwrap_or(last);
        for preset in ledger.presets.iter().filter(|p| !p.estimate) {
            let label = words(&preset.label);
            for date in preset.occurrences(from, to) {
                let found = bank.movements.iter().enumerate().find(|(i, m)| !used[*i] && matches(m, &label, preset.amount, date));
                match found {
                    Some((i, m)) => {
                        used[i] = true;
                        let gap = (m.amount.cents() - preset.amount.cents()).abs();
                        if gap > preset.amount.cents().abs() / 20 && gap > 200 {
                            out.findings.push(Finding::Changed { label: preset.label.clone(), expected: preset.amount, actual: m.amount, date: m.date });
                        }
                    }
                    None => out.findings.push(Finding::Missed { label: preset.label.clone(), amount: preset.amount, date }),
                }
            }
        }
    }
    // Ahead: from the balance known, what is expected, day by day.
    let Some((as_of, start)) = out.balance else { return out };
    let horizon = today.checked_add(31.days()).unwrap_or(today);
    let movements: Vec<&Movement> = bank.movements.iter().collect();
    let flows = expected(&movements, ledger, None, as_of, today, horizon);
    let week = today.checked_add(7.days()).unwrap_or(today);
    out.coming = flows.iter().filter(|f| f.date >= today && f.date <= week && f.amount.is_negative() && !ledger.presets.iter().any(|p| p.estimate && p.label == f.label)).cloned().collect();
    let (forecast, below) = carry(start, as_of, &flows, today, horizon, Money::ZERO);
    out.forecast = forecast;
    // Bank accounts with their reserves say what tops them up (`accounts::top_ups`).
    if let Some((date, at, balance)) = below
        && ledger.bank_accounts.iter().all(|a| a.topped_up_by.is_empty())
    {
        let short = Money(-balance.cents());
        let reserve = ledger.reserves.iter().find(|r| (r.balance - r.floor).cents() >= short.cents()).map(|r| r.title.clone());
        out.findings.push(Finding::Short { label: flows[at].label.clone(), amount: flows[at].amount, date, short, reserve });
    }
    out
}

/// What is expected on an account from `as_of` to `horizon`: its movements
/// read after the balance, then the recurring payments and planned lines of
/// `budgets` (every budget's when None) not seen yet, daily spending spread
/// over its days; in date order.
pub(crate) fn expected(movements: &[&Movement], ledger: &Ledger, budgets: Option<&[String]>, as_of: Date, today: Date, horizon: Date) -> Vec<Expected> {
    let ours = |budget: &str| budgets.is_none_or(|list| list.iter().any(|b| b == budget));
    let last = movements.iter().map(|m| m.date).max();
    let seen_until = last.unwrap_or(as_of).max(as_of);
    let mut flows: Vec<Expected> = Vec::new();
    for m in movements.iter().filter(|m| m.date > as_of) {
        flows.push(Expected { date: m.date, label: m.label.clone(), amount: m.amount });
    }
    for preset in ledger.presets.iter().filter(|p| ours(&p.budget)) {
        if preset.estimate {
            // Spending without a trace, spread over its period's days.
            let per_day = |d: Date| {
                let (a, b) = preset.every.bounds(d);
                let days = b.since(a).map_or(30, |s| i64::from(s.get_days()) + 1).max(1);
                Money(preset.amount.cents() / days)
            };
            let mut d = as_of.tomorrow().unwrap_or(as_of);
            while d <= horizon {
                if preset.from.is_none_or(|f| d >= f) && preset.until.is_none_or(|u| d <= u) {
                    flows.push(Expected { date: d, label: preset.label.clone(), amount: per_day(d) });
                }
                let Ok(next) = d.tomorrow() else { break };
                d = next;
            }
            continue;
        }
        let label = words(&preset.label);
        for date in preset.occurrences(as_of.tomorrow().unwrap_or(as_of), horizon) {
            // Already passed in an export read after the balance: counted there.
            if date <= seen_until && movements.iter().any(|m| m.date > as_of && matches(m, &label, preset.amount, date)) {
                continue;
            }
            flows.push(Expected { date: date.max(today), label: preset.label.clone(), amount: preset.amount });
        }
    }
    for line in ledger.lines.iter().filter(|l| l.planned && l.date > as_of && l.date <= horizon && l.preset.is_none() && ours(&l.budget)) {
        flows.push(Expected { date: line.date.max(today), label: line.label.clone(), amount: line.amount });
    }
    flows.sort_by(|a, b| (a.date, a.amount.cents()).cmp(&(b.date, b.amount.cents())));
    flows
}

/// The balance from `start` on `as_of`, carried day by day with `flows`, as
/// seen from today to `horizon`; and the first day from today it goes below
/// `floor` with a payment out: that day, the flow that takes it there, the
/// balance then.
pub(crate) fn carry(start: Money, as_of: Date, flows: &[Expected], today: Date, horizon: Date, floor: Money) -> (Vec<(Date, Money)>, Option<(Date, usize, Money)>) {
    let mut forecast = Vec::new();
    let mut below = None;
    let mut balance = start;
    let mut day = as_of;
    let mut at = 0;
    while day <= horizon {
        while at < flows.len() && flows[at].date <= day {
            balance += flows[at].amount;
            if below.is_none() && balance.cents() < floor.cents() && flows[at].amount.is_negative() && day >= today {
                below = Some((day, at, balance));
            }
            at += 1;
        }
        if day >= today {
            forecast.push((day, balance));
        }
        // The horizon's own flows count too.
        let Ok(next) = day.tomorrow() else { break };
        day = next;
    }
    (forecast, below)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filler_words_name_nothing() {
        let choir = words("Choir, the year's fee");
        assert_eq!(choir, vec!["choir".to_string(), "year".to_string(), "fee".to_string()]);
        assert!(!words("CB BAKERY OF THE PORT").iter().any(|w| choir.contains(w)));
        assert_eq!(words("PRLV SEPA EDF 4417"), vec!["edf".to_string()]);
    }

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn amounts_and_dates_as_banks_write_them() {
        assert_eq!(parse_amount("-62,00"), Some(Money(-6200)));
        assert_eq!(parse_amount("1 234,56"), Some(Money(123456)));
        assert_eq!(parse_amount("1\u{a0}234,56 €"), Some(Money(123456)));
        assert_eq!(parse_amount("1,234.56"), Some(Money(123456)));
        assert_eq!(parse_amount("+780.00 EUR"), Some(Money(78000)));
        assert_eq!(parse_amount("(62.00)"), Some(Money(-6200)));
        assert_eq!(parse_amount("12"), Some(Money(1200)));
        assert_eq!(parse_amount("1.250"), Some(Money(125000)), "a thousands dot");
        assert_eq!(parse_amount("abc"), None);
        assert_eq!(parse_date("20261002120000[+1:CET]"), Some(day("2026-10-02")));
        assert_eq!(parse_date("02/10/2026"), Some(day("2026-10-02")));
        assert_eq!(parse_date("02.10.26"), Some(day("2026-10-02")));
        assert_eq!(parse_date("2026-10-02T00:00:00"), Some(day("2026-10-02")));
    }

    #[test]
    fn three_exports_read() {
        let ofx = "OFXHEADER:100\nDATA:OFXSGML\n<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><BANKACCTFROM><ACCTID>0123456A</BANKACCTFROM>\n<BANKTRANLIST>\n<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20261001\n<TRNAMT>-780.00\n<FITID>A1\n<NAME>PRLV SEPA AGENCE LOYERS\n</STMTTRN>\n<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20261002\n<TRNAMT>1200.00\n<FITID>A2\n<NAME>VIR SALAIRE\n<MEMO>OCTOBRE\n</STMTTRN>\n</BANKTRANLIST>\n<LEDGERBAL><BALAMT>845.10<DTASOF>20261003</LEDGERBAL>\n</STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>";
        let s = read(ofx).unwrap();
        assert_eq!(s.account, "0123456A");
        assert_eq!(s.movements.len(), 2);
        assert_eq!((s.movements[1].amount, s.movements[1].label.as_str(), s.movements[1].id.as_str()), (Money(120000), "VIR SALAIRE OCTOBRE", "A2"));
        assert_eq!(s.balance, Some((day("2026-10-03"), Money(84510))));

        let camt = r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02"><BkToCstmrStmt><Stmt><Acct><Id><IBAN>FR7600000000000000000000000</IBAN></Id></Acct>
            <Bal><Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp><Amt Ccy="EUR">845.10</Amt><CdtDbtInd>CRDT</CdtDbtInd><Dt><Dt>2026-10-03</Dt></Dt></Bal>
            <Ntry><Amt Ccy="EUR">62.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2026-10-05</Dt></BookgDt><AcctSvcrRef>R9</AcctSvcrRef>
            <NtryDtls><TxDtls><RltdPties><Cdtr><Nm>EDF</Nm></Cdtr></RltdPties><RmtInf><Ustrd>Facture 42</Ustrd></RmtInf></TxDtls></NtryDtls></Ntry>
            </Stmt></BkToCstmrStmt></Document>"#;
        let s = read(camt).unwrap();
        assert_eq!(s.movements[0].amount, Money(-6200));
        assert_eq!(s.movements[0].label, "EDF Facture 42");
        assert_eq!(s.balance, Some((day("2026-10-03"), Money(84510))));

        // A bank's layout with its balance written above the header.
        let csv = "\u{feff}Numéro Compte ;0123456A\nType ;CCP\nCompte tenu en : ;euros\nDate ;03/10/2026\nSolde (EUROS) ;845,10\n\nDate;Libellé;Montant(EUROS)\n01/10/2026;\"PRLV SEPA AGENCE LOYERS\";-780,00\n02/10/2026;VIREMENT DE SALAIRE;1 200,00\n";
        let s = read(csv).unwrap();
        assert_eq!(s.account, "0123456A");
        assert_eq!(s.movements.len(), 2);
        assert_eq!(s.movements[0].amount, Money(-78000));
        assert_eq!(s.balance, Some((day("2026-10-03"), Money(84510))));
        // Debit and credit apart, commas.
        let csv = "Date,Description,Debit,Credit\n2026-10-01,Rent,780.00,\n2026-10-02,Wages,,1200.00\n";
        let s = read(csv).unwrap();
        assert_eq!(s.movements.iter().map(|m| m.amount).collect::<Vec<_>>(), vec![Money(-78000), Money(120000)]);
        // PayPal, in French: net of fees, the name and the type, the balance on each row; a pending one and dollars left out.
        let paypal = "\u{feff}\"Date\",\"Heure\",\"Fuseau horaire\",\"Nom\",\"Type\",\"État\",\"Devise\",\"Avant commission\",\"Commission\",\"Net\",\"Solde\",\"Numéro de transaction\"\n\
            \"01/10/2026\",\"10:02:11\",\"Europe/Paris\",\"Jean Exemple\",\"Paiement de don\",\"Terminé\",\"EUR\",\"12,00\",\"-0,77\",\"11,23\",\"111,23\",\"T1\"\n\
            \"02/10/2026\",\"09:00:00\",\"Europe/Paris\",\"Web Exemple\",\"Paiement Express Checkout\",\"Terminé\",\"EUR\",\"-76,50\",\"0,00\",\"-76,50\",\"34,73\",\"T2\"\n\
            \"02/10/2026\",\"09:30:00\",\"Europe/Paris\",\"Someone\",\"Paiement envoyé\",\"En attente\",\"EUR\",\"-5,00\",\"0,00\",\"-5,00\",\"34,73\",\"T3\"\n\
            \"02/10/2026\",\"09:40:00\",\"Europe/Paris\",\"Shop US\",\"Paiement\",\"Terminé\",\"USD\",\"-10,00\",\"0,00\",\"-10,00\",\"0,00\",\"T4\"\n";
        let s = read(paypal).unwrap();
        assert_eq!(s.movements.iter().map(|m| (m.amount, m.label.as_str(), m.id.as_str())).collect::<Vec<_>>(), vec![(Money(1123), "Jean Exemple · Paiement de don", "T1"), (Money(-7650), "Web Exemple · Paiement Express Checkout", "T2")]);
        assert_eq!(s.balance, Some((day("2026-10-02"), Money(3473))));
        // Stripe's balance history: created, net, description and type.
        let stripe = "id,Type,Source,Amount,Fee,Net,Currency,Created (UTC),Available On (UTC),Description\n\
            txn_1,charge,ch_1,25.00,-1.03,23.97,eur,2026-10-01 10:00:00,2026-10-08 00:00:00,Support\n\
            txn_2,payout,po_1,-100.00,0.00,-100.00,eur,2026-10-03 06:00:00,2026-10-03 06:00:00,STRIPE PAYOUT\n";
        let s = read(stripe).unwrap();
        assert_eq!(s.movements.iter().map(|m| (m.amount, m.label.as_str(), m.id.as_str())).collect::<Vec<_>>(), vec![(Money(2397), "Support · charge", "txn_1"), (Money(-10000), "STRIPE PAYOUT · payout", "txn_2")]);
    }

    #[test]
    fn missed_changed_and_short() {
        let ledger: Ledger = toml::from_str(
            "[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n\
             [[preset]]\nid = 'rent'\nbudget = 'home'\nlabel = 'Loyer agence'\namount = -780\nevery = 'month'\nday = 1\n\
             [[preset]]\nid = 'edf'\nbudget = 'home'\nlabel = 'EDF'\namount = -62\nevery = 'month'\nday = 5\n\
             [[preset]]\nid = 'insurance'\nbudget = 'home'\nlabel = 'Accident insurance'\namount = -15\nevery = 'month'\nday = 10\n\
             [[preset]]\nid = 'wages'\nbudget = 'home'\nlabel = 'Salaire'\namount = 1200\nevery = 'month'\nday = 28\n\
             [[reserve]]\nid = 'savings'\ntitle = 'Livret A'\nbalance = 2000\nas_of = 2026-09-30\n",
        )
        .unwrap();
        let mut bank = Bank::default();
        let movement = |date: &str, amount: i64, label: &str| Movement { account: "a".into(), date: day(date), amount: Money(amount), label: label.into(), id: format!("{date}{amount}") };
        bank.movements = vec![
            movement("2026-08-01", -78000, "PRLV LOYER AGENCE"),
            movement("2026-08-05", -6200, "PRLV EDF"),
            movement("2026-08-10", -1500, "PRLV ACCIDENT INSURANCE"),
            movement("2026-08-28", 120000, "VIR SALAIRE"),
            movement("2026-09-01", -78000, "PRLV LOYER AGENCE"),
            movement("2026-09-05", -7140, "PRLV EDF"),
            // The insurance did not pass in September.
            movement("2026-09-28", 120000, "VIR SALAIRE"),
            movement("2026-10-01", -78000, "PRLV LOYER AGENCE"),
        ];
        bank.accounts = vec![Account { id: "a".into(), title: String::new(), balance: Some((day("2026-10-01"), Money(30000))) }];
        let watch = watch(&bank, &ledger, day("2026-10-03"));
        assert!(watch.findings.contains(&Finding::Missed { label: "Accident insurance".into(), amount: Money(-1500), date: day("2026-09-10") }), "{:#?}", watch.findings);
        assert!(watch.findings.contains(&Finding::Changed { label: "EDF".into(), expected: Money(-6200), actual: Money(-7140), date: day("2026-09-05") }), "{:#?}", watch.findings);
        // €300 on 1 October; EDF (62) on the 5th, insurance (15) on the 10th, wages on the 28th, then rent (780) on 1 November.
        // Before the wages: 300 − 62 − 15 = 223; after: 1423; the rent leaves 643: no shortfall this month.
        assert!(!watch.findings.iter().any(|f| matches!(f, Finding::Short { .. })), "{:#?}", watch.findings);
        assert_eq!(watch.balance_on(day("2026-10-12")), Some(Money(22300)));
        assert_eq!(watch.coming.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), vec!["EDF", "Accident insurance"], "the next seven days, the 10th included");
        // Without the wages: short on 1 November, the savings cover it.
        let mut poorer = ledger.clone();
        poorer.presets.retain(|p| p.label != "Salaire");
        let watch = super::watch(&bank, &poorer, day("2026-10-03"));
        let short = watch.findings.iter().find(|f| matches!(f, Finding::Short { .. })).expect("short");
        assert_eq!(short, &Finding::Short { label: "Loyer agence".into(), amount: Money(-78000), date: day("2026-11-01"), short: Money(55700), reserve: Some("Livret A".into()) });
    }

    #[test]
    fn odd_exports_read_safely() {
        // A footer in the date column, a long digit run: no panic, no movement.
        assert_eq!((parse_date("Total général"), parse_date("Date d’opération")), (None, None));
        assert_eq!((parse_amount("99999999999999999"), parse_amount("99999999999999999999")), (None, None));
        let csv = "Date;Libellé;Montant\n01/10/2026;LOYER;-780,00\nTotal général;;-780,00\n";
        assert_eq!(read(csv).unwrap().movements.len(), 1);
        // PayPal's American download: the month first.
        let paypal = "\"Date\",\"Time\",\"TimeZone\",\"Name\",\"Type\",\"Status\",\"Currency\",\"Gross\",\"Fee\",\"Net\",\"Balance\",\"Transaction ID\"\n\
            \"10/01/2026\",\"10:02:11\",\"PDT\",\"Jane Example\",\"Donation Payment\",\"Completed\",\"USD\",\"12.00\",\"-0.77\",\"11.23\",\"111.23\",\"U1\"\n\
            \"10/13/2026\",\"09:00:00\",\"PDT\",\"Web Example\",\"Express Checkout Payment\",\"Completed\",\"USD\",\"-76.50\",\"0.00\",\"-76.50\",\"34.73\",\"U2\"\n";
        let s = read(paypal).unwrap();
        assert_eq!(s.movements.iter().map(|m| (m.date, m.amount)).collect::<Vec<_>>(), vec![(day("2026-10-01"), Money(1123)), (day("2026-10-13"), Money(-7650))]);
        assert_eq!(s.balance, Some((day("2026-10-13"), Money(3473))));
        // OFX's entities.
        let ofx = "OFXHEADER:100\n<OFX><STMTTRN>\n<DTPOSTED>20261001\n<TRNAMT>-12.00\n<FITID>B1\n<NAME>M&amp;S\n</STMTTRN></OFX>";
        assert_eq!(read(ofx).unwrap().movements[0].label, "M&S");
        // camt.053.001.08, written with line breaks, two days in one file, names under `Pty`.
        let camt = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.08">
  <BkToCstmrStmt>
    <Stmt>
      <Acct><Id><IBAN>FR7600000000000000000000000</IBAN></Id></Acct>
      <Bal>
        <Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp>
        <Amt Ccy="EUR">900.00</Amt>
        <CdtDbtInd>CRDT</CdtDbtInd>
        <Dt><Dt>2026-10-01</Dt></Dt>
      </Bal>
      <Ntry>
        <Amt Ccy="EUR">62.00</Amt>
        <CdtDbtInd>DBIT</CdtDbtInd>
        <Sts>
          <Cd>BOOK</Cd>
        </Sts>
        <BookgDt><Dt>2026-10-01</Dt></BookgDt>
        <NtryDtls><TxDtls><RltdPties><Cdtr><Pty><Nm>EDF</Nm></Pty></Cdtr></RltdPties><RmtInf><Ustrd>Facture 42</Ustrd></RmtInf></TxDtls></NtryDtls>
      </Ntry>
    </Stmt>
    <Stmt>
      <Acct><Id><IBAN>FR7600000000000000000000000</IBAN></Id></Acct>
      <Bal>
        <Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp>
        <Amt Ccy="EUR">858.00</Amt>
        <CdtDbtInd>CRDT</CdtDbtInd>
        <Dt><Dt>2026-10-02</Dt></Dt>
      </Bal>
      <Ntry>
        <Amt Ccy="EUR">5.00</Amt>
        <CdtDbtInd>DBIT</CdtDbtInd>
        <Sts>
          <Cd>PDNG</Cd>
        </Sts>
        <BookgDt><Dt>2026-10-02</Dt></BookgDt>
      </Ntry>
      <Ntry>
        <Amt Ccy="EUR">20.00</Amt>
        <CdtDbtInd>CRDT</CdtDbtInd>
        <Sts>
          <Cd>BOOK</Cd>
        </Sts>
        <BookgDt><Dt>2026-10-02</Dt></BookgDt>
        <AddtlNtryInf>VIR EXEMPLE</AddtlNtryInf>
      </Ntry>
    </Stmt>
  </BkToCstmrStmt>
</Document>"#;
        let s = read(camt).unwrap();
        assert_eq!(s.movements.iter().map(|m| (m.amount, m.label.as_str())).collect::<Vec<_>>(), vec![(Money(-6200), "EDF Facture 42"), (Money(2000), "VIR EXEMPLE")]);
        assert_eq!(s.balance, Some((day("2026-10-02"), Money(85800))));
    }

    /// A real export, named by SIOUL_TEST_EXPORT, read: how many movements and
    /// their days, whether a balance was found; no amount, no label. Run by hand.
    #[test]
    #[ignore]
    fn a_real_export_reads() {
        let path = std::env::var("SIOUL_TEST_EXPORT").expect("SIOUL_TEST_EXPORT");
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
        let s = read(&text).unwrap();
        let first = s.movements.iter().map(|m| m.date).min();
        let last = s.movements.iter().map(|m| m.date).max();
        let ids = s.movements.iter().filter(|m| !m.id.is_empty()).count();
        println!("movements: {}, from {first:?} to {last:?}, with their own id: {ids}, balance found: {}", s.movements.len(), s.balance.is_some());
        assert!(!s.movements.is_empty());
    }

    #[test]
    fn imported_once() {
        let root = std::env::temp_dir().join(format!("sioul-bank-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let csv = "Date;Libellé;Montant\n01/10/2026;LOYER;-780,00\n01/10/2026;LOYER;-780,00\n02/10/2026;SALAIRE;1200,00\n";
        let statement = read(csv).unwrap();
        let mut bank = Bank::load(&root).unwrap();
        assert_eq!(bank.import(&statement, "Courant"), Imported { read: 3, new: 3 }, "two same movements on one day are two");
        assert_eq!(bank.import(&statement, "Courant"), Imported { read: 3, new: 0 });
        bank.save().unwrap();
        let again = Bank::load(&root).unwrap();
        assert_eq!(again.movements, bank.movements);
        assert_eq!(again.accounts, bank.accounts);
        let _ = std::fs::remove_dir_all(&root);
    }
}

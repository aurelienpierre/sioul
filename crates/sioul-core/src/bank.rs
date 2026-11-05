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
    /// The lines it says are not final yet (card holds, `BankWords::pending`;
    /// CAMT's `PDNG`): never movements, which `sioul-bank.toml` keeps for good;
    /// the account's holds in `sioul-movements.toml` (`movements::Hold`).
    pub pending: Vec<Movement>,
    /// What the export says of its booked lines' fees, by their ids: what
    /// the payer paid and the fee kept (PayPal's Gross and Fee, Stripe's
    /// Amount and Fee, Revolut's and Wise's fees, CAMT's charges). Never in
    /// `sioul-bank.toml`, which every version writes whole: in the store's
    /// `[[fee]]` (`movements::acts::take_in`).
    pub fees: Vec<RowFee>,
}

/// One booked line's fee, as its export says it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowFee {
    /// The line's id (`Movement::id`), given once the ids are made (`read`).
    pub id: String,
    /// What the payer paid (money in), or the price (money out), signed as the
    /// line; none when the export gives only the fee.
    pub gross: Option<Money>,
    /// The fee kept, without sign.
    pub fee: Money,
    /// A bank's charges (CAMT's `Chrgs`), not a processor's fee.
    pub bank: bool,
    /// The line's place among the statement's movements, until its id is known.
    at: usize,
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

/// Reads an export, whatever its form. `words`: a CSV's column names, the
/// statuses of rows left out, the lines above its header (`words::BankWords`).
pub fn read(words: &crate::words::BankWords, text: &str) -> Result<Statement, String> {
    let head: String = text.chars().take(2000).collect::<String>().to_ascii_uppercase();
    let mut statement = if head.contains("<OFX>") || head.contains("OFXHEADER") {
        read_ofx(text)
    } else if head.contains("CAMT.053") || head.contains("<BKTOCSTMR") {
        read_camt(text)?
    } else {
        read_csv(words, text)?
    };
    // Ids made where the bank gives none: the same movement read twice keeps its id.
    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let account = statement.account.clone();
    for m in statement.movements.iter_mut().chain(statement.pending.iter_mut()) {
        m.account = account.clone();
        if m.id.is_empty() {
            let base = format!("{}|{}|{}", m.date, m.amount.cents(), m.label);
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            m.id = format!("{:016x}", fnv(&format!("{base}|{n}")));
        }
    }
    for fee in &mut statement.fees {
        fee.id = statement.movements.get(fee.at).map(|m| m.id.clone()).unwrap_or_default();
    }
    statement.fees.retain(|f| !f.id.is_empty());
    if statement.movements.is_empty() && statement.balance.is_none() && statement.pending.is_empty() {
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
            // Booked entries are movements; pending ones (a card hold) are not final yet; the rest, nothing.
            let status = text_at(entry, &["Sts"]).or_else(|| text_at(entry, &["Sts", "Cd"]));
            let pending = status.as_deref() == Some("PDNG");
            if status.is_some_and(|s| s != "BOOK") && !pending {
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
            // The SEPA mandate and the creditor of a direct debit, said in its
            // label for the matching's keys; only beside the bank's own id, since
            // a label makes the id of a line without one, and the same file
            // taken in again must add nothing.
            let mandate = details.and_then(|d| text_at(d, &["Refs", "MndtId"]));
            let creditor = details.and_then(|d| text_at(d, &["RltdPties", "Cdtr", "Id", "PrvtId", "Othr", "Id"]).or_else(|| text_at(d, &["RltdPties", "Cdtr", "Pty", "Id", "PrvtId", "Othr", "Id"])));
            let label = if id.is_empty() { label } else { [Some(label), mandate.map(|m| format!("RUM {m}")), creditor.map(|c| format!("ICS {c}"))].into_iter().flatten().filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ") };
            let movement = Movement { account: String::new(), date, amount, label, id };
            if pending {
                statement.pending.push(movement);
            } else {
                if let Some((gross, fee)) = camt_fee(entry, amount) {
                    statement.fees.push(RowFee { id: String::new(), gross: Some(gross), fee, bank: true, at: statement.movements.len() });
                }
                statement.movements.push(movement);
            }
        }
    }
    Ok(statement)
}

/// What a booked entry's charges took (CAMT's `Chrgs`), when the bank says
/// they are inside its amount (`ChrgInclInd`), or when its instructed amount
/// (`AmtDtls/InstdAmt`, in the same currency) differs from it: what the
/// payer sent, or the price, and the charges. Charges booked as entries of
/// their own are not this entry's.
fn camt_fee(entry: XmlNode, net: Money) -> Option<(Money, Money)> {
    let currency = |n: XmlNode| path(n, &["Amt"]).and_then(|a| a.attribute("Ccy")).map(str::to_string);
    let records = entry.descendants().filter(|n| n.tag_name().name() == "Rcrd" && n.parent().is_some_and(|p| p.tag_name().name() == "Chrgs"));
    let included: i64 = records.filter(|r| text_at(*r, &["ChrgInclInd"]).is_some_and(|v| v.eq_ignore_ascii_case("true"))).filter_map(|r| text_at(r, &["Amt"]).and_then(|a| parse_amount(&a))).map(|m| m.cents().abs()).sum();
    if included > 0 {
        return Some((net + Money(included), Money(included)));
    }
    let instructed = entry.descendants().find(|n| n.tag_name().name() == "InstdAmt")?;
    if currency(instructed) != currency(entry) {
        return None;
    }
    let sent = parse_amount(&text_at(instructed, &["Amt"])?)?.abs();
    let gross = if net.is_negative() { Money(-sent.cents()) } else { sent };
    let fee = gross - net;
    (fee.cents() > 0).then_some((gross, fee))
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
    /// The amount read is the net of fees (a `net` column: PayPal, Stripe).
    net: bool,
    /// What the payer paid, before the fees (PayPal's Gross, Stripe's Amount beside its Net).
    gross: Option<usize>,
    /// The row's fee (PayPal's, Stripe's, Revolut's Fee, Wise's Total fees).
    fee: Option<usize>,
    /// Without a net column, the fee is taken on top of the amount (Revolut)
    /// rather than inside it (Wise), as the running balance says.
    on_top: bool,
}

/// A bank's CSV, PayPal's and Stripe's too: the header row found by its words
/// (date, label or name, amount or net, or debit and credit), `;` or `,`,
/// decimal commas; a balance written above it read too (as some French banks
/// write it: "Solde (EUROS) ;1 234,56" under "Date ;03/10/2026"), or one on each row
/// (PayPal). Net amounts, fees deducted, when given; rows in another currency
/// than most of the file's, and those refused, cancelled or pending, left out.
fn read_csv(words: &crate::words::BankWords, text: &str) -> Result<Statement, String> {
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
            // A column found by the start of its name; `exact`, by its whole name.
            let find = |list: &[String]| names.iter().position(|n| list.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && n.starts_with(&w)));
            let exact = |list: &[String]| names.iter().position(|n| list.iter().any(|w| crate::words::folded(w) == *n));
            let columns = &words.columns;
            let date = find(&columns.date);
            let label = find(&columns.label).or_else(|| exact(&columns.name));
            // Net of fees first (PayPal, Stripe), else the amount; the amount
            // beside a net is what the payer paid (Stripe's Amount).
            let net = exact(&columns.net);
            let plain = find(&columns.amount).filter(|a| Some(*a) != net && exact(&columns.gross) != Some(*a) && exact(&columns.fee) != Some(*a));
            let amount = net.or(plain);
            let (debit, credit) = (find(&columns.debit), find(&columns.credit));
            if let (Some(date), Some(label)) = (date, label)
                && (amount.is_some() || debit.is_some() || credit.is_some())
                && row.len() >= 3
            {
                let kind = exact(&columns.kind).filter(|k| *k != label);
                // Stripe's Description beside its Type; PayPal's Name.
                let label = if names.get(label).is_some_and(|n| n.starts_with("description")) { label } else { exact(&columns.name).unwrap_or(label) };
                header = Some(Columns {
                    date,
                    label,
                    amount,
                    debit,
                    credit,
                    kind,
                    status: exact(&columns.status),
                    impact: exact(&columns.impact),
                    balance: exact(&columns.balance),
                    time: exact(&columns.time),
                    currency: exact(&columns.currency),
                    id: exact(&columns.id),
                    net: net.is_some(),
                    gross: exact(&columns.gross).or_else(|| net.and(plain)),
                    fee: exact(&columns.fee),
                    on_top: false,
                });
                continue;
            }
            // Above the header: an account, a balance, its day.
            let first = names.first().cloned().unwrap_or_default();
            let second = row.get(1).cloned().unwrap_or_default();
            let starts = |list: &[String]| list.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && first.starts_with(&w));
            if starts(&words.above.balance) {
                pre_balance = parse_amount(&second);
            } else if starts(&words.above.date) {
                pre_date = parse_date(&second);
            } else if starts(&words.above.account) && statement.account.is_empty() {
                statement.account = second;
            }
            continue;
        };
        rows.push((row, columns));
    }
    let Some(mut columns) = header else { return Err("no header".into()) };
    columns.on_top = fee_on_top(&columns, &rows);
    for (_, c) in &mut rows {
        c.on_top = columns.on_top;
    }
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
        // A pending row is a card hold, not final yet: kept apart; refused and cancelled ones, nothing.
        let status = c.status.map(|col| fold(row.get(col).map_or("", String::as_str))).unwrap_or_default();
        let said = |list: &[String]| list.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && status.starts_with(&w));
        let pending = said(&words.pending);
        if !pending && said(&words.skipped) {
            continue;
        }
        if c.impact.and_then(|i| row.get(i)).is_some_and(|v| words.memo.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && fold(v).starts_with(&w))) {
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
        if let Some(b) = c.balance.and_then(|b| row.get(b)).and_then(|v| parse_amount(v)).filter(|_| !pending) {
            let when = (date, c.time.and_then(|t| row.get(t)).cloned().unwrap_or_default());
            if latest.as_ref().is_none_or(|(w, _)| when >= *w) {
                latest = Some((when, b));
            }
        }
        let id = c.id.and_then(|i| row.get(i)).cloned().unwrap_or_default();
        let (amount, fee) = row_fee(&c, &row, amount);
        let movement = Movement { account: String::new(), date, amount, label, id };
        if pending {
            statement.pending.push(movement);
        } else {
            if let Some((gross, fee)) = fee {
                statement.fees.push(RowFee { id: String::new(), gross: Some(gross), fee, bank: false, at: statement.movements.len() });
            }
            statement.movements.push(movement);
        }
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

/// A row's net, and what the payer paid with the fee kept when the export
/// says them: the net column's (`net = gross − fee`, the fee a cost without
/// sign); without one, the amount with the fee on top of it (Revolut: the
/// net is the amount less the fee) or inside it (Wise: the amount is the
/// net). Figures that do not add up give no fee.
fn row_fee(c: &Columns, row: &[String], amount: Money) -> (Money, Option<(Money, Money)>) {
    let cell = |col: Option<usize>| col.and_then(|i| row.get(i)).and_then(|v| parse_amount(v));
    let fee = cell(c.fee).map(Money::abs);
    let (net, gross) = match (c.net, fee) {
        (true, _) => (amount, cell(c.gross).or_else(|| fee.map(|f| amount + f))),
        (false, Some(fee)) if c.on_top => (amount - fee, Some(amount)),
        (false, Some(fee)) => (amount, Some(amount + fee)),
        (false, None) => (amount, cell(c.gross)),
    };
    let Some(gross) = gross.filter(|g| *g != net) else { return (net, None) };
    let fee = fee.unwrap_or(gross - net);
    // The same sign, and gross − fee = net to the cent.
    let holds = gross.is_negative() == net.is_negative() && fee.cents() > 0 && gross - fee == net;
    (net, holds.then_some((gross, fee)))
}

/// Without a net column, whether a CSV's fee is taken on top of its amount:
/// what its running balance moves by from one row to the next, read either
/// way the file is ordered, is the amount less the fee more often than the
/// amount alone. None of that said (no balance, no fee), inside.
fn fee_on_top(c: &Columns, rows: &[(Vec<String>, Columns)]) -> bool {
    let (Some(fee), Some(balance), Some(at)) = (c.fee, c.balance, c.amount) else { return false };
    if c.net {
        return false;
    }
    let cell = |row: &[String], i: usize| row.get(i).and_then(|v| parse_amount(v));
    let read: Vec<(Money, Money, Money)> = rows.iter().filter_map(|(row, _)| Some((cell(row, at)?, cell(row, fee).map_or(Money::ZERO, Money::abs), cell(row, balance)?))).collect();
    let (mut on_top, mut inside) = (0, 0);
    for pair in read.windows(2) {
        let ((a0, f0, b0), (a1, f1, b1)) = (pair[0], pair[1]);
        // Oldest first: the second row's balance is the first's moved by the second row; newest first, the other way.
        for (moved, amount, fee) in [(b1 - b0, a1, f1), (b0 - b1, a0, f0)] {
            if fee == Money::ZERO {
                continue;
            }
            if moved == amount - fee {
                on_top += 1;
            } else if moved == amount {
                inside += 1;
            }
        }
    }
    on_top > inside
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
    /// Read, changed by `change` and written back, all under the file's lock,
    /// which the sharing takes too: a change is set over the file as it is
    /// then, never over a copy read earlier. Returns what `change` returns.
    pub fn change<R>(root: &Path, change: impl FnOnce(&mut Bank) -> R) -> Result<R, String> {
        crate::filelock::with_lock(&root.join(MANIFEST), || {
            let mut bank = Bank::load(root)?;
            let out = change(&mut bank);
            bank.save()?;
            Ok(out)
        })
    }

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

    /// Written whole, under the file's lock, which the sharing takes too.
    pub fn save(&self) -> Result<(), String> {
        crate::filelock::with_lock(&self.root.join(MANIFEST), || self.write())
    }

    fn write(&self) -> Result<(), String> {
        let mut doc = toml_edit::DocumentMut::new();
        doc.decor_mut().set_prefix("# Your bank's movements, read from its exports (docs/accounting.md). Kept on your devices only.\n\n");
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

    /// The balance of every account together, and the newest day among them,
    /// each account once: the names a card gives one account (its number, its
    /// IBAN, none: `BankAccount::exports`) are one balance, its newest, never
    /// added up (review of 5 October 2026).
    pub fn balance_in(&self, ledger: &Ledger) -> Option<(Date, Money)> {
        let mut by: std::collections::BTreeMap<String, (Date, Money)> = std::collections::BTreeMap::new();
        for account in &self.accounts {
            let Some((day, amount)) = account.balance else { continue };
            let owner = ledger.bank_accounts.iter().find(|card| card.owns(&account.id)).map_or_else(|| format!("\u{1f}{}", account.id), |card| card.id.clone());
            let kept = by.entry(owner).or_insert((day, amount));
            if day > kept.0 {
                *kept = (day, amount);
            }
        }
        let day = by.values().map(|(d, _)| *d).max()?;
        Some((day, by.values().map(|(_, m)| *m).sum()))
    }

    /// The balance of every account together, and the newest day among them.
    pub fn balance(&self) -> Option<(Date, Money)> {
        let known: Vec<(Date, Money)> = self.accounts.iter().filter_map(|a| a.balance).collect();
        let day = known.iter().map(|(d, _)| *d).max()?;
        Some((day, known.iter().map(|(_, m)| *m).sum()))
    }
}

/// The words of a label that can name a payment: folded, three letters at
/// least, no number, none of `filler`: the words that name nothing, "the",
/// "pour", and what every bank line says ("PRLV SEPA", "CARTE", "VIREMENT";
/// `words::BankWords::filler`). "Choir, the year's fee" is not "CB BAKERY OF THE PORT".
pub(crate) fn words(filler: &[String], text: &str) -> Vec<String> {
    crate::text::fold(text)
        .into_iter()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !w.chars().all(|c| c.is_ascii_digit()) && !filler.iter().any(|f| crate::words::folds_to(f, w)))
        .map(str::to_string)
        .collect()
}

/// Whether a movement stands for a payment expected on `date`: a word of the
/// payment's name in the movement's, or the same amount within days.
pub(crate) fn matches(filler: &[String], movement: &Movement, label: &[String], amount: Money, date: Date) -> bool {
    let near = movement.date >= date.checked_sub(3.days()).unwrap_or(date) && movement.date <= date.checked_add(7.days()).unwrap_or(date);
    if !near || movement.amount.is_negative() != amount.is_negative() {
        return false;
    }
    matches_words(&words(filler, &movement.label), movement, label, amount, date)
}

/// `matches`, the movement's label's words already made (`words`).
pub(crate) fn matches_words(theirs: &[String], movement: &Movement, label: &[String], amount: Money, date: Date) -> bool {
    let near = movement.date >= date.checked_sub(3.days()).unwrap_or(date) && movement.date <= date.checked_add(7.days()).unwrap_or(date);
    if !near || movement.amount.is_negative() != amount.is_negative() {
        return false;
    }
    let named = label.iter().any(|w| theirs.iter().any(|t| t == w));
    let same = (movement.amount.cents() - amount.cents()).abs() <= amount.cents().abs() / 100 + 1;
    named || (same && (movement.date.since(date).map_or(99, |s| s.get_days().abs())) <= 3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::words::Words;

    /// The readers with the packs built in (French and English).
    fn read(text: &str) -> Result<Statement, String> {
        super::read(&Words::builtin().bank, text)
    }

    fn words(text: &str) -> Vec<String> {
        super::words(&Words::builtin().bank.filler, text)
    }

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

    /// Each booked row's fee, as its export says it: PayPal's in French and
    /// English, Stripe's, Revolut's (the fee on top of the amount, as its
    /// running balance shows), Wise's (inside it), and CAMT's charges; a row
    /// whose figures do not add up gives none.
    #[test]
    fn fees_read_from_exports() {
        let fees = |s: &Statement| s.fees.iter().map(|f| (f.id.clone(), f.gross.map(Money::cents), f.fee.cents(), f.bank)).collect::<Vec<_>>();
        let paypal = "\u{feff}\"Date\",\"Heure\",\"Fuseau horaire\",\"Nom\",\"Type\",\"État\",\"Devise\",\"Avant commission\",\"Commission\",\"Net\",\"Solde\",\"Numéro de transaction\"\n\
            \"01/10/2026\",\"10:02:11\",\"Europe/Paris\",\"Client A\",\"Paiement reçu\",\"Terminé\",\"EUR\",\"520,00\",\"-15,43\",\"504,57\",\"504,57\",\"T1\"\n\
            \"02/10/2026\",\"09:00:00\",\"Europe/Paris\",\"Web Exemple\",\"Paiement Express Checkout\",\"Terminé\",\"EUR\",\"-76,50\",\"0,00\",\"-76,50\",\"428,07\",\"T2\"\n\
            \"03/10/2026\",\"09:00:00\",\"Europe/Paris\",\"Client B\",\"Paiement reçu\",\"Terminé\",\"EUR\",\"40,00\",\"-1,51\",\"39,00\",\"467,07\",\"T3\"\n\
            \"04/10/2026\",\"09:00:00\",\"Europe/Paris\",\"\",\"Virement standard\",\"Terminé\",\"EUR\",\"-467,07\",\"0,00\",\"-467,07\",\"0,00\",\"T4\"\n";
        let s = read(paypal).unwrap();
        assert_eq!(s.movements.iter().map(|m| m.amount.cents()).collect::<Vec<_>>(), vec![50457, -7650, 3900, -46707], "the net moved the balance");
        assert_eq!(fees(&s), vec![("T1".to_string(), Some(52000), 1543, false)], "T3's figures do not add up: no fee");
        let english = "\"Date\",\"Time\",\"TimeZone\",\"Name\",\"Type\",\"Status\",\"Currency\",\"Gross\",\"Fee\",\"Net\",\"Balance\",\"Transaction ID\"\n\
            \"10/01/2026\",\"10:02:11\",\"PDT\",\"Client C\",\"Website Payment\",\"Completed\",\"EUR\",\"52.00\",\"-1.86\",\"50.14\",\"50.14\",\"U1\"\n\
            \"10/13/2026\",\"09:00:00\",\"PDT\",\"Client D\",\"Website Payment\",\"Completed\",\"EUR\",\"30.00\",\"-1.22\",\"28.78\",\"78.92\",\"U2\"\n";
        assert_eq!(fees(&read(english).unwrap()), vec![("U1".to_string(), Some(5200), 186, false), ("U2".to_string(), Some(3000), 122, false)]);
        let stripe = "id,Type,Source,Amount,Fee,Net,Currency,Created (UTC),Available On (UTC),Description\n\
            txn_1,charge,ch_1,25.00,-1.03,23.97,eur,2026-10-01 10:00:00,2026-10-08 00:00:00,Support\n\
            txn_2,payout,po_1,-23.97,0.00,-23.97,eur,2026-10-09 06:00:00,2026-10-09 06:00:00,STRIPE PAYOUT\n";
        assert_eq!(fees(&read(stripe).unwrap()), vec![("txn_1".to_string(), Some(2500), 103, false)]);
        // Revolut: the balance moves by the amount less the fee.
        let revolut = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n\
            TOPUP,Current,2026-10-01 08:00:00,2026-10-01 08:00:00,Top-up,100.00,0.00,EUR,COMPLETED,100.00\n\
            EXCHANGE,Current,2026-10-02 08:00:00,2026-10-02 08:00:00,Exchanged to CHF,-50.00,0.50,EUR,COMPLETED,49.50\n\
            CARD_PAYMENT,Current,2026-10-03 08:00:00,2026-10-03 08:00:00,Shop,-10.00,0.00,EUR,REVERTED,49.50\n";
        let s = read(revolut).unwrap();
        assert_eq!(s.movements.iter().map(|m| m.amount.cents()).collect::<Vec<_>>(), vec![10000, -5050], "a reverted row moves nothing");
        assert_eq!(fees(&s), vec![(s.movements[1].id.clone(), Some(-5000), 50, false)]);
        // Wise: the amount already holds the fees.
        let wise = "TransferWise ID,Date,Amount,Currency,Description,Payment Reference,Running Balance,Total fees\n\
            TRANSFER-2,03-10-2026,-10.50,EUR,Sent money to Exemple,,89.50,0.50\n\
            TRANSFER-1,01-10-2026,100.00,EUR,Received money,,100.00,0.00\n";
        let s = read(wise).unwrap();
        assert_eq!(s.movements.iter().map(|m| (m.id.as_str(), m.amount.cents())).collect::<Vec<_>>(), vec![("TRANSFER-2", -1050), ("TRANSFER-1", 10000)]);
        assert_eq!(fees(&s), vec![("TRANSFER-2".to_string(), Some(-1000), 50, false)]);
        // CAMT: charges inside the entry, and an instructed amount that differs.
        let camt = r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02"><BkToCstmrStmt><Stmt><Acct><Id><IBAN>FR7600000000000000000000000</IBAN></Id></Acct>
            <Ntry><Amt Ccy="EUR">98.50</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2026-10-05</Dt></BookgDt><AcctSvcrRef>C1</AcctSvcrRef>
            <Chrgs><Rcrd><Amt Ccy="EUR">1.50</Amt><CdtDbtInd>DBIT</CdtDbtInd><ChrgInclInd>true</ChrgInclInd></Rcrd></Chrgs></Ntry>
            <Ntry><Amt Ccy="EUR">203.10</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2026-10-06</Dt></BookgDt><AcctSvcrRef>C2</AcctSvcrRef>
            <NtryDtls><TxDtls><AmtDtls><InstdAmt><Amt Ccy="EUR">200.00</Amt></InstdAmt></AmtDtls></TxDtls></NtryDtls></Ntry>
            <Ntry><Amt Ccy="EUR">12.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2026-10-06</Dt></BookgDt><AcctSvcrRef>C3</AcctSvcrRef>
            <Chrgs><Rcrd><Amt Ccy="EUR">0.40</Amt><CdtDbtInd>DBIT</CdtDbtInd><ChrgInclInd>false</ChrgInclInd></Rcrd></Chrgs></Ntry>
            </Stmt></BkToCstmrStmt></Document>"#;
        assert_eq!(fees(&read(camt).unwrap()), vec![("C1".to_string(), Some(10000), 150, true), ("C2".to_string(), Some(-20000), 310, true)], "charges booked apart are not the entry's");
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

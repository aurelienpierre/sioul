// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Money: exact amounts, and the amounts written in mail.
//!
//! Amounts are kept in cents so sums never drift. Mail writes them in every
//! national style ("1 234,56 €", "€1,234.56", "$20.00 USD", "-8,49 €").
//!
//! Two readers: `find_amount`, the first amount after a word like "total" or
//! "you received"; and `payment_amount`, for receipts, which also knows that
//! the amount paid is stated in a sentence, on the total's line, or last, and
//! that a company's share capital in the legal footer is never it.

use crate::text::{find_word, fold};
use crate::words::MoneyWords;
use serde::Deserialize;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub};

/// An amount in cents. Written in euros in configuration files: `-250`, `68.5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Deserialize)]
#[serde(from = "f64")]
pub struct Money(pub i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub fn from_units(units: f64) -> Money {
        Money((units * 100.0).round() as i64)
    }

    pub fn cents(self) -> i64 {
        self.0
    }

    pub fn abs(self) -> Money {
        Money(self.0.saturating_abs())
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Whole units and cents, for display: (1234, 56).
    pub fn split(self) -> (i64, i64) {
        let all = self.0.saturating_abs();
        (all / 100, all % 100)
    }
}

impl From<f64> for Money {
    fn from(units: f64) -> Self {
        Money::from_units(units)
    }
}

// Saturating: a hand-written `amount = inf` (or a number past any real
// balance) gives the largest amount, never an overflow.
impl Add for Money {
    type Output = Money;
    fn add(self, other: Money) -> Money {
        Money(self.0.saturating_add(other.0))
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, other: Money) {
        self.0 = self.0.saturating_add(other.0);
    }
}

impl Sub for Money {
    type Output = Money;
    fn sub(self, other: Money) -> Money {
        Money(self.0.saturating_sub(other.0))
    }
}

impl Neg for Money {
    type Output = Money;
    fn neg(self) -> Money {
        Money(self.0.saturating_neg())
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, Add::add)
    }
}

/// An amount found in a message, with its currency (ISO 4217).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amount {
    pub money: Money,
    pub currency: &'static str,
}

/// Currency marks, longest first so "EUR" is not read as something shorter.
const CURRENCIES: &[(&str, &str)] = &[("EUR", "EUR"), ("USD", "USD"), ("GBP", "GBP"), ("CHF", "CHF"), ("CAD", "CAD"), ("€", "EUR"), ("$", "USD"), ("£", "GBP")];

/// The amount a message is about: the first after a word after which it
/// usually comes (`MoneyWords::keywords`: "total", "you received"), else the first.
pub fn find_amount(words: &MoneyWords, text: &str) -> Option<Amount> {
    let chars: Vec<char> = text.chars().collect();
    let amounts = all_amounts(&chars);
    let folded = fold(text);
    let after = words.keywords.iter().filter_map(|k| find_word(&folded, k, 0)).min();
    after
        .and_then(|k| amounts.iter().find(|a| a.start >= k))
        .or_else(|| amounts.first())
        .map(|a| a.amount.clone())
}


/// An amount found in a message, where it was, and what follows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub amount: Amount,
    /// The text after the amount on its line: "à Exemple Retail.", "de Jean (jean@…)".
    pub after: String,
}

/// The amount a payment message is about, in this order of trust:
/// 1. the only amount of the subject ("SHOP: 23,98 € EUR");
/// 2. the amount after a sentence that states the payment ("Vous avez payé 387,00 €");
/// 3. the amount on the total's line, or on the line after its label (receipts set them in a table);
/// 4. the last amount above zero, the way receipts end with what was paid.
///
/// Lines of the legal footer are skipped, so a share capital is never read
/// as a payment. The words: `MoneyWords` (the sentences that state a
/// payment, the labels of the total's line, the legal footer's words).
pub fn payment_amount(words: &MoneyWords, subject: &str, text: &str) -> Option<Found> {
    let in_subject = line_amounts(subject);
    let distinct: Vec<&Amount> = in_subject.iter().map(|(a, _)| a).fold(Vec::new(), |mut v, a| {
        if !v.contains(&a) {
            v.push(a);
        }
        v
    });
    if distinct.len() == 1 {
        return Some(Found { amount: distinct[0].clone(), after: String::new() });
    }
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).filter(|l| !is_legal(words, l)).collect();
    stated(words, &lines).or_else(|| total(words, &lines)).or_else(|| last(&lines))
}

/// The amount stated by a sentence ("Vous avez payé 387,00 € à…"), with what follows it.
pub fn stated_amount(words: &MoneyWords, text: &str) -> Option<Found> {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).filter(|l| !is_legal(words, l)).collect();
    stated(words, &lines)
}

fn is_legal(words: &MoneyWords, line: &str) -> bool {
    let folded = fold(line);
    words.legal.iter().any(|w| find_word(&folded, w, 0).is_some())
}

fn stated(words: &MoneyWords, lines: &[&str]) -> Option<Found> {
    lines.iter().find_map(|line| {
        let folded = fold(line);
        let at = words.stated.iter().filter_map(|p| find_word(&folded, p, 0)).min()?;
        line_amounts_after(line, at).into_iter().next()
    })
}

fn total(words: &MoneyWords, lines: &[&str]) -> Option<Found> {
    lines.iter().enumerate().find_map(|(i, line)| {
        let folded: String = fold(line).into_iter().collect();
        let label = folded.trim_start_matches(|c: char| !c.is_alphanumeric());
        let starts = |list: &[String]| list.iter().map(|t| crate::words::folded(t)).any(|t| !t.is_empty() && label.starts_with(&t));
        let is_total = starts(&words.totals) && !starts(&words.subtotal);
        if !is_total {
            return None;
        }
        let here = line_amounts(line).into_iter().next();
        let next = || lines.get(i + 1).and_then(|l| line_amounts(l).into_iter().next());
        here.or_else(next).map(|(amount, after)| Found { amount, after })
    })
}

fn last(lines: &[&str]) -> Option<Found> {
    lines
        .iter()
        .flat_map(|l| line_amounts(l))
        .filter(|(a, _)| a.money.cents() > 0)
        .last()
        .map(|(amount, after)| Found { amount, after })
}

/// The amounts of one line, each with the rest of the line after it.
fn line_amounts(line: &str) -> Vec<(Amount, String)> {
    line_amounts_after(line, 0).into_iter().map(|f| (f.amount, f.after)).collect()
}

/// The amounts of a line starting at character `from` (as `fold` counts them:
/// folding keeps one character per character).
fn line_amounts_after(line: &str, from: usize) -> Vec<Found> {
    let chars: Vec<char> = line.chars().collect();
    all_amounts(&chars)
        .into_iter()
        .filter(|a| a.start >= from)
        .map(|a| Found { amount: a.amount, after: without_code(chars[a.end.min(chars.len())..].iter().collect::<String>().trim()) })
        .collect()
}

/// "EUR à Exemple" → "à Exemple": a currency code repeated after the symbol is not what follows.
fn without_code(after: &str) -> String {
    CURRENCIES
        .iter()
        .filter(|(mark, _)| mark.len() == 3)
        .find_map(|(mark, _)| after.strip_prefix(mark).filter(|rest| !rest.starts_with(char::is_alphanumeric)))
        .unwrap_or(after)
        .trim()
        .to_string()
}

/// An amount written next to a currency mark, and where it starts and ends.
struct Placed {
    start: usize,
    end: usize,
    amount: Amount,
}

/// Every amount written next to a currency mark, in order. A minus sign right
/// before the number makes it negative ("-8,49 €", a discount).
fn all_amounts(chars: &[char]) -> Vec<Placed> {
    let mut found: Vec<Placed> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match currency_at(chars, i) {
            Some((mark_len, currency)) => {
                let before = number_before(chars, i).map(|(start, money)| (start, i + mark_len, money));
                let after = || number_after(chars, i + mark_len).map(|(start, money, end)| (i.min(start), end, money));
                if let Some((start, end, money)) = before.or_else(after) {
                    let negative = start > 0 && matches!(chars[start - 1], '-' | '−');
                    let money = if negative { -money } else { money };
                    found.push(Placed { start: if negative { start - 1 } else { start }, end, amount: Amount { money, currency } });
                }
                i += mark_len;
            }
            None => i += 1,
        }
    }
    found.sort_by_key(|p| p.start);
    // "€12,00 EUR" is one amount with two marks: the second mark ends it.
    let mut merged: Vec<Placed> = Vec::new();
    for p in found {
        match merged.last_mut() {
            Some(previous) if previous.amount == p.amount && p.start <= previous.end + 1 => previous.end = previous.end.max(p.end),
            _ => merged.push(p),
        }
    }
    merged
}

/// A currency mark starting at `i`: a symbol, or a code that is a whole word.
fn currency_at(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    CURRENCIES.iter().find_map(|(mark, code)| {
        let mark: Vec<char> = mark.chars().collect();
        let matches = chars.get(i..i + mark.len()) == Some(&mark[..]);
        let is_code = mark.len() == 3;
        let bounded = !is_code
            || ((i == 0 || !chars[i - 1].is_alphanumeric()) && chars.get(i + 3).is_none_or(|c| !c.is_alphanumeric()));
        (matches && bounded).then_some((mark.len(), *code))
    })
}

/// Characters that can be inside a written number: digits, separators, spaces.
fn in_number(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | ',' | ' ' | '\u{a0}' | '\u{202f}' | '\'')
}

/// The number that ends just before position `end` (one space allowed).
fn number_before(chars: &[char], end: usize) -> Option<(usize, Money)> {
    let mut stop = end;
    if stop > 0 && matches!(chars[stop - 1], ' ' | '\u{a0}' | '\u{202f}') {
        stop -= 1;
    }
    let mut start = stop;
    while start > 0 && in_number(chars[start - 1]) {
        start -= 1;
    }
    // Separators and spaces before the first digit are not the number's;
    // counted in characters, as `start` is (a narrow no-break space is three bytes).
    let skipped = chars[start..stop].iter().take_while(|c| !c.is_ascii_digit()).count();
    let text: String = chars[start + skipped..stop].iter().collect();
    parse_number(text.trim()).map(|m| (start + skipped, m))
}

/// The number that starts just after position `start` (one space allowed), and where it ends.
fn number_after(chars: &[char], start: usize) -> Option<(usize, Money, usize)> {
    let mut begin = start;
    if chars.get(begin).is_some_and(|c| matches!(c, ' ' | '\u{a0}' | '\u{202f}')) {
        begin += 1;
    }
    let mut end = begin;
    while end < chars.len() && in_number(chars[end]) {
        end += 1;
    }
    let text: String = chars[begin..end].iter().collect();
    let trimmed = text.trim_end();
    parse_number(trimmed.trim()).map(|m| (begin, m, begin + trimmed.chars().count()))
}

/// "1 234,56", "1,234.56", "25,00", "12,5", "1250", "20.00" → cents.
///
/// The last `,` or `.` followed by one or two digits only is the decimal
/// mark; every other separator groups thousands (by threes).
fn parse_number(text: &str) -> Option<Money> {
    let text = text.trim_end_matches(['.', ',', ' ']);
    if !text.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    let decimal_at = text.rfind(['.', ',']).filter(|&p| {
        let tail = &text[p + 1..];
        (1..=2).contains(&tail.len()) && tail.chars().all(|c| c.is_ascii_digit())
    });
    let (whole, cents) = match decimal_at {
        Some(p) if text.len() - p == 2 => (&text[..p], text[p + 1..].parse::<i64>().ok()? * 10),
        Some(p) => (&text[..p], text[p + 1..].parse::<i64>().ok()?),
        None => (text, 0),
    };
    // A run of digits too long for any amount is no amount.
    let units: i64 = grouped_digits(whole).parse().ok()?;
    Some(Money(units.checked_mul(100)?.checked_add(cents)?))
}

/// The digits of a whole part whose thousands are grouped by threes: "1 234" →
/// "1234". When the groups are not threes ("3 25", a reference then a price),
/// only the last group is the number.
fn grouped_digits(whole: &str) -> String {
    let groups: Vec<&str> = whole.split(['.', ',', ' ', '\u{a0}', '\u{202f}', '\'']).filter(|g| !g.is_empty()).collect();
    let well_grouped = groups.first().is_some_and(|g| g.len() <= 3) && groups.iter().skip(1).all(|g| g.len() == 3);
    if well_grouped || groups.len() == 1 { groups.concat() } else { groups.last().map_or_else(String::new, |g| (*g).to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::words::Words;

    /// The readers with the packs built in (French and English).
    fn find_amount(text: &str) -> Option<Amount> {
        super::find_amount(&Words::builtin().money, text)
    }

    fn payment_amount(subject: &str, text: &str) -> Option<Found> {
        super::payment_amount(&Words::builtin().money, subject, text)
    }

    fn amount(text: &str) -> (i64, &'static str) {
        let a = find_amount(text).unwrap();
        (a.money.cents(), a.currency)
    }

    #[test]
    fn national_styles() {
        assert_eq!(amount("Vous avez reçu un paiement de 25,00 € EUR de Jean Exemple"), (2500, "EUR"));
        assert_eq!(amount("A payout of €1,234.56 is on its way"), (123456, "EUR"));
        assert_eq!(amount("Plan Pro … Total $20.00 USD"), (2000, "USD"));
        assert_eq!(amount("Montant TTC\u{a0}: 1\u{202f}234,00\u{a0}€"), (123400, "EUR"));
        assert_eq!(amount("Prix 12 EUR"), (1200, "EUR"));
        assert_eq!(amount("N° 3 25,00 €"), (2500, "EUR"));
    }

    #[test]
    fn the_amount_after_the_keyword() {
        assert_eq!(amount("Abonnement 5,00 € par mois. Total : 60,00 €"), (6000, "EUR"));
        assert!(find_amount("Votre code est 482913").is_none());
    }

    fn paid(subject: &str, text: &str) -> Option<i64> {
        payment_amount(subject, text).map(|f| f.amount.money.cents())
    }

    #[test]
    fn what_was_paid_in_a_receipt() {
        // The subject says it.
        assert_eq!(paid("SHOP: 23,98 € EUR", "Prix 19,99 €\nLivraison 3,99 €"), Some(2398));
        // A sentence says it, and who it went to.
        let found = payment_amount("Reçu", "Merci.\nVous avez payé 387,00 € EUR à Exemple Retail.\n307,50 €").unwrap();
        assert_eq!(found.amount.money.cents(), 38700);
        assert_eq!(found.after, "à Exemple Retail.");
        // The total's label, with the amount on the next line, after a discount.
        assert_eq!(paid("Commande confirmée", "84,99€\n-8,49€\nSous-total\n76,50€\nTotal\n76,50€\nVous avez économisé 8,49€"), Some(7650));
        // Without labels, the last amount above zero; a discount is negative.
        assert_eq!(paid("Reçu de votre paiement", "84,99 € EUR\n0,00 € EUR\n-8,49 € EUR\n76,50 € EUR"), Some(7650));
        // A share capital in the footer is never the payment.
        assert_eq!(paid("Message de notification", "Un message vous attend.\nExemple SA au capital de 1 000 000 € - RCS Paris"), None);
        // One amount with two marks.
        assert_eq!(paid("Paiement reçu", "Vous avez reçu un paiement de €12,00 EUR de Jean Exemple (jean@example.org)."), Some(1200));
    }

    #[test]
    fn untrusted_text_never_panics() {
        // Narrow no-break spaces are three bytes each: the start is counted in characters.
        assert_eq!(amount("\u{202f}\u{202f}\u{202f}\u{202f}1 €"), (100, "EUR"));
        assert_eq!(amount("Prix\u{a0}\u{a0}\u{a0}\u{a0}: 12 €"), (1200, "EUR"));
        // A digit run no amount reaches is none.
        assert!(find_amount("€ 99999999999999999").is_none());
        assert!(find_amount("€ 999999999999999999999999").is_none());
        // One decimal digit is decimals, not a thousands group.
        assert_eq!(amount("Total : 12,5 €"), (1250, "EUR"));
        assert_eq!(amount("Total. 12 €"), (1200, "EUR"));
    }

    #[test]
    fn exact_sums() {
        let total: Money = [Money::from_units(0.1), Money::from_units(0.2)].into_iter().sum();
        assert_eq!(total, Money(30));
        assert_eq!(Money::from_units(-1234.56).split(), (1234, 56));
        // An infinite amount written by hand saturates; sums never overflow.
        let huge = Money::from_units(f64::INFINITY);
        assert_eq!((huge + huge, -Money(i64::MIN), Money(i64::MIN).abs()), (Money(i64::MAX), Money(i64::MAX), Money(i64::MAX)));
    }
}

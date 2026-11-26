// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Numbers never copied as they are out of a message: IBANs, payment card
//! numbers and French social security numbers (NIR), each checked by its own
//! rule (ISO 13616's mod 97, Luhn's, the NIR's key), so that an order number
//! or a phone number stays as written. What agents are handed (the command
//! line's `mcp::mask`) and what a rule copies into a title or a notice
//! (`wiring::engine`) are masked by this one function.

pub const NIR: &str = "[social security number hidden]";

/// A text with its IBANs, card numbers and social security numbers hidden;
/// the last four characters of an IBAN or a card stay, to tell them apart.
pub fn text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let starts_word = i == 0 || !chars[i - 1].is_alphanumeric();
        if starts_word && let Some((end, hidden)) = iban_at(&chars, i).or_else(|| nir_at(&chars, i)).or_else(|| card_at(&chars, i)) {
            out.push_str(&hidden);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// What may stand between the groups of an account or a card number: a
/// space, of any width (HTML mail turned into text has non-breaking ones).
fn separator(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}' | '\u{2007}')
}

/// Runs of `allowed` characters from `start`, a single space (or hyphen,
/// when `hyphens`) allowed between them: each place a run could end, with the
/// characters kept so far. A place is kept only where no letter or digit follows.
fn runs(chars: &[char], start: usize, allowed: impl Fn(char) -> bool, hyphens: bool, most: usize) -> Vec<(usize, String)> {
    let mut ends = Vec::new();
    let mut kept = String::new();
    let mut i = start;
    while i < chars.len() && kept.chars().count() < most {
        if allowed(chars[i]) {
            kept.push(chars[i]);
            i += 1;
            if i == chars.len() || !chars[i].is_alphanumeric() {
                ends.push((i, kept.clone()));
            }
        } else if (separator(chars[i]) || (hyphens && chars[i] == '-')) && i + 1 < chars.len() && allowed(chars[i + 1]) && !kept.is_empty() {
            i += 1;
        } else {
            break;
        }
    }
    ends
}

/// An IBAN starting at `start`, written whole or in groups: ISO 13616, its
/// check digits by mod 97. The longest form that checks out is taken.
pub fn iban_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    let head: Vec<char> = chars.get(start..start + 4)?.to_vec();
    if !(head[0].is_ascii_uppercase() && head[1].is_ascii_uppercase() && head[2].is_ascii_digit() && head[3].is_ascii_digit()) {
        return None;
    }
    let ends = runs(chars, start, |c| c.is_ascii_uppercase() || c.is_ascii_digit(), false, 34);
    ends.into_iter().rev().filter(|(_, kept)| kept.len() >= 15).find(|(_, kept)| iban_checks(kept)).map(|(end, kept)| (end, format!("[IBAN …{}]", &kept[kept.len() - 4..])))
}

fn iban_checks(iban: &str) -> bool {
    let moved = iban[4..].chars().chain(iban[..4].chars());
    let mut rest: u32 = 0;
    for c in moved {
        let value = match c {
            '0'..='9' => c as u32 - '0' as u32,
            'A'..='Z' => c as u32 - 'A' as u32 + 10,
            _ => return false,
        };
        rest = if value >= 10 { (rest * 100 + value) % 97 } else { (rest * 10 + value) % 97 };
    }
    rest == 1
}

/// A payment card number, Luhn's check digit right: 16 or 19 digits from a
/// network's first digit (2 to 6), or 15 from American Express's 34 and 37.
/// Fourteen digits are left alone: a SIRET has a Luhn key too, and is no secret.
pub fn card_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    if !matches!(chars.get(start), Some('2'..='6')) {
        return None;
    }
    let shaped = |digits: &str| match digits.len() {
        16 | 19 => true,
        15 => digits.starts_with("34") || digits.starts_with("37"),
        _ => false,
    };
    let ends = runs(chars, start, |c| c.is_ascii_digit(), true, 19);
    ends.into_iter().rev().filter(|(_, digits)| shaped(digits)).find(|(_, digits)| luhn(digits)).map(|(end, digits)| (end, format!("[card …{}]", &digits[digits.len() - 4..])))
}

fn luhn(digits: &str) -> bool {
    let sum: u32 = digits
        .chars()
        .rev()
        .filter_map(|c| c.to_digit(10))
        .enumerate()
        .map(|(i, d)| if i % 2 == 1 { if d * 2 > 9 { d * 2 - 9 } else { d * 2 } } else { d })
        .sum();
    sum % 10 == 0
}

/// A French social security number (NIR): 15 characters, the first 1, 2, 7
/// or 8, the last two its key, 97 less the first thirteen modulo 97; Corsica's
/// departments, 2A and 2B, count as 19 and 18 in the key.
pub fn nir_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    if !matches!(chars.get(start), Some('1' | '2' | '7' | '8')) {
        return None;
    }
    let ends = runs(chars, start, |c| c.is_ascii_digit() || c == 'A' || c == 'B', false, 15);
    let (end, kept) = ends.into_iter().find(|(_, kept)| kept.len() == 15)?;
    let digits = match &kept[5..7] {
        "2A" => format!("{}19{}", &kept[..5], &kept[7..]),
        "2B" => format!("{}18{}", &kept[..5], &kept[7..]),
        _ => kept,
    };
    let number: u64 = digits[..13].parse().ok()?;
    let key: u64 = digits[13..].parse().ok()?;
    (97 - number % 97 == key).then(|| (end, NIR.to_string()))
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Telling that two things tell of one payment (docs/accounting.md,
//! "Matching"; docs/research/budgets.md, §5): keys first, then a score,
//! then one to one.
//!
//! - **Keys**: the bank's own id within its exports (`bank::Bank::import`,
//!   and a hold posted under its id), the SEPA mandate's reference (RUM) and
//!   the creditor's identifier (ICS), an invoice's, an order's or a tax
//!   notice's reference found in the bank's label, and what Factur-X or UBL
//!   data says (`einvoice`). A key makes a pair sure, even when the amount
//!   changed (the energy bill of the winter).
//! - **Candidates** only (blocking, Christen 2012): the same way, an account
//!   the planned movement may be paid from, an amount within its kind's
//!   tolerance and a day within its kind's window.
//! - **A score** (Fellegi & Sunter 1969): the amount, the days apart, a
//!   reference, and the payee's names compared token by token, TF-IDF with
//!   Jaro–Winkler between tokens ("SoftTFIDF", Cohen et al. 2003).
//! - **One to one**, the best score first, the nearest day on a tie. Sure,
//!   probable (said "probably the same as …", with Not the same beside it),
//!   or kept apart. A pair barely better than the next one is only probable:
//!   two subscriptions of €9.99, two coffees on one day.
//!
//! Nothing here asks the person to validate: an automatic link counts until
//! they correct it, and a correction teaches the payee's name (`movements`).

use crate::money::Money;
use crate::movements::{Sum, Way};
use jiff::ToSpan;
use jiff::civil::{Date, Weekday};
use std::collections::HashMap;

/// What a planned or announced movement is, for its window and its tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Evidence {
    /// A direct debit (a recurring payment, a debit notice): its amount
    /// exactly, around its day, moved off weekends and TARGET2's holidays.
    Debit,
    /// A bill to pay (a letter's, a mail's): its amount exactly, from a
    /// month before its day until a week after.
    Bill,
    /// A card payment told by a receipt or an order: a few percent apart (a
    /// tip, a hold, another currency), from the day before to two weeks after.
    Card,
    /// A transfer, planned or written by hand: its amount exactly, a few days around its day.
    Transfer,
    /// An invoice you issued, its day the one it is due: a client's
    /// transfer of its amount, from a month before to two months after.
    Invoice,
    /// Money given back: up to its amount, up to eight weeks after.
    Refund,
    /// A bill or a debit notice a message told, its day the message's: its
    /// amount exactly, from that day to six weeks after.
    Notice,
}

impl Evidence {
    /// The days a line of the bank may come on, for something expected on `day`.
    pub fn window(self, day: Date) -> (Date, Date) {
        let shift = |n: i64| day.checked_add(n.days()).unwrap_or(day);
        match self {
            // Debits move off weekends and holidays: from two days before to
            // the third business day after the first one on or after its day.
            Evidence::Debit => (shift(-2), business_after(next_business(day), 3)),
            Evidence::Bill => (shift(-31), shift(7)),
            Evidence::Card => (shift(-1), shift(14)),
            Evidence::Transfer => (shift(-3), shift(7)),
            Evidence::Invoice => (shift(-31), shift(62)),
            Evidence::Refund => (day, shift(56)),
            Evidence::Notice => (day, shift(45)),
        }
    }

    /// The day the line is most likely: a debit's first business day on or after its day.
    fn likely(self, day: Date) -> Date {
        match self {
            Evidence::Debit => next_business(day),
            _ => day,
        }
    }
}

/// TARGET2's closing days (the ECB's calendar, which SEPA debits settle by):
/// weekends, New Year's Day, Good Friday, Easter Monday, Labour Day,
/// Christmas Day and the day after.
pub fn business(day: Date) -> bool {
    if matches!(day.weekday(), Weekday::Saturday | Weekday::Sunday) {
        return false;
    }
    let easter = easter(day.year());
    let shift = |n: i64| easter.and_then(|e| e.checked_add(n.days()).ok());
    let fixed = matches!((day.month(), day.day()), (1, 1) | (5, 1) | (12, 25) | (12, 26));
    !(fixed || shift(-2) == Some(day) || shift(1) == Some(day))
}

/// Easter Sunday in the Gregorian calendar (the anonymous algorithm, Meeus 1991).
fn easter(year: i16) -> Option<Date> {
    let y = i32::from(year);
    let (a, b, c) = (y % 19, y / 100, y % 100);
    let (d, e) = (b / 4, b % 4);
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let (i, k) = (c / 4, c % 4);
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    Date::new(year, i8::try_from(month).ok()?, i8::try_from(day).ok()?).ok()
}

/// The first business day on or after `day`.
pub fn next_business(day: Date) -> Date {
    let mut d = day;
    for _ in 0..10 {
        if business(d) {
            return d;
        }
        d = d.tomorrow().unwrap_or(d);
    }
    d
}

/// The `n`-th business day after `day`.
pub fn business_after(day: Date, n: u32) -> Date {
    let mut d = day;
    let mut left = n;
    for _ in 0..(n * 3 + 10) {
        if left == 0 {
            break;
        }
        d = d.tomorrow().unwrap_or(d);
        if business(d) {
            left -= 1;
        }
    }
    d
}

// ---------------------------------------------------------------- names

/// Jaro–Winkler's similarity of two words, 0 to 1 (Winkler 1990; the
/// prefix scale 0.1, four characters at most).
pub fn jaro_winkler(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.is_empty() || b.is_empty() {
        return if a.is_empty() && b.is_empty() { 1.0 } else { 0.0 };
    }
    if a == b {
        return 1.0;
    }
    let reach = (a.len().max(b.len()) / 2).saturating_sub(1);
    let (mut a_hit, mut b_hit) = (vec![false; a.len()], vec![false; b.len()]);
    let mut matches = 0usize;
    for (i, ca) in a.iter().enumerate() {
        let (lo, hi) = (i.saturating_sub(reach), (i + reach + 1).min(b.len()));
        for j in lo..hi {
            if !b_hit[j] && b[j] == *ca {
                a_hit[i] = true;
                b_hit[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let (mut transpositions, mut k) = (0usize, 0usize);
    for (i, ca) in a.iter().enumerate() {
        if !a_hit[i] {
            continue;
        }
        while !b_hit[k] {
            k += 1;
        }
        if *ca != b[k] {
            transpositions += 1;
        }
        k += 1;
    }
    let m = matches as f64;
    let jaro = (m / a.len() as f64 + m / b.len() as f64 + (m - transpositions as f64 / 2.0) / m) / 3.0;
    let prefix = a.iter().zip(b.iter()).take(4).take_while(|(x, y)| x == y).count() as f64;
    jaro + prefix * 0.1 * (1.0 - jaro)
}

/// A name's words as they are compared: folded, letters only (no digit:
/// card numbers, dates and references go), three letters at least (as
/// `bank::words` reads them: "CB" and "SA" name nobody), none of
/// the bank's words that name nothing ("PRLV SEPA", "CB", "VIR":
/// `BankWords::filler`), a payment processor's name first taken off when
/// more follows ("PAYPAL *SHOP" is the shop: `BankWords::processors`).
pub fn tokens(words: &crate::words::BankWords, text: &str) -> Vec<String> {
    let folded: String = crate::text::fold(text).into_iter().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let mut out: Vec<String> = folded
        .split_whitespace()
        .filter(|w| w.chars().count() >= 3 && !w.chars().any(|c| c.is_ascii_digit()) && !words.filler.iter().any(|f| crate::words::folds_to(f, w)))
        .map(str::to_string)
        .collect();
    if out.len() > 1 && words.processors.iter().any(|p| crate::words::folds_to(p, &out[0])) {
        out.remove(0);
    }
    out
}

/// How rare each word is among the names compared: its inverse document frequency.
#[derive(Debug, Clone, Default)]
pub struct Idf {
    seen: HashMap<String, usize>,
    names: usize,
    /// The similarities already said, by the two names (`similarity`): a
    /// recurring payment's name meets the same bank labels month after month.
    said: std::cell::RefCell<HashMap<String, f64>>,
}

impl Idf {
    /// From every name the matching compares, each distinct name counted
    /// once: a preset's name, said by each of its occurrences, is one name,
    /// not a common word.
    pub fn of<'a>(names: impl IntoIterator<Item = &'a Vec<String>>) -> Idf {
        let mut idf = Idf::default();
        let mut distinct: std::collections::BTreeSet<Vec<&String>> = std::collections::BTreeSet::new();
        for name in names {
            let mut words: Vec<&String> = name.iter().collect();
            words.sort();
            words.dedup();
            distinct.insert(words);
        }
        for words in distinct {
            idf.names += 1;
            for w in words {
                *idf.seen.entry(w.clone()).or_insert(0) += 1;
            }
        }
        idf
    }

    /// A word's weight: rarer is heavier, never nothing (one more name than counted).
    fn weight(&self, word: &str) -> f64 {
        let seen = self.seen.get(word).copied().unwrap_or(0);
        ((self.names as f64 + 1.0) / (seen as f64 + 1.0)).ln() + 1.0
    }
}

/// SoftTFIDF (Cohen et al. 2003): the words of one name close to a word of
/// the other (Jaro–Winkler at least 0.9), weighed by their rarity, 0 to 1;
/// the closer of the two ways. The shorter name is also held against the
/// longer one's leading words, as many as it has: a bank writes the payee
/// first, then a card line's city or a debit's creditor's suffix
/// ("EXEMPLEAI SAN FRANCISCO", "RENT AGENCY").
pub fn similarity(a: &[String], b: &[String], idf: &Idf) -> f64 {
    // The same either way: kept once for the pair.
    let (x, y) = (a.join(" "), b.join(" "));
    let key = if x <= y { format!("{x}\u{1f}{y}") } else { format!("{y}\u{1f}{x}") };
    if let Some(said) = idf.said.borrow().get(&key) {
        return *said;
    }
    let sim = similarity_of(a, b, idf);
    idf.said.borrow_mut().insert(key, sim);
    sim
}

fn similarity_of(a: &[String], b: &[String], idf: &Idf) -> f64 {
    let lead = |name: &[String], n: usize| name[..n.min(name.len())].to_vec();
    let whole = soft_tfidf(a, b, idf);
    let leading = if a.len() < b.len() { soft_tfidf(a, &lead(b, a.len()), idf) } else if b.len() < a.len() { soft_tfidf(&lead(a, b.len()), b, idf) } else { 0.0 };
    whole.max(leading)
}

/// SoftTFIDF itself, the closer of its two ways.
fn soft_tfidf(a: &[String], b: &[String], idf: &Idf) -> f64 {
    let one_way = |s: &[String], t: &[String]| -> f64 {
        let norm = |v: &[String]| v.iter().map(|w| idf.weight(w).powi(2)).sum::<f64>().sqrt();
        let (ns, nt) = (norm(s), norm(t));
        if ns == 0.0 || nt == 0.0 {
            return 0.0;
        }
        s.iter()
            .filter_map(|w| t.iter().map(|v| (v, jaro_winkler(w, v))).max_by(|x, y| x.1.total_cmp(&y.1)).filter(|(_, sim)| *sim >= 0.9).map(|(v, sim)| (idf.weight(w) / ns) * (idf.weight(v) / nt) * sim))
            .sum::<f64>()
            .min(1.0)
    };
    one_way(a, b).max(one_way(b, a))
}

// ---------------------------------------------------------------- references

/// The references a text holds, each as it is compared: the creditor's
/// identifier (ICS, "ics:FR12ZZZ123456"), a mandate's reference after its
/// word (RUM, "rum:…"), and the numbers after the words that lead one
/// (`PaymentWords::references`: "facture", "commande", "avis", "invoice",
/// "order"…), digits in them, as `ref:<letters and digits>`.
pub fn references(leads: &[String], text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let upper = text.to_uppercase();
    let words: Vec<&str> = upper.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | '[' | ']')).filter(|w| !w.is_empty()).collect();
    let compact = |w: &str| w.chars().filter(char::is_ascii_alphanumeric).collect::<String>();
    for word in &words {
        let w = compact(word);
        if creditor(&w) {
            out.push(format!("ics:{w}"));
        }
    }
    let leads: Vec<String> = leads.iter().map(|l| crate::words::folded(l)).filter(|l| !l.is_empty()).collect();
    let folded: Vec<String> = words.iter().map(|w| crate::words::folded(w).trim_matches(|c: char| !c.is_alphanumeric()).to_string()).collect();
    for (i, word) in folded.iter().enumerate() {
        let Some(lead) = leads.iter().find(|l| *l == word || l.split_whitespace().last() == Some(word.as_str())) else { continue };
        let mandate = matches!(lead.as_str(), "rum" | "mandat" | "mandate" | "mndtid" | "mandat sepa") || lead.ends_with("mandat");
        // The reference: the next word with a digit, past "n°", "no", ":" and the like.
        for next in words.iter().skip(i + 1).take(3) {
            let w = compact(next);
            // A creditor's identifier is said as one already (`ics:`), never as a number too.
            if creditor(&w) {
                break;
            }
            if w.len() >= 4 && w.len() <= 35 && w.chars().any(|c| c.is_ascii_digit()) {
                out.push(format!("{}:{w}", if mandate { "rum" } else { "ref" }));
                break;
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// A SEPA creditor's identifier (ICS): two letters, two check digits, a
/// business code of three characters (ZZZ most often), the national
/// identifier; France's are thirteen characters long, six digits last.
fn creditor(w: &str) -> bool {
    let b = w.as_bytes();
    let head = b.len() >= 9 && b.len() <= 35 && b[..2].iter().all(u8::is_ascii_uppercase) && b[2..4].iter().all(u8::is_ascii_digit);
    head && (&w[4..7] == "ZZZ" || (w.starts_with("FR") && b.len() == 13 && b[7..].iter().all(u8::is_ascii_digit)))
}

/// A run of digits that is a day ("051026", "20261005"), never a reference.
fn is_date(w: &str) -> bool {
    let n = |range: std::ops::Range<usize>| w.get(range).and_then(|t| t.parse::<i16>().ok());
    let valid = |y: Option<i16>, m: Option<i16>, d: Option<i16>| matches!((y, m, d), (Some(y), Some(m), Some(d)) if (1..=12).contains(&m) && (1..=31).contains(&d) && y >= 0);
    w.chars().all(|c| c.is_ascii_digit())
        && match w.len() {
            6 => valid(n(4..6), n(2..4), n(0..2)),
            8 => valid(n(0..4), n(4..6), n(6..8)) || valid(n(4..8), n(2..4), n(0..2)),
            _ => false,
        }
}

/// The references a bank line's label holds, for keys: those after their
/// words (`references`), and each of its words with a digit in it, five
/// characters at least, as "ref:" (a client's transfer writes the invoice's
/// number bare, "2026-002"); a day's digits, and amounts, too short, are no reference.
pub fn label_references(leads: &[String], label: &str) -> Vec<String> {
    let mut out = references(leads, label);
    for word in label.to_uppercase().split_whitespace() {
        let w: String = word.chars().filter(char::is_ascii_alphanumeric).collect();
        if w.len() >= 5 && w.len() <= 35 && w.chars().any(|c| c.is_ascii_digit()) && !is_date(&w) && !creditor(&w) {
            out.push(format!("ref:{w}"));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// How strongly a shared reference tells one payment: a mandate's or a
/// number's, 8; a creditor's alone (one creditor may hold two mandates), 4.
fn key_weight(a: &[String], b: &[String]) -> f64 {
    let shared = |prefix: &str| a.iter().any(|r| r.starts_with(prefix) && b.contains(r));
    if shared("rum:") || shared("ref:") {
        8.0
    } else if shared("ics:") {
        4.0
    } else {
        0.0
    }
}

// ---------------------------------------------------------------- scoring

/// One side of a pair: what was planned or announced, or what the bank says.
#[derive(Debug, Clone, PartialEq)]
pub struct Side {
    pub way: Way,
    pub sum: Sum,
    pub currency: String,
    pub day: Date,
    pub evidence: Evidence,
    /// The accounts it may be paid from or into (a planned movement: those
    /// that fill its budget; a bank line: its own); empty for any.
    pub accounts: Vec<String>,
    /// Each name it goes by, as `tokens` gives it.
    pub names: Vec<Vec<String>>,
    /// Its references (`references`).
    pub refs: Vec<String>,
}

/// A pair's score and what it rests on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    pub score: f64,
    /// Days between the bank's day and the one most likely.
    pub days: i64,
    /// A shared reference.
    pub key: bool,
}

/// Sure from this score on.
pub const SURE: f64 = 7.0;
/// Probable from this score on; below, kept apart.
pub const PROBABLE: f64 = 4.0;
/// A pair within this of another for one of its sides is only probable.
const MARGIN: f64 = 1.0;

/// The amounts a bank line may say for `e`, signed as it is: exact for a
/// debit, a bill, a transfer and an invoice (a range or "about" as planned);
/// a few percent for a card payment (5 %, a euro at least); up to its
/// amount for a refund; a quarter either way in another currency.
fn tolerance(e: &Side, foreign: bool) -> (Money, Money) {
    let (lo, hi) = e.sum.bounds();
    let widen = |part: i64, floor: i64| {
        let room = (lo.cents().abs().max(hi.cents().abs()) / part).max(floor);
        (Money(lo.cents() - room), Money(hi.cents() + room))
    };
    if foreign {
        return widen(4, 100);
    }
    match e.evidence {
        Evidence::Card => widen(20, 100),
        Evidence::Refund => {
            let full = e.sum.amount();
            let tenth = Money(full.cents() / 10);
            (full.min(tenth), full.max(tenth))
        }
        _ => (Money(lo.cents() - 1), Money(hi.cents() + 1)),
    }
}

/// How well `f` (the bank's line) answers `e` (what was planned or
/// announced); none when it is no candidate.
pub fn score(e: &Side, f: &Side, idf: &Idf) -> Option<Scored> {
    if e.way != f.way {
        return None;
    }
    if !e.accounts.is_empty() && !f.accounts.is_empty() && !f.accounts.iter().any(|a| e.accounts.contains(a)) {
        return None;
    }
    let (from, until) = e.evidence.window(e.day);
    if f.day < from || f.day > until {
        return None;
    }
    let key = key_weight(&e.refs, &f.refs);
    let foreign = !e.currency.eq_ignore_ascii_case(&f.currency);
    let (lo, hi) = tolerance(e, foreign);
    let amount = f.sum.amount();
    let within = amount >= lo && amount <= hi;
    // A payment whose amount changed (the winter's energy bill, a client paying
    // part of an invoice, never more) is the same one, within half of it, by a
    // key, or by a plain name for what is not a card payment nor a refund (as
    // the watch says a debit "changed"): only probable then, the amount no help.
    let delta = (amount.cents() - e.sum.amount().cents()).abs();
    let half = e.sum.amount().cents().abs() / 2;
    // Too far for any name to bridge: no name compared.
    if !within && (delta > half || (key < 8.0 && matches!(e.evidence, Evidence::Card | Evidence::Refund))) {
        return None;
    }
    let names = e.names.iter().flat_map(|a| f.names.iter().map(move |b| (a, b))).filter(|(a, b)| !a.is_empty() && !b.is_empty());
    let named = names.clone().count() > 0;
    let sim = names.map(|(a, b)| similarity(a, b, idf)).fold(0.0, f64::max);
    let changed = delta <= half
        && (key >= 8.0
            || (sim >= 0.5
                && match e.evidence {
                    Evidence::Debit | Evidence::Bill | Evidence::Notice | Evidence::Transfer => true,
                    Evidence::Invoice => amount.cents().abs() <= e.sum.amount().cents().abs(),
                    Evidence::Card | Evidence::Refund => false,
                }));
    if !within && !changed {
        return None;
    }
    // Another currency is said by its name or a reference, never by a near amount alone.
    if foreign && key == 0.0 && sim < 0.5 {
        return None;
    }
    let likely = e.evidence.likely(e.day);
    let days = i64::from((f.day - likely).get_days()).abs();
    let span = i64::from((until - from).get_days()).max(1) as f64;
    let room = ((hi.cents() - lo.cents()).abs() / 2).max(1) as f64;
    let mut s = key;
    s += if delta <= 1 { 3.0 } else if within { 2.0 * (1.0 - (delta as f64 / room).min(1.0)) } else { 0.0 };
    s += 2.0 * (1.0 - (days as f64 / span).min(1.0));
    // Names that share nothing doubt the pair a little, never settle it: the
    // bank's label is cut and coded ("PRLV SEPA ..."), the plan's in your words.
    if named {
        s += if sim >= 0.15 { 4.0 * sim } else { -0.5 };
    }
    Some(Scored { score: s, days, key: key > 0.0 })
}

/// Whether a pair is sure or only probable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// "The same payment", with its sources.
    Sure,
    /// "Probably the same as …", with Not the same beside it.
    Probable,
}

/// One pair kept: what was planned or announced (`expected[e]`), the bank's line (`facts[f]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    pub expected: usize,
    pub fact: usize,
    pub outcome: Outcome,
    pub score: f64,
}

/// Pairs one to one, the best score first, the nearest day on a tie (then
/// the order given). Sure from `SURE` when a reference says it, or when no
/// other candidate of either side comes within `MARGIN`; probable from
/// `PROBABLE`; `allowed` says which pairs may be made at all ("Not the same").
pub fn assign(expected: &[Side], facts: &[Side], idf: &Idf, allowed: &dyn Fn(usize, usize) -> bool) -> Vec<Pair> {
    // Blocking by day: the bank's lines in day order, each planned one's window searched.
    let mut by_day: Vec<usize> = (0..facts.len()).collect();
    by_day.sort_by_key(|&j| facts[j].day);
    let mut pairs: Vec<(usize, usize, Scored)> = Vec::new();
    for (i, e) in expected.iter().enumerate() {
        let (from, until) = e.evidence.window(e.day);
        let start = by_day.partition_point(|&j| facts[j].day < from);
        for &j in by_day[start..].iter().take_while(|&&j| facts[j].day <= until) {
            if let Some(scored) = score(e, &facts[j], idf).filter(|s| s.score >= PROBABLE)
                && allowed(i, j)
            {
                pairs.push((i, j, scored));
            }
        }
    }
    // Each side's two best candidates, for the margin: a pair barely better than the next is only probable.
    let mut best_e: HashMap<usize, [(f64, usize); 2]> = HashMap::new();
    let mut best_f: HashMap<usize, [(f64, usize); 2]> = HashMap::new();
    let keep = |best: &mut [(f64, usize); 2], score: f64, other: usize| {
        if score > best[0].0 {
            best[1] = best[0];
            best[0] = (score, other);
        } else if score > best[1].0 {
            best[1] = (score, other);
        }
    };
    for (i, j, scored) in &pairs {
        keep(best_e.entry(*i).or_insert([(f64::MIN, usize::MAX); 2]), scored.score, *j);
        keep(best_f.entry(*j).or_insert([(f64::MIN, usize::MAX); 2]), scored.score, *i);
    }
    let other = |best: Option<&[(f64, usize); 2]>, this: usize| best.map_or(f64::MIN, |b| if b[0].1 == this { b[1].0 } else { b[0].0 });
    let close = |i: usize, j: usize, s: f64| other(best_e.get(&i), j) >= s - MARGIN || other(best_f.get(&j), i) >= s - MARGIN;
    let mut order: Vec<usize> = (0..pairs.len()).collect();
    order.sort_by(|&x, &y| pairs[y].2.score.total_cmp(&pairs[x].2.score).then(pairs[x].2.days.cmp(&pairs[y].2.days)).then((pairs[x].0, pairs[x].1).cmp(&(pairs[y].0, pairs[y].1))));
    let (mut taken_e, mut taken_f) = (vec![false; expected.len()], vec![false; facts.len()]);
    let mut out = Vec::new();
    for at in order {
        let (i, j, scored) = pairs[at];
        if taken_e[i] || taken_f[j] {
            continue;
        }
        taken_e[i] = true;
        taken_f[j] = true;
        let sure = scored.score >= SURE && (scored.key || !close(i, j, scored.score));
        out.push(Pair { expected: i, fact: j, outcome: if sure { Outcome::Sure } else { Outcome::Probable }, score: scored.score });
    }
    out.sort_by_key(|p| (p.expected, p.fact));
    out
}

/// A set of two or three `candidates` whose amounts add up to `target`: to
/// the cent when `part` is 0, else within that part of it (100: a hundredth,
/// for an order charged in parts). The first set found, in their order.
pub fn adding_up(target: Money, candidates: &[(usize, Money)], part: i64) -> Option<Vec<usize>> {
    // `part` 0: to the cent; else within that part of the target.
    let room = if part == 0 { 1 } else { (target.cents().abs() / part).max(1) };
    let near = |sum: i64| (sum - target.cents()).abs() <= room;
    for a in 0..candidates.len() {
        for b in (a + 1)..candidates.len() {
            if near(candidates[a].1.cents() + candidates[b].1.cents()) {
                return Some(vec![candidates[a].0, candidates[b].0]);
            }
            for c in (b + 1)..candidates.len().min(b + 1 + 40) {
                if near(candidates[a].1.cents() + candidates[b].1.cents() + candidates[c].1.cents()) {
                    return Some(vec![candidates[a].0, candidates[b].0, candidates[c].0]);
                }
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "matching_tests.rs"]
mod tests;

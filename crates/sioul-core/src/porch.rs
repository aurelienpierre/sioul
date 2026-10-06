// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Porch: where new mail waits, checked and sorted, until you look.
//!
//! Every message gets one lane and the reasons for it. The order of the checks
//! is the order of what protects you most: forged mail first, then spam, then
//! the short-lived codes that cannot wait, then cases, then the rest
//! (docs/porch.md). The core returns facts; the sentences are the
//! translator's (`i18n`), in your language.

use crate::card::Card;
use crate::cases::{CaseStore, RouteMatch};
use crate::codes::{self, CodeKind, OneTimeCode};
use crate::config::{Priority, Source};
use crate::lookalike;
use crate::maildir;
use crate::state::PorchState;
use crate::trust::{self, Proof, Trust};
use std::path::{Path, PathBuf};

/// Where a message waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lane {
    /// A one-time code, a password, a reset, a link you asked for: shown at
    /// once, the one exception to the windows (with a warning when its sender is not verified).
    RightNow,
    /// Forged, or flagged as spam: set aside, never deleted.
    SetAside,
    /// It belongs to a case (its id).
    Case(String),
    /// From someone new: it waits until you let them in.
    Screener,
    /// Newsletters and automatic notifications: filed, readable anytime.
    Filed,
    /// From people you know, outside any case.
    People,
    /// From an account you ranked below the others: folded at the bottom.
    Low,
    /// To a shielded public address (its account), from someone you have not let in.
    Public(String),
    /// To a shielded address, and hostile: set aside, its words never shown.
    Hostile,
}

/// Why a message is where it is.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    /// A shielded address's message read as hostile: insults, harassment, threats.
    Hostile,
    /// A shielded address's message, calm or rude, and what it is about.
    Public(crate::shield::Topic),
    /// Why the sender is believed or not.
    Trust(Proof),
    Forged,
    Spam { source: &'static str, score: Option<f32> },
    /// You blocked the sender: set aside for good, never shown.
    Blocked,
    /// The name shown claims a brand the address does not belong to.
    Impersonation { brand: String, domain: String },
    /// A code, password, reset or link: shown at once.
    ExpiresSoon(CodeKind),
    /// It looks like a code, but its sender is not verified: shown at once, with a warning.
    UnverifiedCode(CodeKind),
    /// Verified, from one of your own addresses to another: always let in.
    FromYourself,
    Case { case_id: String, matched: Vec<RouteMatch> },
    Newsletter,
    Automatic,
    FirstMessage,
    KnownPerson,
    /// It came through an account you ranked below the others.
    LowPriority,
}

/// A message with its lane and the reasons, as the Porch shows it.
#[derive(Debug, Clone)]
pub struct Triaged {
    pub card: Card,
    pub lane: Lane,
    pub trust: Trust,
    pub code: Option<OneTimeCode>,
    pub reasons: Vec<Reason>,
    /// The priority of the account it came through.
    pub priority: Priority,
    /// The checks the verdict rests on: Sioul's, else the provider's.
    pub checks: Option<trust::AuthResults>,
    /// For a shielded address: its tone and topic.
    pub assessment: Option<crate::shield::Assessment>,
}

/// A list of who may reach you, one entry per line, `#` for comments: an
/// address, or a pattern holding `*` for any run of characters
/// (`*@example.org`: everyone there; `*@*.example.org`: its subdomains;
/// `news*@example.org`; `@example.org` and a bare `example.org` stand for
/// `*@example.org`); a phone number, `tel:+33465711234`, however written, or
/// a prefix with `*` (`tel:+3319900*`); a contact card, `contact:<its UID>`: all
/// its addresses and numbers, now and later; a category of your contacts,
/// `category:Friends`. Five such lists exist: the senders you let in (the
/// screener's), and who is safe, neutral, restricted or blocked (`Senders`).
#[derive(Debug, Clone, Default)]
pub struct SenderList {
    /// Patterns, lower case, as `normalize` writes them.
    patterns: Vec<String>,
    /// Categories of your contacts, as written (`category:Friends`).
    categories: Vec<String>,
    /// Phone numbers and prefixes, as written after `tel:`: compared by their key (`number_key`).
    numbers: Vec<String>,
    /// Contact cards, by their UID (`contact:…`).
    cards: Vec<String>,
}

/// What starts a line naming a category of your contacts: `category:Friends`.
/// Never a pattern: a pattern has no `:`.
pub const CATEGORY: &str = "category:";
/// What starts a line naming a phone number: `tel:+33465711234`.
pub const TEL: &str = "tel:";
/// What starts a line naming a contact card by its UID: `contact:…`.
pub const CONTACT: &str = "contact:";

/// What follows `head` at the start of a line, case aside; none when empty.
fn after<'a>(line: &'a str, head: &str) -> Option<&'a str> {
    let line = line.trim();
    let start = line.get(..head.len())?;
    start.eq_ignore_ascii_case(head).then(|| line[head.len()..].trim()).filter(|rest| !rest.is_empty())
}

/// The category a line names ("category:Friends" → "Friends"), as written;
/// none for an address or a pattern.
pub fn category_of(line: &str) -> Option<&str> {
    after(line, CATEGORY)
}

/// The card a line names, by its UID ("contact:…").
pub fn card_of(line: &str) -> Option<&str> {
    after(line, CONTACT)
}

/// The number a line names: after `tel:`, or a line that is only a number
/// ("+33 4 65 71 12 34", "04.65.71.12.34", a prefix "+3319900*"); none for
/// anything else.
pub fn number_of(line: &str) -> Option<&str> {
    if let Some(number) = after(line, TEL) {
        return Some(number);
    }
    let line = line.trim();
    let digits = line.chars().filter(char::is_ascii_digit).count();
    (digits >= 3 && line.chars().all(|c| c.is_ascii_digit() || "+*-./() \u{a0}\u{202f}".contains(c))).then_some(line)
}

/// A number's key, as `phones::key` reads it; a prefix keeps its `*`, its
/// digits in the international form when they say their country ("+33 1 99 00*",
/// "003319900*", or "019900*" read in France: "+3319900*"). None when empty.
pub fn number_key(number: &str, region: Option<&crate::phones::Region>) -> Option<String> {
    let number = number.trim();
    let number = after(number, TEL).unwrap_or(number);
    let Some((head, _)) = number.split_once('*') else {
        let key = crate::phones::key(number, region);
        return (!key.is_empty()).then_some(key);
    };
    let digits: String = head.chars().filter(char::is_ascii_digit).collect();
    let key = if head.trim_start().starts_with('+') {
        format!("+{digits}")
    } else if let Some(abroad) = digits.strip_prefix("00") {
        format!("+{abroad}")
    } else if let Some(national) = region.filter(|r| !r.trunk.is_empty()).and_then(|r| digits.strip_prefix(r.trunk).map(|n| (r, n))) {
        format!("+{}{}", national.0.calling, national.1)
    } else {
        digits
    };
    (!key.is_empty()).then(|| format!("{key}*"))
}

/// A category's name as categories are compared: trimmed, without case or
/// accents ("Amis", " amis " and "AMIS" are one), as contacts.rs compares them.
pub fn category_key(name: &str) -> String {
    crate::text::fold(name.trim()).into_iter().collect()
}

/// The senders you let in: they skip the screener.
pub type KnownSenders = SenderList;

/// An entry as a pattern: lower case; `@domain` and `domain` as `*@domain`.
/// None for what cannot be an address or a pattern.
pub fn normalize(entry: &str) -> Option<String> {
    let entry = entry.trim().to_ascii_lowercase();
    // A `:` is a category's, a card's or a number's line, never an address.
    if entry.is_empty() || entry.chars().any(|c| c.is_whitespace() || c == ',' || c == ';' || c == '<' || c == '>' || c == ':') {
        return None;
    }
    let pattern = match entry.strip_prefix('@') {
        Some(domain) => format!("*@{domain}"),
        None if !entry.contains('@') => format!("*@{entry}"),
        None => entry,
    };
    let (local, domain) = pattern.rsplit_once('@')?;
    (!local.is_empty() && !domain.is_empty() && !domain.contains('@')).then_some(pattern)
}

/// What one line of a list names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// An address, or a pattern of addresses (`normalize`).
    Address(String),
    /// A phone number's key (`phones::key`), or a prefix of keys ending with `*`.
    Number(String),
    /// A contact card, by its UID: all its addresses and numbers.
    Card(String),
    /// A category of your contacts, as written.
    Category(String),
}

impl Entry {
    /// What a line, or an entry typed, names: `category:…`, `contact:…`,
    /// a number (`tel:…`, or digits alone), else an address or a pattern.
    /// `region` reads numbers written without their country.
    pub fn read(text: &str, region: Option<&crate::phones::Region>) -> Option<Entry> {
        if let Some(name) = category_of(text) {
            return Some(Entry::Category(name.to_string()));
        }
        if let Some(uid) = card_of(text) {
            return Some(Entry::Card(uid.to_string()));
        }
        if let Some(number) = number_of(text) {
            return number_key(number, region).map(Entry::Number);
        }
        normalize(text).map(Entry::Address)
    }

    /// As a list's file writes it.
    pub fn line(&self) -> String {
        match self {
            Entry::Address(pattern) => pattern.clone(),
            Entry::Number(key) => format!("{TEL}{key}"),
            Entry::Card(uid) => format!("{CONTACT}{uid}"),
            Entry::Category(name) => format!("{CATEGORY}{name}"),
        }
    }

    /// As said to you: a domain without its "*@", a number as its key, a category by its name.
    pub fn shown(&self) -> String {
        match self {
            Entry::Address(pattern) => pattern.trim_start_matches("*@").to_string(),
            Entry::Number(key) => key.clone(),
            Entry::Card(uid) => uid.clone(),
            Entry::Category(name) => name.clone(),
        }
    }

    /// The same entry: categories compared without case or accents.
    pub fn same(&self, other: &Entry) -> bool {
        match (self, other) {
            (Entry::Category(a), Entry::Category(b)) => category_key(a) == category_key(b),
            (a, b) => a == b,
        }
    }
}

/// Whether `text` fits `pattern`, `*` standing for any run of characters.
fn fits(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < t.len() {
        if i < p.len() && p[i] != '*' && p[i] == t[j] {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|&c| c == '*')
}

/// How precise a pattern is: an address beats any pattern; between patterns,
/// the one with more written characters.
fn precision(pattern: &str) -> usize {
    let written = pattern.chars().filter(|&c| c != '*').count();
    if pattern.contains('*') { written } else { 10_000 + written }
}

impl SenderList {
    pub fn parse(text: &str) -> Self {
        let mut list = SenderList::default();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            if let Some(name) = category_of(line) {
                if !list.categories.iter().any(|c| category_key(c) == category_key(name)) {
                    list.categories.push(name.to_string());
                }
            } else if let Some(uid) = card_of(line) {
                if !list.cards.iter().any(|c| c == uid) {
                    list.cards.push(uid.to_string());
                }
            } else if let Some(number) = number_of(line) {
                if !list.numbers.iter().any(|n| n == number) {
                    list.numbers.push(number.to_string());
                }
            } else if let Some(pattern) = normalize(line)
                && !list.patterns.contains(&pattern)
            {
                list.patterns.push(pattern);
            }
        }
        list
    }

    /// The categories of your contacts it names, as written.
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// The numbers and prefixes it names, as written after `tel:`.
    pub fn numbers(&self) -> &[String] {
        &self.numbers
    }

    /// The contact cards it names, by their UID.
    pub fn cards(&self) -> &[String] {
        &self.cards
    }

    /// The first of `keys` (categories as compared, `category_key`) it names, as written.
    fn names_category(&self, keys: &[String]) -> Option<&str> {
        self.categories.iter().find(|c| keys.contains(&category_key(c))).map(String::as_str)
    }

    /// Whether it names this card (its UID).
    fn names_card(&self, uid: &str) -> bool {
        !uid.is_empty() && self.cards.iter().any(|c| c == uid)
    }

    /// Whether it holds this very address (not a pattern that fits it).
    fn holds(&self, address: &str) -> bool {
        self.patterns.iter().any(|p| !p.contains('*') && p == address)
    }

    /// Whether it holds this very number (`key`, as `number_key` reads it), not a prefix that fits it.
    fn holds_number(&self, key: &str, region: Option<&crate::phones::Region>) -> bool {
        self.numbers.iter().filter(|n| !n.contains('*')).any(|n| number_key(n, region).as_deref() == Some(key))
    }

    /// Its prefixes, keyed (`tel:+3319900*`).
    fn prefixes(&self, region: Option<&crate::phones::Region>) -> Vec<String> {
        self.numbers.iter().filter(|n| n.contains('*')).filter_map(|n| number_key(n, region)).collect()
    }

    /// How precisely the list names an address: its most precise entry that fits it.
    pub fn names(&self, address: &str) -> Option<usize> {
        let address = address.trim().to_ascii_lowercase();
        if address.is_empty() {
            return None;
        }
        self.patterns.iter().filter(|p| fits(p, &address)).map(|p| precision(p)).max()
    }

    pub fn knows(&self, card: &Card) -> bool {
        card.from_address.as_deref().is_some_and(|a| self.names(a).is_some())
    }

    /// Reads the file; the list is empty when it does not exist yet.
    pub fn load(path: &Path) -> SenderList {
        SenderList::parse(&std::fs::read_to_string(path).unwrap_or_default())
    }

    /// Every address and pattern: addresses, then patterns, each sorted.
    pub fn entries(&self) -> Vec<String> {
        let mut addresses: Vec<String> = self.patterns.iter().filter(|p| !p.contains('*')).cloned().collect();
        let mut patterns: Vec<String> = self.patterns.iter().filter(|p| p.contains('*')).cloned().collect();
        addresses.sort();
        patterns.sort();
        addresses.into_iter().chain(patterns).collect()
    }

    /// The file's lines but those `gone` names, written beside, then moved: a
    /// crash halfway never empties a list, which would let every blocked
    /// sender back in. Nothing is written when no line goes.
    fn rewrite(path: &Path, gone: impl Fn(&str) -> bool) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(fail(e)),
        };
        let kept: Vec<&str> = text.lines().filter(|l| !gone(l)).collect();
        if kept.len() == text.lines().count() {
            return Ok(());
        }
        let temporary = path.with_extension("txt.new");
        std::fs::write(&temporary, kept.join("\n") + "\n").and_then(|()| std::fs::rename(&temporary, path)).map_err(fail)
    }

    /// A line at the end of the file, the file made if needed.
    fn append(path: &Path, line: &str) -> Result<(), String> {
        use std::io::Write;
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).map_err(fail)?;
        writeln!(file, "{line}").map_err(fail)
    }

    /// Takes an address or a pattern out of the file, keeping the other lines
    /// and the comments; `@example.org` and `*@example.org` are the same entry.
    pub fn remove(path: &Path, entry: &str) -> Result<(), String> {
        let entry = normalize(entry);
        SenderList::rewrite(path, |l| normalize(l).is_some() && normalize(l) == entry)
    }

    /// Takes a category's line out of the file, the other lines and the comments kept.
    pub fn remove_category(path: &Path, name: &str) -> Result<(), String> {
        let key = category_key(name);
        SenderList::rewrite(path, |l| category_of(l).is_some_and(|c| category_key(c) == key))
    }

    /// Takes an entry out of the file, however its line writes it (a number
    /// with or without spaces, a category without case or accents), the other
    /// lines and the comments kept.
    pub fn remove_entry(path: &Path, entry: &Entry, region: Option<&crate::phones::Region>) -> Result<(), String> {
        SenderList::rewrite(path, |l| !l.trim().starts_with('#') && Entry::read(l, region).is_some_and(|e| e.same(entry)))
    }

    /// Adds an entry at the end of the file, as `Entry::line` writes it; nothing when it is already there.
    pub fn add_entry(path: &Path, entry: &Entry, region: Option<&crate::phones::Region>) -> Result<(), String> {
        if let Entry::Category(name) = entry {
            return SenderList::add_category(path, name);
        }
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.lines().filter(|l| !l.trim().starts_with('#')).any(|l| Entry::read(l, region).is_some_and(|e| e.same(entry))) {
            return Ok(());
        }
        SenderList::append(path, &entry.line())
    }

    /// Adds a category's line at the end of the file, as written; nothing when it is already there.
    pub fn add_category(path: &Path, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.contains(['\n', '\r']) {
            return Err(format!("{name}: not a category"));
        }
        if SenderList::load(path).names_category(&[category_key(name)]).is_some() {
            return Ok(());
        }
        SenderList::append(path, &format!("{CATEGORY}{name}"))
    }

    /// Adds an address or a pattern at the end of the file: lets a sender in, or
    /// marks them; nothing when it is already there.
    pub fn let_in(path: &Path, entry: &str) -> Result<(), String> {
        let pattern = normalize(entry).ok_or_else(|| format!("{}: not an address", entry.trim()))?;
        if SenderList::load(path).patterns.contains(&pattern) {
            return Ok(());
        }
        SenderList::append(path, &pattern)
    }
}

/// The four lists (docs/porch.md, "Who may reach you, and when"): where an
/// entry is written. Who someone is (`reach::Who`) adds strangers, who are on
/// none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Standing {
    /// Friends, chosen colleagues, chosen family: by default, at any time.
    Safe,
    /// By your word, where a broader entry would say otherwise; anyone in your
    /// address books is neutral without it.
    Neutral,
    /// Those you hear from only at chosen times: by default, in working hours.
    Restricted,
    /// Spam, harassment: never, on any channel.
    Blocked,
}

impl Standing {
    /// The four, in the order the lists are shown.
    pub const ALL: [Standing; 4] = [Standing::Safe, Standing::Neutral, Standing::Restricted, Standing::Blocked];

    pub fn read(text: &str) -> Option<Standing> {
        match text {
            "safe" => Some(Standing::Safe),
            "neutral" => Some(Standing::Neutral),
            "restricted" => Some(Standing::Restricted),
            "blocked" => Some(Standing::Blocked),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Standing::Safe => "safe",
            Standing::Neutral => "neutral",
            Standing::Restricted => "restricted",
            Standing::Blocked => "blocked",
        }
    }
}

/// What decided who someone is, the most precise first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum By {
    /// Their own address, on a list.
    Address,
    /// Their own number, on a list.
    Number,
    /// Their card, on a list (`contact:<UID>`): your choice for the person.
    Card,
    /// A category their card is in.
    Category,
    /// A pattern their address fits: a domain, `news*@`.
    Domain,
    /// A prefix their number starts with.
    Prefix,
    /// You let them in through the screener: neutral.
    LetIn,
    /// In your address books, on no list: neutral.
    Book,
    /// Nobody: a stranger.
    Default,
}

/// Who someone is, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Judged {
    pub who: crate::reach::Who,
    pub by: By,
    /// What decided: the address or the number, the card's name, the category
    /// as the list writes it, the pattern; "" when nothing did.
    pub name: String,
    /// Their card's name when they are in your address books, whatever decided; "" otherwise.
    pub card: String,
}

impl Judged {
    /// Nobody names them: a stranger.
    pub fn stranger() -> Judged {
        Judged { who: crate::reach::Who::Stranger, by: By::Default, name: String::new(), card: String::new() }
    }
}

/// A name as chat senders are matched to cards: its words, one space between
/// them, without case. Accents count: "Hélène" is not "Helene".
pub fn name_key(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// A card of your address books, as who may reach you needs it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookCard {
    pub uid: String,
    /// Its file (`contacts::Contact::key`).
    pub key: String,
    pub name: String,
    /// Its categories, as compared (`category_key`).
    pub categories: Vec<String>,
    /// Its addresses, lower case.
    pub addresses: Vec<String>,
}

/// Your address books, as who may reach you needs them (docs/porch.md, "Who
/// may reach you, and when"): who is in them, by address, number, name and
/// card, with each card's categories. Read from every address book, on a
/// server or on this device only, read-only ones too.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Book {
    cards: Vec<BookCard>,
    /// Address, lower case → its cards.
    by_address: std::collections::HashMap<String, Vec<usize>>,
    /// A number's key (`phones::key`) → its cards.
    by_number: std::collections::HashMap<String, Vec<usize>>,
    /// A name as compared (`name_key`) → its cards.
    by_name: std::collections::HashMap<String, Vec<usize>>,
    /// Every category in use, as compared → as first written.
    names: std::collections::BTreeMap<String, String>,
    /// The country numbers written without one were read as.
    region: Option<&'static crate::phones::Region>,
}

fn push_once(slot: &mut Vec<usize>, index: usize) {
    if !slot.contains(&index) {
        slot.push(index);
    }
}

impl Book {
    /// From contacts; `region` reads their numbers written without a country.
    pub fn of(contacts: &[crate::contacts::Contact], region: Option<&'static crate::phones::Region>) -> Book {
        let mut book = Book { region, ..Book::default() };
        for contact in contacts {
            let index = book.cards.len();
            let mut keys: Vec<String> = Vec::new();
            for name in contact.categories.iter().map(|c| c.trim()).filter(|c| !c.is_empty()) {
                let key = category_key(name);
                book.names.entry(key.clone()).or_insert_with(|| name.to_string());
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            let addresses: Vec<String> = contact
                .emails
                .iter()
                .map(|e| e.value.trim().strip_prefix("mailto:").unwrap_or(e.value.trim()).trim().to_ascii_lowercase())
                .filter(|a| !a.is_empty())
                .collect();
            for address in &addresses {
                push_once(book.by_address.entry(address.clone()).or_default(), index);
            }
            book.cards.push(BookCard { uid: contact.uid.trim().to_string(), key: contact.key.clone(), name: contact.name.trim().to_string(), categories: keys, addresses });
            for phone in &contact.phones {
                let key = crate::phones::key(&phone.value, region);
                if !key.is_empty() {
                    push_once(book.by_number.entry(key).or_default(), index);
                }
            }
            let name = name_key(&contact.name);
            if !name.is_empty() {
                push_once(book.by_name.entry(name).or_default(), index);
            }
        }
        book
    }

    /// The cards holding this address.
    pub fn cards_of_address(&self, address: &str) -> &[usize] {
        self.by_address.get(&address.trim().to_ascii_lowercase()).map_or(&[], Vec::as_slice)
    }

    /// The cards holding this number, by its key.
    pub fn cards_of_number(&self, key: &str) -> &[usize] {
        self.by_number.get(key).map_or(&[], Vec::as_slice)
    }

    /// The cards with this very name (`name_key`).
    pub fn cards_named(&self, name: &str) -> &[usize] {
        self.by_name.get(&name_key(name)).map_or(&[], Vec::as_slice)
    }

    /// A card, by its place.
    pub fn card(&self, index: usize) -> Option<&BookCard> {
        self.cards.get(index)
    }

    /// A card's place, by its UID or its file.
    pub fn find(&self, id: &str) -> Option<usize> {
        let id = id.trim();
        (!id.is_empty()).then(|| self.cards.iter().position(|c| c.uid == id || c.key == id)).flatten()
    }

    /// The categories of these cards, as compared, each once.
    pub fn categories_of(&self, cards: &[usize]) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        for key in cards.iter().filter_map(|i| self.cards.get(*i)).flat_map(|c| c.categories.iter()) {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        keys
    }

    /// The categories of the cards holding this address, as compared.
    pub fn of_address(&self, address: &str) -> Vec<String> {
        self.categories_of(self.cards_of_address(address))
    }

    /// Every category in use, as first written, in order.
    pub fn names(&self) -> Vec<String> {
        self.names.values().cloned().collect()
    }

    /// Every number on a card, by its key.
    pub fn numbers(&self) -> impl Iterator<Item = &str> {
        self.by_number.keys().map(String::as_str)
    }

    /// How many cards.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// The country numbers written without one were read as.
    pub fn region(&self) -> Option<&'static crate::phones::Region> {
        self.region
    }

    /// From every address book, kept while their files do not change: read
    /// again only when one is added, changed or taken away, or the country
    /// numbers are read in changes; never per message or per call.
    pub fn load(region: Option<&'static crate::phones::Region>) -> std::sync::Arc<Book> {
        static KEPT: std::sync::Mutex<Option<(u64, std::sync::Arc<Book>)>> = std::sync::Mutex::new(None);
        let books = crate::vdir::collections(crate::vdir::Kind::Contacts);
        // Each book's place, and each card's name, size and time; the country.
        let mut seal: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |bytes: &[u8]| {
            for b in bytes {
                seal = (seal ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        mix(region.map_or("", |r| r.code).as_bytes());
        for book in &books {
            mix(book.dir.to_string_lossy().as_bytes());
            for item in book.items() {
                mix(item.to_string_lossy().as_bytes());
                if let Ok(meta) = std::fs::metadata(&item) {
                    mix(&meta.len().to_le_bytes());
                    let time = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
                    mix(&time.to_le_bytes());
                }
            }
        }
        let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((known, index)) = kept.as_ref()
            && *known == seal
        {
            return std::sync::Arc::clone(index);
        }
        let index = std::sync::Arc::new(Book::of(&crate::contacts::all(), region));
        *kept = Some((seal, std::sync::Arc::clone(&index)));
        index
    }
}

/// The four lists of who may reach you: safe, neutral, restricted and
/// blocked, with your address books, their cards and categories, and the
/// senders you let in (docs/porch.md, "Who may reach you, and when"). The most
/// precise entry decides: the person's own address or number; then their
/// card (`contact:<UID>`); then its categories; then patterns (a domain, a
/// prefix), the more precise first; then, neutral: let in through the
/// screener, or in your address books; else a stranger. At one level, blocked
/// before restricted before neutral before safe. An address marked safe stays
/// safe in a domain you blocked; a friend you put on neutral stays neutral in
/// a category you marked safe. Lists judge the sender's address only, after
/// forged mail was set aside: a message whose sender is not verified never
/// changes them (porch.rs, trust.rs), and nobody is judged by the server or
/// the domain another sender shares.
#[derive(Debug, Clone, Default)]
pub struct Senders {
    pub safe: SenderList,
    pub neutral: SenderList,
    pub restricted: SenderList,
    pub blocked: SenderList,
    /// The senders you let in through the screener: neutral, never strangers.
    pub known: SenderList,
    /// Your address books: who is in them, their cards' categories.
    pub contacts: std::sync::Arc<Book>,
}

/// Each list's file, in the order ties are decided: blocked, restricted, neutral, safe.
fn list_paths(config: &crate::config::Config) -> [(Standing, PathBuf); 4] {
    [
        (Standing::Blocked, config.blocked_senders_path()),
        (Standing::Restricted, config.restricted_senders_path()),
        (Standing::Neutral, config.neutral_senders_path()),
        (Standing::Safe, config.safe_senders_path()),
    ]
}

impl Senders {
    /// The lists, the senders you let in, and every address book (kept while their files do not change).
    pub fn load(config: &crate::config::Config) -> Senders {
        Senders {
            safe: SenderList::load(&config.safe_senders_path()),
            neutral: SenderList::load(&config.neutral_senders_path()),
            restricted: SenderList::load(&config.restricted_senders_path()),
            blocked: SenderList::load(&config.blocked_senders_path()),
            known: SenderList::load(&config.known_senders_path()),
            contacts: Book::load(crate::reach::region(config)),
        }
    }

    /// The lists, in the order ties are decided.
    fn lists(&self) -> [(Standing, &SenderList); 4] {
        [(Standing::Blocked, &self.blocked), (Standing::Restricted, &self.restricted), (Standing::Neutral, &self.neutral), (Standing::Safe, &self.safe)]
    }

    /// The country numbers written without one are read as.
    pub fn region(&self) -> Option<&'static crate::phones::Region> {
        self.contacts.region()
    }

    /// The list that names this category, if any (as compared, `category_key`).
    pub fn category_standing(&self, name: &str) -> Option<Standing> {
        let key = [category_key(name)];
        self.lists().into_iter().find(|(_, list)| list.names_category(&key).is_some()).map(|(standing, _)| standing)
    }

    /// The list that names this card itself (`contact:<UID>`), if any.
    pub fn card_standing(&self, uid: &str) -> Option<Standing> {
        self.lists().into_iter().find(|(_, list)| list.names_card(uid.trim())).map(|(standing, _)| standing)
    }

    /// A card's own list: its `contact:<UID>` line; else the one list that
    /// holds every address of it, which is how "Their mail" chose for a
    /// person before cards were on the lists themselves.
    fn card_choice(&self, card: &BookCard) -> Option<Standing> {
        self.card_standing(&card.uid).or_else(|| {
            let first = card.addresses.first()?;
            let (standing, list) = self.lists().into_iter().find(|(_, list)| list.holds(first))?;
            card.addresses.iter().all(|a| list.holds(a)).then_some(standing)
        })
    }

    /// A person's own list, by their card's UID or file (`card_choice`): what their card's chooser shows.
    pub fn person_standing(&self, id: &str) -> Option<Standing> {
        self.card_choice(self.contacts.card(self.contacts.find(id)?)?)
    }

    /// The name of the first of these cards.
    fn card_name(&self, cards: &[usize]) -> String {
        cards.first().and_then(|i| self.contacts.card(*i)).map(|c| c.name.clone()).unwrap_or_default()
    }

    /// One of these cards on a list, the one whose list comes first in the order ties are decided.
    fn by_card(&self, cards: &[usize]) -> Option<Judged> {
        let chosen: Vec<(Standing, &BookCard)> = cards.iter().filter_map(|i| self.contacts.card(*i)).filter_map(|c| self.card_choice(c).map(|s| (s, c))).collect();
        let order = [Standing::Blocked, Standing::Restricted, Standing::Neutral, Standing::Safe];
        let (standing, card) = order.into_iter().find_map(|standing| chosen.iter().find(|(s, _)| *s == standing).copied())?;
        Some(Judged { who: crate::reach::Who::of(standing), by: By::Card, name: card.name.clone(), card: card.name.clone() })
    }

    /// A category of these cards on a list.
    fn by_category(&self, cards: &[usize]) -> Option<Judged> {
        let keys = self.contacts.categories_of(cards);
        if keys.is_empty() {
            return None;
        }
        let (standing, name) = self.lists().into_iter().find_map(|(standing, list)| list.names_category(&keys).map(|name| (standing, name.to_string())))?;
        Some(Judged { who: crate::reach::Who::of(standing), by: By::Category, name, card: self.card_name(cards) })
    }

    /// In your address books on no list: neutral, by their card's name.
    fn by_book(&self, cards: &[usize]) -> Option<Judged> {
        (!cards.is_empty()).then(|| {
            let name = self.card_name(cards);
            Judged { who: crate::reach::Who::Neutral, by: By::Book, name: name.clone(), card: name }
        })
    }

    /// Who an address is, and why: its own entry; else its card's; else the
    /// card's categories; else the most precise pattern; else neutral when
    /// you let it in or a card holds it; else a stranger. At one level,
    /// blocked before restricted before neutral before safe.
    pub fn judge(&self, address: &str) -> Judged {
        let address = address.trim().to_ascii_lowercase();
        if address.is_empty() {
            return Judged::stranger();
        }
        let cards = self.contacts.cards_of_address(&address);
        if let Some((standing, _)) = self.lists().into_iter().find(|(_, list)| list.holds(&address)) {
            return Judged { who: crate::reach::Who::of(standing), by: By::Address, name: address, card: self.card_name(cards) };
        }
        if let Some(judged) = self.by_card(cards).or_else(|| self.by_category(cards)) {
            return judged;
        }
        let pattern = self
            .lists()
            .into_iter()
            .flat_map(|(standing, list)| list.patterns.iter().filter(|p| p.contains('*') && fits(p, &address)).map(move |p| (precision(p), standing, p)))
            // The most precise; at equal precision, the first in the order ties are decided.
            .fold(None, |best: Option<(usize, Standing, &String)>, (p, standing, pattern)| match best {
                Some((b, _, _)) if b >= p => best,
                _ => Some((p, standing, pattern)),
            });
        if let Some((_, standing, pattern)) = pattern {
            return Judged { who: crate::reach::Who::of(standing), by: By::Domain, name: pattern.clone(), card: self.card_name(cards) };
        }
        if self.known.names(&address).is_some() {
            return Judged { who: crate::reach::Who::Neutral, by: By::LetIn, name: String::new(), card: self.card_name(cards) };
        }
        self.by_book(cards).unwrap_or_else(Judged::stranger)
    }

    /// Who an address is.
    pub fn who(&self, address: &str) -> crate::reach::Who {
        self.judge(address).who
    }

    /// Who a message's sender is; one without an address is a stranger.
    pub fn who_of(&self, card: &Card) -> crate::reach::Who {
        card.from_address.as_deref().map_or(crate::reach::Who::Stranger, |a| self.who(a))
    }

    /// Who a phone number is, and why: its own entry; else its card's; else
    /// the card's categories; else the longest prefix on a list; else neutral
    /// when a card holds it; else a stranger. A number Sioul cannot read, or
    /// none (a hidden number), is a stranger here: calls give hidden numbers
    /// their own row (`reach::Row::Hidden`).
    pub fn judge_number(&self, number: &str) -> Judged {
        let region = self.region();
        let Some(key) = number_key(number, region).filter(|k| !k.contains('*')) else {
            return Judged::stranger();
        };
        let cards = self.contacts.cards_of_number(&key);
        if let Some((standing, _)) = self.lists().into_iter().find(|(_, list)| list.holds_number(&key, region)) {
            return Judged { who: crate::reach::Who::of(standing), by: By::Number, name: key, card: self.card_name(cards) };
        }
        if let Some(judged) = self.by_card(cards).or_else(|| self.by_category(cards)) {
            return judged;
        }
        let prefix = self
            .lists()
            .into_iter()
            .flat_map(|(standing, list)| list.prefixes(region).into_iter().filter(|p| fits(p, &key)).map(move |p| (precision(&p), standing, p)))
            .fold(None, |best: Option<(usize, Standing, String)>, (p, standing, prefix)| match best {
                Some((b, _, _)) if b >= p => best,
                _ => Some((p, standing, prefix)),
            });
        if let Some((_, standing, prefix)) = prefix {
            return Judged { who: crate::reach::Who::of(standing), by: By::Prefix, name: prefix, card: self.card_name(cards) };
        }
        self.by_book(cards).unwrap_or_else(Judged::stranger)
    }

    /// Who a name is, for messages whose sender is known by name only (a chat
    /// app's notification): a card with this very name (`name_key`), its own
    /// entry, else its categories, else neutral; between two cards of one
    /// name, the one whose list comes first in the order ties are decided.
    /// None when no card has that name: the caller decides.
    pub fn judge_name(&self, name: &str) -> Option<Judged> {
        let cards = self.contacts.cards_named(name);
        if cards.is_empty() {
            return None;
        }
        self.by_card(cards).or_else(|| self.by_category(cards)).or_else(|| self.by_book(cards))
    }

    /// Who a card is, by its UID or its file: its own entry, else its
    /// categories, else neutral. None for a card no address book holds.
    pub fn judge_card(&self, id: &str) -> Option<Judged> {
        let index = [self.contacts.find(id)?];
        self.by_card(&index).or_else(|| self.by_category(&index)).or_else(|| self.by_book(&index))
    }

    /// Every whole number known, on a card or on a list, by its key, with who
    /// it is (`judge_number`); prefixes aside (`prefixes`). Strangers are not
    /// among them: a number nobody knows is one.
    pub fn numbers(&self) -> Vec<(String, crate::reach::Who)> {
        let region = self.region();
        let mut keys: std::collections::BTreeSet<String> = self.contacts.numbers().map(str::to_string).collect();
        for (_, list) in self.lists() {
            keys.extend(list.numbers.iter().filter(|n| !n.contains('*')).filter_map(|n| number_key(n, region)));
        }
        keys.into_iter().map(|key| {
            let who = self.judge_number(&key).who;
            (key, who)
        }).collect()
    }

    /// The prefixes on the lists (`tel:+3319900*`), each with the list it is on:
    /// for a number no card and no entry of its own names.
    pub fn prefixes(&self) -> Vec<(String, crate::reach::Who)> {
        let region = self.region();
        let mut out: Vec<(String, crate::reach::Who)> = Vec::new();
        for (standing, list) in self.lists() {
            for prefix in list.prefixes(region) {
                if !out.iter().any(|(p, _)| *p == prefix) {
                    out.push((prefix, crate::reach::Who::of(standing)));
                }
            }
        }
        out
    }
}

/// Puts an entry in one list and out of the three others: an address or a
/// pattern, a number (`tel:…`, or digits alone), a card (`contact:<UID>`), or
/// a category; your own choice for it, whatever its card, its categories or
/// its domain say.
pub fn set_standing(config: &crate::config::Config, entry: &str, standing: Standing) -> Result<(), String> {
    let region = crate::reach::region(config);
    let entry = Entry::read(entry, region).ok_or_else(|| format!("{}: not an address", entry.trim()))?;
    if let Entry::Category(name) = &entry {
        return set_category(config, name, Some(standing));
    }
    for (_, path) in &list_paths(config) {
        SenderList::remove_entry(path, &entry, region)?;
    }
    let path = list_paths(config).into_iter().find(|(s, _)| *s == standing).map(|(_, p)| p).unwrap_or_default();
    SenderList::add_entry(&path, &entry, region)
}

/// Takes an entry's own line out of every list: its card, its categories,
/// else its domain, decide ("As their categories say").
pub fn clear_standing(config: &crate::config::Config, entry: &str) -> Result<(), String> {
    let region = crate::reach::region(config);
    let entry = Entry::read(entry, region).ok_or_else(|| format!("{}: not an address", entry.trim()))?;
    if let Entry::Category(name) = &entry {
        return set_category(config, name, None);
    }
    for (_, path) in &list_paths(config) {
        SenderList::remove_entry(path, &entry, region)?;
    }
    Ok(())
}

/// Unblocks an entry: out of the blocked list; still blocked by its card, a
/// category or a broader pattern, it is written neutral, so that it is not.
pub fn unblock(config: &crate::config::Config, entry: &str) -> Result<(), String> {
    let region = crate::reach::region(config);
    let entry = Entry::read(entry, region).ok_or_else(|| format!("{}: not an address", entry.trim()))?;
    SenderList::remove_entry(&config.blocked_senders_path(), &entry, region)?;
    let senders = Senders::load(config);
    let blocked = crate::reach::Who::Blocked;
    let still = match &entry {
        Entry::Address(pattern) => senders.who(&pattern.replace('*', "x")) == blocked,
        Entry::Number(key) => senders.judge_number(&key.replace('*', "0")).who == blocked,
        Entry::Card(uid) => senders.judge_card(uid).is_some_and(|j| j.who == blocked),
        Entry::Category(_) => false,
    };
    if still {
        return set_standing(config, &entry.line(), Standing::Neutral);
    }
    Ok(())
}

/// Puts a category of your contacts on one list and out of the three others;
/// none: on no list, each person as their own entry or their domain says.
pub fn set_category(config: &crate::config::Config, name: &str, standing: Option<Standing>) -> Result<(), String> {
    for (_, path) in &list_paths(config) {
        SenderList::remove_category(path, name)?;
    }
    match standing.and_then(|s| list_paths(config).into_iter().find(|(l, _)| *l == s)) {
        Some((_, path)) => SenderList::add_category(&path, name),
        None => Ok(()),
    }
}

/// A person's state, for all their addresses and numbers (a contact card's
/// chooser): their card on one list (`contact:<UID>`), or on none
/// (`standing` none: their categories decide), and the own entries of their
/// addresses and numbers taken out, so that the card decides for all of
/// them. A card without a UID gets its addresses and numbers written one by
/// one instead.
pub fn set_person(config: &crate::config::Config, uid: &str, addresses: &[String], numbers: &[String], standing: Option<Standing>) -> Result<(), String> {
    let region = crate::reach::region(config);
    let entries: Vec<String> = addresses.iter().filter_map(|a| Entry::read(a.trim().strip_prefix("mailto:").unwrap_or(a.trim()), region)).filter(|e| matches!(e, Entry::Address(_))).chain(numbers.iter().filter_map(|n| number_key(n, region)).filter(|k| !k.contains('*')).map(Entry::Number)).map(|e| e.line()).collect();
    let card = uid.trim();
    if card.is_empty() {
        for entry in &entries {
            match standing {
                Some(standing) => set_standing(config, entry, standing)?,
                None => clear_standing(config, entry)?,
            }
        }
        return Ok(());
    }
    for entry in &entries {
        clear_standing(config, entry)?;
    }
    let line = format!("{CONTACT}{card}");
    match standing {
        Some(standing) => set_standing(config, &line, standing),
        None => clear_standing(config, &line),
    }
}

/// What the triage needs to know beyond the message.
pub struct Context<'a> {
    pub cases: Option<&'a CaseStore>,
    pub known: &'a SenderList,
    /// Who is safe, neutral or blocked; the blocked are set aside for good.
    pub senders: &'a Senders,
    /// The authserv-ids of the provider that delivered the message.
    pub trusted_ids: &'a [String],
    /// Now, as Unix seconds, to hide codes that have expired; none keeps every code.
    pub now: Option<i64>,
    /// The priority of the account the mail came through.
    pub priority: Priority,
    /// The domains of your own addresses, which a fake "your provider" borrows (`lookalike::own_domains`).
    pub own_domains: &'a [String],
    /// The address is public and shielded (`shield`).
    pub shielded: bool,
    /// What an AI made of shielded mail, by Message-ID, when you allowed it.
    pub assessments: Option<&'a std::collections::BTreeMap<String, crate::shield::Assessment>>,
    /// Words that make a sender automatic (filed); empty for the usual ones (`AUTOMATIC`).
    pub filed_words: &'a [String],
    /// Your own addresses, every account's: what you send yourself is let in.
    pub own_addresses: &'a [String],
}

impl Context<'_> {
    /// Past the screener: a sender you let in, or marked safe, or named for
    /// themselves (their address on a list, their card, its categories, your
    /// address books). A domain alone does not make a sender known: their
    /// first mail still waits in the screener, at the times its list says.
    fn knows(&self, card: &Card) -> bool {
        if self.known.knows(card) {
            return true;
        }
        let Some(address) = card.from_address.as_deref() else { return false };
        let judged = self.senders.judge(address);
        judged.who == crate::reach::Who::Safe || matches!(judged.by, By::Address | By::Card | By::Category | By::Book | By::LetIn)
    }

    /// Sent from one of your own addresses, and verified: a file sent from
    /// one device to another. A forged "from yourself" is a phishing trick,
    /// hence the proof asked.
    fn from_yourself(&self, card: &Card, trust: Trust) -> bool {
        trust == Trust::Verified && card.from_address.as_deref().is_some_and(|from| self.own_addresses.iter().any(|own| own.eq_ignore_ascii_case(from.trim())))
    }
}

/// Automatic senders, as Virtual Secretary's notification filter lists them.
pub const AUTOMATIC: &[&str] = &[
    "no-reply", "noreply", "no_reply", "donotreply", "do-not-reply", "notification",
    "ne-pas-repondre", "ne_pas_repondre", "nepasrepondre", "mailer-daemon", "postmaster",
];

/// Gives one message its lane and its reasons.
pub fn triage(card: Card, ctx: &Context) -> Triaged {
    let auth = trust::read_auth_results(&card.headers, ctx.trusted_ids);
    // Bulk headers never rule a code out: they only narrow where it is read (`codes::detect_message`).
    let detected = codes::detect_message(&card);
    // A proof counts for the domain of the address shown, never for another one.
    // A mailing list excuses a DMARC failure (it rewrites what it relays), but
    // no list relays your codes: a code failing DMARC is forged, list headers or not.
    let (trust, proof) = trust::judge_sender(auth.as_ref(), card.is_list && detected.is_none(), card.sender_domain());
    let code = detected.filter(|c| !expired(c, sent(&card), ctx.now));
    // A shielded address's mail is read before anything else is decided.
    let assessment = ctx.shielded.then(|| {
        let by_ai = crate::shield::ai_key(&card).and_then(|key| ctx.assessments?.get(&key)).cloned();
        by_ai.unwrap_or_else(|| crate::shield::assess(&card.subject, &card.excerpt))
    });
    let (lane, reason) = choose_lane(&card, ctx, trust, code.as_ref(), assessment.as_ref());
    let mut reasons = vec![Reason::Trust(proof), reason];
    // The warning goes with a code shown at once, not with one set aside or folded.
    if let Some(c) = &code
        && trust != Trust::Verified
        && lane == Lane::RightNow
    {
        reasons.push(Reason::UnverifiedCode(c.kind));
    }
    Triaged { card, lane, trust, code, reasons, priority: ctx.priority, checks: auth, assessment }
}

/// A code past its validity is no longer a code to show: its message goes to its lane.
fn expired(code: &OneTimeCode, sent: Option<i64>, now: Option<i64>) -> bool {
    let minutes = i64::from(code.lasts_minutes());
    matches!((sent, now), (Some(sent), Some(now)) if now - sent > minutes * 60)
}

/// When a message was sent, for its code's validity: its Date, never later than
/// it reached your provider (its file's time, `maildir::store`), so that a
/// message dated in the future, or not dated, cannot keep a code on top for good.
/// The phone's home screen card hides a code at the same time (sioul-app's `homecard`).
pub fn sent(card: &Card) -> Option<i64> {
    let arrived = card
        .path
        .as_ref()
        .and_then(|path| std::fs::metadata(path).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_secs()).ok());
    match (card.date, arrived) {
        (Some(date), Some(arrived)) => Some(date.min(arrived)),
        (date, arrived) => date.or(arrived),
    }
}

/// The lanes are decided in this order: set aside; for a shielded address,
/// hostile; codes; cases; what you sent yourself; a shielded address's own
/// lane; addresses ranked below; newsletters and automatic senders; the
/// screener; people you know.
fn choose_lane(card: &Card, ctx: &Context, trust: Trust, code: Option<&OneTimeCode>, assessment: Option<&crate::shield::Assessment>) -> (Lane, Reason) {
    set_aside(card, ctx, trust)
        .or_else(|| assessment.filter(|a| a.tone == crate::shield::Tone::Hostile).map(|_| (Lane::Hostile, Reason::Hostile)))
        .or_else(|| right_now(code))
        .or_else(|| in_case(card, ctx))
        .or_else(|| ctx.from_yourself(card, trust).then_some((Lane::People, Reason::FromYourself)))
        .or_else(|| public(card, ctx, assessment))
        .or_else(|| (ctx.priority == Priority::Below).then_some((Lane::Low, Reason::LowPriority)))
        .or_else(|| filed(card, ctx))
        .or_else(|| screener(card, ctx))
        .unwrap_or((Lane::People, Reason::KnownPerson))
}

/// A shielded address's mail from someone you have not let in: its own lane.
fn public(card: &Card, ctx: &Context, assessment: Option<&crate::shield::Assessment>) -> Option<(Lane, Reason)> {
    let assessment = assessment.filter(|_| !ctx.knows(card))?;
    Some((Lane::Public(card.account.clone().unwrap_or_default()), Reason::Public(assessment.topic)))
}

/// Blocked, forged, borrowing a brand's name, or spam: set aside, never deleted.
/// A sender you let in keeps the name they use.
fn set_aside(card: &Card, ctx: &Context, trust: Trust) -> Option<(Lane, Reason)> {
    if ctx.senders.who_of(card) == crate::reach::Who::Blocked {
        return Some((Lane::SetAside, Reason::Blocked));
    }
    if trust == Trust::Forged {
        return Some((Lane::SetAside, Reason::Forged));
    }
    // Your own mail, proven yours: never taken for spam or for a borrowed name.
    if ctx.from_yourself(card, trust) {
        return None;
    }
    let borrowed = (!ctx.knows(card)).then(|| lookalike::impersonation(card.from_name.as_deref(), card.sender_domain(), ctx.own_domains)).flatten();
    if let Some(fake) = borrowed {
        return Some((Lane::SetAside, Reason::Impersonation { brand: fake.brand, domain: fake.domain }));
    }
    let spam = trust::read_spam_verdict(&card.headers).filter(|s| s.flagged)?;
    Some((Lane::SetAside, Reason::Spam { source: spam.source, score: spam.score }))
}

/// What you just asked a site for, from its automatic address too: at once.
/// A forged one was set aside before; one not verified says so (`triage`).
fn right_now(code: Option<&OneTimeCode>) -> Option<(Lane, Reason)> {
    Some((Lane::RightNow, Reason::ExpiresSoon(code?.kind)))
}

fn in_case(card: &Card, ctx: &Context) -> Option<(Lane, Reason)> {
    let routing = ctx.cases?.route(card).into_iter().next()?;
    let id = routing.case.id.clone();
    Some((Lane::Case(id.clone()), Reason::Case { case_id: id, matched: routing.matched }))
}

fn filed(card: &Card, ctx: &Context) -> Option<(Lane, Reason)> {
    let address = card.from_address.as_deref().unwrap_or("");
    let automatic = if ctx.filed_words.is_empty() { AUTOMATIC.iter().any(|a| address.contains(a)) } else { ctx.filed_words.iter().any(|a| address.contains(a.as_str())) };
    let reason = if card.is_list { Reason::Newsletter } else { Reason::Automatic };
    (card.is_list || automatic).then_some((Lane::Filed, reason))
}

fn screener(card: &Card, ctx: &Context) -> Option<(Lane, Reason)> {
    (!ctx.knows(card)).then_some((Lane::Screener, Reason::FirstMessage))
}

/// Everything waiting in the Porch, from every source, minus what you closed it on.
/// Messages already done are skipped by their file name, without being read.
/// Mail from senders you blocked is left out: never shown, never counted.
pub fn gather(sources: &[Source], cases: Option<&CaseStore>, known: &SenderList, senders: &Senders, state: &PorchState, now: i64) -> Vec<Triaged> {
    let own = own_domains(sources);
    let own_addresses = own_addresses(sources);
    let own_domains = own.as_slice();
    let assessments = crate::shield::AiCache::load_all();
    let mut items: Vec<Triaged> = sources
        .iter()
        .flat_map(|src| {
            let account = src.account.as_deref();
            let ctx = Context { cases, known, senders, trusted_ids: &src.trusted_ids, now: Some(now), priority: src.priority, own_domains, shielded: src.shielded, assessments: Some(&assessments), filed_words: &src.filed_words, own_addresses: &own_addresses };
            // An account the Porch was never closed on shows its first window only:
            // all your mail is kept, the Porch is not an archive.
            let fresh = account.is_some_and(|a| !state.done.contains_key(a));
            let since = now - i64::from(crate::config::DEFAULT_SYNC_DAYS) * 86_400;
            maildir::read_messages_where(&src.folder, |origin| !state.is_done(account, origin))
                .into_iter()
                .filter(|card| !fresh || card.date.is_none_or(|d| d >= since))
                .filter(|card| senders.who_of(card) != crate::reach::Who::Blocked)
                .map(|mut card| {
                    card.account = src.account.clone();
                    triage(card, &ctx)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    follow_conversations(&mut items);
    items.sort_by_key(|t| t.card.date.unwrap_or(0));
    items
}

/// A message in the conversation of one of a case's goes to that case, unless
/// it was set aside, is a code or is hostile: those lanes come first.
fn follow_conversations(items: &mut [Triaged]) {
    let groups = crate::threads::group(&items.iter().map(|t| &t.card).collect::<Vec<_>>());
    let mut case_of: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    for (item, group) in items.iter().zip(&groups) {
        if let Lane::Case(id) = &item.lane {
            case_of.entry(*group).or_insert_with(|| id.clone());
        }
    }
    for (item, group) in items.iter_mut().zip(&groups) {
        let Some(id) = case_of.get(group) else { continue };
        if matches!(item.lane, Lane::People | Lane::Screener | Lane::Filed | Lane::Low | Lane::Public(_)) {
            item.lane = Lane::Case(id.clone());
            let reason = Reason::Case { case_id: id.clone(), matched: vec![crate::cases::RouteMatch { field: crate::cases::RouteField::Thread, value: String::new() }] };
            match item.reasons.iter_mut().find(|r| !matches!(r, Reason::Trust(_) | Reason::UnverifiedCode(_))) {
                Some(slot) => *slot = reason,
                None => item.reasons.push(reason),
            }
        }
    }
}

/// Judges files just fetched as the Porch would, each with its account's trusted ids.
pub fn judge(paths: &[PathBuf], sources: &[Source], cases: Option<&CaseStore>, known: &SenderList, senders: &Senders, now: i64) -> Vec<Triaged> {
    let own = own_domains(sources);
    let own_addresses = own_addresses(sources);
    let mut judged = paths
        .iter()
        .filter_map(|path| {
            let src = sources.iter().find(|src| path.starts_with(&src.folder))?;
            let mut card = maildir::read_one(path).filter(|card| senders.who_of(card) != crate::reach::Who::Blocked)?;
            card.account = src.account.clone();
            let ctx = Context { cases, known, senders, trusted_ids: &src.trusted_ids, now: Some(now), priority: src.priority, own_domains: &own, shielded: src.shielded, assessments: None, filed_words: &src.filed_words, own_addresses: &own_addresses };
            Some(triage(card, &ctx))
        })
        .collect::<Vec<_>>();
    follow_conversations(&mut judged);
    judged
}

/// The domains of the sources' own addresses (see `lookalike::own_domains`).
fn own_domains(sources: &[Source]) -> Vec<String> {
    lookalike::own_domains(sources.iter().filter_map(|s| s.address.as_deref()))
}

/// The sources' own addresses.
pub fn own_addresses(sources: &[Source]) -> Vec<String> {
    sources.iter().filter_map(|s| s.address.as_deref()).map(|a| a.trim().to_lowercase()).filter(|a| a.contains('@')).collect()
}

/// What came, as counts: the translator turns it into a few calm sentences.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub codes: usize,
    /// Each case with messages, in order of first appearance, and how many.
    pub cases: Vec<(String, usize)>,
    pub people: usize,
    pub screener: usize,
    pub filed: usize,
    pub set_aside: usize,
    /// From accounts ranked below the others: said once, at the end, and not in the total.
    pub low: usize,
}

/// Counts what came, lane by lane.
pub fn summarise(items: &[Triaged]) -> Summary {
    let count = |lane: Lane| items.iter().filter(|t| t.lane == lane).count();
    let mut cases: Vec<(String, usize)> = Vec::new();
    for t in items {
        if let Lane::Case(id) = &t.lane {
            match cases.iter_mut().find(|(c, _)| c == id) {
                Some((_, n)) => *n += 1,
                None => cases.push((id.clone(), 1)),
            }
        }
    }
    Summary {
        total: items.len() - count(Lane::Low),
        codes: count(Lane::RightNow),
        cases,
        people: count(Lane::People),
        screener: count(Lane::Screener),
        filed: count(Lane::Filed),
        set_aside: count(Lane::SetAside),
        low: count(Lane::Low),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triaged(from: &str, known: &SenderList, blocked: &SenderList) -> Triaged {
        let raw = format!("From: {from}\r\nSubject: Votre abonnement\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\nVotre abonnement a été renouvelé : 69,90 €.\r\n");
        let card = Card::from_bytes(raw.as_bytes()).unwrap();
        let senders = Senders { blocked: blocked.clone(), ..Senders::default() };
        triage(card, &Context { cases: None, known, senders: &senders, trusted_ids: &[], now: None, priority: Priority::Average, own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &[] })
    }

    #[test]
    fn who_may_write_and_when() {
        use crate::reach::Who;
        let senders = Senders {
            safe: SenderList::parse("jane@example.org\n*@family.example"),
            neutral: SenderList::parse("cousin@family.example"),
            blocked: SenderList::parse("@example.org\nnews*@shop.example"),
            ..Senders::default()
        };
        // The most precise entry decides.
        assert_eq!(senders.who("jane@example.org"), Who::Safe);
        assert_eq!(senders.who("JOHN@example.org"), Who::Blocked);
        assert_eq!(senders.who("aunt@family.example"), Who::Safe);
        assert_eq!(senders.who("cousin@family.example"), Who::Neutral);
        assert_eq!(senders.who("newsletter@shop.example"), Who::Blocked);
        // Nobody named, in no address book: strangers, no longer neutral.
        assert_eq!(senders.who("orders@shop.example"), Who::Stranger);
        assert_eq!(senders.judge("someone@elsewhere.example"), Judged::stranger());
        assert!(fits("*@*.example.org", "a@mail.example.org") && !fits("*@*.example.org", "a@example.org"));
        assert_eq!(normalize("@Example.org"), Some("*@example.org".into()));
        assert_eq!(normalize("example.org"), Some("*@example.org".into()));
        assert_eq!(normalize("two words@x"), None);
        // A safe sender skips the screener; a forged message from them is still set aside.
        let none = SenderList::default();
        let ctx = Context { cases: None, known: &none, senders: &senders, trusted_ids: &[], now: None, priority: Priority::Average, own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &[] };
        let card = |from: &str| Card::from_bytes(format!("From: {from}\r\nSubject: Hello\r\n\r\nHi.\r\n").as_bytes()).unwrap();
        assert_eq!(triage(card("Jane <jane@example.org>"), &ctx).lane, Lane::People);
        assert_eq!(triage(card("Someone <someone@elsewhere.example>"), &ctx).lane, Lane::Screener);
        // Their own entry makes them known; a domain alone does not.
        assert_eq!(triage(card("Cousin <cousin@family.example>"), &ctx).lane, Lane::People);
        let forged = Card::from_bytes(b"From: Jane <jane@example.org>\r\nAuthentication-Results: mx.example.net; dmarc=fail (p=reject) header.from=example.org\r\nSubject: Hello\r\n\r\nHi.\r\n").unwrap();
        let trusted = ["mx.example.net".to_string()];
        let strict = Context { trusted_ids: &trusted, ..ctx };
        assert_eq!(triage(forged, &strict).lane, Lane::SetAside);
    }

    fn contact(categories: &[&str], emails: &[&str]) -> crate::contacts::Contact {
        crate::contacts::Contact {
            categories: categories.iter().map(|c| c.to_string()).collect(),
            emails: emails.iter().map(|e| crate::contacts::Labeled { label: String::new(), value: e.to_string() }).collect(),
            ..crate::contacts::Contact::default()
        }
    }

    fn person(uid: &str, name: &str, emails: &[&str], phones: &[&str], categories: &[&str]) -> crate::contacts::Contact {
        crate::contacts::Contact {
            uid: uid.into(),
            key: format!("/books/{uid}.vcf"),
            name: name.into(),
            phones: phones.iter().map(|p| crate::contacts::Labeled { label: String::new(), value: p.to_string() }).collect(),
            ..contact(categories, emails)
        }
    }

    #[test]
    fn from_the_person_to_the_group() {
        use crate::reach::Who;
        // Friends and family on safe, a work domain restricted, one friend demoted to neutral.
        let contacts = [
            contact(&["Amis"], &["Lea@Example.org", "lea.home@mail.example"]),
            contact(&["amis"], &["paul@company.example"]),
            contact(&["Famille", "Pénibles"], &["oncle@family.example"]),
            contact(&["  AMIS "], &["sam@company.example"]),
            contact(&[], &["mailto:Nina@Example.net"]),
        ];
        let senders = Senders {
            safe: SenderList::parse("category:Amis\ncategory:Famille"),
            neutral: SenderList::parse("sam@company.example"),
            restricted: SenderList::parse("*@company.example"),
            blocked: SenderList::parse("category:penibles"),
            contacts: std::sync::Arc::new(Book::of(&contacts, None)),
            ..Senders::default()
        };
        // A category beats a domain: a friend at the company is safe; a colleague no category names is restricted.
        let judged = senders.judge("paul@company.example");
        assert_eq!((judged.who, judged.by, judged.name.as_str()), (Who::Safe, By::Category, "Amis"));
        assert_eq!(senders.judge("boss@company.example"), Judged { who: Who::Restricted, by: By::Domain, name: "*@company.example".into(), card: String::new() });
        // The person's own entry beats the category: one friend on neutral, the others safe.
        assert_eq!((senders.judge("Sam@Company.example").who, senders.judge("Sam@Company.example").by), (Who::Neutral, By::Address));
        // Every address of a card carries its categories, case aside.
        assert_eq!((senders.who("lea@example.org"), senders.who("LEA.HOME@mail.example")), (Who::Safe, Who::Safe));
        // In two categories on two lists, the blocked one wins; names compared without case or accents.
        assert_eq!((senders.judge("oncle@family.example").who, senders.judge("oncle@family.example").name.as_str()), (Who::Blocked, "penibles"));
        // In an address book, on no list: neutral. Nobody: a stranger.
        assert_eq!((senders.judge("nina@example.net").who, senders.judge("nina@example.net").by), (Who::Neutral, By::Book));
        assert_eq!(senders.judge("someone@elsewhere.example"), Judged::stranger());
        assert_eq!(senders.category_standing("AMIS"), Some(Standing::Safe));
        assert_eq!(senders.category_standing("Collègues"), None);
        assert_eq!(senders.contacts.names(), vec!["Amis".to_string(), "Famille".into(), "Pénibles".into()]);
        assert_eq!((category_key(" Équipe "), category_of("Category: Close friends"), category_of("jane@example.org")), ("equipe".to_string(), Some("Close friends"), None));
        assert_eq!(normalize("category:friends"), None, "never a pattern");
    }

    #[test]
    fn numbers_cards_and_names() {
        use crate::reach::Who;
        let france = crate::phones::region_named("FR");
        let contacts = [
            person("dr", "Cabinet du Dr Martin", &["accueil@cabinet.example"], &["01 99 00 00 01"], &["Soins"]),
            person("mum", "Maman", &["maman@example.org"], &["+33 4 65 71 12 34", "05 36 49 00 00"], &[]),
            person("boss", "Le patron", &["boss@company.example"], &["02 61 91 33 44"], &[]),
            person("pest", "Untel", &[], &["03 53 01 00 07"], &["Famille"]),
            person("twin", "Marie", &[], &["06 39 98 99 01"], &[]),
            person("twin2", "Marie", &[], &["06 39 98 99 02"], &[]),
        ];
        let senders = Senders {
            safe: SenderList::parse("contact:mum\ncategory:Soins\ncategory:Famille\ncontact:twin"),
            restricted: SenderList::parse("contact:boss\ntel:+33 3 53 01 00 07\ncontact:twin2"),
            blocked: SenderList::parse("tel:0899*\n02 61 91 33 44"),
            contacts: std::sync::Arc::new(Book::of(&contacts, france)),
            ..Senders::default()
        };
        // A card on a list holds for all its numbers and addresses, however the number is written.
        let mum = senders.judge_number("0465711234");
        assert_eq!((mum.who, mum.by, mum.name.as_str(), mum.card.as_str()), (Who::Safe, By::Card, "Maman", "Maman"));
        assert_eq!((senders.judge_number("+33536490000").who, senders.who("Maman@example.org")), (Who::Safe, Who::Safe));
        // A category of the card: the doctor's office rings as care does.
        assert_eq!((senders.judge_number("+33 1 99 00 00 01").who, senders.judge_number("+33 1 99 00 00 01").by), (Who::Safe, By::Category));
        // The number's own entry beats the card's: the boss's number is blocked, his address restricted.
        assert_eq!((senders.judge_number("02 61 91 33 44").who, senders.judge_number("02 61 91 33 44").by), (Who::Blocked, By::Number));
        assert_eq!(senders.who("boss@company.example"), Who::Restricted);
        // A number's own entry beats a safe category.
        assert_eq!(senders.judge_number("0353010007").who, Who::Restricted);
        // A prefix names every number it starts; a number on no card and no list is a stranger, and so is no number.
        let premium = senders.judge_number("08 99 12 34 56");
        assert_eq!((premium.who, premium.by, premium.name.as_str()), (Who::Blocked, By::Prefix, "+33899*"));
        assert_eq!(senders.judge_number("05 36 49 55 55"), Judged::stranger());
        assert_eq!(senders.judge_number(""), Judged::stranger());
        // Names, for chats: the very name, case and spaces aside; two cards of one name, the stricter.
        assert_eq!(senders.judge_name("  maman ").map(|j| j.who), Some(Who::Safe));
        assert_eq!(senders.judge_name("Marie").map(|j| j.who), Some(Who::Restricted));
        assert_eq!(senders.judge_name("Mama"), None, "no card has that name: the caller decides");
        assert_eq!(senders.judge_card("mum").map(|j| j.who), Some(Who::Safe));
        assert_eq!(senders.judge_card("/books/pest.vcf").map(|j| (j.who, j.by)), Some((Who::Safe, By::Category)), "by its file too");
        assert_eq!(senders.judge_card("nobody"), None);
        // Every number known, with who it is; the prefixes apart.
        let numbers = senders.numbers();
        assert!(numbers.contains(&("+33465711234".to_string(), Who::Safe)) && numbers.contains(&("+33261913344".to_string(), Who::Blocked)), "{numbers:?}");
        assert!(numbers.iter().all(|(_, who)| *who != Who::Stranger) && numbers.len() == 7, "{numbers:?}");
        assert_eq!(senders.prefixes(), vec![("+33899*".to_string(), Who::Blocked)]);
        // Lines: what each names.
        assert_eq!(Entry::read("tel:+33 4 65 71 12 34", france), Some(Entry::Number("+33465711234".into())));
        assert_eq!(Entry::read("04.65.71.12.34", france), Some(Entry::Number("+33465711234".into())));
        assert_eq!(Entry::read("0033 1 99 00*", france), Some(Entry::Number("+3319900*".into())));
        assert_eq!(Entry::read("Contact: ABC-1", france), Some(Entry::Card("ABC-1".into())));
        assert_eq!(Entry::read("@Example.org", france), Some(Entry::Address("*@example.org".into())));
        assert_eq!(Entry::Number("+33465711234".into()).line(), "tel:+33465711234");
        assert!(Entry::Category("Amis".into()).same(&Entry::Category(" AMIS".into())));
    }

    #[test]
    fn a_card_whose_addresses_share_a_list_stands_on_it() {
        use crate::reach::Who;
        let france = crate::phones::region_named("FR");
        // "Their mail" put every address of a card on a list, before cards were on the lists themselves.
        let contacts = [
            person("gran", "Mamie", &["mamie@example.org", "Mamie@Home.example"], &["02 61 91 22 33"], &["Voisins"]),
            person("half", "Paul", &["paul@example.org", "paul@work.example"], &["06 39 98 12 12"], &[]),
        ];
        let senders = Senders {
            safe: SenderList::parse("mamie@example.org\nmamie@home.example\npaul@example.org"),
            restricted: SenderList::parse("category:Voisins"),
            contacts: std::sync::Arc::new(Book::of(&contacts, france)),
            ..Senders::default()
        };
        // Her number follows her addresses' list, before her card's categories.
        assert_eq!((senders.judge_number("+33 2 61 91 22 33").who, senders.judge_number("+33 2 61 91 22 33").by), (Who::Safe, By::Card));
        assert_eq!((senders.person_standing("gran"), senders.card_standing("gran")), (Some(Standing::Safe), None));
        // One address of two on a list: no choice for the card; his number is as his address book says.
        assert_eq!((senders.judge_number("06 39 98 12 12").who, senders.judge_number("06 39 98 12 12").by), (Who::Neutral, By::Book));
        assert_eq!(senders.person_standing("half"), None);
    }

    #[test]
    fn numbers_and_cards_written() {
        use crate::reach::Who;
        let dir = std::env::temp_dir().join(format!("sioul-numbers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let blocked = dir.join("blocked-senders.txt");
        let safe = dir.join("safe-senders.txt");
        let france = crate::phones::region_named("FR");
        std::fs::write(&blocked, "# Mine.\ntel:04 65 71 12 34\njane@example.org\n").unwrap();
        // However the line writes the number, it is the one taken out; the rest stays.
        SenderList::remove_entry(&blocked, &Entry::Number("+33465711234".into()), france).unwrap();
        assert_eq!(std::fs::read_to_string(&blocked).unwrap(), "# Mine.\njane@example.org\n");
        SenderList::add_entry(&safe, &Entry::Card("mum".into()), france).unwrap();
        SenderList::add_entry(&safe, &Entry::Card("mum".into()), france).unwrap();
        SenderList::add_entry(&safe, &Entry::Number("+33465711234".into()), france).unwrap();
        assert_eq!(std::fs::read_to_string(&safe).unwrap(), "contact:mum\ntel:+33465711234\n");
        let list = SenderList::load(&safe);
        assert_eq!((list.cards(), list.numbers(), list.entries()), (&["mum".to_string()][..], &["+33465711234".to_string()][..], Vec::<String>::new()));
        let senders = Senders { safe: list, ..Senders::default() };
        assert_eq!(senders.judge_number("+33 4 65 71 12 34").who, Who::Safe);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn category_lines_kept_as_written() {
        let dir = std::env::temp_dir().join(format!("sioul-categories-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("safe-senders.txt");
        std::fs::write(&path, "# Mine.\ncategory:Close friends\njane@example.org\n").unwrap();
        let list = SenderList::load(&path);
        assert_eq!((list.categories(), list.entries()), (&["Close friends".to_string()][..], vec!["jane@example.org".to_string()]));
        // An address taken out keeps the category's line as written; one added after it.
        SenderList::remove(&path, "jane@example.org").unwrap();
        SenderList::add_category(&path, "Famille").unwrap();
        SenderList::add_category(&path, "famille").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Mine.\ncategory:Close friends\ncategory:Famille\n");
        SenderList::remove_category(&path, "CLOSE FRIENDS").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Mine.\ncategory:Famille\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn borrowed_names_and_blocked_senders_are_set_aside() {
        let none = SenderList::default();
        let fake = triaged("lnfos PrimeVlDEO <info@unrelated.example>", &none, &none);
        assert_eq!(fake.lane, Lane::SetAside);
        assert_eq!(fake.reasons.last(), Some(&Reason::Impersonation { brand: "Prime Video".into(), domain: "unrelated.example".into() }));
        // Blocked comes first, whatever the name.
        let blocked = SenderList::parse("@unrelated.example");
        assert_eq!(triaged("lnfos PrimeVlDEO <info@unrelated.example>", &none, &blocked).reasons.last(), Some(&Reason::Blocked));
        // A sender you let in keeps the name they use.
        let known = SenderList::parse("info@unrelated.example");
        assert_ne!(triaged("lnfos PrimeVlDEO <info@unrelated.example>", &known, &none).lane, Lane::SetAside);
        assert_eq!(blocked.entries(), vec!["*@unrelated.example".to_string()]);
    }

    #[test]
    fn an_account_ranked_below_waits_folded() {
        let none = SenderList::default();
        let raw = "From: Social <notify@social.example>\r\nSubject: Someone liked your post\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\nHello.\r\n";
        let low = Context { cases: None, known: &none, senders: &Senders::default(), trusted_ids: &[], now: None, priority: Priority::Below, own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &[] };
        let t = triage(Card::from_bytes(raw.as_bytes()).unwrap(), &low);
        assert_eq!((t.lane.clone(), t.reasons.last()), (Lane::Low, Some(&Reason::LowPriority)));
        let summary = summarise(&[t]);
        assert_eq!((summary.total, summary.low), (0, 1));
    }

    #[test]
    fn an_entry_taken_out_keeps_the_rest() {
        let path = std::env::temp_dir().join(format!("sioul-senders-{}.txt", std::process::id()));
        std::fs::write(&path, "# mine\njane@example.org\n@shop.example\n").unwrap();
        SenderList::remove(&path, "*@shop.example").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# mine\njane@example.org\n");
        SenderList::remove(&path, "nobody@example.org").unwrap();
        assert_eq!(SenderList::load(&path).entries(), vec!["jane@example.org".to_string()]);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_code_set_aside_is_not_offered() {
        let none = SenderList::default();
        let raw = "From: PayPal <service@unrelated.example>\r\nSubject: Your security code\r\n\r\nYour security code is 482913.\r\n";
        let ctx = Context { cases: None, known: &none, senders: &Senders::default(), trusted_ids: &[], now: None, priority: Priority::Average, own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &[] };
        let t = triage(Card::from_bytes(raw.as_bytes()).unwrap(), &ctx);
        assert_eq!(t.lane, Lane::SetAside);
        assert!(!t.reasons.iter().any(|r| matches!(r, Reason::UnverifiedCode(_))), "{:?}", t.reasons);
    }

    #[test]
    fn a_code_dated_in_the_future_still_expires() {
        let dir = std::env::temp_dir().join(format!("sioul-porch-code-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("code.eml");
        std::fs::write(&path, "From: Bank <codes@bank.example>\r\nSubject: Your verification code\r\nDate: Thu, 01 Oct 2099 10:00:00 +0200\r\n\r\nYour verification code: 482913\r\n").unwrap();
        // It reached the server two hours ago: a code lasts half an hour.
        let arrived = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 3600);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(arrived).unwrap();
        let now = i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()).unwrap();
        let none = SenderList::default();
        let ctx = Context { cases: None, known: &none, senders: &Senders::default(), trusted_ids: &[], now: Some(now), priority: Priority::Average, own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &[] };
        let t = triage(maildir::read_one(&path).unwrap(), &ctx);
        assert!(t.code.is_none() && t.lane != Lane::RightNow, "{:?}", t.lane);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

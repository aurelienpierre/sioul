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

/// A list of senders: one address or pattern per line, `#` for comments. A
/// pattern holds `*` for any run of characters: `*@example.org` (everyone
/// there), `*@*.example.org` (its subdomains), `news*@example.org`;
/// `@example.org` and a bare `example.org` stand for `*@example.org`. A line
/// `category:Friends` names everyone your contacts put in that category.
/// Five such lists exist: the senders you let in (the screener's), and who is
/// safe, neutral, restricted or blocked (`Senders`).
#[derive(Debug, Clone, Default)]
pub struct SenderList {
    /// Patterns, lower case, as `normalize` writes them.
    patterns: Vec<String>,
    /// Categories of your contacts, as written (`category:Friends`).
    categories: Vec<String>,
}

/// What starts a line naming a category of your contacts: `category:Friends`.
/// Never a pattern: a pattern has no `:`.
pub const CATEGORY: &str = "category:";

/// The category a line names ("category:Friends" → "Friends"), as written;
/// none for an address or a pattern.
pub fn category_of(line: &str) -> Option<&str> {
    let line = line.trim();
    let head = line.get(..CATEGORY.len())?;
    head.eq_ignore_ascii_case(CATEGORY).then(|| line[CATEGORY.len()..].trim()).filter(|name| !name.is_empty())
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
    // A `:` is a category's line (`category:Friends`), never an address.
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
        let mut patterns: Vec<String> = Vec::new();
        let mut categories: Vec<String> = Vec::new();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            if let Some(name) = category_of(line) {
                if !categories.iter().any(|c| category_key(c) == category_key(name)) {
                    categories.push(name.to_string());
                }
            } else if let Some(pattern) = normalize(line)
                && !patterns.contains(&pattern)
            {
                patterns.push(pattern);
            }
        }
        SenderList { patterns, categories }
    }

    /// The categories of your contacts it names, as written.
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// The first of `keys` (categories as compared, `category_key`) it names, as written.
    fn names_category(&self, keys: &[String]) -> Option<&str> {
        self.categories.iter().find(|c| keys.contains(&category_key(c))).map(String::as_str)
    }

    /// Whether it holds this very address (not a pattern that fits it).
    fn holds(&self, address: &str) -> bool {
        self.patterns.iter().any(|p| !p.contains('*') && p == address)
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

    /// Every entry: addresses, then patterns, each sorted.
    pub fn entries(&self) -> Vec<String> {
        let mut addresses: Vec<String> = self.patterns.iter().filter(|p| !p.contains('*')).cloned().collect();
        let mut patterns: Vec<String> = self.patterns.iter().filter(|p| p.contains('*')).cloned().collect();
        addresses.sort();
        patterns.sort();
        addresses.into_iter().chain(patterns).collect()
    }

    /// Takes an entry out of the file, keeping the other lines and the comments;
    /// `@example.org` and `*@example.org` are the same entry.
    pub fn remove(path: &Path, entry: &str) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(fail(e)),
        };
        let entry = normalize(entry);
        let kept: Vec<&str> = text.lines().filter(|l| normalize(l).is_none() || normalize(l) != entry).collect();
        if kept.len() == text.lines().count() {
            return Ok(());
        }
        // Written beside, then moved: a crash halfway never empties the list,
        // which would let every blocked sender back in.
        let temporary = path.with_extension("txt.new");
        std::fs::write(&temporary, kept.join("\n") + "\n").and_then(|()| std::fs::rename(&temporary, path)).map_err(fail)
    }

    /// Takes a category's line out of the file, the other lines and the comments kept.
    pub fn remove_category(path: &Path, name: &str) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(fail(e)),
        };
        let key = category_key(name);
        let kept: Vec<&str> = text.lines().filter(|l| category_of(l).is_none_or(|c| category_key(c) != key)).collect();
        if kept.len() == text.lines().count() {
            return Ok(());
        }
        let temporary = path.with_extension("txt.new");
        std::fs::write(&temporary, kept.join("\n") + "\n").and_then(|()| std::fs::rename(&temporary, path)).map_err(fail)
    }

    /// Adds a category's line at the end of the file, as written; nothing when it is already there.
    pub fn add_category(path: &Path, name: &str) -> Result<(), String> {
        use std::io::Write;
        let name = name.trim();
        if name.is_empty() || name.contains(['\n', '\r']) {
            return Err(format!("{name}: not a category"));
        }
        if SenderList::load(path).names_category(&[category_key(name)]).is_some() {
            return Ok(());
        }
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).map_err(fail)?;
        writeln!(file, "{CATEGORY}{name}").map_err(fail)
    }

    /// Adds an address or a pattern at the end of the file: lets a sender in, or
    /// marks them; nothing when it is already there.
    pub fn let_in(path: &Path, entry: &str) -> Result<(), String> {
        use std::io::Write;
        let pattern = normalize(entry).ok_or_else(|| format!("{}: not an address", entry.trim()))?;
        if SenderList::load(path).patterns.contains(&pattern) {
            return Ok(());
        }
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).map_err(fail)?;
        writeln!(file, "{pattern}").map_err(fail)
    }
}

/// Who may reach you (docs/porch.md, "Who may write to you"); when, the
/// matrix of `quiet::Reach` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Standing {
    /// Friends, chosen colleagues, chosen family: by default, at any time.
    Safe,
    /// Everyone no list names, strangers included: by default, in working and admin hours.
    Neutral,
    /// Those you hear from only at chosen times: by default, in working hours.
    Restricted,
    /// Spam, harassment: set aside for good, never shown.
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

/// What decided where a sender stands, the most precise first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum By {
    /// Their own address, on a list.
    Address,
    /// A category their contact is in.
    Category,
    /// A pattern their address fits: a domain, `news*@`.
    Domain,
    /// No list names them: neutral.
    Default,
}

/// Where a sender stands, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Judged {
    pub standing: Standing,
    pub by: By,
    /// What decided: the address, the category as the list writes it, the pattern; "" by default.
    pub name: String,
}

/// Your contacts' categories, by address (docs/porch.md, "Who may write to
/// you"): every address of a card carries its categories. Read from every
/// address book, on a server or on this computer only, read-only ones too.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContactCategories {
    /// Address, lower case → its categories as compared (`category_key`).
    by_address: std::collections::HashMap<String, Vec<String>>,
    /// Every category in use, as compared → as first written.
    names: std::collections::BTreeMap<String, String>,
}

impl ContactCategories {
    pub fn of(contacts: &[crate::contacts::Contact]) -> ContactCategories {
        let mut index = ContactCategories::default();
        for contact in contacts {
            let mut keys: Vec<String> = Vec::new();
            for name in contact.categories.iter().map(|c| c.trim()).filter(|c| !c.is_empty()) {
                let key = category_key(name);
                index.names.entry(key.clone()).or_insert_with(|| name.to_string());
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            if keys.is_empty() {
                continue;
            }
            for email in &contact.emails {
                let address = email.value.trim();
                let address = address.strip_prefix("mailto:").unwrap_or(address).trim().to_ascii_lowercase();
                if address.is_empty() {
                    continue;
                }
                let slot = index.by_address.entry(address).or_default();
                for key in &keys {
                    if !slot.contains(key) {
                        slot.push(key.clone());
                    }
                }
            }
        }
        index
    }

    /// The categories of the contacts holding this address, as compared.
    pub fn of_address(&self, address: &str) -> &[String] {
        self.by_address.get(&address.trim().to_ascii_lowercase()).map_or(&[], Vec::as_slice)
    }

    /// Every category in use, as first written, in order.
    pub fn names(&self) -> Vec<String> {
        self.names.values().cloned().collect()
    }

    /// From every address book, kept while their files do not change: read
    /// again only when one is added, changed or taken away, never per message.
    pub fn load() -> std::sync::Arc<ContactCategories> {
        static KEPT: std::sync::Mutex<Option<(u64, std::sync::Arc<ContactCategories>)>> = std::sync::Mutex::new(None);
        let books = crate::vdir::collections(crate::vdir::Kind::Contacts);
        // Each book's place, and each card's name, size and time.
        let mut seal: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |bytes: &[u8]| {
            for b in bytes {
                seal = (seal ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
            }
        };
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
        let index = std::sync::Arc::new(ContactCategories::of(&crate::contacts::all()));
        *kept = Some((seal, std::sync::Arc::clone(&index)));
        index
    }
}

/// The four lists of who may reach you: safe, neutral (the default for anyone
/// they do not name), restricted and blocked, with your contacts' categories.
/// The most precise entry decides: the person's own address, then their
/// contact's categories, then patterns (a domain), the more precise first;
/// at one level, blocked before restricted before neutral before safe. An
/// address marked safe stays safe in a domain you blocked; a friend you put
/// on neutral stays neutral in a category you marked safe. Lists judge the
/// sender's address only, after forged mail was set aside: a message whose
/// sender is not verified never changes them (porch.rs, trust.rs), and
/// nobody is judged by the server or the domain another sender shares.
#[derive(Debug, Clone, Default)]
pub struct Senders {
    pub safe: SenderList,
    pub neutral: SenderList,
    pub restricted: SenderList,
    pub blocked: SenderList,
    /// Your contacts' categories, by address.
    pub contacts: std::sync::Arc<ContactCategories>,
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
    pub fn load(config: &crate::config::Config) -> Senders {
        Senders {
            safe: SenderList::load(&config.safe_senders_path()),
            neutral: SenderList::load(&config.neutral_senders_path()),
            restricted: SenderList::load(&config.restricted_senders_path()),
            blocked: SenderList::load(&config.blocked_senders_path()),
            contacts: ContactCategories::load(),
        }
    }

    /// The lists, in the order ties are decided.
    fn lists(&self) -> [(Standing, &SenderList); 4] {
        [(Standing::Blocked, &self.blocked), (Standing::Restricted, &self.restricted), (Standing::Neutral, &self.neutral), (Standing::Safe, &self.safe)]
    }

    /// The list that names this category, if any (as compared, `category_key`).
    pub fn category_standing(&self, name: &str) -> Option<Standing> {
        let key = [category_key(name)];
        self.lists().into_iter().find(|(_, list)| list.names_category(&key).is_some()).map(|(standing, _)| standing)
    }

    /// Where an address stands, and why: its own entry; else its contact's
    /// categories; else the most precise pattern; at one level, blocked
    /// before restricted before neutral before safe; neutral when none names it.
    pub fn judge(&self, address: &str) -> Judged {
        let address = address.trim().to_ascii_lowercase();
        if address.is_empty() {
            return Judged { standing: Standing::Neutral, by: By::Default, name: String::new() };
        }
        if let Some((standing, _)) = self.lists().into_iter().find(|(_, list)| list.holds(&address)) {
            return Judged { standing, by: By::Address, name: address };
        }
        let keys = self.contacts.of_address(&address);
        if !keys.is_empty()
            && let Some((standing, name)) = self.lists().into_iter().find_map(|(standing, list)| list.names_category(keys).map(|name| (standing, name.to_string())))
        {
            return Judged { standing, by: By::Category, name };
        }
        self.lists()
            .into_iter()
            .flat_map(|(standing, list)| list.patterns.iter().filter(|p| p.contains('*') && fits(p, &address)).map(move |p| (precision(p), standing, p)))
            // The most precise; at equal precision, the first in the order ties are decided.
            .fold(None, |best: Option<(usize, Standing, &String)>, (p, standing, pattern)| match best {
                Some((b, _, _)) if b >= p => best,
                _ => Some((p, standing, pattern)),
            })
            .map_or(Judged { standing: Standing::Neutral, by: By::Default, name: String::new() }, |(_, standing, pattern)| Judged { standing, by: By::Domain, name: pattern.clone() })
    }

    /// Where an address stands.
    pub fn standing(&self, address: &str) -> Standing {
        self.judge(address).standing
    }

    pub fn standing_of(&self, card: &Card) -> Standing {
        card.from_address.as_deref().map_or(Standing::Neutral, |a| self.standing(a))
    }
}

/// Puts an address or a pattern in one list and out of the three others:
/// your own choice for it, whatever its categories or its domain say.
pub fn set_standing(config: &crate::config::Config, entry: &str, standing: Standing) -> Result<(), String> {
    let pattern = normalize(entry).ok_or_else(|| format!("{}: not an address", entry.trim()))?;
    for (_, path) in &list_paths(config) {
        SenderList::remove(path, &pattern)?;
    }
    let path = list_paths(config).into_iter().find(|(s, _)| *s == standing).map(|(_, p)| p).unwrap_or_default();
    SenderList::let_in(&path, &pattern)
}

/// Takes an address's or a pattern's own entry out of every list: its
/// contact's categories, else its domain, decide ("As their categories say").
pub fn clear_standing(config: &crate::config::Config, entry: &str) -> Result<(), String> {
    let pattern = normalize(entry).ok_or_else(|| format!("{}: not an address", entry.trim()))?;
    for (_, path) in &list_paths(config) {
        SenderList::remove(path, &pattern)?;
    }
    Ok(())
}

/// Unblocks an address or a pattern: out of the blocked list; still blocked
/// by a category or a broader pattern, it is written neutral, so that it is not.
pub fn unblock(config: &crate::config::Config, entry: &str) -> Result<(), String> {
    let pattern = normalize(entry).ok_or_else(|| format!("{}: not an address", entry.trim()))?;
    SenderList::remove(&config.blocked_senders_path(), &pattern)?;
    if Senders::load(config).standing(&pattern.replace('*', "x")) == Standing::Blocked {
        return set_standing(config, &pattern, Standing::Neutral);
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
    /// A sender you let in, or marked safe: past the screener.
    fn knows(&self, card: &Card) -> bool {
        self.known.knows(card) || self.senders.standing_of(card) == Standing::Safe
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
    if ctx.senders.standing_of(card) == Standing::Blocked {
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
                .filter(|card| senders.standing_of(card) != Standing::Blocked)
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
            let mut card = maildir::read_one(path).filter(|card| senders.standing_of(card) != Standing::Blocked)?;
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
        let senders = Senders {
            safe: SenderList::parse("jane@example.org\n*@family.example"),
            neutral: SenderList::parse("cousin@family.example"),
            blocked: SenderList::parse("@example.org\nnews*@shop.example"),
            ..Senders::default()
        };
        // The most precise entry decides.
        assert_eq!(senders.standing("jane@example.org"), Standing::Safe);
        assert_eq!(senders.standing("JOHN@example.org"), Standing::Blocked);
        assert_eq!(senders.standing("aunt@family.example"), Standing::Safe);
        assert_eq!(senders.standing("cousin@family.example"), Standing::Neutral);
        assert_eq!(senders.standing("newsletter@shop.example"), Standing::Blocked);
        assert_eq!(senders.standing("orders@shop.example"), Standing::Neutral);
        // Nobody named: neutral.
        assert_eq!(senders.standing("someone@elsewhere.example"), Standing::Neutral);
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

    #[test]
    fn from_the_person_to_the_group() {
        // Friends and family on safe, a work domain restricted, one friend demoted to neutral.
        let contacts = [
            contact(&["Amis"], &["Lea@Example.org", "lea.home@mail.example"]),
            contact(&["amis"], &["paul@company.example"]),
            contact(&["Famille", "Pénibles"], &["oncle@family.example"]),
            contact(&["  AMIS "], &["sam@company.example"]),
        ];
        let senders = Senders {
            safe: SenderList::parse("category:Amis\ncategory:Famille"),
            neutral: SenderList::parse("sam@company.example"),
            restricted: SenderList::parse("*@company.example"),
            blocked: SenderList::parse("category:penibles"),
            contacts: std::sync::Arc::new(ContactCategories::of(&contacts)),
        };
        // A category beats a domain: a friend at the company is safe; a colleague no category names is restricted.
        let judged = senders.judge("paul@company.example");
        assert_eq!((judged.standing, judged.by, judged.name.as_str()), (Standing::Safe, By::Category, "Amis"));
        assert_eq!(senders.judge("boss@company.example"), Judged { standing: Standing::Restricted, by: By::Domain, name: "*@company.example".into() });
        // The person's own entry beats the category: one friend on neutral, the others safe.
        assert_eq!(senders.judge("Sam@Company.example"), Judged { standing: Standing::Neutral, by: By::Address, name: "sam@company.example".into() });
        // Every address of a card carries its categories, case aside.
        assert_eq!((senders.standing("lea@example.org"), senders.standing("LEA.HOME@mail.example")), (Standing::Safe, Standing::Safe));
        // In two categories on two lists, the blocked one wins; names compared without case or accents.
        assert_eq!(senders.judge("oncle@family.example"), Judged { standing: Standing::Blocked, by: By::Category, name: "penibles".into() });
        // Nobody names them: neutral.
        assert_eq!(senders.judge("someone@elsewhere.example"), Judged { standing: Standing::Neutral, by: By::Default, name: String::new() });
        assert_eq!(senders.category_standing("AMIS"), Some(Standing::Safe));
        assert_eq!(senders.category_standing("Collègues"), None);
        assert_eq!(senders.contacts.names(), vec!["Amis".to_string(), "Famille".into(), "Pénibles".into()]);
        assert_eq!((category_key(" Équipe "), category_of("Category: Close friends"), category_of("jane@example.org")), ("equipe".to_string(), Some("Close friends"), None));
        assert_eq!(normalize("category:friends"), None, "never a pattern");
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

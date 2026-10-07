// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Searching the mail by conditions (docs/client.md, "Searching").
//!
//! The conditions are the mail filters' own (`rules::Condition`: a field, a
//! test, a value; all of them or any, `rules::Join`): a search can become a
//! filter as it is, and what a filter would take, a search finds. The search
//! adds four fields of its own to them (`rules::Field::SEARCH`): anywhere,
//! the address, the folder, a mark (read, flagged, answered).
//!
//! Here:
//! - how a stored message is matched ([`Stored`]): its headers read first,
//!   alone, and the whole message only when a condition needs it (its text,
//!   its attachments); a message seen on its server only ([`Found`]) is
//!   matched on its headers and its parts (BODYSTRUCTURE), its text being
//!   the server's to match;
//! - which folders a search reaches ([`Search::reaches`]): the junk and the
//!   trash only when a folder condition names them;
//! - the same search as IMAP's SEARCH (RFC 9051 §6.4.4; [`imap_key`]), for
//!   the mail a server holds and this device does not;
//! - the sentence that says what is searched ([`sentence`]): "Mail from
//!   …@bank.example…, arrived after 3 June, with an attachment.";
//! - the key a message seen on its server only goes by until it is brought
//!   here ([`ServerRef`], an IMAP URL, RFC 5092).

use crate::card::{Card, ImapOrigin};
use crate::folders::{Folder, Role};
use crate::i18n::{self, Translator};
use crate::porch::Senders;
use crate::rules::{self, Condition, Facts, Field, Join, Message, Place, Test};
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A search: the filters' conditions, all of them (the default) or any of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Search {
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default, rename = "match")]
    pub join: Join,
}

/// Whether a condition reads the whole message (its text, its attachments),
/// not its headers alone.
fn whole(field: Field) -> bool {
    matches!(field, Field::Body | Field::Attachment | Field::AttachmentType | Field::Anywhere)
}

/// Whether a condition is about where a message is (its address, its folder), not the message.
fn scope(field: Field) -> bool {
    matches!(field, Field::Account | Field::Folder)
}

impl Search {
    /// The conditions that say something (`Condition::problem`), those
    /// read from the headers first.
    pub fn effective(&self) -> Vec<&Condition> {
        let mut kept: Vec<&Condition> = self.conditions.iter().filter(|c| c.field != Field::Unknown && c.problem().is_none()).collect();
        kept.sort_by_key(|c| whole(c.field));
        kept
    }

    /// Nothing to look for yet.
    pub fn is_empty(&self) -> bool {
        self.effective().is_empty()
    }

    /// Whether a condition asks who the sender is to you (`Facts::who`).
    pub fn needs_senders(&self) -> bool {
        self.effective().iter().any(|c| c.field == Field::Sender)
    }

    /// The junk and the trash are searched only when a folder condition names them.
    pub fn wants(&self, role: Role) -> bool {
        self.effective().iter().any(|c| c.field == Field::Folder && c.test == Test::Is && rules::folder_named(&c.value, role, ""))
    }

    /// Whether a folder of an account can hold anything this search finds:
    /// the junk and the trash only when named; with all the conditions,
    /// those on the address and the folder must hold.
    pub fn reaches(&self, account: &str, folder: &Folder) -> bool {
        if matches!(folder.role, Role::Junk | Role::Trash) && !self.wants(folder.role) {
            return false;
        }
        let conditions = self.effective();
        if conditions.is_empty() {
            return false;
        }
        let places: Vec<bool> = conditions.iter().filter(|c| scope(c.field)).map(|c| place_holds(c, account, folder)).collect();
        match self.join {
            // Any: a folder named is wholly in; else the message's own conditions decide.
            Join::Any => places.iter().any(|&b| b) || conditions.iter().any(|c| !scope(c.field)),
            Join::All => places.iter().all(|&b| b),
        }
    }

    /// Whether a message holds, its conditions asked one by one: `ask`
    /// answers on its headers (`false`) or the whole of it (`true`); none
    /// when it cannot be known here (the text of a message seen on its
    /// server only: the server matched it). The headers are asked first.
    fn decide(&self, ask: &mut dyn FnMut(&Condition, bool) -> Option<bool>) -> bool {
        let conditions = self.effective();
        if conditions.is_empty() {
            return false;
        }
        let mut answer = |condition: &Condition| -> Option<bool> {
            if !whole(condition.field) {
                return ask(condition, false);
            }
            if condition.field == Field::Anywhere {
                // Found in the headers: the text need not be read.
                let positive = Condition { test: Test::Contains, ..condition.clone() };
                if ask(&positive, false) == Some(true) {
                    return Some(condition.test == Test::Contains);
                }
            }
            ask(condition, true)
        };
        match self.join {
            Join::All => conditions.iter().all(|c| answer(c) != Some(false)),
            Join::Any => {
                let mut unknown = false;
                for condition in conditions {
                    match answer(condition) {
                        Some(true) => return true,
                        Some(false) => {}
                        None => unknown = true,
                    }
                }
                unknown
            }
        }
    }
}

/// An address or folder condition, for a folder of an account.
fn place_holds(condition: &Condition, account: &str, folder: &Folder) -> bool {
    let yes = match condition.field {
        Field::Account => condition.value.trim() == account,
        _ => rules::folder_named(&condition.value, folder.role, &folder.name),
    };
    yes != (condition.test == Test::IsNot)
}

/// The headers of a message file: what comes before the first empty line,
/// read a little at a time, at most 256 KB.
pub fn read_header(path: &Path) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut bytes: Vec<u8> = Vec::with_capacity(16 * 1024);
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut chunk).ok()?;
        let searched_from = bytes.len().saturating_sub(3);
        bytes.extend_from_slice(&chunk[..read]);
        let tail = &bytes[searched_from..];
        let end = tail.windows(4).position(|w| w == b"\r\n\r\n").map(|p| searched_from + p + 4).or_else(|| tail.windows(2).position(|w| w == b"\n\n").map(|p| searched_from + p + 2));
        if let Some(end) = end {
            bytes.truncate(end);
            return Some(bytes);
        }
        if read == 0 || bytes.len() >= 256 * 1024 {
            return Some(bytes);
        }
    }
}

/// A Message-ID without its angle brackets.
fn bare(id: &str) -> String {
    id.trim().trim_matches(['<', '>']).to_string()
}

/// A message stored here, read as little as its conditions need: its
/// headers for most, the whole of it for its text and attachments.
pub struct Stored {
    pub path: PathBuf,
    pub account: String,
    pub folder: Folder,
    flags: String,
    header: OnceCell<Option<Card>>,
    whole: OnceCell<Option<Card>>,
}

impl Stored {
    pub fn new(path: PathBuf, account: &str, folder: &Folder) -> Stored {
        let flags = crate::maildir::flags_of(&path);
        Stored { path, account: account.to_string(), folder: folder.clone(), flags, header: OnceCell::new(), whole: OnceCell::new() }
    }

    fn header_card(&self) -> Option<&Card> {
        self.header
            .get_or_init(|| {
                let mut card = Card::from_bytes(&read_header(&self.path)?)?;
                card.path = Some(self.path.clone());
                card.origin = crate::maildir::origin_of(&self.path);
                Some(card)
            })
            .as_ref()
    }

    fn whole_card(&self) -> Option<&Card> {
        self.whole.get_or_init(|| crate::maildir::read_one(&self.path)).as_ref()
    }

    /// When it was written (its Date header), for the newest first.
    pub fn date(&self) -> Option<i64> {
        self.header_card().and_then(|c| c.date)
    }

    pub fn message_id(&self) -> Option<String> {
        self.header_card().and_then(|c| c.message_id.as_deref()).map(bare)
    }

    fn place(&self) -> Place {
        Place { account: self.account.clone(), folder: self.folder.name.clone(), role: self.folder.role, flags: self.flags.clone() }
    }

    /// Whether it holds for `search`; `senders` judge who its sender is to
    /// you, when a condition asks it.
    pub fn matches(&self, search: &Search, senders: Option<&Senders>) -> bool {
        let Some(card) = self.header_card() else { return false };
        let mut facts = Facts::of(card);
        facts.who = senders.map(|s| s.who_of(card));
        let header = Message::new(card, facts.clone()).placed(self.place());
        let whole: OnceCell<Option<Message>> = OnceCell::new();
        search.decide(&mut |condition, read_whole| {
            if !read_whole {
                return Some(condition.matches(&header));
            }
            let message = whole.get_or_init(|| self.whole_card().map(|card| Message::new(card, facts.clone()).placed(self.place())));
            Some(message.as_ref().is_some_and(|m| condition.matches(m)))
        })
    }
}

/// A message seen on its server only: its headers, its size, its marks,
/// when it reached the server, and its attachments as the server describes
/// them (BODYSTRUCTURE). Its text is not here.
#[derive(Debug, Clone)]
pub struct Found {
    pub account: String,
    pub folder: Folder,
    pub origin: ImapOrigin,
    /// The server's flags, as Maildir letters.
    pub flags: String,
    pub size: u64,
    /// When it reached the server (INTERNALDATE), Unix seconds.
    pub received: Option<i64>,
    /// Its headers as a card, its attachments named (`attachments`, `shape.parts`).
    pub card: Card,
    /// The server described its parts.
    pub parts_known: bool,
}

impl Found {
    /// From what the server gave: its headers, and its parts' names and types when it described them.
    #[allow(clippy::too_many_arguments)]
    pub fn new(account: &str, folder: &Folder, origin: ImapOrigin, flags: String, size: u64, received: Option<i64>, header: &[u8], parts: Option<Vec<(String, String)>>) -> Option<Found> {
        let mut card = Card::from_bytes(header)?;
        card.account = Some(account.to_string());
        card.origin = Some(origin);
        let parts_known = parts.is_some();
        if let Some(parts) = parts {
            card.attachments = parts.iter().map(|(name, _)| name.clone()).filter(|n| !n.is_empty()).collect();
            card.shape.parts = parts;
        }
        Some(Found { account: account.to_string(), folder: folder.clone(), origin, flags, size, received, card, parts_known })
    }

    pub fn date(&self) -> Option<i64> {
        self.card.date
    }

    pub fn message_id(&self) -> Option<String> {
        self.card.message_id.as_deref().map(bare)
    }

    /// Where it is on its server, as a key.
    pub fn place(&self) -> ServerRef {
        ServerRef { account: self.account.clone(), folder: self.folder.name.clone(), origin: self.origin }
    }

    /// Whether it holds for `search`, as far as its headers and parts tell;
    /// what is in its text, the server matched.
    pub fn matches(&self, search: &Search, senders: Option<&Senders>) -> bool {
        let mut facts = Facts::of(&self.card);
        facts.size = Some(self.size);
        // As for mail here (`porch::sent`): its Date, never later than it reached the server.
        facts.arrived = match (self.card.date, self.received) {
            (Some(date), Some(received)) => Some(date.min(received)),
            (date, received) => date.or(received),
        };
        facts.who = senders.map(|s| s.who_of(&self.card));
        let place = Place { account: self.account.clone(), folder: self.folder.name.clone(), role: self.folder.role, flags: self.flags.clone() };
        let message = Message::new(&self.card, facts).placed(place);
        search.decide(&mut |condition, read_whole| match condition.field {
            Field::Body | Field::Anywhere if read_whole => None,
            Field::Attachment | Field::AttachmentType if !self.parts_known => None,
            _ => Some(condition.matches(&message)),
        })
    }
}

/// A message on a server, by its account, its folder's server name and its
/// UIDVALIDITY and UID: `imap://work/INBOX;UIDVALIDITY=7/;UID=42` (RFC 5092's
/// form, the account's id standing for the server), each part percent-encoded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServerRef {
    pub account: String,
    pub folder: String,
    pub origin: ImapOrigin,
}

const SCHEME: &str = "imap://";

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

impl ServerRef {
    pub fn key(&self) -> String {
        format!("{SCHEME}{}/{};UIDVALIDITY={}/;UID={}", percent_encode(&self.account), percent_encode(&self.folder), self.origin.validity, self.origin.uid)
    }

    /// A key back as a place; none for a message's file.
    pub fn parse(key: &str) -> Option<ServerRef> {
        let rest = key.strip_prefix(SCHEME)?;
        let (account, rest) = rest.split_once('/')?;
        let (folder, rest) = rest.split_once(";UIDVALIDITY=")?;
        let (validity, uid) = rest.split_once("/;UID=")?;
        Some(ServerRef { account: percent_decode(account)?, folder: percent_decode(folder)?, origin: ImapOrigin { validity: validity.parse().ok()?, uid: uid.parse().ok()? } })
    }

    /// Whether a key names a message on a server rather than a file.
    pub fn is_key(key: &str) -> bool {
        key.starts_with(SCHEME)
    }
}

// --- IMAP --------------------------------------------------------------------

/// A search key of IMAP's SEARCH (RFC 9051 §6.4.4), its strings kept apart:
/// they go quoted, or as literals when they are not ASCII.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImapKey {
    All,
    /// A key without a string: "UNSEEN", "LARGER 2048", "SINCE 3-Jun-2026".
    Atom(String),
    /// A key and its string: FROM "bank.example".
    Str(&'static str, String),
    /// HEADER "Content-Type" "multipart/mixed".
    Header(&'static str, String),
    /// Gmail's own search (X-GM-EXT-1).
    GmailRaw(String),
    Not(Box<ImapKey>),
    And(Vec<ImapKey>),
    Or(Vec<ImapKey>),
}

/// One word of a SEARCH command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Atom(String),
    /// A string, quoted or sent as a literal.
    Text(String),
}

impl ImapKey {
    /// Its words, as a command's.
    pub fn tokens(&self) -> Vec<Token> {
        let mut out = Vec::new();
        self.push(&mut out, true);
        out
    }

    /// `top`: the command's own list, which needs no parentheses.
    fn push(&self, out: &mut Vec<Token>, top: bool) {
        match self {
            ImapKey::All => out.push(Token::Atom("ALL".into())),
            ImapKey::Atom(atom) => out.push(Token::Atom(atom.clone())),
            ImapKey::Str(key, value) => {
                out.push(Token::Atom((*key).into()));
                out.push(Token::Text(value.clone()));
            }
            ImapKey::Header(name, value) => {
                out.push(Token::Atom("HEADER".into()));
                out.push(Token::Text((*name).into()));
                out.push(Token::Text(value.clone()));
            }
            ImapKey::GmailRaw(query) => {
                out.push(Token::Atom("X-GM-RAW".into()));
                out.push(Token::Text(query.clone()));
            }
            ImapKey::Not(key) => {
                out.push(Token::Atom("NOT".into()));
                key.push(out, false);
            }
            ImapKey::And(keys) => match keys.as_slice() {
                [] => out.push(Token::Atom("ALL".into())),
                [one] => one.push(out, top),
                many => {
                    if !top {
                        out.push(Token::Atom("(".into()));
                    }
                    for key in many {
                        key.push(out, false);
                    }
                    if !top {
                        out.push(Token::Atom(")".into()));
                    }
                }
            },
            ImapKey::Or(keys) => match keys.as_slice() {
                [] => out.push(Token::Atom("ALL".into())),
                [one] => one.push(out, top),
                [first, rest @ ..] => {
                    out.push(Token::Atom("OR".into()));
                    first.push(out, false);
                    ImapKey::Or(rest.to_vec()).push(out, false);
                }
            },
        }
    }

    /// Whether it carries words a server compares (a name, a subject, a
    /// header): what a server may refuse, where days and marks pass.
    pub fn has_text(&self) -> bool {
        match self {
            ImapKey::All | ImapKey::Atom(_) => false,
            ImapKey::Str(..) | ImapKey::Header(..) | ImapKey::GmailRaw(_) => true,
            ImapKey::Not(key) => key.has_text(),
            ImapKey::And(keys) | ImapKey::Or(keys) => keys.iter().any(ImapKey::has_text),
        }
    }

    /// The words as one line, strings quoted: what a test or a log shows.
    pub fn text(&self) -> String {
        let mut line = String::new();
        for token in self.tokens() {
            let word = match &token {
                Token::Atom(a) => a.clone(),
                Token::Text(t) => format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"")),
            };
            if !(line.is_empty() || line.ends_with('(') || word == ")") {
                line.push(' ');
            }
            line.push_str(&word);
        }
        line
    }
}

/// A day as IMAP writes dates: "3-Jun-2026", English month names whatever
/// your language (RFC 9051 §9, date-month).
pub fn imap_date(day: Date) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{}-{}-{}", day.day(), MONTHS[usize::from(day.month().unsigned_abs()) - 1], day.year())
}

fn day(text: &str) -> Option<Date> {
    text.trim().parse::<Date>().ok()
}

/// The headers most messages with attachments wear, signed ones too: what a
/// server without Gmail's search is asked, the parts then checked here.
fn attached() -> ImapKey {
    ImapKey::Or(vec![ImapKey::Header("Content-Type", "multipart/mixed".into()), ImapKey::Header("Content-Type", "multipart/signed".into())])
}

/// One condition as a server key: none when a server cannot say it (who the
/// sender is to you, the day of the week): then the folder's newest are
/// asked, and checked here. A key may find more than the condition (what it
/// finds is checked here, on its headers and parts), never less: what a
/// server would leave out wrongly is not asked of it.
fn imap_of(condition: &Condition, gmail: bool) -> Option<ImapKey> {
    let negative = condition.test.turned().is_some();
    let test = condition.test.turned().unwrap_or(condition.test);
    let value = condition.value.trim().to_string();
    let not = |key: ImapKey| if negative { ImapKey::Not(Box::new(key)) } else { key };
    // Simple words travel to Gmail's own search; others are left to the header.
    let plain = |v: &str| !v.is_empty() && v.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'));
    // A server finds words inside longer ones only: "is not" a whole name,
    // address or subject would leave out "Re: …" and jean-paul@ for paul@.
    if condition.test == Test::IsNot && matches!(condition.field, Field::From | Field::To | Field::Cc | Field::ReplyTo | Field::Subject) {
        return None;
    }
    Some(match condition.field {
        // As here: the sender, the recipients, the subject or the text, not
        // every header (TEXT would find words of the servers' own lines).
        Field::Anywhere => not(ImapKey::Or(["FROM", "TO", "CC", "SUBJECT", "BODY"].into_iter().map(|key| ImapKey::Str(key, value.clone())).collect())),
        Field::From => not(ImapKey::Str("FROM", value)),
        Field::To => not(ImapKey::Str("TO", value)),
        Field::Cc => not(ImapKey::Str("CC", value)),
        Field::ReplyTo => not(ImapKey::Header("Reply-To", value)),
        Field::Subject => not(ImapKey::Str("SUBJECT", value)),
        Field::Body => not(ImapKey::Str("BODY", value)),
        // A newsletter or a list, as `Card::is_list` reads one.
        Field::List if test == Test::Exists => not(ImapKey::Or(vec![
            ImapKey::Header("List-Id", String::new()),
            ImapKey::Header("List-Unsubscribe", String::new()),
            ImapKey::Header("Precedence", "bulk".into()),
            ImapKey::Header("Precedence", "list".into()),
        ])),
        Field::List => not(ImapKey::Header("List-Id", value)),
        Field::Mark => not(ImapKey::Atom(
            match value.as_str() {
                "read" => "SEEN",
                "flagged" => "FLAGGED",
                _ => "ANSWERED",
            }
            .into(),
        )),
        Field::Size => ImapKey::Atom(format!("{} {}", if condition.test == Test::Above { "LARGER" } else { "SMALLER" }, rules::size(&value)?)),
        // The day it arrived is its Date, never later than it reached the
        // server (`porch::sent`). A server reads the Date (SENTBEFORE) and
        // the day it reached it (SINCE, BEFORE), each by its own clock: a day
        // more on either side, the exact day checked here. Before a day:
        // either is (mail moved between servers keeps an old Date and a new
        // day of arrival); after it: both are, so SINCE alone says it.
        Field::Date => {
            let first = day(&value)?;
            let before = |last: Date| -> Option<ImapKey> {
                let end = imap_date(last.tomorrow().ok()?.tomorrow().ok()?);
                Some(ImapKey::Or(vec![ImapKey::Atom(format!("SENTBEFORE {end}")), ImapKey::Atom(format!("BEFORE {end}"))]))
            };
            match condition.test {
                Test::Before => before(first.yesterday().ok()?)?,
                Test::After => ImapKey::Atom(format!("SINCE {}", imap_date(first))),
                _ => ImapKey::And(vec![ImapKey::Atom(format!("SINCE {}", imap_date(first.yesterday().ok()?))), before(day(&condition.until)?)?]),
            }
        }
        // Gmail knows attachments; elsewhere their usual headers are asked, and
        // the parts checked here. Without one is left here: a list's footer
        // makes a message mixed with no attachment.
        Field::Attachment => match (test, gmail) {
            (Test::Exists, true) => not(ImapKey::GmailRaw("has:attachment".into())),
            (Test::Exists, false) if !negative => attached(),
            (Test::Contains, true) if !negative && plain(&value) => ImapKey::GmailRaw(format!("filename:{value}")),
            (Test::Contains, _) if !negative => attached(),
            _ => return None,
        },
        Field::AttachmentType if negative => return None,
        Field::AttachmentType if gmail && value == "pdf" => ImapKey::GmailRaw("filename:pdf".into()),
        Field::AttachmentType if gmail => ImapKey::GmailRaw("has:attachment".into()),
        Field::AttachmentType => attached(),
        Field::Sender | Field::Weekday | Field::Hour | Field::Account | Field::Folder | Field::Unknown => return None,
    })
}

/// What to ask the server for one of an account's folders: none when this
/// search finds nothing there (a folder not named, the junk). `gmail`: the
/// server knows Gmail's own search.
pub fn imap_key(search: &Search, account: &str, folder: &Folder, gmail: bool) -> Option<ImapKey> {
    key_with(search, account, folder, &|condition| imap_of(condition, gmail))
}

/// The same without words, for a server that refused them: the days, the
/// sizes and the marks asked, the rest checked here. GreenMail, for one,
/// reads FROM as a whole address and refuses a part of one ("@bank.example").
pub fn imap_key_without_words(search: &Search, account: &str, folder: &Folder) -> Option<ImapKey> {
    key_with(search, account, folder, &|condition| imap_of(condition, false).filter(|key| !key.has_text()))
}

/// What to ask for a folder, each condition asked as `of` says (none: the
/// server cannot say it, and it is checked here).
fn key_with(search: &Search, account: &str, folder: &Folder, of: &dyn Fn(&Condition) -> Option<ImapKey>) -> Option<ImapKey> {
    if !search.reaches(account, folder) {
        return None;
    }
    let conditions = search.effective();
    let places: Vec<bool> = conditions.iter().filter(|c| scope(c.field)).map(|c| place_holds(c, account, folder)).collect();
    let others: Vec<&Condition> = conditions.iter().copied().filter(|c| !scope(c.field)).collect();
    match search.join {
        Join::Any => {
            // A folder named, or one condition the server cannot say: all of it, checked here.
            if places.iter().any(|&b| b) || others.iter().any(|c| of(c).is_none()) {
                return Some(ImapKey::All);
            }
            let keys: Vec<ImapKey> = others.iter().filter_map(|c| of(c)).collect();
            (!keys.is_empty()).then_some(ImapKey::Or(keys))
        }
        Join::All => {
            let keys: Vec<ImapKey> = others.iter().filter_map(|c| of(c)).collect();
            Some(if keys.is_empty() { ImapKey::All } else { ImapKey::And(keys) })
        }
    }
}

/// The first day all the conditions leave in (its arrival): what is kept
/// here since an earlier day needs no server.
pub fn earliest(search: &Search) -> Option<Date> {
    if search.join == Join::Any {
        return None;
    }
    search
        .effective()
        .iter()
        .filter(|c| c.field == Field::Date)
        .filter_map(|c| match c.test {
            Test::After => day(&c.value)?.tomorrow().ok(),
            Test::Between => day(&c.value),
            _ => None,
        })
        .max()
}

// --- Words -------------------------------------------------------------------

/// How the window names an address and a folder in a sentence.
pub struct Names<'a> {
    pub account: &'a dyn Fn(&str) -> String,
    pub folder: &'a dyn Fn(&str) -> String,
}

/// "3 June", with its year when it is not this one's.
fn day_words(tr: &Translator, text: &str, today: Date) -> String {
    match day(text) {
        Some(d) if d.year() == today.year() => tr.day_month(d),
        Some(d) => format!("{} {}", tr.day_month(d), d.year()),
        None => text.trim().to_string(),
    }
}

/// "5 MB", "500 KB", in your language's units and decimals.
fn size_words(tr: &Translator, bytes: u64) -> String {
    let mut args = i18n::args();
    let (id, value) = if bytes >= 1024 * 1024 { ("size-mb", tr.decimal(bytes as f32 / (1024.0 * 1024.0))) } else { ("size-kb", (bytes / 1024).to_string()) };
    args.set("n", value);
    tr.text(id, Some(&args))
}

/// One condition in words: "from …@bank.example…", "arrived after 3 June";
/// the filters' own words for those the search does not offer (`Condition::said`).
fn phrase(condition: &Condition, tr: &Translator, names: &Names, today: Date) -> String {
    let mut args = i18n::args();
    let value = condition.value.trim().to_string();
    let test = condition.test.id();
    let id = match condition.field {
        Field::ReplyTo | Field::Weekday | Field::Hour | Field::Unknown => return condition.said(tr),
        Field::Date => {
            args.set("day", day_words(tr, &value, today));
            args.set("until", day_words(tr, &condition.until, today));
            format!("search-phrase-date-{test}")
        }
        Field::Size => {
            args.set("size", rules::size(&value).map_or(value, |b| size_words(tr, b)));
            format!("search-phrase-size-{test}")
        }
        Field::Account => {
            args.set("account", (names.account)(&value));
            format!("search-phrase-account-{test}")
        }
        Field::Folder => {
            args.set("folder", (names.folder)(&value));
            format!("search-phrase-folder-{test}")
        }
        Field::Mark => format!("search-phrase-mark-{test}-{value}"),
        Field::AttachmentType => {
            args.set("kind", tr.text(&format!("filter-kind-{value}"), None));
            format!("search-phrase-attachment-type-{test}")
        }
        Field::Sender => {
            args.set("who", tr.text(&format!("filter-who-{value}"), None));
            format!("search-phrase-sender-{test}")
        }
        field => {
            args.set("value", value);
            format!("search-phrase-{}-{test}", field.id())
        }
    };
    tr.text(&id, Some(&args))
}

/// What a search looks for, in a sentence: "Mail from …@bank.example…,
/// arrived after 3 June, with an attachment."
pub fn sentence(search: &Search, tr: &Translator, names: &Names, today: Date) -> String {
    let conditions: Vec<&Condition> = search.conditions.iter().filter(|c| c.field != Field::Unknown && c.problem().is_none()).collect();
    if conditions.is_empty() {
        return tr.text("search-sentence-empty", None);
    }
    let phrases: Vec<String> = conditions.iter().map(|c| phrase(c, tr, names, today)).collect();
    let joined = phrases.join(&tr.text(if search.join == Join::Any { "search-join-any" } else { "search-join-all" }, None));
    let mut args = i18n::args();
    args.set("phrases", joined);
    tr.text("search-sentence", Some(&args))
}

/// What the search's column offers, in your language: the filters' fields
/// the search uses and its own four, with their tests; the kinds of
/// attachments, who a sender is, the marks, the folders' purposes. The
/// addresses and each one's folders are the window's to add.
#[derive(Debug, Clone, Serialize)]
pub struct Form {
    pub fields: Vec<rules::FieldForm>,
    pub kinds: Vec<rules::Choice>,
    pub who: Vec<rules::Choice>,
    pub marks: Vec<rules::Choice>,
    pub roles: Vec<rules::Choice>,
}

pub fn form(tr: &Translator) -> Form {
    let filters = rules::form(tr);
    let fields = Field::SEARCH
        .iter()
        .map(|&field| {
            filters.fields.iter().find(|f| f.id == field.id()).cloned().unwrap_or_else(|| rules::FieldForm {
                id: field.id().into(),
                label: tr.text(&format!("filter-field-{}", field.id()), None),
                tests: field.tests().iter().map(|&test| rules::TestForm { id: test.id().into(), label: tr.text(&format!("filter-test-{}", test.id()), None), value: field.value(test) }).collect(),
            })
        })
        .collect();
    let choice = |id: &str, message: String| rules::Choice { id: id.to_string(), label: tr.text(&message, None) };
    Form {
        fields,
        kinds: filters.kinds,
        who: filters.who,
        marks: rules::MARKS.iter().map(|m| choice(m, format!("filter-mark-{m}"))).collect(),
        roles: rules::ROLES.iter().map(|(id, role)| choice(id, role.message_id().to_string())).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folders;

    fn condition(field: Field, test: Test, value: &str) -> Condition {
        Condition::new(field, test, value)
    }

    fn all(conditions: Vec<Condition>) -> Search {
        Search { conditions, join: Join::All }
    }

    fn inbox() -> Folder {
        folders::folder("INBOX", None, None)
    }

    /// A message on disk, its flags in its name.
    fn stored(dir: &Path, name: &str, raw: &str, folder: &Folder) -> Stored {
        let cur = dir.join("cur");
        std::fs::create_dir_all(&cur).unwrap();
        let path = cur.join(name);
        std::fs::write(&path, raw).unwrap();
        Stored::new(path, "work", folder)
    }

    const BANK: &str = "From: Banque des Berges <no-reply@banque.example>\r\nTo: Noa <noa@example.com>\r\nCc: Paul <paul@example.org>\r\nSubject: Votre relevé de juin\r\nDate: Wed, 03 Jun 2026 10:00:00 +0200\r\nMessage-ID: <releve@banque.example>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\nVotre relevé est joint. Réunion le 12.\r\n--b\r\nContent-Type: application/pdf; name=\"releve-juin.pdf\"\r\nContent-Disposition: attachment; filename=\"releve-juin.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQK\r\n--b--\r\n";

    #[test]
    fn reads_the_conditions_the_window_sends() {
        // The filters' own conditions, as their configuration writes them.
        let search: Search = serde_json::from_str(r#"{"match": "any", "conditions": [{"field": "from", "test": "contains", "value": "@bank.example"}, {"field": "date", "test": "between", "value": "2026-06-01", "until": "2026-06-30"}, {"field": "attachment", "test": "exists"}, {"field": "mark", "test": "is-not", "value": "read"}, {"field": "folder", "test": "is", "value": "inbox"}]}"#).unwrap();
        assert_eq!(search.join, Join::Any);
        assert_eq!(search.conditions.len(), 5);
        assert_eq!(search.conditions[3], Condition::new(Field::Mark, Test::IsNot, "read"));
        assert_eq!(search.effective().len(), 5);
        // Saying nothing: empty text, a day that is none, a test the field does not take.
        assert!(all(vec![condition(Field::Subject, Test::Contains, "  ")]).is_empty());
        assert!(all(vec![condition(Field::Date, Test::After, "June")]).is_empty());
        assert!(all(vec![condition(Field::Subject, Test::Above, "x")]).is_empty());
    }

    #[test]
    fn matches_a_stored_message_reading_as_little_as_needed() {
        let dir = std::env::temp_dir().join(format!("sioul-mailsearch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let message = stored(&dir, "1.U7-1.sioul:2,S", BANK, &inbox());
        let yes = |c: Condition| assert!(message.matches(&all(vec![c.clone()]), None), "{c:?}");
        let no = |c: Condition| assert!(!message.matches(&all(vec![c.clone()]), None), "{c:?}");
        // Case and accents aside, as the filters.
        yes(condition(Field::From, Test::Contains, "@BANQUE.example"));
        yes(condition(Field::From, Test::Is, "no-reply@banque.example"));
        no(condition(Field::From, Test::Is, "banque.example"));
        no(condition(Field::From, Test::NotContains, "banque"));
        yes(condition(Field::Cc, Test::Contains, "paul"));
        yes(condition(Field::To, Test::IsNot, "jane@example.org"));
        yes(condition(Field::Subject, Test::Contains, "releve"));
        yes(condition(Field::Body, Test::Contains, "reunion"));
        no(condition(Field::Body, Test::Contains, "absent"));
        yes(condition(Field::Anywhere, Test::Contains, "réunion"));
        yes(condition(Field::Anywhere, Test::Contains, "paul"));
        yes(condition(Field::Anywhere, Test::NotContains, "absent"));
        no(condition(Field::Anywhere, Test::NotContains, "berges"));
        yes(condition(Field::Attachment, Test::Exists, ""));
        no(condition(Field::Attachment, Test::Missing, ""));
        yes(condition(Field::Attachment, Test::Contains, "JUIN"));
        yes(condition(Field::AttachmentType, Test::Is, "pdf"));
        no(condition(Field::AttachmentType, Test::Is, "image"));
        yes(condition(Field::Size, Test::Below, "1 MB"));
        yes(condition(Field::Mark, Test::Is, "read"));
        no(condition(Field::Mark, Test::IsNot, "read"));
        no(condition(Field::Mark, Test::Is, "flagged"));
        yes(condition(Field::Account, Test::Is, "work"));
        no(condition(Field::Account, Test::IsNot, "work"));
        yes(condition(Field::Folder, Test::Is, "inbox"));
        no(condition(Field::Folder, Test::Is, "Banque"));
        // Its day: its Date, never later than its file (written now, so the Date).
        yes(condition(Field::Date, Test::After, "2026-06-02"));
        no(condition(Field::Date, Test::After, "2026-06-03"));
        yes(Condition { field: Field::Date, test: Test::Between, value: "2026-06-03".into(), until: "2026-06-03".into() });
        // Any of them: one is enough.
        let either = Search { conditions: vec![condition(Field::From, Test::Contains, "absent"), condition(Field::Subject, Test::Contains, "relevé")], join: Join::Any };
        assert!(message.matches(&either, None));
        assert!(!message.matches(&all(either.conditions.clone()), None));
        // The headers alone answer what they can: the whole message is not read for them.
        let light = stored(&dir, "2.U7-2.sioul:2,", BANK, &inbox());
        assert!(!light.matches(&all(vec![condition(Field::From, Test::Contains, "absent"), condition(Field::Body, Test::Contains, "reunion")]), None));
        assert!(light.whole.get().is_none(), "the text was not read");
        assert!(light.matches(&all(vec![condition(Field::Anywhere, Test::Contains, "berges")]), None));
        assert!(light.whole.get().is_none(), "found in the headers: the text was not read");
        assert_eq!(light.message_id().as_deref(), Some("releve@banque.example"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_junk_and_the_trash_only_when_named() {
        let junk = folders::folder("Junk", None, Some(Role::Junk));
        let search = all(vec![condition(Field::From, Test::Contains, "x")]);
        assert!(search.reaches("work", &inbox()));
        assert!(!search.reaches("work", &junk));
        let named = all(vec![condition(Field::From, Test::Contains, "x"), condition(Field::Folder, Test::Is, "junk")]);
        assert!(named.reaches("work", &junk));
        assert!(!named.reaches("work", &inbox()));
        // Another address: not with all of them; with any, its folders still hold what the others say.
        let elsewhere = all(vec![condition(Field::Account, Test::Is, "home"), condition(Field::Subject, Test::Contains, "x")]);
        assert!(!elsewhere.reaches("work", &inbox()));
        assert!(Search { join: Join::Any, ..elsewhere }.reaches("work", &inbox()));
    }

    #[test]
    fn asks_a_server_the_same() {
        let search = all(vec![condition(Field::From, Test::Contains, "@bank.example"), condition(Field::Date, Test::After, "2026-06-02"), condition(Field::Attachment, Test::Exists, ""), condition(Field::Mark, Test::IsNot, "read")]);
        let key = imap_key(&search, "work", &inbox(), false).unwrap();
        // The headers' first, then what reads the whole message; a day more
        // on either side of a date (each server reads days by its own clock).
        assert_eq!(key.text(), r#"FROM "@bank.example" SINCE 2-Jun-2026 NOT SEEN OR HEADER "Content-Type" "multipart/mixed" HEADER "Content-Type" "multipart/signed""#);
        // Gmail knows attachments.
        assert_eq!(imap_key(&search, "work", &inbox(), true).unwrap().text(), r#"FROM "@bank.example" SINCE 2-Jun-2026 NOT SEEN X-GM-RAW "has:attachment""#);
        // Any of them: OR, nested two by two.
        let any = Search { conditions: vec![condition(Field::To, Test::Contains, "paul"), condition(Field::Subject, Test::NotContains, "news"), condition(Field::Size, Test::Above, "2 MB")], join: Join::Any };
        assert_eq!(imap_key(&any, "work", &inbox(), false).unwrap().text(), r#"OR TO "paul" OR NOT SUBJECT "news" LARGER 2097152"#);
        // What a server cannot say: with all of them, left to the check here; with any, everything is asked.
        let who = all(vec![condition(Field::Sender, Test::Is, "stranger"), condition(Field::List, Test::Exists, "")]);
        assert_eq!(imap_key(&who, "work", &inbox(), false).unwrap().text(), r#"OR HEADER "List-Id" "" OR HEADER "List-Unsubscribe" "" OR HEADER "Precedence" "bulk" HEADER "Precedence" "list""#);
        assert_eq!(imap_key(&Search { join: Join::Any, ..who }, "work", &inbox(), false), Some(ImapKey::All));
        // "Is not" a whole address or subject: a server would leave out longer ones (jean-paul@, "Re: …").
        let not_paul = all(vec![condition(Field::From, Test::IsNot, "paul@example.org"), condition(Field::Subject, Test::Contains, "x")]);
        assert_eq!(imap_key(&not_paul, "work", &inbox(), false).unwrap().text(), r#"SUBJECT "x""#);
        assert_eq!(imap_key(&Search { join: Join::Any, ..not_paul }, "work", &inbox(), false), Some(ImapKey::All));
        // Anywhere: the sender, the recipients, the subject or the text, as here; not the servers' own lines.
        let anywhere = all(vec![condition(Field::Anywhere, Test::Contains, "facture")]);
        assert_eq!(imap_key(&anywhere, "work", &inbox(), false).unwrap().text(), r#"OR FROM "facture" OR TO "facture" OR CC "facture" OR SUBJECT "facture" BODY "facture""#);
        let nowhere = all(vec![condition(Field::Anywhere, Test::NotContains, "facture")]);
        assert_eq!(imap_key(&nowhere, "work", &inbox(), false).unwrap().text(), r#"NOT OR FROM "facture" OR TO "facture" OR CC "facture" OR SUBJECT "facture" BODY "facture""#);
        // Without attachments: Gmail knows; elsewhere a list's footer makes a message mixed, so it is checked here.
        let bare = all(vec![condition(Field::Attachment, Test::Missing, "")]);
        assert_eq!(imap_key(&bare, "work", &inbox(), false), Some(ImapKey::All));
        assert_eq!(imap_key(&bare, "work", &inbox(), true).unwrap().text(), r#"NOT X-GM-RAW "has:attachment""#);
        // The day it arrived: its Date or the day it reached the server, whichever came first.
        let before = all(vec![condition(Field::Date, Test::Before, "2026-06-10")]);
        assert_eq!(imap_key(&before, "work", &inbox(), false).unwrap().text(), "OR SENTBEFORE 11-Jun-2026 BEFORE 11-Jun-2026");
        let between = all(vec![Condition { field: Field::Date, test: Test::Between, value: "2026-06-01".into(), until: "2026-06-30".into() }]);
        assert_eq!(imap_key(&between, "work", &inbox(), false).unwrap().text(), "SINCE 31-May-2026 OR SENTBEFORE 2-Jul-2026 BEFORE 2-Jul-2026");
        // Grouped where IMAP needs it.
        let grouped = Search { conditions: vec![between.conditions[0].clone(), condition(Field::Body, Test::Contains, "x")], join: Join::Any };
        assert_eq!(imap_key(&grouped, "work", &inbox(), false).unwrap().text(), r#"OR (SINCE 31-May-2026 OR SENTBEFORE 2-Jul-2026 BEFORE 2-Jul-2026) BODY "x""#);
        // Folders: all of them, a folder not named is not asked; any of them, a folder named is asked everything.
        let in_bank = all(vec![condition(Field::Folder, Test::Is, "Bank"), condition(Field::Subject, Test::Contains, "x")]);
        assert_eq!(imap_key(&in_bank, "work", &inbox(), false), None);
        let bank = folders::folder("Bank", None, None);
        assert_eq!(imap_key(&in_bank, "work", &bank, false).unwrap().text(), r#"SUBJECT "x""#);
        assert_eq!(imap_key(&Search { join: Join::Any, ..in_bank }, "work", &bank, false), Some(ImapKey::All));
        // A server that refused the words: the days and the marks only, the rest checked here.
        assert!(key.has_text() && !ImapKey::Atom("SINCE 2-Jun-2026".into()).has_text());
        assert_eq!(imap_key_without_words(&search, "work", &inbox()).unwrap().text(), "SINCE 2-Jun-2026 NOT SEEN");
        assert_eq!(imap_key_without_words(&any, "work", &inbox()), Some(ImapKey::All));
        assert_eq!(earliest(&search), Some(jiff::civil::date(2026, 6, 3)));
    }

    #[test]
    fn a_message_seen_on_its_server_only() {
        let header = BANK.split("\r\n\r\n").next().unwrap().to_string() + "\r\n\r\n";
        let found = Found::new("work", &inbox(), ImapOrigin { validity: 7, uid: 42 }, "S".into(), 2048, None, header.as_bytes(), Some(vec![("releve-juin.pdf".into(), "application/pdf".into())])).unwrap();
        let holds = |conditions: Vec<Condition>| found.matches(&all(conditions), None);
        // Its text is not here: the server matched it.
        assert!(holds(vec![condition(Field::Body, Test::Contains, "anything")]));
        assert!(holds(vec![condition(Field::Anywhere, Test::Contains, "anything")]));
        assert!(!holds(vec![condition(Field::Anywhere, Test::NotContains, "berges")]), "in its headers: known here");
        assert!(holds(vec![condition(Field::From, Test::Contains, "banque.example"), condition(Field::AttachmentType, Test::Is, "pdf"), condition(Field::Size, Test::Below, "1 MB")]));
        assert!(!holds(vec![condition(Field::From, Test::Is, "banque.example")]));
        assert_eq!(found.card.attachments, vec!["releve-juin.pdf".to_string()]);
        assert_eq!(found.message_id().as_deref(), Some("releve@banque.example"));
        // Its key, back and forth, whatever the folder's name holds.
        let place = ServerRef { account: "perso".into(), folder: "INBOX/Impôts 2026;x".into(), origin: ImapOrigin { validity: 7, uid: 42 } };
        let key = place.key();
        assert!(ServerRef::is_key(&key) && !key.contains(' '), "{key}");
        assert_eq!(ServerRef::parse(&key), Some(place));
        assert_eq!(ServerRef::parse("/home/noa/.local/share/sioul/mail/work/cur/1.U7-1.sioul:2,S"), None);
        assert_eq!(found.place().key(), "imap://work/INBOX;UIDVALIDITY=7/;UID=42");
    }

    /// How fast a mailbox is looked through, on one core, its files in the
    /// page cache: `SIOUL_BENCH_DIR=… cargo test --release -p sioul-core -- --ignored
    /// --nocapture search_speed` (twenty thousand messages written there, then taken away).
    #[test]
    #[ignore]
    fn search_speed() {
        let dir = PathBuf::from(std::env::var("SIOUL_BENCH_DIR").expect("a folder for the messages")).join(format!("bench-{}", std::process::id()));
        let cur = dir.join("cur");
        std::fs::create_dir_all(&cur).unwrap();
        let filler = "Bonjour, voici les nouvelles de la semaine et quelques mots de plus pour faire un texte de taille ordinaire.\r\n".repeat(40);
        for i in 0..20_000 {
            let from = if i % 50 == 0 { "no-reply@banque.example" } else { "someone@example.org" };
            let raw = format!("From: X <{from}>\r\nTo: noa@example.com\r\nSubject: Message {i}\r\nDate: Wed, 03 Jun 2026 10:00:00 +0200\r\nMessage-ID: <{i}@example.org>\r\nDKIM-Signature: v=1; a=rsa-sha256; d=example.org; s=s; b={}\r\n\r\n{filler}{}\r\n", "x".repeat(300), if i % 97 == 0 { "facture" } else { "" });
            std::fs::write(cur.join(format!("{i}.U7-{i}.sioul:2,S")), raw).unwrap();
        }
        let paths: Vec<PathBuf> = std::fs::read_dir(&cur).unwrap().map(|e| e.unwrap().path()).collect();
        for (name, search) in [("headers", all(vec![condition(Field::From, Test::Contains, "banque")])), ("text", all(vec![condition(Field::Body, Test::Contains, "facture")])), ("anywhere", all(vec![condition(Field::Anywhere, Test::Contains, "facture")]))] {
            // Once to fill the page cache, then timed.
            for timed in [false, true] {
                let start = std::time::Instant::now();
                let found = paths.iter().filter(|p| Stored::new((*p).clone(), "work", &inbox()).matches(&search, None)).count();
                if timed {
                    eprintln!("{name}: {found} of {} in {:?}", paths.len(), start.elapsed());
                }
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn says_what_it_looks_for() {
        let today = jiff::civil::date(2026, 10, 7);
        let accounts = |id: &str| if id == "work" { "noa@example.com".to_string() } else { id.to_string() };
        let folders_named = |id: &str| id.to_string();
        let names = Names { account: &accounts, folder: &folders_named };
        let search = all(vec![condition(Field::From, Test::Contains, "@bank.example"), condition(Field::Date, Test::After, "2026-06-03"), condition(Field::Attachment, Test::Exists, ""), condition(Field::Subject, Test::Contains, "")]);
        let en = Translator::new("en");
        assert_eq!(sentence(&search, &en, &names, today), "Mail from …@bank.example…, arrived after 3 June, with an attachment.");
        let fr = Translator::new("fr");
        assert_eq!(sentence(&search, &fr, &names, today), "Le courrier de …@bank.example…, arrivé après le 3 juin, avec une pièce jointe.");
        let any = Search { conditions: vec![condition(Field::Account, Test::Is, "work"), condition(Field::AttachmentType, Test::Is, "pdf"), condition(Field::Mark, Test::IsNot, "read")], join: Join::Any };
        assert_eq!(sentence(&any, &en, &names, today), "Mail in noa@example.com, or with a PDF attached, or not read yet.");
        assert_eq!(sentence(&Search::default(), &en, &names, today), "Say what to look for.");
        // Every field and test the search offers has its words, in both languages.
        for tr in [&en, &fr] {
            let form = form(tr);
            assert_eq!(form.fields.len(), Field::SEARCH.len());
            for field in &form.fields {
                assert!(!field.label.starts_with("filter-") && field.tests.iter().all(|t| !t.label.starts_with("filter-")), "{field:?}");
            }
            for &field in &Field::SEARCH {
                for &test in field.tests() {
                    let value = match (field, field.value(test)) {
                        (Field::Mark, _) => "read",
                        (_, rules::Value::Kind) => "pdf",
                        (_, rules::Value::Who) => "known",
                        (_, rules::Value::Size) => "2 MB",
                        (_, rules::Value::Date | rules::Value::Dates) => "2026-06-03",
                        _ => "x",
                    };
                    let said = phrase(&Condition { field, test, value: value.into(), until: "2026-06-04".into() }, tr, &names, today);
                    assert!(!said.starts_with("search-") && !said.contains('{'), "{field:?} {test:?}: {said}");
                }
            }
            for mark in rules::MARKS {
                for test in [Test::Is, Test::IsNot] {
                    assert!(!phrase(&condition(Field::Mark, test, mark), tr, &names, today).starts_with("search-"));
                }
            }
        }
    }
}

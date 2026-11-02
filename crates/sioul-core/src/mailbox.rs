// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The mailbox (the unified mailbox's §4, part 4): one list of every
//! address's mail, newest first, made from the folders' indexes
//! (`mailcache`, §5.2), never a message parsed but for a shielded address's
//! mail from someone not let in, read once per process for the shield.
//!
//! - **What it lists**: each address's inbox, your own folders and the
//!   archive (received), Sent (sent), the junk (spam); drafts and the trash
//!   never, nor what waits for its Undo, nor mail from blocked senders.
//! - **Marks**: each address's letter on its tint, no two letters alike
//!   (`letters`). Gmail's
//!   All Mail counts as the archive where there is no other, its copies folding
//!   with the inbox's and Sent's.
//! - **Each message once** (decision 6): the copies of one address (`mid:`,
//!   else the digest) make one row, every kind and address it has kept; the
//!   copy that ranks first opens (`Role::rank`, then the first to arrive).
//!   What you do to a row acts on each address's first-ranked copy
//!   (`Row::keys`), never on two copies of one address; it reads as read, and
//!   flagged, as those copies are.
//! - **Labels** (Gmail: an address with All Mail): one message in several
//!   folders. A copy waiting for its Undo, in the trash or in the junk is
//!   where the message is; that address's other copies are left until their
//!   folders' next round brings them up to date.
//! - **Order** (decision 11): by when it was sent, never later than it
//!   arrived (`porch::sent`), newest first; days as headers; two weeks, then
//!   two more at each "Earlier messages" (decision 15), and `PAGE_ROWS` rows
//!   a window at most, so that a crowded fortnight weighs one folder's page.
//! - **Spam** (§5.2, 3): a copy in a Junk folder; one your own filter flagged
//!   where it lies (its log); your provider's flag on a stranger's mail; what
//!   you said is not spam (`$NotJunk`, the label log) wins. Spam rows show
//!   under the Spam chip only (decision 7), with their quiet word.
//! - **Calm** (§4.7): a resting address's rows leave the list, one line
//!   saying so; mail whose time has not come (its cell ○ Later) shows
//!   without its dot (decision 12); spam, forged and hostile mail never have
//!   one; nothing is counted (decision 13).

use crate::card::Card;
use crate::config::Account;
use crate::folders::{Folder, Role};
use crate::i18n::{self, Translator};
use crate::mailcache::{self, Line};
use crate::maildir;
use crate::porch::{By, SenderList, Senders};
use crate::reach::Who;
use crate::spam::labels;
use crate::trust::{self, Trust};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Days of one window: the mailbox's two weeks.
pub const WINDOW_DAYS: i64 = 14;

/// Rows of one window at most: a page as light as one folder's, whatever
/// two weeks hold; "Earlier messages" shows the next ones.
pub const PAGE_ROWS: usize = 500;

/// What the list shows: one chip at a time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chip {
    /// What you received and sent.
    #[default]
    All,
    /// What you received and have not read.
    Unread,
    Sent,
    Spam,
}

impl Chip {
    pub fn parse(text: &str) -> Chip {
        match text {
            "unread" => Chip::Unread,
            "sent" => Chip::Sent,
            "spam" => Chip::Spam,
            _ => Chip::All,
        }
    }
}

fn one() -> u32 {
    1
}

/// What the page asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    #[serde(default)]
    pub chip: Chip,
    /// One address only (its id); empty: every address.
    #[serde(default)]
    pub account: String,
    /// How many two-week windows: one at first, one more at each "Earlier messages".
    #[serde(default = "one")]
    pub windows: u32,
    /// The quick search's words: everything kept, by sender, recipients and subject.
    #[serde(default)]
    pub query: String,
    /// Resting addresses you chose to see all the same ("Show its mail").
    #[serde(default)]
    pub shown_resting: Vec<String>,
}

impl Default for Ask {
    fn default() -> Ask {
        Ask { chip: Chip::All, account: String::new(), windows: 1, query: String::new(), shown_resting: Vec::new() }
    }
}

/// An address as the mailbox takes it, ranked as Mail's column ranks them.
#[derive(Debug, Clone)]
pub struct Address {
    pub account: Account,
    /// Its folders as its server last listed them.
    pub folders: Vec<Folder>,
    /// Whose stamps count for its mail (`Config::mail_sources`).
    pub trusted: Vec<String>,
    /// Its mark's colour, one of the palette's (`tints`).
    pub tint: u8,
    /// When it rests now, the line saying so ("work@example.org rests until 09:00.").
    pub resting: Option<String>,
}

/// Each address's colour: its own when chosen, else the palette's in the
/// order of `ranked` (the addresses' priority), eight at most, then again.
pub fn tints(ranked: &[&Account]) -> Vec<u8> {
    ranked.iter().enumerate().map(|(i, a)| a.colour.filter(|c| usize::from(*c) < crate::config::PALETTE).unwrap_or((i % crate::config::PALETTE) as u8)).collect()
}

/// What weighs each row, read once a page.
pub struct World {
    pub senders: Senders,
    /// The senders you let in through the screener.
    pub known: SenderList,
    /// Always through.
    pub always: crate::everywhere::People,
    pub attention: crate::attention::Attention,
    pub now: crate::attention::Now,
    /// The label log: what you said of a message, on every device.
    pub said: Vec<labels::Entry>,
    /// What your own filter flagged where it lies, and moved into a Junk folder.
    pub flagged: Vec<labels::Flagged>,
    pub moved: Vec<labels::Moved>,
    /// The AI's shield answers, by message.
    pub assessments: BTreeMap<String, crate::shield::Assessment>,
    pub words: std::sync::Arc<crate::words::Words>,
}

impl World {
    /// Read from the configuration and the logs, the moment `now` given.
    pub fn load(config: &crate::config::Config, now: crate::attention::Now) -> World {
        let root = labels::state();
        World {
            senders: Senders::load(config),
            known: crate::porch::KnownSenders::load(&config.known_senders_path()),
            always: crate::everywhere::People::load(&crate::everywhere::People::default_path()),
            attention: crate::attention::Attention::of(config),
            now,
            said: labels::read_all_in(&root),
            flagged: labels::read_flagged_in(&root),
            moved: labels::read_moved_in(&root),
            assessments: crate::shield::AiCache::load_all(),
            words: crate::words::current(),
        }
    }
}

/// One row: a message, once, however many folders and addresses hold it.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// The file of the copy that opens.
    pub key: String,
    /// What the row's actions act on: the first-ranked copy of each address
    /// that holds it, `key` first; never two copies of one address (Gmail's
    /// labels are one message: deleting one deletes them all).
    pub keys: Vec<String>,
    /// Its address (`mid:…`, `sioul:mail/…`).
    pub uri: String,
    /// The copy that opens: its address's id, its folder's server name and purpose.
    pub account: String,
    pub folder: String,
    pub role: Role,
    /// "Ana"; "To Jane, Paul"; "You, to Jane"; "someone at example.org".
    pub who: String,
    /// The sender's address.
    pub address: String,
    pub subject: String,
    /// The day's header it goes under: "Today", "Yesterday", "Monday 26 October".
    pub day: String,
    /// "09:41".
    pub time: String,
    /// The dot: waits to be read, and may reach you now.
    pub unread: bool,
    /// Its file read (the menu's Mark read or unread).
    pub read: bool,
    pub flagged: bool,
    pub answered: bool,
    pub attachments: bool,
    /// "verified", "unverified", "forged"; "own" for what you sent.
    pub trust_level: &'static str,
    /// Its trust in words ("Signed by example.org"; why it is forged).
    pub trust: String,
    /// Its quiet word when it is spam ("probably spam"), else "".
    pub spam: String,
    /// A shielded address's hostile mail: its words kept to itself.
    pub hostile: bool,
    /// Its addresses' marks, ranked; none with a single address.
    pub marks: Vec<Mark>,
    /// "received", "sent", "spam".
    pub kinds: Vec<&'static str>,
    /// Never a message seen on its server only: the folders' menu reads it.
    pub server: bool,
    /// When it was sent (Unix seconds): the order.
    #[serde(skip)]
    pub sent: i64,
}

/// An address's mark: its first letter in its tint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mark {
    pub account: String,
    pub letter: String,
    pub tint: u8,
    /// The whole address, for the tooltip and screen readers.
    pub title: String,
}

/// A resting address's line, and how to see its mail all the same.
#[derive(Debug, Clone, Serialize)]
pub struct Resting {
    pub account: String,
    pub line: String,
}

/// One page of the mailbox.
#[derive(Debug, Clone, Serialize)]
pub struct Page {
    pub rows: Vec<Row>,
    pub resting: Vec<Resting>,
    /// The marks at the end of the chips' row, to keep one address; none with one.
    pub addresses: Vec<Mark>,
    /// Older mail is kept: "Earlier messages".
    pub earlier: bool,
    /// What is there, in words, when nothing is: never a count.
    pub sentence: String,
    pub chip: Chip,
    pub account: String,
    pub windows: u32,
}

/// A message listed, before its row is made: what the chips and the order need.
struct Listed {
    uri: String,
    card: Card,
    /// Its addresses' places in `addresses`, ranked.
    held: Vec<usize>,
    kinds: Vec<&'static str>,
    spam: Option<String>,
    sent_kind: bool,
    received_kind: bool,
    unread_here: bool,
    read: bool,
    /// Each address's first-ranked copy, in `copies`.
    acting: Vec<usize>,
    from_own: bool,
    stranger: bool,
    sent: i64,
    /// Ranked: the first opens.
    copies: Vec<Copy>,
}

/// One copy of a message, as a folder's index gives it.
struct Copy {
    path: PathBuf,
    line: Line,
    /// Its address's place in `addresses`.
    address: usize,
    folder: String,
    role: Role,
    flags: String,
    sent: i64,
    /// Waiting for its Undo.
    hidden: bool,
}

/// Whether a folder is kept here, as a sync keeps it (`sioul_sync::mailbox::fetched`): the
/// inbox always; another folder unless left on the server or without a directory of
/// its own; Gmail's All Mail only where there is no other archive.
fn kept(folder: &Folder, all: &[Folder], account: &Account) -> bool {
    let own_dir = || matches!(Path::new(&folder.local).components().collect::<Vec<_>>().as_slice(), [std::path::Component::Normal(_)]);
    (folder.role != Role::All || !all.iter().any(|f| f.role == Role::Archive)) && (folder.role == Role::Inbox || (!account.skip_folders.contains(&folder.name) && own_dir()))
}

/// When a message was sent, never later than it arrived (`porch::sent`).
fn sent_at(line: &Line) -> i64 {
    match line.date {
        Some(date) if line.arrived > 0 => date.min(line.arrived),
        Some(date) => date,
        None => line.arrived,
    }
}

/// Whether `query` (lower case) is found in its sender, recipients or subject.
fn found(line: &Line, query: &str) -> bool {
    query.is_empty()
        || [line.from_name.as_deref().unwrap_or(""), line.from_address.as_deref().unwrap_or(""), line.subject.as_str()]
            .into_iter()
            .chain(line.to.iter().map(String::as_str))
            .any(|t| t.to_lowercase().contains(query))
}

/// The shield's reading of messages read for it, by address and file: once per process.
static HOSTILE: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);

/// Whether a shielded address's message is hostile, as the Porch reads it
/// (`shield::reading`: the AI's answer, else its words): the message read
/// once per process for it, never written anywhere.
fn hostile(account: &str, path: &Path, world: &World) -> bool {
    let key = format!("{account}/{}", maildir::unique_part(&path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    if let Some(known) = HOSTILE.lock().ok().and_then(|m| m.as_ref().and_then(|m| m.get(&key).copied())) {
        return known;
    }
    let found = maildir::read_one(path).is_some_and(|card| crate::shield::reading(&card, Some(&world.assessments)).tone == crate::shield::Tone::Hostile);
    if let Ok(mut kept) = HOSTILE.lock() {
        kept.get_or_insert_with(HashMap::new).insert(key, found);
    }
    found
}

/// Whether a sender is someone you know, as the Porch counts them: let in,
/// in your address books or on a list by their address, card or category, or safe.
fn knows(world: &World, card: &Card) -> bool {
    let judged = card.from_address.as_deref().map(|a| world.senders.judge(a));
    world.known.knows(card) || judged.is_some_and(|j| j.who == Who::Safe || matches!(j.by, By::Address | By::Card | By::Category | By::Book | By::LetIn))
}

/// "Today", "Yesterday", "Monday 26 October", with the year when not this one.
fn day_title(tr: &Translator, at: &jiff::Zoned, today: jiff::civil::Date) -> String {
    let date = at.date();
    if date == today {
        return tr.text("mailbox-today", None);
    }
    if today.yesterday().is_ok_and(|y| y == date) {
        return tr.text("mailbox-yesterday", None);
    }
    let mut args = i18n::args();
    args.set("weekday", tr.text(&format!("weekday-{}", at.weekday().to_monday_one_offset()), None));
    args.set("day", at.day());
    args.set("month", tr.text(&format!("month-{}", at.month()), None));
    args.set("year", at.year().to_string());
    tr.text(if date.year() == today.year() { "mailbox-day" } else { "mailbox-day-year" }, Some(&args))
}

/// Each address's letter, as its mark shows it: in priority order, the first
/// free one of its local part's initial, its domain's initial, then the local
/// part's next letters, so that no two marks read alike to one who cannot
/// tell their tints apart; the initial again when none is free.
fn letters(accounts: &[&Account]) -> Vec<String> {
    let mut taken: Vec<String> = Vec::new();
    for account in accounts {
        let name = account.address.as_deref().unwrap_or(&account.id);
        let (local, domain) = name.split_once('@').unwrap_or((name, ""));
        let upper = |c: char| c.to_uppercase().collect::<String>();
        let mut local_letters = local.chars().filter(|c| c.is_alphanumeric()).map(upper);
        let initial = local_letters.next();
        let candidates: Vec<String> = initial.clone().into_iter().chain(domain.chars().find(|c| c.is_alphanumeric()).map(upper)).chain(local_letters).collect();
        let letter = candidates.iter().find(|c| !taken.contains(c)).cloned().or(initial).unwrap_or_else(|| "·".to_string());
        taken.push(letter);
    }
    taken
}

/// One page of the mailbox (the module's rules): `addresses` ranked, each with
/// its folders, tint and rest; `hidden`, the files waiting for their Undo;
/// `now` in Unix seconds, the days read on `zone`.
pub fn page(addresses: &[Address], world: &World, ask: &Ask, hidden: &BTreeSet<PathBuf>, tr: &Translator, now: i64, zone: &jiff::tz::TimeZone) -> Page {
    let several = addresses.len() > 1;
    let letters = letters(&addresses.iter().map(|a| &a.account).collect::<Vec<_>>());
    let mark = |i: usize| Mark { account: addresses[i].account.id.clone(), letter: letters[i].clone(), tint: addresses[i].tint, title: addresses[i].account.address.clone().unwrap_or_else(|| addresses[i].account.id.clone()) };
    // Addresses whose server keeps labels (Gmail: an All Mail folder): one message in several folders.
    let labelled: Vec<bool> = addresses.iter().map(|a| a.folders.iter().any(|f| f.role == Role::All)).collect();
    let own: Vec<String> = addresses.iter().filter_map(|a| a.account.address.as_deref().map(str::to_ascii_lowercase)).collect();
    let query = ask.query.trim().to_lowercase();
    let since = now - WINDOW_DAYS * 86_400 * i64::from(ask.windows.max(1));
    let mut older = false;
    // Every copy in view, by its address.
    let mut by_address: BTreeMap<String, Vec<Copy>> = BTreeMap::new();
    for (i, address) in addresses.iter().enumerate() {
        let root = address.account.maildir_path();
        // The trash read only where labels make one message of several folders' copies.
        for folder in address.folders.iter().filter(|f| f.role != Role::Drafts && (f.role != Role::Trash || labelled[i]) && kept(f, &address.folders, &address.account)) {
            for (path, line) in mailcache::folder(&address.account.id, &root, &folder.local) {
                let sent = sent_at(&line);
                if query.is_empty() && sent < since {
                    older |= folder.role != Role::Trash;
                    continue;
                }
                if !found(&line, &query) {
                    continue;
                }
                let Some(uri) = crate::mailfind::address_of_line(&address.account.id, &line) else { continue };
                let flags = maildir::flags_of(&path);
                let hidden = hidden.contains(&path);
                by_address.entry(uri).or_default().push(Copy { path, line, address: i, folder: folder.name.clone(), role: folder.role, flags, sent, hidden });
            }
        }
    }
    let today = jiff::Timestamp::from_second(now).map(|t| t.to_zoned(zone.clone()).date()).unwrap_or_default();
    // What each message is, cheaply, for all; then what it shows, for the rows kept.
    let mut listed: Vec<Listed> = Vec::new();
    for (uri, mut copies) in by_address {
        // What waits for its Undo leaves; where labels make one message (Gmail), a
        // copy waiting, in the trash or in the junk is where the message is: that
        // address's other copies are its labels' old copies until their next round.
        let moved_off: BTreeSet<usize> = copies.iter().filter(|c| labelled[c.address] && (c.hidden || matches!(c.role, Role::Trash | Role::Junk))).map(|c| c.address).collect();
        copies.retain(|c| !c.hidden && c.role != Role::Trash && (c.role == Role::Junk || !moved_off.contains(&c.address)));
        if copies.is_empty() {
            continue;
        }
        // The copy that ranks first opens: the inbox, your folders, the archive, Sent, the junk, All Mail; the first to arrive.
        copies.sort_by(|a, b| (a.role.rank(), a.line.arrived, &a.path).cmp(&(b.role.rank(), b.line.arrived, &b.path)));
        let first = &copies[0];
        let address = &addresses[first.address];
        let mut card = first.line.card(&first.path);
        card.account = Some(address.account.id.clone());
        // Blocked senders: never listed.
        if world.senders.who_of(&card) == Who::Blocked {
            continue;
        }
        let from_own = card.from_address.as_deref().is_some_and(|f| own.contains(&f.trim().to_ascii_lowercase()));
        let sent_copy = |c: &Copy| c.role == Role::Sent || (c.role == Role::All && from_own);
        // What you said wins: not spam, even where it lies in a Junk folder.
        let said_ham = copies.iter().any(|c| c.flags.contains(maildir::NOT_JUNK)) || labels::said_ham_in(&world.said, &card);
        let received_copy = |c: &Copy| (matches!(c.role, Role::Inbox | Role::Other | Role::Archive | Role::All) || (c.role == Role::Junk && said_ham)) && !sent_copy(c);
        let stranger = !knows(world, &card);
        let spam_word = |c: &Copy| -> Option<String> {
            if said_ham {
                return None;
            }
            let id = c.line.message_id.as_deref().map(crate::mailindex::bare_id);
            let account = &addresses[c.address].account.id;
            let class_word = |class: crate::spam::Class| tr.text(if class == crate::spam::Class::Spam { "spam-chip-spam" } else { "spam-chip-unsure" }, None);
            if c.role == Role::Junk {
                let moved = world.moved.iter().rev().find(|m| m.account == *account && id.is_some() && m.message_id.as_deref() == id.as_deref());
                return Some(moved.map_or_else(|| tr.text("mailbox-spam-junk", None), |m| class_word(m.class)));
            }
            // Your filter flagged it where it lies: its log names its place.
            if let Some(origin) = maildir::origin_of(&c.path)
                && let Some(flagged) = world.flagged.iter().rev().find(|f| f.account == *account && f.folder == c.folder && f.uidvalidity == origin.validity && f.uid == origin.uid)
            {
                return Some(class_word(flagged.class));
            }
            // Your provider's word counts for a stranger's mail only.
            (c.line.spam && stranger && !from_own).then(|| tr.text("mailbox-spam-provider", None))
        };
        let spam_words: Vec<Option<String>> = copies.iter().map(|c| spam_word(c)).collect();
        let sent_kind = copies.iter().any(sent_copy);
        let received_kind = copies.iter().zip(&spam_words).any(|(c, s)| received_copy(c) && s.is_none());
        let spam = (!sent_kind && !received_kind).then(|| spam_words.iter().flatten().next().cloned()).flatten();
        let Some(kinds) = (match (&spam, sent_kind, received_kind) {
            (Some(_), _, _) => Some(vec!["spam"]),
            (None, true, true) => Some(vec!["received", "sent"]),
            (None, true, false) => Some(vec!["sent"]),
            (None, false, true) => Some(vec!["received"]),
            (None, false, false) => None,
        }) else {
            continue;
        };
        // Its addresses, ranked; a resting one's rows leave the list, unless another holds it or you asked.
        let mut held: Vec<usize> = copies.iter().map(|c| c.address).collect::<BTreeSet<usize>>().into_iter().collect();
        held.sort_unstable();
        let rests = held.iter().all(|i| addresses[*i].resting.is_some() && !ask.shown_resting.contains(&addresses[*i].account.id));
        if rests || (!ask.account.is_empty() && !held.iter().any(|i| addresses[*i].account.id == ask.account)) {
            continue;
        }
        // What the row's actions act on: each address's first-ranked copy (`Row::keys`).
        let mut acting: Vec<usize> = Vec::new();
        for (n, c) in copies.iter().enumerate() {
            if !acting.iter().any(|&k| copies[k].address == c.address) {
                acting.push(n);
            }
        }
        // Read once a received copy it acts on is read (Gmail's flags are the message's,
        // its All Mail copy's letters old until that folder's next round).
        let acting_received: Vec<&Copy> = acting.iter().map(|&k| &copies[k]).filter(|c| received_copy(c)).collect();
        let read = if !acting_received.is_empty() {
            acting_received.iter().any(|c| c.flags.contains('S'))
        } else if received_kind {
            copies.iter().any(|c| received_copy(c) && c.flags.contains('S'))
        } else {
            first.flags.contains('S')
        };
        let unread_here = received_kind && !read;
        let shown = match ask.chip {
            Chip::All => spam.is_none(),
            Chip::Unread => spam.is_none() && unread_here,
            Chip::Sent => sent_kind,
            Chip::Spam => spam.is_some(),
        };
        if !shown {
            continue;
        }
        let sent = copies.iter().map(|c| c.sent).min().unwrap_or(first.sent);
        listed.push(Listed { uri, card, held, kinds, spam, sent_kind, received_kind, unread_here, read, acting, from_own, stranger, sent, copies });
    }
    // Newest first; a page of rows a window at most, "Earlier messages" showing more.
    listed.sort_by(|a, b| b.sent.cmp(&a.sent).then_with(|| a.copies[0].path.as_os_str().cmp(b.copies[0].path.as_os_str())));
    let more = listed.len() > PAGE_ROWS * ask.windows.max(1) as usize;
    listed.truncate(PAGE_ROWS * ask.windows.max(1) as usize);
    let mut rows = Vec::with_capacity(listed.len());
    for Listed { uri, card, held, kinds, spam, sent_kind, received_kind, unread_here, read, acting, from_own, stranger, sent, copies } in listed {
        let first = &copies[0];
        let address = &addresses[first.address];
        let outgoing = !received_kind && sent_kind;
        let item = crate::view::mail_item(&card, if outgoing { Role::Sent } else { first.role }, false, &first.flags, &address.trusted, &world.words, tr);
        let mut who = item.who.clone();
        if received_kind && sent_kind && from_own {
            let mut args = i18n::args();
            args.set("names", if card.to.is_empty() { tr.text("mail-nobody", None) } else { card.to.join(", ") });
            who = tr.text("mail-you-to", Some(&args));
        }
        let forged = item.trust_level == "forged";
        let shielded = copies.iter().any(|c| addresses[c.address].account.shield);
        let hostile = shielded && received_kind && stranger && !from_own && copies.iter().find(|c| addresses[c.address].account.shield).is_some_and(|c| hostile(&addresses[c.address].account.id, &c.path, world));
        // The dot: received, not read, and the matrix shows it now (decision 12).
        let dot = unread_here && spam.is_none() && !forged && !hostile && {
            let unproven = !matches!(trust::judge_sender(trust::read_auth_results(&card.headers, &address.trusted).as_ref(), card.is_list, card.sender_domain()).0, Trust::Verified);
            let who = if unproven { Who::Stranger } else { world.senders.who_of(&card) };
            let always = !unproven && card.from_address.as_deref().is_some_and(|a| world.always.admits_address(a));
            let area = address.account.area.as_deref().and_then(crate::areas::Area::parse).unwrap_or(crate::areas::Area::WORK);
            world.attention.mail_from(who, always, from_own, area, &world.now).shown
        };
        let at = jiff::Timestamp::from_second(first.sent).map(|t| t.to_zoned(zone.clone())).unwrap_or_else(|_| jiff::Timestamp::UNIX_EPOCH.to_zoned(zone.clone()));
        let mut subject = item.subject.clone();
        if hostile {
            let mut args = i18n::args();
            who = card.sender_domain().map_or_else(
                || tr.text("hostile-someone", None),
                |d| {
                    args.set("domain", d.to_string());
                    tr.text("hostile-someone-at", Some(&args))
                },
            );
            subject = tr.text("hostile-subject", None);
        }
        rows.push(Row {
            key: first.path.display().to_string(),
            keys: acting.iter().map(|&k| copies[k].path.display().to_string()).collect(),
            uri,
            account: address.account.id.clone(),
            folder: first.folder.clone(),
            role: first.role,
            who,
            address: if hostile { String::new() } else { item.address.clone() },
            subject,
            day: day_title(tr, &at, today),
            time: at.strftime("%H:%M").to_string(),
            unread: dot,
            read,
            // As read: the copies it acts on.
            flagged: acting.iter().any(|&k| copies[k].flags.contains('F')),
            answered: copies.iter().any(|c| c.flags.contains('R')),
            attachments: item.attachments,
            trust_level: item.trust_level,
            // Its trust in words: forged mail's mark, and the reader's sender line.
            trust: item.trust.clone(),
            spam: spam.clone().unwrap_or_default(),
            hostile,
            marks: if several { held.iter().map(|i| mark(*i)).collect() } else { Vec::new() },
            kinds,
            server: false,
            sent,
        });
    }
    let sentence = if !rows.is_empty() {
        String::new()
    } else if !query.is_empty() {
        let mut args = i18n::args();
        args.set("query", ask.query.trim().to_string());
        tr.text("mail-not-found", Some(&args))
    } else if older {
        tr.text("mail-nothing-recent", None)
    } else {
        tr.text("mail-empty", None)
    };
    Page {
        rows,
        resting: addresses.iter().filter(|a| !ask.shown_resting.contains(&a.account.id)).filter_map(|a| Some(Resting { account: a.account.id.clone(), line: a.resting.clone()? })).collect(),
        addresses: if several { (0..addresses.len()).map(mark).collect() } else { Vec::new() },
        earlier: more || (query.is_empty() && older),
        sentence,
        chip: ask.chip,
        account: ask.account.clone(),
        windows: ask.windows.max(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::ImapOrigin;
    use crate::folders::folder;

    /// Now, for the stand-ins: Monday 2 November 2026, 14:00 in Paris.
    const NOW: i64 = 1_793_624_400;

    /// Two stand-in addresses, under the tests' own data folder: work's
    /// (inbox, Sent, Junk, an archive, Drafts, the trash) and home's, a
    /// Gmail one (inbox, All Mail, Sent Mail, Spam), shielded.
    struct Stand {
        addresses: Vec<Address>,
        base: PathBuf,
    }

    impl Stand {
        fn new(name: &str) -> Stand {
            crate::config::stand_ins_only();
            let base = crate::config::data_dir().join(format!("mailbox-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let made = |id: &str, address: &str, area: &str, folders: Vec<Folder>, tint: u8| {
                let id = format!("{id}-{name}-{}", std::process::id());
                let mut account = Account::imap(&id, address, "imap.example.org", 993, crate::config::Security::Tls, None);
                account.maildir = Some(base.join(&id).display().to_string());
                account.area = Some(area.into());
                for f in &folders {
                    for sub in ["tmp", "new", "cur"] {
                        std::fs::create_dir_all(account.maildir_path().join(&f.local).join(sub)).unwrap();
                    }
                }
                Address { account, folders, trusted: vec!["mx.example.org".into()], tint, resting: None }
            };
            let work = made(
                "work",
                "noa@work.example.org",
                "work",
                vec![folder("INBOX", Some("/"), Some(Role::Inbox)), folder("Sent", Some("/"), Some(Role::Sent)), folder("Junk", Some("/"), Some(Role::Junk)), folder("Archive", Some("/"), Some(Role::Archive)), folder("Drafts", Some("/"), Some(Role::Drafts)), folder("Trash", Some("/"), Some(Role::Trash))],
                0,
            );
            let mut home = made(
                "home",
                "noa@home.example.org",
                "admin+leisure",
                vec![folder("INBOX", Some("/"), Some(Role::Inbox)), folder("[Gmail]/All Mail", Some("/"), Some(Role::All)), folder("[Gmail]/Sent Mail", Some("/"), Some(Role::Sent)), folder("[Gmail]/Spam", Some("/"), Some(Role::Junk)), folder("[Gmail]/Trash", Some("/"), Some(Role::Trash))],
                1,
            );
            home.account.shield = true;
            Stand { addresses: vec![work, home], base }
        }

        fn dir(&self, address: usize, folder: &str) -> PathBuf {
            let a = &self.addresses[address];
            a.account.maildir_path().join(&a.folders.iter().find(|f| f.name == folder).unwrap().local)
        }

        /// A message `id` from `from` to `to`, dated `date`, arrived `arrived`,
        /// in `address`'s `folder` at `uid` with `flags`; `extra` headers, `text`.
        #[allow(clippy::too_many_arguments)]
        fn put(&self, address: usize, folder: &str, uid: u32, id: &str, from: &str, to: &str, subject: &str, date: i64, arrived: i64, flags: &str, extra: &str, text: &str) -> PathBuf {
            let dir = self.dir(address, folder);
            let name = format!("{arrived}.U7-{uid}.sioul");
            let path = if flags.is_empty() { dir.join("new").join(name) } else { dir.join("cur").join(format!("{name}{}2,{flags}", maildir::INFO)) };
            let date = jiff::Timestamp::from_second(date).unwrap().strftime("%a, %d %b %Y %H:%M:%S +0000").to_string();
            std::fs::write(&path, format!("Message-ID: <{id}>\r\nFrom: {from}\r\nTo: {to}\r\nDate: {date}\r\nSubject: {subject}\r\nAuthentication-Results: mx.example.org; spf=pass smtp.mailfrom=example.org; dkim=pass header.d=example.org; dmarc=pass header.from=example.org\r\n{extra}\r\n{text}\r\n")).unwrap();
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(arrived as u64)).unwrap();
            path
        }

        fn page(&self, world: &World, ask: &Ask, hidden: &BTreeSet<PathBuf>) -> Page {
            page(&self.addresses, world, ask, hidden, &Translator::new("en"), NOW, &jiff::tz::TimeZone::get("Europe/Paris").unwrap())
        }
    }

    impl Drop for Stand {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
            for a in &self.addresses {
                if let Some(dir) = mailcache::account_dir(&a.account.id) {
                    let _ = std::fs::remove_dir_all(dir);
                }
            }
        }
    }

    /// A world at work time (every cell shows), with these lists.
    fn world_at(time: crate::areas::Time, blocked: &str, safe: &str) -> World {
        let mode = crate::quiet::Mode { quiet: time != crate::areas::Time::Work, time, week: crate::areas::Week::default(), reason: crate::quiet::Reason::Working, until: None, back: None, label: String::new() };
        let senders = Senders { blocked: SenderList::parse(blocked), safe: SenderList::parse(safe), ..Senders::default() };
        World {
            senders,
            known: SenderList::default(),
            always: crate::everywhere::People::default(),
            attention: crate::attention::Attention::of(&crate::config::Config::default()),
            now: crate::attention::Now::of(&mode),
            said: Vec::new(),
            flagged: Vec::new(),
            moved: Vec::new(),
            assessments: BTreeMap::new(),
            words: crate::words::current(),
        }
    }

    fn subjects(page: &Page) -> Vec<&str> {
        page.rows.iter().map(|r| r.subject.as_str()).collect()
    }

    fn row<'a>(page: &'a Page, subject: &str) -> &'a Row {
        page.rows.iter().find(|r| r.subject == subject).unwrap_or_else(|| panic!("{subject}: {:?}", subjects(page)))
    }

    /// Newest first by when sent, never later than it arrived (a message
    /// dated ahead takes its arrival's place); the copies of one message one
    /// row with every kind and address (a list to both addresses, a message
    /// sent to yourself, Gmail's All Mail), the inbox's copy opening; drafts
    /// and the trash never; days as headers.
    #[test]
    fn one_row_per_message_by_when_it_was_sent() {
        let stand = Stand::new("rows");
        let hour = 3600;
        stand.put(0, "INBOX", 1, "list@example.org", "List <news@list.example.org>", "noa@work.example.org", "On both lists", NOW - 5 * hour, NOW - 5 * hour, "S", "List-Id: <news.list.example.org>\r\n", "Hello.");
        stand.put(1, "INBOX", 1, "list@example.org", "List <news@list.example.org>", "noa@home.example.org", "On both lists", NOW - 5 * hour, NOW - 5 * hour + 60, "S", "List-Id: <news.list.example.org>\r\n", "Hello.");
        stand.put(0, "Sent", 2, "self@example.org", "Noa <noa@work.example.org>", "Noa <noa@work.example.org>", "A note to myself", NOW - 3 * hour, NOW - 3 * hour, "S", "", "Milk.");
        stand.put(0, "INBOX", 3, "self@example.org", "Noa <noa@work.example.org>", "Noa <noa@work.example.org>", "A note to myself", NOW - 3 * hour, NOW - 3 * hour + 5, "", "", "Milk.");
        stand.put(0, "INBOX", 4, "ahead@example.org", "Ana <ana@example.org>", "noa@work.example.org", "Dated ahead", NOW + 3 * 86_400, NOW - 4 * hour, "", "", "Soon.");
        let inbox = stand.put(1, "INBOX", 5, "gmail@example.org", "Ben <ben@example.org>", "noa@home.example.org", "Kept by Gmail twice", NOW - 2 * hour, NOW - 2 * hour, "S", "", "Hi.");
        stand.put(1, "[Gmail]/All Mail", 9, "gmail@example.org", "Ben <ben@example.org>", "noa@home.example.org", "Kept by Gmail twice", NOW - 2 * hour, NOW - 2 * hour - 30, "S", "", "Hi.");
        stand.put(0, "Drafts", 6, "draft@example.org", "Noa <noa@work.example.org>", "x@example.org", "A draft", NOW - hour, NOW - hour, "S", "", "…");
        stand.put(0, "Trash", 7, "trash@example.org", "Cat <cat@example.org>", "noa@work.example.org", "Thrown away", NOW - hour, NOW - hour, "S", "", "…");
        stand.put(0, "INBOX", 8, "yesterday@example.org", "Cat <cat@example.org>", "noa@work.example.org", "From yesterday", NOW - 26 * hour, NOW - 26 * hour, "S", "", "…");
        let page = stand.page(&world_at(crate::areas::Time::Work, "", ""), &Ask::default(), &BTreeSet::new());
        assert_eq!(subjects(&page), ["Kept by Gmail twice", "A note to myself", "Dated ahead", "On both lists", "From yesterday"]);
        let list = row(&page, "On both lists");
        assert_eq!(list.marks.iter().map(|m| m.account.as_str()).collect::<Vec<_>>(), [stand.addresses[0].account.id.as_str(), stand.addresses[1].account.id.as_str()]);
        assert_eq!((list.marks[0].letter.as_str(), list.marks[1].letter.as_str(), list.marks[1].tint), ("N", "H", 1), "two noa@ addresses: two letters");
        assert!(page.rows.iter().all(|r| !r.trust.is_empty()), "the reader's sender line: its trust in words");
        let own = row(&page, "A note to myself");
        assert_eq!((own.kinds.clone(), own.who.as_str()), (vec!["received", "sent"], "You, to Noa"));
        let gmail = row(&page, "Kept by Gmail twice");
        assert_eq!((gmail.key.as_str(), gmail.role, gmail.marks.len()), (inbox.to_str().unwrap(), Role::Inbox, 1));
        assert_eq!((row(&page, "Dated ahead").day.as_str(), row(&page, "Dated ahead").time.as_str()), ("Today", "10:00"));
        assert_eq!(row(&page, "From yesterday").day, "Yesterday");
        // One address alone: no mark at all.
        let alone = page_of(&stand.addresses[..1], &world_at(crate::areas::Time::Work, "", ""), &Ask::default());
        assert!(alone.rows.iter().all(|r| r.marks.is_empty()) && alone.addresses.is_empty());
        // Sent alone, under its chip.
        let sent = stand.page(&world_at(crate::areas::Time::Work, "", ""), &Ask { chip: Chip::Sent, ..Ask::default() }, &BTreeSet::new());
        assert_eq!(subjects(&sent), ["A note to myself"]);
        // One address alone: what it holds, the list to both included.
        let work = stand.page(&world_at(crate::areas::Time::Work, "", ""), &Ask { account: stand.addresses[0].account.id.clone(), ..Ask::default() }, &BTreeSet::new());
        assert_eq!(subjects(&work), ["A note to myself", "Dated ahead", "On both lists", "From yesterday"]);
    }

    /// A row acts on each address's first-ranked copy, and reads as those
    /// copies are (review of 863e4c4, M1): Gmail's inbox copy read, its All
    /// Mail copy not yet, reads as read; one key per address; a delete
    /// waiting for its Undo, or done, leaves no old copy of a Gmail message
    /// carrying the row; a message in two addresses goes when both copies go.
    #[test]
    fn a_row_acts_on_each_address_and_reads_as_its_copies() {
        let stand = Stand::new("acting");
        let at = NOW - 3600;
        // A safe sender: its dot shows now.
        let world = world_at(crate::areas::Time::Work, "", "ben@example.org");
        let inbox = stand.put(1, "INBOX", 5, "gmail@example.org", "Ben <ben@example.org>", "noa@home.example.org", "Kept by Gmail twice", at, at, "", "", "Hi.");
        let all = stand.put(1, "[Gmail]/All Mail", 9, "gmail@example.org", "Ben <ben@example.org>", "noa@home.example.org", "Kept by Gmail twice", at, at - 30, "F", "", "Hi.");
        let work = stand.put(0, "INBOX", 1, "list@example.org", "List <news@list.example.org>", "noa@work.example.org", "On both lists", at - 600, at - 600, "", "List-Id: <news.list.example.org>\r\n", "Hello.");
        let home = stand.put(1, "INBOX", 1, "list@example.org", "List <news@list.example.org>", "noa@home.example.org", "On both lists", at - 600, at - 540, "", "List-Id: <news.list.example.org>\r\n", "Hello.");
        let none = BTreeSet::new();
        let page = stand.page(&world, &Ask::default(), &none);
        let gmail = row(&page, "Kept by Gmail twice");
        // One key for Gmail's two copies: a delete acts once.
        assert_eq!(gmail.keys, [inbox.display().to_string()]);
        assert!(gmail.unread && !gmail.read, "unread, both copies");
        assert!(!gmail.flagged, "flagged as its inbox copy is, not as All Mail's old letters");
        assert_eq!(row(&page, "On both lists").keys, [work.display().to_string(), home.display().to_string()]);
        // Opening marks the inbox copy read: the dot goes, All Mail's copy not read yet.
        let opened = maildir::set_flags(&inbox, "S").unwrap();
        let page = stand.page(&world, &Ask::default(), &none);
        let gmail = row(&page, "Kept by Gmail twice");
        assert!(!gmail.unread && gmail.read, "read once the copy it acts on is read");
        let unread = stand.page(&world, &Ask { chip: Chip::Unread, ..Ask::default() }, &none);
        assert_eq!(subjects(&unread), ["On both lists"]);
        // Deleted, waiting for its Undo: All Mail's copy does not carry the row.
        let waiting = stand.page(&world, &Ask::default(), &BTreeSet::from([opened.clone()]));
        assert_eq!(subjects(&waiting), ["On both lists"]);
        // Done: the file followed it into the trash; All Mail's copy still waits for its round.
        let trash = stand.dir(1, "[Gmail]/Trash").join("cur").join(opened.file_name().unwrap());
        std::fs::rename(&opened, &trash).unwrap();
        assert!(all.exists());
        assert_eq!(subjects(&stand.page(&world, &Ask::default(), &none)), ["On both lists"]);
        // In two addresses: one copy waiting, the other holds the row; both, it goes.
        let one = stand.page(&world, &Ask::default(), &BTreeSet::from([work.clone()]));
        assert_eq!(row(&one, "On both lists").keys, [home.display().to_string()]);
        let both = stand.page(&world, &Ask::default(), &BTreeSet::from([work, home]));
        assert!(both.rows.is_empty());
    }

    /// Gmail's junk: the message moved there, its All Mail copy not yet gone,
    /// is spam, under the Spam chip only.
    #[test]
    fn a_gmail_message_in_its_junk_is_spam() {
        let stand = Stand::new("labels-junk");
        let at = NOW - 3600;
        stand.put(1, "[Gmail]/Spam", 3, "junked@example.org", "Ads <ads@shop.example>", "noa@home.example.org", "Moved to junk", at, at, "S", "", "Buy.");
        stand.put(1, "[Gmail]/All Mail", 8, "junked@example.org", "Ads <ads@shop.example>", "noa@home.example.org", "Moved to junk", at, at, "S", "", "Buy.");
        let world = world_at(crate::areas::Time::Work, "", "");
        assert!(stand.page(&world, &Ask::default(), &BTreeSet::new()).rows.is_empty());
        assert_eq!(subjects(&stand.page(&world, &Ask { chip: Chip::Spam, ..Ask::default() }, &BTreeSet::new())), ["Moved to junk"]);
    }

    /// No two marks read alike: in priority order, the local part's initial,
    /// the domain's, then the local part's next letters (review of 863e4c4, L18).
    #[test]
    fn marks_never_repeat_a_letter() {
        let account = |address: &str| Account::imap(address, address, "imap.example.org", 993, crate::config::Security::Tls, None);
        let (a, b, c, d) = (account("noa@work.example.org"), account("noa@home.example.org"), account("noa@hub.example.org"), account("ana@example.org"));
        assert_eq!(letters(&[&a, &b, &c, &d]), ["N", "H", "O", "A"]);
        let (e, f) = (account("noa@example.com"), account("noa.ferrand@example.org"));
        assert_eq!(letters(&[&e, &f]), ["N", "E"]);
        // None free: the initial again.
        let (g, h) = (account("n@n.example"), account("n@n.example"));
        assert_eq!(letters(&[&g, &h]), ["N", "N"]);
    }

    /// Each address's colour: its own when chosen, else the palette's in the
    /// order of priority (as the caller ranks them); a colour out of the
    /// palette is not taken.
    #[test]
    fn colours_follow_priority_unless_chosen() {
        let account = |id: &str, colour: Option<u8>| Account { colour, ..Account::imap(id, &format!("{id}@example.org"), "imap.example.org", 993, crate::config::Security::Tls, None) };
        let (a, b, c, d) = (account("a", None), account("b", Some(6)), account("c", None), account("d", Some(9)));
        assert_eq!(tints(&[&a, &b, &c, &d]), [0, 6, 2, 3]);
    }

    fn page_of(addresses: &[Address], world: &World, ask: &Ask) -> Page {
        page(addresses, world, ask, &BTreeSet::new(), &Translator::new("en"), NOW, &jiff::tz::TimeZone::get("Europe/Paris").unwrap())
    }

    /// Spam from each source, under its chip with its word, never a dot: a
    /// Junk folder's copy, one your filter moved there (its class), one it
    /// flagged where it lies, your provider's flag on a stranger's mail; your
    /// provider's flag on someone you know is not spam; what you said wins.
    #[test]
    fn spam_from_each_source_and_not_spam_winning() {
        let stand = Stand::new("spam");
        let at = NOW - 3600;
        let provider = "X-Spam-Flag: YES\r\nReceived: from mail.sender.example (mail.sender.example [203.0.112.9]) by mx.provider.example with ESMTPS\r\n";
        stand.put(0, "Junk", 1, "junk@example.org", "Ads <ads@shop.example>", "noa@work.example.org", "In the junk", at, at, "", "", "Buy.");
        stand.put(0, "Junk", 2, "moved@example.org", "Ads <ads@shop.example>", "noa@work.example.org", "Moved by your filter", at, at, "", "", "Buy.");
        let flagged = stand.put(0, "INBOX", 3, "flagged@example.org", "Ads <ads@shop.example>", "noa@work.example.org", "Flagged where it lies", at, at, "", "", "Buy.");
        stand.put(0, "INBOX", 4, "provider@example.org", "Prize <prize@lottery.example>", "noa@work.example.org", "Your provider's word", at, at, "", provider, "Win.");
        stand.put(0, "INBOX", 5, "friend@example.org", "Ana <ana@example.org>", "noa@work.example.org", "A friend, flagged", at, at, "", provider, "Hello.");
        stand.put(0, "INBOX", 6, "said@example.org", "Prize <prize@lottery.example>", "noa@work.example.org", "Said not spam", at, at, "", provider, "Win.");
        stand.put(0, "Junk", 7, "notjunk@example.org", "Ads <ads@shop.example>", "noa@work.example.org", "Not junk in the junk", at, at, "b", "", "Hi.");
        let mut world = world_at(crate::areas::Time::Work, "", "ana@example.org");
        let account = stand.addresses[0].account.id.clone();
        let judged = |uid: u32, id: &str, class| labels::Moved { at, account: account.clone(), folder: "INBOX".into(), uidvalidity: 7, uid, message_id: Some(id.into()), class };
        world.moved.push(judged(2, "moved@example.org", crate::spam::Class::Unsure));
        let origin = maildir::origin_of(&flagged).unwrap();
        world.flagged.push(labels::Moved { uidvalidity: origin.validity, ..judged(3, "flagged@example.org", crate::spam::Class::Spam) });
        world.said.push(labels::Entry { at, account: account.clone(), folder: "INBOX".into(), uidvalidity: 7, uid: 6, message_id: Some("said@example.org".into()), label: labels::Label::Ham, source: labels::Source::NotSpam });
        let all = stand.page(&world, &Ask::default(), &BTreeSet::new());
        let mut shown = subjects(&all);
        shown.sort_unstable();
        assert_eq!(shown, ["A friend, flagged", "Not junk in the junk", "Said not spam"]);
        let spam = stand.page(&world, &Ask { chip: Chip::Spam, ..Ask::default() }, &BTreeSet::new());
        let words: BTreeMap<&str, &str> = spam.rows.iter().map(|r| (r.subject.as_str(), r.spam.as_str())).collect();
        assert_eq!(words, BTreeMap::from([("In the junk", "in the junk"), ("Moved by your filter", "maybe spam"), ("Flagged where it lies", "probably spam"), ("Your provider's word", "your provider: spam")]));
        assert!(spam.rows.iter().all(|r| !r.unread && r.kinds == ["spam"]));
    }

    /// Calm: a resting address's mail leaves the list, its line says so, and
    /// "Show its mail" brings it back; mail whose time has not come shows
    /// without its dot, a safe sender's with it; blocked senders and what
    /// waits for its Undo never; a shielded address's hostile mail without
    /// its words.
    #[test]
    fn calm_rest_dots_blocked_undo_and_hostile() {
        let mut stand = Stand::new("calm");
        let at = NOW - 3600;
        stand.put(0, "INBOX", 1, "work@example.org", "Iris <iris@client.example>", "noa@work.example.org", "Work mail", at, at, "", "", "Plans.");
        stand.put(1, "INBOX", 2, "stranger@example.org", "Someone <someone@unknown.example>", "noa@home.example.org", "A stranger's", at, at, "", "", "Hello.");
        stand.put(1, "INBOX", 3, "safe@example.org", "Ana <ana@example.org>", "noa@home.example.org", "A safe sender's", at, at, "", "", "Hello.");
        stand.put(1, "INBOX", 4, "blocked@example.org", "Spammer <x@bad.example>", "noa@home.example.org", "From a blocked sender", at, at, "", "", "Buy.");
        let waiting = stand.put(1, "INBOX", 5, "undo@example.org", "Ana <ana@example.org>", "noa@home.example.org", "Waiting for its Undo", at, at, "", "", "Hi.");
        stand.put(1, "INBOX", 6, "hostile@example.org", "Troll <troll@hate.example>", "noa@home.example.org", "You again", at, at, "", "", "I know where you live.");
        stand.addresses[0].resting = Some("noa@work.example.org rests until 09:00.".into());
        let world = world_at(crate::areas::Time::Leisure, "x@bad.example", "ana@example.org");
        let page = stand.page(&world, &Ask::default(), &BTreeSet::from([waiting]));
        assert_eq!(page.resting.iter().map(|r| r.line.as_str()).collect::<Vec<_>>(), ["noa@work.example.org rests until 09:00."]);
        assert!(!subjects(&page).contains(&"Work mail") && !subjects(&page).contains(&"From a blocked sender") && !subjects(&page).contains(&"Waiting for its Undo"), "{:?}", subjects(&page));
        assert!(!row(&page, "A stranger's").unread, "its time has not come: no dot");
        assert!(row(&page, "A safe sender's").unread);
        let hostile = page.rows.iter().find(|r| r.hostile).expect("hostile");
        assert_eq!((hostile.who.as_str(), hostile.unread, hostile.address.as_str()), ("Someone at hate.example", false, ""));
        assert_ne!(hostile.subject, "You again");
        let shown = stand.page(&world, &Ask { shown_resting: vec![stand.addresses[0].account.id.clone()], ..Ask::default() }, &BTreeSet::new());
        assert!(subjects(&shown).contains(&"Work mail") && shown.resting.is_empty());
        // One address alone, chosen: the other's mail out.
        let one = stand.page(&world, &Ask { account: stand.addresses[1].account.id.clone(), ..Ask::default() }, &BTreeSet::new());
        assert!(one.rows.iter().all(|r| r.marks.iter().any(|m| m.account == stand.addresses[1].account.id)));
    }

    /// Two weeks, then two more at each "Earlier messages"; the quick search
    /// looks through everything kept.
    #[test]
    fn two_weeks_then_earlier() {
        let stand = Stand::new("weeks");
        let day = 86_400;
        stand.put(0, "INBOX", 1, "recent@example.org", "Ana <ana@example.org>", "noa@work.example.org", "Recent", NOW - 3 * day, NOW - 3 * day, "S", "", "…");
        stand.put(0, "Archive", 2, "older@example.org", "Ben <ben@example.org>", "noa@work.example.org", "Three weeks ago", NOW - 21 * day, NOW - 21 * day, "S", "", "…");
        stand.put(0, "Archive", 3, "oldest@example.org", "Ben <ben@example.org>", "noa@work.example.org", "Five weeks ago", NOW - 35 * day, NOW - 35 * day, "S", "", "…");
        let world = world_at(crate::areas::Time::Work, "", "");
        let first = stand.page(&world, &Ask::default(), &BTreeSet::new());
        assert_eq!((subjects(&first), first.earlier), (vec!["Recent"], true));
        let second = stand.page(&world, &Ask { windows: 2, ..Ask::default() }, &BTreeSet::new());
        assert_eq!((subjects(&second), second.earlier), (vec!["Recent", "Three weeks ago"], true));
        let third = stand.page(&world, &Ask { windows: 3, ..Ask::default() }, &BTreeSet::new());
        assert_eq!((subjects(&third), third.earlier), (vec!["Recent", "Three weeks ago", "Five weeks ago"], false));
        let found = stand.page(&world, &Ask { query: "FIVE".into(), ..Ask::default() }, &BTreeSet::new());
        assert_eq!(subjects(&found), ["Five weeks ago"]);
        let _ = ImapOrigin { validity: 7, uid: 1 };
    }

    /// A crowded fortnight: a page of rows at most, the newest, and "Earlier
    /// messages" showing the next; a search is cut the same way.
    #[test]
    fn a_crowded_fortnight_is_cut_to_a_page() {
        let stand = Stand::new("crowded");
        let n = PAGE_ROWS + 3;
        for i in 0..n {
            let at = NOW - 3_600 - i as i64 * 60;
            stand.put(0, "INBOX", i as u32 + 1, &format!("m{i}@example.org"), "Ana <ana@example.org>", "noa@work.example.org", &format!("Note {i}"), at, at, "S", "", "…");
        }
        let world = world_at(crate::areas::Time::Work, "", "");
        let first = stand.page(&world, &Ask::default(), &BTreeSet::new());
        assert_eq!((first.rows.len(), first.earlier), (PAGE_ROWS, true));
        assert_eq!((first.rows[0].subject.as_str(), first.rows[PAGE_ROWS - 1].subject.clone()), ("Note 0", format!("Note {}", PAGE_ROWS - 1)), "the newest kept");
        let second = stand.page(&world, &Ask { windows: 2, ..Ask::default() }, &BTreeSet::new());
        assert_eq!((second.rows.len(), second.earlier), (n, false));
        let found = stand.page(&world, &Ask { query: "note".into(), ..Ask::default() }, &BTreeSet::new());
        assert_eq!((found.rows.len(), found.earlier), (PAGE_ROWS, true));
    }
}

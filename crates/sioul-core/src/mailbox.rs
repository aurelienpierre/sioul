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
//!   arrived (`porch::sent`), newest first; days as headers. The window's
//!   list has no horizon (8 November 2026, in the place of decision 15's two
//!   weeks and "Earlier messages"): everything this device keeps, read back
//!   in two-week windows, twice as many each time, until the rows asked for
//!   are filled (`Ask::windows` at 0); an agent asks its own days. The rows'
//!   words are made for the rows asked for alone (`Ask::rows`, sixty at a
//!   time: `PAGE`), after every message of the windows read was sorted.
//! - **Read from** each folder's recent lines kept in this process
//!   (`mailcache::recent`): nothing listed or read again while the folder is
//!   as it was. A folder looked at for the first time has its newest
//!   `FIRST_FILL` messages read at once; the others wait, `STEP_FILL` a look
//!   (`Page::indexing`), for the window to ask again while the page shows.
//! - **Spam** (§5.2, 3): a copy in a Junk folder; one your own filter flagged
//!   where it lies (its log); your provider's flag on a stranger's mail; what
//!   you said is not spam (`$NotJunk`, the label log) wins. Spam rows show
//!   under the Spam chip only (decision 7), with their quiet word. Proven
//!   mail from someone you deal with (`spam::vouched`, part 2 of 4 November
//!   2026) is never spam on your provider's word: flagged, it shows with
//!   "Your provider marked it as spam."; in its Junk folder, unless the
//!   matrix's row says to do nothing, you said anything of it or it was
//!   brought back once (it lies there again by your doing), it shows among
//!   your mail with "Your provider put it in Spam.". A Subject whose tag
//!   Sioul's check took off (`trust::restored_tags`) says the first.
//! - **Calm** (§4.7): the window's filter (`Filter`, docs/areas.md, "A
//!   page's filter by area") shows the addresses whose area is ticked, as
//!   the hours say unless you ticked others (`hours_say`, `areafilter`),
//!   and that are on; without it (an agent's), a resting address's rows
//!   leave the list, one line saying so. Mail whose time has not come (its
//!   cell ○ Later) shows without its dot (decision 12); spam, forged and
//!   hostile mail never have one; nothing is counted (decision 13).
//! - **Your word at once**: what you said of a message (Spam, Not spam)
//!   while it waits for its Undo counts as said already (`World::waiting`):
//!   its row changes kind at once.
//! - **Topics** (docs/topics.md): each row's topic, one at most, and why
//!   (`topics::rules`: your word, your rule for its sender, its
//!   conversation, the topic's seeds), its sender counting only when Sioul's
//!   checks authenticate it; `Ask::topic` keeps a topic's mail alone.

use crate::areas::Area;
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

/// Days of one window: the weeks read at a time, two.
pub const WINDOW_DAYS: i64 = 14;

/// The most windows a list without horizon reads back (`Ask::windows` at 0): 4096 times two weeks, every message kept.
pub const MOST_WINDOWS: u32 = 4096;

/// Rows of one page: what one publish carries to the window (§6).
pub const PAGE: usize = 60;

/// Messages a folder looked at for the first time has read at once: what the first page needs (§6).
pub const FIRST_FILL: usize = 500;

/// Messages read at each later look while some wait, over every folder.
pub const STEP_FILL: usize = 500;

/// What the list shows: one chip at a time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chip {
    /// What you received (8 November 2026: what you sent alone is under
    /// Sent, never mixed in; a message you sent and also received stays).
    #[default]
    All,
    /// What you received and have not read.
    Unread,
    Sent,
    Spam,
    /// What you received and what you sent alike: an agent's (`mailbox`
    /// without a kind), never the window's.
    Every,
}

impl Chip {
    pub fn parse(text: &str) -> Chip {
        match text {
            "unread" => Chip::Unread,
            "sent" => Chip::Sent,
            "spam" => Chip::Spam,
            "every" => Chip::Every,
            _ => Chip::All,
        }
    }
}

fn one() -> u32 {
    1
}

fn a_page() -> usize {
    PAGE
}

/// What the page asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    #[serde(default)]
    pub chip: Chip,
    /// One address only (its id); empty: every address.
    #[serde(default)]
    pub account: String,
    /// How many two-week windows back (an agent's days); 0: as many as the
    /// rows asked for need, every message kept at most (the window's list:
    /// no horizon), read twice as far back each time (`page_with`).
    #[serde(default = "one")]
    pub windows: u32,
    /// The quick search's words: everything kept, by sender, recipients and subject.
    #[serde(default)]
    pub query: String,
    /// Resting addresses you chose to see all the same ("Show its mail").
    #[serde(default)]
    pub shown_resting: Vec<String>,
    /// How many rows the list holds or wants: a page, then one more each time its end shows.
    #[serde(default = "a_page")]
    pub rows: usize,
    /// The list holds nothing yet: every page is new to it (the window's, `mail::MailboxPages`).
    #[serde(default)]
    pub fresh: bool,
    /// The window's filter, its areas and addresses (`Filter`): then no address
    /// rests and `shown_resting` is not read. None: an agent's, as before.
    #[serde(default)]
    pub filter: Option<Filter>,
    /// One topic's mail alone, by its id; empty: every message.
    #[serde(default)]
    pub topic: String,
}

impl Default for Ask {
    fn default() -> Ask {
        Ask { chip: Chip::All, account: String::new(), windows: 1, query: String::new(), shown_resting: Vec::new(), rows: PAGE, fresh: false, filter: None, topic: String::new() }
    }
}

/// Mail's filter, the window's alone (an agent's `mailbox` keeps its own
/// rules): the areas ticked, as the hours say unless you ticked others
/// (`hours_say`, `areafilter::AreaFilter`), and the addresses switched off.
/// An address shows when it is on and its area meets the ticks (`area_of`);
/// a message, when one of the addresses holding it shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    /// The areas ticked: "work+admin", "" for none.
    #[serde(default, with = "area_text")]
    pub areas: Area,
    /// The addresses switched off, by id; every one is on unless said here.
    #[serde(default)]
    pub off: Vec<String>,
}

impl Filter {
    /// Whether the filter shows an address's mail: on, and its area ticked.
    pub fn shows(&self, account: &Account) -> bool {
        !self.off.contains(&account.id) && area_of(account).meets(self.areas)
    }
}

/// An area as text in a JSON or TOML: "work+admin"; "" (or words it does not know) for none.
mod area_text {
    use crate::areas::Area;

    pub fn serialize<S: serde::Serializer>(area: &Area, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&area.id())
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Area, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Area::parse(&text).unwrap_or_default())
    }
}

/// What an address is for, as its card says (Accounts, "What this address is
/// for"); unsaid, work, as everywhere else: an address you did not classify
/// never reaches your evenings (docs/areas.md).
pub fn area_of(account: &Account) -> Area {
    account.area.as_deref().and_then(Area::parse).unwrap_or(Area::WORK)
}

/// What Mail's filter ticks by itself now: what the hours bring
/// (`areafilter::hours_say`), and nothing while you sleep or pause, when
/// every address rests (docs/areas.md, "Sleep").
pub fn hours_say(mode: &crate::quiet::Mode) -> Area {
    if mode.sleeps() { Area::default() } else { crate::areafilter::hours_say(mode) }
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

/// The addresses that sync, ranked by priority as Mail's column ranks them,
/// each with its folders (`folders`: as the server last listed them), its
/// colour, whose stamps count for its mail, and, when it rests at `mode`,
/// the line saying until when ("work@example.org rests until 09:00."; while
/// you sleep, every address). The window's mailbox and the agents' read the same.
pub fn addresses(config: &crate::config::Config, mode: &crate::quiet::Mode, now: &jiff::Zoned, tr: &Translator, folders: impl Fn(&str) -> Vec<Folder>) -> Vec<Address> {
    use crate::areas::{Time, in_view};
    let hours = (mode.time != Time::Any).then_some((mode.time, mode.week));
    let mut ranked: Vec<&Account> = config.accounts.iter().filter(|a| a.syncs()).collect();
    ranked.sort_by_key(|a| a.priority);
    let tints = tints(&ranked);
    let sources = config.mail_sources();
    let until = |at: Option<&jiff::Zoned>| at.map(|z| crate::quiet::until_text(tr, z, now)).unwrap_or_default();
    let said = |id: &str, pairs: &[(&str, String)]| {
        let mut args = i18n::args();
        for (name, value) in pairs {
            args.set(*name, value.clone());
        }
        tr.text(id, Some(&args))
    };
    ranked
        .iter()
        .zip(tints)
        .map(|(account, tint)| {
            let area = area_of(account);
            let rests = hours.is_some_and(|(time, week)| time == Time::Sleep || !in_view(area, time, week));
            let resting = rests.then(|| {
                if mode.sleeps() {
                    said("mailbox-rests-sleep", &[("until", until(mode.until.as_ref()))])
                } else {
                    let back = if area == Area::WORK { mode.back.as_ref().or(mode.until.as_ref()) } else { mode.until.as_ref() };
                    said("mailbox-rests", &[("address", account.address.clone().unwrap_or_else(|| account.id.clone())), ("until", until(back))])
                }
            });
            let trusted = sources.iter().find(|s| s.account.as_deref() == Some(account.id.as_str())).map(|s| s.trusted_ids.clone()).unwrap_or_default();
            Address { account: (*account).clone(), folders: folders(&account.id), trusted, tint, resting }
        })
        .collect()
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
    /// Your own domains, as the Porch has them (`Config::own_domains`): their ARC seals vouch here too.
    pub own_domains: Vec<String>,
    /// What you said of a message while it waits for its Undo (the window's
    /// Spam, Not spam, Junk, Not junk), by its file: said already for its
    /// row, which changes kind at once; its file is where it was meanwhile.
    pub waiting: BTreeMap<PathBuf, labels::Label>,
    /// The domains you deal with (`spam::vouched`), and what the matrix's row
    /// does with proven mail your provider called spam.
    pub dealings: std::sync::Arc<crate::spam::vouched::Dealings>,
    pub proven: crate::spam::vouched::Proven,
    /// What that row did with messages in a Junk folder, every device's.
    pub backs: Vec<labels::Back>,
    /// Mail's topics (`[[topic]]`) and your word on them, every device's;
    /// the topic presets' rules, for the topics started from one.
    pub topics: Vec<crate::topics::Topic>,
    pub topics_said: crate::topics::labels::Said,
    pub topic_presets: std::sync::Arc<crate::topics::presets::Matcher>,
    /// What the topics' table judged of the mail kept here (`topics::judging`): the last word.
    pub topic_judged: crate::topics::judging::Judgments,
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
            own_domains: config.own_domains(),
            waiting: BTreeMap::new(),
            dealings: crate::spam::vouched::current(),
            proven: config.spam.proven(),
            backs: labels::read_back_in(&root),
            topics: config.topics.clone(),
            topics_said: crate::topics::labels::Said::load(),
            topic_presets: crate::topics::presets::Matcher::current(),
            topic_judged: crate::topics::judging::Judgments::load(&crate::topics::judging::table_path()),
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
    /// The quiet word of a message that stays in All: your provider's on
    /// proven mail ("Your provider marked it as spam.", "Your provider put
    /// it in Spam."), else your filter's on one it flagged probably not spam
    /// where it lies ("probably not spam"), else "".
    pub flag: String,
    /// A shielded address's hostile mail: its words kept to itself.
    pub hostile: bool,
    /// Its addresses' marks, ranked; none with a single address.
    pub marks: Vec<Mark>,
    /// "received", "sent", "spam".
    pub kinds: Vec<&'static str>,
    /// Never a message seen on its server only: the folders' menu reads it.
    pub server: bool,
    /// Its topic's name and id, empty for none; why it has it ("yours",
    /// "sender", "conversation", "folder", "category", "preset", "site",
    /// "list", "model": Sioul's guess, "maybe": a doubt, which the window
    /// shows only under that topic's filter) and, for the folder and the
    /// category, its name.
    pub topic: String,
    pub topic_id: String,
    pub topic_why: &'static str,
    pub topic_from: String,
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
    /// Mail older than the windows read is kept (an agent's days; at the
    /// most windows only, for the window's list).
    pub earlier: bool,
    /// The window holds more rows than those given: the next page, when the list's end shows.
    pub more: bool,
    /// Messages wait to be read into their folders' indexes: the window asks again while it shows.
    #[serde(skip)]
    pub indexing: bool,
    /// Messages left out by `page_with`'s `keep_out`, over the days and the address asked.
    #[serde(skip)]
    pub closed: usize,
    /// Messages from blocked senders left out, over the same.
    #[serde(skip)]
    pub blocked: usize,
    /// What is there, in words, when nothing is: never a count.
    pub sentence: String,
    pub chip: Chip,
    pub account: String,
    pub windows: u32,
}

/// A message listed, before its row is made: what the chips and the order need.
struct Listed<'a> {
    uri: String,
    card: Card,
    /// Its addresses' places in `addresses`, ranked.
    held: Vec<usize>,
    kinds: Vec<&'static str>,
    spam: Option<String>,
    /// Flagged probably not spam where it lies: its word.
    flag: Option<String>,
    sent_kind: bool,
    received_kind: bool,
    unread_here: bool,
    read: bool,
    /// Each address's first-ranked copy, in `copies`.
    acting: Vec<usize>,
    from_own: bool,
    stranger: bool,
    sent: i64,
    /// Its topic (`topics::rules`).
    topic: Option<crate::topics::rules::Tagged>,
    /// Ranked: the first opens.
    copies: Vec<Copy<'a>>,
}

/// One copy of a message, as a folder's index gives it.
struct Copy<'a> {
    path: &'a Path,
    line: &'a Line,
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

/// A message's topic (`topics::rules::topic_of`): its sender counts when
/// Sioul's checks authenticate it and it is not yours; its folders as read,
/// its sender's cards' categories.
fn topic_of(world: &World, card: &Card, copies: &[Copy], address: &Address, from_own: bool, addresses: &[Address]) -> Option<crate::topics::rules::Tagged> {
    let results = trust::read_auth_results(&card.headers, &address.trusted);
    let sealers = trust::sealers(&address.trusted, &world.own_domains);
    let excused = card.is_list.then(|| trust::list_excused(results.as_ref(), &card.headers, &sealers)).flatten();
    let (judged, _) = trust::judge_sender(results.as_ref(), excused.as_ref(), card.sender_domain());
    let sender = (!from_own && judged != Trust::Forged && trust::authenticated(results.as_ref(), &card.headers, &sealers)).then(|| card.from_address.as_deref().map(|a| a.trim().to_lowercase())).flatten();
    let id = card.message_id.as_deref().map(crate::mailindex::bare_id);
    let first = copies.first()?;
    let conversation = crate::topics::rules::conversation(&first.line.references, &first.line.in_reply_to);
    let folders: Vec<String> = copies.iter().filter_map(|c| addresses[c.address].folders.iter().find(|f| f.name == c.folder).map(|f| f.display.clone())).collect();
    let categories = sender.as_deref().map(|a| world.senders.contacts.of_address(a)).unwrap_or_default();
    let verified = (!from_own && judged == Trust::Verified).then(|| card.sender_domain()).flatten();
    let facts = crate::topics::rules::Facts { id: id.as_deref(), sender: sender.as_deref(), conversation: &conversation, folders: &folders, categories: &categories, verified_domain: verified, list_id: None, is_list: card.is_list };
    let judged = id.as_deref().and_then(|id| world.topic_judged.of(id));
    crate::topics::rules::with_model(&world.topics, &world.topics_said, &facts, Some(&world.topic_presets), judged)
}

/// Why a row has its topic, as a word the window reads.
fn why_word(why: &crate::topics::rules::Why) -> &'static str {
    use crate::topics::rules::Why;
    match why {
        Why::Yours => "yours",
        Why::Sender => "sender",
        Why::Conversation => "conversation",
        Why::Folder(_) => "folder",
        Why::Category(_) => "category",
        Why::Preset(crate::topics::presets::By::Domain) => "preset",
        Why::Preset(crate::topics::presets::By::Site) => "site",
        Why::Preset(crate::topics::presets::By::List) => "list",
        Why::Model(crate::topics::table::Band::Maybe) => "maybe",
        Why::Model(_) => "model",
    }
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
/// `fill`: how many messages this look may read into the indexes of folders
/// looked at before (`STEP_FILL`; a folder new to this process reads its
/// `FIRST_FILL` newest).
#[allow(clippy::too_many_arguments)]
pub fn page(addresses: &[Address], world: &World, ask: &Ask, hidden: &BTreeSet<PathBuf>, tr: &Translator, now: i64, zone: &jiff::tz::TimeZone, fill: usize) -> Page {
    page_with(addresses, world, ask, hidden, tr, now, zone, fill, &|_| false)
}

/// `page`, leaving out first the messages `keep_out` says by their address (an
/// agent's: a closed project's mail, a work account kept, mail in no
/// project while that is closed; the unified mailbox's part 6): never made
/// into rows, counted over the days and the address asked alone
/// (`Page::closed`), before blocked senders, spam, the kinds and the chip,
/// so that the count is the same whatever the chip.
///
/// `ask.windows` at 0 (the window's list, no horizon): two weeks read first,
/// then twice as many each time while the rows asked for are not filled and
/// older mail is kept, `MOST_WINDOWS` at most; the folders' indexes filled
/// at the first look alone (`fill`), the later ones reading back what is kept.
#[allow(clippy::too_many_arguments)]
pub fn page_with(addresses: &[Address], world: &World, ask: &Ask, hidden: &BTreeSet<PathBuf>, tr: &Translator, now: i64, zone: &jiff::tz::TimeZone, fill: usize, keep_out: &dyn Fn(&str) -> bool) -> Page {
    if ask.windows != 0 {
        return page_of_windows(addresses, world, ask, hidden, tr, now, zone, fill, keep_out);
    }
    let (mut windows, mut fill) = (1, fill);
    loop {
        let page = page_of_windows(addresses, world, &Ask { windows, ..ask.clone() }, hidden, tr, now, zone, fill, keep_out);
        if page.more || !page.earlier || windows >= MOST_WINDOWS {
            return page;
        }
        windows = windows.saturating_mul(2).min(MOST_WINDOWS);
        fill = 0;
    }
}

/// `page_with` over `ask.windows` two-week windows back (one at least).
#[allow(clippy::too_many_arguments)]
fn page_of_windows(addresses: &[Address], world: &World, ask: &Ask, hidden: &BTreeSet<PathBuf>, tr: &Translator, now: i64, zone: &jiff::tz::TimeZone, fill: usize, keep_out: &dyn Fn(&str) -> bool) -> Page {
    let several = addresses.len() > 1;
    let letters = letters(&addresses.iter().map(|a| &a.account).collect::<Vec<_>>());
    let mark = |i: usize| Mark { account: addresses[i].account.id.clone(), letter: letters[i].clone(), tint: addresses[i].tint, title: addresses[i].account.address.clone().unwrap_or_else(|| addresses[i].account.id.clone()) };
    // Addresses whose server keeps labels (Gmail: an All Mail folder): one message in several folders.
    let labelled: Vec<bool> = addresses.iter().map(|a| a.folders.iter().any(|f| f.role == Role::All)).collect();
    let own: Vec<String> = addresses.iter().filter_map(|a| a.account.address.as_deref().map(str::to_ascii_lowercase)).collect();
    let query = ask.query.trim().to_lowercase();
    let since = now - WINDOW_DAYS * 86_400 * i64::from(ask.windows.max(1));
    let mut older = false;
    // The window's filter: the addresses it shows (every one without it). Their
    // folders are all read all the same: a message one of them holds keeps every mark.
    let passes: Vec<bool> = addresses.iter().map(|a| ask.filter.as_ref().is_none_or(|f| f.shows(&a.account))).collect();
    // Each folder's recent lines, kept in this process; a search reads them all.
    let mut looks: Vec<(usize, &Folder, mailcache::Recent)> = Vec::new();
    let (mut budget, mut indexing) = (fill, false);
    for (i, address) in addresses.iter().enumerate() {
        let root = address.account.maildir_path();
        // The trash read only where labels make one message of several folders' copies.
        for folder in address.folders.iter().filter(|f| f.role != Role::Drafts && (f.role != Role::Trash || labelled[i]) && kept(f, &address.folders, &address.account)) {
            let first = !mailcache::is_kept(&address.account.id, &root, &folder.local);
            let recent = mailcache::recent(&address.account.id, &root, &folder.local, if query.is_empty() { since } else { i64::MIN }, if first { FIRST_FILL } else { budget });
            if !first {
                budget = budget.saturating_sub(recent.read);
            }
            indexing |= recent.waiting > 0;
            older |= recent.older && folder.role != Role::Trash && passes[i];
            looks.push((i, folder, recent));
        }
    }
    // Every copy in view, by its address.
    let mut by_address: BTreeMap<String, Vec<Copy>> = BTreeMap::new();
    for (i, folder, recent) in &looks {
        for (path, line) in recent.lines.iter() {
            let sent = mailcache::sent_at(line);
            if query.is_empty() && sent < since {
                older |= folder.role != Role::Trash && passes[*i];
                continue;
            }
            if !found(line, &query) {
                continue;
            }
            let Some(uri) = crate::mailfind::address_of_line(&addresses[*i].account.id, line) else { continue };
            let flags = maildir::flags_of(path);
            let hidden = hidden.contains(path);
            by_address.entry(uri).or_default().push(Copy { path, line, address: *i, folder: folder.name.clone(), role: folder.role, flags, sent, hidden });
        }
    }
    let today = jiff::Timestamp::from_second(now).map(|t| t.to_zoned(zone.clone()).date()).unwrap_or_default();
    // What each message is, cheaply, for all; then what it shows, for the rows kept.
    let mut listed: Vec<Listed> = Vec::new();
    let (mut closed, mut blocked) = (0, 0);
    for (uri, mut copies) in by_address {
        // What waits for its Undo leaves; where labels make one message (Gmail), a
        // copy waiting, in the trash or in the junk is where the message is: that
        // address's other copies are its labels' old copies until their next round.
        let moved_off: BTreeSet<usize> = copies.iter().filter(|c| labelled[c.address] && (c.hidden || matches!(c.role, Role::Trash | Role::Junk))).map(|c| c.address).collect();
        copies.retain(|c| !c.hidden && c.role != Role::Trash && (c.role == Role::Junk || !moved_off.contains(&c.address)));
        if copies.is_empty() || (!ask.account.is_empty() && !copies.iter().any(|c| addresses[c.address].account.id == ask.account)) {
            continue;
        }
        // Kept from whoever asks (an agent): left out before anything else, counted.
        if keep_out(&uri) {
            closed += 1;
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
            blocked += 1;
            continue;
        }
        let from_own = card.from_address.as_deref().is_some_and(|f| own.contains(&f.trim().to_ascii_lowercase()));
        let sent_copy = |c: &Copy| c.role == Role::Sent || (c.role == Role::All && from_own);
        // What you just said, waiting for its Undo, is newer than anything kept: said already.
        let waiting = copies.iter().find_map(|c| world.waiting.get(c.path).copied());
        let said_spam = waiting == Some(labels::Label::Spam);
        // What you said wins: not spam, even where it lies in a Junk folder.
        let said_ham = match waiting {
            Some(label) => label == labels::Label::Ham,
            None => copies.iter().any(|c| c.flags.contains(maildir::NOT_JUNK)) || labels::said_ham_in(&world.said, &card),
        };
        let stranger = !knows(world, &card);
        // Proven mail from someone you deal with (`spam::vouched`): judged once, when it matters.
        let vouched_once = std::cell::OnceCell::new();
        let vouched = || *vouched_once.get_or_init(|| !world.dealings.is_empty() && crate::spam::vouched::vouched(&card, crate::spam::vouched::judged(&card, &address.trusted, &world.own_domains), &world.dealings));
        // In its Junk folder where your provider put it: among your mail, unless the matrix's row
        // does nothing, you said anything of it (here, `$Junk`, the label log), or the row brought
        // it back once or left it at your Undo (it lies there by your doing).
        let said_anything = waiting.is_some() || copies.iter().any(|c| c.flags.contains(maildir::JUNK)) || labels::said_in(&world.said, &card).is_some();
        let handled = || card.message_id.as_deref().map(crate::mailindex::bare_id).is_some_and(|id| labels::handled(&world.backs, &address.account.id, &id));
        let kept_proven = |c: &Copy| c.role == Role::Junk && world.proven != crate::spam::vouched::Proven::Nothing && !said_ham && !said_anything && !handled() && vouched();
        let received_copy = |c: &Copy| (matches!(c.role, Role::Inbox | Role::Other | Role::Archive | Role::All) || (c.role == Role::Junk && (said_ham || kept_proven(c)))) && !sent_copy(c);
        let spam_word = |c: &Copy| -> Option<String> {
            if said_ham {
                return None;
            }
            // Said spam just now: on its way into the junk.
            if said_spam {
                return Some(tr.text("mailbox-spam-junk", None));
            }
            let id = c.line.message_id.as_deref().map(crate::mailindex::bare_id);
            let account = &addresses[c.address].account.id;
            let class_word = |class: crate::spam::Class| tr.text(class.word_id(), None);
            if kept_proven(c) {
                return None;
            }
            if c.role == Role::Junk {
                let moved = world.moved.iter().rev().find(|m| m.account == *account && id.is_some() && m.message_id.as_deref() == id.as_deref());
                return Some(moved.map_or_else(|| tr.text("mailbox-spam-junk", None), |m| class_word(m.class)));
            }
            // Your filter flagged it where it lies: its log names its place. Probably not
            // spam is no spam: marked so in All, never under Spam (`flag_word`).
            if let Some(origin) = maildir::origin_of(&c.path)
                && let Some(flagged) = world.flagged.iter().rev().find(|f| f.account == *account && f.folder == c.folder && f.uidvalidity == origin.validity && f.uid == origin.uid)
                && flagged.class != crate::spam::Class::Ham
            {
                return Some(class_word(flagged.class));
            }
            // Your provider's word counts for a stranger's mail only, never alone for proven mail you deal with.
            (c.line.spam && stranger && !from_own && !vouched()).then(|| tr.text("mailbox-spam-provider", None))
        };
        let spam_words: Vec<Option<String>> = copies.iter().map(|c| spam_word(c)).collect();
        // Flagged probably not spam where it lies (the matrix's "Flag only" for ham): its word,
        // as the Porch's review queue says it, on a row that stays in All (decided 3 November 2026).
        let flag_word = (!said_ham && !said_spam)
            .then(|| {
                copies.iter().find_map(|c| {
                    let origin = maildir::origin_of(&c.path)?;
                    let account = &addresses[c.address].account.id;
                    let flagged = world.flagged.iter().rev().find(|f| f.account == *account && f.folder == c.folder && f.uidvalidity == origin.validity && f.uid == origin.uid)?;
                    (flagged.class == crate::spam::Class::Ham).then(|| tr.text(flagged.class.word_id(), None))
                })
            })
            .flatten();
        // Your provider's word on proven mail, one quiet mark until you say: put in its Junk
        // folder; flagged, or its Subject's tag taken off before a signature verified.
        let brought = || card.message_id.as_deref().map(crate::mailindex::bare_id).is_some_and(|id| labels::brought(&world.backs, &address.account.id, &id));
        let provider_mark = (!said_ham && !said_spam)
            .then(|| {
                if copies.iter().any(kept_proven) || (copies.iter().any(|c| c.role != Role::Junk) && brought() && vouched()) {
                    Some(tr.text("mail-provider-junked", None))
                } else if card.provider_tag_restored() || (copies.iter().any(|c| c.line.spam) && vouched()) {
                    Some(tr.text("mail-provider-marked", None))
                } else {
                    None
                }
            })
            .flatten();
        let flag_word = provider_mark.or(flag_word);
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
        // Its addresses, ranked. With the window's filter, it shows when one of them
        // does; else a resting one's rows leave the list, unless another holds it or you asked.
        let mut held: Vec<usize> = copies.iter().map(|c| c.address).collect::<BTreeSet<usize>>().into_iter().collect();
        held.sort_unstable();
        let rests = match ask.filter {
            Some(_) => held.iter().all(|i| !passes[*i]),
            None => held.iter().all(|i| addresses[*i].resting.is_some() && !ask.shown_resting.contains(&addresses[*i].account.id)),
        };
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
            Chip::All => spam.is_none() && received_kind,
            Chip::Every => spam.is_none(),
            Chip::Unread => spam.is_none() && unread_here,
            Chip::Sent => sent_kind,
            Chip::Spam => spam.is_some(),
        };
        if !shown {
            continue;
        }
        // Its topic: spam has none.
        let topic = if world.topics.is_empty() || spam.is_some() { None } else { topic_of(world, &card, &copies, &addresses[first.address], from_own, addresses) };
        if !ask.topic.is_empty() && topic.as_ref().map(|t| world.topics[t.topic].id.as_str()) != Some(ask.topic.as_str()) {
            continue;
        }
        let sent = copies.iter().map(|c| c.sent).min().unwrap_or(first.sent);
        listed.push(Listed { uri, card, held, kinds, spam, flag: flag_word, sent_kind, received_kind, unread_here, read, acting, from_own, stranger, sent, topic, copies });
    }
    // Newest first; a page of rows a window at most, "Earlier messages" showing more.
    listed.sort_by(|a, b| b.sent.cmp(&a.sent).then_with(|| a.copies[0].path.as_os_str().cmp(b.copies[0].path.as_os_str())));
    let more = listed.len() > ask.rows.max(1);
    listed.truncate(ask.rows.max(1));
    let mut rows = Vec::with_capacity(listed.len());
    for Listed { uri, card, held, kinds, spam, flag, sent_kind, received_kind, unread_here, read, acting, from_own, stranger, sent, topic, copies } in listed {
        let first = &copies[0];
        let address = &addresses[first.address];
        let outgoing = !received_kind && sent_kind;
        let item = crate::view::mail_item(&card, if outgoing { Role::Sent } else { first.role }, false, &first.flags, &address.trusted, &world.own_domains, &world.words, tr);
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
            let results = trust::read_auth_results(&card.headers, &address.trusted);
            let excused = card.is_list.then(|| trust::list_excused(results.as_ref(), &card.headers, &trust::sealers(&address.trusted, &world.own_domains))).flatten();
            let unproven = !matches!(trust::judge_sender(results.as_ref(), excused.as_ref(), card.sender_domain()).0, Trust::Verified);
            let who = if unproven { Who::Stranger } else { world.senders.who_of(&card) };
            let always = !unproven && card.from_address.as_deref().is_some_and(|a| world.always.admits_address(a));
            world.attention.mail_from(who, always, from_own, area_of(&address.account), &world.now).shown
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
            flag: flag.unwrap_or_default(),
            hostile,
            marks: if several { held.iter().map(|i| mark(*i)).collect() } else { Vec::new() },
            kinds,
            server: false,
            topic: topic.as_ref().map(|t| world.topics[t.topic].name.clone()).unwrap_or_default(),
            topic_id: topic.as_ref().map(|t| world.topics[t.topic].id.clone()).unwrap_or_default(),
            topic_why: topic.as_ref().map_or("", |t| why_word(&t.why)),
            topic_from: match topic.as_ref().map(|t| &t.why) {
                Some(crate::topics::rules::Why::Folder(name) | crate::topics::rules::Why::Category(name)) => name.clone(),
                _ => String::new(),
            },
            sent,
        });
    }
    // The filter shows no address: its own state says so, nothing more.
    let none_shown = ask.filter.is_some() && !passes.contains(&true);
    let sentence = if !rows.is_empty() || none_shown {
        String::new()
    } else if !query.is_empty() {
        let mut args = i18n::args();
        args.set("query", ask.query.trim().to_string());
        tr.text("mail-not-found", Some(&args))
    } else {
        tr.text("mail-empty", None)
    };
    Page {
        rows,
        // The window's filter says it all: no resting line beside it.
        resting: if ask.filter.is_some() {
            Vec::new()
        } else {
            addresses.iter().filter(|a| !ask.shown_resting.contains(&a.account.id)).filter_map(|a| Some(Resting { account: a.account.id.clone(), line: a.resting.clone()? })).collect()
        },
        addresses: if several { (0..addresses.len()).map(mark).collect() } else { Vec::new() },
        earlier: query.is_empty() && older,
        more,
        indexing,
        closed,
        blocked,
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
            page(&self.addresses, world, ask, hidden, &Translator::new("en"), NOW, &jiff::tz::TimeZone::get("Europe/Paris").unwrap(), STEP_FILL)
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
            own_domains: Vec::new(),
            waiting: BTreeMap::new(),
            dealings: std::sync::Arc::new(crate::spam::vouched::Dealings::default()),
            proven: crate::spam::vouched::Proven::default(),
            backs: Vec::new(),
            topics: Vec::new(),
            topics_said: crate::topics::labels::Said::default(),
            topic_presets: std::sync::Arc::default(),
            topic_judged: crate::topics::judging::Judgments::default(),
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

    /// Each row's topic and why: a folder's seed, your word, its
    /// conversation, your rule for its sender; a topic's mail alone under its filter.
    #[test]
    fn rows_carry_their_topic() {
        let stand = Stand::new("topics");
        let hour = 3600;
        stand.put(0, "Archive", 1, "kept@example.org", "Ana <ana@example.org>", "noa@work.example.org", "A paper kept", NOW - 5 * hour, NOW - 5 * hour, "S", "", "…");
        stand.put(0, "INBOX", 2, "tagged@example.org", "Ben <ben@example.org>", "noa@work.example.org", "Rehearsal", NOW - 4 * hour, NOW - 4 * hour, "S", "", "…");
        stand.put(0, "INBOX", 3, "reply@example.org", "Eve <eve@example.org>", "noa@work.example.org", "Re: Rehearsal", NOW - 3 * hour, NOW - 3 * hour, "S", "In-Reply-To: <tagged@example.org>\r\n", "…");
        stand.put(0, "INBOX", 4, "always@example.org", "Cat <cat@example.org>", "noa@work.example.org", "Concert", NOW - 2 * hour, NOW - 2 * hour, "S", "", "…");
        stand.put(0, "INBOX", 5, "plain@example.org", "Dan <dan@example.org>", "noa@work.example.org", "Nothing of it", NOW - hour, NOW - hour, "S", "", "…");
        use crate::topics::labels::{Act, Rule, Said};
        let mut world = world_at(crate::areas::Time::Work, "", "");
        world.topics = vec![
            crate::topics::Topic { id: "choir".into(), name: "Chorale".into(), ..crate::topics::Topic::default() },
            crate::topics::Topic { id: "papers".into(), name: "Papiers".into(), seeds: crate::topics::Seeds { folders: vec!["Archive".into()], categories: Vec::new() }, ..crate::topics::Topic::default() },
        ];
        world.topics_said = Said::of(&[
            Act::Message { at: 1, message_id: "tagged@example.org".into(), topic: Some("choir".into()), forget: false },
            Act::Sender { at: 2, address: "cat@example.org".into(), topic: "choir".into(), rule: Rule::Always },
        ]);
        let page = stand.page(&world, &Ask::default(), &BTreeSet::new());
        let topic = |subject: &str| {
            let r = row(&page, subject);
            (r.topic.as_str(), r.topic_id.as_str(), r.topic_why, r.topic_from.as_str())
        };
        assert_eq!(topic("A paper kept"), ("Papiers", "papers", "folder", "Archive"));
        assert_eq!(topic("Rehearsal"), ("Chorale", "choir", "yours", ""));
        assert_eq!(topic("Re: Rehearsal"), ("Chorale", "choir", "conversation", ""));
        assert_eq!(topic("Concert"), ("Chorale", "choir", "sender", ""));
        assert_eq!(topic("Nothing of it"), ("", "", "", ""));
        let choir = stand.page(&world, &Ask { topic: "choir".into(), ..Ask::default() }, &BTreeSet::new());
        assert_eq!(subjects(&choir), ["Concert", "Re: Rehearsal", "Rehearsal"]);
        // The model's word last: a doubt about the plain message, a sure guess the folder overrules.
        use crate::topics::table::Band;
        world.topic_judged.topics.insert("plain@example.org".into(), ("papers".into(), Band::Maybe));
        world.topic_judged.topics.insert("kept@example.org".into(), ("choir".into(), Band::Sure));
        let page = stand.page(&world, &Ask::default(), &BTreeSet::new());
        let plain = row(&page, "Nothing of it");
        assert_eq!((plain.topic_id.as_str(), plain.topic_why), ("papers", "maybe"));
        assert_eq!(row(&page, "A paper kept").topic_why, "folder");
        let papers = stand.page(&world, &Ask { topic: "papers".into(), ..Ask::default() }, &BTreeSet::new());
        assert_eq!(subjects(&papers), ["Nothing of it", "A paper kept"], "the doubt under the topic's filter too");
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
        page(addresses, world, ask, &BTreeSet::new(), &Translator::new("en"), NOW, &jiff::tz::TimeZone::get("Europe/Paris").unwrap(), STEP_FILL)
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
        let ham = stand.put(0, "INBOX", 8, "ham@example.org", "Shop <news@shop.example>", "noa@work.example.org", "Flagged not spam", at, at, "", "", "News.");
        let mut world = world_at(crate::areas::Time::Work, "", "ana@example.org");
        let account = stand.addresses[0].account.id.clone();
        let judged = |uid: u32, id: &str, class| labels::Moved { at, account: account.clone(), folder: "INBOX".into(), uidvalidity: 7, uid, message_id: Some(id.into()), class };
        world.moved.push(judged(2, "moved@example.org", crate::spam::Class::Unsure));
        let origin = maildir::origin_of(&flagged).unwrap();
        world.flagged.push(labels::Moved { uidvalidity: origin.validity, ..judged(3, "flagged@example.org", crate::spam::Class::Spam) });
        world.said.push(labels::Entry { at, account: account.clone(), folder: "INBOX".into(), uidvalidity: 7, uid: 6, message_id: Some("said@example.org".into()), label: labels::Label::Ham, source: labels::Source::NotSpam });
        // Flagged probably not spam where it lies (decided 3 November 2026): in All, with its word, never under Spam.
        let ham_origin = maildir::origin_of(&ham).unwrap();
        world.flagged.push(labels::Moved { uidvalidity: ham_origin.validity, ..judged(8, "ham@example.org", crate::spam::Class::Ham) });
        let all = stand.page(&world, &Ask::default(), &BTreeSet::new());
        let mut shown = subjects(&all);
        shown.sort_unstable();
        assert_eq!(shown, ["A friend, flagged", "Flagged not spam", "Not junk in the junk", "Said not spam"]);
        let marked: BTreeMap<&str, (&str, &str)> = all.rows.iter().map(|r| (r.subject.as_str(), (r.spam.as_str(), r.flag.as_str()))).collect();
        assert_eq!(marked["Flagged not spam"], ("", "probably not spam"));
        assert!(marked.iter().filter(|(s, _)| **s != "Flagged not spam").all(|(_, w)| *w == ("", "")), "{marked:?}");
        let spam = stand.page(&world, &Ask { chip: Chip::Spam, ..Ask::default() }, &BTreeSet::new());
        let words: BTreeMap<&str, &str> = spam.rows.iter().map(|r| (r.subject.as_str(), r.spam.as_str())).collect();
        assert_eq!(words, BTreeMap::from([("In the junk", "in the junk"), ("Moved by your filter", "maybe spam"), ("Flagged where it lies", "probably spam"), ("Your provider's word", "your provider: spam")]));
        assert!(spam.rows.iter().all(|r| !r.unread && r.kinds == ["spam"]));
    }

    /// Proven mail from someone you deal with (part 2 of 4 November 2026):
    /// your provider's flag alone never makes it spam, it shows marked; in
    /// the Junk folder it shows among your mail, marked, as the matrix's row
    /// says, unless you said anything of it or the row left it there at your
    /// Undo; an unproven sender's, or a domain you never dealt with, stays
    /// spam; a Subject whose tag Sioul's check took off says so too.
    #[test]
    fn proven_mail_your_provider_called_spam() {
        let stand = Stand::new("proven");
        let at = NOW - 3600;
        let provider = "X-Spam-Flag: YES\r\nReceived: from mail.bulk.example (mail.bulk.example [203.0.112.9]) by mx.provider.example with ESMTPS\r\n";
        let notice = "Notices <notification@example.org>";
        stand.put(0, "INBOX", 1, "flagged@example.org", notice, "noa@work.example.org", "Flagged by your provider", at, at, "", provider, "Your parcel.");
        stand.put(0, "Junk", 2, "junked@example.org", notice, "noa@work.example.org", "Put in Spam", at, at, "", "", "Your parcel.");
        stand.put(0, "Junk", 3, "stranger@example.org", "Ads <ads@shop.example>", "noa@work.example.org", "Unproven in Spam", at, at, "", "", "Buy.");
        stand.put(0, "Junk", 4, "said@example.org", notice, "noa@work.example.org", "Said spam", at, at, "", "", "Your parcel.");
        stand.put(0, "Junk", 5, "left@example.org", notice, "noa@work.example.org", "Left at your Undo", at, at, "", "", "Your parcel.");
        stand.put(0, "Junk", 6, "keyword@example.org", notice, "noa@work.example.org", "Junk elsewhere", at, at, &format!("S{}", maildir::JUNK), "", "Your parcel.");
        stand.put(0, "INBOX", 8, "brought@example.org", notice, "noa@work.example.org", "Brought back", at, at, "", "", "Your parcel.");
        // Its Subject's tag taken off by this installation's check, before a signature verified.
        let own = crate::config::sioul_authserv_id();
        let restored = format!("Authentication-Results: {own}; dkim=pass (after your provider's subject tag) header.d=example.org; dmarc=pass header.from=example.org\r\n");
        stand.put(0, "INBOX", 7, "tagged@example.org", "Shop <news@example.org>", "noa@work.example.org", "***SPAM*** Your order", at, at, "", &restored, "Thanks.");
        let account = stand.addresses[0].account.id.clone();
        let mut world = world_at(crate::areas::Time::Work, "", "");
        let mut dealings = crate::spam::vouched::Dealings::default();
        dealings.with("commande@example.org", crate::spam::vouched::Dealing::Kept);
        world.dealings = std::sync::Arc::new(dealings);
        world.said.push(labels::Entry { at, account: account.clone(), folder: "Junk".into(), uidvalidity: 7, uid: 4, message_id: Some("said@example.org".into()), label: labels::Label::Spam, source: labels::Source::Junk });
        world.backs.push(labels::Back { at, account: account.clone(), folder: "Junk".into(), uidvalidity: 7, uid: 5, message_id: Some("left@example.org".into()), back: labels::BackKind::Left });
        world.backs.push(labels::Back { at, account: account.clone(), folder: "Junk".into(), uidvalidity: 7, uid: 18, message_id: Some("brought@example.org".into()), back: labels::BackKind::Brought });
        let marks = |world: &World| -> (BTreeMap<String, String>, BTreeMap<String, String>) {
            let all = stand.page(world, &Ask::default(), &BTreeSet::new());
            let spam = stand.page(world, &Ask { chip: Chip::Spam, ..Ask::default() }, &BTreeSet::new());
            (all.rows.iter().map(|r| (r.subject.clone(), r.flag.clone())).collect(), spam.rows.iter().map(|r| (r.subject.clone(), r.spam.clone())).collect())
        };
        let (all, spam) = marks(&world);
        let pairs = |list: &[(&str, &str)]| list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<BTreeMap<_, _>>();
        assert_eq!(all, pairs(&[("Brought back", "Your provider put it in Spam."), ("Flagged by your provider", "Your provider marked it as spam."), ("Put in Spam", "Your provider put it in Spam."), ("Your order", "Your provider marked it as spam.")]));
        assert_eq!(spam, pairs(&[("Junk elsewhere", "in the junk"), ("Left at your Undo", "in the junk"), ("Said spam", "in the junk"), ("Unproven in Spam", "in the junk")]));
        // "Mark only": the same; "Do nothing": the Junk folder's proven mail is spam again, the flag still not.
        world.proven = crate::spam::vouched::Proven::Mark;
        assert_eq!(marks(&world).0["Put in Spam"], "Your provider put it in Spam.");
        world.proven = crate::spam::vouched::Proven::Nothing;
        let (all, spam) = marks(&world);
        assert!(!all.contains_key("Put in Spam") && spam["Put in Spam"] == "in the junk", "{all:?}");
        assert_eq!(all["Flagged by your provider"], "Your provider marked it as spam.");
        // Nobody you deal with: your provider's word, as before.
        world.dealings = std::sync::Arc::new(crate::spam::vouched::Dealings::default());
        let (all, spam) = marks(&world);
        assert_eq!(spam["Flagged by your provider"], "your provider: spam");
        assert_eq!(all["Your order"], "Your provider marked it as spam.", "the tag taken off says so, whoever sent it");
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

    /// All is what you received (8 November 2026): what you sent alone is
    /// under Sent, never mixed in; a message you sent yourself and also
    /// received stays; an agent's Every has both.
    #[test]
    fn sent_mail_is_not_mixed_into_all() {
        let stand = Stand::new("sentout");
        let hour = 3600;
        stand.put(0, "INBOX", 1, "in@example.org", "Ana <ana@example.org>", "noa@work.example.org", "Received", NOW - hour, NOW - hour, "S", "", "…");
        stand.put(0, "Sent", 2, "out@example.org", "Noa <noa@work.example.org>", "Iris <iris@example.org>", "Sent to Iris", NOW - 2 * hour, NOW - 2 * hour, "S", "", "…");
        stand.put(0, "Sent", 3, "self@example.org", "Noa <noa@work.example.org>", "Noa <noa@work.example.org>", "To myself", NOW - 3 * hour, NOW - 3 * hour, "S", "", "…");
        stand.put(0, "INBOX", 4, "self@example.org", "Noa <noa@work.example.org>", "Noa <noa@work.example.org>", "To myself", NOW - 3 * hour, NOW - 3 * hour + 5, "S", "", "…");
        let world = world_at(crate::areas::Time::Work, "", "");
        let chip = |chip: Chip| subjects(&stand.page(&world, &Ask { chip, ..Ask::default() }, &BTreeSet::new())).into_iter().map(str::to_string).collect::<Vec<_>>();
        assert_eq!(chip(Chip::All), ["Received", "To myself"]);
        assert_eq!(chip(Chip::Sent), ["Sent to Iris", "To myself"]);
        assert_eq!(chip(Chip::Every), ["Received", "Sent to Iris", "To myself"]);
        assert_eq!(Chip::parse("every"), Chip::Every);
    }

    /// The window's list has no horizon (`windows` at 0, 8 November 2026):
    /// everything kept, newest first, read back as far as the rows asked for
    /// need; a crowded fortnight reads no further than it; an agent's days
    /// stay its own (`two_weeks_then_earlier`).
    #[test]
    fn the_window_list_reaches_everything_kept() {
        let stand = Stand::new("nohorizon");
        let day = 86_400;
        stand.put(0, "INBOX", 1, "recent@example.org", "Ana <ana@example.org>", "noa@work.example.org", "Recent", NOW - 3 * day, NOW - 3 * day, "S", "", "…");
        stand.put(0, "Archive", 2, "older@example.org", "Ben <ben@example.org>", "noa@work.example.org", "Three weeks ago", NOW - 21 * day, NOW - 21 * day, "S", "", "…");
        stand.put(1, "INBOX", 3, "years@example.org", "Cat <cat@example.org>", "noa@home.example.org", "Three years ago", NOW - 3 * 365 * day, NOW - 3 * 365 * day, "S", "", "…");
        let world = world_at(crate::areas::Time::Work, "", "");
        let all = stand.page(&world, &Ask { windows: 0, ..Ask::default() }, &BTreeSet::new());
        assert_eq!((subjects(&all), all.more, all.earlier), (vec!["Recent", "Three weeks ago", "Three years ago"], false, false));
        let one = stand.page(&world, &Ask { windows: 0, rows: 1, ..Ask::default() }, &BTreeSet::new());
        assert_eq!((subjects(&one), one.more, one.windows), (vec!["Recent"], true, 2), "two weeks hold one row; four, the one more that says more follow");
        let none = stand.page(&world, &Ask { windows: 0, query: "nothing like it".into(), ..Ask::default() }, &BTreeSet::new());
        assert_eq!((none.rows.len(), none.sentence.as_str()), (0, "Nothing matches “nothing like it”."));
    }

    /// An agent's days: two weeks, then two more at each window; the quick
    /// search looks through everything kept.
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

    /// The first index (§6): a folder new to this process has its newest
    /// `FIRST_FILL` messages read at once, whatever the step's budget; the
    /// others wait (`indexing`), `STEP_FILL` at each later look, until none does.
    #[test]
    fn a_folder_new_to_the_process_is_read_at_once_then_in_steps() {
        let stand = Stand::new("first");
        let n = FIRST_FILL + 2;
        for i in 0..n {
            let at = NOW - 3_600 - i as i64 * 60;
            stand.put(0, "INBOX", i as u32 + 1, &format!("f{i}@example.org"), "Ana <ana@example.org>", "noa@work.example.org", &format!("Note {i}"), at, at, "S", "", "…");
        }
        let world = world_at(crate::areas::Time::Work, "", "");
        let look = |fill: usize| page(&stand.addresses, &world, &Ask { rows: n, ..Ask::default() }, &BTreeSet::new(), &Translator::new("en"), NOW, &jiff::tz::TimeZone::get("Europe/Paris").unwrap(), fill);
        let first = look(0);
        assert_eq!((first.rows.len(), first.indexing), (FIRST_FILL, true), "the newest at once, a step's budget aside");
        assert_eq!(first.rows[0].subject, "Note 0");
        let held = look(0);
        assert_eq!((held.rows.len(), held.indexing), (FIRST_FILL, true), "no budget: nothing more read");
        let next = look(STEP_FILL);
        assert_eq!((next.rows.len(), next.indexing), (n, false));
    }

    /// The addresses the window and agents read (part 6, `addresses`): by
    /// priority; a work address rests in your leisure, until work comes back;
    /// every one while you sleep; none at work.
    #[test]
    fn addresses_say_which_rest_and_until_when() {
        use crate::config::Priority;
        let account = |id: &str, area: &str, priority: Priority| Account { area: Some(area.into()), priority, ..Account::imap(id, &format!("noa@{id}.example.org"), "imap.example.org", 993, crate::config::Security::Tls, None) };
        let config = crate::config::Config { accounts: vec![account("home", "leisure+admin", Priority::Average), account("work", "work", Priority::Above)], ..crate::config::Config::default() };
        let zone = jiff::tz::TimeZone::get("Europe/Paris").unwrap();
        let now = jiff::Timestamp::from_second(NOW).unwrap().to_zoned(zone.clone());
        let back = jiff::Timestamp::from_second(NOW + 19 * 3600).unwrap().to_zoned(zone);
        let tr = Translator::new("en");
        let mode = |time, reason| crate::quiet::Mode { quiet: time != crate::areas::Time::Work, time, week: crate::areas::Week::default(), reason, until: Some(back.clone()), back: Some(back.clone()), label: String::new() };
        let folders = |_: &str| vec![folder("INBOX", Some("/"), Some(Role::Inbox))];
        let at_work = addresses(&config, &mode(crate::areas::Time::Work, crate::quiet::Reason::Working), &now, &tr, folders);
        assert_eq!(at_work.iter().map(|a| a.account.id.as_str()).collect::<Vec<_>>(), ["work", "home"], "by priority");
        assert!(at_work.iter().all(|a| a.resting.is_none()));
        let evening = addresses(&config, &mode(crate::areas::Time::Leisure, crate::quiet::Reason::Evening), &now, &tr, folders);
        assert_eq!(evening.iter().map(|a| a.resting.is_some()).collect::<Vec<_>>(), [true, false]);
        assert!(evening[0].resting.as_deref().is_some_and(|l| l.contains("noa@work.example.org") && l.contains("09:00")), "{:?}", evening[0].resting);
        let night = addresses(&config, &mode(crate::areas::Time::Sleep, crate::quiet::Reason::Sleep), &now, &tr, folders);
        assert!(night.iter().all(|a| a.resting.is_some()), "every address while you sleep");
    }

    /// What whoever asks keeps out (an agent's consent, part 6): never a row,
    /// counted over the days and the address asked, the same count whatever
    /// the chip (unread, spam, sent); blocked senders counted apart.
    #[test]
    fn kept_mail_is_counted_whatever_the_chip() {
        let stand = Stand::new("kept");
        let at = NOW - 3600;
        stand.put(0, "INBOX", 1, "closed-1@example.org", "Iris <iris@client.example>", "noa@work.example.org", "Closed, unread", at, at, "", "", "Plans.");
        stand.put(0, "Junk", 2, "closed-2@example.org", "Ads <ads@shop.example>", "noa@work.example.org", "Closed, spam", at, at, "S", "", "Buy.");
        stand.put(0, "INBOX", 3, "open-1@example.org", "Ana <ana@example.org>", "noa@work.example.org", "Open", at, at, "S", "", "Hello.");
        stand.put(1, "INBOX", 4, "closed-3@example.org", "Ben <ben@example.org>", "noa@home.example.org", "Closed, at home", at, at, "S", "", "Hello.");
        stand.put(0, "INBOX", 5, "blocked-1@example.org", "Spammer <x@bad.example>", "noa@work.example.org", "Blocked", at, at, "", "", "Buy.");
        let world = world_at(crate::areas::Time::Work, "x@bad.example", "");
        let kept = |uri: &str| uri.starts_with("mid:closed-");
        let with = |ask: &Ask| page_with(&stand.addresses, &world, ask, &BTreeSet::new(), &Translator::new("en"), NOW, &jiff::tz::TimeZone::get("Europe/Paris").unwrap(), STEP_FILL, &kept);
        for chip in [Chip::All, Chip::Unread, Chip::Sent, Chip::Spam] {
            let page = with(&Ask { chip, ..Ask::default() });
            assert!(page.rows.iter().all(|r| !r.subject.starts_with("Closed")), "{chip:?}: never a row");
            assert_eq!((page.closed, page.blocked), (3, 1), "{chip:?}: the same counts");
        }
        let work = with(&Ask { account: stand.addresses[0].account.id.clone(), chip: Chip::Unread, ..Ask::default() });
        assert_eq!((work.closed, work.blocked), (2, 1), "the address asked alone");
        assert_eq!(subjects(&with(&Ask::default())), ["Open"]);
    }

    /// A crowded fortnight, by pages: the rows asked for alone, the newest,
    /// `more` while the window holds others, "Earlier messages" only past its
    /// end; a search is paged the same way.
    #[test]
    fn a_crowded_fortnight_comes_by_pages() {
        let stand = Stand::new("crowded");
        let n = PAGE + 3;
        for i in 0..n {
            let at = NOW - 3_600 - i as i64 * 60;
            stand.put(0, "INBOX", i as u32 + 1, &format!("m{i}@example.org"), "Ana <ana@example.org>", "noa@work.example.org", &format!("Note {i}"), at, at, "S", "", "…");
        }
        stand.put(0, "Archive", 900, "old@example.org", "Ben <ben@example.org>", "noa@work.example.org", "Three weeks ago", NOW - 21 * 86_400, NOW - 21 * 86_400, "S", "", "…");
        let world = world_at(crate::areas::Time::Work, "", "");
        let first = stand.page(&world, &Ask::default(), &BTreeSet::new());
        assert_eq!((first.rows.len(), first.more, first.earlier), (PAGE, true, true));
        assert_eq!((first.rows[0].subject.as_str(), first.rows[PAGE - 1].subject.clone()), ("Note 0", format!("Note {}", PAGE - 1)), "the newest first");
        let both = stand.page(&world, &Ask { rows: 2 * PAGE, ..Ask::default() }, &BTreeSet::new());
        assert_eq!((both.rows.len(), both.more), (n, false));
        assert_eq!(both.rows[..PAGE].iter().map(|r| &r.key).collect::<Vec<_>>(), first.rows.iter().map(|r| &r.key).collect::<Vec<_>>(), "the first page as it was");
        let found = stand.page(&world, &Ask { query: "note".into(), ..Ask::default() }, &BTreeSet::new());
        assert_eq!((found.rows.len(), found.more, found.earlier), (PAGE, true, false));
    }

    /// The window's filter (docs/areas.md, "A page's filter by area"): what
    /// shows is the mail of the addresses that are on and whose area is
    /// ticked; an address without an area is work's, as everywhere; a message
    /// two addresses hold shows when one of them does, with both marks; no
    /// resting line beside the filter, whatever rests; "Earlier messages" and
    /// the list's sentence only for what it shows.
    #[test]
    fn the_filter_shows_the_addresses_ticked_and_on() {
        let mut stand = Stand::new("filter");
        // A third address, its area unsaid.
        let id = format!("plain-filter-{}", std::process::id());
        let mut plain = Account::imap(&id, "noa@plain.example.org", "imap.example.org", 993, crate::config::Security::Tls, None);
        plain.maildir = Some(stand.base.join(&id).display().to_string());
        for sub in ["tmp", "new", "cur"] {
            std::fs::create_dir_all(plain.maildir_path().join(sub)).unwrap();
        }
        stand.addresses.push(Address { account: plain, folders: vec![folder("INBOX", Some("/"), Some(Role::Inbox))], trusted: vec!["mx.example.org".into()], tint: 2, resting: None });
        let at = NOW - 3600;
        stand.put(0, "INBOX", 1, "work@example.org", "Iris <iris@client.example>", "noa@work.example.org", "Work mail", at, at, "S", "", "Plans.");
        stand.put(1, "INBOX", 2, "home@example.org", "Ana <ana@example.org>", "noa@home.example.org", "Home mail", at - 60, at - 60, "S", "", "Hello.");
        stand.put(2, "INBOX", 3, "plain@example.org", "Ben <ben@example.org>", "noa@plain.example.org", "Unsaid mail", at - 120, at - 120, "S", "", "Hi.");
        stand.put(0, "INBOX", 4, "both@example.org", "List <news@list.example.org>", "noa@work.example.org", "On both", at - 180, at - 180, "S", "List-Id: <news.list.example.org>\r\n", "News.");
        stand.put(1, "INBOX", 5, "both@example.org", "List <news@list.example.org>", "noa@home.example.org", "On both", at - 180, at - 170, "S", "List-Id: <news.list.example.org>\r\n", "News.");
        stand.put(0, "Archive", 6, "old@example.org", "Iris <iris@client.example>", "noa@work.example.org", "Three weeks ago", NOW - 21 * 86_400, NOW - 21 * 86_400, "S", "", "…");
        // Work rests now (the evening): beside the filter, it says nothing.
        stand.addresses[0].resting = Some("noa@work.example.org rests until 09:00.".into());
        let world = world_at(crate::areas::Time::Leisure, "", "");
        let (work, home, plain) = (stand.addresses[0].account.id.clone(), stand.addresses[1].account.id.clone(), stand.addresses[2].account.id.clone());
        let with = |areas: Area, off: &[&String]| stand.page(&world, &Ask { filter: Some(Filter { areas, off: off.iter().map(|s| s.to_string()).collect() }), ..Ask::default() }, &BTreeSet::new());
        let leisure = with(Area::LEISURE, &[]);
        assert_eq!(subjects(&leisure), ["Home mail", "On both"]);
        assert_eq!(row(&leisure, "On both").marks.len(), 2, "both marks, the hidden address's too");
        assert!(leisure.resting.is_empty() && !leisure.earlier, "no resting line; nothing earlier of what shows");
        let at_work = with(Area::WORK, &[]);
        assert_eq!(subjects(&at_work), ["Work mail", "Unsaid mail", "On both"], "unsaid is work's; a resting address shows when ticked");
        assert!(at_work.earlier && at_work.resting.is_empty());
        assert_eq!(subjects(&with(Area::ADMIN, &[])), ["Home mail", "On both"]);
        // The addresses, on or off, and the areas: both must say yes.
        assert_eq!(subjects(&with(Area::ALL, &[&home])), ["Work mail", "Unsaid mail", "On both"]);
        assert_eq!(subjects(&with(Area::WORK, &[&work])), ["Unsaid mail"]);
        assert_eq!(subjects(&with(Area::LEISURE, &[&home])), Vec::<&str>::new());
        // Nothing shown: no sentence, the filter's own state says it.
        for nothing in [with(Area::default(), &[]), with(Area::ALL, &[&work, &home, &plain])] {
            assert!(nothing.rows.is_empty() && nothing.sentence.is_empty() && !nothing.earlier, "{:?}", nothing.sentence);
            assert_eq!(nothing.addresses.len(), 3, "the marks stay, to switch an address back on");
        }
        // Without it (an agent's): the resting line and the rule of before.
        let agents = stand.page(&world, &Ask::default(), &BTreeSet::new());
        assert_eq!(agents.resting.len(), 1);
        assert!(!subjects(&agents).contains(&"Work mail"));
        // Read from the window's ask as JSON: the areas as words.
        let asked: Ask = serde_json::from_str(r#"{"chip":"all","filter":{"areas":"admin+leisure","off":["home"]}}"#).unwrap();
        assert_eq!(asked.filter, Some(Filter { areas: Area::PERSONAL, off: vec!["home".into()] }));
        let none: Ask = serde_json::from_str(r#"{"filter":{"areas":""}}"#).unwrap();
        assert_eq!(none.filter, Some(Filter::default()));
    }

    /// Mail's filter ticks what the hours bring, and nothing while you sleep
    /// or pause, when every address rests (docs/areas.md, "Sleep").
    #[test]
    fn mail_ticks_what_the_hours_bring_and_nothing_at_night() {
        use crate::areas::{Time, Week};
        use crate::quiet::{Mode, Reason};
        let week = Week { work_hours: true, admin_hours: true, ..Week::default() };
        let mode = |time, reason| Mode { quiet: time != Time::Work, time, week, reason, until: None, back: None, label: String::new() };
        assert_eq!(hours_say(&mode(Time::Work, Reason::Working)), Area::WORK);
        assert_eq!(hours_say(&mode(Time::Admin, Reason::AdminTime)), Area::ADMIN);
        assert_eq!(hours_say(&mode(Time::Leisure, Reason::Evening)), Area::LEISURE);
        assert_eq!(hours_say(&mode(Time::Meals, Reason::Meal)), Area::LEISURE);
        assert_eq!(hours_say(&mode(Time::Sleep, Reason::Sleep)), Area::default());
        assert_eq!(hours_say(&mode(Time::Sleep, Reason::Paused)), Area::default());
        assert_eq!(hours_say(&Mode { week: Week::default(), ..mode(Time::Any, Reason::NoHours) }), Area::ALL);
    }

    /// Spam and Not spam (part 1 of 4 November 2026): while it waits for its
    /// Undo, what you said counts at once (its file still where it was); once
    /// done (the file in the junk or out of it, the label logged), the same.
    /// Spam: under Spam only, "in the junk"; Not spam: received, its words
    /// gone (your filter's flag, the junk's, "probably not spam").
    #[test]
    fn spam_and_not_spam_change_the_row_at_once_and_after() {
        let stand = Stand::new("settle");
        let at = NOW - 3600;
        let account = stand.addresses[0].account.id.clone();
        let plain = stand.put(0, "INBOX", 1, "plain@example.org", "Shop <news@shop.example>", "noa@work.example.org", "Plain", at, at, "", "", "News.");
        let flagged = stand.put(0, "INBOX", 2, "flagged@example.org", "Ads <ads@shop.example>", "noa@work.example.org", "Flagged", at, at, "", "", "Buy.");
        let junk = stand.put(0, "Junk", 3, "junk@example.org", "Club <club@club.example>", "noa@work.example.org", "In the junk", at, at, "", "", "Hi.");
        let ham = stand.put(0, "INBOX", 4, "ham@example.org", "Shop <letters@shop.example>", "noa@work.example.org", "Probably fine", at, at, "", "", "News.");
        let mut world = world_at(crate::areas::Time::Work, "", "");
        let place = |path: &Path, id: &str, class| {
            let origin = maildir::origin_of(path).unwrap();
            labels::Moved { at, account: account.clone(), folder: "INBOX".into(), uidvalidity: origin.validity, uid: origin.uid, message_id: Some(id.into()), class }
        };
        world.flagged.push(place(&flagged, "flagged@example.org", crate::spam::Class::Spam));
        world.flagged.push(place(&ham, "ham@example.org", crate::spam::Class::Ham));
        let kinds = |world: &World, chip: Chip| -> BTreeMap<String, (Vec<&'static str>, String, String)> {
            stand.page(world, &Ask { chip, ..Ask::default() }, &BTreeSet::new()).rows.into_iter().map(|r| (r.subject.clone(), (r.kinds.clone(), r.spam.clone(), r.flag.clone()))).collect()
        };
        let before = kinds(&world, Chip::All);
        assert_eq!(before.keys().collect::<Vec<_>>(), ["Plain", "Probably fine"]);
        assert_eq!(before["Probably fine"].2, "probably not spam");
        // Said while waiting for the Undo: Spam on Plain, Not spam on the three others.
        for (path, label) in [(&plain, labels::Label::Spam), (&flagged, labels::Label::Ham), (&junk, labels::Label::Ham), (&ham, labels::Label::Ham)] {
            world.waiting.insert(path.clone(), label);
        }
        let waiting = kinds(&world, Chip::All);
        assert_eq!(waiting.keys().collect::<Vec<_>>(), ["Flagged", "In the junk", "Probably fine"]);
        assert!(waiting.values().all(|(k, spam, flag)| *k == ["received"] && spam.is_empty() && flag.is_empty()), "{waiting:?}");
        let spam = kinds(&world, Chip::Spam);
        assert_eq!(spam.into_iter().collect::<Vec<_>>(), [("Plain".to_string(), (vec!["spam"], "in the junk".to_string(), String::new()))]);
        // Done: Plain moved into the junk, the labels in the log, nothing waiting.
        world.waiting.clear();
        let moved = stand.dir(0, "Junk").join("cur").join(format!("{}{}2,S", plain.file_name().unwrap().to_string_lossy(), maildir::INFO));
        std::fs::rename(&plain, &moved).unwrap();
        let said = |path: &Path, folder: &str, id: &str, source: labels::Source| {
            let origin = maildir::origin_of(path).unwrap();
            labels::Entry { at: at + 60, account: account.clone(), folder: folder.into(), uidvalidity: origin.validity, uid: origin.uid, message_id: Some(id.into()), label: source.label(), source }
        };
        world.said = vec![said(&moved, "INBOX", "plain@example.org", labels::Source::Junk), said(&flagged, "INBOX", "flagged@example.org", labels::Source::NotSpam), said(&ham, "INBOX", "ham@example.org", labels::Source::NotSpam)];
        let junked = stand.put(0, "INBOX", 9, "junk@example.org", "Club <club@club.example>", "noa@work.example.org", "In the junk", at, at, "", "", "Hi.");
        std::fs::remove_file(&junk).unwrap();
        world.said.push(said(&junked, "Junk", "junk@example.org", labels::Source::NotJunk));
        assert_eq!(kinds(&world, Chip::All), waiting, "the same once done");
        assert_eq!(kinds(&world, Chip::Spam).keys().collect::<Vec<_>>(), ["Plain"]);
    }
}

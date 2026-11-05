// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail topics, learned and measured on this computer (docs/topics.md):
//! the corpus's ham read once with what seeds its topics ([`read`]), the
//! first topics proposed from those seeds ([`seeded`], `sioul topics
//! seeds`), and fastText's classifier over every topic at once, tested in
//! time order ([`evaluate`], `sioul topics eval`). Nothing here changes
//! the mail, a table or a setting: this measures, and says what to build with.
//!
//! What [`evaluate`] does:
//!
//! 1. **Labels**: each message's topic from the seeds, one at most
//!    (`sioul_core::topics::seeds`); most mail has none.
//! 2. **Lines**: the sender's words when Sioul's own checks proved who sent
//!    it (`topics::sender_words`), then the spam filter's words of the message.
//! 3. **Time order**: the oldest 80 % learned, the newest 20 % tested. The
//!    learned part's newest fifth, scored by a model of the rest, fits the
//!    temperature (softmax's, `table`; with a class "none" only, else
//!    fastText's own probabilities are kept: `Fit`) and each topic's sure threshold: the
//!    lowest probability whose precision against the seeds is 0.9 or more,
//!    never under 0.7; maybe from 0.5; no topic below.
//! 4. **Two ways to say "no topic"**: a class "none" learned from a share
//!    of the mail no seed names (`none_weights`, the best share on the
//!    learned part's newest fifth), or the top topic's probability under its
//!    sure threshold alone. The better on the test is kept, the simpler
//!    (the threshold) when they are within half a point of macro F1.
//! 5. **Numbers against the seeds**, per topic and in all: precision (a sure
//!    topic on mail no seed names counts against it: the seeds miss mail
//!    the person never filed, so it is a floor), recall, coverage; the same
//!    for senders the learned part never had; macro F1.
//! 6. **The sender's words** and **the bucket count**: the way kept, again
//!    without them, and at each other bucket count; they stay unless leaving
//!    them makes it better; the fewest buckets within half a point of the best.
//! 7. **The fold**: the way kept, folded into the table every device reads
//!    (`topics::table`), in f32 and in f16, checked against fastText's own
//!    predictions; the bands that f16 changes, the table's size.
//! 8. **Churn**: the same model learned a week earlier: the share of its
//!    sure topics on the tested mail that the newer one changes.
//! 9. **The alternative** (with a Model2Vec folder): a nearest centroid on
//!    that static embedding ([`crate::model2vec`]), tested the same way,
//!    and how it folds.

pub mod model;

use crate::corpus;
use crate::labels::{self, Copy, Label};
use crate::model2vec::StaticModel;
use crate::{Cancel, Dirs, LearnError, Progress, Stage, io_error, now, spamcore, train};
use fasttext::matrix::Matrix;
use serde::Serialize;
use sioul_core::contacts::Contact;
use sioul_core::folders::{Folder, Role};
use sioul_core::mailcache::Line;
use sioul_core::spam::tokenize::Lexicon;
use sioul_core::topics::seeds::{self, Kind, Proposal, Seed};
use sioul_core::topics::table::{self, Band, Judged, Precision, Table};
use sioul_core::trust::{self, Trust};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

/// fastText's label prefix here: capitals, which no word of a message has.
const LABEL: &str = "SIOUL_TOPIC_";
const NONE: &str = "SIOUL_TOPIC_NONE";
/// The largest difference allowed between the f32 table's probability and fastText's.
pub const FOLD_TOLERANCE: f64 = 1e-4;
/// Two ways within this much macro F1 are as good: the simpler is kept.
const TIE: f64 = 0.005;
const WEEK: i64 = 7 * 86_400;
/// Topics with fewer examples learned are "small", where the centroid may help.
const SMALL: u64 = 50;
/// Folders' names that are a server's own, never a topic: Gmail's.
const SYSTEM_PREFIXES: [&str; 2] = ["[Gmail]", "[Google Mail]"];

/// One account's Maildir as topics read it: its id, its folder, its folders
/// as its server last listed them (`folders::saved`: their names as read,
/// their purpose), the authserv-ids it trusts (`Config::mail_sources`).
#[derive(Debug, Clone, Default)]
pub struct Maildir {
    pub account: String,
    pub root: PathBuf,
    pub folders: Vec<Folder>,
    pub trusted: Vec<String>,
}

/// What topics read besides the mail: each account's Maildir, the contacts
/// (their categories), your own addresses. Projects are not read: a
/// project is something to complete, and says nothing of a message's topic.
#[derive(Debug, Clone, Default)]
pub struct Sources {
    pub maildirs: Vec<Maildir>,
    pub contacts: Vec<Contact>,
    pub own: Vec<String>,
    /// The topic presets' rules (`topics::presets`): their mail counted, and seeds.
    pub presets: Option<std::sync::Arc<sioul_core::topics::presets::Matcher>>,
}

impl Sources {
    /// The configuration's: every mail account's Maildir, every address book.
    pub fn of(config: &sioul_core::config::Config) -> Sources {
        let sources = config.mail_sources();
        let maildirs = config
            .accounts
            .iter()
            .filter(|a| a.kind == sioul_core::config::AccountKind::Imap)
            .map(|a| Maildir {
                account: a.id.clone(),
                root: a.maildir_path(),
                folders: sioul_core::folders::saved(&a.id),
                trusted: sources.iter().find(|s| s.account.as_deref() == Some(a.id.as_str())).map(|s| s.trusted_ids.clone()).unwrap_or_default(),
            })
            .collect();
        Sources { maildirs, contacts: sioul_core::contacts::all(), own: sioul_core::porch::own_addresses(&sources), presets: Some(sioul_core::topics::presets::Matcher::current()) }
    }
}

/// How topics are read, learned and measured; `Default` holds docs/topics.md's values.
#[derive(Debug, Clone)]
pub struct Options {
    /// The spam filter's: its labels' rules, its words, the threads, the seed.
    pub spam: train::Options,
    pub dim: u32,
    pub epochs: u32,
    pub lr: f64,
    pub min_count: u32,
    pub minn: u32,
    pub maxn: u32,
    /// The table's bucket count first; the others compared.
    pub buckets: Vec<u32>,
    /// The shares of mail no seed names learned as "none", tried in turn.
    pub none_weights: Vec<f64>,
    /// A topic is sure from the lowest probability with this precision…
    pub precision: f64,
    /// …never under this one.
    pub floor: f64,
    /// Maybe from this probability.
    pub maybe: f64,
    /// Whether a sure topic on mail no seed names counts against its
    /// precision when its threshold is chosen (the seeds miss what the person
    /// never filed); false: only confusions between topics count.
    pub count_unseeded: bool,
    pub rules: seeds::Rules,
    /// A folder unused this long (seconds) seeds nothing.
    pub unused: i64,
    /// Every way compared; false: the threshold with the sender's words alone, at the first bucket count.
    pub compare: bool,
    /// A Model2Vec folder: its centroid measured too.
    pub model2vec: Option<PathBuf>,
    /// The language seeds from presets are named in ("fr": "Banque").
    pub language: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            spam: train::Options::default(),
            dim: 100,
            epochs: 25,
            lr: 0.1,
            min_count: 5,
            minn: 3,
            maxn: 6,
            buckets: vec![50_000, 200_000],
            none_weights: vec![0.1, 0.25, 0.5],
            precision: 0.9,
            floor: 0.7,
            maybe: 0.5,
            count_unseeded: true,
            rules: seeds::Rules::default(),
            unused: 365 * 86_400,
            compare: true,
            model2vec: None,
            language: "en".into(),
        }
    }
}

impl Options {
    /// The defaults, with the spam filter's rules and words from the configuration.
    pub fn of(config: &sioul_core::config::Config) -> Options {
        let language = config.language.clone().unwrap_or_else(sioul_core::i18n::system_language);
        Options { spam: train::Options::of(config), language, ..Options::default() }
    }
}

/// One message kept here, as topics read it.
#[derive(Default)]
pub(crate) struct Mail {
    pub(crate) date: i64,
    /// Its correspondent, lowercase: its sender's address; for yours, its first recipient's.
    pub(crate) sender: Option<String>,
    /// Yours: in Sent, or from one of your addresses.
    pub(crate) yours: bool,
    pub(crate) words: Vec<String>,
    /// The words groups are made of (`groups`): its subject's and its own
    /// text's, what it quotes, its attributions and its signature left out
    /// (`reading::parts`), and neither the tokenizer's placeholders nor HTML's
    /// and CSS's words (`grouping_words`). The spam model keeps them all.
    pub(crate) grouping: Vec<String>,
    /// Its sender's words (`topics::sender_words`), when proven.
    pub(crate) senders: Vec<String>,
    /// Its tokens in the Model2Vec model, when one is read.
    pub(crate) tokens: Vec<u32>,
    /// Its subject (300 characters at most), its bare Message-ID, the ids it
    /// answers or cites, the addresses it went to (with `words` only, yours always).
    pub(crate) subject: String,
    /// Its sender's name as the message gives it (none for yours).
    pub(crate) from_name: Option<String>,
    pub(crate) id: Option<String>,
    pub(crate) refs: Vec<String>,
    pub(crate) to: Vec<String>,
    /// The topic preset its proven sender matches (`topics::presets::Matcher::find`), by id.
    pub(crate) preset: Option<String>,
    /// What the order of words reads of it (`sioul_core::topics::rules::Facts`):
    /// its sender when Sioul's checks authenticate it (none for yours), its
    /// sender's domain when verified, its copies' folders as read, its
    /// proven sender's cards' categories (compared), its conversation (its
    /// first first), whether it is a list and its List-Id when read whole.
    pub(crate) proven: Option<String>,
    pub(crate) verified_domain: Option<String>,
    pub(crate) folders: Vec<String>,
    pub(crate) categories: Vec<String>,
    pub(crate) conversation: Vec<String>,
    pub(crate) is_list: bool,
    pub(crate) list_id: Option<String>,
}

impl Mail {
    /// The line fastText and the table read.
    fn line(&self, senders: bool) -> Vec<String> {
        sioul_core::topics::line(&self.words, if senders { &self.senders } else { &[] })
    }
}

/// Folders that seed nothing, by why.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LeftOut {
    /// The server's own: the inbox aside, Sent, the archive, all mail, Gmail's.
    pub system: usize,
    /// Fewer messages than a topic needs.
    pub small: usize,
    /// Unused for a year.
    pub unused: usize,
}

/// What reading found, source by source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Reading {
    /// Messages kept to learn from, each once, and of them yours.
    pub messages: u64,
    pub yours: u64,
    /// Folders read, and the files in them (copies included).
    pub folders: u64,
    pub files: u64,
    /// Junk, trash and draft folders, never read.
    pub folders_left_out: u64,
    /// Messages left out: spam (the spam filter's labels, a $Junk keyword), forged.
    pub spam: u64,
    pub forged: u64,
    /// Messages each kind of seed names (the seeds kept), and of them yours.
    pub folder_named: u64,
    pub folder_yours: u64,
    pub category_named: u64,
    pub category_yours: u64,
    /// Each topic preset's messages, by its id, by how it took them; a
    /// preset of `presets::DAY_ONE` messages or more would start as a topic.
    pub presets: BTreeMap<String, sioul_core::topics::presets::Counted>,
    pub preset_named: u64,
}

/// What was read: every message kept here once, its seeds, what was left out.
pub struct Read {
    pub(crate) mail: Vec<Mail>,
    pub seeds: Vec<Seed>,
    pub left_out: LeftOut,
    pub reading: Reading,
}

impl Read {
    /// Messages read.
    pub fn messages(&self) -> usize {
        self.mail.len()
    }
}

/// A local folder's purpose and its name as the person reads it: as its
/// server last listed it; else its Maildir++ directory's levels (".Loisirs.Chorale"
/// → "Loisirs/Chorale"), or a folder held in another ("Archive/2024").
fn folder_of(maildir: &Maildir, local: &str) -> (Role, String) {
    if local.is_empty() {
        return (Role::Inbox, "INBOX".to_string());
    }
    if let Some(folder) = maildir.folders.iter().find(|f| f.local == local) {
        return (folder.role, folder.display.clone());
    }
    let display = match local.strip_prefix('.') {
        Some(levels) => levels.split('.').map(percent_decoded).collect::<Vec<_>>().join("/"),
        None => local.to_string(),
    };
    (sioul_core::folders::role_of(&display, None), display)
}

/// A level of a Maildir++ directory, its `%XX` read back ("%2E" → ".").
fn percent_decoded(level: &str) -> String {
    let mut out = String::new();
    let mut rest = level;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let code = rest.get(at + 1..at + 3).and_then(|h| u32::from_str_radix(h, 16).ok()).and_then(char::from_u32);
        match code {
            Some('\0') => rest = &rest[at + 3..],
            Some(c) => {
                out.push(c);
                rest = &rest[at + 3..];
            }
            None => {
                out.push('%');
                rest = &rest[at + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The last level of a folder's name: "Loisirs/Chorale" → "Chorale".
fn leaf(display: &str) -> &str {
    display.rsplit('/').find(|l| !l.trim().is_empty()).unwrap_or(display).trim()
}

fn folded(text: &str) -> String {
    sioul_core::text::fold(text.trim()).into_iter().collect()
}

/// Each address's categories (key, name as written), from the contacts' cards.
fn categories_of(contacts: &[Contact]) -> HashMap<String, Vec<(String, String)>> {
    let mut out: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for contact in contacts {
        for email in &contact.emails {
            let address = email.value.trim().to_lowercase();
            if address.is_empty() {
                continue;
            }
            for name in &contact.categories {
                let key = sioul_core::porch::category_key(name);
                if key.is_empty() {
                    continue;
                }
                let list = out.entry(address.clone()).or_default();
                if !list.iter().any(|(k, _)| *k == key) {
                    list.push((key, name.trim().to_string()));
                }
            }
        }
    }
    out
}

/// What the spam filter's labels call spam: the corpus's messages labelled
/// spam (`labels::decide`, the label log included), by their key (the bare
/// Message-ID); then the label log alone, its newest word on each message,
/// for those the corpus no longer holds.
fn spam_keys(dirs: &Dirs, trusted: &BTreeMap<String, Vec<String>>, options: &Options) -> Result<HashSet<String>, LearnError> {
    let mut copies = Vec::new();
    corpus::read_all(dirs, &options.spam.microsoft, |record| copies.push(Copy::judged(&record, trusted, &options.spam.dealings)))?;
    let log = labels::read_log(dirs);
    let (labeled, _) = labels::decide(copies, &log, &labels::read_moved(dirs), &labels::read_flagged(dirs));
    let mut spam: HashSet<String> = labeled.into_iter().filter(|l| l.label == Label::Spam).map(|l| l.key).collect();
    let mut said: BTreeMap<String, (i64, Label)> = BTreeMap::new();
    for entry in &log {
        if let Some(id) = entry.message_id.as_deref().map(sioul_core::mailindex::bare_id).filter(|id| !id.is_empty()) {
            let newest = said.entry(id).or_insert((entry.at, entry.label));
            if entry.at >= newest.0 {
                *newest = (entry.at, entry.label);
            }
        }
    }
    for (id, (_, label)) in said {
        match label {
            Label::Spam => spam.insert(id),
            Label::Ham => spam.remove(&id),
        };
    }
    Ok(spam)
}

/// A message's key: its bare Message-ID; without one, its digest in its account.
fn key_of(account: &str, line: &Line) -> Option<String> {
    match line.message_id.as_deref().map(sioul_core::mailindex::bare_id).filter(|id| !id.is_empty()) {
        Some(id) => Some(id),
        None => line.digest.as_ref().map(|d| format!("digest:{account}/{d}")),
    }
}

/// The addresses a message went to (To, Cc), lowercase, from its bytes (its header block is enough).
fn recipients_in(bytes: &[u8]) -> Vec<String> {
    let Some(headers) = mail_parser::MessageParser::default().parse_headers(bytes) else { return Vec::new() };
    let mut out: Vec<String> = headers.to().into_iter().chain(headers.cc()).flat_map(|a| a.iter()).filter_map(|a| a.address()).map(|a| a.trim().to_lowercase()).filter(|a| a.contains('@')).collect();
    out.dedup();
    out
}

/// The addresses a message went to, from its file's header block.
fn recipients(path: &std::path::Path) -> Vec<String> {
    let Some(header) = sioul_core::mailcache::headers_of(path) else { return Vec::new() };
    recipients_in(&[header.as_slice(), b"\r\n\r\n"].concat())
}

/// A message's start, as much as the card reads: a megabyte at most.
fn start_of(path: &std::path::Path) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(1 << 20).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// HTML's and CSS's words that a message's text keeps when its HTML is read
/// as text (its styles), which say nothing of what the message is about.
const MARKUP: &str = "style styles width height align valign center important table tables tbody thead td tr th class classes nbsp font fonts color colour border borders padding margin margins px pt em rem div span img src href html htm css background bgcolor display none block inline solid dotted dashed family size weight bold italic decoration underline cellpadding cellspacing collapse max min auto media screen webkit moz rgba rgb hex spacing float clear overflow hidden visible position relative absolute vertical middle baseline serif sans arial helvetica verdana tahoma georgia mso msonormal outlook doctype xmlns meta charset viewport utf body head title rel stylesheet role presentation responsive wrapper container column columns spacer gmail yahoo apple detectors format telephone preheader";

/// What a grouping leaves out, as the tokenizer writes it: the markup's words above.
fn noise_words(lexicon: &Lexicon) -> HashSet<String> {
    spamcore::tokens_with(lexicon, "", MARKUP).words.into_iter().chain(MARKUP.split_whitespace().map(str::to_string)).collect()
}

/// A message's words for its groups (`Mail::grouping`): its subject's and
/// its own text's, as the tokenizer writes them, without the placeholders
/// it puts for links, prices, dates and the like (`_URL_`), nor `noise`.
fn grouping_words(lexicon: &Lexicon, noise: &HashSet<String>, subject: &str, excerpt: &str) -> Vec<String> {
    let own: Vec<String> = sioul_core::reading::parts(excerpt)
        .into_iter()
        .filter_map(|part| match part {
            sioul_core::reading::Part::Text { text } => Some(text),
            _ => None,
        })
        .collect();
    spamcore::tokens_with(lexicon, subject, &own.join("\n"))
        .words
        .into_iter()
        .filter(|w| !w.is_empty() && !w.contains(char::is_whitespace) && !w.starts_with('_') && !w.ends_with('_') && !noise.contains(w) && !noise.contains(&w.to_lowercase()))
        .collect()
}

/// One copy of a message in a local folder.
struct Local {
    maildir: usize,
    path: PathBuf,
    role: Role,
    display: String,
    line: Line,
    junk: bool,
}

/// Every copy of every message kept on this device, by key (Message-ID, or
/// the account's digest): each folder of each account's Maildir through its
/// index (`mailcache::folder`), Junk, Trash and Drafts counted and left
/// out. The one place that walks the Maildirs, so that the shared reader
/// (`mailpass`) can take its place alone.
fn kept_copies(sources: &Sources, reading: &mut Reading, system: &mut BTreeSet<String>, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<BTreeMap<String, Vec<Local>>, LearnError> {
    let mut copies: BTreeMap<String, Vec<Local>> = BTreeMap::new();
    for (d, maildir) in sources.maildirs.iter().enumerate() {
        for local in sioul_core::mailcache::folders(&maildir.root) {
            let (role, display) = folder_of(maildir, &local);
            if matches!(role, Role::Junk | Role::Trash | Role::Drafts) {
                reading.folders_left_out += 1;
                system.insert(format!("{}/{}", maildir.account, folded(&display)));
                continue;
            }
            if role != Role::Other && role != Role::Inbox || SYSTEM_PREFIXES.iter().any(|p| display.starts_with(p)) {
                system.insert(format!("{}/{}", maildir.account, folded(&display)));
            }
            reading.folders += 1;
            progress(&Progress { stage: Stage::Tokens, done: reading.folders, total: 0, detail: format!("{} · {display}", maildir.account) });
            for (path, line) in sioul_core::mailcache::folder(&maildir.account, &maildir.root, &local) {
                reading.files += 1;
                let Some(key) = key_of(&maildir.account, &line) else { continue };
                let keywords = sioul_core::maildir::keywords_of(&path);
                copies.entry(key).or_default().push(Local { maildir: d, path, role, display: display.clone(), line, junk: keywords.junk && !keywords.not_junk });
            }
            if cancel.cancelled() {
                return Err(LearnError::Cancelled);
            }
        }
    }
    Ok(copies)
}

/// Reads every message kept on this device, once (the copies in several
/// folders or accounts, by Message-ID, are one): every folder of every
/// account's Maildir but Junk, Trash and Drafts, Sent and the archives
/// included, through each folder's index (`mailcache::folder`), so that no
/// message is read again for what its headers say. Left out: what the spam
/// filter's labels call spam (the corpus serves only for them: `spam_keys`)
/// or a `$Junk` keyword marks; forged mail (`trust::judge_sender`; a list's
/// failing DMARC, which lists break, aside). A project's mail is read like
/// any other. Each message with its seeds: its copies' folders (the
/// server's own aside), its correspondent's categories (the sender's when
/// authenticated, `trust::authenticated`; for yours, its recipients'). With `words`, each message's file is read once
/// for its words, its sender's words (when verified) and, with `model`, its tokens.
#[allow(clippy::too_many_arguments)]
pub fn read(dirs: &Dirs, trusted: &BTreeMap<String, Vec<String>>, sources: &Sources, options: &Options, words: bool, mut model: Option<&mut StaticModel>, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Read, LearnError> {
    let check = || if cancel.cancelled() { Err(LearnError::Cancelled) } else { Ok(()) };
    progress(&Progress { stage: Stage::Labels, done: 0, total: 0, detail: String::new() });
    let spam = spam_keys(dirs, trusted, options)?;
    check()?;
    let mut reading = Reading::default();
    let mut system: BTreeSet<String> = BTreeSet::new();
    let copies = kept_copies(sources, &mut reading, &mut system, progress, cancel)?;
    let lexicon = Lexicon::cached(&options.spam.lexicon);
    let noise = noise_words(&lexicon);
    let categories = categories_of(&sources.contacts);
    let own: HashSet<String> = sources.own.iter().map(|a| a.trim().to_lowercase()).collect();

    let mut mail: Vec<Mail> = Vec::new();
    let mut by_seed: BTreeMap<(Kind, String), (String, Vec<usize>)> = BTreeMap::new();
    let total = copies.len() as u64;
    for (n, (key, mut found)) in copies.into_iter().enumerate() {
        if n % 500 == 0 {
            check()?;
            progress(&Progress { stage: Stage::Tokens, done: n as u64, total, detail: String::new() });
        }
        if spam.contains(&key) || found.iter().any(|c| c.junk) {
            reading.spam += 1;
            continue;
        }
        found.sort_by_key(|c| c.role.rank());
        let first = &found[0];
        let maildir = &sources.maildirs[first.maildir];
        let card = first.line.card(&first.path);
        let results = trust::read_auth_results(&card.headers, &maildir.trusted);
        let sealers = trust::sealers(&maildir.trusted, &[]);
        let (judged, _) = trust::judge_sender(results.as_ref(), None, card.sender_domain());
        // A list's message that fails DMARC is the list's doing (the index keeps no list headers to excuse it by).
        if judged == Trust::Forged && !first.line.is_list {
            reading.forged += 1;
            continue;
        }
        let from = first.line.from_address.as_deref().map(|a| a.trim().to_lowercase());
        let yours = found.iter().any(|c| c.role == Role::Sent) || from.as_ref().is_some_and(|f| own.contains(f));
        let authenticated = judged != Trust::Forged && trust::authenticated(results.as_ref(), &card.headers, &sealers);
        let whole_bytes = if words { start_of(&first.path) } else { None };
        let to = match &whole_bytes {
            Some(bytes) => recipients_in(bytes),
            None if yours => recipients(&first.path),
            None => Vec::new(),
        };
        let i = mail.len();
        let mut preset_of = None;
        let verified_domain = (judged == Trust::Verified && !yours).then(|| card.sender_domain().map(str::to_string)).flatten();
        let list_id = whole_bytes.as_deref().and_then(sioul_core::card::Card::from_bytes).and_then(|c| c.headers.first("List-Id").map(|v| v.rsplit_once('<').map_or(v, |(_, r)| r.split('>').next().unwrap_or(r)).trim().to_lowercase()));
        // A topic preset's mail, for a verified sender only: counted, and a seed in your language.
        if let Some(matcher) = &sources.presets
            && !yours
        {
            let verified = verified_domain.as_deref();
            let list = list_id.clone();
            if let Some((p, by)) = matcher.find(verified, list.as_deref(), card.is_list || first.line.is_list) {
                let preset = &matcher.presets[p];
                reading.presets.entry(preset.id.clone()).or_default().add(by);
                preset_of = Some(preset.id.clone());
                by_seed.entry((Kind::Preset, preset.id.clone())).or_insert_with(|| (sioul_core::topics::presets::name_in(preset, &options.language), Vec::new())).1.push(i);
            }
        }
        for copy in found.iter().filter(|c| c.role == Role::Other && !SYSTEM_PREFIXES.iter().any(|p| c.display.starts_with(p))) {
            by_seed.entry((Kind::Folder, folded(&copy.display))).or_insert_with(|| (leaf(&copy.display).to_string(), Vec::new())).1.push(i);
        }
        let correspondents: Vec<&str> = if yours { to.iter().map(String::as_str).collect() } else if authenticated { from.as_deref().into_iter().collect() } else { Vec::new() };
        for address in correspondents {
            for (category, name) in categories.get(address).into_iter().flatten() {
                by_seed.entry((Kind::Category, category.clone())).or_insert_with(|| (name.clone(), Vec::new())).1.push(i);
            }
        }
        let (mut message_words, mut grouping, mut senders, mut tokens) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        if let Some(whole) = whole_bytes.as_deref().and_then(sioul_core::card::Card::from_bytes) {
            message_words = spamcore::tokens_with(&lexicon, &whole.subject, &whole.excerpt).words.into_iter().filter(|w| !w.is_empty() && !w.contains(char::is_whitespace)).collect();
            grouping = grouping_words(&lexicon, &noise, &whole.subject, &whole.excerpt);
            if judged == Trust::Verified && !yours {
                senders = sioul_core::topics::sender_words(&whole, true);
            }
            if let Some(m) = model.as_deref_mut() {
                tokens = m.tokens(&format!("{}\n{}", whole.subject, whole.excerpt));
            }
        }
        let date = found.iter().map(|c| sioul_core::mailcache::sent_at(&c.line)).filter(|&d| d > 0).min().unwrap_or(0);
        let proven = (authenticated && !yours).then(|| from.clone()).flatten();
        let sender = if yours { to.iter().find(|a| !own.contains(*a)).cloned() } else { from };
        reading.yours += u64::from(yours);
        let (subject, refs) = (first.line.subject.clone(), first.line.refs());
        let id = first.line.message_id.as_deref().map(sioul_core::mailindex::bare_id).filter(|id| !id.is_empty());
        let from_name = if yours { None } else { first.line.from_name.clone().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) };
        let categories_of_sender: Vec<String> = proven.as_deref().and_then(|a| categories.get(a)).map(|c| c.iter().map(|(key, _)| key.clone()).collect()).unwrap_or_default();
        let conversation = sioul_core::topics::rules::conversation(&first.line.references, &first.line.in_reply_to);
        let folders: Vec<String> = found.iter().map(|c| c.display.clone()).collect();
        let is_list = card.is_list || first.line.is_list;
        mail.push(Mail {
            date,
            sender,
            yours,
            words: message_words,
            grouping,
            senders,
            tokens,
            subject,
            from_name,
            id,
            refs,
            to,
            preset: preset_of,
            proven,
            verified_domain,
            folders,
            categories: categories_of_sender,
            conversation,
            is_list,
            list_id,
        });
    }
    reading.messages = mail.len() as u64;
    progress(&Progress { stage: Stage::Tokens, done: total, total, detail: String::new() });

    // Folders with too few messages, or unused for a year, seed nothing.
    let mut left_out = LeftOut { system: system.len(), ..LeftOut::default() };
    let since = now() - options.unused;
    let mut seeds = Vec::new();
    for ((kind, _), (name, mut messages)) in by_seed {
        messages.sort_unstable();
        messages.dedup();
        if kind == Kind::Folder {
            if messages.len() < options.rules.least {
                left_out.small += 1;
                continue;
            }
            if messages.iter().map(|&m| mail[m].date).max().unwrap_or(0) < since {
                left_out.unused += 1;
                continue;
            }
        }
        let yours = messages.iter().filter(|&&m| mail[m].yours).count() as u64;
        let n = messages.len() as u64;
        match kind {
            Kind::Folder => (reading.folder_named += n, reading.folder_yours += yours),
            Kind::Category => (reading.category_named += n, reading.category_yours += yours),
            Kind::Preset => (reading.preset_named += n, ()),
        };
        seeds.push(Seed { kind, name, messages });
    }
    Ok(Read { mail, seeds, left_out, reading })
}

/// The topics the seeds propose, with the counts said beside them.
#[derive(Debug, Clone, Serialize)]
pub struct Seeded {
    /// Messages read.
    pub messages: u64,
    /// Messages a proposed topic holds, and of them yours.
    pub named: u64,
    pub named_yours: u64,
    pub left_out: LeftOut,
    pub reading: Reading,
    pub proposal: Proposal,
}

/// The proposal (`seeds::propose`) from what was read, and each message's topic.
pub fn seeded(read: &Read, options: &Options) -> (Seeded, Vec<Option<usize>>) {
    let mail: Vec<seeds::Mail> = read.mail.iter().map(|m| seeds::Mail { sender: m.sender.clone() }).collect();
    let proposal = seeds::propose(&mail, read.seeds.clone(), &options.rules);
    let labels = proposal.labels(mail.len());
    let named = labels.iter().filter(|l| l.is_some()).count() as u64;
    let named_yours = labels.iter().zip(&read.mail).filter(|(l, m)| l.is_some() && m.yours).count() as u64;
    (Seeded { messages: mail.len() as u64, named, named_yours, left_out: read.left_out.clone(), reading: read.reading.clone(), proposal }, labels)
}

/// How a way says "no topic", or the centroid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Method {
    /// A class "none", learned from this share of the mail no seed names.
    NoneClass { weight: f64 },
    /// The top topic's probability under its sure threshold.
    Threshold,
    /// A nearest centroid on a Model2Vec embedding, with the threshold.
    Centroid,
}

/// One way trained and tested: its settings, then its numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Way {
    method: Method,
    senders: bool,
    bucket: u32,
}

impl Way {
    fn name(&self) -> String {
        let method = match self.method {
            Method::NoneClass { weight } => format!("none-{weight}"),
            Method::Threshold => "threshold".to_string(),
            Method::Centroid => "centroid".to_string(),
        };
        let senders = if self.senders { "" } else { "-no-senders" };
        format!("{method}{senders}-{}", self.bucket)
    }
}

/// What a set of judged messages says against the seeds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    /// Messages a seed gives this topic.
    pub seeded: u64,
    /// Sure of this topic, and so seeded.
    pub right: u64,
    /// Sure of this topic, seeded another.
    pub other_topic: u64,
    /// Sure of this topic, which no seed names.
    pub no_seed: u64,
    /// Seeded this topic, and not sure of it.
    pub missed: u64,
    /// Maybe this topic.
    pub maybe: u64,
}

impl Counts {
    fn add(&mut self, other: &Counts) {
        self.seeded += other.seeded;
        self.right += other.right;
        self.other_topic += other.other_topic;
        self.no_seed += other.no_seed;
        self.missed += other.missed;
        self.maybe += other.maybe;
    }

    pub fn precision(&self) -> Option<f64> {
        let shown = self.right + self.other_topic + self.no_seed;
        (shown > 0).then(|| self.right as f64 / shown as f64)
    }

    /// Precision among the mail some seed names: confusions between topics alone.
    pub fn precision_seeded(&self) -> Option<f64> {
        let shown = self.right + self.other_topic;
        (shown > 0).then(|| self.right as f64 / shown as f64)
    }

    pub fn recall(&self) -> Option<f64> {
        (self.seeded > 0).then(|| self.right as f64 / self.seeded as f64)
    }

    fn f1(&self) -> f64 {
        match (self.precision(), self.recall()) {
            (Some(p), Some(r)) if p + r > 0.0 => 2.0 * p * r / (p + r),
            _ => 0.0,
        }
    }
}

/// A way's numbers on one part.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Scores {
    pub messages: u64,
    /// With a sure topic.
    pub sure: u64,
    /// The mean of each topic's F1, over the topics the part has seeded mail of.
    pub macro_f1: Option<f64>,
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    /// The share with a sure topic.
    pub coverage: f64,
}

/// One topic's numbers on the test.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TopicNumbers {
    pub id: String,
    /// Its examples learned from (the oldest 80 %).
    pub learned: u64,
    /// Its seeded mail in the learned part's newest fifth.
    pub validation: u64,
    /// Sure from this probability; none: never sure.
    pub sure_from: Option<f64>,
    pub test: Counts,
    pub precision: Option<f64>,
    /// Precision among the mail some seed names.
    pub precision_seeded: Option<f64>,
    pub recall: Option<f64>,
    /// The same, from senders never seen in the learned part.
    pub new_senders: Counts,
}

/// A way's numbers.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Measured {
    pub name: String,
    pub method: Option<Method>,
    pub senders: bool,
    pub bucket: u32,
    pub temperature: f64,
    pub validation: Scores,
    pub test: Scores,
    pub new_senders: Scores,
    /// Macro F1 over the small topics (fewer than 50 examples learned).
    pub small: Option<f64>,
    pub topics: Vec<TopicNumbers>,
    pub seconds: f64,
}

/// The parts, in time order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Split {
    pub learned: u64,
    /// Of the learned, the newest fifth: the temperature and thresholds.
    pub validation: u64,
    pub tested: u64,
    pub validation_from: i64,
    pub test_from: i64,
}

/// What phase 0 decides: the way, its sender words, its buckets, its bands.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Decided {
    pub way: String,
    pub method: Option<Method>,
    pub senders: bool,
    pub bucket: u32,
    pub temperature: f64,
    /// Each topic's sure threshold (by id); none: never sure.
    pub sure: BTreeMap<String, Option<f64>>,
    pub maybe: f64,
}

/// The way kept, folded and checked.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Fold {
    pub bucket: u32,
    /// Words the table keeps.
    pub words: u64,
    /// Test messages compared with fastText's predictions (500 at most).
    pub checked: u64,
    /// The largest difference of a probability, the f32 table against fastText's.
    pub error_f32: f64,
    pub error_f16: f64,
    /// Test messages whose topic or band the f16 table changes, of `judged`.
    pub flips: u64,
    pub judged: u64,
    pub bytes_f16: u64,
    pub bytes_f32: u64,
    /// The f32 table differs from fastText by more than `FOLD_TOLERANCE`.
    pub broken: bool,
}

/// A week's more mail learned: what the newer model changes of the older's sure topics on the tested mail.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Churn {
    /// The older model learned from mail before this (Unix seconds).
    pub before: i64,
    /// Sure topics the older model shows.
    pub shown: u64,
    /// Of those, changed or taken off by the newer.
    pub changed: u64,
    /// Sure topics the newer adds.
    pub added: u64,
    pub share: Option<f64>,
}

/// The centroid's fold: its size on a phone, its errors.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CentroidFold {
    pub vocabulary: u64,
    pub dim: u64,
    pub error_f16: f64,
    pub flips: u64,
    pub judged: u64,
    /// One f16 score per piece and topic.
    pub bytes_f16: u64,
    /// The tokenizer the phone would carry beside it.
    pub tokenizer_bytes: u64,
}

/// What `sioul topics eval` says.
#[derive(Debug, Clone, Serialize)]
pub struct Evaluation {
    pub seeded: Seeded,
    pub split: Split,
    /// The "none" class's shares, each on the learned part's newest fifth.
    pub none_weights: Vec<(f64, Scores)>,
    pub ways: Vec<Measured>,
    pub decided: Option<Decided>,
    pub fold: Option<Fold>,
    pub churn: Option<Churn>,
    pub centroid: Option<Measured>,
    pub centroid_fold: Option<CentroidFold>,
    pub seconds: f64,
}

/// The messages measured, their seeds' topics, the parts.
struct Data<'a> {
    mail: &'a [Mail],
    labels: Vec<Option<usize>>,
    topics: usize,
    /// In time order: the oldest 80 %…
    learned: Vec<usize>,
    /// …of which the oldest 80 % (fit) and newest 20 % (validation)…
    fit: Vec<usize>,
    validation: Vec<usize>,
    /// …and the newest 20 %.
    tested: Vec<usize>,
    /// Senders the learned part has.
    known: HashSet<&'a str>,
}

/// The newest fifth of ids in time order, apart: (older, newer).
fn fifths(ids: &[usize]) -> (Vec<usize>, Vec<usize>) {
    let cut = ids.len() - (ids.len() as f64 * 0.2).round() as usize;
    (ids[..cut].to_vec(), ids[cut..].to_vec())
}

/// A model learned: fastText's, each input row's scalar per class (rows × classes), the classes it has.
struct Learned {
    model: fasttext::FastText,
    scalars: Vec<f32>,
    classes: usize,
    present: Vec<bool>,
}

impl Learned {
    /// Each class's score for a line, `-inf` for a class it never learned.
    fn logits(&self, line: &[String]) -> Vec<f64> {
        let (mut rows, mut labels) = (Vec::new(), Vec::new());
        self.model.dict().get_line_from_str(&line.join(" "), &mut rows, &mut labels);
        if let Some(eos) = self.model.dict().get_id(fasttext::dictionary::EOS) {
            rows.push(eos);
        }
        let c = self.classes;
        let mut sums = vec![0f64; c];
        for &r in &rows {
            for (s, x) in sums.iter_mut().zip(&self.scalars[r as usize * c..(r as usize + 1) * c]) {
                *s += f64::from(*x);
            }
        }
        sums.iter().zip(&self.present).map(|(s, &here)| if here { s / rows.len().max(1) as f64 } else { f64::NEG_INFINITY }).collect()
    }
}

/// The file a model learns from, removed whatever happens.
struct LearnFile(PathBuf);

impl Drop for LearnFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn class_label(c: usize, topics: usize) -> String {
    if c == topics { NONE.to_string() } else { format!("{LABEL}{c}") }
}

/// fastText's classifier, softmax, learned from messages `ids` of `data` (see the module).
fn learn(dirs: &Dirs, data: &Data, ids: &[usize], way: &Way, options: &Options, cancel: &Cancel) -> Result<Learned, LearnError> {
    std::fs::create_dir_all(&dirs.cache).map_err(|e| io_error(&dirs.cache, e))?;
    let path = dirs.cache.join(format!("topics-{}.txt", std::process::id()));
    let _guard = LearnFile(path.clone());
    let none = matches!(way.method, Method::NoneClass { .. });
    let classes = data.topics + usize::from(none);
    {
        let file = std::fs::File::create(&path).map_err(|e| io_error(&path, e))?;
        let mut out = BufWriter::new(file);
        let mut order: Vec<usize> = ids.to_vec();
        let mut rng = crate::Rng::new(options.spam.seed);
        for i in (1..order.len()).rev() {
            order.swap(i, rng.below(i + 1));
        }
        let mut share = crate::Rng::new(options.spam.seed.wrapping_add(7));
        let mut lines = 0u64;
        for &i in &order {
            let class = match (data.labels[i], way.method) {
                (Some(t), _) => t,
                (None, Method::NoneClass { weight }) if share.unit() < weight => data.topics,
                _ => continue,
            };
            let line = data.mail[i].line(way.senders);
            if line.is_empty() {
                continue;
            }
            writeln!(out, "{} {}", class_label(class, data.topics), line.join(" ")).map_err(|e| io_error(&path, e))?;
            lines += 1;
        }
        out.flush().map_err(|e| io_error(&path, e))?;
        if lines == 0 {
            return Err(LearnError::TooFew { ham: 0, spam: 0 });
        }
    }
    let args = fasttext::args::Args {
        input: path.clone(),
        model: fasttext::args::ModelName::Supervised,
        loss: fasttext::args::LossName::Softmax,
        dim: options.dim as i32,
        epoch: options.epochs as i32,
        lr: options.lr,
        word_ngrams: 1,
        min_count: options.min_count.max(1) as i32,
        bucket: way.bucket as i32,
        minn: options.minn as i32,
        maxn: options.maxn as i32,
        thread: options.spam.threads.max(1) as i32,
        seed: options.spam.seed as i32,
        label: LABEL.to_string(),
        verbose: 0,
        ..fasttext::args::Args::default()
    };
    let model = fasttext::FastText::train_with_abort(args, cancel.flag()).map_err(|e| LearnError::Language(e.to_string()))?;
    let (names, _) = model.get_labels();
    let rows_of_class: Vec<Option<usize>> = (0..classes).map(|c| names.iter().position(|n| *n == class_label(c, data.topics))).collect();
    let output = model.output_matrix();
    let input = model.input_matrix();
    let directions: Vec<Option<Vec<f32>>> = rows_of_class.iter().map(|r| r.map(|r| output.row(r as i64).to_vec())).collect();
    let mut scalars = vec![0f32; input.rows() as usize * classes];
    for r in 0..input.rows() {
        let row = input.row(r);
        for (c, direction) in directions.iter().enumerate() {
            if let Some(d) = direction {
                scalars[r as usize * classes + c] = row.iter().zip(d).map(|(a, b)| a * b).sum();
            }
        }
    }
    Ok(Learned { model, scalars, classes, present: rows_of_class.iter().map(Option::is_some).collect() })
}

/// Softmax's temperature that best fits scores to their true classes
/// (negative log likelihood, golden section between 0.05 and 20).
fn temperature(scored: &[(Vec<f64>, usize)]) -> f64 {
    let loss = |t: f64| -> f64 {
        scored
            .iter()
            .filter(|(f, y)| f.get(*y).is_some_and(|x| x.is_finite()))
            .map(|(f, y)| -table::softmax(f, t)[*y].max(1e-12).ln())
            .sum()
    };
    if scored.is_empty() {
        return 1.0;
    }
    let (mut a, mut b) = (0.05f64.ln(), 20f64.ln());
    let golden = (5f64.sqrt() - 1.0) / 2.0;
    let mut c = b - golden * (b - a);
    let mut d = a + golden * (b - a);
    let (mut fc, mut fd) = (loss(c.exp()), loss(d.exp()));
    for _ in 0..60 {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - golden * (b - a);
            fc = loss(c.exp());
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + golden * (b - a);
            fd = loss(d.exp());
        }
    }
    ((a + b) / 2.0).exp()
}

/// Each topic's sure threshold from probabilities and seeds: the lowest
/// probability whose precision, over the messages at least that sure of
/// that topic (three at least), is `options.precision`, never under
/// `options.floor`; none when no probability reaches it. Mail no seed
/// names counts against it unless `options.count_unseeded` is false.
fn thresholds(judged: &[(Vec<f64>, Option<usize>)], topics: usize, options: &Options) -> Vec<Option<f64>> {
    (0..topics)
        .map(|t| {
            let mut top: Vec<(f64, bool)> = judged
                .iter()
                .filter(|(p, seed)| argmax(&p[..topics]) == Some(t) && (options.count_unseeded || seed.is_some()))
                .map(|(p, seed)| (p[t], *seed == Some(t)))
                .collect();
            top.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            let (mut right, mut lowest) = (0u64, None);
            for (n, &(p, ok)) in top.iter().enumerate() {
                right += u64::from(ok);
                let last_of_its_value = top.get(n + 1).is_none_or(|next| next.0 < p);
                if last_of_its_value && n + 1 >= 3 && right as f64 / (n + 1) as f64 >= options.precision {
                    lowest = Some(p);
                }
            }
            lowest.map(|p| p.max(options.floor))
        })
        .collect()
}

fn argmax(p: &[f64]) -> Option<usize> {
    p.iter().enumerate().filter(|(_, x)| x.is_finite()).max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).map(|(i, _)| i)
}

fn sure_f32(sure: &[Option<f64>]) -> Vec<f32> {
    sure.iter().map(|s| s.map_or(2.0, |s| s as f32)).collect()
}

/// Judged messages counted against their seeds, topic by topic.
fn counted(judged: &[(usize, Judged)], labels: &[Option<usize>], topics: usize) -> Vec<Counts> {
    let mut counts = vec![Counts::default(); topics];
    for &(m, j) in judged {
        let seed = labels[m];
        if let Some(s) = seed {
            counts[s].seeded += 1;
        }
        match (j.band, j.topic) {
            (Band::Sure, Some(t)) => {
                match seed {
                    Some(s) if s == t => counts[t].right += 1,
                    Some(s) => {
                        counts[t].other_topic += 1;
                        counts[s].missed += 1;
                    }
                    None => counts[t].no_seed += 1,
                }
                continue;
            }
            (Band::Maybe, Some(t)) => counts[t].maybe += 1,
            _ => {}
        }
        if let Some(s) = seed {
            counts[s].missed += 1;
        }
    }
    counts
}

fn scores_of(counts: &[Counts], messages: usize) -> Scores {
    let mut all = Counts::default();
    for c in counts {
        all.add(c);
    }
    let with: Vec<&Counts> = counts.iter().filter(|c| c.seeded > 0).collect();
    let sure = all.right + all.other_topic + all.no_seed;
    Scores {
        messages: messages as u64,
        sure,
        macro_f1: (!with.is_empty()).then(|| with.iter().map(|c| c.f1()).sum::<f64>() / with.len() as f64),
        precision: all.precision(),
        recall: all.recall(),
        coverage: if messages == 0 { 0.0 } else { sure as f64 / messages as f64 },
    }
}

/// A way's calibration on the learned part's newest fifth: its temperature, its thresholds, its numbers there.
struct Calibrated {
    temperature: f64,
    sure: Vec<Option<f64>>,
    validation: Scores,
}

/// How a way's temperature is fitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fit {
    /// On every validation message, "none" the truth of those no seed names
    /// (the class "none" learned).
    All,
    /// None: fastText's own probabilities. Without a class "none", fitted on
    /// seeded mail alone, it would only sharpen them (every seeded message
    /// has a topic), and make mail no seed names sure of one.
    Kept,
    /// On seeded mail: the centroid's scores, cosines, have no scale of their own.
    Seeded,
}

/// Calibrates from each validation message's scores (`logits`).
fn calibrated(data: &Data, logits: &[(usize, Vec<f64>)], fit: Fit, options: &Options) -> Calibrated {
    let k = data.topics;
    let truth: Vec<(Vec<f64>, usize)> = logits
        .iter()
        .filter_map(|(m, f)| match (data.labels[*m], fit) {
            (Some(t), _) => Some((f.clone(), t)),
            (None, Fit::All) => Some((f.clone(), k)),
            (None, _) => None,
        })
        .collect();
    let t = if fit == Fit::Kept { 1.0 } else { temperature(&truth) };
    let probabilities: Vec<(Vec<f64>, Option<usize>)> = logits.iter().map(|(m, f)| (table::softmax(f, t), data.labels[*m])).collect();
    let sure = thresholds(&probabilities, k, options);
    let s32 = sure_f32(&sure);
    let judged: Vec<(usize, Judged)> = logits.iter().zip(&probabilities).map(|((m, _), (p, _))| (*m, table::judged(&p[..k], &s32, options.maybe))).collect();
    Calibrated { temperature: t, sure, validation: scores_of(&counted(&judged, &data.labels, k), judged.len()) }
}

/// A way's numbers on the test, from each tested message's scores.
fn tested(data: &Data, way_name: String, method: Option<Method>, way: Option<&Way>, calibration: &Calibrated, logits: &[(usize, Vec<f64>)], options: &Options) -> (Measured, Vec<(usize, Judged)>) {
    let k = data.topics;
    let s32 = sure_f32(&calibration.sure);
    let judged: Vec<(usize, Judged)> = logits.iter().map(|(m, f)| (*m, table::judged(&table::softmax(f, calibration.temperature)[..k], &s32, options.maybe))).collect();
    let counts = counted(&judged, &data.labels, k);
    let fresh: Vec<(usize, Judged)> = judged.iter().copied().filter(|(m, _)| data.mail[*m].sender.as_deref().is_some_and(|s| !data.known.contains(s))).collect();
    let fresh_counts = counted(&fresh, &data.labels, k);
    let learned_of = |t: usize| data.learned.iter().filter(|&&m| data.labels[m] == Some(t)).count() as u64;
    let validation_of = |t: usize| data.validation.iter().filter(|&&m| data.labels[m] == Some(t)).count() as u64;
    let topics: Vec<TopicNumbers> = (0..k)
        .map(|t| TopicNumbers {
            id: String::new(),
            learned: learned_of(t),
            validation: validation_of(t),
            sure_from: calibration.sure[t],
            precision: counts[t].precision(),
            precision_seeded: counts[t].precision_seeded(),
            recall: counts[t].recall(),
            test: counts[t].clone(),
            new_senders: fresh_counts[t].clone(),
        })
        .collect();
    let small: Vec<f64> = topics.iter().filter(|n| n.learned < SMALL && n.test.seeded > 0).map(|n| n.test.f1()).collect();
    let measured = Measured {
        name: way_name,
        method,
        senders: way.is_none_or(|w| w.senders),
        bucket: way.map_or(0, |w| w.bucket),
        temperature: calibration.temperature,
        validation: calibration.validation.clone(),
        test: scores_of(&counts, judged.len()),
        new_senders: scores_of(&fresh_counts, fresh.len()),
        small: (!small.is_empty()).then(|| small.iter().sum::<f64>() / small.len() as f64),
        topics,
        seconds: 0.0,
    };
    (measured, judged)
}

/// Every message's scores by a model.
fn logits_of(learned: &Learned, data: &Data, ids: &[usize], senders: bool) -> Vec<(usize, Vec<f64>)> {
    ids.iter().map(|&m| (m, learned.logits(&data.mail[m].line(senders)))).collect()
}

/// A way validated (a model of the fit part, on the validation part).
fn validate(dirs: &Dirs, data: &Data, way: &Way, options: &Options, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Calibrated, LearnError> {
    progress(&Progress { stage: Stage::Language, done: 0, total: 0, detail: way.name() });
    let model = learn(dirs, data, &data.fit, way, options, cancel)?;
    let logits = logits_of(&model, data, &data.validation, way.senders);
    let fit = if matches!(way.method, Method::NoneClass { .. }) { Fit::All } else { Fit::Kept };
    Ok(calibrated(data, &logits, fit, options))
}

/// A way validated, then learned from the whole learned part and tested.
fn measure(dirs: &Dirs, data: &Data, way: &Way, calibration: Option<Calibrated>, options: &Options, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<(Measured, Calibrated, Learned, Vec<(usize, Judged)>), LearnError> {
    let started = std::time::Instant::now();
    let calibration = match calibration {
        Some(c) => c,
        None => validate(dirs, data, way, options, progress, cancel)?,
    };
    progress(&Progress { stage: Stage::Language, done: 1, total: 2, detail: way.name() });
    let model = learn(dirs, data, &data.learned, way, options, cancel)?;
    let logits = logits_of(&model, data, &data.tested, way.senders);
    let (mut measured, judged) = tested(data, way.name(), Some(way.method), Some(way), &calibration, &logits, options);
    measured.seconds = started.elapsed().as_secs_f64();
    Ok((measured, calibration, model, judged))
}

fn better(a: Option<f64>, b: Option<f64>) -> bool {
    a.unwrap_or(-1.0) > b.unwrap_or(-1.0) + TIE
}

/// Trains and tests topics as the module says; `model`, a Model2Vec model,
/// for the centroid too (with each message's tokens read by `read`).
pub fn evaluate(dirs: &Dirs, read: &Read, options: &Options, model: Option<&StaticModel>, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Evaluation, LearnError> {
    let started = std::time::Instant::now();
    let check = || if cancel.cancelled() { Err(LearnError::Cancelled) } else { Ok(()) };
    let (seeded, labels) = seeded(read, options);
    let topics = seeded.proposal.topics.len();
    let ids: Vec<String> = seeded.proposal.topics.iter().map(|t| t.id.clone()).collect();
    let mut order: Vec<usize> = (0..read.mail.len()).filter(|&m| !read.mail[m].words.is_empty() || !read.mail[m].senders.is_empty()).collect();
    order.sort_by_key(|&m| (read.mail[m].date, m));
    let (learned, tested_ids) = fifths(&order);
    let (fit, validation) = fifths(&learned);
    let split = Split {
        learned: learned.len() as u64,
        validation: validation.len() as u64,
        tested: tested_ids.len() as u64,
        validation_from: validation.first().map_or(0, |&m| read.mail[m].date),
        test_from: tested_ids.first().map_or(0, |&m| read.mail[m].date),
    };
    let mut evaluation = Evaluation { seeded, split, none_weights: Vec::new(), ways: Vec::new(), decided: None, fold: None, churn: None, centroid: None, centroid_fold: None, seconds: 0.0 };
    let seeded_in = |part: &[usize]| part.iter().any(|&m| labels[m].is_some());
    if topics == 0 || !seeded_in(&fit) || !seeded_in(&learned) || tested_ids.is_empty() || validation.is_empty() {
        evaluation.seconds = started.elapsed().as_secs_f64();
        return Ok(evaluation);
    }
    let known: HashSet<&str> = learned.iter().filter_map(|&m| read.mail[m].sender.as_deref()).collect();
    let data = Data { mail: &read.mail, labels, topics, learned, fit, validation, tested: tested_ids, known };
    let named = |mut m: Measured| {
        for (n, id) in m.topics.iter_mut().zip(&ids) {
            n.id = id.clone();
        }
        m
    };
    let first = options.buckets.first().copied().unwrap_or(50_000);

    // The threshold, with the sender's words.
    let threshold = Way { method: Method::Threshold, senders: true, bucket: first };
    let mut kept = measure(dirs, &data, &threshold, None, options, progress, cancel)?;
    kept.0 = named(kept.0);
    let mut kept_way = threshold;
    evaluation.ways.push(kept.0.clone());
    check()?;
    if options.compare {
        // The class "none": each share validated, the best tested.
        let mut best: Option<(f64, Calibrated)> = None;
        for &weight in &options.none_weights {
            let way = Way { method: Method::NoneClass { weight }, senders: true, bucket: first };
            let calibration = validate(dirs, &data, &way, options, progress, cancel)?;
            evaluation.none_weights.push((weight, calibration.validation.clone()));
            if best.as_ref().is_none_or(|(_, b)| calibration.validation.macro_f1.unwrap_or(-1.0) > b.validation.macro_f1.unwrap_or(-1.0)) {
                best = Some((weight, calibration));
            }
            check()?;
        }
        if let Some((weight, calibration)) = best {
            let way = Way { method: Method::NoneClass { weight }, senders: true, bucket: first };
            let mut none = measure(dirs, &data, &way, Some(calibration), options, progress, cancel)?;
            none.0 = named(none.0);
            evaluation.ways.push(none.0.clone());
            if better(none.0.test.macro_f1, kept.0.test.macro_f1) {
                kept = none;
                kept_way = way;
            }
        }
        check()?;
        // Without the sender's words.
        let way = Way { senders: false, ..kept_way };
        let mut without = measure(dirs, &data, &way, None, options, progress, cancel)?;
        without.0 = named(without.0);
        evaluation.ways.push(without.0.clone());
        if better(without.0.test.macro_f1, kept.0.test.macro_f1) {
            kept = without;
            kept_way = way;
        }
        check()?;
        // Other bucket counts: the fewest within half a point of the best.
        let mut by_bucket: Vec<(u32, Option<f64>)> = vec![(kept_way.bucket, kept.0.test.macro_f1)];
        let mut candidates = Vec::new();
        for &bucket in options.buckets.iter().skip(1) {
            let way = Way { bucket, ..kept_way };
            let mut other = measure(dirs, &data, &way, None, options, progress, cancel)?;
            other.0 = named(other.0);
            evaluation.ways.push(other.0.clone());
            by_bucket.push((bucket, other.0.test.macro_f1));
            candidates.push((way, other));
            check()?;
        }
        let best_f1 = by_bucket.iter().map(|(_, f)| f.unwrap_or(-1.0)).fold(f64::NEG_INFINITY, f64::max);
        if let Some(&(bucket, _)) = by_bucket.iter().filter(|(_, f)| f.unwrap_or(-1.0) >= best_f1 - TIE).min_by_key(|(b, _)| *b)
            && bucket != kept_way.bucket
            && let Some(at) = candidates.iter().position(|(w, _)| w.bucket == bucket)
        {
            let (way, other) = candidates.swap_remove(at);
            kept = other;
            kept_way = way;
        }
    }
    let (measured, calibration, learned_model, judged) = kept;
    evaluation.decided = Some(Decided {
        way: measured.name.clone(),
        method: measured.method,
        senders: kept_way.senders,
        bucket: kept_way.bucket,
        temperature: calibration.temperature,
        sure: ids.iter().cloned().zip(calibration.sure.iter().copied()).collect(),
        maybe: options.maybe,
    });

    // The fold, in f32 and f16, against fastText's own predictions.
    progress(&Progress { stage: Stage::Export, done: 0, total: 2, detail: String::new() });
    evaluation.fold = Some(fold_checked(&learned_model, &data, &kept_way, &calibration, &ids, options, &judged));
    check()?;

    // Churn: the same, learned a week earlier.
    let before = evaluation.split.test_from - WEEK;
    let older: Vec<usize> = data.learned.iter().copied().filter(|&m| read.mail[m].date < before).collect();
    let old = if older.iter().any(|&m| data.labels[m].is_some()) {
        progress(&Progress { stage: Stage::Language, done: 0, total: 0, detail: format!("{}-week-before", kept_way.name()) });
        // Too little to learn from a week earlier: no churn said.
        match learn(dirs, &data, &older, &kept_way, options, cancel) {
            Err(LearnError::Cancelled) => return Err(LearnError::Cancelled),
            other => other.ok(),
        }
    } else {
        None
    };
    if let Some(old) = old {
        let old_logits = logits_of(&old, &data, &data.tested, kept_way.senders);
        let s32 = sure_f32(&calibration.sure);
        let k = data.topics;
        let mut churn = Churn { before, ..Churn::default() };
        for ((_, f), (_, new)) in old_logits.iter().zip(&judged) {
            let was = table::judged(&table::softmax(f, calibration.temperature)[..k], &s32, options.maybe);
            let shown = |j: &Judged| (j.band == Band::Sure).then_some(j.topic).flatten();
            match (shown(&was), shown(new)) {
                (Some(a), b) => {
                    churn.shown += 1;
                    if b != Some(a) {
                        churn.changed += 1;
                    }
                }
                (None, Some(_)) => churn.added += 1,
                (None, None) => {}
            }
        }
        churn.share = (churn.shown > 0).then(|| churn.changed as f64 / churn.shown as f64);
        evaluation.churn = Some(churn);
    }
    check()?;

    // The alternative: a nearest centroid on a static embedding.
    if let Some(model) = model {
        progress(&Progress { stage: Stage::Evaluation, done: 0, total: 0, detail: "centroid".into() });
        let (measured, centroid_fold) = centroid(&data, model, options);
        evaluation.centroid = Some(named(measured));
        evaluation.centroid_fold = Some(centroid_fold);
    }
    progress(&Progress { stage: Stage::Export, done: 2, total: 2, detail: String::new() });
    evaluation.seconds = started.elapsed().as_secs_f64();
    Ok(evaluation)
}

/// A model folded into the table every device reads (`topics::table`):
/// each known word's rows' sums per class, each bucket's scalars, the end of the line's.
fn fold(learned: &Learned, way: &Way, calibration: &Calibrated, ids: &[String], options: &Options, precision: Precision) -> Table {
    let c = learned.classes;
    let dict = learned.model.dict();
    let nwords = dict.nwords();
    let eos = fasttext::dictionary::EOS;
    let mut words = Vec::new();
    let mut word_scores = Vec::new();
    for id in 0..nwords {
        let word = dict.get_word(id);
        if word == eos {
            continue;
        }
        words.push(sioul_core::spam::table::word_hash(word));
        let mut sums = vec![0f64; c];
        for &r in dict.get_subwords(id) {
            for (s, x) in sums.iter_mut().zip(&learned.scalars[r as usize * c..(r as usize + 1) * c]) {
                *s += f64::from(*x);
            }
        }
        word_scores.extend(sums.into_iter().map(|s| s as f32));
    }
    let first = nwords as usize * c;
    let buckets: Vec<f32> = (0..way.bucket as usize * c).map(|i| learned.scalars.get(first + i).copied().unwrap_or(0.0)).collect();
    let mut bias: Vec<f32> = match dict.get_id(eos) {
        Some(e) => learned.scalars[e as usize * c..(e as usize + 1) * c].to_vec(),
        None => vec![0.0; c],
    };
    // A class it never learned can never come: far below the others.
    for (b, &here) in bias.iter_mut().zip(&learned.present) {
        if !here {
            *b = -1e30;
        }
    }
    let mut table = Table {
        tokenizer: sioul_core::spam::tokenize::TOKENIZER,
        senders: sioul_core::topics::SENDERS,
        minn: options.minn,
        maxn: options.maxn,
        bucket: way.bucket,
        topics: ids.len(),
        none: c > ids.len(),
        words,
        word_scores,
        buckets,
        bias,
        temperature: calibration.temperature as f32,
        sure: sure_f32(&calibration.sure),
        maybe: options.maybe as f32,
        precision,
        meta: table::Meta { trained_at: now(), topics: ids.to_vec(), messages: 0, device: sioul_sync::lease::host_name(), lexicon: Some(options.spam.lexicon.clone()), ..table::Meta::default() },
    };
    table.sort();
    table
}

/// The way kept, folded in f32 and f16 and read back as every device reads
/// it, against fastText's predictions on the tested mail (500 at most);
/// the f16 table's topics and bands against the f32 one's on all of it.
fn fold_checked(learned: &Learned, data: &Data, way: &Way, calibration: &Calibrated, ids: &[String], options: &Options, judged: &[(usize, Judged)]) -> Fold {
    let wide = fold(learned, way, calibration, ids, options, Precision::F32);
    let (bytes_f32, bytes_f16) = (wide.to_bytes(), fold(learned, way, calibration, ids, options, Precision::F16).to_bytes());
    let (Ok(wide), Ok(half)) = (Table::from_bytes(&bytes_f32), Table::from_bytes(&bytes_f16)) else {
        return Fold { broken: true, ..Fold::default() };
    };
    let c = learned.classes;
    let (mut error_f32, mut error_f16, mut checked) = (0f64, 0f64, 0u64);
    for &(m, _) in judged.iter().take(500) {
        let line = data.mail[m].line(way.senders);
        let theirs = learned.model.predict(&line.join(" "), c, 0.0);
        if theirs.is_empty() {
            continue;
        }
        checked += 1;
        let (p32, p16) = (wide.probabilities(&line, 1.0), half.probabilities(&line, 1.0));
        for class in (0..c).filter(|&k| learned.present[k]) {
            let label = class_label(class, data.topics);
            let p = theirs.iter().find(|x| x.label == label).map_or(0.0, |x| f64::from(x.prob));
            error_f32 = error_f32.max((p32[class] - p).abs());
            error_f16 = error_f16.max((p16[class] - p).abs());
        }
    }
    let flips = judged
        .iter()
        .filter(|(m, _)| {
            let line = data.mail[*m].line(way.senders);
            let (a, b) = (wide.judge(&line), half.judge(&line));
            a.topic != b.topic || a.band != b.band
        })
        .count() as u64;
    Fold {
        bucket: way.bucket,
        words: wide.words.len() as u64,
        checked,
        error_f32,
        error_f16,
        flips,
        judged: judged.len() as u64,
        bytes_f16: bytes_f16.len() as u64,
        bytes_f32: bytes_f32.len() as u64,
        broken: error_f32 > FOLD_TOLERANCE,
    }
}

/// The centroid on a static embedding: each topic's centroid the mean of its
/// seeded messages' vectors, normalized; a message's score for a topic, its
/// vector (the mean of its tokens' rows) by that centroid; calibrated and
/// tested as fastText's way, with the threshold. Its fold: each token's
/// score per topic, in f16.
fn centroid(data: &Data, model: &StaticModel, options: &Options) -> (Measured, CentroidFold) {
    let k = data.topics;
    let vectors: HashMap<usize, Vec<f32>> = data.learned.iter().chain(&data.tested).filter_map(|&m| Some((m, model.mean(&data.mail[m].tokens)?))).collect();
    let centroids = |ids: &[usize]| -> Vec<Option<Vec<f64>>> {
        let mut sums = vec![vec![0f64; model.dim]; k];
        let mut counts = vec![0usize; k];
        for &m in ids {
            if let (Some(t), Some(v)) = (data.labels[m], vectors.get(&m)) {
                counts[t] += 1;
                for (s, x) in sums[t].iter_mut().zip(v) {
                    *s += f64::from(*x);
                }
            }
        }
        sums.into_iter()
            .zip(counts)
            .map(|(s, n)| {
                let norm = s.iter().map(|x| x * x).sum::<f64>().sqrt();
                (n > 0 && norm > 0.0).then(|| s.into_iter().map(|x| x / norm).collect())
            })
            .collect()
    };
    let score = |v: &[f32], centroids: &[Option<Vec<f64>>]| -> Vec<f64> { centroids.iter().map(|c| c.as_ref().map_or(f64::NEG_INFINITY, |c| v.iter().zip(c).map(|(a, b)| f64::from(*a) * b).sum())).collect() };
    let logits = |ids: &[usize], centroids: &[Option<Vec<f64>>]| -> Vec<(usize, Vec<f64>)> { ids.iter().filter_map(|&m| Some((m, score(vectors.get(&m)?, centroids)))).collect() };
    let fit_centroids = centroids(&data.fit);
    // The validation part's vectors: the fit and validation parts are in `learned`.
    let calibration = calibrated(data, &logits(&data.validation, &fit_centroids), Fit::Seeded, options);
    let all = centroids(&data.learned);
    let test_logits = logits(&data.tested, &all);
    let (measured, judged) = tested(data, "centroid".into(), Some(Method::Centroid), None, &calibration, &test_logits, options);
    // Its fold: per token and topic, `e_t · c`, in f16; a message's score, their mean.
    let s32 = sure_f32(&calibration.sure);
    let mut fold = CentroidFold { vocabulary: model.rows as u64, dim: model.dim as u64, judged: judged.len() as u64, bytes_f16: (model.rows * k * 2) as u64, tokenizer_bytes: model.tokenizer_bytes, ..CentroidFold::default() };
    let mut per_token: HashMap<u32, Vec<f64>> = HashMap::new();
    for ((m, f), (_, j)) in test_logits.iter().zip(&judged) {
        let tokens = &data.mail[*m].tokens;
        let mut half: Vec<f64> = all.iter().map(|c| if c.is_some() { 0.0 } else { f64::NEG_INFINITY }).collect();
        for &t in tokens {
            let scores = per_token.entry(t).or_insert_with(|| {
                all.iter().map(|c| c.as_ref().map_or(0.0, |c| f64::from(table::from_f16(table::to_f16(model.row(t).iter().zip(c).map(|(a, b)| f64::from(*a) * b).sum::<f64>() as f32))))).collect()
            });
            for (h, s) in half.iter_mut().zip(scores.iter()) {
                *h += s / tokens.len().max(1) as f64;
            }
        }
        let (p, q) = (table::softmax(f, calibration.temperature), table::softmax(&half, calibration.temperature));
        fold.error_f16 = p.iter().zip(&q).map(|(a, b)| (a - b).abs()).fold(fold.error_f16, f64::max);
        let again = table::judged(&q[..k], &s32, options.maybe);
        if again.topic != j.topic || again.band != j.band {
            fold.flips += 1;
        }
    }
    (measured, fold)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::synthetic;

    pub(crate) fn options() -> Options {
        let spam = train::Options { threads: 1, seed: 3, ..train::Options::default() };
        Options { spam, dim: 16, epochs: 20, min_count: 2, buckets: vec![2_000, 4_000], none_weights: vec![0.2, 0.5], ..Options::default() }
    }

    /// The stand-in corpus's trusted server.
    pub(crate) fn trusted() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([("home".to_string(), vec!["mx.example.org".to_string()])])
    }

    /// A stand-in home under `root`: topical mail kept in a Maildir of the
    /// account `account` (its own mail index: tests run side by side), ten of
    /// its spam in the inbox here though the spam filter's corpus keeps them
    /// in Junk, twelve replies of yours in Sent to the choir, whose address is
    /// a contact of the category "Chorale".
    pub(crate) fn home(root: &std::path::Path, account: &str) -> (Dirs, Sources) {
        let dirs = Dirs::under(&root.join("spam"));
        let mail = synthetic::topical(11, 900);
        let records: Vec<_> = mail.iter().enumerate().map(|(i, m)| synthetic::record_of(m, 1, i as u32 + 1)).collect();
        corpus::store(&dirs, &records).unwrap();
        let mut here = mail.clone();
        for m in here.iter_mut().filter(|m| m.role == Role::Junk).take(10) {
            m.folder = "INBOX";
            m.role = Role::Inbox;
        }
        here.extend(synthetic::replies(&mail, 12));
        let maildir = root.join("maildir");
        synthetic::store_maildir(&maildir, &here).unwrap();
        let contact = Contact {
            name: "Choir office".into(),
            emails: vec![sioul_core::contacts::Labeled { label: String::new(), value: "contact@chorale.example".into() }],
            categories: vec!["Chorale".into()],
            ..Contact::default()
        };
        let maildirs = vec![Maildir { account: account.into(), root: maildir, folders: Vec::new(), trusted: vec!["mx.example.org".into()] }];
        (dirs, Sources { maildirs, contacts: vec![contact], own: vec!["owner@example.org".into()], presets: None })
    }

    /// Every message kept here, once: the folders and the category seed
    /// their topics, Sent counts (the category by its recipients), what the
    /// spam filter's labels call spam stays out though it is in the inbox,
    /// Junk is never read; the small folder seeds nothing.
    #[test]
    fn mail_kept_here_seeds_topics() {
        let root = std::env::temp_dir().join(format!("sioul-topics-seeds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (dirs, sources) = home(&root, "seeds");
        let read = read(&dirs, &trusted(), &sources, &options(), false, None, &mut |_| {}, &Cancel::new()).unwrap();
        let (seeded, labels) = seeded(&read, &options());
        let names: BTreeSet<&str> = seeded.proposal.topics.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, BTreeSet::from(["Chorale", "Impôts", "Santé"]), "{seeded:#?}");
        let chorale = seeded.proposal.topics.iter().find(|t| t.name == "Chorale").unwrap();
        assert_eq!(chorale.seeds.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![Kind::Folder, Kind::Category]);
        let r = &seeded.reading;
        assert_eq!((r.yours, r.category_yours, r.folders_left_out), (12, 12, 1), "{r:?}");
        assert!(r.spam >= 10 && r.messages > 600 && r.folder_named > 300 && r.category_named > 12, "{r:?}");
        assert!(seeded.named_yours >= 12);
        assert_eq!(seeded.left_out.small, 1, "the small folder");
        assert!(seeded.left_out.system >= 3, "Junk, Archive and Sent: {:?}", seeded.left_out);
        assert_eq!(labels.iter().filter(|l| l.is_some()).count() as u64, seeded.named);
        assert!(read.mail.iter().all(|m| m.words.is_empty()), "headers alone");
        // A stand-in topic preset of the tax office's domain: its mail counted, its seed joined with the folder of its name.
        let presets: sioul_core::topics::presets::TopicPresets = serde_json::from_str(r#"{"checked": "2026-11-05", "topics": [{"id": "taxes", "name": {"en": "Taxes", "fr": "Impôts"}, "area": "admin", "rules": {"domains": [{"domain": "impots.example", "source": "https://impots.example/"}]}}]}"#).unwrap();
        let sources = Sources { presets: Some(std::sync::Arc::new(sioul_core::topics::presets::Matcher::new(&presets, &sioul_core::presets::Presets::default()))), ..sources };
        let options = Options { language: "fr".into(), ..options() };
        let read = super::read(&dirs, &trusted(), &sources, &options, false, None, &mut |_| {}, &Cancel::new()).unwrap();
        let counted = &read.reading.presets["taxes"];
        assert!(counted.by_domain >= 90 && counted.by_site == 0, "{counted:?}");
        let (seeded, _) = super::seeded(&read, &options);
        let impots = seeded.proposal.topics.iter().find(|t| t.name == "Impôts").unwrap();
        assert_eq!(impots.seeds.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![Kind::Preset, Kind::Folder], "{impots:#?}");
        let _ = std::fs::remove_dir_all(root);
    }

    /// Every way trained and tested in time order on the stand-in corpus:
    /// numbers for each, a decision, the fold within fastText's prediction in
    /// f32, churn measured.
    #[test]
    fn topics_are_measured_in_time_order() {
        let root = std::env::temp_dir().join(format!("sioul-topics-eval-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (dirs, sources) = home(&root, "eval");
        let options = options();
        let read = read(&dirs, &trusted(), &sources, &options, true, None, &mut |_| {}, &Cancel::new()).unwrap();
        let evaluation = evaluate(&dirs, &read, &options, None, &mut |_| {}, &Cancel::new()).unwrap();
        assert_eq!(evaluation.seeded.proposal.topics.len(), 3);
        // Threshold, the best none share, without senders, the other bucket count.
        assert_eq!(evaluation.ways.len(), 4, "{:#?}", evaluation.ways);
        assert_eq!(evaluation.none_weights.len(), 2);
        let decided = evaluation.decided.as_ref().unwrap();
        assert_eq!(decided.sure.len(), 3);
        let kept = evaluation.ways.iter().find(|w| w.name == decided.way).unwrap();
        assert!(kept.test.macro_f1.unwrap() > 0.6, "{kept:#?}");
        assert!(kept.topics.iter().all(|t| !t.id.is_empty() && t.learned > 0));
        let fold = evaluation.fold.as_ref().unwrap();
        assert!(!fold.broken && fold.error_f32 < FOLD_TOLERANCE && fold.checked > 0, "{fold:?}");
        assert!(fold.bytes_f16 < fold.bytes_f32);
        assert!(evaluation.churn.is_some());
        assert!(evaluation.split.test_from >= evaluation.split.validation_from);
        assert!(!dirs.cache.read_dir().map(|mut d| d.any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().starts_with("topics-")))).unwrap_or(false), "no learning file left");
        let _ = std::fs::remove_dir_all(root);
    }

    /// The centroid on a stand-in static model whose rows tell the topics' words apart.
    #[test]
    fn the_centroid_is_measured_with_a_model() {
        let root = std::env::temp_dir().join(format!("sioul-topics-centroid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (dirs, sources) = home(&root, "centroid");
        let folder = root.join("model");
        let mut pieces: Vec<(String, f64)> = vec![("<unk>".into(), 0.0)];
        let mut rows: Vec<Vec<f32>> = vec![vec![0.0; 4]];
        for (t, words) in synthetic::TOPIC_WORDS.iter().enumerate() {
            for w in *words {
                pieces.push((format!("\u{2581}{w}"), -5.0));
                let mut row = vec![0.1f32; 4];
                row[t] = 1.0;
                rows.push(row);
            }
        }
        let pieces_ref: Vec<(&str, f64)> = pieces.iter().map(|(p, s)| (p.as_str(), *s)).collect();
        crate::model2vec::write_stand_in(&folder, &pieces_ref, &rows, 0).unwrap();
        let mut model = StaticModel::load(&folder).unwrap();
        let options = Options { compare: false, ..options() };
        let read = read(&dirs, &trusted(), &sources, &options, true, Some(&mut model), &mut |_| {}, &Cancel::new()).unwrap();
        let evaluation = evaluate(&dirs, &read, &options, Some(&model), &mut |_| {}, &Cancel::new()).unwrap();
        assert_eq!(evaluation.ways.len(), 1, "the threshold alone");
        let centroid = evaluation.centroid.as_ref().unwrap();
        assert!(centroid.test.macro_f1.unwrap() > 0.5, "{centroid:#?}");
        let fold = evaluation.centroid_fold.as_ref().unwrap();
        assert!(fold.error_f16 < 1e-2 && fold.bytes_f16 == (rows.len() * 3 * 2) as u64, "{fold:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    /// A grouping's words: no placeholder, no style, nothing quoted, no attribution.
    #[test]
    fn grouping_words_leave_the_noise_out() {
        let lexicon = Lexicon::cached(&train::Options::default().lexicon);
        let noise = noise_words(&lexicon);
        let text = "Votre facture de téléphone est disponible sur https://www.operateur.example/compte pour 12,50 €.\ntable width center important nbsp\n\nLe 3 nov. 2026, Anne Martin a écrit :\n> merveilleux souvenirs de vacances\n";
        let all = spamcore::tokens_with(&lexicon, "Facture", text).words;
        assert!(all.iter().any(|w| w.starts_with('_')), "the spam model's placeholders are there: {all:?}");
        let words = grouping_words(&lexicon, &noise, "Facture", text);
        assert!(!words.is_empty() && words.iter().all(|w| !w.starts_with('_') && !noise.contains(w)), "{words:?}");
        let stems = |text: &str| spamcore::tokens_with(&lexicon, "", text).words;
        for gone in ["width", "center", "important", "nbsp", "merveilleux", "vacances", "ecrit", "anne"] {
            for stem in stems(gone) {
                assert!(!words.contains(&stem), "{gone} ({stem}) in {words:?}");
            }
        }
        assert!(stems("facture").iter().all(|s| words.contains(s)), "{words:?}");
    }

    /// The temperature fits: scores that are too sure are cooled, and thresholds need precision.
    #[test]
    fn temperature_and_thresholds() {
        // Right three times in four with scores far apart: a temperature above 1.
        let scored: Vec<(Vec<f64>, usize)> = (0..40).map(|i| (vec![8.0, 0.0], if i % 4 == 0 { 1 } else { 0 })).collect();
        assert!(temperature(&scored) > 2.0);
        let options = Options::default();
        // Topic 0: ten right from 0.85, five wrong from 0.80 to 0.84: one wrong
        // still keeps the precision at 10 in 11, two do not.
        let mut judged: Vec<(Vec<f64>, Option<usize>)> = (0..10).map(|i| (vec![0.85 + i as f64 * 0.01, 0.1], Some(0))).collect();
        judged.extend((0..5).map(|i| (vec![0.80 + i as f64 * 0.01, 0.15], None)));
        let sure = thresholds(&judged, 2, &options);
        assert!((sure[0].unwrap() - 0.84).abs() < 1e-9, "{sure:?}");
        // Only confusions between topics counted: mail no seed names leaves it at the lowest.
        let seeded = thresholds(&judged, 2, &Options { count_unseeded: false, ..Options::default() });
        assert!((seeded[0].unwrap() - 0.85).abs() < 1e-9, "{seeded:?}");
        assert_eq!(sure[1], None, "never on top");
        // Precise from low: never under the floor.
        let low: Vec<(Vec<f64>, Option<usize>)> = (0..10).map(|i| (vec![0.5 + i as f64 * 0.01, 0.4], Some(0))).collect();
        assert_eq!(thresholds(&low, 2, &options)[0], Some(0.7));
    }
}

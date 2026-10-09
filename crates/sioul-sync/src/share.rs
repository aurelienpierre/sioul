// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What only this computer keeps, shared with your other computers through a
//! folder your sync already carries: Nextcloud, Dropbox, Syncthing
//! (docs/database.md, "The log").
//!
//! Mail, contacts, the agenda and tasks travel by their servers; projects and
//! notes by the folder that holds them. The rest lives in small files here:
//! settings, who may write to you, ties, time, drafts, health, lists kept
//! on this computer. Each computer has a name, a UUID made once,
//! and appends each change it finds in them, as one sealed record, to its own
//! file in the folder (`<computer>-<round>.jsonl`): one writer per file, so
//! the sync never makes conflicted copies.
//!
//! Files stay the truth: Sioul reads and writes them as before. A change is
//! found by comparing them with what they held at the last look; a record from
//! another computer is written into them. For each entry (a setting, a line of
//! a list, a session, a whole file) the record with the later clock wins: a
//! hybrid logical clock, the time made strictly increasing here and never
//! behind a record already read. A change is dated by its file's time, so a
//! change made here before another computer's, but found after, still loses.
//!
//! Sealed: each record is encrypted here (XChaCha20-Poly1305) with a key made
//! from a passphrase (Argon2id) and kept in each computer's keyring. The
//! folder and its server see which computer wrote, when and how much, never
//! what.
//!
//! What is shared comes in parts (`PARTS`: settings, senders, the spam
//! filter's table and label logs, health, time, drafts, projects, lists,
//! notes, papers), each switched on or off on each device. Notes and papers travel
//! one file at a time, each sealed apart in the folder (`blobs`), their
//! records saying which content each file holds; two devices changing one
//! file keep both versions. The spam filter's table is sealed apart too: one
//! file of a few megabytes, read in every exchange (`SPAM_TABLE`). Before another
//! device's change is written into a file here, the file as it was is kept on
//! this device (`history`), to be put back.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use sioul_core::config::Config;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The keyring's name for the key made from the passphrase.
pub const KEY_NAME: &str = "Sharing key";

/// Between the parts of a setting's name.
const SEP: char = '\u{1f}';
/// Before an element's identity, in a list split by elements.
const MARK: char = '\u{1e}';
/// Files larger than this stay here.
const LARGEST: u64 = 16 << 20;
/// What a computer appends past its last full round before it opens a new
/// one, restating every entry it holds (`new_round`): old rounds can then go.
const ROUND_SIZE: u64 = 1 << 20;
/// A computer's file past this size is continued in a new round of its own,
/// nothing restated (docs/database.md, "The log"): a server sent the file whole
/// at each change (WebDAV has no append) is sent a small one, never the opening
/// of a full round (on a computer, a megabyte or two) again and again.
const SEGMENT: u64 = 32 << 10;
/// While the connection is metered, or measured slow (`set_frugal`), a full
/// round, a megabyte or two sent at once, waits, as long as this computer
/// appended less than this since the last one.
const FRUGAL_ROUND_SIZE: u64 = 4 << 20;

/// The sharings (by their memory) whose connection is metered or slow: full
/// rounds wait there (`FRUGAL_ROUND_SIZE`).
static FRUGAL: std::sync::Mutex<BTreeSet<PathBuf>> = std::sync::Mutex::new(BTreeSet::new());

/// Said by the window before its exchanges: the connection is metered, or
/// measured slow (`remote::Sending::slow`); a full round, sent whole, then
/// waits for a better one, within `FRUGAL_ROUND_SIZE`. `memory`: the sharing's.
pub fn set_frugal(memory: &Path, on: bool) {
    if let Ok(mut frugal) = FRUGAL.lock() {
        if on {
            frugal.insert(memory.to_path_buf());
        } else {
            frugal.remove(memory);
        }
    }
}

fn frugal(memory: &Path) -> bool {
    FRUGAL.lock().is_ok_and(|f| f.contains(memory))
}
/// Entries taken out are remembered this long, so an old copy does not bring them back.
const TOMBSTONE_DAYS: i64 = 90;
/// A computer silent this long no longer holds back the removal of old rounds.
const SILENT_DAYS: i64 = 180;
/// How long a file must stay empty, or gone, before its entries count as taken
/// out: one being written, a full disk or a crash never takes everything out
/// everywhere at once.
const EMPTIED_WAIT: i64 = 10 * 60_000;
/// A line of a computer's records longer than this is none (the longest, a
/// file whole of 16 MiB sealed in a line, is about 30 MiB): skipped as broken.
const LONGEST_LINE: usize = 32 << 20;
/// Records are read this much at a time, never a whole file.
const READ_CHUNK: usize = 8 << 20;
/// More files sealed apart than this gone from a folder at once (or three and
/// more, past a fifth of it) are held, said, until you say to take them out.
const MASS: usize = 10;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// The beginning of the names of folders whose files are sealed apart (`Shape::Files`).
const FILES: &str = "files/";

/// How a file divides into entries.
#[derive(Debug, Clone, Copy)]
pub enum Shape {
    /// The file whole: a draft, an invoice.
    Whole,
    /// One entry per line: the sender lists.
    Lines,
    /// One entry per setting of a TOML file.
    Toml(&'static Rules),
    /// The file whole, sealed apart in the folder (`blobs`), its record saying
    /// which content it holds: notes, papers, their pictures and PDFs; the
    /// spam filter's table, a single file (`SPAM_TABLE`).
    Files,
}

/// How a TOML file divides, beyond one entry per setting.
#[derive(Debug)]
pub struct Rules {
    /// Lists whose elements are entries.
    pub keyed: &'static [Keyed],
    /// Tables shared whole, their fields together; `*` stands for any name.
    pub whole: &'static [&'static str],
    /// Settings that stay on each computer.
    pub local: &'static [&'static str],
    /// How the file divides in format 2 (`FORMAT`), when it differs: its
    /// lists named by ids, lists shared whole until then divided element by
    /// element. A device writes it once every device in the sharing reads it
    /// (`format_open`); until then, these rules (format 1), which an older
    /// Sioul reads.
    pub next: Option<&'static Rules>,
}

impl Rules {
    /// The rules of a file in a format: format 2's from 2 on, when they differ.
    pub fn at(&'static self, format: u32) -> &'static Rules {
        match self.next {
            Some(next) if format >= 2 => next,
            _ => self,
        }
    }
}

/// A list whose elements are entries, each named by some of its fields (none:
/// the element itself), less the fields that stay on each computer.
#[derive(Debug)]
pub struct Keyed {
    /// Its name; within tables, its path, `*` standing for any name
    /// (`words.*.*.add`: each list's words you added).
    pub list: &'static str,
    pub by: &'static [&'static str],
    pub local: &'static [&'static str],
    /// Other names the list may have in a file, its entries the same: the
    /// records always say `list`, and a file keeps the name it has. A file
    /// with neither, of another name than its record's, takes the first.
    pub also: &'static [&'static str],
    /// Format 2: each element named by its own `id`, given when it is made
    /// (`sioul_core::ids`), so that it stays one element whatever changes in
    /// it; an element without one (made by an older Sioul, or by hand) by an
    /// id derived from what it holds, the same on every device (`names`).
    /// `by` then says how format 1 named them, to read its records.
    pub ids: bool,
}

/// A keyed list as format 1 names its elements: by `by`, or by themselves.
const fn keyed(list: &'static str, by: &'static [&'static str]) -> Keyed {
    Keyed { list, by, local: &[], also: &[], ids: false }
}

/// A keyed list whose elements are named by their ids (format 2); `by`: as format 1 named them.
const fn with_ids(list: &'static str, by: &'static [&'static str]) -> Keyed {
    Keyed { list, by, local: &[], also: &[], ids: true }
}

/// Where things are on this computer, and how text reads on its screen.
const CONFIG_LOCAL: &[&str] = &["case_store", "known_senders", "blocked_senders", "reading", "history_weeks", "letters.inbox", "dnd.background"];
static CONFIG_RULES: Rules = Rules { keyed: &[Keyed { list: "account", by: &["id"], local: &["maildir", "history_weeks"], also: &[], ids: false }], whole: &[], local: CONFIG_LOCAL, next: Some(&CONFIG_RULES_2) };
/// Format 2: pinned sites, routines and task kinds one each, by their ids;
/// days off one each; the words you added to a list and those you took
/// away, one each (docs/words.md). Until then each of these lists travels
/// whole, and two devices adding to one in the same exchange keep only the
/// later list.
static CONFIG_RULES_2: Rules = Rules {
    keyed: &[
        Keyed { list: "account", by: &["id"], local: &["maildir", "history_weeks"], also: &[], ids: false },
        with_ids("site", &["id"]),
        with_ids("routine", &["id"]),
        with_ids("tasks.kind", &["id"]),
        with_ids("time_off", &[]),
        keyed("words.*.*.add", &[]),
        keyed("words.*.*.remove", &[]),
        // A named list's names and their words: `add = { "Ma Banque" = ["mabanque.example"] }`.
        keyed("words.*.*.add.*", &[]),
    ],
    whole: &[],
    local: CONFIG_LOCAL,
    next: None,
};
static HEALTH_RULES: Rules = Rules {
    keyed: &[keyed("prescription", &["id"]), keyed("medicine", &["id"])],
    whole: &[],
    // An older Sioul's folder of a watch's files (until 8 October 2026), a
    // path on its own computer: never carried, as before.
    local: &["watch_folder"],
    next: None,
};
static LINKS_RULES: Rules = Rules { keyed: &[keyed("link", &[])], whole: &[], local: &[], next: None };
static PORCH_RULES: Rules = Rules { keyed: &[], whole: &["done.*"], local: &[], next: None };
static MONEY_RULES: Rules = Rules { keyed: &[keyed("ignored", &[])], whole: &[], local: &[], next: None };
static TODAY_RULES: Rules = Rules { keyed: &[keyed("aside", &[])], whole: &[], local: &[], next: None };
static TIME_RULES: Rules = Rules { keyed: &[keyed("session", &["start", "task", "project"])], whole: &[], local: &[], next: Some(&TIME_RULES_2) };
/// Format 2: each session by its id, so that one changed on two devices stays
/// one; a session without one by its start, task and project, as before.
static TIME_RULES_2: Rules = Rules { keyed: &[with_ids("session", &["start", "task", "project"])], whole: &[], local: &[], next: None };
/// The hours' overrides and the two pauses (`sioul_core::quiet`); format 2:
/// the lighter days one each, by their date.
static QUIET_RULES: Rules = Rules { keyed: &[], whole: &[], local: &[], next: Some(&QUIET_RULES_2) };
static QUIET_RULES_2: Rules = Rules { keyed: &[keyed("lighter", &[])], whole: &[], local: &[], next: None };
/// How each day went (`sioul_core::reviews`): a date's review at the end of
/// work and the one before sleep travel whole, so that two devices answering
/// the same one keep the later answer, never a mix of both; the weather field by field.
static REVIEWS_RULES: Rules = Rules { keyed: &[], whole: &["*.work", "*.night"], local: &[], next: None };
static PLAIN_RULES: Rules = Rules { keyed: &[], whole: &[], local: &[], next: None };
/// Do-not-disturb's switch (docs/do-not-disturb.md): a table per device, each
/// written by its own device alone, shared whole.
static DND_RULES: Rules = Rules { keyed: &[], whole: &["device.*"], local: &[], next: None };
/// Who may reach you during do-not-disturb: one person each, by its id.
static DND_PEOPLE_RULES: Rules = Rules { keyed: &[keyed("person", &["id"])], whole: &[], local: &[], next: None };
/// Each dose that fell due (`sioul_core::doses`): one entry per field, each
/// device's opening apart, and each device's answer whole, so that two
/// devices never write one entry and an answer never travels in halves.
static DOSES_RULES: Rules = Rules { keyed: &[], whole: &["dose.*.answer.*"], local: &[], next: None };
// Projects and budgets, at the notes folder's root, when they travel here (`share_projects`).
// The projects' records keep their first names, `notes/sioul-cases.toml` and
// `case`, which every version reads; a file renamed `sioul-projects.toml`, its
// tables `[[project]]`, reads and is written under them (`sioul_core::projects`).
static PROJECTS_RULES: Rules = Rules { keyed: &[Keyed { list: "case", by: &["id"], local: &[], also: &["project"], ids: false }], whole: &[], local: &[], next: None };
// The bank's movements: each account by its id, each movement by its account and the bank's own id.
static BANK_RULES: Rules = Rules { keyed: &[keyed("account", &["id"]), keyed("movement", &["account", "id"])], whole: &[], local: &[], next: None };
static LEDGER_RULES: Rules = Rules {
    keyed: &[
        keyed("budget", &["id"]),
        keyed("preset", &["id"]),
        keyed("reserve", &["id"]),
        keyed("bank_account", &["id"]),
        keyed("assign", &["account", "movement"]),
        // No name of their own: each one is itself, so two devices adding some keep both.
        keyed("line", &[]),
        keyed("cover", &[]),
        keyed("mail_rule", &[]),
        keyed("split", &[]),
    ],
    whole: &[],
    local: &[],
    next: Some(&LEDGER_RULES_2),
};
/// Format 2: lines, covers, mail rules, splits and presets each by its id, so
/// that two the same stay two and one changed on two devices stays one; one
/// without an id (made by an older Sioul, or by hand) by an id derived from
/// what it holds.
static LEDGER_RULES_2: Rules = Rules {
    keyed: &[
        keyed("budget", &["id"]),
        with_ids("preset", &["id"]),
        keyed("reserve", &["id"]),
        keyed("bank_account", &["id"]),
        keyed("assign", &["account", "movement"]),
        with_ids("line", &[]),
        with_ids("cover", &[]),
        with_ids("mail_rule", &[]),
        with_ids("split", &[]),
    ],
    whole: &[],
    local: &[],
    next: None,
};
static CONTRACTS_RULES: Rules = Rules { keyed: &[keyed("contract", &["id"])], whole: &[], local: &[], next: None };
static PAPERS_RULES: Rules = Rules { keyed: &[keyed("paper", &["id"])], whole: &[], local: &[], next: None };
/// In the notes folder, what the other parts carry: projects, budgets, the
/// bank, contracts (projects), the papers' wallet and its folder (papers).
const NOT_NOTES: &[&str] = &[
    sioul_core::projects::FILE,
    sioul_core::projects::OLD_FILE,
    sioul_core::projects::BEFORE_RENAME,
    sioul_core::budget::LEDGER,
    sioul_core::bank::MANIFEST,
    sioul_core::contracts::MANIFEST,
    sioul_core::papers::MANIFEST,
    sioul_core::papers::FOLDER,
];

/// The parts of what is shared, each switched on or off on each device
/// (docs/database.md, "Parts"): settings and accounts, senders, the calls
/// your phones screened, the messages from your phones' notifications, the
/// spam filter's table and label logs, health, time, drafts and invoices,
/// projects and money, lists kept here, notes, papers.
/// (texts: the part "texts", SMS phase b, after the phone's messages.)
pub const PARTS: [&str; 13] = ["settings", "senders", "calls", sioul_core::phonemsgs::PART, sioul_core::texts::PART, "spam", "health", "time", "drafts", "projects", "lists", "notes", "papers"];

/// Stores an older Sioul shared and this one no longer has: a Garmin
/// watch's days and the memory of its offers, taken out on 8 October 2026
/// (docs/health.md, "Your watch"). A device on an older version may still
/// send their changes: they are left aside, neither written nor kept
/// waiting, and the rest of the exchange goes on. The files stay where they
/// are on every device. texts: the first build of the texts, never
/// released, kept them in the state folder (`state/texts/`).
const RETIRED: &[&str] = &["data/watch/", "state/watch-offers.json", "state/texts/"];

/// Whether a record's key names a store that left (`RETIRED`).
fn retired(key: &str) -> bool {
    let file = file_of(key);
    RETIRED.iter().any(|store| if store.ends_with('/') { file.starts_with(store) } else { file == *store })
}

/// The phones' logs of the calls they screened (`sioul_core::calls`): each
/// phone writes its own file, a line per call, taken out after a month by
/// that phone alone; every device reads them all, so that a computer's Porch
/// lists what your phones declined. Never a blocked caller's call.
pub const CALLS_LOG: &str = "state/calls/log/";
/// The calls marked Seen, each device's own file, a line per press: seen on
/// one device, gone from the Porch of every device.
pub const CALLS_SEEN: &str = "state/calls/seen/";
/// Where the calls' part keeps its files in the records: remembered beside
/// the notes (`files.json`, not `memory.json`), never kept as an earlier
/// version nor copied aside when sharing starts, so that a call leaves every
/// device after its month (`calls_store`).
const CALLS: &str = "state/calls/";

/// The phones' logs of the messages their notifications brought
/// (`sioul_core::phonemsgs`), for the apps you switched on there: each phone
/// writes its own file, a line per message, padded, taken out after a week
/// by that phone alone; every device reads them all, so that a computer's
/// Porch says what came. Never a code, never a blocked sender's.
pub const PHONE_MESSAGES_LOG: &str = "state/phone-messages/log/";
/// The messages marked Seen, each device's own file: seen on one device,
/// gone from the Porch of every device.
pub const PHONE_MESSAGES_SEEN: &str = "state/phone-messages/seen/";
/// Where the phones' messages keep their files in the records: as the
/// calls', remembered beside the notes, never kept as an earlier version nor
/// copied aside when sharing starts (`calls_store`).
const PHONE_MESSAGES: &str = "state/phone-messages/";

// texts: SMS phase (b) (docs/texts.md, docs/database.md "Texts"). Each
// store a folder of lines in the data folder (an archive), one writer per
// file, every line sealed at rest by its writer (`textseal`) and again in
// the folder; media travel as blobs, outside the records.
/// Each phone's texts, the whole history (`sioul_core::texts::Text`).
pub const TEXTS_LOG: &str = "data/texts/log/";
/// Each computer's texts to send, a random key each.
pub const TEXTS_SEND: &str = "data/texts/send/";
/// Each phone's word on each text it was asked to send.
pub const TEXTS_OUTCOME: &str = "data/texts/outcome/";
/// Where the texts keep their files in the records: as the calls', remembered
/// beside the notes, never kept as an earlier version nor copied aside (`calls_store`).
const TEXTS: &str = "data/texts/";

/// A store, a file or an entry of the calls' part, or of the phones'
/// messages, by its name in the records: logs of lines that leave every
/// device after their time.
fn calls_store(name: &str) -> bool {
    // texts: their stores too.
    name.starts_with(CALLS) || name.starts_with(PHONE_MESSAGES) || name.starts_with(TEXTS)
}

/// The spam filter's table (`sioul_core::spam::table`), made by a training
/// on a computer, read by every device. Sealed apart, as its content's hash:
/// a new table is sent once, and the records stay small (whole in a record,
/// its two megabytes would go again in every round its computer starts). A
/// single file of Sioul's own, in its private folders: read in every
/// exchange, a phone's background step's too. Never the language model nor
/// the corpus beside it: they stay on the computer that made them.
pub const SPAM_TABLE: &str = "files/spam/table.bin";

/// The spam filter's label logs (`sioul_core::spam::labels`): each device
/// writes its own file, a line per act, never rewritten; every device reads
/// them all, so that a message you said is not spam on one is never flagged
/// again on another, and the training on your computer learns from what you
/// said on your phone. Small, read in every exchange.
pub const SPAM_LABELS: &str = "state/spam/labels/";
/// What your own spam filter moved into a Junk folder, each device's own
/// file: no word of yours; the Porch's lane of its catches reads it, and the
/// training, which learns from it until you say otherwise.
pub const SPAM_MOVED: &str = "state/spam/moved/";
/// What your own spam filter flagged where it is, each device's own file:
/// the training's, which learns from it until you say otherwise.
pub const SPAM_FLAGGED: &str = "state/spam/flagged/";

/// Whether a device that never chose shares a part: what was shared before
/// parts had switches, everything but notes and papers, and projects as the
/// setting all devices followed then said (`share_projects`); the spam
/// filter's table, which every device reads. Never the phones' messages,
/// newer than the switches: each device shares them once you turn them on there.
pub fn shared_by_default(part: &str, config: &Config) -> bool {
    match part {
        "projects" => config.share_projects,
        "notes" | "papers" => false,
        // A notification's words leave the phone only once you say so, on each device.
        sioul_core::phonemsgs::PART => false,
        // texts: read and sent through your phone, once you say so on each device.
        sioul_core::texts::PART => false,
        _ => true,
    }
}

/// A file, or a folder of files, that is shared.
#[derive(Debug, Clone)]
pub struct Store {
    /// Its name in the records, the same on every computer: "config/config.toml", "data/drafts/".
    pub name: String,
    pub path: PathBuf,
    /// A folder: each file in it, and below, on its own.
    pub folder: bool,
    pub shape: Shape,
    /// In a folder: names left out (of files sealed apart, at its top only).
    pub skip: &'static [&'static str],
    /// The part it belongs to (`PARTS`).
    pub part: &'static str,
}

/// The folders Sioul keeps its files in.
pub struct Roots {
    pub config: PathBuf,
    pub data: PathBuf,
    pub state: PathBuf,
}

impl Roots {
    pub fn here() -> Roots {
        Roots { config: sioul_core::config::config_dir(), data: sioul_core::config::data_dir(), state: sioul_core::config::state_dir() }
    }
}

/// What is shared, each part as a device that never chose shares it (`shared_by_default`).
pub fn stores(config: &Config, roots: &Roots) -> Vec<Store> {
    stores_of(config, roots, &|part| shared_by_default(part, config))
}

/// The format of what travels, as this Sioul reads and writes it
/// (docs/database.md, "The format of what travels"). 1: as Sioul 0.0.4 and
/// every earlier one wrote it. 2: money lines, covers, mail rules, splits,
/// presets and time sessions each named by its own id (`Keyed::ids`);
/// pinned sites, routines, task kinds, days off, the words you added or took
/// away and the lighter days one each, where they travelled whole. Each
/// device says it in its entry (`devices::Entry::format`), and writes format
/// 2 once every device in the sharing says it (`format_open`): an older
/// Sioul, which reads format 1 only, is never sent what it cannot read.
pub const FORMAT: u32 = 2;

/// The format each part is written in at `FORMAT`: 2 for the parts whose
/// records changed (settings, time, projects and money), 1 for the others.
/// Each record says its part's format when it is not 1 (`Change::f`).
pub fn part_format(part: &str) -> u32 {
    match part {
        "settings" | "time" | "projects" => 2,
        _ => 1,
    }
}

/// What is shared of the parts `shares` keeps. Not shared: caches, how far
/// each sync got, how pages are shown on this screen, the site notices of this
/// computer's browser, your own PGP keys (secret keys never leave the computer
/// they were made on).
pub fn stores_of(config: &Config, roots: &Roots, shares: &dyn Fn(&str) -> bool) -> Vec<Store> {
    let (c, d, s) = (&roots.config, &roots.data, &roots.state);
    let file = |part: &'static str, name: &str, path: PathBuf, shape: Shape| Store { name: name.into(), path, folder: false, shape, skip: &[], part };
    let folder = |part: &'static str, name: &str, path: PathBuf, shape: Shape, skip: &'static [&'static str]| Store { name: name.into(), path, folder: true, shape, skip, part };
    let senders = |chosen: &Option<String>, name: &str| chosen.as_deref().map_or_else(|| c.join(name), sioul_core::config::expand_home);
    let local = sioul_core::vdir::LOCAL;
    let mut stores = vec![
        file("settings", "config/config.toml", c.join("config.toml"), Shape::Toml(&CONFIG_RULES)),
        file("senders", "config/known-senders.txt", senders(&config.known_senders, "known-senders.txt"), Shape::Lines),
        file("senders", "config/blocked-senders.txt", senders(&config.blocked_senders, "blocked-senders.txt"), Shape::Lines),
        file("senders", "config/safe-senders.txt", c.join("safe-senders.txt"), Shape::Lines),
        file("senders", "config/neutral-senders.txt", c.join("neutral-senders.txt"), Shape::Lines),
        file("senders", "config/restricted-senders.txt", c.join("restricted-senders.txt"), Shape::Lines),
        // Who may reach you during do-not-disturb, beside the mail lists (docs/do-not-disturb.md).
        file("senders", "config/dnd-people.toml", c.join(sioul_core::everywhere::PEOPLE_FILE), Shape::Toml(&DND_PEOPLE_RULES)),
        file("settings", "data/links.toml", d.join("links.toml"), Shape::Toml(&LINKS_RULES)),
        file("health", "data/health.toml", d.join("health.toml"), Shape::Toml(&HEALTH_RULES)),
        // Each day's own meals, naps and nights: one entry per field of a block of a day.
        file("health", "data/health-days.toml", d.join("health-days.toml"), Shape::Toml(&PLAIN_RULES)),
        file("time", "data/time/running.toml", d.join("time").join("running.toml"), Shape::Whole),
        folder("time", "data/time/", d.join("time"), Shape::Toml(&TIME_RULES), &["running.toml"]),
        // How each day went, with the plan it is the outcome of (today.toml, quiet.toml, the sessions):
        // every device that plans learns the same (docs/reviews.md).
        folder("time", "data/reviews/", d.join("reviews"), Shape::Toml(&REVIEWS_RULES), &[]),
        folder("drafts", "data/drafts/", d.join("drafts"), Shape::Whole, &[]),
        folder("drafts", "data/invoices/", d.join("invoices"), Shape::Whole, &[]),
        folder("senders", "data/pgp/others/", d.join("pgp").join("others"), Shape::Whole, &[]),
        folder("lists", "data/calendars/local/", d.join("calendars").join(local), Shape::Whole, &[]),
        folder("lists", "data/contacts/local/", d.join("contacts").join(local), Shape::Whole, &[]),
        file("settings", "state/porch.toml", s.join("porch.toml"), Shape::Toml(&PORCH_RULES)),
        file("settings", "state/money.toml", s.join("money.toml"), Shape::Toml(&MONEY_RULES)),
        // Do-not-disturb's switch, pressed on any device (docs/do-not-disturb.md).
        file("settings", "state/do-not-disturb.toml", s.join(sioul_core::everywhere::SWITCH_FILE), Shape::Toml(&DND_RULES)),
        file("time", "state/today.toml", s.join("today.toml"), Shape::Toml(&TODAY_RULES)),
        file("time", "state/quiet.toml", s.join("quiet.toml"), Shape::Toml(&QUIET_RULES)),
        file("time", "state/stopped.toml", s.join("stopped.toml"), Shape::Toml(&PLAIN_RULES)),
        file("health", "state/health-state.toml", s.join("health-state.toml"), Shape::Toml(&PLAIN_RULES)),
        // Each dose that fell due and the answers your devices captured (docs/health.md,
        // "Doses as records"): a file of its own, which an older Sioul leaves alone.
        file("health", "state/health-doses.toml", s.join(sioul_core::doses::FILE), Shape::Toml(&DOSES_RULES)),
        folder("senders", "state/shield/", s.join("shield"), Shape::Whole, &[]),
        folder("lists", "state/dav/local/", s.join("dav").join(local), Shape::Whole, &[]),
        // The spam filter's table alone: its language model and its corpus, beside it, stay here.
        file("spam", SPAM_TABLE, d.join("spam").join("table.bin"), Shape::Files),
        // What you said is spam or not, what your own filter moved and what it
        // flagged: each device's own log (`<device>.jsonl`), a line each,
        // every device's read.
        folder("spam", SPAM_LABELS, s.join("spam").join(sioul_core::spam::labels::FOLDER), Shape::Lines, &[]),
        folder("spam", SPAM_MOVED, s.join("spam").join(sioul_core::spam::labels::MOVED), Shape::Lines, &[]),
        folder("spam", SPAM_FLAGGED, s.join("spam").join(sioul_core::spam::labels::FLAGGED), Shape::Lines, &[]),
        // The calls each phone screened, and those each device marked Seen:
        // each device's own file (`<device>.jsonl`), a line each, every device's read.
        folder("calls", CALLS_LOG, s.join(sioul_core::calls::FOLDER).join(sioul_core::calls::LOG), Shape::Lines, &[]),
        folder("calls", CALLS_SEEN, s.join(sioul_core::calls::FOLDER).join(sioul_core::calls::SEEN_LOG), Shape::Lines, &[]),
        // The messages each phone's notifications brought, and those each device
        // marked Seen: each device's own file, a line each, every device's read.
        folder(sioul_core::phonemsgs::PART, PHONE_MESSAGES_LOG, s.join(sioul_core::phonemsgs::FOLDER).join(sioul_core::phonemsgs::LOG), Shape::Lines, &[]),
        folder(sioul_core::phonemsgs::PART, PHONE_MESSAGES_SEEN, s.join(sioul_core::phonemsgs::FOLDER).join(sioul_core::phonemsgs::SEEN_LOG), Shape::Lines, &[]),
        // texts: each phone's texts and outcomes, each computer's requests, sealed lines, in the data folder.
        folder(sioul_core::texts::PART, TEXTS_LOG, d.join(sioul_core::texts::FOLDER).join(sioul_core::texts::LOG), Shape::Lines, &[]),
        folder(sioul_core::texts::PART, TEXTS_SEND, d.join(sioul_core::texts::FOLDER).join(sioul_core::texts::SEND), Shape::Lines, &[]),
        folder(sioul_core::texts::PART, TEXTS_OUTCOME, d.join(sioul_core::texts::FOLDER).join(sioul_core::texts::OUTCOME), Shape::Lines, &[]),
    ];
    // From the notes folder each device keeps where it likes: projects,
    // budgets, the bank's movements and contracts; the papers' wallet and its
    // files; the notes themselves, each file sealed apart.
    if let Some(notes) = config.notes_root_path() {
        stores.push(file("projects", "notes/sioul-cases.toml", sioul_core::projects::file_in(&notes), Shape::Toml(&PROJECTS_RULES)));
        stores.push(file("projects", "notes/sioul-budgets.toml", notes.join(sioul_core::budget::LEDGER), Shape::Toml(&LEDGER_RULES)));
        stores.push(file("projects", "notes/sioul-bank.toml", notes.join(sioul_core::bank::MANIFEST), Shape::Toml(&BANK_RULES)));
        stores.push(file("projects", "notes/sioul-contracts.toml", notes.join(sioul_core::contracts::MANIFEST), Shape::Toml(&CONTRACTS_RULES)));
        stores.push(file("papers", "notes/sioul-papers.toml", notes.join(sioul_core::papers::MANIFEST), Shape::Toml(&PAPERS_RULES)));
        stores.push(folder("papers", "files/papers/", notes.join(sioul_core::papers::FOLDER), Shape::Files, &[]));
        stores.push(folder("notes", "files/notes/", notes, Shape::Files, NOT_NOTES));
    }
    stores.retain(|store| shares(store.part));
    stores
}

/// The stores as a format divides their files (`Rules::at`): format 2's
/// rules from 2 on, where they differ.
pub fn shaped(stores: &[Store], format: u32) -> Vec<Store> {
    stores
        .iter()
        .map(|store| Store {
            shape: match store.shape {
                Shape::Toml(rules) => Shape::Toml(rules.at(format)),
                other => other,
            },
            ..store.clone()
        })
        .collect()
}

/// What notes or papers would send from here, before their part is switched
/// on: how many files, how many bytes, and how many stay (too big). Listed as
/// an exchange lists them, nothing read through.
pub fn estimate(stores: &[Store]) -> (usize, u64, usize) {
    let (mut found, mut files, known) = (Found::default(), Vec::new(), BTreeMap::new());
    for store in stores.iter().filter(|s| matches!(s.shape, Shape::Files) && s.path.is_dir()) {
        Listing { store: &store.name, root: &store.path, skip: store.skip, known: &known, confirmed: false, files: &mut files, found: &mut found }.list(&store.path);
    }
    files.iter().filter_map(|(_, path)| std::fs::metadata(path).ok()).fold((0, 0, 0), |(count, bytes, big), meta| if meta.len() > crate::blobs::LARGEST { (count, bytes, big + 1) } else { (count + 1, bytes + meta.len(), big) })
}

/// The part a file of the records belongs to, whether this device shares it
/// or not; none for a store this version does not know (its changes wait for
/// a version that knows it).
fn known_part(file: &str) -> Option<&'static str> {
    static KNOWN: std::sync::OnceLock<Vec<(String, bool, &'static str)>> = std::sync::OnceLock::new();
    let known = KNOWN.get_or_init(|| {
        let roots = Roots { config: PathBuf::from("config"), data: PathBuf::from("data"), state: PathBuf::from("state") };
        let config = Config { notes_root: Some("notes".into()), ..Config::default() };
        stores_of(&config, &roots, &|_| true).into_iter().map(|store| (store.name, store.folder, store.part)).collect()
    });
    #[cfg(test)]
    if tests::OLDER.with(|older| older.borrow().iter().any(|store| file.starts_with(store.as_str()))) {
        return None;
    }
    known.iter().find(|(name, folder, _)| !folder && name == file).or_else(|| known.iter().filter(|(name, folder, _)| *folder && file.starts_with(name.as_str())).max_by_key(|(name, _, _)| name.len())).map(|(_, _, part)| *part)
}

/// A file's name in the records, as shown: without the folder Sioul keeps it in
/// ("config/config.toml" → "config.toml", "files/notes/admin/lease.md" → "admin/lease.md").
pub fn shown(file: &str) -> &str {
    ["files/notes/", "files/", "config/", "data/", "state/", "notes/"].iter().find_map(|root| file.strip_prefix(root)).unwrap_or(file)
}

// ---------------------------------------------------------------- entries

/// What the files hold now, entry by entry.
#[derive(Debug, Default)]
struct Found {
    /// Each entry's value, hashed: `<store><file>#<entry>` → its hash.
    hashes: BTreeMap<String, String>,
    /// The values of the entries in files read this time; the others' files did not change.
    values: BTreeMap<String, String>,
    /// When each file last changed, in milliseconds: dates the changes found in it.
    changed: BTreeMap<String, i64>,
    /// Files that could not be read (a TOML file half written, a folder not
    /// there): their entries are neither taken out nor written to.
    unknown: BTreeSet<String>,
    /// Each file's size, time and entries: a file unchanged is not read again.
    files: BTreeMap<String, Stat>,
    /// Files sealed apart left here, too big to travel (`blobs::LARGEST`): said.
    too_big: Vec<String>,
    /// Folders of files sealed apart that list empty while they held files at
    /// the last look, with how many they held: unknown, said.
    emptied: Vec<(String, usize)>,
    /// What else the look has to say (names that are not text).
    said: Vec<String>,
}

/// A file as last read: its size, when it changed (nanoseconds), its entries'
/// hashes, and when it was read (milliseconds, this computer's clock).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Stat {
    size: u64,
    modified: u64,
    entries: Vec<(String, String)>,
    /// A file read within `settling_ms` of its change may have changed again
    /// since at the same size and time (a coarse clock: a FAT card's 2 s): read again.
    #[serde(default, skip_serializing_if = "is_zero")]
    seen: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

impl Found {
    fn is_unknown(&self, key: &str) -> bool {
        let file = file_of(key);
        self.unknown.iter().any(|u| file == u || (u.ends_with('/') && file.starts_with(u.as_str())))
    }
}

/// Folders whose files travel whole (`Shape::Whole`, `Shape::Files`): their
/// keys are the file's name and an empty entry, `#`, and a name may hold `#`
/// itself ("Facture #123.pdf").
const WHOLE_FOLDERS: &[&str] = &["data/drafts/", "data/invoices/", "data/pgp/others/", "data/calendars/local/", "data/contacts/local/", "state/shield/", "state/dav/local/", FILES];

/// `<store><file>`: the part of a key before its entry.
fn file_of(key: &str) -> &str {
    if WHOLE_FOLDERS.iter().any(|folder| key.starts_with(folder))
        && let Some(file) = key.strip_suffix('#')
    {
        return file;
    }
    key.split_once('#').map_or(key, |(file, _)| file)
}

/// The entry a key names in its file.
fn entry_of(key: &str) -> &str {
    key.get(file_of(key).len() + 1..).unwrap_or("")
}

pub(crate) fn modified_ns(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64)
}

/// How a look goes: which stores it reads, whether it must hurry, which
/// folders' emptying you confirmed, this computer's clock (milliseconds).
struct Look<'a> {
    /// Stores read this time; the others are left as last read.
    reads: &'a dyn Fn(&Store) -> bool,
    /// Files sealed apart are not read through while a hurried exchange waits.
    hurry: &'a dyn Fn() -> bool,
    confirmed: &'a BTreeSet<String>,
    clock: i64,
}

/// This computer's clock, in milliseconds.
fn clock_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
thread_local! {
    /// How long a file sealed apart settles before it is read, in this test's thread: not at all, unless a test says.
    static SETTLE: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

/// A file sealed apart changed this recently may still be being written (an
/// editor writing it in place): read at the next look.
fn settling_ms() -> i64 {
    #[cfg(test)]
    return SETTLE.with(std::cell::Cell::get);
    #[cfg(not(test))]
    2_000
}

/// What the stores hold; a file whose size and time did not change since
/// `known` is taken from there, not read. What could not be read is unknown,
/// its last look kept: nothing of it is taken out elsewhere.
fn gather(stores: &[Store], known: &BTreeMap<String, Stat>, look: &Look) -> Found {
    let mut found = Found::default();
    for store in stores {
        if !(look.reads)(store) {
            // Not read this time (notes and papers in a hurried exchange).
            found.unknown.insert(store.name.clone());
            continue;
        }
        if store.folder {
            if !store.path.is_dir() {
                // Notes or papers that held files and are gone (a disk not
                // mounted, a folder moved): unknown, said. A folder that never
                // held any is a new one, filled.
                let held = known.range(store.name.clone()..).take_while(|(file, _)| file.starts_with(&store.name)).count();
                if matches!(store.shape, Shape::Files) && held == 0 {
                    continue;
                }
                if matches!(store.shape, Shape::Files) && !look.confirmed.contains(&store.name) {
                    found.emptied.push((store.name.clone(), held));
                }
                found.unknown.insert(store.name.clone());
                continue;
            }
            let mut files = Vec::new();
            if matches!(store.shape, Shape::Files) {
                let mut listing = Listing { store: &store.name, root: &store.path, skip: store.skip, known, confirmed: look.confirmed.contains(&store.name), files: &mut files, found: &mut found };
                listing.list(&store.path);
            } else {
                list_files(&store.path, &store.path, store.skip, &mut files);
            }
            for (relative, path) in files {
                read_file(store, &format!("{}{relative}", store.name), &path, known, look, &mut found);
            }
        } else if store.path.is_file() {
            read_file(store, &store.name, &store.path, known, look, &mut found);
        } else if !matches!(store.shape, Shape::Whole | Shape::Files) {
            // A settings file not there is not a file emptied; a file whole or
            // sealed apart not there is none yet (the spam filter's table before a training).
            found.unknown.insert(store.name.clone());
        }
    }
    // What is unknown keeps its last look: next time, a folder that held files is still known to have held them.
    let carried: Vec<(String, Stat)> = known.iter().filter(|(file, _)| !found.files.contains_key(*file) && found.is_unknown(file)).map(|(file, stat)| (file.clone(), stat.clone())).collect();
    found.files.extend(carried);
    found
}

/// Every file below a folder, by its path from it with `/`; temporary and hidden files left out.
fn list_files(root: &Path, dir: &Path, skip: &[&str], out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.ends_with(".new") || name.ends_with(".tmp") || name.ends_with('~') || skip.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            list_files(root, &path, skip, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push((relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"), path));
        }
    }
}

/// A listing of a folder of files sealed apart (notes, papers).
struct Listing<'a> {
    store: &'a str,
    root: &'a Path,
    skip: &'a [&'a str],
    known: &'a BTreeMap<String, Stat>,
    /// You said to take out what went from it: a folder listing empty is emptied.
    confirmed: bool,
    files: &'a mut Vec<(String, PathBuf)>,
    found: &'a mut Found,
}

impl Listing<'_> {
    /// Every file below `dir`, by its path from the root with `/`; how many.
    /// Left out: what sync apps, editors and systems make (`made_by_tools`),
    /// `skip` at the top (whatever its case), tools' own folders, a folder
    /// shared through (it holds a seal), links, names that are not text (said).
    /// A folder that cannot be read whole (one name that errs is enough), or
    /// that lists empty while it held files at the last look (a disk not
    /// mounted, access withdrawn), is unknown: nothing in it is taken out
    /// elsewhere, nothing is written into it.
    fn list(&mut self, dir: &Path) -> usize {
        let relative = dir.strip_prefix(self.root).map(|r| r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")).unwrap_or_default();
        let within = if relative.is_empty() { String::new() } else { format!("{relative}/") };
        let prefix = format!("{}{within}", self.store);
        let mut whole = true;
        let mut count = 0;
        match std::fs::read_dir(dir) {
            Err(_) => whole = false,
            Ok(entries) => {
                for entry in entries {
                    let Ok(entry) = entry else {
                        whole = false;
                        continue;
                    };
                    let Ok(kind) = entry.file_type() else {
                        whole = false;
                        continue;
                    };
                    let name = entry.file_name();
                    let Some(name) = name.to_str() else {
                        self.found.said.push(format!("share-not-text:{prefix}{}", name.to_string_lossy()));
                        continue;
                    };
                    let path = entry.path();
                    if is_leftover(name) {
                        remove_if_old(&path);
                        continue;
                    }
                    if made_by_tools(name) || kind.is_symlink() || (dir == self.root && self.skip.iter().any(|skip| skip.eq_ignore_ascii_case(name))) {
                        continue;
                    }
                    if kind.is_dir() {
                        if !matches!(name, "node_modules" | "target") && !path.join("seal.toml").is_file() {
                            count += self.list(&path);
                        }
                    } else if kind.is_file() {
                        self.files.push((format!("{within}{name}"), path));
                        count += 1;
                    }
                }
            }
        }
        let held = self.known.range(prefix.clone()..).take_while(|(file, _)| file.starts_with(&prefix)).count();
        if !whole || (count == 0 && held > 0 && !self.confirmed) {
            if whole {
                self.found.emptied.push((prefix.clone(), held));
            }
            self.found.unknown.insert(prefix);
        }
        count
    }
}

/// What a crash left half written by Sioul: its hidden names ending so.
fn is_leftover(name: &str) -> bool {
    name.starts_with('.') && [TEMPORARY, ".shared.tmp", ".conflict.tmp"].iter().any(|end| name.ends_with(end))
}

/// A file left half written, removed once an hour old: never one still being written.
fn remove_if_old(path: &Path) {
    if std::fs::metadata(path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 3600) {
        let _ = std::fs::remove_file(path);
    }
}

/// What a crash left half written in a folder (the sealed files, the history), removed after an hour.
pub(crate) fn clean_leftovers(dir: &Path) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
        if entry.file_name().to_str().is_some_and(is_leftover) {
            remove_if_old(&entry.path());
        }
    }
}

/// Names that sync apps, editors and systems make beside your files: hidden
/// ones, temporary and backup copies, conflicted copies, locks. Sioul's own
/// copies of a file two devices changed ("lease (conflict …).md") are none of them.
fn made_by_tools(name: &str) -> bool {
    let lower = name.to_lowercase();
    name.starts_with('.')
        || name.starts_with('~')
        || name.ends_with('~')
        || (name.starts_with('#') && name.ends_with('#'))
        || [".tmp", ".temp", ".part", ".partial", ".crdownload", ".download", ".swp", ".swx", ".new"].iter().any(|end| lower.ends_with(end))
        || ["conflicted copy", ".sync-conflict-", "_conflict-", "sfconflict"].iter().any(|word| lower.contains(word))
        || matches!(lower.as_str(), "thumbs.db" | "desktop.ini" | "4913")
}

/// A file sealed apart, as its record says it: its content's hash, its size, when it changed (milliseconds).
#[derive(Debug, Serialize, Deserialize)]
struct Reference {
    h: String,
    s: u64,
    t: i64,
}

fn reference(h: &str, size: u64, modified_ns: u64) -> String {
    serde_json::to_string(&Reference { h: h.to_string(), s: size, t: (modified_ns / 1_000_000) as i64 }).unwrap_or_default()
}

/// What an entry's value is known by: a file sealed apart by its content's hash, anything else by its value's.
fn value_hash(key: &str, value: &str) -> String {
    if key.starts_with(FILES) { serde_json::from_str::<Reference>(value).map(|r| r.h).unwrap_or_default() } else { hash(value) }
}

fn read_file(store: &Store, file: &str, path: &Path, known: &BTreeMap<String, Stat>, look: &Look, found: &mut Found) {
    let Ok(meta) = std::fs::metadata(path) else {
        found.unknown.insert(file.to_string());
        return;
    };
    let sealed_apart = matches!(store.shape, Shape::Files);
    if meta.len() > if sealed_apart { crate::blobs::LARGEST } else { LARGEST } {
        found.unknown.insert(file.to_string());
        if sealed_apart {
            found.too_big.push(file.to_string());
        }
        return;
    }
    let modified = modified_ns(&meta);
    found.changed.insert(file.to_string(), (modified / 1_000_000) as i64);
    // As last read: the same size and time, read well after it changed. Not
    // a log of lines (calls, texts, labels): what it holds is not kept twice
    // in the memory (its lines are its entries' names), it is read again.
    let lines = matches!(store.shape, Shape::Lines);
    if let Some(stat) = known.get(file).filter(|s| !lines && s.size == meta.len() && s.modified == modified && (s.seen == 0 || (s.modified / 1_000_000) as i64 + settling_ms() < s.seen)) {
        for (entry, h) in &stat.entries {
            found.hashes.insert(format!("{file}#{entry}"), h.clone());
            // A file sealed apart: its record made from what was kept, the file not read again.
            if sealed_apart {
                found.values.insert(format!("{file}#{entry}"), reference(h, stat.size, stat.modified));
            }
        }
        found.files.insert(file.to_string(), stat.clone());
        return;
    }
    if sealed_apart {
        // Changed a moment ago (maybe still being written), or a hurried
        // exchange waits: read at the next look. Read through for its hash,
        // never held whole; one that changed while read is read next time.
        let fresh = look.clock - ((modified / 1_000_000) as i64) < settling_ms();
        let read = (!fresh && !(look.hurry)()).then(|| crate::blobs::hash_file(path).ok()).flatten().filter(|(_, size)| *size == meta.len());
        let Some((h, _)) = read else {
            found.unknown.insert(file.to_string());
            return;
        };
        let key = format!("{file}#");
        found.hashes.insert(key.clone(), h.clone());
        found.values.insert(key, reference(&h, meta.len(), modified));
        found.files.insert(file.to_string(), Stat { size: meta.len(), modified, entries: vec![(String::new(), h)], seen: look.clock });
        return;
    }
    let Some(entries) = std::fs::read(path).ok().and_then(|bytes| entries_of(store, &bytes)) else {
        found.unknown.insert(file.to_string());
        return;
    };
    // A log of lines is read at every look: when it was is not kept (the memory would change at each).
    let mut stat = Stat { size: meta.len(), modified, entries: Vec::with_capacity(if lines { 0 } else { entries.len() }), seen: if lines { 0 } else { look.clock } };
    for (entry, value) in entries {
        let key = format!("{file}#{entry}");
        let h = hash(&value);
        if !lines {
            stat.entries.push((entry, h.clone()));
        }
        found.hashes.insert(key.clone(), h);
        found.values.insert(key, value);
    }
    found.files.insert(file.to_string(), stat);
}

/// A file's entries and their values; none when it cannot be read as its
/// shape says. Files sealed apart are read through, never whole (`read_file`).
fn entries_of(store: &Store, bytes: &[u8]) -> Option<Vec<(String, String)>> {
    match store.shape {
        Shape::Whole => Some(vec![(String::new(), B64.encode(bytes))]),
        Shape::Lines => Some(String::from_utf8_lossy(bytes).lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| (l.to_string(), String::new())).collect()),
        Shape::Toml(rules) => toml_entries(&String::from_utf8_lossy(bytes), rules).ok(),
        Shape::Files => None,
    }
}

/// An entry's value, read from its file now.
fn value_of(stores: &[Store], key: &str) -> Option<String> {
    let (file, entry) = (file_of(key), entry_of(key));
    let (store, path) = locate(stores, file)?;
    if matches!(store.shape, Shape::Files) {
        let meta = std::fs::metadata(&path).ok()?;
        let (h, size) = crate::blobs::hash_file(&path).ok()?;
        return entry.is_empty().then(|| reference(&h, size, modified_ns(&meta)));
    }
    let bytes = std::fs::read(path).ok()?;
    entries_of(store, &bytes)?.into_iter().find(|(e, _)| e == entry).map(|(_, value)| value)
}

/// A value as TOML text, `v = …`: one form for one value, whatever the file's layout.
fn leaf_text(value: &toml::Value) -> String {
    let mut table = toml::Table::new();
    table.insert("v".into(), value.clone());
    toml::to_string(&table).unwrap_or_default()
}

fn leaf_value(text: &str) -> Result<toml::Value, String> {
    let mut table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    table.remove("v").ok_or_else(|| "no value".to_string())
}

fn matches_rule(path: &[String], rule: &str) -> bool {
    let parts: Vec<&str> = rule.split('.').collect();
    parts.len() == path.len() && parts.iter().zip(path).all(|(r, p)| *r == "*" || r == p)
}

/// An element's name in its list: its naming fields, or the element itself.
fn identity(element: &toml::Value, by: &[&str]) -> String {
    if by.is_empty() {
        return leaf_text(element);
    }
    by.iter()
        .map(|field| match element.get(field) {
            Some(toml::Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\u{1d}")
}

fn strip_local(element: &mut toml::Value, local: &[&str]) {
    if let toml::Value::Table(table) = element {
        for field in local {
            table.remove(*field);
        }
    }
}

/// The keyed list at `path` in a file: by its name at the top (or one of its
/// other names, `Keyed::also`), by its pattern within tables.
fn keyed_at<'r>(rules: &'r Rules, path: &[String]) -> Option<&'r Keyed> {
    rules.keyed.iter().find(|k| match path {
        [name] => k.list == name || k.also.contains(&name.as_str()),
        _ => k.list.contains('.') && matches_rule(path, k.list),
    })
}

/// The keyed list a record's entry names (`<list>␟␞<element>`, `<table>␟…␟<list>␟␞<element>` within tables).
fn keyed_named<'r>(rules: &'r Rules, list: &[&str]) -> Option<&'r Keyed> {
    let path: Vec<String> = list.iter().map(|part| (*part).to_string()).collect();
    rules.keyed.iter().find(|k| match list {
        [name] => k.list == *name,
        _ => k.list.contains('.') && matches_rule(&path, k.list),
    })
}

/// An element's own id, when it has one.
fn own_id(element: &toml::Value) -> Option<&str> {
    element.get("id").and_then(toml::Value::as_str).filter(|id| !id.is_empty())
}

/// An id derived from what an element without one holds (format 2): its
/// list and `what` (the element as text), the `n`th of the elements that
/// hold the same. The same on every device, whatever its order in each file;
/// shaped as a UUID (version 8: made here, by name). Only a name: never
/// written into the element, never sent out of a sealed record.
pub(crate) fn derived(list: &str, what: &str, n: usize) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"sioul-id\x1f");
    hasher.update(list.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(what.as_bytes());
    if n > 0 {
        hasher.update(format!("\x1f{n}").as_bytes());
    }
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

/// The names of a keyed list's elements (their local fields taken out), in
/// their order. Format 1: as `by` says, or the element itself. Format 2
/// (`Keyed::ids`): each element's own `id`; one without (made by an older
/// Sioul, or by hand), or whose id another element of the list holds too
/// (two devices changed it apart before format 2), by an id derived from
/// what it holds, the first, second… of those that hold the same
/// (`derived`'s `n`). A name never depends on the other elements but those
/// holding the same: two devices holding one element name it alike, so that
/// nothing they send each other is taken for something else.
fn names(elements: &[toml::Value], keyed: &Keyed) -> Vec<String> {
    if !keyed.ids {
        return elements.iter().map(|e| identity(e, keyed.by)).collect();
    }
    let mut ids: BTreeMap<&str, usize> = BTreeMap::new();
    for id in elements.iter().filter_map(own_id) {
        *ids.entry(id).or_default() += 1;
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    elements
        .iter()
        .map(|element| match own_id(element) {
            Some(id) if ids.get(id) == Some(&1) => id.to_string(),
            _ => {
                let text = leaf_text(element);
                let count = seen.entry(text.clone()).or_default();
                *count += 1;
                derived(keyed.list, &text, *count - 1)
            }
        })
        .collect()
}

/// A TOML file's entries: one per setting, one per element of a keyed list,
/// one per table kept whole; the settings that stay here left out.
fn toml_entries(text: &str, rules: &Rules) -> Result<Vec<(String, String)>, String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let mut out = Vec::new();
    for (name, value) in &table {
        flatten(vec![name.clone()], value, rules, &mut out);
    }
    Ok(out)
}

/// The record's name of a keyed list at `path`: its own at the top, its path within tables.
fn list_name(keyed: &Keyed, path: &[String]) -> String {
    if path.len() == 1 { keyed.list.to_string() } else { path.join(&SEP.to_string()) }
}

fn flatten(path: Vec<String>, value: &toml::Value, rules: &Rules, out: &mut Vec<(String, String)>) {
    if rules.local.iter().any(|rule| matches_rule(&path, rule)) {
        return;
    }
    if let toml::Value::Array(elements) = value
        && let Some(keyed) = keyed_at(rules, &path)
    {
        let list = list_name(keyed, &path);
        let elements: Vec<toml::Value> = elements
            .iter()
            .map(|element| {
                let mut element = element.clone();
                strip_local(&mut element, keyed.local);
                element
            })
            .collect();
        for (element, name) in elements.iter().zip(names(&elements, keyed)) {
            out.push((format!("{list}{SEP}{MARK}{name}"), leaf_text(element)));
        }
        return;
    }
    match value {
        toml::Value::Table(table) if !rules.whole.iter().any(|rule| matches_rule(&path, rule)) => {
            for (name, value) in table {
                let mut deeper = path.clone();
                deeper.push(name.clone());
                flatten(deeper, value, rules, out);
            }
        }
        _ => out.push((path.join(&SEP.to_string()), leaf_text(value))),
    }
}

// ---------------------------------------------------------------- writing entries

/// Writes entries into one file: each `(entry, value)`, `None` taking it out;
/// under the file's lock, as the window writes some of these files too
/// (the doses' record: `sioul_core::filelock`). Before the file changes, or
/// goes, `keep` keeps it as it was (`history`).
fn write_entries(store: &Store, path: &Path, changes: &[(&str, Option<&str>)], keep: impl FnOnce() -> Result<(), String>) -> Result<BTreeMap<String, String>, String> {
    sioul_core::filelock::with_lock(path, || {
        let new = rewritten(store, path, changes)?;
        // What the file holds once written, entry by entry, by hash: what the next look compares with.
        let written = new.as_deref().and_then(|bytes| entries_of(store, bytes)).unwrap_or_default().into_iter().map(|(entry, value)| (entry, hash(&value))).collect();
        let old = std::fs::read(path).ok();
        if old == new {
            return Ok(written);
        }
        if old.is_some() {
            keep()?;
        }
        match new {
            Some(bytes) => write_atomically(path, &bytes)?,
            None => match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("{}: {e}", path.display())),
                _ => {}
            },
        }
        Ok(written)
    })
}

/// What a file holds once entries are written into it; none when it goes.
fn rewritten(store: &Store, path: &Path, changes: &[(&str, Option<&str>)]) -> Result<Option<Vec<u8>>, String> {
    match store.shape {
        Shape::Whole => {
            let (_, value) = changes.last().ok_or("nothing")?;
            value.map(|value| B64.decode(value).map_err(|e| e.to_string())).transpose()
        }
        Shape::Files => Err(format!("{}: sealed apart", path.display())),
        Shape::Lines => {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
            for (entry, value) in changes {
                let at = lines.iter().position(|l| l.trim() == *entry);
                match (value, at) {
                    (Some(_), None) => lines.push(entry.to_string()),
                    (None, Some(at)) => {
                        lines.remove(at);
                    }
                    _ => {}
                }
            }
            let mut text = lines.join("\n");
            if !text.is_empty() {
                text.push('\n');
            }
            Ok(Some(text.into_bytes()))
        }
        Shape::Toml(rules) => {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            // A file here under another name than its record's (the projects'
            // file renamed): a list new to it takes its other name.
            let renamed = path.file_name() != Path::new(&store.name).file_name();
            Ok(Some(toml_write(&text, rules, changes, renamed)?.into_bytes()))
        }
    }
}

/// A file written whole beside its place under a hidden name, yours alone
/// (`new_private`), then renamed over it: never half a file, and its folders
/// made yours alone (`private_dirs`).
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_then_rename(path, bytes, false)
}

/// The same, the bytes on the disk before the name changes: a file whose loss
/// costs more than a minute's work (this computer's name and sharing, its memory).
pub(crate) fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_then_rename(path, bytes, true)
}

fn write_then_rename(path: &Path, bytes: &[u8], synced: bool) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        private_dirs(parent).map_err(fail)?;
    }
    // A name starting with a dot: sync apps leave it alone (eDrive skips them,
    // Nextcloud's client by default), so a half-written file never travels.
    // Yours alone (`new_private`): the file it replaces takes its mode too.
    let temporary = temporary(path);
    let written = (|| {
        let mut file = new_private(&temporary)?;
        file.write_all(bytes)?;
        if synced {
            file.sync_all()?;
        }
        std::fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written.map_err(fail)
}

/// A small file of the folder (a computer's notes, the seal, a claim), never
/// read past `SMALL_FILE`: one larger is no such file (whoever wrote it).
pub(crate) fn read_small(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path).ok()?.take(SMALL_FILE + 1).read_to_string(&mut text).ok()?;
    (text.len() as u64 <= SMALL_FILE).then_some(text)
}

/// Small files of the folder are never read past this.
pub(crate) const SMALL_FILE: u64 = 64 << 10;

/// A hidden name beside `path` for a file being written, never another's:
/// two exchanges, two Sioul running, never write into one (".lease.md.4211-7.sioul.tmp").
pub(crate) fn temporary(path: &Path) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let name = path.file_name().map_or_else(|| "file".into(), |n| n.to_string_lossy().to_string());
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    path.with_file_name(format!(".{name}.{}-{n}{TEMPORARY}", std::process::id()))
}

/// How the names of files being written end: what is left of them after a
/// crash is cleaned after an hour (`clean_leftovers`).
pub(crate) const TEMPORARY: &str = ".sioul.tmp";

// ---------------------------------------------------------------- yours alone

/// What the sharing writes is yours alone on Unix, as the calls' logs are
/// (`sioul_core::calls::append_private`): its files 0600, the folders it
/// makes 0700, whatever the umask would let others read (the records, sealed
/// files, the files it brings in from your other devices, the copies it
/// keeps). A file there already with a wider mode is made yours alone the
/// next time the sharing writes it: written beside then renamed, it takes the
/// new file's mode; appended to or copied, its mode is set. Never by a sweep
/// of your folders. Where modes mean nothing (a phone's shared storage,
/// Windows), nothing is asked and nothing said. These folders made, with
/// their parents, each 0700; folders there already keep their mode.
pub fn private_dirs(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

/// A new file to write, yours alone (0600 on Unix); an error when one is there (`create_new`).
pub(crate) fn new_private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// A file the sharing copied (a copy takes its source's mode) or appends to,
/// made yours alone (0600) when others could read it. Never through a link.
pub(crate) fn make_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::symlink_metadata(path)
            && metadata.is_file()
            && metadata.permissions().mode() & 0o077 != 0
        {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// `std::fs::copy`, the copy yours alone (`make_private`).
pub(crate) fn copy_private(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::copy(from, to)?;
    make_private(to);
    Ok(())
}

/// How far a rewritten file's size climbs before it starts again from the smallest.
const PAD_LIMIT: u64 = 4096;

/// A file written again in place must change size. The sharing asks little of
/// the sync app that carries the folder (docs/database.md, "What the sync app
/// must do"): new files and files that grow, copied some day, in any order.
/// Some sync apps tell a change by its size alone: eDrive (Murena's, on /e/OS)
/// downloads a file changed elsewhere only when its size differs from the copy
/// it holds (its `FileDiffUtils.getActionForFileDiff`), and others compare size
/// and time. So each rewrite is padded one step past the size of the file there
/// now, up to `PAD_LIMIT`, then from the smallest again: a copy held elsewhere
/// matches the newest only when a whole turn of rewrites (several hundred)
/// went by unseen, and the next rewrite mends it. `size(pad)` is the file's
/// size with `pad` characters of padding.
pub(crate) fn pad_for(path: &Path, size: impl Fn(usize) -> u64) -> usize {
    let base = size(0);
    let Ok(before) = std::fs::metadata(path).map(|m| m.len()) else { return 0 };
    if before < base + PAD_LIMIT {
        // One step past it: the padding grows a character at a time from about the right length.
        let mut pad = usize::try_from(before.saturating_sub(base) * 3 / 4).unwrap_or(0);
        while size(pad) <= before {
            pad += 1;
        }
        if size(pad) <= base + PAD_LIMIT {
            return pad;
        }
    }
    // A whole turn: the smallest again, never the size there now.
    (0..).find(|&pad| size(pad) != before).unwrap_or(0)
}

/// A TOML value as toml_edit holds it: a table stays a table, a list of tables a list of tables.
fn edit_item(text: &str) -> Result<toml_edit::Item, String> {
    let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    let mut item = doc.as_table().get("v").cloned().ok_or("no value")?;
    match &mut item {
        toml_edit::Item::Value(value) => value.decor_mut().clear(),
        toml_edit::Item::Table(table) => table.decor_mut().clear(),
        _ => {}
    }
    Ok(item)
}

/// Sets or takes out entries in a TOML file's text, keeping the rest as
/// written; `renamed`: the file has another name than its record's (`Keyed::also`).
fn toml_write(text: &str, rules: &Rules, changes: &[(&str, Option<&str>)], renamed: bool) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    // The elements of keyed lists, list by list, named as the file holds them before any of them changes.
    let mut lists: Vec<(Vec<&str>, Vec<(&str, Option<&str>)>)> = Vec::new();
    for (entry, value) in changes {
        let path: Vec<&str> = entry.split(SEP).collect();
        if let Some((element, list)) = path.split_last()
            && let Some(id) = element.strip_prefix(MARK)
            && !list.is_empty()
        {
            match lists.iter_mut().find(|(known, _)| known.as_slice() == list) {
                Some((_, elements)) => elements.push((id, *value)),
                None => lists.push((list.to_vec(), vec![(id, *value)])),
            }
        } else {
            let item = value.map(edit_item).transpose()?;
            set_path(doc.as_table_mut(), &path, item.as_ref())?;
        }
    }
    for (list, elements) in lists {
        let keyed = keyed_named(rules, &list).ok_or_else(|| format!("{}: not a list", list.join(".")))?;
        let Some((name, within)) = list.split_last() else { continue };
        let Some(table) = table_at(doc.as_table_mut(), within, elements.iter().any(|(_, value)| value.is_some()))? else { continue };
        write_elements(table, keyed, name, &elements, renamed && within.is_empty())?;
    }
    Ok(doc.to_string())
}

/// The table at `path` below `table`: made when `make` (implicit, as a
/// file's own tables are), else none when it is not there. A table written
/// inline becomes a table of its own.
fn table_at<'t>(table: &'t mut toml_edit::Table, path: &[&str], make: bool) -> Result<Option<&'t mut toml_edit::Table>, String> {
    let Some((first, rest)) = path.split_first() else { return Ok(Some(table)) };
    if !make && !table.contains_key(first) {
        return Ok(None);
    }
    let item = table.entry(first).or_insert_with(|| {
        let mut new = toml_edit::Table::new();
        new.set_implicit(true);
        toml_edit::Item::Table(new)
    });
    if let Some(inline) = item.as_inline_table().cloned() {
        *item = toml_edit::Item::Table(inline.into_table());
    }
    let inner = item.as_table_mut().ok_or_else(|| format!("{first}: not a table"))?;
    table_at(inner, rest, make)
}

/// The names of a list's elements as read (local fields out); none for one that does not read.
fn names_read(elements: &[Option<toml::Value>], keyed: &Keyed) -> Vec<Option<String>> {
    let read: Vec<toml::Value> = elements.iter().flatten().cloned().collect();
    let mut named = names(&read, keyed).into_iter();
    elements.iter().map(|element| element.as_ref().and_then(|_| named.next())).collect()
}

/// Which of the elements holding the same an element added under `name` is
/// (`derived`'s `n`), when its name was derived from what it holds: added in
/// that order, the first before the second, they take the same names here.
fn nth(keyed: &Keyed, name: &str, element: Option<toml::Value>) -> usize {
    let Some(element) = element.filter(|_| keyed.ids) else { return 0 };
    let text = leaf_text(&element);
    (0..64).find(|&n| derived(keyed.list, &text, n) == name).unwrap_or(0)
}

fn set_path(table: &mut toml_edit::Table, path: &[&str], value: Option<&toml_edit::Item>) -> Result<(), String> {
    match path {
        [] => Ok(()),
        [last] => {
            match value {
                // A value replaced keeps its comments and spacing.
                Some(toml_edit::Item::Value(new)) if table.get(last).is_some_and(toml_edit::Item::is_value) => {
                    if let Some(old) = table.get_mut(last).and_then(toml_edit::Item::as_value_mut) {
                        let decor = old.decor().clone();
                        *old = new.clone();
                        *old.decor_mut() = decor;
                    }
                }
                Some(item) => {
                    table.insert(last, item.clone());
                }
                None => {
                    table.remove(last);
                }
            }
            Ok(())
        }
        [first, rest @ ..] => {
            if value.is_none() && !table.contains_key(first) {
                return Ok(());
            }
            let item = table.entry(first).or_insert_with(|| {
                let mut new = toml_edit::Table::new();
                new.set_implicit(true);
                toml_edit::Item::Table(new)
            });
            if let Some(inline) = item.as_inline_table().cloned() {
                *item = toml_edit::Item::Table(inline.into_table());
            }
            let inner = item.as_table_mut().ok_or_else(|| format!("{first}: not a table"))?;
            set_path(inner, rest, value)
        }
    }
}

/// Where a store is read, as the memory keeps it (`Memory::places`): its
/// path, the projects' file under its first name whichever it has, so that
/// renaming it (`sioul_core::projects::rename_file`) is no move: a move
/// rebuilds the file from the records, and a change made since would go.
fn place_of(store: &Store) -> String {
    let renamed = !store.folder && store.path.file_name().is_some_and(|name| name == sioul_core::projects::FILE);
    let path = if renamed { store.path.with_file_name(sioul_core::projects::OLD_FILE) } else { store.path.clone() };
    path.display().to_string()
}

/// A table read from an entry's text, placed in a file: written where it
/// stands, its own tables under it. toml_edit writes a file's tables in the
/// order of the positions they were read at; those read from the entry's
/// text would put them out of place (a project's routes under the project
/// after it). So its tables keep none, and it takes `at`, the position of
/// the one it replaces.
fn placed(mut table: toml_edit::Table, at: Option<isize>) -> toml_edit::Table {
    fn clear(table: &mut toml_edit::Table) {
        table.set_position(None);
        for (_, item) in table.iter_mut() {
            match item {
                toml_edit::Item::Table(inner) => clear(inner),
                toml_edit::Item::ArrayOfTables(list) => list.iter_mut().for_each(clear),
                _ => {}
            }
        }
    }
    clear(&mut table);
    table.set_position(at);
    table
}

/// The elements named so of a keyed list in `table` (`changes`: each name
/// and its value, none to take it out), all named as the list holds them
/// before any of them changes: replaced (their local fields kept), added (the
/// first of those holding the same before the second), or taken out. `list`:
/// its name in the records; at the top, the list keeps the name it has in
/// the file (`Keyed::also`).
fn write_elements(table: &mut toml_edit::Table, keyed: &Keyed, list: &str, changes: &[(&str, Option<&str>)], renamed: bool) -> Result<(), String> {
    let normal = |text: String| -> Option<toml::Value> {
        let mut element = leaf_value(&text).ok()?;
        strip_local(&mut element, keyed.local);
        Some(element)
    };
    let news: Vec<(&str, Option<toml_edit::Item>, Option<toml::Value>)> = changes.iter().map(|(id, value)| Ok((*id, value.map(edit_item).transpose()?, value.and_then(|v| normal(v.to_string()))))).collect::<Result<_, String>>()?;
    let others: &[&str] = if keyed.list == list { keyed.also } else { &[] };
    let name = std::iter::once(list).chain(others.iter().copied()).find(|n| table.contains_key(n)).unwrap_or(match others.first() {
        Some(other) if renamed => other,
        _ => list,
    });
    if !table.contains_key(name) {
        match news.iter().find_map(|(_, new, _)| new.as_ref()) {
            Some(toml_edit::Item::Table(_)) => {
                table.insert(name, toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()));
            }
            Some(_) => {
                table.insert(name, toml_edit::value(toml_edit::Array::new()));
            }
            None => return Ok(()),
        }
    }
    let mut gone: Vec<usize> = Vec::new();
    match table.get_mut(name) {
        Some(toml_edit::Item::ArrayOfTables(elements)) => {
            let named = names_read(&elements.iter().map(|t| normal(format!("[v]\n{t}"))).collect::<Vec<_>>(), keyed);
            let mut added: Vec<(usize, toml_edit::Table)> = Vec::new();
            for (id, new, value) in news {
                let at = named.iter().position(|n| n.as_deref() == Some(id));
                match (new, at) {
                    (Some(toml_edit::Item::Table(mut element)), Some(at)) => {
                        if let Some(old) = elements.get(at) {
                            for field in keyed.local {
                                if let Some(kept) = old.get(field) {
                                    element.insert(field, kept.clone());
                                }
                            }
                        }
                        if let Some(place) = elements.get_mut(at) {
                            let position = place.position();
                            *place = placed(element, position);
                        }
                    }
                    (Some(toml_edit::Item::Table(element)), None) => added.push((nth(keyed, id, value), element)),
                    (None, Some(at)) => gone.push(at),
                    _ => {}
                }
            }
            added.sort_by_key(|(n, _)| *n);
            for (_, element) in added {
                elements.push(placed(element, None));
            }
            gone.sort_unstable();
            for at in gone.into_iter().rev() {
                elements.remove(at);
            }
        }
        Some(toml_edit::Item::Value(toml_edit::Value::Array(elements))) => {
            let named = names_read(&elements.iter().map(|v| normal(format!("v = {v}"))).collect::<Vec<_>>(), keyed);
            let mut added: Vec<(usize, toml_edit::Value)> = Vec::new();
            for (id, new, value) in news {
                let at = named.iter().position(|n| n.as_deref() == Some(id));
                match (new, at) {
                    (Some(toml_edit::Item::Value(element)), Some(at)) => {
                        elements.replace(at, element);
                    }
                    (Some(toml_edit::Item::Value(element)), None) => added.push((nth(keyed, id, value), element)),
                    (None, Some(at)) => gone.push(at),
                    _ => {}
                }
            }
            added.sort_by_key(|(n, _)| *n);
            for (_, element) in added {
                elements.push(element);
            }
            gone.sort_unstable();
            for at in gone.into_iter().rev() {
                elements.remove(at);
            }
            elements.fmt();
        }
        _ => return Err(format!("{name}: not a list")),
    }
    Ok(())
}

// ---------------------------------------------------------------- the seal

/// The folder's seal: how the key is made from the passphrase, and a value to
/// check it with. The same on every computer; the passphrase never written.
#[derive(Debug, Serialize, Deserialize)]
struct SealFile {
    version: u32,
    salt: String,
    memory_kib: u32,
    passes: u32,
    check: String,
}

const CHECK: &[u8] = b"sioul: the passphrase is right";

/// Why sharing cannot start.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not the passphrase chosen on the first computer.
    WrongPassphrase,
    Other(String),
}

fn derive(passphrase: &str, salt: &[u8], memory_kib: u32, passes: u32) -> Result<[u8; 32], String> {
    let params = argon2::Params::new(memory_kib, passes, 1, Some(32)).map_err(|e| e.to_string())?;
    let mut key = [0u8; 32];
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params).hash_password_into(passphrase.as_bytes(), salt, &mut key).map_err(|e| e.to_string())?;
    Ok(key)
}

/// The key for a folder: made from the passphrase and the folder's seal, which
/// the first computer writes (64 MiB, 3 passes: half a second, once per computer).
pub fn key_for(folder: &Path, passphrase: &str) -> Result<[u8; 32], Refused> {
    key_with(folder, passphrase, 64 * 1024, 3)
}

fn key_with(folder: &Path, passphrase: &str, memory_kib: u32, passes: u32) -> Result<[u8; 32], Refused> {
    let path = folder.join("seal.toml");
    if path.exists() {
        // Written by Sioul in a few lines: one larger is no seal of its.
        let text = read_small(&path).ok_or_else(|| Refused::Other(format!("{}: not a seal", path.display())))?;
        let seal: SealFile = toml::from_str(&text).map_err(|e| Refused::Other(format!("{}: {e}", path.display())))?;
        // The folder passes through others' servers: a seal asking far more than
        // Sioul writes (64 MiB, 3 passes) would only exhaust this computer.
        if seal.memory_kib > 1 << 20 || seal.passes > 16 {
            return Err(Refused::Other(format!("{}: {} KiB, {} passes", path.display(), seal.memory_kib, seal.passes)));
        }
        let salt = B64.decode(&seal.salt).map_err(|e| Refused::Other(e.to_string()))?;
        let key = derive(passphrase, &salt, seal.memory_kib, seal.passes).map_err(Refused::Other)?;
        return match open(&key, "check", &seal.check) {
            Some(plain) if plain == CHECK => Ok(key),
            _ => Err(Refused::WrongPassphrase),
        };
    }
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let key = derive(passphrase, &salt, memory_kib, passes).map_err(Refused::Other)?;
    let seal = SealFile { version: 1, salt: B64.encode(salt), memory_kib, passes, check: seal(&key, "check", CHECK) };
    let text = format!("# Sioul: how your devices' shared records are sealed (docs/database.md).\n{}", toml::to_string(&seal).map_err(|e| Refused::Other(e.to_string()))?);
    private_dirs(folder).map_err(|e| Refused::Other(format!("{}: {e}", folder.display())))?;
    write_atomically(&path, text.as_bytes()).map_err(Refused::Other)?;
    Ok(key)
}

pub(crate) fn seal(key: &[u8; 32], bound: &str, plain: &[u8]) -> String {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, Payload { msg: plain, aad: bound.as_bytes() }).unwrap_or_default();
    B64.encode([nonce.as_slice(), &sealed].concat())
}

pub(crate) fn open(key: &[u8; 32], bound: &str, text: &str) -> Option<Vec<u8>> {
    let bytes = B64.decode(text).ok()?;
    if bytes.len() < 24 {
        return None;
    }
    let (nonce, sealed) = bytes.split_at(24);
    XChaCha20Poly1305::new(key.into()).decrypt(XNonce::from_slice(nonce), Payload { msg: sealed, aad: bound.as_bytes() }).ok()
}

// ---------------------------------------------------------------- this computer

/// This computer, as sharing knows it: its name, the machine it was made on
/// (a copied configuration on another machine gets a name of its own), the
/// folder it shares through.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Here {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub machine: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// The parts this device shares or not, as chosen here (`PARTS`): never
    /// shared, each device chooses its own.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parts: BTreeMap<String, bool>,
}

impl Here {
    /// Whether this device shares a part: as chosen here, else as before parts had switches.
    pub fn shares(&self, part: &str, config: &Config) -> bool {
        self.parts.get(part).copied().unwrap_or_else(|| shared_by_default(part, config))
    }

    pub fn path(state: &Path) -> PathBuf {
        state.join("share").join("here.toml")
    }

    /// This computer's name, made once.
    pub fn load(state: &Path) -> Here {
        let path = Here::path(state);
        let mut here: Here = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        let machine = machine();
        if here.id.is_empty() || (!machine.is_empty() && here.machine != machine) {
            here.id = uuid::Uuid::new_v4().to_string();
            here.machine = machine;
            let _ = here.save(state);
        }
        here
    }

    /// Its bytes on the disk before its name changes: lost, this computer would take another name.
    pub fn save(&self, state: &Path) -> Result<(), String> {
        write_synced(&Here::path(state), toml::to_string(self).map_err(|e| e.to_string())?.as_bytes())
    }

    /// The folder shared through, `~` expanded.
    pub fn folder_path(&self) -> Option<PathBuf> {
        self.folder.as_deref().filter(|f| !f.is_empty()).map(sioul_core::config::expand_home)
    }
}

/// A hash of the machine's own id (`/etc/machine-id`), else of its name.
fn machine() -> String {
    let id = ["/etc/machine-id", "/var/lib/dbus/machine-id"].iter().find_map(|p| std::fs::read_to_string(p).ok()).or_else(|| std::env::var("COMPUTERNAME").ok()).or_else(|| std::fs::read_to_string("/etc/hostname").ok()).unwrap_or_default();
    let id = id.trim();
    if id.is_empty() { String::new() } else { format!("{:016x}", fnv(id.as_bytes())) }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

fn hash(value: &str) -> String {
    format!("{:016x}", fnv(value.as_bytes()))
}

// ---------------------------------------------------------------- memory

/// What this computer remembers between two exchanges (`memory.json`);
/// what it remembers of files sealed apart aside (`files.json`, `Sealed`): a
/// big notes folder makes that long, and what the doses and claims ask
/// (`heard`, `written`, `traffic`) reads the rest alone.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Memory {
    #[serde(default)]
    computer: String,
    /// The last clock given or read.
    #[serde(default)]
    clock: u64,
    /// This computer's round (its file) and the records in it.
    #[serde(default)]
    round: u32,
    #[serde(default)]
    seq: u64,
    /// The size of this computer's file after its last writing: shorter, a sync
    /// app put back an older copy of it, and a new one starts (`exchange`).
    #[serde(default)]
    own_size: u64,
    /// The size of this computer's file when its round began: a new round once
    /// it grew `ROUND_SIZE` past it (a round opening on more than that does not
    /// start another one each minute).
    #[serde(default)]
    round_base: u64,
    /// The last round that opens on every entry this computer holds (a full
    /// round, `new_round`); the rounds after it continue it, `SEGMENT` each at
    /// most. 0: kept by an older Sioul, whose every round was full.
    #[serde(default)]
    full_round: u32,
    /// What this computer appended since its last full round (bytes).
    #[serde(default)]
    grown: u64,
    /// When this computer's sealed files were last looked for, and the
    /// history and leftovers last tidied (milliseconds).
    #[serde(default)]
    checked: i64,
    #[serde(default)]
    tidied: i64,
    /// Others' records already read in: done once, when sharing starts.
    #[serde(default)]
    joined: bool,
    /// How far each other computer's records were read: its round and the bytes read in it.
    #[serde(default)]
    read: BTreeMap<String, (u32, u64)>,
    /// The last record read from each other computer: its round and number.
    /// Its claims say how far it wrote (`lease::Claim::wrote`): read that far,
    /// everything it said until then is known here (docs/health.md, "Knowing").
    #[serde(default)]
    read_n: BTreeMap<String, (u32, u64)>,
    /// When a line of another computer's could not be read here (Unix seconds): what it said is lost.
    #[serde(default)]
    broken: BTreeMap<String, i64>,
    /// Files whose entries are read again from every computer's records, this
    /// one's too, at the next exchange (`rebuild`): a record lost here comes back.
    #[serde(default)]
    rebuild: BTreeSet<String>,
    /// Others' changes not written yet (their file could not be read, or is
    /// one this version does not know): tried again at each exchange.
    #[serde(default)]
    pending: BTreeMap<String, (Option<String>, u64, String)>,
    /// Each entry's last change: its clock, the computer that made it, its value's hash ("" once taken out).
    #[serde(default)]
    entries: BTreeMap<String, Known>,
    /// Where each store was read (its path): a store new here, or moved (another
    /// notes folder), joins as a new computer would (`exchange`).
    #[serde(default)]
    places: BTreeMap<String, String>,
    /// Files found empty or gone after holding entries, since when (milliseconds):
    /// their entries count as taken out only once that has lasted (`EMPTIED_WAIT`).
    #[serde(default)]
    emptied: BTreeMap<String, i64>,
    /// The files as last read.
    #[serde(default)]
    files: BTreeMap<String, Stat>,
    /// When each part last sent a change, and last received one here (Unix seconds).
    #[serde(default)]
    traffic: BTreeMap<String, (i64, i64)>,
    /// The format this device writes (`FORMAT`): 0 (an older Sioul's memory)
    /// or 1 until every device in the sharing reads format 2 (`format_open`),
    /// then 2, once and for all (`switch_format`).
    #[serde(default)]
    format: u32,
    /// A record in format 2 met while this device still wrote format 1:
    /// another device writes it already; this one does from its next exchange.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    switch: bool,
    /// Lists an older Sioul sends whole, divided element by element in format
    /// 2: the clock and computer of the last whole list applied here, by its
    /// entry; an older whole list is passed over (`translate`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    floors: BTreeMap<String, (u64, String)>,
    /// Format 1's entries that named several elements here (an older Sioul
    /// noting two sessions at one start, task and project): the element each
    /// stands for in format 2, as last sent or written, so that an older
    /// Sioul's change to it changes that one (`translate`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    aliases: BTreeMap<String, String>,
    /// Others' changes in a format this device does not write yet (format 2,
    /// before its switch) or does not know (a newer Sioul's): their value,
    /// clock, computer and format, written once it can.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    waiting: BTreeMap<String, (Option<String>, u64, String, u32)>,
    /// Parts met in a format newer than this Sioul knows: that format, and the
    /// build that wrote it. Nothing of them is sent from here, nor written
    /// here, until this Sioul is updated (`share-newer`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    newer: BTreeMap<String, (u32, String)>,
    /// What it remembers of files sealed apart, in `files.json` beside.
    #[serde(skip)]
    sealed: Sealed,
}

/// What this computer remembers of files sealed apart (notes, papers), beside
/// its memory (`files.json`). Their entries, their files as last read and the
/// changes waiting for them are kept here too, out of `memory.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Sealed {
    #[serde(default)]
    computer: String,
    #[serde(default)]
    entries: BTreeMap<String, Known>,
    #[serde(default)]
    files: BTreeMap<String, Stat>,
    #[serde(default)]
    pending: BTreeMap<String, (Option<String>, u64, String)>,
    /// For each change waiting, the content it replaces where it was made: its base.
    #[serde(default)]
    bases: BTreeMap<String, String>,
    /// For each change waiting, every content the others' records named for
    /// its file so far, versions and bases (`receive`).
    #[serde(default)]
    named: BTreeMap<String, BTreeSet<String>>,
    /// The sealed files this computer put in the folder (`blobs`): when a
    /// record last pointed to each (milliseconds), and the size it sealed.
    #[serde(default)]
    mine: BTreeMap<String, (i64, u64)>,
    /// Sealed files the others' records named lately, by name: when
    /// (milliseconds). None is taken out within 90 days of that.
    #[serde(default)]
    named_at: BTreeMap<String, i64>,
    /// Sealed files found damaged, as they were then (`blobs::fingerprint`):
    /// not opened again until another copy comes.
    #[serde(default)]
    damaged: BTreeMap<String, (u64, u64, u64)>,
    /// Changes waiting for their sealed file, since when (milliseconds): said after a day.
    #[serde(default)]
    missing: BTreeMap<String, i64>,
    /// Changes that could not be written here (a full disk, a name a folder
    /// takes here): their sealed file, when to try again, how many times so far.
    #[serde(default)]
    failing: BTreeMap<String, (String, i64, u32)>,
    /// Files gone here, since when (milliseconds): taken out elsewhere once
    /// gone a while (`EMPTIED_WAIT`).
    #[serde(default)]
    gone: BTreeMap<String, i64>,
    /// Folders whose files gone at once you said to take out everywhere (`confirm_gone`).
    #[serde(default)]
    confirmed: BTreeSet<String>,
    /// When this computer's sealed files were last looked for, and the
    /// history and leftovers last tidied (milliseconds): kept in `memory.json`
    /// now (`Memory::checked`, `tidied`), so that this file, large with many
    /// notes or texts, is written only when what it holds changes. Read once,
    /// from an older Sioul's.
    #[serde(default, skip_serializing)]
    checked: i64,
    #[serde(default, skip_serializing)]
    tidied: i64,
    /// The spam filter's tables this computer sealed in the folder, by name,
    /// oldest first: past the newest `KEPT_TABLES`, the older go from the
    /// folder at once rather than 90 days on (`prune_tables`). This device's
    /// own memory, never shared; empty in an older Sioul's, whose tables go
    /// as before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tables: Vec<String>,
}

/// The spam filter's tables a computer keeps in the folder: the one in use
/// and the one before it, for a device that has not read the newest record
/// yet. Each training that does no worse seals a new one, about two
/// megabytes, once a week at most (docs/spam-filter.md); kept 90 days each,
/// they would be a dozen, and a phone whose sync app never deletes a file
/// (eDrive) would keep every one it brought.
const KEPT_TABLES: usize = 2;

/// This computer's own spam filter's tables past the newest `KEPT_TABLES`
/// taken out of the folder now, unless a record of this computer's names
/// one again (`used`: a table put back): that one goes by the usual rule.
/// Never another device's file: only those this computer sealed (`mine`).
/// A device that reads only the newest record, as every Sioul does, never
/// asks for them; an earlier version of the table kept by reference on
/// another device can no longer be put back from the folder.
fn prune_tables(folder: &Path, sealed: &mut Sealed, used: &BTreeSet<String>) {
    sealed.tables.retain(|name| sealed.mine.contains_key(name));
    while sealed.tables.len() > KEPT_TABLES {
        let old = sealed.tables.remove(0);
        if used.contains(&old) {
            continue;
        }
        let _ = std::fs::remove_file(folder.join("blobs").join(&old));
        sealed.mine.remove(&old);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Known {
    c: u64,
    w: String,
    h: String,
    /// For a file sealed apart changed here: the content it replaced, said again in a new round.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    b: String,
}

impl Memory {
    /// `memory.json` alone: what the doses and claims ask.
    fn load(path: &Path, computer: &str) -> Memory {
        let memory: Memory = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if memory.computer == computer { memory } else { Memory { computer: computer.to_string(), round: 1, ..Memory::default() } }
    }

    /// The whole memory, files sealed apart too (`files.json`), their entries
    /// and changes among the others while an exchange runs.
    fn load_all(path: &Path, computer: &str) -> Memory {
        let mut memory = Memory::load(path, computer);
        match std::fs::read_to_string(sealed_path(path)).ok().and_then(|t| serde_json::from_str::<Sealed>(&t).ok()).filter(|s| s.computer == computer) {
            Some(sealed) => {
                // An older Sioul's times, kept in this file then.
                memory.checked = memory.checked.max(sealed.checked);
                memory.tidied = memory.tidied.max(sealed.tidied);
                memory.sealed = sealed;
            }
            // Lost, or not readable: notes and papers, and the calls, join
            // again as a new device's would (their files read again from the
            // records first), rather than each going out as new.
            None => memory.places.retain(|store, _| !store.starts_with(FILES) && !calls_store(store)),
        }
        memory.entries.append(&mut memory.sealed.entries);
        memory.files.append(&mut memory.sealed.files);
        memory.pending.append(&mut memory.sealed.pending);
        memory
    }

    /// Both files written, each when it changed only (a phone's storage
    /// wears), its bytes on the disk before its name changes. The calls'
    /// entries go beside the notes': a log of lines names each entry by its
    /// line, and `memory.json`, written at every exchange and read by the
    /// doses' knowing, stays small.
    fn save(&mut self, path: &Path) -> Result<(), String> {
        let apart = |key: &String| key.starts_with(FILES) || calls_store(key);
        let (sealed, kept): (BTreeMap<_, _>, BTreeMap<_, _>) = std::mem::take(&mut self.entries).into_iter().partition(|(key, _)| apart(key));
        (self.sealed.entries, self.entries) = (sealed, kept);
        let (sealed, kept): (BTreeMap<_, _>, BTreeMap<_, _>) = std::mem::take(&mut self.files).into_iter().partition(|(file, _)| apart(file));
        (self.sealed.files, self.files) = (sealed, kept);
        let (sealed, kept): (BTreeMap<_, _>, BTreeMap<_, _>) = std::mem::take(&mut self.pending).into_iter().partition(|(key, _)| apart(key));
        (self.sealed.pending, self.pending) = (sealed, kept);
        self.sealed.computer = self.computer.clone();
        let write = |path: &Path, text: String| {
            if std::fs::read(path).is_ok_and(|old| old == text.as_bytes()) {
                return Ok(());
            }
            write_synced(path, text.as_bytes())
        };
        write(&sealed_path(path), serde_json::to_string(&self.sealed).map_err(|e| e.to_string())?)?;
        write(path, serde_json::to_string(self).map_err(|e| e.to_string())?)
    }

    /// A new clock: after every clock given or read, and not before `physical` (milliseconds).
    fn tick(&mut self, physical: i64) -> u64 {
        self.clock = (self.clock + 1).max((physical.max(0) as u64) << 16);
        self.clock
    }
}

/// Where what this computer remembers of files sealed apart is kept: `files.json`, beside its memory.
fn sealed_path(memory: &Path) -> PathBuf {
    memory.with_file_name("files.json")
}

/// One line of a computer's file: its number in the round, its clock, the sealed change.
#[derive(Debug, Serialize, Deserialize)]
struct Line {
    n: u64,
    c: u64,
    s: String,
}

/// A change, sealed in a line: the entry and its new value, none once taken out.
#[derive(Debug, Serialize, Deserialize)]
struct Change {
    k: String,
    v: Option<String>,
    /// For a file sealed apart: the content it replaces where it was made, by
    /// its hash ("" for a file new there). Another device holding something
    /// else changed it too: both are kept.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    b: String,
    /// The build that wrote it (`sioul_core::build::DESCRIBED`), on the first
    /// line of each batch this computer appends, a new round's first line
    /// among them: the lines after it, until the next that says one, are that
    /// build's. Sealed with the change: the server never learns it. An older
    /// Sioul passes over it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    build: String,
    /// The format its part is written in (`part_format`), when not 1: a
    /// device that does not know it keeps the change waiting and says so
    /// (`share-newer`); one that does not write it yet writes it from its
    /// next exchange (`Memory::switch`). Sealed with the change. An older
    /// Sioul passes over it, and its changes say none: format 1.
    #[serde(default, skip_serializing_if = "first_format")]
    f: u32,
}

fn first_format(f: &u32) -> bool {
    *f <= 1
}

/// A computer's own notes in the folder: when it last exchanged, its round, how far it read the others.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Seen {
    #[serde(default)]
    at: i64,
    #[serde(default)]
    round: u32,
    #[serde(default)]
    read: BTreeMap<String, u32>,
    /// Its size changed at each writing (`pad_for`): read by nobody.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pad: String,
}

fn bound(computer: &str, round: u32, n: u64, clock: u64) -> String {
    format!("{computer}:{round}:{n}:{clock}")
}

fn round_file(folder: &Path, computer: &str, round: u32) -> PathBuf {
    folder.join(format!("{computer}-{round}.jsonl"))
}

/// Every computer sharing through the folder: those that wrote records, and
/// those that only read so far (their notes); in the copy fetched from the
/// server too (`remote::overlay`).
fn computers(folder: &Path) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = rounds_with(folder).into_keys().collect();
    for dir in std::iter::once(folder.to_path_buf()).chain(crate::remote::overlay(folder)) {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(id) = name.strip_suffix(".toml").filter(|id| id.len() == 36 && uuid::Uuid::parse_str(id).is_ok()) {
                out.insert(id.to_string());
            }
        }
    }
    out
}

/// The computers' files in the folder and in the copy fetched from the
/// server (`remote::overlay`): computer → its rounds, those of either.
fn rounds_with(folder: &Path) -> BTreeMap<String, Vec<u32>> {
    let mut all = rounds(folder);
    if let Some(fetched) = crate::remote::overlay(folder) {
        for (computer, theirs) in rounds(&fetched) {
            let list = all.entry(computer).or_default();
            list.extend(theirs);
            list.sort_unstable();
            list.dedup();
        }
    }
    all
}

/// Another computer's round, from the longer of its two copies: the folder's,
/// and the one fetched from the server (`remote::overlay`). Records only
/// grow: the longer holds all the shorter does, and where each line starts
/// is the same in both.
fn open_round(folder: &Path, computer: &str, round: u32) -> std::io::Result<std::fs::File> {
    let here = round_file(folder, computer, round);
    let Some(fetched) = crate::remote::overlay(folder).map(|f| round_file(&f, computer, round)) else { return std::fs::File::open(here) };
    let size = |path: &Path| std::fs::metadata(path).map(|m| m.len()).ok();
    match (size(&here), size(&fetched)) {
        (Some(a), Some(b)) if b > a => std::fs::File::open(fetched),
        (None, Some(_)) => std::fs::File::open(fetched),
        _ => std::fs::File::open(here),
    }
}

/// The computers' files in the folder: computer → its rounds.
fn rounds(folder: &Path) -> BTreeMap<String, Vec<u32>> {
    let mut out: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for entry in std::fs::read_dir(folder).into_iter().flatten().filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".jsonl") else { continue };
        // "<uuid>-<round>": a UUID is 36 characters; a conflicted copy's name is longer.
        let Some((computer, round)) = stem.rsplit_once('-') else { continue };
        if computer.len() != 36 || uuid::Uuid::parse_str(computer).is_err() {
            continue;
        }
        if let Ok(round) = round.parse::<u32>() {
            out.entry(computer.to_string()).or_default().push(round);
        }
    }
    for list in out.values_mut() {
        list.sort_unstable();
    }
    out
}

// ---------------------------------------------------------------- formats

/// A change kept waiting (`Memory::waiting`): the later of two for one entry.
fn wait(waiting: &mut BTreeMap<String, (Option<String>, u64, String, u32)>, key: String, change: (Option<String>, u64, String, u32)) {
    if waiting.get(&key).is_none_or(|(_, c, w, _)| (change.1, change.2.as_str()) > (*c, w.as_str())) {
        waiting.insert(key, change);
    }
}

/// Why a device holds back format 2 (`format_holders`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holds {
    /// Its Sioul does not read format 2, or does not say: an older one.
    Older,
    /// Its entry does not read here yet (its seal not arrived, damaged).
    Unread,
    /// It reads format 2, but what it wrote before is not all read here yet.
    Unheard,
}

/// The devices that hold back format 2 (`FORMAT`), and why: each device of
/// the sharing but this one, heard from in the last `SILENT_DAYS` (its notes'
/// time, its entry's) and not gone (`left`), must say in its entry that it
/// reads format 2 (`devices::Entry::format`), and all it wrote up to that
/// entry (`wrote`) must be read here (`read_n`). A device known by its
/// records alone, or whose entry does not read here, holds it back: an older
/// Sioul cannot read format 2 and would double what it changes.
fn format_holders(folder: &Path, key: &[u8; 32], computer: &str, read_n: &BTreeMap<String, (u32, u64)>, now_ms: i64) -> Vec<(String, Holds)> {
    let (entries, unread) = crate::devices::all(folder, key);
    let mut ids = computers(folder);
    ids.extend(entries.iter().map(|e| e.id.clone()));
    ids.extend(unread.iter().cloned());
    let heard = Heard { read: read_n.clone(), broken: BTreeMap::new() };
    let mut out = Vec::new();
    for id in ids.iter().filter(|id| id.as_str() != computer) {
        let entry = entries.iter().find(|e| &e.id == id);
        if entry.is_some_and(|e| e.left) {
            continue;
        }
        let last = read_seen(folder, id).map_or(0, |s| s.at).max(entry.map_or(0, |e| e.started.max(e.imported).max(e.closed)));
        if last > 0 && now_ms / 1000 - last > SILENT_DAYS * 86_400 {
            continue;
        }
        match entry {
            None if unread.contains(id) => out.push((id.clone(), Holds::Unread)),
            Some(entry) if entry.format >= 2 => {
                if entry.wrote.is_some_and(|wrote| !heard.complete(id, wrote)) {
                    out.push((id.clone(), Holds::Unheard));
                }
            }
            _ => out.push((id.clone(), Holds::Older)),
        }
    }
    out
}

/// Whether this device may write format 2: another device at least shares
/// the folder (alone, it writes format 1: a device joining later may run an
/// older Sioul), and none holds it back (`format_holders`). A device not
/// joined yet holds nothing format 1 could lose: every other device saying
/// it reads format 2 is enough, and it joins in format 2.
fn format_open(sharing: &Sharing, memory: &Memory, now_ms: i64) -> bool {
    let others = computers(sharing.folder).into_iter().any(|id| id != sharing.computer) || crate::devices::all(sharing.folder, sharing.key).0.iter().any(|e| e.id != sharing.computer && !e.left);
    others && format_holders(sharing.folder, sharing.key, sharing.computer, &memory.read_n, now_ms).iter().all(|(_, holds)| !memory.joined && *holds == Holds::Unheard)
}

/// What this device knows of the sharing's formats, for the window
/// (Settings ▸ Your folder and sharing, the status line): the format it
/// writes, the parts met in a newer format than it knows (their format,
/// the build that wrote it), and, while it writes format 1, the devices that
/// hold format 2 back and why.
#[derive(Debug, Default, Clone)]
pub struct Formats {
    pub writes: u32,
    pub newer: Vec<(String, u32, String)>,
    pub holders: Vec<(String, Holds)>,
}

pub fn formats(folder: &Path, key: &[u8; 32], memory: &Path, computer: &str, now_ms: i64) -> Formats {
    let state = Memory::load(memory, computer);
    let holders = if state.format < 2 && state.joined { format_holders(folder, key, computer, &state.read_n, now_ms) } else { Vec::new() };
    Formats { writes: state.format.max(1), newer: state.newer.into_iter().map(|(part, (f, build))| (part, f, build)).collect(), holders }
}

/// The format this device writes (`FORMAT`): 1 until every device reads format 2 (`format_open`), then 2.
pub fn writes(memory: &Path) -> u32 {
    format_of(memory).max(1)
}

/// The format this device writes, as its memory says (`Memory::format`), whichever computer's it is.
fn format_of(memory: &Path) -> u32 {
    std::fs::read_to_string(memory).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| v.get("format").and_then(serde_json::Value::as_u64)).map_or(0, |f| f as u32)
}

/// How format 2 names an entry format 1 named so (its path in its file).
enum Renamed<'r> {
    /// As format 1 does.
    Same,
    /// An element of a list whose elements format 2 names otherwise (`Keyed::ids`).
    Element(&'r Keyed),
    /// A list format 1 shares whole, divided element by element in format 2.
    Whole(&'r Keyed),
}

fn renamed<'r>(rules: &Rules, next: &'r Rules, entry: &str) -> Renamed<'r> {
    let path: Vec<&str> = entry.split(SEP).collect();
    match path.split_last() {
        Some((element, list)) if element.starts_with(MARK) && !list.is_empty() => match (keyed_named(rules, list), keyed_named(next, list)) {
            (Some(before), Some(after)) if before.ids == after.ids && before.by == after.by => Renamed::Same,
            (_, Some(after)) => Renamed::Element(after),
            _ => Renamed::Same,
        },
        _ => match (keyed_named(rules, &path), keyed_named(next, &path)) {
            (None, Some(after)) => Renamed::Whole(after),
            _ => Renamed::Same,
        },
    }
}

/// The elements of a file's keyed lists (format 2's), each with its entry
/// in format 1 (its own, or its whole list's when format 1 shares the list
/// whole), its entry in format 2, and its value.
fn aligned(text: &str, rules: &Rules, next: &Rules) -> Vec<(String, String, String)> {
    let Ok(table) = text.parse::<toml::Table>() else { return Vec::new() };
    let mut out = Vec::new();
    for (name, value) in &table {
        align(vec![name.clone()], value, rules, next, &mut out);
    }
    out
}

fn align(path: Vec<String>, value: &toml::Value, rules: &Rules, next: &Rules, out: &mut Vec<(String, String, String)>) {
    if next.local.iter().any(|rule| matches_rule(&path, rule)) {
        return;
    }
    if let toml::Value::Array(elements) = value
        && let Some(keyed) = keyed_at(next, &path)
    {
        let strip = |local: &[&str]| -> Vec<toml::Value> {
            elements
                .iter()
                .map(|element| {
                    let mut element = element.clone();
                    strip_local(&mut element, local);
                    element
                })
                .collect()
        };
        let after = strip(keyed.local);
        let list = list_name(keyed, &path);
        let before: Vec<String> = match keyed_at(rules, &path) {
            Some(before) => {
                let list = list_name(before, &path);
                names(&strip(before.local), before).into_iter().map(|name| format!("{list}{SEP}{MARK}{name}")).collect()
            }
            None => vec![path.join(&SEP.to_string()); elements.len()],
        };
        for ((element, name), entry) in after.iter().zip(names(&after, keyed)).zip(before) {
            out.push((entry, format!("{list}{SEP}{MARK}{name}"), leaf_text(element)));
        }
        return;
    }
    if let toml::Value::Table(table) = value
        && !next.whole.iter().any(|rule| matches_rule(&path, rule))
    {
        for (name, value) in table {
            let mut deeper = path.clone();
            deeper.push(name.clone());
            align(deeper, value, rules, next, out);
        }
    }
}

/// The name format 2 gives an element format 1 named `ident` when the file
/// here does not hold it: format 1 named it by itself (`by` empty), and its
/// own id, else what it holds, names it in format 2 as the first of those
/// holding the same (`names`); format 1 named it by its id: that id. By
/// other fields (a session by its start, task and project), it cannot be
/// told: a name nothing holds.
fn stateless(keyed: &Keyed, ident: &str) -> String {
    if keyed.by.is_empty()
        && let Some(id) = leaf_value(ident).ok().as_ref().and_then(own_id).map(str::to_string)
    {
        return id;
    }
    if keyed.by == ["id"] && !ident.is_empty() {
        return ident.to_string();
    }
    derived(keyed.list, ident, 0)
}

/// An element's own id, when there is an element and it has one.
fn own_id_of(element: Option<&toml::Value>) -> Option<String> {
    element.and_then(own_id).map(str::to_string)
}

/// The element's name (after `␞`) in an entry.
fn element_name(entry: &str) -> &str {
    entry.rsplit_once(MARK).map_or("", |(_, name)| name)
}

/// Format 1's changes (`older`: by entry, the later of each), for the files
/// format 2 divides otherwise, in format 2's names, merged as format 1 would
/// have merged them. An element's change takes the name format 2 gives that
/// element: its own id (an older Sioul rewriting a month of sessions without
/// their ids: the id it had here is kept, and put back in it), else the
/// name format 2 gives the element format 1 named so in the file here, else
/// one derived from it (`stateless`). Several of format 1's entries for one
/// element (its versions: an older Sioul's change is a new version and the
/// old one taken out) make one change: the latest version held, at the
/// latest clock among them; none held, taken out. A list format 1 sent
/// whole is applied element by element, at its clock: what it holds is set,
/// what it no longer holds is taken out, the later word winning element by
/// element; an older whole list than one applied here before is passed over
/// (`Memory::floors`).
fn translate(stores: &[Store], older: &BTreeMap<String, (Option<String>, u64, String)>, memory: &mut Memory, winners: &BTreeMap<String, (Option<String>, u64, String)>) -> Vec<(String, Option<String>, u64, String)> {
    let mut versions: BTreeMap<String, Vec<(Option<String>, u64, String)>> = BTreeMap::new();
    let mut out = Vec::new();
    let mut here: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    for (key, (value, c, w)) in older {
        let (file, entry) = (file_of(key), entry_of(key));
        let Some((store, path)) = locate(stores, file) else { continue };
        let Shape::Toml(rules) = store.shape else { continue };
        let next = rules.at(2);
        match renamed(rules, next, entry) {
            Renamed::Same => out.push((key.clone(), value.clone(), *c, w.clone())),
            Renamed::Element(keyed) => {
                let (list, ident) = entry.rsplit_once(SEP).map_or(("", ""), |(list, element)| (list, element.trim_start_matches(MARK)));
                let elements = here.entry(file.to_string()).or_insert_with(|| aligned(&std::fs::read_to_string(&path).unwrap_or_default(), rules, next));
                // The element format 1 names so here: the one it stood for when
                // several held that name (`Memory::aliases`), else the first, as
                // format 1 writes it.
                let alias = memory.aliases.get(key).cloned();
                let mine = alias.as_ref().and_then(|alias| elements.iter().find(|(before, after, _)| before == entry && format!("{file}#{after}") == *alias)).or_else(|| elements.iter().find(|(before, _, _)| before == entry));
                let kept = mine.and_then(|(_, _, text)| leaf_value(text).ok().as_ref().and_then(own_id).map(str::to_string));
                let mut value = value.clone();
                let given = value.as_deref().and_then(|v| leaf_value(v).ok()).map(|mut element| {
                    strip_local(&mut element, keyed.local);
                    element
                });
                let name = match (&given, own_id_of(given.as_ref()), kept) {
                    (Some(_), Some(id), _) => id,
                    // An older Sioul writing it without the id it has here (it
                    // rewrites a month of sessions without them): the same
                    // element, its id kept and put back in it.
                    (Some(element), None, Some(id)) => {
                        let mut element = element.clone();
                        if let toml::Value::Table(table) = &mut element {
                            table.insert("id".into(), toml::Value::String(id.clone()));
                        }
                        value = Some(leaf_text(&element));
                        id
                    }
                    (Some(element), None, None) => derived(keyed.list, &leaf_text(element), 0),
                    (None, _, _) => mine.map_or_else(|| stateless(keyed, ident), |(_, after, _)| element_name(after).to_string()),
                };
                // Format 1 writes over the element it names so (a session by
                // its start, task and project): the one here goes when format
                // 2 names the new one otherwise.
                if given.is_some()
                    && !keyed.by.is_empty()
                    && let Some((_, after, _)) = mine
                    && element_name(after) != name
                {
                    versions.entry(format!("{file}#{after}")).or_default().push((None, *c, w.clone()));
                }
                if alias.is_some() {
                    memory.aliases.insert(key.clone(), format!("{file}#{list}{SEP}{MARK}{name}"));
                }
                versions.entry(format!("{file}#{list}{SEP}{MARK}{name}")).or_default().push((value, *c, w.clone()));
            }
            Renamed::Whole(keyed) => {
                if memory.floors.get(key).is_some_and(|(fc, fw)| (*c, w.as_str()) <= (*fc, fw.as_str())) {
                    continue;
                }
                memory.floors.insert(key.clone(), (*c, w.clone()));
                let path: Vec<String> = entry.split(SEP).map(str::to_string).collect();
                let prefix = format!("{file}#{}{SEP}{MARK}", list_name(keyed, &path));
                let elements: Vec<toml::Value> = value
                    .as_deref()
                    .and_then(|v| leaf_value(v).ok())
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|mut element| {
                        strip_local(&mut element, keyed.local);
                        element
                    })
                    .collect();
                let listed: BTreeSet<String> = names(&elements, keyed).into_iter().map(|name| format!("{prefix}{name}")).collect();
                for (element, name) in elements.iter().zip(names(&elements, keyed)) {
                    versions.entry(format!("{prefix}{name}")).or_default().push((Some(leaf_text(element)), *c, w.clone()));
                }
                let known: BTreeSet<String> = memory.entries.iter().filter(|(k, known)| !known.h.is_empty() && k.starts_with(&prefix)).map(|(k, _)| k.clone()).chain(winners.iter().filter(|(k, (v, _, _))| v.is_some() && k.starts_with(&prefix)).map(|(k, _)| k.clone())).collect();
                for gone in known.difference(&listed) {
                    versions.entry(gone.clone()).or_default().push((None, *c, w.clone()));
                }
            }
        }
    }
    for (key, list) in versions {
        let Some(last) = list.iter().max_by(|a, b| (a.1, &a.2).cmp(&(b.1, &b.2))) else { continue };
        let held = list.iter().filter(|v| v.0.is_some()).max_by(|a, b| (a.1, &a.2).cmp(&(b.1, &b.2))).and_then(|v| v.0.clone());
        out.push((key, held, last.1, last.2.clone()));
    }
    out
}

/// This device writes format 2 from now on (`FORMAT`), once and for all: its
/// memory is said in format 2's names. Each entry of a file format 2 divides
/// otherwise takes the name format 2 gives the element it named, as the
/// file holds it now (`aligned`; the element it held last, when the file
/// holds several of that name), else one derived from it (`stateless`);
/// several entries for one element (its versions) make one, at the latest
/// clock among them, holding the latest version held. A list sent whole
/// takes one entry per element when the file still holds what was last sent
/// (else they go out as changes made here), and its clock becomes that
/// list's floor. These files' last looks are forgotten: read again, in
/// format 2. Changes of theirs waiting to be written wait with the others'
/// older records, translated at the next reading (`translate`).
fn switch_format(memory: &mut Memory, stores: &[Store]) {
    for store in stores {
        let Shape::Toml(rules) = store.shape else { continue };
        let Some(next) = rules.next else { continue };
        let belongs = |file: &str| locate(stores, file).is_some_and(|(s, _)| s.name == store.name);
        let files: BTreeSet<String> = memory.entries.keys().map(|key| file_of(key).to_string()).filter(|file| belongs(file)).collect();
        for file in files {
            let text = locate(stores, &file).and_then(|(_, path)| std::fs::read_to_string(path).ok()).unwrap_or_default();
            let elements = aligned(&text, rules, next);
            let before: BTreeMap<String, String> = toml_entries(&text, rules).unwrap_or_default().into_iter().collect();
            let keys: Vec<String> = memory.entries.keys().filter(|key| file_of(key) == file).cloned().collect();
            let mut grouped: BTreeMap<String, Vec<Known>> = BTreeMap::new();
            for key in keys {
                let entry = entry_of(&key).to_string();
                let renaming = renamed(rules, next, &entry);
                if matches!(renaming, Renamed::Same) {
                    continue;
                }
                let Some(known) = memory.entries.remove(&key) else { continue };
                match renaming {
                    Renamed::Same => {}
                    Renamed::Whole(_) => {
                        memory.floors.insert(key.clone(), (known.c, known.w.clone()));
                        if !known.h.is_empty() && before.get(&entry).is_some_and(|value| hash(value) == known.h) {
                            for (_, after, text) in elements.iter().filter(|(before, _, _)| *before == entry) {
                                grouped.entry(format!("{file}#{after}")).or_default().push(Known { c: known.c, w: known.w.clone(), h: hash(text), b: String::new() });
                            }
                        }
                    }
                    Renamed::Element(keyed) => {
                        let mine: Vec<&(String, String, String)> = elements.iter().filter(|(before, _, _)| *before == entry).collect();
                        let after = match mine.iter().find(|(_, _, text)| hash(text) == known.h).or(mine.first()) {
                            Some((_, after, _)) => after.clone(),
                            None => {
                                let (list, ident) = entry.rsplit_once(SEP).map_or(("", ""), |(list, element)| (list, element.trim_start_matches(MARK)));
                                format!("{list}{SEP}{MARK}{}", stateless(keyed, ident))
                            }
                        };
                        // Several elements of that name here: the one it stood for, kept.
                        if mine.len() > 1 {
                            memory.aliases.insert(key.clone(), format!("{file}#{after}"));
                        }
                        grouped.entry(format!("{file}#{after}")).or_default().push(known);
                    }
                }
            }
            for (key, versions) in grouped {
                let Some(last) = versions.iter().max_by(|a, b| (a.c, &a.w).cmp(&(b.c, &b.w))).cloned() else { continue };
                let held = versions.iter().filter(|v| !v.h.is_empty()).max_by(|a, b| (a.c, &a.w).cmp(&(b.c, &b.w))).map(|v| v.h.clone()).unwrap_or_default();
                memory.entries.insert(key, Known { c: last.c, w: last.w, h: held, b: String::new() });
            }
        }
        memory.files.retain(|file, _| !belongs(file));
        let pending: Vec<String> = memory.pending.keys().filter(|key| belongs(file_of(key))).cloned().collect();
        for key in pending {
            if let Some((value, c, w)) = memory.pending.remove(&key) {
                wait(&mut memory.waiting, key, (value, c, w, 1));
            }
        }
    }
    memory.format = 2;
    memory.switch = false;
}

// ---------------------------------------------------------------- exchange

/// Sharing as set up on this computer.
pub struct Sharing<'a> {
    pub folder: &'a Path,
    pub computer: &'a str,
    pub key: &'a [u8; 32],
    /// Where this computer keeps what it remembers: `<state>/share/memory.json`.
    pub memory: &'a Path,
    /// Notes and papers too (`Shape::Files`): not for an exchange that must be
    /// quick (a dose's alarm, a notification's button, Sioul closing), nor
    /// without the system's leave to read them all. A single file sealed apart
    /// (the spam filter's table) goes in every exchange.
    pub files: bool,
    /// Set while a hurried exchange waits for this one: its work on notes and
    /// papers stops where it is, and goes on at the next.
    pub hurry: Option<&'a std::sync::atomic::AtomicBool>,
}

/// What an exchange did.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The stores written to, by name: what to read again.
    pub written: BTreeSet<String>,
    /// Changes found here and sent.
    pub sent: usize,
    /// Changes from the others written here.
    pub received: usize,
    /// What went wrong, in a word each; the rest went on.
    pub problems: Vec<String>,
    /// The others' changes waiting to be written here (their file unreadable, or not shared here).
    pub pending: usize,
    /// When this computer's files were read for it (milliseconds, its clock):
    /// everything captured here before is in its records, up to `wrote`. 0
    /// when it did not run (`share-busy`).
    pub looked: i64,
    /// How far this computer's records went once it ended: round and number;
    /// none before sharing joined.
    pub wrote: Option<(u32, u64)>,
}

/// Before the first exchange, a copy of every shared file, in case. Not of
/// notes and papers, which may be large: each file is kept before anything
/// is written over it (`history`). Not of the calls' logs: nothing removes
/// these copies, and a call leaves every device after its month.
fn keep_copies(stores: &[Store], memory: &Path, today: &str) {
    let Some(base) = memory.parent() else { return };
    let aside = base.join(format!("before-sharing-{today}"));
    for store in stores.iter().filter(|s| !matches!(s.shape, Shape::Files) && !calls_store(&s.name)) {
        let target = aside.join(store.name.trim_end_matches('/'));
        if store.folder {
            let mut files = Vec::new();
            list_files(&store.path, &store.path, store.skip, &mut files);
            for (relative, path) in files {
                let to = target.join(relative);
                let _ = to.parent().map(private_dirs);
                let _ = copy_private(&path, &to);
            }
        } else if store.path.is_file() {
            let _ = target.parent().map(private_dirs);
            let _ = copy_private(&store.path, &target);
        }
    }
}

/// One exchange: the changes found here go out, the others' come in. One at a
/// time on this computer, whatever runs it (`exchange_lock`): another running,
/// a quick one waits for it; a whole one does nothing, and says so (`share-busy`).
pub fn exchange(sharing: &Sharing, stores: &[Store], now_ms: i64) -> Result<Outcome, String> {
    private_dirs(sharing.folder).map_err(|e| format!("{}: {e}", sharing.folder.display()))?;
    let Some(_running) = exchange_lock(sharing.memory, !sharing.files)? else { return Ok(Outcome { problems: vec!["share-busy".into()], ..Outcome::default() }) };
    // What was fetched from the server too, read beside the folder, the newer
    // copy of each other device's file (`remote`); none until a server's
    // folder is confirmed to be this one.
    crate::remote::attach(sharing.folder, sharing.memory);
    let mut memory = Memory::load_all(sharing.memory, sharing.computer);
    let mut outcome = Outcome::default();
    // Notes and papers only when asked (not for a dose's alarm, a button
    // pressed, Sioul closing), and while no hurried exchange waits for this
    // one: their work stops where it is, and goes on at the next.
    let hurried = || sharing.hurry.is_some_and(|hurry| hurry.load(std::sync::atomic::Ordering::Relaxed));
    // A single file sealed apart (the spam filter's table), in every exchange:
    // one file of Sioul's own, quick to read, never in a shared storage.
    let reads = |store: &Store| sharing.files || !matches!(store.shape, Shape::Files) || !store.folder;
    // This computer's own records in the folder go further than its memory: the
    // memory was restored from a backup, lost, or started again ("Stop sharing",
    // then again). Its numbering goes on after the folder's last line (the
    // others never read a number twice, nor miss one), and every file is read
    // again from all the records, its own too: what it marked after its memory's
    // time comes back here.
    if let Some((round, n, size)) = last_own(sharing.folder, sharing.computer)
        && (round, n) > (memory.round, memory.seq)
    {
        outcome.problems.push("share-own-ahead".into());
        memory.round = round;
        memory.seq = n;
        memory.own_size = size;
        memory.round_base = 0;
        // Which of its rounds was full is not known: none taken out until
        // the next full round (`remove_old_rounds` keeps every round from 1).
        memory.full_round = 1;
        memory.read.clear();
        memory.rebuild.insert("*".into());
    }
    if !memory.joined {
        let today = jiff::Timestamp::from_millisecond(now_ms).map(|t| t.strftime("%Y-%m-%d").to_string()).unwrap_or_default();
        keep_copies(stores, sharing.memory, &today);
    }
    // The format this device writes (`FORMAT`): format 2 once every device
    // in the sharing reads it (`format_open`), or once another device writes
    // it already (`Memory::switch`); its memory is then said in format 2's
    // names, once (`switch_format`). Its files are read and written as its
    // format divides them; an older format's records are translated
    // (`translate`).
    if memory.format < 2 && (memory.switch || format_open(sharing, &memory, now_ms)) {
        switch_format(&mut memory, stores);
    }
    let originals = stores;
    let shaped_stores = shaped(stores, memory.format);
    let stores = shaped_stores.as_slice();
    // Parts met in a newer format: nothing of them goes out from here until this Sioul is updated.
    let held: BTreeSet<&'static str> = memory.newer.keys().filter_map(|part| PARTS.iter().find(|p| **p == part.as_str()).copied()).collect();
    let holds = |key: &str| known_part(file_of(key)).is_some_and(|part| held.contains(part));
    let clock = clock_ms();
    let confirmed = memory.sealed.confirmed.clone();
    let mut found = gather(stores, &memory.files, &Look { reads: &reads, hurry: &hurried, confirmed: &confirmed, clock });
    outcome.problems.extend(found.too_big.iter().map(|file| format!("share-too-big:{file}")));
    outcome.problems.append(&mut found.said);
    outcome.problems.extend(found.emptied.iter().map(|(folder, held)| format!("share-vanished:{folder}:{held}")));
    let mut out: Vec<(String, Option<String>, u64, String)> = Vec::new();

    // Stores new here (projects shared from now on, a store a new version
    // adds) or moved (another notes folder): joined as a new computer joins.
    // Their files are rebuilt from all the records first, then only what they
    // alone hold goes out: an old copy here never beats the others' newer
    // values, an empty new folder is filled, and nothing is taken out
    // elsewhere. A memory from before places were kept takes every store where
    // it is. A store not read this time joins when it is.
    let place = place_of;
    if memory.joined && memory.places.is_empty() {
        memory.places = stores.iter().map(|s| (s.name.clone(), place(s))).collect();
    }
    let joining: BTreeSet<String> = if memory.joined { stores.iter().filter(|s| reads(s) && memory.places.get(&s.name) != Some(&place(s))).map(|s| s.name.clone()).collect() } else { BTreeSet::new() };
    let joins = |key: &str| locate(stores, file_of(key)).is_some_and(|(store, _)| joining.contains(&store.name));
    if !joining.is_empty() {
        memory.entries.retain(|key, _| !joins(key));
        memory.read.clear();
        memory.rebuild.extend(joining.iter().cloned());
    }

    // A file sealed apart found empty that was not at the last look (an
    // editor writing it in place, a crash): held as it was, a while.
    let mut held_empty: BTreeSet<String> = BTreeSet::new();
    let emptied_now: Vec<String> = found.files.iter().filter(|(file, stat)| file.starts_with(FILES) && stat.size == 0 && memory.files.get(*file).is_some_and(|before| before.size > 0)).map(|(file, _)| file.clone()).collect();
    for file in emptied_now {
        let since = *memory.emptied.entry(file.clone()).or_insert(now_ms);
        if now_ms - since < EMPTIED_WAIT {
            found.hashes.remove(&format!("{file}#"));
            found.unknown.insert(file.clone());
            if let Some(before) = memory.files.get(&file) {
                found.files.insert(file.clone(), before.clone());
            }
            held_empty.insert(file);
        }
    }
    // Not read this time (a quick exchange): held as they were.
    let unread = |file: &str| locate(stores, file).is_some_and(|(store, _)| !reads(store));
    memory.emptied.retain(|file, _| !file.starts_with(FILES) || held_empty.contains(file) || unread(file));

    // Files sealed apart gone here: taken out elsewhere once gone a while
    // (`EMPTIED_WAIT`: a file being replaced, a disk away a moment), and never
    // many at once (a folder moved away, access withdrawn, a disk not
    // mounted) until you say so (`confirm_gone`). Meanwhile another device's
    // change to one of them still comes, unless many went at once.
    let mut held_gone: BTreeSet<String> = BTreeSet::new();
    let mut gone_since: BTreeMap<String, i64> = memory.sealed.gone.iter().filter(|(file, _)| unread(file)).map(|(file, since)| (file.clone(), *since)).collect();
    if memory.joined {
        let mut by_store: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
        for key in memory.entries.iter().filter(|(key, known)| key.starts_with(FILES) && !known.h.is_empty()).map(|(key, _)| key) {
            let Some((store, _)) = locate(stores, file_of(key)) else { continue };
            if !reads(store) || joining.contains(&store.name) {
                continue;
            }
            let counted = by_store.entry(store.name.clone()).or_default();
            counted.0 += 1;
            if !found.hashes.contains_key(key) && !found.is_unknown(key) {
                counted.1.push(file_of(key).to_string());
            }
        }
        for (store, (held, gone)) in by_store.into_iter().filter(|(_, (_, gone))| !gone.is_empty()) {
            let confirmed = memory.sealed.confirmed.contains(&store);
            let mass = gone.len() > MASS || (gone.len() >= 3 && gone.len() * 5 > held);
            if mass && !confirmed {
                outcome.problems.push(format!("share-vanished:{store}:{}", gone.len()));
            }
            for file in gone {
                let since = memory.sealed.gone.get(&file).copied().unwrap_or(now_ms);
                if mass && !confirmed {
                    gone_since.insert(file.clone(), since);
                    found.unknown.insert(file);
                } else if !confirmed && now_ms - since < EMPTIED_WAIT {
                    gone_since.insert(file.clone(), since);
                    held_gone.insert(file);
                }
            }
        }
    }
    memory.sealed.gone = gone_since;
    let value = |key: &str| found.values.get(key).cloned().or_else(|| value_of(stores, key));

    // Changes made here since the last look, dated by their file. When joining,
    // the others' records come first, and only what they do not hold goes out.
    let mut older: Vec<String> = Vec::new();
    if memory.joined {
        for (key, h) in &found.hashes {
            if memory.entries.get(key).is_some_and(|k| k.h == *h) || joins(key) || holds(key) {
                continue;
            }
            if key.starts_with(FILES) {
                // An older copy put back by hand (a backup's, its time before
                // the version last seen here): not sent; that version comes
                // back, the copy kept beside (`older`).
                let file = file_of(key);
                if memory.entries.get(key).is_some_and(|k| !k.h.is_empty()) && found.files.get(file).zip(memory.files.get(file)).is_some_and(|(now, before)| now.modified < before.modified) {
                    older.push(key.clone());
                    continue;
                }
                if hurried() {
                    continue;
                }
            }
            let Some(value) = value(key) else { continue };
            // A file sealed apart goes into the folder before its record.
            if !seal_apart(sharing, stores, key, &value, &mut memory.sealed, &mut outcome.problems, now_ms) {
                continue;
            }
            let base = base_of(&memory, key);
            let clock = memory.tick(found.changed.get(file_of(key)).copied().unwrap_or(now_ms).min(now_ms));
            memory.entries.insert(key.clone(), Known { c: clock, w: sharing.computer.to_string(), h: h.clone(), b: base.clone() });
            out.push((key.clone(), Some(value), clock, base));
        }
        // A settings file (the settings, the doses' record…) that held entries
        // and is now gone or of no bytes at all: a crash, a full disk, a file
        // being written in place, rather than a choice. Held for a while
        // (`EMPTIED_WAIT`): never everything taken out, everywhere, at once. A
        // list emptied on purpose, or a file gone from a folder of files (a
        // draft sent), is taken out at once; a note or a paper after a while
        // (above). A store no longer shared here (a part switched off, a notes
        // folder unset) takes nothing out elsewhere.
        let settings = |file: &str| stores.iter().find(|s| !s.folder && s.name == file && matches!(s.shape, Shape::Toml(_)));
        let held: BTreeSet<String> = memory.entries.iter().filter(|(_, k)| !k.h.is_empty()).map(|(key, _)| file_of(key).to_string()).filter(|file| settings(file).is_some()).collect();
        let mut waiting: BTreeSet<String> = BTreeSet::new();
        for file in held {
            let nothing = settings(&file).is_some_and(|s| std::fs::metadata(&s.path).map_or(true, |m| m.len() == 0));
            if !nothing || found.hashes.keys().any(|k| file_of(k) == file) {
                memory.emptied.remove(&file);
            } else if now_ms - *memory.emptied.entry(file.clone()).or_insert(now_ms) < EMPTIED_WAIT {
                outcome.problems.push(format!("share-emptied:{file}"));
                waiting.insert(file);
            }
        }
        let gone: Vec<String> = memory
            .entries
            .iter()
            .filter(|(key, known)| !known.h.is_empty() && !found.hashes.contains_key(*key) && !found.is_unknown(key) && !held_gone.contains(file_of(key)) && !holds(key))
            .filter(|(key, _)| locate(stores, file_of(key)).is_some_and(|(store, _)| reads(store)) && !waiting.contains(file_of(key)))
            .map(|(key, _)| key.clone())
            .collect();
        for key in gone {
            let base = base_of(&memory, &key);
            let clock = memory.tick(now_ms);
            memory.entries.insert(key.clone(), Known { c: clock, w: sharing.computer.to_string(), h: String::new(), b: base.clone() });
            out.push((key, None, clock, base));
        }
    }

    // The others' records, from where each was left; those not written last
    // time first. A change for a part this device does not share is not kept
    // waiting: switched on, the part joins and reads it again.
    let newer = |c: u64, w: &str, than: Option<(u64, &str)>| than.is_none_or(|(tc, tw)| (c, w) > (tc, tw));
    // Nor one for a store that left (`RETIRED`), which an older device may still send.
    let off = |key: &str| retired(key) || (locate(stores, file_of(key)).is_none() && known_part(file_of(key)).is_some());
    let mut winners: BTreeMap<String, (Option<String>, u64, String)> = std::mem::take(&mut memory.pending)
        .into_iter()
        .filter(|(key, (_, c, w))| !off(key) && newer(*c, w, memory.entries.get(key).map(|k| (k.c, k.w.as_str()))))
        .collect();
    // Records of an older format (format 1: an older Sioul's, or this device's
    // own before it wrote format 2), for files format 2 divides otherwise: by
    // their entries, the later of each, translated once all are read (`translate`).
    let mut older_format: BTreeMap<String, (Option<String>, u64, String)> = BTreeMap::new();
    let divided_otherwise = |key: &str| locate(originals, file_of(key)).is_some_and(|(store, _)| matches!(store.shape, Shape::Toml(rules) if rules.next.is_some()));
    let later = |older_format: &mut BTreeMap<String, (Option<String>, u64, String)>, key: String, value: Option<String>, c: u64, w: &str| {
        if older_format.get(&key).is_none_or(|(_, tc, tw)| (c, w) > (*tc, tw.as_str())) {
            older_format.insert(key, (value, c, w.to_string()));
        }
    };
    // Changes that waited for this device to write format 2, or for this
    // Sioul to know their part's format: in, once it can.
    let ready: Vec<String> = memory.waiting.iter().filter(|(key, (_, _, _, f))| memory.format >= 2 && known_part(file_of(key)).is_some_and(|part| *f <= part_format(part))).map(|(key, _)| key.clone()).collect();
    for key in ready {
        let Some((value, c, w, f)) = memory.waiting.remove(&key) else { continue };
        if off(&key) {
            continue;
        }
        if f < 2 && divided_otherwise(&key) {
            later(&mut older_format, key, value, c, &w);
        } else if newer(c, &w, memory.entries.get(&key).map(|k| (k.c, k.w.as_str()))) && newer(c, &w, winners.get(&key).map(|(_, c, w)| (*c, w.as_str()))) {
            winners.insert(key, (value, c, w));
        }
    }
    // A part no longer newer than this Sioul (it was updated): said no more.
    memory.newer.retain(|part, (f, _)| *f > part_format(part));
    // The build each computer's lines were written by, as its batches say: for `share-newer`.
    let mut builds: BTreeMap<String, String> = BTreeMap::new();
    // For files sealed apart: the content each change replaces where it was
    // made (its base), and every content the others' records name for each
    // file, their versions and what those replaced: a file here among them is
    // an older version, not a change of its own (`receive`).
    let mut bases: BTreeMap<String, String> = std::mem::take(&mut memory.sealed.bases).into_iter().filter(|(key, _)| winners.contains_key(key)).collect();
    let mut named: BTreeMap<String, BTreeSet<String>> = std::mem::take(&mut memory.sealed.named).into_iter().filter(|(key, _)| winners.contains_key(key)).collect();
    for (key, (value, _, _)) in winners.iter().filter(|(key, _)| key.starts_with(FILES)) {
        named.entry(key.clone()).or_default().extend(value.as_deref().map(|v| value_hash(key, v)).into_iter().chain(bases.get(key).cloned()).filter(|h| !h.is_empty()));
    }
    let all = rounds_with(sharing.folder);
    // Rebuilding a file: this computer's own records are read again too, for its entries only.
    let rebuilding = std::mem::take(&mut memory.rebuild);
    let rebuilt = |file: &str| rebuilding.contains("*") || rebuilding.contains(file) || rebuilding.iter().any(|r| r.ends_with('/') && file.starts_with(r.as_str()));
    for (computer, their_rounds) in all.iter().filter(|(c, _)| *c != sharing.computer || !rebuilding.is_empty()) {
        let own = computer == sharing.computer;
        let (mut start, mut skip) = if own { (0, 0) } else { memory.read.get(computer).copied().unwrap_or((0, 0)) };
        if !their_rounds.contains(&start) {
            // Not begun, or that round was removed: the oldest kept starts with all that counts.
            (start, skip) = (their_rounds[0], 0);
        }
        'rounds: for &round in their_rounds.iter().filter(|r| **r >= start) {
            let mut offset = if round == start { skip } else { 0 };
            let opened = if own { std::fs::File::open(round_file(sharing.folder, computer, round)) } else { open_round(sharing.folder, computer, round) };
            let Ok(mut file) = opened else {
                if !own {
                    memory.read.insert(computer.clone(), (round, offset));
                }
                continue;
            };
            // Shorter than what was read of it: an older copy put back by a sync
            // app. What it held past that point is lost here, said; it is read
            // again from its start (lines already known change nothing).
            if !own && file.metadata().is_ok_and(|m| m.len() < offset) {
                outcome.problems.push(format!("share-other-cut:{computer}"));
                memory.broken.insert(computer.clone(), now_ms / 1000);
                offset = 0;
            }
            {
                use std::io::Seek;
                let _ = file.seek(std::io::SeekFrom::Start(offset));
            }
            // Only whole lines: one still arriving is read next time. Each line on
            // its own: one that is not text (a disk's damage), or longer than any
            // record, is one broken line, not the end of everything after it.
            let opens = |line: &[u8]| {
                let record = std::str::from_utf8(line).ok().and_then(|text| serde_json::from_str::<Line>(text).ok())?;
                let change = open(sharing.key, &bound(computer, round, record.n, record.c), &record.s).and_then(|p| serde_json::from_slice::<Change>(&p).ok())?;
                Some((record, change))
            };
            let mut lines = LineReader::new(file);
            let mut next = lines.next();
            while let Some((line, length)) = next.take() {
                next = lines.next();
                let Some(record) = std::str::from_utf8(&line).ok().and_then(|text| serde_json::from_str::<Line>(text).ok()) else {
                    // Never passed over in silence: it may have been a dose marked taken.
                    offset += length;
                    if !own {
                        outcome.problems.push(format!("share-other-line:{computer}"));
                        memory.broken.insert(computer.clone(), now_ms / 1000);
                    }
                    continue;
                };
                let Some((_, change)) = opens(&line) else {
                    outcome.problems.push(format!("share-other-seal:{computer}"));
                    // One line that does not open while the next does: a damaged line,
                    // lost and said. None opening: another key, or a seal not here
                    // yet (the sync app is slow): read again from there next time.
                    if !own && next.as_ref().is_some_and(|(next, _)| opens(next).is_some()) {
                        offset += length;
                        memory.broken.insert(computer.clone(), now_ms / 1000);
                        continue;
                    }
                    if !own {
                        memory.read.insert(computer.clone(), (round, offset));
                    }
                    break 'rounds;
                };
                offset += length;
                if !change.build.is_empty() {
                    builds.insert(computer.clone(), change.build.clone());
                }
                // Sealed files named lately are never taken out from under the record naming them (`blobs::sweep`).
                if change.k.starts_with(FILES)
                    && let Some(value) = change.v.as_deref()
                {
                    memory.sealed.named_at.insert(crate::blobs::name(sharing.key, &value_hash(&change.k, value)), now_ms);
                }
                if own {
                    // Only the entries of the files rebuilt; the rest is this computer's as it is.
                    if rebuilt(file_of(&change.k)) && !off(&change.k) && memory.format >= 2 && change.f < 2 && divided_otherwise(&change.k) {
                        later(&mut older_format, change.k, change.v, record.c, computer);
                    } else if rebuilt(file_of(&change.k)) && !off(&change.k) && newer(record.c, computer, winners.get(&change.k).map(|(_, c, w)| (*c, w.as_str()))) && memory.entries.get(&change.k).is_none_or(|k| (record.c, computer.as_str()) > (k.c, k.w.as_str())) {
                        bases.insert(change.k.clone(), change.b);
                        winners.insert(change.k, (change.v, record.c, computer.clone()));
                    }
                    continue;
                }
                // The lines follow each other: after a gap (lines lost to a stale copy of
                // the file, or to a damaged one) what that computer marked is not known
                // here, said as a doubt, until it starts a new round with all it holds.
                let last = memory.read_n.get(computer).copied();
                if last.is_none_or(|last| (round, record.n) > last) {
                    let follows = match last {
                        Some((r, n)) if r == round => record.n == n + 1,
                        _ => record.n == 1,
                    };
                    if follows {
                        memory.read_n.insert(computer.clone(), (round, record.n));
                    } else {
                        if !outcome.problems.iter().any(|p| *p == format!("share-other-gap:{computer}")) {
                            outcome.problems.push(format!("share-other-gap:{computer}"));
                        }
                        memory.broken.insert(computer.clone(), now_ms / 1000);
                    }
                }
                memory.clock = memory.clock.max(record.c);
                if off(&change.k) {
                    continue;
                }
                // Its part in a format this Sioul does not know (a newer one's):
                // kept waiting, never written here, said once; nothing of that
                // part goes out from here until this Sioul is updated.
                let format = change.f.max(1);
                if let Some(part) = known_part(file_of(&change.k))
                    && format > part_format(part)
                {
                    if !memory.newer.contains_key(part) {
                        outcome.problems.push(format!("share-newer:{part}"));
                    }
                    memory.newer.insert(part.to_string(), (format, builds.get(computer).cloned().unwrap_or_default()));
                    wait(&mut memory.waiting, change.k, (change.v, record.c, computer.clone(), format));
                    continue;
                }
                // Format 2 while this device writes format 1: another device
                // writes it already; this one does from its next exchange, and
                // the change waits for it.
                if format >= 2 && memory.format < 2 {
                    memory.switch = true;
                    wait(&mut memory.waiting, change.k, (change.v, record.c, computer.clone(), format));
                    continue;
                }
                // Format 1 for a file format 2 divides otherwise: translated once all are read.
                if format < 2 && memory.format >= 2 && divided_otherwise(&change.k) {
                    later(&mut older_format, change.k, change.v, record.c, computer);
                    continue;
                }
                if change.k.starts_with(FILES) {
                    named.entry(change.k.clone()).or_default().extend(change.v.as_deref().map(|v| value_hash(&change.k, v)).into_iter().chain([change.b.clone()]).filter(|h| !h.is_empty()));
                }
                let known = memory.entries.get(&change.k).map(|k| (k.c, k.w.as_str()));
                let pending = winners.get(&change.k).map(|(_, c, w)| (*c, w.as_str()));
                if newer(record.c, computer, known) && newer(record.c, computer, pending) {
                    bases.insert(change.k.clone(), change.b);
                    winners.insert(change.k, (change.v, record.c, computer.clone()));
                }
            }
            if !own {
                memory.read.insert(computer.clone(), (round, offset));
            }
        }
    }

    // An older format's changes, in format 2's names (`translate`), then as any other.
    if !older_format.is_empty() {
        for (key, value, c, w) in translate(originals, &older_format, &mut memory, &winners) {
            if newer(c, &w, memory.entries.get(&key).map(|k| (k.c, k.w.as_str()))) && newer(c, &w, winners.get(&key).map(|(_, c, w)| (*c, w.as_str()))) {
                bases.remove(&key);
                winners.insert(key, (value, c, w));
            }
        }
    }

    // The others' changes, written into the files: notes and papers taken out
    // first, so that a file renamed by its case comes in the same exchange.
    let mut by_file: BTreeMap<String, Vec<(String, Option<String>)>> = BTreeMap::new();
    for (key, (value, _, _)) in &winners {
        by_file.entry(file_of(key).to_string()).or_default().push((key.clone(), value.clone()));
    }
    let mut order: Vec<&String> = by_file.keys().collect();
    order.sort_by_key(|file| !(file.starts_with(FILES) && by_file[*file].first().is_some_and(|(_, value)| value.is_none())));
    let mut written_keys: BTreeSet<String> = BTreeSet::new();
    // What was written, by key: what the next look compares with, whatever happens to the file after.
    let mut written_hashes: BTreeMap<String, String> = BTreeMap::new();
    // Files written: as they are now (a note, a paper: read next time only if it changes), or to read again.
    let mut written_files: BTreeMap<String, Option<Stat>> = BTreeMap::new();
    // Files sealed apart already as the others' change says: known, nothing written.
    let mut settled: BTreeSet<String> = BTreeSet::new();
    let history = crate::history::root(sharing.memory);
    let mut kept = false;
    let mut names: Option<Names> = None;
    let receiving = Receiving { sharing, history: &history, now_ms, clock };
    for file in order {
        let changes = &by_file[file];
        let Some((store, path)) = locate(stores, file) else { continue };
        if matches!(store.shape, Shape::Files) {
            // Not read this time, too big here, in a folder that cannot be read
            // or emptied of a sudden, or a hurried exchange waits: left waiting.
            let Some((key, value)) = changes.first() else { continue };
            if !reads(store) || hurried() || found.is_unknown(file) {
                continue;
            }
            let blob = value.as_deref().and_then(|v| serde_json::from_str::<Reference>(v).ok()).map(|r| crate::blobs::name(sharing.key, &r.h)).unwrap_or_default();
            // Could not be written lately (a full disk, a name a folder takes here): tried again after a while, not at every exchange.
            if memory.sealed.failing.get(key).is_some_and(|(failed, again, _)| *failed == blob && now_ms < *again) {
                continue;
            }
            if !writable(store, &path, sharing) {
                outcome.problems.push(format!("share-refused:{file}"));
                continue;
            }
            let names = names.get_or_insert_with(|| Names::new(&memory.entries, &found));
            if value.is_some() && names.clash(file) {
                outcome.problems.push(format!("share-name-clash:{file}"));
                continue;
            }
            let base = bases.get(key).map_or("", String::as_str);
            let seen = |h: &str| h == base || named.get(key).is_some_and(|n| n.contains(h));
            let back_off = |failing: &mut BTreeMap<String, (String, i64, u32)>| {
                let tried = failing.get(key).map_or(0, |(_, _, n)| *n);
                let wait = [MINUTE, 10 * MINUTE, HOUR, 6 * HOUR][(tried as usize).min(3)];
                failing.insert(key.clone(), (blob.clone(), now_ms + wait, tried + 1));
            };
            match receive(&receiving, store, file, &path, value.as_deref(), found.hashes.get(key).map(String::as_str), &seen, memory.entries.get(key), found.files.get(file), &mut memory.sealed.damaged) {
                Ok(Received::AlreadySo) => {
                    settled.insert(key.clone());
                    memory.sealed.missing.remove(key);
                    memory.sealed.failing.remove(key);
                }
                Ok(Received::Later) => {}
                Ok(Received::Written { stat, copy, kept: into_history }) => {
                    kept |= into_history;
                    outcome.written.insert(store.name.clone());
                    outcome.received += 1;
                    written_keys.insert(key.clone());
                    written_hashes.insert(key.clone(), stat.as_ref().and_then(|s| s.entries.first()).map(|(_, h)| h.clone()).unwrap_or_default());
                    names.written(file, stat.is_some());
                    written_files.insert(file.clone(), stat);
                    memory.sealed.missing.remove(key);
                    memory.sealed.failing.remove(key);
                    if let Some(copy) = copy {
                        outcome.problems.push(format!("{}:{copy}", if value.is_some() { "share-conflict" } else { "share-conflict-gone" }));
                    }
                }
                // Not here yet: it comes with the sync app, and is written then; said after a day.
                Err(crate::blobs::Fault::Missing) => {
                    if now_ms - *memory.sealed.missing.entry(key.clone()).or_insert(now_ms) > DAY {
                        outcome.problems.push(format!("share-missing:{file}"));
                    }
                }
                Err(crate::blobs::Fault::Broken) => outcome.problems.push(format!("share-damaged:{file}")),
                Err(crate::blobs::Fault::Room) => {
                    outcome.problems.push(format!("share-no-room:{file}"));
                    back_off(&mut memory.sealed.failing);
                }
                Err(crate::blobs::Fault::Local(e)) => {
                    outcome.problems.push(e);
                    back_off(&mut memory.sealed.failing);
                }
            }
            continue;
        }
        if found.unknown.contains(file) && !matches!(store.shape, Shape::Whole) && path.exists() {
            // Half written, or written by hand and broken: left as it is.
            outcome.problems.push(format!("share-unreadable:{}", path.display()));
            continue;
        }
        if store.folder && !writable(store, &path, sharing) {
            outcome.problems.push(format!("share-refused:{file}"));
            continue;
        }
        let entries: Vec<(&str, Option<&str>)> = changes.iter().map(|(key, value)| (entry_of(key), value.as_deref())).collect();
        // The file as it was, kept to be put back; never a log of calls, which
        // has nothing to put back and leaves every device after its month.
        let keep = || {
            if calls_store(file) {
                return Ok(());
            }
            kept = true;
            crate::history::keep(&history, store.part, file, &path, now_ms)
        };
        match write_entries(store, &path, &entries, keep) {
            Ok(hashes) => {
                outcome.written.insert(store.name.clone());
                outcome.received += changes.len();
                for (key, _) in changes {
                    written_keys.insert(key.clone());
                    written_hashes.insert(key.clone(), hashes.get(entry_of(key)).cloned().unwrap_or_default());
                }
                written_files.insert(file.clone(), None);
            }
            Err(e) => outcome.problems.push(e),
        }
    }
    for (key, (value, clock, computer)) in winners {
        if written_keys.contains(&key) || settled.contains(&key) {
            let h = written_hashes.get(&key).cloned().unwrap_or_else(|| value.as_deref().map(|v| value_hash(&key, v)).unwrap_or_default());
            memory.entries.insert(key, Known { c: clock, w: computer, h, b: String::new() });
        } else {
            if let Some(base) = bases.remove(&key).filter(|b| !b.is_empty()) {
                memory.sealed.bases.insert(key.clone(), base);
            }
            if let Some(contents) = named.remove(&key) {
                memory.sealed.named.insert(key.clone(), contents);
            }
            memory.pending.insert(key, (value, clock, computer));
        }
    }
    // What the next look compares with: the files as found, those written
    // here as they are now (notes and papers), or to read again (the rest:
    // small). What was written is known by what was written (`written_hashes`):
    // a change made here after it is a change made here, sent.
    let mut files = found.files.clone();
    for (file, stat) in written_files {
        match stat {
            Some(stat) => files.insert(file, stat),
            None => files.remove(&file),
        };
    }

    // Joining, the whole sharing or a store: what only this computer holds goes out.
    if !memory.joined || !joining.is_empty() {
        for (key, h) in &found.hashes {
            if memory.entries.contains_key(key) || memory.pending.contains_key(key) || (memory.joined && !joins(key)) || (key.starts_with(FILES) && hurried()) || holds(key) {
                continue;
            }
            let Some(value) = value(key) else { continue };
            if !seal_apart(sharing, stores, key, &value, &mut memory.sealed, &mut outcome.problems, now_ms) {
                continue;
            }
            let clock = memory.tick(found.changed.get(file_of(key)).copied().unwrap_or(now_ms).min(now_ms));
            memory.entries.insert(key.clone(), Known { c: clock, w: sharing.computer.to_string(), h: h.clone(), b: String::new() });
            out.push((key.clone(), Some(value), clock, String::new()));
        }
        memory.joined = true;
    }

    // Older copies put back by hand: the version last seen here comes back,
    // the copy kept beside, said; one that cannot come back now (not here
    // yet) is held, its copy not sent.
    for key in older.into_iter().filter(|key| !written_keys.contains(key)) {
        let file = file_of(&key).to_string();
        let (Some((store, path)), Some(known), Some(before)) = (locate(stores, &file), memory.entries.get(&key), memory.files.get(&file).cloned()) else { continue };
        let back = serde_json::to_string(&Reference { h: known.h.clone(), s: before.size, t: (before.modified / 1_000_000) as i64 }).unwrap_or_default();
        let came = if hurried() || !writable(store, &path, sharing) { Ok(Received::Later) } else { receive(&receiving, store, &file, &path, Some(&back), found.hashes.get(&key).map(String::as_str), &|_| false, Some(known), found.files.get(&file), &mut memory.sealed.damaged) };
        match came {
            Ok(Received::Written { stat, copy, kept: into_history }) => {
                kept |= into_history;
                outcome.problems.push(format!("share-older-copy:{}", copy.unwrap_or_else(|| shown(&file).to_string())));
                if let Some(stat) = stat {
                    files.insert(file, stat);
                }
            }
            _ => {
                outcome.problems.push(format!("share-older-copy:{}", shown(&file)));
                files.insert(file, before);
            }
        }
    }

    outcome.sent = out.len();
    outcome.pending = memory.pending.len();
    if memory.round == 0 {
        memory.round = 1;
    }
    // This computer's file cut short, or gone: a sync app put back an older copy
    // (eDrive keeps a file's old content when only its version changed, and may
    // send it back). What it said past that point is lost for whoever had not
    // read it: a new file opens on every entry this computer has the last word on.
    let own = |memory: &Memory| std::fs::metadata(round_file(sharing.folder, sharing.computer, memory.round)).map_or(0, |m| m.len());
    if memory.own_size > own(&memory) {
        outcome.problems.push("share-own-cut".into());
        new_round(sharing, &mut memory, stores, &found, &hurried, now_ms)?;
        memory.round_base = own(&memory);
    }
    // Past `SEGMENT`, what comes goes into a new round continuing this one,
    // nothing restated: the file sent at each change stays small.
    if !out.is_empty() && own(&memory) >= SEGMENT {
        if memory.full_round == 0 {
            memory.full_round = memory.round;
        }
        memory.round += 1;
        memory.seq = 0;
        memory.round_base = 0;
    }
    let before = own(&memory);
    append(sharing, &mut memory, &out)?;
    memory.grown += own(&memory).saturating_sub(before);
    // The first round holds all this computer had when it joined: a full one.
    if memory.full_round == 0 && memory.round == 1 {
        memory.full_round = 1;
        memory.grown = 0;
    }
    // When each part last sent and received a change, for the window.
    let part_of = |key: &str| locate(stores, file_of(key)).map(|(store, _)| store.part);
    for part in out.iter().filter_map(|(key, ..)| part_of(key)) {
        memory.traffic.entry(part.to_string()).or_default().0 = now_ms / 1000;
    }
    for part in written_keys.iter().filter_map(|key| part_of(key)) {
        memory.traffic.entry(part.to_string()).or_default().1 = now_ms / 1000;
    }
    // Now and then, the sealed files this computer's records point to looked for (`look_after_sealed`).
    if sharing.files && !hurried() && now_ms - memory.checked >= 10 * MINUTE {
        look_after_sealed(sharing, stores, &mut memory, &found, &hurried, &mut outcome.problems, now_ms);
        memory.checked = now_ms;
    }
    // Damaged sealed files no change waits for any more are forgotten.
    if !memory.sealed.damaged.is_empty() {
        let waited: BTreeSet<String> = memory.pending.iter().filter(|(key, _)| key.starts_with(FILES)).filter_map(|(key, (value, _, _))| value.as_deref().map(|v| crate::blobs::name(sharing.key, &value_hash(key, v)))).collect();
        memory.sealed.damaged.retain(|name, _| waited.contains(name));
    }
    // The files this computer sealed in the folder that no record points to
    // any more go, in time; never one a record named lately.
    memory.sealed.named_at.retain(|_, at| now_ms - *at < crate::blobs::UNUSED_DAYS * DAY);
    if !memory.sealed.mine.is_empty() {
        let used: BTreeSet<String> = memory
            .entries
            .iter()
            .filter(|(key, known)| key.starts_with(FILES) && !known.h.is_empty())
            .map(|(_, known)| known.h.clone())
            .chain(memory.pending.iter().filter(|(key, _)| key.starts_with(FILES)).filter_map(|(key, (value, _, _))| value.as_deref().map(|v| value_hash(key, v))))
            .map(|h| crate::blobs::name(sharing.key, &h))
            .chain(memory.sealed.named_at.keys().cloned())
            .collect();
        prune_tables(sharing.folder, &mut memory.sealed, &used);
        let mut last: BTreeMap<String, i64> = memory.sealed.mine.iter().map(|(name, (at, _))| (name.clone(), *at)).collect();
        crate::blobs::sweep(sharing.folder, &mut last, &used, now_ms);
        memory.sealed.mine.retain(|name, (at, _)| last.get(name).map(|kept| *at = *kept).is_some());
    }
    // The history held within bounds, and what a crash left half written cleaned: after a writing, or hourly.
    if kept || now_ms - memory.tidied >= HOUR {
        crate::history::prune(&history, now_ms, crate::history::cap(&history), &|_, file| locate(stores, file).is_some_and(|(_, path)| path.exists()));
        clean_leftovers(&sharing.folder.join("blobs"));
        memory.tidied = now_ms;
    }
    // A full round, past `ROUND_SIZE` appended since the last: then the
    // rounds before it can go. Waits while the connection is metered or slow.
    if memory.grown > ROUND_SIZE && (!frugal(sharing.memory) || memory.grown > FRUGAL_ROUND_SIZE) {
        new_round(sharing, &mut memory, stores, &found, &hurried, now_ms)?;
        memory.round_base = own(&memory);
    }
    memory.own_size = own(&memory);
    memory.files = files;
    // Where each store was read; one not read this time where it was.
    let places: BTreeMap<String, String> = stores.iter().filter_map(|s| if reads(s) { Some((s.name.clone(), place(s))) } else { memory.places.get(&s.name).map(|p| (s.name.clone(), p.clone())) }).collect();
    memory.places = places;
    // What you said to take out went with this exchange; what waited and no longer does is forgotten.
    memory.sealed.confirmed.retain(|store| !stores.iter().any(|s| s.name == *store && reads(s)));
    memory.sealed.missing.retain(|key, _| memory.pending.contains_key(key));
    memory.sealed.failing.retain(|key, _| memory.pending.contains_key(key));
    write_seen(sharing, &memory, &all, now_ms);
    // What the devices' registry says of this export (`devices`).
    outcome.looked = clock;
    outcome.wrote = memory.joined.then_some((memory.round.max(1), memory.seq));
    remove_old_rounds(sharing, &memory, now_ms);
    memory.save(sharing.memory)?;
    Ok(outcome)
}

/// The whole lines of a computer's records from where they were left, read a
/// piece at a time (`READ_CHUNK`), one line held whole at most: a line longer
/// than any record (`LONGEST_LINE`: whoever wrote it) is given empty, a
/// broken line, and skipped to its end. A last line not ended yet is left for
/// the next exchange.
struct LineReader {
    file: std::fs::File,
    held: Vec<u8>,
    start: usize,
    /// Inside a line too long: its bytes so far, skipped.
    skipping: Option<u64>,
    end: bool,
}

impl LineReader {
    fn new(file: std::fs::File) -> LineReader {
        LineReader { file, held: Vec::new(), start: 0, skipping: None, end: false }
    }

    /// The next whole line and its length in the file.
    fn next(&mut self) -> Option<(Vec<u8>, u64)> {
        use std::io::Read;
        loop {
            if let Some(at) = self.held[self.start..].iter().position(|b| *b == b'\n') {
                let line = &self.held[self.start..=self.start + at];
                self.start += at + 1;
                return Some(match self.skipping.take() {
                    Some(skipped) => (Vec::new(), skipped + line.len() as u64),
                    None => (line.to_vec(), line.len() as u64),
                });
            }
            self.held.drain(..self.start);
            self.start = 0;
            if self.skipping.is_some() || self.held.len() > LONGEST_LINE {
                *self.skipping.get_or_insert(0) += self.held.len() as u64;
                self.held.clear();
            }
            if self.end {
                return None;
            }
            let mut piece = vec![0u8; READ_CHUNK];
            match self.file.read(&mut piece) {
                Ok(0) | Err(_) => self.end = true,
                Ok(n) => self.held.extend_from_slice(&piece[..n]),
            }
        }
    }
}

/// The sealed files this computer's records point to, looked for now and
/// then: one gone from the folder (taken out by hand, by another device that
/// sealed it long ago and swept it, by the server) or not of the size it was
/// sealed at (cut, damaged on the way) is sealed again from the file here,
/// while the file still holds that content.
fn look_after_sealed(sharing: &Sharing, stores: &[Store], memory: &mut Memory, found: &Found, hurried: &dyn Fn() -> bool, problems: &mut Vec<String>, now_ms: i64) {
    let ours: Vec<(String, String)> = memory.entries.iter().filter(|(key, known)| key.starts_with(FILES) && known.w == sharing.computer && !known.h.is_empty() && found.hashes.get(*key) == Some(&known.h)).map(|(key, known)| (key.clone(), known.h.clone())).collect();
    for (key, h) in ours {
        if hurried() {
            return;
        }
        let name = crate::blobs::name(sharing.key, &h);
        let there = std::fs::metadata(sharing.folder.join("blobs").join(&name)).map_or(0, |m| m.len());
        if there > 0 && memory.sealed.mine.get(&name).is_none_or(|(_, size)| *size == there) {
            continue;
        }
        let Some((_, path)) = locate(stores, file_of(&key)) else { continue };
        match crate::blobs::put_again(sharing.folder, sharing.key, &path, &h) {
            Ok(size) => {
                memory.sealed.mine.insert(name, (now_ms, size));
            }
            Err(e) => problems.push(e),
        }
    }
}

/// The names of the notes and papers here or known, as a storage that does not
/// tell case, nor how an accent is written, sees them (a phone's, a memory
/// card's, Windows', a Mac's): two it would take for one are never both written.
struct Names {
    by_folded: BTreeMap<String, String>,
}

impl Names {
    fn new(entries: &BTreeMap<String, Known>, found: &Found) -> Names {
        let known = entries.iter().filter(|(key, known)| key.starts_with(FILES) && !known.h.is_empty()).map(|(key, _)| key);
        let here = found.hashes.keys().filter(|key| key.starts_with(FILES));
        Names { by_folded: known.chain(here).map(|key| (folded(file_of(key)), file_of(key).to_string())).collect() }
    }

    /// Whether another file of that name, as such a storage sees it, is here or known.
    fn clash(&self, file: &str) -> bool {
        self.by_folded.get(&folded(file)).is_some_and(|other| other != file)
    }

    fn written(&mut self, file: &str, there: bool) {
        let name = folded(file);
        if there {
            self.by_folded.insert(name, file.to_string());
        } else if self.by_folded.get(&name).is_some_and(|other| other == file) {
            self.by_folded.remove(&name);
        }
    }
}

/// A name composed one way (NFC), its case folded.
fn folded(name: &str) -> String {
    icu_normalizer::ComposingNormalizerBorrowed::new_nfc().normalize(name).to_lowercase()
}

/// What came of another device's change to a file sealed apart.
enum Received {
    /// The file here holds it already.
    AlreadySo,
    /// Written: the file as it is now, for the next look to compare with
    /// (none once taken out); the copy kept beside of a version changed here
    /// too, if one was made; whether a version went into the history.
    Written { stat: Option<Stat>, copy: Option<String>, kept: bool },
    /// The file changed here since it was looked at: left for the next
    /// exchange, which sends that change, both kept.
    Later,
}

/// What writing the others' changes needs.
struct Receiving<'a> {
    sharing: &'a Sharing<'a>,
    history: &'a Path,
    now_ms: i64,
    /// This computer's clock (milliseconds): a file is never dated after it.
    clock: i64,
}

#[cfg(test)]
thread_local! {
    /// Run just before a received file takes its place, in this test's thread: an edit made then.
    static BEFORE_WRITING: std::cell::RefCell<Option<Box<dyn Fn(&Path)>>> = std::cell::RefCell::new(None);
}

/// Another device's change to a file sealed apart, written here: its content
/// opened from the folder beside the file and checked, then put in its place,
/// the file as it was kept first (`history`: by reference when a record named
/// that content, else copied). A file here that is neither what that change
/// replaced nor a content the others' records name (`seen`) changed here too:
/// it is kept beside, under a name that says so (its path in the folder
/// returned), never lost. A file changed here since it was looked at
/// (`gathered`) is not written over. Nothing is written without room for it,
/// for the copy kept, and some left (`disk::kept_free`).
#[allow(clippy::too_many_arguments)]
fn receive(how: &Receiving, store: &Store, file: &str, path: &Path, value: Option<&str>, current: Option<&str>, seen: &dyn Fn(&str) -> bool, known: Option<&Known>, gathered: Option<&Stat>, damaged: &mut BTreeMap<String, (u64, u64, u64)>) -> Result<Received, crate::blobs::Fault> {
    use crate::blobs::Fault;
    let sharing = how.sharing;
    let wanted = value.map(serde_json::from_str::<Reference>).transpose().map_err(|_| Fault::Broken)?;
    // What is here: as gathered, else (written since, or not read) read now.
    let current = match current {
        Some(here) => Some(here.to_string()),
        None if path.is_file() => crate::blobs::hash_file(path).ok().map(|(h, _)| h),
        None => None,
    };
    match (&wanted, &current) {
        (Some(wanted), Some(here)) if wanted.h == *here => return Ok(Received::AlreadySo),
        (None, None) => return Ok(Received::AlreadySo),
        _ => {}
    }
    let local = |e: std::io::Error| Fault::Local(format!("{}: {e}", path.display()));
    let size_here = std::fs::metadata(path).map_or(0, |m| m.len());
    let diverged = current.as_deref().is_some_and(|here| !seen(here));
    // The version here kept by reference when a record named it: its content is in the folder.
    let by_reference = current.as_deref().is_some_and(|here| known.is_some_and(|k| k.h == here) || seen(here));
    let room = wanted.as_ref().map_or(0, |w| w.s) + if by_reference { 0 } else { size_here } + if diverged { size_here } else { 0 };
    if wanted.as_ref().is_some_and(|w| w.s > crate::blobs::LARGEST) {
        return Err(Fault::Broken);
    }
    if !crate::disk::fits(path, room) {
        return Err(Fault::Room);
    }
    let temporary = temporary(path);
    if let Some(wanted) = &wanted {
        // Found damaged before, and the same copy still: not opened again.
        let blob = crate::blobs::name(sharing.key, &wanted.h);
        let source = blob_folder(sharing.folder, sharing.key, &wanted.h);
        let copy = std::fs::metadata(crate::blobs::path(&source, sharing.key, &wanted.h)).ok().map(|m| crate::blobs::fingerprint(&m));
        if copy.is_some() && damaged.get(&blob) == copy.as_ref() {
            return Err(Fault::Broken);
        }
        if let Some(parent) = path.parent() {
            private_dirs(parent).map_err(local)?;
        }
        match crate::blobs::get(&source, sharing.key, &wanted.h, &temporary) {
            Err(Fault::Broken) => {
                damaged.extend(copy.map(|copy| (blob, copy)));
                return Err(Fault::Broken);
            }
            Err(fault) => return Err(fault),
            Ok(()) => {
                damaged.remove(&blob);
            }
        }
    }
    let placed = (|| {
        #[cfg(test)]
        BEFORE_WRITING.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook(path);
            }
        });
        // Changed here since it was looked at (an editor, another program): not
        // written over; the next exchange sends that change first.
        let same = match (std::fs::symlink_metadata(path).ok(), gathered) {
            (None, None) => true,
            (Some(now), Some(then)) => now.len() == then.size && modified_ns(&now) == then.modified,
            _ => false,
        };
        if !same {
            return Ok(None);
        }
        let copy = match &current {
            Some(here) if diverged => Some(conflict_copy(path, here, known, how.now_ms).map_err(Fault::Local)?),
            _ => None,
        };
        let mut kept = false;
        if let Some(here) = current.as_deref()
            && path.exists()
        {
            let kept_now = if by_reference { crate::history::keep_sealed(how.history, store.part, file, here, size_here, how.now_ms) } else { crate::history::keep(how.history, store.part, file, path, how.now_ms) };
            kept_now.map_err(|e| if e.starts_with("share-no-room") { Fault::Room } else { Fault::Local(e) })?;
            kept = true;
        }
        let stat = match &wanted {
            Some(wanted) => {
                // The spam filter's table it replaces, kept beside when this Sioul can use it (`table.prev.bin`).
                if store.name == SPAM_TABLE {
                    sioul_core::spam::table::keep_previous(path);
                }
                std::fs::rename(&temporary, path).map_err(local)?;
                // Its time as where it was made, never after this computer's clock (another's may be ahead).
                if let Ok(written) = std::fs::File::options().write(true).open(path) {
                    let _ = written.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_millis(wanted.t.min(how.clock).max(0) as u64));
                }
                let now = std::fs::metadata(path).map_err(local)?;
                Some(Stat { size: now.len(), modified: modified_ns(&now), entries: vec![(String::new(), wanted.h.clone())], seen: how.clock })
            }
            None => {
                match std::fs::remove_file(path) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(local(e)),
                    _ => {}
                }
                // Its folders, left empty, go too.
                let mut dir = path.parent();
                while let Some(folder) = dir.filter(|d| *d != store.path && d.starts_with(&store.path)) {
                    if std::fs::remove_dir(folder).is_err() {
                        break;
                    }
                    dir = folder.parent();
                }
                None
            }
        };
        Ok(Some((stat, copy, kept)))
    })();
    if !matches!(placed, Ok(Some(_))) {
        let _ = std::fs::remove_file(&temporary);
    }
    let in_store = |copy: PathBuf| copy.strip_prefix(&store.path).map_or_else(|_| copy.display().to_string(), |r| r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
    Ok(match placed? {
        Some((stat, copy, kept)) => Received::Written { stat, copy: copy.map(in_store), kept },
        None => Received::Later,
    })
}

/// Where a sealed file is opened from: the sharing folder, else the copy
/// fetched from the server (`remote::overlay`) when only it holds that file.
fn blob_folder(folder: &Path, key: &[u8; 32], hash: &str) -> PathBuf {
    let there = |dir: &Path| std::fs::metadata(crate::blobs::path(dir, key, hash)).is_ok_and(|m| m.len() > 0);
    match crate::remote::overlay(folder) {
        Some(fetched) if !there(folder) && there(&fetched) => fetched,
        _ => folder.to_path_buf(),
    }
}

/// The name of the copy kept beside a file two devices changed, the same on
/// every device whatever its language: "lease (conflict 2026-10-05 16.05)",
/// dated (in this computer's time zone) by when the version it holds was made.
pub fn conflict_name(stem: &str, at_ms: i64) -> String {
    let when = jiff::Timestamp::from_millisecond(at_ms).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%d %H.%M").to_string()).unwrap_or_default();
    format!("{stem} (conflict {when})")
}

/// A file two devices changed, copied beside it before the other's version
/// takes its place, under `conflict_name`: dated by when the version it holds
/// was made, so that every device holding that version names it alike and one
/// copy travels, not one each. Returns where it is.
fn conflict_copy(path: &Path, here: &str, known: Option<&Known>, now_ms: i64) -> Result<PathBuf, String> {
    let at = known.filter(|k| k.h == here).map(|k| (k.c >> 16) as i64).or_else(|| std::fs::metadata(path).ok().map(|m| (modified_ns(&m) / 1_000_000) as i64)).unwrap_or(now_ms);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem.to_string(), format!(".{extension}")),
        _ => (name.clone(), String::new()),
    };
    let named = conflict_name(&stem, at);
    let mut n = 1;
    loop {
        let copy = path.with_file_name(if n == 1 { format!("{named}{extension}") } else { format!("{named} {n}{extension}") });
        if !copy.exists() {
            let temporary = temporary(&copy);
            return copy_private(path, &temporary).and_then(|()| std::fs::rename(&temporary, &copy)).map(|()| copy.clone()).map_err(|e| {
                let _ = std::fs::remove_file(&temporary);
                format!("{}: {e}", copy.display())
            });
        }
        // Kept already: another device's copy of the same version came first.
        if crate::blobs::hash_file(&copy).is_ok_and(|(h, _)| h == here) {
            return Ok(copy);
        }
        n += 1;
    }
}

/// Whether a file may be written where a record says: never through a link
/// below its store's folder (a link to elsewhere would carry the write out of
/// it), nor over a link, nor inside the sharing folder (it would travel in
/// plain) or Sioul's own state, but for the stores of that state itself
/// (`state/…`: the shield's answers, local lists' state, the spam filter's
/// label logs), which live there by design, each in its own folder.
fn writable(store: &Store, path: &Path, sharing: &Sharing) -> bool {
    writable_in(store, path, Some(sharing.folder), sharing.memory)
}

fn writable_in(store: &Store, path: &Path, folder: Option<&Path>, memory: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(&store.path) else { return false };
    let mut at = store.path.clone();
    for part in relative.components() {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    // Where it would land: the deepest of its folders that is there.
    let Some(there) = path.ancestors().skip(1).find(|p| p.exists()).and_then(|p| p.canonicalize().ok()) else { return true };
    let inside = |root: &Path| root.canonicalize().is_ok_and(|root| there.starts_with(root));
    let state = memory.parent().and_then(Path::parent);
    // A store of Sioul's own state writes in its own folder there (the path was
    // checked above: no link, nothing outside it); every other store, never in that state.
    let own_state = store.name.starts_with("state/") && state.is_some_and(|state| store.path.starts_with(state));
    !folder.is_some_and(inside) && (own_state || !state.is_some_and(inside))
}

/// Names Windows keeps for its devices ("aux.md", "COM1.txt"), and names
/// ending in a dot or a space, which it cuts.
fn kept_by_windows(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let numbered = (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && matches!(stem.as_bytes()[3], b'1'..=b'9');
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered || name.ends_with('.') || name.ends_with(' ')
}

/// The store and the file a key's file part names.
fn locate<'a>(stores: &'a [Store], file: &str) -> Option<(&'a Store, PathBuf)> {
    if let Some(store) = stores.iter().find(|s| !s.folder && s.name == file) {
        return Some((store, store.path.clone()));
    }
    let store = stores.iter().filter(|s| s.folder && file.starts_with(s.name.as_str())).max_by_key(|s| s.name.len())?;
    let relative = &file[store.name.len()..];
    // Never outside its folder, on Windows too, where "\" also separates and
    // "C:" starts another place.
    if relative.is_empty() || relative.split('/').any(|part| part.is_empty() || part == ".." || part == "." || part.contains(['\\', ':'])) {
        return None;
    }
    // Files sealed apart: never what tools make, nor what another part carries
    // (whatever its case), nor a name Windows keeps for itself, there.
    if matches!(store.shape, Shape::Files)
        && (relative.split('/').any(|part| made_by_tools(part) || matches!(part, "node_modules" | "target") || (cfg!(windows) && kept_by_windows(part))) || relative.split('/').next().is_some_and(|top| store.skip.iter().any(|skip| skip.eq_ignore_ascii_case(top))))
    {
        return None;
    }
    Some((store, relative.split('/').fold(store.path.clone(), |path, part| path.join(part))))
}

/// What putting a version back would change, said before it is done (`put_back_preview`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Back {
    /// A file whole (a note, a paper, a draft): it goes back as it was.
    pub whole: bool,
    /// Entries (settings, lines, marks, sessions) that go back to what they were.
    pub changed: usize,
    /// Entries taken out since, which come back.
    pub returning: usize,
    /// Entries added since, which stay.
    pub kept: usize,
}

/// What putting back `stamp` would change in `file` (`put_back`).
pub fn put_back_preview(memory: &Path, stores: &[Store], part: &str, file: &str, stamp: &str) -> Result<Back, String> {
    // Entries as this device's format divides the file.
    let shaped_stores = shaped(stores, format_of(memory));
    let stores = shaped_stores.as_slice();
    let (store, path) = locate(stores, file).filter(|(store, _)| store.part == part).ok_or_else(|| format!("{file}: not shared"))?;
    let kept = crate::history::version(&crate::history::root(memory), part, file, stamp).ok_or_else(|| format!("{file}: {stamp}: not kept"))?;
    let (crate::history::Kept::Copy(version), Shape::Toml(_) | Shape::Lines) = (kept, store.shape) else { return Ok(Back { whole: true, ..Back::default() }) };
    let (old, now) = entries_then_and_now(store, &version, &path)?;
    Ok(Back {
        whole: false,
        changed: old.iter().filter(|(entry, value)| now.get(*entry).is_some_and(|v| v != *value)).count(),
        returning: old.keys().filter(|entry| !now.contains_key(*entry)).count(),
        kept: now.keys().filter(|entry| !old.contains_key(*entry)).count(),
    })
}

/// A file's entries in a version kept, and in the file now.
fn entries_then_and_now(store: &Store, version: &Path, path: &Path) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>), String> {
    let old = std::fs::read(version).ok().and_then(|bytes| entries_of(store, &bytes)).ok_or_else(|| format!("{}: not readable", version.display()))?;
    let now = std::fs::read(path).ok().and_then(|bytes| entries_of(store, &bytes)).unwrap_or_default();
    Ok((old.into_iter().collect(), now.into_iter().collect()))
}

/// A version put back (`history`). A file whole goes back as it was, the file
/// as it is now kept first. A file of entries (settings, lists, the doses'
/// record, sessions) is put back entry by entry: each entry the version held
/// goes back to what it was, those taken out since come back, and those added
/// since stay. Putting back never takes out, on any device, what was added
/// after that version. The next exchange sends what changed as changes made
/// here, dated now. `stores` holds every part, those switched off too (a part
/// off here is put back here only); `vault` the sharing folder and its key,
/// for a version kept by reference. Waits for an exchange running.
pub fn put_back(memory: &Path, vault: Option<(&Path, &[u8; 32])>, stores: &[Store], part: &str, file: &str, stamp: &str, now_ms: i64) -> Result<(), String> {
    let shaped_stores = shaped(stores, format_of(memory));
    let stores = shaped_stores.as_slice();
    let (store, path) = locate(stores, file).filter(|(store, _)| store.part == part).ok_or_else(|| format!("{file}: not shared"))?;
    let root = crate::history::root(memory);
    let kept = crate::history::version(&root, part, file, stamp).ok_or_else(|| format!("{file}: {stamp}: not kept"))?;
    let _running = exchange_lock(memory, true)?;
    if store.folder && !writable_in(store, &path, vault.map(|(folder, _)| folder), memory) {
        return Err(format!("{}: not written there", path.display()));
    }
    if let (crate::history::Kept::Copy(version), Shape::Toml(_) | Shape::Lines) = (&kept, store.shape) {
        let (old, now) = entries_then_and_now(store, version, &path)?;
        let back: Vec<(&str, Option<&str>)> = old.iter().filter(|(entry, value)| now.get(*entry) != Some(*value)).map(|(entry, value)| (entry.as_str(), Some(value.as_str()))).collect();
        if back.is_empty() {
            return Ok(());
        }
        return write_entries(store, &path, &back, || crate::history::keep(&root, part, file, &path, now_ms)).map(|_| ());
    }
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        private_dirs(parent).map_err(fail)?;
    }
    let temporary = temporary(&path);
    let brought = match kept {
        crate::history::Kept::Copy(version) => copy_private(&version, &temporary).map_err(fail),
        crate::history::Kept::Sealed { hash, .. } => {
            let (folder, key) = vault.ok_or_else(|| format!("{file}: kept in the sharing folder, which is not set here"))?;
            crate::blobs::get(&blob_folder(folder, key, &hash), key, &hash, &temporary).map_err(|fault| match fault {
                crate::blobs::Fault::Missing => format!("share-missing:{file}"),
                crate::blobs::Fault::Broken => format!("share-damaged:{file}"),
                crate::blobs::Fault::Room => format!("share-no-room:{file}"),
                crate::blobs::Fault::Local(e) => e,
            })
        }
    };
    let put = || {
        brought?;
        if path.exists() {
            crate::history::keep(&root, part, file, &path, now_ms)?;
        }
        // Dated now, wherever the copy took its time from: a change made here, which the others take.
        let _ = std::fs::File::options().write(true).open(&temporary).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        std::fs::rename(&temporary, &path).map_err(fail)
    };
    let done = if matches!(store.shape, Shape::Files) { put() } else { sioul_core::filelock::with_lock(&path, put) };
    if done.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    done
}

/// For a file sealed apart, the content a change made here replaces: its hash as last known ("" new).
fn base_of(memory: &Memory, key: &str) -> String {
    if key.starts_with(FILES) { memory.entries.get(key).map(|k| k.h.clone()).unwrap_or_default() } else { String::new() }
}

/// A file sealed apart put in the folder before its record goes out (`blobs`);
/// false when it could not be (it changed while read, a full disk): its record
/// waits for the next look. Anything else needs nothing.
fn seal_apart(sharing: &Sharing, stores: &[Store], key: &str, value: &str, sealed: &mut Sealed, problems: &mut Vec<String>, now_ms: i64) -> bool {
    if !key.starts_with(FILES) {
        return true;
    }
    let (Some((_, path)), Ok(reference)) = (locate(stores, file_of(key)), serde_json::from_str::<Reference>(value)) else { return false };
    match crate::blobs::put(sharing.folder, sharing.key, &path, &reference.h) {
        Ok(written) => {
            let name = crate::blobs::name(sharing.key, &reference.h);
            if let Some(size) = written {
                sealed.mine.insert(name.clone(), (now_ms, size));
            }
            // A table of the spam filter this computer sealed: the newest of its own (`prune_tables`).
            if file_of(key) == SPAM_TABLE && sealed.mine.contains_key(&name) {
                sealed.tables.retain(|t| *t != name);
                sealed.tables.push(name);
            }
            true
        }
        Err(e) => {
            problems.push(e);
            false
        }
    }
}

fn append(sharing: &Sharing, memory: &mut Memory, changes: &[(String, Option<String>, u64, String)]) -> Result<(), String> {
    if changes.is_empty() {
        return Ok(());
    }
    let path = round_file(sharing.folder, sharing.computer, memory.round);
    let mut text = String::new();
    for (at, (key, value, clock, base)) in changes.iter().enumerate() {
        memory.seq += 1;
        let build = if at == 0 { sioul_core::build::DESCRIBED.to_string() } else { String::new() };
        // Its part's format, once this device writes format 2.
        let f = if memory.format >= 2 { known_part(file_of(key)).map_or(1, part_format) } else { 1 };
        let plain = serde_json::to_vec(&Change { k: key.clone(), v: value.clone(), b: base.clone(), build, f }).map_err(|e| e.to_string())?;
        let line = Line { n: memory.seq, c: *clock, s: seal(sharing.key, &bound(sharing.computer, memory.round, memory.seq, *clock), &plain) };
        text.push_str(&serde_json::to_string(&line).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    // A line left half written (Sioul stopped, a full disk): cut back to the
    // last whole line, so that the next record is not glued to it and lost.
    if let Some(whole) = unfinished(&path) {
        let file = std::fs::OpenOptions::new().write(true).open(&path).map_err(fail)?;
        file.set_len(whole).map_err(fail)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&path).map_err(fail)?;
    // A round begun before its files were yours alone: made so at this write.
    make_private(&path);
    file.write_all(text.as_bytes()).map_err(fail)?;
    file.sync_all().map_err(fail)
}

/// Where a file's last whole line ends, when it does not end one: its end
/// read back a piece at a time, never the file whole. None when it ends a
/// line, or is empty or not there.
fn unfinished(path: &Path) -> Option<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mut last = [0u8];
    file.seek(SeekFrom::Start(size.checked_sub(1)?)).ok()?;
    file.read_exact(&mut last).ok()?;
    if last[0] == b'\n' {
        return None;
    }
    let mut end = size;
    while end > 0 {
        let from = end.saturating_sub(64 << 10);
        let mut piece = vec![0u8; (end - from) as usize];
        file.seek(SeekFrom::Start(from)).ok()?;
        file.read_exact(&mut piece).ok()?;
        if let Some(at) = piece.iter().rposition(|b| *b == b'\n') {
            return Some(from + at as u64 + 1);
        }
        end = from;
    }
    Some(0)
}

/// This computer's last record in the folder: its round, number, and that file's size.
fn last_own(folder: &Path, computer: &str) -> Option<(u32, u64, u64)> {
    use std::io::{Read, Seek};
    let round = *rounds(folder).get(computer)?.last()?;
    let mut file = std::fs::File::open(round_file(folder, computer, round)).ok()?;
    let size = file.metadata().ok()?.len();
    // The tail is enough: lines are short; a line longer than that was a whole round's opening.
    let from = size.saturating_sub(64 * 1024);
    let mut tail = Vec::new();
    file.seek(std::io::SeekFrom::Start(from)).ok()?;
    file.read_to_end(&mut tail).ok()?;
    let n = tail.split(|b| *b == b'\n').rev().find_map(|line| std::str::from_utf8(line).ok().and_then(|t| serde_json::from_str::<Line>(t).ok())).map_or(0, |l| l.n);
    Some((round, n, size))
}

/// A new file for this computer, opening on every entry it holds the last
/// word on, with their clocks; entries taken out long ago are forgotten. A
/// file sealed apart missing from the folder (taken out there by hand) is put back first.
fn new_round(sharing: &Sharing, memory: &mut Memory, stores: &[Store], found: &Found, hurried: &dyn Fn() -> bool, now_ms: i64) -> Result<(), String> {
    let oldest = ((now_ms - TOMBSTONE_DAYS * 86_400_000).max(0) as u64) << 16;
    memory.entries.retain(|_, known| !known.h.is_empty() || known.c >= oldest);
    let ours: Vec<(String, Option<String>, u64, String)> = memory
        .entries
        .iter()
        .filter(|(_, known)| known.w == sharing.computer)
        .filter_map(|(key, known)| {
            if known.h.is_empty() {
                return (!found.hashes.contains_key(key)).then(|| (key.clone(), None, known.c, known.b.clone()));
            }
            // A file sealed apart from its last look, never read through for this.
            let value = match found.values.get(key) {
                Some(value) => value.clone(),
                None if key.starts_with(FILES) => found.files.get(file_of(key)).and_then(|stat| stat.entries.first().map(|(_, h)| reference(h, stat.size, stat.modified)))?,
                None => value_of(stores, key)?,
            };
            (value_hash(key, &value) == known.h).then(|| (key.clone(), Some(value), known.c, known.b.clone()))
        })
        .collect();
    let mut problems = Vec::new();
    for (key, value, _, _) in ours.iter().filter(|_| sharing.files && !hurried()) {
        if let Some(value) = value {
            seal_apart(sharing, stores, key, value, &mut memory.sealed, &mut problems, now_ms);
        }
    }
    memory.round += 1;
    memory.seq = 0;
    memory.full_round = memory.round;
    memory.grown = 0;
    append(sharing, memory, &ours)
}

/// The sealed files this computer put in the folder (`blobs/<name>`), by
/// name, and the size it sealed each at: what it sends to the server itself
/// beside the sync app (`remote::send`), whole, and nothing else of `blobs/`.
pub fn own_sealed(memory: &Path, computer: &str) -> BTreeMap<String, u64> {
    std::fs::read_to_string(sealed_path(memory)).ok().and_then(|t| serde_json::from_str::<Sealed>(&t).ok()).filter(|s| s.computer == computer).map(|s| s.mine.into_iter().map(|(name, (_, size))| (name, size)).collect()).unwrap_or_default()
}

/// How far this computer wrote its records: its round and the number of the
/// last, for its claims to say (`lease::Claim::wrote`); none before sharing.
pub fn written(memory: &Path, computer: &str) -> Option<(u32, u64)> {
    let memory = Memory::load(memory, computer);
    memory.joined.then_some((memory.round.max(1), memory.seq))
}

/// What this computer read of the others' records.
#[derive(Debug, Default, Clone)]
pub struct Heard {
    /// Each computer's last record read here: its round and number.
    pub read: BTreeMap<String, (u32, u64)>,
    /// When a line of each could not be read here (Unix seconds).
    pub broken: BTreeMap<String, i64>,
}

impl Heard {
    /// Whether everything `computer` wrote up to `wrote` (its round and
    /// number, as its claim says) is read here. Nothing written yet (number 0)
    /// is all read; a round begun anew opens with everything it holds.
    pub fn complete(&self, computer: &str, wrote: (u32, u64)) -> bool {
        wrote.1 == 0 || self.read.get(computer).is_some_and(|read| *read >= wrote)
    }
}

/// When each part last sent a change from here, and last received one (Unix seconds; 0 never).
pub fn traffic(memory: &Path, computer: &str) -> BTreeMap<String, (i64, i64)> {
    Memory::load(memory, computer).traffic
}

pub fn heard(memory: &Path, computer: &str) -> Heard {
    let memory = Memory::load(memory, computer);
    Heard { read: memory.read_n, broken: memory.broken }
}

/// The entries of `files` read again from every computer's records, this
/// one's too, at the next exchange; this computer forgets it held them, so
/// that a record lost or broken here, started again empty, takes nothing out
/// elsewhere and gets back what was written (docs/database.md, "Prudence").
pub fn rebuild(memory: &Path, computer: &str, files: &[&str]) -> Result<(), String> {
    let _running = exchange_lock(memory, true)?;
    let mut state = Memory::load_all(memory, computer);
    state.entries.retain(|key, _| !files.contains(&file_of(key)));
    state.pending.retain(|key, _| !files.contains(&file_of(key)));
    for file in files {
        state.files.remove(*file);
        state.rebuild.insert((*file).to_string());
    }
    // The others' records read again from their oldest kept round.
    state.read.clear();
    state.save(memory)
}

/// The files gone at once from a folder of notes or papers (`share-vanished`)
/// taken out everywhere at the next exchange, as you said: the folder by its
/// name in the records ("files/notes/").
pub fn confirm_gone(memory: &Path, computer: &str, store: &str) -> Result<(), String> {
    let _running = exchange_lock(memory, true)?;
    let mut state = Memory::load_all(memory, computer);
    state.sealed.confirmed.insert(store.to_string());
    state.save(memory)
}

/// One exchange at a time on this computer, whatever runs it (two Sioul
/// started, the command line): a lock on `<state>/share/exchange.lock`, held
/// while it runs, let go when the file returned is. `wait`: until the
/// running one ends; else none while another holds it a second on.
pub fn exchange_lock(memory: &Path, wait: bool) -> Result<Option<std::fs::File>, String> {
    let path = memory.with_file_name("exchange.lock");
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        private_dirs(parent).map_err(fail)?;
    }
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path).map_err(fail)?;
    if wait {
        file.lock().map_err(fail)?;
        return Ok(Some(file));
    }
    // A program starting elsewhere in Sioul holds a copy of every file open
    // then until it runs (a Mac's take milliseconds): the last exchange's lock,
    // let go a moment before, may still be held by it. Tried again for a second.
    for _ in 0..50 {
        match file.try_lock() {
            Ok(()) => return Ok(Some(file)),
            Err(std::fs::TryLockError::WouldBlock) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(std::fs::TryLockError::Error(e)) => return Err(fail(e)),
        }
    }
    Ok(None)
}

fn seen_path(folder: &Path, computer: &str) -> PathBuf {
    folder.join(format!("{computer}.toml"))
}

/// This computer's notes in the folder, when they change or a quarter of an
/// hour after the last: the sync is not asked to carry a file every minute.
fn write_seen(sharing: &Sharing, memory: &Memory, all: &BTreeMap<String, Vec<u32>>, now_ms: i64) {
    let mut seen = Seen {
        at: now_ms / 1000,
        round: memory.round,
        read: memory.read.iter().filter(|(c, _)| all.contains_key(*c)).map(|(c, (round, _))| (c.clone(), *round)).collect(),
        pad: String::new(),
    };
    if read_seen_in(sharing.folder, sharing.computer).is_some_and(|old| old.round == seen.round && old.read == seen.read && seen.at - old.at < 15 * 60) {
        return;
    }
    let path = seen_path(sharing.folder, sharing.computer);
    let text = |seen: &Seen, pad: usize| toml::to_string(&Seen { pad: ".".repeat(pad), read: seen.read.clone(), ..*seen }).unwrap_or_default();
    seen.pad = ".".repeat(pad_for(&path, |pad| text(&seen, pad).len() as u64));
    if let Ok(text) = toml::to_string(&seen) {
        let _ = write_atomically(&path, text.as_bytes());
    }
}

fn read_seen_in(folder: &Path, computer: &str) -> Option<Seen> {
    read_small(&seen_path(folder, computer)).and_then(|t| toml::from_str(&t).ok())
}

/// Another computer's notes, the later of the folder's and the copy fetched
/// from the server (`remote::overlay`); the folder's on a tie.
fn read_seen(folder: &Path, computer: &str) -> Option<Seen> {
    let here = read_seen_in(folder, computer);
    match (here, crate::remote::overlay(folder).and_then(|fetched| read_seen_in(&fetched, computer))) {
        (Some(here), Some(fetched)) => Some(if fetched.at > here.at { fetched } else { here }),
        (here, fetched) => here.or(fetched),
    }
}

/// This computer's old files, once every other computer heard lately has read past them.
fn remove_old_rounds(sharing: &Sharing, memory: &Memory, now_ms: i64) {
    let all = rounds(sharing.folder);
    let others: Vec<Seen> = computers(sharing.folder).iter().filter(|c| *c != sharing.computer).filter_map(|c| read_seen(sharing.folder, c)).filter(|s| now_ms / 1000 - s.at < SILENT_DAYS * 86_400).collect();
    let read_up_to = others.iter().map(|s| s.read.get(sharing.computer).copied().unwrap_or(0)).min().unwrap_or(memory.round);
    // Never the last full round, nor the rounds continuing it: a device
    // joining reads from the oldest kept, and finds every entry there.
    let full = if memory.full_round == 0 { memory.round } else { memory.full_round };
    for round in all.get(sharing.computer).into_iter().flatten().filter(|r| **r < full && **r < read_up_to) {
        let _ = std::fs::remove_file(round_file(sharing.folder, sharing.computer, *round));
    }
}

/// Another computer sharing through the folder, and when it last exchanged (Unix seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Other {
    pub id: String,
    pub heard: i64,
}

/// The other computers sharing through the folder.
pub fn others(folder: &Path, computer: &str) -> Vec<Other> {
    computers(folder).into_iter().filter(|c| c != computer).map(|c| Other { heard: read_seen(folder, &c).map_or(0, |s| s.at), id: c }).collect()
}

/// Whether the folder already holds a seal: another computer shares through it.
pub fn sealed(folder: &Path) -> bool {
    folder.join("seal.toml").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    thread_local! {
        /// Stores this thread's exchanges know nothing of, as an older Sioul's
        /// (`known_part`): their records wait, written nowhere, taken out nowhere.
        pub(super) static OLDER: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// A computer of its own: its folders, its name, its memory.
    pub(super) struct Computer {
        pub(super) roots: Roots,
        pub(super) id: String,
        pub(super) memory: PathBuf,
    }

    impl Computer {
        pub(super) fn new(base: &Path, name: &str) -> Computer {
            let root = base.join(name);
            let roots = Roots { config: root.join("config"), data: root.join("data"), state: root.join("state") };
            for dir in [&roots.config, &roots.data, &roots.state] {
                std::fs::create_dir_all(dir).unwrap();
            }
            let memory = roots.state.join("share").join("memory.json");
            Computer { roots, id: uuid::Uuid::new_v4().to_string(), memory }
        }

        pub(super) fn exchange(&self, folder: &Path, key: &[u8; 32], now: i64) -> Outcome {
            let stores = stores(&Config::default(), &self.roots);
            exchange(&Sharing { folder, computer: &self.id, key, memory: &self.memory, files: true, hurry: None }, &stores, now).unwrap()
        }

        /// An exchange carrying the projects and budgets of `notes`, this computer's notes folder.
        pub(super) fn exchange_notes(&self, folder: &Path, key: &[u8; 32], now: i64, notes: &Path) -> Outcome {
            let config = Config { share_projects: true, notes_root: Some(notes.display().to_string()), ..Config::default() };
            let stores = stores(&config, &self.roots);
            exchange(&Sharing { folder, computer: &self.id, key, memory: &self.memory, files: true, hurry: None }, &stores, now).unwrap()
        }

        /// An exchange with `notes` as this computer's notes folder, the parts `on` shared besides those shared by default.
        fn exchange_with(&self, folder: &Path, key: &[u8; 32], now: i64, notes: &Path, on: &[&str]) -> Outcome {
            let stores = self.stores_with(notes, &|part| on.contains(&part));
            exchange(&Sharing { folder, computer: &self.id, key, memory: &self.memory, files: true, hurry: None }, &stores, now).unwrap()
        }

        fn stores_with(&self, notes: &Path, on: &dyn Fn(&str) -> bool) -> Vec<Store> {
            let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
            stores_of(&config, &self.roots, &|part| on(part) || shared_by_default(part, &config))
        }

        pub(super) fn write(&self, relative: &str, text: &str) {
            let path = self.path(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        pub(super) fn read(&self, relative: &str) -> String {
            std::fs::read_to_string(self.path(relative)).unwrap_or_default()
        }

        pub(super) fn path(&self, relative: &str) -> PathBuf {
            let (root, rest) = relative.split_once('/').unwrap();
            match root {
                "config" => self.roots.config.join(rest),
                "data" => self.roots.data.join(rest),
                _ => self.roots.state.join(rest),
            }
        }
    }

    pub(super) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-share-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub(super) fn quick_key(folder: &Path, passphrase: &str) -> Result<[u8; 32], Refused> {
        key_with(folder, passphrase, 64, 1)
    }

    fn put(dir: &Path, relative: &str, bytes: &[u8]) {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    /// Bytes that do not compress, the same for the same seed: a scan, a picture.
    fn noise(length: usize, seed: u32) -> Vec<u8> {
        let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
        (0..length)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect()
    }

    /// The files of a notes folder, hidden ones left out, by their path from it.
    fn files_in(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut found = Vec::new();
        list_files(dir, dir, &[], &mut found);
        found.into_iter().map(|(name, path)| (name, std::fs::read(path).unwrap())).collect()
    }

    /// A version kept, copied or by reference (its content opened from the folder).
    fn version_text(history: &Path, part: &str, file: &str, stamp: &str, folder: &Path, key: &[u8; 32]) -> String {
        match crate::history::version(history, part, file, stamp) {
            Some(crate::history::Kept::Copy(path)) => std::fs::read_to_string(path).unwrap(),
            Some(crate::history::Kept::Sealed { hash, .. }) => {
                let target = temporary(&history.join("opened"));
                crate::blobs::get(folder, key, &hash, &target).unwrap();
                let text = std::fs::read_to_string(&target).unwrap();
                std::fs::remove_file(target).unwrap();
                text
            }
            None => panic!("{file}: {stamp}: not kept"),
        }
    }

    /// The copies a device kept of a file two devices changed: "lease (… …).md", not a sync app's.
    fn copies_of(dir: &Path, stem: &str) -> Vec<Vec<u8>> {
        files_in(dir).into_iter().filter(|(name, _)| name.starts_with(&format!("{stem} (")) && name.ends_with(").md") && !made_by_tools(name)).map(|(_, bytes)| bytes).collect()
    }

    pub(super) const MINUTE: i64 = 60_000;
    pub(super) const NOW: i64 = 1_790_000_000_000;

    /// How eDrive (Murena's sync on /e/OS) carries a folder between a phone and
    /// the server, after its `FileDiffUtils`: a file new on either side is
    /// copied; a file changed on the phone goes up; a file changed on the server
    /// comes down only when its size differs from the phone's copy (else only
    /// its version is noted); nothing is ever deleted. The server here is the
    /// desktop's folder: Nextcloud's client keeps them equal.
    #[derive(Default)]
    struct Edrive {
        /// path → (the server's content when last seen, the phone's content then).
        known: BTreeMap<String, (String, String)>,
    }

    impl Edrive {
        fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
            let mut found = Vec::new();
            list_files(root, root, &[], &mut found);
            found.into_iter().filter(|(name, _)| !name.split('/').any(|p| p.starts_with('.'))).filter_map(|(name, path)| std::fs::read(path).ok().map(|b| (name, b))).collect()
        }

        fn carry(&mut self, server: &Path, phone: &Path) {
            let digest = |bytes: &[u8]| hash(&String::from_utf8_lossy(bytes));
            let (up, down) = (Self::files(phone), Self::files(server));
            for (name, bytes) in &up {
                let changed_here = self.known.get(name).is_none_or(|(_, local)| *local != digest(bytes));
                if changed_here {
                    let to = server.join(name);
                    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                    std::fs::write(&to, bytes).unwrap();
                    self.known.insert(name.clone(), (digest(bytes), digest(bytes)));
                }
            }
            for (name, bytes) in &down {
                let local = phone.join(name);
                match (self.known.get(name), std::fs::read(&local)) {
                    (Some((remote, _)), _) if *remote == digest(bytes) => {}
                    // Changed there, the same size here: only the version is noted.
                    (Some(_), Ok(here)) if here.len() == bytes.len() => {
                        self.known.insert(name.clone(), (digest(bytes), digest(&here)));
                    }
                    _ => {
                        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
                        std::fs::write(&local, bytes).unwrap();
                        self.known.insert(name.clone(), (digest(bytes), digest(bytes)));
                    }
                }
            }
        }
    }

    /// How a sync app carries the folder between a phone and the server, in
    /// the ways they differ: the sharing must hold with each (docs/database.md,
    /// "What the sync app must do").
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Carrying {
        /// Every change, both ways, deletions too (Nextcloud's desktop client, Dropbox, Drive, OneDrive).
        Exact,
        /// A change from the server comes only when the size differs (eDrive).
        SizeOnly,
        /// Half the files each time, late, in any order; nothing deleted.
        Late,
        /// An older copy of a file put back from time to time, a conflicted copy beside it.
        StaleCopies,
        /// New files first come as empty placeholders, the content at the next look (on-demand files).
        Placeholders,
    }

    struct Carrier {
        how: Carrying,
        turn: u64,
        /// path → (the server's content when last seen, the phone's content then).
        known: BTreeMap<String, (String, String)>,
        /// Every version seen on the server, to put an older one back.
        history: BTreeMap<String, Vec<Vec<u8>>>,
    }

    impl Carrier {
        fn new(how: Carrying) -> Carrier {
            Carrier { how, turn: 0, known: BTreeMap::new(), history: BTreeMap::new() }
        }

        fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
            let mut found = Vec::new();
            list_files(root, root, &[], &mut found);
            found.into_iter().filter(|(name, _)| !name.split('/').any(|p| p.starts_with('.'))).filter_map(|(name, path)| std::fs::read(path).ok().map(|b| (name, b))).collect()
        }

        fn write(path: &Path, bytes: &[u8]) {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }

        fn carry(&mut self, server: &Path, phone: &Path) {
            self.turn += 1;
            let digest = |bytes: &[u8]| hash(&String::from_utf8_lossy(bytes));
            let (up, down) = (Self::files(phone), Self::files(server));
            for (name, bytes) in &up {
                let changed_here = self.known.get(name).is_none_or(|(_, local)| *local != digest(bytes));
                // A placeholder never goes up.
                if changed_here && !(self.how == Carrying::Placeholders && bytes.is_empty() && down.contains_key(name)) {
                    Self::write(&server.join(name), bytes);
                    self.known.insert(name.clone(), (digest(bytes), digest(bytes)));
                }
            }
            for (i, (name, bytes)) in down.iter().enumerate() {
                self.history.entry(name.clone()).or_default().push(bytes.clone());
                if self.how == Carrying::Late && (i as u64 + self.turn) % 2 == 0 {
                    continue;
                }
                let local = phone.join(name);
                let here = std::fs::read(&local).ok();
                match (self.known.get(name), here) {
                    (Some((remote, _)), _) if *remote == digest(bytes) => {}
                    (Some(_), Some(here)) if self.how == Carrying::SizeOnly && here.len() == bytes.len() => {
                        self.known.insert(name.clone(), (digest(bytes), digest(&here)));
                    }
                    (None, None) if self.how == Carrying::Placeholders => {
                        Self::write(&local, b"");
                        self.known.insert(name.clone(), (String::new(), digest(b"")));
                    }
                    _ => {
                        Self::write(&local, bytes);
                        self.known.insert(name.clone(), (digest(bytes), digest(bytes)));
                    }
                }
            }
            if self.how == Carrying::Exact {
                for name in up.keys().filter(|n| !down.contains_key(*n) && self.known.contains_key(*n)) {
                    let _ = std::fs::remove_file(phone.join(name));
                }
            }
            // From time to time, an older copy of a record put back on the server, a conflicted copy beside it.
            if self.how == Carrying::StaleCopies && self.turn % 7 == 0 {
                for (name, versions) in self.history.iter().filter(|(n, _)| n.ends_with(".jsonl")) {
                    if versions.len() > 2 {
                        let current = std::fs::read(server.join(name)).unwrap_or_default();
                        Self::write(&server.join(format!("{name} (conflicted copy)")), &current);
                        Self::write(&server.join(name), &versions[versions.len() / 2]);
                    }
                }
            }
        }
    }

    #[test]
    fn doses_are_never_known_wrongly_whatever_carries_them() {
        for how in [Carrying::Exact, Carrying::SizeOnly, Carrying::Late, Carrying::StaleCopies, Carrying::Placeholders] {
            let base = scratch(&format!("carrier-{how:?}").to_lowercase());
            let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
            let key = quick_key(&server, "four words make a passphrase").unwrap();
            let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
            let mut carrier = Carrier::new(how);
            let mut marked: Vec<String> = Vec::new();
            for minute in 0..120 {
                let now = NOW + minute * MINUTE;
                // A dose marked on the desk every ten minutes, its claim renewed each minute.
                if minute % 10 == 0 {
                    marked.push(format!("dose@{minute}"));
                    let record: String = marked.iter().map(|d| format!("\"{d}\" = {minute}\n")).collect();
                    desk.write("state/health-state.toml", &format!("[taken]\n{record}"));
                }
                desk.exchange(&server, &key, now);
                let wrote = written(&desk.memory, &desk.id);
                crate::lease::renew(&server, &key, "health", &desk.id, now / 1000, now / 1000, false, crate::lease::Rule::FollowsYou, wrote).unwrap();
                carrier.carry(&server, &phone_folder);
                // texts: a first import running on the phone beside it, a batch each minute and a picture each ten.
                let import: Vec<sioul_core::texts::Text> = (0..20).map(|i| sioul_core::texts::Text { id: format!("sms-{minute}-{i}"), at: NOW - 86_400_000 + minute * MINUTE + i, direction: "in".into(), with: vec!["+33199001234".into()], body: format!("Message {minute}-{i}"), ..sioul_core::texts::Text::default() }).collect();
                sioul_core::texts::append(&sioul_core::texts::own_file(&texts_of(&phone), sioul_core::texts::LOG, &phone.id), &import, &crate::textseal::TextSeal::new(&key), None).unwrap();
                if minute % 10 == 0 {
                    let picture = base.join(format!("picture-{minute}.bin"));
                    std::fs::write(&picture, vec![minute as u8; 256 * 1024]).unwrap();
                    let (hash, _) = crate::blobs::hash_file(&picture).unwrap();
                    crate::blobs::put(&phone_folder, &key, &picture, &hash).unwrap();
                }
                phone.exchange_with(&phone_folder, &key, now + 30_000, &base.join("phone-notes"), &[sioul_core::texts::PART]);
                // What the phone takes as known must be there: a dose known not taken that was taken is the harm.
                if let Some(claim) = crate::lease::claims(&phone_folder, &key, "health").into_iter().find(|c| c.computer == desk.id)
                    && let Some(wrote) = claim.wrote
                    && heard(&phone.memory, &phone.id).complete(&desk.id, wrote)
                {
                    let record = phone.read("state/health-state.toml");
                    let claimed = (claim.renewed - NOW / 1000) / 60;
                    for dose in marked.iter().filter(|d| d[5..].parse::<i64>().unwrap() < claimed) {
                        assert!(record.contains(dose.as_str()), "{how:?}, minute {minute}: the phone takes {dose} as known, its record lacks it:\n{record}");
                    }
                }
            }
            // In the end every dose is there, whatever carried them.
            for minute in 120..140 {
                let now = NOW + minute * MINUTE;
                desk.exchange(&server, &key, now);
                carrier.carry(&server, &phone_folder);
                phone.exchange(&phone_folder, &key, now + 30_000);
            }
            let record = phone.read("state/health-state.toml");
            for dose in &marked {
                assert!(record.contains(dose.as_str()), "{how:?}: {dose} never came:\n{record}");
            }
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn edrive_brings_every_rewrite() {
        let base = scratch("edrive");
        let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
        let key = quick_key(&server, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let mut edrive = Edrive::default();
        let health = |computer: &Computer, folder: &Path, now: i64, closed: bool| {
            let wrote = written(&computer.memory, &computer.id);
            if closed {
                crate::lease::close(folder, &key, "health", &computer.id, now / 1000, wrote).unwrap();
            } else {
                crate::lease::renew(folder, &key, "health", &computer.id, now / 1000, now / 1000, false, crate::lease::Rule::FollowsYou, wrote).unwrap();
            }
        };

        // The desk marks a dose and shares it; the phone joins through eDrive.
        desk.write("state/health-state.toml", "[taken]\n\"levothyroxine@2026-10-05T07:30\" = 1790000000\n");
        desk.exchange(&server, &key, NOW);
        health(&desk, &server, NOW, false);
        edrive.carry(&server, &phone_folder);
        phone.exchange(&phone_folder, &key, NOW + MINUTE);
        assert!(phone.read("state/health-state.toml").contains("levothyroxine@2026-10-05T07:30"));

        // Hours of rewrites on the desk, each carried: every one comes, the last one closed.
        for minute in 2..200 {
            let now = NOW + minute * MINUTE;
            if minute == 120 {
                desk.write("state/health-state.toml", &format!("{}\"levothyroxine@2026-10-06T07:30\" = 1790090000\n", desk.read("state/health-state.toml")));
            }
            desk.exchange(&server, &key, now);
            health(&desk, &server, now, minute == 199);
            edrive.carry(&server, &phone_folder);
            for name in [format!("{}.toml", desk.id), format!("leases/health/{}.lease", desk.id)] {
                assert_eq!(std::fs::read(phone_folder.join(&name)).ok(), std::fs::read(server.join(&name)).ok(), "minute {minute}: {name} stale on the phone");
            }
        }
        // On the phone: the second dose, and the desk's claim saying it closed and how far it wrote.
        phone.exchange(&phone_folder, &key, NOW + 200 * MINUTE);
        assert!(phone.read("state/health-state.toml").contains("levothyroxine@2026-10-06T07:30"));
        let claim = crate::lease::claims(&phone_folder, &key, "health").into_iter().find(|c| c.computer == desk.id).unwrap();
        assert!(claim.closed && claim.wrote == written(&desk.memory, &desk.id), "{claim:?}");
        // The phone's own files went up untouched.
        let phone_record = format!("{}-1.jsonl", phone.id);
        assert_eq!(std::fs::read(server.join(&phone_record)).ok(), std::fs::read(phone_folder.join(&phone_record)).ok());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The build that wrote them (docs/database.md, "The log"): on the first
    /// line of each batch a computer appends, sealed with it; lines without it
    /// (an older Sioul's) read as ever.
    #[test]
    fn each_batch_of_records_says_its_build() {
        let base = scratch("build");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
        desk.exchange(&folder, &key, NOW);
        let own = round_file(&folder, &desk.id, 1);
        let first = std::fs::read_to_string(&own).unwrap().lines().count();
        desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\nc@example.org\n");
        desk.exchange(&folder, &key, NOW + MINUTE);
        let text = std::fs::read_to_string(&own).unwrap();
        let builds: Vec<String> = text
            .lines()
            .map(|l| {
                let line: Line = serde_json::from_str(l).unwrap();
                serde_json::from_slice::<Change>(&open(&key, &bound(&desk.id, 1, line.n, line.c), &line.s).unwrap()).unwrap().build
            })
            .collect();
        assert!(first >= 2 && builds.len() > first, "{builds:?}");
        for (n, build) in builds.iter().enumerate() {
            assert_eq!(build.as_str(), if n == 0 || n == first { sioul_core::build::DESCRIBED } else { "" }, "line {n}");
        }
        assert!(!text.contains(sioul_core::build::DESCRIBED), "sealed: the server never learns it");
        // A new round opens with the build too.
        std::fs::write(&own, text.lines().next().unwrap().to_string() + "\n").unwrap();
        desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        let line: Line = serde_json::from_str(std::fs::read_to_string(round_file(&folder, &desk.id, 2)).unwrap().lines().next().unwrap()).unwrap();
        let change: Change = serde_json::from_slice(&open(&key, &bound(&desk.id, 2, line.n, line.c), &line.s).unwrap()).unwrap();
        assert_eq!(change.build, sioul_core::build::DESCRIBED);
        // Another computer reads them all, those written before the build was said among them.
        let laptop = Computer::new(&base, "laptop");
        let stranger = uuid::Uuid::new_v4().to_string();
        append_to(&round_file(&folder, &stranger, 1), &record_of(&key, &stranger, 1, ((NOW + MINUTE) as u64) << 16, &Change { k: "config/safe-senders.txt#d@example.org".into(), v: Some(String::new()), b: String::new(), build: String::new(), f: 0 }));
        laptop.exchange(&folder, &key, NOW + 3 * MINUTE);
        let safe = laptop.read("config/safe-senders.txt");
        assert!(["a@", "b@", "c@", "d@"].iter().all(|a| safe.contains(a)), "{safe}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_record_put_back_shorter_starts_again() {
        let base = scratch("cut");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        desk.write("config/safe-senders.txt", "a@example.org\n");
        desk.exchange(&folder, &key, NOW);
        desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
        desk.exchange(&folder, &key, NOW + MINUTE);
        // A sync app puts back the first version of the desk's file: its second line is lost.
        let own = round_file(&folder, &desk.id, 1);
        let text = std::fs::read_to_string(&own).unwrap();
        std::fs::write(&own, text.lines().next().unwrap().to_string() + "\n").unwrap();
        desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\nc@example.org\n");
        let outcome = desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert!(outcome.problems.iter().any(|p| p == "share-own-cut"), "{outcome:?}");
        assert!(round_file(&folder, &desk.id, 2).exists(), "a new file, opening on all the desk holds");
        // A computer joining now hears all three.
        let laptop = Computer::new(&base, "laptop");
        laptop.exchange(&folder, &key, NOW + 3 * MINUTE);
        let safe = laptop.read("config/safe-senders.txt");
        assert!(["a@", "b@", "c@"].iter().all(|a| safe.contains(a)), "{safe}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_memory_restored_or_lost_goes_on_numbering() {
        let base = scratch("restored");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        desk.write("state/health-state.toml", "[taken]\n\"a@100\" = 100\n");
        desk.exchange(&folder, &key, NOW);
        phone.exchange(&folder, &key, NOW + MINUTE);
        // A backup of the desk now; then a dose marked and shared.
        let backup = (std::fs::read(&desk.memory).unwrap(), desk.read("state/health-state.toml"));
        desk.write("state/health-state.toml", "[taken]\n\"a@100\" = 100\n\"b@800\" = 800\n");
        desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        phone.exchange(&folder, &key, NOW + 3 * MINUTE);
        assert!(phone.read("state/health-state.toml").contains("b@800"));
        let before = written(&desk.memory, &desk.id).unwrap();
        // The desk restored from its backup: the dose it marked comes back, its numbering goes on.
        std::fs::write(&desk.memory, &backup.0).unwrap();
        desk.write("state/health-state.toml", &backup.1);
        let outcome = desk.exchange(&folder, &key, NOW + 4 * MINUTE);
        assert!(outcome.problems.iter().any(|p| p == "share-own-ahead"), "{outcome:?}");
        assert!(desk.read("state/health-state.toml").contains("b@800"), "{}", desk.read("state/health-state.toml"));
        desk.write("state/health-state.toml", &format!("{}\"c@900\" = 900\n", desk.read("state/health-state.toml")));
        desk.exchange(&folder, &key, NOW + 5 * MINUTE);
        let after = written(&desk.memory, &desk.id).unwrap();
        assert!(after > before && after.0 == before.0, "numbering goes on in the same round: {before:?} → {after:?}");
        // The phone reads it all, without a gap, and knows it all.
        let outcome = phone.exchange(&folder, &key, NOW + 6 * MINUTE);
        assert!(!outcome.problems.iter().any(|p| p.starts_with("share-other-gap")), "{outcome:?}");
        assert!(phone.read("state/health-state.toml").contains("c@900"));
        assert!(heard(&phone.memory, &phone.id).complete(&desk.id, after));
        // "Stop sharing", then again: the memory gone, the same.
        std::fs::remove_file(&desk.memory).unwrap();
        desk.write("state/health-state.toml", &format!("{}\"d@950\" = 950\n", desk.read("state/health-state.toml")));
        desk.exchange(&folder, &key, NOW + 7 * MINUTE);
        assert!(written(&desk.memory, &desk.id).unwrap() > after);
        phone.exchange(&folder, &key, NOW + 8 * MINUTE);
        assert!(phone.read("state/health-state.toml").contains("d@950"));
        assert!(heard(&phone.memory, &phone.id).complete(&desk.id, written(&desk.memory, &desk.id).unwrap()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_gap_is_a_doubt_and_a_damaged_line_is_one_line() {
        let base = scratch("gap");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        desk.write("config/safe-senders.txt", "a@example.org\n");
        desk.exchange(&folder, &key, NOW);
        phone.exchange(&folder, &key, NOW + MINUTE);
        // A line that is not text, then one that is: the second is read all the same.
        let own = round_file(&folder, &desk.id, 1);
        let mut bytes = std::fs::read(&own).unwrap();
        bytes.extend_from_slice(b"\xff\xfe broken\n");
        std::fs::write(&own, &bytes).unwrap();
        desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
        desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        let outcome = phone.exchange(&folder, &key, NOW + 3 * MINUTE);
        assert!(outcome.problems.iter().any(|p| p.starts_with("share-other-line")), "{outcome:?}");
        assert!(phone.read("config/safe-senders.txt").contains("b@example.org"));
        // Lines missing (a stale copy of the file put back, then written after): a gap, said, and not known.
        let line = |n: u64, value: &str| {
            let clock = (NOW as u64) << 16 | n;
            let plain = serde_json::to_vec(&Change { k: "config/safe-senders.txt#".to_string() + value, v: Some(String::new()), b: String::new(), build: String::new(), f: 0 }).unwrap();
            serde_json::to_string(&Line { n, c: clock, s: seal(&key, &bound(&desk.id, 1, n, clock), &plain) }).unwrap() + "\n"
        };
        let last = written(&desk.memory, &desk.id).unwrap().1;
        let mut bytes = std::fs::read(&own).unwrap();
        bytes.extend_from_slice(line(last + 3, "c@example.org").as_bytes());
        std::fs::write(&own, bytes).unwrap();
        let outcome = phone.exchange(&folder, &key, NOW + 4 * MINUTE);
        assert!(outcome.problems.iter().any(|p| p.starts_with("share-other-gap")), "{outcome:?}");
        assert!(!heard(&phone.memory, &phone.id).complete(&desk.id, (1, last + 3)), "past a gap, nothing is known");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn stores_that_leave_arrive_or_move_take_nothing_out() {
        let base = scratch("stores");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let (desk_notes, phone_notes, moved) = (base.join("desk-notes"), base.join("phone-notes"), base.join("moved-notes"));
        let ledger = |notes: &Path| std::fs::read_to_string(notes.join("sioul-budgets.toml")).unwrap_or_default();
        let line = |label: &str, amount: f64| format!("\n[[line]]\nbudget = \"home\"\ndate = \"2026-10-01\"\namount = {amount}\nlabel = \"{label}\"\n");
        std::fs::create_dir_all(&desk_notes).unwrap();
        std::fs::write(desk_notes.join("sioul-budgets.toml"), format!("[[budget]]\nid = \"home\"\ntitle = \"Home\"\n{}", line("Rent", -620.0))).unwrap();
        // The phone holds an old copy of its own, made long ago, and shares without its projects.
        std::fs::create_dir_all(&phone_notes).unwrap();
        std::fs::write(phone_notes.join("sioul-budgets.toml"), format!("[[budget]]\nid = \"home\"\ntitle = \"Old home\"\n{}", line("Old", -1.0))).unwrap();
        let old = std::fs::File::options().write(true).open(phone_notes.join("sioul-budgets.toml")).unwrap();
        old.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)).unwrap();
        desk.exchange_notes(&folder, &key, NOW, &desk_notes);
        phone.exchange(&folder, &key, NOW + MINUTE);
        // Projects shared on the phone from now on: the desk's newer budget wins there, nothing old goes out.
        phone.exchange_notes(&folder, &key, NOW + 2 * MINUTE, &phone_notes);
        desk.exchange_notes(&folder, &key, NOW + 3 * MINUTE, &desk_notes);
        assert!(ledger(&phone_notes).contains("title = \"Home\"") && ledger(&phone_notes).contains("Rent"), "{}", ledger(&phone_notes));
        assert!(ledger(&desk_notes).contains("title = \"Home\""), "{}", ledger(&desk_notes));
        // The phone's notes folder moved to an empty one: filled; nothing taken out on the desk.
        phone.exchange_notes(&folder, &key, NOW + 4 * MINUTE, &moved);
        desk.exchange_notes(&folder, &key, NOW + 5 * MINUTE, &desk_notes);
        assert!(ledger(&moved).contains("Rent"), "{}", ledger(&moved));
        assert!(ledger(&desk_notes).contains("Rent"), "{}", ledger(&desk_notes));
        // Projects no longer shared on the phone: nothing taken out on the desk.
        phone.exchange(&folder, &key, NOW + 6 * MINUTE);
        desk.exchange_notes(&folder, &key, NOW + 7 * MINUTE, &desk_notes);
        assert!(ledger(&desk_notes).contains("Rent"), "{}", ledger(&desk_notes));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_file_emptied_takes_nothing_out_at_once() {
        let base = scratch("emptied");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let settings = "language = \"fr\"\n\n[tasks]\nkind = \"plan\"\n";
        desk.write("config/config.toml", settings);
        desk.exchange(&folder, &key, NOW);
        laptop.exchange(&folder, &key, NOW + MINUTE);
        // Read of no bytes (a crash, a full disk, written in place): held.
        desk.write("config/config.toml", "");
        let outcome = desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert!(outcome.problems.iter().any(|p| p.starts_with("share-emptied")), "{outcome:?}");
        laptop.exchange(&folder, &key, NOW + 3 * MINUTE);
        assert!(laptop.read("config/config.toml").contains("language = \"fr\""));
        // Back before the wait: nothing happened.
        desk.write("config/config.toml", settings);
        assert_eq!(desk.exchange(&folder, &key, NOW + 4 * MINUTE).sent, 0);
        // A list emptied on purpose is taken out at once.
        desk.write("config/safe-senders.txt", "a@example.org\n");
        desk.exchange(&folder, &key, NOW + 5 * MINUTE);
        desk.write("config/safe-senders.txt", "");
        desk.exchange(&folder, &key, NOW + 6 * MINUTE);
        laptop.exchange(&folder, &key, NOW + 7 * MINUTE);
        assert!(!laptop.read("config/safe-senders.txt").contains("a@example.org"));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The file a server is sent at each change stays small (docs/database.md,
    /// "The log"): past `SEGMENT`, a round is continued in a new one, nothing
    /// restated; a full round only past `ROUND_SIZE` appended (or later while
    /// the connection is metered); old rounds go only before the last full
    /// one; a device joining late reads every entry from what is kept.
    #[test]
    fn a_change_sends_a_small_file_and_a_late_joiner_reads_everything() {
        let base = scratch("segments");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        // A desk holding much: three thousand senders, its opening far past a segment.
        let many: String = (0..3000).map(|n| format!("sender{n}@example.org\n")).collect();
        desk.write("config/known-senders.txt", &many);
        desk.exchange(&folder, &key, NOW);
        laptop.exchange(&folder, &key, NOW + 30_000);
        let current = |c: &Computer| {
            let all = rounds(&folder);
            let last = *all.get(&c.id).unwrap().last().unwrap();
            std::fs::metadata(round_file(&folder, &c.id, last)).unwrap().len()
        };
        assert!(current(&desk) > SEGMENT, "the first file holds it all");
        // A sender a minute for an hour: each change goes into a small file.
        let mut list = many.clone();
        for minute in 1..=60 {
            list.push_str(&format!("new{minute}@example.org\n"));
            desk.write("config/known-senders.txt", &list);
            desk.exchange(&folder, &key, NOW + minute * MINUTE);
            assert!(current(&desk) <= SEGMENT + 4096, "minute {minute}: the file sent is {} bytes", current(&desk));
            laptop.exchange(&folder, &key, NOW + minute * MINUTE + 30_000);
        }
        assert!(laptop.read("config/known-senders.txt").contains("new60@example.org"));
        // Past a megabyte appended: a full round, then the rounds before it go once the laptop read past them.
        let more: String = (0..8000).map(|n| format!("later{n}@example.org\n")).collect();
        list.push_str(&more);
        desk.write("config/known-senders.txt", &list);
        desk.exchange(&folder, &key, NOW + 61 * MINUTE);
        let full = Memory::load(&desk.memory, &desk.id).full_round;
        assert!(full > 1, "a full round opened");
        for minute in 62..66 {
            laptop.exchange(&folder, &key, NOW + minute * MINUTE + 30_000);
            list.push_str(&format!("after{minute}@example.org\n"));
            desk.write("config/known-senders.txt", &list);
            desk.exchange(&folder, &key, NOW + minute * MINUTE);
        }
        let kept = rounds(&folder).get(&desk.id).cloned().unwrap();
        assert!(kept[0] >= full && kept.len() > 1, "only the last full round and those continuing it: {kept:?} (full {full})");
        // A device joining now reads everything from what is kept.
        let late = Computer::new(&base, "late");
        late.exchange(&folder, &key, NOW + 70 * MINUTE);
        let known = late.read("config/known-senders.txt");
        for address in ["sender0@", "sender2999@", "new1@", "new60@", "later7999@", "after65@"] {
            assert!(known.contains(address), "{address} missing for the late joiner");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// While the connection is metered or slow, a full round waits, within
    /// `FRUGAL_ROUND_SIZE`; then it opens all the same.
    #[test]
    fn a_full_round_waits_while_the_connection_is_metered() {
        let base = scratch("frugal");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        let memory_full = || Memory::load(&desk.memory, &desk.id).full_round;
        let lines = |from: usize, count: usize| -> String { (from..from + count).map(|n| format!("s{n}@example.org\n")).collect() };
        set_frugal(&desk.memory, true);
        let mut list = lines(0, 6000);
        desk.write("config/known-senders.txt", &list);
        desk.exchange(&folder, &key, NOW);
        list.push_str(&lines(6000, 7000));
        desk.write("config/known-senders.txt", &list);
        desk.exchange(&folder, &key, NOW + MINUTE);
        let waited = memory_full();
        assert!(Memory::load(&desk.memory, &desk.id).grown > ROUND_SIZE && waited <= 1, "past a megabyte, metered: no full round yet ({waited})");
        set_frugal(&desk.memory, false);
        list.push_str(&lines(13000, 10));
        desk.write("config/known-senders.txt", &list);
        desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert!(memory_full() > 1, "not metered any more: the full round opens");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_big_round_does_not_start_another_each_minute() {
        let base = scratch("big");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        desk.write("data/drafts/big.toml", &format!("body = \"{}\"\n", "x".repeat(ROUND_SIZE as usize + 1000)));
        desk.exchange(&folder, &key, NOW);
        desk.write("config/safe-senders.txt", "a@example.org\n");
        desk.exchange(&folder, &key, NOW + MINUTE);
        let rounds_then = rounds(&folder).get(&desk.id).map_or(0, |r| *r.last().unwrap());
        for minute in 2..6 {
            desk.write("config/safe-senders.txt", &format!("a@example.org\nn{minute}@example.org\n"));
            desk.exchange(&folder, &key, NOW + minute * MINUTE);
        }
        assert_eq!(rounds(&folder).get(&desk.id).map_or(0, |r| *r.last().unwrap()), rounds_then, "small changes after a big round stay in it");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_half_written_line_is_cut_before_the_next() {
        let base = scratch("half");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        desk.write("config/safe-senders.txt", "a@example.org\n");
        desk.exchange(&folder, &key, NOW);
        let own = round_file(&folder, &desk.id, 1);
        let mut text = std::fs::read_to_string(&own).unwrap();
        text.push_str("{\"n\":99,\"c\":1,\"s\":\"half");
        std::fs::write(&own, text).unwrap();
        desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
        desk.exchange(&folder, &key, NOW + MINUTE);
        assert!(std::fs::read_to_string(&own).unwrap().lines().all(|l| l.ends_with('}')), "whole lines only");
        laptop.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert!(laptop.read("config/safe-senders.txt").contains("b@example.org"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_days_reviews_travel_whole() {
        use sioul_core::reviews::{Felt, Mix, Reviews};
        let base = scratch("reviews");
        let folder = base.join("Nextcloud").join("sioul-shared");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let at = |ms: i64| std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64);
        let month = "data/reviews/2026-10.toml";
        // The morning's weather and the end of work, on the desk.
        desk.write(month, "[2026-10-06]\nweather = \"haze\"\n\n[2026-10-06.work]\nat = \"2026-10-06T17:04:00+02:00\"\nfelt = \"heavy\"\nmix = \"too_much\"\n");
        dated(&desk.path(month), at(NOW - 10 * MINUTE));
        desk.exchange(&folder, &key, NOW);
        phone.exchange(&folder, &key, NOW + MINUTE);
        assert!(phone.read(month).contains("felt = \"heavy\""));
        // The same work review answered again on both: its feeling on the desk,
        // then its mix on the phone, with the night.
        desk.write(month, &desk.read(month).replace("felt = \"heavy\"", "felt = \"usual\""));
        dated(&desk.path(month), at(NOW + 90_000));
        phone.write(month, &format!("{}\n[2026-10-06.night]\nat = \"2026-10-06T22:10:00+02:00\"\nmix = \"about_right\"\nmemo = \"\"\"\nA walk.\n\"\"\"\n", phone.read(month).replace("mix = \"too_much\"", "mix = \"about_right\"")));
        dated(&phone.path(month), at(NOW + 100_000));
        phone.exchange(&folder, &key, NOW + 2 * MINUTE);
        desk.exchange(&folder, &key, NOW + 3 * MINUTE);
        phone.exchange(&folder, &key, NOW + 4 * MINUTE);
        for computer in [&desk, &phone] {
            let text = computer.read(month);
            let reviews = Reviews::load(&computer.path("data/reviews"));
            let day = reviews.day("2026-10-06".parse().unwrap()).unwrap();
            assert_eq!(day.weather, Some(sioul_core::today::Weather::Haze), "{text}");
            // The later answer whole: never the desk's feeling with the phone's mix.
            assert_eq!(day.work.as_ref().map(|r| (r.felt, r.mix)), Some((Some(Felt::Heavy), Some(Mix::AboutRight))), "{text}");
            assert_eq!(day.night.as_ref().map(|r| (r.mix, r.memo.trim().to_string())), Some((Some(Mix::AboutRight), "A walk.".to_string())), "{text}");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn two_computers_agree() {
        let base = scratch("agree");
        let folder = base.join("Nextcloud").join("sioul-shared");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));

        // The desk shares first: its settings, its lists, a draft, a session.
        desk.write("config/config.toml", "language = \"fr\"\ncase_store = \"~/Notes\"\n\n[tasks]\nkind = \"plan\"\n\n[[account]]\nid = \"perso\"\nkind = \"imap\"\nmaildir = \"/desk/mail\"\n");
        desk.write("config/blocked-senders.txt", "spam@example.org\n");
        desk.write("data/drafts/a1.toml", "subject = \"Hello\"\n");
        desk.write("data/time/2026-10.toml", "[[session]]\ntask = \"t1\"\nstart = 100\nminutes = 25\n");
        assert_eq!(desk.exchange(&folder, &key, NOW).received, 0);

        // The laptop joins with its own: its notes folder stays, its blocked sender joins the desk's.
        laptop.write("config/config.toml", "language = \"en\"\ncase_store = \"~/Nextcloud/Notes\"\n");
        laptop.write("config/blocked-senders.txt", "*@pushy.example.com\n");
        let outcome = laptop.exchange(&folder, &key, NOW + MINUTE);
        assert!(outcome.received > 0 && outcome.sent > 0, "{outcome:?}");
        let config = laptop.read("config/config.toml");
        assert!(config.contains("language = \"fr\""), "the shared settings win when joining: {config}");
        assert!(config.contains("case_store = \"~/Nextcloud/Notes\""), "where things are stays here: {config}");
        assert!(config.contains("id = \"perso\"") && !config.contains("/desk/mail"), "accounts come, without this computer's paths: {config}");
        assert_eq!(laptop.read("data/drafts/a1.toml"), "subject = \"Hello\"\n");
        assert!(laptop.read("data/time/2026-10.toml").contains("task = \"t1\""));
        let blocked = laptop.read("config/blocked-senders.txt");
        assert!(blocked.contains("spam@example.org") && blocked.contains("*@pushy.example.com"), "{blocked}");

        // The desk hears the laptop's sender.
        desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert!(desk.read("config/blocked-senders.txt").contains("*@pushy.example.com"));

        // Both work apart: a session each in the same month, a setting each, a draft sent on the laptop.
        desk.write("data/time/2026-10.toml", "[[session]]\ntask = \"t1\"\nstart = 100\nminutes = 25\n\n[[session]]\ntask = \"t2\"\nstart = 200\nminutes = 50\n");
        desk.write("config/config.toml", &desk.read("config/config.toml").replace("kind = \"plan\"", "kind = \"list\""));
        laptop.write("data/time/2026-10.toml", &format!("{}\n[[session]]\nproject = \"p\"\nstart = 300\nminutes = 10\n", laptop.read("data/time/2026-10.toml")));
        let config = laptop.read("config/config.toml").replace("language = \"fr\"", "language = \"de\"");
        laptop.write("config/config.toml", &config);
        std::fs::remove_file(laptop.path("data/drafts/a1.toml")).unwrap();
        desk.exchange(&folder, &key, NOW + 3 * MINUTE);
        laptop.exchange(&folder, &key, NOW + 4 * MINUTE);
        desk.exchange(&folder, &key, NOW + 5 * MINUTE);
        for computer in [&desk, &laptop] {
            let time = computer.read("data/time/2026-10.toml");
            assert!(time.contains("\"t1\"") && time.contains("\"t2\"") && time.contains("project = \"p\""), "{time}");
            let config = computer.read("config/config.toml");
            assert!(config.contains("language = \"de\"") && config.contains("kind = \"list\""), "{config}");
            assert!(!computer.path("data/drafts/a1.toml").exists(), "a draft sent on one is gone on both");
        }
        assert!(desk.read("config/config.toml").contains("maildir = \"/desk/mail\""), "the desk keeps its own path");

        // Nothing changed: nothing goes out.
        assert_eq!(desk.exchange(&folder, &key, NOW + 6 * MINUTE).sent, 0);
        assert_eq!(laptop.exchange(&folder, &key, NOW + 7 * MINUTE).sent, 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn projects_and_budgets_travel_when_asked() {
        let base = scratch("projects");
        let folder = base.join("Nextcloud").join("Documents").join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
        let projects = |notes: &Path| std::fs::read_to_string(sioul_core::projects::file_in(notes)).unwrap_or_default();
        let ledger = |notes: &Path| std::fs::read_to_string(notes.join("sioul-budgets.toml")).unwrap_or_default();
        let bank = |notes: &Path| std::fs::read_to_string(notes.join("sioul-bank.toml")).unwrap_or_default();
        let movement = |id: &str, label: &str| format!("\n[[movement]]\naccount = \"main\"\ndate = 2026-10-01\namount = -20.0\nlabel = \"{label}\"\nid = \"{id}\"\n");
        std::fs::create_dir_all(&desk_notes).unwrap();
        std::fs::write(desk_notes.join("sioul-cases.toml"), "[[case]]\nid = \"acme\"\ntitle = \"Acme site\"\nstatus = \"open\"\n\n[[case.route]]\nfrom_domains = [\"acme.example\"]\n").unwrap();
        std::fs::write(desk_notes.join("sioul-budgets.toml"), "[[budget]]\nid = \"home\"\ntitle = \"Home\"\n\n[[line]]\nbudget = \"home\"\ndate = \"2026-10-01\"\namount = -620.0\nlabel = \"Rent\"\n").unwrap();
        std::fs::write(desk_notes.join("sioul-bank.toml"), format!("[[account]]\nid = \"main\"\ntitle = \"Main\"\nbalance = 1200.0\nas_of = 2026-10-01\n{}", movement("fitid-1", "Rent"))).unwrap();
        desk.exchange_notes(&folder, &key, NOW, &desk_notes);

        // The phone, with a notes folder of its own: the project, the budget and the bank come.
        phone.exchange_notes(&folder, &key, NOW + MINUTE, &phone_notes);
        assert!(projects(&phone_notes).contains("id = \"acme\"") && projects(&phone_notes).contains("acme.example"), "{}", projects(&phone_notes));
        assert!(ledger(&phone_notes).contains("Rent"), "{}", ledger(&phone_notes));
        assert!(bank(&phone_notes).contains("fitid-1") && bank(&phone_notes).contains("1200"), "{}", bank(&phone_notes));
        // The phone had no projects' file: it makes one of the new name, the desk keeps its own.
        assert!(phone_notes.join(sioul_core::projects::FILE).exists() && projects(&phone_notes).contains("[[project]]") && !projects(&phone_notes).contains("[[case"), "{}", projects(&phone_notes));
        assert!(projects(&desk_notes).contains("[[case]]") && !desk_notes.join(sioul_core::projects::FILE).exists());

        // A line each, apart: both kept on both.
        std::fs::write(desk_notes.join("sioul-budgets.toml"), format!("{}\n[[line]]\nbudget = \"home\"\ndate = \"2026-10-02\"\namount = -12.5\nlabel = \"Bread\"\n", ledger(&desk_notes))).unwrap();
        std::fs::write(phone_notes.join("sioul-budgets.toml"), format!("{}\n[[line]]\nbudget = \"home\"\ndate = \"2026-10-02\"\namount = -3.2\nlabel = \"Coffee\"\n", ledger(&phone_notes))).unwrap();
        // An export taken in on each: both movements on both.
        std::fs::write(desk_notes.join("sioul-bank.toml"), format!("{}{}", bank(&desk_notes), movement("fitid-2", "Bread"))).unwrap();
        std::fs::write(phone_notes.join("sioul-bank.toml"), format!("{}{}", bank(&phone_notes), movement("fitid-3", "Coffee"))).unwrap();
        desk.exchange_notes(&folder, &key, NOW + 2 * MINUTE, &desk_notes);
        phone.exchange_notes(&folder, &key, NOW + 3 * MINUTE, &phone_notes);
        desk.exchange_notes(&folder, &key, NOW + 4 * MINUTE, &desk_notes);
        for notes in [&desk_notes, &phone_notes] {
            let text = ledger(notes);
            assert!(text.contains("Rent") && text.contains("Bread") && text.contains("Coffee"), "{text}");
            let mut ids: Vec<String> = sioul_core::bank::Bank::load(notes).unwrap().movements.into_iter().map(|m| m.id).collect();
            ids.sort();
            assert_eq!(ids, ["fitid-1", "fitid-2", "fitid-3"], "{}", bank(notes));
        }

        // Each file keeps its names: a project made on the phone reaches the desk's
        // `[[case]]`, as an older Sioul reads it; renamed on the desk, the file
        // still exchanges under the same records, both ways.
        use sioul_core::projects::{ProjectEdit, ProjectStore, file_in, rename_file, save_project, set_ai};
        save_project(&file_in(&phone_notes), "", &ProjectEdit { title: "Garden".into(), ..ProjectEdit::default() }).unwrap();
        phone.exchange_notes(&folder, &key, NOW + 5 * MINUTE, &phone_notes);
        desk.exchange_notes(&folder, &key, NOW + 6 * MINUTE, &desk_notes);
        assert!(projects(&desk_notes).matches("[[case]]").count() == 2 && !projects(&desk_notes).contains("[[project"), "{}", projects(&desk_notes));
        // Added after a project with a route: each route stays under its own project.
        let desk_store = ProjectStore::load(&desk_notes).unwrap();
        assert!(desk_store.get("acme").unwrap().routes.len() == 1 && desk_store.get("garden").unwrap().routes.is_empty(), "{}", projects(&desk_notes));
        rename_file(&desk_notes).unwrap();
        set_ai(&file_in(&desk_notes), "acme", true).unwrap();
        desk.exchange_notes(&folder, &key, NOW + 7 * MINUTE, &desk_notes);
        phone.exchange_notes(&folder, &key, NOW + 8 * MINUTE, &phone_notes);
        desk.exchange_notes(&folder, &key, NOW + 9 * MINUTE, &desk_notes);
        for notes in [&desk_notes, &phone_notes] {
            let store = ProjectStore::load(notes).unwrap();
            let mut ids: Vec<&str> = store.projects.iter().map(|p| p.id.as_str()).collect();
            ids.sort();
            assert_eq!(ids, ["acme", "garden"], "{}", projects(notes));
            assert!(store.get("acme").unwrap().ai && store.get("acme").unwrap().routes.len() == 1 && store.get("garden").unwrap().routes.is_empty(), "{}", projects(notes));
            assert!(!projects(notes).contains("[[case"), "{}", projects(notes));
        }
        assert!(desk_notes.join(sioul_core::projects::BEFORE_RENAME).exists());

        // Without the choice, a notes folder stays home.
        let laptop = Computer::new(&base, "laptop");
        // A literal string: a Windows path's backslashes are no escapes.
        laptop.write("config/config.toml", &format!("case_store = '{}'\n", base.join("laptop-notes").display()));
        let config: Config = toml::from_str(&laptop.read("config/config.toml")).unwrap();
        assert!(stores(&config, &laptop.roots).iter().all(|s| !s.name.starts_with("notes/")));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn what_was_read_is_known_and_a_lost_record_comes_back() {
        let base = scratch("doses");
        let folder = base.join("shared");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        desk.write("state/health-state.toml", "[taken]\n\"d@100\" = 110\n");
        desk.exchange(&folder, &key, NOW);
        phone.exchange(&folder, &key, NOW + MINUTE);
        assert!(phone.read("state/health-state.toml").contains("d@100"));
        // What the desk wrote, the phone read: as far as the desk's claim would say.
        let wrote = written(&desk.memory, &desk.id).unwrap();
        assert!(wrote.1 > 0);
        let read = heard(&phone.memory, &phone.id);
        assert!(read.complete(&desk.id, wrote));
        assert!(!read.complete(&desk.id, (wrote.0, wrote.1 + 1)), "a record not read yet is not known");
        assert!(read.complete("someone", (1, 0)), "nothing written, nothing to read");

        // A line of the desk's that does not read is said, never passed over; what follows is still read.
        desk.write("state/health-state.toml", "[taken]\n\"d@100\" = 110\n\"d@200\" = 210\n");
        desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        let round = round_file(&folder, &desk.id, wrote.0);
        let mut text = std::fs::read_to_string(&round).unwrap();
        let last = text.trim_end().rfind('\n').map_or(0, |i| i + 1);
        text.insert_str(last, "{broken\n");
        std::fs::write(&round, text).unwrap();
        let outcome = phone.exchange(&folder, &key, NOW + 3 * MINUTE);
        assert!(outcome.problems.iter().any(|p| p == &format!("share-other-line:{}", desk.id)), "{:?}", outcome.problems);
        assert!(heard(&phone.memory, &phone.id).broken.contains_key(&desk.id));
        assert!(phone.read("state/health-state.toml").contains("d@200"), "the next line came");

        // The phone's record is lost: set aside, started again empty, rebuilt from the records.
        phone.write("state/health-state.toml", &format!("{}\"d@300\" = 310\n", phone.read("state/health-state.toml")));
        phone.exchange(&folder, &key, NOW + 4 * MINUTE);
        assert!(desk.exchange(&folder, &key, NOW + 5 * MINUTE).received > 0);
        assert!(desk.read("state/health-state.toml").contains("d@300"));
        std::fs::remove_file(phone.path("state/health-state.toml")).unwrap();
        rebuild(&phone.memory, &phone.id, &["state/health-state.toml"]).unwrap();
        // A dose marked meanwhile, in a new record holding only it.
        phone.write("state/health-state.toml", "[taken]\n\"d@400\" = 410\n");
        phone.exchange(&folder, &key, NOW + 6 * MINUTE);
        let back = phone.read("state/health-state.toml");
        assert!(back.contains("d@100") && back.contains("d@200") && back.contains("d@300") && back.contains("d@400"), "{back}");
        // Nothing taken out on the desk: every mark is still there, the new one too.
        desk.exchange(&folder, &key, NOW + 7 * MINUTE);
        let there = desk.read("state/health-state.toml");
        assert!(there.contains("d@100") && there.contains("d@200") && there.contains("d@300") && there.contains("d@400"), "{there}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_later_word_wins() {
        let base = scratch("later");
        let folder = base.join("shared");
        let key = quick_key(&folder, "a passphrase of some length").unwrap();
        let (a, b) = (Computer::new(&base, "a"), Computer::new(&base, "b"));
        a.write("config/safe-senders.txt", "friend@example.org\n");
        a.exchange(&folder, &key, NOW);
        b.exchange(&folder, &key, NOW + MINUTE);
        assert!(b.read("config/safe-senders.txt").contains("friend@example.org"));

        // Safe on one, then blocked later on the other: blocked everywhere, and safe nowhere.
        b.write("config/safe-senders.txt", "");
        b.write("config/blocked-senders.txt", "friend@example.org\n");
        b.exchange(&folder, &key, NOW + 2 * MINUTE);
        a.exchange(&folder, &key, NOW + 3 * MINUTE);
        assert!(!a.read("config/safe-senders.txt").contains("friend@example.org"));
        assert!(a.read("config/blocked-senders.txt").contains("friend@example.org"));

        // A list that cannot be read is left alone, and nothing of it is taken out elsewhere.
        a.write("state/quiet.toml", "work_until = 5\n");
        let sent = a.exchange(&folder, &key, NOW + 4 * MINUTE);
        let received = b.exchange(&folder, &key, NOW + 5 * MINUTE);
        assert!(b.read("state/quiet.toml").contains("work_until = 5"), "{sent:?} {received:?}");
        b.write("state/quiet.toml", "work_until = [ broken");
        b.exchange(&folder, &key, NOW + 6 * MINUTE);
        a.exchange(&folder, &key, NOW + 7 * MINUTE);
        assert!(a.read("state/quiet.toml").contains("work_until = 5"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sealed_with_one_passphrase() {
        let base = scratch("seal");
        let folder = base.join("shared");
        let key = quick_key(&folder, "the right passphrase").unwrap();
        assert_eq!(quick_key(&folder, "the right passphrase"), Ok(key));
        assert_eq!(quick_key(&folder, "another passphrase"), Err(Refused::WrongPassphrase));

        // What travels says nothing of what it holds.
        let a = Computer::new(&base, "a");
        a.write("config/safe-senders.txt", "friend@example.org\n");
        a.exchange(&folder, &key, NOW);
        let file = std::fs::read_to_string(round_file(&folder, &a.id, 1)).unwrap();
        assert!(!file.contains("friend") && !file.contains("safe-senders"), "{file}");
        assert_eq!(others(&folder, "someone else").len(), 1);
        // A seal asking more than this computer can give is refused before it is tried.
        let text = std::fs::read_to_string(folder.join("seal.toml")).unwrap();
        std::fs::write(folder.join("seal.toml"), text.replace("memory_kib = 64", "memory_kib = 4294967295")).unwrap();
        assert!(matches!(quick_key(&folder, "the right passphrase"), Err(Refused::Other(_))));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn records_stay_in_their_folders() {
        let roots = Roots { config: PathBuf::from("config"), data: PathBuf::from("data"), state: PathBuf::from("state") };
        let stores = stores(&Config::default(), &roots);
        assert!(locate(&stores, "data/drafts/a1.toml").is_some());
        for file in ["data/drafts/../x", "data/drafts/", "data/drafts//x", "data/drafts/a\\..\\..\\x", "data/drafts/C:x"] {
            assert!(locate(&stores, file).is_none(), "{file}");
        }
    }

    #[test]
    fn rounds_start_over_and_old_ones_go() {
        let base = scratch("rounds");
        let folder = base.join("shared");
        let key = quick_key(&folder, "a passphrase of some length").unwrap();
        let (a, b) = (Computer::new(&base, "a"), Computer::new(&base, "b"));
        a.write("config/safe-senders.txt", "friend@example.org\n");
        a.exchange(&folder, &key, NOW);
        b.exchange(&folder, &key, NOW + MINUTE);
        // A large file, changed until a full round opens (a megabyte appended), the rounds before it continued in between.
        let mut now = NOW + MINUTE;
        while Memory::load(&a.memory, &a.id).full_round <= 1 {
            now += MINUTE;
            a.write("data/drafts/2026-10-01.json", &format!("{{\"steps\": {now}, \"pad\": \"{}\"}}", "x".repeat(100_000)));
            a.exchange(&folder, &key, now);
        }
        assert!(round_file(&folder, &a.id, 1).exists(), "kept until b has read past it");
        // b reads past round 1.
        b.exchange(&folder, &key, now + MINUTE);
        assert!(b.read("data/drafts/2026-10-01.json").contains(&format!("\"steps\": {now}")));
        a.exchange(&folder, &key, now + 2 * MINUTE);
        assert!(!round_file(&folder, &a.id, 1).exists(), "read by everyone: removed");
        // A third computer joins later: round 2 alone holds the day.
        let c = Computer::new(&base, "c");
        c.exchange(&folder, &key, now + 3 * MINUTE);
        assert!(c.read("data/drafts/2026-10-01.json").contains(&format!("\"steps\": {now}")));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn notes_travel_file_by_file_whatever_carries_them() {
        for how in [Carrying::Exact, Carrying::SizeOnly, Carrying::Late, Carrying::StaleCopies, Carrying::Placeholders] {
            let base = scratch(&format!("notes-{how:?}").to_lowercase());
            let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
            let key = quick_key(&server, "four words make a passphrase").unwrap();
            let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
            let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
            put(&desk_notes, "lease.md", b"# Lease\nSigned on 2 October.\n");
            put(&desk_notes, "admin/taxes.md", b"# Taxes\nNotice received.\n");
            put(&desk_notes, "admin/scan.pdf", &noise(1_500_000, 1));
            // Made by tools, or carried by another part: they stay.
            for name in ["lease.md~", ".lease.md.swp", "lease (conflicted copy 2026-10-05 140533).md", "taxes.sync-conflict-20261005-140533-ABCDEFG.md", "~$letter.docx", "sioul-cases.toml", "sioul-projects.toml", "sioul-cases.toml.before-rename", "papers/passport.pdf"] {
                put(&desk_notes, name, b"stays home");
            }
            let mut carrier = Carrier::new(how);
            let run = |carrier: &mut Carrier, minutes: std::ops::Range<i64>| {
                for minute in minutes {
                    let now = NOW + minute * MINUTE;
                    desk.exchange_with(&server, &key, now, &desk_notes, &["notes"]);
                    carrier.carry(&server, &phone_folder);
                    phone.exchange_with(&phone_folder, &key, now + 30_000, &phone_notes, &["notes"]);
                }
            };
            run(&mut carrier, 0..20);
            let there = files_in(&phone_notes);
            assert_eq!(there.keys().map(String::as_str).collect::<Vec<_>>(), ["admin/scan.pdf", "admin/taxes.md", "lease.md"], "{how:?}");
            assert!(there["admin/scan.pdf"] == noise(1_500_000, 1), "{how:?}: the scan came whole");
            // The phone changes a note and writes another; the desk takes one out.
            put(&phone_notes, "lease.md", b"# Lease\nSigned on 2 October.\nKeys: two.\n");
            put(&phone_notes, "new.md", b"# New\n");
            std::fs::remove_file(desk_notes.join("admin/taxes.md")).unwrap();
            run(&mut carrier, 20..40);
            for notes in [&desk_notes, &phone_notes] {
                let files = files_in(notes);
                assert!(!files.contains_key("admin/taxes.md"), "{how:?}: {}", notes.display());
                assert_eq!(files.get("lease.md").map(Vec::as_slice), Some(&b"# Lease\nSigned on 2 October.\nKeys: two.\n"[..]), "{how:?}: {}", notes.display());
                assert_eq!(files.get("new.md").map(Vec::as_slice), Some(&b"# New\n"[..]), "{how:?}: {}", notes.display());
                assert!(copies_of(notes, "lease").is_empty(), "{how:?}: no conflict where there was none");
            }
            // Each content sealed once, apart; nothing of the notes readable in the folder.
            let sealed = std::fs::read_dir(server.join("blobs")).unwrap().filter_map(Result::ok).filter(|e| !e.file_name().to_string_lossy().starts_with('.')).count();
            assert_eq!(sealed, 5, "{how:?}: the lease twice, the taxes, the scan, the new note");
            let mut everything = Vec::new();
            list_files(&server, &server, &[], &mut everything);
            for (_, path) in everything {
                let bytes = std::fs::read(&path).unwrap();
                for secret in [&b"Signed on"[..], b"Taxes", b"Keys: two", b"stays home", b"lease.md"] {
                    assert!(!bytes.windows(secret.len()).any(|w| w == secret), "{how:?}: {} shows {}", path.display(), String::from_utf8_lossy(secret));
                }
            }
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn a_note_changed_on_both_keeps_both_whatever_carries_them() {
        for how in [Carrying::Exact, Carrying::SizeOnly, Carrying::Late, Carrying::StaleCopies, Carrying::Placeholders] {
            let base = scratch(&format!("notes-both-{how:?}").to_lowercase());
            let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
            let key = quick_key(&server, "four words make a passphrase").unwrap();
            let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
            let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
            put(&desk_notes, "lease.md", b"Rent: 620\n");
            put(&desk_notes, "taxes.md", b"Notice received.\n");
            let mut carrier = Carrier::new(how);
            let run = |carrier: &mut Carrier, minutes: std::ops::Range<i64>| {
                for minute in minutes {
                    let now = NOW + minute * MINUTE;
                    desk.exchange_with(&server, &key, now, &desk_notes, &["notes"]);
                    carrier.carry(&server, &phone_folder);
                    phone.exchange_with(&phone_folder, &key, now + 30_000, &phone_notes, &["notes"]);
                }
            };
            run(&mut carrier, 0..10);
            assert_eq!(files_in(&phone_notes).len(), 2, "{how:?}");
            // In the same minute: the lease changed on both; the taxes taken out on the desk, changed on the phone.
            put(&desk_notes, "lease.md", b"Rent: 640\n");
            put(&phone_notes, "lease.md", b"Rent: 620\nKeys: two\n");
            std::fs::remove_file(desk_notes.join("taxes.md")).unwrap();
            put(&phone_notes, "taxes.md", b"Notice received.\nPaid on 3 October.\n");
            run(&mut carrier, 10..40);
            for notes in [&desk_notes, &phone_notes] {
                let files = files_in(notes);
                assert_eq!(files.get("lease.md").map(Vec::as_slice), Some(&b"Rent: 620\nKeys: two\n"[..]), "{how:?}: {}", notes.display());
                assert_eq!(copies_of(notes, "lease"), [b"Rent: 640\n".to_vec()], "{how:?}: {}: {:?}", notes.display(), files.keys().collect::<Vec<_>>());
                assert_eq!(files.get("taxes.md").map(Vec::as_slice), Some(&b"Notice received.\nPaid on 3 October.\n"[..]), "{how:?}: {}", notes.display());
                assert_eq!(files.len(), 3, "{how:?}: {}: {:?}", notes.display(), files.keys().collect::<Vec<_>>());
            }
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn a_big_folder_sends_only_what_changed() {
        let base = scratch("big-folder");
        let notes = base.join("notes");
        // Shared through a folder inside the notes folder, as Sioul may suggest: not carried as notes.
        let folder = notes.join("sioul-shared");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        for n in 0..300 {
            put(&notes, &format!("days/{n:03}.md"), format!("# Day {n}\n").as_bytes());
        }
        for n in 0..3 {
            put(&notes, &format!("scans/{n}.pdf"), &noise(1_500_000, n));
        }
        // Read after they changed, not in the same millisecond (such a file is read again, its time too coarse to trust).
        std::thread::sleep(std::time::Duration::from_millis(5));
        let blobs = || std::fs::read_dir(folder.join("blobs")).unwrap().count();
        assert_eq!(desk.exchange_with(&folder, &key, NOW, &notes, &["notes"]).sent, 303);
        assert_eq!(blobs(), 303);
        // Nothing changed: no file read again, nothing sent.
        crate::blobs::HASHED.with(|n| n.set(0));
        assert_eq!(desk.exchange_with(&folder, &key, NOW + MINUTE, &notes, &["notes"]).sent, 0);
        assert_eq!(crate::blobs::HASHED.with(std::cell::Cell::get), 0, "a file of the same size and time is not read");
        // One file touched (its time, not its content), one changed: both read, one sent, sealed alone.
        std::fs::File::options().write(true).open(notes.join("days/007.md")).unwrap().set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(60)).unwrap();
        put(&notes, "scans/1.pdf", &noise(1_500_000, 9));
        let outcome = desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &notes, &["notes"]);
        assert_eq!(outcome.sent, 1, "{outcome:?}");
        assert_eq!(crate::blobs::HASHED.with(std::cell::Cell::get), 2);
        assert_eq!(blobs(), 304);
        // Too big to travel: it stays, said; nothing else moves.
        std::fs::File::create(notes.join("scans/film.mp4")).unwrap().set_len(crate::blobs::LARGEST + 1).unwrap();
        let outcome = desk.exchange_with(&folder, &key, NOW + 3 * MINUTE, &notes, &["notes"]);
        assert!(outcome.problems.iter().any(|p| p == "share-too-big:files/notes/scans/film.mp4"), "{outcome:?}");
        assert_eq!(outcome.sent, 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_note_changed_on_both_keeps_both() {
        for desk_later in [false, true] {
            let base = scratch(&format!("both-{desk_later}"));
            let folder = base.join("Sioul");
            let key = quick_key(&folder, "four words make a passphrase").unwrap();
            let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
            let (desk_notes, laptop_notes) = (base.join("desk-notes"), base.join("laptop-notes"));
            put(&desk_notes, "lease.md", b"Rent: 620\n");
            desk.exchange_with(&folder, &key, NOW, &desk_notes, &["notes"]);
            laptop.exchange_with(&folder, &key, NOW + MINUTE, &laptop_notes, &["notes"]);
            assert_eq!(std::fs::read(laptop_notes.join("lease.md")).unwrap(), b"Rent: 620\n");
            // Changed on both, apart; the one that exchanges later is the later word.
            put(&desk_notes, "lease.md", b"Rent: 640\n");
            put(&laptop_notes, "lease.md", b"Rent: 620\nKeys: two\n");
            let (first, second): (&Computer, &Computer) = if desk_later { (&laptop, &desk) } else { (&desk, &laptop) };
            let notes = |c: &Computer| if std::ptr::eq(c, &desk) { desk_notes.clone() } else { laptop_notes.clone() };
            let mut said = Vec::new();
            for minute in 2..10 {
                let (who, at) = if minute % 2 == 0 { (first, minute) } else { (second, minute) };
                said.extend(who.exchange_with(&folder, &key, NOW + at * MINUTE, &notes(who), &["notes"]).problems);
            }
            let (later, earlier): (&[u8], &[u8]) = if desk_later { (b"Rent: 640\n", b"Rent: 620\nKeys: two\n") } else { (b"Rent: 620\nKeys: two\n", b"Rent: 640\n") };
            for notes in [&desk_notes, &laptop_notes] {
                assert_eq!(std::fs::read(notes.join("lease.md")).unwrap(), later, "desk later: {desk_later}");
                assert_eq!(copies_of(notes, "lease"), [earlier.to_vec()], "desk later: {desk_later}: {:?}", files_in(notes).keys().collect::<Vec<_>>());
            }
            assert!(said.iter().any(|p| p.starts_with("share-conflict:lease (")), "{said:?}");
            // Settled: nothing more goes either way.
            assert_eq!(desk.exchange_with(&folder, &key, NOW + 10 * MINUTE, &desk_notes, &["notes"]).sent, 0);
            assert_eq!(laptop.exchange_with(&folder, &key, NOW + 11 * MINUTE, &laptop_notes, &["notes"]).sent, 0);
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn a_part_switched_off_takes_nothing_out_and_sends_nothing() {
        let base = scratch("part-off");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
        for name in ["a", "b", "c", "d", "e", "f"] {
            put(&desk_notes, &format!("{name}.md"), format!("# {name}\n").as_bytes());
        }
        desk.exchange_with(&folder, &key, NOW, &desk_notes, &["notes"]);
        phone.exchange_with(&folder, &key, NOW + MINUTE, &phone_notes, &["notes"]);
        assert_eq!(files_in(&phone_notes).len(), 6);
        // Notes switched off on the phone, then changed there; the desk changes one meanwhile.
        phone.exchange_with(&folder, &key, NOW + 2 * MINUTE, &phone_notes, &[]);
        std::fs::remove_file(phone_notes.join("a.md")).unwrap();
        put(&phone_notes, "b.md", b"# b, changed on the phone while off\n");
        put(&phone_notes, "new.md", b"# new\n");
        put(&desk_notes, "c.md", b"# c, changed on the desk\n");
        desk.exchange_with(&folder, &key, NOW + 3 * MINUTE, &desk_notes, &["notes"]);
        assert_eq!(phone.exchange_with(&folder, &key, NOW + 4 * MINUTE, &phone_notes, &[]).sent, 0, "a part switched off sends nothing");
        desk.exchange_with(&folder, &key, NOW + 5 * MINUTE, &desk_notes, &["notes"]);
        let desk_files = files_in(&desk_notes);
        assert!(desk_files.contains_key("a.md") && desk_files["b.md"] == b"# b\n" && !desk_files.contains_key("new.md"), "nothing taken out, nothing came: {desk_files:?}");
        assert_eq!(std::fs::read(phone_notes.join("c.md")).unwrap(), b"# c\n", "nothing comes into a part switched off");
        // Switched on again: it joins as a new device. What the others hold comes
        // first, a note changed here meanwhile is kept beside, what only it holds goes.
        phone.exchange_with(&folder, &key, NOW + 6 * MINUTE, &phone_notes, &["notes"]);
        desk.exchange_with(&folder, &key, NOW + 7 * MINUTE, &desk_notes, &["notes"]);
        phone.exchange_with(&folder, &key, NOW + 8 * MINUTE, &phone_notes, &["notes"]);
        desk.exchange_with(&folder, &key, NOW + 9 * MINUTE, &desk_notes, &["notes"]);
        for notes in [&desk_notes, &phone_notes] {
            let files = files_in(notes);
            assert_eq!(files.get("a.md").map(Vec::as_slice), Some(&b"# a\n"[..]), "{}", notes.display());
            assert_eq!(files.get("b.md").map(Vec::as_slice), Some(&b"# b\n"[..]), "{}", notes.display());
            assert_eq!(files.get("c.md").map(Vec::as_slice), Some(&b"# c, changed on the desk\n"[..]), "{}", notes.display());
            assert_eq!(files.get("new.md").map(Vec::as_slice), Some(&b"# new\n"[..]), "{}", notes.display());
            assert_eq!(copies_of(notes, "b"), [b"# b, changed on the phone while off\n".to_vec()], "{}", notes.display());
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Item 8 of 9 October: on Unix, what the sharing writes is yours alone,
    /// as the calls' logs are: its files 0600, the folders it makes 0700 (the
    /// seal, the records, the sealed files, the notes and settings it brings
    /// in, the copies and versions it keeps). A file here already readable by
    /// others becomes yours alone the next time the sharing writes it; one it
    /// never writes keeps its mode: nothing sweeps your folders.
    #[cfg(unix)]
    #[test]
    fn what_the_sharing_writes_is_yours_alone() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777;
        let set = |path: &Path, mode: u32| std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        let base = scratch("yours-alone");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (desk_notes, laptop_notes) = (base.join("desk-notes"), base.join("laptop-notes"));
        put(&desk_notes, "admin/lease.md", b"Rent: 620\n");
        desk.write("config/config.toml", "language = \"fr\"\n");
        // The laptop's own files, as an editor leaves them (others may read
        // them): one the desk's version will replace, a setting the desk's
        // will join, and a note the sharing never writes here.
        put(&laptop_notes, "admin/lease.md", b"Rent: 600\n");
        put(&laptop_notes, "mine.md", b"Mine\n");
        laptop.write("config/config.toml", "theme = \"dark\"\n");
        for path in [laptop_notes.join("admin/lease.md"), laptop_notes.join("mine.md"), laptop.path("config/config.toml")] {
            set(&path, 0o644);
        }
        set(&laptop_notes.join("admin"), 0o755);
        // The laptop's lease from the day before: the desk's is the later word.
        std::fs::File::options().write(true).open(laptop_notes.join("admin/lease.md")).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_millis((NOW - 24 * 60 * MINUTE) as u64)).unwrap();
        let mut now = NOW;
        for _ in 0..3 {
            now += MINUTE;
            desk.exchange_with(&folder, &key, now, &desk_notes, &["notes"]);
            now += MINUTE;
            laptop.exchange_with(&folder, &key, now, &laptop_notes, &["notes"]);
        }
        assert_eq!(std::fs::read(laptop_notes.join("admin/lease.md")).unwrap(), b"Rent: 620\n");
        assert!(laptop.read("config/config.toml").contains("language = \"fr\""), "{}", laptop.read("config/config.toml"));
        assert_eq!(copies_of(&laptop_notes.join("admin"), "lease").len(), 1, "the laptop's version kept beside");
        // Written by the sharing: yours alone, whatever they were.
        assert_eq!(mode(&laptop_notes.join("admin/lease.md")), 0o600);
        assert_eq!(mode(&laptop.path("config/config.toml")), 0o600);
        for (name, _) in files_in(&laptop_notes.join("admin")).into_iter().filter(|(n, _)| n.starts_with("lease (")) {
            assert_eq!(mode(&laptop_notes.join("admin").join(&name)), 0o600, "{name}");
        }
        assert_eq!(mode(&desk_notes.join("mine.md")), 0o600, "brought to the desk");
        // Never written here by the sharing: as they were.
        assert_eq!(mode(&laptop_notes.join("mine.md")), 0o644);
        assert_eq!(mode(&desk_notes.join("admin/lease.md")), 0o644);
        assert_eq!(mode(&laptop_notes.join("admin")), 0o755, "a folder there already keeps its mode");
        // Everything in the sharing folder, and the versions kept: files 0600, folders 0700.
        let history = crate::history::root(&laptop.memory);
        assert!(!crate::history::files(&history, "settings").is_empty(), "a version kept");
        for root in [folder.clone(), history.clone()] {
            let mut stack = vec![root];
            while let Some(dir) = stack.pop() {
                assert_eq!(mode(&dir), 0o700, "{}", dir.display());
                for entry in std::fs::read_dir(&dir).unwrap().map(Result::unwrap) {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else {
                        assert_eq!(mode(&path), 0o600, "{}", path.display());
                    }
                }
            }
        }
        // A record file from before this was so: yours alone at its next line.
        let rounds: Vec<PathBuf> = std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "jsonl") && p.to_string_lossy().contains(&desk.id)).collect();
        assert!(!rounds.is_empty(), "the desk's records");
        for round in &rounds {
            set(round, 0o644);
        }
        desk.write("config/config.toml", "language = \"en\"\n");
        desk.exchange_with(&folder, &key, now + MINUTE, &desk_notes, &["notes"]);
        let written: Vec<&PathBuf> = rounds.iter().filter(|r| mode(r) == 0o600).collect();
        assert_eq!(written.len(), 1, "the round written to, and it alone: {rounds:?}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn earlier_versions_are_kept_and_put_back() {
        let base = scratch("history");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (desk_notes, laptop_notes) = (base.join("desk-notes"), base.join("laptop-notes"));
        let history = crate::history::root(&laptop.memory);
        let mut now = NOW;
        let both = |now: &mut i64| {
            *now += MINUTE;
            desk.exchange_with(&folder, &key, *now, &desk_notes, &["notes"]);
            *now += MINUTE;
            laptop.exchange_with(&folder, &key, *now, &laptop_notes, &["notes"])
        };
        // Three versions written on the desk, of one size and in one millisecond:
        // the laptop keeps the two it had before each came.
        desk.write("config/config.toml", "language = \"fr\"\n");
        for n in 1..=3 {
            put(&desk_notes, "lease.md", format!("version {n}\n").as_bytes());
            let at = std::time::UNIX_EPOCH + std::time::Duration::from_nanos(NOW as u64 * 1_000_000 + n * 100);
            std::fs::File::options().write(true).open(desk_notes.join("lease.md")).unwrap().set_modified(at).unwrap();
            both(&mut now);
        }
        desk.write("config/config.toml", "language = \"en\"\n");
        both(&mut now);
        assert_eq!(std::fs::read(laptop_notes.join("lease.md")).unwrap(), b"version 3\n");
        let kept = crate::history::versions(&history, "notes", "files/notes/lease.md");
        let texts: Vec<String> = kept.iter().map(|v| version_text(&history, "notes", "files/notes/lease.md", &v.stamp, &folder, &key)).collect();
        assert_eq!(texts, ["version 2\n", "version 1\n"], "the newest first");
        assert_eq!(crate::history::files(&history, "settings").iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>(), ["config/config.toml"]);
        // Version 1 put back on the laptop: the one there now is kept too, and the next exchange sends it.
        let every = laptop.stores_with(&laptop_notes, &|_| true);
        put_back(&laptop.memory, Some((&folder, &key)), &every, "notes", "files/notes/lease.md", &kept[1].stamp, now).unwrap();
        assert_eq!(std::fs::read(laptop_notes.join("lease.md")).unwrap(), b"version 1\n");
        assert_eq!(crate::history::versions(&history, "notes", "files/notes/lease.md").len(), 3);
        // The setting as it was before the desk's last change, put back the same way.
        let setting = crate::history::versions(&history, "settings", "config/config.toml");
        put_back(&laptop.memory, Some((&folder, &key)), &every, "settings", "config/config.toml", &setting[0].stamp, now).unwrap();
        assert!(laptop.read("config/config.toml").contains("\"fr\""));
        // A file of one part is not put back as another's; nor a version never kept.
        assert!(put_back(&laptop.memory, Some((&folder, &key)), &every, "settings", "files/notes/lease.md", &kept[1].stamp, now).is_err());
        assert!(put_back(&laptop.memory, Some((&folder, &key)), &every, "notes", "files/notes/lease.md", "2020-01-01T00-00-00.000Z", now).is_err());
        now += MINUTE;
        assert!(laptop.exchange_with(&folder, &key, now, &laptop_notes, &["notes"]).sent >= 2);
        now += MINUTE;
        desk.exchange_with(&folder, &key, now, &desk_notes, &["notes"]);
        assert_eq!(std::fs::read(desk_notes.join("lease.md")).unwrap(), b"version 1\n", "put back on one, put back on all");
        assert!(copies_of(&desk_notes, "lease").is_empty(), "{:?}", files_in(&desk_notes).into_iter().map(|(n, b)| (n, String::from_utf8_lossy(&b).to_string())).collect::<Vec<_>>());
        assert!(desk.read("config/config.toml").contains("\"fr\""));
        // A note taken out on the desk (not its last: a folder emptied waits
        // for a word) goes on the laptop once gone a while, kept there, and can be put back.
        put(&desk_notes, "other.md", b"other\n");
        both(&mut now);
        std::fs::remove_file(desk_notes.join("lease.md")).unwrap();
        both(&mut now);
        assert!(laptop_notes.join("lease.md").exists(), "not before ten minutes");
        for _ in 0..6 {
            both(&mut now);
        }
        assert!(!laptop_notes.join("lease.md").exists());
        let kept = crate::history::versions(&history, "notes", "files/notes/lease.md");
        put_back(&laptop.memory, Some((&folder, &key)), &every, "notes", "files/notes/lease.md", &kept[0].stamp, now).unwrap();
        both(&mut now);
        both(&mut now);
        assert_eq!(std::fs::read(desk_notes.join("lease.md")).unwrap(), b"version 1\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_damaged_blob_or_record_never_erases_the_file_here() {
        let base = scratch("damaged");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (desk_notes, laptop_notes) = (base.join("desk-notes"), base.join("laptop-notes"));
        put(&desk_notes, "lease.md", b"version 1\n");
        desk.exchange_with(&folder, &key, NOW, &desk_notes, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + MINUTE, &laptop_notes, &["notes"]);
        // The next version's sealed file damaged on the way: said, the file here stays as it is.
        put(&desk_notes, "lease.md", b"version 2, longer\n");
        desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &desk_notes, &["notes"]);
        let sealed = crate::blobs::path(&folder, &key, &crate::blobs::hash_file(&desk_notes.join("lease.md")).unwrap().0);
        let mut bytes = std::fs::read(&sealed).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0x20;
        std::fs::write(&sealed, &bytes).unwrap();
        for minute in [3, 4] {
            let outcome = laptop.exchange_with(&folder, &key, NOW + minute * MINUTE, &laptop_notes, &["notes"]);
            assert!(outcome.problems.iter().any(|p| p == "share-damaged:files/notes/lease.md") && outcome.pending == 1, "{outcome:?}");
            assert_eq!(std::fs::read(laptop_notes.join("lease.md")).unwrap(), b"version 1\n");
        }
        // Only a placeholder of the next (a file kept on demand): not there yet, nothing said, nothing lost.
        put(&desk_notes, "lease.md", b"version 3\n");
        desk.exchange_with(&folder, &key, NOW + 5 * MINUTE, &desk_notes, &["notes"]);
        let next = crate::blobs::path(&folder, &key, &crate::blobs::hash_file(&desk_notes.join("lease.md")).unwrap().0);
        let whole = std::fs::read(&next).unwrap();
        std::fs::write(&next, b"").unwrap();
        let outcome = laptop.exchange_with(&folder, &key, NOW + 6 * MINUTE, &laptop_notes, &["notes"]);
        assert!(outcome.problems.is_empty() && outcome.pending == 1, "{outcome:?}");
        assert_eq!(std::fs::read(laptop_notes.join("lease.md")).unwrap(), b"version 1\n");
        // A record that is not one, and a line that does not open: said, the file here stays.
        let last = written(&desk.memory, &desk.id).unwrap();
        let clock = ((NOW + 7 * MINUTE) as u64) << 16;
        let plain = serde_json::to_vec(&Change { k: "files/notes/lease.md#".into(), v: Some("not a reference".into()), b: String::new(), build: String::new(), f: 0 }).unwrap();
        let mut text = std::fs::read_to_string(round_file(&folder, &desk.id, last.0)).unwrap();
        text.push_str("{\"n\": 99, \"c\": 1, \"s\": \"broken\"}\n");
        text.push_str(&(serde_json::to_string(&Line { n: last.1 + 1, c: clock, s: seal(&key, &bound(&desk.id, last.0, last.1 + 1, clock), &plain) }).unwrap() + "\n"));
        std::fs::write(round_file(&folder, &desk.id, last.0), text).unwrap();
        let outcome = laptop.exchange_with(&folder, &key, NOW + 8 * MINUTE, &laptop_notes, &["notes"]);
        assert!(outcome.problems.iter().any(|p| p.starts_with("share-other-seal")) && outcome.problems.iter().any(|p| p == "share-damaged:files/notes/lease.md"), "{outcome:?}");
        assert_eq!(std::fs::read(laptop_notes.join("lease.md")).unwrap(), b"version 1\n");
        // The whole one comes at last, then a later change: written.
        std::fs::write(&next, &whole).unwrap();
        put(&desk_notes, "lease.md", b"version 4\n");
        desk.exchange_with(&folder, &key, NOW + 9 * MINUTE, &desk_notes, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + 10 * MINUTE, &laptop_notes, &["notes"]);
        assert_eq!(std::fs::read(laptop_notes.join("lease.md")).unwrap(), b"version 4\n");
        assert!(copies_of(&laptop_notes, "lease").is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_notes_folder_found_empty_takes_nothing_out() {
        let base = scratch("notes-emptied");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (desk_notes, laptop_notes) = (base.join("desk-notes"), base.join("laptop-notes"));
        for n in 0..6 {
            put(&desk_notes, &format!("note-{n}.md"), format!("# {n}\n").as_bytes());
        }
        desk.exchange_with(&folder, &key, NOW, &desk_notes, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + MINUTE, &laptop_notes, &["notes"]);
        // Every note gone at once here (a card not mounted, another program): said, nothing taken out there.
        for n in 0..6 {
            std::fs::remove_file(desk_notes.join(format!("note-{n}.md"))).unwrap();
        }
        let outcome = desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &desk_notes, &["notes"]);
        assert!(outcome.problems.iter().any(|p| p == "share-vanished:files/notes/:6") && outcome.sent == 0, "{outcome:?}");
        // Gone as a folder, the same.
        std::fs::remove_dir_all(&desk_notes).unwrap();
        assert_eq!(desk.exchange_with(&folder, &key, NOW + 3 * MINUTE, &desk_notes, &["notes"]).sent, 0);
        laptop.exchange_with(&folder, &key, NOW + 4 * MINUTE, &laptop_notes, &["notes"]);
        assert_eq!(files_in(&laptop_notes).len(), 6);
        // One taken out on purpose there goes, as ever, once gone a while; nothing is written here meanwhile.
        std::fs::remove_file(laptop_notes.join("note-0.md")).unwrap();
        put(&laptop_notes, "note-1.md", b"# 1, changed\n");
        laptop.exchange_with(&folder, &key, NOW + 5 * MINUTE, &laptop_notes, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + 16 * MINUTE, &laptop_notes, &["notes"]);
        assert!(desk.exchange_with(&folder, &key, NOW + 17 * MINUTE, &desk_notes, &["notes"]).pending > 0);
        assert!(!desk_notes.exists());
        // Notes switched off and on here: filled again from the others, as a new device's.
        desk.exchange_with(&folder, &key, NOW + 18 * MINUTE, &desk_notes, &[]);
        desk.exchange_with(&folder, &key, NOW + 19 * MINUTE, &desk_notes, &["notes"]);
        let files = files_in(&desk_notes);
        assert_eq!(files.keys().map(String::as_str).collect::<Vec<_>>(), ["note-1.md", "note-2.md", "note-3.md", "note-4.md", "note-5.md"]);
        assert_eq!(files["note-1.md"], b"# 1, changed\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A record of another device's, sealed: its file, as a sync app would bring it.
    pub(super) fn record_of(key: &[u8; 32], computer: &str, n: u64, clock: u64, change: &Change) -> String {
        let plain = serde_json::to_vec(change).unwrap();
        serde_json::to_string(&Line { n, c: clock, s: seal(key, &bound(computer, 1, n, clock), &plain) }).unwrap() + "\n"
    }

    pub(super) fn append_to(path: &Path, text: &str) {
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    pub(super) fn copy_dir(from: &Path, to: &Path) {
        for entry in std::fs::read_dir(from).unwrap().filter_map(Result::ok) {
            let target = to.join(entry.file_name());
            if entry.path().is_dir() {
                std::fs::create_dir_all(&target).unwrap();
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::create_dir_all(to).unwrap();
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn dated(path: &Path, at: std::time::SystemTime) {
        std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
    }

    // F1
    #[test]
    fn a_put_back_takes_out_nothing_added_since() {
        let base = scratch("put-back-entries");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let mut now = NOW;
        let both = |now: &mut i64| {
            *now += MINUTE;
            desk.exchange(&folder, &key, *now);
            *now += MINUTE;
            phone.exchange(&folder, &key, *now);
        };
        desk.write("state/health-state.toml", "[taken]\nd1 = 1\n");
        desk.write("config/config.toml", "language = \"fr\"\n");
        both(&mut now);
        // On the phone: a dose, then another, the language changed, an account added.
        phone.write("state/health-state.toml", "[taken]\nd1 = 1\nd2 = 2\n");
        both(&mut now);
        phone.write("state/health-state.toml", "[taken]\nd1 = 1\nd2 = 2\nd3 = 3\n");
        phone.write("config/config.toml", "language = \"en\"\n\n[[account]]\nid = \"new\"\nkind = \"imap\"\n");
        both(&mut now);
        both(&mut now);
        let history = crate::history::root(&desk.memory);
        let doses = crate::history::versions(&history, "health", "state/health-state.toml");
        assert_eq!(doses.len(), 2, "{doses:?}");
        let every = stores_of(&Config::default(), &desk.roots, &|_| true);
        // The doses' record as it was before the last dose came: said first, nothing taken out.
        let back = put_back_preview(&desk.memory, &every, "health", "state/health-state.toml", &doses[0].stamp).unwrap();
        assert_eq!(back, Back { whole: false, changed: 0, returning: 0, kept: 1 });
        put_back(&desk.memory, Some((&folder, &key)), &every, "health", "state/health-state.toml", &doses[0].stamp, now).unwrap();
        // The settings as they were: the language back, the account added since kept.
        let settings = crate::history::versions(&history, "settings", "config/config.toml");
        let back = put_back_preview(&desk.memory, &every, "settings", "config/config.toml", &settings[0].stamp).unwrap();
        assert_eq!(back, Back { whole: false, changed: 1, returning: 0, kept: 1 });
        put_back(&desk.memory, Some((&folder, &key)), &every, "settings", "config/config.toml", &settings[0].stamp, now).unwrap();
        both(&mut now);
        both(&mut now);
        for computer in [&desk, &phone] {
            let doses = computer.read("state/health-state.toml");
            assert!(["d1", "d2", "d3"].iter().all(|d| doses.contains(d)), "a dose marked after that version was taken out: {doses}");
            let config = computer.read("config/config.toml");
            assert!(config.contains("language = \"fr\"") && config.contains("id = \"new\""), "{config}");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    // F2
    #[test]
    fn a_quick_exchange_leaves_notes_for_later() {
        let base = scratch("quick");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let (dn, pn) = (base.join("desk-notes"), base.join("phone-notes"));
        put(&dn, "a.md", b"# a\n");
        desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]);
        phone.exchange_with(&folder, &key, NOW + MINUTE, &pn, &["notes"]);
        // A dose and a note on the desk: the phone's alarm takes the dose, leaves the note.
        desk.write("state/health-state.toml", "[taken]\nd1 = 1\n");
        put(&dn, "a.md", b"# a, changed\n");
        desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &dn, &["notes"]);
        let stores = phone.stores_with(&pn, &|part| part == "notes");
        let quick = exchange(&Sharing { folder: &folder, computer: &phone.id, key: &key, memory: &phone.memory, files: false, hurry: None }, &stores, NOW + 3 * MINUTE).unwrap();
        assert!(phone.read("state/health-state.toml").contains("d1"));
        assert_eq!(std::fs::read(pn.join("a.md")).unwrap(), b"# a\n");
        assert_eq!(quick.pending, 1, "{quick:?}");
        // The next whole exchange brings it; nothing joins again (nothing goes out).
        let whole = phone.exchange_with(&folder, &key, NOW + 4 * MINUTE, &pn, &["notes"]);
        assert_eq!(std::fs::read(pn.join("a.md")).unwrap(), b"# a, changed\n");
        assert_eq!(whole.sent, 0, "{whole:?}");
        // Hurried while it runs: its notes wait where they are, the rest is done.
        put(&dn, "a.md", b"# a, again\n");
        desk.write("state/health-state.toml", "[taken]\nd1 = 1\nd2 = 2\n");
        desk.exchange_with(&folder, &key, NOW + 5 * MINUTE, &dn, &["notes"]);
        let hurry = std::sync::atomic::AtomicBool::new(true);
        exchange(&Sharing { folder: &folder, computer: &phone.id, key: &key, memory: &phone.memory, files: true, hurry: Some(&hurry) }, &stores, NOW + 6 * MINUTE).unwrap();
        assert!(phone.read("state/health-state.toml").contains("d2"));
        assert_eq!(std::fs::read(pn.join("a.md")).unwrap(), b"# a, changed\n");
        hurry.store(false, std::sync::atomic::Ordering::Relaxed);
        exchange(&Sharing { folder: &folder, computer: &phone.id, key: &key, memory: &phone.memory, files: true, hurry: Some(&hurry) }, &stores, NOW + 7 * MINUTE).unwrap();
        assert_eq!(std::fs::read(pn.join("a.md")).unwrap(), b"# a, again\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    // F4
    #[test]
    fn notes_gone_at_once_wait_for_a_word() {
        let base = scratch("vanished");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
        put(&dn, "today.md", b"# Today\n");
        for n in 0..20 {
            put(&dn, &format!("archive/{n}.md"), format!("# {n}\n").as_bytes());
        }
        desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + MINUTE, &ln, &["notes"]);
        // A folder listing empty (a disk not mounted on its place): unknown, said, nothing taken out.
        std::fs::remove_dir_all(dn.join("archive")).unwrap();
        std::fs::create_dir(dn.join("archive")).unwrap();
        let outcome = desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &dn, &["notes"]);
        assert!(outcome.problems.iter().any(|p| p == "share-vanished:files/notes/archive/:20") && outcome.sent == 0, "{outcome:?}");
        laptop.exchange_with(&folder, &key, NOW + 3 * MINUTE, &ln, &["notes"]);
        assert_eq!(files_in(&ln).len(), 21);
        // Gone with its folder, many at once: held the same, ten minutes and more.
        std::fs::remove_dir(dn.join("archive")).unwrap();
        for minute in [4, 20] {
            let outcome = desk.exchange_with(&folder, &key, NOW + minute * MINUTE, &dn, &["notes"]);
            assert!(outcome.problems.iter().any(|p| p == "share-vanished:files/notes/:20") && outcome.sent == 0, "{outcome:?}");
        }
        laptop.exchange_with(&folder, &key, NOW + 21 * MINUTE, &ln, &["notes"]);
        assert_eq!(files_in(&ln).len(), 21);
        // A folder that cannot be read: unknown, nothing of it taken out.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            put(&dn, "locked/x.md", b"# x\n");
            desk.exchange_with(&folder, &key, NOW + 22 * MINUTE, &dn, &["notes"]);
            std::fs::set_permissions(dn.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
            if std::fs::read_dir(dn.join("locked")).is_err() {
                for minute in [23, 40] {
                    desk.exchange_with(&folder, &key, NOW + minute * MINUTE, &dn, &["notes"]);
                }
                laptop.exchange_with(&folder, &key, NOW + 41 * MINUTE, &ln, &["notes"]);
                assert!(ln.join("locked/x.md").exists());
            }
            std::fs::set_permissions(dn.join("locked"), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // You say so: they go, everywhere.
        confirm_gone(&desk.memory, &desk.id, "files/notes/").unwrap();
        let outcome = desk.exchange_with(&folder, &key, NOW + 42 * MINUTE, &dn, &["notes"]);
        assert!(outcome.sent >= 20, "{outcome:?}");
        laptop.exchange_with(&folder, &key, NOW + 43 * MINUTE, &ln, &["notes"]);
        let left: Vec<String> = files_in(&ln).into_keys().collect();
        assert!(left.iter().all(|f| !f.starts_with("archive/")) && left.contains(&"today.md".to_string()), "{left:?}");
        assert!(!ln.join("archive").exists(), "its folder, left empty, goes too");
        let _ = std::fs::remove_dir_all(&base);
    }

    // F5 and F21
    #[test]
    fn no_room_holds_a_file_back_and_tries_again_later() {
        let base = scratch("no-room");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
        put(&dn, "scan.pdf", &noise(100_000, 1));
        put(&dn, "other.md", b"other\n");
        desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + MINUTE, &ln, &["notes"]);
        put(&dn, "scan.pdf", &noise(100_000, 2));
        desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &dn, &["notes"]);
        // No room on the laptop: said, the file there as it was, tried again after a while only.
        crate::disk::ROOM.with(|room| room.set(Some(1000)));
        let outcome = laptop.exchange_with(&folder, &key, NOW + 3 * MINUTE, &ln, &["notes"]);
        assert!(outcome.problems.iter().any(|p| p == "share-no-room:files/notes/scan.pdf") && outcome.pending == 1, "{outcome:?}");
        let outcome = laptop.exchange_with(&folder, &key, NOW + 3 * MINUTE + 30_000, &ln, &["notes"]);
        assert!(outcome.problems.is_empty(), "not opened again within the minute: {outcome:?}");
        assert_eq!(std::fs::read(ln.join("scan.pdf")).unwrap(), noise(100_000, 1));
        crate::disk::ROOM.with(|room| room.set(Some(u64::MAX)));
        laptop.exchange_with(&folder, &key, NOW + 5 * MINUTE, &ln, &["notes"]);
        assert_eq!(std::fs::read(ln.join("scan.pdf")).unwrap(), noise(100_000, 2));
        // What it held kept by reference (its content was sealed in the folder already), not copied.
        let kept = crate::history::versions(&crate::history::root(&laptop.memory), "notes", "files/notes/scan.pdf");
        assert!(kept.len() == 1 && kept[0].sealed.is_some(), "{kept:?}");
        assert_eq!(version_text_bytes(&crate::history::root(&laptop.memory), "notes", "files/notes/scan.pdf", &kept[0].stamp, &folder, &key), noise(100_000, 1));
        let _ = std::fs::remove_dir_all(&base);
    }

    fn version_text_bytes(history: &Path, part: &str, file: &str, stamp: &str, folder: &Path, key: &[u8; 32]) -> Vec<u8> {
        let Some(crate::history::Kept::Sealed { hash, .. }) = crate::history::version(history, part, file, stamp) else { panic!("{file}: not by reference") };
        let target = temporary(&history.join("opened"));
        crate::blobs::get(folder, key, &hash, &target).unwrap();
        let bytes = std::fs::read(&target).unwrap();
        std::fs::remove_file(target).unwrap();
        bytes
    }

    // F6
    #[test]
    fn huge_or_endless_files_in_the_folder_are_never_read_whole() {
        let base = scratch("huge");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        desk.write("config/safe-senders.txt", "a@example.org\n");
        desk.exchange(&folder, &key, NOW);
        // From anyone who can write the folder: a line of 40 MB, then a record of a key holder's; notes to each other of 100 kB.
        let stranger = uuid::Uuid::new_v4().to_string();
        {
            let mut file = std::fs::File::create(round_file(&folder, &stranger, 1)).unwrap();
            let piece = vec![b'x'; 1 << 20];
            for _ in 0..40 {
                file.write_all(&piece).unwrap();
            }
            file.write_all(b"\n").unwrap();
        }
        let clock = ((NOW + MINUTE) as u64) << 16;
        append_to(&round_file(&folder, &stranger, 1), &record_of(&key, &stranger, 1, clock, &Change { k: "config/safe-senders.txt#b@example.org".into(), v: Some(String::new()), b: String::new(), build: String::new(), f: 0 }));
        std::fs::write(seen_path(&folder, &stranger), "x".repeat(100_000)).unwrap();
        let outcome = laptop.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert!(outcome.problems.iter().any(|p| *p == format!("share-other-line:{stranger}")), "{outcome:?}");
        let safe = laptop.read("config/safe-senders.txt");
        assert!(safe.contains("a@example.org") && safe.contains("b@example.org"), "{safe}");
        assert!(others(&folder, &laptop.id).iter().any(|o| o.id == stranger && o.heard == 0), "its notes, too long, not read");
        // An own file that does not end a line is mended from its end, never read whole.
        let own = round_file(&folder, &desk.id, 1);
        let whole = std::fs::metadata(&own).unwrap().len();
        append_to(&own, "{\"n\": 99, \"c\": 1, \"s\": \"half");
        assert_eq!(unfinished(&own), Some(whole));
        desk.write("config/safe-senders.txt", "a@example.org\nc@example.org\n");
        desk.exchange(&folder, &key, NOW + 3 * MINUTE);
        assert!(std::fs::read_to_string(&own).unwrap().lines().all(|l| l.ends_with('}')));
        let _ = std::fs::remove_dir_all(&base);
    }

    // F7 and F8
    #[test]
    fn an_edit_made_while_receiving_is_never_lost() {
        let base = scratch("while-receiving");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
        for name in ["a.md", "b.md", "lease.md"] {
            put(&dn, name, format!("{name}: one\n").as_bytes());
        }
        desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + MINUTE, &ln, &["notes"]);
        for name in ["a.md", "b.md", "lease.md"] {
            put(&dn, name, format!("{name}: two, from the desk\n").as_bytes());
        }
        desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &dn, &["notes"]);
        // While the laptop writes them: the lease edited there just before its turn (an editor
        // saving), and a.md, written already, edited while b.md is written.
        let (lease, first) = (ln.join("lease.md"), ln.join("a.md"));
        BEFORE_WRITING.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move |path: &Path| {
                if path.ends_with("lease.md") {
                    std::fs::write(&lease, "lease.md: edited on the laptop\n").unwrap();
                }
                if path.ends_with("b.md") {
                    std::fs::write(&first, "a.md: edited on the laptop\n").unwrap();
                }
            }))
        });
        laptop.exchange_with(&folder, &key, NOW + 3 * MINUTE, &ln, &["notes"]);
        BEFORE_WRITING.with(|hook| *hook.borrow_mut() = None);
        assert_eq!(std::fs::read_to_string(ln.join("lease.md")).unwrap(), "lease.md: edited on the laptop\n", "not written over");
        for minute in 4..10 {
            if minute % 2 == 0 {
                laptop.exchange_with(&folder, &key, NOW + minute * MINUTE, &ln, &["notes"]);
            } else {
                desk.exchange_with(&folder, &key, NOW + minute * MINUTE, &dn, &["notes"]);
            }
        }
        // Both edits went out. The lease, edited before the desk's came, is
        // kept both ways; a.md, edited on top of the desk's, follows it.
        for notes in [&dn, &ln] {
            let all: Vec<String> = files_in(notes).into_values().map(|b| String::from_utf8(b).unwrap()).collect();
            for text in ["lease.md: edited on the laptop\n", "lease.md: two, from the desk\n", "a.md: edited on the laptop\n"] {
                assert!(all.iter().any(|t| t == text), "{}: {text:?} lost: {all:?}", notes.display());
            }
            assert_eq!(std::fs::read_to_string(notes.join("a.md")).unwrap(), "a.md: edited on the laptop\n");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    // F9
    #[test]
    fn a_part_switched_off_keeps_nothing_waiting() {
        let base = scratch("off-pending");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let off = stores_of(&Config::default(), &phone.roots, &|part| part != "drafts");
        for n in 0..20 {
            desk.write(&format!("data/drafts/{n}.eml"), &"x".repeat(200_000));
            desk.exchange(&folder, &key, NOW + 2 * n * MINUTE);
            exchange(&Sharing { folder: &folder, computer: &phone.id, key: &key, memory: &phone.memory, files: true, hurry: None }, &off, NOW + (2 * n + 1) * MINUTE).unwrap();
        }
        assert!(std::fs::metadata(&phone.memory).unwrap().len() < 200_000, "memory.json holds the drafts of a part switched off");
        // Switched on: it joins, and the drafts come.
        phone.exchange(&folder, &key, NOW + 41 * MINUTE);
        assert_eq!(phone.read("data/drafts/19.eml").len(), 200_000);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A device on an older Sioul still shares a watch's days and the memory
    /// of its offers (taken out on 8 October 2026): left aside here, never
    /// written, never kept waiting, never a problem; the rest of the
    /// exchange comes in, and the older device's files stay as they are.
    #[test]
    fn an_older_sioul_s_watch_is_left_aside() {
        let base = scratch("watch-older");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (older, desk) = (Computer::new(&base, "older"), Computer::new(&base, "desk"));
        // The older Sioul's stores: this version's, and the watch's as it shared them.
        let mut with_watch = stores(&Config::default(), &older.roots);
        with_watch.push(Store { name: "data/watch/".into(), path: older.roots.data.join("watch"), folder: true, shape: Shape::Whole, skip: &["imported.json"], part: "watch" });
        with_watch.push(Store { name: "state/watch-offers.json".into(), path: older.roots.state.join("watch-offers.json"), folder: false, shape: Shape::Whole, skip: &[], part: "watch" });
        let older_exchange = |now: i64| exchange(&Sharing { folder: &folder, computer: &older.id, key: &key, memory: &older.memory, files: true, hurry: None }, &with_watch, now).unwrap();
        older.write("data/watch/2026-10-07.json", "{\"steps\": 4200}");
        older.write("state/watch-offers.json", "{\"declined\": {}}");
        older.write("config/safe-senders.txt", "friend@example.org\n");
        let sent = older_exchange(NOW);
        assert!(sent.problems.is_empty() && sent.sent >= 3, "{sent:?}");
        for n in 1..=2 {
            let came = desk.exchange(&folder, &key, NOW + n * MINUTE);
            assert!(came.problems.is_empty() && came.pending == 0, "{came:?}");
        }
        assert_eq!(desk.read("config/safe-senders.txt"), "friend@example.org\n", "the rest came in");
        assert!(!desk.path("data/watch").exists() && !desk.path("state/watch-offers.json").exists(), "nothing of the watch written");
        // Its records named by their stores (the folders' own paths hold the test's name).
        let memory = std::fs::read_to_string(&desk.memory).unwrap();
        assert!(!memory.contains("data/watch/") && !memory.contains("watch-offers"), "nothing of the watch kept waiting");
        // A new day from the older device, and a change it receives: the exchanges go on both ways.
        older.write("data/watch/2026-10-08.json", "{\"steps\": 900}");
        older_exchange(NOW + 3 * MINUTE);
        desk.write("config/safe-senders.txt", "friend@example.org\nneighbour@example.org\n");
        let came = desk.exchange(&folder, &key, NOW + 4 * MINUTE);
        assert!(came.problems.is_empty() && came.pending == 0 && came.sent >= 1, "{came:?}");
        let back = older_exchange(NOW + 5 * MINUTE);
        assert!(back.problems.is_empty() && back.received >= 1, "{back:?}");
        assert_eq!(older.read("config/safe-senders.txt"), "friend@example.org\nneighbour@example.org\n");
        assert_eq!(older.read("data/watch/2026-10-07.json"), "{\"steps\": 4200}", "its own files as they were");
        assert!(!desk.path("data/watch").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    // F10
    #[cfg(unix)]
    #[test]
    fn nothing_is_written_through_a_link_nor_into_the_sharing_folder() {
        let base = scratch("through-link");
        let notes = base.join("desk-notes");
        // Shared through a folder inside the notes, as Sioul may suggest.
        let folder = notes.join("sioul-shared");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        let outside = base.join("outside");
        put(&notes, "lease.md", b"x\n");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, notes.join("link")).unwrap();
        desk.exchange_with(&folder, &key, NOW, &notes, &["notes"]);
        // A key holder's records: one through the link, one into the sharing folder.
        let source = base.join("evil");
        std::fs::write(&source, b"[Desktop Entry]\nExec=sh -c id\n").unwrap();
        let (h, s) = crate::blobs::hash_file(&source).unwrap();
        crate::blobs::put(&folder, &key, &source, &h).unwrap();
        let other = uuid::Uuid::new_v4().to_string();
        let value = reference(&h, s, 0);
        let mut text = String::new();
        for (n, file) in [(1, "files/notes/link/evil.desktop#"), (2, "files/notes/sioul-shared/plain.md#")] {
            text += &record_of(&key, &other, n, ((NOW + MINUTE) as u64) << 16 | n, &Change { k: file.into(), v: Some(value.clone()), b: String::new(), build: String::new(), f: 0 });
        }
        std::fs::write(round_file(&folder, &other, 1), text).unwrap();
        let outcome = desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &notes, &["notes"]);
        assert!(!outside.join("evil.desktop").exists(), "written through the link");
        assert!(!folder.join("plain.md").exists(), "written in plain into the sharing folder");
        assert!(outcome.problems.iter().filter(|p| p.starts_with("share-refused:")).count() == 2, "{outcome:?}");
        let _ = std::fs::remove_dir_all(&base);
    }

    // F11
    #[test]
    fn a_content_used_again_is_never_lost_to_a_sweep() {
        let base = scratch("sweep-race");
        let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
        let key = quick_key(&server, "four words make a passphrase").unwrap();
        let (desk, phone, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"), Computer::new(&base, "laptop"));
        let (dn, pn, ln) = (base.join("desk-notes"), base.join("phone-notes"), base.join("laptop-notes"));
        let day = 86_400_000;
        put(&dn, "a.md", b"template\n");
        put(&dn, "keep.md", b"keep\n");
        desk.exchange_with(&server, &key, NOW, &dn, &["notes"]);
        put(&dn, "a.md", b"changed\n");
        desk.exchange_with(&server, &key, NOW + MINUTE, &dn, &["notes"]);
        // The template's sealed file is old (written long ago).
        std::fs::write(base.join("template"), b"template\n").unwrap();
        let sealed = crate::blobs::path(&server, &key, &crate::blobs::hash_file(&base.join("template")).unwrap().0);
        dated(&sealed, std::time::SystemTime::now() - std::time::Duration::from_secs(100 * 86_400));
        copy_dir(&server, &phone_folder);
        // The phone holds the same content again: it finds it sealed, seals nothing, and its record goes.
        put(&pn, "b.md", b"template\n");
        phone.exchange_with(&phone_folder, &key, NOW + 91 * day, &pn, &["notes"]);
        // The desk, not hearing of it yet, sweeps it.
        desk.exchange_with(&server, &key, NOW + 91 * day + MINUTE, &dn, &["notes"]);
        assert!(!sealed.exists(), "swept, as unused for 90 days by what the desk knew");
        // The phone's sync app carries both ways: its records up, the sweep down.
        for entry in std::fs::read_dir(&phone_folder).unwrap().filter_map(Result::ok) {
            if entry.file_name().to_string_lossy().ends_with(".jsonl") {
                std::fs::copy(entry.path(), server.join(entry.file_name())).unwrap();
            }
        }
        let _ = std::fs::remove_file(phone_folder.join("blobs").join(sealed.file_name().unwrap()));
        laptop.exchange_with(&server, &key, NOW + 91 * day + 2 * MINUTE, &ln, &["notes"]);
        assert!(!ln.join("b.md").exists());
        // A day later, still missing: said.
        let outcome = laptop.exchange_with(&server, &key, NOW + 92 * day + 3 * MINUTE, &ln, &["notes"]);
        assert!(outcome.problems.iter().any(|p| p == "share-missing:files/notes/b.md"), "{outcome:?}");
        // The phone looks after its sealed files: gone, sealed again; it comes.
        phone.exchange_with(&phone_folder, &key, NOW + 92 * day + 4 * MINUTE, &pn, &["notes"]);
        assert!(phone_folder.join("blobs").join(sealed.file_name().unwrap()).exists());
        copy_dir(&phone_folder.join("blobs"), &server.join("blobs"));
        laptop.exchange_with(&server, &key, NOW + 92 * day + 5 * MINUTE, &ln, &["notes"]);
        assert_eq!(std::fs::read(ln.join("b.md")).ok().as_deref(), Some(&b"template\n"[..]));
        let _ = std::fs::remove_dir_all(&base);
    }

    // F12
    #[test]
    fn one_exchange_at_a_time() {
        let base = scratch("one-at-a-time");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        desk.write("config/safe-senders.txt", "a@example.org\n");
        // Another Sioul (or the command line) exchanging: a whole exchange does nothing, and says so.
        let running = exchange_lock(&desk.memory, true).unwrap();
        let outcome = desk.exchange(&folder, &key, NOW);
        assert_eq!((outcome.problems.as_slice(), outcome.sent), (&["share-busy".to_string()][..], 0));
        drop(running);
        assert_eq!(desk.exchange(&folder, &key, NOW + MINUTE).sent, 1);
        // Held a moment only (a program starting elsewhere in Sioul holds a copy
        // of the lock until it runs, a Mac's for milliseconds): waited for.
        let memory = desk.memory.clone();
        let (held, release) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let running = exchange_lock(&memory, true).unwrap();
            held.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(150));
            drop(running);
        });
        release.recv().unwrap();
        desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
        let outcome = desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert_eq!((outcome.problems.len(), outcome.sent), (0, 1), "{outcome:?}");
        holder.join().unwrap();
        // Files being written never share a name.
        let path = folder.join("x.toml");
        let (one, two) = (temporary(&path), temporary(&path));
        assert!(one != two && one.file_name().unwrap().to_string_lossy().starts_with('.'));
        let _ = std::fs::remove_dir_all(&base);
    }

    // F14
    #[test]
    fn names_one_storage_takes_for_one_are_never_both_written() {
        // The decision, whatever the disk running this: a name another file
        // here or known has by its case, or by how an accent is written, is not its own.
        let known = |file: &str| (format!("{file}#"), Known { c: 1, w: "desk".into(), h: "h".into(), b: String::new() });
        let names = Names::new(&[known("files/notes/Lease.md"), known("files/notes/\u{e9}t\u{e9}.md")].into(), &Found::default());
        assert!(names.clash("files/notes/lease.md") && names.clash("files/notes/LEASE.md") && names.clash("files/notes/e\u{301}te\u{301}.md"));
        assert!(!names.clash("files/notes/Lease.md") && !names.clash("files/notes/\u{e9}t\u{e9}.md") && !names.clash("files/notes/other.md"));
        // Between two devices, with the names this disk can hold apart: Windows'
        // and a Mac's take "Lease.md" and "lease.md" for one file, a Mac's
        // "\u{e9}t\u{e9}" written two ways too; Linux's holds both.
        let base = scratch("case");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
        let apart = |one: &str, other: &str| {
            let probe = base.join("probe");
            std::fs::create_dir_all(&probe).unwrap();
            std::fs::write(probe.join(one), b"").unwrap();
            let apart = !probe.join(other).exists();
            std::fs::remove_dir_all(&probe).unwrap();
            apart
        };
        put(&dn, "other.md", b"other\n");
        let mut pairs = 0;
        for (one, other) in [("Lease.md", "lease.md"), ("\u{e9}t\u{e9}.md", "e\u{301}te\u{301}.md")] {
            if apart(one, other) {
                put(&dn, one, b"one\n");
                put(&dn, other, b"other way\n");
                pairs += 1;
            }
        }
        desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]);
        let outcome = laptop.exchange_with(&folder, &key, NOW + MINUTE, &ln, &["notes"]);
        assert_eq!(outcome.problems.iter().filter(|p| p.starts_with("share-name-clash:")).count(), pairs, "{outcome:?}");
        assert_eq!(files_in(&ln).len(), 1 + pairs);
        // What another part carries is never a note, whatever its case.
        let config = Config { notes_root: Some("/notes".into()), ..Config::default() };
        let every = stores_of(&config, &laptop.roots, &|_| true);
        assert!(locate(&every, "files/notes/Sioul-Cases.toml").is_none() && locate(&every, "files/notes/Sioul-Projects.toml").is_none() && locate(&every, "files/notes/Papers/id.pdf").is_none());
        assert_eq!(kept_by_windows("aux.md"), true);
        assert_eq!(kept_by_windows("auxiliary.md"), false);
        let _ = std::fs::remove_dir_all(&base);
    }

    // F15
    #[test]
    fn a_name_with_a_hash_travels() {
        let base = scratch("hash-name");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let (dn, pn) = (base.join("desk-notes"), base.join("phone-notes"));
        put(&dn, "Issue #12.md", b"# Issue 12\n");
        put(&dn, "#ideas.md", b"# Ideas\n");
        put(&dn, "papers/Facture #123.pdf", &noise(10_000, 3));
        desk.write("data/drafts/a#1.toml", "subject = \"Hash\"\n");
        for m in 0..3 {
            desk.exchange_with(&folder, &key, NOW + 2 * m * MINUTE, &dn, &["notes", "papers"]);
            phone.exchange_with(&folder, &key, NOW + (2 * m + 1) * MINUTE, &pn, &["notes", "papers"]);
        }
        let files = files_in(&pn);
        assert_eq!(files.get("Issue #12.md").map(Vec::as_slice), Some(&b"# Issue 12\n"[..]), "{:?}", files.keys().collect::<Vec<_>>());
        assert!(files.contains_key("#ideas.md") && files.get("papers/Facture #123.pdf") == Some(&noise(10_000, 3)));
        assert_eq!(phone.read("data/drafts/a#1.toml"), "subject = \"Hash\"\n");
        assert_eq!(file_of("files/papers/Facture #123.pdf#"), "files/papers/Facture #123.pdf");
        assert_eq!((file_of("config/safe-senders.txt#a#b@example.org"), entry_of("config/safe-senders.txt#a#b@example.org")), ("config/safe-senders.txt", "a#b@example.org"));
        let _ = std::fs::remove_dir_all(&base);
    }

    // F16
    #[test]
    fn an_old_copy_does_not_roll_the_others_back() {
        let base = scratch("old-copy");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
        let old = std::time::UNIX_EPOCH + std::time::Duration::from_millis(NOW as u64);
        put(&dn, "lease.md", b"Rent: 620\n");
        put(&dn, "other.md", b"other\n");
        dated(&dn.join("lease.md"), old);
        desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + MINUTE, &ln, &["notes"]);
        put(&ln, "lease.md", b"Rent: 640\n");
        laptop.exchange_with(&folder, &key, NOW + 86_400_000, &ln, &["notes"]);
        desk.exchange_with(&folder, &key, NOW + 86_400_000 + MINUTE, &dn, &["notes"]);
        // Restored from a backup: the old content, with its old time.
        put(&dn, "lease.md", b"Rent: 620\n");
        dated(&dn.join("lease.md"), old);
        let outcome = desk.exchange_with(&folder, &key, NOW + 86_400_000 + 2 * MINUTE, &dn, &["notes"]);
        assert!(outcome.problems.iter().any(|p| p.starts_with("share-older-copy:lease (conflict ")), "{outcome:?}");
        assert_eq!(std::fs::read(dn.join("lease.md")).unwrap(), b"Rent: 640\n", "the version last seen came back");
        desk.exchange_with(&folder, &key, NOW + 86_400_000 + 3 * MINUTE, &dn, &["notes"]);
        laptop.exchange_with(&folder, &key, NOW + 86_400_000 + 4 * MINUTE, &ln, &["notes"]);
        assert_eq!(std::fs::read(ln.join("lease.md")).unwrap(), b"Rent: 640\n");
        assert_eq!(copies_of(&ln, "lease"), [b"Rent: 620\n".to_vec()], "the older copy kept beside, everywhere");
        let _ = std::fs::remove_dir_all(&base);
    }

    // F22
    #[test]
    fn a_file_being_written_waits() {
        let base = scratch("settling");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
        let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
        put(&dn, "keep.md", b"keep\n");
        dated(&dn.join("keep.md"), std::time::SystemTime::now() - std::time::Duration::from_secs(120));
        // Changed this very moment: read once it has settled.
        SETTLE.with(|settle| settle.set(60_000));
        put(&dn, "a.md", b"# a\n");
        assert_eq!(desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]).sent, 1);
        dated(&dn.join("a.md"), std::time::SystemTime::now() - std::time::Duration::from_secs(120));
        assert_eq!(desk.exchange_with(&folder, &key, NOW + MINUTE, &dn, &["notes"]).sent, 1);
        SETTLE.with(|settle| settle.set(0));
        laptop.exchange_with(&folder, &key, NOW + 2 * MINUTE, &ln, &["notes"]);
        // Emptied in place (an editor writing it, a crash): held as it was for ten minutes.
        std::fs::write(dn.join("a.md"), b"").unwrap();
        assert_eq!(desk.exchange_with(&folder, &key, NOW + 3 * MINUTE, &dn, &["notes"]).sent, 0);
        laptop.exchange_with(&folder, &key, NOW + 4 * MINUTE, &ln, &["notes"]);
        assert_eq!(std::fs::read(ln.join("a.md")).unwrap(), b"# a\n");
        put(&dn, "a.md", b"# a, written\n");
        assert_eq!(desk.exchange_with(&folder, &key, NOW + 5 * MINUTE, &dn, &["notes"]).sent, 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    // F23
    #[test]
    fn the_doses_never_read_the_notes_memory() {
        let base = scratch("memory-apart");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        let dn = base.join("desk-notes");
        for n in 0..50 {
            put(&dn, &format!("note-{n}.md"), format!("# {n}\n").as_bytes());
        }
        desk.write("state/health-state.toml", "[taken]\nd1 = 1\n");
        desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]);
        let memory = std::fs::read_to_string(&desk.memory).unwrap();
        assert!(!memory.contains("files/notes/note-") && memory.contains("health-state"), "{memory}");
        assert!(std::fs::read_to_string(sealed_path(&desk.memory)).unwrap().contains("files/notes/note-7.md"));
        assert!(written(&desk.memory, &desk.id).is_some_and(|(_, n)| n > 50));
        // Its own file lost: notes and papers join again, nothing goes out as new, nothing copied beside.
        std::fs::remove_file(sealed_path(&desk.memory)).unwrap();
        let outcome = desk.exchange_with(&folder, &key, NOW + MINUTE, &dn, &["notes"]);
        assert_eq!(outcome.sent, 0, "{outcome:?}");
        assert_eq!(files_in(&dn).len(), 50);
        let _ = std::fs::remove_dir_all(&base);
    }

    // F24
    #[test]
    fn what_is_said_and_left_behind() {
        // Named alike in every language: one copy, not one per device.
        let at = NOW;
        assert_eq!(conflict_name("lease", at), conflict_name("lease", at));
        assert!(conflict_name("lease", at).starts_with("lease (conflict 20"));
        let base = scratch("left-behind");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let desk = Computer::new(&base, "desk");
        let dn = base.join("desk-notes");
        put(&dn, "lease.md", b"lease\n");
        // What a crash left half written, an hour old: cleaned; one being written: left.
        put(&dn, ".lease.md.1-1.sioul.tmp", b"half");
        dated(&dn.join(".lease.md.1-1.sioul.tmp"), std::time::SystemTime::now() - std::time::Duration::from_secs(7200));
        put(&dn, ".lease.md.1-2.sioul.tmp", b"being written");
        // A name that is not text, where the disk holds one (Linux's does; a Mac's refuses it, Windows' names are text).
        #[cfg(unix)]
        let latin = {
            use std::os::unix::ffi::OsStrExt;
            std::fs::write(dn.join(std::ffi::OsStr::from_bytes(b"caf\xe9.md")), b"latin-1").is_ok()
        };
        #[cfg(not(unix))]
        let latin = false;
        let outcome = desk.exchange_with(&folder, &key, NOW, &dn, &["notes"]);
        assert!(!dn.join(".lease.md.1-1.sioul.tmp").exists() && dn.join(".lease.md.1-2.sioul.tmp").exists());
        assert_eq!(outcome.problems.iter().any(|p| p.starts_with("share-not-text:files/notes/caf")), latin, "{outcome:?}");
        assert_eq!(outcome.sent, 1, "{outcome:?}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn each_device_chooses_its_parts() {
        let roots = Roots { config: PathBuf::from("config"), data: PathBuf::from("data"), state: PathBuf::from("state") };
        let config = Config { notes_root: Some("/notes".into()), ..Config::default() };
        let parts = |stores: &[Store]| stores.iter().map(|s| s.part).collect::<BTreeSet<_>>();
        // A device that never chose: what was shared before parts had switches.
        assert_eq!(parts(&stores(&config, &roots)), ["calls", "drafts", "health", "lists", "senders", "settings", "spam", "time"].into());
        let old = Config { share_projects: true, ..config.clone() };
        assert!(parts(&stores(&old, &roots)).contains("projects"), "projects as the old setting said");
        // Its own choices, kept in its own file, never in a shared one.
        let mut here = Here::default();
        here.parts.insert("notes".into(), true);
        here.parts.insert("health".into(), false);
        here.parts.insert("projects".into(), false);
        let chosen = stores_of(&old, &roots, &|part| here.shares(part, &old));
        assert!(chosen.iter().any(|s| s.name == "files/notes/") && !chosen.iter().any(|s| s.part == "health" || s.part == "projects"));
        let notes = chosen.iter().find(|s| s.name == "files/notes/").unwrap();
        assert!(["sioul-projects.toml", "sioul-cases.toml", "sioul-cases.toml.before-rename", "sioul-papers.toml", "papers"].iter().all(|n| notes.skip.contains(n)), "what other parts carry stays out of the notes");
        let back: Here = toml::from_str(&toml::to_string(&here).unwrap()).unwrap();
        assert_eq!(back.parts, here.parts);
        assert!(stores_of(&config, &roots, &|_| true).iter().all(|s| s.name != "state/share/here.toml"));
        // Written into the notes folder, never what tools make nor what another part carries.
        let every = stores_of(&config, &roots, &|_| true);
        assert!(locate(&every, "files/notes/admin/lease.md").is_some());
        for file in ["files/notes/.obsidian/app.json", "files/notes/lease.md~", "files/notes/sioul-cases.toml", "files/notes/sioul-projects.toml", "files/notes/papers/id.pdf", "files/notes/a (conflicted copy).md"] {
            assert!(locate(&every, file).is_none(), "{file}");
        }
        assert_eq!(locate(&every, "files/papers/id.pdf").map(|(s, _)| s.part), Some("papers"));
        assert_eq!(shown("files/notes/admin/lease.md"), "admin/lease.md");
        assert_eq!(shown("files/papers/id.pdf"), "papers/id.pdf");
        assert_eq!(shown("config/config.toml"), "config.toml");
    }

    /// The spam filter's table: from the computer that trained it to the
    /// others, sealed apart, in every exchange (a phone's background step's
    /// too); the good one it replaces kept beside; never the language model
    /// nor the corpus beside it.
    #[test]
    fn the_spam_table_travels_alone_in_every_exchange() {
        use sioul_core::spam::features::{FEATURES, N};
        use sioul_core::spam::table::{self, Table};
        let base = scratch("spam-table");
        let folder = base.join("folder");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let made = |bias: f32| {
            let mut made = Table {
                tokenizer: sioul_core::spam::tokenize::TOKENIZER,
                features: FEATURES,
                dim: 2,
                minn: 3,
                maxn: 6,
                bucket: 4,
                words: vec![(table::word_hash("lotteri"), 2.0)],
                buckets: vec![0.0; 4],
                weights: vec![0.0; N],
                means: vec![0.0; N],
                bias,
                text_mean: 0.0,
                platt_a: -1.0,
                platt_b: 0.0,
                meta: table::Meta { device: "desk".into(), ..table::Meta::default() },
            };
            made.sort();
            made
        };
        // Each table its own time: a file is known by its size and time.
        let trained = |bias: f32, second: u64| {
            let path = desk.path("data/spam/table.bin");
            made(bias).write(&path).unwrap();
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(second)).unwrap();
        };
        // What a training leaves on the desk: the table, the language model, the corpus.
        trained(0.5, 1_789_990_000);
        desk.write("data/spam/language.bin", "the vocabulary in plain text: Jane, the lease, the lottery");
        desk.write("data/spam/corpus/home/INBOX.jsonl.gz", "Subject: the lease");
        // Quick exchanges only (a dose's alarm, a phone's background step): it travels all the same.
        let quick = |c: &Computer, now: i64| exchange(&Sharing { folder: &folder, computer: &c.id, key: &key, memory: &c.memory, files: false, hurry: None }, &stores(&Config::default(), &c.roots), now).unwrap();
        quick(&desk, NOW);
        let came = quick(&phone, NOW + MINUTE);
        assert!(came.written.contains(SPAM_TABLE), "{came:?}");
        assert_eq!(std::fs::read(phone.path("data/spam/table.bin")).unwrap(), std::fs::read(desk.path("data/spam/table.bin")).unwrap());
        assert!(!phone.path("data/spam/language.bin").exists() && !phone.path("data/spam/corpus").exists());
        assert!(!table::previous(&phone.path("data/spam/table.bin")).exists(), "nothing before the first");
        // Sealed: nothing of the table, nor of what is beside it, readable in the folder.
        let mut files = Vec::new();
        list_files(&folder, &folder, &[], &mut files);
        for (name, path) in &files {
            let bytes = std::fs::read(path).unwrap();
            for plain in [&b"SIOULSPM"[..], b"vocabulary", b"lottery"] {
                assert!(!bytes.windows(plain.len()).any(|w| w == plain), "{name}");
            }
        }
        // A new training: the phone takes it, the good one before kept beside.
        trained(1.5, 1_789_990_600);
        quick(&desk, NOW + 2 * MINUTE);
        assert!(quick(&phone, NOW + 3 * MINUTE).written.contains(SPAM_TABLE));
        let path = phone.path("data/spam/table.bin");
        assert_eq!((Table::read(&path).unwrap().bias, Table::read(&table::previous(&path)).unwrap().bias), (1.5, 0.5));
        // The language model and the corpus have no place in the sharing.
        let every = stores_of(&Config::default(), &desk.roots, &|_| true);
        for file in ["files/spam/language.bin", "files/spam/corpus/home/INBOX.jsonl.gz", "files/spam/", "data/spam/language.bin"] {
            assert!(locate(&every, file).is_none(), "{file}");
        }
        assert_eq!(locate(&every, SPAM_TABLE).map(|(s, p)| (s.part, p)), Some(("spam", desk.path("data/spam/table.bin"))));
        assert_eq!(shown(SPAM_TABLE), "spam/table.bin");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Item 11 of 9 October: the spam filter's tables a computer seals, about
    /// one a week, are not kept 90 days each: past the newest two, its own
    /// older ones leave the folder at once; another device's file stays. A
    /// phone that read none of them for a while takes the newest and asks
    /// for nothing older: a device reads the newest record of a file only,
    /// so their absence is never missed, today's Sioul or an older one.
    #[test]
    fn only_the_two_newest_spam_tables_stay_in_the_folder() {
        let base = scratch("spam-tables");
        let folder = base.join("folder");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let quick = |c: &Computer, now: i64| exchange(&Sharing { folder: &folder, computer: &c.id, key: &key, memory: &c.memory, files: false, hurry: None }, &stores(&Config::default(), &c.roots), now).unwrap();
        // Another device's sealed file: never this computer's to take out.
        let stranger = folder.join("blobs").join("0f".repeat(32));
        put(&folder, &format!("blobs/{}", "0f".repeat(32)), b"another device's");
        let tables = || std::fs::read_dir(folder.join("blobs")).unwrap().filter_map(Result::ok).filter(|e| !e.file_name().to_string_lossy().starts_with('.') && e.path() != stranger).count();
        // A training: a table of its own content, and its own time.
        let trained = |n: u64| {
            let path = desk.path("data/spam/table.bin");
            put(path.parent().unwrap(), "table.bin", &noise(4096, n as u32));
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_789_990_000 + n * 600)).unwrap();
        };
        let mut now = NOW;
        for n in 1..=4 {
            trained(n);
            now += MINUTE;
            quick(&desk, now);
            now += MINUTE;
            let came = quick(&phone, now);
            assert!(came.problems.is_empty(), "{came:?}");
            assert_eq!(std::fs::read(phone.path("data/spam/table.bin")).unwrap(), noise(4096, n as u32));
            assert_eq!(tables(), n.min(2) as usize, "after training {n}");
        }
        assert_eq!(Memory::load_all(&desk.memory, &desk.id).sealed.tables.len(), 2);
        // The phone away for three trainings: back, it takes the newest, and nothing waits.
        for n in 5..=7 {
            trained(n);
            now += MINUTE;
            quick(&desk, now);
        }
        assert_eq!(tables(), 2);
        now += MINUTE;
        let came = quick(&phone, now);
        assert!(came.problems.is_empty() && came.pending == 0, "{came:?}");
        assert_eq!(std::fs::read(phone.path("data/spam/table.bin")).unwrap(), noise(4096, 7));
        let came = quick(&phone, now + 2 * DAY);
        assert!(came.problems.is_empty() && came.pending == 0, "nothing missing, a day on: {came:?}");
        assert!(stranger.exists(), "another device's file stays");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Each device writes its own label log; every device reads them all: a
    /// message said not spam on the phone is not spam on the desk (its
    /// `said_ham`), and what the phone's filter moved or flagged is known
    /// there, quick exchanges only. Sealed: no Message-ID readable in the folder.
    #[test]
    fn a_label_on_one_device_reaches_the_others() {
        use sioul_core::spam::labels::{self, Entry, Label, Moved, Source};
        let base = scratch("spam-labels");
        let folder = base.join("folder");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let quick = |c: &Computer, now: i64| exchange(&Sharing { folder: &folder, computer: &c.id, key: &key, memory: &c.memory, files: false, hurry: None }, &stores(&Config::default(), &c.roots), now).unwrap();
        let spam_state = |c: &Computer| c.roots.state.join("spam");
        // A message the desk's filter flagged, as the desk's Porch reads it.
        let mut card = sioul_core::card::Card::from_bytes(b"From: Prize <win@lottery.test>\r\nMessage-ID: <win-1@lottery.test>\r\nSubject: You won\r\n\r\nClaim it.\r\n").unwrap();
        card.account = Some("home".into());
        card.origin = Some(sioul_core::card::ImapOrigin { validity: 7, uid: 42 });
        assert!(!labels::said_ham_in(&labels::read_all_in(&spam_state(&desk)), &card));
        // "Not spam" on the phone; the phone's filter moved another message.
        let said = Entry { at: 1_791_360_000, account: "home".into(), folder: "INBOX".into(), uidvalidity: 7, uid: 42, message_id: Some("win-1@lottery.test".into()), label: Label::Ham, source: Source::NotSpam };
        labels::append_to(&labels::own_log(&spam_state(&phone), &phone.id), &said).unwrap();
        let moved = Moved { at: 1_791_360_100, account: "home".into(), folder: "INBOX".into(), uidvalidity: 7, uid: 43, message_id: Some("deal-2@shop.test".into()), class: sioul_core::spam::Class::Spam };
        labels::append_to(&labels::own_moved_log(&spam_state(&phone), &phone.id), &moved).unwrap();
        // And flagged a third where it is, as maybe spam: the training on the desk must know it.
        let flagged = Moved { at: 1_791_360_200, uid: 44, message_id: Some("flag-3@shop.test".into()), class: sioul_core::spam::Class::Unsure, ..moved.clone() };
        labels::append_to(&labels::own_flagged_log(&spam_state(&phone), &phone.id), &flagged).unwrap();
        quick(&phone, NOW);
        let came = quick(&desk, NOW + MINUTE);
        let theirs = format!("{SPAM_LABELS}{}.jsonl", phone.id);
        assert!(came.written.contains(SPAM_LABELS) && came.written.contains(SPAM_MOVED) && came.written.contains(SPAM_FLAGGED) && came.received == 3, "{came:?}");
        assert!(desk.roots.state.join("spam/labels").join(format!("{}.jsonl", phone.id)).exists(), "the phone's own file, beside the desk's");
        // On the desk: the phone's own file, read with the desk's own; the message not spam there too.
        assert!(labels::said_ham_in(&labels::read_all_in(&spam_state(&desk)), &card));
        assert_eq!(labels::read_moved_in(&spam_state(&desk)), vec![moved]);
        assert_eq!(labels::read_flagged_in(&spam_state(&desk)), vec![flagged]);
        // The desk says spam later, on its own log: the newest word wins on both devices.
        let later = Entry { at: 1_791_360_500, label: Label::Spam, source: Source::Junk, ..said.clone() };
        labels::append_to(&labels::own_log(&spam_state(&desk), &desk.id), &later).unwrap();
        quick(&desk, NOW + 2 * MINUTE);
        quick(&phone, NOW + 3 * MINUTE);
        for device in [&desk, &phone] {
            let all = labels::read_all_in(&spam_state(device));
            assert_eq!(all.len(), 2);
            assert!(!labels::said_ham_in(&all, &card));
        }
        // Each file has one writer: the phone's own log unchanged on the phone but for its own line.
        assert_eq!(labels::read_from::<Entry>(&labels::own_log(&spam_state(&phone), &phone.id)), vec![said]);
        // Sealed: nothing of them readable in the folder.
        let mut files = Vec::new();
        list_files(&folder, &folder, &[], &mut files);
        for (name, path) in &files {
            let bytes = std::fs::read(path).unwrap();
            for plain in [&b"win-1@lottery.test"[..], b"not-spam", b"deal-2", b"flag-3"] {
                assert!(!bytes.windows(plain.len()).any(|w| w == plain), "{name}");
            }
        }
        let every = stores_of(&Config::default(), &desk.roots, &|_| true);
        assert_eq!(locate(&every, &theirs).map(|(s, _)| s.part), Some("spam"));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A call a phone declined, as Java wrote it (fiction numbers only).
    fn declined(at: i64, key: &str) -> sioul_core::calls::Held {
        sioul_core::calls::Held { at, key: key.into(), number: key.into(), who: "stranger".into(), name: "Cabinet du Dr Martin".into(), column: "sleep".into(), why: "matrix".into(), ..sioul_core::calls::Held::default() }
    }

    /// The calls' folder of a device.
    fn calls_of(c: &Computer) -> PathBuf {
        c.roots.state.join(sioul_core::calls::FOLDER)
    }

    /// A phone copies Java's call into its own log; the desk hears of it,
    /// marks it seen, and the phone hears of that. Sealed in the folder,
    /// remembered beside `memory.json`, never kept as an earlier version or a
    /// first-share copy; with `files.json` lost, nothing goes out again.
    #[test]
    fn a_phone_s_calls_reach_the_desk_and_seen_comes_back() {
        use sioul_core::calls;
        let base = scratch("calls");
        let folder = base.join("folder");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let own = calls::own_file(&calls_of(&phone), calls::LOG, &phone.id);
        assert_eq!(calls::carry_into(&own, &[declined(NOW - 3_600_000, "+33199001234")], None, NOW).unwrap(), 1);
        phone.exchange(&folder, &key, NOW);
        let came = desk.exchange(&folder, &key, NOW + MINUTE);
        assert!(came.written.contains(CALLS_LOG), "{came:?}");
        let on_desk = calls::read_logs(&calls_of(&desk), NOW + MINUTE);
        assert_eq!(on_desk.iter().map(|h| (h.device.as_str(), h.key.as_str(), h.rang, h.name.as_str())).collect::<Vec<_>>(), [(phone.id.as_str(), "+33199001234", false, "Cabinet du Dr Martin")]);
        // Seen on the desk, in its own log: the phone hears of it.
        calls::mark_seen(&calls_of(&desk), &desk.id, &[on_desk[0].id()], NOW + 2 * MINUTE).unwrap();
        desk.exchange(&folder, &key, NOW + 3 * MINUTE);
        let came = phone.exchange(&folder, &key, NOW + 4 * MINUTE);
        assert!(came.written.contains(CALLS_SEEN), "{came:?}");
        assert!(calls::read_seen(&calls_of(&phone)).seen.contains(&on_desk[0].id()));
        // Each file has one writer: the phone never wrote a seen line, the desk no call.
        assert!(!calls::own_file(&calls_of(&phone), calls::SEEN_LOG, &phone.id).exists());
        assert!(!calls::own_file(&calls_of(&desk), calls::LOG, &desk.id).exists());
        // Sealed: nothing of them readable in the folder.
        let mut files = Vec::new();
        list_files(&folder, &folder, &[], &mut files);
        for (name, path) in &files {
            let bytes = std::fs::read(path).unwrap();
            for plain in [&b"199001234"[..], b"Cabinet", b"stranger"] {
                assert!(!bytes.windows(plain.len()).any(|w| w == plain), "{name}");
            }
        }
        let every = stores_of(&Config::default(), &desk.roots, &|_| true);
        assert_eq!(locate(&every, &format!("{CALLS_LOG}{}.jsonl", phone.id)).map(|(s, _)| s.part), Some("calls"));
        for c in [&desk, &phone] {
            // Remembered beside memory.json, never in it.
            let small = Memory::load(&c.memory, &c.id);
            assert!(small.entries.keys().chain(small.files.keys()).chain(small.pending.keys()).all(|k| !calls_store(k)), "memory.json stays small");
            assert!(Memory::load_all(&c.memory, &c.id).entries.keys().any(|k| calls_store(k)));
            // Neither an earlier version nor a first-share copy of a log of calls.
            let share = c.memory.parent().unwrap();
            let mut kept = Vec::new();
            list_files(share, share, &[], &mut kept);
            assert!(kept.iter().all(|(name, _)| !name.contains("calls")), "{kept:?}");
        }
        // files.json lost: the calls' stores join again, and nothing goes out again as new.
        std::fs::remove_file(sealed_path(&desk.memory)).unwrap();
        let again = desk.exchange(&folder, &key, NOW + 5 * MINUTE);
        assert_eq!(again.sent, 0, "{again:?}");
        assert_eq!(calls::read_logs(&calls_of(&desk), NOW + 5 * MINUTE).len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Two phones and a desk: each phone's log its own, every device reading both.
    #[test]
    fn several_phones_each_keep_their_own_log() {
        use sioul_core::calls;
        let base = scratch("calls-phones");
        let folder = base.join("folder");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, a, b) = (Computer::new(&base, "desk"), Computer::new(&base, "phone-a"), Computer::new(&base, "phone-b"));
        calls::carry_into(&calls::own_file(&calls_of(&a), calls::LOG, &a.id), &[declined(NOW - 7_200_000, "+33199001234")], None, NOW).unwrap();
        calls::carry_into(&calls::own_file(&calls_of(&b), calls::LOG, &b.id), &[declined(NOW - 3_600_000, "+33465710042")], None, NOW).unwrap();
        for (n, c) in [&a, &b, &desk, &a, &b].iter().enumerate() {
            c.exchange(&folder, &key, NOW + n as i64 * MINUTE);
        }
        for c in [&desk, &a, &b] {
            let heard = calls::read_logs(&calls_of(c), NOW + 10 * MINUTE);
            assert_eq!(heard.iter().map(|h| (h.device.clone(), h.key.clone())).collect::<Vec<_>>(), [(a.id.clone(), "+33199001234".to_string()), (b.id.clone(), "+33465710042".to_string())]);
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// An older Sioul knows neither of the calls' stores: their records wait,
    /// written nowhere, taken out nowhere; updated, it reads them.
    #[test]
    fn an_older_sioul_keeps_the_calls_waiting() {
        use sioul_core::calls;
        let base = scratch("calls-older");
        let folder = base.join("folder");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let own = calls::own_file(&calls_of(&phone), calls::LOG, &phone.id);
        calls::carry_into(&own, &[declined(NOW - 3_600_000, "+33199001234")], None, NOW).unwrap();
        phone.exchange(&folder, &key, NOW);
        // The desk's older Sioul: no store of calls, and none known.
        let older_stores = stores(&Config::default(), &desk.roots).into_iter().filter(|s| !calls_store(&s.name)).collect::<Vec<_>>();
        OLDER.with(|older| *older.borrow_mut() = vec![CALLS_LOG.to_string(), CALLS_SEEN.to_string()]);
        for n in 1..=3 {
            let outcome = exchange(&Sharing { folder: &folder, computer: &desk.id, key: &key, memory: &desk.memory, files: true, hurry: None }, &older_stores, NOW + n * MINUTE).unwrap();
            assert_eq!(outcome.pending, 1, "kept waiting: {outcome:?}");
        }
        assert!(!calls_of(&desk).exists(), "nothing written");
        phone.exchange(&folder, &key, NOW + 4 * MINUTE);
        assert_eq!(calls::read_held(&own).len(), 1, "nothing taken out");
        // Updated: it knows both stores, they join, and the call comes in.
        OLDER.with(|older| older.borrow_mut().clear());
        let came = desk.exchange(&folder, &key, NOW + 5 * MINUTE);
        assert!(came.written.contains(CALLS_LOG) && came.pending == 0, "{came:?}");
        assert_eq!(calls::read_logs(&calls_of(&desk), NOW + 5 * MINUTE).len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The part switched off on the desk: nothing of it comes, nothing waits.
    #[test]
    fn the_calls_part_switched_off_receives_nothing_and_keeps_nothing_waiting() {
        use sioul_core::calls;
        let base = scratch("calls-off");
        let folder = base.join("folder");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        calls::carry_into(&calls::own_file(&calls_of(&phone), calls::LOG, &phone.id), &[declined(NOW - 3_600_000, "+33199001234")], None, NOW).unwrap();
        phone.exchange(&folder, &key, NOW);
        let off = stores_of(&Config::default(), &desk.roots, &|part| part != "calls" && shared_by_default(part, &Config::default()));
        let outcome = exchange(&Sharing { folder: &folder, computer: &desk.id, key: &key, memory: &desk.memory, files: true, hurry: None }, &off, NOW + MINUTE).unwrap();
        assert_eq!(outcome.pending, 0, "{outcome:?}");
        assert!(!calls_of(&desk).exists());
        // Switched on again, it joins and reads them then.
        let came = desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert!(came.written.contains(CALLS_LOG), "{came:?}");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A month on, the phone takes its own line out: every device's copy loses it.
    #[test]
    fn a_trimmed_line_leaves_every_device() {
        use sioul_core::calls;
        let base = scratch("calls-trim");
        let folder = base.join("folder");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let own = calls::own_file(&calls_of(&phone), calls::LOG, &phone.id);
        calls::carry_into(&own, &[declined(NOW - 3_600_000, "+33199001234")], None, NOW).unwrap();
        phone.exchange(&folder, &key, NOW);
        desk.exchange(&folder, &key, NOW + MINUTE);
        assert_eq!(calls::read_held(&calls::own_file(&calls_of(&desk), calls::LOG, &phone.id)).len(), 1);
        let later = NOW + (calls::KEPT_DAYS + 1) * 86_400_000;
        assert_eq!(calls::trim_own(&calls_of(&phone), &phone.id, later).unwrap(), 1);
        phone.exchange(&folder, &key, later);
        desk.exchange(&folder, &key, later + MINUTE);
        assert!(calls::read_held(&calls::own_file(&calls_of(&desk), calls::LOG, &phone.id)).is_empty(), "gone from the desk's copy too");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A message a phone's notification brought (fiction numbers only).
    fn texted(at: i64) -> sioul_core::phonemsgs::Line {
        sioul_core::phonemsgs::Line {
            at,
            id: format!("{at}-0123456789abcdef"),
            app: "foundation.e.message".into(),
            label: "Message".into(),
            kind: "text".into(),
            name: "Cabinet du Dr Martin".into(),
            key: "+33199001234".into(),
            who: "neutral".into(),
            text: "Votre rendez-vous est déplacé à jeudi 10 h 30.".into(),
            shows: at,
            ..sioul_core::phonemsgs::Line::default()
        }
    }

    /// The phones' messages' folder of a device.
    fn messages_of(c: &Computer) -> PathBuf {
        c.roots.state.join(sioul_core::phonemsgs::FOLDER)
    }

    /// A phone's message reaches the desk, both with the part on: sealed and
    /// padded in the folder, remembered beside `memory.json`, never kept as an
    /// earlier version; Seen on the desk comes back to the phone.
    #[test]
    fn a_phone_s_messages_reach_the_desk_sealed_and_seen_comes_back() {
        use sioul_core::{calls, phonemsgs};
        let base = scratch("phone-messages");
        let (folder, notes) = (base.join("folder"), base.join("notes"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let on = [phonemsgs::PART];
        let own = calls::own_file(&messages_of(&phone), phonemsgs::LOG, &phone.id);
        assert_eq!(phonemsgs::carry_into(&own, &[texted(NOW - 3_600_000)], NOW).unwrap(), 1);
        assert!(std::fs::read_to_string(&own).unwrap().lines().all(|l| l.len() % phonemsgs::PAD_STEP == 0), "padded");
        phone.exchange_with(&folder, &key, NOW, &notes, &on);
        let came = desk.exchange_with(&folder, &key, NOW + MINUTE, &notes, &on);
        assert!(came.written.contains(PHONE_MESSAGES_LOG), "{came:?}");
        let on_desk = phonemsgs::read_logs(&messages_of(&desk), NOW + MINUTE);
        assert_eq!(on_desk.iter().map(|l| (l.device.as_str(), l.key.as_str(), l.text.as_str())).collect::<Vec<_>>(), [(phone.id.as_str(), "+33199001234", "Votre rendez-vous est déplacé à jeudi 10 h 30.")]);
        // Seen on the desk, in its own log: the phone hears of it.
        calls::mark_seen(&messages_of(&desk), &desk.id, &[on_desk[0].id.clone()], NOW + 2 * MINUTE).unwrap();
        desk.exchange_with(&folder, &key, NOW + 3 * MINUTE, &notes, &on);
        let came = phone.exchange_with(&folder, &key, NOW + 4 * MINUTE, &notes, &on);
        assert!(came.written.contains(PHONE_MESSAGES_SEEN), "{came:?}");
        assert!(calls::read_seen(&messages_of(&phone)).seen.contains(&on_desk[0].id));
        // Sealed: nothing of it readable in the folder.
        let mut files = Vec::new();
        list_files(&folder, &folder, &[], &mut files);
        for (name, path) in &files {
            let bytes = std::fs::read(path).unwrap();
            for plain in [&b"199001234"[..], b"Cabinet", b"rendez-vous", b"neutral"] {
                assert!(!bytes.windows(plain.len()).any(|w| w == plain), "{name}");
            }
        }
        let every = stores_of(&Config::default(), &desk.roots, &|_| true);
        assert_eq!(locate(&every, &format!("{PHONE_MESSAGES_LOG}{}.jsonl", phone.id)).map(|(s, _)| s.part), Some(phonemsgs::PART));
        for c in [&desk, &phone] {
            let small = Memory::load(&c.memory, &c.id);
            assert!(small.entries.keys().chain(small.files.keys()).chain(small.pending.keys()).all(|k| !calls_store(k)), "memory.json stays small");
            let share = c.memory.parent().unwrap();
            let mut kept = Vec::new();
            list_files(share, share, &[], &mut kept);
            assert!(kept.iter().all(|(name, _)| !name.contains("phone-messages")), "{kept:?}");
        }
        // files.json lost: nothing goes out again as new.
        std::fs::remove_file(sealed_path(&desk.memory)).unwrap();
        let again = desk.exchange_with(&folder, &key, NOW + 5 * MINUTE, &notes, &on);
        assert_eq!(again.sent, 0, "{again:?}");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The part is off until turned on, on every device: a desk that never
    /// turned it on receives nothing and keeps nothing waiting; turned on, it
    /// reads them. A phone that never turned it on sends nothing.
    #[test]
    fn the_phone_messages_part_is_off_until_turned_on_and_off_receives_nothing() {
        use sioul_core::{calls, phonemsgs};
        let base = scratch("phone-messages-off");
        let (folder, notes) = (base.join("folder"), base.join("notes"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        assert!(!shared_by_default(phonemsgs::PART, &Config::default()));
        assert!(stores(&Config::default(), &desk.roots).iter().all(|s| s.part != phonemsgs::PART));
        phonemsgs::carry_into(&calls::own_file(&messages_of(&phone), phonemsgs::LOG, &phone.id), &[texted(NOW - 3_600_000)], NOW).unwrap();
        // The phone's part off: nothing of it leaves.
        phone.exchange(&folder, &key, NOW);
        desk.exchange_with(&folder, &key, NOW + MINUTE, &notes, &[phonemsgs::PART]);
        assert!(phonemsgs::read_logs(&messages_of(&desk), NOW + MINUTE).is_empty(), "the phone's part off");
        // The phone's on, the desk's off: nothing comes, nothing waits.
        phone.exchange_with(&folder, &key, NOW + 2 * MINUTE, &notes, &[phonemsgs::PART]);
        let outcome = desk.exchange(&folder, &key, NOW + 3 * MINUTE);
        assert_eq!(outcome.pending, 0, "{outcome:?}");
        assert!(!messages_of(&desk).join(phonemsgs::LOG).exists());
        // Turned on on the desk: it joins and reads it.
        let came = desk.exchange_with(&folder, &key, NOW + 4 * MINUTE, &notes, &[phonemsgs::PART]);
        assert!(came.written.contains(PHONE_MESSAGES_LOG), "{came:?}");
        assert_eq!(phonemsgs::read_logs(&messages_of(&desk), NOW + 4 * MINUTE).len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A week on, the phone takes its own line out: every device's copy loses it.
    #[test]
    fn a_trimmed_line_of_phone_messages_leaves_every_device() {
        use sioul_core::{calls, phonemsgs};
        let base = scratch("phone-messages-trim");
        let (folder, notes) = (base.join("folder"), base.join("notes"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let on = [phonemsgs::PART];
        let own = calls::own_file(&messages_of(&phone), phonemsgs::LOG, &phone.id);
        phonemsgs::carry_into(&own, &[texted(NOW - 3_600_000)], NOW).unwrap();
        phone.exchange_with(&folder, &key, NOW, &notes, &on);
        desk.exchange_with(&folder, &key, NOW + MINUTE, &notes, &on);
        let theirs = calls::own_file(&messages_of(&desk), phonemsgs::LOG, &phone.id);
        assert_eq!(phonemsgs::read_file(&theirs).len(), 1);
        let later = NOW + phonemsgs::KEPT_DAYS * 86_400_000;
        assert_eq!(phonemsgs::trim_own(&messages_of(&phone), &phone.id, later).unwrap(), 1);
        phone.exchange_with(&folder, &key, later, &notes, &on);
        desk.exchange_with(&folder, &key, later + MINUTE, &notes, &on);
        assert!(phonemsgs::read_file(&theirs).is_empty(), "gone from the desk's copy too");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// An older Sioul knows neither of the phones' messages' stores: their
    /// records wait, written nowhere; updated, it reads them.
    #[test]
    fn an_older_sioul_keeps_the_phone_s_messages_waiting() {
        use sioul_core::{calls, phonemsgs};
        let base = scratch("phone-messages-older");
        let (folder, notes) = (base.join("folder"), base.join("notes"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let on = [phonemsgs::PART];
        phonemsgs::carry_into(&calls::own_file(&messages_of(&phone), phonemsgs::LOG, &phone.id), &[texted(NOW - 3_600_000)], NOW).unwrap();
        phone.exchange_with(&folder, &key, NOW, &notes, &on);
        // The desk's older Sioul: no store of the phones' messages, and none known.
        let older_stores = desk.stores_with(&notes, &|part| part == phonemsgs::PART).into_iter().filter(|s| s.part != phonemsgs::PART).collect::<Vec<_>>();
        OLDER.with(|older| *older.borrow_mut() = vec![PHONE_MESSAGES_LOG.to_string(), PHONE_MESSAGES_SEEN.to_string()]);
        for n in 1..=2 {
            let outcome = exchange(&Sharing { folder: &folder, computer: &desk.id, key: &key, memory: &desk.memory, files: true, hurry: None }, &older_stores, NOW + n * MINUTE).unwrap();
            assert_eq!(outcome.pending, 1, "kept waiting: {outcome:?}");
        }
        assert!(!messages_of(&desk).exists(), "nothing written");
        OLDER.with(|older| older.borrow_mut().clear());
        let came = desk.exchange_with(&folder, &key, NOW + 3 * MINUTE, &notes, &on);
        assert!(came.written.contains(PHONE_MESSAGES_LOG) && came.pending == 0, "{came:?}");
        let _ = std::fs::remove_dir_all(&base);
    }

    // ---------------------------------------------------------------- texts (SMS phase b)

    /// The texts' folder of a device (in its data folder).
    fn texts_of(c: &Computer) -> PathBuf {
        c.roots.data.join(sioul_core::texts::FOLDER)
    }

    /// A text a computer asks the phone to send (fiction numbers only).
    fn to_send(key: &str, phone: &str, written: i64) -> sioul_core::texts::Request {
        sioul_core::texts::Request { key: key.into(), phone: phone.into(), to: "01 99 00 12 34".into(), body: "J'arrive dans dix minutes.".into(), sub: -1, written, ahead: 0, again: String::new(), device: String::new() }
    }

    /// The phone's step as the app takes it: every request read, decided,
    /// claimed in the ledger before it is "sent", its outcome written.
    fn phone_step(phone: &Computer, ledger: &Path, seal: &crate::textseal::TextSeal, now: i64) -> Vec<String> {
        use sioul_core::texts;
        let book = texts::Ledger::open(ledger, now).unwrap();
        let mut sent = Vec::new();
        for r in texts::read_requests(&texts_of(phone), seal) {
            if texts::verdict(&r, &phone.id, now, &book, None) == texts::Verdict::Send {
                texts::record(ledger, &texts::Claim { key: r.key.clone(), at: now, state: "claimed".into(), ..texts::Claim::default() }).unwrap();
                let outcome = texts::Outcome { key: r.key.clone(), state: "sent".into(), at: now, row: "sms-412".into(), ..texts::Outcome::default() };
                texts::append(&texts::own_file(&texts_of(phone), texts::OUTCOME, &phone.id), &[outcome], seal, None).unwrap();
                sent.push(r.key);
            }
        }
        sent
    }

    /// A text written on the desk reaches the phone, which sends it once and
    /// says so; the desk hears it. Sealed in the folder and at rest on each
    /// device, remembered beside `memory.json`; read again after `files.json`
    /// is lost, it is never sent twice.
    #[test]
    fn a_text_written_on_the_desk_is_sent_once_by_the_phone_and_its_outcome_comes_back() {
        use sioul_core::texts::{self, Sealer};
        let base = scratch("texts");
        let (folder, notes, ledger) = (base.join("folder"), base.join("notes"), base.join("phone-private"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let seal = crate::textseal::TextSeal::new(&key);
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let on = [texts::PART];
        texts::Ledger::open(&ledger, NOW - MINUTE).unwrap();
        let request = to_send("0123456789abcdef0123456789abcdef", &phone.id, NOW);
        texts::append(&texts::own_file(&texts_of(&desk), texts::SEND, &desk.id), std::slice::from_ref(&request), &seal, None).unwrap();
        desk.exchange_with(&folder, &key, NOW, &notes, &on);
        let came = phone.exchange_with(&folder, &key, NOW + MINUTE, &notes, &on);
        assert!(came.written.contains(TEXTS_SEND), "{came:?}");
        assert_eq!(phone_step(&phone, &ledger, &seal, NOW + MINUTE), [request.key.clone()]);
        phone.exchange_with(&folder, &key, NOW + 2 * MINUTE, &notes, &on);
        let came = desk.exchange_with(&folder, &key, NOW + 3 * MINUTE, &notes, &on);
        assert!(came.written.contains(TEXTS_OUTCOME), "{came:?}");
        let outcomes = texts::read_outcomes(&texts_of(&desk), &seal);
        assert_eq!(texts::state_of(&request.key, &outcomes).map(|o| o.state.as_str()), Some("sent"));
        // Sealed in the folder, and at rest on both devices.
        for root in [&folder, &desk.roots.data, &phone.roots.data] {
            let mut files = Vec::new();
            list_files(root, root, &[], &mut files);
            for (name, path) in &files {
                let bytes = std::fs::read(path).unwrap();
                for plain in [&b"199001234"[..], b"dix minutes", b"sms-412"] {
                    assert!(!bytes.windows(plain.len()).any(|w| w == plain), "{name}");
                }
            }
        }
        assert!(seal.open(std::fs::read_to_string(texts::own_file(&texts_of(&phone), texts::SEND, &desk.id)).unwrap().lines().next().unwrap()).is_some());
        for c in [&desk, &phone] {
            let small = Memory::load(&c.memory, &c.id);
            assert!(small.entries.keys().chain(small.files.keys()).all(|k| !calls_store(k)), "memory.json stays small");
        }
        // The phone's files.json lost: the request read again, never sent again.
        std::fs::remove_file(sealed_path(&phone.memory)).unwrap();
        phone.exchange_with(&folder, &key, NOW + 4 * MINUTE, &notes, &on);
        assert!(phone_step(&phone, &ledger, &seal, NOW + 4 * MINUTE).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The part is off until turned on: nothing of the texts travels from a
    /// device that never turned it on, and a desk with it off receives nothing.
    #[test]
    fn the_texts_part_is_off_until_turned_on() {
        use sioul_core::texts;
        assert!(!shared_by_default(texts::PART, &Config::default()));
        let base = scratch("texts-off");
        let (folder, notes) = (base.join("folder"), base.join("notes"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let seal = crate::textseal::TextSeal::new(&key);
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        assert!(stores(&Config::default(), &desk.roots).iter().all(|s| s.part != texts::PART));
        let text = texts::Text { id: "sms-1".into(), at: NOW - MINUTE, direction: "in".into(), with: vec!["+33199001234".into()], body: "Bonjour".into(), ..texts::Text::default() };
        texts::append(&texts::own_file(&texts_of(&phone), texts::LOG, &phone.id), &[text], &seal, None).unwrap();
        phone.exchange_with(&folder, &key, NOW, &notes, &[texts::PART]);
        let outcome = desk.exchange(&folder, &key, NOW + MINUTE);
        assert_eq!(outcome.pending, 0, "{outcome:?}");
        assert!(!texts_of(&desk).exists());
        let came = desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &notes, &[texts::PART]);
        assert!(came.written.contains(TEXTS_LOG), "{came:?}");
        assert_eq!(texts::read_logs(&texts_of(&desk), &seal).len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The archive only grows: a year on, nothing is trimmed by age; a text
    /// deleted on the phone gets a line of its own, and every device keeps
    /// the text, marked.
    #[test]
    fn the_texts_archive_only_grows_and_a_deletion_is_marked_everywhere() {
        use sioul_core::texts;
        let base = scratch("texts-archive");
        let (folder, notes) = (base.join("folder"), base.join("notes"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let seal = crate::textseal::TextSeal::new(&key);
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let on = [texts::PART];
        let own = texts::own_file(&texts_of(&phone), texts::LOG, &phone.id);
        let id = |t: &texts::Text| t.line_id();
        let text = texts::Text { id: "sms-1".into(), at: NOW - 3 * 365 * 86_400_000, direction: "in".into(), with: vec!["+33199001234".into()], body: "Bonjour".into(), ..texts::Text::default() };
        texts::append(&own, std::slice::from_ref(&text), &seal, Some(&id)).unwrap();
        phone.exchange_with(&folder, &key, NOW, &notes, &on);
        desk.exchange_with(&folder, &key, NOW + MINUTE, &notes, &on);
        let later = NOW + 400 * 86_400_000;
        assert_eq!(texts::trim_own(&texts_of(&phone), &phone.id, &seal, later).unwrap() + texts::trim_others(&texts_of(&desk), &desk.id, &seal, later).unwrap(), 0);
        texts::append(&own, &[texts::Text { deleted: later, body: String::new(), ..text }], &seal, Some(&id)).unwrap();
        phone.exchange_with(&folder, &key, later, &notes, &on);
        desk.exchange_with(&folder, &key, later + MINUTE, &notes, &on);
        let kept = texts::read_logs(&texts_of(&desk), &seal);
        assert_eq!(kept.iter().map(|t| (t.body.as_str(), t.deleted > 0)).collect::<Vec<_>>(), [("Bonjour", true)]);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The first import of a whole history goes a batch at a time (500 texts
    /// and their media each step); every exchange in between still carries
    /// the dose marked and do-not-disturb pressed just before it, and the
    /// media go as blobs, never in the records.
    #[test]
    fn the_first_texts_build_s_records_are_passed_over() {
        // texts: its stores were in the state folder; left aside, never kept waiting, and written nowhere.
        let key = |folder: &str| format!("{folder}phone.jsonl#a line");
        assert!(retired(&key("state/texts/log/")));
        assert!(!retired(&key(TEXTS_LOG)));
    }

    /// A computer holding ten thousand texts (the owner's, 8 October 2026:
    /// 9,833 and a 25 MB `files.json` rewritten at each exchange): an idle
    /// exchange writes nothing of its memory; what it keeps of them is measured.
    #[test]
    fn an_idle_exchange_writes_nothing_with_ten_thousand_texts() {
        use sioul_core::texts;
        let base = scratch("texts-idle");
        let (folder, notes) = (base.join("folder"), base.join("notes"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let seal = crate::textseal::TextSeal::new(&key);
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let on = [texts::PART];
        let own = texts::own_file(&texts_of(&phone), texts::LOG, &phone.id);
        for batch in 0..20 {
            let lines: Vec<texts::Text> = (0..texts::BATCH)
                .map(|i| {
                    let n = batch * texts::BATCH + i;
                    texts::Text { id: format!("sms-{n}"), at: NOW - 400 * 86_400_000 + n as i64 * MINUTE, direction: "in".into(), with: vec!["+33199001234".into()], body: format!("Message {n}, about as long as a text usually is, give or take."), ..texts::Text::default() }
                })
                .collect();
            texts::append(&own, &lines, &seal, None).unwrap();
        }
        // The desk's own sealed file too (its spam filter's table): its time
        // of use was set at every exchange, and the 25 MB file written again.
        desk.write("data/spam/table.bin", "a table of the desk's training");
        phone.exchange_with(&folder, &key, NOW, &notes, &on);
        desk.exchange_with(&folder, &key, NOW + MINUTE, &notes, &on);
        desk.exchange_with(&folder, &key, NOW + 2 * MINUTE, &notes, &on);
        assert!(!Memory::load_all(&desk.memory, &desk.id).sealed.mine.is_empty(), "the desk sealed its table");
        let sealed = sealed_path(&desk.memory);
        let (size, memory_size) = (std::fs::metadata(&sealed).unwrap().len(), std::fs::metadata(&desk.memory).unwrap().len());
        let stamp = |p: &Path| std::fs::metadata(p).unwrap().modified().unwrap();
        let before = stamp(&sealed);
        std::thread::sleep(std::time::Duration::from_millis(20));
        // Half an hour without a change: the sealed files looked for, the history tidied, nothing of it written.
        for minute in 3..33 {
            desk.exchange_with(&folder, &key, NOW + minute * MINUTE, &notes, &on);
        }
        eprintln!("ten thousand texts: files.json {size} bytes, memory.json {memory_size} bytes; the phone's log {} bytes", std::fs::metadata(&own).unwrap().len());
        assert_eq!(stamp(&sealed), before, "files.json rewritten by idle exchanges");
        assert!(std::fs::metadata(&desk.memory).unwrap().len() < 64 << 10, "memory.json stays small");
        assert!(size < 2 * std::fs::metadata(&own).unwrap().len(), "the texts' lines not kept twice: {size}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_first_import_goes_in_batches_and_never_holds_back_the_doses() {
        use sioul_core::texts;
        let base = scratch("texts-import");
        let (folder, notes) = (base.join("folder"), base.join("notes"));
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let seal = crate::textseal::TextSeal::new(&key);
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let on = [texts::PART];
        let own = texts::own_file(&texts_of(&phone), texts::LOG, &phone.id);
        let picture = vec![7u8; 512 * 1024];
        let source = base.join("picture.bin");
        std::fs::write(&source, &picture).unwrap();
        let (hash, size) = crate::blobs::hash_file(&source).unwrap();
        for batch in 0..6 {
            let now = NOW + batch * 10 * MINUTE;
            // A dose taken and do-not-disturb pressed on the phone, then the import's next step.
            phone.write("state/health-state.toml", &format!("[taken]\n\"dose@{batch}\" = {}\n", now / 1000));
            phone.write("state/do-not-disturb.toml", &format!("[device.{}]\npressed = {now}\n", phone.id));
            let lines: Vec<texts::Text> = (0..texts::BATCH)
                .map(|i| {
                    let n = batch as usize * texts::BATCH + i;
                    let mut text = texts::Text { id: format!("sms-{n}"), at: NOW - 400 * 86_400_000 + n as i64 * MINUTE, direction: "in".into(), with: vec!["+33199001234".into()], body: format!("Message {n}"), ..texts::Text::default() };
                    if i == 0 {
                        text.parts = vec![texts::Part { seq: 0, ct: "image/png".into(), size, hash: hash.clone(), state: "here".into(), ..texts::Part::default() }];
                    }
                    text
                })
                .collect();
            crate::blobs::put(&folder, &key, &source, &hash).unwrap();
            texts::append(&own, &lines, &seal, None).unwrap();
            let sent = phone.exchange_with(&folder, &key, now, &notes, &on);
            assert!(sent.sent <= texts::BATCH + 10, "a batch at a time: {sent:?}");
            desk.exchange_with(&folder, &key, now + MINUTE, &notes, &on);
            // On time: the dose and the press of this very step are on the desk.
            assert!(desk.read("state/health-state.toml").contains(&format!("dose@{batch}")), "batch {batch}");
            assert!(desk.read("state/do-not-disturb.toml").contains(&now.to_string()), "batch {batch}");
            assert_eq!(texts::read_logs(&texts_of(&desk), &seal).len(), (batch as usize + 1) * texts::BATCH);
        }
        // The media: a blob in the folder, never in the records.
        let mut files = Vec::new();
        list_files(&folder, &folder, &[], &mut files);
        let records: u64 = files.iter().filter(|(name, _)| !name.starts_with("blobs")).filter_map(|(_, p)| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
        assert!(records < 6 * texts::BATCH as u64 * 2048, "the records hold lines, not media: {records} bytes");
        let target = base.join("fetched.bin");
        crate::blobs::get(&folder, &key, &hash, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), picture);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn toml_entries_and_back() {
        let rules = &CONFIG_RULES;
        let text = "# mine\nlanguage = \"fr\" # kept\ncase_store = \"~/Notes\"\n\n[quiet]\npersonal = [\"joy\", \"family\"]\n\n[[account]]\nid = \"a\"\nmaildir = \"/x\"\n";
        let entries = toml_entries(text, rules).unwrap();
        let names: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"language") && names.contains(&"quiet\u{1f}personal") && names.contains(&"account\u{1f}\u{1e}a"), "{names:?}");
        assert!(!names.iter().any(|n| n.starts_with("case_store")));
        // Writing keeps the rest of the file as written.
        let account = entries.iter().find(|(k, _)| k.starts_with("account")).unwrap().1.replace("id = \"a\"", "id = \"a\"\nmuted = true");
        let new = toml_write(text, rules, &[("language", Some("v = \"en\"\n")), ("account\u{1f}\u{1e}a", Some(&account)), ("quiet\u{1f}work", Some("v = [\"client\"]\n"))], false).unwrap();
        assert!(new.starts_with("# mine\nlanguage = \"en\""), "{new}");
        assert!(new.contains("muted = true") && new.contains("maildir = \"/x\""), "{new}");
        assert!(new.contains("work = [\"client\"]"), "{new}");
        // A list of words, one at a time.
        let money = "ignored = [\"a\", \"b\"]\n";
        let new = toml_write(money, &MONEY_RULES, &[("ignored\u{1f}\u{1e}v = \"a\"\n", None), ("ignored\u{1f}\u{1e}v = \"c\"\n", Some("v = \"c\"\n"))], false).unwrap();
        assert_eq!(new, "ignored = [\"b\", \"c\"]\n");
    }

    /// A medicine's takes and their own amounts, its generic name and its
    /// strength travel with it, one entry by
    /// its id (docs/health.md, "Kept as"): written into the file of a device
    /// whose copy has no amounts (an older Sioul's), they arrive whole, and the
    /// times every dose falls due at are the same.
    #[test]
    fn a_medicines_takes_travel_whole() {
        let new = "[[medicine]]\nid = \"iron\"\nname = \"Iron\"\ndose = \"08:00 · 1 tablet, 20:00 · 2 tablets\"\ngeneric = \"ferrous sulfate\"\nstrength = \"80 mg\"\nsince = 2026-09-01\n\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"20:00\"]\n\n[medicine.schedule.amounts]\n\"08:00\" = \"1 tablet\"\n\"20:00\" = \"2 tablets\"\n";
        let entries = toml_entries(new, &HEALTH_RULES).unwrap();
        assert_eq!(entries.len(), 1, "one entry for the medicine, its takes in it: {entries:?}");
        let (entry, value) = &entries[0];
        assert!(entry.starts_with("medicine") && value.contains("2 tablets"), "{entry} {value}");
        let old = "[[medicine]]\nid = \"iron\"\nname = \"Iron\"\ndose = \"1 tablet\"\n\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"20:00\"]\n";
        let written = toml_write(old, &HEALTH_RULES, &[(entry.as_str(), Some(value.as_str()))], false).unwrap();
        let health: sioul_core::health::Health = toml::from_str(&written).unwrap();
        let takes: Vec<(String, String)> = health.medicines[0].takes().into_iter().map(|t| (t.time, t.amount)).collect();
        assert_eq!(takes, [("08:00".to_string(), "1 tablet".to_string()), ("20:00".to_string(), "2 tablets".to_string())]);
        // Its generic name, strength and first day too.
        assert_eq!((health.medicines[0].generic.as_str(), health.medicines[0].strength.as_str(), health.medicines[0].since.map(|d| d.to_string())), ("ferrous sulfate", "80 mg", Some("2026-09-01".to_string())));
        let before: sioul_core::health::Health = toml::from_str(old).unwrap();
        let day = |h: &sioul_core::health::Health| {
            let start: jiff::Zoned = "2026-10-08T00:00[Europe/Paris]".parse().unwrap();
            let end: jiff::Zoned = "2026-10-09T00:00[Europe/Paris]".parse().unwrap();
            h.doses(&start, &end).into_iter().map(|d| (d.key, d.dose)).collect::<Vec<_>>()
        };
        let (now, then) = (day(&health), day(&before));
        assert_eq!(now.iter().map(|d| &d.0).collect::<Vec<_>>(), then.iter().map(|d| &d.0).collect::<Vec<_>>(), "the same doses, the same keys");
        assert_eq!(now.iter().map(|d| d.1.as_str()).collect::<Vec<_>>(), ["1 tablet", "2 tablets"]);
    }

    /// Do-not-disturb on every device (docs/do-not-disturb.md): each device's
    /// table of the switch travels whole and is never written by another; the
    /// list of people travels a person at a time; a Sioul that knows neither
    /// file shares the rest and takes nothing of them out.
    #[test]
    fn do_not_disturb_travels_each_device_its_table_and_the_list_a_person_at_a_time() {
        use sioul_core::everywhere::{People, Person, Switch, change_own, change_people};
        let base = scratch("dnd");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let switch = |c: &Computer| c.roots.state.join(sioul_core::everywhere::SWITCH_FILE);
        let people = |c: &Computer| c.roots.config.join(sioul_core::everywhere::PEOPLE_FILE);
        // The desk turns it on; each adds someone to the list, before hearing of the other.
        change_own(&switch(&desk), &desk.id, |s| {
            s.press(&desk.id, true, 0, NOW);
            true
        })
        .unwrap();
        change_people(&people(&phone), |p| {
            p.add(Person { name: "Alice".into(), phones: vec!["+262639981234".into()], ..Person::default() }, None);
        })
        .unwrap();
        change_people(&people(&desk), |p| {
            p.add(Person { name: "Bob".into(), emails: vec!["bob@example.org".into()], ..Person::default() }, None);
        })
        .unwrap();
        desk.exchange(&folder, &key, NOW + MINUTE);
        phone.exchange(&folder, &key, NOW + 2 * MINUTE);
        desk.exchange(&folder, &key, NOW + 3 * MINUTE);
        let heard = Switch::read(&switch(&phone)).unwrap();
        let latest = heard.latest().unwrap();
        assert!(latest.on && latest.from == desk.id, "{heard:?}");
        for c in [&desk, &phone] {
            let names: Vec<String> = People::read(&people(c)).unwrap().people.into_iter().map(|p| p.name).collect();
            assert!(names.contains(&"Alice".to_string()) && names.contains(&"Bob".to_string()), "{names:?}");
        }
        // The phone turns it off, having heard the desk: its own table only.
        change_own(&switch(&phone), &phone.id, |s| {
            s.press(&phone.id, false, 0, NOW + 4 * MINUTE);
            true
        })
        .unwrap();
        phone.exchange(&folder, &key, NOW + 5 * MINUTE);
        desk.exchange(&folder, &key, NOW + 6 * MINUTE);
        let seen = Switch::read(&switch(&desk)).unwrap();
        assert!(!seen.latest().unwrap().on, "{seen:?}");
        assert!(seen.device[&desk.id].on, "the desk's own table as it wrote it");
        // Someone taken off on the desk: gone on the phone too.
        let bob = People::read(&people(&desk)).unwrap().people.into_iter().find(|p| p.name == "Bob").unwrap().id;
        change_people(&people(&desk), |p| {
            p.remove(&bob);
        })
        .unwrap();
        desk.exchange(&folder, &key, NOW + 7 * MINUTE);
        phone.exchange(&folder, &key, NOW + 8 * MINUTE);
        assert_eq!(People::read(&people(&phone)).unwrap().people.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Alice"]);
        // A Sioul that knows neither file (an older one): writes neither, takes nothing out.
        let old = Computer::new(&base, "old");
        let older: Vec<Store> = stores(&Config::default(), &old.roots).into_iter().filter(|s| !s.name.contains("do-not-disturb") && !s.name.contains("dnd-people")).collect();
        exchange(&Sharing { folder: &folder, computer: &old.id, key: &key, memory: &old.memory, files: true, hurry: None }, &older, NOW + 9 * MINUTE).unwrap();
        assert!(!switch(&old).exists() && !people(&old).exists());
        desk.exchange(&folder, &key, NOW + 10 * MINUTE);
        assert_eq!(People::read(&people(&desk)).unwrap().people.len(), 1);
        assert_eq!(Switch::read(&switch(&desk)).unwrap().device.len(), 2);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A computer's table written as Sioul quits there (its inhibition ended
    /// with it) travels, and the phone reads it as closed, never silenced.
    #[test]
    fn a_computers_table_written_as_sioul_quits_reaches_the_phone_as_closed() {
        use sioul_core::everywhere::{Follows, Switch, change_own, others};
        let base = scratch("dnd-closed");
        let folder = base.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
        let switch = |c: &Computer| c.roots.state.join(sioul_core::everywhere::SWITCH_FILE);
        // The phone turns do-not-disturb on; the desk silences itself, says so, then Sioul quits there.
        change_own(&switch(&phone), &phone.id, |s| {
            s.press(&phone.id, true, 0, NOW);
            true
        })
        .unwrap();
        phone.exchange(&folder, &key, NOW + MINUTE);
        desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        change_own(&switch(&desk), &desk.id, |s| {
            let own = s.device.entry(desk.id.clone()).or_default();
            (own.kind, own.why, own.silenced, own.at) = ("computer".into(), "manual".into(), true, NOW + 2 * MINUTE);
            true
        })
        .unwrap();
        desk.exchange(&folder, &key, NOW + 3 * MINUTE);
        phone.exchange(&folder, &key, NOW + 4 * MINUTE);
        let read = |at: i64| {
            let s = Switch::read(&switch(&phone)).unwrap();
            others(&s, &phone.id, &[(desk.id.clone(), at / 1000)], at / 1000, NOW, &|_| true)
        };
        assert_eq!(read(NOW + 4 * MINUTE), vec![(desk.id.clone(), Follows::Silenced)]);
        // Sioul quits on the desk (the window closed, SIGTERM): its table as it then holds, sent.
        change_own(&switch(&desk), &desk.id, |s| {
            let own = s.device.get_mut(&desk.id).unwrap();
            own.closing(false);
            own.at = NOW + 5 * MINUTE;
            true
        })
        .unwrap();
        desk.exchange(&folder, &key, NOW + 5 * MINUTE);
        phone.exchange(&folder, &key, NOW + 6 * MINUTE);
        assert_eq!(read(NOW + 6 * MINUTE), vec![(desk.id.clone(), Follows::Closed)]);
        let table = &Switch::read(&switch(&phone)).unwrap().device[&desk.id];
        assert!(table.closed && !table.silenced, "{table:?}");
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
#[path = "share_format_tests.rs"]
mod format_tests;

#[cfg(test)]
mod probe {
    /// What a copy of real folders holds, in counts: `SIOUL_SHARE_PROBE=<dir with config, data, state>`.
    #[test]
    #[ignore]
    fn probe() {
        let Some(base) = std::env::var_os("SIOUL_SHARE_PROBE").map(std::path::PathBuf::from) else { return };
        let roots = super::Roots { config: base.join("config"), data: base.join("data"), state: base.join("state") };
        let config: sioul_core::config::Config = std::fs::read_to_string(roots.config.join("config.toml")).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        let stores = super::stores(&config, &roots);
        let look = super::Look { reads: &|_| true, hurry: &|| false, confirmed: &Default::default(), clock: super::clock_ms() };
        let found = super::gather(&stores, &Default::default(), &look);
        for store in &stores {
            let count = found.hashes.keys().filter(|k| super::file_of(k).starts_with(store.name.as_str()) && (store.folder || super::file_of(k) == store.name)).count();
            println!("{:32} {count}", store.name);
        }
        println!("unknown: {:?}", found.unknown);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The words Sioul looks for (docs/words.md): every list a recogniser
//! matches text against (one-time codes, bills, letters, folders, brands,
//! the spam filter's tokenizer…), out of the code and into data that you
//! can change.
//!
//! Three layers make each list:
//! 1. **language packs**, shipped: `crates/sioul-core/data/words/<language>.toml`,
//!    a file per language, as the `.ftl` files are;
//! 2. **country packs**, shipped: `data/words/countries/<code>.toml`, the
//!    names of a country (its public bodies, its brands, its banks'
//!    services), and `data/words/international.toml`, always in use: the
//!    words of mail programs, servers and apps in any language, brands of
//!    every country, shared mail providers;
//! 3. **your changes**, in config.toml under `[words]`: for each list, the
//!    words you added (`add`) and the shipped ones you took away (`remove`),
//!    never a copy of the list, so that a newer Sioul's words still reach you.
//!
//! A list as Sioul uses it: the words of the packs in use, less the
//! shipped words you took away, plus yours; compared folded (`text::fold`:
//! capitals and accents aside), each once, the first spelling kept. A word
//! both taken away and added is there: you put it back.
//!
//! The packs in use: the languages of `[words] languages`, else the
//! interface's and English; the countries of `[words] countries`, else
//! `[contacts] region`, else the locale's; and the international pack. A
//! newer pack on disk (`$XDG_DATA_HOME/sioul/words/`, then each of
//! `$XDG_DATA_DIRS`' `sioul/words/`) wins over the built-in one by its
//! `checked` date, as the site presets do (`presets.rs`): a distribution
//! updates them without building Sioul; your own changes never go there.
//!
//! ```toml
//! [words]
//! languages = ["fr", "en"]
//! countries = ["FR"]
//!
//! [words.codes.code]
//! add = ["Bestätigungscode"]
//! remove = ["code temporaire"]
//!
//! [words.brands.brands]
//! add = { "Ma Banque" = ["mabanque.example"] }
//! remove = ["Banque Exemple"]
//! ```

use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

/// Named lists, in their order: brands and their domains, bodies and the
/// words their letters carry, months by their number.
pub type Named = Vec<(String, Vec<String>)>;

/// The packs built into Sioul: their file, under `data/words/`, and their text.
const SHIPPED: &[(&str, &str)] = &[
    ("en.toml", include_str!("../data/words/en.toml")),
    ("fr.toml", include_str!("../data/words/fr.toml")),
    // The lists whose wording is public and stable; letters, papers, bank
    // exports and payments wait for real examples (docs/words.md).
    ("de.toml", include_str!("../data/words/de.toml")),
    ("es.toml", include_str!("../data/words/es.toml")),
    ("it.toml", include_str!("../data/words/it.toml")),
    ("countries/FR.toml", include_str!("../data/words/countries/FR.toml")),
    ("international.toml", include_str!("../data/words/international.toml")),
];

/// The lists that are named lists (`Named`): their entries are names, not words.
pub const MAPS: &[&str] = &["brands.brands", "letters.senders", "letters.months", "letters.numbers", "letters.fixed_delays", "capture.months"];

/// The languages read before the configuration says: Sioul's first two, as every list was until 8 October 2026.
pub const FIRST_LANGUAGES: [&str; 2] = ["fr", "en"];
/// The country whose names were read before the configuration said: France's.
pub const FIRST_COUNTRIES: [&str; 1] = ["FR"];

// ---------------------------------------------------------------- the lists

/// One-time codes, passwords, resets, sign-in links, addresses to confirm (`codes`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Codes {
    /// Names a code: a code right beside it is taken.
    pub code: Vec<String>,
    /// Points at a code without naming it: counts in a message about signing in.
    pub pointing: Vec<String>,
    /// Says the message is about signing in or checking who you are.
    pub sign_in: Vec<String>,
    /// Between "your" and "code", makes it no secret of yours ("your zip code").
    pub not_yours: Vec<String>,
    /// A shop's code: the message hands no secret over.
    pub promo: Vec<String>,
    pub password: Vec<String>,
    pub reset: Vec<String>,
    pub link: Vec<String>,
    /// What a sign-in link's message is about, in its subject or first lines.
    pub intents: Vec<String>,
    /// What may expire or work once: the link, its button, the code.
    pub link_words: Vec<String>,
    pub validity: Vec<String>,
    /// Lines answering a request of yours ("if you did not ask for this").
    pub request: Vec<String>,
    pub confirm: Vec<String>,
    /// Hands a code over, the code right after ("your code: K7Q-2M9").
    pub giving: Vec<String>,
    /// Whose code, a few words before the word for a code.
    pub owners: Vec<String>,
    /// Says a code is yours, right after it ("123456 is your code").
    pub owned_after: Vec<String>,
    /// May stand between a phrase and its code ("is").
    pub linking: Vec<String>,
    pub once: Vec<String>,
    /// Words before a validity ("valid 10 minutes").
    pub expiry_context: Vec<String>,
    /// The word for a code itself.
    pub noun: Vec<String>,
    /// Starts of the units of a validity, after its number.
    pub unit_minutes: Vec<String>,
    pub unit_hours: Vec<String>,
    pub unit_days: Vec<String>,
}

/// Other apps' notifications that hand a code over or ask for an approval now (`appnotes`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Approvals {
    pub phrases: Vec<String>,
    /// "Is this you?": counts beside what it is about (`asked_about`).
    pub asks: Vec<String>,
    pub asked_about: Vec<String>,
    /// Words in the name of a channel for codes or sign-ins.
    pub channels: Vec<String>,
}

/// Amounts in mail and letters (`money`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoneyWords {
    /// Words after which the amount a message is about comes.
    pub keywords: Vec<String>,
    /// Sentences stating a payment; the amount follows them.
    pub stated: Vec<String>,
    /// Labels of the line that holds what was paid.
    pub totals: Vec<String>,
    /// Labels of a subtotal, which is no total.
    pub subtotal: Vec<String>,
    /// Lines of a legal footer, whose amounts are never a payment.
    pub legal: Vec<String>,
}

/// Mail about money (`payments`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Payments {
    pub not_payments: Vec<String>,
    pub refund: Vec<String>,
    pub paid: Vec<String>,
    pub received: Vec<String>,
    pub order: Vec<String>,
    pub bill: Vec<String>,
    /// The label above who was paid, in a receipt.
    pub to: Vec<String>,
    /// The label above who paid, in a receipt.
    pub from: Vec<String>,
    /// Words before the party's name, after an amount ("de Jean").
    pub leads: Vec<String>,
    /// A subject naming an order and saying it is confirmed.
    pub order_words: Vec<String>,
    pub confirmed_words: Vec<String>,
}

/// A CSV export's columns, by their names (`bank`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BankColumns {
    pub date: Vec<String>,
    pub label: Vec<String>,
    pub name: Vec<String>,
    pub net: Vec<String>,
    pub amount: Vec<String>,
    pub debit: Vec<String>,
    pub credit: Vec<String>,
    pub kind: Vec<String>,
    pub status: Vec<String>,
    pub impact: Vec<String>,
    pub balance: Vec<String>,
    pub time: Vec<String>,
    pub currency: Vec<String>,
    pub id: Vec<String>,
}

/// Lines above a CSV export's header.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BankAbove {
    pub balance: Vec<String>,
    pub date: Vec<String>,
    pub account: Vec<String>,
}

/// Bank and payment-processor exports (`bank`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BankWords {
    /// Words that name nothing in a movement's label.
    pub filler: Vec<String>,
    pub columns: BankColumns,
    pub above: BankAbove,
    /// Statuses of rows left out.
    pub skipped: Vec<String>,
    /// A balance impact saying the row moves no money.
    pub memo: Vec<String>,
}

/// Money moved between your own accounts (`accounts`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountWords {
    pub between: Vec<String>,
}

/// What a letter is, the gravest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LetterKinds {
    pub formal_notice: Vec<String>,
    pub tax_notice: Vec<String>,
    pub decision: Vec<String>,
    /// With `decision`: a way of appeal.
    pub decision_appeal: Vec<String>,
    pub reminder: Vec<String>,
    pub appointment: Vec<String>,
    pub bill: Vec<String>,
    pub acknowledgment: Vec<String>,
    pub attestation: Vec<String>,
    pub contract: Vec<String>,
}

/// Paper letters, read by OCR (`letters`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LetterWords {
    /// Bodies that write to everyone: the name shown, the words their letters carry.
    pub senders: Named,
    pub kinds: LetterKinds,
    pub registered: Vec<String>,
    pub registered_receipt: Vec<String>,
    /// A line that asks for an amount.
    pub asks: Vec<String>,
    /// Month names, by the month's number.
    pub months: Named,
    pub ordinals: Vec<String>,
    /// Numbers written as words, by their value.
    pub numbers: Named,
    /// Words before a date by which something is asked.
    pub deadline: Vec<String>,
    pub delay_words: Vec<String>,
    pub delay_of: Vec<String>,
    pub unit_months: Vec<String>,
    pub unit_days: Vec<String>,
    pub unit_weeks: Vec<String>,
    /// Delays said in words, by their days.
    pub fixed_delays: Named,
    /// Says a delay counts from the letter's own date.
    pub from_letter: Vec<String>,
    /// The word before an appointment's time.
    pub at: Vec<String>,
    /// Labels of your number with them.
    pub references: Vec<String>,
}

/// The kinds of paper, by the words a file's name or a subject carries (`papers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaperKinds {
    pub passport: Vec<String>,
    pub identity: Vec<String>,
    pub residence: Vec<String>,
    pub driving: Vec<String>,
    pub health_cover: Vec<String>,
    pub health_card: Vec<String>,
    pub tax_notice: Vec<String>,
    pub rent_receipt: Vec<String>,
    pub bank_details: Vec<String>,
    pub payslip: Vec<String>,
    pub warranty: Vec<String>,
    pub insurance: Vec<String>,
    pub certificate: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaperWords {
    pub kinds: PaperKinds,
}

/// The kinds of contract, by the words a label, a subject or a sender carries (`contracts`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContractKinds {
    pub rent: Vec<String>,
    pub energy: Vec<String>,
    pub health: Vec<String>,
    pub insurance: Vec<String>,
    pub telecom: Vec<String>,
    pub hosting: Vec<String>,
    pub subscription: Vec<String>,
    pub bank: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContractWords {
    pub kinds: ContractKinds,
}

/// Automatic senders (`porch`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SenderWords {
    /// An address holding one of these is filed.
    pub automatic: Vec<String>,
}

/// Prefixes of a subject already answered or forwarded (`compose`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Replies {
    pub forward: Vec<String>,
    pub reply: Vec<String>,
}

/// A message's quoted history (`reading`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Quotes {
    /// Lines that open a forwarded or answered message.
    pub openings: Vec<String>,
    pub header_names: Vec<String>,
    /// The first header of a quoted block.
    pub from_names: Vec<String>,
    /// One of these makes a block of headers a quoted message.
    pub enough_names: Vec<String>,
    /// Endings of an attribution line ("Jane wrote:").
    pub wrote: Vec<String>,
    /// Starts of an attribution broken over two lines.
    pub attribution_starts: Vec<String>,
}

/// Mail folders by their names (`folders`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderWords {
    pub sent: Vec<String>,
    pub drafts: Vec<String>,
    pub junk: Vec<String>,
    pub trash: Vec<String>,
    pub archive: Vec<String>,
}

/// Senders who borrow a name (`lookalike`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrandWords {
    /// Brands and public services, and their domains.
    pub brands: Named,
    /// Mail providers whose domains millions share.
    pub shared: Vec<String>,
    /// Words of a service, which a fake name wraps a brand or your domain in
    /// ("PayPal Service", "janedoe.example Mail Admin"): an everyday brand's
    /// name beside these alone is borrowed (`lookalike::impersonation`).
    pub service_words: Vec<String>,
    /// The brands whose name is also an everyday word or a place ("Orange",
    /// "Apple", "La Poste"), by name: borrowed only alone or beside the words
    /// of a service; any other brand counts anywhere in a name.
    pub everyday: Vec<String>,
}

/// A site's notification that someone calls (`sites`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CallWords {
    pub missed: Vec<String>,
    pub ringing: Vec<String>,
}

/// Voicemail an operator sends by mail (`voicemail`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoicemailWords {
    /// The operators' domains: only their mail is read as voicemail.
    pub operators: Vec<String>,
    pub words: Vec<String>,
    pub hidden: Vec<String>,
    /// Words before the caller's number, matched as whole words.
    pub caller_leads: Vec<String>,
    pub own_line: Vec<String>,
    pub duration: Vec<String>,
}

/// A topic's words, for the shield.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShieldTopics {
    pub work: Vec<String>,
    pub support: Vec<String>,
    pub press: Vec<String>,
    pub thanks: Vec<String>,
    pub donation: Vec<String>,
}

/// The shield of a public address (`shield`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShieldWords {
    pub threats: Vec<String>,
    pub insults: Vec<String>,
    pub rude: Vec<String>,
    pub you: Vec<String>,
    pub topics: ShieldTopics,
}

/// Words of your tasks' categories.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskWords {
    /// Never proposed, never scheduled (`plan`).
    pub optional: Vec<String>,
    /// The Tasks page's own fold.
    pub joy: Vec<String>,
    /// Left out of Free time's offers when movement is off (`pause`).
    pub movement: Vec<String>,
    /// In view in quiet time, when `[quiet] personal` is unset.
    pub personal: Vec<String>,
    /// Work, never in view in quiet time, when `[quiet] work` is unset.
    pub work: Vec<String>,
    /// Leisure, among the personal ones (`areas`).
    pub leisure: Vec<String>,
    /// Yours, outside work, before your settings are read (`areas`).
    pub usual_personal: Vec<String>,
    /// A notes folder of these names holds the Pause's sounds.
    pub sounds_folders: Vec<String>,
}

/// The days of the week.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weekdays {
    pub monday: Vec<String>,
    pub tuesday: Vec<String>,
    pub wednesday: Vec<String>,
    pub thursday: Vec<String>,
    pub friday: Vec<String>,
    pub saturday: Vec<String>,
    pub sunday: Vec<String>,
}

/// Words by the kind of task they say (`tasks::KINDS`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskKinds {
    pub call: Vec<String>,
    pub write: Vec<String>,
    pub online: Vec<String>,
    pub out: Vec<String>,
    pub read: Vec<String>,
    pub think: Vec<String>,
    pub make: Vec<String>,
}

/// Quick capture (`capture`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureWords {
    pub weekdays: Weekdays,
    /// Month names, by the month's number.
    pub months: Named,
    /// Words after "@" that name a kind.
    pub kind_words: TaskKinds,
    /// A first word that says a kind plainly.
    pub kind_verbs: TaskKinds,
    pub today: Vec<String>,
    pub tomorrow: Vec<String>,
    pub next_week: Vec<String>,
    /// "in 3 days".
    #[serde(rename = "in")]
    pub in_: Vec<String>,
    pub unit_days: Vec<String>,
    pub unit_weeks: Vec<String>,
    pub ordinals: Vec<String>,
}

/// The spam filter's tokenizer (`spam::tokenize`). A table keeps the words it
/// was trained with, and reads every message with them (`spam::table::Meta`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct SpamWords {
    /// A provider's spam mark at the start of a subject, taken off first.
    pub provider_tags: Vec<String>,
    pub months: Vec<String>,
    pub keys: Vec<String>,
    pub elisions: Vec<String>,
    pub number_words: Vec<String>,
    pub stop_words: Vec<String>,
}

/// Reading scans.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OcrWords {
    /// Tesseract's names of the languages' models.
    pub tesseract: Vec<String>,
}

/// Every list Sioul looks for, as the packs in use and your changes make them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Words {
    pub codes: Codes,
    pub approvals: Approvals,
    pub money: MoneyWords,
    pub payments: Payments,
    pub bank: BankWords,
    pub accounts: AccountWords,
    pub letters: LetterWords,
    pub papers: PaperWords,
    pub contracts: ContractWords,
    pub senders: SenderWords,
    pub replies: Replies,
    pub quotes: Quotes,
    pub folders: FolderWords,
    pub brands: BrandWords,
    pub calls: CallWords,
    pub voicemail: VoicemailWords,
    pub shield: ShieldWords,
    pub tasks: TaskWords,
    pub capture: CaptureWords,
    pub spam: SpamWords,
    pub ocr: OcrWords,
    /// The languages whose packs made these words.
    #[serde(skip)]
    pub languages: Vec<String>,
    /// The countries whose packs made these words.
    #[serde(skip)]
    pub countries: Vec<String>,
}

// ---------------------------------------------------------------- the packs

/// A pack's lists, or a list's words: tables keep their order, as the file writes them.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    List(Vec<String>),
    Table(Vec<(String, Node)>),
}

impl Node {
    fn table() -> Node {
        Node::Table(Vec::new())
    }

    /// The node at a dotted path ("codes.code"), if there is one.
    pub fn get(&self, path: &str) -> Option<&Node> {
        path.split('.').filter(|p| !p.is_empty()).try_fold(self, |node, part| match node {
            Node::Table(entries) => entries.iter().find(|(k, _)| k == part).map(|(_, n)| n),
            Node::List(_) => None,
        })
    }

    /// The words of the list at a dotted path; none when there is no such list.
    pub fn list(&self, path: &str) -> &[String] {
        match self.get(path) {
            Some(Node::List(words)) => words,
            _ => &[],
        }
    }

    fn entry_mut(&mut self, key: &str) -> Option<&mut Node> {
        match self {
            Node::Table(entries) => entries.iter_mut().find(|(k, _)| k == key).map(|(_, n)| n),
            Node::List(_) => None,
        }
    }

    /// Every list with its dotted path, in order; a named list's entries as "path.name".
    pub fn flatten(&self) -> Vec<(String, Vec<String>)> {
        let mut out = Vec::new();
        fn walk(node: &Node, path: &str, out: &mut Vec<(String, Vec<String>)>) {
            match node {
                Node::List(words) => out.push((path.to_string(), words.clone())),
                Node::Table(entries) => {
                    for (key, child) in entries {
                        let path = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                        walk(child, &path, out);
                    }
                }
            }
        }
        walk(self, "", &mut out);
        out
    }
}

fn node_of_value(value: &toml_edit::Value) -> Option<Node> {
    match value {
        toml_edit::Value::Array(array) => Some(Node::List(array.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())),
        toml_edit::Value::InlineTable(table) => Some(Node::Table(table.iter().filter_map(|(k, v)| node_of_value(v).map(|n| (k.to_string(), n))).collect())),
        // A pack's own fields ("language", "checked") are no list.
        _ => None,
    }
}

fn node_of_item(item: &toml_edit::Item) -> Option<Node> {
    match item {
        toml_edit::Item::Table(table) => Some(Node::Table(table.iter().filter_map(|(k, v)| node_of_item(v).map(|n| (k.to_string(), n))).collect())),
        toml_edit::Item::Value(value) => node_of_value(value),
        _ => None,
    }
}

/// One pack: what it is for, when it was checked, its lists.
#[derive(Debug, Clone)]
pub struct Pack {
    /// Its file under `data/words/`: "fr.toml", "countries/FR.toml", "international.toml".
    pub file: String,
    /// "2026-10-08": the newest copy wins.
    pub checked: String,
    pub lists: Node,
}

impl Pack {
    /// A pack's text read; none when it is no TOML.
    pub fn parse(file: &str, text: &str) -> Option<Pack> {
        let doc: toml_edit::DocumentMut = text.parse().ok()?;
        let checked = doc.get("checked").and_then(toml_edit::Item::as_str).unwrap_or("").to_string();
        let lists = node_of_item(doc.as_item()).unwrap_or_else(Node::table);
        Some(Pack { file: file.to_string(), checked, lists })
    }

    /// The language a language pack is for ("fr"), none for the others.
    pub fn language(&self) -> Option<&str> {
        (!self.file.contains('/') && self.file != "international.toml").then(|| self.file.trim_end_matches(".toml"))
    }

    /// The country a country pack is for ("FR"), none for the others.
    pub fn country(&self) -> Option<&str> {
        self.file.strip_prefix("countries/").map(|f| f.trim_end_matches(".toml"))
    }
}

/// The packs built in, as shipped: no copy on disk read.
fn shipped() -> &'static [Pack] {
    static SHIPPED_PACKS: LazyLock<Vec<Pack>> = LazyLock::new(|| SHIPPED.iter().filter_map(|(file, text)| Pack::parse(file, text)).collect());
    &SHIPPED_PACKS
}

/// Where newer packs may be: yours first, then the system's (as `presets::places`).
fn places() -> Vec<PathBuf> {
    let mut places = vec![crate::config::data_dir().join("words")];
    let system = std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| if cfg!(windows) { Default::default() } else { "/usr/local/share:/usr/share".into() });
    places.extend(std::env::split_paths(&system).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join("sioul").join("words")));
    places
}

/// The packs, each the newest of the built-in one and the copies on disk, by `checked`.
fn newest() -> &'static [Pack] {
    static NEWEST: LazyLock<Vec<Pack>> = LazyLock::new(|| {
        let places = places();
        shipped()
            .iter()
            .map(|pack| {
                let mut best = pack.clone();
                for place in &places {
                    if let Some(found) = std::fs::read_to_string(place.join(&pack.file)).ok().and_then(|t| Pack::parse(&pack.file, &t))
                        && found.checked > best.checked
                    {
                        best = found;
                    }
                }
                best
            })
            .collect()
    });
    &NEWEST
}

/// The languages Sioul has a pack for: "en", "fr".
pub fn available_languages() -> Vec<&'static str> {
    shipped().iter().filter_map(Pack::language).collect()
}

/// The countries Sioul has a pack for: "FR".
pub fn available_countries() -> Vec<&'static str> {
    shipped().iter().filter_map(Pack::country).collect()
}

// ---------------------------------------------------------------- what is in use

fn strings(value: Option<&toml::Value>) -> Option<Vec<String>> {
    value?.as_array().map(|list| list.iter().filter_map(|v| v.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect())
}

fn deduplicated(list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(list.len());
    for word in list {
        if !out.contains(&word) {
            out.push(word);
        }
    }
    out
}

/// The languages read in your mail: `[words] languages`, else the interface's and English.
pub fn languages(config: &Config) -> Vec<String> {
    let said = strings(config.words.get("languages")).map(|list| list.iter().map(|l| base_language(l)).filter(|l| !l.is_empty()).collect::<Vec<_>>());
    deduplicated(said.unwrap_or_else(|| {
        let interface = config.language.clone().unwrap_or_else(crate::i18n::system_language);
        vec![base_language(&interface), "en".to_string()]
    }))
}

fn base_language(code: &str) -> String {
    code.split(['_', '-', '.', '@']).next().unwrap_or("").trim().to_ascii_lowercase()
}

/// The countries whose names are read: `[words] countries`, else `[contacts] region`, else the locale's.
pub fn countries(config: &Config) -> Vec<String> {
    let said = strings(config.words.get("countries")).map(|list| list.iter().map(|c| c.trim().to_ascii_uppercase()).filter(|c| !c.is_empty()).collect::<Vec<_>>());
    deduplicated(said.unwrap_or_else(|| {
        let region = config.contacts.region.as_deref().map(str::trim).filter(|r| !r.is_empty()).map(str::to_ascii_uppercase);
        let locale = || {
            ["LC_ALL", "LC_TELEPHONE", "LANG"]
                .iter()
                .filter_map(|v| std::env::var(v).ok())
                .find_map(|v| v.split(['.', '@']).next().and_then(|b| b.split(['_', '-']).nth(1)).map(str::to_ascii_uppercase).filter(|c| c.len() == 2))
        };
        region.or_else(locale).into_iter().collect()
    }))
}

/// The packs in use, in their order: the languages', the countries', the international one.
fn in_use<'a>(packs: &'a [Pack], languages: &[String], countries: &[String]) -> Vec<&'a Pack> {
    let mut out: Vec<&Pack> = Vec::new();
    for language in languages {
        out.extend(packs.iter().filter(|p| p.language() == Some(language.as_str())));
    }
    for country in countries {
        out.extend(packs.iter().filter(|p| p.country() == Some(country.as_str())));
    }
    out.extend(packs.iter().filter(|p| p.file == "international.toml"));
    out
}

/// Each pack's lists added to `into`, in order: a list's words after those already there.
fn merge(into: &mut Node, from: &Node) {
    let (Node::Table(target), Node::Table(source)) = (into, from) else { return };
    for (key, node) in source {
        match target.iter_mut().find(|(k, _)| k == key) {
            Some((_, existing)) => match (existing, node) {
                (Node::List(words), Node::List(more)) => words.extend(more.iter().cloned()),
                (existing @ Node::Table(_), Node::Table(_)) => merge(existing, node),
                // A list in one pack, a table in another: the first pack's stays.
                _ => {}
            },
            None => target.push((key.clone(), node.clone())),
        }
    }
}

/// A word as lists compare it: trimmed, lowercase, without its accents (`text::fold`).
pub fn folded(text: &str) -> String {
    crate::text::fold(text.trim()).into_iter().collect()
}

/// Whether a folded text holds one of a list's words as a whole word, as
/// docs/words.md says words are matched: no letter or digit right before it,
/// nor right after, where the word itself begins or ends with one ("du" in
/// "part du 01…", never in "durée"; "de :" before anything). A word added in
/// `[words]` is trimmed, and needs no spaces. The voicemail's callers' leads
/// and the sites' call words are read so.
pub fn holds_word(list: &[String], text: &str) -> bool {
    let alnum = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    list.iter().map(|w| folded(w)).filter(|w| !w.is_empty()).any(|word| {
        let (starts, ends) = (alnum(word.chars().next()), alnum(word.chars().next_back()));
        text.match_indices(word.as_str()).any(|(at, _)| (!starts || !alnum(text[..at].chars().next_back())) && (!ends || !alnum(text[at + word.len()..].chars().next())))
    })
}

/// Whether a word (a category, a folder's name) is one of a list's, folded.
pub fn is_named(word: &str, list: &[String]) -> bool {
    let word = folded(word);
    !word.is_empty() && list.iter().any(|w| folded(w) == word)
}

/// Whether one of these words (a task's categories) is one of a list's, folded.
pub fn named_in(words: &[String], list: &[String]) -> bool {
    let list: Vec<String> = list.iter().map(|w| folded(w)).filter(|w| !w.is_empty()).collect();
    words.iter().any(|w| list.contains(&folded(w)))
}

/// Your changes put on the merged lists: `[words.<list>]` tables with `add`
/// and `remove`. A list's `add` is words; a named list's `add` is names and
/// their words, its `remove` names. The shipped words you took away go
/// first, then yours come: one both taken away and added is there.
fn apply(into: &mut Node, changes: &toml::Table) {
    for (key, value) in changes {
        // `languages`, `countries`: settings of the words, no list.
        let toml::Value::Table(change) = value else { continue };
        let is_change = change.contains_key("add") || change.contains_key("remove");
        if is_change {
            apply_one(into, key, change);
        }
        let deeper: toml::Table = change.iter().filter(|(k, _)| !matches!(k.as_str(), "add" | "remove")).map(|(k, v)| (k.clone(), v.clone())).collect();
        if deeper.is_empty() {
            continue;
        }
        if into.entry_mut(key).is_none()
            && let Node::Table(entries) = into
        {
            entries.push((key.clone(), Node::table()));
        }
        if let Some(child @ Node::Table(_)) = into.entry_mut(key) {
            apply(child, &deeper);
        }
    }
}

fn apply_one(into: &mut Node, key: &str, change: &toml::Table) {
    let removed: Vec<String> = strings(change.get("remove")).unwrap_or_default().iter().map(|w| folded(w)).collect();
    let add = change.get("add");
    if into.entry_mut(key).is_none() {
        let Node::Table(entries) = into else { return };
        let node = match add {
            Some(toml::Value::Table(_)) => Node::table(),
            Some(toml::Value::Array(_)) => Node::List(Vec::new()),
            _ => return,
        };
        entries.push((key.to_string(), node));
    }
    match (into.entry_mut(key), add) {
        (Some(Node::List(words)), add) => {
            words.retain(|w| !removed.contains(&folded(w)));
            if let Some(add) = strings(add) {
                words.extend(add);
            }
        }
        (Some(Node::Table(entries)), add) => {
            entries.retain(|(name, _)| !removed.contains(&folded(name)));
            if let Some(toml::Value::Table(add)) = add {
                for (name, words) in add {
                    let words = strings(Some(words)).unwrap_or_default();
                    match entries.iter_mut().find(|(n, _)| folded(n) == folded(name)) {
                        Some((_, Node::List(existing))) => existing.extend(words),
                        Some(_) => {}
                        None => entries.push((name.clone(), Node::List(words))),
                    }
                }
            }
        }
        _ => {}
    }
}

/// Each list's words once, compared folded, the first spelling kept; empty words dropped.
fn tidy(node: &mut Node) {
    match node {
        Node::List(words) => {
            let mut seen: Vec<String> = Vec::with_capacity(words.len());
            words.retain(|w| {
                let key = folded(w);
                let keep = !key.is_empty() && !seen.contains(&key);
                seen.push(key);
                keep
            });
        }
        Node::Table(entries) => entries.iter_mut().for_each(|(_, n)| tidy(n)),
    }
}

/// The merged lists as the typed structs read them: a named list as pairs, in order.
fn value_of(node: &Node, path: &str) -> toml::Value {
    match node {
        Node::List(words) => toml::Value::Array(words.iter().cloned().map(toml::Value::String).collect()),
        Node::Table(entries) if MAPS.contains(&path) => toml::Value::Array(
            entries
                .iter()
                .filter_map(|(name, n)| match n {
                    Node::List(words) => Some(toml::Value::Array(vec![toml::Value::String(name.clone()), toml::Value::Array(words.iter().cloned().map(toml::Value::String).collect())])),
                    Node::Table(_) => None,
                })
                .collect(),
        ),
        Node::Table(entries) => toml::Value::Table(
            entries
                .iter()
                .map(|(key, n)| {
                    let child = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                    (key.clone(), value_of(n, &child))
                })
                .collect(),
        ),
    }
}

static BUILTIN: LazyLock<Arc<Words>> = LazyLock::new(|| {
    let (languages, countries) = (FIRST_LANGUAGES.map(String::from).to_vec(), FIRST_COUNTRIES.map(String::from).to_vec());
    Arc::new(typed(&lists(&in_use(shipped(), &languages, &countries), None), languages, countries))
});

/// The lists made from these packs, with these changes (none: the packs alone).
fn lists(packs: &[&Pack], changes: Option<&toml::Table>) -> Node {
    let mut tree = Node::table();
    for pack in packs {
        merge(&mut tree, &pack.lists);
    }
    if let Some(changes) = changes {
        apply(&mut tree, changes);
    }
    tidy(&mut tree);
    tree
}

fn typed(tree: &Node, languages: Vec<String>, countries: Vec<String>) -> Words {
    let mut words: Words = value_of(tree, "").try_into().unwrap_or_else(|e| {
        eprintln!("sioul: the words: {e}");
        Words::default()
    });
    words.languages = languages;
    words.countries = countries;
    words
}

impl Words {
    /// The built-in packs alone, for the languages and countries every list
    /// was written for until 8 October 2026 (French and English, France):
    /// what tests and callers without a configuration read.
    pub fn builtin() -> Arc<Words> {
        BUILTIN.clone()
    }

    /// The same, borrowed for the program's life: where a context gives no words.
    pub fn builtin_ref() -> &'static Words {
        &BUILTIN
    }

    /// The words as your configuration makes them: the packs in use, the
    /// newest of each, then your changes. Made once for each configuration.
    pub fn of(config: &Config) -> Arc<Words> {
        type Cache = Mutex<Vec<(String, Arc<Words>)>>;
        static CACHE: LazyLock<Cache> = LazyLock::new(|| Mutex::new(Vec::new()));
        let (languages, countries) = (languages(config), countries(config));
        // Keyed by all it is made from: the older settings too (`older_settings`),
        // else two configurations alike but for `filed_words` shared one list.
        let key = format!("{languages:?}|{countries:?}|{}|{:?}|{:?}|{:?}", toml::to_string(&config.words).unwrap_or_default(), config.filed_words, config.quiet.personal, config.quiet.work);
        if let Ok(cache) = CACHE.lock()
            && let Some((_, words)) = cache.iter().find(|(k, _)| *k == key)
        {
            return words.clone();
        }
        let tree = lists(&in_use(newest(), &languages, &countries), Some(&config.words));
        let mut words = typed(&tree, languages, countries);
        older_settings(config, &mut words);
        let words = Arc::new(words);
        if let Ok(mut cache) = CACHE.lock() {
            cache.retain(|(k, _)| *k != key);
            cache.insert(0, (key, words.clone()));
            cache.truncate(4);
        }
        words
    }
}

/// The settings that held whole lists before `[words]` (`filed_words`,
/// `[quiet] personal` and `work`): while written, and while `[words]` says
/// nothing of the same list, they are the list, as they were.
fn older_settings(config: &Config, words: &mut Words) {
    let changed = |recogniser: &str, list: &str| config.words.get(recogniser).and_then(|r| r.get(list)).is_some();
    if let Some(filed) = config.filed_words.as_ref().filter(|f| !f.is_empty())
        && !changed("senders", "automatic")
    {
        words.senders.automatic = filed.clone();
    }
    if let Some(personal) = &config.quiet.personal
        && !changed("tasks", "personal")
    {
        words.tasks.personal = personal.clone();
    }
    if let Some(work) = &config.quiet.work
        && !changed("tasks", "work")
    {
        words.tasks.work = work.clone();
    }
}

/// The words of the configuration this program loaded last (`set_current`).
static CURRENT: std::sync::RwLock<Option<Arc<Words>>> = std::sync::RwLock::new(None);

/// The words of the configuration this program loaded last, else the
/// built-in ones: for the few readers deep in other crates that see no
/// configuration (a server's folder names, `folders::role_of`). The window
/// and the command line say them (`set_current`) when they read the
/// configuration; tests never do, so they read the built-in ones.
pub fn current() -> Arc<Words> {
    CURRENT.read().ok().and_then(|c| c.clone()).unwrap_or_else(Words::builtin)
}

/// Says which words this program uses now (`current`).
pub fn set_current(words: Arc<Words>) {
    if let Ok(mut current) = CURRENT.write() {
        *current = Some(words);
    }
}

/// The packs built in, with one payment processor's wording added as its
/// users would add it under `[words]` (the shipped packs leave it out, as
/// one person's): what the tests of payments read.
#[cfg(test)]
pub(crate) fn with_processor() -> Arc<Words> {
    let config: Config = toml::from_str(
        "[words.payments.paid]\nadd = [\"recu pour votre paiement\", \"recu de votre paiement\", \"vous avez autorise un paiement\", \"vous avez envoye un paiement\"]\n\
         [words.payments.not_payments]\nadd = [\"rapport d'activite\"]\n[words.payments.to]\nadd = [\"paiement a\"]\n[words.payments.from]\nadd = [\"paiement de\"]\n\
         [words.payments.leads]\nadd = [\"en faveur de\"]\n[words.money.stated]\nadd = [\"vous avez autorise un paiement de\"]\n",
    )
    .unwrap_or_default();
    let (languages, countries) = (FIRST_LANGUAGES.map(String::from).to_vec(), FIRST_COUNTRIES.map(String::from).to_vec());
    Arc::new(typed(&lists(&in_use(shipped(), &languages, &countries), Some(&config.words)), languages, countries))
}

/// The lists in use, as dotted paths: the packs' alone (`shipped`) and with
/// your changes (`yours`), for the settings that show what you changed.
pub struct Compared {
    pub shipped: Node,
    pub yours: Node,
}

/// The lists of a configuration, shipped and changed.
pub fn compared(config: &Config) -> Compared {
    let packs = in_use(newest(), &languages(config), &countries(config));
    Compared { shipped: lists(&packs, None), yours: lists(&packs, Some(&config.words)) }
}

// ---------------------------------------------------------------- writing your changes

/// The table of a list's changes in the configuration (`[words.codes.code]`),
/// made with the tables above it, those implicit, so the file says only it.
fn changes_table<'a>(doc: &'a mut toml_edit::DocumentMut, path: &str) -> Option<&'a mut toml_edit::Table> {
    let mut table = doc.as_table_mut();
    if !table.contains_key("words") {
        let mut words = toml_edit::Table::new();
        words.set_implicit(true);
        table.insert("words", toml_edit::Item::Table(words));
    }
    table = table.get_mut("words")?.as_table_mut()?;
    for part in path.split('.').filter(|p| !p.is_empty()) {
        if !table.contains_key(part) {
            let mut next = toml_edit::Table::new();
            next.set_implicit(true);
            table.insert(part, toml_edit::Item::Table(next));
        }
        table = table.get_mut(part)?.as_table_mut()?;
    }
    Some(table)
}

/// Takes a list's changes out, and the tables above them left empty.
fn drop_changes(doc: &mut toml_edit::DocumentMut, path: &str) {
    fn prune(table: &mut toml_edit::Table, parts: &[&str]) {
        let Some((first, rest)) = parts.split_first() else { return };
        let Some(child) = table.get_mut(first).and_then(toml_edit::Item::as_table_mut) else { return };
        if rest.is_empty() {
            child.remove("add");
            child.remove("remove");
        } else {
            prune(child, rest);
        }
        if child.is_empty() {
            table.remove(first);
        }
    }
    let parts: Vec<&str> = std::iter::once("words").chain(path.split('.').filter(|p| !p.is_empty())).collect();
    prune(doc.as_table_mut(), &parts);
}

fn array(words: &[String]) -> toml_edit::Item {
    toml_edit::value(words.iter().map(String::as_str).collect::<toml_edit::Array>())
}

/// Writes a list's changes so that it holds `wanted`, compared with what the
/// packs alone give (`shipped`): the words not shipped go to `add`, the
/// shipped ones not wanted to `remove`. Wanting the shipped list takes the
/// changes out. A named list (`MAPS`) is not written here: see `write_names`.
pub fn write_list(config_path: &Path, path: &str, wanted: &[String], shipped: &[String]) -> Result<(), String> {
    let shipped_folded: Vec<String> = shipped.iter().map(|w| folded(w)).collect();
    let wanted_folded: Vec<String> = wanted.iter().map(|w| folded(w)).collect();
    let add: Vec<String> = deduplicated(wanted.iter().filter(|w| !folded(w).is_empty() && !shipped_folded.contains(&folded(w))).map(|w| w.trim().to_string()).collect());
    let remove: Vec<String> = shipped.iter().filter(|w| !wanted_folded.contains(&folded(w))).cloned().collect();
    let mut doc = crate::config::read_document(config_path)?;
    drop_changes(&mut doc, path);
    if !add.is_empty() || !remove.is_empty() {
        let table = changes_table(&mut doc, path).ok_or_else(|| format!("{}: words.{path} is not a table", config_path.display()))?;
        table.set_implicit(false);
        if !add.is_empty() {
            table.insert("add", array(&add));
        }
        if !remove.is_empty() {
            table.insert("remove", array(&remove));
        }
    }
    crate::config::write_document(config_path, &doc)
}

/// Takes every change of a recogniser out ("codes": `[words.codes…]`): back to the shipped lists.
pub fn reset(config_path: &Path, recogniser: &str) -> Result<(), String> {
    let mut doc = crate::config::read_document(config_path)?;
    if let Some(words) = doc.get_mut("words").and_then(toml_edit::Item::as_table_mut) {
        words.remove(recogniser);
        if words.is_empty() {
            doc.remove("words");
        }
    }
    crate::config::write_document(config_path, &doc)
}

/// Takes the changes of these lists out ("folders.trash"…): one line of the
/// words tab back to the shipped lists, the other lists of its recognisers kept.
pub fn reset_lists(config_path: &Path, paths: &[&str]) -> Result<(), String> {
    let mut doc = crate::config::read_document(config_path)?;
    for path in paths {
        drop_changes(&mut doc, path);
    }
    crate::config::write_document(config_path, &doc)
}

/// Writes a named list's changes (`MAPS`: brands and their domains, bodies
/// and their words) so that it holds `wanted`, compared with what the packs
/// alone give (`shipped`), names and words folded: a name not shipped goes to
/// `add` with its words; a shipped name not wanted to `remove`; a shipped name
/// given more words, those words to `add`; a shipped name given fewer, to
/// `remove` and back in `add` with the words wanted (added wins). Wanting the
/// shipped list takes the changes out.
pub fn write_names(config_path: &Path, path: &str, wanted: &[(String, Vec<String>)], shipped: &[(String, Vec<String>)]) -> Result<(), String> {
    let folded_all = |words: &[String]| words.iter().map(|w| folded(w)).filter(|w| !w.is_empty()).collect::<Vec<_>>();
    let trimmed = |words: &[String]| deduplicated(words.iter().map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect());
    let mut add: Vec<(String, Vec<String>)> = Vec::new();
    let mut remove: Vec<String> = Vec::new();
    for (name, words) in shipped {
        let Some((_, wanted_words)) = wanted.iter().find(|(n, _)| folded(n) == folded(name)) else {
            remove.push(name.clone());
            continue;
        };
        let (have, want) = (folded_all(words), folded_all(wanted_words));
        if have.iter().all(|w| want.contains(w)) {
            let more: Vec<String> = trimmed(wanted_words).into_iter().filter(|w| !have.contains(&folded(w))).collect();
            if !more.is_empty() {
                add.push((name.clone(), more));
            }
        } else {
            remove.push(name.clone());
            add.push((name.clone(), trimmed(wanted_words)));
        }
    }
    for (name, words) in wanted {
        if !folded(name).is_empty() && !shipped.iter().any(|(n, _)| folded(n) == folded(name)) && !add.iter().any(|(n, _)| folded(n) == folded(name)) {
            add.push((name.trim().to_string(), trimmed(words)));
        }
    }
    let mut doc = crate::config::read_document(config_path)?;
    drop_changes(&mut doc, path);
    if !add.is_empty() || !remove.is_empty() {
        let table = changes_table(&mut doc, path).ok_or_else(|| format!("{}: words.{path} is not a table", config_path.display()))?;
        table.set_implicit(false);
        if !add.is_empty() {
            let mut names = toml_edit::InlineTable::new();
            for (name, words) in &add {
                names.insert(name.as_str(), toml_edit::Value::Array(words.iter().map(String::as_str).collect()));
            }
            table.insert("add", toml_edit::value(names));
        }
        if !remove.is_empty() {
            table.insert("remove", array(&remove));
        }
    }
    crate::config::write_document(config_path, &doc)
}

/// Writes `[words] languages` (codes, "fr") or `countries` ("FR"), the order kept.
pub fn write_setting(config_path: &Path, key: &str, values: &[String]) -> Result<(), String> {
    if !matches!(key, "languages" | "countries") {
        return Err(format!("words.{key}: no such setting"));
    }
    let mut doc = crate::config::read_document(config_path)?;
    let table = changes_table(&mut doc, "").ok_or_else(|| format!("{}: words is not a table", config_path.display()))?;
    table.set_implicit(false);
    table.insert(key, array(&deduplicated(values.to_vec())));
    crate::config::write_document(config_path, &doc)
}

/// For a configuration older than these words (a config.toml without a
/// `[words]` table, written before 8 October 2026): writes the languages
/// and the country every list was written for until then, so that nothing
/// changes until you change it. Returns whether it wrote.
pub fn keep_first(config_path: &Path) -> Result<bool, String> {
    let Ok(text) = std::fs::read_to_string(config_path) else { return Ok(false) };
    let doc: toml_edit::DocumentMut = text.parse().map_err(|e| format!("{}: {e}", config_path.display()))?;
    if doc.contains_key("words") {
        return Ok(false);
    }
    write_setting(config_path, "languages", &FIRST_LANGUAGES.map(String::from))?;
    write_setting(config_path, "countries", &FIRST_COUNTRIES.map(String::from))?;
    Ok(true)
}

/// `keep_first`, once on each device: at the first start of a Sioul with these
/// words. A configuration from before keeps its French, English and France;
/// one made afterwards (a new person's, by this Sioul) is left to its
/// interface's language and English (`languages`). `marker` says it was done.
pub fn keep_first_once(config_path: &Path, marker: &Path) -> Result<bool, String> {
    if marker.exists() {
        return Ok(false);
    }
    let wrote = keep_first(config_path)?;
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(marker, b"").map_err(|e| format!("{}: {e}", marker.display()))?;
    Ok(wrote)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(text: &str) -> Config {
        toml::from_str(text).unwrap()
    }

    /// Every list the typed structs know, as dotted paths, and the named lists.
    fn known() -> Vec<String> {
        let value = toml::Value::try_from(Words::default()).unwrap();
        fn walk(value: &toml::Value, path: &str, out: &mut Vec<String>) {
            match value {
                toml::Value::Table(table) => {
                    for (key, child) in table {
                        walk(child, &if path.is_empty() { key.clone() } else { format!("{path}.{key}") }, out);
                    }
                }
                _ => out.push(path.to_string()),
            }
        }
        let mut out = Vec::new();
        walk(&value, "", &mut out);
        out
    }

    #[test]
    fn every_pack_reads_and_holds_known_lists_only() {
        let known = known();
        for (file, text) in SHIPPED {
            let pack = Pack::parse(file, text).unwrap_or_else(|| panic!("{file} is no TOML"));
            assert_eq!(pack.checked.len(), 10, "{file}: its checked date");
            for (path, words) in pack.lists.flatten() {
                let map = MAPS.iter().find(|m| path.starts_with(&format!("{m}.")));
                assert!(known.contains(&path) || map.is_some(), "{file}: {path} is no list Sioul reads");
                for word in &words {
                    assert!(!folded(word).is_empty(), "{file}: {path} holds an empty word");
                }
            }
        }
        assert_eq!(available_languages(), ["en", "fr", "de", "es", "it"]);
        assert_eq!(available_countries(), ["FR"]);
    }

    /// The packs merged give every list back as the code held it on 8 October
    /// 2026 (v0.0.3), a person's own words left out (tests/fixtures/words-golden.json).
    #[test]
    fn the_packs_give_back_the_lists_of_before() {
        let golden: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/words-golden.json")).unwrap();
        let golden = golden["lists"].as_object().unwrap();
        let (languages, countries) = (FIRST_LANGUAGES.map(String::from).to_vec(), FIRST_COUNTRIES.map(String::from).to_vec());
        let tree = lists(&in_use(shipped(), &languages, &countries), None);
        let ours: std::collections::BTreeMap<String, Vec<String>> = tree.flatten().into_iter().collect();
        let set = |words: &[String]| words.iter().map(|w| folded(w)).collect::<std::collections::BTreeSet<_>>();
        for (path, words) in golden {
            let words: Vec<String> = words.as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect();
            let ours_path = if path == "ocr.languages" { "ocr.tesseract" } else { path.as_str() };
            let found = ours.get(ours_path).cloned().unwrap_or_default();
            assert_eq!(set(&found), set(&words), "{path}");
        }
        // And nothing more: every list of the packs is one of before.
        for path in ours.keys() {
            let before = if path == "ocr.tesseract" { "ocr.languages" } else { path.as_str() };
            assert!(golden.contains_key(before), "{path} is new");
        }
    }

    #[test]
    fn typed_lists_and_named_lists_in_their_order() {
        let words = Words::builtin();
        assert!(words.codes.code.iter().any(|w| w == "code de verification") && words.codes.code.iter().any(|w| w == "verification code"));
        assert_eq!(words.letters.months.iter().find(|(n, _)| n == "10").map(|(_, w)| w.len()), Some(3));
        // Named lists keep the packs' order: France's names, then the international ones.
        let brands: Vec<&str> = words.brands.brands.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(brands.first(), Some(&"Ameli"));
        assert!(brands.contains(&"Amazon") && brands.contains(&"Boursorama"));
        assert_eq!(words.capture.in_, ["dans", "in"]);
        assert_eq!(words.ocr.tesseract, ["fra", "eng"]);
        assert_eq!((words.languages.as_slice(), words.countries.as_slice()), (&["fr".to_string(), "en".into()][..], &["FR".to_string()][..]));
    }

    #[test]
    fn your_changes_merge_with_the_packs() {
        let config = config(
            "[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n\n\
             [words.codes.code]\nadd = [\"Bestätigungscode\", \"código de verificación\"]\nremove = [\"Code Temporaire\"]\n\n\
             [words.folders.trash]\nadd = [\"Papierkorb\"]\nremove = [\"bin\", \"corbeille\"]\n\n\
             [words.senders.automatic]\nadd = [\"noreply\", \"nicht-antworten\"]\nremove = [\"noreply\"]\n\n\
             [words.brands.brands]\nadd = { \"Ma Banque\" = [\"mabanque.example\"], Amazon = [\"amazon.example\"] }\nremove = [\"boursorama\"]\n\n\
             [words.letters.senders.\"Ma Caisse\"]\nadd = [\"caisse exemple\"]\n\n\
             [words.newer.list]\nadd = [\"anything\"]\n",
        );
        let words = Words::of(&config);
        let has = |list: &[String], w: &str| list.iter().any(|x| x == w);
        assert!(has(&words.codes.code, "Bestätigungscode") && has(&words.codes.code, "código de verificación"));
        assert!(!has(&words.codes.code, "code temporaire"), "taken away, capitals aside");
        assert!(has(&words.folders.trash, "Papierkorb") && !has(&words.folders.trash, "bin") && !has(&words.folders.trash, "corbeille"));
        assert!(has(&words.folders.trash, "trash"));
        assert!(has(&words.senders.automatic, "noreply"), "taken away and added: put back");
        assert_eq!(words.senders.automatic.iter().filter(|w| *w == "noreply").count(), 1);
        let brand = |name: &str| words.brands.brands.iter().find(|(n, _)| n == name).map(|(_, d)| d.clone());
        assert_eq!(brand("Ma Banque"), Some(vec!["mabanque.example".to_string()]));
        assert!(brand("Amazon").unwrap().contains(&"amazon.example".to_string()) && brand("Amazon").unwrap().contains(&"amazon.fr".to_string()));
        assert_eq!(brand("Boursorama"), None, "a named list's entry taken away by its name, capitals aside");
        assert!(words.letters.senders.iter().any(|(n, w)| n == "Ma Caisse" && w == &["caisse exemple"]));
        // The rest as shipped.
        assert_eq!(words.codes.reset, Words::builtin().codes.reset);
    }

    #[test]
    fn languages_and_countries_in_use() {
        let english = config("language = \"en\"\n[words]\nlanguages = [\"en\"]\ncountries = []\n");
        let words = Words::of(&english);
        assert!(words.codes.code.iter().all(|w| !w.starts_with("code de")), "no French");
        assert!(words.brands.brands.iter().all(|(n, _)| n != "Ameli"), "no French names");
        assert!(words.brands.brands.iter().any(|(n, _)| n == "Amazon"), "the international pack, always");
        assert!(words.folders.sent.iter().any(|w| w == "envoyés"), "folders' names, whatever you read");
        assert_eq!(words.ocr.tesseract, ["eng"]);
        let unsaid = config("language = \"fr\"\n[contacts]\nregion = \"fr\"\n");
        assert_eq!((languages(&unsaid), countries(&unsaid)), (vec!["fr".to_string(), "en".into()], vec!["FR".to_string()]));
        let english_first = config("language = \"en_GB\"\n[words]\ncountries = [\"fr\", \"FR\"]\n");
        assert_eq!((languages(&english_first), countries(&english_first)), (vec!["en".to_string()], vec!["FR".to_string()]));
    }

    /// German, Spanish and Italian, once read: a code, the bin's name, a day typed; none of it in English alone.
    #[test]
    fn german_spanish_and_italian_once_read() {
        let today: jiff::civil::Date = "2026-10-03".parse().unwrap();
        for (language, subject, body, bin, friday) in [
            ("de", "Ihr Bestätigungscode", "Ihr Bestätigungscode lautet 482913. Er ist 10 Minuten gültig.", "Papierkorb", "Freitag"),
            ("es", "Tu código de verificación", "Tu código de verificación es 482913. Caduca en 10 minutos.", "Papelera", "viernes"),
            ("it", "Il tuo codice di verifica", "Il tuo codice di verifica è 482913. Scade tra 10 minuti.", "Cestino", "venerdi"),
        ] {
            let words = Words::of(&config(&format!("[words]\nlanguages = [\"{language}\"]\ncountries = []\n")));
            let found = crate::codes::detect(&words.codes, subject, body).unwrap_or_else(|| panic!("{language}: no code"));
            assert_eq!(found.code.as_deref(), Some("482913"), "{language}");
            assert_eq!(crate::folders::role_in(&words.folders, bin, None), crate::folders::Role::Trash, "{language}");
            let day = crate::capture::parse_day_with(&words.capture, friday, today).map(|d| d.to_string());
            assert_eq!(day.as_deref(), Some("2026-10-09"), "{language}");
            assert_eq!(words.ocr.tesseract.len(), 1, "{language}");
        }
        let english = Words::of(&config("[words]\nlanguages = [\"en\"]\ncountries = []\n"));
        assert!(crate::codes::detect(&english.codes, "Ihr Bestätigungscode", "Ihr Bestätigungscode lautet 482913.").is_none());
        assert_eq!(crate::folders::role_in(&english.folders, "Papierkorb", None), crate::folders::Role::Other);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-words-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    #[test]
    fn writing_keeps_only_the_differences() {
        let path = scratch("write");
        std::fs::write(&path, "# Mine\nfetch_minutes = 15\n\n[mail]\nthreads = true # by conversation\n").unwrap();
        let shipped = Words::builtin().folders.trash.clone();
        let mut wanted: Vec<String> = shipped.iter().filter(|w| *w != "bin").cloned().collect();
        wanted.push("Papierkorb".into());
        write_list(&path, "folders.trash", &wanted, &shipped).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# Mine") && text.contains("# by conversation"), "comments kept: {text}");
        assert!(text.contains("[words.folders.trash]") && !text.contains("[words]\n") && !text.contains("[words.folders]\n"), "{text}");
        let config = Config::load(&path).unwrap();
        let words = Words::of(&config);
        assert!(words.folders.trash.contains(&"Papierkorb".to_string()) && !words.folders.trash.contains(&"bin".to_string()));
        // Back to the shipped list: no change left, no empty table.
        write_list(&path, "folders.trash", &shipped, &shipped).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("words"), "{text}");
        // Languages written, then reset of a recogniser.
        write_setting(&path, "languages", &["fr".into(), "en".into()]).unwrap();
        write_list(&path, "codes.code", &["mon code".into()], &[]).unwrap();
        reset(&path, "codes").unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(languages(&config), ["fr", "en"]);
        assert!(!std::fs::read_to_string(&path).unwrap().contains("codes"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_older_configuration_keeps_what_it_read() {
        let path = scratch("first");
        std::fs::write(&path, "language = \"en\"\n").unwrap();
        assert!(keep_first(&path).unwrap());
        let config = Config::load(&path).unwrap();
        assert_eq!((languages(&config), countries(&config)), (vec!["fr".to_string(), "en".into()], vec!["FR".to_string()]));
        assert!(!keep_first(&path).unwrap(), "once");
        let none = path.with_file_name("none.toml");
        assert!(!keep_first(&none).unwrap(), "no configuration, nothing written");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// Once on each device: a configuration made after the first start keeps its interface's language.
    #[test]
    fn the_first_start_keeps_an_older_configuration_once() {
        let path = scratch("once");
        let marker = path.with_file_name("state").join("words-first-start");
        std::fs::write(&path, "language = \"en\"\n").unwrap();
        assert!(keep_first_once(&path, &marker).unwrap());
        assert!(marker.exists());
        std::fs::write(&path, "language = \"en\"\n").unwrap();
        assert!(!keep_first_once(&path, &marker).unwrap());
        assert!(!std::fs::read_to_string(&path).unwrap().contains("words"));
        // A new device: no configuration yet, the marker written all the same.
        let new = path.with_file_name("new.toml");
        let new_marker = path.with_file_name("new-state").join("words-first-start");
        assert!(!keep_first_once(&new, &new_marker).unwrap());
        assert!(new_marker.exists());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A named list is written as its differences: a brand of yours, a domain
    /// more, a domain fewer, a brand taken away; back to the shipped list, nothing.
    #[test]
    fn a_named_list_is_written_as_its_differences() {
        let path = scratch("names");
        std::fs::write(&path, "[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n").unwrap();
        let shipped = Words::builtin().brands.brands.clone();
        let mut wanted = shipped.clone();
        wanted.retain(|(name, _)| name != "Chronopost");
        if let Some((_, domains)) = wanted.iter_mut().find(|(name, _)| name == "Colissimo") {
            domains.push("colissimo.example".into());
        }
        wanted.push(("Ma Banque".into(), vec!["mabanque.example".into()]));
        write_names(&path, "brands.brands", &wanted, &shipped).unwrap();
        let brands = Words::of(&Config::load(&path).unwrap()).brands.brands.clone();
        assert!(brands.iter().any(|(n, d)| n == "Ma Banque" && d == &["mabanque.example".to_string()]), "{brands:?}");
        assert!(!brands.iter().any(|(n, _)| n == "Chronopost"));
        let colissimo = |brands: &[(String, Vec<String>)]| brands.iter().find(|(n, _)| n == "Colissimo").map(|(_, d)| d.clone()).unwrap_or_default();
        assert!(colissimo(&brands).contains(&"colissimo.example".to_string()) && colissimo(&brands).contains(&"laposte.fr".to_string()));
        let mut fewer = brands.clone();
        if let Some((_, domains)) = fewer.iter_mut().find(|(name, _)| name == "Colissimo") {
            domains.retain(|d| d != "laposte.fr");
        }
        write_names(&path, "brands.brands", &fewer, &shipped).unwrap();
        let brands = Words::of(&Config::load(&path).unwrap()).brands.brands.clone();
        assert!(!colissimo(&brands).contains(&"laposte.fr".to_string()) && colissimo(&brands).contains(&"colissimo.example".to_string()), "{brands:?}");
        write_names(&path, "brands.brands", &shipped, &shipped).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("brands"));
        // A line of lists taken back at once, the languages kept.
        write_list(&path, "folders.trash", &["Papierkorb".into()], &Words::builtin().folders.trash).unwrap();
        write_list(&path, "folders.sent", &["Gesendet".into()], &Words::builtin().folders.sent).unwrap();
        reset_lists(&path, &["folders.trash", "folders.sent"]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("folders") && text.contains("languages"), "{text}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}

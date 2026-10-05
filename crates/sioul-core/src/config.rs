// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The configuration file: accounts, admin windows, the case store.
//!
//! Default location: `$XDG_CONFIG_HOME/sioul/config.toml`, else
//! `~/.config/sioul/config.toml`. Passwords never go in it: they belong in the
//! system keyring (docs/architecture.md). See examples/config.toml.
//!
//! Sioul writes to this file too, when you add or remove an account. It edits
//! it in place with `toml_edit`, so your comments and your order stay.

use crate::window::AdminWindow;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// The language of what Sioul says ("fr", "en"); the session's language if unset.
    pub language: Option<String>,
    /// "light" or "dark"; the system's colours if unset.
    #[serde(default)]
    pub theme: Option<String>,
    /// The folder of Markdown files that holds the record of each case.
    pub case_store: Option<String>,
    /// Projects, budgets and the bank's movements (`sioul-cases.toml`,
    /// `sioul-budgets.toml`, `sioul-bank.toml`) travel sealed with the sharing
    /// between your devices, for a notes folder no sync carries; else they
    /// travel with that folder (docs/database.md).
    #[serde(default)]
    pub share_projects: bool,
    /// A text file of senders you know: one address or `@domain` per line.
    pub known_senders: Option<String>,
    /// A text file of senders you blocked, in the same form.
    pub blocked_senders: Option<String>,
    /// How far back mail and the agenda reach, in weeks; 0 for everything.
    /// Raising it fetches the older mail the next sync does not have yet.
    #[serde(default)]
    pub history_weeks: Option<u32>,
    /// Minutes between two fetches of the folders a server does not push (IDLE keeps the inbox current).
    #[serde(default)]
    pub fetch_minutes: Option<u32>,
    /// Where new notes go in the case store.
    #[serde(default)]
    pub notes_folder: Option<String>,
    /// Words that make a sender automatic, its mail filed: "no-reply", "notification"…
    #[serde(default)]
    pub filed_words: Option<Vec<String>>,
    /// How long text reads: the family, size and line spacing of notes, mail and task notes.
    #[serde(default)]
    pub reading: Reading,
    #[serde(default)]
    pub mail: MailSettings,
    #[serde(default)]
    pub tasks: TaskSettings,
    #[serde(default)]
    pub agenda: AgendaSettings,
    #[serde(default)]
    pub map: MapSettings,
    /// Reminders before dates: events, dates asked, waits, payments (docs/reminders.md).
    #[serde(default)]
    pub reminders: ReminderSettings,
    /// Paper letters: where their scans arrive (docs/porch.md).
    #[serde(default)]
    pub letters: LetterSettings,
    /// Routines: timed steps played one at a time (docs/tasks.md).
    #[serde(rename = "routine", default)]
    pub routines: Vec<crate::routines::Routine>,
    /// The place of the weather in the status line; none, no weather.
    #[serde(default)]
    pub weather: WeatherSettings,
    /// Your Bitwarden account, to fill sites' forms; read here, nothing installed.
    #[serde(default)]
    pub bitwarden: BitwardenSettings,
    /// GitHub's issues and pull requests as tasks: off unless asked (docs/github.md).
    #[serde(default)]
    pub github: GithubSettings,
    /// Who sends your invoices, and what an hour costs by default.
    #[serde(default)]
    pub invoice: InvoiceSettings,
    /// Your accounts, those switched on (`Config::load` sets the others aside).
    #[serde(rename = "account", default)]
    pub accounts: Vec<Account>,
    /// Sites kept open in Sioul (docs/sites.md): not accounts, they hold no
    /// sign-in of Sioul's. Older files kept them as `[[account]]` of kind
    /// "portal": `Config::load` reads them here, `migrate_sites` moves them.
    #[serde(rename = "site", default)]
    pub sites: Vec<Account>,
    /// Accounts switched off: kept with their settings, neither synced nor shown.
    #[serde(skip)]
    pub accounts_off: Vec<Account>,
    #[serde(rename = "window", default)]
    pub windows: Vec<AdminWindow>,
    /// When offices are open, for tasks that need one; Monday to Friday 9:00–17:00 when unsaid.
    #[serde(rename = "office_hours", default)]
    pub office_hours: Vec<AdminWindow>,
    /// Days off: holidays, sick leave; quiet from the first day to the last.
    #[serde(rename = "time_off", default)]
    pub time_off: Vec<TimeOff>,
    #[serde(default)]
    pub quiet: QuietSettings,
    /// The Porch's own choices (docs/porch.md).
    #[serde(default)]
    pub porch: PorchSettings,
    /// The camera, microphone and speaker calls in sites use (docs/sites.md).
    #[serde(default)]
    pub calls: CallSettings,
}

/// The devices of calls in sites, by the names the system gives them; unsaid, the system's own.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
pub struct CallSettings {
    #[serde(default)]
    pub camera: Option<String>,
    #[serde(default)]
    pub microphone: Option<String>,
    #[serde(default)]
    pub speaker: Option<String>,
}

/// What the Porch shows of what Sioul knows.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct PorchSettings {
    /// Projects whose lane the Porch leaves out: their mail stays on their page in Projects.
    #[serde(default)]
    pub hidden_projects: Vec<String>,
}

/// Days off, from one day to another, both included.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TimeOff {
    #[serde(deserialize_with = "crate::budget::dates::required")]
    pub from: jiff::civil::Date,
    #[serde(deserialize_with = "crate::budget::dates::required")]
    pub until: jiff::civil::Date,
    #[serde(default)]
    pub label: String,
}

/// What quiet time keeps: the categories of tasks that are yours (family, friends, leisure).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct QuietSettings {
    /// Task categories that stay in view in quiet time; "joy", "personal", "family", "friends", "leisure" when unsaid.
    #[serde(default)]
    pub personal: Option<Vec<String>>,
    /// Task categories that are work: never in view in quiet time; "work", "travail", "pro", "client" when unsaid.
    #[serde(default)]
    pub work: Option<Vec<String>>,
}

impl QuietSettings {
    pub fn personal_categories(&self) -> Vec<String> {
        self.personal.clone().unwrap_or_else(|| ["joy", "personal", "family", "friends", "leisure", "health", "perso", "famille", "amis", "loisirs", "santé"].iter().map(|s| s.to_string()).collect())
    }

    pub fn work_categories(&self) -> Vec<String> {
        self.work.clone().unwrap_or_else(|| ["work", "travail", "pro", "client", "boulot"].iter().map(|s| s.to_string()).collect())
    }
}

impl Config {
    /// Working hours, for quiet time: the days written with an end (by the week's
    /// editor). Older admin windows, a start and minutes, keep opening the Porch
    /// and leave quiet time off until working hours are set.
    pub fn working_hours(&self) -> Vec<AdminWindow> {
        self.windows.iter().filter(|w| w.end.is_some() && w.kind() == "work").cloned().collect()
    }

    /// The week's hours, of every kind: work, admin, leisure (docs/areas.md).
    pub fn week_hours(&self) -> Vec<AdminWindow> {
        self.windows.iter().filter(|w| w.end.is_some()).cloned().collect()
    }

    /// When offices are open: the configuration's, else Monday to Friday 9:00–17:00.
    pub fn office_hours(&self) -> Vec<AdminWindow> {
        if self.office_hours.is_empty() { crate::window::default_office_hours() } else { self.office_hours.clone() }
    }
}

/// How long text reads (docs/client.md, "Calm first"): lines need air.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Reading {
    /// A font family; empty for the desktop's.
    #[serde(default)]
    pub family: String,
    #[serde(default = "Reading::default_size")]
    pub size: u32,
    /// Line height, as a multiple of the font's.
    #[serde(default = "Reading::default_spacing")]
    pub spacing: f64,
}

impl Reading {
    fn default_size() -> u32 {
        16
    }

    fn default_spacing() -> f64 {
        1.5
    }
}

impl Default for Reading {
    fn default() -> Reading {
        Reading { family: String::new(), size: Reading::default_size(), spacing: Reading::default_spacing() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MailSettings {
    /// Folders shown by conversation, your answers from Sent among them.
    #[serde(default)]
    pub threads: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct TaskSettings {
    /// The list new tasks go into: "account/id".
    #[serde(default)]
    pub list: Option<String>,
    /// Minutes counted for a task without an estimate.
    #[serde(default)]
    pub estimate: Option<u32>,
    /// The kinds of task, as you named them (`[[tasks.kind]]`); Sioul's when unsaid.
    #[serde(rename = "kind", default)]
    pub kinds: Option<Vec<TaskKind>>,
}

/// A kind of task, as you named it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TaskKind {
    /// Written in the task's concept: "call", "errand".
    pub id: String,
    /// Empty for Sioul's own kinds: named in your language.
    #[serde(default)]
    pub label: String,
}

/// Sioul's kinds, before you name your own.
fn default_kinds() -> Vec<TaskKind> {
    crate::tasks::KINDS.iter().map(|k| TaskKind { id: k.to_string(), label: String::new() }).collect()
}

impl Config {
    /// The kinds of task, id and name: yours as you named them, else Sioul's in your language.
    pub fn task_kinds(&self, tr: &crate::i18n::Translator) -> Vec<(String, String)> {
        self.tasks
            .kinds
            .clone()
            .unwrap_or_else(default_kinds)
            .into_iter()
            .filter(|k| crate::tasks::is_kind_id(&k.id))
            .map(|k| {
                let label = if k.label.trim().is_empty() { tr.text(&format!("task-kind-{}", k.id), None) } else { k.label.trim().to_string() };
                (k.id, label)
            })
            .collect()
    }
}

/// A kind renamed (`from` and `to`), taken away (`to` empty) or added (`from`
/// empty, `to` its name). Tasks keep the kind they have: taking a kind away
/// only takes it off the choices.
pub fn change_kind(path: &Path, config: &Config, from: &str, to: &str) -> Result<(), String> {
    let mut kinds = config.tasks.kinds.clone().unwrap_or_else(default_kinds);
    let to = to.trim();
    if from.is_empty() {
        if to.is_empty() {
            return Ok(());
        }
        let taken: Vec<String> = kinds.iter().map(|k| k.id.clone()).collect();
        kinds.push(TaskKind { id: crate::cases::new_id(to, &taken), label: to.to_string() });
    } else if to.is_empty() {
        kinds.retain(|k| k.id != from);
    } else {
        let kind = kinds.iter_mut().find(|k| k.id == from).ok_or_else(|| format!("{from}: no such kind"))?;
        kind.label = to.to_string();
    }
    let mut doc = read_document(path)?;
    if !doc.contains_key("tasks") {
        doc["tasks"] = toml_edit::table();
    }
    let tasks = doc["tasks"].as_table_mut().ok_or_else(|| format!("{}: tasks is not a table", path.display()))?;
    if kinds.is_empty() {
        tasks["kind"] = toml_edit::value(toml_edit::Array::new());
    } else {
        let mut list = toml_edit::ArrayOfTables::new();
        for kind in &kinds {
            let mut table = toml_edit::Table::new();
            table["id"] = toml_edit::value(kind.id.as_str());
            if !kind.label.is_empty() {
                table["label"] = toml_edit::value(kind.label.as_str());
            }
            list.push(table);
        }
        tasks["kind"] = Item::ArrayOfTables(list);
    }
    write_document(path, &doc)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AgendaSettings {
    /// The hour the day planning opens on.
    #[serde(default)]
    pub day_start: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MapSettings {
    /// Raster tiles, "https://…/{z}/{x}/{y}.png"; OpenStreetMap's when unset.
    #[serde(default)]
    pub tiles: Option<String>,
    /// Postal addresses may be sent to OpenStreetMap's geocoder, once each, to place contacts.
    #[serde(default)]
    pub geocode: bool,
}

/// Paper letters (docs/porch.md): the folder their scans arrive in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct LetterSettings {
    /// Unset: `letters/inbox` in the case store.
    #[serde(default)]
    pub inbox: Option<String>,
}

/// Reminders before dates (docs/reminders.md): one quiet notification each, never again.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReminderSettings {
    /// Events: at the end of the working day before, and at the alarms they carry.
    #[serde(default = "yes")]
    pub events: bool,
    /// A date asked: so many working days before, when work starts; 0 for none.
    #[serde(default = "two")]
    pub asked_days: u32,
    /// A payment planned: so many working days before, when work starts; 0 for none.
    #[serde(default = "two")]
    pub payment_days: u32,
    /// A wait after a step done (an answer due), once it is over.
    #[serde(default = "yes")]
    pub waits: bool,
    /// What sites notified, gathered in one notification at set times (a site
    /// in real time, and a call, come at once).
    #[serde(default = "yes")]
    pub gather: bool,
    /// The times of the gathered notification: "09:00"; unsaid, three a day.
    #[serde(default)]
    pub gathered: Option<Vec<String>>,
}

impl ReminderSettings {
    /// When sites' notifications are gathered: yours, else 9:00, 13:00 and 18:00
    /// (three a day helped most in a field trial: Fitz et al. 2019).
    pub fn gathered_times(&self) -> Vec<String> {
        self.gathered.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| ["09:00", "13:00", "18:00"].iter().map(|t| t.to_string()).collect())
    }
}

impl Default for ReminderSettings {
    fn default() -> Self {
        ReminderSettings { events: true, asked_days: 2, payment_days: 2, waits: true, gather: true, gathered: None }
    }
}

fn two() -> u32 {
    2
}

/// What GitHub brings, once asked; its token is in the keyring.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GithubSettings {
    #[serde(default)]
    pub enabled: bool,
    /// Issues and pull requests assigned to you.
    #[serde(default = "yes")]
    pub assigned: bool,
    /// Pull requests whose review is asked of you.
    #[serde(default = "yes")]
    pub reviews: bool,
    /// What you opened.
    #[serde(default)]
    pub created: bool,
    /// Where you are mentioned.
    #[serde(default)]
    pub mentioned: bool,
}

fn yes() -> bool {
    true
}

impl Default for GithubSettings {
    fn default() -> Self {
        GithubSettings { enabled: false, assigned: true, reviews: true, created: false, mentioned: false }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct BitwardenSettings {
    /// The account's e-mail.
    #[serde(default)]
    pub email: Option<String>,
    /// "bitwarden.com" when unset, "bitwarden.eu", or your server's address.
    #[serde(default)]
    pub server: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct WeatherSettings {
    /// "Lyon, Auvergne-Rhône-Alpes, France".
    #[serde(default)]
    pub place: Option<String>,
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct InvoiceSettings {
    #[serde(default)]
    pub name: String,
    /// Postal address, on lines.
    #[serde(default)]
    pub address: String,
    /// SIRET, or the business number where you are.
    #[serde(default)]
    pub siret: String,
    /// The VAT line: "TVA non applicable, art. 293 B du CGI" for a French micro-entrepreneur.
    #[serde(default)]
    pub vat: String,
    /// Invoice numbers start with it: "2026-".
    #[serde(default)]
    pub prefix: String,
    /// An hour's fee when a project does not say.
    #[serde(default)]
    pub rate: f64,
    #[serde(default)]
    pub currency: String,
    /// Where the PDFs of invoices go; "~/Documents/Invoices" when unsaid.
    #[serde(default)]
    pub folder: String,
    /// Payment details printed at the bottom: IBAN, terms.
    #[serde(default)]
    pub payment: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Imap,
    Jmap,
    /// A site kept open in Sioul (`[[site]]`, docs/sites.md): the bank, the tax
    /// office, Proton without Bridge. Not an account: it holds no sign-in of
    /// Sioul's. A `[[site]]` says no kind.
    #[default]
    Portal,
    /// Contacts and calendars on a CardDAV and CalDAV server.
    Dav,
}

/// How the connection to the mail server is encrypted. There is no plain option.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    /// TLS from the first byte, usually port 993; preferred by RFC 8314 §3.
    #[default]
    Tls,
    /// A plain connection upgraded by STARTTLS before anything else is said, usually port 143.
    Starttls,
}

impl Security {
    pub fn as_str(self) -> &'static str {
        match self {
            Security::Tls => "tls",
            Security::Starttls => "starttls",
        }
    }
}

/// How an account's mail ranks against the others'.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Its mail comes first in every lane.
    Above,
    #[default]
    Average,
    /// Its mail skips the screener and waits folded at the bottom of the Porch:
    /// notifications read sometimes, never answered. Codes still come at once.
    Below,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::Above => "above",
            Priority::Average => "average",
            Priority::Below => "below",
        }
    }

    pub fn parse(text: &str) -> Option<Priority> {
        match text.trim().to_ascii_lowercase().as_str() {
            "above" => Some(Priority::Above),
            "average" => Some(Priority::Average),
            "below" => Some(Priority::Below),
            _ => None,
        }
    }
}

/// Days of mail the first sync brings, when the account does not say.
pub const DEFAULT_SYNC_DAYS: u32 = 14;

#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    pub id: String,
    #[serde(default)]
    pub kind: AccountKind,
    /// Switched off in Accounts: kept with its settings, neither synced nor shown.
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub address: Option<String>,
    /// Where this account's mail is kept on disk (Maildir); see [`Account::maildir_path`].
    #[serde(default)]
    pub maildir: Option<String>,
    /// For portals: the address of the web mailbox.
    #[serde(default)]
    pub url: Option<String>,
    /// For portals, what the site is: "mailbox" (a secure mailbox), "chat", "dating", "other".
    #[serde(default)]
    pub site: Option<String>,
    /// For portals: its notifications shown at once, not at your pace.
    #[serde(default)]
    pub realtime: bool,
    /// For portals: its sounds silenced.
    #[serde(default)]
    pub muted: bool,
    /// For portals: kept open while the window is, to receive its notifications.
    #[serde(default)]
    pub background: Option<bool>,
    /// For portals: the microphone, the camera, sharing the screen, for calls;
    /// unsaid, as its kind goes (chats, video calls and dating sites have them).
    #[serde(default)]
    pub microphone: Option<bool>,
    #[serde(default)]
    pub camera: Option<bool>,
    #[serde(default)]
    pub screen: Option<bool>,
    /// For portals: your own categories, any words ("Banque", "Santé"): the list filters by them.
    #[serde(default)]
    pub categories: Vec<String>,
    /// For portals: the domains whose mail says something waits there ("you have a new
    /// message in your secure mailbox"); the site's own domain when unsaid.
    #[serde(default)]
    pub announced_by: Vec<String>,
    /// "personal" for an address, or a site, of yours outside work: it stays in view in quiet time.
    #[serde(default)]
    pub area: Option<String>,
    /// The authserv-id your provider writes in Authentication-Results (RFC 8601).
    #[serde(default)]
    pub trusted_authserv_ids: Vec<String>,
    /// The IMAP server, found from the address when the account is added.
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub security: Security,
    /// The login, when it is not the address.
    #[serde(default)]
    pub username: Option<String>,
    /// How many days of mail the first sync brings.
    #[serde(default)]
    pub sync_days: Option<u32>,
    /// How it signs in: "google" for Google's OAuth (its tokens in the keyring); a password otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
    /// Folders not kept here, by their server names: they stay on the server only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_folders: Vec<String>,
    /// How far back this account's mail reaches, in weeks, 0 for everything;
    /// the configuration's `history_weeks` when unset.
    #[serde(default)]
    pub history_weeks: Option<u32>,
    /// How its mail ranks against the other accounts'.
    #[serde(default)]
    pub priority: Priority,
    /// Your name as the From line shows it.
    #[serde(default)]
    pub name: Option<String>,
    /// The SMTP submission server, found from the address when first needed.
    #[serde(default)]
    pub smtp_host: Option<String>,
    #[serde(default)]
    pub smtp_port: Option<u16>,
    #[serde(default)]
    pub smtp_security: Security,
    /// Added below what you write, in Markdown.
    #[serde(default)]
    pub signature: Option<String>,
    /// Minutes between two fetches of this account, when it should come more or less often than the rest.
    #[serde(default)]
    pub fetch_minutes: Option<u32>,
    /// A public address that may bring hostile mail: its mail is read first, held, and summarised (`shield`).
    #[serde(default)]
    pub shield: bool,
    /// For a shielded account, an AI may read its new mail to say its tone and topic (docs/ai.md).
    #[serde(default)]
    pub shield_ai: bool,
}

impl Account {
    /// How far back this account's mail reaches, in days; None for everything.
    pub fn history_days(&self) -> Option<i64> {
        history_days(self.history_weeks)
    }

    /// A mail account fetched over IMAP; `username` only when it differs from the address.
    pub fn imap(id: &str, address: &str, host: &str, port: u16, security: Security, username: Option<&str>) -> Account {
        Account {
            id: id.to_string(),
            kind: AccountKind::Imap,
            enabled: true,
            address: Some(address.to_string()),
            maildir: None,
            url: None,
            site: None,
            realtime: false,
            muted: false,
            background: None,
            microphone: None,
            camera: None,
            screen: None,
            categories: Vec::new(),
            announced_by: Vec::new(),
            area: None,
            auth: None,
            skip_folders: Vec::new(),
            trusted_authserv_ids: Vec::new(),
            host: Some(host.to_string()),
            port: Some(port),
            security,
            username: username.filter(|u| *u != address).map(str::to_string),
            sync_days: None,
            history_weeks: None,
            priority: Priority::Average,
            name: None,
            smtp_host: None,
            smtp_port: None,
            smtp_security: Security::Tls,
            signature: None,
            fetch_minutes: None,
            shield: false,
            shield_ai: false,
        }
    }

    /// The Maildir of this account: `maildir` if set, else `~/.local/share/sioul/mail/<id>`.
    pub fn maildir_path(&self) -> PathBuf {
        self.maildir.as_deref().map_or_else(|| data_dir().join("mail").join(&self.id), expand_home)
    }

    /// The name to log in with: `username`, else the address.
    pub fn login(&self) -> Option<&str> {
        self.username.as_deref().or(self.address.as_deref())
    }

    /// The port to use: `port`, else the usual one for the encryption.
    pub fn port_or_default(&self) -> u16 {
        self.port.unwrap_or(match self.security {
            Security::Tls => 993,
            Security::Starttls => 143,
        })
    }

    /// The SMTP port to use: `smtp_port`, else the usual one for the encryption (RFC 8314).
    pub fn smtp_port_or_default(&self) -> u16 {
        self.smtp_port.unwrap_or(match self.smtp_security {
            Security::Tls => 465,
            Security::Starttls => 587,
        })
    }

    /// Whether Sioul fetches this account's mail itself.
    pub fn syncs(&self) -> bool {
        self.kind == AccountKind::Imap && self.host.is_some()
    }

    /// A contacts-and-calendars account: `host` names its keyring entry, `url` is where its server was found.
    pub fn dav(id: &str, address: &str, host: &str, url: Option<&str>, username: Option<&str>) -> Account {
        Account {
            kind: AccountKind::Dav,
            url: url.map(str::to_string),
            port: None,
            ..Account::imap(id, address, host, 0, Security::Tls, username)
        }
    }

    /// Whether Sioul syncs this account's contacts and calendars.
    pub fn is_dav(&self) -> bool {
        self.kind == AccountKind::Dav && self.host.is_some()
    }
}

/// A folder of mail to read, the account it belongs to, and the provider ids to trust for it.
#[derive(Debug, Clone)]
pub struct Source {
    pub account: Option<String>,
    /// The account's own address.
    pub address: Option<String>,
    pub folder: PathBuf,
    pub trusted_ids: Vec<String>,
    pub priority: Priority,
    /// A public address whose mail is read first (`shield`).
    pub shielded: bool,
    /// Words that make a sender automatic: the configuration's `filed_words`.
    pub filed_words: Vec<String>,
}

impl Config {
    /// The mail account a stored message belongs to, from where its file is.
    pub fn account_of(&self, file: &Path) -> Option<&Account> {
        self.accounts.iter().filter(|a| a.syncs()).find(|a| file.starts_with(a.maildir_path()))
    }

    /// Reads a configuration file; each account takes the general history
    /// unless it has its own. Sites written as accounts by older files join
    /// the sites; accounts switched off are set aside.
    pub fn load(path: &Path) -> Result<Config, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut config: Config = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        config.settle();
        Ok(config)
    }

    /// What `load` does once the file is read: sites apart, accounts switched off apart.
    pub fn settle(&mut self) {
        let (sites, accounts): (Vec<Account>, Vec<Account>) = std::mem::take(&mut self.accounts).into_iter().partition(|a| a.kind == AccountKind::Portal);
        let mut all_sites = sites;
        all_sites.append(&mut self.sites);
        for site in &mut all_sites {
            site.kind = AccountKind::Portal;
        }
        self.sites = all_sites;
        let (on, off): (Vec<Account>, Vec<Account>) = accounts.into_iter().partition(|a| a.enabled);
        self.accounts = on;
        self.accounts_off = off;
        let history = self.history_weeks;
        for account in self.accounts.iter_mut().chain(self.accounts_off.iter_mut()) {
            account.history_weeks = account.history_weeks.or(history);
        }
    }

    /// The site with this id.
    pub fn site(&self, id: &str) -> Option<&Account> {
        self.sites.iter().find(|s| s.id == id)
    }

    /// Every account, those switched off too, in the file's order of each.
    pub fn every_account(&self) -> impl Iterator<Item = &Account> {
        self.accounts.iter().chain(self.accounts_off.iter())
    }

    /// How far back mail and the agenda reach, in days; None for everything.
    pub fn history_days(&self) -> Option<i64> {
        history_days(self.history_weeks)
    }

    /// The case store's folder, with `~` expanded. On Android, unset: a folder
    /// of the app's own storage, which no other app, nor any sync, reads.
    pub fn case_store_path(&self) -> Option<PathBuf> {
        self.case_store.as_deref().map(expand_home).or_else(|| cfg!(target_os = "android").then(|| expand_home("~/Notes")))
    }

    /// The known senders' file: `known_senders`, else `known-senders.txt` next to the configuration.
    pub fn known_senders_path(&self) -> PathBuf {
        self.known_senders.as_deref().map_or_else(|| config_dir().join("known-senders.txt"), expand_home)
    }

    /// The blocked senders' file: `blocked_senders`, else `blocked-senders.txt` next to the configuration.
    pub fn blocked_senders_path(&self) -> PathBuf {
        self.blocked_senders.as_deref().map_or_else(|| config_dir().join("blocked-senders.txt"), expand_home)
    }

    /// Who is safe (their mail reaches you at any hour): `safe-senders.txt` next to the configuration.
    pub fn safe_senders_path(&self) -> PathBuf {
        config_dir().join("safe-senders.txt")
    }

    /// Who is neutral by your word, where a broader safe or blocked entry would name them otherwise.
    pub fn neutral_senders_path(&self) -> PathBuf {
        config_dir().join("neutral-senders.txt")
    }

    /// Every authserv-id of every account, for messages read outside an account.
    pub fn all_trusted_ids(&self) -> Vec<String> {
        self.accounts.iter().flat_map(|a| a.trusted_authserv_ids.iter().cloned()).collect()
    }

    /// The account with this id.
    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// The mail folders of every account that has mail on disk (portals have none).
    /// Sioul's own checks are trusted first, then the provider's.
    pub fn mail_sources(&self) -> Vec<Source> {
        let own = sioul_authserv_id();
        self.accounts
            .iter()
            .filter(|a| a.kind != AccountKind::Portal)
            .map(|a| Source {
                account: Some(a.id.clone()),
                address: a.address.clone(),
                folder: a.maildir_path(),
                trusted_ids: std::iter::once(own.clone()).chain(a.trusted_authserv_ids.iter().cloned()).collect(),
                priority: a.priority,
                shielded: a.shield,
                filed_words: self.filed_words.clone().unwrap_or_default(),
            })
            .collect()
    }

    /// An id for a new account from its address, unused yet: "example", then "example-someone".
    pub fn free_id(&self, address: &str) -> String {
        let (local, domain) = address.rsplit_once('@').unwrap_or((address, address));
        let label = domain.split('.').next().unwrap_or(domain);
        [label.to_string(), format!("{label}-{local}")]
            .into_iter()
            .chain((2..).map(|n| format!("{label}-{n}")))
            .map(|id| slug(&id))
            .find(|id| self.every_account().all(|a| &a.id != id))
            .unwrap_or_default()
    }
}

/// Lowercase letters, digits and dashes: safe as a folder name and a TOML value.
pub fn slug(text: &str) -> String {
    let mapped: String = text.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    mapped.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

/// Adds a mail account at the end of the configuration file, creating the file if needed.
pub fn add_imap_account(path: &Path, account: &Account) -> Result<(), String> {
    let host = account.host.as_deref().ok_or_else(|| format!("{}: no server", account.id))?;
    let mut table = Table::new();
    table["id"] = value(account.id.as_str());
    table["kind"] = value("imap");
    if let Some(address) = &account.address {
        table["address"] = value(address.as_str());
    }
    table["host"] = value(host);
    table["port"] = value(i64::from(account.port_or_default()));
    table["security"] = value(account.security.as_str());
    if let Some(username) = &account.username {
        table["username"] = value(username.as_str());
    }
    if let Some(days) = account.sync_days {
        table["sync_days"] = value(i64::from(days));
    }
    table["trusted_authserv_ids"] = value(account.trusted_authserv_ids.iter().map(String::as_str).collect::<Array>());
    append_account(path, table)
}

/// Adds a contacts-and-calendars account at the end of the configuration file.
pub fn add_dav_account(path: &Path, account: &Account) -> Result<(), String> {
    let host = account.host.as_deref().ok_or_else(|| format!("{}: no server", account.id))?;
    let mut table = Table::new();
    table["id"] = value(account.id.as_str());
    table["kind"] = value("dav");
    if let Some(address) = &account.address {
        table["address"] = value(address.as_str());
    }
    table["host"] = value(host);
    if let Some(url) = &account.url {
        table["url"] = value(url.as_str());
    }
    if let Some(username) = &account.username {
        table["username"] = value(username.as_str());
    }
    if let Some(auth) = &account.auth {
        table["auth"] = value(auth.as_str());
    }
    append_account(path, table)
}

/// Adds a site: a web-only mailbox, a chat, opened in Sioul's Sites.
pub fn add_portal(path: &Path, id: &str, url: &str) -> Result<(), String> {
    add_site(path, id, "", url)
}

/// Adds a site as the Sites page offers it: its name, address, type
/// ("mailbox", "chat"…), what it is for ("" for its type's), the domains whose
/// mail announces it, your categories ("Banque").
pub fn add_site_as(path: &Path, id: &str, name: &str, url: &str, kind: &str, area: &str, announced_by: &[String], categories: &[String]) -> Result<(), String> {
    let mut table = Table::new();
    table["id"] = value(id);
    if !name.trim().is_empty() {
        table["name"] = value(name.trim());
    }
    table["url"] = value(url);
    table["site"] = value(crate::sites::type_of(kind));
    if let Some(area) = crate::areas::Area::parse(area) {
        table["area"] = value(area.id());
    }
    if !announced_by.is_empty() {
        let mut list = toml_edit::Array::new();
        for domain in announced_by.iter().map(|d| d.trim().trim_start_matches('@').to_lowercase()).filter(|d| !d.is_empty()) {
            list.push(domain);
        }
        table["announced_by"] = value(list);
    }
    let categories: Vec<&str> = categories.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
    if !categories.is_empty() {
        let mut list = toml_edit::Array::new();
        for category in categories {
            list.push(category);
        }
        table["categories"] = value(list);
    }
    append_site(path, table)
}

/// Adds a site with the name it shows.
pub fn add_site(path: &Path, id: &str, name: &str, url: &str) -> Result<(), String> {
    let mut table = Table::new();
    table["id"] = value(id);
    if !name.trim().is_empty() {
        table["name"] = value(name.trim());
    }
    table["url"] = value(url);
    append_site(path, table)
}

/// Sites written as `[[account]]` of kind "portal" by older files, moved to
/// `[[site]]`, in their order, each with the comments above it; their kind
/// goes. Returns whether any moved.
pub fn migrate_sites(path: &Path) -> Result<bool, String> {
    let mut doc = read_document(path)?;
    let Some(accounts) = doc.get_mut("account").and_then(Item::as_array_of_tables_mut) else { return Ok(false) };
    let is_site = |t: &Table| t.get("kind").and_then(Item::as_str) == Some("portal");
    let moving: Vec<Table> = accounts.iter().filter(|t| is_site(t)).cloned().collect();
    if moving.is_empty() {
        return Ok(false);
    }
    accounts.retain(|t| !is_site(t));
    if accounts.is_empty() {
        doc.remove("account");
    }
    let sites = doc.entry("site").or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| format!("{}: `site` is not a list of tables", path.display()))?;
    for mut table in moving {
        table.remove("kind");
        sites.push(table);
    }
    write_document(path, &doc)?;
    Ok(true)
}

fn append_site(path: &Path, mut table: Table) -> Result<(), String> {
    migrate_sites(path)?;
    let mut doc = read_document(path)?;
    let trailing = doc.trailing().as_str().unwrap_or("").trim_end().to_string();
    table.decor_mut().set_prefix(if trailing.trim().is_empty() { "\n".to_string() } else { format!("{trailing}\n\n") });
    doc.set_trailing("");
    let sites = doc.entry("site").or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
    let sites = sites.as_array_of_tables_mut().ok_or_else(|| format!("{}: `site` is not a list of tables", path.display()))?;
    sites.push(table);
    write_document(path, &doc)
}

/// Moves a site `delta` places in the list: the order the Sites page shows. At either end, it stays.
pub fn move_site(path: &Path, id: &str, delta: i64) -> Result<(), String> {
    migrate_sites(path)?;
    let mut doc = read_document(path)?;
    let Some(sites) = doc.get_mut("site").and_then(Item::as_array_of_tables_mut) else { return Ok(()) };
    let mut tables: Vec<Table> = sites.iter().cloned().collect();
    let Some(from) = tables.iter().position(|t| t.get("id").and_then(Item::as_str) == Some(id)) else { return Ok(()) };
    let Some(to) = usize::try_from(from as i64 + delta).ok().filter(|to| *to < tables.len()) else { return Ok(()) };
    // The tables trade places; each keeps the comments above it.
    swap_tables(&mut tables, from, to);
    let mut moved = ArrayOfTables::new();
    for table in tables {
        moved.push(table);
    }
    *sites = moved;
    write_document(path, &doc)
}

/// Two tables trade places. Each read from the file keeps its place in it
/// (`Table::position`), which orders the file when written: the places stay
/// with the slots, or the file would come back in its old order.
fn swap_tables(tables: &mut [Table], from: usize, to: usize) {
    let (a, b) = (tables[from].position(), tables[to].position());
    tables.swap(from, to);
    tables[from].set_position(a);
    tables[to].set_position(b);
}

/// Takes a site out of the configuration; what its pages kept (sign-ins, cookies) stays in its profile.
pub fn remove_site(path: &Path, id: &str) -> Result<(), String> {
    migrate_sites(path)?;
    let mut doc = read_document(path)?;
    let Some(sites) = doc.get_mut("site").and_then(Item::as_array_of_tables_mut) else { return Ok(()) };
    let before = sites.len();
    sites.retain(|t| t.get("id").and_then(Item::as_str) != Some(id));
    if sites.len() == before {
        return Err(format!("{}: no site {id}", path.display()));
    }
    write_document(path, &doc)
}

/// An account switched on or off: off, it keeps its settings and is neither synced nor shown.
pub fn set_enabled(path: &Path, id: &str, on: bool) -> Result<(), String> {
    let mut doc = read_document(path)?;
    let table = doc
        .get_mut("account")
        .and_then(Item::as_array_of_tables_mut)
        .and_then(|accounts| accounts.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id)))
        .ok_or_else(|| format!("{}: no account {id}", path.display()))?;
    if on {
        table.remove("enabled");
    } else {
        table["enabled"] = value(false);
    }
    write_document(path, &doc)
}

fn append_account(path: &Path, mut table: Table) -> Result<(), String> {
    let mut doc = read_document(path)?;
    // Comments at the end of the file stay above what is appended, where they
    // were written: below it, a commented `language = …` would land inside the account.
    let trailing = doc.trailing().as_str().unwrap_or("").trim_end().to_string();
    table.decor_mut().set_prefix(if trailing.trim().is_empty() { "\n".to_string() } else { format!("{trailing}\n\n") });
    doc.set_trailing("");
    let accounts = doc.entry("account").or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
    let accounts = accounts.as_array_of_tables_mut().ok_or_else(|| format!("{}: `account` is not a list of tables", path.display()))?;
    accounts.push(table);
    write_document(path, &doc)
}

/// Moves an account `delta` places among those of its kind (sites among
/// sites): the order the lists show them in. At either end, it stays.
pub fn move_account(path: &Path, id: &str, delta: i64) -> Result<(), String> {
    let mut doc = read_document(path)?;
    let Some(accounts) = doc.get_mut("account").and_then(Item::as_array_of_tables_mut) else { return Ok(()) };
    let mut tables: Vec<Table> = accounts.iter().cloned().collect();
    let kind_of = |t: &Table| t.get("kind").and_then(Item::as_str).unwrap_or("").to_string();
    let Some(from) = tables.iter().position(|t| t.get("id").and_then(Item::as_str) == Some(id)) else { return Ok(()) };
    let kind = kind_of(&tables[from]);
    let same: Vec<usize> = tables.iter().enumerate().filter(|(_, t)| kind_of(t) == kind).map(|(i, _)| i).collect();
    let at = same.iter().position(|&i| i == from).unwrap_or(0) as i64;
    let Some(&to) = usize::try_from(at + delta).ok().and_then(|t| same.get(t)) else { return Ok(()) };
    // The tables trade places; each keeps the comments above it.
    swap_tables(&mut tables, from, to);
    let mut moved = ArrayOfTables::new();
    for table in tables {
        moved.push(table);
    }
    *accounts = moved;
    write_document(path, &doc)
}

/// Removes an account from the configuration. Its mail and its password are not touched here.
pub fn remove_account(path: &Path, id: &str) -> Result<(), String> {
    let mut doc = read_document(path)?;
    let accounts = doc.get_mut("account").and_then(Item::as_array_of_tables_mut);
    let Some(accounts) = accounts else { return Ok(()) };
    let index = accounts.iter().position(|t| t.get("id").and_then(Item::as_str) == Some(id));
    if let Some(index) = index {
        accounts.remove(index);
    }
    write_document(path, &doc)
}

/// Records the authserv-ids to trust for an account (learned at its first sync).
pub fn set_trusted_ids(path: &Path, id: &str, ids: &[String]) -> Result<(), String> {
    set_account_item(path, id, "trusted_authserv_ids", value(ids.iter().map(String::as_str).collect::<Array>()))
}

/// Records an account's sending server (found from its address when first needed).
pub fn set_smtp(path: &Path, id: &str, host: &str, port: u16, security: Security) -> Result<(), String> {
    set_account_item(path, id, "smtp_host", value(host))?;
    set_account_item(path, id, "smtp_port", value(i64::from(port)))?;
    set_account_item(path, id, "smtp_security", value(security.as_str()))
}

/// Days of history from a setting in weeks: two weeks by default, None for everything (0).
pub fn history_days(weeks: Option<u32>) -> Option<i64> {
    match weeks.unwrap_or(DEFAULT_HISTORY_WEEKS) {
        0 => None,
        weeks => Some(i64::from(weeks) * 7),
    }
}

/// Weeks of history when the configuration says nothing.
pub const DEFAULT_HISTORY_WEEKS: u32 = 2;

/// Minutes between two fetches of the folders the server does not push.
pub const DEFAULT_FETCH_MINUTES: u32 = 5;

/// Sets how far back mail and the agenda reach, in weeks (0 for everything).
pub fn set_history(path: &Path, weeks: u32) -> Result<(), String> {
    let mut doc = read_document(path)?;
    doc["history_weeks"] = value(i64::from(weeks));
    write_document(path, &doc)
}

/// Sets an account's priority; "average", the default, is written too, so the choice shows.
pub fn set_priority(path: &Path, id: &str, priority: Priority) -> Result<(), String> {
    set_account_item(path, id, "priority", value(priority.as_str()))
}

/// Sets your name as recipients see it and your signature (Markdown); empty removes them.
pub fn set_writing(path: &Path, id: &str, name: &str, signature: &str) -> Result<(), String> {
    let mut doc = read_document(path)?;
    let table = doc
        .get_mut("account")
        .and_then(Item::as_array_of_tables_mut)
        .and_then(|accounts| accounts.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id)))
        .ok_or_else(|| format!("{}: no account {id}", path.display()))?;
    for (key, text) in [("name", name.trim()), ("signature", signature.trim())] {
        if text.is_empty() {
            table.remove(key);
        } else {
            table[key] = value(text);
        }
    }
    write_document(path, &doc)
}

/// A setting's value, as the window gives it back.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SettingValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Ints(Vec<i64>),
    Texts(Vec<String>),
    Windows(Vec<WindowValue>),
    TimeOff(Vec<TimeOffValue>),
    Routes(Vec<crate::cases::RouteValue>),
    /// Names with their ids: kinds of task, categories. Shown, never read back.
    Named(Vec<NamedValue>),
    /// One name changed: renamed (`from`, `to`), taken away (`to` empty), added (`from` empty).
    Rename(RenameValue),
}

/// A name and what it stands for.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NamedValue {
    pub id: String,
    pub label: String,
    /// Why it cannot be renamed or taken away here (Google's calendars); empty when it can.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub locked: String,
    /// Said beside it, not part of its name: where a calendar lives.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// One name changed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenameValue {
    pub from: String,
    pub to: String,
}

/// Days off, as the settings edit them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimeOffValue {
    /// "2026-12-24".
    pub from: String,
    pub until: String,
    #[serde(default)]
    pub label: String,
}

/// A day's hours, as the settings edit them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WindowValue {
    pub day: String,
    pub start: String,
    /// "17:00".
    #[serde(default)]
    pub end: String,
    /// Older form of the end, read and not written.
    #[serde(default)]
    pub minutes: u32,
}

impl SettingValue {
    /// As a TOML item: an empty text or list removes the key (the default comes back).
    fn item(&self) -> Option<Item> {
        Some(match self {
            SettingValue::Bool(b) => value(*b),
            SettingValue::Int(n) => value(*n),
            SettingValue::Float(f) => value(*f),
            SettingValue::Text(t) if t.trim().is_empty() => return None,
            SettingValue::Text(t) => value(t.trim()),
            SettingValue::Ints(list) if list.is_empty() => return None,
            SettingValue::Ints(list) => value(list.iter().copied().collect::<Array>()),
            SettingValue::Texts(list) if list.is_empty() => return None,
            SettingValue::Texts(list) => value(list.iter().map(String::as_str).collect::<Array>()),
            SettingValue::Windows(_) | SettingValue::TimeOff(_) | SettingValue::Routes(_) | SettingValue::Named(_) | SettingValue::Rename(_) => return None,
        })
    }
}

/// Sets one setting in the file, in place, keeping its comments and order:
/// `history_weeks`, `reading.size` (a table's key), `account.<id>.priority`
/// (an account's), `window` (every admin window at once). An empty value
/// removes the key, so its default comes back.
pub fn set_value(path: &Path, key: &str, setting: &SettingValue) -> Result<(), String> {
    let mut doc = read_document(path)?;
    if key == "time_off" {
        let days: &[TimeOffValue] = match setting {
            SettingValue::TimeOff(days) => days,
            // An empty list reads as the first kind of list it fits, numbers: none.
            SettingValue::Ints(none) if none.is_empty() => &[],
            _ => return Err(format!("{key}: days off expected")),
        };
        let date = |text: &str| -> Result<toml_edit::Datetime, String> {
            let d: jiff::civil::Date = text.trim().parse().map_err(|_| format!("{text}: a day as 2026-12-24"))?;
            Ok(toml_edit::Datetime { date: Some(toml_edit::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None })
        };
        let mut tables = ArrayOfTables::new();
        for off in days {
            let mut table = Table::new();
            table["from"] = value(date(&off.from)?);
            table["until"] = value(date(&off.until)?);
            if !off.label.trim().is_empty() {
                table["label"] = value(off.label.trim());
            }
            tables.push(table);
        }
        if days.is_empty() {
            doc.remove(key);
        } else {
            doc[key] = Item::ArrayOfTables(tables);
        }
        return write_document(path, &doc);
    }
    if key == "window" || key == "window.admin" || key == "window.leisure" || key == "office_hours" {
        let windows: &[WindowValue] = match setting {
            SettingValue::Windows(windows) => windows,
            // An empty list reads as the first kind of list it fits, numbers: none.
            SettingValue::Ints(none) if none.is_empty() => &[],
            _ => return Err(format!("{key}: windows expected")),
        };
        // The week's hours of one kind: those of the other kinds stay as they are.
        let (table_key, kind) = match key {
            "window.admin" => ("window", "admin"),
            "window.leisure" => ("window", "leisure"),
            "window" => ("window", "work"),
            other => (other, ""),
        };
        let kind_of = |t: &Table| match t.get("kind").and_then(Item::as_str) {
            Some("admin") => "admin",
            Some("leisure") => "leisure",
            _ => "work",
        };
        let mut tables = ArrayOfTables::new();
        if !kind.is_empty()
            && let Some(old) = doc.get(table_key).and_then(Item::as_array_of_tables)
        {
            for table in old.iter().filter(|t| kind_of(t) != kind) {
                tables.push(table.clone());
            }
        }
        // One row a day: a day given twice keeps its first hours.
        let mut seen: Vec<String> = Vec::new();
        for w in windows {
            if seen.contains(&w.day) {
                continue;
            }
            seen.push(w.day.clone());
            let mut table = Table::new();
            table["day"] = value(w.day.as_str());
            table["start"] = value(w.start.as_str());
            if w.end.is_empty() {
                table["minutes"] = value(i64::from(w.minutes));
            } else {
                table["end"] = value(w.end.as_str());
            }
            if kind == "admin" || kind == "leisure" {
                table["kind"] = value(kind);
            }
            tables.push(table);
        }
        if tables.is_empty() {
            doc.remove(table_key);
        } else {
            doc[table_key] = Item::ArrayOfTables(tables);
        }
        return write_document(path, &doc);
    }
    // A site's field: in `[[site]]`, where an older file's sites are moved first.
    if let Some(rest) = key.strip_prefix("site.") {
        let (id, field) = rest.rsplit_once('.').ok_or_else(|| format!("{key}: site.<id>.<field> expected"))?;
        drop(doc);
        migrate_sites(path)?;
        let mut doc = read_document(path)?;
        let table = doc
            .get_mut("site")
            .and_then(Item::as_array_of_tables_mut)
            .and_then(|sites| sites.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id)))
            .ok_or_else(|| format!("{}: no site {id}", path.display()))?;
        match setting.item() {
            Some(item) => table[field] = item,
            None => {
                table.remove(field);
            }
        }
        return write_document(path, &doc);
    }
    if let Some(rest) = key.strip_prefix("account.") {
        let (id, field) = rest.rsplit_once('.').ok_or_else(|| format!("{key}: account.<id>.<field> expected"))?;
        let table = doc
            .get_mut("account")
            .and_then(Item::as_array_of_tables_mut)
            .and_then(|accounts| accounts.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id)))
            .ok_or_else(|| format!("{}: no account {id}", path.display()))?;
        match setting.item() {
            Some(item) => table[field] = item,
            None => {
                table.remove(field);
            }
        }
        return write_document(path, &doc);
    }
    match key.split_once('.') {
        Some((table, field)) => {
            if !doc.contains_key(table) {
                doc[table] = toml_edit::table();
            }
            // A table written inline (`reading = { size = 18 }`) is a table too.
            let section = doc[table].as_table_like_mut().ok_or_else(|| format!("{}: {table} is not a table", path.display()))?;
            match setting.item() {
                // In place: the comments above the key stay.
                Some(item) => match section.get_mut(field) {
                    Some(slot) => *slot = item,
                    None => {
                        section.insert(field, item);
                    }
                },
                None => {
                    section.remove(field);
                }
            }
        }
        None => match setting.item() {
            Some(item) => doc[key] = item,
            None => {
                doc.remove(key);
            }
        },
    }
    write_document(path, &doc)
}

fn set_account_item(path: &Path, id: &str, key: &str, item: Item) -> Result<(), String> {
    let mut doc = read_document(path)?;
    let table = doc
        .get_mut("account")
        .and_then(Item::as_array_of_tables_mut)
        .and_then(|accounts| accounts.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id)))
        .ok_or_else(|| format!("{}: no account {id}", path.display()))?;
    table[key] = item;
    write_document(path, &doc)
}

fn read_document(path: &Path) -> Result<DocumentMut, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::from("# Sioul configuration (examples/config.toml explains each setting).\n"),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    text.parse().map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes next to the file, then renames, so a crash never leaves half a
/// configuration. A configuration that is a link (kept in a dotfiles folder)
/// is written where it points, the link kept.
fn write_document(path: &Path, doc: &DocumentMut) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let target = std::fs::canonicalize(path).ok().filter(|_| path.is_symlink());
    let path = target.as_deref().unwrap_or(path);
    let text = doc.to_string();
    // What Sioul could read before, it reads after: a value of the wrong kind
    // (a number with decimals where a whole one goes) is refused, never written.
    if let Err(e) = toml::from_str::<Config>(&text)
        && std::fs::read_to_string(path).is_ok_and(|old| toml::from_str::<Config>(&old).is_ok())
    {
        return Err(format!("{}: {e}", path.display()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let temporary = path.with_extension("toml.new");
    std::fs::write(&temporary, text).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)
}

/// The name under which Sioul writes its own checks of a message (RFC 8601
/// authserv-id): unique to this installation and under `.invalid`, so no
/// sender can write results that pass for Sioul's. Made once, kept in the state folder.
pub fn sioul_authserv_id() -> String {
    let path = state_dir().join("authserv-id");
    if let Some(id) = std::fs::read_to_string(&path).ok().map(|t| t.trim().to_string()).filter(|t| t.ends_with(".invalid")) {
        return id;
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    path.hash(&mut hasher);
    let id = format!("sioul-{:012x}.invalid", hasher.finish() & 0xffff_ffff_ffff);
    let _ = std::fs::create_dir_all(state_dir()).and_then(|()| std::fs::write(&path, &id));
    id
}

/// Where the configuration lives by default.
pub fn default_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// `$XDG_CONFIG_HOME/sioul`, else `~/.config/sioul`.
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", "~/.config").join("sioul")
}

/// `$XDG_DATA_HOME/sioul`, else `~/.local/share/sioul`: the mail.
pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", "~/.local/share").join("sioul")
}

/// `$XDG_STATE_HOME/sioul`, else `~/.local/state/sioul`: where sync and the Porch stopped.
pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", "~/.local/state").join("sioul")
}

/// `$<variable>/sioul` when the variable is set, on every system (tests set
/// them); else Linux's XDG fallback, and the system's own folders on Windows
/// (`%APPDATA%`, `%LOCALAPPDATA%`) and macOS (`~/Library/Application Support`).
fn xdg(variable: &str, fallback: &str) -> PathBuf {
    if let Some(value) = std::env::var_os(variable).filter(|v| !v.is_empty()) {
        return PathBuf::from(value);
    }
    #[cfg(any(windows, target_os = "macos"))]
    if let Some(dirs) = directories::BaseDirs::new() {
        return match variable {
            "XDG_CONFIG_HOME" => dirs.config_dir().to_path_buf(),
            "XDG_CACHE_HOME" => dirs.cache_dir().to_path_buf(),
            "XDG_STATE_HOME" => dirs.data_local_dir().join("state"),
            _ => dirs.data_local_dir().to_path_buf(),
        };
    }
    expand_home(fallback)
}

/// `$XDG_CACHE_HOME/sioul`, else `~/.cache/sioul`: copies made to open attachments.
pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", "~/.cache").join("sioul")
}

/// Makes Sioul's own folders (configuration, data, state, cache) yours
/// alone, as they hold mail, keys, drafts and caches: made with mode 0700 on
/// Unix, and narrowed to it when they exist already. Their parents are made
/// as usual and never changed, and a folder that is a link is left as it is.
/// Nothing to do elsewhere: a Windows profile's folders are the user's own.
/// Called at start; what cannot be done is left, said by the first write that fails.
pub fn make_private_dirs() {
    #[cfg(unix)]
    for dir in [config_dir(), data_dir(), state_dir(), cache_dir()] {
        let _ = private_dir(&dir);
    }
}

/// One folder of Sioul's own, made or narrowed to 0700.
#[cfg(unix)]
fn private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(dir)?;
            if metadata.is_dir() && metadata.permissions().mode() & 0o077 != 0 {
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
            Ok(())
        }
        other => other,
    }
}

/// Where the desktop puts downloads: `XDG_DOWNLOAD_DIR` on Linux, the system's elsewhere.
pub fn downloads_dir() -> PathBuf {
    #[cfg(any(windows, target_os = "macos"))]
    if let Some(dir) = directories::UserDirs::new().and_then(|u| u.download_dir().map(Path::to_path_buf)) {
        return dir;
    }
    let dirs = expand_home("~/.config/user-dirs.dirs");
    let named = std::fs::read_to_string(dirs).ok().and_then(|text| {
        let line = text.lines().find(|l| l.trim_start().starts_with("XDG_DOWNLOAD_DIR="))?;
        let value = line.split_once('=')?.1.trim().trim_matches('"');
        Some(PathBuf::from(value.replace("$HOME", &std::env::var("HOME").unwrap_or_default())))
    });
    named.unwrap_or_else(|| expand_home("~/Downloads"))
}

/// Expands a leading `~` to the home folder (`USERPROFILE` on Windows):
/// "~/Notes", and "~\Notes" as Windows writes it.
pub fn expand_home(path: &str) -> PathBuf {
    // Windows' own first there: a Unix shell's HOME may read "/c/Users/…".
    let (first, second) = if cfg!(windows) { ("USERPROFILE", "HOME") } else { ("HOME", "USERPROFILE") };
    let home = std::env::var_os(first).or_else(|| std::env::var_os(second)).map(PathBuf::from);
    match (path.strip_prefix('~'), home) {
        // A rest starting with a separator would replace the home when joined.
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches(['/', '\\'])),
        _ => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_written_in_place() {
        let dir = std::env::temp_dir().join(format!("sioul-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# Mine.\ncase_store = \"~/Notes\"\n\n[[account]]\nid = \"a\"\nkind = \"imap\"\n").unwrap();
        set_value(&path, "reading.spacing", &SettingValue::Float(1.6)).unwrap();
        set_value(&path, "some_days", &SettingValue::Ints(vec![30, 30, 30, 30, 30, 0, 0])).unwrap();
        set_value(&path, "account.a.shield", &SettingValue::Bool(true)).unwrap();
        set_value(&path, "window", &SettingValue::Windows(vec![WindowValue { day: "tuesday".into(), start: "10:00".into(), end: "17:00".into(), minutes: 0 }])).unwrap();
        // Hours for admin beside working hours: each kind kept when the other is saved.
        set_value(&path, "window.admin", &SettingValue::Windows(vec![WindowValue { day: "tuesday".into(), start: "18:00".into(), end: "19:00".into(), minutes: 0 }])).unwrap();
        let both = Config::load(&path).unwrap();
        assert_eq!(both.working_hours().len(), 1);
        assert_eq!(both.week_hours().iter().filter(|w| w.kind() == "admin").count(), 1);
        set_value(&path, "window", &SettingValue::Windows(vec![WindowValue { day: "monday".into(), start: "09:00".into(), end: "17:00".into(), minutes: 0 }])).unwrap();
        let again = Config::load(&path).unwrap();
        assert_eq!(again.working_hours()[0].day, "monday");
        assert_eq!(again.week_hours().iter().filter(|w| w.kind() == "admin").map(|w| w.start.as_str()).collect::<Vec<_>>(), vec!["18:00"]);
        set_value(&path, "case_store", &SettingValue::Text("~/Notes".into())).unwrap();
        set_value(&path, "reading.size", &SettingValue::Int(18)).unwrap();
        set_value(&path, "reading.size", &SettingValue::Text(String::new())).unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!((config.reading.spacing, config.accounts[0].shield), (1.6, true));
        assert!(std::fs::read_to_string(&path).unwrap().contains("some_days = [30, 30, 30, 30, 30, 0, 0]"));
        assert_eq!((config.windows.len(), config.case_store.as_deref(), config.reading.size), (2, Some("~/Notes"), 16));
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("# Mine."));
        // The last day off, the last hours taken away: the window's JSON `[]` reads as numbers.
        let empty: SettingValue = serde_json::from_str("[]").unwrap();
        set_value(&path, "time_off", &SettingValue::TimeOff(vec![TimeOffValue { from: "2026-12-24".into(), until: "2026-12-26".into(), label: String::new() }])).unwrap();
        set_value(&path, "time_off", &empty).unwrap();
        set_value(&path, "window.admin", &empty).unwrap();
        let config = Config::load(&path).unwrap();
        assert!(config.time_off.is_empty() && config.week_hours().iter().all(|w| w.kind() != "admin"));
        // A table written inline takes a setting too, its comment above it kept.
        std::fs::write(&path, "# Mine.\n# Easier to read.\nreading = { size = 18 }\n").unwrap();
        set_value(&path, "reading.spacing", &SettingValue::Float(1.8)).unwrap();
        set_value(&path, "reading.size", &SettingValue::Int(20)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# Easier to read.\nreading = {"), "{text}");
        assert_eq!((Config::load(&path).unwrap().reading.size, Config::load(&path).unwrap().reading.spacing), (20, 1.8));
        // A value Sioul could not read back is refused: the file stays readable.
        assert!(set_value(&path, "reading.size", &SettingValue::Float(1.5)).is_err());
        assert_eq!(Config::load(&path).unwrap().reading.size, 20);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sites_keep_the_order_chosen() {
        let dir = std::env::temp_dir().join(format!("sioul-order-{}", std::process::id()));
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "# Mine.\n[[site]]\nid = \"a\"\nurl = \"https://a.example.org\"\n\n# The second.\n[[site]]\nid = \"b\"\nurl = \"https://b.example.org\"\n").unwrap();
        move_site(&path, "b", -1).unwrap();
        let ids = |path: &Path| Config::load(path).unwrap().sites.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&path), ["b", "a"]);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# The second.\n[[site]]\nid = \"b\""), "{text}");
        std::fs::write(&path, "[[account]]\nid = \"one\"\nkind = \"imap\"\n\n[[account]]\nid = \"two\"\nkind = \"imap\"\n").unwrap();
        move_account(&path, "two", -1).unwrap();
        assert_eq!(Config::load(&path).unwrap().accounts.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["two", "one"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A configuration kept in a dotfiles folder, linked from its place: written there, the link kept.
    #[cfg(unix)]
    #[test]
    fn a_linked_configuration_stays_linked() {
        let dir = std::env::temp_dir().join(format!("sioul-linked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("dotfiles")).unwrap();
        let (real, link) = (dir.join("dotfiles").join("config.toml"), dir.join("config.toml"));
        std::fs::write(&real, "# Mine.\n").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        set_history(&link, 4).unwrap();
        assert!(link.is_symlink());
        assert!(std::fs::read_to_string(&real).unwrap().contains("history_weeks = 4"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn history_goes_above_the_accounts() {
        let path = std::env::temp_dir().join(format!("sioul-history-{}.toml", std::process::id()));
        std::fs::write(&path, "# mine\nlanguage = \"fr\"\n\n[[account]]\nid = \"a\"\nkind = \"imap\"\n").unwrap();
        set_history(&path, 0).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.find("history_weeks = 0").unwrap() < text.find("[[account]]").unwrap(), "{text}");
        let config = Config::load(&path).unwrap();
        assert_eq!((config.history_days(), config.accounts[0].history_days()), (None, None));
        set_history(&path, 6).unwrap();
        assert_eq!(Config::load(&path).unwrap().accounts[0].history_days(), Some(42));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn kinds_named_your_way() {
        let path = std::env::temp_dir().join(format!("sioul-kinds-{}.toml", std::process::id()));
        std::fs::write(&path, "# mine\n[tasks]\nestimate = 20\n\n[[account]]\nid = \"a\"\nkind = \"imap\"\n").unwrap();
        let tr = crate::i18n::Translator::new("en");
        let ids = |config: &Config| config.task_kinds(&tr).into_iter().map(|(id, _)| id).collect::<Vec<_>>();
        let config = Config::load(&path).unwrap();
        assert_eq!(ids(&config).len(), crate::tasks::KINDS.len(), "Sioul's kinds until you name yours");
        change_kind(&path, &config, "", "Rendez-vous").unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(ids(&config).last().map(String::as_str), Some("rendez-vous"));
        change_kind(&path, &config, "call", "Phone").unwrap();
        change_kind(&path, &Config::load(&path).unwrap(), "think", "").unwrap();
        let config = Config::load(&path).unwrap();
        let kinds = config.task_kinds(&tr);
        assert!(kinds.contains(&("call".to_string(), "Phone".to_string())));
        assert!(kinds.contains(&("write".to_string(), "Writing".to_string())), "the others keep Sioul's names");
        assert!(!kinds.iter().any(|(id, _)| id == "think"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# mine") && text.contains("estimate = 20") && config.accounts.len() == 1, "{text}");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn reads_the_example() {
        let config: Config = toml::from_str(include_str!("../../../examples/config.toml")).unwrap();
        assert!(config.sites.iter().any(|s| s.id == "proton" && s.kind == AccountKind::Portal));
        assert_eq!((config.windows.len(), config.working_hours().len(), config.time_off.len()), (3, 3, 1));
        assert_eq!(config.account("personal").and_then(|a| a.area.as_deref()), Some("personal"));
        let gmail = config.account("gmail").unwrap();
        assert_eq!(gmail.port_or_default(), 993);
        assert!(gmail.syncs());
    }

    #[test]
    fn writes_accounts_and_keeps_comments() {
        let dir = std::env::temp_dir().join(format!("sioul-config-{}", std::process::id()));
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "# My settings.\ncase_store = \"~/Notes\"\n\n# Uncomment to choose:\n# language = \"en\"\n").unwrap();
        let account = Account::imap("example", "someone@example.org", "imap.example.org", 993, Security::Tls, Some("someone@example.org"));
        assert_eq!(account.username, None);
        add_imap_account(&path, &account).unwrap();
        add_portal(&path, "portal", "https://mail.example.net").unwrap();
        set_trusted_ids(&path, "example", &["mx.example.org".into()]).unwrap();
        set_priority(&path, "example", Priority::Below).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# My settings.\n"));
        assert!(text.find("# language").unwrap() < text.find("[[account]]").unwrap(), "{text}");
        // A site is no account: it has its own list.
        assert!(text.contains("[[site]]") && !text.contains("kind = \"portal\""), "{text}");
        let config = Config::load(&path).unwrap();
        assert_eq!((config.accounts.len(), config.sites.len()), (1, 1));
        assert_eq!(config.accounts[0].trusted_authserv_ids, vec!["mx.example.org"]);
        assert_eq!(config.accounts[0].priority, Priority::Below);
        assert_eq!(config.free_id("someone@example.org"), "example-someone");
        // Switched off: kept, set aside.
        set_enabled(&path, "example", false).unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!((config.accounts.len(), config.accounts_off.len()), (0, 1));
        assert_eq!(config.free_id("someone@example.org"), "example-someone", "an id stays taken while off");
        set_enabled(&path, "example", true).unwrap();
        remove_account(&path, "example").unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(config.accounts.len(), 0);
        assert_eq!(config.free_id("someone@example.org"), "example");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sites_moved_out_of_the_accounts() {
        let dir = std::env::temp_dir().join(format!("sioul-sites-{}", std::process::id()));
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "[[account]]\nid = \"mail\"\nkind = \"imap\"\nhost = \"imap.example.org\"\n\n# My bank.\n[[account]]\nid = \"bank\"\nkind = \"portal\"\nurl = \"https://bank.example.org\"\nrealtime = true\n").unwrap();
        // Read before moving: among the sites already.
        let config = Config::load(&path).unwrap();
        assert_eq!((config.accounts.len(), config.sites.len()), (1, 1));
        assert!(migrate_sites(&path).unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# My bank.\n[[site]]\nid = \"bank\"") && !text.contains("portal"), "{text}");
        assert!(!migrate_sites(&path).unwrap(), "once");
        set_value(&path, "site.bank.muted", &SettingValue::Bool(true)).unwrap();
        move_site(&path, "bank", -1).unwrap();
        let config = Config::load(&path).unwrap();
        assert!(config.site("bank").is_some_and(|s| s.muted && s.realtime));
        remove_site(&path, "bank").unwrap();
        assert!(Config::load(&path).unwrap().sites.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sioul_folders_are_yours_alone() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let root = std::env::temp_dir().join(format!("sioul-private-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // Made: 0700.
        let new = root.join("share").join("sioul");
        private_dir(&new).unwrap();
        assert_eq!(mode(&new), 0o700);
        // There already, open to others: narrowed; its parent is left as it was.
        let old = root.join("state").join("sioul");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(root.join("state"), std::fs::Permissions::from_mode(0o755)).unwrap();
        private_dir(&old).unwrap();
        assert_eq!((mode(&old), mode(&root.join("state"))), (0o700, 0o755));
        // A link to a folder elsewhere is left as its owner made it.
        let elsewhere = root.join("synced");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::set_permissions(&elsewhere, std::fs::Permissions::from_mode(0o755)).unwrap();
        let link = root.join("cache");
        std::fs::create_dir_all(&link).unwrap();
        std::os::unix::fs::symlink(&elsewhere, link.join("sioul")).unwrap();
        private_dir(&link.join("sioul")).unwrap();
        assert_eq!(mode(&elsewhere), 0o755);
        std::fs::remove_dir_all(&root).unwrap();
    }
}

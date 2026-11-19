// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's side that talks to the world: mail servers, calendars and
//! contacts, the keyring, other services (Google, GitHub, Bitwarden), and the
//! sharing between your devices.
//!
//! Fetching changes nothing on the server: folders are opened with EXAMINE
//! and messages fetched with BODY.PEEK, so read flags, folders and mail stay
//! as your other apps left them (RFC 9051 §6.3.3 and §6.4.5). The server is
//! written to only when you act ([`mailbox::act`], [`send`]). Connections are
//! encrypted from the first byte or upgraded by STARTTLS before the login;
//! there is no plain option.
//!
//! What to do with what comes is decided in
//! [sioul-core](../sioul_core/index.html): this crate brings what servers hold
//! into the plain files the core reads (a Maildir, vdir folders), and takes
//! back to them what you did. The window ([sioul-app](../sioul_app/index.html))
//! and the command line ([sioul](../sioul/index.html)) call it, and so does
//! the spam filter's training ([sioul-learn](../sioul_learn/index.html)) for
//! the mail it learns from.
//!
//! The functions that reach a server block until it answers: those that speak
//! IMAP make a small `tokio` runtime of their own and wait for it, those that
//! speak HTTP use `ureq`, which blocks. The window calls them off its own
//! thread.
//!
//! # Where to start reading
//!
//! - **Fetching mail**: [`fetch::sync`] fetches what is new since the last
//!   time; [`fetch::watch`] keeps the inbox open (IDLE) and fetches what
//!   arrives, until stopped.
//! - **Acting and sending**: [`mailbox::act`] (a message marked read, flagged,
//!   moved, put in the trash, on the server), [`send::send`].
//! - **What went wrong**: [`SyncError`], said in your language by
//!   [`SyncError::sentence`].
//! - **Contacts and calendars**: [`dav::sync`], both ways.
//! - **Sharing between devices**: [`share::exchange`], one exchange with the
//!   shared folder.
//!
//! Names such as docs/database.md are the design notes in the repository's
//! `docs/` folder. The website shows them too, under the same name:
//! <https://sioul.aurelienpierre.com/dev/database.html>.
//!
//! # Modules, by theme
//!
//! ## Mail
//!
//! - [`discover`](mod@discover): finding an account's servers from its address, as Thunderbird does.
//! - [`scout`]: what a server offers for one address: mail, calendars and contacts, a Nextcloud's apps.
//! - [`imap`]: an encrypted, logged-in IMAP session.
//! - [`sasl`]: signing in to IMAP and SMTP with an access token instead of a password (XOAUTH2).
//! - [`microsoft`]: Microsoft's mail (Outlook.com, Hotmail, Live, MSN, Microsoft 365), signed in with OAuth 2.0.
//! - `loopback` (not public): the door your browser comes back to after a sign-in on Google's or Microsoft's page.
//! - [`fetch`]: new mail fetched read-only into the account's Maildir, once or as it arrives.
//! - [`mailbox`]: an account's folders, and what you do to its messages on the server.
//! - [`search`]: searching the mail a server holds and this device does not; a message found there brought here.
//! - [`send`]: sending through SMTP submission, then a copy into Sent.
//! - [`verify`]: Sioul's own checks of who sent a message: SPF, DKIM, DMARC, ARC, reverse DNS.
//! - [`unsubscribe`]: leaving a mailing list in one click, the request itself (RFC 8058).
//! - [`keys`]: looking up someone's OpenPGP key, only when you ask.
//! - [`securitykey`]: your security key's certificate, looked up when you ask; GnuPG asked to let the key go.
//! - [`antivirus`]: checking a file before it opens.
//! - [`shield_ai`]: the AI reading of a shielded address, when you allow it.
//! - [`notify`]: the desktop's notifications: a verified code, a reminder, new mail, the time running.
//!
//! ## Contacts, calendars and other services
//!
//! - [`dav`]: contacts and calendars on a server (CardDAV, CalDAV), synced both ways with the folders on disk.
//! - [`google`]: Google's calendars and contacts, signed in with OAuth 2.0.
//! - [`google_tasks`]: Google Tasks, over its REST API.
//! - [`github`]: GitHub's REST API, read only: the issues and pull requests that are yours.
//! - [`bitwarden`]: a Bitwarden vault, read only: logins and their one-time codes, to fill a site's form.
//! - [`favicon`]: a site's own icon, asked of the site itself.
//! - [`geocode`]: where an address is, asked of OpenStreetMap's geocoder.
//! - [`weather`]: the weather, asked of Open-Meteo.
//! - [`ocr`]: a scan's text, read by the system's own programs.
//! - [`extensions`]: programs apart from Sioul run in a sandbox, what they may ask done for them, their signed list and their files.
//!
//! ## Passwords
//!
//! - [`secret`]: passwords in the system keyring, never in a file.
//! - `android`, built for Android only (so not shown here): Android's KeyStore and the
//!   network's DNS servers, reached through Java.
//!
//! ## Sharing between your devices
//!
//! - [`share`]: what only this device keeps, shared with your other devices through a folder your sync app carries.
//! - [`blobs`]: notes and papers in the shared folder, sealed, one file at a time.
//! - [`history`]: what a shared file held before another device's change, kept on this device.
//! - [`devices`]: each device says how it is: when it started, closed, last read and wrote.
//! - [`peers`]: what this device saw of the others, kept here: read for the table of your devices.
//! - [`device_table`]: every device of yours in one table, for the window and the command line.
//! - [`lease`]: one computer at a time for a part of the work: reminding doses, numbering invoices.
//! - [`remote`]: the shared folder read from its server too, when the sync app is late.
//! - [`loginflow`]: signing in to a Nextcloud through its own page in the browser, for a device joining the others.
//!
//! ## The computer
//!
//! - [`disk`]: the room left on a disk.
//! - [`dnd`]: the desktop's do-not-disturb during Sioul's pauses (Linux and the BSDs).
//! - [`power`]: on mains power or not, saving power or not, idle or not, a metered connection or not: for the spam filter's training by itself.

// The test feature accepts any server certificate: never in a build made for use.
#[cfg(all(feature = "insecure-test-tls", not(debug_assertions)))]
compile_error!("insecure-test-tls accepts any certificate: for test builds only, never with --release");

#[cfg(target_os = "android")]
pub mod android;
pub mod antivirus;
pub mod bitwarden;
pub mod blobs;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod bus;
pub mod calendars;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod colord;
pub mod dav;
pub mod device_grid;
pub mod device_table;
pub mod devices;
pub mod dispatch;
pub mod discover;
pub mod favicon;
// A calendar fetched by its web address, held to what a calendar needs (sioul-core's `subscribed`).
pub mod feeds;
pub mod disk;
// The desktop's do-not-disturb during Sioul's pauses (Plasma's inhibition, dconf's changes).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod dnd;
pub mod extensions;
pub mod fetch;
pub mod filters;
pub mod geocode;
pub mod github;
pub mod google;
pub mod google_tasks;
pub mod history;
// The encrypted, logged-in IMAP session: also the spam filter's corpus (sioul-learn) on desktops.
pub mod imap;
pub mod keys;
pub mod lease;
// Signing in to a Nextcloud through its own page in the browser (Login Flow v2): a new device joins through it.
pub mod loginflow;
// The door a browser comes back to after Google's or Microsoft's sign-in page (RFC 8252).
mod loopback;
pub mod mailbox;
// Microsoft's mail, signed in with OAuth 2.0 (docs/microsoft.md).
pub mod microsoft;
pub mod notify;
pub mod ocr;
pub mod peers;
pub mod photo;
// What the computer says of itself (power, saving, idle, a metered connection): the spam filter's training by itself.
pub mod power;
// The sharing folder fetched from its server too, beside the sync app (docs/database.md).
pub mod remote;
pub mod renumbered;
// Signing in to IMAP and SMTP with an access token (XOAUTH2): Google's mail, Microsoft's.
pub mod sasl;
pub mod scheduling;
pub mod scout;
pub mod search;
pub mod secret;
pub mod securitykey;
pub mod send;
pub mod share;
// Each wait for the network bounded, on ureq's transport (`ureq::unversioned`, no semver): this module alone.
mod stalls;
pub mod suggest;
pub mod trigger;
// texts: their lines sealed at rest (SMS phase b, docs/texts.md).
pub mod textseal;
pub mod shield_ai;
pub mod unsubscribe;
pub mod verify;
pub mod weather;

pub use discover::{Found, FoundBy, Smtp, discover, discover_smtp};
pub use fetch::{Control, Report, inbox, sync, test, watch};

use sioul_core::config::{self, Account, Config};
use sioul_core::i18n::{self, Translator};
use sioul_core::{maildir, trust};
use std::path::Path;

/// Where Google makes app passwords: Gmail refuses the usual password over IMAP.
pub const GMAIL_APP_PASSWORDS: &str = "https://myaccount.google.com/apppasswords";

/// Where Yahoo makes app passwords: the account's security page, under
/// "External connections" (Yahoo's help, SLN15241, read 17 November 2026).
pub const YAHOO_APP_PASSWORDS: &str = "https://login.yahoo.com/account/security";

/// Sioul's words for what this crate says itself (a hint, a page in your
/// browser): the configuration's language, else the session's, as the window.
pub(crate) fn translator() -> Translator {
    // Tests never read the configuration of whoever runs them.
    let configured = if cfg!(test) { None } else { Config::load(&config::default_path()).ok().and_then(|c| c.language) };
    Translator::new(&configured.unwrap_or_else(i18n::system_language))
}

/// Another program (a scanner, a reader of scans), run unseen: on Windows,
/// without a console window flashing over Sioul's (CREATE_NO_WINDOW); with
/// the core file limit the system gave, not Sioul's (`coredump::as_before`).
pub(crate) fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    sioul_core::coredump::as_before(&mut command);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x0800_0000);
        command
    };
    command
}

/// Whether an address is Gmail's.
pub fn is_gmail(address: &str) -> bool {
    let address = address.trim().to_ascii_lowercase();
    address.ends_with("@gmail.com") || address.ends_with("@googlemail.com")
}

/// The password as the server wants it. Google shows app passwords as four
/// groups of four letters ("abcd efgh ijkl mnop"); the spaces are not part
/// of it. Yahoo's servers take app passwords only, which hold no space
/// either: the same is done for them. How Yahoo shows its app passwords is
/// not verified here (no Yahoo account was used): sixteen letters, in one
/// group or four, are taken as Google's are.
pub fn tidy_password(host: &str, typed: &str) -> String {
    let groups: Vec<&str> = typed.split_whitespace().collect();
    let lengths: Vec<usize> = groups.iter().map(|g| g.len()).collect();
    let shaped = lengths == [16] || lengths == [4, 4, 4, 4];
    let letters = groups.iter().all(|g| g.chars().all(|c| c.is_ascii_alphabetic()));
    if AppPasswords::only_at(host).is_some() && shaped && letters { groups.concat() } else { typed.to_string() }
}

/// What learning the provider's authserv-id gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Learned {
    /// The account already trusts an id.
    Known,
    /// Learned and written into the configuration.
    Id(String),
    /// Enough mail, but no id holds a majority: the provider records no checks.
    Nothing,
    /// Too little mail to tell yet.
    TooEarly,
}

/// Learns which authserv-id the provider writes, from the account's mail, and
/// records it in the configuration (see `trust::learn_provider_id`).
pub fn learn_provider(config_path: &Path, account: &Account) -> Result<Learned, String> {
    if !account.trusted_authserv_ids.is_empty() {
        return Ok(Learned::Known);
    }
    let cards = maildir::read_messages(&account.maildir_path());
    match trust::learn_provider_id(cards.iter().map(|c| &c.headers)) {
        Some(id) => {
            config::set_trusted_ids(config_path, &account.id, std::slice::from_ref(&id))?;
            Ok(Learned::Id(id))
        }
        None if cards.len() >= 20 => Ok(Learned::Nothing),
        None => Ok(Learned::TooEarly),
    }
}

/// The sentences of `SyncError::Said`: what Sioul found itself, said in your
/// language, never in English inside a translated sentence.
pub mod said {
    /// A folder made without a name.
    pub const FOLDER_NAME: &str = "sync-said-folder-name";
    /// An item its server refused because it is an event in a list of tasks only (its title follows).
    pub const REFUSED_TASKS_ONLY: &str = "sync-said-refused-tasks-only";
    /// A task in a calendar of events only (its title follows).
    pub const REFUSED_EVENTS_ONLY: &str = "sync-said-refused-events-only";
    /// A task whose start and due are not of one value type, or whose due comes before its start (its title follows).
    pub const REFUSED_DATES: &str = "sync-said-refused-dates";
    /// Any other item its server refused (its title, then what the server said, follow).
    pub const REFUSED: &str = "sync-said-refused";
    /// No folder of that name on this account (the name follows).
    pub const NO_FOLDER: &str = "sync-said-no-folder";
    /// A folder Sioul keeps no copy of (the name follows).
    pub const NOT_KEPT: &str = "sync-said-not-kept";
    /// The inbox, Sent, Drafts, the trash, the junk or the archive: never deleted (the name follows).
    pub const PURPOSE: &str = "sync-said-purpose";
    /// A file that no fetch of Sioul's wrote.
    pub const NOT_FETCHED: &str = "sync-said-not-fetched";
    /// A file in no folder Sioul knows for the account.
    pub const UNKNOWN_FOLDER: &str = "sync-said-unknown-folder";
    /// The folder's UIDVALIDITY changed since the message was fetched.
    pub const RENUMBERED: &str = "sync-said-renumbered";
    /// The message is gone from the server.
    pub const GONE: &str = "sync-said-gone";
    /// No sending server found for an account's first message (its domain follows).
    pub const NO_SENDING_SERVER: &str = "sync-said-no-sending-server";
    /// A search for an address's servers cut short at `discover::WHOLE` (its domain follows).
    pub const SEARCH_SLOW: &str = "sync-said-search-slow";
    /// A message seen on the server only, to be brought here before this act.
    pub const OPEN_FIRST: &str = "sync-said-open-first";
}

/// What went wrong, in the categories the interface says in your language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// The text given is not an address.
    BadAddress,
    /// No settings found for this domain.
    NotFound(String),
    /// The account has no server in the configuration (a portal, or no `host`).
    NoServer,
    /// No password in the keyring for this account yet.
    NoPassword,
    /// The system keyring did not answer.
    Keyring(String),
    /// The server could not be reached, or the connection dropped.
    Network(String),
    /// The server's certificate could not be checked: nothing was sent.
    Tls(String),
    /// The server's certificate is for another name than the one asked:
    /// nothing was sent. The name asked is most often an alias of a host's
    /// server (`imap.<your domain>`), and the host's own name must be given
    /// instead. `asked`: the name asked; `presented`: the names the
    /// certificate is for, where they could be read, as plain words
    /// (`certificate_refused`). They come from a certificate nobody vouched
    /// for: they are shown, never offered to use.
    TlsName { asked: String, presented: String },
    /// The server refused the login.
    Login(String),
    /// The server wants an app password, not the account's usual one (Gmail,
    /// Yahoo, iCloud…); whose it is, so that the sentence names where it is made.
    AppPassword(AppPasswords, String),
    /// The server refused a command.
    Server(String),
    /// The mail or the sync state could not be written.
    Disk(String),
    /// The sending server refused the message.
    Refused(String),
    /// Sent, but its copy could not be filed in Sent.
    NotFiled(String),
    /// Sent, but not to each of its Bcc recipients: an encrypted message's
    /// copy of their own was refused. `missed`: those it did not reach, for
    /// a draft to them alone; `detail`: whom and why; `filed`: whether its
    /// copy went into Sent all the same.
    PartlySent { missed: Vec<String>, detail: String, filed: bool },
    /// The message could not be written (no recipient, an attachment gone): said as is.
    Message(String),
    /// A server's answer named an address on another server (a calendar, an
    /// item, the account's principal): it was not asked, and the login was
    /// not sent there.
    Elsewhere(String),
    /// What Sioul found itself, never a server's words: its sentence in your
    /// language (`said::RENUMBERED`, "sync-said-renumbered"), and what it
    /// names, if anything (a folder's name). Matched as `Server` was, which
    /// held these in English before (the review of 5 October 2026).
    Said(&'static str, String),
}

/// Whose server wants an app password: the sentence that says so names the
/// page where that provider makes one, and no page for a server Sioul does
/// not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPasswords {
    /// Google's mail: myaccount.google.com/apppasswords.
    Google,
    /// Yahoo's mail: its account's security page, "External connections".
    Yahoo,
    /// Another server, which asked for one in its own words.
    Elsewhere,
}

impl AppPasswords {
    /// What the dialog that asks a password again shows for this provider:
    /// its guide, its field's name (message ids), and the page where its app
    /// password is made; none for a server Sioul knows no page of.
    pub fn words(self) -> Option<(&'static str, &'static str, &'static str)> {
        match self {
            AppPasswords::Google => Some(("ui-gmail-app-password-guide", "ui-gmail-app-password-field", GMAIL_APP_PASSWORDS)),
            AppPasswords::Yahoo => Some(("ui-yahoo-app-password-guide", "ui-yahoo-app-password-field", YAHOO_APP_PASSWORDS)),
            AppPasswords::Elsewhere => None,
        }
    }

    /// The provider whose server `host` is, receiving or sending, when it
    /// takes app passwords only: Google's and Yahoo's, by their names.
    pub fn only_at(host: &str) -> Option<Self> {
        if host.eq_ignore_ascii_case(discover::GOOGLE_IMAP) || host.eq_ignore_ascii_case(discover::GOOGLE_SMTP) {
            Some(AppPasswords::Google)
        } else if discover::is_yahoo_host(host) {
            Some(AppPasswords::Yahoo)
        } else {
            None
        }
    }
}

/// What the account form says before a password is typed, for the server
/// found, with the page to open when there is one: where Google or Yahoo
/// makes the app password its server wants, else the general advice.
pub fn password_hint(host: &str) -> (&'static str, Option<&'static str>) {
    match AppPasswords::only_at(host) {
        Some(AppPasswords::Google) => ("account-gmail-hint", Some(GMAIL_APP_PASSWORDS)),
        Some(AppPasswords::Yahoo) => ("account-yahoo-hint", Some(YAHOO_APP_PASSWORDS)),
        Some(AppPasswords::Elsewhere) | None => ("account-app-password-hint", None),
    }
}

/// The most names of a refused certificate that are said.
const NAMES_SAID: usize = 3;

/// A certificate the TLS handshake with `host` refused, receiving or
/// sending. One that is valid but for another name gets its own sentence
/// (`SyncError::TlsName`): `host` is most often an alias of a host's server.
/// The names it is for are said when rustls gives them (the DNS names of
/// the certificate, a few of them, letters, digits, hyphens, dots and the
/// wildcard's star alone: they are words from outside). Any other refusal
/// is said with rustls's words.
pub(crate) fn certificate_refused(error: &rustls::Error, host: &str) -> SyncError {
    let asked = host.trim().trim_end_matches('.').to_ascii_lowercase();
    match error {
        rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForNameContext { presented, .. }) => {
            let names: Vec<&str> = presented
                .iter()
                .filter_map(|name| name.strip_prefix("DnsName(\"")?.strip_suffix("\")"))
                .filter(|name| !name.is_empty() && name.len() <= 253 && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'*')))
                .take(NAMES_SAID)
                .collect();
            SyncError::TlsName { asked, presented: names.join(", ") }
        }
        rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForName) => SyncError::TlsName { asked, presented: String::new() },
        other => SyncError::Tls(other.to_string()),
    }
}

/// A password the server `host` refused, receiving or sending, in the words
/// it `said`. Google's and Yahoo's servers take app passwords only: any
/// refusal there says where to make one. So does a server that asks for one
/// in its own words ("application-specific password required"), without a
/// page Sioul could name. Any other refusal is said with the server's words.
pub(crate) fn password_refused(host: &str, said: &str) -> SyncError {
    let detail = said.trim().to_string();
    let lower = detail.to_ascii_lowercase();
    match AppPasswords::only_at(host) {
        Some(whose) => SyncError::AppPassword(whose, detail),
        None if lower.contains("application-specific password") || lower.contains("app password") => SyncError::AppPassword(AppPasswords::Elsewhere, detail),
        None => SyncError::Login(detail),
    }
}

impl SyncError {
    /// The message that says it ("sync-error-network").
    /// What went wrong, as a code and never its detail (no address, no
    /// name): for the phone's log ("sync-error-login").
    pub fn code(&self) -> &'static str {
        self.message_id()
    }

    fn message_id(&self) -> &'static str {
        match self {
            SyncError::BadAddress => "sync-error-address",
            SyncError::NotFound(_) => "sync-error-not-found",
            SyncError::NoServer => "sync-error-no-server",
            SyncError::NoPassword => "sync-error-no-password",
            SyncError::Keyring(_) => "sync-error-keyring",
            SyncError::Network(_) => "sync-error-network",
            SyncError::Tls(_) => "sync-error-tls",
            SyncError::TlsName { presented, .. } if presented.is_empty() => "sync-error-tls-name",
            SyncError::TlsName { .. } => "sync-error-tls-names",
            SyncError::Login(d) if d == google::SIGN_IN_AGAIN => "sync-error-google-again",
            SyncError::Login(d) if d == google::IN_TESTING => "sync-error-google-testing",
            SyncError::Login(d) if d == github::TOKEN_REFUSED => "sync-error-github-token",
            // Microsoft's refusals, each with what to do (microsoft.rs).
            SyncError::Login(d) if microsoft::message_of(d).is_some() => microsoft::message_of(d).unwrap_or("sync-error-login"),
            SyncError::Login(_) => "sync-error-login",
            SyncError::AppPassword(AppPasswords::Google, _) => "sync-error-app-password-google",
            SyncError::AppPassword(AppPasswords::Yahoo, _) => "sync-error-app-password-yahoo",
            SyncError::AppPassword(AppPasswords::Elsewhere, _) => "sync-error-app-password",
            SyncError::Server(_) => "sync-error-server",
            SyncError::Disk(_) => "sync-error-disk",
            SyncError::Refused(_) => "send-error-refused",
            SyncError::NotFiled(_) => "send-error-not-filed",
            SyncError::PartlySent { filed: true, .. } => "send-error-partly-sent",
            SyncError::PartlySent { filed: false, .. } => "send-error-partly-sent-not-filed",
            SyncError::Message(_) => "send-error-message",
            SyncError::Elsewhere(_) => "sync-error-elsewhere",
            SyncError::Said(id, _) => id,
        }
    }

    /// The technical detail, shown after the sentence.
    pub fn detail(&self) -> &str {
        match self {
            SyncError::BadAddress | SyncError::NoServer | SyncError::NoPassword => "",
            SyncError::NotFound(d)
            | SyncError::Keyring(d)
            | SyncError::Network(d)
            | SyncError::Tls(d)
            | SyncError::TlsName { presented: d, .. }
            | SyncError::Login(d)
            | SyncError::AppPassword(_, d)
            | SyncError::Server(d)
            | SyncError::Disk(d)
            | SyncError::Refused(d)
            | SyncError::NotFiled(d)
            | SyncError::PartlySent { detail: d, .. }
            | SyncError::Message(d)
            | SyncError::Elsewhere(d)
            | SyncError::Said(_, d) => d,
        }
    }

    /// Whether trying again later can help: a refused password does not get
    /// better by retrying, and retries can lock the account.
    pub fn is_lasting(&self) -> bool {
        matches!(
            self,
            SyncError::Login(_) | SyncError::AppPassword(..) | SyncError::Tls(_) | SyncError::TlsName { .. } | SyncError::NoServer | SyncError::NoPassword | SyncError::BadAddress
        )
    }

    /// Whether a password, given again, can help: none kept here yet, or the
    /// server refused the one kept (or wants an app password).
    pub fn wants_password(&self) -> bool {
        matches!(self, SyncError::NoPassword | SyncError::Login(_) | SyncError::AppPassword(..))
    }

    /// The sentence, in your language: "home: the server cannot be reached (timed out)."
    pub fn sentence(&self, tr: &Translator, account: &str) -> String {
        let mut args = i18n::args();
        args.set("account", account.to_string());
        args.set("detail", self.detail().to_string());
        if let SyncError::TlsName { asked, presented } = self {
            args.set("host", asked.clone());
            args.set("names", presented.clone());
        }
        tr.text(self.message_id(), Some(&args))
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.message_id(), self.detail())
    }
}

impl std::error::Error for SyncError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_app_passwords_lose_their_spaces() {
        assert_eq!(tidy_password("imap.gmail.com", "abcd efgh ijkl mnop"), "abcdefghijklmnop");
        assert_eq!(tidy_password("imap.gmail.com", " abcdefghijklmnop "), "abcdefghijklmnop");
        // Anything else is kept as typed, spaces included.
        assert_eq!(tidy_password("imap.gmail.com", "my long pass phrase"), "my long pass phrase");
        assert_eq!(tidy_password("mail.example.org", "abcd efgh ijkl mnop"), "abcd efgh ijkl mnop");
        // Yahoo's servers, receiving and sending, take app passwords only: the same.
        assert_eq!(tidy_password("imap.mail.yahoo.com", "abcd efgh ijkl mnop"), "abcdefghijklmnop");
        assert_eq!(tidy_password("SMTP.mail.yahoo.com", "abcd efgh ijkl mnop"), "abcdefghijklmnop");
        assert_eq!(tidy_password("imap.mail.yahoo.com", "my long pass phrase"), "my long pass phrase");
        assert_eq!(AppPasswords::Yahoo.words().map(|(_, _, page)| page), Some(YAHOO_APP_PASSWORDS));
        assert_eq!(AppPasswords::Elsewhere.words(), None);
        assert!(is_gmail("Someone@GMail.com") && !is_gmail("someone@example.org"));
    }

    /// A password refused where only an app password gets in: the sentence
    /// names the page of the provider whose server refused, Google's for
    /// Google, Yahoo's for Yahoo with its section and buttons as Yahoo names
    /// them, and neither for another server, in English and in French. The
    /// form says the same before a password is typed, and the dialog that
    /// asks for a password again.
    #[test]
    fn an_app_password_s_sentence_names_where_its_provider_makes_it() {
        let (en, fr) = (Translator::new("en"), Translator::new("fr"));
        let google = password_refused("imap.gmail.com", "[AUTHENTICATIONFAILED] Invalid credentials (Failure)");
        let yahoo = password_refused("imap.mail.yahoo.com", "[AUTHENTICATIONFAILED] Authentication failed.");
        let other = password_refused("mail.example.org", "Application-specific password required");
        assert_eq!((google.code(), yahoo.code(), other.code()), ("sync-error-app-password-google", "sync-error-app-password-yahoo", "sync-error-app-password"));
        for error in [&google, &yahoo, &other] {
            assert!(error.is_lasting() && error.wants_password(), "{error}");
        }
        let yahoo_page = YAHOO_APP_PASSWORDS.trim_start_matches("https://");
        let google_page = GMAIL_APP_PASSWORDS.trim_start_matches("https://");
        assert_eq!((yahoo_page, google_page), ("login.yahoo.com/account/security", "myaccount.google.com/apppasswords"));
        let names = |sentence: &str, word: &str| sentence.to_lowercase().contains(word);
        for (tr, account, section, steps) in [
            (&en, "jane", "under \"External connections\"", ["\"Create app password\"", "\"Generate password\""]),
            (&fr, "jeanne", "sous «\u{202f}Connexions externes\u{202f}»", ["«\u{202f}Créer un mot de passe d’application\u{202f}»", "«\u{202f}Générer le mot de passe\u{202f}»"]),
        ] {
            let said = yahoo.sentence(tr, account);
            assert!(said.starts_with(account) && said.contains(yahoo_page) && said.contains(section) && steps.iter().all(|step| said.contains(step)), "{said}");
            assert!(!names(&said, "google") && !names(&said, "sync-error"), "{said}");
            let said = google.sentence(tr, account);
            assert!(said.starts_with(account) && said.contains(google_page) && !names(&said, "yahoo") && !names(&said, "sync-error"), "{said}");
            let said = other.sentence(tr, account);
            assert!(said.starts_with(account) && !names(&said, "google") && !names(&said, "gmail") && !names(&said, "yahoo") && !names(&said, "sync-error"), "{said}");
            // Before the password is typed, and when it is asked again.
            for id in ["account-yahoo-hint", "ui-yahoo-app-password-guide"] {
                let said = tr.text(id, None);
                assert!(said.contains(yahoo_page) && said.contains(section) && said.contains(steps[0]) && !names(&said, "google"), "{id}: {said}");
            }
            assert!(names(&tr.text("ui-yahoo-app-password-field", None), "yahoo"));
        }
        assert_eq!(password_hint("imap.gmail.com"), ("account-gmail-hint", Some(GMAIL_APP_PASSWORDS)));
        for host in ["imap.mail.yahoo.com", "Export.IMAP.Mail.Yahoo.com"] {
            assert_eq!(password_hint(host), ("account-yahoo-hint", Some(YAHOO_APP_PASSWORDS)), "{host}");
        }
        assert_eq!(password_hint("mail.example.org"), ("account-app-password-hint", None));
        assert_eq!((AppPasswords::only_at("smtp.gmail.com"), AppPasswords::only_at("smtp.mail.yahoo.com"), AppPasswords::only_at("smtp.example.org")), (Some(AppPasswords::Google), Some(AppPasswords::Yahoo), None));
    }

    /// A certificate that is valid, but for another name than the one asked:
    /// the name asked is most often an alias of a host's server. Its own
    /// sentence, in English and French, with the name asked and the names
    /// the certificate is for, as plain words (a few DNS names, nothing
    /// else a certificate could carry). It says to take the server's name
    /// from the provider, never from the certificate. Any other refusal of a
    /// certificate keeps the general sentence.
    #[test]
    fn a_certificate_for_another_name_says_the_name_is_an_alias() {
        let presented = ["DnsName(\"ssl0.host.example\")", "IpAddress(192.0.2.1)", "DnsName(\"<b>press here</b>\")", "DnsName(\"*.host.example\")", "DirectoryName", "DnsName(\"www.ssl0.host.example\")", "DnsName(\"fourth.host.example\")"];
        let other_name = rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForNameContext {
            expected: rustls::pki_types::ServerName::try_from("imap.example.org").unwrap(),
            presented: presented.iter().map(|name| name.to_string()).collect(),
        });
        let refused = certificate_refused(&other_name, " IMAP.Example.org. ");
        assert_eq!(refused, SyncError::TlsName { asked: "imap.example.org".into(), presented: "ssl0.host.example, *.host.example, www.ssl0.host.example".into() });
        assert!(refused.is_lasting() && !refused.wants_password(), "trying again cannot help, nor another password");
        assert_eq!(refused.code(), "sync-error-tls-names");
        let unnamed = certificate_refused(&rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForName), "smtp.example.org");
        assert_eq!(unnamed, SyncError::TlsName { asked: "smtp.example.org".into(), presented: String::new() });
        assert_eq!(unnamed.code(), "sync-error-tls-name");
        // Both say where to take the right name, that another server may have
        // answered, that a name being added is probably an alias, and what to
        // do for an address that worked before.
        for (language, account, words) in [
            ("en", "home", ["is probably an alias of your provider's server", "your mail provider's help", "another server may have answered", "If this address worked before", "try again later"]),
            ("fr", "maison", ["est sans doute un alias du serveur de votre fournisseur", "l’aide de votre fournisseur de courriel", "un autre serveur a pu répondre", "Si cette adresse marchait avant", "réessayez plus tard"]),
        ] {
            let tr = Translator::new(language);
            let said = refused.sentence(&tr, account);
            assert!(said.starts_with(account) && said.contains("imap.example.org") && said.contains("(ssl0.host.example, *.host.example, www.ssl0.host.example)"), "{said}");
            assert!(words.iter().all(|w| said.contains(w)) && !said.contains("sync-error") && !said.contains("press here"), "{said}");
            let said = unnamed.sentence(&tr, account);
            assert!(said.starts_with(account) && said.contains("smtp.example.org") && words.iter().all(|w| said.contains(w)) && !said.contains("()") && !said.contains("sync-error"), "{said}");
            // Where the settings came from, said before a password is typed.
            for by in [FoundBy::IspdbMx, FoundBy::Records] {
                let said = tr.text(by.message_id(), None);
                assert!(!said.is_empty() && !said.contains("account-by"), "{by:?}: {said}");
            }
        }
        // Another refusal of a certificate: the general sentence, with rustls's words.
        let unknown = certificate_refused(&rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer), "imap.example.org");
        assert!(matches!(&unknown, SyncError::Tls(words) if !words.is_empty()) && unknown.code() == "sync-error-tls");
    }

    /// What Sioul found itself is said in your language, never as English
    /// inside a translated sentence (the review of 5 October 2026): "the
    /// server renumbered this folder" was the detail of "le serveur a refusé
    /// une commande (…)".
    #[test]
    fn what_sioul_found_itself_is_said_in_your_language() {
        let fr = Translator::new("fr");
        let renumbered = SyncError::Said(said::RENUMBERED, String::new()).sentence(&fr, "travail");
        assert!(renumbered.starts_with("travail") && renumbered.contains("renuméroté") && !renumbered.contains("renumbered"), "{renumbered}");
        let folder = SyncError::Said(said::NO_FOLDER, "Banque".into()).sentence(&fr, "travail");
        assert!(folder.contains("«\u{202f}Banque\u{202f}»"), "{folder}");
        let en = Translator::new("en");
        assert_eq!(SyncError::Said(said::GONE, String::new()).sentence(&en, "work"), "work: this message is no longer on the server.");
        // Never a lasting error: trying again later may help.
        assert!(!SyncError::Said(said::RENUMBERED, String::new()).is_lasting());
    }
}

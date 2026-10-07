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
//! <https://aurelienpierre.github.io/sioul/dev/database.html>.
//!
//! # Modules, by theme
//!
//! ## Mail
//!
//! - [`discover`](mod@discover): finding an account's servers from its address, as Thunderbird does.
//! - [`scout`]: what a server offers for one address: mail, calendars and contacts, a Nextcloud's apps.
//! - [`imap`]: an encrypted, logged-in IMAP session.
//! - [`sasl`]: signing in to IMAP and SMTP with an access token instead of a password (XOAUTH2).
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
//! - [`lease`]: one computer at a time for a part of the work: reminding doses, numbering invoices.
//! - [`remote`]: the shared folder read from its server too, when the sync app is late.
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
pub mod dav;
pub mod devices;
pub mod discover;
pub mod favicon;
pub mod disk;
// The desktop's do-not-disturb during Sioul's pauses (Plasma's inhibition, dconf's changes).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub mod dnd;
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
pub mod mailbox;
pub mod notify;
pub mod ocr;
// What the computer says of itself (power, saving, idle, a metered connection): the spam filter's training by itself.
pub mod power;
// The sharing folder fetched from its server too, beside the sync app (docs/database.md).
pub mod remote;
// Signing in to IMAP and SMTP with an access token (XOAUTH2): Google's mail.
pub mod sasl;
pub mod scout;
pub mod search;
pub mod secret;
pub mod securitykey;
pub mod send;
pub mod share;
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

/// Sioul's words for what this crate says itself (a hint, a page in your
/// browser): the configuration's language, else the session's, as the window.
pub(crate) fn translator() -> Translator {
    // Tests never read the configuration of whoever runs them.
    let configured = if cfg!(test) { None } else { Config::load(&config::default_path()).ok().and_then(|c| c.language) };
    Translator::new(&configured.unwrap_or_else(i18n::system_language))
}

/// Another program (a scanner, a reader of scans), run unseen: on Windows,
/// without a console window flashing over Sioul's (CREATE_NO_WINDOW).
pub(crate) fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let command = std::process::Command::new(program);
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
/// groups of four letters ("abcd efgh ijkl mnop"); the spaces are not part of it.
pub fn tidy_password(host: &str, typed: &str) -> String {
    let groups: Vec<&str> = typed.split_whitespace().collect();
    let lengths: Vec<usize> = groups.iter().map(|g| g.len()).collect();
    let shaped = lengths == [16] || lengths == [4, 4, 4, 4];
    let letters = groups.iter().all(|g| g.chars().all(|c| c.is_ascii_alphabetic()));
    if host.eq_ignore_ascii_case("imap.gmail.com") && shaped && letters { groups.concat() } else { typed.to_string() }
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
    /// The server refused the login.
    Login(String),
    /// The server wants an app password, not the account's usual one (Gmail, iCloud…).
    AppPassword(String),
    /// The server refused a command.
    Server(String),
    /// The mail or the sync state could not be written.
    Disk(String),
    /// The sending server refused the message.
    Refused(String),
    /// Sent, but its copy could not be filed in Sent.
    NotFiled(String),
    /// The message could not be written (no recipient, an attachment gone): said as is.
    Message(String),
}

impl SyncError {
    /// The message that says it ("sync-error-network").
    fn message_id(&self) -> &'static str {
        match self {
            SyncError::BadAddress => "sync-error-address",
            SyncError::NotFound(_) => "sync-error-not-found",
            SyncError::NoServer => "sync-error-no-server",
            SyncError::NoPassword => "sync-error-no-password",
            SyncError::Keyring(_) => "sync-error-keyring",
            SyncError::Network(_) => "sync-error-network",
            SyncError::Tls(_) => "sync-error-tls",
            SyncError::Login(d) if d == google::SIGN_IN_AGAIN => "sync-error-google-again",
            SyncError::Login(d) if d == google::IN_TESTING => "sync-error-google-testing",
            SyncError::Login(d) if d == github::TOKEN_REFUSED => "sync-error-github-token",
            SyncError::Login(_) => "sync-error-login",
            SyncError::AppPassword(_) => "sync-error-app-password",
            SyncError::Server(_) => "sync-error-server",
            SyncError::Disk(_) => "sync-error-disk",
            SyncError::Refused(_) => "send-error-refused",
            SyncError::NotFiled(_) => "send-error-not-filed",
            SyncError::Message(_) => "send-error-message",
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
            | SyncError::Login(d)
            | SyncError::AppPassword(d)
            | SyncError::Server(d)
            | SyncError::Disk(d)
            | SyncError::Refused(d)
            | SyncError::NotFiled(d)
            | SyncError::Message(d) => d,
        }
    }

    /// Whether trying again later can help: a refused password does not get
    /// better by retrying, and retries can lock the account.
    pub fn is_lasting(&self) -> bool {
        matches!(
            self,
            SyncError::Login(_) | SyncError::AppPassword(_) | SyncError::Tls(_) | SyncError::NoServer | SyncError::NoPassword | SyncError::BadAddress
        )
    }

    /// Whether a password, given again, can help: none kept here yet, or the
    /// server refused the one kept (or wants an app password).
    pub fn wants_password(&self) -> bool {
        matches!(self, SyncError::NoPassword | SyncError::Login(_) | SyncError::AppPassword(_))
    }

    /// The sentence, in your language: "home: the server cannot be reached (timed out)."
    pub fn sentence(&self, tr: &Translator, account: &str) -> String {
        let mut args = i18n::args();
        args.set("account", account.to_string());
        args.set("detail", self.detail().to_string());
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
        assert!(is_gmail("Someone@GMail.com") && !is_gmail("someone@example.org"));
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's background service: finding mail servers, keeping passwords,
//! fetching mail, what you do to it on the server, sending, and the one
//! notification that cannot wait.
//!
//! Fetching changes nothing on the server: folders are opened with EXAMINE
//! and messages fetched with BODY.PEEK, so read flags, folders and mail stay
//! as your other apps left them (RFC 9051 §6.3.3 and §6.4.5). The server is
//! written to only when you act (`mailbox::act`, `send`). Connections are
//! encrypted from the first byte or upgraded by STARTTLS before the login;
//! there is no plain option.

// The test feature accepts any server certificate: never in a build made for use.
#[cfg(all(feature = "insecure-test-tls", not(debug_assertions)))]
compile_error!("insecure-test-tls accepts any certificate: for test builds only, never with --release");

#[cfg(target_os = "android")]
pub mod android;
pub mod antivirus;
pub mod bitwarden;
pub mod dav;
pub mod discover;
pub mod favicon;
pub mod disk;
pub mod fetch;
pub mod geocode;
pub mod github;
pub mod google;
pub mod google_tasks;
mod imap;
pub mod keys;
pub mod lease;
pub mod mailbox;
pub mod notify;
pub mod ocr;
pub mod scout;
pub mod secret;
pub mod send;
pub mod share;
pub mod shield_ai;
pub mod verify;
pub mod weather;

pub use discover::{Found, FoundBy, Smtp, discover, discover_smtp};
pub use fetch::{Control, Report, sync, test, watch};

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

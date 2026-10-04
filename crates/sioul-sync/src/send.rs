// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sending: SMTP submission (RFC 6409), over TLS from the first byte (465,
//! RFC 8314) or upgraded by STARTTLS (587), with the account's login and
//! password; then a copy into Sent with `APPEND`, except at Gmail, which files
//! its own.

use crate::SyncError;
use crate::fetch::block_on;
use crate::imap::{self, COMMAND, Server};
use crate::mailbox;
use mail_send::SmtpClientBuilder;
use mail_send::smtp::message::Message;
use sioul_core::compose::{self, Draft, Outgoing};
use sioul_core::pgp;
use sioul_core::config::{self, Account, Security};
use sioul_core::folders::Role;
use sioul_core::i18n::Translator;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SMTP: Duration = Duration::from_secs(60);

/// Sends a draft from its account: the sending server is found first when the
/// account has none, and kept in the configuration at `config_path`. A
/// `NotFiled` error means it was sent all the same.
pub fn send_draft(config_path: &Path, account: &Account, password: &str, draft: &Draft, tr: &Translator) -> Result<(), SyncError> {
    let account = with_smtp(config_path, account)?;
    let name = account.name.clone().unwrap_or_default();
    let address = account.address.clone().unwrap_or_default();
    let date = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    // Your key, when you have one for this address: it signs, it reads what is
    // encrypted to you, and Autocrypt hands it to whom you write.
    let keys = pgp::Keys::load();
    let own = keys.own_for(&address);
    let passphrase = own.and_then(|c| crate::secret::pgp_passphrase(&c.fingerprint().to_hex())).unwrap_or_default();
    let autocrypt = own.and_then(|c| pgp::autocrypt_header(c, &address));
    let protection = if draft.sign || draft.encrypt {
        let say = |id: &str, address: &str| {
            let mut args = sioul_core::i18n::args();
            args.set("address", address.to_string());
            SyncError::Message(tr.text(id, Some(&args)))
        };
        let sign = match (draft.sign, own) {
            (true, Some(cert)) => Some((cert, passphrase.as_str())),
            (true, None) => return Err(say("pgp-no-own-key", &address)),
            (false, _) => None,
        };
        let mut encrypt_to = Vec::new();
        if draft.encrypt {
            for recipient in draft.recipients() {
                encrypt_to.push(keys.for_address(&recipient).ok_or_else(|| say("pgp-missing-key", &recipient))?);
            }
            // Yours too, to read it again in Sent.
            encrypt_to.extend(own);
        }
        Some(compose::Protection { sign, encrypt_to })
    } else {
        None
    };
    let outgoing = compose::build_with(draft, (&name, &address), tr, date, protection.as_ref(), autocrypt.as_deref()).map_err(SyncError::Message)?;
    send(&account, password, &outgoing)
}

fn with_smtp(config_path: &Path, account: &Account) -> Result<Account, SyncError> {
    if account.smtp_host.is_some() {
        return Ok(account.clone());
    }
    let smtp = crate::discover_smtp(account.address.as_deref().unwrap_or(""), account.host.as_deref())?;
    config::set_smtp(config_path, &account.id, &smtp.host, smtp.port, smtp.security).map_err(SyncError::Disk)?;
    let mut account = account.clone();
    account.smtp_host = Some(smtp.host);
    account.smtp_port = Some(smtp.port);
    account.smtp_security = smtp.security;
    Ok(account)
}

/// Sends the message, then files a copy in Sent.
pub fn send(account: &Account, password: &str, outgoing: &Outgoing) -> Result<(), SyncError> {
    let host = account.smtp_host.clone().ok_or(SyncError::NoServer)?;
    let login = account.login().ok_or(SyncError::NoServer)?.to_string();
    block_on(async {
        let builder = SmtpClientBuilder::new(host.as_str(), account.smtp_port_or_default())
            .map_err(SyncError::Network)?
            .implicit_tls(account.smtp_security == Security::Tls)
            // Not this computer's name, which providers copy into the message's
            // Received header for everyone to read: an address literal, as RFC
            // 5321 §4.1.4 allows a client without a name to give.
            .helo_host("[127.0.0.1]")
            .credentials((login.as_str(), password))
            .timeout(SMTP);
        // Android: mail-send checks certificates with rustls-platform-verifier,
        // which there needs Java code Sioul does not ship; the same check as
        // for IMAP instead (Android's certificates, else Mozilla's).
        #[cfg(target_os = "android")]
        let builder = {
            let mut builder = builder;
            builder.tls_connector = tokio_rustls::TlsConnector::from(crate::imap::tls_config());
            builder
        };
        #[cfg(feature = "insecure-test-tls")]
        let builder = if std::env::var_os("SIOUL_TEST_INSECURE_TLS").is_some() { builder.allow_invalid_certs() } else { builder };
        let mut client = builder.connect().await.map_err(smtp_error)?;
        let message = Message::new(outgoing.from.as_str(), outgoing.recipients.iter().map(String::as_str), outgoing.raw.as_slice());
        client.send(message).await.map_err(smtp_error)?;
        let _ = client.quit().await;
        Ok(())
    })?;
    if account.host.as_deref().is_some_and(|h| h.eq_ignore_ascii_case("imap.gmail.com")) {
        return Ok(());
    }
    // Sent, but its copy may not be filed: that is said, the message is not sent twice.
    file_in_sent(account, password, &outgoing.raw).map_err(|e| SyncError::NotFiled(e.detail().to_string()))
}

/// A copy in Sent, read.
fn file_in_sent(account: &Account, password: &str, raw: &[u8]) -> Result<(), SyncError> {
    let server = Server::of(account)?;
    block_on(async {
        let mut session = imap::open(&server, password).await?;
        let sent = mailbox::role_folder(&mut session, account, Role::Sent, "Sent").await?;
        let result = imap::within(COMMAND, session.append(&sent, Some("(\\Seen)"), None, raw)).await?.map_err(imap::server);
        let _ = session.logout().await;
        result
    })
}

fn smtp_error(e: mail_send::Error) -> SyncError {
    match e {
        mail_send::Error::AuthenticationFailed(r) => SyncError::Login(r.message().to_string()),
        mail_send::Error::MissingCredentials | mail_send::Error::UnsupportedAuthMechanism => SyncError::Login(e.to_string()),
        mail_send::Error::Tls(t) => SyncError::Tls(t.to_string()),
        mail_send::Error::InvalidTLSName => SyncError::Tls(e.to_string()),
        mail_send::Error::Io(_) | mail_send::Error::Timeout => SyncError::Network(e.to_string()),
        mail_send::Error::UnexpectedReply(r) => SyncError::Refused(r.message().to_string()),
        other => SyncError::Refused(other.to_string()),
    }
}

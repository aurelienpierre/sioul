// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sending: SMTP submission (RFC 6409), over TLS from the first byte (465,
//! RFC 8314) or upgraded by STARTTLS (587), with the account's login and
//! password, or an access token (`AUTH XOAUTH2`) for an account signed in
//! with Google or Microsoft; then a copy into Sent with `APPEND`, except at
//! Gmail and at Microsoft, which file their own.
//!
//! Building a message and sending it are two steps. A message signed with a
//! key kept here is built when it leaves, at the end of its ten seconds
//! ([`send_draft`]). One signed with your security key is built at Send, in
//! the writing window, with its PIN and maybe a touch ([`build_draft`] with the
//! key), and sent as built once its ten seconds are gone ([`send_built`]).

use crate::SyncError;
use crate::fetch::block_on;
use crate::imap::{self, COMMAND, Server};
use crate::mailbox;
use crate::sasl::{self, OAuth};
use mail_send::SmtpClientBuilder;
use mail_send::smtp::message::Message;
use sioul_core::compose::{self, Draft, Outgoing};
use sioul_core::config::{self, Account, Security};
use sioul_core::folders::Role;
use sioul_core::i18n::Translator;
use sioul_core::pgp::{self, Place};
use sioul_core::securitykey::{self, CardAccess, CardError, CardSigner, Purpose};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SMTP: Duration = Duration::from_secs(60);

/// Sends a draft from its account: the sending server is found first when the
/// account has none, and kept in the configuration at `config_path`. A
/// `NotFiled` error means it was sent all the same. A draft your security key
/// signs is not sent from here: it is built at Send, in the writing window.
pub fn send_draft(config_path: &Path, account: &Account, password: &str, draft: &Draft, tr: &Translator) -> Result<(), SyncError> {
    let account = with_smtp(config_path, account)?;
    let outgoing = build_draft(&account, draft, tr, None).map_err(|e| e.into_sync(tr))?;
    send(&account, password, &outgoing)
}

/// Sends a message built at Send (signed with your security key), its ten
/// seconds gone: the sending server is found first when the account has none.
pub fn send_built(config_path: &Path, account: &Account, password: &str, outgoing: &Outgoing) -> Result<(), SyncError> {
    let account = with_smtp(config_path, account)?;
    send(&account, password, outgoing)
}

/// Why a draft could not be built.
#[derive(Debug)]
pub enum BuildError {
    /// Your security key of this identifier signs it, and it was not given:
    /// the draft is built at Send, in the writing window, with its PIN.
    CardNeeded(String),
    /// Your security key could not sign.
    Card(CardError),
    /// Anything else, as the sending says it.
    Other(SyncError),
}

impl BuildError {
    /// The problem as sending says it, in a sentence where needed.
    pub fn into_sync(self, tr: &Translator) -> SyncError {
        match self {
            BuildError::CardNeeded(_) => SyncError::Message(tr.text("seckey-sign-in-window", None)),
            BuildError::Card(e) => SyncError::Message(e.sentence(tr, Purpose::Sign)),
            BuildError::Other(e) => e,
        }
    }
}

/// The message a draft makes, protected as it asks: signed with your key
/// (here, its passphrase from the keyring; on your security key, through
/// `card`), encrypted to every recipient's key and to yours, your key handed
/// to whom you write by an Autocrypt header. Nothing goes on the network.
pub fn build_draft(account: &Account, draft: &Draft, tr: &Translator, card: Option<&CardAccess>) -> Result<Outgoing, BuildError> {
    build_draft_with(&pgp::Keys::load(), account, draft, tr, card)
}

fn build_draft_with(keys: &pgp::Keys, account: &Account, draft: &Draft, tr: &Translator, card: Option<&CardAccess>) -> Result<Outgoing, BuildError> {
    let name = account.name.clone().unwrap_or_default();
    let address = account.address.clone().unwrap_or_default();
    let date = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    // Your key, when you have one for this address: it signs; your own
    // certificate (your security key's on every device, as your computer read
    // it) reads what is encrypted to you, and Autocrypt hands it to whom you write.
    let own = keys.own_for(&address);
    let yours = keys.self_cert_for(&address);
    let autocrypt = yours.and_then(|cert| pgp::autocrypt_header(cert, &address));
    let say = |id: &str, pairs: &[(&str, String)]| {
        let mut args = sioul_core::i18n::args();
        for (name, value) in pairs {
            args.set(*name, value.clone());
        }
        BuildError::Other(SyncError::Message(tr.text(id, Some(&args))))
    };
    // The signer: your key here, opened with its passphrase; or your security key.
    let mut here = None;
    let mut on_card = None;
    if draft.sign {
        match &own {
            Some(pgp::Own { cert, place: Place::Here }) => {
                let passphrase = crate::secret::pgp_passphrase(&cert.fingerprint().to_hex()).unwrap_or_default();
                here = Some(pgp::here_signer(cert, &passphrase).map_err(|e| BuildError::Other(SyncError::Message(e)))?);
            }
            Some(pgp::Own { cert, place: Place::Card { ident } }) => {
                let access = card.ok_or_else(|| BuildError::CardNeeded(ident.clone()))?;
                let fingerprint = keys.card_for(&address).map(|(known, _)| known.sign.clone()).unwrap_or_default();
                let public = pgp::key_of(cert, &fingerprint).ok_or(BuildError::Card(CardError::OtherKeys))?;
                on_card = Some(CardSigner::new(access, ident, public));
            }
            // Your security key's certificate expired: said, with how to renew it.
            None => {
                return Err(match keys.card_for(&address) {
                    Some((known, cert)) => {
                        let now = date;
                        let expires = securitykey::expiry_of(cert, &[&known.sign, &known.decrypt]);
                        let line = securitykey::expiry_line(tr, expires, &cert.fingerprint().to_hex(), now).map(|(line, _)| line);
                        BuildError::Other(SyncError::Message(line.unwrap_or_else(|| tr.text("seckey-cannot-sign", None))))
                    }
                    None => say("pgp-no-own-key", &[("address", address.clone())]),
                });
            }
        }
    }
    let signer: Option<&mut pgp::Signing<'_>> = match (&mut here, &mut on_card) {
        (Some(pair), _) => Some(pair),
        (_, Some(signer)) => Some(signer),
        _ => None,
    };
    let mut protection = if draft.sign || draft.encrypt {
        let mut encrypt_to = Vec::new();
        if draft.encrypt {
            for recipient in draft.recipients() {
                encrypt_to.push(keys.for_address(&recipient).ok_or_else(|| say("pgp-missing-key", &[("address", recipient.clone())]))?);
            }
            // Yours too, to read it again in Sent (an expired certificate of yours: the window said so first).
            if let Some(cert) = yours
                && !encrypt_to.iter().any(|c| c.fingerprint() == cert.fingerprint())
            {
                encrypt_to.push(cert);
            }
        }
        Some(compose::Protection { sign: signer, encrypt_to })
    } else {
        None
    };
    compose::build_with(draft, (&name, &address), tr, date, protection.as_mut(), autocrypt.as_deref()).map_err(|e| match card.and_then(|c| c.failure()) {
        Some(failure) => BuildError::Card(failure),
        None => BuildError::Other(SyncError::Message(e)),
    })
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

/// The sending server's client, logged in with `credentials` when given
/// (an account signed in with OAuth logs in after, `sasl::smtp_xoauth2`).
fn smtp_builder<'a>(account: &Account, host: &'a str, credentials: Option<(&'a str, &'a str)>) -> Result<SmtpClientBuilder<&'a str>, SyncError> {
    let builder = SmtpClientBuilder::new(host, account.smtp_port_or_default())
        .map_err(SyncError::Network)?
        .implicit_tls(account.smtp_security == Security::Tls)
        // Not this computer's name, which providers copy into the message's
        // Received header for everyone to read: an address literal, as RFC
        // 5321 §4.1.4 allows a client without a name to give.
        .helo_host("[127.0.0.1]")
        .timeout(SMTP);
    let builder = match credentials {
        Some(credentials) => builder.credentials(credentials),
        None => builder,
    };
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
    Ok(builder)
}

/// Sends the message, then files a copy in Sent. An account signed in with
/// OAuth logs in with an access token (`password` unused): the one kept,
/// then, refused, a fresh one, once; a refusal no token changes (Microsoft's
/// Authenticated SMTP switched off) is said at once.
pub fn send(account: &Account, password: &str, outgoing: &Outgoing) -> Result<(), SyncError> {
    let host = account.smtp_host.clone().ok_or(SyncError::NoServer)?;
    let login = account.login().ok_or(SyncError::NoServer)?.to_string();
    let oauth = OAuth::of(account);
    block_on(async {
        let mut client = match oauth {
            None => smtp_builder(account, &host, Some((login.as_str(), password)))?.connect().await.map_err(smtp_error)?,
            Some(oauth) => {
                // Microsoft's token to Microsoft's sending server alone, before any is asked.
                oauth.may_reach(&host, true)?;
                let connect = || async { smtp_builder(account, &host, None)?.connect().await.map_err(smtp_error) };
                smtp_sign_in_with_tokens(&login, connect, |fresh| oauth.token_async(&login, fresh), oauth).await?
            }
        };
        let message = Message::new(outgoing.from.as_str(), outgoing.recipients.iter().map(String::as_str), outgoing.raw.as_slice());
        client.send(message).await.map_err(smtp_error)?;
        let _ = client.quit().await;
        Ok(())
    })?;
    if files_its_own_sent(account) {
        return Ok(());
    }
    // Sent, but its copy may not be filed: that is said, the message is not sent twice.
    file_in_sent(account, password, &outgoing.raw).map_err(|e| SyncError::NotFiled(e.detail().to_string()))
}

/// AUTH XOAUTH2 on connections `connect` makes, with the tokens `token`
/// gives: the one kept first; refused, a fresh one, once; refused again, the
/// access must be given again. A refusal no token changes (Microsoft's
/// Authenticated SMTP switched off) is said at once (`OAuth::smtp_refused`).
async fn smtp_sign_in_with_tokens<T, C, CF, K, KF>(login: &str, connect: C, token: K, oauth: OAuth) -> Result<mail_send::SmtpClient<T>, SyncError>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    C: Fn() -> CF,
    CF: Future<Output = Result<mail_send::SmtpClient<T>, SyncError>>,
    K: Fn(bool) -> KF,
    KF: Future<Output = Result<String, SyncError>>,
{
    let mut refusal = String::new();
    for fresh in [false, true] {
        let token = token(fresh).await?;
        let mut client = connect().await?;
        match sasl::smtp_xoauth2(&mut client, login, &token).await.map_err(smtp_error)? {
            Ok(()) => return Ok(client),
            Err(said) => {
                let _ = client.quit().await;
                if let Some(lasting) = oauth.smtp_refused(&said) {
                    return Err(lasting);
                }
                refusal = said;
            }
        }
    }
    Err(oauth.refused(&refusal))
}

/// Gmail and Microsoft (Outlook.com and Microsoft 365 alike) file a copy of
/// each message sent through their SMTP server in Sent themselves: an APPEND
/// would make it twice.
fn files_its_own_sent(account: &Account) -> bool {
    account.host.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(crate::discover::GOOGLE_IMAP) || crate::microsoft::is_imap_host(h))
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

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::pgp::{Keys, SessionKeys, Unlock};
    use sioul_core::securitykey::{KnownCard, SoftCard, SoftReaders};

    /// Part 8, 9: an SMTP server standing in for Microsoft's, without TLS:
    /// `535 5.7.3` asks a fresh token, once; `535 5.7.139` (Authenticated
    /// SMTP switched off) is said at once, no second token asked.
    #[test]
    fn microsoft_smtp_refusals() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            for (first, second, connections) in [("535 5.7.3 Authentication unsuccessful", Some("235 2.7.0 Authentication successful"), 2), ("535 5.7.139 Authentication unsuccessful, SmtpClientAuthentication is disabled for the Tenant. Visit https://aka.ms/smtp_auth_disabled for more information.", None, 1)] {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let port = listener.local_addr().unwrap().port();
                let server = tokio::spawn(async move {
                    let mut heard = Vec::new();
                    for answer in [Some(first), second].into_iter().flatten().take(connections) {
                        let (stream, _) = listener.accept().await.unwrap();
                        let (read, mut write) = stream.into_split();
                        let mut lines = BufReader::new(read).lines();
                        write.write_all(b"220 stand-in Microsoft ESMTP\r\n").await.unwrap();
                        let _ehlo = lines.next_line().await.unwrap().unwrap();
                        write.write_all(b"250-stand-in\r\n250-AUTH LOGIN XOAUTH2\r\n250 8BITMIME\r\n").await.unwrap();
                        heard.push(lines.next_line().await.unwrap().unwrap());
                        write.write_all(format!("{answer}\r\n").as_bytes()).await.unwrap();
                        // QUIT after a refusal, or nothing more.
                        if let Ok(Some(_)) = lines.next_line().await {
                            let _ = write.write_all(b"221 bye\r\n").await;
                        }
                    }
                    heard
                });
                let connect = || async move { mail_send::SmtpClientBuilder::new("127.0.0.1", port).unwrap().helo_host("[127.0.0.1]").connect_plain().await.map_err(smtp_error) };
                let asked = std::sync::Mutex::new(Vec::new());
                let token = |fresh: bool| {
                    asked.lock().unwrap().push(fresh);
                    async move { Ok::<_, SyncError>(if fresh { "fresh".to_string() } else { "kept".to_string() }) }
                };
                // The client dropped once signed in: the stand-in then hears its connection end.
                let outcome = smtp_sign_in_with_tokens("you@example.org", connect, token, OAuth::Microsoft { personal: false }).await.map(drop);
                let heard = server.await.unwrap();
                match second {
                    Some(_) => {
                        assert!(outcome.is_ok(), "{:?}", outcome.err());
                        assert_eq!(*asked.lock().unwrap(), vec![false, true], "the kept token, then a fresh one, once");
                        assert_eq!(heard, vec![format!("AUTH XOAUTH2 {}", sasl::xoauth2_base64("you@example.org", "kept")), format!("AUTH XOAUTH2 {}", sasl::xoauth2_base64("you@example.org", "fresh"))]);
                    }
                    None => {
                        assert_eq!(outcome.err(), Some(SyncError::Said(crate::microsoft::SMTP_OFF, first.to_string())));
                        assert_eq!(*asked.lock().unwrap(), vec![false], "no second token: none would change it");
                    }
                }
            }
        });
    }

    /// Part 8, 10: Microsoft's servers file their own copy in Sent, as Gmail's do: no APPEND.
    #[test]
    fn microsoft_files_its_own_sent_copy() {
        let mut account = Account::imap("work", "you@contoso.example", "outlook.office365.com", 993, Security::Tls, None);
        account.auth = Some("microsoft".into());
        assert!(files_its_own_sent(&account));
        assert!(files_its_own_sent(&Account::imap("g", "you@gmail.com", "imap.gmail.com", 993, Security::Tls, None)));
        assert!(!files_its_own_sent(&Account::imap("o", "you@example.org", "mail.example.org", 993, Security::Tls, None)));
    }

    #[test]
    fn a_draft_your_security_key_signs_is_built_with_the_key() {
        let (card, public) = SoftCard::generate("0006:12345678", &["Card Holder <me@example.org>"], "123456").unwrap();
        let known = KnownCard {
            ident: "0006:12345678".into(),
            sign: card.info.sign.as_ref().unwrap().fingerprint.clone(),
            decrypt: card.info.decrypt.as_ref().unwrap().fingerprint.clone(),
            cert: pgp::certificates_in(&public)[0].fingerprint().to_hex(),
            addresses: vec!["me@example.org".into()],
            ..KnownCard::default()
        };
        let keys = Keys { own: pgp::certificates_in(&public), cards: vec![known], ..Keys::default() };
        let account: Account = toml::from_str("id = \"me\"\naddress = \"me@example.org\"\nname = \"Card Holder\"\n").unwrap();
        let mut draft = Draft::new("me");
        draft.to = vec!["Jane <jane@example.org>".into()];
        draft.subject = "Signed at Send".into();
        draft.body = "Hello.".into();
        draft.sign = true;
        let tr = Translator::new("en");
        // Not from the ten seconds' end: at Send, with the key.
        assert!(matches!(build_draft_with(&keys, &account, &draft, &tr, None), Err(BuildError::CardNeeded(ident)) if ident == "0006:12345678"));
        let readers = SoftReaders::with(vec![card]);
        let wrong = sioul_core::securitykey::Password::from("000000");
        let access = CardAccess::new(&readers, &wrong, &|| {});
        assert!(matches!(build_draft_with(&keys, &account, &draft, &tr, Some(&access)), Err(BuildError::Card(CardError::WrongPin { left: 2 }))));
        let pin = sioul_core::securitykey::Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        let outgoing = build_draft_with(&keys, &account, &draft, &tr, Some(&access)).unwrap();
        let text = String::from_utf8_lossy(&outgoing.raw).to_string();
        assert!(text.contains("multipart/signed") && text.contains("Autocrypt: addr=me@example.org"), "{text}");
        // The signature checks out with the key's certificate, as a reader's would.
        let reader = Keys { others: pgp::certificates_in(&public), ..Keys::default() };
        let sessions = SessionKeys::new();
        let (_, view) = pgp::open(&outgoing.raw, &reader, &Unlock::new(&|_| None, &sessions)).unwrap();
        assert!(view.signatures.len() == 1 && view.signatures[0].good, "{view:?}");
        assert_eq!(readers.card("0006:12345678", |c| c.operations), Some(1));
        // The CLI and the end of the ten seconds say where to send it from.
        assert!(BuildError::CardNeeded("x".into()).into_sync(&tr).detail().contains("window"));
    }

    /// Batch A (18 October 2026): a phone that knows your security key only
    /// by what your computer shared (its certificate, public parts) encrypts
    /// to the recipients and to you, every recipient once, none dropped nor
    /// added, and hands your key by Autocrypt; your security key opens the
    /// sent copy. A recipient whose key is unknown stops it, said.
    #[test]
    fn the_phone_encrypts_to_its_recipients_and_to_you() {
        let (card, public) = SoftCard::generate("0006:12345678", &["Card Holder <me@example.org>"], "123456").unwrap();
        let mine = pgp::certificates_in(&public);
        let known = KnownCard {
            ident: "0006:12345678".into(),
            sign: card.info.sign.as_ref().unwrap().fingerprint.clone(),
            decrypt: card.info.decrypt.as_ref().unwrap().fingerprint.clone(),
            cert: mine[0].fingerprint().to_hex(),
            addresses: vec!["me@example.org".into()],
            ..KnownCard::default()
        };
        // Jane's key, on a stand-in card of hers.
        let (jane_card, jane_public) = SoftCard::generate("0006:00000002", &["Jane <jane@example.org>"], "654321").unwrap();
        let jane_cert = pgp::certificates_in(&jane_public);
        let jane_known = KnownCard {
            ident: "0006:00000002".into(),
            sign: jane_card.info.sign.as_ref().unwrap().fingerprint.clone(),
            decrypt: jane_card.info.decrypt.as_ref().unwrap().fingerprint.clone(),
            cert: jane_cert[0].fingerprint().to_hex(),
            addresses: vec!["jane@example.org".into()],
            ..KnownCard::default()
        };
        let keys = Keys { own: mine.clone(), others: jane_cert.clone(), cards: vec![known] };
        let account: Account = toml::from_str("id = \"me\"\naddress = \"me@example.org\"\nname = \"Card Holder\"\n").unwrap();
        let mut draft = Draft::new("me");
        draft.to = vec!["Jane <jane@example.org>".into()];
        draft.subject = "Encrypted from the phone".into();
        draft.body = "Hello Jane.".into();
        draft.encrypt = true;
        let tr = Translator::new("en");
        let outgoing = build_draft_with(&keys, &account, &draft, &tr, None).unwrap();
        let text = String::from_utf8_lossy(&outgoing.raw).to_string();
        assert!(text.contains("multipart/encrypted") && text.contains("Autocrypt: addr=me@example.org"), "{text}");
        // Encrypted to Jane's key and to yours, no other (`sioul_core::pgp`'s own
        // tests open such a copy with the key itself).
        let recipients = pgp::recipients_of(&outgoing.raw);
        let decrypts = [jane_known.decrypt.clone(), keys.cards[0].decrypt.clone()];
        assert_eq!(recipients.len(), 2, "{recipients:?}");
        for recipient in &recipients {
            assert!(decrypts.iter().any(|d| !recipient.is_empty() && d.to_ascii_uppercase().ends_with(&recipient.to_ascii_uppercase())), "{recipient} among {decrypts:?}");
        }
        assert!(decrypts.iter().all(|d| recipients.iter().any(|r| d.to_ascii_uppercase().ends_with(&r.to_ascii_uppercase()))), "Jane and you");
        let _ = (jane_card, card);
        // The phone opens it: nothing of the key there, said (`PgpView::security_key`).
        let sessions = SessionKeys::new();
        let (opened, view) = pgp::open(&outgoing.raw, &keys, &Unlock::new(&|_| None, &sessions)).unwrap();
        assert!(opened.is_none() && view.security_key.as_deref() == Some("0006:12345678"), "{view:?}");
        // A recipient without a key: stopped and said, never sent to fewer.
        draft.to.push("Bob <bob@example.org>".into());
        assert!(build_draft_with(&keys, &account, &draft, &tr, None).is_err());
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An encrypted, logged-in IMAP session.
//!
//! TLS is rustls with the ring provider, checked against the system's
//! certificates (Mozilla's list when a system has none). STARTTLS is done here,
//! by hand, before the IMAP client starts: the login never travels in clear.
//! An account signed in with Google gives an access token instead of a
//! password (SASL XOAUTH2, `sasl.rs`).

use crate::SyncError;
use crate::sasl::{ImapXoauth2, OAuth};
use async_imap::{Client, Session};
use rustls::pki_types::ServerName;
use sioul_core::config::{Account, Security};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

pub(crate) type Imap = Session<TlsStream<TcpStream>>;

pub(crate) const CONNECT: Duration = Duration::from_secs(20);
pub(crate) const COMMAND: Duration = Duration::from_secs(60);

/// Where to connect, and as whom.
#[derive(Debug, Clone)]
pub(crate) struct Server {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub login: String,
    /// Signed in with an access token rather than a password.
    pub oauth: Option<OAuth>,
}

impl Server {
    pub fn of(account: &Account) -> Result<Server, SyncError> {
        Ok(Server {
            host: account.host.clone().ok_or(SyncError::NoServer)?,
            port: account.port_or_default(),
            security: account.security,
            login: account.login().ok_or(SyncError::NoServer)?.to_string(),
            oauth: OAuth::of(account),
        })
    }
}

/// Connects, encrypts and logs in: with the password, or, for an account
/// signed in with OAuth, with an access token (the password is then unused).
pub(crate) async fn open(server: &Server, password: &str) -> Result<Imap, SyncError> {
    if let Some(oauth) = server.oauth {
        return sign_in_with_tokens(&server.login, || connect(server), |fresh| oauth.token_async(&server.login, fresh), oauth).await;
    }
    let client = connect(server).await?;
    within(COMMAND, client.login(&server.login, password)).await?.map_err(|(e, _)| login_error(e, &server.host))
}

/// AUTHENTICATE XOAUTH2 on connections `connect` makes, with the tokens
/// `token` gives: the one kept first; refused, a fresh one, once (the one kept
/// may have ended early: a password changed, the access withdrawn); refused
/// again, the access must be given again.
pub(crate) async fn sign_in_with_tokens<T, C, CF, K, KF>(login: &str, connect: C, token: K, oauth: OAuth) -> Result<Session<T>, SyncError>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send,
    C: Fn() -> CF,
    CF: Future<Output = Result<Client<T>, SyncError>>,
    K: Fn(bool) -> KF,
    KF: Future<Output = Result<String, SyncError>>,
{
    let mut refusal = String::new();
    for fresh in [false, true] {
        let token = token(fresh).await?;
        let client = connect().await?;
        let mut sasl = ImapXoauth2::new(login, &token);
        match within(COMMAND, client.authenticate("XOAUTH2", &mut sasl)).await? {
            Ok(session) => return Ok(session),
            Err((async_imap::error::Error::No(m) | async_imap::error::Error::Bad(m), _)) => refusal = sasl.refusal.clone().unwrap_or_else(|| m.trim().to_string()),
            Err((other, _)) => return Err(SyncError::Network(other.to_string())),
        }
    }
    Err(oauth.refused(&refusal))
}

/// Connects and encrypts, up to the server's greeting.
async fn connect(server: &Server) -> Result<Client<TlsStream<TcpStream>>, SyncError> {
    let tcp = within(CONNECT, TcpStream::connect((server.host.as_str(), server.port))).await?.map_err(network)?;
    let tcp = match server.security {
        Security::Tls => tcp,
        Security::Starttls => starttls(tcp).await?,
    };
    let name = ServerName::try_from(server.host.clone()).map_err(|e| SyncError::Network(e.to_string()))?;
    let tls = within(CONNECT, TlsConnector::from(tls_config()).connect(name, tcp)).await?.map_err(tls_error)?;
    let mut client = Client::new(tls);
    if server.security == Security::Tls {
        // The server speaks first (RFC 9051 §7.1); after STARTTLS it already has.
        let greeting = within(CONNECT, client.read_response()).await?.map_err(network)?;
        greeting.ok_or_else(|| SyncError::Network("closed before greeting".into()))?;
    }
    Ok(client)
}

/// Upgrades a plain connection before anything else is said (RFC 9051 §6.2.1):
/// the greeting, STARTTLS, its OK. Read a byte at a time, so nothing meant for
/// TLS is swallowed.
async fn starttls(mut tcp: TcpStream) -> Result<TcpStream, SyncError> {
    let greeting = within(CONNECT, read_line(&mut tcp)).await??;
    if !greeting.starts_with("* OK") {
        return Err(SyncError::Server(greeting));
    }
    tcp.write_all(b"s1 STARTTLS\r\n").await.map_err(network)?;
    loop {
        let line = within(CONNECT, read_line(&mut tcp)).await??;
        if line.starts_with("s1 OK") {
            return Ok(tcp);
        }
        if line.starts_with("s1 ") {
            return Err(SyncError::Server(line));
        }
    }
}

async fn read_line(tcp: &mut TcpStream) -> Result<String, SyncError> {
    let mut line = Vec::new();
    loop {
        let byte = tcp.read_u8().await.map_err(network)?;
        if byte == b'\n' {
            return Ok(String::from_utf8_lossy(&line).trim_end().to_string());
        }
        line.push(byte);
        if line.len() > 8192 {
            return Err(SyncError::Server("greeting too long".into()));
        }
    }
}

pub(crate) fn tls_config() -> Arc<rustls::ClientConfig> {
    #[cfg(feature = "insecure-test-tls")]
    if std::env::var_os("SIOUL_TEST_INSECURE_TLS").is_some() {
        return insecure::config();
    }
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            let (added, _) = roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
            if added == 0 {
                // A system without a certificate store, such as a bare container.
                roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            }
            let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .expect("ring supports TLS 1.2 and 1.3")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

/// Waits at most `limit`; a server that does not answer is a network problem.
pub(crate) async fn within<F: Future>(limit: Duration, future: F) -> Result<F::Output, SyncError> {
    timeout(limit, future).await.map_err(|_| SyncError::Network(format!("no answer within {} s", limit.as_secs())))
}

fn network(e: std::io::Error) -> SyncError {
    SyncError::Network(e.to_string())
}

/// A certificate problem is told apart from a network one: it may be an attack.
fn tls_error(e: std::io::Error) -> SyncError {
    match e.get_ref().and_then(|inner| inner.downcast_ref::<rustls::Error>()) {
        Some(tls) => SyncError::Tls(tls.to_string()),
        None => network(e),
    }
}

/// A refused login; "application-specific password required" (Gmail) gets its own
/// sentence, since the usual password will never work there. So does any
/// password Google's server refuses: it takes app passwords only.
fn login_error(e: async_imap::error::Error, host: &str) -> SyncError {
    match e {
        async_imap::error::Error::No(m) | async_imap::error::Error::Bad(m) => {
            let detail = m.trim().to_string();
            let lower = detail.to_ascii_lowercase();
            if lower.contains("application-specific password") || lower.contains("app password") || host.eq_ignore_ascii_case(crate::discover::GOOGLE_IMAP) {
                SyncError::AppPassword(detail)
            } else {
                SyncError::Login(detail)
            }
        }
        other => SyncError::Network(other.to_string()),
    }
}

/// Any other IMAP error.
pub(crate) fn server(e: async_imap::error::Error) -> SyncError {
    match e {
        async_imap::error::Error::No(m) | async_imap::error::Error::Bad(m) => SyncError::Server(m.trim().to_string()),
        other => SyncError::Network(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_app_passwords_apart() {
        let gmail = async_imap::error::Error::No("[ALERT] Application-specific password required: https://support.google.com/accounts/answer/185833 (Failure)".into());
        assert!(matches!(login_error(gmail, "mail.example.org"), SyncError::AppPassword(_)));
        let wrong = async_imap::error::Error::No("[AUTHENTICATIONFAILED] Authentication failed.".into());
        assert!(matches!(login_error(wrong, "mail.example.org"), SyncError::Login(_)));
        // Google refusing a password: an app password, made at Google, is the way.
        let google = async_imap::error::Error::No("[AUTHENTICATIONFAILED] Invalid credentials (Failure)".into());
        assert!(matches!(login_error(google, "imap.gmail.com"), SyncError::AppPassword(_)));
    }

    /// Reaches real servers, so it runs only when asked: `cargo test -- --ignored`.
    /// It stops at the greeting: no login is ever attempted.
    #[test]
    #[ignore]
    fn reaches_real_servers_up_to_the_greeting() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        for (host, port, security) in [("imap.gmail.com", 993, Security::Tls), ("mail.ecloud.global", 993, Security::Tls), ("outlook.office365.com", 143, Security::Starttls)] {
            let server = Server { host: host.into(), port, security, login: String::new(), oauth: None };
            let client = runtime.block_on(connect(&server));
            assert!(client.is_ok(), "{host}:{port}: {:?}", client.err());
        }
        let wrong_name = Server { host: "wrong.host.badssl.com".into(), port: 443, security: Security::Tls, login: String::new(), oauth: None };
        assert!(matches!(runtime.block_on(connect(&wrong_name)), Err(SyncError::Tls(_))));
    }
}

/// For tests against a local server with a self-signed certificate only: built
/// with the `insecure-test-tls` feature, never in Sioul itself.
#[cfg(feature = "insecure-test-tls")]
mod insecure {
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, SignatureScheme};
    use std::sync::Arc;

    #[derive(Debug)]
    struct AnyCertificate;

    impl ServerCertVerifier for AnyCertificate {
        fn verify_server_cert(&self, _: &CertificateDer, _: &[CertificateDer], _: &ServerName, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(&self, _: &[u8], _: &CertificateDer, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
        }
    }

    pub(super) fn config() -> Arc<rustls::ClientConfig> {
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("ring supports TLS 1.2 and 1.3")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyCertificate))
            .with_no_client_auth();
        Arc::new(config)
    }
}

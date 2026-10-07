// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Signing in to IMAP and SMTP with an OAuth 2.0 access token instead of a
//! password: SASL XOAUTH2, as Google defines it
//! (<https://developers.google.com/workspace/gmail/imap/xoauth2-protocol>), and
//! as Microsoft takes it too. The client's first answer is
//! `base64("user=" user "\x01auth=Bearer " token "\x01\x01")`; a refused
//! token brings a last challenge, the reason as base64 JSON
//! (`{"status":"401","schemes":"bearer mac","scope":"https://mail.google.com/"}`),
//! answered with an empty line, then the refusal (`NO`, `535`).
//!
//! Who gives the tokens is [`OAuth`]: Google only, for now. Nothing here is
//! Google's but that choice, so that another provider's sign-in is one more
//! arm of it.

use crate::SyncError;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sioul_core::config::{Account, AccountKind};

/// The initial client response, before base64 (the IMAP and SMTP clients encode it).
pub fn xoauth2(user: &str, token: &str) -> String {
    format!("user={user}\x01auth=Bearer {token}\x01\x01")
}

/// The same, as sent on the wire.
pub fn xoauth2_base64(user: &str, token: &str) -> String {
    STANDARD.encode(xoauth2(user, token))
}

/// What a refusal's challenge says, read for a person: Google's JSON status
/// and scope, else the text as it came.
pub fn reason(challenge: &[u8]) -> String {
    let text = String::from_utf8_lossy(challenge).trim().to_string();
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(json) => {
            let status = json["status"].as_str().unwrap_or("");
            let scope = json["scope"].as_str().unwrap_or("");
            [status, scope].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ")
        }
        Err(_) => text,
    }
}

/// XOAUTH2 for async-imap's `AUTHENTICATE`: the token at the first
/// continuation, an empty line at the next one (the refusal's reason, kept).
pub(crate) struct ImapXoauth2 {
    response: String,
    sent: bool,
    /// The server's reason, when it refused.
    pub refusal: Option<String>,
}

impl ImapXoauth2 {
    pub fn new(user: &str, token: &str) -> ImapXoauth2 {
        ImapXoauth2 { response: xoauth2(user, token), sent: false, refusal: None }
    }
}

impl async_imap::Authenticator for &mut ImapXoauth2 {
    type Response = Vec<u8>;

    fn process(&mut self, challenge: &[u8]) -> Vec<u8> {
        if self.sent {
            self.refusal = Some(reason(challenge));
            return Vec::new();
        }
        self.sent = true;
        self.response.clone().into_bytes()
    }
}

/// `AUTH XOAUTH2` on an SMTP connection already encrypted and greeted: Ok
/// when accepted, the refusal's words otherwise (after the empty line its
/// challenge asks). mail-send's own XOAUTH2 answers that challenge with the
/// token again, not with the empty line Google's page asks: done here instead.
pub(crate) async fn smtp_xoauth2<T>(client: &mut mail_send::SmtpClient<T>, user: &str, token: &str) -> Result<Result<(), String>, mail_send::Error>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let reply = client.cmd(format!("AUTH XOAUTH2 {}\r\n", xoauth2_base64(user, token))).await?;
    match reply.code() {
        235 => Ok(Ok(())),
        334 => {
            let why = STANDARD.decode(reply.message().trim()).map(|b| reason(&b)).unwrap_or_default();
            let end = client.cmd(b"\r\n").await?;
            Ok(Err(format!("{} {} ({why})", end.code(), end.message()).trim().to_string()))
        }
        code => Ok(Err(format!("{code} {}", reply.message()))),
    }
}

/// Who gives an account its access tokens, when it signs in with OAuth
/// rather than a password.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuth {
    /// Google's sign-in, the mail grant kept in the keyring (`google::Purpose::Mail`).
    Google,
}

impl OAuth {
    /// The provider of a mail account that signs in with one (`auth = "google"`);
    /// None: a password. Calendars signed in with Google are not mail: their
    /// token is asked by the DAV client.
    pub fn of(account: &Account) -> Option<OAuth> {
        match (account.auth.as_deref(), account.kind) {
            (Some("google"), AccountKind::Imap) => Some(OAuth::Google),
            _ => None,
        }
    }

    /// An access token for `login`'s mail: kept in memory while it lasts,
    /// else asked with the refresh token; `fresh` asks a new one (after a refusal).
    pub fn token(self, login: &str, fresh: bool) -> Result<String, SyncError> {
        match self {
            OAuth::Google => crate::google::access_token_for(crate::google::Purpose::Mail, login, fresh),
        }
    }

    /// The same, from async code: the token is asked over blocking HTTP.
    pub(crate) async fn token_async(self, login: &str, fresh: bool) -> Result<String, SyncError> {
        let login = login.to_string();
        tokio::task::spawn_blocking(move || self.token(&login, fresh)).await.map_err(|e| SyncError::Network(e.to_string()))?
    }

    /// What a token refused even fresh says: the access must be given again
    /// (it was withdrawn, or the mail was not ticked on the provider's page).
    pub(crate) fn refused(self, detail: &str) -> SyncError {
        let _ = detail;
        match self {
            OAuth::Google => SyncError::Login(crate::google::SIGN_IN_AGAIN.into()),
        }
    }

    /// What stands for the account's password, so that a watcher parked
    /// after a refusal sees a new sign-in as a new credential (`fetch::Parked`):
    /// the grant's mark. None kept: the sign-in must be done again.
    pub(crate) fn mark(self, login: &str) -> Result<String, SyncError> {
        match self {
            OAuth::Google => crate::google::grant_for(crate::google::Purpose::Mail, login).map(|g| format!("google:{}", g.signed_in)).ok_or_else(|| SyncError::Login(crate::google::SIGN_IN_AGAIN.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    /// Google's own example, character for character.
    #[test]
    fn xoauth2_as_google_writes_it() {
        let raw = xoauth2("someuser@example.com", "ya29.vF9dft4qmTc2Nvb3RlckBhdHRhdmlzdGEuY29tCg");
        assert_eq!(raw, "user=someuser@example.com\u{1}auth=Bearer ya29.vF9dft4qmTc2Nvb3RlckBhdHRhdmlzdGEuY29tCg\u{1}\u{1}");
        assert_eq!(
            xoauth2_base64("someuser@example.com", "ya29.vF9dft4qmTc2Nvb3RlckBhdHRhdmlzdGEuY29tCg"),
            "dXNlcj1zb21ldXNlckBleGFtcGxlLmNvbQFhdXRoPUJlYXJlciB5YTI5LnZGOWRmdDRxbVRjMk52YjNSbGNrQmhkSFJoZG1semRHRXVZMjl0Q2cBAQ=="
        );
        // Its refusal, decoded: the status and the scope it wanted.
        let challenge = STANDARD.decode("eyJzdGF0dXMiOiI0MDEiLCJzY2hlbWVzIjoiYmVhcmVyIG1hYyIsInNjb3BlIjoiaHR0cHM6Ly9tYWlsLmdvb2dsZS5jb20vIn0K").unwrap();
        assert_eq!(reason(&challenge), "401 https://mail.google.com/");
        assert_eq!(reason(b"no JSON here"), "no JSON here");
    }

    #[test]
    fn only_mail_accounts_signed_in_with_google_use_tokens() {
        let mut mail = Account::imap("g", "you@gmail.com", "imap.gmail.com", 993, sioul_core::config::Security::Tls, None);
        assert_eq!(OAuth::of(&mail), None, "a password, by default");
        mail.auth = Some("google".into());
        assert_eq!(OAuth::of(&mail), Some(OAuth::Google));
        let mut calendars = Account::dav("c", "you@gmail.com", "apidata.googleusercontent.com", None, None);
        calendars.auth = Some("google".into());
        assert_eq!(OAuth::of(&calendars), None, "calendars ask their token themselves");
    }

    /// An IMAP server standing in for Gmail's, without TLS: it takes one
    /// token and refuses any other the way Google does (a JSON challenge,
    /// the empty line awaited, then NO). What the client sent is returned.
    async fn imap_stand_in(listener: TcpListener, accepted: &'static str, connections: usize) -> Vec<String> {
        let mut heard = Vec::new();
        for _ in 0..connections {
            let (stream, _) = listener.accept().await.unwrap();
            let (read, mut write) = stream.into_split();
            let mut lines = BufReader::new(read).lines();
            write.write_all(b"* OK [CAPABILITY IMAP4rev1 AUTH=XOAUTH2 AUTH=PLAIN] Stand-in ready\r\n").await.unwrap();
            let command = lines.next_line().await.unwrap().unwrap();
            heard.push(command.clone());
            let tag = command.split(' ').next().unwrap().to_string();
            assert!(command.ends_with("AUTHENTICATE XOAUTH2"), "{command}");
            write.write_all(b"+ \r\n").await.unwrap();
            let response = lines.next_line().await.unwrap().unwrap();
            heard.push(response.clone());
            if response == xoauth2_base64("you@example.org", accepted) {
                write.write_all(format!("{tag} OK Success\r\n").as_bytes()).await.unwrap();
                let logout = lines.next_line().await.unwrap().unwrap();
                let logout_tag = logout.split(' ').next().unwrap().to_string();
                write.write_all(format!("* BYE\r\n{logout_tag} OK\r\n").as_bytes()).await.unwrap();
            } else {
                write.write_all(b"+ eyJzdGF0dXMiOiI0MDEiLCJzY2hlbWVzIjoiYmVhcmVyIG1hYyIsInNjb3BlIjoiaHR0cHM6Ly9tYWlsLmdvb2dsZS5jb20vIn0K\r\n").await.unwrap();
                let empty = lines.next_line().await.unwrap().unwrap();
                heard.push(format!("[{empty}]"));
                write.write_all(format!("{tag} NO [AUTHENTICATIONFAILED] Invalid credentials (Failure)\r\n").as_bytes()).await.unwrap();
            }
        }
        heard
    }

    #[test]
    fn imap_authenticate_xoauth2_with_a_fresh_token_after_a_refusal() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = tokio::spawn(imap_stand_in(listener, "fresh-token", 2));
            let connect = || async move {
                let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.map_err(|e| SyncError::Network(e.to_string()))?;
                let mut client = async_imap::Client::new(tcp);
                client.read_response().await.map_err(|e| SyncError::Network(e.to_string()))?;
                Ok(client)
            };
            let asked = std::sync::Mutex::new(Vec::new());
            let token = |fresh: bool| {
                asked.lock().unwrap().push(fresh);
                async move { Ok::<_, SyncError>(if fresh { "fresh-token".to_string() } else { "stale-token".to_string() }) }
            };
            let mut session = crate::imap::sign_in_with_tokens("you@example.org", connect, token, OAuth::Google).await.unwrap();
            session.logout().await.unwrap();
            let heard = server.await.unwrap();
            assert_eq!(*asked.lock().unwrap(), vec![false, true], "the kept token, then a fresh one, once");
            assert_eq!(heard[1], xoauth2_base64("you@example.org", "stale-token"));
            assert_eq!(heard[2], "[]", "Google's challenge answered with an empty line");
            assert_eq!(heard[4], xoauth2_base64("you@example.org", "fresh-token"));
        });
    }

    #[test]
    fn imap_refusing_even_a_fresh_token_asks_to_sign_in_again() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = tokio::spawn(imap_stand_in(listener, "never-given", 2));
            let connect = || async move {
                let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.map_err(|e| SyncError::Network(e.to_string()))?;
                let mut client = async_imap::Client::new(tcp);
                client.read_response().await.map_err(|e| SyncError::Network(e.to_string()))?;
                Ok(client)
            };
            let token = |fresh: bool| async move { Ok::<_, SyncError>(format!("token-{fresh}")) };
            let refused = crate::imap::sign_in_with_tokens("you@example.org", connect, token, OAuth::Google).await.err();
            assert_eq!(refused, Some(SyncError::Login(crate::google::SIGN_IN_AGAIN.into())));
            assert_eq!(server.await.unwrap().len(), 6, "two tries, no third");
            // No token at all (no grant here): said as is, nothing sent.
            let none = |_: bool| async move { Err::<String, _>(SyncError::Login(crate::google::SIGN_IN_AGAIN.into())) };
            let never = || async move { Err::<async_imap::Client<tokio::net::TcpStream>, _>(SyncError::Network("not reached".into())) };
            assert_eq!(crate::imap::sign_in_with_tokens("you@example.org", never, none, OAuth::Google).await.err(), Some(SyncError::Login(crate::google::SIGN_IN_AGAIN.into())));
        });
    }

    /// An SMTP server standing in for Gmail's, without TLS: the first token
    /// refused as Google does (334 with its reason, the empty line, 535), the
    /// second accepted. What the client sent is returned.
    #[test]
    fn smtp_auth_xoauth2_as_google_answers() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let (read, mut write) = stream.into_split();
                let mut lines = BufReader::new(read).lines();
                let mut heard = Vec::new();
                write.write_all(b"220 stand-in ESMTP\r\n").await.unwrap();
                let ehlo = lines.next_line().await.unwrap().unwrap();
                heard.push(ehlo);
                write.write_all(b"250-stand-in\r\n250-AUTH LOGIN PLAIN XOAUTH2 OAUTHBEARER\r\n250 8BITMIME\r\n").await.unwrap();
                for accepted in [false, true] {
                    let auth = lines.next_line().await.unwrap().unwrap();
                    heard.push(auth);
                    if accepted {
                        write.write_all(b"235 2.7.0 Accepted\r\n").await.unwrap();
                    } else {
                        write.write_all(b"334 eyJzdGF0dXMiOiI0MDEiLCJzY2hlbWVzIjoiYmVhcmVyIG1hYyIsInNjb3BlIjoiaHR0cHM6Ly9tYWlsLmdvb2dsZS5jb20vIn0K\r\n").await.unwrap();
                        let empty = lines.next_line().await.unwrap().unwrap();
                        heard.push(format!("[{empty}]"));
                        write.write_all(b"535-5.7.8 Username and Password not accepted.\r\n535 5.7.8 https://support.google.com/mail/?p=BadCredentials\r\n").await.unwrap();
                    }
                }
                heard
            });
            let builder = mail_send::SmtpClientBuilder::new("127.0.0.1", port).unwrap().helo_host("[127.0.0.1]");
            let mut client = builder.connect_plain().await.unwrap();
            let refused = smtp_xoauth2(&mut client, "you@example.org", "stale-token").await.unwrap();
            assert!(refused.as_ref().is_err_and(|why| why.starts_with("535") && why.ends_with("(401 https://mail.google.com/)")), "{refused:?}");
            assert_eq!(smtp_xoauth2(&mut client, "you@example.org", "fresh-token").await.unwrap(), Ok(()));
            let heard = server.await.unwrap();
            assert_eq!(heard[1], format!("AUTH XOAUTH2 {}", xoauth2_base64("you@example.org", "stale-token")));
            assert_eq!(heard[2], "[]", "Google's challenge answered with an empty line");
            assert_eq!(heard[3], format!("AUTH XOAUTH2 {}", xoauth2_base64("you@example.org", "fresh-token")));
        });
    }
}

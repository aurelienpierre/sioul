// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Google's calendars and contacts, over its CalDAV and CardDAV, signed in
//! with OAuth 2.0 as native apps should be (RFC 8252): your browser opens on
//! Google's page, Google sends the answer back to this computer (a loopback
//! address, a port chosen at random), PKCE (RFC 7636) proves it is the same
//! program asking. The OAuth client is Sioul's own when the build was given
//! one (`SIOUL_GOOGLE_CLIENT_ID` and `SIOUL_GOOGLE_CLIENT_SECRET` at build
//! time: a key registered once for Sioul, as Thunderbird has its own), else
//! yours, made once in your Google Cloud project (docs/google.md). What
//! stays: the client and the refresh token, in the system keyring; the access
//! token, in memory.

use crate::SyncError;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// What Sioul asks, all at once (Google adds none later to a desktop app):
/// who you are, your calendars, your contacts, your tasks.
pub const SCOPES: &str = "openid email https://www.googleapis.com/auth/calendar https://www.googleapis.com/auth/carddav https://www.googleapis.com/auth/tasks";
const AUTHORIZE: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const REVOKE: &str = "https://oauth2.googleapis.com/revoke";
/// What a refused refresh token says (`SyncError::Login`): the account must be signed in again.
pub const SIGN_IN_AGAIN: &str = "google-again";
/// The same, about a week after signing in with your own key: Google ended it
/// because your project is still in testing, and says it will again.
pub const IN_TESTING: &str = "google-testing";

/// Sioul's own OAuth client, when the build was given one: a desktop client
/// whose secret is no secret (Google says so of installed applications), the
/// same for everyone. None: each person uses a key of their own.
pub fn built_in() -> Option<(&'static str, &'static str)> {
    match (option_env!("SIOUL_GOOGLE_CLIENT_ID"), option_env!("SIOUL_GOOGLE_CLIENT_SECRET")) {
        (Some(id), Some(secret)) if !id.trim().is_empty() => Some((id, secret)),
        _ => None,
    }
}
/// Where Google's calendars and address books are found.
pub fn caldav_start(email: &str) -> String {
    format!("https://apidata.googleusercontent.com/caldav/v2/{}/user", email.trim())
}
pub const CARDDAV_START: &str = "https://www.googleapis.com/.well-known/carddav";

/// Your OAuth client, and what it was given; kept in the keyring as JSON.
#[derive(Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Grant {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    pub scopes: Vec<String>,
    /// When Google gave it, Unix seconds: a sign-in ended after a week tells a project left in testing.
    #[serde(default)]
    pub signed_in: i64,
}

/// Printed without the refresh token or the secret, should it ever be.
impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grant").field("client_id", &self.client_id).field("scopes", &self.scopes).field("signed_in", &self.signed_in).finish_non_exhaustive()
    }
}

fn keyring_name(email: &str) -> String {
    format!("google:{}", email.trim().to_lowercase())
}

/// The grant kept for an address.
pub fn grant(email: &str) -> Option<Grant> {
    crate::secret::named(&keyring_name(email)).and_then(|t| serde_json::from_str(&t).ok())
}

fn random(bytes: usize) -> String {
    let mut out = Vec::with_capacity(bytes);
    while out.len() < bytes {
        out.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    }
    out.truncate(bytes);
    URL_SAFE_NO_PAD.encode(out)
}

/// A sign-in under way: the page to open, and the door Google comes back to.
pub struct SignIn {
    listener: TcpListener,
    verifier: String,
    state: String,
    redirect: String,
    /// The page to open in your browser.
    pub url: String,
}

fn query(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", encode(v))).collect::<Vec<_>>().join("&")
}

pub(crate) fn encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Opens the door on this computer and makes the page to open: PKCE's
/// challenge (SHA-256 of a random verifier) and a random state.
pub fn begin(client_id: &str, email: &str) -> Result<SignIn, SyncError> {
    let listener = TcpListener::bind("127.0.0.1:0").or_else(|_| TcpListener::bind("[::1]:0")).map_err(|e| SyncError::Network(e.to_string()))?;
    let address = listener.local_addr().map_err(|e| SyncError::Network(e.to_string()))?;
    let redirect = if address.is_ipv6() { format!("http://[::1]:{}", address.port()) } else { format!("http://127.0.0.1:{}", address.port()) };
    let verifier = random(48);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random(16);
    let url = format!(
        "{AUTHORIZE}?{}",
        query(&[
            ("client_id", client_id.trim()),
            ("redirect_uri", &redirect),
            ("response_type", "code"),
            ("scope", SCOPES),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("state", &state),
            ("login_hint", email.trim()),
        ])
    );
    Ok(SignIn { listener, verifier, state, redirect, url })
}

/// The answer Google's page sends back to this computer: its code, checked
/// against the state. `stop` ends the wait (Cancel).
fn wait_for_code(sign_in: &SignIn, deadline: Duration, stop: &AtomicBool) -> Result<String, SyncError> {
    sign_in.listener.set_nonblocking(true).map_err(|e| SyncError::Network(e.to_string()))?;
    let until = Instant::now() + deadline;
    loop {
        if stop.load(Ordering::Relaxed) {
            return Err(SyncError::Message("google-cancelled".into()));
        }
        if Instant::now() > until {
            return Err(SyncError::Message("google-timeout".into()));
        }
        let (mut stream, _) = match sign_in.listener.accept() {
            Ok(accepted) => accepted,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(SyncError::Network(e.to_string())),
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let mut line = String::new();
        if BufReader::new(&stream).read_line(&mut line).is_err() {
            continue;
        }
        // "GET /?state=…&code=…&scope=… HTTP/1.1"; anything else (a favicon) is not it.
        let target = line.split_whitespace().nth(1).unwrap_or("");
        let Some(params) = target.strip_prefix("/?") else {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        };
        let params: HashMap<String, String> = params.split('&').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), decode(v))).collect();
        let (said, result) = match (params.get("state"), params.get("code"), params.get("error")) {
            (Some(state), Some(code), _) if subtle::ConstantTimeEq::ct_eq(state.as_bytes(), sign_in.state.as_bytes()).into() => ("google-page-granted", Ok(code.clone())),
            (_, _, Some(_)) => ("google-page-denied", Err(SyncError::Message("google-denied".into()))),
            _ => ("google-page-foreign", Err(SyncError::Message("google-state".into()))),
        };
        // The page your browser shows, in your language.
        let body = crate::translator().text(said, None).replace('&', "&amp;").replace('<', "&lt;");
        let page = format!("<!doctype html><meta charset=utf-8><title>Sioul</title><p style=\"font:1.2em sans-serif;margin:3em\">{body}</p>");
        let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len()).as_bytes());
        return result;
    }
}

fn post(url: &str, form: &[(&str, &str)]) -> Result<serde_json::Value, SyncError> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).build().into();
    let mut response = agent.post(url).header("Content-Type", "application/x-www-form-urlencoded").send(query(form)).map_err(|e| SyncError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body: serde_json::Value = serde_json::from_str(&response.body_mut().read_to_string().unwrap_or_default()).unwrap_or_default();
    if status == 200 {
        return Ok(body);
    }
    let error = body["error"].as_str().unwrap_or("");
    Err(if error == "invalid_grant" { SyncError::Login(SIGN_IN_AGAIN.into()) } else { SyncError::Server(format!("Google: {status} {error}")) })
}

/// The e-mail the ID token says, from its payload (it came straight from Google over TLS).
fn email_of(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let json: serde_json::Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?).ok()?;
    json["email"].as_str().map(str::to_lowercase)
}

/// Waits for Google's answer (five minutes at most, or until `stop`), trades
/// its code for tokens, checks it is the address you gave, and keeps the
/// grant in the keyring.
pub fn finish(sign_in: SignIn, client_id: &str, client_secret: &str, email: &str, stop: &AtomicBool) -> Result<Grant, SyncError> {
    let code = wait_for_code(&sign_in, Duration::from_secs(300), stop)?;
    let answer = post(
        TOKEN,
        &[("client_id", client_id.trim()), ("client_secret", client_secret.trim()), ("code", &code), ("code_verifier", &sign_in.verifier), ("grant_type", "authorization_code"), ("redirect_uri", &sign_in.redirect)],
    )?;
    // The browser may be signed in to another Google account: its access goes back.
    if let Some(signed_in) = answer["id_token"].as_str().and_then(email_of)
        && signed_in != email.trim().to_lowercase()
    {
        if let Some(token) = answer["refresh_token"].as_str().or(answer["access_token"].as_str()) {
            let _ = post(REVOKE, &[("token", token)]);
        }
        return Err(SyncError::Message(format!("google-other:{signed_in}")));
    }
    let refresh_token = answer["refresh_token"].as_str().ok_or_else(|| SyncError::Server("Google gave no refresh token".into()))?.to_string();
    let scopes: Vec<String> = answer["scope"].as_str().unwrap_or("").split_whitespace().map(str::to_string).collect();
    let signed_in = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let grant = Grant { client_id: client_id.trim().into(), client_secret: client_secret.trim().into(), refresh_token, scopes, signed_in };
    crate::secret::save_named(&keyring_name(email), &serde_json::to_string(&grant).map_err(|e| SyncError::Server(e.to_string()))?)?;
    if let Some(access) = answer["access_token"].as_str() {
        let lasts = answer["expires_in"].as_u64().unwrap_or(3600);
        remember(email, access, lasts);
    }
    Ok(grant)
}

/// Access tokens, by address, until about when they end; in memory only.
static ACCESS: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);

fn remember(email: &str, token: &str, lasts: u64) {
    if let Ok(mut access) = ACCESS.lock() {
        access.get_or_insert_with(HashMap::new).insert(email.trim().to_lowercase(), (token.to_string(), Instant::now() + Duration::from_secs(lasts.saturating_sub(60))));
    }
}

/// An access token for an address: the one in memory while it lasts, else a
/// new one from the refresh token. `fresh` asks a new one (after a 401).
pub fn access_token(email: &str, fresh: bool) -> Result<String, SyncError> {
    // A local stand-in for Google, in tests only (never in Sioul's builds).
    #[cfg(feature = "insecure-test-tls")]
    if let Some(token) = std::env::var_os("SIOUL_TEST_GOOGLE_TOKEN") {
        return Ok(token.to_string_lossy().to_string());
    }
    let key = email.trim().to_lowercase();
    if !fresh
        && let Some((token, until)) = ACCESS.lock().ok().and_then(|a| a.as_ref().and_then(|m| m.get(&key).cloned()))
        && until > Instant::now()
    {
        return Ok(token);
    }
    // No grant kept: Google's page again.
    let grant = grant(email).ok_or_else(|| SyncError::Login(SIGN_IN_AGAIN.into()))?;
    let answer = post(TOKEN, &[("client_id", &grant.client_id), ("client_secret", &grant.client_secret), ("refresh_token", &grant.refresh_token), ("grant_type", "refresh_token")]).map_err(|e| ended(e, &grant))?;
    let token = answer["access_token"].as_str().ok_or_else(|| SyncError::Server("Google gave no access token".into()))?.to_string();
    remember(email, &token, answer["expires_in"].as_u64().unwrap_or(3600));
    Ok(token)
}

/// A refused refresh token about seven days after signing in with a key of
/// your own: Google's limit for projects left in testing, said as such.
fn ended(error: SyncError, grant: &Grant) -> SyncError {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let days = (now - grant.signed_in) as f64 / 86_400.0;
    let own = built_in().is_none_or(|(id, _)| id != grant.client_id);
    match error {
        SyncError::Login(d) if d == SIGN_IN_AGAIN && own && grant.signed_in > 0 && (6.5..9.0).contains(&days) => SyncError::Login(IN_TESTING.into()),
        other => other,
    }
}

/// Gives the access back to Google and forgets it here (the account is removed).
pub fn revoke(email: &str) -> Result<(), SyncError> {
    if let Some(grant) = grant(email) {
        let _ = post(REVOKE, &[("token", &grant.refresh_token)]);
    }
    if let Ok(mut access) = ACCESS.lock()
        && let Some(map) = access.as_mut()
    {
        map.remove(&email.trim().to_lowercase());
    }
    crate::secret::forget_named(&keyring_name(email))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sign_in_page_with_pkce() {
        let sign_in = begin("123.apps.googleusercontent.com", "you@example.org").unwrap();
        assert!(sign_in.url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?client_id=123.apps.googleusercontent.com&redirect_uri=http%3A%2F%2F127.0.0.1%3A"));
        assert!(sign_in.url.contains("&code_challenge_method=S256&") && sign_in.url.contains("&login_hint=you%40example.org"));
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(sign_in.verifier.as_bytes()));
        assert!(sign_in.url.contains(&format!("code_challenge={challenge}&")));
        assert!(sign_in.verifier.len() >= 43, "RFC 7636: 43 characters at least");
    }

    #[test]
    fn google_comes_back_here() {
        let sign_in = begin("id", "you@example.org").unwrap();
        let port = sign_in.listener.local_addr().unwrap().port();
        let state = sign_in.state.clone();
        let browser = std::thread::spawn(move || {
            // A favicon first, then the answer.
            for path in ["/favicon.ico".to_string(), format!("/?state={state}&code=4%2F0Abc&scope=email")] {
                let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes()).unwrap();
                let mut answer = String::new();
                let _ = std::io::Read::read_to_string(&mut stream, &mut answer);
            }
        });
        assert_eq!(wait_for_code(&sign_in, Duration::from_secs(10), &AtomicBool::new(false)).unwrap(), "4/0Abc");
        // Cancel ends the wait.
        let other = begin("id", "you@example.org").unwrap();
        assert!(matches!(wait_for_code(&other, Duration::from_secs(10), &AtomicBool::new(true)), Err(SyncError::Message(m)) if m == "google-cancelled"));
        browser.join().unwrap();
        // The ID token's e-mail, from its payload.
        let payload = URL_SAFE_NO_PAD.encode(br#"{"email":"You@Example.org"}"#);
        assert_eq!(email_of(&format!("x.{payload}.y")).as_deref(), Some("you@example.org"));
    }
}

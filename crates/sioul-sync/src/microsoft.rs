// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Microsoft's mail: Outlook.com, Hotmail, Live and MSN (personal accounts),
//! and Microsoft 365 (work or school accounts), over IMAP and SMTP signed in
//! with OAuth 2.0 (docs/microsoft.md). Microsoft takes no password and no app
//! password from mail programs any more.
//!
//! The sign-in is a native app's (RFC 8252): your browser opens on
//! Microsoft's page, Microsoft sends its answer back to this device
//! (`loopback.rs`, on 127.0.0.1 only: Microsoft takes no IPv6 loopback), and
//! PKCE (RFC 7636) proves it is the same program asking. No client secret,
//! ever: Sioul is a public client, and Microsoft refuses a secret from one.
//! The application is Sioul's own when the build was given one
//! (`SIOUL_MICROSOFT_CLIENT_ID`), else yours, registered once in Microsoft Entra.
//!
//! What stays: the grant (the application, the tenant, the refresh token), in
//! the system keyring only, split over a few entries when it is longer than
//! Windows keeps in one; the access token, in memory. Microsoft gives a new
//! refresh token at each use and asks to keep the newest: it is written back
//! each time. Microsoft offers no way for an app to give a token back:
//! removing an account forgets it here, and you withdraw the consent on your
//! account's page (`forget` says which).
//!
//! A token goes to Microsoft's sign-in (`login.microsoftonline.com`), and to
//! the account's own mail servers, each named exactly (`is_imap_host`,
//! `is_smtp_host`): never to a server a configuration names otherwise.

use crate::SyncError;
use crate::loopback::{self, Verdict};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sioul_core::config::{MICROSOFT_IMAP, MICROSOFT_SMTP_PERSONAL, MICROSOFT_SMTP_WORK};
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

/// Microsoft's sign-in: its page, its token endpoint.
pub const LOGIN: &str = "https://login.microsoftonline.com";
/// The tenant every personal Microsoft account signs in to: an ID token
/// whose `tid` is this one is a personal account's.
pub const CONSUMERS_TID: &str = "9188040d-6c67-4c5b-b112-36a304b66dad";
/// Reading mail over IMAP.
pub const IMAP_SCOPE: &str = "https://outlook.office.com/IMAP.AccessAsUser.All";
/// Sending over SMTP.
pub const SMTP_SCOPE: &str = "https://outlook.office.com/SMTP.Send";
/// What the sign-in asks, all at once: reading, sending, staying signed in,
/// and who you are (no other permission: Microsoft's terms ask the fewest).
pub const MAIL_SCOPES: &str = "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access openid email profile";
/// What a refresh asks: the mail's alone (one resource per token).
pub const REFRESH_SCOPES: &str = "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access";
/// IMAP for every account, on 993, encrypted from the first byte.
pub const IMAP_HOST: &str = MICROSOFT_IMAP;
/// SMTP for personal accounts, on 587 with STARTTLS (Microsoft has no 465).
pub const SMTP_PERSONAL: &str = MICROSOFT_SMTP_PERSONAL;
/// SMTP for work and school accounts, on 587 with STARTTLS.
pub const SMTP_WORK: &str = MICROSOFT_SMTP_WORK;
pub const SMTP_PORT: u16 = 587;
/// Where a personal account withdraws an app's access.
pub const CONSENT_PERSONAL: &str = "https://account.live.com/consent/Manage";
/// Where a work or school account reviews and removes the apps that have access.
pub const CONSENT_WORK: &str = "https://myapps.microsoft.com";
/// Microsoft's page on Outlook.com's IMAP switch (off by default) and its settings.
pub const IMAP_HELP: &str = "https://support.microsoft.com/en-us/office/pop-imap-and-smtp-settings-for-outlook-com-d088b986-291d-42b8-9564-9c414e2aa040";
/// Microsoft's page on Authenticated SMTP switched off by an organisation.
pub const SMTP_AUTH_HELP: &str = "https://aka.ms/smtp_auth_disabled";

// What Microsoft's refusals become (`SyncError::Login`, each said by its own
// sentence: `message_of`): what to do, never a code alone.
/// Microsoft asks you to sign in again (the access withdrawn, a second factor wanted…).
pub const AGAIN: &str = "microsoft-again";
/// The same after a password changed or reset.
pub const AGAIN_PASSWORD: &str = "microsoft-again-password";
/// The same after 90 days without use.
pub const AGAIN_UNUSED: &str = "microsoft-again-unused";
/// The same because your organisation asks you to sign in again every so often.
pub const AGAIN_FREQUENCY: &str = "microsoft-again-frequency";
/// Microsoft's answer reached Sioul after its code had ended (a code lasts about a minute).
pub const AGAIN_LATE: &str = "microsoft-again-late";
/// Your organisation must approve Sioul once.
pub const ADMIN: &str = "microsoft-admin";
/// Your organisation's rules block this app or this sign-in.
pub const BLOCKED: &str = "microsoft-blocked";
/// Microsoft does not know the application (a mistyped id, or a build given a wrong one).
pub const UNKNOWN_APP: &str = "microsoft-unknown-app";
/// The application has no `http://127.0.0.1` redirect under "Mobile and desktop applications".
pub const REDIRECT: &str = "microsoft-redirect";
/// The application's registration takes it for a confidential (web) client.
pub const CONFIDENTIAL: &str = "microsoft-confidential";
/// A secret was sent: a fault of Sioul's, never meant to happen.
pub const SECRET_SENT: &str = "microsoft-secret-sent";
/// The application's registration does not take this kind of account.
pub const ACCOUNT_TYPES: &str = "microsoft-account-types";
/// Microsoft's page gave reading or sending, not both.
pub const SCOPES_MISSING: &str = "microsoft-scopes";
/// A password refused by Microsoft's IMAP server, which takes only its own sign-in.
pub const PASSWORD: &str = "microsoft-password";

// What Sioul says itself (`SyncError::Said`, the message id and what it names).
/// Cancel pressed while Microsoft's page was open.
pub const CANCELLED: &str = "microsoft-cancelled";
/// No answer from Microsoft's page within five minutes.
pub const TIMEOUT: &str = "microsoft-timeout";
/// You declined on Microsoft's page (a personal account).
pub const DECLINED: &str = "microsoft-declined";
/// Microsoft's page ended without access for a work account: declined, or an approval wanted.
pub const DECLINED_WORK: &str = "microsoft-declined-work";
/// Signed in, but the mailbox stays closed to IMAP: Outlook.com's switch, or an activity to confirm.
pub const NOT_CONNECTED_PERSONAL: &str = "sync-said-microsoft-not-connected-personal";
/// Signed in, but the mailbox stays closed to IMAP: the organisation's choice, or another account chosen.
pub const NOT_CONNECTED_WORK: &str = "sync-said-microsoft-not-connected-work";
/// The organisation switched off sending by mail programs (Authenticated SMTP).
pub const SMTP_OFF: &str = "sync-said-microsoft-smtp-off";
/// An account signed in with Microsoft whose configuration names another server: nothing sent there.
pub const OTHER_HOST: &str = "sync-said-microsoft-other-host";
/// The mailbox did not open with the account Microsoft's page signed in (that account follows).
pub const OTHER_ACCOUNT: &str = "microsoft-other-account";
/// The mailbox refused the access Microsoft had just given, or stayed closed to IMAP: its switch, most often.
pub const MAILBOX_REFUSED: &str = "microsoft-mailbox-refused";

/// The message that says a `SyncError::Login` of this module ("sync-error-microsoft-again").
pub(crate) fn message_of(detail: &str) -> Option<&'static str> {
    Some(match detail {
        AGAIN => "sync-error-microsoft-again",
        AGAIN_PASSWORD => "sync-error-microsoft-again-password",
        AGAIN_UNUSED => "sync-error-microsoft-again-unused",
        AGAIN_FREQUENCY => "sync-error-microsoft-again-frequency",
        AGAIN_LATE => "sync-error-microsoft-again-late",
        ADMIN => "sync-error-microsoft-admin",
        BLOCKED => "sync-error-microsoft-blocked",
        UNKNOWN_APP => "sync-error-microsoft-unknown-app",
        REDIRECT => "sync-error-microsoft-redirect",
        CONFIDENTIAL => "sync-error-microsoft-confidential",
        SECRET_SENT => "sync-error-microsoft-secret-sent",
        ACCOUNT_TYPES => "sync-error-microsoft-account-types",
        SCOPES_MISSING => "sync-error-microsoft-scopes",
        PASSWORD => "sync-error-microsoft-password",
        _ => return None,
    })
}

/// Sioul's own application, when the build was given one
/// (`SIOUL_MICROSOFT_CLIENT_ID`): an id, no secret (Microsoft refuses a
/// secret from a program on your device). The same on every system, the
/// phone's included. None: each person gives an application of their own.
pub fn built_in() -> Option<&'static str> {
    option_env!("SIOUL_MICROSOFT_CLIENT_ID").map(str::trim).filter(|id| !id.is_empty())
}

/// What discovery makes of an address before anyone signs in: a personal
/// account's (Outlook.com's domains, or a domain whose mail Outlook.com
/// receives), a work or school account's (Microsoft 365), or not known.
/// The ID token's `tid` settles it at sign-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Personal,
    Work,
    Unknown,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Personal => "personal",
            Kind::Work => "work",
            Kind::Unknown => "unknown",
        }
    }

    pub fn parse(text: &str) -> Kind {
        match text {
            "personal" => Kind::Personal,
            "work" => Kind::Work,
            _ => Kind::Unknown,
        }
    }

    /// The sending server of this kind (work's when not known: Microsoft 365's).
    pub fn smtp_host(self) -> &'static str {
        match self {
            Kind::Personal => SMTP_PERSONAL,
            Kind::Work | Kind::Unknown => SMTP_WORK,
        }
    }
}

/// Whose accounts the sign-in takes: the `{tenant}` of Microsoft's endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authority {
    /// Personal Microsoft accounts only.
    Consumers,
    /// Work and school accounts only.
    Organizations,
    /// Either; Microsoft's page lets you choose.
    Common,
    /// One organisation's directory, by its id or one of its domains (an
    /// application registered for that organisation alone needs it).
    Tenant(String),
}

impl Authority {
    /// The authority a kind of address signs in with.
    pub fn of(kind: Kind) -> Authority {
        match kind {
            Kind::Personal => Authority::Consumers,
            Kind::Work => Authority::Organizations,
            Kind::Unknown => Authority::Common,
        }
    }

    /// A directory given by hand: its id (a GUID) or a domain, nothing else
    /// (it goes into the path of Microsoft's address). None when empty.
    pub fn tenant(given: &str) -> Result<Option<Authority>, SyncError> {
        let given = given.trim().to_ascii_lowercase();
        if given.is_empty() {
            return Ok(None);
        }
        if is_tenant(&given) { Ok(Some(Authority::Tenant(given))) } else { Err(SyncError::Login(UNKNOWN_APP.into())) }
    }

    /// The path segment.
    pub fn segment(&self) -> &str {
        match self {
            Authority::Consumers => "consumers",
            Authority::Organizations => "organizations",
            Authority::Common => "common",
            Authority::Tenant(tenant) => tenant,
        }
    }
}

/// A directory's id or domain as Microsoft's paths take it: letters, digits,
/// dots and hyphens, at most a domain's length; nothing that could reach
/// another path or host.
fn is_tenant(text: &str) -> bool {
    !text.is_empty() && text.len() <= 253 && !text.starts_with(['.', '-']) && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

/// Whether an IMAP server is Microsoft's, named exactly: an access token is given to no other.
pub fn is_imap_host(host: &str) -> bool {
    host.eq_ignore_ascii_case(IMAP_HOST)
}

/// Whether an SMTP server is one of Microsoft's two, named exactly.
pub fn is_smtp_host(host: &str) -> bool {
    host.eq_ignore_ascii_case(SMTP_PERSONAL) || host.eq_ignore_ascii_case(SMTP_WORK)
}

/// What the keyring keeps for an address signed in with Microsoft.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Grant {
    /// The application signed in with: Sioul's, or yours.
    pub client_id: String,
    /// Where refreshes go: `consumers` for a personal account, the
    /// organisation's id (`tid`) for a work or school one.
    pub tenant: String,
    /// A personal account (Outlook.com, Hotmail, Live, MSN).
    pub personal: bool,
    pub refresh_token: String,
    pub scopes: Vec<String>,
    /// When you signed in on Microsoft's page, Unix seconds: kept through
    /// refreshes, so that it marks a sign-in (`sasl::OAuth::mark`).
    pub signed_in: i64,
    /// Who Microsoft's page signed in: its address (the ID token's `email`,
    /// else `preferred_username`); maybe not the address given (an alias).
    pub signed_in_as: String,
}

/// Printed without the refresh token, should it ever be.
impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grant")
            .field("client_id", &self.client_id)
            .field("tenant", &self.tenant)
            .field("personal", &self.personal)
            .field("scopes", &self.scopes)
            .field("signed_in", &self.signed_in)
            .field("signed_in_as", &self.signed_in_as)
            .finish_non_exhaustive()
    }
}

/// The grant as the keyring keeps it: the refresh token's first part in
/// this entry, the rest in `parts` more (`<name>#2`, `<name>#3`…), and the
/// start of the whole token's SHA-256, to tell a whole token from parts
/// written at different times.
#[derive(serde::Serialize, serde::Deserialize)]
struct Kept {
    client_id: String,
    tenant: String,
    personal: bool,
    refresh_token: String,
    #[serde(default)]
    parts: usize,
    #[serde(default)]
    check: String,
    scopes: Vec<String>,
    signed_in: i64,
    #[serde(default)]
    signed_in_as: String,
}

/// At most this many entries after the first: a token of about 9 KB, several times the longest reported.
const MOST_PARTS: usize = 3;

/// Its entry in the keyring: `microsoft-mail:<login>`, the login lowercased.
fn keyring_name(login: &str) -> String {
    format!("microsoft-mail:{}", login.trim().to_lowercase())
}

/// Where grants are kept: the system keyring, or, in tests, memory.
trait Store {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>, SyncError>;
    fn set(&self, name: &str, value: &[u8]) -> Result<(), SyncError>;
    fn forget(&self, name: &str) -> Result<(), SyncError>;
}

/// The system keyring, through `secret.rs`.
struct Keyring;

impl Store for Keyring {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>, SyncError> {
        crate::secret::named_bytes(name)
    }
    fn set(&self, name: &str, value: &[u8]) -> Result<(), SyncError> {
        crate::secret::save_named_bytes(name, value)
    }
    fn forget(&self, name: &str) -> Result<(), SyncError> {
        crate::secret::forget_named(name)
    }
}

fn check_of(token: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(token.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// The grant written under `name`: the first entry at most `secret::ENTRY_BYTES`
/// long, the token's rest in parts as long; the parts first, the first entry
/// last, then the parts an older, longer token left (at most `before`).
fn save_in(store: &dyn Store, name: &str, grant: &Grant, before: usize) -> Result<(), SyncError> {
    let limit = crate::secret::ENTRY_BYTES;
    let token = grant.refresh_token.as_str();
    let mut kept = Kept {
        client_id: grant.client_id.clone(),
        tenant: grant.tenant.clone(),
        personal: grant.personal,
        refresh_token: String::new(),
        parts: 0,
        check: check_of(token),
        scopes: grant.scopes.clone(),
        signed_in: grant.signed_in,
        signed_in_as: grant.signed_in_as.clone(),
    };
    let json = |kept: &Kept| serde_json::to_vec(kept).map_err(|e| SyncError::Keyring(e.to_string()));
    // What the first entry holds besides the token, with room for a part count.
    kept.parts = MOST_PARTS;
    let mut first = token.len().min(limit.saturating_sub(json(&kept)?.len()));
    // A token with characters JSON escapes takes more room than its length: less of it, then.
    loop {
        while !token.is_char_boundary(first) {
            first -= 1;
        }
        kept.refresh_token = token[..first].to_string();
        if json(&kept)?.len() <= limit || first == 0 {
            break;
        }
        first = first.saturating_sub(64);
    }
    let rest = &token[first..];
    let mut parts = Vec::new();
    let mut at = 0;
    while at < rest.len() {
        let mut end = (at + limit).min(rest.len());
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        parts.push(&rest[at..end]);
        at = end;
    }
    kept.parts = parts.len();
    if kept.parts > MOST_PARTS {
        return Err(SyncError::Keyring(format!("{name}: a refresh token of {} bytes, longer than Sioul keeps", token.len())));
    }
    for (index, part) in parts.iter().enumerate() {
        store.set(&format!("{name}#{}", index + 2), part.as_bytes())?;
    }
    store.set(name, &json(&kept)?)?;
    for index in kept.parts + 2..=before + 1 {
        store.forget(&format!("{name}#{index}"))?;
    }
    Ok(())
}

/// The grant kept under `name`, its token joined again from its parts; None
/// when there is none, or when its parts do not make the token it was (a
/// write cut short): then you sign in again, rather than Sioul sending a token
/// that is not one. The number of parts comes along, for the next write.
fn load_from(store: &dyn Store, name: &str) -> Result<Option<(Grant, usize)>, SyncError> {
    let Some(bytes) = store.get(name)? else { return Ok(None) };
    let Ok(kept) = serde_json::from_slice::<Kept>(&bytes) else { return Ok(None) };
    let mut token = kept.refresh_token.clone();
    for index in 2..kept.parts.min(MOST_PARTS) + 2 {
        match store.get(&format!("{name}#{index}"))? {
            Some(part) => token.push_str(&String::from_utf8_lossy(&part)),
            None => return Ok(None),
        }
    }
    if kept.parts > MOST_PARTS || (!kept.check.is_empty() && kept.check != check_of(&token)) {
        return Ok(None);
    }
    let grant = Grant {
        client_id: kept.client_id,
        tenant: kept.tenant,
        personal: kept.personal,
        refresh_token: token,
        scopes: kept.scopes,
        signed_in: kept.signed_in,
        signed_in_as: kept.signed_in_as,
    };
    Ok(Some((grant, kept.parts)))
}

/// The grant kept on this device for a login (the account's address, or the
/// user name its mailbox opened with).
pub fn grant(login: &str) -> Option<Grant> {
    load_from(&Keyring, &keyring_name(login)).ok().flatten().map(|(grant, _)| grant)
}

/// Keeps a grant for a login, replacing the one kept.
fn keep(login: &str, grant: &Grant) -> Result<(), SyncError> {
    let name = keyring_name(login);
    let before = load_from(&Keyring, &name).ok().flatten().map_or(MOST_PARTS, |(_, parts)| parts);
    save_in(&Keyring, &name, grant, before)
}

/// An application of your own kept on this device for an address (its id,
/// and the directory refreshes go to when it was given one): to sign in
/// with again. Sioul's own is not "yours".
pub fn own_app(login: &str) -> Option<String> {
    grant(login).map(|g| g.client_id).filter(|id| !id.is_empty() && Some(id.as_str()) != built_in())
}

/// A sign-in under way: the page to open, and the door Microsoft comes back to.
pub struct SignIn {
    listener: TcpListener,
    verifier: String,
    state: String,
    redirect: String,
    authority: Authority,
    /// The page to open in your browser.
    pub url: String,
}

/// Opens the door on this device (127.0.0.1 only) and makes Microsoft's page
/// to open: PKCE's challenge (S256), a random state, your address as a hint,
/// and the account chooser, since a browser often holds more than one
/// Microsoft account, and a personal and a work account may share an address.
pub fn begin(client_id: &str, authority: &Authority, address: &str) -> Result<SignIn, SyncError> {
    let (listener, redirect) = loopback::open(false)?;
    let verifier = loopback::random(48);
    let challenge = loopback::challenge(&verifier);
    let state = loopback::random(16);
    let url = format!(
        "{LOGIN}/{}/oauth2/v2.0/authorize?{}",
        authority.segment(),
        loopback::query(&[
            ("client_id", client_id.trim()),
            ("response_type", "code"),
            ("redirect_uri", &redirect),
            ("response_mode", "query"),
            ("scope", MAIL_SCOPES),
            ("state", &state),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("login_hint", address.trim()),
            ("prompt", "select_account"),
        ])
    );
    Ok(SignIn { listener, verifier, state, redirect, authority: authority.clone(), url })
}

/// When in the sign-in a refusal came: it says different things.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Microsoft's page sent you back with an error.
    Page,
    /// The code traded for tokens.
    Redeem,
    /// A refresh token traded for an access token.
    Refresh,
}

/// The first `AADSTS<digits>` of Entra's description.
fn aadsts(description: &str) -> Option<u64> {
    let at = description.find("AADSTS")? + 6;
    let digits: String = description[at..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// What Microsoft's refusal means, as Sioul says it: OAuth's `error`,
/// Entra's codes (`error_codes`, else the description's AADSTS), the
/// account's kind when known, and the step.
fn refusal(status: u16, error: &str, codes: &[u64], description: &str, personal: Option<bool>, step: Step) -> SyncError {
    let code = codes.first().copied().or_else(|| aadsts(description));
    let login = |id: &str| SyncError::Login(id.to_string());
    match (error, code) {
        (_, Some(50173)) => login(AGAIN_PASSWORD),
        (_, Some(70008 | 700082 | 54005)) if step != Step::Refresh => login(AGAIN_LATE),
        (_, Some(70008 | 700082)) => login(AGAIN_UNUSED),
        (_, Some(70043)) => login(AGAIN_FREQUENCY),
        // A second factor asked: only Microsoft's page can.
        (_, Some(50076 | 50079 | 50074)) => login(AGAIN),
        (_, Some(90094 | 90008)) => login(ADMIN),
        (_, Some(65004)) | ("access_denied", _) => SyncError::Said(if personal == Some(true) { DECLINED } else { DECLINED_WORK }, String::new()),
        ("consent_required", _) | (_, Some(65001)) => login(if personal == Some(true) { AGAIN } else { ADMIN }),
        (_, Some(53003 | 530035)) => login(BLOCKED),
        (_, Some(700016)) | ("unauthorized_client", _) => login(UNKNOWN_APP),
        (_, Some(50011)) => login(REDIRECT),
        (_, Some(7000218)) => login(CONFIDENTIAL),
        (_, Some(700025)) => login(SECRET_SENT),
        (_, Some(50020 | 50194)) => login(ACCOUNT_TYPES),
        ("invalid_grant", _) if step == Step::Redeem => login(AGAIN_LATE),
        ("invalid_grant" | "interaction_required" | "login_required", _) => login(AGAIN),
        // Its words as they came: the status (none from Microsoft's page), the error, the code.
        _ => {
            let status = (status != 0).then(|| status.to_string()).unwrap_or_default();
            let said = [status, error.to_string(), code.map(|c| format!("AADSTS{c}")).unwrap_or_default()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
            SyncError::Server(format!("Microsoft: {said}"))
        }
    }
}

/// Posts a form to Microsoft's token endpoint (or a stand-in in tests):
/// HTTPS only, no redirect followed (a refresh token goes nowhere else), no
/// secret. The answer's JSON, else its refusal said (`refusal`).
fn post(url: &str, form: &[(&str, &str)], personal: Option<bool>, step: Step) -> Result<serde_json::Value, SyncError> {
    #[cfg(not(test))]
    if !url.starts_with(&format!("{LOGIN}/")) {
        return Err(SyncError::Elsewhere(url.split('?').next().unwrap_or_default().to_string()));
    }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).max_redirects(0).https_only(!cfg!(test)).build().into();
    let mut response = agent.post(url).header("Content-Type", "application/x-www-form-urlencoded").send(loopback::query(form)).map_err(|e| SyncError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body: serde_json::Value = serde_json::from_str(&response.body_mut().read_to_string().unwrap_or_default()).unwrap_or_default();
    if status == 200 {
        return Ok(body);
    }
    let codes: Vec<u64> = body["error_codes"].as_array().map(|codes| codes.iter().filter_map(serde_json::Value::as_u64).collect()).unwrap_or_default();
    Err(refusal(status, body["error"].as_str().unwrap_or(""), &codes, body["error_description"].as_str().unwrap_or(""), personal, step))
}

/// What the ID token says, from its payload (it came straight from
/// Microsoft's token endpoint over TLS; Sioul only reads who signed in, and
/// the server's own login decides): `tid`, and the address.
fn claims(id_token: &str) -> Option<(String, String)> {
    let payload = id_token.split('.').nth(1)?;
    let json: serde_json::Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?).ok()?;
    let tid = json["tid"].as_str().unwrap_or("").to_ascii_lowercase();
    // An address: `email`, else `preferred_username` when it is one (a phone number is not).
    let address = [&json["email"], &json["preferred_username"]].into_iter().filter_map(|v| v.as_str()).map(|v| v.trim().to_lowercase()).find(|v| v.contains('@')).unwrap_or_default();
    Some((tid, address))
}

/// Whether Microsoft gave what mail needs: reading and sending. An answer
/// without its scopes gives what was asked (Microsoft says so).
fn gave_mail(scopes: &[String]) -> bool {
    let has = |wanted: &str| scopes.iter().any(|s| s.eq_ignore_ascii_case(wanted) || s.rsplit('/').next().is_some_and(|last| wanted.ends_with(&format!("/{last}")) && last.contains('.')));
    scopes.is_empty() || (has(IMAP_SCOPE) && has(SMTP_SCOPE))
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The answer Microsoft's page sends back to this device: its code, checked
/// against the state. An answer whose state is not this sign-in's is
/// refused, and the wait goes on: a page from an earlier sign-in, or anyone
/// else, cannot end it. An error is read only from an answer whose state is.
fn wait_for_code(sign_in: &SignIn, deadline: Duration, stop: &AtomicBool, personal: Option<bool>) -> Result<String, SyncError> {
    let ends = loopback::Ends { cancelled: SyncError::Said(CANCELLED, String::new()), timed_out: SyncError::Said(TIMEOUT, String::new()) };
    loopback::wait(&sign_in.listener, &sign_in.state, deadline, stop, ends, |answer| {
        // On a phone, the way back to Sioul, whose code lasts a minute.
        let granted = if cfg!(target_os = "android") { "microsoft-page-granted-phone" } else { "microsoft-page-granted" };
        match (answer.state_ok, answer.get("code"), answer.get("error")) {
            (true, Some(code), _) => Verdict::Done(granted, Ok(code.to_string())),
            (true, _, Some(error)) => Verdict::Done("microsoft-page-refused", Err(refusal(0, error, &[], answer.get("error_description").unwrap_or(""), personal, Step::Page))),
            _ => Verdict::Ignored("microsoft-page-foreign"),
        }
    })
}

/// Waits for Microsoft's answer (five minutes at most, or until `stop`),
/// trades its code for tokens at once (a code lasts about a minute), reads
/// who signed in and whether the account is a personal or a work one, and
/// keeps the grant in the keyring under `address`. The access token is kept
/// in memory. Who signed in is not checked against the address here: an
/// alias, a shared mailbox, a phone number's sign-in differ; the mail
/// server's own login decides (the window's `outlook.rs`).
pub fn finish(sign_in: SignIn, client_id: &str, address: &str, stop: &AtomicBool) -> Result<Grant, SyncError> {
    finish_at(sign_in, client_id, address, stop, None, &Keyring)
}

/// `finish`, the code traded at `token_url` (Microsoft's, or a stand-in in
/// tests), the grant kept in `store`.
fn finish_at(sign_in: SignIn, client_id: &str, address: &str, stop: &AtomicBool, token_url: Option<&str>, store: &dyn Store) -> Result<Grant, SyncError> {
    let hinted = match sign_in.authority {
        Authority::Consumers => Some(true),
        Authority::Organizations | Authority::Tenant(_) => Some(false),
        Authority::Common => None,
    };
    let code = wait_for_code(&sign_in, Duration::from_secs(300), stop, hinted)?;
    let endpoint = token_url.map_or_else(|| format!("{LOGIN}/{}/oauth2/v2.0/token", sign_in.authority.segment()), str::to_string);
    let answer = post(
        &endpoint,
        &[("client_id", client_id.trim()), ("scope", MAIL_SCOPES), ("code", &code), ("redirect_uri", &sign_in.redirect), ("grant_type", "authorization_code"), ("code_verifier", &sign_in.verifier)],
        hinted,
        Step::Redeem,
    )?;
    let refresh_token = answer["refresh_token"].as_str().filter(|t| !t.is_empty()).ok_or_else(|| SyncError::Server("Microsoft gave no refresh token".into()))?.to_string();
    let scopes: Vec<String> = answer["scope"].as_str().unwrap_or("").split_whitespace().map(str::to_string).collect();
    if !gave_mail(&scopes) {
        return Err(SyncError::Login(SCOPES_MISSING.into()));
    }
    let (tid, signed_in_as) = answer["id_token"].as_str().and_then(claims).unwrap_or_default();
    // Personal or work: the token's tenant says; without one, the authority asked.
    let personal = if tid.is_empty() { hinted.unwrap_or(false) } else { tid == CONSUMERS_TID };
    let tenant = if personal {
        "consumers".to_string()
    } else if is_tenant(&tid) {
        tid
    } else {
        sign_in.authority.segment().to_string()
    };
    let grant = Grant { client_id: client_id.trim().into(), tenant, personal, refresh_token, scopes, signed_in: now(), signed_in_as };
    let name = keyring_name(address);
    let before = load_from(store, &name).ok().flatten().map_or(MOST_PARTS, |(_, parts)| parts);
    save_in(store, &name, &grant, before)?;
    if let Some(access) = answer["access_token"].as_str() {
        remember(&name, access, answer["expires_in"].as_u64().unwrap_or(3600));
    }
    Ok(grant)
}

/// The grant kept for `from` moved to `to` (the user name a mailbox opened
/// with, when it is not the address given), its access token with it.
pub fn move_grant(from: &str, to: &str) -> Result<(), SyncError> {
    let (from_name, to_name) = (keyring_name(from), keyring_name(to));
    if from_name == to_name {
        return Ok(());
    }
    let (grant, parts) = load_from(&Keyring, &from_name)?.ok_or_else(|| SyncError::Login(AGAIN.into()))?;
    keep(to, &grant)?;
    let access = ACCESS.lock().ok().and_then(|a| a.as_ref().and_then(|m| m.get(&from_name).cloned()));
    if let Some((token, until)) = access {
        remember_until(&to_name, &token, until);
    }
    forget_entries(&from_name, parts)
}

/// Access tokens, by grant (its keyring name), until about when they end; in memory only.
static ACCESS: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);

fn remember(key: &str, token: &str, lasts: u64) {
    remember_until(key, token, Instant::now() + Duration::from_secs(lasts.saturating_sub(60)));
}

fn remember_until(key: &str, token: &str, until: Instant) {
    if let Ok(mut access) = ACCESS.lock() {
        access.get_or_insert_with(HashMap::new).insert(key.to_string(), (token.to_string(), until));
    }
}

/// The access token kept in memory under a grant's name, while it lasts.
fn remembered(key: &str) -> Option<String> {
    ACCESS.lock().ok().and_then(|a| a.as_ref().and_then(|m| m.get(key).cloned())).filter(|(_, until)| *until > Instant::now()).map(|(token, _)| token)
}

/// A local stand-in's token, in test builds only (`insecure-test-tls`, never
/// in Sioul's builds): with it, an account signed in with Microsoft may be
/// pointed at a local IMAP and SMTP server for a test (`OAuth::may_reach`).
pub(crate) fn test_token() -> Option<String> {
    #[cfg(feature = "insecure-test-tls")]
    if let Some(token) = std::env::var_os("SIOUL_TEST_MICROSOFT_TOKEN") {
        return Some(token.to_string_lossy().to_string());
    }
    None
}

/// An access token for a login's mail (IMAP and SMTP alike): the one in
/// memory while it lasts, else a new one from the refresh token, whose
/// successor is kept. `fresh` asks a new one (after a refusal).
pub fn access_token(login: &str, fresh: bool) -> Result<String, SyncError> {
    if let Some(token) = test_token() {
        return Ok(token);
    }
    let key = keyring_name(login);
    if !fresh && let Some(token) = remembered(&key) {
        return Ok(token);
    }
    // No grant kept: Microsoft's page again.
    let (grant, parts) = load_from(&Keyring, &key)?.ok_or_else(|| SyncError::Login(AGAIN.into()))?;
    // The directory kept, checked as a typed one is: it goes into the path of Microsoft's address.
    let tenant = if is_tenant(&grant.tenant) { grant.tenant.as_str() } else { "common" };
    let endpoint = format!("{LOGIN}/{tenant}/oauth2/v2.0/token");
    let (token, lasts) = refreshed(&endpoint, &grant, &Keyring, &key, parts)?;
    remember(&key, &token, lasts);
    Ok(token)
}

/// A new access token traded for a grant's refresh token at `endpoint`, and
/// how long it lasts, in seconds. Microsoft's answer brings a new refresh
/// token: kept in `store` in place of the old one (Microsoft asks it; the old
/// one keeps working until its own end, so a write that fails is said and
/// does not stop the mail, and two of Sioul's processes refreshing at once
/// do not lock each other out: the last one written stays).
fn refreshed(endpoint: &str, grant: &Grant, store: &dyn Store, name: &str, parts: usize) -> Result<(String, u64), SyncError> {
    let answer = post(
        endpoint,
        &[("client_id", &grant.client_id), ("grant_type", "refresh_token"), ("refresh_token", &grant.refresh_token), ("scope", REFRESH_SCOPES)],
        Some(grant.personal),
        Step::Refresh,
    )?;
    let token = answer["access_token"].as_str().ok_or_else(|| SyncError::Server("Microsoft gave no access token".into()))?.to_string();
    if let Some(next) = answer["refresh_token"].as_str().filter(|next| !next.is_empty() && *next != grant.refresh_token) {
        let renewed = Grant { refresh_token: next.to_string(), ..grant.clone() };
        if let Err(e) = save_in(store, name, &renewed, parts) {
            eprintln!("sioul: Microsoft's new refresh token for {} could not be kept ({e}); the one kept still works until its own end", name.trim_start_matches("microsoft-mail:"));
        }
    }
    Ok((token, answer["expires_in"].as_u64().unwrap_or(3600)))
}

fn forget_entries(name: &str, parts: usize) -> Result<(), SyncError> {
    if let Ok(mut access) = ACCESS.lock()
        && let Some(map) = access.as_mut()
    {
        map.remove(name);
    }
    // The entry and every part it may have had: those of the longest token kept at least.
    crate::secret::forget_named_all(name, parts.max(MOST_PARTS) + 1)
}

/// Forgets a login's grant on this device, keyring and memory. Microsoft is
/// not asked anything (it offers apps no way to give a token back): the
/// page where you withdraw Sioul's access comes back, a personal account's
/// or a work account's.
pub fn forget(login: &str, personal: bool) -> Result<&'static str, SyncError> {
    let name = keyring_name(login);
    let parts = load_from(&Keyring, &name).ok().flatten().map_or(MOST_PARTS, |(_, parts)| parts);
    forget_entries(&name, parts)?;
    Ok(if personal { CONSENT_PERSONAL } else { CONSENT_WORK })
}

/// Whether a text is an application's id as Microsoft gives them: a GUID
/// (`00000000-0000-0000-0000-000000000000`). Anything else (a secret pasted
/// in its place) is refused before Microsoft is asked.
pub fn is_app_id(text: &str) -> bool {
    let text = text.trim();
    text.len() == 36 && text.char_indices().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() })
}

/// The application to sign in with: the id given (yours), else yours kept
/// on this device for this address, else Sioul's own. None when this build
/// has none and none is given or kept: then each person gives their own.
pub fn app_for(login: &str, given: &str) -> Result<Option<String>, SyncError> {
    let given = given.trim();
    if !given.is_empty() {
        return if is_app_id(given) { Ok(Some(given.to_ascii_lowercase())) } else { Err(SyncError::Login(UNKNOWN_APP.into())) };
    }
    Ok(own_app(login).or_else(|| built_in().map(str::to_string)))
}

/// The authority a sign-in goes to: the directory given (an application of
/// your own registered for one organisation), else the kept grant's (a
/// personal account's, or its organisation's), else the kind discovery found.
pub fn authority_for(login: &str, kind: Kind, tenant: &str) -> Result<Authority, SyncError> {
    if let Some(given) = Authority::tenant(tenant)? {
        return Ok(given);
    }
    Ok(match grant(login) {
        Some(kept) if kept.personal => Authority::Consumers,
        Some(kept) if is_tenant(&kept.tenant) && kept.tenant != "common" => Authority::Tenant(kept.tenant),
        _ => Authority::of(kind),
    })
}

/// Signs an address's mail in with Microsoft, or signs it in again:
/// Microsoft's page (`show` opens it: the window's browser, the terminal's
/// line), its answer, then the mailbox itself tried with the access given,
/// before anything is kept: as the address, else as the account Microsoft's
/// page signed in (an alias's mailbox opens under its primary address).
/// `account` comes back at Microsoft's servers, signed in with Microsoft, its
/// sending server the one of the kind the sign-in found, its user name set
/// when the mailbox opened under another; a grant kept under its former login
/// is forgotten. When the mailbox does not open, nothing is kept: the grant
/// is forgotten too.
pub fn sign_in(mut account: sioul_core::config::Account, client_id: &str, authority: &Authority, show: impl FnOnce(&str), stop: &AtomicBool) -> Result<(sioul_core::config::Account, Grant), SyncError> {
    use sioul_core::config::Security;
    let address = account.address.clone().unwrap_or_default().trim().to_lowercase();
    let before = account.login().map(str::to_lowercase).filter(|login| account.is_microsoft() && *login != address);
    let started = begin(client_id, authority, &address)?;
    show(&started.url);
    let grant = finish(started, client_id, &address, stop)?;
    account.auth = Some("microsoft".into());
    account.host = Some(IMAP_HOST.into());
    account.port = Some(993);
    account.security = Security::Tls;
    account.username = None;
    account.smtp_host = Some(if grant.personal { SMTP_PERSONAL } else { SMTP_WORK }.into());
    account.smtp_port = Some(SMTP_PORT);
    account.smtp_security = Security::Starttls;
    // The mailbox's own refusals: the token refused, or IMAP closed. Anything else is said as it is.
    let refused = |e: &SyncError| matches!(e, SyncError::Login(d) if d == AGAIN) || matches!(e, SyncError::Said(id, _) if *id == NOT_CONNECTED_PERSONAL || *id == NOT_CONNECTED_WORK);
    let other = grant.signed_in_as.clone();
    let opened = match crate::test(&account, "") {
        Ok(_) => Ok(()),
        // Refused as the address: as the account Microsoft's page signed in, once.
        Err(first) if refused(&first) && !other.is_empty() && other != address => {
            // Moved, or forgotten: never a grant left behind under the address.
            move_grant(&address, &other).inspect_err(|_| drop(forget(&address, grant.personal)))?;
            account.username = Some(other.clone());
            crate::test(&account, "").map(drop).map_err(|second| {
                let _ = forget(&other, grant.personal);
                if refused(&second) { SyncError::Said(OTHER_ACCOUNT, other.clone()) } else { second }
            })
        }
        // Nothing kept: the mailbox's switch, then a sign-in again.
        Err(first) => {
            let _ = forget(&address, grant.personal);
            Err(if refused(&first) { SyncError::Said(MAILBOX_REFUSED, String::new()) } else { first })
        }
    };
    opened?;
    // Kept under another login before (an alias's): that grant is the old one.
    if let Some(before) = before.filter(|b| Some(b.as_str()) != account.login().map(str::to_lowercase).as_deref()) {
        let _ = forget(&before, grant.personal);
    }
    Ok((account, grant))
}

/// The link an organisation's administrator opens to approve Sioul for
/// everyone there, once: Microsoft's admin consent, for the organisation's
/// directory (its id, or the domain of the address; never `common`).
pub fn admin_consent_url(client_id: &str, tenant: &str) -> String {
    let tenant = Some(tenant.trim().to_ascii_lowercase()).filter(|t| is_tenant(t)).unwrap_or_else(|| "organizations".into());
    format!("{LOGIN}/{tenant}/v2.0/adminconsent?{}", loopback::query(&[("client_id", client_id.trim()), ("scope", MAIL_SCOPES), ("redirect_uri", "http://127.0.0.1"), ("state", "sioul")]))
}

/// Microsoft's IMAP refusal that is not about the token: "User is
/// authenticated but not connected", the mailbox closed to IMAP. Said for
/// what it is, with what to do; no second token is asked.
pub(crate) fn not_connected(personal: bool, refusal: &str) -> Option<SyncError> {
    refusal
        .to_ascii_lowercase()
        .contains("authenticated but not connected")
        .then(|| SyncError::Said(if personal { NOT_CONNECTED_PERSONAL } else { NOT_CONNECTED_WORK }, refusal.trim().to_string()))
}

/// Microsoft's SMTP refusal that no token changes: Authenticated SMTP
/// switched off for the organisation or the mailbox (`5.7.139`).
pub(crate) fn smtp_refused(refusal: &str) -> Option<SyncError> {
    let off = refusal.contains("5.7.139") || refusal.to_ascii_lowercase().contains("smtpclientauthentication is disabled");
    off.then(|| SyncError::Said(SMTP_OFF, refusal.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    /// The keyring, stood in for in memory: no test reaches the system's.
    #[derive(Default)]
    struct Memory(Mutex<HashMap<String, Vec<u8>>>);

    impl Store for Memory {
        fn get(&self, name: &str) -> Result<Option<Vec<u8>>, SyncError> {
            Ok(self.0.lock().unwrap().get(name).cloned())
        }
        fn set(&self, name: &str, value: &[u8]) -> Result<(), SyncError> {
            assert!(value.len() <= crate::secret::ENTRY_BYTES, "{name}: {} bytes, more than Windows keeps", value.len());
            self.0.lock().unwrap().insert(name.to_string(), value.to_vec());
            Ok(())
        }
        fn forget(&self, name: &str) -> Result<(), SyncError> {
            self.0.lock().unwrap().remove(name);
            Ok(())
        }
    }

    fn id_token(claims: &str) -> String {
        format!("eyJhbGciOiJub25lIn0.{}.", URL_SAFE_NO_PAD.encode(claims))
    }

    /// Part 8, 1: Microsoft's page, as the design asks it.
    #[test]
    fn a_sign_in_page_with_pkce_and_the_account_chooser() {
        for (authority, segment) in [(Authority::Consumers, "consumers"), (Authority::Organizations, "organizations"), (Authority::Common, "common"), (Authority::Tenant("contoso.example".into()), "contoso.example")] {
            let sign_in = begin("00000000-1111-2222-3333-444444444444", &authority, "you@example.org").unwrap();
            let port = sign_in.listener.local_addr().unwrap().port();
            assert!(
                sign_in.url.starts_with(&format!("https://login.microsoftonline.com/{segment}/oauth2/v2.0/authorize?client_id=00000000-1111-2222-3333-444444444444&response_type=code&redirect_uri=http%3A%2F%2F127.0.0.1%3A{port}&response_mode=query&scope=")),
                "{}",
                sign_in.url
            );
            assert!(sign_in.url.contains("&scope=https%3A%2F%2Foutlook.office.com%2FIMAP.AccessAsUser.All%20https%3A%2F%2Foutlook.office.com%2FSMTP.Send%20offline_access%20openid%20email%20profile&"), "{}", sign_in.url);
            assert!(sign_in.url.contains(&format!("&code_challenge={}&code_challenge_method=S256&", loopback::challenge(&sign_in.verifier))));
            assert!(sign_in.url.ends_with("&login_hint=you%40example.org&prompt=select_account"), "{}", sign_in.url);
            assert!(sign_in.verifier.len() >= 43, "RFC 7636: 43 characters at least");
            assert!(!sign_in.url.contains("secret"), "no secret, ever");
            assert!(sign_in.listener.local_addr().unwrap().ip().is_ipv4(), "no IPv6 loopback for Microsoft");
        }
        // A directory given by hand: an id or a domain, nothing that reaches another path.
        assert_eq!(Authority::tenant(" Contoso.Example ").unwrap(), Some(Authority::Tenant("contoso.example".into())));
        assert_eq!(Authority::tenant("").unwrap(), None);
        for bad in ["evil.example/oauth", "x?y", "a@b", "../common", "-x"] {
            assert!(Authority::tenant(bad).is_err(), "{bad}");
        }
    }

    /// Part 8, 2: the door. A favicon first; an answer of another sign-in
    /// refused while the wait goes on; Cancel; an answer read late; a refusal.
    #[test]
    fn microsoft_comes_back_here() {
        let sign_in = begin("id", &Authority::Common, "you@example.org").unwrap();
        let port = sign_in.listener.local_addr().unwrap().port();
        let state = sign_in.state.clone();
        let browser = std::thread::spawn(move || {
            let mut pages = Vec::new();
            for path in ["/favicon.ico".to_string(), "/?state=another&code=stolen".to_string(), "/?error=access_denied&state=another".to_string(), format!("/?code=M.C5_BAY.2.U.code&state={state}")] {
                let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes()).unwrap();
                let mut answer = String::new();
                let _ = stream.read_to_string(&mut answer);
                pages.push(answer);
            }
            pages
        });
        assert_eq!(wait_for_code(&sign_in, Duration::from_secs(10), &AtomicBool::new(false), None).unwrap(), "M.C5_BAY.2.U.code");
        let pages = browser.join().unwrap();
        assert!(pages[0].starts_with("HTTP/1.1 404"));
        assert!(pages[1].starts_with("HTTP/1.1 200") && pages[2].starts_with("HTTP/1.1 200"), "refused, said in the browser");
        // Cancel ends the wait; so does the time, an answer waiting being read first.
        let other = begin("id", &Authority::Common, "you@example.org").unwrap();
        assert_eq!(wait_for_code(&other, Duration::from_secs(10), &AtomicBool::new(true), None).err(), Some(SyncError::Said(CANCELLED, String::new())));
        let late = begin("id", &Authority::Common, "you@example.org").unwrap();
        let mut browser = std::net::TcpStream::connect(("127.0.0.1", late.listener.local_addr().unwrap().port())).unwrap();
        browser.write_all(format!("GET /?code=late&state={} HTTP/1.1\r\n\r\n", late.state).as_bytes()).unwrap();
        assert_eq!(wait_for_code(&late, Duration::ZERO, &AtomicBool::new(false), None).unwrap(), "late");
        let nothing = begin("id", &Authority::Common, "you@example.org").unwrap();
        assert_eq!(wait_for_code(&nothing, Duration::ZERO, &AtomicBool::new(false), None).err(), Some(SyncError::Said(TIMEOUT, String::new())));
        // Declined on Microsoft's page: said as such, a work account's with the approval it may have wanted.
        for (personal, said) in [(Some(true), DECLINED), (None, DECLINED_WORK)] {
            let declined = begin("id", &Authority::Common, "you@example.org").unwrap();
            let mut browser = std::net::TcpStream::connect(("127.0.0.1", declined.listener.local_addr().unwrap().port())).unwrap();
            let description = "AADSTS65004%3A+User+declined+to+consent+to+access+the+app.";
            browser.write_all(format!("GET /?error=access_denied&error_subcode=cancel&error_description={description}&state={} HTTP/1.1\r\n\r\n", declined.state).as_bytes()).unwrap();
            assert_eq!(wait_for_code(&declined, Duration::from_secs(10), &AtomicBool::new(false), personal).err(), Some(SyncError::Said(said, String::new())));
        }
    }

    /// A stand-in for Microsoft's token endpoint: each answer in turn; what
    /// was asked comes back.
    fn token_endpoint(answers: Vec<(&'static str, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut asked = Vec::new();
            for (status, answer) in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let (mut length, mut line, mut head) = (0, String::new(), String::new());
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    head.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                asked.push(format!("{head}{}", String::from_utf8(body).unwrap()));
                stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len()).as_bytes()).unwrap();
            }
            asked
        });
        (format!("http://127.0.0.1:{port}/consumers/oauth2/v2.0/token"), server)
    }

    /// Part 8, 3: the code traded for tokens, as a public client does: the
    /// verifier, the same redirect, the scopes, no secret. The ID token's
    /// tenant tells a personal account (Outlook.com's SMTP) from a work one.
    #[test]
    fn the_code_brings_a_grant_of_the_right_kind() {
        let store = Memory::default();
        for (tid, personal, tenant, smtp) in [(CONSUMERS_TID, true, "consumers", SMTP_PERSONAL), ("0a1b2c3d-0000-4000-8000-00000000c0de", false, "0a1b2c3d-0000-4000-8000-00000000c0de", SMTP_WORK)] {
            let claims = format!(r#"{{"tid":"{tid}","preferred_username":"You@Example.org","email":"you@example.org"}}"#);
            let answer = format!(r#"{{"token_type":"Bearer","scope":"https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send","expires_in":4469,"access_token":"EwBIA.access","refresh_token":"M.C5_BAY.0.U.refresh","id_token":"{}"}}"#, id_token(&claims));
            let (endpoint, server) = token_endpoint(vec![("200 OK", answer)]);
            let sign_in = begin("app-id", &Authority::Common, "you@example.org").unwrap();
            let (port, state) = (sign_in.listener.local_addr().unwrap().port(), sign_in.state.clone());
            let (verifier, redirect) = (sign_in.verifier.clone(), sign_in.redirect.clone());
            let browser = std::thread::spawn(move || {
                let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                stream.write_all(format!("GET /?code=the-code&state={state} HTTP/1.1\r\n\r\n").as_bytes()).unwrap();
                let mut answer = String::new();
                let _ = stream.read_to_string(&mut answer);
            });
            let grant = finish_at(sign_in, "app-id", "You@Example.org", &AtomicBool::new(false), Some(&endpoint), &store).unwrap();
            browser.join().unwrap();
            let asked = server.join().unwrap();
            let body = asked[0].rsplit("\r\n\r\n").next().unwrap();
            assert_eq!(body, loopback::query(&[("client_id", "app-id"), ("scope", MAIL_SCOPES), ("code", "the-code"), ("redirect_uri", &redirect), ("grant_type", "authorization_code"), ("code_verifier", &verifier)]));
            assert!(!asked[0].contains("client_secret") && !asked[0].to_ascii_lowercase().contains("\norigin:"), "a public client: no secret, no Origin");
            assert_eq!((grant.personal, grant.tenant.as_str(), grant.signed_in_as.as_str()), (personal, tenant, "you@example.org"));
            assert_eq!(if grant.personal { Kind::Personal.smtp_host() } else { Kind::Work.smtp_host() }, smtp);
            // Kept under the address, lowercased; the access token in memory.
            let (kept, _) = load_from(&store, "microsoft-mail:you@example.org").unwrap().unwrap();
            assert_eq!(kept, grant);
            assert_eq!(remembered("microsoft-mail:you@example.org").as_deref(), Some("EwBIA.access"));
            assert!(!format!("{grant:?}").contains("M.C5_BAY"), "Debug never prints the token");
        }
        // Reading without sending, or the reverse, is not mail.
        assert!(!gave_mail(&["https://outlook.office.com/IMAP.AccessAsUser.All".into(), "offline_access".into()]));
        assert!(gave_mail(&["IMAP.AccessAsUser.All".into(), "SMTP.Send".into()]), "as some answers write them");
        assert!(gave_mail(&[]), "no scope said: what was asked");
    }

    /// Part 8, 4: each refresh brings a new refresh token, kept in place of
    /// the old one and sent the next time; an answer without one keeps it.
    #[test]
    fn refresh_tokens_rotate() {
        let store = Memory::default();
        let grant = Grant { client_id: "app-id".into(), tenant: "consumers".into(), personal: true, refresh_token: "RT1".into(), scopes: vec![], signed_in: 1, signed_in_as: "you@example.org".into() };
        save_in(&store, "microsoft-mail:you@example.org", &grant, 0).unwrap();
        let (endpoint, server) = token_endpoint(vec![
            ("200 OK", r#"{"access_token":"AT1","expires_in":3600,"refresh_token":"RT2"}"#.into()),
            ("200 OK", r#"{"access_token":"AT2","expires_in":3600}"#.into()),
        ]);
        for expected in ["AT1", "AT2"] {
            let (kept, parts) = load_from(&store, "microsoft-mail:you@example.org").unwrap().unwrap();
            assert_eq!(refreshed(&endpoint, &kept, &store, "microsoft-mail:you@example.org", parts).unwrap(), (expected.to_string(), 3600));
        }
        let asked = server.join().unwrap();
        assert!(asked[0].ends_with(&loopback::query(&[("client_id", "app-id"), ("grant_type", "refresh_token"), ("refresh_token", "RT1"), ("scope", REFRESH_SCOPES)])), "{}", asked[0]);
        assert!(asked[1].contains("refresh_token=RT2"), "the newest sent: {}", asked[1]);
        let (kept, _) = load_from(&store, "microsoft-mail:you@example.org").unwrap().unwrap();
        assert_eq!((kept.refresh_token.as_str(), kept.signed_in), ("RT2", 1), "the newest kept, the sign-in's time with it");
    }

    /// Part 8, 5: Microsoft's refusals, each said by its sentence.
    #[test]
    fn refusals_are_said_for_what_they_are() {
        let said = |error: &str, code: u64, personal: Option<bool>, step: Step| refusal(400, error, &[code], "", personal, step);
        let login = |id: &str| SyncError::Login(id.into());
        assert_eq!(said("invalid_grant", 50173, Some(true), Step::Refresh), login(AGAIN_PASSWORD));
        assert_eq!(said("invalid_grant", 700082, Some(true), Step::Refresh), login(AGAIN_UNUSED));
        assert_eq!(said("invalid_grant", 70008, Some(false), Step::Refresh), login(AGAIN_UNUSED));
        assert_eq!(said("invalid_grant", 70008, None, Step::Redeem), login(AGAIN_LATE), "a code a minute old");
        assert_eq!(said("invalid_grant", 54005, None, Step::Redeem), login(AGAIN_LATE));
        assert_eq!(said("invalid_grant", 70043, Some(false), Step::Refresh), login(AGAIN_FREQUENCY));
        assert_eq!(said("invalid_grant", 50076, Some(false), Step::Refresh), login(AGAIN));
        assert_eq!(said("invalid_grant", 90094, Some(false), Step::Redeem), login(ADMIN));
        assert_eq!(said("invalid_grant", 65001, Some(false), Step::Refresh), login(ADMIN));
        assert_eq!(said("invalid_grant", 65001, Some(true), Step::Refresh), login(AGAIN), "your own consent, withdrawn: give it again");
        assert_eq!(said("invalid_request", 53003, Some(false), Step::Refresh), login(BLOCKED));
        assert_eq!(said("invalid_request", 530035, Some(false), Step::Redeem), login(BLOCKED));
        assert_eq!(said("unauthorized_client", 700016, None, Step::Redeem), login(UNKNOWN_APP));
        assert_eq!(said("invalid_request", 50011, None, Step::Page), login(REDIRECT));
        assert_eq!(said("invalid_client", 7000218, None, Step::Redeem), login(CONFIDENTIAL));
        assert_eq!(said("invalid_request", 700025, None, Step::Redeem), login(SECRET_SENT));
        assert_eq!(said("invalid_request", 50194, None, Step::Page), login(ACCOUNT_TYPES));
        assert_eq!(said("access_denied", 65004, Some(true), Step::Page), SyncError::Said(DECLINED, String::new()));
        // The code read from the description when no list comes.
        assert_eq!(refusal(400, "invalid_grant", &[], "AADSTS700082: The refresh token has expired due to inactivity.", Some(true), Step::Refresh), login(AGAIN_UNUSED));
        assert_eq!(refusal(400, "invalid_grant", &[], "", None, Step::Redeem), login(AGAIN_LATE));
        assert_eq!(refusal(503, "temporarily_unavailable", &[], "AADSTS90033: busy", None, Step::Refresh), SyncError::Server("Microsoft: 503 temporarily_unavailable AADSTS90033".into()));
        assert_eq!(refusal(0, "invalid_request", &[], "AADSTS90014: missing", None, Step::Page), SyncError::Server("Microsoft: invalid_request AADSTS90014".into()), "no status from Microsoft's page");
        // Each has its sentence, in both languages.
        for (id, expected) in [(AGAIN, "sign in"), (ADMIN, "administrator"), (BLOCKED, "rules"), (UNKNOWN_APP, "application"), (PASSWORD, "Sign in with Microsoft")] {
            let en = SyncError::Login(id.into()).sentence(&sioul_core::i18n::Translator::new("en"), "work");
            let fr = SyncError::Login(id.into()).sentence(&sioul_core::i18n::Translator::new("fr"), "travail");
            assert!(en.starts_with("work: ") && en.contains(expected) && !en.contains("microsoft-"), "{en}");
            assert!(fr.starts_with("travail") && !fr.contains("microsoft-") && fr != en, "{fr}");
        }
        for id in [AGAIN, AGAIN_PASSWORD, AGAIN_UNUSED, AGAIN_FREQUENCY, AGAIN_LATE, ADMIN, BLOCKED, UNKNOWN_APP, REDIRECT, CONFIDENTIAL, SECRET_SENT, ACCOUNT_TYPES, SCOPES_MISSING, PASSWORD] {
            assert!(message_of(id).is_some(), "{id}");
            assert!(SyncError::Login(id.into()).is_lasting(), "{id}: no retry hammers Microsoft");
        }
    }

    /// Part 8, 6: a 4 KB refresh token (longer than any seen) kept in parts,
    /// each within what Windows keeps (the stand-in refuses more), joined
    /// again whole; a shorter one after it leaves no part behind; parts that
    /// do not make the token (a write cut short) give no grant.
    #[test]
    fn a_long_refresh_token_is_kept_in_parts() {
        let store = Memory::default();
        let name = "microsoft-mail:you@example.org";
        let long: String = (0..4096).map(|i| char::from(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!*$"[i % 68])).collect();
        let grant = Grant { client_id: "app-id".into(), tenant: "0a1b2c3d-0000-4000-8000-00000000c0de".into(), personal: false, refresh_token: long.clone(), scopes: vec![IMAP_SCOPE.into(), SMTP_SCOPE.into()], signed_in: 1_760_000_000, signed_in_as: "you@example.org".into() };
        save_in(&store, name, &grant, 0).unwrap();
        let (kept, parts) = load_from(&store, name).unwrap().unwrap();
        assert_eq!((kept.refresh_token.len(), parts), (4096, 1), "the first entry and one part");
        assert_eq!(kept, grant);
        // Twice as long: two parts; a token longer than Sioul keeps is refused, not cut.
        let double = Grant { refresh_token: long.repeat(2), ..grant.clone() };
        save_in(&store, name, &double, parts).unwrap();
        assert_eq!(load_from(&store, name).unwrap().unwrap(), (double, 3));
        assert!(save_in(&store, name, &Grant { refresh_token: long.repeat(3), ..grant.clone() }, 3).is_err());
        let parts = 3;
        // Shorter: one entry, the parts gone.
        let short = Grant { refresh_token: "M.C5_BAY.short".into(), ..grant.clone() };
        save_in(&store, name, &short, parts).unwrap();
        assert_eq!(load_from(&store, name).unwrap().unwrap(), (short, 0));
        assert_eq!(store.0.lock().unwrap().len(), 1, "no part left behind");
        // A part from another write: no grant, rather than a token that is not one.
        save_in(&store, name, &grant, 0).unwrap();
        store.set(&format!("{name}#2"), b"from another write").unwrap();
        assert_eq!(load_from(&store, name).unwrap(), None);
        // A part missing: the same.
        save_in(&store, name, &grant, 1).unwrap();
        store.forget(&format!("{name}#2")).unwrap();
        assert_eq!(load_from(&store, name).unwrap(), None);
    }

    /// Part 8, 6, in the system's own keyring: Windows' Credential Manager
    /// keeps 2560 bytes an entry, which a Microsoft grant passes. Run in the
    /// Windows job of the CI (build.yml tests sioul-sync), under a name of
    /// its own, removed after; never on another system, whose keyring is the
    /// person's own (the round trip above covers the splitting everywhere).
    #[cfg(windows)]
    #[test]
    fn windows_keeps_a_4_kb_token() {
        let login = format!("windows-test-{}@example.org", uuid::Uuid::new_v4());
        let long: String = (0..4096).map(|i| char::from(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!*$"[i % 68])).collect();
        let grant = Grant { client_id: "app-id".into(), tenant: "consumers".into(), personal: true, refresh_token: long, scopes: vec![], signed_in: 1, signed_in_as: login.clone() };
        // As one entry it does not fit: refused at Windows' limit (`CRED_MAX_CREDENTIAL_BLOB_SIZE`).
        let whole = keyring::Entry::new("sioul", &keyring_name(&login)).unwrap();
        assert!(whole.set_secret(&[b'x'; 2600]).is_err(), "Windows keeps at most 2560 bytes an entry");
        keep(&login, &grant).unwrap();
        let back = grant(&login);
        forget(&login, true).unwrap();
        assert_eq!(back, Some(grant));
        assert_eq!(grant(&login), None, "forgotten, parts and all");
        assert!(crate::secret::named_bytes(&format!("{}#2", keyring_name(&login))).unwrap().is_none());
    }

    /// Part 8, 8 and 9: Microsoft's refusals that no token changes.
    #[test]
    fn closed_mailboxes_and_switched_off_sending_are_said() {
        assert_eq!(not_connected(true, "User is authenticated but not connected."), Some(SyncError::Said(NOT_CONNECTED_PERSONAL, "User is authenticated but not connected.".into())));
        assert_eq!(not_connected(false, "user is authenticated but not connected"), Some(SyncError::Said(NOT_CONNECTED_WORK, "user is authenticated but not connected".into())));
        assert_eq!(not_connected(true, "AUTHENTICATE failed."), None);
        let off = "535 5.7.139 Authentication unsuccessful, SmtpClientAuthentication is disabled for the Tenant. Visit https://aka.ms/smtp_auth_disabled for more information.";
        assert_eq!(smtp_refused(off), Some(SyncError::Said(SMTP_OFF, off.into())));
        assert_eq!(smtp_refused("535 5.7.3 Authentication unsuccessful"), None);
        // Said in both languages, with what to do.
        let en = sioul_core::i18n::Translator::new("en");
        assert!(SyncError::Said(SMTP_OFF, off.into()).sentence(&en, "work").contains("administrator"));
        assert!(SyncError::Said(NOT_CONNECTED_PERSONAL, String::new()).sentence(&en, "home").contains("IMAP"));
        // Only Microsoft's servers, named exactly, are given a token.
        assert!(is_imap_host("outlook.office365.com") && is_imap_host("Outlook.Office365.com"));
        for host in ["outlook.office365.com.evil.example", "evil-outlook.office365.com", "imap.example.org", "outlook.office365.com."] {
            assert!(!is_imap_host(host), "{host}");
        }
        assert!(is_smtp_host("smtp-mail.outlook.com") && is_smtp_host("smtp.office365.com") && !is_smtp_host("smtp.office365.com.evil.example"));
        // The administrator's link: the organisation's directory, never `common`.
        let url = admin_consent_url("app-id", "contoso.example");
        assert!(url.starts_with("https://login.microsoftonline.com/contoso.example/v2.0/adminconsent?client_id=app-id&scope=https%3A%2F%2Foutlook.office.com%2FIMAP.AccessAsUser.All%20"), "{url}");
        assert!(admin_consent_url("app-id", "x/../common").starts_with("https://login.microsoftonline.com/organizations/"));
    }
}

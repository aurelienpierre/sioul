// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What this device keeps of its extensions, never shared
//! (`$XDG_STATE_HOME/sioul/extensions.toml`): the last manifest's serial it
//! accepted, when it last read the manifest, and for each extension it
//! enabled the version in use (as accepted, with what it is given), an
//! update waiting for the person's yes, whether it holds a sign-in, when it
//! last ran and what it brought, and its last problem. The window and the
//! phone's background step both change it, one at a time (`change`).

use super::manifest::Offer;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// This device's extensions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// The highest serial of a manifest accepted here: a lower one is refused.
    #[serde(default)]
    pub serial: u64,
    /// The SHA-256 of the manifest accepted under `serial`: another one under
    /// the same serial is refused. Empty before the first.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manifest: String,
    /// When the manifest was last read (seconds since 1970).
    #[serde(default)]
    pub checked: i64,
    #[serde(default, rename = "extension")]
    pub extensions: Vec<Installed>,
}

/// An agenda an extension brought, as last seen here: every agenda it ever
/// named is kept (a pupil gone stays listed, with where its lessons went).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brought {
    pub id: String,
    pub name: String,
    /// The lessons its last answer held.
    #[serde(default)]
    pub events: usize,
    /// The calendar its lessons go into, by its place ("account/id"), as
    /// this device last wrote them; the shared setting
    /// (`[extensions.calendars]`) says it for every device. "" none yet.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub calendar: String,
    /// When its lessons were last read and written here (seconds since 1970); 0 never.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub read: i64,
    /// Why its lessons were not written at the last refresh: one word
    /// (`door::Problem::word`, "choose" when no calendar is chosen); none when they were.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// The extension's last suggestion that these are the person's own busy
    /// time (`AgendaReply::busy`): the default for the person's per-agenda
    /// choice, kept so the settings page shows it between refreshes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub busy: bool,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// A set of contacts an extension brought, as last seen here: every set it
/// ever named is kept, with where its contacts went.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BroughtSet {
    pub id: String,
    pub name: String,
    /// The people its last answer held.
    #[serde(default)]
    pub people: usize,
    /// The address book its contacts go into, by its place ("account/id"),
    /// as this device last wrote them; the shared setting
    /// (`[extensions.addressbooks]`) says it for every device. "" none yet.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub book: String,
    /// When its contacts were last read and written here (seconds since 1970); 0 never.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub read: i64,
    /// Why its contacts were not written at the last refresh: one word
    /// (`cards::Problem::word`, "choose" when no address book is chosen,
    /// "refused" when some card was not); none when they were.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// Its people whose cards were recognised as those of people who reach
    /// the person in a special way, and left as they are but for the mark
    /// (`cards::Done::recognised`), at the last refresh that wrote here.
    #[serde(default, skip_serializing_if = "is_zero_count")]
    pub recognised: usize,
}

fn is_zero_count(n: &usize) -> bool {
    *n == 0
}

/// The most agendas, and the most sets of contacts, the record keeps of one extension.
pub const MOST_KEPT: usize = 64;

/// An extension this device enabled once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    /// The version in use, as accepted: its hosts and sign-in are what it is given.
    pub offer: Offer,
    #[serde(default)]
    pub enabled: bool,
    /// A newer version that asks for more, waiting for the person's yes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<Offer>,
    /// Whether a sign-in is kept in this device's keyring.
    #[serde(default)]
    pub signed_in: bool,
    /// When it last ran, and last brought something (seconds since 1970).
    #[serde(default)]
    pub last_run: i64,
    #[serde(default)]
    pub last_brought: i64,
    #[serde(default)]
    pub agendas: Vec<Brought>,
    /// The sets of contacts it brought (`MOST_KEPT` at most).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<BroughtSet>,
    /// Its last problem, one word Sioul says in a sentence ("sign-in",
    /// "unreachable", "refused", "changed", "stopped-time", …); none when the last run went well.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// That problem's own detail, shown under its sentence: the extension's
    /// words (its problem's `detail`, each value the call hid replaced), or
    /// Sioul's when the call stopped; in English as written, `scrub`bed and
    /// cut to `DETAIL_WIDTH` characters. Empty when none. A Sioul 0.1.0
    /// reading this record passes over the key and drops it at its next write
    /// (the record is this device's alone, never shared).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// Removed here: no call of it runs, a call under way ends at its next
    /// ask, and its sign-in is still to forget while the keyring refuses.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leaving: bool,
}

impl Installed {
    pub fn new(offer: Offer) -> Self {
        Self { offer, enabled: true, waiting: None, signed_in: false, last_run: 0, last_brought: 0, agendas: Vec::new(), contacts: Vec::new(), problem: None, detail: String::new(), leaving: false }
    }

    /// Its last problem noted, with its detail as the card shows it
    /// (`detail_shown`); none clears both.
    pub fn note_problem(&mut self, problem: Option<String>, detail: &str) {
        self.detail = if problem.is_some() { detail_shown(detail) } else { String::new() };
        self.problem = problem;
    }
}

/// The most characters a problem's detail keeps (`Installed::detail`).
pub const DETAIL_WIDTH: usize = 200;

/// A problem's detail as the record keeps it and the card shows it: on one
/// line, `scrub`bed, cut to `DETAIL_WIDTH` characters.
pub fn detail_shown(detail: &str) -> String {
    let plain: String = detail.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let plain = scrub(plain.trim());
    let mut cut: String = plain.chars().take(DETAIL_WIDTH).collect();
    if cut.len() < plain.len() {
        cut.push('…');
    }
    cut
}

impl State {
    pub fn get(&self, id: &str) -> Option<&Installed> {
        self.extensions.iter().find(|e| e.offer.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Installed> {
        self.extensions.iter_mut().find(|e| e.offer.id == id)
    }

    /// A manifest taken, by its serial and the SHA-256 of its bytes: refused
    /// when its serial is lower than the last accepted here or than the
    /// lowest this release of Sioul accepts (`LOWEST_SERIAL`, so that a
    /// fresh device, or one whose record was lost, cannot be served an older
    /// signed manifest either), and when it is another manifest under the
    /// serial accepted last (two manifests signed with one number cannot be
    /// alternated). The same manifest read again is taken.
    pub fn accept(&mut self, serial: u64, sha256: &str) -> Result<(), String> {
        let lowest = self.serial.max(super::LOWEST_SERIAL);
        if serial < lowest {
            return Err(format!("an older manifest (serial {serial}, {lowest} at least)"));
        }
        if serial == self.serial && !self.manifest.is_empty() && self.manifest != sha256 {
            return Err(format!("another manifest under serial {serial}"));
        }
        self.serial = serial;
        self.manifest = sha256.to_string();
        Ok(())
    }
}

/// Where this device keeps it.
pub fn path() -> PathBuf {
    crate::config::state_dir().join("extensions.toml")
}

/// Where an extension's file is kept, by its hash: Sioul's private data.
pub fn file_path(offer: &Offer) -> PathBuf {
    crate::config::data_dir().join("extensions").join(format!("{}-{}.wasm", offer.id, &offer.sha256[..offer.sha256.len().min(16)]))
}

/// Where an extension's log is kept, on this device only.
pub fn log_path(id: &str) -> PathBuf {
    crate::config::state_dir().join("extensions").join(format!("{id}.log"))
}

/// The state at `path`; an absent or unreadable one is empty. Each offer it
/// keeps is checked as a manifest's are (its id above all, which names a
/// file, a log and a keyring entry): one that fails is left out, as if never
/// installed (M2: nothing signs this record).
pub fn load(path: &Path) -> State {
    let mut state: State = crate::files::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default();
    state.extensions.retain(|i| super::manifest::check(&i.offer).is_ok());
    for installed in &mut state.extensions {
        if installed.waiting.as_ref().is_some_and(|w| super::manifest::check(w).is_err() || w.id != installed.offer.id) {
            installed.waiting = None;
        }
    }
    state
}

/// The state at `path` changed by `f`, read and written under its lock (the
/// window and the phone's background process both change it); what `f` returns.
pub fn change<T>(path: &Path, f: impl FnOnce(&mut State) -> T) -> Result<T, String> {
    crate::filelock::with_lock(path, || {
        let mut state = load(path);
        let out = f(&mut state);
        let text = toml::to_string(&state).map_err(|e| e.to_string())?;
        if let Some(folder) = path.parent() {
            crate::files::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(out)
    })
}

/// The most lines an extension's log keeps, and the most bytes.
pub const LOG_LINES: usize = 400;
pub const LOG_BYTES: usize = 128 << 10;
/// The most characters a line of the log holds (the engine cuts its own the same).
pub const LOG_WIDTH: usize = 300;

/// A line as the log keeps it: no control character, `scrub`bed, `LOG_WIDTH` characters at most.
fn log_line(line: &str) -> String {
    let plain: String = line.chars().filter(|c| !c.is_control()).collect();
    let plain = scrub(&plain);
    let mut cut: String = plain.chars().take(LOG_WIDTH).collect();
    if cut.len() < plain.len() {
        cut.push('…');
    }
    cut
}

/// What Sioul takes out of every line of an extension's log and of a
/// problem's detail, whoever wrote it (the extension, its service's answer it
/// repeats, Sioul), each replaced by `[hidden]`: an address's query and
/// fragment (`?…`, `#…=`) and its user; a header's value that signs in
/// (`Authorization`, `Cookie`, `Set-Cookie`, an API key's); a `Bearer` or
/// `Basic` token; a JSON Web Token; the value of a pair named as a password,
/// a token, a secret, a key, a code, a session, a cookie or a signature, in
/// each form a line may write it (`password=…`, `password = '…'`,
/// `"refresh_token": "…"`, the same JSON escaped inside a string, a header
/// as a `["name", "value"]` pair); and any run of 32 letters, digits,
/// `-` or `_` holding both a letter and a digit (a token, a key, a long id).
/// The engine hides first the values a call named (`hide`, the secret, the
/// credentials): this is the net under it, for what an extension never
/// named. Idempotent: a text scrubbed reads the same scrubbed again.
pub fn scrub(text: &str) -> String {
    use regex::{Captures, Regex};
    use std::sync::LazyLock;
    // A query or a fragment of names and values, after an address or a path alone.
    static QUERY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"([?#])[A-Za-z0-9_.%\[\]-]+=[^\s"'<>]*"#).expect("a pattern"));
    // An address's user (and password) before its host.
    static USER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\b([a-z][a-z0-9+.-]*://)[^\s/?#@"'<>]+@"#).expect("a pattern"));
    // A header that signs in, as a line, a JSON pair (escaped too) or a `["name", "value"]`
    // pair as Sioul hands headers over: its whole value.
    static HEADER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)\b(authorization|proxy-authorization|cookie|set-cookie|x-api-key|x-goog-api-key|x-auth-token|x-csrf-token)(\\?"\s*,\s*|\\?"?\s*[:=]\s*)(\[hidden\]|\\"(?:[^"\\]|\\[^"])*\\"|"(?:[^"\\]|\\.)*"|'[^']*'|[^\r\n"]+)"#).expect("a pattern")
    });
    static BEARER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(bearer|basic)\s+[A-Za-z0-9._~+/=-]{6,}").expect("a pattern"));
    static JWT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"eyJ[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{4,}(?:\.[A-Za-z0-9_-]*)?").expect("a pattern"));
    // `name=value`, as in a form, a query or a cookie, and `name = 'value'` as in a
    // configuration or a program: the value bare, or quoted whole.
    static ASSIGNED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)(^|[^A-Za-z0-9_])(password|passwd|pwd|pass|secret|client_secret|token|access_token|refresh_token|id_token|idtoken|refreshtoken|accesstoken|api_key|apikey|key|code|code_verifier|session|sessionid|sid|sig|signature|auth|credentials?|otp|pin|cookie|assertion|state|nonce)(\s*=\s*)('[^']*'|\\"(?:[^"\\]|\\[^"])*\\"|"(?:[^"\\]|\\.)*"|[^\s&"',;}]*)"#).expect("a pattern")
    });
    // `"name": "value"` or `name: value`, as in JSON or a header; never a word that also names a thing (`key`, `code`, `state`).
    static SAID: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)(^|[^A-Za-z0-9_])(\\?"?)(password|passwd|pwd|secret|client_secret|token|access_token|refresh_token|id_token|idtoken|refreshtoken|accesstoken|api_key|apikey|cookie|session|otp|credentials?|code_verifier|assertion)(\\?"?\s*:\s*)(\\"(?:[^"\\]|\\[^"])*\\"|"(?:[^"\\]|\\.)*"|'[^']*'|[^\s,;}]+)"#).expect("a pattern")
    });
    static OPAQUE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9_-]{32,}").expect("a pattern"));
    const HIDDEN: &str = "[hidden]";
    let text = QUERY.replace_all(text, |c: &Captures| format!("{}{HIDDEN}", &c[1]));
    let text = USER.replace_all(&text, |c: &Captures| format!("{}{HIDDEN}@", &c[1]));
    let text = HEADER.replace_all(&text, |c: &Captures| format!("{}{}{HIDDEN}", &c[1], &c[2]));
    let text = BEARER.replace_all(&text, |c: &Captures| format!("{} {HIDDEN}", &c[1]));
    let text = JWT.replace_all(&text, HIDDEN);
    let text = ASSIGNED.replace_all(&text, |c: &Captures| format!("{}{}{}{HIDDEN}", &c[1], &c[2], &c[3]));
    let text = SAID.replace_all(&text, |c: &Captures| format!("{}{}{}{}{HIDDEN}", &c[1], &c[2], &c[3], &c[4]));
    let text = OPAQUE.replace_all(&text, |c: &Captures| {
        let run = &c[0];
        if run.bytes().any(|b| b.is_ascii_digit()) && run.bytes().any(|b| b.is_ascii_alphabetic()) { HIDDEN.to_string() } else { run.to_string() }
    });
    text.into_owned()
}

/// The last `most` lines of an extension's log at `path`, newest first,
/// each read as the log keeps it (`log_line`: a log an older Sioul wrote is
/// `scrub`bed here too); none when it cannot be read.
pub fn log_tail(path: &Path, most: usize) -> Vec<String> {
    let text = crate::files::read_to_string(path).unwrap_or_default();
    text.lines().rev().filter(|l| !l.trim().is_empty()).take(most).map(log_line).collect()
}

/// Lines added to an extension's log at `path`, its oldest lines dropped
/// beyond `LOG_LINES` and `LOG_BYTES`. The engine's lines come hidden and
/// cut; every line is cut here again, whoever wrote it (X6), and the window's
/// own lines name nothing an extension said.
pub fn append_log(path: &Path, lines: &[String]) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }
    crate::filelock::with_lock(path, || {
        let before = crate::files::read_to_string(path).unwrap_or_default();
        let added: Vec<String> = lines.iter().map(|l| log_line(l)).collect();
        let mut all: Vec<&str> = before.lines().collect();
        all.extend(added.iter().map(String::as_str));
        let mut keep = all.len().saturating_sub(LOG_LINES);
        let mut bytes: usize = all[keep..].iter().map(|l| l.len() + 1).sum();
        while bytes > LOG_BYTES && keep < all.len() {
            bytes -= all[keep].len() + 1;
            keep += 1;
        }
        let text = all[keep..].join("\n") + "\n";
        if let Some(folder) = path.parent() {
            crate::files::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_keeps_its_serial_and_its_extensions() {
        let folder = std::env::temp_dir().join(format!("sioul-extensions-state-{}", std::process::id()));
        let path = folder.join("extensions.toml");
        let offer = crate::extensions::manifest::parse(crate::extensions::manifest::SAMPLE).unwrap().offers.remove(0);
        let clients = BroughtSet { id: "clients".into(), name: "Clients".into(), people: 128, book: "cloud/clients".into(), read: 1_795_000_000, problem: None, recognised: 3 };
        change(&path, |state| {
            state.accept(7, "a7").unwrap();
            let mut installed = Installed::new(offer.clone());
            installed.contacts.push(clients.clone());
            state.extensions.push(installed);
        })
        .unwrap();
        let mut state = load(&path);
        assert_eq!((state.serial, state.manifest.as_str()), (7, "a7"));
        assert!(state.get("timetable").unwrap().enabled);
        assert_eq!(state.get("timetable").unwrap().contacts, vec![clients], "a set of contacts kept as brought");
        assert!(state.accept(6, "a6").is_err(), "an older manifest");
        assert!(state.accept(7, "a7").is_ok(), "the same one again");
        assert!(state.accept(7, "b7").is_err(), "another manifest under the same serial");
        assert!(state.accept(8, "a8").is_ok());
        let log = folder.join("timetable.log");
        for round in 0..3 {
            append_log(&log, &(0..200).map(|i| format!("{round}-{i}")).collect::<Vec<_>>()).unwrap();
        }
        let kept = std::fs::read_to_string(&log).unwrap();
        assert_eq!(kept.lines().count(), LOG_LINES);
        assert_eq!(kept.lines().last(), Some("2-199"));
        assert_eq!(kept.lines().next(), Some("1-0"));
        // X6: every line cut, whoever wrote it; the log bounded in bytes too.
        append_log(&log, &[format!("long {}\u{7}", "x".repeat(100_000))]).unwrap();
        let last = std::fs::read_to_string(&log).unwrap().lines().last().unwrap().to_string();
        assert!(last.chars().count() <= LOG_WIDTH + 1 && !last.contains('\u{7}'), "{}", last.len());
        for _ in 0..3 {
            append_log(&log, &(0..400).map(|i| format!("{i} {}", "y".repeat(290))).collect::<Vec<_>>()).unwrap();
        }
        assert!(std::fs::metadata(&log).unwrap().len() as usize <= LOG_BYTES);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// What never stands in a log or a detail is taken out whoever wrote it,
    /// with invented secrets of every kind; what diagnoses a problem stays
    /// (a host, a path, a status, a service's error code); scrubbed twice reads the same.
    #[test]
    fn a_log_line_never_holds_a_secret_whoever_wrote_it() {
        let secrets = [
            ("GET https://calendar.example.org/feed?calendarId=cal-4821&businessId=biz-7730&key=k7f3a9e2b1c done", &["cal-4821", "biz-7730", "k7f3a9e2b1c"][..], "GET https://calendar.example.org/feed?[hidden] done"),
            ("asked /getCalendarIcalEvents?calendarId=-Cinvented&key=KEYinvented0000", &["KEYinvented0000", "-Cinvented"], "asked /getCalendarIcalEvents?[hidden]"),
            ("returned to example-app://cb#access_token=AT-fragment-5678&state=s", &["AT-fragment-5678"], "returned to example-app://cb#[hidden]"),
            ("https://alex:hunter2@example.org/x", &["alex", "hunter2"], "https://[hidden]@example.org/x"),
            ("Authorization: Bearer AT-invented-123456", &["AT-invented-123456"], "Authorization: [hidden]"),
            (r#"headers {"authorization":"Bearer abcdef123456","accept":"text/plain"}"#, &["abcdef123456"], r#"headers {"authorization":[hidden],"accept":"text/plain"}"#),
            ("Cookie: sid=abc123; theme=dark", &["abc123", "dark"], "Cookie: [hidden]"),
            ("Set-Cookie: SESSION=xyz789; Path=/", &["xyz789"], "Set-Cookie: [hidden]"),
            ("sent bearer RT-plain-0042 to it", &["RT-plain-0042"], "sent bearer [hidden] to it"),
            (r#"{"email":"alex@example.org","password":"hunter2-secret","returnSecureToken":true}"#, &["hunter2-secret"], r#"{"email":"alex@example.org","password":[hidden],"returnSecureToken":true}"#),
            ("grant_type=refresh_token&refresh_token=RT-invented-9999", &["RT-invented-9999"], "grant_type=refresh_token&refresh_token=[hidden]"),
            ("password: hunter2 and token: tk-1234", &["hunter2", "tk-1234"], "password: [hidden] and token: [hidden]"),
            ("id eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJpbnZlbnRlZCJ9.c2lnbmF0dXJl end", &["eyJhbGci", "c2lnbmF0dXJl"], "id [hidden] end"),
            ("kept AMf-vBxInvented0Refresh0Token0Value000000000000 here", &["AMf-vBx"], "kept [hidden] here"),
            ("x-goog-api-key: AIzaInvented00000000000000000000000000", &["AIzaInvented"], "x-goog-api-key: [hidden]"),
            ("form code=OC-1234&code_verifier=v3r1f13r", &["OC-1234", "v3r1f13r"], "form code=[hidden]&code_verifier=[hidden]"),
            // Spaces around `=`, and a quoted value, as a configuration or a program writes them.
            ("password = hunter2 and token = tk-1234", &["hunter2", "tk-1234"], "password = [hidden] and token = [hidden]"),
            ("password='hunter2'", &["hunter2"], "password=[hidden]"),
            (r#"password="hunter 2" then"#, &["hunter"], "password=[hidden] then"),
            // JSON escaped inside a string (a body's Debug, JSON within JSON).
            (r#"body "{\"password\":\"hunter2\",\"email\":\"a@example.org\"}""#, &["hunter2"], r#"body "{\"password\":[hidden],\"email\":\"a@example.org\"}""#),
            (r#"{\"authorization\":\"Bearer abcdef123456\"}"#, &["abcdef123456", "Bearer"], r#"{\"authorization\":[hidden]}"#),
            // The headers as Sioul hands them over, `["name", "value"]`.
            (r#"[["set-cookie","__Host-x=abc123"],["x-goog-api-key","AIzaShort1"],["accept","text/plain"]]"#, &["abc123", "AIzaShort1"], r#"[["set-cookie",[hidden]],["x-goog-api-key",[hidden]],["accept","text/plain"]]"#),
            (r#"[["Cookie", "sid=xyz789"]]"#, &["xyz789"], r#"[["Cookie", [hidden]]]"#),
        ];
        for (line, gone, said) in secrets {
            let scrubbed = scrub(line);
            for secret in gone {
                assert!(!scrubbed.contains(secret), "{secret} in {scrubbed}");
            }
            assert_eq!(scrubbed, said, "{line}");
            assert_eq!(scrub(&scrubbed), scrubbed, "scrubbed twice: {line}");
        }
        for kept in [
            "sign-in: the password sign-in (identitytoolkit) answered HTTP 400 with INVALID_LOGIN_CREDENTIALS",
            "POST identitytoolkit.googleapis.com/v1/accounts:signInWithPassword answered 400 in 412 ms (231 bytes)",
            "POST securetoken.googleapis.com/v1/token answered 400 in 312 ms (120 bytes)",
            "POST firestore.googleapis.com/v1/projects/planity-production/databases/(default)/documents/businesses/[hidden]:runQuery answered 200 in 830 ms (5120 bytes)",
            "the token's renewal (securetoken) answered HTTP 400 with TOKEN_EXPIRED",
            "TOO_MANY_ATTEMPTS_TRY_LATER, then USER_DISABLED",
            "the web address has no key; the email field was empty",
            "keyring: no entry kept for it on this device",
            "keyring: reading its entry failed (Platform secure storage failure: Java exception was thrown)",
            "refresh: 12 appointment(s) from Firestore, 3 event(s) from the feed",
            "agenda pupil-1: 140 lessons written",
            "2026-11-26T09:58:12Z sioul: the sign-in form shown (email, password, web address)",
        ] {
            assert_eq!(scrub(kept), kept);
        }
        // The log and the detail are both scrubbed, whoever wrote the line.
        assert_eq!(log_line("2026-11-26T09:58:12Z refresh with refresh_token=RT-invented-9999"), "2026-11-26T09:58:12Z refresh with refresh_token=[hidden]");
        assert_eq!(detail_shown("the feed https://calendar.example.org/feed?key=k7f3a9e2b1c\nanswered 401"), "the feed https://calendar.example.org/feed?[hidden] answered 401");
        assert!(detail_shown(&"word ".repeat(100)).chars().count() <= DETAIL_WIDTH + 1);
    }

    /// The card's lines of the log: the newest first, as many as asked,
    /// scrubbed even when an older Sioul wrote them unscrubbed.
    #[test]
    fn the_log_s_last_lines_come_newest_first_and_scrubbed() {
        let path = std::env::temp_dir().join(format!("sioul-extensions-tail-{}.log", std::process::id()));
        std::fs::write(&path, "one\ntwo with key=k7f3a9e2b1c\n\nthree\n").unwrap();
        assert_eq!(log_tail(&path, 2), vec!["three".to_string(), "two with key=[hidden]".to_string()]);
        assert_eq!(log_tail(&path, 50).len(), 3);
        let _ = std::fs::remove_file(&path);
        assert!(log_tail(&path, 50).is_empty(), "no log: no line");
    }

    /// A problem's detail is kept with it and goes with it; a record written
    /// by Sioul 0.1.0 (no detail) reads, and one with a detail reads where
    /// 0.1.0's `Installed` would pass over the key.
    #[test]
    fn a_problem_keeps_its_detail_and_an_older_record_reads() {
        let offer = crate::extensions::manifest::parse(crate::extensions::manifest::SAMPLE).unwrap().offers.remove(0);
        let mut installed = Installed::new(offer);
        installed.note_problem(Some("sign-in".into()), "the password sign-in answered HTTP 400 with INVALID_LOGIN_CREDENTIALS");
        assert_eq!(installed.detail, "the password sign-in answered HTTP 400 with INVALID_LOGIN_CREDENTIALS");
        let text = toml::to_string(&State { extensions: vec![installed.clone()], ..State::default() }).unwrap();
        assert!(text.contains("detail = "), "{text}");
        let back: State = toml::from_str(&text).unwrap();
        assert_eq!(back.extensions[0], installed);
        installed.note_problem(None, "anything");
        assert_eq!((installed.problem.as_deref(), installed.detail.as_str()), (None, ""));
        let older = text.lines().filter(|l| !l.starts_with("detail = ")).collect::<Vec<_>>().join("\n");
        let read: State = toml::from_str(&older).unwrap();
        assert_eq!((read.extensions[0].problem.as_deref(), read.extensions[0].detail.as_str()), (Some("sign-in"), ""));
    }

    /// M2: an offer kept in the record is checked as a manifest's are when the record is read.
    #[test]
    fn a_kept_offer_that_is_not_one_is_left_out() {
        let path = std::env::temp_dir().join(format!("sioul-extensions-kept-{}.toml", std::process::id()));
        let offer = crate::extensions::manifest::parse(crate::extensions::manifest::SAMPLE).unwrap().offers.remove(0);
        let wrong = crate::extensions::manifest::Offer { id: "../x".into(), ..offer.clone() };
        let state = State { extensions: vec![Installed::new(offer.clone()), Installed::new(wrong)], ..State::default() };
        std::fs::write(&path, toml::to_string(&state).unwrap()).unwrap();
        let read = load(&path);
        assert_eq!(read.extensions.iter().map(|i| i.offer.id.as_str()).collect::<Vec<_>>(), vec!["timetable"]);
        let _ = std::fs::remove_file(&path);
    }

    /// S2: a fresh device, or one whose record was lost or no longer reads,
    /// still refuses a manifest older than the lowest this release accepts.
    #[test]
    fn an_older_manifest_is_refused_on_a_fresh_device_too() {
        let fresh = || State::default();
        assert!(fresh().accept(super::super::LOWEST_SERIAL - 1, "old").is_err(), "below the floor compiled in");
        assert!(fresh().accept(super::super::LOWEST_SERIAL, "now").is_ok());
        let unreadable = std::env::temp_dir().join(format!("sioul-extensions-unreadable-{}.toml", std::process::id()));
        std::fs::write(&unreadable, "serial = [").unwrap();
        assert!(load(&unreadable).accept(super::super::LOWEST_SERIAL - 1, "old").is_err(), "a record that no longer reads gives no way back");
        let _ = std::fs::remove_file(&unreadable);
    }
}

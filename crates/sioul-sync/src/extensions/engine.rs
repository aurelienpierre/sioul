// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The engine: an extension's file run in a sandbox of its own
//! (docs/extensions.md). wasmi, an interpreter written in Rust, runs the
//! WebAssembly core module: no code is made at run time, so it runs the same
//! on Linux, in the Flatpak, on Windows, on macOS and on Android.
//!
//! - **Only Sioul's interface**: the module may import the nine functions of
//!   the `sioul` module and nothing else (no WASI, no undefined host call). A
//!   module that imports anything outside that fixed interface is not started
//!   (`Stop::AsksMore`). What it may *do* with each function is its grants,
//!   enforced at call time, not at load: every extension wasm imports all nine
//!   whatever it calls (the shared crate's `&mut dyn Host` vtable pins every
//!   method), so an import it will not be allowed to use is defined inertly and
//!   the ask itself is refused (`web` to an ungranted host, `secret` or
//!   `sign_in` without the grant for it).
//! - **Limits**: one memory of 64 MiB at most, one table; computation given
//!   in slices of fuel, 4 billion steps in all; 5 minutes for a call, the
//!   person's time on a sign-in page aside; 100 web requests a call.
//! - **One calm end**: a module that loops, grows without end, traps or
//!   answers nonsense ends with a `Stop`, which the window says in one
//!   sentence; the module is thrown away, nothing of it is taken in.
//!
//! Each ask is counted, then checked here (its form, its host, its size, the
//! time left) before what it reaches does it (`Services`: the network, the
//! keyring, the person); the log is kept here, every hidden value replaced
//! once the call has ended. Never run on the window's thread: a call may last
//! minutes.
//!
//! **What Sioul holds for a call is bounded in bytes**, outside the module's
//! own memory: a line is cut when it is kept (`LINE_KEPT`), Sioul's own
//! lines are counted (`SAID_MOST`), a host its manifest does not name is
//! kept once, cut short, with a count (`REFUSED_HOSTS`), and the values to
//! hide are capped in number and in bytes (`HIDE_MOST`, `HIDE_BYTES`, a long
//! value kept in pieces of `HIDE_PIECE`). When no more can be hidden, the
//! extension's lines of that call are dropped rather than written unhidden.
//! `finish` then works on those bounds alone.

use serde_json::{Value, json};
use sioul_core::extensions::manifest::Grants;
use std::time::{Duration, Instant};
use wasmi::{Caller, CompilationMode, Config, EnforcedLimits, Engine, Extern, ExternType, Func, Linker, Memory, Module, ResumableCall, Store, StoreLimits, StoreLimitsBuilder, Val};

/// The interface's limits (docs/extensions.md, "Limits of a call").
#[derive(Debug, Clone)]
pub struct Limits {
    /// The file's size.
    pub file: usize,
    /// The module's memory, in bytes.
    pub memory: usize,
    /// Steps of the interpreter in all, and how many are given at a time.
    pub steps: u64,
    pub slice: u64,
    /// A call's time, the person's time on a sign-in page aside.
    pub time: Duration,
    /// The person's time on a sign-in page.
    pub sign_in: Duration,
    /// Web requests a call, each one's time, a request's body, an answer's body once unpacked.
    pub requests: u32,
    pub request_time: Duration,
    pub request_body: usize,
    pub answer_body: usize,
    /// A call, an ask, an answer, a reply.
    pub call: usize,
    pub ask: usize,
    pub answer: usize,
    pub reply: usize,
    /// Asks of any kind a call.
    pub asks: u32,
    /// Lines the extension may write in its log a call, and a line's length.
    pub log_lines: usize,
    pub log_width: usize,
}

impl Default for Limits {
    fn default() -> Self {
        const MIB: usize = 1 << 20;
        Self {
            file: 8 * MIB,
            memory: 64 * MIB,
            steps: 4_000_000_000,
            slice: 20_000_000,
            time: Duration::from_secs(300),
            sign_in: Duration::from_secs(15 * 60),
            requests: 100,
            request_time: Duration::from_secs(30),
            request_body: 256 * 1024,
            answer_body: 4 * MIB,
            call: 64 * 1024,
            ask: MIB,
            answer: 5 * MIB,
            reply: 8 * MIB,
            asks: 5_000,
            log_lines: 50,
            log_width: 300,
        }
    }
}

/// A line of the log as it is kept, in bytes: the extension's and Sioul's own.
const LINE_KEPT: usize = 4096;
/// Sioul's own lines a call (a request each, at most 100, and a few more).
const SAID_MOST: usize = 160;
/// The hosts a manifest does not name kept a call, each cut to `HOST_SHOWN` characters.
const REFUSED_HOSTS: usize = 8;
const HOST_SHOWN: usize = 80;
/// The values hidden a call, in number and in bytes; a longer value is kept
/// in pieces of `HIDE_PIECE` bytes, each hidden, which side by side hide it whole.
const HIDE_MOST: usize = 256;
const HIDE_BYTES: usize = 128 << 10;
const HIDE_PIECE: usize = 2048;

/// `text` cut at `most` bytes, on a character's edge; whether it was cut.
fn cut_at(text: &str, most: usize) -> (String, bool) {
    if text.len() <= most {
        return (text.to_string(), false);
    }
    let mut end = most;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

/// A line of the call's log: its text, cut at `LINE_KEPT` when kept; whether
/// it was; whether it is the extension's (dropped when hiding is full).
#[derive(Debug, Clone)]
struct Line {
    text: String,
    cut: bool,
    own: bool,
}

/// How a call ended without a reply: said in one sentence by the window
/// (`word` names it), its detail in the log only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// The file is not a module this engine reads.
    NotRead(String),
    /// It imports what its manifest does not grant.
    AsksMore(String),
    /// It speaks another interface.
    Interface(i32),
    /// It took too long: steps or time.
    TooLong,
    /// It asked for more memory than it may have.
    Memory,
    /// It ended in an error of its own.
    Trapped(String),
    /// It replied what cannot be read.
    Nonsense(String),
    /// It was removed meanwhile: the call ended before it could keep anything.
    Removed,
}

impl Stop {
    /// The word the window says it by ("stopped-time"…), kept as the extension's last problem.
    pub fn word(&self) -> &'static str {
        match self {
            Self::NotRead(_) => "stopped-unread",
            Self::AsksMore(_) => "stopped-asks-more",
            Self::Interface(_) => "stopped-interface",
            Self::TooLong => "stopped-time",
            Self::Memory => "stopped-memory",
            Self::Trapped(_) => "stopped-error",
            Self::Nonsense(_) => "stopped-nonsense",
            Self::Removed => "stopped-removed",
        }
    }

    /// What happened, for the log.
    pub fn detail(&self) -> String {
        match self {
            Self::NotRead(why) => format!("the file does not read: {why}"),
            Self::AsksMore(what) => format!("it imports {what}, which is not a function of Sioul's interface"),
            Self::Interface(n) => format!("it speaks interface {n}"),
            Self::TooLong => "it took too long".into(),
            Self::Memory => "it asked for too much memory".into(),
            Self::Trapped(why) => format!("it ended in an error of its own: {why}"),
            Self::Nonsense(why) => format!("it replied nonsense: {why}"),
            Self::Removed => "it was removed meanwhile".into(),
        }
    }
}

/// A web request, checked, for `Services::web`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebRequest {
    pub method: String,
    pub url: String,
    /// The host, as checked (lower case).
    pub host: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

/// A web request's answer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebAnswer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// What the asks reach once checked: the network, the keyring, the person.
/// A refusal is one word of the interface's (`network`, `size`, `keyring`…).
pub trait Services {
    /// The request made, `wait` at most; the body cut at `most` bytes once unpacked (`size` beyond).
    fn web(&mut self, request: &WebRequest, wait: Duration, most: usize) -> Result<WebAnswer, &'static str>;
    fn secret(&mut self) -> Result<Option<String>, &'static str>;
    fn keep_secret(&mut self, secret: &str) -> Result<(), &'static str>;
    fn forget_secret(&mut self) -> Result<(), &'static str>;
    /// The sign-in page shown (`host` named above it), `wait` at most: the
    /// address it returned to, which begins with `watch`; none when the person closed it.
    fn sign_in(&mut self, open: &str, watch: &str, host: &str, wait: Duration) -> Result<Option<String>, &'static str>;
    /// Seconds since 1970.
    fn now(&self) -> i64;
    /// Whether the extension was removed while it runs: the call ends at its
    /// next ask or slice, before anything is kept (its secret above all).
    fn removed(&mut self) -> bool {
        false
    }
}

/// What a call came to: its reply, or why it stopped; and its log's lines,
/// each hidden value replaced, each cut.
#[derive(Debug)]
pub struct Outcome {
    pub reply: Result<String, Stop>,
    pub log: Vec<String>,
    /// The most bytes Sioul held at once for the call's log and its hidden values.
    pub held: usize,
    /// When the call's last web answer came (seconds since 1970): the age of
    /// the data it brought; none when it asked the web nothing.
    pub answered: Option<i64>,
}

/// The kinds of ask, each its own import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Now,
    Log,
    Hide,
    Random,
    Web,
    Secret,
    SignIn,
}

/// The functions of Sioul's interface (docs/extensions.md): the only imports a
/// module may have. Every extension wasm imports all nine whatever it calls —
/// the shared crate runs each call through `&mut dyn Host`, whose vtable pins
/// every trait method, so LLD cannot strip an unused import. What a module may
/// actually *do* with each is its grants, enforced at call time (`web_answer`,
/// `secret_answer`, `sign_in_answer`), never by gating the load or the linker:
/// a capability boundary that leaned on the optimizer stripping an unused
/// vtable import would not be sound. A module importing anything outside this
/// set (another module, an unknown name, a non-function) is not started.
const INTERFACE_FUNCS: [&str; 9] = ["answer", "reply", "now", "log", "hide", "random", "web", "secret", "sign_in"];

/// The credential values a `sign-in` call carries (`Call::SignIn`), to hide
/// from every log line of the call; empty for any other call or shape. Sioul
/// keeps the values only in the call's JSON and drops them when the call ends.
fn credential_values(call: &str) -> Vec<String> {
    match serde_json::from_str::<Value>(call) {
        Ok(Value::Object(map)) => match map.get("credentials") {
            Some(Value::Object(fields)) => fields.values().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// One call's state, the store's data.
struct Visit<S> {
    services: S,
    grants: Grants,
    limits: Limits,
    may_sign_in: bool,
    signed_in: bool,
    started: Instant,
    /// The person's time on a sign-in page, not counted.
    aside: Duration,
    /// The last answer, until the module copies it.
    kept: Vec<u8>,
    reply: Option<Vec<u8>>,
    hidden: Vec<String>,
    /// The bytes `hidden` holds; whether a value could not be hidden (the bounds reached).
    hidden_bytes: usize,
    hiding_full: bool,
    log: Vec<Line>,
    /// The extension's lines, and Sioul's own, written this call.
    lines: usize,
    said_lines: usize,
    /// Hosts its manifest does not name, cut short, each with how many asks named it; and the rest.
    refused_hosts: Vec<(String, u32)>,
    refused_more: u32,
    requests: u32,
    asks: u32,
    stop: Option<Stop>,
    store_limits: StoreLimits,
    /// The most bytes held at once for the log and the hidden values (`Outcome::held`).
    held: usize,
    /// When the last web answer came (`Outcome::answered`).
    answered: Option<i64>,
}

impl<S: Services> Visit<S> {
    /// What the log and the hidden values hold now, in bytes; the most kept.
    fn weigh(&mut self) {
        let now = self.log.iter().map(|l| l.text.len()).sum::<usize>() + self.hidden_bytes + self.refused_hosts.iter().map(|(h, _)| h.len()).sum::<usize>();
        self.held = self.held.max(now);
    }

    fn spent(&self) -> Duration {
        self.started.elapsed().saturating_sub(self.aside)
    }

    fn left(&self) -> Duration {
        self.limits.time.saturating_sub(self.spent())
    }

    /// A value never to reach the log: kept whole when short, else in pieces
    /// that side by side hide it whole; when the bounds are reached, hiding
    /// is full and the extension's lines of the call will be dropped.
    fn hide(&mut self, value: &str) {
        if value.len() < 4 {
            return;
        }
        let mut pieces: Vec<&str> = Vec::new();
        let mut rest = value;
        while rest.len() > HIDE_PIECE {
            let mut end = HIDE_PIECE;
            while !rest.is_char_boundary(end) {
                end -= 1;
            }
            pieces.push(&rest[..end]);
            rest = &rest[end..];
        }
        // A last piece too short to look for joins the one before it.
        match pieces.last_mut() {
            Some(last) if rest.len() < 4 => {
                let start = value.len() - rest.len() - last.len();
                *last = &value[start..];
            }
            _ => pieces.push(rest),
        }
        for piece in pieces {
            if self.hidden.iter().any(|h| h == piece) {
                continue;
            }
            if self.hidden.len() >= HIDE_MOST || self.hidden_bytes + piece.len() > HIDE_BYTES {
                self.hiding_full = true;
                return;
            }
            self.hidden_bytes += piece.len();
            self.hidden.push(piece.to_string());
        }
    }

    /// Every string inside a JSON text hidden, the text itself too.
    fn hide_json(&mut self, text: &str) {
        self.hide(text);
        fn walk(value: &Value, out: &mut Vec<String>) {
            match value {
                Value::String(s) => out.push(s.clone()),
                Value::Array(all) => all.iter().for_each(|v| walk(v, out)),
                Value::Object(all) => all.values().for_each(|v| walk(v, out)),
                _ => {}
            }
        }
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            let mut strings = Vec::new();
            walk(&value, &mut strings);
            for s in strings {
                self.hide(&s);
            }
        }
    }

    /// A line of Sioul's own in the extension's log, cut when kept; past `SAID_MOST`, none.
    fn said(&mut self, line: String) {
        if self.said_lines >= SAID_MOST {
            return;
        }
        self.said_lines += 1;
        let at = jiff::Timestamp::from_second(self.services.now()).map(|t| t.to_string()).unwrap_or_default();
        let (text, cut) = cut_at(&format!("{at} sioul: {line}"), LINE_KEPT);
        self.log.push(Line { text, cut, own: false });
    }

    /// A host its manifest does not name, asked: kept once a call, cut short, with a count.
    fn refused_host(&mut self, host: &str) {
        let shown: String = host.chars().take(HOST_SHOWN).collect();
        if let Some((_, count)) = self.refused_hosts.iter_mut().find(|(h, _)| *h == shown) {
            *count += 1;
        } else if self.refused_hosts.len() < REFUSED_HOSTS {
            self.refused_hosts.push((shown, 1));
        } else {
            self.refused_more += 1;
        }
    }
}

fn refused(why: &str) -> Value {
    json!({ "refused": why })
}

// ---------------------------------------------------------------- the asks

fn now_answer<S: Services>(visit: &mut Visit<S>) -> Value {
    let unix = visit.services.now();
    json!({ "now": jiff::Timestamp::from_second(unix).map(|t| t.to_string()).unwrap_or_default(), "unix": unix })
}

fn log_answer<S: Services>(visit: &mut Visit<S>, ask: &Value) -> Value {
    let Some(line) = ask["line"].as_str() else { return refused("form") };
    // Past its lines, or once a value could not be hidden: nothing more of the extension's is kept.
    if visit.lines >= visit.limits.log_lines || visit.hiding_full {
        return json!({ "logged": false });
    }
    visit.lines += 1;
    let at = jiff::Timestamp::from_second(visit.services.now()).map(|t| t.to_string()).unwrap_or_default();
    // Cut again once hidden values are replaced (`finish`): `LINE_KEPT` bytes meanwhile.
    let line: String = line.chars().filter(|c| !c.is_control()).collect();
    let (text, cut) = cut_at(&format!("{at} {line}"), LINE_KEPT);
    visit.log.push(Line { text, cut, own: true });
    json!({ "logged": true })
}

/// The most random bytes an ask gives.
const RANDOM_MOST: u64 = 64;

/// Random bytes from the system (E1): a sign-in's `state`, PKCE's verifier.
/// Never logged; given in URL-safe base64 without padding.
fn random_answer(ask: &Value) -> Value {
    use base64::Engine;
    use chacha20poly1305::aead::OsRng;
    use chacha20poly1305::aead::rand_core::RngCore;
    let bytes = match &ask["bytes"] {
        Value::Null => 32,
        value => match value.as_u64() {
            Some(n @ 1..=RANDOM_MOST) => n,
            _ => return refused("form"),
        },
    };
    let mut drawn = vec![0u8; bytes as usize];
    OsRng.fill_bytes(&mut drawn);
    json!({ "random": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&drawn) })
}

fn hide_answer<S: Services>(visit: &mut Visit<S>, ask: &Value) -> Value {
    let Some(values) = ask["values"].as_array() else { return refused("form") };
    let mut count = 0;
    for value in values.iter().filter_map(Value::as_str) {
        visit.hide(value);
        count += 1;
    }
    json!({ "hidden": count })
}

/// Header names Sioul sets itself.
fn forbidden_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["host", "content-length", "transfer-encoding", "connection", "upgrade", "te", "trailer", "keep-alive"].contains(&name.as_str()) || name.starts_with("proxy-")
}

/// An `https` address's host, checked: the default port alone, no user name.
fn https_host(url: &str) -> Result<String, &'static str> {
    let rest = url.strip_prefix("https://").ok_or("scheme")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') || authority.is_empty() {
        return Err("address");
    }
    let host = match authority.rsplit_once(':') {
        Some((host, "443")) => host,
        Some(_) => return Err("address"),
        None => authority,
    };
    if url.bytes().any(|b| b.is_ascii_whitespace() || b.is_ascii_control()) {
        return Err("address");
    }
    Ok(host.to_ascii_lowercase())
}

fn web_answer<S: Services>(visit: &mut Visit<S>, ask: &Value) -> Value {
    let method = match ask["method"].as_str() {
        Some("GET") => "GET",
        Some("POST") => "POST",
        _ => return refused("form"),
    };
    let Some(url) = ask["url"].as_str() else { return refused("form") };
    let host = match https_host(url) {
        Ok(host) => host,
        Err(why) => return refused(why),
    };
    if !visit.grants.hosts.contains(&host) {
        visit.refused_host(&host);
        return refused("host");
    }
    let mut headers = Vec::new();
    for header in ask["headers"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let (Some(name), Some(value)) = (header[0].as_str(), header[1].as_str()) else { return refused("form") };
        let token = !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
        if !token || value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) || name.len() + value.len() > 8192 {
            return refused("form");
        }
        if forbidden_header(name) {
            return refused("form");
        }
        if name.eq_ignore_ascii_case("authorization") {
            visit.hide(value);
            if let Some((_, token)) = value.split_once(' ') {
                visit.hide(token.trim());
            }
        }
        headers.push((name.to_string(), value.to_string()));
    }
    if headers.len() > 32 {
        return refused("form");
    }
    let body = match &ask["body"] {
        Value::Null => None,
        Value::String(body) if body.len() <= visit.limits.request_body => Some(body.clone()),
        Value::String(_) => return refused("size"),
        _ => return refused("form"),
    };
    if visit.requests >= visit.limits.requests {
        return refused("count");
    }
    let left = visit.left();
    if left < Duration::from_secs(1) {
        return refused("time");
    }
    visit.requests += 1;
    let request = WebRequest { method: method.to_string(), url: url.to_string(), host: host.clone(), headers, body };
    let wait = left.min(visit.limits.request_time);
    let most = visit.limits.answer_body;
    // A host and a status, never a path (W3): a path may name a pupil or an account.
    match visit.services.web(&request, wait, most) {
        Ok(answer) => {
            visit.answered = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64);
            visit.said(format!("{method} {host}: {} ({} bytes)", answer.status, answer.body.len()));
            let headers: Vec<[String; 2]> = answer.headers.iter().map(|(n, v)| [n.to_ascii_lowercase(), v.clone()]).collect();
            json!({ "status": answer.status, "headers": headers, "body": answer.body })
        }
        Err(why) => {
            visit.said(format!("{method} {host}: {why}"));
            refused(why)
        }
    }
}

fn secret_answer<S: Services>(visit: &mut Visit<S>, ask: &Value) -> Value {
    // The capability gate, at call time. Every module imports `secret`, but
    // only a sign-in — a redirect (`sign_in`) or credentials — may keep one.
    // This replicates, at call time, the linker gate the load check used to
    // stand in for, and no longer leans on the optimizer stripping the import.
    if visit.grants.sign_in.is_none() && visit.grants.credentials.is_empty() {
        return refused("not-now");
    }
    if ask["get"] == true {
        return match visit.services.secret() {
            Ok(Some(secret)) => {
                visit.hide_json(&secret);
                json!({ "secret": secret })
            }
            Ok(None) => json!({ "secret": null }),
            Err(why) => refused(why),
        };
    }
    if let Some(secret) = ask["keep"].as_str() {
        // Refused whole, never cut: a secret cut would not sign in again.
        if secret.len() > sioul_core::extensions::SECRET_BYTES {
            visit.said(format!("refused a secret of {} bytes: a keyring entry holds {} on every system", secret.len(), sioul_core::extensions::SECRET_BYTES));
            return refused("size");
        }
        visit.hide_json(secret);
        return match visit.services.keep_secret(secret) {
            Ok(()) => json!({ "kept": true }),
            Err(why) => refused(why),
        };
    }
    if ask["forget"] == true {
        return match visit.services.forget_secret() {
            Ok(()) => json!({ "forgotten": true }),
            Err(why) => refused(why),
        };
    }
    refused("form")
}

fn sign_in_answer<S: Services>(visit: &mut Visit<S>, ask: &Value) -> Value {
    if !visit.may_sign_in {
        return refused("not-now");
    }
    if visit.signed_in {
        return refused("count");
    }
    let (Some(open), Some(watch)) = (ask["open"].as_str(), ask["watch"].as_str()) else { return refused("form") };
    let host = match https_host(open) {
        Ok(host) => host,
        Err(why) => return refused(why),
    };
    if !visit.grants.hosts.contains(&host) {
        return refused("host");
    }
    let Some(prefix) = visit.grants.sign_in.clone() else { return refused("not-now") };
    if !watch.starts_with(&prefix) || !sioul_core::extensions::manifest::watched(watch) {
        return refused("form");
    }
    visit.signed_in = true;
    visit.said(format!("a sign-in page shown on {host}"));
    let shown = Instant::now();
    let wait = visit.limits.sign_in;
    let outcome = visit.services.sign_in(open, watch, &host, wait);
    visit.aside += shown.elapsed();
    match outcome {
        Ok(Some(returned)) if sioul_core::extensions::manifest::returns_to(&returned, watch) => {
            visit.hide(&returned);
            // The values of its query and of its fragment (`#access_token=…`), each hidden.
            if let Some((_, query)) = returned.split_once(['?', '#']) {
                for pair in query.split(['&', '#']) {
                    if let Some((_, value)) = pair.split_once('=') {
                        visit.hide(value);
                        visit.hide(&value.replace('+', " ").replace("%2B", "+").replace("%2F", "/").replace("%3D", "="));
                    }
                }
            }
            visit.said("the sign-in page returned".into());
            json!({ "returned": returned })
        }
        Ok(_) => {
            visit.said("the sign-in page was closed".into());
            json!({ "cancelled": true })
        }
        Err(why) => refused(why),
    }
}

// ---------------------------------------------------------------- memory, and the imports

fn memory_of<S>(caller: &Caller<'_, Visit<S>>) -> Option<Memory> {
    caller.get_export("memory").and_then(Extern::into_memory)
}

/// The call ended by the host, with why.
fn halt<S, T>(caller: &mut Caller<'_, Visit<S>>, stop: Stop) -> Result<T, wasmi::Error> {
    caller.data_mut().stop.get_or_insert(stop);
    Err(wasmi::Error::new("stopped by Sioul"))
}

/// `len` bytes of the module's memory at `ptr`; none outside it.
fn read<S>(caller: &Caller<'_, Visit<S>>, ptr: i32, len: i32, most: usize) -> Option<Vec<u8>> {
    let (ptr, len) = (usize::try_from(ptr).ok()?, usize::try_from(len).ok()?);
    if len > most {
        return None;
    }
    let mut bytes = vec![0u8; len];
    memory_of(caller)?.read(caller, ptr, &mut bytes).ok()?;
    Some(bytes)
}

/// One ask: counted, read, checked, done, its answer kept for `answer`; its length.
fn ask<S: Services>(mut caller: Caller<'_, Visit<S>>, kind: Kind, ptr: i32, len: i32) -> Result<i32, wasmi::Error> {
    let (most, asks) = (caller.data().limits.ask, caller.data().limits.asks);
    if caller.data().spent() > caller.data().limits.time {
        return halt(&mut caller, Stop::TooLong);
    }
    // Removed meanwhile: nothing more is done for it, its secret above all.
    if caller.data_mut().services.removed() {
        return halt(&mut caller, Stop::Removed);
    }
    // Counted before anything is copied: past the count, the ask is not read.
    caller.data_mut().asks += 1;
    let bytes = if caller.data().asks > asks {
        None
    } else {
        match read(&caller, ptr, len, most) {
            Some(bytes) => Some(bytes),
            None => return halt(&mut caller, Stop::Trapped("an ask outside its memory, or larger than 1 MiB".into())),
        }
    };
    let visit = caller.data_mut();
    let answer = match bytes {
        None => refused("count"),
        Some(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Err(_) => refused("form"),
            Ok(ask) => match kind {
                Kind::Now => now_answer(visit),
                Kind::Log => log_answer(visit, &ask),
                Kind::Hide => hide_answer(visit, &ask),
                Kind::Random => random_answer(&ask),
                Kind::Web => web_answer(visit, &ask),
                Kind::Secret => secret_answer(visit, &ask),
                Kind::SignIn => sign_in_answer(visit, &ask),
            },
        },
    };
    let mut text = answer.to_string().into_bytes();
    if text.len() > visit.limits.answer {
        text = refused("size").to_string().into_bytes();
    }
    let len = i32::try_from(text.len()).unwrap_or(0);
    visit.kept = text;
    visit.weigh();
    Ok(len)
}

/// Every function of the interface, defined unconditionally: a module imports
/// all nine whatever its grants, so all nine are wired. An ask the grants do
/// not allow is refused at call time (`web_answer`, `secret_answer`,
/// `sign_in_answer`), so an imported-but-ungranted function is inert.
fn link<S: Services + 'static>(linker: &mut Linker<Visit<S>>) -> Result<(), wasmi::Error> {
    linker.func_wrap("sioul", "answer", |mut caller: Caller<'_, Visit<S>>, ptr: i32| -> Result<(), wasmi::Error> {
        let kept = std::mem::take(&mut caller.data_mut().kept);
        let written = usize::try_from(ptr).ok().zip(memory_of(&caller)).is_some_and(|(at, memory)| memory.write(&mut caller, at, &kept).is_ok());
        if !written {
            return halt(&mut caller, Stop::Trapped("an answer asked outside its memory".into()));
        }
        Ok(())
    })?;
    linker.func_wrap("sioul", "reply", |mut caller: Caller<'_, Visit<S>>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
        if caller.data().reply.is_some() {
            return halt(&mut caller, Stop::Nonsense("it replied twice".into()));
        }
        let most = caller.data().limits.reply;
        match read(&caller, ptr, len, most) {
            Some(bytes) => {
                caller.data_mut().reply = Some(bytes);
                Ok(())
            }
            None => halt(&mut caller, Stop::Nonsense("a reply outside its memory, or larger than 8 MiB".into())),
        }
    })?;
    linker.func_wrap("sioul", "now", |caller: Caller<'_, Visit<S>>, ptr: i32, len: i32| ask(caller, Kind::Now, ptr, len))?;
    linker.func_wrap("sioul", "log", |caller: Caller<'_, Visit<S>>, ptr: i32, len: i32| ask(caller, Kind::Log, ptr, len))?;
    linker.func_wrap("sioul", "hide", |caller: Caller<'_, Visit<S>>, ptr: i32, len: i32| ask(caller, Kind::Hide, ptr, len))?;
    linker.func_wrap("sioul", "random", |caller: Caller<'_, Visit<S>>, ptr: i32, len: i32| ask(caller, Kind::Random, ptr, len))?;
    linker.func_wrap("sioul", "web", |caller: Caller<'_, Visit<S>>, ptr: i32, len: i32| ask(caller, Kind::Web, ptr, len))?;
    linker.func_wrap("sioul", "secret", |caller: Caller<'_, Visit<S>>, ptr: i32, len: i32| ask(caller, Kind::Secret, ptr, len))?;
    linker.func_wrap("sioul", "sign_in", |caller: Caller<'_, Visit<S>>, ptr: i32, len: i32| ask(caller, Kind::SignIn, ptr, len))?;
    Ok(())
}

// ---------------------------------------------------------------- a call

/// Why the module stopped, from wasmi's error and what the host said.
fn stop_of<S>(store: &mut Store<Visit<S>>, error: &wasmi::Error) -> Stop {
    if let Some(stop) = store.data_mut().stop.take() {
        return stop;
    }
    match error.as_trap_code() {
        Some(wasmi::TrapCode::OutOfFuel) => Stop::TooLong,
        Some(wasmi::TrapCode::GrowthOperationLimited) => Stop::Memory,
        Some(code) => Stop::Trapped(code.to_string()),
        None => Stop::Trapped(error.to_string()),
    }
}

/// A function of the module run to its end, fuel given in slices, the steps
/// and the time checked between them; its one result, an `i32`.
fn run<S: Services>(store: &mut Store<Visit<S>>, func: &Func, params: &[Val]) -> Result<i32, Stop> {
    let (slice, steps, time) = (store.data().limits.slice, store.data().limits.steps, store.data().limits.time);
    store.set_fuel(slice).map_err(|e| Stop::Trapped(e.to_string()))?;
    let mut used = 0u64;
    let mut results = [Val::I32(0)];
    let mut call = func.call_resumable(&mut *store, params, &mut results);
    loop {
        match call {
            Ok(ResumableCall::Finished) => return results[0].i32().ok_or_else(|| Stop::Nonsense("a result that is not an i32".into())),
            Ok(ResumableCall::OutOfFuel(out)) => {
                used += slice;
                if used >= steps || store.data().spent() > time {
                    return Err(Stop::TooLong);
                }
                if store.data_mut().services.removed() {
                    return Err(Stop::Removed);
                }
                store.set_fuel(slice.max(out.required_fuel())).map_err(|e| Stop::Trapped(e.to_string()))?;
                call = out.resume(&mut *store, &mut results);
            }
            Ok(ResumableCall::HostTrap(trap)) => {
                let error = trap.into_host_error();
                return Err(stop_of(store, &error));
            }
            Err(error) => return Err(stop_of(store, &error)),
        }
    }
}

/// The log's lines with every hidden value replaced, then cut at `width`
/// characters. A line cut when it was kept loses its last `HIDE_PIECE` bytes
/// more: a value it held across that cut cannot then show in part. When a
/// value could not be hidden (`full`), the extension's own lines are dropped,
/// and one line of Sioul's says so. Bounded: at most `HIDE_MOST` values over
/// lines of `LINE_KEPT` bytes.
fn finish(log: Vec<Line>, hidden: &mut [String], width: usize, full: bool) -> Vec<String> {
    hidden.sort_by_key(|h| std::cmp::Reverse(h.len()));
    let dropped = if full { log.iter().filter(|l| l.own).count() } else { 0 };
    let mut out: Vec<String> = log
        .into_iter()
        .filter(|l| !(full && l.own))
        .map(|line| {
            let mut text = line.text;
            for value in hidden.iter() {
                if text.contains(value.as_str()) {
                    text = text.replace(value.as_str(), "[hidden]");
                }
            }
            if line.cut {
                let (kept, _) = cut_at(&text, text.len().saturating_sub(HIDE_PIECE + 3));
                text = kept;
            }
            let mut cut: String = text.chars().take(width).collect();
            if line.cut || cut.len() < text.len() {
                cut.push('…');
            }
            cut
        })
        .collect();
    if dropped > 0 {
        out.push(format!("sioul: {dropped} lines of this call dropped: a value it named could not be hidden"));
    }
    out
}

/// Runs one call of the extension in `file`, given only what `grants` say:
/// `call` is the call's JSON, `may_sign_in` whether it may show a sign-in
/// page (a `sign-in` call). The services come back with the outcome.
pub fn call<S: Services + 'static>(file: &[u8], grants: &Grants, call: &str, services: S, limits: &Limits, may_sign_in: bool) -> (Outcome, S) {
    let visit = Visit {
        services,
        grants: grants.clone(),
        limits: limits.clone(),
        may_sign_in,
        signed_in: false,
        started: Instant::now(),
        aside: Duration::ZERO,
        kept: Vec::new(),
        reply: None,
        hidden: Vec::new(),
        hidden_bytes: 0,
        hiding_full: false,
        log: Vec::new(),
        lines: 0,
        said_lines: 0,
        refused_hosts: Vec::new(),
        refused_more: 0,
        requests: 0,
        asks: 0,
        stop: None,
        held: 0,
        answered: None,
        store_limits: StoreLimitsBuilder::new().memory_size(limits.memory).memories(1).tables(1).table_elements(10_000).instances(1).trap_on_grow_failure(true).build(),
    };
    let mut config = Config::default();
    config.consume_fuel(true).compilation_mode(CompilationMode::LazyTranslation).enforced_limits(EnforcedLimits::strict());
    let engine = Engine::new(&config);
    let mut store = Store::new(&engine, visit);
    store.limiter(|visit| &mut visit.store_limits);
    // A credentials sign-in's values are hidden before the module runs, so no
    // log line of this call can carry them, whatever the extension writes.
    for value in credential_values(call) {
        store.data_mut().hide(&value);
    }
    let reply = called(&engine, &mut store, file, call, limits);
    let mut visit = store.into_data();
    // The hosts its manifest does not name: one line a call, each cut short, with its count.
    if !visit.refused_hosts.is_empty() {
        let mut named: Vec<String> = visit.refused_hosts.iter().map(|(host, count)| format!("{host} ({count})")).collect();
        if visit.refused_more > 0 {
            named.push(format!("{} more", visit.refused_more));
        }
        let asked: u32 = visit.refused_hosts.iter().map(|(_, c)| c).sum::<u32>() + visit.refused_more;
        visit.said(format!("refused {asked} requests to hosts its manifest does not name: {}", named.join(", ")));
    }
    if let Err(stop) = &reply {
        visit.said(format!("stopped: {}", stop.detail()));
    }
    visit.weigh();
    let log = finish(std::mem::take(&mut visit.log), &mut visit.hidden, limits.log_width, visit.hiding_full);
    (Outcome { reply, log, held: visit.held, answered: visit.answered }, visit.services)
}

fn called<S: Services + 'static>(engine: &Engine, store: &mut Store<Visit<S>>, file: &[u8], call: &str, limits: &Limits) -> Result<String, Stop> {
    if file.len() > limits.file {
        return Err(Stop::NotRead("larger than 8 MiB".into()));
    }
    if call.len() > limits.call {
        return Err(Stop::Nonsense("a call larger than Sioul sends".into()));
    }
    let module = Module::new(engine, file).map_err(|e| Stop::NotRead(e.to_string()))?;
    // Only Sioul's interface: an import outside the fixed nine functions (another
    // module, an unknown name, a non-function), and it is not started. A module
    // that imports a function it will not be allowed to use still loads; the ask
    // is refused at call time. The grants gate the asks, never the load.
    for import in module.imports() {
        if import.module() != "sioul" || !INTERFACE_FUNCS.contains(&import.name()) || !matches!(import.ty(), ExternType::Func(_)) {
            return Err(Stop::AsksMore(format!("{}.{}", import.module(), import.name())));
        }
    }
    let mut linker = Linker::new(engine);
    link(&mut linker).map_err(|e| Stop::Trapped(e.to_string()))?;
    store.set_fuel(limits.slice).map_err(|e| Stop::Trapped(e.to_string()))?;
    let instance = linker.instantiate_and_start(&mut *store, &module).map_err(|e| match e.as_trap_code() {
        Some(_) => stop_of(store, &e),
        None => Stop::Nonsense(format!("it does not start: {e}")),
    })?;
    let exported = |name: &str| Stop::Nonsense(format!("it exports no {name} of the interface's form"));
    let memory = instance.get_memory(&*store, "memory").ok_or_else(|| exported("memory"))?;
    // Each export of the interface's form (its types checked), then run untyped, in slices.
    instance.get_typed_func::<(), i32>(&*store, "sioul_interface").map_err(|_| exported("sioul_interface"))?;
    instance.get_typed_func::<i32, i32>(&*store, "sioul_alloc").map_err(|_| exported("sioul_alloc"))?;
    instance.get_typed_func::<(i32, i32), i32>(&*store, "sioul_call").map_err(|_| exported("sioul_call"))?;
    let func = |name: &str| instance.get_func(&*store, name).ok_or_else(|| exported(name));
    let (interface, alloc, entry) = (func("sioul_interface")?, func("sioul_alloc")?, func("sioul_call")?);
    let version = run(store, &interface, &[])?;
    if u32::try_from(version).ok() != Some(sioul_core::extensions::INTERFACE) {
        return Err(Stop::Interface(version));
    }
    let len = i32::try_from(call.len()).map_err(|_| Stop::Nonsense("a call too large".into()))?;
    let at = run(store, &alloc, &[Val::I32(len)])?;
    let written = usize::try_from(at).ok().filter(|at| *at != 0).is_some_and(|at| memory.write(&mut *store, at, call.as_bytes()).is_ok());
    if !written {
        return Err(Stop::Nonsense("no room for the call in its memory".into()));
    }
    let status = run(store, &entry, &[Val::I32(at), Val::I32(len)])?;
    let Some(reply) = store.data_mut().reply.take() else { return Err(Stop::Nonsense("it gave no reply".into())) };
    if status != 0 {
        return Err(Stop::Nonsense(format!("its call returned {status}")));
    }
    String::from_utf8(reply).map_err(|_| Stop::Nonsense("a reply that is not UTF-8".into()))
}

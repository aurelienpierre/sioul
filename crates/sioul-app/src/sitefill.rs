// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A site's login filled from Bitwarden on a phone (docs/android.md,
//! "Sites"; docs/sites.md, "Bitwarden"). The site's screen
//! (SiteScreen.java) runs in the window's process; its ⋮ ▸ Fill from
//! Bitwarden and Choose a login… ask here (`answer`, verb "ask"), with the
//! address the web view committed for the page shown, never one a page's
//! script gave:
//! - the vault open and one login for the page's site (its only one, unless
//!   another was chosen there last): the whole call the page runs
//!   (`sites::fill_call`, the values quoted here), at once;
//! - anything else goes through the window, which the screen brings forward:
//!   the vault locked (its unlock card), several logins or none, or Choose a
//!   login… (the chooser), in the window's own cards (SiteFill.qml). The
//!   request waits here until the window reads it: at once when it runs, or
//!   once it has started after Android ended Sioul (`started`);
//! - the window's answer (`chosen`, `cancelled`) brings the site back
//!   forward (verb "site-fill-back", SiteScreen.comeBack). The login chosen
//!   waits here as a one-use ticket, the address asked and the login's id
//!   for two minutes, never its password; the screen takes it (verb "take")
//!   with the address it shows then, and the password is read from the
//!   vault only then, for the same scheme, host and port alone, while the
//!   vault is still open.
//! - a request waits two minutes for the window's answer; then the window's
//!   card closes and its cancel brings the site back, saying no login was
//!   chosen in time. A screen that goes while its fill waits, or finds no
//!   window, tells it here ("forget"): the request and any login chosen for
//!   it are dropped, and the window closes its card (review of 22 November
//!   2026, finding 8).
//!
//! Nothing here keeps a password: the call is built when it is handed. The
//! call once quoted, and the answer as Java gives it back
//! (`sioul_site_fill_free`), are overwritten before they are freed; the
//! texts made on the way (the quoting, the call growing as it is written)
//! are not, nor are Java's own strings, which live until collected (review
//! of 22 November 2026, finding 9).

use crate::backend::{QtThread, Shared, tr};
use std::sync::{Arc, Mutex, PoisonError};

/// How long a login chosen in the window waits for its site, and a request
/// for the window to read it as it starts: two minutes.
const WAITS: i64 = 120;

/// What the screen asked: the site's own login filled ("fill"), or any login chosen ("choose").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Fill,
    Choose,
}

impl Mode {
    // Read from the screen's question, which only a phone asks (`answer`).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn of(word: &str) -> Option<Mode> {
        match word {
            "fill" => Some(Mode::Fill),
            "choose" => Some(Mode::Choose),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Mode::Fill => "fill",
            Mode::Choose => "choose",
        }
    }
}

/// What a question of the screen comes to.
#[derive(PartialEq, Eq)]
pub(crate) enum Answer {
    /// The call the page runs: the login filled.
    Call(String),
    /// The window comes forward and asks there.
    Window,
    /// Still asked in the window: nothing yet.
    Waiting,
    /// Said on the screen; nothing filled.
    Said(String),
    /// Nothing filled, nothing said: cancelled in the window, or nothing was asked.
    Nothing,
}

/// Printed without the call, which holds a password.
impl std::fmt::Debug for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Answer::Call(_) => f.write_str("Call(…)"),
            Answer::Window => f.write_str("Window"),
            Answer::Waiting => f.write_str("Waiting"),
            Answer::Said(words) => f.debug_tuple("Said").field(words).finish(),
            Answer::Nothing => f.write_str("Nothing"),
        }
    }
}

impl Answer {
    /// As the screen reads it: {"call"}, {"window": true}, {"waiting": true},
    /// {"said"}, or {}. The call's own text is wiped once quoted; the answer
    /// is made with room for the zero C adds (`alarms::handed`), so that it
    /// is not moved, leaving a copy behind, as it is handed.
    pub(crate) fn json(self) -> String {
        match self {
            Answer::Call(mut call) => {
                let mut quoted = serde_json::to_string(&call).unwrap_or_else(|_| "\"\"".into());
                crate::sites::wipe(&mut call);
                let mut out = String::with_capacity(quoted.len() + 9 + 1);
                out.push_str("{\"call\":");
                out.push_str(&quoted);
                out.push('}');
                crate::sites::wipe(&mut quoted);
                out
            }
            Answer::Window => r#"{"window":true}"#.into(),
            Answer::Waiting => r#"{"waiting":true}"#.into(),
            Answer::Said(words) => serde_json::json!({ "said": words }).to_string(),
            Answer::Nothing => "{}".into(),
        }
    }
}

/// A fill the window is asked for.
struct Request {
    /// Its number, which the window's answer names: an older card's answer never counts for a newer request.
    serial: i32,
    /// The page's address, as the screen's web view committed it.
    url: String,
    mode: Mode,
    /// When it was asked (Unix seconds).
    at: i64,
}

/// The login chosen in the window, waiting for its site: the address asked and the login's id, never its password.
struct Ticket {
    url: String,
    id: String,
    at: i64,
}

/// The fills under way: the request waiting for the window, the ticket waiting for the site.
pub(crate) struct Fills {
    serial: i32,
    request: Option<Request>,
    ticket: Option<Ticket>,
    /// A request ran out of its two minutes in the window: said once by the site back in front.
    unanswered: bool,
}

impl Fills {
    pub(crate) const fn new() -> Fills {
        Fills { serial: 0, request: None, ticket: None, unanswered: false }
    }

    /// A fill asked by the screen for the page at `url`. `vault`: the
    /// window's state, when the window runs; `account`: a Bitwarden account
    /// is set on this device. A new question drops a ticket not taken.
    pub(crate) fn ask(&mut self, vault: Option<&Shared>, account: bool, url: &str, mode: Mode, now: i64) -> Answer {
        if origin(url).is_none() {
            return Answer::Said(tr().text("site-screen-fill-https", None));
        }
        if !account {
            return Answer::Said(tr().text("bitwarden-missing", None));
        }
        self.ticket = None;
        self.unanswered = false;
        if mode == Mode::Fill
            && let Some(shared) = vault
            && let Some(id) = crate::sites::fill_at_once(shared, url)
        {
            self.request = None;
            return match crate::sites::fill_for(shared, url, &id) {
                Ok(call) => Answer::Call(call),
                Err(why) => Answer::Said(unfilled(why)),
            };
        }
        self.serial = self.serial.checked_add(1).unwrap_or(1);
        self.request = Some(Request { serial: self.serial, url: url.to_string(), mode, at: now });
        Answer::Window
    }

    /// The request the window has to answer, while it waits (two minutes from
    /// the tap): {serial, url, mode, left}, `left` its seconds still to wait,
    /// after which the window reads it again and closes its card.
    pub(crate) fn waiting(&self, now: i64) -> Option<serde_json::Value> {
        self.request.as_ref().filter(|r| now - r.at <= WAITS).map(|r| serde_json::json!({ "serial": r.serial, "url": r.url, "mode": r.mode.word(), "left": WAITS - (now - r.at) }))
    }

    /// The login `id` chosen in the window for request `serial`, while it
    /// waits: kept as the ticket the site takes, the request answered.
    /// Whether it was the request waiting.
    pub(crate) fn chosen(&mut self, serial: i32, id: &str, now: i64) -> bool {
        if id.is_empty() || !self.request.as_ref().is_some_and(|r| r.serial == serial && now - r.at <= WAITS) {
            return false;
        }
        let Some(request) = self.request.take() else { return false };
        self.ticket = Some(Ticket { url: request.url, id: id.to_string(), at: now });
        true
    }

    /// Request `serial` cancelled in the window, or its card closed when its
    /// two minutes ran out (then the site says so as it comes back):
    /// nothing to fill. Whether it was the request kept.
    pub(crate) fn cancelled(&mut self, serial: i32, now: i64) -> bool {
        let Some(request) = self.request.as_ref().filter(|r| r.serial == serial) else { return false };
        self.unanswered = now - request.at > WAITS;
        self.request = None;
        self.ticket = None;
        true
    }

    /// The screen that asked has gone, or found no window (verb "forget"):
    /// its request and any login chosen for it dropped. Whether a request
    /// was waiting, whose card the window then closes.
    pub(crate) fn forget(&mut self) -> bool {
        self.ticket = None;
        self.unanswered = false;
        self.request.take().is_some()
    }

    /// The screen back in front takes the window's answer, once, with the
    /// page's address now: the call for the login chosen, when the page is
    /// at the same scheme, host and port as asked, within two minutes, and
    /// the vault still open; else what to say. The ticket goes either way.
    pub(crate) fn take(&mut self, vault: Option<&Shared>, url: &str, now: i64) -> Answer {
        let Some(ticket) = self.ticket.take() else {
            if std::mem::take(&mut self.unanswered) {
                return Answer::Said(tr().text("site-screen-fill-unanswered", None));
            }
            return if self.request.is_some() { Answer::Waiting } else { Answer::Nothing };
        };
        if now - ticket.at > WAITS {
            return Answer::Said(tr().text("site-screen-fill-late", None));
        }
        if !same_origin(&ticket.url, url) {
            return Answer::Said(tr().text("site-screen-fill-changed", None));
        }
        let Some(shared) = vault else { return Answer::Said(unfilled("bitwarden-locked")) };
        match crate::sites::fill_for(shared, &ticket.url, &ticket.id) {
            Ok(call) => Answer::Call(call),
            Err(why) => Answer::Said(unfilled(why)),
        }
    }
}

/// What a fill that could not be made says: the vault locked meanwhile, or the login gone.
fn unfilled(why: &str) -> String {
    tr().text(if why == "bitwarden-locked" { "site-screen-fill-locked" } else { why }, None)
}

/// An https address's host (lower case, no user) and port (443 when unsaid); None otherwise.
fn origin(url: &str) -> Option<(String, u16)> {
    crate::sites::origin_parts(url).filter(|(scheme, ..)| *scheme == "https").map(|(_, host, port)| (host, port))
}

/// Whether two addresses are the same https origin: scheme, host and port
/// (SiteWeb.sameOrigin, the screen's own check, holds the same rule).
pub(crate) fn same_origin(asked: &str, now: &str) -> bool {
    origin(asked).is_some_and(|asked| origin(now) == Some(asked))
}

/// The fills of this process.
static FILLS: Mutex<Fills> = Mutex::new(Fills::new());

/// The window, once it runs: its state holds the vault, and a request is told to it.
static WINDOW: Mutex<Option<(QtThread, Arc<Shared>)>> = Mutex::new(None);

fn fills() -> std::sync::MutexGuard<'static, Fills> {
    FILLS.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn window() -> Option<(QtThread, Arc<Shared>)> {
    WINDOW.lock().ok().and_then(|w| w.clone())
}

/// The window has started (backend's `start`): the vault is its own from
/// now, and a request that waited for it is told (SiteFill.qml also reads
/// it as it is made).
pub(crate) fn started(qt: &QtThread, shared: &Arc<Shared>) {
    if let Ok(mut window) = WINDOW.lock() {
        *window = Some((qt.clone(), Arc::clone(shared)));
    }
    if fills().waiting(jiff::Timestamp::now().as_second()).is_some() {
        let _ = qt.queue(|mut sioul| sioul.as_mut().site_fill_asked());
    }
}

/// The screen's question (SiteScreen.nativeFill): "ask" {url, mode}, a fill
/// asked; "take" {url}, the window's answer taken as the screen comes back;
/// "forget" {}, the screen gone with its fill, whose card the window closes.
/// The answer as the screen reads it (`Answer::json`). Only a phone asks.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn answer(verb: &str, json: &str) -> String {
    let asked: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let url = asked["url"].as_str().unwrap_or("");
    let now = jiff::Timestamp::now().as_second();
    let window = window();
    let shared = window.as_ref().map(|(_, shared)| shared.as_ref());
    let answer = match verb {
        "ask" => match Mode::of(asked["mode"].as_str().unwrap_or("")) {
            Some(mode) => {
                let answer = fills().ask(shared, crate::sites::account_set(), url, mode, now);
                if answer == Answer::Window
                    && let Some((qt, _)) = &window
                {
                    let _ = qt.queue(|mut sioul| sioul.as_mut().site_fill_asked());
                }
                answer
            }
            None => Answer::Nothing,
        },
        "take" => fills().take(shared, url, now),
        "forget" => {
            if fills().forget()
                && let Some((qt, _)) = &window
            {
                let _ = qt.queue(|mut sioul| sioul.as_mut().site_fill_asked());
            }
            Answer::Nothing
        }
        _ => Answer::Nothing,
    };
    answer.json()
}

/// The request the window has to answer (SiteFill.qml), as JSON {serial, url, mode, left}; "" when none.
pub(crate) fn waiting() -> String {
    fills().waiting(jiff::Timestamp::now().as_second()).map(|r| r.to_string()).unwrap_or_default()
}

/// The login chosen in the window for request `serial`: its ticket kept, the site brought back.
pub(crate) fn chosen(serial: i32, id: &str) {
    if fills().chosen(serial, id, jiff::Timestamp::now().as_second()) {
        back();
    }
}

/// Request `serial` cancelled in the window, or its two minutes over: the site brought back, nothing filled.
pub(crate) fn cancelled(serial: i32) {
    if fills().cancelled(serial, jiff::Timestamp::now().as_second()) {
        back();
    }
}

/// The site's screen that asked brought back forward (SiteScreen.comeBack); nothing on a computer.
fn back() {
    let _ = crate::steps::java("site-fill-back", "{}");
}

/// SiteScreen.nativeFill's side (android/main.cpp): a verb and its JSON, a
/// JSON answer given back to `sioul_site_fill_free`; a panic answers {}.
///
/// # Safety
/// `verb` and `json` are null, or zero-terminated texts valid for the call.
#[cfg(target_os = "android")]
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_site_fill(verb: *const std::ffi::c_char, json: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    // SAFETY: each null or a C string valid for the call, as this function's contract says.
    let (verb, json) = unsafe { (crate::alarms::key_of(verb), crate::alarms::key_of(json)) };
    let answer = std::panic::catch_unwind(|| answer(&verb, &json)).unwrap_or_else(|_| "{}".to_string());
    crate::alarms::handed(answer)
}

/// An answer of `sioul_site_fill` given back: wiped, then freed (it may hold a password).
///
/// # Safety
/// `text` is null, or came from `sioul_site_fill` and is given back once.
#[cfg(target_os = "android")]
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_site_fill_free(text: *mut std::ffi::c_char) {
    if text.is_null() {
        return;
    }
    // SAFETY: `text` was made by `alarms::handed` (CString::into_raw) and is given back once.
    let mut bytes = unsafe { std::ffi::CString::from_raw(text) }.into_bytes();
    for byte in bytes.iter_mut() {
        // SAFETY: a valid, aligned byte of the vector, written through its own reference.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::{Vault, fill_call};
    use sioul_sync::bitwarden::Item;

    /// A made-up vault, open: (id, name, user name, password, its site). No real person, no server.
    fn open(logins: &[(&str, &str, &str, &str, &str)]) -> Shared {
        let shared = Shared::default();
        let items = logins
            .iter()
            .map(|(id, name, user, password, site)| {
                let mut item = Item::default();
                item.id = id.to_string();
                item.name = name.to_string();
                item.username = user.to_string();
                item.password = password.to_string();
                item.uris = vec![site.to_string()];
                item
            })
            .collect();
        *shared.bitwarden.lock().unwrap() = Some(Vault::of(items));
        shared
    }

    fn call(answer: &Answer) -> &str {
        match answer {
            Answer::Call(call) => call,
            other => panic!("a call expected, not {other:?}"),
        }
    }

    /// A fill asked: the vault open and the site's one login, the call at
    /// once, its values the login's; the vault locked, the window not yet
    /// running, several logins, none, or Choose a login…: the window, a
    /// request waiting for it with the address and what was asked; no
    /// account set, or a page not on https: said, nothing asked of the window.
    #[test]
    fn a_fill_asked_fills_at_once_or_goes_through_the_window() {
        // A login filled is remembered for its site, in the test's own state folder.
        sioul_core::config::stand_ins_only();
        let shared = open(&[
            ("one", "Bank", "jane@example.org", "pass-one", "https://bank.example.org"),
            ("a", "Forum A", "jane", "pass-a", "https://forum.example.net"),
            ("b", "Forum B", "j.doe", "pass-b", "https://forum.example.net"),
        ]);
        let now = 1_000_000;
        let mut fills = Fills::new();
        let filled = fills.ask(Some(&shared), true, "https://bank.example.org/login", Mode::Fill, now);
        assert_eq!(call(&filled), fill_call("https://bank.example.org", "jane@example.org", "pass-one", ""), "held to the origin asked");
        assert!(fills.waiting(now).is_none(), "filled at once: nothing asked of the window");

        let through = |vault: Option<&Shared>, url: &str, mode: Mode| {
            let mut fills = Fills::new();
            let answer = fills.ask(vault, true, url, mode, now);
            (answer, fills.waiting(now))
        };
        let locked = Shared::default();
        for (vault, url, mode, why) in [
            (Some(&locked), "https://bank.example.org/login", Mode::Fill, "the vault locked"),
            (None, "https://bank.example.org/login", Mode::Fill, "the window not running yet"),
            (Some(&shared), "https://forum.example.net/sign-in", Mode::Fill, "several logins"),
            (Some(&shared), "https://shop.example.com/", Mode::Fill, "none"),
            (Some(&shared), "https://bank.example.org/login", Mode::Choose, "Choose a login…"),
        ] {
            let (answer, waiting) = through(vault, url, mode);
            assert_eq!(answer, Answer::Window, "{why}");
            let waiting = waiting.expect(why);
            assert_eq!((waiting["url"].as_str(), waiting["mode"].as_str()), (Some(url), Some(mode.word())), "{why}");
        }

        let mut fills = Fills::new();
        assert_eq!(fills.ask(Some(&shared), false, "https://bank.example.org/login", Mode::Fill, now), Answer::Said(tr().text("bitwarden-missing", None)));
        for page in ["http://bank.example.org/login", "", "about:blank", "https://", "file:///sdcard/a.html", "https://bank.example.org:99999/"] {
            assert_eq!(fills.ask(Some(&shared), true, page, Mode::Fill, now), Answer::Said(tr().text("site-screen-fill-https", None)), "{page}");
        }
        assert!(fills.waiting(now).is_none(), "nothing asked of the window");
    }

    /// The login chosen in the window waits as a ticket the site takes once:
    /// the call for that login when the page is still at the scheme, host and
    /// port asked (another path is the same page); then nothing. Another
    /// request's answer, a second answer, a ticket older than two minutes,
    /// another host, port or scheme, the vault locked meanwhile or the window
    /// gone: nothing filled, said, and the ticket gone. Cancel leaves nothing;
    /// a request not answered yet says it waits; a new question drops a
    /// ticket not taken.
    #[test]
    fn the_ticket_is_taken_once_for_its_address_within_two_minutes() {
        sioul_core::config::stand_ins_only();
        let shared = open(&[("a", "Forum A", "jane", "p\"a'ss</script>\u{2028}", "https://forum.example.net"), ("b", "Forum B", "j.doe", "pass-b", "https://forum.example.net")]);
        let asked = "https://forum.example.net/sign-in?next=%2Finbox";
        let now = 2_000_000;
        let mut fills = Fills::new();
        let choose = |fills: &mut Fills, at: i64| {
            assert_eq!(fills.ask(Some(&shared), true, asked, Mode::Fill, at), Answer::Window);
            fills.waiting(at).unwrap()["serial"].as_i64().unwrap() as i32
        };

        let serial = choose(&mut fills, now);
        assert_eq!(fills.take(Some(&shared), asked, now), Answer::Waiting, "not answered yet");
        assert!(!fills.chosen(serial + 1, "a", now), "another request's answer");
        assert!(!fills.chosen(serial, "", now), "no login named");
        assert!(fills.chosen(serial, "a", now));
        assert!(!fills.chosen(serial, "b", now) && !fills.cancelled(serial, now), "answered once");
        assert!(fills.waiting(now).is_none());
        let taken = fills.take(Some(&shared), "https://FORUM.example.net:443/inbox#top", now + 30);
        assert_eq!(call(&taken), fill_call("https://forum.example.net", "jane", "p\"a'ss</script>\u{2028}", ""), "held to the origin asked, not the page's now");
        assert_eq!(fills.take(Some(&shared), asked, now + 31), Answer::Nothing, "one use");

        let late = choose(&mut fills, now);
        assert!(fills.chosen(late, "a", now));
        assert_eq!(fills.take(Some(&shared), asked, now + WAITS + 1), Answer::Said(tr().text("site-screen-fill-late", None)));
        assert_eq!(fills.take(Some(&shared), asked, now + WAITS + 2), Answer::Nothing, "gone, unused");

        for elsewhere in ["https://look-alike.example.org/sign-in", "https://forum.example.net:8443/sign-in", "http://forum.example.net/sign-in", "https://evil.example.org@forum.example.net.example.org/", "about:blank", ""] {
            let serial = choose(&mut fills, now);
            assert!(fills.chosen(serial, "a", now));
            assert_eq!(fills.take(Some(&shared), elsewhere, now + 1), Answer::Said(tr().text("site-screen-fill-changed", None)), "{elsewhere}");
            assert_eq!(fills.take(Some(&shared), asked, now + 2), Answer::Nothing, "{elsewhere}: the ticket gone");
        }

        let locked_meanwhile = Shared::default();
        for vault in [Some(&locked_meanwhile), None] {
            let serial = choose(&mut fills, now);
            assert!(fills.chosen(serial, "b", now));
            assert_eq!(fills.take(vault, asked, now + 1), Answer::Said(tr().text("site-screen-fill-locked", None)));
            assert_eq!(fills.take(Some(&shared), asked, now + 2), Answer::Nothing);
        }

        let serial = choose(&mut fills, now);
        assert!(fills.chosen(serial, "gone", now));
        assert_eq!(fills.take(Some(&shared), asked, now + 1), Answer::Said(tr().text("bitwarden-none", None)), "a login no longer in the vault");

        let serial = choose(&mut fills, now);
        assert!(!fills.cancelled(serial + 1, now));
        assert!(fills.cancelled(serial, now));
        assert_eq!(fills.take(Some(&shared), asked, now + 1), Answer::Nothing, "cancelled: nothing, nothing said");

        let serial = choose(&mut fills, now);
        assert!(fills.chosen(serial, "a", now));
        let again = choose(&mut fills, now + 5);
        assert_ne!(again, serial, "each request its own number");
        assert_eq!(fills.take(Some(&shared), asked, now + 6), Answer::Waiting, "the older ticket dropped by the new question");

        let stale = Fills { serial: 7, request: Some(Request { serial: 7, url: asked.into(), mode: Mode::Choose, at: now }), ticket: None, unanswered: false };
        assert!(stale.waiting(now + WAITS + 1).is_none(), "a request older than two minutes is not begun by a window starting later");
    }

    /// A card never outlives its request (review of 22 November 2026,
    /// finding 8). A request waits two minutes from the tap, its seconds
    /// left told to the window; then no login is taken for it, and the
    /// window's cancel brings the site back, which says once that none was
    /// chosen in time. A screen that goes while its fill waits has its
    /// request and any login chosen for it dropped ("forget"); the window
    /// is told only when a request was waiting.
    #[test]
    fn a_request_no_longer_waiting_closes_its_card() {
        sioul_core::config::stand_ins_only();
        let shared = open(&[("a", "Forum A", "jane", "pass-a", "https://forum.example.net"), ("b", "Forum B", "j.doe", "pass-b", "https://forum.example.net")]);
        let asked = "https://forum.example.net/sign-in";
        let now = 3_000_000;
        let mut fills = Fills::new();
        assert_eq!(fills.ask(Some(&shared), true, asked, Mode::Fill, now), Answer::Window);
        let serial = fills.waiting(now).unwrap()["serial"].as_i64().unwrap() as i32;
        assert_eq!(fills.waiting(now + 30).unwrap()["left"], WAITS - 30, "its seconds left");
        assert_eq!(fills.waiting(now + WAITS).unwrap()["left"], 0);
        assert!(fills.waiting(now + WAITS + 1).is_none(), "two minutes over: no longer waiting");
        assert!(!fills.chosen(serial, "a", now + WAITS + 1), "no login taken for it then");
        assert!(fills.cancelled(serial, now + WAITS + 1), "the window's cancel brings the site back");
        assert_eq!(fills.take(Some(&shared), asked, now + WAITS + 2), Answer::Said(tr().text("site-screen-fill-unanswered", None)), "said once");
        assert_eq!(fills.take(Some(&shared), asked, now + WAITS + 3), Answer::Nothing);
        // Cancelled in time: nothing said.
        assert_eq!(fills.ask(Some(&shared), true, asked, Mode::Fill, now), Answer::Window);
        let serial = fills.waiting(now).unwrap()["serial"].as_i64().unwrap() as i32;
        assert!(fills.cancelled(serial, now + WAITS));
        assert_eq!(fills.take(Some(&shared), asked, now + WAITS + 1), Answer::Nothing);
        // The screen gone: the request forgotten, the window told; a cancel after it changes nothing.
        assert_eq!(fills.ask(Some(&shared), true, asked, Mode::Choose, now), Answer::Window);
        let serial = fills.waiting(now).unwrap()["serial"].as_i64().unwrap() as i32;
        assert!(fills.forget(), "a request was waiting: the window closes its card");
        assert!(fills.waiting(now).is_none());
        assert!(!fills.cancelled(serial, now) && !fills.chosen(serial, "a", now), "the card's answer to it counts for nothing");
        assert_eq!(fills.take(Some(&shared), asked, now), Answer::Nothing);
        assert!(!fills.forget(), "nothing waiting: nothing to tell the window");
        // A login chosen, the screen gone before taking it: the ticket dropped too.
        assert_eq!(fills.ask(Some(&shared), true, asked, Mode::Fill, now), Answer::Window);
        let serial = fills.waiting(now).unwrap()["serial"].as_i64().unwrap() as i32;
        assert!(fills.chosen(serial, "b", now));
        assert!(!fills.forget(), "the window had answered already");
        assert_eq!(fills.take(Some(&shared), asked, now + 1), Answer::Nothing, "no login left for another screen");
        // The question as it comes: answered by nothing.
        assert_eq!(answer("forget", "{}"), "{}");
    }

    /// The screen reads the answer as JSON: the call whole, its text the
    /// same; the window, waiting, words, nothing. Printed, a call never shows.
    #[test]
    fn the_answer_reads_as_json_and_never_prints_its_call() {
        let call = fill_call("https://bank.example.org", "jane", "s3cret\"\\</script>", "");
        let read: serde_json::Value = serde_json::from_str(&Answer::Call(call.clone()).json()).unwrap();
        assert_eq!(read["call"].as_str(), Some(call.as_str()));
        assert_eq!(serde_json::from_str::<serde_json::Value>(&Answer::Window.json()).unwrap(), serde_json::json!({ "window": true }));
        assert_eq!(serde_json::from_str::<serde_json::Value>(&Answer::Waiting.json()).unwrap(), serde_json::json!({ "waiting": true }));
        assert_eq!(serde_json::from_str::<serde_json::Value>(&Answer::Said("Bitwarden \"locked\"".into()).json()).unwrap(), serde_json::json!({ "said": "Bitwarden \"locked\"" }));
        assert_eq!(Answer::Nothing.json(), "{}");
        assert_eq!(format!("{:?}", Answer::Call(call)), "Call(…)");
    }

    /// The screen's question read as it comes (`answer`): a page not on
    /// https said at once, an unknown mode or verb, or JSON that does not
    /// read, nothing; the window not running and nothing chosen, nothing to take.
    #[test]
    fn the_screen_s_question_is_read_as_it_comes() {
        let said: serde_json::Value = serde_json::from_str(&answer("ask", r#"{"url": "http://bank.example.org/login", "mode": "fill"}"#)).unwrap();
        assert_eq!(said["said"].as_str(), Some(tr().text("site-screen-fill-https", None).as_str()));
        assert_eq!(answer("ask", r#"{"url": "https://bank.example.org/login", "mode": "everything"}"#), "{}");
        assert_eq!(answer("ask", "not json"), "{}");
        assert_eq!(answer("forget", r#"{"url": "https://bank.example.org/login"}"#), "{}");
        assert_eq!(answer("take", r#"{"url": "https://bank.example.org/login"}"#), "{}");
        assert_eq!(waiting(), "");
    }

    /// The same origin, as the screen checks it (SiteWeb.sameOrigin): https,
    /// the same host whatever its case and a user before it, the same port
    /// (443 said or not); another host, port or scheme, a host that only
    /// begins like it, or an address that is not https: not the same. The
    /// cases are those the phone's Java check reads too, from one file
    /// (review of 22 November 2026, finding 5), so that both rules hold the
    /// same; a backslash ends the host, as browsers read it.
    #[test]
    fn the_same_origin_is_the_scheme_the_host_and_the_port() {
        let cases = include_str!("../tests/same-origin.txt");
        let mut read = 0;
        for line in cases.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let fields: Vec<&str> = line.split('\t').collect();
            let &[verdict, asked, now] = fields.as_slice() else { panic!("three fields: {line:?}") };
            let address = |text: &'static str| if text == "\"\"" { "" } else { text };
            assert!(verdict == "same" || verdict == "other", "{line:?}");
            assert_eq!(same_origin(address(asked), address(now)), verdict == "same", "{line:?}");
            read += 1;
        }
        assert!(read >= 30, "{read} cases");
    }
}

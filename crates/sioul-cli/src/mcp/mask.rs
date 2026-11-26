// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What is never handed to an agent as it is (docs/ai.md, "Masking"): the
//! one-time code, password or sign-in link a message carries, IBANs, payment
//! card numbers and French social security numbers (NIR). Each number is
//! checked by its own rule (ISO 13616's mod 97, Luhn's, the NIR's key), so an
//! order number or a phone number stays as written. The window shows them all.

use serde_json::Value;
use sioul_core::codes::{self, CodeKind};
use sioul_core::words::Words;

const CODE: &str = "[code hidden]";
const LINK: &str = "[link hidden]";
const NIR: &str = "[social security number hidden]";
const PASSWORD: &str = "[This message gives a password: its words stay in Sioul's window.]";

/// The masks' own marks, as an agent may give them back: a text holding one
/// is what the agent was shown, not what the person wrote.
const MARKS: &[&str] = &[CODE, LINK, NIR, "[This message gives a password", "[IBAN …", "[card …"];

/// The first of the masks' marks a text holds, if any.
pub fn mark_in(text: &str) -> Option<&'static str> {
    MARKS.iter().copied().find(|mark| text.contains(mark))
}

/// A message's subject and text as an agent gets them, when it carries a
/// short-lived secret (`codes::detect_text`): every code hidden wherever it is
/// written, spaced or not; every link hidden, as good as the code (a reset,
/// a sign-in link, an address to confirm); a password's message kept whole
/// from the agent, as where the password sits is not known. Then the numbers
/// `text` hides. `words`: what the codes detector looks for, as the
/// configuration makes it (`Words::of`), so that a code it finds in the
/// window is hidden from the agent too.
pub fn message(words: &Words, subject: &str, body: &str) -> (String, String) {
    let found = secrets(words, subject, body);
    if found.kinds.is_empty() {
        return (text(subject), text(body));
    }
    let hide = |text: &str| {
        // Links first: a code inside one goes with it.
        let text = hide_links(text);
        found.codes.iter().fold(text, |text, code| hide_code(&text, code))
    };
    if found.kinds.contains(&CodeKind::Password) {
        let own = secrets(words, subject, "").kinds.contains(&CodeKind::Password);
        return (if own { PASSWORD.to_string() } else { self::text(&hide(subject)) }, PASSWORD.to_string());
    }
    (self::text(&hide(subject)), self::text(&hide(body)))
}

/// What a message carries: the kinds of secret, and the codes themselves.
#[derive(Default)]
struct Secrets {
    kinds: Vec<CodeKind>,
    codes: Vec<String>,
}

/// Characters `codes::detect` is given at once to find a secret that is not
/// a code (it reads 6000), and how far two of these pieces overlap: more than
/// any of its phrases.
const PIECE: usize = 5000;
const OVERLAP: usize = 200;

/// Every secret of a message. `codes::detect_text` reads a text with its
/// line breaks (a label at a line's start counts: "Bring the cake\nCode:
/// 7319"), the first 6000 characters, the first phrase it knows and the code
/// beside it, and says one kind: so each number that could be a code is
/// asked about with the words around it, in a text where codes printed apart
/// ("4 8 2 9 1 3") are joined too; and the rest is read piece by piece with
/// every digit blanked, so that a password or a link further on is not
/// hidden behind a code. A piece cut inside a line never starts one (`piece_of`).
fn secrets(words: &Words, subject: &str, body: &str) -> Secrets {
    let chars: Vec<char> = format!("{subject}\n{body}").chars().collect();
    let mut found = Secrets::default();
    let joined = squeezed(&chars);
    for text in [&chars, &joined] {
        for code in codes_in(words, text) {
            if !found.codes.contains(&code) {
                found.codes.push(code);
            }
        }
    }
    if !found.codes.is_empty() {
        found.kinds.push(CodeKind::Code);
    }
    let blank: Vec<char> = chars.iter().map(|c| if c.is_ascii_digit() { '_' } else { *c }).collect();
    let mut start = 0;
    loop {
        let end = (start + PIECE).min(blank.len());
        if let Some(kind) = codes::detect_text(&words.codes, &piece_of(&blank, start, end)).map(|s| s.kind)
            && !found.kinds.contains(&kind)
        {
            found.kinds.push(kind);
        }
        if end == blank.len() {
            break;
        }
        start = end - OVERLAP;
    }
    found
}

/// How far around a number `codes::detect` is asked about it: its phrases sit
/// up to 160 characters before a code, or 100 after it (codes.rs). A piece
/// that starts nearer is tried too, as `detect` reads only the first phrase
/// of a piece ("never share your code… your code: 482913").
const BEFORE: [usize; 4] = [250, 170, 90, 30];
const AFTER: usize = 170;

/// A stretch of a text as `codes::detect_text` is given it: cut inside a
/// line, it starts after "… ", so that a label the cut left at its start is
/// not taken for one at a line's start ("…at 10:30, bring your Password:
/// open Settings" is none, wherever the cut falls).
fn piece_of(chars: &[char], start: usize, end: usize) -> String {
    let text: String = chars[start..end].iter().collect();
    if start > 0 && !matches!(chars[start - 1], '\n' | '\r') { format!("… {text}") } else { text }
}

/// The codes a text holds, as `codes::detect_text` reads them.
fn codes_in(words: &Words, chars: &[char]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for at in (0..chars.len()).filter(|&i| could_be_code(chars, i)) {
        let end = (at + AFTER).min(chars.len());
        for before in BEFORE {
            let piece = piece_of(chars, at.saturating_sub(before), end);
            // The codes found already, hidden: the next one comes out.
            let mut piece = found.iter().fold(piece, |piece, code| hide_code(&piece, code));
            let mut new = false;
            for _ in 0..4 {
                let Some(code) = codes::detect_text(&words.codes, &piece).and_then(|s| s.code) else { break };
                piece = hide_code(&piece, &code);
                if !found.contains(&code) {
                    found.push(code);
                }
                new = true;
            }
            if new {
                break;
            }
        }
    }
    found
}

/// Whether a code could start here, as `codes::detect` takes one: four to
/// eight digits that are not a year, or three and three ("482 913"); or
/// capitals and digits as a code of letters and digits is printed ("F7G2K",
/// "AB12CD", "K7Q-2M9"), which the detector reads right after its phrase.
fn could_be_code(chars: &[char], at: usize) -> bool {
    if at > 0 && (chars[at - 1].is_alphanumeric() || chars[at - 1] == '+') {
        return false;
    }
    if mixed_at(chars, at) {
        return true;
    }
    if !chars[at].is_ascii_digit() {
        return false;
    }
    let run = chars[at..].iter().take_while(|c| c.is_ascii_digit()).count();
    let digits: String = chars[at..at + run].iter().collect();
    match run {
        3 => chars.get(at + 3).is_some_and(|c| apart(*c)) && chars.get(at + 4..at + 7).is_some_and(|w| w.iter().all(char::is_ascii_digit)),
        4 => !digits.parse::<u32>().is_ok_and(|year| (1900..=2099).contains(&year)),
        5..=8 => true,
        _ => false,
    }
}

/// A token of a letters-and-digits code's shape starts here, as the detector
/// takes one (`codes`' mixed codes): five to ten capitals, digits and single
/// hyphens, two capitals and two digits at least, a word of its own.
fn mixed_at(chars: &[char], at: usize) -> bool {
    let run = chars[at..].iter().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || **c == '-').count();
    if chars.get(at + run).is_some_and(|c| c.is_alphanumeric()) {
        return false;
    }
    let token: String = chars[at..at + run].iter().collect::<String>().trim_end_matches('-').to_string();
    let letters = token.chars().filter(char::is_ascii_uppercase).count();
    let digits = token.chars().filter(char::is_ascii_digit).count();
    (5..=10).contains(&token.chars().count()) && letters >= 2 && digits >= 2 && !token.contains("--") && !token.starts_with('-')
}

/// What may stand between the characters of a code as it is printed: a
/// space of any width, a hyphen, a dot, or the line break or tab of a table
/// made text.
fn apart(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}' | '\u{2007}' | '-' | '.' | '\n' | '\r' | '\t')
}

/// The text with short groups of digits joined when together they make four
/// to eight ("4 8 2 9 1 3", "48 29 13", "482 913"): a code printed in boxes,
/// or by a table turned into text, read as one. A longer series (a phone
/// number) stays as it is.
fn squeezed(chars: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() || (i > 0 && chars[i - 1].is_alphanumeric()) {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // The groups from here, one separator between two.
        let (mut end, mut digits, mut groups, mut short) = (i, Vec::new(), 0, true);
        loop {
            let length = chars[end..].iter().take_while(|c| c.is_ascii_digit()).count();
            digits.extend_from_slice(&chars[end..end + length]);
            short &= length <= 3;
            groups += 1;
            end += length;
            if end + 1 < chars.len() && apart(chars[end]) && chars[end + 1].is_ascii_digit() {
                end += 1;
            } else {
                break;
            }
        }
        let alone = end == chars.len() || !chars[end].is_alphanumeric();
        if groups > 1 && short && alone && (4..=8).contains(&digits.len()) {
            out.extend(digits);
        } else {
            out.extend_from_slice(&chars[i..end]);
        }
        i = end;
    }
    out
}

/// Every place `code` is written, its characters next to each other or
/// apart ("482 913", "482-913", "4 8 2 9 1 3"), as a word of its own; and
/// for a code printed with letters before it ("G-482913"), its digits alone too.
fn hide_code(text: &str, code: &str) -> String {
    let digits = code.split_once('-').map(|(_, d)| d).filter(|d| d.len() >= 4 && d.chars().all(|c| c.is_ascii_digit()));
    let text = hide_code_as(text, code);
    digits.map_or(text.clone(), |digits| hide_code_as(&text, digits))
}

fn hide_code_as(text: &str, code: &str) -> String {
    let wanted: Vec<char> = code.chars().filter(|c| c.is_alphanumeric()).collect();
    let chars: Vec<char> = text.chars().collect();
    if wanted.is_empty() {
        return text.to_string();
    }
    // Where the code written from `start` ends, if it is.
    let ends_at = |start: usize| -> Option<usize> {
        let mut at = start;
        for (n, want) in wanted.iter().enumerate() {
            if n > 0 && at < chars.len() && apart(chars[at]) && chars.get(at + 1) == Some(want) {
                at += 1;
            }
            if chars.get(at) != Some(want) {
                return None;
            }
            at += 1;
        }
        (at == chars.len() || !chars[at].is_alphanumeric()).then_some(at)
    };
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if (i == 0 || !chars[i - 1].is_alphanumeric()) && let Some(end) = ends_at(i) {
            out.push_str(CODE);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// A text with its IBANs, card numbers and social security numbers hidden;
/// the last four characters of an IBAN or a card stay, to tell them apart.
pub fn text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let starts_word = i == 0 || !chars[i - 1].is_alphanumeric();
        if starts_word && let Some((end, hidden)) = bank_key_at(&chars, i) {
            out.push_str(&hidden);
            i = end;
            continue;
        }
        if let Some(end) = signing_link_at(&chars, i) {
            out.push_str(LINK);
            i = end;
            continue;
        }
        if starts_word && let Some((end, hidden)) = iban_at(&chars, i).or_else(|| nir_at(&chars, i)).or_else(|| card_at(&chars, i)) {
            out.push_str(&hidden);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// A bank line's or a hold's key (`bank:<account>/<id>`, `hold:<account>/<id>`,
/// alone or in a movement's address) whose account is an export's own name
/// for it: its whole number said by its last four (`bank::masked_name`), as
/// a card's name is. A declared account's key names its card instead, and
/// stays whole. Returns where the account's name ends (its `/`) and what replaces it.
fn bank_key_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    let head: String = chars.get(start..start + 5)?.iter().collect();
    if head != "bank:" && head != "hold:" {
        return None;
    }
    let from = start + 5;
    let slash = chars[from..].iter().take(60).position(|c| *c == '/')? + from;
    if slash == from || chars[from..slash].iter().any(|c| matches!(c, '\n' | '\r' | '"' | '<' | '>')) {
        return None;
    }
    let account: String = chars[from..slash].iter().collect();
    let masked = sioul_core::bank::masked_name(&account);
    (masked != account).then(|| (slash, format!("{head}{masked}")))
}

/// Every text of an answer's data masked as `text` masks it: the data says
/// no more than the lines do.
pub fn json(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(text(&s)),
        Value::Array(items) => Value::Array(items.into_iter().map(json).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(key, value)| (key, json(value))).collect()),
        other => other,
    }
}

/// Every web address replaced: one with a scheme ("https://", "HTTPS://",
/// an app's own "name://"), one that starts with "www.", or a host and a path
/// ("bank.example/reset/…"), as a mail turned into text writes them.
fn hide_links(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some(end) = link_end(&chars, i) {
            out.push_str(LINK);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Where the link that starts at `i` ends, if one does: its characters up to
/// a space or a bracket, a sentence's own stop left after it.
fn link_end(chars: &[char], i: usize) -> Option<usize> {
    let starts = i == 0 || !(chars[i - 1].is_alphanumeric() || matches!(chars[i - 1], '+' | '-' | '.' | '_' | '/' | '@' | '%' | '='));
    if !starts || !link_at(&chars[i..]) {
        return None;
    }
    let mut end = i;
    while end < chars.len() && !chars[end].is_whitespace() && !matches!(chars[end], '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}') {
        end += 1;
    }
    while end > i + 1 && matches!(chars[end - 1], '.' | ',' | ';' | ':' | '!' | '?') {
        end -= 1;
    }
    Some(end)
}

/// A link that starts at `i` and opens something signed in, wherever it is
/// written (an order's "see your order" link, a meeting's link with its
/// password, a magic link): where it ends. Such a link is as good as a
/// password, in any message, whatever else it says (`carries_secret`).
fn signing_link_at(chars: &[char], i: usize) -> Option<usize> {
    let end = link_end(chars, i)?;
    let link: String = chars[i..end].iter().collect();
    carries_secret(&link).then_some(end)
}

/// The names of a link's parameters that hand a secret over, whatever their
/// value of eight characters or more: the key, the token, the signature, the
/// password. Compared lowercase, "_", "-" and "." aside; a name ending in
/// "token" or "key" ("access_token", "apikey") or naming an authorisation
/// ("auth", "authcode", "authorization"; never "author") counts too.
const SECRET_NAMES: &[&str] = &["key", "token", "otp", "sig", "signature", "jwt", "pwd", "pass", "password", "passwd", "secret", "oobcode"];

/// Names that hand a secret over only with an opaque value (`opaque`): "?login=jdupont" is a name, "?login=9fK2…" signs in.
const OPAQUE_NAMES: &[&str] = &["login", "signin", "session", "sessionid", "sid", "ticket", "magic", "code", "state", "nonce", "hash"];

/// Words of a path that say a link signs in, resets or confirms: with an
/// opaque value beside them (`opaque`), the link carries what signs in.
const SIGNING_PATHS: &[&str] = &[
    "authenticate", "authentication", "auth", "login", "log-in", "signin", "sign-in", "sign_in", "magic", "magic-link", "magiclink", "sso", "verify", "verification", "confirm",
    "confirmation", "activate", "activation", "reset", "reset-password", "password", "passwordless", "token", "otp", "unlock",
];

/// Whether a link carries what signs in whoever opens it, without the
/// message saying so: a password before its host ("ftp://user:pass@host");
/// a parameter named for a key, a token, a signature or a password whose
/// value is eight characters or more ("…/authenticate?key=…", "…/j/123?pwd=…"),
/// or for a session or a login whose value is opaque, in its query, after its
/// "#", or in a link it wraps ("?url=https%3A…%3Ftoken%3D…"); a path word that
/// signs in, resets or confirms beside an opaque value ("…/login/magic/9fK2…");
/// or a token in its path alone (a JWT, or an opaque value of 32 characters
/// or more). A plain page ("…/login", "…/orders/1234", "?author=jean-dupont",
/// a campaign's "?utm_campaign=2026-11-weekly-digest", a tracking link's
/// "?upn=…") is none.
fn carries_secret(link: &str) -> bool {
    let lower = link.to_ascii_lowercase();
    let decoded = percent_decoded(&lower);
    // A password before the host.
    let after_scheme = |text: &str| text.split_once("://").map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or("").to_string());
    if after_scheme(&lower).is_some_and(|authority| authority.split_once('@').is_some_and(|(user, _)| user.contains(':'))) {
        return true;
    }
    // Every parameter, of the link and of any link it wraps.
    let mut values = Vec::new();
    for param in decoded.split(['?', '&', '#']).skip(1).map(|p| p.trim_start_matches("amp;")) {
        let Some((name, value)) = param.split_once('=') else { continue };
        let name: String = name.chars().filter(|c| !matches!(c, '_' | '-' | '.')).collect();
        let authorises = (name.starts_with("auth") && !name.starts_with("author")) || name == "authorization";
        let strong = SECRET_NAMES.contains(&name.as_str()) || name.ends_with("token") || name.ends_with("key") || authorises || name.contains("signature");
        if (strong && value.chars().count() >= 8) || (OPAQUE_NAMES.contains(&name.as_str()) && opaque(value)) {
            return true;
        }
        // A campaign's or a tracking link's own words are no token.
        if !name.starts_with("utm") {
            values.push(value.to_string());
        }
    }
    // The path after its host.
    let path = decoded.split(['?', '#']).next().unwrap_or("");
    let path = path.split_once("://").map_or(path, |(_, rest)| rest);
    let segments: Vec<&str> = path.split('/').skip(1).filter(|s| !s.is_empty()).collect();
    if segments.iter().any(|s| jwt(s) || (opaque(s) && s.chars().count() >= 32)) {
        return true;
    }
    let signing = segments.iter().any(|s| SIGNING_PATHS.contains(s));
    signing && segments.iter().map(|s| s.to_string()).chain(values).any(|v| opaque(&v))
}

/// A link's "%XX" read once ("%3D" is "="), so that a link it wraps is read too.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = text.get(i + 1..i + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A value no person types: twenty characters or more of letters, digits,
/// "-", "_", "." and "%", with a run of eight or more that mixes letters and
/// digits ("9fK2a8Lm", never "2026-11-weekly-digest", whose parts are words and a date).
fn opaque(value: &str) -> bool {
    let mixed = value.split(['-', '_', '.', '%']).any(|run| run.chars().count() >= 8 && run.chars().all(|c| c.is_ascii_alphanumeric()) && run.chars().any(|c| c.is_ascii_digit()) && run.chars().any(|c| c.is_ascii_alphabetic()));
    value.chars().count() >= 20 && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '%' | '.')) && mixed
}

/// A JSON Web Token: three parts of base64url joined by dots, the first a JSON object's ("eyJ").
fn jwt(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 3 && parts[0].starts_with("eyj") && parts.iter().all(|p| p.len() >= 4 && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')))
}

/// Whether a link starts the text.
fn link_at(rest: &[char]) -> bool {
    let lower: String = rest.iter().take(4).map(char::to_ascii_lowercase).collect();
    if lower == "www." {
        return true;
    }
    if !rest.first().is_some_and(char::is_ascii_alphanumeric) {
        return false;
    }
    // A scheme: a letter, then letters, digits, "+", "-" or ".", then "://".
    let scheme = rest.iter().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')).count();
    if rest[0].is_ascii_alphabetic() && rest.get(scheme..scheme + 3) == Some(&[':', '/', '/'][..]) {
        return true;
    }
    // A host and a path: names and dots, a top-level name of letters, then "/".
    let host: String = rest.iter().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.')).collect();
    let top = host.rsplit('.').next().unwrap_or("");
    rest.get(host.chars().count()) == Some(&'/') && host.contains('.') && top.len() >= 2 && top.chars().all(|c| c.is_ascii_alphabetic())
}

/// What may stand between the groups of an account or a card number: a
/// space, of any width (HTML mail turned into text has non-breaking ones).
fn separator(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}' | '\u{2007}')
}

/// Runs of `allowed` characters from `start`, a single space (or hyphen,
/// when `hyphens`) allowed between them: each place a run could end, with the
/// characters kept so far. A place is kept only where no letter or digit follows.
fn runs(chars: &[char], start: usize, allowed: impl Fn(char) -> bool, hyphens: bool, most: usize) -> Vec<(usize, String)> {
    let mut ends = Vec::new();
    let mut kept = String::new();
    let mut i = start;
    while i < chars.len() && kept.chars().count() < most {
        if allowed(chars[i]) {
            kept.push(chars[i]);
            i += 1;
            if i == chars.len() || !chars[i].is_alphanumeric() {
                ends.push((i, kept.clone()));
            }
        } else if (separator(chars[i]) || (hyphens && chars[i] == '-')) && i + 1 < chars.len() && allowed(chars[i + 1]) && !kept.is_empty() {
            i += 1;
        } else {
            break;
        }
    }
    ends
}

/// An IBAN starting at `start`, written whole or in groups: ISO 13616, its
/// check digits by mod 97. The longest form that checks out is taken.
fn iban_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    let head: Vec<char> = chars.get(start..start + 4)?.to_vec();
    if !(head[0].is_ascii_uppercase() && head[1].is_ascii_uppercase() && head[2].is_ascii_digit() && head[3].is_ascii_digit()) {
        return None;
    }
    let ends = runs(chars, start, |c| c.is_ascii_uppercase() || c.is_ascii_digit(), false, 34);
    ends.into_iter().rev().filter(|(_, kept)| kept.len() >= 15).find(|(_, kept)| iban_checks(kept)).map(|(end, kept)| (end, format!("[IBAN …{}]", &kept[kept.len() - 4..])))
}

fn iban_checks(iban: &str) -> bool {
    let moved = iban[4..].chars().chain(iban[..4].chars());
    let mut rest: u32 = 0;
    for c in moved {
        let value = match c {
            '0'..='9' => c as u32 - '0' as u32,
            'A'..='Z' => c as u32 - 'A' as u32 + 10,
            _ => return false,
        };
        rest = if value >= 10 { (rest * 100 + value) % 97 } else { (rest * 10 + value) % 97 };
    }
    rest == 1
}

/// A payment card number, Luhn's check digit right: 16 or 19 digits from a
/// network's first digit (2 to 6), or 15 from American Express's 34 and 37.
/// Fourteen digits are left alone: a SIRET has a Luhn key too, and is no secret.
fn card_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    if !matches!(chars.get(start), Some('2'..='6')) {
        return None;
    }
    let shaped = |digits: &str| match digits.len() {
        16 | 19 => true,
        15 => digits.starts_with("34") || digits.starts_with("37"),
        _ => false,
    };
    let ends = runs(chars, start, |c| c.is_ascii_digit(), true, 19);
    ends.into_iter().rev().filter(|(_, digits)| shaped(digits)).find(|(_, digits)| luhn(digits)).map(|(end, digits)| (end, format!("[card …{}]", &digits[digits.len() - 4..])))
}

fn luhn(digits: &str) -> bool {
    let sum: u32 = digits
        .chars()
        .rev()
        .filter_map(|c| c.to_digit(10))
        .enumerate()
        .map(|(i, d)| if i % 2 == 1 { if d * 2 > 9 { d * 2 - 9 } else { d * 2 } } else { d })
        .sum();
    sum % 10 == 0
}

/// A French social security number (NIR): 15 characters, the first 1, 2, 7
/// or 8, the last two its key, 97 less the first thirteen modulo 97; Corsica's
/// departments, 2A and 2B, count as 19 and 18 in the key.
fn nir_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    if !matches!(chars.get(start), Some('1' | '2' | '7' | '8')) {
        return None;
    }
    let ends = runs(chars, start, |c| c.is_ascii_digit() || c == 'A' || c == 'B', false, 15);
    let (end, kept) = ends.into_iter().find(|(_, kept)| kept.len() == 15)?;
    let digits = match &kept[5..7] {
        "2A" => format!("{}19{}", &kept[..5], &kept[7..]),
        "2B" => format!("{}18{}", &kept[..5], &kept[7..]),
        _ => kept,
    };
    let number: u64 = digits[..13].parse().ok()?;
    let key: u64 = digits[13..].parse().ok()?;
    (97 - number % 97 == key).then(|| (end, NIR.to_string()))
}

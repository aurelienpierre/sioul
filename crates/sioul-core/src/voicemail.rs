// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Voicemail the operator sends by e-mail (docs/android.md, "Calls";
//! docs/research/call-screening.md, 5.4). Free Mobile, set to, mails each
//! message left on your voicemail: "un email vous notifiant le numéro
//! appelant, la date et la durée du message", and with the sound, "en pièce
//! jointe … sous forme de fichier son (.wav)" (assistance.free.fr, article
//! 940; the Freebox line likewise, article 572). Its exact words, sender and
//! file name are not published: read tolerantly. A message is one when it
//! comes from Free's domains and either carries a sound, or says voicemail
//! with a caller (or a hidden one) and a length; the caller's number, when
//! the message was left and how long it lasts are read from its subject and
//! text, the time from its Date when the text gives none.
//!
//! Each is then linked to the call Sioul refused (`link`): the same number,
//! left within half an hour after it. Sioul plays its sound on demand
//! (`reading::attachment`); the mail itself stays where the Porch puts it.
//! Nothing is transcribed (a later option: research 5.3, CS18).

use crate::calls::Held;
use crate::phones::{self, Region};
use mail_parser::{MessageParser, MimeHeaders};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Free's own domains: Free Mobile, the Freebox, their mail servers.
pub const FREE_DOMAINS: [&str; 4] = ["free-mobile.fr", "free.fr", "freebox.fr", "proxad.net"];
/// A voicemail is linked to a call refused at most this long before it (seconds).
pub const LINK_BEFORE: i64 = 30 * 60;
/// … or this long after it: clocks differ (seconds).
pub const LINK_AFTER: i64 = 3 * 60;

/// A voicemail, as its mail says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voicemail {
    pub path: PathBuf,
    /// The caller's number as written ("01 99 00 12 34"); "" when hidden or not said.
    pub number: String,
    /// `phones::key` of it; "" when hidden or not said.
    pub key: String,
    /// The caller hid their number ("numéro masqué").
    pub hidden: bool,
    /// When the message was left (Unix seconds): the mail's words, else its Date.
    pub at: i64,
    /// How long it lasts, when said.
    pub seconds: Option<u32>,
    /// The sound's place among the mail's attachments; none in a notice without it.
    pub sound: Option<u32>,
}

/// Whether an address is Free's own (a subdomain too).
pub fn from_free(address: &str) -> bool {
    let address = address.trim().to_ascii_lowercase();
    let Some((_, domain)) = address.rsplit_once('@') else { return false };
    FREE_DOMAINS.iter().any(|d| domain == *d || domain.ends_with(&format!(".{d}")))
}

/// A quick look at a mail's head before reading it whole: a "From" with
/// Free's name in it, on its line or the lines that continue it. Voicemail
/// mails carry their sound: most mail never is.
pub fn may_be(raw: &[u8]) -> bool {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| raw.windows(2).position(|w| w == b"\n\n")).unwrap_or(raw.len().min(16 * 1024));
    let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
    let mut in_from = false;
    for line in head.lines() {
        if !line.starts_with([' ', '\t']) {
            in_from = line.starts_with("from:");
        }
        if in_from && (line.contains("free") || line.contains("proxad")) {
            return true;
        }
    }
    false
}

/// Accents and case aside, for the words looked for.
fn folded(text: &str) -> String {
    crate::text::fold(text).into_iter().collect::<String>().to_lowercase()
}

/// The voicemail a mail is, if it is one. `zone` reads the time its text
/// gives (French time, as Free writes it); `region`, numbers without a
/// country; `trusted`, the authserv-ids of the provider that delivered it
/// (`config::Source::trusted_ids`): a mail its own checks find forged in
/// Free's name is never one.
pub fn read(raw: &[u8], path: &Path, region: Option<&Region>, zone: &jiff::tz::TimeZone, trusted: &[String]) -> Option<Voicemail> {
    let message = MessageParser::default().parse(raw)?;
    let from = message.from()?.first()?.address()?.to_ascii_lowercase();
    if !from_free(&from) {
        return None;
    }
    let auth = crate::trust::read_auth_results(&crate::headers::RawHeaders::parse(raw), trusted);
    let (trust, _) = crate::trust::judge_sender(auth.as_ref(), false, from.rsplit_once('@').map(|(_, d)| d));
    if trust == crate::trust::Trust::Forged {
        return None;
    }
    let subject = message.subject().unwrap_or("").to_string();
    let mut body = message.body_text(0).map(|b| b.into_owned()).unwrap_or_default();
    if body.trim().is_empty()
        && let Some(html) = message.body_html(0)
    {
        body = mail_parser::decoders::html::html_to_text(&html);
    }
    let found = message.attachments().enumerate().find(|(_, part)| is_sound(part)).map(|(i, part)| (i as u32, part.attachment_name().unwrap_or("").to_string()));
    let sound = found.as_ref().map(|(i, _)| *i);
    let text = format!("{subject}\n{body}");
    let words = folded(&text);
    let says_voicemail = ["message vocal", "messages vocaux", "messagerie vocale", "repondeur", "nouveau message", "voicemail", "voice message"].iter().any(|w| words.contains(w));
    let hidden = said_hidden(&words);
    // The subject, the text, else the sound's own name ("0199001234_20261006.wav").
    let number = caller(&subject, region).or_else(|| caller(&body, region)).or_else(|| found.as_ref().and_then(|(_, name)| caller(name, region)));
    let seconds = duration(&words);
    // A notice without its sound counts when it says voicemail, who, and how long.
    if sound.is_none() && !(says_voicemail && (number.is_some() || hidden) && seconds.is_some()) {
        return None;
    }
    if sound.is_some() && !says_voicemail && number.is_none() && !hidden {
        return None;
    }
    let at = left_at(&body, zone).or_else(|| left_at(&subject, zone)).or_else(|| message.date().map(|d| d.to_timestamp()))?;
    let (number, key) = match number {
        Some((written, key)) if !hidden || !key.is_empty() => (written, key),
        _ => (String::new(), String::new()),
    };
    Some(Voicemail { path: path.to_path_buf(), hidden: key.is_empty() && hidden, number, key, at, seconds, sound })
}

/// An attachment that is a sound: an audio type, or a sound file's name.
fn is_sound(part: &mail_parser::MessagePart) -> bool {
    let by_type = part.content_type().is_some_and(|t| t.ctype().eq_ignore_ascii_case("audio"));
    let by_name = part.attachment_name().is_some_and(|n| {
        let n = n.to_ascii_lowercase();
        [".wav", ".mp3", ".ogg", ".amr", ".m4a", ".opus"].iter().any(|e| n.ends_with(e))
    });
    by_type || by_name
}

/// The caller hid their number, as the mail says it.
fn said_hidden(words: &str) -> bool {
    ["numero masque", "numero prive", "numero inconnu", "appelant inconnu", "appel masque", "anonyme", "hidden number", "withheld", "private number", "unknown caller", "unknown number"].iter().any(|w| words.contains(w))
}

/// The caller's number in a text: the numbers written in it, the one after
/// words that name a caller first ("de la part du", "appelant", "from"), never
/// one after words that name your own line ("votre ligne", "your number").
fn caller(text: &str, region: Option<&Region>) -> Option<(String, String)> {
    let mut found: Vec<(i32, String, String)> = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let starts = c.is_ascii_digit() || (c == '+' && chars.get(i + 1).is_some_and(char::is_ascii_digit));
        if !starts {
            i += 1;
            continue;
        }
        let begin = i;
        let mut digits = 0;
        let mut end = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || (i == begin && chars[i] == '+') || matches!(chars[i], ' ' | '.' | '-' | '\u{a0}' | '\u{202f}')) {
            if chars[i].is_ascii_digit() {
                digits += 1;
                end = i + 1;
            }
            i += 1;
        }
        if !(9..=15).contains(&digits) {
            continue;
        }
        let written: String = chars[begin..end].iter().collect::<String>().trim().to_string();
        // A date, a time, a length: never a number (they hold "/" or ":", which end the run).
        let key = phones::key(&written, region);
        // A number as a country writes it (its international form), never a run of
        // digits no plan reads (a file's time stamp); without a country known, a number's length.
        if !phones::is_whole(&key) || !(key.starts_with('+') || (region.is_none() && (9..=12).contains(&digits))) {
            continue;
        }
        let before: String = folded(&chars[begin.saturating_sub(32)..begin].iter().collect::<String>());
        let score = if ["part du", "part de", "appelant", "appele par", "from", "caller", "numero :", "numero:", "de :", "de:", " du ", " de "].iter().any(|w| before.contains(w)) { 2 } else { 1 };
        let score = if ["votre ligne", "votre numero", "sur la ligne", "your line", "your number"].iter().any(|w| before.contains(w)) { 0 } else { score };
        found.push((score, written, key));
    }
    found.into_iter().enumerate().max_by_key(|(order, (score, _, _))| (*score, -(*order as i64))).filter(|(_, (score, _, _))| *score > 0).map(|(_, (_, written, key))| (written, key))
}

/// When the message was left, as the text writes it: "06/10/2026 à 09:31",
/// "le 06/10/26 à 9h31", "2026-10-06 09:31:05". In `zone`.
fn left_at(text: &str, zone: &jiff::tz::TimeZone) -> Option<i64> {
    let chars: Vec<char> = text.chars().collect();
    let number_at = |from: usize| -> Option<(i64, usize)> {
        let mut end = from;
        while end < chars.len() && chars[end].is_ascii_digit() && end - from < 4 {
            end += 1;
        }
        (end > from).then(|| (chars[from..end].iter().collect::<String>().parse::<i64>().unwrap_or(-1), end))
    };
    let skip = |mut at: usize, words: &[&str]| -> usize {
        loop {
            while at < chars.len() && matches!(chars[at], ' ' | ',' | '\u{a0}' | '\u{202f}') {
                at += 1;
            }
            let rest: String = folded(&chars[at..chars.len().min(at + 6)].iter().collect::<String>());
            match words.iter().find(|w| rest.starts_with(**w)) {
                Some(w) => at += w.chars().count(),
                None => return at,
            }
        }
    };
    for start in 0..chars.len() {
        if !chars[start].is_ascii_digit() || (start > 0 && chars[start - 1].is_ascii_digit()) {
            continue;
        }
        let Some((first, after_first)) = number_at(start) else { continue };
        let sep = chars.get(after_first).copied();
        let (year, month, day, after_date) = match sep {
            Some('/') => {
                let Some((month, after_month)) = number_at(after_first + 1) else { continue };
                if chars.get(after_month) != Some(&'/') {
                    continue;
                }
                let Some((year, after_year)) = number_at(after_month + 1) else { continue };
                (if year < 100 { 2000 + year } else { year }, month, first, after_year)
            }
            Some('-') if first > 1900 => {
                let Some((month, after_month)) = number_at(after_first + 1) else { continue };
                if chars.get(after_month) != Some(&'-') {
                    continue;
                }
                let Some((day, after_day)) = number_at(after_month + 1) else { continue };
                (first, month, day, after_day)
            }
            _ => continue,
        };
        let at_time = skip(after_date, &["a ", "at ", "-", "t"]);
        let Some((hour, after_hour)) = number_at(at_time) else { continue };
        let sep = chars.get(after_hour).copied();
        if !matches!(sep, Some(':') | Some('h') | Some('H')) {
            continue;
        }
        let (minute, after_minute) = number_at(after_hour + 1).unwrap_or((0, after_hour + 1));
        let second = if chars.get(after_minute) == Some(&':') { number_at(after_minute + 1).map_or(0, |(s, _)| s) } else { 0 };
        let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) = (i16::try_from(year), i8::try_from(month), i8::try_from(day), i8::try_from(hour), i8::try_from(minute), i8::try_from(second)) else { continue };
        let (Ok(civil), Ok(time)) = (jiff::civil::Date::new(year, month, day), jiff::civil::Time::new(hour, minute, second, 0)) else { continue };
        if let Ok(zoned) = civil.to_datetime(time).to_zoned(zone.clone()) {
            return Some(zoned.timestamp().as_second());
        }
    }
    None
}

/// How long the message lasts: "Durée : 42 secondes", "durée du message :
/// 0:42", "1 min 05 s", "duration: 42 s". Read after the word that says it.
fn duration(words: &str) -> Option<u32> {
    let at = ["duree", "duration", "length", "longueur"].iter().filter_map(|w| words.find(w).map(|i| i + w.len())).min()?;
    let rest: String = words[at..].chars().take(48).collect();
    let rest = rest.trim_start_matches(|c: char| !c.is_ascii_digit());
    let mut numbers: Vec<(u32, String)> = Vec::new();
    let mut chars = rest.chars().peekable();
    while numbers.len() < 3 {
        let mut digits = String::new();
        while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
            digits.push(c);
            chars.next();
        }
        if digits.is_empty() {
            break;
        }
        let mut unit = String::new();
        while let Some(c) = chars.peek().copied() {
            if c.is_ascii_digit() {
                break;
            }
            unit.push(c);
            chars.next();
            if unit.trim().len() > 10 {
                break;
            }
        }
        numbers.push((digits.parse().ok()?, unit.trim().to_string()));
        if !(unit.trim().is_empty() || unit.trim() == ":" || unit.trim().starts_with("min") || unit.trim().starts_with('h') || unit.trim() == "m") {
            break;
        }
    }
    match numbers.as_slice() {
        [(minutes, sep), (seconds, _), ..] if sep == ":" => Some(minutes * 60 + seconds),
        [(minutes, unit), (seconds, _), ..] if unit.starts_with("min") || unit == "m" => Some(minutes * 60 + seconds),
        [(minutes, unit), ..] if unit.starts_with("min") => Some(minutes * 60),
        [(seconds, unit), ..] if unit.starts_with('s') || unit.is_empty() => Some(*seconds),
        _ => None,
    }
}

/// Each voicemail linked to the call it followed: the same number (or both
/// hidden), left at most half an hour after the call refused (a few minutes
/// before it too: clocks differ); the nearest first, one call each.
pub fn link(held: &[Held], mails: &[Voicemail]) -> BTreeMap<String, usize> {
    let mut linked: BTreeMap<String, usize> = BTreeMap::new();
    let mut order: Vec<usize> = (0..mails.len()).collect();
    order.sort_by_key(|i| mails[*i].at);
    for i in order {
        let mail = &mails[i];
        let candidate = held
            .iter()
            .filter(|h| !linked.contains_key(&h.id()))
            .filter(|h| if mail.key.is_empty() { h.hidden } else { !h.hidden && h.key == mail.key })
            .filter(|h| {
                let call = h.at / 1000;
                mail.at >= call - LINK_AFTER && mail.at - call <= LINK_BEFORE
            })
            .min_by_key(|h| (mail.at - h.at / 1000).abs());
        if let Some(h) = candidate {
            linked.insert(h.id(), i);
        }
    }
    linked
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAV: &str = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAIA+AAACABAAZGF0YQAAAAA=";

    /// A mail as Free might send it: `from`, `subject`, a text body, a sound or none.
    fn mail(from: &str, subject: &str, body: &str, sound: Option<(&str, &str)>) -> Vec<u8> {
        let mut raw = format!("From: {from}\r\nTo: someone@example.org\r\nSubject: {subject}\r\nDate: Tue, 06 Oct 2026 09:33:00 +0200\r\nMIME-Version: 1.0\r\n");
        match sound {
            Some((kind, name)) => {
                raw.push_str("Content-Type: multipart/mixed; boundary=\"b1\"\r\n\r\n--b1\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n");
                raw.push_str(body);
                raw.push_str(&format!("\r\n--b1\r\nContent-Type: {kind}; name=\"{name}\"\r\nContent-Disposition: attachment; filename=\"{name}\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{WAV}\r\n--b1--\r\n"));
            }
            None => {
                raw.push_str("Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n");
                raw.push_str(body);
                raw.push_str("\r\n");
            }
        }
        raw.into_bytes()
    }

    fn paris() -> jiff::tz::TimeZone {
        jiff::tz::TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> i64 {
        text.parse::<jiff::Zoned>().unwrap().timestamp().as_second()
    }

    fn read_it(raw: &[u8]) -> Option<Voicemail> {
        read(raw, Path::new("/mail/cur/1"), phones::region_named("FR"), &paris(), &["mx.example.net".to_string()])
    }

    #[test]
    fn free_s_voicemail_with_its_sound() {
        let raw = mail(
            "Free Mobile <messagerie@free-mobile.fr>",
            "Nouveau message vocal de 01 99 00 12 34",
            "Bonjour,\r\nVous avez reçu un nouveau message vocal sur votre ligne 06 39 98 00 01 de la part du 01 99 00 12 34 le 06/10/2026 à 09:31.\r\nDurée du message : 42 secondes.\r\n",
            Some(("audio/x-wav", "message.wav")),
        );
        assert!(may_be(&raw));
        let v = read_it(&raw).unwrap();
        assert_eq!((v.number.as_str(), v.key.as_str(), v.hidden), ("01 99 00 12 34", "+33199001234", false));
        assert_eq!(v.at, at("2026-10-06T09:31[Europe/Paris]"));
        assert_eq!((v.seconds, v.sound), (Some(42), Some(0)));
    }

    #[test]
    fn read_tolerantly_whatever_the_words() {
        // The number in the text only, after the subscriber's own line; the length as 0:42; the time with seconds.
        let raw = mail(
            "Messagerie Vocale <voicemail@mobile.free.fr>",
            "Vous avez un nouveau message",
            "Votre ligne : 06 39 98 00 01\nAppelant : +33 1 99 00 56 78\nDate : 06/10/2026 09:40:05\nDurée : 0:42\n",
            Some(("application/octet-stream", "0199005678_20261006094005.wav")),
        );
        let v = read_it(&raw).unwrap();
        assert_eq!(v.key, "+33199005678");
        assert_eq!(v.at, at("2026-10-06T09:40:05[Europe/Paris]"));
        assert_eq!((v.seconds, v.sound), (Some(42), Some(0)));
        // A hidden caller, minutes and seconds, the time as "9h05", the date in the subject's year form.
        let raw = mail("Free <noreply@free.fr>", "Message vocal", "Un message vocal de numéro masqué le 07/10/26 à 9h05. Durée : 1 min 05 s.", Some(("audio/wav", "message.wav")));
        let v = read_it(&raw).unwrap();
        assert!(v.hidden && v.key.is_empty() && v.number.is_empty());
        assert_eq!(v.at, at("2026-10-07T09:05[Europe/Paris]"));
        assert_eq!(v.seconds, Some(65));
        // No time in the words: the mail's Date.
        let raw = mail("Free Mobile <messagerie@free-mobile.fr>", "Message vocal de 0199004321", "Écoutez la pièce jointe.", Some(("audio/x-wav", "m.wav")));
        let v = read_it(&raw).unwrap();
        assert_eq!((v.key.as_str(), v.at, v.seconds), ("+33199004321", at("2026-10-06T09:33[Europe/Paris]"), None));
        // A notice without its sound ("simple" mode): who, when, how long; nothing to play.
        let raw = mail("Free Mobile <messagerie@free-mobile.fr>", "Nouveau message vocal", "Nouveau message vocal de la part du 01 99 00 12 34 le 06/10/2026 à 09:31. Durée : 12 s.", None);
        let v = read_it(&raw).unwrap();
        assert_eq!((v.key.as_str(), v.seconds, v.sound), ("+33199001234", Some(12), None));
        // The number in the sound's name only, after its time stamp: the stamp is no number.
        let raw = mail("Free Mobile <messagerie@free-mobile.fr>", "Nouveau message vocal", "Vous avez un nouveau message.", Some(("audio/x-wav", "20261006093100_0199005555.wav")));
        assert_eq!(read_it(&raw).unwrap().key, "+33199005555");
        // English words, a numeric date the other way round.
        let raw = mail("Free Mobile <voicemail@free-mobile.fr>", "New voicemail", "You have a new voice message from +33 4 65 71 12 34 on 2026-10-06 at 09:31. Duration: 42 s", Some(("audio/x-wav", "vm.wav")));
        let v = read_it(&raw).unwrap();
        assert_eq!((v.key.as_str(), v.at, v.seconds), ("+33465711234", at("2026-10-06T09:31[Europe/Paris]"), Some(42)));
    }

    #[test]
    fn other_mail_is_not_voicemail() {
        // Forged in Free's name, as the provider's own checks say: never.
        let forged = mail(
            "Free Mobile <messagerie@free-mobile.fr>\r\nAuthentication-Results: mx.example.net; dmarc=fail (p=reject) header.from=free-mobile.fr",
            "Nouveau message vocal de 01 99 00 12 34",
            "Durée : 42 secondes.",
            Some(("audio/x-wav", "message.wav")),
        );
        assert!(read_it(&forged).is_none());
        // Not Free's: never, whatever it says or carries.
        let raw = mail("Free Mobile <messagerie@free-mobile.example>", "Nouveau message vocal de 01 99 00 12 34", "Durée : 42 secondes.", Some(("audio/x-wav", "message.wav")));
        assert!(read_it(&raw).is_none());
        assert!(!from_free("x@freemobile.fr") && from_free("a@free-mobile.fr") && from_free("a@smtp.free.fr") && !from_free("a@notfree.fr"));
        // Free's invoice, its code changed: no sound, no caller with a length.
        let raw = mail("Free Mobile <noreply@free-mobile.fr>", "Votre facture Free Mobile", "Votre facture de 19,99 € est disponible. Ligne 06 39 98 00 01.", None);
        assert!(read_it(&raw).is_none());
        let raw = mail("Free Mobile <noreply@free-mobile.fr>", "Votre code de messagerie vocale a été modifié", "Le code de votre messagerie vocale a été modifié le 06/10/2026 à 09:31.", None);
        assert!(read_it(&raw).is_none());
        // The quick look reads the head only; a From folded on two lines is one.
        assert!(!may_be(b"From: Jane <jane@example.org>\r\nSubject: free tickets\r\n\r\nfree\r\n"));
        assert!(may_be(b"From: =?UTF-8?Q?Messagerie_vocale?=\r\n <messagerie@free-mobile.fr>\r\nSubject: x\r\n\r\nbody\r\n"));
        assert!(!may_be(b"From: Jane\r\n <jane@example.org>\r\nX-Note: free\r\n\r\nbody\r\n"));
    }

    fn held(at: i64, key: &str) -> Held {
        Held { at: at * 1000, key: key.into(), hidden: key.is_empty(), who: if key.is_empty() { "hidden".into() } else { "stranger".into() }, ..Held::default() }
    }

    fn voicemail(at: i64, key: &str) -> Voicemail {
        Voicemail { path: PathBuf::from("/m"), number: key.into(), key: key.into(), hidden: key.is_empty(), at, seconds: Some(42), sound: Some(0) }
    }

    #[test]
    fn each_voicemail_goes_with_the_call_it_followed() {
        let t = at("2026-10-06T09:30[Europe/Paris]");
        let calls = vec![held(t, "+33199001234"), held(t + 600, "+33199001234"), held(t + 60, ""), held(t + 120, "+33465711234")];
        let mails = vec![voicemail(t + 30, "+33199001234"), voicemail(t + 700, "+33199001234"), voicemail(t + 90, ""), voicemail(t + 4 * 3600, "+33465711234")];
        let linked = link(&calls, &mails);
        assert_eq!(linked.get(&calls[0].id()), Some(&0));
        assert_eq!(linked.get(&calls[1].id()), Some(&1), "the second call has the second message");
        assert_eq!(linked.get(&calls[2].id()), Some(&2), "hidden with hidden");
        assert_eq!(linked.get(&calls[3].id()), None, "four hours later is another call's");
        // A number that never called: nothing linked.
        assert!(link(&calls, &[voicemail(t, "+33199009999")]).is_empty());
    }
}

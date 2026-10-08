// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The tools that read what the person's phone shares with their computers,
//! read-only: the phone's messages (`phonemsgs`: who wrote, in which app and
//! conversation, when, and the words where the phone sent them) and the
//! calls it screened (`calls`: who called, when, whether it rang or went to
//! voicemail). Read from the logs the sharing carries to this computer; none
//! when the parts "Messages from your phone" and "Calls" are off.
//!
//! A code or an approval never left the phone: its line says that one came,
//! and so does this answer, never a code; what the words still hold of one
//! is masked as in mail. A message the phone held (do-not-disturb, the
//! matrix of what reaches you) is not given before the phone lets it
//! through. Others' words come as data, framed. What concerns a contact the
//! person keeps from agents (a closed project's client), and all of it while
//! things outside projects are closed, is left out and counted (`access`).

use super::access;
use super::mask;
use super::tools::{Answer, Args};
use crate::{Session, one_line};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use serde_json::json;
use sioul_core::consent::Consent;
use sioul_core::links::{self, Loaded};
use sioul_core::{calls, phonemsgs, phones};
use std::collections::BTreeSet;

/// A moment in milliseconds, as the logs keep them, in the person's time zone.
fn instant_ms(ms: i64) -> String {
    Timestamp::from_millisecond(ms).map(|t| t.to_zoned(TimeZone::system()).strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()).unwrap_or_default()
}

/// A moment in milliseconds, as a line says it.
fn said_ms(s: &Session, ms: i64) -> String {
    crate::date(s, Some(ms.div_euclid(1000)))
}

/// The people kept from agents: their names and their numbers' keys.
struct Kept {
    names: BTreeSet<String>,
    keys: BTreeSet<String>,
}

impl Kept {
    fn of(s: &Session, loaded: &Loaded, consent: &Consent) -> Kept {
        let region = sioul_core::reach::region(&s.config);
        let mut kept = Kept { names: BTreeSet::new(), keys: BTreeSet::new() };
        for contact in loaded.contacts.iter().filter(|c| !consent.allows(&links::contact_uri(&c.uid))) {
            kept.names.insert(contact.name.to_lowercase());
            kept.keys.extend(contact.phones.iter().map(|p| phones::key(&p.value, region)).filter(|k| !k.is_empty()));
        }
        kept
    }

    fn holds(&self, name: &str, key: &str) -> bool {
        (!key.is_empty() && self.keys.contains(key)) || (!name.trim().is_empty() && self.names.contains(&name.trim().to_lowercase()))
    }
}

/// The contact's name for a number's key, as the person's address books know it.
fn known_name(s: &Session, loaded: &Loaded, key: &str) -> Option<String> {
    let region = sioul_core::reach::region(&s.config);
    (!key.is_empty()).then(|| loaded.contacts.iter().find(|c| c.phones.iter().any(|p| phones::key(&p.value, region) == key)).map(|c| c.name.clone())).flatten()
}

/// The words of a query, folded: case and accents aside.
fn folded(text: &str) -> String {
    sioul_core::text::fold(text).into_iter().collect()
}

pub fn phone_messages(s: &Session, args: &Args) -> Result<Answer, String> {
    let query = args.text("query")?.unwrap_or_default();
    let since = args.date("since")?;
    let limit = args.number("limit", 50, 1, 500)? as usize;
    let now_ms = Zoned::now().timestamp().as_millisecond();
    let from_ms = since.and_then(|d| d.to_zoned(TimeZone::system()).ok()).map_or(i64::MIN, |z| z.timestamp().as_millisecond());
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    let kept = Kept::of(s, &loaded, &consent);
    let all = phonemsgs::read_logs(&phonemsgs::folder(), now_ms);
    let in_scope: Vec<&phonemsgs::Line> = all.iter().filter(|l| l.at >= from_ms).collect();
    // Held by the phone until later: not given before the person may see it.
    let waiting = in_scope.iter().filter(|l| l.shows > now_ms).count();
    let ready: Vec<&phonemsgs::Line> = in_scope.into_iter().filter(|l| l.shows <= now_ms).collect();
    // Kept from agents: everything while things outside projects are closed; a kept contact's.
    let (open, closed): (Vec<&phonemsgs::Line>, Vec<&phonemsgs::Line>) = ready.into_iter().partition(|l| consent.outside && !kept.holds(&l.name, &l.key));
    let closed = closed.len();
    let looked = sioul_core::words::Words::of(&s.config);
    let words: Vec<String> = query.split_whitespace().map(folded).collect();
    // The words as the agent is shown them, masked: a code is not found by its digits.
    let shown_text = |l: &phonemsgs::Line| if l.code || l.withheld { String::new() } else { mask::message(&looked, "", &l.text).1 };
    let mut found: Vec<&phonemsgs::Line> = open
        .into_iter()
        .filter(|l| {
            let haystack = folded(&format!("{} {} {} {}", l.name, l.title, if l.label.is_empty() { &l.app } else { &l.label }, shown_text(l)));
            words.iter().all(|w| haystack.contains(w.as_str()))
        })
        .collect();
    found.sort_by_key(|l| std::cmp::Reverse(l.at));
    let total = found.len();
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for l in found.into_iter().take(limit) {
        let app = if l.label.is_empty() { l.app.clone() } else { l.label.clone() };
        let who = if !l.name.is_empty() {
            l.name.clone()
        } else {
            known_name(s, &loaded, &l.key).unwrap_or_else(|| calls::shown_number(&l.key, sioul_core::reach::region(&s.config)))
        };
        let place = if l.group && !l.title.is_empty() { format!(" in “{}”", l.title) } else { String::new() };
        let words = if l.code {
            "(A code or an approval came: it stays on the phone, never given.)".to_string()
        } else if l.withheld {
            "(Its words stayed on the phone: its app sends who and when only.)".to_string()
        } else {
            let text = shown_text(l);
            let picture = if l.picture { " [a picture, kept on the phone]" } else { "" };
            format!("{}{picture}", text.split_whitespace().collect::<Vec<_>>().join(" "))
        };
        lines.push(format!("{} · {app} · {who}{place}", said_ms(s, l.at)));
        lines.push(format!("    {words}"));
        out.push(json!({
            "at": instant_ms(l.at), "app": app, "kind": l.kind, "from": who, "conversation": l.title, "group": l.group, "text": if l.code || l.withheld { None } else { Some(shown_text(l)) },
            "picture": l.picture, "code_came": l.code, "withheld": l.withheld, "phone": l.device,
        }));
    }
    // Their words, each on one line, inside a frame made for this answer: whatever they say, they cannot end it.
    let lines: Vec<String> = lines.iter().map(|l| if l.starts_with("    ") { format!("    {}", one_line(l.trim_start())) } else { one_line(l) }).collect();
    let mut text = if lines.is_empty() { Vec::new() } else { super::read::framed("The phone's messages, as their senders wrote them,", &lines.join("\n")) };
    if out.is_empty() {
        text.push(if all.is_empty() { "No message from a phone here: the phone shares them once its part “Messages from your phone” is on.".into() } else { "No message from a phone matches.".into() });
    } else if total > limit {
        text.push(format!("… {} more", total - limit));
    }
    if waiting > 0 {
        text.push(format!("({waiting} more came while the phone held them; they are given once the phone lets them through, as the person sees them.)"));
    }
    text.extend(access::left_out(closed));
    let text = text.join("\n");
    Ok(Answer { text, data: json!({ "messages": out, "total": total, "waiting": waiting, "kept_from_agents": closed }) })
}

pub fn calls(s: &Session, args: &Args) -> Result<Answer, String> {
    let since = args.date("since")?;
    let limit = args.number("limit", 50, 1, 500)? as usize;
    let now_ms = Zoned::now().timestamp().as_millisecond();
    let from_ms = since.and_then(|d| d.to_zoned(TimeZone::system()).ok()).map_or(i64::MIN, |z| z.timestamp().as_millisecond());
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    let kept = Kept::of(s, &loaded, &consent);
    let region = sioul_core::reach::region(&s.config);
    let all = calls::read_logs(&calls::folder(), now_ms);
    let in_scope: Vec<&calls::Held> = all.iter().filter(|h| h.at >= from_ms).collect();
    let (open, closed): (Vec<&calls::Held>, Vec<&calls::Held>) = in_scope.into_iter().partition(|h| consent.outside && !kept.holds(&h.name, &h.key));
    let closed = closed.len();
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for h in open.iter().rev().take(limit) {
        let who = if h.hidden {
            "a hidden number".to_string()
        } else {
            known_name(s, &loaded, &h.key).or_else(|| Some(h.name.clone()).filter(|n| !n.is_empty())).unwrap_or_else(|| calls::shown_number(&h.key, region))
        };
        let number = if h.hidden { String::new() } else { calls::shown_number(&h.key, region) };
        let what = if h.rang { "rang" } else { "declined, sent to voicemail" };
        let shown = if number.is_empty() || number == who { who.clone() } else { format!("{who} ({number})") };
        lines.push(format!("{} · {shown} · {what}", said_ms(s, h.at)));
        out.push(json!({ "at": instant_ms(h.at), "from": who, "number": number, "hidden": h.hidden, "rang": h.rang, "row": h.who, "time": h.column, "verified": h.verified, "phone": h.device }));
    }
    if out.is_empty() {
        lines.push(if all.is_empty() { "No call here: a phone of the person's lists the calls it screens once its part “Calls” is on.".into() } else { "No call then.".into() });
    } else if open.len() > limit {
        lines.push(format!("… {} more", open.len() - limit));
    }
    lines.extend(access::left_out(closed));
    let text = lines.iter().map(|l| one_line(l)).collect::<Vec<_>>().join("\n");
    Ok(Answer { text, data: json!({ "calls": out, "total": open.len(), "kept_from_agents": closed }) })
}

// Texts (docs/texts.md): read, searched, and drafted, behind `[mcp] texts`.

/// The tools that read or draft texts, which `[mcp] texts` keeps from agents unless it is true.
pub const TEXTS: [&str; 3] = ["texts", "search_texts", "draft_text"];

/// What opens the texts sealed at rest on this computer: the sharing key,
/// read from the keyring now, while `[mcp] texts` is true, and only for this.
/// It never reaches the agent. Tests use a key of their own, never a keyring.
fn sealer() -> Result<sioul_sync::textseal::TextSeal, String> {
    #[cfg(test)]
    return Ok(sioul_sync::textseal::TextSeal::new(&super::tests::TEXTS_KEY));
    #[cfg(not(test))]
    {
        let unhex = |text: &str| -> Option<[u8; 32]> {
            let bytes: Option<Vec<u8>> = (0..text.len()).step_by(2).map(|i| text.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
            <[u8; 32]>::try_from(bytes?).ok()
        };
        sioul_sync::secret::named(sioul_sync::share::KEY_NAME)
            .and_then(|key| unhex(&key))
            .map(|key| sioul_sync::textseal::TextSeal::new(&key))
            .ok_or_else(|| "Sioul cannot open the texts here: this computer holds no sharing key. The person shares texts from their phone first (Settings ▸ Your folder and sharing).".to_string())
    }
}

/// The texts as an agent may read them: each one's words with its codes
/// masked (as in mail: never given, never found by their digits), those of a
/// conversation kept from agents left out, and how many were.
struct Shelf {
    texts: Vec<sioul_core::texts::Text>,
    requests: Vec<sioul_core::texts::Request>,
    outcomes: Vec<sioul_core::texts::Outcome>,
    kept: usize,
    any: bool,
    loaded: Loaded,
}

impl Shelf {
    fn read(s: &Session, seal: &dyn sioul_core::texts::Sealer) -> Shelf {
        use sioul_core::texts;
        let root = texts::folder();
        let loaded = Loaded::read(&s.config);
        let consent = access::of(s, &loaded);
        let kept_people = Kept::of(s, &loaded, &consent);
        let looked = sioul_core::words::Words::of(&s.config);
        let masked = |body: &str| if body.is_empty() { String::new() } else { mask::message(&looked, "", body).1 };
        let all = texts::read_logs(&root, seal);
        let any = !all.is_empty();
        let open = |with: &[String]| consent.outside && !with.iter().any(|k| kept_people.holds("", k));
        let (open_texts, closed): (Vec<texts::Text>, Vec<texts::Text>) = all.into_iter().partition(|t| open(&t.with));
        let texts: Vec<texts::Text> = open_texts
            .into_iter()
            .map(|t| {
                let parts = t.parts.iter().map(|p| texts::Part { text: masked(&p.text), ..p.clone() }).collect();
                texts::Text { body: masked(&t.body), parts, fields: Default::default(), addrs: Vec::new(), ..t }
            })
            .collect();
        let region = sioul_core::reach::region(&s.config);
        let requests = texts::read_requests(&root, seal).into_iter().filter(|r| open(&[phones::key(&r.to, region)])).map(|r| texts::Request { body: masked(&r.body), ..r }).collect();
        Shelf { texts, requests, outcomes: texts::read_outcomes(&root, seal), kept: closed.len(), any, loaded }
    }
}

/// What a conversation's page needs besides the lines: names from the address books, no media opened.
fn viewer<'a>(s: &'a Session, now: &'a Zoned, name_of: &'a dyn Fn(&str) -> Option<String>, media_here: &'a dyn Fn(&str) -> bool) -> sioul_core::texts::Viewer<'a> {
    sioul_core::texts::Viewer { now, tr: &s.tr, region: sioul_core::reach::region(&s.config), name_of, phone_shared: None, media_here }
}

/// A message on its lines: when, who, its words, its parts said, its state.
fn message_lines(m: &sioul_core::texts::Message, title: &str) -> Vec<String> {
    let who = if m.direction == "out" { "You".to_string() } else if m.from.is_empty() { title.to_string() } else { m.from.clone() };
    let mut head = format!("{} · {who}", m.when);
    if m.deleted {
        head.push_str(" · (deleted on the phone, kept here)");
    }
    if let Some(said) = &m.said {
        head.push_str(&format!(" · {}", said.text));
    }
    let mut lines = vec![one_line(&head)];
    let words = m.body.split_whitespace().collect::<Vec<_>>().join(" ");
    if !words.is_empty() {
        lines.push(format!("    {}", one_line(&words)));
    }
    for part in &m.parts {
        lines.push(format!("    [{}]", one_line(&part.said)));
    }
    lines
}

pub fn texts(s: &Session, args: &Args) -> Result<Answer, String> {
    use sioul_core::texts;
    let wanted = args.text("conversation")?;
    let limit = args.number("limit", 30, 1, 500)? as usize;
    let seal = sealer()?;
    let shelf = Shelf::read(s, &seal);
    let now = Zoned::now();
    let region = sioul_core::reach::region(&s.config);
    let name_of = |key: &str| known_name(s, &shelf.loaded, key);
    let media_here = |_: &str| false;
    let v = viewer(s, &now, &name_of, &media_here);
    let conversations = texts::conversations(&shelf.texts, &shelf.requests, &v);
    let mut lines = Vec::new();
    let data;
    match wanted {
        None => {
            for c in conversations.iter().take(limit) {
                lines.push(format!("{} · {}{}  [{}]", c.when, c.title, if c.group { " (a group)" } else { "" }, c.id));
                let last = c.last.split_whitespace().collect::<Vec<_>>().join(" ");
                if !last.is_empty() {
                    lines.push(format!("    {last}"));
                }
            }
            data = json!({ "conversations": conversations.iter().take(limit).collect::<Vec<_>>(), "total": conversations.len(), "kept_from_agents": shelf.kept });
            if conversations.len() > limit {
                lines.push(format!("… {} more", conversations.len() - limit));
            }
        }
        Some(asked) => {
            // By its id ("+33199001234", several joined by ","), or a number of it.
            let id = if conversations.iter().any(|c| c.id == asked) { asked.clone() } else { texts::conversation_id(&asked.split(',').map(|n| phones::key(n, region)).collect::<Vec<_>>()) };
            let Some(talk) = conversations.iter().find(|c| c.id == id) else {
                return Err(if shelf.kept > 0 { access::kept("This conversation, if it is one,") } else { format!("No conversation “{}”: `texts` lists them, with their ids.", one_line(&asked)) });
            };
            let all = texts::messages(&id, &shelf.texts, &shelf.requests, &shelf.outcomes, &v);
            let shown: Vec<&texts::Message> = all.iter().rev().take(limit).rev().collect();
            for m in &shown {
                lines.extend(message_lines(m, &talk.title));
            }
            if all.len() > limit {
                lines.insert(0, format!("(the {limit} newest of {})", all.len()));
            }
            let drafts: Vec<sioul_core::textdraft::TextDraft> = sioul_core::textdraft::all(&seal).into_iter().filter(|d| d.conversation == id).collect();
            data = json!({ "conversation": talk, "messages": shown, "drafts_waiting": drafts.len(), "kept_from_agents": shelf.kept });
            if !drafts.is_empty() {
                lines.push(format!("({} draft{} written by an agent wait{} in the Texts page for the person.)", drafts.len(), if drafts.len() == 1 { "" } else { "s" }, if drafts.len() == 1 { "s" } else { "" }));
            }
        }
    }
    let mut text = if lines.is_empty() { Vec::new() } else { super::read::framed("The texts, as their people wrote them,", &lines.join("\n")) };
    if lines.is_empty() {
        text.push(if shelf.any { "No texts to give here.".into() } else { "No texts here: this computer keeps them once the phone shares the part “Texts”.".into() });
    }
    text.extend(access::left_out(shelf.kept));
    Ok(Answer { text: text.join("\n"), data })
}

pub fn search_texts(s: &Session, args: &Args) -> Result<Answer, String> {
    use sioul_core::texts;
    let query = args.needed("query")?;
    let limit = args.number("limit", 20, 1, 200)? as usize;
    let seal = sealer()?;
    let shelf = Shelf::read(s, &seal);
    let now = Zoned::now();
    let name_of = |key: &str| known_name(s, &shelf.loaded, key);
    let media_here = |_: &str| false;
    let v = viewer(s, &now, &name_of, &media_here);
    // In the words as the agent is given them, masked: a code is never found by its digits.
    let found = texts::search(&query, &shelf.texts, &shelf.requests, &shelf.outcomes, &v, limit);
    let mut lines = Vec::new();
    for f in &found {
        lines.push(format!("{}  [{}]", one_line(&f.title), f.conversation));
        lines.extend(message_lines(&f.message, &f.title).into_iter().map(|l| format!("  {l}")));
    }
    let mut text = if lines.is_empty() { vec![format!("No text holds “{}”.", one_line(&query))] } else { super::read::framed("The texts found, as their people wrote them,", &lines.join("\n")) };
    text.extend(access::left_out(shelf.kept));
    Ok(Answer { text: text.join("\n"), data: json!({ "found": found, "kept_from_agents": shelf.kept }) })
}

pub fn draft_text(s: &Session, args: &Args) -> Result<Answer, String> {
    use sioul_core::texts;
    let to = args.needed_line("to")?;
    let body = args.body("body")?;
    let region = sioul_core::reach::region(&s.config);
    if to.contains([',', ';']) {
        return Err("A draft goes to one person: Sioul does not send texts to a group from a computer.".into());
    }
    if let Some(why) = texts::refusal(&to, &body, region) {
        return Err(match why {
            "empty" => "“body” is needed: the words of the text.".to_string(),
            "too-long" => format!("A text drafted here holds at most {} characters.", texts::WRITE_MAX),
            "short-number" => format!("“{}” is no number a text can go to from a computer (a short number is refused).", one_line(&to)),
            other => format!("This text cannot be drafted: {other}."),
        });
    }
    let key = phones::key(&to, region);
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    if !consent.outside || Kept::of(s, &loaded, &consent).holds("", &key) {
        return Err(access::kept("This conversation"));
    }
    let seal = sealer()?;
    let draft = sioul_core::textdraft::TextDraft { id: texts::new_key(), to: key.clone(), conversation: texts::conversation_id(std::slice::from_ref(&key)), body, written: Zoned::now().timestamp().as_millisecond(), by: String::new() };
    sioul_core::textdraft::add(&draft, &seal)?;
    let who = known_name(s, &loaded, &key).unwrap_or_else(|| calls::shown_number(&key, region));
    let text = format!(
        "Saved as a draft in the Texts page, in the conversation with {}  [{}]\n(Not sent: the person reads it there, then uses it and sends it themselves, or discards it.)",
        one_line(&who),
        draft.conversation
    );
    Ok(Answer { text, data: json!({ "id": draft.id, "conversation": draft.conversation, "to": who, "sent": false }) })
}

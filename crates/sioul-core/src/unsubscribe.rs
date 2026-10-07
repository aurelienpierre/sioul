// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Leaving a mailing list in one click (docs/client.md, "Unsubscribing").
//!
//! A list's message says how to leave it in `List-Unsubscribe` (RFC 2369): web
//! addresses, mail addresses. When it also carries `List-Unsubscribe-Post:
//! List-Unsubscribe=One-Click` and a valid DKIM signature covers both headers
//! (RFC 8058 §3.1), an HTTPS POST does it, nothing else asked. Else a message
//! to the list's address does it, sent as any message is; else only a web page
//! can, and it opens in the browser.
//!
//! Nothing is contacted for mail set aside (forged, spam, a borrowed name, a
//! blocked sender) or hostile, nor when nothing proves the address it names
//! is the list's own: a valid signature of the domain shown in From (the
//! message verified), or a valid signature covering `List-Unsubscribe` by the
//! domain the address belongs to. A forged message could otherwise make Sioul
//! post to, or write to, anyone. What was done is kept on this device
//! (`Record`), so that a list is not asked twice and you can see it.

use crate::card::Card;
use crate::headers::RawHeaders;
use crate::i18n::Translator;
use crate::porch::{Lane, Reason};
use crate::trust::{self, Trust};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The value of `List-Unsubscribe-Post` that asks for one click (RFC 8058 §3.1).
pub const ONE_CLICK: &str = "List-Unsubscribe=One-Click";

/// What one click does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Way {
    /// An HTTPS POST of `List-Unsubscribe=One-Click` to this address (RFC 8058).
    OneClick { url: String },
    /// A message to the list's address, with the subject and text its link asks for.
    Mail { to: String, subject: String, body: String },
    /// Only a web page: it opens in the browser, where you finish.
    Page { url: String },
}

impl Way {
    /// "one-click", "mail", "page": as the record and the window name it.
    pub fn id(&self) -> &'static str {
        match self {
            Way::OneClick { .. } => "one-click",
            Way::Mail { .. } => "mail",
            Way::Page { .. } => "page",
        }
    }
}

/// Why one click does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Forged: DMARC failed under a policy that says so.
    Forged,
    /// Flagged as spam: answering tells its sender your address is read.
    Spam,
    /// It borrows a brand's name, or one of your own domains.
    Borrowed,
    /// You blocked its sender.
    Blocked,
    /// Hostile mail to a shielded address.
    Hostile,
    /// Nothing proves the address it names is its list's own.
    Unproven,
    /// Two `List-Unsubscribe` headers, or two `List-Unsubscribe-Post`: Sioul does not choose.
    Ambiguous,
    /// No web or mail address Sioul can use (an IP address, a local name, several recipients).
    Unusable,
}

impl Refusal {
    fn id(self) -> &'static str {
        match self {
            Refusal::Forged => "forged",
            Refusal::Spam => "spam",
            Refusal::Borrowed => "borrowed",
            Refusal::Blocked => "blocked",
            Refusal::Hostile => "hostile",
            Refusal::Unproven => "unproven",
            Refusal::Ambiguous => "ambiguous",
            Refusal::Unusable => "unusable",
        }
    }
}

/// The list a message comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    /// What names it for good, lower case: its List-Id (RFC 2919), else the sender's address.
    pub key: String,
    /// What to call it: List-Id's name, else the sender's name, else the address.
    pub name: String,
}

/// What a message offers to leave its list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub list: List,
    pub way: Result<Way, Refusal>,
}

/// How to leave the message's list, or why one click does nothing; none when
/// it names no way out (`List-Unsubscribe`). `trust`, `lane` and `reasons`
/// are the Porch's verdict on it (`porch::triage`); `trusted_ids` the
/// authserv-ids of its account (Sioul's own, then the provider's).
pub fn offer(card: &Card, trusted_ids: &[String], trust: Trust, lane: &Lane, reasons: &[Reason]) -> Option<Offer> {
    let headers = &card.headers;
    let values: Vec<&str> = headers.all("List-Unsubscribe").collect();
    let first = *values.first()?;
    let list = list_of(card);
    Some(Offer { list, way: way(card, headers, &values, first, trusted_ids, trust, lane, reasons) })
}

#[allow(clippy::too_many_arguments)]
fn way(card: &Card, headers: &RawHeaders, values: &[&str], first: &str, trusted_ids: &[String], trust: Trust, lane: &Lane, reasons: &[Reason]) -> Result<Way, Refusal> {
    // What the Porch set aside is never answered.
    if *lane == Lane::Hostile || reasons.contains(&Reason::Hostile) {
        return Err(Refusal::Hostile);
    }
    if reasons.contains(&Reason::Blocked) {
        return Err(Refusal::Blocked);
    }
    if trust == Trust::Forged || reasons.contains(&Reason::Forged) {
        return Err(Refusal::Forged);
    }
    if reasons.iter().any(|r| matches!(r, Reason::Impersonation { .. })) {
        return Err(Refusal::Borrowed);
    }
    if reasons.iter().any(Reason::is_spam) || *lane == Lane::SetAside {
        return Err(Refusal::Spam);
    }
    if values.len() > 1 || headers.all("List-Unsubscribe-Post").count() > 1 {
        return Err(Refusal::Ambiguous);
    }
    let uris = uris(first);
    let signed = signatures(headers, trusted_ids);
    let from = card.sender_domain().unwrap_or("").to_ascii_lowercase();
    let verified = trust == Trust::Verified;
    // The address it names is its list's: the message is verified, or the
    // domain the address belongs to signed `List-Unsubscribe`.
    let proven = |domain: &str| verified || signed.iter().any(|s| s.covers("list-unsubscribe") && trust::aligned(&s.domain, domain));
    let web = |scheme: &str| uris.iter().find(|u| u.get(..scheme.len()).is_some_and(|s| s.eq_ignore_ascii_case(scheme))).cloned();
    let https = web("https://");
    // RFC 8058: one POST, when the sender's domain, or the address's, signed both headers.
    let one_click = headers.first("List-Unsubscribe-Post").is_some_and(|v| v.trim().eq_ignore_ascii_case(ONE_CLICK));
    if one_click
        && let Some(url) = https.as_deref()
        && let Some(host) = host_of(url)
        && signed.iter().any(|s| s.covers("list-unsubscribe") && s.covers("list-unsubscribe-post") && (trust::aligned(&s.domain, &from) || trust::aligned(&s.domain, &host)))
    {
        return Ok(Way::OneClick { url: url.to_string() });
    }
    let mut unproven = false;
    if let Some(uri) = uris.iter().find(|u| crate::mailto::is_mailto(u)) {
        match by_mail(uri) {
            Some(way @ Way::Mail { .. }) if proven(domain_of_address(&way)) => return Ok(way),
            Some(_) => unproven = true,
            None => {}
        }
    }
    if let Some(url) = https.or_else(|| web("http://"))
        && let Some(host) = host_of(&url)
    {
        if proven(&host) {
            return Ok(Way::Page { url });
        }
        unproven = true;
    }
    Err(if unproven { Refusal::Unproven } else { Refusal::Unusable })
}

/// The domain a `Way::Mail` writes to.
fn domain_of_address(way: &Way) -> &str {
    match way {
        Way::Mail { to, .. } => to.rsplit_once('@').map_or("", |(_, d)| d),
        _ => "",
    }
}

/// The message a `mailto:` address asks for: one recipient (a list's own
/// address), its subject and text, "unsubscribe" where it says none. Copies
/// it asks for (`cc`, `bcc`) are not sent: one click writes to the list alone.
fn by_mail(uri: &str) -> Option<Way> {
    let asked = crate::mailto::parse(uri)?;
    let [to] = asked.to.as_slice() else { return None };
    let to = crate::compose::address_of(to)?.to_ascii_lowercase();
    let (_, domain) = to.rsplit_once('@')?;
    if !public_host(domain) {
        return None;
    }
    let or_unsubscribe = |text: &str| if text.trim().is_empty() { "unsubscribe".to_string() } else { text.to_string() };
    Some(Way::Mail { subject: or_unsubscribe(&asked.subject), body: or_unsubscribe(&asked.body), to })
}

/// The addresses of `List-Unsubscribe`, in order: each between angle brackets,
/// spaces inside taken out (RFC 2369 §2), comments left aside.
pub fn uris(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = value;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else { break };
        let uri: String = rest[open + 1..open + close].chars().filter(|c| !c.is_whitespace()).collect();
        if !uri.is_empty() {
            out.push(uri);
        }
        rest = &rest[open + close + 1..];
    }
    out
}

/// The host of a web address, lower case; none when it is not a name on the
/// Internet (an IP address, "localhost", a local network's name): one click
/// reaches nothing on your own network.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    // Credentials put before the host to mislead are refused: "https://bank.example@evil.example".
    if authority.contains('@') {
        return None;
    }
    let host = authority.rsplit_once(':').filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit())).map_or(authority, |(host, _)| host).trim_end_matches('.').to_ascii_lowercase();
    public_host(&host).then_some(host)
}

/// A name on the Internet: letters, digits, hyphens and dots, two labels at
/// least, the last of letters; never an IP address nor a local name.
fn public_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    let top = labels.last().copied().unwrap_or("");
    let local = ["localhost", "local", "lan", "internal", "home", "arpa", "localdomain", "invalid", "test"].contains(&top);
    labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && top.len() >= 2
        && top.chars().all(|c| c.is_ascii_alphabetic())
        && !local
}

/// The list a message comes from: its List-Id ("Type & Pixels
/// <letter.typeandpixels.example>"), else its sender.
pub fn list_of(card: &Card) -> List {
    let address = card.from_address.clone().unwrap_or_default().to_ascii_lowercase();
    let sender = card.from_name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| address.clone());
    let Some(id) = card.headers.first("List-Id") else { return List { key: address, name: sender } };
    let (name, key) = match (id.rfind('<'), id.rfind('>')) {
        (Some(open), Some(close)) if open < close => (id[..open].trim().trim_matches('"').trim().to_string(), id[open + 1..close].trim().to_ascii_lowercase()),
        _ => (String::new(), id.trim().to_ascii_lowercase()),
    };
    if key.is_empty() {
        return List { key: address, name: sender };
    }
    // A List-Id's name is often a raw label ("newsletter"): the sender's name reads better.
    let name = if name.is_empty() { sender } else { name };
    List { key: format!("list-id:{key}"), name }
}

/// A DKIM signature that passed, with the headers it covers (its `h=` tag).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Signed {
    domain: String,
    /// Lower case; only the headers every signature matching the result covers.
    covers: Vec<String>,
}

impl Signed {
    fn covers(&self, header: &str) -> bool {
        self.covers.iter().any(|h| h == header)
    }
}

/// The signatures that passed, as the trusted results header records them
/// (`trust::dkim_passes`), each with what it covers, read from the
/// DKIM-Signature header it names (domain, selector, start of the signature).
/// When several headers match one result (a copy, altered), a header counts as
/// covered only if all of them cover it: a copy can only take away.
fn signatures(headers: &RawHeaders, trusted_ids: &[String]) -> Vec<Signed> {
    let all: Vec<Vec<(String, String)>> = headers.all("DKIM-Signature").map(tags).collect();
    let tag = |tags: &[(String, String)], name: &str| tags.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).unwrap_or_default();
    trust::dkim_passes(headers, trusted_ids)
        .into_iter()
        .filter_map(|pass| {
            let matching: Vec<&Vec<(String, String)>> = all
                .iter()
                .filter(|t| trust::aligned(&tag(t, "d"), &pass.domain))
                .filter(|t| pass.selector.as_ref().is_none_or(|s| tag(t, "s").eq_ignore_ascii_case(s)))
                .filter(|t| pass.b.as_ref().is_none_or(|b| tag(t, "b").starts_with(b.as_str())))
                .collect();
            let first = matching.first()?;
            let covered = |t: &Vec<(String, String)>| tag(t, "h").split(':').map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()).collect::<Vec<_>>();
            let covers: Vec<String> = covered(first).into_iter().filter(|h| matching.iter().all(|t| covered(t).contains(h))).collect();
            Some(Signed { domain: tag(first, "d").to_ascii_lowercase(), covers })
        })
        .collect()
}

/// A DKIM-Signature header's tags (RFC 6376 §3.2), names lower case, the
/// spaces of each value taken out (they fold a long `b=` or `h=`).
fn tags(value: &str) -> Vec<(String, String)> {
    value
        .split(';')
        .filter_map(|tag| {
            let (name, value) = tag.split_once('=')?;
            Some((name.trim().to_ascii_lowercase(), value.chars().filter(|c| !c.is_whitespace()).collect()))
        })
        .collect()
}

/// The lists you left, kept on this device (`$XDG_STATE_HOME/sioul/unsubscribed.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    #[serde(default, rename = "list")]
    pub lists: Vec<Left>,
}

/// One list left, or whose page was opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Left {
    /// The list's `key` (`List`).
    pub key: String,
    pub name: String,
    /// The sender's address.
    #[serde(default)]
    pub address: String,
    /// "one-click", "mail" (done), "page" (its page opened: you finish there).
    pub way: String,
    /// When, as Unix seconds.
    pub at: i64,
}

impl Left {
    /// Done by Sioul (one click, a message), not only a page opened.
    pub fn done(&self) -> bool {
        self.way != "page"
    }
}

impl Record {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("unsubscribed.toml")
    }

    /// The record; empty when the file does not exist or cannot be read.
    pub fn load(path: &Path) -> Record {
        std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    /// The newest entry for a list.
    pub fn find(&self, key: &str) -> Option<&Left> {
        self.lists.iter().filter(|l| l.key == key).max_by_key(|l| l.at)
    }

    /// Adds an entry, the list's older ones replaced; written beside, then moved.
    pub fn add(path: &Path, left: Left) -> Result<(), String> {
        crate::filelock::with_lock(path, || {
            let mut record = Record::load(path);
            // A page opened later does not undo a list left.
            if !left.done() && record.find(&left.key).is_some_and(Left::done) {
                return Ok(());
            }
            record.lists.retain(|l| l.key != left.key);
            record.lists.push(left);
            let fail = |e: std::io::Error| format!("{}: {e}", path.display());
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(fail)?;
            }
            let text = toml::to_string(&record).map_err(|e| e.to_string())?;
            let temporary = path.with_extension("toml.new");
            std::fs::write(&temporary, text).and_then(|()| std::fs::rename(&temporary, path)).map_err(fail)
        })
    }
}

/// The Reader's button, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnsubscribeView {
    /// One click acts now; else it only says why it does not (`tip`).
    pub offered: bool,
    /// "one-click", "mail", "page", or "" when nothing is offered.
    pub way: &'static str,
    /// The button's name.
    pub label: String,
    /// What one click does, why it does nothing, or when it was done.
    pub tip: String,
    /// Left already: the button rests.
    pub done: bool,
}

/// The button for a message's offer. `sends_from` is the address a message
/// goes out from (the account the message came to), none when it cannot send.
pub fn view(offer: &Offer, record: &Record, sends_from: Option<&str>, tr: &Translator, today: jiff::civil::Date) -> UnsubscribeView {
    let name = offer.list.name.clone();
    let say = |id: &str, pairs: &[(&str, String)]| {
        let mut args = crate::i18n::args();
        args.set("list", name.clone());
        for (key, value) in pairs {
            args.set(key.to_string(), value.clone());
        }
        tr.text(id, Some(&args))
    };
    let day = |at: i64| jiff::Timestamp::from_second(at).map(|t| tr.day_in(t.to_zoned(jiff::tz::TimeZone::system()).date(), today)).unwrap_or_default();
    let label = tr.text("unsubscribe-label", None);
    let past = record.find(&offer.list.key);
    if let Some(left) = past.filter(|l| l.done()) {
        let way = tr.text(&format!("unsubscribe-way-{}", left.way), None);
        return UnsubscribeView { offered: false, way: "", label: tr.text("unsubscribe-done-label", None), tip: say("unsubscribe-done", &[("date", day(left.at)), ("way", way)]), done: true };
    }
    let refused = |id: &str| UnsubscribeView { offered: false, way: "", label: label.clone(), tip: tr.text(&format!("unsubscribe-not-{id}"), None), done: false };
    match &offer.way {
        Err(refusal) => refused(refusal.id()),
        Ok(Way::OneClick { url }) => UnsubscribeView { offered: true, way: "one-click", label: label.clone(), tip: say("unsubscribe-tip-one-click", &[("host", host_of(url).unwrap_or_default())]), done: false },
        Ok(Way::Mail { to, .. }) => match sends_from {
            Some(from) => UnsubscribeView { offered: true, way: "mail", label: label.clone(), tip: say("unsubscribe-tip-mail", &[("to", to.clone()), ("from", from.to_string())]), done: false },
            None => refused("no-sender"),
        },
        Ok(Way::Page { url }) => {
            let mut tip = say("unsubscribe-tip-page", &[("host", host_of(url).unwrap_or_default())]);
            if let Some(opened) = past {
                tip = format!("{tip} {}", say("unsubscribe-page-opened", &[("date", day(opened.at))]));
            }
            UnsubscribeView { offered: true, way: "page", label: label.clone(), tip, done: false }
        }
    }
}

/// The message that asks a list to stop, plain text, from your address.
pub fn request(from: (&str, &str), to: &str, subject: &str, body: &str, date: i64) -> Result<crate::compose::Outgoing, String> {
    let domain = from.1.rsplit_once('@').map_or("localhost", |(_, d)| d);
    let message_id = format!("{}@{domain}", crate::compose::new_message_id());
    let raw = mail_builder::MessageBuilder::new()
        .from(from)
        .to(to)
        .subject(subject)
        .message_id(message_id.as_str())
        .date(date)
        .text_body(body)
        .write_to_vec()
        .map_err(|e| e.to_string())?;
    Ok(crate::compose::Outgoing { raw, from: from.1.to_string(), recipients: vec![to.to_string()] })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIOUL: &str = "sioul-test.invalid";

    /// A newsletter as a sending service writes it, signed by `signer` over `signed`, Sioul's checks on top.
    fn letter(extra: &str, signer: &str, signed: &str, dmarc: &str) -> Card {
        let raw = format!(
            "Authentication-Results: {SIOUL};\r\n\tdkim=pass header.d={signer} header.s=s1 header.b=QUJDREVG;\r\n\tspf=pass smtp.mailfrom=bounce.{signer}; dmarc={dmarc} header.from=letters.example\r\n\
             DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed; d={signer}; s=s1;\r\n\th={signed};\r\n\tbh=Zm9v; b=QUJDREVG\r\n\t SElKS0xN\r\n\
             From: Type & Pixels <letter@letters.example>\r\nTo: you@example.org\r\nSubject: Issue 112\r\n\
             List-Id: Type & Pixels <issues.letters.example>\r\n{extra}Content-Type: text/plain\r\n\r\nThis week.\r\n"
        );
        Card::from_bytes(raw.as_bytes()).unwrap()
    }

    fn judged(card: &Card) -> Option<Offer> {
        let ids = vec![SIOUL.to_string()];
        let auth = trust::read_auth_results(&card.headers, &ids);
        let (trust, _) = trust::judge_sender(auth.as_ref(), card.is_list, card.sender_domain());
        offer(card, &ids, trust, &Lane::Filed, &[Reason::Newsletter])
    }

    const BOTH: &str = "from:to:subject:date:list-unsubscribe:list-unsubscribe-post";
    const POST: &str = "List-Unsubscribe-Post: List-Unsubscribe=One-Click\r\n";

    #[test]
    fn one_click_when_both_headers_are_signed() {
        let links = "List-Unsubscribe: <https://letters.example/u/a1b2?l=112>, <mailto:leave@letters.example?subject=stop>\r\n";
        let card = letter(&format!("{links}{POST}"), "letters.example", BOTH, "pass");
        let found = judged(&card).unwrap();
        assert_eq!(found.list, List { key: "list-id:issues.letters.example".into(), name: "Type & Pixels".into() });
        assert_eq!(found.way, Ok(Way::OneClick { url: "https://letters.example/u/a1b2?l=112".into() }));
        // Signed by the sending service, the address its own: one click too.
        let service = "List-Unsubscribe: <https://unsub.mailer.example/u/a1b2>\r\n";
        let card = letter(&format!("{service}{POST}"), "mailer.example", BOTH, "pass");
        assert_eq!(judged(&card).unwrap().way, Ok(Way::OneClick { url: "https://unsub.mailer.example/u/a1b2".into() }));
        // List-Unsubscribe-Post left out of the signature: the message instead (RFC 8058 §3.1).
        let card = letter(&format!("{links}{POST}"), "letters.example", "from:to:subject:list-unsubscribe", "pass");
        assert_eq!(judged(&card).unwrap().way, Ok(Way::Mail { to: "leave@letters.example".into(), subject: "stop".into(), body: "unsubscribe".into() }));
        // A copy of the signature that covers less: the headers count as not covered.
        let copied = format!("{links}{POST}DKIM-Signature: v=1; d=letters.example; s=s1; h=from:to; bh=Zm9v; b=QUJDREVGSElKS0xN\r\n");
        let card = letter(&copied, "letters.example", BOTH, "pass");
        assert!(matches!(judged(&card).unwrap().way, Ok(Way::Mail { .. })));
        // A second List-Unsubscribe added above the signed one: Sioul does not choose.
        let added = format!("List-Unsubscribe: <https://evil.example/collect>\r\n{links}{POST}");
        let card = letter(&added, "letters.example", BOTH, "pass");
        assert_eq!(judged(&card).unwrap().way, Err(Refusal::Ambiguous));
    }

    #[test]
    fn nothing_for_what_nothing_proves() {
        // Not verified (DMARC fails, no signature of the sender's domain): no address of it is used.
        let links = "List-Unsubscribe: <https://elsewhere.example/u>, <mailto:leave@elsewhere.example>\r\n";
        let card = letter(&format!("{links}{POST}"), "signer.example", "from:to", "fail");
        let ids = vec![SIOUL.to_string()];
        let found = offer(&card, &ids, Trust::Unverified, &Lane::Filed, &[Reason::Newsletter]).unwrap();
        assert_eq!(found.way, Err(Refusal::Unproven));
        // A mailing list (DMARC broken by the list) whose own domain signed the header: its address is proven.
        let list = "List-Unsubscribe: <mailto:club-leave@lists.signer.example>\r\n";
        let card = letter(list, "signer.example", "from:to:list-unsubscribe", "fail");
        let found = offer(&card, &ids, Trust::Unverified, &Lane::Filed, &[Reason::Newsletter]).unwrap();
        assert_eq!(found.way, Ok(Way::Mail { to: "club-leave@lists.signer.example".into(), subject: "unsubscribe".into(), body: "unsubscribe".into() }));
        // Forged, spam, a borrowed name, blocked, hostile: never.
        let card = letter(&format!("{links}{POST}"), "letters.example", BOTH, "pass");
        for (lane, reasons, refusal) in [
            (Lane::SetAside, vec![Reason::Forged], Refusal::Forged),
            (Lane::SetAside, vec![Reason::Spam { source: "rspamd", score: None }], Refusal::Spam),
            (Lane::SetAside, vec![Reason::Impersonation { brand: "Bank".into(), domain: "letters.example".into() }], Refusal::Borrowed),
            (Lane::SetAside, vec![Reason::Blocked], Refusal::Blocked),
            (Lane::Hostile, vec![Reason::Hostile], Refusal::Hostile),
        ] {
            assert_eq!(offer(&card, &ids, Trust::Verified, &lane, &reasons).unwrap().way, Err(refusal));
        }
        // No List-Unsubscribe: no button at all.
        let card = letter("", "letters.example", BOTH, "pass");
        assert!(judged(&card).is_none());
    }

    #[test]
    fn a_page_when_only_a_page_is_named() {
        let card = letter("List-Unsubscribe: <https://letters.example/preferences>\r\n", "letters.example", "from:to", "pass");
        assert_eq!(judged(&card).unwrap().way, Ok(Way::Page { url: "https://letters.example/preferences".into() }));
        // An address on your own network, or with a name put before the host: nothing.
        for url in ["https://192.168.1.1/u", "https://router.lan/u", "https://letters.example@evil.example/u", "https://localhost/u", "ftp://letters.example/u"] {
            let card = letter(&format!("List-Unsubscribe: <{url}>\r\n"), "letters.example", "from:to", "pass");
            assert_eq!(judged(&card).unwrap().way, Err(Refusal::Unusable), "{url}");
        }
        // Several recipients, or copies: not by mail.
        let card = letter("List-Unsubscribe: <mailto:a@letters.example,b@letters.example>\r\n", "letters.example", "from:to", "pass");
        assert_eq!(judged(&card).unwrap().way, Err(Refusal::Unusable));
        let card = letter("List-Unsubscribe: <mailto:leave@letters.example?cc=someone@example.org&body=stop%20please>\r\n", "letters.example", "from:to", "pass");
        assert_eq!(judged(&card).unwrap().way, Ok(Way::Mail { to: "leave@letters.example".into(), subject: "unsubscribe".into(), body: "stop please".into() }));
        assert_eq!(uris(" <https://a.example/x> ,\r\n\t(comment) <mailto: leave@a.example>"), vec!["https://a.example/x".to_string(), "mailto:leave@a.example".into()]);
    }

    #[test]
    fn the_record_keeps_what_was_done() {
        let dir = std::env::temp_dir().join(format!("sioul-unsubscribed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("unsubscribed.toml");
        let left = |way: &str, at: i64| Left { key: "list-id:issues.letters.example".into(), name: "Type & Pixels".into(), address: "letter@letters.example".into(), way: way.into(), at };
        Record::add(&path, left("page", 100)).unwrap();
        Record::add(&path, left("one-click", 200)).unwrap();
        // A page opened later does not undo a list left.
        Record::add(&path, left("page", 300)).unwrap();
        let record = Record::load(&path);
        assert_eq!(record.lists, vec![left("one-click", 200)]);
        // The button rests, and says when.
        let card = letter("List-Unsubscribe: <https://letters.example/preferences>\r\n", "letters.example", "from:to", "pass");
        let found = judged(&card).unwrap();
        let tr = Translator::new("en");
        let today = jiff::civil::date(2026, 10, 6);
        let shown = view(&found, &record, Some("you@example.org"), &tr, today);
        assert!(shown.done && !shown.offered && shown.tip.contains("Type & Pixels"), "{shown:?}");
        let shown = view(&found, &Record::default(), Some("you@example.org"), &tr, today);
        assert!(shown.offered && shown.way == "page" && shown.tip.contains("letters.example"), "{shown:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_request_is_plain_text_to_the_list_alone() {
        let sent = request(("Noa", "noa@example.org"), "leave@letters.example", "unsubscribe", "unsubscribe", 1_790_000_000).unwrap();
        let text = String::from_utf8_lossy(&sent.raw);
        assert_eq!(sent.recipients, vec!["leave@letters.example".to_string()]);
        assert!(text.contains("Subject: unsubscribe") && text.contains("leave@letters.example") && text.contains("text/plain") && !text.contains("text/html"), "{text}");
    }
}

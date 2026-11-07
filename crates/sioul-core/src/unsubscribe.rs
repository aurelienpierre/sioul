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
//! Nothing is contacted for forged mail, a borrowed name, a blocked sender or
//! hostile mail, nor when nothing proves the address it names is the list's
//! own: a valid signature of the domain shown in From (the message verified),
//! or a valid signature covering `List-Unsubscribe` by the domain the address
//! belongs to. A forged message could otherwise make Sioul post to, or write
//! to, anyone. Spam is a matter of proof (part 1 of 4 November 2026): from a
//! verified sender, its list's way out is used whatever the verdict, yours
//! or the filters' (someone gave your address without your consent to a
//! sending service that keeps the rules); from an unverified one, never, as
//! answering would tell whoever forged it that your address is read. What
//! was done is kept on this device (`Record`), so that a list is not asked
//! twice and you can see it.

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
    /// Your own filter finds it maybe spam: the same, until you say "Not spam".
    Doubt,
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
            Refusal::Doubt => "doubt",
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
    let verified = trust == Trust::Verified;
    // Spam and a doubt, a matter of proof (part 1 of 4 November 2026): a verified
    // sender's way out is used whatever the verdict. Unverified, spam is never
    // answered, nor a doubt (decided 3 November 2026): either would tell whoever
    // forged it that your address is read. "Not spam" brings the button back:
    // said not spam, the message is protected, your filter no longer judges it
    // (`porch::protected`).
    if !verified && (reasons.iter().any(Reason::is_spam) || *lane == Lane::SetAside) {
        return Err(Refusal::Spam);
    }
    if !verified && reasons.iter().any(|r| matches!(r, Reason::Unsure { .. })) {
        return Err(Refusal::Doubt);
    }
    if values.len() > 1 || headers.all("List-Unsubscribe-Post").count() > 1 {
        return Err(Refusal::Ambiguous);
    }
    let uris = uris(first);
    let signed = signatures(headers, trusted_ids);
    let from = card.sender_domain().unwrap_or("").to_ascii_lowercase();
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

/// A message lying in a junk folder (yours or your provider's): from an
/// unverified sender, refused as spam; a verified sender's way out stays,
/// and the message stays where it is (part 1 of 4 November 2026).
pub fn in_junk(offer: &mut Offer, trust: Trust) {
    if trust != Trust::Verified {
        offer.way = Err(Refusal::Spam);
    }
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

/// One copy on this device of a message you said something of, as
/// `spam_lists` reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaidCopy {
    /// Its Message-ID, bare (`mailindex::bare_id`).
    pub id: String,
    pub list: List,
    /// What it offers, judged as the Reader judges it (`offer`, then
    /// `in_junk`); none when it names no way out, or nobody needs to know
    /// (its list is out of the question already).
    pub way: Option<Result<Way, Refusal>>,
    /// How far its sender is proven, wherever the copy lies.
    pub trust: Trust,
    /// Its Date, Unix seconds; 0 without one.
    pub date: i64,
    /// Its sender's address.
    pub address: String,
    /// `$NotJunk` without `$Junk` on it: Not spam said in another mail client (`labels::said_ham`).
    pub not_junk: bool,
}

impl SaidCopy {
    /// The address this copy may be left by: one click, from a verified sender.
    pub fn leaves_by(&self) -> Option<&str> {
        fit(self).ok()
    }
}

/// A list to leave because you called its mail spam: one POST, to the address
/// its newest message names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpamList {
    pub list: List,
    pub url: String,
    /// The sender's address, for the record.
    pub address: String,
    /// The message whose address this is.
    pub message_id: String,
}

impl SpamList {
    /// The sender's domain, as the command line says it.
    pub fn domain(&self) -> &str {
        self.address.rsplit_once('@').map_or(self.address.as_str(), |(_, domain)| domain)
    }

    /// The record's entry once its server took the request.
    pub fn left(&self, at: i64) -> Left {
        Left { key: self.list.key.clone(), name: self.list.name.clone(), address: self.address.clone(), way: Way::OneClick { url: String::new() }.id().to_string(), at }
    }
}

/// What was not to leave, and why: lists, but for `gone`, whose list nobody can read any more.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NotLeft {
    /// Lists that offer no one click: another way out, or none.
    pub not_one_click: usize,
    /// Lists whose sender is not proven.
    pub unverified: usize,
    /// Lists refused for what they are: forged, a borrowed name, a blocked sender, hostile, ambiguous, unusable.
    pub refused: usize,
    /// Messages you called spam with no copy left on this device.
    pub gone: usize,
    /// Lists in the record already.
    pub left: usize,
    /// Lists with mail you called spam and a message you said is not.
    pub said_not_spam: usize,
}

impl NotLeft {
    /// Each count that is not zero, in a sentence.
    pub fn lines(&self, tr: &Translator) -> Vec<String> {
        [
            ("leave-spam-not-one-click", self.not_one_click),
            ("leave-spam-not-unverified", self.unverified),
            ("leave-spam-not-refused", self.refused),
            ("leave-spam-not-gone", self.gone),
            ("leave-spam-not-left", self.left),
            ("leave-spam-not-said", self.said_not_spam),
        ]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(id, n)| tr.text(id, Some(&tr.counted(n))))
        .collect()
    }
}

/// `spam_lists`'s answer: what to leave, by name, and what was not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpamLists {
    pub lists: Vec<SpamList>,
    pub not: NotLeft,
}

/// Why a message gives its list no way out, the worst last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Unfit {
    NotOneClick,
    Unverified,
    Refused,
}

/// The address one copy may be left by: one click, from a verified sender.
/// A signature aligned only with the address's host passes `offer`; a POST
/// then tells whoever forged the rest that the address is read, so the sender
/// must be proven here too.
fn fit(copy: &SaidCopy) -> Result<&str, Unfit> {
    match &copy.way {
        Some(Ok(Way::OneClick { url })) if copy.trust == Trust::Verified => Ok(url),
        Some(Ok(Way::OneClick { .. })) | Some(Err(Refusal::Spam | Refusal::Doubt | Refusal::Unproven)) => Err(Unfit::Unverified),
        Some(Err(_)) => Err(Unfit::Refused),
        Some(Ok(_)) | None => Err(Unfit::NotOneClick),
    }
}

/// The address a message may be left by: every copy here says it. A copy
/// that proves less, or another message under the same Message-ID (anyone who
/// knows one can reuse it), takes it away: a POST not sent costs nothing.
fn agreed<'a>(copies: &[&'a SaidCopy]) -> Result<&'a str, Unfit> {
    let fits: Vec<Result<&str, Unfit>> = copies.iter().map(|copy| fit(copy)).collect();
    if let Some(worst) = fits.iter().filter_map(|f| f.err()).max() {
        return Err(worst);
    }
    let url = fits.first().and_then(|f| f.ok()).ok_or(Unfit::NotOneClick)?;
    let same = copies.iter().all(|c| c.list.key == copies[0].list.key) && fits.iter().all(|f| *f == Ok(url));
    if same { Ok(url) } else { Err(Unfit::Refused) }
}

/// The lists to leave because you called their mail spam, and how many were
/// not and why (docs/client.md, "Unsubscribing"). A list is left when:
///
/// 1. your word: the newest entry of `said` (every device's label log, oldest
///    first) about a message is Spam; a Junk folder, a filter's verdict or a
///    provider's keyword is no word of yours;
/// 2. one click: its copies here (`copies`, one per copy of each message
///    named in `said`) offer one, as the Reader judges them;
/// 3. a verified sender, wherever the copy lies;
/// 4. nothing against it: the list is not in `record`, and you said Not spam
///    to no message of it.
///
/// One POST per list, to the newest such message's address. A list that does
/// not qualify counts once, for its newest message's reason.
pub fn spam_lists(said: &[crate::spam::labels::Entry], copies: &[SaidCopy], record: &Record) -> SpamLists {
    use crate::spam::labels::{self, Label};
    use std::collections::{BTreeMap, BTreeSet};
    let newest = labels::newest_by_message_id(said);
    let mut by_id: BTreeMap<&str, Vec<&SaidCopy>> = BTreeMap::new();
    for copy in copies {
        by_id.entry(copy.id.as_str()).or_default().push(copy);
    }
    // Your word on each message, Not spam from another mail client too.
    let word = |id: &str, label: Label| if by_id.get(id).is_some_and(|c| c.iter().any(|c| c.not_junk)) { Label::Ham } else { label };
    let said_ham: BTreeSet<&str> = newest.iter().filter(|(id, entry)| word(id, entry.label) == Label::Ham).flat_map(|(id, _)| by_id.get(id).into_iter().flatten().map(|copy| copy.list.key.as_str())).collect();

    /// A list's messages you called spam: the newest that gives a way out, and the newest of those that do not.
    #[derive(Default)]
    struct Messages<'a> {
        fit: Option<(i64, &'a str, &'a str, &'a SaidCopy)>,
        unfit: Option<(i64, Unfit)>,
    }
    let mut found = SpamLists::default();
    let mut by_list: BTreeMap<&str, Messages> = BTreeMap::new();
    for (id, entry) in &newest {
        if word(id, entry.label) != Label::Spam {
            continue;
        }
        let Some(copies) = by_id.get(id) else {
            found.not.gone += 1;
            continue;
        };
        let front = copies[0];
        let list = by_list.entry(front.list.key.as_str()).or_default();
        match agreed(copies) {
            // The later Date, then the Message-ID: the same answer whatever the order of the logs.
            Ok(url) if list.fit.is_none_or(|(date, other, _, _)| (front.date, *id) > (date, other)) => list.fit = Some((front.date, id, url, front)),
            Ok(_) => {}
            Err(unfit) if list.unfit.is_none_or(|(date, _)| front.date >= date) => list.unfit = Some((front.date, unfit)),
            Err(_) => {}
        }
    }
    for (key, messages) in by_list {
        if record.find(key).is_some() {
            found.not.left += 1;
        } else if said_ham.contains(key) {
            found.not.said_not_spam += 1;
        } else if let Some((_, id, url, copy)) = messages.fit {
            found.lists.push(SpamList { list: copy.list.clone(), url: url.to_string(), address: copy.address.clone(), message_id: id.to_string() });
        } else {
            match messages.unfit.map(|(_, unfit)| unfit) {
                Some(Unfit::Refused) => found.not.refused += 1,
                Some(Unfit::Unverified) => found.not.unverified += 1,
                Some(Unfit::NotOneClick) | None => found.not.not_one_click += 1,
            }
        }
    }
    found.lists.sort_by_key(|l| (l.list.name.to_lowercase(), l.list.key.clone()));
    found
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
    let day = |at: i64| jiff::Timestamp::from_second(at).map(|t| tr.day_in(t.to_zoned(crate::zone::current()).date(), today)).unwrap_or_default();
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
        let excused = card.is_list.then(|| trust::list_excused(auth.as_ref(), &card.headers, &trust::sealers(&ids, &[]))).flatten();
        let (trust, _) = trust::judge_sender(auth.as_ref(), excused.as_ref(), card.sender_domain());
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
        // Forged, a borrowed name, blocked, hostile: never, whoever sent it.
        let card = letter(&format!("{links}{POST}"), "letters.example", BOTH, "pass");
        for (lane, reasons, refusal) in [
            (Lane::SetAside, vec![Reason::Forged], Refusal::Forged),
            (Lane::SetAside, vec![Reason::Impersonation { brand: "Bank".into(), domain: "letters.example".into() }], Refusal::Borrowed),
            (Lane::SetAside, vec![Reason::Blocked], Refusal::Blocked),
            (Lane::Hostile, vec![Reason::Hostile], Refusal::Hostile),
        ] {
            assert_eq!(offer(&card, &ids, Trust::Verified, &lane, &reasons).unwrap().way, Err(refusal));
        }
        // Spam and a doubt from an unverified sender: never (decided 3 November 2026).
        for (lane, reasons, refusal) in [
            (Lane::SetAside, vec![Reason::Spam { source: "rspamd", score: None }], Refusal::Spam),
            (Lane::Review, vec![Reason::LearnedSpam { p: 0.97 }], Refusal::Spam),
            (Lane::Review, vec![Reason::Unsure { p: 0.62 }], Refusal::Doubt),
        ] {
            assert_eq!(offer(&card, &ids, Trust::Unverified, &lane, &reasons).unwrap().way, Err(refusal));
        }
        // Said not spam: no doubt left (`porch::protected`), the button is back; probably not spam keeps it.
        assert!(offer(&card, &ids, Trust::Verified, &Lane::Filed, &[Reason::Newsletter]).unwrap().way.is_ok());
        assert!(offer(&card, &ids, Trust::Verified, &Lane::Review, &[Reason::LearnedHam { p: 0.1 }]).unwrap().way.is_ok());
        // No List-Unsubscribe: no button at all.
        let card = letter("", "letters.example", BOTH, "pass");
        assert!(judged(&card).is_none());
    }

    /// Unsubscribing follows proof, not the verdict (part 1 of 4 November
    /// 2026): verified or not, × spam (your provider's, your filter's), maybe
    /// spam or not spam, × the header's way (one click, a message, a page).
    /// A verified sender's way out is used whatever the verdict, as it would
    /// be for its ham; an unverified sender's spam and doubts are refused, its
    /// ham only through the list's own signature; in a junk folder the same.
    #[test]
    fn unsubscribing_follows_proof_not_the_verdict() {
        let ids = vec![SIOUL.to_string()];
        let verdicts: [(&str, Lane, Vec<Reason>); 4] = [
            ("spam", Lane::SetAside, vec![Reason::Spam { source: "SpamAssassin", score: None }]),
            ("learned", Lane::Review, vec![Reason::LearnedSpam { p: 0.97 }]),
            ("maybe", Lane::Review, vec![Reason::Unsure { p: 0.62 }]),
            ("ham", Lane::Filed, vec![Reason::Newsletter]),
        ];
        let ways = [
            // One click: both headers signed by the sender's domain.
            (format!("List-Unsubscribe: <https://letters.example/u/a1b2>, <mailto:leave@letters.example>\r\n{POST}"), BOTH, "one-click"),
            // A message: no one click, the list's address first.
            ("List-Unsubscribe: <mailto:leave@letters.example>, <https://letters.example/u/a1b2>\r\n".to_string(), "from:to:subject:date", "mail"),
            // A page alone.
            ("List-Unsubscribe: <https://letters.example/preferences>\r\n".to_string(), "from:to:subject:date", "page"),
        ];
        for (header, signed, way) in &ways {
            let card = letter(header, "letters.example", signed, "pass");
            for (verdict, lane, reasons) in &verdicts {
                let verified = offer(&card, &ids, Trust::Verified, lane, reasons).unwrap().way;
                assert_eq!(verified.as_ref().map(Way::id), Ok(*way), "verified, {verdict}, {way}");
                let unverified = offer(&card, &ids, Trust::Unverified, lane, reasons).unwrap().way;
                let expected = match *verdict {
                    "spam" | "learned" => Err(Refusal::Spam),
                    "maybe" => Err(Refusal::Doubt),
                    // Ham: only the one click, whose signature covers its own header, is proven.
                    _ if *way == "one-click" => Ok("one-click"),
                    _ => Err(Refusal::Unproven),
                };
                assert_eq!(unverified.as_ref().map(Way::id).map_err(|r| *r), expected, "unverified, {verdict}, {way}");
                // In a junk folder: the verified way stays, the unverified one is spam.
                let mut junked = offer(&card, &ids, Trust::Verified, lane, reasons).unwrap();
                in_junk(&mut junked, Trust::Verified);
                assert_eq!(junked.way.as_ref().map(Way::id), Ok(*way), "verified in the junk, {verdict}, {way}");
                let mut junked = offer(&card, &ids, Trust::Unverified, lane, reasons).unwrap();
                in_junk(&mut junked, Trust::Unverified);
                assert_eq!(junked.way, Err(Refusal::Spam), "unverified in the junk, {verdict}, {way}");
            }
        }
        // The button's words for spam: "Unsubscribe", as for any list.
        let card = letter(&ways[0].0, "letters.example", BOTH, "pass");
        let found = offer(&card, &ids, Trust::Verified, &Lane::SetAside, &verdicts[0].2).unwrap();
        let shown = view(&found, &Record::default(), Some("you@example.org"), &Translator::new("en"), jiff::civil::date(2026, 11, 4));
        assert!(shown.offered && shown.label == "Unsubscribe", "{shown:?}");
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

    /// A label, `at` a given moment on some device.
    fn word(id: &str, at: i64, source: crate::spam::labels::Source) -> crate::spam::labels::Entry {
        crate::spam::labels::Entry { at, account: "home".into(), folder: "INBOX".into(), uidvalidity: 7, uid: 1, message_id: Some(id.into()), label: source.label(), source }
    }

    /// A copy that offers one click from a verified sender.
    fn fine(id: &str, key: &str, date: i64) -> SaidCopy {
        SaidCopy {
            id: id.into(),
            list: List { key: key.into(), name: format!("List {key}") },
            way: Some(Ok(Way::OneClick { url: format!("https://{key}.example/u/{id}") })),
            trust: Trust::Verified,
            date,
            address: format!("news@{key}.example"),
            not_junk: false,
        }
    }

    const NONE_LEFT: NotLeft = NotLeft { not_one_click: 0, unverified: 0, refused: 0, gone: 0, left: 0, said_not_spam: 0 };

    /// Rule 1: the newest word across every device's log wins; a copy with no
    /// word, a Junk folder's or a filter's, is not asked at all.
    #[test]
    fn your_newest_word_decides() {
        use crate::spam::labels::Source;
        let copies = [fine("m1", "a", 10)];
        // Spam said on one device, Not spam later on another: your newest word.
        let said = [word("m1", 100, Source::Junk), word("m1", 200, Source::NotSpam)];
        let found = spam_lists(&said, &copies, &Record::default());
        assert_eq!(found, SpamLists::default(), "no mail of it said spam: nothing to leave, nothing to count");
        // As two devices' logs hold it, read together: the later word wins, whichever log it is in.
        let root = std::env::temp_dir().join(format!("sioul-leave-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let log = |device: &str| root.join(crate::spam::labels::FOLDER).join(format!("{device}.jsonl"));
        crate::spam::labels::append_to(&log("zz-phone"), &word("m1", 100, Source::Junk)).unwrap();
        crate::spam::labels::append_to(&log("aa-desk"), &word("m1", 200, Source::NotSpam)).unwrap();
        assert_eq!(spam_lists(&crate::spam::labels::read_all_in(&root), &copies, &Record::default()), SpamLists::default(), "Not spam, later, on the other device");
        crate::spam::labels::append_to(&log("zz-phone"), &word("m1", 300, Source::Junk)).unwrap();
        assert_eq!(spam_lists(&crate::spam::labels::read_all_in(&root), &copies, &Record::default()).lists.len(), 1, "Spam again, later still");
        let _ = std::fs::remove_dir_all(&root);
        // The other way round: Spam it is.
        let said = [word("m1", 100, Source::NotSpam), word("m1", 200, Source::Junk)];
        let found = spam_lists(&said, &copies, &Record::default());
        assert_eq!(found.lists.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(), ["https://a.example/u/m1"]);
        assert_eq!(found.not, NONE_LEFT);
        // A sender blocked from the message says it too.
        assert_eq!(spam_lists(&[word("m1", 100, Source::Block)], &copies, &Record::default()).lists.len(), 1);
        // In a Junk folder, or flagged, with no word of yours: not left, not even counted.
        let found = spam_lists(&[], &copies, &Record::default());
        assert_eq!(found, SpamLists::default());
        // Not spam said since in another mail client: `$NotJunk` on its copy.
        let retracted = SaidCopy { not_junk: true, ..fine("m1", "a", 10) };
        let found = spam_lists(&[word("m1", 100, Source::Junk)], &[retracted.clone()], &Record::default());
        assert_eq!(found, SpamLists::default());
        // And it speaks for its list: another message of it, said spam, is not left.
        let found = spam_lists(&[word("m1", 100, Source::Junk), word("m2", 110, Source::Junk)], &[retracted, fine("m2", "a", 20)], &Record::default());
        assert_eq!((found.lists.len(), found.not), (0, NotLeft { said_not_spam: 1, ..NONE_LEFT }));
        // A message whose copies are gone: counted, no POST.
        let found = spam_lists(&[word("gone", 100, Source::Junk)], &[], &Record::default());
        assert_eq!((found.lists.len(), found.not), (0, NotLeft { gone: 1, ..NONE_LEFT }));
    }

    /// Rule 2: every refusal, and every other way, is only counted.
    #[test]
    fn only_one_click_is_left() {
        use crate::spam::labels::Source;
        let said = [word("m1", 100, Source::Junk)];
        let counted = |copy: SaidCopy| {
            let found = spam_lists(&said, &[copy], &Record::default());
            assert!(found.lists.is_empty());
            found.not
        };
        for refusal in [Refusal::Forged, Refusal::Borrowed, Refusal::Blocked, Refusal::Hostile, Refusal::Ambiguous, Refusal::Unusable] {
            assert_eq!(counted(SaidCopy { way: Some(Err(refusal)), ..fine("m1", "a", 10) }), NotLeft { refused: 1, ..NONE_LEFT }, "{refusal:?}");
        }
        for refusal in [Refusal::Spam, Refusal::Doubt, Refusal::Unproven] {
            assert_eq!(counted(SaidCopy { way: Some(Err(refusal)), trust: Trust::Unverified, ..fine("m1", "a", 10) }), NotLeft { unverified: 1, ..NONE_LEFT }, "{refusal:?}");
        }
        let by_mail = Way::Mail { to: "leave@a.example".into(), subject: "stop".into(), body: "stop".into() };
        assert_eq!(counted(SaidCopy { way: Some(Ok(by_mail)), ..fine("m1", "a", 10) }), NotLeft { not_one_click: 1, ..NONE_LEFT });
        assert_eq!(counted(SaidCopy { way: Some(Ok(Way::Page { url: "https://a.example/p".into() })), ..fine("m1", "a", 10) }), NotLeft { not_one_click: 1, ..NONE_LEFT });
        assert_eq!(counted(SaidCopy { way: None, ..fine("m1", "a", 10) }), NotLeft { not_one_click: 1, ..NONE_LEFT });
    }

    /// Rule 3, on a real `offer`: a signature aligned only with the address's
    /// host passes it, from a sender nothing proves; the batch does not.
    #[test]
    fn an_unverified_sender_whose_host_is_aligned_is_not_left() {
        use crate::spam::labels::Source;
        let ids = vec![SIOUL.to_string()];
        let card = letter(&format!("List-Unsubscribe: <https://unsub.mailer.example/u/a1b2>\r\n{POST}"), "mailer.example", BOTH, "fail");
        let way = offer(&card, &ids, Trust::Unverified, &Lane::Filed, &[Reason::Newsletter]).unwrap().way;
        assert_eq!(way, Ok(Way::OneClick { url: "https://unsub.mailer.example/u/a1b2".into() }), "the Reader's one click");
        let copy = SaidCopy { way: Some(way), trust: Trust::Unverified, ..fine("m1", "a", 10) };
        let found = spam_lists(&[word("m1", 100, Source::Junk)], &[copy.clone()], &Record::default());
        assert_eq!((found.lists.len(), found.not), (0, NotLeft { unverified: 1, ..NONE_LEFT }));
        // Wherever the copy lies: one proven copy does not vouch for another that is not.
        let found = spam_lists(&[word("m1", 100, Source::Junk)], &[fine("m1", "a", 10), copy], &Record::default());
        assert_eq!((found.lists.len(), found.not), (0, NotLeft { unverified: 1, ..NONE_LEFT }));
        // Another message under the same Message-ID, another address: nothing either.
        let reused = fine("m1", "b", 10);
        let found = spam_lists(&[word("m1", 100, Source::Junk)], &[fine("m1", "a", 10), reused], &Record::default());
        assert_eq!((found.lists.len(), found.not), (0, NotLeft { refused: 1, ..NONE_LEFT }));
    }

    /// One POST per list, the newest message's address; rule 4's two skips.
    #[test]
    fn one_post_per_list_and_nothing_against_it() {
        use crate::spam::labels::Source;
        let said = [word("old", 100, Source::Junk), word("new", 90, Source::Junk), word("other", 100, Source::Junk)];
        let copies = [fine("old", "a", 10), fine("new", "a", 20), fine("other", "b", 5)];
        let found = spam_lists(&said, &copies, &Record::default());
        assert_eq!(found.lists.iter().map(|l| (l.url.as_str(), l.message_id.as_str(), l.domain())).collect::<Vec<_>>(), [("https://a.example/u/new", "new", "a.example"), ("https://b.example/u/other", "other", "b.example")]);
        // A newer message that does not qualify takes nothing from an older, proven one.
        let forged = SaidCopy { way: Some(Err(Refusal::Forged)), trust: Trust::Forged, ..fine("new", "a", 30) };
        let found = spam_lists(&said[..2], &[fine("old", "a", 10), forged], &Record::default());
        assert_eq!(found.lists.iter().map(|l| l.message_id.as_str()).collect::<Vec<_>>(), ["old"]);
        assert_eq!(found.not, NONE_LEFT);
        // A list with one message you said is not spam: skipped, the other messages' words whatever.
        let said = [word("old", 100, Source::Junk), word("new", 110, Source::NotSpam), word("other", 100, Source::Junk)];
        let found = spam_lists(&said, &copies, &Record::default());
        assert_eq!(found.lists.iter().map(|l| l.list.key.as_str()).collect::<Vec<_>>(), ["b"]);
        assert_eq!(found.not, NotLeft { said_not_spam: 1, ..NONE_LEFT });
        // In the record, left or its page opened: skipped.
        let said = [word("old", 100, Source::Junk), word("other", 100, Source::Junk)];
        let left = |key: &str, way: &str| Left { key: key.into(), name: String::new(), address: String::new(), way: way.into(), at: 1 };
        let found = spam_lists(&said, &copies, &Record { lists: vec![left("a", "one-click"), left("b", "page")] });
        assert_eq!((found.lists.len(), found.not), (0, NotLeft { left: 2, ..NONE_LEFT }));
        // What the counts say, in both languages, each number in its own sentence.
        let all = NotLeft { not_one_click: 1, unverified: 2, refused: 3, gone: 4, left: 5, said_not_spam: 6 };
        for language in ["en", "fr"] {
            let lines = all.lines(&Translator::new(language));
            assert_eq!(lines.len(), 6);
            for (n, line) in lines.iter().enumerate() {
                let shown = line.replace(['\u{2068}', '\u{2069}'], "");
                assert!(shown.contains(&(n + 1).to_string()) && !shown.contains("leave-spam"), "{line}");
            }
        }
        assert!(NONE_LEFT.lines(&Translator::new("en")).is_empty());
        let chosen = fine("m1", "a", 10);
        let list = SpamList { list: chosen.list.clone(), url: "https://a.example/u".into(), address: chosen.address.clone(), message_id: "m1".into() };
        assert_eq!(list.left(42), Left { key: "a".into(), name: "List a".into(), address: "news@a.example".into(), way: "one-click".into(), at: 42 });
    }

    #[test]
    fn the_request_is_plain_text_to_the_list_alone() {
        let sent = request(("Noa", "noa@example.org"), "leave@letters.example", "unsubscribe", "unsubscribe", 1_790_000_000).unwrap();
        let text = String::from_utf8_lossy(&sent.raw);
        assert_eq!(sent.recipients, vec!["leave@letters.example".to_string()]);
        assert!(text.contains("Subject: unsubscribe") && text.contains("leave@letters.example") && text.contains("text/plain") && !text.contains("text/html"), "{text}");
    }
}

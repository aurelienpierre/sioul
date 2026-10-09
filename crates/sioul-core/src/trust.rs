// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Who really sent a message, from what the servers recorded.
//!
//! It reads the Authentication-Results headers (RFC 8601) written by your own
//! provider, and nobody else's: any sender can write that header into a
//! message, so only the ones carrying an authserv-id you trust count (RFC 8601
//! §5 and §7.1). Providers differ in what they record, so Sioul also checks
//! each message itself as it is fetched (`sioul_sync::verify`, with
//! Stalwart's mail-auth): its results are one more such header, under its own
//! id ([`config::sioul_authserv_id`](crate::config::sioul_authserv_id)),
//! trusted first.
//!
//! The spam verdicts of the provider's own filters (SpamAssassin, rspamd) are
//! read here too, for the same reason: they are only as good as their source.

use crate::headers::RawHeaders;
use std::net::IpAddr;

/// The result of one authentication method (RFC 8601 §2.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    SoftFail,
    Neutral,
    None,
    TempError,
    PermError,
    Other,
}

impl Outcome {
    fn parse(word: &str) -> Self {
        match word.to_ascii_lowercase().as_str() {
            "pass" => Outcome::Pass,
            "fail" | "hardfail" => Outcome::Fail,
            "softfail" => Outcome::SoftFail,
            "neutral" => Outcome::Neutral,
            "none" => Outcome::None,
            "temperror" => Outcome::TempError,
            "permerror" => Outcome::PermError,
            _ => Outcome::Other,
        }
    }
}

/// What one Authentication-Results header says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthResults {
    pub authserv_id: String,
    pub spf: Option<Outcome>,
    pub dkim: Option<Outcome>,
    pub dmarc: Option<Outcome>,
    pub arc: Option<Outcome>,
    /// The reverse DNS of the server that handed the message over.
    pub iprev: Option<Outcome>,
    /// The domain of the signature that passed, else of the last one read (`header.d`, `header.i`).
    pub dkim_domain: Option<String>,
    /// The domains of every signature that passed: any domain can sign, so a
    /// signature vouches only for the sender's own domain (`judge_sender`).
    pub dkim_passed: Vec<String>,
    /// The domain SPF was checked for (`smtp.mailfrom`).
    pub spf_domain: Option<String>,
    /// The domain DMARC was checked for (`header.from`).
    pub dmarc_domain: Option<String>,
    /// What that domain asks of receivers when DMARC fails (`policy.dmarc`): none, quarantine, reject.
    pub dmarc_policy: Option<String>,
}

/// How far the sender can be believed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// The sender's domain vouches for the message (DMARC, or a valid DKIM signature).
    Verified,
    /// Nothing proves it either way.
    Unverified,
    /// The domain in From says the message is not from it (DMARC fail).
    Forged,
}

/// Why the sender is believed or not; the sentence is the translator's (`i18n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proof {
    DmarcPass,
    DmarcFail,
    /// DMARC failed, but the domain asks nothing of receivers when it does (`p=none`).
    DmarcFailNoPolicy,
    /// DMARC failed on a mailing list, which rewrites what it relays and breaks signatures.
    DmarcFailList,
    DkimPass,
    BothFailed,
    NothingProves,
    NoResults,
}

/// The topmost Authentication-Results header written by a server you trust.
pub fn read_auth_results(headers: &RawHeaders, trusted_ids: &[String]) -> Option<AuthResults> {
    headers
        .all("Authentication-Results")
        .filter_map(parse_auth_results)
        .find(|r| trusted_ids.iter().any(|id| id.eq_ignore_ascii_case(&r.authserv_id)))
}

/// A DKIM signature that passed, as a results header records it: the
/// signer's domain (`header.d`, else the domain of `header.i`), its selector
/// (`header.s`) and the start of the signature itself (`header.b`), when written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DkimPass {
    pub domain: String,
    pub selector: Option<String>,
    /// As written: base64 is case-sensitive.
    pub b: Option<String>,
}

/// The DKIM signatures that passed, read from the header `read_auth_results`
/// reads (the topmost from a server you trust); none from a header that
/// cannot be read to its end.
pub fn dkim_passes(headers: &RawHeaders, trusted_ids: &[String]) -> Vec<DkimPass> {
    let trusted = |value: &&str| parse_auth_results(value).is_some_and(|r| trusted_ids.iter().any(|id| id.eq_ignore_ascii_case(&r.authserv_id)));
    let Some(value) = headers.all("Authentication-Results").find(trusted) else { return Vec::new() };
    let (parts, balanced) = results_of(value);
    if !balanced {
        return Vec::new();
    }
    parts
        .iter()
        .skip(1)
        .filter_map(|part| {
            let (method, rest) = part.trim().split_once('=')?;
            if !method.split('/').next().unwrap_or(method).trim().eq_ignore_ascii_case("dkim") {
                return None;
            }
            let words = words_of(rest);
            if Outcome::parse(words.first()?) != Outcome::Pass {
                return None;
            }
            let property = |name: &str| words.iter().filter_map(|w| w.split_once('=')).find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.trim_matches(['"', ';']).to_string());
            let domain = property("header.d").or_else(|| property("header.i").map(|i| i.rsplit_once('@').map_or(i.clone(), |(_, d)| d.to_string())))?;
            Some(DkimPass { domain: domain.to_ascii_lowercase(), selector: property("header.s").map(|s| s.to_ascii_lowercase()), b: property("header.b").filter(|b| !b.is_empty()) })
        })
        .collect()
}

/// The authserv-id your provider writes, learned from its mail: the id of the
/// topmost Authentication-Results header of most messages.
///
/// A provider that records its checks adds its header on top of whatever the
/// message carried, so the topmost one is its own (RFC 8601 §5). A sender can
/// forge headers below it, never above. When the provider records nothing,
/// the topmost header is whatever senders wrote, and no id wins a majority:
/// then nothing is learned and trust waits for Sioul's own checks.
/// Sioul's own results sit above the provider's, under `.invalid`: they are skipped.
pub fn learn_provider_id<'a>(all_headers: impl IntoIterator<Item = &'a RawHeaders>) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    let mut messages = 0usize;
    for headers in all_headers {
        messages += 1;
        let topmost = headers
            .all("Authentication-Results")
            .filter_map(parse_auth_results)
            .map(|r| r.authserv_id.to_ascii_lowercase())
            .find(|id| !id.ends_with(".invalid"));
        let Some(id) = topmost else {
            continue;
        };
        // An id is a host name; Microsoft's internal headers start with "spf=" instead.
        if !id.contains('.') || id.contains('=') {
            continue;
        }
        match counts.iter_mut().find(|(known, _)| *known == id) {
            Some((_, n)) => *n += 1,
            None => counts.push((id, 1)),
        }
    }
    let (id, n) = counts.into_iter().max_by_key(|(_, n)| *n)?;
    (n >= LEARN_MINIMUM && n * 2 > messages).then_some(id)
}

/// Messages needed before an id is learned.
const LEARN_MINIMUM: usize = 5;

/// Parses `authserv-id [version]; method=result props; ...`, comments removed.
fn parse_auth_results(value: &str) -> Option<AuthResults> {
    let (parts, balanced) = results_of(value);
    let mut parts = parts.into_iter();
    let authserv_id = parts.next()?.split_whitespace().next()?.to_string();
    if !balanced {
        // A quote or a comment left open: the writer copied a sender's text as
        // it came, so nothing after it proves anything. A DMARC failure said
        // anywhere still counts: it can only set mail aside.
        let fails = value.to_ascii_lowercase().contains("dmarc=fail");
        return Some(AuthResults { authserv_id, dmarc: fails.then_some(Outcome::Fail), ..AuthResults::default() });
    }
    let mut results = AuthResults { authserv_id, ..AuthResults::default() };
    for part in parts {
        record_method(&mut results, &part);
    }
    Some(results)
}

/// Records `method=result`: several DKIM signatures count as a pass if any
/// passes, and a DMARC failure is never undone by a pass beside it.
fn record_method(results: &mut AuthResults, part: &str) {
    let Some((method, rest)) = part.trim().split_once('=') else { return };
    let method = method.split('/').next().unwrap_or(method).trim().to_ascii_lowercase();
    let words = words_of(rest);
    let outcome = Outcome::parse(words.first().map_or("", String::as_str));
    // The properties after the result: "header.d=example.org policy.dmarc=reject".
    let property = |name: &str| {
        words
            .iter()
            .filter_map(|w| w.split_once('='))
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim_matches(['"', ';']).to_ascii_lowercase())
    };
    let domain_of = |value: String| value.rsplit_once('@').map_or(value.clone(), |(_, d)| d.to_string());
    let signer = || property("header.d").or_else(|| property("header.i").map(domain_of));
    // Every signature that passed is kept: the sender's own may come after a service's.
    if let ("dkim", Outcome::Pass, Some(domain)) = (method.as_str(), outcome, signer()) {
        results.dkim_passed.push(domain);
    }
    let slot = match method.as_str() {
        "spf" => &mut results.spf,
        "dkim" => &mut results.dkim,
        "dmarc" => &mut results.dmarc,
        "arc" => &mut results.arc,
        "iprev" => &mut results.iprev,
        _ => return,
    };
    // Several results of one method: the first pass stays, but for DMARC a failure wins.
    let kept = if method == "dmarc" { Outcome::Fail } else { Outcome::Pass };
    if *slot == Some(kept) {
        return;
    }
    *slot = Some(outcome);
    match method.as_str() {
        "dkim" => results.dkim_domain = signer(),
        "spf" => results.spf_domain = property("smtp.mailfrom").map(domain_of).or_else(|| property("smtp.helo")),
        "dmarc" => {
            results.dmarc_domain = property("header.from");
            results.dmarc_policy = property("policy.dmarc");
        }
        _ => {}
    }
}

/// The results of a header, split at the semicolons between them, comments
/// removed (they may nest: "(google.com: domain of (x))"), and whether every
/// quote and comment was closed. A semicolon, a parenthesis or an equals sign
/// inside a quoted string belongs to a value: a sender chooses the address
/// `"a; dmarc=pass b"@example.org`, and checkers write it as it is, quoted
/// (RFC 8601 §2.2, RFC 5322 §3.2); a backslash escapes the next character.
fn results_of(value: &str) -> (Vec<String>, bool) {
    let mut parts = Vec::new();
    let mut part = String::new();
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for c in value.chars() {
        let in_comment = depth > 0;
        if escaped {
            escaped = false;
        } else if c == '\\' && (quoted || in_comment) {
            escaped = true;
        } else if c == '"' && !in_comment {
            quoted = !quoted;
        } else if !quoted && c == '(' {
            depth += 1;
            continue;
        } else if !quoted && c == ')' {
            depth = depth.saturating_sub(1);
            continue;
        } else if !quoted && !in_comment && c == ';' {
            parts.push(std::mem::take(&mut part));
            continue;
        }
        if !in_comment {
            part.push(c);
        }
    }
    parts.push(part);
    (parts, depth == 0 && !quoted && !escaped)
}

/// The words of a result, a quoted string kept whole: `pass smtp.mailfrom="a b"@x`.
fn words_of(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut escaped) = (false, false);
    for c in text.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' && quoted {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if c.is_whitespace() && !quoted {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// A domain's organisational domain, as DMARC's relaxed alignment reads it
/// (RFC 7489 §3.2): the name registered under its public suffix, by the
/// Public Suffix List ("mail.example.co.uk" → "example.co.uk"; the `psl`
/// crate carries the list). None for a public suffix itself ("co.uk",
/// "github.io"), which nobody owns as a whole.
pub fn organisational(domain: &str) -> Option<String> {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    psl::domain_str(&domain).map(str::to_string)
}

/// Whether two domains belong together, as DMARC's relaxed alignment has it:
/// the same, or the same organisational domain (`organisational`):
/// "mail.example.org", "shop.example.org" and "example.org" do;
/// "a.github.io" and "github.io" do not, nor "victim.co.uk" and "co.uk": a
/// name under a public suffix belongs to whoever registered it.
pub(crate) fn aligned(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().trim_end_matches('.').to_ascii_lowercase(), b.trim().trim_end_matches('.').to_ascii_lowercase());
    !a.is_empty() && !b.is_empty() && (a == b || organisational(&a).is_some_and(|org| organisational(&b).as_deref() == Some(org.as_str())))
}

/// The verdict on the sender, when the domain shown is not at hand: the domain
/// DMARC was checked for stands for it. See `judge_sender`.
pub fn judge(results: Option<&AuthResults>, mailing_list: bool) -> (Trust, Proof) {
    judge_sender(results, mailing_list, None)
}

/// The verdict on the sender, and why. A DMARC failure is a forgery when the
/// domain asks receivers to act on it (quarantine, reject, or a provider's
/// result that does not say); with `p=none`, or on a mailing list, which
/// rewrites what it relays, it only leaves the sender unverified.
///
/// A proof counts only for the domain you are shown (`sender_domain`, the
/// address in From): a valid signature of another domain proves nothing about
/// it (anyone can sign with their own domain), and neither does a DMARC pass
/// for another From than the one shown (a message carrying two).
pub fn judge_sender(results: Option<&AuthResults>, mailing_list: bool, sender_domain: Option<&str>) -> (Trust, Proof) {
    let Some(r) = results else { return (Trust::Unverified, Proof::NoResults) };
    let sender_domain = sender_domain.filter(|d| !d.is_empty());
    let checked = r.dmarc_domain.as_deref().filter(|d| !d.is_empty());
    let dmarc_for_sender = match (sender_domain, checked) {
        (Some(shown), Some(checked)) => aligned(shown, checked),
        _ => true,
    };
    let signed_by_sender = sender_domain.or(checked).is_some_and(|from| r.dkim_passed.iter().any(|d| aligned(from, d)));
    match r.dmarc {
        Some(Outcome::Pass) if dmarc_for_sender => (Trust::Verified, Proof::DmarcPass),
        Some(Outcome::Fail) if mailing_list => (Trust::Unverified, Proof::DmarcFailList),
        Some(Outcome::Fail) if r.dmarc_policy.as_deref() == Some("none") => (Trust::Unverified, Proof::DmarcFailNoPolicy),
        Some(Outcome::Fail) => (Trust::Forged, Proof::DmarcFail),
        _ if signed_by_sender => (Trust::Verified, Proof::DkimPass),
        _ if r.dkim == Some(Outcome::Fail) && r.spf == Some(Outcome::Fail) => (Trust::Unverified, Proof::BothFailed),
        _ => (Trust::Unverified, Proof::NothingProves),
    }
}

/// Whether a message is authenticated, as the results its account trusts
/// record it (`read_auth_results`: Sioul's own stamp, else your provider's).
/// It is not when SPF and DKIM both failed, unless DMARC passed, or ARC passed
/// on a chain whose newest seal is a sealer's you trust (`sealers`: your
/// provider's, your own domains'; `arc_sealer`): forwarding breaks SPF, and a
/// forwarder that changes the message breaks DKIM, but anyone can seal a
/// chain of their own. "none" (nothing published to check) is no failure, nor
/// is a softfail; no results at all say nothing. DMARC failing under a policy
/// is forged (`judge_sender`), set aside before this is asked. Mail that is
/// not authenticated is not taken as coming from the address it shows
/// (porch.rs: its sender is a stranger).
pub fn authenticated(results: Option<&AuthResults>, headers: &RawHeaders, sealers: &[String]) -> bool {
    let Some(r) = results else { return true };
    if !(r.spf == Some(Outcome::Fail) && r.dkim == Some(Outcome::Fail)) || r.dmarc == Some(Outcome::Pass) {
        return true;
    }
    r.arc == Some(Outcome::Pass) && arc_sealer(headers).is_some_and(|sealer| sealers.iter().any(|s| aligned(&sealer, s)))
}

/// The domain that sealed the newest ARC set: the `d=` of the `ARC-Seal`
/// with the highest instance (`i=`), lower case (RFC 8617 §4.1.3).
pub fn arc_sealer(headers: &RawHeaders) -> Option<String> {
    headers
        .all("ARC-Seal")
        .filter_map(|seal| {
            let tags: Vec<(&str, &str)> = seal.split(';').filter_map(|tag| tag.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
            let tag = |name: &str| tags.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| *v);
            Some((tag("i")?.parse::<u32>().ok()?, tag("d")?.to_ascii_lowercase()))
        })
        .max_by_key(|(instance, _)| *instance)
        .map(|(_, domain)| domain)
}

/// Where the message entered your provider: the first server, from the top of
/// the `Received` chain, that handed it over from a public address. SPF and the
/// reverse DNS are checked on it; every hop below it could be written by anyone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    pub ip: IpAddr,
    /// The name the sending server gave (HELO/EHLO).
    pub helo: String,
    /// The receiving server.
    pub by: String,
}

/// Walking down from the top as `written_on_arrival` does, your provider's
/// own hops passed (`inside_hop`), the first other line: where the message
/// came in, when the address its server saw reads (`seen_ip`) and is public.
/// A line written another way gives none: the lines below it are the
/// sender's to write, so no address of theirs is taken for where it came in.
pub fn boundary(headers: &RawHeaders) -> Option<Boundary> {
    let received = headers.all("Received").find(|r| !inside_hop(r))?;
    let ip = seen_ip(received).filter(is_public)?;
    let (first, rest) = from_clause(received)?;
    // qmail writes the name given in HELO apart, when it is not the reverse DNS name it writes first.
    let helo = leading_groups(rest).first().and_then(|g| helo_group(g)).unwrap_or_else(|| first.split('(').next().unwrap_or(first)).trim_end_matches('.').to_string();
    let by = received.split(" by ").nth(1)?.split_whitespace().next()?.trim_end_matches([';', '.']).to_string();
    Some(Boundary { ip, helo, by })
}

/// The envelope sender (MAIL FROM), as the receiving server recorded it in Return-Path;
/// empty for a bounce (`<>`).
pub fn mail_from(headers: &RawHeaders) -> Option<String> {
    let value = headers.first("Return-Path")?.trim();
    Some(value.trim_start_matches('<').trim_end_matches('>').trim().to_string())
}

fn is_public(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            // Shared (carrier-grade NAT, 100.64/10), "this network" (0/8), benchmarking (198.18/15), reserved (240/4).
            let unrouted = (a == 100 && (64..128).contains(&b)) || a == 0 || (a == 198 && (b & 0xfe) == 18) || a >= 240;
            !(v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified() || v4.is_documentation() || unrouted)
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_public(&IpAddr::V4(v4)),
            None => !(v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80),
        },
    }
}

/// The verdict of the provider's own spam filter, when it left one.
#[derive(Debug, Clone, PartialEq)]
pub struct SpamVerdict {
    pub flagged: bool,
    pub score: Option<f32>,
    pub source: &'static str,
}

/// Reads SpamAssassin's X-Spam-* headers, else rspamd's, among the fields
/// your provider wrote (`written_on_arrival`) and nowhere else: a sender
/// writes what it likes into its own message, "X-Spam-Flag: NO" too, and its
/// outgoing server's filter judges what it sends, not what you receive.
pub fn read_spam_verdict(headers: &RawHeaders) -> Option<SpamVerdict> {
    let provider = written_on_arrival(headers);
    spamassassin(&provider).or_else(|| rspamd(&provider))
}

/// The fields your provider wrote on a message once it took it in, top to
/// bottom: those above the `Received` line where it came in from the Internet.
///
/// Each server adds its lines on top of what it is handed (RFC 5321 §4.4), so
/// whatever a sender wrote, its own servers' lines included, lies below that
/// line, and nothing above it is theirs. Walking down from the top, the
/// provider's own hops are passed: a line without `from` (a server handing
/// the message to itself), one from a private or local address (a filter, an
/// antivirus, the next server inside), the delivery into the mailbox (LMTP:
/// Dovecot and Cyrus add their line on top of the filters' verdicts). The
/// walk stops at the first line from a public address, where the message came
/// in, or at one it cannot read (an address written another way): the fields
/// above it are the provider's. Sioul's own results, on the very top, are
/// neither a line nor a verdict. Mail that never came from outside (only
/// private addresses) ends at its lowest `Received`; mail without any (copied
/// into the mailbox, never delivered) has nothing of its provider's. A filter
/// that adds its verdict at the bottom of the block, or just below the line
/// where the message came in, cannot be told from the sender: its verdict is not read.
fn written_on_arrival(headers: &RawHeaders) -> RawHeaders {
    let mut lowest = 0;
    for (at, (name, value)) in headers.fields().enumerate() {
        if !name.eq_ignore_ascii_case("Received") {
            continue;
        }
        if !inside_hop(value) {
            return headers.top(at);
        }
        lowest = at;
    }
    headers.top(lowest)
}

/// A hop inside your provider, as its `Received` line says: no `from`, a
/// private or local address in it, the delivery into the mailbox, or a hop
/// between Microsoft 365's own servers (`microsoft_hop`).
fn inside_hop(received: &str) -> bool {
    let from = received.trim_start().get(..5).is_some_and(|w| w.eq_ignore_ascii_case("from "));
    if !from || microsoft_hop(received) {
        return true;
    }
    match seen_ip(received) {
        Some(ip) => !is_public(&ip),
        None => lmtp(received),
    }
}

/// The names Microsoft 365's servers carry between themselves, as each
/// writes its own after `by`: its mailboxes and relays (`*.prod.outlook.com`,
/// `*.prod.exchangelabs.com`), its frontends inside (`*.outlook.office365.com`)
/// and its filters (`*.prod.protection.outlook.com`).
const MICROSOFT_INSIDE: [&str; 4] = [".prod.outlook.com", ".prod.exchangelabs.com", ".outlook.office365.com", ".prod.protection.outlook.com"];

/// A hop between Microsoft 365's own servers: received by one of the names
/// they carry between themselves (`MICROSOFT_INSIDE`). Those hops pass the
/// message from one public address of Microsoft's to another, so an address
/// cannot tell them from where the message came in, which is the hop received
/// by its Internet frontend, `*.mail.protection.outlook.com` ("Received: from
/// mail.sender.example (203.0.113.25) by BN8NAM12FT034.mail.protection.outlook.com
/// (10.13.182.135) with Microsoft SMTP Server … via Frontend Transport"). The
/// receiving server writes its own name, and the lines above where the message
/// came in are Microsoft's: no sender writes one of them.
fn microsoft_hop(received: &str) -> bool {
    let Some(by) = received.split(" by ").nth(1).and_then(|b| b.split_whitespace().next()) else { return false };
    let by = by.trim_end_matches([';', '.']).to_ascii_lowercase();
    !by.ends_with(".mail.protection.outlook.com") && MICROSOFT_INSIDE.iter().any(|suffix| by.ends_with(suffix))
}

/// "… with LMTP id …", LMTPS, LMTPA, LMTPSA (RFC 3848): the mailbox's own delivery.
fn lmtp(received: &str) -> bool {
    let words: Vec<&str> = received.split_whitespace().collect();
    words.windows(2).any(|pair| pair[0].eq_ignore_ascii_case("with") && pair[1].get(..4).is_some_and(|p| p.eq_ignore_ascii_case("LMTP")))
}

/// `X-Spam-Flag: YES`, `X-Spam-Status: Yes, score=9.1 required=5.0 tests=...`.
fn spamassassin(headers: &RawHeaders) -> Option<SpamVerdict> {
    let flag = headers.first("X-Spam-Flag").map(|v| v.trim().eq_ignore_ascii_case("yes"));
    let status = headers.first("X-Spam-Status");
    if flag.is_none() && status.is_none() {
        return None;
    }
    let says_yes = status.is_some_and(|s| s.trim_start().to_ascii_lowercase().starts_with("yes"));
    let score = status.and_then(|s| number_after(s, "score="));
    Some(SpamVerdict { flagged: flag.unwrap_or(false) || says_yes, score, source: "SpamAssassin" })
}

/// `X-Spamd-Result: default: True [12.30 / 15.00]; ...`, `X-Spam: Yes`, `X-Rspamd-Score`.
fn rspamd(headers: &RawHeaders) -> Option<SpamVerdict> {
    let result = headers.first("X-Spamd-Result");
    let flag = headers.first("X-Spam").map(|v| v.trim().eq_ignore_ascii_case("yes"));
    if result.is_none() && flag.is_none() {
        return None;
    }
    let says_true = result.is_some_and(|r| r.to_ascii_lowercase().contains("default: true"));
    let score = result
        .and_then(|r| number_after(r, "["))
        .or_else(|| headers.first("X-Rspamd-Score").and_then(|s| s.trim().parse().ok()));
    Some(SpamVerdict { flagged: flag.unwrap_or(false) || says_true, score, source: "rspamd" })
}

fn number_after(text: &str, marker: &str) -> Option<f32> {
    let start = text.find(marker)? + marker.len();
    let number: String = text[start..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    number.parse().ok()
}

/// The IP addresses of the servers the message came through, newest first.
///
/// Kept for the next step: the sender's reputation is checked on the address
/// your provider received the message from, the trust boundary
/// (docs/design.md, "Trust").
pub fn route_ips(headers: &RawHeaders) -> Vec<IpAddr> {
    headers.all("Received").filter_map(seen_ip).collect()
}

/// The address the receiving server saw, `[203.0.113.7]` or `[IPv6:2001:db8::1]`,
/// in a Received header's `from` clause: `from helo (rdns [203.0.113.7])`,
/// `from rdns ([203.0.113.7]:5126 helo=x)`, or `from [203.0.113.7] (helo=x)`
/// when the address has no name. The first word is, with most servers, the
/// name the sender gave in HELO, which it may write `[192.0.2.1]`, as it may
/// the `helo=` in the parentheses: neither counts while the server wrote the
/// address it saw.
fn bracketed_ip(received: &str) -> Option<IpAddr> {
    let (first, rest) = from_clause(received)?;
    let address = |inner: &str| inner.strip_prefix("IPv6:").unwrap_or(inner).parse::<IpAddr>().ok();
    let mut depth = 0usize;
    for (at, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '[' if depth > 0 => {
                let before = rest[..at].trim_end().to_ascii_lowercase();
                let helo = ["helo=", "ehlo=", "helo", "ehlo"].iter().any(|w| before.strip_suffix(w).is_some_and(|b| b.is_empty() || b.ends_with([' ', '\t', '('])));
                let seen = rest[at + 1..].find(']').and_then(|end| address(&rest[at + 1..at + 1 + end]));
                if let (false, Some(ip)) = (helo, seen) {
                    return Some(ip);
                }
            }
            _ => {}
        }
    }
    first.strip_prefix('[').and_then(|f| f.split(']').next()).and_then(address)
}

/// A `Received` line's `from` clause, up to " by ": its first word (the
/// name given in HELO, with most servers; the reverse DNS name with qmail)
/// and what follows it; none for a line without `from`.
fn from_clause(received: &str) -> Option<(&str, &str)> {
    let clause = received.split(" by ").next().unwrap_or(received).trim_start();
    let words = clause.get(..5).filter(|w| w.eq_ignore_ascii_case("from ")).map(|_| clause[5..].trim_start())?;
    Some(words.split_once(char::is_whitespace).map_or((words, ""), |(first, rest)| (first, rest.trim_start())))
}

/// The parenthesised groups that open `text`, one after the other:
/// "(HELO x) (203.0.113.5) with SMTP" → "HELO x", "203.0.113.5".
fn leading_groups(text: &str) -> Vec<&str> {
    let mut groups = Vec::new();
    let mut at = text.trim_start();
    while let Some(inner) = at.strip_prefix('(') {
        let mut depth = 1usize;
        let Some(end) = inner.char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        }) else {
            break;
        };
        groups.push(&inner[..end]);
        at = inner[end + 1..].trim_start();
    }
    groups
}

/// The name a group gives in HELO, qmail's "(HELO mail.sender.example)".
fn helo_group(group: &str) -> Option<&str> {
    let (head, name) = group.trim().split_once(char::is_whitespace)?;
    (head.eq_ignore_ascii_case("HELO") || head.eq_ignore_ascii_case("EHLO")).then(|| name.trim())
}

/// The address in parentheses without brackets, right after the first word,
/// as Microsoft 365 and Exchange write it ("from mail.sender.example
/// (203.0.113.25) by …", "(2001:db8::25)"), and qmail ("from unknown (HELO
/// mail.sender.example) (203.0.113.5) by …"; "(ident@203.0.113.5)", where what
/// comes before the last "@" is what the sender's ident service said). The
/// group holds the address alone, a HELO group before it passed: a group
/// holding anything else is not read.
fn parenthesised_ip(rest: &str) -> Option<IpAddr> {
    let groups = leading_groups(rest);
    let mut groups = groups.iter().skip_while(|g| helo_group(g).is_some());
    let group = groups.next()?.trim();
    let address = group.rsplit_once('@').map_or(group, |(_, ip)| ip);
    address.strip_prefix("IPv6:").unwrap_or(address).parse().ok()
}

/// The address the receiving server saw, in any of the forms servers write
/// it: in brackets (`bracketed_ip`), else in parentheses alone
/// (`parenthesised_ip`: Microsoft 365, Exchange, qmail).
fn seen_ip(received: &str) -> Option<IpAddr> {
    bracketed_ip(received).or_else(|| from_clause(received).and_then(|(_, rest)| parenthesised_ip(rest)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(text: &str) -> RawHeaders {
        RawHeaders::parse(text.as_bytes())
    }

    #[test]
    fn trusts_only_your_provider() {
        let h = headers(
            "Authentication-Results: mx.example.net; dmarc=pass header.from=bank.example\n\
             Authentication-Results: attacker.example; dmarc=pass\n\n",
        );
        let mine = read_auth_results(&h, &["mx.example.net".into()]).unwrap();
        assert_eq!(mine.dmarc, Some(Outcome::Pass));
        assert!(read_auth_results(&h, &["other.example".into()]).is_none());
    }

    #[test]
    fn reads_comments_and_several_signatures() {
        let h = headers(
            "Authentication-Results: mx.example.net (version 2);\n\
             \tdkim=fail (bad sig) header.d=a.example; dkim=pass header.d=b.example;\n\
             \tspf=softfail (domain of (x) is weird) smtp.mailfrom=b.example\n\n",
        );
        let r = read_auth_results(&h, &["mx.example.net".into()]).unwrap();
        assert_eq!(r.dkim, Some(Outcome::Pass));
        assert_eq!(r.spf, Some(Outcome::SoftFail));
        assert_eq!(judge_sender(Some(&r), false, Some("b.example")).0, Trust::Verified);
        assert_eq!(judge_sender(Some(&r), false, Some("news.b.example")).0, Trust::Verified);
        // A signature of another domain proves nothing about the sender: anyone can sign.
        assert_eq!(judge_sender(Some(&r), false, Some("bank.example")), (Trust::Unverified, Proof::NothingProves));
        assert_eq!(judge(Some(&r), false).0, Trust::Unverified, "no domain to compare: nothing proves it");
    }

    #[test]
    fn a_sender_cannot_write_results_into_a_value() {
        // Sioul's own checks write a quoted envelope sender as it is: its semicolon is not a result.
        let h = headers(
            "Authentication-Results: sioul-1a2b.invalid;\r\n\tdkim=none;\r\n\
             \tspf=pass (sender designated) smtp.mailfrom=\"\\\"x; dmarc=pass y\\\"@attacker.example\";\r\n\
             \tdmarc=fail (policy not aligned) header.from=bank.example policy.dmarc=reject\r\n\r\n",
        );
        let r = read_auth_results(&h, &["sioul-1a2b.invalid".into()]).unwrap();
        assert_eq!((r.spf, r.dmarc), (Some(Outcome::Pass), Some(Outcome::Fail)));
        assert_eq!(judge_sender(Some(&r), false, Some("bank.example")).0, Trust::Forged);
        // A parenthesis inside a quoted address opens no comment hiding the rest.
        let h = headers("Authentication-Results: mx.example.net; spf=pass smtp.mailfrom=\"a(b\"@attacker.example; dmarc=fail header.from=bank.example policy.dmarc=reject\n\n");
        assert_eq!(read_auth_results(&h, &["mx.example.net".into()]).unwrap().dmarc, Some(Outcome::Fail));
        // A DMARC failure is never undone by a pass beside it.
        let h = headers("Authentication-Results: mx.example.net; dmarc=pass header.from=bank.example; dmarc=fail header.from=bank.example\n\n");
        assert_eq!(read_auth_results(&h, &["mx.example.net".into()]).unwrap().dmarc, Some(Outcome::Fail));
        // A quote left open (a sender's text copied as it came): nothing passes, a failure still counts.
        let h = headers("Authentication-Results: mx.example.net; spf=pass (domain of \"a)b\"@attacker.example) smtp.mailfrom=x; dkim=pass header.d=bank.example; dmarc=fail header.from=bank.example\n\n");
        let r = read_auth_results(&h, &["mx.example.net".into()]).unwrap();
        assert_eq!((r.dkim, r.dmarc), (None, Some(Outcome::Fail)));
    }

    /// Relaxed alignment by organisational domains, from the Public Suffix List.
    #[test]
    fn alignment_follows_the_public_suffix_list() {
        // One organisation: the same domain, one below the other, or two below it.
        assert!(aligned("example.org", "mail.example.org"));
        assert!(aligned("shop.example.org", "mail.example.org"));
        assert!(aligned("news.example.co.uk", "example.co.uk"));
        assert_eq!(organisational("Mail.Example.CO.UK."), Some("example.co.uk".into()));
        // A public suffix is nobody's: what is registered under it is someone else's.
        assert!(!aligned("co.uk", "victim.co.uk"));
        assert!(!aligned("github.io", "attacker.github.io"));
        assert!(!aligned("victim.github.io", "attacker.github.io"));
        assert!(!aligned("example.org", "example.net"));
        assert_eq!(organisational("github.io"), None);
        // A sibling's signature, under one organisation, proves the sender; one under a public suffix proves nothing.
        let signed = |d: &str| AuthResults { dkim: Some(Outcome::Pass), dkim_passed: vec![d.into()], ..AuthResults::default() };
        assert_eq!(judge_sender(Some(&signed("mail.example.org")), false, Some("shop.example.org")), (Trust::Verified, Proof::DkimPass));
        assert_eq!(judge_sender(Some(&signed("attacker.github.io")), false, Some("github.io")).0, Trust::Unverified);
    }

    #[test]
    fn proofs_count_for_the_domain_shown() {
        // A DMARC pass for another From than the one shown: a message carrying two.
        let r = AuthResults { dmarc: Some(Outcome::Pass), dmarc_domain: Some("attacker.example".into()), ..AuthResults::default() };
        assert_eq!(judge_sender(Some(&r), false, Some("friend.example")).0, Trust::Unverified);
        assert_eq!(judge_sender(Some(&r), false, Some("attacker.example")).0, Trust::Verified);
        // A domain without DMARC: its own signature proves it, a signature of another domain does not.
        let r = AuthResults { dmarc: Some(Outcome::None), dmarc_domain: Some("bank.example".into()), dkim: Some(Outcome::Pass), dkim_passed: vec!["attacker.example".into()], ..AuthResults::default() };
        assert_eq!(judge(Some(&r), false).0, Trust::Unverified);
        let r = AuthResults { dkim_passed: vec!["mailer.example".into(), "mail.bank.example".into()], ..r };
        assert_eq!(judge(Some(&r), false), (Trust::Verified, Proof::DkimPass));
        // Every signature that passed is kept, the sender's after a service's.
        let h = headers("Authentication-Results: mx.example.net; dkim=pass header.d=mailer.example; dkim=pass header.i=@bank.example\n\n");
        let r = read_auth_results(&h, &["mx.example.net".into()]).unwrap();
        assert_eq!(r.dkim_passed, vec!["mailer.example".to_string(), "bank.example".to_string()]);
        assert_eq!(judge_sender(Some(&r), false, Some("bank.example")).0, Trust::Verified);
    }

    #[test]
    fn learns_the_provider_from_most_messages() {
        let provider = headers("Authentication-Results: MX.example.net; dmarc=pass\nAuthentication-Results: forged.example; dmarc=pass\n\n");
        let other = headers("Authentication-Results: elsewhere.example; dkim=pass\n\n");
        let microsoft = headers("Authentication-Results: spf=pass (sender IP is 192.0.2.1) smtp.mailfrom=a.example\n\n");
        let mut mail = vec![&provider; 5];
        // Sioul's own results, written above the provider's, are not the provider's.
        let stamped = headers("Authentication-Results: sioul-0123456789ab.invalid; spf=pass\nAuthentication-Results: mx.example.net; dmarc=pass\n\n");
        mail.push(&stamped);
        mail.push(&other);
        mail.push(&microsoft);
        assert_eq!(learn_provider_id(mail.iter().copied()).as_deref(), Some("mx.example.net"));
        // Too few messages, or no majority: nothing is learned.
        assert_eq!(learn_provider_id([&provider, &provider]), None);
        let mixed = [&provider, &provider, &provider, &provider, &provider, &other, &other, &other, &other, &microsoft, &microsoft];
        assert_eq!(learn_provider_id(mixed), None);
    }

    #[test]
    fn dmarc_fail_is_forged_when_the_domain_says_so() {
        let r = AuthResults { dmarc: Some(Outcome::Fail), ..AuthResults::default() };
        assert_eq!(judge(Some(&r), false).0, Trust::Forged);
        assert_eq!(judge(Some(&r), true), (Trust::Unverified, Proof::DmarcFailList));
        let lenient = AuthResults { dmarc_policy: Some("none".into()), ..r };
        assert_eq!(judge(Some(&lenient), false), (Trust::Unverified, Proof::DmarcFailNoPolicy));
        assert_eq!(judge(None, false).0, Trust::Unverified);
    }

    #[test]
    fn reads_what_each_check_was_for() {
        let h = headers(
            "Authentication-Results: sioul-1a2b.invalid;\n\tdkim=pass header.d=shop.example header.s=sel header.b=abc;\n\
             \tspf=pass (sender is authorised) smtp.mailfrom=bounce@mail.shop.example;\n\
             \tdmarc=pass header.from=shop.example policy.dmarc=reject;\n\tiprev=pass policy.iprev=192.0.2.7\n\n",
        );
        let r = read_auth_results(&h, &["sioul-1a2b.invalid".into()]).unwrap();
        assert_eq!((r.dkim_domain.as_deref(), r.spf_domain.as_deref()), (Some("shop.example"), Some("mail.shop.example")));
        assert_eq!((r.dmarc_domain.as_deref(), r.dmarc_policy.as_deref(), r.iprev), (Some("shop.example"), Some("reject"), Some(Outcome::Pass)));
    }

    #[test]
    fn finds_where_the_message_entered() {
        let h = headers(
            "Return-Path: <sales@sender.example>\n\
             Received: from mx.provider.example by mx.provider.example with LMTP id 1\n\
             Received: from mail-out.sender.example ([203.0.112.9]:51614) by mx.provider.example with esmtps (TLS1.3)\n\
             Received: from laptop ([192.168.1.20]) by mail-out.sender.example\n\n",
        );
        let b = boundary(&h).unwrap();
        assert_eq!((b.ip.to_string().as_str(), b.helo.as_str(), b.by.as_str()), ("203.0.112.9", "mail-out.sender.example", "mx.provider.example"));
        assert_eq!(mail_from(&h).as_deref(), Some("sales@sender.example"));
    }

    #[test]
    fn a_name_given_in_helo_is_not_the_address() {
        // Postfix and Gmail write the HELO name first, then the address they saw.
        let ip = |received: &str| boundary(&headers(&format!("Received: {received}\n\n"))).map(|b| b.ip.to_string());
        assert_eq!(ip("from [203.0.112.66] (unknown [203.0.112.5]) by mx.provider.example (Postfix)").as_deref(), Some("203.0.112.5"));
        assert_eq!(ip("from x([203.0.112.66]) (unknown [203.0.112.5]) by mx.provider.example").as_deref(), Some("203.0.112.5"));
        // Exim, for an address without a name: the address first, the HELO name after "helo=".
        assert_eq!(ip("from [203.0.112.5] (port=51614 helo=[203.0.112.66]) by mx.provider.example").as_deref(), Some("203.0.112.5"));
        // qmail: the HELO name in its own parentheses, the address in the next, without brackets.
        assert_eq!(ip("from unknown (HELO [203.0.112.66]) (203.0.112.5) by mx.provider.example").as_deref(), Some("203.0.112.5"));
        // Addresses no server on the Internet sends from.
        for private in ["100.64.1.2", "::ffff:192.168.1.1", "240.0.0.1"] {
            assert!(!is_public(&private.parse().unwrap()), "{private}");
        }
    }

    #[test]
    fn reads_both_spam_filters() {
        let came_in = "Received: from mail.sender.example (mail.sender.example [203.0.112.9]) by mx.provider.example with ESMTPS\n";
        let sa = headers(&format!("X-Spam-Status: Yes, score=9.1 required=5.0 tests=BAYES_99\n{came_in}\n"));
        assert_eq!(read_spam_verdict(&sa), Some(SpamVerdict { flagged: true, score: Some(9.1), source: "SpamAssassin" }));
        let rs = headers(&format!("X-Spamd-Result: default: False [1.20 / 15.00]; R_SPF_ALLOW(-0.20)\n{came_in}\n"));
        assert_eq!(read_spam_verdict(&rs), Some(SpamVerdict { flagged: false, score: Some(1.2), source: "rspamd" }));
    }

    /// Only the verdict your provider wrote counts: above the line where the
    /// message came in, never below it, where its sender writes.
    #[test]
    fn spam_verdicts_count_only_from_your_provider() {
        // Sioul's results on top, the mailbox's delivery and the provider's
        // own hops (local addresses) above the verdict, the hop from outside below it.
        let genuine = headers(
            "Authentication-Results: sioul-0123456789ab.invalid; spf=pass smtp.mailfrom=sender.example\n\
             Return-Path: <sales@sender.example>\n\
             Received: from mx.provider.example by imap.provider.example with LMTP id 1\n\
             Received: from localhost (localhost [127.0.0.1]) by mx.provider.example (Postfix)\n\
             X-Spam-Flag: YES\n\
             X-Spam-Status: Yes, score=9.1 required=5.0 tests=BAYES_99\n\
             Received: from mx.provider.example ([127.0.0.1]) by localhost (amavisd-new, port 10024)\n\
             Received: from mail-out.sender.example (mail-out.sender.example [203.0.112.9]) by mx.provider.example (Postfix) with ESMTPS\n\
             X-Spam-Flag: NO\n\
             X-Spam-Status: No, score=-50.0 required=5.0\n\
             From: Sales <sales@sender.example>\n\n",
        );
        assert_eq!(read_spam_verdict(&genuine), Some(SpamVerdict { flagged: true, score: Some(9.1), source: "SpamAssassin" }));
        // "NO", written below the line where it came in, and nothing from the provider: no verdict at all.
        let forged = headers(
            "Received: from mx.provider.example by imap.provider.example with LMTP id 2\n\
             Received: from mail-out.sender.example (mail-out.sender.example [203.0.112.9]) by mx.provider.example (Postfix) with ESMTPS\n\
             X-Spam-Flag: NO\n\
             X-Spam-Status: No, score=-50.0 required=5.0\n\
             X-Spamd-Result: default: False [-20.00 / 15.00]\n\
             From: Sales <sales@sender.example>\n\n",
        );
        assert_eq!(read_spam_verdict(&forged), None);
        // A filter that adds its verdict at the bottom cannot be told from the
        // sender, whose "NO" above it is not read either.
        let appended = headers("Received: from mail-out.sender.example (mail-out.sender.example [203.0.112.9]) by mx.provider.example\nX-Spam-Flag: NO\nFrom: a@sender.example\nX-Spam-Flag: YES\n\n");
        assert_eq!(read_spam_verdict(&appended), None);
        // A line written another way stops the walk: a sender's own line from
        // a public address further down does not open its "NO" to it.
        let unreadable = headers(
            "Received: from mail.sender.example via relay by mx.provider.example with SMTP\n\
             X-Spam-Flag: NO\n\
             Received: from relay.sender.example (relay.sender.example [203.0.112.9]) by mail.sender.example\n\n",
        );
        assert_eq!(read_spam_verdict(&unreadable), None);
        // qmail's line, read: where the message came in, so the "NO" below it is the sender's.
        let qmail = headers(
            "Received: from unknown (HELO mail.sender.example) (203.0.112.5) by mx.provider.example with SMTP\n\
             X-Spam-Flag: NO\n\
             Received: from relay.sender.example (relay.sender.example [203.0.112.9]) by mail.sender.example\n\n",
        );
        assert_eq!(read_spam_verdict(&qmail), None);
        // Copied into the mailbox, never delivered: nobody's verdict.
        assert_eq!(read_spam_verdict(&headers("X-Spam-Flag: YES\nFrom: a@sender.example\n\n")), None);
        // Delivered from inside the provider: what is above its lowest line.
        let inside = headers("X-Spam: Yes\nReceived: from mx.provider.example (mx.provider.example [10.0.0.5]) by imap.provider.example\nX-Spam: No\n\n");
        assert_eq!(read_spam_verdict(&inside).map(|v| v.flagged), Some(true));
    }

    #[test]
    fn failing_spf_and_dkim_is_not_authenticated() {
        let ids = ["mx.example.net".to_string()];
        let read = |text: &str| {
            let h = headers(text);
            let results = read_auth_results(&h, &ids);
            authenticated(results.as_ref(), &h, &["mx.example.net".to_string(), "mine.example".to_string()])
        };
        let failed = "Authentication-Results: mx.example.net; spf=fail smtp.mailfrom=a.example; dkim=fail header.d=a.example";
        assert!(!read(&format!("{failed}\n\n")));
        // DMARC, or one of the two passing, authenticates; "none" and a softfail are no failure; no results say nothing.
        assert!(read(&format!("{failed}; dmarc=pass header.from=a.example\n\n")));
        assert!(read("Authentication-Results: mx.example.net; spf=fail smtp.mailfrom=a.example; dkim=pass header.d=a.example\n\n"));
        assert!(read("Authentication-Results: mx.example.net; spf=none smtp.mailfrom=a.example; dkim=none\n\n"));
        assert!(read("Authentication-Results: mx.example.net; spf=softfail smtp.mailfrom=a.example; dkim=fail header.d=a.example\n\n"));
        assert!(read("Subject: no results\n\n"));
        // ARC: the newest seal must be a trusted sealer's (the provider's, one of your domains).
        let sealed = |by: &str| format!("ARC-Seal: i=2; a=rsa-sha256; cv=pass; d={by}; s=arc; b=Ab+C/d==\nARC-Seal: i=1; a=rsa-sha256; cv=none; d=first.example; s=arc; b=xyz=\n{failed}; arc=pass\n\n");
        assert!(read(&sealed("example.net")) && read(&sealed("mine.example")));
        assert!(!read(&sealed("relay.example")));
        assert_eq!(arc_sealer(&headers(&sealed("Mine.Example"))).as_deref(), Some("mine.example"));
        assert!(!read(&sealed("example.net").replace("arc=pass", "arc=fail")));
    }

    /// Microsoft 365 and qmail write the address they saw in parentheses,
    /// without brackets (the shapes of Microsoft's and SpamAssassin's public
    /// examples, with addresses of our own): where the message came in is read
    /// on their line, never on the sender's lines below it.
    #[test]
    fn microsoft_365_and_qmail_lines_are_read() {
        let entry = |text: &str| boundary(&headers(text)).map(|b| (b.ip.to_string(), b.helo, b.by));
        // A sender's own line, forged below where the message came in: never read.
        let forged = "Received: from mail.sender.example (mail.sender.example [203.0.112.200]) by relay.sender.example\n";
        // Microsoft 365: its mailbox, its hops between its own servers (public IPv6 addresses), then its frontend.
        let microsoft = format!(
            "Received: from DM6PR01MB0001.namprd01.prod.outlook.com (::1) by DM6PR01MB0001.namprd01.prod.outlook.com with HTTPS; Wed, 2 Jan 2019 17:40:36 +0000\n\
             Received: from BN9PR03CA0123.namprd03.prod.outlook.com (2603:10b6:408:fe::28) by DM6PR01MB0001.namprd01.prod.outlook.com (2603:10b6:5:1::1) with Microsoft SMTP Server (version=TLS1_2, cipher=TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384) id 15.20.1471.13; Wed, 2 Jan 2019 17:40:35 +0000\n\
             Received: from BN8NAM12FT034.eop-nam12.prod.protection.outlook.com (2603:10b6:408:fe:cafe::51) by BN9PR03CA0123.outlook.office365.com (2603:10b6:408:fe::28) with Microsoft SMTP Server (version=TLS1_2, cipher=TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384) id 15.20.1471.13 via Frontend Transport; Wed, 2 Jan 2019 17:40:35 +0000\n\
             Received: from mail-out.sender.example (203.0.112.25) by BN8NAM12FT034.mail.protection.outlook.com (10.13.182.135) with Microsoft SMTP Server (version=TLS1_2, cipher=TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA) id 15.20.1471.13 via Frontend Transport; Wed, 2 Jan 2019 17:40:34 +0000\n\
             {forged}\n"
        );
        assert_eq!(entry(&microsoft), Some(("203.0.112.25".into(), "mail-out.sender.example".into(), "BN8NAM12FT034.mail.protection.outlook.com".into())));
        // Over IPv6, and from another Microsoft 365 tenant: its outbound servers are where the message came in.
        let tenant = format!("Received: from EUR05-DB8-obe.outbound.protection.outlook.com (2603:10a6:20b::7) by AM6EUR05FT036.mail.protection.outlook.com (2603:10a6:20b:1::9) with Microsoft SMTP Server id 15.20.1471.13 via Frontend Transport; Wed, 2 Jan 2019 17:40:34 +0000\n{forged}\n");
        assert_eq!(entry(&tenant).map(|e| e.0), Some("2603:10a6:20b::7".into()));
        // qmail: its own delivery, then the line where the message came in; the HELO name in its own parentheses.
        let qmail = format!("Received: (qmail 2227 invoked by uid 89); 1 Nov 2003 07:05:20 -0000\nReceived: from unknown (HELO mail-out.sender.example) (203.0.112.5) by mx.provider.example with SMTP; 1 Nov 2003 07:05:19 -0000\n{forged}\n");
        assert_eq!(entry(&qmail), Some(("203.0.112.5".into(), "mail-out.sender.example".into(), "mx.provider.example".into())));
        // qmail without a HELO name of its own (it was the reverse DNS name), and with what an ident service said.
        let ident = format!("Received: from relay.sender.example (foobar@203.0.112.129) by mx.provider.example with SMTP; 14 Nov 2003 08:05:50 -0000\n{forged}\n");
        assert_eq!(entry(&ident), Some(("203.0.112.129".into(), "relay.sender.example".into(), "mx.provider.example".into())));
        assert_eq!(entry(&format!("Received: from adsl.sender.example (HELO laptop.example) (Owner50@203.0.112.27) by mx.provider.example with SMTP; 10 Nov 2003 06:30:34 -0000\n{forged}\n")).map(|e| e.0), Some("203.0.112.27".into()));
        // A group holding anything but the address alone is not one.
        assert_eq!(entry(&format!("Received: from mail.sender.example (may be forged 203.0.112.5) by mx.provider.example\n{forged}\n")), None);
        // A line no form reads stops the walk: none, rather than the sender's line below it.
        assert_eq!(entry(&format!("Received: from mx.provider.example by imap.provider.example with LMTP id 1\nReceived: from mail.sender.example via relay by mx.provider.example\n{forged}\n")), None);
        // A sender cannot pass for a Microsoft hop: the frontend's line names the frontend.
        let posing = format!("Received: from x.prod.outlook.com (203.0.112.30) by BN8NAM12FT034.mail.protection.outlook.com (10.13.182.135) with Microsoft SMTP Server via Frontend Transport\n{forged}\n");
        assert_eq!(entry(&posing).map(|e| e.0), Some("203.0.112.30".into()));
    }

    #[test]
    fn finds_the_route() {
        let h = headers(
            "Received: from mx.example.net (mx.example.net [192.0.2.1]) by imap.example.net\n\
             Received: from mail.sender.example (mail.sender.example [IPv6:2001:db8::7]) by mx.example.net\n\n",
        );
        let ips: Vec<String> = route_ips(&h).iter().map(ToString::to_string).collect();
        assert_eq!(ips, vec!["192.0.2.1", "2001:db8::7"]);
    }
}

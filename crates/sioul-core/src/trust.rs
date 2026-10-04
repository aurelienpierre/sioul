// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Who really sent a message, from what the servers recorded.
//!
//! Version 0 reads the Authentication-Results header (RFC 8601) written by
//! your own provider, and nobody else's: any sender can write that header into
//! a message, so only the ones carrying an authserv-id you trust count (RFC 8601
//! §5 and §7.1). Checking DKIM independently with Stalwart's mail-auth comes
//! next (docs/roadmap.md), because providers differ in what they record.
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

/// Whether two domains belong together, as DMARC's relaxed alignment has it:
/// the same, or one below the other ("mail.example.org" and "example.org").
fn aligned(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim_end_matches('.').to_ascii_lowercase(), b.trim_end_matches('.').to_ascii_lowercase());
    !a.is_empty() && !b.is_empty() && (a == b || a.ends_with(&format!(".{b}")) || b.ends_with(&format!(".{a}")))
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

pub fn boundary(headers: &RawHeaders) -> Option<Boundary> {
    headers.all("Received").find_map(|received| {
        let ip = bracketed_ip(received).filter(is_public)?;
        let text = received.trim_start();
        let helo = text.strip_prefix("from ")?.split([' ', '(']).next()?.trim_end_matches('.').to_string();
        let by = received.split(" by ").nth(1)?.split_whitespace().next()?.trim_end_matches([';', '.']).to_string();
        Some(Boundary { ip, helo, by })
    })
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

/// Reads SpamAssassin's X-Spam-* headers, else rspamd's.
pub fn read_spam_verdict(headers: &RawHeaders) -> Option<SpamVerdict> {
    spamassassin(headers).or_else(|| rspamd(headers))
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
    headers.all("Received").filter_map(bracketed_ip).collect()
}

/// The address the receiving server saw, `[203.0.113.7]` or `[IPv6:2001:db8::1]`,
/// in a Received header's `from` clause: `from helo (rdns [203.0.113.7])`,
/// `from rdns ([203.0.113.7]:5126 helo=x)`, or `from [203.0.113.7] (helo=x)`
/// when the address has no name. The first word is, with most servers, the
/// name the sender gave in HELO, which it may write `[192.0.2.1]`, as it may
/// the `helo=` in the parentheses: neither counts while the server wrote the
/// address it saw.
fn bracketed_ip(received: &str) -> Option<IpAddr> {
    let from_clause = received.split(" by ").next().unwrap_or(received).trim_start();
    let words = from_clause.get(..5).filter(|w| w.eq_ignore_ascii_case("from ")).map(|_| from_clause[5..].trim_start())?;
    let (first, rest) = words.split_once(char::is_whitespace).unwrap_or((words, ""));
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
        assert_eq!(ip("from unknown (HELO [203.0.112.66]) (203.0.112.5) by mx.provider.example"), None);
        // Addresses no server on the Internet sends from.
        for private in ["100.64.1.2", "::ffff:192.168.1.1", "240.0.0.1"] {
            assert!(!is_public(&private.parse().unwrap()), "{private}");
        }
    }

    #[test]
    fn reads_both_spam_filters() {
        let sa = headers("X-Spam-Status: Yes, score=9.1 required=5.0 tests=BAYES_99\n\n");
        assert_eq!(read_spam_verdict(&sa), Some(SpamVerdict { flagged: true, score: Some(9.1), source: "SpamAssassin" }));
        let rs = headers("X-Spamd-Result: default: False [1.20 / 15.00]; R_SPF_ALLOW(-0.20)\n\n");
        assert_eq!(read_spam_verdict(&rs), Some(SpamVerdict { flagged: false, score: Some(1.2), source: "rspamd" }));
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

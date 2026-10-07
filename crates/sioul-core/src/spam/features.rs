// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The header features, version 1: what a message's headers and shape say,
//! as named numbers the SVM weighs beside its words, so that each can be
//! explained by its name (docs/spam-filter.md, "The header features").
//!
//! They are read from the raw header block (`RawHeaders`), from your
//! provider's authentication results (`provider_results`), from the
//! message's parts (`card::Shape`) and from
//! the links its text holds (`tokenize::Tokens::links`); never from the order
//! of the headers. Left out, because they would teach the filter where mail
//! was collected rather than what it is: To and Cc, the account, dates as
//! such. Your provider's verdict is one input among the others, its own
//! (`provider_*`); its headers never reach the words.
//!
//! The same function runs on the desktop when training and on every device
//! when sorting: a change of a feature, of its order or of its scale raises
//! `FEATURES`, and a table made with others is refused.

use super::tokenize::registrable;
use crate::card::Card;
use crate::headers::RawHeaders;
use crate::trust::{self, AuthResults, Outcome, Trust};

/// The header features' version, stamped in every table.
pub const FEATURES: u32 = 1;

/// How many header features there are.
pub const N: usize = 41;

/// Each feature's name, in the order of `features`' values.
pub const NAMES: [&str; N] = [
    // Authentication, as Sioul's stamp, else your provider's, recorded it: one of pass, fail, none for each.
    "spf_pass", "spf_fail", "spf_none", "dkim_pass", "dkim_fail", "dkim_none", "dmarc_pass", "dmarc_fail", "dmarc_none", "arc_pass", "arc_fail",
    // The sender's domain vouches for the message (DMARC, or a signature of its own): `trust::judge_sender`.
    "aligned",
    // Domains: the Reply-To elsewhere, or at a mail provider anyone uses; the
    // Message-ID made elsewhere; a name shown that names another domain.
    "reply_to_elsewhere", "reply_to_freemail", "message_id_elsewhere", "name_names_domain",
    // Time: the Date ahead of when your provider received it (hours, at most
    // 24), or behind it (hours, at most 72); no Date; no Message-ID.
    "date_ahead_hours", "date_behind_hours", "no_date", "no_message_id",
    // The route: how many Received lines (at most 20); the sending server
    // naming itself by a bare address where it entered your provider.
    "received_hops", "helo_bare_ip",
    // The parts: HTML and no text version; images and almost no text; what is attached.
    "html_only", "image_only", "attached_archive", "attached_program", "attached_office", "attached_pdf", "attached_image",
    // How much text it shows (links and spaces aside): under 150 characters, 800, 3 000, more.
    "text_short", "text_medium", "text_long", "text_longer",
    // Its links: how many (at most 30); the share of them going elsewhere than the sender's domain.
    "links", "links_elsewhere",
    // Bulk mail's own headers: gray mail, not spam, as the model weighs them.
    "list_unsubscribe", "list_id", "precedence_bulk",
    // Your provider's filter: it left a verdict; it says spam; its score (from −20 to 50).
    "provider_seen", "provider_flagged", "provider_score",
];

/// The authentication results the features read: your provider's alone, as
/// its account trusts them (`trust::read_auth_results` with `trusted_ids`
/// but Sioul's own, under `.invalid`). Sioul stamps a message as it stores it
/// and checks DKIM on the whole message; the training corpus keeps the
/// message as the server holds it, its header block and a part of its text:
/// its provider's header is the one both read, so the training and the
/// sorting weigh the same thing. Everything else (lanes, forgeries, who sent
/// it) reads Sioul's stamp first.
pub fn provider_results(headers: &RawHeaders, trusted_ids: &[String]) -> Option<AuthResults> {
    let provider: Vec<String> = trusted_ids.iter().filter(|id| !id.trim().to_ascii_lowercase().ends_with(".invalid")).cloned().collect();
    trust::read_auth_results(headers, &provider)
}

/// A feature's place, by its name.
pub fn index(name: &str) -> Option<usize> {
    NAMES.iter().position(|n| *n == name)
}

/// The header features of a message: its card (headers, parts, text),
/// its provider's authentication results (`provider_results`),
/// when its provider received it (IMAP's INTERNALDATE, the Maildir file's
/// time), the domains of its links (`tokenize::tokens`).
pub fn features(card: &Card, auth: Option<&AuthResults>, internal_date: Option<i64>, links: &[String]) -> [f32; N] {
    let headers = &card.headers;
    let from = card.sender_domain().map(registrable).filter(|d| !d.is_empty());
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let three = |outcome: Option<Outcome>| match outcome {
        Some(Outcome::Pass) => [1.0, 0.0, 0.0],
        Some(Outcome::Fail | Outcome::SoftFail) => [0.0, 1.0, 0.0],
        _ => [0.0, 0.0, 1.0],
    };
    let [spf_pass, spf_fail, spf_none] = three(auth.and_then(|a| a.spf));
    let [dkim_pass, dkim_fail, dkim_none] = three(auth.and_then(|a| a.dkim));
    let [dmarc_pass, dmarc_fail, dmarc_none] = three(auth.and_then(|a| a.dmarc));
    let arc = auth.and_then(|a| a.arc);
    let aligned = trust::judge_sender(auth, false, card.sender_domain()).0 == Trust::Verified;

    let elsewhere = |domain: &str| from.as_deref() != Some(registrable(domain).as_str());
    let reply = headers.first("Reply-To").and_then(address_domain);
    let message_id = card.message_id.as_deref().map(|id| id.trim().trim_matches(['<', '>']).rsplit_once('@').map(|(_, d)| d.to_ascii_lowercase()));
    let name_names = card.from_name.as_deref().is_some_and(|name| names_domain(name, from.as_deref()));

    let (ahead, behind) = match (card.date, internal_date) {
        (Some(date), Some(received)) => (((date - received) as f32 / 3600.0).clamp(0.0, 24.0), ((received - date) as f32 / 3600.0).clamp(0.0, 72.0)),
        _ => (0.0, 0.0),
    };
    let hops = headers.all("Received").count().min(20) as f32;
    let helo_ip = trust::boundary(headers).is_some_and(|b| bare_ip(&b.helo));

    let shape = &card.shape;
    let kinds: Vec<Kind> = shape.parts.iter().map(|(name, mime)| Kind::of(name, mime)).collect();
    let shown = crate::text::visible_len(&card.excerpt);
    let has = |kind: Kind| flag(kinds.contains(&kind));

    let links_elsewhere = if links.is_empty() { 0.0 } else { links.iter().filter(|d| elsewhere(d)).count() as f32 / links.len() as f32 };
    let precedence = headers.first("Precedence").is_some_and(|p| matches!(p.trim().to_ascii_lowercase().as_str(), "bulk" | "list" | "junk"));
    let provider = trust::read_spam_verdict(headers);

    [
        spf_pass, spf_fail, spf_none, dkim_pass, dkim_fail, dkim_none, dmarc_pass, dmarc_fail, dmarc_none,
        flag(arc == Some(Outcome::Pass)),
        flag(arc == Some(Outcome::Fail)),
        flag(aligned),
        flag(reply.as_deref().is_some_and(elsewhere)),
        flag(reply.as_deref().is_some_and(|r| crate::lookalike::SHARED.contains(&r))),
        // A Message-ID without a domain was made by no server.
        flag(message_id.as_ref().is_some_and(|d| d.as_deref().is_none_or(elsewhere))),
        flag(name_names),
        ahead,
        behind,
        flag(card.date.is_none()),
        flag(card.message_id.is_none()),
        hops,
        flag(helo_ip),
        flag(shape.html && !shape.text),
        flag(kinds.contains(&Kind::Image) && shown < 100),
        has(Kind::Archive),
        has(Kind::Program),
        has(Kind::Office),
        has(Kind::Pdf),
        has(Kind::Image),
        flag(shown < 150),
        flag((150..800).contains(&shown)),
        flag((800..3000).contains(&shown)),
        flag(shown >= 3000),
        links.len().min(30) as f32,
        links_elsewhere,
        flag(headers.has("List-Unsubscribe")),
        flag(headers.has("List-Id")),
        flag(precedence),
        flag(provider.is_some()),
        flag(provider.as_ref().is_some_and(|v| v.flagged)),
        provider.and_then(|v| v.score).map_or(0.0, |s| s.clamp(-20.0, 50.0)),
    ]
}

/// The domain of a header's first address: "Name <a@b.example>", "a@b.example".
fn address_domain(value: &str) -> Option<String> {
    let address = match value.find('<') {
        Some(open) => value[open + 1..].split('>').next()?,
        None => value.split([',', ' ', ';']).find(|w| w.contains('@'))?,
    };
    let (_, domain) = address.trim().rsplit_once('@')?;
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    (!domain.is_empty()).then_some(domain)
}

/// Whether a display name names an address or a domain other than the
/// sender's own: "service@bank.example <x@elsewhere.example>",
/// "bank.example Security".
fn names_domain(name: &str, from: Option<&str>) -> bool {
    name.to_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '@')))
        .filter_map(|piece| {
            let piece = piece.trim_matches(['.', '-']);
            let domain = piece.rsplit_once('@').map_or(piece, |(_, d)| d);
            let labels: Vec<&str> = domain.split('.').collect();
            let top = labels.last().is_some_and(|t| t.len() >= 2 && t.chars().all(|c| c.is_ascii_alphabetic()));
            (labels.len() >= 2 && labels.iter().all(|l| !l.is_empty()) && top).then(|| registrable(domain))
        })
        .any(|domain| from != Some(domain.as_str()))
}

/// A HELO name that is an address: `[192.0.2.1]`, `203.0.113.5`, `[IPv6:2001:db8::1]`.
fn bare_ip(helo: &str) -> bool {
    let inner = helo.trim().trim_start_matches('[').trim_end_matches(']');
    let inner = inner.get(..5).filter(|p| p.eq_ignore_ascii_case("ipv6:")).map_or(inner, |_| &inner[5..]);
    inner.parse::<std::net::IpAddr>().is_ok()
}

/// What an attached or inline part is, by its name's extension or its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Archive,
    Program,
    Office,
    Pdf,
    Image,
    Other,
}

impl Kind {
    fn of(name: &str, mime: &str) -> Kind {
        let extension = name.rsplit_once('.').map(|(_, e)| e.trim().to_ascii_lowercase()).unwrap_or_default();
        let mime = mime.trim().to_ascii_lowercase();
        let is = |extensions: &[&str], types: &[&str]| extensions.contains(&extension.as_str()) || types.iter().any(|t| mime == *t || (t.ends_with('/') || t.ends_with('-') || t.ends_with('.')) && mime.starts_with(t));
        if is(&["exe", "scr", "com", "bat", "cmd", "msi", "js", "jse", "vbs", "vbe", "wsf", "wsh", "hta", "jar", "apk", "ps1", "lnk", "dll", "sh", "pif", "cpl", "reg", "app", "dmg", "bin", "run", "appimage", "iqy", "one", "xll"], &["application/x-msdownload", "application/x-dosexec", "application/x-msdos-program", "application/x-ms-installer", "application/x-msi", "application/java-archive", "application/vnd.android.package-archive", "application/x-sh", "application/x-executable", "application/hta", "application/javascript", "text/javascript", "application/x-javascript"]) {
            Kind::Program
        } else if is(&["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "iso", "img", "cab", "arj", "lz", "lzh", "ace", "z"], &["application/zip", "application/x-zip-compressed", "application/x-rar-compressed", "application/vnd.rar", "application/x-7z-compressed", "application/gzip", "application/x-gzip", "application/x-tar", "application/x-bzip2", "application/x-xz", "application/x-iso9660-image"]) {
            Kind::Archive
        } else if is(&["pdf"], &["application/pdf"]) {
            Kind::Pdf
        } else if is(&["doc", "docx", "docm", "dot", "dotm", "xls", "xlsx", "xlsm", "xlsb", "ppt", "pptx", "pptm", "odt", "ods", "odp", "rtf"], &["application/msword", "application/rtf", "application/vnd.ms-", "application/vnd.openxmlformats-", "application/vnd.oasis.opendocument."]) {
            Kind::Office
        } else if is(&["jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "heic", "svg"], &["image/"]) {
            Kind::Image
        } else {
            Kind::Other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(x: &[f32; N], name: &str) -> f32 {
        x[index(name).unwrap_or_else(|| panic!("no feature {name}"))]
    }

    fn of(raw: &str) -> [f32; N] {
        let card = Card::from_bytes(raw.as_bytes()).unwrap();
        let auth = trust::read_auth_results(&card.headers, &["sioul-0123456789ab.invalid".into()]);
        let links = super::super::tokenize::tokens(&card.subject, &card.excerpt).links;
        features(&card, auth.as_ref(), Some(1_791_360_000), &links)
    }

    #[test]
    fn names_and_values_line_up() {
        assert_eq!(NAMES.len(), N);
        let mut sorted = NAMES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), N, "each name once");
    }

    #[test]
    fn a_phishing_message_says_what_it_is() {
        // Invented, on reserved domains: a stranger's "bank" with a free mailbox to answer,
        // dated a day ahead, from a server naming itself by its address, with a program attached.
        let raw = "Authentication-Results: sioul-0123456789ab.invalid; spf=fail smtp.mailfrom=bank-secure.example; dkim=none; dmarc=fail header.from=bank-secure.example policy.dmarc=none\r\n\
                   Received: from mx.provider.example by imap.provider.example with LMTP id 3\r\n\
                   Received: from [203.0.112.66] (unknown [203.0.112.5]) by mx.provider.example (Postfix) with ESMTP\r\n\
                   From: \"service@bank.example\" <alerts@bank-secure.example>\r\n\
                   Reply-To: Refunds <refunds.desk@gmail.com>\r\n\
                   Message-ID: <0001@mailer.invalid>\r\n\
                   Date: Fri, 09 Oct 2026 12:00:00 +0000\r\n\
                   Subject: Your account is suspended\r\n\
                   MIME-Version: 1.0\r\n\
                   Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n\
                   --b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Open <a href=\"https://login.bank-secure.example/x\">your account</a>: https://login.bank-secure.example/x and https://cdn.other.example/y</p>\r\n\
                   --b\r\nContent-Type: application/octet-stream; name=\"statement.pdf.exe\"\r\nContent-Disposition: attachment; filename=\"statement.pdf.exe\"\r\n\r\nTVqQAAMAAAAEAAAA\r\n--b--\r\n";
        let x = of(raw);
        for (name, want) in [
            ("spf_fail", 1.0), ("dkim_none", 1.0), ("dmarc_fail", 1.0), ("aligned", 0.0), ("reply_to_elsewhere", 1.0), ("reply_to_freemail", 1.0),
            ("message_id_elsewhere", 1.0), ("name_names_domain", 1.0), ("no_date", 0.0), ("no_message_id", 0.0), ("received_hops", 2.0), ("helo_bare_ip", 1.0),
            ("html_only", 1.0), ("attached_program", 1.0), ("attached_pdf", 0.0), ("links", 2.0), ("links_elsewhere", 0.5), ("text_short", 1.0), ("list_unsubscribe", 0.0),
        ] {
            assert_eq!(value(&x, name), want, "{name}");
        }
        // Dated 1 day 12 hours after it arrived: ahead, clipped at a day.
        assert_eq!((value(&x, "date_ahead_hours"), value(&x, "date_behind_hours")), (24.0, 0.0));
    }

    #[test]
    fn a_newsletter_is_bulk_not_forged() {
        let raw = "Authentication-Results: sioul-0123456789ab.invalid; spf=pass smtp.mailfrom=news.shop.example; dkim=pass header.d=shop.example; dmarc=pass header.from=shop.example\r\n\
                   X-Spam-Status: No, score=-1.5 required=5.0\r\n\
                   Received: from mail.shop.example (mail.shop.example [203.0.112.9]) by mx.provider.example with ESMTPS\r\n\
                   From: Shop <news@shop.example>\r\n\
                   Message-ID: <n-42@mail.shop.example>\r\n\
                   Date: Wed, 07 Oct 2026 07:00:00 +0000\r\n\
                   List-Unsubscribe: <https://shop.example/u?x=1>\r\n\
                   List-Id: <news.shop.example>\r\n\
                   Precedence: bulk\r\n\
                   Subject: This week\r\n\r\n\
                   New in store. See https://shop.example/new and https://www.shop.example/sale\r\n";
        let x = of(raw);
        for (name, want) in [
            ("spf_pass", 1.0), ("dkim_pass", 1.0), ("dmarc_pass", 1.0), ("aligned", 1.0), ("message_id_elsewhere", 0.0), ("reply_to_elsewhere", 0.0),
            ("list_unsubscribe", 1.0), ("list_id", 1.0), ("precedence_bulk", 1.0), ("links", 2.0), ("links_elsewhere", 0.0), ("html_only", 0.0),
            ("date_ahead_hours", 0.0), ("received_hops", 1.0), ("helo_bare_ip", 0.0),
        ] {
            assert_eq!(value(&x, name), want, "{name}");
        }
        // Received at 08:00, dated 07:00: an hour behind.
        assert_eq!(value(&x, "date_behind_hours"), 1.0);
        // The provider's verdict, written above the line where the message came in: seen, not spam, its score.
        assert_eq!((value(&x, "provider_seen"), value(&x, "provider_flagged"), value(&x, "provider_score")), (1.0, 0.0, -1.5));
    }

    /// Sioul's stamp on top, or not: the features are the same, read from the
    /// provider's header that the training corpus has too.
    #[test]
    fn sioul_stamp_or_not_the_same_features() {
        let provider = "Authentication-Results: mx.provider.example; spf=pass smtp.mailfrom=shop.example; dkim=pass header.d=shop.example; dmarc=pass header.from=shop.example\r\n\
                        From: Shop <news@shop.example>\r\nMessage-ID: <n-1@shop.example>\r\nDate: Wed, 07 Oct 2026 07:00:00 +0000\r\nSubject: This week\r\n\r\nNew in store.\r\n";
        let stamped = format!("Authentication-Results: sioul-0123456789ab.invalid; spf=fail smtp.mailfrom=shop.example; dkim=fail header.d=shop.example; dmarc=none\r\n{provider}");
        let ids = ["sioul-0123456789ab.invalid".to_string(), "mx.provider.example".to_string()];
        let read = |raw: &str| {
            let card = Card::from_bytes(raw.as_bytes()).unwrap();
            let links = super::super::tokenize::tokens(&card.subject, &card.excerpt).links;
            (features(&card, provider_results(&card.headers, &ids).as_ref(), Some(1_791_360_000), &links), trust::read_auth_results(&card.headers, &ids))
        };
        let ((plain, _), (with_stamp, sioul_first)) = (read(provider), read(&stamped));
        assert_eq!(plain, with_stamp);
        assert_eq!((value(&plain, "spf_pass"), value(&plain, "dkim_pass"), value(&plain, "aligned")), (1.0, 1.0, 1.0));
        // What the lanes read differs: Sioul's own checks come first there.
        assert_eq!(sioul_first.map(|r| r.authserv_id).as_deref(), Some("sioul-0123456789ab.invalid"));
    }

    #[test]
    fn nothing_known_says_nothing() {
        // No results, no Date, no Message-ID: the "none" of each check, and what is missing.
        let x = of("From: someone@example.org\r\nSubject: hi\r\n\r\nhello\r\n");
        assert_eq!((value(&x, "spf_none"), value(&x, "dkim_none"), value(&x, "dmarc_none")), (1.0, 1.0, 1.0));
        assert_eq!((value(&x, "no_date"), value(&x, "no_message_id"), value(&x, "provider_seen")), (1.0, 1.0, 0.0));
        assert_eq!((value(&x, "date_ahead_hours"), value(&x, "date_behind_hours"), value(&x, "links_elsewhere")), (0.0, 0.0, 0.0));
        assert_eq!(names_domain("Jane Doe", Some("example.org")), false);
        assert_eq!(names_domain("example.org team", Some("example.org")), false);
        assert!(names_domain("Amazon.fr Service", Some("example.org")));
        assert!(bare_ip("[IPv6:2001:db8::1]") && bare_ip("192.0.2.1") && !bare_ip("mail.example.org"));
        assert_eq!(address_domain("\"Desk\" <Desk@Help.Example.org>").as_deref(), Some("help.example.org"));
    }
}

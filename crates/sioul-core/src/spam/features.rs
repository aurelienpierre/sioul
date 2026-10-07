// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The header features, version 3: what a message's headers and shape say,
//! as named numbers, read by the classifier as words of their own beside the
//! message's words (`header_words`), so that each can be explained by its
//! name (docs/spam-filter.md, "The header features").
//!
//! They are read from the raw header block (`RawHeaders`), from your
//! provider's authentication results (`provider_results`), from the
//! message's parts (`card::Shape`) and from
//! the links its text holds (`tokenize::Tokens::links`); never from the order
//! of the headers. Left out, because they would teach the filter where mail
//! was collected rather than what it is: To and Cc, the account, dates as
//! such, the provider's own marks.
//!
//! A value can be missing (`f32::NAN`): your provider's checks when it wrote
//! none, or none under a name you trust; the route where it cannot be read.
//! A missing value stands at the training's mean, which is over the messages
//! that have it: it weighs nothing (`table::Table::score`). So mail from a
//! provider that writes no results, or reached through one whose lines are
//! written another way, is not told apart by that alone.
//!
//! Version 2 (from 7 October 2026), measured on a real mailbox: version 1
//! taught the filter which account a message came to. Only one provider
//! wrote results that were trusted, so "no results" meant "that other
//! account", whose share of spam the filter then learned; the count of
//! `Received` lines and ARC were each provider's own too. Version 2 makes
//! missing checks missing (not "none"), drops the count of hops, ARC and the
//! provider's verdict (the Porch applies that verdict itself, as its own
//! rule), and adds what the message carries wherever it is received: a DKIM
//! signature, one by the sender's own domain, a bounce address at it.
//!
//! Version 3 (the same day): the same facts, read by fastText's classifier
//! as words (`header_words`: a fact that holds, its name; a number, its bin)
//! rather than weighed by an SVM beside the mean of the words' vectors.
//!
//! The same function runs on the desktop when training and on every device
//! when sorting: a change of a feature, of its order or of its scale raises
//! `FEATURES`, and a table made with others is refused.

use super::tokenize::registrable;
use crate::card::Card;
use crate::headers::RawHeaders;
use crate::trust::{self, AuthResults, Outcome, Trust};

/// The header features' version, stamped in every table.
pub const FEATURES: u32 = 3;

/// How many header features there are.
pub const N: usize = 35;

/// Each feature's name, in the order of `features`' values.
pub const NAMES: [&str; N] = [
    // Authentication as your provider checked it (`provider_results`): pass,
    // fail, or neither for each check; missing when it wrote no results
    // under a name you trust. Then the sender's domain vouching for the
    // message (DMARC, or a signature of its own: `trust::judge_sender`), missing alike.
    "spf_pass", "spf_fail", "dkim_pass", "dkim_fail", "dmarc_pass", "dmarc_fail", "aligned",
    // What the message carries itself, the same wherever it is received:
    // a DKIM signature; one by the sender's own domain (d= the From's
    // registrable domain); its bounce address (Return-Path) at that domain.
    "dkim_signed", "dkim_signed_by_sender", "return_path_at_sender",
    // Domains: the Reply-To elsewhere, or at a mail provider anyone uses; the
    // Message-ID made elsewhere; a name shown that names another domain.
    "reply_to_elsewhere", "reply_to_freemail", "message_id_elsewhere", "name_names_domain",
    // Time: the Date ahead of when your provider received it (hours, at most
    // 24), or behind it (hours, at most 72); no Date; no Message-ID.
    "date_ahead_hours", "date_behind_hours", "no_date", "no_message_id",
    // The route: the sending server naming itself by a bare address where it
    // entered your provider; missing where that line cannot be read.
    "helo_bare_ip",
    // The parts: HTML and no text version; images and almost no text; what is attached.
    "html_only", "image_only", "attached_archive", "attached_program", "attached_office", "attached_pdf", "attached_image",
    // How much text it shows (links and spaces aside): under 150 characters, 800, 3 000, more.
    "text_short", "text_medium", "text_long", "text_longer",
    // Its links: how many (at most 30); the share of them going elsewhere than the sender's domain.
    "links", "links_elsewhere",
    // Bulk mail's own headers: gray mail, not spam, as the model weighs them.
    "list_unsubscribe", "list_id", "precedence_bulk",
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

/// The header facts as words, as the classifier reads them after the
/// message's own (`table::Kind::Rows`), each with its feature's place in
/// `NAMES`: a fact that holds, `H:` and its name in capitals; a number in a
/// few bins (the Date an hour or more ahead; an hour or a day or more
/// behind; no link, 1 to 3, 4 to 10, 11 or more; links elsewhere, half of
/// them or more); a fact that does not hold, or is missing, none. No word of
/// a message can be one of them: the tokenizer lowercases every word and
/// splits on punctuation, and its own placeholders read `_URL_`; so no
/// sender writes a header fact into a text.
pub fn header_words(x: &[f32]) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for (h, (name, &value)) in NAMES.iter().zip(x).enumerate() {
        if value.is_nan() {
            continue;
        }
        let word = match *name {
            "date_ahead_hours" => (value >= 1.0).then_some("DATE_AHEAD"),
            "date_behind_hours" if value >= 24.0 => Some("DATE_BEHIND_DAY"),
            "date_behind_hours" => (value >= 1.0).then_some("DATE_BEHIND"),
            "links" => Some(match value as u32 {
                0 => "LINKS_0",
                1..=3 => "LINKS_1_3",
                4..=10 => "LINKS_4_10",
                _ => "LINKS_11",
            }),
            _ => (value >= 0.5).then_some(""),
        };
        match word {
            Some("") => out.push((format!("H:{}", name.to_ascii_uppercase()), h)),
            Some(word) => out.push((format!("H:{word}"), h)),
            None => {}
        }
    }
    out
}

/// Every word `header_words` can give, each with its feature's place: what
/// the classifier may have learned of the header facts.
pub fn all_header_words() -> Vec<(String, usize)> {
    NAMES
        .iter()
        .enumerate()
        .flat_map(|(h, name)| {
            let words: Vec<String> = match *name {
                "date_ahead_hours" => vec!["DATE_AHEAD".into()],
                "date_behind_hours" => vec!["DATE_BEHIND".into(), "DATE_BEHIND_DAY".into()],
                "links" => ["LINKS_0", "LINKS_1_3", "LINKS_4_10", "LINKS_11"].map(String::from).to_vec(),
                name => vec![name.to_ascii_uppercase()],
            };
            words.into_iter().map(move |w| (format!("H:{w}"), h))
        })
        .collect()
}

/// The header features of a message: its card (headers, parts, text),
/// its provider's authentication results (`provider_results`),
/// when its provider received it (IMAP's INTERNALDATE, the Maildir file's
/// time), the domains of its links (`tokenize::tokens`). A value it cannot
/// know is `f32::NAN` (see the module).
pub fn features(card: &Card, auth: Option<&AuthResults>, internal_date: Option<i64>, links: &[String]) -> [f32; N] {
    let headers = &card.headers;
    let from = card.sender_domain().map(registrable).filter(|d| !d.is_empty());
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    // Pass, fail, neither; both missing without results.
    let checked = |outcome: Option<Outcome>| match (auth, outcome) {
        (None, _) => [f32::NAN, f32::NAN],
        (Some(_), Some(Outcome::Pass)) => [1.0, 0.0],
        (Some(_), Some(Outcome::Fail | Outcome::SoftFail)) => [0.0, 1.0],
        (Some(_), _) => [0.0, 0.0],
    };
    let [spf_pass, spf_fail] = checked(auth.and_then(|a| a.spf));
    let [dkim_pass, dkim_fail] = checked(auth.and_then(|a| a.dkim));
    let [dmarc_pass, dmarc_fail] = checked(auth.and_then(|a| a.dmarc));
    let aligned = if auth.is_some() { flag(trust::judge_sender(auth, false, card.sender_domain()).0 == Trust::Verified) } else { f32::NAN };
    // What it carries itself: who signed it, where its bounces go.
    let at_sender = |domain: &str| from.as_deref().is_some_and(|f| f == registrable(domain));
    let signers: Vec<String> = headers.all("DKIM-Signature").filter_map(dkim_domain).collect();
    let bounces = headers.first("Return-Path").and_then(address_domain);

    let elsewhere = |domain: &str| from.as_deref() != Some(registrable(domain).as_str());
    let reply = headers.first("Reply-To").and_then(address_domain);
    let message_id = card.message_id.as_deref().map(|id| id.trim().trim_matches(['<', '>']).rsplit_once('@').map(|(_, d)| d.to_ascii_lowercase()));
    let name_names = card.from_name.as_deref().is_some_and(|name| names_domain(name, from.as_deref()));

    let (ahead, behind) = match (card.date, internal_date) {
        (Some(date), Some(received)) => (((date - received) as f32 / 3600.0).clamp(0.0, 24.0), ((received - date) as f32 / 3600.0).clamp(0.0, 72.0)),
        _ => (0.0, 0.0),
    };
    let helo_ip = trust::boundary(headers).map_or(f32::NAN, |b| flag(bare_ip(&b.helo)));

    let shape = &card.shape;
    let kinds: Vec<Kind> = shape.parts.iter().map(|(name, mime)| Kind::of(name, mime)).collect();
    let shown = crate::text::visible_len(&card.excerpt);
    let has = |kind: Kind| flag(kinds.contains(&kind));

    let links_elsewhere = if links.is_empty() { 0.0 } else { links.iter().filter(|d| elsewhere(d)).count() as f32 / links.len() as f32 };
    let precedence = headers.first("Precedence").is_some_and(|p| matches!(p.trim().to_ascii_lowercase().as_str(), "bulk" | "list" | "junk"));

    [
        spf_pass,
        spf_fail,
        dkim_pass,
        dkim_fail,
        dmarc_pass,
        dmarc_fail,
        aligned,
        flag(!signers.is_empty()),
        flag(signers.iter().any(|d| at_sender(d))),
        flag(bounces.as_deref().is_some_and(at_sender)),
        flag(reply.as_deref().is_some_and(elsewhere)),
        flag(reply.as_deref().is_some_and(|r| crate::lookalike::SHARED.contains(&r))),
        // A Message-ID without a domain was made by no server.
        flag(message_id.as_ref().is_some_and(|d| d.as_deref().is_none_or(elsewhere))),
        flag(name_names),
        ahead,
        behind,
        flag(card.date.is_none()),
        flag(card.message_id.is_none()),
        helo_ip,
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
    ]
}

/// The signing domain of a DKIM-Signature (its `d=` tag), lowercase.
fn dkim_domain(value: &str) -> Option<String> {
    value
        .split(';')
        .find_map(|tag| {
            let (key, domain) = tag.split_once('=')?;
            key.trim().eq_ignore_ascii_case("d").then(|| domain.trim().trim_end_matches('.').to_ascii_lowercase())
        })
        .filter(|d| !d.is_empty())
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
            ("spf_fail", 1.0), ("dkim_pass", 0.0), ("dkim_fail", 0.0), ("dmarc_fail", 1.0), ("aligned", 0.0), ("dkim_signed", 0.0), ("return_path_at_sender", 0.0),
            ("reply_to_elsewhere", 1.0), ("reply_to_freemail", 1.0), ("message_id_elsewhere", 1.0), ("name_names_domain", 1.0), ("no_date", 0.0), ("no_message_id", 0.0),
            ("helo_bare_ip", 1.0), ("html_only", 1.0), ("attached_program", 1.0), ("attached_pdf", 0.0), ("links", 2.0), ("links_elsewhere", 0.5), ("text_short", 1.0),
            ("list_unsubscribe", 0.0),
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
                   Return-Path: <bounces@news.shop.example>\r\n\
                   Received: from mail.shop.example (mail.shop.example [203.0.112.9]) by mx.provider.example with ESMTPS\r\n\
                   DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed; d=esp.example; s=s1; bh=x; b=y\r\n\
                   DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed; d=Shop.Example; s=s2; bh=x; b=y\r\n\
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
            ("spf_pass", 1.0), ("dkim_pass", 1.0), ("dmarc_pass", 1.0), ("aligned", 1.0), ("dkim_signed", 1.0), ("dkim_signed_by_sender", 1.0), ("return_path_at_sender", 1.0),
            ("message_id_elsewhere", 0.0), ("reply_to_elsewhere", 0.0), ("list_unsubscribe", 1.0), ("list_id", 1.0), ("precedence_bulk", 1.0), ("links", 2.0),
            ("links_elsewhere", 0.0), ("html_only", 0.0), ("date_ahead_hours", 0.0), ("helo_bare_ip", 0.0),
        ] {
            assert_eq!(value(&x, name), want, "{name}");
        }
        // Received at 08:00, dated 07:00: an hour behind.
        assert_eq!(value(&x, "date_behind_hours"), 1.0);
        // Its provider's verdict, a spam one or not, is no feature: the Porch applies it as its own rule.
        let flagged = of(&raw.replace("X-Spam-Status: No, score=-1.5", "X-Spam-Flag: YES\r\nX-Spam-Status: Yes, score=12.5"));
        let tagged = of(&raw.replace("Subject: This week", "Subject: ***Potentiel-SPAM*** This week"));
        assert_eq!(flagged.map(f32::to_bits), x.map(f32::to_bits));
        assert_eq!(tagged.map(f32::to_bits), x.map(f32::to_bits), "the subject's words are the tokenizer's, its tag none of the features'");
        // Signed by an email service only, its bounces there: carried, not by the sender.
        let esp = of(&raw.replace("d=Shop.Example", "d=esp.example").replace("bounces@news.shop.example", "bounce@esp.example"));
        assert_eq!((value(&esp, "dkim_signed"), value(&esp, "dkim_signed_by_sender"), value(&esp, "return_path_at_sender")), (1.0, 0.0, 0.0));
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
        // Bit for bit: a missing value (no route here) is NaN in both, which equals nothing.
        assert_eq!(plain.map(f32::to_bits), with_stamp.map(f32::to_bits));
        assert_eq!((value(&plain, "spf_pass"), value(&plain, "dkim_pass"), value(&plain, "aligned")), (1.0, 1.0, 1.0));
        // What the lanes read differs: Sioul's own checks come first there.
        assert_eq!(sioul_first.map(|r| r.authserv_id).as_deref(), Some("sioul-0123456789ab.invalid"));
    }

    /// The header facts as words: those that hold, numbers in bins, nothing for a missing one.
    #[test]
    fn header_facts_as_words() {
        let mut x = [0.0f32; N];
        x[index("dkim_signed").unwrap()] = 1.0;
        x[index("aligned").unwrap()] = f32::NAN;
        x[index("links").unwrap()] = 5.0;
        x[index("links_elsewhere").unwrap()] = 0.6;
        x[index("date_behind_hours").unwrap()] = 30.0;
        let words: Vec<String> = header_words(&x).into_iter().map(|(w, _)| w).collect();
        assert_eq!(words, ["H:DKIM_SIGNED", "H:DATE_BEHIND_DAY", "H:LINKS_4_10", "H:LINKS_ELSEWHERE"]);
        x[index("date_behind_hours").unwrap()] = 2.0;
        x[index("date_ahead_hours").unwrap()] = 0.5;
        x[index("links").unwrap()] = 0.0;
        let words = header_words(&x);
        assert!(words.iter().any(|(w, h)| w == "H:DATE_BEHIND" && NAMES[*h] == "date_behind_hours"));
        assert!(words.iter().any(|(w, h)| w == "H:LINKS_0" && NAMES[*h] == "links"));
        assert!(!words.iter().any(|(w, _)| w == "H:DATE_AHEAD"), "half an hour ahead is no fact");
        // Nothing known: no word at all, not even a bin.
        assert!(header_words(&[f32::NAN; N]).is_empty());
        // Every word it gives is one of all those it can give; each fact once.
        let all = all_header_words();
        let full = header_words(&[1.0; N]);
        assert!(full.iter().chain(&words).all(|w| all.contains(w)), "{full:?}");
        assert_eq!(full.len(), N, "every fact once, a number in one bin: {full:?}");
        // No word of a text can be one: the tokenizer's words are lowercase, split on punctuation.
        let text = super::super::tokenize::tokens("H:DKIM_SIGNED", "h:dkim_signed H:LINKS_0 __dkim_signed");
        assert!(text.words.iter().all(|w| !w.starts_with("H:")), "{:?}", text.words);
    }

    #[test]
    fn nothing_known_says_nothing() {
        // No results: each check missing, not "none" (a provider that writes none is not told apart by it);
        // no Received line: the route missing; no Date, no Message-ID: said.
        let x = of("From: someone@example.org\r\nSubject: hi\r\n\r\nhello\r\n");
        for name in ["spf_pass", "spf_fail", "dkim_pass", "dkim_fail", "dmarc_pass", "dmarc_fail", "aligned", "helo_bare_ip"] {
            assert!(value(&x, name).is_nan(), "{name} missing");
        }
        assert_eq!((value(&x, "dkim_signed"), value(&x, "dkim_signed_by_sender"), value(&x, "return_path_at_sender")), (0.0, 0.0, 0.0));
        assert_eq!((value(&x, "no_date"), value(&x, "no_message_id")), (1.0, 1.0));
        assert_eq!((value(&x, "date_ahead_hours"), value(&x, "date_behind_hours"), value(&x, "links_elsewhere")), (0.0, 0.0, 0.0));
        assert_eq!(names_domain("Jane Doe", Some("example.org")), false);
        assert_eq!(names_domain("example.org team", Some("example.org")), false);
        assert!(names_domain("Amazon.fr Service", Some("example.org")));
        assert!(bare_ip("[IPv6:2001:db8::1]") && bare_ip("192.0.2.1") && !bare_ip("mail.example.org"));
        assert_eq!(dkim_domain(" v=1; a=rsa-sha256; d=Mail.Example.org.; s=k1").as_deref(), Some("mail.example.org"));
        assert_eq!(dkim_domain("v=1; s=k1"), None);
        assert_eq!(address_domain("\"Desk\" <Desk@Help.Example.org>").as_deref(), Some("help.example.org"));
    }
}

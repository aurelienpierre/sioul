// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Invented mail for the tests, this crate's and the command line's: ham and
//! spam on reserved domains (RFC 2606), no real person, no real address, no
//! real text. Deterministic: the same seed, the same mailbox.

use crate::Rng;
use crate::corpus::{Leaf, Node, Record, Text};
use sioul_core::folders::Role;

/// Words of everyday mail (French and English).
const HAM: &[&str] = &[
    "bonjour", "reunion", "demain", "projet", "facture", "merci", "dossier", "rendez", "planning", "meeting", "tomorrow", "schedule", "thanks", "report", "draft", "family",
    "dinner", "weekend", "photos", "garden", "school", "doctor", "appointment", "contract", "invoice", "budget", "agenda", "minutes", "review", "chapter", "manuscript",
    "concert", "rehearsal", "lesson", "notes", "travail", "semaine", "voyage", "billet", "train", "hotel", "piano", "partition", "repetition", "atelier", "jardin",
];

/// Words of spam.
const SPAM: &[&str] = &[
    "winner", "lottery", "prize", "casino", "pills", "bitcoin", "investment", "urgent", "claim", "free", "offer", "crypto", "discount", "gagnant", "gratuit", "loterie",
    "cadeau", "bonus", "jackpot", "wallet", "inheritance", "beneficiary", "transfer", "million", "guaranteed", "profit", "exclusive", "limited", "congratulations",
    "selected", "reward", "voucher", "pharmacy", "replica", "dating", "singles", "loan", "approved", "credit", "refinance",
];

/// Words of any mail.
const COMMON: &[&str] = &["the", "you", "and", "for", "with", "this", "your", "le", "la", "les", "vous", "nous", "pour", "avec", "est", "une", "des", "please", "today", "now"];

const HAM_DOMAINS: &[&str] = &["example.org", "example.com", "example.net", "mairie.example", "banque.example", "orchestre.example"];
const SPAM_DOMAINS: &[&str] = &["lottery.test", "offers.invalid", "pills.test", "crypto.invalid", "prizes.test"];
const NAMES: &[&str] = &["Alice Martin", "Bruno Petit", "Chloe Durand", "David Leroy", "Emma Moreau", "Felix Garnier"];

/// One invented message, and where it sits.
#[derive(Debug, Clone)]
pub struct Mail {
    pub account: &'static str,
    pub folder: &'static str,
    pub role: Role,
    pub flags: Vec<String>,
    /// INTERNALDATE, Unix seconds.
    pub date: i64,
    pub raw: Vec<u8>,
}

/// The mailbox's first day: 1 January 2024; its two years end before today,
/// as a real mailbox's messages all came before a training.
const START: i64 = 1_704_067_200;

fn pick<'a>(rng: &mut Rng, words: &[&'a str]) -> &'a str {
    words[rng.below(words.len())]
}

/// The words of a message: mostly its own kind's, some common, a few of the other kind's.
fn words(rng: &mut Rng, spam: bool, count: usize) -> Vec<&'static str> {
    (0..count)
        .map(|_| {
            let roll = rng.unit();
            let (own, other) = if spam { (SPAM, HAM) } else { (HAM, SPAM) };
            if roll < 0.65 {
                pick(rng, own)
            } else if roll < 0.95 {
                pick(rng, COMMON)
            } else {
                pick(rng, other)
            }
        })
        .collect()
}

/// An RFC 5322 date.
fn date_header(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds).map(|t| t.strftime("%a, %d %b %Y %H:%M:%S +0000").to_string()).unwrap_or_default()
}

/// One message: plain text, sometimes with list headers or a link.
pub fn message(rng: &mut Rng, spam: bool, serial: u32, date: i64) -> Vec<u8> {
    let domain = if spam { pick(rng, SPAM_DOMAINS) } else { pick(rng, HAM_DOMAINS) };
    let name = if spam { "Prize Department" } else { pick(rng, NAMES) };
    let local = if spam { "noreply" } else { "contact" };
    let length = 3 + rng.below(4);
    let subject = words(rng, spam, length).join(" ");
    let length = 30 + rng.below(90);
    let mut body = words(rng, spam, length).join(" ");
    if rng.unit() < 0.5 {
        body.push_str(&format!(" https://www.{domain}/page{serial}"));
    }
    let mut raw = format!(
        "From: {name} <{local}@{domain}>\r\nTo: owner@example.org\r\nSubject: {subject}\r\nDate: {}\r\nMessage-ID: <{serial}.{date}@{domain}>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n",
        date_header(date)
    );
    if rng.unit() < 0.3 {
        raw.push_str(&format!("List-Unsubscribe: <https://{domain}/unsubscribe>\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(&body);
    raw.push_str("\r\n");
    raw.into_bytes()
}

/// Words of a spam campaign never seen before (the newest weeks).
const CAMPAIGN: &[&str] = &["vitamins", "supplement", "detox", "slimming", "capsules", "miracle", "keto", "collagen", "booster", "metabolism", "glucose", "formula"];

/// A message of that new campaign: its words, a few common ones, a link.
pub fn campaign(rng: &mut Rng, serial: u32, date: i64) -> Vec<u8> {
    let pick_words = |rng: &mut Rng, count: usize| (0..count).map(|_| if rng.unit() < 0.8 { pick(rng, CAMPAIGN) } else { pick(rng, COMMON) }).collect::<Vec<_>>().join(" ");
    let subject = pick_words(rng, 4);
    let length = 40 + rng.below(40);
    let body = pick_words(rng, length);
    format!(
        "From: Health Shop <shop@detox.invalid>\r\nTo: owner@example.org\r\nSubject: {subject}\r\nDate: {}\r\nMessage-ID: <{serial}.{date}@detox.invalid>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body} https://www.detox.invalid/offer{serial}\r\n",
        date_header(date)
    )
    .into_bytes()
}

/// A mailbox over two years: ham in two accounts' inboxes and archives, spam
/// in their Junk folders, a few spam marked `$Junk` in an inbox and a few ham
/// rescued from Junk with `$NotJunk`.
pub fn mailbox(seed: u64, ham: usize, spam: usize) -> Vec<Mail> {
    let mut rng = Rng::new(seed);
    let total = ham + spam;
    let mut kinds: Vec<bool> = (0..ham).map(|_| false).chain((0..spam).map(|_| true)).collect();
    // Shuffled, so both kinds come all along the two years.
    for i in (1..kinds.len()).rev() {
        let j = rng.below(i + 1);
        kinds.swap(i, j);
    }
    kinds
        .into_iter()
        .enumerate()
        .map(|(i, spam)| {
            let date = START + (i as i64) * (2 * 365 * 86_400 / total.max(1) as i64);
            let account = if rng.unit() < 0.5 { "home" } else { "work" };
            let raw = message(&mut rng, spam, i as u32 + 1, date);
            let roll = rng.unit();
            let (folder, role, flags) = match (spam, roll) {
                (true, r) if r < 0.1 => ("INBOX", Role::Inbox, vec!["$Junk".to_string()]),
                (true, _) => ("Junk", Role::Junk, vec![]),
                (false, r) if r < 0.05 => ("Junk", Role::Junk, vec!["$NotJunk".to_string(), "\\Seen".to_string()]),
                (false, r) if r < 0.4 => ("Archive", Role::Archive, vec!["\\Seen".to_string()]),
                (false, _) => ("INBOX", Role::Inbox, vec!["\\Seen".to_string()]),
            };
            Mail { account, folder, role, flags, date, raw }
        })
        .collect()
}

/// The corpus record a download makes of a plain-text message.
pub fn record_of(mail: &Mail, uidvalidity: u32, uid: u32) -> Record {
    let raw = String::from_utf8_lossy(&mail.raw).into_owned();
    let (header, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    Record {
        account: mail.account.to_string(),
        folder: mail.folder.to_string(),
        role: mail.role,
        uidvalidity,
        uid,
        date: mail.date,
        flags: mail.flags.clone(),
        size: mail.raw.len() as u32,
        header: format!("{header}\r\n\r\n"),
        header_latin1: false,
        structure: Some(Node::Leaf(Leaf { mime: "text/plain".into(), charset: Some("utf-8".into()), encoding: "8bit".into(), octets: body.len() as u32, ..Leaf::default() })),
        plain: Some(Text { at: vec![1], text: body.chars().take(6000).collect() }),
        html: None,
        fetched: mail.date + 60,
    }
}

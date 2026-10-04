// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A morning's mail, sorted: every lane, the summary, the one exception.

use sioul_core::cases::CaseStore;
use sioul_core::codes::CodeKind;
use sioul_core::i18n::Translator;
use sioul_core::maildir;
use sioul_core::porch::{self, Context, KnownSenders, Lane, Reason};
use sioul_core::trust::Trust;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn the_porch_sorts_a_morning() {
    let root = fixtures();
    let store = CaseStore::load(&root).unwrap();
    assert!(store.missing_files().is_empty());
    let known = KnownSenders::parse(&std::fs::read_to_string(root.join("known-senders.txt")).unwrap());
    let ids = vec!["mx.example.net".to_string()];
    let senders = porch::Senders::default();
    let ctx = Context { cases: Some(&store), known: &known, senders: &senders, trusted_ids: &ids, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &[] };
    let triaged: Vec<_> = maildir::read_messages(&root.join("porch")).into_iter().map(|c| porch::triage(c, &ctx)).collect();
    let find = |part: &str| triaged.iter().find(|t| t.card.subject.contains(part)).unwrap();

    let code = find("sécurité");
    assert_eq!(code.lane, Lane::RightNow);
    assert_eq!(code.code.as_ref().and_then(|c| c.code.as_deref()), Some("482913"));
    // A code you may have just asked for, its sender not verified: at once, with a warning.
    let unverified = find("vérification");
    assert_eq!(unverified.lane, Lane::RightNow);
    assert!(unverified.reasons.contains(&Reason::UnverifiedCode(CodeKind::Code)));
    assert_eq!(find("Remboursement").trust, Trust::Forged);
    assert_eq!(find("Remboursement").lane, Lane::SetAside);
    assert_eq!(find("You won").lane, Lane::SetAside);
    assert_eq!(find("avis").lane, Lane::Case("taxes".into()));
    assert_eq!(find("This week").lane, Lane::Filed);
    assert_eq!(find("Hello").lane, Lane::Screener);
    assert_eq!(find("Saturday").lane, Lane::People);

    let summary = porch::summarise(&triaged);
    assert_eq!(
        Translator::new("en").summary(&summary, Some(&store)),
        "Eight letters came. Two codes are ready above. One belongs to a project: Taxes. \
         One is from people you know. One is from someone new and waits in the screener. \
         One newsletter or notification was filed. Two forged or spam messages were set aside."
    );
    assert_eq!(
        Translator::new("fr").summary(&summary, Some(&store)),
        "Huit lettres sont arrivées. Deux codes sont prêts ci-dessus. Une concerne un projet\u{202f}: Taxes. \
         Une vient de quelqu’un que vous connaissez. Une vient d’un nouvel expéditeur et attend votre accord. \
         Une lettre d’information ou notification a été rangée. Deux messages falsifiés ou indésirables ont été mis de côté."
    );
}

#[test]
fn what_you_send_yourself_comes_in() {
    let known = KnownSenders::default();
    let senders = porch::Senders::default();
    let ids = vec!["mx.example.net".to_string()];
    let own = vec!["me@example.org".to_string(), "me@work.example".to_string()];
    let ctx = Context { cases: None, known: &known, senders: &senders, trusted_ids: &ids, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &own };
    let mail = |auth: &str| {
        let raw = format!("Authentication-Results: mx.example.net; dkim={auth} header.d=example.org; spf={auth} smtp.mailfrom=example.org; dmarc={auth} header.from=example.org\r\nFrom: Me <me@example.org>\r\nTo: me@work.example\r\nSubject: The scan\r\nMessage-ID: <scan@example.org>\r\nX-Spam-Flag: YES\r\n\r\nThe file.\r\n");
        sioul_core::card::Card::from_bytes(raw.as_bytes()).unwrap()
    };
    // Verified: in, past the screener and the spam flag.
    let mine = porch::triage(mail("pass"), &ctx);
    assert_eq!(mine.lane, Lane::People);
    assert!(mine.reasons.contains(&Reason::FromYourself));
    // Your own address, forged: set aside as any forgery.
    let forged = porch::triage(mail("fail"), &ctx);
    assert_eq!(forged.lane, Lane::SetAside);
}

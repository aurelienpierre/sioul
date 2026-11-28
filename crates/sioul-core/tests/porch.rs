// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A morning's mail, sorted: every lane, the summary, the one exception.

use sioul_core::projects::ProjectStore;
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
    // A triage reads your words on spam where Sioul keeps them: never the person's own here.
    sioul_core::config::stand_ins_only();
    let root = fixtures();
    let store = ProjectStore::load(&root).unwrap();
    assert!(store.missing_files().is_empty());
    let known = KnownSenders::parse(&std::fs::read_to_string(root.join("known-senders.txt")).unwrap());
    let ids = vec!["mx.example.net".to_string()];
    let senders = porch::Senders::default();
    let ctx = Context { projects: Some(&store), known: &known, senders: &senders, trusted_ids: &ids, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, words: None, own_addresses: &[], spam: None };
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
    assert_eq!(find("avis").lane, Lane::Project("taxes".into()));
    assert_eq!(find("This week").lane, Lane::Filed);
    assert_eq!(find("Hello").lane, Lane::Screener);
    assert_eq!(find("Saturday").lane, Lane::People);

    let summary = porch::summarise(&triaged);
    assert_eq!(
        Translator::new("en").summary(&summary, Some(&store)),
        "Eight e-mails came. Two codes are ready above. One belongs to a project: Taxes. \
         One is from people you know. One is from someone new and waits in the screener. \
         One newsletter or notification was filed. Two forged or spam messages were set aside."
    );
    assert_eq!(
        Translator::new("fr").summary(&summary, Some(&store)),
        "Huit courriels sont arrivés. Deux codes sont prêts ci-dessus. Un concerne un projet\u{202f}: Taxes. \
         Un vient de quelqu’un que vous connaissez. Un vient d’un nouvel expéditeur et attend votre accord. \
         Une infolettre ou notification a été rangée. Deux courriels falsifiés ou indésirables ont été mis de côté."
    );
}

#[test]
fn what_you_send_yourself_comes_in() {
    // A triage reads your words on spam where Sioul keeps them: never the person's own here.
    sioul_core::config::stand_ins_only();
    let known = KnownSenders::default();
    let senders = porch::Senders::default();
    let ids = vec!["mx.example.net".to_string()];
    let own = vec!["me@example.org".to_string(), "me@work.example".to_string()];
    let ctx = Context { projects: None, known: &known, senders: &senders, trusted_ids: &ids, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, words: None, own_addresses: &own, spam: None };
    let mail = |auth: &str| {
        // The provider's spam flag, above the line where the message came in: its own.
        let raw = format!("Authentication-Results: mx.example.net; dkim={auth} header.d=example.org; spf={auth} smtp.mailfrom=example.org; dmarc={auth} header.from=example.org\r\nX-Spam-Flag: YES\r\nReceived: from mail.example.org (mail.example.org [203.0.112.20]) by mx.example.net with ESMTPS\r\nFrom: Me <me@example.org>\r\nTo: me@work.example\r\nSubject: The scan\r\nMessage-ID: <scan@example.org>\r\n\r\nThe file.\r\n");
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

/// Codes, sign-in links and resets that sites send through their newsletter
/// services, with the same bulk headers (`List-Unsubscribe`, `List-Id`,
/// `Precedence: bulk`, a service's own markers): at once, as any code. A
/// newsletter that talks about passwords stays filed; a forged code stays set
/// aside, list headers or not; a code whose sender is not verified keeps its warning.
#[test]
fn codes_sent_with_bulk_headers_come_at_once() {
    // A triage reads your words on spam where Sioul keeps them: never the person's own here.
    sioul_core::config::stand_ins_only();
    let known = KnownSenders::default();
    let senders = porch::Senders::default();
    let ids = vec!["mx.example.net".to_string()];
    let ctx = Context { projects: None, known: &known, senders: &senders, trusted_ids: &ids, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, words: None, own_addresses: &[], spam: None };
    let triaged: Vec<_> = maildir::read_messages(&fixtures().join("porch-bulk")).into_iter().map(|c| porch::triage(c, &ctx)).collect();
    assert_eq!(triaged.len(), 7);
    assert!(triaged.iter().all(|t| t.card.is_list), "every one carries bulk headers");
    let find = |part: &str| triaged.iter().find(|t| t.card.subject.contains(part)).unwrap_or_else(|| panic!("{part}"));
    let kind = |part: &str| find(part).code.as_ref().map(|c| (c.kind, c.code.clone()));

    // An account's security code, a shop's sign-in code, a sign-in link, a reset in French.
    for (part, expected) in [
        ("security code", (CodeKind::Code, Some("482913".to_string()))),
        ("Sign-in attempt", (CodeKind::Code, Some("551204".to_string()))),
        ("Sign in to Readwell", (CodeKind::SignInLink, None)),
        ("Réinitialisation", (CodeKind::PasswordReset, None)),
    ] {
        let t = find(part);
        assert_eq!((t.lane.clone(), kind(part)), (Lane::RightNow, Some(expected)), "{part}: {:?}", t.reasons);
        assert_eq!(t.trust, Trust::Verified, "{part}");
        assert!(!t.reasons.iter().any(|r| matches!(r, Reason::UnverifiedCode(_))), "{part}");
    }
    // A newsletter's articles about passwords, resets and sign-in links: filed, no code.
    let news = find("Weekly Byte");
    assert_eq!((news.lane.clone(), news.code.clone(), news.reasons.last()), (Lane::Filed, None, Some(&Reason::Newsletter)));
    // A code failing DMARC under a policy that rejects it: forged, whatever its list headers say.
    let forged = find("Your verification code");
    assert_eq!((forged.lane.clone(), forged.trust), (Lane::SetAside, Trust::Forged));
    assert_eq!(forged.reasons.last(), Some(&Reason::Forged));
    // A code from a sender nothing verifies: at once, with its warning.
    let unverified = find("one-time code");
    assert_eq!((unverified.lane.clone(), kind("one-time code")), (Lane::RightNow, Some((CodeKind::Code, Some("309118".to_string())))));
    assert!(unverified.reasons.contains(&Reason::UnverifiedCode(CodeKind::Code)));
}

/// "Not spam" on mail your provider flagged: its `$NotJunk` keyword, in its
/// file name as each sync keeps it, takes it out of Set aside for good, into
/// the lane it would have had; `$Junk` beside it says nothing either way.
#[test]
fn mail_said_not_spam_leaves_set_aside() {
    // A triage reads your words on spam where Sioul keeps them: never the person's own here.
    sioul_core::config::stand_ins_only();
    let known = KnownSenders::default();
    let senders = porch::Senders::default();
    let ids = vec!["mx.example.net".to_string()];
    let ctx = Context { projects: None, known: &known, senders: &senders, trusted_ids: &ids, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, words: None, own_addresses: &[], spam: None };
    let raw = "Authentication-Results: mx.example.net; dkim=pass header.d=lottery.test; spf=pass smtp.mailfrom=lottery.test; dmarc=pass header.from=lottery.test\r\n\
               X-Spam-Flag: YES\r\n\
               Received: from mail.lottery.test (mail.lottery.test [203.0.112.40]) by mx.example.net with ESMTPS\r\n\
               From: Prize <win@lottery.test>\r\nTo: you@example.org\r\nSubject: You won\r\nMessage-ID: <win-1@lottery.test>\r\n\r\nClaim it.\r\n";
    let stored = |flags: String| {
        let mut card = sioul_core::card::Card::from_bytes(raw.as_bytes()).unwrap();
        card.path = Some(PathBuf::from(format!("/mail/home/cur/1759400000.U7-1.sioul{}2,{flags}", maildir::INFO)));
        porch::triage(card, &ctx)
    };
    let flagged = stored("S".into());
    assert_eq!(flagged.lane, Lane::SetAside);
    assert!(flagged.reasons.iter().any(Reason::is_spam));
    let said = stored(format!("S{}", maildir::NOT_JUNK));
    assert_eq!(said.lane, Lane::Screener, "a stranger's mail, as it would have been");
    assert!(!said.reasons.iter().any(Reason::is_spam));
    assert_eq!(stored(format!("S{}{}", maildir::JUNK, maildir::NOT_JUNK)).lane, Lane::SetAside);
}

/// A code always goes through (24 November 2026): a message carrying one
/// that the Porch set aside as forged (DMARC failing) is said by a notice,
/// from its address, never with its code nor its words; a forged message
/// without a code says nothing; a code that passed its checks is told whole.
#[test]
fn a_forged_code_is_said_never_shown() {
    sioul_core::config::stand_ins_only();
    let known = KnownSenders::default();
    let ids = vec!["mx.example.net".to_string()];
    let senders = porch::Senders::default();
    let ctx = Context { projects: None, known: &known, senders: &senders, trusted_ids: &ids, now: None, priority: Default::default(), own_domains: &[], shielded: false, assessments: None, words: None, own_addresses: &[], spam: None };
    let message = |dmarc: &str, body: &str| {
        let raw = format!(
            "From: Service <codes@service.example>\r\nTo: you@example.org\r\nSubject: Votre code\r\nMessage-ID: <code-{dmarc}@service.example>\r\n\
             Authentication-Results: mx.example.net; spf={dmarc} smtp.mailfrom=service.example; dkim={dmarc} header.d=service.example; dmarc={dmarc} (p=reject) header.from=service.example\r\n\
             Content-Type: text/plain; charset=UTF-8\r\n\r\n{body}\r\n"
        );
        porch::triage(sioul_core::card::Card::from_bytes(raw.as_bytes()).unwrap(), &ctx)
    };
    let tr = Translator::new("en");
    let forged = message("fail", "Votre code de vérification : 551204. Il expire dans 10 minutes.");
    assert_eq!((forged.lane.clone(), forged.trust), (Lane::SetAside, Trust::Forged));
    let said = sioul_core::view::set_aside_codes(std::slice::from_ref(&forged), &tr);
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].title, "Code from codes@service.example (forged)");
    assert_eq!(said[0].code, None);
    assert!(!said[0].body().contains("551204") && !said[0].notice.what.contains("551204"), "{:?}", said[0].body());
    let notice = said[0].notice("Copy", "Open the message");
    assert!(notice.actions.iter().all(|a| !matches!(a.act, sioul_core::notice::Act::Copy(_))), "nothing to copy");
    assert!(sioul_core::view::codes(std::slice::from_ref(&forged), &tr).is_empty(), "never among the codes shown");
    // Forged, no code: nothing said.
    assert!(sioul_core::view::set_aside_codes(&[message("fail", "Bonjour, voici la facture.")], &tr).is_empty());
    // Passing: told whole, among the codes.
    let proven = message("pass", "Votre code de vérification : 551204. Il expire dans 10 minutes.");
    assert_eq!(proven.lane, Lane::RightNow);
    assert!(sioul_core::view::set_aside_codes(std::slice::from_ref(&proven), &tr).is_empty());
    assert_eq!(sioul_core::view::codes(std::slice::from_ref(&proven), &tr)[0].code.as_deref(), Some("551204"));
    // A name that borrows a brand its address does not belong to (should-fix 5, the owner's yes of 26 November
    // 2026): said as the forged one is, from its address, not verified, never its code.
    let raw = "From: PayPal <billing@lumen.example.net>\r\nTo: you@example.org\r\nSubject: Votre code\r\nMessage-ID: <code-name@lumen.example.net>\r\n\
               Authentication-Results: mx.example.net; spf=pass smtp.mailfrom=lumen.example.net; dkim=pass header.d=lumen.example.net; dmarc=pass (p=reject) header.from=lumen.example.net\r\n\
               Content-Type: text/plain; charset=UTF-8\r\n\r\nVotre code de vérification : 551204. Il expire dans 10 minutes.\r\n";
    let borrowed = porch::triage(sioul_core::card::Card::from_bytes(raw.as_bytes()).unwrap(), &ctx);
    assert_eq!(borrowed.lane, Lane::SetAside);
    assert!(borrowed.reasons.iter().any(|r| matches!(r, porch::Reason::Impersonation { .. })), "{:?}", borrowed.reasons);
    let said = sioul_core::view::set_aside_codes(std::slice::from_ref(&borrowed), &tr);
    assert_eq!(said.len(), 1);
    assert_eq!((said[0].title.as_str(), said[0].code.as_deref()), ("Code from billing@lumen.example.net (not verified)", None));
    assert_eq!(said[0].warning.as_deref(), Some(tr.text("right-now-set-aside-name", None).as_str()));
    assert!(!said[0].body().contains("551204"), "{:?}", said[0].body());
}

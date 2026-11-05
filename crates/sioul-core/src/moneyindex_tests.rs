// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The money index's and the history's tests (`moneyindex`), on invented
//! mail in a folder of its own; PayPal's domain alone is a real one, its own
//! rule being what is tested.

use super::*;
use crate::movements::{Input, derive_with_gone};

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

/// "Votre facture n° 2026-042 de 52,00 €" and the next month's are one shape; another subject is another.
#[test]
fn a_subjects_shape_is_without_its_numbers() {
    assert_eq!(shape("Votre facture n° 2026-042 de 52,00 €"), shape("Votre facture n° 2026-043 de 61,30 €"));
    assert_eq!(shape("Votre facture n° 2026-042 de 52,00 €"), "votre facture n° # de #");
    assert_ne!(shape("Vous avez reçu un paiement"), shape("Black Friday : -50 % sur tout"));
    let mut ignored = BTreeSet::new();
    not_a_payment(&mut ignored, "mid:a@example.org", Some("paypal.fr"), "PayPal", "Vous avez reçu 12,00 €");
    let line = |key: &str, subject: &str| MailLine {
        key: key.into(),
        path: None,
        date: day("2026-10-01"),
        payment: Payment { kind: PaymentKind::Received, amount: None, party: String::new(), fee: None, net: None },
        subject: subject.into(),
        sender: "PayPal".into(),
        sender_domain: Some("paypal.fr".into()),
        budget: None,
        preset: None,
        label: String::new(),
        state: MailState::Proposed,
        doubtful: false,
        refs: Vec::new(),
        due: None,
        instalments: None,
    };
    assert_eq!(super::ignored(&ignored, &line("mid:a@example.org", "Vous avez reçu 12,00 €")), Some(Ignored::Itself));
    assert_eq!(super::ignored(&ignored, &line("mid:b@example.org", "Vous avez reçu 40,00 €")), Some(Ignored::Shape), "mail like this from that sender");
    assert_eq!(super::ignored(&ignored, &line("mid:c@example.org", "Votre relevé mensuel")), None, "another shape counts");
    assert!(count_again(&mut ignored, Some("paypal.fr"), "PayPal", "Vous avez reçu 99,00 €"));
    assert_eq!(super::ignored(&ignored, &line("mid:b@example.org", "Vous avez reçu 40,00 €")), None);
}

/// A message written into a folder, as Sioul stores it.
fn message(dir: &Path, n: u32, raw: &str) {
    for sub in ["tmp", "new", "cur"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    std::fs::write(dir.join("cur").join(format!("177700{n:04}.U7-{n}.stand:2,S")), raw).unwrap();
}

fn mail(id: &str, from: &str, subject: &str, date: &str, body: &str) -> String {
    format!("Message-ID: <{id}>\r\nFrom: {from}\r\nTo: Jane <jane@example.org>\r\nSubject: {subject}\r\nDate: {date}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n")
}

/// The history (design §5): every money mail of the address, Sent mail
/// included, once; never a picnic, a sale's newsletter, or what lies in the
/// junk; newest first; what became of each in words (counted in a budget,
/// not counted as you said, of this shape, not tied); its filters; each
/// message opened once, ever.
#[test]
fn the_history_holds_every_money_mail_once() {
    let account = format!("stand-money-{}", std::process::id());
    let root = std::env::temp_dir().join(format!("sioul-moneyindex-{account}"));
    let _ = std::fs::remove_dir_all(&root);
    message(&root, 1, &mail("p1@paypal.example", "PayPal <service@paypal.fr>", "Vous avez reçu un paiement", "Thu, 01 Oct 2026 10:00:00 +0200", "Vous avez reçu un paiement de 52,00 € EUR de Client A."));
    message(&root, 2, &mail("p2@paypal.example", "PayPal <service@paypal.fr>", "Vous avez reçu un paiement", "Mon, 05 Oct 2026 10:00:00 +0200", "Vous avez reçu un paiement de 40,00 € EUR de Client B."));
    message(&root, 3, &mail("r1@shop.example", "Boutique <contact@shop.example>", "Confirmation de paiement", "Tue, 06 Oct 2026 10:00:00 +0200", "Vous avez payé 387,00 € à Exemple Retail."));
    message(&root, 4, &mail("x1@friends.example", "Lou <lou@friends.example>", "Samedi, le pique-nique", "Wed, 07 Oct 2026 10:00:00 +0200", "On apporte 2 salades."));
    message(&root, 5, &mail("n1@shop.example", "Boutique <news@shop.example>", "Black Friday : 20 € offerts", "Thu, 08 Oct 2026 10:00:00 +0200", "Total 20,00 €"));
    message(&root.join(".Sent"), 6, &mail("i1@example.org", "Jane <jane@example.org>", "Invoice 2026-042", "Fri, 09 Oct 2026 10:00:00 +0200", "Please find invoice 2026-042.\r\nTotal 480,00 €"));
    message(&root.join(".Junk"), 7, &mail("j1@scam.example", "Banque <alerte@scam.example>", "Confirmation de paiement", "Sat, 10 Oct 2026 10:00:00 +0200", "Vous avez payé 999,00 €."));
    let config: crate::config::Config = toml::from_str(&format!("language = \"fr\"\n[[account]]\nid = \"{account}\"\nkind = \"imap\"\naddress = \"jane@example.org\"\nmaildir = \"{}\"\n", root.display())).unwrap();
    let ledger: Ledger = toml::from_str("[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n[[line]]\nid = 'r'\nbudget = 'home'\ndate = 2026-10-06\namount = -387\nlabel = 'Exemple Retail'\nlinks = ['mid:r1@shop.example']\n").unwrap();
    let (bank, store) = (crate::bank::Bank::default(), Store::default());
    let words = crate::words::Words::of(&config);
    let input = Input { ledger: &ledger, bank: &bank, contracts: &[], letters: &[], store: &store, words: &words };
    let (movements, gone) = derive_with_gone(&input, day("2026-09-01"), day("2026-12-31"), day("2026-11-01"));
    let books = Books { counted: ledger.clone(), ledger, bank, movements, gone, from: day("2026-09-01"), to: day("2026-12-31") };
    let mut ignored = BTreeSet::new();
    not_a_payment(&mut ignored, "mid:p1@paypal.example", Some("paypal.fr"), "PayPal", "Vous avez reçu un paiement");
    let page = history(&config, &books, &store, &ignored, &MailQuery::default(), usize::MAX);
    let keys: Vec<&str> = page.rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["mid:i1@example.org", "mid:r1@shop.example", "mid:p2@paypal.example", "mid:p1@paypal.example"], "{:#?}", page.rows);
    let became: Vec<&Became> = page.rows.iter().map(|r| &r.became).collect();
    assert!(matches!(became[1], Became::Recorded { budget, .. } if budget == "home"), "{became:?}");
    assert_eq!((became[2], became[3]), (&Became::Ignored { shape: true }, &Became::Ignored { shape: false }));
    assert_eq!(became[0], &Became::Untied);
    assert_eq!((page.rows[0].kind.as_str(), page.rows[0].amount), ("bill", Some(Money(-48000))), "Sent mail is read too");
    let tr = Translator::new("en");
    let words_of = |r: &MailRow| row_words(&tr, &books.ledger, r);
    assert_eq!(words_of(&page.rows[1])[2], "counted in Home");
    assert_eq!(words_of(&page.rows[2])[2], "not counted: mail like this from PayPal is no longer counted");
    assert_eq!(words_of(&page.rows[0])[2], "not tied to any movement");
    // Its filters: tied or not, words, kind.
    let found = |q: MailQuery| history(&config, &books, &store, &ignored, &q, usize::MAX).rows.into_iter().map(|r| r.key).collect::<Vec<_>>();
    assert_eq!(found(MailQuery { tied: Some(true), ..MailQuery::default() }), ["mid:r1@shop.example"]);
    assert_eq!(found(MailQuery { text: "387".into(), ..MailQuery::default() }), ["mid:r1@shop.example"]);
    assert_eq!(found(MailQuery { kind: Some(PaymentKind::Received), way: Some(Way::In), ..MailQuery::default() }), ["mid:p2@paypal.example", "mid:p1@paypal.example"]);
    // Read once, ever: the index keeps each message's word, even for nothing.
    let reader = Reader::new(&books.ledger.mail_rules, &words);
    let picnic = std::fs::read_dir(root.join("cur")).unwrap().filter_map(Result::ok).map(|e| e.path()).find(|p| p.to_string_lossy().contains(".U7-4.")).unwrap();
    assert_eq!(crate::mailpass::kept_for(&reader, &account, &picnic), Some(None));
    let again = crate::mailpass::folder(&reader, &account, &root, "", usize::MAX);
    assert_eq!(again.opened, 0, "nothing opened twice");
    let _ = crate::mailpass::forget_account(&account);
    if let Some(dir) = crate::mailcache::account_dir(&account) {
        let _ = std::fs::remove_dir_all(dir);
    }
    let _ = std::fs::remove_dir_all(&root);
}

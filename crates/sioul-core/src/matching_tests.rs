// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The matching's tests (`matching`), on invented payees and references.

use super::*;

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

fn words() -> std::sync::Arc<crate::words::Words> {
    crate::words::Words::builtin()
}

fn side(way: Way, cents: i64, on: &str, evidence: Evidence, name: &str) -> Side {
    Side { way, sum: Sum::Exact(Money(cents)), currency: "EUR".into(), day: day(on), evidence, accounts: Vec::new(), names: vec![tokens(&words().bank, name)], refs: Vec::new() }
}

/// Names compared as the research says (Cohen et al. 2003): the bank's
/// words that name nothing, card numbers, dates and a processor's prefix
/// taken off; words a little misspelt still close; other payees far.
#[test]
fn names_are_compared_token_by_token() {
    let w = words();
    assert_eq!(tokens(&w.bank, "PAYPAL *SHOP EXEMPLE 05/10 X1234"), ["shop", "exemple"]);
    assert_eq!(tokens(&w.bank, "CB SHOP"), ["shop"]);
    assert_eq!(tokens(&w.bank, "PAYPAL"), ["paypal"], "a processor alone stays its name");
    assert!((jaro_winkler("martha", "marhta") - 0.961).abs() < 0.001);
    assert_eq!(jaro_winkler("abc", "abc"), 1.0);
    let names = [tokens(&w.bank, "PRLV SEPA RENT AGENCY"), tokens(&w.bank, "Rent"), tokens(&w.bank, "CB NETFLIX"), tokens(&w.bank, "Spotify")];
    let idf = Idf::of(names.iter());
    assert!(similarity(&names[0], &names[1], &idf) > 0.5, "the rent and its agency");
    assert_eq!(similarity(&names[2], &names[3], &idf), 0.0, "two subscriptions");
    assert!(similarity(&tokens(&w.bank, "Exemplestream"), &tokens(&w.bank, "EXEMPLSTREAM"), &idf) > 0.5, "a letter dropped");
}

/// References: an invoice's number after its word, a creditor's identifier
/// anywhere, a mandate's after its word; a label's numbers, never its dates.
#[test]
fn references_are_found_and_dates_are_not() {
    let leads = &words().payments.references;
    assert_eq!(references(leads, "Votre facture n° FA-2026-0042 du 2 novembre"), ["ref:FA20260042"]);
    assert_eq!(references(leads, "Identifiant créancier FR12ZZZ123456, RUM : EDF-778899-01"), ["ics:FR12ZZZ123456", "rum:EDF77889901"]);
    let label = label_references(leads, "VIR CLIENT A FACTURE 2026-002 05/11/26 051126");
    assert!(label.contains(&"ref:2026002".to_string()), "{label:?}");
    assert!(!label.iter().any(|r| r.contains("051126")), "a day is no reference: {label:?}");
    assert!(label_references(leads, "PRLV SEPA EDF FR12ZZZ123456").contains(&"ics:FR12ZZZ123456".to_string()));
}

/// Debits move off weekends and TARGET2's holidays (the ECB's calendar):
/// Easter 2026 is on 5 April, so Good Friday the 3rd and Easter Monday the 6th close.
#[test]
fn business_days_follow_target2() {
    assert!(!business(day("2026-04-03")) && !business(day("2026-04-06")) && business(day("2026-04-07")));
    assert!(!business(day("2026-12-25")) && !business(day("2026-12-26")) && !business(day("2027-01-01")) && !business(day("2026-05-01")));
    let saturday = day("2026-11-07");
    assert_eq!(saturday.weekday(), Weekday::Saturday);
    assert_eq!(next_business(saturday), day("2026-11-09"), "the Monday after");
    assert_eq!(business_after(day("2026-12-24"), 1), day("2026-12-28"), "past Christmas and the day after");
}

/// The score's three outcomes (Fellegi & Sunter 1969): sure (its amount, its
/// day, its name), probable (its amount and day, another name), apart (an
/// amount out of its kind's tolerance, a day out of its window); a card
/// payment a few percent off; a refund partly given back; another currency
/// told by its name, never by a near amount alone.
#[test]
fn the_score_says_sure_probable_or_apart() {
    let all = [tokens(&words().bank, "Rent"), tokens(&words().bank, "PRLV SEPA RENT AGENCY"), tokens(&words().bank, "Plumber"), tokens(&words().bank, "CB PLOMBERIE DUPONT")];
    let idf = Idf::of(all.iter());
    let rent = side(Way::Out, -60000, "2026-11-01", Evidence::Debit, "Rent");
    let sure = score(&rent, &side(Way::Out, -60000, "2026-11-02", Evidence::Transfer, "PRLV SEPA RENT AGENCY"), &idf).unwrap();
    assert!(sure.score >= SURE, "{sure:?}");
    let plumber = side(Way::Out, -12000, "2026-11-25", Evidence::Transfer, "Plumber");
    let probable = score(&plumber, &side(Way::Out, -12000, "2026-11-26", Evidence::Transfer, "CB PLOMBERIE DUPONT"), &idf).unwrap();
    assert!(probable.score >= PROBABLE && probable.score < SURE, "{probable:?}");
    // A debit whose amount changed, by its plain name: the same, probably, as the
    // watch says it "changed"; another payee's changed amount, nothing.
    let changed = score(&rent, &side(Way::Out, -61000, "2026-11-02", Evidence::Transfer, "PRLV SEPA RENT AGENCY"), &idf).unwrap();
    assert!(changed.score >= PROBABLE && changed.score < SURE, "{changed:?}");
    assert_eq!(score(&rent, &side(Way::Out, -61000, "2026-11-02", Evidence::Transfer, "CB PLOMBERIE DUPONT"), &idf), None, "another payee's");
    assert_eq!(score(&rent, &side(Way::Out, -60000, "2026-11-20", Evidence::Transfer, "PRLV SEPA RENT AGENCY"), &idf), None, "out of its window");
    assert_eq!(score(&rent, &side(Way::In, 60000, "2026-11-02", Evidence::Transfer, "PRLV SEPA RENT AGENCY"), &idf), None, "the other way");
    // A card payment with a tip, within a few percent; a refund partly given back, weeks later.
    let meal = side(Way::Out, -3800, "2026-11-03", Evidence::Card, "Restaurant Exemple");
    assert!(score(&meal, &side(Way::Out, -3950, "2026-11-05", Evidence::Transfer, "CB RESTAURANT EXEMPLE"), &idf).is_some());
    assert!(score(&meal, &side(Way::Out, -4500, "2026-11-05", Evidence::Transfer, "CB RESTAURANT EXEMPLE"), &idf).is_none());
    let refund = side(Way::In, 2398, "2026-10-01", Evidence::Refund, "Shop");
    assert!(score(&refund, &side(Way::In, 1000, "2026-11-04", Evidence::Transfer, "PAYPAL *SHOP"), &idf).is_some(), "partly, five weeks on");
    assert!(score(&refund, &side(Way::In, 3000, "2026-11-04", Evidence::Transfer, "PAYPAL *SHOP"), &idf).is_none(), "never more");
    // Another currency: $20.00 settled €18.47, said by its name; without the name, nothing.
    let mut dollars = side(Way::Out, -2000, "2026-11-01", Evidence::Card, "Exempleai");
    dollars.currency = "USD".into();
    assert!(score(&dollars, &side(Way::Out, -1847, "2026-11-03", Evidence::Transfer, "CB EXEMPLEAI SAN FRANCISCO"), &idf).is_some());
    assert!(score(&dollars, &side(Way::Out, -1847, "2026-11-03", Evidence::Transfer, "CB BOULANGERIE"), &idf).is_none());
}

/// One to one, the best first; a pair barely better than the next one only
/// probable (two coffees on one day); a pair "Not the same" never made.
#[test]
fn assignment_is_one_to_one_and_doubts_near_equals() {
    let expected = [side(Way::Out, -999, "2026-11-05", Evidence::Debit, "Exemplestream"), side(Way::Out, -999, "2026-11-05", Evidence::Debit, "Musicexemple")];
    let facts = [side(Way::Out, -999, "2026-11-05", Evidence::Transfer, "CB MUSICEXEMPLE"), side(Way::Out, -999, "2026-11-05", Evidence::Transfer, "CB EXEMPLESTREAM")];
    let idf = Idf::of(expected.iter().chain(facts.iter()).flat_map(|s| s.names.iter()));
    let pairs = assign(&expected, &facts, &idf, &|_, _| true);
    assert_eq!(pairs.iter().map(|p| (p.expected, p.fact, p.outcome)).collect::<Vec<_>>(), vec![(0, 1, Outcome::Sure), (1, 0, Outcome::Sure)], "the names decide");
    // Two coffees, one receipt: one of them, probably.
    let receipt = [side(Way::Out, -250, "2026-11-05", Evidence::Card, "Cafe Exemple")];
    let coffees = [side(Way::Out, -250, "2026-11-05", Evidence::Transfer, "CB CAFE EXEMPLE"), side(Way::Out, -250, "2026-11-05", Evidence::Transfer, "CB CAFE EXEMPLE")];
    let idf = Idf::of(receipt.iter().chain(coffees.iter()).flat_map(|s| s.names.iter()));
    let pairs = assign(&receipt, &coffees, &idf, &|_, _| true);
    assert_eq!(pairs.iter().map(|p| (p.fact, p.outcome)).collect::<Vec<_>>(), vec![(0, Outcome::Probable)]);
    // "Not the same" for the first: the second, never the first.
    let pairs = assign(&receipt, &coffees, &idf, &|_, f| f != 0);
    assert_eq!(pairs.iter().map(|p| p.fact).collect::<Vec<_>>(), vec![1]);
    // Two or three adding up: to the cent, or within a hundredth.
    assert_eq!(adding_up(Money(-10000), &[(7, Money(-6000)), (8, Money(-2500)), (9, Money(-4000))], 0), Some(vec![7, 9]));
    assert_eq!(adding_up(Money(-10000), &[(7, Money(-6050)), (9, Money(-4000))], 0), None);
    assert_eq!(adding_up(Money(-10000), &[(7, Money(-6050)), (9, Money(-4000))], 100), Some(vec![7, 9]));
}

/// A payout without the processor's export carries the payments told of,
/// oldest first, as far as its amount goes, its fees within the bounds: three
/// payments of €520, €400 and €300 paid out as €1,184.10 carry all three
/// (€35.90 of fees); a payout of €504.57 carries the first alone; one that
/// asks for fees no processor takes carries nothing.
#[test]
fn a_payout_carries_the_oldest_payments_within_the_bounds() {
    // PayPal's own bounds (presets/processors.json): up to 6.5 % plus €0.35 a payment.
    let paypal = crate::processors::built_in().by_id("paypal").unwrap().bounds;
    let bounds = |gross: Money, n: usize| paypal.of(gross, n);
    let told = [Money(52000), Money(40000), Money(30000)];
    assert_eq!(carried_prefix(Money(118410), &told, &bounds), Some(3));
    assert_eq!(carried_prefix(Money(50457), &told, &bounds), Some(1));
    assert_eq!(carried_prefix(Money(88000), &told, &bounds), Some(2), "€920 paid, €40 of fees: within 6.5 % plus €0.70");
    assert_eq!(carried_prefix(Money(70000), &told, &bounds), None, "€220 of fees on €920: no processor's");
    assert_eq!(carried_prefix(Money(130000), &told, &bounds), None, "more than was told of");
    assert_eq!(carried_prefix(Money(-5000), &told, &bounds), None);
    // A platform that always keeps a share: the fewest payments that leave at least its least.
    let etsy = crate::processors::built_in().by_id("etsy").unwrap().bounds;
    assert_eq!(carried_prefix(Money(10000), &[Money(10000), Money(5000)], &|g, n| etsy.of(g, n)), None, "no fee at all is no Etsy payout, nor a third of it");
    assert_eq!(carried_prefix(Money(9350), &[Money(10000), Money(1000)], &|g, n| etsy.of(g, n)), Some(1));
    assert_eq!(plain_words("VIR PAYPAL EUROPE S.A.R.L."), ["vir", "paypal", "europe", "s", "a", "r", "l"]);
}

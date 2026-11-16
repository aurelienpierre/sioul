// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Factur-X XML of the samples (`invoice::samples`): each VAT case with
//! its category, reason and identifiers as EN 16931 asks; read back by
//! Sioul's own reader of received e-invoices. The schema and the rules
//! themselves are checked by the CI's validator (Mustang), over the same
//! samples.

use super::*;
use crate::invoice::samples;

/// The text of every element named `name` (its local name), in order.
fn all(xml: &str, name: &str) -> Vec<String> {
    let doc = roxmltree::Document::parse(xml).expect("well-formed XML");
    doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == name).map(|n| n.text().unwrap_or("").trim().to_string()).collect()
}

/// The `schemeID` of every element named `name`, with its text.
fn schemes(xml: &str, name: &str) -> Vec<(String, String)> {
    let doc = roxmltree::Document::parse(xml).expect("well-formed XML");
    doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == name).map(|n| (n.attribute("schemeID").unwrap_or("").to_string(), n.text().unwrap_or("").trim().to_string())).collect()
}

fn sample(name: &str) -> samples::Sample {
    samples::valid().into_iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no sample {name}"))
}

/// Each sample has its XML, in the EN 16931 profile, read back as it was issued.
#[test]
fn every_sample_reads_back() {
    for s in samples::valid() {
        let xml = xml(&s.invoice).unwrap_or_else(|why| panic!("{}: {why:?}", s.name));
        assert_eq!(all(&xml, "ID").get(if s.invoice.issuer.preset == "fr" { 1 } else { 0 }).map(String::as_str), Some(PROFILE), "{}", s.name);
        let read = crate::einvoice::from_xml(&xml).unwrap_or_else(|| panic!("{}: not read back", s.name));
        assert_eq!(read.number, s.invoice.number, "{}", s.name);
        assert_eq!(read.amount.map(|m| m.0), Some(s.invoice.totals.due), "{}", s.name);
        assert_eq!(read.currency, s.invoice.currency, "{}", s.name);
        assert_eq!(all(&xml, "TypeCode")[0], s.invoice.kind.code().to_string(), "{}", s.name);
        // Each line, each breakdown.
        assert_eq!(all(&xml, "LineID").len(), s.invoice.lines.len(), "{}", s.name);
        assert_eq!(all(&xml, "BasisAmount").len(), s.invoice.totals.breakdown.len(), "{}", s.name);
    }
}

/// The French franchise: exempt (E), its code and its mention, the SIREN as
/// the legal registration, the VAT number given; without one, no XML yet.
#[test]
fn the_french_franchise_is_exempt_with_its_mention() {
    let s = sample("fr-franchise");
    let text = xml(&s.invoice).unwrap();
    assert!(all(&text, "CategoryCode").iter().all(|c| c == "E"), "{text}");
    assert_eq!(all(&text, "ExemptionReasonCode"), ["VATEX-FR-FRANCHISE"]);
    assert_eq!(all(&text, "ExemptionReason"), ["TVA non applicable, article 293 B du CGI"]);
    assert!(schemes(&text, "ID").contains(&("0002".into(), "732829320".into())), "the SIREN of the SIRET");
    assert!(schemes(&text, "ID").iter().any(|(scheme, value)| scheme == "VA" && value.starts_with("FR")));
    assert_eq!(all(&text, "ID")[0], "S1", "France's business process: services");
    // Time: minutes at an hourly price for 60 of them.
    assert!(text.contains("<ram:BasisQuantity unitCode=\"MIN\">60</ram:BasisQuantity>") && text.contains("<ram:BilledQuantity unitCode=\"MIN\">150</ram:BilledQuantity>"), "{text}");
    assert!(text.contains("<udt:DateTimeString format=\"102\">20261102</udt:DateTimeString>"), "{text}");
    let without_vat = samples::without_xml();
    assert_eq!(without(&without_vat.invoice), Some(Without::FranchiseWithoutVat));
    assert_eq!(xml(&without_vat.invoice), Err(Without::FranchiseWithoutVat));
}

/// The reverse charge: AE, both VAT numbers; not subject to VAT: O, no VAT
/// number on either side, no rate; intra-EU goods and exports: K and G with their codes.
#[test]
fn each_vat_case_has_its_category() {
    let xml_of = |name: &str| xml(&sample(name).invoice).unwrap();
    let reverse = xml_of("fr-reverse-charge");
    assert_eq!(all(&reverse, "ExemptionReasonCode"), ["VATEX-EU-AE"]);
    let vat: Vec<String> = schemes(&reverse, "ID").into_iter().filter(|(s, _)| s == "VA").map(|(_, v)| v).collect();
    assert_eq!(vat.len(), 2, "both VAT numbers: {reverse}");
    assert!(vat.contains(&"BE0123456749".to_string()));

    let outside = xml_of("uk-not-registered");
    assert_eq!(all(&outside, "CategoryCode").iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>(), ["O"]);
    assert!(all(&outside, "RateApplicablePercent").is_empty(), "no rate when not subject to VAT");
    assert_eq!(all(&outside, "ExemptionReasonCode"), ["VATEX-EU-O"]);
    assert!(schemes(&outside, "ID").iter().all(|(s, _)| s != "VA"));
    assert!(schemes(&outside, "ID").contains(&("FC".into(), "1234567890".into())), "the UTR as a tax number");

    let intra = xml_of("fr-intra-eu-goods");
    assert_eq!(all(&intra, "ExemptionReasonCode"), ["VATEX-EU-IC"]);
    assert_eq!(all(&intra, "CountryID").last().map(String::as_str), Some("DE"), "delivered to its country");
    assert_eq!(all(&intra, "ID")[0], "B1", "goods");
    assert_eq!(all(&xml_of("fr-export"), "ExemptionReasonCode"), ["VATEX-EU-G"]);

    let liable = xml_of("fr-liable");
    assert_eq!(all(&liable, "RateApplicablePercent").into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>(), ["20", "5.5"]);
    assert!(all(&liable, "ExemptionReason").is_empty(), "taxed: no exemption");
    assert_eq!(all(&liable, "ActualAmount"), ["100.00"], "the discount");
    assert_eq!(all(&liable, "ID")[0], "M1", "goods and services");
    assert_eq!(all(&liable, "TaxTotalAmount"), [amount(sample("fr-liable").invoice.totals.tax)]);

    let german = xml_of("de-franchise");
    assert_eq!(all(&german, "ExemptionReason"), ["Steuerbefreiung für Kleinunternehmer gemäß § 19 UStG"]);
    assert!(schemes(&german, "ID").contains(&("FC".into(), "12/345/67890".into())), "the Steuernummer");
}

/// A credit note cites the invoice it credits; a final invoice deducts its
/// deposits; IBAN, BIC and the reference a transfer carries.
#[test]
fn credit_notes_deposits_and_payment() {
    let credit = xml(&sample("fr-credit-note").invoice).unwrap();
    assert_eq!(all(&credit, "TypeCode")[0], "381");
    assert!(credit.contains("<ram:IssuerAssignedID>2026-001</ram:IssuerAssignedID>") && credit.contains("<qdt:DateTimeString format=\"102\">20261116</qdt:DateTimeString>"), "{credit}");
    assert!(all(&credit, "IBANID").is_empty(), "nothing to pay on a credit note");
    let last = sample("fr-final");
    let text = xml(&last.invoice).unwrap();
    assert_eq!(all(&text, "TotalPrepaidAmount"), [amount(last.invoice.totals.deposits)]);
    assert_eq!(all(&text, "DuePayableAmount"), [amount(last.invoice.totals.due)]);
    assert_eq!(all(&text, "IBANID"), ["FR7630006000011234567890189"]);
    assert_eq!(all(&text, "BICID"), ["EXMPFRPP"]);
    assert_eq!(all(&text, "PaymentReference"), ["2026-006"]);
    assert!(text.contains("<ram:TypeCode>58</ram:TypeCode>"), "a SEPA transfer: {text}");
    assert_eq!(all(&xml(&sample("fr-deposit").invoice).unwrap(), "TypeCode")[0], "386");
}

/// Lines outside the scope of VAT beside taxed ones: no XML (BR-O-11 to 14), said.
#[test]
fn outside_the_scope_stays_alone() {
    let mut mixed = sample("fr-liable").invoice;
    mixed.totals.breakdown.push(crate::invoice::TaxBreak { category: "O".into(), ..Default::default() });
    assert_eq!(without(&mixed), Some(Without::OutsideMixed));
    assert!(Without::OutsideMixed.said(&Translator::new("fr")).contains("Factur-X"));
}

#[test]
fn numbers_are_written_as_the_schema_reads_them() {
    assert_eq!((amount(123450), amount(-5), amount(0)), ("1234.50".to_string(), "-0.05".to_string(), "0.00".to_string()));
    assert_eq!((decimal(1500, 3), decimal(2000, 3), decimal(125, 3), decimal(550, 2), decimal(2000, 2)), ("1.5".into(), "2".into(), "0.125".into(), "5.5".into(), "20".into()));
    assert_eq!(day("2026-11-16"), Some("20261116".into()));
    assert_eq!(clean("a\u{1}b & <c>"), "ab & <c>");
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Invoices of fictional businesses on example.org, one per preset and VAT
//! scheme, for the tests of the issued file and for the fixtures the CI's
//! validator reads (`.github/workflows/e-invoices.yml`). Their SIREN, SIRET
//! and French VAT numbers are made up and pass their checks; the IBANs are
//! the documented examples of each country (ISO 13616, as Wikipedia's
//! article on the IBAN lists them), their keys valid.
//!
//! `write_fixtures` (ignored unless asked) writes them as Sioul issues
//! them: `valid/` holds the Factur-X files a validator must accept,
//! `invalid/` one made wrong on purpose (its totals do not add up),
//! `without-xml/` the French franchise without a VAT number, a PDF/A-3 that
//! carries no XML yet (design §12, question 1).
//!
//!     SIOUL_INVOICE_FIXTURES=/some/folder cargo test --release -p sioul-core --lib -- --ignored --exact invoice::samples::write_fixtures

use super::life;
use super::pdf::{self, Options};
use super::record::{Address, Client, Identifier, Invoice, Kind, Line, Nature};
use crate::config::{BankSetting, IdSetting, InvoiceSettings, TaxSetting};
use jiff::civil::Date;

pub(crate) const SIRET: &str = "732 829 320 00017";
pub(crate) const SIREN: &str = "732 829 320";
pub(crate) const CLIENT_SIREN: &str = "904 123 452";
pub(crate) const IBAN_FR: &str = "FR76 3000 6000 0112 3456 7890 189";
pub(crate) const IBAN_DE: &str = "DE89 3704 0044 0532 0130 00";
pub(crate) const IBAN_BE: &str = "BE68 5390 0754 7034";
pub(crate) const IBAN_GB: &str = "GB82 WEST 1234 5698 7654 32";
pub(crate) const IBAN_CH: &str = "CH93 0076 2011 6238 5295 7";

/// One sample: its name, the settings it was issued under, the invoice.
pub(crate) struct Sample {
    pub name: &'static str,
    pub settings: InvoiceSettings,
    pub invoice: Invoice,
}

fn day(text: &str) -> Date {
    text.parse().expect("a day")
}

fn id(kind: &str, value: &str) -> IdSetting {
    IdSetting { kind: kind.into(), value: value.into(), printed: true }
}

fn scheme(scheme: &str, mention: &str) -> Vec<TaxSetting> {
    vec![TaxSetting { from: String::new(), scheme: scheme.into(), mention: mention.into() }]
}

fn bank(iban: &str, bic: &str) -> Vec<BankSetting> {
    vec![BankSetting { holder: String::new(), iban: iban.into(), bic: bic.into() }]
}

/// A French EI micro-entrepreneur, under `scheme`, with or without a VAT number.
pub(crate) fn french(scheme_id: &str, with_vat: bool) -> InvoiceSettings {
    let mut ids = vec![id("siret", SIRET)];
    if with_vat {
        ids.push(id("vat", &super::ids::vat_fr_of(SIREN).expect("a VAT number")));
    }
    InvoiceSettings {
        preset: "fr".into(),
        legal_form: "ei".into(),
        regime: "micro".into(),
        name: "Camille Exemple".into(),
        address: "2 place de l’Exemple".into(),
        postcode: "69001".into(),
        city: "Lyon".into(),
        email: "camille@example.org".into(),
        ids,
        tax: scheme(scheme_id, ""),
        banks: bank(IBAN_FR, "EXMPFRPP"),
        rate: 60.0,
        ..InvoiceSettings::default()
    }
}

fn client(name: &str, street: &str, postcode: &str, city: &str, country: &str, ids: &[(&str, &str)]) -> Client {
    Client {
        kind: "business".into(),
        name: name.into(),
        address: Address { lines: vec![street.into()], postcode: postcode.into(), city: city.into(), country: country.into() },
        ids: ids.iter().map(|(k, v)| Identifier { kind: (*k).into(), value: (*v).into(), printed: true }).collect(),
        email: "billing@example.org".into(),
        ..Client::default()
    }
}

pub(crate) fn french_client() -> Client {
    client("Studio Lumen\nAnna Exemple", "1 rue de l’Exemple", "75002", "Paris", "FR", &[("siren", CLIENT_SIREN)])
}

/// A line of `quantity` thousandths of `unit` at `price` cents, of `nature`, at `rate` basis points.
pub(crate) fn line(description: &str, quantity: i64, unit: &str, price: i64, nature: Nature, category: &str, rate: i64) -> Line {
    Line { quantity, unit: unit.into(), price, base: 1, nature, category: category.into(), rate, ..Line::of_time(description, 0, 0, category, rate) }
}

/// A draft made as `life::issue` makes it on `date`, numbered `number`, nothing written.
pub(crate) fn issued(s: &InvoiceSettings, client: Client, language: &str, currency: &str, lines: Vec<Line>, number: &str, date: &str) -> Invoice {
    let date = day(date);
    let terms = life::terms_of(s, None, None);
    let mut draft = life::blank(s, client, "", language, currency, terms, date);
    draft.lines = lines;
    draft.date = date.to_string();
    draft.number = number.into();
    life::refresh(&mut draft, s, date);
    draft
}

/// Time billed: a line per task, minutes at an hourly price.
fn time(lines: &[(&str, u32)], price: i64) -> Vec<Line> {
    lines.iter().map(|(d, m)| Line::of_time(d, *m, price, "E", 0)).collect()
}

/// Every sample with Factur-X data, in both languages where it matters.
pub(crate) fn valid() -> Vec<Sample> {
    let mut out = Vec::new();
    let mut add = |name: &'static str, settings: InvoiceSettings, invoice: Invoice| out.push(Sample { name, settings, invoice });

    // France, the franchise, a VAT number (services to businesses abroad ask one).
    let s = french("franchise", true);
    let mut inv = issued(&s, french_client(), "fr", "EUR", time(&[("Maquettes du site", 150), ("Réunion de lancement", 45)], 6000), "2026-001", "2026-11-16");
    inv.period_from = "2026-11-02".into();
    inv.period_to = "2026-11-13".into();
    inv.order = "BC-2026-17".into();
    add("fr-franchise", s.clone(), inv.clone());

    // Its credit note: one line cancelled.
    let mut credit = life::credit(&inv, Some(&[1]), &s, day("2026-11-20"));
    credit.number = "2026-002".into();
    credit.date = "2026-11-20".into();
    life::refresh(&mut credit, &s, day("2026-11-20"));
    add("fr-credit-note", s.clone(), credit);

    // Services to a business in another member state: the reverse charge, both VAT numbers.
    let belgian = client("Atelier Exemple SRL", "Rue de l’Exemple 1", "1000", "Bruxelles", "BE", &[("vat", "BE0123456749")]);
    add("fr-reverse-charge", s.clone(), issued(&s, belgian, "en", "EUR", vec![line("Photography workshop", 1000, "DAY", 45000, Nature::Service, "E", 0)], "2026-003", "2026-11-16"));

    // An EI liable for VAT: two rates, a discount, a deposit invoice and the final one deducting it.
    let s = french("liable", true);
    let mut workshop = line("Atelier photo", 2000, "DAY", 50000, Nature::Service, "S", 2000);
    workshop.discount = 10000;
    workshop.details = "Deux journées, matériel compris".into();
    let book = line("Livre « Lumière »", 3000, "C62", 2500, Nature::Goods, "S", 550);
    add("fr-liable", s.clone(), issued(&s, french_client(), "fr", "EUR", vec![workshop, book.clone()], "2026-004", "2026-11-16"));
    let blank = issued(&s, french_client(), "fr", "EUR", Vec::new(), "", "2026-11-16");
    let mut deposit = life::deposit(&s, blank, "Acompte, mission de conseil", 30000, day("2026-11-16"));
    deposit.number = "2026-005".into();
    deposit.date = "2026-11-16".into();
    life::refresh(&mut deposit, &s, day("2026-11-16"));
    add("fr-deposit", s.clone(), deposit.clone());
    let mut last = issued(&s, french_client(), "fr", "EUR", vec![line("Mission de conseil", 5000, "DAY", 40000, Nature::Service, "S", 2000)], "2026-006", "2026-12-15");
    life::cite_deposits(&mut last, &[deposit], &s, day("2026-12-15"));
    add("fr-final", s.clone(), last);

    // Goods to a business in another member state (exempt delivery), and an export outside the EU.
    let german = client("Beispiel GmbH", "Beispielstraße 1", "10115", "Berlin", "DE", &[("vat", "DE123456789")]);
    add("fr-intra-eu-goods", s.clone(), issued(&s, german, "en", "EUR", vec![line("Prints, 40 × 50 cm", 4000, "C62", 12000, Nature::Goods, "K", 0)], "2026-007", "2026-11-16"));
    let swiss = client("Beispiel AG", "Bahnhofstrasse 1", "8001", "Zürich", "CH", &[]);
    add("fr-export", s.clone(), issued(&s, swiss, "en", "EUR", vec![line("Prints, 40 × 50 cm", 2000, "C62", 12000, Nature::Goods, "G", 0)], "2026-008", "2026-11-16"));

    // A company: its capital and register printed.
    let company = InvoiceSettings { legal_form: "sasu".into(), name: "Lumen Conseil".into(), capital: "1 000 €".into(), ids: vec![id("siren", SIREN), id("rcs", "Lyon"), id("vat", &super::ids::vat_fr_of(SIREN).expect("a VAT number"))], ..french("liable", true) };
    add("fr-company", company.clone(), issued(&company, french_client(), "fr", "EUR", time(&[("Conseil", 240)], 9000), "2026-009", "2026-11-16"));

    // Germany: the small business (§ 19 UStG) with its tax number; liable at 19 %.
    let german_seller = |scheme_id: &str| InvoiceSettings {
        preset: "de".into(),
        legal_form: "sole".into(),
        name: "Kim Beispiel".into(),
        address: "Beispielweg 2".into(),
        postcode: "10117".into(),
        city: "Berlin".into(),
        email: "kim@example.org".into(),
        ids: if scheme_id == "liable" { vec![id("steuernummer", "12/345/67890"), id("vat", "DE123456788")] } else { vec![id("steuernummer", "12/345/67890")] },
        tax: scheme(scheme_id, ""),
        banks: bank(IBAN_DE, "EXMPDEFF"),
        ..InvoiceSettings::default()
    };
    let german_client = client("Muster KG", "Musterstraße 3", "80331", "München", "DE", &[]);
    let s = german_seller("franchise");
    add("de-franchise", s.clone(), issued(&s, german_client.clone(), "en", "EUR", vec![line("Webdesign", 1000, "LS", 120000, Nature::Service, "E", 0)], "2026-010", "2026-11-16"));
    let s = german_seller("liable");
    add("de-liable", s.clone(), issued(&s, german_client, "en", "EUR", vec![line("Webdesign", 1000, "LS", 120000, Nature::Service, "S", 1900)], "2026-011", "2026-11-16"));

    // Belgium: liable at 21 %; the franchise, its mention the person's own words.
    let belgian_seller = |scheme_id: &str, mention: &str| InvoiceSettings {
        preset: "be".into(),
        legal_form: "sole".into(),
        name: "Alex Exemple".into(),
        address: "Rue de l’Exemple 2".into(),
        postcode: "1000".into(),
        city: "Bruxelles".into(),
        email: "alex@example.org".into(),
        ids: vec![id("vat", "BE0203201340")],
        tax: scheme(scheme_id, mention),
        banks: bank(IBAN_BE, ""),
        ..InvoiceSettings::default()
    };
    let belgian_client = client("Exemple SA", "Avenue de l’Exemple 5", "4000", "Liège", "BE", &[("vat", "BE0123456749")]);
    let s = belgian_seller("liable", "");
    add("be-liable", s.clone(), issued(&s, belgian_client.clone(), "fr", "EUR", vec![line("Traduction", 12000, "C62", 1500, Nature::Service, "S", 2100)], "2026-012", "2026-11-16"));
    let s = belgian_seller("franchise", "Régime particulier de franchise des petites entreprises");
    add("be-franchise", s.clone(), issued(&s, belgian_client, "fr", "EUR", vec![line("Traduction", 12000, "C62", 1500, Nature::Service, "E", 0)], "2026-013", "2026-11-16"));

    // The United Kingdom: not registered (no VAT at all), and registered at 20 %, in pounds.
    let british_seller = |scheme_id: &str| InvoiceSettings {
        preset: "uk".into(),
        legal_form: "sole".into(),
        name: "Robin Example".into(),
        address: "2 Example Road".into(),
        postcode: "M1 1AA".into(),
        city: "Manchester".into(),
        email: "robin@example.org".into(),
        ids: if scheme_id == "liable" { vec![id("utr", "1234567890"), id("vat", "GB123456789")] } else { vec![id("utr", "1234567890")] },
        tax: scheme(scheme_id, ""),
        banks: bank(IBAN_GB, ""),
        currency: "GBP".into(),
        ..InvoiceSettings::default()
    };
    let british_client = client("Example Ltd", "1 Example Street", "EC1A 1BB", "London", "GB", &[]);
    let s = british_seller("none");
    add("uk-not-registered", s.clone(), issued(&s, british_client.clone(), "en", "GBP", time(&[("Illustration", 300)], 5000), "2026-014", "2026-11-16"));
    let s = british_seller("liable");
    add("uk-liable", s.clone(), issued(&s, british_client, "en", "GBP", vec![line("Illustration", 1000, "LS", 90000, Nature::Service, "S", 2000)], "2026-015", "2026-11-16"));

    // Switzerland: not registered; registered at 8.1 %, its UID with MWST its VAT number; in francs.
    let swiss_seller = |scheme_id: &str| InvoiceSettings {
        preset: "ch".into(),
        legal_form: "sole".into(),
        name: "Noa Beispiel".into(),
        address: "Beispielgasse 2".into(),
        postcode: "3011".into(),
        city: "Bern".into(),
        email: "noa@example.org".into(),
        ids: vec![id("uid", if scheme_id == "liable" { "CHE-123.456.788 MWST" } else { "CHE-123.456.788" })],
        tax: scheme(scheme_id, ""),
        banks: bank(IBAN_CH, ""),
        currency: "CHF".into(),
        ..InvoiceSettings::default()
    };
    let swiss_client = client("Muster GmbH", "Musterweg 4", "8001", "Zürich", "CH", &[]);
    let s = swiss_seller("none");
    add("ch-not-registered", s.clone(), issued(&s, swiss_client.clone(), "fr", "CHF", time(&[("Fotografie", 180)], 12000), "2026-016", "2026-11-16"));
    let s = swiss_seller("liable");
    add("ch-liable", s.clone(), issued(&s, swiss_client, "fr", "CHF", vec![line("Fotografie", 1000, "DAY", 98000, Nature::Service, "S", 810)], "2026-017", "2026-11-16"));

    // Elsewhere: every line one's own; not registered, a franchise of its own words, liable at a rate typed.
    let elsewhere = |scheme_id: &str, mention: &str, ids: Vec<IdSetting>| InvoiceSettings {
        preset: "elsewhere".into(),
        name: "Sam Example".into(),
        address: "2 Example Lane".into(),
        postcode: "97201".into(),
        city: "Portland".into(),
        country: "US".into(),
        email: "sam@example.org".into(),
        ids,
        tax: scheme(scheme_id, mention),
        banks: bank(IBAN_DE, ""),
        currency: "USD".into(),
        ..InvoiceSettings::default()
    };
    let american_client = client("Example LLC", "1 Example Avenue", "62701", "Springfield", "US", &[]);
    let s = elsewhere("none", "", vec![id("tax-id", "12-3456789")]);
    add("elsewhere-not-registered", s.clone(), issued(&s, american_client.clone(), "en", "USD", time(&[("Consulting", 120)], 15000), "2026-018", "2026-11-16"));
    let s = elsewhere("franchise", "Small business exemption", vec![id("tax-id", "12-3456789")]);
    add("elsewhere-franchise", s.clone(), issued(&s, american_client.clone(), "en", "USD", time(&[("Consulting", 120)], 15000), "2026-019", "2026-11-16"));
    // Liable where the rates are the person's: a Norwegian seller at 25 %, in crowns.
    let s = InvoiceSettings { address: "Eksempelgata 2".into(), postcode: "0150".into(), city: "Oslo".into(), country: "NO".into(), currency: "NOK".into(), ..elsewhere("liable", "", vec![id("vat", "NO123456785MVA")]) };
    add("elsewhere-liable", s.clone(), issued(&s, american_client, "en", "NOK", vec![line("Consulting", 2000, "HUR", 150000, Nature::Service, "S", 2500)], "2026-020", "2026-11-16"));

    // A long invoice: its lines over three pages, the head again on each.
    let s = french("franchise", true);
    let many: Vec<(String, u32)> = (1..=70).map(|n| (format!("Retouche de la photographie n° {n}, série « Lumière d’automne »"), 15 + (n % 4) * 15)).collect();
    let lines: Vec<Line> = many.iter().map(|(d, m)| Line::of_time(d, *m, 6000, "E", 0)).collect();
    add("fr-long", s.clone(), issued(&s, french_client(), "fr", "EUR", lines, "2026-021", "2026-11-16"));
    out
}

/// The French franchise without a VAT number: no XML until BR-FR-CO-16 is read.
pub(crate) fn without_xml() -> Sample {
    let s = french("franchise", false);
    let invoice = issued(&s, french_client(), "fr", "EUR", time(&[("Maquettes du site", 150)], 6000), "2026-030", "2026-11-16");
    Sample { name: "fr-franchise-no-vat", settings: s, invoice }
}

/// One made wrong on purpose: its total with VAT a cent off (BR-CO-15).
pub(crate) fn broken() -> Sample {
    let s = french("liable", true);
    let mut invoice = issued(&s, french_client(), "fr", "EUR", vec![line("Atelier photo", 1000, "DAY", 50000, Nature::Service, "S", 2000)], "2026-031", "2026-11-16");
    invoice.totals.with_tax += 1;
    invoice.totals.due += 1;
    Sample { name: "broken-totals", settings: s, invoice }
}

/// The fixtures, written where `SIOUL_INVOICE_FIXTURES` says (nothing without
/// it); marked a test's own for the scan of raw writers (`agenda`'s tests).
#[cfg(test)]
#[test]
#[ignore]
fn write_fixtures() {
    let Some(root) = std::env::var_os("SIOUL_INVOICE_FIXTURES").map(std::path::PathBuf::from) else { return };
    let write = |folder: &str, sample: &Sample| {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        let rendered = pdf::render(&sample.invoice, &Options { qr: sample.settings.qr != Some(false) }).unwrap_or_else(|e| panic!("{}: {e}", sample.name));
        std::fs::write(dir.join(format!("{}.pdf", sample.name)), &rendered.pdf).unwrap();
        if let Some(xml) = &rendered.xml {
            std::fs::write(dir.join(format!("{}.xml", sample.name)), xml).unwrap();
        }
        rendered
    };
    for sample in valid() {
        let rendered = write("valid", &sample);
        assert!(rendered.xml.is_some(), "{}: {:?}", sample.name, rendered.without);
    }
    assert!(write("without-xml", &without_xml()).xml.is_none());
    assert!(write("invalid", &broken()).xml.is_some());
    // The kinds: invoices, a credit note, a deposit invoice.
    assert!(valid().iter().any(|s| s.invoice.kind == Kind::CreditNote) && valid().iter().any(|s| s.invoice.kind == Kind::Deposit));
}

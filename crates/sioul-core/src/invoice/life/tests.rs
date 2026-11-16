// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The life of invoices, on fictional businesses of example.org; their SIREN,
//! SIRET and VAT numbers are made up and pass their checks.

use super::*;
use crate::config::{IdSetting, LineSetting, TaxSetting};
use crate::invoice::record::Status;
use crate::invoice::{all_in, draft_in, drafts_in, find_in, html, remove_draft, save_draft};
use crate::timelog::{self, Session};
use std::path::PathBuf;

const SIREN: &str = "732 829 320";
const SIRET: &str = "732 829 320 00017";
const CLIENT_SIREN: &str = "904 123 452";
const CLIENT_SIRET: &str = "904 123 452 00008";
const DESK: &str = "desk-1";

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

fn en() -> Translator {
    Translator::new("en")
}

/// Folders of a test's own: invoices, time, notes.
struct Place {
    root: PathBuf,
}

impl Place {
    fn new(name: &str) -> Place {
        let root = std::env::temp_dir().join(format!("sioul-life-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("notes")).unwrap();
        Place { root }
    }
    fn invoices(&self) -> PathBuf {
        self.root.join("invoices")
    }
    fn time(&self) -> PathBuf {
        self.root.join("time")
    }
    fn ledger(&self) -> PathBuf {
        self.root.join("notes").join(crate::budget::LEDGER)
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A French EI micro-entrepreneur, as Settings says it now.
fn french(scheme: &str) -> InvoiceSettings {
    InvoiceSettings {
        preset: "fr".into(),
        legal_form: "ei".into(),
        regime: "micro".into(),
        name: "Camille Exemple".into(),
        address: "2 place de l’Exemple".into(),
        postcode: "69001".into(),
        city: "Lyon".into(),
        ids: vec![IdSetting { kind: "siret".into(), value: SIRET.into(), printed: true }],
        tax: vec![TaxSetting { from: String::new(), scheme: scheme.into(), mention: String::new() }],
        rate: 60.0,
        ..InvoiceSettings::default()
    }
}

fn business() -> Client {
    Client {
        kind: "business".into(),
        name: "Studio Lumen".into(),
        address: Address { lines: vec!["1 rue de l’Exemple".into()], postcode: "75002".into(), city: "Paris".into(), country: "FR".into() },
        ids: vec![Identifier { kind: "siren".into(), value: CLIENT_SIREN.into(), printed: true }],
        ..Client::default()
    }
}

fn project() -> Project {
    Project { id: "lumen".into(), title: "Studio Lumen".into(), kind: Some("project".into()), rate: Some(60.0), budget: Some("work".into()), ..Project::default() }
}

/// Time noted for the project, as the Time page notes it.
fn noted(place: &Place, sessions: &[(i64, u32, &str)]) -> Vec<Entry> {
    for (start, minutes, note) in sessions {
        timelog::record_in(&place.time(), &Session { project: "lumen".into(), start: *start, minutes: *minutes, note: note.to_string(), ..Session::default() }).unwrap();
    }
    entries_of(place)
}

fn entries_of(place: &Place) -> Vec<Entry> {
    crate::timereport::entries(&timelog::sessions_in(&place.time()), &[], &[project()], &jiff::tz::TimeZone::UTC)
}

fn steps(place: &Place) -> (PathBuf, PathBuf) {
    std::fs::write(place.ledger(), "").unwrap();
    (place.time(), place.ledger())
}

fn fresh(language: &str) -> Invoice {
    Invoice { language: language.into(), currency: "EUR".into(), client: business(), terms: Terms { days: 30, end_of_month: false }, ..Invoice::default() }
}

fn here() -> Numbering {
    Numbering { device: DESK.into(), ..Numbering::default() }
}

/// A line of `nature` priced `price` cents, its tax placed by `refresh`.
fn sold(description: &str, price: i64, nature: Nature) -> Line {
    Line { quantity: 1000, unit: "C62".into(), price, base: 1, nature, ..Line::of_time(description, 0, 0, "S", 0) }
}

/// Issues a draft made of `draft`, in `place`, on `on`.
fn issued(place: &Place, draft: &Invoice, s: &InvoiceSettings, on: &str) -> Invoice {
    let saved = save_draft(&place.invoices(), draft).unwrap();
    let time = place.time();
    let ledger = place.ledger();
    if !ledger.exists() {
        std::fs::write(&ledger, "").unwrap();
    }
    issue(&place.invoices(), &saved.draft, s, day(on), &here(), &Steps { time: Some(&time), ledger: Some((&ledger, "work")) }, &en()).unwrap()
}

/// The time of a project billed once: a line per note, its time at the
/// project's rate; issued, numbered after the last, its time marked, its
/// budget line planned; the next invoice has nothing to bill.
#[test]
fn an_invoice_bills_each_hour_once() {
    let place = Place::new("hours");
    let s = french("franchise");
    let entries = noted(&place, &[(1_790_000_000, 90, "Build the site"), (1_790_010_000, 25, "Call with the client"), (1_790_020_000, 30, "Build the site")]);
    let draft = from_time(&entries, &project(), fresh("fr"), &[], &s, day("2026-10-05")).unwrap();
    assert_eq!(draft.lines.iter().map(|l| (l.description.as_str(), l.minutes(), l.amount)).collect::<Vec<_>>(), vec![("Build the site", Some(120), 12000), ("Call with the client", Some(25), 2500)]);
    assert_eq!((draft.totals.due, draft.issuer.name.as_str(), draft.lines[0].category.as_str()), (14500, "Camille Exemple EI", "E"));
    let saved = save_draft(&place.invoices(), &draft).unwrap();
    // 2026-001 was given before.
    let (time, ledger) = steps(&place);
    crate::invoice::write_new(&place.invoices(), &Invoice { number: "2026-001".into(), currency: "EUR".into(), ..Invoice::default() }).unwrap();
    let issued = issue(&place.invoices(), &saved.draft, &s, day("2026-10-05"), &here(), &Steps { time: Some(&time), ledger: Some((&ledger, "work")) }, &en()).unwrap();
    assert_eq!((issued.number.as_str(), issued.due.as_str(), issued.series.as_str()), ("2026-002", "2026-11-04", "2026-{N:3}"));
    assert!(timelog::sessions_in(&time).iter().all(|x| x.invoice == "2026-002"), "its time marked");
    assert!(drafts_in(&place.invoices()).is_empty(), "the draft is the invoice now");
    let budget = std::fs::read_to_string(&ledger).unwrap();
    assert!(budget.contains("sioul:invoice/2026-002") && budget.contains("planned = true") && budget.contains("145"), "{budget}");
    let records = all_in(&place.invoices());
    assert!(from_time(&entries_of(&place), &project(), fresh("fr"), &records, &s, day("2026-10-06")).is_none(), "nothing left to bill");
    // The journal is this device's own file, in its own folder.
    assert!(place.invoices().join(series::FOLDER).join(format!("{DESK}.toml")).is_file());
    assert!(!place.invoices().join(series::OLD_JOURNAL).exists());
    // Printed in its own language: « EI », the identifiers, the franchise's mention, the B2B lines.
    let page = html(&issued, &Translator::new(&issued.language));
    for said in ["Camille Exemple EI", "SIRET 732 829 320 00017", "SIREN 904 123 452", "Facture n° 2026-002", "TVA non applicable, article 293 B du CGI", "Escompte pour paiement anticipé : néant.", "12,4\u{202f}%", "40 €", "Prestations de services", "2 h", "60\u{a0}€ / h"] {
        assert!(page.contains(said), "{said}:\n{page}");
    }
}

/// Legal form, income regime and VAT scheme apart: an EI micro-entrepreneur
/// liable to VAT gets « EI », VAT lines per rate and no franchise mention,
/// and its declaration counts what it received excluding VAT; a company
/// under the franchise gets the mention, its capital, and no « EI ».
#[test]
fn form_regime_and_scheme_are_three_settings() {
    let place = Place::new("three");
    let mut s = french("liable");
    s.ids.push(IdSetting { kind: "vat".into(), value: crate::invoice::ids::vat_fr_of(SIREN).unwrap(), printed: true });
    let mut draft = fresh("fr");
    let line = |description: &str, price: i64, rate: i64| Line { quantity: 2000, unit: "DAY".into(), price, base: 1, own_tax: true, ..Line::of_time(description, 0, 0, "S", rate) };
    draft.lines = vec![line("Workshop", 50000, 2000), line("Book", 1000, 550)];
    refresh(&mut draft, &s, day("2026-11-16"));
    assert_eq!(draft.issuer.name, "Camille Exemple EI");
    assert_eq!((draft.totals.without_tax, draft.totals.tax, draft.totals.with_tax), (102_000, 20_110, 122_110));
    assert!(checks(&draft, &s, day("2026-11-16"), &[]).iter().all(|c| !c.required), "{:?}", checks(&draft, &s, day("2026-11-16"), &[]));
    let page = html(&draft, &Translator::new("fr"));
    for said in ["Camille Exemple EI", "TVA 20\u{202f}%", "TVA 5,5\u{202f}%", "Total HT", "Total TTC", "1\u{202f}221,10"] {
        assert!(page.contains(said), "{said}:\n{page}");
    }
    assert!(!page.contains("293 B"), "no franchise under VAT:\n{page}");
    let issued = issued(&place, &draft, &s, "2026-11-16");
    // Half paid in December: its declaration counts what came in, without the VAT.
    add_payment(&place.invoices(), DESK, &issued.number, Payment { date: "2026-12-02".into(), amount: 61_055, means: "transfer".into(), ..Payment::default() }, &en()).unwrap();
    let all = all_in(&place.invoices());
    assert_eq!(received(&all, day("2026-12-01"), day("2026-12-31")).get("service"), Some(&51_000));
    assert_eq!(all[0].status(&all), Status::PartlyPaid);

    let company = InvoiceSettings { legal_form: "sasu".into(), name: "Lumen Conseil".into(), capital: "1 000 €".into(), ids: vec![IdSetting { kind: "siren".into(), value: SIREN.into(), printed: true }, IdSetting { kind: "rcs".into(), value: "Lyon".into(), printed: true }], ..french("franchise") };
    let mut draft = fresh("fr");
    draft.lines = vec![Line::of_time("Advice", 60, 9000, "E", 0)];
    refresh(&mut draft, &company, day("2026-11-16"));
    assert_eq!(draft.issuer.name, "Lumen Conseil", "a company is no EI");
    assert!(checks(&draft, &company, day("2026-11-16"), &[]).iter().all(|c| !c.required), "{:?}", checks(&draft, &company, day("2026-11-16"), &[]));
    let page = html(&draft, &Translator::new("fr"));
    for said in ["TVA non applicable, article 293 B du CGI", "SASU au capital de 1 000 €", "RCS Lyon"] {
        assert!(page.contains(said), "{said}:\n{page}");
    }
    assert!(!page.contains(" EI") && !page.contains("TVA 20"), "{page}");
    // Without its capital, a company's invoice waits.
    let bare = InvoiceSettings { capital: String::new(), ..company };
    assert!(checks(&draft, &bare, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("capital")));
}

/// A table from before November 2026 keeps working: a VAT line citing 293 B
/// makes it France, the micro regime, the franchise with its very words; its
/// legal form is asked once (« EI » not printed meanwhile, a company under
/// the franchise must not read it: the review's F15); any other is
/// "elsewhere", its lines printed as they were.
#[test]
fn an_older_table_keeps_its_words() {
    let older = InvoiceSettings { name: "Studio Exemple SASU".into(), address: "2 place de l’Exemple\n69001 Lyon".into(), siret: SIRET.into(), vat: "TVA non applicable, art. 293 B du CGI".into(), payment: "IBAN FR76 0000 0000".into(), ..InvoiceSettings::default() };
    assert_eq!((preset_of(&older).id.as_str(), legal_form_of(&older).as_str(), regime_of(&older).as_str()), ("fr", "", "micro"));
    assert_eq!(scheme_on(&older, day("2026-11-16")), ("franchise".to_string(), "TVA non applicable, art. 293 B du CGI".to_string()));
    let issuer = issuer_on(&older, day("2026-11-16"));
    assert_eq!((issuer.name.as_str(), issuer.ids[0].kind.as_str()), ("Studio Exemple SASU", "siret"), "no « EI » until the form is said");
    let mut draft = fresh("en");
    draft.lines = vec![Line::of_time("Advice", 60, 9000, "E", 0)];
    refresh(&mut draft, &older, day("2026-11-16"));
    assert!(checks(&draft, &older, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("legal form")));
    let said = InvoiceSettings { legal_form: "ei".into(), name: "Camille Exemple EI".into(), ..older.clone() };
    assert_eq!(issuer_on(&said, day("2026-11-16")).name, "Camille Exemple EI", "not doubled");
    assert!(checks(&draft, &said, day("2026-11-16"), &[]).iter().all(|c| !c.text.contains("legal form")));
    assert_eq!(pattern_of(&older, Kind::Invoice).unwrap(), Pattern::of_prefix(""));
    let elsewhere = InvoiceSettings { name: "Robin Example".into(), vat: "VAT not charged".into(), siret: "12-3456789".into(), prefix: "R-".into(), ..InvoiceSettings::default() };
    assert_eq!((preset_of(&elsewhere).id.as_str(), scheme_on(&elsewhere, day("2026-11-16")).0.as_str()), ("elsewhere", "none"));
    let mut draft = fresh("en");
    draft.lines = vec![Line::of_time("Design", 60, 5000, "O", 0)];
    refresh(&mut draft, &elsewhere, day("2026-11-16"));
    let page = html(&draft, &en());
    for said in ["VAT not charged", "SIRET 12-3456789", "Late payment"] {
        assert!(page.contains(said), "{said}:\n{page}");
    }
    assert_eq!(pattern_of(&elsewhere, Kind::Invoice).unwrap().number(day("2027-01-01"), 42), "R-042");
    // Its late-payment sentence taken out by a line of Settings.
    let quiet = InvoiceSettings { lines: vec![LineSetting { id: "older-late".into(), off: true, ..LineSetting::default() }], ..elsewhere };
    refresh(&mut draft, &quiet, day("2026-11-16"));
    assert!(!html(&draft, &en()).contains("Late payment"));
    assert_eq!(marked_name("EI Camille Exemple", "EI"), "EI Camille Exemple");
    assert_eq!(marked_name("Camille Exemple (EI)", "EI"), "Camille Exemple (EI)");
    assert_eq!(marked_name("Camille Exemple, entrepreneur individuel", "EI"), "Camille Exemple, entrepreneur individuel");
    assert_eq!(marked_name("Leila Eid", "EI"), "Leila Eid EI");
}

/// What the preset asks, said before issue: the issuer's SIREN required, a
/// French business client's recommended now and required from September
/// 2027; a number that fails its key refused, a SIRET being given accepted;
/// a consumer and a public body get no B2B lines; two late-payment rates
/// never; a scheme the country does not offer refused.
#[test]
fn checks_say_what_is_missing() {
    let mut s = french("franchise");
    let mut draft = fresh("en");
    draft.client.ids.clear();
    draft.lines = vec![Line::of_time("Advice", 60, 9000, "E", 0)];
    refresh(&mut draft, &s, day("2026-11-16"));
    let now = checks(&draft, &s, day("2026-11-16"), &[]);
    assert_eq!(now.iter().map(|c| c.required).collect::<Vec<_>>(), [false], "{now:?}");
    assert!(now[0].text.contains("SIREN") && now[0].text.contains("September 2027"), "{now:?}");
    let later = checks(&draft, &s, day("2027-09-01"), &[]);
    assert!(later.iter().any(|c| c.required && c.text.contains("SIREN")), "{later:?}");
    s.ids = vec![IdSetting { kind: "siren".into(), value: "732 829 321".into(), printed: true }];
    assert!(checks(&draft, &s, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("732 829 321")), "a wrong key");
    s.ids.clear();
    assert!(checks(&draft, &s, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("SIREN or SIRET")));
    // A SIRET being given (F28): accepted, and printed so.
    for pending in [IdSetting { kind: "siret".into(), value: "en cours d’attribution".into(), printed: true }, IdSetting { kind: "other".into(), value: "SIRET en cours d’attribution".into(), printed: true }] {
        s.ids = vec![pending];
        refresh(&mut draft, &s, day("2026-11-16"));
        assert!(checks(&draft, &s, day("2026-11-16"), &[]).iter().all(|c| !c.required), "{:?}", checks(&draft, &s, day("2026-11-16"), &[]));
        assert!(html(&draft, &en()).contains("SIRET en cours d’attribution"));
    }
    let s = french("franchise");
    // A consumer: no penalties, no €40, no escompte.
    let mut person = fresh("fr");
    person.client = Client { kind: "consumer".into(), name: "Anna Exemple".into(), ..Client::default() };
    person.lines = vec![Line::of_time("Cours", 60, 4000, "E", 0)];
    refresh(&mut person, &s, day("2026-11-16"));
    assert_eq!(person.penalty_rate, 0);
    let page = html(&person, &Translator::new("fr"));
    assert!(!page.contains("40 €") && !page.contains("Escompte") && page.contains("293 B"), "{page}");
    assert!(checks(&person, &s, day("2026-11-16"), &[]).iter().all(|c| !c.required), "a consumer's address is theirs to give");
    // A public body (F29): the late-payment lines of L441-10 are not printed for it.
    let mut public = fresh("fr");
    public.client.kind = "public".into();
    public.lines = vec![Line::of_time("Atelier", 60, 4000, "E", 0)];
    refresh(&mut public, &s, day("2026-11-16"));
    assert!(public.notes.iter().all(|n| !matches!(n.legal.as_str(), "penalties" | "indemnity" | "escompte")), "{:?}", public.notes);
    // Two late-payment rates never (F27): payment words that speak of them, and Sioul's lines.
    let typed = InvoiceSettings { payment: "IBAN FR76 0000 — Pénalités de retard : trois fois le taux d’intérêt légal ; indemnité de 40 €".into(), ..s.clone() };
    refresh(&mut draft, &typed, day("2026-11-16"));
    assert!(checks(&draft, &typed, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("late fees")));
    let one = InvoiceSettings { lines: vec![LineSetting { id: "penalties".into(), off: true, ..LineSetting::default() }, LineSetting { id: "indemnity".into(), off: true, ..LineSetting::default() }], ..typed };
    refresh(&mut draft, &one, day("2026-11-16"));
    assert!(checks(&draft, &one, day("2026-11-16"), &[]).iter().all(|c| !c.text.contains("late fees")));
    // A scheme France does not offer (F29).
    let none = french("none");
    refresh(&mut draft, &none, day("2026-11-16"));
    assert!(checks(&draft, &none, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("“none”")));
}

/// Where a line's VAT is due (the review's F4, F5, F6, F14): a service to a
/// business in another member state is reverse-charged, whatever the scheme,
/// with both VAT numbers, the client's checked by its own country; liable,
/// goods to one are an intra-EU delivery; a service to a business outside
/// the EU is outside the scope; an export of goods asks for its words; a
/// business whose country is unknown is asked for it; a German small
/// business reverse-charging asks both VAT numbers.
#[test]
fn a_line_s_vat_follows_where_it_is_due() {
    let mut s = french("franchise");
    s.ids.push(IdSetting { kind: "vat".into(), value: crate::invoice::ids::vat_fr_of(SIREN).unwrap(), printed: true });
    let belgian = |vat: &str| Client {
        kind: "business".into(),
        name: "Atelier Bruxelles".into(),
        address: Address { lines: vec!["Rue de l’Exemple 1".into()], postcode: "1000".into(), city: "Bruxelles".into(), country: "BE".into() },
        ids: if vat.is_empty() { Vec::new() } else { vec![Identifier { kind: "vat".into(), value: vat.into(), printed: true }] },
        ..Client::default()
    };
    let mut abroad = fresh("en");
    abroad.client = belgian("BE0417497106");
    abroad.lines = vec![Line::of_time("Advice", 60, 9000, "E", 0)];
    refresh(&mut abroad, &s, day("2026-11-16"));
    assert_eq!((abroad.lines[0].category.as_str(), abroad.lines[0].reason_code.as_str()), ("AE", "VATEX-EU-AE"));
    assert!(checks(&abroad, &s, day("2026-11-16"), &[]).iter().all(|c| !c.required), "a Belgian number is checked as Belgian: {:?}", checks(&abroad, &s, day("2026-11-16"), &[]));
    let page = html(&abroad, &en());
    assert!(page.contains("Reverse charge") && page.contains("BE0417497106") && !page.contains("293 B"), "{page}");
    abroad.client = belgian("");
    s.ids.retain(|i| i.kind != "vat");
    refresh(&mut abroad, &s, day("2026-11-16"));
    let said = checks(&abroad, &s, day("2026-11-16"), &[]);
    assert_eq!(said.iter().filter(|c| c.required && c.text.contains("VAT number")).count(), 2, "{said:?}");
    // Liable: goods to an EU business, an intra-EU delivery with its words; a service to a US business, outside the scope.
    let mut liable = french("liable");
    liable.ids.push(IdSetting { kind: "vat".into(), value: crate::invoice::ids::vat_fr_of(SIREN).unwrap(), printed: true });
    let mut goods = fresh("fr");
    goods.client = belgian("BE0417497106");
    goods.lines = vec![sold("Tirage d’art", 30_000, Nature::Goods)];
    refresh(&mut goods, &liable, day("2026-11-16"));
    assert_eq!((goods.lines[0].category.as_str(), goods.totals.tax), ("K", 0));
    assert!(html(&goods, &Translator::new("fr")).contains("Exonération, art. 262 ter I du CGI"));
    let mut us = fresh("en");
    us.client = Client { kind: "business".into(), name: "Example Inc.".into(), address: Address { lines: vec!["1 Example Road".into()], city: "Springfield".into(), country: "US".into(), ..Address::default() }, ..Client::default() };
    us.lines = vec![Line::of_time("Development", 600, 9000, "S", 2000)];
    refresh(&mut us, &liable, day("2026-11-16"));
    assert_eq!((us.lines[0].category.as_str(), us.totals.tax), ("O", 0), "no French VAT on a service to a US business");
    assert!(html(&us, &en()).contains("259-1"));
    // An export of goods: no words in Sioul's sources, a required check says so until the line says its own.
    us.lines = vec![sold("Print", 30_000, Nature::Goods)];
    refresh(&mut us, &liable, day("2026-11-16"));
    assert_eq!(us.lines[0].category, "G");
    assert!(checks(&us, &liable, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("export")));
    us.lines[0].exemption = "Exonération des exportations".into();
    us.lines[0].own_tax = true;
    refresh(&mut us, &liable, day("2026-11-16"));
    assert!(checks(&us, &liable, day("2026-11-16"), &[]).iter().all(|c| !c.text.contains("export")), "the line's own words");
    // A consumer abroad keeps the domestic rate.
    us.client.kind = "consumer".into();
    us.lines = vec![Line::of_time("Course", 60, 4000, "S", 0)];
    refresh(&mut us, &liable, day("2026-11-16"));
    assert_eq!((us.lines[0].category.as_str(), us.lines[0].rate), ("S", 2000));
    // A business with no country and no address: asked for.
    let mut nowhere = fresh("en");
    nowhere.client = Client { kind: "business".into(), name: "Somewhere Ltd".into(), ..Client::default() };
    nowhere.lines = vec![Line::of_time("Advice", 60, 9000, "E", 0)];
    refresh(&mut nowhere, &s, day("2026-11-16"));
    assert!(checks(&nowhere, &s, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("country")));
    // Germany, small business: a service to a French business, both VAT numbers asked (F14).
    let klein = InvoiceSettings { preset: "de".into(), name: "Robin Beispiel".into(), address: "Beispielweg 1".into(), postcode: "10115".into(), city: "Berlin".into(), ids: vec![IdSetting { kind: "steuernummer".into(), value: "12/345/67890".into(), printed: true }], tax: vec![TaxSetting { from: String::new(), scheme: "franchise".into(), mention: String::new() }], ..InvoiceSettings::default() };
    let mut german = fresh("en");
    german.client.ids.clear();
    german.lines = vec![Line::of_time("Beratung", 60, 9000, "E", 0)];
    refresh(&mut german, &klein, day("2026-11-16"));
    assert_eq!(german.lines[0].category, "AE");
    assert_eq!(checks(&german, &klein, day("2026-11-16"), &[]).iter().filter(|c| c.required && c.text.contains("VAT number")).count(), 1, "the client's");
    assert!(checks(&german, &klein, day("2026-11-16"), &[]).iter().any(|c| c.required && c.text.contains("USt-IdNr.")), "and the issuer's");
}

/// Issuing is all or nothing: a budget file that cannot be written undoes
/// the record, the journal and the time marked; sharing on with the drafts
/// part off refuses; time billed meanwhile is never billed again; a draft
/// id is its file's name, never a path (F20).
#[test]
fn issuing_is_all_or_nothing() {
    let place = Place::new("atomic");
    let s = french("franchise");
    let entries = noted(&place, &[(1_790_000_000, 60, "Build the site")]);
    let draft = save_draft(&place.invoices(), &from_time(&entries, &project(), fresh("en"), &[], &s, day("2026-10-05")).unwrap()).unwrap();
    let time = place.time();
    let tr = en();
    let broken = place.root.join("notes").join("missing").join("budget.toml");
    let failed = issue(&place.invoices(), &draft.draft, &s, day("2026-10-05"), &here(), &Steps { time: Some(&time), ledger: Some((&broken, "work")) }, &tr);
    assert!(failed.is_err());
    assert!(all_in(&place.invoices()).is_empty(), "no record");
    assert_eq!(series::Journal::load(&place.invoices()), Journal::default(), "the journal as before");
    assert!(timelog::sessions_in(&time).iter().all(|x| x.invoice.is_empty()), "its time not marked");
    assert!(draft_in(&place.invoices(), &draft.draft).is_some(), "the draft kept");
    // Sharing on, invoices not shared from here: refused, in words.
    let off = Numbering { device: DESK.into(), sharing: true, drafts_shared: false, claimed: BTreeMap::new() };
    let refused = issue(&place.invoices(), &draft.draft, &s, day("2026-10-05"), &off, &Steps { time: Some(&time), ledger: None }, &tr).unwrap_err();
    assert!(refused.contains("Drafts and invoices"), "{refused}");
    // Another device numbered up to 2026-007: the claim counts.
    let claimed = Numbering { device: DESK.into(), sharing: true, drafts_shared: true, claimed: BTreeMap::from([("2026-{N:3}".to_string(), 7u64)]) };
    let issued = issue(&place.invoices(), &draft.draft, &s, day("2026-10-05"), &claimed, &Steps { time: Some(&time), ledger: None }, &tr).unwrap();
    assert_eq!(issued.number, "2026-008");
    assert_eq!(series::Journal::load(&place.invoices()).series["2026-{N:3}"].last, 8);
    // A second draft of the same time, made before: refused, its time billed.
    let again = save_draft(&place.invoices(), &from_time(&entries, &project(), fresh("en"), &[], &s, day("2026-10-05")).unwrap()).unwrap();
    let twice = issue(&place.invoices(), &again.draft, &s, day("2026-10-06"), &here(), &Steps { time: Some(&time), ledger: None }, &tr).unwrap_err();
    assert!(twice.contains("2026-008"), "{twice}");
    // A record deleted by hand: its number is not given again; the gap is said.
    crate::invoice::remove_issued(&place.invoices(), "2026-008").unwrap();
    assert_eq!(series::missing(&series::Journal::load(&place.invoices()), &all_in(&place.invoices())).last().and_then(|m| m.number.clone()).as_deref(), Some("2026-008"));
    // A draft's id is its file's name: a path written inside a file is never one (F20).
    let drafts = place.invoices().join(crate::invoice::DRAFTS);
    std::fs::write(drafts.join("odd name.toml"), std::fs::read_to_string(drafts.join(format!("{}.toml", again.draft))).unwrap().replace(&format!("draft = \"{}\"", again.draft), "draft = \"../2026-001\"")).unwrap();
    assert!(drafts_in(&place.invoices()).iter().all(|d| d.draft != "../2026-001" && d.draft != "odd name"));
    assert!(remove_draft(&place.invoices(), "../2026-001").is_err() && draft_in(&place.invoices(), "../x").is_none());
}

/// The time billed is what the issued records say (F17): a crash between a
/// record and its marks never lets the same time be billed again; the marks
/// are made again from the records before each issue.
#[test]
fn billed_time_comes_from_the_records() {
    let place = Place::new("billed");
    let s = french("franchise");
    let entries = noted(&place, &[(1_790_000_000, 60, "Build the site")]);
    let invoice = issued(&place, &from_time(&entries, &project(), fresh("en"), &[], &s, day("2026-10-05")).unwrap(), &s, "2026-10-05");
    // The marks lost (a crash right after the record): the records still bill that time.
    timelog::update_in(&place.time(), &invoice.sessions(), |x| x.invoice.clear()).unwrap();
    let records = all_in(&place.invoices());
    assert!(from_time(&entries_of(&place), &project(), fresh("en"), &records, &s, day("2026-10-06")).is_none(), "nothing left to bill");
    assert_eq!(reconcile_marks(&records, &place.time()).unwrap(), 1);
    assert!(timelog::sessions_in(&place.time()).iter().all(|x| x.invoice == invoice.number), "the marks made again");
}

/// A credit note cites the invoice; it is never paid nor unpaid (F10);
/// issued, it settles what it credits, frees the time it cancels, lowers
/// the invoice's planned budget line (F26); never more than is left to
/// credit (F11); its VAT is the credited lines' (F16).
#[test]
fn a_credit_note_cancels_and_frees_the_time() {
    let place = Place::new("credit");
    let s = french("franchise");
    let entries = noted(&place, &[(1_790_000_000, 60, "Build the site"), (1_790_010_000, 30, "Call")]);
    let invoice = issued(&place, &from_time(&entries, &project(), fresh("en"), &[], &s, day("2026-10-05")).unwrap(), &s, "2026-10-05");
    assert!(std::fs::read_to_string(place.ledger()).unwrap().contains("amount = 90.0"));
    // The call's line only: a correction; its time freed, the rest stays billed.
    let some = credit(&invoice, Some(&[1]), &s, day("2026-10-07"));
    assert_eq!((some.kind, some.corrects.as_ref().map(|c| c.number.as_str())), (Kind::CreditNote, Some("2026-001")));
    let note = issued(&place, &some, &s, "2026-10-07");
    assert_eq!((note.number.as_str(), note.totals.due), ("2026-002", 3000), "in the invoices' series");
    let marks: Vec<String> = timelog::sessions_in(&place.time()).into_iter().map(|x| x.invoice).collect();
    assert_eq!(marks, ["2026-001", ""]);
    assert!(std::fs::read_to_string(place.ledger()).unwrap().contains("amount = 60.0"), "the invoice's money expected lowered by the credit");
    let all = all_in(&place.invoices());
    let kept = all.iter().find(|i| i.number == "2026-001").unwrap();
    assert_eq!((kept.status(&all), kept.left(&all)), (Status::Issued, 6000), "partly credited, not partly paid");
    let credit_note = all.iter().find(|i| i.number == "2026-002").unwrap();
    assert_eq!(credit_note.status(&all), Status::CreditNote);
    assert!(pay_rest(&place.invoices(), DESK, "2026-002", day("2026-10-08"), "", &en()).is_err(), "no Paid on a credit note");
    let page = html(&note, &en());
    assert!(page.contains("Credit note No. 2026-002") && page.contains("2026-001") && !page.contains("Late payment"), "{page}");
    // More than is left to credit: refused, the credit notes named.
    let all_again = credit(kept, None, &s, day("2026-10-08"));
    let said = checks(&all_again, &s, day("2026-10-08"), &all);
    assert!(said.iter().any(|c| c.required && c.text.contains("2026-002")), "{said:?}");
    // The rest credited: the invoice is credited, all its time free, its budget line gone.
    issued(&place, &credit(kept, Some(&[0]), &s, day("2026-10-08")), &s, "2026-10-08");
    let all = all_in(&place.invoices());
    assert_eq!(all.iter().find(|i| i.number == "2026-001").unwrap().status(&all), Status::Credited);
    assert!(timelog::sessions_in(&place.time()).iter().all(|x| x.invoice.is_empty()));
    assert!(!std::fs::read_to_string(place.ledger()).unwrap().contains("sioul:invoice/2026-001"), "nothing expected any more");
    // Liable for VAT since: a credit note of a franchise invoice still carries no VAT, and its mention.
    let liable = InvoiceSettings { tax: vec![TaxSetting { from: "2026-10-06".into(), scheme: "liable".into(), mention: String::new() }, TaxSetting { from: String::new(), scheme: "franchise".into(), mention: String::new() }], ..s.clone() };
    let late = credit(&invoice, Some(&[0]), &liable, day("2026-10-08"));
    assert_eq!((late.totals.tax, late.lines[0].category.as_str()), (0, "E"));
    assert!(html(&late, &en()).contains("293 B"), "{}", html(&late, &en()));
    // A draft changes freely: a line added, the totals made again; it stays a draft.
    let id = save_draft(&place.invoices(), &late).unwrap().draft;
    let changed = change_draft(&place.invoices(), &id, &liable, day("2026-10-08"), |d| d.lines.push(Line { own_tax: true, ..Line::of_time("Travel", 30, 6000, "E", 0) })).unwrap();
    assert_eq!((changed.lines.len(), changed.totals.due, changed.number.as_str()), (2, 9000, ""));
}

/// A deposit invoice, then the final invoice deducting it; payments in each
/// device's own life file, never in the record (decision 1 of the review);
/// never more than is left to pay.
#[test]
fn deposits_and_payments() {
    let place = Place::new("deposit");
    let s = french("franchise");
    let tr = en();
    let first = issued(&place, &deposit(&s, fresh("en"), "Deposit, 30 % of the site", 90_000, day("2026-09-01")), &s, "2026-09-01");
    assert_eq!((first.kind, first.totals.due), (Kind::Deposit, 90_000));
    assert!(html(&first, &tr).contains("Deposit invoice No. 2026-001"));
    let mut last = fresh("en");
    last.lines = vec![Line { quantity: 1000, unit: "LS".into(), price: 300_000, base: 1, ..Line::of_time("The site", 0, 0, "E", 0) }];
    cite_deposits(&mut last, std::slice::from_ref(&first), &s, day("2026-10-01"));
    assert_eq!((last.totals.with_tax, last.totals.deposits, last.totals.due), (300_000, 90_000, 210_000));
    let final_invoice = issued(&place, &last, &s, "2026-10-01");
    assert!(html(&final_invoice, &tr).contains("Deposits invoiced (2026-001)"));
    let record = std::fs::read_to_string(place.invoices().join("2026-002.toml")).unwrap();
    // Paid in two steps, by hand: the record never changes.
    add_payment(&place.invoices(), DESK, "2026-002", Payment { date: "2026-10-10".into(), amount: 100_000, means: "transfer".into(), ..Payment::default() }, &tr).unwrap();
    assert!(add_payment(&place.invoices(), "phone-2", "2026-002", Payment { date: "2026-10-11".into(), amount: 200_000, means: "transfer".into(), ..Payment::default() }, &tr).is_err(), "more than is left");
    let rest = pay_rest(&place.invoices(), "phone-2", "2026-002", day("2026-10-20"), "", &tr).unwrap().unwrap();
    assert_eq!((rest.amount, rest.means.as_str()), (110_000, "transfer"));
    assert_eq!(std::fs::read_to_string(place.invoices().join("2026-002.toml")).unwrap(), record, "the record as issued");
    let all = all_in(&place.invoices());
    assert_eq!(find_in(&place.invoices(), "2026-002").unwrap().status(&all), Status::Paid);
    // The phone's payment taken back on the desk: a removal in the desk's own file.
    take_back(&place.invoices(), DESK, std::slice::from_ref(&rest.id)).unwrap();
    let all = all_in(&place.invoices());
    assert_eq!(find_in(&place.invoices(), "2026-002").unwrap().status(&all), Status::PartlyPaid);
    unpay(&place.invoices(), DESK, "2026-002", &tr).unwrap();
    assert_eq!(find_in(&place.invoices(), "2026-002").unwrap().payments.len(), 0);
}

/// The bank's lines (F1, F2, F9): a payment said by hand takes in the bank
/// line as its evidence, never a second payment; one transfer for two
/// invoices pays each, the oldest first, the rest said; a line in another
/// currency writes nothing.
#[test]
fn a_bank_line_pays_what_is_left() {
    let place = Place::new("bank");
    let s = french("franchise");
    let tr = en();
    let mut first = fresh("en");
    first.lines = vec![sold("Logo", 50_000, Nature::Service)];
    let a = issued(&place, &first, &s, "2026-10-01");
    let mut second = fresh("en");
    second.lines = vec![sold("Site", 70_000, Nature::Service)];
    let b = issued(&place, &second, &s, "2026-10-02");
    // Paid by hand, then the bank's line comes: its evidence, no second payment.
    let hand = pay_rest(&place.invoices(), DESK, &a.number, day("2026-10-20"), "transfer", &tr).unwrap().unwrap();
    let taken = take_bank_line(&place.invoices(), DESK, std::slice::from_ref(&a.number), "sioul:movement/m1", day("2026-10-21"), 50_000, "EUR").unwrap();
    assert_eq!((taken.entries.len(), taken.overpaid), (1, 0));
    let paid = find_in(&place.invoices(), &a.number).unwrap().payments;
    assert_eq!((paid.len(), paid[0].id.as_str(), paid[0].movement.as_str(), paid[0].date.as_str()), (1, hand.id.as_str(), "sioul:movement/m1", "2026-10-21"));
    // The evidence's Undo: the hand payment is said by hand again.
    take_back(&place.invoices(), DESK, &taken.ids()).unwrap();
    assert_eq!(find_in(&place.invoices(), &a.number).unwrap().payments[0].movement, "");
    unpay(&place.invoices(), DESK, &a.number, &tr).unwrap();
    // One transfer of €1,250 for both: each paid, the oldest first, €50 beyond.
    let both = take_bank_line(&place.invoices(), DESK, &[b.number.clone(), a.number.clone()], "sioul:movement/m2", day("2026-11-02"), 125_000, "EUR").unwrap();
    assert_eq!((both.entries.iter().map(|(n, _, amount)| (n.as_str(), *amount)).collect::<Vec<_>>(), both.overpaid), (vec![(a.number.as_str(), 50_000), (b.number.as_str(), 70_000)], 5_000));
    let all = all_in(&place.invoices());
    assert!(all.iter().all(|i| i.status(&all) == Status::Paid));
    // Again with the same line: nothing more.
    assert_eq!(take_bank_line(&place.invoices(), DESK, std::slice::from_ref(&a.number), "sioul:movement/m2", day("2026-11-02"), 50_000, "EUR").unwrap().entries.len(), 0);
    // A line in dollars on euro invoices: nothing written.
    take_back(&place.invoices(), DESK, &both.ids()).unwrap();
    let usd = take_bank_line(&place.invoices(), DESK, std::slice::from_ref(&a.number), "sioul:movement/m3", day("2026-11-03"), 50_000, "USD").unwrap();
    assert!(usd.entries.is_empty() && usd.overpaid == 50_000);
}

/// A draft's service day (F24): the day of issue when nothing says it,
/// shown on the draft and printed on the invoice.
#[test]
fn the_service_day_is_the_issue_s_by_default() {
    let place = Place::new("service-day");
    let s = french("franchise");
    let mut draft = fresh("en");
    draft.lines = vec![sold("Workshop", 90_000, Nature::Service)];
    refresh(&mut draft, &s, day("2026-11-16"));
    assert_eq!((draft.period_from.as_str(), draft.period_default), ("2026-11-16", true));
    let invoice = issued(&place, &draft, &s, "2026-11-20");
    assert_eq!((invoice.period_from.as_str(), invoice.period_default), ("2026-11-20", false), "the issue's day");
    assert!(html(&invoice, &en()).contains("Service on 20 November 2026"));
}

/// Every supported preset, under each of its schemes, for a business
/// client: what it requires, filled, is printed (identifiers, the
/// franchise's mention, the legal lines), and nothing required is missing.
#[test]
fn each_preset_prints_what_it_requires() {
    for preset in presets::all().iter().filter(|p| p.supported) {
        for scheme in &preset.tax.schemes {
            for form in preset.legal_form.iter().map(|f| f.id.clone()).chain(std::iter::once(String::new())).take(2) {
                // A value for each identifier a rule asks, that passes its check.
                let value = |kind: &str| match preset.id_kind(kind).map(|k| k.check.as_str()) {
                    Some("siren") => SIREN.to_string(),
                    Some("siret") => SIRET.to_string(),
                    Some("vat") | Some("vat-fr") => crate::invoice::ids::vat_fr_of(SIREN).unwrap(),
                    _ => format!("{}-12345", kind.to_uppercase()),
                };
                let mut kinds: Vec<String> = preset.require.iter().filter(|r| r.who == Who::Issuer).flat_map(|r| r.any.iter().cloned()).collect();
                kinds.sort();
                kinds.dedup();
                let s = InvoiceSettings {
                    preset: preset.id.clone(),
                    legal_form: form.clone(),
                    name: "Robin Example".into(),
                    address: "1 Example Street".into(),
                    postcode: "1000".into(),
                    city: "Example".into(),
                    email: "robin@example.org".into(),
                    capital: "1 000".into(),
                    ids: kinds.iter().map(|k| IdSetting { kind: k.clone(), value: value(k), printed: true }).collect(),
                    tax: vec![TaxSetting { from: String::new(), scheme: scheme.clone(), mention: if preset.mention_on(day("2026-11-16")).is_empty() { "Franchise mention of the person's own".into() } else { String::new() } }],
                    currency: preset.currency.clone(),
                    ..InvoiceSettings::default()
                };
                let mut client = business();
                client.address.country = preset.country.clone();
                client.ids = preset
                    .require
                    .iter()
                    .filter(|r| r.who == Who::Client)
                    .flat_map(|r| r.any.iter())
                    .map(|k| Identifier {
                        kind: k.clone(),
                        value: match k.as_str() {
                            "siren" => CLIENT_SIREN.into(),
                            "siret" => CLIENT_SIRET.into(),
                            _ => value(k),
                        },
                        printed: true,
                    })
                    .collect();
                let mut draft = Invoice { language: "fr".into(), currency: currency_of(&s, ""), client, terms: terms_of(&s, None, None), ..Invoice::default() };
                draft.lines = vec![Line::of_time("Advice", 60, 9000, "S", 0)];
                refresh(&mut draft, &s, day("2026-11-16"));
                let said = checks(&draft, &s, day("2026-11-16"), &[]);
                assert!(said.iter().all(|c| !c.required), "{} {scheme} {form}: {said:?}", preset.id);
                let page = html(&draft, &Translator::new("fr"));
                for id in &s.ids {
                    assert!(page.contains(&id.value), "{} {scheme}: {} printed:\n{page}", preset.id, id.kind);
                }
                if scheme == "franchise" {
                    assert!(page.contains(&issuer_on(&s, day("2026-11-16")).mention), "{}: the franchise's mention:\n{page}", preset.id);
                }
                if scheme == "liable" && preset.id != presets::ELSEWHERE {
                    assert!(draft.totals.tax > 0, "{}: VAT", preset.id);
                }
                for note in &draft.notes {
                    assert!(page.contains(&note.text.replace('\'', "&#39;").replace('"', "&quot;")) || page.contains(&note.text), "{}: {}", preset.id, note.text);
                }
            }
        }
    }
}

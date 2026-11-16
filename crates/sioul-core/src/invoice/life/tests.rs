// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The life of invoices, on fictional businesses of example.org; their SIREN,
//! SIRET and VAT numbers are made up and pass their checks.

use super::*;
use crate::config::{IdSetting, LineSetting, TaxSetting};
use crate::invoice::record::Status;
use crate::invoice::{all_in, draft_in, drafts_in, find_in, html, save_draft};
use crate::timelog::{self, Session};
use std::path::PathBuf;

const SIREN: &str = "732 829 320";
const SIRET: &str = "732 829 320 00017";
const CLIENT_SIREN: &str = "904 123 452";
const CLIENT_SIRET: &str = "904 123 452 00008";

fn day(text: &str) -> Date {
    text.parse().unwrap()
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

/// A French EI micro-entrepreneur under the franchise, as Settings says it now.
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
    crate::timereport::entries(&timelog::sessions_in(&place.time()), &[], &[project()], &jiff::tz::TimeZone::UTC)
}

fn steps(place: &Place) -> (PathBuf, PathBuf) {
    std::fs::write(place.ledger(), "").unwrap();
    (place.time(), place.ledger())
}

fn fresh(language: &str) -> Invoice {
    Invoice { language: language.into(), currency: "EUR".into(), client: business(), terms: Terms { days: 30, end_of_month: false }, ..Invoice::default() }
}

/// The time of a project billed once: a line per note, its time at the
/// project's rate; issued, numbered after the last, its time marked, its
/// budget line planned; the next invoice has nothing to bill.
#[test]
fn an_invoice_bills_each_hour_once() {
    let place = Place::new("hours");
    let s = french("franchise");
    let entries = noted(&place, &[(1_790_000_000, 90, "Build the site"), (1_790_010_000, 25, "Call with the client"), (1_790_020_000, 30, "Build the site")]);
    let draft = from_time(&entries, &project(), fresh("fr"), &s, day("2026-10-05")).unwrap();
    assert_eq!(draft.lines.iter().map(|l| (l.description.as_str(), l.minutes(), l.amount)).collect::<Vec<_>>(), vec![("Build the site", Some(120), 12000), ("Call with the client", Some(25), 2500)]);
    assert_eq!((draft.totals.due, draft.issuer.name.as_str(), draft.lines[0].category.as_str()), (14500, "Camille Exemple EI", "E"));
    let saved = save_draft(&place.invoices(), &draft).unwrap();
    // 2026-001 was given before.
    let (time, ledger) = steps(&place);
    crate::invoice::write_new(&place.invoices(), &Invoice { number: "2026-001".into(), currency: "EUR".into(), ..Invoice::default() }).unwrap();
    let issued = issue(&place.invoices(), &saved.draft, &s, day("2026-10-05"), &Numbering::default(), None, &Steps { time: Some(&time), ledger: Some((&ledger, "work")) }, &Translator::new("en")).unwrap();
    assert_eq!((issued.number.as_str(), issued.due.as_str(), issued.series.as_str()), ("2026-002", "2026-11-04", "2026-{N:3}"));
    assert!(timelog::sessions_in(&time).iter().all(|x| x.invoice == "2026-002"), "its time marked");
    assert!(drafts_in(&place.invoices()).is_empty(), "the draft is the invoice now");
    let budget = std::fs::read_to_string(&ledger).unwrap();
    assert!(budget.contains("sioul:invoice/2026-002") && budget.contains("planned = true") && budget.contains("145"), "{budget}");
    let entries = crate::timereport::entries(&timelog::sessions_in(&time), &[], &[project()], &jiff::tz::TimeZone::UTC);
    assert!(from_time(&entries, &project(), fresh("fr"), &s, day("2026-10-06")).is_none(), "nothing left to bill");
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
    let line = |description: &str, price: i64, rate: i64| Line { quantity: 2000, unit: "DAY".into(), price, base: 1, category: "S".into(), rate, ..Line::of_time(description, 0, 0, "S", rate) };
    draft.lines = vec![line("Workshop", 50000, 2000), line("Book", 1000, 550)];
    refresh(&mut draft, &s, day("2026-11-16"));
    assert_eq!(draft.issuer.name, "Camille Exemple EI");
    assert_eq!((draft.totals.without_tax, draft.totals.tax, draft.totals.with_tax), (102_000, 20_110, 122_110));
    assert!(checks(&draft, &s, day("2026-11-16")).iter().all(|c| !c.required), "{:?}", checks(&draft, &s, day("2026-11-16")));
    let page = html(&draft, &Translator::new("fr"));
    for said in ["Camille Exemple EI", "TVA 20\u{202f}%", "TVA 5,5\u{202f}%", "Total HT", "Total TTC", "1\u{202f}221,10"] {
        assert!(page.contains(said), "{said}:\n{page}");
    }
    assert!(!page.contains("293 B"), "no franchise under VAT:\n{page}");
    let saved = save_draft(&place.invoices(), &draft).unwrap();
    let (time, _) = steps(&place);
    let issued = issue(&place.invoices(), &saved.draft, &s, day("2026-11-16"), &Numbering::default(), None, &Steps { time: Some(&time), ledger: None }, &Translator::new("en")).unwrap();
    // Half paid in December: its declaration counts what came in, without the VAT.
    add_payment(&place.invoices(), &issued.number, Payment { date: "2026-12-02".into(), amount: 61_055, means: "transfer".into(), movement: String::new() }).unwrap();
    let all = all_in(&place.invoices());
    assert_eq!(received(&all, day("2026-12-01"), day("2026-12-31")).get("service"), Some(&51_000));
    assert_eq!(all[0].status(&all), Status::PartlyPaid);

    let company = InvoiceSettings { legal_form: "sasu".into(), name: "Lumen Conseil".into(), capital: "1 000 €".into(), ids: vec![IdSetting { kind: "siren".into(), value: SIREN.into(), printed: true }, IdSetting { kind: "rcs".into(), value: "Lyon".into(), printed: true }], ..french("franchise") };
    let mut draft = fresh("fr");
    draft.lines = vec![Line::of_time("Advice", 60, 9000, "E", 0)];
    refresh(&mut draft, &company, day("2026-11-16"));
    assert_eq!(draft.issuer.name, "Lumen Conseil", "a company is no EI");
    assert!(checks(&draft, &company, day("2026-11-16")).iter().all(|c| !c.required), "{:?}", checks(&draft, &company, day("2026-11-16")));
    let page = html(&draft, &Translator::new("fr"));
    for said in ["TVA non applicable, article 293 B du CGI", "SASU au capital de 1 000 €", "RCS Lyon"] {
        assert!(page.contains(said), "{said}:\n{page}");
    }
    assert!(!page.contains(" EI") && !page.contains("TVA 20"), "{page}");
    // Without its capital, a company's invoice waits.
    let bare = InvoiceSettings { capital: String::new(), ..company };
    assert!(checks(&draft, &bare, day("2026-11-16")).iter().any(|c| c.required && c.text.contains("capital")));
}

/// A table from before November 2026 keeps working as it was: a VAT line
/// citing 293 B makes it France, an EI, the micro regime, the franchise with
/// its very words (« EI » not doubled); any other is "elsewhere", its lines
/// printed as they were.
#[test]
fn an_older_table_keeps_its_words() {
    let older = InvoiceSettings { name: "Camille Exemple EI".into(), address: "2 place de l’Exemple\n69001 Lyon".into(), siret: SIRET.into(), vat: "TVA non applicable, art. 293 B du CGI".into(), payment: "IBAN FR76 0000 0000".into(), ..InvoiceSettings::default() };
    assert_eq!((preset_of(&older).id.as_str(), legal_form_of(&older).as_str(), regime_of(&older).as_str()), ("fr", "ei", "micro"));
    assert_eq!(scheme_on(&older, day("2026-11-16")), ("franchise".to_string(), "TVA non applicable, art. 293 B du CGI".to_string()));
    let issuer = issuer_on(&older, day("2026-11-16"));
    assert_eq!((issuer.name.as_str(), issuer.ids[0].kind.as_str()), ("Camille Exemple EI", "siret"));
    assert_eq!(pattern_of(&older, Kind::Invoice).unwrap(), Pattern::of_prefix(""));
    let elsewhere = InvoiceSettings { name: "Robin Example".into(), vat: "VAT not charged".into(), siret: "12-3456789".into(), prefix: "R-".into(), ..InvoiceSettings::default() };
    assert_eq!((preset_of(&elsewhere).id.as_str(), scheme_on(&elsewhere, day("2026-11-16")).0.as_str()), ("elsewhere", "none"));
    let mut draft = fresh("en");
    draft.lines = vec![Line::of_time("Design", 60, 5000, "O", 0)];
    refresh(&mut draft, &elsewhere, day("2026-11-16"));
    let page = html(&draft, &Translator::new("en"));
    for said in ["VAT not charged", "SIRET 12-3456789", "Late payment"] {
        assert!(page.contains(said), "{said}:\n{page}");
    }
    assert_eq!(pattern_of(&elsewhere, Kind::Invoice).unwrap().number(day("2027-01-01"), 42), "R-042");
    // Its late-payment sentence taken out by a line of Settings.
    let quiet = InvoiceSettings { lines: vec![LineSetting { id: "older-late".into(), off: true, ..LineSetting::default() }], ..elsewhere };
    refresh(&mut draft, &quiet, day("2026-11-16"));
    assert!(!html(&draft, &Translator::new("en")).contains("Late payment"));
    assert_eq!(marked_name("EI Camille Exemple", "EI"), "EI Camille Exemple");
    assert_eq!(marked_name("Camille Exemple (EI)", "EI"), "Camille Exemple (EI)");
    assert_eq!(marked_name("Camille Exemple, entrepreneur individuel", "EI"), "Camille Exemple, entrepreneur individuel");
    assert_eq!(marked_name("Leila Eid", "EI"), "Leila Eid EI");
}

/// What the preset asks, said before issue: the issuer's SIREN required, a
/// French business client's recommended now and required from September
/// 2027; a number that fails its key refused; a consumer gets no B2B lines.
#[test]
fn checks_say_what_is_missing() {
    let mut s = french("franchise");
    let mut draft = fresh("en");
    draft.client.ids.clear();
    draft.lines = vec![Line::of_time("Advice", 60, 9000, "E", 0)];
    refresh(&mut draft, &s, day("2026-11-16"));
    let now = checks(&draft, &s, day("2026-11-16"));
    assert_eq!(now.iter().map(|c| c.required).collect::<Vec<_>>(), [false], "{now:?}");
    assert!(now[0].text.contains("SIREN") && now[0].text.contains("September 2027"), "{now:?}");
    let later = checks(&draft, &s, day("2027-09-01"));
    assert!(later.iter().any(|c| c.required && c.text.contains("SIREN")), "{later:?}");
    s.ids = vec![IdSetting { kind: "siren".into(), value: "732 829 321".into(), printed: true }];
    assert!(checks(&draft, &s, day("2026-11-16")).iter().any(|c| c.required && c.text.contains("732 829 321")), "a wrong key");
    s.ids.clear();
    assert!(checks(&draft, &s, day("2026-11-16")).iter().any(|c| c.required && c.text.contains("SIREN or SIRET")));
    // A client in Belgium, a service: reverse charge, both VAT numbers asked.
    let s = french("franchise");
    let mut abroad = fresh("en");
    abroad.client.address.country = "BE".into();
    abroad.client.ids.clear();
    abroad.lines = vec![Line::of_time("Advice", 60, 9000, "E", 0)];
    refresh(&mut abroad, &s, day("2026-11-16"));
    assert_eq!(abroad.lines[0].category, "AE");
    let said = checks(&abroad, &s, day("2026-11-16"));
    assert!(said.iter().filter(|c| c.required && c.text.contains("VAT number")).count() == 2, "{said:?}");
    let page = html(&abroad, &Translator::new("en"));
    assert!(page.contains("Reverse charge") && !page.contains("293 B"), "{page}");
    // A consumer: no penalties, no €40, no escompte.
    let mut person = fresh("fr");
    person.client = Client { kind: "consumer".into(), name: "Anna Exemple".into(), ..Client::default() };
    person.lines = vec![Line::of_time("Cours", 60, 4000, "E", 0)];
    refresh(&mut person, &s, day("2026-11-16"));
    assert_eq!(person.penalty_rate, 0);
    let page = html(&person, &Translator::new("fr"));
    assert!(!page.contains("40 €") && !page.contains("Escompte") && page.contains("293 B"), "{page}");
    assert!(checks(&person, &s, day("2026-11-16")).iter().all(|c| !c.required), "a consumer's address is theirs to give");
}

/// Issuing is all or nothing: a budget file that cannot be written undoes
/// the record, the journal and the time marked; sharing on with the drafts
/// part off refuses; time billed meanwhile is never billed again.
#[test]
fn issuing_is_all_or_nothing() {
    let place = Place::new("atomic");
    let s = french("franchise");
    let entries = noted(&place, &[(1_790_000_000, 60, "Build the site")]);
    let draft = save_draft(&place.invoices(), &from_time(&entries, &project(), fresh("en"), &s, day("2026-10-05")).unwrap()).unwrap();
    let time = place.time();
    let tr = Translator::new("en");
    let broken = place.root.join("notes").join("missing").join("budget.toml");
    let failed = issue(&place.invoices(), &draft.draft, &s, day("2026-10-05"), &Numbering::default(), None, &Steps { time: Some(&time), ledger: Some((&broken, "work")) }, &tr);
    assert!(failed.is_err());
    assert!(all_in(&place.invoices()).is_empty(), "no record");
    assert_eq!(series::Journal::load(&place.invoices()), Journal::default(), "the journal as before");
    assert!(timelog::sessions_in(&time).iter().all(|x| x.invoice.is_empty()), "its time not marked");
    assert!(draft_in(&place.invoices(), &draft.draft).is_some(), "the draft kept");
    // Sharing on, invoices not shared from here: refused, in words.
    let off = Numbering { sharing: true, drafts_shared: false, claimed: BTreeMap::new() };
    let refused = issue(&place.invoices(), &draft.draft, &s, day("2026-10-05"), &off, None, &Steps { time: Some(&time), ledger: None }, &tr).unwrap_err();
    assert!(refused.contains("Drafts and invoices"), "{refused}");
    // Another device numbered up to 2026-007: the claim counts.
    let claimed = Numbering { sharing: true, drafts_shared: true, claimed: BTreeMap::from([("2026-{N:3}".to_string(), 7u64)]) };
    let issued = issue(&place.invoices(), &draft.draft, &s, day("2026-10-05"), &claimed, None, &Steps { time: Some(&time), ledger: None }, &tr).unwrap();
    assert_eq!(issued.number, "2026-008");
    assert_eq!(series::Journal::load(&place.invoices()).series["2026-{N:3}"].last, 8);
    // A second draft of the same time, made before: refused, its time billed.
    let again = save_draft(&place.invoices(), &from_time(&entries, &project(), fresh("en"), &s, day("2026-10-05")).unwrap()).unwrap();
    let twice = issue(&place.invoices(), &again.draft, &s, day("2026-10-06"), &Numbering::default(), None, &Steps { time: Some(&time), ledger: None }, &tr).unwrap_err();
    assert!(twice.contains("2026-008"), "{twice}");
    // A record deleted by hand: its number is not given again; the gap is said.
    crate::invoice::remove_issued(&place.invoices(), "2026-008").unwrap();
    assert_eq!(series::missing(&series::Journal::load(&place.invoices()), &all_in(&place.invoices())).last().map(String::as_str), Some("2026-008"));
}

/// A credit note cites the invoice; issued, it settles what it credits and
/// frees the time it cancels to be billed again.
#[test]
fn a_credit_note_cancels_and_frees_the_time() {
    let place = Place::new("credit");
    let s = french("franchise");
    let entries = noted(&place, &[(1_790_000_000, 60, "Build the site"), (1_790_010_000, 30, "Call")]);
    let tr = Translator::new("en");
    let time = place.time();
    let none = Steps { time: Some(&time), ledger: None };
    let draft = save_draft(&place.invoices(), &from_time(&entries, &project(), fresh("en"), &s, day("2026-10-05")).unwrap()).unwrap();
    let invoice = issue(&place.invoices(), &draft.draft, &s, day("2026-10-05"), &Numbering::default(), None, &none, &tr).unwrap();
    // The call's line only: a correction; its time freed, the rest stays billed.
    let some = save_draft(&place.invoices(), &credit(&invoice, Some(&[1]), &s, day("2026-10-07"))).unwrap();
    assert_eq!((some.kind, some.corrects.as_ref().map(|c| c.number.as_str())), (Kind::CreditNote, Some("2026-001")));
    let note = issue(&place.invoices(), &some.draft, &s, day("2026-10-07"), &Numbering::default(), None, &none, &tr).unwrap();
    assert_eq!((note.number.as_str(), note.totals.due), ("2026-002", 3000), "in the invoices' series");
    let marks: Vec<String> = timelog::sessions_in(&time).into_iter().map(|x| x.invoice).collect();
    assert_eq!(marks, ["2026-001", ""]);
    let all = all_in(&place.invoices());
    let kept = all.iter().find(|i| i.number == "2026-001").unwrap();
    assert_eq!((kept.status(&all), kept.left(&all)), (Status::PartlyPaid, 6000));
    let page = html(&note, &tr);
    assert!(page.contains("Credit note No. 2026-002") && page.contains("2026-001") && !page.contains("Late payment"), "{page}");
    // Liable for VAT since: a credit note of a franchise invoice still carries no VAT, and its mention.
    let liable = InvoiceSettings { tax: vec![TaxSetting { from: "2026-10-06".into(), scheme: "liable".into(), mention: String::new() }, TaxSetting { from: String::new(), scheme: "franchise".into(), mention: String::new() }], ..s.clone() };
    let late = credit(kept, Some(&[0]), &liable, day("2026-10-08"));
    assert_eq!((late.totals.tax, late.lines[0].category.as_str()), (0, "E"));
    assert!(html(&late, &tr).contains("293 B"), "{}", html(&late, &tr));
    // The rest credited: the invoice is credited, all its time free.
    let rest = save_draft(&place.invoices(), &credit(kept, Some(&[0]), &s, day("2026-10-08"))).unwrap();
    issue(&place.invoices(), &rest.draft, &s, day("2026-10-08"), &Numbering::default(), None, &none, &tr).unwrap();
    let all = all_in(&place.invoices());
    assert_eq!(all.iter().find(|i| i.number == "2026-001").unwrap().status(&all), Status::Credited);
    assert!(timelog::sessions_in(&time).iter().all(|x| x.invoice.is_empty()));
}

/// A deposit invoice, then the final invoice deducting it; payments add up.
#[test]
fn deposits_and_payments() {
    let place = Place::new("deposit");
    let s = french("franchise");
    let tr = Translator::new("en");
    let time = place.time();
    let none = Steps { time: Some(&time), ledger: None };
    let first = save_draft(&place.invoices(), &deposit(&s, fresh("en"), "Deposit, 30 % of the site", 90_000, day("2026-09-01"))).unwrap();
    let paid_first = issue(&place.invoices(), &first.draft, &s, day("2026-09-01"), &Numbering::default(), None, &none, &tr).unwrap();
    assert_eq!((paid_first.kind, paid_first.totals.due), (Kind::Deposit, 90_000));
    assert!(html(&paid_first, &tr).contains("Deposit invoice No. 2026-001"));
    let mut last = fresh("en");
    last.lines = vec![Line { quantity: 1000, unit: "LS".into(), price: 300_000, base: 1, ..Line::of_time("The site", 0, 0, "E", 0) }];
    cite_deposits(&mut last, &[paid_first.clone()], &s, day("2026-10-01"));
    assert_eq!((last.totals.with_tax, last.totals.deposits, last.totals.due), (300_000, 90_000, 210_000));
    let saved = save_draft(&place.invoices(), &last).unwrap();
    let final_invoice = issue(&place.invoices(), &saved.draft, &s, day("2026-10-01"), &Numbering::default(), None, &none, &tr).unwrap();
    assert!(html(&final_invoice, &tr).contains("Deposits invoiced (2026-001)"));
    // Paid in two steps, by hand; the second payment's Undo.
    add_payment(&place.invoices(), &final_invoice.number, Payment { date: "2026-10-10".into(), amount: 100_000, means: "transfer".into(), movement: String::new() }).unwrap();
    let rest = pay_rest(&place.invoices(), &final_invoice.number, day("2026-10-20"), "").unwrap().unwrap();
    assert_eq!((rest.amount, rest.means.as_str()), (110_000, "transfer"));
    let all = all_in(&place.invoices());
    assert_eq!(find_in(&place.invoices(), "2026-002").unwrap().status(&all), Status::Paid);
    assert!(remove_payment(&place.invoices(), "2026-002", &rest).unwrap());
    let all = all_in(&place.invoices());
    assert_eq!(find_in(&place.invoices(), "2026-002").unwrap().status(&all), Status::PartlyPaid);
    // A bank line matched twice is one payment.
    let matched = Payment { date: "2026-10-21".into(), amount: 110_000, means: "transfer".into(), movement: "bank:main:42".into() };
    add_payment(&place.invoices(), "2026-002", matched.clone()).unwrap();
    add_payment(&place.invoices(), "2026-002", matched).unwrap();
    assert_eq!(find_in(&place.invoices(), "2026-002").unwrap().payments.len(), 2);
    unpay(&place.invoices(), "2026-002").unwrap();
    assert_eq!(find_in(&place.invoices(), "2026-002").unwrap().payments.len(), 1, "what the bank said stays");
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
                    Some("vat-fr") => crate::invoice::ids::vat_fr_of(SIREN).unwrap(),
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
                    capital: "1 000".into(),
                    ids: kinds.iter().map(|k| IdSetting { kind: k.clone(), value: value(k), printed: true }).collect(),
                    tax: vec![TaxSetting { from: String::new(), scheme: scheme.clone(), mention: if preset.mention_on(day("2026-11-16")).is_empty() { "Franchise mention of the person's own".into() } else { String::new() } }],
                    currency: preset.currency.clone(),
                    ..InvoiceSettings::default()
                };
                let mut client = business();
                client.address.country = preset.country.clone();
                client.ids = preset.require.iter().filter(|r| r.who == Who::Client).flat_map(|r| r.any.iter()).map(|k| Identifier { kind: k.clone(), value: match k.as_str() {
                        "siren" => CLIENT_SIREN.into(),
                        "siret" => CLIENT_SIRET.into(),
                        _ => value(k),
                    }, printed: true }).collect();
                let mut draft = Invoice { language: "fr".into(), currency: currency_of(&s, ""), client, terms: terms_of(&s, None, None), ..Invoice::default() };
                draft.lines = vec![Line::of_time("Advice", 60, 9000, "S", 0)];
                refresh(&mut draft, &s, day("2026-11-16"));
                let said = checks(&draft, &s, day("2026-11-16"));
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

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The issued file of the samples (`invoice::samples`): the same words as
//! the page to print, a PDF/A-3 carrying its Factur-X XML and its XMP, the
//! QR code where it belongs, pages as many as it takes, the same bytes each
//! time. PDF/A and the Factur-X rules themselves are checked by the CI's
//! validator (Mustang, with veraPDF inside), over the same samples.

use super::*;
use crate::invoice::{html, samples};

/// The words of a text, split at spaces and line ends only (a no-break space joins).
fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text.split([' ', '\n', '\r', '\t']).filter(|w| !w.is_empty()).map(str::to_string).collect();
    out.sort();
    out
}

/// The words a page to print shows: its tags and style sheet taken out.
fn html_words(page: &str) -> Vec<String> {
    let body = page.split("</style>").nth(1).unwrap_or(page).replace("<br>", "\n");
    let mut text = String::new();
    let mut tag = false;
    for c in body.chars() {
        match c {
            '<' => {
                tag = true;
                text.push(' ');
            }
            '>' => tag = false,
            c if !tag => text.push(c),
            _ => {}
        }
    }
    words(&text.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&"))
}

fn every_sample() -> Vec<samples::Sample> {
    let mut all = samples::valid();
    all.push(samples::without_xml());
    all
}

/// The words the layout drew, the page's own aside (its number, the QR
/// code's caption); each inside its page, below no line's bottom, and none
/// over another on its line.
fn drawn_words(pages: &[Vec<Mark>], fonts: &Fonts) -> Vec<String> {
    let mut all = String::new();
    for page in pages {
        let mut spans: Vec<(f32, f32, f32, &str)> = Vec::new();
        for mark in page {
            if let Mark::Text { x, baseline, style, text, furniture } = mark {
                let width = fonts.width(text, *style);
                assert!(*x >= 0.0 && x + width <= WIDTH && *baseline > 0.0 && *baseline <= HEIGHT - 10.0, "outside its page: {text} at ({x}, {baseline})");
                if !furniture {
                    assert!(*baseline <= BOTTOM, "below the lines' bottom: {text} at {baseline}");
                    all.push_str(text);
                    all.push('\n');
                }
                for (other_x, other_end, other_baseline, other) in &spans {
                    let same_line = (baseline - other_baseline).abs() < 1.0;
                    assert!(!same_line || x + width <= *other_x + 0.5 || *x >= other_end - 0.5, "“{text}” over “{other}”");
                }
                spans.push((*x, x + width, *baseline, text.as_str()));
            }
        }
    }
    words(&all)
}

/// Word for word what `html::current` says, in both languages, for every
/// sample: the words the layout drew, each inside its page.
#[test]
fn the_pdf_says_what_the_page_says() {
    let fonts = Fonts::load().unwrap();
    for s in every_sample() {
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let page = html::html(&s.invoice, &tr);
            let c = content(&s.invoice, &tr);
            assert_eq!(words(&c.texts().join("\n")), html_words(&page), "{} in {language}", s.name);
            let qr = epc::payload(&s.invoice).and_then(|p| epc::matrix(&p));
            let pages = lay_out(&c, qr.as_ref(), &fonts, &tr);
            let (drawn, said) = (drawn_words(&pages, &fonts), html_words(&page));
            if drawn != said {
                let only = |a: &[String], b: &[String]| a.iter().filter(|w| a.iter().filter(|x| x == w).count() > b.iter().filter(|x| x == w).count()).cloned().collect::<std::collections::BTreeSet<_>>();
                panic!("{} in {language}, as drawn: drawn only {:?}, the page's only {:?}", s.name, only(&drawn, &said), only(&said, &drawn));
            }
        }
    }
    // A line longer than a page goes on to the next, its last detail drawn.
    let s = samples::valid().into_iter().find(|s| s.name == "fr-long-line").unwrap();
    let tr = Translator::new("fr");
    let pages = lay_out(&content(&s.invoice, &tr), None, &fonts, &tr);
    assert!(pages.len() >= 2, "{} pages", pages.len());
    assert!(pages.iter().flatten().any(|m| matches!(m, Mark::Text { text, .. } if text == "lumiere-automne-080.tif")));
}

/// The number written at `at` in `pdf`.
fn number_at(pdf: &[u8], at: usize) -> usize {
    let digits = pdf[at..].iter().take_while(|b| b.is_ascii_digit()).count();
    std::str::from_utf8(&pdf[at..at + digits]).unwrap().parse().unwrap()
}

/// The offsets of a classic cross-reference table each point at their object.
fn objects_where_the_table_says(pdf: &[u8]) {
    let at = pdf.windows(10).rposition(|w| w == b"startxref\n").expect("startxref") + 10;
    let xref = number_at(pdf, at);
    assert!(pdf[xref..].starts_with(b"xref\n0 "));
    let count = number_at(pdf, xref + 7);
    let entries = xref + 7 + count.to_string().len() + 1;
    for n in 1..count {
        let entry = &pdf[entries + 20 * n..entries + 20 * n + 20];
        if entry[17] == b'n' {
            let offset = number_at(entry, 0);
            assert!(pdf[offset..].starts_with(format!("{n} 0 obj").as_bytes()), "object {n} at {offset}");
        }
    }
    assert!(pdf.ends_with(b"%%EOF"));
}

/// A PDF/A-3 (part 3, level B, in its XMP), its XML attached as Factur-X
/// asks (`factur-x.xml`, text/xml, Alternative, in the catalog's /AF), the
/// Factur-X properties in its XMP; Sioul's own reader of received invoices
/// finds the number and the amount due in it.
#[test]
fn the_file_is_a_pdf_a_3_with_its_xml() {
    let s = samples::valid().into_iter().find(|s| s.name == "fr-franchise").unwrap();
    let rendered = render(&s.invoice, &Options::default()).unwrap();
    let pdf = &rendered.pdf;
    let text = String::from_utf8_lossy(pdf);
    assert!(text.starts_with("%PDF-1.7\n%"), "{}", &text[..20]);
    for said in ["/AFRelationship/Alternative", "/AF[", "/F(factur-x.xml)/UF(factur-x.xml)", "/Subtype/text#2Fxml", "/ModDate(D:20261116", "/OutputIntents", "<pdfaid:part>3</pdfaid:part>", "<pdfaid:conformance>B</pdfaid:conformance>", "<fx:ConformanceLevel>EN 16931</fx:ConformanceLevel>", "<fx:DocumentFileName>factur-x.xml</fx:DocumentFileName>", "<pdfaSchema:prefix>fx</pdfaSchema:prefix>"] {
        assert!(text.contains(said), "{said}");
    }
    objects_where_the_table_says(pdf);
    let read = crate::einvoice::from_pdf(pdf).expect("its XML read back");
    assert_eq!((read.number.as_str(), read.amount.map(|m| m.0)), ("2026-001", Some(s.invoice.totals.due)));
    assert_eq!(rendered.xml.as_deref().map(crate::einvoice::from_xml).map(|e| e.map(|e| e.number)), Some(Some("2026-001".into())));
    // The same bytes each time: nothing in it but the record.
    assert_eq!(render(&s.invoice, &Options::default()).unwrap().pdf, *pdf);
}

/// The French franchise without a VAT number: a PDF/A-3 still, without its
/// XML (design §12, question 1), and the reason said.
#[test]
fn a_franchise_without_vat_number_carries_no_xml_yet() {
    let s = samples::without_xml();
    let rendered = render(&s.invoice, &Options::default()).unwrap();
    let text = String::from_utf8_lossy(&rendered.pdf);
    assert!(rendered.xml.is_none() && rendered.without == Some(cii::Without::FranchiseWithoutVat));
    assert!(text.contains("<pdfaid:part>3</pdfaid:part>") && !text.contains("/EmbeddedFile") && !text.contains("fx:") && !text.contains("/AF["), "a PDF/A without attachment");
    objects_where_the_table_says(&rendered.pdf);
}

fn qr_on(invoice: &Invoice, qr: bool) -> bool {
    let tr = Translator::new("fr");
    let fonts = Fonts::load().unwrap();
    let code = if qr { epc::payload(invoice).and_then(|p| epc::matrix(&p)) } else { None };
    lay_out(&content(invoice, &tr), code.as_ref(), &fonts, &tr).iter().flatten().any(|m| matches!(m, Mark::Qr { .. }))
}

/// The QR code on euro invoices with an IBAN, unless turned off; never on a
/// credit note, nor in pounds.
#[test]
fn the_qr_code_goes_where_it_may() {
    let sample = |name: &str| samples::valid().into_iter().find(|s| s.name == name).unwrap().invoice;
    assert!(qr_on(&sample("fr-franchise"), true));
    assert!(!qr_on(&sample("fr-franchise"), false), "turned off in Settings");
    assert!(!qr_on(&sample("fr-credit-note"), true));
    assert!(!qr_on(&sample("uk-liable"), true));
    assert!(qr_on(&sample("de-liable"), true));
}

/// A long invoice on several pages: the table's head again on each, each
/// page saying which it is.
#[test]
fn a_long_invoice_takes_pages() {
    let s = samples::valid().into_iter().find(|s| s.name == "fr-long").unwrap();
    let tr = Translator::new("fr");
    let fonts = Fonts::load().unwrap();
    let pages = lay_out(&content(&s.invoice, &tr), None, &fonts, &tr);
    assert!(pages.len() >= 2, "{} pages", pages.len());
    for (i, page) in pages.iter().enumerate() {
        let texts: Vec<&str> = page.iter().filter_map(|m| if let Mark::Text { text, .. } = m { Some(text.as_str()) } else { None }).collect();
        assert!(texts.contains(&tr.text("invoice-description", None).as_str()), "page {}: the head", i + 1);
        let last = texts.last().unwrap();
        assert!(last.ends_with(&format!("page {} sur {}", i + 1, pages.len())), "page {}: {last}", i + 1);
        for mark in page {
            if let Mark::Text { baseline, .. } = mark {
                assert!(*baseline < HEIGHT - 20.0, "within the page");
            }
        }
    }
    render(&s.invoice, &Options::default()).unwrap();
}

/// A character the font lacks is said before issue and never drawn.
#[test]
fn characters_the_font_lacks_wait() {
    let mut s = samples::without_xml();
    s.invoice.client.name = "山田 Studio".into();
    assert_eq!(unprintable(&s.invoice), ['山', '田']);
    let refused = render(&s.invoice, &Options::default()).unwrap_err();
    assert!(refused.contains("山田"), "{refused}");
    let checks = crate::invoice::life::checks(&s.invoice, &s.settings, "2026-11-16".parse().unwrap());
    assert!(checks.iter().any(|c| c.required && c.text.contains("山田")), "{checks:?}");
    // What it lacks but has a near one: a non-breaking hyphen, printed as a hyphen.
    s.invoice.client.name = "Jean\u{2011}Paul Exemple".into();
    assert!(unprintable(&s.invoice).is_empty());
    render(&s.invoice, &Options::default()).unwrap();
}

/// Right-to-left writing is held back until Sioul orders it: Hebrew would
/// print backwards, its digits reversed.
#[test]
fn right_to_left_writing_waits() {
    let mut s = samples::without_xml();
    s.invoice.client.name = "סטודיו שלום".into();
    assert!(!right_to_left(&s.invoice).is_empty());
    assert!(render(&s.invoice, &Options::default()).is_err());
    let checks = crate::invoice::life::checks(&s.invoice, &s.settings, "2026-11-16".parse().unwrap());
    assert!(checks.iter().any(|c| c.required && c.text.contains('ס')), "{checks:?}");
}

/// Every sample makes its file, in its language and in the other.
#[test]
fn every_sample_makes_its_file() {
    for s in every_sample() {
        for language in ["en", "fr"] {
            let invoice = Invoice { language: language.into(), ..s.invoice.clone() };
            let rendered = render(&invoice, &Options::default()).unwrap_or_else(|e| panic!("{} in {language}: {e}", s.name));
            assert!(rendered.pdf.len() < 200_000, "{}: {} bytes", s.name, rendered.pdf.len());
        }
    }
}

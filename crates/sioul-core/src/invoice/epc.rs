// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The EPC payment QR code (docs/projects.md, "The issued file"): a euro
//! invoice with an IBAN carries the code a banking app scans to fill in the
//! transfer, the same data printed beside it in plain text, as EPC069-12
//! asks (European Payments Council, "Quick Response Code: Guidelines to
//! Enable the Data Capture for the Initiation of a SEPA Credit Transfer",
//! version 3.1, 19 March 2024; research-generic.md §4).
//!
//! The payload: `BCD`, version `002` (the BIC may be left out), character
//! set `1` (UTF-8), `SCT`, the BIC, the account holder's name (at most 70
//! characters, the holder's own, so that the payer's bank checks the payee
//! without a warning), the IBAN, the amount due (`EUR120.50`), no purpose
//! code, no structured reference, then the invoice's number as the
//! remittance information; no separator after the last element; 331 bytes
//! at most. Error correction M, version 13 at most. Drawn in the PDF as
//! squares (`pdf`), never as a picture. A setting turns it off.

use super::record::{Invoice, Kind};

/// The largest payload EPC069-12 allows, in bytes.
pub const LARGEST: usize = 331;

/// The largest QR version EPC069-12 allows (69 × 69 modules).
const VERSION: u8 = 13;

/// Whether an IBAN's check digits hold (ISO 13616, modulo 97); spaces aside.
pub fn iban_valid(iban: &str) -> bool {
    let iban: String = iban.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    if !(15..=34).contains(&iban.len()) || !iban.bytes().all(|b| b.is_ascii_alphanumeric()) || !iban[..2].bytes().all(|b| b.is_ascii_uppercase()) || !iban[2..4].bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let moved = format!("{}{}", &iban[4..], &iban[..4]);
    let mut rest: u32 = 0;
    for c in moved.chars() {
        let value = c.to_digit(36).unwrap_or(0);
        rest = if value < 10 { (rest * 10 + value) % 97 } else { (rest * 100 + value) % 97 };
    }
    rest == 1
}

/// The payload of `invoice`'s QR code: a euro invoice (not a credit note)
/// with something to pay and a valid IBAN among its issuer's accounts; none
/// otherwise, or when it would not fit.
pub fn payload(invoice: &Invoice) -> Option<String> {
    if invoice.kind == Kind::CreditNote || !invoice.currency.trim().eq_ignore_ascii_case("EUR") || invoice.number.is_empty() {
        return None;
    }
    let due = invoice.totals.due;
    if !(1..=99_999_999_999).contains(&due) {
        return None;
    }
    let bank = invoice.issuer.banks.iter().find(|b| iban_valid(&b.iban))?;
    let iban: String = bank.iban.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    let bic: String = bank.bic.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    let bic = if matches!(bic.len(), 8 | 11) { bic } else { String::new() };
    let holder = if bank.holder.trim().is_empty() { invoice.issuer.name.trim() } else { bank.holder.trim() };
    let name: String = holder.chars().filter(|c| !c.is_control()).take(70).collect();
    if name.trim().is_empty() {
        return None;
    }
    let reference: String = invoice.number.chars().filter(|c| !c.is_control()).take(140).collect();
    let amount = format!("EUR{}.{:02}", due / 100, due % 100);
    let text = ["BCD", "002", "1", "SCT", &bic, name.trim(), &iban, &amount, "", "", &reference].join("\n");
    (text.len() <= LARGEST).then_some(text)
}

/// A QR code's modules: `size` × `size`, dark or light, without its quiet zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matrix {
    pub size: usize,
    dark: Vec<bool>,
}

impl Matrix {
    /// Whether the module at column `x`, row `y` is dark.
    pub fn dark(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size && self.dark[y * self.size + x]
    }
}

/// The QR code of `payload`, error correction M, version 13 at most; none when it does not fit.
pub fn matrix(payload: &str) -> Option<Matrix> {
    use qrcodegen::{QrCode, QrCodeEcc, QrSegment, Version};
    let segments = QrSegment::make_segments(payload);
    let code = QrCode::encode_segments_advanced(&segments, QrCodeEcc::Medium, Version::MIN, Version::new(VERSION), None, false).ok()?;
    let size = usize::try_from(code.size()).ok()?;
    let dark = (0..size).flat_map(|y| (0..size).map(move |x| (x, y))).map(|(x, y)| code.get_module(x as i32, y as i32)).collect();
    Some(Matrix { size, dark })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoice::record::{Bank, Issuer, Totals};

    /// A documented test IBAN (the German example of ISO 13616 and the
    /// ECBS), its key valid; one digit off, refused.
    pub const IBAN: &str = "DE89 3704 0044 0532 0130 00";

    #[test]
    fn an_iban_is_checked_by_its_key() {
        assert!(iban_valid(IBAN));
        assert!(iban_valid("FR76 3000 6000 0112 3456 7890 189"));
        assert!(iban_valid("GB82 WEST 1234 5698 7654 32"));
        assert!(!iban_valid("DE89 3704 0044 0532 0130 01"));
        assert!(!iban_valid("FR76"));
        assert!(!iban_valid(""));
    }

    fn invoice(due: i64) -> Invoice {
        Invoice {
            number: "2026-004".into(),
            currency: "EUR".into(),
            issuer: Issuer { name: "Camille Exemple EI".into(), banks: vec![Bank { holder: String::new(), iban: IBAN.into(), bic: "COBADEFFXXX".into() }], ..Issuer::default() },
            totals: Totals { due, ..Totals::default() },
            ..Invoice::default()
        }
    }

    /// The payload as EPC069-12 lays it out: eleven elements, no separator
    /// after the last; the name of the issuer when the account says none.
    #[test]
    fn the_payload_says_what_the_invoice_prints() {
        let text = payload(&invoice(12_050)).unwrap();
        assert_eq!(text, "BCD\n002\n1\nSCT\nCOBADEFFXXX\nCamille Exemple EI\nDE89370400440532013000\nEUR120.50\n\n\n2026-004");
        let code = matrix(&text).unwrap();
        assert!(code.size >= 21 && code.size <= 69, "{}", code.size);
        // A finder pattern in each corner but one.
        assert!(code.dark(0, 0) && code.dark(code.size - 1, 0) && code.dark(0, code.size - 1));
        // Nothing to pay, another currency, a credit note, a wrong IBAN: no code.
        assert!(payload(&invoice(0)).is_none());
        assert!(payload(&Invoice { currency: "CHF".into(), ..invoice(100) }).is_none());
        assert!(payload(&Invoice { kind: Kind::CreditNote, ..invoice(100) }).is_none());
        let mut wrong = invoice(100);
        wrong.issuer.banks[0].iban = "DE89 3704 0044 0532 0130 01".into();
        assert!(payload(&wrong).is_none());
        // A name past 70 characters is cut; a holder's own name first.
        let mut long = invoice(100);
        long.issuer.banks[0].holder = "Camille Exemple, entrepreneur individuel, studio de photographie et de design".into();
        assert_eq!(payload(&long).unwrap().lines().nth(5).unwrap().chars().count(), 70);
    }
}

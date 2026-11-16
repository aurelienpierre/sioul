// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Business identifiers checked by their own keys, before an invoice is
//! issued with a wrong one: a SIREN and a SIRET by the Luhn sum; a VAT
//! number by its own country, the two letters it starts with (a French one
//! by its two-digit key, another member state's, the United Kingdom's or
//! Switzerland's by its form), never by the issuer's. Other identifiers (a
//! Steuernummer, a UTR, an EIN) are kept as plain text: no key is checked
//! that Sioul does not know. "En cours d'attribution", a SIRET being given,
//! passes as such.

/// How an identifier's value is checked (`check` in a preset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// Nothing checked: any text.
    Text,
    /// Nine digits, Luhn.
    Siren,
    /// Fourteen digits, Luhn, its first nine a SIREN.
    Siret,
    /// "FR", a two-digit key, a SIREN.
    VatFr,
    /// Any country's VAT number, by its own country's form (`vat_valid`).
    Vat,
}

impl Check {
    /// The check a preset names ("siren", "siret", "vat-fr"); text for any other.
    pub fn named(name: &str) -> Check {
        match name {
            "siren" => Check::Siren,
            "siret" => Check::Siret,
            "vat-fr" => Check::VatFr,
            "vat" => Check::Vat,
            _ => Check::Text,
        }
    }
}

/// The value as written, spaces and dots taken out: "732 829 320" → "732829320".
pub fn compact(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace() && !matches!(c, '.' | '-')).collect::<String>().to_uppercase()
}

/// The Luhn sum of a string of digits is a multiple of ten.
fn luhn(digits: &str) -> bool {
    let mut sum = 0u32;
    for (i, c) in digits.chars().rev().enumerate() {
        let Some(mut d) = c.to_digit(10) else { return false };
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum.is_multiple_of(10)
}

fn digits(value: &str, count: usize) -> bool {
    value.len() == count && value.bytes().all(|b| b.is_ascii_digit())
}

/// La Poste's SIREN, whose establishments' SIRETs are checked by another
/// rule: the sum of their digits is a multiple of five (INSEE).
const LA_POSTE: &str = "356000000";

/// A SIRET or SIREN not given yet, said so: « en cours d'attribution » (SP F31808).
pub fn pending(value: &str) -> bool {
    let folded: String = crate::text::fold(value).into_iter().collect();
    folded.contains("en cours d") || folded.contains("pending")
}

/// Whether `value` passes its check; plain text always does, when not empty.
pub fn valid(check: Check, value: &str) -> bool {
    let v = compact(value);
    if matches!(check, Check::Siren | Check::Siret) && pending(value) {
        return true;
    }
    match check {
        Check::Vat => vat_valid(value),
        Check::Text => !value.trim().is_empty(),
        Check::Siren => digits(&v, 9) && luhn(&v),
        Check::Siret => digits(&v, 14) && luhn(&v[..9]) && (luhn(&v) || (v.starts_with(LA_POSTE) && v.bytes().map(|b| u32::from(b - b'0')).sum::<u32>().is_multiple_of(5))),
        Check::VatFr => {
            let Some(rest) = v.strip_prefix("FR") else { return false };
            if rest.len() != 11 {
                return false;
            }
            let (key, siren) = rest.split_at(2);
            if !digits(siren, 9) || !luhn(siren) {
                return false;
            }
            // A key of two digits: (12 + 3 × (SIREN mod 97)) mod 97. Newer
            // keys with letters exist for a few numbers; their rule is not
            // public, so they are taken as written.
            match key.parse::<u64>() {
                Ok(key) => siren.parse::<u64>().is_ok_and(|n| (12 + 3 * (n % 97)) % 97 == key),
                Err(_) => key.chars().all(|c| c.is_ascii_alphanumeric()),
            }
        }
    }
}

/// The forms of VAT numbers by their country's two letters (the EU's VIES
/// forms; the United Kingdom's; Switzerland's UID with its suffix):
/// (letters, shortest and longest after them, what they may hold).
const VAT_FORMS: &[(&str, usize, usize, &str)] = &[
    ("AT", 9, 9, "U8"),
    ("BE", 10, 10, "digits"),
    ("BG", 9, 10, "digits"),
    ("CY", 9, 9, "alnum"),
    ("CZ", 8, 10, "digits"),
    ("DE", 9, 9, "digits"),
    ("DK", 8, 8, "digits"),
    ("EE", 9, 9, "digits"),
    ("EL", 9, 9, "digits"),
    ("ES", 9, 9, "alnum"),
    ("FI", 8, 8, "digits"),
    ("HR", 11, 11, "digits"),
    ("HU", 8, 8, "digits"),
    ("IE", 8, 9, "alnum"),
    ("IT", 11, 11, "digits"),
    ("LT", 9, 12, "digits"),
    ("LU", 8, 8, "digits"),
    ("LV", 11, 11, "digits"),
    ("MT", 8, 8, "digits"),
    ("NL", 12, 12, "alnum"),
    ("PL", 10, 10, "digits"),
    ("PT", 9, 9, "digits"),
    ("RO", 2, 10, "digits"),
    ("SE", 12, 12, "digits"),
    ("SI", 8, 8, "digits"),
    ("SK", 10, 10, "digits"),
    ("XI", 5, 12, "alnum"),
    ("GB", 5, 12, "alnum"),
];

/// A VAT number checked by its own country: a French one by its key (a
/// SIREN with its two digits), another by its country's form; one whose
/// first letters name no country Sioul knows is taken as written.
pub fn vat_valid(value: &str) -> bool {
    let v = compact(value);
    if v.len() < 4 {
        return false;
    }
    let (country, rest) = v.split_at(2);
    if country == "FR" {
        return valid(Check::VatFr, &v);
    }
    // Switzerland: CHE, nine digits, and MWST, TVA or IVA after it when liable.
    if let Some(digits) = v.strip_prefix("CHE") {
        let digits = digits.trim_end_matches("MWST").trim_end_matches("TVA").trim_end_matches("IVA");
        return digits.len() == 9 && digits.bytes().all(|b| b.is_ascii_digit());
    }
    match VAT_FORMS.iter().find(|(c, ..)| *c == country) {
        Some((_, least, most, kind)) => {
            (*least..=*most).contains(&rest.len())
                && match *kind {
                    "digits" => rest.bytes().all(|b| b.is_ascii_digit()),
                    "U8" => rest.starts_with('U') && rest[1..].bytes().all(|b| b.is_ascii_digit()),
                    _ => rest.bytes().all(|b| b.is_ascii_alphanumeric()),
                }
        }
        None => !value.trim().is_empty(),
    }
}

/// A French VAT number made from a SIREN, with its numeric key.
pub fn vat_fr_of(siren: &str) -> Option<String> {
    let s = compact(siren);
    let n = s.parse::<u64>().ok().filter(|_| digits(&s, 9) && luhn(&s))?;
    Some(format!("FR{:02}{s}", (12 + 3 * (n % 97)) % 97))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Made-up numbers that pass their checks, and the same with one digit wrong.
    #[test]
    fn identifiers_are_checked_by_their_keys() {
        assert!(valid(Check::Siren, "732 829 320"));
        assert!(!valid(Check::Siren, "732 829 321"));
        assert!(!valid(Check::Siren, "73282932"), "eight digits");
        assert!(valid(Check::Siret, "732 829 320 00017"));
        assert!(!valid(Check::Siret, "732 829 320 00018"));
        assert!(!valid(Check::Siret, "732 829 321 00017"), "its SIREN wrong");
        let vat = vat_fr_of("732829320").unwrap();
        assert!(valid(Check::VatFr, &vat), "{vat}");
        assert!(valid(Check::VatFr, &format!("{} {} {}", &vat[..4], &vat[4..7], &vat[7..])));
        let wrong = format!("FR{:02}732829320", (vat[2..4].parse::<u32>().unwrap() + 1) % 97);
        assert!(!valid(Check::VatFr, &wrong));
        assert!(!valid(Check::VatFr, "DE123456789"));
        assert!(valid(Check::Text, "12/345/67890") && !valid(Check::Text, "  "));
        // A VAT number by its own country (the review of phase 1, F5).
        assert!(valid(Check::Vat, "BE0417497106") && valid(Check::Vat, "DE 123 456 789") && valid(Check::Vat, &vat));
        assert!(!valid(Check::Vat, "BE04174971") && !valid(Check::Vat, "DE12345678") && !valid(Check::Vat, &wrong));
        assert!(valid(Check::Vat, "CHE-123.456.789 MWST") && valid(Check::Vat, "ATU12345678") && valid(Check::Vat, "NL123456789B01"));
        // A SIRET being given.
        assert!(valid(Check::Siret, "en cours d’attribution") && pending("SIRET en cours d'attribution"));
        assert_eq!(Check::named("siren"), Check::Siren);
        assert_eq!(Check::named("vat"), Check::Vat);
        assert_eq!(Check::named("ust-idnr"), Check::Text);
    }
}

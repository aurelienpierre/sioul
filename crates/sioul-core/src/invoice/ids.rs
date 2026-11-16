// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Business identifiers checked by their own keys, before an invoice is
//! issued with a wrong one: a SIREN and a SIRET by the Luhn sum, a French VAT
//! number by its two-digit key. Other identifiers (a Steuernummer, a UTR, an
//! EIN) are kept as plain text: no key is checked that Sioul does not know.

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
}

impl Check {
    /// The check a preset names ("siren", "siret", "vat-fr"); text for any other.
    pub fn named(name: &str) -> Check {
        match name {
            "siren" => Check::Siren,
            "siret" => Check::Siret,
            "vat-fr" => Check::VatFr,
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

/// Whether `value` passes its check; plain text always does, when not empty.
pub fn valid(check: Check, value: &str) -> bool {
    let v = compact(value);
    match check {
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
        assert_eq!(Check::named("siren"), Check::Siren);
        assert_eq!(Check::named("ust-idnr"), Check::Text);
    }
}

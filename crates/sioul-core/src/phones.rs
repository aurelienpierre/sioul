// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Phone numbers, compared rather than rewritten: "04 65 71 12 34",
//! "04.65.71.12.34", "+33 4 65 71 12 34", "+33 (0)4 65 71 12 34" and
//! "0033465711234" are one number, written five ways. Each number gets a key,
//! its international form (E.164, ITU-T) with its extension, and two numbers
//! with one key are the same; what the card says stays as it was written.
//!
//! A number written without its country is read as the country chosen in the
//! Contacts settings, else the country of the system's locale. A small table
//! knows how the usual countries write their numbers at home: the trunk
//! prefix dropped in the international form ("0" in most of Europe, "1" in
//! North America, none in Spain or Italy) and how long a number is, so that a
//! short code ("3631", "112") is never given a country. Values with letters
//! ("1-555-SIOUL"), stars and hashes ("*#06#") are compared as written.
//! The rules are those of each country's numbering plan, as the ITU publishes
//! them (https://www.itu.int/oth/T0202); libphonenumber knows every country
//! but weighs megabytes, for what this table does for the usual ones.

/// How a country writes its numbers at home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// ISO 3166-1: "FR".
    pub code: &'static str,
    /// The country calling code, without "+": "33".
    pub calling: &'static str,
    /// What starts a national number and goes in the international form: "0"
    /// in most of Europe, "1" in North America; "" where nothing does (Spain,
    /// Italy, whose leading 0 belongs to the number).
    pub trunk: &'static str,
    /// What dials abroad from there besides "00": "011" in North America.
    pub international: &'static str,
    /// How many digits a national number has, its trunk prefix included:
    /// fewer or more, it is a short code or something else, compared as written.
    pub digits: (usize, usize),
    /// The French plan: France and its overseas departments share ten-digit
    /// numbers, whose first digits say which country code they take.
    pub french: bool,
}

const fn region(code: &'static str, calling: &'static str, trunk: &'static str, international: &'static str, digits: (usize, usize)) -> Region {
    Region { code, calling, trunk, international, digits, french: false }
}

const fn french(code: &'static str, calling: &'static str) -> Region {
    Region { code, calling, trunk: "0", international: "00", digits: (10, 10), french: true }
}

/// The countries known here (docs/client.md, "Duplicates").
pub const REGIONS: &[Region] = &[
    french("FR", "33"),
    french("RE", "262"),
    french("YT", "262"),
    french("GP", "590"),
    french("MQ", "596"),
    french("GF", "594"),
    region("BE", "32", "0", "00", (9, 10)),
    region("CH", "41", "0", "00", (10, 10)),
    region("LU", "352", "", "00", (6, 11)),
    region("DE", "49", "0", "00", (6, 15)),
    region("AT", "43", "0", "00", (6, 14)),
    region("NL", "31", "0", "00", (10, 10)),
    region("GB", "44", "0", "00", (10, 11)),
    region("IE", "353", "0", "00", (7, 10)),
    region("ES", "34", "", "00", (9, 9)),
    region("PT", "351", "", "00", (9, 9)),
    region("IT", "39", "", "00", (6, 11)),
    region("DK", "45", "", "00", (8, 8)),
    region("NO", "47", "", "00", (8, 8)),
    region("SE", "46", "0", "00", (7, 10)),
    region("FI", "358", "0", "00", (6, 12)),
    region("PL", "48", "", "00", (9, 9)),
    region("GR", "30", "", "00", (10, 10)),
    region("US", "1", "1", "011", (10, 11)),
    region("CA", "1", "1", "011", (10, 11)),
    region("AU", "61", "0", "0011", (10, 10)),
    region("NZ", "64", "0", "00", (8, 11)),
    region("JP", "81", "0", "010", (10, 11)),
    region("IN", "91", "0", "00", (11, 11)),
    region("MA", "212", "0", "00", (10, 10)),
    region("DZ", "213", "0", "00", (9, 10)),
    region("TN", "216", "", "00", (8, 8)),
    region("SN", "221", "", "00", (9, 9)),
    region("CI", "225", "", "00", (10, 10)),
];

/// A country of the table, by its ISO code ("fr" too).
pub fn region_named(code: &str) -> Option<&'static Region> {
    let code = code.trim();
    REGIONS.iter().find(|r| r.code.eq_ignore_ascii_case(code))
}

/// The country a locale names: "fr_FR.UTF-8" → France, "en-GB" → the United
/// Kingdom; none for a language alone ("fr"), "C" or a country not known here.
pub fn region_of_locale(locale: &str) -> Option<&'static Region> {
    let base = locale.split(['.', '@']).next().unwrap_or("");
    let country = base.split(['_', '-']).nth(1)?;
    region_named(country)
}

/// The country numbers without one are read as, unless the settings say:
/// the system's locale (LC_ALL, LC_TELEPHONE, LANG), else Sioul's language's
/// own (`language_locale`, "fr_FR" for French).
pub fn usual_region(language_locale: &str) -> Option<&'static Region> {
    ["LC_ALL", "LC_TELEPHONE", "LANG"].iter().filter_map(|v| std::env::var(v).ok()).find_map(|v| region_of_locale(&v)).or_else(|| region_of_locale(language_locale))
}

/// The country chosen in the settings (`setting`, an ISO code), else the usual one.
/// A code not known here reads numbers without a country as they are written.
pub fn chosen(setting: Option<&str>, language_locale: &str) -> Option<&'static Region> {
    match setting.map(str::trim).filter(|s| !s.is_empty()) {
        Some(code) => region_named(code),
        None => usual_region(language_locale),
    }
}

/// The French plan's overseas numbers, by the three digits after the 0:
/// Réunion and Mayotte share +262, Guadeloupe (with Saint-Martin and
/// Saint-Barthélemy) +590, French Guiana +594, Martinique +596 (ARCEP).
fn french_overseas(national: &str) -> Option<&'static str> {
    match national.get(..3)? {
        "262" | "263" | "269" | "639" | "692" | "693" => Some("262"),
        "590" | "690" | "691" => Some("590"),
        "594" | "694" => Some("594"),
        "596" | "696" | "697" => Some("596"),
        _ => None,
    }
}

/// The country codes whose numbers never start with 0: written "+33 06…",
/// the 0 is the trunk prefix kept by mistake. Italy's 0 is part of the number.
fn drops_written_trunk(calling: &str) -> bool {
    REGIONS.iter().any(|r| r.calling == calling && r.trunk == "0")
}

/// "+330465711234" → "+33465711234": a trunk 0 written after a country code that has none.
fn without_written_trunk(digits: &str) -> String {
    // Country codes are a prefix code (no code starts another): at most one matches.
    for length in 1..=3 {
        if let Some((calling, rest)) = digits.split_at_checked(length)
            && rest.starts_with('0')
            && drops_written_trunk(calling)
        {
            return format!("{calling}{}", &rest[1..]);
        }
    }
    digits.to_string()
}

/// Characters that only space a number out: spaces of every width, dots,
/// dashes, slashes; and the invisible marks of writing direction phones put
/// around numbers (iOS writes "\u{202a}+33 6…\u{202c}").
fn is_spacing(c: char) -> bool {
    matches!(c, ' ' | '\t' | '.' | '-' | '/' | '\u{a0}' | '\u{202f}' | '\u{2009}' | '\u{2007}' | '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}')
        || matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

/// The extension at the end of a number, after its marker: "poste 12",
/// "ext. 12", "x12", "#12", ",12", ";ext=12". Returns the number before it and the digits.
fn split_extension(value: &str) -> (&str, &str) {
    let trimmed = value.trim_end();
    let before_digits = trimmed.trim_end_matches(|c: char| c.is_ascii_digit());
    if before_digits.len() == trimmed.len() {
        return (value, "");
    }
    let digits = &trimmed[before_digits.len()..];
    let head = before_digits.trim_end_matches(|c: char| c == ' ' || c == '\u{a0}' || c == '\u{202f}' || c == '.' || c == ':' || c == '=');
    let lower = head.to_ascii_lowercase();
    for marker in ["extension", "poste", "ext", "x", "#", ",", ";"] {
        if lower.ends_with(marker) {
            let number = head[..head.len() - marker.len()].trim_end_matches(|c: char| is_spacing(c) || c == ';' || c == ',');
            // "x" only after the number ("…0100 x12"), never inside a word.
            if marker == "x" && !number.ends_with(|c: char| c.is_ascii_digit() || c == ')') {
                return (value, "");
            }
            return (number, digits);
        }
    }
    (value, "")
}

/// The digits of a number, with "+" first when it is written internationally;
/// none when it holds anything a number does not (letters, "*", "#").
fn digits_of(number: &str) -> Option<String> {
    let mut out = String::new();
    for (i, c) in number.trim().chars().enumerate() {
        match c {
            '+' if i == 0 => out.push('+'),
            '0'..='9' => out.push(c),
            '(' | ')' => {}
            c if is_spacing(c) => {}
            _ => return None,
        }
    }
    (out.trim_start_matches('+').len() > 0).then_some(out)
}

/// "+33 (0)6…": the trunk prefix some write in parentheses after the country
/// code, out; Italy's 0 belongs to the number and stays.
fn without_zero_in_parentheses(number: &str) -> String {
    let Some(at) = number.find("(0)") else { return number.to_string() };
    let before: String = number[..at].chars().filter(char::is_ascii_digit).collect();
    let international = number.trim_start().starts_with('+') || before.starts_with("00");
    if !international {
        return number.to_string();
    }
    let calling = before.trim_start_matches("00");
    let kept = if calling == "39" { "0" } else { "" };
    format!("{}{kept}{}", &number[..at], &number[at + 3..])
}

/// A number's key, to compare it: its international form ("+33465711234"),
/// its extension after ";ext=". A number written without its country takes
/// `region`'s; a short code and a value that is not a number are compared as
/// written, spaces aside. Never shown, never written to a card.
pub fn key(value: &str, region: Option<&Region>) -> String {
    let value = value.trim_matches(|c: char| c.is_whitespace() || is_spacing(c));
    let value = value.get(..4).filter(|s| s.eq_ignore_ascii_case("tel:")).map_or(value, |_| value[4..].trim());
    let (number, extension) = split_extension(value);
    let Some(main) = number_key(number, region) else {
        return value.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    };
    if extension.is_empty() { main } else { format!("{main};ext={extension}") }
}

/// The number's own key, without the extension; none when it is not a number.
fn number_key(number: &str, region: Option<&Region>) -> Option<String> {
    let digits = digits_of(&without_zero_in_parentheses(number))?;
    if let Some(international) = digits.strip_prefix('+') {
        return Some(format!("+{}", without_written_trunk(international)));
    }
    let Some(region) = region else { return Some(digits) };
    // Abroad: "00", or the country's own prefix ("011" in North America).
    for prefix in [region.international, "00"] {
        if let Some(rest) = digits.strip_prefix(prefix).filter(|rest| rest.len() >= 6 && !rest.starts_with('0')) {
            return Some(format!("+{}", without_written_trunk(rest)));
        }
    }
    let (fewest, most) = region.digits;
    if !(fewest..=most).contains(&digits.len()) {
        // A short code ("3631"), or something else: as written.
        return Some(digits);
    }
    match region.trunk {
        "" => Some(format!("+{}{digits}", region.calling)),
        // North America: ten digits, or eleven after its "1".
        "1" => match digits.len() {
            10 if !digits.starts_with(['0', '1']) => Some(format!("+1{digits}")),
            11 if digits.starts_with('1') => Some(format!("+{digits}")),
            _ => Some(digits),
        },
        trunk => match digits.strip_prefix(trunk) {
            Some(national) if !national.starts_with('0') => {
                let calling = if region.french { french_overseas(national).unwrap_or("33") } else { region.calling };
                Some(format!("+{calling}{national}"))
            }
            // Without its trunk prefix ("465711234" in France): as written.
            _ => Some(digits),
        },
    }
}

/// Whether a key stands for a whole number (not a short code), so that two
/// cards sharing it may be one person.
pub fn is_whole(key: &str) -> bool {
    let digits = key.split(';').next().unwrap_or(key);
    digits.starts_with('+') || (digits.len() >= 7 && digits.chars().all(|c| c.is_ascii_digit()))
}

/// Whether a number is written in its international form, as people write it: "+33 6…".
pub fn is_international(value: &str) -> bool {
    let value = value.trim_matches(|c: char| c.is_whitespace() || is_spacing(c));
    let value = value.get(..4).filter(|s| s.eq_ignore_ascii_case("tel:")).map_or(value, |_| value[4..].trim());
    value.starts_with('+')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_in(value: &str, country: &str) -> String {
        key(value, region_named(country))
    }

    #[test]
    fn france_its_spaces_and_its_prefix() {
        let all = ["\u{202a}+33 4 65 71 12 34\u{202c}", "04 65 71 12 34", "04.65.71.12.34", "04-65-71-12-34", "0465711234", "+33 4 65 71 12 34", "+33 (0)4 65 71 12 34", "+33 04 65 71 12 34", "0033 4 65 71 12 34", "tel:+33-4-65-71-12-34", "04\u{a0}65\u{202f}71 12 34"];
        for written in all {
            assert_eq!(key_in(written, "FR"), "+33465711234", "{written}");
        }
        // Overseas departments share the ten-digit plan, with their own codes.
        assert_eq!(key_in("0692 12 34 56", "FR"), "+262692123456");
        assert_eq!(key_in("+262 692 12 34 56", "FR"), "+262692123456");
        assert_eq!(key_in("05 90 12 34 56", "FR"), "+590590123456");
        assert_eq!(key_in("0696 12 34 56", "FR"), "+596696123456");
        assert_eq!(key_in("0594 12 34 56", "FR"), "+594594123456");
        // From Réunion, a Paris number is still France's.
        assert_eq!(key_in("01 99 00 67 89", "RE"), "+33199006789");
        assert_eq!(key_in("0692 12 34 56", "RE"), "+262692123456");
        // Short codes and numbers without their 0 are left as written.
        assert_eq!(key_in("36 31", "FR"), "3631");
        assert_eq!(key_in("112", "FR"), "112");
        assert_eq!(key_in("465711234", "FR"), "465711234");
    }

    #[test]
    fn neighbours_with_and_without_a_trunk_prefix() {
        assert_eq!(key_in("0475 12 34 56", "BE"), "+32475123456");
        assert_eq!(key_in("02 123 45 67", "BE"), "+3221234567");
        assert_eq!(key_in("+32 (0)2 123 45 67", "FR"), "+3221234567");
        assert_eq!(key_in("044 000 12 34", "CH"), "+41440001234");
        assert_eq!(key_in("0041 44 000 12 34", "FR"), "+41440001234");
        // Luxembourg has no trunk prefix: the whole number takes +352.
        assert_eq!(key_in("621 123 456", "LU"), "+352621123456");
        assert_eq!(key_in("+352 621 123 456", "FR"), "+352621123456");
        assert_eq!(key_in("030 1234567", "DE"), "+49301234567");
        assert_eq!(key_in("+49 (0)30 1234567", "DE"), "+49301234567");
        assert_eq!(key_in("0151 23456789", "DE"), "+4915123456789");
        assert_eq!(key_in("020 7946 0018", "GB"), "+442079460018");
        assert_eq!(key_in("+44 (0)20 7946 0018", "FR"), "+442079460018");
        assert_eq!(key_in("07700 900123", "GB"), "+447700900123");
        assert_eq!(key_in("06 12345678", "NL"), "+31612345678");
        assert_eq!(key_in("01 234 5678", "IE"), "+35312345678");
        assert_eq!(key_in("0664 1234567", "AT"), "+436641234567");
    }

    #[test]
    fn spain_portugal_and_italy_keep_their_numbers_whole() {
        assert_eq!(key_in("612 34 56 78", "ES"), "+34612345678");
        assert_eq!(key_in("91 123 45 67", "ES"), "+34911234567");
        assert_eq!(key_in("912 345 678", "PT"), "+351912345678");
        // Italy's leading 0 is part of the number, in the international form too.
        assert_eq!(key_in("06 1234 5678", "IT"), "+390612345678");
        assert_eq!(key_in("+39 06 1234 5678", "FR"), "+390612345678");
        assert_eq!(key_in("+39 (0)6 1234 5678", "FR"), "+390612345678");
        assert_eq!(key_in("347 123 4567", "IT"), "+393471234567");
        assert_eq!(key_in("20 12 34 56", "DK"), "+4520123456");
        assert_eq!(key_in("412 34 567", "NO"), "+4741234567");
        assert_eq!(key_in("512 345 678", "PL"), "+48512345678");
        assert_eq!(key_in("21 0123 4567", "GR"), "+302101234567");
        assert_eq!(key_in("070 123 45 67", "SE"), "+46701234567");
        assert_eq!(key_in("040 1234567", "FI"), "+358401234567");
    }

    #[test]
    fn north_america_and_further() {
        for written in ["(514) 555-0100", "514-555-0100", "514.555.0100", "1 514 555 0100", "+1 (514) 555-0100", "+1-514-555-0100"] {
            assert_eq!(key_in(written, "CA"), "+15145550100", "{written}");
        }
        assert_eq!(key_in("(212) 555-0100", "US"), "+12125550100");
        // Seven digits without the area code: as written.
        assert_eq!(key_in("555-0100", "US"), "5550100");
        // Abroad from there: "011".
        assert_eq!(key_in("011 33 4 65 71 12 34", "US"), "+33465711234");
        assert_eq!(key_in("0412 345 678", "AU"), "+61412345678");
        assert_eq!(key_in("0011 33 4 65 71 12 34", "AU"), "+33465711234");
        assert_eq!(key_in("021 123 4567", "NZ"), "+64211234567");
        assert_eq!(key_in("090-1234-5678", "JP"), "+819012345678");
        assert_eq!(key_in("010 33 4 65 71 12 34", "JP"), "+33465711234");
        assert_eq!(key_in("098765 43210", "IN"), "+919876543210");
        assert_eq!(key_in("06 61 23 45 67", "MA"), "+212661234567");
        assert_eq!(key_in("0551 23 45 67", "DZ"), "+213551234567");
        assert_eq!(key_in("20 123 456", "TN"), "+21620123456");
        assert_eq!(key_in("77 123 45 67", "SN"), "+221771234567");
        assert_eq!(key_in("07 07 12 34 56", "CI"), "+2250707123456");
    }

    #[test]
    fn extensions_kept_and_odd_values_left_alone() {
        assert_eq!(key_in("+33 1 99 00 67 89 poste 123", "FR"), "+33199006789;ext=123");
        assert_eq!(key_in("01 99 00 67 89 ext. 123", "FR"), "+33199006789;ext=123");
        assert_eq!(key_in("+1 212 555 0100 x12", "US"), "+12125550100;ext=12");
        assert_eq!(key_in("tel:+1-212-555-0100;ext=12", "FR"), "+12125550100;ext=12");
        assert_eq!(key_in("01 99 00 67 89 #12", "FR"), "+33199006789;ext=12");
        assert_eq!(key_in("01 99 00 67 89,12", "FR"), "+33199006789;ext=12");
        // Another extension is another number.
        assert_ne!(key_in("+33 1 99 00 67 89 poste 12", "FR"), key_in("+33 1 99 00 67 89", "FR"));
        // Not numbers: compared as written, spaces aside, case aside.
        assert_eq!(key_in("*#06#", "FR"), "*#06#");
        assert_eq!(key_in("1-555-SIOUL", "US"), "1-555-sioul");
        assert_eq!(key_in("sip:jane@example.org", "FR"), "sip:jane@example.org");
        // No country known: national numbers compared by their digits.
        assert_eq!(key("04 65 71 12 34", None), "0465711234");
        assert_eq!(key("+33 4 65 71 12 34", None), "+33465711234");
        assert!(is_whole("+33465711234") && is_whole("0465711234") && !is_whole("3631") && !is_whole("*#06#"));
        assert!(is_international(" +33 4 65") && is_international("tel:+33465") && !is_international("04 65"));
    }

    #[test]
    fn the_country_from_the_settings_or_the_locale() {
        assert_eq!(region_of_locale("fr_FR.UTF-8").map(|r| r.code), Some("FR"));
        assert_eq!(region_of_locale("fr_BE.UTF-8@euro").map(|r| r.code), Some("BE"));
        assert_eq!(region_of_locale("en-GB").map(|r| r.code), Some("GB"));
        assert_eq!(region_of_locale("fr").map(|r| r.code), None);
        assert_eq!(region_of_locale("C.UTF-8").map(|r| r.code), None);
        assert_eq!(region_of_locale("pt_BR.UTF-8").map(|r| r.code), None, "a country not in the table");
        assert_eq!(chosen(Some("ch"), "fr_FR").map(|r| r.code), Some("CH"));
        assert_eq!(chosen(Some("XX"), "fr_FR"), None, "an unknown code reads numbers as written");
        // Every country once, each code a code of its own.
        let codes: std::collections::BTreeSet<&str> = REGIONS.iter().map(|r| r.code).collect();
        assert_eq!(codes.len(), REGIONS.len());
    }
}

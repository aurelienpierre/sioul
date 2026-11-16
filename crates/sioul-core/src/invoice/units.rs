// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The units a line is counted in (BT-130): UN/ECE Recommendation 20's
//! codes, the ones Sioul offers and prints in words (`invoice-unit-…`).
//! EN 16931 takes Recommendation 20's codes only, and Factur-X's rules refuse
//! any other (FX-SCH-A-000613): a word people write for a unit ("hrs",
//! "jours", "pcs") is read as its code, anything else is refused
//! (`life::checks`, the agents' `draft_invoice`).

/// The codes Sioul offers: an hour, a minute, a day, one (a unit), a month,
/// a kilometre, a lump sum.
pub const CODES: [&str; 7] = ["HUR", "MIN", "DAY", "C62", "MON", "KMT", "LS"];

/// Whether `code` is one of Sioul's units.
pub fn known(code: &str) -> bool {
    CODES.contains(&code)
}

/// The code a word or a code stands for: "HRS", "h", "heures" → HUR;
/// "pcs", "unité" → C62; none for anything else.
pub fn code(word: &str) -> Option<&'static str> {
    let word = word.trim().trim_end_matches('.').to_lowercase();
    if let Some(code) = CODES.iter().find(|c| c.eq_ignore_ascii_case(&word)) {
        return Some(code);
    }
    Some(match word.as_str() {
        "h" | "hr" | "hrs" | "hour" | "hours" | "heure" | "heures" => "HUR",
        "mn" | "minute" | "minutes" => "MIN",
        "d" | "j" | "day" | "days" | "jour" | "jours" | "journée" | "journées" => "DAY",
        "u" | "unit" | "units" | "unité" | "unités" | "pc" | "pcs" | "piece" | "pieces" | "pièce" | "pièces" | "ea" | "each" => "C62",
        "mo" | "month" | "months" | "mois" => "MON",
        "km" | "kilometre" | "kilometres" | "kilometer" | "kilometers" | "kilomètre" | "kilomètres" => "KMT",
        "lump sum" | "flat fee" | "forfait" => "LS",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_read_as_their_codes() {
        assert_eq!(code("HRS"), Some("HUR"));
        assert_eq!(code(" heures "), Some("HUR"));
        assert_eq!(code("pcs"), Some("C62"));
        assert_eq!(code("Jours"), Some("DAY"));
        assert_eq!(code("hur"), Some("HUR"));
        assert_eq!(code("forfait"), Some("LS"));
        assert_eq!(code("PCE"), None, "not one of Sioul's");
        assert_eq!(code("bushels"), None);
        assert!(CODES.iter().all(|c| known(c) && code(c) == Some(c)));
    }
}

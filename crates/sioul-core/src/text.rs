// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Text helpers shared by the detectors.
//!
//! Comparisons ignore case and French accents, so "Réinitialiser" matches
//! "reinitialiser". Folding maps one character to exactly one character, so a
//! position found in the folded text is valid in the original text too: codes
//! are matched on folded text and copied from the original.

/// Lowercases one character and strips its accent, one character out for one in.
pub fn fold_char(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'À' | 'Â' | 'Ä' | 'Á' | 'æ' | 'Æ' => 'a',
        'ç' | 'Ç' => 'c',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'î' | 'ï' | 'í' | 'Î' | 'Ï' | 'Í' => 'i',
        'ô' | 'ö' | 'ó' | 'Ô' | 'Ö' | 'Ó' | 'œ' | 'Œ' => 'o',
        'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => 'u',
        'ÿ' | 'Ÿ' => 'y',
        // Typographic apostrophes and non-breaking spaces are the norm in French mail.
        '\u{2019}' | '\u{2018}' => '\'',
        '\u{a0}' | '\u{202f}' => ' ',
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}

/// The folded characters of a text.
pub fn fold(text: &str) -> Vec<char> {
    text.chars().map(fold_char).collect()
}

/// Whether a character can belong to a word, for word-boundary checks.
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// Position of `needle` as a whole word in folded `haystack`, from `from` on.
///
/// Whole words only: "otp" must not match inside "hotpot".
pub fn find_word(haystack: &[char], needle: &str, from: usize) -> Option<usize> {
    let needle = fold(needle);
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let last = haystack.len() - needle.len();
    (from..=last).find(|&i| {
        haystack[i..i + needle.len()] == needle[..]
            && (i == 0 || !is_word_char(haystack[i - 1]))
            && (i + needle.len() == haystack.len() || !is_word_char(haystack[i + needle.len()]))
    })
}

/// Whether `text` contains `needle` as a whole word, ignoring case and accents.
pub fn contains_word(text: &str, needle: &str) -> bool {
    find_word(&fold(text), needle, 0).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_ignores_case_and_accents() {
        assert!(contains_word("Réinitialiser votre mot de passe", "reinitialiser votre mot de passe"));
        assert!(contains_word("CODE D\u{2019}ACCÈS", "code d'acces"));
    }

    #[test]
    fn whole_words_only() {
        assert!(!contains_word("a hotpot recipe", "otp"));
        assert!(contains_word("your OTP: 123456", "otp"));
    }
}

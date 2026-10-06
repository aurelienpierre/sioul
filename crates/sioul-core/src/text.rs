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

/// A text as the detectors read it: its web addresses left out (a tracking
/// link is hundreds of characters long, and its digits are no code), with the
/// brackets around them; invisible characters left out (the zero-width
/// joiners that pad a newsletter's preview line); each run of spaces made one
/// space, or one line break when it holds one. A single space of any kind
/// stays as it is: "482 913" with a narrow no-break space is still one code.
pub fn readable(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    // The run of spaces being read: its first character, its length, whether it breaks the line.
    let mut run: Option<(char, usize, bool)> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if is_invisible(c) {
            i += 1;
            continue;
        }
        if let Some(length) = link_at(&chars, i) {
            // The bracket opened just before it goes with it, and the spaces before that join the run.
            if run.is_none() && out.ends_with(['<', '(', '[']) {
                out.pop();
                while let Some(space) = out.chars().last().filter(|c| c.is_whitespace()) {
                    out.pop();
                    run = Some(run.map_or((space, 1, space == '\n'), |(first, n, broken)| (first, n + 1, broken || space == '\n')));
                }
            }
            i += length;
            if chars.get(i).is_some_and(|c| matches!(c, '>' | ')' | ']')) {
                i += 1;
            }
            // Where it was, the words around stay apart.
            run = Some(run.map_or((' ', 2, false), |(first, n, broken)| (first, n + 2, broken)));
            continue;
        }
        if c.is_whitespace() {
            run = Some(match run {
                None => (c, 1, c == '\n'),
                Some((first, n, broken)) => (first, n + 1, broken || c == '\n'),
            });
            i += 1;
            continue;
        }
        if let Some((first, n, broken)) = run.take()
            && !out.is_empty()
        {
            out.push(if broken {
                '\n'
            } else if n == 1 && first != '\t' && first != '\r' {
                first
            } else {
                ' '
            });
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Characters that show nothing: zero-width spaces and joiners, the word
/// joiner, the byte order mark, the soft hyphen, the combining grapheme joiner.
fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}'..='\u{2064}' | '\u{feff}' | '\u{00ad}' | '\u{034f}' | '\u{180e}')
}

/// The length of the web address starting at `at` ("https://…", "http://…",
/// "www.…"), its last punctuation left to the sentence; none when there is none.
fn link_at(chars: &[char], at: usize) -> Option<usize> {
    if at > 0 && chars[at - 1].is_alphanumeric() {
        return None;
    }
    let starts = |prefix: &str| prefix.chars().enumerate().all(|(k, p)| chars.get(at + k).is_some_and(|c| c.to_ascii_lowercase() == p));
    let prefix = ["https://", "http://", "www."].into_iter().find(|p| starts(p))?.chars().count();
    let mut end = at + prefix;
    while end < chars.len() && !chars[end].is_whitespace() && !matches!(chars[end], '<' | '>' | '"' | '\'') {
        end += 1;
    }
    while end > at + prefix && matches!(chars[end - 1], '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']') {
        end -= 1;
    }
    (end > at + prefix).then_some(end - at)
}

/// How many characters a text shows, its web addresses and spaces left out.
pub fn visible_len(text: &str) -> usize {
    readable(text).chars().filter(|c| !c.is_whitespace()).count()
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

    #[test]
    fn read_without_links_or_padding() {
        // Links go, with their brackets, their sentence's stop staying.
        assert_eq!(readable("Sign in <https://click.example/ls?u=482913&t=1> now."), "Sign in now.");
        assert_eq!(readable("Open https://example.org/a/b. Then (www.example.org/x) here"), "Open . Then here");
        // Invisible padding goes; runs of spaces become one, or one line break.
        assert_eq!(readable("Hello\u{200c}\u{a0}\u{200c}\u{a0}\u{200c}\u{a0}   you\n\n\n   there"), "Hello you\nthere");
        // A single space of any kind stays as it is: digits printed in groups keep it.
        assert_eq!(readable("482\u{202f}913"), "482\u{202f}913");
        assert_eq!(visible_len("  Hi https://example.org/very/long/link  "), 2);
    }
}

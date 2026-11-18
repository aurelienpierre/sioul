// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Titles at the start of a contact's name (Mr, Mme, Dr…), as data
//! (`data/titles.toml`), never as code: what the Contacts page's filter "has a
//! title" and its action "Move the title into its field" find, and what the
//! guess of a name's parts puts into vCard's honorific prefix (RFC 6350
//! §6.2.2, `N`'s fourth part). A title is the name's first word, whatever its
//! case, a full stop after it or not, and only when a name follows it that
//! begins with a letter: "Mme" alone, "Meredith", "Drake" or "Mr & Mrs Smith"
//! hold none. `TITLE`, the job, is another field and is never read here.

use serde::Deserialize;

const TITLES: &str = include_str!("../data/titles.toml");

#[derive(Debug, Deserialize)]
struct Titles {
    #[serde(default)]
    language: Vec<Language>,
}

#[derive(Debug, Deserialize)]
struct Language {
    #[allow(dead_code)]
    code: String,
    words: Vec<String>,
}

/// Every title known, lower case, without its full stop, each once.
fn known() -> &'static [String] {
    static KNOWN: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    KNOWN.get_or_init(|| {
        let titles: Titles = toml::from_str(TITLES).unwrap_or(Titles { language: Vec::new() });
        let mut words: Vec<String> = Vec::new();
        for word in titles.language.iter().flat_map(|l| l.words.iter()) {
            let word = word.trim().trim_end_matches('.').to_lowercase();
            if !word.is_empty() && !words.contains(&word) {
                words.push(word);
            }
        }
        words
    })
}

/// Whether a word is a title, as written ("Mme", "mme.", "DR"): its case and a
/// full stop after it aside.
pub fn is_title(word: &str) -> bool {
    let bare = word.trim().strip_suffix('.').unwrap_or(word.trim()).to_lowercase();
    !bare.is_empty() && known().contains(&bare)
}

/// A name's title and the name after it: "Mme Jeanne Martin" → ("Mme",
/// "Jeanne Martin"), "dr. Paul Exemple" → ("dr.", "Paul Exemple"); the title as
/// written, its full stop kept. None when the name begins with no title, or
/// when nothing that begins with a letter follows it.
pub fn split(name: &str) -> Option<(String, String)> {
    let name = name.trim();
    let end = name.find(char::is_whitespace)?;
    let (first, rest) = (&name[..end], name[end..].trim());
    (is_title(first) && rest.chars().next().is_some_and(char::is_alphabetic)).then(|| (first.to_string(), rest.to_string()))
}

/// Two titles the same, as written or not: "Mme" and "mme.".
pub fn same(a: &str, b: &str) -> bool {
    let bare = |t: &str| t.trim().trim_end_matches('.').to_lowercase();
    !bare(a).is_empty() && bare(a) == bare(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_data_and_well_formed() {
        let titles: Titles = toml::from_str(TITLES).expect("data/titles.toml reads");
        assert_eq!(titles.language.iter().map(|l| l.code.as_str()).collect::<Vec<_>>(), ["fr", "en"]);
        for word in ["M", "Mr", "Mme", "Mlle", "Dr", "Me", "Pr", "Mrs", "Ms", "Miss", "Prof"] {
            assert!(is_title(word) && is_title(&format!("{word}.")) && is_title(&word.to_uppercase()) && is_title(&word.to_lowercase()), "{word}");
        }
        assert!(!is_title("") && !is_title("."));
    }

    #[test]
    fn a_title_found_in_french_and_english_names() {
        let found = |name: &str| split(name).map(|(t, r)| format!("{t}|{r}"));
        assert_eq!(found("Mme Jeanne Martin").as_deref(), Some("Mme|Jeanne Martin"));
        assert_eq!(found("M. Jean Dupont").as_deref(), Some("M.|Jean Dupont"));
        assert_eq!(found("  mlle   Lou Exemple ").as_deref(), Some("mlle|Lou Exemple"));
        assert_eq!(found("Me Camille Exemple").as_deref(), Some("Me|Camille Exemple"));
        assert_eq!(found("PR Sam Exemple").as_deref(), Some("PR|Sam Exemple"));
        assert_eq!(found("Mrs. Jane Example").as_deref(), Some("Mrs.|Jane Example"));
        assert_eq!(found("Ms Alex Example").as_deref(), Some("Ms|Alex Example"));
        assert_eq!(found("Miss Lou Example").as_deref(), Some("Miss|Lou Example"));
        assert_eq!(found("dr. Paul Example").as_deref(), Some("dr.|Paul Example"));
        assert_eq!(found("Prof Kim Example").as_deref(), Some("Prof|Kim Example"));
        // Never inside a word, never alone, never before what is no name.
        for none in ["Meredith Example", "Drake Example", "Mme", "Mme.", "Mrs ", "Mr & Mrs Example", "Mistral Example", "Prune Example", "Jeanne Martin", ""] {
            assert_eq!(split(none), None, "{none}");
        }
        assert!(same("Mme", "mme.") && !same("Mme", "Mlle") && !same("", ""));
    }
}

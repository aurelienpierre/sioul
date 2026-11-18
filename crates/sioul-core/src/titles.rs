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
    #[serde(default)]
    joiners: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Language {
    #[allow(dead_code)]
    code: String,
    words: Vec<String>,
    #[serde(default)]
    unsure: Vec<String>,
}

/// The data, read once: every title, those unsure without their full stop, the joiners; lower case.
struct Known {
    words: Vec<String>,
    unsure: Vec<String>,
    joiners: Vec<String>,
}

fn known() -> &'static Known {
    static KNOWN: std::sync::OnceLock<Known> = std::sync::OnceLock::new();
    KNOWN.get_or_init(|| {
        let titles: Titles = toml::from_str(TITLES).unwrap_or(Titles { language: Vec::new(), joiners: Vec::new() });
        let bare = |list: &mut Vec<String>, word: &str| {
            let word = word.trim().trim_end_matches('.').to_lowercase();
            if !word.is_empty() && !list.contains(&word) {
                list.push(word);
            }
        };
        let (mut words, mut unsure, mut joiners) = (Vec::new(), Vec::new(), Vec::new());
        for language in &titles.language {
            language.words.iter().for_each(|w| bare(&mut words, w));
            language.unsure.iter().for_each(|w| bare(&mut unsure, w));
        }
        titles.joiners.iter().for_each(|w| bare(&mut joiners, w));
        Known { words, unsure, joiners }
    })
}

fn bare(word: &str) -> String {
    word.trim().strip_suffix('.').unwrap_or(word.trim()).to_lowercase()
}

/// Whether a word is a title, as written ("Mme", "mme.", "DR"): its case and a
/// full stop after it aside.
pub fn is_title(word: &str) -> bool {
    let bare = bare(word);
    !bare.is_empty() && known().words.contains(&bare)
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

/// Whether a name is two people's: after its title, a joiner ("et", "and",
/// "&") or another title ("M. et Mme Dupont", "Mr and Mrs Smith", "Mme Jeanne
/// et Paul Martin"). The title's move never changes such a name.
pub fn household(name: &str) -> bool {
    let words: Vec<&str> = name.split_whitespace().collect();
    // Another title, not a middle initial ("Jean M. Dupont").
    words.iter().skip(1).any(|w| known().joiners.contains(&bare(w)) || (is_title(w) && bare(w).chars().count() > 1)) || name.contains('&') || name.contains('+')
}

/// Whether a title found is sure enough to move without asking: written with
/// its full stop ("M.", "Dr."), or a word that is no other ("Mme", "Mrs"),
/// never on a card that looks like a company when it has no full stop.
pub fn sure(title: &str, company: bool) -> bool {
    if title.trim().ends_with('.') {
        return true;
    }
    !company && !known().unsure.contains(&bare(title))
}

/// Two titles the same, as written or not: "Mme" and "mme.".
pub fn same(a: &str, b: &str) -> bool {
    !bare(a).is_empty() && bare(a) == bare(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_data_and_well_formed() {
        let titles: Titles = toml::from_str(TITLES).expect("data/titles.toml reads");
        assert!(titles.joiners.contains(&"et".to_string()), "the joiners at the top of the file, before any [[language]], else they are that language's");
        assert_eq!(titles.language.iter().map(|l| l.code.as_str()).collect::<Vec<_>>(), ["fr", "en"]);
        assert!(titles.language.iter().all(|l| l.unsure.iter().all(|u| l.words.contains(u))), "an unsure word is one of the words");
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

    /// Two people in one name are never taken for one person's title; the
    /// unsure words, and any title without its full stop on a company, are said unsure.
    #[test]
    fn households_and_unsure_titles() {
        for two in ["M. et Mme Dupont", "Mr and Mrs Smith", "Mme Jeanne et Paul Martin", "M & Mme Dupont", "Jeanne + Paul"] {
            assert!(household(two), "{two}");
        }
        assert!(!household("Mme Jeanne Martin") && !household("Dr Paul Exemple") && !household("M. Jean M. Dupont"));
        assert!(sure("Mme", false) && sure("Mrs", false) && sure("M.", false) && sure("Dr", false) && sure("Dr.", true));
        for unsure in ["Me", "Pr", "PR", "M", "Prof", "Docteur"] {
            assert!(!sure(unsure, false), "{unsure}");
        }
        assert!(!sure("Dr", true) && !sure("Miss", true) && !sure("Mr", true), "no full stop, on a company");
        assert!(split("Madame Jeanne Martin").is_some(), "in full too");
    }
}

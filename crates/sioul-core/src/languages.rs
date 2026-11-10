// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The languages found in your mail (docs/words.md, "The languages found in
//! your mail"). Which words Sioul reads depends on the languages read
//! (`[words] languages`): a new configuration gets, once, its interface's
//! language, English, and the languages its mail is written in; later, a
//! language found anew is offered, never switched on by itself.
//!
//! - **Which language a message is in** (`language_of`): its subject and the
//!   start of its text, folded, counted against each shipped pack's most
//!   common words (`[detect] common`, none of them another shipped
//!   language's); the one with three or more and twice the next, else none.
//! - **Found** (`found_in`): over the newest `LAST` messages received (not
//!   spam, not in Junk, Trash, Sent or Drafts), a language counts when at
//!   least `SHARE` of them (5 %) and `AT_LEAST` of them (20) are in it.
//! - **Decided once** (`decide_once`): a configuration without `[words]
//!   languages` (made by a Sioul after 8 October 2026: an older one had
//!   French and English written in at its first start) gets them written,
//!   once `AT_LEAST` messages were read: the interface's, English, then
//!   the languages found, most first.
//! - **Offered** (`to_offer`): a language found that is not read, offered
//!   once on this device (`languages-offered.toml` in Sioul's state folder), whether it
//!   was taken or not: "Some of your mail is in German. Read German too?"

use crate::config::Config;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The messages looked at: the newest received.
pub const LAST: usize = 500;
/// A language counts when this share of those messages is in it…
pub const SHARE: f64 = 0.05;
/// …and at least this many; fewer messages read, nothing is decided yet.
pub const AT_LEAST: usize = 20;

/// Which shipped language `text` is written in, by `lists` ([`crate::words::detection_lists`]):
/// the language with three common words or more and at least twice as many
/// as the next; none when too few or too close.
pub fn language_of(text: &str, lists: &[(String, HashSet<String>)]) -> Option<String> {
    let words: Vec<String> = crate::words::folded(text).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect();
    let mut counts: Vec<(usize, &str)> = lists.iter().map(|(language, common)| (words.iter().filter(|w| common.contains(*w)).count(), language.as_str())).collect();
    counts.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    let best = counts.first()?;
    let next = counts.get(1).map_or(0, |n| n.0);
    (best.0 >= 3 && best.0 >= 2 * next).then(|| best.1.to_string())
}

/// The detection's lists as `language_of` reads them.
pub fn lists() -> Vec<(String, HashSet<String>)> {
    crate::words::detection_lists().into_iter().map(|(language, words)| (language, words.into_iter().collect())).collect()
}

/// What the newest messages are written in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Messages read.
    pub read: usize,
    /// Each language found and how many messages are in it, most first.
    pub languages: Vec<(String, usize)>,
}

impl Found {
    /// The languages enough messages are in: `SHARE` of those read, and `AT_LEAST`.
    pub fn written_in(&self) -> Vec<String> {
        let enough = AT_LEAST.max((self.read as f64 * SHARE).ceil() as usize);
        self.languages.iter().filter(|(_, n)| *n >= enough).map(|(language, _)| language.clone()).collect()
    }
}

/// The languages of `texts` (each a message's subject and text), `LAST` at most.
pub fn found_in(texts: impl IntoIterator<Item = String>, lists: &[(String, HashSet<String>)]) -> Found {
    let mut found = Found::default();
    let mut counts: Vec<(String, usize)> = Vec::new();
    for text in texts.into_iter().take(LAST) {
        found.read += 1;
        if let Some(language) = language_of(&text, lists) {
            match counts.iter_mut().find(|(l, _)| *l == language) {
                Some((_, n)) => *n += 1,
                None => counts.push((language, 1)),
            }
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    found.languages = counts;
    found
}

/// The newest `last` messages received, on every mail account here, as
/// `found_in` reads them: subject and the start of their text. Spam and the
/// Junk, Trash, Sent and Drafts folders are left out: what others write to
/// you, not what spam is written in.
pub fn mail_texts(config: &Config, last: usize) -> Vec<String> {
    use crate::folders::Role;
    let mut lines: Vec<(i64, PathBuf)> = Vec::new();
    for account in config.accounts.iter().filter(|a| a.kind == crate::config::AccountKind::Imap) {
        let root = account.maildir_path();
        let saved = crate::folders::saved(&account.id);
        let locals: Vec<String> = if saved.is_empty() {
            vec![String::new()]
        } else {
            saved.iter().filter(|f| !matches!(f.role, Role::Junk | Role::Trash | Role::Sent | Role::Drafts)).map(|f| f.local.clone()).collect()
        };
        for local in locals {
            for (path, line) in crate::mailcache::folder(&account.id, &root, &local) {
                if !line.spam {
                    lines.push((crate::mailcache::sent_at(&line), path));
                }
            }
        }
    }
    lines.sort_by(|a, b| b.0.cmp(&a.0));
    lines.dedup_by(|a, b| a.1 == b.1);
    lines
        .into_iter()
        .take(last)
        .filter_map(|(_, path)| {
            let raw = crate::filelock::read_whole(&path).ok()?;
            let card = crate::card::Card::from_bytes(&raw)?;
            Some(format!("{}\n{}", card.subject, card.excerpt))
        })
        .collect()
}

/// A new configuration's languages, written once (the docs above): none
/// when its configuration already says which (`[words] languages`), or
/// when fewer than `AT_LEAST` messages were read; else the languages written.
pub fn decide_once(config_path: &Path, config: &Config, found: &Found) -> Result<Option<Vec<String>>, String> {
    if config.words.get("languages").is_some() || found.read < AT_LEAST {
        return Ok(None);
    }
    let shipped = crate::words::available_languages();
    let mut languages = crate::words::languages(config);
    for language in found.written_in() {
        if shipped.contains(&language.as_str()) && !languages.contains(&language) {
            languages.push(language);
        }
    }
    // Only when the file still says nothing of them: the window and the sharing write it too.
    let written = crate::filelock::with_lock(config_path, || {
        let text = std::fs::read_to_string(config_path).unwrap_or_default();
        let doc: toml_edit::DocumentMut = text.parse().map_err(|e| format!("{}: {e}", config_path.display()))?;
        if doc.get("words").and_then(|w| w.get("languages")).is_some() {
            return Ok(false);
        }
        crate::words::write_setting(config_path, "languages", &languages)?;
        Ok::<_, String>(true)
    })?;
    Ok(written.then_some(languages))
}

/// Where this device keeps the languages it offered: `languages-offered.toml` in Sioul's state folder.
pub fn offered_path() -> PathBuf {
    crate::config::state_dir().join("languages-offered.toml")
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Offered {
    #[serde(default)]
    offered: Vec<String>,
}

/// The languages offered on this device before, taken or not.
pub fn offered(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str::<Offered>(&text).ok()).map(|o| o.offered).unwrap_or_default()
}

/// `language` offered: never again on this device.
pub fn mark_offered(path: &Path, language: &str) -> Result<(), String> {
    let mut all = offered(path);
    if all.iter().any(|l| l == language) {
        return Ok(());
    }
    all.push(language.to_string());
    let text = format!("# Sioul: the languages found in your mail and offered on this device, each once (docs/words.md).\n{}", toml::to_string(&Offered { offered: all }).map_err(|e| e.to_string())?);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// The languages to offer now: found, shipped, not read, never offered here.
pub fn to_offer(config: &Config, found: &Found, offered: &[String]) -> Vec<String> {
    let read = crate::words::languages(config);
    let shipped = crate::words::available_languages();
    found.written_in().into_iter().filter(|l| shipped.contains(&l.as_str()) && !read.contains(l) && !offered.contains(l)).collect()
}

/// "Read German too": `language` added to the languages read, as Settings ▸ Words adds it.
pub fn read_too(config_path: &Path, config: &Config, language: &str) -> Result<(), String> {
    let mut languages = crate::words::languages(config);
    if !languages.iter().any(|l| l == language) {
        languages.push(language.to_string());
    }
    crate::words::write_setting(config_path, "languages", &languages)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GERMAN: &str = "Sehr geehrte Damen und Herren, vielen Dank für Ihre Bestellung. Die Rechnung ist beigefügt. Mit freundlichen Grüßen";
    const FRENCH: &str = "Bonjour, votre commande est en route. Vous pouvez suivre le colis sur notre site. Merci et à bientôt.";
    const ENGLISH: &str = "Hello, thank you for your order. It will ship in two days and we will tell you when it is on its way.";
    const SPANISH: &str = "Hola, gracias por su pedido. Le avisaremos cuando esté en camino, como siempre, desde nuestra tienda.";
    const ITALIAN: &str = "Buongiorno, grazie per il suo ordine. Le scriveremo quando sarà spedito, come sempre. Cordiali saluti";

    /// Each shipped language is told by its common words; a few words, a
    /// list of names, two languages as much: none.
    #[test]
    fn a_message_s_language_is_told_by_its_common_words() {
        let lists = lists();
        assert_eq!(lists.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), ["en", "fr", "de", "es", "it"]);
        // None of a language's common words is another's: a word counts once.
        for (i, (a, first)) in lists.iter().enumerate() {
            for (b, second) in &lists[i + 1..] {
                assert!(first.is_disjoint(second), "{a} and {b}: {:?}", first.intersection(second).collect::<Vec<_>>());
            }
        }
        for (text, language) in [(GERMAN, "de"), (FRENCH, "fr"), (ENGLISH, "en"), (SPANISH, "es"), (ITALIAN, "it")] {
            assert_eq!(language_of(text, &lists).as_deref(), Some(language), "{text}");
        }
        assert_eq!(language_of("Rechnung 2026-11", &lists), None, "too few words");
        assert_eq!(language_of("Paris, Berlin, Madrid, Roma", &lists), None);
        assert_eq!(language_of(&format!("{ENGLISH} {FRENCH}"), &lists), None, "as much of each");
    }

    /// 5 % and 20 messages: a language under either is not found; fewer
    /// than 20 read decide nothing.
    #[test]
    fn a_language_counts_from_five_percent_and_twenty_messages() {
        let lists = lists();
        let mail = |french: usize, german: usize, english: usize| -> Vec<String> {
            std::iter::repeat_n(FRENCH.to_string(), french).chain(std::iter::repeat_n(GERMAN.to_string(), german)).chain(std::iter::repeat_n(ENGLISH.to_string(), english)).collect()
        };
        let found = found_in(mail(400, 30, 70), &lists);
        assert_eq!((found.read, found.languages.clone()), (500, vec![("fr".to_string(), 400), ("en".to_string(), 70), ("de".to_string(), 30)]));
        assert_eq!(found.written_in(), ["fr", "en", "de"]);
        assert_eq!(found_in(mail(480, 24, 0), &lists).written_in(), ["fr"], "24 of 500 is under 5 %");
        assert_eq!(found_in(mail(100, 19, 0), &lists).written_in(), ["fr"], "19 is under 20");
        assert_eq!(found_in(mail(700, 0, 0), &lists).read, LAST, "the newest 500 alone");
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-languages-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A new configuration gets its interface's language, English and the
    /// languages found, written once; one that says its languages is never
    /// changed; a language found later is offered once, never switched on.
    #[test]
    fn a_new_configuration_reads_its_mail_s_languages_and_a_new_one_is_offered() {
        let dir = scratch("decide");
        let path = dir.join("config.toml");
        std::fs::write(&path, "language = \"fr\"\n").unwrap();
        let config = Config::load(&path).unwrap();
        let found = |pairs: &[(&str, usize)], read: usize| Found { read, languages: pairs.iter().map(|(l, n)| (l.to_string(), *n)).collect() };
        // Too little mail yet: nothing written.
        assert_eq!(decide_once(&path, &config, &found(&[("de", 10)], 12)).unwrap(), None);
        assert!(!std::fs::read_to_string(&path).unwrap().contains("[words]"));
        // Enough: French (the interface's), English, then German and Spanish found; a language not shipped never.
        let decided = decide_once(&path, &config, &found(&[("fr", 300), ("de", 150), ("es", 30), ("xx", 20)], 500)).unwrap();
        assert_eq!(decided, Some(vec!["fr".to_string(), "en".to_string(), "de".to_string(), "es".to_string()]));
        let config = Config::load(&path).unwrap();
        assert_eq!(crate::words::languages(&config), ["fr", "en", "de", "es"]);
        // Said once: never again, whatever is found.
        assert_eq!(decide_once(&path, &config, &found(&[("it", 300)], 500)).unwrap(), None);
        // Italian found later: offered, never read until taken; offered once on this device.
        let offered_at = dir.join("offered.toml");
        let later = found(&[("fr", 250), ("it", 200), ("de", 50)], 500);
        assert_eq!(to_offer(&config, &later, &offered(&offered_at)), ["it"]);
        assert_eq!(crate::words::languages(&Config::load(&path).unwrap()), ["fr", "en", "de", "es"], "not switched on by itself");
        mark_offered(&offered_at, "it").unwrap();
        mark_offered(&offered_at, "it").unwrap();
        assert_eq!(offered(&offered_at), ["it"]);
        assert!(to_offer(&config, &later, &offered(&offered_at)).is_empty(), "offered once");
        // Taken: read from now on.
        read_too(&path, &config, "it").unwrap();
        assert_eq!(crate::words::languages(&Config::load(&path).unwrap()), ["fr", "en", "de", "es", "it"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

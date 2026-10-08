// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The shield of a public address. A contact address brings work, and also
//! insults; its mail is read first, here, before you see any of it:
//!
//! - **its tone**: calm, rude (swearing, contempt), or hostile (insults aimed
//!   at you, harassment, threats). Hostile mail is set aside without its
//!   sender's words being shown: not the subject, not a line of the text;
//! - **its topic**: work (a job, a mission, a quote), a question about your
//!   software, the press, thanks, or other; work comes first.
//!
//! The reading is made of word lists (`words::ShieldWords`: the languages in
//! use, French and English built in, and your changes), matched on whole
//! words with case and accents ignored; an insult near "you" weighs more,
//! and shouting adds to it. When you allow it, an AI reads the message too
//! (`ai_prompt`), more finely; its answer is kept and takes precedence.

use crate::text::{find_word, fold};
use crate::words::ShieldWords;
use serde::{Deserialize, Serialize};

/// How a message speaks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    #[default]
    Calm,
    /// Swearing or contempt, not aimed at you.
    Rude,
    /// Insults aimed at you, harassment, threats.
    Hostile,
}

/// What a message is about.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Topic {
    /// A job, a mission, a quote, a collaboration.
    Work,
    /// A question about your software or your work.
    Support,
    Press,
    Thanks,
    /// Money given: a donation, sponsoring.
    Donation,
    #[default]
    Other,
}

/// What the shield made of a message.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assessment {
    pub tone: Tone,
    pub topic: Topic,
    /// The words that weighed, to say why, on request.
    #[serde(default)]
    pub words: Vec<String>,
    /// One neutral line on what it asks, when an AI read it.
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub by_ai: bool,
}

/// Where `phrase` appears as whole words in `folded`, every time.
fn positions(folded: &[char], phrase: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = find_word(folded, phrase, from) {
        out.push(at);
        from = at + 1;
    }
    out
}

/// Whether a word saying "you" (`you`) stands within a few words of `at`;
/// an elided one ("t'") anywhere it is written, the word after it joined.
fn near_you(you: &[String], folded: &[char], at: usize) -> bool {
    let start = at.saturating_sub(40);
    let end = (at + 40).min(folded.len());
    let window = &folded[start..end];
    you.iter().any(|w| {
        let elided: Vec<char> = fold(w.trim());
        find_word(window, w, 0).is_some() || (w.trim().ends_with('\'') && !elided.is_empty() && window.windows(elided.len()).any(|p| p == elided.as_slice()))
    })
}

/// Shouting: most letters in capitals, over a real length of text.
fn shouts(text: &str) -> bool {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 40 && letters.iter().filter(|c| c.is_uppercase()).count() * 10 >= letters.len() * 6
}

/// Reads a message's subject and text: its tone and its topic, with the
/// words in use (`words::current`).
pub fn assess(subject: &str, text: &str) -> Assessment {
    assess_with(&crate::words::current().shield, subject, text)
}

/// `assess` with these words. Each word counts once, however many lists
/// (languages) hold it.
pub fn assess_with(lists: &ShieldWords, subject: &str, text: &str) -> Assessment {
    let whole = format!("{subject}\n{text}");
    let folded = fold(&whole);
    let mut score = 0.0f32;
    let mut words = Vec::new();
    for threat in &lists.threats {
        if !positions(&folded, threat).is_empty() {
            score += 3.0;
            words.push(threat.to_string());
        }
    }
    for insult in &lists.insults {
        for at in positions(&folded, insult) {
            score += if near_you(&lists.you, &folded, at) { 3.0 } else { 1.5 };
            if !words.iter().any(|w| w == insult) {
                words.push(insult.to_string());
            }
        }
    }
    let mut rude = 0.0f32;
    for word in &lists.rude {
        let found = positions(&folded, word).len();
        if found > 0 {
            rude += found as f32;
            if !words.iter().any(|w| w == word) {
                words.push(word.to_string());
            }
        }
    }
    if shouts(text) {
        rude += 1.0;
    }
    if whole.matches("!!").count() >= 3 {
        rude += 0.5;
    }
    let tone = if score >= 3.0 || score + rude >= 4.0 {
        Tone::Hostile
    } else if score > 0.0 || rude >= 1.0 {
        Tone::Rude
    } else {
        Tone::Calm
    };
    // The topic most spoken of; work wins a tie, as it matters most.
    let t = &lists.topics;
    let topic = [(Topic::Work, &t.work), (Topic::Support, &t.support), (Topic::Press, &t.press), (Topic::Thanks, &t.thanks), (Topic::Donation, &t.donation)]
        .into_iter()
        .map(|(topic, keys)| (keys.iter().map(|k| positions(&folded, k).len()).sum::<usize>(), topic))
        .filter(|(n, _)| *n > 0)
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map_or(Topic::Other, |(_, t)| t);
    Assessment { tone, topic, words, summary: String::new(), by_ai: false }
}

/// Where an AI's answer about a message is kept: its Message-ID, which its
/// sender writes, and a digest of who sent it and what it says (FNV-1a), so
/// that a message borrowing the ID of a calm one is read again, never taken for it.
pub fn ai_key(card: &crate::card::Card) -> Option<String> {
    let id = crate::mailindex::bare_id(card.message_id.as_deref()?);
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in [card.from_address.as_deref().unwrap_or(""), card.subject.as_str(), card.excerpt.as_str()].iter().flat_map(|part| part.bytes().chain(std::iter::once(0))) {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{id}#{digest:016x}"))
}

/// How a message to a shielded address reads: the AI's answer about this
/// very message when it read it (found under `ai_key`, where
/// `sioul_sync::shield_ai` keeps it), else the word lists'. Every reader goes
/// through here (the Porch, `sioul shield`, the MCP server), so that none
/// looks an answer up another way, misses it, and passes on mail the AI found hostile.
pub fn reading(card: &crate::card::Card, answers: Option<&std::collections::BTreeMap<String, Assessment>>) -> Assessment {
    answers.and_then(|answers| answers.get(&ai_key(card)?)).cloned().unwrap_or_else(|| assess(&card.subject, &card.excerpt))
}

/// What an AI is asked about a message, to answer in one line of JSON.
pub fn ai_prompt(subject: &str, text: &str) -> String {
    let excerpt: String = text.chars().take(4000).collect();
    format!(
        "You screen mail sent to a public contact address, so that its owner, who is vulnerable to harassment, never reads hostile text. \
         Answer with one line of JSON and nothing else: {{\"tone\": \"calm\" | \"rude\" | \"hostile\", \"topic\": \"work\" | \"support\" | \"press\" | \"thanks\" | \"donation\" | \"other\", \"summary\": \"…\"}}. \
         tone: hostile for insults aimed at the reader, harassment, threats, intimidation; rude for swearing or contempt not aimed at the reader; calm otherwise. \
         topic: work for a job, a mission, a quote, a collaboration; support for a question about software. \
         summary: at most 15 neutral words saying what the message asks, in the message's language, without quoting any insult.\n\nSubject: {subject}\n\n{excerpt}"
    )
}

/// An AI's answer, read; None when it is not the JSON asked for.
pub fn read_ai_answer(answer: &str) -> Option<Assessment> {
    #[derive(Deserialize)]
    struct Answer {
        tone: Tone,
        topic: Topic,
        #[serde(default)]
        summary: String,
    }
    let start = answer.find('{')?;
    let end = answer.rfind('}')?;
    // "} … {": no object, and no panic on a range turned around.
    let parsed: Answer = serde_json::from_str(answer.get(start..=end)?).ok()?;
    Some(Assessment { tone: parsed.tone, topic: parsed.topic, words: Vec::new(), summary: parsed.summary.chars().take(200).collect(), by_ai: true })
}

/// What an AI made of shielded mail, one file per account
/// (`$XDG_STATE_HOME/sioul/shield/<account>.toml`): asked once per message,
/// kept under `ai_key`, read with `reading`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiCache {
    #[serde(default)]
    pub messages: std::collections::BTreeMap<String, Assessment>,
}

impl AiCache {
    pub fn folder() -> std::path::PathBuf {
        crate::config::state_dir().join("shield")
    }

    pub fn path(account: &str) -> std::path::PathBuf {
        AiCache::folder().join(format!("{account}.toml"))
    }

    pub fn load(account: &str) -> AiCache {
        std::fs::read_to_string(AiCache::path(account)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Every account's answers together, under `ai_key`.
    pub fn load_all() -> std::collections::BTreeMap<String, Assessment> {
        let Ok(entries) = std::fs::read_dir(AiCache::folder()) else { return Default::default() };
        entries
            .filter_map(Result::ok)
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|t| toml::from_str::<AiCache>(&t).ok())
            .flat_map(|c| c.messages)
            .collect()
    }

    pub fn save(&self, account: &str) -> Result<(), String> {
        let path = AiCache::path(account);
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)?;
        std::fs::rename(&temporary, &path).map_err(fail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_and_topic() {
        let job = assess("Mission de conseil", "Bonjour, nous recrutons un consultant en traitement d'image pour une mission de trois mois. Quel serait votre tarif ?");
        assert_eq!((job.tone, job.topic), (Tone::Calm, Topic::Work));
        let support = assess("The app crashes on export", "Hi, since the last update the app crashes when I export a file. How to fix this bug?");
        assert_eq!((support.tone, support.topic), (Tone::Calm, Topic::Support));
        let rude = assess("Export", "This export module is garbage, what the fuck.");
        assert_eq!(rude.tone, Tone::Rude);
        let insult = assess("Ton logiciel", "T'es qu'un connard prétentieux, ton logiciel est nul.");
        assert_eq!(insult.tone, Tone::Hostile);
        assert!(insult.words.contains(&"connard".to_string()));
        let threat = assess("hey", "I know where you live.");
        assert_eq!(threat.tone, Tone::Hostile);
        // Whole words only: "connard" is not in "connaissance", "con" never counts.
        assert_eq!(assess("Merci", "Merci pour la connaissance partagée, c'est un contact précieux.").tone, Tone::Calm);
        assert_eq!(assess("Merci", "Merci pour vos articles, quelle reconnaissance !").topic, Topic::Thanks);
    }

    /// Your words: a German insult and a topic of your own work, once added.
    #[test]
    fn your_words_are_read() {
        let config: crate::config::Config = toml::from_str("[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n[words.shield.insults]\nadd = [\"Mistkerl\"]\n[words.shield.you]\nadd = [\"du\"]\n[words.shield.topics.support]\nadd = [\"pixelmill\"]\n").unwrap();
        let yours = crate::words::Words::of(&config);
        assert_eq!(assess_with(&yours.shield, "Hallo", "Du bist ein Mistkerl.").tone, Tone::Hostile);
        assert_eq!(assess("Hallo", "Du bist ein Mistkerl.").tone, Tone::Calm, "not in the packs");
        assert_eq!(assess_with(&yours.shield, "pixelmill", "Une question sur pixelmill.").topic, Topic::Support);
        // "t'" said before a verb, as the packs read it.
        assert_eq!(assess("Toi", "T'es qu'un connard.").tone, Tone::Hostile);
    }

    #[test]
    fn an_ai_answer_is_read() {
        let read = read_ai_answer("Here: {\"tone\": \"hostile\", \"topic\": \"other\", \"summary\": \"Complains about the export: \\\"rude\\\"\"}").unwrap();
        assert_eq!((read.tone, read.topic, read.by_ai), (Tone::Hostile, Topic::Other, true));
        assert!(read.summary.starts_with("Complains"));
        assert!(read_ai_answer("no json").is_none());
        assert!(read_ai_answer("} said the model, then {").is_none());
        assert!(ai_prompt("s", "t").contains("Subject: s"));
    }

    /// The AI's answer is kept under `ai_key` and read back under it, by
    /// every reader: never under the bare Message-ID, which its sender writes.
    #[test]
    fn the_ai_answer_is_found_where_it_is_kept() {
        let card = |text: &str| crate::card::Card::from_bytes(format!("From: Someone <someone@hostile.example>\r\nSubject: Words for you\r\nMessage-ID: <h-1@hostile.example>\r\n\r\n{text}\r\n").as_bytes()).unwrap();
        let calm_words = card("Some unkind sentences.");
        assert_eq!(assess(&calm_words.subject, &calm_words.excerpt).tone, Tone::Calm, "the word lists find nothing");
        let hostile = Assessment { tone: Tone::Hostile, by_ai: true, ..Assessment::default() };
        let answers: std::collections::BTreeMap<String, Assessment> = [(ai_key(&calm_words).unwrap(), hostile.clone())].into();
        assert_eq!(reading(&calm_words, Some(&answers)), hostile);
        // Under the bare Message-ID, nothing is kept: an answer kept there would be read for any message borrowing it.
        assert!(!answers.contains_key("h-1@hostile.example"));
        // Another text under the same Message-ID is read again, by the word lists here.
        assert_eq!(reading(&card("Thanks for the export module."), Some(&answers)).tone, Tone::Calm);
        assert!(!reading(&card("Thanks for the export module."), Some(&answers)).by_ai);
        // No answers at all: the word lists.
        assert_eq!(reading(&calm_words, None).tone, Tone::Calm);
    }
}

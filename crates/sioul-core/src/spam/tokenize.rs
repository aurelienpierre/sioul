// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The spam filter's tokenizer, version 2: what a message says, as the words
//! the language model learns on the desktop and the table scores on every
//! device.
//!
//! A port of Virtual Secretary's text pipeline (its `src/core/nlp.py`:
//! `Tokenizer.prefilter`, `normalize_token`, `lemmatize`; `patterns.py`;
//! `language.py`; `utils.py`: `typography_undo`), rewritten for the `regex`
//! crate, which has no look-around: a look-behind or a look-ahead of one
//! character becomes a character matched before or after the part replaced,
//! and the search goes on from the end of that part, so that the character
//! after one match can stand before the next (`Pass::apply`). Training and
//! scoring share this one function: any change to a step changes the words,
//! so it raises `TOKENIZER`, and a table made by another version is refused.
//!
//! The steps, in order:
//! 1. **The text:** the subject, a blank line, the body (HTML mail as its visible text).
//!    A mark your provider put at the start of the subject (`***Potentiel-SPAM***`,
//!    `[SPAM]`, `{Spam?}`) is taken off first (`without_provider_tags`): it is
//!    the provider's verdict, not the sender's words, and a model that read it
//!    would learn to copy the provider (version 2: a provider may tag nearly
//!    every message it sets aside).
//! 2. **Normalization** (`normalize`): invisible characters out (zero-width
//!    spaces and joiners would otherwise split or hide words); NFKC; letters
//!    borrowed from other scripts folded into the Latin ones they mimic, in
//!    words that mix them ("Vіаgrа" with Cyrillic і and а); lowercase (after
//!    NFKC, so that its capitals, "™" → "TM", "𝐕" → "V", come out lowercase);
//!    Virtual Secretary's table (apostrophes, spaces, dashes, quotes,
//!    ligatures, fractions, arrows; accents stripped, every accent, not only
//!    French ones); the symbols its patterns read written in ASCII
//!    (€ → "eur", £ → "gbp", ° → "deg", ⌘ → "cmd"); everything else not ASCII
//!    dropped (Virtual Secretary's choice); whitespace collapsed.
//! 3. **Cleanup** (`clean`): runs of dots, dashes and question marks made one,
//!    a character repeated ten times or more, BB code and base64 blocks
//!    removed, markup brackets dropped (their text kept).
//! 4. **Placeholders** (`placeholders`): Virtual Secretary's 36 meta-tokens,
//!    in its order, with `_IBAN_` and `_PHONE_` after the addresses; each link
//!    also gives its domain (`Tokens::links`).
//! 5. **Splitting** (`split`): words on whitespace and punctuation, a dot or a
//!    comma kept between letters or digits ("2.5", "e.g"), an apostrophe
//!    between letters ("don't", "aujourd'hui"), French elisions split off and
//!    dropped ("l'information" → "information").
//! 6. **Per word** (`word`): placeholders kept; number words → `_NUMBER_`; five
//!    digits or more, or a hexadecimal hash → `_HASH_`; numbers → `_NUMBER_`;
//!    single characters and Virtual Secretary's stop words out; its French and
//!    English suffix stemmer.
//!
//! The words these steps look for (a provider's marks, month and key names,
//! elisions, number words, stop words) are a `Lexicon`: the spam lists of the
//! language packs in use, and your changes (`words::SpamWords`). A table keeps
//! the lexicon it was trained with (`table::Meta::lexicon`) and every device
//! reads each message with it (`tokens_with`), so a change of words counts
//! from the next training, on every device at once; a table made before
//! 8 October 2026 keeps none and reads with the packs built in, French and
//! English, which are the words that made it. A change of words does not
//! raise `TOKENIZER`; a change of a step still does. The modifier keys of a
//! shortcut, the ordinal suffixes, the units and the stemmer stay in code.
//!
//! Where this port departs from Virtual Secretary on purpose, the line says so
//! ("Virtual Secretary …"). Tests: `tests` below, on Virtual Secretary's own
//! inputs (`src/tests/test-patterns.py`).

use crate::words::SpamWords;
use regex::{Captures, Regex};
use std::collections::HashSet;
use std::sync::{Arc, LazyLock, Mutex};

/// The tokenizer's version, stamped in every table (`table.rs`).
pub const TOKENIZER: u32 = 2;

/// What the tokenizer read in a message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tokens {
    /// The words, in order, repeats kept: lowercase ASCII, placeholders in
    /// capitals (`_URL_`, `_PRICE_`…). The language model learns them as one
    /// line, `words.join(" ")`.
    pub words: Vec<String>,
    /// The domain of each link, in order (`registrable`): for the header
    /// features, never for the language model.
    pub links: Vec<String>,
}

/// The words the tokenizer looks for (`words::SpamWords`) and the patterns
/// made from them (see the module).
pub struct Lexicon {
    /// The words, as given.
    pub words: SpamWords,
    provider_tag: Regex,
    passes: Vec<Pass>,
    elisions: HashSet<String>,
    number_words: HashSet<String>,
    stop_words: HashSet<String>,
}

impl std::fmt::Debug for Lexicon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lexicon").field("words", &self.words).finish_non_exhaustive()
    }
}

/// A list's words as the steps write the text (`normalize`: lowercase, no
/// accents, ASCII), without empty ones.
fn normalized(list: &[String]) -> Vec<String> {
    list.iter().map(|w| normalize(w).trim().to_string()).filter(|w| !w.is_empty()).collect()
}

/// An alternation of words, each escaped, the longest first; one that never
/// matches when there is none (an empty alternation would match anywhere).
fn alternatives(words: &[String]) -> String {
    let escaped: Vec<String> = words.iter().map(|w| regex::escape(w)).collect();
    let escaped: Vec<&str> = escaped.iter().map(String::as_str).collect();
    if escaped.is_empty() { r"[^\s\S]".to_string() } else { longest_first(&escaped) }
}

impl Lexicon {
    /// The patterns made from these words.
    pub fn new(words: &SpamWords) -> Lexicon {
        // A provider's mark is read in the subject as written: each word lowercase
        // (the pattern ignores case), and without its accents ("indesirable").
        let mut marks: Vec<String> = Vec::new();
        for mark in words.provider_tags.iter().map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty()) {
            let bare = crate::words::folded(&mark);
            marks.push(mark);
            marks.push(bare);
        }
        let months = alternatives(&normalized(&words.months));
        // Any single letter after a modifier, as Virtual Secretary read it.
        let keys = format!("{}|[a-z]", alternatives(&normalized(&words.keys)));
        Lexicon {
            words: words.clone(),
            provider_tag: provider_tag(&alternatives(&marks)),
            passes: passes(&months, &keys),
            elisions: normalized(&words.elisions).into_iter().collect(),
            number_words: normalized(&words.number_words).into_iter().collect(),
            stop_words: normalized(&words.stop_words).into_iter().collect(),
        }
    }

    /// The packs built in, French and English (`words::Words::builtin`):
    /// the words of every table that keeps none.
    pub fn builtin() -> Arc<Lexicon> {
        static BUILTIN: LazyLock<Arc<Lexicon>> = LazyLock::new(|| Arc::new(Lexicon::new(&crate::words::Words::builtin_ref().spam)));
        Arc::clone(&BUILTIN)
    }

    /// The lexicon of these words, made once: its patterns take a moment, and
    /// a device reads with one table's words at a time.
    pub fn cached(words: &SpamWords) -> Arc<Lexicon> {
        type Cache = Mutex<Vec<(SpamWords, Arc<Lexicon>)>>;
        static CACHE: LazyLock<Cache> = LazyLock::new(|| Mutex::new(Vec::new()));
        let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, found)) = cache.iter().find(|(w, _)| w == words) {
            return Arc::clone(found);
        }
        let made = Arc::new(Lexicon::new(words));
        cache.insert(0, (words.clone(), Arc::clone(&made)));
        cache.truncate(4);
        made
    }

    /// The subject without the marks a provider put at its start, as many as there are.
    pub fn without_provider_tags<'a>(&self, subject: &'a str) -> &'a str {
        let mut rest = subject;
        while let Some(found) = self.provider_tag.find(rest).filter(|m| m.end() > 0) {
            rest = &rest[found.end()..];
        }
        rest
    }
}

/// The words of a message, and the domains its links point to, read with
/// the packs built in (`Lexicon::builtin`).
pub fn tokens(subject: &str, body: &str) -> Tokens {
    tokens_with(&Lexicon::builtin(), subject, body)
}

/// The words of a message, and the domains its links point to, read with
/// `lexicon` (a table's own: `table::Table::lexicon`).
pub fn tokens_with(lexicon: &Lexicon, subject: &str, body: &str) -> Tokens {
    let subject = lexicon.without_provider_tags(subject);
    let text = normalize(&format!("{subject}\n\n{body}"));
    let text = clean(&text);
    let mut links = Vec::new();
    let text = placeholders(lexicon, &text, &mut links);
    let words = split(lexicon, &text).into_iter().filter_map(|w| word(lexicon, w)).collect();
    Tokens { words, links }
}

// 1. The text.

/// A provider's mark, at the start of a subject: one of its words (`marks`,
/// an alternation: spam, junk, pourriel, indésirable) between stars,
/// brackets, braces or parentheses, with what goes with it
/// ("***Potentiel-SPAM***", "[SPAM?]", "{Spam}", "(Indésirable)"), or "SPAM:".
fn provider_tag(marks: &str) -> Regex {
    let word = format!("(?:{marks})");
    Regex::new(&format!(r"(?i)^\s*(?:\*{{2,}}[^*]{{0,40}}?{word}[^*]{{0,40}}?\*{{2,}}|\[[^\]]{{0,40}}?{word}[^\]]{{0,40}}?\]|\{{[^}}]{{0,40}}?{word}[^}}]{{0,40}}?\}}|\([^)]{{0,40}}?{word}[^)]{{0,40}}?\)|{word}\s*:)\s*"))
        .expect("a valid pattern")
}

/// The subject without the marks a provider put at its start, read with the
/// packs built in (`Lexicon::without_provider_tags`).
pub fn without_provider_tags(subject: &str) -> &str {
    Lexicon::builtin().without_provider_tags(subject)
}

// 2. Normalization.

/// A text as the placeholders read it: lowercase ASCII, one space between words.
pub fn normalize(text: &str) -> String {
    let visible: String = text.chars().filter(|c| !invisible(*c)).collect();
    let composed = icu_normalizer::ComposingNormalizerBorrowed::new_nfkc().normalize(&visible);
    let folded = fold_lookalikes(&composed);
    let mut out = String::with_capacity(folded.len());
    for c in folded.chars().flat_map(char::to_lowercase) {
        ascii(c, &mut out);
    }
    out.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}

/// Characters that show nothing: dropped before anything reads words, so that
/// one put inside a word ("V\u{200b}iagra") neither splits nor hides it.
/// Sioul's own (`text::readable`), with the marks that turn text around and
/// the tag and variation characters.
fn invisible(c: char) -> bool {
    matches!(c,
        '\u{00ad}' | '\u{034f}' | '\u{061c}' | '\u{115f}' | '\u{1160}' | '\u{17b4}' | '\u{17b5}' | '\u{180b}'..='\u{180e}'
        | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}'
        | '\u{3164}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}' | '\u{ffa0}' | '\u{e0000}'..='\u{e007f}' | '\u{e0100}'..='\u{e01ef}')
}

/// The Latin letter a Cyrillic, Greek or Armenian one mimics, case kept (the
/// families of `lookalike.rs`, by the letter they pass for).
fn lookalike(c: char) -> Option<char> {
    Some(match c {
        'А' | 'Α' => 'A', 'а' | 'α' => 'a',
        'В' | 'Β' => 'B', 'в' => 'b',
        'С' | 'Ϲ' => 'C', 'с' | 'ϲ' => 'c',
        'ԁ' => 'd', 'Ԁ' => 'D',
        'Е' | 'Ё' | 'Ε' => 'E', 'е' | 'ё' | 'ε' => 'e',
        'Н' | 'Η' | 'Һ' => 'H', 'һ' | 'н' | 'հ' => 'h',
        'І' | 'Ї' | 'Ι' | 'Ӏ' => 'I', 'і' | 'ї' | 'ι' => 'i',
        'Ј' => 'J', 'ј' => 'j',
        'К' | 'Κ' => 'K', 'к' | 'κ' => 'k',
        'ӏ' => 'l',
        'М' | 'Μ' => 'M', 'м' => 'm',
        'Ν' => 'N', 'ո' => 'n',
        'О' | 'Ο' => 'O', 'о' | 'ο' | 'օ' => 'o',
        'Р' | 'Ρ' => 'P', 'р' | 'ρ' => 'p',
        'Ԛ' => 'Q', 'ԛ' => 'q',
        'Ѕ' => 'S', 'ѕ' => 's',
        'Т' | 'Τ' => 'T', 'т' | 'τ' => 't',
        'υ' | 'ս' => 'u',
        'ν' => 'v',
        'Ԝ' => 'W', 'ԝ' | 'ω' => 'w',
        'Х' | 'Χ' => 'X', 'х' | 'χ' => 'x',
        'У' | 'Υ' => 'Y', 'у' => 'y',
        'Ζ' => 'Z',
        _ => return None,
    })
}

/// Lookalikes folded where they hide a Latin word: in a word that holds a
/// Latin letter ("Pаypal"), or made of nothing but lookalikes ("РАУ"). A word
/// of another script stays itself, to be dropped with the rest of it (step 2).
fn fold_lookalikes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word: Vec<char> = Vec::new();
    let flush = |word: &mut Vec<char>, out: &mut String| {
        let latin = word.iter().any(char::is_ascii_alphabetic);
        let all = word.iter().all(|c| c.is_ascii() || lookalike(*c).is_some());
        let fold = (latin || all) && word.iter().any(|c| lookalike(*c).is_some());
        out.extend(word.drain(..).map(|c| if fold { lookalike(c).unwrap_or(c) } else { c }));
    };
    for c in text.chars() {
        if c.is_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// One character in ASCII, as Virtual Secretary's table writes it
/// (`utils.py`, `_unicode_to_ascii`), else its letter without its accent;
/// nothing when it has no ASCII form.
fn ascii(c: char, out: &mut String) {
    let text: &str = match c {
        // Apostrophes.
        '\u{2019}' | '`' | '\u{2018}' | '\u{2bc}' | '\u{b4}' | '\u{2032}' => "'",
        '\u{2033}' => "''",
        // Spaces, and two private-use bullets of PDF fonts.
        '\u{f8b9}' | '\u{f81f}' => " ",
        // Dashes.
        '\u{2010}'..='\u{2015}' | '\u{ff0d}' | '\u{fe63}' => "-",
        // Decorations, as spaces; the multiplication sign as x.
        '↑' | '↵' | '☙' | '❧' | '🔗' | '•' | '©' | '®' | '|' | '¦' | '™' | '·' | '✔' => " ",
        '×' => "x",
        'ƒ' => "f",
        // Ligatures (NFKC already splits most).
        'ĳ' => "ij",
        'œ' | '\u{a7f9}' => "oe",
        'æ' => "ae",
        'ß' => "ss",
        'ø' => "o",
        'đ' | 'ð' => "d",
        'ł' => "l",
        'ı' => "i",
        'þ' => "th",
        '\u{fb00}' => "ff",
        '\u{fb01}' => "fi",
        '\u{fb02}' => "fl",
        '\u{fb03}' => "ffi",
        '\u{fb04}' => "ffl",
        '\u{fb05}' | '\u{fb06}' => "st",
        '\u{2026}' => "...",
        // Quotes.
        '«' | '»' | '“' | '”' | '„' => "\"",
        // Fractions: NFKC writes ½ as 1⁄2, its slash a fraction slash, which
        // Virtual Secretary then dropped ("12"); written as a slash here.
        '\u{2044}' => "/",
        // Arrows.
        '←' | '⇐' => "<-",
        '→' | '⇒' => "->",
        '↔' | '⇔' => "<->",
        // The symbols Virtual Secretary's patterns read, which dropping all
        // that is not ASCII would lose: prices, degrees and "n°", the command key.
        '€' => "eur",
        '£' => "gbp",
        '°' => "deg",
        '⌘' => "cmd",
        c if c.is_ascii() => {
            out.push(c);
            return;
        }
        c if c.is_whitespace() => " ",
        c => {
            if let Some(base) = unaccented(c) {
                out.push(base);
            }
            return;
        }
    };
    out.push_str(text);
}

/// A Latin letter without its accents ("é" → 'e', "ñ" → 'n', "ä" → 'a'):
/// the letter its canonical decomposition starts with, when the rest are
/// accents. Virtual Secretary stripped the French accents only and dropped
/// the other letters ("señor" → "seor").
fn unaccented(c: char) -> Option<char> {
    let mut buffer = [0; 4];
    let decomposed = icu_normalizer::DecomposingNormalizerBorrowed::new_nfd().normalize(c.encode_utf8(&mut buffer));
    let mut chars = decomposed.chars();
    let base = chars.next().filter(char::is_ascii_alphabetic)?;
    chars.all(|m| ('\u{300}'..='\u{36f}').contains(&m)).then_some(base)
}

// 3. Cleanup (Virtual Secretary's `characters_cleanup`, in its order).

static DOTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.{2,}").expect("dots"));
// Virtual Secretary made every "~" a dash too, which hid "~/" paths from the placeholders.
static DASHES_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-{2,}").expect("dashes"));
static QUESTIONS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\?{2,}").expect("questions"));
static BB_IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\[img[a-zA-Z0-9 ="]*?\].*?\[/img\]"#).expect("bb img"));
static BB_QUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\[quote[a-zA-Z0-9 ="]*?\].*?\[/quote\]"#).expect("bb quote"));
static MARKUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\[\{<]([^\n\r]+?)[\]\}>]").expect("markup"));
static BASE_64: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:[A-Za-z0-9\+/]{4}){64,}(?:[A-Za-z0-9\+/]{2}==|[A-Za-z0-9\+/]{3}=)?").expect("base64"));

/// Runs of dots, dashes and question marks made one; a character repeated ten
/// times or more, BB code and base64 blocks removed; markup brackets dropped,
/// their text kept.
fn clean(text: &str) -> String {
    let text = DOTS.replace_all(text, "...");
    let text = DASHES_RUN.replace_all(&text, "-");
    let text = QUESTIONS.replace_all(&text, "?");
    let text = repeated(&text);
    let text = BB_IMG.replace_all(&text, " ");
    let text = BB_QUOTE.replace_all(&text, " ");
    let text = MARKUP.replace_all(&text, " ${1} ");
    let text = BASE_64.replace_all(&text, " ");
    text.into_owned()
}

/// A character repeated ten times or more ("==========", "xxxxxxxxxx") made a
/// space: Virtual Secretary's `(.)\1{9,}`, whose back-reference the `regex` crate lacks.
fn repeated(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let run = chars[i..].iter().take_while(|c| **c == chars[i]).count();
        if run >= 10 {
            out.push(' ');
        } else {
            out.extend(&chars[i..i + run]);
        }
        i += run;
    }
    out
}

// 4. Placeholders.

/// What may stand before a placeholder: the start, a space, an opening
/// bracket or quote (Virtual Secretary's `regex_starter`).
const BEFORE: &str = r#"(?:^|[\s\[\(\{<'"`;>])"#;
/// What may stand after it, end of markup (`regex_stopper`).
const AFTER: &str = r#"(?:$|[\s\]\)\}>'"`;<])"#;
/// What may stand after it, end of a word (`end_of_word`).
const END: &str = r#"(?:$|[\s\]\)\}>'"`;:,\?!\.<])"#;
/// A sign before a number (`regex_algebra`; its ≠ and ± are gone with what is not ASCII).
const SIGN: &str = r"[\+\-=]";
/// A sign, then maybe a space, before a number (`(regex_algebra)? ?`); a sign alone.
const SIGN_SPACE: &str = r"[\+\-=]? ?";
const SIGN_ONLY: &str = r"[\+\-=]?";
/// A number with its separators, as Virtual Secretary's units read it.
const AMOUNT: &str = r"[0-9]+(?:[\.,\-\+/ ][0-9]*)*?";
/// What a file name is made of, and what stands around one.
const NAME: &str = r#"[^#%<>&\*\{\}\\/\?\$!\|="'@\s\x08]"#;
const NOT_NAME: &str = r#"(?:^|[#%<>&\*\{\}\\/\?\$!\|="'@\s\x08])"#;
/// After a file name: the end, a space, or one character then the end or a
/// space (Virtual Secretary's `(?![\.\S]\S)`).
const FILE_END: &str = r"(?:$|\s|\S$|\S\s)";
const PREFIXES: &str = "nano|n|micro|milli|m|centi|c|deci|d|deca|hecto|kilo|k|mega|giga|g";

/// One substitution: the part named `m` is replaced, what is matched around it
/// stays, and the search goes on from the end of `m`.
struct Pass {
    re: Regex,
    with: &'static str,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    /// A link: its host goes to `Tokens::links`.
    Url,
    /// Kept only when its check digits are right.
    Iban,
    /// Kept only with eight to fifteen digits.
    Phone,
}

impl Pass {
    fn new(pattern: &str, with: &'static str) -> Pass {
        Pass::of(pattern, with, Kind::Plain)
    }

    fn of(pattern: &str, with: &'static str, kind: Kind) -> Pass {
        let re = Regex::new(pattern).unwrap_or_else(|e| panic!("{with}: {e}"));
        Pass { re, with, kind }
    }

    fn apply(&self, text: &str, links: &mut Vec<String>) -> String {
        let mut out = String::with_capacity(text.len());
        let (mut last, mut at) = (0, 0);
        while at <= text.len() {
            let Some(caps) = self.re.captures_at(text, at) else { break };
            let Some(m) = caps.name("m") else { break };
            at = if m.end() > at { m.end() } else { at + 1 };
            if !self.keeps(&caps, m.as_str(), links) {
                continue;
            }
            out.push_str(&text[last..m.start()]);
            out.push_str(self.with);
            last = m.end();
        }
        out.push_str(&text[last..]);
        out
    }

    /// Whether a match is replaced; a link's domain is kept on the way.
    fn keeps(&self, caps: &Captures, matched: &str, links: &mut Vec<String>) -> bool {
        match self.kind {
            Kind::Plain => true,
            Kind::Url => {
                if let Some(host) = caps.name("host").or_else(|| caps.name("www")) {
                    links.push(registrable(host.as_str()));
                }
                true
            }
            Kind::Iban => iban_valid(matched),
            Kind::Phone => (8..=15).contains(&matched.chars().filter(char::is_ascii_digit).count()),
        }
    }
}

/// Alternatives longest first: Virtual Secretary's "tif|tiff" took "x.tiff" for "x.tif" and an "f".
fn longest_first(words: &[&str]) -> String {
    let mut sorted: Vec<&str> = words.to_vec();
    sorted.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    sorted.dedup();
    sorted.join("|")
}

fn file(extensions: &[&str], with: &'static str) -> Pass {
    Pass::new(&format!(r"(?i){NOT_NAME}(?P<m>{NAME}+\.(?:{})){FILE_END}", longest_first(extensions)), with)
}

/// A number then a unit, as Virtual Secretary's physical quantities: `head`
/// before the number (a sign, a space), `unit` after it.
fn unit(head: &str, unit: &str, with: &'static str) -> Pass {
    Pass::new(&format!(r"(?i){BEFORE}(?P<m>{head}{AMOUNT} ?{unit}){END}"), with)
}

/// The placeholders' passes, in Virtual Secretary's order; `months` and
/// `keys`, a lexicon's month and key names as alternations.
fn passes(months: &str, keys: &str) -> Vec<Pass> {
    let link = r"[A-Za-z0-9\-_\.~:/\[\]@!\$&'\(\)\*\+,;=%]";
    let host = r"[A-Za-z0-9\-_\.~]+";
    vec![
        // Addresses and handles, before anything splits them at the @.
        Pass::new(r"(?P<m>[0-9A-Za-z_\-\+\.]*@[0-9A-Za-z_\-\+\.]+|user\-?[0-9]+)", " _USER_ "),
        // Links (they hold addresses of servers), and "www." ones written without "//".
        Pass::of(&format!(r"(?i){BEFORE}(?P<m>(?:(?:(?:http|ftp)s?)?:?//(?P<host>{host})|(?P<www>www\.{host}))(?::[0-9]*)?/?{link}*(?:\?{link}*)?(?:\#{link}*)?){END}"), " _URL_ ", Kind::Url),
        Pass::new(&format!(r"(?i){BEFORE}(?P<m>(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){{3}}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)|(?:fe80::)?(?:[0-9a-f]{{1,4}}:){{3,8}}(?::?/?[0-9a-f]{{1,4}})*){AFTER}"), " _IP_ "),
        // Sioul's own: bank accounts and phone numbers, before their digits read as anything else.
        Pass::of(&format!(r"(?i){BEFORE}(?P<m>[a-z]{{2}}[0-9]{{2}}(?: ?[a-z0-9]{{4}}){{2,7}}(?: ?[a-z0-9]{{1,3}})?){END}"), " _IBAN_ ", Kind::Iban),
        Pass::of(&format!(r"{BEFORE}(?P<m>(?:\+|00)[1-9][0-9]{{0,2}}(?:[ \.\-]?\(?[0-9]{{1,4}}\)?){{2,6}}|0[1-9](?:[ \.\-]?[0-9]{{2}}){{4}}|\([0-9]{{3}}\) ?[0-9]{{3}}[ \.\-]?[0-9]{{4}}){END}"), " _PHONE_ ", Kind::Phone),
        // Files, by their extension, before paths.
        file(&["php", "m", "py", "sh", "c", "cxx", "cpp", "h", "hxx", "a", "asm", "awk", "asp", "class", "java", "yml", "yaml", "js", "css", "cl"], " _CODEFILE_ "),
        file(&["db", "sql", "sqlite"], " _DATABASEFILE_ "),
        file(&["bmp", "jpg", "jpeg", "jpe", "jp2", "j2c", "j2k", "jpc", "jpf", "jpx", "png", "ico", "svg", "webp", "heif", "heic", "tif", "tiff", "hdr", "exr", "ppm", "pfm", "nef", "rw2", "cr2", "cr3", "crw", "dng", "raf", "arw", "srf", "sr2", "iiq", "3fr", "dcr", "ari", "pef", "x3f", "erf", "raw", "rwz", "orf"], " _IMAGEFILE_ "),
        file(&["xfc", "kra", "psd", "ai", "indd", "ps", "eps", "pdf", "xlsx", "docx", "pptx", "doc", "xls", "ppt", "odt", "ods", "odp", "odg", "odf", "wpd"], " _DOCUMENTFILE_ "),
        file(&["txt", "md", "html", "xml", "xhtml", "xmp", "json", "tex", "rst", "rtf"], " _TEXTFILE_ "),
        file(&["zip", "gzip", "gz", "tar", "bz", "iso", "rar", "img"], " _ARCHIVEFILE_ "),
        Pass::new(&format!(r"(?i){NOT_NAME}(?P<m>{NAME}+\.(?:{})(?:\.[a-z0-9]+)*)(?:$|[^a-zA-Z])", longest_first(&["so", "exe", "dmg", "appimage", "bin", "run", "apk", "jar", "cmd", "workflow", "action", "autorun", "osx", "app", "vb", "dll", "scr", "rpm", "deb", "distinfo"])), " _BINARYFILE_ "),
        // Dates, then times.
        Pass::new(&format!(r"(?i)(?P<m>(?:[0-9]{{1,2}})? (?:{months})\.?(?: [0-9]{{1,2}})?(?: [0-9]{{2,4}}))(?:$|[^:])"), " _DATE_ "),
        Pass::new(r#"(?:^|[\s\[\(\{<'"`;])(?P<m>[0-9]{1,4}[\-/][0-9]{2}[\-/][0-9]{2,4})(?:$|[\s\]\)\}>'"`;:,\?!\.t])"#, " _DATE_ "),
        Pass::new(r#"(?i)(?:^|[\s\[\(\{<'"`;t])(?P<m>[0-9]{1,2} ?(?:h|:|am|pm) ?(?:[0-9]{2})?(?::[0-9]{2})? ?(?:h|am|pm|z|utc)? ?(?:[\+\-][0-9]{1,4})?)(?:$|[\s\]\)\}>'"`;:,\?!\.])"#, " _TIME_ "),
        // Keyboard shortcuts, before "f4" reads as an aperture.
        Pass::new(&format!(r"(?i){BEFORE}(?P<m>(?:fn|tab|ctrl|shift|alt|altgr|maj|command|cmd|option|menu)(?: ?\+ ?(?:{keys}))+){END}"), " _SHORTCUT_ "),
        // Paths, after dates written with slashes. Virtual Secretary's drive
        // letter was a capital, never found in lowercase text.
        Pass::new(&format!(r"{BEFORE}(?P<m>(?:[a-z]:\\{{1,2}}|\./|~/|/)(?:{NAME}+(?:\\|/)?)+){END}"), " _PATH_ "),
        // Five digits or more, one by one or in groups: one long number, read as a hash per word.
        Pass::new(&format!(r#"(?:^|[\s\['\("])(?P<m>{SIGN}?(?:[0-9]+[\.,/\+\-\s]*){{5,}})(?:$|[\s:;,\?!\]\)'"])"#), "123456789"),
        // Numbers with their units.
        unit(SIGN_SPACE, "(?:ev|il)s?", " _EXPOSURE_ "),
        Pass::new(&format!(r"(?i){BEFORE}(?P<m>1/[0-9]+ ?(?:th|s|sec)){END}"), " _SHUTTERSPEED_ "),
        // Virtual Secretary's alternatives were not grouped: "iso 100" needed no
        // end, "100 iso" no start. Grouped here.
        Pass::new(&format!(r"(?i){BEFORE}(?P<m>(?:iso|asa) ?{AMOUNT}|{AMOUNT} ?(?:iso|asa)s?){END}"), " _SENSIBILITY_ "),
        unit(SIGN_SPACE, r"(?:cd/m2|cd/m\^2|nit|nits)", " _LUMINANCE_ "),
        unit(SIGN_SPACE, "(?:kilo|k|mega|m|giga|g|tera|t|peta|p)?i?(?:b|o|bit|byte)s?", " _FILESIZE_ "),
        unit(SIGN_SPACE, &format!("(?:{PREFIXES})?(?:m|meter|metre|in|inch|inche|ft|foot|feet|'|'')s?"), " _DISTANCE_ "),
        unit(SIGN_SPACE, &format!("(?:{PREFIXES})?(?:g|gram|gramme|lb|pound)s?"), " _WEIGHT_ "),
        unit(SIGN_ONLY, "(?:degc|degree c|celsius|k|degf|kelvin)", " _TEMPERATURE_ "),
        unit(SIGN_SPACE, "(?:deg|degree|degre|rad|radian|sr|steradian)s?", " _ANGLE_ "),
        unit(SIGN_ONLY, &format!("(?:{PREFIXES})?(?:hz|hertz)"), " _FREQUENCY_ "),
        unit(SIGN_SPACE, "%", " _PERCENT_ "),
        unit(SIGN_SPACE, "(?:db|decibel)s?", " _GAIN_ "),
        Pass::new(&format!(r"(?i){BEFORE}(?P<m>f/?[0-9]+\.?[0-9]?)"), " _APERTURE_ "),
        Pass::new(&format!(r"(?i){BEFORE}(?P<m>{SIGN}? ?[0-9]+ ?(?:kilo|k|mega|m|giga|g|tera|t|peta|p)?(?:p|px|pixels|pix)s?){END}"), " _PIXELS_ "),
        // Ordinals: 1st, 2nd, 3e, 1er, 2nde; "n°" (its ° written "deg").
        Pass::new(&format!(r"(?i){BEFORE}(?P<m>[0-9]+(?:st|nd|rd|th|e|er|ere|nde|eme)){END}"), " _ORDINAL_ "),
        Pass::new(r"(?P<m>ndeg ?[0-9]+)", " _ORDINAL_ "),
        // Prices, the currency before or after (€ and £ written "eur" and "gbp").
        Pass::new(&format!(r"(?i){BEFORE}(?P<m>{SIGN}?(?:k?(?:usd|eur|gbp|\$) ?(?:[0-9]+[\.,\+\- ]?)+|(?:[0-9]+[\.,\+\- ]?)+ ?k?(?:usd|eur|gbp|\$))){END}"), " _PRICE_ "),
        Pass::new(r"(?P<m>[0-9]+x[0-9]+)", " _RESOLUTION_ "),
        // Paths without their root, after dates.
        Pass::new(&format!(r"{BEFORE}(?P<m>(?:{NAME}+(?:\\|/)){{2,}}(?:{NAME}+)?){END}"), " _PATH_ "),
        // Dashes inside words, as n-grams are written: "e-mail" → "e_mail".
        Pass::new(r"[0-9A-Za-z_](?P<m>[\-_=]+)[0-9A-Za-z_]", "_"),
    ]
}

/// The placeholders, in Virtual Secretary's order; then its last cleanups: a
/// colon is a space (C++ members), a backslash nothing (LaTeX).
fn placeholders(lexicon: &Lexicon, text: &str, links: &mut Vec<String>) -> String {
    let mut text = text.to_string();
    for pass in &lexicon.passes {
        text = pass.apply(&text, links);
    }
    text.replace(':', " ").replace('\\', "")
}

/// The domain a host belongs to, as registered: its last two labels, or three
/// under a country's own second level ("co.uk", "gouv.fr", "com.au"). An
/// approximation of the Public Suffix List, which Sioul does not carry.
pub fn registrable(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host.parse::<std::net::IpAddr>().is_ok() {
        return host.to_string();
    }
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    let second_level = ["co", "com", "net", "org", "gov", "gouv", "ac", "edu", "ne", "or", "go", "mil", "nom", "ltd", "plc", "sch", "asso"];
    let keep = match labels.as_slice() {
        [.., second, top] if top.len() == 2 && second_level.contains(second) => 3,
        _ => 2,
    };
    labels[labels.len().saturating_sub(keep)..].join(".")
}

/// Whether an IBAN's check digits are right (ISO 13616: the first four
/// characters moved to the end, letters as 10 to 35, the number modulo 97 is 1).
fn iban_valid(text: &str) -> bool {
    let compact: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !(15..=34).contains(&compact.len()) {
        return false;
    }
    let mut rest: u32 = 0;
    for c in compact[4..].iter().chain(&compact[..4]) {
        let Some(value) = c.to_digit(36) else { return false };
        rest = if value < 10 { (rest * 10 + value) % 97 } else { (rest * 100 + value) % 97 };
    }
    rest == 1
}

// 5. Splitting.

fn word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The words of the text: runs of letters, digits and underscores; a dot or a
/// comma between two of them kept ("2.5", "1,000", "e.g"), an apostrophe
/// between two letters ("don't", "aujourd'hui"); French elisions split off
/// and dropped ("l'information", "qu'il": `Lexicon::elisions`).
fn split<'a>(lexicon: &Lexicon, text: &'a str) -> Vec<&'a str> {
    let bytes = text.as_bytes();
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    for i in 0..=bytes.len() {
        let joined = |i: usize| {
            i > 0 && i + 1 < bytes.len() && {
                let (before, here, after) = (bytes[i - 1], bytes[i], bytes[i + 1]);
                match here {
                    b'.' | b',' => word_byte(before) && word_byte(after),
                    b'\'' => before.is_ascii_alphabetic() && after.is_ascii_alphabetic(),
                    _ => false,
                }
            }
        };
        let inside = i < bytes.len() && (word_byte(bytes[i]) || (start.is_some() && joined(i)));
        match (inside, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                words.extend(elided(lexicon, &text[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    words
}

/// A word without its elided articles and pronouns, before a vowel or a
/// mute h (the lexicon's elisions): "l'information" → "information",
/// "qu'aujourd'hui" → "aujourd'hui".
fn elided<'a>(lexicon: &Lexicon, mut word: &'a str) -> Option<&'a str> {
    while let Some((head, rest)) = word.split_once('\'') {
        let vowel = rest.bytes().next().is_some_and(|b| b"aeiouyh".contains(&b));
        if !(vowel && lexicon.elisions.contains(head)) {
            break;
        }
        word = rest;
    }
    (!word.is_empty()).then_some(word)
}

// 6. Per word (`normalize_token`).

/// Every placeholder a word can be.
pub const PLACEHOLDERS: &[&str] = &[
    "_USER_", "_URL_", "_IP_", "_IBAN_", "_PHONE_", "_CODEFILE_", "_DATABASEFILE_", "_IMAGEFILE_", "_DOCUMENTFILE_", "_TEXTFILE_",
    "_ARCHIVEFILE_", "_BINARYFILE_", "_DATE_", "_TIME_", "_SHORTCUT_", "_PATH_", "_EXPOSURE_", "_SHUTTERSPEED_", "_SENSIBILITY_",
    "_LUMINANCE_", "_FILESIZE_", "_DISTANCE_", "_WEIGHT_", "_TEMPERATURE_", "_ANGLE_", "_FREQUENCY_", "_PERCENT_", "_GAIN_",
    "_APERTURE_", "_PIXELS_", "_ORDINAL_", "_PRICE_", "_RESOLUTION_", "_NUMBER_", "_HASH_",
];

// The number words and the stop words are the lexicon's (`Lexicon`): the
// packs' Virtual Secretary `REPLACEMENTS` and stop words (`language.py`), as
// the steps above write them (accents stripped, dashes as underscores, elided
// words without their article: "est" for "c'est"). Single characters are
// left out with every word of one.

/// Virtual Secretary's `HASH_PATTERN_FAST`, `^[0-9a-f]{6,}$`, took words made
/// of the letters a to f ("decade", "facade") for hashes: a digit is asked here.
fn hash(word: &str) -> bool {
    let digits = word.bytes().filter(u8::is_ascii_digit).count();
    (digits == word.len() && digits >= 5) || (word.len() >= 6 && digits > 0 && word.bytes().all(|b| b.is_ascii_hexdigit()))
}

static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    // Virtual Secretary's `NUMBER_PATTERN_FAST`, its groups of thousands repeated ("1.000.000").
    Regex::new(r"^[\+\-=]?(?:0x[0-9a-f]+u?|[0-9]+(?:[\.,][0-9]+)*e[\+\-]?[0-9]+[fu]?|[0-9]+(?:[\.,][0-9]+)*[fu]?)$").expect("number")
});

/// One word as the model learns it, or none (`normalize_token`).
fn word(lexicon: &Lexicon, token: &str) -> Option<String> {
    let token = token.trim_matches(|c: char| "?!#=+-,:;'\"^*./`()[]{}& \n\r\t<>".contains(c));
    if token.is_empty() {
        return None;
    }
    if PLACEHOLDERS.contains(&token) {
        return Some(token.to_string());
    }
    if lexicon.number_words.contains(token) {
        return Some("_NUMBER_".into());
    }
    // Words joined by dashes, written with underscores, are n-grams; others lose
    // the underscores around them (Markdown's _italics_).
    let compound = token.len() > 2 && token[1..token.len() - 1].contains('_');
    let token = if compound { token } else { token.trim_matches('_') };
    if !compound && hash(token) {
        return Some("_HASH_".into());
    }
    if !compound && NUMBER.is_match(token) {
        return Some("_NUMBER_".into());
    }
    if token.len() < 2 || lexicon.stop_words.contains(token) {
        return None;
    }
    let joined = token.replace('_', "");
    let stemmed = stem(&joined);
    (!stemmed.is_empty()).then_some(stemmed)
}

/// Virtual Secretary's French and English suffix stemmer (`lemmatize`), on
/// each run of letters and digits of a word, the dots and apostrophes between
/// them kept: its patterns ended at those as at the word's end. Its first
/// rule, the elisions, is the splitting's (step 5).
pub fn stem(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut run = String::new();
    for c in word.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            run.push(c);
        } else {
            out.push_str(&stem_run(&run));
            run.clear();
            out.push(c);
        }
    }
    out.push_str(&stem_run(&run));
    out
}

fn stem_run(run: &str) -> String {
    if run.is_empty() {
        return String::new();
    }
    let mut w = single_consonants(run);
    // Each rule: the shortest stem it leaves before the suffix, the suffixes it
    // tries (the earliest start first, as a regular expression finds them), what replaces them.
    let rules: &[(usize, &[&str], &str)] = &[
        // Plurals: lenses → lens; the stem must be letters (PLURAL_S).
        (4, &["sees", "ees", "ses", "es", "ss", "s"], ""),
        // British -our: colour → color, not tour.
        (3, &["our"], "or"),
        // -ity, -ite: activity → activ (then -tiv: act), not city.
        (4, &["ity", "ite"], ""),
        // Feminine -e: capitale → capital.
        (4, &["ee", "e"], ""),
        // -trice, -teur, -tor: operateur → operat.
        (4, &["trice", "teur", "tor"], "t"),
        // -ed: managed → manag.
        (4, &["ed"], ""),
        // -ment, -ement: management → manag.
        (4, &["ement", "ment"], ""),
        // -tion, -sion: information → informat (then -at: inform); not action, too short.
        (4, &["tion", "sion"], "*ion"),
        // -ist, -ism: feminist → femin.
        (3, &["ist", "ism"], ""),
        // -at: optimisat → optimis.
        (4, &["at"], ""),
        // -tif, -tiv: actif → act.
        (2, &["tif", "tiv"], "t"),
        // -y → i: apply → appli.
        (3, &["y"], "i"),
        // -er after five: optimizer → optimiz.
        (5, &["er"], ""),
        // -iz, -yz → -is, -ys: optimiz → optimis.
        (4, &["iz", "yz"], "*s"),
        // -eur → -or: serveur → servor.
        (3, &["eur"], "or"),
        // -iqu, -ic → -i: politiqu → politi.
        (3, &["iqu", "ic"], "i"),
    ];
    for (index, (least, suffixes, with)) in rules.iter().enumerate() {
        // The longest suffix that leaves a long enough stem: the earliest match.
        // Plurals need letters before them, not digits (`[a-zA-Z]{4,}`).
        let fits = |suffix: &&&str| {
            let at = w.len().wrapping_sub(suffix.len());
            w.len() >= suffix.len() + least && w.ends_with(**suffix) && (index != 0 || w.as_bytes()[at - least..at].iter().all(u8::is_ascii_alphabetic))
        };
        let Some(suffix) = suffixes.iter().filter(fits).max_by_key(|s| s.len()) else { continue };
        let at = w.len() - suffix.len();
        let replacement = match *with {
            // -tion → -t, -sion → -s; -iz → -is, -yz → -ys: the suffix's first letter stays.
            "*ion" => suffix[..1].to_string(),
            "*s" => format!("{}s", &suffix[..1]),
            other => other.to_string(),
        };
        w.truncate(at);
        w.push_str(&replacement);
    }
    w
}

/// Double consonants made single after the first two letters: commission → comision.
fn single_consonants(run: &str) -> String {
    let bytes = run.as_bytes();
    let mut out = String::with_capacity(run.len());
    let mut i = 0;
    while i < bytes.len() {
        out.push(bytes[i] as char);
        if i >= 2 && i + 1 < bytes.len() && bytes[i] == bytes[i + 1] && b"bcfghjklmnpqrstvwxz".contains(&bytes[i]) {
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The placeholders a text holds once read, in order.
    fn placed(text: &str) -> Vec<String> {
        let mut links = Vec::new();
        placeholders(&Lexicon::builtin(), &clean(&normalize(text)), &mut links).split_whitespace().filter(|w| w.starts_with('_') && w.ends_with('_') && w.len() > 2).map(str::to_string).collect()
    }

    fn words(text: &str) -> Vec<String> {
        tokens("", text).words
    }

    /// A provider's mark is no word of the message; the same words elsewhere stay.
    #[test]
    fn providers_marks_are_taken_off() {
        for (subject, rest) in [
            ("***Potentiel-SPAM*** Votre colis", "Votre colis"),
            ("***Possible-SPAM***Votre colis", "Votre colis"),
            ("**SPAM** [SPAM?] {Spam} Offer", "Offer"),
            ("[POURRIEL] (Indésirable) SPAM: hello", "hello"),
            ("  [Junk mail]   hello", "hello"),
            ("[Ham] hello", "[Ham] hello"),
            ("Re: spam filters [SPAM]", "Re: spam filters [SPAM]"),
            ("Spammers again", "Spammers again"),
            ("", ""),
        ] {
            assert_eq!(without_provider_tags(subject), rest, "{subject}");
        }
        assert_eq!(tokens("***Potentiel-SPAM*** Facture", "").words, tokens("Facture", "").words);
        assert!(tokens("Spam filters", "").words.len() == 2, "a subject about spam keeps its words");
    }

    #[test]
    fn times_as_virtual_secretary_caught_them() {
        // test-patterns.py: "Should be caught".
        for time in ["12h15", "12:15", "12:15:00", "12am", "12 am", "12 h", "12:15:00Z", "12:15:00+01", "12:15:00 UTC+1", "6:45 am", "6:45am", "6 am", "6am", "24 h", "12 h 12", "12:45 UTC+2", "12:45Z", "14:45+01", "14:45:00-02", "15H"] {
            assert_eq!(placed(&format!("- {time}")), ["_TIME_"], "{time}");
        }
        assert_eq!(placed("Added:\n2023-03-21T11:27:45+0000"), ["_DATE_", "_TIME_"]);
        // "Should be ignored", as times; "12 : 01" was caught by Virtual Secretary too (its reference list).
        for other in ["1", "23", "2022-12-01", "1 €"] {
            assert!(!placed(&format!("- {other}")).contains(&"_TIME_".to_string()), "{other}");
        }
        assert_eq!(placed("- 45 $"), ["_PRICE_"]);
    }

    #[test]
    fn links_and_their_domains() {
        // test-patterns.py's URLs, each alone on its line, at the start or after a word.
        let urls = [
            "https://moi.com", "https://moi.com/", "https://moi.com/page", "http://domain.ext/page/subpage/", "http://domain.ext/page/subpage/#stuff",
            "http://domain.ext/page/subpage/?q=x&r=0:1+i", "http://domain.ext/page/subpage/?q=x&r=0:1+i#stuff", "http://domain.ext/page/subpage/#stuff?q=x&r=0:1+i",
            "ftp://ftp.me.com/page", "//meh.photo", "http://localhost:2080",
            "https://web.archive.org/web/20181103230658im_/http://atelier-malopelli.com/wp-content/uploads/2016/03/Universelle-Sol-Re%CC%81-2-.jpg",
        ];
        for url in urls {
            assert_eq!(placed(url), ["_URL_"], "{url}");
            assert_eq!(placed(&format!("Stuff {url} blah")), ["_URL_"], "{url}");
        }
        // Glued to a word, Virtual Secretary left it; in markup, Markdown and brackets, found.
        assert!(placed("Stuffhttps://moi.com blah").is_empty());
        for wrapped in ["Blah <a href=\"https://at.dot.com/page\">Blah</a>.", "Blah [link](https://at.dot.com/page/)", "Meh [https://at.dot.com/page]", "<http://domain.ext/page/subpage/?q=x&r=0:1+i#stuff>"] {
            assert!(placed(wrapped).contains(&"_URL_".to_string()), "{wrapped}");
        }
        let t = tokens("Votre colis", "Suivez-le sur https://track.shop.example.co.uk/x?id=1 ou www.Colis-Express.example/suivi.");
        assert_eq!(t.links, ["example.co.uk", "colis-express.example"]);
        assert_eq!(t.words.iter().filter(|w| *w == "_URL_").count(), 2);
        assert_eq!((registrable("mail.news.example.org"), registrable("impots.gouv.fr"), registrable("192.0.2.7")), ("example.org".into(), "impots.gouv.fr".into(), "192.0.2.7".into()));
    }

    #[test]
    fn addresses_numbers_and_prices() {
        for ip in ["192.168.1.1", "172.0.0.0", "79.241.182.32", "2001:0db8:0000:85a3:0000:0000:ac1f:8001", "2001:db8:0:85a3::ac1f:8001", "2001:41D0:1:2E4e::1"] {
            assert_eq!(placed(ip), ["_IP_"], "{ip}");
        }
        for price in ["13€", "13 €", "13.54 €", "€13.54", "€ 13.54", "50.2k€", "1 000 €", "1,000.54 €", "52,52 €", "+50 €", "-200€", "$45", "£12"] {
            assert_eq!(placed(price), ["_PRICE_"], "{price}");
        }
        for percent in ["20%", "25 %", "26.1 %", "27.5%"] {
            assert_eq!(placed(percent), ["_PERCENT_"], "{percent}");
        }
        assert!(placed("21").is_empty());
        // Hash or number, per word (test-patterns.py's first lines).
        let one = |w: &str| word(&Lexicon::builtin(), w).unwrap_or_default();
        assert_eq!([one("2045"), one("e62fabc2"), one("2.5"), one("4.999.23")], ["_NUMBER_", "_HASH_", "_NUMBER_", "_NUMBER_"]);
        // In a text, five numbers or more in a row are one long number, as in Virtual Secretary.
        assert_eq!(words("2045 e62fabc2 2.5 4.999.23"), ["_NUMBER_", "_HASH_", "_HASH_"]);
        // Five digits or more are a hash; words of the letters a to f are words.
        assert_eq!(words("commande 482913 decade"), ["comand", "_HASH_", "decad"]);
        assert_eq!(placed("ma.mail+tag@example.org ou @pseudo"), ["_USER_", "_USER_"]);
    }

    #[test]
    fn phones_and_bank_accounts() {
        // ARCEP's numbers kept for fiction, written every usual way.
        for phone in ["01 99 00 12 34", "01.99.00.12.34", "0199001234", "+33 1 99 00 12 34", "0033 5 36 49 12 34", "+33 (0)4 65 71 12 34"] {
            assert_eq!(placed(&format!("Appelez le {phone} vite")), ["_PHONE_"], "{phone}");
        }
        // The usual example of a French IBAN: its check digits hold; one digit off, it is no IBAN.
        assert_eq!(placed("IBAN : FR76 3000 6000 0112 3456 7890 189"), ["_IBAN_"]);
        assert_eq!(placed("FR7630006000011234567890189"), ["_IBAN_"]);
        assert!(!placed("FR76 3000 6000 0112 3456 7890 188").contains(&"_IBAN_".to_string()));
    }

    #[test]
    fn units_shortcuts_and_paths() {
        assert_eq!(placed("50mm f/1.8 G"), ["_DISTANCE_", "_APERTURE_"]);
        assert_eq!(placed("1/800s 1/250 sec 1/1000th"), ["_SHUTTERSPEED_", "_SHUTTERSPEED_", "_SHUTTERSPEED_"]);
        for shortcut in ["Shift+Click", "Ctrl+Click", "Shift+Maj+E", "tab + t", "ctrl + tab", "cmd + shift + f1", "⌘+C"] {
            assert_eq!(placed(shortcut), ["_SHORTCUT_"], "{shortcut}");
        }
        for path in [r"C:\\windows\stuff\doc", "~/images/stuff/i", "/home/user/stuff", "./subdir/here"] {
            assert_eq!(placed(path), ["_PATH_"], "{path}");
        }
        // f/8 is an aperture, not a path; a link is a link.
        assert_eq!(placed("f/8"), ["_APERTURE_"]);
        assert_eq!(placed("https://company.com"), ["_URL_"]);
        assert_eq!(placed("n° 12; 20°C; 45°; 1080p; 1920x1080; 3e"), ["_ORDINAL_", "_TEMPERATURE_", "_ANGLE_", "_PIXELS_", "_RESOLUTION_", "_ORDINAL_"]);
        // As in Virtual Secretary, an amount runs over commas and spaces: "12, 20°C" is one temperature.
        assert_eq!(placed("n° 12, 20°C"), ["_TEMPERATURE_"]);
        assert_eq!(placed("facture.pdf, photo.tiff et setup.exe"), ["_DOCUMENTFILE_", "_IMAGEFILE_", "_BINARYFILE_"]);
        assert_eq!(placed("le 12 janvier 2026, le 2026-01-12"), ["_DATE_", "_DATE_"]);
    }

    #[test]
    fn french_elisions_and_contractions() {
        assert_eq!(words("L’information qu’il n’a pas d’autre"), ["inform", "autr"]);
        assert_eq!(words("aujourd'hui, jusqu'à présent"), ["aujourd'hui", "present"]);
        // English contractions stay whole; "it's" is a stop word.
        assert_eq!(words("Don't wait, it's free"), ["don't", "wait", "free"]);
        // Accents go, every one: French, Spanish, German.
        assert_eq!(words("Réponse urgente señor Bestätigung"), ["repons", "urgent", "senor", "bestatigung"]);
    }

    #[test]
    fn invisible_letters_and_lookalikes() {
        // Zero-width characters inside a word neither split nor hide it.
        assert_eq!(words("V\u{200b}i\u{200c}a\u{200d}g\u{2060}r\u{feff}a"), words("Viagra"));
        // Cyrillic and Greek letters passing for Latin ones, in a Latin word, and a word of nothing else.
        assert_eq!(words("Vіаgrа"), words("Viagra"));
        assert_eq!(words("PаyPаl"), words("PayPal"));
        assert_eq!(words("РАУ"), words("PAY"));
        // Mathematical letters and full-width ones read as letters, lowercase.
        assert_eq!(words("𝐕𝐢𝐚𝐠𝐫𝐚 ＦＲＥＥ"), words("viagra free"));
        // A word of another script is no Latin word: dropped with what is not ASCII.
        assert_eq!(words("привет"), Vec::<String>::new());
        // ½ is one half (Virtual Secretary read "12"); ™ the letters TM.
        assert_eq!(words("½ ™"), ["_NUMBER_", "_NUMBER_", "tm"]);
    }

    #[test]
    fn stems_as_virtual_secretary() {
        let stems: Vec<String> = ["lenses", "colour", "activity", "capitale", "operateur", "managed", "management", "action", "commission", "feminist", "actif", "apply", "optimizer", "serveur", "politique", "process", "tour"].iter().map(|w| stem(w)).collect();
        // As its patterns do, not as its comments say: "action" is too short for
        // -tion, and "activity" loses -ity, then -tiv.
        assert_eq!(stems, ["lens", "color", "act", "capital", "oper", "manag", "manag", "action", "comis", "femin", "act", "appli", "optimis", "servor", "politi", "proc", "tour"]);
    }

    #[test]
    fn output_is_ascii_and_cleaned() {
        let t = tokens("RE: Offre exclusive !!!!!!!!!!!!", "Bonjour,\n\n« Gagnez 1 000 € » — cliquez ici : https://promo.example/x\n==========\nLa suite… ????? -- aGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQgaGVsbG8gd29ybGQ=");
        assert!(t.words.iter().all(|w| w.is_ascii() && !w.contains(' ')), "{:?}", t.words);
        assert_eq!(t.words, ["offr", "exclusiv", "bonjor", "gagnez", "_PRICE_", "cliquez", "_URL_", "suit"]);
        assert_eq!(t.links, ["promo.example"]);
    }

    /// A lexicon reads with its own words, written as the text is (accents
    /// off, lowercase); the packs built in do not know them.
    #[test]
    fn a_lexicon_reads_with_its_own_words() {
        let mut words = crate::words::Words::builtin_ref().spam.clone();
        words.provider_tags.push("Verdächtig".into());
        words.stop_words.push("Rechnung".into());
        words.number_words.push("drei".into());
        words.months.push("März".into());
        words.elisions.retain(|e| e != "l");
        let lexicon = Lexicon::new(&words);
        let read = |subject: &str, body: &str| tokens_with(&lexicon, subject, body).words;
        let builtin = |subject: &str, body: &str| tokens(subject, body).words;
        assert_eq!(read("[VERDÄCHTIG] Angebot", ""), read("Angebot", ""));
        assert_eq!(read("(verdachtig) Angebot", ""), read("Angebot", ""));
        assert_ne!(builtin("[VERDÄCHTIG] Angebot", ""), builtin("Angebot", ""));
        assert_eq!(read("", "Rechnung drei"), ["_NUMBER_"]);
        assert_eq!(builtin("", "Rechnung drei").len(), 2);
        assert!(read("", "am 3 März 2026").contains(&"_DATE_".to_string()));
        assert!(!builtin("", "am 3 März 2026").contains(&"_DATE_".to_string()));
        assert_ne!(read("", "l'information"), builtin("", "l'information"));
        assert_eq!(read("[SPAM] Offre", ""), builtin("Offre", ""), "the built-in marks stay");
        // No words at all: nothing taken off, nothing dropped, nothing breaks.
        let empty = Lexicon::new(&SpamWords::default());
        assert_eq!(empty.without_provider_tags("[SPAM] Offre"), "[SPAM] Offre");
        let unknown = tokens_with(&empty, "", "the 3 mars 2026, ctrl+x").words;
        assert!(unknown.contains(&"the".to_string()) && unknown.contains(&"_SHORTCUT_".to_string()) && !unknown.contains(&"_DATE_".to_string()), "{unknown:?}");
        // The same words give the same lexicon, made once.
        assert!(Arc::ptr_eq(&Lexicon::cached(&words), &Lexicon::cached(&words.clone())));
    }
}

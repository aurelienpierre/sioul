// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! One-time codes, passwords, password resets, sign-in links and addresses
//! to confirm, sent by mail.
//!
//! You asked for them a moment ago, on a site, and they expire within
//! minutes or hours: they are the one exception to the admin windows, shown
//! at once and quietly, from automatic addresses too (docs/porch.md, "Right
//! now"). Fake "your code" messages are a common phishing trick: a forged one
//! was set aside before, and one whose sender is not verified comes with a
//! warning. The words it looks for are the `codes` lists of the word packs
//! in use, with your changes (`words::Codes`, docs/words.md).
//!
//! Many sites send these through the same services as their newsletters, with
//! the same headers (`List-Unsubscribe`, `Precedence: bulk`): those headers
//! never decide that a message is not a code (`detect_message`). They only
//! narrow where a code is looked for: in the subject and the top of the text,
//! where a site puts what you asked for, and not in a newsletter's articles,
//! which talk about passwords without handing one over.

use crate::text::{find_word, fold};
use crate::words::Codes;

/// What kind of short-lived secret a message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeKind {
    Code,
    Password,
    PasswordReset,
    SignInLink,
    /// An address or an account to confirm: "confirm your email".
    Confirmation,
}

impl CodeKind {
    /// How long one stays "right now" when its message does not say, in
    /// minutes: codes live 5 to 15 minutes, sign-in links and resets an hour
    /// or two, an address to confirm a day; a temporary password lasts until
    /// you have used it: a week.
    pub fn usual_minutes(self) -> u32 {
        match self {
            CodeKind::Code => 30,
            CodeKind::SignInLink => 60,
            CodeKind::PasswordReset => 120,
            CodeKind::Confirmation => 24 * 60,
            CodeKind::Password => 7 * 24 * 60,
        }
    }
}

/// A short-lived secret found in a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneTimeCode {
    pub kind: CodeKind,
    /// The code itself, digits only or as printed ("G-482913"); none for links and resets.
    pub code: Option<String>,
    /// How long the message says it stays valid.
    pub expires_minutes: Option<u32>,
}

/// The longest a message stays "right now", whatever it says: a week, as a
/// password waiting for its first use. "Valid for 90 days", written by anyone,
/// would otherwise keep it on top for three months.
const LONGEST_MINUTES: u32 = 7 * 24 * 60;

impl OneTimeCode {
    /// How long it stays "right now": what its message says, else what its kind usually lasts.
    pub fn lasts_minutes(&self) -> u32 {
        self.expires_minutes.unwrap_or_else(|| self.kind.usual_minutes()).min(LONGEST_MINUTES)
    }
}

/// Characters of subject and body looked at: codes sit at the top of these messages.
const SCAN_LIMIT: usize = 6000;
/// Characters of a bulk message's text read, its web addresses left out
/// (`detect_message`): the top, where a site puts the code you asked for.
const LEAD: usize = 1000;
/// Characters of the text, after the subject, where a sign-in link's message
/// says what it is about ("Click the link below to sign in").
const INTENT_LEAD: usize = 300;
/// How far around the phrase a code is looked for, in characters.
const AFTER_WINDOW: usize = 160;
const BEFORE_WINDOW: usize = 100;
/// How far after "link" its validity is said, in characters.
const VALIDITY_WINDOW: usize = 80;

/// Finds a short-lived secret in a message, if it carries one. Phrases are
/// compared folded: lowercase, without accents (see `text`).
pub fn detect(words: &Codes, subject: &str, body: &str) -> Option<OneTimeCode> {
    read(words, subject, body, false)
}

/// Finds a short-lived secret in a text read whole, its line breaks kept (a
/// text message, a chat's, a note): what `detect` reads as a body, so that a
/// label at a line's start ("Bring the cake\nCode: 7319") is read as one,
/// which a subject, read on one line, never shows.
pub fn detect_text(words: &Codes, text: &str) -> Option<OneTimeCode> {
    read(words, "", text, false)
}

/// Finds a short-lived secret in a message read from mail. Bulk headers
/// (`Card::is_list`: `List-Unsubscribe`, `List-Id`, `Precedence: bulk`) are
/// how many sites send codes and links too, so they never rule one out: they
/// only narrow the reading to the subject and the top of the text (`LEAD`),
/// and a phrase that hands nothing over by itself (a reset, a link, an
/// address to confirm, a password), when it is not in the subject, counts only
/// beside a line that answers a request ("if you did not ask for this…",
/// `Codes::request`) or a link said to expire. A newsletter's article on
/// passwords is none.
pub fn detect_message(words: &Codes, card: &crate::card::Card) -> Option<OneTimeCode> {
    read(words, &card.subject, &card.excerpt, card.is_list)
}

fn read(w: &Codes, subject: &str, body: &str, bulk: bool) -> Option<OneTimeCode> {
    let subject = crate::text::readable(subject).replace('\n', " ");
    let mut body = crate::text::readable(body);
    if bulk {
        body = body.chars().take(LEAD).collect();
    }
    let text: String = format!("{subject}\n{body}").chars().take(SCAN_LIMIT).collect();
    let original: Vec<char> = text.chars().collect();
    // A phrase broken over two lines is still the phrase.
    let folded: Vec<char> = fold(&text).into_iter().map(|c| if c == '\n' { ' ' } else { c }).collect();
    // Positions before this one are the subject's.
    let in_subject = subject.chars().count();
    let expires_minutes = expiry_minutes(w, &folded);
    let promotion = first_hit(&folded, &w.promo).is_some();
    let secret = |kind, code| Some(OneTimeCode { kind, code, expires_minutes });
    // A code phrase without a code nearby ("never share your security
    // code", a heading) is no code: the next phrase is read, and so on.
    // A code named before its word in its own sentence ("482913 est le code de votre transaction de …")
    // is read before the promotions' gate: a shop's name further on ("… chez Pneus Discount", "Promo …")
    // never undoes it; a promotion in its own sentence does (`not_yours`: "… de votre bon d'achat").
    if let Some(code) = named_after(w, &folded, &codes(&original)) {
        return secret(CodeKind::Code, Some(code));
    }
    if !promotion && let Some(code) = code_in(w, &original, &folded) {
        return secret(CodeKind::Code, Some(code));
    }
    let brief = lasts_briefly(w, &folded, expires_minutes);
    // In bulk mail, a phrase without a code beside it counts in the subject,
    // or beside a cue: "your password is" is also an article's sentence.
    let cued = !bulk || first_hit(&folded, &w.request).is_some() || brief;
    let counts = |hit: Option<(usize, usize)>| hit.is_some_and(|(start, _)| start < in_subject || cued);
    // A password after its label, as a text or a form gives it: "MDP : …", "Password: …".
    let labelled_password = labelled(w, &original, &folded, &w.password_label, true).into_iter().find(|&(_, end)| gives_value(&original, end));
    for (kind, phrases) in [(CodeKind::Password, &w.password), (CodeKind::PasswordReset, &w.reset), (CodeKind::SignInLink, &w.link), (CodeKind::Confirmation, &w.confirm)] {
        let hit = first_hit(&folded, phrases);
        let hit = if kind == CodeKind::Password { [hit, labelled_password].into_iter().flatten().min_by_key(|&(start, _)| start) } else { hit };
        if counts(hit) {
            return secret(kind, None);
        }
    }
    // "Sign in to Medium" above, "this link expires in 15 minutes" below: a
    // link to sign in. A shop's "log in to see your offers" is none.
    let intent = first_hit(&folded, &w.intents).filter(|&(start, _)| start <= in_subject + INTENT_LEAD);
    if intent.is_some() && brief && !promotion {
        return secret(CodeKind::SignInLink, None);
    }
    None
}

/// The longest a link said to expire may last to be one you just asked for:
/// two days ("valid for 48 hours"); a sale's "valid until Sunday" is none.
const BRIEF_MINUTES: u32 = 2 * 24 * 60;

/// Whether a link, its button or a code is said to expire soon or to work
/// once: "This link expires in 15 minutes", "ce lien n'est valable qu'une fois".
fn lasts_briefly(w: &Codes, folded: &[char], expires_minutes: Option<u32>) -> bool {
    let soon = expires_minutes.is_some_and(|m| m <= BRIEF_MINUTES);
    every_hit(folded, &w.link_words).into_iter().any(|(_, end)| {
        let window = &folded[end..(end + VALIDITY_WINDOW).min(folded.len())];
        let said = |words: &[String]| words.iter().any(|w| find_word(window, w, 0).is_some());
        said(&w.once) || (soon && said(&w.validity))
    })
}

/// The earliest of the phrases in the text, as (start, end) positions.
fn first_hit(folded: &[char], phrases: &[String]) -> Option<(usize, usize)> {
    phrases
        .iter()
        .filter_map(|p| find_word(folded, p, 0).map(|start| (start, start + fold(p).len())))
        .min_by_key(|&(start, _)| start)
}

/// Every place of each phrase in the text, as (start, end) positions.
fn every_hit(folded: &[char], phrases: &[String]) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    for phrase in phrases {
        let length = fold(phrase).len();
        let mut from = 0;
        while let Some(start) = find_word(folded, phrase, from) {
            hits.push((start, start + length));
            from = start + 1;
        }
    }
    hits
}

/// The code beside the first phrase, in the text's order, that has one; else
/// a code said to be yours right after it ("123456 is your Instagram code").
fn code_in(w: &Codes, original: &[char], folded: &[char]) -> Option<String> {
    let found = codes(original);
    // Each hit: where, whether it hands its code over right after it (`giving`), whether it names a code (`names`: a
    // phrase of `Codes::code`, after which a year's shape is a code too).
    let mut hits: Vec<(usize, usize, bool, bool)> = every_hit(folded, &w.code).into_iter().map(|(start, end)| (start, end, false, true)).collect();
    hits.extend(every_hit(folded, &w.giving).into_iter().map(|(start, end)| (start, end, true, false)));
    hits.extend(owned_codes(w, original, folded).into_iter().map(|(start, end)| (start, end, true, false)));
    // "Code : 4821", "PIN: 4821": the word for a code as a label, the code after its colon.
    // One token of a code's shape after its colon, nothing more on its line or in its sentence.
    let one_code = |end: usize| found.iter().find(|f| f.start >= end).is_some_and(|f| !f.year && ties(w, &folded[end..f.start], true) && alone_after(original, f.end));
    hits.extend(labelled(w, original, folded, &w.noun, false).into_iter().filter(|&(_, end)| one_code(end)).map(|(start, end)| (start, end, true, false)));
    // "Enter the code below", only in a message about signing in, checking who you are or paying; never
    // before a shop's code ("avec le code EXEMPLE20", "use code SAVE10": `shop_code_after`).
    if first_hit(folded, &w.sign_in).is_some() {
        hits.extend(every_hit(folded, &w.pointing).into_iter().filter(|&(_, end)| !shop_code_after(original, end)).map(|(start, end)| (start, end, false, false)));
    }
    hits.sort_unstable();
    // A code said to be yours right after it is the surest reading: first.
    named_after(w, folded, &found).or_else(|| hits.into_iter().find_map(|(start, end, giving, names)| code_beside(w, folded, &found, (start, end), giving, names)))
}

/// Whether the first token after a pointing phrase, on its line, is a shop's
/// code: neither digits nor any lowercase letter ("EXEMPLE20", "SAVE10",
/// "NOEL"). Digits ("le code 551204"), a colon, the line's end or ordinary
/// words ("le code suivant pour…") are not.
fn shop_code_after(original: &[char], end: usize) -> bool {
    let mut at = end;
    while at < original.len() && on_line(original[at]) {
        at += 1;
    }
    let token: Vec<char> = original[at..].iter().copied().take_while(|c| c.is_alphanumeric() || *c == '-').collect();
    !token.is_empty() && !token.iter().all(char::is_ascii_digit) && !token.iter().any(|c| c.is_lowercase())
}

/// Whether a folded word is one of a list's, the list's words folded too.
fn is_in(list: &[String], folded_word: &str) -> bool {
    list.iter().any(|w| crate::words::folded(w) == folded_word)
}

/// The words of a text, as (start, end) positions: runs of letters and digits,
/// with the hyphens and apostrophes inside them ("sign-in", "d'accès").
fn words(folded: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < folded.len() {
        if !folded[i].is_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < folded.len() && (folded[i].is_alphanumeric() || (matches!(folded[i], '-' | '\'') && folded.get(i + 1).is_some_and(|c| c.is_alphanumeric()))) {
            i += 1;
        }
        out.push((start, i));
    }
    out
}

/// "Your Instagram code", "your Uber login code", "votre code Instagram":
/// the owner, up to two words, "code", and in French a name after it,
/// written with a capital ("Instagram", never "postal"); none of the words
/// one that makes it no secret (`Codes::not_yours`), nothing but spaces between
/// them. As (start, end): what must be followed by the code itself.
fn owned_codes(w: &Codes, original: &[char], folded: &[char]) -> Vec<(usize, usize)> {
    let words = words(folded);
    let text = |(start, end): (usize, usize)| folded[start..end].iter().collect::<String>();
    let mut out = Vec::new();
    for (index, &code) in words.iter().enumerate() {
        if !is_in(&w.noun, &text(code)) {
            continue;
        }
        // Back: the owner one to three words before, only spaces between.
        let owner = (1..=3).filter_map(|back| index.checked_sub(back)).find(|&at| is_in(&w.owners, &text(words[at])));
        let Some(owner) = owner else { continue };
        let between = &words[owner + 1..index];
        // Words and spaces only: "your account. Code: …" is two sentences.
        let spaced = folded[words[owner].1..code.0].iter().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '\''));
        if !spaced || between.iter().any(|&between| is_in(&w.not_yours, &text(between))) {
            continue;
        }
        out.push((words[owner].0, code.1));
        // A name after "code", with its capital: "votre code Instagram : 123456".
        if let Some(&name) = words.get(index + 1)
            && folded[code.1..name.0].iter().all(|c| *c == ' ')
            && original[name.0].is_uppercase()
            && !is_in(&w.not_yours, &text(name))
        {
            out.push((words[owner].0, name.1));
        }
    }
    out
}

/// Spaces on a line: plain, non-breaking, narrow, a tab.
fn on_line(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{a0}' | '\u{202f}' | '\u{2009}')
}

/// Where a label hands a secret over, right before a colon or "=" ("MDP :
/// …", "Code : 4821", "PIN: 4821"): the label at a line's start or after a
/// full stop, a question or an exclamation mark, never after a comma, a dash
/// or a word ("Porte B, code : 4821", "QR-Code: 12345678", "Zip code:" are
/// none); for the word for a code (`!password`), none of the three words
/// before it on its line one that makes it someone else's code
/// (`Codes::not_yours`: "Erreur. Code : 5003", "Colis prêt. Code : 482913"
/// are none). A password's label also after another field of its line,
/// written with a capital ("Identifiant : jdupont Mot de passe : …"), or
/// after its owner, an article before it at most ("Votre mot de passe : …",
/// "La tua password: …"). "your PIN:" is the owner's (`owned_codes`). As
/// (start, end) of the label: what follows its colon is the secret, one
/// token of it (`gives_value`, `alone_after`).
fn labelled(w: &Codes, original: &[char], folded: &[char], labels: &[String], password: bool) -> Vec<(usize, usize)> {
    let words = words(folded);
    let text = |(start, end): (usize, usize)| folded[start..end].iter().collect::<String>();
    let mut out = Vec::new();
    for (start, end) in every_hit(folded, labels) {
        let spaces = folded[end..].iter().take_while(|c| on_line(**c)).count();
        if !matches!(folded.get(end + spaces), Some(':' | '=' | '\u{ff1a}')) {
            continue;
        }
        // What stands before it on its line: nothing, or what ends a sentence.
        let mut back = start;
        while back > 0 && on_line(original[back - 1]) {
            back -= 1;
        }
        let opens = back == 0 || matches!(original[back - 1], '\n' | '\r' | '.' | '!' | '?');
        let line_start = original[..start].iter().rposition(|c| matches!(c, '\n' | '\r')).map_or(0, |at| at + 1);
        // The words before it on its line, nearest last.
        let before: Vec<(usize, usize)> = words.iter().copied().filter(|&(s, e)| s >= line_start && e <= back).collect();
        if !password {
            let theirs = before.iter().rev().take(3).any(|&b| is_in(&w.not_yours, &text(b)));
            if opens && !theirs {
                out.push((start, end));
            }
            continue;
        }
        let field_before = original[line_start..back].iter().any(|c| matches!(c, ':' | '=' | '\u{ff1a}'));
        let word_before = before.last().copied();
        let named = original[start].is_uppercase() && field_before && word_before.is_none_or(|b| !is_in(&w.not_yours, &text(b)));
        // "Votre mot de passe :", "La tua password:": an owner right before it, at a line's start or a
        // sentence's, an article of three letters at most before the owner.
        let sentence: Vec<(usize, usize)> = before.iter().copied().filter(|&(s, _)| !original[s..back].iter().any(|c| matches!(c, '.' | '!' | '?'))).collect();
        let owned = match sentence.as_slice() {
            [owner] => is_in(&w.owners, &text(*owner)),
            [article, owner] => is_in(&w.owners, &text(*owner)) && text(*article).chars().count() <= 3 && !is_in(&w.not_yours, &text(*article)),
            _ => false,
        } && {
            let first = sentence.first().map_or(start, |&(s, _)| s);
            let mut at = first;
            while at > line_start && on_line(original[at - 1]) {
                at -= 1;
            }
            at == line_start || matches!(original[at - 1], '.' | '!' | '?')
        };
        if opens || named || owned {
            out.push((start, end));
        }
    }
    out
}

/// Whether nothing but the end of its line or of its sentence follows a
/// value ending at `end` (a mark, a bracket, a closing quote): one token,
/// so that "Mot de passe : celui choisi à l'inscription" gives none.
fn alone_after(original: &[char], end: usize) -> bool {
    let mut at = end;
    while at < original.len() && on_line(original[at]) {
        at += 1;
    }
    at >= original.len() || matches!(original[at], '\n' | '\r' | '.' | ',' | ';' | '!' | '?' | ')' | ']' | '(' | '»' | '"' | '”' | '|')
}

/// Whether a label's colon is followed by what it gives, on its line or the
/// next: something other than the dots or stars that hide it ("Password: ••••••" gives none).
fn gives_value(original: &[char], end: usize) -> bool {
    let mut at = end;
    while at < original.len() && on_line(original[at]) {
        at += 1;
    }
    if !matches!(original.get(at), Some(':' | '=' | '\u{ff1a}')) {
        return false;
    }
    at += 1;
    let mut breaks = 0;
    while at < original.len() && (on_line(original[at]) || matches!(original[at], '\n' | '\r')) {
        breaks += usize::from(original[at] == '\n');
        at += 1;
    }
    let value: Vec<char> = original[at..].iter().copied().take_while(|c| !c.is_whitespace()).collect();
    breaks <= 1 && !value.is_empty() && !value.iter().all(|c| matches!(c, '*' | '•' | '●' | '∙' | '·' | '.' | '…')) && alone_after(original, at + value.len())
}

/// A code followed by what says it is yours: "123456 is your Instagram code",
/// "482913 est votre code de vérification". Up to three words between "your"
/// and "code", none that makes it no secret. Or what says it is the code of
/// something of yours ("123456 is the code for your transaction of …",
/// "000000 est le code de votre transaction de …", as a bank or a payment
/// service texts it): a phrase of `Codes::owned_after` that ends in no owner
/// ("is the", "est le") counts only when an owner follows the word for a code
/// in its sentence, within `OWNER_AFTER` words, and neither the words between
/// them nor the one the owner names make it no secret: "4821 est le code de
/// la porte", "75011 is the zip code of your town", "1234 est le code de votre
/// casier" are none.
fn named_after(w: &Codes, folded: &[char], found: &[Found]) -> Option<String> {
    found.iter().find_map(|f| {
        let rest = &folded[f.end..(f.end + 160).min(folded.len())];
        let lead = rest.iter().take_while(|c| **c == ' ').count();
        // The longest phrase that follows: "è il tuo" before "è il".
        let owned = w
            .owned_after
            .iter()
            .filter(|p| {
                let p = fold(p);
                rest[lead..].starts_with(&p) && rest.get(lead + p.len()).is_none_or(|c| !c.is_alphanumeric())
            })
            .max_by_key(|p| fold(p).len())?;
        let after = &rest[lead + fold(owned).len()..];
        let words = words(after);
        let text = |(start, end): (usize, usize)| after[start..end].iter().collect::<String>();
        let code = words.iter().take(4).position(|&word| is_in(&w.noun, &text(word)))?;
        let clean = words[..code].iter().all(|&word| !is_in(&w.not_yours, &text(word)));
        let spaced = after[..words[code].0].iter().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '\''));
        // "Is your": the phrase says whose it is. "Is the": an owner must follow in its sentence.
        let says_whose = crate::words::folded(owned).split_whitespace().last().is_some_and(|last| is_in(&w.owners, last));
        (clean && spaced && (says_whose || owner_follows(w, after, &words, code))).then(|| f.code.clone())
    })
}

/// How many words after the word for a code its owner may come ("the code
/// for your transaction", "le code pour valider votre paiement").
const OWNER_AFTER: usize = 6;

/// Whether an owner ("your", "votre") follows the word for a code at
/// `words[code]` in its sentence, within `OWNER_AFTER` words, with no word
/// between them nor the one after the owner that makes it no secret
/// (`Codes::not_yours`): "the code for your transaction" is yours, "the zip
/// code of your town" and "the code of your locker" are none.
fn owner_follows(w: &Codes, text: &[char], words: &[(usize, usize)], code: usize) -> bool {
    let word = |(start, end): (usize, usize)| text[start..end].iter().collect::<String>();
    let mut between = Vec::new();
    for (i, &(start, end)) in words.iter().enumerate().skip(code + 1).take(OWNER_AFTER) {
        // The sentence ends: a full stop, a question or an exclamation, a line.
        let previous_end = words[i - 1].1;
        if text[previous_end..start].iter().any(|c| matches!(c, '.' | '!' | '?' | '\n' | ';')) {
            return false;
        }
        let this = word((start, end));
        if is_in(&w.not_yours, &this) {
            return false;
        }
        if is_in(&w.owners, &this) {
            let named = words.get(i + 1).filter(|&&(next, _)| !text[end..next].iter().any(|c| matches!(c, '.' | '!' | '?' | '\n' | ';')));
            return named.is_none_or(|&next| !is_in(&w.not_yours, &word(next))) && between.iter().all(|b: &String| !is_in(&w.not_yours, b));
        }
        between.push(this);
    }
    false
}

/// The code next to a phrase: right after it, whatever its shape ("votre code
/// est : 482 913", "your sign-in code: K7Q-2M9"); else, for a phrase of
/// `Codes::code`, digits further after it, or the nearest before it
/// ("G-482913 is your Google verification code"). A year's shape ("2024")
/// counts only right after a phrase that names a code (`names`: "votre code
/// de sécurité est 2024"), never further, before, after a label or "your
/// code" alone ("Votre code : 2026" stays none).
fn code_beside(w: &Codes, folded: &[char], found: &[Found], (start, end): (usize, usize), giving: bool, names: bool) -> Option<String> {
    if names
        && let Some(next) = found.iter().find(|f| f.start >= end)
        && next.year
        && ties(w, &folded[end..next.start], giving)
    {
        return Some(next.code.clone());
    }
    if let Some(next) = found.iter().filter(|f| !f.year).find(|f| f.start >= end)
        && ties(w, &folded[end..next.start], giving)
    {
        return Some(next.code.clone());
    }
    if giving {
        return None;
    }
    let plain = || found.iter().filter(|f| !f.mixed && !f.year);
    plain()
        .find(|f| f.start >= end && f.start < end + AFTER_WINDOW)
        .or_else(|| plain().filter(|f| f.start + BEFORE_WINDOW >= start && f.end <= start).last())
        .map(|f| f.code.clone())
}

/// Whether what stands between a phrase and a code ties them: spaces, a
/// colon, "is" or "est", or a linking phrase of the list ("est le": "votre
/// code est le 482913"). After "your code", a colon or one of these words
/// at least: "votre code postal : 54390" gives no code.
fn ties(w: &Codes, gap: &[char], giving: bool) -> bool {
    let gap: String = gap.iter().collect();
    let words: Vec<&str> = gap.split(|c: char| c.is_whitespace() || c == ':' || c == '=').filter(|word| !word.is_empty()).collect();
    let linked = gap.contains([':', '=']) || !words.is_empty();
    (words.is_empty() || is_in(&w.linking, &words.join(" "))) && (linked || !giving)
}

/// A code-shaped stretch of the text: where it is, the code as kept, and
/// whether it mixes letters and digits (then read only right after its phrase).
struct Found {
    start: usize,
    end: usize,
    code: String,
    mixed: bool,
    /// Four digits that read as a year (1900 to 2099): a code only where a
    /// phrase names it right beside it (`named_after`, `code_beside`), never
    /// elsewhere (review of 25 November 2026, should-fix 3).
    year: bool,
}

/// Every code-shaped stretch of the text, in order. Groups of digits of the
/// same size one space apart, a non-breaking one too ("482 913", "48 29 13"),
/// are read together, a code or none: a phone number or an amount is read
/// whole, never in parts. Groups of other sizes are read one by one.
fn codes(chars: &[char]) -> Vec<Found> {
    let tokens = tokens(chars);
    let mut found = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let mut j = i + 1;
        if is_digits(&tokens[i].text) {
            while j < tokens.len() && is_digits(&tokens[j].text) && tokens[j].start == tokens[j - 1].end + 1 && is_space(chars[tokens[j - 1].end]) {
                j += 1;
            }
        }
        let run = &tokens[i..j];
        if run.len() > 1 && run.iter().all(|t| t.text.len() == run[0].text.len()) {
            found.extend(grouped_code(run).map(|code| Found { start: run[0].start, end: run[run.len() - 1].end, code, mixed: false, year: false }));
        } else {
            for token in run {
                let code = code_from(token).map(|c| (c, false)).or_else(|| mixed_code(&token.text).map(|c| (c, true)));
                if let Some((code, mixed)) = code {
                    found.push(Found { start: token.start, end: token.end, code, mixed, year: false });
                } else if year_alone(token) {
                    found.push(Found { start: token.start, end: token.end, code: token.text.clone(), mixed: false, year: true });
                }
            }
        }
        i = j;
    }
    found
}

fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

/// A space between groups of digits: plain, non-breaking, narrow, thin or figure.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}' | '\u{2007}')
}

/// A token and what surrounds it, enough to tell a code from a price or a phone number.
struct Token {
    text: String,
    /// Where it starts and ends in the text, in characters.
    start: usize,
    end: usize,
    before: char,
    after: String,
}

/// Maximal runs of letters, digits and hyphens.
fn tokens(chars: &[char]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !(chars[i].is_ascii_alphanumeric()) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
            i += 1;
        }
        let text: String = chars[start..i].iter().collect::<String>().trim_end_matches('-').to_string();
        let before = if start > 0 { chars[start - 1] } else { ' ' };
        let after: String = chars[i..(i + 4).min(chars.len())].iter().collect();
        tokens.push(Token { end: start + text.len(), text, start, before, after });
    }
    tokens
}

/// Whether a token alone is a code: 4 to 8 digits (not a year, a price or a
/// phone number), a prefixed code ("G-482913"), or two groups of three or
/// four digits joined by a hyphen ("123-456"), kept as digits.
fn code_from(token: &Token) -> Option<String> {
    let t = &token.text;
    if is_digits(t) && (4..=8).contains(&t.len()) && !looks_like_amount_or_year(token) {
        return Some(t.clone());
    }
    prefixed_code(t).or_else(|| hyphened_code(token))
}

/// "123-456", "4829-1357": two groups of digits of the same size, three or
/// four, joined by a hyphen; a date ("2026-10") or a phone number is none.
fn hyphened_code(token: &Token) -> Option<String> {
    let (first, second) = token.text.split_once('-')?;
    let ok = is_digits(first)
        && is_digits(second)
        && first.len() == second.len()
        && (3..=4).contains(&first.len())
        && !is_year(first)
        && !is_year(second)
        && !looks_like_amount_or_year(token);
    ok.then(|| format!("{first}{second}"))
}

/// Digits printed in groups of the same size, 4 to 8 in all: "482 913",
/// "48 29 13", "4 8 2 9 1 3", "4829 1357". A phone number ("06 12 34 56 78"),
/// a card's number or an amount ("120 450 €") is none.
fn grouped_code(run: &[Token]) -> Option<String> {
    let (first, last) = (&run[0], &run[run.len() - 1]);
    let joined: String = run.iter().map(|t| t.text.as_str()).collect();
    let ok = (1..=4).contains(&first.text.len())
        && (4..=8).contains(&joined.len())
        && !run.iter().any(|t| is_year(&t.text))
        && first.before != '+'
        && !amount_follows(last);
    ok.then_some(joined)
}

/// A code of letters and digits ("K7Q-2M9", "AB12CD", "F7G2K"): 5 to 10
/// characters, uppercase, two letters and two digits at least, so that no
/// word is one; read only right after its phrase (`code_beside`).
fn mixed_code(text: &str) -> Option<String> {
    let letters = text.chars().filter(char::is_ascii_uppercase).count();
    let digits = text.chars().filter(char::is_ascii_digit).count();
    let shaped = text.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-') && !text.contains("--");
    (shaped && (5..=10).contains(&text.len()) && letters >= 2 && digits >= 2).then(|| text.to_string())
}

fn looks_like_amount_or_year(token: &Token) -> bool {
    is_year(&token.text) || token.before == '+' || matches!(token.before, '€' | '$' | '£') || amount_follows(token) || decimals_follow(token)
}

/// ",56" or ".56" right after: the whole part of an amount ("1234,56 EUR").
fn decimals_follow(token: &Token) -> bool {
    let mut after = token.after.chars();
    matches!(after.next(), Some(',' | '.')) && after.next().is_some_and(|c| c.is_ascii_digit())
}

/// Four digits that read as a year, standing alone: not part of a date
/// ("24/11/2026", "2026-11-24", "24.11.2026"), an amount or a number.
fn year_alone(token: &Token) -> bool {
    let after = token.after.chars().next();
    is_year(&token.text)
        && !matches!(token.before, '/' | '.' | '-' | '+' | '€' | '$' | '£')
        && !matches!(after, Some('/' | '-'))
        && !amount_follows(token)
        && !decimals_follow(token)
}

fn is_year(text: &str) -> bool {
    text.len() == 4 && text.parse::<u32>().is_ok_and(|y| (1900..=2099).contains(&y))
}

/// "€", "%" or "euros" right after: an amount.
fn amount_follows(token: &Token) -> bool {
    let after = token.after.trim_start().to_ascii_lowercase();
    after.starts_with('€') || after.starts_with('%') || after.starts_with("eur")
}

/// "G-482913": letters, a hyphen, 4 to 8 digits.
fn prefixed_code(text: &str) -> Option<String> {
    let (prefix, digits) = text.split_once('-')?;
    let ok = (1..=3).contains(&prefix.len())
        && prefix.chars().all(|c| c.is_ascii_uppercase())
        && (4..=8).contains(&digits.len())
        && digits.chars().all(|c| c.is_ascii_digit());
    ok.then(|| text.to_string())
}

/// "valable 10 minutes", "expires in 15 minutes", "valid for 1 hour", "for 7 days".
fn expiry_minutes(w: &Codes, folded: &[char]) -> Option<u32> {
    let starts = |unit: &str, list: &[String]| list.iter().map(|u| crate::words::folded(u)).any(|u| !u.is_empty() && unit.starts_with(&u));
    let mut i = 0;
    while i < folded.len() {
        if !folded[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < folded.len() && folded[i].is_ascii_digit() {
            i += 1;
        }
        let number: u32 = folded[start..i].iter().collect::<String>().parse().unwrap_or(0);
        let unit: String = folded[i..(i + 8).min(folded.len())].iter().collect::<String>().trim_start().to_string();
        let before: String = folded[start.saturating_sub(40)..start].iter().collect();
        let in_context = w.expiry_context.iter().map(|c| crate::words::folded(c)).any(|c| !c.is_empty() && before.contains(&c));
        let minutes = if starts(&unit, &w.unit_minutes) {
            Some(number)
        } else if starts(&unit, &w.unit_hours) {
            Some(number.saturating_mul(60))
        } else if starts(&unit, &w.unit_days) {
            Some(number.saturating_mul(24 * 60))
        } else {
            None
        };
        if let (true, Some(m)) = (in_context, minutes) {
            return Some(m);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::words::Words;

    /// The detectors with the packs built in (French and English).
    fn detect(subject: &str, body: &str) -> Option<OneTimeCode> {
        super::detect(&Words::builtin().codes, subject, body)
    }

    fn detect_message(card: &crate::card::Card) -> Option<OneTimeCode> {
        super::detect_message(&Words::builtin().codes, card)
    }

    /// The words come from the configuration: one added is read, one taken away is not.
    #[test]
    fn the_words_you_add_and_take_away() {
        let config: crate::config::Config = toml::from_str(
            "[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n[words.codes.code]\nadd = [\"Bestätigungscode\"]\nremove = [\"code de connexion\"]\n",
        )
        .unwrap();
        let yours = Words::of(&config);
        let german = ("Ihr Bestätigungscode", "Bestätigungscode: 482913");
        assert_eq!(detect(german.0, german.1), None, "not with the packs built in");
        assert_eq!(super::detect(&yours.codes, german.0, german.1).and_then(|c| c.code).as_deref(), Some("482913"));
        let french = ("Connexion", "Votre code de connexion : 551204");
        assert_eq!(detect(french.0, french.1).and_then(|c| c.code).as_deref(), Some("551204"));
        assert_eq!(super::detect(&yours.codes, french.0, french.1), None, "taken away");
    }

    #[test]
    fn french_bank_code() {
        let found = detect(
            "Votre code de sécurité",
            "Bonjour,\nVotre code de sécurité est : 482 913.\nIl est valable 10 minutes.",
        )
        .unwrap();
        assert_eq!(found.kind, CodeKind::Code);
        assert_eq!(found.code.as_deref(), Some("482913"));
        assert_eq!(found.expires_minutes, Some(10));
    }

    #[test]
    fn code_before_the_phrase() {
        let found = detect("G-482913 is your Google verification code", "").unwrap();
        assert_eq!(found.code.as_deref(), Some("G-482913"));
        let found = detect("123456 est votre code de vérification", "Ce code expire dans 5 min.").unwrap();
        assert_eq!(found.code.as_deref(), Some("123456"));
        assert_eq!(found.expires_minutes, Some(5));
    }

    #[test]
    fn years_prices_and_postcodes_are_not_codes() {
        let found = detect("Your verification code", "Your verification code for 2026: 4821").unwrap();
        assert_eq!(found.code.as_deref(), Some("4821"));
        assert!(detect("Votre code postal", "Votre nouveau code postal : 54390").is_none());
        assert!(detect("Security", "Never share your security code with anyone.").is_none());
    }

    #[test]
    fn what_you_just_asked_for() {
        let found = detect("Welcome", "Here is your password: Xk82-pq. Change it after you sign in.").unwrap();
        assert_eq!((found.kind, found.lasts_minutes()), (CodeKind::Password, 7 * 24 * 60));
        let found = detect("Confirmez votre adresse e-mail", "Cliquez sur le lien ci-dessous.").unwrap();
        assert_eq!((found.kind, found.lasts_minutes()), (CodeKind::Confirmation, 24 * 60));
        let found = detect("Forgot your password?", "Choose a new password with this link. It is valid for 2 days.").unwrap();
        assert_eq!((found.kind, found.lasts_minutes()), (CodeKind::PasswordReset, 2 * 24 * 60));
        let found = detect("Your 2FA code", "Use 551 204 to sign in.").unwrap();
        assert_eq!(found.code.as_deref(), Some("551204"));
        // A shop's code is not yours to rush for.
        assert!(detect("Your access code", "Use access code 2468 for 20 % off this week.").is_none());
    }

    #[test]
    fn notices_that_name_a_password_carry_none() {
        assert!(detect("Nouveau document", "Un document est disponible. Connectez-vous à votre espace avec vos identifiants.").is_none());
        assert!(detect("Mot de passe modifié", "Votre nouveau mot de passe a bien été enregistré.").is_none());
        assert!(detect("Your statement", "Sign in with your login details to read it.").is_none());
        let found = detect("Bienvenue", "Vos identifiants\u{a0}: jdupont / Xk82pq").unwrap();
        assert_eq!(found.kind, CodeKind::Password);
        // Whatever the message says, a week at most on top.
        let long = detect("Confirm your email", "This link is valid for 90 days.").unwrap();
        assert_eq!((long.expires_minutes, long.lasts_minutes()), (Some(90 * 24 * 60), 7 * 24 * 60));
    }

    #[test]
    fn the_code_after_a_warning_or_a_heading() {
        // The first phrase has no code nearby: the next ones are read.
        let filler = "We will never ask for it by phone or by mail. ".repeat(4);
        let body = format!("Never share your security code with anyone.\n{filler}\nYour security code: 482913");
        assert_eq!(detect("Security", &body).unwrap().code.as_deref(), Some("482913"));
        let body = format!("Code de vérification\n{filler}\nVotre code de vérification est : 551204");
        assert_eq!(detect("Votre code de vérification", &body).unwrap().code.as_deref(), Some("551204"));
    }

    #[test]
    fn codes_printed_in_groups() {
        for (body, code) in [
            ("Votre code de vérification : 48 29 13.", "482913"),
            ("Your verification code is 482\u{a0}913.", "482913"),
            ("Votre code de sécurité\u{202f}: 482\u{202f}913", "482913"),
            ("Your verification code: 4 8 2 9 1 3", "482913"),
            ("Votre code : 482913", "482913"),
        ] {
            assert_eq!(detect("Code", body).and_then(|c| c.code).as_deref(), Some(code), "{body}");
        }
        // A phone number, an amount, a year: no code, whole or in part.
        assert!(detect("Votre code de vérification", "Il vous sera envoyé au 06 12 34 56 78.").is_none());
        assert!(detect("Your verification code", "It goes to +33 6 12 34 56 78 by text.").is_none());
        assert!(detect("Votre code de vérification", "Montant : 120 450 €").is_none());
        assert!(detect("Bienvenue", "Votre code : 2026").is_none());
    }

    #[test]
    fn codes_of_letters_and_digits_only_where_given() {
        assert_eq!(detect("Your sign-in code", "Your sign-in code is K7Q-2M9.").unwrap().code.as_deref(), Some("K7Q-2M9"));
        assert_eq!(detect("Bienvenue", "Votre code\u{a0}: AB12CD").unwrap().code.as_deref(), Some("AB12CD"));
        // Words, an order number, a postcode, a shop's code: none.
        assert!(detect("Your code", "Your code is ready: open the app.").is_none());
        assert!(detect("Your code", "Your code is ab12cd").is_none());
        assert!(detect("Votre commande FR000000", "Votre code de confirmation vous sera envoyé par SMS.\nCommande FR000000 : 2 articles.").is_none());
        assert!(detect("Votre code postal", "Votre code postal : AB12CD").is_none());
        assert!(detect("Votre code", "Votre code : SPRING20 pour 20 % de réduction.").is_none());
    }

    #[test]
    fn resets_and_links() {
        let found = detect("Réinitialisation de votre mot de passe", "Cliquez sur le lien.").unwrap();
        assert_eq!(found.kind, CodeKind::PasswordReset);
        let found = detect("Your sign-in link", "Valid for 1 hour.").unwrap();
        assert_eq!(found.kind, CodeKind::SignInLink);
        assert_eq!(found.expires_minutes, Some(60));
    }

    /// A message as a site's mailing service sends it: its bulk headers, then the text.
    fn bulk(subject: &str, body: &str) -> crate::card::Card {
        let raw = format!(
            "From: Service <no-reply@service.example>\r\nTo: you@example.org\r\nSubject: {subject}\r\n\
             List-Unsubscribe: <https://mail.service.example/u/a1b2c3>, <mailto:unsubscribe@service.example>\r\n\
             List-Unsubscribe-Post: List-Unsubscribe=One-Click\r\nPrecedence: bulk\r\nX-Mailer: Mailing Service 4.2\r\n\
             Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{body}\r\n"
        );
        let card = crate::card::Card::from_bytes(raw.as_bytes()).unwrap();
        assert!(card.is_list);
        card
    }

    #[test]
    fn codes_and_links_sent_with_bulk_headers() {
        // A sign-in link, worded as newsletter services word them.
        let link = bulk(
            "Sign in to Readwell",
            "Hi,\n\nClick the link below to sign in to your Readwell account.\n\nSign in to Readwell <https://click.service.example/ls/click?upn=u001.Zm9vYmFyYmF6cXV4-2FZm9vYmFyYmF6cXV4-2B&token=4829137715>\n\n\
             This link will expire in 15 minutes and can only be used once. If you didn't request this, you can safely ignore this email.",
        );
        let found = detect_message(&link).unwrap();
        assert_eq!((found.kind, found.expires_minutes), (CodeKind::SignInLink, Some(15)));
        let lien = bulk("Connexion à votre espace", "Bonjour,\n\nPour vous connecter, cliquez sur le bouton ci-dessous.\n\nMe connecter\n\nCe lien est valable 30 minutes et ne fonctionne qu'une fois.");
        assert_eq!(detect_message(&lien).map(|c| c.kind), Some(CodeKind::SignInLink));
        // A code from an account service, a shop, a bank.
        let microsoft = bulk("Microsoft account security code", "Please use the following security code for the Microsoft account jo***@example.org.\n\nSecurity code: 482913\n\nIf you don't recognise the account, you can click a link to remove your email address from that account.");
        assert_eq!(detect_message(&microsoft).and_then(|c| c.code).as_deref(), Some("482913"));
        let shop = bulk("Your Picshare code", "Your Picshare code: 551204\n\nEnter it in the app to finish signing in. It expires in 10 minutes.");
        let found = detect_message(&shop).unwrap();
        assert_eq!((found.code.as_deref(), found.expires_minutes), (Some("551204"), Some(10)));
        let bank = bulk("Validation de votre opération", "Bonjour,\n\nVotre code de validation\u{a0}: 77 41 08\n\nIl expire dans 5 minutes. Ne le communiquez à personne.");
        let found = detect_message(&bank).unwrap();
        assert_eq!((found.code.as_deref(), found.expires_minutes), (Some("774108"), Some(5)));
        // A tracking link and a padded preview line before the code take nothing from the reading.
        let padding = "\u{200c}\u{a0}".repeat(300);
        let tracked = bulk("Verify your email", &format!("{padding}\nView in browser: https://click.service.example/v/{}\n\nYour verification code\n\nhttps://click.service.example/c/{}\n\n482913", "x".repeat(900), "y".repeat(400)));
        assert_eq!(detect_message(&tracked).and_then(|c| c.code).as_deref(), Some("482913"));
        // A password reset from a no-reply address, its link said to expire.
        let reset = bulk("Your account", "Someone asked to change the password of your account.\nUse this link to choose a new password: https://service.example/r/9f8e7d\nThe link expires in 1 hour.");
        assert_eq!(detect_message(&reset).map(|c| c.kind), Some(CodeKind::PasswordReset));
    }

    #[test]
    fn a_newsletter_about_passwords_hands_none_over() {
        let article = "This week: a large provider asked millions of people to reset your password after a breach. \
                       We explain what a password reset is for, why a sign-in link beats a reused password, and why you should \
                       confirm your email only on the site you signed up to. Your security code should never travel by mail: \
                       banks send theirs by text message instead. Remember: your password is yours alone.";
        let news = bulk("The Weekly Byte #87: passwords, passkeys and you", &format!("{article}\n\n{}", "More stories below.\n".repeat(20)));
        assert_eq!(detect_message(&news), None, "{news:?}");
        // A store's newsletter with a promotion code and a link to sign in.
        let store = bulk("Autumn sale", "Use the code AUTUMN25 at checkout for 25 % off. Log in to see your offers: this link is valid until Sunday.");
        assert_eq!(detect_message(&store), None);
        // A security notice: a new sign-in, nothing to use.
        let notice = bulk("New sign-in to your account", "We noticed a new sign-in to your account from Firefox on Linux. If this was you, there is nothing to do. If this wasn't you, change your password.");
        assert_eq!(detect_message(&notice), None);
    }

    #[test]
    fn codes_said_in_other_words() {
        for (subject, body, code) in [
            ("Your login", "123456 is your Picshare code. Don't share it.", "123456"),
            ("Code", "482913 est votre code Picshare.", "482913"),
            ("Votre code", "Votre code Picshare\u{202f}: 551204", "551204"),
            ("Sign in", "Use this code to sign in: 123-456", "123456"),
            ("Vérification", "Saisissez le code suivant pour vérifier votre identité :\n\n4829 1357", "48291357"),
            ("Steam", "Login Code: F7G2K", "F7G2K"),
        ] {
            assert_eq!(detect(subject, body).and_then(|c| c.code).as_deref(), Some(code), "{body}");
        }
        // Codes that are no secret of yours.
        assert!(detect("Your order", "Your zip code: 54390").is_none());
        assert!(detect("Votre compte", "Votre code Client : 123456").is_none());
        assert!(detect("Thanks", "Use this code at checkout: 4821").is_none());
        assert!(detect("Order", "Your order 2026-1004 shipped on 06-10.").is_none());
    }

    /// A code or a password after the word for it and a colon, as texts and
    /// forms give them ("MDP : …", "Here is your PIN: …", "code: …"); what
    /// only looks like one is none.
    #[test]
    fn secrets_after_their_label() {
        for body in ["MDP : Xk82pq!", "Salut, voici l'accès.\nMDP: lune-Rouge-42", "Identifiant : jdupont Mot de passe : Xk82pq", "Login: jdupont, Password: Xk82pq"] {
            assert_eq!(detect("", body).map(|c| c.kind), Some(CodeKind::Password), "{body}");
        }
        for (body, code) in [
            ("Here is your PIN: 4821", "4821"),
            ("Your PIN is 482913.", "482913"),
            ("Atelier poterie jeudi soir. Code : 6107", "6107"),
            ("code: 7314", "7314"),
            ("Votre code PIN : 4829", "4829"),
            ("Le service vous attribue un code provisoire. Votre code provisoire est le 39174826.", "39174826"),
            ("Your temporary code is 551204.", "551204"),
        ] {
            assert_eq!(detect("", body).and_then(|c| c.code).as_deref(), Some(code), "{body}");
        }
        for body in [
            "Zip Code: 94110",
            "Postal code: 94110",
            "Code postal : 54390",
            "Password: ••••••",
            "Change your password: open Settings, then Security.",
            "Code : 2026",
            "MDP oublié ? Appelle-moi.",
        ] {
            assert_eq!(detect("", body), None, "{body}");
        }
        // A label added in the configuration is read; the packs alone do not know it.
        let config: crate::config::Config = toml::from_str("[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n[words.codes.password_label]\nadd = [\"Kennwort\"]\n").unwrap();
        assert_eq!(detect("", "Kennwort: Xk82pq"), None);
        assert_eq!(super::detect(&Words::of(&config).codes, "", "Kennwort: Xk82pq").map(|c| c.kind), Some(CodeKind::Password));
    }

    /// The text of 24 November 2026, as a payment service sent it from a
    /// short number (its shape: the code first, then "est le code de votre
    /// transaction de …"), in each language the detector knows, with an
    /// invented code, amount and shop; and the payment and sign-in texts
    /// usual in English and French.
    #[test]
    fn a_transaction_code_named_after_its_digits() {
        let all: crate::config::Config = toml::from_str("[words]\nlanguages = [\"fr\", \"en\", \"de\", \"es\", \"it\"]\ncountries = [\"FR\"]\n").unwrap();
        let every = Words::of(&all);
        for (title, body, code) in [
            ("36123", "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Boutique Exemple. Gardez ce code pour vous", "482913"),
            ("36123", "ExamplePay: 482913 est le code de votre transaction de 1234,56 EUR chez Boutique Exemple. Gardez ce code pour vous", "482913"),
            ("36123", "ExamplePay: 482913 is the code for your transaction of 23.90 EUR at Example Shop. Keep this code to yourself", "482913"),
            ("36123", "ExamplePay: 482913 ist der Code für Ihre Transaktion von 23,90 EUR bei Beispielladen. Behalten Sie diesen Code für sich", "482913"),
            ("36123", "ExamplePay: 482913 es el código de tu transacción de 23,90 EUR en Tienda Ejemplo. Guarda este código para ti", "482913"),
            ("36123", "ExamplePay: 482913 è il codice della tua transazione di 23,90 EUR presso Negozio Esempio. Tieni questo codice per te", "482913"),
        ] {
            assert_eq!(super::detect(&every.codes, title, body).and_then(|c| c.code).as_deref(), Some(code), "{body}");
        }
        // French and English alone, as the packs built in.
        for (title, body, code) in [
            ("36123", "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Boutique Exemple. Gardez ce code pour vous", "482913"),
            ("36123", "ExamplePay: 482913 is the code for your transaction of 23.90 EUR at Example Shop. Keep this code to yourself", "482913"),
            // Payments: 3-D Secure and the payment services' usual wordings.
            ("BANQUE", "Code de validation de votre paiement de 49,90 EUR chez BOUTIQUE EXEMPLE : 551204. Ne le communiquez à personne.", "551204"),
            ("BANQUE", "Pour valider votre achat de 49,90 EUR, saisissez le code 551204. Il expire dans 5 minutes.", "551204"),
            ("BANQUE", "Votre code est le 551204. Ne le communiquez à personne.", "551204"),
            ("Bank", "Use code 551204 to approve your payment of 49.90 EUR at Example Shop.", "551204"),
            ("Bank", "Your one-time passcode for your purchase at EXAMPLE SHOP is 551204. Never share it.", "551204"),
            // Sign-ins.
            ("Service", "<#> 551204 est votre code de vérification Exemple.", "551204"),
            ("Service", "Exemple : votre code de connexion est 551204. Il expire dans 10 min.", "551204"),
            ("Service", "Your Example verification code is: 551204", "551204"),
            ("Service", "551204 is your Example login code. Don't share it with anyone.", "551204"),
            ("Service", "Utilisez le code 551204 pour vous connecter à Exemple.", "551204"),
        ] {
            assert_eq!(detect(title, body).and_then(|c| c.code).as_deref(), Some(code), "{body}");
        }
        // What only looks like one: a parcel number, an amount, a date, a phone number, a year,
        // a door's code, a postcode, a locker's, an order's, a client number.
        for (title, body) in [
            ("38080", "Votre colis 6A12345678901 est en route. Suivi : https://suivi.example/6A12345678901"),
            ("Colis", "Votre colis 48291357 sera livré demain. Code de suivi : 48291357."),
            ("BANQUE", "Un paiement de 1234,56 EUR a été débité le 24/11/2026."),
            ("BANQUE", "Prélèvement de 482913 EUR le 2026-11-24."),
            ("Agenda", "Rendez-vous le 24/11/2026 à 21h54, au 06 12 34 56 78."),
            ("Mairie", "Le marché de Noël 2026 ouvre le 5 décembre."),
            ("Alice", "4821 est le code de la porte, à ce soir !"),
            ("Alice", "4821 is the code for the gate, see you tonight!"),
            ("Mairie", "75011 est le code postal de votre nouvelle adresse."),
            ("Casier", "4821 est le code de votre casier. Bonne séance !"),
            ("Shop", "4821 is the code for your order of 23.90 EUR. Thank you!"),
            ("Shop", "Votre numéro client est 48291357."),
        ] {
            assert_eq!(detect(title, body), None, "{body}");
        }
    }

    /// The label rule held tight (review of 24 November 2026, item 8): a label
    /// counts at a line's start or after a full stop, never after a comma, a
    /// dash or a word, never when the words before it make it someone else's
    /// code, and only with one token of a code's shape after it; a password's
    /// label takes one token too. In English and French. And item 2: an owner
    /// (with an article at most) before a password's label opens it, in the
    /// five languages.
    #[test]
    fn labels_that_are_someone_else_s() {
        for body in [
            "Erreur. Code : 5003",
            "Error. Code: 5003",
            "QR-Code: 12345678",
            "QR code: 12345678",
            "Porte B, code : 4821",
            "Door B, code: 4821",
            "Colis prêt. Code : 482913",
            "Parcel ready. Code: 482913",
            "Code : 5003 erreur interne du serveur",
            "Mot de passe : celui choisi à l'inscription",
            "Password: the one you chose when you signed up",
            "Change your password: open Settings, then Security.",
        ] {
            assert_eq!(detect("", body), None, "{body}");
            assert_eq!(super::detect_text(&Words::builtin().codes, body), None, "{body}");
        }
        let all: crate::config::Config = toml::from_str("[words]\nlanguages = [\"fr\", \"en\", \"de\", \"es\", \"it\"]\ncountries = [\"FR\"]\n").unwrap();
        let every = Words::of(&all);
        for body in ["Votre mot de passe : Xk82pq", "Ton mdp : Xk82pq", "Your password: Xk82pq", "Bonjour.\nVotre mot de passe : Xk82pq", "Ihr Passwort: Xk82pq", "Tu contraseña: Xk82pq", "La tua password: Xk82pq"] {
            assert_eq!(super::detect(&every.codes, "", body).map(|c| c.kind), Some(CodeKind::Password), "{body}");
        }
        // An owner far in its sentence, or a word that is no article before it: none.
        for body in ["Pensez à changer votre mot de passe : Xk82pq", "Your new phone password: Xk82pq soon"] {
            assert_eq!(super::detect(&every.codes, "", body), None, "{body}");
        }
    }

    /// A text read whole keeps its line breaks (`detect_text`): a label at a
    /// line's start is read there, which a subject read on one line never
    /// shows; a colon earlier in the text makes no later word a label.
    #[test]
    fn a_text_keeps_its_lines() {
        let words = Words::builtin();
        assert_eq!(super::detect_text(&words.codes, "Bring the cake\nCode: 7319").and_then(|c| c.code).as_deref(), Some("7319"));
        assert_eq!(super::detect_text(&words.codes, "Hello\nPassword: Lune-Rouge-58").map(|c| c.kind), Some(CodeKind::Password));
        assert_eq!(super::detect_text(&words.codes, "We meet at 10:30, bring your Password: open Settings first"), None);
        assert_eq!(super::detect_text(&words.codes, "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Boutique Exemple. Gardez ce code pour vous").and_then(|c| c.code).as_deref(), Some("482913"));
    }

    /// The digits before the word, as a payment service texts them, and what
    /// only looks like it: an order's number, a year, an amount, a parcel.
    #[test]
    fn digits_before_a_word_that_is_no_code() {
        for body in [
            "123456 est le numéro de votre commande.",
            "123456 is the number of your order.",
            "2026 est l'année de votre adhésion.",
            "1234,56 est le montant de votre transaction.",
            "48291357 est le numéro de votre colis.",
            "48291357 est le code de votre colis.",
            "48291357 is the code of your parcel.",
        ] {
            assert_eq!(detect("", body), None, "{body}");
        }
    }

    /// Review of 25 November 2026, must-fixes 1 and 2: a shop's code after a
    /// pointing phrase is no one-time code, whatever the message says of a
    /// purchase; a shop's name further on never undoes a code named before
    /// its word; a promotion in the code's own sentence does.
    #[test]
    fn a_shop_s_code_is_none_and_a_shop_s_name_undoes_nothing() {
        for (title, body) in [
            ("36111", "Boutique Exemple: -20% dès 60E d'achat avec le code EXEMPLE20 jusqu'au 30/11 en magasin et sur boutique.example STOP 36111"),
            ("36111", "Boutique Exemple: -20% ce week-end avec le code NOEL sur votre achat en ligne. STOP au 36111"),
            ("Shop", "Use code SAVE10 at checkout to get $10 off your next purchase."),
            ("Shop", "Enter code WELCOME at checkout to save on your first purchase."),
            ("Boutique", "Merci pour votre achat : saisissez ce code CADEAU15 lors de votre prochaine commande."),
            ("Boutique", "4821 est le code de votre bon d'achat."),
        ] {
            assert_eq!(detect(title, body), None, "{body}");
        }
        assert_eq!(detect_message(&bulk("Derniers jours : -20 % avec le code NOEL20", "Jusqu'à dimanche, dès 50 € d'achat sur tout le site.")), None);
        for (title, body, code) in [
            ("36123", "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Pneus Discount Exemple. Gardez ce code pour vous", "482913"),
            ("36123", "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Promo Exemple. Gardez ce code pour vous", "482913"),
            ("36123", "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Exemple Coupon. Gardez ce code pour vous", "482913"),
            ("36123", "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Exemple Voucher. Gardez ce code pour vous", "482913"),
            ("36123", "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Réduction Exemple. Gardez ce code pour vous", "482913"),
            ("36123", "ExamplePay: 482913 is the code for your transaction of 23.90 EUR at Example Discount Shop. Keep this code to yourself", "482913"),
            // Still codes: digits after a pointing phrase, in a message about paying or signing in.
            ("BANQUE", "Pour valider votre achat de 49,90 EUR, saisissez le code 551204.", "551204"),
            ("Bank", "Use code 551204 to approve your payment of 49.90 EUR at Example Shop.", "551204"),
            ("Service", "Utilisez le code 551204 pour vous connecter à Exemple.", "551204"),
            ("Vérification", "Saisissez le code suivant pour vérifier votre identité :\n\n4829 1357", "48291357"),
        ] {
            assert_eq!(detect(title, body).and_then(|c| c.code).as_deref(), Some(code), "{body}");
        }
    }

    /// Review of 25 November 2026, should-fix 3: a code that reads as a year
    /// is a code right beside a phrase that names it, before or after; a
    /// year anywhere else stays none (a date, "Votre code : 2026", a label).
    #[test]
    fn a_year_s_shape_beside_its_phrase_is_a_code() {
        for (title, body, code) in [
            ("36123", "ExamplePay: 2024 est le code de votre transaction de 23,90 EUR chez Boutique Exemple.", "2024"),
            ("Bank", "1987 is your Example verification code.", "1987"),
            ("BANQUE", "Votre code de sécurité est 2031. Il expire dans 5 minutes.", "2031"),
            ("Service", "Your verification code: 1999", "1999"),
        ] {
            assert_eq!(detect(title, body).and_then(|c| c.code).as_deref(), Some(code), "{body}");
        }
        for (title, body) in [
            ("Bienvenue", "Votre code : 2026"),
            ("", "Code : 2026"),
            ("Your verification code", "Your verification code for 2026 is below."),
            ("Agenda", "Votre code de vérification arrivera le 24/11/2026."),
            ("Mairie", "Le marché de Noël 2026 ouvre le 5 décembre."),
            ("BANQUE", "Votre code de sécurité vous sera envoyé en 2026."),
        ] {
            assert_eq!(detect(title, body), None, "{body}");
        }
        // A year beside its phrase, a real code further on: the code.
        assert_eq!(detect("Your verification code", "Your verification code for 2026: 4821").and_then(|c| c.code).as_deref(), Some("4821"));
    }

    /// Decimals and a currency sign make an amount, never the code before
    /// "ce code"; the informal pointing phrases in a message about paying;
    /// the longest phrase ("è il tuo" before "è il"); the owner within six
    /// words after the code's word; the owners of German and Italian.
    #[test]
    fn amounts_informal_phrases_and_owners() {
        for (title, body) in [
            ("BANQUE", "Votre paiement de 1234,56 EUR est en attente. Ne partagez jamais ce code."),
            ("Bank", "Your payment of €4821 is pending. Never share this code."),
            ("", "482913 est le code que nous vous envoyons ce soir pour confirmer votre transaction."),
        ] {
            assert_eq!(detect(title, body), None, "{body}");
        }
        for pointing in ["saisis le code", "entre le code", "utilise le code", "tapez le code"] {
            let body = format!("Pour confirmer ton paiement, {pointing} 551204.");
            assert_eq!(detect("BANQUE", &body).and_then(|c| c.code).as_deref(), Some("551204"), "{body}");
        }
        assert_eq!(detect("Bank", "Enter code 551204 to confirm your payment.").and_then(|c| c.code).as_deref(), Some("551204"));
        let all: crate::config::Config = toml::from_str("[words]\nlanguages = [\"fr\", \"en\", \"de\", \"es\", \"it\"]\ncountries = [\"FR\"]\n").unwrap();
        let every = Words::of(&all);
        for body in ["482913 è il tuo codice Esempio.", "482913 ist der Code für deine Zahlung.", "482913 è il codice della sua transazione.", "482913 è il codice della vostra transazione."] {
            assert_eq!(super::detect(&every.codes, "", body).and_then(|c| c.code).as_deref(), Some("482913"), "{body}");
        }
    }

    #[test]
    fn a_text_version_that_says_nothing() {
        // Its text version only points to the HTML one, which holds the code.
        let raw = "From: Service <no-reply@service.example>\r\nSubject: Your code\r\nList-Id: <accounts.service.example>\r\n\
                   MIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"b\"\r\n\r\n\
                   --b\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\nView this email in your browser: https://service.example/view/a1b2c3\r\n\
                   --b\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<html><body><p>Hello,</p><p>Your verification code is <b>482913</b>.</p><p>It expires in 10 minutes.</p></body></html>\r\n--b--\r\n";
        let card = crate::card::Card::from_bytes(raw.as_bytes()).unwrap();
        assert_eq!(detect_message(&card).and_then(|c| c.code).as_deref(), Some("482913"), "{:?}", card.excerpt);
    }
}

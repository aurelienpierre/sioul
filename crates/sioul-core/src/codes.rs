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
//! warning. The detector reads French and English.

use crate::text::{find_word, fold};

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

/// Phrases are compared folded: lowercase, without accents (see `text`).
const CODE_PHRASES: &[&str] = &[
    "verification code", "security code", "confirmation code", "authentication code",
    "login code", "sign-in code", "sign in code", "one-time code", "one time code",
    "one-time password", "one time password", "single-use code", "passcode", "otp",
    "validation code", "access code", "2fa", "two-factor", "two factor",
    "2-step verification", "two-step verification",
    "code de verification", "code de securite", "code de confirmation",
    "code d'authentification", "code d'acces", "code de connexion",
    "code a usage unique", "mot de passe a usage unique", "code temporaire",
    "code de validation", "code d'identification", "double authentification",
    "authentification a deux facteurs",
];
/// A shop's code is no secret of yours: "your code SPRING20 for 20 % off".
const PROMO_PHRASES: &[&str] = &[
    "promo", "discount", "coupon", "voucher", "gift code", "% off",
    "reduction", "bon d'achat", "code avantage", "code cadeau",
];
/// Only the words that hand a password over: "log in with your login details"
/// (every notice of a public service) or "your new password was saved" carry none.
const PASSWORD_PHRASES: &[&str] = &[
    "temporary password", "initial password", "your password is", "your new password is",
    "your new password:", "here is your password", "here is your new password",
    "here are your login details", "your login details are", "your login details:",
    "mot de passe provisoire", "mot de passe temporaire", "votre mot de passe est",
    "votre nouveau mot de passe est", "votre nouveau mot de passe :", "votre nouveau mot de passe:",
    "voici votre mot de passe", "voici votre nouveau mot de passe", "mot de passe initial",
    "voici vos identifiants", "vos identifiants sont", "vos identifiants :", "vos identifiants:",
    "vos identifiants de connexion sont", "vos identifiants de connexion :",
];
const RESET_PHRASES: &[&str] = &[
    "reset your password", "password reset", "reset password", "reset link",
    "forgot your password", "forgotten password", "choose a new password",
    "set a new password", "create a new password", "set your password", "create your password",
    "reinitialiser votre mot de passe", "reinitialisation de votre mot de passe",
    "reinitialisation du mot de passe", "mot de passe oublie", "lien de reinitialisation",
    "definir votre mot de passe", "definissez votre mot de passe",
    "choisir un nouveau mot de passe", "choisissez un nouveau mot de passe",
    "creer votre mot de passe", "creez votre mot de passe",
];
const LINK_PHRASES: &[&str] = &[
    "sign-in link", "sign in link", "login link", "log in link", "magic link", "connection link",
    "click to sign in", "click to log in",
    "lien de connexion", "lien magique", "se connecter avec ce lien", "cliquez pour vous connecter",
];
const CONFIRM_PHRASES: &[&str] = &[
    "confirm your email", "verify your email", "confirm your e-mail", "verify your e-mail",
    "confirm your address", "verify your address", "activate your account",
    "confirm your account", "confirm your registration", "confirm your subscription",
    "confirmez votre adresse", "confirmer votre adresse", "verifiez votre adresse",
    "verifier votre adresse", "validez votre adresse", "valider votre adresse",
    "activez votre compte", "activer votre compte", "confirmez votre compte",
    "confirmer votre compte", "confirmez votre inscription", "confirmer votre inscription",
];

/// Characters of subject and body looked at: codes sit at the top of these messages.
const SCAN_LIMIT: usize = 6000;
/// How far around the phrase a code is looked for, in characters.
const AFTER_WINDOW: usize = 160;
const BEFORE_WINDOW: usize = 100;

/// Words that hand a code over: "your code is K7Q-2M9", "votre code : AB12CD".
/// The code must follow them at once, "is", "est" or a colon between.
const GIVING_PHRASES: &[&str] = &["your code", "votre code", "ton code"];
/// What may stand between a phrase and the code it gives, besides spaces and a colon.
const LINKING_WORDS: &[&str] = &["is", "est"];

/// Finds a short-lived secret in a message, if it carries one.
pub fn detect(subject: &str, body: &str) -> Option<OneTimeCode> {
    let text: String = format!("{subject}\n{body}").chars().take(SCAN_LIMIT).collect();
    let original: Vec<char> = text.chars().collect();
    let folded = fold(&text);
    let expires_minutes = expiry_minutes(&folded);
    let kinds = [
        (CodeKind::Code, CODE_PHRASES),
        (CodeKind::Password, PASSWORD_PHRASES),
        (CodeKind::PasswordReset, RESET_PHRASES),
        (CodeKind::SignInLink, LINK_PHRASES),
        (CodeKind::Confirmation, CONFIRM_PHRASES),
    ];
    let promotion = first_hit(&folded, PROMO_PHRASES).is_some();
    kinds.iter().find_map(|&(kind, phrases)| {
        if kind != CodeKind::Code {
            first_hit(&folded, phrases)?;
            return Some(OneTimeCode { kind, code: None, expires_minutes });
        }
        if promotion {
            return None;
        }
        // A code phrase without a code nearby ("never share your security
        // code", a heading) is no code: the next phrase is read, and so on.
        let code = code_in(&original, &folded)?;
        Some(OneTimeCode { kind, code: Some(code), expires_minutes })
    })
}

/// The earliest of the phrases in the text, as (start, end) positions.
fn first_hit(folded: &[char], phrases: &[&str]) -> Option<(usize, usize)> {
    phrases
        .iter()
        .filter_map(|p| find_word(folded, p, 0).map(|start| (start, start + fold(p).len())))
        .min_by_key(|&(start, _)| start)
}

/// Every place of each phrase in the text, as (start, end) positions.
fn every_hit(folded: &[char], phrases: &[&str]) -> Vec<(usize, usize)> {
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

/// The code beside the first phrase, in the text's order, that has one.
fn code_in(original: &[char], folded: &[char]) -> Option<String> {
    let found = codes(original);
    let mut hits: Vec<(usize, usize, bool)> = every_hit(folded, CODE_PHRASES).into_iter().map(|(start, end)| (start, end, false)).collect();
    hits.extend(every_hit(folded, GIVING_PHRASES).into_iter().map(|(start, end)| (start, end, true)));
    hits.sort_unstable();
    hits.into_iter().find_map(|(start, end, giving)| code_beside(folded, &found, (start, end), giving))
}

/// The code next to a phrase: right after it, whatever its shape ("votre code
/// est : 482 913", "your sign-in code: K7Q-2M9"); else, for a phrase of
/// `CODE_PHRASES`, digits further after it, or the nearest before it
/// ("G-482913 is your Google verification code").
fn code_beside(folded: &[char], found: &[Found], (start, end): (usize, usize), giving: bool) -> Option<String> {
    if let Some(next) = found.iter().find(|f| f.start >= end)
        && ties(&folded[end..next.start], giving)
    {
        return Some(next.code.clone());
    }
    if giving {
        return None;
    }
    let plain = || found.iter().filter(|f| !f.mixed);
    plain()
        .find(|f| f.start >= end && f.start < end + AFTER_WINDOW)
        .or_else(|| plain().filter(|f| f.start + BEFORE_WINDOW >= start && f.end <= start).last())
        .map(|f| f.code.clone())
}

/// Whether what stands between a phrase and a code ties them: spaces, a
/// colon, "is" or "est". After "your code", a colon or one of these words
/// at least: "votre code postal : 54390" gives no code.
fn ties(gap: &[char], giving: bool) -> bool {
    let gap: String = gap.iter().collect();
    let words: Vec<&str> = gap.split(|c: char| c.is_whitespace() || c == ':' || c == '=').filter(|w| !w.is_empty()).collect();
    let linked = gap.contains([':', '=']) || !words.is_empty();
    words.len() <= 1 && words.iter().all(|w| LINKING_WORDS.contains(w)) && (linked || !giving)
}

/// A code-shaped stretch of the text: where it is, the code as kept, and
/// whether it mixes letters and digits (then read only right after its phrase).
struct Found {
    start: usize,
    end: usize,
    code: String,
    mixed: bool,
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
            found.extend(grouped_code(run).map(|code| Found { start: run[0].start, end: run[run.len() - 1].end, code, mixed: false }));
        } else {
            for token in run {
                let code = code_from(token).map(|c| (c, false)).or_else(|| mixed_code(&token.text).map(|c| (c, true)));
                if let Some((code, mixed)) = code {
                    found.push(Found { start: token.start, end: token.end, code, mixed });
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
/// phone number), or a prefixed code ("G-482913").
fn code_from(token: &Token) -> Option<String> {
    let t = &token.text;
    if is_digits(t) && (4..=8).contains(&t.len()) && !looks_like_amount_or_year(token) {
        return Some(t.clone());
    }
    prefixed_code(t)
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

/// A code of letters and digits ("K7Q-2M9", "AB12CD"): 6 to 10 characters,
/// uppercase, two letters and two digits at least, so that no word is one.
fn mixed_code(text: &str) -> Option<String> {
    let letters = text.chars().filter(char::is_ascii_uppercase).count();
    let digits = text.chars().filter(char::is_ascii_digit).count();
    let shaped = text.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-') && !text.contains("--");
    (shaped && (6..=10).contains(&text.len()) && letters >= 2 && digits >= 2).then(|| text.to_string())
}

fn looks_like_amount_or_year(token: &Token) -> bool {
    is_year(&token.text) || token.before == '+' || amount_follows(token)
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
fn expiry_minutes(folded: &[char]) -> Option<u32> {
    const CONTEXT: &[&str] = &["valid", "valable", "expire", "pendant", "during", "within", "dans", "for"];
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
        let in_context = CONTEXT.iter().any(|w| before.contains(w));
        let minutes = if unit.starts_with("min") {
            Some(number)
        } else if unit.starts_with("heure") || unit.starts_with("hour") {
            Some(number.saturating_mul(60))
        } else if unit.starts_with("day") || unit.starts_with("jour") {
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
}

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
//!
//! Many sites send these through the same services as their newsletters, with
//! the same headers (`List-Unsubscribe`, `Precedence: bulk`): those headers
//! never decide that a message is not a code (`detect_message`). They only
//! narrow where a code is looked for: in the subject and the top of the text,
//! where a site puts what you asked for, and not in a newsletter's articles,
//! which talk about passwords without handing one over.

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
    "login code", "log-in code", "sign-in code", "sign in code", "signin code", "one-time code", "one time code",
    "one-time password", "one time password", "single-use code", "passcode", "otp",
    "validation code", "access code", "activation code", "2fa", "two-factor", "two factor",
    "2-step verification", "two-step verification", "verification pin", "one-time pin",
    "code de verification", "code de securite", "code de confirmation",
    "code d'authentification", "code d'acces", "code de connexion", "code d'activation",
    "code a usage unique", "mot de passe a usage unique", "code temporaire",
    "code de validation", "code d'identification", "double authentification",
    "authentification a deux facteurs",
];
/// Words that point at a code without naming its use: "enter the code below",
/// "utilisez le code suivant". A code beside them counts only in a message
/// about signing in or checking who you are (`SIGN_IN_CUES`), so that "use
/// this code at checkout" is no secret of yours.
const POINTING_PHRASES: &[&str] = &[
    "following code", "code below", "this code", "enter the code", "use the code", "copy the code", "type the code",
    "code suivant", "ce code", "code ci-dessous", "code ci dessous", "saisissez le code", "entrez le code",
    "utilisez le code", "indiquez le code", "renseignez le code", "copiez le code",
];
/// What a message about signing in or checking who you are says somewhere.
const SIGN_IN_CUES: &[&str] = &[
    "sign in", "sign-in", "signin", "log in", "log-in", "login", "sign into", "log into", "signing in", "logging in",
    "sign up", "sign-up", "verify", "verification", "confirm", "identity", "authenticate", "authentication",
    "connexion", "connecter", "connectez", "identifier", "identification", "verifier", "verification",
    "confirmer", "identite", "authentifier", "authentification", "inscription",
];
/// Words between "your" and "code", or right after "code", that make it no
/// secret of yours: "your zip code: 54390", "votre code client : 123456".
const NOT_A_SECRET: &[&str] = &[
    "zip", "postal", "post", "postcode", "area", "country", "dialing", "dialling", "promo", "promotional", "promotion",
    "discount", "coupon", "gift", "voucher", "referral", "invite", "invitation", "tracking", "booking", "reservation",
    "order", "customer", "client", "member", "membership", "product", "item", "error", "source", "dress", "tax", "vat",
    "bank", "sort", "swift", "bic", "iban", "routing", "door", "gate", "building", "lockbox", "wifi", "wi-fi", "parking",
    "locker", "promotionnel", "cadeau", "avantage", "reduction", "parrainage", "suivi", "colis", "commande",
    "adherent", "membre", "produit", "article", "erreur", "vestimentaire", "porte", "portail", "immeuble", "casier",
    "banque", "guichet", "naf", "ape", "insee", "fiscal", "tva", "pays", "region", "departement", "commune",
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
    "sign-in link", "sign in link", "signin link", "login link", "log in link", "log-in link", "magic link", "connection link",
    "click to sign in", "click to log in", "link to sign in", "link to log in", "sign in with this link", "log in with this link",
    "this link will sign you in", "this link will log you in",
    "confirm your login", "confirm your sign-in", "confirm your sign in", "verify your login", "verify your sign-in", "verify your sign in",
    "lien de connexion", "lien magique", "se connecter avec ce lien", "cliquez pour vous connecter",
    "lien pour vous connecter", "lien pour se connecter", "confirmez votre connexion", "confirmer votre connexion",
    "validez votre connexion", "valider votre connexion",
];
/// What a sign-in link's message is about, in its subject or its first lines:
/// "Sign in to Medium", "Connexion à votre espace". Alone it proves nothing
/// (every notice says "log in to read it"): the link must also be said to
/// expire or to work once (`LINK_WORDS`, `VALIDITY_WORDS`).
const SIGN_IN_INTENTS: &[&str] = &[
    "sign in", "sign-in", "signin", "log in", "log-in", "login", "sign into", "log into", "signing in", "logging in",
    "connexion", "vous connecter", "se connecter", "connectez-vous",
];
/// What may be said to expire or to work once: the link, its button, the code.
const LINK_WORDS: &[&str] = &["link", "button", "code", "lien", "bouton"];
/// "This link expires in 15 minutes", "ce lien est valable une heure".
const VALIDITY_WORDS: &[&str] = &["expire", "expires", "expired", "expiration", "valid", "valable", "valide", "expirera", "expirer"];
/// How far after "link" its validity is said, in characters.
const VALIDITY_WINDOW: usize = 80;
/// Lines that answer a request of yours: "if you did not ask for this,
/// ignore this message". A newsletter does not write them.
const REQUEST_CUES: &[&str] = &[
    "if you did not request", "if you didn't request", "if you did not ask", "if you didn't ask",
    "if this wasn't you", "if this was not you", "if it wasn't you", "if you did not try", "if you didn't try",
    "if you did not make this request", "if you didn't make this request", "you can safely ignore", "you can ignore this",
    "ignore this email", "ignore this e-mail", "ignore this message",
    "si vous n'avez pas demande", "si vous n'etes pas a l'origine", "si ce n'est pas vous", "si vous n'avez pas essaye",
    "si vous n'avez pas fait cette demande", "ignorez ce message", "ignorer ce message", "ignorez cet e-mail",
    "ignorez cet email", "ignorer cet e-mail", "ignorer cet email", "ignorez ce mail",
    // What comes with a password handed over.
    "change it after", "change it when you", "at your first login", "at your first sign-in", "when you first sign in",
    "changez-le", "modifiez-le", "a votre premiere connexion", "lors de votre premiere connexion", "des votre premiere connexion",
];
const CONFIRM_PHRASES: &[&str] = &[
    "confirm your email", "verify your email", "confirm your e-mail", "verify your e-mail",
    "confirm your address", "verify your address", "activate your account",
    "confirm your account", "confirm your registration", "confirm your subscription",
    "verify your account", "validate your email", "validate your e-mail",
    "confirmez votre adresse", "confirmer votre adresse", "verifiez votre adresse",
    "confirmez votre e-mail", "confirmer votre e-mail", "confirmez votre email", "confirmer votre email",
    "verifiez votre e-mail", "verifier votre e-mail", "verifiez votre email", "verifier votre email",
    "verifier votre adresse", "validez votre adresse", "valider votre adresse",
    "activez votre compte", "activer votre compte", "confirmez votre compte",
    "confirmer votre compte", "confirmez votre inscription", "confirmer votre inscription",
];

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

/// Words that hand a code over: "your code is K7Q-2M9", "votre code : AB12CD".
/// The code must follow them at once, "is", "est" or a colon between.
const GIVING_PHRASES: &[&str] = &["your code", "votre code", "ton code"];
/// Whose code it is, a few words before "code": "your Instagram code: 123456".
const OWNERS: &[&str] = &["your", "votre", "ton"];
/// What says a code is yours, right after it: "123456 is your Instagram code".
const OWNED_AFTER: &[&str] = &["is your", "est votre", "est ton"];
/// What may stand between a phrase and the code it gives, besides spaces and a colon.
const LINKING_WORDS: &[&str] = &["is", "est"];

/// Finds a short-lived secret in a message, if it carries one.
pub fn detect(subject: &str, body: &str) -> Option<OneTimeCode> {
    read(subject, body, false)
}

/// Finds a short-lived secret in a message read from mail. Bulk headers
/// (`Card::is_list`: `List-Unsubscribe`, `List-Id`, `Precedence: bulk`) are
/// how many sites send codes and links too, so they never rule one out: they
/// only narrow the reading to the subject and the top of the text (`LEAD`),
/// and a phrase that hands nothing over by itself (a reset, a link, an
/// address to confirm, a password), when it is not in the subject, counts only
/// beside a line that answers a request ("if you did not ask for this…",
/// `REQUEST_CUES`) or a link said to expire. A newsletter's article on
/// passwords is none.
pub fn detect_message(card: &crate::card::Card) -> Option<OneTimeCode> {
    read(&card.subject, &card.excerpt, card.is_list)
}

fn read(subject: &str, body: &str, bulk: bool) -> Option<OneTimeCode> {
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
    let expires_minutes = expiry_minutes(&folded);
    let promotion = first_hit(&folded, PROMO_PHRASES).is_some();
    let secret = |kind, code| Some(OneTimeCode { kind, code, expires_minutes });
    // A code phrase without a code nearby ("never share your security
    // code", a heading) is no code: the next phrase is read, and so on.
    if !promotion && let Some(code) = code_in(&original, &folded) {
        return secret(CodeKind::Code, Some(code));
    }
    let brief = lasts_briefly(&folded, expires_minutes);
    // In bulk mail, a phrase without a code beside it counts in the subject,
    // or beside a cue: "your password is" is also an article's sentence.
    let cued = !bulk || first_hit(&folded, REQUEST_CUES).is_some() || brief;
    let counts = |hit: Option<(usize, usize)>| hit.is_some_and(|(start, _)| start < in_subject || cued);
    for (kind, phrases) in [(CodeKind::Password, PASSWORD_PHRASES), (CodeKind::PasswordReset, RESET_PHRASES), (CodeKind::SignInLink, LINK_PHRASES), (CodeKind::Confirmation, CONFIRM_PHRASES)] {
        if counts(first_hit(&folded, phrases)) {
            return secret(kind, None);
        }
    }
    // "Sign in to Medium" above, "this link expires in 15 minutes" below: a
    // link to sign in. A shop's "log in to see your offers" is none.
    let intent = first_hit(&folded, SIGN_IN_INTENTS).filter(|&(start, _)| start <= in_subject + INTENT_LEAD);
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
fn lasts_briefly(folded: &[char], expires_minutes: Option<u32>) -> bool {
    const ONCE: &[&str] = &["used once", "only once", "use once", "single use", "single-use", "one-time use", "one time use", "une seule fois", "usage unique", "qu'une fois"];
    let soon = expires_minutes.is_some_and(|m| m <= BRIEF_MINUTES);
    every_hit(folded, LINK_WORDS).into_iter().any(|(_, end)| {
        let window = &folded[end..(end + VALIDITY_WINDOW).min(folded.len())];
        let said = |words: &[&str]| words.iter().any(|w| find_word(window, w, 0).is_some());
        said(ONCE) || (soon && said(VALIDITY_WORDS))
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

/// The code beside the first phrase, in the text's order, that has one; else
/// a code said to be yours right after it ("123456 is your Instagram code").
fn code_in(original: &[char], folded: &[char]) -> Option<String> {
    let found = codes(original);
    let mut hits: Vec<(usize, usize, bool)> = every_hit(folded, CODE_PHRASES).into_iter().map(|(start, end)| (start, end, false)).collect();
    hits.extend(every_hit(folded, GIVING_PHRASES).into_iter().map(|(start, end)| (start, end, true)));
    hits.extend(owned_codes(original, folded).into_iter().map(|(start, end)| (start, end, true)));
    // "Enter the code below", only in a message about signing in or checking who you are.
    if first_hit(folded, SIGN_IN_CUES).is_some() {
        hits.extend(every_hit(folded, POINTING_PHRASES).into_iter().map(|(start, end)| (start, end, false)));
    }
    hits.sort_unstable();
    hits.into_iter().find_map(|(start, end, giving)| code_beside(folded, &found, (start, end), giving)).or_else(|| named_after(folded, &found))
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
/// one that makes it no secret (`NOT_A_SECRET`), nothing but spaces between
/// them. As (start, end): what must be followed by the code itself.
fn owned_codes(original: &[char], folded: &[char]) -> Vec<(usize, usize)> {
    let words = words(folded);
    let text = |(start, end): (usize, usize)| folded[start..end].iter().collect::<String>();
    let mut out = Vec::new();
    for (index, &code) in words.iter().enumerate() {
        if text(code) != "code" {
            continue;
        }
        // Back: the owner one to three words before, only spaces between.
        let owner = (1..=3).filter_map(|back| index.checked_sub(back)).find(|&at| OWNERS.contains(&text(words[at]).as_str()));
        let Some(owner) = owner else { continue };
        let between = &words[owner + 1..index];
        // Words and spaces only: "your account. Code: …" is two sentences.
        let spaced = folded[words[owner].1..code.0].iter().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '\''));
        if !spaced || between.iter().any(|&w| NOT_A_SECRET.contains(&text(w).as_str())) {
            continue;
        }
        out.push((words[owner].0, code.1));
        // A name after "code", with its capital: "votre code Instagram : 123456".
        if let Some(&name) = words.get(index + 1)
            && folded[code.1..name.0].iter().all(|c| *c == ' ')
            && original[name.0].is_uppercase()
            && !NOT_A_SECRET.contains(&text(name).as_str())
        {
            out.push((words[owner].0, name.1));
        }
    }
    out
}

/// A code followed by what says it is yours: "123456 is your Instagram code",
/// "482913 est votre code de vérification". Up to three words between "your"
/// and "code", none that makes it no secret.
fn named_after(folded: &[char], found: &[Found]) -> Option<String> {
    found.iter().find_map(|f| {
        let rest = &folded[f.end..(f.end + 60).min(folded.len())];
        let lead = rest.iter().take_while(|c| **c == ' ').count();
        let owned = OWNED_AFTER.iter().find(|p| {
            let p = fold(p);
            rest[lead..].starts_with(&p) && rest.get(lead + p.len()).is_none_or(|c| !c.is_alphanumeric())
        })?;
        let after = &rest[lead + owned.chars().count()..];
        let words = words(after);
        let text = |(start, end): (usize, usize)| after[start..end].iter().collect::<String>();
        let code = words.iter().take(4).position(|&w| text(w) == "code")?;
        let clean = words[..code].iter().all(|&w| !NOT_A_SECRET.contains(&text(w).as_str()));
        let spaced = after[..words[code].0].iter().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '\''));
        (clean && spaced).then(|| f.code.clone())
    })
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

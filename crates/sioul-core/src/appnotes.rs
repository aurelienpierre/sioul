// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Other apps' notifications on a phone (docs/android.md, "Notifications from
//! other apps"; research: docs/research/android-chats.md, 7 and 8). Android
//! hands Sioul's listener every notification the person lets it see;
//! Sioul decides, notification by notification, whether it comes now or
//! waits, and Android holds it (it is snoozed: Android brings it back at its
//! time, whole, with its own tap and actions; Sioul never cancels one).
//!
//! Two kinds, decided per notification, not per app:
//! - **between people**: SMS, chats, mail, a missed call. Who wrote, as far
//!   as the app says it (a number, an address, a contact of the phone's
//!   address book, else a name), then what reaches you when (the matrix of
//!   `attention`, its Messages rows; a mail app's, its Mail rows; a missed
//!   call, its Calls rows). A group is judged as the conversation, on its
//!   own row, never by a member's name; "always through" goes by
//!   conversation, which the app identifies, never by a name, which anyone
//!   can take (research 9.3); the blocked never, whatever a conversation says.
//! - **from an automaton**: everything else (shops, news, social networks,
//!   a browser's sites). Gathered: it comes at the next of the times set for
//!   that, `[reminders] gathered` (three a day unless set otherwise, as the
//!   sites' notifications on a computer), never while you sleep, pause or
//!   take free time.
//!
//! When each kind may come is the matrix of what reaches you's to say
//! (`attention`: its people's rows, the rows of automatons and of the apps
//! you set to come at once); the times above are its usual values.
//!
//! Never held: calls, alarms, what runs (music, a call in progress, a
//! download), the reminders you set, Sioul's own; codes and sign-ins or
//! payments to approve (`codes::detect` on the notification's words, the
//! approval phrases below, and Android 15's redaction, which says the
//! phone's own assistant judged it a code).
//!
//! What is kept, on the phone alone and never shared: the person's choices
//! (`Choices`, `app-notes.toml` in the configuration), and what Sioul held,
//! by a hash of Android's key, with the apps' and conversations' names it
//! saw in the last weeks, so that the person can choose for them (`Ledger`,
//! `app-notes-seen.toml` in the state). Never what a notification says.

use crate::areas::{Area, in_view};
use crate::attention::{self, Attention, Event, Level, Source};
use crate::i18n::Translator;
use crate::quiet::Mode;
use crate::reach::{Channel, Who};
use jiff::Zoned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Sioul's own package: its notifications are never touched.
pub const OWN: &str = "com.aurelienpierre.sioul";
/// A gathered time still counts this long after it (seconds): what arrives
/// in the very minute of a gathering comes with it.
pub const GRACE: i64 = 60;
/// How far ahead a moment is looked for (days).
pub const HORIZON_DAYS: i64 = 8;
/// Held with no moment found in the coming days (a pause, which lasts until
/// you come back): asked again this late (seconds). Sioul brings it back
/// sooner when what moves its time changes (the pause ends: `review`).
pub const RECHECK: i64 = 6 * 3600;
/// Do-not-disturb's switch or a focus session without a known end: asked again this soon.
pub const GATE_RECHECK: i64 = 30 * 60;
/// Held for good (a blocked sender, a conversation set to never): looked at again weekly.
pub const FOR_GOOD: i64 = 7 * 86_400;
/// Held again on its return for less than this (seconds)…
pub const AGAIN_SHORT: i64 = 5 * 60;
/// …this many times in a row, it is let through rather than held again and
/// again: the times worked out disagree with the time it is. A long hold (a
/// pause, the night) is no such loop and never counts.
pub const AGAIN_MAX: u32 = 3;
/// Apps and sites seen are forgotten this long after (seconds), unless chosen for.
const SEEN_KEPT: i64 = 30 * 86_400;
/// Conversations seen, the same.
const TALK_KEPT: i64 = 7 * 86_400;
/// At most so many conversations kept seen.
const TALKS_MAX: usize = 60;
/// What came back is said on the Porch this long after.
const BACK_SAID: i64 = 2 * 3600;

// ---------------------------------------------------------------- what Java hands

/// A person as an app names them in its notification (`android.app.Person`),
/// with what the phone's address book says of them when Java could ask it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Person {
    pub name: String,
    /// The app's own key for them.
    pub key: String,
    /// "tel:+33…", "mailto:…", a contact's lookup address, or nothing.
    pub uri: String,
    /// The phone's address book: the numbers and addresses of the contact
    /// its lookup address names.
    pub numbers: Vec<String>,
    pub emails: Vec<String>,
    /// The phone's address book again: those of the one contact with this
    /// name, when the app gave a name alone. Counted only for an app that
    /// names people as the address book does (`Sender::Named::book`).
    pub by_name: Vec<String>,
    pub emails_by_name: Vec<String>,
    pub bot: bool,
}

/// One message of a conversation (`Notification.MessagingStyle`); no sender: the person themselves.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Message {
    pub sender: Option<Person>,
    pub text: String,
}

/// A notification as the listener hands it (AppNotes.java): what Android
/// says of it, its words included. Read in memory to decide, never kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Posted {
    /// Android's key: the same notification updated keeps it.
    pub key: String,
    pub package: String,
    /// The app's name as the phone shows it.
    pub app: String,
    pub channel: String,
    pub channel_name: String,
    /// It sounded or vibrated as it came (`Ranking.getLastAudiblyAlertedMillis`).
    pub alerted: bool,
    /// Its importance (`Ranking.getImportance`): under 3, its channel makes
    /// no sound (made silent); unknown, under 0.
    #[serde(default = "unknown_importance")]
    pub importance: i32,
    /// Android's own do-not-disturb held it (no sound, no banner).
    pub intercepted: bool,
    /// `Notification.category`: "msg", "email", "call", "promo"…
    pub category: String,
    pub tag: String,
    pub group: String,
    /// A group's summary: Android takes it with its last child.
    pub summary: bool,
    /// Ongoing, a foreground service's, or not clearable: never touched.
    pub ongoing: bool,
    /// A full-screen intent: a call ringing, an alarm.
    pub full_screen: bool,
    /// "android.app.Notification$MessagingStyle", "…$BigTextStyle"…
    pub template: String,
    pub title: String,
    pub text: String,
    pub sub: String,
    pub big: String,
    pub lines: Vec<String>,
    /// The conversation's title (a group's name, `<server> #<channel>`).
    pub conversation: String,
    pub group_conversation: bool,
    /// Android counts it as a conversation (`Ranking.isConversation`).
    pub is_conversation: bool,
    /// The conversation's shortcut.
    pub shortcut: String,
    pub messages: Vec<Message>,
    /// The person themselves, as the conversation names them (`EXTRA_MESSAGING_PERSON`).
    pub user: Option<Person>,
    /// `EXTRA_PEOPLE_LIST`, and the older `EXTRA_PEOPLE` as people with a URI only.
    pub people: Vec<Person>,
    /// The phone's own assistant judged it sensitive (a code), and Android
    /// gave Sioul a redacted copy (Android 15).
    pub redacted: bool,
    /// The phone's default SMS app.
    pub sms_app: bool,
}

fn unknown_importance() -> i32 {
    -1
}

// ---------------------------------------------------------------- what it is

/// Who wrote, as the notification identifies them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sender {
    /// Numbers and addresses: from the app (tel:, mailto:) or from the phone's
    /// address book. `contact`: the phone's address book has them, so they
    /// are someone you know (neutral) even when Sioul's address books do not.
    Known { numbers: Vec<String>, emails: Vec<String>, contact: bool },
    /// A name only. `book`: the app names people as the phone's address book
    /// does (SMS apps, WhatsApp): a name that says who; else a name anyone
    /// can take, which may only hold more (`named`).
    Named { name: String, book: bool },
    /// Nobody named.
    Unknown,
}

/// A conversation, as the app identifies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    /// Hashed, the app's own id within (its shortcut, else its title): `talk_key`.
    pub key: String,
    /// As the app titles it: a group's name, `<server> #<channel>`, a person's name.
    pub title: String,
    pub group: bool,
}

/// Between people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Talk {
    pub via: Channel,
    pub sender: Sender,
    pub conversation: Option<Conversation>,
    /// The address it came to (a mail app's sub-text), for that address's area.
    pub to: String,
    /// An SMS: a sender named only and found in no address book is a
    /// business's sender id ("AMELI"), an automaton.
    pub sms: bool,
}

/// What a notification is, for whether it may come now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Never held: calls, alarms, what runs, the reminders you set, Sioul's own.
    Untouched,
    /// A code, a sign-in or a payment to approve: at once, whatever the time.
    Code,
    /// The app or the site set to come at once.
    AtOnce,
    People(Talk),
    /// From an automaton: the app, or a site in a browser (its host).
    Automaton { site: String },
}

/// Categories never held: what runs or shows a state, what the person set.
const UNTOUCHED: &[&str] = &[
    "call", "alarm", "navigation", "transport", "stopwatch", "location_sharing", "sys", "service", "progress", "status", "err", "car_emergency",
    "car_warning", "car_information", "workout", "reminder", "event",
];

/// Apps that name people as the phone's address book does: their names say who.
const BOOK_APPS: &[&str] = &["com.whatsapp", "com.whatsapp.w4b"];

/// SMS apps, besides the phone's default one (`Posted::sms_app`).
const SMS_APPS: &[&str] = &[
    "com.google.android.apps.messaging", "com.android.messaging", "com.android.mms", "com.samsung.android.messaging", "org.fossify.messages",
    "com.simplemobiletools.smsmessenger", "com.moez.QKSMS", "dev.octoshrimpy.quik", "foundation.e.message",
];

/// Mail apps: their notifications are mail, judged on the Mail row.
const MAIL_APPS: &[&str] = &[
    "ch.protonmail.android", "com.google.android.gm", "com.fsck.k9", "net.thunderbird.android", "net.thunderbird.android.beta", "eu.faircode.email",
    "com.microsoft.office.outlook", "foundation.e.mail", "de.tutao.tutanota", "org.kman.AquaMail", "com.samsung.android.email.provider",
    "com.yahoo.mobile.client.android.mail", "me.proton.android.mail",
];

/// Firefox's browsers post every site's notification on this one channel, the site in the sub-text.
const FIREFOX_SITES: &str = "mozac.feature.webnotifications.generic.channel";

/// What asks for an approval now: a sign-in, a payment, an identity to
/// confirm. Read folded (case and accents aside), as whole words.
const APPROVALS: &[&str] = &[
    "approve this sign-in", "approve sign-in", "approve the sign-in", "approve your sign-in", "approve login", "approve the login", "approve your login",
    "approve this login", "approve the payment", "approve your payment", "approve this payment", "approve the purchase", "approve this purchase",
    "approve the transaction", "approve this transaction", "are you trying to sign in",
    "trying to sign in", "trying to log in", "confirm it's you", "confirm it is you", "verify it's you", "verify it is you", "confirm your identity",
    "sign-in request", "sign in request", "login request", "login attempt", "sign-in attempt", "new sign-in", "new login", "3-d secure", "3d secure",
    "strong authentication", "authenticate the payment", "confirm the payment", "confirm your payment", "confirm the transaction",
    "confirmez qu'il s'agit de vous", "confirmez votre identite", "demande de connexion",
    "tentative de connexion", "nouvelle connexion", "validez votre connexion", "valider votre connexion", "validez la connexion",
    "validez votre paiement", "valider votre paiement", "validez le paiement", "validez votre achat", "validez l'achat", "validez votre operation",
    "valider votre operation", "validez l'operation", "validez cette operation", "confirmez votre paiement", "confirmez le paiement",
    "confirmez votre operation", "confirmez l'operation", "authentifiez", "authentification forte", "authentification requise",
    "approuvez la connexion", "approuver la connexion", "approuvez le paiement", "certicode", "securipass", "secur'pass", "pass securite",
];

/// "Is this you?": a sign-in's question only beside what it is about (a
/// friend's "is this you on the photo?" is none).
const ASKS: &[&str] = &["is it you", "is this you", "was this you", "was it you", "est-ce bien vous", "c'est bien vous", "est-ce vous", "c'etait vous"];
/// What such a question is about.
const ASKED_ABOUT: &[&str] = &[
    "sign in", "sign-in", "signing in", "log in", "login", "logging in", "new device", "payment", "purchase", "transaction", "account",
    "connexion", "connecter", "nouvel appareil", "paiement", "achat", "operation", "compte",
];

/// A channel named for codes or sign-ins (Proton Mail's "login" channel): its notifications are never held.
const CODE_CHANNELS: &[&str] = &[
    "otp", "2fa", "mfa", "one-time", "one time", "verification", "verification code", "verification codes", "authentication", "authentification",
    "sign-in", "sign in", "login", "log-in", "logins", "connexion", "connexions",
];

/// Whether a notification hands over a code or asks for an approval now.
pub fn is_code(p: &Posted) -> bool {
    if p.redacted {
        return true;
    }
    let mut body = vec![p.text.as_str(), p.big.as_str(), p.sub.as_str()];
    body.extend(p.lines.iter().map(String::as_str));
    body.extend(p.messages.iter().map(|m| m.text.as_str()));
    let body = body.into_iter().filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n");
    if crate::codes::detect(&p.title, &body).is_some() {
        return true;
    }
    let folded: Vec<char> = crate::text::fold(&format!("{}\n{body}", p.title)).into_iter().map(|c| if c == '\n' { ' ' } else { c }).collect();
    let said = |phrases: &[&str]| phrases.iter().any(|phrase| crate::text::find_word(&folded, phrase, 0).is_some());
    if said(APPROVALS) || (said(ASKS) && said(ASKED_ABOUT)) {
        return true;
    }
    let channel: Vec<char> = crate::text::fold(&format!("{} {}", p.channel.replace(['_', '.'], " "), p.channel_name));
    CODE_CHANNELS.iter().any(|word| crate::text::find_word(&channel, word, 0).is_some())
}

/// A browser's site the notification is from, as its host: Chromium's
/// browsers name it in the tag ("p#https://site#…", "n#…"), the channel
/// ("web:https://site;…") and the group ("Web:site"); Firefox's in the
/// sub-text, on their one channel for sites (research 8.1).
pub fn site_of(p: &Posted) -> Option<String> {
    let tagged = p.tag.strip_prefix("p#").or_else(|| p.tag.strip_prefix("n#")).and_then(|rest| rest.split('#').next());
    let channel = p.channel.strip_prefix("web:").and_then(|rest| rest.split(';').next());
    let grouped = p.group.strip_prefix("Web:");
    let firefox = (p.channel == FIREFOX_SITES).then_some(p.sub.as_str());
    [tagged, channel, grouped, firefox].into_iter().flatten().map(host_of).find(|h| !h.is_empty())
}

/// `https://Mail.Example.org:443/inbox` → `mail.example.org`; `""` when it is no host.
fn host_of(origin: &str) -> String {
    let rest = origin.trim().split_once("://").map_or(origin.trim(), |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.split(':').next().unwrap_or("").trim_end_matches('.').to_lowercase();
    let fine = host.contains('.') && host.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-');
    if fine { host } else { String::new() }
}

/// A phone number as written in a notification: digits, at least seven (a
/// short code has six at most), with spaces, dots, dashes, brackets and a
/// leading "+" allowed ("+262 6 39 98 01 01").
pub fn number_in(text: &str) -> Option<String> {
    let text = text.trim().trim_start_matches('\u{202a}').trim_end_matches('\u{202c}').trim();
    let fine = !text.is_empty() && text.chars().enumerate().all(|(i, c)| c.is_ascii_digit() || " .-()/\u{a0}\u{202f}".contains(c) || (c == '+' && i == 0));
    let digits = text.chars().filter(char::is_ascii_digit).count();
    (fine && digits >= 7).then(|| text.to_string())
}

/// A short code an SMS comes from ("38015", "36179"): a business's, an automaton.
fn short_code(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty() && text.len() <= 6 && text.chars().all(|c| c.is_ascii_digit())
}

/// An e-mail address as an app writes a sender: "alice@example.org", "Alice <alice@example.org>".
fn address_in(text: &str) -> Option<String> {
    let text = text.trim();
    let inner = text.rsplit_once('<').and_then(|(_, rest)| rest.split_once('>')).map_or(text, |(a, _)| a).trim();
    let (local, domain) = inner.split_once('@')?;
    let fine = !local.is_empty() && domain.contains('.') && !inner.contains(char::is_whitespace);
    fine.then(|| inner.to_lowercase())
}

/// What a person is, as the app identifies them: their numbers and
/// addresses (the app's URI, the phone's address book), else their name.
fn sender_of(person: &Person, book: bool) -> Sender {
    let mut numbers = person.numbers.clone();
    let mut emails = person.emails.clone();
    if book && person.uri.trim().is_empty() {
        numbers.extend(person.by_name.iter().cloned());
        emails.extend(person.emails_by_name.iter().cloned());
    }
    let contact = !numbers.is_empty() || !emails.is_empty();
    let uri = person.uri.trim();
    if let Some(number) = uri.strip_prefix("tel:") {
        numbers.insert(0, percent_decoded(number));
    } else if let Some(address) = uri.strip_prefix("mailto:") {
        emails.insert(0, percent_decoded(address).to_lowercase());
    }
    numbers.retain(|n| !n.trim().is_empty());
    emails.retain(|e| !e.trim().is_empty());
    if !numbers.is_empty() || !emails.is_empty() {
        return Sender::Known { numbers, emails, contact };
    }
    // An app's name for someone may be their number or their address itself.
    if let Some(number) = number_in(&person.name) {
        return Sender::Known { numbers: vec![number], emails: Vec::new(), contact: false };
    }
    if let Some(address) = address_in(&person.name) {
        return Sender::Known { numbers: Vec::new(), emails: vec![address], contact: false };
    }
    let name = one_line(&person.name);
    if name.is_empty() { Sender::Unknown } else { Sender::Named { name, book } }
}

/// "%2B262639980101" → "+262639980101": what a URI keeps encoded.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%').then(|| bytes.get(i + 1..i + 3)).flatten().and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// A name on one line: an app's words never break Sioul's.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A notification's key as the ledger keeps it: hashed, Android's own key
/// naming the app and its tags (a site, a chat's id).
pub fn key_hash(key: &str) -> String {
    format!("{:016x}", fnv(key))
}

/// A conversation's key: the app and its own id for it, hashed.
pub fn talk_key(package: &str, id: &str) -> String {
    format!("{:016x}", fnv(&format!("{package}\u{1f}{id}")))
}

/// The conversation a notification belongs to: its shortcut, else its
/// title (a group), else, for a conversation between two, the one who
/// writes; for an SMS app's plain notification, its title (the address
/// book's name, or the number).
fn conversation_of(p: &Posted, sender: Option<&Person>, sms: bool) -> Option<Conversation> {
    // A group as the app says it, or as its messages show it: two people or more write in it, the person aside.
    let mut writers: Vec<String> = p.messages.iter().filter_map(|m| m.sender.as_ref()).filter(|s| !is_user(p, s)).map(|s| if s.key.trim().is_empty() { one_line(&s.name) } else { s.key.trim().to_string() }).filter(|w| !w.is_empty()).collect();
    writers.sort();
    writers.dedup();
    let group = p.group_conversation || writers.len() >= 2;
    let title = [p.conversation.as_str(), sender.map_or("", |s| s.name.as_str()), p.title.as_str()].into_iter().map(one_line).find(|t| !t.is_empty()).unwrap_or_default();
    let from = sender.map_or("", |s| if s.key.trim().is_empty() { s.name.trim() } else { s.key.trim() });
    let plain = if sms { p.title.trim() } else { "" };
    let id = [p.shortcut.trim(), p.conversation.trim(), from, plain].into_iter().find(|i| !i.is_empty())?;
    Some(Conversation { key: talk_key(&p.package, id), title, group })
}

/// Whether a message's sender is the person themselves, as the conversation
/// names them (some apps name them rather than leave the sender out).
fn is_user(p: &Posted, sender: &Person) -> bool {
    p.user.as_ref().is_some_and(|user| {
        let (key, name) = (user.key.trim(), one_line(&user.name));
        (!key.is_empty() && key == sender.key.trim()) || (!name.is_empty() && name == one_line(&sender.name))
    })
}

/// What a notification is (`Kind`), as its app and Android say it, and as
/// the person chose for its app and site (`Choices`).
pub fn classify(p: &Posted, choices: &Choices) -> Kind {
    if p.package == OWN || p.ongoing || p.full_screen || p.summary || UNTOUCHED.contains(&p.category.as_str()) {
        return Kind::Untouched;
    }
    if is_code(p) {
        return Kind::Code;
    }
    let app = choices.app(&p.package);
    if app.kind == AppKind::AtOnce {
        return Kind::AtOnce;
    }
    let site = site_of(p);
    if let Some(site) = &site
        && choices.site.get(site).is_some_and(|s| s.at_once)
    {
        return Kind::AtOnce;
    }
    let automaton = Kind::Automaton { site: site.clone().unwrap_or_default() };
    if app.kind == AppKind::Automaton || (app.kind == AppKind::Usual && site.is_some()) {
        return automaton;
    }
    let mail = MAIL_APPS.contains(&p.package.as_str()) || p.category == "email";
    let sms = p.sms_app || SMS_APPS.contains(&p.package.as_str());
    let via = if matches!(p.category.as_str(), "missed_call" | "voicemail") {
        Channel::Calls
    } else if mail {
        Channel::Mail
    } else {
        Channel::Messages
    };
    let styled = p.template.ends_with("$MessagingStyle") || !p.messages.is_empty() || p.is_conversation || !p.shortcut.trim().is_empty();
    let identified = p.people.iter().any(|person| person.uri.starts_with("tel:") || person.uri.starts_with("mailto:"));
    let between = app.kind == AppKind::People || via != Channel::Messages || styled || identified || sms || p.category == "msg";
    let advert = matches!(p.category.as_str(), "promo" | "social" | "recommendation");
    if !between || (advert && app.kind != AppKind::People) {
        return automaton;
    }
    // SMS apps and the phone's own (a missed call) name people as the address book does.
    let book = sms || via == Channel::Calls || BOOK_APPS.contains(&p.package.as_str());
    // Who wrote: the last message's sender, else the people named, else the title.
    let last = p.messages.iter().rev().find_map(|m| m.sender.as_ref().filter(|s| !is_user(p, s)));
    let person = last.or_else(|| p.people.first());
    // A machine that says it is one (a bank's assistant, a bot in a chat).
    if person.is_some_and(|s| s.bot) && app.kind != AppKind::People {
        return automaton;
    }
    let sender = match person {
        Some(person) => sender_of(person, book),
        None => {
            let title = one_line(&p.title);
            if sms && short_code(&title) {
                return automaton;
            }
            sender_of(&Person { name: title, ..Person::default() }, book)
        }
    };
    if sms && let Sender::Named { name, .. } = &sender
        && short_code(name)
    {
        return automaton;
    }
    let to = if via == Channel::Mail { address_in(&p.sub).unwrap_or_default() } else { String::new() };
    let conversation = if via == Channel::Messages { conversation_of(p, person, sms) } else { None };
    Kind::People(Talk { via, sender, conversation, to, sms })
}

/// Who wrote, from what the address books and lists say of a name
/// (`found`: none when no card has it): an app that names people as the
/// phone's address book does is taken at its word; a name anyone can take
/// may only hold more (restricted and blocked kept), never let through more
/// than someone unknown.
pub fn named(found: Option<Who>, book: bool) -> Who {
    match found {
        Some(who) if book => who,
        Some(who @ (Who::Restricted | Who::Blocked)) => who,
        _ => Who::Stranger,
    }
}

// ---------------------------------------------------------------- the person's choices

/// What an app is, as the person says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppKind {
    /// Notification by notification: between people, or from an automaton.
    #[default]
    Usual,
    /// Messages between people, all of them.
    People,
    /// An automaton's: gathered, all of them.
    Automaton,
    /// At once, as Sioul's own notifications come.
    AtOnce,
}

impl AppKind {
    pub const ALL: [AppKind; 4] = [AppKind::Usual, AppKind::People, AppKind::Automaton, AppKind::AtOnce];

    pub fn id(self) -> &'static str {
        match self {
            AppKind::Usual => "usual",
            AppKind::People => "people",
            AppKind::Automaton => "automaton",
            AppKind::AtOnce => "at-once",
        }
    }

    pub fn read(id: &str) -> Option<AppKind> {
        AppKind::ALL.into_iter().find(|k| k.id() == id)
    }
}

/// When a conversation comes, as the person says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Through {
    /// As who writes says: a group, as people you do not know.
    #[default]
    Usual,
    /// At once, whatever the time: sleep, pauses and do-not-disturb included.
    Always,
    /// With the automatons' notifications, at the gathered times.
    Gathered,
    /// Held for good.
    Never,
}

impl Through {
    pub const ALL: [Through; 4] = [Through::Usual, Through::Always, Through::Gathered, Through::Never];

    pub fn id(self) -> &'static str {
        match self {
            Through::Usual => "usual",
            Through::Always => "always",
            Through::Gathered => "gathered",
            Through::Never => "never",
        }
    }

    pub fn read(id: &str) -> Option<Through> {
        Through::ALL.into_iter().find(|t| t.id() == id)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppChoice {
    #[serde(default)]
    pub kind: AppKind,
    /// What the app is for: its automaton's notifications then come only in
    /// that area's times, and its messages from anyone but your safe people.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<Area>,
    /// Its name, as last seen.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TalkChoice {
    #[serde(default)]
    pub through: Through,
    /// Its title and its app's name, to show it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteChoice {
    #[serde(default)]
    pub at_once: bool,
}

/// The person's choices, this phone's own (`$XDG_CONFIG_HOME/sioul/app-notes.toml`):
/// never shared, like what the listener sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choices {
    /// Holding at all; off, everything comes as the apps send it.
    #[serde(default = "yes")]
    pub hold: bool,
    /// By the app's package.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub app: BTreeMap<String, AppChoice>,
    /// By the conversation's key (`talk_key`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub conversation: BTreeMap<String, TalkChoice>,
    /// By the site's host.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub site: BTreeMap<String, SiteChoice>,
}

fn yes() -> bool {
    true
}

impl Default for Choices {
    fn default() -> Choices {
        Choices { hold: true, app: BTreeMap::new(), conversation: BTreeMap::new(), site: BTreeMap::new() }
    }
}

impl Choices {
    pub fn default_path() -> PathBuf {
        crate::config::config_dir().join("app-notes.toml")
    }

    /// As kept; what does not read is no choice at all (everything as usual).
    pub fn load(path: &Path) -> Choices {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        write_whole(path, &toml::to_string(self).map_err(|e| e.to_string())?)
    }

    pub fn app(&self, package: &str) -> AppChoice {
        self.app.get(package).cloned().unwrap_or_default()
    }

    pub fn through(&self, conversation: &str) -> Through {
        self.conversation.get(conversation).map_or(Through::Usual, |c| c.through)
    }
}

/// Written whole under another name, then renamed: never half a file.
fn write_whole(path: &Path, text: &str) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let temporary = path.with_extension(format!("toml.{}", std::process::id()));
    std::fs::write(&temporary, text).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)
}

// ---------------------------------------------------------------- when

/// What time it is at any moment (`reach::Clock`, through `Live`): sleep, a
/// pause, free time, the hours, as they stand now.
pub trait TimeSource {
    fn mode(&self, at: &Zoned) -> Mode;
    /// Where what time it is changes between `from` and `until`: each
    /// frame's start, in order, after `from`.
    fn starts(&self, from: &Zoned, until: &Zoned) -> Vec<Zoned>;
}

/// The clock as it stands now (`reach::Clock`): what a decision asks of it.
#[derive(Debug, Clone)]
pub struct Live {
    pub clock: crate::reach::Clock,
}

impl TimeSource for Live {
    fn mode(&self, at: &Zoned) -> Mode {
        self.clock.mode(at)
    }

    fn starts(&self, from: &Zoned, until: &Zoned) -> Vec<Zoned> {
        let stamp = from.timestamp().as_second();
        self.clock.frames(from, until).into_iter().filter(|f| f.start > stamp).filter_map(|f| jiff::Timestamp::from_second(f.start).ok()).map(|t| t.to_zoned(from.time_zone().clone())).collect()
    }
}

/// Do-not-disturb's switch or a focus session, holding now: a layer of the
/// matrix (as usual, only Always through's messages come, docs/attention.md).
pub struct Gate {
    /// When it ends, when known (Unix seconds); else asked again in half an hour.
    pub until: Option<i64>,
}

/// What a decision asks of the moment.
pub struct Ask<'a> {
    pub now: &'a Zoned,
    pub clock: &'a dyn TimeSource,
    /// What reaches you when (`attention`, `[attention]`).
    pub attention: &'a Attention,
    /// The gathered times, "09:00" (`[reminders] gathered`).
    pub gathered: &'a [String],
    /// What the app (or the address a mail came to) is for, when said.
    pub area: Option<Area>,
    pub gate: Option<Gate>,
    /// One of today's slots of time for you then (`attention::Slots`): a layer of the matrix.
    pub slot_at: &'a dyn Fn(&Zoned) -> bool,
    /// Free time's "Nothing at all", for this Free time.
    pub nothing: bool,
    /// Where things change that the clock does not know (Unix seconds): the
    /// ends of slots of time for you, of do-not-disturb's switch.
    pub also: &'a [i64],
}

impl Ask<'_> {
    fn stamp(&self) -> i64 {
        self.now.timestamp().as_second()
    }

    /// Do-not-disturb's switch or a focus session holding at `at`.
    fn gated(&self, at: &Zoned) -> bool {
        self.gate.as_ref().is_some_and(|g| at.timestamp().as_second() < g.until.unwrap_or(self.stamp() + GATE_RECHECK))
    }

    /// The moments after now where what may come can change: where what
    /// time it is changes, the ends the clock does not know, the end of
    /// do-not-disturb (or when to ask again, without one), in order.
    fn moments(&self) -> Vec<Zoned> {
        let stamp = self.stamp();
        let Ok(until) = self.now.checked_add(jiff::Span::new().hours(HORIZON_DAYS * 24)) else { return Vec::new() };
        let mut moments = self.clock.starts(self.now, &until);
        let gate_end = self.gate.as_ref().map(|g| g.until.unwrap_or(stamp + GATE_RECHECK));
        for second in self.also.iter().copied().chain(gate_end).filter(|s| *s > stamp && *s < until.timestamp().as_second()) {
            if let Ok(t) = jiff::Timestamp::from_second(second) {
                moments.push(t.to_zoned(self.now.time_zone().clone()));
            }
        }
        moments.sort_by_key(Zoned::timestamp);
        moments.dedup_by_key(|z| z.timestamp());
        moments
    }

    /// The moment at `at` as the matrix reads it: what time it is then, a
    /// slot of time for you, do-not-disturb, Free time's "Nothing at all".
    fn moment(&self, at: &Zoned, mode: &Mode) -> attention::Now {
        let mut now = attention::Now::of(mode).layers((self.slot_at)(at), self.gated(at));
        now.nothing = self.nothing && mode.free();
        now
    }

    /// What the matrix says of a kind at `at`.
    fn level(&self, kind: attention::Kind, at: &Zoned, mode: &Mode) -> Level {
        self.attention.level(attention::Row::Own(kind), &self.moment(at, mode))
    }

    /// Whether the app is for that time (its area), or says nothing of it.
    fn area_fits(&self, mode: &Mode) -> bool {
        self.area.is_none_or(|a| in_view(a, mode.time, mode.week))
    }
}

/// Why a notification comes now or waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// Never held: calls, alarms, what runs, the reminders you set.
    Untouched,
    /// A code, a sign-in or a payment to approve.
    Code,
    /// Holding is off.
    Off,
    /// The app or the site comes at once.
    AtOnce,
    /// Someone who may reach you now.
    Allowed,
    /// A conversation let through always.
    Always,
    /// Someone who may not reach you now: until they may.
    Waiting,
    /// From an automaton: at the next gathered time.
    Gathered,
    /// Held for good: blocked, or a conversation set to never.
    Never,
    /// Held again and again within minutes on its return: let through.
    Again,
}

impl Why {
    pub fn id(self) -> &'static str {
        match self {
            Why::Untouched => "untouched",
            Why::Code => "code",
            Why::Off => "off",
            Why::AtOnce => "at-once",
            Why::Allowed => "allowed",
            Why::Always => "always",
            Why::Waiting => "waiting",
            Why::Gathered => "gathered",
            Why::Never => "never",
            Why::Again => "again",
        }
    }
}

/// Now, or held until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// Unix seconds; none: it comes now.
    pub until: Option<i64>,
    pub why: Why,
}

impl Decision {
    pub fn through(why: Why) -> Decision {
        Decision { until: None, why }
    }

    pub fn held(until: i64, why: Why) -> Decision {
        Decision { until: Some(until), why }
    }
}

/// The first moment from now on, within the coming days, when `fits`
/// holds: now itself, else where something changes (`Ask::moments`).
pub fn first_moment(ask: &Ask, fits: &dyn Fn(&Zoned, &Mode) -> bool) -> Option<Zoned> {
    if fits(ask.now, &ask.clock.mode(ask.now)) {
        return Some(ask.now.clone());
    }
    ask.moments().into_iter().find(|at| fits(at, &ask.clock.mode(at)))
}

/// The gathered times as times of day, in order; "8:30" reads as 08:30.
/// None that reads: 09:00, 13:00 and 18:00.
pub fn times_of(gathered: &[String]) -> Vec<jiff::civil::Time> {
    let read = |text: &str| {
        let (h, m) = text.trim().split_once([':', 'h', 'H'])?;
        let (h, m) = (h.trim().parse::<i8>().ok()?, if m.trim().is_empty() { 0 } else { m.trim().parse::<i8>().ok()? });
        jiff::civil::Time::new(h, m, 0, 0).ok()
    };
    let mut times: Vec<jiff::civil::Time> = gathered.iter().filter_map(|t| read(t)).collect();
    if times.is_empty() {
        times = [9, 13, 18].iter().filter_map(|h| jiff::civil::Time::new(*h, 0, 0, 0).ok()).collect();
    }
    times.sort();
    times.dedup();
    times
}

/// The next gathering from now in which an automaton's notification may
/// come: one the notification matrix gathers at (as usual, not while you
/// sleep, pause or take free time, nor in a slot of time for you or under
/// do-not-disturb), within its app's area. One that began less than a minute
/// ago counts. None within the coming days.
pub fn next_gathering(ask: &Ask) -> Option<Zoned> {
    let times = times_of(ask.gathered);
    let zone = ask.now.time_zone().clone();
    let stamp = ask.stamp();
    for day in 0..=HORIZON_DAYS {
        let Ok(date) = ask.now.date().checked_add(jiff::Span::new().days(day)) else { continue };
        for time in &times {
            let Ok(at) = date.to_datetime(*time).to_zoned(zone.clone()) else { continue };
            if at.timestamp().as_second() + GRACE <= stamp {
                continue;
            }
            let mode = ask.clock.mode(&at);
            if ask.level(attention::Kind::AppAutomatons, &at, &mode) == Level::Gathered && ask.area_fits(&mode) {
                return Some(at);
            }
        }
    }
    None
}

/// Held until the next gathering; through when it is this very one.
fn gathered(ask: &Ask) -> Decision {
    match next_gathering(ask) {
        Some(at) if at.timestamp().as_second() <= ask.stamp() => Decision::through(Why::Gathered),
        Some(at) => Decision::held(at.timestamp().as_second(), Why::Gathered),
        None => Decision::held(ask.stamp() + RECHECK, Why::Gathered),
    }
}

/// At once, at the times the matrix lets these apps come (as usual, as
/// Sioul's own notifications come): else when it does.
fn at_once(ask: &Ask) -> Decision {
    match first_moment(ask, &|at, mode| ask.level(attention::Kind::AppAtOnce, at, mode) == Level::Now && ask.area_fits(mode)) {
        Some(at) if at.timestamp().as_second() <= ask.stamp() => Decision::through(Why::AtOnce),
        Some(at) => Decision::held(at.timestamp().as_second(), Why::AtOnce),
        None => Decision::held(ask.stamp() + RECHECK, Why::AtOnce),
    }
}

/// Someone's message: now when the matrix lets them through
/// (`attention::Attention::decide`: their row on its channel, a group's own
/// row, Always through's, the time, its layers), the notification's area
/// fitting now for all but your safe people and Always through (as mail to an
/// address for another time, docs/areas.md); else at their next time.
fn person(talk: &Talk, who: Who, group: bool, always: bool, ask: &Ask) -> Decision {
    if who == Who::Blocked {
        return Decision::held(ask.stamp() + FOR_GOOD, Why::Never);
    }
    let open = |at: &Zoned, mode: &Mode, area: Option<Area>| ask.attention.decide(&Event::of(Source::Message { via: talk.via, who, group, always }).for_area(area), &ask.moment(at, mode)).told;
    // Its area and its times never meeting in the coming days, the matrix alone decides: nothing waits for good.
    let next = first_moment(ask, &|at, mode| open(at, mode, ask.area)).or_else(|| first_moment(ask, &|at, mode| open(at, mode, None)));
    match next {
        Some(at) if at.timestamp().as_second() <= ask.stamp() => Decision::through(Why::Allowed),
        Some(at) => Decision::held(at.timestamp().as_second(), Why::Waiting),
        None => Decision::held(ask.stamp() + RECHECK, Why::Waiting),
    }
}

/// A conversation let through always: Always through's row (at once at any
/// time, as usual), read against who writes, or the group's row.
fn always(talk: &Talk, who: Who, group: bool, ask: &Ask) -> Decision {
    match person(talk, who, group, true, ask) {
        Decision { until: None, why: Why::Allowed } => Decision::through(Why::Always),
        other => other,
    }
}

/// Whether a notification comes now, or until when it waits.
/// `who`: the sender as the address books and lists judge them (`named` for
/// a name), none when nobody knows them; `listed`: the Always through list
/// holds them. The blocked never, whatever their conversation says (blocked
/// beats Always through, docs/attention.md, Q3).
pub fn decide(kind: &Kind, who: Option<Who>, listed: bool, choices: &Choices, ask: &Ask) -> Decision {
    match kind {
        Kind::Untouched => Decision::through(Why::Untouched),
        Kind::Code => Decision::through(Why::Code),
        _ if !choices.hold => Decision::through(Why::Off),
        Kind::AtOnce => at_once(ask),
        Kind::Automaton { .. } => gathered(ask),
        Kind::People(talk) => {
            if who == Some(Who::Blocked) {
                return Decision::held(ask.stamp() + FOR_GOOD, Why::Never);
            }
            // A group is judged as the conversation, on its own row: anyone in it may write.
            let group = talk.conversation.as_ref().is_some_and(|c| c.group);
            let through = talk.conversation.as_ref().map_or(Through::Usual, |c| choices.through(&c.key));
            match through {
                Through::Always => always(talk, who.unwrap_or(Who::Stranger), group, ask),
                Through::Gathered => gathered(ask),
                Through::Never => Decision::held(ask.stamp() + FOR_GOOD, Why::Never),
                Through::Usual => {
                    // An SMS named only, whom no address book knows: a business's sender id.
                    if talk.sms && who.is_none() && matches!(talk.sender, Sender::Named { .. }) {
                        return gathered(ask);
                    }
                    person(talk, who.unwrap_or(Who::Stranger), group, listed && !group, ask)
                }
            }
        }
    }
}

// ---------------------------------------------------------------- what Sioul held

/// What a held notification waits for, to work its time out again when
/// something changes (a pause ends, the hours or the choices change: `review`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    /// The app's package and name.
    pub app: String,
    pub label: String,
    /// "people", "gathered", "at-once", "never".
    pub kind: String,
    /// The channel (`Channel::id`) and who wrote (`Who::id`), for a message.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub via: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub who: String,
    /// The Always through list holds the sender.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub admitted: bool,
    /// A group's conversation: its own row, whoever writes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub group: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub conversation: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub site: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<Area>,
    pub since: i64,
    pub until: i64,
    /// Held again on its return so many times.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub again: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// An app as Sioul saw it last: its name, when, what came of it, the channels that rang while held.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenApp {
    pub label: String,
    pub seen: i64,
    /// The last decision: `Why::id`, and until when when held.
    pub why: String,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub until: i64,
    /// Its channels that sounded while Sioul held their notification, by
    /// channel id: their names and when. Made silent, they are taken off.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rang: BTreeMap<String, Rang>,
}

fn is_zero_i64(n: &i64) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rang {
    pub name: String,
    pub at: i64,
}

/// A conversation seen: to choose for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenTalk {
    pub app: String,
    pub label: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub group: bool,
    pub seen: i64,
}

/// A browser's site seen: to choose for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenSite {
    pub app: String,
    pub seen: i64,
}

/// What came back at its time, for the Porch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Back {
    pub at: i64,
    pub apps: Vec<String>,
}

/// What Sioul held and saw, on this phone (`$XDG_STATE_HOME/sioul/app-notes-seen.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// Held now, by `key_hash`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub held: BTreeMap<String, Held>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub apps: BTreeMap<String, SeenApp>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub conversations: BTreeMap<String, SeenTalk>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sites: BTreeMap<String, SeenSite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub back: Vec<Back>,
    /// When the listener last heard from Android (Unix seconds): the access works.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub heard: i64,
}

impl Ledger {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("app-notes-seen.toml")
    }

    /// As kept; empty when missing or unreadable (what was held comes back at its time all the same).
    pub fn load(path: &Path) -> Ledger {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        write_whole(path, &toml::to_string(self).map_err(|e| e.to_string())?)
    }

    /// A notification decided: the app, its conversation or site seen; held, kept to be worked out again.
    pub fn note(&mut self, p: &Posted, kind: &Kind, who: Option<Who>, admitted: bool, area: Option<Area>, decision: &Decision, now: i64) {
        self.heard = now;
        let label = if p.app.trim().is_empty() { p.package.clone() } else { one_line(&p.app) };
        let app = self.apps.entry(p.package.clone()).or_default();
        app.label = label.clone();
        app.seen = now;
        app.why = decision.why.id().to_string();
        app.until = decision.until.unwrap_or(0);
        // A channel that sounded while held: the guide to make it silent. Made silent: no more.
        if !p.channel.is_empty() {
            if decision.until.is_some() && p.alerted && !p.intercepted {
                app.rang.insert(p.channel.clone(), Rang { name: one_line(&p.channel_name), at: now });
            } else if (0..3).contains(&p.importance) {
                app.rang.remove(&p.channel);
            }
        }
        match kind {
            Kind::People(talk) => {
                if let Some(c) = &talk.conversation {
                    self.conversations.insert(c.key.clone(), SeenTalk { app: p.package.clone(), label: label.clone(), title: c.title.clone(), group: c.group, seen: now });
                }
            }
            Kind::Automaton { site } if !site.is_empty() => {
                self.sites.insert(site.clone(), SeenSite { app: p.package.clone(), seen: now });
            }
            _ => {}
        }
        let hash = key_hash(&p.key);
        let Some(until) = decision.until else {
            self.held.remove(&hash);
            return;
        };
        let (kind_id, via, conversation, site) = match kind {
            Kind::People(talk) => ("people", talk.via.id(), talk.conversation.as_ref().map(|c| c.key.clone()).unwrap_or_default(), String::new()),
            Kind::Automaton { site } => ("gathered", "", String::new(), site.clone()),
            Kind::AtOnce => ("at-once", "", String::new(), String::new()),
            _ => ("gathered", "", String::new(), String::new()),
        };
        let kind_id = match decision.why {
            Why::Never => "never",
            Why::Gathered => "gathered",
            _ => kind_id,
        };
        // A group is judged as the conversation, on its own row, whoever wrote in it (`decide`);
        // the one who wrote kept, for the blocked.
        let group = matches!(kind, Kind::People(t) if t.conversation.as_ref().is_some_and(|c| c.group));
        let admitted = admitted && !group;
        let again = self.held.get(&hash).map_or(0, |h| h.again);
        self.held.insert(hash, Held { app: p.package.clone(), label, kind: kind_id.into(), via: via.to_string(), who: who.map(|w| w.id().to_string()).unwrap_or_default(), admitted, group, conversation, site, area, since: now, until, again });
    }

    /// A held notification Android brought back and that comes now: no
    /// longer held, and said on the Porch with what came back with it.
    pub fn came_back(&mut self, hash: &str, now: i64) {
        let Some(held) = self.held.remove(hash) else { return };
        match self.back.last_mut() {
            Some(back) if now - back.at < 120 => {
                if !back.apps.contains(&held.label) {
                    back.apps.push(held.label);
                }
            }
            _ => self.back.push(Back { at: now, apps: vec![held.label] }),
        }
    }

    /// Whether this notification is one Sioul held that Android brings back now (or a little before its time).
    pub fn returning(&self, hash: &str, now: i64) -> bool {
        self.held.get(hash).is_some_and(|h| now >= h.until - 120)
    }

    /// The old forgotten: held marks long past their time (the app took the
    /// notification away meanwhile), apps, sites and conversations not seen
    /// for weeks (unless chosen for), what came back yesterday.
    pub fn forget_old(&mut self, now: i64, choices: &Choices) {
        self.held.retain(|_, h| h.until + 86_400 > now);
        self.apps.retain(|package, a| now - a.seen < SEEN_KEPT || choices.app.contains_key(package));
        self.sites.retain(|host, s| now - s.seen < SEEN_KEPT || choices.site.contains_key(host));
        self.conversations.retain(|key, c| now - c.seen < TALK_KEPT || choices.conversation.contains_key(key));
        if self.conversations.len() > TALKS_MAX {
            let mut by_age: Vec<(i64, String)> = self.conversations.iter().filter(|(k, _)| !choices.conversation.contains_key(*k)).map(|(k, c)| (c.seen, k.clone())).collect();
            by_age.sort();
            for (_, key) in by_age.into_iter().take(self.conversations.len() - TALKS_MAX) {
                self.conversations.remove(&key);
            }
        }
        for app in self.apps.values_mut() {
            app.rang.retain(|_, r| now - r.at < TALK_KEPT);
        }
        self.back.retain(|b| now - b.at < 86_400);
    }
}

/// A held notification's time worked out again, as things stand now (a
/// pause ended, the hours or the choices changed): its new time, none when it
/// may come now. `who`, `admitted` and the rest as kept when it was held.
pub fn again(held: &Held, choices: &Choices, ask: &Ask) -> Decision {
    if !choices.hold {
        return Decision::through(Why::Off);
    }
    let app = choices.app(&held.app);
    if app.kind == AppKind::AtOnce || (!held.site.is_empty() && choices.site.get(&held.site).is_some_and(|s| s.at_once)) {
        return at_once(ask);
    }
    let who = Who::read(&held.who);
    // The blocked never, whatever their conversation says.
    if who == Some(Who::Blocked) {
        return Decision::held(held.until.max(ask.stamp() + 60), Why::Never);
    }
    let talk = Talk { via: Channel::read(&held.via).unwrap_or(Channel::Messages), sender: Sender::Unknown, conversation: None, to: String::new(), sms: false };
    if !held.conversation.is_empty() {
        match choices.through(&held.conversation) {
            Through::Always => return always(&talk, who.unwrap_or(Who::Stranger), held.group, ask),
            Through::Gathered => return gathered(ask),
            Through::Never => return Decision::held(held.until.max(ask.stamp() + 60), Why::Never),
            Through::Usual => {}
        }
    }
    match held.kind.as_str() {
        _ if app.kind == AppKind::Automaton => gathered(ask),
        "people" | "never" => person(&talk, who.unwrap_or(Who::Stranger), held.group, held.admitted, ask),
        _ => gathered(ask),
    }
}

// ---------------------------------------------------------------- words

/// "Discord", "Discord and Le Monde", "Discord, Le Monde and WhatsApp",
/// "Discord, Le Monde, WhatsApp and two other apps": numbers in words,
/// "many other apps" past twelve.
pub fn apps_in_words(tr: &Translator, labels: &[String]) -> String {
    let and = tr.text("word-and", None);
    match labels {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} {and} {b}"),
        [a, b, c] => format!("{a}, {b} {and} {c}"),
        [a, b, c, rest @ ..] => {
            let others = if rest.len() > 12 { tr.text("appnotes-others-many", None) } else { tr.text("appnotes-others", Some(&tr.counted(rest.len()))) };
            format!("{a}, {b}, {c} {and} {others}")
        }
    }
}

/// The Porch's line on other apps (docs/porch.md): what waits and until
/// when, what came back lately. Names and words, never a count past twelve.
pub fn porch_line(tr: &Translator, ledger: &Ledger, now: &Zoned) -> String {
    let stamp = now.timestamp().as_second();
    let mut sentences = Vec::new();
    let mut waiting: Vec<&Held> = ledger.held.values().filter(|h| h.kind != "never" && h.until > stamp).collect();
    waiting.sort_by_key(|h| (h.until, h.label.clone()));
    if let Some(first) = waiting.first() {
        let mut labels: Vec<String> = Vec::new();
        for h in &waiting {
            if !labels.contains(&h.label) {
                labels.push(h.label.clone());
            }
        }
        let mut args = crate::i18n::args();
        args.set("apps", apps_in_words(tr, &labels));
        args.set("when", when_text(tr, first.until, now));
        let one_time = waiting.iter().all(|h| h.until == first.until);
        sentences.push(tr.text(if one_time { "appnotes-porch-held" } else { "appnotes-porch-held-first" }, Some(&args)));
    }
    if let Some(back) = ledger.back.iter().rev().find(|b| stamp - b.at < BACK_SAID && !b.apps.is_empty()) {
        let mut args = crate::i18n::args();
        args.set("apps", apps_in_words(tr, &back.apps));
        args.set("when", when_text(tr, back.at, now));
        sentences.push(tr.text("appnotes-porch-back", Some(&args)));
    }
    sentences.join(" ")
}

/// "13:00", "tomorrow at 09:00", "Monday 12 October at 09:00".
fn when_text(tr: &Translator, at: i64, now: &Zoned) -> String {
    match jiff::Timestamp::from_second(at) {
        Ok(t) => crate::quiet::until_text(tr, &t.to_zoned(now.time_zone().clone()), now),
        Err(_) => String::new(),
    }
}

/// Why an app's last notification came or waits, in a sentence (the setup page).
pub fn why_text(tr: &Translator, why: &str, until: i64, now: &Zoned) -> String {
    let mut args = crate::i18n::args();
    args.set("when", if until > 0 { when_text(tr, until, now) } else { String::new() });
    let held = until > now.timestamp().as_second();
    let id = match (why, held) {
        ("waiting", true) | ("gathered", true) | ("at-once", true) | ("never", true) => format!("appnotes-why-{why}-held"),
        _ => format!("appnotes-why-{why}"),
    };
    tr.text(&id, Some(&args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::{Column, Person, Row};
    use crate::needs::{Days, Needs};
    use crate::quiet::{Blocks, Overrides};
    use crate::window::AdminWindow;

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    /// Working hours Monday to Friday 09:00–17:00, Health's meals, naps and
    /// night (bed at 23:00, winding down from 22:00, waking at 07:00), the
    /// overrides given (a pause, free time).
    struct TestClock {
        overrides: Overrides,
    }

    impl TestClock {
        fn new() -> TestClock {
            TestClock { overrides: Overrides::default() }
        }
    }

    fn week() -> Vec<AdminWindow> {
        ["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: None }).collect()
    }

    impl TimeSource for TestClock {
        fn mode(&self, at: &Zoned) -> Mode {
            let needs = Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() };
            crate::quiet::mode(&week(), &[], &self.overrides, &Blocks::of(&needs, &Days::default(), &[], at), at)
        }

        fn starts(&self, from: &Zoned, until: &Zoned) -> Vec<Zoned> {
            // Every five minutes, kept where what time it is changes.
            let mut out = Vec::new();
            let mut last = self.mode(from);
            let mut at = from.clone();
            while at.timestamp() < until.timestamp() {
                at = at.checked_add(jiff::Span::new().minutes(5)).unwrap();
                let mode = self.mode(&at);
                if (mode.time, mode.reason.clone()) != (last.time, last.reason.clone()) {
                    out.push(at.clone());
                    last = mode;
                }
            }
            out
        }
    }

    fn never_slot(_: &Zoned) -> bool {
        false
    }

    /// The matrix of what reaches you as usual, but the neutral in leisure on
    /// mail and messages too (as these tests' people come then).
    fn usual() -> &'static Attention {
        static USUAL: std::sync::OnceLock<Attention> = std::sync::OnceLock::new();
        USUAL.get_or_init(|| {
            let mut matrix = Attention::usual();
            for channel in [Channel::Mail, Channel::Messages] {
                matrix.set(Row::People(channel, Person::Neutral), Column::Leisure, Level::Now).unwrap();
            }
            matrix
        })
    }

    fn ask<'a>(now: &'a Zoned, clock: &'a TestClock, area: Option<Area>, gate: Option<Gate>) -> Ask<'a> {
        static GATHERED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        let gathered = GATHERED.get_or_init(|| vec!["09:00".into(), "13:00".into(), "18:00".into()]);
        Ask { now, clock, attention: usual(), gathered, area, gate, slot_at: &never_slot, nothing: false, also: &[] }
    }

    fn posted(json: &str) -> Posted {
        serde_json::from_str(json).unwrap()
    }

    fn until_of(d: Decision) -> String {
        d.until.map(|u| jiff::Timestamp::from_second(u).unwrap().to_zoned(jiff::tz::TimeZone::get("Europe/Paris").unwrap()).datetime().to_string()).unwrap_or_default()
    }

    // Fixtures, as AppNotes.java hands them (invented people and numbers).
    const SMS_NUMBER: &str = r#"{"key":"0|com.google.android.apps.messaging|1|sms|10001","package":"com.google.android.apps.messaging","app":"Messages","category":"msg","template":"android.app.Notification$MessagingStyle","title":"+262 6 39 98 01 01","text":"Tu es où ?","messages":[{"sender":{"name":"+262 6 39 98 01 01","key":"+262639980101"},"text":"Tu es où ?"}],"sms_app":true,"shortcut":"conv-41"}"#;
    const SMS_CONTACT: &str = r#"{"key":"0|com.google.android.apps.messaging|2|sms|10001","package":"com.google.android.apps.messaging","app":"Messages","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Maman","messages":[{"sender":{"name":"Maman","uri":"tel:%2B262639980102"},"text":"Rappelle-moi"}],"sms_app":true}"#;
    const SMS_SENDER_ID: &str = r#"{"key":"0|foundation.e.message|3|null|10002","package":"foundation.e.message","app":"Message","title":"AMELI","text":"Votre attestation est disponible dans votre compte.","messages":[{"sender":{"name":"AMELI"},"text":"Votre attestation est disponible dans votre compte."}],"sms_app":true}"#;
    const SMS_SHORT: &str = r#"{"key":"0|foundation.e.message|4|null|10002","package":"foundation.e.message","app":"Message","title":"38015","text":"Votre colis arrive demain entre 9h et 13h.","sms_app":true}"#;
    const SMS_CODE: &str = r#"{"key":"0|foundation.e.message|5|null|10002","package":"foundation.e.message","app":"Message","title":"BANQUE","text":"Votre code de vérification : 482913. Ne le communiquez à personne.","sms_app":true}"#;
    const WHATSAPP_PERSON: &str = r#"{"key":"0|com.whatsapp|1|262639980103@s.whatsapp.net|10100","package":"com.whatsapp","app":"WhatsApp","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Jeanne Martin","shortcut":"262639980103@s.whatsapp.net","is_conversation":true,"messages":[{"sender":{"name":"Jeanne Martin","key":"1","uri":"content://com.android.contacts/contacts/lookup/0r1-ABC/1","numbers":["+262 6 39 98 01 03"]},"text":"On se voit demain ?"}]}"#;
    const WHATSAPP_GROUP: &str = r#"{"key":"0|com.whatsapp|1|120363000000000001@g.us|10100","package":"com.whatsapp","app":"WhatsApp","category":"msg","template":"android.app.Notification$MessagingStyle","title":"École Jules Ferry","conversation":"École Jules Ferry","group_conversation":true,"shortcut":"120363000000000001@g.us","is_conversation":true,"messages":[{"sender":{"name":"Mme Durand"},"text":"Sortie annulée demain, les enfants restent à l'école."}]}"#;
    const DISCORD_SERVER: &str = r##"{"key":"0|com.discord|7|null|10200","package":"com.discord","app":"Discord","channel":"messages","channel_name":"Messages","category":"msg","group":"GROUP_MESSAGE_CREATE","template":"android.app.Notification$MessagingStyle","title":"Rust FR #général","conversation":"Rust FR #général","group_conversation":true,"shortcut":"110022003344556677","is_conversation":true,"messages":[{"sender":{"name":"ferris","key":"ferris"},"text":"Nouvelle version ce soir"}]}"##;
    const DISCORD_DM: &str = r#"{"key":"0|com.discord|8|null|10200","package":"com.discord","app":"Discord","channel":"dm","category":"msg","template":"android.app.Notification$MessagingStyle","title":"alice","shortcut":"110022003344556688","is_conversation":true,"messages":[{"sender":{"name":"alice","key":"alice"},"text":"tu as vu ?"}]}"#;
    const CHROME_SITE: &str = r#"{"key":"0|com.android.chrome|-1|p#https://forum.example.net#2|10300","package":"com.android.chrome","app":"Chrome","tag":"p#https://forum.example.net#2","channel":"web:https://forum.example.net;1759000000000","group":"Web:forum.example.net","title":"Nouvelle réponse","text":"Quelqu'un a répondu à votre sujet","sub":"forum.example.net"}"#;
    const FIREFOX_SITE: &str = r#"{"key":"0|org.mozilla.fennec_fdroid|5|a1b2|10301","package":"org.mozilla.fennec_fdroid","app":"Fennec","channel":"mozac.feature.webnotifications.generic.channel","title":"New post","text":"Your feed has news","sub":"news.example.com"}"#;
    const PROTON: &str = r#"{"key":"0|ch.protonmail.android|42|null|10400","package":"ch.protonmail.android","app":"Proton Mail","channel":"v7_email_channel_id","channel_name":"Emails","title":"Alice Martin","text":"Dîner vendredi","sub":"me@proton.example"}"#;
    const PROTON_LOGIN: &str = r#"{"key":"0|ch.protonmail.android|43|null|10400","package":"ch.protonmail.android","app":"Proton Mail","channel":"v7_login_channel_id","channel_name":"Login","title":"New sign-in to your account","text":"A new device signed in."}"#;
    const SHOP: &str = r#"{"key":"0|com.shop.example|1|null|10500","package":"com.shop.example","app":"Shop","category":"promo","channel":"offers","title":"-30 % sur les chaussures","text":"Jusqu'à dimanche seulement","alerted":true}"#;
    const BANK_CODE: &str = r#"{"key":"0|com.bank.example|1|null|10600","package":"com.bank.example","app":"Bank","title":"Your security code","text":"Your security code is 482913. It expires in 5 minutes."}"#;
    const BANK_APPROVAL: &str = r#"{"key":"0|com.bank.example|2|null|10600","package":"com.bank.example","app":"Banque","title":"Paiement en ligne","text":"Validez votre paiement de 42,00 € dans l'application."}"#;
    const REDACTED: &str = r#"{"key":"0|com.otp.example|1|null|10700","package":"com.otp.example","app":"Authenticator","title":"Authenticator","text":"Sensitive notification content hidden","redacted":true}"#;

    #[test]
    fn what_a_notification_is() {
        let choices = Choices::default();
        let talk = |json: &str| match classify(&posted(json), &choices) {
            Kind::People(t) => t,
            other => panic!("{other:?}"),
        };
        // SMS: a number is someone, a contact's tel: URI too; a sender id or a short code is an automaton.
        let sms = talk(SMS_NUMBER);
        assert_eq!((sms.via, sms.sms), (Channel::Messages, true));
        assert_eq!(sms.sender, Sender::Known { numbers: vec!["+262 6 39 98 01 01".into()], emails: vec![], contact: false });
        assert_eq!(talk(SMS_CONTACT).sender, Sender::Known { numbers: vec!["+262639980102".into()], emails: vec![], contact: false });
        assert_eq!(talk(SMS_SENDER_ID).sender, Sender::Named { name: "AMELI".into(), book: true });
        assert_eq!(classify(&posted(SMS_SHORT), &choices), Kind::Automaton { site: String::new() });
        assert_eq!(classify(&posted(SMS_CODE), &choices), Kind::Code);
        // WhatsApp: a person by the phone's address book; a group by its conversation.
        let jeanne = talk(WHATSAPP_PERSON);
        assert_eq!(jeanne.sender, Sender::Known { numbers: vec!["+262 6 39 98 01 03".into()], emails: vec![], contact: true }, "the phone's contact");
        let school = talk(WHATSAPP_GROUP);
        let conversation = school.conversation.clone().unwrap();
        assert_eq!((conversation.title.as_str(), conversation.group), ("École Jules Ferry", true));
        assert_eq!(conversation.key, talk_key("com.whatsapp", "120363000000000001@g.us"));
        assert_eq!(school.sender, Sender::Named { name: "Mme Durand".into(), book: true });
        // A name alone, found in the phone's address book: WhatsApp's counts, Discord's does not.
        let by_name = r#"{"key":"k","package":"com.whatsapp","app":"WhatsApp","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Paul","messages":[{"sender":{"name":"Paul","by_name":["+262 6 39 98 01 04"]},"text":"Salut"}]}"#;
        assert_eq!(talk(by_name).sender, Sender::Known { numbers: vec!["+262 6 39 98 01 04".into()], emails: vec![], contact: true });
        let discord_by_name = by_name.replace("com.whatsapp", "com.discord");
        assert_eq!(talk(discord_by_name.as_str()).sender, Sender::Named { name: "Paul".into(), book: false });
        // A bot says it is one: an automaton. A six-digit short code is no phone number.
        let bot = r#"{"key":"b","package":"com.bank.example","app":"Bank","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Assistant","messages":[{"sender":{"name":"Assistant","bot":true},"text":"Votre relevé est prêt."}]}"#;
        assert_eq!(classify(&posted(bot), &choices), Kind::Automaton { site: String::new() });
        assert_eq!(classify(&posted(r#"{"key":"s6","package":"foundation.e.message","title":"123456","text":"Votre colis est arrivé.","sms_app":true}"#), &choices), Kind::Automaton { site: String::new() });
        assert_eq!(number_in("06 39 98 01"), Some("06 39 98 01".into()), "eight digits");
        // The person's own replies, named by the app: neither who wrote nor a second writer.
        let replied = talk(r#"{"key":"r","package":"com.discord","category":"msg","user":{"name":"Me","key":"me"},"messages":[{"sender":{"name":"alice","key":"alice"},"text":"tu viens ?"},{"sender":{"name":"Me","key":"me"},"text":"oui"}]}"#);
        assert_eq!(replied.sender, Sender::Named { name: "alice".into(), book: false });
        assert!(!replied.conversation.unwrap().group);
        // Two people writing in a conversation the app does not call a group: a group all the same.
        let two = talk(r#"{"key":"t","package":"org.example.chat","category":"msg","conversation":"Voisins","messages":[{"sender":{"name":"Ana"},"text":"Bonjour"},{"sender":{"name":"Bob"},"text":"Salut"}]}"#);
        assert!(two.conversation.unwrap().group);
        // A missed call: the phone's names are the address book's.
        let missed = talk(r#"{"key":"m","package":"com.android.dialer","category":"missed_call","title":"Maman"}"#);
        assert_eq!((missed.via, missed.sender), (Channel::Calls, Sender::Named { name: "Maman".into(), book: true }));
        // An SMS app's plain notification: its title is the conversation.
        let plain = talk(r#"{"key":"k2","package":"com.android.mms","app":"SMS","title":"Paul","text":"Salut"}"#);
        assert_eq!(plain.conversation.map(|c| c.key), Some(talk_key("com.android.mms", "Paul")));
        // Discord: names anyone can take; a server's channel is a group, a direct message is not.
        let server = talk(DISCORD_SERVER);
        assert!(server.conversation.as_ref().unwrap().group);
        assert_eq!(server.sender, Sender::Named { name: "ferris".into(), book: false });
        let dm = talk(DISCORD_DM);
        assert_eq!((dm.conversation.as_ref().unwrap().group, dm.conversation.as_ref().unwrap().title.as_str()), (false, "alice"));
        // Browsers: the site, by Chromium's tag and channel, by Firefox's sub-text.
        assert_eq!(classify(&posted(CHROME_SITE), &choices), Kind::Automaton { site: "forum.example.net".into() });
        assert_eq!(classify(&posted(FIREFOX_SITE), &choices), Kind::Automaton { site: "news.example.com".into() });
        // Proton Mail: mail, a sender's name, the address it came to; its sign-ins never held.
        let proton = talk(PROTON);
        assert_eq!((proton.via, proton.to.as_str()), (Channel::Mail, "me@proton.example"));
        assert_eq!(proton.sender, Sender::Named { name: "Alice Martin".into(), book: false });
        assert_eq!(classify(&posted(PROTON_LOGIN), &choices), Kind::Code);
        // A shop's promotion: an automaton. A bank's code, its approval, a redacted copy: at once.
        assert_eq!(classify(&posted(SHOP), &choices), Kind::Automaton { site: String::new() });
        for json in [BANK_CODE, BANK_APPROVAL, REDACTED] {
            assert_eq!(classify(&posted(json), &choices), Kind::Code, "{json}");
        }
        let asked = |text: &str| is_code(&Posted { title: "Alice".into(), text: text.into(), ..Posted::default() });
        assert!(asked("Is this you signing in from Lyon?") && asked("Est-ce bien vous qui tentez une connexion ?"));
        assert!(!asked("Is this you on the photo? Haha") && !asked("C'est bien vous sur la photo ?"));
        // Never held: a call, an alarm, what runs, a summary, a reminder you set, Sioul's own.
        for json in [
            r#"{"package":"com.whatsapp","category":"call","title":"Jeanne"}"#,
            r#"{"package":"org.fossify.clock","category":"alarm","full_screen":true}"#,
            r#"{"package":"org.example.music","ongoing":true,"title":"A song"}"#,
            r#"{"package":"com.whatsapp","summary":true,"title":"3 messages"}"#,
            r#"{"package":"ws.xsoh.etar","category":"event","title":"Dentist at 14:00"}"#,
            r#"{"package":"com.aurelienpierre.sioul","title":"Taken"}"#,
        ] {
            assert_eq!(classify(&posted(json), &choices), Kind::Untouched, "{json}");
        }
        // The person's word for an app or a site comes first.
        let mut chosen = Choices::default();
        chosen.app.insert("com.discord".into(), AppChoice { kind: AppKind::Automaton, ..AppChoice::default() });
        chosen.app.insert("com.shop.example".into(), AppChoice { kind: AppKind::AtOnce, ..AppChoice::default() });
        chosen.site.insert("forum.example.net".into(), SiteChoice { at_once: true });
        assert_eq!(classify(&posted(DISCORD_DM), &chosen), Kind::Automaton { site: String::new() });
        assert_eq!(classify(&posted(SHOP), &chosen), Kind::AtOnce);
        assert_eq!(classify(&posted(CHROME_SITE), &chosen), Kind::AtOnce);
        // An app said to carry messages between people: its plain notifications too.
        chosen.app.insert("org.example.old.sms".into(), AppChoice { kind: AppKind::People, ..AppChoice::default() });
        let old = classify(&posted(r#"{"package":"org.example.old.sms","title":"+262 6 39 98 01 05","text":"Coucou"}"#), &chosen);
        assert!(matches!(old, Kind::People(Talk { sender: Sender::Known { .. }, .. })), "{old:?}");
    }

    #[test]
    fn names_anyone_can_take_only_hold_more() {
        assert_eq!(named(Some(Who::Safe), true), Who::Safe, "WhatsApp names people as the address book does");
        assert_eq!(named(Some(Who::Safe), false), Who::Stranger, "a Discord name is anyone's");
        assert_eq!(named(Some(Who::Neutral), false), Who::Stranger);
        assert_eq!(named(Some(Who::Restricted), false), Who::Restricted);
        assert_eq!(named(Some(Who::Blocked), false), Who::Blocked);
        assert_eq!(named(None, true), Who::Stranger);
    }

    #[test]
    fn who_and_when_decide() {
        let clock = TestClock::new();
        let choices = Choices::default();
        let kind = |json: &str| classify(&posted(json), &choices);
        // Tuesday 6 October 2026, 10:00: working hours.
        let morning = at("2026-10-06T10:00[Europe/Paris]");
        let evening = at("2026-10-06T20:30[Europe/Paris]");
        let night = at("2026-10-06T23:30[Europe/Paris]");
        let decide_at = |json: &str, who: Option<Who>, now: &Zoned| decide(&kind(json), who, false, &choices, &ask(now, &clock, None, None));
        // A stranger's SMS: now in work time; in the evening, held until work starts on Wednesday.
        assert_eq!(decide_at(SMS_NUMBER, Some(Who::Stranger), &morning), Decision::through(Why::Allowed));
        let held = decide_at(SMS_NUMBER, Some(Who::Stranger), &evening);
        assert_eq!((held.why, until_of(held).as_str()), (Why::Waiting, "2026-10-07T09:00:00"));
        // Someone in the address book: in the evening yes, during the night no, until… leisure at waking.
        assert_eq!(decide_at(WHATSAPP_PERSON, Some(Who::Neutral), &evening), Decision::through(Why::Allowed));
        let held = decide_at(WHATSAPP_PERSON, Some(Who::Neutral), &night);
        assert_eq!((held.why, until_of(held).as_str()), (Why::Waiting, "2026-10-07T07:00:00"));
        // Safe people in the evening; in the night held until waking (docs/attention.md, Q4: before, let through);
        // on the Always through list, at once; the blocked held for good, on the list or not.
        assert_eq!(decide_at(SMS_CONTACT, Some(Who::Safe), &evening), Decision::through(Why::Allowed));
        let held = decide_at(SMS_CONTACT, Some(Who::Safe), &night);
        assert_eq!((held.why, until_of(held).as_str()), (Why::Waiting, "2026-10-07T07:00:00"));
        assert_eq!(decide(&kind(SMS_CONTACT), Some(Who::Safe), true, &choices, &ask(&night, &clock, None, None)), Decision::through(Why::Allowed));
        assert_eq!(decide_at(SMS_CONTACT, Some(Who::Blocked), &morning).why, Why::Never);
        assert_eq!(decide(&kind(SMS_CONTACT), Some(Who::Blocked), true, &choices, &ask(&morning, &clock, None, None)).why, Why::Never);
        // A business's SMS id nobody knows: gathered; the same name found in a card: a person.
        let ameli = decide_at(SMS_SENDER_ID, None, &morning);
        assert_eq!((ameli.why, until_of(ameli).as_str()), (Why::Gathered, "2026-10-06T13:00:00"));
        assert_eq!(decide_at(SMS_SENDER_ID, Some(Who::Neutral), &morning), Decision::through(Why::Allowed));
        // A group: as people you do not know, whoever wrote in it.
        let group = decide_at(WHATSAPP_GROUP, Some(Who::Safe), &evening);
        assert_eq!((group.why, until_of(group).as_str()), (Why::Waiting, "2026-10-07T09:00:00"));
        // The school's group let through always: in the evening and in the night.
        let mut chosen = Choices::default();
        let school = talk_key("com.whatsapp", "120363000000000001@g.us");
        chosen.conversation.insert(school.clone(), TalkChoice { through: Through::Always, title: "École Jules Ferry".into(), app: "WhatsApp".into() });
        for now in [&evening, &night] {
            assert_eq!(decide(&kind(WHATSAPP_GROUP), None, false, &chosen, &ask(now, &clock, None, None)), Decision::through(Why::Always));
        }
        // Blocked beats Always through (Q3): a blocked person writing there is held (before: let through).
        assert_eq!(decide(&kind(WHATSAPP_GROUP), Some(Who::Blocked), false, &chosen, &ask(&evening, &clock, None, None)).why, Why::Never);
        // A noisy group gathered, another never.
        chosen.conversation.insert(school.clone(), TalkChoice { through: Through::Gathered, ..TalkChoice::default() });
        let gathered = decide(&kind(WHATSAPP_GROUP), None, false, &chosen, &ask(&morning, &clock, None, None));
        assert_eq!((gathered.why, until_of(gathered).as_str()), (Why::Gathered, "2026-10-06T13:00:00"));
        chosen.conversation.insert(school, TalkChoice { through: Through::Never, ..TalkChoice::default() });
        assert_eq!(decide(&kind(WHATSAPP_GROUP), None, false, &chosen, &ask(&morning, &clock, None, None)).why, Why::Never);
        // Codes and untouched ones come whatever the time; holding off, everything comes.
        assert_eq!(decide_at(BANK_CODE, None, &night), Decision::through(Why::Code));
        let off = Choices { hold: false, ..Choices::default() };
        assert_eq!(decide(&kind(SHOP), None, false, &off, &ask(&night, &clock, None, None)), Decision::through(Why::Off));
        // An app at once comes as Sioul's own notifications: now in the evening, at waking in the night.
        let mut at_once = Choices::default();
        at_once.app.insert("com.shop.example".into(), AppChoice { kind: AppKind::AtOnce, ..AppChoice::default() });
        let shop = classify(&posted(SHOP), &at_once);
        assert_eq!(decide(&shop, None, false, &at_once, &ask(&evening, &clock, None, None)), Decision::through(Why::AtOnce));
        let waking = decide(&shop, None, false, &at_once, &ask(&night, &clock, None, None));
        assert_eq!((waking.why, until_of(waking).as_str()), (Why::AtOnce, "2026-10-07T07:00:00"));
    }

    #[test]
    fn mail_and_areas() {
        let clock = TestClock::new();
        let choices = Choices::default();
        let proton = classify(&posted(PROTON), &choices);
        let evening = at("2026-10-06T20:30[Europe/Paris]");
        let morning = at("2026-10-06T10:00[Europe/Paris]");
        // A name only, in Proton Mail: someone unknown; to a work address, in the evening: tomorrow's work.
        let who = named(Some(Who::Safe), false);
        let held = decide(&proton, Some(who), false, &choices, &ask(&evening, &clock, Some(Area::WORK), None));
        assert_eq!((held.why, until_of(held).as_str()), (Why::Waiting, "2026-10-07T09:00:00"));
        // Someone known writing to a leisure address in work time: held until the evening's leisure.
        let held = decide(&proton, Some(Who::Neutral), false, &choices, &ask(&morning, &clock, Some(Area::LEISURE), None));
        assert_eq!((held.why, until_of(held).as_str()), (Why::Waiting, "2026-10-06T17:00:00"));
        // Safe people to any address, any time.
        assert_eq!(decide(&proton, Some(Who::Safe), false, &choices, &ask(&morning, &clock, Some(Area::LEISURE), None)).why, Why::Allowed);
        // An automaton for work: its gatherings in work time only.
        let shop = classify(&posted(SHOP), &choices);
        let held = decide(&shop, None, false, &choices, &ask(&evening, &clock, Some(Area::WORK), None));
        assert_eq!(until_of(held), "2026-10-07T09:00:00");
        // An app for leisure in work time: the 18:00 gathering, after work.
        let held = decide(&shop, None, false, &choices, &ask(&morning, &clock, Some(Area::LEISURE), None));
        assert_eq!(until_of(held), "2026-10-06T18:00:00");
    }

    #[test]
    fn gathered_times_across_midnight_sleep_and_pauses() {
        let clock = TestClock::new();
        let choices = Choices::default();
        let shop = classify(&posted(SHOP), &choices);
        let when = |now: &str, clock: &TestClock| until_of(decide(&shop, None, false, &choices, &ask(&at(now), clock, None, None)));
        // n times a day: 09:00, 13:00, 18:00.
        assert_eq!(when("2026-10-06T08:00[Europe/Paris]", &clock), "2026-10-06T09:00:00");
        assert_eq!(when("2026-10-06T09:30[Europe/Paris]", &clock), "2026-10-06T13:00:00");
        assert_eq!(when("2026-10-06T13:20[Europe/Paris]", &clock), "2026-10-06T18:00:00");
        // After the last one, across midnight, to the first of the next day.
        assert_eq!(when("2026-10-06T18:30[Europe/Paris]", &clock), "2026-10-07T09:00:00");
        // Arriving within the minute of a gathering: it comes with it.
        assert_eq!(decide(&shop, None, false, &choices, &ask(&at("2026-10-06T13:00:30[Europe/Paris]"), &clock, None, None)), Decision::through(Why::Gathered));
        // A gathering in the night, or at lunch, is skipped: never while you sleep; meals are leisure, so 13:00 is free.
        let late = vec!["06:30".to_string(), "13:00".into(), "22:30".into()];
        let ask_late = |now: &'static str| {
            let now = Box::leak(Box::new(at(now)));
            Ask { now, clock: &clock, attention: usual(), gathered: &late, area: None, gate: None, slot_at: &never_slot, nothing: false, also: &[] }
        };
        assert_eq!(until_of(decide(&shop, None, false, &choices, &ask_late("2026-10-06T20:00[Europe/Paris]"))), "2026-10-07T13:00:00", "22:30 and 06:30 are asleep");
        // A pause: nothing gathered until it ends; asked again six hours on, brought back when it ends (`again`).
        let paused = TestClock { overrides: Overrides { paused_since: Some(at("2026-10-06T08:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() } };
        let now = at("2026-10-06T08:30[Europe/Paris]");
        let held = decide(&shop, None, false, &choices, &ask(&now, &paused, None, None));
        assert_eq!((held.why, until_of(held).as_str()), (Why::Gathered, "2026-10-06T14:30:00"));
        // Free time: automatons wait for it to end (the night's start ends it), then the next gathering.
        let free = TestClock { overrides: Overrides { free_since: Some(at("2026-10-06T15:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() } };
        let held = decide(&shop, None, false, &choices, &ask(&at("2026-10-06T16:00[Europe/Paris]"), &free, None, None));
        assert_eq!(until_of(held), "2026-10-07T09:00:00", "18:00 falls in free time, which lasts until the night");
        // Do-not-disturb's switch until 18:30: the 18:00 gathering skipped.
        let gate = Gate { until: Some(at("2026-10-06T18:30[Europe/Paris]").timestamp().as_second()) };
        let held = decide(&shop, None, false, &choices, &ask(&at("2026-10-06T14:00[Europe/Paris]"), &clock, None, Some(gate)));
        assert_eq!(until_of(held), "2026-10-07T09:00:00");
        // A slot of time for you at 13:00: that gathering skipped.
        let slot = |z: &Zoned| z.date() == at("2026-10-06T00:00[Europe/Paris]").date() && z.hour() == 13;
        let now = at("2026-10-06T10:00[Europe/Paris]");
        let usual = vec!["09:00".to_string(), "13:00".into(), "18:00".into()];
        let with_slot = Ask { now: &now, clock: &clock, attention: self::usual(), gathered: &usual, area: None, gate: None, slot_at: &slot, nothing: false, also: &[] };
        assert_eq!(until_of(decide(&shop, None, false, &choices, &with_slot)), "2026-10-06T18:00:00");
        // No time that reads: three a day.
        assert_eq!(times_of(&["later".into()]).len(), 3);
        assert_eq!(times_of(&["8h30".into(), "08:30".into(), " 17:00 ".into()]).iter().map(|t| t.to_string()).collect::<Vec<_>>(), ["08:30:00", "17:00:00"]);
    }

    #[test]
    fn do_not_disturb_lets_its_list_through() {
        let clock = TestClock::new();
        let choices = Choices::default();
        let sms = classify(&posted(SMS_CONTACT), &choices);
        let morning = at("2026-10-06T10:00[Europe/Paris]");
        let gate = || Some(Gate { until: Some(at("2026-10-06T11:00[Europe/Paris]").timestamp().as_second()) });
        // Mum is Always through: she comes during focus; someone neutral waits for its end.
        assert_eq!(decide(&sms, Some(Who::Neutral), true, &choices, &ask(&morning, &clock, None, gate())).why, Why::Allowed);
        let held = decide(&sms, Some(Who::Neutral), false, &choices, &ask(&morning, &clock, None, gate()));
        assert_eq!((held.why, until_of(held).as_str()), (Why::Waiting, "2026-10-06T11:00:00"));
        // A focus session without a known end: asked again in half an hour.
        let open = Some(Gate { until: None });
        let held = decide(&sms, Some(Who::Neutral), false, &choices, &ask(&morning, &clock, None, open));
        assert_eq!(until_of(held), "2026-10-06T10:30:00");
    }

    #[test]
    fn the_matrix_changes_when() {
        let clock = TestClock::new();
        let choices = Choices::default();
        let sms = classify(&posted(SMS_CONTACT), &choices);
        let shop = classify(&posted(SHOP), &choices);
        let gathered = vec!["09:00".to_string(), "13:00".into(), "18:00".into()];
        let decided = |kind: &Kind, who: Option<Who>, listed: bool, matrix: &Attention, now: &Zoned, clock: &TestClock, gate: Option<Gate>| {
            decide(kind, who, listed, &choices, &Ask { now, clock, attention: matrix, gathered: &gathered, area: None, gate, slot_at: &never_slot, nothing: false, also: &[] })
        };
        let mut matrix = usual().clone();
        // Always through under do-not-disturb: at once whatever their row (Q2), as usual; their row's times with ☆.
        let evening = at("2026-10-06T20:30[Europe/Paris]");
        let gate = || Some(Gate { until: Some(at("2026-10-06T21:00[Europe/Paris]").timestamp().as_second()) });
        assert_eq!(decided(&sms, Some(Who::Stranger), true, &matrix, &evening, &clock, gate()).why, Why::Allowed);
        matrix.set(Row::People(Channel::Messages, Person::Always), Column::Leisure, Level::As).unwrap();
        matrix.set(Row::People(Channel::Messages, Person::Always), Column::Dnd, Level::Through).unwrap();
        assert_eq!(decided(&sms, Some(Who::Stranger), true, &matrix, &evening, &clock, gate()).why, Why::Waiting, "a stranger's row in the evening");
        assert_eq!(decided(&sms, Some(Who::Neutral), true, &matrix, &evening, &clock, gate()).why, Why::Allowed, "the neutral come in leisure");
        assert_eq!(decided(&sms, Some(Who::Safe), false, &matrix, &evening, &clock, gate()).why, Why::Waiting, "not on the list: do-not-disturb holds them");
        // Messages from people in Free time: your safe people's come, as usual; held when you say so, until it ends
        // at the night, and the night holds them too (Q4: before, they came at 22:00): at waking.
        let free = TestClock { overrides: Overrides { free_since: Some(at("2026-10-06T15:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() } };
        let afternoon = at("2026-10-06T16:00[Europe/Paris]");
        assert_eq!(decided(&sms, Some(Who::Safe), false, &matrix, &afternoon, &free, None).why, Why::Allowed, "as usual");
        matrix.set(Row::People(Channel::Messages, Person::Safe), Column::Free, Level::Later).unwrap();
        let held = decided(&sms, Some(Who::Safe), false, &matrix, &afternoon, &free, None);
        assert_eq!((held.why, until_of(held).as_str()), (Why::Waiting, "2026-10-07T07:00:00"));
        // Automatons not gathered in working hours: Tuesday's 09:00 and 13:00 skipped, 18:00 kept.
        let morning = at("2026-10-06T08:00[Europe/Paris]");
        assert_eq!(until_of(decided(&shop, None, false, &matrix, &morning, &clock, None)), "2026-10-06T09:00:00", "as usual");
        matrix.set(Row::Own(crate::attention::Kind::AppAutomatons), Column::Work, Level::Later).unwrap();
        assert_eq!(until_of(decided(&shop, None, false, &matrix, &morning, &clock, None)), "2026-10-06T18:00:00");
        // Time for you everywhere (Q12): the listener reads the slots, which hold automatons as the window's did.
        let in_slot = |z: &Zoned| z.hour() == 9;
        let slotted = Ask { now: &morning, clock: &clock, attention: usual(), gathered: &gathered, area: None, gate: None, slot_at: &in_slot, nothing: false, also: &[] };
        assert_eq!(until_of(decide(&shop, None, false, &choices, &slotted)), "2026-10-06T13:00:00");
        // "Nothing at all" in Free time: the safe wait too, Always through comes.
        let nothing = Ask { now: &afternoon, clock: &free, attention: usual(), gathered: &gathered, area: None, gate: None, slot_at: &never_slot, nothing: true, also: &[] };
        assert_eq!(decide(&sms, Some(Who::Safe), false, &choices, &nothing).why, Why::Waiting);
        assert_eq!(decide(&sms, Some(Who::Safe), true, &choices, &nothing).why, Why::Allowed);
        // A mail app's notification takes the Mail rows (Q5): a safe sender's at night waits for waking (before: let through).
        let proton = classify(&posted(PROTON), &choices);
        let night = at("2026-10-06T23:30[Europe/Paris]");
        let held = decide(&proton, Some(Who::Safe), false, &choices, &ask(&night, &clock, None, None));
        assert_eq!((held.why, until_of(held).as_str()), (Why::Waiting, "2026-10-07T07:00:00"));
        // A missed call by the Calls row (Q23): a stranger's in work time, never at night.
        let missed = classify(&posted(r#"{"key":"m","package":"com.android.dialer","category":"missed_call","title":"+262 6 39 98 01 07"}"#), &choices);
        assert_eq!(decide(&missed, Some(Who::Stranger), false, &choices, &ask(&at("2026-10-06T10:00[Europe/Paris]"), &clock, None, None)).why, Why::Allowed);
        assert_eq!(decide(&missed, Some(Who::Stranger), false, &choices, &ask(&night, &clock, None, None)).why, Why::Waiting);
    }

    #[test]
    fn what_was_held_and_came_back() {
        let clock = TestClock::new();
        let choices = Choices::default();
        let now = at("2026-10-06T10:00[Europe/Paris]");
        let stamp = now.timestamp().as_second();
        let mut ledger = Ledger::default();
        for json in [SHOP, DISCORD_SERVER, CHROME_SITE, WHATSAPP_GROUP] {
            let p = posted(json);
            let kind = classify(&p, &choices);
            let decision = decide(&kind, Some(Who::Stranger), false, &choices, &ask(&now, &clock, None, None));
            ledger.note(&p, &kind, Some(Who::Stranger), false, None, &decision, stamp);
        }
        // The shop's sounded while held: its channel to make silent.
        assert_eq!(ledger.apps["com.shop.example"].rang.keys().collect::<Vec<_>>(), ["offers"]);
        assert_eq!(ledger.conversations.len(), 2, "Discord's channel and the school's group");
        assert!(ledger.sites.contains_key("forum.example.net"));
        assert_eq!(ledger.held.len(), 2, "the shop and the site wait for 13:00; strangers write in work time");
        let tr = Translator::new("en");
        assert_eq!(porch_line(&tr, &ledger, &now), "Notifications held until 13:00: Chrome and Shop.");
        let fr = Translator::new("fr");
        assert_eq!(porch_line(&fr, &ledger, &now), "Notifications retenues jusqu’à 13:00\u{202f}: Chrome et Shop.");
        // Android brings them back at 13:00: they come, and the Porch says so for two hours.
        let one = at("2026-10-06T13:00[Europe/Paris]");
        let hash = key_hash(&posted(SHOP).key);
        assert!(ledger.returning(&hash, one.timestamp().as_second()) && !ledger.returning(&hash, stamp));
        ledger.came_back(&hash, one.timestamp().as_second());
        ledger.came_back(&key_hash(&posted(CHROME_SITE).key), one.timestamp().as_second() + 5);
        assert!(ledger.held.is_empty());
        assert_eq!(porch_line(&tr, &ledger, &one), "Back in your notifications at 13:00: Shop and Chrome.");
        assert_eq!(porch_line(&tr, &ledger, &at("2026-10-06T15:30[Europe/Paris]")), "");
        // Nothing of what the notifications said is kept: names of apps and conversations only.
        let text = toml::to_string(&ledger).unwrap();
        for said in ["chaussures", "Sortie", "Nouvelle version", "répondu", "+33", "Mme Durand", "ferris", "120363"] {
            assert!(!text.contains(said), "{said} kept: {text}");
        }
        assert!(text.contains("École Jules Ferry") && text.contains("Rust FR #général"), "{text}");
        // Kept a day, then forgotten; a conversation unseen for a week forgotten unless chosen for.
        let mut chosen = Choices::default();
        let school = talk_key("com.whatsapp", "120363000000000001@g.us");
        chosen.conversation.insert(school.clone(), TalkChoice { through: Through::Always, ..TalkChoice::default() });
        ledger.forget_old(stamp + 8 * 86_400, &chosen);
        assert!(ledger.back.is_empty());
        assert_eq!(ledger.conversations.keys().collect::<Vec<_>>(), [&school]);
    }

    #[test]
    fn held_ones_worked_out_again() {
        let choices = Choices::default();
        let paused = TestClock { overrides: Overrides { paused_since: Some(at("2026-10-06T08:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() } };
        let back = TestClock { overrides: Overrides { paused_since: Some(at("2026-10-06T08:00[Europe/Paris]").timestamp().as_second()), paused_ended: Some(at("2026-10-06T10:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() } };
        let held = Held { app: "com.shop.example".into(), label: "Shop".into(), kind: "gathered".into(), since: 0, until: at("2026-10-06T11:30[Europe/Paris]").timestamp().as_second(), ..Held::default() };
        let now = at("2026-10-06T10:00:30[Europe/Paris]");
        // Still paused: after the pause, asked again.
        assert_eq!(again(&held, &choices, &ask(&now, &paused, None, None)).why, Why::Gathered);
        // The pause over at 10:00: the next gathering, 13:00.
        assert_eq!(until_of(again(&held, &choices, &ask(&now, &back, None, None))), "2026-10-06T13:00:00");
        // A message from a stranger held in the evening; its conversation now let through always: now.
        let talk = Held { kind: "people".into(), via: "messages".into(), who: "stranger".into(), conversation: "c1".into(), ..held.clone() };
        let mut chosen = Choices::default();
        chosen.conversation.insert("c1".into(), TalkChoice { through: Through::Always, ..TalkChoice::default() });
        let evening = at("2026-10-06T20:30[Europe/Paris]");
        assert_eq!(again(&talk, &chosen, &ask(&evening, &TestClock::new(), None, None)), Decision::through(Why::Always));
        assert_eq!(until_of(again(&talk, &choices, &ask(&evening, &TestClock::new(), None, None))), "2026-10-07T09:00:00");
        // A group held in the evening, a safe person writing in it: still a group's row on review.
        let clock = TestClock::new();
        let mut ledger = Ledger::default();
        let p = posted(WHATSAPP_GROUP);
        let kind = classify(&p, &choices);
        let decision = decide(&kind, Some(Who::Safe), true, &choices, &ask(&evening, &clock, None, None));
        ledger.note(&p, &kind, Some(Who::Safe), true, None, &decision, evening.timestamp().as_second());
        let group = ledger.held.values().next().unwrap().clone();
        assert_eq!((group.who.as_str(), group.admitted, group.group), ("safe", false, true));
        assert_eq!(until_of(again(&group, &choices, &ask(&evening, &clock, None, None))), "2026-10-07T09:00:00");
        // Its conversation set to gathered, then to always: back now.
        let mut gathered_then_always = Choices::default();
        gathered_then_always.conversation.insert(group.conversation.clone(), TalkChoice { through: Through::Always, ..TalkChoice::default() });
        let gathered_held = Held { kind: "gathered".into(), ..group.clone() };
        assert_eq!(again(&gathered_held, &gathered_then_always, &ask(&evening, &clock, None, None)), Decision::through(Why::Always));
        // Holding switched off: everything back now.
        let off = Choices { hold: false, ..Choices::default() };
        assert_eq!(again(&held, &off, &ask(&now, &paused, None, None)), Decision::through(Why::Off));
    }

    #[test]
    fn choices_and_ledger_read_and_written() {
        let dir = std::env::temp_dir().join(format!("sioul-appnotes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut choices = Choices::default();
        assert!(choices.hold, "holding is on until switched off");
        choices.app.insert("com.discord".into(), AppChoice { kind: AppKind::Automaton, area: Some(Area::LEISURE), label: "Discord".into() });
        choices.conversation.insert("c1".into(), TalkChoice { through: Through::Always, title: "École".into(), app: "WhatsApp".into() });
        let path = dir.join("app-notes.toml");
        choices.save(&path).unwrap();
        assert_eq!(Choices::load(&path), choices);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("kind = \"automaton\"") && text.contains("area = \"leisure\"") && text.contains("through = \"always\""), "{text}");
        assert_eq!(Choices::load(&dir.join("none.toml")), Choices::default());
        std::fs::write(&path, "hold = false\n").unwrap();
        assert!(!Choices::load(&path).hold);
        std::fs::write(&path, "hold = [broken").unwrap();
        assert_eq!(Choices::load(&path), Choices::default(), "a file that does not read is no choice at all");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(AppKind::read("at-once"), Some(AppKind::AtOnce));
        assert_eq!(Through::read("never"), Some(Through::Never));
    }

    #[test]
    fn words_in_both_languages() {
        let now = at("2026-10-06T10:00[Europe/Paris]");
        let names = |n: usize| (1..=n).map(|i| format!("App{i}")).collect::<Vec<_>>();
        for (language, n, said) in [
            ("en", 1, "App1"),
            ("en", 2, "App1 and App2"),
            ("en", 3, "App1, App2 and App3"),
            ("en", 4, "App1, App2, App3 and one other app"),
            ("en", 6, "App1, App2, App3 and three other apps"),
            ("en", 15, "App1, App2, App3 and twelve other apps"),
            ("en", 16, "App1, App2, App3 and many other apps"),
            ("fr", 2, "App1 et App2"),
            ("fr", 4, "App1, App2, App3 et une autre application"),
            ("fr", 5, "App1, App2, App3 et deux autres applications"),
            ("fr", 20, "App1, App2, App3 et beaucoup d’autres applications"),
        ] {
            assert_eq!(apps_in_words(&Translator::new(language), &names(n)), said, "{language} {n}");
        }
        let tr = Translator::new("en");
        let at13 = at("2026-10-06T13:00[Europe/Paris]").timestamp().as_second();
        assert_eq!(why_text(&tr, "gathered", at13, &now), "From an automaton: held until 13:00, a gathered time.");
        assert_eq!(why_text(&tr, "allowed", 0, &now), "From someone who may reach you now: let through.");
        let fr = Translator::new("fr");
        assert_eq!(why_text(&fr, "gathered", at13, &now), "D’un automate\u{202f}: retenue jusqu’à 13:00, une heure de regroupement.");
        for why in ["untouched", "code", "off", "at-once", "allowed", "always", "waiting", "gathered", "never", "again"] {
            for tr in [&tr, &fr] {
                let said = why_text(tr, why, 0, &now);
                assert!(!said.starts_with("appnotes-"), "{why}: {said}");
                let held = why_text(tr, why, at13, &now);
                assert!(!held.starts_with("appnotes-"), "{why}: {held}");
            }
        }
    }
}

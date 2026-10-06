// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! New mail as a notification, at the times it may come (docs/porch.md,
//! "Notifications"). The Porch keeps mail until you look; what your lists let
//! through now is also told, once per batch, never once per message: the
//! count in words and the first senders with their subjects. Batched
//! notifications helped attention and mood where none at all made people more
//! anxious (Fitz et al. 2019): what helps is predictability, not silence.
//!
//! Mail that comes while it may not (outside its list's times, during sleep
//! or a pause, Free time included) waits; when a time begins in which some of
//! it may come, one notification says the Porch opens.
//!
//! Never told: what is set aside (forged, spam, a borrowed name, blocked),
//! hostile mail, codes (they have their own notification), what you send
//! yourself, the less important accounts, newsletters unless asked, and mail
//! read elsewhere before it came here. Each message once, by its account and
//! Message-ID, told or waiting (`Ledger`), on this device.

use crate::i18n::Translator;
use crate::porch::{Lane, Reason, Triaged};
use crate::quiet::Mode;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// How many letters a notification names: the count says the rest.
pub const NAMED: usize = 3;

/// A message's mark is kept this long: the Porch shows its last two weeks.
const KEPT: i64 = 30 * 86_400;

/// A letter as a notification names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letter {
    /// Its account and Message-ID, hashed (`key`).
    pub key: String,
    pub sender: String,
    pub subject: String,
}

impl Letter {
    pub fn of(t: &Triaged) -> Letter {
        Letter { key: key(t), sender: one_line(t.card.sender()), subject: one_line(&t.card.subject) }
    }
}

/// A sender's name or a subject on one line: a mail's words never break the notification's.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A message's key: its account and Message-ID, else its file's unique name
/// (which a change of flags keeps), hashed: the same message fetched again
/// keeps it, the same one in another account is another; and the ledger
/// holds no address and no Message-ID.
pub fn key(t: &Triaged) -> String {
    let account = t.card.account.as_deref().unwrap_or("");
    let id = t
        .card
        .message_id
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .or_else(|| t.card.path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).map(|n| format!("file:{}", crate::maildir::unique_part(n))))
        .unwrap_or_default();
    format!("{:016x}", fnv(&format!("{account}\u{1f}{id}")))
}

/// Read before it came here (the server said \Seen: read on a phone, in a
/// webmail): nothing to tell.
fn read_already(t: &Triaged) -> bool {
    t.card.path.as_deref().is_some_and(|p| crate::maildir::flags_of(p).contains('S'))
}

/// Whether a message is never told, whatever the time: set aside (forged,
/// spam, a borrowed name, blocked), hostile, a code (it has its own
/// notification), from yourself, in the less important accounts' lane, a
/// newsletter in the Filed lane unless `newsletters` (an automatic sender
/// there, a bill from no-reply, is told), or read already.
pub fn never(t: &Triaged, newsletters: bool) -> bool {
    match &t.lane {
        Lane::SetAside | Lane::Hostile | Lane::RightNow | Lane::Low => return true,
        Lane::Filed if !newsletters && t.reasons.contains(&Reason::Newsletter) => return true,
        _ => {}
    }
    t.reasons.iter().any(|r| matches!(r, Reason::FromYourself | Reason::Blocked)) || read_already(t)
}

/// Whether new mail may be told now: not while you sleep, nor in a pause,
/// Free time included (docs/pauses.md): it waits, and comes when the time
/// allows (`waited`).
pub fn may_tell(mode: &Mode) -> bool {
    !(mode.sleeps() || mode.paused() || mode.free())
}

/// The moment as new mail sees it: what now is for, why, whether mail may
/// be told, and whether a do-not-disturb you switched on lets only some
/// people through (`gated`). When it changes, a time began: what waited and
/// may come now is told once (`waited`).
pub fn moment(mode: &Mode, may: bool, gated: bool) -> String {
    format!("{}:{:?}:{may}:{gated}", mode.time.id(), mode.reason)
}

/// What was done with each new message, by key: told (or never to tell), or
/// waiting for a time it may come in; with when. On this device only, in
/// `$XDG_STATE_HOME/sioul/mail-notified.toml`, forgotten after 30 days.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub told: BTreeMap<String, i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub waiting: BTreeMap<String, i64>,
}

impl Ledger {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("mail-notified.toml")
    }

    /// As kept; empty when missing or unreadable (nothing then is told twice
    /// but what was waiting, which waits no more).
    pub fn load(path: &Path) -> Ledger {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written whole under another name, then renamed: never half a ledger.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        let temporary = path.with_extension(format!("toml.{}", std::process::id()));
        std::fs::write(&temporary, text).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
    }

    /// Whether a message was dealt with already: told, or waiting.
    pub fn known(&self, key: &str) -> bool {
        self.told.contains_key(key) || self.waiting.contains_key(key)
    }

    /// Told (or never to tell): never again.
    pub fn tell(&mut self, key: &str, at: i64) {
        self.waiting.remove(key);
        self.told.insert(key.to_string(), at);
    }

    /// Waiting for a time it may come in.
    pub fn wait(&mut self, key: &str, at: i64) {
        if !self.told.contains_key(key) {
            self.waiting.entry(key.to_string()).or_insert(at);
        }
    }

    /// Marks older than 30 days forgotten: their mail left the Porch's view long ago.
    pub fn forget_old(&mut self, now: i64) {
        self.told.retain(|_, at| now - *at < KEPT);
        self.waiting.retain(|_, at| now - *at < KEPT);
    }
}

/// A fetch's arrivals, sorted.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Sorted {
    /// To tell now, in the order they came.
    pub now: Vec<Letter>,
    /// Waiting for a time they may come in: their keys.
    pub later: Vec<String>,
    /// Never to tell: their keys.
    pub never: Vec<String>,
}

/// Sorts the inbox's arrivals of a fetch, as the Porch judged them: those
/// told now (`now`: a notification may come now, the matrix lets the
/// sender's list through now as the Porch shows it, and a do-not-disturb
/// you switched on lets the sender through), those that wait, those never
/// told. A message the ledger knows is left out: told once.
pub fn sort(arrivals: &[Triaged], ledger: &Ledger, newsletters: bool, now: impl Fn(&Triaged) -> bool) -> Sorted {
    let mut out = Sorted::default();
    let mut seen = BTreeSet::new();
    for t in arrivals {
        let key = key(t);
        if ledger.known(&key) || !seen.insert(key.clone()) {
            continue;
        }
        if never(t, newsletters) {
            out.never.push(key);
        } else if now(t) {
            out.now.push(Letter::of(t));
        } else {
            out.later.push(key);
        }
    }
    out
}

/// The mail that waited and may come now (`now`, as for `sort`): among the
/// Porch's messages (`items`), those waiting in the ledger, still unread and
/// not set aside since (a sender blocked meanwhile).
pub fn waited(items: &[Triaged], ledger: &Ledger, newsletters: bool, now: impl Fn(&Triaged) -> bool) -> Vec<Letter> {
    let mut seen = BTreeSet::new();
    items.iter().filter(|t| ledger.waiting.contains_key(&key(t)) && !never(t, newsletters) && now(t)).map(Letter::of).filter(|l| seen.insert(l.key.clone())).collect()
}

/// Among the Porch's messages, those waiting that will never be told now:
/// read meanwhile, blocked, set aside. Their marks become told.
pub fn gone_by(items: &[Triaged], ledger: &Ledger, newsletters: bool) -> Vec<String> {
    items.iter().filter(|t| never(t, newsletters)).map(key).filter(|k| ledger.waiting.contains_key(k)).collect()
}

/// The notification of new mail: "Two letters", and the first senders with
/// their subjects, "Murena, Your invoice · Alice, Dinner on Friday".
pub fn batch(tr: &Translator, letters: &[Letter]) -> (String, String) {
    (tr.text("mail-note-title", Some(&tr.counted(letters.len()))), named(tr, letters))
}

/// When a time begins in which mail that waited may come: "The Porch opens:
/// three letters wait for you.", the first senders below.
pub fn opens(tr: &Translator, letters: &[Letter]) -> (String, String) {
    (tr.text("mail-note-opens", Some(&tr.counted(letters.len()))), named(tr, letters))
}

/// The first letters, "Sender, Subject", joined by " · ".
fn named(tr: &Translator, letters: &[Letter]) -> String {
    letters
        .iter()
        .take(NAMED)
        .map(|l| {
            let mut args = crate::i18n::args();
            args.set("sender", l.sender.clone());
            args.set("subject", if l.subject.is_empty() { tr.text("mail-no-subject", None) } else { l.subject.clone() });
            tr.text("mail-note-letter", Some(&args))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{Area, Time, Week};
    use crate::porch::{SenderList, Senders};
    use crate::quiet::{Reach, Reason as Why};

    /// A message as the Porch judges it, fetched into `account`; `headers` before the subject.
    fn message(account: &str, from: &str, headers: &str, subject: &str, id: &str, senders: &Senders, priority: crate::config::Priority) -> Triaged {
        let raw = format!("From: {from}\r\nMessage-ID: <{id}>\r\n{headers}Subject: {subject}\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\n{subject}.\r\n");
        let known = SenderList::parse("alice@example.org");
        let trusted = ["mx.example.net".to_string()];
        let own = ["me@example.net".to_string()];
        let ctx = crate::porch::Context { cases: None, known: &known, senders, trusted_ids: &trusted, now: None, priority, own_domains: &[], shielded: false, assessments: None, filed_words: &[], own_addresses: &own };
        let mut card = crate::card::Card::from_bytes(raw.as_bytes()).unwrap();
        card.account = Some(account.to_string());
        card.path = Some(PathBuf::from(format!("/mail/{account}/new/1759400000.U1-{}.sioul", fnv(id) % 10_000)));
        crate::porch::triage(card, &ctx)
    }

    fn mode(time: Time, reason: Why) -> Mode {
        Mode { quiet: false, time, week: Week { work_hours: true, admin_hours: false, meals: true, sleep: true }, reason, until: None, back: None, label: String::new() }
    }

    #[test]
    fn a_batch_once_at_its_time() {
        let senders = Senders { safe: SenderList::parse("alice@example.org"), blocked: SenderList::parse("spam@example.com"), ..Senders::default() };
        let usual = crate::config::Priority::default();
        let alice = message("home", "Alice <alice@example.org>", "", "Dinner on Friday", "1@example.org", &senders, usual);
        let bill = message("home", "Murena <no-reply@murena.example>", "", "Your invoice", "2@murena.example", &senders, usual);
        let bob = message("home", "Bob <bob@example.org>", "", "Photos", "3@example.org", &senders, usual);
        let news = message("home", "Shop <news@shop.example>", "List-Id: <news.shop.example>\r\nList-Unsubscribe: <mailto:u@shop.example>\r\n", "Sale", "4@shop.example", &senders, usual);
        let code = message("home", "Bank <codes@bank.example>", "", "Your verification code: 482913", "5@bank.example", &senders, usual);
        let forged = message("home", "Alice <alice@example.org>", "Authentication-Results: mx.example.net; dmarc=fail (p=reject) header.from=example.org\r\n", "Urgent", "6@example.org", &senders, usual);
        let own = message("home", "Me <me@example.net>", "Authentication-Results: mx.example.net; dmarc=pass header.from=example.net\r\n", "A file", "7@example.net", &senders, usual);
        let low = message("social", "Network <hello@social.example>", "", "Someone liked", "8@social.example", &senders, crate::config::Priority::Below);
        let mut read = message("home", "Carol <carol@example.org>", "", "Read on the phone", "9@example.org", &senders, usual);
        read.card.path = Some(PathBuf::from("/mail/home/cur/1759400000.U1-9.sioul:2,S"));
        assert_eq!(bill.lane, Lane::Filed, "an automatic sender is filed");
        assert_eq!((news.lane.clone(), code.lane.clone(), forged.lane.clone(), low.lane.clone()), (Lane::Filed, Lane::RightNow, Lane::SetAside, Lane::Low));
        let arrivals = vec![bill.clone(), alice.clone(), bob.clone(), news.clone(), code, forged, own, low, read];

        // In view: the safe sender always; the others as the matrix says (here: the bill and Bob too).
        let ledger = Ledger::default();
        let sorted = sort(&arrivals, &ledger, false, |t| t.card.from_address.as_deref() != Some("bob@example.org"));
        assert_eq!(sorted.now.iter().map(|l| l.sender.as_str()).collect::<Vec<_>>(), ["Murena", "Alice"], "{sorted:?}");
        assert_eq!(sorted.later, [key(&bob)], "Bob waits for his list's times");
        assert_eq!(sorted.never.len(), 6, "the newsletter, the code, the forged one, your own, the less important account, the one read already");
        // Newsletters when asked.
        assert!(sort(&[news.clone()], &ledger, true, |_| true).now.len() == 1);

        // The words: the count in words, the first senders with their subjects.
        let en = Translator::new("en");
        assert_eq!(batch(&en, &sorted.now), ("Two letters".to_string(), "Murena, Your invoice · Alice, Dinner on Friday".to_string()));
        let fr = Translator::new("fr");
        assert_eq!(batch(&fr, &sorted.now), ("Deux lettres".to_string(), "Murena, Your invoice · Alice, Dinner on Friday".to_string()));
        assert_eq!(batch(&fr, &sorted.now[..1]).0, "Une lettre");
        assert_eq!(batch(&en, &sorted.now[..1]).0, "One letter");
        // Past twelve, digits; three named at most; a message without a subject says so.
        let many: Vec<Letter> = (0..13).map(|i| Letter { key: i.to_string(), sender: format!("S{i}"), subject: if i == 0 { String::new() } else { format!("T{i}") } }).collect();
        assert_eq!(batch(&en, &many), ("13 letters".to_string(), "S0, (no subject) · S1, T1 · S2, T2".to_string()));

        // Told once, by account and Message-ID: fetched again, nothing; the same message in another account is another.
        let mut ledger = Ledger::default();
        for letter in &sorted.now {
            ledger.tell(&letter.key, 100);
        }
        for k in &sorted.never {
            ledger.tell(k, 100);
        }
        for k in &sorted.later {
            ledger.wait(k, 100);
        }
        assert!(sort(&arrivals, &ledger, false, |_| true).now.is_empty(), "never twice");
        let elsewhere = message("work", "Alice <alice@example.org>", "", "Dinner on Friday", "1@example.org", &senders, usual);
        assert_eq!(sort(&[elsewhere], &ledger, false, |_| true).now.len(), 1);
        // A message without a Message-ID: its file's unique name, whatever its flags.
        let mut nameless = alice.clone();
        nameless.card.message_id = None;
        let flagged = Triaged { card: crate::card::Card { path: Some(PathBuf::from("/mail/home/cur/1759400000.U1-42.sioul:2,F")), ..nameless.card.clone() }, ..nameless.clone() };
        nameless.card.path = Some(PathBuf::from("/mail/home/new/1759400000.U1-42.sioul"));
        assert_eq!(key(&nameless), key(&flagged));

        // Asleep (or in a pause): nothing now, everything in view waits.
        let asleep = sort(&[bob.clone()], &Ledger::default(), false, |_| false);
        assert!(asleep.now.is_empty() && asleep.later.len() == 1);
        // A do-not-disturb you switched on: the people it lets through are told, the others wait.
        let gate = |t: &Triaged| t.card.from_address.as_deref() == Some("alice@example.org");
        let held = sort(&[alice.clone(), bob.clone()], &Ledger::default(), false, gate);
        assert_eq!((held.now.len(), held.now[0].sender.as_str(), held.later.len()), (1, "Alice", 1));

        // A time begins: what waited and may come now, told once.
        let waiting = waited(&arrivals, &ledger, false, |_| true);
        assert_eq!(waiting.iter().map(|l| l.sender.as_str()).collect::<Vec<_>>(), ["Bob"]);
        assert_eq!(opens(&en, &waiting).0, "The Porch opens: one letter waits for you.");
        assert_eq!(opens(&fr, &[waiting[0].clone(), waiting[0].clone(), waiting[0].clone()]).0, "Le Porche ouvre\u{202f}: trois lettres vous attendent.");
        assert!(waited(&arrivals, &ledger, false, |_| false).is_empty(), "not while its list may not come");
        for letter in &waiting {
            ledger.tell(&letter.key, 200);
        }
        assert!(waited(&arrivals, &ledger, false, |_| true).is_empty(), "once per window");
        // Read meanwhile: no longer waiting.
        let mut ledger = Ledger::default();
        ledger.wait(&key(&bob), 100);
        let mut seen = bob.clone();
        seen.card.path = Some(PathBuf::from("/mail/home/cur/1759400000.U1-3.sioul:2,S"));
        assert!(waited(&[seen.clone()], &ledger, false, |_| true).is_empty());
        assert_eq!(gone_by(&[seen], &ledger, false), [key(&bob)]);
    }

    #[test]
    fn the_matrix_and_the_moment() {
        let senders = Senders::default();
        let usual = crate::config::Priority::default();
        let stranger = message("work", "Someone <someone@elsewhere.example>", "", "A question", "10@elsewhere.example", &senders, usual);
        let week = Week { work_hours: true, admin_hours: false, meals: true, sleep: true };
        let senders = &senders;
        let in_view = |time: Time| move |t: &Triaged| crate::quiet::mail_in_view(t, senders, &Reach::default(), Area::WORK, time, week);
        // A neutral sender writing to a work address: told in work time, waiting in the evening.
        assert_eq!(sort(&[stranger.clone()], &Ledger::default(), false, in_view(Time::Work)).now.len(), 1);
        assert_eq!(sort(&[stranger.clone()], &Ledger::default(), false, in_view(Time::Leisure)).later.len(), 1);
        // Sleep, the pause and Free time hold mail; work, the evening and meals do not.
        assert!(!may_tell(&mode(Time::Sleep, Why::Sleep)) && !may_tell(&mode(Time::Sleep, Why::Paused)) && !may_tell(&mode(Time::Leisure, Why::FreeTime)));
        assert!(may_tell(&mode(Time::Work, Why::Working)) && may_tell(&mode(Time::Leisure, Why::Evening)) && may_tell(&mode(Time::Meals, Why::Meal)));
        // A time beginning is a change of moment; the same moment twice is none.
        let evening = moment(&mode(Time::Leisure, Why::Evening), true, false);
        assert_eq!(evening, moment(&mode(Time::Leisure, Why::Evening), true, false));
        assert_ne!(evening, moment(&mode(Time::Work, Why::Working), true, false));
        assert_ne!(moment(&mode(Time::Sleep, Why::Sleep), false, false), moment(&mode(Time::Leisure, Why::Evening), true, false), "waking");
        assert_ne!(moment(&mode(Time::Work, Why::Working), true, true), moment(&mode(Time::Work, Why::Working), true, false), "a do-not-disturb ends");
        // Kept and read back; old marks forgotten.
        let path = std::env::temp_dir().join(format!("sioul-mail-notified-{}.toml", std::process::id()));
        let mut ledger = Ledger::default();
        ledger.tell("a", 1_000);
        ledger.wait("b", 5_000_000);
        ledger.save(&path).unwrap();
        let mut back = Ledger::load(&path);
        assert_eq!(back, ledger);
        back.forget_old(1_000 + KEPT);
        assert!(!back.known("a") && back.known("b"));
        let _ = std::fs::remove_file(&path);
    }
}

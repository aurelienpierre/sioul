// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Settings ▸ What reaches you (« Ce qui vous joint », ReachesTab.qml), as
//! the window reads it (`view`; the redesign of 24 November 2026: the rule
//! once, then only the exceptions): the moment now in sentences and the
//! three states; the usual rule, a sentence per circle, from the start
//! chosen (`exceptions::rule`); your exceptions, a sentence each, read from
//! every store as differences (`exceptions::list`), with where they do
//! nothing on this device; the card that adds or changes one (`act`); What
//! comes when, worked out, read-only, each cell with the line that decides
//! it. And a person's sheet (PersonSheet.qml): their list and why, Always
//! through, and what each channel does with them at each time. The matrix
//! is `sioul_core::attention` (docs/attention.md); the exceptions
//! `sioul_core::exceptions`; this file says them in words, and writes what
//! is changed here.

use crate::backend::{config_path, load_config};
use serde_json::{Value, json};
use sioul_core::appnotes::Choices;
use sioul_core::attention::{self, Attention, Column, Kind, Level, Now, Person, Preset, Row};
use sioul_core::config::{Config, SettingValue};
use sioul_core::exceptions::{self, Device, Exception, Notice, Paths, Place, Stores, What, When, Whom};
use sioul_core::everywhere::People;
use sioul_core::i18n::Translator;
use sioul_core::reach::{Channel, Who};

// ---------------------------------------------------------------- what applies here

/// Which rows reach you through this device and your phones.
#[derive(Debug, Clone, Copy, Default)]
struct Reach {
    /// This device is a phone.
    phone: bool,
    /// A phone of yours screens calls: the calls' rows apply.
    calls: bool,
    /// A phone of yours holds other apps' notifications: the messages' rows apply.
    messages: bool,
    /// Your phones, as the sharing knows them (on a computer).
    phones: Phones,
}

/// Your phones in the sharing, as their entries say (`devices::Entry::notifications`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Phones {
    /// None known.
    #[default]
    None,
    /// Known, each saying it does not give Sioul notification access: none holds a message.
    WithoutAccess,
    /// One says it gives it, or says nothing (an older Sioul, which does not
    /// tell: counted as holding messages, as before phones said it).
    Holding,
}

impl Phones {
    /// What the devices' entries say: a phone that left the sharing counts no more.
    fn of(entries: &[sioul_sync::devices::Entry]) -> Phones {
        let phones: Vec<_> = entries.iter().filter(|d| d.kind == sioul_sync::devices::PHONE && !d.left).collect();
        if phones.iter().any(|d| d.notifications != Some(false)) {
            Phones::Holding
        } else if phones.is_empty() {
            Phones::None
        } else {
            Phones::WithoutAccess
        }
    }
}

fn reach() -> Reach {
    let phone = cfg!(target_os = "android");
    let screens = crate::calls::screens_here();
    let calls = screens || crate::calls::moment()["shown"] == true;
    let phones = if phone { Phones::None } else { phones_known() };
    Reach { phone, calls, messages: phone || phones == Phones::Holding, phones }
}

/// Your phones in the sharing: a phone's messages' rows apply where it holds them.
fn phones_known() -> Phones {
    let Some((_, Some((folder, key)))) = crate::share::vault() else { return Phones::None };
    Phones::of(&sioul_sync::devices::all(&folder, &key).0)
}

/// The channels whose rows apply, mail first.
fn channels(reach: Reach) -> Vec<Channel> {
    let mut out = vec![Channel::Mail];
    if reach.calls {
        out.push(Channel::Calls);
    }
    if reach.messages {
        out.push(Channel::Messages);
    }
    out
}

/// Sioul's own kinds that act on this device: sites on a computer, other
/// apps and the alarm at waking on a phone; the time running, always there, left unsaid.
fn kinds(phone: bool) -> Vec<Kind> {
    use Kind::*;
    let mut out = vec![Codes, Doses];
    if phone {
        out.push(Wake);
    }
    out.extend([Alarms, Before, Dates, Needs, Move, WorkOver]);
    if phone {
        out.extend([AppAutomatons, AppAtOnce]);
    } else {
        out.extend([Sites, SitesLive, SiteCalls]);
    }
    out
}

/// The rows the tab shows here, by id: every channel's (By person says when
/// one does not apply yet), but the call screening's (numbers nobody knows and
/// hidden numbers on calls: Settings ▸ Devices ▸ this phone's card), and the kinds that act on this device.
fn shown_rows(phone: bool) -> Vec<Row> {
    let mut rows: Vec<Row> = Channel::ALL.iter().flat_map(|c| attention::channel_rows(*c)).filter(|r| !attention::screened(*r)).collect();
    let mut own = kinds(phone);
    own.push(Kind::Time);
    rows.extend(Kind::ALL.iter().filter(|k| own.contains(k)).map(|k| Row::Own(*k)));
    rows
}

// ---------------------------------------------------------------- words

thread_local! {
    /// The language a test speaks; the window's otherwise.
    static SPOKEN: std::cell::Cell<Option<&'static Translator>> = const { std::cell::Cell::new(None) };
    /// The people and the address books a test stands in for Contacts ▸
    /// Your people and the lists (`porch::Senders`); the store's and the
    /// files' otherwise.
    static STORE: std::cell::RefCell<Option<(sioul_core::persons::Persons, sioul_core::porch::Senders)>> = const { std::cell::RefCell::new(None) };
}

/// One of your people as Who is in which circle lists them (reviews M1, M5,
/// L8): their id, name, and what holds them away from the row Your people,
/// each said with its door; nothing said: their cards here are judged on it.
struct Close {
    id: String,
    name: String,
    notes: Vec<Value>,
}

/// Whether Contacts ▸ Your people holds someone today besides you
/// (`persons::Persons::circle`): their line, rows and words show only then.
fn circle_holds_someone() -> bool {
    let today = sioul_core::zone::now().date();
    STORE.with(|kept| kept.borrow().as_ref().map(|(store, _)| !store.circle(today).is_empty())).unwrap_or_else(|| !sioul_core::persons::Persons::cached().circle(today).is_empty())
}

/// Your people today, in the household's order, each by the row they are
/// judged on (`close_of`); none: the page shows no line of theirs.
fn your_people() -> Vec<Close> {
    let today = sioul_core::zone::now().date();
    STORE.with(|kept| kept.borrow().as_ref().map(|(store, senders)| close_of(store, senders, today))).unwrap_or_else(|| close_of(&sioul_core::persons::Persons::cached(), &sioul_core::porch::Senders::load(&load_config()), today))
}

/// Each of your people, by the row their cards here are judged on, as the
/// lists judge every sender (`porch::Senders`): no card linked, or none on
/// this device (deleted, merged, in a book this device does not have),
/// said, with the door that links one (their page); a card of a book
/// someone else can write, said (L8); a card, an address or a number held
/// out of the circle by what restricts or blocks it, said with its reason
/// and the door to that card's sheet (M1). Absence proves nothing: said.
fn close_of(store: &sioul_core::persons::Persons, senders: &sioul_core::porch::Senders, today: jiff::civil::Date) -> Vec<Close> {
    let page = |said: String, id: &str| json!({ "text": said, "door": "page", "key": id, "label": text("reach-circles-tie") });
    let sheet = |said: String, key: &str| json!({ "text": said, "door": "sheet", "key": key, "label": text("person-card-sheet") });
    store
        .circle(today)
        .into_iter()
        .map(|person| {
            let name = person.name.clone();
            let mut notes: Vec<Value> = Vec::new();
            let linked: Vec<&str> = person.cards.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
            let found: Vec<usize> = linked.iter().filter_map(|uid| senders.contacts.find(uid)).collect();
            if linked.is_empty() {
                notes.push(page(say("reach-circles-untied", &[("name", name.clone())]), &person.id));
            } else if found.is_empty() {
                notes.push(page(say("reach-circles-missing", &[("name", name.clone())]), &person.id));
            } else if found.len() < linked.len() {
                let mut args = tr().counted(linked.len() - found.len());
                args.set("name", name.clone());
                notes.push(json!({ "text": tr().text("reach-circles-missing-some", Some(&args)), "door": "", "key": "", "label": "" }));
            }
            for index in found {
                let Some(card) = senders.contacts.card(index) else { continue };
                if card.held {
                    notes.push(json!({ "text": say("reach-circles-book", &[("name", name.clone())]), "door": "", "key": "", "label": "" }));
                    continue;
                }
                match senders.judge_card(&card.uid) {
                    Some(judged) if !judged.circle => {
                        notes.push(sheet(say("reach-circles-held", &[("name", name.clone()), ("why", why(&judged))]), &card.key));
                        continue;
                    }
                    _ => {}
                }
                // Each address and number of the card, judged as a sender is: one held out by its own entry.
                let entries = card.addresses.iter().map(|a| (a.clone(), senders.judge(a))).chain(senders.contacts.numbers_of_card(index).into_iter().map(|k| (k.clone(), senders.judge_number(&k))));
                for (entry, judged) in entries.filter(|(_, j)| !j.circle) {
                    notes.push(sheet(say("reach-circles-held-entry", &[("name", name.clone()), ("entry", entry), ("why", why(&judged))]), &card.key));
                }
            }
            Close { id: person.id.clone(), name, notes }
        })
        .collect()
}

/// Why a sender stands where the lists put them, as their sheet says it
/// (`senders::said`), in this file's language: "Restricted, as you chose for
/// Léa Martin.", "Blocked, as the group Bloqués says."; an own entry, "Restricted, by your own choice.".
fn why(judged: &sioul_core::porch::Judged) -> String {
    use sioul_core::porch::By;
    let one = text(&format!("sender-one-{}", judged.who.id()));
    match judged.by {
        By::Address | By::Number => say("reach-circles-own", &[("list", one)]),
        By::Card => say("sender-from-card", &[("list", one), ("name", judged.name.clone())]),
        By::Category => say("sender-from-category", &[("list", one), ("name", judged.name.clone())]),
        By::Domain | By::Prefix => say("sender-from-domain", &[("list", one), ("name", judged.name.trim_start_matches("*@").to_string())]),
        By::LetIn => say("sender-from-let-in", &[("list", one)]),
        By::Book => say("sender-from-book", &[("list", one)]),
        By::Default => say("sender-from-default", &[("list", one)]),
        By::Circle => say("sender-from-circle", &[("name", judged.name.clone())]),
    }
}

/// The notice that your people have a line of their own (review M2), when
/// it is due (`persons::Persons::circle_untold`): one informative notice,
/// opening What reaches you at their line.
fn circle_notice(store: &sioul_core::persons::Persons, today: jiff::civil::Date) -> Option<sioul_core::notice::Notice> {
    use sioul_core::notice::{Kind as NoticeKind, Notice, address};
    store.circle_untold(today).then(|| Notice::new(address::settings("attention.people"), NoticeKind::Minor, text("reach-people-told-title"), text("reach-people-told")))
}

/// The first time this version starts with someone in Contacts ▸ Your
/// people (review M2): one calm notice says that they have a line of their
/// own in What reaches you and that their calls ring at every time, with
/// the door to it. Said once on the devices that share the persons, never
/// at a start again: the mark (`Persons::circle_told`) is written in the
/// store, read from its file here, never from the window's state, as the
/// notice is posted (a computer's claim, made just before it is posted; on
/// a phone, once handed to Android). One that waits, held by a pause or a
/// do-not-disturb, comes when it ends; one never posted comes at the next
/// start. Never lost comes before never doubled: two devices starting
/// before the mark travels may each say it.
pub(crate) fn tell_circle_once() {
    let path = sioul_core::persons::Persons::default_path();
    let today = sioul_core::zone::now().date();
    let Some(notice) = sioul_core::persons::Persons::read(&path).ok().and_then(|store| circle_notice(&store, today)) else { return };
    let mark = move || {
        sioul_core::persons::change(&path, |store| store.circle_told = store.circle_told.max(sioul_core::persons::CIRCLE_TOLD))
            .map_err(|e| eprintln!("sioul: your people's notice: {e}"))
            .is_ok()
    };
    if cfg!(target_os = "android") {
        if let Some(json) = crate::eventalarms::notice_json(&notice.key(), &notice, false, None, jiff::Timestamp::now().as_second()) {
            crate::steps::java("notice", &json.to_string());
            mark();
        }
        return;
    }
    crate::tell::tell_with(notice, sioul_core::notice::told(None), None, Some(Box::new(mark)), true);
}

/// This file's words: the window's language, or a test's.
fn tr() -> &'static Translator {
    SPOKEN.with(|spoken| spoken.get()).unwrap_or_else(crate::backend::tr)
}

/// A message with its arguments, as `backend::say` has it, in this file's language.
fn say(id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = sioul_core::i18n::args();
    for (key, value) in pairs {
        args.set(key.to_string(), value.clone());
    }
    tr().text(id, Some(&args))
}

fn text(id: &str) -> String {
    tr().text(id, None)
}

/// A sentence about `what`, its verb agreeing: `plural` or not.
fn text_with(id: &str, what: &str, plural: bool, more: &[(&'static str, String)]) -> String {
    let mut args = sioul_core::i18n::args();
    args.set("what", what.to_string());
    args.set("n", if plural { 2 } else { 1 });
    for (key, value) in more {
        args.set(*key, value.clone());
    }
    capital(&tr().text(id, Some(&args)))
}

/// The first letter in capitals.
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The first letter in small letters: a time named inside a sentence.
fn small(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_lowercase().chain(chars).collect()).unwrap_or_default()
}

/// "a, b and c"; with `or`, "a, b or c".
fn listed(items: &[String], or: bool) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), text(if or { "attention-or" } else { "word-and" })),
    }
}

/// "09:00, 13:00 and 18:00": the gathered times.
fn gathered_times(config: &Config) -> String {
    let times: Vec<String> = sioul_core::appnotes::times_of(&config.reminders.gathered_times()).iter().map(|t| t.strftime("%H:%M").to_string()).collect();
    listed(&times, false)
}

/// A subject of a sentence: some channels of some people, or one of Sioul's own kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subject {
    People(Channel, Person),
    Own(Kind),
}

/// Who, in words: "your safe senders, your contacts and strangers"; everyone;
/// everyone else (`rest`: the others were said before in the same card).
fn who_words(persons: &[Person], everyone: bool, rest: bool) -> String {
    if everyone {
        return text(if rest { "attention-who-everyone-else" } else { "attention-who-everyone" });
    }
    // "Neutral" said as "your contacts" (decision 17), apart from the kinds of senders.
    let known: Vec<String> = persons.iter().filter(|p| matches!(p, Person::Safe | Person::Restricted)).map(|p| text(&format!("attention-who-kind-{}", p.id()))).collect();
    let mut items = Vec::new();
    if !known.is_empty() {
        let mut args = sioul_core::i18n::args();
        args.set("kinds", listed(&known, false));
        items.push(tr().text("attention-who-senders", Some(&args)));
    }
    for person in persons.iter().filter(|p| matches!(p, Person::People | Person::Neutral | Person::Stranger | Person::Hidden | Person::Groups | Person::Always)) {
        items.push(text(&format!("attention-who-{}", person.id())));
    }
    listed(&items, false)
}

/// Whether the mail channel's words take a plural verb: English "mail" is
/// one mass noun, French says « les courriels » (`attention-of-mail`'s `.plural`).
fn mail_plural() -> bool {
    tr().attribute("attention-of-mail", "plural").is_some_and(|p| p == "yes")
}

/// "mail, calls and messages", in the language's own words (French: "les courriels…").
fn channels_words(channels: &[Channel]) -> String {
    listed(&channels.iter().map(|c| text(&format!("attention-of-{}", c.id()))).collect::<Vec<_>>(), false)
}

/// Some channels from some people: "mail and messages from strangers".
fn from_words(channels: &[Channel], who: &str) -> String {
    let mut args = sioul_core::i18n::args();
    args.set("channels", channels_words(channels));
    args.set("who", who.to_string());
    tr().text("attention-from", Some(&args))
}

/// The people subjects of one sentence, said: per channel the people it
/// names, then channels naming the same people together. `said` keeps, per
/// channel, who was named already in the card, for "everyone else".
/// Returns the words and whether they are plural.
fn people_words(subjects: &[(Channel, Person)], said: &mut Vec<(Channel, Person)>) -> Option<(String, bool)> {
    let mut by_channel: Vec<(Channel, Vec<Person>)> = Vec::new();
    for (channel, person) in subjects {
        match by_channel.iter_mut().find(|(c, _)| c == channel) {
            Some((_, persons)) => persons.push(*person),
            None => by_channel.push((*channel, vec![*person])),
        }
    }
    if by_channel.is_empty() {
        return None;
    }
    // Each channel's people in words: everyone, everyone else, or by name.
    let mut worded: Vec<(Vec<Channel>, String)> = Vec::new();
    for (channel, persons) in &by_channel {
        let all: Vec<Person> = matrix_persons(*channel);
        let before: Vec<Person> = said.iter().filter(|(c, _)| c == channel).map(|(_, p)| *p).collect();
        let fresh = persons.iter().all(|p| !before.contains(p));
        let everyone = persons.iter().all(|p| *p != Person::Always) && all.iter().all(|p| persons.contains(p) || before.contains(p)) && fresh;
        let who = who_words(persons, everyone, everyone && !before.is_empty());
        match worded.iter_mut().find(|(_, w)| *w == who) {
            Some((channels, _)) => channels.push(*channel),
            None => worded.push((vec![*channel], who)),
        }
    }
    for (channel, persons) in &by_channel {
        said.extend(persons.iter().map(|p| (*channel, *p)));
    }
    let parts: Vec<String> = worded.iter().map(|(channels, who)| from_words(channels, who)).collect();
    let plural = parts.len() > 1 || worded.iter().any(|(channels, _)| channels.len() > 1 || channels[0] != Channel::Mail || mail_plural());
    Some((listed(&parts, false), plural))
}

/// The people of a channel the matrix decides for (the states), the call
/// screening's aside: numbers nobody knows and hidden numbers, on calls; Your
/// people only while the circle holds someone.
fn matrix_persons(channel: Channel) -> Vec<Person> {
    let people = circle_holds_someone();
    attention::persons(channel).iter().copied().filter(|p| p.state() && !attention::screened(Row::People(channel, *p)) && (people || *p != Person::People)).collect()
}

/// Sioul's own kinds in words, the three reminders together when all are
/// there: "your reminders, doses and the codes you ask for".
fn kinds_words(subjects: &[Kind]) -> Option<(String, bool)> {
    let reminders = [Kind::Alarms, Kind::Before, Kind::Dates];
    let together = reminders.iter().all(|k| subjects.contains(k));
    let mut items: Vec<(String, bool)> = Vec::new();
    for kind in subjects {
        if together && reminders.contains(kind) {
            if *kind == Kind::Alarms {
                items.push((text("attention-what-reminders"), true));
            }
            continue;
        }
        let singular = matches!(kind, Kind::Wake | Kind::Move | Kind::WorkOver);
        items.push((text(&format!("attention-what-{}", kind.id())), !singular));
    }
    if items.is_empty() {
        return None;
    }
    let plural = items.len() > 1 || items[0].1;
    Some((listed(&items.into_iter().map(|(w, _)| w).collect::<Vec<_>>(), false), plural))
}

/// A group of subjects in words: people first, then Sioul's own.
/// A group of subjects in words, a sentence's subject each: the people,
/// then Sioul's own, apart, so that each sentence stays short.
fn subjects_words(subjects: &[Subject], said: &mut Vec<(Channel, Person)>) -> Vec<(String, bool)> {
    let people: Vec<(Channel, Person)> = subjects.iter().filter_map(|s| if let Subject::People(c, p) = s { Some((*c, *p)) } else { None }).collect();
    let own: Vec<Kind> = subjects.iter().filter_map(|s| if let Subject::Own(k) = s { Some(*k) } else { None }).collect();
    [people_words(&people, said), kinds_words(&own)].into_iter().flatten().collect()
}

/// A kind's level as a computer tells it while a pause or a do-not-disturb
/// holds (`held`): what is to know comes, or would come, as shown without a
/// notification (docs/notifications.md); the rest as its cell says.
fn known_only(level: Level, kind: Kind, held: bool) -> Level {
    if held && kind.class() == Some(sioul_core::notice::Class::Informative) && matches!(level, Level::Now | Level::Event | Level::Gathered) { Level::Quiet } else { level }
}

/// Whether the system's do-not-disturb holds during a column (docs/attention.md, §3.3).
fn silenced(config: &Config, column: Column) -> bool {
    match column {
        Column::Pause | Column::Free => config.dnd.pauses,
        Column::Sleep => config.dnd.sleep,
        Column::Dnd => true,
        _ => false,
    }
}

/// The time now as a column: the pause, sleep, Free time, then the hours; work when both open.
fn column_now(now: &Now) -> Column {
    now.times.first().copied().unwrap_or(Column::Work)
}

// ---------------------------------------------------------------- the page

/// Your devices, for "Where" and the phones' names: every device of the
/// sharing that has not left, by the name you gave it, this one marked;
/// this one alone without the sharing.
fn devices(config: &Config) -> Vec<Device> {
    let Some((here, vault)) = crate::share::vault() else { return Vec::new() };
    let named = |id: &str, fallback: &str| sioul_core::device_said::name_of(config, id).map(str::to_string).unwrap_or_else(|| fallback.to_string());
    let mut out: Vec<Device> = vault
        .map(|(folder, key)| sioul_sync::devices::all(&folder, &key).0)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| !e.left)
        .map(|e| Device { name: named(&e.id, &e.name), phone: e.kind == sioul_sync::devices::PHONE, here: e.id == here, id: e.id })
        .collect();
    if !out.iter().any(|d| d.here) {
        out.push(Device { id: here.clone(), name: named(&here, &text("reach-this-device")), phone: cfg!(target_os = "android"), here: true });
    }
    out
}

/// A person's name by the id their own row writes: their card's, else the
/// number or the address as you would read it. Never stored.
fn person_name(cards: &[sioul_core::contacts::Contact], who: &str) -> String {
    let region = region();
    let named = |card: Option<&sioul_core::contacts::Contact>| card.map(|c| c.name.trim().to_string()).filter(|n| !n.is_empty());
    if let Some(uid) = who.strip_prefix(sioul_core::porch::CONTACT) {
        return named(cards.iter().find(|c| c.uid.trim() == uid)).unwrap_or_else(|| text("sender-card-elsewhere"));
    }
    if let Some(number) = who.strip_prefix(sioul_core::porch::TEL) {
        let wanted = sioul_core::phones::key(number, region);
        return named(cards.iter().find(|c| c.phones.iter().any(|p| sioul_core::phones::key(&p.value, region) == wanted))).unwrap_or_else(|| sioul_core::calls::shown_number(&wanted, region));
    }
    named(sioul_core::contacts::by_address(cards, who)).unwrap_or_else(|| who.to_string())
}

/// Where a rule does nothing on what this device knows, said there (the
/// redesign's point 4), by the channel it concerns: on a phone, messages
/// when Sioul holds none of other apps' (no access, holding off) or no
/// person's (people's messages not held), calls when Sioul does not screen
/// them; on a computer, as far as the sharing knows the phones.
fn idle(reach: Reach) -> Vec<(Channel, String)> {
    let mut out = Vec::new();
    if reach.phone {
        let choices = crate::appnotes::choices_here();
        let messages = match crate::appnotes::access_here() {
            Some(false) => Some("reach-idle-messages-access"),
            _ if !choices.hold => Some("reach-idle-messages-hold"),
            _ if !choices.holds_people() => Some("reach-idle-messages-people"),
            _ => None,
        };
        if let Some(id) = messages {
            out.push((Channel::Messages, text(id)));
        }
        if !crate::calls::screens_here() {
            out.push((Channel::Calls, text("reach-idle-calls-phone")));
        }
        return out;
    }
    match reach.phones {
        Phones::None => {
            for channel in [Channel::Calls, Channel::Messages] {
                out.push((channel, text("reach-idle-no-phone")));
            }
            return out;
        }
        Phones::WithoutAccess => out.push((Channel::Messages, text("reach-idle-messages-no-access"))),
        Phones::Holding => {}
    }
    if !reach.calls {
        out.push((Channel::Calls, text("reach-idle-calls")));
    }
    out
}

/// The channels an exception speaks of: a circle's or a person's, an app's
/// and a conversation's (messages); none for Sioul's own notices.
fn channels_of(e: &Exception) -> Vec<Channel> {
    match (&e.what, &e.whom) {
        (What::Channel { channel }, _) => vec![*channel],
        (What::App { .. } | What::Conversation { .. }, _) => vec![Channel::Messages],
        (What::Anything, Whom::Person { .. }) => vec![Channel::Calls, Channel::Messages],
        (What::Anything, Whom::Circle { person }) => Channel::ALL.into_iter().filter(|c| attention::persons(*c).contains(person) && !attention::screened(Row::People(*c, *person))).collect(),
        (What::Anything, _) => Channel::ALL.to_vec(),
        (What::Own { .. }, _) => Vec::new(),
    }
}

/// What an exception's line says beside it where it does nothing: the
/// sentence of each channel it speaks of whose rules do nothing, once each.
/// An exception for one device speaks of that device alone: nothing is said
/// of it elsewhere.
fn idle_of(e: &Exception, idle: &[(Channel, String)]) -> String {
    if !matches!(e.place, Place::Everywhere) && !matches!(&e.place, Place::Device { id, .. } if *id == crate::notices::here_id()) {
        return String::new();
    }
    let channels = channels_of(e);
    if channels.is_empty() {
        return String::new();
    }
    let said: Vec<&String> = idle.iter().filter(|(c, _)| channels.contains(c)).map(|(_, s)| s).collect();
    // Said only when none of its channels acts here.
    if channels.iter().all(|c| idle.iter().any(|(i, _)| i == c)) {
        let mut once: Vec<&String> = Vec::new();
        for line in said {
            if !once.contains(&line) {
                once.push(line);
            }
        }
        return once.into_iter().cloned().collect::<Vec<_>>().join(" ");
    }
    String::new()
}

/// How many a circle holds, in words: "2 groups, 5 people and 1 domain";
/// "" for everyone else, who are on no list.
fn count_of(circle: Person) -> String {
    let config = load_config();
    let counted = |id: &str, n: usize| (n > 0).then(|| tr().text(id, Some(&tr().counted(n))));
    let parts: Vec<String> = match circle {
        Person::Always => vec![counted("reach-count-people", People::load(&People::default_path()).people.len())],
        Person::People => vec![counted("reach-count-people", your_people().len())],
        Person::Neutral => vec![counted("reach-count-cards", sioul_core::contacts::all().len())],
        Person::Safe | Person::Restricted | Person::Blocked => {
            let path = match circle {
                Person::Safe => config.safe_senders_path(),
                Person::Restricted => config.restricted_senders_path(),
                _ => config.blocked_senders_path(),
            };
            let list = sioul_core::porch::SenderList::load(&path);
            let entries = list.entries();
            let domains = entries.iter().filter(|e| e.contains('*')).count();
            let people = list.cards().len() + list.numbers().len() + entries.len() - domains;
            vec![counted("reach-count-groups", list.categories().len()), counted("reach-count-people", people), counted("reach-count-domains", domains)]
        }
        _ => Vec::new(),
    }
    .into_iter()
    .flatten()
    .collect();
    listed(&parts, false)
}

/// A choice of the card's drop-downs: its value (what the core reads) and its words.
fn choice(value: Value, label: String) -> Value {
    json!({ "value": value, "label": label })
}

/// The card's choices that do not depend on one another (the redesign's
/// "Add an exception…"): What (anything, mail, calls, messages, the apps and
/// conversations known here, Sioul's own notices that act on this device),
/// From whom (anyone, the circles, your contacts' groups, a person…), Where
/// (every device, then each).
fn card_choices(reach: Reach, devices: &[Device], apps: Option<&Choices>, attention: &Attention) -> Value {
    let what = |w: What| serde_json::to_value(w).unwrap_or(Value::Null);
    let mut whats = vec![choice(what(What::Anything), text("reach-what-anything"))];
    for channel in Channel::ALL {
        whats.push(choice(what(What::Channel { channel }), text(&format!("except-{}", channel.id()))));
    }
    // The apps known here: their rows' names, and on a phone those it has seen chosen for.
    let mut known: Vec<(String, String)> = attention.sources().keys().filter_map(|id| id.strip_prefix(attention::APP_ROW).map(|p| (p.to_string(), attention.source_name(id).to_string()))).collect();
    if let Some(apps) = apps {
        for (package, choice) in &apps.app {
            if !known.iter().any(|(p, _)| p == package) {
                known.push((package.clone(), choice.label.clone()));
            }
        }
    }
    for (package, name) in known {
        let label = if name.trim().is_empty() { package.clone() } else { name.clone() };
        whats.push(choice(what(What::App { package, name }), label));
    }
    if let Some(apps) = apps {
        for (key, talk) in apps.conversation.iter().filter(|(_, t)| !t.title.trim().is_empty()) {
            let e = Exception { what: What::Conversation { key: key.clone(), title: talk.title.clone(), app: talk.app.clone() }, whom: Whom::Anyone, place: Place::Everywhere, when: When::pattern(exceptions::Pattern::Always), store: None };
            whats.push(choice(what(e.what.clone()), exceptions::subject_words(tr(), &e)));
        }
    }
    let mut own: Vec<Kind> = kinds(reach.phone);
    own.push(Kind::Time);
    for kind in Kind::ALL.into_iter().filter(|k| own.contains(k)) {
        whats.push(choice(what(What::Own { notice: Notice::Kind(kind) }), attention::row_label(tr(), Row::Own(kind))));
    }
    for notice in [Notice::NewMail, Notice::Newsletters] {
        whats.push(choice(what(What::Own { notice }), text(&format!("except-own-{}", notice.id()))));
    }
    let whom = |w: Whom| serde_json::to_value(w).unwrap_or(Value::Null);
    let mut whoms = vec![choice(whom(Whom::Anyone), capital(&text("except-whom-anyone")))];
    let people = circle_holds_someone();
    for person in exceptions::CIRCLES.into_iter().filter(|p| *p != Person::People || people) {
        whoms.push(choice(whom(Whom::Circle { person }), capital(&text(&format!("except-whom-{}", person.id())))));
    }
    let senders = sioul_core::porch::Senders::load(&load_config());
    for name in senders.contacts.names() {
        whoms.push(choice(whom(Whom::Group { name: name.clone() }), capital(&say("except-whom-group", &[("group", name)]))));
    }
    // A person: chosen by name in a field under the drop-down (`find`).
    whoms.push(json!({ "value": { "kind": "person", "id": "", "name": "" }, "label": text("reach-whom-person"), "search": true }));
    let place = |p: Place| serde_json::to_value(p).unwrap_or(Value::Null);
    let mut places = vec![choice(place(Place::Everywhere), text("reach-where-everywhere"))];
    for device in devices {
        let label = if device.here { say("reach-where-here", &[("device", device.name.clone())]) } else { device.name.clone() };
        places.push(choice(place(Place::Device { id: device.id.clone(), name: device.name.clone() }), label));
    }
    json!({ "what": whats, "whom": whoms, "where": places })
}

/// Settings ▸ What reaches you, as JSON (ReachesTab.qml; the redesign of 24
/// November 2026): {now: sentences, state, phone, start {id, choices [{id,
/// label}]}, rule [{circle, text, count}], idle \[sentences\], exceptions
/// [{text, note, value}], card {what, whom, where}, grid (the worked-out
/// grid, each cell with the line that decides it and the exception it
/// would start from), now_columns, circle (your people by name, for Who is
/// in which circle), apps (on a phone: this phone's switches for other apps)}.
pub(crate) fn view(realtime: bool) -> String {
    let _ = realtime;
    let config = load_config();
    let attention = crate::hours::attention();
    let reach = reach();
    let now = crate::hours::attention_now();
    let start = exceptions::start_of(&config, &attention);
    let devices = devices(&config);
    let apps = reach.phone.then(crate::appnotes::choices_here);
    let cards = sioul_core::contacts::all();
    let name = |who: &str| person_name(&cards, who);
    let list = exceptions::list(&Stores { attention: &attention, config: &config, devices: &devices, apps: apps.as_ref(), name: &name });
    let idle = idle(reach);
    let people = circle_holds_someone();
    let rule: Vec<Value> = exceptions::rule(tr(), start, people)
        .into_iter()
        .map(|line| {
            // Everyone else's calls are the call screening's: said where a phone of yours screens them.
            let text = if line.circle == Person::Stranger && reach.calls { format!("{} {}", line.text, screening_line(&config)) } else { line.text };
            json!({ "circle": line.circle.id(), "text": text, "count": count_of(line.circle) })
        })
        .collect();
    // Said once under the rule: each sentence of a channel whose rules do nothing here.
    let mut idle_said: Vec<String> = Vec::new();
    for (_, line) in &idle {
        if !idle_said.contains(line) {
            idle_said.push(line.clone());
        }
    }
    let lines: Vec<Value> = list.iter().map(|e| exception_line(e, &attention, people, &idle)).collect();
    let mut now_columns: Vec<&str> = now.times.iter().map(|c| c.id()).collect();
    now_columns.extend([(now.slot, Column::Slot), (now.dnd, Column::Dnd)].iter().filter(|(on, _)| *on).map(|(_, c)| c.id()));
    json!({
        "now": now_sentences(&attention, &now, reach),
        // As usual, Do not disturb or Reachable, as the status line's button says it.
        "state": crate::everywhere::moment()["state"].as_str().unwrap_or("usual"),
        "phone": reach.phone,
        "start": {
            "id": start.id(),
            "choices": Preset::ALL.iter().map(|p| json!({ "id": p.id(), "label": text(&format!("attention-preset-{}", p.id())) })).collect::<Vec<_>>(),
        },
        "rule": rule,
        "idle": idle_said,
        "exceptions": lines,
        "card": card_choices(reach, &devices, apps.as_ref(), &attention),
        "now_columns": now_columns,
        "grid": worked_out(&attention, &list, &table_rows(reach.phone), start, &config, reach.phone),
        "circle": {
            "title": text("reach-circle-people"),
            "help": text("reach-circles-people-help"),
            "people": your_people().iter().map(|p| json!({ "id": p.id, "name": p.name, "notes": p.notes })).collect::<Vec<_>>(),
        },
        // Android lets Sioul set exact alarms (Devices says when it does not).
        "exact": !reach.phone || crate::alarms::exact(),
    })
    .to_string()
}

/// An exception as the page lists it: its sentence (your people named where
/// they follow it, while the circle holds someone), where it does nothing
/// here, its value; a phone's table of calls with the question asked before
/// ✕, since taking it out takes the whole table, its groups' rows and its
/// switches with it.
fn exception_line(e: &Exception, attention: &Attention, people: bool, idle: &[(Channel, String)]) -> Value {
    let follow = if people { exceptions::followed_by_people(e, attention) } else { Vec::new() };
    let ask = if matches!(e.store, Some(exceptions::Store::Phone { .. })) { text("reach-remove-phone-ask") } else { String::new() };
    json!({ "text": exceptions::sentence_for(tr(), e, &follow), "note": idle_of(e, idle), "ask": ask, "value": e })
}

/// The rows of the worked-out grid, by id: mail's, calls' and messages',
/// the call screening's aside; Sioul's own that act on this device. Your
/// people's rows only while the circle holds someone.
fn table_rows(phone: bool) -> Vec<Row> {
    let people = circle_holds_someone();
    shown_rows(phone).into_iter().filter(|r| people || !matches!(r, Row::People(_, Person::People))).collect()
}

/// What comes when, worked out (read-only): the matrix's grid, a word per
/// cell (`attention::grid`), each cell with the line that decides it
/// (`exceptions::decide`, asked for this device: the usual rule for its
/// circle or kind, or an exception), marked `changed` where an exception
/// does, and the exception it would start from ("Add an exception for
/// this…": its row's what and who, every device, its cells read as a pattern
/// or as your own times). Your people's row that follows Safe's is decided
/// by Safe's lines. What the matrix alone does not say is said over it
/// (`decided_over`): new mail never told, Free time's "Nothing at all", a
/// site's news not gathered, and on a phone with a table of its own, that
/// table for the calls of the people it has rows for.
fn worked_out(attention: &Attention, list: &[Exception], rows: &[Row], start: Preset, config: &Config, phone: bool) -> Value {
    let grid = attention::grid(attention, tr(), rows, start);
    let mut value = serde_json::to_value(&grid).unwrap_or(Value::Null);
    let here = crate::notices::here_id();
    let table = if phone { crate::calls::line_here(config) } else { None };
    let Some(shown) = value["rows"].as_array_mut() else { return value };
    for line in shown.iter_mut() {
        let Some(row) = line["id"].as_str().and_then(Row::read) else { continue };
        let (what, whom) = match row {
            Row::People(channel, person) => (What::Channel { channel }, Whom::Circle { person }),
            Row::Own(kind) => (What::Own { notice: Notice::Kind(kind) }, Whom::Anyone),
        };
        // Your people's mail or messages not written: as Safe's stand, so Safe's lines decide them.
        let mut ask = exceptions::Ask::row(attention, row);
        ask.device = Some(here.clone());
        let cells = Column::ALL.map(|c| attention.cell(row, c));
        let usual = match row {
            Row::People(_, person) => say("reach-by-rule", &[("circle", text(&format!("except-circle-{}", person.id())))]),
            Row::Own(_) => text("reach-by-sioul"),
        };
        let from = Exception { what, whom, place: Place::Everywhere, when: exceptions::reading(row, &cells), store: None };
        let from = if exceptions::store_of(&from).is_ok() { serde_json::to_value(&from).unwrap_or(Value::Null) } else { Value::Null };
        // Each level's word on this row, as the grid words it.
        let words: Vec<(String, String)> = line["choices"].as_array().map(|marks| marks.iter().map(|m| (m["id"].as_str().unwrap_or_default().to_string(), m["word"].as_str().unwrap_or_default().to_string())).collect()).unwrap_or_default();
        let Some(cells) = line["cells"].as_array_mut() else { continue };
        for cell in cells.iter_mut() {
            let Some(column) = cell["column"].as_str().and_then(Column::read) else { continue };
            let level = cell["value"].as_str().and_then(Level::read).unwrap_or(Level::Now);
            // A phone's table speaks of its own moments alone: elsewhere every device's lines decide.
            let mut asked = ask.clone();
            if table_moment(row, column, config).is_none() {
                asked.device = None;
            }
            let by = exceptions::decide(list, &asked, column);
            let mut said = match by {
                Some(i) => say("reach-by-exception", &[("line", exceptions::sentence(tr(), &list[i]))]),
                None => usual.clone(),
            };
            let mut changed = by.is_some();
            if let Some((over, why)) = decided_over(row, column, level, list, config, table.as_ref()) {
                if over != level {
                    let word = words.iter().find(|(id, _)| id == over.id()).map(|(_, w)| w.clone()).unwrap_or_default();
                    cell["value"] = json!(over.id());
                    cell["word"] = json!(word);
                    cell["said"] = json!(cell_said(row, column, over));
                }
                if let Some(why) = why {
                    said = why;
                    changed = true;
                }
            }
            cell["by"] = json!(said);
            cell["changed"] = json!(changed);
            cell["except"] = from.clone();
        }
    }
    value
}

/// The moment of a phone's table of calls a column of the matrix reads as
/// (`calls::line_frames`): work, the rest of the day, the night; none where
/// the table says nothing (a pause, do-not-disturb, time for you, the night
/// or Free time when they turn do-not-disturb on: every phone's rule holds).
fn table_moment(row: Row, column: Column, config: &Config) -> Option<sioul_core::calls_line::When> {
    use sioul_core::calls_line::When;
    if !matches!(row, Row::People(Channel::Calls, Person::Safe | Person::Neutral | Person::Restricted)) {
        return None;
    }
    match column {
        Column::Work => Some(When::Work),
        Column::Admin | Column::Leisure | Column::Meals => Some(When::Day),
        Column::Free if !config.dnd.pauses && !config.free_time.nothing => Some(When::Day),
        Column::Sleep if !config.dnd.sleep => Some(When::Night),
        _ => None,
    }
}

/// What decides a cell over the matrix's own value, and the sentence of the
/// line that does (none: the line `decide` named stays): new mail never
/// told (mail that would come at once is shown without a word); Free time's
/// "Nothing at all" (only Always through comes); a site's news not gathered
/// (never told); this phone's own table of calls, at its moments.
fn decided_over(row: Row, column: Column, level: Level, list: &[Exception], config: &Config, table: Option<&sioul_core::calls_line::Grid>) -> Option<(Level, Option<String>)> {
    let line = |e: &Exception| Some(say("reach-by-exception", &[("line", exceptions::sentence(tr(), e))]));
    match row {
        // As the matrix reads it (`Attention::level`): every state's row held in Free time.
        Row::People(_, person) if config.free_time.nothing && column == Column::Free && person.state() => Some((Level::Later, Some(text("reach-by-free-nothing")))),
        Row::People(Channel::Mail, _) if !config.reminders.mail && level == Level::Now => {
            let e = list.iter().find(|e| matches!(e.what, What::Own { notice: Notice::NewMail }))?;
            Some((Level::Quiet, line(e)))
        }
        Row::Own(Kind::Sites) if !config.reminders.gather && level == Level::Gathered => {
            let e = list.iter().find(|e| matches!(e.store, Some(exceptions::Store::Switch { ref key }) if key == "reminders.gather"))?;
            Some((Level::Never, line(e)))
        }
        Row::People(Channel::Calls, person) => {
            use sioul_core::calls_line::{Cell, Row as Line};
            let moment = table_moment(row, column, config)?;
            let line_row = match person {
                Person::Safe => Line::Safe,
                Person::Neutral => Line::Contacts,
                _ => Line::Restricted,
            };
            let level = match table?.cell(&line_row, moment)? {
                Cell::Rings => Level::Now,
                Cell::Voicemail => Level::Later,
                Cell::Usual => return None,
            };
            // The line that names the table: `decide`, asked for this phone, already gives it.
            Some((level, None))
        }
        _ => None,
    }
}

/// A cell of the worked-out grid in a sentence, as `attention::grid` says it
/// ("Mail, Safe, at Work: At once."), for a value said over the matrix's.
fn cell_said(row: Row, column: Column, level: Level) -> String {
    let label = attention::row_label(tr(), row);
    let group = text(&format!("attention-group-{}", row.group()));
    let mut args = sioul_core::i18n::args();
    args.set("row", if matches!(row, Row::People(..)) { format!("{group}, {label}") } else { label });
    args.set("column", attention::column_label(tr(), column));
    args.set("value", attention::level_label(tr(), row, level));
    tr().text("attention-cell", Some(&args))
}

/// One of the card's values, as the core reads it; none when it does not read.
fn read<T: serde::de::DeserializeOwned>(asked: &Value, key: &str) -> Option<T> {
    serde_json::from_value(asked[key].clone()).ok()
}

/// The card's "When it reaches you" for what it names so far
/// (`exceptions::offered`): each choice in words, then whether your own
/// times may be set, and each moment's levels.
fn offered(asked: &Value) -> Value {
    let (Some(what), Some(whom), Some(place)) = (read::<What>(asked, "what"), read::<Whom>(asked, "whom"), read::<Place>(asked, "where")) else { return json!({ "when": [], "own": false, "moments": [], "why": text("reach-when-pick") }) };
    // The exception changed (Edit): its own When kept as a choice where the card cannot say it.
    let old: Option<Exception> = read(asked, "old");
    // A device that cannot keep it: said, nothing offered.
    let probe = Exception { what: what.clone(), whom: whom.clone(), place: place.clone(), when: When::pattern(exceptions::Pattern::Always), store: None };
    if let Some(why) = exceptions::place_refused(&probe, &devices(&load_config())) {
        return json!({ "when": [], "own": false, "moments": [], "why": text(why) });
    }
    // Each choice with the cells it writes on its first row, where it is one of times: "Your own
    // times…" starts from the pattern chosen before it.
    let cells_of = |w: &When| -> Value {
        let When::Pattern { pattern } = w else { return Value::Null };
        let probe = Exception { what: what.clone(), whom: whom.clone(), place: place.clone(), when: w.clone(), store: None };
        let cells = match exceptions::store_of(&probe) {
            Ok(exceptions::Store::Rows { rows }) => rows.first().and_then(|r| Row::read(r)).and_then(|r| exceptions::cells(r, *pattern)),
            Ok(exceptions::Store::Source { id }) => exceptions::source_cells(&id, *pattern),
            _ => None,
        };
        cells.map_or(Value::Null, |c| json!(c.map(|l| l.id())))
    };
    let when: Vec<Value> = exceptions::offered_for(&what, &whom, &place, old.as_ref())
        .into_iter()
        .map(|w| json!({ "value": serde_json::to_value(&w).unwrap_or(Value::Null), "label": capital(&exceptions::when_words(tr(), &w, &what)), "cells": cells_of(&w) }))
        .collect();
    let own = exceptions::own_times(&what, &whom, &place);
    // Nothing to choose for what is chosen: said why (Mail on one device, anyone on a channel…).
    let why = if when.is_empty() && !own { text("reach-when-pick") } else { String::new() };
    let probe = Exception { what: what.clone(), whom, place, when: When::Own { cells: [Level::Now; 9] }, store: None };
    let channel = match &what {
        What::Channel { channel } => Some(*channel),
        What::App { .. } | What::Conversation { .. } => Some(Channel::Messages),
        _ => None,
    };
    let moments: Vec<Value> = if own {
        exceptions::own_choices(&probe)
            .iter()
            .zip(Column::ALL)
            .map(|(levels, column)| json!({ "column": column.id(), "label": attention::column_label(tr(), column), "levels": levels.iter().map(|l| json!({ "value": l.id(), "label": capital(&exceptions::level_words(tr(), *l, channel)) })).collect::<Vec<_>>() }))
            .collect()
    } else {
        Vec::new()
    };
    json!({ "when": when, "own": own, "moments": moments, "why": why })
}

/// People of your address books whose names hold `text`, for "From whom: a
/// person…": their card's own row (`contact:<UID>`), at most twelve.
fn find(text: &str) -> Value {
    let wanted: String = sioul_core::text::fold(text.trim()).into_iter().collect::<String>().to_lowercase();
    if wanted.chars().count() < 2 {
        return json!([]);
    }
    let found: Vec<Value> = sioul_core::contacts::all()
        .into_iter()
        .filter(|c| !c.uid.trim().is_empty() && attention::is_person(&attention::person_row(&format!("contact:{}", c.uid.trim()))))
        .filter(|c| sioul_core::text::fold(&c.name).into_iter().collect::<String>().to_lowercase().contains(&wanted))
        .take(12)
        .map(|c| choice(serde_json::to_value(Whom::Person { id: format!("contact:{}", c.uid.trim()), name: c.name.trim().to_string() }).unwrap_or(Value::Null), c.name.trim().to_string()))
        .collect();
    json!(found)
}

/// Where the page's writes land: the configuration, and on a phone its own choices for other apps.
fn paths() -> Paths {
    Paths { config: config_path(), apps: cfg!(target_os = "android").then(sioul_core::appnotes::Choices::default_path) }
}

/// A change made on the page, and how to undo it.
pub(crate) struct Changed {
    /// The status line's words: "Taken out: Mail from your contacts…".
    pub(crate) line: String,
    /// What the stores held before, put back by Undo.
    pub(crate) kept: exceptions::Kept,
}

/// What the page asks (ReachesTab.qml's `act`): "offered" {what, whom,
/// where} the card's choices for When; "find" {text} people by name;
/// "save" {old?, new} an exception made or changed; "remove" {value} one
/// taken out; "start" {id, sure} a start chosen, asked first when it
/// replaces exceptions. Answers {problem, ask, ...} as JSON, and the change
/// made with what Undo puts back.
pub(crate) fn act(verb: &str, json: &str) -> (String, Option<Changed>) {
    let asked: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let answer = |problem: String| json!({ "problem": problem }).to_string();
    match verb {
        "offered" => (offered(&asked).to_string(), None),
        "find" => (find(asked["text"].as_str().unwrap_or_default()).to_string(), None),
        "save" | "remove" => {
            let new: Option<Exception> = serde_json::from_value(asked["new"].clone()).ok();
            let old: Option<Exception> = serde_json::from_value(if verb == "remove" { asked["value"].clone() } else { asked["old"].clone() }).ok();
            if old.is_none() && new.is_none() {
                return (answer(text("reach-not-saved")), None);
            }
            let kept = match exceptions::save(&paths(), old.as_ref(), new.as_ref(), &devices(&load_config())) {
                Ok(Some(kept)) => kept,
                // Saved as it was: nothing written, nothing to undo.
                Ok(None) => return (answer(String::new()), None),
                Err(exceptions::Refusal::Words(id)) => return (answer(text(id)), None),
                // The core's own words go to the log; the person reads a sentence of theirs.
                Err(exceptions::Refusal::Failed(e)) => {
                    eprintln!("What reaches you: {e}");
                    return (answer(text("reach-not-saved")), None);
                }
            };
            let line = match (&old, &new) {
                (Some(old), None) => say("reach-removed", &[("line", exceptions::sentence(tr(), old))]),
                (None, Some(new)) => say("reach-added", &[("line", exceptions::sentence(tr(), new))]),
                (_, Some(new)) => say("reach-changed", &[("line", exceptions::sentence(tr(), new))]),
                (None, None) => String::new(),
            };
            (answer(String::new()), Some(Changed { line, kept }))
        }
        "start" => {
            let Some(to) = asked["id"].as_str().and_then(Preset::read) else { return (answer(text("reach-not-saved")), None) };
            let attention = crate::hours::attention();
            let config = load_config();
            let from = exceptions::start_of(&config, &attention);
            // The start shown and kept: nothing to write. Shown but never kept (settings from before it
            // was kept read as As Sioul does): choosing it keeps it, every row as it was.
            if from == to && exceptions::stored_start(&config).is_some() {
                return (answer(String::new()), None);
            }
            let devices = devices(&config);
            let cards = sioul_core::contacts::all();
            let name = |who: &str| person_name(&cards, who);
            let apps = cfg!(target_os = "android").then(crate::appnotes::choices_here);
            let list = exceptions::list(&Stores { attention: &attention, config: &config, devices: &devices, apps: apps.as_ref(), name: &name });
            let gone: Vec<String> = exceptions::gives_way(&list, from, to).into_iter().map(|i| exceptions::sentence(tr(), &list[i])).collect();
            if !gone.is_empty() && asked["sure"] != true {
                let mut args = tr().counted(gone.len());
                args.set("start", text(&format!("attention-preset-{}", to.id())));
                args.set("lines", gone.join(" "));
                return (json!({ "problem": "", "ask": tr().text("reach-start-replaces", Some(&args)) }).to_string(), None);
            }
            let paths = paths();
            let rows = exceptions::start_rows(from, to);
            let kept = exceptions::keep(&paths, &[], &rows);
            if let Err(e) = exceptions::apply(&paths, &exceptions::choosing(from, to), to) {
                let _ = exceptions::put_back(&paths, &kept);
                eprintln!("What reaches you: {e}");
                return (answer(text("reach-not-saved")), None);
            }
            let line = say("reach-started", &[("start", text(&format!("attention-preset-{}", to.id())))]);
            (answer(String::new()), Some(Changed { line, kept }))
        }
        _ => (answer(text("reach-not-saved")), None),
    }
}

/// Undo of a change of the page: the stores put back as they were; what went wrong, else "".
pub(crate) fn undo(kept: &exceptions::Kept) -> String {
    match exceptions::put_back(&paths(), kept) {
        Ok(()) => String::new(),
        Err(e) => e,
    }
}

/// The moment now, in sentences: the time and until when, the layers on top
/// of it, then what comes at once, what is shown without a word, what waits
/// (`Attention::person` and `level`: what the matrix decides now).
/// The card "What reaches you now" (the status line's button, ReachCard.qml;
/// the proposal's 3.5 and 3.6): what now is and what comes at once, in two
/// sentences; on a phone, what it holds, by app, never by conversation; the
/// call screening in a sentence while a phone of yours screens calls, and
/// Expecting a call (`calls::moment`); Mail every minute (Real time).
pub(crate) fn now_card(realtime: bool) -> String {
    let attention = crate::hours::attention();
    let now = crate::hours::attention_now();
    let mut said = now_sentences(&attention, &now, reach());
    said.truncate(2);
    let held = if cfg!(target_os = "android") { crate::appnotes::porch_line() } else { String::new() };
    let calls = crate::calls::moment();
    let screening = if calls["shown"] == true { screening_line(&load_config()) } else { String::new() };
    // The conversations the phone holds, by their titles: on the phone alone, never sent elsewhere.
    let talks = crate::appnotes::held_talks();
    json!({ "now": said, "held": held, "talks": talks, "realtime": realtime, "screening": screening, "calls": calls }).to_string()
}

/// The call screening in a sentence, as `[screening]` says it (`Screening::of`).
fn screening_line(config: &Config) -> String {
    use sioul_core::screening::{Hours, Rule, Screening};
    let screening = Screening::of(config);
    if !screening.on {
        return text("reach-card-screening-off");
    }
    let when = text(match screening.hours {
        Hours::Always => "reach-card-screening-always",
        Hours::OutsideWork => "reach-card-screening-outside-work",
    });
    let id = match screening.unknown {
        Rule::Voicemail => "reach-card-screening-voicemail",
        Rule::Work => "reach-card-screening-work",
        Rule::Ring => "reach-card-screening-ring",
    };
    say(id, &[("when", when)])
}

fn now_sentences(attention: &Attention, now: &Now, reach: Reach) -> Vec<String> {
    let mode = crate::hours::mode_now();
    let column = column_now(now);
    let time = small(&attention::column_label(tr(), column));
    let at = sioul_core::zone::now();
    let mut out = vec![match mode.until.as_ref().filter(|_| !mode.paused()) {
        Some(until) => say("attention-now-time", &[("time", time), ("until", sioul_core::quiet::until_text(tr(), until, &at))]),
        None => say("attention-now-time-open", &[("time", time)]),
    }];
    for (on, layer) in [(now.slot, Column::Slot), (now.dnd, Column::Dnd)] {
        if on {
            out.push(say("attention-now-layer", &[("layer", attention::column_label(tr(), layer))]));
        }
    }
    if now.nothing {
        out.push(text("attention-now-nothing"));
    }
    let mut groups: Vec<(u8, Vec<Subject>)> = Vec::new();
    let mut add = |rank: u8, subject: Subject| match groups.iter_mut().find(|(r, _)| *r == rank) {
        Some((_, subjects)) => subjects.push(subject),
        None => groups.push((rank, vec![subject])),
    };
    for channel in channels(reach) {
        for person in matrix_persons(channel) {
            match attention.person(channel, person, false, now) {
                Level::Now => add(0, Subject::People(channel, person)),
                // A text in the shade without a sound (◑, decision 13).
                Level::Quiet if channel == Channel::Messages => add(3, Subject::People(channel, person)),
                Level::Quiet => add(2, Subject::People(channel, person)),
                // At the gathered times by day (◎, held apps phase 1, A3).
                Level::Gathered => add(1, Subject::People(channel, person)),
                Level::Never => {}
                _ => add(5, Subject::People(channel, person)),
            }
        }
    }
    // Of Sioul's own, what matters most: codes, doses, reminders; on a
    // computer, what is to know shown alone while a pause or a do-not-disturb holds.
    let held_here = !reach.phone && (mode.paused() || now.dnd || now.times.iter().any(|c| silenced(&load_config(), *c)));
    for kind in [Kind::Codes, Kind::Doses, Kind::Alarms, Kind::Before, Kind::Dates] {
        match known_only(attention.level(Row::Own(kind), now), kind, held_here) {
            Level::Quiet => add(2, Subject::Own(kind)),
            Level::Now | Level::Event => add(0, Subject::Own(kind)),
            Level::Gathered => add(1, Subject::Own(kind)),
            Level::Never => {}
            _ => add(5, Subject::Own(kind)),
        }
    }
    groups.sort_by_key(|(rank, _)| *rank);
    let mut said = Vec::new();
    let config = load_config();
    for (rank, subjects) in groups {
        for (what, plural) in subjects_words(&subjects, &mut said) {
            out.push(match rank {
                0 => text_with("attention-card-now", &what, plural, &[]),
                1 => text_with("attention-card-gathered", &what, plural, &[("times", gathered_times(&config))]),
                2 => text_with("attention-card-quiet", &what, plural, &[]),
                3 => text_with("attention-card-shade", &what, plural, &[]),
                _ => text_with("attention-now-wait", &what, plural, &[]),
            });
        }
    }
    out
}

// ---------------------------------------------------------------- a channel, at each time

/// What a channel does with someone at each time, in sentences: "Their mail
/// comes at once during work, admin, leisure and meals. It is shown without
/// a notification during sleep, a pause and free time." Then the layers,
/// when they hold more. `id`: the sentence's key, "attention-sheet-line"
/// for someone, "attention-always-line" for Always through.
fn channel_line(attention: &Attention, channel: Channel, person: Person, always: bool, id: &str) -> Option<String> {
    if person == Person::Blocked {
        return Some(text(&format!("{id}-blocked-{}", channel.id())));
    }
    let at = |column: Column| -> Level {
        // Always through's own row: "as their list" said as such, not as one person's.
        let level = if person == Person::Always { attention.cell(Row::People(channel, Person::Always), column) } else { attention.person(channel, person, always, &Now::time(column)) };
        // A phone cannot show what it holds: shown, not told, is held.
        if channel != Channel::Mail && level == Level::Quiet { Level::Later } else { level }
    };
    let mut parts: Vec<(Level, Vec<Column>)> = Vec::new();
    for column in Column::TIMES {
        let level = at(column);
        match parts.iter_mut().find(|(l, _)| *l == level) {
            Some((_, columns)) => columns.push(column),
            None => parts.push((level, vec![column])),
        }
    }
    let during = |columns: &[Column]| -> String {
        if columns.len() == Column::TIMES.len() {
            return text("attention-during-any");
        }
        let mut args = sioul_core::i18n::args();
        args.set("times", listed(&columns.iter().map(|c| text(&format!("attention-during-{}", c.id()))).collect::<Vec<_>>(), false));
        tr().text("attention-during", Some(&args))
    };
    let mut sentences = Vec::new();
    for (index, (level, columns)) in parts.iter().enumerate() {
        let verb = format!("{id}-{}-{}", channel.id(), level_word(*level));
        let mut args = sioul_core::i18n::args();
        args.set("during", during(columns));
        args.set("first", if index == 0 { "yes" } else { "no" });
        sentences.push(tr().text(&verb, Some(&args)));
    }
    // The layers, when they hold more than the time does; said once when both hold the same.
    let row = Row::People(channel, if always || person == Person::Always { Person::Always } else { person });
    let held = |layer: Column| match attention.cell(row, layer) {
        Level::Through | Level::Now | Level::As => None,
        Level::Quiet if channel == Channel::Calls => Some(Level::Later),
        other => Some(other),
    };
    let layers: Vec<(&str, Level)> = match (held(Column::Slot), held(Column::Dnd)) {
        (Some(slot), Some(dnd)) if slot == dnd => vec![("both", slot)],
        (slot, dnd) => [("slot", slot), ("dnd", dnd)].into_iter().filter_map(|(id, level)| level.map(|l| (id, l))).collect(),
    };
    for (layer, level) in layers {
        let mut args = sioul_core::i18n::args();
        args.set("layer", layer);
        sentences.push(tr().text(&format!("{id}-layer-{}-{}", channel.id(), level_word(level)), Some(&args)));
    }
    Some(sentences.join(" "))
}

/// A level's word in the keys of the sentences.
fn level_word(level: Level) -> &'static str {
    match level {
        Level::Now | Level::Through => "now",
        Level::Quiet => "quiet",
        Level::Never => "never",
        Level::As => "as",
        // A message's at the gathered times by day (held apps phase 1, A3).
        Level::Gathered => "gathered",
        _ => "later",
    }
}

// ---------------------------------------------------------------- a person's sheet

/// The card with this file or UID, from every address book.
fn card(key: &str) -> Option<sioul_core::contacts::Contact> {
    let key = key.trim();
    (!key.is_empty()).then(|| sioul_core::contacts::all().into_iter().find(|c| c.key == key || c.uid.trim() == key)).flatten()
}

/// A JSON array of addresses, or one address.
fn addresses_of(text: &str) -> Vec<String> {
    let text = text.trim();
    let list: Vec<String> = if text.starts_with('[') { serde_json::from_str(text).unwrap_or_default() } else { vec![text.to_string()] };
    list.into_iter().map(|a| a.trim().to_ascii_lowercase()).filter(|a| !a.is_empty()).collect()
}

/// Who the sheet is about: a card (from Contacts, or a sender's card), else
/// a sender's addresses alone, or a caller's number alone (`tel:…`, from a
/// call's line on the Porch).
struct Someone {
    card: Option<sioul_core::contacts::Contact>,
    addresses: Vec<String>,
}

/// How numbers are written here: the contacts' country, else the language's.
fn region() -> Option<&'static sioul_core::phones::Region> {
    sioul_core::phones::chosen(load_config().contacts.region.as_deref(), &tr().text("qt-locale", None))
}

impl Someone {
    fn of(key: &str, addresses: &str) -> Someone {
        let addresses = addresses_of(addresses);
        let card = card(key).or_else(|| {
            let all = sioul_core::contacts::all();
            addresses.iter().find_map(|a| match a.strip_prefix(sioul_core::porch::TEL) {
                // A caller's number: the card holding it, as the number's key.
                Some(number) => {
                    let region = region();
                    let wanted = sioul_core::phones::key(number, region);
                    sioul_core::phones::is_whole(&wanted).then(|| all.iter().find(|c| c.phones.iter().any(|p| sioul_core::phones::key(&p.value, region) == wanted)).cloned()).flatten()
                }
                None => sioul_core::contacts::by_address(&all, a).cloned(),
            })
        });
        Someone { card, addresses }
    }

    fn name(&self) -> String {
        let first = || {
            self.addresses.first().map(|a| match a.strip_prefix(sioul_core::porch::TEL) {
                Some(number) => {
                    let region = region();
                    sioul_core::calls::shown_number(&sioul_core::phones::key(number, region), region)
                }
                None => a.clone(),
            })
        };
        self.card.as_ref().map(|c| c.name.trim().to_string()).filter(|n| !n.is_empty()).or_else(first).unwrap_or_default()
    }

    fn emails(&self) -> Vec<String> {
        let mut out: Vec<String> = self.card.iter().flat_map(|c| c.emails.iter().map(|e| e.value.trim().trim_start_matches("mailto:").to_ascii_lowercase())).collect();
        for address in self.addresses.iter().filter(|a| !a.starts_with(sioul_core::porch::TEL)) {
            if !out.contains(address) {
                out.push(address.clone());
            }
        }
        out
    }

    fn phones(&self) -> Vec<String> {
        let mut out: Vec<String> = self.card.iter().flat_map(|c| c.phones.iter().map(|p| p.value.clone())).collect();
        out.extend(self.addresses.iter().filter_map(|a| a.strip_prefix(sioul_core::porch::TEL)).map(str::to_string));
        out
    }

    /// Their own row's id (decision 16): their card's (`person.contact:<UID>`),
    /// else their number's (`person.tel:<key>`) or their address's; none when
    /// nothing of theirs is known. Never a name.
    fn row_id(&self) -> Option<String> {
        if let Some(uid) = self.card.as_ref().map(|c| c.uid.trim().to_string()).filter(|u| !u.is_empty()) {
            let id = attention::person_row(&format!("contact:{uid}"));
            if attention::is_person(&id) {
                return Some(id);
            }
            // A UID no row can name (spaces in it): their card's first number (review of part 6, F9).
            let number = self.card.as_ref().and_then(|c| c.phones.iter().map(|p| sioul_core::phones::key(&p.value, region())).find(|k| sioul_core::phones::is_whole(k)));
            return number.map(|key| attention::person_row(&format!("tel:{key}"))).filter(|id| attention::is_person(id));
        }
        let first = self.addresses.first()?;
        let who = match first.strip_prefix(sioul_core::porch::TEL) {
            Some(number) => format!("tel:{}", sioul_core::phones::key(number, region())),
            None => first.clone(),
        };
        Some(attention::person_row(&who)).filter(|id| attention::is_person(id) && id != attention::PERSON_ROW && !id.ends_with("tel:"))
    }

    /// Their entry on the Always through list, when they are on it.
    fn always(&self, config: &Config) -> Option<String> {
        let people = People::load(&People::default_path());
        let region = sioul_core::phones::chosen(config.contacts.region.as_deref(), &tr().text("qt-locale", None));
        let uid = self.card.as_ref().map(|c| c.uid.trim().to_string()).unwrap_or_default();
        let emails = self.emails();
        let phones = self.phones();
        people
            .people
            .iter()
            .find(|p| {
                (!uid.is_empty() && p.contact == uid)
                    || emails.iter().any(|e| p.emails.iter().any(|x| x.trim().eq_ignore_ascii_case(e)))
                    || phones.iter().any(|n| {
                        let key = sioul_core::phones::key(n, region);
                        sioul_core::phones::is_whole(&key) && p.phones.iter().any(|x| sioul_core::phones::key(x, region) == key)
                    })
            })
            .map(|p| p.id.clone())
    }

    /// Their list, as the card's ("Their list") or the address's ("Their mail").
    fn list(&self) -> Value {
        match &self.card {
            Some(card) => serde_json::from_str(&crate::senders::person_json(&card.key)).unwrap_or(Value::Null),
            None => {
                let mut said: Value = serde_json::from_str(&crate::senders::standing_json(&json!(self.addresses).to_string())).unwrap_or(Value::Null);
                said["who"] = said["standing"].clone();
                said["own"] = json!([]);
                said
            }
        }
    }
}

/// Their own times on their sheet (decision 16): {title, help, shown, note,
/// cells [{column, label, level, choices [{value, label}]}]}; none to set for
/// someone on Always through, nor for someone known by nothing here.
fn own_times(attention: &Attention, someone: &Someone, always: bool, who: Who) -> Value {
    let id = someone.row_id();
    let note = if always { text("attention-sheet-times-always") } else if id.is_none() { text("attention-sheet-times-none") } else { String::new() };
    let set = id.as_deref().and_then(|id| attention.source(id));
    let label = |level: Level| match level {
        Level::Now => text("attention-level-now"),
        Level::Later => text("attention-person-held"),
        Level::Quiet => text("attention-level-quiet-messages"),
        _ => text("attention-level-as"),
    };
    let cells: Vec<Value> = if always || id.is_none() || who == Who::Blocked {
        Vec::new()
    } else {
        Column::ALL
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let level = set.map_or(Level::As, |cells| cells[index]);
                let choices: Vec<Value> = attention::person_choices(*column).iter().map(|l| json!({ "value": l.id(), "label": label(*l) })).collect();
                json!({ "column": column.id(), "label": attention::column_label(tr(), *column), "level": level.id(), "choices": choices })
            })
            .collect()
    };
    json!({ "title": text("attention-sheet-times"), "help": text("attention-sheet-times-help"), "shown": !cells.is_empty(), "note": note, "cells": cells })
}

/// A person's sheet, as JSON (PersonSheet.qml): {title, name, who, said,
/// choice, choices [{value, label}], own \[lines\], always, lines [one per
/// channel], note, problem, calls_title, calls [their calls of the month,
/// newest first]}. `key`: a card's file or UID; `addresses`: a sender's, a
/// JSON array or one, or a caller's number (`tel:…`) (their card found when
/// one has them).
pub(crate) fn person(key: &str, addresses: &str) -> String {
    person_with(&Someone::of(key, addresses), String::new())
}

fn person_with(someone: &Someone, note: String) -> String {
    let config = load_config();
    let attention = crate::hours::attention();
    let list = someone.list();
    let who = list["who"].as_str().and_then(Who::read).unwrap_or(Who::Stranger);
    // One of your people: the row Your people (their card's, judged as every sender is).
    let row = Person::of(who, list["circle"] == true);
    let always = someone.always(&config).is_some() && who != Who::Blocked;
    let reach = reach();
    let lines: Vec<String> = Channel::ALL
        .iter()
        .filter_map(|channel| {
            // Someone no list nor address book knows: their calls are the call screening's, apart.
            let line = if *channel == Channel::Calls && !always && attention::screened(Row::People(Channel::Calls, row)) { text("attention-sheet-line-calls-screening") } else { channel_line(&attention, *channel, row, always, "attention-sheet-line")? };
            let missing = match channel {
                Channel::Calls if !reach.calls => Some(text("attention-sheet-calls-none")),
                Channel::Calls if someone.phones().is_empty() => Some(text("attention-sheet-no-number")),
                Channel::Mail if someone.emails().is_empty() => Some(text("attention-sheet-no-address")),
                _ => None,
            };
            Some(match missing {
                Some(missing) => format!("{line} {missing}"),
                None => line,
            })
        })
        .collect();
    json!({
        "title": say("attention-sheet-title", &[("name", someone.name())]),
        "name": someone.name(),
        "card": someone.card.as_ref().map(|c| c.key.clone()).unwrap_or_default(),
        "addresses": someone.addresses,
        "who": who.id(),
        "circle": row == Person::People,
        "said": list["said"],
        "choice": list["choice"],
        "choices": list["choices"],
        "own": list["own"],
        "always": always,
        "always_help": text(if who == Who::Blocked { "attention-sheet-always-blocked" } else if always { "attention-sheet-always-help" } else { "attention-sheet-always-off-help" }),
        "lines": lines,
        "times": own_times(&attention, someone, always, who),
        "note": note,
        // Their calls of the month, from every phone sharing its own, rang or declined.
        "calls_title": text("calls-history-title"),
        "calls": if who == Who::Blocked { Vec::new() } else { crate::calls::history(&someone.phones()) },
        // Their messages of the week, from the phones that share them.
        "texts_title": text("phonemsgs-history-title"),
        "texts": if who == Who::Blocked { Vec::new() } else { crate::phonemsgs::history(&someone.phones()) },
        // texts: their conversation on the Texts page, when texts are read here ("" otherwise).
        "texts_id": crate::texts::conversation_of(&someone.phones()),
    })
    .to_string()
}

/// A change made on a person's sheet (`verb`: "list", `value` a list's name
/// or "" for their categories; "always", `value` "on" or "off"). Blocked and
/// Always through exclude each other (Q3): choosing one takes them off the
/// other, said. Returns {sheet: the sheet again, line: what the status line says}.
pub(crate) fn person_change(key: &str, addresses: &str, verb: &str, value: &str) -> String {
    let someone = Someone::of(key, addresses);
    let config = load_config();
    let mut said: Vec<String> = Vec::new();
    match verb {
        "list" => {
            let was_always = someone.always(&config).is_some();
            let line = match &someone.card {
                Some(card) => crate::senders::set_person(&card.key, value),
                None => crate::senders::set_standing_of(&json!(someone.addresses).to_string(), value),
            };
            said.push(line);
            // Blocking takes them off Always through (`porch::set_standing`); someone written
            // on it by hand, with none of the card's lines, is taken off here.
            if value == "blocked" && was_always {
                if let Some(id) = someone.always(&load_config()) {
                    let _ = crate::everywhere::change("remove", &json!({ "id": id }).to_string());
                }
                said.push(say("attention-sheet-off-always", &[("name", someone.name())]));
            }
        }
        "always" if value == "on" => {
            // Off the blocked list first: their own list as their categories say, neutral when these still block them.
            let blocked = |someone: &Someone| Who::read(someone.list()["who"].as_str().unwrap_or_default()) == Some(Who::Blocked);
            let was_blocked = blocked(&someone);
            if was_blocked {
                let choose = |choice: &str| match &someone.card {
                    Some(card) => crate::senders::set_person(&card.key, choice),
                    None => crate::senders::set_standing_of(&json!(someone.addresses).to_string(), choice),
                };
                choose("");
                if blocked(&someone) {
                    choose("neutral");
                }
            }
            let (answer, _) = match &someone.card {
                Some(card) => crate::everywhere::change("add-contact", &json!({ "uid": card.uid }).to_string()),
                // A sender's addresses, or a caller's number (a call's line on the Porch).
                None => crate::everywhere::change("add", &json!({ "id": "", "name": "", "phones": someone.phones().join("\n"), "emails": someone.emails().join("\n") }).to_string()),
            };
            said.push(say("attention-sheet-on-always", &[("name", someone.name())]));
            if was_blocked {
                said.push(say("attention-sheet-off-blocked", &[("name", someone.name())]));
            }
            // What the list said: why it could not be written, or that they came off the blocked list.
            said.push(serde_json::from_str::<Value>(&answer).ok().and_then(|a| a["said"].as_str().map(str::to_string)).unwrap_or_default());
        }
        "always" => {
            if let Some(id) = someone.always(&config) {
                let _ = crate::everywhere::change("remove", &json!({ "id": id }).to_string());
                said.push(say("attention-sheet-off-always", &[("name", someone.name())]));
            }
        }
        // A cell of their own row ({"column", "level"}: now, later, quiet under do-not-disturb, as).
        "times" => match (someone.row_id(), someone.always(&config).is_some()) {
            (Some(id), false) => {
                let asked: Value = serde_json::from_str(value).unwrap_or(Value::Null);
                let word = format!("{}:{}", asked["column"].as_str().unwrap_or_default(), asked["level"].as_str().unwrap_or_default());
                if let Err(e) = attention::apply_change(&config_path(), &format!("attention.{id}"), None, &SettingValue::Texts(vec![word])) {
                    said.push(e);
                }
            }
            (_, true) => said.push(text("attention-sheet-times-always")),
            (None, _) => said.push(text("attention-sheet-times-none")),
        },
        _ => {}
    }
    let line: Vec<String> = said.into_iter().filter(|s| !s.is_empty()).collect();
    json!({ "sheet": serde_json::from_str::<Value>(&person_with(&Someone::of(key, addresses), line.join(" "))).unwrap_or(Value::Null), "line": line.join(" ") }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decision 16 of 18 October 2026: a person's own times on their sheet,
    /// a cell each (in the shade under do-not-disturb alone), their row by
    /// their card's UID, else their number; none on Always through; written
    /// with no name.
    #[test]
    fn a_sheet_sets_their_own_times_with_no_name() {
        speak("en");
        let card = sioul_core::contacts::Contact { uid: "card-9".into(), key: "/books/card-9.vcf".into(), name: "Noa Example".into(), ..Default::default() };
        let someone = Someone { card: Some(card), addresses: Vec::new() };
        assert_eq!(someone.row_id().as_deref(), Some("person.contact:card-9"));
        // Written whole, as every language reads it (a national number reads by the language's country).
        assert_eq!(Someone { card: None, addresses: vec!["tel:+33 1 99 00 12 34".into()] }.row_id().as_deref(), Some("person.tel:+33199001234"));
        assert_eq!(Someone { card: None, addresses: vec!["noa@example.org".into()] }.row_id().as_deref(), Some("person.noa@example.org"));
        // A UID with a space: their card's first number (review of part 6, F9).
        let spaced = sioul_core::contacts::Contact { uid: "card 10".into(), phones: vec![sioul_core::contacts::Labeled { label: String::new(), value: "+33 1 99 00 12 35".into() }], ..Default::default() };
        assert_eq!(Someone { card: Some(spaced), addresses: Vec::new() }.row_id().as_deref(), Some("person.tel:+33199001235"));
        let attention = Attention::usual();
        let times = own_times(&attention, &someone, false, Who::Neutral);
        let cells = times["cells"].as_array().unwrap();
        assert_eq!(cells.len(), 9);
        assert!(cells.iter().all(|c| c["level"] == "as"), "{cells:?}");
        let offered = |column: &str| cells.iter().find(|c| c["column"] == column).unwrap()["choices"].as_array().unwrap().iter().map(|c| c["value"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(offered("work"), ["now", "later", "as"]);
        assert_eq!(offered("dnd"), ["now", "quiet", "later", "as"]);
        assert_eq!(own_times(&attention, &someone, true, Who::Neutral)["shown"], false, "Always through: no row of their own");
        // Written as the sheet writes it: their row by its id, no name.
        let id = someone.row_id().unwrap();
        let dir = std::env::temp_dir().join(format!("sioul-own-times-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        attention::apply_change(&path, &format!("attention.{id}"), None, &SettingValue::Texts(vec!["work:later".into()])).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("person.contact:card-9") && written.contains("work:later") && !written.contains("Noa"), "{written}");
        let read = Attention::of(&Config::load(&path).unwrap());
        assert_eq!(own_times(&read, &someone, false, Who::Neutral)["cells"][0]["level"], "later");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The card's call screening in a sentence, for each rule and both hours, in both languages.
    #[test]
    fn the_card_says_the_call_screening() {
        for language in ["en", "fr"] {
            speak(language);
            for (words, has) in [
                ("", "voicemail"),
                ("[screening]\nunknown = \"work\"\n", "work"),
                ("[screening]\nunknown = \"ring\"\nhours = \"outside-work\"\n", "ring"),
                ("[screening]\non = false\n", "off"),
            ] {
                let config: Config = toml::from_str(words).unwrap();
                let line = screening_line(&config);
                said_well(&line, language);
                assert!(!line.contains("reach-card"), "{language} {has}: {line}");
            }
            let at_all_hours: Config = toml::from_str("").unwrap();
            let outside: Config = toml::from_str("[screening]\nhours = \"outside-work\"\n").unwrap();
            assert_ne!(screening_line(&at_all_hours), screening_line(&outside), "{language}: the hours said");
        }
    }

    /// This thread speaks `language` from now on.
    fn speak(language: &str) {
        let translator: &'static Translator = Box::leak(Box::new(Translator::new(language)));
        SPOKEN.with(|spoken| spoken.set(Some(translator)));
    }

    /// A sentence said whole: no key, no placeable, a capital, a full stop; French typography, « type ».
    fn said_well(line: &str, language: &str) {
        assert!(!line.trim().is_empty(), "{language}: an empty line");
        assert!(!line.contains("attention-") && !line.contains('{') && !line.contains('$'), "{language}: {line}");
        assert!(line.chars().next().is_some_and(|c| !c.is_lowercase()), "{language}: no capital: {line}");
        assert!(line.ends_with('.') || line.ends_with(".)"), "{language}: no full stop: {line}");
        assert!(!line.contains("sorte"), "{language}: {line}");
        if language == "fr" {
            for mark in [" :", " ;", " !", " ?", "« ", " »", "'"] {
                assert!(!line.contains(mark), "fr: typography ({mark:?}) in {line}");
            }
        }
    }

    /// Every cell at the last value it may take, Always through's layers at ☆: each value said somewhere.
    fn changed_everywhere() -> Attention {
        let mut a = Attention::usual();
        for row in Row::ALL {
            for column in Column::ALL {
                if let Some(level) = attention::choices(row, column).last() {
                    let _ = a.set(row, column, *level);
                }
                if row.always() && !column.is_time() {
                    let _ = a.set(row, column, Level::Through);
                }
            }
        }
        a
    }

    /// A label or a short cell said whole: no key, no placeable; French typography, « type ».
    fn worded(word: &str, language: &str) {
        assert!(!word.trim().is_empty(), "{language}: an empty word");
        assert!(!word.contains("reach-") && !word.contains("attention-") && !word.contains('{') && !word.contains('$'), "{language}: {word}");
        assert!(!word.contains("sorte") && !word.contains("genre"), "{language}: {word}");
        if language == "fr" {
            for mark in [" :", " ;", " !", " ?", "« ", " »", "'"] {
                assert!(!word.contains(mark), "fr: typography ({mark:?}) in {word}");
            }
        }
    }

    /// Every line of the page in both languages, for the usual matrix, the
    /// presets and a matrix changed everywhere, on a computer and a phone:
    /// the rule's sentences, the counts, the card's choices, the worked-out
    /// grid's lines that decide, the sentences where a rule does nothing.
    #[test]
    fn every_line_of_the_page_is_said_whole_in_both_languages() {
        for language in ["en", "fr"] {
            speak(language);
            for preset in Preset::ALL {
                for line in exceptions::rule(tr(), preset, true) {
                    said_well(&line.text, language);
                }
            }
            for (attention, start) in [(Attention::usual(), Preset::Usual), (Preset::Quieter.matrix(), Preset::Quieter), (Preset::Reachable.matrix(), Preset::Reachable), (changed_everywhere(), Preset::Usual)] {
                for phone in [false, true] {
                    let reach = Reach { phone, calls: phone, messages: phone, ..Reach::default() };
                    let card = card_choices(reach, &[Device { id: "p".into(), name: "Phone".into(), phone: true, here: true }], None, &attention);
                    for part in ["what", "whom", "where"] {
                        for choice in card[part].as_array().unwrap() {
                            worded(choice["label"].as_str().unwrap(), language);
                        }
                    }
                    let grid = worked_out(&attention, &[], &table_rows(phone), start, &Config::default(), phone);
                    for row in grid["rows"].as_array().unwrap() {
                        for cell in row["cells"].as_array().unwrap() {
                            said_well(cell["by"].as_str().unwrap(), language);
                        }
                    }
                }
            }
            for id in ["reach-idle-no-phone", "reach-idle-messages-no-access", "reach-idle-calls", "reach-idle-messages-access", "reach-idle-messages-hold", "reach-idle-messages-people", "reach-idle-calls-phone", "reach-by-sioul", "reach-where-this-phone", "reach-where-phones", "reach-where-every", "reach-not-kept", "reach-same-as-usual", "reach-not-offered", "reach-not-saved", "reach-remove-phone-ask", "reach-by-free-nothing"] {
                said_well(&text(id), language);
            }
            for (id, n) in [("reach-count-people", 1), ("reach-count-people", 5), ("reach-count-groups", 2), ("reach-count-domains", 1), ("reach-count-cards", 312)] {
                worded(&tr().text(id, Some(&tr().counted(n))), language);
            }
            for id in ["reach-rule", "reach-exceptions", "reach-add", "reach-dnd", "reach-apps", "reach-apps-computer", "reach-worked-out", "reach-circles", "reach-start", "reach-what", "reach-whom", "reach-where", "reach-when", "reach-whom-person", "reach-when-own", "reach-what-anything", "reach-where-everywhere", "reach-edit", "reach-remove", "reach-card-add", "reach-card-edit", "reach-save", "reach-add-for-this", "reach-none", "reach-find", "reach-remove-phone-take", "reach-remove-phone-keep"] {
                worded(&text(id), language);
            }
            for id in ["reach-removed", "reach-added", "reach-changed"] {
                said_well(&say(id, &[("line", "Mail from your contacts: never.".to_string())]), language);
            }
            said_well(&say("reach-started", &[("start", text("attention-preset-quieter"))]), language);
            let mut args = tr().counted(2);
            args.set("start", text("attention-preset-quieter"));
            args.set("lines", "Mail from your contacts: never. Messages from your people: always.".to_string());
            said_well(&tr().text("reach-start-replaces", Some(&args)), language);
            said_well(&text("reach-rule-help"), language);
        }
        assert_eq!(capital("les courriels"), "Les courriels");
        assert_eq!(small("Temps libre"), "temps libre");
    }

    /// As Sioul ships it, the page says the rule in seven sentences and
    /// lists no exception; the worked-out grid shows the calls' rows (not
    /// the call screening's), each cell decided by the usual rule; an
    /// exception decides its own cells, marked, and the cell it would start
    /// from is its row's.
    #[test]
    fn the_usual_page_says_the_rule_and_no_exception() {
        speak("en");
        let usual = Attention::usual();
        let rows = table_rows(false);
        assert!(rows.contains(&Row::People(Channel::Calls, Person::Neutral)) && !rows.contains(&Row::People(Channel::Calls, Person::Stranger)));
        let grid = worked_out(&usual, &[], &rows, Preset::Usual, &Config::default(), false);
        let cell = |grid: &Value, row: &str, column: &str| grid["rows"].as_array().unwrap().iter().find(|r| r["id"] == row).unwrap()["cells"].as_array().unwrap().iter().find(|c| c["column"] == column).unwrap().clone();
        let safe = cell(&grid, "mail.safe", "work");
        assert_eq!(safe["by"], "Decided by the usual rule for Safe.");
        assert_eq!(safe["changed"], false);
        assert_eq!(safe["except"]["what"]["channel"], "mail");
        assert_eq!(safe["except"]["when"]["pattern"], "awake");
        let mut changed = Attention::usual();
        changed.set(Row::People(Channel::Mail, Person::Neutral), Column::Leisure, Level::Never).unwrap();
        let list = vec![Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Neutral }, place: Place::Everywhere, when: When::Own { cells: Column::ALL.map(|c| changed.cell(Row::People(Channel::Mail, Person::Neutral), c)) }, store: None }];
        let grid = worked_out(&changed, &list, &rows, Preset::Usual, &Config::default(), false);
        let contacts = cell(&grid, "mail.neutral", "leisure");
        assert_eq!(contacts["changed"], true);
        assert!(contacts["by"].as_str().unwrap().starts_with("Decided by your exception: Mail from your contacts:"), "{}", contacts["by"]);
        assert_eq!(cell(&grid, "messages.neutral", "leisure")["changed"], false);
        assert_eq!(cell(&grid, "codes", "work")["by"], text("reach-by-sioul"));
    }

    /// What the matrix alone does not say is said over it: new mail never
    /// told (mail that would come at once is shown without a word, its line
    /// named), Free time's "Nothing at all" (every state's row held); your
    /// people's mail that follows Safe's is decided by Safe's line.
    #[test]
    fn the_grid_says_what_the_switches_and_safe_s_line_decide() {
        speak("en");
        let rows = vec![Row::People(Channel::Mail, Person::Safe), Row::People(Channel::Mail, Person::People), Row::People(Channel::Messages, Person::Neutral)];
        let mut config = Config::default();
        config.reminders.mail = false;
        config.free_time.nothing = true;
        let safe_row = Row::People(Channel::Mail, Person::Safe);
        let mut attention = Attention::usual();
        attention.set(safe_row, Column::Leisure, Level::Later).unwrap();
        assert!(attention.follows(Row::People(Channel::Mail, Person::People)));
        let safe = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Safe }, place: Place::Everywhere, when: When::Own { cells: Column::ALL.map(|c| attention.cell(safe_row, c)) }, store: Some(exceptions::Store::Rows { rows: vec![safe_row.id()] }) };
        let new_mail = Exception { what: What::Own { notice: Notice::NewMail }, whom: Whom::Anyone, place: Place::Everywhere, when: When::pattern(exceptions::Pattern::Never), store: Some(exceptions::Store::Switch { key: "reminders.mail".into() }) };
        let list = vec![safe, new_mail];
        let grid = worked_out(&attention, &list, &rows, Preset::Usual, &config, false);
        let cell = |row: &str, column: &str| grid["rows"].as_array().unwrap().iter().find(|r| r["id"] == row).unwrap()["cells"].as_array().unwrap().iter().find(|c| c["column"] == column).unwrap().clone();
        let work = cell("mail.safe", "work");
        assert_eq!(work["value"], "quiet", "{work}");
        assert_eq!(work["by"], say("reach-by-exception", &[("line", exceptions::sentence(tr(), &list[1]))]));
        assert_eq!(work["changed"], true);
        let people = cell("mail.people", "leisure");
        assert_eq!(people["by"], say("reach-by-exception", &[("line", exceptions::sentence(tr(), &list[0]))]), "Safe's line decides your people's mail");
        let free = cell("messages.neutral", "free");
        assert_eq!(free["value"], "later");
        assert_eq!(free["by"], text("reach-by-free-nothing"));
        for language in ["en", "fr"] {
            speak(language);
            said_well(&text("reach-by-free-nothing"), language);
        }
    }


    /// Your people (decision of 20 November 2026): no line, no row of What
    /// comes when and no word of theirs while Contacts ▸ Your people holds
    /// nobody besides you, one sentence and its door instead; once someone is
    /// there, their line between Always through and Safe, reading When awake
    /// as Safe's, their rows in What comes when, and Who is in which circle
    /// listing each by the row they are judged on (reviews M1, M5, L8): no
    /// card linked, none on this device, a card of a book someone else can
    /// write, a card or an address held out by a restriction, each said with
    /// its door; in both languages, said well. The stand-ins are the store
    /// and the address books, judged by the program's own `porch::Senders`.
    #[test]
    fn your_people_get_a_line_once_someone_is_in_it() {
        use sioul_core::persons::{Circle, Person as Someone, Persons};
        use sioul_core::porch::{Book, SenderList, Senders};
        let usual = Attention::usual();
        let card = |uid: &str, name: &str, emails: &[&str], place: &str| sioul_core::contacts::Contact { uid: uid.into(), key: format!("/stand-in/{uid}.vcf"), name: name.into(), place: place.into(), emails: emails.iter().map(|e| sioul_core::contacts::Labeled { label: String::new(), value: e.to_string() }).collect(), ..sioul_core::contacts::Contact::default() };
        let cards = [card("c-lea", "Léa Martin", &["lea@example.org"], ""), card("c-maud", "Maud", &["maud@example.org"], ""), card("c-ugo", "Ugo", &["ugo@example.org", "ugo.home@example.org"], ""), card("c-vic", "Vic", &["vic@example.org"], "nextcloud/shared")];
        let held: std::collections::BTreeSet<String> = ["nextcloud/shared".to_string()].into();
        let today = sioul_core::zone::now().date();
        let stand_in = |store: &Persons| {
            let senders = Senders { restricted: SenderList::parse("contact:c-maud\nugo@example.org"), contacts: std::sync::Arc::new(Book::trusting(&cards, None, &held)), circle: std::sync::Arc::new(Circle::of(store, today)), ..Senders::default() };
            STORE.with(|kept| *kept.borrow_mut() = Some((store.clone(), senders)));
        };
        for language in ["en", "fr"] {
            speak(language);
            // Nobody but you, and a child's other parent: no line.
            let mut store = Persons::default();
            store.put(Someone { name: "Sam".into(), relation: "other-parent".into(), cards: vec!["c-sam".into()], ..Someone::default() });
            stand_in(&store);
            assert!(your_people().is_empty(), "{language}");
            assert_eq!(exceptions::rule(tr(), Preset::Usual, circle_holds_someone()).len(), 6, "{language}: no line for nobody");
            assert!(!table_rows(false).iter().any(|r| matches!(r, Row::People(_, Person::People))), "no row for nobody");
            assert!(!matrix_persons(Channel::Calls).contains(&Person::People));
            // Léa, a card here; Alix, none linked; Noé, a card linked that is not on this device; Maud, a
            // card on Restricted by hand; Ugo, one address restricted; Vic, a card of a book others can write.
            store.put(Someone { name: "Léa".into(), relation: "child".into(), cards: vec!["c-lea".into()], ..Someone::default() });
            store.put(Someone { name: "Alix".into(), relation: "helped".into(), ..Someone::default() });
            store.put(Someone { name: "Noé".into(), relation: "kin".into(), cards: vec!["c-gone".into()], ..Someone::default() });
            store.put(Someone { name: "Maud".into(), relation: "kin".into(), cards: vec!["c-maud".into()], ..Someone::default() });
            store.put(Someone { name: "Ugo".into(), relation: "kin".into(), cards: vec!["c-ugo".into(), "c-gone-too".into()], ..Someone::default() });
            store.put(Someone { name: "Vic".into(), relation: "kin".into(), cards: vec!["c-vic".into()], ..Someone::default() });
            stand_in(&store);
            assert_eq!(exceptions::rule(tr(), Preset::Usual, circle_holds_someone()).len(), 7, "{language}: their line");
            assert!(!count_of(Person::People).is_empty());
            // Review M9: as Safe is now, tuned by a preset or by hand, in the worked-out grid.
            let quieter = Preset::Quieter.matrix();
            let grid = attention::grid(&quieter, tr(), &table_rows(false), Preset::Quieter);
            let cells = |id: &str| grid.rows.iter().find(|r| r.id == id).map(|r| r.cells.iter().map(|c| c.value.clone()).collect::<Vec<_>>()).unwrap();
            assert_eq!(cells("mail.people"), cells("mail.safe"), "What comes when: theirs as Safe's");
            let listed: Vec<(String, Vec<(String, String, String)>)> = your_people()
                .iter()
                .map(|p| (p.name.clone(), p.notes.iter().map(|n| (n["text"].as_str().unwrap().to_string(), n["door"].as_str().unwrap().to_string(), n["key"].as_str().unwrap().to_string())).collect()))
                .collect();
            let notes = |name: &str| listed.iter().find(|(n, _)| n == name).map(|(_, notes)| notes.clone()).unwrap();
            assert_eq!(listed.len(), 6, "everyone but the other parent: {listed:?}");
            assert!(notes("Léa").is_empty(), "Léa's card here, judged on Your people: nothing to say");
            for name in ["Alix", "Noé"] {
                let said = notes(name);
                assert_eq!(said.len(), 1, "{name}: {said:?}");
                said_well(&said[0].0, language);
                let id = &store.persons.iter().find(|p| p.name == name).unwrap().id;
                assert!(said[0].0.contains(name) && said[0].1 == "page" && &said[0].2 == id, "{name}: their page, where a card is linked: {said:?}");
            }
            assert_ne!(notes("Alix")[0].0, notes("Noé")[0].0, "no card linked, and a card linked that is not here: two sentences");
            let maud = notes("Maud");
            assert_eq!((maud.len(), maud[0].1.as_str(), maud[0].2.as_str()), (1, "sheet", "/stand-in/c-maud.vcf"), "{maud:?}");
            said_well(&maud[0].0, language);
            assert!(maud[0].0.starts_with("Maud") && maud[0].0.contains(&text("sender-one-restricted")), "{}", maud[0].0);
            let ugo = notes("Ugo");
            assert_eq!(ugo.len(), 2, "one card not here, one address restricted: {ugo:?}");
            assert!(ugo.iter().any(|(t, d, _)| t.contains("ugo@example.org") && d == "sheet") && ugo.iter().any(|(_, d, _)| d.is_empty()), "{ugo:?}");
            for (said, _, _) in &ugo {
                said_well(said, language);
            }
            let vic = notes("Vic");
            assert_eq!(vic.len(), 1, "{vic:?}");
            said_well(&vic[0].0, language);
            for id in ["reach-circles-tie", "sender-circle", "person-card-sheet", "phone-calls-row-people"] {
                worded(&text(id), language);
            }
            // A person's sheet and page say why, in the circle's words.
            for id in ["sender-from-circle", "person-reaches", "person-reaches-untied"] {
                said_well(&say(id, &[("name", "Léa".to_string())]), language);
            }
            said_well(&text("phone-calls-people-note"), language);
            said_well(&text("phone-calls-people-dnd"), language);
            for channel in [Channel::Mail, Channel::Calls, Channel::Messages] {
                assert!(table_rows(true).contains(&Row::People(channel, Person::People)), "{channel:?}");
                said_well(&channel_line(&usual, channel, Person::People, false, "attention-sheet-line").unwrap(), language);
            }
            assert!(matrix_persons(Channel::Calls).contains(&Person::People));
            // The moment now names them where they come.
            assert!(!who_words(&[Person::People], false, false).is_empty());
        }
        STORE.with(|kept| *kept.borrow_mut() = None);
        speak("en");
        assert_eq!(who_words(&[Person::People, Person::Neutral], false, false), "your people and your contacts");
        // Their calls ring at every time: the sheet's line says so.
        assert_eq!(channel_line(&usual, Channel::Calls, Person::People, false, "attention-sheet-line").unwrap(), tr().text("attention-sheet-line-calls-now", Some(&{
            let mut args = sioul_core::i18n::args();
            args.set("during", text("attention-during-any"));
            args.set("first", "yes");
            args
        })));
    }

    /// The notice that your people have a line of their own (review M2):
    /// due while the circle holds someone and it was never told, about What
    /// reaches you's People line, informative, said well in both languages;
    /// none once told, none for nobody.
    #[test]
    fn your_people_are_told_once_with_the_door() {
        use sioul_core::persons::{CIRCLE_TOLD, Person as Someone, Persons};
        let today = jiff::civil::date(2026, 11, 20);
        let mut store = Persons::default();
        assert!(circle_notice(&store, today).is_none(), "nobody: nothing");
        store.put(Someone { name: "Léa".into(), relation: "child".into(), cards: vec!["c-lea".into()], ..Someone::default() });
        for language in ["en", "fr"] {
            speak(language);
            let notice = circle_notice(&store, today).expect("due");
            assert_eq!(notice.about, "sioul:settings/attention.people", "its door: the People line");
            assert_eq!(notice.kind, sioul_core::notice::Kind::Minor, "to know");
            worded(&notice.title, language);
            said_well(&notice.body, language);
        }
        speak("en");
        store.circle_told = CIRCLE_TOLD;
        assert!(circle_notice(&store, today).is_none(), "told: never again");
    }

    /// On a computer, a phone in the sharing holds messages unless its entry
    /// says it does not give Sioul notification access (an older Sioul's,
    /// which says nothing, holds them); where no phone acts on a channel,
    /// the page says so beside the rules and the exceptions that do nothing.
    #[test]
    fn a_rule_that_does_nothing_says_so() {
        use sioul_sync::devices::{COMPUTER, Entry, PHONE};
        let entry = |kind: &str, notifications: Option<bool>, left: bool| Entry { id: "x".into(), kind: kind.into(), notifications, left, working: true, ..Entry::default() };
        assert_eq!(Phones::of(&[]), Phones::None);
        assert_eq!(Phones::of(&[entry(COMPUTER, None, false)]), Phones::None);
        assert_eq!(Phones::of(&[entry(PHONE, Some(true), false)]), Phones::Holding);
        assert_eq!(Phones::of(&[entry(PHONE, None, false)]), Phones::Holding, "an older Sioul says nothing: holding, as before phones said it");
        assert_eq!(Phones::of(&[entry(PHONE, Some(false), false)]), Phones::WithoutAccess, "it said no");
        assert_eq!(Phones::of(&[entry(PHONE, Some(false), false), entry(PHONE, None, true)]), Phones::WithoutAccess, "one that left counts no more");
        speak("en");
        let computer = |phones: Phones, calls: bool| Reach { messages: phones == Phones::Holding, phones, calls, ..Reach::default() };
        let said = |reach: Reach| idle(reach).into_iter().map(|(c, s)| (c, s)).collect::<Vec<_>>();
        assert!(said(computer(Phones::Holding, true)).is_empty());
        assert_eq!(said(computer(Phones::None, false)).len(), 2, "no phone: calls and messages");
        let without = said(computer(Phones::WithoutAccess, true));
        assert_eq!(without, [(Channel::Messages, text("reach-idle-messages-no-access"))]);
        // An exception on messages says it beside itself; one on mail, or on calls that act, says nothing.
        let on = |what: What| Exception { what, whom: Whom::Circle { person: Person::Safe }, place: Place::Everywhere, when: When::pattern(exceptions::Pattern::Always), store: None };
        assert_eq!(idle_of(&on(What::Channel { channel: Channel::Messages }), &without), text("reach-idle-messages-no-access"));
        assert_eq!(idle_of(&on(What::Channel { channel: Channel::Mail }), &without), "");
        assert_eq!(idle_of(&on(What::Anything), &without), "", "mail and calls still act");
        let nothing = said(computer(Phones::None, false));
        assert_eq!(idle_of(&on(What::Channel { channel: Channel::Calls }), &nothing), text("reach-idle-no-phone"));
        // Messages said once, from the channels' sentences.
        assert_eq!(channels(computer(Phones::WithoutAccess, false)), [Channel::Mail]);
        assert_eq!(channels(computer(Phones::Holding, false)), [Channel::Mail, Channel::Messages]);
    }
}

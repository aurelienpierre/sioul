// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Settings ▸ What reaches you (« Ce qui vous joint », ReachesTab.qml), as
//! the window reads it: the moment now in sentences, where you started from
//! (the presets), a card per time and per layer saying in words what comes
//! at once, what is shown without a notification, what waits and until
//! when, the matrix's grid, the exceptions; and a person's sheet
//! (PersonSheet.qml): their list and why, Always through, and what each
//! channel does with them at each time. The matrix is
//! `sioul_core::attention` (docs/attention.md); this file says it in words,
//! and writes what is changed here.

use crate::backend::{config_path, load_config};
use serde_json::{Value, json};
use sioul_core::attention::{self, Attention, Column, Kind, Level, Now, Person, Preset, Row};
use sioul_core::behaviours::{self, Behaviour, Circle, Group};
use sioul_core::config::{Config, SettingValue};
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
    out.extend([Alarms, Before, DayBefore, Dates, Needs, Move, WorkOver]);
    if phone {
        out.extend([AppAutomatons, AppAtOnce]);
    } else {
        out.extend([Sites, SitesLive, SiteCalls]);
    }
    out
}

/// The rows the tab shows here, by id: every channel's (By person says when
/// one does not apply yet), but the call screening's (numbers nobody knows and
/// hidden numbers on calls: Settings ▸ This phone), and the kinds that act on this device.
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
    for person in persons.iter().filter(|p| matches!(p, Person::Neutral | Person::Stranger | Person::Hidden | Person::Groups | Person::Always)) {
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
/// screening's aside: numbers nobody knows and hidden numbers, on calls.
fn matrix_persons(channel: Channel) -> Vec<Person> {
    attention::persons(channel).iter().copied().filter(|p| p.state() && !attention::screened(Row::People(channel, *p))).collect()
}

/// Sioul's own kinds in words, the four reminders together when all are
/// there: "your reminders, doses and the codes you ask for".
fn kinds_words(subjects: &[Kind]) -> Option<(String, bool)> {
    let reminders = [Kind::Alarms, Kind::Before, Kind::DayBefore, Kind::Dates];
    let together = reminders.iter().all(|k| subjects.contains(k));
    let mut items: Vec<(String, bool)> = Vec::new();
    for kind in subjects {
        if together && reminders.contains(kind) {
            if *kind == Kind::Alarms {
                items.push((text("attention-what-reminders"), true));
            }
            continue;
        }
        let singular = matches!(kind, Kind::Wake | Kind::DayBefore | Kind::Move | Kind::WorkOver);
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

// ---------------------------------------------------------------- the tab

/// The tab, as JSON (ReachesTab.qml; design-reach §1): {now: sentences,
/// state: "usual", "dnd" or "reachable", column: the time now, now_columns: those read now (a layer's too),
/// presets [{id, label, current, changes}], presets_note, changed, people {channels
/// [{id, label, note}], rows [{id, label, help, cells [{channel, value,
/// label, choices [{id, label}]}]}], blocked {label, help, word}, notes
/// \[lines\]}, own [{id, label, holds, value, label_value, choices [{id,
/// label}], switches [{kind, label, on}], gathered}], moments {rows [{id,
/// label, who, by, phone}], holds [{id, label, sioul, android}],
/// principle}, elsewhere {line, links [{id, label}]}, calls, rows [ids
/// shown in What comes when], grid (`attention::grid`, a short word per
/// cell), phone, exact}.
pub(crate) fn view(realtime: bool) -> String {
    let _ = realtime;
    let config = load_config();
    let attention = crate::hours::attention();
    let reach = reach();
    let now = crate::hours::attention_now();
    let current = column_now(&now);
    let usual = Attention::usual();
    let rows = table_rows(reach.phone);
    // What the page shows, alone, is compared (review R3): a row this device's
    // table does not show, and the calls' rows, are not counted.
    let presets: Vec<Value> = Preset::ALL
        .iter()
        .map(|p| {
            let changes = attention.changes_in(&p.matrix(), &rows);
            json!({ "id": p.id(), "label": text(&format!("attention-preset-{}", p.id())), "current": changes == 0, "changes": changes })
        })
        .collect();
    let from_usual = attention.changes_in(&usual, &rows);
    let changed = if from_usual == 0 { String::new() } else { tr().text("attention-changes", Some(&tr().counted(from_usual))) };
    let mut now_columns: Vec<&str> = now.times.iter().map(|c| c.id()).collect();
    now_columns.extend([(now.slot, Column::Slot), (now.dnd, Column::Dnd)].iter().filter(|(on, _)| *on).map(|(_, c)| c.id()));
    let grid = attention::grid(&attention, tr(), &rows, Preset::Usual);
    json!({
        "now": now_sentences(&attention, &now, reach),
        // As usual, Do not disturb or Reachable, as the status line's button says it.
        "state": crate::everywhere::moment()["state"].as_str().unwrap_or("usual"),
        "column": current.id(),
        "now_columns": now_columns,
        "presets": presets,
        // A preset leaves the calls as they are: each phone's (review R3).
        "presets_note": text("reach-presets-calls"),
        "changed": changed,
        "people": people(&attention, reach),
        "own": own_groups(&attention, &config),
        "moments": moments(),
        "elsewhere": elsewhere(&attention, &config, reach),
        "calls": text("reach-calls"),
        "rows": rows.iter().map(|r| r.id()).collect::<Vec<_>>(),
        "grid": grid,
        "phone": reach.phone,
        // Android lets Sioul set exact alarms (Devices says when it does not).
        "exact": !reach.phone || crate::alarms::exact(),
    })
    .to_string()
}

/// The rows of What comes when, by id: mail's and messages', but the blocked
/// (never, said once); Sioul's own that act on this device. Calls are set
/// for each phone in Settings ▸ Devices: their rows stay in the store and
/// the engine, never on this page.
fn table_rows(phone: bool) -> Vec<Row> {
    shown_rows(phone).into_iter().filter(|r| !matches!(r, Row::People(Channel::Calls, _))).collect()
}

/// A behaviour, or "Your own", in words.
fn behaviour_label(behaviour: Option<Behaviour>) -> String {
    match behaviour {
        Some(b) => text(&format!("reach-behaviour-{}", b.id())),
        None => text("reach-own"),
    }
}

/// The people's table (design 1.1): a line per circle, a cell per channel,
/// each the behaviour its rows read (`behaviours::reading`), "own" when
/// none, with the behaviours it may take; the blocked last, never; then
/// what stays automatic.
fn people(attention: &Attention, reach: Reach) -> Value {
    let channels: Vec<Value> = behaviours::CHANNELS.iter().map(|c| json!({ "id": c.id(), "label": text(&format!("attention-group-{}", c.id())), "note": channel_note(*c, reach) })).collect();
    let rows: Vec<Value> = Circle::ALL
        .iter()
        .map(|circle| {
            let cells: Vec<Value> = behaviours::CHANNELS
                .iter()
                .map(|channel| {
                    let read = behaviours::reading(attention, *channel, *circle);
                    let choices: Vec<Value> = behaviours::offered(*channel, *circle).iter().map(|b| json!({ "id": b.id(), "label": behaviour_label(Some(*b)), "help": if *b == Behaviour::Never { text(&format!("reach-behaviour-never-help-{}", channel.id())) } else { String::new() } })).collect();
                    json!({
                        "channel": channel.id(),
                        "value": read.map_or("own", Behaviour::id),
                        "label": behaviour_label(read),
                        "choices": choices,
                        "rows": behaviours::rows(*channel, *circle).iter().map(|r| r.id()).collect::<Vec<_>>(),
                    })
                })
                .collect();
            json!({ "id": circle.id(), "label": text(&format!("reach-circle-{}", circle.id())), "help": text(&format!("reach-circle-{}-help", circle.id())), "cells": cells })
        })
        .collect();
    json!({
        "channels": channels,
        "rows": rows,
        "blocked": { "label": text("reach-circle-blocked"), "help": text("reach-circle-blocked-help"), "word": text("attention-word-never") },
        "notes": [text("reach-people-automatic"), text("reach-people-mail-off"), text("reach-people-groups-note")],
        "own_help": text("reach-own-help"),
    })
}

/// Sioul's own notices in four groups (design 1.2): each its choice as its
/// rows read it (`behaviours::group_reading`), "own" when none; care's
/// switches one by one; the gathered times under Sites and apps.
fn own_groups(attention: &Attention, config: &Config) -> Value {
    let groups: Vec<Value> = Group::ALL
        .iter()
        .map(|group| {
            let read = behaviours::group_reading(attention, *group);
            let choice_label = |choice: &str| text(&format!("reach-choice-{}-{choice}", group.id()));
            let switches: Vec<Value> = if *group == Group::Care {
                group.kinds().iter().map(|k| json!({ "kind": k.id(), "label": attention::row_label(tr(), Row::Own(*k)), "on": behaviours::care(attention, *k) })).collect()
            } else {
                Vec::new()
            };
            let choices: Vec<Value> = if *group == Group::Care { Vec::new() } else { group.choices().iter().map(|c| json!({ "id": c, "label": choice_label(c) })).collect() };
            json!({
                "id": group.id(),
                "label": text(&format!("reach-group-{}", group.id())),
                "holds": text(&format!("reach-group-{}-holds", group.id())),
                "value": read.unwrap_or("own"),
                "value_label": match read {
                    Some(choice) if *group != Group::Care => choice_label(choice),
                    Some(_) => String::new(),
                    None => text("reach-own"),
                },
                "choices": choices,
                "switches": switches,
                "gathered": if *group == Group::Sites { say("reach-gathered", &[("times", gathered_times(config))]) } else { String::new() },
            })
        })
        .collect();
    json!(groups)
}

/// The three moments and who holds what on a phone (design 1.3), in words.
fn moments() -> Value {
    let rows: Vec<Value> = ["usual", "dnd", "reachable"]
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "label": text(&format!("reach-moment-{id}")),
                "who": text(&format!("reach-moment-{id}-who")),
                "by": text(&format!("reach-moment-{id}-by")),
                "phone": text(&format!("reach-moment-{id}-phone")),
            })
        })
        .collect();
    let holds: Vec<Value> = ["call", "message", "dnd", "modes"]
        .iter()
        .map(|id| json!({ "id": id, "label": text(&format!("reach-hold-{id}")), "sioul": text(&format!("reach-hold-{id}-sioul")), "android": text(&format!("reach-hold-{id}-android")) }))
        .collect();
    json!({
        "columns": [text("reach-moment-who"), text("reach-moment-by"), text("reach-moment-phone")],
        "rows": rows,
        "holds_title": text("reach-holds"),
        "holds_columns": [text("reach-holds-sioul"), text("reach-holds-android")],
        "holds": holds,
        "principle": text("reach-principle"),
    })
}

/// What is set elsewhere, counted (design 1.5): a site's Real time and
/// Silenced (a computer's sites), an event's own reminder (the coming
/// sixty days), Health's days changed from today on, a person's own times;
/// one line, and a link to each place that holds some.
fn elsewhere(attention: &Attention, config: &Config, reach: Reach) -> Value {
    let sites: Vec<sioul_core::sites::Site> = if reach.phone { Vec::new() } else { config.accounts.iter().filter_map(sioul_core::sites::Site::of).collect() };
    let live = sites.iter().filter(|s| s.realtime && !s.muted).count();
    let muted = sites.iter().filter(|s| s.muted).count();
    let now = sioul_core::zone::now();
    let stamp = now.timestamp().as_second();
    let events = sioul_core::agenda::occurrences(stamp, stamp + 60 * 86_400).iter().filter(|e| !e.remind.trim().is_empty()).map(|e| e.uid.clone()).collect::<std::collections::BTreeSet<_>>().len();
    let health = sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path()).0.keys().filter(|d| **d >= now.date()).count();
    let persons = attention.sources().keys().filter(|id| attention::is_person(id)).count();
    let counted = |id: &str, n: usize| {
        let mut args = sioul_core::i18n::args();
        args.set("n", n);
        tr().text(id, Some(&args))
    };
    let mut parts: Vec<String> = Vec::new();
    let mut links: Vec<Value> = Vec::new();
    let mut link = |id: &str| {
        if !links.iter().any(|l| l["id"] == id) {
            links.push(json!({ "id": id, "label": text(&format!("reach-elsewhere-open-{id}")) }));
        }
    };
    for (n, id, place) in [(live, "reach-elsewhere-sites-live", "sites"), (muted, "reach-elsewhere-sites-muted", "sites"), (events, "reach-elsewhere-events", "agenda"), (health, "reach-elsewhere-health", "health"), (persons, "reach-elsewhere-persons", "contacts")] {
        if n > 0 {
            parts.push(counted(id, n));
            link(place);
        }
    }
    let line = if parts.is_empty() { text("reach-elsewhere-none") } else { say("reach-elsewhere", &[("what", listed(&parts, false))]) };
    json!({ "line": line, "links": links })
}

/// A choice made on the page (`what`: "people.<channel>.<circle>",
/// "group.<group>", "care.<kind>"; `value`: a behaviour's or a choice's
/// id), the cells it names written (`attention::apply_rows`). "" when kept,
/// else why not.
pub(crate) fn choose(what: &str, value: &str) -> String {
    match choose_at(&config_path(), what, value) {
        Ok(()) => String::new(),
        Err(e) => e,
    }
}

/// `choose`, on the configuration at `path`.
fn choose_at(path: &std::path::Path, what: &str, value: &str) -> Result<(), String> {
    let config = if path.exists() { Config::load(path)? } else { Config::default() };
    writes_of(what, value).and_then(|rows| attention::apply_rows(path, &config, &rows))
}

/// The cells a choice of the page writes (`choose`), per row; why not, when it cannot.
fn writes_of(what: &str, value: &str) -> Result<Vec<(Row, behaviours::Cells)>, String> {
    if let Some(rest) = what.strip_prefix("people.") {
        let (channel, circle) = rest.split_once('.').unwrap_or((rest, ""));
        match (Channel::read(channel), Circle::read(circle), Behaviour::read(value)) {
            (Some(channel), Some(circle), Some(behaviour)) if behaviours::CHANNELS.contains(&channel) => behaviours::writes(channel, circle, behaviour),
            _ => Err(format!("{what}: {value}: not understood")),
        }
    } else if let Some(group) = what.strip_prefix("group.") {
        match Group::read(group) {
            Some(group) if group != Group::Care => behaviours::group_writes(group, value, None),
            _ => Err(format!("{what}: not understood")),
        }
    } else if let Some(kind) = what.strip_prefix("care.") {
        match Kind::read(kind) {
            Some(kind) => behaviours::group_writes(Group::Care, value, Some(kind)),
            None => Err(format!("{what}: not understood")),
        }
    } else {
        Err(format!("{what}: not understood"))
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
                Level::Never => {}
                _ => add(5, Subject::People(channel, person)),
            }
        }
    }
    // Of Sioul's own, what matters most: codes, doses, reminders; on a
    // computer, what is to know shown alone while a pause or a do-not-disturb holds.
    let held_here = !reach.phone && (mode.paused() || now.dnd || now.times.iter().any(|c| silenced(&load_config(), *c)));
    for kind in [Kind::Codes, Kind::Doses, Kind::Alarms, Kind::Before, Kind::DayBefore, Kind::Dates] {
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

/// What a channel's rows say of where they apply: calls once a phone screens
/// them; messages on a phone that holds other apps' notifications, as far
/// as the sharing knows (`Phones`); "" where they apply here.
fn channel_note(channel: Channel, reach: Reach) -> String {
    match channel {
        Channel::Calls if !reach.calls => text("attention-channel-calls-none"),
        Channel::Messages if !reach.messages && reach.phones == Phones::WithoutAccess => text("attention-channel-messages-no-access"),
        Channel::Messages if !reach.messages => text("attention-channel-messages-none"),
        Channel::Messages if !reach.phone => text("attention-channel-messages-phone"),
        _ => String::new(),
    }
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
        _ => "later",
    }
}

// ---------------------------------------------------------------- writing

/// A row changed from the tab: its words whole (`words`, a JSON list), as
/// `attention::apply` reads them. "" when kept, else why not.
pub(crate) fn set_row(row: &str, words: &str, shown: &str) -> String {
    let words: Vec<String> = serde_json::from_str(words).unwrap_or_default();
    // What the row showed: only the cells changed from it are set, over the row as it is now.
    let shown: Option<Vec<String>> = serde_json::from_str(shown).ok();
    match attention::apply_change(&config_path(), &format!("attention.{row}"), shown.map(SettingValue::Texts).as_ref(), &SettingValue::Texts(words)) {
        Ok(()) => String::new(),
        Err(e) => e,
    }
}

/// A preset chosen ("usual", "quieter", "reachable"): every row as it says,
/// the fixed cells untouched. "" when kept, else why not.
pub(crate) fn preset(id: &str) -> String {
    let Some(preset) = Preset::read(id) else { return format!("{id}: usual, quieter or reachable") };
    match attention::apply_preset(&config_path(), &load_config(), preset) {
        Ok(()) => String::new(),
        Err(e) => e,
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
    let always = someone.always(&config).is_some() && who != Who::Blocked;
    let reach = reach();
    let lines: Vec<String> = Channel::ALL
        .iter()
        .filter_map(|channel| {
            // Someone no list nor address book knows: their calls are the call screening's, apart.
            let line = if *channel == Channel::Calls && !always && attention::screened(Row::People(Channel::Calls, Person::of(who))) { text("attention-sheet-line-calls-screening") } else { channel_line(&attention, *channel, Person::of(who), always, "attention-sheet-line")? };
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

    /// Every word of the page, for the usual matrix, the presets and a matrix
    /// changed everywhere, on a computer and a phone, in both languages: the
    /// labels and cells whole, each help and note one sentence, said well.
    #[test]
    fn every_line_of_the_page_is_said_whole_in_both_languages() {
        let config = Config::default();
        for language in ["en", "fr"] {
            speak(language);
            for attention in [Attention::usual(), Preset::Quieter.matrix(), Preset::Reachable.matrix(), changed_everywhere()] {
                for phone in [false, true] {
                    let reach = Reach { phone, calls: phone, messages: phone, ..Reach::default() };
                    let page = people(&attention, reach);
                    for channel in page["channels"].as_array().unwrap() {
                        worded(channel["label"].as_str().unwrap(), language);
                    }
                    for row in page["rows"].as_array().unwrap() {
                        worded(row["label"].as_str().unwrap(), language);
                        said_well(row["help"].as_str().unwrap(), language);
                        for cell in row["cells"].as_array().unwrap() {
                            worded(cell["label"].as_str().unwrap(), language);
                            for choice in cell["choices"].as_array().unwrap() {
                                worded(choice["label"].as_str().unwrap(), language);
                            }
                        }
                    }
                    for note in page["notes"].as_array().unwrap() {
                        said_well(note.as_str().unwrap(), language);
                    }
                    said_well(page["own_help"].as_str().unwrap(), language);
                    said_well(page["blocked"]["help"].as_str().unwrap(), language);
                    for group in own_groups(&attention, &config).as_array().unwrap() {
                        worded(group["label"].as_str().unwrap(), language);
                        said_well(group["holds"].as_str().unwrap(), language);
                        for choice in group["choices"].as_array().unwrap() {
                            worded(choice["label"].as_str().unwrap(), language);
                        }
                        for switch in group["switches"].as_array().unwrap() {
                            worded(switch["label"].as_str().unwrap(), language);
                        }
                        if let Some(gathered) = group["gathered"].as_str().filter(|g| !g.is_empty()) {
                            said_well(gathered, language);
                        }
                    }
                }
                for channel in Channel::ALL {
                    for person in attention::persons(channel).iter().filter(|p| **p != Person::Always) {
                        for always in [false, true] {
                            said_well(&channel_line(&attention, channel, *person, always, "attention-sheet-line").unwrap(), language);
                        }
                    }
                }
            }
            let moments = moments();
            for word in moments["columns"].as_array().unwrap().iter().chain(moments["holds_columns"].as_array().unwrap()) {
                worded(word.as_str().unwrap(), language);
            }
            for row in moments["rows"].as_array().unwrap().iter().chain(moments["holds"].as_array().unwrap()) {
                for (key, value) in row.as_object().unwrap() {
                    if key != "id" {
                        worded(value.as_str().unwrap(), language);
                    }
                }
            }
            worded(&text("reach-holds"), language);
            said_well(moments["principle"].as_str().unwrap(), language);
            said_well(&text("reach-calls"), language);
            said_well(&text("reach-elsewhere-none"), language);
            for (id, n) in [("reach-elsewhere-sites-live", 1), ("reach-elsewhere-sites-muted", 2), ("reach-elsewhere-events", 3), ("reach-elsewhere-health", 1), ("reach-elsewhere-persons", 2)] {
                let mut args = sioul_core::i18n::args();
                args.set("n", n);
                let part = tr().text(id, Some(&args));
                worded(&part, language);
                said_well(&say("reach-elsewhere", &[("what", part)]), language);
            }
            for id in ["sites", "agenda", "health", "contacts"] {
                worded(&text(&format!("reach-elsewhere-open-{id}")), language);
            }
            for id in ["reach-tab-rules", "reach-tab-table", "reach-people", "reach-own-notices", "reach-moments", "reach-circles", "reach-start", "reach-calls-open", "busy", "busy-task"] {
                worded(&text(id), language);
            }
            for id in ["reach-circles-help", "busy-help", "busy-task-help", "reach-behaviour-never-help-mail", "reach-behaviour-never-help-messages"] {
                said_well(&text(id), language);
            }
            for key in ["attention-channel-calls-none", "attention-channel-messages-none", "attention-channel-messages-no-access", "attention-channel-messages-phone"] {
                said_well(&text(key), language);
            }
        }
        assert_eq!(capital("les courriels"), "Les courriels");
        assert_eq!(small("Temps libre"), "temps libre");
    }

    /// As Sioul ships it, the page reads a behaviour on every line and a
    /// choice in every group, in words; What comes when holds no call.
    #[test]
    fn the_usual_page_reads_as_sioul_ships_it() {
        let usual = Attention::usual();
        speak("en");
        let page = people(&usual, Reach { phone: true, calls: true, messages: true, ..Reach::default() });
        let read = |circle: &str| -> Vec<(String, String)> {
            let row = page["rows"].as_array().unwrap().iter().find(|r| r["id"] == circle).unwrap();
            row["cells"].as_array().unwrap().iter().map(|c| (c["value"].as_str().unwrap().to_string(), c["label"].as_str().unwrap().to_string())).collect()
        };
        let pair = |a: &str, la: &str, b: &str, lb: &str| vec![(a.to_string(), la.to_string()), (b.to_string(), lb.to_string())];
        assert_eq!(read("always"), pair("always", "Always", "always", "Always"));
        assert_eq!(read("safe"), pair("awake", "When awake", "awake", "When awake"));
        assert_eq!(read("contacts"), pair("work-admin", "In work and admin hours", "awake", "When awake"));
        assert_eq!(read("restricted"), pair("work", "In work hours", "work", "In work hours"));
        assert_eq!(read("others"), pair("work-admin", "In work and admin hours", "work-admin", "In work and admin hours"));
        let groups = own_groups(&usual, &Config::default());
        let value = |id: &str| groups.as_array().unwrap().iter().find(|g| g["id"] == id).unwrap()["value"].as_str().unwrap().to_string();
        assert_eq!([value("asked"), value("reminders"), value("care"), value("sites")], ["sleep", "awake", "on", "gathered"]);
        assert!(table_rows(true).iter().all(|r| !matches!(r, Row::People(Channel::Calls, _))));
        assert!(table_rows(false).contains(&Row::Own(Kind::Sites)) && table_rows(true).contains(&Row::Own(Kind::AppAutomatons)));
        // A row changed in the grid: "Your own", in words.
        let mut own = Attention::usual();
        own.set(Row::People(Channel::Mail, Person::Safe), Column::Meals, Level::Later).unwrap();
        let page = people(&own, Reach::default());
        let safe = page["rows"].as_array().unwrap().iter().find(|r| r["id"] == "safe").unwrap();
        assert_eq!((safe["cells"][0]["value"].as_str(), safe["cells"][0]["label"].as_str()), (Some("own"), Some("Your own")));
        speak("fr");
        let page = people(&usual, Reach::default());
        let contacts = page["rows"].as_array().unwrap().iter().find(|r| r["id"] == "contacts").unwrap();
        assert_eq!(contacts["label"], "Vos contacts");
        assert_eq!(contacts["cells"][1]["label"], "Quand vous ne dormez pas");
    }

    /// The page's choices name their rows and cells, and refuse what they do not offer.
    #[test]
    fn a_choice_names_its_rows() {
        let rows = |what: &str, value: &str| writes_of(what, value).map(|w| w.iter().map(|(r, _)| r.id()).collect::<Vec<_>>());
        assert_eq!(rows("people.messages.contacts", "never"), Ok(vec!["messages.neutral".to_string(), "messages.groups".to_string()]));
        assert_eq!(rows("people.mail.others", "never"), Ok(vec!["mail.stranger".to_string()]));
        assert_eq!(rows("group.reminders", "off"), Ok(vec!["before".to_string(), "day-before".to_string(), "dates".to_string()]));
        assert_eq!(rows("care.move", "off"), Ok(vec!["move".to_string()]));
        assert!(writes_of("people.messages.contacts", "gathered").is_err(), "no gathered times for people");
        assert!(writes_of("people.messages.always", "never").is_err(), "Always through is never held");
        assert!(writes_of("people.calls.safe", "always").is_err(), "calls are set per phone");
        assert!(writes_of("group.care", "off").is_err(), "care is switch by switch");
        assert!(writes_of("care.doses", "off").is_err(), "doses are no care switch");
    }

    /// On a computer, a phone in the sharing holds messages unless its entry
    /// says it does not give Sioul notification access (an older Sioul's,
    /// which says nothing, holds them); when every phone says no, the
    /// messages' rows say why they do not apply, and the moment leaves messages out.
    #[test]
    fn a_phone_holds_messages_as_far_as_the_sharing_knows() {
        use sioul_sync::devices::{COMPUTER, Entry, PHONE};
        let entry = |kind: &str, notifications: Option<bool>, left: bool| Entry { id: "x".into(), kind: kind.into(), notifications, left, working: true, ..Entry::default() };
        assert_eq!(Phones::of(&[]), Phones::None);
        assert_eq!(Phones::of(&[entry(COMPUTER, None, false)]), Phones::None);
        assert_eq!(Phones::of(&[entry(PHONE, Some(true), false)]), Phones::Holding);
        assert_eq!(Phones::of(&[entry(PHONE, None, false)]), Phones::Holding, "an older Sioul says nothing: holding, as before phones said it");
        assert_eq!(Phones::of(&[entry(PHONE, Some(false), false)]), Phones::WithoutAccess, "it said no");
        assert_eq!(Phones::of(&[entry(PHONE, Some(false), false), entry(PHONE, Some(true), false)]), Phones::Holding);
        assert_eq!(Phones::of(&[entry(PHONE, Some(false), false), entry(PHONE, None, false)]), Phones::Holding);
        assert_eq!(Phones::of(&[entry(PHONE, Some(false), false), entry(PHONE, None, true)]), Phones::WithoutAccess, "one that left counts no more");
        assert_eq!(Phones::of(&[entry(PHONE, Some(true), true)]), Phones::None);
        speak("en");
        let computer = |phones: Phones| Reach { messages: phones == Phones::Holding, phones, ..Reach::default() };
        assert_eq!(channel_note(Channel::Messages, computer(Phones::None)), text("attention-channel-messages-none"));
        assert_eq!(channel_note(Channel::Messages, computer(Phones::WithoutAccess)), "None of your phones gives Sioul notification access, as each last said: these rows apply once one does (Settings ▸ This phone, on the phone).");
        assert_eq!(channel_note(Channel::Messages, computer(Phones::Holding)), text("attention-channel-messages-phone"));
        assert_eq!(channel_note(Channel::Mail, computer(Phones::WithoutAccess)), "");
        // The moment and the channels said: messages only from a phone that holds them.
        assert_eq!(channels(computer(Phones::WithoutAccess)), [Channel::Mail]);
        assert_eq!(channels(computer(Phones::Holding)), [Channel::Mail, Channel::Messages]);
    }
}

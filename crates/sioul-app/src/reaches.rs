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
use jiff::Zoned;
use serde_json::{Value, json};
use sioul_core::attention::{self, Attention, Column, Kind, Level, Now, Person, Phone, Preset, Row, Senders};
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
    /// This phone screens calls: what its own do-not-disturb lets ring follows.
    screens: bool,
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
    Reach { phone, calls, messages: phone || phones == Phones::Holding, screens, phones }
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
/// one does not apply yet), and the kinds that act on this device.
fn shown_rows(phone: bool) -> Vec<Row> {
    let mut rows: Vec<Row> = Channel::ALL.iter().flat_map(|c| attention::channel_rows(*c)).collect();
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

/// Who, in words: "your safe and neutral senders and strangers"; everyone;
/// everyone else (`rest`: the others were said before in the same card).
fn who_words(persons: &[Person], everyone: bool, rest: bool) -> String {
    if everyone {
        return text(if rest { "attention-who-everyone-else" } else { "attention-who-everyone" });
    }
    let known: Vec<String> = persons.iter().filter(|p| matches!(p, Person::Safe | Person::Neutral | Person::Restricted)).map(|p| text(&format!("attention-who-kind-{}", p.id()))).collect();
    let mut items = Vec::new();
    if !known.is_empty() {
        let mut args = sioul_core::i18n::args();
        args.set("kinds", listed(&known, false));
        items.push(tr().text("attention-who-senders", Some(&args)));
    }
    for person in persons.iter().filter(|p| matches!(p, Person::Stranger | Person::Hidden | Person::Groups | Person::Always)) {
        items.push(text(&format!("attention-who-{}", person.id())));
    }
    listed(&items, false)
}

/// "mail, calls and messages", in the language's own words (French: "le courrier…").
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
        let all: Vec<Person> = attention::persons(*channel).iter().copied().filter(|p| p.state()).collect();
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
    let plural = parts.len() > 1 || worded.iter().any(|(channels, _)| channels.len() > 1 || channels[0] != Channel::Mail);
    Some((listed(&parts, false), plural))
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

// ---------------------------------------------------------------- a card per time

/// The times a row comes in (not held): its own, for "waits until".
fn comes_in(attention: &Attention, row: Row) -> Vec<Column> {
    Column::TIMES.iter().copied().filter(|c| matches!(attention.cell(row, *c), Level::Now | Level::Quiet | Level::Gathered | Level::Event)).collect()
}

/// Until when something held at `column` waits, in words: "until you wake",
/// "for a work or admin time".
fn until_words(column: Column, comes: &[Column]) -> String {
    let waking = [Column::Work, Column::Admin, Column::Leisure, Column::Meals].iter().all(|c| comes.contains(c));
    match column {
        Column::Sleep if waking => return text("attention-until-wake"),
        Column::Pause if waking => return text("attention-until-back"),
        Column::Free if waking => return text("attention-until-free"),
        Column::Meals if [Column::Work, Column::Admin, Column::Leisure].iter().all(|c| comes.contains(c)) => return text("attention-until-meal"),
        Column::Slot => return text("attention-until-slot"),
        Column::Dnd => return text("attention-until-dnd"),
        _ => {}
    }
    let times: Vec<String> = comes.iter().filter(|c| matches!(c, Column::Work | Column::Admin | Column::Leisure | Column::Meals)).map(|c| text(&format!("attention-until-{}", c.id()))).collect();
    if times.is_empty() {
        return text("attention-until-none");
    }
    let mut args = sioul_core::i18n::args();
    args.set("times", listed(&times, true));
    tr().text("attention-until-times", Some(&args))
}

/// What one time, or one layer, lets through, in sentences: at once, at the
/// gathered times, when its event falls then, shown without a notification,
/// waiting until…, not at all; Always through first when it says more than
/// their own lists.
fn card_lines(attention: &Attention, config: &Config, column: Column, reach: Reach) -> Vec<String> {
    let layer = !column.is_time();
    let mut lines = Vec::new();
    let mut said: Vec<(Channel, Person)> = Vec::new();
    let chans = channels(reach);
    // Always through, as their own row says unless it says more.
    let mut always: Vec<(Level, Vec<Channel>)> = Vec::new();
    for channel in &chans {
        let level = attention.cell(Row::People(*channel, Person::Always), column);
        // Said only when it gives them more than every list does then.
        let lists = attention::persons(*channel).iter().filter(|p| p.state()).map(|p| attention.cell(Row::People(*channel, *p), column)).max().unwrap_or(Level::Now);
        if level == Level::As || (level != Level::Through && level >= lists) {
            continue;
        }
        match always.iter_mut().find(|(l, _)| *l == level) {
            Some((_, channels)) => channels.push(*channel),
            None => always.push((level, vec![*channel])),
        }
    }
    for (level, channels) in &always {
        let what = from_words(channels, &text("attention-who-always"));
        let plural = channels.len() > 1 || channels[0] != Channel::Mail;
        let line = match level {
            Level::Through => text_with("attention-card-always-through", "", true, &[]),
            Level::Now => text_with("attention-card-always-now", &what, plural, &[]),
            Level::Quiet if layer => text_with("attention-card-layer-quiet", &what, plural, &[]),
            Level::Quiet => text_with("attention-card-quiet", &what, plural, &[]),
            _ => continue,
        };
        lines.push(line);
    }
    // Everyone else's rows, and Sioul's own, grouped by what they do here.
    type Key = (u8, Vec<Column>);
    let mut groups: Vec<(Key, Vec<Subject>)> = Vec::new();
    let mut add = |key: Key, subject: Subject| match groups.iter_mut().find(|(k, _)| *k == key) {
        Some((_, subjects)) => subjects.push(subject),
        None => groups.push((key, vec![subject])),
    };
    // The order said: at once, the event's, gathered, shown, waiting (calls to voicemail last), not at all.
    let rank = |level: Level, calls: bool| -> u8 {
        match level {
            Level::Now => 0,
            Level::Event => 1,
            Level::Gathered => 2,
            Level::Quiet => 3,
            Level::Later if calls => 5,
            Level::Later => 4,
            _ => 6,
        }
    };
    for channel in &chans {
        for person in attention::persons(*channel).iter().filter(|p| p.state()) {
            let row = Row::People(*channel, *person);
            let level = attention.cell(row, column);
            let calls = *channel == Channel::Calls;
            let until = if level == Level::Later && !calls { comes_in(attention, row) } else { Vec::new() };
            add((rank(level, calls), until), Subject::People(*channel, *person));
        }
    }
    for kind in kinds(reach.phone) {
        let row = Row::Own(kind);
        let level = attention.cell(row, column);
        let until = if level == Level::Later { comes_in(attention, row) } else { Vec::new() };
        // Codes not told stay on the Porch: a sentence of their own.
        let key = if kind == Kind::Codes && level == Level::Never { 7 } else { rank(level, false) };
        add((key, until), Subject::Own(kind));
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    for ((rank, until), subjects) in groups {
        // Mail not at all stays in its lane: said apart from the kinds not at all.
        let (mail_never, rest): (Vec<Subject>, Vec<Subject>) = subjects.into_iter().partition(|s| rank == 6 && matches!(s, Subject::People(..)));
        for (what, plural) in subjects_words(&mail_never, &mut said) {
            lines.push(text_with("attention-card-never-mail", &what, plural, &[]));
        }
        for (what, plural) in subjects_words(&rest, &mut said) {
            lines.push(match rank {
                0 if layer => text_with("attention-card-as-usual", &what, plural, &[]),
                0 => text_with("attention-card-now", &what, plural, &[]),
                1 => text_with("attention-card-event", &what, plural, &[]),
                2 => text_with("attention-card-gathered", &what, plural, &[("times", gathered_times(config))]),
                3 if layer => text_with("attention-card-layer-quiet", &what, plural, &[]),
                3 => text_with("attention-card-quiet", &what, plural, &[]),
                4 => text_with("attention-card-wait", &what, plural, &[("until", until_words(column, &until))]),
                5 => text_with("attention-card-voicemail", &what, plural, &[]),
                7 => text_with("attention-card-never-codes", &what, plural, &[]),
                _ => text_with("attention-card-never", &what, plural, &[]),
            });
        }
    }
    lines
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

/// What the device's own do-not-disturb does then: not silenced; or, on a
/// phone, whom Sioul's mode lets through.
fn system_line(attention: &Attention, config: &Config, column: Column, reach: Reach) -> String {
    if !silenced(config, column) {
        return text(if reach.phone { "attention-system-calm-phone" } else { "attention-system-calm-computer" });
    }
    if !reach.phone {
        return text("attention-system-computer");
    }
    let silence = attention.silence(&[column], false, Phone { screens: reach.screens });
    let senders = |senders: Senders| match senders {
        Senders::None => None,
        Senders::Starred => Some(text("attention-senders-starred")),
        Senders::Contacts => Some(text("attention-senders-contacts")),
        Senders::Anyone => Some(text("attention-senders-anyone")),
    };
    let mut items = Vec::new();
    match (senders(silence.calls), senders(silence.messages)) {
        (Some(calls), Some(messages)) if calls == messages => items.push(from_words(&[Channel::Calls, Channel::Messages], &calls)),
        (calls, messages) => {
            items.extend(calls.map(|who| from_words(&[Channel::Calls], &who)));
            items.extend(messages.map(|who| from_words(&[Channel::Messages], &who)));
        }
    }
    if silence.repeat {
        items.push(text("attention-system-repeat"));
    }
    if silence.conversations {
        items.push(text("attention-system-conversations"));
    }
    items.push(text("attention-system-alarms"));
    if silence.events {
        items.push(text("attention-system-events"));
    }
    if silence.doses {
        items.push(text("attention-system-doses"));
    }
    let mut args = sioul_core::i18n::args();
    args.set("what", listed(&items, false));
    tr().text("attention-system-phone", Some(&args))
}

/// A set of hours in words, days with the same hours together: "Monday to
/// Friday, 09:00 to 17:00"; none when none is set.
fn hours_words(config: &Config, kind: &str) -> Option<String> {
    let windows: Vec<&sioul_core::window::AdminWindow> = config.windows.iter().filter(|w| w.kind() == kind).collect();
    let mut days: Vec<Vec<(String, String)>> = vec![Vec::new(); 7];
    for w in &windows {
        if let Some(day) = w.weekday() {
            let range = (w.start.trim().to_string(), w.end_text());
            let slot = &mut days[day.to_monday_zero_offset() as usize];
            if !slot.contains(&range) {
                slot.push(range);
            }
        }
    }
    for slot in &mut days {
        slot.sort();
    }
    let day = |i: usize| text(&format!("weekday-{}", i + 1));
    let mut parts = Vec::new();
    let mut at = 0;
    while at < 7 {
        if days[at].is_empty() {
            at += 1;
            continue;
        }
        let mut last = at;
        while last + 1 < 7 && days[last + 1] == days[at] {
            last += 1;
        }
        let when = match last - at {
            0 => day(at),
            1 => listed(&[day(at), day(last)], false),
            _ => {
                let mut args = sioul_core::i18n::args();
                args.set("from", day(at));
                args.set("to", day(last));
                tr().text("attention-when-days", Some(&args))
            }
        };
        let ranges: Vec<String> = days[at]
            .iter()
            .map(|(from, to)| {
                let mut args = sioul_core::i18n::args();
                args.set("from", from.clone());
                args.set("to", to.clone());
                tr().text("attention-when-range", Some(&args))
            })
            .collect();
        parts.push(format!("{when}, {}", listed(&ranges, false)));
        at = last + 1;
    }
    // Days with other hours after a semicolon, as each language writes it.
    let joined = parts.into_iter().reduce(|one, other| say("attention-when-list", &[("one", one), ("other", other)]))?;
    Some(capital(&joined))
}

/// When a time holds, in words.
fn when_words(config: &Config, column: Column) -> String {
    match column {
        // A sentence, its full stop said by each language.
        Column::Work => hours_words(config, "work").map(|hours| say("attention-when-hours", &[("hours", hours)])).unwrap_or_else(|| text("attention-when-work-none")),
        Column::Admin => hours_words(config, "admin").map(|hours| say("attention-when-hours", &[("hours", hours)])).unwrap_or_else(|| text("attention-when-admin-none")),
        Column::Dnd if config.dnd.focus => text("attention-when-dnd-focus"),
        other => text(&format!("attention-when-{}", other.id())),
    }
}

/// The named holds outside the matrix, said in the time they act in (Q25).
/// Free time's, Nothing at all, is its card's switch, its help under it.
fn also_lines(column: Column) -> Vec<String> {
    let ids: &[&str] = match column {
        Column::Work => &["attention-also-meeting"],
        Column::Admin => &["attention-also-areas", "attention-also-meeting"],
        Column::Leisure | Column::Meals => &["attention-also-areas"],
        Column::Pause => &["attention-also-porch-rests"],
        _ => &[],
    };
    ids.iter().map(|id| text(id)).collect()
}

/// The time now as a column: the pause, sleep, Free time, then the hours; work when both open.
fn column_now(now: &Now) -> Column {
    now.times.first().copied().unwrap_or(Column::Work)
}

// ---------------------------------------------------------------- the tab

/// The tab, as JSON (ReachesTab.qml): {now: sentences, column: the time
/// now, presets [{id, label, current, changes}], changed, cards [{id,
/// label, layer, now, when, lines, system, also}], switches \[lines\],
/// channels [{id, label, note}], rows [ids shown here], grid (the
/// matrix's, `attention::grid`), mail: {spam, areas}, gathered, exceptions:
/// {always \[lines\], sites \[lines\], events, health}, phone}.
pub(crate) fn view(realtime: bool) -> String {
    let config = load_config();
    let attention = crate::hours::attention();
    let reach = reach();
    let now = crate::hours::attention_now();
    let current = column_now(&now);
    let usual = Attention::usual();
    let presets: Vec<Value> = Preset::ALL
        .iter()
        .map(|p| {
            let changes = attention.changes_from(&p.matrix());
            json!({ "id": p.id(), "label": text(&format!("attention-preset-{}", p.id())), "current": changes == 0, "changes": changes })
        })
        .collect();
    let from_usual = attention.changes_from(&usual);
    let changed = if from_usual == 0 { String::new() } else { tr().text("attention-changes", Some(&tr().counted(from_usual))) };
    // The time now first, then the others in their order, then the layers.
    let mut order: Vec<Column> = vec![current];
    order.extend(Column::TIMES.iter().copied().filter(|c| *c != current));
    order.extend([Column::Slot, Column::Dnd].iter().filter(|c| **c != current));
    let cards: Vec<Value> = order
        .iter()
        .map(|column| {
            json!({
                "id": column.id(),
                "label": attention::column_label(tr(), *column),
                "layer": !column.is_time(),
                "now": *column == current || (column == &Column::Dnd && now.dnd) || (column == &Column::Slot && now.slot),
                "when": when_words(&config, *column),
                "lines": card_lines(&attention, &config, *column, reach),
                "system": system_line(&attention, &config, *column, reach),
                "also": also_lines(*column),
            })
        })
        .collect();
    let mut switches = Vec::new();
    if reach.calls {
        let moment = crate::calls::moment();
        let through = moment["through"] == true;
        let line = moment["line"].as_str().unwrap_or_default().to_string();
        switches.push(if through && !line.is_empty() { line } else { text("attention-switch-through-off") });
    }
    switches.push(text(if realtime { "attention-switch-realtime-on" } else { "attention-switch-realtime-off" }));
    let channels: Vec<Value> = Channel::ALL.iter().map(|c| json!({ "id": c.id(), "label": text(&format!("attention-group-{}", c.id())), "note": channel_note(*c, reach) })).collect();
    let rows = shown_rows(reach.phone);
    let grid = attention::grid(&attention, tr(), &rows, Preset::Usual);
    json!({
        "now": now_sentences(&attention, &now, reach),
        "column": current.id(),
        "presets": presets,
        "changed": changed,
        "cards": cards,
        "switches": switches,
        "channels": channels,
        "rows": rows.iter().map(|r| r.id()).collect::<Vec<_>>(),
        "grid": grid,
        "gathered": say("attention-gathered-at", &[("times", gathered_times(&config))]),
        "mail": { "spam": spam_line(&config), "areas": text("attention-mail-areas") },
        "exceptions": exceptions(&attention, &config, reach),
        "phone": reach.phone,
        // Android lets Sioul set exact alarms (This phone says when it does not).
        "exact": !reach.phone || crate::alarms::exact(),
    })
    .to_string()
}

/// What Sioul's own spam filter does with strangers' mail, in one sentence
/// (its table: Mail ⚙ ▸ Your own spam filter).
fn spam_line(config: &Config) -> String {
    use sioul_core::spam::{Action, Class};
    let actions = config.spam.actions();
    let held = [Class::Spam, Class::Unsure].iter().any(|c| actions.of(*c) != Action::Nothing);
    text(if held { "attention-spam-held" } else { "attention-spam-told" })
}

/// The moment now, in sentences: the time and until when, the layers on top
/// of it, then what comes at once, what is shown without a word, what waits
/// (`Attention::person` and `level`: what the matrix decides now).
fn now_sentences(attention: &Attention, now: &Now, reach: Reach) -> Vec<String> {
    let mode = crate::hours::mode_now();
    let column = column_now(now);
    let time = small(&attention::column_label(tr(), column));
    let at = Zoned::now();
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
        for person in attention::persons(channel).iter().filter(|p| p.state()) {
            match attention.person(channel, *person, false, now) {
                Level::Now => add(0, Subject::People(channel, *person)),
                Level::Quiet => add(2, Subject::People(channel, *person)),
                Level::Never => {}
                _ => add(3, Subject::People(channel, *person)),
            }
        }
    }
    // Of Sioul's own, what matters most: codes, doses, reminders.
    for kind in [Kind::Codes, Kind::Doses, Kind::Alarms, Kind::Before, Kind::DayBefore, Kind::Dates] {
        match attention.level(Row::Own(kind), now) {
            Level::Now | Level::Event => add(0, Subject::Own(kind)),
            Level::Gathered => add(1, Subject::Own(kind)),
            Level::Never => {}
            _ => add(3, Subject::Own(kind)),
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

/// The exceptions in words: what Always through does on each channel, your
/// sites' own choices (a computer), where events and Health's days say theirs.
fn exceptions(attention: &Attention, config: &Config, reach: Reach) -> Value {
    let always: Vec<String> = channels(reach).iter().filter_map(|c| channel_line(attention, *c, Person::Always, true, "attention-always-line")).collect();
    let sites: Vec<String> = config
        .accounts
        .iter()
        .filter_map(sioul_core::sites::Site::of)
        .map(|site| {
            let how = if site.muted {
                text("attention-site-muted")
            } else if site.realtime {
                text("attention-site-live")
            } else {
                text("attention-site-gathered")
            };
            say("attention-site-line", &[("name", site.name.clone()), ("how", how)])
        })
        .collect();
    json!({ "always": always, "sites": if reach.phone { Vec::new() } else { sites } })
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
        Level::Quiet if channel != Channel::Mail => Some(Level::Later),
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
            let line = channel_line(&attention, *channel, Person::of(who), always, "attention-sheet-line")?;
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
        _ => {}
    }
    let line: Vec<String> = said.into_iter().filter(|s| !s.is_empty()).collect();
    json!({ "sheet": serde_json::from_str::<Value>(&person_with(&Someone::of(key, addresses), line.join(" "))).unwrap_or(Value::Null), "line": line.join(" ") }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn every_card_and_line_is_said_whole_in_both_languages() {
        let mut config = Config::default();
        config.dnd.sleep = true;
        config.dnd.focus = true;
        config.windows = sioul_core::window::parse_ranges("mo-fr 09:00-12:00, 14:00-17:00; sa 10:00-12:00");
        for language in ["en", "fr"] {
            speak(language);
            for attention in [Attention::usual(), Preset::Quieter.matrix(), Preset::Reachable.matrix(), changed_everywhere()] {
                for phone in [false, true] {
                    for (calls, messages) in [(false, false), (true, true)] {
                        let reach = Reach { phone, calls, messages, screens: phone && calls, ..Reach::default() };
                        for column in Column::ALL {
                            let mut lines = card_lines(&attention, &config, column, reach);
                            assert!(!lines.is_empty(), "{language} {column:?}: an empty card");
                            lines.push(system_line(&attention, &config, column, reach));
                            lines.push(when_words(&config, column));
                            lines.extend(also_lines(column));
                            for line in &lines {
                                said_well(line, language);
                            }
                        }
                    }
                }
                for channel in Channel::ALL {
                    for person in attention::persons(channel).iter().filter(|p| **p != Person::Always) {
                        for always in [false, true] {
                            said_well(&channel_line(&attention, channel, *person, always, "attention-sheet-line").unwrap(), language);
                        }
                    }
                    said_well(&channel_line(&attention, channel, Person::Always, true, "attention-always-line").unwrap(), language);
                }
            }
            said_well(&text("attention-mail-areas"), language);
            for key in ["attention-spam-held", "attention-spam-told", "attention-switch-through-off", "attention-switch-realtime-on", "attention-switch-realtime-off", "attention-channel-calls-none", "attention-channel-messages-none", "attention-channel-messages-no-access", "attention-channel-messages-phone"] {
                said_well(&text(key), language);
            }
        }
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

    #[test]
    fn the_usual_cards_say_what_sioul_does() {
        let config = Config::default();
        let usual = Attention::usual();
        let computer = Reach::default();
        speak("en");
        let has = |lines: &[String], line: &str| assert!(lines.iter().any(|l| l == line), "{line:?} not in {lines:#?}");
        let leisure = card_lines(&usual, &config, Column::Leisure, computer);
        has(&leisure, "Mail from your Always through people comes at once, whatever their list.");
        // Where every list comes at once already, Always through gives nothing more: not said.
        assert!(!card_lines(&usual, &config, Column::Work, computer).iter().any(|l| l.contains("Always through")));
        has(&leisure, "Mail from your safe senders comes at once.");
        has(&leisure, "Mail from your restricted senders waits for a work time.");
        has(&leisure, "Mail from everyone else waits for a work or admin time.");
        has(&leisure, "Your sites' notifications come at the gathered times: 09:00, 13:00 and 18:00.");
        let sleep = card_lines(&usual, &config, Column::Sleep, computer);
        has(&sleep, "Mail from your Always through people is shown in Sioul, without a notification.");
        has(&sleep, "Mail from your safe senders is shown in Sioul, without a notification.");
        has(&sleep, "The codes and links you ask for and your doses come at once.");
        let work = card_lines(&usual, &config, Column::Work, computer);
        has(&work, "Mail from everyone comes at once.");
        has(&work, "“Work hours are over” does not come at this time.");
        // Under do-not-disturb: at most shown; the layers never let more through.
        let dnd = card_lines(&usual, &config, Column::Dnd, computer);
        has(&dnd, "Mail from everyone is shown without a notification, when its time lets it come.");
        // A phone screening calls: the calls' rows, voicemail, and the system's line.
        let phone = Reach { phone: true, calls: true, messages: true, screens: true, ..Reach::default() };
        let leisure = card_lines(&usual, &config, Column::Leisure, phone);
        has(&leisure, "Mail, calls and messages from your safe senders come at once.");
        assert!(leisure.iter().any(|l| l.starts_with("Calls from") && l.contains("voicemail")), "{leisure:#?}");
        assert_eq!(system_line(&usual, &config, Column::Leisure, phone), "The phone is not silenced.");
        let pause = system_line(&usual, &config, Column::Pause, phone);
        assert!(pause.starts_with("The phone is silenced; it lets through calls") && pause.contains("from your contacts") && pause.contains("alarms"), "{pause}");
        // A person's lines: a neutral contact, and the same on Always through.
        assert_eq!(
            channel_line(&usual, Channel::Mail, Person::Neutral, false, "attention-sheet-line").unwrap(),
            "Their mail comes at once during work and admin. It waits during leisure, meals, sleep, a pause and free time. During time for you and under do-not-disturb, their mail is at most shown, without a notification."
        );
        assert!(channel_line(&usual, Channel::Calls, Person::Neutral, true, "attention-sheet-line").unwrap().starts_with("Their calls ring at any time."));
        speak("fr");
        has(&card_lines(&usual, &config, Column::Work, computer), "Le courrier de tout le monde arrive tout de suite.");
        has(&card_lines(&usual, &config, Column::Leisure, computer), "Le courrier de vos expéditeurs sûrs arrive tout de suite.");
    }

    #[test]
    fn hours_in_words() {
        let mut config = Config::default();
        config.windows = sioul_core::window::parse_ranges("mo-fr 09:00-17:00; sa 10:00-12:00");
        speak("en");
        assert_eq!(hours_words(&config, "work").as_deref(), Some("Monday to Friday, 09:00 to 17:00; Saturday, 10:00 to 12:00"));
        assert_eq!(when_words(&config, Column::Admin), text("attention-when-admin-none"));
        speak("fr");
        assert_eq!(hours_words(&config, "work").as_deref(), Some("Du lundi au vendredi, de 09:00 à 17:00\u{202f}; samedi, de 10:00 à 12:00"));
        assert_eq!(capital("le courrier"), "Le courrier");
        assert_eq!(small("Temps libre"), "temps libre");
    }
}

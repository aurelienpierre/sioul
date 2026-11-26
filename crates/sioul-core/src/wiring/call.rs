// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Calls in the rules (docs/rules.md, "Calls"): a call as a rule's
//! conditions read it (`Seen`, `holds`), the events a call that ended is
//! (`events`), and what the rules about calls do (`run`): the presets the
//! engine runs, "missed-call-task" and "call-closes-task", as your settings
//! say them (`[presets]`, `preset_settings`), then your own rules about
//! calls, in their order.
//!
//! They run where a call is first read: on the phone that had it, once, as
//! it copies the call from its history (`calls::carry_history`), so that two
//! devices never act on the same call; what they write is stamped with the
//! call's own time (`callback`). A call that came while a rule was off is
//! never acted on later; your own rule acts on the calls from the moment it
//! was made or turned on (`Rule::since`). `rings` is the moment the phone's
//! screening decides, in Java, within milliseconds: no rule of the engine
//! follows it (`declare::follows`).
//!
//! Each act is written in the record of acts, as every rule's (`Ran::acts`,
//! the object the call's address): "task" for a task made (its Undo takes
//! it out while it is as the rule left it), "done" for a task closed (its
//! Undo opens it again, and no call closes it again), "joined" for a call's
//! time added (no Undo of its own). A rule never acts twice on one call
//! (`Acts::touched`). Nothing is said in the status line: what closes by
//! itself is not counted (docs/tasks.md).
//!
//! A device before this one (0.1.0, and the builds of November 2026 before
//! calls were objects) runs the same two presets as its call-back rules, by
//! the switches Porch ⚙ ▸ Calls had (`[porch] call_back`, `call_again`):
//! "missed-call-task" writes them too (`config::set_preset`), so that an
//! older phone makes the tasks the presets say; a change an older device
//! made to them is carried back (`missed_call_preset`). An older phone
//! closes a task once you spoke, whatever "call-closes-task" says.

use crate::calls::{Call, Direction};
use crate::callback::{Book, Did, Kept, Write};
use crate::config::{Config, MISSED_CALL_TASK, PresetSettings};
use crate::i18n::Translator;
use crate::phones::Region;
use crate::wiring::acts::{Act, Acts};
use crate::wiring::declare;
use crate::wiring::store::{Condition, Join, Rule, Rules, When};
use jiff::tz::TimeZone;

/// The preset that closes a call's task once you spoke with them.
pub const CALL_CLOSES_TASK: &str = "call-closes-task";

/// A call as a rule's conditions read it.
#[derive(Debug, Clone)]
pub struct Seen {
    /// What its phone's history says, and their card when your address books have the number.
    pub call: Call,
    /// Their circle for you (`reach::Who`): "safe", "neutral", "restricted",
    /// "stranger", "blocked"; "" for a hidden number.
    pub circle: &'static str,
    /// The phone's name as you named it, else its system's; "" unknown.
    pub phone_name: String,
}

/// A circle's id, as rules name it (`declare::CIRCLES`).
pub fn circle_of(who: crate::reach::Who) -> &'static str {
    use crate::reach::Who;
    match who {
        Who::Safe => "safe",
        Who::Neutral => "neutral",
        Who::Restricted => "restricted",
        Who::Stranger => "stranger",
        Who::Blocked => "blocked",
    }
}

/// The events a call that ended is: it `ends`; it `is-missed` (theirs that
/// nobody answered, or yours that did not reach them: `Call::missed`,
/// `Call::not_reached`) or it `is-answered` (you spoke: `Call::spoke`).
/// Never `rings`, the screening's moment.
pub fn events(call: &Call, threshold: u32) -> Vec<&'static str> {
    let mut out = vec!["ends"];
    if call.missed() || call.not_reached(threshold) {
        out.push("is-missed");
    }
    if call.spoke(threshold) {
        out.push("is-answered");
    }
    out
}

/// Whether a test turns its condition around: then it holds when none of its values does.
fn turned(test: &str) -> bool {
    matches!(test, "is-not" | "missing" | "not-contains")
}

/// Whether a call's "when" holds: every condition, or one, as it says.
pub fn holds(when: &When, seen: &Seen, region: Option<&Region>, zone: &TimeZone) -> bool {
    let each = |c: &Condition| holds_one(c, seen, region, zone);
    match when.join {
        Join::All => !when.conditions.is_empty() && when.conditions.iter().all(each),
        Join::Any => when.conditions.iter().any(each),
    }
}

/// Folded: case and accents aside.
fn folded(text: &str) -> String {
    crate::text::fold(text.trim()).into_iter().collect()
}

fn holds_one(c: &Condition, seen: &Seen, region: Option<&Region>, zone: &TimeZone) -> bool {
    let call = &seen.call;
    let values: Vec<&str> = c.value.iter().map(|v| v.trim()).filter(|v| !v.is_empty()).collect();
    let among = |one: &dyn Fn(&str) -> bool| if turned(&c.test) { !values.iter().any(|v| one(v)) } else { values.iter().any(|v| one(v)) };
    let at = jiff::Timestamp::from_millisecond(call.at).unwrap_or(jiff::Timestamp::UNIX_EPOCH).to_zoned(zone.clone());
    let card = call.contact.as_ref().filter(|c| !c.uri.trim().is_empty());
    match (c.field.as_str(), c.test.as_str()) {
        ("card", "exists") => card.is_some(),
        ("card", "missing") => card.is_none(),
        ("card", _) => card.is_some_and(|card| values.iter().any(|v| crate::links::id_of(&card.uri) == *v)),
        // A hidden number is none of the numbers named: "is not" holds, "is" never.
        ("number", _) => if call.hidden { turned(&c.test) } else { among(&|v| crate::phones::key(v, region) == call.number) },
        ("circle", _) => among(&|v| if v == "known" { card.is_some() } else { !seen.circle.is_empty() && seen.circle == v }),
        ("way", _) => among(&|v| (v == "out") == (call.direction == Direction::Outgoing)),
        ("answered", _) => among(&|v| (v == "yes") == call.answered),
        ("length", test) => values.first().and_then(|v| v.parse::<i64>().ok()).is_some_and(|n| if test == "above" { call.seconds > n } else { call.seconds < n }),
        ("date", test) => {
            let day = at.date();
            let first = values.first().and_then(|v| crate::rules::day(v));
            match test {
                "after" => first.is_some_and(|d| day > d),
                "before" => first.is_some_and(|d| day < d),
                _ => first.zip(crate::rules::day(c.until.trim())).is_some_and(|(from, until)| from <= day && day <= until),
            }
        }
        ("weekday", _) => {
            let index = usize::try_from(at.weekday().to_monday_zero_offset()).unwrap_or(0);
            let today = crate::rules::DAYS[index % 7];
            let days: Vec<String> = values.iter().flat_map(|v| v.split(',')).map(|d| d.trim().to_ascii_lowercase()).collect();
            if turned(&c.test) { !days.iter().any(|d| d == today) } else { days.iter().any(|d| d == today) }
        }
        ("hour", test) => {
            let time = at.time();
            let first = values.first().and_then(|v| crate::rules::time(v));
            match test {
                "after" => first.is_some_and(|t| time >= t),
                "before" => first.is_some_and(|t| time < t),
                _ => first.zip(crate::rules::time(c.until.trim())).is_some_and(|(from, until)| if from <= until { from <= time && time < until } else { time >= from || time < until }),
            }
        }
        ("phone", _) => among(&|v| folded(v) == folded(&call.phone) || (!seen.phone_name.is_empty() && folded(v) == folded(&seen.phone_name))),
        _ => false,
    }
}

/// "missed-call-task" as your settings say it: `[presets.missed-call-task]`;
/// before it was written (`config::migrate_presets`), the old switches of
/// Porch ⚙ ▸ Calls; and when those no longer say what the preset wrote for
/// older devices (`config::switches_of_preset`), a device before format 3
/// changed them since: what it said is carried, the ways kept when it
/// turned both off.
pub fn missed_call_preset(config: &Config) -> PresetSettings {
    let old = (config.porch.call_back, config.porch.call_again);
    match config.presets.get(MISSED_CALL_TASK) {
        None => crate::config::preset_of_switches(old.0, old.1, None),
        Some(set) if crate::config::switches_of_preset(set) != old => crate::config::preset_of_switches(old.0, old.1, set.ways.as_deref()),
        Some(set) => set.clone(),
    }
}

/// A preset of the engine as your settings say it: on unless said.
pub fn preset_settings(config: &Config, id: &str) -> PresetSettings {
    if id == MISSED_CALL_TASK {
        return missed_call_preset(config);
    }
    config.presets.get(id).cloned().unwrap_or_default()
}

/// Whether a preset of the engine is on, as your settings say.
pub fn preset_on(config: &Config, id: &str) -> bool {
    let set = preset_settings(config, id);
    set.on != Some(false) && (id != MISSED_CALL_TASK || set.ways.as_ref().is_none_or(|w| !w.is_empty()))
}

/// The rule a preset of the engine runs, as your settings say it: "missed-call-task" for the ways you chose.
pub fn preset_rule(config: &Config, preset: &declare::Preset) -> Rule {
    let mut rule = preset.rule.clone();
    if preset.id == MISSED_CALL_TASK {
        let ways = preset_settings(config, MISSED_CALL_TASK).ways.unwrap_or_else(|| vec!["in".into(), "out".into()]);
        for condition in rule.when.conditions.iter_mut().filter(|c| c.field == "way") {
            condition.value = ways.clone();
        }
    }
    rule
}

/// The rules about calls this phone runs, in order: the presets of the
/// engine that are on, as your settings say them (`preset_rule`), then
/// your own rules about calls that run here (`Rule::runs`).
pub fn rules_here(config: &Config, own: &Rules) -> Vec<Rule> {
    let mut out: Vec<Rule> = declare::presets().iter().filter(|p| p.runs == "engine" && p.rule.when.kind == "call" && preset_on(config, p.id)).map(|p| preset_rule(config, p)).collect();
    out.extend(own.running().filter(|r| r.when.kind == "call").cloned());
    out
}

/// Whether `rule` acts on a call (`seen`): it follows one of the call's
/// events, its conditions hold, and the call came once it was on (a
/// preset's: whenever it was given, since a call is given once).
pub fn acts_on(rule: &Rule, seen: &Seen, threshold: u32, region: Option<&Region>, zone: &TimeZone) -> bool {
    rule.when.kind == "call"
        && events(&seen.call, threshold).contains(&rule.when.event.as_str())
        && (rule.id.starts_with("preset:") || seen.call.at >= rule.since.saturating_mul(1000))
        && holds(&rule.when, seen, region, zone)
}

/// What the rules about calls did: what to write, and the acts to record
/// once it is written (each `wrote`, depth 1, its task's digest as written).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ran {
    pub writes: Vec<Write>,
    pub acts: Vec<Act>,
}

/// What the rules about calls (`rules`, in order) make of a phone's new
/// calls (`calls`, oldest first, as their phone first reads them) and the
/// tasks made so (`kept`): each call in turn, each rule that acts on it
/// (`acts_on`) and has not acted on it on any device (`acts`): "call-back"
/// opens their task or adds the call's time to it, "mark-done" closes it
/// (`callback::Book`). What is to be written, and its acts, at `now_ms`.
#[allow(clippy::too_many_arguments)]
pub fn run(calls: &[Seen], kept: &[Kept], rules: &[Rule], acts: &Acts, threshold: u32, region: Option<&Region>, tr: &Translator, zone: &TimeZone, now_ms: i64) -> Ran {
    let mut book = Book::of(kept);
    let mut done: Vec<Act> = Vec::new();
    for seen in calls {
        let object = seen.call.address();
        for rule in rules.iter().filter(|r| acts_on(r, seen, threshold, region, zone) && !acts.touched(&r.id, &object)) {
            let act = |kind: &str, uid: &str| Act { at: now_ms, rule: rule.id.clone(), object: object.clone(), act: kind.into(), result: crate::links::task_uri(uid), wrote: true, depth: 1, same: super::engine::same(rule), ..Act::default() };
            match (rule.then.kind.as_str(), rule.then.action.as_str()) {
                ("task", "call-back") => match book.call_back(&seen.call, tr, zone) {
                    Some(Did::Made(uid)) => done.push(act("task", &uid)),
                    Some(Did::Joined(uid)) => done.push(act("joined", &uid)),
                    None => {}
                },
                ("task", "mark-done") => {
                    if let Some(uid) = book.close(&seen.call, zone) {
                        done.push(act("done", &uid));
                    }
                }
                _ => {}
            }
        }
    }
    // Each act's digest: its task as written now, which Undo compares with.
    for act in &mut done {
        act.digest = book.text(&crate::links::id_of(&act.result)).map(|text| super::engine::digest(text.as_bytes())).unwrap_or_default();
    }
    Ran { writes: book.writes(), acts: done }
}

/// What a rule about calls would have done with the calls given (`calls`,
/// the last days', oldest first): the calls it acts on. Its own `since` is
/// left aside: the preview asks what it would have done had it been there.
pub fn preview<'a>(rule: &Rule, calls: &'a [Seen], threshold: u32, region: Option<&Region>, zone: &TimeZone) -> Vec<&'a Seen> {
    let rule = Rule { since: 0, ..rule.clone() };
    calls.iter().filter(|seen| acts_on(&rule, seen, threshold, region, zone)).collect()
}

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The rules about calls (`wiring::call`): every behaviour the call-back
//! rules had (`callback`, before calls were objects), now through the engine
//! and its two presets; the presets as your settings say them, the old
//! switches carried into them and back; your own rule copied from a preset;
//! a call's conditions; an older device beside this one.

use super::*;
use crate::callback::{Kept, Mark, Write};
use crate::calls::{HistoryRow, Logged};
use crate::lines;
use crate::notices::State;
use crate::tasks::{self, ContactRef, Status};
use jiff::Zoned;
use std::path::Path;

fn paris() -> TimeZone {
    TimeZone::get("Europe/Paris").unwrap()
}

/// 4 November 2026 at a time, in Paris (ms).
fn at(hhmm: &str) -> i64 {
    format!("2026-11-04T{hhmm}:00+01:00[Europe/Paris]").parse::<Zoned>().unwrap().timestamp().as_millisecond()
}

fn zoned(at_ms: i64, zone: &TimeZone) -> Zoned {
    jiff::Timestamp::from_millisecond(at_ms).unwrap().to_zoned(zone.clone())
}

fn noa() -> ContactRef {
    ContactRef { name: "Noa Linden".into(), uri: "sioul:contact/noa-1".into() }
}

/// A call of the phone's history: Android's row (`kind`, its type), with
/// Noa's card for the number 01 99 00 12 34, none for any other.
fn call(hhmm: &str, kind: i32, seconds: i64, number: &str) -> Call {
    let row = HistoryRow { at: at(hhmm), seconds, kind, number: number.into(), presentation: 1 };
    let logged = Logged { device: "phone-1".into(), ..Logged::of(&row, crate::phones::region_named("FR")).unwrap() };
    let contact = (logged.key == "+33199001234").then(noa);
    Call::of(&logged, contact, State::default())
}

const NOA: &str = "01 99 00 12 34";

/// The two switches Porch ⚙ ▸ Calls had, and the voicemail threshold: what
/// the call-back rules read before calls were objects.
#[derive(Debug, Clone, Copy)]
struct Switches {
    missed: bool,
    unreached: bool,
    threshold: u32,
}

fn rules() -> Switches {
    Switches { missed: true, unreached: true, threshold: 90 }
}

fn tr() -> Translator {
    Translator::new("en")
}

/// Settings as a Sioul before format 3 left them: the two switches, the threshold.
fn older_settings(s: &Switches) -> Config {
    toml::from_str(&format!("[porch]\ncall_back = {}\ncall_again = {}\nvoicemail_seconds = {}\n", s.missed, s.unreached, s.threshold)).unwrap()
}

fn seen(call: Call) -> Seen {
    Seen { call, circle: "neutral", phone_name: "Pocket".into() }
}

/// What the engine writes for these calls, the presets as the switches
/// say (`missed_call_preset` reads them before any preset was written).
fn apply(calls: &[Call], kept: &[Kept], s: &Switches, tr: &Translator, zone: &TimeZone) -> Vec<Write> {
    let config = older_settings(s);
    let rules = rules_here(&config, &Rules::default());
    let seen: Vec<Seen> = calls.iter().cloned().map(seen).collect();
    run(&seen, kept, &rules, &Acts::default(), crate::calls::threshold(&config), crate::phones::region_named("FR"), tr, zone, 0).writes
}

/// The writes as the tasks they leave: each new one with no file yet.
fn kept_after(writes: &[Write], kept: &[Kept]) -> Vec<Kept> {
    let mut out: Vec<Kept> = kept.to_vec();
    for write in writes {
        match write {
            Write::New { uid, text } => out.push(Kept::read(Path::new(&format!("{uid}.ics")), text, &paris()).unwrap()),
            Write::Changed { file, text } => {
                let task = Kept::read(file, text, &paris()).unwrap();
                out.retain(|k| k.uid != task.uid);
                out.push(task);
            }
        }
    }
    out
}

/// Rule 1: a missed call from someone in your address books makes "Call
/// back …", for its day, tied to their card, of the kind "call", with no
/// reminder; nothing for a number in nobody's card, nor with the rule off.
#[test]
fn a_missed_call_makes_a_task_to_call_back() {
    let writes = apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris());
    let [Write::New { uid, text }] = writes.as_slice() else { panic!("{writes:?}") };
    assert_eq!(*uid, format!("sioul-call-{}-{}", crate::notices::digest("sioul:contact/noa-1"), at("09:30")));
    let task = tasks::task_of_text(text, &paris()).unwrap();
    assert_eq!(task.title, "Call back Noa Linden");
    assert_eq!(task.notes, "Missed call at 09:30.");
    assert_eq!(task.due.get(..10), Some("2026-11-04"));
    assert_eq!(task.kind, "call");
    assert_eq!(task.contacts, vec![noa()]);
    assert!(!text.contains("BEGIN:VALARM"), "tasks never nag");
    assert!(text.contains("X-SIOUL-CALL-BACK:sioul:contact/noa-1") && text.contains(&format!("X-SIOUL-CALLS:{}-in", at("09:30"))));
    assert!(!text.contains("99 00") && !text.contains("199001234"), "no number in the task");
    // Stamped with the call: the same bytes whenever it is made.
    assert_eq!(apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris()), writes);
    assert!(apply(&[call("09:30", 3, 0, "04 65 71 00 00")], &[], &rules(), &tr(), &paris()).is_empty(), "in nobody's card");
    assert!(apply(&[call("09:30", 3, 0, NOA)], &[], &Switches { missed: false, ..rules() }, &tr(), &paris()).is_empty(), "the rule off");
}

/// Rule 2: a call of yours that did not reach them makes "Call … again":
/// not answered, or shorter than the voicemail threshold (90 s, or as
/// set); nothing once you spoke, nor with the rule off.
#[test]
fn a_call_that_did_not_reach_them_makes_a_task_to_call_again() {
    for (seconds, threshold, makes) in [(0, 90, true), (40, 90, true), (95, 90, false), (95, 120, true), (65, 60, false)] {
        let writes = apply(&[call("14:10", 2, seconds, NOA)], &[], &Switches { threshold, ..rules() }, &tr(), &paris());
        assert_eq!(writes.len(), usize::from(makes), "{seconds} s, threshold {threshold}");
        if let [Write::New { text, .. }] = writes.as_slice() {
            let task = tasks::task_of_text(text, &paris()).unwrap();
            assert_eq!((task.title.as_str(), task.notes.as_str()), ("Call Noa Linden again", "Your call at 14:10 did not reach them."));
        }
    }
    assert!(apply(&[call("14:10", 2, 0, NOA)], &[], &Switches { unreached: false, ..rules() }, &tr(), &paris()).is_empty());
}

/// One open task per person: another such call adds its time to the
/// task's line, either rule, never a count; a line you wrote yourself
/// stays yours; once the task is done, the next call makes a new one.
#[test]
fn one_open_task_per_person() {
    let first = apply(&[call("09:30", 3, 0, NOA), call("09:41", 3, 0, NOA)], &[], &rules(), &tr(), &paris());
    assert_eq!(first.len(), 1, "{first:?}");
    let kept = kept_after(&first, &[]);
    assert_eq!(tasks::task_of_text(&kept[0].text, &paris()).unwrap().notes, "Missed call at 09:30, also at 09:41.");
    // Later, your call that did not reach them: the same task, its way said.
    let later = apply(&[call("14:10", 2, 0, NOA)], &kept, &rules(), &tr(), &paris());
    let kept = kept_after(&later, &kept);
    assert_eq!(kept.len(), 1);
    let task = tasks::task_of_text(&kept[0].text, &paris()).unwrap();
    assert_eq!((task.title.as_str(), task.notes.as_str()), ("Call back Noa Linden", "Missed call at 09:30, also at 09:41, you called at 14:10."));
    assert_eq!(kept[0].calls.len(), 3);
    // The same call read again: nothing.
    assert!(apply(&[call("14:10", 2, 0, NOA)], &kept, &rules(), &tr(), &paris()).is_empty());
    // The next day: its day said.
    let tomorrow = Call { at: at("09:30") + 86_400_000, id: "next".into(), ..call("09:30", 3, 0, NOA) };
    let next = kept_after(&apply(&[tomorrow], &kept, &rules(), &tr(), &paris()), &kept);
    assert_eq!(tasks::task_of_text(&next[0].text, &paris()).unwrap().notes, "Missed call at 09:30, also at 09:41, you called at 14:10, also on Thu 5 Nov at 09:30.");
    // A line you wrote: kept; the calls' own line still grows.
    let yours = Kept { text: tasks::replace_lines(&kept[0].text, |l| lines::name(l) == "DESCRIPTION", vec!["DESCRIPTION:Ask about Saturday".into()], &zoned(at("15:00"), &paris())).unwrap(), ..kept[0].clone() };
    let after = kept_after(&apply(&[call("16:00", 3, 0, NOA)], &[yours], &rules(), &tr(), &paris()), &[]);
    assert_eq!(tasks::task_of_text(&after[0].text, &paris()).unwrap().notes, "Ask about Saturday");
    assert_eq!(after[0].calls.len(), 4);
    // Done by hand: the next missed call makes a new task.
    let done = Kept { text: tasks::set_status(&kept[0].text, Status::Completed, &paris(), &zoned(at("15:00"), &paris())).unwrap(), open: false, ..kept[0].clone() };
    let again = apply(&[call("16:00", 3, 0, NOA)], &[done], &rules(), &tr(), &paris());
    assert!(matches!(again.as_slice(), [Write::New { .. }]), "{again:?}");
}

/// The task closes by itself once you spoke with them after its first
/// call (a call of theirs you answered, however short; one of yours at
/// least the threshold): done at that call's end, the call named, the
/// same bytes whenever; never by a call before it, nor a short one of
/// yours, and never again once you opened it again.
#[test]
fn the_task_closes_by_itself_once_you_spoke() {
    let kept = kept_after(&apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris()), &[]);
    assert!(apply(&[call("10:00", 2, 40, NOA)], &kept, &rules(), &tr(), &paris()).iter().all(|w| matches!(w, Write::Changed { .. })), "short: the task grows, stays open");
    assert!(apply(&[Call { at: at("09:00"), ..call("09:00", 1, 300, NOA) }], &kept, &rules(), &tr(), &paris()).is_empty(), "a call before the task's first");
    for spoken in [call("11:00", 2, 95, NOA), call("11:00", 1, 300, NOA)] {
        let writes = apply(std::slice::from_ref(&spoken), &kept, &rules(), &tr(), &paris());
        let [Write::Changed { text, .. }] = writes.as_slice() else { panic!("{writes:?}") };
        let task = tasks::task_of_text(text, &paris()).unwrap();
        assert_eq!(task.status, Status::Completed);
        let end = zoned(spoken.at + spoken.seconds * 1000, &paris());
        assert_eq!(task.completed, Some(end.timestamp().as_second()));
        assert!(text.contains(&format!("X-SIOUL-CALLED:{}", Mark::of(&spoken).word())));
        assert_eq!(apply(std::slice::from_ref(&spoken), &kept, &rules(), &tr(), &paris()), writes, "the same bytes");
    }
    // A call of theirs you answered closes it, however short: you picked up.
    let writes = apply(&[call("10:30", 1, 40, NOA)], &kept, &rules(), &tr(), &paris());
    let [Write::Changed { text, .. }] = writes.as_slice() else { panic!("{writes:?}") };
    assert_eq!(tasks::task_of_text(text, &paris()).unwrap().status, Status::Completed, "answered 0:40");
    // Opened again after a call closed it: yours to close; a new missed call still joins it.
    let closed = kept_after(&apply(&[call("11:00", 1, 300, NOA)], &kept, &rules(), &tr(), &paris()), &kept);
    let reopened = Kept { text: tasks::set_status(&closed[0].text, Status::NeedsAction, &paris(), &zoned(at("12:00"), &paris())).unwrap(), open: true, ..closed[0].clone() };
    assert!(reopened.called);
    assert!(apply(&[call("13:00", 1, 300, NOA)], &[reopened.clone()], &rules(), &tr(), &paris()).is_empty(), "never closed again by a call");
    let joined = apply(&[call("13:30", 3, 0, NOA)], &[reopened], &rules(), &tr(), &paris());
    assert!(matches!(joined.as_slice(), [Write::Changed { .. }]), "{joined:?}");
}

/// The presets as your settings say them: `[presets]` once written; the old
/// switches before (and both off is off, both ways kept); "call-closes-task"
/// off leaves a task open after you spoke; "missed-call-task" for one way
/// alone; a change a device before format 3 made to the old switches is
/// carried (they no longer say what the preset wrote for it).
#[test]
fn the_presets_follow_your_settings() {
    let s = rules();
    let missed = call("09:30", 3, 0, NOA);
    let unreached = call("10:00", 2, 0, NOA);
    let run_with = |config: &Config, calls: &[Call], kept: &[Kept]| {
        let rules = rules_here(config, &Rules::default());
        let seen: Vec<Seen> = calls.iter().cloned().map(seen).collect();
        run(&seen, kept, &rules, &Acts::default(), 90, crate::phones::region_named("FR"), &tr(), &paris(), 0).writes
    };
    // Written: theirs only.
    let theirs: Config = toml::from_str("[porch]\ncall_back = true\ncall_again = false\n[presets.missed-call-task]\non = true\nways = [\"in\"]\n").unwrap();
    assert_eq!(missed_call_preset(&theirs).ways, Some(vec!["in".to_string()]));
    assert_eq!(run_with(&theirs, &[missed.clone()], &[]).len(), 1);
    assert!(run_with(&theirs, &[unreached.clone()], &[]).is_empty(), "yours left out");
    // Off: nothing, the ways kept for when it is on again.
    let off: Config = toml::from_str("[porch]\ncall_back = false\ncall_again = false\n[presets.missed-call-task]\non = false\nways = [\"out\"]\n").unwrap();
    assert!(run_with(&off, &[missed.clone(), unreached.clone()], &[]).is_empty());
    assert_eq!(missed_call_preset(&off), crate::config::PresetSettings { on: Some(false), ways: Some(vec!["out".into()]) });
    // A device before format 3 turned "A task to call back" on since: carried, theirs alone.
    let older_word: Config = toml::from_str("[porch]\ncall_back = true\ncall_again = false\n[presets.missed-call-task]\non = false\nways = [\"out\"]\n").unwrap();
    assert_eq!(missed_call_preset(&older_word), crate::config::PresetSettings { on: Some(true), ways: Some(vec!["in".into()]) });
    // Before anything was written: the old switches, both off is off with both ways.
    assert_eq!(missed_call_preset(&older_settings(&Switches { missed: false, unreached: false, ..s })), crate::config::PresetSettings { on: Some(false), ways: Some(vec!["in".into(), "out".into()]) });
    assert!(preset_on(&older_settings(&s), MISSED_CALL_TASK) && preset_on(&older_settings(&s), CALL_CLOSES_TASK), "on unless said");
    // "call-closes-task" off: you spoke, the task stays open.
    let kept = kept_after(&apply(&[missed.clone()], &[], &s, &tr(), &paris()), &[]);
    let open: Config = toml::from_str("[presets.call-closes-task]\non = false\n").unwrap();
    assert!(run_with(&open, &[call("11:00", 1, 300, NOA)], &kept).is_empty());
    assert_eq!(run_with(&older_settings(&s), &[call("11:00", 1, 300, NOA)], &kept).len(), 1, "on, it closes");
}

/// The old switches carried into `[presets.missed-call-task]` once, the
/// format marked 3, the old keys kept as they were for older devices; a
/// preset written already is left alone; setting it writes the old keys it
/// stands for, which an older phone reads.
#[test]
fn the_switches_are_carried_into_the_presets() {
    let dir = std::env::temp_dir().join(format!("sioul-call-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "# mine\nlanguage = \"en\"\n\n[porch]\ncalls = true\ncall_again = false\n").unwrap();
    assert!(crate::config::migrate_presets(&path).unwrap());
    let config = Config::load(&path).unwrap();
    assert_eq!(config.presets.get(MISSED_CALL_TASK), Some(&crate::config::PresetSettings { on: Some(true), ways: Some(vec!["in".into()]) }));
    assert_eq!((config.porch.call_back, config.porch.call_again, config.format), (true, false, Some(crate::config::FORMAT)));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# mine") && !text.contains("call_back"), "an unsaid switch stays unsaid: {text}");
    assert!(!crate::config::migrate_presets(&path).unwrap(), "once");
    // Set in the Presets tab: yours only; an older phone reads "call back" off, "call again" on.
    crate::config::set_preset(&path, MISSED_CALL_TASK, &crate::config::PresetSettings { on: Some(true), ways: Some(vec!["out".into()]) }).unwrap();
    let config = Config::load(&path).unwrap();
    assert_eq!((config.porch.call_back, config.porch.call_again), (false, true));
    assert_eq!(missed_call_preset(&config).ways, Some(vec!["out".to_string()]));
    crate::config::set_preset(&path, MISSED_CALL_TASK, &crate::config::PresetSettings { on: Some(false), ways: Some(vec!["out".into()]) }).unwrap();
    let config = Config::load(&path).unwrap();
    assert_eq!((config.porch.call_back, config.porch.call_again, preset_on(&config, MISSED_CALL_TASK)), (false, false, false));
    crate::config::set_preset(&path, CALL_CLOSES_TASK, &crate::config::PresetSettings { on: Some(false), ways: None }).unwrap();
    assert!(!preset_on(&Config::load(&path).unwrap(), CALL_CLOSES_TASK));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A device on an older Sioul beside this one: the phone that had the call
/// writes its task, whichever Sioul it runs, and the other writes nothing
/// (calls are given to the rules only where they are first read); an older
/// phone's task reads here as this one's, and the next call joins it; the
/// task this one writes reads on an older device by the same lines.
#[test]
fn an_older_device_and_this_one_write_each_task_once() {
    // The task an older phone wrote for a missed call (the same function, its lines unchanged).
    let older = kept_after(&apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris()), &[]);
    let text = &older[0].text;
    assert!(text.contains("X-SIOUL-CALL-BACK:sioul:contact/noa-1") && text.contains(&format!("X-SIOUL-CALLS:{}-in", at("09:30"))));
    // This phone reads it and the next missed call joins it: no second task.
    let joined = apply(&[call("09:45", 3, 0, NOA)], &older, &rules(), &tr(), &paris());
    assert!(matches!(joined.as_slice(), [Write::Changed { .. }]), "{joined:?}");
    // A computer is given no call (it never reads a phone's history): it writes nothing.
    assert!(run(&[], &older, &rules_here(&older_settings(&rules()), &Rules::default()), &Acts::default(), 90, None, &tr(), &paris(), 0).writes.is_empty());
    // The same call given twice (two processes): one task, the same bytes.
    assert_eq!(apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris()), apply(&[call("09:30", 3, 0, NOA)], &[], &rules(), &tr(), &paris()));
    assert!(apply(&[call("09:30", 3, 0, NOA)], &older, &rules(), &tr(), &paris()).is_empty(), "its own call again: nothing");
}

/// Your own rule copied from a preset ("Copy to make my own"): it runs as
/// the preset does, from the moment it was made; beside the preset it makes
/// no second task; its conditions are yours (here: the safe alone).
#[test]
fn a_rule_copied_from_a_preset_runs_like_it() {
    let mut rule = declare::from_preset(MISSED_CALL_TASK).expect("offered");
    assert!(rule.problem().is_none() && rule.when.kind == "call" && rule.then.action == "call-back");
    rule.id = "mine".into();
    rule.on = true;
    rule.since = at("09:00") / 1000;
    rule.when.conditions.push(Condition { field: "circle".into(), test: "is".into(), value: vec!["safe".into()], until: String::new() });
    let safe = |c: Call| Seen { circle: "safe", ..seen(c) };
    let threshold = 90;
    let region = crate::phones::region_named("FR");
    // Alone (the preset off): the safe one's missed call, from its moment.
    let writes = run(&[safe(call("09:30", 3, 0, NOA))], &[], std::slice::from_ref(&rule), &Acts::default(), threshold, region, &tr(), &paris(), 0).writes;
    assert_eq!(writes.len(), 1);
    assert!(run(&[safe(call("08:30", 3, 0, NOA))], &[], std::slice::from_ref(&rule), &Acts::default(), threshold, region, &tr(), &paris(), 0).writes.is_empty(), "before it was made");
    assert!(run(&[seen(call("09:30", 3, 0, NOA))], &[], std::slice::from_ref(&rule), &Acts::default(), threshold, region, &tr(), &paris(), 0).writes.is_empty(), "not safe");
    // Beside the preset: one task.
    let both = vec![rules_here(&older_settings(&rules()), &Rules::default()), vec![rule.clone()]].concat();
    assert_eq!(run(&[safe(call("09:30", 3, 0, NOA))], &[], &both, &Acts::default(), threshold, region, &tr(), &paris(), 0).writes, writes);
    // The preview names what it would have done, its moment aside.
    let calls = [safe(call("08:30", 3, 0, NOA)), seen(call("08:40", 3, 0, NOA)), safe(call("08:50", 1, 300, NOA))];
    assert_eq!(preview(&rule, &calls, threshold, region, &paris()).len(), 1);
}

/// What a rule can read on a call: their card, their number, their circle,
/// which way, answered, how long, its day, weekday and hour, its phone.
#[test]
fn a_call_s_fields_read_as_conditions() {
    let region = crate::phones::region_named("FR");
    let when = |field: &str, test: &str, value: &[&str], until: &str| When { kind: "call".into(), event: "ends".into(), conditions: vec![Condition { field: field.into(), test: test.into(), value: value.iter().map(|v| v.to_string()).collect(), until: until.into() }], ..When::default() };
    let mine = seen(call("14:10", 2, 95, NOA));
    let stranger = Seen { circle: "stranger", ..seen(call("09:30", 3, 0, "04 65 71 00 00")) };
    let yes = |w: When, s: &Seen| holds(&w, s, region, &paris());
    assert!(yes(when("card", "exists", &[], ""), &mine) && yes(when("card", "missing", &[], ""), &stranger));
    assert!(yes(when("card", "is", &["noa-1"], ""), &mine) && !yes(when("card", "is", &["other"], ""), &mine));
    assert!(yes(when("number", "is", &["+33 1 99 00 12 34"], ""), &mine) && yes(when("number", "is-not", &[NOA], ""), &stranger));
    assert!(yes(when("circle", "is", &["known"], ""), &mine) && yes(when("circle", "is", &["stranger"], ""), &stranger) && !yes(when("circle", "is", &["known"], ""), &stranger));
    assert!(yes(when("way", "is", &["out"], ""), &mine) && yes(when("way", "is-not", &["out"], ""), &stranger));
    assert!(yes(when("answered", "is", &["yes"], ""), &mine) && yes(when("answered", "is", &["no"], ""), &stranger));
    assert!(yes(when("length", "above", &["60"], ""), &mine) && !yes(when("length", "above", &["120"], ""), &mine));
    assert!(yes(when("date", "between", &["2026-11-01"], "2026-11-30"), &mine) && !yes(when("date", "after", &["2026-11-04"], ""), &mine));
    assert!(yes(when("weekday", "is", &["wed"], ""), &mine) && yes(when("weekday", "is-not", &["sat,sun"], ""), &mine));
    assert!(yes(when("hour", "after", &["14:00"], ""), &mine) && yes(when("hour", "between", &["22:00"], "10:00"), &stranger) && !yes(when("hour", "between", &["22:00"], "10:00"), &mine));
    assert!(yes(when("phone", "is", &["pocket"], ""), &mine) && yes(when("phone", "is", &["phone-1"], ""), &mine) && !yes(when("phone", "is", &["tablet"], ""), &mine));
    assert_eq!(events(&mine.call, 90), vec!["ends", "is-answered"]);
    assert_eq!(events(&stranger.call, 90), vec!["ends", "is-missed"]);
    assert_eq!(events(&call("10:00", 2, 40, NOA), 90), vec!["ends", "is-missed"], "your short call reached their voicemail");
}

/// Each act is written in the record of acts, as every rule's: a missed call
/// made a task ("task", on the call's address, the task's digest as written),
/// a second one joined it ("joined"), a call in which you spoke closed it
/// ("done"). A rule never acts twice on one call, on any device (the record
/// read). Undo of "task" takes the task out while it is as the rule left it;
/// Undo of "done" opens it again, and no call closes it again.
#[test]
fn a_call_rule_s_acts_are_recorded_and_undone() {
    use crate::vdir::{Collection, Kind};
    use crate::wiring::engine;
    crate::config::stand_ins_only();
    let dir = std::env::temp_dir().join(format!("sioul-call-acts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let list = Collection { kind: Kind::Calendars, account: "local".into(), id: "tasks".into(), dir: dir.join("tasks"), name: "Tasks".into(), color: None, read_only: false, components: vec!["VTODO".into()] };
    let place = engine::Place { device: "phone-1".into(), acts: dir.join("acts"), ties: dir.join("links.toml"), topics: dir.join("topics") };
    let find = |id: &str| (id == "local/tasks").then(|| list.clone());
    let ruled = rules_here(&older_settings(&rules()), &Rules::default());
    let go = |calls: &[Call], kept: &[Kept], acts: &Acts| run(&calls.iter().cloned().map(seen).collect::<Vec<_>>(), kept, &ruled, acts, 90, crate::phones::region_named("FR"), &tr(), &paris(), 1_000);
    // A missed call: one task, one act.
    let first = go(&[call("09:30", 3, 0, NOA)], &[], &Acts::default());
    let [Write::New { uid, text }] = first.writes.as_slice() else { panic!("{:?}", first.writes) };
    let made = Act { list: "local/tasks".into(), ..first.acts[0].clone() };
    assert_eq!((made.act.as_str(), made.rule.as_str(), made.object.as_str()), ("task", "preset:missed-call-task", call("09:30", 3, 0, NOA).address().as_str()));
    assert_eq!((made.result.clone(), made.digest.clone(), made.wrote, made.depth), (crate::links::task_uri(uid), engine::digest(text.as_bytes()), true, 1));
    // Recorded: the same call given again makes nothing, even with no task found.
    let record = Acts::of(first.acts.clone());
    assert!(go(&[call("09:30", 3, 0, NOA)], &[], &record).writes.is_empty());
    // Undo while it is as the rule left it: taken out.
    let path = list.dir.join(format!("{uid}.ics"));
    crate::vdir::store_item(&path, text).unwrap();
    assert!(engine::undo(&made, &place, None, &find, 2_000).unwrap().put_back && !path.exists());
    // Written again; a second missed call joins it, then you spoke: "joined", "done".
    crate::vdir::store_item(&path, text).unwrap();
    let kept = vec![Kept::read(&path, text, &paris()).unwrap()];
    let later = go(&[call("09:45", 3, 0, NOA), call("11:00", 1, 300, NOA)], &kept, &record);
    assert_eq!(later.acts.iter().map(|a| a.act.as_str()).collect::<Vec<_>>(), ["joined", "done"]);
    let [Write::Changed { text: closed, .. }] = later.writes.as_slice() else { panic!("{:?}", later.writes) };
    assert!(later.acts.iter().all(|a| a.digest == engine::digest(closed.as_bytes()) && a.object.starts_with("sioul:call/phone-1:")));
    assert_eq!(later.acts[1].rule, "preset:call-closes-task");
    crate::vdir::store_item(&path, closed).unwrap();
    // "joined" has no Undo of its own; "done"'s opens the task again, its X-SIOUL-CALLED kept.
    assert!(!engine::undo(&Act { list: "local/tasks".into(), ..later.acts[0].clone() }, &place, None, &find, 2_500).unwrap().put_back);
    // The phone's record holds its acts; the Undo is pressed on another device, whose opening
    // travels as the task: the phone never opens it a second time (nothing to take back).
    let listed = |acts: &[Act]| acts.iter().map(|a| Act { list: "local/tasks".into(), ..a.clone() }).collect::<Vec<_>>();
    crate::wiring::acts::append(&place.log(), &listed(&[first.acts.clone(), later.acts.clone()].concat())).unwrap();
    let desk = engine::Place { device: "desk".into(), ..place.clone() };
    assert!(engine::undo(&Act { list: "local/tasks".into(), ..later.acts[1].clone() }, &desk, None, &find, 3_000).unwrap().put_back);
    assert!(Acts::read(&place.acts).to_take_back("phone-1").is_empty(), "the phone opens it no second time");
    let reopened = std::fs::read_to_string(&path).unwrap();
    assert_eq!(tasks::task_of_text(&reopened, &paris()).unwrap().status, Status::NeedsAction);
    assert!(reopened.contains("X-SIOUL-CALLED:"));
    let kept = vec![Kept::read(&path, &reopened, &paris()).unwrap()];
    let all = Acts::of([first.acts.clone(), later.acts.clone()].concat());
    assert!(go(&[call("13:00", 1, 300, NOA)], &kept, &all).writes.is_empty(), "never closed again by a call");
    // The task, changed since it was made: its Undo leaves it.
    assert!(!engine::undo(&made, &place, None, &find, 4_000).unwrap().put_back && path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
